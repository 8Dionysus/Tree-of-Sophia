#![cfg(not(target_arch = "wasm32"))]
//! Native selected packets against frozen outputs from the maintained historical Python engine.
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
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
    registry_denied: bool,
    registry_consulted: Vec<tos_foundation::Digest256>,
    originals_denied: bool,
    original_ordinals: Vec<i64>,
    original_rights: u64,
    philosophy_rows: Vec<(tos_compiler::PhilosophyOriginalCollection, u64)>,
    philosophy_counts: Option<(u64, u64)>,
    corpus_rows: Vec<(tos_compiler::CorpusOriginalCollection, u64)>,
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
            registry_denied: true,
            registry_consulted: vec![],
            originals_denied: true,
            original_ordinals: vec![],
            original_rights: 0,
            philosophy_rows: vec![],
            philosophy_counts: None,
            corpus_rows: vec![],
        }
    }
}
impl<'hold> InspectCurrentAuthority<'hold> for Authority {
    fn authorize_corpus_original_current(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        collection: tos_compiler::CorpusOriginalCollection,
        ordinal: u64,
        raw: &[u8],
        sha: tos_foundation::Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check_selected()?;
        if self.originals_denied {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::Unavailable,
                message: "synthetic corpus original grant unavailable",
            });
        }
        assert_eq!(receipt.source_cut, self.scope.source_cut);
        assert_eq!(
            receipt.membership_root,
            self.scope.source_membership_root.to_hex()
        );
        assert_eq!(
            receipt.descriptor_sha256,
            self.scope.descriptor_sha256.to_hex()
        );
        assert_eq!(tos_foundation::Digest256::of_bytes(raw), sha);
        self.corpus_rows.push((collection, ordinal));
        Ok(())
    }
    fn authorize_philosophy_original_current(
        &mut self,
        receipt: &tos_compiler::PhilosophyOriginalReceipt,
        collection: tos_compiler::PhilosophyOriginalCollection,
        ordinal: u64,
        raw: &[u8],
        sha: tos_foundation::Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check_selected()?;
        if self.originals_denied {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::Unavailable,
                message: "synthetic philosophy original grant unavailable",
            });
        }
        assert_eq!(receipt.source_cut, self.scope.source_cut);
        assert_eq!(
            receipt.membership_root,
            self.scope.source_membership_root.to_hex()
        );
        assert_eq!(
            receipt.descriptor_sha256,
            self.scope.descriptor_sha256.to_hex()
        );
        assert_eq!(tos_foundation::Digest256::of_bytes(raw), sha);
        self.philosophy_rows.push((collection, ordinal));
        self.philosophy_counts = Some((receipt.nodes, receipt.edges));
        Ok(())
    }
    fn authorize_navigation_original_current(
        &mut self,
        receipt: &tos_compiler::NavigationOriginalReceipt,
        ordinal: i64,
        raw: &[u8],
        sha: tos_foundation::Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check_selected()?;
        if self.originals_denied {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::Unavailable,
                message: "synthetic original grant unavailable",
            });
        }
        assert_eq!(receipt.source_cut, self.scope.source_cut);
        assert_eq!(
            receipt.membership_root,
            self.scope.source_membership_root.to_hex()
        );
        assert_eq!(
            receipt.descriptor_sha256,
            self.scope.descriptor_sha256.to_hex()
        );
        assert_eq!(tos_foundation::Digest256::of_bytes(raw), sha);
        self.original_rights = receipt.rights;
        self.original_ordinals.push(ordinal);
        Ok(())
    }
    fn authorize_registry_current(
        &mut self,
        _: &str,
        raw: &[u8],
        sha: tos_foundation::Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check_selected()?;
        if self.registry_denied {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::Unavailable,
                message: "synthetic registry grant unavailable",
            });
        }
        assert_eq!(tos_foundation::Digest256::of_bytes(raw), sha);
        self.registry_consulted.push(sha);
        Ok(())
    }
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
    ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
        self.check_selected()?;
        if self.scope.operation_id == STORED_LENS_OPERATION {
            assert_eq!(self.catalog_consulted, 1);
        }
        if self.scope.operation_id == tos_query::knowledge_contracts::KNOWLEDGE_CONTRACTS_OPERATION
        {
            assert_eq!(self.registry_consulted.len(), 2);
        }
        if self.scope.operation_id == tos_query::source_dossier::DOSSIER_OPERATION
            || self.scope.operation_id == "tos.source.descend"
        {
            assert_eq!(
                self.original_ordinals,
                (-1..self.original_rights as i64).collect::<Vec<_>>()
            );
        }
        if self.scope.intended_use == tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE {
            use tos_compiler::PhilosophyOriginalCollection::{Edges, Header, Nodes};
            let (nodes, edges) = self
                .philosophy_counts
                .expect("original grants precede hold");
            let expected = std::iter::once((Header, 0))
                .chain((0..nodes).map(|i| (Nodes, i)))
                .chain((0..edges).map(|i| (Edges, i)))
                .collect::<Vec<_>>();
            assert_eq!(self.philosophy_rows, expected);
        }
        if self.scope.intended_use == tos_query::corpus_read::CORPUS_INTENDED_USE {
            assert_eq!(
                self.corpus_rows.first(),
                Some(&(tos_compiler::CorpusOriginalCollection::Header, 0))
            );
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
fn field_mut<'a>(v: &'a mut JsonValue, k: &str) -> &'a mut JsonValue {
    let JsonValue::Object(entries) = v else {
        panic!("expected object while selecting {k}");
    };
    for (name, value) in entries {
        if name.as_str() == Some(k) {
            return value;
        }
    }
    panic!("missing object field {k}");
}
fn replace_exact_string(value: &mut JsonValue, historical: &str, native: &str) {
    let JsonValue::String(actual) = value else {
        panic!("expected string availability metadata");
    };
    assert_eq!(actual.as_str(), Some(historical));
    *value = JsonValue::String(tos_foundation::JsonString::from_utf8(native));
}
fn adapt_historical_contract_availability(oracle: &mut JsonValue) {
    // The pinned Python oracle predates the native route wording. Keep the
    // source bundle exact and adapt only these eight availability statements;
    // schemas, registry data and all other contract fields still compare byte
    // for byte with the native response.
    let contracts = field_mut(oracle, "contracts");
    let api = field_mut(contracts, "api");
    let operations = field_mut(api, "operations");
    let JsonValue::Array(operations) = operations else {
        panic!("knowledge API operations must be an array");
    };
    let mut adapted = 0;
    for operation in operations {
        let operation_id = field(operation, "operation_id")
            .as_str()
            .unwrap()
            .to_owned();
        match operation_id.as_str() {
            "tos.source.read.capabilities" => {
                replace_exact_string(
                    field_mut(operation, "available_on"),
                    "Python local HTTP, CLI and native MCP; source owner is opt-in. Cloudflare/D1 advertises explicit unavailability, not a source-read capability",
                    "native local HTTP, CLI and MCP; source owner is opt-in. Cloudflare/D1 advertises explicit unavailability, not a source-read capability",
                );
                adapted += 1;
            }
            "tos.source.read.contracts" => {
                replace_exact_string(
                    field_mut(operation, "available_on"),
                    "Python local HTTP, CLI and native MCP",
                    "native local HTTP, CLI and MCP",
                );
                adapted += 1;
            }
            "tos.source.handle.discover" | "tos.source.record.read" => {
                replace_exact_string(
                    field_mut(operation, "available_on"),
                    "explicitly selected Python source-owner service; no source mutation or rights grant",
                    "explicitly selected native source-owner route; no source mutation or rights grant",
                );
                adapted += 1;
            }
            "tos.knowledge.search" => {
                let modes = field_mut(operation, "modes");
                let compressed = field_mut(modes, "compressed");
                replace_exact_string(
                    field_mut(compressed, "available_on"),
                    "explicitly selected tos_local_prepared_read_model_v1 in Python",
                    "explicitly selected tos_local_prepared_read_model_v1 in the native Rust owner",
                );
                adapted += 1;
            }
            "tos.knowledge.search.capabilities" => {
                replace_exact_string(
                    field_mut(operation, "available_on"),
                    "Python local HTTP, CLI and native MCP; Cloudflare/D1 HTTP advertises its validated indexed-search engine",
                    "native local HTTP, CLI and MCP; Cloudflare/D1 HTTP advertises its validated indexed-search engine",
                );
                adapted += 1;
            }
            "tos.knowledge.node.inspect" | "tos.knowledge.relation.inspect" => {
                let targets = field_mut(operation, "source_read_targets");
                replace_exact_string(
                    field_mut(targets, "availability"),
                    "target projection only; the owner-bound SourceReadService must revalidate it before issuing a handle",
                    "target projection only; the native owner-bound source-read route must revalidate it before issuing a handle",
                );
                adapted += 1;
            }
            _ => {}
        }
    }
    assert_eq!(
        adapted, 8,
        "only the known historical route statements may adapt"
    );
    // Registry v46 retires Python owner handles; the historical output remains
    // independently authenticated before applying these authored route changes.
    let registry = field_mut(contracts, "relation_type_registry");
    assert_eq!(field(registry, "registry_version").as_u64(), Some(45));
    *field_mut(registry, "registry_version") =
        parse_json(b"46", JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root();
    let JsonValue::Array(relations) = field_mut(registry, "relations") else {
        panic!("registry relations");
    };
    let mut moved = 0;
    for relation in relations {
        let id = field(relation, "relation_type_id").as_str().unwrap();
        let (old, new) = match id {
            "tos.relation.philosophy-historical" | "tos.relation.philosophy-evidential" => (
                "scripts/philosophy_graph_projection_common.py",
                "ToS/philosophy/trunk/relation-kinds/README.md",
            ),
            "tos.relation.projection-pressure" => (
                "scripts/philosophy_atlas_projection_common.py",
                "ToS/philosophy/atlas/README.md",
            ),
            "tos.relation.projection-structure" => (
                "scripts/philosophy_atlas_projection_common.py",
                "ToS/philosophy/graph-workbench/README.md",
            ),
            _ => continue,
        };
        replace_exact_string(field_mut(relation, "owner_ref"), old, new);
        moved += 1;
    }
    assert_eq!(moved, 4);
    let JsonValue::Array(refs) = field_mut(registry, "source_refs") else {
        panic!("registry source refs");
    };
    assert_eq!(refs.len(), 9);
    refs.retain(|v| v.as_str() != Some("scripts/philosophy_graph_projection_common.py"));
    assert_eq!(refs.len(), 8);
}

fn adapt_duplicate_inline_philosophy_oracle(oracle: &mut JsonValue) {
    let cases = field_mut(oracle, "cases");
    for (case_name, expected_rows, expected_unique) in [("view", 2, 1), ("view-full", 3, 2)] {
        let case = field_mut(cases, case_name);
        assert_eq!(
            field(case, "node_count").as_u64(),
            Some(expected_rows as u64)
        );
        let raw_ids = {
            let rows_value = field_mut(case, "nodes");
            let JsonValue::Array(rows) = rows_value else {
                panic!("historical philosophy view nodes must be an array");
            };
            assert_eq!(
                rows.len(),
                expected_rows,
                "raw {case_name} duplicate fixture"
            );
            let raw_ids = rows
                .iter()
                .map(|row| field(row, "node_id").as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            let mut unique_ids = Vec::<String>::new();
            for id in &raw_ids {
                if !unique_ids.contains(id) {
                    unique_ids.push(id.clone());
                }
            }
            assert_eq!(unique_ids.len(), expected_unique);
            let duplicate_id = unique_ids
                .iter()
                .find(|id| raw_ids.iter().filter(|raw| *raw == *id).count() == 2)
                .expect("historical duplicate-inline fixture must have one repeated node id");
            let duplicate_rows = rows
                .iter()
                .filter(|row| field(row, "node_id").as_str() == Some(duplicate_id.as_str()))
                .collect::<Vec<_>>();
            assert_eq!(duplicate_rows.len(), 2);
            assert_eq!(duplicate_rows[0], duplicate_rows[1]);
            let mut kept_rows = Vec::<String>::new();
            let mut deduplicated_rows = Vec::new();
            for row in std::mem::take(rows) {
                let id = field(&row, "node_id").as_str().unwrap().to_owned();
                if !kept_rows.contains(&id) {
                    kept_rows.push(id);
                    deduplicated_rows.push(row);
                }
            }
            assert_eq!(kept_rows, unique_ids);
            *rows = deduplicated_rows;
            raw_ids
        };
        let mut unique_ids = Vec::<String>::new();
        for id in &raw_ids {
            if !unique_ids.contains(id) {
                unique_ids.push(id.clone());
            }
        }
        assert_eq!(unique_ids.len(), expected_unique);
        {
            let view = field_mut(case, "view");
            let node_ids_value = field_mut(view, "node_ids");
            let JsonValue::Array(node_ids) = node_ids_value else {
                panic!("historical philosophy view node_ids must be an array");
            };
            let raw_view_ids = node_ids
                .iter()
                .map(|id| id.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(raw_view_ids, raw_ids);
            let mut kept_ids = Vec::<String>::new();
            let mut deduplicated_ids = Vec::new();
            for id in std::mem::take(node_ids) {
                let text = id.as_str().unwrap().to_owned();
                if !kept_ids.contains(&text) {
                    kept_ids.push(text);
                    deduplicated_ids.push(id);
                }
            }
            assert_eq!(kept_ids, unique_ids);
            *node_ids = deduplicated_ids;
        }
        *field_mut(case, "node_count") = JsonValue::Number(tos_foundation::JsonNumber {
            kind: tos_foundation::JsonNumberKind::Int,
            lexeme: expected_unique.to_string(),
        });
    }
}
fn canonical(v: &JsonValue) -> Vec<u8> {
    canonical_bytes_v1(
        v,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap()
}
const HISTORICAL_ORACLE_MANIFEST_SHA256: &str =
    "d75b5cbe97c29f0d5ac756b96e3c4530beb1380ae448ee40fc0b60234fb5eca5";
const HISTORICAL_ORACLE_PROVENANCE: &str =
    "bc98e9ac77b30738b768581a8644c4ce830785ccc2aeba24b1bd1c7197a3b662";
const HISTORICAL_ORACLE_SOURCE: &str = "db15df0d2a46a3c219a8d29fc5f101228500e9bf";
const HISTORICAL_ORACLE_TREE: &str = "553287212eca7f29c0aec8089ebfd2d7f9b3692a";
const HISTORICAL_ORACLE_PRODUCT: &str =
    "4392830f969983432e821449eb193eec13797bc6169923915a52b663c822118e";

fn historical_oracle(name: &str) -> JsonValue {
    let manifest_raw = include_bytes!("fixtures/selected_lens_oracles/manifest.json");
    assert_eq!(
        tos_foundation::Digest256::of_bytes(manifest_raw).to_hex(),
        HISTORICAL_ORACLE_MANIFEST_SHA256,
        "frozen historical capture manifest"
    );
    let manifest = parse_json(
        manifest_raw,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    assert_eq!(
        field(&manifest, "schema_version").as_str(),
        Some("tos_selected_lens_historical_oracles_v1")
    );
    assert_eq!(field(&manifest, "capture_status").as_str(), Some("PASS"));
    assert_eq!(field(&manifest, "capture_complete").as_bool(), Some(true));
    assert_eq!(
        field(&manifest, "capture_provenance_sha256").as_str(),
        Some(HISTORICAL_ORACLE_PROVENANCE)
    );
    assert_eq!(
        field(&manifest, "historical_python_commit").as_str(),
        Some("b095824a7a7728f16ce09a9c8c213c8944bce574")
    );
    assert_eq!(
        field(&manifest, "historical_python_tree").as_str(),
        Some("2a8a3faa283030101d7728cc530d69f5a3766270")
    );
    assert_eq!(
        field(&manifest, "historical_python_support_manifest_sha256").as_str(),
        Some("ff8506375c89ab5d0d62a2a1a786815d70961d6dea966c9ae1023d2eeb681a58")
    );
    assert_eq!(
        field(&manifest, "installed_access_consumer_sha256").as_str(),
        Some("7b663723a70104ad727a45ae7de7a36dd02ac2eeaa9070677425a573305e843f")
    );
    assert_eq!(
        field(&manifest, "installed_access_consumer_commit").as_str(),
        Some("32d490242fc9391d5ae3ef59871d3403443ccf37")
    );
    assert_eq!(
        field(&manifest, "installed_access_consumer_tree").as_str(),
        Some("7d97a1f123c46252a34bb98df3ce16524e57fb29")
    );
    assert_eq!(
        field(&manifest, "capture_source_commit").as_str(),
        Some(HISTORICAL_ORACLE_SOURCE)
    );
    assert_eq!(
        field(&manifest, "capture_source_tree").as_str(),
        Some(HISTORICAL_ORACLE_TREE)
    );
    assert_eq!(
        field(&manifest, "capture_source_prepost_equal").as_bool(),
        Some(true)
    );
    assert_eq!(
        field(&manifest, "native_selected_lens_product_sha256").as_str(),
        Some(HISTORICAL_ORACLE_PRODUCT)
    );
    assert_eq!(
        field(&manifest, "captured_test_functions").as_u64(),
        Some(9)
    );
    assert_eq!(
        field(&manifest, "captured_python_invocations").as_u64(),
        Some(14)
    );

    let compressed: &[u8] = match name {
        "lenses" => include_bytes!("fixtures/selected_lens_oracles/lenses.json.gz"),
        "legacy-search" => include_bytes!("fixtures/selected_lens_oracles/legacy-search.json.gz"),
        "contracts" => include_bytes!("fixtures/selected_lens_oracles/contracts.json.gz"),
        "dossiers" => include_bytes!("fixtures/selected_lens_oracles/dossiers.json.gz"),
        "shared-file-rights" => {
            include_bytes!("fixtures/selected_lens_oracles/shared-file-rights.json.gz")
        }
        "remaining-navigation" => {
            include_bytes!("fixtures/selected_lens_oracles/remaining-navigation.json.gz")
        }
        "source-gap" => include_bytes!("fixtures/selected_lens_oracles/source-gap.json.gz"),
        "corpus-reads" => include_bytes!("fixtures/selected_lens_oracles/corpus-reads.json.gz"),
        "philosophy-01" => include_bytes!("fixtures/selected_lens_oracles/philosophy-01.json.gz"),
        "philosophy-02" => include_bytes!("fixtures/selected_lens_oracles/philosophy-02.json.gz"),
        "philosophy-03" => include_bytes!("fixtures/selected_lens_oracles/philosophy-03.json.gz"),
        "philosophy-04" => include_bytes!("fixtures/selected_lens_oracles/philosophy-04.json.gz"),
        "philosophy-05" => include_bytes!("fixtures/selected_lens_oracles/philosophy-05.json.gz"),
        "philosophy-06" => include_bytes!("fixtures/selected_lens_oracles/philosophy-06.json.gz"),
        _ => panic!("unknown R4 oracle fixture {name}"),
    };
    let case = field(&manifest, "cases")
        .as_array()
        .unwrap()
        .iter()
        .find(|case| field(case, "name").as_str() == Some(name))
        .unwrap_or_else(|| panic!("R4 oracle manifest lacks {name}"));
    assert_eq!(
        tos_foundation::Digest256::of_bytes(compressed).to_hex(),
        field(case, "fixture_sha256").as_str().unwrap(),
        "compressed R4 oracle fixture hash: {name}"
    );
    assert_eq!(
        compressed.len() as u64,
        field(case, "fixture_bytes").as_u64().unwrap(),
        "compressed R4 oracle fixture size: {name}"
    );
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(compressed)
        .read_to_end(&mut decoded)
        .unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&decoded).to_hex(),
        field(case, "raw_sha256").as_str().unwrap(),
        "historical Python stdout hash: {name}"
    );
    assert_eq!(
        decoded.len() as u64,
        field(case, "raw_bytes").as_u64().unwrap(),
        "historical Python stdout size: {name}"
    );
    parse_json(
        &decoded,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 64 * 1024 * 1024,
            max_visits: 20_000_000,
            ..JsonLimits::default()
        },
    )
    .unwrap()
    .into_root()
}

const CORPUS_FIXTURE_MANIFEST_SHA256: &str =
    "018bb2d64f28817f36fc932ebc9da0b10aac7d0ed33bfd067698a0f09a7fb600";

fn verify_corpus_fixture_files(root: &Path) {
    let manifest = std::fs::read(root.join("fixture-files.sha256")).unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&manifest).to_hex(),
        CORPUS_FIXTURE_MANIFEST_SHA256,
        "captured corpus fixture manifest"
    );
    let manifest = std::str::from_utf8(&manifest).unwrap();
    let mut seen = BTreeSet::new();
    for line in manifest.lines() {
        let (expected_sha, relative) = line.split_once("  ").unwrap();
        assert_eq!(expected_sha.len(), 64);
        let relative = Path::new(relative);
        assert!(
            relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        );
        assert!(
            seen.insert(relative.to_path_buf()),
            "duplicate corpus fixture path"
        );
        let raw = std::fs::read(root.join(relative)).unwrap();
        assert_eq!(
            tos_foundation::Digest256::of_bytes(&raw).to_hex(),
            expected_sha,
            "captured corpus fixture {}",
            relative.display()
        );
    }
    assert_eq!(seen.len(), 65, "all R4 corpus fixture inputs are pinned");
}

static TEMP_FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct TestTempDir(PathBuf);
impl TestTempDir {
    fn new(prefix: &str) -> Self {
        for _ in 0..64 {
            let sequence = TEMP_FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root =
                std::env::temp_dir().join(format!("{prefix}-{}-{sequence}", std::process::id()));
            match std::fs::create_dir(&root) {
                Ok(()) => return Self(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test temporary directory: {error}"),
            }
        }
        panic!("could not allocate unique test temporary directory")
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_fixture_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = target.join(entry.file_name());
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            copy_fixture_tree(&from, &to);
        } else {
            assert!(kind.is_file(), "fixture inputs must be regular files");
            std::fs::copy(&from, &to).unwrap();
        }
    }
}

fn git_output(repository: &Path, arguments: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("run native Git fixture command {arguments:?}: {error}"));
    assert!(
        output.status.success(),
        "native Git fixture command {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn native_corpus_capture(
    source: &Path,
    prefixes: &[String],
    capture: &Path,
    restored: &Path,
) -> (String, String, String) {
    use tos_source_store::{CaptureGitRequest, CaptureRestoreLimits, GitCaptureLimits, ReadLimits};

    git_output(source, &["init", "--quiet"]);
    let mut add = Command::new("git");
    add.arg("-C").arg(source).args(["add", "--"]).args(prefixes);
    let add_output = add.output().expect("start native Git add");
    assert!(
        add_output.status.success(),
        "native Git add: {}",
        String::from_utf8_lossy(&add_output.stderr)
    );
    git_output(
        source,
        &[
            "-c",
            "user.name=ToS Software Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "selected corpus read fixture",
        ],
    );
    let commit = String::from_utf8(git_output(source, &["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .to_owned();
    let tree = String::from_utf8(git_output(source, &["rev-parse", "HEAD^{tree}"]))
        .unwrap()
        .trim()
        .to_owned();
    let deadline = Instant::now() + Duration::from_secs(120);
    let cancelled = AtomicBool::new(false);
    let capture_result = tos_source_store::capture_git(
        CaptureGitRequest {
            repository: source,
            commit: &commit,
            include_prefixes: prefixes,
            exclude_prefixes: &[],
            exclude_path_parts: &[],
            output: capture,
        },
        GitCaptureLimits {
            max_members: 512,
            max_member_bytes: 8 * 1024 * 1024,
            max_source_bytes: 8 * 1024 * 1024,
            max_metadata_bytes: 2 * 1024 * 1024,
            max_tree_bytes: 8 * 1024 * 1024,
            max_archive_bytes: 8 * 1024 * 1024,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let selection = tos_source_store::SoftwareCaptureSelectionV1 {
        source_git_commit: commit.clone(),
        source_git_tree: tree.clone(),
        capture_manifest_sha256: capture_result.manifest_sha256,
    };
    tos_source_store::restore_capture(
        capture,
        restored,
        &selection,
        CaptureRestoreLimits {
            metadata: ReadLimits {
                max_manifest_bytes: 2 * 1024 * 1024,
                max_manifest_entries: 512,
                max_selected_object_bytes: 8 * 1024 * 1024,
                json: JsonLimits {
                    max_bytes: 2 * 1024 * 1024,
                    ..JsonLimits::default()
                },
            },
            max_archive_bytes: 8 * 1024 * 1024,
            max_decoded_bytes: 8 * 1024 * 1024,
            max_source_bytes: 8 * 1024 * 1024,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    (commit, tree, capture_result.manifest_sha256.to_hex())
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
// The historical oracle's page positions and content remain frozen. Only
// publication identity changes when the selected registry revision changes.
fn rebind_historical_lens_cursors(oracle: JsonValue, publication: &JsonValue) -> JsonValue {
    fn digest(value: &serde_json::Value, bytes: &mut Vec<u8>) {
        use serde_json::Value;
        match value {
            Value::Null => bytes.extend_from_slice(b"n;"),
            Value::Bool(value) => bytes.extend_from_slice(if *value { b"b1;" } else { b"b0;" }),
            Value::Number(value) => {
                let number = value.as_f64().unwrap();
                let number = if number == 0.0 { 0.0 } else { number };
                bytes.extend_from_slice(format!("d{:016x};", number.to_bits()).as_bytes());
            }
            Value::String(value) => {
                bytes.extend_from_slice(format!("s{}:", value.len()).as_bytes());
                bytes.extend_from_slice(value.as_bytes());
            }
            Value::Array(values) => {
                bytes.extend_from_slice(format!("a{}[", values.len()).as_bytes());
                for value in values {
                    digest(value, bytes);
                }
                bytes.push(b']');
            }
            Value::Object(values) => {
                bytes.extend_from_slice(format!("o{}{{", values.len()).as_bytes());
                let sorted: std::collections::BTreeMap<_, _> = values.iter().collect();
                for (key, value) in sorted {
                    digest(&Value::String(key.clone()), bytes);
                    digest(value, bytes);
                }
                bytes.push(b'}');
            }
        }
    }
    fn cursor(fingerprint: &str, position: usize) -> String {
        const DIGITS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let raw = format!(
            "{{\"v\":1,\"fingerprint\":\"{fingerprint}\",\"n\":{position},\"r\":{position}}}"
        );
        let mut bits = 0_u32;
        let mut available = 0;
        let mut encoded = String::new();
        for byte in raw.bytes() {
            bits = (bits << 8) | u32::from(byte);
            available += 8;
            while available >= 6 {
                available -= 6;
                encoded.push(DIGITS[((bits >> available) & 63) as usize] as char);
            }
        }
        if available > 0 {
            encoded.push(DIGITS[((bits << (6 - available)) & 63) as usize] as char);
        }
        encoded
    }
    let cases = field(&oracle, "cases").as_array().unwrap();
    let first = cases
        .iter()
        .find(|case| field(case, "name").as_str() == Some("paged-context"))
        .unwrap();
    let fingerprint = field(field(first, "packet"), "fingerprint")
        .as_str()
        .unwrap();
    let publication: serde_json::Value = serde_json::from_slice(&canonical(publication)).unwrap();
    let mut bytes = Vec::new();
    digest(
        &serde_json::json!({
            "schema": "tos_published_lens_cursor_v1",
            "publication": publication,
            "fingerprint": fingerprint,
        }),
        &mut bytes,
    );
    let current = tos_foundation::Digest256::of_bytes(&bytes).to_hex();
    let mut encoded = String::from_utf8(
        canonical_bytes_v1(
            &oracle,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::new(32 * 1024 * 1024, 96, 2_000_000, 4096).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    for position in 1..=6 {
        let old = cursor(
            "f5e77c4e6a6cb49ce73d13682a95cfe8cdbc47c31c6939036541dd256fc18d70",
            position,
        );
        // One emitted page token, the subsequent request and its published
        // compiled spec. Stale and malformed-input tokens remain unchanged.
        assert_eq!(encoded.matches(&old).count(), 3);
        encoded = encoded.replace(&old, &cursor(&current, position));
    }
    parse_json(
        encoded.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::new(32 * 1024 * 1024, 96, 2_000_000, 4096).unwrap(),
    )
    .unwrap()
    .into_root()
}

#[test]
fn normalized_selected_lenses_match_frozen_historical_outputs_and_hold_current_disclosure() {
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
    let oracle = rebind_historical_lens_cursors(historical_oracle("lenses"), &publication);
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
/// the legacy search behavior; no new corpus or fixture producer.
#[test]
fn normalized_selected_legacy_search_matches_frozen_packets_and_exact_counts() {
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
    let oracle = historical_oracle("legacy-search");
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

#[test]
fn normalized_selected_contracts_require_exact_registry_carriers_and_current_hold() {
    use tos_query::knowledge_contracts::{
        KNOWLEDGE_CONTRACTS_INTENDED_USE, KNOWLEDGE_CONTRACTS_OPERATION, KnowledgeContractBudget,
        execute_selected_knowledge_contracts,
    };
    let fixture = build_native_fixture();
    let raw = fixture.registry_originals();
    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(raw[0]),
        bound.selection().entity_registry_sha256
    );
    assert_eq!(
        tos_foundation::Digest256::of_bytes(raw[1]),
        bound.selection().relation_registry_sha256
    );
    let mut oracle = historical_oracle("contracts");
    adapt_historical_contract_availability(&mut oracle);
    let mut model = cold
        .fork_reader_with_vm_budget(budget().inspect.max_read_vm_steps)
        .unwrap();
    let contract_budget = KnowledgeContractBudget {
        max_input_bytes: 4_000_000,
        max_registry_bytes: 1_000_000,
        max_response_bytes: 4_000_000,
        json: JsonLimits::default(),
    };
    let authority = |granted: bool| {
        let mut a = Authority::new(&bound);
        a.scope.operation_id = KNOWLEDGE_CONTRACTS_OPERATION.into();
        a.scope.intended_use = KNOWLEDGE_CONTRACTS_INTENDED_USE.into();
        a.registry_denied = !granted;
        a
    };
    let mut granted = authority(true);
    let mut result = execute_selected_knowledge_contracts(
        &mut model,
        &bound,
        &mut granted,
        raw,
        contract_budget,
        budget().inspect,
    )
    .unwrap();
    assert_eq!(
        canonical(
            parse_json(&result, JsonMode::PublishedStrict, JsonLimits::default())
                .unwrap()
                .root()
        ),
        canonical(&oracle)
    );
    granted.withdrawn.store(true, Ordering::SeqCst);
    assert!(matches!(result.recheck(),Err(ref e) if e.code==SearchV2ErrorCode::StalePolicy));
    let mut denied = authority(false);
    assert!(
        matches!(execute_selected_knowledge_contracts(&mut model,&bound,&mut denied,raw,contract_budget,budget().inspect),Err(ref e) if e.code==SearchV2ErrorCode::Unavailable)
    );
    let mut changed = raw[0].to_vec();
    changed.push(b' ');
    let mut a = authority(true);
    assert!(
        matches!(execute_selected_knowledge_contracts(&mut model,&bound,&mut a,[&changed,raw[1]],contract_budget,budget().inspect),Err(ref e) if e.code==SearchV2ErrorCode::StaleSelection)
    );
    assert!(a.registry_consulted.is_empty());
    let mut a = authority(true);
    assert!(
        matches!(execute_selected_knowledge_contracts(&mut model,&bound,&mut a,raw,KnowledgeContractBudget {max_registry_bytes:1,..contract_budget},budget().inspect),Err(ref e) if e.code==SearchV2ErrorCode::BudgetExceeded)
    );
}

#[test]
fn normalized_selected_dossiers_match_frozen_historical_packets_and_hold_rights() {
    use tos_compiler::knowledge_full_fixture::build_native_fixture_with_navigation_inputs;
    use tos_query::source_dossier::{
        DOSSIER_INTENDED_USE, DOSSIER_OPERATION, DossierBudget, execute_selected_dossier,
        execute_selected_source_navigation_descend, selected_source_navigation_descend_available,
    };
    // Existing frozen full navigation fixture. No shortened PR252 nodes
    // are padded to fit the real producer; original strings remain unchanged.
    let oracle = historical_oracle("dossiers");
    let raw_rows = |name| {
        field(&oracle, name)
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().as_bytes())
            .collect::<Vec<_>>()
    };
    let nodes = raw_rows("nodes");
    let edges = raw_rows("edges");
    let rights = raw_rows("rights");
    let fixture = build_native_fixture_with_navigation_inputs(
        field(&oracle, "header").as_str().unwrap().as_bytes(),
        &nodes,
        &edges,
        &rights,
    );
    let mut cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let caps = DossierBudget {
        inspect: budget().inspect,
        max_candidates: budget().max_candidates,
        max_work_steps: u64::try_from(budget().max_path_steps)
            .expect("fixture traversal budget fits u64"),
        block_size: budget().block_size,
    };
    let current = || {
        let mut a = Authority::new(&bound);
        a.scope.operation_id = DOSSIER_OPERATION.into();
        a.scope.intended_use = DOSSIER_INTENDED_USE.into();
        a.originals_denied = false;
        a
    };
    for case in field(&oracle, "cases").as_array().unwrap() {
        let object_id = field(case, "object_id").as_str().unwrap();
        let limit = field(case, "limit").as_u64().unwrap() as usize;
        let mut authority = current();
        let mut packet =
            execute_selected_dossier(&mut cold, &bound, &mut authority, object_id, limit, caps)
                .unwrap();
        assert_eq!(
            &*packet,
            canonical(field(case, "packet")),
            "{object_id}:{limit}"
        );
        authority.withdrawn.store(true, Ordering::SeqCst);
        assert_eq!(
            packet.recheck().unwrap_err().code,
            SearchV2ErrorCode::StalePolicy
        );
    }
    assert!(cold.navigation_original_available());
    assert!(selected_source_navigation_descend_available(&cold, &bound));
    for case in field(&oracle, "descends").as_array().unwrap() {
        let request = tos_query::SourceDescendRequest {
            node_id: field(case, "node_id").as_str().unwrap().to_owned(),
            max_depth: field(case, "max_depth").as_u64().unwrap() as u8,
            limit: field(case, "limit").as_u64().unwrap() as usize,
            at_least_commit_seq: None,
        };
        let descent_authority = || {
            let mut authority = current();
            authority.scope.operation_id = "tos.source.descend".into();
            authority.scope.intended_use = "read_only_public_metadata_navigation_v1".into();
            authority
        };
        let mut authority = descent_authority();
        let mut packet = execute_selected_source_navigation_descend(
            &mut cold,
            &bound,
            &mut authority,
            &request,
            caps,
            512 * 1024 * 1024,
        )
        .unwrap();
        assert_eq!(&*packet, canonical(field(case, "packet")));
        authority.withdrawn.store(true, Ordering::SeqCst);
        assert_eq!(
            packet.recheck().unwrap_err().code,
            SearchV2ErrorCode::StalePolicy
        );
        drop(packet);
        let mut denied = descent_authority();
        denied.originals_denied = true;
        assert_eq!(
            execute_selected_source_navigation_descend(
                &mut cold,
                &bound,
                &mut denied,
                &request,
                caps,
                512 * 1024 * 1024,
            )
            .err()
            .unwrap()
            .code,
            SearchV2ErrorCode::Unavailable
        );
        assert_eq!(
            execute_selected_source_navigation_descend(
                &mut cold,
                &bound,
                &mut descent_authority(),
                &request,
                caps,
                1,
            )
            .err()
            .unwrap()
            .code,
            SearchV2ErrorCode::BudgetExceeded
        );
    }
    let case = &field(&oracle, "cases").as_array().unwrap()[0];
    let object_id = field(case, "object_id").as_str().unwrap();
    let mut denied = current();
    denied.originals_denied = true;
    assert_eq!(
        execute_selected_dossier(&mut cold, &bound, &mut denied, object_id, 300, caps)
            .err()
            .unwrap()
            .code,
        SearchV2ErrorCode::Unavailable
    );
    let mut tiny = caps;
    tiny.inspect.max_rows = 1;
    assert_eq!(
        execute_selected_dossier(&mut cold, &bound, &mut current(), object_id, 300, tiny)
            .err()
            .unwrap()
            .code,
        SearchV2ErrorCode::BudgetExceeded
    );
}

#[test]
fn normalized_selected_dossiers_preserve_shared_file_membership_rights_controls() {
    use tos_compiler::knowledge_full_fixture::build_native_fixture_with_navigation_inputs_bounded;
    use tos_compiler::knowledge_stage::StageLimits;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    use tos_query::source_dossier::{
        DOSSIER_INTENDED_USE, DOSSIER_OPERATION, DossierBudget, execute_selected_dossier,
    };
    // Reuse the frozen synthetic rights fixtures and their unique assertions.
    // Emit full published test rows explicitly: the abbreviated pure-query rows
    // themselves are not claimed to satisfy the compiler input contract.
    let groups = historical_oracle("shared-file-rights");
    let caps = DossierBudget {
        inspect: budget().inspect,
        max_candidates: budget().max_candidates,
        max_work_steps: u64::try_from(budget().max_path_steps).unwrap(),
        block_size: budget().block_size,
    };
    for group in groups.as_array().unwrap() {
        let rows = |name| {
            field(group, name)
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().as_bytes())
                .collect::<Vec<_>>()
        };
        let nodes = rows("nodes");
        let edges = rows("edges");
        let rights = rows("rights");
        let fixture = build_native_fixture_with_navigation_inputs_bounded(
            field(group, "header").as_str().unwrap().as_bytes(),
            &nodes,
            &edges,
            &rights,
            StageLimits {
                sqlite: tos_compiler::Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 1024 * 1024,
            },
            deadline,
        );
        let mut cold = fixture.open().unwrap();
        let bound =
            bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
        for case in field(group, "cases").as_array().unwrap() {
            let mut current = Authority::new(&bound);
            current.scope.operation_id = DOSSIER_OPERATION.into();
            current.scope.intended_use = DOSSIER_INTENDED_USE.into();
            current.originals_denied = false;
            let mut packet = execute_selected_dossier(
                &mut cold,
                &bound,
                &mut current,
                field(case, "object_id").as_str().unwrap(),
                field(case, "limit").as_u64().unwrap() as usize,
                caps,
            )
            .unwrap();
            assert_eq!(
                &*packet,
                canonical(field(case, "packet")),
                "{}",
                field(case, "name").as_str().unwrap()
            );
            current.withdrawn.store(true, Ordering::SeqCst);
            assert_eq!(
                packet.recheck().unwrap_err().code,
                SearchV2ErrorCode::StalePolicy
            );
        }
    }
}

#[test]
fn normalized_selected_dossiers_preserve_remaining_source_navigation_boundaries() {
    use tos_compiler::knowledge_full_fixture::build_native_fixture_with_navigation_inputs_bounded;
    use tos_compiler::knowledge_stage::StageLimits;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    use tos_query::source_dossier::{
        DOSSIER_INTENDED_USE, DOSSIER_OPERATION, DossierBudget, execute_selected_dossier,
    };
    // Reuse the frozen synthetic rights fixtures and their unique assertions.
    // Emit full published test rows explicitly: the abbreviated pure-query rows
    // themselves are not claimed to satisfy the compiler input contract.
    let groups = historical_oracle("remaining-navigation");
    let caps = DossierBudget {
        inspect: budget().inspect,
        max_candidates: budget().max_candidates,
        max_work_steps: u64::try_from(budget().max_path_steps).unwrap(),
        block_size: budget().block_size,
    };
    for group in groups.as_array().unwrap() {
        let rows = |name| {
            field(group, name)
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().as_bytes())
                .collect::<Vec<_>>()
        };
        let nodes = rows("nodes");
        let edges = rows("edges");
        let rights = rows("rights");
        let fixture = build_native_fixture_with_navigation_inputs_bounded(
            field(group, "header").as_str().unwrap().as_bytes(),
            &nodes,
            &edges,
            &rights,
            StageLimits {
                sqlite: tos_compiler::Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 1024 * 1024,
            },
            deadline,
        );
        let mut cold = fixture.open().unwrap();
        let bound =
            bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
        for case in field(group, "cases").as_array().unwrap() {
            let mut current = Authority::new(&bound);
            current.scope.operation_id = DOSSIER_OPERATION.into();
            current.scope.intended_use = DOSSIER_INTENDED_USE.into();
            current.originals_denied = false;
            let mut packet = execute_selected_dossier(
                &mut cold,
                &bound,
                &mut current,
                field(case, "object_id").as_str().unwrap(),
                field(case, "limit").as_u64().unwrap() as usize,
                caps,
            )
            .unwrap();
            assert_eq!(
                &*packet,
                canonical(field(case, "packet")),
                "{}",
                field(case, "name").as_str().unwrap()
            );
            current.withdrawn.store(true, Ordering::SeqCst);
            assert_eq!(
                packet.recheck().unwrap_err().code,
                SearchV2ErrorCode::StalePolicy
            );
        }
    }
}

#[test]
fn normalized_selected_philosophy_reads_match_frozen_historical_packets_and_hold_projection() {
    use tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant;
    for variant in [
        PhilosophyFixtureViewVariant::ReferencesV2,
        PhilosophyFixtureViewVariant::InlineBothV1,
        PhilosophyFixtureViewVariant::InlineNodesV1,
        PhilosophyFixtureViewVariant::InlineEdgesV1,
        PhilosophyFixtureViewVariant::DuplicateInlineV1,
        PhilosophyFixtureViewVariant::DuplicateDanglingReferencesV1,
    ] {
        selected_philosophy_variant_parity(variant);
    }
}

fn selected_philosophy_variant_parity(
    variant: tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant,
) {
    use tos_compiler::{
        PhilosophyOriginalCollection,
        knowledge_full_fixture::build_native_fixture_with_philosophy_view_variant,
    };
    use tos_query::philosophy_read::{
        PHILOSOPHY_INTENDED_USE, PhilosophyDirection, PhilosophyReadBudget, PhilosophyReadRequest,
        execute_selected_philosophy,
    };
    // One finite software fixture goes through the normal producer, seal and
    // cold open. Its exact originals feed the maintained historical reader;
    // this is not authored philosophy, source or publication admission.
    let fixture = build_native_fixture_with_philosophy_view_variant(variant);
    let mut cold = fixture.open().unwrap();
    let receipt = cold.philosophy_original_receipt().unwrap().clone();
    let mut originals = |collection, count| {
        let mut rows = Vec::new();
        let mut after = None;
        for ordinal in 0..count {
            let page = cold
                .philosophy_original_page_under_caller_budget(
                    collection,
                    after,
                    1,
                    budget().inspect.max_payload_bytes,
                    budget().inspect.max_payload_bytes as u64,
                )
                .unwrap();
            assert_eq!(page.rows.len(), 1);
            let row = page.rows.into_iter().next().unwrap();
            assert_eq!(row.ordinal, ordinal);
            rows.push(
                parse_json(&row.raw, JsonMode::PublishedStrict, budget().inspect.json)
                    .unwrap()
                    .into_root(),
            );
            after = Some(ordinal);
        }
        rows
    };
    let header = originals(PhilosophyOriginalCollection::Header, 1).remove(0);
    let nodes = originals(PhilosophyOriginalCollection::Nodes, receipt.nodes);
    let edges = originals(PhilosophyOriginalCollection::Edges, receipt.edges);
    let mut projection = header.as_object().unwrap().to_vec();
    projection.push((
        tos_foundation::JsonString::from_utf8("nodes"),
        JsonValue::Array(nodes),
    ));
    projection.push((
        tos_foundation::JsonString::from_utf8("edges"),
        JsonValue::Array(edges),
    ));
    let projection = JsonValue::Object(projection);
    let mut oracle = historical_oracle(match variant {
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::ReferencesV2 => "philosophy-01",
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::InlineBothV1 => "philosophy-02",
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::InlineNodesV1 => "philosophy-03",
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::InlineEdgesV1 => "philosophy-04",
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::DuplicateInlineV1 => "philosophy-05",
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::DuplicateDanglingReferencesV1 => "philosophy-06",
    });
    if matches!(
        variant,
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::DuplicateInlineV1
    ) {
        // This historical fixture intentionally repeats an inline node row.
        // Prove the source duplicate exists, then compare the native bounded
        // projection against its first-occurrence, id-deduplicated disclosure.
        // The authored fixture bytes and every other output field stay intact.
        adapt_duplicate_inline_philosophy_oracle(&mut oracle);
    }
    let id = |name| field(&oracle, name).as_str().unwrap().to_owned();
    let path = |direction, from_id, to_id, excluded_edge_ids| PhilosophyReadRequest::Path {
        from_id,
        to_id,
        direction,
        excluded_edge_ids,
        max_depth: 3,
        view_id: None,
        alternative_limit: 1,
        layers: vec![],
        predicates: vec![],
    };
    let requests = vec![
        (
            "node",
            PhilosophyReadRequest::Node {
                node_id: id("left"),
            },
        ),
        (
            "edge",
            PhilosophyReadRequest::Edge {
                edge_id: id("edge"),
            },
        ),
        (
            "neighborhood",
            PhilosophyReadRequest::Neighborhood {
                node_id: id("left"),
                depth: 2,
                limit: 1,
                layers: vec![],
                predicates: vec![],
            },
        ),
        (
            "path",
            path(
                PhilosophyDirection::Outgoing,
                id("left"),
                id("right"),
                vec![],
            ),
        ),
        (
            "path-incoming",
            path(
                PhilosophyDirection::Incoming,
                id("right"),
                id("left"),
                vec![],
            ),
        ),
        (
            "path-excluded",
            path(
                PhilosophyDirection::Either,
                id("left"),
                id("right"),
                vec![id("edge")],
            ),
        ),
        (
            "view",
            PhilosophyReadRequest::View {
                view_id: id("view"),
                limit: 1,
            },
        ),
        ("views", PhilosophyReadRequest::Views),
        (
            "view-full",
            PhilosophyReadRequest::View {
                view_id: id("view"),
                limit: 1000,
            },
        ),
        (
            "search",
            PhilosophyReadRequest::Search {
                query: id("view"),
                limit: 10,
            },
        ),
        ("layers", PhilosophyReadRequest::Layers),
        (
            "clusters",
            PhilosophyReadRequest::Clusters {
                view_id: Some(id("view")),
                cluster_kind: None,
                limit: 1,
            },
        ),
        (
            "review",
            PhilosophyReadRequest::Review {
                view_id: id("view"),
            },
        ),
        ("snapshot", PhilosophyReadRequest::Snapshot),
        (
            "unresolved",
            PhilosophyReadRequest::Unresolved {
                view_id: Some(id("view")),
            },
        ),
    ];
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let caps = PhilosophyReadBudget {
        inspect: budget().inspect,
        max_work_steps: u64::try_from(budget().max_path_steps).unwrap(),
    };
    let current = |request: &PhilosophyReadRequest| {
        let mut a = Authority::new(&bound);
        a.scope.operation_id = request.operation_id().into();
        a.scope.intended_use = PHILOSOPHY_INTENDED_USE.into();
        a.originals_denied = false;
        a
    };
    let full_family = matches!(
        variant,
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::ReferencesV2
    );
    for (name, request) in &requests {
        // Existing generic reads and lease refusals run once. Compatibility
        // variants exercise exactly the view/count/search behavior that moved.
        if !full_family && !matches!(*name, "view" | "view-full" | "views" | "search") {
            continue;
        }
        let mut authority = current(request);
        let mut packet =
            execute_selected_philosophy(&mut cold, &bound, &mut authority, request, caps).unwrap();
        assert_eq!(
            &*packet,
            canonical(field(field(&oracle, "cases"), name)),
            "{variant:?}: {name}"
        );
        if *name == "view" {
            let decoded = parse_json(&packet, JsonMode::PublishedStrict, caps.inspect.json)
                .unwrap()
                .into_root();
            let view = field(&decoded, "view");
            // Nested metadata describes the original complete view; top-level
            // counts and IDs describe the limit=1 disclosure under held custody.
            assert_eq!(
                field(view, "node_count"),
                field(&decoded, "available_node_count")
            );
            assert_eq!(
                field(view, "edge_count"),
                field(&decoded, "available_edge_count")
            );
            assert!(field(view, "node_ids").as_array().unwrap().len() <= 1);
            assert!(view.object_get("nodes").is_none());
            assert!(view.object_get("edges").is_none());
        }
        packet.recheck().unwrap();
    }
    if !full_family {
        return;
    }
    let request = &requests[0].1;
    let mut denied = current(request);
    denied.originals_denied = true;
    assert_eq!(
        execute_selected_philosophy(&mut cold, &bound, &mut denied, request, caps)
            .err()
            .unwrap()
            .code,
        SearchV2ErrorCode::Unavailable
    );
    let mut tiny = caps;
    tiny.inspect.max_rows = 1;
    assert_eq!(
        execute_selected_philosophy(&mut cold, &bound, &mut current(request), request, tiny)
            .err()
            .unwrap()
            .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    let mut authority = current(request);
    let mut held =
        execute_selected_philosophy(&mut cold, &bound, &mut authority, request, caps).unwrap();
    authority.withdrawn.store(true, Ordering::SeqCst);
    assert_eq!(
        held.recheck().unwrap_err().code,
        SearchV2ErrorCode::StalePolicy
    );
}

#[test]
fn released_public_source_gap_packets_match_frozen_historical_outputs_without_source_grants() {
    use tos_query::source_gap::{
        PublicSourceGapRecord, SourceGapBudget, SourceGapRequest, compute_source_gap_packet,
    };
    struct Probe(Option<tos_query::AbortReason>);
    impl tos_query::AbortProbe for Probe {
        fn reason(&self) -> Option<tos_query::AbortReason> {
            self.0
        }
    }
    // The existing declaration selects public records. No test/provider IDs or
    // source payload paths are substituted for the owner's current allowlist.
    let oracle = historical_oracle("source-gap");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let declaration_raw =
        std::fs::read(repository.join("access/contracts/runtime-data.v1.json")).unwrap();
    let declaration = parse_json(
        &declaration_raw,
        JsonMode::PublishedStrict,
        budget().inspect.json,
    )
    .unwrap()
    .into_root();
    let subjects = field(&declaration, "subjects").as_array().unwrap();
    let mut source_refs = subjects
        .iter()
        .filter_map(|subject| {
            let source_ref = field(subject, "source_path").as_str()?;
            let roles = field(subject, "consumer_roles").as_array()?;
            let has_query = roles.iter().any(|role| role.as_str() == Some("query-core"));
            let has_http = roles
                .iter()
                .any(|role| role.as_str() == Some("http-reader"));
            (source_ref.starts_with(tos_query::source_gap::SOURCE_GAP_LEDGER_PREFIX)
                && source_ref.ends_with(tos_query::source_gap::SOURCE_GAP_RECORD_SUFFIX)
                && has_query
                && has_http)
                .then(|| source_ref.to_owned())
        })
        .collect::<Vec<_>>();
    source_refs.sort();
    assert!(
        !source_refs.is_empty(),
        "owner declaration selects public access-request records"
    );
    let historical_records = field(&oracle, "records").as_array().unwrap();
    assert_eq!(
        historical_records.len(),
        source_refs.len(),
        "R4 source owner input set is current"
    );
    let mut record_bytes = Vec::with_capacity(source_refs.len());
    for (source_ref, historical) in source_refs.iter().zip(historical_records) {
        assert_eq!(
            field(historical, "source_ref").as_str(),
            Some(source_ref.as_str())
        );
        let raw = std::fs::read(repository.join(source_ref)).unwrap();
        assert_eq!(
            raw,
            field(historical, "raw").as_str().unwrap().as_bytes(),
            "{source_ref}"
        );
        record_bytes.push(raw);
    }
    let records = source_refs
        .iter()
        .zip(record_bytes.iter())
        .map(|(source_ref, raw)| PublicSourceGapRecord { source_ref, raw })
        .collect::<Vec<_>>();
    let first = parse_json(
        &record_bytes[0],
        JsonMode::PublishedStrict,
        budget().inspect.json,
    )
    .unwrap()
    .into_root();
    let first_material = field(&first, "material");
    let expected_queries = [
        String::new(),
        field(first_material, "title").as_str().unwrap().to_owned(),
        field(&first, "request_id").as_str().unwrap().to_owned(),
        format!(
            "\u{2003}{}\u{2003}",
            field(first_material, "responsibility").as_str().unwrap()
        ),
        "\0absent\0".to_owned(),
    ];
    let cases = field(&oracle, "cases").as_array().unwrap();
    assert_eq!(cases.len(), expected_queries.len() + 1);
    for (index, case) in cases.iter().enumerate() {
        let expected_query = if index == 0 {
            &expected_queries[0]
        } else {
            &expected_queries[index - 1]
        };
        let expected_limit = if index == 0 { 1 } else { 100 };
        assert_eq!(field(case, "query").as_str(), Some(expected_query.as_str()));
        assert_eq!(field(case, "limit").as_u64(), Some(expected_limit));
    }
    let caps = SourceGapBudget {
        json: budget().inspect.json,
        max_work_steps: budget().inspect.max_read_vm_steps,
        max_response_bytes: budget().inspect.max_response_bytes,
    };
    for case in field(&oracle, "cases").as_array().unwrap() {
        let request = SourceGapRequest {
            query: field(case, "query").as_str().unwrap().into(),
            limit: usize::try_from(field(case, "limit").as_u64().unwrap()).unwrap(),
        };
        let body = compute_source_gap_packet(&records, &request, caps, &Probe(None)).unwrap();
        assert_eq!(body, canonical(field(case, "packet")));
    }
    let request = SourceGapRequest {
        query: String::new(),
        limit: 100,
    };
    assert_eq!(
        compute_source_gap_packet(
            &records,
            &request,
            caps,
            &Probe(Some(tos_query::AbortReason::Cancelled))
        )
        .unwrap_err()
        .code,
        SearchV2ErrorCode::Cancelled
    );
    let mut tiny = caps;
    tiny.max_work_steps = 1;
    assert_eq!(
        compute_source_gap_packet(&records, &request, tiny, &Probe(None))
            .unwrap_err()
            .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    let original = field(&oracle, "records").as_array().unwrap()[0]
        .object_get("raw")
        .unwrap()
        .as_str()
        .unwrap();
    let mut unsafe_record = parse_json(original.as_bytes(), JsonMode::PublishedStrict, caps.json)
        .unwrap()
        .into_root();
    let fields = if let JsonValue::Object(fields) = &mut unsafe_record {
        fields
    } else {
        unreachable!()
    };
    let flag = fields
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("personal_or_confidential_data_committed"))
        .unwrap();
    flag.1 = JsonValue::Bool(true);
    let raw = canonical(&unsafe_record);
    let unsafe_members = [PublicSourceGapRecord {
        source_ref: records[0].source_ref,
        raw: &raw,
    }];
    assert_eq!(
        compute_source_gap_packet(&unsafe_members, &request, caps, &Probe(None))
            .unwrap_err()
            .code,
        SearchV2ErrorCode::CorruptSelectedCarrier
    );
}

#[test]
fn captured_selected_corpus_reads_match_frozen_packets_and_addressed_cost() {
    use tos_compiler::knowledge_full_fixture::build_native_fixture_with_captured_corpus;
    use tos_query::corpus_read::{
        CORPUS_INTENDED_USE, CorpusReadBudget, CorpusReadContext, CorpusReadRequest,
        execute_selected_corpus,
    };
    // Reuse the maintained finite topology fixture and real Git software
    // capture/restore. Its independent capture identity is not an authored cut.
    // Duplicates and nonobjects protect original ordinal custody; the indexed
    // addressed read must fit below the complete component's row count.
    let oracle_values = historical_oracle("corpus-reads");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/selected-corpus");
    verify_corpus_fixture_files(&fixture_root);
    for archived_oracle in oracle_values.as_array().unwrap() {
        let transport = field(archived_oracle, "transport").as_str().unwrap();
        let temporary = TestTempDir::new("tos-query-selected-corpus");
        let source = temporary.path().join("source");
        copy_fixture_tree(&fixture_root.join(transport), &source);
        let source_path = field(archived_oracle, "source_path").as_str().unwrap();
        let capture = temporary.path().join("capture");
        let restored = temporary.path().join("restored");
        let prefixes = if transport == "monolithic" {
            vec!["ToS/derived-exports/tos_corpus_index.min.json".to_owned()]
        } else {
            vec![
                "ToS/derived-exports/tos_corpus_index.min.json".to_owned(),
                "ToS/derived-exports/philosophy_graph_projection.min.json".to_owned(),
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json".to_owned(),
                "ToS/doctrine/semantic-interchange/entity-types.v1.json".to_owned(),
                "ToS/doctrine/semantic-interchange/relation-types.v1.json".to_owned(),
                "ToS/derived-exports/tos_corpus_index.min.parts".to_owned(),
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.parts"
                    .to_owned(),
            ]
        };
        let (commit, tree, manifest_sha) =
            native_corpus_capture(&source, &prefixes, &capture, &restored);
        let source_bytes = std::fs::read(restored.join(source_path)).unwrap();
        let source_sha = tos_foundation::Digest256::of_bytes(&source_bytes).to_hex();
        assert_eq!(
            source_sha,
            field(archived_oracle, "source_sha").as_str().unwrap(),
            "{transport} corpus fixture bytes"
        );
        let mut oracle = archived_oracle.clone();
        let value = |text: &str| JsonValue::String(tos_foundation::JsonString::from_utf8(text));
        *field_mut(&mut oracle, "base") = value(temporary.path().to_string_lossy().as_ref());
        *field_mut(&mut oracle, "capture") = value(capture.to_string_lossy().as_ref());
        *field_mut(&mut oracle, "restored") = value(restored.to_string_lossy().as_ref());
        *field_mut(&mut oracle, "commit") = value(&commit);
        *field_mut(&mut oracle, "tree") = value(&tree);
        *field_mut(&mut oracle, "manifest_sha") = value(&manifest_sha);
        *field_mut(&mut oracle, "source_sha") = value(&source_sha);
        *field_mut(&mut oracle, "root") = value(restored.to_string_lossy().as_ref());
        *field_mut(&mut oracle, "index") =
            value(restored.join(source_path).to_string_lossy().as_ref());
        // Both the status response and the summary embed the selected filesystem
        // location. Rebase only those authenticated historical location fields.
        for summary in [false, true] {
            let cases = field_mut(&mut oracle, "cases");
            let container = if summary {
                field_mut(cases, "summary")
            } else {
                cases
            };
            let status = field_mut(container, "status");
            assert_eq!(field(status, "tos_root"), field(archived_oracle, "root"));
            assert_eq!(field(status, "index_path"), field(archived_oracle, "index"));
            *field_mut(status, "tos_root") = value(restored.to_string_lossy().as_ref());
            *field_mut(status, "index_path") =
                value(restored.join(source_path).to_string_lossy().as_ref());
        }
        let s = |name| field(&oracle, name).as_str().unwrap().to_owned();
        let mut fixture = build_native_fixture_with_captured_corpus(
            &capture,
            &restored,
            &commit,
            &tree,
            &manifest_sha,
            source_path,
        );
        let mut cold = fixture.open().unwrap();
        let receipt = cold.corpus_original_receipt().unwrap().clone();
        assert_eq!(receipt.origin.source_sha256, s("source_sha"));
        assert_eq!(receipt.origin.source_path, s("source_path"));
        for collection in std::iter::once(tos_compiler::CorpusOriginalCollection::Header)
            .chain(tos_compiler::CorpusOriginalCollection::ROWS)
        {
            let expected = field(field(&oracle, "originals"), collection.as_str())
                .as_array()
                .unwrap();
            let mut after = None;
            let mut ordinal = 0usize;
            loop {
                let page = cold
                    .corpus_original_page_under_caller_budget(
                        collection,
                        &tos_compiler::CorpusOriginalSelector::All,
                        after,
                        2,
                        budget().inspect.max_payload_bytes,
                        u64::try_from(budget().inspect.max_payload_bytes)
                            .unwrap()
                            .checked_mul(2)
                            .unwrap(),
                    )
                    .unwrap();
                for row in &page.rows {
                    assert_eq!(row.ordinal, u64::try_from(ordinal).unwrap());
                    assert_eq!(
                        row.raw_sha256,
                        tos_foundation::Digest256::of_bytes(&row.raw).to_hex()
                    );
                    let actual =
                        parse_json(&row.raw, JsonMode::PublishedStrict, budget().inspect.json)
                            .unwrap()
                            .into_root();
                    assert_eq!(
                        canonical(&actual),
                        canonical(&expected[ordinal]),
                        "{} {} ordinal {ordinal}",
                        s("transport"),
                        collection.as_str()
                    );
                    ordinal += 1;
                }
                // Advance through the final empty page as well as partial pages:
                // no retained row may be omitted or appended after logical EOF.
                if page.rows.is_empty() {
                    break;
                }
                after = page.rows.last().map(|row| row.ordinal);
            }
            assert_eq!(
                ordinal,
                expected.len(),
                "{} {} EOF",
                s("transport"),
                collection.as_str()
            );
        }
        let bound =
            bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
        let context = CorpusReadContext {
            tos_root: s("root"),
            index_path: s("index"),
        };
        let caps = CorpusReadBudget {
            inspect: budget().inspect,
            max_work_steps: u64::try_from(budget().max_path_steps).unwrap(),
        };
        let current = |request: &CorpusReadRequest| {
            let mut authority = Authority::new(&bound);
            authority.scope.operation_id = request.operation_id().into();
            authority.scope.intended_use = CORPUS_INTENDED_USE.into();
            authority.originals_denied = false;
            authority
        };
        let view = |id: &str| CorpusReadRequest::GraphView {
            view_id: id.into(),
            limit: 1,
        };
        let requests = vec![
            ("status", CorpusReadRequest::Status),
            ("summary", CorpusReadRequest::Summary),
            (
                "search",
                CorpusReadRequest::Search {
                    query: s("query"),
                    limit: 2,
                    resource_kind: None,
                },
            ),
            (
                "search-filtered",
                CorpusReadRequest::Search {
                    query: String::new(),
                    limit: 2,
                    resource_kind: field(&oracle, "kind").as_str().map(str::to_owned),
                },
            ),
            (
                "resources",
                CorpusReadRequest::Resources {
                    resource_kind: field(&oracle, "kind").as_str().map(str::to_owned),
                    owner_branch: Some(s("branch")),
                    limit: 1,
                },
            ),
            ("node", CorpusReadRequest::Node { node_id: s("node") }),
            (
                "endpoint",
                CorpusReadRequest::Node {
                    node_id: s("endpoint"),
                },
            ),
            (
                "pack",
                CorpusReadRequest::RelationPack { pack_id: s("pack") },
            ),
            ("topology", view("corpus-topology")),
            ("route", view("route-graph")),
            ("promotion", view("promotion-flow")),
            (
                "packet",
                CorpusReadRequest::Packet {
                    query: " ".into(),
                    view_id: Some("route-graph".into()),
                    limit: 1,
                },
            ),
            (
                "packet-empty",
                CorpusReadRequest::Packet {
                    query: String::new(),
                    view_id: Some(String::new()),
                    limit: 1,
                },
            ),
        ];
        let mut addressed_rows = 0;
        for (name, request) in &requests {
            let mut authority = current(request);
            let mut packet =
                execute_selected_corpus(&mut cold, &bound, &mut authority, &context, request, caps)
                    .unwrap();
            assert_eq!(
                &*packet,
                canonical(field(field(&oracle, "cases"), name)),
                "{name}"
            );
            packet.recheck().unwrap();
            if *name == "node" {
                addressed_rows = authority.corpus_rows.len();
            }
        }
        let request = CorpusReadRequest::Node { node_id: s("node") };
        let mut tiny = caps;
        if s("transport") == "monolithic" {
            assert_eq!(addressed_rows, 6);
        }
        assert!(addressed_rows > 0);
        tiny.inspect.max_rows = u64::try_from(addressed_rows).unwrap();
        assert!(receipt.collections.iter().map(|c| c.rows).sum::<u64>() > tiny.inspect.max_rows);
        let mut authority = current(&request);
        let mut held =
            execute_selected_corpus(&mut cold, &bound, &mut authority, &context, &request, tiny)
                .unwrap();
        assert_eq!(authority.corpus_rows.len(), addressed_rows);
        authority.withdrawn.store(true, Ordering::SeqCst);
        assert_eq!(
            held.recheck().unwrap_err().code,
            SearchV2ErrorCode::StalePolicy
        );
        drop(held);
        tiny.inspect.max_rows -= 1;
        assert_eq!(
            execute_selected_corpus(
                &mut cold,
                &bound,
                &mut current(&request),
                &context,
                &request,
                tiny
            )
            .err()
            .unwrap()
            .code,
            SearchV2ErrorCode::BudgetExceeded
        );
        let mut denied = current(&request);
        denied.originals_denied = true;
        assert_eq!(
            execute_selected_corpus(&mut cold, &bound, &mut denied, &context, &request, caps)
                .err()
                .unwrap()
                .code,
            SearchV2ErrorCode::Unavailable
        );
        drop(cold);
        // An outer model rehash must not make a changed addressed index key
        // trustworthy while the root-bound original packet remains unchanged.
        {
            let db = rusqlite::Connection::open(&fixture.path).unwrap();
            assert!(db.execute(
            "UPDATE corpus_original_rows SET node_id=?1 WHERE collection='nodes' AND node_id=?2",
            rusqlite::params![format!("{}!", s("node")), s("node")],
        ).unwrap() > 0);
        }
        let bytes = std::fs::read(&fixture.path).unwrap();
        fixture.expectation.model_sha256 = tos_foundation::Digest256::of_bytes(&bytes).to_hex();
        fixture.expectation.model_size_bytes = u64::try_from(bytes.len()).unwrap();
        assert!(fixture.open().is_err());
        drop(fixture);
    }
}
