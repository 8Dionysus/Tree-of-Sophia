//! Maintained source dossier semantics. Raw navigation and complete rights must
//! come from one selected original component; normalized metadata alone is
//! insufficient. The kernel never grants permission to Item payload bytes.
use crate::knowledge_inspect::{Reader, execute_selected_carrier_packet};
use crate::search_v2::SearchKind;
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use crate::source_read_projection::{object, text};
use crate::{BoundCmpKnowledge, DisclosableInspect, InspectBudget, InspectCurrentAuthority};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tos_compiler::{NavigationOriginalMember, VerifiedKnowledgeModel};
use tos_foundation::JsonValue;
use tos_foundation::{CanonicalProfile, Digest256, canonical_bytes_v1};

pub const DOSSIER_OPERATION: &str = "tos.dossier.inspect";
pub const DOSSIER_INTENDED_USE: &str = "read_only_public_source_dossier_v1";
/// Whole maintained compatibility query over a bounded selected source graph.
/// Row/decoded/response/VM limits remain the existing shared inspect budget.
#[derive(Clone, Copy, Debug)]
pub struct DossierBudget {
    pub inspect: InspectBudget,
    pub max_candidates: usize,
    pub max_work_steps: u64,
    pub block_size: usize,
}

fn corrupt(message: &'static str) -> SearchV2Error {
    err(SearchV2ErrorCode::CorruptSelectedCarrier, message)
}

fn members<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    collection: &str,
    count: u64,
) -> Result<BTreeMap<String, NavigationOriginalMember>, SearchV2Error> {
    let mut result = BTreeMap::new();
    let mut after: Option<String> = None;
    while let Some(member) = read.original_member(collection, after.as_deref())? {
        if after.as_ref().is_some_and(|id| member.id <= *id)
            || member.collection != collection
            || result.len() as u64 >= count
        {
            return Err(corrupt("selected original membership differs"));
        }
        after = Some(member.id.clone());
        result.insert(member.id.clone(), member);
    }
    if result.len() as u64 != count {
        return Err(corrupt("selected original membership incomplete"));
    }
    Ok(result)
}

fn original_payloads<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    source: &str,
    kind: SearchKind,
    members: &BTreeMap<String, NavigationOriginalMember>,
    budget: DossierBudget,
    candidate_work: &mut usize,
) -> Result<BTreeMap<String, JsonValue>, SearchV2Error> {
    let sources = [source.to_owned()];
    let expected = read.scope_count(kind, &sources)?;
    if expected > budget.max_candidates.saturating_sub(*candidate_work) as u64 {
        return Err(err(
            SearchV2ErrorCode::BudgetExceeded,
            "dossier candidate cap exceeded",
        ));
    }
    let mut after: Option<(String, i64)> = None;
    let mut scanned = 0u64;
    let mut originals = BTreeMap::new();
    loop {
        let page = read.candidate_ids(
            kind,
            &sources,
            after.as_ref().map(|(s, p)| (s.as_str(), *p)),
            budget.block_size,
        )?;
        if page.is_empty() {
            break;
        }
        for (graph, position, id) in page {
            read.check_interrupt()?;
            *candidate_work = candidate_work.checked_add(1).ok_or_else(|| {
                err(
                    SearchV2ErrorCode::BudgetExceeded,
                    "dossier candidate cap exceeded",
                )
            })?;
            scanned += 1;
            if *candidate_work > budget.max_candidates || scanned > expected || graph != source {
                return Err(corrupt("selected dossier scope differs"));
            }
            after = Some((graph, position));
            let carriers = read.items(kind, "id", &id, 1, false)?;
            let carrier = carriers
                .into_iter().next()
                .ok_or_else(|| corrupt("selected dossier carrier absent"))?;
            if s(get(&carrier, "source_graph")) != source {
                return Err(corrupt("selected dossier source differs"));
            }
            let record = get(&carrier, "source_record");
            let payload = get(record, "payload");
            let native = s(get(
                payload,
                if kind == SearchKind::Nodes {
                    "node_id"
                } else {
                    "edge_id"
                },
            ));
            let Some(member) = members.get(native) else {
                continue;
            };
            let canonical = canonical_bytes_v1(
                payload,
                CanonicalProfile::SourceRecordDigestV1,
                budget.inspect.json,
            )
            .map_err(|_| {
                err(
                    SearchV2ErrorCode::BudgetExceeded,
                    "dossier original canonical cap exceeded",
                )
            })?;
            // Placeholder records may repeat an original ID. Only the root-bound
            // original canonical digest admits its exact semantic payload.
            if Digest256::of_bytes(&canonical).to_hex() != member.canonical_original_sha256 {
                continue;
            }
            if crate::knowledge_lens_spec::stable_digest(payload)? != member.semantic_sha256
                || s(get(record, "digest")) != member.semantic_sha256
            {
                return Err(corrupt("selected dossier original digest differs"));
            }
            let native = native.to_owned();
            if let std::collections::btree_map::Entry::Vacant(slot) = originals.entry(native) {
                let record = take_field(carrier, "source_record")?;
                slot.insert(take_field(record, "payload")?);
            }
        }
    }
    if scanned != expected || originals.len() != members.len() {
        return Err(corrupt("selected dossier original coverage incomplete"));
    }
    Ok(originals)
}

/// Exact original source and rights semantics, under one current selected hold.
/// Component retention is custody; the owner still authorizes disclosure.
pub fn execute_selected_dossier<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    object_id: &str,
    limit: usize,
    budget: DossierBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    if object_id.is_empty()
        || object_id.len() > budget.inspect.max_field_bytes
        || !(1..=300).contains(&limit)
    {
        return Err(err(
            SearchV2ErrorCode::InvalidRequest,
            "invalid dossier request",
        ));
    }
    if budget.max_candidates == 0
        || budget.max_work_steps == 0
        || budget.block_size == 0
        || budget.block_size > budget.inspect.max_rows as usize
    {
        return Err(err(
            SearchV2ErrorCode::BudgetExceeded,
            "invalid dossier budget",
        ));
    }
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        DOSSIER_OPERATION,
        DOSSIER_INTENDED_USE,
        budget.inspect,
        |read| {
            let (header, nodes, edges, rights) = load_original_navigation(read, bound, budget, None)?;
            compute_dossier(
                &header,
                &nodes,
                &edges.into_values().collect::<Vec<_>>(),
                &rights,
                object_id,
                limit,
                budget.max_work_steps,
                &mut || read.check_interrupt(),
            )
        },
    )
}
/// Availability reflects the real selected original component, never the
/// normalized graph alone or a borrowed dossier result.
pub fn selected_source_navigation_descend_available(
    model: &VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
) -> bool {
    if !model.navigation_original_available() || bound.check_model(model).is_err() {
        return false;
    }
    let Ok(receipt) = model.navigation_original_receipt() else { return false; };
    receipt.profile == tos_compiler::NAVIGATION_ORIGINAL_PROFILE
        && Some(receipt.source_graph.as_str()) == bound.source_for_adapter("source-navigation-node-edge-v1")
        && receipt.descriptor_sha256 == bound.selection().vocabulary.descriptor_sha256.to_hex()
        && receipt.source_cut == bound.selection().source_cut
        && receipt.membership_root == bound.selection().source_membership_root.to_hex()
}

/// Native selected-original descent, with the same bibliographic visibility,
/// BFS ordering and terminal owner disclosure lease as the maintained route.
pub fn execute_selected_source_navigation_descend<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &crate::SourceDescendRequest,
    budget: DossierBudget,
    max_retained_bytes: usize,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    if request.node_id.is_empty() || request.node_id.len() > budget.inspect.max_field_bytes
        || !(1..=8).contains(&request.max_depth) || !(1..=300).contains(&request.limit)
        || request.at_least_commit_seq.is_some() {
        // This immutable selected-original receipt has no publication sequence.
        return Err(err(SearchV2ErrorCode::InvalidRequest, "invalid selected original descent request"));
    }
    if budget.max_candidates == 0 || budget.max_work_steps == 0 || budget.block_size == 0
        || budget.block_size > budget.inspect.max_rows as usize {
        return Err(err(SearchV2ErrorCode::BudgetExceeded, "invalid selected original descent budget"));
    }
    execute_selected_carrier_packet(model, bound, authority,
        "tos.source.descend", "read_only_public_metadata_navigation_v1", budget.inspect, |read| {
            let (header, mut nodes, mut edges, _rights) =
                load_original_navigation(read, bound, budget, Some(max_retained_bytes))?;
            nodes.retain(|_, node| !crate::knowledge_lens_spec::truthy(get(get(node, "properties"), "packet_id")));
            if !nodes.contains_key(&request.node_id) {
                return Err(err(SearchV2ErrorCode::UnknownExactId, "unknown source-navigation node"));
            }
            let mut depths = BTreeMap::from([(request.node_id.clone(), 0u8)]);
            let mut queue = VecDeque::from([(request.node_id.clone(), 0u8)]);
            let mut selected_edges = Vec::new();
            let mut selected_edge_ids = BTreeSet::new();
            let mut truncated = false;
            let mut work = Work { left: budget.max_work_steps, interrupt: &mut || read.check_interrupt() };
            while let Some((current, depth)) = queue.pop_front() {
                work.step(1)?;
                if depth >= request.max_depth { continue; }
                // The root-bound BTree membership map supplies edge-ID order.
                for (id, edge) in &edges {
                    work.step(1)?;
                    if s(get(edge, "from_id")) != current { continue; }
                    let target = s(get(edge, "to_id"));
                    if !nodes.contains_key(target) { continue; }
                    if !depths.contains_key(target) && depths.len() >= request.limit {
                        truncated = true;
                        continue;
                    }
                    if selected_edge_ids.insert(id.clone()) { selected_edges.push(id.clone()); }
                    if !depths.contains_key(target) {
                        depths.insert(target.to_owned(), depth + 1);
                        queue.push_back((target.to_owned(), depth + 1));
                    }
                }
            }
            let mut ordered: Vec<_> = depths.into_iter().collect();
            ordered.sort_by(|a, b| (a.1, &a.0).cmp(&(b.1, &b.0)));
            let mut selected_nodes = Vec::with_capacity(ordered.len());
            for (id, depth) in ordered {
                work.step(1)?;
                let JsonValue::Object(mut fields) = nodes.remove(&id)
                    .ok_or_else(|| corrupt("selected descent node absent"))? else {
                    return Err(corrupt("selected descent node is not object"));
                };
                if fields.iter().any(|(key, _)| key.as_str() == Some("depth")) {
                    return Err(corrupt("selected source node owns reserved depth field"));
                }
                fields.push((tos_foundation::JsonString::from_utf8("depth"), descent_number(depth as usize)));
                selected_nodes.push(JsonValue::Object(fields));
            }
            let mut result_edges = Vec::with_capacity(selected_edges.len());
            for id in selected_edges {
                work.step(1)?;
                result_edges.push(edges.remove(&id).ok_or_else(|| corrupt("selected descent edge absent"))?);
            }
            let authority_note = take_field(header, "authority_boundary")?;
            if authority_note.as_str().is_none_or(str::is_empty) {
                return Err(corrupt("source-navigation authority boundary must be nonempty string"));
            }
            Ok(object(vec![
                ("schema", text("tos_source_descent_v1")), ("root_id", text(&request.node_id)),
                ("max_depth", descent_number(request.max_depth as usize)), ("limit", descent_number(request.limit)),
                ("truncated", JsonValue::Bool(truncated)),
                ("counts", object(vec![("nodes", descent_number(selected_nodes.len())), ("edges", descent_number(result_edges.len()))])),
                ("nodes", JsonValue::Array(selected_nodes)), ("edges", JsonValue::Array(result_edges)),
                ("authority_note", authority_note),
            ]))
        })
}

fn descent_number(value: usize) -> JsonValue {
    JsonValue::Number(tos_foundation::JsonNumber { kind: tos_foundation::JsonNumberKind::Int, lexeme: value.to_string() })
}

fn take_field(value: JsonValue, field: &str) -> Result<JsonValue, SearchV2Error> {
    let JsonValue::Object(entries) = value else {
        return Err(corrupt("selected original envelope is not an object"));
    };
    entries.into_iter().find_map(|(key, value)|
        (key.as_str() == Some(field)).then_some(value)
    ).ok_or_else(|| corrupt("selected original envelope field absent"))
}

fn load_original_navigation<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    bound: &BoundCmpKnowledge<'_>,
    budget: DossierBudget,
    max_retained_bytes: Option<usize>,
) -> Result<(JsonValue, BTreeMap<String, JsonValue>, BTreeMap<String, JsonValue>, Vec<JsonValue>), SearchV2Error> {
            let receipt = read.original_receipt()?;
            let source = bound
                .source_for_adapter("source-navigation-node-edge-v1")
                .ok_or_else(|| {
                    err(
                        SearchV2ErrorCode::Unavailable,
                        "selected navigation source unavailable",
                    )
                })?;
            if receipt.profile != tos_compiler::NAVIGATION_ORIGINAL_PROFILE
                || receipt.source_graph != source
                || receipt.descriptor_sha256
                    != bound.selection().vocabulary.descriptor_sha256.to_hex()
                || receipt.source_cut != bound.selection().source_cut
                || receipt.membership_root != bound.selection().source_membership_root.to_hex()
            {
                return Err(corrupt("selected dossier original binding differs"));
            }
            let navigation_state_base = if let Some(cap) = max_retained_bytes {
                // The receipt total covers header/rights; member_index_bytes
                // covers metadata, not node/edge raw bodies. Price those bodies
                // from the authenticated member raw sizes below, before loading.
                let original = receipt.total_bytes.checked_add(receipt.member_index_bytes)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation retained byte range"))?;
                let members = receipt.nodes.checked_add(receipt.edges)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation retained member range"))?;
                let sources = [source.to_owned()];
                let candidates = read.scope_count(SearchKind::Nodes, &sources)?
                    .checked_add(read.scope_count(SearchKind::Relations, &sources)?)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation candidate count range"))?;
                if candidates > budget.max_candidates {
                    return Err(err(SearchV2ErrorCode::BudgetExceeded, "navigation candidate cap exceeded"));
                }
                // ObservedInspectCarrier has two owned bounded strings. Price
                // vector capacity growth and the source/id candidate-page tuple.
                let observed_slot = budget.inspect.max_field_bytes.checked_mul(2)
                    .and_then(|n| n.checked_add(std::mem::size_of::<crate::knowledge_inspect::ObservedInspectCarrier>()))
                    .and_then(|n| n.checked_mul(2))
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation observed state overflow"))?;
                let page_slot = budget.inspect.max_field_bytes.checked_mul(2)
                    .and_then(|n| n.checked_add(std::mem::size_of::<(String, i64, String)>()))
                    .and_then(|n| n.checked_mul(2))
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation page state overflow"))?;
                // A conservative logical DOM/index/controller/BFS/encoding
                // ceiling, not RSS or an allocator-fit assertion.
                let state = original.checked_mul(256)
                    .and_then(|n| n.checked_add(budget.inspect.max_payload_bytes.checked_mul(256)?))
                    .and_then(|n| n.checked_add(members.checked_mul(512)?))
                    .and_then(|n| n.checked_add(candidates.checked_mul(observed_slot)?))
                    .and_then(|n| n.checked_add(budget.block_size.checked_mul(page_slot)?))
                    .and_then(|n| n.checked_add(budget.inspect.max_response_bytes.checked_mul(4)?))
                    .and_then(|n| n.checked_add(4096))
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation retained state overflow"))?;
                if cap == 0 || state > cap {
                    return Err(err(SearchV2ErrorCode::BudgetExceeded, "navigation retained state exceeds owner allowance"));
                }
                Some(state)
            } else { None };
            let (ordinal, header) = read
                .original_row(&receipt, None)?
                .ok_or_else(|| corrupt("selected original header absent"))?;
            if ordinal != -1 {
                return Err(corrupt("selected original header ordinal differs"));
            }
            let mut rights = Vec::new();
            let mut after = Some(-1);
            while let Some((ordinal, row)) = read.original_row(&receipt, after)? {
                if ordinal != rights.len() as i64 || rights.len() as u64 >= receipt.rights {
                    return Err(corrupt("selected original rights order differs"));
                }
                after = Some(ordinal);
                rights.push(row);
            }
            if rights.len() as u64 != receipt.rights {
                return Err(corrupt("selected original rights incomplete"));
            }
            let node_members = members(read, "nodes", receipt.nodes)?;
            let edge_members = members(read, "edges", receipt.edges)?;
            if let Some((base, cap)) = navigation_state_base.zip(max_retained_bytes) {
                let raw_bytes = node_members.values().chain(edge_members.values())
                    .try_fold(0u64, |sum, member| sum.checked_add(member.raw_bytes))
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation raw member byte range"))?;
                let state = raw_bytes.checked_mul(256).and_then(|n| n.checked_add(base))
                    .ok_or_else(|| err(SearchV2ErrorCode::BudgetExceeded, "navigation original DOM state overflow"))?;
                if state > cap {
                    return Err(err(SearchV2ErrorCode::BudgetExceeded, "navigation original DOM exceeds owner allowance"));
                }
            }
            let mut candidate_work = 0;
            let nodes = original_payloads(
                read,
                source,
                SearchKind::Nodes,
                &node_members,
                budget,
                &mut candidate_work,
            )?;
            let edges = original_payloads(
                read,
                source,
                SearchKind::Relations,
                &edge_members,
                budget,
                &mut candidate_work,
            )?;
    Ok((header, nodes, edges, rights))
}

const KINDS: &[&str] = &[
    "branch",
    "era",
    "region",
    "tradition",
    "source_planting",
    "work",
    "expression",
    "edition",
    "item",
    "file",
    "link",
];
const BIB: &[&str] = &["has_expression", "embodied_by", "exemplified_by"];
const LINKS: &[&str] = &[
    "described_by",
    "metadata_at",
    "downloadable_at",
    "rights_statement_at",
];
fn get<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue {
    v.object_get(key).unwrap_or(&JsonValue::Null)
}
fn s(v: &JsonValue) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &JsonValue) -> &[JsonValue] {
    v.as_array().unwrap_or(&[])
}
fn strings(v: &JsonValue) -> BTreeSet<String> {
    arr(v)
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn texts(v: impl IntoIterator<Item = String>) -> JsonValue {
    JsonValue::Array(v.into_iter().map(|v| text(&v)).collect())
}
fn err(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn kind(node: &JsonValue) -> &str {
    s(get(node, "node_kind"))
}
fn positive(record: &JsonValue) -> bool {
    ["licensed", "public_domain_reviewed"].contains(&s(get(record, "assessment_status")))
        && ["authorized", "authorized_with_conditions"]
            .contains(&s(get(record, "redistribution_posture")))
}
fn reviewed(record: &JsonValue) -> bool {
    positive(record)
        && ["accepted", "accepted_with_limits"].contains(&s(get(record, "review_status")))
}
fn root_id(id: &str) -> bool {
    id.strip_prefix("tos.rights.").is_some_and(|body| {
        !body.is_empty()
            && body.split(['.', '-']).all(|p| {
                !p.is_empty()
                    && p.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
    })
}
fn layer_id(id: &str) -> bool {
    root_id(id)
        && id.strip_prefix("tos.rights.").is_some_and(|v| {
            v.split_once(".layer.")
                .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
        })
}
struct Work<'a> {
    left: u64,
    interrupt: &'a mut dyn FnMut() -> Result<(), SearchV2Error>,
}
impl Work<'_> {
    fn step(&mut self, n: usize) -> Result<(), SearchV2Error> {
        self.left = self.left.checked_sub(n as u64).ok_or_else(|| {
            err(
                SearchV2ErrorCode::BudgetExceeded,
                "dossier work cap exceeded",
            )
        })?;
        (self.interrupt)()
    }
}
fn aggregate<'a>(
    records: &[&'a JsonValue],
    w: &mut Work<'_>,
) -> Result<Vec<&'a JsonValue>, SearchV2Error> {
    let mut groups: BTreeMap<&str, Vec<&JsonValue>> = BTreeMap::new();
    for r in records {
        w.step(1)?;
        let source = s(get(r, "source_ref"));
        if !source.is_empty() {
            groups.entry(source).or_default().push(r);
        }
    }
    let mut out = vec![];
    for rs in groups.values() {
        w.step(rs.len())?;
        if rs.iter().any(|r| r.object_get("assessment_kind").is_some()) {
            if rs
                .iter()
                .any(|r| !["aggregate", "layer"].contains(&s(get(r, "assessment_kind"))))
            {
                continue;
            }
            let roots: Vec<_> = rs
                .iter()
                .filter(|r| s(get(r, "assessment_kind")) == "aggregate")
                .copied()
                .collect();
            if roots.len() == 1 {
                out.extend(roots);
            }
            continue;
        }
        let roots: Vec<_> = rs
            .iter()
            .filter(|r| root_id(s(get(r, "rights_id"))) && !layer_id(s(get(r, "rights_id"))))
            .copied()
            .collect();
        let layers = rs
            .iter()
            .filter(|r| layer_id(s(get(r, "rights_id"))))
            .count();
        if roots.len() == 1 && roots.len() + layers == rs.len() {
            out.extend(roots);
        }
    }
    Ok(out)
}
#[derive(Default)]
struct Membership {
    item: String,
    rights: BTreeSet<String>,
    manifests: BTreeSet<String>,
    legacy: bool,
    valid: bool,
}
fn membership(
    edge: &JsonValue,
    strict: bool,
    w: &mut Work<'_>,
) -> Result<Membership, SearchV2Error> {
    let refs = get(edge, "source_refs");
    let sources = strings(refs);
    let mut m = Membership {
        item: s(get(edge, "from_id")).into(),
        valid: refs.as_array().is_some() && sources.len() == arr(refs).len() && !sources.is_empty(),
        ..Membership::default()
    };
    m.manifests = sources.clone();
    let properties = get(edge, "properties");
    if properties.object_get("item_file_contexts").is_none() {
        m.legacy = true;
        m.valid &= sources.len() == 1;
        return Ok(m);
    }
    let contexts = get(properties, "item_file_contexts");
    let mut manifests = BTreeSet::new();
    if contexts.as_array().is_none() || arr(contexts).is_empty() {
        m.valid = false;
    }
    for c in arr(contexts) {
        w.step(1)?;
        if c.as_object().is_none() {
            m.valid = false;
            continue;
        }
        let manifest = s(get(c, "manifest_ref"));
        if manifest.is_empty()
            || !sources.contains(manifest)
            || !manifests.insert(manifest.to_owned())
        {
            m.valid = false;
        }
        let right = get(c, "rights_ref");
        let right_text = s(right);
        if !right_text.is_empty() {
            m.rights.insert(right_text.into());
        } else if !strict || matches!(right, JsonValue::Null) || right.as_str() == Some("") {
            m.legacy = true;
        } else {
            m.valid = false;
        }
    }
    if manifests != sources {
        m.valid = false;
    }
    if strict && (m.rights.len() > 1 || !m.rights.is_empty() && m.legacy) {
        m.valid = false;
    }
    Ok(m)
}
fn scopes(record: &JsonValue, w: &mut Work<'_>) -> Result<BTreeSet<String>, SearchV2Error> {
    w.step(arr(get(record, "scope_refs")).len())?;
    Ok(strings(get(record, "scope_refs")))
}
fn file_edge(e: &JsonValue) -> bool {
    s(get(e, "edge_kind")) == "authored_item_manifest" && s(get(e, "predicate_id")) == "has_file"
}
fn file_filter<'a>(
    rights: Vec<&'a JsonValue>,
    ids: &BTreeSet<String>,
    nodes: &BTreeMap<String, JsonValue>,
    edges: &BTreeMap<String, &JsonValue>,
    incoming: &BTreeMap<String, Vec<&JsonValue>>,
    w: &mut Work<'_>,
) -> Result<Vec<&'a JsonValue>, SearchV2Error> {
    let files: BTreeSet<_> = ids
        .iter()
        .filter(|id| nodes.get(*id).is_some_and(|n| kind(n) == "file"))
        .cloned()
        .collect();
    if files.is_empty() {
        return Ok(rights);
    }
    let bib: BTreeSet<_> = ids
        .iter()
        .filter(|id| {
            nodes
                .get(*id)
                .is_some_and(|n| ["work", "expression", "edition"].contains(&kind(n)))
        })
        .cloned()
        .collect();
    let mut members: BTreeMap<String, Vec<Membership>> = BTreeMap::new();
    for e in edges.values() {
        w.step(1)?;
        if !file_edge(e) {
            continue;
        }
        let item = s(get(e, "from_id"));
        let file = s(get(e, "to_id"));
        if !ids.contains(item)
            || !nodes.get(item).is_some_and(|n| kind(n) == "item")
            || !files.contains(file)
        {
            continue;
        }
        members
            .entry(file.into())
            .or_default()
            .push(membership(e, true, w)?);
    }
    for ms in members.values_mut() {
        let mut counts = BTreeMap::new();
        for m in ms.iter() {
            *counts.entry(m.item.clone()).or_insert(0usize) += 1;
        }
        for m in ms {
            if counts[&m.item] > 1 {
                m.valid = false;
            }
        }
    }
    let mut out = vec![];
    for r in &rights {
        w.step(1)?;
        let scope = scopes(r, w)?;
        let file_scopes: Vec<_> = scope.intersection(&files).collect();
        if file_scopes.is_empty() || !scope.is_disjoint(&bib) {
            out.push(*r);
            continue;
        }
        let source = s(get(r, "source_ref"));
        let mut every = true;
        for file in file_scopes {
            let mut bound = false;
            for m in members.get(file).into_iter().flatten() {
                w.step(1)?;
                if !m.valid {
                    continue;
                }
                if !m.rights.is_empty() {
                    if m.rights.contains(source) {
                        bound = true;
                        break;
                    }
                } else if m.legacy && scope.contains(&m.item) {
                    let owner: Vec<_> = incoming
                        .get(file)
                        .into_iter()
                        .flatten()
                        .filter(|e| file_edge(e))
                        .collect();
                    if owner.len() != 1 || s(get(owner[0], "from_id")) != m.item {
                        continue;
                    }
                    let mut refs = BTreeSet::new();
                    let mut valid = true;
                    for other in &rights {
                        w.step(1)?;
                        let other_scope = scopes(other, w)?;
                        if other_scope.contains(file) && other_scope.contains(&m.item) {
                            let sr = s(get(other, "source_ref"));
                            if sr.is_empty() {
                                valid = false;
                            } else {
                                refs.insert(sr);
                            }
                        }
                    }
                    if valid && refs.len() == 1 && refs.contains(source) {
                        bound = true;
                        break;
                    }
                }
            }
            if !bound {
                every = false;
                break;
            }
        }
        if every {
            out.push(*r);
        }
    }
    Ok(out)
}
fn admit(
    id: &str,
    nodes: &BTreeMap<String, JsonValue>,
    ids: &mut BTreeSet<String>,
    limit: usize,
    truncated: &mut bool,
) -> bool {
    if ids.contains(id) {
        return true;
    }
    if !nodes.contains_key(id) {
        return false;
    }
    if ids.len() >= limit {
        *truncated = true;
        return false;
    }
    ids.insert(id.into());
    true
}
/// Private kernel. Only the selected wrapper may turn this value into a held
/// packet; caller supplies authenticated exact original carriers and checks.
pub(crate) fn compute_dossier(
    header: &JsonValue,
    nodes: &BTreeMap<String, JsonValue>,
    all_edges: &[JsonValue],
    all_rights: &[JsonValue],
    object_id: &str,
    limit: usize,
    max_steps: u64,
    interrupt: &mut dyn FnMut() -> Result<(), SearchV2Error>,
) -> Result<JsonValue, SearchV2Error> {
    let mut w = Work {
        left: max_steps,
        interrupt,
    };
    w.step(1)?;
    let selected = nodes.get(object_id).ok_or_else(|| {
        err(
            SearchV2ErrorCode::UnknownIdentifier,
            "unknown ToS dossier object",
        )
    })?;
    let selected_kind = kind(selected);
    if !["work", "expression", "edition", "item", "file", "link"].contains(&selected_kind) {
        return Err(err(
            SearchV2ErrorCode::InvalidRequest,
            "dossier object kind unsupported",
        ));
    }
    let mut ordered: Vec<_> = all_edges.iter().collect();
    ordered.sort_by_key(|e| s(get(e, "edge_id")));
    let mut incoming: BTreeMap<String, Vec<&JsonValue>> = BTreeMap::new();
    let mut semantic: BTreeMap<String, Vec<&JsonValue>> = BTreeMap::new();
    for e in ordered {
        w.step(1)?;
        incoming
            .entry(s(get(e, "to_id")).into())
            .or_default()
            .push(e);
        if s(get(e, "edge_kind")) == "authored_item_manifest"
            || s(get(e, "edge_kind")) == "evidence_claim"
                && (BIB.contains(&s(get(e, "predicate_id")))
                    || LINKS.contains(&s(get(e, "predicate_id"))))
        {
            semantic
                .entry(s(get(e, "from_id")).into())
                .or_default()
                .push(e);
        }
    }
    let mut ids = BTreeSet::from([object_id.to_owned()]);
    let mut edges: BTreeMap<String, &JsonValue> = BTreeMap::new();
    let mut truncated = false;
    let mut roots = BTreeSet::new();
    if selected_kind == "work" {
        roots.insert(object_id.to_owned());
    } else {
        let mut q = VecDeque::from([object_id.to_owned()]);
        let mut visited = BTreeSet::new();
        while let Some(current) = q.pop_front() {
            w.step(1)?;
            if !visited.insert(current.clone()) {
                continue;
            }
            let current_kind = kind(&nodes[&current]);
            if current_kind == "work" {
                roots.insert(current);
                continue;
            }
            for e in incoming.get(&current).into_iter().flatten() {
                w.step(1)?;
                let structural =
                    current_kind == "file" && s(get(e, "edge_kind")) == "authored_item_manifest";
                let allowed = if current_kind == "link" { LINKS } else { BIB };
                if !structural
                    && (s(get(e, "edge_kind")) != "evidence_claim"
                        || !allowed.contains(&s(get(e, "predicate_id"))))
                {
                    continue;
                }
                let parent = s(get(e, "from_id"));
                if !admit(parent, nodes, &mut ids, limit, &mut truncated) {
                    continue;
                }
                edges.insert(s(get(e, "edge_id")).into(), e);
                q.push_back(parent.into());
            }
        }
        if roots.is_empty() {
            roots = ids
                .iter()
                .filter(|id| kind(&nodes[*id]) != "link")
                .cloned()
                .collect();
        }
    }
    let mut q: VecDeque<_> = roots.into_iter().collect();
    let mut visited = BTreeSet::new();
    while let Some(current) = q.pop_front() {
        w.step(1)?;
        if !visited.insert(current.clone()) {
            continue;
        }
        for e in semantic.get(&current).into_iter().flatten() {
            w.step(1)?;
            let target = s(get(e, "to_id"));
            if !admit(target, nodes, &mut ids, limit, &mut truncated) {
                continue;
            }
            edges.insert(s(get(e, "edge_id")).into(), e);
            q.push_back(target.into());
        }
    }
    let works: Vec<_> = ids
        .iter()
        .filter(|id| kind(&nodes[*id]) == "work")
        .cloned()
        .collect();
    let mut q = VecDeque::new();
    for work in works {
        for e in incoming.get(&work).into_iter().flatten() {
            w.step(1)?;
            if s(get(e, "edge_kind")) != "authored_source_planting" {
                continue;
            }
            let parent = s(get(e, "from_id"));
            if admit(parent, nodes, &mut ids, limit, &mut truncated) {
                edges.insert(s(get(e, "edge_id")).into(), e);
                q.push_back(parent.to_owned());
            }
        }
    }
    let mut visited = BTreeSet::new();
    while let Some(current) = q.pop_front() {
        w.step(1)?;
        if !visited.insert(current.clone()) {
            continue;
        }
        let current_kind = kind(&nodes[&current]);
        for e in incoming.get(&current).into_iter().flatten() {
            w.step(1)?;
            let branch = s(get(e, "edge_kind")) == "authored_branch_hierarchy";
            let planting = current_kind == "source_planting"
                && s(get(e, "edge_kind")) == "authored_source_planting"
                && s(get(e, "predicate_id")) == "has_source_planting";
            if !(branch || planting) {
                continue;
            }
            let parent = s(get(e, "from_id"));
            if admit(parent, nodes, &mut ids, limit, &mut truncated) {
                edges.insert(s(get(e, "edge_id")).into(), e);
                q.push_back(parent.into());
            }
        }
    }
    let component: Vec<_> = ids.iter().map(|id| &nodes[id]).collect();
    let mut paths = vec![];
    for era in component.iter().filter(|n| kind(n) == "era") {
        let era = s(get(era, "node_id"));
        let mut q = VecDeque::from([(era.to_owned(), vec![era.to_owned()], Vec::<String>::new())]);
        let mut seen = BTreeSet::from([era.to_owned()]);
        while let Some((current, np, ep)) = q.pop_front() {
            w.step(1)?;
            if current == object_id {
                paths.push(object(vec![
                    ("node_ids", texts(np)),
                    ("edge_ids", texts(ep)),
                ]));
                break;
            }
            for (id, e) in &edges {
                w.step(1)?;
                if s(get(e, "from_id")) != current {
                    continue;
                }
                let target = s(get(e, "to_id"));
                if !target.is_empty() && seen.insert(target.to_owned()) {
                    let mut np = np.clone();
                    np.push(target.into());
                    let mut ep = ep.clone();
                    ep.push(id.clone());
                    q.push_back((target.into(), np, ep));
                }
            }
        }
    }
    let mut rights = vec![];
    for r in all_rights {
        w.step(1)?;
        if r.as_object().is_some() && !scopes(r, &mut w)?.is_disjoint(&ids) {
            rights.push(r);
        }
    }
    if selected_kind != "file" {
        rights = file_filter(rights, &ids, nodes, &edges, &incoming, &mut w)?;
    }
    let mut decision_scope = BTreeSet::from([object_id.to_owned()]);
    let mut complete = false;
    let mut positives: BTreeMap<String, bool> = BTreeMap::new();
    let mut membership_gap = false;
    let decision_rights;
    if selected_kind == "link" {
        decision_scope = edges
            .values()
            .filter(|e| {
                s(get(e, "to_id")) == object_id && s(get(e, "edge_kind")) == "evidence_claim"
            })
            .map(|e| s(get(e, "from_id")).to_owned())
            .collect();
    }
    if selected_kind == "file" {
        let mut members: BTreeMap<String, Membership> = BTreeMap::new();
        let owner_edges: Vec<_> = incoming
            .get(object_id)
            .into_iter()
            .flatten()
            .filter(|e| file_edge(e) && s(get(e, "to_id")) == object_id)
            .collect();
        let mut valid = !owner_edges.is_empty();
        let mut represented = BTreeSet::new();
        for e in owner_edges {
            w.step(1)?;
            let mut m = membership(e, false, &mut w)?;
            if m.item.is_empty() || !nodes.get(&m.item).is_some_and(|n| kind(n) == "item") {
                valid = false;
                continue;
            }
            valid &= m.valid;
            represented.extend(m.manifests.iter().cloned());
            if let Some(old) = members.get_mut(&m.item) {
                valid = false;
                old.rights.append(&mut m.rights);
                old.legacy |= m.legacy;
            } else {
                members.insert(m.item.clone(), m);
            }
        }
        if selected.object_get("source_refs").is_some() {
            let raw = get(selected, "source_refs");
            let refs = strings(raw);
            if raw.as_array().is_none()
                || refs.is_empty()
                || refs.len() != arr(raw).len()
                || refs != represented
            {
                valid = false;
            }
        }
        let member_ids: BTreeSet<_> = members.keys().cloned().collect();
        decision_scope = member_ids.clone();
        decision_scope.insert(object_id.to_owned());
        complete = !member_ids.is_empty() && member_ids.is_subset(&ids);
        valid &= complete;
        let legacy_single = member_ids.len() == 1;
        let mut bound: BTreeMap<String, &JsonValue> = BTreeMap::new();
        for (item, m) in members {
            w.step(1)?;
            if m.rights.len() > 1
                || !m.rights.is_empty() && m.legacy
                || m.rights.is_empty() && !(legacy_single && m.legacy)
            {
                valid = false;
                positives.insert(item, false);
                continue;
            }
            let mut associated = vec![];
            for r in &rights {
                w.step(1)?;
                let scope = scopes(r, &mut w)?;
                if !m.rights.is_empty() {
                    if !m.rights.contains(s(get(r, "source_ref")))
                        || !scope.contains(&item) && !scope.contains(object_id)
                    {
                        continue;
                    }
                } else if !scope.contains(&item) || !scope.contains(object_id) {
                    continue;
                }
                associated.push(*r);
            }
            if m.rights.is_empty() {
                let sources: BTreeSet<_> = associated
                    .iter()
                    .map(|r| s(get(r, "source_ref")))
                    .filter(|s| !s.is_empty())
                    .collect();
                if sources.len() != 1
                    || associated
                        .iter()
                        .any(|r| s(get(r, "source_ref")).is_empty())
                {
                    valid = false;
                    positives.insert(item, false);
                    continue;
                }
            }
            if associated.is_empty() {
                valid = false;
            }
            let accepted = aggregate(&associated, &mut w)?.iter().any(|r| reviewed(r));
            positives.insert(item, accepted);
            for r in associated {
                bound.insert(s(get(r, "rights_id")).into(), r);
            }
        }
        rights = bound.into_values().collect();
        decision_rights = rights.clone();
        complete &= valid;
        membership_gap = !complete;
    } else {
        decision_rights = rights
            .iter()
            .filter(|r| !strings(get(r, "scope_refs")).is_disjoint(&decision_scope))
            .copied()
            .collect::<Vec<_>>();
        if selected_kind == "item" {
            rights = decision_rights.clone();
        }
    }
    let links: Vec<_> = if selected_kind == "link" {
        vec![selected]
    } else {
        component
            .iter()
            .filter(|n| kind(n) == "link")
            .copied()
            .collect()
    };
    let statuses: BTreeSet<_> = links
        .iter()
        .map(|n| s(get(get(n, "properties"), "access_status")))
        .collect();
    let technical = if statuses.contains("open_download") {
        "downloadable"
    } else if statuses.contains("open_view") {
        "viewable"
    } else if statuses.contains("metadata_only") {
        "metadata_only"
    } else if statuses
        .iter()
        .any(|s| ["restricted", "login_required", "unavailable"].contains(s))
    {
        "restricted_or_unavailable"
    } else {
        "unknown"
    };
    let aggregates = aggregate(&decision_rights, &mut w)?;
    let positive = aggregates.iter().any(|r| positive(r));
    let reviewed = aggregates.iter().any(|r| reviewed(r));
    let all_members = selected_kind == "file"
        && complete
        && !positives.is_empty()
        && positives.values().all(|v| *v);
    let posture = if selected_kind == "file" {
        if all_members {
            "reviewed_reuse_route"
        } else if membership_gap || positives.values().any(|v| *v) {
            "membership_scoped_review_required"
        } else if positive {
            "candidate_requires_human_review"
        } else if !decision_rights.is_empty() {
            "not_cleared"
        } else {
            "unknown"
        }
    } else if reviewed {
        "reviewed_reuse_route"
    } else if positive {
        "candidate_requires_human_review"
    } else if !decision_rights.is_empty() {
        "not_cleared"
    } else {
        "unknown"
    };
    let mut gaps = vec![];
    if decision_rights.is_empty() {
        gaps.push("no associated public rights record");
    } else if aggregates.is_empty() {
        gaps.push("no unambiguous aggregate rights assessment");
    }
    if positive && !reviewed {
        gaps.push("positive rights route exists but has no accepted human review");
    }
    if membership_gap {
        gaps.push("File membership or its exact rights binding is incomplete");
    } else if selected_kind == "file" && !all_members {
        gaps.push("not every exact Item membership has an accepted positive rights route");
    }
    if !component.iter().any(|n| kind(n) == "link") {
        gaps.push("no first-class associated Link record");
    }
    let mut refs = BTreeSet::new();
    for n in &component {
        let r = s(get(n, "source_ref"));
        if !r.is_empty() {
            refs.insert(r.to_owned());
        }
    }
    for e in edges.values() {
        refs.extend(strings(get(e, "source_refs")));
    }
    for r in &rights {
        if let Some(r) = get(r, "source_ref").as_str() {
            refs.insert(r.to_owned());
        }
    }
    rights.sort_by_key(|r| s(get(r, "rights_id")));
    let chain = JsonValue::Object(
        KINDS
            .iter()
            .map(|k| {
                (
                    tos_foundation::JsonString::from_utf8(k),
                    JsonValue::Array(
                        component
                            .iter()
                            .filter(|n| kind(n) == *k)
                            .map(|n| (*n).clone())
                            .collect(),
                    ),
                )
            })
            .collect(),
    );
    let open = if selected_kind == "file" {
        all_members
    } else {
        reviewed
    };
    w.step(1)?;
    Ok(object(vec![
        ("schema", text("tos_source_dossier_v1")),
        ("object_id", text(object_id)),
        ("object", selected.clone()),
        (
            "agent_summary",
            object(vec![
                ("technical_access", text(technical)),
                ("rights_posture", text(posture)),
                ("human_review_required", JsonValue::Bool(!open)),
                ("can_conclude_legal_openness", JsonValue::Bool(open)),
                ("availability_is_license", JsonValue::Bool(false)),
                ("rights_scope_refs", texts(decision_scope)),
                ("gaps", texts(gaps.into_iter().map(str::to_owned))),
            ]),
        ),
        ("chain", chain),
        ("tree_paths", JsonValue::Array(paths)),
        (
            "relations",
            JsonValue::Array(edges.into_values().cloned().collect()),
        ),
        (
            "rights",
            JsonValue::Array(rights.into_iter().cloned().collect()),
        ),
        ("source_refs", texts(refs)),
        ("truncated", JsonValue::Bool(truncated)),
        ("authority_note", get(header, "authority_boundary").clone()),
    ]))
}
