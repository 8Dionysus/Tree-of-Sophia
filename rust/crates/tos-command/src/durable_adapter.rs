//! Private real-byte PostgreSQL/STO laboratory path. No ToS source admission.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use postgres::{Client, IsolationLevel, NoTls, Transaction};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_segment_store::{ByteDurabilityReceipt, SegmentError, SegmentStore, VerificationBudget};

const PROFILE_ID: &[u8] = b"cmd2.lab.embedded-revision";
const PROFILE_VERSION: &[u8] = b"1";
const LAB_MAGIC: &[u8; 8] = b"CMD2LAB1";
const MAX_MEMBERS: usize = 64;
const MAX_CUT: u64 = 100_000;

#[derive(Debug)]
pub enum DurableError {
    Database(postgres::Error),
    Storage(SegmentError),
    Invalid(&'static str),
    Conflict(&'static str),
    Refused(&'static str),
    Corrupt(&'static str),
    Indeterminate(&'static str),
}

impl fmt::Display for DurableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database: {error}"),
            Self::Storage(error) => write!(f, "storage: {error}"),
            Self::Invalid(reason) => write!(f, "invalid input: {reason}"),
            Self::Conflict(reason) => write!(f, "conflict: {reason}"),
            Self::Refused(reason) => write!(f, "refused: {reason}"),
            Self::Corrupt(reason) => write!(f, "corrupt metadata: {reason}"),
            Self::Indeterminate(reason) => write!(f, "indeterminate: {reason}"),
        }
    }
}

impl std::error::Error for DurableError {}

impl From<postgres::Error> for DurableError {
    fn from(error: postgres::Error) -> Self {
        Self::Database(error)
    }
}

impl From<SegmentError> for DurableError {
    fn from(error: SegmentError) -> Self {
        Self::Storage(error)
    }
}

pub type DurableResult<T> = std::result::Result<T, DurableError>;

#[derive(Clone, Debug)]
pub struct DurableShadowMember {
    pub member_slot: u32,
    pub subject: String,
    pub expected_predecessor: Option<(u64, Digest256)>,
    pub proposed_revision: u64,
    pub exact_bytes: Vec<u8>,
    pub receipt: ByteDurabilityReceipt,
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowWriteIdentity<'a> {
    pub member_slot: u32,
    pub subject: &'a str,
    pub expected_predecessor: Option<(u64, Digest256)>,
    pub proposed_revision: u64,
    pub exact_bytes: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct RegisterShadowAttempt<'a> {
    pub domain: &'a str,
    pub prepare_id: &'a [u8],
    pub command_id: &'a str,
    pub raw_request_digest: Digest256,
    pub delta_digest: Digest256,
}

#[derive(Clone, Debug)]
pub struct CommitShadowAttempt<'a> {
    pub domain: &'a str,
    pub prepare_id: &'a [u8],
    pub receipts: &'a [ByteDurabilityReceipt],
    pub expected_contract_digest: Digest256,
    pub expected_rule_version: u64,
    pub expected_rights_version: u64,
    pub job_id: &'a str,
    pub job_fence: u64,
    pub full_base_seq: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableCommitReceipt {
    pub domain: String,
    pub prepare_id: Vec<u8>,
    pub command_id: String,
    pub commit_seq: u64,
    pub raw_request_digest: Digest256,
    pub delta_digest: Digest256,
    pub member_root: Digest256,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttemptResolution {
    Registered,
    Ready,
    Aborted,
    Committed(DurableCommitReceipt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CancelOutcome {
    Cancelled { pin_fenced: bool },
    AlreadyAborted,
    AlreadyCommitted(DurableCommitReceipt),
}

#[derive(Clone, Debug)]
pub struct DurableTiming {
    pub verification: Duration,
    pub lock_wait: Duration,
    pub lock_held: Duration,
    pub transaction: Duration,
}

/// Opaque cold-verified placement. It does not grant disclosure: each warm
/// selected read checks current rights under the coordinator's read lock.
#[derive(Clone, Debug)]
pub struct ColdRecoveredMember {
    domain: String,
    subject: String,
    revision: u64,
    receipt: ByteDurabilityReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColdCut {
    domain: String,
    through_commit_seq: u64,
    log_digest: Digest256,
    state_digest: Digest256,
    schema_profile_digest: Digest256,
    database_oid: u64,
    audit_generation: u64,
    historical_members: u64,
}

impl ColdCut {
    pub fn through_commit_seq(&self) -> u64 {
        self.through_commit_seq
    }
    pub fn log_digest(&self) -> Digest256 {
        self.log_digest
    }
    pub fn state_digest(&self) -> Digest256 {
        self.state_digest
    }
    pub fn audit_generation(&self) -> u64 {
        self.audit_generation
    }
    pub fn historical_members(&self) -> u64 {
        self.historical_members
    }
}

impl ColdRecoveredMember {
    pub fn digest(&self) -> Digest256 {
        self.receipt.coordinate().sha256
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// A deliberately non-source test profile. The exact bytes embed the proposed
/// owner revision; no post-seal metadata assignment may silently renumber it.
pub fn lab_record_bytes(subject: &str, revision: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(18 + subject.len() + payload.len());
    bytes.extend_from_slice(LAB_MAGIC);
    bytes.extend_from_slice(&revision.to_be_bytes());
    bytes.extend_from_slice(&(subject.len() as u16).to_be_bytes());
    bytes.extend_from_slice(subject.as_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn check_lab_record(bytes: &[u8], subject: &str, revision: u64) -> DurableResult<()> {
    if bytes.len() < 18 || &bytes[..8] != LAB_MAGIC {
        return Err(DurableError::Invalid("lab record envelope differs"));
    }
    let embedded_revision = u64::from_be_bytes(
        bytes[8..16]
            .try_into()
            .map_err(|_| DurableError::Invalid("lab revision absent"))?,
    );
    let subject_len = u16::from_be_bytes(
        bytes[16..18]
            .try_into()
            .map_err(|_| DurableError::Invalid("lab subject length absent"))?,
    ) as usize;
    let end = 18usize
        .checked_add(subject_len)
        .ok_or(DurableError::Invalid("lab subject length overflow"))?;
    if end > bytes.len() || &bytes[18..end] != subject.as_bytes() || embedded_revision != revision {
        return Err(DurableError::Invalid(
            "embedded subject or revision differs from proposal",
        ));
    }
    Ok(())
}

/// Commitment over the exact proposed, owner-checked members. This is a
/// synthetic lab delta, not a legacy source command input profile.
pub fn durable_shadow_delta(members: &[DurableShadowMember]) -> Digest256 {
    let identities: Vec<_> = members
        .iter()
        .map(|member| ShadowWriteIdentity {
            member_slot: member.member_slot,
            subject: &member.subject,
            expected_predecessor: member.expected_predecessor,
            proposed_revision: member.proposed_revision,
            exact_bytes: &member.exact_bytes,
        })
        .collect();
    durable_shadow_delta_prepared(&identities)
}

pub fn durable_shadow_delta_prepared(members: &[ShadowWriteIdentity<'_>]) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-lab-delta-v1");
    part(&mut hasher, &(members.len() as u64).to_be_bytes());
    for member in members {
        part(&mut hasher, &member.member_slot.to_be_bytes());
        part(&mut hasher, member.subject.as_bytes());
        part(&mut hasher, &member.proposed_revision.to_be_bytes());
        match member.expected_predecessor {
            Some((version, digest)) => {
                part(&mut hasher, b"present");
                part(&mut hasher, &version.to_be_bytes());
                part(&mut hasher, digest.as_bytes());
            }
            None => part(&mut hasher, b"absent"),
        }
        part(
            &mut hasher,
            Digest256::of_bytes(member.exact_bytes).as_bytes(),
        );
        part(
            &mut hasher,
            &(member.exact_bytes.len() as u64).to_be_bytes(),
        );
    }
    hasher.finalize()
}

fn part(hasher: &mut Digest256Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn update_member_root(hasher: &mut Digest256Hasher, row: &postgres::Row) {
    let slot: i32 = row.get("member_slot");
    let subject: String = row.get("subject");
    let revision: i64 = row.get("proposed_revision");
    let receipt_id: String = row.get("sto_receipt_id");
    let content_digest: String = row.get("content_digest");
    let content_length: i64 = row.get("content_length");
    part(hasher, &slot.to_be_bytes());
    part(hasher, subject.as_bytes());
    part(hasher, &revision.to_be_bytes());
    part(hasher, receipt_id.as_bytes());
    part(hasher, content_digest.as_bytes());
    part(hasher, &content_length.to_be_bytes());
}

fn metadata_locator_digest(row: &postgres::Row) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-metadata-locator-v1");
    for column in [
        "domain",
        "subject",
        "profile_id",
        "profile_version",
        "content_digest",
        "custody_domain_digest",
        "segment_digest",
        "frame_digest",
        "sto_receipt_id",
        "durability_class",
    ] {
        part(&mut hasher, row.get::<_, String>(column).as_bytes());
    }
    for column in ["prepare_id", "store_id", "custody_domain", "pin_id"] {
        part(&mut hasher, &row.get::<_, Vec<u8>>(column));
    }
    for column in [
        "revision",
        "commit_seq",
        "content_length",
        "pin_fence",
        "segment_size",
        "frame_header_offset",
        "frame_length",
    ] {
        part(&mut hasher, &row.get::<_, i64>(column).to_be_bytes());
    }
    for column in ["member_slot", "frame_index"] {
        part(&mut hasher, &row.get::<_, i32>(column).to_be_bytes());
    }
    hasher.finalize()
}

fn as_i64(value: u64) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Invalid("number exceeds PostgreSQL bigint"))
}

fn as_u64(value: i64) -> DurableResult<u64> {
    u64::try_from(value).map_err(|_| DurableError::Corrupt("negative metadata number"))
}

fn parse_hex(value: String) -> DurableResult<Digest256> {
    Digest256::from_hex(&value).map_err(|_| DurableError::Corrupt("invalid metadata digest"))
}

fn schema_profile_digest() -> Digest256 {
    Digest256::of_bytes(include_bytes!("durable_schema.sql"))
}

fn lock_audit_fence(tx: &mut Transaction<'_>, domain: &str) -> DurableResult<u64> {
    let row = tx
        .query_opt(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?
        .ok_or(DurableError::Corrupt("domain audit fence absent"))?;
    if row.get::<_, String>(1) != "normal" {
        return Err(DurableError::Refused("physical maintenance in progress"));
    }
    as_u64(row.get(0))
}

fn database_oid(tx: &mut Transaction<'_>) -> DurableResult<u64> {
    let oid: i64 = tx
        .query_one(
            "SELECT oid::bigint FROM pg_database WHERE datname=current_database()",
            &[],
        )?
        .get(0);
    as_u64(oid)
}

pub struct DurablePgCoordinator {
    client: Client,
}

impl DurablePgCoordinator {
    pub fn connect(url: &str) -> DurableResult<Self> {
        Ok(Self {
            client: Client::connect(url, NoTls)?,
        })
    }

    pub fn backend_pid(&mut self) -> DurableResult<i32> {
        Ok(self
            .client
            .query_one("SELECT pg_backend_pid()", &[])?
            .get(0))
    }

    pub fn init_lab_schema(&mut self) -> DurableResult<()> {
        self.client
            .batch_execute(include_str!("durable_schema.sql"))?;
        Ok(())
    }

    pub fn create_domain(&mut self, domain: &str, contract_digest: Digest256) -> DurableResult<()> {
        if domain.is_empty() {
            return Err(DurableError::Invalid("empty domain"));
        }
        self.client.execute(
            "INSERT INTO cmd2_domain(domain,contract_digest,schema_profile_digest)
             VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
            &[
                &domain,
                &contract_digest.to_hex(),
                &schema_profile_digest().to_hex(),
            ],
        )?;
        Ok(())
    }

    pub fn set_job_epoch(&mut self, domain: &str, job_id: &str, epoch: u64) -> DurableResult<()> {
        let mut tx = self.client.transaction()?;
        lock_audit_fence(&mut tx, domain)?;
        let changed = tx.execute(
            "INSERT INTO cmd2_job(domain,job_id,fence_epoch) VALUES($1,$2,$3)
             ON CONFLICT(domain,job_id) DO UPDATE SET fence_epoch=EXCLUDED.fence_epoch
             WHERE EXCLUDED.fence_epoch >= cmd2_job.fence_epoch",
            &[&domain, &job_id, &as_i64(epoch)?],
        )?;
        if changed != 1 {
            return Err(DurableError::Refused("job fence cannot decrease"));
        }
        tx.commit()?;
        Ok(())
    }

    pub fn register_attempt(&mut self, request: &RegisterShadowAttempt<'_>) -> DurableResult<()> {
        if request.domain.is_empty()
            || request.prepare_id.is_empty()
            || request.command_id.is_empty()
        {
            return Err(DurableError::Invalid("empty attempt identity"));
        }
        let mut tx = self.client.transaction()?;
        lock_audit_fence(&mut tx, request.domain)?;
        let changed = tx.execute(
            "INSERT INTO cmd2_attempt(domain,prepare_id,command_id,raw_request_digest,delta_digest,
             state,attempt_fence) VALUES($1,$2,$3,$4,$5,'registered',1)
             ON CONFLICT DO NOTHING",
            &[
                &request.domain,
                &request.prepare_id,
                &request.command_id,
                &request.raw_request_digest.to_hex(),
                &request.delta_digest.to_hex(),
            ],
        )?;
        if changed == 0 {
            let row = tx.query_opt(
                "SELECT command_id,raw_request_digest,delta_digest,state
                 FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
                &[&request.domain, &request.prepare_id],
            )?;
            let Some(row) = row else {
                return Err(DurableError::Conflict(
                    "command ID already belongs to a different prepare ID",
                ));
            };
            if row.get::<_, String>(0) != request.command_id
                || row.get::<_, String>(1) != request.raw_request_digest.to_hex()
                || row.get::<_, String>(2) != request.delta_digest.to_hex()
            {
                return Err(DurableError::Conflict("attempt identity collision"));
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Attach exact STO handles to a registered private attempt. The shared
    /// pin guard protects the pre-transaction verification through this row
    /// transition; no domain sequencer lock is held during byte I/O.
    pub fn attach_ready(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        members: &[DurableShadowMember],
    ) -> DurableResult<()> {
        if members.is_empty() || members.len() > MAX_MEMBERS {
            return Err(DurableError::Invalid("invalid member count"));
        }
        let receipts: Vec<_> = members
            .iter()
            .map(|member| member.receipt.clone())
            .collect();
        let _guard = store.verify_and_hold(&receipts, verification_budget(receipts.len()))?;
        let first = &members[0].receipt;
        let mut slots = std::collections::HashSet::new();
        let mut subjects = std::collections::HashSet::new();
        for member in members {
            check_shadow_member(domain, prepare_id, member)?;
            if !slots.insert(member.member_slot) || !subjects.insert(member.subject.as_str()) {
                return Err(DurableError::Invalid("duplicate member slot or subject"));
            }
            if member.receipt.pin_id() != first.pin_id()
                || member.receipt.fence_epoch() != first.fence_epoch()
                || member.receipt.store_id() != first.store_id()
            {
                return Err(DurableError::Invalid("compound members use different pins"));
            }
        }
        let delta_digest = durable_shadow_delta(members);
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one(
            "SELECT state,delta_digest FROM cmd2_attempt
             WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        let state: String = row.get(0);
        if row.get::<_, String>(1) != delta_digest.to_hex() {
            return Err(DurableError::Conflict(
                "attached delta differs from registration",
            ));
        }
        if state == "ready" {
            let existing = tx.query(
                "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?;
            if existing.len() != members.len() {
                return Err(DurableError::Conflict("ready member count differs"));
            }
            for member in members {
                let row = existing
                    .iter()
                    .find(|row| row.get::<_, i32>("member_slot") == member.member_slot as i32)
                    .ok_or(DurableError::Conflict("ready member slot differs"))?;
                check_member_row(
                    row,
                    &member.receipt,
                    &member.subject,
                    member.proposed_revision,
                )?;
            }
            tx.commit()?;
            return Ok(());
        }
        if state != "registered" {
            return Err(DurableError::Refused("attempt is not attachable"));
        }
        for member in members {
            let receipt = &member.receipt;
            let coordinate = receipt.coordinate();
            let (expected_revision, expected_digest) = match member.expected_predecessor {
                Some((version, digest)) => (Some(as_i64(version)?), Some(digest.to_hex())),
                None => (None, None),
            };
            tx.execute(
                "INSERT INTO cmd2_member (
                  domain,prepare_id,member_slot,profile_id,profile_version,subject,
                  expected_revision,expected_digest,proposed_revision,content_digest,content_length,
                  store_id,custody_domain_digest,custody_domain,pin_id,pin_fence,
                  segment_digest,segment_size,frame_index,frame_header_offset,
                  frame_digest,frame_length,sto_receipt_id,durability_class)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,
                         $17,$18,$19,$20,$21,$22,$23,$24)",
                &[
                    &domain,
                    &prepare_id,
                    &(member.member_slot as i32),
                    &"cmd2.lab.embedded-revision",
                    &"1",
                    &member.subject,
                    &expected_revision,
                    &expected_digest,
                    &as_i64(member.proposed_revision)?,
                    &coordinate.sha256.to_hex(),
                    &as_i64(coordinate.size_bytes)?,
                    &receipt.store_id().as_slice(),
                    &receipt.domain_digest().to_hex(),
                    &receipt.custody_domain(),
                    &receipt.pin_id().as_slice(),
                    &as_i64(receipt.fence_epoch())?,
                    &receipt.segment_digest().to_hex(),
                    &as_i64(receipt.segment_size())?,
                    &(receipt.frame_index() as i32),
                    &as_i64(coordinate.header_offset)?,
                    &coordinate.sha256.to_hex(),
                    &as_i64(coordinate.size_bytes)?,
                    &receipt.receipt_id().to_hex(),
                    &"LinuxFileAndDirectorySyncReopenSha256V1",
                ],
            )?;
        }
        tx.execute(
            "UPDATE cmd2_attempt SET state='ready' WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &prepare_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// One private shadow commit. STO verifies whole pinned segments before
    /// PostgreSQL begins; the guard prevents an exclusive pin abort through
    /// the short attempt-row and sequencer transaction.
    pub fn commit_shadow(
        &mut self,
        store: &SegmentStore,
        request: &CommitShadowAttempt<'_>,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        if request.receipts.is_empty() || request.receipts.len() > MAX_MEMBERS {
            return Err(DurableError::Invalid("invalid receipt count"));
        }
        let verification_start = Instant::now();
        let guard = store.verify_and_hold(
            request.receipts,
            verification_budget(request.receipts.len()),
        )?;
        if guard.prepare_id() != request.prepare_id {
            return Err(DurableError::Conflict("guard prepare ID differs"));
        }
        let verification = verification_start.elapsed();
        let tx_start = Instant::now();
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let lock_start = Instant::now();
        lock_audit_fence(&mut tx, request.domain)?;
        let fence_acquired = Instant::now();
        let attempt = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&request.domain, &request.prepare_id],
        )?;
        let state: String = attempt.get("state");
        let replayed = state == "committed";
        if !replayed && (state != "ready" || attempt.get::<_, i64>("attempt_fence") != 1) {
            return Err(DurableError::Refused(
                "attempt is not ready at original fence",
            ));
        }
        let member_rows = tx.query(
            "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot",
            &[&request.domain, &request.prepare_id],
        )?;
        if member_rows.len() != request.receipts.len() {
            return Err(DurableError::Corrupt("ready member count differs"));
        }
        for row in &member_rows {
            let slot: i32 = row.get("member_slot");
            let receipt = request
                .receipts
                .iter()
                .find(|receipt| receipt.binding().member_slot == slot as u32)
                .ok_or(DurableError::Conflict("receipt member slot missing"))?;
            let subject: String = row.get("subject");
            let revision = as_u64(row.get("proposed_revision"))?;
            check_member_row(row, receipt, &subject, revision)?;
        }
        tx.query_one(
            "SELECT 1 FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&request.domain],
        )?;
        let lock_wait = lock_start.elapsed();
        // The fence is the first serialization lock in this lab profile.
        // Report its full hold duration, including later attempt/domain waits.
        // A fresh READ COMMITTED statement after the row-lock wait observes
        // any rights/rule/sequence decision made by the preceding owner.
        let domain = tx.query_one(
            "SELECT head_seq,rights_version,rights_allowed,rule_version,contract_digest,
                    schema_profile_digest
             FROM cmd2_domain WHERE domain=$1",
            &[&request.domain],
        )?;
        let head = as_u64(domain.get(0))?;
        if head >= MAX_CUT {
            return Err(DurableError::Refused("shadow cut budget exceeded"));
        }
        let rights_version = as_u64(domain.get(1))?;
        let rights_allowed: bool = domain.get(2);
        if !rights_allowed || rights_version != request.expected_rights_version {
            return Err(DurableError::Refused("current local rights changed"));
        }
        if replayed {
            let receipt = receipt_from_committed_attempt(&mut tx, &attempt)?;
            let lock_held = fence_acquired.elapsed();
            tx.commit()
                .map_err(|_| DurableError::Indeterminate("replay transaction outcome unknown"))?;
            drop(guard);
            return Ok((
                DurableCommitReceipt {
                    replayed: true,
                    ..receipt
                },
                DurableTiming {
                    verification,
                    lock_wait,
                    lock_held,
                    transaction: tx_start.elapsed(),
                },
            ));
        }
        if head != request.full_base_seq {
            return Err(DurableError::Conflict(
                "FullOnly base drift; re-audit outside lock",
            ));
        }
        if as_u64(domain.get(3))? != request.expected_rule_version
            || domain.get::<_, String>(4) != request.expected_contract_digest.to_hex()
            || domain.get::<_, Option<String>>(5) != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Conflict(
                "rule/schema/registry contract changed",
            ));
        }
        let lease = tx.query_opt(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR UPDATE",
            &[&request.domain, &request.job_id],
        )?;
        if lease.map(|row| row.get::<_, i64>(0)) != Some(as_i64(request.job_fence)?) {
            return Err(DurableError::Refused("job fence changed"));
        }
        let seq = head
            .checked_add(1)
            .ok_or(DurableError::Corrupt("commit sequence overflow"))?;
        let seq_db = as_i64(seq)?;
        let mut member_hasher = Digest256Hasher::new();
        part(&mut member_hasher, b"cmd2-member-root-v1");
        part(
            &mut member_hasher,
            &(member_rows.len() as u64).to_be_bytes(),
        );
        for row in &member_rows {
            let subject: String = row.get("subject");
            let revision: i64 = row.get("proposed_revision");
            let expected_revision: Option<i64> = row.get("expected_revision");
            let expected_digest: Option<String> = row.get("expected_digest");
            let current = tx.query_opt(
                "SELECT revision,content_digest FROM cmd2_current
                 WHERE domain=$1 AND subject=$2",
                &[&request.domain, &subject],
            )?;
            match (current, expected_revision, expected_digest.as_deref()) {
                (None, None, None) if revision == 1 => {}
                (Some(current), Some(expected), Some(digest))
                    if current.get::<_, i64>(0) == expected
                        && current.get::<_, String>(1) == digest
                        && expected.checked_add(1) == Some(revision) => {}
                _ => return Err(DurableError::Conflict("exact predecessor/revision changed")),
            }
            let slot: i32 = row.get("member_slot");
            update_member_root(&mut member_hasher, row);
            // The source-visible projection and retained exact locator are
            // inserted from the same attached row inside this transaction.
            tx.execute(
                "INSERT INTO cmd2_history (
                  domain,subject,revision,commit_seq,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class)
                 SELECT domain,subject,proposed_revision,$3,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class
                 FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 AND member_slot=$4",
                &[&request.domain, &request.prepare_id, &seq_db, &slot],
            )?;
            tx.execute(
                "DELETE FROM cmd2_current WHERE domain=$1 AND subject=$2",
                &[&request.domain, &subject],
            )?;
            tx.execute(
                "INSERT INTO cmd2_current (
                  domain,subject,revision,commit_seq,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class)
                 SELECT domain,subject,proposed_revision,$3,prepare_id,member_slot,profile_id,profile_version,
                  content_digest,content_length,store_id,custody_domain_digest,custody_domain,pin_id,
                  pin_fence,segment_digest,segment_size,frame_index,frame_header_offset,frame_digest,
                  frame_length,sto_receipt_id,durability_class
                 FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 AND member_slot=$4",
                &[&request.domain, &request.prepare_id, &seq_db, &slot],
            )?;
        }
        let members_root = member_hasher.finalize();
        let command_id: String = attempt.get("command_id");
        let raw_request_digest: String = attempt.get("raw_request_digest");
        let delta_digest: String = attempt.get("delta_digest");
        let mut receipt_hasher = Digest256Hasher::new();
        for value in [
            request.domain.as_bytes(),
            request.prepare_id,
            command_id.as_bytes(),
            &seq.to_be_bytes(),
            raw_request_digest.as_bytes(),
            delta_digest.as_bytes(),
            members_root.as_bytes(),
        ] {
            part(&mut receipt_hasher, value);
        }
        let receipt_digest = receipt_hasher.finalize();
        tx.execute(
            "UPDATE cmd2_domain SET head_seq=$2 WHERE domain=$1",
            &[&request.domain, &seq_db],
        )?;
        tx.execute(
            "INSERT INTO cmd2_receipt(domain,command_id,prepare_id,commit_seq,raw_request_digest,
             delta_digest,receipt_digest,members_root) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &request.domain,
                &command_id,
                &request.prepare_id,
                &seq_db,
                &raw_request_digest,
                &delta_digest,
                &receipt_digest.to_hex(),
                &members_root.to_hex(),
            ],
        )?;
        tx.execute(
            "INSERT INTO cmd2_log(domain,commit_seq,event_kind,command_id,delta_digest,members_root)
             VALUES($1,$2,'command',$3,$4,$5)",
            &[
                &request.domain,
                &seq_db,
                &command_id,
                &delta_digest,
                &members_root.to_hex(),
            ],
        )?;
        let event_id = format!("{}:{seq}", request.domain);
        tx.execute(
            "INSERT INTO cmd2_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&request.domain, &seq_db, &event_id],
        )?;
        tx.execute(
            "UPDATE cmd2_attempt SET state='committed',commit_seq=$3,receipt_digest=$4
             WHERE domain=$1 AND prepare_id=$2",
            &[
                &request.domain,
                &request.prepare_id,
                &seq_db,
                &receipt_digest.to_hex(),
            ],
        )?;
        let lock_held = fence_acquired.elapsed();
        tx.commit()
            .map_err(|_| DurableError::Indeterminate("commit outcome unknown; retain pin"))?;
        drop(guard);
        Ok((
            DurableCommitReceipt {
                domain: request.domain.to_owned(),
                prepare_id: request.prepare_id.to_vec(),
                command_id,
                commit_seq: seq,
                raw_request_digest: parse_hex(raw_request_digest)?,
                delta_digest: parse_hex(delta_digest)?,
                member_root: members_root,
                replayed: false,
            },
            DurableTiming {
                verification,
                lock_wait,
                lock_held,
                transaction: tx_start.elapsed(),
            },
        ))
    }

    pub fn resolve_attempt(
        &mut self,
        domain: &str,
        prepare_id: &[u8],
    ) -> DurableResult<AttemptResolution> {
        let mut tx = self.client.transaction()?;
        lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        let resolution = match row.get::<_, String>("state").as_str() {
            "registered" => AttemptResolution::Registered,
            "ready" => AttemptResolution::Ready,
            "aborted" => AttemptResolution::Aborted,
            "committed" => {
                AttemptResolution::Committed(receipt_from_committed_attempt(&mut tx, &row)?)
            }
            _ => return Err(DurableError::Corrupt("unknown attempt state")),
        };
        tx.commit()?;
        Ok(resolution)
    }

    /// Fence the attempt in PostgreSQL first. The STO exclusive abort happens
    /// only after the transaction has committed and released both DB locks.
    /// A failed STO abort leaves retained forensic bytes and a durable DB
    /// refusal; the caller can retry/reconcile, never infer a safe delete.
    pub fn cancel_attempt(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
    ) -> DurableResult<CancelOutcome> {
        let mut tx = self.client.transaction()?;
        lock_audit_fence(&mut tx, domain)?;
        let attempt = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        let state: String = attempt.get("state");
        if state == "committed" {
            let receipt = receipt_from_committed_attempt(&mut tx, &attempt)?;
            tx.commit()?;
            return Ok(CancelOutcome::AlreadyCommitted(receipt));
        }
        let member_rows = tx.query(
            "SELECT pin_id,pin_fence FROM cmd2_member
             WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &prepare_id],
        )?;
        let pin = if member_rows.is_empty() {
            None
        } else {
            let pin: Vec<u8> = member_rows[0].get(0);
            let fence: i64 = member_rows[0].get(1);
            if pin.len() != 16
                || member_rows
                    .iter()
                    .any(|row| row.get::<_, Vec<u8>>(0) != pin || row.get::<_, i64>(1) != fence)
            {
                return Err(DurableError::Corrupt(
                    "attempt has inconsistent member pins",
                ));
            }
            let mut pin_id = [0u8; 16];
            pin_id.copy_from_slice(&pin);
            Some((pin_id, as_u64(fence)?))
        };
        if state != "aborted" {
            if state != "registered" && state != "ready" {
                return Err(DurableError::Corrupt("unknown attempt state"));
            }
            tx.execute(
                "UPDATE cmd2_attempt SET state='aborted',attempt_fence=attempt_fence+1
                 WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?;
        }
        tx.commit()
            .map_err(|_| DurableError::Indeterminate("cancel fence outcome unknown; retain pin"))?;
        let pin_fenced = match pin {
            None => false,
            Some((pin_id, fence)) => store.abort_uncommitted(pin_id, prepare_id, fence).is_ok(),
        };
        Ok(CancelOutcome::Cancelled { pin_fenced })
    }

    pub fn revoke_local(&mut self, domain: &str) -> DurableResult<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        lock_audit_fence(&mut tx, domain)?;
        tx.query_one(
            "SELECT 1 FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let row = tx.query_one(
            "SELECT head_seq,rights_version FROM cmd2_domain WHERE domain=$1",
            &[&domain],
        )?;
        let seq = as_u64(row.get::<_, i64>(0))?
            .checked_add(1)
            .ok_or(DurableError::Corrupt("sequence overflow"))?;
        if seq > MAX_CUT {
            return Err(DurableError::Refused("shadow cut budget exceeded"));
        }
        let rights_version = as_u64(row.get::<_, i64>(1))?
            .checked_add(1)
            .ok_or(DurableError::Corrupt("rights version overflow"))?;
        tx.execute(
            "UPDATE cmd2_domain SET head_seq=$2,rights_version=$3,rights_allowed=false
             WHERE domain=$1",
            &[&domain, &as_i64(seq)?, &as_i64(rights_version)?],
        )?;
        let event_id = format!("rights.revoke:{seq}");
        let empty = Digest256::of_bytes(b"").to_hex();
        tx.execute(
            "INSERT INTO cmd2_log(domain,commit_seq,event_kind,command_id,delta_digest,members_root)
             VALUES($1,$2,'rights',$3,$4,$4)",
            &[&domain, &as_i64(seq)?, &event_id, &empty],
        )?;
        tx.execute(
            "INSERT INTO cmd2_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&domain, &as_i64(seq)?, &event_id],
        )?;
        tx.commit()?;
        Ok(seq)
    }

    pub fn head_seq(&mut self, domain: &str) -> DurableResult<u64> {
        let value: i64 = self
            .client
            .query_one(
                "SELECT head_seq FROM cmd2_domain WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        as_u64(value)
    }

    pub fn receipt_count(&mut self, domain: &str) -> DurableResult<i64> {
        Ok(self
            .client
            .query_one(
                "SELECT count(*) FROM cmd2_receipt WHERE domain=$1",
                &[&domain],
            )?
            .get(0))
    }

    pub fn history_count(&mut self, domain: &str) -> DurableResult<i64> {
        Ok(self
            .client
            .query_one(
                "SELECT count(*) FROM cmd2_history WHERE domain=$1",
                &[&domain],
            )?
            .get(0))
    }

    /// Full pin and segment verification is an explicit cold job, never an
    /// implicit per-query scan. The returned handle remains private here.
    pub fn cold_recover_exact(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        subject: &str,
        revision: u64,
    ) -> DurableResult<ColdRecoveredMember> {
        let row = self
            .client
            .query_opt(
                "SELECT * FROM cmd2_history
                 WHERE domain=$1 AND subject=$2 AND revision=$3",
                &[&domain, &subject, &as_i64(revision)?],
            )?
            .ok_or(DurableError::Corrupt("committed historical locator absent"))?;
        let pin: Vec<u8> = row.get("pin_id");
        if pin.len() != 16 {
            return Err(DurableError::Corrupt("historical pin ID malformed"));
        }
        let mut pin_id = [0u8; 16];
        pin_id.copy_from_slice(&pin);
        let receipts = store.recover_sealed(pin_id)?;
        let expected_id: String = row.get("sto_receipt_id");
        let receipt = receipts
            .into_iter()
            .find(|receipt| receipt.receipt_id().to_hex() == expected_id)
            .ok_or(DurableError::Corrupt("historical frame receipt absent"))?;
        check_history_locator(&row, &receipt, domain, subject, revision)?;
        Ok(ColdRecoveredMember {
            domain: domain.to_owned(),
            subject: subject.to_owned(),
            revision,
            receipt,
        })
    }

    /// Current local rights stay locked through selected frame verification
    /// and staging. A real external owner needs its own equivalent read fence.
    pub fn warm_read_selected(
        &mut self,
        store: &SegmentStore,
        recovered: &ColdRecoveredMember,
        max_bytes: u64,
    ) -> DurableResult<Vec<u8>> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let allowed: bool = tx
            .query_one(
                "SELECT rights_allowed FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&recovered.domain],
            )?
            .get(0);
        if !allowed {
            return Err(DurableError::Refused("current rights revoked"));
        }
        let row = tx
            .query_opt(
                "SELECT * FROM cmd2_history
                 WHERE domain=$1 AND subject=$2 AND revision=$3",
                &[
                    &recovered.domain,
                    &recovered.subject,
                    &as_i64(recovered.revision)?,
                ],
            )?
            .ok_or(DurableError::Corrupt("selected historical locator absent"))?;
        check_history_locator(
            &row,
            &recovered.receipt,
            &recovered.domain,
            &recovered.subject,
            recovered.revision,
        )?;
        let mut bytes = Vec::new();
        store.read_selected(&recovered.receipt, max_bytes, &mut bytes)?;
        tx.commit()?;
        Ok(bytes)
    }

    /// Offline/full cold job: one MVCC metadata cut, contiguous log and
    /// receipt audit, plus every referenced sealed pin and exact segment.
    /// This lab digest is not a source membership/index completeness seal.
    pub fn cold_verify_cut(
        &mut self,
        store: &SegmentStore,
        domain: &str,
    ) -> DurableResult<ColdCut> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let domain_row = tx.query_one(
            "SELECT d.head_seq,d.schema_profile_digest,f.generation,f.maintenance_state
                 FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain)
                 WHERE d.domain=$1",
            &[&domain],
        )?;
        if domain_row.get::<_, String>(3) != "normal" {
            return Err(DurableError::Refused("physical maintenance is active"));
        }
        let profile_digest = schema_profile_digest();
        if domain_row.get::<_, Option<String>>(1) != Some(profile_digest.to_hex()) {
            return Err(DurableError::Conflict("coordinator schema profile changed"));
        }
        let head = as_u64(domain_row.get::<_, i64>(0))?;
        let audit_generation = as_u64(domain_row.get::<_, i64>(2))?;
        let database_oid = database_oid(&mut tx)?;
        if head > MAX_CUT {
            return Err(DurableError::Refused("cold cut exceeds laboratory budget"));
        }
        let log_rows = tx.query(
            "SELECT commit_seq,event_kind,command_id,delta_digest,members_root
             FROM cmd2_log WHERE domain=$1 AND commit_seq <= $2 ORDER BY commit_seq",
            &[&domain, &as_i64(head)?],
        )?;
        if log_rows.len() as u64 != head {
            return Err(DurableError::Corrupt("cold cut log has a gap"));
        }
        let mut pins: HashMap<[u8; 16], Vec<ByteDurabilityReceipt>> = HashMap::new();
        let mut segment_bytes = 0u64;
        let mut historical_members = 0u64;
        let mut latest: HashMap<String, (u64, Digest256)> = HashMap::new();
        let mut log_hasher = Digest256Hasher::new();
        part(&mut log_hasher, b"cmd2-cold-cut-v1");
        let mut state_hasher = Digest256Hasher::new();
        part(&mut state_hasher, b"cmd2-private-complete-state-v1");
        part(&mut state_hasher, domain.as_bytes());
        part(&mut state_hasher, &head.to_be_bytes());
        part(&mut state_hasher, &database_oid.to_be_bytes());
        part(&mut state_hasher, profile_digest.to_hex().as_bytes());
        let mut command_events = 0u64;
        for (index, log) in log_rows.iter().enumerate() {
            let seq: i64 = log.get(0);
            if seq != index as i64 + 1 {
                return Err(DurableError::Corrupt("cold cut sequence gap"));
            }
            let kind: String = log.get(1);
            let command_id: String = log.get(2);
            let delta_digest: String = log.get(3);
            let members_root: String = log.get(4);
            for bytes in [
                &seq.to_be_bytes()[..],
                kind.as_bytes(),
                command_id.as_bytes(),
                delta_digest.as_bytes(),
                members_root.as_bytes(),
            ] {
                part(&mut log_hasher, bytes);
            }
            let history = tx.query(
                "SELECT * FROM cmd2_history WHERE domain=$1 AND commit_seq=$2 ORDER BY member_slot",
                &[&domain, &seq],
            )?;
            let outbox = tx.query_opt(
                "SELECT event_id FROM cmd2_outbox WHERE domain=$1 AND commit_seq=$2",
                &[&domain, &seq],
            )?;
            let expected_event_id = if kind == "command" {
                format!("{domain}:{seq}")
            } else {
                command_id.clone()
            };
            if outbox.map(|row| row.get::<_, String>(0)) != Some(expected_event_id) {
                return Err(DurableError::Corrupt("cold cut outbox event differs"));
            }
            if kind != "command" {
                if !history.is_empty() {
                    return Err(DurableError::Corrupt("authority event has source history"));
                }
                continue;
            }
            command_events += 1;
            let attempt = tx
                .query_opt(
                    "SELECT * FROM cmd2_attempt WHERE domain=$1 AND command_id=$2",
                    &[&domain, &command_id],
                )?
                .ok_or(DurableError::Corrupt("command event has no attempt"))?;
            if attempt.get::<_, String>("state") != "committed"
                || attempt.get::<_, Option<i64>>("commit_seq") != Some(seq)
            {
                return Err(DurableError::Corrupt("command event attempt not committed"));
            }
            let receipt = receipt_from_committed_attempt(&mut tx, &attempt)?;
            if receipt.delta_digest.to_hex() != delta_digest
                || receipt.member_root.to_hex() != members_root
            {
                return Err(DurableError::Corrupt("command event receipt differs"));
            }
            let members = tx.query(
                "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot",
                &[&domain, &receipt.prepare_id],
            )?;
            if history.len() != members.len() || members.is_empty() || members.len() > MAX_MEMBERS {
                return Err(DurableError::Corrupt("cold cut member count differs"));
            }
            let mut member_hasher = Digest256Hasher::new();
            part(&mut member_hasher, b"cmd2-member-root-v1");
            part(&mut member_hasher, &(members.len() as u64).to_be_bytes());
            for (member, historical) in members.iter().zip(history.iter()) {
                let slot: i32 = member.get("member_slot");
                if historical.get::<_, i32>("member_slot") != slot
                    || historical.get::<_, Vec<u8>>("prepare_id") != receipt.prepare_id
                    || historical.get::<_, String>("subject") != member.get::<_, String>("subject")
                    || historical.get::<_, i64>("revision")
                        != member.get::<_, i64>("proposed_revision")
                {
                    return Err(DurableError::Corrupt(
                        "cold history/member identity differs",
                    ));
                }
                update_member_root(&mut member_hasher, member);
                let pin: Vec<u8> = historical.get("pin_id");
                if pin.len() != 16 {
                    return Err(DurableError::Corrupt("cold history pin malformed"));
                }
                let mut pin_id = [0u8; 16];
                pin_id.copy_from_slice(&pin);
                if !pins.contains_key(&pin_id) {
                    if pins.len() >= 10_000 {
                        return Err(DurableError::Refused("cold pin budget exceeded"));
                    }
                    let receipts = store.recover_sealed(pin_id)?;
                    let first = receipts
                        .first()
                        .ok_or(DurableError::Corrupt("sealed pin has no frames"))?;
                    segment_bytes = segment_bytes
                        .checked_add(first.segment_size())
                        .ok_or(DurableError::Refused("cold byte budget overflow"))?;
                    if segment_bytes > 256 * 1024 * 1024 {
                        return Err(DurableError::Refused("cold byte budget exceeded"));
                    }
                    pins.insert(pin_id, receipts);
                }
                let selected = pins
                    .get(&pin_id)
                    .and_then(|receipts| {
                        receipts.iter().find(|candidate| {
                            candidate.receipt_id().to_hex()
                                == historical.get::<_, String>("sto_receipt_id")
                        })
                    })
                    .ok_or(DurableError::Corrupt("cold committed frame absent"))?;
                check_history_locator(
                    historical,
                    selected,
                    domain,
                    &historical.get::<_, String>("subject"),
                    as_u64(historical.get("revision"))?,
                )?;
                check_member_row(
                    member,
                    selected,
                    &member.get::<_, String>("subject"),
                    as_u64(member.get("proposed_revision"))?,
                )?;
                // The private STO handle was recovered from the actual sealed
                // pin and full segment. Bind its physical placement as well
                // as the corresponding coordinator row into this cut.
                part(&mut state_hasher, &selected.placement().encode());
                let subject: String = historical.get("subject");
                let revision = as_u64(historical.get("revision"))?;
                let commitment = metadata_locator_digest(historical);
                if latest
                    .get(&subject)
                    .is_some_and(|(previous, _)| *previous >= revision)
                {
                    return Err(DurableError::Corrupt(
                        "historical revision is not monotonic",
                    ));
                }
                latest.insert(subject, (revision, commitment));
                historical_members += 1;
            }
            if member_hasher.finalize() != receipt.member_root {
                return Err(DurableError::Corrupt("cold command member root differs"));
            }
        }
        let future_history: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_history WHERE domain=$1 AND commit_seq > $2",
                &[&domain, &as_i64(head)?],
            )?
            .get(0);
        if future_history != 0 {
            return Err(DurableError::Corrupt(
                "history extends beyond committed head",
            ));
        }
        let current = tx.query("SELECT * FROM cmd2_current WHERE domain=$1", &[&domain])?;
        if current.len() != latest.len() {
            return Err(DurableError::Corrupt(
                "current membership differs from history",
            ));
        }
        for row in &current {
            let subject: String = row.get("subject");
            let revision = as_u64(row.get("revision"))?;
            if latest.get(&subject) != Some(&(revision, metadata_locator_digest(row))) {
                return Err(DurableError::Corrupt(
                    "current locator differs from latest history",
                ));
            }
        }
        let receipt_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_receipt WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        let outbox_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_outbox WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        let history_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_history WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        if as_u64(receipt_count)? != command_events
            || as_u64(outbox_count)? != head
            || as_u64(history_count)? != historical_members
        {
            return Err(DurableError::Corrupt("cold cut table membership differs"));
        }
        // This is deliberately an offline metadata scan. The publication
        // transaction later compares only its trigger-maintained generation;
        // no full scan or segment hashing occurs under the sequencer lock.
        // The row encoding is a PostgreSQL-16 laboratory profile, bound by
        // schema_profile_digest and database_oid, not a portable source codec.
        let audited_tables = [
            ("cmd2_job", "job_id"),
            ("cmd2_predicate", "kind,owner,scope,token"),
            ("cmd2_attempt", "prepare_id"),
            ("cmd2_member", "prepare_id,member_slot"),
            ("cmd2_current", "subject"),
            ("cmd2_history", "subject,revision"),
            ("cmd2_receipt", "command_id"),
            ("cmd2_log", "commit_seq"),
            ("cmd2_outbox", "commit_seq"),
        ];
        let mut audited_rows = 0usize;
        for (table, order) in audited_tables {
            part(&mut state_hasher, table.as_bytes());
            let query = format!(
                "SELECT row_to_json(t)::text FROM {table} t WHERE domain=$1 ORDER BY {order} LIMIT 100001"
            );
            let rows = tx.query(&query, &[&domain])?;
            if rows.len() > 100_000 || audited_rows.saturating_add(rows.len()) > 100_000 {
                return Err(DurableError::Refused("cold metadata row budget exceeded"));
            }
            audited_rows += rows.len();
            part(&mut state_hasher, &(rows.len() as u64).to_be_bytes());
            for row in rows {
                let encoded: String = row.get(0);
                if encoded.len() > 1_048_576 {
                    return Err(DurableError::Refused(
                        "cold metadata row exceeds byte budget",
                    ));
                }
                part(&mut state_hasher, encoded.as_bytes());
            }
        }
        let domain_state: String = tx
            .query_one(
                "SELECT row_to_json(d)::text FROM
             (SELECT domain,head_seq,rights_version,rights_allowed,rule_version,
                     contract_digest,schema_profile_digest
              FROM cmd2_domain WHERE domain=$1) d",
                &[&domain],
            )?
            .get(0);
        part(&mut state_hasher, domain_state.as_bytes());
        let cut = ColdCut {
            domain: domain.to_owned(),
            through_commit_seq: head,
            log_digest: log_hasher.finalize(),
            state_digest: state_hasher.finalize(),
            schema_profile_digest: profile_digest,
            database_oid,
            audit_generation,
            historical_members,
        };
        tx.commit()?;
        Ok(cut)
    }

    pub fn seal_shadow_cut(&mut self, cut: &ColdCut) -> DurableResult<()> {
        let mut tx = self.client.transaction()?;
        // The audit fence is the first metadata lock for every laboratory
        // writer. Its generation substitutes for a full scan under this
        // short publication transaction.
        let generation = lock_audit_fence(&mut tx, &cut.domain)?;
        let row = tx.query_one(
            "SELECT head_seq,published_seq,complete_cut_digest,complete_cut_generation,
                    schema_profile_digest FROM cmd2_domain
             WHERE domain=$1 FOR UPDATE",
            &[&cut.domain],
        )?;
        let head = as_u64(row.get::<_, i64>(0))?;
        let published = as_u64(row.get::<_, i64>(1))?;
        let published_digest: Option<String> = row.get(2);
        let published_generation: Option<i64> = row.get(3);
        if row.get::<_, Option<String>>(4) != Some(cut.schema_profile_digest.to_hex())
            || cut.schema_profile_digest != schema_profile_digest()
            || database_oid(&mut tx)? != cut.database_oid
        {
            return Err(DurableError::Conflict(
                "database or coordinator schema changed",
            ));
        }
        if head != cut.through_commit_seq {
            return Err(DurableError::Conflict("audited head advanced; re-audit"));
        }
        if published == cut.through_commit_seq && published_digest.is_some() {
            let sealed_generation = published_generation
                .ok_or(DurableError::Corrupt("published fence generation absent"))?;
            if published_digest != Some(cut.state_digest.to_hex())
                || generation != as_u64(sealed_generation)?
                || !(cut.audit_generation == generation
                    || cut.audit_generation.checked_add(1) == Some(generation))
            {
                return Err(DurableError::Conflict(
                    "published cut identity or fence differs",
                ));
            }
            tx.commit()?;
            return Ok(());
        }
        if published > cut.through_commit_seq || generation != cut.audit_generation {
            return Err(DurableError::Conflict(
                "audited metadata generation changed",
            ));
        }
        let sealed_generation = generation
            .checked_add(1)
            .ok_or(DurableError::Corrupt("audit generation overflow"))?;
        tx.execute(
            "UPDATE cmd2_domain SET published_seq=$2,complete_cut_digest=$3,
                    complete_cut_generation=$4 WHERE domain=$1",
            &[
                &cut.domain,
                &as_i64(cut.through_commit_seq)?,
                &cut.state_digest.to_hex(),
                &as_i64(sealed_generation)?,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn published_seq(&mut self, domain: &str) -> DurableResult<u64> {
        let value: i64 = self
            .client
            .query_one(
                "SELECT published_seq FROM cmd2_domain WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        as_u64(value)
    }
}

fn verification_budget(member_count: usize) -> VerificationBudget {
    VerificationBudget {
        max_receipts: member_count,
        max_segments: 1,
        max_total_segment_bytes: 64 * 1024 * 1024,
    }
}

fn check_shadow_member(
    domain: &str,
    prepare_id: &[u8],
    member: &DurableShadowMember,
) -> DurableResult<()> {
    let receipt = &member.receipt;
    let binding = receipt.binding();
    let coordinate = receipt.coordinate();
    if binding.profile_id != PROFILE_ID
        || binding.profile_version != PROFILE_VERSION
        || binding.subject_key != member.subject.as_bytes()
        || binding.member_slot != member.member_slot
        || receipt.prepare_id() != prepare_id
        || receipt.custody_domain() != domain.as_bytes()
        || coordinate.sha256 != Digest256::of_bytes(&member.exact_bytes)
        || coordinate.size_bytes != member.exact_bytes.len() as u64
    {
        return Err(DurableError::Invalid(
            "STO receipt and member binding differ",
        ));
    }
    if member.proposed_revision == 0
        || match member.expected_predecessor {
            None => member.proposed_revision != 1,
            Some((version, _)) => version.checked_add(1) != Some(member.proposed_revision),
        }
    {
        return Err(DurableError::Invalid(
            "owner revision is not exact successor",
        ));
    }
    check_lab_record(
        &member.exact_bytes,
        &member.subject,
        member.proposed_revision,
    )
}

fn check_member_row(
    row: &postgres::Row,
    receipt: &ByteDurabilityReceipt,
    subject: &str,
    revision: u64,
) -> DurableResult<()> {
    let coordinate = receipt.coordinate();
    if row.get::<_, String>("subject") != subject
        || receipt.binding().profile_id != PROFILE_ID
        || receipt.binding().profile_version != PROFILE_VERSION
        || receipt.binding().subject_key != subject.as_bytes()
        || row.get::<_, i32>("member_slot") != receipt.binding().member_slot as i32
        || row.get::<_, Vec<u8>>("prepare_id") != receipt.prepare_id()
        || row.get::<_, i64>("proposed_revision") != as_i64(revision)?
        || row.get::<_, String>("profile_id") != "cmd2.lab.embedded-revision"
        || row.get::<_, String>("profile_version") != "1"
        || row.get::<_, Vec<u8>>("store_id") != receipt.store_id()
        || row.get::<_, String>("custody_domain_digest") != receipt.domain_digest().to_hex()
        || row.get::<_, Vec<u8>>("custody_domain") != receipt.custody_domain()
        || row.get::<_, Vec<u8>>("pin_id") != receipt.pin_id()
        || row.get::<_, i64>("pin_fence") != as_i64(receipt.fence_epoch())?
        || row.get::<_, String>("segment_digest") != receipt.segment_digest().to_hex()
        || row.get::<_, i64>("segment_size") != as_i64(receipt.segment_size())?
        || row.get::<_, i32>("frame_index") != receipt.frame_index() as i32
        || row.get::<_, i64>("frame_header_offset") != as_i64(coordinate.header_offset)?
        || row.get::<_, String>("frame_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("frame_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("content_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("content_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("sto_receipt_id") != receipt.receipt_id().to_hex()
        || row.get::<_, String>("durability_class") != "LinuxFileAndDirectorySyncReopenSha256V1"
    {
        return Err(DurableError::Conflict(
            "durable member metadata differs from receipt",
        ));
    }
    Ok(())
}

fn receipt_from_committed_attempt(
    tx: &mut Transaction<'_>,
    attempt: &postgres::Row,
) -> DurableResult<DurableCommitReceipt> {
    let domain: String = attempt.get("domain");
    let prepare_id: Vec<u8> = attempt.get("prepare_id");
    let command_id: String = attempt.get("command_id");
    let commit_seq: i64 = attempt.get("commit_seq");
    let receipt_digest: String = attempt.get("receipt_digest");
    let row = tx
        .query_opt(
            "SELECT commit_seq,raw_request_digest,delta_digest,receipt_digest,members_root
             FROM cmd2_receipt WHERE domain=$1 AND command_id=$2 AND prepare_id=$3",
            &[&domain, &command_id, &prepare_id],
        )?
        .ok_or(DurableError::Corrupt("committed attempt has no receipt"))?;
    let raw_request_digest: String = row.get(1);
    let delta_digest: String = row.get(2);
    let members_root: String = row.get(4);
    if row.get::<_, i64>(0) != commit_seq
        || row.get::<_, String>(3) != receipt_digest
        || raw_request_digest != attempt.get::<_, String>("raw_request_digest")
        || delta_digest != attempt.get::<_, String>("delta_digest")
    {
        return Err(DurableError::Corrupt("attempt and receipt differ"));
    }
    let log = tx
        .query_opt(
            "SELECT event_kind,command_id,delta_digest,members_root FROM cmd2_log
             WHERE domain=$1 AND commit_seq=$2",
            &[&domain, &commit_seq],
        )?
        .ok_or(DurableError::Corrupt("committed receipt has no log event"))?;
    if log.get::<_, String>(0) != "command"
        || log.get::<_, String>(1) != command_id
        || log.get::<_, String>(2) != delta_digest
        || log.get::<_, String>(3) != members_root
    {
        return Err(DurableError::Corrupt("receipt and log differ"));
    }
    let outbox = tx.query_opt(
        "SELECT event_id FROM cmd2_outbox WHERE domain=$1 AND commit_seq=$2",
        &[&domain, &commit_seq],
    )?;
    if outbox.map(|row| row.get::<_, String>(0)) != Some(format!("{domain}:{commit_seq}")) {
        return Err(DurableError::Corrupt("committed outbox event differs"));
    }
    let members = tx.query(
        "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot",
        &[&domain, &prepare_id],
    )?;
    if members.is_empty() || members.len() > MAX_MEMBERS {
        return Err(DurableError::Corrupt("committed member count invalid"));
    }
    let mut members_hasher = Digest256Hasher::new();
    part(&mut members_hasher, b"cmd2-member-root-v1");
    part(&mut members_hasher, &(members.len() as u64).to_be_bytes());
    for member in &members {
        update_member_root(&mut members_hasher, member);
    }
    if members_hasher.finalize().to_hex() != members_root {
        return Err(DurableError::Corrupt("committed member root differs"));
    }
    let commit_seq_u64 = as_u64(commit_seq)?;
    let members_root_digest = parse_hex(members_root.clone())?;
    let mut receipt_hasher = Digest256Hasher::new();
    for value in [
        domain.as_bytes(),
        &prepare_id,
        command_id.as_bytes(),
        &commit_seq_u64.to_be_bytes(),
        raw_request_digest.as_bytes(),
        delta_digest.as_bytes(),
        members_root_digest.as_bytes(),
    ] {
        part(&mut receipt_hasher, value);
    }
    if receipt_hasher.finalize().to_hex() != receipt_digest {
        return Err(DurableError::Corrupt("committed receipt digest differs"));
    }
    Ok(DurableCommitReceipt {
        domain,
        prepare_id,
        command_id,
        commit_seq: commit_seq_u64,
        raw_request_digest: parse_hex(raw_request_digest)?,
        delta_digest: parse_hex(delta_digest)?,
        member_root: members_root_digest,
        replayed: false,
    })
}

fn check_history_locator(
    row: &postgres::Row,
    receipt: &ByteDurabilityReceipt,
    domain: &str,
    subject: &str,
    revision: u64,
) -> DurableResult<()> {
    let coordinate = receipt.coordinate();
    if row.get::<_, String>("domain") != domain
        || row.get::<_, String>("subject") != subject
        || receipt.binding().profile_id != PROFILE_ID
        || receipt.binding().profile_version != PROFILE_VERSION
        || receipt.binding().subject_key != subject.as_bytes()
        || row.get::<_, i64>("revision") != as_i64(revision)?
        || row.get::<_, i32>("member_slot") != receipt.binding().member_slot as i32
        || row.get::<_, Vec<u8>>("prepare_id") != receipt.prepare_id()
        || row.get::<_, String>("profile_id") != "cmd2.lab.embedded-revision"
        || row.get::<_, String>("profile_version") != "1"
        || row.get::<_, Vec<u8>>("store_id") != receipt.store_id()
        || row.get::<_, String>("custody_domain_digest") != receipt.domain_digest().to_hex()
        || row.get::<_, Vec<u8>>("custody_domain") != receipt.custody_domain()
        || row.get::<_, Vec<u8>>("pin_id") != receipt.pin_id()
        || row.get::<_, i64>("pin_fence") != as_i64(receipt.fence_epoch())?
        || row.get::<_, String>("segment_digest") != receipt.segment_digest().to_hex()
        || row.get::<_, i64>("segment_size") != as_i64(receipt.segment_size())?
        || row.get::<_, i32>("frame_index") != receipt.frame_index() as i32
        || row.get::<_, i64>("frame_header_offset") != as_i64(coordinate.header_offset)?
        || row.get::<_, String>("frame_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("frame_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("content_digest") != coordinate.sha256.to_hex()
        || row.get::<_, i64>("content_length") != as_i64(coordinate.size_bytes)?
        || row.get::<_, String>("sto_receipt_id") != receipt.receipt_id().to_hex()
        || row.get::<_, String>("durability_class") != "LinuxFileAndDirectorySyncReopenSha256V1"
    {
        return Err(DurableError::Corrupt(
            "committed locator and STO receipt differ",
        ));
    }
    Ok(())
}
