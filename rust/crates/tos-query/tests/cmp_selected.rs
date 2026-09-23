#![cfg(not(target_arch = "wasm32"))]

use std::{
    fs,
    fs::File,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use tos_compiler::{
    CandidateReceipt, ImmutableModelCustody, LegacyPartitionedNavigation, Limits,
    PublicationAuthority, SelectedExpectation, SelectionFence, SourceBinding, VerifiedSelection,
    compile_navigation, open_selected_model, publish_candidate,
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
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
use tos_query::{
    AbortProbe, AbortReason, AdapterAdmissionBudget, Budget, Charged, CmpPinnedModel,
    CurrentPolicy, DisclosureLease, DisclosureScope, PinnedLocalModel, QueryError, QueryErrorCode,
    RawRecord, ReadModel, SourceDescendRequest, SourcePin, SqliteReadModel, source_descend,
};

struct FixtureLease;
impl DisclosureLease for FixtureLease {
    fn recheck(&mut self) -> Result<(), QueryError> {
        Ok(())
    }
}

struct CountedAbort {
    calls: AtomicU64,
    after: u64,
    reason: AbortReason,
}
impl AbortProbe for CountedAbort {
    fn reason(&self) -> Option<AbortReason> {
        (self.calls.fetch_add(1, Ordering::Relaxed) >= self.after).then_some(self.reason)
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
    let model = SqliteReadModel::new(pinned, FixturePolicy { denied }, scope, admission()).unwrap();
    (dir, model)
}

fn admission() -> AdapterAdmissionBudget {
    AdapterAdmissionBudget {
        max_selected_open_vm_steps: 1_000_000,
        max_metadata_vm_steps: 1_000_000,
        max_metadata_decoded_bytes: 1_000_000,
        max_metadata_rows: 64,
    }
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

fn field<'a>(value: &'a JsonValue, key: &str) -> &'a JsonValue {
    value.object_get(key).unwrap()
}

fn semantic_eq(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(name, value)| {
                    b.iter()
                        .find(|(other, _)| other == name)
                        .is_some_and(|(_, other)| semantic_eq(value, other))
                })
        }
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| semantic_eq(x, y))
        }
        _ => left == right,
    }
}

#[test]
fn selected_cmp_model_matches_frozen_python_packet_and_refuses_current_denial() {
    let oracle = parse_json(
        include_bytes!("fixtures/cmp_selected_python_oracle.json"),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    let (dir, mut model) = selected(None);
    let initialized = model.admission_charge();
    assert!(initialized.selected_open_vm_steps > 1);
    assert!(initialized.metadata_vm_steps > 1);
    assert!(initialized.metadata_decoded_bytes > 40);
    assert!(initialized.metadata_rows > 1);
    let exact_admission = AdapterAdmissionBudget {
        max_metadata_decoded_bytes: initialized.metadata_decoded_bytes,
        max_metadata_rows: initialized.metadata_rows as usize,
        ..admission()
    };
    let exact_reader = model
        .fork_reader(
            FixturePin,
            FixturePolicy { denied: None },
            model.disclosure_scope().clone(),
            exact_admission,
        )
        .unwrap();
    assert_eq!(
        exact_reader.admission_charge().metadata_decoded_bytes,
        initialized.metadata_decoded_bytes
    );
    drop(exact_reader);
    for tight in [
        AdapterAdmissionBudget {
            max_metadata_decoded_bytes: initialized.metadata_decoded_bytes - 1,
            ..admission()
        },
        AdapterAdmissionBudget {
            max_metadata_rows: initialized.metadata_rows as usize - 1,
            ..admission()
        },
        AdapterAdmissionBudget {
            max_metadata_vm_steps: 1,
            ..admission()
        },
        AdapterAdmissionBudget {
            max_selected_open_vm_steps: 1,
            ..admission()
        },
    ] {
        assert_eq!(
            model
                .fork_reader(
                    FixturePin,
                    FixturePolicy { denied: None },
                    model.disclosure_scope().clone(),
                    tight
                )
                .err()
                .unwrap()
                .code,
            QueryErrorCode::BudgetExceeded
        );
    }
    let mut wrong_layer = model.disclosure_scope().clone();
    wrong_layer.carrier_layer = "tos_item_payload_v1".into();
    let mut missing_issuer = model.disclosure_scope().clone();
    missing_issuer.policy_issuer_ref.clear();
    for scope in [wrong_layer, missing_issuer] {
        assert_eq!(
            model
                .fork_reader(
                    FixturePin,
                    FixturePolicy { denied: None },
                    scope,
                    admission()
                )
                .err()
                .unwrap()
                .code,
            QueryErrorCode::Unavailable
        );
    }
    let request = SourceDescendRequest {
        node_id: "id.alpha".into(),
        max_depth: 2,
        limit: 300,
        at_least_commit_seq: Some(7),
    };
    model.set_abort_probe(Some(Arc::new(CountedAbort {
        calls: AtomicU64::new(0),
        after: 2,
        reason: AbortReason::Cancelled,
    })));
    assert_eq!(
        model
            .exact_visible_node("id.alpha", 1_000_000, 1_000_000, 100_000, 100, 100)
            .unwrap_err()
            .code,
        QueryErrorCode::Cancelled
    );
    model.set_abort_probe(Some(Arc::new(CountedAbort {
        calls: AtomicU64::new(0),
        after: 0,
        reason: AbortReason::DeadlineExceeded,
    })));
    assert_eq!(
        source_descend(&mut model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::DeadlineExceeded
    );
    model.set_abort_probe(None);
    let exact = model
        .exact_visible_node("id.alpha", 1_000_000, 1_000_000, 100_000, 100, 100)
        .unwrap();
    let exact_bytes = exact.charged.bytes as usize + exact.record.unwrap().raw.len();
    model
        .exact_visible_node("id.alpha", exact_bytes, 1_000_000, 100_000, 100, 100)
        .unwrap();
    assert_eq!(
        model
            .exact_visible_node("id.alpha", exact_bytes - 1, 1_000_000, 100_000, 100, 100)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    for (probes, rows) in [(1, 2), (2, 1)] {
        assert_eq!(
            model
                .exact_visible_node("id.alpha", exact_bytes, 1_000_000, 100_000, probes, rows)
                .unwrap_err()
                .code,
            QueryErrorCode::BudgetExceeded
        );
    }
    // Two SQLite i64 columns and one 64-byte digest are the exact decoded
    // header width even when a selected row is hidden by the visibility rule.
    let hidden = model
        .exact_visible_node("id.packet", 2 * 8 + 64, 1, 100_000, 1, 1)
        .unwrap();
    assert!(hidden.record.is_none());
    assert_eq!(hidden.charged.bytes, 80);
    let missing = model
        .exact_visible_node("id.absent", 80, 1, 100_000, 1, 1)
        .unwrap();
    assert!(missing.record.is_none());
    assert_eq!(missing.charged.bytes, 0);
    for id in ["id.packet", "id.absent"] {
        assert_eq!(
            model
                .exact_visible_node(id, 79, 1, 100_000, 1, 1)
                .unwrap_err()
                .code,
            QueryErrorCode::BudgetExceeded
        );
    }
    let adjacency = model
        .visible_outgoing("id.alpha", None, 1, 1_000_000, 1_000_000, 100_000, 100, 100)
        .unwrap();
    let adjacency_bytes = adjacency.charged.bytes as usize
        + adjacency
            .edges
            .iter()
            .map(|edge| edge.raw.len())
            .sum::<usize>();
    model
        .visible_outgoing(
            "id.alpha",
            None,
            1,
            adjacency_bytes,
            1_000_000,
            100_000,
            100,
            100,
        )
        .unwrap();
    assert_eq!(
        model
            .visible_outgoing(
                "id.alpha",
                None,
                1,
                adjacency_bytes - 1,
                1_000_000,
                100_000,
                100,
                100
            )
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    let tight_page = model
        .visible_outgoing(
            "id.alpha",
            None,
            1,
            adjacency_bytes,
            1_000_000,
            100_000,
            4,
            5,
        )
        .unwrap();
    assert_eq!(tight_page.edges.len(), 1);
    assert!(!tight_page.exhausted);
    assert!(
        model
            .visible_outgoing("id.alpha", Some("edge.visible"), 1, 104, 1, 100_000, 4, 5,)
            .unwrap()
            .exhausted
    );
    for (probes, rows) in [(3, 5), (4, 4)] {
        assert_eq!(
            model
                .visible_outgoing(
                    "id.alpha",
                    None,
                    1,
                    adjacency_bytes,
                    1_000_000,
                    100_000,
                    probes,
                    rows
                )
                .unwrap_err()
                .code,
            QueryErrorCode::BudgetExceeded
        );
    }
    let empty_page = model
        .visible_outgoing("id.beta", None, 1, 72, 1, 100_000, 1, 1)
        .unwrap();
    assert!(empty_page.edges.is_empty());
    assert_eq!(empty_page.charged.bytes, 72);
    assert_eq!(
        model
            .visible_outgoing("id.beta", None, 1, 71, 1, 100_000, 1, 1)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    for (probes, rows) in [(0, 1), (1, 0)] {
        assert_eq!(
            model
                .visible_outgoing("id.beta", None, 1, 72, 1, 100_000, probes, rows)
                .unwrap_err()
                .code,
            QueryErrorCode::BudgetExceeded
        );
    }
    let packet = source_descend(&mut model, &request, budget()).unwrap();
    let actual = parse_json(&packet, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert!(semantic_eq(actual.root(), field(oracle.root(), "expected")));
    let mut warm_reader = model
        .fork_reader(
            FixturePin,
            FixturePolicy { denied: None },
            model.disclosure_scope().clone(),
            admission(),
        )
        .unwrap();
    let warm_packet = source_descend(&mut warm_reader, &request, budget()).unwrap();
    let warm_actual = parse_json(
        &warm_packet,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert!(semantic_eq(
        warm_actual.root(),
        field(oracle.root(), "expected")
    ));
    drop(warm_reader);
    assert_eq!(
        field(actual.root(), "authority_note").as_str(),
        Some("fixture")
    );
    let beta = SourceDescendRequest {
        node_id: "id.beta".into(),
        ..request.clone()
    };
    let empty = source_descend(&mut model, &beta, budget()).unwrap();
    let empty = parse_json(&empty, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(
        field(field(empty.root(), "counts"), "edges").as_u64(),
        Some(0)
    );
    let mut byte_tight = budget();
    byte_tight.max_bytes = 1;
    assert_eq!(
        source_descend(&mut model, &request, byte_tight)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    let mut tiny = budget();
    tiny.max_cpu_steps = 1;
    assert_eq!(
        source_descend(&mut model, &request, tiny).unwrap_err().code,
        QueryErrorCode::BudgetExceeded
    );
    drop(model);
    fs::remove_dir_all(dir).unwrap();
    let (denied_dir, mut denied_model) = selected(Some("id.beta"));
    assert_eq!(
        source_descend(&mut denied_model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::PolicyDenied
    );
    drop(denied_model);
    fs::remove_dir_all(denied_dir).unwrap();
}
