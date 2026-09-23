//! Native seek adapter for an owner-opened, pinned CMP navigation model.
//! This module never resolves selected.json or an artifact pathname.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use tos_foundation::Digest256;

use crate::source_descend::json_string;
use crate::{
    AdjacencyPage, Binding, Charged, DisclosureLease, ExactNode, QueryError, QueryErrorCode,
    RawRecord, ReadModel,
};
use tos_compiler::{VerifiedSelectedModel, VerifiedSelection};

const METADATA_ROWS_CAP: usize = 64;
const METADATA_KEY_BYTES_CAP: u64 = 128;
const METADATA_VALUE_BYTES_CAP: u64 = 4096;
const AUTHORITY_VALUE_BYTES_CAP: u64 = 262_144;
const SQLITE_I64_RESULT_BYTES: u64 = 8;
const SHA256_HEX_RESULT_BYTES: u64 = 64;
const NODE_HEADER_RESULT_BYTES: u64 = 2 * SQLITE_I64_RESULT_BYTES + SHA256_HEX_RESULT_BYTES;
const ADJACENCY_CERT_RESULT_BYTES: u64 = SQLITE_I64_RESULT_BYTES + SHA256_HEX_RESULT_BYTES;

/// Caller-selected warm adapter initialization admission. `decoded_bytes`
/// counts SQLite result-column payload: each returned i64 as 8 bytes and each
/// returned TEXT/BLOB by its UTF-8/byte length. It is not file/page I/O or
/// allocator footprint; CMP separately meters cold/fork SQLite startup.
#[derive(Clone, Copy, Debug)]
pub struct AdapterAdmissionBudget {
    pub max_selected_open_vm_steps: u64,
    pub max_metadata_vm_steps: u64,
    pub max_metadata_decoded_bytes: u64,
    pub max_metadata_rows: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterAdmissionCharge {
    pub selected_open_vm_steps: u64,
    pub metadata_vm_steps: u64,
    pub metadata_decoded_bytes: u64,
    pub metadata_rows: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortReason {
    Cancelled,
    DeadlineExceeded,
}

/// A cheap owner/transport cancellation probe. Deadline clocks and client
/// disconnect signals stay with the caller; QRY checks at domain boundaries
/// and inside every SQLite VM progress callback during a seek.
pub trait AbortProbe: Send + Sync {
    fn reason(&self) -> Option<AbortReason>;
}

/// Implement only for an owner-verified selected model whose SQLite connection
/// holds the already-digested immutable inode. The receipt, full source cut,
/// index completeness and live pin are established by that owner, not QRY.
pub trait PinnedLocalModel {
    fn binding(&self) -> &Binding;
    fn selected_model_sha256(&self) -> Digest256;
    fn owner_receipt_id(&self) -> &str;
    fn source_authority_boundary(&self) -> &str;
    /// Actual SQLite startup VM steps from the owner opener/fork receipt.
    fn selected_open_vm_steps(&self) -> u64;
    fn connection(&self) -> &Connection;
    fn check_pin(&mut self) -> Result<(), QueryError>;
    fn source_disclosure(&mut self) -> Result<Box<dyn DisclosureLease>, QueryError>;
}

/// The source owner's live sealed-cut pin, distinct from CMP's pinned file.
pub trait SourcePin {
    fn check_sealed_cut(&mut self, selection: &VerifiedSelection) -> Result<(), QueryError>;
    /// Hold the exact source cut through packet disclosure.
    fn acquire_disclosure(
        &mut self,
        selection: &VerifiedSelection,
    ) -> Result<Box<dyn DisclosureLease>, QueryError>;
}

/// CMP's verified selected inode plus the source owner's live cut pin.
pub struct CmpPinnedModel<S: SourcePin> {
    model: VerifiedSelectedModel,
    binding: Binding,
    source_pin: S,
}

impl<S: SourcePin> CmpPinnedModel<S> {
    pub fn new(model: VerifiedSelectedModel, source_pin: S) -> Result<Self, QueryError> {
        let selection = model.selection();
        let binding = Binding {
            source_cut: selection.source_cut.clone(),
            through_commit_seq: selection.through_commit_seq,
            membership_root: digest(&selection.membership_root)?,
            projection_root: digest(&selection.projection_root_sha256)?,
            index_root: digest(&selection.model_sha256)?,
            index_generation: selection.index_generation.clone(),
            route_map_version: selection.route_map_version.clone(),
            reader_abi: selection.reader_abi.clone(),
            model_abi: selection.model_abi.clone(),
            selection_profile: selection.selection_profile.clone(),
        };
        let mut selected = Self {
            model,
            binding,
            source_pin,
        };
        selected.check_pin()?;
        Ok(selected)
    }

    /// Open another warm reader on CMP's already-admitted inode. The caller
    /// supplies a fresh source-owner pin for this reader; no pathname is
    /// resolved and no source right is inherited from the prior reader.
    pub fn fork_reader<T: SourcePin>(
        &self,
        source_pin: T,
        max_open_vm_steps: u64,
    ) -> Result<CmpPinnedModel<T>, QueryError> {
        let model = self
            .model
            .fork_reader_with_vm_budget(max_open_vm_steps)
            .map_err(|reason| {
                if matches!(reason, tos_compiler::Error::Budget(_)) {
                    error(
                        QueryErrorCode::BudgetExceeded,
                        "selected model warm-reader VM admission exceeded",
                    )
                } else {
                    error(
                        QueryErrorCode::StaleSelection,
                        "selected model warm reader unavailable",
                    )
                }
            })?;
        CmpPinnedModel::new(model, source_pin)
    }
}

impl<S: SourcePin> PinnedLocalModel for CmpPinnedModel<S> {
    fn binding(&self) -> &Binding {
        &self.binding
    }
    fn selected_model_sha256(&self) -> Digest256 {
        self.binding.index_root
    }
    fn owner_receipt_id(&self) -> &str {
        &self.model.selection().owner_receipt_id
    }
    fn source_authority_boundary(&self) -> &str {
        &self.model.selection().authority_boundary
    }
    fn selected_open_vm_steps(&self) -> u64 {
        self.model.open_vm_steps()
    }
    fn connection(&self) -> &Connection {
        self.model.connection()
    }
    fn check_pin(&mut self) -> Result<(), QueryError> {
        self.model.check_pin().map_err(|_| {
            error(
                QueryErrorCode::StaleSelection,
                "selected model inode changed",
            )
        })?;
        self.source_pin.check_sealed_cut(self.model.selection())
    }
    fn source_disclosure(&mut self) -> Result<Box<dyn DisclosureLease>, QueryError> {
        self.model.check_pin().map_err(|_| {
            error(
                QueryErrorCode::StaleSelection,
                "selected model inode changed",
            )
        })?;
        self.source_pin.acquire_disclosure(self.model.selection())
    }
}

pub type CmpSqliteReadModel<S, G> = SqliteReadModel<CmpPinnedModel<S>, G>;

/// Source-owner disclosure route for this selected public metadata projection.
/// The export/publication and policy fields must come from their actual ToS
/// owners; a CMP file receipt or access packaging receipt alone is no grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisclosureScope {
    pub operation_id: String,
    pub carrier_layer: String,
    pub intended_use: String,
    pub selected_binding: Binding,
    pub corpus_revision: String,
    pub data_revision: String,
    pub selected_export_receipt_id: String,
    pub selected_model_receipt_id: String,
    pub policy_issuer_ref: String,
    pub policy_receipt_id: String,
    pub policy_epoch: String,
    pub withdrawal_generation: String,
}

impl DisclosureScope {
    fn validate(
        &self,
        binding: &Binding,
        selected_model_receipt_id: &str,
    ) -> Result<(), QueryError> {
        if self.operation_id != "tos.source.descend"
            || self.carrier_layer != "tos_source_navigation_public_metadata_v1"
            || self.intended_use != "read_only_public_metadata_navigation_v1"
            || &self.selected_binding != binding
            || self.selected_model_receipt_id != selected_model_receipt_id
            || [
                &self.corpus_revision,
                &self.data_revision,
                &self.selected_export_receipt_id,
                &self.policy_issuer_ref,
                &self.policy_receipt_id,
                &self.policy_epoch,
                &self.withdrawal_generation,
            ]
            .iter()
            .any(|value| value.is_empty())
        {
            return Err(error(
                QueryErrorCode::Unavailable,
                "source-owner public metadata disclosure scope absent or mismatched",
            ));
        }
        Ok(())
    }
}

/// Source-owner current policy for the explicit carried layer and intended
/// use. Historical selection and Item payload rights cannot imply metadata
/// disclosure permission, nor can metadata permission grant Item payload use.
pub trait CurrentPolicy {
    fn authorize_current(
        &mut self,
        scope: &DisclosureScope,
        record: &RawRecord,
    ) -> Result<Charged, QueryError>;
    /// Recheck all selected records under one current-policy hold. It must
    /// serialize revocation through disclosure, not return a stale observation.
    fn acquire_disclosure(
        &mut self,
        scope: &DisclosureScope,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease>, QueryError>;
}

struct CombinedLease {
    source: Box<dyn DisclosureLease>,
    rights: Box<dyn DisclosureLease>,
}
impl DisclosureLease for CombinedLease {
    fn recheck(&mut self) -> Result<(), QueryError> {
        self.source.recheck()?;
        self.rights.recheck()?;
        self.source.recheck()
    }
}

pub struct SqliteReadModel<P: PinnedLocalModel, G: CurrentPolicy> {
    pinned: P,
    policy: G,
    disclosure_scope: DisclosureScope,
    authority: RawRecord,
    admission_charge: AdapterAdmissionCharge,
    abort_probe: Option<Arc<dyn AbortProbe>>,
}

fn error(code: QueryErrorCode, message: &'static str) -> QueryError {
    QueryError { code, message }
}

fn sql_error(error_value: rusqlite::Error) -> QueryError {
    if matches!(&error_value, rusqlite::Error::SqliteFailure(failure, _) if failure.code == ErrorCode::OperationInterrupted)
    {
        error(QueryErrorCode::BudgetExceeded, "SQLite VM budget exhausted")
    } else {
        error(
            QueryErrorCode::CorruptSelectedCarrier,
            "selected SQLite model query failed",
        )
    }
}

fn budgeted<T>(
    connection: &Connection,
    max_vm_steps: u64,
    abort_probe: Option<&Arc<dyn AbortProbe>>,
    run: impl FnOnce() -> Result<T, QueryError>,
) -> Result<(T, u64), QueryError> {
    if let Some(reason) = abort_probe.and_then(|probe| probe.reason()) {
        return Err(abort_error(reason));
    }
    if max_vm_steps == 0 {
        return Err(error(
            QueryErrorCode::BudgetExceeded,
            "SQLite VM budget absent",
        ));
    }
    let steps = Arc::new(AtomicU64::new(0));
    let observed = Arc::clone(&steps);
    let interrupted = Arc::new(AtomicU8::new(0));
    let interruption = Arc::clone(&interrupted);
    let probe = abort_probe.cloned();
    connection.progress_handler(
        1,
        Some(move || {
            if let Some(reason) = probe.as_ref().and_then(|probe| probe.reason()) {
                interruption.store(
                    match reason {
                        AbortReason::Cancelled => 1,
                        AbortReason::DeadlineExceeded => 2,
                    },
                    Ordering::Relaxed,
                );
                return true;
            }
            observed.fetch_add(1, Ordering::Relaxed) >= max_vm_steps
        }),
    );
    let result = run();
    connection.progress_handler(0, None::<fn() -> bool>);
    let charged = steps.load(Ordering::Relaxed);
    match interrupted.load(Ordering::Relaxed) {
        1 => return Err(abort_error(AbortReason::Cancelled)),
        2 => return Err(abort_error(AbortReason::DeadlineExceeded)),
        _ => {}
    }
    if charged > max_vm_steps {
        return Err(error(
            QueryErrorCode::BudgetExceeded,
            "SQLite VM budget exhausted",
        ));
    }
    result.map(|value| (value, charged))
}

fn abort_error(reason: AbortReason) -> QueryError {
    match reason {
        AbortReason::Cancelled => error(
            QueryErrorCode::Cancelled,
            "query cancelled before disclosure",
        ),
        AbortReason::DeadlineExceeded => error(
            QueryErrorCode::DeadlineExceeded,
            "query deadline exceeded before disclosure",
        ),
    }
}

fn read_metadata(
    connection: &Connection,
    admission: AdapterAdmissionBudget,
) -> Result<(BTreeMap<String, String>, AdapterAdmissionCharge), QueryError> {
    // One COUNT scalar and four aggregate scalars are returned even for an
    // empty table. Refuse before their transfer when the caller cannot admit
    // that fixed decoded-column cost.
    const PREFLIGHT_SCALARS_BYTES: u64 = 5 * 8;
    if admission.max_metadata_rows == 0
        || admission.max_metadata_decoded_bytes < PREFLIGHT_SCALARS_BYTES
        || admission.max_metadata_vm_steps == 0
    {
        return Err(error(
            QueryErrorCode::BudgetExceeded,
            "selected metadata admission budget absent",
        ));
    }
    let ((values, decoded_bytes, row_count), steps) = budgeted(
        connection,
        admission.max_metadata_vm_steps,
        None,
        || {
            let row_limit = admission.max_metadata_rows.min(METADATA_ROWS_CAP);
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM (SELECT 1 FROM metadata LIMIT ?1)",
                    params![(row_limit + 1) as i64],
                    |row| row.get(0),
                )
                .map_err(sql_error)?;
            if count < 0 || count as usize > row_limit {
                return Err(error(
                    QueryErrorCode::BudgetExceeded,
                    "selected metadata row admission exceeded",
                ));
            }
            // A bounded scalar preflight prevents transferring any key/value
            // before its length and the total decoded-column cost are known.
            let (key_bytes, value_bytes, max_key, oversized): (i64, i64, i64, i64) = connection
                .query_row(
                    "SELECT COALESCE(SUM(length(CAST(key AS BLOB))),0),
                            COALESCE(SUM(length(CAST(value AS BLOB))),0),
                            COALESCE(MAX(length(CAST(key AS BLOB))),0),
                            COALESCE(SUM(CASE WHEN (key='authority_boundary' AND length(CAST(value AS BLOB))>?1)
                                  OR (key<>'authority_boundary' AND length(CAST(value AS BLOB))>?2)
                                  THEN 1 ELSE 0 END),0)
                     FROM metadata",
                    params![AUTHORITY_VALUE_BYTES_CAP, METADATA_VALUE_BYTES_CAP],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(sql_error)?;
            if key_bytes < 0 || value_bytes < 0 || max_key < 0 || oversized < 0 {
                return Err(error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "selected metadata lengths invalid",
                ));
            }
            if max_key as u64 > METADATA_KEY_BYTES_CAP || oversized != 0 {
                return Err(error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "selected metadata field oversized",
                ));
            }
            let decoded_bytes = PREFLIGHT_SCALARS_BYTES
                .checked_add(key_bytes as u64)
                .and_then(|value| value.checked_add(value_bytes as u64))
                .ok_or_else(|| {
                    error(
                        QueryErrorCode::BudgetExceeded,
                        "selected metadata byte total overflow",
                    )
                })?;
            if decoded_bytes > admission.max_metadata_decoded_bytes {
                return Err(error(
                    QueryErrorCode::BudgetExceeded,
                    "selected metadata decoded-byte admission exceeded",
                ));
            }
            let mut statement = connection
                .prepare("SELECT CASE WHEN length(CAST(key AS BLOB))<=?1 THEN key END,
                                 CASE WHEN (key='authority_boundary' AND length(CAST(value AS BLOB))<=?2)
                                        OR (key<>'authority_boundary' AND length(CAST(value AS BLOB))<=?3)
                                      THEN value END
                          FROM metadata LIMIT ?4")
                .map_err(sql_error)?;
            let rows = statement
                .query_map(
                    params![
                        METADATA_KEY_BYTES_CAP,
                        AUTHORITY_VALUE_BYTES_CAP,
                        METADATA_VALUE_BYTES_CAP,
                        (row_limit + 1) as i64
                    ],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .map_err(sql_error)?;
            let mut values = BTreeMap::new();
            for row in rows {
                let (key, value) = row.map_err(sql_error)?;
                let (Some(key), Some(value)) = (key, value) else {
                    return Err(error(
                        QueryErrorCode::CorruptSelectedCarrier,
                        "selected metadata field invalid",
                    ));
                };
                if values.insert(key, value).is_some() || values.len() > row_limit {
                    return Err(error(
                        QueryErrorCode::CorruptSelectedCarrier,
                        "selected metadata keys invalid",
                    ));
                }
            }
            if values.len() != count as usize {
                return Err(error(
                    QueryErrorCode::StaleSelection,
                    "selected metadata row count changed during admission",
                ));
            }
            Ok((values, decoded_bytes, count as u64))
        },
    )?;
    Ok((
        values,
        AdapterAdmissionCharge {
            selected_open_vm_steps: 0,
            metadata_vm_steps: steps,
            metadata_decoded_bytes: decoded_bytes,
            metadata_rows: row_count,
        },
    ))
}

fn metadata<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, QueryError> {
    values.get(key).map(String::as_str).ok_or_else(|| {
        error(
            QueryErrorCode::CorruptSelectedCarrier,
            "required selected metadata absent",
        )
    })
}

fn digest(hex: &str) -> Result<Digest256, QueryError> {
    Digest256::from_hex(hex).map_err(|_| {
        error(
            QueryErrorCode::CorruptSelectedCarrier,
            "selected digest invalid",
        )
    })
}

fn selected_carrier(
    binding: &Binding,
    raw: Vec<u8>,
    expected_size: i64,
    expected_sha: &str,
) -> Result<RawRecord, QueryError> {
    if expected_size < 0 || raw.len() as u64 != expected_size as u64 {
        return Err(error(
            QueryErrorCode::CorruptSelectedCarrier,
            "selected carrier length mismatch",
        ));
    }
    let sha256 = digest(expected_sha)?;
    if Digest256::of_bytes(&raw) != sha256 {
        return Err(error(
            QueryErrorCode::CorruptSelectedCarrier,
            "selected carrier SHA mismatch",
        ));
    }
    Ok(RawRecord {
        binding: binding.clone(),
        raw,
        sha256,
    })
}

impl<P: PinnedLocalModel, G: CurrentPolicy> SqliteReadModel<P, G> {
    pub fn new(
        mut pinned: P,
        policy: G,
        disclosure_scope: DisclosureScope,
        admission: AdapterAdmissionBudget,
    ) -> Result<Self, QueryError> {
        pinned.check_pin()?;
        if admission.max_selected_open_vm_steps == 0
            || pinned.selected_open_vm_steps() > admission.max_selected_open_vm_steps
        {
            return Err(error(
                QueryErrorCode::BudgetExceeded,
                "selected model opener VM admission exceeded",
            ));
        }
        if pinned.owner_receipt_id().is_empty()
            || pinned.binding().index_root != pinned.selected_model_sha256()
        {
            return Err(error(
                QueryErrorCode::StaleSelection,
                "selected model receipt or SHA binding invalid",
            ));
        }
        let binding = pinned.binding().clone();
        disclosure_scope.validate(&binding, pinned.owner_receipt_id())?;
        let (values, mut admission_charge) = read_metadata(pinned.connection(), admission)?;
        admission_charge.selected_open_vm_steps = pinned.selected_open_vm_steps();
        let membership_hex = binding.membership_root.to_hex();
        let projection_hex = binding.projection_root.to_hex();
        for (key, expected) in [
            ("model_abi", binding.model_abi.as_str()),
            ("selection_profile", binding.selection_profile.as_str()),
            ("source_cut", binding.source_cut.as_str()),
            ("membership_root", membership_hex.as_str()),
            ("projection_root_sha256", projection_hex.as_str()),
            ("index_generation", binding.index_generation.as_str()),
            ("route_map_version", binding.route_map_version.as_str()),
            ("reader_abi", binding.reader_abi.as_str()),
        ] {
            if metadata(&values, key)? != expected {
                return Err(error(
                    QueryErrorCode::StaleSelection,
                    "selected model metadata binding mismatch",
                ));
            }
        }
        if metadata(&values, "through_commit_seq")?.parse::<u64>().ok()
            != Some(binding.through_commit_seq)
            || metadata(&values, "complete")? != "true"
            || binding.model_abi != "tos_source_navigation_read_model_v1"
            || binding.selection_profile != "tos_source_navigation_visible_v1"
        {
            return Err(error(
                QueryErrorCode::StaleSelection,
                "selected model ABI or completeness invalid",
            ));
        }
        let note = metadata(&values, "authority_boundary")?;
        if note.is_empty() || note != pinned.source_authority_boundary() {
            return Err(error(
                QueryErrorCode::CorruptSelectedCarrier,
                "source authority note absent",
            ));
        }
        let raw = json_string(note);
        let authority = RawRecord {
            binding,
            sha256: Digest256::of_bytes(&raw),
            raw,
        };
        pinned.check_pin()?;
        Ok(Self {
            pinned,
            policy,
            disclosure_scope,
            authority,
            admission_charge,
            abort_probe: None,
        })
    }

    pub fn admission_charge(&self) -> AdapterAdmissionCharge {
        self.admission_charge
    }

    pub fn disclosure_scope(&self) -> &DisclosureScope {
        &self.disclosure_scope
    }

    pub fn set_abort_probe(&mut self, probe: Option<Arc<dyn AbortProbe>>) {
        self.abort_probe = probe;
    }
}

impl<S: SourcePin, G: CurrentPolicy> SqliteReadModel<CmpPinnedModel<S>, G> {
    /// Each concurrent reader gets its own SQLite connection, source pin and
    /// current policy. CMP reuses the cold-verified inode without full rehash.
    pub fn fork_reader<T: SourcePin, H: CurrentPolicy>(
        &self,
        source_pin: T,
        policy: H,
        disclosure_scope: DisclosureScope,
        admission: AdapterAdmissionBudget,
    ) -> Result<SqliteReadModel<CmpPinnedModel<T>, H>, QueryError> {
        SqliteReadModel::new(
            self.pinned
                .fork_reader(source_pin, admission.max_selected_open_vm_steps)?,
            policy,
            disclosure_scope,
            admission,
        )
    }
}

impl<P: PinnedLocalModel, G: CurrentPolicy> ReadModel for SqliteReadModel<P, G> {
    fn check_interrupt(&mut self) -> Result<(), QueryError> {
        if let Some(reason) = self.abort_probe.as_ref().and_then(|probe| probe.reason()) {
            return Err(abort_error(reason));
        }
        Ok(())
    }
    fn selected_binding(&mut self) -> Result<Binding, QueryError> {
        self.pinned.check_pin()?;
        Ok(self.pinned.binding().clone())
    }

    fn exact_visible_node(
        &mut self,
        id: &str,
        max_bytes: usize,
        max_carrier_bytes: usize,
        max_vm_steps: u64,
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<ExactNode, QueryError> {
        if (max_bytes as u64) < NODE_HEADER_RESULT_BYTES || max_work_probes < 1 || max_work_rows < 1
        {
            return Err(error(
                QueryErrorCode::BudgetExceeded,
                "selected node metadata over byte cap",
            ));
        }
        let body_cap =
            ((max_bytes as u64) - NODE_HEADER_RESULT_BYTES).min(max_carrier_bytes as u64);
        let binding = self.pinned.binding().clone();
        let connection = self.pinned.connection();
        let ((record, header_rows, probes), steps) =
            budgeted(connection, max_vm_steps, self.abort_probe.as_ref(), || {
                let header: Option<(i64, i64, Option<String>)> = connection
                    .query_row(
                        "SELECT visible,carrier_size,
                                CASE WHEN length(CAST(carrier_sha256 AS BLOB))=?2
                                     THEN carrier_sha256 END
                         FROM nodes WHERE node_id=?1",
                        params![id, SHA256_HEX_RESULT_BYTES],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()
                    .map_err(sql_error)?;
                let Some((visible, size, sha)) = header else {
                    return Ok((None, 0, 1));
                };
                let sha = sha.ok_or_else(|| {
                    error(
                        QueryErrorCode::CorruptSelectedCarrier,
                        "selected node SHA size invalid",
                    )
                })?;
                digest(&sha)?;
                if size < 0 || (visible != 0 && visible != 1) {
                    return Err(error(
                        QueryErrorCode::CorruptSelectedCarrier,
                        "visible node metadata invalid",
                    ));
                }
                if visible == 0 {
                    return Ok((None, 1, 1));
                }
                if size as u64 > body_cap {
                    return Err(error(
                        QueryErrorCode::BudgetExceeded,
                        "selected node over byte cap",
                    ));
                }
                if max_work_probes < 2 || max_work_rows < 2 {
                    return Err(error(
                        QueryErrorCode::BudgetExceeded,
                        "selected node carrier row admission exceeded",
                    ));
                }
                let raw: Option<Vec<u8>> = connection
                    .query_row(
                        "SELECT CASE WHEN length(carrier)<=?2 AND carrier_size<=?2 THEN carrier END
                 FROM nodes WHERE node_id=?1 AND visible=1",
                        params![id, i64::try_from(body_cap).unwrap_or(i64::MAX)],
                        |row| row.get(0),
                    )
                    .map_err(sql_error)?;
                let raw = raw.ok_or_else(|| {
                    error(
                        QueryErrorCode::BudgetExceeded,
                        "selected node actual carrier over byte cap",
                    )
                })?;
                Ok((Some(selected_carrier(&binding, raw, size, &sha)?), 2, 2))
            })?;
        Ok(ExactNode {
            binding,
            complete_unique_lookup: true,
            charged: Charged {
                probes,
                rows: header_rows,
                bytes: if header_rows == 0 {
                    0
                } else {
                    NODE_HEADER_RESULT_BYTES
                },
                cpu_steps: steps,
            },
            record,
        })
    }

    fn visible_outgoing(
        &mut self,
        from_id: &str,
        after_edge_id: Option<&str>,
        max_rows: usize,
        max_bytes: usize,
        max_carrier_bytes: usize,
        max_vm_steps: u64,
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<AdjacencyPage, QueryError> {
        if max_rows == 0 || max_rows > 1024 {
            return Err(error(
                QueryErrorCode::InvalidRequest,
                "invalid adjacency page size",
            ));
        }
        const PREFLIGHT_RESULT_BYTES: u64 = 4 * SQLITE_I64_RESULT_BYTES;
        let minimum = ADJACENCY_CERT_RESULT_BYTES + PREFLIGHT_RESULT_BYTES;
        if (max_bytes as u64) < minimum || max_work_probes < 2 || max_work_rows < 2 {
            return Err(error(
                QueryErrorCode::BudgetExceeded,
                "selected adjacency metadata preflight over byte cap",
            ));
        }
        let binding = self.pinned.binding().clone();
        let connection = self.pinned.connection();
        let ((expected_count, expected_digest, edges, exhausted, scanned, meta_bytes), steps) =
            budgeted(connection, max_vm_steps, self.abort_probe.as_ref(), || {
                let certificate: Option<(i64, Option<String>)> = connection
                    .query_row(
                        "SELECT edge_count,
                                CASE WHEN length(CAST(edges_sha256 AS BLOB))=?2
                                     THEN edges_sha256 END
                         FROM adjacency WHERE from_id=?1",
                        params![from_id, SHA256_HEX_RESULT_BYTES],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()
                    .map_err(sql_error)?;
                let (count, sha) = certificate.ok_or_else(|| {
                    error(
                        QueryErrorCode::IndexIncomplete,
                        "visible adjacency certificate absent",
                    )
                })?;
                if count < 0 {
                    return Err(error(
                        QueryErrorCode::IndexIncomplete,
                        "visible adjacency count invalid",
                    ));
                }
                let sha = sha.ok_or_else(|| {
                    error(
                        QueryErrorCode::IndexIncomplete,
                        "visible adjacency certificate SHA size invalid",
                    )
                })?;
                let expected_digest = digest(&sha)?;
                // This indexed LIMIT limits SQLite work to one page plus the
                // exhaustion lookahead. The aggregate returns four i64s, not
                // edge IDs or SHA strings; no unbounded text reaches Rust.
                let (preflight_count, header_bytes, max_sha_len, min_sha_len):
                    (i64, i64, i64, i64) = connection.query_row(
                    "SELECT COUNT(*),
                            COALESCE(SUM(length(CAST(edge_id AS BLOB)) + 8 + length(CAST(carrier_sha256 AS BLOB))),0),
                            COALESCE(MAX(length(CAST(carrier_sha256 AS BLOB))),0),
                            COALESCE(MIN(length(CAST(carrier_sha256 AS BLOB))),0)
                     FROM (SELECT edge_id,carrier_sha256 FROM edges
                           WHERE visible=1 AND from_id=?1 AND edge_id>?2
                           ORDER BY edge_id LIMIT ?3)",
                    params![from_id, after_edge_id.unwrap_or(""), (max_rows + 1) as i64],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                ).map_err(sql_error)?;
                if preflight_count < 0
                    || preflight_count as usize > max_rows + 1
                    || header_bytes < 0
                    || max_sha_len < 0
                    || min_sha_len < 0
                    || (preflight_count > 0
                        && (max_sha_len as u64 != SHA256_HEX_RESULT_BYTES
                            || min_sha_len as u64 != SHA256_HEX_RESULT_BYTES))
                {
                    return Err(error(
                        QueryErrorCode::IndexIncomplete,
                        "selected adjacency header lengths invalid",
                    ));
                }
                let meta_bytes = minimum.checked_add(header_bytes as u64).ok_or_else(|| {
                    error(
                        QueryErrorCode::BudgetExceeded,
                        "selected adjacency metadata overflow",
                    )
                })?;
                if meta_bytes > max_bytes as u64 {
                    return Err(error(
                        QueryErrorCode::BudgetExceeded,
                        "selected adjacency metadata over byte cap",
                    ));
                }
                let scanned = preflight_count as u64;
                let emitted = scanned.min(max_rows as u64);
                if 3 + emitted > max_work_probes || 2 + scanned + emitted > max_work_rows {
                    return Err(error(
                        QueryErrorCode::BudgetExceeded,
                        "selected adjacency row or probe admission exceeded",
                    ));
                }
                let mut stmt = connection.prepare(
                    "SELECT CASE WHEN length(CAST(edge_id AS BLOB))<=?4 THEN edge_id END,
                            carrier_size,
                            CASE WHEN length(CAST(carrier_sha256 AS BLOB))=?5 THEN carrier_sha256 END
                     FROM edges WHERE visible=1 AND from_id=?1 AND edge_id>?2
                     ORDER BY edge_id LIMIT ?3"
                ).map_err(sql_error)?;
                let rows = stmt
                    .query_map(
                        params![
                            from_id,
                            after_edge_id.unwrap_or(""),
                            (max_rows + 1) as i64,
                            i64::try_from(max_bytes).unwrap_or(i64::MAX),
                            SHA256_HEX_RESULT_BYTES
                        ],
                        |row| {
                            Ok((
                                row.get::<_, Option<String>>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, Option<String>>(2)?,
                            ))
                        },
                    )
                    .map_err(sql_error)?;
                let mut headers = Vec::new();
                for row in rows {
                    let (edge_id, size, sha) = row.map_err(sql_error)?;
                    let (Some(edge_id), Some(sha)) = (edge_id, sha) else {
                        return Err(error(
                            QueryErrorCode::IndexIncomplete,
                            "selected adjacency header invalid",
                        ));
                    };
                    digest(&sha)?;
                    headers.push((edge_id, size, sha));
                }
                if headers.len() != preflight_count as usize {
                    return Err(error(
                        QueryErrorCode::StaleSelection,
                        "selected adjacency changed during preflight",
                    ));
                }
                let scanned = headers.len() as u64;
                let exhausted = headers.len() <= max_rows;
                headers.truncate(max_rows);
                let mut edges = Vec::with_capacity(headers.len());
                let mut transferred = 0u64;
                for (edge_id, size, sha) in headers {
                    let remaining = (max_bytes as u64)
                        .saturating_sub(meta_bytes)
                        .saturating_sub(transferred);
                    if size < 0 || size as u64 > remaining.min(max_carrier_bytes as u64) {
                        return Err(error(
                            QueryErrorCode::BudgetExceeded,
                            "selected adjacency over byte cap",
                        ));
                    }
                    let raw: Option<Vec<u8>> = connection.query_row(
                    "SELECT CASE WHEN length(carrier)<=?3 AND carrier_size<=?3 THEN carrier END
                     FROM edges WHERE edge_id=?1 AND visible=1 AND from_id=?2",
                    params![edge_id, from_id, i64::try_from(remaining.min(max_carrier_bytes as u64)).unwrap_or(i64::MAX)], |row| row.get(0),
                ).map_err(sql_error)?;
                    let raw = raw.ok_or_else(|| {
                        error(
                            QueryErrorCode::BudgetExceeded,
                            "selected adjacency actual carrier over byte cap",
                        )
                    })?;
                    transferred += raw.len() as u64;
                    edges.push(selected_carrier(&binding, raw, size, &sha)?);
                }
                Ok((
                    count as u64,
                    expected_digest,
                    edges,
                    exhausted,
                    scanned,
                    meta_bytes,
                ))
            })?;
        let emitted_count = edges.len() as u64;
        Ok(AdjacencyPage {
            binding,
            from_id: from_id.to_owned(),
            after_edge_id: after_edge_id.map(str::to_owned),
            edges,
            exhausted,
            expected_count,
            expected_digest,
            charged: Charged {
                probes: 3 + emitted_count,
                rows: scanned + 2 + emitted_count,
                bytes: meta_bytes,
                cpu_steps: steps,
            },
        })
    }

    fn authorize_current(&mut self, record: &RawRecord) -> Result<Charged, QueryError> {
        self.policy
            .authorize_current(&self.disclosure_scope, record)
    }

    fn authority_boundary(&mut self, max_bytes: usize) -> Result<RawRecord, QueryError> {
        if self.authority.raw.len() > max_bytes {
            return Err(error(
                QueryErrorCode::BudgetExceeded,
                "selected authority over byte cap",
            ));
        }
        Ok(self.authority.clone())
    }

    fn check_pin(&mut self, binding: &Binding) -> Result<(), QueryError> {
        if self.pinned.binding() != binding {
            return Err(error(
                QueryErrorCode::StaleSelection,
                "selected model binding changed",
            ));
        }
        self.pinned.check_pin()
    }

    fn acquire_disclosure(
        &mut self,
        binding: &Binding,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease>, QueryError> {
        self.check_pin(binding)?;
        let source = self.pinned.source_disclosure()?;
        self.disclosure_scope
            .validate(binding, self.pinned.owner_receipt_id())?;
        let rights = self
            .policy
            .acquire_disclosure(&self.disclosure_scope, selected)?;
        let mut combined = CombinedLease { source, rights };
        combined.recheck()?;
        Ok(Box::new(combined))
    }
}
