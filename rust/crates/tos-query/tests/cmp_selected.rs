#![cfg(not(target_arch = "wasm32"))]

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use tos_compiler::{
    CandidateReceipt, LegacyPartitionedNavigation, Limits, PublicationAuthority,
    SelectedExpectation, SelectionFence, SourceBinding, VerifiedSelection, compile_navigation,
    open_selected_model, publish_candidate,
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
use tos_query::{
    Budget, Charged, CmpPinnedModel, CurrentPolicy, DisclosureLease, QueryError, QueryErrorCode,
    RawRecord, ReadModel, SourceDescendRequest, SourcePin, SqliteReadModel, source_descend,
};

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
    fn authorize_current(&mut self, record: &RawRecord) -> Result<Charged, QueryError> {
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
        _: &tos_query::Binding,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease>, QueryError> {
        for record in selected {
            self.authorize_current(record)?;
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
            max_cold_open_bytes: receipt.sqlite_size_bytes,
            max_cold_open_vm_steps: 1_000_000,
        },
    )
    .unwrap();
    let pinned = CmpPinnedModel::new(verified, FixturePin).unwrap();
    let model = SqliteReadModel::new(pinned, FixturePolicy { denied }).unwrap();
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
    let request = SourceDescendRequest {
        node_id: "id.alpha".into(),
        max_depth: 2,
        limit: 300,
        at_least_commit_seq: Some(7),
    };
    let exact = model
        .exact_visible_node("id.alpha", 1_000_000, 100_000)
        .unwrap();
    let exact_bytes = exact.charged.bytes as usize + exact.record.unwrap().raw.len();
    model
        .exact_visible_node("id.alpha", exact_bytes, 100_000)
        .unwrap();
    assert_eq!(
        model
            .exact_visible_node("id.alpha", exact_bytes - 1, 100_000)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    let adjacency = model
        .visible_outgoing("id.alpha", None, 1, 1_000_000, 100_000)
        .unwrap();
    let adjacency_bytes = adjacency.charged.bytes as usize
        + adjacency
            .edges
            .iter()
            .map(|edge| edge.raw.len())
            .sum::<usize>();
    model
        .visible_outgoing("id.alpha", None, 1, adjacency_bytes, 100_000)
        .unwrap();
    assert_eq!(
        model
            .visible_outgoing("id.alpha", None, 1, adjacency_bytes - 1, 100_000)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    let packet = source_descend(&mut model, &request, budget()).unwrap();
    let actual = parse_json(&packet, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert!(semantic_eq(actual.root(), field(oracle.root(), "expected")));
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
