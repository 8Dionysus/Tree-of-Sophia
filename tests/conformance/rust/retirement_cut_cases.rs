//! Row 14 through the actual source-store cut and bounded schema worker.
//! Fixtures mirror tests/test_corpus_source_retirement.py's actual adapter
//! fixture. Source bytes are real contracts; identities/routing are explicit
//! synthetic transport claims, never a production accepted base or review.
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_source_store::{CorpusCutReader, CutReadLimits};
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::retirement_rules::{
    RetirementFamilyReport, RetirementLimits, RetirementRefusal, inspect_retirements_from_cut,
};
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};

const SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const SOURCE: &str = "ToS/source-witnesses/records/old.md";
const SURVIVING: &str = "ToS/source-witnesses/records/surviving.md";
const UNRELATED: &str = "ToS/source-witnesses/records/unrelated.md";
const EXTRA: &str = "ToS/source-witnesses/records/new-unrelated.md";
const REVIEW: &str = "ToS/review-ledger/source-retirement-review.md";
const EVENT: &str = "ToS/source-witnesses/retirements/old-source.json";
const EVENT_ID: &str = "tos.event.fixture-retirement";
const OLD_ID: &str = "tos.synthetic.source.retiring";
const SURVIVING_ID: &str = "tos.synthetic.source.surviving";
const UNRELATED_ID: &str = "tos.synthetic.source.unrelated";
const SOURCE_RAW: &[u8] = b"Original source bytes retained in history.\n";
const REVIEW_RAW: &[u8] = b"Synthetic source-owner review; no production approval.\n";

fn write_snapshot(
    root: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    base: Option<SourceRevision>,
    identities: Value,
    dependencies: Value,
    retirements: Value,
    changed_mode: Option<&str>,
) -> SourceRevision {
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("revisions")).unwrap();
    let entries: Vec<_> = files.iter().map(|(path,raw)| {
        let digest = Digest256::of_bytes(raw).to_hex();
        fs::write(root.join("objects").join(&digest), raw).unwrap();
        serde_json::json!({"path":path,"sha256":digest,"size_bytes":raw.len(),"mode":if changed_mode==Some(path.as_str()) {493} else {420}})
    }).collect();
    let mut manifest = serde_json::json!({"schema_version":"tos_corpus_snapshot_v1",
        "base_revision":base.map(|r|r.0.to_hex()), "files":entries,
        "identities":identities,"dependencies":dependencies,"retirements":retirements,
        "validator_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"});
    let revision = SourceRevision(Digest256::of_bytes(&canonical_json(&manifest)));
    manifest["revision"] = Value::String(revision.0.to_hex());
    let dir = root.join("revisions").join(revision.0.to_hex());
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("snapshot.json"), canonical_json(&manifest)).unwrap();
    revision
}

struct Fixture {
    _temporary: TempDir,
    root: PathBuf,
    base: SourceRevision,
    files: BTreeMap<String, Vec<u8>>,
    event: Value,
}
impl Fixture {
    fn new(incoming: bool, reuse_event_id: bool) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("store");
        let files = BTreeMap::from([
            (
                SCHEMA.into(),
                fs::read(fixtures().join("../../..").join(SCHEMA)).unwrap(),
            ),
            (SOURCE.into(), SOURCE_RAW.to_vec()),
            (
                SURVIVING.into(),
                b"Surviving synthetic source record.\n".to_vec(),
            ),
            (
                UNRELATED.into(),
                b"Unrelated synthetic source record.\n".to_vec(),
            ),
        ]);
        let mut identities =
            serde_json::json!({OLD_ID:SOURCE,SURVIVING_ID:SURVIVING,UNRELATED_ID:UNRELATED});
        if reuse_event_id {
            identities[EVENT_ID] = Value::String(SOURCE.into());
        }
        let dependencies = if incoming {
            serde_json::json!({SURVIVING:[SOURCE]})
        } else {
            serde_json::json!({})
        };
        let base = write_snapshot(
            &root,
            &files,
            None,
            identities,
            dependencies,
            serde_json::json!([]),
            None,
        );
        let source_digest = Digest256::of_bytes(SOURCE_RAW).to_hex();
        let review_digest = Digest256::of_bytes(REVIEW_RAW).to_hex();
        let event = serde_json::json!({
            "schema_version":"tos_provenance_event_v1","event_id":EVENT_ID,"event_type":"migration",
            "started_at":"2026-09-14T00:00:00Z","ended_at":"2026-09-14T00:01:00Z",
            "agent_refs":["model:synthetic-test"],
            "inputs":[{"ref":SOURCE,"role":"retired_source","sha256":source_digest},
                {"ref":REVIEW,"role":"source_owner_review","sha256":review_digest}],
            "outputs":[{"ref":EVENT,"role":"corpus_retirement_event"}],
            "method":{"maker_type":"model","name":"corpus-source-retirement","version":"1",
                "configuration":{"base_revision":base.0.to_hex(),
                    "retirements":[{"path":SOURCE,"sha256":source_digest}],
                    "reason":"Retain superseded fixture source in immutable history.",
                    "review_ref":REVIEW,"review_sha256":review_digest}},
            "status":"completed","event_version":1,"receipt_refs":[REVIEW]
        });
        Self {
            _temporary: temporary,
            root,
            base,
            files,
            event,
        }
    }
    fn current(
        &self,
        event_raw: Vec<u8>,
        extra: Option<(&str, Vec<u8>)>,
        mode: Option<&str>,
    ) -> SourceRevision {
        let mut files = self.files.clone();
        files.remove(SOURCE);
        files.insert(REVIEW.into(), REVIEW_RAW.to_vec());
        files.insert(EVENT.into(), event_raw.clone());
        if let Some((path, raw)) = extra {
            files.insert(path.into(), raw);
        }
        write_snapshot(
            &self.root,
            &files,
            Some(self.base),
            serde_json::json!({SURVIVING_ID:SURVIVING,UNRELATED_ID:UNRELATED,EVENT_ID:EVENT}),
            serde_json::json!({EVENT:[REVIEW]}),
            serde_json::json!([{"path":SOURCE,"sha256":Digest256::of_bytes(SOURCE_RAW).to_hex(),
                "event_ref":EVENT,"event_sha256":Digest256::of_bytes(&event_raw).to_hex(),"event_size_bytes":event_raw.len()}]),
            mode,
        )
    }
    fn cut(
        &self,
        current: SourceRevision,
        cancelled: &AtomicBool,
        deadline: Instant,
    ) -> CorpusCutReader {
        let reader = CorpusReader::open_existing(
            &self.root,
            ReadLimits {
                max_manifest_bytes: 1_048_576,
                max_manifest_entries: 512,
                max_selected_object_bytes: 2_097_152,
                json: JsonLimits::default(),
            },
        )
        .unwrap();
        reader
            .open_source_cut(
                current,
                CutReadLimits {
                    max_revisions: 4,
                    max_members: 512,
                    max_total_bytes: 16_777_216,
                    max_member_bytes: 2_097_152,
                },
                deadline,
                cancelled,
            )
            .unwrap()
    }
}
fn limits(deadline: Instant) -> RetirementLimits {
    RetirementLimits {
        max_member_bytes: 1_048_576,
        max_total_bytes: 8_388_608,
        max_state_bytes: 1_048_576,
        max_entries: 512,
        deadline,
    }
}
fn worker(
    cut: &CorpusCutReader,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> (CutWorkerSchemaExecutor, Digest256) {
    let path = super::validation_cut_cases::selected_worker_path();
    assert!(path.is_absolute());
    let hash_started = Instant::now();
    let worker_raw = fs::read(&path).unwrap();
    let worker_bytes = worker_raw.len();
    let digest = Digest256::of_bytes(&worker_raw);
    drop(worker_raw);
    let hash_elapsed = hash_started.elapsed();
    let preparation_started = Instant::now();
    let prepared = CutWorkerSchemaExecutor::from_cut(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            absolute_path: path,
            sha256: digest,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 32,
            max_receipt_bytes: 32_768,
        },
        deadline,
        cancelled,
    );
    eprintln!(
        "schema worker fixture phase=prepare family=retirement image_bytes={} expected_hash_ms={} preparation_ms={} wall_ms={} result={:?}",
        worker_bytes,
        hash_elapsed.as_millis(),
        preparation_started.elapsed().as_millis(),
        ExecutorBudget::laboratory().execution_wall.as_millis(),
        prepared.as_ref().map(|_| ())
    );
    (prepared.unwrap(), digest)
}
fn inspect(
    fixture: &Fixture,
    revision: SourceRevision,
) -> Result<RetirementFamilyReport, RetirementRefusal> {
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let cut = fixture.cut(revision, &cancelled, deadline);
    let (mut schemas, _) = worker(&cut, &cancelled, deadline);
    inspect_retirements_from_cut(&cut, limits(deadline), &cancelled, &mut schemas)
}

#[test]
fn retirement_cut_worker_carries_exact_transition_without_unrelated_raw_reads() {
    let fixture = Fixture::new(false, false);
    let event_raw = canonical_json(&fixture.event);
    let revision = fixture.current(event_raw.clone(), None, None);
    // Existing Python actual-route oracle corrupts an unrelated object: this
    // narrow operation compares exact survivor metadata without rereading it.
    let unrelated = Digest256::of_bytes(&fixture.files[UNRELATED]).to_hex();
    fs::write(
        fixture.root.join("objects").join(unrelated),
        b"corrupt unrelated object\n",
    )
    .unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let cut = fixture.cut(revision, &cancelled, deadline);
    let (mut schemas, worker_digest) = worker(&cut, &cancelled, deadline);
    let report =
        inspect_retirements_from_cut(&cut, limits(deadline), &cancelled, &mut schemas).unwrap();
    assert_eq!(report.revision, revision);
    assert_eq!(report.base_revision, Some(fixture.base));
    assert_eq!(
        report.schema_sha256,
        Some(Digest256::of_bytes(&fixture.files[SCHEMA]))
    );
    assert_eq!(
        report.event_identities,
        BTreeMap::from([(EVENT_ID.into(), EVENT.into())])
    );
    let index = report.membership_transition.unwrap();
    assert!(!index.identities.contains_key(OLD_ID));
    assert_eq!(index.identities[SURVIVING_ID], SURVIVING);
    assert_eq!(index.identities[UNRELATED_ID], UNRELATED);
    assert_eq!(index.identities[EVENT_ID], EVENT);
    assert_eq!(
        index.dependencies,
        BTreeMap::from([(EVENT.into(), vec![REVIEW.into()])])
    );
    assert!(!report.source_admission_complete);
    let retained = cut
        .read_retirement(revision, 0, 1_048_576, deadline, &cancelled)
        .unwrap();
    assert_eq!(retained.raw, SOURCE_RAW);
    assert_eq!(retained.event_raw, event_raw);
    assert!(
        cut.current()
            .member(&RelativePath::parse(SOURCE).unwrap())
            .is_none()
    );
    let receipt = schemas
        .receipts()
        .first()
        .expect("actual schema execution receipt");
    assert_eq!(receipt.path, EVENT);
    assert_eq!(receipt.contract, SCHEMA);
    assert_eq!(receipt.source_revision, revision);
    assert_eq!(receipt.source_raw_sha256, Digest256::of_bytes(&event_raw));
    assert_eq!(receipt.execution.worker_sha256, worker_digest);
    assert!(receipt.valid);
    assert!(
        cut.read_member(
            revision,
            &RelativePath::parse(UNRELATED).unwrap(),
            1_048_576,
            deadline,
            &cancelled
        )
        .is_err()
    );
}

#[test]
fn retirement_cut_worker_rejects_python_source_semantic_negative_controls() {
    for case in [
        "ordinary event",
        "failed status",
        "wrong base",
        "wrong targets",
        "empty reason",
        "wrong review digest",
        "missing review",
        "outside review owner",
        "unbound inputs",
        "wrong output",
        "no actor",
        "reversed chronology",
        "configuration extra",
    ] {
        let mut fixture = Fixture::new(false, false);
        match case {
            "ordinary event" => fixture.event["event_type"] = serde_json::json!("annotation"),
            "failed status" => fixture.event["status"] = serde_json::json!("failed"),
            "wrong base" => {
                fixture.event["method"]["configuration"]["base_revision"] =
                    serde_json::json!("b".repeat(64))
            }
            "wrong targets" => {
                fixture.event["method"]["configuration"]["retirements"][0]["sha256"] =
                    serde_json::json!("b".repeat(64))
            }
            "empty reason" => {
                fixture.event["method"]["configuration"]["reason"] = serde_json::json!("  ")
            }
            "wrong review digest" => {
                fixture.event["method"]["configuration"]["review_sha256"] =
                    serde_json::json!("b".repeat(64))
            }
            "missing review" => {
                fixture.event["method"]["configuration"]["review_ref"] =
                    serde_json::json!("ToS/review-ledger/absent.md")
            }
            "outside review owner" => {
                fixture.event["method"]["configuration"]["review_ref"] =
                    serde_json::json!(SURVIVING)
            }
            "unbound inputs" => {
                fixture.event["inputs"].as_array_mut().unwrap().pop();
            }
            "wrong output" => fixture.event["outputs"][0]["ref"] = serde_json::json!(SURVIVING),
            "no actor" => fixture.event["agent_refs"] = serde_json::json!([]),
            "reversed chronology" => {
                fixture.event["ended_at"] = serde_json::json!("2026-09-13T00:00:00Z")
            }
            "configuration extra" => {
                fixture.event["method"]["configuration"]["extra"] = serde_json::json!(true)
            }
            _ => unreachable!(),
        }
        let revision = fixture.current(canonical_json(&fixture.event), None, None);
        assert!(
            matches!(
                inspect(&fixture, revision),
                Err(RetirementRefusal::Source(_))
            ),
            "{case}"
        );
    }
    let fixture = Fixture::new(false, false);
    let raw = String::from_utf8(canonical_json(&fixture.event)).unwrap();
    let raw = raw.replacen('{', "{\"status\":\"failed\",", 1).into_bytes();
    let revision = fixture.current(raw, None, None);
    assert!(matches!(
        inspect(&fixture, revision),
        Err(RetirementRefusal::Source(_))
    ));
}

#[test]
fn retirement_cut_rejects_incoming_dependencies_and_base_identity_reuse() {
    for (incoming, reuse) in [(true, false), (false, true)] {
        let fixture = Fixture::new(incoming, reuse);
        let revision = fixture.current(canonical_json(&fixture.event), None, None);
        let result = inspect(&fixture, revision);
        let Err(RetirementRefusal::Source(message)) = result else {
            panic!("expected source refusal: {result:?}");
        };
        assert!(
            message.contains(if incoming {
                "incoming source dependency"
            } else {
                "reuses an accepted source identity"
            }),
            "{message}"
        );
    }
}

#[test]
fn retirement_cut_routes_broader_membership_or_survivor_metadata_edits_to_full_owner() {
    for case in ["survivor changed", "extra member", "survivor mode"] {
        let fixture = Fixture::new(false, false);
        let extra = match case {
            "survivor changed" => Some((SURVIVING, b"Changed surviving record.\n".to_vec())),
            "extra member" => Some((EXTRA, b"New unrelated source record.\n".to_vec())),
            _ => None,
        };
        let mode = if case == "survivor mode" {
            Some(SURVIVING)
        } else {
            None
        };
        let revision = fixture.current(canonical_json(&fixture.event), extra, mode);
        let report = inspect(&fixture, revision).unwrap();
        assert!(report.membership_transition.is_none(), "{case}");
        assert!(!report.source_admission_complete);
    }
}

#[test]
fn retirement_cut_refuses_damaged_retained_source_after_valid_event_worker_check() {
    let fixture = Fixture::new(false, false);
    let revision = fixture.current(canonical_json(&fixture.event), None, None);
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let cut = fixture.cut(revision, &cancelled, deadline);
    let (mut schemas, _) = worker(&cut, &cancelled, deadline);
    // Custody must reject exact retained source damage despite valid event
    // bytes, schema verdict and exact SHA metadata claims.
    fs::write(
        fixture
            .root
            .join("objects")
            .join(Digest256::of_bytes(SOURCE_RAW).to_hex()),
        b"damaged\n",
    )
    .unwrap();
    assert!(matches!(
        inspect_retirements_from_cut(&cut, limits(deadline), &cancelled, &mut schemas),
        Err(RetirementRefusal::Source(_))
    ));
}
