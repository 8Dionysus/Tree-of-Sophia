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
        registries: [(String, Digest256); 2],
        registry_grants: u8,
        philosophy_granted: bool,
        corpus_granted: bool,
    }
    impl Authority {
        fn new(
            bound: &BoundCmpKnowledge<'_>,
            request: &R,
            held: Arc<AtomicUsize>,
            controls: Arc<Controls>,
        ) -> Self {
            let intended = match request.operation() {
                op if op.is_corpus() => tos_query::corpus_read::CORPUS_INTENDED_USE,
                op if op.is_philosophy() => tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE,
                O::Dossier => tos_query::source_dossier::DOSSIER_INTENDED_USE,
                O::Explore => tos_query::knowledge_exploration::EXPLORATION_INTENDED_USE,
                O::Temporal => tos_query::TEMPORAL_INTENDED_USE,
                O::Lens => tos_query::knowledge_lens::LENS_INTENDED_USE,
                O::Focus => tos_query::knowledge_lens::FOCUS_INTENDED_USE,
                O::StoredLens => tos_query::knowledge_lens::STORED_LENS_INTENDED_USE,
                O::SearchCapabilities => {
                    tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_INTENDED_USE
                }
                O::Contracts => tos_query::knowledge_contracts::KNOWLEDGE_CONTRACTS_INTENDED_USE,
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
                registries: [
                    (
                        selected.entity_registry_id.clone(),
                        selected.entity_registry_sha256,
                    ),
                    (
                        selected.relation_registry_id.clone(),
                        selected.relation_registry_sha256,
                    ),
                ],
                registry_grants: 0,
                philosophy_granted: false,
                corpus_granted: false,
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
        fn authorize_corpus_view_identity_current(
            &mut self,
            _: &tos_compiler::CorpusOriginalReceipt,
            _: u64,
            _: Option<&str>,
            _: Digest256,
        ) -> Result<(), SearchV2Error> {
            self.check_selected()?;
            assert_eq!(self.inspect.operation_id, O::CorpusSummary.id());
            self.corpus_granted = true;
            Ok(())
        }
        fn authorize_corpus_original_current(
            &mut self,
            receipt: &tos_compiler::CorpusOriginalReceipt,
            collection: tos_compiler::CorpusOriginalCollection,
            ordinal: u64,
            raw: &[u8],
            sha: Digest256,
        ) -> Result<(), SearchV2Error> {
            assert_eq!(
                self.inspect.intended_use,
                tos_query::corpus_read::CORPUS_INTENDED_USE
            );
            assert_eq!(
                receipt.descriptor_sha256,
                self.inspect.descriptor_sha256.to_hex()
            );
            assert_eq!(receipt.source_cut, self.inspect.source_cut);
            assert_eq!(
                receipt.membership_root,
                self.inspect.source_membership_root.to_hex()
            );
            assert_eq!(Digest256::of_bytes(raw), sha);
            match collection {
                tos_compiler::CorpusOriginalCollection::Header => {
                    assert_eq!(ordinal, 0);
                    assert_eq!(sha.to_hex(), receipt.header_sha256);
                }
                _ => assert!(
                    receipt
                        .collections
                        .iter()
                        .any(|c| c.collection == collection.as_str() && ordinal < c.rows)
                ),
            }
            self.corpus_granted = true;
            Ok(()) // Fixture-only transport reference, never installed authority.
        }

        fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
            Some(self.controls.clone())
        }
        fn authorize_philosophy_original_current(
            &mut self,
            receipt: &tos_compiler::PhilosophyOriginalReceipt,
            collection: tos_compiler::PhilosophyOriginalCollection,
            ordinal: u64,
            raw: &[u8],
            sha: Digest256,
        ) -> Result<(), SearchV2Error> {
            use tos_compiler::PhilosophyOriginalCollection as C;
            assert_eq!(
                self.inspect.intended_use,
                tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE
            );
            assert_eq!(receipt.source_cut, self.inspect.source_cut);
            assert_eq!(
                receipt.descriptor_sha256,
                self.inspect.descriptor_sha256.to_hex()
            );
            assert_eq!(
                receipt.membership_root,
                self.inspect.source_membership_root.to_hex()
            );
            assert_eq!(Digest256::of_bytes(raw), sha);
            match collection {
                C::Header => {
                    assert_eq!(ordinal, 0);
                    assert_eq!(sha.to_hex(), receipt.header_sha256);
                }
                C::Nodes => assert!(ordinal < receipt.nodes),
                C::Edges => assert!(ordinal < receipt.edges),
            }
            self.philosophy_granted = true;
            Ok(()) // Fixture-only oracle; production compares the retained producer receipt.
        }
        fn authorize_navigation_original_current(
            &mut self,
            _: &tos_compiler::NavigationOriginalReceipt,
            ordinal: i64,
            raw: &[u8],
            sha: Digest256,
        ) -> Result<(), SearchV2Error> {
            assert_eq!(self.inspect.operation_id, O::Dossier.id());
            assert!(ordinal >= -1);
            assert_eq!(Digest256::of_bytes(raw), sha);
            Ok(()) // Test-only QRY reference, never the managed-local factory.
        }
        fn authorize_registry_current(
            &mut self,
            id: &str,
            raw: &[u8],
            sha: Digest256,
        ) -> Result<(), SearchV2Error> {
            assert_eq!(self.inspect.operation_id, O::Contracts.id());
            assert_eq!(Digest256::of_bytes(raw), sha);
            let at = self
                .registries
                .iter()
                .position(|(selected_id, selected_sha)| selected_id == id && *selected_sha == sha)
                .expect("grant exact selected original registry carrier");
            self.registry_grants |= 1 << at;
            Ok(())
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
            if self.inspect.intended_use == tos_query::corpus_read::CORPUS_INTENDED_USE {
                assert!(self.corpus_granted);
                assert!(observed.is_empty());
            } else if self.inspect.intended_use
                == tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE
            {
                assert!(self.philosophy_granted);
                assert!(
                    observed.is_empty(),
                    "phi originals are separately granted under the same hold"
                );
            } else if self.inspect.operation_id == O::Contracts.id() {
                assert_eq!(
                    self.registry_grants, 3,
                    "one current hold must cover both selected registry grants"
                );
                assert!(
                    observed.is_empty(),
                    "registry carriers are separately granted, not graph rows"
                );
            } else if self.inspect.operation_id
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
        corpus_context: Option<tos_query::corpus_read::CorpusReadContext>,
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
            (operation.is_corpus()
                && self.fixture.corpus_original.is_some()
                && self.corpus_context.is_some())
                || (operation.is_philosophy() && self.fixture.philosophy_original.is_some())
                || (operation == O::Dossier && self.fixture.navigation_original.is_some())
                || matches!(
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
                        | O::Contracts
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
            let packet = if matches!(request, R::CorpusViewIds) {
                tos_access::knowledge::execute_selected_corpus_view_ids(
                    &mut model,
                    &bound,
                    &mut inspect,
                    tos_query::corpus_read::CorpusReadBudget {
                        inspect: budgets.inspect,
                        max_work_steps: budgets.inspect.max_read_vm_steps,
                    },
                    probe,
                )?
            } else if let R::Corpus(request) = &request {
                tos_access::knowledge::execute_selected_corpus(
                    &mut model,
                    &bound,
                    &mut inspect,
                    self.corpus_context.as_ref().expect("actual corpus context"),
                    request,
                    tos_query::corpus_read::CorpusReadBudget {
                        inspect: budgets.inspect,
                        max_work_steps: budgets.inspect.max_read_vm_steps,
                    },
                    probe,
                )?
            } else if matches!(&request, R::Contracts) {
                tos_access::knowledge::execute_selected_knowledge_contracts(
                    &mut model,
                    &bound,
                    &mut inspect,
                    self.fixture.registry_originals(),
                    contract_budget(),
                    budgets.inspect,
                    probe,
                )?
            } else {
                tos_access::knowledge::execute_selected_knowledge(
                    &mut model,
                    &bound,
                    &mut catalog,
                    &mut inspect,
                    &mut *self.checkpoints.lock().unwrap(),
                    request,
                    budgets,
                    probe,
                )?
            };
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
    fn contract_budget() -> tos_query::knowledge_contracts::KnowledgeContractBudget {
        tos_query::knowledge_contracts::KnowledgeContractBudget {
            max_input_bytes: 4_000_000,
            max_registry_bytes: 1_000_000,
            max_response_bytes: 1_000_000,
            json: JsonLimits::default(),
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
            corpus_context: None,
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
    }

    #[test]
    fn process_exploration_checkpoint_atomic_lifecycle_on_selected_state() {
        let executor = Arc::new(Executor {
            fixture: build_fixture(),
            corpus_context: None,
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
        // Another adapter sharing the store cannot prepare the same input
        // while this reservation is live, even on another thread.
        let mut peer = store.clone();
        let input = cursor.to_owned();
        let snapshot = revision.to_owned();
        let copied_state = state.clone();
        let packet = first.clone();
        std::thread::spawn(move || {
            assert!(matches!(
                peer.prepare(
                    Some(&input),
                    &snapshot,
                    Some(&copied_state),
                    &packet,
                    budgets().exploration
                ),
                Err(SearchV2Error {
                    code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                    ..
                })
            ));
        })
        .join()
        .unwrap();
        drop(staged);
        // Failure after token/input reservation must release both atomically.
        let mut tiny_response = budgets().exploration;
        tiny_response.read.max_response_bytes = 1;
        let mut oversized = store
            .prepare(Some(cursor), revision, Some(&state), &first, tiny_response)
            .unwrap();
        assert!(matches!(
            oversized.stage_response(b"{}"),
            Err(SearchV2Error {
                code: tos_query::search_v2::SearchV2ErrorCode::BudgetExceeded,
                ..
            })
        ));
        assert!(oversized.commit().is_err(), "unstaged body cannot commit");
        drop(oversized);
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

    #[test]
    fn managed_local_installed_entrypoint_retains_real_release_and_kernel_custody() {
        use std::process::Command;
        use tos_access::release_state::{ManagedRelease, NATIVE_DATA_SCHEMA};
        use tos_compiler::knowledge_full_fixture::{
            NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS, build_native_fixture_with_navigation_inputs,
        };
        use tos_compiler::{
            NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeKnowledgeSelection, NativeSelectionPaths,
            NativeSelectionProducer, prepare_native_knowledge_artifact,
        };
        use tos_foundation::{CanonicalProfile, JsonNumber, JsonNumberKind, canonical_bytes_v1};
        // Reuse the maintained software fixture, exporting only original inputs.
        // The existing independent QRY dossier case owns Python domain equality.
        let script = r#"
import json,sys,tempfile
from pathlib import Path
sys.path.insert(0,sys.argv[1])
from test_access_contract import write_fixture
with tempfile.TemporaryDirectory() as d:
 root=Path(d);write_fixture(root)
 nav=json.loads((root/'ToS/derived-exports/tos_corpus_index.min.json').read_text())['source_navigation']
 print(json.dumps(nav,ensure_ascii=False,separators=(',',':'),allow_nan=False))
"#;
        let output = Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../access/tests"
            ))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let nav = parse_json(
            &output.stdout,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let mut header = nav.clone();
        if let JsonValue::Object(fields) = &mut header {
            fields.retain(|(key, _)| !matches!(key.as_str(), Some("nodes" | "edges" | "rights")));
        }
        let original = |key: &str| {
            nav.object_get(key)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(json_bytes)
                .collect::<Vec<_>>()
        };
        let nodes = original("nodes");
        let edges = original("edges");
        let rights = original("rights");
        let fixture = build_native_fixture_with_navigation_inputs(
            &json_bytes(&header),
            &nodes.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            &edges.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            &rights.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        );
        let object_id = nav
            .object_get("nodes")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node.object_get("node_kind").and_then(JsonValue::as_str) == Some("item"))
            .unwrap()
            .object_get("node_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let cold_limits = fixture.cold_limits();
        let process = NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS;
        let base = fixture.path.parent().unwrap().to_path_buf();
        let install = base.join("software/bin/tos-access");
        fs::create_dir_all(install.parent().unwrap()).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_tos-access"), &install).unwrap();
        let software_sha = Digest256::of_bytes(&fs::read(&install).unwrap());
        // The producer ran in this existing test executable, not the consumer
        // executable. Keep its actual code fingerprint as a separate member.
        let producer_program = base.join("software/bin/native-producer-fixture");
        fs::copy(std::env::current_exe().unwrap(), &producer_program).unwrap();
        let producer_sha = Digest256::of_bytes(&fs::read(&producer_program).unwrap());
        let data_root = base.join("native-snapshot");
        fs::create_dir_all(data_root.join("data")).unwrap();
        let paths = NativeSelectionPaths {
            model: "data/model.sqlite3".into(),
            descriptor: "data/descriptor.json".into(),
            entity_registry: "data/entity-registry.json".into(),
            relation_registry: "data/relation-registry.json".into(),
        };
        fs::copy(&fixture.path, data_root.join(&paths.model)).unwrap();
        fs::write(data_root.join(&paths.descriptor), &fixture.descriptor_bytes).unwrap();
        let registries = fixture.registry_originals();
        fs::write(data_root.join(&paths.entity_registry), registries[0]).unwrap();
        fs::write(data_root.join(&paths.relation_registry), registries[1]).unwrap();
        fs::write(data_root.join("data/navigation-input.json"), &output.stdout).unwrap();
        // This public subset is declared by the existing software owner, never
        // discovered from a runtime checkout or current filename/count rule.
        let ledger_paths = tos_access::release_state::public_source_gap_paths().unwrap();
        let source_root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."));
        let ledger = ledger_paths
            .iter()
            .map(|source| {
                let raw = fs::read(source_root.join(source)).unwrap();
                let member = format!("data/{source}");
                fs::create_dir_all(data_root.join(&member).parent().unwrap()).unwrap();
                fs::write(data_root.join(&member), &raw).unwrap();
                (source.clone(), raw)
            })
            .collect::<Vec<_>>();

        // This explicit ioctl is confined to the fresh isolated owned copy.
        // OPS admits this exact case before execution; unsupported custody is
        // a concrete failure, never a skipped positive/fallback grant.
        let measurement = prepare_native_knowledge_artifact(
            &data_root.join(&paths.model),
            &fixture.stage_receipt,
        )
        .unwrap_or_else(|error| panic!("isolated native fs-verity custody refusal: {error}"));
        let selection = NativeKnowledgeSelection::from_producer(
            paths.clone(),
            NativeSelectionProducer {
                stage: fixture.stage_receipt.clone(),
                seal: fixture.seal_receipt.clone(),
                navigation_original: fixture.navigation_original.clone(),
                philosophy_original: None,
                corpus_original: None,
            },
            fixture.expectation.clone(),
            measurement,
            cold_limits,
            process,
            &fixture.descriptor_bytes,
            registries[0],
            registries[1],
            NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
            JsonLimits::default().max_bytes,
        )
        .unwrap();
        fs::write(
            data_root.join("data/native-selection.json"),
            selection.encode(JsonLimits::default().max_bytes).unwrap(),
        )
        .unwrap();
        let canonical = |value: &JsonValue| {
            canonical_bytes_v1(
                value,
                CanonicalProfile::CorpusSnapshotV1,
                JsonLimits::default(),
            )
            .unwrap()
        };
        let number = |n: u64| {
            JsonValue::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme: n.to_string(),
            })
        };
        let mut members = vec![
            paths.model.clone(),
            paths.descriptor.clone(),
            paths.entity_registry.clone(),
            paths.relation_registry.clone(),
            "data/native-selection.json".into(),
            "data/navigation-input.json".into(),
        ];
        members.extend(ledger_paths.iter().map(|source| format!("data/{source}")));
        members.sort();
        let members = members
            .into_iter()
            .map(|path| {
                let raw = fs::read(data_root.join(&path)).unwrap();
                object(vec![
                    ("path", text(&path)),
                    ("size_bytes", number(raw.len() as u64)),
                    ("sha256", text(&Digest256::of_bytes(&raw).to_hex())),
                ])
            })
            .collect();
        let source_sha = Digest256::of_bytes(&output.stdout).to_hex();
        let compiler = object(vec![
            ("schema", text(&fixture.expectation.model_abi)),
            ("compiler_version", text(tos_compiler::COMPILER_VERSION)),
            ("compiler_sha256", text(&producer_sha.to_hex())),
            (
                "compiler_paths",
                JsonValue::Array(vec![text("software/bin/native-producer-fixture")]),
            ),
            (
                "input_bindings",
                object(vec![(
                    "software/bin/native-producer-fixture",
                    text(&producer_sha.to_hex()),
                )]),
            ),
        ]);
        let mut source_bindings = vec![
            (
                tos_foundation::JsonString::from_utf8("data/navigation-input.json"),
                text(&source_sha),
            ),
            (
                tos_foundation::JsonString::from_utf8(
                    tos_access::release_state::RUNTIME_DATA_DECLARATION_PATH,
                ),
                text(
                    &Digest256::of_bytes(tos_access::release_state::RUNTIME_DATA_DECLARATION)
                        .to_hex(),
                ),
            ),
        ];
        source_bindings.extend(ledger.iter().map(|(source, raw)| {
            (
                tos_foundation::JsonString::from_utf8(source),
                text(&Digest256::of_bytes(raw).to_hex()),
            )
        }));
        let corpus = fixture.stage_receipt.membership_root.clone();
        let mut manifest = object(vec![
            ("schema_version", text(NATIVE_DATA_SCHEMA)),
            ("corpus_revision", text(&corpus)),
            ("input_bindings", JsonValue::Object(source_bindings)),
            ("compiler", compiler),
            ("members", JsonValue::Array(members)),
            ("native_selection", text("data/native-selection.json")),
        ]);
        let revision = Digest256::of_bytes(&canonical(&manifest)).to_hex();
        if let JsonValue::Object(fields) = &mut manifest {
            fields.push((
                tos_foundation::JsonString::from_utf8("data_revision"),
                text(&revision),
            ));
        }
        let manifest_raw = canonical(&manifest);
        fs::write(data_root.join("data/manifest.json"), &manifest_raw).unwrap();
        let root = base.join("release");
        for dir in [
            "pairs",
            "bindings",
            "revocations/data",
            "revocations/corpus",
            "revocations/software",
        ] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(root.join(".release.lock"), []).unwrap();
        let pair = object(vec![
            ("schema_version", text("tos_access_release_pair_v1")),
            ("software_sha256", text(&software_sha.to_hex())),
            ("data_revision", text(&revision)),
            (
                "data_manifest_sha256",
                text(&Digest256::of_bytes(&manifest_raw).to_hex()),
            ),
            ("corpus_revision", text(&corpus)),
            ("query_schema", text(&fixture.expectation.model_abi)),
            ("compiler_version", text(tos_compiler::COMPILER_VERSION)),
        ]);
        let pair_raw = canonical(&pair);
        let pair_id = Digest256::of_bytes(&pair_raw).to_hex();
        fs::write(root.join(format!("pairs/{pair_id}.json")), pair_raw).unwrap();
        fs::write(
            root.join(format!("bindings/{pair_id}.json")),
            canonical(&object(vec![
                ("data_root", text(data_root.to_str().unwrap())),
                ("software_archive", text(install.to_str().unwrap())),
            ])),
        )
        .unwrap();
        fs::write(
            root.join("current.json"),
            canonical(&object(vec![
                ("schema_version", text("tos_access_release_pointer_v1")),
                ("current", text(&pair_id)),
                ("previous", JsonValue::Null),
            ])),
        )
        .unwrap();
        let reference = Executor {
            fixture,
            corpus_context: None,
            held: Arc::new(AtomicUsize::new(0)),
            checkpoints: Mutex::new(
                tos_access::exploration_checkpoints::ProcessExplorationCheckpoints::new(
                    tos_access::exploration_checkpoints::CheckpointLimits {
                        ttl: Duration::from_secs(900),
                        max_entries: 128,
                        max_encoded_bytes: 32 * 1024 * 1024,
                    },
                )
                .unwrap(),
            ),
            controls: Arc::new(Controls::default()),
        };
        let catalog = reference
            .knowledge(R::Catalog, Arc::new(NeverAbort))
            .unwrap()
            .body;
        let dossier = reference
            .knowledge(
                R::Dossier {
                    object_id: object_id.clone(),
                    limit: 300,
                },
                Arc::new(NeverAbort),
            )
            .unwrap()
            .body;
        let source_gap_request = tos_query::source_gap::SourceGapRequest {
            query: String::new(),
            limit: 20,
        };
        let source_gap_budget = tos_query::source_gap::SourceGapBudget {
            json: JsonLimits::default(),
            max_work_steps: cold_limits.max_vm_steps,
            max_response_bytes: 1_048_576,
        };
        let borrowed_ledger = ledger
            .iter()
            .map(
                |(source, raw)| tos_query::source_gap::PublicSourceGapRecord {
                    source_ref: source,
                    raw,
                },
            )
            .collect::<Vec<_>>();
        let source_gap = tos_query::source_gap::compute_source_gap_packet(
            &borrowed_ledger,
            &source_gap_request,
            source_gap_budget,
            &NeverAbort,
        )
        .unwrap();
        // The established prlimit utility sets live child kernel limits, not an
        // ENV admission boolean. Cargo/test fixture setup remains unrestricted.
        let child = || {
            let mut cmd = Command::new("prlimit");
            cmd.arg(format!("--as={}", process.address_space_bytes))
                .arg(format!("--fsize={}", process.file_size_bytes))
                .arg("--")
                .arg(&install)
                .arg("--release-root")
                .arg(&root)
                .env_remove("TOS_RELEASE_ROOT");
            cmd
        };
        let result = child().args(["knowledge", "catalog"]).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, [catalog.as_slice(), b"\n"].concat());
        let descriptor = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|op| op.operation_id == O::Dossier.id())
            .unwrap();
        let input = mcp_input(
            &descriptor.mcp_tool,
            &object(vec![("object_id", text(&object_id))]),
        );
        let mut rpc = child()
            .arg("mcp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        rpc.stdin.take().unwrap().write_all(&input).unwrap();
        let result = rpc.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let frame_cap = tos_access::mcp::tool_result_frame_byte_bound(1_048_576, 65_536).unwrap();
        check_mcp_packet(last_frame(&result.stdout), &dossier, frame_cap);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut server = child()
            .arg("serve")
            .arg(address.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        let mut socket = loop {
            match TcpStream::connect(address) {
                Ok(socket) => break socket,
                Err(error) => {
                    let exited = server.try_wait().unwrap().is_some();
                    if exited || std::time::Instant::now() >= deadline {
                        if !exited {
                            server.kill().unwrap();
                        }
                        let output = server.wait_with_output().unwrap();
                        panic!(
                            "managed native HTTP startup failed {error}: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        };
        let path = descriptor
            .http_path
            .replace("{object_id}", &path_id(&object_id));
        let mut response = Vec::new();
        let received = socket
            .set_read_timeout(Some(Duration::from_secs(30)))
            .and_then(|_| write!(socket, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n"))
            .and_then(|_| socket.read_to_end(&mut response));
        received.unwrap();
        assert_eq!(http_packet(&response), dossier);
        for method in ["GET", "HEAD"] {
            let mut socket = TcpStream::connect(address).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            write!(
                socket,
                "{method} /api/source-gaps HTTP/1.1\r\nHost: localhost\r\n\r\n"
            )
            .unwrap();
            let mut bytes = vec![];
            socket.read_to_end(&mut bytes).unwrap();
            assert!(bytes.starts_with(b"HTTP/1.1 200 "));
            if method == "GET" {
                assert_eq!(http_packet(&bytes), source_gap);
            } else {
                assert!(bytes.ends_with(b"\r\n\r\n"));
                assert!(
                    String::from_utf8_lossy(&bytes)
                        .contains(&format!("Content-Length: {}", source_gap.len()))
                );
            }
        }
        server.kill().unwrap();
        server.wait().unwrap();
        let release = ManagedRelease::open(&root).unwrap();
        let ledger_packet = tos_access::managed_local::execute_selected_source_gap(
            &release,
            &source_gap_request,
            source_gap_budget,
            cold_limits.max_work_bytes as usize,
            Arc::new(NeverAbort),
        )
        .unwrap();
        assert_eq!(ledger_packet.body, source_gap);
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(".release.lock"))
            .unwrap();
        assert!(
            lock.try_lock().is_err(),
            "prepared public-ledger bytes retain the actual release holder"
        );
        drop(ledger_packet);
        lock.try_lock().unwrap();
        lock.unlock().unwrap();
        let lease = release.acquire().unwrap();
        assert!(
            lock.try_lock().is_err(),
            "real shared holder must serialize withdrawal"
        );
        drop(lease);
        lock.try_lock().unwrap();
        lock.unlock().unwrap();
        // Missing published bytes cannot turn the declared complete subset into
        // a smaller successful search result, even with an unchanged manifest.
        lock.try_lock().unwrap();
        let missing = data_root.join(format!("data/{}", ledger[0].0));
        fs::remove_file(&missing).unwrap();
        lock.unlock().unwrap();
        assert!(
            tos_access::managed_local::execute_selected_source_gap(
                &release,
                &source_gap_request,
                source_gap_budget,
                cold_limits.max_work_bytes as usize,
                Arc::new(NeverAbort)
            )
            .is_err()
        );
        lock.try_lock().unwrap();
        fs::write(missing, &ledger[0].1).unwrap();
        lock.unlock().unwrap();
        // An equal-SHA fresh inode without fs-verity is still not custody.
        // Corrupt only this isolated fixture, under its real release lock.
        lock.try_lock().unwrap();
        let retained = base.join("retained.verity.sqlite3");
        fs::rename(data_root.join(&paths.model), &retained).unwrap();
        fs::copy(&retained, data_root.join(&paths.model)).unwrap();
        lock.unlock().unwrap();
        let result = child().args(["knowledge", "catalog"]).output().unwrap();
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.is_empty());
        lock.try_lock().unwrap();
        fs::remove_file(data_root.join(&paths.model)).unwrap();
        fs::rename(&retained, data_root.join(&paths.model)).unwrap();
        lock.unlock().unwrap();
        let result = Command::new("prlimit")
            .args(["--as=unlimited", "--fsize=unlimited", "--"])
            .arg(&install)
            .arg("--release-root")
            .arg(&root)
            .args(["knowledge", "catalog"])
            .env_remove("TOS_RELEASE_ROOT")
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.is_empty());
        fs::rename(
            root.join(".release.lock"),
            root.join("retained.release.lock"),
        )
        .unwrap();
        let result = child().args(["knowledge", "catalog"]).output().unwrap();
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.is_empty());
        fs::rename(
            root.join("retained.release.lock"),
            root.join(".release.lock"),
        )
        .unwrap();
        let revoked = object(vec![
            ("schema_version", text("tos_access_release_revocation_v1")),
            ("kind", text("data")),
            ("digest", text(&revision)),
            ("reason", text("software fixture withdrawal")),
            ("owner_ref", text("maintained native release case")),
        ]);
        fs::write(
            root.join(format!("revocations/data/{revision}.json")),
            canonical(&revoked),
        )
        .unwrap();
        let result = child().args(["knowledge", "catalog"]).output().unwrap();
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.is_empty());
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
            corpus_context: None,
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
    fn maintained_selected_contracts_require_original_carriers_on_all_native_wires() {
        let executor = Arc::new(Executor {
            fixture: build_native_fixture(),
            corpus_context: None,
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
        let operation = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|op| op.operation_id == O::Contracts.id())
            .unwrap();
        let packet = executor
            .knowledge(R::Contracts, Arc::new(NeverAbort))
            .unwrap();
        let expected = packet.body.clone();
        drop(packet);
        let value =
            parse_json(&expected, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        let originals = executor.fixture.registry_originals();
        for (at, (key, _)) in tos_query::knowledge_contracts::KNOWLEDGE_REGISTRY_CONTRACTS
            .iter()
            .enumerate()
        {
            let registry = parse_json(
                originals[at],
                JsonMode::PublishedStrict,
                JsonLimits::default(),
            )
            .unwrap();
            // Complete packet emission sorts object keys; source registry
            // lexical identity is separately checked by QRY's exact SHA gate.
            // Compare every field/array/numeric kind in the same owner canonical
            // profile here; transport checks below still compare raw bytes.
            let identity = |value: &JsonValue| {
                tos_foundation::canonical_bytes_v1(
                    value,
                    tos_foundation::CanonicalProfile::SourceRecordDigestV1,
                    JsonLimits::default(),
                )
                .unwrap()
            };
            assert_eq!(
                identity(
                    value
                        .root()
                        .object_get("contracts")
                        .unwrap()
                        .object_get(key)
                        .unwrap()
                ),
                identity(registry.root())
            );
        }
        let args: Vec<String> = operation
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
            "contracts CLI: {}",
            String::from_utf8_lossy(&errors)
        );
        assert_eq!(&writer.bytes[..writer.bytes.len() - 1], expected);
        let response = handle_get(executor.as_ref(), "GET", &operation.http_path, profile);
        assert_eq!(
            response.status,
            200,
            "contracts HTTP: {}",
            String::from_utf8_lossy(&response.body)
        );
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        tos_access::http::write_response(&mut writer, response).unwrap();
        assert_eq!(http_packet(&writer.bytes), expected);
        let mut writer = McpHeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
            source_frame: false,
        };
        run_io(
            Cursor::new(mcp_input(&operation.mcp_tool, &object(vec![]))),
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
        let response = handle_get(executor.as_ref(), "HEAD", &operation.http_path, profile);
        assert_eq!(response.status, 200);
        let mut writer = HeldWriter {
            bytes: vec![],
            held: executor.held.clone(),
        };
        tos_access::http::write_response(&mut writer, response).unwrap();
        assert!(writer.bytes.ends_with(b"\r\n\r\n"));
        assert!(
            String::from_utf8_lossy(&writer.bytes)
                .contains(&format!("Content-Length: {}\r\n", expected.len()))
        );
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        // The common dispatcher cannot discover registries or issue their grants.
        let cold = executor.fixture.open().unwrap();
        let bound = bind_verified_knowledge(
            &cold,
            &executor.fixture.vocabulary,
            &executor.fixture.descriptor_bytes,
        )
        .unwrap();
        let mut model = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
        let mut catalog = Authority::new(
            &bound,
            &R::Contracts,
            executor.held.clone(),
            executor.controls.clone(),
        );
        let mut inspect = Authority::new(
            &bound,
            &R::Contracts,
            executor.held.clone(),
            executor.controls.clone(),
        );
        let denied = tos_access::knowledge::execute_selected_knowledge(
            &mut model,
            &bound,
            &mut catalog,
            &mut inspect,
            &mut *executor.checkpoints.lock().unwrap(),
            R::Contracts,
            budgets(),
            Arc::new(NeverAbort),
        );
        assert!(matches!(denied, Err(ref e) if e.code == tos_access::AccessErrorCode::Unavailable));
        assert_eq!(inspect.registry_grants, 0);
        let mut changed = originals[0].to_vec();
        changed.push(b' ');
        let denied = tos_access::knowledge::execute_selected_knowledge_contracts(
            &mut model,
            &bound,
            &mut inspect,
            [&changed, originals[1]],
            contract_budget(),
            budgets().inspect,
            Arc::new(NeverAbort),
        );
        assert!(denied.is_err());
        assert_eq!(inspect.registry_grants, 0);
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        drop(model);
        drop(bound);
        drop(cold);
        for action in [1, 2] {
            executor
                .controls
                .after_prepare
                .store(action, Ordering::SeqCst);
            let mut output = vec![];
            let mut errors = vec![];
            assert_eq!(
                cli::run_cli(&args, executor.as_ref(), profile, &mut output, &mut errors),
                1
            );
            assert!(output.is_empty());
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            executor.controls.revoked.store(false, Ordering::SeqCst);
            executor.controls.cancelled.store(false, Ordering::SeqCst);
            let response = handle_get(executor.as_ref(), "GET", &operation.http_path, profile);
            let mut output = vec![];
            tos_access::http::write_response(&mut output, response).unwrap();
            assert!(!output.starts_with(b"HTTP/1.1 200 "));
            assert!(!output.windows(expected.len()).any(|w| w == expected));
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            executor.controls.revoked.store(false, Ordering::SeqCst);
            executor.controls.cancelled.store(false, Ordering::SeqCst);
            let mut output = vec![];
            run_io(
                Cursor::new(mcp_input(&operation.mcp_tool, &object(vec![]))),
                &mut output,
                executor.as_ref(),
                mcp_profile,
            )
            .unwrap();
            assert!(!String::from_utf8_lossy(last_frame(&output)).contains("structuredContent"));
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            executor.controls.revoked.store(false, Ordering::SeqCst);
            executor.controls.cancelled.store(false, Ordering::SeqCst);
        }
    }

    #[test]
    fn maintained_selected_philosophy_get_head_and_mcp_hold_exact_packets() {
        use tos_compiler::{
            PhilosophyOriginalCollection as C,
            knowledge_full_fixture::build_native_fixture_with_philosophy_original,
        };
        let fixture = build_native_fixture_with_philosophy_original();
        let mut cold = fixture.open().unwrap();
        let read = |cold: &mut tos_compiler::VerifiedKnowledgeModel<'_>, collection, count| {
            cold.philosophy_original_page_under_caller_budget(
                collection,
                None,
                count,
                budgets().inspect.max_payload_bytes,
                budgets().inspect.max_decoded_bytes as u64,
            )
            .unwrap()
            .rows
            .into_iter()
            .map(|row| {
                parse_json(&row.raw, JsonMode::PublishedStrict, budgets().inspect.json)
                    .unwrap()
                    .into_root()
            })
            .collect::<Vec<_>>()
        };
        let header = read(&mut cold, C::Header, 1).remove(0);
        let nodes = read(&mut cold, C::Nodes, 2);
        let edge = read(&mut cold, C::Edges, 1).remove(0);
        let id =
            |value: &JsonValue, key| value.object_get(key).unwrap().as_str().unwrap().to_owned();
        let left = id(&nodes[0], "node_id");
        let right = id(&nodes[1], "node_id");
        let edge_id = id(&edge, "edge_id");
        let view = id(
            &header.object_get("views").unwrap().as_array().unwrap()[0],
            "view_id",
        );
        drop(cold);
        let executor = Executor {
            fixture,
            corpus_context: None,
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
        };
        let number = |n: usize| {
            JsonValue::Number(tos_foundation::JsonNumber {
                kind: tos_foundation::JsonNumberKind::Int,
                lexeme: n.to_string(),
            })
        };
        // The shared QRY differential owns thirteen independent Python packets.
        // These same actual originals exercise only maintained caller selectors and delivery.
        let path_args = |from: &str, to: &str, direction: &str, excluded: Vec<JsonValue>| {
            object(vec![
                ("from_id", text(from)),
                ("to_id", text(to)),
                ("max_depth", number(3)),
                ("direction", text(direction)),
                ("excluded_edge_ids", JsonValue::Array(excluded)),
            ])
        };
        let cases = vec![
            (
                O::PhilosophyNode,
                path_id(&left),
                String::new(),
                object(vec![("node_id", text(&left))]),
            ),
            (
                O::PhilosophyEdge,
                path_id(&edge_id),
                String::new(),
                object(vec![("edge_id", text(&edge_id))]),
            ),
            (
                O::PhilosophyNeighborhood,
                path_id(&left),
                "?depth=2&limit=1".into(),
                object(vec![
                    ("node_id", text(&left)),
                    ("depth", number(2)),
                    ("limit", number(1)),
                ]),
            ),
            (
                O::PhilosophyPath,
                String::new(),
                format!(
                    "?from={}&to={}&max_depth=3",
                    path_id(&left),
                    path_id(&right)
                ),
                path_args(&left, &right, "outgoing", vec![]),
            ),
            (
                O::PhilosophyPath,
                String::new(),
                format!(
                    "?from={}&to={}&max_depth=3&direction=incoming",
                    path_id(&right),
                    path_id(&left)
                ),
                path_args(&right, &left, "incoming", vec![]),
            ),
            (
                O::PhilosophyPath,
                String::new(),
                format!(
                    "?from={}&to={}&max_depth=3&direction=either&exclude={}",
                    path_id(&left),
                    path_id(&right),
                    path_id(&edge_id)
                ),
                path_args(&left, &right, "either", vec![text(&edge_id)]),
            ),
            (
                O::PhilosophyView,
                path_id(&view),
                "?limit=1".into(),
                object(vec![("view_id", text(&view)), ("limit", number(1))]),
            ),
            (
                O::PhilosophyViews,
                String::new(),
                String::new(),
                object(vec![]),
            ),
            (
                O::PhilosophyLayers,
                String::new(),
                String::new(),
                object(vec![]),
            ),
            (
                O::PhilosophyClusters,
                String::new(),
                format!("?view_id={}&limit=1", path_id(&view)),
                object(vec![("view_id", text(&view)), ("limit", number(1))]),
            ),
            (
                O::PhilosophyReview,
                String::new(),
                format!("?view_id={}", path_id(&view)),
                object(vec![("view_id", text(&view))]),
            ),
            (
                O::PhilosophySnapshot,
                String::new(),
                String::new(),
                object(vec![]),
            ),
            (
                O::PhilosophyUnresolved,
                String::new(),
                format!("?view_id={}", path_id(&view)),
                object(vec![("view_id", text(&view))]),
            ),
        ];
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
        let mcp_profile = profile.with_mcp_frame_budget(
            tos_access::mcp::tool_result_frame_byte_bound(
                profile.max_response_bytes,
                profile.max_request_bytes.min(profile.max_line_bytes),
            )
            .unwrap(),
        );
        for (op, encoded, query, args) in &cases {
            let operation = tos_access::registered_operations()
                .unwrap()
                .iter()
                .find(|row| row.operation_id == op.id())
                .unwrap();
            assert!(
                operation.cli_command.is_none(),
                "maintained phi has no one-shot CLI"
            );
            assert!(executor.knowledge_available(*op));
            let packet = executor
                .knowledge(R::from_arguments(*op, args).unwrap(), Arc::new(NeverAbort))
                .unwrap();
            let expected = packet.body.clone();
            drop(packet);
            let base = operation.http_path.split('{').next().unwrap();
            let target = format!("{base}{encoded}{query}");
            for method in ["GET", "HEAD"] {
                let response = handle_get(&executor, method, &target, profile);
                assert_eq!(
                    response.status,
                    200,
                    "{op:?}/{method}: {}",
                    String::from_utf8_lossy(&response.body)
                );
                let mut writer = HeldWriter {
                    bytes: vec![],
                    held: executor.held.clone(),
                };
                tos_access::http::write_response(&mut writer, response).unwrap();
                if method == "GET" {
                    assert_eq!(http_packet(&writer.bytes), expected);
                } else {
                    assert!(writer.bytes.ends_with(b"\r\n\r\n"));
                    assert!(
                        String::from_utf8_lossy(&writer.bytes)
                            .contains(&format!("Content-Length: {}", expected.len()))
                    );
                }
                assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            }
            let mut writer = McpHeldWriter {
                bytes: vec![],
                held: executor.held.clone(),
                source_frame: false,
            };
            run_io(
                Cursor::new(mcp_input(&operation.mcp_tool, args)),
                &mut writer,
                &executor,
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
        // The changed original-only hold must survive through the final transport fence.
        let (op, encoded, query, args) = &cases[0];
        let operation = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|row| row.operation_id == op.id())
            .unwrap();
        let target = format!(
            "{}{}{}",
            operation.http_path.split('{').next().unwrap(),
            encoded,
            query
        );
        for change in [1, 2] {
            executor
                .controls
                .after_prepare
                .store(change, Ordering::SeqCst);
            let response = handle_get(&executor, "GET", &target, profile);
            let mut bytes = vec![];
            tos_access::http::write_response(&mut bytes, response).unwrap();
            let marker = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            let header = std::str::from_utf8(&bytes[..marker]).unwrap();
            assert!(
                header.starts_with(if change == 1 {
                    "HTTP/1.1 409 "
                } else {
                    "HTTP/1.1 408 "
                }),
                "{header}"
            );
            let length = header
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
                .unwrap()
                .parse::<usize>()
                .unwrap();
            assert_eq!(
                bytes.len() - marker,
                length,
                "complete bounded refusal body"
            );
            let refusal = parse_json(
                &bytes[marker..],
                JsonMode::PublishedStrict,
                JsonLimits::default(),
            )
            .unwrap();
            assert_eq!(
                refusal
                    .root()
                    .object_get("code")
                    .and_then(JsonValue::as_str),
                Some(if change == 1 {
                    "stale_selection"
                } else {
                    "cancelled"
                })
            );
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            executor.controls.revoked.store(false, Ordering::SeqCst);
            executor.controls.cancelled.store(false, Ordering::SeqCst);
            let mut bytes = vec![];
            run_io(
                Cursor::new(mcp_input(&operation.mcp_tool, args)),
                &mut bytes,
                &executor,
                mcp_profile,
            )
            .unwrap();
            assert!(!String::from_utf8_lossy(last_frame(&bytes)).contains("structuredContent"));
            assert_eq!(executor.held.load(Ordering::SeqCst), 0);
            executor.controls.revoked.store(false, Ordering::SeqCst);
            executor.controls.cancelled.store(false, Ordering::SeqCst);
        }
        executor.controls.after_prepare.store(0, Ordering::SeqCst);
        let deadline = profile.with_query_timeout(Duration::ZERO);
        let response = handle_get(&executor, "GET", &target, deadline);
        assert_eq!(response.status, 408);
        assert!(String::from_utf8_lossy(&response.body).contains("deadline_exceeded"));
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn maintained_selected_legacy_search_and_capabilities_all_native_wires() {
        use tos_query::knowledge_legacy_search::LegacySearchRequest;
        let executor = Arc::new(Executor {
            fixture: build_native_fixture(),
            corpus_context: None,
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
            corpus_context: None,
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
                    O::from_id(&descriptor.operation_id).is_some_and(|op| {
                        op == O::ExplorationContracts || executor.knowledge_available(op)
                    })
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
                    O::from_id(&op.operation_id).is_some_and(|op| {
                        op == O::ExplorationContracts || executor.knowledge_available(op)
                    })
                }
            })
            .map(|op| op.mcp_tool.as_str())
            .collect();
        assert_eq!(seen, expected);
        // Exact contracts carrier/body lifecycle has its own affected fixture case.
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
    }
    fn captured_corpus_executor_fixture() -> (Executor, JsonValue) {
        let script = r#"
import hashlib,json,subprocess,sys,tempfile
from pathlib import Path
repo=Path(sys.argv[1]);sys.path[:0]=[str(repo/'access/src'),str(repo/'access/tests'),str(repo/'scripts')]
from fixture_support import write_corpus_topology_fixture
from corpus_archive import capture_git,restore_capture
base=Path(tempfile.mkdtemp(prefix='tos-access-corpus-'));source=base/'source';source.mkdir()
write_corpus_topology_fixture(source)
source_path='ToS/derived-exports/tos_corpus_index.min.json'
payload=json.loads((source/source_path).read_text())
def git(*args):
 return subprocess.check_output(['git','-C',str(source),*args],stderr=subprocess.PIPE,text=True).strip()
git('init','-q');git('add',source_path);git('-c','user.name=ToS Software Fixture','-c','user.email=fixture@example.invalid','commit','-qm','existing corpus read input')
commit=git('rev-parse','HEAD');tree=git('rev-parse','HEAD^{tree}')
capture=base/'capture';restored=base/'restored';capture_git(source,commit,[source_path],capture);restore_capture(capture,restored)
json.dump({'capture':str(capture),'restored':str(restored),'commit':commit,'tree':tree,'manifest_sha':hashlib.sha256((capture/'capture.json').read_bytes()).hexdigest(),'source_path':source_path,'node':payload['nodes'][0]['node_id'],'pack':payload['relation_packs'][-1]['pack_id'],'view':payload['graph_views'][0]['view_id'],'query':payload['nodes'][0]['label']},sys.stdout)
"#;
        let output = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata = parse_json(
            &output.stdout,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let value = |key| {
            metadata
                .root()
                .object_get(key)
                .and_then(JsonValue::as_str)
                .unwrap()
                .to_owned()
        };
        let fixture =
            tos_compiler::knowledge_full_fixture::build_native_fixture_with_captured_corpus(
                std::path::Path::new(&value("capture")),
                std::path::Path::new(&value("restored")),
                &value("commit"),
                &value("tree"),
                &value("manifest_sha"),
                &value("source_path"),
            );
        let executor = Executor {
            fixture,
            corpus_context: Some(tos_query::corpus_read::CorpusReadContext {
                tos_root: value("restored"),
                index_path: format!("{}/{}", value("restored"), value("source_path")),
            }),
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
        };
        (executor, metadata.into_root())
    }
    #[test]
    fn captured_selected_corpus_get_head_and_all_mcp_tools_hold_exact_packets() {
        // The QRY differential owns Python packet comparison. This same finite
        // maintained input is captured/restored by its real software producer;
        // here only transport and original-member hold boundaries are exercised.
        let (executor, metadata) = captured_corpus_executor_fixture();
        let value = |key| {
            metadata
                .object_get(key)
                .and_then(JsonValue::as_str)
                .unwrap()
                .to_owned()
        };
        let number = |n: usize| {
            JsonValue::Number(tos_foundation::JsonNumber {
                kind: tos_foundation::JsonNumberKind::Int,
                lexeme: n.to_string(),
            })
        };
        let cases = vec![
            (
                O::CorpusStatus,
                object(vec![]),
                String::new(),
                String::new(),
            ),
            (
                O::CorpusSummary,
                object(vec![]),
                String::new(),
                String::new(),
            ),
            (
                O::CorpusSearch,
                object(vec![("query", text(&value("query"))), ("limit", number(2))]),
                String::new(),
                format!("?query={}&limit=2", path_id(&value("query"))),
            ),
            (
                O::CorpusResources,
                object(vec![
                    ("resource_kind", text("")),
                    ("owner_branch", text("")),
                    ("limit", number(1)),
                ]),
                String::new(),
                String::new(),
            ),
            (
                O::CorpusNode,
                object(vec![("node_id", text(&value("node")))]),
                path_id(&value("node")),
                String::new(),
            ),
            (
                O::CorpusRelationPack,
                object(vec![("pack_id", text(&value("pack")))]),
                path_id(&value("pack")),
                String::new(),
            ),
            (
                O::CorpusGraphView,
                object(vec![
                    ("view_id", text(&value("view"))),
                    ("limit", number(1)),
                ]),
                path_id(&value("view")),
                "?limit=1".into(),
            ),
            (
                O::CorpusPacket,
                object(vec![
                    ("query", text("")),
                    ("view_id", text("")),
                    ("limit", number(1)),
                ]),
                String::new(),
                String::new(),
            ),
        ];
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
        let mcp_profile = profile.with_mcp_frame_budget(
            tos_access::mcp::tool_result_frame_byte_bound(
                profile.max_response_bytes,
                profile.max_request_bytes,
            )
            .unwrap(),
        );
        let mut http_count = 0;
        for (op, args, encoded, query) in &cases {
            let route = tos_access::registered_operations()
                .unwrap()
                .iter()
                .find(|row| row.operation_id == op.id())
                .unwrap();
            assert!(route.cli_command.is_none());
            assert!(executor.knowledge_available(*op));
            let prepared = executor
                .knowledge(R::from_arguments(*op, args).unwrap(), Arc::new(NeverAbort))
                .unwrap();
            let expected = prepared.body.clone();
            drop(prepared);
            if !route.http_method.is_empty() {
                http_count += 1;
                assert_eq!(route.http_method, "GET");
                let target = format!(
                    "{}{}{}",
                    route.http_path.split('{').next().unwrap(),
                    encoded,
                    query
                );
                for method in ["GET", "HEAD"] {
                    let response = handle_get(&executor, method, &target, profile);
                    assert_eq!(
                        response.status,
                        200,
                        "{op:?}: {}",
                        String::from_utf8_lossy(&response.body)
                    );
                    let mut writer = HeldWriter {
                        bytes: vec![],
                        held: executor.held.clone(),
                    };
                    tos_access::http::write_response(&mut writer, response).unwrap();
                    if method == "GET" {
                        assert_eq!(http_packet(&writer.bytes), expected);
                    } else {
                        assert!(writer.bytes.ends_with(b"\r\n\r\n"));
                        assert!(
                            String::from_utf8_lossy(&writer.bytes)
                                .contains(&format!("Content-Length: {}", expected.len()))
                        );
                    }
                    assert_eq!(executor.held.load(Ordering::SeqCst), 0);
                }
            } else {
                assert!(matches!(op, O::CorpusResources | O::CorpusPacket));
                assert!(route.http_path.is_empty());
            }
            let mut writer = McpHeldWriter {
                bytes: vec![],
                held: executor.held.clone(),
                source_frame: false,
            };
            run_io(
                Cursor::new(mcp_input(&route.mcp_tool, args)),
                &mut writer,
                &executor,
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
        assert_eq!(http_count, 6);
        // Original grants alone do not replace a current final delivery check.
        let (op, args, _, _) = &cases[0];
        let route = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|row| row.operation_id == op.id())
            .unwrap();
        executor.controls.after_prepare.store(1, Ordering::SeqCst);
        let response = handle_get(&executor, "GET", &route.http_path, profile);
        let mut wire = vec![];
        tos_access::http::write_response(&mut wire, response).unwrap();
        assert!(wire.starts_with(b"HTTP/1.1 409 "));
        assert!(!String::from_utf8_lossy(&wire).contains("\"index_exists\":true"));
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
        executor.controls.after_prepare.store(0, Ordering::SeqCst);
        executor.controls.revoked.store(false, Ordering::SeqCst);
        executor.controls.after_prepare.store(2, Ordering::SeqCst);
        let mut wire = vec![];
        run_io(
            Cursor::new(mcp_input(&route.mcp_tool, args)),
            &mut wire,
            &executor,
            mcp_profile,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(last_frame(&wire)).contains("isError"));
        assert_eq!(executor.held.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn installed_software_site_static_and_selected_boot_preserve_delivery_boundaries() {
        use tos_access::{
            http::{handle_get_with_software, write_response},
            site::SoftwareSite,
        };
        use tos_compiler::knowledge_full_fixture::build_native_fixture_with_philosophy_original;
        // Reuse both real selected producer fixtures; the companion ELF below
        // is only an integrity fixture and is never executed as a product.
        let (corpus, _) = captured_corpus_executor_fixture();
        let held = Arc::clone(&corpus.held);
        let controls = Arc::clone(&corpus.controls);
        let philosophy = Executor {
            fixture: build_native_fixture_with_philosophy_original(),
            corpus_context: None,
            held: Arc::clone(&held),
            controls: Arc::clone(&controls),
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
        };
        struct Boot {
            corpus: Executor,
            philosophy: Executor,
        }
        impl AccessExecutor for Boot {
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
            fn knowledge(
                &self,
                request: R,
                probe: Arc<dyn AbortProbe>,
            ) -> Result<PreparedPacket, AccessError> {
                match request {
                    R::CorpusViewIds => self.corpus.knowledge(request, probe),
                    R::PhilosophyViewIds => self.philosophy.knowledge(request, probe),
                    _ => unreachable!("boot does not prefetch maintained graph packets"),
                }
            }
        }
        let boot = Boot { corpus, philosophy };
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
        let expected_corpus = boot
            .knowledge(R::CorpusViewIds, Arc::new(Controls::default()))
            .unwrap();
        let expected_phi = boot
            .knowledge(R::PhilosophyViewIds, Arc::new(Controls::default()))
            .unwrap();
        let first = |raw: &[u8], field: &str| {
            let doc = parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
            doc.root().object_get(field).unwrap().as_array().unwrap()[0]
                .object_get("view_id")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        };
        let corpus_id = first(&expected_corpus.body, "graph_views");
        let phi_id = first(&expected_phi.body, "views");
        drop((expected_corpus, expected_phi));
        assert_eq!(held.load(Ordering::SeqCst), 0);
        let base = boot
            .philosophy
            .fixture
            .path
            .parent()
            .unwrap()
            .join("installed-site");
        fs::create_dir_all(base.join("access/src/tos_access/web_dist/assets")).unwrap();
        let executable = base.join("access/src/tos_access/tos-access");
        let mut header = vec![0u8; 64];
        header[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        header[18..20].copy_from_slice(b"\x3e\x00");
        fs::write(&executable, &header).unwrap();
        let js = b"export const softwareOwned=true;\n";
        let css = b"body{margin:0}\n";
        std::os::unix::fs::symlink(
            "tos-graph.js",
            base.join("access/src/tos_access/web_dist/assets/link.js"),
        )
        .unwrap();
        let js_path = base.join("access/src/tos_access/web_dist/assets/tos-graph.js");
        fs::write(&js_path, js).unwrap();
        fs::write(
            base.join("access/src/tos_access/web_dist/assets/tos-graph.css"),
            css,
        )
        .unwrap();
        let number = |n: usize| {
            JsonValue::Number(tos_foundation::JsonNumber {
                kind: tos_foundation::JsonNumberKind::Int,
                lexeme: n.to_string(),
            })
        };
        let member = |path: &str, raw: &[u8]| {
            object(vec![
                ("path", text(path)),
                ("size_bytes", number(raw.len())),
                ("sha256", text(&Digest256::of_bytes(raw).to_hex())),
            ])
        };
        let lock = b"version = 4\n";
        let pin = include_bytes!("../../../../rust-toolchain.toml");
        fs::write(base.join("Cargo.lock"), lock).unwrap();
        fs::write(base.join("rust-toolchain.toml"), pin).unwrap();
        let source_ref = Digest256::of_bytes(&boot.philosophy.fixture.descriptor_bytes).to_hex();
        let manifest = object(vec![
            ("schema_version", text("tos_software_bundle_manifest_v1")),
            ("software_ref", text(&source_ref)),
            ("data_included", JsonValue::Bool(false)),
            ("source_dirty", JsonValue::Bool(false)),
            (
                "native_access",
                object(vec![
                    ("schema_version", text("tos_native_access_build_v1")),
                    ("target", text("x86_64-unknown-linux-gnu")),
                    ("source_commit", text(&source_ref)),
                    ("source_tree", text(&source_ref)),
                    ("profile", text("debug")),
                    ("lock_sha256", text(&Digest256::of_bytes(lock).to_hex())),
                    (
                        "toolchain",
                        text(
                            include_str!("../../../../rust-toolchain.toml")
                                .lines()
                                .find_map(|line| {
                                    line.trim()
                                        .strip_prefix("channel = ")
                                        .and_then(|value| value.strip_prefix('"'))
                                        .and_then(|value| value.strip_suffix('"'))
                                })
                                .unwrap(),
                        ),
                    ),
                    ("sha256", text(&Digest256::of_bytes(&header).to_hex())),
                    ("size_bytes", number(header.len())),
                ]),
            ),
            (
                "members",
                JsonValue::Array(vec![
                    member("access/src/tos_access/tos-access", &header),
                    member("Cargo.lock", lock),
                    member("rust-toolchain.toml", pin),
                    member("access/src/tos_access/web_dist/assets/tos-graph.js", js),
                    member("access/src/tos_access/web_dist/assets/link.js", js),
                    member("access/src/tos_access/web_dist/assets/tos-graph.css", css),
                ]),
            ),
        ]);
        fs::write(
            base.join("software.manifest.json"),
            tos_foundation::emit_value_preserved_json(&manifest, JsonLimits::default()).unwrap(),
        )
        .unwrap();
        let site = SoftwareSite::open(&executable, Arc::new(Controls::default())).unwrap();
        // Actual kernel image A cannot be rebound to fixture path/proof B.
        // No executable copy or additional program is needed.
        let wrong_running = SoftwareSite::open_running(&executable, Arc::new(Controls::default()))
            .err()
            .unwrap();
        assert_eq!(wrong_running.code, tos_access::AccessErrorCode::Unavailable);
        assert_eq!(
            wrong_running.message,
            "installed path differs from running image"
        );

        struct TwoHolds {
            bytes: Vec<u8>,
            held: Arc<AtomicUsize>,
        }
        impl Write for TwoHolds {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                assert_eq!(self.held.load(Ordering::SeqCst), 2);
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                assert_eq!(self.held.load(Ordering::SeqCst), 2);
                Ok(())
            }
        }
        let response = handle_get_with_software(&boot, "GET", "/", profile, &site);
        assert_eq!(response.status, 200);
        let html = String::from_utf8(response.body.clone()).unwrap();
        assert!(html.contains(&format!("\"default_view\":\"{corpus_id}\"")));
        assert!(html.contains(&format!("\"default_philosophy_view\":\"{phi_id}\"")));
        assert!(html.contains("/static/assets/tos-graph.js"));
        let mut writer = TwoHolds {
            bytes: vec![],
            held: Arc::clone(&held),
        };
        write_response(&mut writer, response).unwrap();
        assert_eq!(held.load(Ordering::SeqCst), 0);
        let wire = String::from_utf8(writer.bytes).unwrap();
        for header in [
            "Content-Type: text/html; charset=utf-8",
            "Cache-Control: no-cache",
            "Permissions-Policy: tools=(self)",
            "Cross-Origin-Embedder-Policy: require-corp",
            "X-Frame-Options: DENY",
            "script-src 'self' 'nonce-",
        ] {
            assert!(wire.contains(header), "{header}");
        }
        let head = handle_get_with_software(&boot, "HEAD", "/", profile, &site);
        let mut writer = TwoHolds {
            bytes: vec![],
            held: Arc::clone(&held),
        };
        write_response(&mut writer, head).unwrap();
        assert!(writer.bytes.ends_with(b"\r\n\r\n"));
        assert_eq!(held.load(Ordering::SeqCst), 0);
        for (path, mime, raw) in [
            (
                "/static/assets/tos-graph.js",
                "text/javascript",
                js.as_slice(),
            ),
            ("/static/assets/tos-graph.css", "text/css", css.as_slice()),
        ] {
            for method in ["GET", "HEAD"] {
                let response = handle_get_with_software(&boot, method, path, profile, &site);
                assert_eq!(response.status, 200);
                assert_eq!(response.body, raw);
                let mut wire = vec![];
                write_response(&mut wire, response).unwrap();
                let split = wire.windows(4).position(|x| x == b"\r\n\r\n").unwrap() + 4;
                let headers = String::from_utf8_lossy(&wire[..split]);
                assert!(headers.contains(mime));
                assert!(headers.contains(&format!("Content-Length: {}", raw.len())));
                assert_eq!(
                    &wire[split..],
                    if method == "HEAD" {
                        b"".as_slice()
                    } else {
                        raw
                    }
                );
            }
        }
        for path in [
            "/static/../software.manifest.json",
            "/static/%2e%2e/software.manifest.json",
            "/static/%2Fetc/passwd",
            "/static/assets%5Coutside.js",
            "/static/%00",
        ] {
            assert_eq!(
                handle_get_with_software(&boot, "GET", path, profile, &site).status,
                400
            );
        }
        assert_eq!(
            handle_get_with_software(&boot, "GET", "/static/assets/undeclared.js", profile, &site)
                .status,
            404
        );
        assert_eq!(
            handle_get_with_software(&boot, "GET", "/static/assets/link.js", profile, &site).status,
            404
        );
        let no_owner = Synthetic {
            allowed: false,
            calls: Mutex::new(vec![]),
        };
        let unavailable = handle_get_with_software(&no_owner, "GET", "/", profile, &site);
        assert_eq!(unavailable.status, 200);
        let html = String::from_utf8(unavailable.body).unwrap();
        assert!(html.contains("\"default_view\":\"\""));
        assert!(html.contains("\"philosophy\":false"));
        let response = handle_get_with_software(&boot, "GET", "/", profile, &site);
        controls.revoked.store(true, Ordering::SeqCst);
        let mut wire = vec![];
        write_response(&mut wire, response).unwrap();
        assert_eq!(held.load(Ordering::SeqCst), 0);
        assert!(wire.starts_with(b"HTTP/1.1 409 Conflict"));
        assert!(!String::from_utf8_lossy(&wire).contains("window.__TOS_GRAPH_BOOT__"));
        controls.revoked.store(false, Ordering::SeqCst);
        // Replacement/deletion refuses without serving a different program.
        let staged = handle_get_with_software(&no_owner, "GET", "/", profile, &site);
        fs::rename(&executable, executable.with_extension("old")).unwrap();
        fs::write(&executable, &header).unwrap();
        let mut replaced = vec![];
        write_response(&mut replaced, staged).unwrap();
        assert!(replaced.starts_with(b"HTTP/1.1 503 Service Unavailable"));
        fs::remove_file(&executable).unwrap();
        assert_eq!(
            handle_get_with_software(&no_owner, "GET", "/", profile, &site).status,
            503
        );
        fs::rename(executable.with_extension("old"), &executable).unwrap();
        let site = SoftwareSite::open(&executable, Arc::new(Controls::default())).unwrap();
        let deleted = handle_get_with_software(&no_owner, "GET", "/", profile, &site);
        assert_eq!(deleted.status, 200);
        fs::remove_file(&executable).unwrap();
        let mut wire = vec![];
        write_response(&mut wire, deleted).unwrap();
        assert!(wire.starts_with(b"HTTP/1.1 503 Service Unavailable"));
        assert!(!String::from_utf8_lossy(&wire).contains("window.__TOS_GRAPH_BOOT__"));
        // Restore only the tiny integrity fixture for the independent asset
        // lifetime assertion; no real executable is copied or run.
        fs::write(&executable, &header).unwrap();
        let site = SoftwareSite::open(&executable, Arc::new(Controls::default())).unwrap();

        let response =
            handle_get_with_software(&boot, "GET", "/static/assets/tos-graph.js", profile, &site);
        fs::write(&js_path, b"changed").unwrap();
        let mut wire = vec![];
        write_response(&mut wire, response).unwrap();
        assert!(wire.starts_with(b"HTTP/1.1 503 Service Unavailable"));
        assert!(String::from_utf8_lossy(&wire).contains("Content-Type: application/json"));
        assert!(!wire.ends_with(js));
        let timed = handle_get_with_software(
            &boot,
            "GET",
            "/",
            profile.with_query_timeout(Duration::ZERO),
            &site,
        );
        assert_eq!(timed.status, 408);
        let cancelled = Arc::new(Controls::default());
        cancelled.cancelled.store(true, Ordering::SeqCst);
        assert_eq!(
            SoftwareSite::open(&executable, cancelled)
                .err()
                .unwrap()
                .code,
            tos_access::AccessErrorCode::Cancelled
        );
        // The real installed binary/layout acceptance is a separate admitted
        // assembly stage. This test never executes the integrity fixture ELF.
    }
    #[test]
    fn native_software_archive_streaming_and_retained_identity_refuse_rebinding() {
        use tos_access::software_archive::{ArchiveLimits, VerifiedArchive};
        use zip::{ZipWriter, write::SimpleFileOptions};
        // A small integrity-only image. It is never executed or represented as
        // admitted software/startup; the required actual case below owns that.
        let base = std::env::temp_dir().join(format!(
            "tos-archive-integrity-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&base).unwrap();
        let path = base.join("software.zip");
        let mut elf = vec![0u8; 64];
        elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        elf[18..20].copy_from_slice(&[0x3e, 0]);
        let lock = b"version = 4\n".to_vec();
        let pin = include_bytes!("../../../../rust-toolchain.toml").to_vec();
        let files = std::collections::BTreeMap::from([
            ("Cargo.lock", lock.clone()),
            ("rust-toolchain.toml", pin.clone()),
            ("access/src/tos_access/tos-access", elf.clone()),
            (
                "access/src/tos_access/web_dist/assets/tos-graph.css",
                b"body{}".to_vec(),
            ),
            (
                "access/src/tos_access/web_dist/assets/tos-graph.js",
                b"fixture-static-js".to_vec(),
            ),
        ]);
        let number = |n: usize| {
            JsonValue::Number(tos_foundation::JsonNumber {
                kind: tos_foundation::JsonNumberKind::Int,
                lexeme: n.to_string(),
            })
        };
        let source = Digest256::of_bytes(b"integrity-only-source").to_hex();
        let manifest = object(vec![
            ("schema_version", text("tos_software_bundle_manifest_v1")),
            ("software_ref", text(&source)),
            ("source_dirty", JsonValue::Bool(false)),
            ("data_included", JsonValue::Bool(false)),
            (
                "native_access",
                object(vec![
                    ("schema_version", text("tos_native_access_build_v1")),
                    ("target", text("x86_64-unknown-linux-gnu")),
                    ("source_commit", text(&source)),
                    ("source_tree", text(&source)),
                    ("profile", text("debug")),
                    ("sha256", text(&Digest256::of_bytes(&elf).to_hex())),
                    ("size_bytes", number(elf.len())),
                    ("lock_sha256", text(&Digest256::of_bytes(&lock).to_hex())),
                    (
                        "toolchain",
                        text(
                            toml::from_str::<toml::Value>(std::str::from_utf8(&pin).unwrap())
                                .unwrap()
                                .get("toolchain")
                                .unwrap()
                                .get("channel")
                                .unwrap()
                                .as_str()
                                .unwrap(),
                        ),
                    ),
                ]),
            ),
            (
                "members",
                JsonValue::Array(
                    files
                        .iter()
                        .map(|(name, bytes)| {
                            object(vec![
                                ("path", text(name)),
                                ("size_bytes", number(bytes.len())),
                                ("sha256", text(&Digest256::of_bytes(bytes).to_hex())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]);
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, raw) in files
            .iter()
            .map(|(n, b)| (*n, b.clone()))
            .chain(std::iter::once((
                "software.manifest.json",
                json_bytes(&manifest),
            )))
        {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored)
                        .unix_permissions(if name.ends_with("tos-access") {
                            0o755
                        } else {
                            0o644
                        }),
                )
                .unwrap();
            writer.write_all(&raw).unwrap();
        }
        let original = writer.finish().unwrap().into_inner();
        let sidecar = path.with_extension("zip.manifest.json");
        let mut external = manifest.clone();
        let JsonValue::Object(fields) = &mut external else {
            unreachable!()
        };
        fields.push((
            tos_foundation::JsonString::from_utf8("archive_sha256"),
            text(&Digest256::of_bytes(&original).to_hex()),
        ));
        fields.push((
            tos_foundation::JsonString::from_utf8("archive_size_bytes"),
            number(original.len()),
        ));
        fs::write(&path, &original).unwrap();
        fs::write(&sidecar, json_bytes(&external)).unwrap();
        let limits = ArchiveLimits {
            max_total_bytes: 100_000,
            max_archive_bytes: 100_000,
            max_members: 16,
            max_metadata_bytes: 16_384,
        };
        let mut verified = VerifiedArchive::open(&path, limits).unwrap();
        assert_eq!(verified.manifest(), &manifest);
        let intact = base.join("intact");
        verified.extract(&intact).unwrap();
        assert_eq!(
            fs::read(intact.join("access/src/tos_access/tos-access")).unwrap(),
            elf
        );
        assert!(
            VerifiedArchive::open(
                &path,
                ArchiveLimits {
                    max_metadata_bytes: 224,
                    ..limits
                }
            )
            .is_err(),
            "finite metadata admission rejects before owned central expansion"
        );
        // Whole archive SHA is updated, but the member CRC/SHA must still fail.
        let mut changed = original.clone();
        let offset = changed
            .windows(b"fixture-static-js".len())
            .position(|p| p == b"fixture-static-js")
            .unwrap();
        changed[offset] ^= 1;
        let JsonValue::Object(fields) = &mut external else {
            unreachable!()
        };
        let (_, digest) = fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("archive_sha256"))
            .unwrap();
        *digest = text(&Digest256::of_bytes(&changed).to_hex());
        fs::write(&path, &changed).unwrap();
        fs::write(&sidecar, json_bytes(&external)).unwrap();
        assert!(
            VerifiedArchive::open(&path, limits).is_err(),
            "CRC/SHA failure cannot be repaired by sidecar SHA"
        );
        let destination = base.join("extracted");
        assert!(
            verified.extract(&destination).is_err(),
            "previous verification does not survive retained-byte mutation"
        );
        assert!(!destination.exists());
        // A rebound pathname cannot replace the verified archive object.
        fs::write(&path, &original).unwrap();
        let JsonValue::Object(fields) = &mut external else {
            unreachable!()
        };
        let (_, digest) = fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("archive_sha256"))
            .unwrap();
        *digest = text(&Digest256::of_bytes(&original).to_hex());
        fs::write(&sidecar, json_bytes(&external)).unwrap();
        let mut retained = VerifiedArchive::open(&path, limits).unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"different mutable name").unwrap();
        assert!(retained.extract(&destination).is_err());
        assert!(!destination.exists());
    }
    #[test]
    #[ignore = "requires OPS-admitted native receipt, frontend products and finite archive budgets"]
    fn actual_native_software_archive_startup_and_unavailable_boot() {
        use std::process::{Command, Stdio};
        // This is a required host case when selected. Missing product custody
        // fails, rather than substituting CARGO_BIN_EXE or the tiny ELF fixture.
        let binary = std::env::var_os("TOS_NATIVE_MANAGED_CONSUMER_BIN")
            .expect("OPS must provide the exact admitted current native binary");
        let receipt = std::env::var_os("TOS_NATIVE_ACCESS_BUILD_RECEIPT")
            .expect("OPS must provide its exact build-owned receipt");
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap();
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tos-native-software-{}-{tick}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let package = root.join("software.zip");
        let installed = root.join("installed");
        let web_dist = std::env::var_os("TOS_NATIVE_SOFTWARE_WEB_DIST")
            .expect("OPS must provide the genuine admitted current frontend handoff");
        let total = std::env::var("TOS_NATIVE_SOFTWARE_MAX_TOTAL_BYTES")
            .expect("OPS must provide the admitted uncompressed closure budget");
        let compressed = std::env::var("TOS_NATIVE_SOFTWARE_MAX_ARCHIVE_BYTES")
            .expect("OPS must provide the admitted compressed archive budget");
        let count = std::env::var("TOS_NATIVE_SOFTWARE_MAX_MEMBERS")
            .expect("OPS must provide the finite software metadata/member budget");
        let metadata = std::env::var("TOS_NATIVE_SOFTWARE_MAX_METADATA_BYTES")
            .expect("OPS must provide the finite archive metadata structural budget");
        let source = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repository)
            .output()
            .unwrap();
        assert!(source.status.success());
        let source = String::from_utf8(source.stdout).unwrap();
        let limits = || {
            [
                "--max-total-bytes",
                total.as_str(),
                "--max-archive-bytes",
                compressed.as_str(),
                "--max-members",
                count.as_str(),
                "--max-metadata-bytes",
                metadata.as_str(),
            ]
        };
        let assembly = Command::new(&binary)
            .args(["software", "build"])
            .arg("--root")
            .arg(&repository)
            .arg("--web-dist")
            .arg(web_dist)
            .arg("--output")
            .arg(&package)
            .arg("--source-ref")
            .arg(source.trim())
            .arg("--native-access-binary")
            .arg(&binary)
            .arg("--native-access-receipt")
            .arg(receipt)
            .args(limits())
            .output()
            .unwrap();
        assert!(
            assembly.status.success(),
            "native Rust assembly: {}",
            String::from_utf8_lossy(&assembly.stderr)
        );
        let manifest = parse_json(
            &assembly.stdout,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: 1_049_600,
                ..JsonLimits::default()
            },
        )
        .unwrap()
        .into_root();
        assert_eq!(
            manifest.object_get("software_ref").unwrap().as_str(),
            Some(source.trim())
        );
        assert_eq!(
            manifest.object_get("data_included"),
            Some(&JsonValue::Bool(false))
        );
        assert_eq!(
            manifest.object_get("source_dirty"),
            Some(&JsonValue::Bool(false))
        );
        let verified = Command::new(&binary)
            .args(["software", "verify"])
            .arg("--archive")
            .arg(&package)
            .args(limits())
            .output()
            .unwrap();
        assert!(
            verified.status.success(),
            "native Rust verification: {}",
            String::from_utf8_lossy(&verified.stderr)
        );
        let extracted = Command::new(&binary)
            .args(["software", "extract"])
            .arg("--archive")
            .arg(&package)
            .arg("--destination")
            .arg(&installed)
            .args(limits())
            .output()
            .unwrap();
        assert!(
            extracted.status.success(),
            "native Rust extraction: {}",
            String::from_utf8_lossy(&extracted.stderr)
        );
        let overwrite = Command::new(&binary)
            .args(["software", "extract"])
            .arg("--archive")
            .arg(&package)
            .arg("--destination")
            .arg(&installed)
            .args(limits())
            .output()
            .unwrap();
        assert!(
            !overwrite.status.success(),
            "fresh extraction refuses overwrite"
        );
        let program = installed.join("access/src/tos_access/tos-access");
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        struct StopChild(std::process::Child);
        impl Drop for StopChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        // Assembly/verification/extraction are the real Rust tooling entrypoint.
        // The installed serve child has no Python/checkout discovery/data owner.
        let mut server = StopChild(
            Command::new(&program)
                .arg("serve")
                .arg(address.to_string())
                .current_dir(&outside)
                .env_clear()
                .env("PATH", "")
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            if TcpStream::connect(address).is_ok() {
                break;
            }
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "native software entrypoint exited before loopback readiness"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "native software entrypoint readiness timeout"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let request = |method: &str, path: &str| {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            write!(
                stream,
                "{method} {path} HTTP/1.1\r\nHost: localhost\r\n\r\n"
            )
            .unwrap();
            let mut wire = Vec::new();
            stream
                .take(16 * 1024 * 1024 + 65_537)
                .read_to_end(&mut wire)
                .unwrap();
            assert!(
                wire.len() <= 16 * 1024 * 1024 + 65_536,
                "bounded software wire"
            );
            wire
        };
        let split = |wire: &[u8]| {
            wire.windows(4)
                .position(|part| part == b"\r\n\r\n")
                .unwrap()
                + 4
        };
        let shell = request("GET", "/");
        let head_end = split(&shell);
        let headers = String::from_utf8_lossy(&shell[..head_end]);
        assert!(headers.starts_with("HTTP/1.1 200 "));
        assert!(headers.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(headers.contains("Cache-Control: no-cache\r\n"));
        assert!(headers.contains("Content-Security-Policy:") && headers.contains("'nonce-"));
        assert!(headers.contains("X-Content-Type-Options: nosniff\r\n"));
        assert!(headers.contains(&format!("Content-Length: {}\r\n", shell.len() - head_end)));
        let html = std::str::from_utf8(&shell[head_end..]).unwrap();
        let boot = html
            .split("window.__TOS_GRAPH_BOOT__=")
            .nth(1)
            .unwrap()
            .split(";</script>")
            .next()
            .unwrap();
        let boot = parse_json(
            boot.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        for field in ["default_view", "default_philosophy_view"] {
            assert_eq!(boot.object_get(field).unwrap().as_str(), Some(""));
        }
        assert_eq!(
            boot.object_get("write_enabled"),
            Some(&JsonValue::Bool(false))
        );
        for capability in ["corpus", "philosophy"] {
            assert_eq!(
                boot.object_get("capabilities")
                    .unwrap()
                    .object_get(capability),
                Some(&JsonValue::Bool(false))
            );
        }
        let head = request("HEAD", "/");
        assert_eq!(split(&head), head.len());
        assert!(
            String::from_utf8_lossy(&head)
                .contains(&format!("Content-Length: {}\r\n", shell.len() - head_end))
        );
        for (asset, mime) in [
            ("tos-graph.js", "text/javascript"),
            ("tos-graph.css", "text/css"),
        ] {
            let path = format!("/static/assets/{asset}");
            let member = installed
                .join("access/src/tos_access/web_dist/assets")
                .join(asset);
            assert!(fs::metadata(&member).unwrap().len() <= 16 * 1024 * 1024);
            let expected = fs::read(&member).unwrap();
            let wire = request("GET", &path);
            assert_eq!(http_packet(&wire), expected);
            let headers = String::from_utf8_lossy(&wire[..split(&wire)]);
            assert!(headers.contains(&format!("Content-Type: {mime}\r\n")));
            assert!(headers.contains("Cache-Control: no-cache\r\n"));
            let head = request("HEAD", &path);
            assert_eq!(split(&head), head.len());
            assert!(
                String::from_utf8_lossy(&head)
                    .contains(&format!("Content-Length: {}\r\n", expected.len()))
            );
        }
        let unavailable = request("GET", "/api/knowledge/catalog");
        assert!(unavailable.starts_with(b"HTTP/1.1 503 "));
        let traversal = request("GET", "/static/%2e%2e/Cargo.lock");
        assert!(!traversal.starts_with(b"HTTP/1.1 200 "));
        // A live process must refuse disappearance of its exact software image.
        let moved = program.with_extension("held");
        fs::rename(&program, &moved).unwrap();
        let refused = request("GET", "/");
        assert!(refused.starts_with(b"HTTP/1.1 503 "));
        assert!(!String::from_utf8_lossy(&refused).contains("window.__TOS_GRAPH_BOOT__"));
        drop(server);
        // Keep admitted package/installation evidence in TMPDIR for OPS custody.
    }
}
