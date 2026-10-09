#![cfg(not(target_arch = "wasm32"))]
//! Native producer and exact selected reads with explicit owner expectations.
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
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
    AbortProbe, AbortReason, BoundCmpKnowledge, IndexedDisclosureScope, InspectBudget,
    InspectCurrentAuthority, InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier,
    bind_verified_knowledge, compare_temporal_operands, execute_selected_temporal,
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
fn err(code: SearchV2ErrorCode) -> SearchV2Error {
    SearchV2Error {
        code,
        message: "synthetic checkpoint admission",
    }
}
struct Probe {
    calls: AtomicUsize,
    after: usize,
    reason: AbortReason,
}
impl AbortProbe for Probe {
    fn reason(&self) -> Option<AbortReason> {
        (self.calls.fetch_add(1, Ordering::Relaxed) >= self.after).then_some(self.reason)
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
                        assert!(
                            get(page, "returned_nodes")
                                .as_u64()
                                .is_some_and(|count| count <= 1),
                            "page node cap changed for {focus_id}/{profile}/{direction}"
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
