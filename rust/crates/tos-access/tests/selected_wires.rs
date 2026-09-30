#![cfg(not(target_arch = "wasm32"))]

use std::{
    fs,
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Write},
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
    ) -> Result<PreparedPacket<'static>, AccessError> {
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
    impl<'hold> CatalogCurrentAuthority<'hold> for Authority {
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
        ) -> Result<Box<dyn CatalogDisclosureLease + 'hold>, CatalogError> {
            Ok(Box::new(self.lease()))
        }
    }
    impl<'hold> InspectCurrentAuthority<'hold> for Authority {
        fn authorize_corpus_view_identity_current(
            &mut self,
            _: &tos_compiler::CorpusOriginalReceipt,
            _: u64,
            _: Option<&str>,
            _: Digest256,
        ) -> Result<(), SearchV2Error> {
            InspectCurrentAuthority::check_selected(self)?;
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
        ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
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
        ) -> Result<PreparedPacket<'static>, AccessError> {
            unreachable!()
        }
        fn knowledge_search_legacy_available(&self) -> bool {
            true
        }
        fn knowledge_search_legacy(
            &self,
            request: tos_query::knowledge_legacy_search::LegacySearchRequest,
            probe: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket<'static>, AccessError> {
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
        ) -> Result<PreparedPacket<'static>, AccessError> {
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
    #[ignore = "requires OPS-retained binary and source checkout plus isolated fs-verity admission"]
    fn managed_local_installed_entrypoint_retains_real_release_and_kernel_custody() {
        use std::process::Command;
        use tos_access::release_state::{ManagedRelease, NATIVE_DATA_SCHEMA};
        use tos_compiler::knowledge_full_fixture::{
            NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS,
            build_native_fixture_with_navigation_inputs_bounded,
        };
        use tos_compiler::{
            NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeKnowledgeSelection, NativeSelectionPaths,
            NativeSelectionProducer, prepare_native_knowledge_artifact,
        };
        use tos_foundation::{CanonicalProfile, JsonNumber, JsonNumberKind, canonical_bytes_v1};
        // Absolute setup deadline belongs to the existing 240-second finite
        // isolated case. The enclosing admitted runner remains the hard wall.
        let producer_deadline = std::time::Instant::now() + Duration::from_secs(240);
        use crate::native_child::{OwnedChild, bounded_child_output, bounded_output, bounded_sha};
        fn bounded_wait(child: &mut OwnedChild) -> std::process::ExitStatus {
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    return status;
                }
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("native interactive child exceeded 60-second deadline");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        fn bounded_line<R: BufRead>(reader: &mut R, max: usize) -> Result<Option<String>, String> {
            let mut bytes = Vec::new();
            loop {
                let available = reader.fill_buf().map_err(|error| error.to_string())?;
                if available.is_empty() {
                    return if bytes.is_empty() {
                        Ok(None)
                    } else {
                        Err("native MCP frame ended before newline".into())
                    };
                }
                let take = available
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(available.len(), |index| index + 1);
                if bytes.len().saturating_add(take) > max {
                    return Err("native MCP frame exceeded byte cap".into());
                }
                bytes.extend_from_slice(&available[..take]);
                reader.consume(take);
                if bytes.last() == Some(&b'\n') {
                    return String::from_utf8(bytes)
                        .map(Some)
                        .map_err(|_| "native MCP frame is not UTF-8".into());
                }
            }
        }
        fn next_mcp_frame(
            frames: &std::sync::mpsc::Receiver<Result<Option<String>, String>>,
            child: &mut OwnedChild,
        ) -> String {
            match frames.recv_timeout(Duration::from_secs(60)) {
                Ok(Ok(Some(frame))) => frame,
                other => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("native MCP frame unavailable within 60 seconds: {other:?}");
                }
            }
        }
        fn bounded_http(socket: &mut TcpStream) -> Vec<u8> {
            const HTTP_MAX: usize = 1_048_576 + 8192;
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                assert!(
                    !remaining.is_zero(),
                    "native HTTP response exceeded 30-second deadline"
                );
                socket.set_read_timeout(Some(remaining)).unwrap();
                let read = socket.read(&mut chunk).unwrap();
                if read == 0 {
                    return bytes;
                }
                assert!(
                    bytes.len().saturating_add(read) <= HTTP_MAX,
                    "native HTTP response exceeded byte cap"
                );
                bytes.extend_from_slice(&chunk[..read]);
            }
        }
        fn source_guard(source_root: &std::path::Path, source_ref: &str, ledger_paths: &[String]) {
            let head = bounded_output(
                Command::new("git")
                    .current_dir(source_root)
                    .args(["rev-parse", "HEAD"]),
                128,
            );
            assert!(head.status.success());
            assert_eq!(String::from_utf8(head.stdout).unwrap().trim(), source_ref);
            let clean = bounded_output(
                Command::new("git")
                    .current_dir(source_root)
                    .args([
                        "status",
                        "--porcelain=v1",
                        "--untracked-files=all",
                        "--",
                        "access",
                        "rust",
                        "ToS/doctrine/semantic-interchange",
                    ])
                    .args(ledger_paths),
                4096,
            );
            assert!(
                clean.status.success() && clean.stdout.is_empty(),
                "retained selected source closure is dirty"
            );
        }
        fn same_search_rows(left: &JsonValue, right: &JsonValue) {
            for key in ["source_revision", "query", "filters", "nodes", "relations"] {
                assert_eq!(
                    left.object_get(key),
                    right.object_get(key),
                    "indexed page differs across wire/process for {key}"
                );
            }
        }
        fn advanced_search_rows(first: &JsonValue, second: &JsonValue) {
            let mut advanced = false;
            for key in ["nodes", "relations"] {
                let first = first.object_get(key).unwrap().as_array().unwrap();
                let second = second.object_get(key).unwrap().as_array().unwrap();
                assert!(first.len() <= 1 && second.len() <= 1);
                for row in second {
                    assert!(
                        !first.contains(row),
                        "resumed indexed page repeated the first {key} carrier"
                    );
                }
                advanced |= !second.is_empty();
            }
            assert!(
                advanced,
                "fixture must exercise real nonempty resumed search results"
            );
        }
        fn bounded_source(path: &std::path::Path) -> Vec<u8> {
            let mut file = tos_fd_open::open_absolute_regular(path, 1_048_576).unwrap();
            let mut raw = Vec::new();
            std::io::Read::by_ref(&mut file)
                .take(1_048_577)
                .read_to_end(&mut raw)
                .unwrap();
            assert!(
                raw.len() <= 1_048_576,
                "selected source grew beyond byte cap"
            );
            raw
        }
        let selected_binary = PathBuf::from(
            std::env::var_os("TOS_NATIVE_INDEXED_CONSUMER_BIN")
                .expect("OPS must provide the retained native indexed consumer ELF"),
        )
        .canonicalize()
        .unwrap();
        let expected_binary_sha = std::env::var("TOS_NATIVE_INDEXED_CONSUMER_SHA256")
            .expect("OPS must provide the retained ELF sha256");
        assert_eq!(
            bounded_sha(&selected_binary, 256 * 1024 * 1024).to_hex(),
            expected_binary_sha,
            "selected ELF changed before fixture assembly"
        );
        let producer_binary = std::env::current_exe().unwrap();
        let expected_producer_sha = std::env::var("TOS_NATIVE_INDEXED_PRODUCER_SHA256")
            .expect("OPS must provide the retained test ELF sha256");
        assert_eq!(
            bounded_sha(&producer_binary, 512 * 1024 * 1024).to_hex(),
            expected_producer_sha,
            "selected producer ELF changed before fixture assembly"
        );
        let source_root = PathBuf::from(
            std::env::var_os("TOS_NATIVE_INDEXED_SOURCE_ROOT")
                .expect("OPS must provide a retained source checkout"),
        )
        .canonicalize()
        .unwrap();
        let compile_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("compiler crate path has a repository root");
        assert_ne!(
            source_root.as_path(),
            compile_root,
            "runtime fixture source must be retained separately from the shared compiler checkout"
        );
        // OPS sets this only for the frozen source before compiling this ELF.
        // A runtime environment value cannot silently select different fixture,
        // imported Python software or public-ledger source than that build.
        let compiled_source_ref = option_env!("TOS_NATIVE_INDEXED_COMPILED_SOURCE_COMMIT")
            .expect("OPS must bind this test ELF to its immutable source commit at compile time");
        let source_ref = std::env::var("TOS_NATIVE_INDEXED_SOURCE_COMMIT")
            .expect("OPS must provide the exact retained source commit");
        assert!(source_ref.len() == 40 && source_ref.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            source_ref, compiled_source_ref,
            "runtime source differs from compiled source selection"
        );
        let ledger_paths = tos_access::release_state::public_source_gap_paths().unwrap();
        source_guard(&source_root, &source_ref, &ledger_paths);
        assert_eq!(
            bounded_source(&source_root.join("access/tests/test_access_contract.py")),
            include_bytes!("../../../../access/tests/test_access_contract.py").as_slice(),
            "Python fixture source differs from compile-time input"
        );
        assert_eq!(
            bounded_source(
                &source_root.join(tos_access::release_state::RUNTIME_DATA_DECLARATION_PATH)
            ),
            tos_access::release_state::RUNTIME_DATA_DECLARATION,
            "public-ledger selection differs from compile-time input"
        );
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
        let output = bounded_output(
            Command::new("python3")
                .args(["-I", "-B", "-c"])
                .arg(script)
                .arg(source_root.join("access/tests")),
            1_048_576,
        );
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
        // Same declared selected-cold envelope for the existing producer stage.
        // Per-family normalization, Search and temp caps remain distinct; the
        // admitted recipe accounts for their coexisting physical products.
        let stage_limits = tos_compiler::knowledge_stage::StageLimits {
            sqlite: tos_compiler::Limits {
                max_rows: 100_000,
                max_row_bytes: 1_048_576,
                max_output_bytes: 64 * 1024 * 1024,
                max_work_bytes: 100 * 1024 * 1024,
                sqlite_cache_kib: 8192,
                max_sql_vm_steps: 100_000_000,
            },
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 2,
            max_seek_bytes: 1_048_576,
        };
        let fixture = build_native_fixture_with_navigation_inputs_bounded(
            &json_bytes(&header),
            &nodes.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            &edges.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            &rights.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            stage_limits,
            producer_deadline,
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
        assert_eq!(
            stage_limits.sqlite.max_output_bytes,
            cold_limits.max_file_bytes
        );
        assert_eq!(
            stage_limits.sqlite.max_work_bytes,
            cold_limits.max_work_bytes
        );
        assert_eq!(
            stage_limits.sqlite.max_sql_vm_steps,
            cold_limits.max_vm_steps
        );
        assert_eq!(stage_limits.sqlite.max_rows, cold_limits.max_rows);
        assert_eq!(
            stage_limits.sqlite.max_row_bytes as usize,
            cold_limits.max_row_bytes
        );
        assert_eq!(
            u64::from(stage_limits.sqlite.sqlite_cache_kib),
            cold_limits.sqlite_cache_kib
        );
        let process = NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS;
        let base = fixture.path.parent().unwrap().to_path_buf();
        let install = base.join("software/bin/tos-access");
        fs::create_dir_all(install.parent().unwrap()).unwrap();
        fs::copy(&selected_binary, &install).unwrap();
        let software_sha = bounded_sha(&install, 256 * 1024 * 1024);
        assert_eq!(software_sha.to_hex(), expected_binary_sha);
        // The producer ran in this existing test executable, not the consumer
        // executable. Keep its actual code fingerprint as a separate member.
        let producer_program = base.join("software/bin/native-producer-fixture");
        fs::copy(&producer_binary, &producer_program).unwrap();
        let producer_sha = bounded_sha(&producer_program, 512 * 1024 * 1024);
        assert_eq!(
            producer_sha.to_hex(),
            expected_producer_sha,
            "copied producer ELF differs from retained running image"
        );
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
        source_guard(&source_root, &source_ref, &ledger_paths);
        let ledger = ledger_paths
            .iter()
            .map(|source| {
                let raw = bounded_source(&source_root.join(source));
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
                managed_source: None,
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
                let member = data_root.join(&path);
                let sha = bounded_sha(&member, cold_limits.max_file_bytes);
                object(vec![
                    ("path", text(&path)),
                    ("size_bytes", number(fs::metadata(&member).unwrap().len())),
                    ("sha256", text(&sha.to_hex())),
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
        let result = bounded_output(child().args(["knowledge", "catalog"]), 1_048_576);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, [catalog.as_slice(), b"\n"].concat());
        let indexed_cli = bounded_output(
            child().args([
                "knowledge",
                "search",
                "source",
                "--mode",
                "indexed",
                "--limit",
                "1",
            ]),
            1_048_576,
        );
        assert!(
            indexed_cli.status.success(),
            "{}",
            String::from_utf8_lossy(&indexed_cli.stderr)
        );
        let cli_packet = parse_json(
            &indexed_cli.stdout,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        assert_eq!(
            cli_packet.object_get("schema").unwrap().as_str(),
            Some("tos_knowledge_search_indexed_v2")
        );
        let cli_cursor = cli_packet
            .object_get("page")
            .unwrap()
            .object_get("next_cursor")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        assert!(cli_cursor.len() <= 16 * 1024);
        let cli_resume = bounded_output(
            child().args([
                "knowledge",
                "search",
                "source",
                "--mode",
                "indexed",
                "--limit",
                "1",
                "--cursor",
                &cli_cursor,
            ]),
            1_048_576,
        );
        assert!(
            cli_resume.status.success(),
            "{}",
            String::from_utf8_lossy(&cli_resume.stderr)
        );
        let cli_resume = parse_json(
            &cli_resume.stdout,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        assert_eq!(
            cli_resume
                .object_get("page")
                .unwrap()
                .object_get("cursor")
                .unwrap()
                .as_str(),
            Some(cli_cursor.as_str())
        );
        advanced_search_rows(&cli_packet, &cli_resume);
        let descriptor = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|op| op.operation_id == O::Dossier.id())
            .unwrap();
        let input = mcp_input(
            &descriptor.mcp_tool,
            &object(vec![("object_id", text(&object_id))]),
        );
        let mut rpc = OwnedChild(
            child()
                .arg("mcp")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        rpc.stdin.take().unwrap().write_all(&input).unwrap();
        let result = bounded_child_output(rpc, 3 * 1024 * 1024);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let frame_cap = tos_access::mcp::tool_result_frame_byte_bound(1_048_576, 65_536).unwrap();
        check_mcp_packet(last_frame(&result.stdout), &dossier, frame_cap);
        // Actual MCP pages use the same unsigned paging request and reacquire
        // the selected release holder for every page.
        let mut search_rpc = OwnedChild(
            child()
                .arg("mcp")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut search_input = search_rpc.stdin.take().unwrap();
        let mut search_output = BufReader::new(search_rpc.stdout.take().unwrap());
        let (frames_tx, frames_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            loop {
                let result = bounded_line(&mut search_output, frame_cap);
                let done = !matches!(result, Ok(Some(_)));
                if frames_tx.send(result).is_err() || done {
                    break;
                }
            }
        });
        writeln!(search_input, "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2025-11-25\"}}}}")
            .unwrap();
        let mut frame = next_mcp_frame(&frames_rx, &mut search_rpc);
        assert!(frame.contains("\"protocolVersion\":\"2025-11-25\""));
        writeln!(
            search_input,
            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}"
        )
        .unwrap();
        writeln!(search_input, "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"tos_knowledge_search\",\"arguments\":{{\"mode\":\"indexed\",\"query\":\"source\",\"limit\":1}}}}}}")
            .unwrap();
        frame = next_mcp_frame(&frames_rx, &mut search_rpc);
        let first = parse_json(
            frame.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let first = first
            .object_get("result")
            .unwrap()
            .object_get("structuredContent")
            .unwrap();
        assert_eq!(
            first.object_get("schema").unwrap().as_str(),
            Some("tos_knowledge_search_indexed_v2")
        );
        same_search_rows(&cli_packet, first);
        let search_cursor = first
            .object_get("page")
            .unwrap()
            .object_get("next_cursor")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        assert!(search_cursor.len() <= 16 * 1024);
        writeln!(search_input, "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{{\"name\":\"tos_knowledge_search\",\"arguments\":{{\"mode\":\"indexed\",\"query\":\"source\",\"limit\":1,\"cursor\":\"{search_cursor}\"}}}}}}")
            .unwrap();
        frame = next_mcp_frame(&frames_rx, &mut search_rpc);
        let second = parse_json(
            frame.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let second = second
            .object_get("result")
            .unwrap()
            .object_get("structuredContent")
            .unwrap();
        assert_eq!(
            second
                .object_get("page")
                .unwrap()
                .object_get("cursor")
                .unwrap()
                .as_str(),
            Some(search_cursor.as_str())
        );
        same_search_rows(&cli_resume, second);
        drop(search_input);
        drop(frames_rx);
        let search_result = bounded_wait(&mut search_rpc);
        assert!(
            search_result.success(),
            "native MCP child refused completion: {search_result}"
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut server = OwnedChild(
            child()
                .arg("serve")
                .arg(address.to_string())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
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
                        let output = bounded_child_output(server, 16 * 1024);
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
        socket
            .set_write_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        write!(socket, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        let response = bounded_http(&mut socket);
        assert_eq!(http_packet(&response), dossier);
        for method in ["GET", "HEAD"] {
            let mut socket = TcpStream::connect(address).unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            write!(
                socket,
                "{method} /api/source-gaps HTTP/1.1\r\nHost: localhost\r\n\r\n"
            )
            .unwrap();
            let bytes = bounded_http(&mut socket);
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
        let search_http = |target: &str| {
            let mut socket = TcpStream::connect(address).unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            write!(socket, "GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            bounded_http(&mut socket)
        };
        let first_http = search_http("/api/knowledge/search?mode=indexed&query=source&limit=1");
        let first_http_len = http_packet(&first_http).len();
        let first_http = parse_json(
            http_packet(&first_http),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        same_search_rows(&cli_packet, &first_http);
        let mut head_socket = TcpStream::connect(address).unwrap();
        head_socket
            .set_write_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        head_socket
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        write!(head_socket, "HEAD /api/knowledge/search?mode=indexed&query=source&limit=1 HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        let head_bytes = bounded_http(&mut head_socket);
        assert!(head_bytes.starts_with(b"HTTP/1.1 200 "));
        assert!(head_bytes.ends_with(b"\r\n\r\n"));
        assert!(
            String::from_utf8_lossy(&head_bytes)
                .contains(&format!("Content-Length: {first_http_len}"))
        );
        let cursor = first_http
            .object_get("page")
            .unwrap()
            .object_get("next_cursor")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        assert!(cursor.len() <= 16 * 1024);
        let resumed = search_http(&format!(
            "/api/knowledge/search?mode=indexed&query=source&limit=1&cursor={cursor}"
        ));
        let resumed = parse_json(
            http_packet(&resumed),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        assert_eq!(
            resumed
                .object_get("page")
                .unwrap()
                .object_get("cursor")
                .unwrap()
                .as_str(),
            Some(cursor.as_str())
        );
        same_search_rows(&cli_resume, &resumed);
        let stale = search_http(&format!(
            "/api/knowledge/search?mode=indexed&query=node&limit=1&cursor={cursor}"
        ));
        assert!(stale.starts_with(b"HTTP/1.1 409 "));
        let cross_process = search_http(&format!(
            "/api/knowledge/search?mode=indexed&query=source&limit=1&cursor={search_cursor}"
        ));
        let cross_process = parse_json(
            http_packet(&cross_process),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        assert_eq!(
            cross_process
                .object_get("page")
                .unwrap()
                .object_get("cursor")
                .unwrap()
                .as_str(),
            Some(search_cursor.as_str())
        );
        same_search_rows(&cli_resume, &cross_process);
        let withdrawn_path = root.join(format!("revocations/data/{revision}.json"));
        let withdrawn_record = object(vec![
            ("schema_version", text("tos_access_release_revocation_v1")),
            ("kind", text("data")),
            ("digest", text(&revision)),
            ("reason", text("indexed fixture withdrawal")),
            ("owner_ref", text("maintained native release case")),
        ]);
        let withdrawal_lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(".release.lock"))
            .unwrap();
        withdrawal_lock.try_lock().unwrap();
        fs::write(&withdrawn_path, canonical(&withdrawn_record)).unwrap();
        withdrawal_lock.unlock().unwrap();
        let refused = search_http(&format!(
            "/api/knowledge/search?mode=indexed&query=source&limit=1&cursor={cursor}"
        ));
        assert!(refused.starts_with(b"HTTP/1.1 503 "));
        withdrawal_lock.try_lock().unwrap();
        fs::remove_file(withdrawn_path).unwrap();
        withdrawal_lock.unlock().unwrap();
        server.kill().unwrap();
        let _ = bounded_child_output(server, 16 * 1024);
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
        let result = bounded_output(child().args(["knowledge", "catalog"]), 1_048_576);
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.is_empty());
        lock.try_lock().unwrap();
        fs::remove_file(data_root.join(&paths.model)).unwrap();
        fs::rename(&retained, data_root.join(&paths.model)).unwrap();
        lock.unlock().unwrap();
        let result = bounded_output(
            Command::new("prlimit")
                .args(["--as=unlimited", "--fsize=unlimited", "--"])
                .arg(&install)
                .arg("--release-root")
                .arg(&root)
                .args(["knowledge", "catalog"])
                .env_remove("TOS_RELEASE_ROOT"),
            1_048_576,
        );
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.is_empty());
        fs::rename(
            root.join(".release.lock"),
            root.join("retained.release.lock"),
        )
        .unwrap();
        let result = bounded_output(child().args(["knowledge", "catalog"]), 1_048_576);
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
        let result = bounded_output(child().args(["knowledge", "catalog"]), 1_048_576);
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.is_empty());
        source_guard(&source_root, &source_ref, &ledger_paths);
        assert_eq!(
            bounded_sha(&selected_binary, 256 * 1024 * 1024).to_hex(),
            expected_binary_sha,
            "retained consumer ELF changed during installed case"
        );
        assert_eq!(
            bounded_sha(&producer_binary, 512 * 1024 * 1024).to_hex(),
            expected_producer_sha,
            "retained producer ELF changed during installed case"
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
            ) -> Result<PreparedPacket<'static>, AccessError> {
                unreachable!()
            }
            fn knowledge(
                &self,
                request: R,
                probe: Arc<dyn AbortProbe>,
            ) -> Result<PreparedPacket<'static>, AccessError> {
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
        let no_owner = tos_access::NoOwner;
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
        // Integrity-only install composition; this tiny ELF is never executed.
        let prefix = base.join("prefix");
        verified.install(&prefix).unwrap();
        assert_eq!(
            fs::read_link(prefix.join("bin/tos")).unwrap(),
            PathBuf::from("../software/access/src/tos_access/tos-access")
        );
        assert_eq!(
            fs::read(prefix.join("software/access/src/tos_access/tos-access")).unwrap(),
            elf
        );
        assert!(
            verified.install(&prefix).is_err(),
            "fresh prefix cannot overwrite an installation"
        );
        let occupied = base.join("occupied-prefix");
        std::os::unix::fs::symlink("missing-target", &occupied).unwrap();
        assert!(
            verified.install(&occupied).is_err(),
            "dangling link is an occupied prefix"
        );
        assert_eq!(
            fs::read_link(&occupied).unwrap(),
            PathBuf::from("missing-target")
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
        let repository = PathBuf::from(
            std::env::var_os("TOS_NATIVE_SOFTWARE_SOURCE_ROOT")
                .expect("OPS must provide the exact clean software build source"),
        )
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
        let command_products = std::env::var_os("TOS_NATIVE_SOFTWARE_COMMAND_PRODUCTS");
        let mut assembly_command = Command::new(&binary);
        assembly_command.args(["software", "build"]);
        if let Some(products) = &command_products {
            assembly_command
                .arg("--native-command-products")
                .arg(products);
        }
        let assembly = assembly_command
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
            .args(["software", "install"])
            .arg("--archive")
            .arg(&package)
            .arg("--prefix")
            .arg(&installed)
            .args(limits())
            .output()
            .unwrap();
        assert!(
            extracted.status.success(),
            "native Rust installation: {}",
            String::from_utf8_lossy(&extracted.stderr)
        );
        let overwrite = Command::new(&binary)
            .args(["software", "install"])
            .arg("--archive")
            .arg(&package)
            .arg("--prefix")
            .arg(&installed)
            .args(limits())
            .output()
            .unwrap();
        assert!(
            !overwrite.status.success(),
            "fresh installation refuses overwrite"
        );
        let program = installed.join("bin/tos");
        let image = installed.join("software/access/src/tos_access/tos-access");
        // The native closure must not bring back the retired Python runtime.
        let members = manifest.object_get("members").unwrap().as_array().unwrap();
        assert!(members.iter().all(|member| {
            let name = member.object_get("path").unwrap().as_str().unwrap();
            !name.ends_with(".py") && name != "access/pyproject.toml"
        }));
        assert_eq!(
            fs::read_link(&program).unwrap(),
            PathBuf::from("../software/access/src/tos_access/tos-access")
        );
        let installed_commands = |prefix: &Path| {
            if command_products.is_none() {
                return;
            }
            for role in [
                "tos-native-owner-command",
                "tos-schema-worker",
                "tos-validation-lanes",
                "tos-release-check",
                "tos-software-ci",
            ] {
                assert_eq!(
                    fs::read_link(prefix.join("bin").join(role)).unwrap(),
                    PathBuf::from(format!("../software/native/bin/{role}"))
                );
            }
            let commands = crate::native_child::bounded_output(
                Command::new("/usr/bin/python3")
                    .arg(repository.join("scripts/verify_rust_mechanics_install.py"))
                    .arg("--command-entries-only")
                    .arg("--installed-prefix")
                    .arg(prefix),
                65_536,
            );
            assert!(
                commands.status.success(),
                "installed ops entries: {}",
                String::from_utf8_lossy(&commands.stderr)
            );
            let owner_consumer = std::env::var_os("TOS_NATIVE_SOFTWARE_OWNER_CONSUMER_BIN")
                .expect("cohort consumer requires the admitted existing owner-text test binary");
            let owner_consumer = PathBuf::from(owner_consumer);
            let expected_sha = std::env::var("TOS_NATIVE_SOFTWARE_OWNER_CONSUMER_SHA256")
                .expect("cohort consumer requires exact admitted owner test SHA");
            let expected_size = std::env::var("TOS_NATIVE_SOFTWARE_OWNER_CONSUMER_SIZE_BYTES")
                .expect("cohort consumer requires exact admitted owner test size")
                .parse::<u64>()
                .unwrap();
            let held =
                tos_fd_open::open_absolute_regular(&owner_consumer, 512 * 1024 * 1024).unwrap();
            assert_eq!(held.metadata().unwrap().len(), expected_size);
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(held.metadata().unwrap().permissions().mode() & 0o777, 0o500);
            assert_eq!(
                crate::native_child::bounded_sha(&owner_consumer, 512 * 1024 * 1024).to_hex(),
                expected_sha
            );
            for (case, seconds) in [
                (
                    "command_owner_text_cases::native_owner_text_cli_extracts_replays_and_recovers_completed_stage",
                    240,
                ),
                (
                    "command_claim_publication_cases::maintained_agent_record_correction_whole_transaction_and_access",
                    600,
                ),
            ] {
                let owner = crate::native_child::bounded_output_until(
                    Command::new(&owner_consumer)
                        .arg(case)
                        .args(["--exact", "--test-threads=1", "--nocapture"])
                        .env(
                            "TOS_NATIVE_PREPARED_CONSUMER_BIN",
                            prefix.join("software/access/src/tos_access/tos-access"),
                        )
                        .env(
                            "TOS_NATIVE_PREPARED_CONSUMER_SHA256",
                            manifest
                                .object_get("native_access")
                                .unwrap()
                                .object_get("sha256")
                                .unwrap()
                                .as_str()
                                .unwrap(),
                        )
                        .env(
                            "TOS_NATIVE_OWNER_COMMAND_PATH",
                            prefix.join("software/native/bin/tos-native-owner-command"),
                        )
                        .env(
                            "TOS_SCHEMA_WORKER_PATH",
                            prefix.join("software/native/bin/tos-schema-worker"),
                        ),
                    65_536,
                    Duration::from_secs(seconds),
                );
                assert_eq!(
                    crate::native_child::bounded_sha(&owner_consumer, 512 * 1024 * 1024).to_hex(),
                    expected_sha
                );
                assert!(
                    owner.status.success(),
                    "installed owner/worker: {}",
                    String::from_utf8_lossy(&owner.stderr)
                );
                assert!(
                    String::from_utf8_lossy(&owner.stdout).contains("1 passed; 0 failed"),
                    "existing installed owner case must actually execute: {}",
                    String::from_utf8_lossy(&owner.stdout)
                );
            }
        };
        installed_commands(&installed);
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        for option in ["--help", "--version"] {
            let help = Command::new(&program)
                .arg(option)
                .current_dir(&outside)
                .env_clear()
                .env("PATH", "")
                .env("TOS_RELEASE_ROOT", "/missing-owner-must-not-affect-help")
                .output()
                .unwrap();
            assert!(
                help.status.success(),
                "installed software metadata without selected owner"
            );
            assert!(help.stderr.is_empty());
            assert!(
                String::from_utf8_lossy(&help.stdout).starts_with(if option == "--help" {
                    "usage: tos "
                } else {
                    "tos "
                })
            );
        }

        let usage = Command::new(&program)
            .current_dir(&outside)
            .env_clear()
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(usage.status.code(), Some(2));
        assert!(usage.stdout.is_empty());
        assert!(String::from_utf8_lossy(&usage.stderr).starts_with("usage:"));
        // The same installed bin/tos provides actual stdio MCP without Python.
        let operations = tos_access::registered_operations().unwrap();
        let catalog = operations
            .iter()
            .find(|op| op.operation_id == tos_access::KnowledgeOperation::Catalog.id())
            .unwrap();
        let mut input = mcp_input(&catalog.mcp_tool, &object(vec![]));
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
        let contracts = operations
            .iter()
            .find(|op| op.operation_id == tos_access::KnowledgeOperation::ExplorationContracts.id())
            .unwrap();
        input.extend_from_slice(format!("{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{{\"name\":\"{}\",\"arguments\":{{}}}}}}\n", contracts.mcp_tool).as_bytes());
        let mut rpc = Command::new(&program)
            .arg("mcp")
            .current_dir(&outside)
            .env_clear()
            .env("PATH", "")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        rpc.stdin.take().unwrap().write_all(&input).unwrap();
        let output = rpc.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let frames = output
            .stdout
            .split(|b| *b == b'\n')
            .filter(|f| !f.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(
            frames.len(),
            3,
            "complete initialize, tools/list and software-contract frames"
        );
        let advertised =
            parse_json(frames[1], JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        let tools = advertised
            .root()
            .object_get("result")
            .unwrap()
            .object_get("tools")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(
            !tools
                .iter()
                .any(|tool| tool.object_get("name").unwrap().as_str()
                    == Some(catalog.mcp_tool.as_str())),
            "NoOwner does not advertise selected catalog"
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.object_get("name").unwrap().as_str()
                    == Some(contracts.mcp_tool.as_str()))
        );
        let returned =
            parse_json(frames[2], JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        let returned = returned
            .root()
            .object_get("result")
            .unwrap()
            .object_get("structuredContent")
            .unwrap();
        for (field, raw) in tos_access::exploration_contracts::CONTRACTS {
            let expected =
                parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
            // The software packet canonically orders object members; the
            // packaged schema retains source order. Use the same complete
            // schema identity comparison as the existing contracts harness.
            let canonical = |value| {
                tos_foundation::canonical_bytes_v1(
                    value,
                    tos_foundation::CanonicalProfile::CorpusSnapshotV1,
                    JsonLimits::default(),
                )
                .unwrap()
            };
            assert_eq!(
                canonical(returned.object_get(field).unwrap()),
                canonical(expected.root())
            );
        }
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
                .join("software/access/src/tos_access/web_dist/assets")
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
        let moved = image.with_extension("held");
        fs::rename(&image, &moved).unwrap();
        let refused = request("GET", "/");
        assert!(refused.starts_with(b"HTTP/1.1 503 "));
        assert!(!String::from_utf8_lossy(&refused).contains("window.__TOS_GRAPH_BOOT__"));
        drop(server);
        // Restore from the retained verified package into another fresh prefix.
        // The unavailable first prefix remains untouched and inspectable.
        let restored = root.join("restored");
        let restore = Command::new(&binary)
            .args(["software", "install"])
            .arg("--archive")
            .arg(&package)
            .arg("--prefix")
            .arg(&restored)
            .args(limits())
            .output()
            .unwrap();
        assert!(
            restore.status.success(),
            "native restore: {}",
            String::from_utf8_lossy(&restore.stderr)
        );
        installed_commands(&restored);
        let restored_program = restored.join("bin/tos");
        let version = Command::new(&restored_program)
            .arg("--version")
            .current_dir(&outside)
            .env_clear()
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(version.status.success());
        assert!(version.stderr.is_empty());
        assert!(String::from_utf8_lossy(&version.stdout).starts_with("tos "));
        assert_eq!(
            crate::native_child::bounded_sha(
                &restored.join("software/access/src/tos_access/tos-access"),
                256 * 1024 * 1024
            ),
            crate::native_child::bounded_sha(&moved, 256 * 1024 * 1024)
        );
        assert!(
            !image.exists(),
            "restore must not overwrite the earlier prefix"
        );
        // Keep admitted package/installation evidence in TMPDIR for OPS custody.
    }
}

// Reuse the existing bounded native child and streamed hash custody helper.
#[path = "support/native_child.rs"]
mod native_child;

// Actual local prepared publisher and the existing transport harness; QRY owns
// compressed ranking, verification and cursor semantics in its focused tests.
mod prepared_compressed {
    use super::*;
    use tos_compiler::local_prepared::{
        PreparedRows, PublicationLimits, publish_prepared_rows_until,
    };
    use tos_foundation::emit_python_compact_json;

    fn json(raw: &[u8]) -> JsonValue {
        parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root()
    }
    struct Rows(Vec<JsonValue>);
    impl PreparedRows for Rows {
        fn visit(
            &mut self,
            kind: &str,
            sink: &mut dyn FnMut(&JsonValue) -> tos_compiler::Result<()>,
        ) -> tos_compiler::Result<()> {
            if kind == "node" {
                for row in &self.0 {
                    sink(row)?;
                }
            }
            Ok(())
        }
    }
    fn same_rows(a: &JsonValue, b: &JsonValue) {
        for field in ["schema", "nodes", "relations"] {
            assert!(
                semantic_eq(a.object_get(field).unwrap(), b.object_get(field).unwrap()),
                "{field}"
            );
        }
    }
    #[test]
    fn prepared_catalog_full_delta_cli_http_mcp_and_current_fence() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-api-prepared-catalog-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("prepared.sqlite");
        let binding_path = dir.join("binding.json");
        let header = json(format!(r#"{{"schema":"tos_knowledge_graph_v1","source_revision":"{}","normalization_binding":{{"schema":"tos_knowledge_graph_normalization_binding_v1","processor_digest":"{}","entity_registry_digest":"{}","relation_registry_digest":"{}","configuration_digest":"{}"}},"authority_boundary":{{"source_owner":"Tree-of-Sophia","is_source":false,"is_canon":false,"writes_to_tree":false}},"query_properties":[]}}"#, "a".repeat(64), "b".repeat(64), "b".repeat(64), "b".repeat(64), "b".repeat(64)).as_bytes());
        let catalog = json(format!(r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{}","lenses":[],"source_wording":"Schicksal","unknown":{{"z":false,"a":0}}}}"#, "a".repeat(64)).as_bytes());
        let publication = PublicationLimits {
            max_bytes: 4 * 1024 * 1024,
            max_mutations: 100_000,
            max_row_bytes: 4096,
            max_metadata_bytes: 65_536,
            max_changes: 16,
            max_change_bytes: 65_536,
        };
        let binding = publish_prepared_rows_until(
            &path,
            &header,
            &catalog,
            &mut Rows(vec![]),
            publication,
            std::time::Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        fs::write(
            &binding_path,
            emit_python_compact_json(&binding, JsonLimits::default()).unwrap(),
        )
        .unwrap();
        let executor = tos_access::prepared_local::PreparedLocalExecutor::open(
            path.clone(),
            binding_path.clone(),
            None,
        )
        .unwrap();
        let profile = tos_access::prepared_local::profile();
        let expected = emit_python_compact_json(&catalog, JsonLimits::default()).unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            cli::run_cli(
                &["knowledge".into(), "catalog".into()],
                &executor,
                profile,
                &mut out,
                &mut err
            ),
            0,
            "{}",
            String::from_utf8_lossy(&err)
        );
        assert_eq!(out.strip_suffix(b"\n").unwrap_or(&out), expected);
        let response = handle_get(&executor, "GET", "/api/knowledge/catalog", profile);
        assert_eq!(response.status, 200);
        assert_eq!(response.body, expected);
        drop(response);
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"tos_knowledge_catalog\",\"arguments\":{}}}\n"
        );
        let mut output = Vec::new();
        run_io(
            Cursor::new(input.as_bytes()),
            &mut output,
            &executor,
            profile,
        )
        .unwrap();
        let lines = output
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let rpc = json(lines[1]);
        let structured = rpc
            .object_get("result")
            .unwrap()
            .object_get("structuredContent")
            .unwrap();
        assert_eq!(
            emit_python_compact_json(structured, JsonLimits::default()).unwrap(),
            expected
        );
        let mut held = executor
            .knowledge(
                tos_access::KnowledgeRequest::Catalog,
                profile.deadline_probe(),
            )
            .unwrap();
        let next_catalog = json(format!(r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{}","lenses":[],"source_wording":"Schicksal successor","unknown":{{"z":false,"a":0}}}}"#, "a".repeat(64)).as_bytes());
        let next = tos_compiler::local_prepared::apply_prepared_delta_until(
            &path,
            &binding,
            &header,
            &next_catalog,
            std::iter::empty(),
            publication,
            std::time::Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(
            held.fence.recheck().unwrap_err().code,
            tos_access::AccessErrorCode::StaleSelection
        );
        drop(held);
        assert_eq!(
            handle_get(&executor, "GET", "/api/knowledge/catalog", profile).status,
            409
        );
        fs::write(
            &binding_path,
            emit_python_compact_json(&next, JsonLimits::default()).unwrap(),
        )
        .unwrap();
        let successor = tos_access::prepared_local::PreparedLocalExecutor::open(
            path.clone(),
            binding_path,
            None,
        )
        .unwrap();
        let response = handle_get(&successor, "GET", "/api/knowledge/catalog", profile);
        assert_eq!(response.status, 200);
        assert_eq!(
            response.body,
            emit_python_compact_json(&next_catalog, JsonLimits::default()).unwrap()
        );
        drop(response);
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute(
            "UPDATE edge_meta SET json_chunk='{}' WHERE key='knowledge_catalog'",
            [],
        )
        .unwrap();
        drop(db);
        assert_ne!(
            handle_get(&successor, "GET", "/api/knowledge/catalog", profile).status,
            200
        );
        drop(successor);
        drop(executor);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore = "requires OPS-protected native prepared consumer and finite admitted host profile"]
    fn prepared_compressed_native_publisher_cli_http_mcp_and_current_fence() {
        let selected_binary = PathBuf::from(
            std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
                .expect("OPS must provide the protected prepared consumer ELF"),
        );
        assert!(selected_binary.is_absolute());
        let expected_sha = std::env::var("TOS_NATIVE_PREPARED_CONSUMER_SHA256")
            .expect("OPS must provide the exact prepared consumer ELF sha256");
        let image_cap = 256 * 1024 * 1024;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("tos-api-prepared-{}-{nonce}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("prepared.sqlite");
        let binding_path = dir.join("binding.json");
        let header=json(format!(r#"{{"schema":"tos_knowledge_graph_v1","source_revision":"{}","normalization_binding":{{"schema":"tos_knowledge_graph_normalization_binding_v1","processor_digest":"{}","entity_registry_digest":"{}","relation_registry_digest":"{}","configuration_digest":"{}"}},"authority_boundary":{{"source_owner":"Tree-of-Sophia","is_source":false,"is_canon":false,"writes_to_tree":false}},"query_properties":[]}}"#,"a".repeat(64),"b".repeat(64),"b".repeat(64),"b".repeat(64),"b".repeat(64)).as_bytes());
        let catalog = json(
            format!(
                r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{}","lenses":[]}}"#,
                "a".repeat(64)
            )
            .as_bytes(),
        );
        let mut rows=Rows(vec![
            json(br#"{"id":"a","entity_id":"ea","native_id":"a-native","source_graph":"philosophy","kind_id":"concept","type_id":"concept","display":{"title":{"en":"common"}}}"#),
            json(br#"{"id":"b","entity_id":"eb","native_id":"b-native","source_graph":"philosophy","kind_id":"concept","type_id":"concept","display":{"title":{"en":"common"}}}"#),
        ]);
        let binding = publish_prepared_rows_until(
            &path,
            &header,
            &catalog,
            &mut rows,
            PublicationLimits {
                max_bytes: 4 * 1024 * 1024,
                max_mutations: 100_000,
                max_row_bytes: 4096,
                max_metadata_bytes: 65_536,
                max_changes: 16,
                max_change_bytes: 65_536,
            },
            std::time::Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        fs::write(
            &binding_path,
            emit_python_compact_json(&binding, JsonLimits::default()).unwrap(),
        )
        .unwrap();
        let executor = tos_access::prepared_local::PreparedLocalExecutor::open(
            path.clone(),
            binding_path.clone(),
            None,
        )
        .unwrap();
        let profile =
            tos_access::prepared_local::profile().with_query_timeout(Duration::from_secs(5));
        assert_eq!(profile.max_request_bytes, 65_536);
        assert_eq!(profile.max_response_bytes, 4 * 1024 * 1024);
        let cli_page = |cursor: Option<&str>| {
            let mut args = vec![
                "knowledge".into(),
                "search".into(),
                "".into(),
                "--mode".into(),
                "compressed".into(),
                "--limit".into(),
                "1".into(),
                "--sources".into(),
                "philosophy".into(),
                "--kind".into(),
                "concept".into(),
            ];
            if let Some(cursor) = cursor {
                args.extend(["--cursor".into(), cursor.into()]);
            }
            let (mut out, mut err) = (Vec::new(), Vec::new());
            assert_eq!(
                cli::run_cli(&args, &executor, profile, &mut out, &mut err),
                0,
                "{}",
                String::from_utf8_lossy(&err)
            );
            json(&out)
        };
        let first = cli_page(None);
        assert_eq!(
            first.object_get("nodes").unwrap().as_array().unwrap()[0]
                .object_get("id")
                .unwrap()
                .as_str(),
            Some("a")
        );
        let cursor = first
            .object_get("page")
            .unwrap()
            .object_get("next_cursor")
            .unwrap()
            .as_str()
            .unwrap();
        let second = cli_page(Some(cursor));
        assert_eq!(
            second.object_get("nodes").unwrap().as_array().unwrap()[0]
                .object_get("id")
                .unwrap()
                .as_str(),
            Some("b")
        );
        let response = handle_get(
            &executor,
            "GET",
            &format!(
                "/api/knowledge/search?mode=compressed&query=&sources=philosophy&kind_ids=concept&limit=1&cursor={cursor}"
            ),
            profile,
        );
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        same_rows(&second, &json(&response.body));
        drop(response);
        let input = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2025-11-25\"}}}}\n{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"tos_knowledge_search\",\"arguments\":{{\"mode\":\"compressed\",\"query\":\"\",\"sources\":[\"philosophy\"],\"kind_ids\":[\"concept\"],\"limit\":1,\"cursor\":\"{cursor}\"}}}}}}\n"
        );
        let mut output = Vec::new();
        run_io(
            Cursor::new(input.as_bytes()),
            &mut output,
            &executor,
            profile,
        )
        .unwrap();
        let lines = output
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        let rpc = json(lines[1]);
        same_rows(
            &second,
            rpc.object_get("result")
                .unwrap()
                .object_get("structuredContent")
                .unwrap(),
        );
        assert_eq!(
            handle_get(
                &executor,
                "GET",
                "/api/knowledge/search?mode=compressed&cursor=a&cursor=b",
                profile
            )
            .status,
            400
        );
        assert_eq!(
            handle_get(
                &executor,
                "GET",
                "/api/knowledge/search?mode=compressed&query=",
                profile.with_query_timeout(Duration::ZERO)
            )
            .status,
            408
        );
        // Exercise the actual executable selection, not just run_cli: explicit
        // prepared paths must override an irrelevant inherited release root.
        // OPS supplies a retained product and its exact build-owned digest.
        use std::os::unix::fs::PermissionsExt;
        let image = tos_fd_open::open_absolute_regular(&selected_binary, image_cap).unwrap();
        let retained_binary = dir.join("tos-prepared");
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&retained_binary)
            .unwrap();
        let copied = std::io::copy(&mut (&image).take(image_cap + 1), &mut destination).unwrap();
        assert!(copied <= image_cap);
        destination.sync_all().unwrap();
        drop(destination);
        drop(image);
        fs::set_permissions(&retained_binary, fs::Permissions::from_mode(0o500)).unwrap();
        assert_eq!(
            crate::native_child::bounded_sha(&retained_binary, image_cap).to_hex(),
            expected_sha
        );
        let actual = crate::native_child::bounded_output_until(
            std::process::Command::new(&retained_binary)
                .env("TOS_RELEASE_ROOT", dir.join("irrelevant-release-root"))
                .arg("--prepared-read-model")
                .arg(&path)
                .arg("--prepared-binding")
                .arg(&binding_path)
                .args([
                    "knowledge",
                    "search",
                    "",
                    "--mode",
                    "compressed",
                    "--limit",
                    "1",
                    "--sources",
                    "philosophy",
                    "--kind",
                    "concept",
                    "--cursor",
                    cursor,
                ]),
            profile.max_response_bytes + 1,
            Duration::from_secs(5),
        );
        assert!(
            actual.status.success(),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        same_rows(&second, &json(&actual.stdout));
        let request = tos_query::compressed_search::CompressedSearchRequest {
            query: "".into(),
            limit: 1,
            ..Default::default()
        };
        let mut packet = executor
            .knowledge_search_compressed(request, profile.deadline_probe())
            .unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA user_version=77").unwrap();
        drop(db);
        assert_eq!(
            packet.fence.recheck().unwrap_err().code,
            tos_access::AccessErrorCode::StaleSelection
        );
        drop(packet);
        drop(executor);
        fs::remove_dir_all(dir).unwrap();
    }
}

mod local_reading {
    use super::*;
    use tos_query::reading_search::{READING_MANIFEST_REF, reading_fixture::ReadingFixture};

    fn json(raw: &[u8]) -> JsonValue {
        parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root()
    }
    fn same_capability(a: &JsonValue, b: &JsonValue) {
        assert!(semantic_eq(a, b), "whole reading capability differs");
    }
    #[test]
    #[ignore = "requires OPS-protected reading consumer and finite admitted host profile"]
    fn reading_search_original_cli_http_mcp_and_current_fence() {
        use std::os::unix::fs::PermissionsExt;
        let binary = PathBuf::from(
            std::env::var_os("TOS_NATIVE_READING_CONSUMER_BIN")
                .expect("OPS must provide the protected reading consumer ELF"),
        );
        assert!(binary.is_absolute());
        let expected_sha = std::env::var("TOS_NATIVE_READING_CONSUMER_SHA256")
            .expect("OPS must provide the exact reading consumer ELF sha256");
        let fixture = ReadingFixture::new_shared_root();
        let executor =
            tos_access::reading::ReadingLocalExecutor::open(fixture.roots.source_root.clone())
                .unwrap();
        let profile = AccessProfile::new(65_536, 1_048_576, 65_536)
            .with_mcp_frame_budget(
                tos_access::mcp::tool_result_frame_byte_bound(1_048_576, 65_536).unwrap(),
            )
            .with_query_timeout(Duration::from_secs(5));
        let args = [
            "reading-search",
            "--query",
            "судьбы",
            "--language",
            "ru",
            "--limit",
            "1",
            "--group-by",
            "speaker,formula,speaker",
        ]
        .map(str::to_owned);
        let (mut cli_body, mut stderr) = (Vec::new(), Vec::new());
        assert_eq!(
            cli::run_cli(&args, &executor, profile, &mut cli_body, &mut stderr),
            0,
            "{}",
            String::from_utf8_lossy(&stderr)
        );
        let capability = json(&cli_body);
        assert_eq!(
            capability.object_get("available").unwrap().as_bool(),
            Some(true)
        );
        assert_eq!(
            capability.object_get("provider_ref").unwrap().as_str(),
            Some(tos_query::reading_search::READING_PROVIDER_REF)
        );
        let raw = capability.object_get("result").unwrap();
        let reference = tos_query::reading_search::execute_reading_search(
            &fixture.roots,
            &tos_query::reading_search::ReadingSoftware::embedded(),
            &fixture.request(),
            tos_query::reading_search::ReadingSearchBudget::local_default(),
            profile.deadline_probe(),
        )
        .unwrap();
        assert!(semantic_eq(raw, &json(&reference.body)));
        drop(reference);
        let response = handle_get(
            &executor,
            "GET",
            "/api/zarathustra/reading?query=%20%D1%81%D1%83%D0%B4%D1%8C%D0%B1%D1%8B%20&language=%20RU%20&limit=1&group_by=speaker,formula,speaker",
            profile,
        );
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        same_capability(&capability, &json(&response.body));
        drop(response);
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"tos_zarathustra_reading_search\",\"arguments\":{\"query\":\" судьбы \",\"language\":\" RU \",\"limit\":1,\"group_by\":[\"speaker\",\"formula\",\"speaker\"]}}}\n",
        );
        let mut output = Vec::new();
        run_io(
            Cursor::new(input.as_bytes()),
            &mut output,
            &executor,
            profile,
        )
        .unwrap();
        let lines = output
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        let rpc = json(lines[1]);
        same_capability(
            &capability,
            rpc.object_get("result")
                .unwrap()
                .object_get("structuredContent")
                .unwrap(),
        );

        // The real binary selects data only and cannot inherit software or a
        // release owner from either selected data bytes or this irrelevant env.
        let image_cap = 256 * 1024 * 1024;
        let image = tos_fd_open::open_absolute_regular(&binary, image_cap).unwrap();
        let retained = fixture.root.join("tos-reading");
        let mut dest = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&retained)
            .unwrap();
        assert!(std::io::copy(&mut (&image).take(image_cap + 1), &mut dest).unwrap() <= image_cap);
        dest.sync_all().unwrap();
        drop(dest);
        drop(image);
        fs::set_permissions(&retained, fs::Permissions::from_mode(0o500)).unwrap();
        assert_eq!(
            crate::native_child::bounded_sha(&retained, image_cap).to_hex(),
            expected_sha
        );
        let actual = crate::native_child::bounded_output_until(
            std::process::Command::new(&retained)
                .env(
                    "TOS_RELEASE_ROOT",
                    fixture.root.join("irrelevant-release-root"),
                )
                .arg("--root")
                .arg(&fixture.roots.source_root)
                .args(&args),
            profile.max_response_bytes + 1,
            Duration::from_secs(5),
        );
        assert!(
            actual.status.success(),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        same_capability(&capability, &json(&actual.stdout));
        let missing = handle_get(
            &tos_access::NoOwner,
            "GET",
            "/api/zarathustra/reading?query=x",
            profile,
        );
        assert_eq!(missing.status, 200);
        assert_eq!(
            json(&missing.body)
                .object_get("available")
                .unwrap()
                .as_bool(),
            Some(false)
        );
        drop(missing);
        let no_owner_input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"tos_zarathustra_reading_search\",\"arguments\":{\"query\":\"x\"}}}\n",
        );
        let mut no_owner_output = Vec::new();
        run_io(
            Cursor::new(no_owner_input.as_bytes()),
            &mut no_owner_output,
            &tos_access::NoOwner,
            profile,
        )
        .unwrap();
        let lines = no_owner_output
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        let discovery = json(lines[1]);
        assert!(
            discovery
                .object_get("result")
                .unwrap()
                .object_get("tools")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool.object_get("name").and_then(JsonValue::as_str)
                    == Some(tos_access::reading::MCP_TOOL))
        );
        let unavailable_rpc = json(lines[2]);
        let unavailable_capability = unavailable_rpc
            .object_get("result")
            .unwrap()
            .object_get("structuredContent")
            .unwrap();
        assert_eq!(
            unavailable_capability
                .object_get("available")
                .unwrap()
                .as_bool(),
            Some(false)
        );
        assert!(matches!(
            unavailable_capability.object_get("result"),
            Some(JsonValue::Null)
        ));
        assert_eq!(
            handle_get(
                &executor,
                "GET",
                "/api/zarathustra/reading?query=x",
                profile.with_query_timeout(Duration::ZERO)
            )
            .status,
            408
        );
        assert_eq!(
            handle_get(
                &executor,
                "GET",
                "/api/zarathustra/reading?query=x&group_by=none,other",
                profile
            )
            .status,
            400
        );
        let mut packet = executor
            .reading_search(fixture.request(), profile.deadline_probe())
            .unwrap();
        let manifest = fixture.roots.analysis_root.join(READING_MANIFEST_REF);
        let moved = manifest.with_extension("held");
        fs::rename(&manifest, &moved).unwrap();
        assert!(packet.fence.recheck().is_err());
        drop(packet);
        let unavailable = handle_get(
            &executor,
            "GET",
            "/api/zarathustra/reading?query=x",
            profile,
        );
        assert_eq!(unavailable.status, 200);
        assert_eq!(
            json(&unavailable.body)
                .object_get("available")
                .unwrap()
                .as_bool(),
            Some(false)
        );
        drop(unavailable);
        fs::rename(moved, manifest).unwrap();
        drop(executor);
    }
}

// Prepared inspect/lens transport coverage is separate from catalog/search cases.
mod prepared_inspect_lens {
    use super::*;
    use std::path::Path;
    use tos_compiler::local_prepared::{
        PreparedChange, PreparedRows, PublicationLimits, apply_prepared_delta_until,
        publish_prepared_rows_until,
    };
    use tos_foundation::emit_python_compact_json;
    fn json(raw: &[u8]) -> JsonValue {
        parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root()
    }
    fn encode(v: &JsonValue) -> Vec<u8> {
        emit_python_compact_json(v, JsonLimits::default()).unwrap()
    }
    struct Rows {
        nodes: Vec<JsonValue>,
        relations: Vec<JsonValue>,
    }
    impl PreparedRows for Rows {
        fn visit(
            &mut self,
            kind: &str,
            sink: &mut dyn FnMut(&JsonValue) -> tos_compiler::Result<()>,
        ) -> tos_compiler::Result<()> {
            for row in if kind == "node" {
                &self.nodes
            } else {
                &self.relations
            } {
                sink(row)?;
            }
            Ok(())
        }
    }
    fn node(id: &str, entity: &str, wording: &str) -> JsonValue {
        json(format!(r#"{{"id":"{id}","entity_id":"{entity}","native_id":"{id}-native","source_graph":"philosophy","kind_id":"concept","type_id":"concept","display":{{"title":{{"de":"{wording}"}}}},"properties":{{"zero":0,"false":false}},"source_refs":["ToS/philosophy/{id}.md"],"source_record":{{"payload":{{"properties":{{"source_record":{{"record_type":"node","record_id":"tos.node.{id}","record_version":1,"wording":"{wording}","zero":0,"false":false}}}}}},"digest":"{}","transform_version":"tos-knowledge-normalization-v2","field_map":{{}}}}}}"#, "c".repeat(64)).as_bytes())
    }
    fn wires(
        executor: &tos_access::prepared_local::PreparedLocalExecutor,
        profile: AccessProfile,
        args: &[String],
        path: &str,
        tool: &str,
        arguments: &str,
    ) -> JsonValue {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            cli::run_cli(args, executor, profile, &mut out, &mut err),
            0,
            "{}",
            String::from_utf8_lossy(&err)
        );
        let expected = json(&out);
        let response = handle_get(executor, "GET", path, profile);
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        assert!(semantic_eq(&expected, &json(&response.body)));
        drop(response);
        let input = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2025-11-25\"}}}}\n{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"{tool}\",\"arguments\":{arguments}}}}}\n"
        );
        let mut output = Vec::new();
        run_io(
            Cursor::new(input.as_bytes()),
            &mut output,
            executor,
            profile,
        )
        .unwrap();
        let lines = output
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        let rpc = json(lines[1]);
        assert!(semantic_eq(
            &expected,
            rpc.object_get("result")
                .unwrap()
                .object_get("structuredContent")
                .unwrap()
        ));
        expected
    }
    fn lens_wires(
        executor: &tos_access::prepared_local::PreparedLocalExecutor,
        profile: AccessProfile,
        spec_path: &Path,
        spec: &JsonValue,
    ) -> JsonValue {
        let args = vec![
            "lens".into(),
            "compile".into(),
            spec_path.to_str().unwrap().into(),
        ];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            cli::run_cli(&args, executor, profile, &mut out, &mut err),
            0,
            "{}",
            String::from_utf8_lossy(&err)
        );
        let expected = json(&out);
        let response = tos_access::http::handle_post(
            executor,
            "/api/knowledge/lenses/compile",
            &encode(spec),
            profile,
        );
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        assert!(semantic_eq(&expected, &json(&response.body)));
        drop(response);
        let input = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2025-11-25\"}}}}\n{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"tos_knowledge_lens_compile\",\"arguments\":{{\"spec\":{}}}}}}}\n",
            String::from_utf8(encode(spec)).unwrap()
        );
        let mut output = Vec::new();
        run_io(
            Cursor::new(input.as_bytes()),
            &mut output,
            executor,
            profile,
        )
        .unwrap();
        let lines = output
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        let rpc = json(lines[1]);
        assert!(semantic_eq(
            &expected,
            rpc.object_get("result")
                .unwrap()
                .object_get("structuredContent")
                .unwrap()
        ));
        expected
    }
    fn actual(binary: &Path, path: &Path, binding: &Path, args: &[String], expected: &JsonValue) {
        let out = crate::native_child::bounded_output_until(
            std::process::Command::new(binary)
                .arg("--prepared-read-model")
                .arg(path)
                .arg("--prepared-binding")
                .arg(binding)
                .args(args),
            4 * 1024 * 1024 + 1,
            Duration::from_secs(5),
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(semantic_eq(expected, &json(&out.stdout)));
    }
    #[test]
    #[ignore = "requires OPS-protected inspect/lens consumer and finite admitted host profile"]
    fn prepared_inspect_lens_full_delta_cli_http_mcp_and_current_fence() {
        use std::os::unix::fs::PermissionsExt;
        let selected = PathBuf::from(
            std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
                .expect("OPS protected native consumer required"),
        );
        assert!(selected.is_absolute());
        let sha =
            std::env::var("TOS_NATIVE_PREPARED_CONSUMER_SHA256").expect("OPS exact SHA required");
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-prepared-inspect-lens-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        let image_cap = 256 * 1024 * 1024;
        let image = tos_fd_open::open_absolute_regular(&selected, image_cap).unwrap();
        let binary = dir.join("tos-access");
        let mut dest = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&binary)
            .unwrap();
        assert!(std::io::copy(&mut (&image).take(image_cap + 1), &mut dest).unwrap() <= image_cap);
        dest.sync_all().unwrap();
        drop(dest);
        drop(image);
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o500)).unwrap();
        assert_eq!(
            crate::native_child::bounded_sha(&binary, image_cap).to_hex(),
            sha
        );
        let path = dir.join("prepared.sqlite");
        let binding_path = dir.join("binding.json");
        let header=json(format!(r#"{{"schema":"tos_knowledge_graph_v1","source_revision":"{}","normalization_binding":{{"schema":"tos_knowledge_graph_normalization_binding_v1","processor_digest":"{}","entity_registry_digest":"{}","relation_registry_digest":"{}","configuration_digest":"{}"}},"authority_boundary":{{"source_owner":"Tree-of-Sophia","is_source":false,"is_canon":false,"writes_to_tree":false}},"query_properties":[]}}"#,"a".repeat(64),"b".repeat(64),"b".repeat(64),"b".repeat(64),"b".repeat(64)).as_bytes());
        let catalog = json(
            format!(
                r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{}","lenses":[]}}"#,
                "a".repeat(64)
            )
            .as_bytes(),
        );
        let a = node("a", "shared", "Schicksal");
        let a2 = node("a2", "shared", "Schicksal zweite");
        let b = node("b", "eb", "Wiederkehr");
        let relation=json(br#"{"id":"r","native_id":"r-native","source_graph":"philosophy","from_id":"a","to_id":"b","predicate_id":"related","relation_type_id":"related","properties":{"zero":0,"false":false}}"#);
        let publication = PublicationLimits {
            max_bytes: 4 * 1024 * 1024,
            max_mutations: 100_000,
            max_row_bytes: 4096,
            max_metadata_bytes: 65_536,
            max_changes: 16,
            max_change_bytes: 65_536,
        };
        let binding = publish_prepared_rows_until(
            &path,
            &header,
            &catalog,
            &mut Rows {
                nodes: vec![a.clone(), a2.clone(), b.clone()],
                relations: vec![relation],
            },
            publication,
            std::time::Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        fs::write(&binding_path, encode(&binding)).unwrap();
        let executor = tos_access::prepared_local::PreparedLocalExecutor::open(
            path.clone(),
            binding_path.clone(),
            None,
        )
        .unwrap();
        let profile = tos_access::prepared_local::profile();
        let node_args = ["knowledge", "node", "shared", "--relation-limit", "1"].map(str::to_owned);
        let relation_args = ["knowledge", "relation", "r-native"].map(str::to_owned);
        let inspect = |exec: &tos_access::prepared_local::PreparedLocalExecutor| {
            let n = wires(
                exec,
                profile,
                &node_args,
                "/api/knowledge/nodes/shared?relation_limit=1",
                "tos_knowledge_node",
                r#"{"node_id":"shared","relation_limit":1}"#,
            );
            assert_eq!(
                n.object_get("shared_entity_id").unwrap().as_bool(),
                Some(true)
            );
            assert_eq!(
                n.object_get("matches").unwrap().as_array().unwrap().len(),
                2
            );
            assert_eq!(
                n.object_get("related_relations")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert!(
                !n.object_get("source_read_targets")
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .is_empty()
            );
            let targets = n.object_get("source_read_targets").unwrap();
            for id in ["a", "a2"] {
                let target = targets.object_get(id).unwrap();
                assert_eq!(
                    target.object_get("source_revision").unwrap().as_str(),
                    Some("a".repeat(64).as_str())
                );
                assert_eq!(
                    target
                        .object_get("target")
                        .unwrap()
                        .object_get("record_ref")
                        .unwrap()
                        .object_get("id")
                        .unwrap()
                        .as_str(),
                    Some(format!("tos.node.{id}").as_str())
                );
            }
            let r = wires(
                exec,
                profile,
                &relation_args,
                "/api/knowledge/relations/r-native",
                "tos_knowledge_relation",
                r#"{"relation_id":"r-native"}"#,
            );
            assert_eq!(
                r.object_get("endpoints").unwrap().as_array().unwrap().len(),
                2
            );
            let endpoints = r.object_get("endpoints").unwrap().as_array().unwrap();
            assert!(
                endpoints
                    .iter()
                    .any(|v| v.object_get("id").and_then(JsonValue::as_str) == Some("a"))
            );
            assert!(
                endpoints
                    .iter()
                    .any(|v| v.object_get("id").and_then(JsonValue::as_str) == Some("b"))
            );
            (n, r)
        };
        let spec=json(br#"{"schema_version":"tos_lens_spec_v1","lens_id":"prepared-transport","sources":["philosophy"]}"#);
        let spec_path = dir.join("lens.json");
        fs::write(&spec_path, encode(&spec)).unwrap();
        let lens_args = vec![
            "lens".into(),
            "compile".into(),
            spec_path.to_str().unwrap().into(),
        ];
        let first_lens = lens_wires(&executor, profile, &spec_path, &spec);
        actual(&binary, &path, &binding_path, &lens_args, &first_lens);
        let mut held_lens = executor
            .knowledge(
                tos_access::KnowledgeRequest::Lens(spec.clone()),
                profile.deadline_probe(),
            )
            .unwrap();
        let (first, relation_packet) = inspect(&executor);
        assert!(semantic_eq(
            &first.object_get("matches").unwrap().as_array().unwrap()[0],
            &a
        ));
        assert!(semantic_eq(
            &first.object_get("matches").unwrap().as_array().unwrap()[1],
            &a2
        ));
        actual(&binary, &path, &binding_path, &node_args, &first);
        actual(
            &binary,
            &path,
            &binding_path,
            &relation_args,
            &relation_packet,
        );
        let zero = handle_get(
            &executor,
            "GET",
            "/api/knowledge/nodes/shared?relation_limit=0",
            profile,
        );
        assert_eq!(zero.status, 200);
        assert!(
            json(&zero.body)
                .object_get("related_relations")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        drop(zero);
        let mut held = executor
            .knowledge(
                tos_access::KnowledgeRequest::Node {
                    node_id: "shared".into(),
                    relation_limit: 1,
                },
                profile.deadline_probe(),
            )
            .unwrap();
        let next_a = node("a", "shared", "Schicksal weiter");
        let next = apply_prepared_delta_until(
            &path,
            &binding,
            &header,
            &catalog,
            [Ok(PreparedChange {
                operation: "update".into(),
                kind: "node".into(),
                identifier: "a".into(),
                item: Some(next_a.clone()),
                source_order: Some(0),
            })],
            publication,
            std::time::Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(
            held.fence.recheck().unwrap_err().code,
            tos_access::AccessErrorCode::StaleSelection
        );
        drop(held);
        assert_eq!(
            held_lens.fence.recheck().unwrap_err().code,
            tos_access::AccessErrorCode::StaleSelection
        );
        drop(held_lens);
        assert_eq!(
            handle_get(&executor, "GET", "/api/knowledge/nodes/shared", profile).status,
            409
        );
        fs::write(&binding_path, encode(&next)).unwrap();
        let successor = tos_access::prepared_local::PreparedLocalExecutor::open(
            path.clone(),
            binding_path.clone(),
            None,
        )
        .unwrap();
        let second_lens = lens_wires(&successor, profile, &spec_path, &spec);
        actual(&binary, &path, &binding_path, &lens_args, &second_lens);
        assert!(
            !semantic_eq(&first_lens, &second_lens),
            "delta must change lens body/fingerprint"
        );
        let (second, second_relation) = inspect(&successor);
        assert!(semantic_eq(
            &second.object_get("matches").unwrap().as_array().unwrap()[0],
            &next_a
        ));
        actual(&binary, &path, &binding_path, &node_args, &second);
        actual(
            &binary,
            &path,
            &binding_path,
            &relation_args,
            &second_relation,
        );
        assert_eq!(
            handle_get(&successor, "GET", "/api/knowledge/nodes/missing", profile).status,
            404
        );
        assert_eq!(
            handle_get(
                &successor,
                "GET",
                "/api/knowledge/nodes/shared",
                profile.with_query_timeout(Duration::ZERO)
            )
            .status,
            408
        );
        drop(successor);
        drop(executor);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires OPS protected temporal/focus/stored-lens products and finite admission"]
    fn prepared_temporal_focus_stored_lens_native_rows_cli_http_mcp_and_current_fence() {
        use tos_access::KnowledgeRequest;
        use tos_foundation::JsonString;
        let selected = PathBuf::from(
            std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
                .expect("exact native CLI required"),
        );
        let expected_sha = std::env::var("TOS_NATIVE_PREPARED_CONSUMER_SHA256")
            .expect("exact native CLI SHA required");
        assert_eq!(
            crate::native_child::bounded_sha(&selected, 256 * 1024 * 1024).to_hex(),
            expected_sha
        );
        let fixture = tos_compiler::knowledge_full_fixture::build_native_fixture_bounded(
            tos_compiler::knowledge_stage::StageLimits {
                sqlite: tos_compiler::Limits {
                    max_rows: 1000,
                    max_row_bytes: 1_048_576,
                    max_output_bytes: 32 * 1024 * 1024,
                    max_work_bytes: 128 * 1024 * 1024,
                    sqlite_cache_kib: 8192,
                    max_sql_vm_steps: 100_000_000,
                },
                max_temp_bytes: 16 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 1_048_576,
            },
            std::time::Instant::now() + Duration::from_secs(30),
        );
        assert!(fixture.graph_input_bytes.len() <= 1_048_576);
        let graph = json(&fixture.graph_input_bytes);
        let nodes = graph
            .object_get("nodes")
            .unwrap()
            .as_array()
            .unwrap()
            .to_vec();
        let relations = graph
            .object_get("relations")
            .unwrap()
            .as_array()
            .unwrap()
            .to_vec();
        assert!(nodes.len() <= 200 && relations.len() <= 200);
        let claim = nodes
            .iter()
            .find(|n| {
                n.object_get("kind_id").and_then(JsonValue::as_str) == Some("claim")
                    && n.object_get("source_graph").and_then(JsonValue::as_str)
                        == Some("source-claims")
            })
            .unwrap();
        let id = claim.object_get("id").unwrap().as_str().unwrap();
        let operand = JsonValue::Object(vec![
            (
                JsonString::from_utf8("node_id"),
                claim.object_get("id").unwrap().clone(),
            ),
            (
                JsonString::from_utf8("content_revision"),
                claim.object_get("content_revision").unwrap().clone(),
            ),
        ]);
        let temporal = JsonValue::Object(vec![
            (
                JsonString::from_utf8("schema_version"),
                JsonValue::String(JsonString::from_utf8("tos_temporal_comparison_request_v1")),
            ),
            (
                JsonString::from_utf8("source_revision"),
                graph.object_get("source_revision").unwrap().clone(),
            ),
            (JsonString::from_utf8("left"), operand.clone()),
            (JsonString::from_utf8("right"), operand),
        ]);
        let mut header = graph.clone();
        let JsonValue::Object(fields) = &mut header else {
            panic!("graph header")
        };
        fields.retain(|(k, _)| !matches!(k.as_str(), Some("nodes" | "relations")));
        let JsonValue::Object(boundary) = &mut fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("authority_boundary"))
            .unwrap()
            .1
        else {
            panic!("native fixture authority boundary")
        };
        boundary.push((
            JsonString::from_utf8("source_owner"),
            JsonValue::String(JsonString::from_utf8("Tree-of-Sophia")),
        ));
        let revision = header
            .object_get("source_revision")
            .unwrap()
            .as_str()
            .unwrap();
        let spec=json(br#"{"schema_version":"tos_lens_spec_v1","lens_id":"prepared-stored","detail":"full","sources":["source-claims"],"limits":{"nodes":100,"relations":200,"groups":100}}"#);
        let catalog=json(format!(r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{revision}","lenses":[{}]}}"#,String::from_utf8(encode(&spec)).unwrap()).as_bytes());
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-prepared-operations-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("prepared.sqlite");
        let binding_path = dir.join("binding.json");
        let request_path = dir.join("temporal.json");
        let publication = PublicationLimits {
            max_bytes: 4 * 1024 * 1024,
            max_mutations: 100_000,
            max_row_bytes: 1_048_576,
            max_metadata_bytes: 1_048_576,
            max_changes: 16,
            max_change_bytes: 65_536,
        };
        let binding = publish_prepared_rows_until(
            &path,
            &header,
            &catalog,
            &mut Rows {
                nodes: nodes.clone(),
                relations,
            },
            publication,
            std::time::Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        fs::write(&binding_path, encode(&binding)).unwrap();
        fs::write(&request_path, encode(&temporal)).unwrap();
        let executor = tos_access::prepared_local::PreparedLocalExecutor::open(
            path.clone(),
            binding_path.clone(),
            None,
        )
        .unwrap();
        let profile = tos_access::prepared_local::profile();
        let args = vec![
            "knowledge".into(),
            "temporal-compare".into(),
            request_path.to_str().unwrap().into(),
        ];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            cli::run_cli(&args, &executor, profile, &mut out, &mut err),
            0,
            "{}",
            String::from_utf8_lossy(&err)
        );
        let temporal_packet = json(&out);
        assert_eq!(
            temporal_packet
                .object_get("comparison")
                .unwrap()
                .object_get("status")
                .unwrap()
                .as_str(),
            Some("comparable")
        );
        let response = tos_access::http::handle_post(
            &executor,
            "/api/knowledge/temporal/compare",
            &encode(&temporal),
            profile,
        );
        assert_eq!(response.status, 200);
        assert!(semantic_eq(&temporal_packet, &json(&response.body)));
        drop(response);
        let rpc = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2025-11-25\"}}}}\n{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"tos_knowledge_temporal_compare\",\"arguments\":{{\"request\":{}}}}}}}\n",
            String::from_utf8(encode(&temporal)).unwrap()
        );
        let mut output = Vec::new();
        run_io(Cursor::new(rpc.as_bytes()), &mut output, &executor, profile).unwrap();
        let rpc = json(
            output
                .split(|b| *b == b'\n')
                .filter(|v| !v.is_empty())
                .nth(1)
                .unwrap(),
        );
        assert!(semantic_eq(
            &temporal_packet,
            rpc.object_get("result")
                .unwrap()
                .object_get("structuredContent")
                .unwrap()
        ));
        actual(&selected, &path, &binding_path, &args, &temporal_packet);
        let focus_args = vec![
            "knowledge".into(),
            "focus".into(),
            id.to_owned(),
            "--sources".into(),
            "source-claims".into(),
        ];
        let focus = wires(
            &executor,
            profile,
            &focus_args,
            &format!("/api/knowledge/focus/{id}?sources=source-claims"),
            "tos_knowledge_focus",
            &format!(r#"{{"node_id":"{id}","sources":["source-claims"]}}"#),
        );
        assert!(
            focus
                .object_get("nodes")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n.object_get("id") == claim.object_get("id"))
        );
        actual(&selected, &path, &binding_path, &focus_args, &focus);
        let stored_args = ["lens", "open", "prepared-stored"].map(str::to_owned);
        let stored = wires(
            &executor,
            profile,
            &stored_args,
            "/api/knowledge/lenses/prepared-stored",
            "tos_knowledge_lens_open",
            r#"{"lens_id":"prepared-stored"}"#,
        );
        assert!(
            stored
                .object_get("nodes")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n.object_get("id") == claim.object_get("id"))
        );
        actual(&selected, &path, &binding_path, &stored_args, &stored);
        assert_eq!(
            handle_get(
                &executor,
                "GET",
                "/api/knowledge/lenses/unknown-lens",
                profile
            )
            .status,
            404
        );
        let mut bad = temporal.clone();
        let JsonValue::Object(fields) = bad.object_get("left").unwrap().clone() else {
            panic!("operand")
        };
        let mut left = JsonValue::Object(fields);
        let JsonValue::Object(fields) = &mut left else {
            unreachable!()
        };
        fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("content_revision"))
            .unwrap()
            .1 = JsonValue::String(JsonString::from_utf8(&"f".repeat(64)));
        let JsonValue::Object(fields) = &mut bad else {
            unreachable!()
        };
        fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("left"))
            .unwrap()
            .1 = left;
        assert_eq!(
            tos_access::http::handle_post(
                &executor,
                "/api/knowledge/temporal/compare",
                &encode(&bad),
                profile
            )
            .status,
            409
        );
        let mut held = executor
            .knowledge(
                KnowledgeRequest::Temporal(temporal.clone()),
                profile.deadline_probe(),
            )
            .unwrap();
        held.fence.recheck().unwrap();
        let mut next_header = header.clone();
        let JsonValue::Object(fields) = &mut next_header else {
            unreachable!()
        };
        fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("source_revision"))
            .unwrap()
            .1 = JsonValue::String(JsonString::from_utf8(&"d".repeat(64)));
        let next_catalog = json(
            format!(
                r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{}","lenses":[{}]}}"#,
                "d".repeat(64),
                String::from_utf8(encode(&spec)).unwrap()
            )
            .as_bytes(),
        );
        let next = apply_prepared_delta_until(
            &path,
            &binding,
            &next_header,
            &next_catalog,
            std::iter::empty::<tos_compiler::Result<tos_compiler::local_prepared::PreparedChange>>(
            ),
            publication,
            std::time::Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(
            held.fence.recheck().unwrap_err().code,
            tos_access::AccessErrorCode::StaleSelection
        );
        drop(held);
        assert_eq!(
            handle_get(
                &executor,
                "GET",
                "/api/knowledge/lenses/prepared-stored",
                profile
            )
            .status,
            409
        );
        fs::write(&binding_path, encode(&next)).unwrap();
        let successor =
            tos_access::prepared_local::PreparedLocalExecutor::open(path, binding_path, None)
                .unwrap();
        let mut successor_request = temporal.clone();
        let JsonValue::Object(fields) = &mut successor_request else {
            unreachable!()
        };
        fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("source_revision"))
            .unwrap()
            .1 = JsonValue::String(JsonString::from_utf8(&"d".repeat(64)));
        let response = tos_access::http::handle_post(
            &successor,
            "/api/knowledge/temporal/compare",
            &encode(&successor_request),
            profile,
        );
        assert_eq!(response.status, 200);
        assert_eq!(
            json(&response.body)
                .object_get("comparison")
                .unwrap()
                .object_get("status")
                .unwrap()
                .as_str(),
            Some("comparable")
        );
        drop(response);
        drop(successor);
        drop(executor);
        drop(fixture);
        fs::remove_dir_all(dir).unwrap();
    }
    include!("support/prepared_explore.rs");
}
