//! Opt-in transaction-coupled metadata audit deltas for private CMD2 successors.
//!
//! This module is deliberately not wired into the V1 coordinator. Its SQL
//! companion is installed only by a separate explicit opt-in route. Callers
//! must use one REPEATABLE READ or SERIALIZABLE PostgreSQL transaction for the
//! interval, selector, and final-row reads.

use postgres::Transaction;
use postgres::fallible_iterator::FallibleIterator;
use postgres::types::{ToSql, Type};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher};

const AUDIT_INTERVAL_PREFLIGHT_SQL: &str = "SELECT count(*),
                coalesce(sum(coalesce(octet_length(old_key),0)
                           + coalesce(octet_length(new_key),0)
                           + coalesce(octet_length(old_commitment),0)
                           + coalesce(octet_length(new_commitment),0)),0),
                coalesce(max(greatest(coalesce(octet_length(old_key),0),
                                     coalesce(octet_length(new_key),0))),0)::bigint
         FROM cmd2_audit_delta_v1
         WHERE domain=$1 AND generation>$2 AND generation<=$3";

const AUDIT_INTERVAL_ROWS_SQL: &str =
    "SELECT generation,table_id,operation,old_key,new_key,old_commitment,new_commitment
         FROM cmd2_audit_delta_v1
         WHERE domain=$1 AND generation>$2 AND generation<=$3
         ORDER BY generation";

const ROW_KEY_PROFILE: &[u8] = b"cmd2-audit-delta-v1:table-u16be,key-len-u32be,key-json-utf8";

/// Table identifiers follow `METADATA_TABLES` order; the semantic domain
/// header is appended as table 11.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(i16)]
pub enum MetadataTable {
    Job = 1,
    Predicate = 2,
    Attempt = 3,
    Member = 4,
    Current = 5,
    History = 6,
    Receipt = 7,
    Log = 8,
    Outbox = 9,
    SourceIndex = 10,
    DomainHeader = 11,
}

impl MetadataTable {
    pub const ALL: [Self; 11] = [
        Self::Job,
        Self::Predicate,
        Self::Attempt,
        Self::Member,
        Self::Current,
        Self::History,
        Self::Receipt,
        Self::Log,
        Self::Outbox,
        Self::SourceIndex,
        Self::DomainHeader,
    ];

    pub const fn id(self) -> i16 {
        self as i16
    }

    pub fn from_id(value: i16) -> Option<Self> {
        Self::ALL.into_iter().find(|table| table.id() == value)
    }

    pub const fn key_arity(self) -> usize {
        match self {
            Self::Job
            | Self::Attempt
            | Self::Current
            | Self::Receipt
            | Self::Log
            | Self::Outbox
            | Self::DomainHeader => 1,
            Self::Predicate => 4,
            Self::Member | Self::History => 2,
            Self::SourceIndex => 3,
        }
    }

    pub const fn sql_name(self) -> &'static str {
        match self {
            Self::Job => "cmd2_job",
            Self::Predicate => "cmd2_predicate",
            Self::Attempt => "cmd2_attempt",
            Self::Member => "cmd2_member",
            Self::Current => "cmd2_current",
            Self::History => "cmd2_history",
            Self::Receipt => "cmd2_receipt",
            Self::Log => "cmd2_log",
            Self::Outbox => "cmd2_outbox",
            Self::SourceIndex => "cmd2_source_index",
            Self::DomainHeader => "cmd2_domain",
        }
    }
}

/// A stable row identity. `key` is the UTF-8 byte sequence returned by
/// `cmd2_audit_delta_v1_row_key(table_id, row_to_json(row))`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StableRowKey {
    pub table: MetadataTable,
    pub key: Vec<u8>,
}

impl StableRowKey {
    pub(crate) fn parse(
        table: MetadataTable,
        key: Vec<u8>,
        max_key_bytes: usize,
    ) -> Result<Self, AuditDeltaError> {
        if key.is_empty() || key.len() > max_key_bytes {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::MalformedStableKey,
            ));
        }
        let value: Value = serde_json::from_slice(&key)
            .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::MalformedStableKey))?;
        let values = value.as_array().ok_or(AuditDeltaError::ColdRequired(
            ColdRequiredReason::MalformedStableKey,
        ))?;
        if values.len() != table.key_arity() {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::MalformedStableKey,
            ));
        }
        for (index, component) in values.iter().enumerate() {
            let is_bytea = matches!(
                (table, index),
                (MetadataTable::Attempt, 0) | (MetadataTable::Member, 0)
            );
            let is_number = matches!(
                (table, index),
                (MetadataTable::Member, 1)
                    | (MetadataTable::History, 1)
                    | (MetadataTable::Log, 0)
                    | (MetadataTable::Outbox, 0)
            );
            if is_number {
                if component.as_i64().is_none() {
                    return Err(AuditDeltaError::ColdRequired(
                        ColdRequiredReason::MalformedStableKey,
                    ));
                }
            } else if let Some(text) = component.as_str() {
                if is_bytea && !valid_pg_bytea_hex(text) {
                    return Err(AuditDeltaError::ColdRequired(
                        ColdRequiredReason::MalformedStableKey,
                    ));
                }
            } else {
                return Err(AuditDeltaError::ColdRequired(
                    ColdRequiredReason::MalformedStableKey,
                ));
            }
        }
        Ok(Self { table, key })
    }

    /// Full-tree key used by an opt-in CMD builder. The table tag and JSON key
    /// bytes are length-framed so a table/key boundary cannot alias another.
    pub fn framed_tree_key(&self) -> Result<Vec<u8>, AuditDeltaError> {
        let key_len = u32::try_from(self.key.len())
            .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::MalformedStableKey))?;
        let mut framed = Vec::with_capacity(ROW_KEY_PROFILE.len() + 2 + 4 + self.key.len());
        framed.extend_from_slice(ROW_KEY_PROFILE);
        framed.extend_from_slice(&self.table.id().to_be_bytes());
        framed.extend_from_slice(&key_len.to_be_bytes());
        framed.extend_from_slice(&self.key);
        Ok(framed)
    }

    fn json_value(&self) -> Result<Value, AuditDeltaError> {
        serde_json::from_slice(&self.key)
            .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::MalformedStableKey))
    }
}

fn valid_pg_bytea_hex(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("\\x") else {
        return false;
    };
    !hex.is_empty() && hex.len() % 2 == 0 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowCommitment(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditDeltaOperation {
    Insert,
    Update,
    Delete,
}

impl AuditDeltaOperation {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "I" => Some(Self::Insert),
            "U" => Some(Self::Update),
            "D" => Some(Self::Delete),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditDelta {
    pub generation: u64,
    pub operation: AuditDeltaOperation,
    pub old_key: Option<StableRowKey>,
    pub new_key: Option<StableRowKey>,
    pub old_commitment: Option<RowCommitment>,
    pub new_commitment: Option<RowCommitment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditDeltaLimits {
    pub max_rows: usize,
    pub max_bytes: usize,
    pub max_key_bytes: usize,
}

/// Caller-owned accounting for the logical audit payload paths.
///
/// These fields accumulate across helper calls and remain populated when a
/// helper returns an error. Interval/final-lookup counts describe rows
/// delivered by the PostgreSQL client and decoded field widths, not protocol
/// overhead or physical pages. Preflight fields are aggregate witnesses only;
/// `preflight_journal_rows_witnessed` is not a count of rows physically scanned.
/// Isolation, profile, fence, deadline, SQL-binding, and other gate/control
/// query traffic is explicitly excluded from these payload totals. No metric
/// estimates physical I/O.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuditDeltaWork {
    pub interval_rows_returned: u64,
    pub interval_logical_bytes_returned: u64,
    pub preflight_witnesses_returned: u64,
    pub preflight_journal_rows_witnessed: u64,
    pub preflight_payload_bytes_witnessed: u64,
    pub preflight_max_key_bytes_witnessed: u64,
    pub final_lookup_key_json_bytes_attempted: u64,
    pub final_lookup_rows_returned: u64,
    pub final_lookup_logical_bytes_returned: u64,
}

impl AuditDeltaWork {
    fn add_interval_row(&mut self) -> Result<(), AuditDeltaError> {
        add_work_counter(&mut self.interval_rows_returned, 1)
    }

    fn add_interval_bytes(&mut self, bytes: usize) -> Result<(), AuditDeltaError> {
        add_work_counter(
            &mut self.interval_logical_bytes_returned,
            work_amount(bytes)?,
        )
    }

    fn add_preflight_witness(&mut self) -> Result<(), AuditDeltaError> {
        add_work_counter(&mut self.preflight_witnesses_returned, 1)
    }

    fn add_preflight_journal_rows(&mut self, rows: u64) -> Result<(), AuditDeltaError> {
        add_work_counter(&mut self.preflight_journal_rows_witnessed, rows)
    }

    fn add_preflight_payload_bytes(&mut self, bytes: u64) -> Result<(), AuditDeltaError> {
        add_work_counter(&mut self.preflight_payload_bytes_witnessed, bytes)
    }

    fn observe_preflight_max_key_bytes(&mut self, bytes: u64) {
        self.preflight_max_key_bytes_witnessed = self.preflight_max_key_bytes_witnessed.max(bytes);
    }

    fn add_final_lookup_key_json_bytes(&mut self, bytes: usize) -> Result<(), AuditDeltaError> {
        add_work_counter(
            &mut self.final_lookup_key_json_bytes_attempted,
            work_amount(bytes)?,
        )
    }

    fn add_final_lookup_row(&mut self) -> Result<(), AuditDeltaError> {
        add_work_counter(&mut self.final_lookup_rows_returned, 1)
    }

    fn add_final_lookup_bytes(&mut self, bytes: usize) -> Result<(), AuditDeltaError> {
        add_work_counter(
            &mut self.final_lookup_logical_bytes_returned,
            work_amount(bytes)?,
        )
    }
}

fn work_amount(bytes: usize) -> Result<u64, AuditDeltaError> {
    u64::try_from(bytes)
        .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::WorkCounterOverflow))
}

fn add_work_counter(counter: &mut u64, amount: u64) -> Result<(), AuditDeltaError> {
    let next = counter
        .checked_add(amount)
        .ok_or(AuditDeltaError::ColdRequired(
            ColdRequiredReason::WorkCounterOverflow,
        ))?;
    *counter = next;
    Ok(())
}

/// Absolute request budget. Reusing the same value across nested helpers
/// prevents a long interval from receiving a fresh timeout at each phase.
#[derive(Clone, Copy, Debug)]
pub struct AuditDeltaControl<'a> {
    pub deadline: Instant,
    pub cancelled: &'a AtomicBool,
}

impl AuditDeltaControl<'_> {
    fn check(self) -> Result<(), AuditDeltaError> {
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::CancelledOrExpired,
            ))
        } else {
            Ok(())
        }
    }

    fn bound_statement_timeout(self, tx: &mut Transaction<'_>) -> Result<(), AuditDeltaError> {
        self.check()?;
        let millis = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .clamp(1, i32::MAX as u128) as i64;
        tx.query_one(
            "SELECT set_config(
               'statement_timeout',
               (LEAST(
                 $1::bigint,
                 CASE WHEN current_setting('statement_timeout')='0' THEN $1::bigint
                      ELSE GREATEST(1,ceil(extract(epoch FROM
                           current_setting('statement_timeout')::interval)*1000)::bigint)
                 END
               ))::text || 'ms',
               true
             )",
            &[&millis],
        )?;
        Ok(())
    }
}

impl AuditDeltaLimits {
    fn validate(self) -> Result<Self, AuditDeltaError> {
        if self.max_rows == 0 || self.max_bytes == 0 || self.max_key_bytes == 0 {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::InvalidLimits,
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColdRequiredReason {
    InvalidBounds,
    InvalidLimits,
    IntervalTooLarge,
    GenerationGap,
    MaintenanceActive,
    RepeatableSnapshotRequired,
    UnknownTable,
    UnknownOperation,
    MalformedStableKey,
    MalformedCommitment,
    ChainDiscontinuity,
    FinalCommitmentMismatch,
    CancelledOrExpired,
    DomainNotActivated,
    ProfileMismatch,
    WorkCounterOverflow,
}

#[derive(Debug)]
pub enum AuditDeltaError {
    ColdRequired(ColdRequiredReason),
    Database(postgres::Error),
}

impl fmt::Display for AuditDeltaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ColdRequired(reason) => write!(formatter, "cold audit required: {reason:?}"),
            Self::Database(error) => write!(formatter, "audit delta database error: {error}"),
        }
    }
}

impl Error for AuditDeltaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::ColdRequired(_) => None,
        }
    }
}

impl From<postgres::Error> for AuditDeltaError {
    fn from(value: postgres::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalSelector {
    pub domain: String,
    pub head_seq: u64,
    pub published_seq: u64,
    pub complete_cut_digest: Option<String>,
    pub complete_cut_generation: Option<u64>,
    pub selected_generation_digest: Option<String>,
    pub source_projection_digest: Option<String>,
}

/// Strictly verified, bounded journal interval. `initial_commitments` are the
/// commitments asserted by the first transition for each key; a caller with
/// its previously selected tree must compare those assertions before applying
/// `expected_final_commitments` to that tree.
#[derive(Clone, Debug)]
pub struct VerifiedAuditInterval {
    from_generation: u64,
    through_generation: u64,
    deltas: Vec<AuditDelta>,
    initial_commitments: BTreeMap<StableRowKey, Option<RowCommitment>>,
    expected_final_commitments: BTreeMap<StableRowKey, Option<RowCommitment>>,
}

impl VerifiedAuditInterval {
    pub const fn from_generation(&self) -> u64 {
        self.from_generation
    }

    pub const fn through_generation(&self) -> u64 {
        self.through_generation
    }

    pub fn deltas(&self) -> &[AuditDelta] {
        &self.deltas
    }

    pub fn initial_commitments(&self) -> &BTreeMap<StableRowKey, Option<RowCommitment>> {
        &self.initial_commitments
    }

    pub fn expected_final_commitments(&self) -> &BTreeMap<StableRowKey, Option<RowCommitment>> {
        &self.expected_final_commitments
    }

    /// Fetch final row commitments in the same caller-owned snapshot and
    /// require them to equal the journal replay result. Missing rows are
    /// represented by `None` and are valid only when the journal ends deleted.
    pub fn verify_final_rows(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        limits: AuditDeltaLimits,
    ) -> Result<(), AuditDeltaError> {
        self.verify_final_rows_controlled(tx, domain, limits, None)
    }

    pub fn verify_final_rows_controlled(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        limits: AuditDeltaLimits,
        control: Option<AuditDeltaControl<'_>>,
    ) -> Result<(), AuditDeltaError> {
        let mut work = AuditDeltaWork::default();
        self.verify_final_rows_controlled_with_work(tx, domain, limits, control, &mut work)
    }

    pub fn verify_final_rows_with_work(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        limits: AuditDeltaLimits,
        work: &mut AuditDeltaWork,
    ) -> Result<(), AuditDeltaError> {
        self.verify_final_rows_controlled_with_work(tx, domain, limits, None, work)
    }

    pub fn verify_final_rows_controlled_with_work(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        limits: AuditDeltaLimits,
        control: Option<AuditDeltaControl<'_>>,
        work: &mut AuditDeltaWork,
    ) -> Result<(), AuditDeltaError> {
        let observed = fetch_final_commitments_controlled_with_work(
            tx,
            domain,
            self.expected_final_commitments.keys().cloned().collect(),
            limits,
            control,
            work,
        )?;
        if observed != self.expected_final_commitments {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::FinalCommitmentMismatch,
            ));
        }
        Ok(())
    }
}

fn require_repeatable_snapshot(tx: &mut Transaction<'_>) -> Result<(), AuditDeltaError> {
    let row = tx.query_one("SELECT current_setting('transaction_isolation')", &[])?;
    let isolation: String = row.get(0);
    if isolation != "repeatable read" && isolation != "serializable" {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::RepeatableSnapshotRequired,
        ));
    }
    Ok(())
}

/// Load `from_generation..=current_generation` under a stable PostgreSQL
/// snapshot. There must be exactly one journal row for every generation.
pub fn load_interval(
    tx: &mut Transaction<'_>,
    domain: &str,
    from_generation: u64,
    limits: AuditDeltaLimits,
) -> Result<VerifiedAuditInterval, AuditDeltaError> {
    load_interval_controlled(tx, domain, from_generation, limits, None)
}

pub fn load_interval_controlled(
    tx: &mut Transaction<'_>,
    domain: &str,
    from_generation: u64,
    limits: AuditDeltaLimits,
    control: Option<AuditDeltaControl<'_>>,
) -> Result<VerifiedAuditInterval, AuditDeltaError> {
    let mut work = AuditDeltaWork::default();
    load_interval_controlled_with_work(tx, domain, from_generation, limits, control, &mut work)
}

pub fn load_interval_controlled_with_work(
    tx: &mut Transaction<'_>,
    domain: &str,
    from_generation: u64,
    limits: AuditDeltaLimits,
    control: Option<AuditDeltaControl<'_>>,
    work: &mut AuditDeltaWork,
) -> Result<VerifiedAuditInterval, AuditDeltaError> {
    if let Some(control) = control {
        control.bound_statement_timeout(tx)?;
    }
    require_repeatable_snapshot(tx)?;
    if let Some(control) = control {
        control.check()?;
        control.bound_statement_timeout(tx)?;
    }
    let limits = limits.validate()?;
    let profile = tx.query_opt(
        "SELECT baseline_generation,profile_digest
         FROM cmd2_audit_delta_v1_domain WHERE domain=$1",
        &[&domain],
    )?;
    if let Some(control) = control {
        control.check()?;
        control.bound_statement_timeout(tx)?;
    }
    let profile = profile.ok_or(AuditDeltaError::ColdRequired(
        ColdRequiredReason::DomainNotActivated,
    ))?;
    let baseline_generation = nonnegative(profile.get::<_, i64>(0))?;
    let expected_profile_digest = audit_delta_schema_digest().to_hex();
    let stored_profile_digest: String = profile.get(1);
    if stored_profile_digest.trim_end() != expected_profile_digest.as_str() {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::ProfileMismatch,
        ));
    }
    if from_generation < baseline_generation {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::InvalidBounds,
        ));
    }
    let fence = tx.query_opt(
        "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1",
        &[&domain],
    )?;
    if let Some(control) = control {
        control.check()?;
        control.bound_statement_timeout(tx)?;
    }
    let fence = fence.ok_or(AuditDeltaError::ColdRequired(
        ColdRequiredReason::GenerationGap,
    ))?;
    let through_generation = nonnegative(fence.get::<_, i64>(0))?;
    if fence.get::<_, String>(1) != "normal" {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::MaintenanceActive,
        ));
    }
    if from_generation > through_generation {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::InvalidBounds,
        ));
    }
    let expected_rows = through_generation - from_generation;
    if expected_rows > limits.max_rows as u64 {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::IntervalTooLarge,
        ));
    }
    let from = to_pg_i64(from_generation)?;
    let through = to_pg_i64(through_generation)?;
    let preflight = tx.query_one(AUDIT_INTERVAL_PREFLIGHT_SQL, &[&domain, &from, &through])?;
    work.add_preflight_witness()?;
    if let Some(control) = control {
        control.check()?;
    }
    let raw_row_count = preflight.try_get::<_, i64>(0)?;
    let row_count = nonnegative(raw_row_count)?;
    work.add_preflight_journal_rows(row_count)?;
    let raw_encoded_bytes = preflight.try_get::<_, i64>(1)?;
    let encoded_bytes = nonnegative(raw_encoded_bytes)?;
    work.add_preflight_payload_bytes(encoded_bytes)?;
    let raw_max_key_bytes = preflight.try_get::<_, i64>(2)?;
    let max_key_bytes = nonnegative(raw_max_key_bytes)?;
    work.observe_preflight_max_key_bytes(max_key_bytes);
    verify_delta_row_count(
        from_generation,
        through_generation,
        row_count,
        limits.max_rows,
    )?;
    if encoded_bytes > limits.max_bytes as u64 || max_key_bytes > limits.max_key_bytes as u64 {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::IntervalTooLarge,
        ));
    }

    // This is the first materialization of interval rows; aggregate count and
    // exact encoded byte bounds above have already admitted the read.
    let row_capacity = usize::try_from(row_count)
        .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::IntervalTooLarge))?;
    let mut deltas = Vec::with_capacity(row_capacity);
    if let Some(control) = control {
        control.bound_statement_timeout(tx)?;
    }
    let parameters: [&(dyn ToSql + Sync); 3] = [&domain, &from, &through];
    let mut rows = tx.query_raw(AUDIT_INTERVAL_ROWS_SQL, parameters)?;
    let mut next_generation =
        from_generation
            .checked_add(1)
            .ok_or(AuditDeltaError::ColdRequired(
                ColdRequiredReason::InvalidBounds,
            ))?;
    let mut state = BTreeMap::new();
    let mut initial = BTreeMap::new();
    loop {
        let row = match rows.next()? {
            Some(row) => row,
            None => break,
        };
        work.add_interval_row()?;
        if let Some(control) = control {
            control.check()?;
        }
        let raw_generation = row.try_get::<_, i64>(0)?;
        work.add_interval_bytes(8)?;
        let generation = nonnegative(raw_generation)?;
        if generation != next_generation {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::GenerationGap,
            ));
        }
        next_generation = next_generation
            .checked_add(1)
            .ok_or(AuditDeltaError::ColdRequired(
                ColdRequiredReason::InvalidBounds,
            ))?;
        let table_id: i16 = row.try_get(1)?;
        work.add_interval_bytes(2)?;
        let table = MetadataTable::from_id(table_id).ok_or(AuditDeltaError::ColdRequired(
            ColdRequiredReason::UnknownTable,
        ))?;
        let operation_text: String = row.try_get(2)?;
        work.add_interval_bytes(operation_text.len())?;
        let operation = AuditDeltaOperation::parse(&operation_text).ok_or(
            AuditDeltaError::ColdRequired(ColdRequiredReason::UnknownOperation),
        )?;
        let old_key_value = row.try_get::<_, Option<Vec<u8>>>(3)?;
        work.add_interval_bytes(old_key_value.as_ref().map_or(0, Vec::len))?;
        let old_key = old_key_value
            .map(|key| StableRowKey::parse(table, key, limits.max_key_bytes))
            .transpose()?;
        let new_key_value = row.try_get::<_, Option<Vec<u8>>>(4)?;
        work.add_interval_bytes(new_key_value.as_ref().map_or(0, Vec::len))?;
        let new_key = new_key_value
            .map(|key| StableRowKey::parse(table, key, limits.max_key_bytes))
            .transpose()?;
        let old_commitment_value = row.try_get::<_, Option<Vec<u8>>>(5)?;
        work.add_interval_bytes(old_commitment_value.as_ref().map_or(0, Vec::len))?;
        let old_commitment = parse_commitment(old_commitment_value)?;
        let new_commitment_value = row.try_get::<_, Option<Vec<u8>>>(6)?;
        work.add_interval_bytes(new_commitment_value.as_ref().map_or(0, Vec::len))?;
        let new_commitment = parse_commitment(new_commitment_value)?;
        let delta = AuditDelta {
            generation,
            operation,
            old_key,
            new_key,
            old_commitment,
            new_commitment,
        };
        apply_delta(&delta, &mut state, &mut initial)?;
        deltas.push(delta);
    }
    if next_generation != through_generation.saturating_add(1) {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::GenerationGap,
        ));
    }
    Ok(VerifiedAuditInterval {
        from_generation,
        through_generation,
        deltas,
        initial_commitments: initial,
        expected_final_commitments: state,
    })
}

fn apply_delta(
    delta: &AuditDelta,
    state: &mut BTreeMap<StableRowKey, Option<RowCommitment>>,
    initial: &mut BTreeMap<StableRowKey, Option<RowCommitment>>,
) -> Result<(), AuditDeltaError> {
    use AuditDeltaOperation::{Delete, Insert, Update};
    match delta.operation {
        Insert
            if delta.old_key.is_none()
                && delta.old_commitment.is_none()
                && delta.new_key.is_some()
                && delta.new_commitment.is_some() =>
        {
            transition(
                delta.new_key.as_ref().expect("shape checked"),
                None,
                delta.new_commitment,
                state,
                initial,
            )?;
        }
        Delete
            if delta.old_key.is_some()
                && delta.old_commitment.is_some()
                && delta.new_key.is_none()
                && delta.new_commitment.is_none() =>
        {
            transition(
                delta.old_key.as_ref().expect("shape checked"),
                delta.old_commitment,
                None,
                state,
                initial,
            )?;
        }
        Update
            if delta.old_key.is_some()
                && delta.old_commitment.is_some()
                && delta.new_key.is_some()
                && delta.new_commitment.is_some() =>
        {
            let old_key = delta.old_key.as_ref().expect("shape checked");
            let new_key = delta.new_key.as_ref().expect("shape checked");
            if old_key == new_key {
                transition(
                    old_key,
                    delta.old_commitment,
                    delta.new_commitment,
                    state,
                    initial,
                )?;
            } else {
                transition(old_key, delta.old_commitment, None, state, initial)?;
                transition(new_key, None, delta.new_commitment, state, initial)?;
            }
        }
        _ => {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::MalformedCommitment,
            ));
        }
    }
    Ok(())
}

fn verify_delta_row_count(
    from_generation: u64,
    through_generation: u64,
    row_count: u64,
    max_rows: usize,
) -> Result<(), AuditDeltaError> {
    if from_generation > through_generation {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::InvalidBounds,
        ));
    }
    let expected = through_generation - from_generation;
    if expected > max_rows as u64 {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::IntervalTooLarge,
        ));
    }
    if row_count != expected {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::GenerationGap,
        ));
    }
    Ok(())
}

fn transition(
    key: &StableRowKey,
    before: Option<RowCommitment>,
    after: Option<RowCommitment>,
    state: &mut BTreeMap<StableRowKey, Option<RowCommitment>>,
    initial: &mut BTreeMap<StableRowKey, Option<RowCommitment>>,
) -> Result<(), AuditDeltaError> {
    if let Some(current) = state.get(key) {
        if *current != before {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::ChainDiscontinuity,
            ));
        }
    } else {
        initial.insert(key.clone(), before);
    }
    state.insert(key.clone(), after);
    Ok(())
}

/// Query final row commitments for bounded, unique stable keys. This uses
/// table-specific primary-key joins and the exact same SQL commitment
/// function as the journal; rows and bytes are bounded before any result set
/// is materialized.
pub fn fetch_final_commitments(
    tx: &mut Transaction<'_>,
    domain: &str,
    keys: BTreeSet<StableRowKey>,
    limits: AuditDeltaLimits,
) -> Result<BTreeMap<StableRowKey, Option<RowCommitment>>, AuditDeltaError> {
    fetch_final_commitments_controlled(tx, domain, keys, limits, None)
}

pub fn fetch_final_commitments_controlled(
    tx: &mut Transaction<'_>,
    domain: &str,
    keys: BTreeSet<StableRowKey>,
    limits: AuditDeltaLimits,
    control: Option<AuditDeltaControl<'_>>,
) -> Result<BTreeMap<StableRowKey, Option<RowCommitment>>, AuditDeltaError> {
    let mut work = AuditDeltaWork::default();
    fetch_final_commitments_controlled_with_work(tx, domain, keys, limits, control, &mut work)
}

pub fn fetch_final_commitments_controlled_with_work(
    tx: &mut Transaction<'_>,
    domain: &str,
    keys: BTreeSet<StableRowKey>,
    limits: AuditDeltaLimits,
    control: Option<AuditDeltaControl<'_>>,
    work: &mut AuditDeltaWork,
) -> Result<BTreeMap<StableRowKey, Option<RowCommitment>>, AuditDeltaError> {
    if let Some(control) = control {
        control.bound_statement_timeout(tx)?;
    }
    require_repeatable_snapshot(tx)?;
    if let Some(control) = control {
        control.check()?;
        control.bound_statement_timeout(tx)?;
    }
    let limits = limits.validate()?;
    if keys.len() > limits.max_rows {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::IntervalTooLarge,
        ));
    }
    let mut groups: BTreeMap<MetadataTable, Vec<(StableRowKey, Value)>> = BTreeMap::new();
    let mut key_bytes = 0usize;
    for key in keys {
        if key.key.len() > limits.max_key_bytes {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::MalformedStableKey,
            ));
        }
        key_bytes = key_bytes
            .checked_add(key.key.len())
            .ok_or(AuditDeltaError::ColdRequired(
                ColdRequiredReason::IntervalTooLarge,
            ))?;
        if key_bytes > limits.max_bytes {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::IntervalTooLarge,
            ));
        }
        let value = key.json_value()?;
        if key.table == MetadataTable::DomainHeader
            && value
                .as_array()
                .and_then(|parts| parts.first())
                .and_then(Value::as_str)
                != Some(domain)
        {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::MalformedStableKey,
            ));
        }
        groups.entry(key.table).or_default().push((key, value));
    }
    let mut encoded_groups = BTreeMap::new();
    let mut encoded_bytes = key_bytes;
    for (table, rows) in &groups {
        let values = rows.iter().map(|(_, value)| value).collect::<Vec<_>>();
        let json = serde_json::to_string(&values)
            .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::MalformedStableKey))?;
        encoded_bytes =
            encoded_bytes
                .checked_add(json.len())
                .ok_or(AuditDeltaError::ColdRequired(
                    ColdRequiredReason::IntervalTooLarge,
                ))?;
        encoded_groups.insert(*table, json);
    }
    let result_bytes = groups
        .values()
        .map(Vec::len)
        .sum::<usize>()
        .checked_mul(32)
        .ok_or(AuditDeltaError::ColdRequired(
            ColdRequiredReason::IntervalTooLarge,
        ))?;
    if !matches!(
        encoded_bytes.checked_add(result_bytes),
        Some(total) if total <= limits.max_bytes
    ) {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::IntervalTooLarge,
        ));
    }

    let mut observed = BTreeMap::new();
    for (table, rows) in groups {
        if let Some(control) = control {
            control.bound_statement_timeout(tx)?;
        }
        let json = encoded_groups
            .get(&table)
            .expect("encoded group follows preflight");
        let id = table.id();
        let sql = final_lookup_sql(table);
        let params: [&(dyn ToSql + Sync); 3] = [&domain, &json, &id];
        work.add_final_lookup_key_json_bytes(json.len())?;
        let mut returned = tx.query_raw(&sql, params)?;
        let mut by_ordinal = BTreeMap::new();
        let mut returned_count = 0usize;
        loop {
            let row = match returned.next()? {
                Some(row) => row,
                None => break,
            };
            work.add_final_lookup_row()?;
            returned_count = returned_count
                .checked_add(1)
                .ok_or(AuditDeltaError::ColdRequired(
                    ColdRequiredReason::WorkCounterOverflow,
                ))?;
            if let Some(control) = control {
                control.check()?;
            }
            let raw_ordinal = row.try_get::<_, i64>(0)?;
            work.add_final_lookup_bytes(8)?;
            let ordinal = nonnegative(raw_ordinal)?;
            let commitment_value = row.try_get::<_, Option<Vec<u8>>>(1)?;
            work.add_final_lookup_bytes(commitment_value.as_ref().map_or(0, Vec::len))?;
            let digest = parse_commitment(commitment_value)?;
            if by_ordinal.insert(ordinal, digest).is_some() {
                return Err(AuditDeltaError::ColdRequired(
                    ColdRequiredReason::FinalCommitmentMismatch,
                ));
            }
        }
        if returned_count != rows.len() {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::FinalCommitmentMismatch,
            ));
        }
        for (index, (key, _)) in rows.into_iter().enumerate() {
            let ordinal = index as u64 + 1;
            let digest = by_ordinal
                .remove(&ordinal)
                .ok_or(AuditDeltaError::ColdRequired(
                    ColdRequiredReason::FinalCommitmentMismatch,
                ))?;
            observed.insert(key, digest);
        }
        if !by_ordinal.is_empty() {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::FinalCommitmentMismatch,
            ));
        }
    }
    Ok(observed)
}

/// Validate the exact production PostgreSQL binding/decode boundary before
/// expensive cold setup. The impossible interval and empty key array disclose
/// no source rows, activate no domain, and mint no admitted source proof.
pub(crate) fn verify_sql_binding_types(
    tx: &mut Transaction<'_>,
    control: AuditDeltaControl<'_>,
) -> Result<(), AuditDeltaError> {
    let shape = |statement: &postgres::Statement, params: &[Type], columns: &[Type]| {
        statement.params() == params
            && statement.columns().len() == columns.len()
            && statement
                .columns()
                .iter()
                .zip(columns)
                .all(|(actual, expected)| actual.type_() == expected)
    };
    let domain = "";
    let from = 0i64;
    let through = 0i64;
    let parameters: [&(dyn ToSql + Sync); 3] = [&domain, &from, &through];
    control.bound_statement_timeout(tx)?;
    let preflight = tx.prepare(AUDIT_INTERVAL_PREFLIGHT_SQL)?;
    if !shape(
        &preflight,
        &[Type::TEXT, Type::INT8, Type::INT8],
        &[Type::INT8, Type::INT8, Type::INT8],
    ) {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::ProfileMismatch,
        ));
    }
    let counts = tx.query_one(&preflight, &parameters)?;
    for index in 0..3 {
        if counts.try_get::<_, i64>(index)? != 0 {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::ProfileMismatch,
            ));
        }
    }
    control.bound_statement_timeout(tx)?;
    let rows = tx.prepare(AUDIT_INTERVAL_ROWS_SQL)?;
    if !shape(
        &rows,
        &[Type::TEXT, Type::INT8, Type::INT8],
        &[
            Type::INT8,
            Type::INT2,
            Type::TEXT,
            Type::BYTEA,
            Type::BYTEA,
            Type::BYTEA,
            Type::BYTEA,
        ],
    ) || !tx.query(&rows, &parameters)?.is_empty()
    {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::ProfileMismatch,
        ));
    }
    let json = String::from("[]");
    for table in MetadataTable::ALL {
        control.bound_statement_timeout(tx)?;
        let id = table.id();
        let statement = tx.prepare(&final_lookup_sql(table))?;
        if !shape(
            &statement,
            &[Type::TEXT, Type::TEXT, Type::INT2],
            &[Type::INT8, Type::BYTEA],
        ) {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::ProfileMismatch,
            ));
        }
        let parameters: [&(dyn ToSql + Sync); 3] = [&domain, &json, &id];
        if !tx.query(&statement, &parameters)?.is_empty() {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::ProfileMismatch,
            ));
        }
        control.check()?;
    }
    control.check()
}

fn final_lookup_sql(table: MetadataTable) -> String {
    let (relation, condition) = match table {
        MetadataTable::Job => ("cmd2_job", "t.job_id=k.value->>0"),
        MetadataTable::Predicate => (
            "cmd2_predicate",
            "t.kind=k.value->>0 AND t.owner=k.value->>1 AND t.scope=k.value->>2 AND t.token=k.value->>3",
        ),
        MetadataTable::Attempt => (
            "cmd2_attempt",
            "t.prepare_id=decode(substr(k.value->>0,3),'hex')",
        ),
        MetadataTable::Member => (
            "cmd2_member",
            "t.prepare_id=decode(substr(k.value->>0,3),'hex') AND t.member_slot=(k.value->>1)::integer",
        ),
        MetadataTable::Current => ("cmd2_current", "t.subject=k.value->>0"),
        MetadataTable::History => (
            "cmd2_history",
            "t.subject=k.value->>0 AND t.revision=(k.value->>1)::bigint",
        ),
        MetadataTable::Receipt => ("cmd2_receipt", "t.command_id=k.value->>0"),
        MetadataTable::Log => ("cmd2_log", "t.commit_seq=(k.value->>0)::bigint"),
        MetadataTable::Outbox => ("cmd2_outbox", "t.commit_seq=(k.value->>0)::bigint"),
        MetadataTable::SourceIndex => (
            "cmd2_source_index",
            "t.kind=k.value->>0 AND t.token=k.value->>1 AND t.path=k.value->>2",
        ),
        MetadataTable::DomainHeader => ("cmd2_domain", "t.domain=k.value->>0"),
    };
    format!(
        "SELECT k.ordinality::bigint,
                CASE WHEN t.domain IS NULL THEN NULL
                     ELSE cmd2_audit_delta_v1_row_commitment($3,row_to_json(t)) END
         FROM jsonb_array_elements($2::text::jsonb) WITH ORDINALITY AS k(value,ordinality)
         LEFT JOIN {relation} t ON t.domain=$1 AND {condition}
         ORDER BY k.ordinality"
    )
}

/// Digest for the opt-in schema identity. V1's `schema_profile_digest()` is
/// intentionally unchanged; an opt-in consumer binds this value separately.
pub fn audit_delta_schema_digest() -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    profile_part(&mut hasher, b"cmd2-audit-delta-schema-v1");
    profile_part(&mut hasher, include_bytes!("durable_schema.sql"));
    profile_part(&mut hasher, include_bytes!("audit_delta_schema_v1.sql"));
    profile_part(&mut hasher, ROW_KEY_PROFILE);
    hasher.finalize()
}

/// Activate journaling after the opt-in schema is installed. The caller
/// supplies a freshly cold-verified cut's observed fence generation and the
/// reviewed opt-in profile digest. Existing compatible markers are reused at
/// the exact current generation, allowing a new cold cut to reset past older
/// maintenance gaps without deleting or rewriting journal history.
pub fn activate_domain(
    tx: &mut Transaction<'_>,
    domain: &str,
    expected_generation: u64,
    profile_digest: Digest256,
) -> Result<u64, AuditDeltaError> {
    require_repeatable_snapshot(tx)?;
    let compiled_profile = audit_delta_schema_digest();
    if profile_digest != compiled_profile {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::ProfileMismatch,
        ));
    }
    let digest = profile_digest.to_hex();
    let expected = to_pg_i64(expected_generation)?;
    let fence = tx.query_opt(
        "SELECT generation,maintenance_state FROM cmd2_audit_fence
         WHERE domain=$1 FOR UPDATE",
        &[&domain],
    )?;
    let fence = fence.ok_or(AuditDeltaError::ColdRequired(
        ColdRequiredReason::GenerationGap,
    ))?;
    let observed_generation = fence.get::<_, i64>(0);
    if observed_generation != expected {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::GenerationGap,
        ));
    }
    if fence.get::<_, String>(1) != "normal" {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::MaintenanceActive,
        ));
    }
    if let Some(existing) = tx.query_opt(
        "SELECT baseline_generation,profile_digest
         FROM cmd2_audit_delta_v1_domain WHERE domain=$1",
        &[&domain],
    )? {
        let baseline_generation = nonnegative(existing.get::<_, i64>(0))?;
        let stored_profile_digest: String = existing.get(1);
        if stored_profile_digest.trim_end() != digest.as_str() {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::ProfileMismatch,
            ));
        }
        if baseline_generation > expected_generation {
            return Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::InvalidBounds,
            ));
        }
        return Ok(expected_generation);
    }
    let inserted = tx.execute(
        "INSERT INTO cmd2_audit_delta_v1_domain(domain,baseline_generation,profile_digest)
         VALUES($1,$2,$3)",
        &[&domain, &expected, &digest],
    )?;
    if inserted != 1 {
        return Err(AuditDeltaError::ColdRequired(
            ColdRequiredReason::ProfileMismatch,
        ));
    }
    Ok(expected_generation)
}

fn profile_part(hasher: &mut Digest256Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

/// Read selection/publication fields independently from the semantic domain
/// header. Invoke this in the same RR transaction as `load_interval` and
/// `fetch_final_commitments`.
pub fn read_final_selector(
    tx: &mut Transaction<'_>,
    domain: &str,
) -> Result<FinalSelector, AuditDeltaError> {
    read_final_selector_controlled(tx, domain, None)
}

pub fn read_final_selector_controlled(
    tx: &mut Transaction<'_>,
    domain: &str,
    control: Option<AuditDeltaControl<'_>>,
) -> Result<FinalSelector, AuditDeltaError> {
    if let Some(control) = control {
        control.bound_statement_timeout(tx)?;
    }
    require_repeatable_snapshot(tx)?;
    if let Some(control) = control {
        control.check()?;
        control.bound_statement_timeout(tx)?;
    }
    let row = tx.query_opt(
        "SELECT domain,head_seq,published_seq,complete_cut_digest,
                complete_cut_generation,selected_generation_digest,source_projection_digest
         FROM cmd2_domain WHERE domain=$1",
        &[&domain],
    )?;
    if let Some(control) = control {
        control.check()?;
    }
    let row = row.ok_or(AuditDeltaError::ColdRequired(
        ColdRequiredReason::GenerationGap,
    ))?;
    Ok(FinalSelector {
        domain: row.get(0),
        head_seq: nonnegative(row.get(1))?,
        published_seq: nonnegative(row.get(2))?,
        complete_cut_digest: row.get(3),
        complete_cut_generation: row.get::<_, Option<i64>>(4).map(nonnegative).transpose()?,
        selected_generation_digest: row.get(5),
        source_projection_digest: row.get(6),
    })
}

fn parse_commitment(value: Option<Vec<u8>>) -> Result<Option<RowCommitment>, AuditDeltaError> {
    value
        .map(|bytes| {
            let digest: [u8; 32] = bytes.try_into().map_err(|_| {
                AuditDeltaError::ColdRequired(ColdRequiredReason::MalformedCommitment)
            })?;
            Ok(RowCommitment(digest))
        })
        .transpose()
}

fn nonnegative(value: i64) -> Result<u64, AuditDeltaError> {
    u64::try_from(value)
        .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::MalformedCommitment))
}

fn to_pg_i64(value: u64) -> Result<i64, AuditDeltaError> {
    i64::try_from(value)
        .map_err(|_| AuditDeltaError::ColdRequired(ColdRequiredReason::InvalidBounds))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> StableRowKey {
        StableRowKey {
            table: MetadataTable::Current,
            key: serde_json::to_vec(&[name]).unwrap(),
        }
    }

    fn commitment(byte: u8) -> Option<RowCommitment> {
        Some(RowCommitment([byte; 32]))
    }

    #[test]
    fn repeated_updates_require_a_contiguous_old_to_new_chain() {
        let row_key = key("subject");
        let mut state = BTreeMap::new();
        let mut initial = BTreeMap::new();
        transition(
            &row_key,
            commitment(1),
            commitment(2),
            &mut state,
            &mut initial,
        )
        .unwrap();
        transition(
            &row_key,
            commitment(2),
            commitment(3),
            &mut state,
            &mut initial,
        )
        .unwrap();
        assert_eq!(initial.get(&row_key), Some(&commitment(1)));
        assert_eq!(state.get(&row_key), Some(&commitment(3)));
        assert!(matches!(
            transition(
                &row_key,
                commitment(1),
                commitment(4),
                &mut state,
                &mut initial
            ),
            Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::ChainDiscontinuity
            ))
        ));
    }

    #[test]
    fn key_change_records_delete_and_insert_sides() {
        let old_key = key("old");
        let new_key = key("new");
        let delta = AuditDelta {
            generation: 1,
            operation: AuditDeltaOperation::Update,
            old_key: Some(old_key.clone()),
            new_key: Some(new_key.clone()),
            old_commitment: commitment(1),
            new_commitment: commitment(2),
        };
        let mut state = BTreeMap::new();
        let mut initial = BTreeMap::new();
        apply_delta(&delta, &mut state, &mut initial).unwrap();
        assert_eq!(initial.get(&old_key), Some(&commitment(1)));
        assert_eq!(initial.get(&new_key), Some(&None));
        assert_eq!(state.get(&old_key), Some(&None));
        assert_eq!(state.get(&new_key), Some(&commitment(2)));
    }

    #[test]
    fn interval_is_exclusive_at_start_and_rejects_a_missing_generation() {
        assert!(verify_delta_row_count(8, 10, 2, 2).is_ok());
        assert!(matches!(
            verify_delta_row_count(8, 10, 1, 2),
            Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::GenerationGap
            ))
        ));
        assert!(matches!(
            verify_delta_row_count(8, 7, 0, 2),
            Err(AuditDeltaError::ColdRequired(
                ColdRequiredReason::InvalidBounds
            ))
        ));
    }
}
