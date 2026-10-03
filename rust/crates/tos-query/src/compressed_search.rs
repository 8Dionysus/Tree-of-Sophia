//! Full-carrier compressed search on an explicitly selected local prepared v1
//! publication. Four-phase progress and joins share one retained transaction;
//! source/rights and external pathname/current publication checks stay caller-owned.
use crate::compressed_search_sqlite::{Read, check_header, query_kind};
use crate::compressed_search_state::*;
pub use crate::compressed_search_state::{
    CompressedSearchError, CompressedSearchErrorCode, CompressedSearchRequest,
    PublishedSearchLimits, SCHEMA,
};
use rusqlite::Connection;
use std::sync::Arc;
use tos_compiler::local_prepared::{
    PreparedReadError, PreparedReadErrorCode, PreparedReadLimits, PreparedReadTransaction,
};
use tos_foundation::{Digest256, JsonMode, JsonValue, parse_json};

const OUTPUT_RESERVE: usize = 524_288;
const FINAL_READ_BYTES: usize = 66_560;
const FINAL_READ_ROWS: usize = 513;
const RANKS: &[&str] = &[
    "exact-identity",
    "identity-prefix",
    "visible-text",
    "serialized-text",
];
const NODE_COLUMNS: &[&str] = &[
    "id",
    "entity_id",
    "native_id",
    "source_graph",
    "kind_id",
    "type_id",
];
const RELATION_COLUMNS: &[&str] = &[
    "id",
    "native_id",
    "source_graph",
    "from_id",
    "to_id",
    "predicate_id",
    "relation_type_id",
];
const OBJECTS: &[(&str, &str)] = &[
    ("prepared_documents", "table"),
    ("search_header", "table"),
    ("search_documents", "table"),
    ("search_values", "table"),
    ("search_text_chunks", "table"),
    ("search_terms", "table"),
    ("search_blocks", "table"),
    ("search_document_terms", "table"),
    ("search_blocks_nonempty", "index"),
    ("sqlite_autoindex_prepared_documents_1", "index"),
    ("sqlite_autoindex_prepared_documents_2", "index"),
];

fn prepared_error(e: PreparedReadError) -> CompressedSearchError {
    err(
        match e.code {
            PreparedReadErrorCode::StaleBinding => CompressedSearchErrorCode::StaleBinding,
            PreparedReadErrorCode::BudgetExceeded => CompressedSearchErrorCode::BudgetExceeded,
            PreparedReadErrorCode::Unavailable => CompressedSearchErrorCode::Unavailable,
        },
        e.detail,
    )
}
fn body_budget(read_limits: PreparedReadLimits, limits: PublishedSearchLimits) -> Result<usize> {
    limits.validate()?;
    let available = limits.body_bytes.min(
        read_limits
            .max_response_bytes
            .saturating_sub(OUTPUT_RESERVE),
    );
    if available < read_limits.max_row_bytes.saturating_mul(2) {
        return Err(invalid(
            "search body budget must admit one maximal carrier per kind",
        ));
    }
    Ok(available / 2)
}
fn key(read: &mut Read<'_>, view: &PreparedReadTransaction<'_>) -> Result<(Vec<u8>, usize)> {
    if view
        .binding()
        .object_get("read_model_schema")
        .and_then(JsonValue::as_str)
        != Some("tos_local_prepared_read_model_v1")
        || view.top().object_get("schema").and_then(JsonValue::as_str)
            != Some("tos_published_knowledge_reader_v2")
    {
        return Err(unavailable(
            "published search requires a local prepared reader header",
        ));
    }
    let names = compact(
        &JsonValue::Array(OBJECTS.iter().map(|(name, _)| string(name)).collect()),
        8192,
    )?;
    let names =
        std::str::from_utf8(&names).map_err(|_| unavailable("invalid search object names"))?;
    let objects = read.query(
        "SELECT name,type FROM sqlite_master WHERE name IN (SELECT value FROM json_each(?1))",
        &[&names],
        true,
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )?;
    if objects.len() != OBJECTS.len()
        || OBJECTS
            .iter()
            .any(|(name, kind)| !objects.iter().any(|(n, k)| n == name && k == kind))
    {
        return Err(unavailable(
            "prepared search tables or address indexes are unavailable",
        ));
    }
    let mut work = crate::compressed_search_sqlite::Work::default();
    let (_, key) = check_header(read, view.binding(), &mut work)?;
    read.charge_bytes(work.metadata_bytes)?;
    Ok((key, work.metadata_bytes))
}

fn capability_checked(
    read: &mut Read<'_>,
    view: &PreparedReadTransaction<'_>,
    limits: PublishedSearchLimits,
    body_per_kind: usize,
) -> Result<JsonValue> {
    view.check_current().map_err(prepared_error)?;
    read.absorb_owner(view)?;
    key(read, view)?;
    view.check_current().map_err(prepared_error)?;
    read.absorb_owner(view)?;
    Ok(object(vec![
        ("available", JsonValue::Bool(true)),
        ("schema", string(SCHEMA)),
        (
            "read_model_schema",
            string("tos_local_prepared_read_model_v1"),
        ),
        ("ordering_scope", string("global-rank")),
        ("cursor", string("authenticated-stateless-15-minute")),
        ("counts", string("exact-only-initial-complete-kind")),
        ("writes_to_tree", JsonValue::Bool(false)),
        (
            "limits",
            object(vec![
                ("candidate_budget", number(limits.candidate_budget as u64)),
                (
                    "verification_bytes",
                    number(limits.verification_bytes as u64),
                ),
                ("metadata_bytes", number(limits.metadata_bytes as u64)),
                ("body_bytes", number(limits.body_bytes as u64)),
                ("body_bytes_per_kind", number(body_per_kind as u64)),
                ("cursor_bytes", number(MAX_CURSOR_BYTES as u64)),
                ("inner_pages_per_kind", number(1)),
            ]),
        ),
        ("source_revision", top_field(view, "source_revision")?),
        ("authority_boundary", top_field(view, "authority_boundary")?),
    ]))
}
fn top_field(view: &PreparedReadTransaction<'_>, name: &str) -> Result<JsonValue> {
    view.top()
        .object_get(name)
        .cloned()
        .ok_or_else(|| unavailable("incomplete prepared reader header"))
}

/// One request meter retained through the caller's post-read COMMIT/BEGIN
/// current-binding observation. It changes no transaction boundary itself.
pub struct PreparedSearchSession<'a> {
    read: Read<'a>,
}
impl<'a> PreparedSearchSession<'a> {
    pub fn new(connection: &'a Connection, limits: PreparedReadLimits) -> Result<Self> {
        Ok(Self {
            read: Read::new(connection, limits)?,
        })
    }
    pub fn new_with_abort(
        connection: &'a Connection,
        limits: PreparedReadLimits,
        abort: Option<Arc<dyn crate::AbortProbe>>,
    ) -> Result<Self> {
        Ok(Self {
            read: Read::with_abort(connection, limits, abort)?,
        })
    }
    pub fn search(
        &mut self,
        binding: &JsonValue,
        request: CompressedSearchRequest,
        limits: PublishedSearchLimits,
        now: u64,
    ) -> Result<JsonValue> {
        let normalized = request.normalize()?;
        let incoming = request.cursor.as_deref().map(decode_outer).transpose()?;
        let body_per_kind = body_budget(self.read.limits, limits)?;
        self.read.check_abort()?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                self.read
                    .check_abort()
                    .err()
                    .unwrap_or_else(|| prepared_error(e))
            })?;
        self.read.absorb_owner(&view)?;
        let result = search_checked(
            &mut self.read,
            &view,
            request,
            normalized,
            limits,
            body_per_kind,
            now,
            incoming,
        );
        drop(view);
        self.read.reset_owner();
        self.read.check_abort()?;
        result
    }
    pub fn capability(
        &mut self,
        binding: &JsonValue,
        limits: PublishedSearchLimits,
    ) -> Result<JsonValue> {
        let body_per_kind = body_budget(self.read.limits, limits)?;
        self.read.check_abort()?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                self.read
                    .check_abort()
                    .err()
                    .unwrap_or_else(|| prepared_error(e))
            })?;
        self.read.absorb_owner(&view)?;
        let result = capability_checked(&mut self.read, &view, limits, body_per_kind);
        drop(view);
        self.read.reset_owner();
        self.read.check_abort()?;
        result
    }
    /// Admit the schema and independent binding without reading catalog or search rows.
    /// Used before an exact-source operation paired with this publication.
    pub fn admit_binding(&mut self, binding: &JsonValue) -> Result<()> {
        self.read.check_abort()?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                self.read
                    .check_abort()
                    .err()
                    .unwrap_or_else(|| prepared_error(e))
            })?;
        let result = self.read.absorb_owner(&view);
        drop(view);
        self.read.reset_owner();
        result
    }
    /// Pair the selected exact-source vector with the publication in this snapshot.
    pub fn source_binding(&mut self, binding: &JsonValue, expected_digest: &str) -> Result<()> {
        let cap = 1_048_576_i64;
        let rows: Vec<(Option<String>, Option<String>, Option<String>)> = self.read.query(
            "SELECT CASE WHEN typeof(binding)='text' AND length(CAST(binding AS BLOB))<=? THEN binding END,CASE WHEN typeof(inputs)='text' AND length(CAST(inputs AS BLOB))<=? THEN inputs END,CASE WHEN typeof(sha256)='text' AND length(sha256)=64 THEN sha256 END FROM prepared_source_state WHERE singleton=1 LIMIT 2",
            &[&cap, &cap], true, |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let [(Some(stored_binding), Some(inputs), Some(digest))] = rows.as_slice() else {
            return Err(unavailable("prepared source state missing or invalid"));
        };
        let parsed = tos_compiler::prepared_source_binding::validate_prepared_source_state(
            binding,
            stored_binding.as_bytes(),
            inputs.as_bytes(),
            digest,
            Default::default(),
        )
        .map_err(|_| unavailable("prepared source state binding invalid"))?;
        if parsed.digest() != expected_digest {
            return Err(err(
                CompressedSearchErrorCode::StaleBinding,
                "selected source vector differs from publication",
            ));
        }
        self.read.check_abort()
    }
    pub fn recheck_binding(&mut self, binding: &JsonValue) -> Result<()> {
        self.read.check_abort()?;
        let view =
            PreparedReadTransaction::recheck_binding(self.read.db, binding, self.read.limits)
                .map_err(|e| {
                    self.read
                        .check_abort()
                        .err()
                        .unwrap_or_else(|| prepared_error(e))
                })?;
        let result = self.read.absorb_owner(&view);
        drop(view);
        self.read.reset_owner();
        result
    }

    /// Return the exact stored catalog under the same request meter and
    /// caller-held snapshot used by search. External currentness stays owned
    /// by the caller's fresh-snapshot recheck and disclosure fence.
    pub fn inspect(
        &mut self,
        binding: &JsonValue,
        kind: crate::search_v2::SearchKind,
        identifier: &str,
        relation_limit: usize,
    ) -> std::result::Result<JsonValue, crate::search_v2::SearchV2Error> {
        use crate::prepared_inspect::storage_error;
        self.read.check_abort().map_err(storage_error)?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                storage_error(
                    self.read
                        .check_abort()
                        .err()
                        .unwrap_or_else(|| prepared_error(e)),
                )
            })?;
        self.read.absorb_owner(&view).map_err(storage_error)?;
        let result = crate::prepared_inspect::inspect(
            &mut self.read,
            &view,
            kind,
            identifier,
            relation_limit,
        );
        drop(view);
        self.read.reset_owner();
        self.read.check_abort().map_err(storage_error)?;
        result
    }

    pub fn lens(
        &mut self,
        binding: &JsonValue,
        spec: &JsonValue,
    ) -> std::result::Result<JsonValue, crate::search_v2::SearchV2Error> {
        use crate::prepared_inspect::storage_error;
        self.read.check_abort().map_err(storage_error)?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                storage_error(
                    self.read
                        .check_abort()
                        .err()
                        .unwrap_or_else(|| prepared_error(e)),
                )
            })?;
        self.read.absorb_owner(&view).map_err(storage_error)?;
        let result = crate::prepared_lens::lens(&mut self.read, &view, spec);
        drop(view);
        self.read.reset_owner();
        self.read.check_abort().map_err(storage_error)?;
        result
    }

    pub fn temporal(
        &mut self,
        binding: &JsonValue,
        request: &JsonValue,
    ) -> std::result::Result<JsonValue, crate::search_v2::SearchV2Error> {
        use crate::prepared_inspect::storage_error;
        self.read.check_abort().map_err(storage_error)?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                storage_error(
                    self.read
                        .check_abort()
                        .err()
                        .unwrap_or_else(|| prepared_error(e)),
                )
            })?;
        self.read.absorb_owner(&view).map_err(storage_error)?;
        let result = crate::prepared_operations::temporal(&mut self.read, &view, request);
        drop(view);
        self.read.reset_owner();
        self.read.check_abort().map_err(storage_error)?;
        result
    }

    pub fn focus(
        &mut self,
        binding: &JsonValue,
        request: &crate::knowledge_focus::KnowledgeFocusRequest,
    ) -> std::result::Result<JsonValue, crate::search_v2::SearchV2Error> {
        let spec = crate::prepared_operations::focus_spec(&self.read, request)?;
        self.lens(binding, &spec)
    }

    pub fn stored_lens(
        &mut self,
        binding: &JsonValue,
        identifier: &str,
    ) -> std::result::Result<JsonValue, crate::search_v2::SearchV2Error> {
        use crate::prepared_inspect::storage_error;
        self.read.check_abort().map_err(storage_error)?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                storage_error(
                    self.read
                        .check_abort()
                        .err()
                        .unwrap_or_else(|| prepared_error(e)),
                )
            })?;
        self.read.absorb_owner(&view).map_err(storage_error)?;
        let result = (|| {
            let catalog = catalog_checked(&mut self.read, &view).map_err(storage_error)?;
            let spec = crate::prepared_operations::stored_spec(&catalog, identifier)?;
            crate::prepared_lens::lens(&mut self.read, &view, &spec)
        })();
        drop(view);
        self.read.reset_owner();
        self.read.check_abort().map_err(storage_error)?;
        result
    }

    pub fn explore(
        &mut self,
        binding: &JsonValue,
        request: &JsonValue,
        checkpoints: &mut dyn crate::knowledge_exploration::ExplorationCheckpoints,
    ) -> std::result::Result<
        crate::prepared_exploration::PreparedExploration,
        crate::search_v2::SearchV2Error,
    > {
        use crate::prepared_inspect::storage_error;
        self.read.check_abort().map_err(storage_error)?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                storage_error(
                    self.read
                        .check_abort()
                        .err()
                        .unwrap_or_else(|| prepared_error(e)),
                )
            })?;
        self.read.absorb_owner(&view).map_err(storage_error)?;
        let result =
            crate::prepared_exploration::explore(&mut self.read, &view, request, checkpoints);
        drop(view);
        self.read.reset_owner();
        self.read.check_abort().map_err(storage_error)?;
        result
    }

    pub fn catalog(&mut self, binding: &JsonValue) -> Result<JsonValue> {
        self.read.check_abort()?;
        let view = PreparedReadTransaction::admit(self.read.db, binding, self.read.limits)
            .map_err(|e| {
                self.read
                    .check_abort()
                    .err()
                    .unwrap_or_else(|| prepared_error(e))
            })?;
        self.read.absorb_owner(&view)?;
        let result = catalog_checked(&mut self.read, &view);
        drop(view);
        self.read.reset_owner();
        self.read.check_abort()?;
        result
    }
}

fn catalog_checked(read: &mut Read<'_>, view: &PreparedReadTransaction<'_>) -> Result<JsonValue> {
    let key = "knowledge_catalog";
    let probes = read.query(
        "SELECT part,typeof(json_chunk),length(CAST(json_chunk AS BLOB)) FROM edge_meta WHERE key=?1 ORDER BY part LIMIT 257",
        &[&key], false,
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?)),
    )?;
    if probes.is_empty() || probes.len() > 256 {
        return Err(unavailable("prepared catalog chunk count invalid"));
    }
    let mut bytes = 0usize;
    for (index, (part, kind, length)) in probes.iter().enumerate() {
        if *part != index as i64 || kind != "text" {
            return Err(unavailable("prepared catalog chunks incomplete or invalid"));
        }
        let length = length
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| unavailable("prepared catalog chunk length invalid"))?;
        if length > 131_072 {
            return Err(budget("prepared catalog chunk byte budget exceeded"));
        }
        bytes = bytes
            .checked_add(length)
            .ok_or_else(|| budget("prepared catalog byte overflow"))?;
        if bytes
            > read
                .limits
                .max_row_bytes
                .min(read.limits.max_response_bytes)
        {
            return Err(budget("prepared catalog byte budget exceeded"));
        }
    }
    read.charge_bytes(bytes)?;
    let chunks = read.query(
        "SELECT part,json_chunk FROM edge_meta WHERE key=?1 ORDER BY part LIMIT 257",
        &[&key],
        false,
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )?;
    if chunks.len() != probes.len() {
        return Err(unavailable("prepared catalog chunk closure changed"));
    }
    let mut raw = Vec::with_capacity(bytes);
    for (index, (part, chunk)) in chunks.into_iter().enumerate() {
        if part != index as i64 || Some(chunk.len() as i64) != probes[index].2 {
            return Err(unavailable("prepared catalog chunk closure changed"));
        }
        raw.extend_from_slice(chunk.as_bytes());
    }
    let value = parse_json(&raw, JsonMode::PublishedStrict, json_limits(bytes))
        .map_err(|_| unavailable("prepared catalog JSON invalid"))?
        .into_root();
    if value.object_get("schema").and_then(JsonValue::as_str) != Some("tos_knowledge_catalog_v1")
        || value.object_get("source_revision") != view.top().object_get("source_revision")
        || view
            .top()
            .object_get("catalog_sha256")
            .and_then(JsonValue::as_str)
            != Some(Digest256::of_bytes(&raw).to_hex().as_str())
    {
        return Err(unavailable(
            "prepared catalog differs from selected snapshot",
        ));
    }
    view.check_current().map_err(prepared_error)?;
    read.absorb_owner(view)?;
    Ok(value)
}
fn search_checked(
    read: &mut Read<'_>,
    view: &PreparedReadTransaction<'_>,
    request: CompressedSearchRequest,
    normalized: NormalizedRequest,
    limits: PublishedSearchLimits,
    body_per_kind: usize,
    now: u64,
    incoming: Option<(OuterState, JsonValue, String)>,
) -> Result<JsonValue> {
    view.check_current().map_err(prepared_error)?;
    read.absorb_owner(view)?;
    let binding_hash = hash(view.binding())?;
    let query_hash = hash(&JsonValue::Array(vec![
        view.binding().clone(),
        string(&normalized.needle),
        normalized.filters.clone(),
    ]))?;
    if incoming
        .as_ref()
        .is_some_and(|(s, _, _)| s.binding != binding_hash)
    {
        return Err(err(
            CompressedSearchErrorCode::StaleBinding,
            "search cursor selects another publication; restart query",
        ));
    }
    let (key, header_bytes) = key(read, view)?;
    let mut state = if let Some((state, raw, mac)) = incoming {
        if !constant_time_equal(
            &hmac(&key, MAC_DOMAIN, &compact(&raw, MAX_CURSOR_BYTES)?),
            &mac,
        ) {
            return Err(cursor_error(
                "invalid published search cursor integrity; restart query",
            ));
        }
        if state.query != query_hash {
            return Err(cursor_error(
                "published search cursor query/filter mismatch",
            ));
        }
        if state.expires <= now {
            return Err(err(
                CompressedSearchErrorCode::CursorExpired,
                "published search cursor expired",
            ));
        }
        state
    } else {
        OuterState::new(binding_hash, query_hash, now)?
    };
    let byte_share = read
        .limits
        .max_bytes
        .saturating_sub(read.bytes)
        .saturating_sub(FINAL_READ_BYTES)
        / 2;
    let row_share = read
        .limits
        .max_rows
        .saturating_sub(read.rows)
        .saturating_sub(FINAL_READ_ROWS)
        / 2;
    if byte_share
        < (limits.metadata_bytes + limits.verification_bytes).max(
            read.limits
                .max_row_bytes
                .saturating_mul(4)
                .saturating_add(1024),
        )
        || row_share < 270
    {
        return Err(budget(
            "reader budget cannot admit bounded search and carrier continuation",
        ));
    }
    let (nodes, node_ranks, node_work) = kind_page(
        read,
        &mut state.nodes,
        "node",
        view.binding(),
        &normalized.needle,
        &object(vec![
            ("source_graph", strings(&normalized.sources)),
            ("kind_id", strings(&normalized.kind_ids)),
        ]),
        request.limit,
        limits,
        body_per_kind,
        byte_share,
        row_share,
        now,
    )?;
    let (relations, relation_ranks, relation_work) = kind_page(
        read,
        &mut state.relations,
        "relation",
        view.binding(),
        &normalized.needle,
        &object(vec![
            ("source_graph", strings(&normalized.sources)),
            ("predicate_id", strings(&normalized.predicate_ids)),
        ]),
        request.limit,
        limits,
        body_per_kind,
        byte_share,
        row_share,
        now,
    )?;
    let more = !state.nodes.exhausted
        || !state.nodes.pending.is_empty()
        || !state.relations.exhausted
        || !state.relations.pending.is_empty();
    let next_cursor = if more {
        string(&encode_outer(&state, &key)?)
    } else {
        JsonValue::Null
    };
    let count = |state: &KindState, rows: usize| {
        if request.cursor.is_none() && state.exhausted && state.pending.is_empty() {
            number(rows as u64)
        } else {
            JsonValue::Null
        }
    };
    let packet = object(vec![
        ("schema", string(SCHEMA)),
        ("source_revision", top_field(view, "source_revision")?),
        ("query", string(&normalized.display_query)),
        ("filters", normalized.filters),
        (
            "page",
            object(vec![
                (
                    "cursor",
                    request
                        .cursor
                        .as_deref()
                        .map(string)
                        .unwrap_or(JsonValue::Null),
                ),
                ("next_cursor", next_cursor),
                ("limit_per_kind", number(request.limit as u64)),
                ("ordering_scope", string("global-rank")),
                ("has_more", JsonValue::Bool(more)),
            ]),
        ),
        (
            "counts",
            object(vec![
                ("returned_nodes", number(nodes.len() as u64)),
                ("returned_relations", number(relations.len() as u64)),
                ("scope", string("exact-only-initial-complete-kind")),
                ("matching_nodes", count(&state.nodes, nodes.len())),
                (
                    "matching_relations",
                    count(&state.relations, relations.len()),
                ),
            ]),
        ),
        ("nodes", JsonValue::Array(nodes)),
        ("relations", JsonValue::Array(relations)),
        (
            "ranks",
            object(vec![
                ("nodes", JsonValue::Array(node_ranks)),
                ("relations", JsonValue::Array(relation_ranks)),
            ]),
        ),
        ("authority_boundary", top_field(view, "authority_boundary")?),
        (
            "work",
            object(vec![
                ("nodes", node_work),
                ("relations", relation_work),
                ("read_rows", number(read.rows as u64)),
                ("read_bytes", number(read.bytes as u64)),
                ("read_vm_steps", number(read.steps())),
                ("search_header_bytes", number(header_bytes as u64)),
            ]),
        ),
    ]);
    compact(&packet, read.limits.max_response_bytes)?;
    view.check_current().map_err(prepared_error)?;
    read.absorb_owner(view)?;
    Ok(packet)
}

#[allow(clippy::too_many_arguments)]
fn kind_page(
    read: &mut Read<'_>,
    state: &mut KindState,
    kind: &str,
    binding: &JsonValue,
    needle: &str,
    filters: &JsonValue,
    limit: usize,
    limits: PublishedSearchLimits,
    body_per_kind: usize,
    byte_share: usize,
    row_share: usize,
    now: u64,
) -> Result<(Vec<JsonValue>, Vec<JsonValue>, JsonValue)> {
    let byte_end = read.bytes.saturating_add(byte_share);
    let row_end = read.rows.saturating_add(row_share);
    let mut inner_pages = 0;
    let mut search_work = JsonValue::Null;
    let mut deferred = false;
    let mut body_bytes = 0usize;
    if state.pending.is_empty() && !state.exhausted {
        let page = query_kind(
            read,
            binding,
            kind,
            needle,
            filters,
            limit,
            state.cursor.as_ref(),
            limits,
            now,
        )?;
        read.charge_bytes(
            page.work
                .metadata_bytes
                .saturating_add(page.work.verification_bytes),
        )?;
        state.pending = page
            .matches
            .iter()
            .map(|m| {
                Ok(Pending {
                    address: m.address,
                    rank: m.rank,
                    id_hash: id_hash(&m.id)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        state.cursor = page.cursor;
        state.exhausted = !page.has_more;
        inner_pages = 1;
        search_work = page.work.json();
    }
    let mut items = Vec::new();
    let mut ranks = Vec::new();
    while !state.pending.is_empty() && items.len() < limit {
        if read.rows.saturating_add(270) > row_end {
            deferred = true;
            break;
        }
        let pending = &state.pending[0];
        let address = pending.address as i64;
        let id_size=read.one("SELECT CASE WHEN typeof(id)='text' THEN length(CAST(id AS BLOB)) ELSE -1 END FROM prepared_documents WHERE doc_id=?1 AND kind=?2 LIMIT 2",&[&address,&kind],true,|r|r.get::<_,i64>(0))?.filter(|n|*n>0).ok_or_else(||unavailable("missing or invalid prepared search address mapping"))? as usize;
        if id_size > read.limits.max_row_bytes {
            return Err(budget(
                "prepared search identity exceeds the reader row budget",
            ));
        }
        if read.bytes.saturating_add(id_size) > byte_end {
            deferred = true;
            break;
        }
        let identifier = read
            .one(
                "SELECT id FROM prepared_documents WHERE doc_id=?1 AND kind=?2 LIMIT 2",
                &[&address, &kind],
                true,
                |r| r.get::<_, String>(0),
            )?
            .ok_or_else(|| unavailable("missing prepared address identity"))?;
        if id_hash(&string(&identifier))? != pending.id_hash {
            return Err(unavailable(
                "prepared search address differs from exact search identity",
            ));
        }
        let columns = if kind == "node" {
            NODE_COLUMNS
        } else {
            RELATION_COLUMNS
        };
        let table = if kind == "node" {
            "knowledge_nodes"
        } else {
            "knowledge_relations"
        };
        let sizes = columns
            .iter()
            .map(|column| format!("length(CAST({column} AS BLOB))"))
            .collect::<Vec<_>>()
            .join("+");
        let types = columns
            .iter()
            .chain(std::iter::once(&"json"))
            .map(|column| format!("typeof({column})='text'"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let(body,indexed)=read.one(&format!("SELECT CASE WHEN {types} THEN length(CAST(json AS BLOB)) ELSE -1 END,({sizes}) FROM {table} WHERE id=?1 LIMIT 2"),&[&identifier],true,|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?.filter(|(body,indexed)|*body>0&&*indexed>=0).ok_or_else(||unavailable("missing or invalid prepared search carrier"))?;
        let body = body as usize;
        let indexed = indexed as usize;
        if body > read.limits.max_row_bytes {
            return Err(budget(
                "prepared search carrier exceeds the reader row budget",
            ));
        }
        if body.saturating_add(indexed).saturating_add(1024)
            > read.limits.max_row_bytes.max(131_072).saturating_add(4096)
        {
            return Err(budget(
                "prepared search carrier exceeds the reader record budget",
            ));
        }
        let digest_key = format!("knowledge_{kind}_digest:{identifier}");
        let(chunks,digest_bytes)=read.one("SELECT count(*),coalesce(sum(CASE WHEN typeof(json_chunk)='text' THEN length(CAST(json_chunk AS BLOB)) ELSE 1025 END),0) FROM edge_meta WHERE key=?1",&[&digest_key],true,|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?.ok_or_else(||unavailable("missing checksum metadata"))?;
        if !(1..=256).contains(&chunks) || !(1..=1024).contains(&digest_bytes) {
            return Err(unavailable("invalid selected carrier checksum framing"));
        }
        let fetch = id_size
            .saturating_add(body)
            .saturating_add(indexed)
            .saturating_add(digest_bytes as usize);
        if body_bytes.saturating_add(body) > body_per_kind
            || read.bytes.saturating_add(fetch) > byte_end
            || read.rows.saturating_add(2 + chunks as usize) > row_end
        {
            deferred = true;
            break;
        }
        // Match maintained read.items exact-key closure, including its bounded
        // identity select, all duplicate index columns, and emitted-byte digest.
        let exact = read
            .one(
                &format!("SELECT id FROM {table} WHERE id=?1 ORDER BY id LIMIT 2"),
                &[&identifier],
                true,
                |r| r.get::<_, String>(0),
            )?
            .ok_or_else(|| unavailable("missing exact carrier identity"))?;
        if exact != identifier {
            return Err(unavailable("selected carrier identity differs"));
        }
        let row = read
            .one(
                &format!(
                    "SELECT {},json FROM {table} WHERE id=?1 ORDER BY id LIMIT 2",
                    columns.join(",")
                ),
                &[&identifier],
                true,
                |r| {
                    let mut values = Vec::new();
                    for i in 0..columns.len() {
                        values.push(r.get::<_, String>(i)?);
                    }
                    Ok((values, r.get::<_, String>(columns.len())?))
                },
            )?
            .ok_or_else(|| unavailable("selected carrier disappeared"))?;
        let digest_rows = read.query(
            "SELECT part,json_chunk FROM edge_meta WHERE key=?1 ORDER BY part LIMIT 257",
            &[&digest_key],
            true,
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )?;
        if digest_rows.len() != chunks as usize
            || digest_rows
                .iter()
                .enumerate()
                .any(|(i, (part, _))| *part != i as i64)
        {
            return Err(unavailable("prepared checksum chunks are incomplete"));
        }
        let raw_digest = digest_rows
            .iter()
            .map(|(_, s)| s.as_str())
            .collect::<String>();
        if raw_digest.len() != digest_bytes as usize {
            return Err(unavailable("prepared checksum byte framing changed"));
        }
        let digest = parse_json(
            raw_digest.as_bytes(),
            JsonMode::PublishedStrict,
            json_limits(1024),
        )
        .map_err(|_| unavailable("invalid emitted row checksum"))?
        .into_root();
        if digest.as_object().is_none_or(|o| o.len() != 1)
            || digest.object_get("sha256").and_then(JsonValue::as_str)
                != Some(Digest256::of_bytes(row.1.as_bytes()).to_hex().as_str())
        {
            return Err(unavailable("emitted knowledge row checksum differs"));
        }
        if row.1.len() != body {
            return Err(unavailable("selected carrier byte framing changed"));
        }
        let item = parse_json(
            row.1.as_bytes(),
            JsonMode::PublishedStrict,
            json_limits(read.limits.max_row_bytes),
        )
        .map_err(|_| unavailable("prepared carrier contains invalid or over-complex JSON"))?
        .into_root();
        if item.as_object().is_none()
            || columns.iter().zip(&row.0).any(|(column, expected)| {
                tos_compiler::local_prepared::index_value(&item, column)
                    .ok()
                    .as_deref()
                    != Some(expected.as_str())
            })
        {
            return Err(unavailable(
                "knowledge row identity/index columns differ from its full packet",
            ));
        }
        if id_hash(item.object_get("id").unwrap_or(&JsonValue::Null))? != pending.id_hash {
            return Err(unavailable(
                "selected prepared search identity closure differs",
            ));
        }
        ranks.push(object(vec![
            ("doc_id", number(pending.address)),
            ("rank", number(pending.rank as u64)),
            (
                "explanation",
                string(if needle.is_empty() {
                    "all-items"
                } else {
                    RANKS[pending.rank as usize]
                }),
            ),
        ]));
        items.push(item);
        body_bytes += body;
        state.pending.remove(0);
    }
    let work = object(vec![
        ("inner_pages", number(inner_pages)),
        ("body_rows", number(items.len() as u64)),
        ("body_bytes", number(body_bytes as u64)),
        ("deferred", JsonValue::Bool(deferred)),
        ("search", search_work),
    ]);
    Ok((items, ranks, work))
}
