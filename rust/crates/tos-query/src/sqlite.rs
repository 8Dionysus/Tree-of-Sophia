//! Native seek adapter for an owner-opened, pinned CMP navigation model.
//! This module never resolves selected.json or an artifact pathname.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
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
const OPEN_VM_CAP: u64 = 1_000_000;

/// Implement only for an owner-verified selected model whose SQLite connection
/// holds the already-digested immutable inode. The receipt, full source cut,
/// index completeness and live pin are established by that owner, not QRY.
pub trait PinnedLocalModel {
    fn binding(&self) -> &Binding;
    fn selected_model_sha256(&self) -> Digest256;
    fn owner_receipt_id(&self) -> &str;
    fn source_authority_boundary(&self) -> &str;
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
    ) -> Result<CmpPinnedModel<T>, QueryError> {
        let model = self.model.fork_reader().map_err(|_| {
            error(
                QueryErrorCode::StaleSelection,
                "selected model warm reader unavailable",
            )
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

/// Source-owner current policy. Historical selection does not freeze rights.
pub trait CurrentPolicy {
    fn authorize_current(&mut self, record: &RawRecord) -> Result<Charged, QueryError>;
    /// Recheck all selected records under one current-policy hold. It must
    /// serialize revocation through disclosure, not return a stale observation.
    fn acquire_disclosure(
        &mut self,
        binding: &Binding,
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
    authority: RawRecord,
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
    run: impl FnOnce() -> Result<T, QueryError>,
) -> Result<(T, u64), QueryError> {
    if max_vm_steps == 0 {
        return Err(error(
            QueryErrorCode::BudgetExceeded,
            "SQLite VM budget absent",
        ));
    }
    let steps = Arc::new(AtomicU64::new(0));
    let observed = Arc::clone(&steps);
    connection.progress_handler(
        1,
        Some(move || observed.fetch_add(1, Ordering::Relaxed) >= max_vm_steps),
    );
    let result = run();
    connection.progress_handler(0, None::<fn() -> bool>);
    let charged = steps.load(Ordering::Relaxed);
    if charged > max_vm_steps {
        return Err(error(
            QueryErrorCode::BudgetExceeded,
            "SQLite VM budget exhausted",
        ));
    }
    result.map(|value| (value, charged))
}

fn read_metadata(connection: &Connection) -> Result<BTreeMap<String, String>, QueryError> {
    let (values, _) = budgeted(connection, OPEN_VM_CAP, || {
        let mut statement = connection
            .prepare("SELECT key, CASE WHEN (key='authority_boundary' AND length(CAST(value AS BLOB))<=262144) OR (key<>'authority_boundary' AND length(CAST(value AS BLOB))<=4096) THEN value END FROM metadata LIMIT ?1")
            .map_err(sql_error)?;
        let rows = statement
            .query_map(params![(METADATA_ROWS_CAP + 1) as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(sql_error)?;
        let mut values = BTreeMap::new();
        for row in rows {
            let (key, value) = row.map_err(sql_error)?;
            let value = value.ok_or_else(|| {
                error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "selected metadata value oversized",
                )
            })?;
            if key.len() > 128
                || values.insert(key, value).is_some()
                || values.len() > METADATA_ROWS_CAP
            {
                return Err(error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "selected metadata keys invalid",
                ));
            }
        }
        Ok(values)
    })?;
    Ok(values)
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
    pub fn new(mut pinned: P, policy: G) -> Result<Self, QueryError> {
        pinned.check_pin()?;
        if pinned.owner_receipt_id().is_empty()
            || pinned.binding().index_root != pinned.selected_model_sha256()
        {
            return Err(error(
                QueryErrorCode::StaleSelection,
                "selected model receipt or SHA binding invalid",
            ));
        }
        let binding = pinned.binding().clone();
        let values = read_metadata(pinned.connection())?;
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
            authority,
        })
    }
}

impl<S: SourcePin, G: CurrentPolicy> SqliteReadModel<CmpPinnedModel<S>, G> {
    /// Each concurrent reader gets its own SQLite connection, source pin and
    /// current policy. CMP reuses the cold-verified inode without full rehash.
    pub fn fork_reader<T: SourcePin, H: CurrentPolicy>(
        &self,
        source_pin: T,
        policy: H,
    ) -> Result<SqliteReadModel<CmpPinnedModel<T>, H>, QueryError> {
        SqliteReadModel::new(self.pinned.fork_reader(source_pin)?, policy)
    }
}

impl<P: PinnedLocalModel, G: CurrentPolicy> ReadModel for SqliteReadModel<P, G> {
    fn selected_binding(&mut self) -> Result<Binding, QueryError> {
        self.pinned.check_pin()?;
        Ok(self.pinned.binding().clone())
    }

    fn exact_visible_node(
        &mut self,
        id: &str,
        max_bytes: usize,
        max_vm_steps: u64,
    ) -> Result<ExactNode, QueryError> {
        const HEADER_CHARGE: usize = 72;
        if max_bytes < HEADER_CHARGE {
            return Err(error(
                QueryErrorCode::BudgetExceeded,
                "selected node metadata over byte cap",
            ));
        }
        let body_cap = max_bytes - HEADER_CHARGE;
        let binding = self.pinned.binding().clone();
        let connection = self.pinned.connection();
        let (record, steps) =
            budgeted(connection, max_vm_steps, || {
                let header: Option<(i64, i64, String)> = connection
                    .query_row(
                        "SELECT visible,carrier_size,carrier_sha256 FROM nodes WHERE node_id=?1",
                        params![id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()
                    .map_err(sql_error)?;
                let Some((visible, size, sha)) = header else {
                    return Ok(None);
                };
                if visible == 0 {
                    return Ok(None);
                }
                if visible != 1 || size < 0 {
                    return Err(error(
                        QueryErrorCode::CorruptSelectedCarrier,
                        "visible node metadata invalid",
                    ));
                }
                if size as u64 > body_cap as u64 {
                    return Err(error(
                        QueryErrorCode::BudgetExceeded,
                        "selected node over byte cap",
                    ));
                }
                let raw: Vec<u8> = connection.query_row(
                "SELECT carrier FROM nodes WHERE node_id=?1 AND visible=1 AND carrier_size<=?2",
                params![id, i64::try_from(body_cap).unwrap_or(i64::MAX)], |row| row.get(0),
            ).map_err(sql_error)?;
                selected_carrier(&binding, raw, size, &sha).map(Some)
            })?;
        Ok(ExactNode {
            binding,
            complete_unique_lookup: true,
            charged: Charged {
                probes: 1,
                rows: 1,
                bytes: HEADER_CHARGE as u64,
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
        max_vm_steps: u64,
    ) -> Result<AdjacencyPage, QueryError> {
        if max_rows == 0 || max_rows > 1024 {
            return Err(error(
                QueryErrorCode::InvalidRequest,
                "invalid adjacency page size",
            ));
        }
        let binding = self.pinned.binding().clone();
        let connection = self.pinned.connection();
        let ((expected_count, expected_digest, edges, exhausted, scanned, meta_bytes), steps) =
            budgeted(connection, max_vm_steps, || {
                let certificate: Option<(i64, String)> = connection
                    .query_row(
                        "SELECT edge_count,edges_sha256 FROM adjacency WHERE from_id=?1",
                        params![from_id],
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
                let expected_digest = digest(&sha)?;
                let mut stmt = connection.prepare(
                "SELECT edge_id,carrier_size,carrier_sha256 FROM edges WHERE visible=1 AND from_id=?1 AND edge_id>?2 ORDER BY edge_id LIMIT ?3"
            ).map_err(sql_error)?;
                let rows = stmt
                    .query_map(
                        params![from_id, after_edge_id.unwrap_or(""), (max_rows + 1) as i64],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, String>(2)?,
                            ))
                        },
                    )
                    .map_err(sql_error)?;
                let mut headers = Vec::new();
                let mut meta_bytes = 0u64;
                for row in rows {
                    let header = row.map_err(sql_error)?;
                    meta_bytes =
                        meta_bytes.saturating_add((header.0.len() + header.2.len() + 8) as u64);
                    if meta_bytes > max_bytes as u64 {
                        return Err(error(
                            QueryErrorCode::BudgetExceeded,
                            "selected adjacency metadata over byte cap",
                        ));
                    }
                    headers.push(header);
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
                    if size < 0 || size as u64 > remaining {
                        return Err(error(
                            QueryErrorCode::BudgetExceeded,
                            "selected adjacency over byte cap",
                        ));
                    }
                    let raw: Vec<u8> = connection.query_row(
                    "SELECT carrier FROM edges WHERE edge_id=?1 AND visible=1 AND from_id=?2 AND carrier_size<=?3",
                    params![edge_id, from_id, i64::try_from(remaining).unwrap_or(i64::MAX)], |row| row.get(0),
                ).map_err(sql_error)?;
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
        Ok(AdjacencyPage {
            binding,
            from_id: from_id.to_owned(),
            after_edge_id: after_edge_id.map(str::to_owned),
            edges,
            exhausted,
            expected_count,
            expected_digest,
            charged: Charged {
                probes: 2 + scanned,
                rows: scanned + 1,
                bytes: meta_bytes,
                cpu_steps: steps,
            },
        })
    }

    fn authorize_current(&mut self, record: &RawRecord) -> Result<Charged, QueryError> {
        self.policy.authorize_current(record)
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
        let rights = self.policy.acquire_disclosure(binding, selected)?;
        let mut combined = CombinedLease { source, rights };
        combined.recheck()?;
        Ok(Box::new(combined))
    }
}
