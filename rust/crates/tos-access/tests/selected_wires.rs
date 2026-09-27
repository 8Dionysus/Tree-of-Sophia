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
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
    use tos_access::{KnowledgeOperation as O, KnowledgeRequest as R};
    use tos_compiler::knowledge_full_fixture::{
        FullKnowledgeFixture, build_fixture, build_native_fixture,
    };
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
    #[derive(Default)]
    struct Controls {
        revoked: AtomicBool,
        cancelled: AtomicBool,
        // Fixture-only withdrawal/cancellation immediately after preparation.
        after_prepare: AtomicU8,
        catalog_grants: AtomicUsize,
    }
    impl AbortProbe for Controls {
        fn reason(&self) -> Option<tos_query::AbortReason> {
            self.cancelled
                .load(Ordering::SeqCst)
                .then_some(tos_query::AbortReason::Cancelled)
        }
    }
    struct Lease(Arc<AtomicUsize>, Arc<Controls>);
    impl Drop for Lease {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    impl InspectDisclosureLease for Lease {
        fn recheck(&mut self) -> Result<(), SearchV2Error> {
            use tos_query::search_v2::SearchV2ErrorCode as Code;
            let code = if self.1.revoked.load(Ordering::SeqCst) {
                Some(Code::StalePolicy)
            } else if self.1.cancelled.load(Ordering::SeqCst) {
                Some(Code::Cancelled)
            } else {
                None
            };
            match code {
                Some(code) => Err(SearchV2Error {
                    code,
                    message: "fixture disclosure withdrawn or cancelled",
                }),
                None => Ok(()),
            }
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
        controls: Arc<Controls>,
    }
    impl Authority {
        fn new(
            bound: &BoundCmpKnowledge<'_>,
            request: &R,
            held: Arc<AtomicUsize>,
            controls: Arc<Controls>,
        ) -> Self {
            let intended = match request.operation() {
                O::Explore => tos_query::knowledge_exploration::EXPLORATION_INTENDED_USE,
                O::Temporal => tos_query::TEMPORAL_INTENDED_USE,
                O::Lens => tos_query::knowledge_lens::LENS_INTENDED_USE,
                O::Focus => tos_query::knowledge_lens::FOCUS_INTENDED_USE,
                O::StoredLens => tos_query::knowledge_lens::STORED_LENS_INTENDED_USE,
                O::SearchCapabilities => {
                    tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_INTENDED_USE
                }
                _ => "read_only_public_knowledge_inspect_v1",
            };
            Self::for_scope(bound, request.operation().id(), intended, held, controls)
        }
        fn for_scope(
            bound: &BoundCmpKnowledge<'_>,
            operation: &str,
            intended: &str,
            held: Arc<AtomicUsize>,
            controls: Arc<Controls>,
        ) -> Self {
            let policy = CurrentPolicyBinding {
                scope: "synthetic-wire".into(),
                issuer_ref: "synthetic-issuer".into(),
                authorization_receipt_id: "synthetic-receipt".into(),
                policy_epoch: "synthetic-epoch".into(),
                withdrawal_generation: "synthetic-withdrawal".into(),
            };
            let selected = bound.selection();
            let inspect = IndexedDisclosureScope {
                operation_id: operation.into(),
                carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                intended_use: intended.into(),
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
                controls,
            }
        }
        fn lease(&self) -> Lease {
            self.held.fetch_add(1, Ordering::SeqCst);
            Lease(Arc::clone(&self.held), Arc::clone(&self.controls))
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
        fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
            Some(self.controls.clone())
        }
        fn authorize_catalog_current(&mut self, sha: Digest256) -> Result<(), SearchV2Error> {
            assert_eq!(self.inspect.operation_id, O::StoredLens.id());
            assert_eq!(sha, self.catalog.catalog_packet_sha256);
            self.controls.catalog_grants.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
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
            if self.inspect.operation_id
                == tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_OPERATION
            {
                assert!(
                    observed.is_empty(),
                    "capabilities authenticates selected engine binding without disclosing graph rows"
                );
            } else {
                assert!(!observed.is_empty());
            }
            Ok(Box::new(self.lease()))
        }
    }
    struct Executor {
        fixture: FullKnowledgeFixture,
        held: Arc<AtomicUsize>,
        checkpoints: Mutex<tos_access::exploration_checkpoints::ProcessExplorationCheckpoints>,
        controls: Arc<Controls>,
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
        fn knowledge_search_legacy_available(&self) -> bool {
            true
        }
        fn knowledge_search_legacy(
            &self,
            request: tos_query::knowledge_legacy_search::LegacySearchRequest,
            probe: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket, AccessError> {
            use tos_query::knowledge_legacy_search::{
                LEGACY_SEARCH_INTENDED_USE, LEGACY_SEARCH_OPERATION,
            };
            let cold = self.fixture.open().unwrap();
            let bound = bind_verified_knowledge(
                &cold,
                &self.fixture.vocabulary,
                &self.fixture.descriptor_bytes,
            )
            .unwrap();
            let mut model = cold.fork_reader_with_vm_budget(20_000_000).unwrap();
            let mut authority = Authority::for_scope(
                &bound,
                LEGACY_SEARCH_OPERATION,
                LEGACY_SEARCH_INTENDED_USE,
                Arc::clone(&self.held),
                Arc::clone(&self.controls),
            );
            let packet = tos_access::knowledge::execute_selected_legacy_search(
                &mut model,
                &bound,
                &mut authority,
                &request,
                legacy_budget(),
                probe,
            )?;
            match self.controls.after_prepare.load(Ordering::SeqCst) {
                1 => self.controls.revoked.store(true, Ordering::SeqCst),
                2 => self.controls.cancelled.store(true, Ordering::SeqCst),
                _ => {}
            }
            Ok(packet)
        }
        fn knowledge_available(&self, operation: O) -> bool {
            matches!(
                operation,
                O::Catalog
                    | O::Node
                    | O::Relation
                    | O::Explore
                    | O::Temporal
                    | O::Lens
                    | O::Focus
                    | O::StoredLens
                    | O::SearchCapabilities
            )
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
            let mut catalog = Authority::new(
                &bound,
                &request,
                Arc::clone(&self.held),
                Arc::clone(&self.controls),
            );
            let mut inspect = Authority::new(
                &bound,
                &request,
                Arc::clone(&self.held),
                Arc::clone(&self.controls),
            );
            let budgets = budgets();
            let packet = tos_access::knowledge::execute_selected_knowledge(
                &mut model,
                &bound,
                &mut catalog,
                &mut inspect,
                &mut *self.checkpoints.lock().unwrap(),
                request,
                budgets,
                probe,
            )?;
            match self.controls.after_prepare.load(Ordering::SeqCst) {
                1 => self.controls.revoked.store(true, Ordering::SeqCst),
                2 => self.controls.cancelled.store(true, Ordering::SeqCst),
                _ => {}
            }
            Ok(packet)
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
    fn legacy_budget() -> tos_query::knowledge_legacy_search::LegacySearchBudget {
        // Same full-scan work allowances as the QRY actual legacy differential
        // fixture; the native packet cap remains independently one megabyte.
        let mut inspect = budgets().inspect;
        inspect.max_read_vm_steps = 20_000_000;
        inspect.max_matches = 1000;
        inspect.max_rows = 100_000;
        inspect.max_decoded_bytes = 128_000_000;
        tos_query::knowledge_legacy_search::LegacySearchBudget {
            inspect,
            document: tos_query::SearchDocumentBudget {
                max_carrier_bytes: 1_000_000,
                max_document_bytes: 4_000_000,
                max_document_code_points: 4_000_000,
                json: JsonLimits::default(),
            },
            max_candidates: 100_000,
            max_document_bytes: 128_000_000,
            max_document_code_points: 128_000_000,
            max_retained_per_kind: 100_100,
            max_retained_bytes: 8_000_000,
            block_size: 64,
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
            controls: Arc::new(Controls::default()),
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
        // A fixture node may be isolated or finish within one page. Select a
        // genuine paused result from the maintained graph instead of treating
        // array order as a traversal guarantee. Each attempt uses the actual
        // selected producer/query path and leaves no invented continuation.
        let first = graph
            .root()
            .object_get("nodes")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find_map(|node| {
                let node_id = node.object_get("id").unwrap().as_str().unwrap();
                let raw = format!(
                    "{{\"focus_node_id\":\"{node_id}\",\"page_nodes\":1,\"page_relations\":1,\"max_depth\":2}}"
                );
                let request = parse_json(
                    raw.as_bytes(),
                    JsonMode::PublishedStrict,
                    JsonLimits::default(),
                )
                .unwrap()
                .into_root();
                let packet = executor
                    .knowledge(R::Explore(request), Arc::new(NeverAbort))
                    .unwrap();
                let page = parse_json(
                    &packet.body,
                    JsonMode::PublishedStrict,
                    JsonLimits::default(),
                )
                .unwrap()
                .into_root();
                page.object_get("page")
                    .unwrap()
                    .object_get("next_cursor")
                    .unwrap()
                    .as_str()
                    .is_some()
                    .then_some(page)
            })
            .expect("genuine selected fixture has a resumable neighborhood");
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

    fn json_bytes(value: &JsonValue) -> Vec<u8> {
        tos_foundation::emit_value_preserved_json(value, JsonLimits::default()).unwrap()
    }
    fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (tos_foundation::JsonString::from_utf8(key), value))
                .collect(),
        )
    }
    fn text(value: &str) -> JsonValue {
        JsonValue::String(tos_foundation::JsonString::from_utf8(value))
    }
    fn path_id(value: &str) -> String {
        value
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                    (byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect()
    }
    fn http_packet(response: &[u8]) -> &[u8] {
        let marker = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let header = std::str::from_utf8(&response[..marker]).unwrap();
        assert!(header.starts_with("HTTP/1.1 200 "), "{header}");
        let length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert_eq!(response.len() - marker, length);
        &response[marker..]
    }
    fn mcp_input(tool: &str, arguments: &JsonValue) -> Vec<u8> {
        let mut input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n".to_vec();
        input.extend(json_bytes(&object(vec![
            ("jsonrpc", text("2.0")),
            (
                "id",
                JsonValue::Number(tos_foundation::JsonNumber {
                    kind: tos_foundation::JsonNumberKind::Int,
                    lexeme: "2".into(),
                }),
            ),
            ("method", text("tools/call")),
            (
                "params",
                object(vec![("name", text(tool)), ("arguments", arguments.clone())]),
            ),
        ])));
        input.push(b'\n');
        input
    }
    fn last_frame(output: &[u8]) -> &[u8] {
        output
            .split(|b| *b == b'\n')
            .filter(|frame| !frame.is_empty())
            .last()
            .unwrap()
    }
    fn check_mcp_packet(frame: &[u8], expected: &[u8], cap: usize) {
        assert!(frame.len() + 1 <= cap);
        let document = parse_json(
            frame,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: cap,
                ..JsonLimits::default()
            },
        )
        .unwrap();
        let result = document
            .root()
            .object_get("result")
            .unwrap_or_else(|| panic!("selected MCP refusal: {}", String::from_utf8_lossy(frame)));
        assert_eq!(
            result.object_get("content").unwrap().as_array().unwrap()[0]
                .object_get("text")
                .unwrap()
                .as_str()
                .unwrap()
                .as_bytes(),
            expected
        );
        assert!(frame.ends_with(b"}}"));
        assert_eq!(
            &frame[frame.len() - expected.len() - 2..frame.len() - 2],
            expected
        );
    }

    #[test]
    fn real_selected_query_families_survive_native_wires_and_disclosure_changes() {
        let executor = Arc::new(Executor {
            fixture: build_native_fixture(),
            held: Arc::new(AtomicUsize::new(0)),
            controls: Arc::new(Controls::default()),
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
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
        let mcp_profile = profile.with_mcp_frame_budget(
            tos_access::mcp::tool_result_frame_byte_bound(
                profile.max_response_bytes,
                profile.max_request_bytes.min(profile.max_line_bytes),
            )
            .unwrap(),
        );
        let graph = parse_json(
            &executor.fixture.graph_input_bytes,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let nodes = graph
            .root()
            .object_get("nodes")
            .unwrap()
            .as_array()
            .unwrap();
        let claim = nodes
            .iter()
            .find(|node| {
                node.object_get("kind_id").and_then(JsonValue::as_str) == Some("claim")
                    && node.object_get("type_id").and_then(JsonValue::as_str)
                        == Some("tos.entity.claim")
            })
            .expect("genuine maintained native Claim operand");
        let claim_id = claim.object_get("id").unwrap().as_str().unwrap();
        let operand = object(vec![
            ("node_id", text(claim_id)),
            (
                "content_revision",
                claim.object_get("content_revision").unwrap().clone(),
            ),
        ]);
        let temporal = object(vec![
            ("schema_version", text("tos_temporal_comparison_request_v1")),
            (
                "source_revision",
                graph.root().object_get("source_revision").unwrap().clone(),
            ),
            ("left", operand.clone()),
            ("right", operand),
        ]);
        // Retrieve the authored stored spec through the actual selected catalog,
        // instead of embedding a lens ID or maintained source list in consumers.
        let catalog = executor
            .knowledge(R::Catalog, Arc::new(NeverAbort))
            .unwrap();
        let catalog_value = parse_json(
            &catalog.body,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let lens = catalog_value
            .root()
            .object_get("lenses")
            .unwrap()
            .as_array()
            .unwrap()
            .first()
            .expect("real selected stored LensSpec")
            .clone();
        let lens_id = lens
            .object_get("lens_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        drop(catalog);
        // Initial continuations are nondeterministic process-owned cursors.
        // Obtain one actual paused traversal, then compare its exact admitted
        // replay packet across POST and MCP using the same consumed input.
        let first = nodes
            .iter()
            .find_map(|node| {
                let request = object(vec![
                    ("focus_node_id", node.object_get("id").unwrap().clone()),
                    (
                        "page_nodes",
                        parse_json(b"1", JsonMode::PublishedStrict, JsonLimits::default())
                            .unwrap()
                            .into_root(),
                    ),
                    (
                        "page_relations",
                        parse_json(b"1", JsonMode::PublishedStrict, JsonLimits::default())
                            .unwrap()
                            .into_root(),
                    ),
                    (
                        "max_depth",
                        parse_json(b"2", JsonMode::PublishedStrict, JsonLimits::default())
                            .unwrap()
                            .into_root(),
                    ),
                ]);
                let packet = executor
                    .knowledge(R::Explore(request), Arc::new(NeverAbort))
                    .unwrap();
                let value = parse_json(
                    &packet.body,
                    JsonMode::PublishedStrict,
                    JsonLimits::default(),
                )
                .unwrap()
                .into_root();
                value
                    .object_get("page")
                    .unwrap()
                    .object_get("next_cursor")
                    .unwrap()
                    .as_str()
                    .is_some()
                    .then_some(value)
            })
            .expect("genuine resumable native neighborhood");
        let cursor = first
            .object_get("page")
            .unwrap()
            .object_get("next_cursor")
            .unwrap()
            .as_str()
            .unwrap();
        let explore = object(vec![("cursor", text(cursor))]);
        let focus = tos_query::knowledge_focus::KnowledgeFocusRequest::new(claim_id.to_owned());
        // Optional suffix, request body, MCP args, selected request.
        let cases = vec![
            (
                Some("-".to_owned()),
                Some(json_bytes(&temporal)),
                object(vec![("request", temporal.clone())]),
                R::Temporal(temporal),
            ),
            (
                Some("-".to_owned()),
                Some(json_bytes(&lens)),
                object(vec![("spec", lens.clone())]),
                R::Lens(lens),
            ),
            (
                Some(claim_id.to_owned()),
                None,
                object(vec![("node_id", text(claim_id))]),
                R::Focus(focus),
            ),
            (
                Some(lens_id.clone()),
                None,
                object(vec![("lens_id", text(&lens_id))]),
                R::StoredLens { lens_id },
            ),
            (
                None,
                Some(json_bytes(&explore)),
                object(vec![("request", explore.clone())]),
                R::Explore(explore),
            ),
        ];
        for (suffix, body, arguments, request) in &cases {
            let operation = tos_access::registered_operations()
                .unwrap()
                .iter()
                .find(|op| op.operation_id == request.operation().id())
                .unwrap();
            let path = operation
                .http_path
                .replace("{node_id}", &path_id(claim_id))
                .replace("{lens_id}", &path_id(suffix.as_deref().unwrap_or_default()));
            let packet = executor
                .knowledge(request.clone(), Arc::new(NeverAbort))
                .unwrap();
            let expected = packet.body.clone();
            drop(packet);
            if let Some(suffix) = suffix {
                let mut args: Vec<String> = operation
                    .cli_command
                    .as_ref()
                    .unwrap()
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                args.push(suffix.clone());
                let mut writer = HeldWriter {
                    bytes: Vec::new(),
                    held: Arc::clone(&executor.held),
                };
                let mut errors = Vec::new();
                assert_eq!(
                    cli::run_cli_with_input(
                        &args,
                        executor.as_ref(),
                        profile,
                        &mut Cursor::new(body.clone().unwrap_or_default()),
                        &mut writer,
                        &mut errors
                    ),
                    0,
                    "{}: {}",
                    operation.operation_id,
                    String::from_utf8_lossy(&errors)
                );
                assert!(errors.is_empty());
                assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
            }
            let response = if operation.http_method == "POST" {
                tos_access::http::handle_post(
                    executor.as_ref(),
                    &path,
                    body.as_ref().unwrap(),
                    profile,
                )
            } else {
                handle_get(executor.as_ref(), "GET", &path, profile)
            };
            assert_eq!(
                response.status,
                200,
                "{}: HTTP {} {} response: {}",
                operation.operation_id,
                operation.http_method,
                path,
                String::from_utf8_lossy(&response.body)
            );
            assert!(
                executor.held.load(Ordering::SeqCst) > 0,
                "{}: successful prepared HTTP packet must retain disclosure hold",
                operation.operation_id
            );
            let mut writer = HeldWriter {
                bytes: Vec::new(),
                held: Arc::clone(&executor.held),
            };
            tos_access::http::write_response(&mut writer, response).unwrap();
            assert_eq!(http_packet(&writer.bytes), expected);
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let owner = Arc::clone(&executor);
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                serve_connection(stream, owner, profile);
            });
            let mut client = TcpStream::connect(address).unwrap();
            let wire_body = if operation.http_method == "POST" {
                body.clone().unwrap()
            } else {
                Vec::new()
            };
            let header = format!(
                "{} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                operation.http_method,
                wire_body.len()
            );
            client.write_all(header.as_bytes()).unwrap();
            client.write_all(&wire_body).unwrap();
            let mut bytes = Vec::new();
            client.read_to_end(&mut bytes).unwrap();
            server.join().unwrap();
            assert_eq!(http_packet(&bytes), expected);
            let mut writer = McpHeldWriter {
                bytes: Vec::new(),
                held: Arc::clone(&executor.held),
                source_frame: false,
            };
            run_io(
                Cursor::new(mcp_input(&operation.mcp_tool, arguments)),
                &mut writer,
                executor.as_ref(),
                mcp_profile,
            )
            .unwrap();
            check_mcp_packet(
                last_frame(&writer.bytes),
                &expected,
                mcp_profile.max_mcp_frame_bytes,
            );
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        }
        assert!(
            executor.controls.catalog_grants.load(Ordering::SeqCst) >= 4,
            "stored-lens direct/CLI/HTTP/MCP consultations authorize exact selected catalog"
        );

        // The real prepared temporal packet becomes undisclosable when the
        // fixture's current binding is withdrawn or owner probe is cancelled.
        // Every transport must refuse before emitting packet bytes and drop hold.
        let (suffix, body, arguments, request) = &cases[0];
        let operation = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|op| op.operation_id == request.operation().id())
            .unwrap();
        let reset = || {
            executor.controls.revoked.store(false, Ordering::SeqCst);
            executor.controls.cancelled.store(false, Ordering::SeqCst);
        };
        for change in [1, 2] {
            executor
                .controls
                .after_prepare
                .store(change, Ordering::SeqCst);
            let mut args: Vec<String> = operation
                .cli_command
                .as_ref()
                .unwrap()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            args.push(suffix.as_ref().unwrap().clone());
            let mut output = Vec::new();
            let mut errors = Vec::new();
            assert_eq!(
                cli::run_cli_with_input(
                    &args,
                    executor.as_ref(),
                    profile,
                    &mut Cursor::new(body.clone().unwrap()),
                    &mut output,
                    &mut errors
                ),
                1
            );
            assert!(output.is_empty());
            assert!(!errors.is_empty());
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            reset();
            let response = tos_access::http::handle_post(
                executor.as_ref(),
                &operation.http_path,
                body.as_ref().unwrap(),
                profile,
            );
            let mut output = Vec::new();
            tos_access::http::write_response(&mut output, response).unwrap();
            assert!(
                !String::from_utf8_lossy(&output).contains("tos_temporal_comparison_result_v1")
            );
            assert!(!String::from_utf8_lossy(&output).starts_with("HTTP/1.1 200 "));
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            reset();
            let mut output = Vec::new();
            run_io(
                Cursor::new(mcp_input(&operation.mcp_tool, arguments)),
                &mut output,
                executor.as_ref(),
                mcp_profile,
            )
            .unwrap();
            let frame = last_frame(&output);
            assert!(String::from_utf8_lossy(frame).contains("\"isError\":true"));
            assert!(!String::from_utf8_lossy(frame).contains("structuredContent"));
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            reset();
        }
        executor.controls.after_prepare.store(0, Ordering::SeqCst);
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let mut args: Vec<String> = operation
            .cli_command
            .as_ref()
            .unwrap()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        args.push(suffix.as_ref().unwrap().clone());
        assert_eq!(
            cli::run_cli_with_input(
                &args,
                executor.as_ref(),
                profile.with_query_timeout(Duration::ZERO),
                &mut Cursor::new(body.clone().unwrap()),
                &mut output,
                &mut errors
            ),
            1
        );
        assert!(output.is_empty());
        assert!(String::from_utf8_lossy(&errors).contains("deadline_exceeded"));
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        let deadline = profile.with_query_timeout(Duration::ZERO);
        let response = tos_access::http::handle_post(
            executor.as_ref(),
            &operation.http_path,
            body.as_ref().unwrap(),
            deadline,
        );
        assert_eq!(response.status, 408);
        let mut output = Vec::new();
        tos_access::http::write_response(&mut output, response).unwrap();
        assert!(String::from_utf8_lossy(&output).contains("deadline_exceeded"));
        let mut output = Vec::new();
        run_io(
            Cursor::new(mcp_input(&operation.mcp_tool, arguments)),
            &mut output,
            executor.as_ref(),
            mcp_profile.with_query_timeout(Duration::ZERO),
        )
        .unwrap();
        let frame = String::from_utf8_lossy(last_frame(&output));
        assert!(frame.contains("\"isError\":true"));
        assert!(!frame.contains("structuredContent"));
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn maintained_selected_legacy_search_and_capabilities_all_native_wires() {
        use tos_query::knowledge_legacy_search::LegacySearchRequest;
        let executor = Arc::new(Executor {
            fixture: build_native_fixture(),
            held: Arc::new(AtomicUsize::new(0)),
            controls: Arc::new(Controls::default()),
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
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
        let mcp_profile = profile.with_mcp_frame_budget(
            tos_access::mcp::tool_result_frame_byte_bound(
                profile.max_response_bytes,
                profile.max_request_bytes.min(profile.max_line_bytes),
            )
            .unwrap(),
        );
        let search = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|op| op.operation_id == tos_access::SEARCH_OPERATION_ID)
            .unwrap();
        let graph = parse_json(
            &executor.fixture.graph_input_bytes,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let claim = graph
            .root()
            .object_get("nodes")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n.object_get("kind_id").and_then(JsonValue::as_str) == Some("claim"))
            .unwrap();
        let source = claim.object_get("source_graph").unwrap().as_str().unwrap();
        let kind = claim.object_get("kind_id").unwrap().as_str().unwrap();
        let filtered = LegacySearchRequest {
            sources: Some(vec![source.into()]),
            kind_ids: vec![kind.into()],
            offset: 1,
            limit: 2,
            ..Default::default()
        };
        let cases = [
            (
                LegacySearchRequest::default(),
                vec![],
                String::new(),
                object(vec![
                    ("sources", JsonValue::Null),
                    ("kind_ids", JsonValue::Null),
                    ("predicate_ids", JsonValue::Null),
                    ("cursor", JsonValue::Null),
                ]),
            ),
            (
                filtered,
                vec![
                    "--sources".into(),
                    source.into(),
                    "--kind".into(),
                    kind.into(),
                    "--offset=1".into(),
                    "--limit=2".into(),
                    "--mode=legacy".into(),
                ],
                format!(
                    "?sources={}&kind_ids={}&offset=1&limit=2&mode=legacy",
                    path_id(source),
                    path_id(kind)
                ),
                object(vec![
                    ("sources", JsonValue::Array(vec![text(source)])),
                    ("kind_ids", JsonValue::Array(vec![text(kind)])),
                    (
                        "offset",
                        parse_json(b"1", JsonMode::PublishedStrict, JsonLimits::default())
                            .unwrap()
                            .into_root(),
                    ),
                    (
                        "limit",
                        parse_json(b"2", JsonMode::PublishedStrict, JsonLimits::default())
                            .unwrap()
                            .into_root(),
                    ),
                    ("mode", text("legacy")),
                ]),
            ),
        ];
        for (request, suffix, query, arguments) in cases {
            let packet = executor
                .knowledge_search_legacy(request, Arc::new(NeverAbort))
                .unwrap();
            let expected = packet.body.clone();
            drop(packet);
            let mut args: Vec<String> = search
                .cli_command
                .as_ref()
                .unwrap()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            args.extend(suffix);
            let mut writer = HeldWriter {
                bytes: vec![],
                held: executor.held.clone(),
            };
            let mut errors = vec![];
            let code = cli::run_cli(&args, executor.as_ref(), profile, &mut writer, &mut errors);
            assert_eq!(code, 0, "legacy CLI: {}", String::from_utf8_lossy(&errors));
            assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
            let response = handle_get(
                executor.as_ref(),
                "GET",
                &format!("{}{query}", search.http_path),
                profile,
            );
            assert_eq!(
                response.status,
                200,
                "legacy HTTP: {}",
                String::from_utf8_lossy(&response.body)
            );
            let mut writer = HeldWriter {
                bytes: vec![],
                held: executor.held.clone(),
            };
            tos_access::http::write_response(&mut writer, response).unwrap();
            assert_eq!(http_packet(&writer.bytes), expected);
            let input = mcp_input(&search.mcp_tool, &arguments);
            let mut writer = McpHeldWriter {
                bytes: vec![],
                held: executor.held.clone(),
                source_frame: false,
            };
            run_io(
                Cursor::new(input),
                &mut writer,
                executor.as_ref(),
                mcp_profile,
            )
            .unwrap();
            check_mcp_packet(
                last_frame(&writer.bytes),
                &expected,
                mcp_profile.max_mcp_frame_bytes,
            );
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        }
        let caps = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|op| op.operation_id == O::SearchCapabilities.id())
            .unwrap();
        let packet = executor
            .knowledge(R::SearchCapabilities, Arc::new(NeverAbort))
            .unwrap();
        let expected = packet.body.clone();
        drop(packet);
        let value =
            parse_json(&expected, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        assert_eq!(
            value
                .root()
                .object_get("default_mode")
                .and_then(JsonValue::as_str),
            Some("legacy")
        );
        let modes = value.root().object_get("modes").unwrap();
        assert_eq!(
            modes
                .object_get("compressed")
                .unwrap()
                .object_get("available"),
            Some(&JsonValue::Bool(false))
        );
        let args: Vec<String> = caps
            .cli_command
            .as_ref()
            .unwrap()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        let mut errors = vec![];
        assert_eq!(
            cli::run_cli(&args, executor.as_ref(), profile, &mut writer, &mut errors),
            0,
            "{}",
            String::from_utf8_lossy(&errors)
        );
        assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
        let response = handle_get(executor.as_ref(), "GET", &caps.http_path, profile);
        assert_eq!(response.status, 200);
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        tos_access::http::write_response(&mut writer, response).unwrap();
        assert_eq!(http_packet(&writer.bytes), expected);
        let input = mcp_input(&caps.mcp_tool, &object(vec![]));
        let mut writer = McpHeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
            source_frame: false,
        };
        run_io(
            Cursor::new(input),
            &mut writer,
            executor.as_ref(),
            mcp_profile,
        )
        .unwrap();
        check_mcp_packet(
            last_frame(&writer.bytes),
            &expected,
            mcp_profile.max_mcp_frame_bytes,
        );
        // Engine selection never supplies a prepared publication or a current public issuer.
        let mut output = vec![];
        let mut errors = vec![];
        let mut args: Vec<String> = search
            .cli_command
            .as_ref()
            .unwrap()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        args.push("--mode=compressed".into());
        assert_eq!(
            cli::run_cli(&args, executor.as_ref(), profile, &mut output, &mut errors),
            3
        );
        assert!(output.is_empty());
        let response = handle_get(
            executor.as_ref(),
            "GET",
            &format!("{}?mode=compressed", search.http_path),
            profile,
        );
        assert_eq!(response.status, 503);
        // Current withdrawal and caller cancellation share the established final disclosure fence.
        for action in [1, 2] {
            executor
                .controls
                .after_prepare
                .store(action, Ordering::SeqCst);
            let mut output = vec![];
            let mut errors = vec![];
            let args: Vec<String> = search
                .cli_command
                .as_ref()
                .unwrap()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            assert_eq!(
                cli::run_cli(&args, executor.as_ref(), profile, &mut output, &mut errors),
                1
            );
            assert!(output.is_empty());
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            executor.controls.revoked.store(false, Ordering::SeqCst);
            executor.controls.cancelled.store(false, Ordering::SeqCst);
        }
    }

    #[test]
    fn maintained_native_options_files_head_and_mcp_advertisement() {
        assert_eq!(cli::parse_serve_address(&[]).unwrap(), "127.0.0.1:8080");
        assert_eq!(
            cli::parse_serve_address(&["--host=::1".into(), "--port".into(), "8081".into()])
                .unwrap(),
            "[::1]:8081"
        );
        assert!(cli::parse_serve_address(&["--port=65536".into()]).is_err());
        let executor = Arc::new(Executor {
            fixture: build_native_fixture(),
            controls: Arc::new(Controls::default()),
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
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
        let mcp_profile = profile.with_mcp_frame_budget(
            tos_access::mcp::tool_result_frame_byte_bound(
                profile.max_response_bytes,
                profile.max_request_bytes.min(profile.max_line_bytes),
            )
            .unwrap(),
        );
        let graph = parse_json(
            &executor.fixture.graph_input_bytes,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let node = graph
            .root()
            .object_get("nodes")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n.object_get("kind_id").and_then(JsonValue::as_str) == Some("claim"))
            .unwrap();
        let id = node.object_get("id").unwrap().as_str().unwrap();
        let source = node.object_get("source_graph").unwrap().as_str().unwrap();
        let operation = |kind: O| {
            tos_access::registered_operations()
                .unwrap()
                .iter()
                .find(|op| op.operation_id == kind.id())
                .unwrap()
        };
        let args = |kind: O, suffix: Vec<String>| {
            let mut args: Vec<String> = operation(kind)
                .cli_command
                .as_ref()
                .unwrap()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            args.extend(suffix);
            args
        };
        let mut focus = tos_query::knowledge_focus::KnowledgeFocusRequest::new(id);
        focus.sources = Some(vec![source.into()]);
        focus.depth = 0;
        focus.direction = tos_query::knowledge_focus::FocusDirection::Incoming;
        focus.profile = tos_query::knowledge_focus::FocusProfile::All;
        focus.node_limit = 2;
        focus.relation_limit = 0;
        let selected = executor
            .knowledge(R::Focus(focus), Arc::new(NeverAbort))
            .unwrap();
        let expected = selected.body.clone();
        drop(selected);
        let focus_args = args(
            O::Focus,
            vec![
                id.into(),
                "--sources".into(),
                executor
                    .fixture
                    .vocabulary
                    .sources
                    .iter()
                    .find(|other| other.source_graph_id != source)
                    .unwrap()
                    .source_graph_id
                    .clone(),
                "--sources".into(),
                source.into(),
                "--depth=0".into(),
                "--direction=incoming".into(),
                "--node-limit=2".into(),
                "--relation-limit=0".into(),
                "--profile=all".into(),
            ],
        );
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        let mut errors = vec![];
        assert_eq!(
            cli::run_cli(
                &focus_args,
                executor.as_ref(),
                profile,
                &mut writer,
                &mut errors
            ),
            0,
            "{}",
            String::from_utf8_lossy(&errors)
        );
        assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
        let node_args = args(
            O::Node,
            vec![
                id.into(),
                "--relation-limit=1".into(),
                "--relation-limit=0".into(),
            ],
        );
        let packet = executor
            .knowledge(
                R::Node {
                    node_id: id.into(),
                    relation_limit: 0,
                },
                Arc::new(NeverAbort),
            )
            .unwrap();
        let expected = packet.body.clone();
        drop(packet);
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        let mut errors = vec![];
        assert_eq!(
            cli::run_cli(
                &node_args,
                executor.as_ref(),
                profile,
                &mut writer,
                &mut errors
            ),
            0
        );
        assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
        let path = operation(O::Node)
            .http_path
            .replace("{node_id}", &path_id(id));
        let target = format!("{path}?relation_limit=bad&relation_limit=0");
        let packet = executor
            .knowledge(
                R::Node {
                    node_id: id.into(),
                    relation_limit: 200,
                },
                Arc::new(NeverAbort),
            )
            .unwrap();
        let expected = packet.body.clone();
        drop(packet);
        let response = handle_get(executor.as_ref(), "GET", &target, profile);
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        tos_access::http::write_response(&mut writer, response).unwrap();
        assert_eq!(
            http_packet(&writer.bytes),
            expected,
            "first repeated malformed integer uses default"
        );
        let response = handle_get(executor.as_ref(), "HEAD", &target, profile);
        assert_eq!(response.status, 200);
        assert_eq!(response.body, expected);
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        tos_access::http::write_response(&mut writer, response).unwrap();
        assert!(
            writer.bytes.ends_with(b"\r\n\r\n"),
            "HEAD emits headers only"
        );
        assert!(
            String::from_utf8_lossy(&writer.bytes)
                .contains(&format!("Content-Length: {}\r\n", expected.len()))
        );
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        let unknown = format!(
            "/{}",
            Digest256::of_bytes(&executor.fixture.descriptor_bytes).to_hex()
        );
        let response = handle_get(executor.as_ref(), "HEAD", &unknown, profile);
        assert_eq!(response.status, 404);
        let mut output = vec![];
        tos_access::http::write_response(&mut output, response).unwrap();
        assert!(output.ends_with(b"\r\n\r\n"));
        assert_eq!(
            handle_get(executor.as_ref(), "POST", &path, profile).status,
            405
        );
        // File paths are explicit and bounded; the fixture's existing owned
        // directory receives request companions and owns their Drop cleanup.
        let operand = object(vec![
            ("node_id", text(id)),
            (
                "content_revision",
                node.object_get("content_revision").unwrap().clone(),
            ),
        ]);
        let temporal = object(vec![
            ("schema_version", text("tos_temporal_comparison_request_v1")),
            (
                "source_revision",
                graph.root().object_get("source_revision").unwrap().clone(),
            ),
            ("left", operand.clone()),
            ("right", operand),
        ]);
        let catalog = executor
            .knowledge(R::Catalog, Arc::new(NeverAbort))
            .unwrap();
        let value = parse_json(
            &catalog.body,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let lens = value
            .root()
            .object_get("lenses")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .clone();
        drop(catalog);
        for (request, body) in [
            (R::Temporal(temporal.clone()), temporal),
            (R::Lens(lens.clone()), lens),
        ] {
            let op = request.operation();
            let file = executor
                .fixture
                .path
                .with_extension(format!("{}.request.json", op.id()));
            fs::write(&file, json_bytes(&body)).unwrap();
            let packet = executor.knowledge(request, Arc::new(NeverAbort)).unwrap();
            let expected = packet.body.clone();
            drop(packet);
            let command = args(op, vec![file.to_str().unwrap().into()]);
            let mut writer = HeldWriter {
                bytes: vec![],
                held: executor.held.clone(),
            };
            let mut errors = vec![];
            assert_eq!(
                cli::run_cli(
                    &command,
                    executor.as_ref(),
                    profile,
                    &mut writer,
                    &mut errors
                ),
                0
            );
            assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
            fs::write(&file, b"[]").unwrap();
            let mut output = vec![];
            let mut errors = vec![];
            assert_eq!(
                cli::run_cli(
                    &command,
                    executor.as_ref(),
                    profile,
                    &mut output,
                    &mut errors
                ),
                2
            );
            assert!(output.is_empty());
            assert!(String::from_utf8_lossy(&errors).contains("invalid_request"));
            fs::write(&file, vec![b' '; profile.max_request_bytes + 1]).unwrap();
            let mut output = vec![];
            let mut errors = vec![];
            assert_eq!(
                cli::run_cli(
                    &command,
                    executor.as_ref(),
                    profile,
                    &mut output,
                    &mut errors
                ),
                2
            );
            assert!(output.is_empty());
            assert!(
                String::from_utf8_lossy(&errors).contains("budget_exceeded"),
                "file byte refusal: {}",
                String::from_utf8_lossy(&errors)
            );
            fs::remove_file(&file).unwrap();
            let mut output = vec![];
            let mut errors = vec![];
            assert_eq!(
                cli::run_cli(
                    &command,
                    executor.as_ref(),
                    profile,
                    &mut output,
                    &mut errors
                ),
                2
            );
            assert!(output.is_empty());
            assert!(!errors.is_empty());
        }
        let mut output = vec![];
        let mut errors = vec![];
        assert_eq!(
            cli::run_cli(
                &args(O::Focus, vec![id.into(), "--direction=invalid".into()]),
                executor.as_ref(),
                profile,
                &mut output,
                &mut errors
            ),
            2
        );
        assert!(output.is_empty());
        let mut input = mcp_input("", &object(vec![]));
        // Keep the actual standard handshake, replacing only tools/call with
        // tools/list; descriptor supplies the expected advertised ready set.
        let end = input
            .iter()
            .enumerate()
            .filter(|(_, b)| **b == b'\n')
            .nth(1)
            .unwrap()
            .0
            + 1;
        input.truncate(end);
        input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n");
        let mut output = vec![];
        run_io(
            Cursor::new(input),
            &mut output,
            executor.as_ref(),
            mcp_profile,
        )
        .unwrap();
        let value = parse_json(
            last_frame(&output),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let tools = value
            .root()
            .object_get("result")
            .unwrap()
            .object_get("tools")
            .unwrap()
            .as_array()
            .unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for tool in tools {
            let name = tool.object_get("name").unwrap().as_str().unwrap();
            assert!(seen.insert(name));
            let descriptor = tos_access::registered_operations()
                .unwrap()
                .iter()
                .find(|op| op.mcp_tool == name)
                .unwrap();
            assert!(
                if descriptor.operation_id == tos_access::SEARCH_OPERATION_ID {
                    executor.knowledge_search_legacy_available()
                        || executor.knowledge_search_indexed_available()
                } else {
                    O::from_id(&descriptor.operation_id)
                        .is_some_and(|op| executor.knowledge_available(op))
                }
            );
            assert_eq!(
                tool.object_get("inputSchema").unwrap(),
                &descriptor.input_schema
            );
        }
        let expected: std::collections::BTreeSet<_> = tos_access::registered_operations()
            .unwrap()
            .iter()
            .filter(|op| {
                if op.operation_id == tos_access::SEARCH_OPERATION_ID {
                    executor.knowledge_search_legacy_available()
                        || executor.knowledge_search_indexed_available()
                } else {
                    O::from_id(&op.operation_id).is_some_and(|op| executor.knowledge_available(op))
                }
            })
            .map(|op| op.mcp_tool.as_str())
            .collect();
        assert_eq!(seen, expected);
        let mut output = vec![];
        run_io(
            Cursor::new(mcp_input(
                &operation(O::Contracts).mcp_tool,
                &object(vec![]),
            )),
            &mut output,
            executor.as_ref(),
            mcp_profile,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(last_frame(&output)).contains("error"));
        assert!(!String::from_utf8_lossy(last_frame(&output)).contains("structuredContent"));
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
    }
}
