//! Bounded, private SQL pair for one owner-selected prepared/D1 transition.
//!
//! This is a mechanical producer. The caller admits source, rights and the
//! prepared/D1 pair, and supplies complete affected rows including digest,
//! search, lens and metadata companions. This module verifies selected old
//! rows at publication, but does not certify affected dependency closure.
//! A local SQLite candidate cannot supply those grants.
//! The caller exclusively owns the artifact directory throughout emission and
//! cleanup. Path identity checks detect replacements; they are not a lock
//! against active concurrent writers between filesystem operations.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use tos_foundation::{Digest256, Digest256Hasher};

pub const D1_PAIR_SCHEMA: &str = "tos_rust_prepared_d1_sql_pair_v1";
const MAX_STATEMENT_BYTES: usize = 100_000;
pub const MAX_D1_SQL_ROW_VALUE_BYTES: usize = 2_000_000;
const TEXT_CHUNK_BYTES: usize = 16_000;
const READ_MODEL_SCHEMA: &str = "tos_cloudflare_edge_read_model_v9";

#[derive(Clone, Copy, Debug)]
pub struct D1PairLimits {
    pub max_transitions: u64,
    pub max_work_bytes: u64,
    pub max_sql_bytes: u64,
    /// Maximum rendered SQL literal row size including its fixed margin.
    /// Existing callers keep the published 2 MB bound; the offline prepared
    /// adapter can pass its separately declared 4 MB row allowance.
    pub max_row_bytes: usize,
}
impl Default for D1PairLimits {
    fn default() -> Self {
        Self {
            max_transitions: 512,
            max_work_bytes: 128 * 1024 * 1024,
            max_sql_bytes: 128 * 1024 * 1024,
            max_row_bytes: MAX_D1_SQL_ROW_VALUE_BYTES,
        }
    }
}

/// An exact selected source/prepared/D1 pair supplied by the owner. The
/// producer checks shape and emitted SQL guards, not independent admission.
#[derive(Clone, Debug)]
pub struct D1PairInput {
    /// Exact mechanical lineage form. Catch-up has no predecessor prepared
    /// database or binding, so it must name its retained source-input proof
    /// instead of filling that gap with the successor binding.
    pub predecessor_mode: D1PredecessorMode,
    pub base_d1_revision: String,
    pub before_source_revision: String,
    pub after_source_revision: String,
    pub before_prepared_binding: String,
    pub after_prepared_binding: String,
    pub before_source_inputs_sha256: String,
    pub after_source_inputs_sha256: String,
    pub before_navigation_sha256: String,
    pub after_navigation_sha256: String,
    pub before_rights_sha256: Option<String>,
    pub after_rights_sha256: Option<String>,
    pub implementation_sha256: String,
    /// Exact Rust source digest for a one-time source-navigation integrity
    /// migration. Existing prepared-pair lineages leave this absent.
    pub migration_implementation_sha256: Option<String>,
    /// Exact installed optional derived stores. Each named store has its own
    /// predecessor state guard and successor epoch seal in both SQL directions.
    pub auxiliary_stores: Vec<D1AuxiliaryStore>,
    /// Exact admitted edge_meta JSON, including its original member order.
    pub before_reader_top: String,
    pub after_reader_top: String,
    pub limits: D1PairLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum D1PredecessorMode {
    PreparedDelta,
    SourceInputsCatchup,
    Bootstrap,
    /// Add native navigation integrity companions to a present product. The
    /// header-only form retains, but does not re-audit, existing row companions.
    SourceNavigationIntegrity {
        header_only: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum D1AuxiliaryStore {
    CompactLens,
    LensMemberships,
}
impl D1AuxiliaryStore {
    pub fn identity(self) -> (&'static str, &'static str, D1Table) {
        match self {
            Self::CompactLens => (
                "knowledge_compact_lens",
                "knowledge_compact_lens_state",
                D1Table::KnowledgeCompactLens,
            ),
            Self::LensMemberships => (
                "knowledge_lens_memberships",
                "knowledge_lens_membership_state",
                D1Table::KnowledgeLensMemberships,
            ),
        }
    }
    pub fn schema(self) -> &'static str {
        match self {
            Self::CompactLens => "tos_compact_lens_carrier_v1",
            Self::LensMemberships => "tos_lens_membership_index_v1",
        }
    }
}

/// Fixed, published v9 table shapes. Every transition must include the full
/// row; SQL compares the selected predecessor before touching serving rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum D1Table {
    KnowledgeNodes,
    KnowledgeRelations,
    KnowledgeSearchDocuments,
    KnowledgeSearchGrams,
    KnowledgeSearchGramStats,
    KnowledgeLensOrder,
    SourceNavigationNodes,
    SourceNavigationNodePayload,
    SourceNavigationEdges,
    SourceNavigationEdgePayload,
    SourceNavigationRights,
    SourceNavigationRightsPayload,
    KnowledgeCompactLens,
    KnowledgeLensMemberships,
    EdgeMeta,
}
impl D1Table {
    const CORE: [Self; 13] = [
        Self::KnowledgeNodes,
        Self::KnowledgeRelations,
        Self::KnowledgeSearchDocuments,
        Self::KnowledgeSearchGrams,
        Self::KnowledgeSearchGramStats,
        Self::KnowledgeLensOrder,
        Self::SourceNavigationNodes,
        Self::SourceNavigationNodePayload,
        Self::SourceNavigationEdges,
        Self::SourceNavigationEdgePayload,
        Self::SourceNavigationRights,
        Self::SourceNavigationRightsPayload,
        Self::EdgeMeta,
    ];
    fn selected(spec: &D1PairInput) -> Vec<Self> {
        let mut tables = Self::CORE.to_vec();
        tables.extend(spec.auxiliary_stores.iter().map(|store| store.identity().2));
        tables
    }
    pub fn shape(
        self,
    ) -> (
        &'static str,
        &'static [&'static str],
        &'static [&'static str],
    ) {
        match self {
            Self::KnowledgeNodes => (
                "knowledge_nodes",
                &[
                    "id",
                    "entity_id",
                    "native_id",
                    "source_graph",
                    "kind_id",
                    "type_id",
                    "title_text",
                    "summary_text",
                    "search_text",
                    "json",
                ],
                &["id"],
            ),
            Self::KnowledgeRelations => (
                "knowledge_relations",
                &[
                    "id",
                    "native_id",
                    "source_graph",
                    "from_id",
                    "to_id",
                    "predicate_id",
                    "relation_type_id",
                    "label_text",
                    "explanation_text",
                    "search_text",
                    "json",
                ],
                &["id"],
            ),
            Self::KnowledgeSearchDocuments => (
                "knowledge_search_documents",
                &[
                    "kind",
                    "position",
                    "id",
                    "source_graph",
                    "kind_id",
                    "predicate_id",
                    "id_lower",
                    "native_id_lower",
                    "identity_values",
                    "visible_values",
                    "document_chars",
                    "document_digest",
                ],
                &["kind", "position"],
            ),
            Self::KnowledgeSearchGrams => (
                "knowledge_search_grams",
                &["kind", "n", "gram", "position"],
                &["kind", "n", "gram", "position"],
            ),
            Self::KnowledgeSearchGramStats => (
                "knowledge_search_gram_stats",
                &["kind", "n", "gram", "postings"],
                &["kind", "n", "gram"],
            ),
            Self::KnowledgeLensOrder => (
                "knowledge_lens_order",
                &["kind", "id", "sort_key", "from_id", "to_id"],
                &["kind", "id"],
            ),
            Self::SourceNavigationNodes => (
                "source_navigation_nodes",
                &[
                    "node_id",
                    "ord",
                    "node_kind",
                    "source_ref",
                    "label",
                    "identity_status",
                    "properties_json",
                    "json",
                ],
                &["node_id"],
            ),
            Self::SourceNavigationNodePayload => (
                "source_navigation_node_payload",
                &["id", "part", "json_chunk"],
                &["id", "part"],
            ),
            Self::SourceNavigationEdges => (
                "source_navigation_edges",
                &[
                    "edge_id",
                    "ord",
                    "from_id",
                    "to_id",
                    "edge_kind",
                    "predicate_id",
                    "review_status",
                    "source_refs_json",
                    "json",
                ],
                &["edge_id"],
            ),
            Self::SourceNavigationEdgePayload => (
                "source_navigation_edge_payload",
                &["id", "part", "json_chunk"],
                &["id", "part"],
            ),
            Self::SourceNavigationRights => (
                "source_navigation_rights",
                &["rights_id", "ord", "scope_refs_json", "json"],
                &["rights_id"],
            ),
            Self::SourceNavigationRightsPayload => (
                "source_navigation_rights_payload",
                &["id", "part", "json_chunk"],
                &["id", "part"],
            ),
            Self::KnowledgeCompactLens => (
                "knowledge_compact_lens",
                &["kind", "id", "source_sha256", "seed_sha256", "json"],
                &["kind", "id"],
            ),
            Self::KnowledgeLensMemberships => (
                "knowledge_lens_memberships",
                &["kind", "field", "value", "id", "sort_key"],
                &["kind", "field", "value", "id"],
            ),
            Self::EdgeMeta => (
                "edge_meta",
                &["key", "part", "json_chunk"],
                &["key", "part"],
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum D1Cell {
    Null,
    Integer(i64),
    Text(String),
}
#[derive(Clone, Debug)]
pub struct D1RowTransition {
    pub table: D1Table,
    pub before: Option<Vec<D1Cell>>,
    pub after: Option<Vec<D1Cell>>,
}

#[derive(Clone, Debug)]
pub struct D1PairReceipt {
    pub schema: &'static str,
    pub base_d1_revision: String,
    pub target_d1_revision: String,
    pub forward_sha256: String,
    pub rollback_sha256: String,
    pub forward_bytes: u64,
    pub rollback_bytes: u64,
    pub changed_rows: u64,
    pub pair_manifest: PathBuf,
    /// Digest of the exact source-owner receipt packet captured by the
    /// selected-pair holder. This is absent for mechanical-only emission.
    pub owner_admission_sha256: Option<String>,
    /// True only for the exact selected-pair/currentness hold. This is not
    /// affected-row closure, semantic, D1-application or consumer admission.
    pub selected_pair_owner_admitted: bool,
    pub d1_applied: bool,
    pub consumer_switched: bool,
}

/// Result for the maintained offline Edge adapter. Unlike D1PairReceipt, its
/// reverse SQL is optional and its independently owned SQL/manifest targets
/// may live in different directories. It still proves mechanics only.
#[derive(Clone, Debug)]
pub struct D1CaptureReceipt {
    pub schema: &'static str,
    pub base_d1_revision: String,
    pub target_d1_revision: String,
    pub forward_sha256: String,
    pub rollback_sha256: Option<String>,
    pub forward_bytes: u64,
    pub rollback_bytes: u64,
    pub changed_rows: u64,
    pub manifest_path: PathBuf,
    pub selected_pair_owner_admitted: bool,
    pub d1_applied: bool,
    pub consumer_switched: bool,
}

#[derive(Debug)]
pub enum D1PairFailure {
    FullOnlyRightsTransition,
    FullOnlyAuxiliaryTransition,
    Invalid(&'static str),
    Budget(&'static str),
    Io(std::io::Error),
}
impl From<std::io::Error> for D1PairFailure {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
pub type D1PairResult<T> = std::result::Result<T, D1PairFailure>;

fn digest(raw: &[u8]) -> String {
    let mut hash = Digest256Hasher::new();
    hash.update(raw);
    hash.finalize().to_hex()
}
fn valid_digest(raw: &str) -> bool {
    Digest256::from_hex(raw).is_ok()
}

/// Exact predecessor bindings accepted by the private lens stores. The
/// Python byte profile is retained only for a previously maintained reader;
/// both candidates bind the caller-selected top and live exploration epoch.
pub fn auxiliary_binding_candidates(top_raw: &str, epoch: u64) -> D1PairResult<Vec<String>> {
    if epoch > 9_007_199_254_740_991 || top_raw.len() > 32_000 {
        return Err(D1PairFailure::Invalid("D1 auxiliary binding epoch/top"));
    }
    let top: Value = serde_json::from_str(top_raw)
        .map_err(|_| D1PairFailure::Invalid("D1 auxiliary binding top JSON"))?;
    let mut result = Vec::new();
    for parts in [
        crate::d1_public_metadata::reader_binding_parts(&top),
        crate::d1_public_metadata::python_reader_binding_parts(&top),
    ] {
        if let Ok((_, prefix, suffix)) = parts {
            let binding = format!("{prefix}{epoch}{suffix}");
            if !result.contains(&binding) {
                result.push(binding);
            }
        }
    }
    if result.is_empty() {
        return Err(D1PairFailure::Invalid("D1 auxiliary reader top"));
    }
    Ok(result)
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn cell_sql(value: &D1Cell) -> String {
    match value {
        D1Cell::Null => "NULL".into(),
        D1Cell::Integer(number) => number.to_string(),
        D1Cell::Text(text) => quote(text),
    }
}
fn row_bytes(values: &[D1Cell], limit: usize) -> D1PairResult<usize> {
    let mut total = 0usize;
    for value in values {
        let bytes = match value {
            D1Cell::Null => 4,
            D1Cell::Integer(value) => value.to_string().len(),
            D1Cell::Text(value) => {
                if value.contains('\0') {
                    return Err(D1PairFailure::Invalid("NUL in D1 SQL text"));
                }
                2usize
                    .checked_add(value.len())
                    .and_then(|size| {
                        size.checked_add(
                            value
                                .as_bytes()
                                .iter()
                                .filter(|byte| **byte == b'\'')
                                .count(),
                        )
                    })
                    .ok_or(D1PairFailure::Budget("row bytes"))?
            }
        };
        total = total
            .checked_add(bytes)
            .ok_or(D1PairFailure::Budget("row bytes"))?;
    }
    if total.checked_add(1024).is_none_or(|size| size > limit) {
        return Err(D1PairFailure::Budget("D1 row bytes"));
    }
    Ok(total)
}
fn row_key(table: D1Table, values: &[D1Cell]) -> D1PairResult<Vec<D1Cell>> {
    let (_, columns, keys) = table.shape();
    if values.len() != columns.len() {
        return Err(D1PairFailure::Invalid("D1 row shape"));
    }
    let mut result = Vec::with_capacity(keys.len());
    for key in keys {
        let index = columns
            .iter()
            .position(|column| column == key)
            .ok_or(D1PairFailure::Invalid("D1 primary key shape"))?;
        let value = values[index].clone();
        if matches!(value, D1Cell::Null) {
            return Err(D1PairFailure::Invalid("null D1 primary key"));
        }
        if matches!(&value, D1Cell::Text(text) if text.is_empty() || text.len() > 4096) {
            return Err(D1PairFailure::Budget("D1 primary key bytes"));
        }
        result.push(value);
    }
    Ok(result)
}
fn validate_row(change: &D1RowTransition, max_row_bytes: usize) -> D1PairResult<Vec<D1Cell>> {
    if change.before.is_none() && change.after.is_none() {
        return Err(D1PairFailure::Invalid("empty D1 transition"));
    }
    let mut key = None;
    for row in [&change.before, &change.after].into_iter().flatten() {
        row_bytes(row, max_row_bytes)?;
        let current = row_key(change.table, row)?;
        if key.as_ref().is_some_and(|prior| prior != &current) {
            return Err(D1PairFailure::Invalid("D1 transition key changed"));
        }
        key = Some(current);
    }
    if change.before == change.after {
        return Err(D1PairFailure::Invalid("unchanged D1 row transition"));
    }
    let key = key.expect("one side required");
    if change.table == D1Table::EdgeMeta {
        if let D1Cell::Text(name) = &key[0] {
            if name == "data_revision" || name == "knowledge_reader_top" {
                return Err(D1PairFailure::Invalid("reserved D1 publication metadata"));
            }
        }
    }
    Ok(key)
}
fn selected_binding(raw: &str, source_revision: &str) -> D1PairResult<()> {
    if raw.len() > 1_048_576 {
        return Err(D1PairFailure::Budget("prepared binding bytes"));
    }
    let binding: Value =
        serde_json::from_str(raw).map_err(|_| D1PairFailure::Invalid("prepared binding JSON"))?;
    if binding.get("schema").and_then(Value::as_str) != Some("tos_published_knowledge_snapshot_v1")
        || binding.get("read_model_schema").and_then(Value::as_str)
            != Some("tos_local_prepared_read_model_v1")
        || binding.get("source_revision").and_then(Value::as_str) != Some(source_revision)
        || !binding
            .get("data_revision")
            .and_then(Value::as_str)
            .is_some_and(valid_digest)
        || !binding
            .get("metadata_sha256")
            .and_then(Value::as_str)
            .is_some_and(valid_digest)
        || !binding
            .get("publication_epoch")
            .and_then(Value::as_u64)
            .is_some_and(|epoch| epoch <= 9_007_199_254_740_991)
    {
        return Err(D1PairFailure::Invalid("selected prepared binding"));
    }
    Ok(())
}

/// The versioned target identity can be determined before successor reader
/// metadata is framed. It does not establish source or D1 admission.
pub fn target_d1_revision(spec: &D1PairInput) -> D1PairResult<String> {
    if spec
        .auxiliary_stores
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(D1PairFailure::Invalid("D1 auxiliary store selection"));
    }
    for value in [
        &spec.base_d1_revision,
        &spec.before_source_revision,
        &spec.after_source_revision,
        &spec.before_source_inputs_sha256,
        &spec.after_source_inputs_sha256,
        &spec.before_navigation_sha256,
        &spec.after_navigation_sha256,
        &spec.implementation_sha256,
    ] {
        if !valid_digest(value) {
            return Err(D1PairFailure::Invalid("D1 binding digest"));
        }
    }
    if let Some(migration) = spec.migration_implementation_sha256.as_deref() {
        if !valid_digest(migration) {
            return Err(D1PairFailure::Invalid("D1 migration implementation digest"));
        }
    }
    if matches!(
        spec.predecessor_mode,
        D1PredecessorMode::SourceNavigationIntegrity { .. }
    ) != spec.migration_implementation_sha256.is_some()
    {
        return Err(D1PairFailure::Invalid(
            "D1 migration implementation digest presence",
        ));
    }
    let (Some(before_rights), Some(after_rights)) =
        (&spec.before_rights_sha256, &spec.after_rights_sha256)
    else {
        return Err(D1PairFailure::FullOnlyRightsTransition);
    };
    if !valid_digest(before_rights) || !valid_digest(after_rights) {
        return Err(D1PairFailure::FullOnlyRightsTransition);
    }
    match spec.predecessor_mode {
        D1PredecessorMode::PreparedDelta => {
            selected_binding(&spec.before_prepared_binding, &spec.before_source_revision)?;
            selected_binding(&spec.after_prepared_binding, &spec.after_source_revision)?;
        }
        D1PredecessorMode::SourceInputsCatchup => {
            if !spec.before_prepared_binding.is_empty() {
                return Err(D1PairFailure::Invalid(
                    "catch-up has no predecessor prepared binding",
                ));
            }
            selected_binding(&spec.after_prepared_binding, &spec.after_source_revision)?;
        }
        D1PredecessorMode::Bootstrap => {
            selected_binding(&spec.before_prepared_binding, &spec.before_source_revision)?;
            selected_binding(&spec.after_prepared_binding, &spec.after_source_revision)?;
            if spec.before_prepared_binding != spec.after_prepared_binding
                || spec.before_source_revision != spec.after_source_revision
            {
                return Err(D1PairFailure::Invalid(
                    "bootstrap must retain one prepared snapshot",
                ));
            }
        }
        D1PredecessorMode::SourceNavigationIntegrity { .. } => {
            if !spec.before_prepared_binding.is_empty()
                || !spec.after_prepared_binding.is_empty()
                || spec.before_source_revision != spec.after_source_revision
                || spec.before_source_inputs_sha256 != spec.after_source_inputs_sha256
                || spec.before_navigation_sha256 != spec.after_navigation_sha256
                || spec.before_rights_sha256 != spec.after_rights_sha256
            {
                return Err(D1PairFailure::Invalid(
                    "source-navigation integrity retained inputs",
                ));
            }
        }
    }
    if spec.limits.max_transitions == 0
        || spec.limits.max_work_bytes == 0
        || spec.limits.max_sql_bytes == 0
    {
        return Err(D1PairFailure::Budget("D1 pair limits"));
    }
    let lineage = match spec.predecessor_mode {
        D1PredecessorMode::PreparedDelta => json!({
            "schema": D1_PAIR_SCHEMA,
            "base_d1_revision": spec.base_d1_revision,
            "before_source_revision": spec.before_source_revision,
            "after_source_revision": spec.after_source_revision,
            "before_prepared_binding": spec.before_prepared_binding,
            "after_prepared_binding": spec.after_prepared_binding,
            "before_source_inputs_sha256": spec.before_source_inputs_sha256,
            "after_source_inputs_sha256": spec.after_source_inputs_sha256,
            "before_navigation_sha256": spec.before_navigation_sha256,
            "after_navigation_sha256": spec.after_navigation_sha256,
            "before_rights_sha256": before_rights,
            "after_rights_sha256": after_rights,
            "auxiliary_stores": spec.auxiliary_stores.iter().map(|store| store.identity().0).collect::<Vec<_>>(),
            "implementation_sha256": spec.implementation_sha256,
        }),
        D1PredecessorMode::SourceInputsCatchup => json!({
            "schema": "tos_prepared_source_d1_manifest_reconciliation_v1",
            "base_d1_revision": spec.base_d1_revision,
            "before_d1_source_revision": spec.before_source_revision,
            "after_source_revision": spec.after_source_revision,
            "after_prepared_binding": spec.after_prepared_binding,
            "before_source_inputs_sha256": spec.before_source_inputs_sha256,
            "after_source_inputs_sha256": spec.after_source_inputs_sha256,
            "before_navigation_sha256": spec.before_navigation_sha256,
            "after_navigation_sha256": spec.after_navigation_sha256,
            "before_rights_sha256": before_rights,
            "after_rights_sha256": after_rights,
            "auxiliary_stores": spec.auxiliary_stores.iter().map(|store| store.identity().0).collect::<Vec<_>>(),
            "implementation_sha256": spec.implementation_sha256,
        }),
        D1PredecessorMode::Bootstrap => json!({
            "schema": "tos_source_navigation_bootstrap_d1_v1",
            "base_d1_revision": spec.base_d1_revision,
            "source_revision": spec.after_source_revision,
            "prepared_binding": spec.after_prepared_binding,
            "source_inputs_sha256": spec.after_source_inputs_sha256,
            "navigation_sha256": spec.after_navigation_sha256,
            "rights_sha256": after_rights,
            "auxiliary_stores": spec.auxiliary_stores.iter().map(|store| store.identity().0).collect::<Vec<_>>(),
            "implementation_sha256": spec.implementation_sha256,
        }),
        D1PredecessorMode::SourceNavigationIntegrity { header_only } => json!({
            "schema": "tos_native_navigation_integrity_migration_v1",
            "header_only": header_only,
            "base_d1_revision": spec.base_d1_revision,
            "source_revision": spec.after_source_revision,
            "source_inputs_sha256": spec.after_source_inputs_sha256,
            "source_navigation_sha256": spec.after_navigation_sha256,
            "rights_sha256": after_rights,
            "implementation_sha256": spec.implementation_sha256,
            "migration_implementation_sha256": spec.migration_implementation_sha256,
            "auxiliary_stores": spec.auxiliary_stores.iter().map(|store| store.identity().0).collect::<Vec<_>>(),
        }),
    };
    Ok(digest(
        &serde_json::to_vec(&lineage).map_err(|_| D1PairFailure::Invalid("lineage JSON"))?,
    ))
}

fn selection(spec: &D1PairInput) -> D1PairResult<String> {
    let target = target_d1_revision(spec)?;
    if spec.before_reader_top.len() > 32_000 || spec.after_reader_top.len() > 32_000 {
        return Err(D1PairFailure::Budget("D1 reader top framing"));
    }
    let before: Value = serde_json::from_str(&spec.before_reader_top)
        .map_err(|_| D1PairFailure::Invalid("predecessor reader JSON"))?;
    let after: Value = serde_json::from_str(&spec.after_reader_top)
        .map_err(|_| D1PairFailure::Invalid("successor reader JSON"))?;
    if before.get("data_revision").and_then(Value::as_str) != Some(spec.base_d1_revision.as_str())
        || before.get("source_revision").and_then(Value::as_str)
            != Some(spec.before_source_revision.as_str())
        || before.get("read_model_schema").and_then(Value::as_str) != Some(READ_MODEL_SCHEMA)
        || after.get("source_revision").and_then(Value::as_str)
            != Some(spec.after_source_revision.as_str())
        || after.get("read_model_schema").and_then(Value::as_str) != Some(READ_MODEL_SCHEMA)
    {
        return Err(D1PairFailure::Invalid("selected D1 reader tops"));
    }
    if after.get("data_revision").and_then(Value::as_str) != Some(target.as_str()) {
        return Err(D1PairFailure::Invalid("successor D1 reader revision"));
    }
    Ok(target)
}

/// Only files created by this invocation may be removed after refusal.
#[derive(Default)]
struct OwnedOutputs(Vec<(PathBuf, u64, u64)>);
impl OwnedOutputs {
    fn record(&mut self, path: &Path, file: &File) -> D1PairResult<()> {
        let metadata = file.metadata()?;
        self.0
            .push((path.to_owned(), metadata.dev(), metadata.ino()));
        Ok(())
    }
    fn publish(&mut self, pending: &Path, target: &Path) -> D1PairResult<()> {
        let expected = self
            .0
            .iter()
            .find(|(path, _, _)| path == pending)
            .map(|(_, dev, ino)| (*dev, *ino))
            .ok_or(D1PairFailure::Invalid("unowned D1 pending output"))?;
        let metadata = fs::symlink_metadata(pending)?;
        if !metadata.is_file() || (metadata.dev(), metadata.ino()) != expected {
            return Err(D1PairFailure::Invalid("D1 pending output identity changed"));
        }
        // hard_link never replaces a target which appeared after preflight.
        fs::hard_link(pending, target)?;
        // The link can only belong to the pending inode we held. Never adopt
        // a replacement observed through the target pathname as our output.
        self.0.push((target.to_owned(), expected.0, expected.1));
        let published = fs::symlink_metadata(target)?;
        if !published.is_file() || (published.dev(), published.ino()) != expected {
            return Err(D1PairFailure::Invalid(
                "D1 published output identity changed",
            ));
        }
        self.remove_owned(pending)?;
        Ok(())
    }
    fn remove_owned(&self, path: &Path) -> D1PairResult<()> {
        let (_, dev, ino) = self
            .0
            .iter()
            .find(|(owned, _, _)| owned == path)
            .ok_or(D1PairFailure::Invalid("unowned D1 cleanup path"))?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() || (metadata.dev(), metadata.ino()) != (*dev, *ino) {
            return Err(D1PairFailure::Invalid("D1 cleanup path identity changed"));
        }
        fs::remove_file(path)?;
        Ok(())
    }
    fn cleanup(&self) {
        for (path, _, _) in &self.0 {
            let _ = self.remove_owned(path);
        }
    }
}

struct SqlSink {
    writer: BufWriter<File>,
    hash: Digest256Hasher,
    bytes: u64,
    max_bytes: u64,
}
impl SqlSink {
    fn new(path: &Path, max_bytes: u64, owned: &mut OwnedOutputs) -> D1PairResult<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        owned.record(path, &file)?;
        Ok(Self {
            writer: BufWriter::new(file),
            hash: Digest256Hasher::new(),
            bytes: 0,
            max_bytes,
        })
    }
    fn line(&mut self, sql: &str) -> D1PairResult<()> {
        if sql.len() > MAX_STATEMENT_BYTES {
            return Err(D1PairFailure::Budget("D1 SQL statement bytes"));
        }
        self.bytes = self
            .bytes
            .checked_add(sql.len() as u64 + 1)
            .ok_or(D1PairFailure::Budget("D1 SQL bytes"))?;
        if self.bytes > self.max_bytes {
            return Err(D1PairFailure::Budget("D1 SQL bytes"));
        }
        self.writer.write_all(sql.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.hash.update(sql.as_bytes());
        self.hash.update(b"\n");
        Ok(())
    }
    fn finish(mut self) -> D1PairResult<(String, u64)> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok((self.hash.finalize().to_hex(), self.bytes))
    }
}

fn stage_name(revision: &str, table: D1Table, suffix: &str) -> String {
    format!(
        "tos_rust_d1_{}_{}_{}",
        &revision[..12],
        table.shape().0,
        suffix
    )
}
fn stage_schema(sink: &mut SqlSink, revision: &str, table: D1Table) -> D1PairResult<()> {
    let (serving, _, keys) = table.shape();
    for suffix in ["keys", "before", "after"] {
        let stage = stage_name(revision, table, suffix);
        sink.line(&format!("DROP TABLE IF EXISTS {stage};"))?;
        let source = if suffix == "keys" {
            keys.join(",")
        } else {
            "*".into()
        };
        sink.line(&format!(
            "CREATE TABLE {stage} AS SELECT {source} FROM {serving} WHERE 0;"
        ))?;
    }
    sink.line(&format!(
        "CREATE UNIQUE INDEX {} ON {} ({});",
        stage_name(revision, table, "keys_idx"),
        stage_name(revision, table, "keys"),
        keys.join(",")
    ))?;
    Ok(())
}
fn key_selector(table: D1Table, key: &[D1Cell]) -> String {
    let (_, _, keys) = table.shape();
    keys.iter()
        .zip(key)
        .map(|(column, value)| format!("{column} IS {}", cell_sql(value)))
        .collect::<Vec<_>>()
        .join(" AND ")
}
fn insert_row(
    sink: &mut SqlSink,
    revision: &str,
    table: D1Table,
    side: &str,
    row: &[D1Cell],
    key: &[D1Cell],
) -> D1PairResult<()> {
    let (_, columns, keys) = table.shape();
    let stage = stage_name(revision, table, side);
    let mut seeds = Vec::with_capacity(row.len());
    for (column, value) in columns.iter().zip(row) {
        if keys.contains(column) {
            seeds.push(cell_sql(value));
        } else if matches!(value, D1Cell::Text(_)) {
            seeds.push("''".into());
        } else {
            seeds.push(cell_sql(value));
        }
    }
    sink.line(&format!(
        "INSERT INTO {stage} VALUES ({});",
        seeds.join(",")
    ))?;
    let selector = key_selector(table, key);
    for (column, value) in columns.iter().zip(row) {
        if keys.contains(column) {
            continue;
        }
        if let D1Cell::Text(text) = value {
            let mut start = 0;
            while start < text.len() {
                let mut end = (start + TEXT_CHUNK_BYTES).min(text.len());
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                let chunk = &text[start..end];
                sink.line(&format!(
                    "UPDATE {stage} SET {column}={column}||{} WHERE {selector};",
                    quote(chunk)
                ))?;
                start = end;
            }
        }
    }
    Ok(())
}
fn stage_change(
    sink: &mut SqlSink,
    revision: &str,
    change: &D1RowTransition,
    key: &[D1Cell],
    reverse: bool,
) -> D1PairResult<()> {
    let table = change.table;
    let keys = stage_name(revision, table, "keys");
    sink.line(&format!(
        "INSERT INTO {keys} VALUES ({});",
        key.iter().map(cell_sql).collect::<Vec<_>>().join(",")
    ))?;
    let (before, after) = if reverse {
        (&change.after, &change.before)
    } else {
        (&change.before, &change.after)
    };
    if let Some(row) = before {
        insert_row(sink, revision, table, "before", row, key)?;
    }
    if let Some(row) = after {
        insert_row(sink, revision, table, "after", row, key)?;
    }
    Ok(())
}
fn publication(
    sink: &mut SqlSink,
    revision: &str,
    base: &str,
    target: &str,
    counts: &BTreeMap<&'static str, u64>,
    spec: &D1PairInput,
    reverse: bool,
) -> D1PairResult<()> {
    let current = "(SELECT json_extract(group_concat(json_chunk,''),'$.sha256') FROM (SELECT json_chunk FROM edge_meta WHERE key='data_revision' ORDER BY part))";
    sink.line("CREATE TABLE IF NOT EXISTS tos_delta_publications (revision TEXT PRIMARY KEY, base_revision TEXT NOT NULL);")?;
    let trigger = format!("tos_rust_d1_{}_publish", &revision[..12]);
    let retry_trigger = format!("tos_rust_d1_{}_retry", &revision[..12]);
    sink.line(&format!("DROP TRIGGER IF EXISTS {trigger};"))?;
    sink.line(&format!("DROP TRIGGER IF EXISTS {retry_trigger};"))?;
    let mut body = Vec::new();
    body.push(format!(
        "SELECT CASE WHEN {current} IS NOT {} THEN RAISE(ABORT,'stale D1 predecessor') END;",
        quote(base)
    ));
    body.push("SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND typeof(epoch)='integer' AND epoch>=0 AND epoch<=9007199254740989) OR (SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name IN ('knowledge_exploration_revision_insert','knowledge_exploration_revision_update','knowledge_exploration_revision_delete'))!=3 THEN RAISE(ABORT,'D1 publication clock unavailable') END;".into());
    let mut auxiliary_guards = Vec::new();
    let mut auxiliary_seals = Vec::new();
    if !spec.auxiliary_stores.is_empty() {
        let (prior_top, next_top) = if reverse {
            (&spec.after_reader_top, &spec.before_reader_top)
        } else {
            (&spec.before_reader_top, &spec.after_reader_top)
        };
        let prior_top: Value = serde_json::from_str(prior_top)
            .map_err(|_| D1PairFailure::Invalid("auxiliary predecessor top JSON"))?;
        let next_top: Value = serde_json::from_str(next_top)
            .map_err(|_| D1PairFailure::Invalid("auxiliary successor top JSON"))?;
        let clock = "(SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1)";
        let (_, old_prefix, old_suffix) =
            crate::d1_public_metadata::reader_binding_parts(&prior_top)
                .map_err(|_| D1PairFailure::Invalid("auxiliary predecessor binding"))?;
        let old_binding = format!("{}||{clock}||{}", quote(&old_prefix), quote(&old_suffix));
        let old_python = crate::d1_public_metadata::python_reader_binding_parts(&prior_top)
            .ok()
            .filter(|(_, prefix, suffix)| prefix != &old_prefix || suffix != &old_suffix)
            .map(|(_, prefix, suffix)| {
                format!(
                    " OR binding=({}||{clock}||{})",
                    quote(&prefix),
                    quote(&suffix)
                )
            })
            .unwrap_or_default();
        let (_, new_prefix, new_suffix) =
            crate::d1_public_metadata::reader_binding_parts(&next_top)
                .map_err(|_| D1PairFailure::Invalid("auxiliary successor binding"))?;
        for store in &spec.auxiliary_stores {
            let (_, state, _) = store.identity();
            let schema = store.schema();
            auxiliary_guards.push(format!(
                "SELECT CASE WHEN typeof({clock})!='integer' OR {clock}<0 OR {clock}>9007199254740991 OR NOT EXISTS(SELECT 1 FROM {state} WHERE singleton=1 AND schema={} AND (binding=({old_binding}){old_python}) AND valid=1) THEN RAISE(ABORT,'stale lens auxiliary publication') END;",
                quote(schema)
            ));
            auxiliary_seals.push(format!(
                "SELECT CASE WHEN typeof({clock})!='integer' OR {clock}<0 OR {clock}>9007199254740991 THEN RAISE(ABORT,'invalid auxiliary successor epoch') END; UPDATE {state} SET binding=({}||{clock}||{}),valid=1 WHERE singleton=1;",
                quote(&new_prefix), quote(&new_suffix)
            ));
        }
    }
    body.extend(auxiliary_guards);
    for table in D1Table::selected(spec) {
        let (serving, columns, keys) = table.shape();
        let count = counts.get(serving).copied().unwrap_or(0);
        if count == 0 {
            continue;
        }
        let key_stage = stage_name(revision, table, "keys");
        let old_stage = stage_name(revision, table, "before");
        let new_stage = stage_name(revision, table, "after");
        let join = keys
            .iter()
            .map(|key| format!("t.{key} IS k.{key}"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let names = columns.join(",");
        let selected = columns
            .iter()
            .map(|column| format!("t.{column}"))
            .collect::<Vec<_>>()
            .join(",");
        body.push(format!("SELECT CASE WHEN (SELECT count(*) FROM {key_stage})!={count} THEN RAISE(ABORT,'incomplete D1 stage') END;"));
        body.push(format!("SELECT CASE WHEN EXISTS(SELECT {selected} FROM {serving} t WHERE EXISTS(SELECT 1 FROM {key_stage} k WHERE {join}) EXCEPT SELECT {names} FROM {old_stage}) OR EXISTS(SELECT {names} FROM {old_stage} EXCEPT SELECT {selected} FROM {serving} t WHERE EXISTS(SELECT 1 FROM {key_stage} k WHERE {join})) THEN RAISE(ABORT,'D1 predecessor row differs') END;"));
        body.push(format!("DELETE FROM {serving} WHERE rowid IN (SELECT t.rowid FROM {key_stage} k CROSS JOIN {serving} t WHERE {join});"));
        body.push(format!("INSERT INTO {serving} SELECT * FROM {new_stage};"));
    }
    body.push(format!("SELECT CASE WHEN {current} IS NOT {} THEN RAISE(ABORT,'D1 successor revision differs') END;", quote(target)));
    body.extend(auxiliary_seals);
    // Keep each serving transition inside one trigger invocation. Plain INSERT
    // and UPDATE preserve nested edge_meta writes and their clock triggers;
    // OR REPLACE can silently stop this body before its revision seal.
    // SQLite limits one statement to 100 KiB. A bounded trigger body may still
    // exceed that at many table families; refuse instead of weakening guards.
    sink.line(&format!("CREATE TRIGGER {trigger} AFTER INSERT ON tos_delta_publications WHEN NEW.revision={} BEGIN {} END;",
        quote(target), body.join(" ")))?;
    sink.line(&format!("CREATE TRIGGER {retry_trigger} AFTER UPDATE OF base_revision ON tos_delta_publications WHEN NEW.revision={} BEGIN {} END;",
        quote(target), body.join(" ")))?;
    sink.line(&format!(
        "UPDATE tos_delta_publications SET base_revision={} WHERE revision={} AND {current} IS NOT {};",
        quote(base), quote(target), quote(target)
    ))?;
    sink.line(&format!(
        "INSERT INTO tos_delta_publications SELECT {},{} WHERE {current} IS NOT {} AND NOT EXISTS (SELECT 1 FROM tos_delta_publications WHERE revision={});",
        quote(target),
        quote(base),
        quote(target),
        quote(target)
    ))?;
    sink.line(&format!("DROP TRIGGER {trigger};"))?;
    sink.line(&format!("DROP TRIGGER {retry_trigger};"))?;
    for table in D1Table::selected(spec) {
        for suffix in ["keys", "before", "after"] {
            sink.line(&format!(
                "DROP TABLE {};",
                stage_name(revision, table, suffix)
            ))?;
        }
    }
    Ok(())
}

fn stage_meta(
    sink: &mut SqlSink,
    revision: &str,
    before_revision: &str,
    after_revision: &str,
    before_top: &str,
    after_top: &str,
    reverse: bool,
) -> D1PairResult<()> {
    let (old_revision, new_revision, old_top, new_top) = if reverse {
        (after_revision, before_revision, after_top, before_top)
    } else {
        (before_revision, after_revision, before_top, after_top)
    };
    for (key, old, new) in [
        (
            "data_revision",
            json!({"sha256":old_revision}).to_string(),
            json!({"sha256":new_revision}).to_string(),
        ),
        (
            "knowledge_reader_top",
            old_top.to_owned(),
            new_top.to_owned(),
        ),
    ] {
        let change = D1RowTransition {
            table: D1Table::EdgeMeta,
            before: Some(vec![
                D1Cell::Text(key.into()),
                D1Cell::Integer(0),
                D1Cell::Text(old),
            ]),
            after: Some(vec![
                D1Cell::Text(key.into()),
                D1Cell::Integer(0),
                D1Cell::Text(new),
            ]),
        };
        let key = row_key(
            D1Table::EdgeMeta,
            change.before.as_ref().expect("old metadata"),
        )?;
        stage_change(sink, revision, &change, &key, false)?;
    }
    Ok(())
}

fn combined_sql(fw: &SqlSink, rv: &SqlSink, limit: u64) -> D1PairResult<()> {
    if fw
        .bytes
        .checked_add(rv.bytes)
        .is_none_or(|bytes| bytes > limit)
    {
        return Err(D1PairFailure::Budget("combined D1 SQL bytes"));
    }
    Ok(())
}

/// Emit an immutable private pair. `rows` is consumed once; no complete change
/// list is materialized. A manifest selected last binds both SQL files. The
/// caller must independently establish source/rights/currentness and import.
pub fn emit_d1_pair<I>(
    spec: &D1PairInput,
    rows: I,
    forward: &Path,
    rollback: &Path,
    manifest: &Path,
) -> D1PairResult<D1PairReceipt>
where
    I: IntoIterator<Item = D1RowTransition>,
{
    emit_d1_pair_inner(spec, rows, forward, rollback, manifest)
}

fn emit_d1_pair_inner<I>(
    spec: &D1PairInput,
    rows: I,
    forward: &Path,
    rollback: &Path,
    manifest: &Path,
) -> D1PairResult<D1PairReceipt>
where
    I: IntoIterator<Item = D1RowTransition>,
{
    let target = selection(spec)?;
    let paths = [forward, rollback, manifest];
    let pending = paths.map(|path| {
        path.with_extension(format!(
            "{}next",
            path.extension().and_then(|e| e.to_str()).unwrap_or("")
        ))
    });
    if paths
        .iter()
        .copied()
        .chain(pending.iter().map(PathBuf::as_path))
        .any(|path| !path.is_absolute() || path.exists())
        || paths
            .iter()
            .copied()
            .chain(pending.iter().map(PathBuf::as_path))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 6
    {
        return Err(D1PairFailure::Invalid(
            "distinct fresh absolute D1 pair paths",
        ));
    }
    if paths.iter().any(|path| path.parent() != forward.parent()) {
        return Err(D1PairFailure::Invalid(
            "D1 pair requires one artifact directory",
        ));
    }
    if paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| name.is_empty() || name.len() > 255)
    }) {
        return Err(D1PairFailure::Invalid("D1 pair filename"));
    }
    fs::create_dir_all(
        forward
            .parent()
            .ok_or(D1PairFailure::Invalid("D1 pair parent"))?,
    )?;
    let mut owned = OwnedOutputs::default();
    let selected_tables = D1Table::selected(spec);
    let result = (|| -> D1PairResult<D1PairReceipt> {
        let mut fw = SqlSink::new(&pending[0], spec.limits.max_sql_bytes, &mut owned)?;
        let mut rv = SqlSink::new(&pending[1], spec.limits.max_sql_bytes, &mut owned)?;
        let mut counts = BTreeMap::new();
        for table in D1Table::selected(spec) {
            stage_schema(&mut fw, &target, table)?;
            stage_schema(&mut rv, &spec.base_d1_revision, table)?;
        }
        combined_sql(&fw, &rv, spec.limits.max_sql_bytes)?;
        let mut work = 0u64;
        let mut changed = 0u64;
        for row in rows {
            if !selected_tables.contains(&row.table) {
                return Err(D1PairFailure::Invalid("unselected D1 auxiliary transition"));
            }
            let key = validate_row(&row, spec.limits.max_row_bytes)?;
            changed += 1;
            if changed > spec.limits.max_transitions {
                return Err(D1PairFailure::Budget("D1 transition count"));
            }
            let mut size = 0usize;
            for cells in row.before.iter().chain(row.after.iter()) {
                size = size
                    .checked_add(row_bytes(cells, spec.limits.max_row_bytes)?)
                    .ok_or(D1PairFailure::Budget("D1 work bytes"))?;
            }
            work = work
                .checked_add(size as u64)
                .ok_or(D1PairFailure::Budget("D1 work bytes"))?;
            if work > spec.limits.max_work_bytes {
                return Err(D1PairFailure::Budget("D1 work bytes"));
            }
            *counts.entry(row.table.shape().0).or_insert(0u64) += 1;
            stage_change(&mut fw, &target, &row, &key, false)?;
            stage_change(&mut rv, &spec.base_d1_revision, &row, &key, true)?;
            combined_sql(&fw, &rv, spec.limits.max_sql_bytes)?;
        }
        if changed == 0 {
            return Err(D1PairFailure::Invalid("empty D1 transition"));
        }
        *counts.entry("edge_meta").or_insert(0) += 2;
        stage_meta(
            &mut fw,
            &target,
            &spec.base_d1_revision,
            &target,
            &spec.before_reader_top,
            &spec.after_reader_top,
            false,
        )?;
        stage_meta(
            &mut rv,
            &spec.base_d1_revision,
            &spec.base_d1_revision,
            &target,
            &spec.before_reader_top,
            &spec.after_reader_top,
            true,
        )?;
        combined_sql(&fw, &rv, spec.limits.max_sql_bytes)?;
        publication(
            &mut fw,
            &target,
            &spec.base_d1_revision,
            &target,
            &counts,
            spec,
            false,
        )?;
        publication(
            &mut rv,
            &spec.base_d1_revision,
            &target,
            &spec.base_d1_revision,
            &counts,
            spec,
            true,
        )?;
        combined_sql(&fw, &rv, spec.limits.max_sql_bytes)?;
        let (forward_hash, forward_bytes) = fw.finish()?;
        let (rollback_hash, rollback_bytes) = rv.finish()?;
        if forward_bytes
            .checked_add(rollback_bytes)
            .is_none_or(|bytes| bytes > spec.limits.max_sql_bytes)
        {
            return Err(D1PairFailure::Budget("combined D1 SQL bytes"));
        }
        let packet = json!({
            "schema": D1_PAIR_SCHEMA,
            "base_d1_revision": spec.base_d1_revision,
            "target_d1_revision": target,
            "forward": {"name": forward.file_name().unwrap().to_string_lossy(), "sha256": forward_hash, "bytes": forward_bytes},
            "rollback": {"name": rollback.file_name().unwrap().to_string_lossy(), "sha256": rollback_hash, "bytes": rollback_bytes},
            "changed_rows": changed,
            "before_rights_sha256": spec.before_rights_sha256,
            "after_rights_sha256": spec.after_rights_sha256,
            "auxiliary_stores": spec.auxiliary_stores.iter().map(|store| store.identity().0).collect::<Vec<_>>(),
            "publication": "private-pair-manifest-last",
            "selected_pair_owner_admitted": false,
            "source_currentness_verified": false,
            "semantic_acceptance": false,
            "d1_applied": false,
            "consumer_switched": false,
        });
        let raw = serde_json::to_vec(&packet)
            .map_err(|_| D1PairFailure::Invalid("pair manifest JSON"))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending[2])?;
        owned.record(&pending[2], &file)?;
        file.write_all(&raw)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        owned.publish(&pending[0], forward)?;
        owned.publish(&pending[1], rollback)?;
        File::open(forward.parent().expect("pair directory"))?.sync_all()?;
        owned.publish(&pending[2], manifest)?;
        File::open(forward.parent().expect("pair directory"))?.sync_all()?;
        Ok(D1PairReceipt {
            schema: D1_PAIR_SCHEMA,
            base_d1_revision: spec.base_d1_revision.clone(),
            target_d1_revision: target,
            forward_sha256: forward_hash,
            rollback_sha256: rollback_hash,
            forward_bytes,
            rollback_bytes,
            changed_rows: changed,
            pair_manifest: manifest.into(),
            owner_admission_sha256: None,
            selected_pair_owner_admitted: false,
            d1_applied: false,
            consumer_switched: false,
        })
    })();
    if result.is_err() {
        owned.cleanup();
    }
    result
}

/// Emit a bounded private capture with optional rollback and independently
/// owned SQL/manifest destinations. The manifest is always published last;
/// the caller must keep each destination directory exclusive for the call.
pub fn emit_d1_capture<I>(
    spec: &D1PairInput,
    rows: I,
    forward: &Path,
    rollback: Option<&Path>,
    manifest: &Path,
) -> D1PairResult<D1CaptureReceipt>
where
    I: IntoIterator<Item = D1RowTransition>,
{
    let target = selection(spec)?;
    let mut destinations = vec![forward];
    if let Some(rollback) = rollback {
        destinations.push(rollback);
    }
    destinations.push(manifest);
    let pending = destinations
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(D1PairFailure::Invalid("D1 capture filename"))?;
            Ok(path.with_file_name(format!("{name}.next")))
        })
        .collect::<D1PairResult<Vec<_>>>()?;
    let all_paths = destinations
        .iter()
        .copied()
        .chain(pending.iter().map(PathBuf::as_path))
        .collect::<std::collections::BTreeSet<_>>();
    if all_paths.len() != destinations.len() + pending.len()
        || destinations
            .iter()
            .copied()
            .chain(pending.iter().map(PathBuf::as_path))
            .any(|path| {
                !path.is_absolute()
                    || path.exists()
                    || fs::symlink_metadata(path).is_ok()
                    || path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_none_or(|name| name.is_empty() || name.len() > 255)
            })
    {
        return Err(D1PairFailure::Invalid(
            "distinct fresh absolute D1 capture paths",
        ));
    }
    let mut parent_dirs = destinations
        .iter()
        .map(|path| {
            path.parent()
                .ok_or(D1PairFailure::Invalid("D1 capture parent"))
        })
        .collect::<D1PairResult<Vec<_>>>()?;
    parent_dirs.sort();
    parent_dirs.dedup();
    for parent in &parent_dirs {
        fs::create_dir_all(parent)?;
    }

    let forward_pending = &pending[0];
    let manifest_pending = pending.last().expect("manifest pending path");
    let rollback_pending = rollback.map(|_| &pending[1]);
    let mut owned = OwnedOutputs::default();
    let selected_tables = D1Table::selected(spec);
    let result = (|| -> D1PairResult<D1CaptureReceipt> {
        let mut fw = SqlSink::new(forward_pending, spec.limits.max_sql_bytes, &mut owned)?;
        let mut rv = rollback_pending
            .map(|path| SqlSink::new(path, spec.limits.max_sql_bytes, &mut owned))
            .transpose()?;
        let mut counts = BTreeMap::new();
        for table in D1Table::selected(spec) {
            stage_schema(&mut fw, &target, table)?;
            if let Some(rv) = rv.as_mut() {
                stage_schema(rv, &spec.base_d1_revision, table)?;
            }
        }
        if let Some(rv) = rv.as_ref() {
            combined_sql(&fw, rv, spec.limits.max_sql_bytes)?;
        }
        let mut work = 0u64;
        let mut changed = 0u64;
        for row in rows {
            if !selected_tables.contains(&row.table) {
                return Err(D1PairFailure::Invalid("unselected D1 auxiliary transition"));
            }
            let key = validate_row(&row, spec.limits.max_row_bytes)?;
            changed = changed
                .checked_add(1)
                .ok_or(D1PairFailure::Budget("D1 transition count"))?;
            if changed > spec.limits.max_transitions {
                return Err(D1PairFailure::Budget("D1 transition count"));
            }
            let mut size = 0usize;
            for cells in row.before.iter().chain(row.after.iter()) {
                size = size
                    .checked_add(row_bytes(cells, spec.limits.max_row_bytes)?)
                    .ok_or(D1PairFailure::Budget("D1 work bytes"))?;
            }
            work = work
                .checked_add(size as u64)
                .ok_or(D1PairFailure::Budget("D1 work bytes"))?;
            if work > spec.limits.max_work_bytes {
                return Err(D1PairFailure::Budget("D1 work bytes"));
            }
            *counts.entry(row.table.shape().0).or_insert(0u64) += 1;
            stage_change(&mut fw, &target, &row, &key, false)?;
            if let Some(rv) = rv.as_mut() {
                stage_change(rv, &spec.base_d1_revision, &row, &key, true)?;
                combined_sql(&fw, rv, spec.limits.max_sql_bytes)?;
            }
        }
        if changed == 0 {
            return Err(D1PairFailure::Invalid("empty D1 transition"));
        }
        *counts.entry("edge_meta").or_insert(0) += 2;
        stage_meta(
            &mut fw,
            &target,
            &spec.base_d1_revision,
            &target,
            &spec.before_reader_top,
            &spec.after_reader_top,
            false,
        )?;
        if let Some(rv) = rv.as_mut() {
            stage_meta(
                rv,
                &spec.base_d1_revision,
                &spec.base_d1_revision,
                &target,
                &spec.before_reader_top,
                &spec.after_reader_top,
                true,
            )?;
            combined_sql(&fw, rv, spec.limits.max_sql_bytes)?;
        }
        publication(
            &mut fw,
            &target,
            &spec.base_d1_revision,
            &target,
            &counts,
            spec,
            false,
        )?;
        if let Some(rv) = rv.as_mut() {
            publication(
                rv,
                &spec.base_d1_revision,
                &target,
                &spec.base_d1_revision,
                &counts,
                spec,
                true,
            )?;
            combined_sql(&fw, rv, spec.limits.max_sql_bytes)?;
        }
        let (forward_sha256, forward_bytes) = fw.finish()?;
        let (rollback_sha256, rollback_bytes) = match rv {
            Some(rv) => {
                let (digest, bytes) = rv.finish()?;
                (Some(digest), bytes)
            }
            None => (None, 0),
        };
        if rollback_sha256.is_some()
            && forward_bytes
                .checked_add(rollback_bytes)
                .is_none_or(|bytes| bytes > spec.limits.max_sql_bytes)
        {
            return Err(D1PairFailure::Budget("combined D1 SQL bytes"));
        }
        let packet = json!({
            "schema": "tos_rust_private_d1_capture_manifest_v1",
            "base_d1_revision": spec.base_d1_revision,
            "target_d1_revision": target,
            "forward": {"name": forward.file_name().unwrap().to_string_lossy(), "sha256": forward_sha256, "bytes": forward_bytes},
            "rollback": rollback.zip(rollback_sha256.as_ref()).map(|(path, hash)| json!({"name": path.file_name().unwrap().to_string_lossy(), "sha256": hash, "bytes": rollback_bytes})),
            "changed_rows": changed,
            "before_rights_sha256": spec.before_rights_sha256,
            "after_rights_sha256": spec.after_rights_sha256,
            "auxiliary_stores": spec.auxiliary_stores.iter().map(|store| store.identity().0).collect::<Vec<_>>(),
            "publication": "private-capture-manifest-last",
            "selected_pair_owner_admitted": false,
            "source_currentness_verified": false,
            "semantic_acceptance": false,
            "d1_applied": false,
            "consumer_switched": false,
        });
        let raw = serde_json::to_vec(&packet)
            .map_err(|_| D1PairFailure::Invalid("capture manifest JSON"))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(manifest_pending)?;
        owned.record(manifest_pending, &file)?;
        file.write_all(&raw)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        if let Some(rollback) = rollback {
            owned.publish(rollback_pending.expect("rollback pending path"), rollback)?;
        }
        owned.publish(forward_pending, forward)?;
        for parent in destinations
            .iter()
            .filter_map(|path| path.parent())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|parent| *parent != manifest.parent().expect("manifest parent"))
        {
            File::open(parent)?.sync_all()?;
        }
        owned.publish(manifest_pending, manifest)?;
        File::open(manifest.parent().expect("manifest parent"))?.sync_all()?;
        Ok(D1CaptureReceipt {
            schema: "tos_rust_private_d1_capture_manifest_v1",
            base_d1_revision: spec.base_d1_revision.clone(),
            target_d1_revision: target,
            forward_sha256,
            rollback_sha256,
            forward_bytes,
            rollback_bytes,
            changed_rows: changed,
            manifest_path: manifest.into(),
            selected_pair_owner_admitted: false,
            d1_applied: false,
            consumer_switched: false,
        })
    })();
    if result.is_err() {
        owned.cleanup();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> String {
        digest(value.as_bytes())
    }
    fn selected() -> D1PairInput {
        let base = hash("base");
        let source = hash("source");
        let before_binding = json!({"schema":"tos_published_knowledge_snapshot_v1",
            "read_model_schema":"tos_local_prepared_read_model_v1",
            "source_revision":source,"data_revision":hash("prepared-before"),
            "metadata_sha256":hash("before-top"),"publication_epoch":1})
        .to_string();
        let after_binding = json!({"schema":"tos_published_knowledge_snapshot_v1",
            "read_model_schema":"tos_local_prepared_read_model_v1",
            "source_revision":source,"data_revision":hash("prepared-after"),
            "metadata_sha256":hash("after-top"),"publication_epoch":2})
        .to_string();
        let mut spec = D1PairInput {
            predecessor_mode: D1PredecessorMode::PreparedDelta,
            base_d1_revision: base.clone(), before_source_revision: source.clone(), after_source_revision: source,
            before_prepared_binding: before_binding, after_prepared_binding: after_binding,
            before_source_inputs_sha256: hash("before-inputs"), after_source_inputs_sha256: hash("after-inputs"),
            before_navigation_sha256: hash("before-nav"), after_navigation_sha256: hash("after-nav"),
            before_rights_sha256: Some(hash("rights")), after_rights_sha256: Some(hash("rights")),
            implementation_sha256: hash("implementation"),
            migration_implementation_sha256: None,
            auxiliary_stores: Vec::new(),
            before_reader_top: json!({"data_revision":base,"source_revision":hash("source"),"read_model_schema":READ_MODEL_SCHEMA}).to_string(),
            after_reader_top: json!({"data_revision":"","source_revision":hash("source"),"read_model_schema":READ_MODEL_SCHEMA}).to_string(),
            limits: D1PairLimits::default(),
        };
        let mut after: Value = serde_json::from_str(&spec.after_reader_top).unwrap();
        after["data_revision"] = target_d1_revision(&spec).unwrap().into();
        spec.after_reader_top = after.to_string();
        spec
    }
    #[test]
    fn rights_changes_are_mechanical_and_missing_roots_stay_full_only() {
        let mut spec = selected();
        spec.after_navigation_sha256 = spec.before_navigation_sha256.clone();
        spec.after_rights_sha256 = Some(hash("changed"));
        let mut after: Value = serde_json::from_str(&spec.after_reader_top).unwrap();
        after["data_revision"] = target_d1_revision(&spec).unwrap().into();
        spec.after_reader_top = after.to_string();
        assert!(matches!(selection(&spec), Ok(_)));
        spec.after_rights_sha256 = None;
        assert!(matches!(
            selection(&spec),
            Err(D1PairFailure::FullOnlyRightsTransition)
        ));
    }
    #[test]
    fn installed_auxiliary_is_part_of_the_pair_identity() {
        let mut spec = selected();
        let original = target_d1_revision(&spec).unwrap();
        spec.auxiliary_stores = vec![D1AuxiliaryStore::CompactLens];
        assert_ne!(target_d1_revision(&spec).unwrap(), original);
        spec.auxiliary_stores = vec![
            D1AuxiliaryStore::LensMemberships,
            D1AuxiliaryStore::CompactLens,
        ];
        assert!(matches!(
            target_d1_revision(&spec),
            Err(D1PairFailure::Invalid("D1 auxiliary store selection"))
        ));
    }
    #[test]
    fn bounded_pair_emits_manifest_last_and_rejects_bad_rows() {
        let root = std::env::temp_dir().join(format!(
            "tos-d1-test-{}-{}",
            std::process::id(),
            hash("pair")
        ));
        fs::create_dir_all(&root).unwrap();
        let paths = [
            root.join("forward.sql"),
            root.join("rollback.sql"),
            root.join("pair.json"),
        ];
        let _ = paths.iter().for_each(|path| {
            let _ = fs::remove_file(path);
        });
        let row = D1RowTransition {
            table: D1Table::KnowledgeNodes,
            before: Some(vec![
                D1Cell::Text("a".into()),
                D1Cell::Text("e".into()),
                D1Cell::Text("a".into()),
                D1Cell::Text("source".into()),
                D1Cell::Text("kind".into()),
                D1Cell::Text("type".into()),
                D1Cell::Text("old".into()),
                D1Cell::Text("".into()),
                D1Cell::Text("old".into()),
                D1Cell::Text("{\"id\":\"a\"}".into()),
            ]),
            after: Some(vec![
                D1Cell::Text("a".into()),
                D1Cell::Text("e".into()),
                D1Cell::Text("a".into()),
                D1Cell::Text("source".into()),
                D1Cell::Text("kind".into()),
                D1Cell::Text("type".into()),
                D1Cell::Text("new".into()),
                D1Cell::Text("".into()),
                D1Cell::Text("new".into()),
                D1Cell::Text("{\"id\":\"a\"}".into()),
            ]),
        };
        let selected = selected();
        let receipt =
            emit_d1_pair(&selected, [row.clone()], &paths[0], &paths[1], &paths[2]).unwrap();
        assert_eq!(receipt.changed_rows, 1);
        assert!(paths.iter().all(|path| path.is_file()));
        let forward = fs::read_to_string(&paths[0]).unwrap();
        assert!(forward.contains("D1 predecessor row differs"));
        assert!(forward.contains("stale D1 predecessor"));
        assert!(forward.contains("knowledge_nodes"));
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for table in D1Table::selected(&selected) {
            let (name, columns, _) = table.shape();
            let definition = columns
                .iter()
                .map(|column| {
                    let ty = if ["position", "part", "ord", "n", "document_chars", "postings"]
                        .contains(column)
                    {
                        "INTEGER"
                    } else {
                        "TEXT"
                    };
                    format!("{column} {ty}")
                })
                .collect::<Vec<_>>()
                .join(",");
            db.execute_batch(&format!("CREATE TABLE {name} ({definition});"))
                .unwrap();
        }
        db.execute_batch("CREATE TABLE knowledge_exploration_clock (singleton INTEGER PRIMARY KEY,epoch INTEGER NOT NULL);
            INSERT INTO knowledge_exploration_clock VALUES(1,0);
            CREATE TRIGGER knowledge_exploration_revision_insert AFTER INSERT ON edge_meta WHEN NEW.key='data_revision' BEGIN UPDATE knowledge_exploration_clock SET epoch=epoch+1 WHERE singleton=1; END;
            CREATE TRIGGER knowledge_exploration_revision_update AFTER UPDATE ON edge_meta WHEN NEW.key='data_revision' OR OLD.key='data_revision' BEGIN UPDATE knowledge_exploration_clock SET epoch=epoch+1 WHERE singleton=1; END;
            CREATE TRIGGER knowledge_exploration_revision_delete AFTER DELETE ON edge_meta WHEN OLD.key='data_revision' BEGIN UPDATE knowledge_exploration_clock SET epoch=epoch+1 WHERE singleton=1; END;").unwrap();
        db.execute(
            "INSERT INTO edge_meta VALUES (?1,0,?2)",
            rusqlite::params![
                "data_revision",
                json!({"sha256":selected.base_d1_revision}).to_string()
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO edge_meta VALUES (?1,0,?2)",
            rusqlite::params!["knowledge_reader_top", selected.before_reader_top],
        )
        .unwrap();
        let old = row
            .before
            .as_ref()
            .unwrap()
            .iter()
            .map(cell_sql)
            .collect::<Vec<_>>()
            .join(",");
        db.execute_batch(&format!("INSERT INTO knowledge_nodes VALUES ({old});"))
            .unwrap();
        let base_epoch: i64 = db
            .query_row(
                "SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        db.execute_batch(&forward).unwrap();
        let first_epoch: i64 = db
            .query_row(
                "SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(first_epoch > base_epoch);
        assert_eq!(
            db.query_row(
                "SELECT json_extract(json_chunk,'$.sha256') FROM edge_meta WHERE key='data_revision'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            receipt.target_d1_revision
        );
        assert_eq!(
            db.query_row(
                "SELECT title_text FROM knowledge_nodes WHERE id='a'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "new"
        );
        db.execute_batch(&forward).unwrap(); // Serving target makes replay inert.
        db.execute_batch(&fs::read_to_string(&paths[1]).unwrap())
            .unwrap();
        let reverse_epoch: i64 = db
            .query_row(
                "SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(reverse_epoch > first_epoch);
        assert_eq!(
            db.query_row(
                "SELECT json_extract(json_chunk,'$.sha256') FROM edge_meta WHERE key='data_revision'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            selected.base_d1_revision
        );
        assert_eq!(
            db.query_row(
                "SELECT title_text FROM knowledge_nodes WHERE id='a'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "old"
        );
        db.execute_batch(&forward).unwrap(); // Historical target marker uses UPDATE.
        db.execute_batch(&fs::read_to_string(&paths[1]).unwrap())
            .unwrap(); // Historical base marker also uses UPDATE.
        db.execute(
            "UPDATE knowledge_nodes SET title_text='tampered' WHERE id='a'",
            [],
        )
        .unwrap();
        assert!(db.execute_batch(&forward).is_err());
        assert_eq!(
            db.query_row(
                "SELECT title_text FROM knowledge_nodes WHERE id='a'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "tampered"
        );
        for path in paths {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn fresh_output_races_preserve_files_not_created_by_capture() {
        for collision in ["pending-manifest", "final-forward"] {
            let spec = selected();
            let directory = tempfile::tempdir().unwrap();
            let forward = directory.path().join("forward.sql");
            let rollback = directory.path().join("rollback.sql");
            let manifest = directory.path().join("pair.json");
            let foreign = if collision == "pending-manifest" {
                manifest.with_extension("jsonnext")
            } else {
                forward.clone()
            };
            let rows = std::iter::once_with(|| {
                fs::write(&foreign, b"independent writer").unwrap();
                D1RowTransition {
                    table: D1Table::EdgeMeta,
                    before: None,
                    after: Some(vec![
                        D1Cell::Text("fixture-race".into()),
                        D1Cell::Integer(0),
                        D1Cell::Text("{}".into()),
                    ]),
                }
            });
            assert!(emit_d1_pair(&spec, rows, &forward, &rollback, &manifest).is_err());
            assert_eq!(fs::read(&foreign).unwrap(), b"independent writer");
            assert!(!rollback.exists() && !manifest.exists());
            assert!(!forward.with_extension("sqlnext").exists());
            assert!(!rollback.with_extension("sqlnext").exists());
        }
    }
    #[test]
    fn cleanup_preserves_a_replaced_published_path() {
        let directory = tempfile::tempdir().unwrap();
        let pending = directory.path().join("pending.sql");
        let target = directory.path().join("published.sql");
        let held = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .unwrap();
        let mut owned = OwnedOutputs::default();
        owned.record(&pending, &held).unwrap();
        owned.publish(&pending, &target).unwrap();
        fs::remove_file(&target).unwrap();
        fs::write(&target, b"foreign successor").unwrap();
        assert!(owned.remove_owned(&target).is_err());
        owned.cleanup();
        assert_eq!(fs::read(&target).unwrap(), b"foreign successor");
    }

    #[test]
    fn unselected_auxiliary_rows_refuse_before_pair_publication() {
        let spec = selected();
        let directory = tempfile::tempdir().unwrap();
        let forward = directory.path().join("forward.sql");
        let rollback = directory.path().join("rollback.sql");
        let manifest = directory.path().join("pair.json");
        let rows = [D1RowTransition {
            table: D1Table::KnowledgeCompactLens,
            before: None,
            after: Some(vec![
                D1Cell::Text("node".into()),
                D1Cell::Text("x".into()),
                D1Cell::Text(hash("source")),
                D1Cell::Text(hash("seed")),
                D1Cell::Text("{}".into()),
            ]),
        }];
        assert!(matches!(
            emit_d1_pair(&spec, rows, &forward, &rollback, &manifest),
            Err(D1PairFailure::Invalid("unselected D1 auxiliary transition"))
        ));
        assert!(!forward.exists() && !rollback.exists() && !manifest.exists());
        assert!(!forward.with_extension("sqlnext").exists());
        assert!(!rollback.with_extension("sqlnext").exists());
    }
}
