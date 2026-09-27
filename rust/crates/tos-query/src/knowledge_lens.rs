//! Complete bounded lens execution on one cold-admitted selected carrier.
//! General matching streams candidates; only bounded winners retain payloads.
use crate::{
    inspect_plan::InspectBudget,
    knowledge_lens_spec::*,
    knowledge_presentation::{knowledge_scene, lens_carrier},
    search_v2::{SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, text},
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    knowledge_binding::BoundCmpKnowledge,
    knowledge_inspect::{
        DisclosableInspect, InspectCurrentAuthority, execute_selected_carrier_packet,
    },
    search_v2::SearchKind,
};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(target_arch = "wasm32"))]
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
pub const LENS_OPERATION: &str = "tos.lens.compile";
pub const LENS_INTENDED_USE: &str = "read_only_public_knowledge_lens_v1";
pub const FOCUS_OPERATION: &str = "tos.knowledge.focus";
pub const FOCUS_INTENDED_USE: &str = "read_only_public_knowledge_focus_v1";
pub const STORED_LENS_OPERATION: &str = "tos.lens.open";
pub const STORED_LENS_INTENDED_USE: &str = "read_only_public_stored_lens_v1";
/// Continuation identity for an already authorized selected execution. This
/// value binds a cursor; it neither issues a scope nor authorizes disclosure.
#[cfg(not(target_arch = "wasm32"))]
pub fn lens_continuation_binding(
    bound: &BoundCmpKnowledge<'_>,
    scope: &crate::IndexedDisclosureScope,
) -> JsonValue {
    object(vec![
        ("schema", text("tos_selected_lens_continuation_v1")),
        ("operation_id", text(&scope.operation_id)),
        ("carrier_layer", text(&scope.carrier_layer)),
        ("intended_use", text(&scope.intended_use)),
        (
            "selected_model_receipt_id",
            text(&scope.selected_model_receipt_id),
        ),
        ("source_cut", text(&scope.source_cut)),
        // Decimal text preserves every u64 sequence under Python's digest
        // grammar, whose JSON number branch intentionally uses binary64.
        (
            "through_commit_seq",
            text(&scope.through_commit_seq.to_string()),
        ),
        (
            "source_membership_root",
            text(&scope.source_membership_root.to_hex()),
        ),
        ("descriptor_sha256", text(&scope.descriptor_sha256.to_hex())),
        (
            "selected_index_sha256",
            text(&scope.selected_index_sha256.to_hex()),
        ),
        (
            "catalog_packet_sha256",
            text(&bound.selection().catalog_packet_sha256.to_hex()),
        ),
        (
            "catalog_index_root_sha256",
            text(&bound.selection().catalog_index_root_sha256.to_hex()),
        ),
        ("policy_issuer_ref", text(&scope.policy_issuer_ref)),
        ("policy_receipt_id", text(&scope.policy_receipt_id)),
        ("policy_scope", text(&scope.policy_scope)),
        ("policy_epoch", text(&scope.policy_epoch)),
        ("withdrawal_generation", text(&scope.withdrawal_generation)),
    ])
}
#[derive(Clone, Copy, Debug)]
pub struct LensBudget {
    pub inspect: InspectBudget,
    pub max_candidates: usize,
    pub max_path_steps: usize,
    pub max_adjacency_rows: usize,
    pub block_size: usize,
}
#[derive(Clone, Debug, Default)]
pub struct LensExecutionCounts {
    pub available_nodes: u64,
    pub available_relations: u64,
    pub matched_nodes: u64,
    pub matched_relations: u64,
    pub eligible_relations: u64,
    pub identity_expansion_limited: bool,
}
pub(crate) fn identity_selector(rule: &JsonValue, node: bool) -> Option<(String, Vec<String>)> {
    if rule.object_get("_property_binding").is_some() {
        return None;
    }
    let field = string(get(rule, "field"));
    if !(if node {
        &["id", "entity_id", "native_id"][..]
    } else {
        &["id", "native_id"][..]
    })
    .contains(&field)
        || !matches!(string(get(rule, "op")), "eq" | "in")
    {
        return None;
    }
    let value = get(rule, "value");
    let values = value.as_array().unwrap_or(std::slice::from_ref(value));
    if values.iter().any(|v| v.as_str().is_none()) {
        return None;
    }
    Some((
        field.to_owned(),
        values.iter().map(|v| string(v).to_owned()).collect(),
    ))
}
pub(crate) fn neighbors(r: &JsonValue, node: &str, direction: &str) -> Vec<String> {
    let mut out = vec![];
    if direction != "incoming" && string(get(r, "from_id")) == node {
        out.push(string(get(r, "to_id")).to_owned())
    }
    if direction != "outgoing" && string(get(r, "to_id")) == node {
        out.push(string(get(r, "from_id")).to_owned())
    }
    out.retain(|s| !s.is_empty());
    out
}
pub(crate) fn relation_regime(
    r: &JsonValue,
    spec: &JsonValue,
    vocabulary: &LensVocabulary,
) -> bool {
    let predicates = array(field(spec, "traversal.predicate_ids"));
    (predicates.is_empty() || predicates.iter().any(|p| p == get(r, "predicate_id")))
        && (string(field(spec, "traversal.profile")) != "overview"
            || (!vocabulary
                .overview_excluded_predicates
                .iter()
                .any(|p| p == string(get(r, "predicate_id")))
                && !vocabulary
                    .overview_excluded_relation_types
                    .iter()
                    .any(|p| p == string(get(r, "relation_type_id")))))
}
#[cfg(not(target_arch = "wasm32"))]
pub fn execute_selected_lens<A: InspectCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    value: &JsonValue,
    budget_value: LensBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    execute_selected_lens_request(
        model,
        bound,
        authority,
        LensRequest::Compile(value),
        budget_value,
    )
}
#[cfg(not(target_arch = "wasm32"))]
pub fn execute_selected_focus<A: InspectCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &crate::knowledge_focus::KnowledgeFocusRequest,
    budget_value: LensBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    execute_selected_lens_request(
        model,
        bound,
        authority,
        LensRequest::Focus(request),
        budget_value,
    )
}
#[cfg(not(target_arch = "wasm32"))]
pub fn execute_selected_stored_lens<A: InspectCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    lens_id: &str,
    budget_value: LensBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    if lens_id.is_empty() || lens_id.len() > 128 {
        return Err(invalid("invalid stored lens identifier"));
    }
    execute_selected_lens_request(
        model,
        bound,
        authority,
        LensRequest::Stored(lens_id),
        budget_value,
    )
}
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
enum LensRequest<'a> {
    Compile(&'a JsonValue),
    Focus(&'a crate::knowledge_focus::KnowledgeFocusRequest),
    Stored(&'a str),
}
#[cfg(not(target_arch = "wasm32"))]
fn execute_selected_lens_request<A: InspectCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: LensRequest<'_>,
    budget_value: LensBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    bound.require_source_revision()?;
    if budget_value.max_candidates == 0
        || budget_value.max_candidates >= i64::MAX as usize
        || budget_value.max_path_steps == 0
        || budget_value.max_adjacency_rows == 0
        || budget_value.block_size == 0
        || budget_value.block_size > budget_value.inspect.max_rows as usize
    {
        return Err(budget());
    }
    let (operation, intended_use) = match request {
        LensRequest::Compile(_) => (LENS_OPERATION, LENS_INTENDED_USE),
        LensRequest::Focus(_) => (FOCUS_OPERATION, FOCUS_INTENDED_USE),
        LensRequest::Stored(_) => (STORED_LENS_OPERATION, STORED_LENS_INTENDED_USE),
    };
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        operation,
        intended_use,
        budget_value.inspect,
        |read| {
            let publication = lens_continuation_binding(bound, read.disclosure_scope());
            let header = read.header()?;
            let vocabulary = LensVocabulary::from_selected(bound, &header)?;
            let value = match request {
                LensRequest::Compile(value) => value.clone(),
                LensRequest::Focus(request) => {
                    crate::knowledge_focus::focus_lens_spec(request, &vocabulary)?
                }
                LensRequest::Stored(identifier) => {
                    let catalog = read.catalog_packet(bound)?;
                    stored_lens_spec(&catalog, identifier)?
                }
            };
            let public_spec = normalize_lens_spec(&value, &vocabulary)?;
            let sources = array(get(&public_spec, "sources"))
                .iter()
                .map(|v| string(v).to_owned())
                .collect::<Vec<_>>();
            let available = (
                read.scope_count(SearchKind::Nodes, &sources)?,
                read.scope_count(SearchKind::Relations, &sources)?,
            );
            let mut plan = crate::lens_plan::LensPlan::native(
                public_spec,
                vocabulary,
                bound.require_source_revision()?,
                get(&header, "authority_boundary").clone(),
                publication,
                budget_value,
                available,
                read.abort_probe(),
            )?;
            while !plan.advance()? {
                read.check_interrupt()?;
                let need = plan
                    .need()
                    .ok_or_else(|| corrupt("native lens need absent"))?;
                use crate::lens_plan::{
                    LensCandidate, LensCandidateCursor, LensCandidatePage, LensNeed, LensReply,
                };
                let reply = match &*need {
                    LensNeed::ExactRows { kind, ids, .. } => {
                        let mut rows = vec![];
                        let mut raw_bytes = vec![];
                        for id in ids {
                            for (row, size) in read.items_with_sizes(*kind, "id", id, 1, true)? {
                                rows.push(row);
                                raw_bytes.push(size);
                            }
                        }
                        LensReply::Rows { rows, raw_bytes }
                    }
                    LensNeed::LookupRows {
                        field,
                        identifier,
                        limit,
                    } => {
                        let values = read.items_with_sizes(
                            SearchKind::Nodes,
                            field,
                            identifier,
                            *limit,
                            true,
                        )?;
                        let (rows, raw_bytes) = values.into_iter().unzip();
                        LensReply::Rows { rows, raw_bytes }
                    }
                    LensNeed::CandidateIds {
                        kind,
                        sources,
                        after,
                        limit,
                        ..
                    } => {
                        let cursor = match after {
                            None => None,
                            Some(LensCandidateCursor::SourceOrder { source, position }) => {
                                Some((source.as_str(), *position))
                            }
                            _ => return Err(corrupt("native lens cursor profile differs")),
                        };
                        let rows = read
                            .candidate_ids(*kind, sources, cursor, *limit)?
                            .into_iter()
                            .map(|(source, position, id)| LensCandidate {
                                id,
                                source: Some(source),
                                position: Some(position),
                            })
                            .collect();
                        LensReply::Candidates(LensCandidatePage { rows })
                    }
                    LensNeed::IdentityIds {
                        identifier,
                        sources,
                        after,
                        limit,
                    } => LensReply::Ids(read.identity_ids(identifier, sources, after, *limit)?),
                    LensNeed::IncidentIds {
                        identifier,
                        after,
                        limit,
                    } => LensReply::Ids(read.incident_ids(identifier, after, *limit)?),
                    _ => return Err(corrupt("published lens read requested by native profile")),
                };
                drop(need);
                plan.resume(reply)?;
            }
            read.check_interrupt()?;
            plan.finish()
        },
    )
}
pub(crate) fn is_truthy(v: &JsonValue) -> bool {
    match v {
        JsonValue::Null => false,
        JsonValue::Bool(b) => *b,
        JsonValue::Number(n) => n.lexeme.parse::<f64>().ok().is_some_and(|n| n != 0.),
        JsonValue::String(_) => !string(v).is_empty(),
        JsonValue::Array(a) => !a.is_empty(),
        JsonValue::Object(o) => !o.is_empty(),
    }
}
pub(crate) fn json_spaces(s: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in s.chars() {
        out.push(c);
        if quoted {
            if escaped {
                escaped = false
            } else if c == '\\' {
                escaped = true
            } else if c == '"' {
                quoted = false
            }
        } else if c == '"' {
            quoted = true
        } else if c == ':' || c == ',' {
            out.push(' ')
        }
    }
    out
}

fn count_number(value: u64) -> JsonValue {
    JsonValue::Number(tos_foundation::JsonNumber {
        kind: tos_foundation::JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

/// Finalize a bounded, exact selection. Selection/count authority remains with
/// the selected executor; this pure function adds delivery and presentation.
pub fn finalize_knowledge_lens(
    public: &JsonValue,
    mut nodes: Vec<JsonValue>,
    mut relations: Vec<JsonValue>,
    revision: &str,
    authority: &JsonValue,
    counts: LensExecutionCounts,
    resolved_focus: Option<&JsonValue>,
    inclusion: &JsonValue,
    traversed: &BTreeSet<String>,
    publication: Option<&JsonValue>,
    vocabulary: &LensVocabulary,
) -> Result<JsonValue, SearchV2Error> {
    sort_items(&mut nodes, field(public, "composition.sort_nodes"));
    sort_items(&mut relations, field(public, "composition.sort_relations"));
    let focus = if let Some(requested) = field(public, "seed.focus_node_id").as_str() {
        let resolved = resolved_focus.ok_or_else(|| corrupt("resolved lens focus absent"))?;
        let n = nodes
            .iter()
            .find(|n| get(n, "id") == get(resolved, "id"))
            .ok_or_else(|| corrupt("resolved lens focus missing from retained nodes"))?;
        let by = if string(get(n, "id")) == requested {
            "id"
        } else if string(get(n, "entity_id")) == requested {
            "entity_id"
        } else {
            "native_id"
        };
        let mut out = object(vec![
            ("requested_id", text(requested)),
            ("resolved_by", text(by)),
            ("node_id", get(n, "id").clone()),
        ]);
        for k in [
            "entity_id",
            "native_id",
            "source_graph",
            "kind_id",
            "type_id",
            "display",
        ] {
            set(&mut out, k, get(n, k).clone())
        }
        out
    } else {
        JsonValue::Null
    };
    let groups = groups(
        &nodes,
        &relations,
        array(field(public, "composition.group_by")),
        uint(field(public, "limits.groups")),
    );
    let refs: BTreeSet<_> = nodes
        .iter()
        .chain(&relations)
        .flat_map(|v| {
            array(get(v, "source_refs"))
                .iter()
                .filter_map(JsonValue::as_str)
        })
        .filter(|s| !s.is_empty())
        .collect();
    let missing_summaries = nodes
        .iter()
        .filter(|n| string(field(n, "display.summary_state")) == "missing")
        .count();
    let missing_explanations = relations
        .iter()
        .filter(|n| string(field(n, "display.explanation_state")) == "missing")
        .count();
    let no_summary = nodes
        .iter()
        .filter(|n| {
            matches!(
                field(n, "display.provenance.source_summary_available"),
                JsonValue::Bool(false)
            )
        })
        .count();
    let no_explanation = relations
        .iter()
        .filter(|n| {
            matches!(
                field(n, "display.provenance.source_explanation_available"),
                JsonValue::Bool(false)
            )
        })
        .count();
    let truncated_nodes = counts
        .matched_nodes
        .saturating_sub(uint(field(public, "limits.nodes")) as u64);
    let truncated_relations = counts
        .eligible_relations
        .saturating_sub(relations.len() as u64);
    let mut lens_for_digest = public.clone();
    remove(&mut lens_for_digest, "pagination");
    let pairs = |items: &[JsonValue]| {
        JsonValue::Array(
            items
                .iter()
                .map(|i| {
                    JsonValue::Array(vec![
                        get(i, "id").clone(),
                        get(i, "content_revision").clone(),
                    ])
                })
                .collect(),
        )
    };
    let fingerprint = stable_digest(&object(vec![
        ("execution_version", text("tos-lens-execution-v7")),
        ("source_revision", text(revision)),
        ("lens", lens_for_digest),
        ("nodes", pairs(&nodes)),
        ("relations", pairs(&relations)),
        ("groups", JsonValue::Array(groups.clone())),
    ]))?;
    let cursor_fingerprint = if let Some(publication) = publication {
        if !matches!(get(public, "pagination"), JsonValue::Null) {
            Some(stable_digest(&object(vec![
                ("schema", text("tos_published_lens_cursor_v1")),
                ("publication", publication.clone()),
                ("fingerprint", text(&fingerprint)),
            ]))?)
        } else {
            None
        }
    } else {
        None
    };
    let mut warnings = vec![];
    if counts.identity_expansion_limited {
        warnings.push(text("identity carrier expansion reached the node budget; use resumable exploration or narrower sources"))
    }
    for (n, template) in [
        (
            missing_summaries,
            "nodes expose an explicit missing-summary state",
        ),
        (
            missing_explanations,
            "relations expose an explicit missing-explanation state",
        ),
        (
            no_summary,
            "nodes use transparent metadata synthesis because no source summary is projected",
        ),
        (
            no_explanation,
            "relations use transparent metadata synthesis because no source explanation is projected",
        ),
    ] {
        if n > 0 {
            warnings.push(text(&format!("{n} {template}")))
        }
    }
    if truncated_nodes > 0 {
        warnings.push(text(&format!(
            "node selector exceeded its bounded result by {truncated_nodes} nodes"
        )))
    }
    if truncated_relations > 0 {
        warnings.push(text(&format!(
            "relation selector exceeded its bounded result by {truncated_relations} relations"
        )))
    }
    let lang = string(get(public, "language"));
    let detail = string(get(public, "detail"));
    let delivered_nodes = nodes
        .iter()
        .map(|n| lens_carrier(n, detail, Some(lang)))
        .collect::<Result<Vec<_>, _>>()?;
    let delivered_relations = relations
        .iter()
        .map(|r| lens_carrier(r, detail, Some(lang)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = object(vec![
        ("schema", text("tos_lens_result_v1")),
        ("source_revision", text(revision)),
        ("lens", public.clone()),
        ("fingerprint", text(&fingerprint)),
        ("presentation", get(public, "presentation").clone()),
        ("focus", focus.clone()),
        ("nodes", JsonValue::Array(delivered_nodes)),
        ("relations", JsonValue::Array(delivered_relations)),
        ("groups", JsonValue::Array(groups.clone())),
        (
            "facets",
            object(vec![
                ("node_kinds", counter(&nodes, "kind_id")),
                ("predicates", counter(&relations, "predicate_id")),
                ("sources", counter(&nodes, "source_graph")),
            ]),
        ),
        (
            "counts",
            object(vec![
                ("available_nodes", count_number(counts.available_nodes)),
                (
                    "available_relations",
                    count_number(counts.available_relations),
                ),
                ("matched_nodes", count_number(counts.matched_nodes)),
                ("matched_relations", count_number(counts.matched_relations)),
                (
                    "eligible_relations",
                    count_number(counts.eligible_relations),
                ),
                ("nodes", number(nodes.len())),
                ("relations", number(relations.len())),
                ("groups", number(groups.len())),
                ("truncated_nodes", count_number(truncated_nodes)),
                ("truncated_relations", count_number(truncated_relations)),
                (
                    "identity_expansion_limited",
                    JsonValue::Bool(counts.identity_expansion_limited),
                ),
                ("missing_node_summaries", number(missing_summaries)),
                (
                    "missing_relation_explanations",
                    number(missing_explanations),
                ),
                ("nodes_without_source_summary", number(no_summary)),
                (
                    "relations_without_source_explanation",
                    number(no_explanation),
                ),
            ]),
        ),
        (
            "source_refs",
            JsonValue::Array(refs.iter().map(|s| text(s)).collect()),
        ),
        ("warnings", JsonValue::Array(warnings)),
        ("authority_boundary", authority.clone()),
        (
            "agent_summary",
            object(vec![
                ("lens_id", get(public, "lens_id").clone()),
                ("focus_node_id", get(&focus, "node_id").clone()),
                ("node_count", number(nodes.len())),
                ("relation_count", number(relations.len())),
                ("group_count", number(groups.len())),
                ("source_ref_count", number(refs.len())),
                ("is_source", JsonValue::Bool(false)),
                ("writes_to_tree", JsonValue::Bool(false)),
            ]),
        ),
    ]);
    if boolean(get(public, "explain")) {
        set(
            &mut out,
            "inclusion",
            object(vec![
                ("nodes", inclusion.clone()),
                (
                    "relations",
                    object(
                        relations
                            .iter()
                            .map(|r| {
                                (
                                    string(get(r, "id")),
                                    object(vec![
                                        (
                                            "kind",
                                            text(if traversed.contains(string(get(r, "id"))) {
                                                "traversal"
                                            } else {
                                                "endpoint-policy"
                                            }),
                                        ),
                                        (
                                            "endpoint_policy",
                                            field(public, "composition.endpoint_policy").clone(),
                                        ),
                                    ]),
                                )
                            })
                            .collect(),
                    ),
                ),
                ("authority", text("query-execution-not-semantic-proof")),
            ]),
        );
        // Published packets retain Python's member order. Native canonical
        // emission sorts members, so this shared order leaves native bytes
        // unchanged while avoiding a second published result constructor.
        if let JsonValue::Object(fields) = &mut out {
            let position = fields
                .iter()
                .position(|(key, _)| key.as_str() == Some("inclusion"))
                .expect("inclusion was just inserted");
            let inclusion = fields.remove(position);
            let focus_position = fields
                .iter()
                .position(|(key, _)| key.as_str() == Some("focus"))
                .expect("lens result contains focus");
            fields.insert(focus_position + 1, inclusion);
        }
    }
    out = paginate_lens(out, cursor_fingerprint.as_deref())?;
    let scene = knowledge_scene(
        array(get(&out, "nodes")),
        array(get(&out, "relations")),
        get(&focus, "node_id").as_str(),
        None,
        vocabulary,
    )?;
    set(&mut out, "scene", scene);
    Ok(out)
}
fn counter(items: &[JsonValue], key: &str) -> JsonValue {
    let mut counts = BTreeMap::new();
    for i in items {
        *counts.entry(py_string(get(i, key))).or_insert(0usize) += 1;
    }
    object(
        counts
            .iter()
            .map(|(k, v)| (k.as_str(), number(*v)))
            .collect(),
    )
}
fn groups(
    nodes: &[JsonValue],
    relations: &[JsonValue],
    fields: &[JsonValue],
    limit: usize,
) -> Vec<JsonValue> {
    let mut out = vec![];
    for f in fields {
        let f = string(f);
        let mut values: BTreeMap<String, JsonValue> = BTreeMap::new();
        let mut ordered_keys = vec![];
        for (kind, items) in [("node", nodes), ("relation", relations)] {
            for item in items {
                let raw = field(item, f);
                for member in raw.as_array().unwrap_or(std::slice::from_ref(raw)) {
                    if matches!(member, JsonValue::Null) {
                        continue;
                    }
                    let key = py_string(member);
                    if !values.contains_key(&key) {
                        ordered_keys.push(key.clone());
                        values.insert(
                            key.clone(),
                            object(vec![
                                ("field", text(f)),
                                ("value", member.clone()),
                                ("node_ids", JsonValue::Array(vec![])),
                                ("relation_ids", JsonValue::Array(vec![])),
                            ]),
                        );
                    }
                    let entry = values.get_mut(&key).unwrap();
                    let slot = format!("{kind}_ids");
                    let mut ids = array(get(entry, &slot)).to_vec();
                    ids.push(get(item, "id").clone());
                    set(entry, &slot, JsonValue::Array(ids));
                }
            }
        }
        ordered_keys.sort_by_key(|s| {
            tos_foundation::python_casefold_unicode16_v1(
                s,
                s.chars().count(),
                s.chars().count().saturating_mul(3),
                s.len().saturating_mul(3),
            )
            .expect("admitted Unicode casefold bounds")
        });
        for key in ordered_keys {
            let mut entry = values.remove(&key).unwrap();
            let nc = array(get(&entry, "node_ids")).len();
            let rc = array(get(&entry, "relation_ids")).len();
            set(&mut entry, "node_count", number(nc));
            set(&mut entry, "relation_count", number(rc));
            out.push(entry);
            if out.len() >= limit {
                return out;
            }
        }
    }
    out
}
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
fn b64_encode(input: &[u8]) -> String {
    let mut out = String::new();
    let mut bits = 0u32;
    let mut count = 0usize;
    for byte in input {
        bits = (bits << 8) | *byte as u32;
        count += 8;
        while count >= 6 {
            count -= 6;
            out.push(BASE64[((bits >> count) & 63) as usize] as char)
        }
    }
    if count > 0 {
        out.push(BASE64[((bits << (6 - count)) & 63) as usize] as char)
    }
    out
}
fn b64_decode(s: &str) -> Result<Vec<u8>, SearchV2Error> {
    if s.len() % 4 == 1 {
        return Err(invalid("invalid lens cursor"));
    }
    let mut out = vec![];
    let mut bits = 0u32;
    let mut count = 0usize;
    for byte in s.bytes() {
        let v = BASE64
            .iter()
            .position(|b| *b == byte)
            .ok_or_else(|| invalid("invalid lens cursor"))?;
        bits = (bits << 6) | v as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8)
        }
    }
    Ok(out)
}
pub fn paginate_lens(
    mut result: JsonValue,
    cursor_fingerprint: Option<&str>,
) -> Result<JsonValue, SearchV2Error> {
    let options = field(&result, "lens.pagination").clone();
    if matches!(options, JsonValue::Null) {
        return Ok(result);
    }
    let fingerprint = cursor_fingerprint
        .unwrap_or(string(get(&result, "fingerprint")))
        .to_owned();
    let mut no = 0;
    let mut ro = 0;
    let nodes = array(get(&result, "nodes"));
    let relations = array(get(&result, "relations"));
    if let Some(cursor) = get(&options, "cursor").as_str() {
        let bytes = b64_decode(cursor)?;
        let token = parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default())
            .map_err(|_| invalid("invalid lens cursor"))?
            .into_root();
        if token.as_object().is_none_or(|o| {
            o.len() != 4
                || o.iter()
                    .any(|(k, _)| !matches!(k.as_str(), Some("v" | "fingerprint" | "n" | "r")))
        }) || get(&token, "v").as_u64() != Some(1)
            || get(&token, "fingerprint")
                .as_str()
                .is_none_or(|s| tos_foundation::Digest256::from_hex(s).is_err())
        {
            return Err(invalid("invalid lens cursor"));
        }
        let n = get(&token, "n")
            .as_u64()
            .filter(|n| *n <= 2000)
            .ok_or_else(|| invalid("invalid lens cursor position"))? as usize;
        let r = get(&token, "r")
            .as_u64()
            .filter(|n| *n <= 2000)
            .ok_or_else(|| invalid("invalid lens cursor position"))? as usize;
        if string(get(&token, "fingerprint")) != fingerprint {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::StaleSelection,
                message: "lens query or snapshot changed; restart pagination",
            });
        }
        if n > nodes.len() || r > relations.len() {
            return Err(invalid("invalid lens cursor position"));
        }
        no = n;
        ro = r;
    }
    let primary = &nodes[no..(no + uint(get(&options, "nodes"))).min(nodes.len())];
    let rels = relations[ro..(ro + uint(get(&options, "relations"))).min(relations.len())].to_vec();
    let primary_ids: BTreeSet<_> = primary.iter().map(|n| string(get(n, "id"))).collect();
    let primary_list = JsonValue::Array(primary.iter().map(|n| get(n, "id").clone()).collect());
    let mut selected = primary_ids.clone();
    for relation in &rels {
        selected.insert(string(get(relation, "from_id")));
        selected.insert(string(get(relation, "to_id")));
    }
    if let Some(f) = field(&result, "focus.node_id").as_str() {
        selected.insert(f);
    }
    let retained_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| selected.contains(string(get(n, "id"))))
        .cloned()
        .collect();
    let contexts = JsonValue::Array(
        retained_nodes
            .iter()
            .filter(|n| !primary_ids.contains(string(get(n, "id"))))
            .map(|n| get(n, "id").clone())
            .collect(),
    );
    let nn = no + primary.len();
    let nr = ro + rels.len();
    let has_more = nn < nodes.len() || nr < relations.len();
    let next = if has_more {
        let bytes = format!("{{\"v\":1,\"fingerprint\":\"{fingerprint}\",\"n\":{nn},\"r\":{nr}}}");
        text(&b64_encode(bytes.as_bytes()))
    } else {
        JsonValue::Null
    };
    let relation_ids: BTreeSet<_> = rels.iter().map(|r| string(get(r, "id"))).collect();
    let selected_owned: BTreeSet<_> = selected.iter().map(|s| (*s).to_owned()).collect();
    let relation_owned: BTreeSet<_> = relation_ids.iter().map(|s| (*s).to_owned()).collect();
    let mut new_groups = vec![];
    for group in array(get(&result, "groups")) {
        let mut group = group.clone();
        let ns: Vec<_> = array(get(&group, "node_ids"))
            .iter()
            .filter(|id| selected_owned.contains(string(id)))
            .cloned()
            .collect();
        let rs: Vec<_> = array(get(&group, "relation_ids"))
            .iter()
            .filter(|id| relation_owned.contains(string(id)))
            .cloned()
            .collect();
        if !ns.is_empty() || !rs.is_empty() {
            set(&mut group, "node_count", number(ns.len()));
            set(&mut group, "relation_count", number(rs.len()));
            set(&mut group, "node_ids", JsonValue::Array(ns));
            set(&mut group, "relation_ids", JsonValue::Array(rs));
            new_groups.push(group)
        }
    }
    let page = object(vec![
        ("next_cursor", next),
        ("has_more", JsonValue::Bool(has_more)),
        ("primary_node_ids", primary_list),
        ("context_node_ids", contexts),
        ("returned_nodes", number(retained_nodes.len())),
        ("returned_relations", number(rels.len())),
        ("scope", text("bounded-lens-result")),
        ("counts_scope", text("complete-bounded-result")),
    ]);
    set(&mut result, "nodes", JsonValue::Array(retained_nodes));
    set(&mut result, "relations", JsonValue::Array(rels));
    set(&mut result, "groups", JsonValue::Array(new_groups));
    set(&mut result, "page", page);
    if let Some(inclusion) = result.object_get("inclusion") {
        let mut inc = inclusion.clone();
        for (k, ids) in [("nodes", &selected_owned), ("relations", &relation_owned)] {
            let fields = get(&inc, k)
                .as_object()
                .unwrap_or(&[])
                .iter()
                .filter(|(key, _)| ids.contains(key.as_str().unwrap_or("")))
                .map(|(key, v)| (key.clone(), v.clone()))
                .collect();
            set(&mut inc, k, JsonValue::Object(fields));
        }
        set(&mut result, "inclusion", inc);
    }
    Ok(result)
}
