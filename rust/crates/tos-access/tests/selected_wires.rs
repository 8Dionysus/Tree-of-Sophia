#![cfg(not(target_arch = "wasm32"))]

use std::{
    fs,
    fs::File,
    io::{Cursor, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tos_access::{
    AccessError, AccessExecutor, AccessProfile, Params, PreparedPacket, QuerySession, cli,
    http::{handle_get, serve_connection},
    mcp::run_io,
};
use tos_compiler::{
    CandidateReceipt, ImmutableModelCustody, LegacyPartitionedNavigation, Limits,
    PublicationAuthority, SelectedExpectation, SelectionFence, SourceBinding, VerifiedSelection,
    compile_navigation, open_selected_model, publish_candidate,
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
use tos_query::{
    AbortProbe, AdapterAdmissionBudget, Budget, Charged, CmpPinnedModel, CurrentPolicy,
    DisclosureLease, DisclosureScope, PinnedLocalModel, QueryError, QueryErrorCode, RawRecord,
    SourcePin, SqliteReadModel,
};

struct FixtureCustody;
impl ImmutableModelCustody for FixtureCustody {
    fn verify_held(&self, pinned: &File, _: &str, size_bytes: u64) -> tos_compiler::Result<()> {
        if pinned.metadata()?.len() != size_bytes {
            return Err(tos_compiler::Error::Invalid("fixture custody size"));
        }
        Ok(())
    }
}

struct FixtureLease;
impl DisclosureLease for FixtureLease {
    fn recheck(&mut self) -> Result<(), QueryError> {
        Ok(())
    }
}

const ROOT_SHA: &str = "2d9edbd88ebee606fc6fc4b06e23b43d4c73ccaed15e2ffb171d7518ead2f3e3";
const MEMBERSHIP_ROOT: &str = "e92c74487c3cdcc852cc761711fcd2c826a527321f806da82b936e630b1613b7";

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tos-compiler/tests/fixtures/tos_corpus_index.min.json")
        .canonicalize()
        .expect("trusted test fixture has a normalized absolute path")
}

fn binding() -> SourceBinding {
    SourceBinding {
        owner_profile: "fixture_owner_v1".into(),
        source_cut: "fixture_sealed_cut".into(),
        through_commit_seq: 7,
        membership_root: MEMBERSHIP_ROOT.into(),
        index_generation: "fixture_generation".into(),
        route_map_version: "fixture_route".into(),
        reader_abi: "fixture_reader".into(),
        projection_root_sha256: ROOT_SHA.into(),
        complete: true,
    }
}

struct FixtureOwner;
struct FixtureFence;
impl SelectionFence for FixtureFence {
    fn receipt_id(&self) -> &str {
        "fixture-owner-selection"
    }
    fn recheck_held(&self) -> tos_compiler::Result<()> {
        Ok(())
    }
}
impl PublicationAuthority for FixtureOwner {
    type Fence = FixtureFence;
    fn acquire_selection_fence(
        &mut self,
        _: &SourceBinding,
        _: &CandidateReceipt,
    ) -> tos_compiler::Result<Self::Fence> {
        Ok(FixtureFence)
    }
}

struct FixturePin;
impl SourcePin for FixturePin {
    fn check_sealed_cut(&mut self, selected: &VerifiedSelection) -> Result<(), QueryError> {
        if selected.source_cut != "fixture_sealed_cut" || selected.through_commit_seq != 7 {
            return Err(QueryError {
                code: QueryErrorCode::StaleSelection,
                message: "fixture cut changed",
            });
        }
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        selected: &VerifiedSelection,
    ) -> Result<Box<dyn DisclosureLease>, QueryError> {
        self.check_sealed_cut(selected)?;
        Ok(Box::new(FixtureLease))
    }
}

struct FixturePolicy {
    denied: Option<&'static str>,
}
impl CurrentPolicy for FixturePolicy {
    fn authorize_current(
        &mut self,
        _: &DisclosureScope,
        record: &RawRecord,
    ) -> Result<Charged, QueryError> {
        if let Some(denied) = self.denied {
            let parsed = parse_json(
                &record.raw,
                JsonMode::PublishedStrict,
                JsonLimits::default(),
            )
            .unwrap();
            if parsed
                .root()
                .object_get("node_id")
                .and_then(JsonValue::as_str)
                == Some(denied)
            {
                return Err(QueryError {
                    code: QueryErrorCode::PolicyDenied,
                    message: "fixture rights withdrawn",
                });
            }
        }
        Ok(Charged {
            probes: 1,
            cpu_steps: 1,
            ..Charged::default()
        })
    }
    fn acquire_disclosure(
        &mut self,
        scope: &DisclosureScope,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease>, QueryError> {
        for record in selected {
            self.authorize_current(scope, record)?;
        }
        Ok(Box::new(FixtureLease))
    }
}

fn selected(
    denied: Option<&'static str>,
) -> (
    PathBuf,
    SqliteReadModel<CmpPinnedModel<FixturePin>, FixturePolicy>,
) {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-qry-cmp-{}-{tick}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("candidate.sqlite3");
    let mut input =
        LegacyPartitionedNavigation::open(&fixture(), ROOT_SHA, Limits::default()).unwrap();
    let receipt = compile_navigation(&mut input, &binding(), &path, Limits::default()).unwrap();
    publish_candidate(&path, &binding(), &receipt, &dir, None, &mut FixtureOwner).unwrap();
    let verified = open_selected_model(
        &dir,
        &SelectedExpectation {
            source: &binding(),
            model_sha256: &receipt.sqlite_sha256,
            model_size_bytes: receipt.sqlite_size_bytes,
            owner_receipt_id: "fixture-owner-selection",
            custody: Arc::new(FixtureCustody),
            max_cold_open_bytes: receipt.sqlite_size_bytes,
            max_cold_open_vm_steps: 1_000_000,
        },
    )
    .unwrap();
    let pinned = CmpPinnedModel::new(verified, FixturePin).unwrap();
    let scope = DisclosureScope {
        operation_id: "tos.source.descend".into(),
        carrier_layer: "tos_source_navigation_public_metadata_v1".into(),
        intended_use: "read_only_public_metadata_navigation_v1".into(),
        selected_binding: pinned.binding().clone(),
        corpus_revision: "fixture-corpus-revision".into(),
        data_revision: "fixture-data-revision".into(),
        selected_export_receipt_id: "fixture-export-receipt".into(),
        selected_model_receipt_id: pinned.owner_receipt_id().into(),
        policy_issuer_ref: "fixture-issuer".into(),
        policy_receipt_id: "fixture-policy-receipt".into(),
        policy_epoch: "fixture-policy-epoch".into(),
        withdrawal_generation: "fixture-withdrawal-generation".into(),
    };
    let model = SqliteReadModel::new(
        pinned,
        FixturePolicy { denied },
        scope,
        AdapterAdmissionBudget {
            max_selected_open_vm_steps: 1_000_000,
            max_metadata_vm_steps: 1_000_000,
            max_metadata_decoded_bytes: 1_000_000,
            max_metadata_rows: 64,
        },
    )
    .unwrap();
    (dir, model)
}

fn budget() -> Budget {
    Budget {
        max_probes: 10_000,
        max_rows: 10_000,
        max_bytes: 1_000_000,
        max_cpu_steps: 1_000_000,
        max_edges: 100,
        max_response_bytes: 100_000,
        max_request_bytes: 4096,
        page_rows: 1,
        json: JsonLimits::default(),
    }
}

type Model = SqliteReadModel<CmpPinnedModel<FixturePin>, FixturePolicy>;
struct SelectedExecutor {
    session: Mutex<QuerySession<Model>>,
}
impl AccessExecutor for SelectedExecutor {
    fn source_descend_available(&self) -> bool {
        true
    }
    fn source_descend(
        &self,
        request: Params,
        abort_probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        let mut session = self.session.lock().unwrap();
        session.model.set_abort_probe(Some(abort_probe));
        let result = session.execute(request);
        session.model.set_abort_probe(None);
        result
    }
}

fn expected() -> JsonValue {
    let oracle = parse_json(
        include_bytes!("../../tos-query/tests/fixtures/cmp_selected_python_oracle.json"),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    oracle.root().object_get("expected").unwrap().clone()
}

fn semantic_eq(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.iter()
                        .find(|(other, _)| other == key)
                        .is_some_and(|(_, other)| semantic_eq(value, other))
                })
        }
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| semantic_eq(a, b))
        }
        _ => left == right,
    }
}

fn assert_packet(bytes: &[u8]) {
    let packet = parse_json(bytes, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert!(semantic_eq(packet.root(), &expected()));
}

#[test]
fn selected_cmp_packet_survives_all_three_native_wire_adapters() {
    let (dir, model) = selected(None);
    let executor = Arc::new(SelectedExecutor {
        session: Mutex::new(QuerySession {
            model,
            budget: budget(),
        }),
    });
    let profile = AccessProfile::new(65_536, 1_048_576, 65_536);

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(
        cli::run_cli(
            &[
                "source".into(),
                "descend".into(),
                "id.alpha".into(),
                "--max-depth".into(),
                "2".into()
            ],
            executor.as_ref(),
            profile,
            &mut stdout,
            &mut stderr
        ),
        0
    );
    assert!(stderr.is_empty());
    assert_packet(stdout.strip_suffix(b"\n").unwrap());

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let http_executor: Arc<dyn AccessExecutor> = executor.clone();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve_connection(stream, http_executor, profile);
    });
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .write_all(
            b"GET /api/source/navigation/id.alpha?max_depth=2 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        )
        .unwrap();
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    server.join().unwrap();
    let header_end = response
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap()
        + 4;
    assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert_packet(&response[header_end..]);

    let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"id.alpha","max_depth":2}}}
"#;
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, executor.as_ref(), profile).unwrap();
    let lines = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    let rpc = parse_json(lines[1], JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let packet = rpc
        .root()
        .object_get("result")
        .unwrap()
        .object_get("structuredContent")
        .unwrap();
    assert!(semantic_eq(packet, &expected()));

    let timed = handle_get(
        executor.as_ref(),
        "GET",
        "/api/source/navigation/id.alpha?max_depth=2",
        profile.with_query_timeout(Duration::ZERO),
    );
    assert_eq!(timed.status, 408);
    assert!(
        std::str::from_utf8(&timed.body)
            .unwrap()
            .contains("deadline_exceeded")
    );

    drop(executor);
    fs::remove_dir_all(dir).unwrap();
}

// Reuse the maintained producer fixture, rather than another transport-only
// packet, for the new selected knowledge binding. Synthetic policy grants
// demonstrate custody/fence mechanics only.
mod selected_knowledge {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tos_access::{KnowledgeOperation as O, KnowledgeRequest as R};
    use tos_compiler::knowledge_full_fixture::{FullKnowledgeFixture, build_fixture};
    use tos_foundation::Digest256;
    use tos_query::knowledge_exploration::{
        ExplorationBudget, ExplorationCheckpoint, ExplorationCheckpoints,
    };
    use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error};
    use tos_query::{
        BoundCmpKnowledge, CatalogBudget, CatalogCurrentAuthority, CatalogDisclosureLease,
        CatalogDisclosureScope, CatalogError, IndexedDisclosureScope, InspectBudget,
        InspectCurrentAuthority, InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier,
        bind_verified_knowledge,
    };
    struct Lease(Arc<AtomicUsize>);
    impl Drop for Lease {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    impl InspectDisclosureLease for Lease {
        fn recheck(&mut self) -> Result<(), SearchV2Error> {
            Ok(())
        }
    }
    impl CatalogDisclosureLease for Lease {
        fn recheck(&mut self) -> Result<(), CatalogError> {
            Ok(())
        }
    }
    struct Authority {
        policy: CurrentPolicyBinding,
        catalog: CatalogDisclosureScope,
        inspect: IndexedDisclosureScope,
        held: Arc<AtomicUsize>,
    }
    impl Authority {
        fn new(bound: &BoundCmpKnowledge<'_>, request: &R, held: Arc<AtomicUsize>) -> Self {
            let policy = CurrentPolicyBinding {
                scope: "synthetic-wire".into(),
                issuer_ref: "synthetic-issuer".into(),
                authorization_receipt_id: "synthetic-receipt".into(),
                policy_epoch: "synthetic-epoch".into(),
                withdrawal_generation: "synthetic-withdrawal".into(),
            };
            let selected = bound.selection();
            let inspect = IndexedDisclosureScope {
                operation_id: request.operation().id().into(),
                carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                intended_use: if request.operation() == O::Explore {
                    "read_only_public_knowledge_exploration_v1"
                } else {
                    "read_only_public_knowledge_inspect_v1"
                }
                .into(),
                selected_model_receipt_id: bound.owner_receipt_id().into(),
                source_cut: selected.source_cut.clone(),
                through_commit_seq: selected.through_commit_seq,
                source_membership_root: selected.source_membership_root,
                descriptor_sha256: selected.vocabulary.descriptor_sha256,
                selected_index_sha256: selected.index_root_sha256,
                policy_issuer_ref: policy.issuer_ref.clone(),
                policy_receipt_id: policy.authorization_receipt_id.clone(),
                policy_scope: policy.scope.clone(),
                policy_epoch: policy.policy_epoch.clone(),
                withdrawal_generation: policy.withdrawal_generation.clone(),
            };
            let catalog = CatalogDisclosureScope {
                operation_id: "tos.knowledge.catalog".into(),
                carrier_layer: inspect.carrier_layer.clone(),
                intended_use: "read_only_public_knowledge_catalog_v1".into(),
                selected_model_receipt_id: inspect.selected_model_receipt_id.clone(),
                source_cut: inspect.source_cut.clone(),
                through_commit_seq: inspect.through_commit_seq,
                source_membership_root: inspect.source_membership_root,
                descriptor_sha256: inspect.descriptor_sha256,
                selected_index_sha256: inspect.selected_index_sha256,
                catalog_packet_sha256: selected.catalog_packet_sha256,
                policy_issuer_ref: policy.issuer_ref.clone(),
                policy_receipt_id: policy.authorization_receipt_id.clone(),
                policy_scope: policy.scope.clone(),
                policy_epoch: policy.policy_epoch.clone(),
                withdrawal_generation: policy.withdrawal_generation.clone(),
            };
            Self {
                policy,
                catalog,
                inspect,
                held,
            }
        }
        fn lease(&self) -> Lease {
            self.held.fetch_add(1, Ordering::SeqCst);
            Lease(Arc::clone(&self.held))
        }
    }
    impl CatalogCurrentAuthority for Authority {
        fn policy_binding(&self) -> CurrentPolicyBinding {
            self.policy.clone()
        }
        fn disclosure_scope(&self) -> CatalogDisclosureScope {
            self.catalog.clone()
        }
        fn check_selected(&mut self) -> Result<(), CatalogError> {
            Ok(())
        }
        fn authorize_current(&mut self, sha: Digest256) -> Result<(), CatalogError> {
            assert_eq!(sha, self.catalog.catalog_packet_sha256);
            Ok(())
        }
        fn acquire_disclosure(
            &mut self,
            _: &CatalogDisclosureScope,
            _: Digest256,
        ) -> Result<Box<dyn CatalogDisclosureLease>, CatalogError> {
            Ok(Box::new(self.lease()))
        }
    }
    impl InspectCurrentAuthority for Authority {
        fn policy_binding(&self) -> CurrentPolicyBinding {
            self.policy.clone()
        }
        fn disclosure_scope(&self) -> IndexedDisclosureScope {
            self.inspect.clone()
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
            observed: &[ObservedInspectCarrier],
        ) -> Result<Box<dyn InspectDisclosureLease>, SearchV2Error> {
            assert!(!observed.is_empty());
            Ok(Box::new(self.lease()))
        }
    }
    struct Executor {
        fixture: FullKnowledgeFixture,
        held: Arc<AtomicUsize>,
        checkpoints: Mutex<tos_access::exploration_checkpoints::ProcessExplorationCheckpoints>,
    }
    impl AccessExecutor for Executor {
        fn source_descend_available(&self) -> bool {
            false
        }
        fn source_descend(
            &self,
            _: Params,
            _: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket, AccessError> {
            unreachable!()
        }
        fn knowledge_available(&self, operation: O) -> bool {
            matches!(operation, O::Catalog | O::Node | O::Relation | O::Explore)
        }
        fn knowledge(
            &self,
            request: R,
            probe: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket, AccessError> {
            let cold = self.fixture.open().unwrap();
            let bound = bind_verified_knowledge(
                &cold,
                &self.fixture.vocabulary,
                &self.fixture.descriptor_bytes,
            )
            .unwrap();
            let mut model = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
            let mut catalog = Authority::new(&bound, &request, Arc::clone(&self.held));
            let mut inspect = Authority::new(&bound, &request, Arc::clone(&self.held));
            let budgets = budgets();
            tos_access::knowledge::execute_selected_knowledge(
                &mut model,
                &bound,
                &mut catalog,
                &mut inspect,
                &mut *self.checkpoints.lock().unwrap(),
                request,
                budgets,
                probe,
            )
        }
    }
    fn budgets() -> tos_access::knowledge::SelectedKnowledgeBudgets {
        let read = InspectBudget {
            max_open_vm_steps: 100_000_000,
            max_read_vm_steps: 1_000_000,
            max_matches: 64,
            max_rows: 1000,
            max_field_bytes: 8192,
            max_payload_bytes: 1_000_000,
            max_decoded_bytes: 8_000_000,
            max_response_bytes: 1_000_000,
            json: JsonLimits::default(),
        };
        tos_access::knowledge::SelectedKnowledgeBudgets {
            catalog: CatalogBudget {
                max_open_vm_steps: 100_000_000,
                max_read_vm_steps: 1_000_000,
                max_packet_bytes: 1_000_000,
                max_decoded_bytes: 1_000_032,
                json: JsonLimits::default(),
            },
            inspect: read,
            lens: tos_query::knowledge_lens::LensBudget {
                inspect: read,
                max_candidates: 1000,
                max_path_steps: 1000,
                max_adjacency_rows: 1000,
                block_size: 64,
            },
            exploration: ExplorationBudget {
                read,
                max_work_units: 1,
                max_session_nodes: 1000,
                max_session_relations: 1000,
                max_state_bytes: 1_000_000,
                max_checkpoint_bytes: 2_000_000,
                max_checkpoints: 8,
            },
        }
    }
    struct HeldWriter {
        bytes: Vec<u8>,
        held: Arc<AtomicUsize>,
    }
    impl Write for HeldWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            assert!(self.held.load(Ordering::SeqCst) > 0);
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            assert!(self.held.load(Ordering::SeqCst) > 0);
            Ok(())
        }
    }
    struct McpHeldWriter {
        bytes: Vec<u8>,
        held: Arc<AtomicUsize>,
        source_frame: bool,
    }
    impl Write for McpHeldWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.source_frame = bytes
                .windows(b"structuredContent".len())
                .any(|w| w == b"structuredContent");
            if self.source_frame {
                assert!(self.held.load(Ordering::SeqCst) > 0);
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if self.source_frame {
                assert!(self.held.load(Ordering::SeqCst) > 0);
            }
            Ok(())
        }
    }
    struct NeverAbort;
    impl AbortProbe for NeverAbort {
        fn reason(&self) -> Option<tos_query::AbortReason> {
            None
        }
    }
    #[test]
    fn real_selected_catalog_and_inspect_packets_survive_all_native_wires() {
        let executor = Arc::new(Executor {
            fixture: build_fixture(),
            held: Arc::new(AtomicUsize::new(0)),
            checkpoints: Mutex::new(
                tos_access::exploration_checkpoints::ProcessExplorationCheckpoints::new(
                    tos_access::exploration_checkpoints::CheckpointLimits {
                        ttl: Duration::from_secs(60),
                        max_entries: 16,
                        max_encoded_bytes: 2_000_000,
                    },
                )
                .unwrap(),
            ),
        });
        let graph = parse_json(
            &executor.fixture.graph_input_bytes,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let id = |kind: &str| {
            graph.root().object_get(kind).unwrap().as_array().unwrap()[0]
                .object_get("id")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        };
        let cases = [
            (
                R::Catalog,
                vec!["knowledge".into(), "catalog".into()],
                "/api/knowledge/catalog".to_owned(),
                "tos_knowledge_catalog",
                "{}".to_owned(),
            ),
            (
                R::Node {
                    node_id: id("nodes"),
                    relation_limit: 200,
                },
                vec!["knowledge".into(), "node".into(), id("nodes")],
                format!("/api/knowledge/nodes/{}", id("nodes")),
                "tos_knowledge_node",
                format!("{{\"node_id\":\"{}\"}}", id("nodes")),
            ),
            (
                R::Relation {
                    relation_id: id("relations"),
                },
                vec!["knowledge".into(), "relation".into(), id("relations")],
                format!("/api/knowledge/relations/{}", id("relations")),
                "tos_knowledge_relation",
                format!("{{\"relation_id\":\"{}\"}}", id("relations")),
            ),
        ];
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
        // This fixture admits every packet under its declared cap, including
        // MCP's escaped text copy, structured copy and bounded request ID.
        // Production's default equal packet/frame caps remain unchanged.
        let mcp_profile = profile.with_mcp_frame_budget(
            tos_access::mcp::tool_result_frame_byte_bound(
                profile.max_response_bytes,
                profile.max_request_bytes.min(profile.max_line_bytes),
            )
            .expect("declared fixture byte caps fit usize"),
        );
        for (request, args, path, tool, arguments) in cases {
            let packet = executor.knowledge(request, Arc::new(NeverAbort)).unwrap();
            let expected = packet.body.clone();
            drop(packet);
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            let mut writer = HeldWriter {
                bytes: Vec::new(),
                held: Arc::clone(&executor.held),
            };
            let mut error = Vec::new();
            assert_eq!(
                cli::run_cli(&args, executor.as_ref(), profile, &mut writer, &mut error),
                0
            );
            assert!(error.is_empty());
            assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let owner = Arc::clone(&executor);
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                serve_connection(stream, owner, profile);
            });
            let mut client = TcpStream::connect(address).unwrap();
            client
                .write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
                .unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).unwrap();
            server.join().unwrap();
            let marker = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            assert_eq!(&response[marker..], expected);
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            let initialize=b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
            let mut input = initialize.to_vec();
            input.extend(format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"{tool}\",\"arguments\":{arguments}}}}}\n").as_bytes());
            // Initialization has no source lease; tool results retain one.
            let mut output = McpHeldWriter {
                bytes: Vec::new(),
                held: Arc::clone(&executor.held),
                source_frame: false,
            };
            run_io(
                Cursor::new(input),
                &mut output,
                executor.as_ref(),
                mcp_profile,
            )
            .unwrap();
            let last = output
                .bytes
                .split(|b| *b == b'\n')
                .filter(|frame| !frame.is_empty())
                .last()
                .unwrap();
            let result = parse_json(
                last,
                JsonMode::PublishedStrict,
                JsonLimits {
                    max_bytes: mcp_profile.max_mcp_frame_bytes,
                    ..JsonLimits::default()
                },
            )
            .unwrap();
            let rpc_result = result.root().object_get("result").unwrap_or_else(|| {
                panic!(
                    "{tool}: selected packet bytes={}, RPC frame={}",
                    expected.len(),
                    String::from_utf8_lossy(last)
                )
            });
            let text = rpc_result
                .object_get("content")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .object_get("text")
                .unwrap()
                .as_str()
                .unwrap();
            assert_eq!(text.as_bytes(), expected);
            // structuredContent is the unchanged raw packet immediately before
            // the result and JSON-RPC closing braces, not a reserialization.
            assert!(last.ends_with(b"}}"));
            assert_eq!(
                &last[last.len() - expected.len() - 2..last.len() - 2],
                expected
            );
            assert!(last.len() + 1 <= mcp_profile.max_mcp_frame_bytes);
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        }
        let raw = format!(
            "{{\"focus_node_id\":\"{}\",\"page_nodes\":1,\"page_relations\":1,\"max_depth\":2}}",
            id("nodes")
        );
        let request = parse_json(
            raw.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let first = executor
            .knowledge(R::Explore(request), Arc::new(NeverAbort))
            .unwrap();
        let first = parse_json(
            &first.body,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let cursor = first
            .object_get("page")
            .unwrap()
            .object_get("next_cursor")
            .unwrap()
            .as_str()
            .expect("bounded query pauses before complete traversal");
        let revision = first
            .object_get("snapshot_revision")
            .unwrap()
            .as_str()
            .unwrap();
        let mut store = executor.checkpoints.lock().unwrap();
        let ExplorationCheckpoint::State(state) = store.load(cursor, revision).unwrap() else {
            panic!("new successor state")
        };
        let staged = store
            .prepare(
                Some(cursor),
                revision,
                Some(&state),
                &first,
                budgets().exploration,
            )
            .unwrap();
        assert!(matches!(
            store.load(cursor, revision),
            Err(SearchV2Error {
                code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                ..
            })
        ));
        drop(staged);
        assert!(matches!(
            store.load(cursor, revision),
            Ok(ExplorationCheckpoint::State(_))
        ));
        let mut small = budgets().exploration;
        small.max_checkpoint_bytes = 1;
        assert!(matches!(
            store.prepare(Some(cursor), revision, Some(&state), &first, small),
            Err(SearchV2Error {
                code: tos_query::search_v2::SearchV2ErrorCode::BudgetExceeded,
                ..
            })
        ));
        assert!(matches!(
            store.load(cursor, revision),
            Ok(ExplorationCheckpoint::State(_))
        ));
        assert!(matches!(
            store.load(cursor, "changed-selection"),
            Err(SearchV2Error {
                code: tos_query::search_v2::SearchV2ErrorCode::StaleContinuation,
                ..
            })
        ));
        assert!(matches!(
            store.load("unknown-cursor", revision),
            Err(SearchV2Error {
                code: tos_query::search_v2::SearchV2ErrorCode::CursorExpired,
                ..
            })
        ));
        drop(store);
        let continuation = parse_json(
            format!("{{\"cursor\":\"{cursor}\"}}").as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let page = executor
            .knowledge(R::Explore(continuation.clone()), Arc::new(NeverAbort))
            .unwrap();
        let replay = executor
            .knowledge(R::Explore(continuation), Arc::new(NeverAbort))
            .unwrap();
        assert_eq!(
            page.body, replay.body,
            "repeating a consumed cursor returns its exact admitted page"
        );
    }
}
