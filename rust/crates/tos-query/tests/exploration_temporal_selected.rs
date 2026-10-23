#![cfg(all(not(target_arch = "wasm32"), feature = "native-models"))]
//! Native producer and exact selected reads with explicit owner expectations.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tos_compiler::knowledge_full_fixture::build_native_fixture;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonString, JsonValue, canonical_bytes_v1,
    parse_json,
};
use tos_query::knowledge_exploration::{
    EXPLORATION_INTENDED_USE, EXPLORATION_OPERATION, ExplorationBudget, ExplorationCheckpoint,
    ExplorationCheckpoints, ExplorationState, PreparedExplorationCheckpoint,
    execute_selected_exploration,
};
use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode};
use tos_query::{
    AbortProbe, BoundCmpKnowledge, IndexedDisclosureScope, InspectBudget, InspectCurrentAuthority,
    InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier, bind_verified_knowledge,
    compare_temporal_operands, execute_selected_temporal,
};
fn get<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue {
    v.object_get(key).unwrap()
}
fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn object(rows: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        rows.into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn set(v: &mut JsonValue, key: &str, value: JsonValue) {
    let JsonValue::Object(o) = v else {
        panic!("object")
    };
    if let Some((_, v)) = o.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *v = value
    } else {
        o.push((JsonString::from_utf8(key), value))
    }
}
fn parse(bytes: &[u8]) -> JsonValue {
    parse_json(bytes, JsonMode::PublishedStrict, JsonLimits::default())
        .unwrap()
        .into_root()
}
fn canonical(v: &JsonValue) -> Vec<u8> {
    canonical_bytes_v1(
        v,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap()
}
// Packets sort object keys; source-field equality still preserves every scalar
// (including numeric kind and lexeme), array position, and unique object member.
fn same_json_fields(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Object(left), JsonValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    left.iter().filter(|(other, _)| other == key).count() == 1
                        && right.iter().filter(|(other, _)| other == key).count() == 1
                        && right
                            .iter()
                            .find(|(other, _)| other == key)
                            .is_some_and(|(_, other)| same_json_fields(value, other))
                })
        }
        (JsonValue::Array(left), JsonValue::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(a, b)| same_json_fields(a, b))
        }
        _ => left == right,
    }
}
fn err(code: SearchV2ErrorCode) -> SearchV2Error {
    SearchV2Error {
        code,
        message: "synthetic checkpoint admission",
    }
}
struct Lease;
impl InspectDisclosureLease for Lease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        Ok(())
    }
}
struct Authority {
    scope: IndexedDisclosureScope,
    policy: CurrentPolicyBinding,
    withdrawn: bool,
    probe: Option<Arc<dyn AbortProbe>>,
}
impl Authority {
    fn new(bound: &BoundCmpKnowledge<'_>, operation: &str, intended: &str) -> Self {
        let policy = CurrentPolicyBinding {
            scope: "synthetic-query".into(),
            issuer_ref: "synthetic-issuer".into(),
            authorization_receipt_id: "synthetic-receipt".into(),
            policy_epoch: "synthetic-epoch".into(),
            withdrawal_generation: "synthetic-generation".into(),
        };
        Self {
            scope: IndexedDisclosureScope {
                operation_id: operation.into(),
                carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                intended_use: intended.into(),
                selected_model_receipt_id: bound.owner_receipt_id().into(),
                source_cut: bound.selection().source_cut.clone(),
                through_commit_seq: bound.selection().through_commit_seq,
                source_membership_root: bound.selection().source_membership_root,
                descriptor_sha256: bound.selection().vocabulary.descriptor_sha256,
                selected_index_sha256: bound.selection().index_root_sha256,
                policy_issuer_ref: policy.issuer_ref.clone(),
                policy_receipt_id: policy.authorization_receipt_id.clone(),
                policy_scope: policy.scope.clone(),
                policy_epoch: policy.policy_epoch.clone(),
                withdrawal_generation: policy.withdrawal_generation.clone(),
            },
            policy,
            withdrawn: false,
            probe: None,
        }
    }
}
impl<'hold> InspectCurrentAuthority<'hold> for Authority {
    fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
        self.probe.clone()
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.scope.clone()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        Ok(())
    }
    fn authorize_current(&mut self, _: &InspectedCarrier) -> Result<(), SearchV2Error> {
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        _: &IndexedDisclosureScope,
        _: &[ObservedInspectCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
        if self.withdrawn {
            Err(err(SearchV2ErrorCode::StalePolicy))
        } else {
            Ok(Box::new(Lease))
        }
    }
}
fn read_budget() -> InspectBudget {
    InspectBudget {
        max_open_vm_steps: 100_000_000,
        max_read_vm_steps: 1_000_000,
        max_matches: 64,
        max_rows: 1000,
        max_field_bytes: 16384,
        max_payload_bytes: 1_000_000,
        max_decoded_bytes: 8_000_000,
        max_response_bytes: 1_000_000,
        json: JsonLimits::default(),
    }
}
fn exploration_budget(work: usize) -> ExplorationBudget {
    ExplorationBudget {
        read: read_budget(),
        max_work_units: work,
        max_session_nodes: 10000,
        max_session_relations: 20000,
        max_state_bytes: 1_000_000,
        max_checkpoint_bytes: 2_000_000,
        max_checkpoints: 128,
    }
}
#[derive(Clone)]
enum Stored {
    State(ExplorationState),
    Replay(JsonValue, Digest256),
}
#[derive(Default)]
struct Store {
    rows: BTreeMap<String, (String, Stored)>,
    ordinal: usize,
    refuse: bool,
    commits: usize,
}
#[derive(Default)]
struct Checkpoints(Arc<Mutex<Store>>);
struct Staged {
    store: Arc<Mutex<Store>>,
    input: Option<String>,
    revision: String,
    next: Option<String>,
    state: Option<ExplorationState>,
    packet: JsonValue,
    state_bytes: usize,
    max_bytes: usize,
    body_sha: Option<Digest256>,
}
impl PreparedExplorationCheckpoint for Staged {
    fn next_cursor(&self) -> Option<&str> {
        self.next.as_deref()
    }
    fn stage_response(&mut self, body: &[u8]) -> Result<(), SearchV2Error> {
        if self.body_sha.is_some()
            || self
                .state_bytes
                .checked_add(body.len())
                .is_none_or(|bytes| bytes > self.max_bytes)
        {
            return Err(err(SearchV2ErrorCode::BudgetExceeded));
        }
        self.body_sha = Some(Digest256::of_bytes(body));
        Ok(())
    }
    fn commit(&mut self) -> Result<(), SearchV2Error> {
        if self.body_sha.is_none() {
            return Err(err(SearchV2ErrorCode::CorruptSelectedCarrier));
        }
        let mut store = self.store.lock().unwrap();
        if store.refuse {
            return Err(err(SearchV2ErrorCode::BudgetExceeded));
        }
        if let Some(input) = &self.input {
            store.rows.insert(
                input.clone(),
                (
                    self.revision.clone(),
                    Stored::Replay(
                        self.packet.clone(),
                        self.body_sha
                            .ok_or(err(SearchV2ErrorCode::CorruptSelectedCarrier))?,
                    ),
                ),
            );
        }
        if let Some(next) = &self.next {
            store.rows.insert(
                next.clone(),
                (
                    self.revision.clone(),
                    Stored::State(self.state.clone().unwrap()),
                ),
            );
        }
        store.commits += 1;
        Ok(())
    }
}
impl ExplorationCheckpoints for Checkpoints {
    fn load(
        &mut self,
        cursor: &str,
        revision: &str,
    ) -> Result<ExplorationCheckpoint, SearchV2Error> {
        let store = self.0.lock().unwrap();
        let (bound, row) = store
            .rows
            .get(cursor)
            .ok_or(err(SearchV2ErrorCode::CursorExpired))?;
        if bound != revision {
            return Err(err(SearchV2ErrorCode::StaleContinuation));
        }
        Ok(match row {
            Stored::State(v) => ExplorationCheckpoint::State(v.clone()),
            Stored::Replay(packet, sha) => ExplorationCheckpoint::Replay {
                packet: packet.clone(),
                packet_sha256: *sha,
            },
        })
    }
    fn prepare(
        &mut self,
        input: Option<&str>,
        revision: &str,
        successor: Option<&ExplorationState>,
        packet: &JsonValue,
        budget: ExplorationBudget,
    ) -> Result<Box<dyn PreparedExplorationCheckpoint>, SearchV2Error> {
        let mut store = self.0.lock().unwrap();
        if store.refuse {
            return Err(err(SearchV2ErrorCode::BudgetExceeded));
        }
        let mut limits = budget.read.json;
        limits.max_bytes = limits.max_bytes.min(budget.max_state_bytes);
        let state_bytes = successor
            .map(|state| state.encoded_state_count(limits))
            .transpose()?
            .unwrap_or(0);
        if state_bytes > budget.max_checkpoint_bytes {
            return Err(err(SearchV2ErrorCode::BudgetExceeded));
        }
        store.ordinal += 1;
        let next = successor.map(|_| format!("{:064x}", store.ordinal));
        let mut packet = packet.clone();
        let mut page = get(&packet, "page").clone();
        set(
            &mut page,
            "next_cursor",
            next.as_deref().map_or(JsonValue::Null, text),
        );
        set(&mut packet, "page", page);
        Ok(Box::new(Staged {
            store: self.0.clone(),
            input: input.map(str::to_owned),
            revision: revision.into(),
            next,
            state: successor.cloned(),
            packet,
            state_bytes,
            max_bytes: budget.max_checkpoint_bytes,
            body_sha: None,
        }))
    }
}
fn comparable(mut packet: JsonValue) -> JsonValue {
    let JsonValue::Object(o) = &mut packet else {
        panic!("packet")
    };
    o.retain(|(k, _)| k.as_str() != Some("snapshot_revision"));
    let mut page = get(&packet, "page").clone();
    set(&mut page, "next_cursor", JsonValue::Null);
    set(&mut packet, "page", page);
    packet
}
fn cursor_request(cursor: &str) -> JsonValue {
    JsonValue::Object(vec![(JsonString::from_utf8("cursor"), text(cursor))])
}
fn selected_temporal_request(
    source_revision: &str,
    left_id: &str,
    left_content_revision: &str,
    right_id: &str,
    right_content_revision: &str,
) -> JsonValue {
    object(vec![
        ("schema_version", text("tos_temporal_comparison_request_v1")),
        ("source_revision", text(source_revision)),
        (
            "left",
            object(vec![
                ("node_id", text(left_id)),
                ("content_revision", text(left_content_revision)),
            ]),
        ),
        (
            "right",
            object(vec![
                ("node_id", text(right_id)),
                ("content_revision", text(right_content_revision)),
            ]),
        ),
    ])
}

fn mapped_type() -> JsonValue {
    object(vec![("status", text("mapped"))])
}

#[test]
fn genuine_native_temporal_and_exploration_preserve_checkpoint_admission() {
    let fixture = build_native_fixture();
    let graph = parse(&fixture.graph_input_bytes);
    let source_revision = get(&graph, "source_revision").as_str().unwrap();
    let graph_nodes = get(&graph, "nodes").as_array().unwrap();
    let graph_relations = get(&graph, "relations").as_array().unwrap();
    let claim_nodes = graph_nodes
        .iter()
        .filter(|node| {
            get(node, "kind_id").as_str() == Some("claim")
                && get(node, "type_id").as_str() == Some("tos.entity.claim")
        })
        .collect::<Vec<_>>();
    assert!(
        !claim_nodes.is_empty(),
        "native fixture must contain a source Claim"
    );
    assert_eq!(
        get(get(claim_nodes[0], "attributes"), "source_claim")
            .object_get("claim_id")
            .and_then(JsonValue::as_str),
        Some("tos.claim.jenseits-1886-commission.date"),
        "the fixture must keep its named reported commissioning Claim"
    );
    let claim_attributes = get(claim_nodes[0], "attributes");
    let source_claim = get(claim_attributes, "source_claim");
    let source_object = get(source_claim, "object");
    let expected_wording = object(vec![
        ("language", text("de")),
        ("text", text("03. 06.1886")),
    ]);
    assert_eq!(get(source_object, "source_wording"), &expected_wording);
    let source_claim_digest = Digest256::of_bytes(&canonical(source_claim)).to_hex();
    let literal_digest = Digest256::of_bytes(&canonical(source_object)).to_hex();
    assert_eq!(
        get(claim_attributes, "source_sha256").as_str(),
        Some(source_claim_digest.as_str()),
        "the native Claim digest must cover the exact source record"
    );

    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let mut model = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
    let content_revision = get(claim_nodes[0], "content_revision").as_str().unwrap();
    let claim_id = get(claim_nodes[0], "id").as_str().unwrap();
    let temporal_request = selected_temporal_request(
        source_revision,
        claim_id,
        content_revision,
        claim_id,
        content_revision,
    );
    let mut authority = Authority::new(
        &bound,
        tos_query::TEMPORAL_OPERATION,
        tos_query::TEMPORAL_INTENDED_USE,
    );
    let mut packet = execute_selected_temporal(
        &mut model,
        &bound,
        &mut authority,
        &temporal_request,
        read_budget(),
    )
    .unwrap();
    packet.recheck().unwrap();
    let packet = parse(&packet);
    assert_eq!(
        get(get(&packet, "comparison"), "status").as_str(),
        Some("comparable")
    );
    assert_eq!(
        get(get(&packet, "comparison"), "relation").as_str(),
        Some("equal")
    );
    let left = get(&packet, "left");
    let returned_claim = get(left, "claim");
    let returned_value = get(left, "value");
    let normalized_time = get(left, "normalized_time");
    assert_eq!(
        get(get(returned_claim, "attributes"), "source_claim"),
        source_claim
    );
    assert_eq!(
        get(get(returned_value, "attributes"), "value"),
        source_object
    );
    assert_eq!(get(normalized_time, "raw"), source_object);
    assert_eq!(
        get(normalized_time, "source_wording"),
        get(source_object, "source_wording")
    );
    assert_eq!(
        get(get(returned_value, "attributes"), "value_sha256").as_str(),
        Some(literal_digest.as_str()),
        "the selected temporal value digest must cover its canonical raw literal"
    );
    assert_eq!(
        get(get(&packet, "authority_boundary"), "creates_inferred_claim"),
        &JsonValue::Bool(false)
    );
    assert_eq!(
        get(get(&packet, "authority_boundary"), "performs_assessment"),
        &JsonValue::Bool(false)
    );

    let nonclaim = graph_nodes
        .iter()
        .find(|node| node != &claim_nodes[0])
        .expect("native fixture must include a non-Claim carrier");
    let nonclaim_id = get(nonclaim, "id").as_str().unwrap();
    let nonclaim_revision = get(nonclaim, "content_revision").as_str().unwrap();
    let request = selected_temporal_request(
        source_revision,
        nonclaim_id,
        nonclaim_revision,
        claim_id,
        content_revision,
    );
    let mut authority = Authority::new(
        &bound,
        tos_query::TEMPORAL_OPERATION,
        tos_query::TEMPORAL_INTENDED_USE,
    );
    let packet =
        execute_selected_temporal(&mut model, &bound, &mut authority, &request, read_budget())
            .unwrap();
    let packet = parse(&packet);
    assert_eq!(
        get(get(&packet, "comparison"), "status").as_str(),
        Some("unsupported")
    );
    assert_eq!(
        get(get(&packet, "comparison"), "relation"),
        &JsonValue::Null
    );

    let mut sources = std::collections::BTreeSet::new();
    for row in graph_nodes.iter().chain(graph_relations) {
        if let Some(source) = get(row, "source_graph").as_str() {
            sources.insert(source.to_owned());
        }
    }
    assert!(!sources.is_empty());
    let sources = JsonValue::Array(sources.iter().map(|source| text(source)).collect());
    let node_ids = graph_nodes
        .iter()
        .map(|node| get(node, "id").as_str().unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let relation_ids = graph_relations
        .iter()
        .map(|relation| get(relation, "id").as_str().unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();

    // Exercise every fixture focus across both profiles and directions using
    // the native owner directly. Bounded pages must stay on the selected graph,
    // remain replayable, and terminate without the former Python oracle.
    for work in [2, 512] {
        let budget = exploration_budget(work);
        for focus in graph_nodes {
            let focus_id = get(focus, "id").as_str().unwrap();
            for profile in ["all", "overview"] {
                for direction in ["incoming", "outgoing", "either"] {
                    let first_request = object(vec![
                        ("focus_node_id", text(focus_id)),
                        ("sources", sources.clone()),
                        ("profile", text(profile)),
                        ("direction", text(direction)),
                        ("max_depth", parse(b"2")),
                        ("page_nodes", parse(b"1")),
                        ("page_relations", parse(b"1")),
                    ]);
                    let mut request = first_request.clone();
                    let mut checkpoints = Checkpoints::default();
                    let mut pages = Vec::new();
                    let mut first_cursor = None;
                    let mut completed = false;
                    for page_number in 0..256 {
                        let mut authority =
                            Authority::new(&bound, EXPLORATION_OPERATION, EXPLORATION_INTENDED_USE);
                        let mut response = execute_selected_exploration(
                            &mut model,
                            &bound,
                            &mut authority,
                            &mut checkpoints,
                            &request,
                            budget,
                        )
                        .unwrap();
                        response.recheck().unwrap();
                        let response = parse(&response);
                        assert_eq!(
                            get(&response, "source_revision").as_str(),
                            Some(source_revision)
                        );
                        assert_eq!(
                            get(&response, "focus")
                                .object_get("node_id")
                                .and_then(JsonValue::as_str),
                            Some(focus_id)
                        );
                        assert_eq!(
                            get(&response, "schema").as_str(),
                            Some("tos_exploration_result_v1")
                        );
                        assert!(matches!(
                            get(&response, "status").as_str(),
                            Some("paused" | "complete" | "limit_reached")
                        ));
                        let page = get(&response, "page");
                        // The cap selects primary nodes; complete edge endpoints
                        // are additionally returned as contextual nodes.
                        assert!(
                            get(page, "primary_node_ids").as_array().unwrap().len() <= 1,
                            "page node cap changed for {focus_id}/{profile}/{direction}"
                        );
                        assert_eq!(
                            get(page, "returned_nodes").as_u64().unwrap() as usize,
                            get(&response, "nodes").as_array().unwrap().len()
                        );
                        assert!(
                            get(page, "returned_relations")
                                .as_u64()
                                .is_some_and(|count| count <= 1),
                            "page relation cap changed for {focus_id}/{profile}/{direction}"
                        );
                        for node in get(&response, "nodes").as_array().unwrap() {
                            assert!(node_ids.contains(get(node, "id").as_str().unwrap()));
                        }
                        for relation in get(&response, "relations").as_array().unwrap() {
                            assert!(relation_ids.contains(get(relation, "id").as_str().unwrap()));
                        }
                        let next = get(page, "next_cursor").as_str();
                        if page_number == 0 {
                            first_cursor = next.map(str::to_owned);
                        }
                        pages.push(comparable(response.clone()));
                        if let Some(next) = next {
                            assert_eq!(get(&response, "status").as_str(), Some("paused"));
                            request = cursor_request(next);
                        } else {
                            assert!(matches!(
                                get(&response, "status").as_str(),
                                Some("complete" | "limit_reached")
                            ));
                            completed = true;
                            break;
                        }
                    }
                    assert!(completed, "bounded exploration did not terminate");
                    assert_eq!(checkpoints.0.lock().unwrap().commits, pages.len());
                    if let (Some(cursor), Some(expected)) = (first_cursor, pages.get(1)) {
                        let before = checkpoints.0.lock().unwrap().commits;
                        let mut authority =
                            Authority::new(&bound, EXPLORATION_OPERATION, EXPLORATION_INTENDED_USE);
                        let replay = execute_selected_exploration(
                            &mut model,
                            &bound,
                            &mut authority,
                            &mut checkpoints,
                            &cursor_request(&cursor),
                            budget,
                        )
                        .unwrap();
                        assert_eq!(canonical(&comparable(parse(&replay))), canonical(expected));
                        assert_eq!(checkpoints.0.lock().unwrap().commits, before);
                        authority.withdrawn = true;
                        assert!(matches!(
                            execute_selected_exploration(
                                &mut model,
                                &bound,
                                &mut authority,
                                &mut checkpoints,
                                &cursor_request(&cursor),
                                budget
                            ),
                            Err(SearchV2Error {
                                code: SearchV2ErrorCode::StalePolicy,
                                ..
                            })
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn genuine_relation_origin_keeps_exact_endpoint_closure_and_refuses_stale_revision() {
    let fixture = build_native_fixture();
    let graph = parse(&fixture.graph_input_bytes);
    let source_revision = get(&graph, "source_revision").as_str().unwrap();
    let nodes = get(&graph, "nodes").as_array().unwrap();
    let relations = get(&graph, "relations").as_array().unwrap();
    let relation = relations
        .first()
        .expect("native fixture must include a relation origin");
    let mut source_names = std::collections::BTreeSet::new();
    for row in nodes.iter().chain(relations) {
        if let Some(source) = get(row, "source_graph").as_str() {
            source_names.insert(source.to_owned());
        }
    }
    let sources = JsonValue::Array(source_names.iter().map(|source| text(source)).collect());

    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let mut model = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
    let relation_id = get(relation, "id").as_str().unwrap();
    let from_id = get(relation, "from_id").as_str().unwrap();
    let to_id = get(relation, "to_id").as_str().unwrap();
    let request = object(vec![
        ("schema_version", text("tos_exploration_request_v2")),
        ("source_revision", text(source_revision)),
        (
            "origin",
            object(vec![
                ("kind", text("relation")),
                ("id", text(relation_id)),
                (
                    "content_revision",
                    get(relation, "content_revision").clone(),
                ),
            ]),
        ),
        ("sources", sources),
        ("profile", text("all")),
        ("direction", text("either")),
        ("max_depth", parse(b"0")),
        ("page_nodes", parse(b"2")),
        ("page_relations", parse(b"1")),
    ]);
    let mut checkpoints = Checkpoints::default();
    let mut authority = Authority::new(&bound, EXPLORATION_OPERATION, EXPLORATION_INTENDED_USE);
    let mut response = execute_selected_exploration(
        &mut model,
        &bound,
        &mut authority,
        &mut checkpoints,
        &request,
        exploration_budget(512),
    )
    .unwrap();
    response.recheck().unwrap();
    let response = parse(&response);
    assert_eq!(
        get(&response, "schema").as_str(),
        Some("tos_exploration_result_v2")
    );
    assert_eq!(
        get(get(&response, "origin"), "kind").as_str(),
        Some("relation")
    );
    assert_eq!(
        get(get(&response, "origin"), "id").as_str(),
        Some(relation_id)
    );
    let endpoints = get(get(&response, "origin"), "endpoints");
    assert_eq!(
        get(get(endpoints, "from"), "node_id").as_str(),
        Some(from_id)
    );
    assert_eq!(get(get(endpoints, "to"), "node_id").as_str(), Some(to_id));
    let context_relations = get(get(&response, "page"), "context_relation_ids")
        .as_array()
        .unwrap();
    assert_eq!(context_relations, &[text(relation_id)]);
    let returned_nodes = get(&response, "nodes").as_array().unwrap();
    let returned_ids = returned_nodes
        .iter()
        .map(|node| get(node, "id").as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(returned_ids, [from_id, to_id].into_iter().collect());
    assert!(returned_nodes.iter().any(|node| {
        get(node, "id").as_str() == Some(from_id)
            && get(node, "content_revision") == get(get(endpoints, "from"), "content_revision")
    }));
    assert!(returned_nodes.iter().any(|node| {
        get(node, "id").as_str() == Some(to_id)
            && get(node, "content_revision") == get(get(endpoints, "to"), "content_revision")
    }));

    let mut stale = request;
    let mut origin = get(&stale, "origin").clone();
    set(&mut origin, "content_revision", text(&"0".repeat(64)));
    set(&mut stale, "origin", origin);
    let mut authority = Authority::new(&bound, EXPLORATION_OPERATION, EXPLORATION_INTENDED_USE);
    assert!(matches!(
        execute_selected_exploration(
            &mut model,
            &bound,
            &mut authority,
            &mut checkpoints,
            &stale,
            exploration_budget(512),
        ),
        Err(SearchV2Error {
            code: SearchV2ErrorCode::StaleSelection,
            ..
        })
    ));
}

#[test]
fn genuine_catalogue_temporal_source_profile_bytes_and_line_are_exact() {
    const SOURCE_REF: &str =
        "ToS/source-witnesses/relations/nietzsche-letter-705-catalogue-date/source-claims.jsonl";
    const CLAIM_DIGEST: &str = "b884e76996edac910334e3abf44cf0f82bee3be37d00c001e18e3295ebe3dee8";
    let line = include_bytes!(
        "../../../../ToS/source-witnesses/relations/nietzsche-letter-705-catalogue-date/source-claims.jsonl"
    );
    let mut data_lines = line
        .split(|byte| *byte == b'\n')
        .filter(|bytes| !bytes.is_empty());
    let source = parse(data_lines.next().expect("catalogue Claim source line"));
    assert!(
        data_lines.next().is_none(),
        "focused source file is one JSONL row"
    );
    let source_bytes = canonical(&source);
    let source_digest = Digest256::of_bytes(&source_bytes).to_hex();
    assert_eq!(
        source_digest, CLAIM_DIGEST,
        "canonical source Claim bytes drifted"
    );
    assert_eq!(
        get(&source, "claim_id").as_str(),
        Some("tos.claim.nietzsche-letter-705.catalogue-date")
    );
    assert_eq!(
        get(&source, "schema_version").as_str(),
        Some("tos_document_catalogue_claim_v1")
    );
    assert_eq!(
        get(&source, "predicate").as_str(),
        Some("document_catalogue_date")
    );
    assert_eq!(
        get(&source, "subject_ref").as_str(),
        Some("tos.letter.nietzsche-naumann-1886-705")
    );

    let registry = parse(include_bytes!(
        "../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json"
    ));
    let relation = get(&registry, "relations")
        .as_array()
        .unwrap()
        .iter()
        .find(|relation| {
            get(relation, "relation_type_id").as_str()
                == Some("tos.relation.document-catalogue-date")
        })
        .expect("owned document-catalogue-date relation");
    let profile = get(relation, "source_claim_profile").clone();
    assert_eq!(
        get(&profile, "reader").as_str(),
        Some("document-catalogue-temporal-v1")
    );
    assert_eq!(
        get(&profile, "assertion_layers").as_array(),
        Some([text("bibliographic_assertion")].as_slice())
    );
    assert_eq!(
        get(
            &get(&profile, "schemas").as_array().unwrap()[0],
            "schema_ref"
        )
        .as_str(),
        Some("ToS/contracts/document-catalogue-claim.schema.json")
    );

    let claim_id = get(&source, "claim_id").as_str().unwrap();
    let subject_id = "source-claims:document:tos.letter.nietzsche-naumann-1886-705";
    let claim_node_id = format!("source-claims:claim:{claim_id}");
    let value_node_id = format!("literal:sha256:{}", {
        let literal = object(vec![
            ("claim_ref", text(claim_id)),
            ("value", get(&source, "object").clone()),
        ]);
        Digest256::of_bytes(&canonical(&literal)).to_hex()
    });
    let source_line = parse(b"1");
    let source_refs = JsonValue::Array(vec![text(SOURCE_REF)]);
    let source_canonical = String::from_utf8(source_bytes.clone()).unwrap();
    let claim_semantics = object(vec![
        ("claim_id", get(&source, "claim_id").clone()),
        ("claim_version", get(&source, "claim_version").clone()),
        ("object_node_id", text(&value_node_id)),
        ("subject_node_id", text(subject_id)),
        (
            "relation_type_id",
            get(relation, "relation_type_id").clone(),
        ),
        ("source_predicate_id", get(&source, "predicate").clone()),
        ("predicate_mapping_status", text("mapped")),
        ("source_claim_profile", profile),
        ("source_canonical_json", text(&source_canonical)),
    ]);
    let claim = object(vec![
        ("id", text(&claim_node_id)),
        ("native_id", text(&format!("claim:{claim_id}"))),
        ("source_graph", text("source-claims")),
        ("kind_id", text("claim")),
        ("type_id", text("tos.entity.claim")),
        ("type_mapping", mapped_type()),
        ("content_revision", text(&"c".repeat(64))),
        (
            "attributes",
            object(vec![
                ("source_claim", source.clone()),
                ("source_sha256", text(&source_digest)),
                ("source_line", source_line.clone()),
                ("source_refs", source_refs.clone()),
            ]),
        ),
        ("semantics", object(vec![("claim", claim_semantics)])),
        ("source_refs", source_refs.clone()),
    ]);
    let raw_value = get(&source, "object").clone();
    let raw_digest = Digest256::of_bytes(&canonical(&raw_value)).to_hex();
    let time = object(vec![
        ("kind", get(&raw_value, "kind").clone()),
        ("raw", raw_value.clone()),
        ("calendar", get(&raw_value, "calendar").clone()),
        (
            "declared_year_numbering",
            get(&raw_value, "year_numbering").clone(),
        ),
        ("certainty", get(&raw_value, "certainty").clone()),
        ("precision", text("day")),
        ("comparison_calendar", JsonValue::Null),
        ("year_numbering", JsonValue::Null),
        ("sort_start", JsonValue::Null),
        ("sort_end", JsonValue::Null),
        ("role", get(&raw_value, "role").clone()),
        ("source_wording", get(&raw_value, "source_wording").clone()),
        ("normalization_status", text("structured-source")),
        ("source_field", text("object")),
        (
            "issues",
            JsonValue::Array(vec![
                text("calendar-not-comparable"),
                text("year-numbering-not-comparable"),
            ]),
        ),
    ]);
    let value = object(vec![
        ("id", text(&value_node_id)),
        ("native_id", text(&value_node_id)),
        ("source_graph", text("source-claims")),
        ("type_id", text("tos.entity.temporal-assertion")),
        ("type_mapping", mapped_type()),
        ("content_revision", text(&"d".repeat(64))),
        (
            "attributes",
            object(vec![
                ("claim_ref", text(claim_id)),
                ("value", raw_value.clone()),
                ("source_sha256", text(&source_digest)),
                ("value_sha256", text(&raw_digest)),
                ("source_line", source_line.clone()),
            ]),
        ),
        ("semantics", object(vec![("time", time)])),
        ("source_refs", source_refs.clone()),
    ]);
    let subject = object(vec![
        ("id", text(subject_id)),
        ("entity_id", get(&source, "subject_ref").clone()),
        ("source_graph", text("source-claims")),
        ("type_id", text("tos.entity.document")),
        ("type_mapping", mapped_type()),
        (
            "semantics",
            object(vec![(
                "type_ancestors",
                JsonValue::Array(vec![text("tos.entity.document")]),
            )]),
        ),
    ]);
    let rows = BTreeMap::from([
        (claim_node_id.clone(), vec![claim.clone()]),
        (value_node_id.clone(), vec![value.clone()]),
        (subject_id.to_owned(), vec![subject]),
    ]);
    let revision = "a".repeat(64);
    let content_revision = "c".repeat(64);
    let request = selected_temporal_request(
        &revision,
        &claim_node_id,
        &content_revision,
        &claim_node_id,
        &content_revision,
    );
    let result = compare_temporal_operands(
        &revision,
        &request,
        "source-claims",
        |id| Ok(rows.get(id).cloned().unwrap_or_default()),
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        get(get(&result, "comparison"), "status").as_str(),
        Some("undetermined")
    );
    assert_eq!(
        get(get(&result, "comparison"), "relation"),
        &JsonValue::Null
    );
    let returned_claim = get(get(&result, "left"), "claim");
    let returned_value = get(get(&result, "left"), "value");
    assert_eq!(
        get(
            get(&get(returned_claim, "semantics"), "claim"),
            "source_canonical_json"
        )
        .as_str(),
        Some(source_canonical.as_str())
    );
    assert_eq!(
        get(get(returned_claim, "attributes"), "source_line"),
        &source_line
    );
    assert_eq!(
        get(get(returned_value, "attributes"), "source_line"),
        &source_line
    );
    assert_eq!(
        get(
            get(&get(returned_claim, "semantics"), "claim"),
            "source_claim_profile"
        ),
        get(relation, "source_claim_profile")
    );
    assert_eq!(
        get(&get(&get(&result, "left"), "normalized_time"), "raw")
            .object_get("role")
            .and_then(JsonValue::as_str),
        Some("catalogue-assigned-document-date")
    );
    assert_eq!(
        get(
            &get(&result, "authority_boundary"),
            "creates_inferred_claim"
        ),
        &JsonValue::Bool(false)
    );
    for code in [
        "declared-calendar-unavailable",
        "declared-year-numbering-unavailable",
        "comparison-calendar-unavailable",
        "comparison-year-numbering-unavailable",
        "absolute-date-envelope-unavailable",
    ] {
        assert!(
            get(get(&result, "comparison"), "reasons")
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| get(reason, "code").as_str() == Some(code))
        );
    }

    let mut drifted_value = value;
    let mut attributes = get(&drifted_value, "attributes").clone();
    set(&mut attributes, "source_line", parse(b"2"));
    set(&mut drifted_value, "attributes", attributes);
    let drifted = BTreeMap::from([
        (claim_node_id.clone(), vec![claim]),
        (value_node_id, vec![drifted_value]),
        (subject_id.to_owned(), rows.get(subject_id).unwrap().clone()),
    ]);
    let result = compare_temporal_operands(
        &revision,
        &request,
        "source-claims",
        |id| Ok(drifted.get(id).cloned().unwrap_or_default()),
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        get(get(&result, "comparison"), "status").as_str(),
        Some("undetermined")
    );
    assert!(
        get(get(&result, "comparison"), "reasons")
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| get(reason, "code").as_str()
                == Some("document-catalogue-exact-source-binding-inconsistent"))
    );
}

/// P01-P03 share one selected-reader family, with fresh producer-owned fixture bytes.
#[test]
fn selected_claim_projection_preserves_conflicts_unknown_calendar_and_year_precision() {
    use serde_json::{Value, json};
    use tos_compiler::knowledge_full_fixture::build_native_fixture_with_claim_source;
    use tos_query::{execute_selected_claim_list, execute_selected_claim_projection};

    let client_json_roundtrip = |node: &std::ffi::OsStr, raw: &[u8]| {
        use std::io::Write;
        assert!(raw.len() <= 1_000_000);
        let mut child = std::process::Command::new(node)
            .args(["-e", "process.stdout.write(JSON.stringify(JSON.parse(require('fs').readFileSync(0,'utf8'))))"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(raw).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        assert!(result.stdout.len() <= 1_000_000);
        result.stdout
    };

    fn rewrite(
        value: &mut Value,
        original: &Value,
        replacement: &Value,
        names: &[(String, String)],
    ) {
        if value == original {
            *value = replacement.clone();
            return;
        }
        match value {
            Value::String(s) => {
                for (old, new) in names {
                    *s = s.replace(old, new);
                }
            }
            Value::Array(a) => {
                for v in a {
                    rewrite(v, original, replacement, names);
                }
            }
            Value::Object(o) => {
                for v in o.values_mut() {
                    rewrite(v, original, replacement, names);
                }
            }
            _ => {}
        }
    }
    // Explicit synthetic external records: neither Claim self nor subject/basis
    // supplies the about/knower bindings. They assert no admitted owner identity.
    fn reference(id: &str) -> Value {
        json!({"id":id,"version":3,"digest":"1".repeat(64)})
    }
    fn coordinate(value: i64) -> Value {
        json!({"type":"CoordinatePosition","resolution":"exact","value":value,
            "trs":{"id":"fixture-axis","version":"rules-1"},"unit":"tick","origin":"O"})
    }
    fn basis() -> Value {
        json!({"source":reference("fixture:external-evidence"),"source_locator":null,
            "raw_evidence":"explicit synthetic supplemental assertion"})
    }
    fn temporal_filter(cutoff: i64) -> Value {
        json!({"profile_ref":"tos_claim_coordinate_filter_v1",
            "assertion_scope":"SupplementalCarrierOnly","assertion_selection":"ExactlyOnePerPredicate",
            "non_total":"RefusePage","roles":[{"role":"AttributedKnowledge",
                "about_ref":reference("fixture:external-about"),"knower_ref":reference("fixture:external-knower"),
                "operator":"LessOrEqual","cutoff":coordinate(cutoff),"basis":basis()}],
            "world_valid_time":null})
    }
    fn temporal_packet(value: i64) -> Value {
        let mut roles = Vec::new();
        for role in [
            "AttributedKnowledge",
            "SourcePublication",
            "SystemTransaction",
        ] {
            let mut assertion = json!({"profile_ref":"tos_role_coordinate_assertion_v1",
                "about_ref":reference("fixture:external-about"),"position":coordinate(value)});
            if role == "AttributedKnowledge" {
                assertion["knower_ref"] = reference("fixture:external-knower");
            }
            roles.push(json!({"role":role,"assertion":assertion,"basis":basis()}));
        }
        roles.push(json!({"role":"ActualInterval","assertion":{
            "profile_ref":"tos_world_coordinate_extent_v1","about_ref":reference("fixture:external-about"),
            "extent":{"type":"Interval","beginning":{"position":coordinate(4),"inclusivity":"Inclusive"},
                "end":{"position":coordinate(10),"inclusivity":"Exclusive"}}},"basis":basis()}));
        json!({"schema_version":"tos_temporal_claim_carrier_v1","roles":roles})
    }
    let template: Value = serde_json::from_slice(include_bytes!(
        "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
    ))
    .unwrap();
    let original_claim = &template["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node_kind"] == "claim")
        .unwrap()["properties"]["source_claim"];
    let original_object = original_claim["object"].clone();
    let old_id = original_claim["claim_id"].as_str().unwrap();
    let old_digest = Digest256::of_bytes(&canonical(&parse(
        &serde_json::to_vec(original_claim).unwrap(),
    )))
    .to_hex();
    let literal = template["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node_kind"] == "literal")
        .unwrap();
    let original_literal_id = literal["node_id"].as_str().unwrap();
    for case in [
        "conflicting-dates",
        "unknown-calendar",
        "year-only",
        "unsupported-carrier",
        "mixed-carrier",
    ] {
        let objects = match case {
            "conflicting-dates" | "mixed-carrier" => {
                let mut other = original_object.clone();
                other["value"] = json!("1887-06-03");
                other["source_wording"]["text"] = json!("03. 06.1887");
                vec![original_object.clone(), other]
            }
            "unknown-calendar" => {
                let mut source = original_object.clone();
                source["calendar"] = Value::Null;
                vec![source]
            }
            "year-only" => {
                let mut source = original_object.clone();
                source["value"] = json!("1886");
                source["source_wording"]["text"] = json!("1886");
                vec![source]
            }
            _ => vec![original_object.clone()],
        };
        let mut source = template.clone();
        for collection in ["nodes", "edges", "claim_traces"] {
            source[collection] = json!([]);
        }
        // Shared source identity/evidence carriers stay unchanged. Only test Claim/literal rows vary.
        for n in template["nodes"].as_array().unwrap() {
            if n["node_kind"] != "claim" && n["node_kind"] != "literal" {
                source["nodes"].as_array_mut().unwrap().push(n.clone());
            }
        }
        for (index, source_object) in objects.iter().enumerate() {
            let new_id = format!("tos.claim.synthetic.projection-{case}-{index}");
            let mut record = original_claim.clone();
            record["claim_id"] = json!(new_id);
            record["object"] = source_object.clone();
            if case == "conflicting-dates" {
                // Exact native evidence is retained; the opt-in client transport
                // must refuse to expose this integer as a JavaScript Number.
                record["qualifiers"]["client_boundary_evidence"] =
                    json!({"exact_large":9007199254740993u64,"raw_decimal":"-0.00000000000000001"});
            }
            match case {
                "conflicting-dates" | "mixed-carrier" => {
                    record["qualifiers"]["temporal_claim_carrier"] =
                        if case == "mixed-carrier" && index == 1 {
                            Value::Null
                        } else {
                            temporal_packet(if index == 0 { 4 } else { 12 })
                        };
                }
                "unknown-calendar" => record["qualifiers"]["temporal_claim_carrier"] = Value::Null,
                "unsupported-carrier" => {
                    record["qualifiers"]["temporal_claim_carrier"] =
                        json!({"schema_version":"unsupported_fixture_version","roles":[]})
                }
                _ => {}
            }
            let digest =
                Digest256::of_bytes(&canonical(&parse(&serde_json::to_vec(&record).unwrap())))
                    .to_hex();
            let raw_digest = Digest256::of_bytes(&canonical(&parse(
                &serde_json::to_vec(source_object).unwrap(),
            )))
            .to_hex();
            let literal_id = format!(
                "literal:sha256:{}",
                Digest256::of_bytes(&canonical(&parse(
                    &serde_json::to_vec(&json!({"claim_ref":new_id,"value":source_object}))
                        .unwrap()
                )))
                .to_hex()
            );
            let old_raw = Digest256::of_bytes(&canonical(&parse(
                &serde_json::to_vec(&original_object).unwrap(),
            )))
            .to_hex();
            let names = vec![
                (original_literal_id.to_owned(), literal_id),
                (old_id.to_owned(), new_id),
                (old_digest.clone(), digest),
                (old_raw, raw_digest),
            ];
            for collection in ["nodes", "edges", "claim_traces"] {
                for row in template[collection].as_array().unwrap() {
                    if collection == "nodes"
                        && row["node_kind"] != "claim"
                        && row["node_kind"] != "literal"
                    {
                        continue;
                    }
                    let mut row = row.clone();
                    rewrite(&mut row, &original_object, source_object, &names);
                    if collection == "nodes" && row["node_kind"] == "claim" {
                        row["properties"]["source_claim"] = record.clone();
                        row["properties"]["qualifiers"] = record["qualifiers"].clone();
                        row["properties"]
                            .as_object_mut()
                            .unwrap()
                            .remove("human_forms");
                        row["properties"]
                            .as_object_mut()
                            .unwrap()
                            .remove("human_forms_source_ref");
                    }
                    if collection == "claim_traces" {
                        row["qualifiers"] = record["qualifiers"].clone();
                    }
                    source[collection].as_array_mut().unwrap().push(row);
                }
            }
        }
        let source_raw = serde_json::to_vec(&source).unwrap();
        let fixture = build_native_fixture_with_claim_source(&source_raw);
        let model_digest = || {
            assert!(std::fs::metadata(&fixture.path).unwrap().len() <= 64 * 1024 * 1024);
            Digest256::of_bytes(&std::fs::read(&fixture.path).unwrap())
        };
        let model_before = model_digest();
        let graph = parse(&fixture.graph_input_bytes);
        let claims: Vec<_> = get(&graph, "nodes")
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| get(n, "kind_id").as_str() == Some("claim"))
            .collect();
        assert_eq!(claims.len(), objects.len());
        let members = JsonValue::Array(
            claims
                .iter()
                .map(|n| {
                    object(vec![
                        ("node_id", get(n, "id").clone()),
                        ("content_revision", get(n, "content_revision").clone()),
                    ])
                })
                .collect(),
        );
        let revision = get(&graph, "source_revision").clone();
        let cold = fixture.open().unwrap();
        let bound =
            bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
        let mut model = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
        let mut previous = None;
        for calendar in ["gregorian", "julian"] {
            let request = object(vec![
                ("schema_version", text("tos_claim_projection_request_v1")),
                ("source_revision", revision.clone()),
                ("members", members.clone()),
                (
                    "display_perspective",
                    object(vec![
                        ("mode", text("SourcePreserving")),
                        (
                            "profile_ref",
                            object(vec![
                                ("id", text("tos.display.synthetic.projection")),
                                ("version", parse(b"1")),
                                ("digest", text(&format!("sha256:{}", "a".repeat(64)))),
                            ]),
                        ),
                        ("requested_calendar", text(calendar)),
                        ("requested_language", text("en")),
                    ]),
                ),
            ]);
            let authority = || {
                Authority::new(
                    &bound,
                    "tos.knowledge.temporal.project_claims",
                    "read_only_public_knowledge_claim_projection_v1",
                )
            };
            let mut packet = execute_selected_claim_projection(
                &mut model,
                &bound,
                &mut authority(),
                &request,
                read_budget(),
            )
            .unwrap();
            packet.recheck().unwrap();
            let result = parse(&packet);
            assert_eq!(get(&result, "source_revision"), &revision);
            assert_eq!(
                get(&result, "projection_scope").as_str(),
                Some("exact-selected-model-members")
            );
            assert_eq!(
                get(&result, "profile_binding").as_str(),
                Some("caller-declared-display-request")
            );
            let rows = get(&result, "rows").as_array().unwrap();
            assert_eq!(rows.len(), claims.len());
            let returned_objects = rows
                .iter()
                .map(|row| {
                    canonical(get(
                        get(
                            get(get(get(row, "operand"), "claim"), "attributes"),
                            "source_claim",
                        ),
                        "object",
                    ))
                })
                .collect::<std::collections::BTreeSet<_>>();
            let expected_objects = objects
                .iter()
                .map(|v| canonical(&parse(&serde_json::to_vec(v).unwrap())))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                returned_objects, expected_objects,
                "every contradictory source dating must survive separately"
            );
            for (row, claim) in rows.iter().zip(&claims) {
                assert!(same_json_fields(get(get(row, "operand"), "claim"), claim));
                assert_eq!(get(row, "derived_coordinate"), &JsonValue::Null);
                assert_eq!(
                    get(row, "derived_coordinate_status").as_str(),
                    Some("Unresolved")
                );
                let returned = get(
                    get(get(get(row, "operand"), "claim"), "attributes"),
                    "source_claim",
                );
                assert!(same_json_fields(
                    get(returned, "subject_ref"),
                    &parse(&serde_json::to_vec(&original_claim["subject_ref"]).unwrap())
                ));
                let source_object = get(returned, "object");
                let expected = objects
                    .iter()
                    .find(|v| {
                        same_json_fields(&parse(&serde_json::to_vec(v).unwrap()), source_object)
                    })
                    .unwrap();
                assert!(same_json_fields(
                    source_object,
                    &parse(&serde_json::to_vec(expected).unwrap())
                ));
                if case == "unknown-calendar" {
                    assert_eq!(get(source_object, "calendar"), &JsonValue::Null);
                }
                if case == "year-only" {
                    assert_eq!(get(source_object, "value").as_str(), Some("1886"));
                }
            }
            if let Some(before) = previous.replace(get(&result, "rows").clone()) {
                assert_eq!(
                    before,
                    *get(&result, "rows"),
                    "display calendars cannot alter source rows"
                );
            }
            let mut stale = request.clone();
            let mut wrong_members = members.clone();
            let JsonValue::Array(ref mut a) = wrong_members else {
                unreachable!()
            };
            set(&mut a[0], "content_revision", text(&"0".repeat(64)));
            set(&mut stale, "members", wrong_members);
            assert!(matches!(
                execute_selected_claim_projection(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &stale,
                    read_budget()
                ),
                Err(SearchV2Error {
                    code: SearchV2ErrorCode::StaleSelection,
                    ..
                })
            ));
            let mut stale_cut = request.clone();
            set(&mut stale_cut, "source_revision", text(&"0".repeat(64)));
            let mut bad_mode = request.clone();
            let mut display = get(&request, "display_perspective").clone();
            set(&mut display, "mode", text("Convert"));
            set(&mut bad_mode, "display_perspective", display);
            let mut float_version = request.clone();
            let mut display = get(&request, "display_perspective").clone();
            let mut profile = get(&display, "profile_ref").clone();
            set(&mut profile, "version", parse(b"1.0"));
            set(&mut display, "profile_ref", profile);
            set(&mut float_version, "display_perspective", display);
            for (bad, code) in [
                (stale_cut, SearchV2ErrorCode::StaleSelection),
                (bad_mode, SearchV2ErrorCode::InvalidRequest),
                (float_version, SearchV2ErrorCode::InvalidRequest),
            ] {
                match execute_selected_claim_projection(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &bad,
                    read_budget(),
                ) {
                    Err(reason) => assert_eq!(reason.code, code),
                    Ok(_) => panic!("invalid projection boundary accepted"),
                }
            }
            let mut tiny = read_budget();
            tiny.max_payload_bytes = 8;
            assert!(matches!(
                execute_selected_claim_projection(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &request,
                    tiny
                ),
                Err(SearchV2Error {
                    code: SearchV2ErrorCode::BudgetExceeded,
                    ..
                })
            ));
            let mut denied = authority();
            denied.withdrawn = true;
            assert!(matches!(
                execute_selected_claim_projection(
                    &mut model,
                    &bound,
                    &mut denied,
                    &request,
                    read_budget()
                ),
                Err(SearchV2Error {
                    code: SearchV2ErrorCode::StalePolicy,
                    ..
                })
            ));
            let mut wrong_scope = Authority::new(
                &bound,
                tos_query::TEMPORAL_OPERATION,
                tos_query::TEMPORAL_INTENDED_USE,
            );
            assert!(
                execute_selected_claim_projection(
                    &mut model,
                    &bound,
                    &mut wrong_scope,
                    &request,
                    read_budget()
                )
                .is_err()
            );
        }
        // The list provider discovers the same real source Claims without caller members.
        let authority = || {
            Authority::new(
                &bound,
                "tos.knowledge.temporal.list_claims",
                "read_only_public_knowledge_claim_list_v1",
            )
        };
        let source_claim = get(get(claims[0], "attributes"), "source_claim");
        let mut filters = vec![object(vec![])];
        for key in [
            "subject_ref",
            "predicate",
            "claim_type",
            "assertion_layer",
            "visibility",
        ] {
            filters.push(object(vec![(key, get(source_claim, key).clone())]));
        }
        filters.push(object(vec![(
            "subject_ref",
            text("tos.event.synthetic.outside-selected-subject"),
        )]));
        let mut continuation = None;
        let mut saw_empty_progress = false;
        for filter in filters {
            let expected = claims
                .iter()
                .filter(|claim| {
                    let source = get(get(claim, "attributes"), "source_claim");
                    filter
                        .as_object()
                        .unwrap()
                        .iter()
                        .all(|(key, value)| get(source, key.as_str().unwrap()) == value)
                })
                .map(|claim| get(claim, "id").as_str().unwrap().to_owned())
                .collect::<std::collections::BTreeSet<_>>();
            for scan in [1, 64] {
                let mut request = object(vec![
                    ("schema_version", text("tos_claim_list_request_v1")),
                    ("source_revision", revision.clone()),
                    ("filter", filter.clone()),
                    ("page_size", parse(b"1")),
                    ("scan_limit", parse(scan.to_string().as_bytes())),
                    ("after", JsonValue::Null),
                ]);
                let mut found = std::collections::BTreeSet::new();
                let mut ended = false;
                for _ in 0..=get(&graph, "nodes").as_array().unwrap().len() {
                    let mut packet = execute_selected_claim_list(
                        &mut model,
                        &bound,
                        &mut authority(),
                        &request,
                        read_budget(),
                    )
                    .unwrap();
                    packet.recheck().unwrap();
                    let page = parse(&packet);
                    assert_eq!(get(&page, "source_revision"), &revision);
                    assert_eq!(
                        get(&page, "selected_model_receipt_id").as_str(),
                        Some(bound.owner_receipt_id())
                    );
                    assert_eq!(
                        get(&page, "cursor_scope").as_str(),
                        Some("untrusted-suffix-position")
                    );
                    assert_eq!(
                        get(&page, "binds_persisted_chronology"),
                        &JsonValue::Bool(false)
                    );
                    let rows = get(&page, "rows").as_array().unwrap();
                    assert!(rows.len() <= 1);
                    assert!(get(&page, "examined_candidates").as_u64().unwrap() <= scan);
                    for row in rows {
                        let id = get(row, "node_id").as_str().unwrap();
                        assert!(
                            found.insert(id.to_owned()),
                            "paging repeated an examined Claim"
                        );
                        let original = claims
                            .iter()
                            .find(|claim| get(claim, "id").as_str() == Some(id))
                            .unwrap();
                        assert!(same_json_fields(get(row, "carrier"), original));
                        let source = get(get(original, "attributes"), "source_claim");
                        let reference = get(row, "claim_ref");
                        assert_eq!(get(reference, "id"), get(source, "claim_id"));
                        assert_eq!(get(reference, "version"), get(source, "claim_version"));
                        assert_eq!(
                            get(reference, "digest").as_str().unwrap(),
                            format!(
                                "sha256:{}",
                                Digest256::of_bytes(&canonical(source)).to_hex()
                            )
                        );
                    }
                    let next = get(&page, "next").clone();
                    if next.is_null() {
                        assert_eq!(get(&page, "exhausted"), &JsonValue::Bool(true));
                        ended = true;
                        break;
                    }
                    assert_eq!(get(&page, "exhausted"), &JsonValue::Bool(false));
                    if rows.is_empty() {
                        saw_empty_progress = true;
                    }
                    assert_ne!(
                        get(&request, "after"),
                        &next,
                        "keyset must advance on nonmatches too"
                    );
                    set(&mut request, "after", next);
                    if continuation.is_none() {
                        continuation = Some(request.clone());
                    }
                }
                assert!(ended, "bounded suffix traversal did not terminate");
                assert_eq!(
                    found, expected,
                    "last examined cursor must not skip prefetched Claims"
                );
            }
        }
        // Opt-in supplemental coordinate filtering uses the same actual keyset,
        // source authentication and disclosure lease, with page-atomic refusal.
        let mut temporal_page = |filter: Value, after: JsonValue, page_size: u64| {
            let request = object(vec![
                ("schema_version", text("tos_claim_list_request_v1")),
                ("source_revision", revision.clone()),
                ("filter", parse(&serde_json::to_vec(&filter).unwrap())),
                ("page_size", parse(page_size.to_string().as_bytes())),
                ("scan_limit", parse(b"64")),
                ("after", after),
            ]);
            execute_selected_claim_list(
                &mut model,
                &bound,
                &mut authority(),
                &request,
                read_budget(),
            )
            .and_then(|mut packet| {
                packet.recheck()?;
                Ok(parse(&packet))
            })
        };
        let selected =
            temporal_page(json!({"temporal":temporal_filter(8)}), JsonValue::Null, 64).unwrap();
        if case == "conflicting-dates" {
            let rows = get(&selected, "rows").as_array().unwrap();
            assert_eq!(rows.len(), 1);
            let proof = get(&rows[0], "temporal_filter_result");
            assert_eq!(get(proof, "outcome").as_str(), Some("Match"));
            assert_eq!(
                get(proof, "source_revision").as_str().unwrap(),
                format!("sha256:{}", revision.as_str().unwrap())
            );
            assert_eq!(
                get(get(proof, "claim_ref"), "id"),
                get(get(&rows[0], "claim_ref"), "id")
            );
            assert_eq!(
                get(get(proof, "claim_ref"), "digest").as_str().unwrap(),
                get(get(&rows[0], "claim_ref"), "digest")
                    .as_str()
                    .unwrap()
                    .strip_prefix("sha256:")
                    .unwrap()
            );
            assert_eq!(
                get(get(get(&rows[0], "carrier"), "attributes"), "source_claim")
                    .object_get("qualifiers")
                    .unwrap()
                    .object_get("temporal_claim_carrier")
                    .unwrap()
                    .object_get("roles")
                    .unwrap()
                    .as_array()
                    .unwrap()[0]
                    .object_get("assertion")
                    .unwrap()
                    .object_get("position")
                    .unwrap()
                    .object_get("value")
                    .unwrap()
                    .as_u64(),
                Some(4)
            );
            let empty =
                temporal_page(json!({"temporal":temporal_filter(0)}), JsonValue::Null, 64).unwrap();
            assert!(get(&empty, "rows").as_array().unwrap().is_empty());
            assert_eq!(get(&empty, "exhausted"), &JsonValue::Bool(true));
            let all = json!({"temporal":temporal_filter(100)});
            let first = temporal_page(all.clone(), JsonValue::Null, 1).unwrap();
            let cursor = get(&first, "next").clone();
            assert!(!cursor.is_null());
            let second = temporal_page(all, cursor.clone(), 1).unwrap();
            assert_eq!(get(&second, "rows").as_array().unwrap().len(), 1);
            assert_ne!(get(&first, "rows"), get(&second, "rows"));
            assert_eq!(
                temporal_page(json!({"temporal":temporal_filter(99)}), cursor, 1)
                    .err()
                    .unwrap()
                    .code,
                SearchV2ErrorCode::InvalidRequest
            );
            for role in ["SourcePublication", "SystemTransaction"] {
                let mut f = temporal_filter(8);
                f["roles"][0]["role"] = json!(role);
                f["roles"][0].as_object_mut().unwrap().remove("knower_ref");
                let page = temporal_page(json!({"temporal":f}), JsonValue::Null, 64).unwrap();
                assert_eq!(get(&page, "rows").as_array().unwrap().len(), 1);
            }
            for (at, expected) in [(7, 2), (10, 0)] {
                let mut f = temporal_filter(8);
                f["roles"] = json!([]);
                f["world_valid_time"] = json!({"profile_ref":"tos_world_coordinate_at_filter_v1",
                    "about_ref":reference("fixture:external-about"),"at":coordinate(at),"basis":basis()});
                let page = temporal_page(json!({"temporal":f}), JsonValue::Null, 64).unwrap();
                assert_eq!(get(&page, "rows").as_array().unwrap().len(), expected);
            }
            for boundary in ["knower", "about-self", "axis", "refusal-dominates-false"] {
                let mut f = temporal_filter(0);
                match boundary {
                    "knower" => f["roles"][0]["knower_ref"] = reference("fixture:other-knower"),
                    "about-self" => {
                        let source = get(get(claims[0], "attributes"), "source_claim");
                        f["roles"][0]["about_ref"] = json!({"id":get(source,"claim_id").as_str().unwrap(),
                            "version":get(source,"claim_version").as_u64().unwrap(),
                            "digest":Digest256::of_bytes(&canonical(source)).to_hex()});
                    }
                    "axis" => f["roles"][0]["cutoff"]["origin"] = json!("other-origin"),
                    _ => {
                        let mut missing = f["roles"][0].clone();
                        missing["role"] = json!("SourcePublication");
                        missing.as_object_mut().unwrap().remove("knower_ref");
                        missing["about_ref"] = reference("fixture:other-about");
                        f["roles"].as_array_mut().unwrap().push(missing);
                    }
                }
                let page = temporal_page(json!({"temporal":f}), JsonValue::Null, 64).unwrap();
                assert_eq!(get(&page, "outcome").as_str(), Some("NonTotalRefusal"));
                for absent in ["rows", "next", "exhausted"] {
                    assert!(page.object_get(absent).is_none());
                }
            }
        } else {
            assert_eq!(
                get(&selected, "schema_version").as_str(),
                Some("tos_claim_list_non_total_refusal_v1")
            );
            assert_eq!(get(&selected, "outcome").as_str(), Some("NonTotalRefusal"));
            for absent in ["rows", "next", "exhausted"] {
                assert!(selected.object_get(absent).is_none());
            }
            let refusal = get(&selected, "refusal");
            assert_eq!(get(refusal, "outcome").as_str(), Some("Refused"));
            if case == "unsupported-carrier" {
                assert_eq!(get(refusal, "domain").as_str(), Some("Unsupported"));
            }
            if case == "unknown-calendar" || case == "mixed-carrier" {
                assert_eq!(
                    get(refusal, "schema_version").as_str(),
                    Some("tos_claim_coordinate_source_refusal_v1")
                );
                assert_eq!(get(refusal, "source_temporal_packet"), &JsonValue::Null);
            }
        }
        // Invalid whole queries cannot become an empty success behind metadata.
        for invalid in ["vacuous", "float-cutoff"] {
            let mut f = temporal_filter(8);
            if invalid == "vacuous" {
                f["roles"] = json!([]);
            } else {
                f["roles"][0]["cutoff"]["value"] = json!(1.0);
            }
            assert_eq!(
                temporal_page(
                    json!({"subject_ref":"tos.event.no-match","temporal":f}),
                    JsonValue::Null,
                    64
                )
                .err()
                .unwrap()
                .code,
                SearchV2ErrorCode::InvalidRequest
            );
        }
        if case == "conflicting-dates" {
            let request = object(vec![
                ("schema_version", text("tos_claim_list_request_v1")),
                ("source_revision", revision.clone()),
                (
                    "filter",
                    parse(&serde_json::to_vec(&json!({"temporal":temporal_filter(100)})).unwrap()),
                ),
                ("page_size", parse(b"1")),
                ("scan_limit", parse(b"64")),
                ("after", JsonValue::Null),
            ]);
            let mut one_row = read_budget();
            one_row.max_matches = 1;
            let page = execute_selected_claim_list(
                &mut model,
                &bound,
                &mut authority(),
                &request,
                one_row,
            )
            .unwrap();
            assert_eq!(get(&parse(&page), "rows").as_array().unwrap().len(), 1);
            let mut logical_limited = request.clone();
            set(&mut logical_limited, "scan_limit", parse(b"2000"));
            let mut one_comparison = read_budget();
            one_comparison.json.max_visits = 2048;
            one_comparison.max_rows = 4000;
            assert!(matches!(
                execute_selected_claim_list(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &logical_limited,
                    one_comparison
                ),
                Err(SearchV2Error {
                    code: SearchV2ErrorCode::BudgetExceeded,
                    ..
                })
            ));
        }
        assert!(
            saw_empty_progress,
            "non-Claim/nonmatch gaps must remain resumable"
        );
        let continued = continuation.unwrap();
        let mut changed_index = continued.clone();
        let mut cursor = get(&continued, "after").clone();
        set(&mut cursor, "index_sha256", text(&"0".repeat(64)));
        set(&mut changed_index, "after", cursor);
        let mut changed_filter = continued.clone();
        set(
            &mut changed_filter,
            "filter",
            object(vec![("subject_ref", text("tos.event.other-filter"))]),
        );
        let mut unknown_field = continued.clone();
        set(
            &mut unknown_field,
            "filter",
            object(vec![("members", JsonValue::Array(vec![]))]),
        );
        let mut bad_count = continued.clone();
        set(&mut bad_count, "scan_limit", parse(b"1.0"));
        for (bad, code) in [
            (changed_index, SearchV2ErrorCode::StaleSelection),
            (changed_filter, SearchV2ErrorCode::InvalidRequest),
            (unknown_field, SearchV2ErrorCode::InvalidRequest),
            (bad_count, SearchV2ErrorCode::InvalidRequest),
        ] {
            match execute_selected_claim_list(
                &mut model,
                &bound,
                &mut authority(),
                &bad,
                read_budget(),
            ) {
                Err(reason) => assert_eq!(reason.code, code),
                Ok(_) => panic!("invalid list boundary accepted"),
            }
        }
        for key in [
            "frozen_chronology",
            "attributed_knowledge",
            "world_valid_time",
            "system_as_of",
            "source_publication_as_of",
        ] {
            let mut unsupported = continued.clone();
            set(
                &mut unsupported,
                "filter",
                object(vec![(key, JsonValue::Null)]),
            );
            assert!(matches!(
                execute_selected_claim_list(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &unsupported,
                    read_budget()
                ),
                Err(SearchV2Error {
                    code: SearchV2ErrorCode::UnsupportedModel,
                    ..
                })
            ));
        }
        let mut changed_policy = authority();
        changed_policy.policy.policy_epoch = "synthetic-new-current-policy".into();
        changed_policy.scope.policy_epoch = changed_policy.policy.policy_epoch.clone();
        assert!(matches!(
            execute_selected_claim_list(
                &mut model,
                &bound,
                &mut changed_policy,
                &continued,
                read_budget()
            ),
            Err(SearchV2Error {
                code: SearchV2ErrorCode::StalePolicy,
                ..
            })
        ));
        let mut denied = authority();
        denied.withdrawn = true;
        assert!(matches!(
            execute_selected_claim_list(&mut model, &bound, &mut denied, &continued, read_budget()),
            Err(SearchV2Error {
                code: SearchV2ErrorCode::StalePolicy,
                ..
            })
        ));
        let mut wrong_operation = Authority::new(
            &bound,
            tos_query::TEMPORAL_OPERATION,
            tos_query::TEMPORAL_INTENDED_USE,
        );
        assert!(
            execute_selected_claim_list(
                &mut model,
                &bound,
                &mut wrong_operation,
                &continued,
                read_budget()
            )
            .is_err()
        );
        let mut tiny = read_budget();
        tiny.max_rows = 1;
        assert!(matches!(
            execute_selected_claim_list(&mut model, &bound, &mut authority(), &continued, tiny),
            Err(SearchV2Error {
                code: SearchV2ErrorCode::BudgetExceeded,
                ..
            })
        ));
        if case == "conflicting-dates" {
            use tos_query::execute_selected_claim_integer_wire;
            let outer = |payload: JsonValue| {
                object(vec![
                    ("schema_version", text("tos_exact_integer_wire_v1")),
                    ("integer_encoding", text("SafeJsonNumberV1")),
                    ("operation", text("tos.knowledge.temporal.list_claims")),
                    ("payload", payload),
                ])
            };
            let mut first = continued.clone();
            set(&mut first, "after", JsonValue::Null);
            let wide_source_request = outer(first.clone());
            let mut packet = execute_selected_claim_integer_wire(
                &mut model,
                &bound,
                &mut authority(),
                &wide_source_request,
                read_budget(),
            )
            .unwrap();
            packet.recheck().unwrap();
            let refused = parse(&packet);
            assert_eq!(get(&refused, "outcome").as_str(), Some("Unsupported"));
            assert_eq!(
                get(&refused, "representation").as_str(),
                Some("OpaqueRawJson")
            );
            for absent in ["payload", "next", "cursor"] {
                assert!(refused.object_get(absent).is_none());
            }
            let raw = get(&refused, "raw_json").as_str().unwrap();
            assert!(raw.contains("9007199254740993"));
            assert_eq!(
                get(&refused, "emitted_json_sha256").as_str(),
                Some(Digest256::of_bytes(raw.as_bytes()).to_prefixed().as_str())
            );
            // A safe empty page still crosses the actual selected provider.
            set(
                &mut first,
                "filter",
                object(vec![("subject_ref", text("tos.event.no-match"))]),
            );
            let safe_request = outer(first);
            let mut safe = execute_selected_claim_integer_wire(
                &mut model,
                &bound,
                &mut authority(),
                &safe_request,
                read_budget(),
            )
            .unwrap();
            safe.recheck().unwrap();
            let safe_value = parse(&safe);
            assert_eq!(get(&safe_value, "outcome").as_str(), Some("Encoded"));
            assert!(
                get(get(&safe_value, "payload"), "rows")
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            // The native cursor cannot silently enter the opt-in profile.
            assert_eq!(
                execute_selected_claim_integer_wire(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &outer(continued.clone()),
                    read_budget(),
                )
                .err()
                .unwrap()
                .code,
                SearchV2ErrorCode::InvalidRequest
            );
            let mut withdrawn = authority();
            withdrawn.withdrawn = true;
            assert_eq!(
                execute_selected_claim_integer_wire(
                    &mut model,
                    &bound,
                    &mut withdrawn,
                    &wide_source_request,
                    read_budget(),
                )
                .err()
                .unwrap()
                .code,
                SearchV2ErrorCode::StalePolicy
            );
            // The new wire workspace is bounded even when a rejection reads
            // no Claim rows; the native decoded transfer counter alone cannot
            // account for its retained raw_json/tree/emission temporaries.
            for request in [&wide_source_request, &safe_request] {
                let mut tiny_wire = read_budget();
                tiny_wire.max_decoded_bytes = 1;
                assert_eq!(
                    execute_selected_claim_integer_wire(
                        &mut model,
                        &bound,
                        &mut authority(),
                        request,
                        tiny_wire,
                    )
                    .err()
                    .unwrap()
                    .code,
                    SearchV2ErrorCode::BudgetExceeded
                );
            }
            if let Some(node) = std::env::var_os("TOS_INTEGER_WIRE_NODE") {
                let js = |bytes: &[u8]| client_json_roundtrip(&node, bytes);
                let outer_roundtrip = parse(&js(&packet));
                assert_eq!(get(&outer_roundtrip, "raw_json"), get(&refused, "raw_json"));
                assert_eq!(
                    get(&outer_roundtrip, "emitted_json_sha256"),
                    get(&refused, "emitted_json_sha256")
                );
                let repeated_request = parse(&js(&canonical(&safe_request)));
                let repeated = execute_selected_claim_integer_wire(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &repeated_request,
                    read_budget(),
                )
                .unwrap();
                assert_eq!(canonical(&parse(&js(&repeated))), canonical(&safe_value));
            }
        }
        if case == "mixed-carrier" {
            if let Some(node) = std::env::var_os("TOS_INTEGER_WIRE_NODE") {
                use tos_query::execute_selected_claim_integer_wire;
                // Both source Claims contain only Safe numeric leaves. The
                // temporal carrier is opaque here: this is metadata pagination.
                let native_request = object(vec![
                    ("schema_version", text("tos_claim_list_request_v1")),
                    ("source_revision", revision.clone()),
                    ("filter", object(vec![])),
                    ("page_size", parse(b"64")),
                    ("scan_limit", parse(b"64")),
                    ("after", JsonValue::Null),
                ]);
                let mut native = execute_selected_claim_list(
                    &mut model,
                    &bound,
                    &mut authority(),
                    &native_request,
                    read_budget(),
                )
                .unwrap();
                native.recheck().unwrap();
                let native_value = parse(&native);
                let expected = get(&native_value, "rows").as_array().unwrap();
                assert_eq!(expected.len(), 2);
                assert_eq!(get(&native_value, "exhausted"), &JsonValue::Bool(true));
                let mut payload = native_request.clone();
                set(&mut payload, "page_size", parse(b"1"));
                let mut found = Vec::new();
                let mut consumed_cursor = false;
                let mut ended = false;
                for _ in 0..=get(&graph, "nodes").as_array().unwrap().len() {
                    let request = object(vec![
                        ("schema_version", text("tos_exact_integer_wire_v1")),
                        ("integer_encoding", text("SafeJsonNumberV1")),
                        ("operation", text("tos.knowledge.temporal.list_claims")),
                        ("payload", payload.clone()),
                    ]);
                    let client_request = parse(&client_json_roundtrip(&node, &canonical(&request)));
                    assert_eq!(canonical(&client_request), canonical(&request));
                    if !get(&payload, "after").is_null() {
                        consumed_cursor = true;
                        let mut withdrawn = authority();
                        withdrawn.withdrawn = true;
                        assert_eq!(
                            execute_selected_claim_integer_wire(
                                &mut model,
                                &bound,
                                &mut withdrawn,
                                &client_request,
                                read_budget(),
                            )
                            .err()
                            .unwrap()
                            .code,
                            SearchV2ErrorCode::StalePolicy
                        );
                    }
                    let mut packet = execute_selected_claim_integer_wire(
                        &mut model,
                        &bound,
                        &mut authority(),
                        &client_request,
                        read_budget(),
                    )
                    .unwrap();
                    packet.recheck().unwrap();
                    let page = parse(&client_json_roundtrip(&node, &packet));
                    assert_eq!(canonical(&page), canonical(&parse(&packet)));
                    assert_eq!(get(&page, "outcome").as_str(), Some("Encoded"));
                    assert_eq!(
                        get(&page, "representation").as_str(),
                        Some("StructuredJson")
                    );
                    let value = get(&page, "payload");
                    assert_eq!(get(value, "source_revision"), &revision);
                    let rows = get(value, "rows").as_array().unwrap();
                    assert!(rows.len() <= 1);
                    if found.is_empty() {
                        assert_eq!(rows.len(), 1);
                    }
                    for row in rows {
                        let index = found.len();
                        assert!(index < expected.len());
                        // The transport preserves native source order and all
                        // exact source refs; it cannot skip/repeat a Claim.
                        assert_eq!(canonical(row), canonical(&expected[index]));
                        assert!(!found.contains(&get(row, "node_id").clone()));
                        found.push(get(row, "node_id").clone());
                        let source = get(get(get(row, "carrier"), "attributes"), "source_claim");
                        let reference = get(row, "claim_ref");
                        assert_eq!(get(reference, "id"), get(source, "claim_id"));
                        assert_eq!(get(reference, "version"), get(source, "claim_version"));
                        assert_eq!(
                            get(reference, "digest").as_str().unwrap(),
                            format!(
                                "sha256:{}",
                                Digest256::of_bytes(&canonical(source)).to_hex()
                            )
                        );
                    }
                    let next = get(value, "next").clone();
                    if next.is_null() {
                        assert_eq!(get(value, "exhausted"), &JsonValue::Bool(true));
                        ended = true;
                        break;
                    }
                    assert_eq!(get(value, "exhausted"), &JsonValue::Bool(false));
                    assert_eq!(
                        get(&next, "schema_version").as_str(),
                        Some("tos_integer_wire_cursor_v1")
                    );
                    assert_eq!(
                        get(&next, "integer_encoding").as_str(),
                        Some("SafeJsonNumberV1")
                    );
                    assert_eq!(
                        get(&next, "operation").as_str(),
                        Some("tos.knowledge.temporal.list_claims")
                    );
                    assert_eq!(
                        get(get(&next, "native_cursor"), "source_revision"),
                        &revision
                    );
                    // Use the ACTUAL returned cursor after client JSON, then
                    // reject a profile mutation before a valid continuation.
                    let mut bad_cursor = next.clone();
                    set(
                        &mut bad_cursor,
                        "integer_encoding",
                        text("CanonicalDecimalStringV1"),
                    );
                    let mut bad = request.clone();
                    let mut bad_payload = payload.clone();
                    set(&mut bad_payload, "after", bad_cursor);
                    set(&mut bad, "payload", bad_payload);
                    assert_eq!(
                        execute_selected_claim_integer_wire(
                            &mut model,
                            &bound,
                            &mut authority(),
                            &bad,
                            read_budget(),
                        )
                        .err()
                        .unwrap()
                        .code,
                        SearchV2ErrorCode::InvalidRequest
                    );
                    assert_ne!(get(&payload, "after"), &next);
                    set(&mut payload, "after", next);
                }
                assert!(ended && consumed_cursor);
                assert_eq!(found.len(), expected.len());
                eprintln!(
                    "Safe integer wire nonempty Node pagination: exact Claims={}, returned cursor consumed",
                    found.len()
                );
            }
        }
        assert_eq!(
            model_digest(),
            model_before,
            "selected read must preserve physical model bytes"
        );
    }
}
