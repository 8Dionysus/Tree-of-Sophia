//! Maintained v1 full-scan search over exact cold-selected normalized carriers.
//! Counts are complete or refused. Offset is not an indexed-v2 cursor. Every
//! consulted row remains under current authorization and the final held lease.
use crate::knowledge_inspect::{Reader, execute_selected_carrier_packet};
use crate::search_document::lower_search_value;
use crate::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode, SelectedQueryVocabulary};
use crate::source_read_projection::{object, text};
use crate::{
    BoundCmpKnowledge, DisclosableInspect, InspectBudget, InspectCurrentAuthority,
    SearchDocumentBudget,
};
use std::collections::BTreeSet;
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{
    CanonicalProfile, JsonNumber, JsonNumberKind, JsonValue, canonical_bytes_v1,
    python_lower_unicode16_v1, python_strip_unicode16_v1,
};

pub const LEGACY_SEARCH_OPERATION: &str = "tos.knowledge.search.legacy";
pub const LEGACY_SEARCH_INTENDED_USE: &str = "read_only_public_knowledge_search_v1";
pub const SEARCH_CAPABILITIES_OPERATION: &str = "tos.knowledge.search.capabilities";
pub const SEARCH_CAPABILITIES_INTENDED_USE: &str =
    "read_only_public_knowledge_search_capabilities_v1";

#[derive(Clone, Debug)]
pub struct LegacySearchRequest {
    pub query: String,
    pub sources: Option<Vec<String>>,
    pub kind_ids: Vec<String>,
    pub predicate_ids: Vec<String>,
    pub offset: usize,
    pub limit: usize,
}
impl Default for LegacySearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            sources: None,
            kind_ids: vec![],
            predicate_ids: vec![],
            offset: 0,
            limit: 40,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct LegacySearchBudget {
    pub inspect: InspectBudget,
    pub document: SearchDocumentBudget,
    /// Includes every nonmatching carrier and lookahead; shared across kinds.
    pub max_candidates: usize,
    /// Aggregate emitted/lowered document work; separate from SQL decoded bytes.
    pub max_document_bytes: u64,
    pub max_document_code_points: u64,
    /// Maximum offset+limit per kind, admitted before any scan.
    pub max_retained_per_kind: usize,
    /// Complete payloads retained across both kinds, including offset winners.
    pub max_retained_bytes: usize,
    pub block_size: usize,
}
fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn budget() -> SearchV2Error {
    error(
        SearchV2ErrorCode::BudgetExceeded,
        "legacy search budget exceeded",
    )
}
fn invalid() -> SearchV2Error {
    error(
        SearchV2ErrorCode::InvalidRequest,
        "invalid legacy search request",
    )
}
fn get<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue {
    v.object_get(key).unwrap_or(&JsonValue::Null)
}
fn string(v: &JsonValue) -> &str {
    v.as_str().unwrap_or("")
}
fn n(v: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: v.to_string(),
    })
}
fn strings(v: &[String]) -> JsonValue {
    JsonValue::Array(v.iter().map(|s| text(s)).collect())
}
fn set(v: &[String]) -> Vec<String> {
    v.iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn lower(v: &str, cap: usize) -> Result<String, SearchV2Error> {
    if v.len() > cap {
        return Err(budget());
    }
    python_lower_unicode16_v1(v, cap, cap, cap).map_err(|_| budget())
}
fn values(item: &JsonValue, fields: &[&str], cap: usize) -> Result<Vec<String>, SearchV2Error> {
    let mut result = vec![];
    let mut bytes = 0usize;
    for field in fields {
        let value = get(get(item, "display"), field);
        let mut add = |v: &str| -> Result<(), SearchV2Error> {
            let value = lower(v, cap)?;
            bytes = bytes.checked_add(value.len()).ok_or_else(budget)?;
            if bytes > cap {
                return Err(budget());
            }
            result.push(value);
            Ok(())
        };
        if let Some(v) = value.as_str() {
            add(v)?;
        } else if let Some(entries) = value.as_object() {
            for (_, v) in entries {
                if let Some(v) = v.as_str() {
                    add(v)?;
                }
            }
        }
    }
    Ok(result)
}
fn rank(
    item: &JsonValue,
    needle: &str,
    kind: SearchKind,
    cap: usize,
) -> Result<(u8, String), SearchV2Error> {
    let id = lower(string(get(item, "id")), cap)?;
    if needle.is_empty() {
        return Ok((3, id));
    }
    let native = lower(string(get(item, "native_id")), cap)?;
    let primary = values(
        item,
        if kind == SearchKind::Nodes {
            &["title"]
        } else {
            &["label"]
        },
        cap,
    )?;
    let visible = values(
        item,
        if kind == SearchKind::Nodes {
            &["title", "kind_label", "summary"]
        } else {
            &["label", "inverse_label", "statement", "explanation"]
        },
        cap,
    )?;
    let value = if id == needle || native == needle || primary.iter().any(|v| v == needle) {
        0
    } else if id.starts_with(needle)
        || native.starts_with(needle)
        || primary.iter().any(|v| v.starts_with(needle))
    {
        1
    } else if visible.iter().any(|v| v.contains(needle)) {
        2
    } else {
        3
    };
    Ok((value, id))
}
struct Work {
    candidates: usize,
    bytes: u64,
    chars: u64,
    retained_bytes: usize,
}
struct Hit {
    key: (u8, String, u64),
    item: JsonValue,
    bytes: usize,
}
fn scan<A: InspectCurrentAuthority + ?Sized>(
    reader: &mut Reader<'_, '_, A>,
    kind: SearchKind,
    sources: &[String],
    filter: &[String],
    needle: &str,
    request: &LegacySearchRequest,
    caps: LegacySearchBudget,
    work: &mut Work,
) -> Result<(usize, Vec<JsonValue>), SearchV2Error> {
    let expected = reader.scope_count(kind, sources)?;
    if expected > caps.max_candidates.saturating_sub(work.candidates) as u64 {
        return Err(budget());
    }
    let mut after: Option<(String, i64)> = None;
    let mut scanned = 0u64;
    let mut matching = 0usize;
    let mut top: Vec<Hit> = vec![];
    let mut retained = 0usize;
    let keep = request.offset + request.limit;
    loop {
        reader.check_interrupt()?;
        let ids = reader.candidate_ids(
            kind,
            sources,
            after.as_ref().map(|(s, p)| (s.as_str(), *p)),
            caps.block_size,
        )?;
        if ids.is_empty() {
            break;
        }
        for (source, position, id) in ids {
            reader.check_interrupt()?;
            work.candidates = work.candidates.checked_add(1).ok_or_else(budget)?;
            scanned += 1;
            if work.candidates > caps.max_candidates {
                return Err(budget());
            }
            after = Some((source, position));
            let mut rows = reader.items(kind, "id", &id, 1, false)?;
            let item = rows.pop().ok_or_else(|| {
                error(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "legacy candidate absent",
                )
            })?;
            if !filter.is_empty()
                && !filter.iter().any(|s| {
                    s == string(get(
                        &item,
                        if kind == SearchKind::Nodes {
                            "kind_id"
                        } else {
                            "predicate_id"
                        },
                    ))
                })
            {
                continue;
            }
            let mut document_budget = caps.document;
            document_budget.max_document_bytes = document_budget.max_document_bytes.min(
                caps.max_document_bytes
                    .saturating_sub(work.bytes)
                    .min(usize::MAX as u64) as usize,
            );
            document_budget.max_document_code_points =
                document_budget.max_document_code_points.min(
                    caps.max_document_code_points
                        .saturating_sub(work.chars)
                        .min(usize::MAX as u64) as usize,
                );
            let document = lower_search_value(&item, document_budget).map_err(|err| {
                error(
                    if err.code == crate::QueryErrorCode::BudgetExceeded {
                        SearchV2ErrorCode::BudgetExceeded
                    } else {
                        SearchV2ErrorCode::CorruptSelectedCarrier
                    },
                    "legacy search document invalid",
                )
            })?;
            work.bytes = work
                .bytes
                .checked_add(document.lower.len() as u64)
                .ok_or_else(budget)?;
            work.chars = work
                .chars
                .checked_add(document.code_points as u64)
                .ok_or_else(budget)?;
            if work.bytes > caps.max_document_bytes || work.chars > caps.max_document_code_points {
                return Err(budget());
            }
            reader.check_interrupt()?;
            if !needle.is_empty() && !document.lower.contains(needle) {
                continue;
            }
            matching = matching.checked_add(1).ok_or_else(budget)?;
            let (r, id) = rank(&item, needle, kind, caps.inspect.max_field_bytes)?;
            let key = (r, id, position as u64);
            let insertion = top.partition_point(|v| v.key <= key);
            if insertion >= keep {
                continue;
            }
            // Exact compact payload bytes, independently of Unicode lower
            // expansion/contraction. Logical accounting is not heap size.
            let bytes = canonical_bytes_v1(
                &item,
                CanonicalProfile::SourceRecordDigestV1,
                caps.inspect.json,
            )
            .map_err(|_| budget())?
            .len();
            if top.len() == keep {
                retained -= top.pop().unwrap().bytes;
            }
            retained = retained.checked_add(bytes).ok_or_else(budget)?;
            if retained > caps.max_retained_bytes.saturating_sub(work.retained_bytes) {
                return Err(budget());
            }
            top.insert(insertion, Hit { key, item, bytes });
        }
    }
    if scanned != expected {
        return Err(error(
            SearchV2ErrorCode::IndexIncomplete,
            "legacy scope enumeration incomplete",
        ));
    }
    let selected: Vec<_> = top.into_iter().skip(request.offset).collect();
    for hit in &selected {
        work.retained_bytes = work
            .retained_bytes
            .checked_add(hit.bytes)
            .ok_or_else(budget)?;
    }
    Ok((matching, selected.into_iter().map(|v| v.item).collect()))
}
pub fn execute_selected_legacy_search<A: InspectCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &LegacySearchRequest,
    caps: LegacySearchBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    // Transport has already supplied typed strings; retained source vocabulary
    // owns closure. Legacy permits unknown kind/predicate IDs (zero matches).
    let mut input_bytes = request.query.len() as u64;
    let mut input_values = 0u64;
    for value in request
        .kind_ids
        .iter()
        .chain(&request.predicate_ids)
        .chain(request.sources.as_deref().unwrap_or(&[]))
    {
        input_values = input_values.checked_add(1).ok_or_else(budget)?;
        input_bytes = input_bytes
            .checked_add(value.len() as u64)
            .ok_or_else(budget)?;
        if input_values > caps.inspect.max_rows
            || input_bytes > caps.inspect.max_decoded_bytes
            || value.len() > caps.inspect.max_field_bytes
        {
            return Err(budget());
        }
    }
    if request.query.len() > caps.inspect.max_field_bytes {
        return Err(budget());
    }
    let query = python_strip_unicode16_v1(&request.query, caps.inspect.max_field_bytes)
        .map_err(|_| budget())?;
    if query.chars().count() > 256
        || request.offset > 100_000
        || !(1..=100).contains(&request.limit)
    {
        return Err(invalid());
    }
    let needle = lower(query, 4096)?;
    let kinds = set(&request.kind_ids);
    let predicates = set(&request.predicate_ids);
    if kinds.len() > 100 || predicates.len() > 100 {
        return Err(invalid());
    }
    let requested = request.sources.as_ref().map(|v| set(v)).unwrap_or_default();
    let sources = if requested.is_empty() {
        set(bound.registered_source_ids())
    } else {
        requested
    };
    if sources
        .iter()
        .any(|s| !bound.registered_source_ids().contains(s))
    {
        return Err(invalid());
    }
    if caps.max_candidates == 0
        || caps.max_document_bytes == 0
        || caps.max_document_code_points == 0
        || caps.max_retained_bytes == 0
        || caps.block_size == 0
        || caps.block_size > caps.max_candidates
        || request.offset + request.limit > caps.max_retained_per_kind
        || caps.document.max_document_bytes == 0
        || caps.document.max_document_code_points == 0
    {
        return Err(budget());
    }
    bound.require_source_revision()?;
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        LEGACY_SEARCH_OPERATION,
        LEGACY_SEARCH_INTENDED_USE,
        caps.inspect,
        |reader| {
            let header = reader.header()?;
            let mut work = Work {
                candidates: 0,
                bytes: 0,
                chars: 0,
                retained_bytes: 0,
            };
            let (node_count, nodes) = scan(
                reader,
                SearchKind::Nodes,
                &sources,
                &kinds,
                &needle,
                request,
                caps,
                &mut work,
            )?;
            let (relation_count, relations) = scan(
                reader,
                SearchKind::Relations,
                &sources,
                &predicates,
                &needle,
                request,
                caps,
                &mut work,
            )?;
            Ok(object(vec![
                ("schema", text("tos_knowledge_search_v1")),
                ("source_revision", text(bound.require_source_revision()?)),
                ("query", text(query)),
                (
                    "filters",
                    object(vec![
                        ("sources", strings(&sources)),
                        ("kind_ids", strings(&kinds)),
                        ("predicate_ids", strings(&predicates)),
                    ]),
                ),
                (
                    "page",
                    object(vec![
                        ("offset", n(request.offset)),
                        ("limit_per_kind", n(request.limit)),
                    ]),
                ),
                (
                    "counts",
                    object(vec![
                        ("matching_nodes", n(node_count)),
                        ("matching_relations", n(relation_count)),
                        ("returned_nodes", n(nodes.len())),
                        ("returned_relations", n(relations.len())),
                    ]),
                ),
                ("nodes", JsonValue::Array(nodes)),
                ("relations", JsonValue::Array(relations)),
                (
                    "authority_boundary",
                    header
                        .object_get("authority_boundary")
                        .cloned()
                        .unwrap_or_else(|| object(vec![])),
                ),
            ]))
        },
    )
}
/// Describes this exact selected engine, under a distinct current held grant.
/// It neither probes ambient publications nor advertises public activation.
pub fn execute_selected_search_capabilities<A: InspectCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    caps: InspectBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        SEARCH_CAPABILITIES_OPERATION,
        SEARCH_CAPABILITIES_INTENDED_USE,
        caps,
        |reader| {
            reader.check_interrupt()?;
            Ok(object(vec![
                ("schema", text("tos_knowledge_search_capabilities_v1")),
                ("default_mode", text("legacy")),
                ("explicit_mode_required", JsonValue::Bool(false)),
                ("writes_to_tree", JsonValue::Bool(false)),
                (
                    "modes",
                    object(vec![
                        (
                            "legacy",
                            object(vec![
                                ("available", JsonValue::Bool(true)),
                                ("schema", text("tos_knowledge_search_v1")),
                                ("verification", text("engine-selection-only")),
                                ("pagination", text("offset")),
                            ]),
                        ),
                        (
                            "indexed",
                            object(vec![
                                ("available", JsonValue::Bool(true)),
                                ("schema", text("tos_knowledge_search_indexed_v2")),
                                ("verification", text("engine-selection-only")),
                                ("pagination", text("cursor")),
                                (
                                    "min_normalized_query_code_points",
                                    n(crate::search_v2::SEARCH_QUERY_MIN_CODE_POINTS),
                                ),
                            ]),
                        ),
                        (
                            "compressed",
                            object(vec![
                                ("available", JsonValue::Bool(false)),
                                ("schema", text("tos_knowledge_search_compressed_v3")),
                                (
                                    "reason",
                                    text("explicit-local-prepared-publication-required"),
                                ),
                                ("writes_to_tree", JsonValue::Bool(false)),
                            ]),
                        ),
                    ]),
                ),
            ]))
        },
    )
}
