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
use tos_compiler::{
    ControlledKnowledgeModel, ControlledQueryHeap, ControlledSearchKind, Error as CompilerError,
    VerifiedKnowledgeModel,
};
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
pub(crate) fn rank(
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
    let value = rank_lowered(&id, &native, &primary, &visible, needle);
    Ok((value, id))
}
/// One maintained ordering rule for normalized legacy search, independent of
/// the owning representation and its original allocation/work admission.
pub(crate) fn rank_lowered(
    id: &str,
    native: &str,
    primary: &[String],
    visible: &[String],
    needle: &str,
) -> u8 {
    if id == needle || native == needle || primary.iter().any(|v| v == needle) {
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
    }
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
fn scan<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
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
pub fn execute_selected_legacy_search<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &LegacySearchRequest,
    caps: LegacySearchBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
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

/// Same maintained normalized-carrier v1 semantics over the compiler-owned
/// model and its original Query/Capture budget. This is a sibling delivery path;
/// the legacy Verified entrypoint above remains unchanged.
struct ControlledWork {
    candidates: usize,
    decoded_bytes: u64,
    document_bytes: u64,
    document_code_points: u64,
    retained_bytes: usize,
    read_vm_steps: u64,
}

struct ControlledHit {
    key: (u8, String, u64),
    canonical_item: Vec<u8>,
    bytes: usize,
}

fn controlled_compiler_error(reason: CompilerError) -> SearchV2Error {
    match reason {
        CompilerError::Budget(_) | CompilerError::SqliteVmBudget { .. } => budget(),
        CompilerError::Invalid(_) => error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "controlled selected legacy carrier invalid",
        ),
        CompilerError::Sql(rusqlite::Error::SqliteFailure(failure, _))
            if failure.code == rusqlite::ErrorCode::OperationInterrupted =>
        {
            budget()
        }
        CompilerError::Sql(_) => error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "controlled selected legacy query failed",
        ),
        _ => error(
            SearchV2ErrorCode::Unavailable,
            "controlled selected legacy owner refused",
        ),
    }
}

fn controlled_abort<A: InspectCurrentAuthority<'_> + ?Sized>(
    authority: &A,
) -> Result<(), SearchV2Error> {
    match authority
        .abort_probe()
        .and_then(|probe| probe.reason())
    {
        Some(crate::AbortReason::Cancelled) => Err(error(
            SearchV2ErrorCode::Cancelled,
            "selected knowledge query cancelled",
        )),
        Some(crate::AbortReason::DeadlineExceeded) => Err(error(
            SearchV2ErrorCode::DeadlineExceeded,
            "selected knowledge query deadline exceeded",
        )),
        None => Ok(()),
    }
}

fn controlled_scope_state(
    scope: &crate::knowledge_packet::IndexedDisclosureScope,
    policy: &crate::search_v2::CurrentPolicyBinding,
) -> Result<usize, SearchV2Error> {
    let scope_fields = [
        scope.operation_id.capacity(),
        scope.carrier_layer.capacity(),
        scope.intended_use.capacity(),
        scope.selected_model_receipt_id.capacity(),
        scope.source_cut.capacity(),
        scope.policy_issuer_ref.capacity(),
        scope.policy_receipt_id.capacity(),
        scope.policy_scope.capacity(),
        scope.policy_epoch.capacity(),
        scope.withdrawal_generation.capacity(),
    ];
    let policy_fields = [
        policy.scope.capacity(),
        policy.issuer_ref.capacity(),
        policy.authorization_receipt_id.capacity(),
        policy.policy_epoch.capacity(),
        policy.withdrawal_generation.capacity(),
    ];
    scope_fields
        .into_iter()
        .chain(policy_fields)
        .try_fold(
            std::mem::size_of_val(scope)
                .checked_add(std::mem::size_of_val(policy))
                .ok_or_else(budget)?,
            |sum, size| sum.checked_add(size).ok_or_else(budget),
        )
}

fn controlled_workspace_bytes(
    request: &LegacySearchRequest,
    caps: LegacySearchBudget,
    input_bytes: u64,
    input_values: u64,
    registered_bytes: usize,
    registered_sources: usize,
    metadata_bytes: usize,
) -> Result<usize, SearchV2Error> {
    let keep = request.offset.checked_add(request.limit).ok_or_else(budget)?;
    let fields = caps.inspect.max_field_bytes;
    let document = usize::try_from(
        caps.max_document_bytes
            .min(caps.document.max_document_bytes as u64),
    )
    .map_err(|_| budget())?;
    let retained_slots = keep
        .checked_mul(std::mem::size_of::<ControlledHit>())
        .ok_or_else(budget)?;
    let retained_keys = keep.checked_mul(fields).ok_or_else(budget)?;
    let input_copies = usize::try_from(input_bytes)
        .map_err(|_| budget())?
        .checked_mul(2)
        .ok_or_else(budget)?;
    let input_slots = usize::try_from(input_values)
        .map_err(|_| budget())?
        .checked_mul(
            std::mem::size_of::<String>()
                + 3 * std::mem::size_of::<usize>()
                + std::mem::size_of::<JsonValue>(),
        )
        .ok_or_else(budget)?;
    let catalog_copies = registered_bytes.checked_mul(2).ok_or_else(budget)?;
    let catalog_slots = registered_sources
        .checked_mul(
            std::mem::size_of::<String>()
                + 3 * std::mem::size_of::<usize>()
                + std::mem::size_of::<JsonValue>(),
        )
        .ok_or_else(budget)?;
    let output_slots = keep
        .checked_mul(2)
        .and_then(|count| count.checked_mul(std::mem::size_of::<JsonValue>()))
        .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<Vec<JsonValue>>()))
        .ok_or_else(budget)?;
    let response_envelope = 32usize
        .checked_mul(
            std::mem::size_of::<(String, JsonValue)>()
                + 5 * std::mem::size_of::<usize>(),
        )
        .and_then(|bytes| bytes.checked_add(512))
        .ok_or_else(budget)?;
    let ranking_fields = fields.checked_mul(4).ok_or_else(budget)?;
    let document_workspace = document.checked_mul(3).ok_or_else(budget)?;
    [
        std::mem::size_of::<ControlledWork>(),
        std::mem::size_of::<Vec<ControlledHit>>(),
        std::mem::size_of::<Vec<crate::ObservedInspectCarrier>>(),
        std::mem::size_of::<ControlledQueryHeap<'static, 'static, 'static>>(),
        std::mem::size_of::<DisclosableInspect<'static>>(),
        retained_slots,
        retained_keys,
        input_copies,
        input_slots,
        catalog_copies,
        catalog_slots,
        output_slots,
        response_envelope,
        caps.max_retained_bytes,
        caps.inspect.max_payload_bytes,
        ranking_fields,
        document_workspace,
        usize::try_from(input_bytes).map_err(|_| budget())?,
        caps.inspect.max_response_bytes,
        metadata_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| sum.checked_add(value).ok_or_else(budget))
}

fn scan_controlled<'hold, 'model, 'state, 'budget, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'model, 'state, 'budget>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    kind: SearchKind,
    sources: &[String],
    filter: &[String],
    needle: &str,
    request: &LegacySearchRequest,
    caps: LegacySearchBudget,
    work: &mut ControlledWork,
    consulted: &mut Vec<crate::ObservedInspectCarrier>,
    heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
) -> Result<(usize, Vec<ControlledHit>), SearchV2Error> {
    let remaining_candidates = caps.max_candidates.saturating_sub(work.candidates);
    let remaining_rows = caps
        .inspect
        .max_rows
        .saturating_sub(work.candidates as u64);
    let max_rows = remaining_candidates.min(usize::try_from(remaining_rows).unwrap_or(usize::MAX) as usize) as u64;
    let max_decoded = caps.inspect.max_decoded_bytes.saturating_sub(work.decoded_bytes);
    let max_vm = caps.inspect.max_read_vm_steps.saturating_sub(work.read_vm_steps);
    let mut matching = 0usize;
    let mut top: Vec<ControlledHit> = Vec::with_capacity(request.offset + request.limit);
    let mut retained = 0usize;
    let keep = request.offset + request.limit;
    let controlled_kind = match kind {
        SearchKind::Nodes => ControlledSearchKind::Nodes,
        SearchKind::Relations => ControlledSearchKind::Relations,
    };
    let scan = model
        .visit_controlled_legacy_search_rows(
            controlled_kind,
            sources,
            max_rows,
            max_decoded,
            max_vm,
            caps.inspect.max_payload_bytes,
            caps.inspect.max_field_bytes,
            caps.inspect.json,
            heap,
            |source_graph, position, payload_sha256, item, charge, heap| {
                if work.candidates >= caps.max_candidates
                    || work.candidates as u64 >= caps.inspect.max_rows
                {
                    return Err(budget());
                }
                work.candidates = work.candidates.checked_add(1).ok_or_else(budget)?;
                authority.check_selected()?;
                let id = string(get(item, "id"));
                let id_bytes = id.len();
                let source_bytes = source_graph.len();
                let payload_heap = item.retained_storage_bytes().map_err(|_| budget())?;
                let mut authorized = None;
                heap.with_temporary(
                    id_bytes
                        .checked_add(payload_heap)
                        .and_then(|n| n.checked_add(std::mem::size_of::<crate::InspectedCarrier>()))
                        .ok_or_else(budget)?,
                    || {
                    authorized = Some(authority.authorize_current_borrowed(
                        kind,
                        id,
                        position,
                        payload_sha256,
                        item,
                    ));
                })
                .map_err(controlled_compiler_error)?;
                authorized.ok_or_else(budget)??;

                let grows_observed = consulted.len() == consulted.capacity();
                let observation_hold = id_bytes
                    .checked_add(source_bytes)
                    .and_then(|n| {
                        n.checked_add(if grows_observed {
                            std::mem::size_of::<crate::ObservedInspectCarrier>()
                        } else {
                            0
                        })
                    })
                    .ok_or_else(budget)?;
                heap.retain(observation_hold).map_err(controlled_compiler_error)?;
                if grows_observed {
                    consulted.try_reserve_exact(1).map_err(|_| budget())?;
                }
                let mut observed_id = String::with_capacity(id_bytes);
                observed_id.push_str(id);
                let mut observed_source = String::with_capacity(source_bytes);
                observed_source.push_str(source_graph);
                consulted.push(crate::ObservedInspectCarrier {
                    kind,
                    id: observed_id,
                    source_graph: observed_source,
                    position,
                    payload_sha256,
                });
                charge(
                    id_bytes
                        .checked_add(source_bytes)
                        .ok_or_else(budget)?,
                )
                .map_err(controlled_compiler_error)?;

                if !filter.is_empty()
                    && !filter.iter().any(|value| {
                        value == string(get(
                            item,
                            if kind == SearchKind::Nodes {
                                "kind_id"
                            } else {
                                "predicate_id"
                            },
                        ))
                    })
                {
                    return Ok(());
                }

                let mut document_budget = caps.document;
                document_budget.max_document_bytes = document_budget.max_document_bytes.min(
                    caps.max_document_bytes
                        .saturating_sub(work.document_bytes)
                        .min(usize::MAX as u64) as usize,
                );
                document_budget.max_document_code_points =
                    document_budget.max_document_code_points.min(
                        caps.max_document_code_points
                            .saturating_sub(work.document_code_points)
                            .min(usize::MAX as u64) as usize,
                    );
                charge(item.retained_storage_bytes().map_err(|_| budget())?)
                    .map_err(controlled_compiler_error)?;
                let document = lower_search_value(item, document_budget).map_err(|err| {
                    error(
                        if err.code == crate::QueryErrorCode::BudgetExceeded {
                            SearchV2ErrorCode::BudgetExceeded
                        } else {
                            SearchV2ErrorCode::CorruptSelectedCarrier
                        },
                        "legacy search document invalid",
                    )
                })?;
                work.document_bytes = work
                    .document_bytes
                    .checked_add(document.lower.len() as u64)
                    .ok_or_else(budget)?;
                work.document_code_points = work
                    .document_code_points
                    .checked_add(document.code_points as u64)
                    .ok_or_else(budget)?;
                if work.document_bytes > caps.max_document_bytes
                    || work.document_code_points > caps.max_document_code_points
                {
                    return Err(budget());
                }
                charge(document.lower.len()).map_err(controlled_compiler_error)?;
                controlled_abort(authority)?;
                if !needle.is_empty() && !document.lower.contains(needle) {
                    return Ok(());
                }
                matching = matching.checked_add(1).ok_or_else(budget)?;
                let (rank, id_key) = rank(item, needle, kind, caps.inspect.max_field_bytes)?;
                let key = (rank, id_key, position);
                let insertion = top.partition_point(|hit| hit.key <= key);
                if insertion >= keep {
                    return Ok(());
                }
                charge(item.retained_storage_bytes().map_err(|_| budget())?)
                    .map_err(controlled_compiler_error)?;
                let canonical_item = heap
                    .canonicalize_owned_query_json(item, caps.inspect.json)
                    .map_err(controlled_compiler_error)?;
                let bytes = canonical_item.len();
                if top.len() == keep {
                    retained = retained.checked_sub(top.pop().unwrap().bytes).ok_or_else(budget)?;
                }
                retained = retained.checked_add(bytes).ok_or_else(budget)?;
                if retained > caps.max_retained_bytes.saturating_sub(work.retained_bytes) {
                    return Err(budget());
                }
                top.insert(
                    insertion,
                    ControlledHit {
                        key,
                        canonical_item,
                        bytes,
                    },
                );
                Ok(())
            },
        )
        .map_err(controlled_compiler_error)?
        .map_err(|reason| reason)?;
    work.decoded_bytes = work
        .decoded_bytes
        .checked_add(scan.decoded_bytes)
        .ok_or_else(budget)?;
    work.read_vm_steps = work
        .read_vm_steps
        .checked_add(scan.vm_steps)
        .ok_or_else(budget)?;
    if work.decoded_bytes > caps.inspect.max_decoded_bytes
        || work.read_vm_steps > caps.inspect.max_read_vm_steps
    {
        return Err(budget());
    }
    let selected: Vec<_> = top.into_iter().skip(request.offset).collect();
    for hit in &selected {
        work.retained_bytes = work
            .retained_bytes
            .checked_add(hit.bytes)
            .ok_or_else(budget)?;
    }
    Ok((matching, selected))
}

pub fn execute_controlled_legacy_search<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &LegacySearchRequest,
    caps: LegacySearchBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
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
    if request.offset > 100_000
        || !(1..=100).contains(&request.limit)
    {
        return Err(invalid());
    }
    if caps.inspect.max_open_vm_steps == 0
        || caps.inspect.max_read_vm_steps == 0
        || caps.inspect.max_matches == 0
        || caps.inspect.max_matches >= i64::MAX as usize
        || caps.inspect.max_rows == 0
        || caps.inspect.max_payload_bytes == 0
        || caps.inspect.max_payload_bytes > i64::MAX as usize
        || caps.inspect.max_field_bytes == 0
        || caps.inspect.max_field_bytes > i64::MAX as usize
        || caps.inspect.max_response_bytes == 0
        || caps.inspect.max_decoded_bytes == 0
        || caps.max_candidates == 0
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
    for source in request.sources.as_deref().unwrap_or(&[]) {
        if !source.is_empty() && !bound.registered_source_ids().contains(source) {
            return Err(invalid());
        }
    }
    bound.require_source_revision()?;
    bound.check_controlled_model(model)?;
    model
        .check_query_open_vm_admission(caps.inspect.max_open_vm_steps)
        .map_err(controlled_compiler_error)?;
    let metadata_forecast = authority.disclosure_metadata_state_upper_bound()?;
    let registered_sources = bound.registered_source_ids();
    let registered_bytes = registered_sources.iter().try_fold(0usize, |sum, source| {
        sum.checked_add(source.len()).ok_or_else(budget)
    })?;
    let workspace = controlled_workspace_bytes(
        request,
        caps,
        input_bytes,
        input_values,
        registered_bytes,
        registered_sources.len(),
        metadata_forecast,
    )?;
    model.charge_query_work(
        usize::try_from(input_bytes).map_err(|_| budget())?,
    ).map_err(controlled_compiler_error)?;

    let mut output = None;
    let mut query_error = None;
    let admission = model.with_owned_query_workspace(workspace, |model| {
        let result = (|| {
            let query = python_strip_unicode16_v1(&request.query, caps.inspect.max_field_bytes)
                .map_err(|_| budget())?;
            if query.chars().count() > 256 {
                return Err(invalid());
            }
            let needle = lower(query, 4096)?;
            let kinds = set(&request.kind_ids);
            let predicates = set(&request.predicate_ids);
            // Preserve the maintained v1 request boundary before scanning.
            if kinds.len() > 100 || predicates.len() > 100 {
                return Err(invalid());
            }
            let requested = request.sources.as_ref().map(|v| set(v)).unwrap_or_default();
            let sources = if requested.is_empty() {
                set(bound.registered_source_ids())
            } else {
                requested
            };
            let policy = authority.policy_binding();
            let scope = authority.disclosure_scope();
            let actual_metadata = controlled_scope_state(&scope, &policy)?;
            if actual_metadata > metadata_forecast {
                return Err(budget());
            }
            scope.validate_for(
                bound,
                &policy,
                LEGACY_SEARCH_OPERATION,
                LEGACY_SEARCH_INTENDED_USE,
            )?;
            controlled_abort(authority)?;
            authority.check_selected()?;
            if let Some(proof) = bound.source_basis().managed_source() {
                authority.authorize_managed_source_current(proof)?;
            }
            if let Some(proof) = bound.source_basis().managed_source_v2() {
                authority.authorize_managed_source_v2_current(proof)?;
            }

            let mut work = ControlledWork {
                candidates: 0,
                decoded_bytes: 0,
                document_bytes: 0,
                document_code_points: 0,
                retained_bytes: 0,
                read_vm_steps: 0,
            };
            let mut consulted = Vec::new();
            let mut heap = model.new_owned_query_heap();
            let (node_count, node_hits) = scan_controlled(
                model, bound, authority, SearchKind::Nodes, &sources, &kinds, &needle,
                request, caps, &mut work, &mut consulted, &mut heap,
            )?;
            let (relation_count, relation_hits) = scan_controlled(
                model, bound, authority, SearchKind::Relations, &sources, &predicates, &needle,
                request, caps, &mut work, &mut consulted, &mut heap,
            )?;

            let mut nodes = Vec::with_capacity(node_hits.len());
            for hit in node_hits {
                model.charge_query_work(hit.bytes).map_err(controlled_compiler_error)?;
                let value = model
                    .with_owned_query_json(&hit.canonical_item, caps.inspect.json, |value| {
                        let retained = value
                            .retained_storage_bytes()
                            .map_err(|_| CompilerError::Budget("controlled query output value"))?;
                        heap.retain(retained)?;
                        Ok(value.clone())
                    })
                    .map_err(controlled_compiler_error)?;
                nodes.push(value);
            }
            let mut relations = Vec::with_capacity(relation_hits.len());
            for hit in relation_hits {
                model.charge_query_work(hit.bytes).map_err(controlled_compiler_error)?;
                let value = model
                    .with_owned_query_json(&hit.canonical_item, caps.inspect.json, |value| {
                        let retained = value
                            .retained_storage_bytes()
                            .map_err(|_| CompilerError::Budget("controlled query output value"))?;
                        heap.retain(retained)?;
                        Ok(value.clone())
                    })
                    .map_err(controlled_compiler_error)?;
                relations.push(value);
            }
            let boundary = model
                .with_owned_query_json(
                    model.selection().authority_boundary.as_bytes(),
                    caps.inspect.json,
                    |value| {
                        let retained = value
                            .retained_storage_bytes()
                            .map_err(|_| CompilerError::Budget("controlled query output boundary"))?;
                        heap.retain(retained)?;
                        Ok(value.clone())
                    },
                )
                .map_err(controlled_compiler_error)?;
            let packet = object(vec![
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
                ("authority_boundary", boundary),
            ]);
            controlled_abort(authority)?;
            authority.check_selected()?;
            bound.check_controlled_model(model)?;
            let mut response_limits = caps.inspect.json;
            response_limits.max_bytes = response_limits
                .max_bytes
                .min(caps.inspect.max_response_bytes);
            let body = model
                .canonicalize_owned_query_json(&packet, response_limits)
                .map_err(controlled_compiler_error)?;
            if body.len() > caps.inspect.max_response_bytes {
                return Err(budget());
            }
            authority.check_selected()?;
            bound.check_controlled_model(model)?;
            let mut lease = authority.acquire_disclosure(&scope, &consulted)?;
            lease.recheck()?;
            controlled_abort(authority)?;
            authority.check_selected()?;
            bound.check_controlled_model(model)?;
            Ok(DisclosableInspect::from_controlled(body, lease))
        })();
        match result {
            Ok(packet) => {
                output = Some(packet);
                Ok(())
            }
            Err(reason) => {
                query_error = Some(reason);
                Err(CompilerError::Invalid("controlled legacy search refused"))
            }
        }
    });
    if let Some(reason) = query_error {
        return Err(reason);
    }
    admission.map_err(controlled_compiler_error)?;
    output.ok_or_else(|| error(
        SearchV2ErrorCode::Unavailable,
        "controlled legacy search produced no packet",
    ))
}
/// Describes this exact selected engine, under a distinct current held grant.
/// It neither probes ambient publications nor advertises public activation.
pub fn execute_selected_search_capabilities<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    caps: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
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
