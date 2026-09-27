#![cfg(not(target_arch = "wasm32"))]
//! Entire native selected packets against the maintained independent Python engine.
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tos_compiler::knowledge_full_fixture::build_native_fixture;
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};
use tos_query::{
    BoundCmpKnowledge, IndexedDisclosureScope, InspectBudget, InspectCurrentAuthority,
    InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier, bind_verified_knowledge,
    knowledge_focus::{FocusDirection, FocusProfile, KnowledgeFocusRequest},
    knowledge_lens::{
        FOCUS_INTENDED_USE, FOCUS_OPERATION, LENS_INTENDED_USE, LENS_OPERATION, LensBudget,
        STORED_LENS_INTENDED_USE, STORED_LENS_OPERATION, execute_selected_focus,
        execute_selected_lens, execute_selected_stored_lens, lens_continuation_binding,
    },
    search_v2::{CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode},
};

fn withdrawal() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::StalePolicy,
        message: "synthetic lens withdrawal",
    }
}
struct Lease(Arc<AtomicBool>);
impl InspectDisclosureLease for Lease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        if self.0.load(Ordering::SeqCst) {
            Err(withdrawal())
        } else {
            Ok(())
        }
    }
}
struct Authority {
    scope: IndexedDisclosureScope,
    policy: CurrentPolicyBinding,
    withdrawn: Arc<AtomicBool>,
    consulted: Vec<String>,
    catalog_denied: bool,
    catalog_consulted: usize,
}
impl Authority {
    fn new(bound: &BoundCmpKnowledge<'_>) -> Self {
        let policy = CurrentPolicyBinding {
            scope: "synthetic-lens".into(),
            issuer_ref: "synthetic-issuer".into(),
            authorization_receipt_id: "synthetic-receipt".into(),
            policy_epoch: "synthetic-epoch".into(),
            withdrawal_generation: "synthetic-withdrawal".into(),
        };
        Self {
            scope: IndexedDisclosureScope {
                operation_id: LENS_OPERATION.into(),
                carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                intended_use: LENS_INTENDED_USE.into(),
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
            withdrawn: Arc::new(AtomicBool::new(false)),
            consulted: vec![],
            catalog_denied: false,
            catalog_consulted: 0,
        }
    }
}
impl InspectCurrentAuthority for Authority {
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.scope.clone()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        if self.withdrawn.load(Ordering::SeqCst) {
            Err(withdrawal())
        } else {
            Ok(())
        }
    }
    fn authorize_current(&mut self, carrier: &InspectedCarrier) -> Result<(), SearchV2Error> {
        self.check_selected()?;
        self.consulted.push(carrier.id.clone());
        Ok(())
    }
    fn authorize_catalog_current(
        &mut self,
        _: tos_foundation::Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check_selected()?;
        if self.catalog_denied {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::PolicyBindingUnavailable,
                message: "synthetic catalog denial",
            });
        }
        self.catalog_consulted += 1;
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        _: &IndexedDisclosureScope,
        consulted: &[ObservedInspectCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease>, SearchV2Error> {
        self.check_selected()?;
        if self.scope.operation_id == STORED_LENS_OPERATION {
            assert_eq!(self.catalog_consulted, 1);
        }
        assert_eq!(
            consulted.iter().map(|r| &r.id).collect::<Vec<_>>(),
            self.consulted.iter().collect::<Vec<_>>()
        );
        Ok(Box::new(Lease(self.withdrawn.clone())))
    }
}
fn budget() -> LensBudget {
    LensBudget {
        inspect: InspectBudget {
            max_open_vm_steps: 100_000_000,
            max_read_vm_steps: 20_000_000,
            max_matches: 1000,
            max_rows: 100_000,
            max_field_bytes: 8192,
            max_payload_bytes: 1_000_000,
            max_decoded_bytes: 128_000_000,
            max_response_bytes: 8_000_000,
            json: JsonLimits::default(),
        },
        max_candidates: 100_000,
        max_path_steps: 100_000,
        max_adjacency_rows: 100_000,
        block_size: 16,
    }
}
fn field<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap()
}
fn canonical(v: &JsonValue) -> Vec<u8> {
    canonical_bytes_v1(
        v,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap()
}
fn focus_request(value: &JsonValue) -> KnowledgeFocusRequest {
    let mut request = KnowledgeFocusRequest::new(field(value, "node_id").as_str().unwrap());
    if let Some(v) = value.object_get("sources") {
        request.sources = Some(
            v.as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect(),
        );
    }
    for (key, slot) in [
        ("depth", &mut request.depth),
        ("node_limit", &mut request.node_limit),
        ("relation_limit", &mut request.relation_limit),
    ] {
        if let Some(v) = value.object_get(key) {
            *slot = v.as_u64().unwrap() as usize;
        }
    }
    if let Some(v) = value.object_get("direction") {
        request.direction = match v.as_str().unwrap() {
            "incoming" => FocusDirection::Incoming,
            "outgoing" => FocusDirection::Outgoing,
            "either" => FocusDirection::Either,
            _ => panic!("direction"),
        };
    }
    if let Some(v) = value.object_get("profile") {
        request.profile = match v.as_str().unwrap() {
            "all" => FocusProfile::All,
            "overview" => FocusProfile::Overview,
            _ => panic!("profile"),
        };
    }
    request
}
#[test]
fn normalized_selected_lenses_match_independent_python_and_hold_current_disclosure() {
    let fixture = build_native_fixture();
    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let publication = lens_continuation_binding(&bound, &Authority::new(&bound).scope);
    let catalog: Vec<u8> = cold
        .connection()
        .query_row("SELECT packet FROM catalog_index_meta", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&catalog),
        bound.selection().catalog_packet_sha256
    );
    let mut child = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/selected_lens_python_oracle.py"
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("independent Python oracle");
    let mut input = child.stdin.take().unwrap();
    input.write_all(b"[").unwrap();
    input.write_all(&fixture.graph_input_bytes).unwrap();
    input.write_all(b",").unwrap();
    input.write_all(&fixture.descriptor_bytes).unwrap();
    input.write_all(b",").unwrap();
    input.write_all(&canonical(&publication)).unwrap();
    input.write_all(b",").unwrap();
    input.write_all(&catalog).unwrap();
    input.write_all(b"]").unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "Python oracle failed");
    let oracle = parse_json(
        &output.stdout,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 64 * 1024 * 1024,
            max_visits: 10_000_000,
            ..JsonLimits::default()
        },
    )
    .unwrap()
    .into_root();
    let mut model = cold
        .fork_reader_with_vm_budget(budget().inspect.max_read_vm_steps)
        .unwrap();
    let cases = field(&oracle, "cases").as_array().unwrap();
    assert!(cases.len() >= 40, "bounded operation coverage");
    for case in cases {
        let name = field(case, "name").as_str().unwrap();
        let mut authority = Authority::new(&bound);
        let operation = case
            .object_get("operation")
            .and_then(JsonValue::as_str)
            .unwrap_or("compile");
        let result = if operation == "focus" {
            authority.scope.operation_id = FOCUS_OPERATION.into();
            authority.scope.intended_use = FOCUS_INTENDED_USE.into();
            execute_selected_focus(
                &mut model,
                &bound,
                &mut authority,
                &focus_request(field(case, "request")),
                budget(),
            )
        } else if operation == "stored" {
            authority.scope.operation_id = STORED_LENS_OPERATION.into();
            authority.scope.intended_use = STORED_LENS_INTENDED_USE.into();
            let result = execute_selected_stored_lens(
                &mut model,
                &bound,
                &mut authority,
                field(case, "identifier").as_str().unwrap(),
                budget(),
            );
            assert_eq!(authority.catalog_consulted, 1);
            result
        } else {
            execute_selected_lens(
                &mut model,
                &bound,
                &mut authority,
                field(case, "spec"),
                budget(),
            )
        };
        if let Some(error) = case.object_get("error") {
            let expected = if error.as_str() == Some("unknown") {
                SearchV2ErrorCode::UnknownIdentifier
            } else if error.as_str() == Some("stale") {
                SearchV2ErrorCode::StaleSelection
            } else {
                SearchV2ErrorCode::InvalidRequest
            };
            assert!(
                matches!(result, Err(ref error) if error.code == expected),
                "{name}: expected {expected:?}"
            );
            continue;
        }
        let mut result = result.unwrap_or_else(|error| panic!("{name}: {error:?}"));
        result.recheck().unwrap();
        let actual = parse_json(&result, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        assert_eq!(
            canonical(actual.root()),
            canonical(field(case, "packet")),
            "{name}"
        );
        authority.withdrawn.store(true, Ordering::SeqCst);
        assert!(
            matches!(
                result.recheck(),
                Err(SearchV2Error {
                    code: SearchV2ErrorCode::StalePolicy,
                    ..
                })
            ),
            "{name}: retained disclosure lease must see withdrawal"
        );
    }
    let mut wrong_scope = Authority::new(&bound);
    let focus = KnowledgeFocusRequest::new(
        field(field(&cases[0], "packet"), "nodes")
            .as_array()
            .unwrap()[0]
            .object_get("id")
            .unwrap()
            .as_str()
            .unwrap(),
    );
    assert!(matches!(
        execute_selected_focus(&mut model, &bound, &mut wrong_scope, &focus, budget()),
        Err(SearchV2Error {
            code: SearchV2ErrorCode::PolicyBindingUnavailable,
            ..
        })
    ));
    let mut denied_catalog = Authority::new(&bound);
    denied_catalog.scope.operation_id = STORED_LENS_OPERATION.into();
    denied_catalog.scope.intended_use = STORED_LENS_INTENDED_USE.into();
    denied_catalog.catalog_denied = true;
    assert!(matches!(
        execute_selected_stored_lens(
            &mut model,
            &bound,
            &mut denied_catalog,
            "fixture-absent-lens",
            budget()
        ),
        Err(SearchV2Error {
            code: SearchV2ErrorCode::PolicyBindingUnavailable,
            ..
        })
    ));
    // A current owner may issue a new policy for the same immutable bytes.
    // Old cursors must then restart even though the content fingerprint is equal.
    let continuation = cases
        .iter()
        .find(|c| field(c, "name").as_str() == Some("continuation-1"))
        .unwrap();
    for epoch in [true, false] {
        let mut authority = Authority::new(&bound);
        if epoch {
            authority.policy.policy_epoch.push_str("-new");
            authority.scope.policy_epoch = authority.policy.policy_epoch.clone();
        } else {
            authority.policy.withdrawal_generation.push_str("-new");
            authority.scope.withdrawal_generation = authority.policy.withdrawal_generation.clone();
        }
        assert!(matches!(
            execute_selected_lens(
                &mut model,
                &bound,
                &mut authority,
                field(continuation, "spec"),
                budget()
            ),
            Err(SearchV2Error {
                code: SearchV2ErrorCode::StaleSelection,
                ..
            })
        ));
    }
    let spec = field(&cases[0], "spec");
    for small in [
        LensBudget {
            max_candidates: 1,
            ..budget()
        },
        LensBudget {
            inspect: InspectBudget {
                max_rows: 1,
                ..budget().inspect
            },
            block_size: 1,
            ..budget()
        },
        LensBudget {
            inspect: InspectBudget {
                max_decoded_bytes: 1,
                ..budget().inspect
            },
            ..budget()
        },
        LensBudget {
            inspect: InspectBudget {
                max_response_bytes: 1,
                ..budget().inspect
            },
            ..budget()
        },
        LensBudget {
            inspect: InspectBudget {
                max_read_vm_steps: 1,
                ..budget().inspect
            },
            ..budget()
        },
    ] {
        let mut authority = Authority::new(&bound);
        assert!(matches!(
            execute_selected_lens(&mut model, &bound, &mut authority, spec, small),
            Err(SearchV2Error {
                code: SearchV2ErrorCode::BudgetExceeded,
                ..
            })
        ));
    }
    let path = cases
        .iter()
        .find(|c| field(c, "name").as_str() == Some("path-revisit"))
        .unwrap();
    let mut authority = Authority::new(&bound);
    assert!(matches!(
        execute_selected_lens(
            &mut model,
            &bound,
            &mut authority,
            field(path, "spec"),
            LensBudget {
                max_path_steps: 1,
                ..budget()
            }
        ),
        Err(SearchV2Error {
            code: SearchV2ErrorCode::BudgetExceeded,
            ..
        })
    ));
}

/// Same genuine normalized producer and current-disclosure seam, narrowed to
/// the maintained legacy search operation; no new corpus or fixture producer.
#[test]
fn normalized_selected_legacy_search_matches_python_packets_and_exact_counts() {
    use tos_query::knowledge_legacy_search::{
        LEGACY_SEARCH_INTENDED_USE, LEGACY_SEARCH_OPERATION, LegacySearchBudget,
        LegacySearchRequest, SEARCH_CAPABILITIES_INTENDED_USE, SEARCH_CAPABILITIES_OPERATION,
        execute_selected_legacy_search, execute_selected_search_capabilities,
    };
    let fixture = build_native_fixture();
    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let caps = LegacySearchBudget {
        inspect: budget().inspect,
        document: tos_query::SearchDocumentBudget {
            max_carrier_bytes: 1_000_000,
            max_document_bytes: 4_000_000,
            max_document_code_points: 1_000_000,
            json: JsonLimits::default(),
        },
        max_candidates: 100_000,
        max_document_bytes: 128_000_000,
        max_document_code_points: 128_000_000,
        max_retained_per_kind: 100_100,
        max_retained_bytes: 8_000_000,
        block_size: 16,
    };
    let script = r#"
import json,sys
from pathlib import Path
sys.path.insert(0,sys.argv[1])
from tos_access import knowledge as k
from tos_access.core import ToSAccessCore
graph,descriptor=json.load(sys.stdin)
k.KNOWLEDGE_SOURCES=tuple(s['source_graph_id'] for s in descriptor['sources'])
first=graph['nodes'][0]
requests=[{}, {'query':' '}, {'query':': '}, {'query':graph['source_revision']},
          {'query':first['id']}, {'query':first['native_id'].upper()},
          {'query':first['source_refs'][0]}, {'query':first['native_id'][:1]},
          {'query':'\u2003'+first['native_id']+'\u001c'},
          {'offset':1,'limit':1}, {'offset':100_000}, {'sources':[]},
          {'sources':['',first['source_graph'],first['source_graph']]},
          {'kind_ids':[first['kind_id'],'',first['kind_id']]},
          {'kind_ids':['unregistered-fixture-kind']},
          {'predicate_ids':[graph['relations'][0]['predicate_id']]},
          {'predicate_ids':['unregistered-fixture-predicate']},
          {'query':'x'*257}, {'offset':100_001}, {'limit':0},
          {'sources':['unregistered-fixture-source']}]
requests += [{'sources':[source]} for source in k.KNOWLEDGE_SOURCES]
cases=[]
for request in requests:
    try: packet=k.search_knowledge_graph(graph,**request)
    except ValueError: cases.append({'request':request,'error':'invalid'})
    else: cases.append({'request':request,'packet':packet})
# Exact maintained engine-selection profile; does not create a public grant.
class SelectedEngine:
    _prepared_reader=None
    _data_guard=None
    def _query_store(self): return None
json.dump({'cases':cases,'capabilities':ToSAccessCore.knowledge_search_capabilities(SelectedEngine())},sys.stdout,ensure_ascii=False)
"#;
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../access/src"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(b"[").unwrap();
    input.write_all(&fixture.graph_input_bytes).unwrap();
    input.write_all(b",").unwrap();
    input.write_all(&fixture.descriptor_bytes).unwrap();
    input.write_all(b"]").unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let oracle = parse_json(
        &output.stdout,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 32_000_000,
            max_visits: 4_000_000,
            ..JsonLimits::default()
        },
    )
    .unwrap()
    .into_root();
    let mut model = cold
        .fork_reader_with_vm_budget(caps.inspect.max_read_vm_steps)
        .unwrap();
    let authority_for = |operation: &str, intended: &str| {
        let mut authority = Authority::new(&bound);
        authority.scope.operation_id = operation.into();
        authority.scope.intended_use = intended.into();
        authority
    };
    let mut first_request = None;
    for case in field(&oracle, "cases").as_array().unwrap() {
        let raw = field(case, "request");
        let mut request = LegacySearchRequest::default();
        if let Some(v) = raw.object_get("query") {
            request.query = v.as_str().unwrap().into();
        }
        for (key, slot) in [
            ("kind_ids", &mut request.kind_ids),
            ("predicate_ids", &mut request.predicate_ids),
        ] {
            if let Some(v) = raw.object_get(key) {
                *slot = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect();
            }
        }
        if let Some(v) = raw.object_get("sources") {
            request.sources = Some(
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect(),
            );
        }
        if let Some(v) = raw.object_get("offset") {
            request.offset = v.as_u64().unwrap() as usize;
        }
        if let Some(v) = raw.object_get("limit") {
            request.limit = v.as_u64().unwrap() as usize;
        }
        let mut authority = authority_for(LEGACY_SEARCH_OPERATION, LEGACY_SEARCH_INTENDED_USE);
        let result =
            execute_selected_legacy_search(&mut model, &bound, &mut authority, &request, caps);
        if case.object_get("error").is_some() {
            assert!(
                matches!(result,Err(ref e) if e.code==SearchV2ErrorCode::InvalidRequest),
                "{raw:?}"
            );
            continue;
        }
        let mut result = result.unwrap_or_else(|e| panic!("{raw:?}: {e:?}"));
        result.recheck().unwrap();
        let actual = parse_json(&result, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        assert_eq!(
            canonical(actual.root()),
            canonical(field(case, "packet")),
            "{raw:?}"
        );
        // Even filtered, skipped-offset and nonmatching carriers were consulted.
        if request.sources.as_ref().is_none_or(|s| s.is_empty()) {
            assert!(!authority.consulted.is_empty());
        }
        authority.withdrawn.store(true, Ordering::SeqCst);
        assert!(matches!(result.recheck(),Err(ref e) if e.code==SearchV2ErrorCode::StalePolicy));
        first_request.get_or_insert(request);
    }
    let mut authority = authority_for(
        SEARCH_CAPABILITIES_OPERATION,
        SEARCH_CAPABILITIES_INTENDED_USE,
    );
    let mut result =
        execute_selected_search_capabilities(&mut model, &bound, &mut authority, caps.inspect)
            .unwrap();
    assert_eq!(
        canonical(
            parse_json(&result, JsonMode::PublishedStrict, JsonLimits::default())
                .unwrap()
                .root()
        ),
        canonical(field(&oracle, "capabilities"))
    );
    authority.withdrawn.store(true, Ordering::SeqCst);
    assert!(matches!(result.recheck(),Err(ref e) if e.code==SearchV2ErrorCode::StalePolicy));
    let request = first_request.unwrap();
    for narrow in [
        LegacySearchBudget {
            max_candidates: 1,
            ..caps
        },
        LegacySearchBudget {
            max_document_bytes: 1,
            ..caps
        },
        LegacySearchBudget {
            max_retained_bytes: 1,
            ..caps
        },
        LegacySearchBudget {
            max_retained_per_kind: 1,
            ..caps
        },
    ] {
        let mut authority = authority_for(LEGACY_SEARCH_OPERATION, LEGACY_SEARCH_INTENDED_USE);
        assert!(
            matches!(execute_selected_legacy_search(&mut model,&bound,&mut authority,&request,narrow),Err(ref e) if e.code==SearchV2ErrorCode::BudgetExceeded)
        );
    }
    let mut wrong_scope = Authority::new(&bound);
    assert!(
        execute_selected_legacy_search(&mut model, &bound, &mut wrong_scope, &request, caps)
            .is_err()
    );
}
