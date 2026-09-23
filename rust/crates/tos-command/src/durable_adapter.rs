//! Private real-byte PostgreSQL/STO laboratory path. No ToS source admission.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use postgres::fallible_iterator::FallibleIterator;
use postgres::{Client, IsolationLevel, NoTls, Transaction};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_segment_store::{
    AttemptRecovery, ByteDurabilityReceipt, PlacementGenerationRowV1, SegmentError, SegmentStore,
    VerificationBudget,
};

const PROFILE_ID: &[u8] = b"cmd2.lab.embedded-revision";
const PROFILE_VERSION: &[u8] = b"1";
const LAB_MAGIC: &[u8; 8] = b"CMD2LAB1";
const COLD_AUDIT_PROFILE: &[u8] = b"cmd2-private-complete-state-v3:pg16-row-to-json:fenced-intent";
const RECEIPT_PROFILE: &[u8] = b"cmd2-receipt-v2:attempt-fence";
const HISTORY_KEY_TAG: &[u8] = b"cmd2-history-key-v1";
const CURRENT_KEY_TAG: &[u8] = b"cmd2-current-key-v1";
const MAX_MEMBERS: usize = 64;
const MAX_CUT: u64 = 100_000;
const MAX_MEMBERSHIP_KEY_BYTES: usize = 4096;
const MAX_TOTAL_MEMBERSHIP_KEY_BYTES: usize = 16 * 1024 * 1024;
const MAX_COLD_AUDIT_ELAPSED: Duration = Duration::from_secs(300);

fn check_cold_deadline(started: Instant) -> DurableResult<()> {
    if started.elapsed() > MAX_COLD_AUDIT_ELAPSED {
        Err(DurableError::Refused("cold audit deadline exceeded"))
    } else {
        Ok(())
    }
}

/// The framed keys are sorted as raw unsigned bytes by STO. The exact
/// PostgreSQL traversal therefore orders by UTF-8 byte length, UTF-8 bytes,
/// then revision, rather than by the database's text collation.
fn membership_key(
    tag: &[u8],
    domain: &str,
    subject: &str,
    revision: Option<u64>,
) -> DurableResult<Vec<u8>> {
    if domain.is_empty() || subject.is_empty() || revision == Some(0) {
        return Err(DurableError::Invalid("membership key identity is empty"));
    }
    if tag.len() + 8 + domain.len() + subject.len() + usize::from(revision.is_some()) * 8
        > MAX_MEMBERSHIP_KEY_BYTES
    {
        return Err(DurableError::Refused("membership key exceeds byte budget"));
    }
    let domain_len = u32::try_from(domain.len())
        .map_err(|_| DurableError::Refused("membership domain key too long"))?;
    let subject_len = u32::try_from(subject.len())
        .map_err(|_| DurableError::Refused("membership subject key too long"))?;
    let mut key = Vec::with_capacity(
        tag.len() + 8 + domain.len() + subject.len() + usize::from(revision.is_some()) * 8,
    );
    key.extend_from_slice(tag);
    key.extend_from_slice(&domain_len.to_be_bytes());
    key.extend_from_slice(domain.as_bytes());
    key.extend_from_slice(&subject_len.to_be_bytes());
    key.extend_from_slice(subject.as_bytes());
    if let Some(revision) = revision {
        key.extend_from_slice(&revision.to_be_bytes());
    }
    Ok(key)
}

fn sort_complete_membership(rows: &mut [PlacementGenerationRowV1]) -> DurableResult<()> {
    rows.sort_unstable_by(|a, b| a.key.cmp(&b.key));
    if rows.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(DurableError::Corrupt("duplicate membership key"));
    }
    Ok(())
}

fn logical_membership_root(tag: &[u8], rows: &[PlacementGenerationRowV1]) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-logical-membership-root-v1");
    part(&mut hasher, tag);
    part(&mut hasher, &(rows.len() as u64).to_be_bytes());
    for row in rows {
        part(&mut hasher, &row.key);
        part(&mut hasher, row.logical_digest.as_bytes());
        part(&mut hasher, &row.logical_length.to_be_bytes());
    }
    hasher.finalize()
}

#[cfg(test)]
mod membership_key_tests {
    use super::{CURRENT_KEY_TAG, HISTORY_KEY_TAG, membership_key};

    #[test]
    fn framed_history_and_current_keys_are_injective_and_byte_ordered() {
        let current_a = membership_key(CURRENT_KEY_TAG, "d", "a", None).unwrap();
        let current_aa = membership_key(CURRENT_KEY_TAG, "d", "aa", None).unwrap();
        let other_domain = membership_key(CURRENT_KEY_TAG, "dd", "a", None).unwrap();
        let history_a1 = membership_key(HISTORY_KEY_TAG, "d", "a", Some(1)).unwrap();
        let history_a2 = membership_key(HISTORY_KEY_TAG, "d", "a", Some(2)).unwrap();
        let history_aa1 = membership_key(HISTORY_KEY_TAG, "d", "aa", Some(1)).unwrap();
        assert!(current_a < current_aa);
        assert_ne!(current_a, other_domain);
        assert!(history_a1 < history_a2);
        assert!(history_a2 < history_aa1);
        assert_ne!(history_a1, current_a);
        assert!(membership_key(HISTORY_KEY_TAG, "d", "a", Some(0)).is_err());
    }
}

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
    pub attempt_fence: u64,
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
    current_members: u64,
    history_membership_root: Digest256,
    current_membership_root: Digest256,
    history_rows: Vec<PlacementGenerationRowV1>,
    current_rows: Vec<PlacementGenerationRowV1>,
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
    pub fn current_members(&self) -> u64 {
        self.current_members
    }
    pub fn history_membership_root(&self) -> Digest256 {
        self.history_membership_root
    }
    pub fn current_membership_root(&self) -> Digest256 {
        self.current_membership_root
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
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"cmd2-coordinator-schema-profile-v3");
    part(&mut hasher, include_bytes!("durable_schema.sql"));
    part(&mut hasher, COLD_AUDIT_PROFILE);
    part(&mut hasher, RECEIPT_PROFILE);
    hasher.finalize()
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
        let mut client = Client::connect(url, NoTls)?;
        let version: i32 = client
            .query_one("SELECT current_setting('server_version_num')::integer", &[])?
            .get(0);
        if version / 10_000 != 16 {
            return Err(DurableError::Refused(
                "CMD2 audit row profile requires PostgreSQL 16",
            ));
        }
        Ok(Self { client })
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

    /// Return the durable registration fence that must be bound into the
    /// synced STO intent. A repeated registration cannot restart an attempt
    /// that already advanced beyond `registered`.
    pub fn register_attempt(&mut self, request: &RegisterShadowAttempt<'_>) -> DurableResult<u64> {
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
        let attempt_fence = if changed == 0 {
            let row = tx.query_opt(
                "SELECT command_id,raw_request_digest,delta_digest,state,attempt_fence
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
            if row.get::<_, String>(3) != "registered" {
                return Err(DurableError::Refused("registered attempt already advanced"));
            }
            as_u64(row.get::<_, i64>(4))?
        } else {
            1
        };
        if attempt_fence == 0 {
            return Err(DurableError::Corrupt("registered attempt fence is zero"));
        }
        tx.commit()?;
        Ok(attempt_fence)
    }

    /// Attach exact STO handles to a registered private attempt. The shared
    /// pin guard protects the pre-transaction verification through this row
    /// transition; no domain sequencer lock is held during byte I/O.
    pub fn attach_ready(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_attempt_fence: u64,
        members: &[DurableShadowMember],
    ) -> DurableResult<()> {
        if expected_attempt_fence == 0 || members.is_empty() || members.len() > MAX_MEMBERS {
            return Err(DurableError::Invalid("invalid member count"));
        }
        let receipts: Vec<_> = members
            .iter()
            .map(|member| member.receipt.clone())
            .collect();
        let _guard = store.verify_and_hold_fenced(
            prepare_id,
            expected_attempt_fence,
            0,
            &receipts,
            verification_budget(receipts.len()),
        )?;
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
            "SELECT state,delta_digest,attempt_fence FROM cmd2_attempt
             WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        let state: String = row.get(0);
        if row.get::<_, String>(1) != delta_digest.to_hex() {
            return Err(DurableError::Conflict(
                "attached delta differs from registration",
            ));
        }
        if as_u64(row.get::<_, i64>(2))? != expected_attempt_fence {
            return Err(DurableError::Conflict("registered attempt fence changed"));
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
        if request.attempt_fence == 0
            || request.receipts.is_empty()
            || request.receipts.len() > MAX_MEMBERS
        {
            return Err(DurableError::Invalid("invalid receipt count"));
        }
        let verification_start = Instant::now();
        let guard = store.verify_and_hold_fenced(
            request.prepare_id,
            request.attempt_fence,
            0,
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
        if as_u64(attempt.get::<_, i64>("attempt_fence"))? != request.attempt_fence {
            return Err(DurableError::Conflict("attempt fence changed after seal"));
        }
        if !replayed && state != "ready" {
            return Err(DurableError::Refused(
                "attempt is not ready at registered fence",
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
        part(&mut receipt_hasher, RECEIPT_PROFILE);
        for value in [
            request.domain.as_bytes(),
            request.prepare_id,
            &request.attempt_fence.to_be_bytes(),
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
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
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
        let stored_attempt_fence = as_u64(attempt.get::<_, i64>("attempt_fence"))?;
        let pre_abort_attempt_fence = if state == "aborted" {
            stored_attempt_fence
                .checked_sub(1)
                .filter(|fence| *fence > 0)
                .ok_or(DurableError::Corrupt("aborted attempt fence invalid"))?
        } else {
            if stored_attempt_fence == 0 {
                return Err(DurableError::Corrupt("registered attempt fence is zero"));
            }
            stored_attempt_fence
        };
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
        // A seal can survive SIGKILL before attach_ready wrote member rows.
        // The durable STO intent discovers that pin by exact prepare ID; the
        // already-committed PostgreSQL attempt fence is the abort authority.
        // No physical lookup or absent receipt alone authorizes abort.
        let recovered = store.recover_attempt_fenced(prepare_id, pre_abort_attempt_fence, 0)?;
        let pin_fenced =
            match recovered {
                Some(AttemptRecovery::Sealed { receipts }) => {
                    let first = receipts
                        .first()
                        .ok_or(DurableError::Corrupt("sealed attempt has no frames"))?;
                    if receipts.iter().any(|r| {
                        r.pin_id() != first.pin_id() || r.fence_epoch() != first.fence_epoch()
                    }) || pin.is_some_and(|(id, fence)| {
                        id != first.pin_id() || fence != first.fence_epoch()
                    }) {
                        return Err(DurableError::Corrupt(
                            "attempt pin differs from member rows",
                        ));
                    }
                    store
                        .abort_uncommitted_fenced(
                            first.pin_id(),
                            prepare_id,
                            pre_abort_attempt_fence,
                            0,
                            first.fence_epoch(),
                        )
                        .is_ok()
                }
                Some(AttemptRecovery::Aborted { pin_id, .. }) => {
                    if pin.is_some_and(|(id, _)| id != pin_id) {
                        return Err(DurableError::Corrupt(
                            "aborted pin differs from member rows",
                        ));
                    }
                    true
                }
                Some(AttemptRecovery::IntentOnly { pin_id })
                | Some(AttemptRecovery::Preparing { pin_id, .. }) => {
                    if pin.is_some_and(|(id, _)| id != pin_id) {
                        return Err(DurableError::Corrupt(
                            "preparing pin differs from member rows",
                        ));
                    }
                    false
                }
                None => {
                    if pin.is_some() {
                        return Err(DurableError::Indeterminate(
                            "member pin has no durable intent",
                        ));
                    }
                    false
                }
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
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
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
        let prepare_id: Vec<u8> = row.get("prepare_id");
        let attempt = self
            .client
            .query_opt(
                "SELECT state,attempt_fence,commit_seq FROM cmd2_attempt
                 WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?
            .ok_or(DurableError::Corrupt("historical attempt absent"))?;
        if attempt.get::<_, String>(0) != "committed"
            || attempt.get::<_, Option<i64>>(2) != Some(row.get("commit_seq"))
        {
            return Err(DurableError::Corrupt("historical attempt not committed"));
        }
        let attempt_fence = as_u64(attempt.get::<_, i64>(1))?;
        let receipts = match store.recover_attempt_fenced(&prepare_id, attempt_fence, 0)? {
            Some(AttemptRecovery::Sealed { receipts }) => receipts,
            _ => return Err(DurableError::Corrupt("historical fenced intent not sealed")),
        };
        if receipts.iter().any(|receipt| receipt.pin_id() != pin_id) {
            return Err(DurableError::Corrupt("historical fenced pin differs"));
        }
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
        if store.custody_domain() != recovered.domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
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
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let started = Instant::now();
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        tx.batch_execute("SET LOCAL statement_timeout = '60s'; SET LOCAL work_mem = '4MB'")?;
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
        // Pre-admit every selected row before any client-side materialization.
        // The PostgreSQL snapshot is stable across this bound and the later
        // integrity traversal; an oversized row never crosses into a Row.
        let mut admitted_rows = 0u64;
        let mut admitted_bytes = 0u64;
        for (table, _) in audited_tables {
            check_cold_deadline(started)?;
            let query = format!(
                "SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0),
                        coalesce(sum(octet_length(row_to_json(t)::text)),0)
                 FROM {table} t WHERE domain=$1"
            );
            let row = tx.query_one(&query, &[&domain])?;
            admitted_rows = admitted_rows
                .checked_add(as_u64(row.get::<_, i64>(0))?)
                .ok_or(DurableError::Refused("cold metadata row count overflow"))?;
            admitted_bytes = admitted_bytes
                .checked_add(as_u64(row.get::<_, i64>(2))?)
                .ok_or(DurableError::Refused("cold metadata byte count overflow"))?;
            if admitted_rows > 100_000
                || row.get::<_, i32>(1) > 1_048_576
                || admitted_bytes > 64 * 1024 * 1024
            {
                return Err(DurableError::Refused(
                    "cold metadata preadmission budget exceeded",
                ));
            }
        }
        let mut recovered_pins = 0usize;
        let mut segment_bytes = 0u64;
        let mut historical_members = 0u64;
        let mut latest: HashMap<String, (u64, Digest256, tos_segment_store::PlacementV1)> =
            HashMap::new();
        let mut latest_subject_bytes = 0usize;
        let mut membership_key_bytes = 0usize;
        let mut history_rows = Vec::new();
        let mut log_hasher = Digest256Hasher::new();
        part(&mut log_hasher, b"cmd2-cold-cut-v1");
        let mut state_hasher = Digest256Hasher::new();
        part(&mut state_hasher, COLD_AUDIT_PROFILE);
        part(&mut state_hasher, domain.as_bytes());
        part(&mut state_hasher, &head.to_be_bytes());
        part(&mut state_hasher, &database_oid.to_be_bytes());
        part(&mut state_hasher, profile_digest.to_hex().as_bytes());
        let mut command_events = 0u64;
        let mut next_log_seq = 1u64;
        loop {
            check_cold_deadline(started)?;
            // Keyset pagination keeps the ordered log bounded in process
            // memory while the same REPEATABLE READ snapshot holds throughout
            // all linked history/receipt/outbox checks.
            let log_rows = tx.query(
                "SELECT commit_seq,event_kind,command_id,delta_digest,members_root
                 FROM cmd2_log WHERE domain=$1 AND commit_seq >= $2 AND commit_seq <= $3
                 ORDER BY commit_seq LIMIT 128",
                &[&domain, &as_i64(next_log_seq)?, &as_i64(head)?],
            )?;
            if log_rows.is_empty() {
                break;
            }
            for log in &log_rows {
                check_cold_deadline(started)?;
                let seq: i64 = log.get(0);
                if seq != as_i64(next_log_seq)? {
                    return Err(DurableError::Corrupt("cold cut sequence gap"));
                }
                next_log_seq = next_log_seq
                    .checked_add(1)
                    .ok_or(DurableError::Corrupt("cold sequence overflow"))?;
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
                    "SELECT * FROM cmd2_history WHERE domain=$1 AND commit_seq=$2
                 ORDER BY member_slot LIMIT 65",
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
                    "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2
                 ORDER BY member_slot LIMIT 65",
                    &[&domain, &receipt.prepare_id],
                )?;
                if history.len() != members.len()
                    || members.is_empty()
                    || members.len() > MAX_MEMBERS
                {
                    return Err(DurableError::Corrupt("cold cut member count differs"));
                }
                if recovered_pins >= 10_000 {
                    return Err(DurableError::Refused("cold pin budget exceeded"));
                }
                let attempt_fence = as_u64(attempt.get::<_, i64>("attempt_fence"))?;
                let sealed =
                    match store.recover_attempt_fenced(&receipt.prepare_id, attempt_fence, 0)? {
                        Some(AttemptRecovery::Sealed { receipts }) => receipts,
                        _ => return Err(DurableError::Corrupt("cold fenced intent not sealed")),
                    };
                let first = sealed
                    .first()
                    .ok_or(DurableError::Corrupt("sealed attempt has no frames"))?;
                if sealed
                    .iter()
                    .any(|candidate| candidate.pin_id() != first.pin_id())
                {
                    return Err(DurableError::Corrupt("compound fenced pin differs"));
                }
                recovered_pins += 1;
                segment_bytes = segment_bytes
                    .checked_add(first.segment_size())
                    .ok_or(DurableError::Refused("cold byte budget overflow"))?;
                if segment_bytes > 256 * 1024 * 1024 {
                    return Err(DurableError::Refused("cold byte budget exceeded"));
                }
                let mut member_hasher = Digest256Hasher::new();
                part(&mut member_hasher, b"cmd2-member-root-v1");
                part(&mut member_hasher, &(members.len() as u64).to_be_bytes());
                for (member, historical) in members.iter().zip(history.iter()) {
                    check_cold_deadline(started)?;
                    let slot: i32 = member.get("member_slot");
                    if historical.get::<_, i32>("member_slot") != slot
                        || historical.get::<_, Vec<u8>>("prepare_id") != receipt.prepare_id
                        || historical.get::<_, String>("subject")
                            != member.get::<_, String>("subject")
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
                    if pin_id != first.pin_id() {
                        return Err(DurableError::Corrupt("cold history fenced pin differs"));
                    }
                    let selected = sealed
                        .iter()
                        .find(|candidate| {
                            candidate.receipt_id().to_hex()
                                == historical.get::<_, String>("sto_receipt_id")
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
                    let coordinate = selected.coordinate();
                    let key = membership_key(HISTORY_KEY_TAG, domain, &subject, Some(revision))?;
                    membership_key_bytes = membership_key_bytes
                        .checked_add(key.len())
                        .ok_or(DurableError::Refused("membership key byte count overflow"))?;
                    if membership_key_bytes > MAX_TOTAL_MEMBERSHIP_KEY_BYTES
                        || history_rows.len() >= MAX_CUT as usize
                    {
                        return Err(DurableError::Refused("history membership budget exceeded"));
                    }
                    history_rows.push(PlacementGenerationRowV1 {
                        key,
                        logical_digest: coordinate.sha256,
                        logical_length: coordinate.size_bytes,
                        placement: selected.placement(),
                    });
                    if latest
                        .get(&subject)
                        .is_some_and(|(previous, _, _)| *previous >= revision)
                    {
                        return Err(DurableError::Corrupt(
                            "historical revision is not monotonic",
                        ));
                    }
                    if !latest.contains_key(&subject) {
                        latest_subject_bytes = latest_subject_bytes
                            .checked_add(subject.len())
                            .ok_or(DurableError::Refused("subject byte count overflow"))?;
                        if latest_subject_bytes > 16 * 1024 * 1024
                            || latest.len() >= MAX_CUT as usize
                        {
                            return Err(DurableError::Refused("current identity budget exceeded"));
                        }
                    }
                    latest.insert(subject, (revision, commitment, selected.placement()));
                    historical_members += 1;
                }
                if member_hasher.finalize() != receipt.member_root {
                    return Err(DurableError::Corrupt("cold command member root differs"));
                }
            }
        }
        if next_log_seq != head + 1 {
            return Err(DurableError::Corrupt("cold cut log has a gap"));
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
        let mut current = tx.query_raw("SELECT * FROM cmd2_current WHERE domain=$1", &[&domain])?;
        let mut current_members = 0usize;
        let mut current_rows = Vec::new();
        while let Some(row) = current.next()? {
            check_cold_deadline(started)?;
            current_members = current_members
                .checked_add(1)
                .ok_or(DurableError::Refused("current member count overflow"))?;
            if current_members > MAX_CUT as usize {
                return Err(DurableError::Refused("current member budget exceeded"));
            }
            let subject: String = row.get("subject");
            let revision = as_u64(row.get("revision"))?;
            let Some((latest_revision, latest_digest, placement)) = latest.get(&subject) else {
                return Err(DurableError::Corrupt("current member has no history"));
            };
            if *latest_revision != revision || *latest_digest != metadata_locator_digest(&row) {
                return Err(DurableError::Corrupt(
                    "current locator differs from latest history",
                ));
            }
            let key = membership_key(CURRENT_KEY_TAG, domain, &subject, None)?;
            membership_key_bytes = membership_key_bytes
                .checked_add(key.len())
                .ok_or(DurableError::Refused("membership key byte count overflow"))?;
            if membership_key_bytes > MAX_TOTAL_MEMBERSHIP_KEY_BYTES {
                return Err(DurableError::Refused("current membership budget exceeded"));
            }
            current_rows.push(PlacementGenerationRowV1 {
                key,
                logical_digest: placement.coordinate().sha256,
                logical_length: placement.coordinate().size_bytes,
                placement: *placement,
            });
        }
        drop(current);
        if current_members != latest.len() {
            return Err(DurableError::Corrupt(
                "current membership differs from history",
            ));
        }
        if history_rows.len() as u64 != historical_members || current_rows.len() != current_members
        {
            return Err(DurableError::Corrupt(
                "membership stream cardinality differs",
            ));
        }
        sort_complete_membership(&mut history_rows)?;
        sort_complete_membership(&mut current_rows)?;
        let history_membership_root = logical_membership_root(HISTORY_KEY_TAG, &history_rows);
        let current_membership_root = logical_membership_root(CURRENT_KEY_TAG, &current_rows);
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
        let mut audited_rows = 0usize;
        let mut audited_metadata_bytes = 0usize;
        for (table, order) in audited_tables {
            check_cold_deadline(started)?;
            part(&mut state_hasher, table.as_bytes());
            let query = format!(
                "SELECT row_to_json(t)::text FROM {table} t WHERE domain=$1 ORDER BY {order}"
            );
            let mut rows = tx.query_raw(&query, &[&domain])?;
            let mut table_rows = 0u64;
            while let Some(row) = rows.next()? {
                check_cold_deadline(started)?;
                audited_rows = audited_rows
                    .checked_add(1)
                    .ok_or(DurableError::Refused("cold metadata row count overflow"))?;
                if audited_rows > 100_000 {
                    return Err(DurableError::Refused("cold metadata row budget exceeded"));
                }
                table_rows += 1;
                let encoded: String = row.get(0);
                if encoded.len() > 1_048_576 {
                    return Err(DurableError::Refused(
                        "cold metadata row exceeds byte budget",
                    ));
                }
                audited_metadata_bytes = audited_metadata_bytes
                    .checked_add(encoded.len())
                    .ok_or(DurableError::Refused("cold metadata byte count overflow"))?;
                if audited_metadata_bytes > 64 * 1024 * 1024 {
                    return Err(DurableError::Refused("cold metadata byte budget exceeded"));
                }
                part(&mut state_hasher, encoded.as_bytes());
            }
            part(&mut state_hasher, &table_rows.to_be_bytes());
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
            current_members: current_members as u64,
            history_membership_root,
            current_membership_root,
            history_rows,
            current_rows,
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
    let attempt_fence = as_u64(attempt.get::<_, i64>("attempt_fence"))?;
    if attempt_fence == 0 {
        return Err(DurableError::Corrupt("committed attempt fence is zero"));
    }
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
    part(&mut receipt_hasher, RECEIPT_PROFILE);
    for value in [
        domain.as_bytes(),
        &prepare_id,
        &attempt_fence.to_be_bytes(),
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
