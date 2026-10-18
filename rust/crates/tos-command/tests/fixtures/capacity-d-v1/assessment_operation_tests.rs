//! Proposed source-only tests against 75315785221c58304f3e9079ea5b1ee65c6f5cfe.
//! Real protected v1 owner, asserted schema subprocess, native evaluation,
//! prepare and publication. No historical file padding or fabricated report.
//! Module inclusion is cfg(test); no runtime capability or schema changes.
use super::*;
use crate::source_creation_store::native_owner_test_support as support;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::time::Duration;
use tos_validation::FormatProfile;

const NOW: &str = "2026-10-17T12:00:00Z";
const SUBJECT: &str = "tos.claim.capacity-fixture";
type Failure = (&'static str, SourceCommandError);
type Snapshot = BTreeMap<String, (u64, u64, u32, usize, Digest256)>;

fn value(v: Value) -> JsonValue {
    cmd::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn plain(v: &JsonValue) -> Value {
    serde_json::from_slice(&cmd::canonical(v).unwrap()).unwrap()
}
fn reference(id: &str, payload: &Value) -> Value {
    serde_json::json!({"id":id,"version":1,
        "digest":cmd::record_digest(&value(payload.clone())).unwrap().to_prefixed()})
}
fn envelope(id: &str, payload: Value, origin: Value) -> Value {
    serde_json::json!({"id":id,"version":1,"payload":payload,"origin_id":origin})
}
fn secure(path: &Path) {
    fs::create_dir_all(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn snapshot(root: &Path) -> Snapshot {
    let mut result = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let meta = entry.file_type().unwrap();
            assert!(!meta.is_symlink());
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                assert!(meta.is_file());
                let raw = fs::read(entry.path()).unwrap();
                let metadata = entry.metadata().unwrap();
                result.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    (
                        metadata.dev(),
                        metadata.ino(),
                        metadata.mode(),
                        raw.len(),
                        Digest256::of_bytes(&raw),
                    ),
                );
            }
        }
    }
    result
}

struct Fixture {
    _temporary: tempfile::TempDir,
    owner: ProtectedAssessmentJournal,
    owner_path: PathBuf,
    owner_raw: Vec<u8>,
    public_root: PathBuf,
    journal: PathBuf,
    config: JsonValue,
    scope: JsonValue,
    current: JsonValue,
    event: JsonValue,
    records: Vec<JsonValue>,
    cut: CorpusCutReader,
    context: CommandContext,
}
impl Fixture {
    fn new() -> Self {
        let mut builder = tempfile::Builder::new();
        builder.permissions(fs::Permissions::from_mode(0o700));
        let temporary = if let Some(directory) = std::env::var_os("TOS_D_R4_EXACT_EVIDENCE_DIR") {
            let directory = PathBuf::from(directory).join("fixtures");
            secure(&directory);
            builder.tempdir_in(directory).unwrap()
        } else {
            builder.tempdir().unwrap()
        };
        let public_root = temporary.path().join("public");
        secure(&public_root);
        let journal = temporary.path().join("journal");
        secure(&journal);
        let contracts = public_root.join("ToS/contracts");
        secure(&contracts);
        let repository = support::repository();
        for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .unwrap()
                    .ends_with(".schema.json")
            {
                fs::copy(entry.path(), contracts.join(entry.file_name())).unwrap();
            }
        }
        // Same authored policy and qualification shape as the maintained
        // tests/conformance/rust/native_public_assessment_fixture.rs v1 case.
        let policy: Value = serde_json::from_slice(
            &fs::read(
                repository.join("ToS/doctrine/semantic-interchange/assessment-policy.v1.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let policy_ref = reference("tos.policy.knowledge-assessment", &policy);
        let execution = serde_json::json!({"procedure_ref":"fixture:source-check",
            "model_ref":"fixture:not-a-real-model"});
        let execution_ref = reference("tos.method.fixture-review", &execution);
        let calibration = serde_json::json!({"synthetic":true});
        let calibration_ref = reference("tos.review.fixture-calibration", &calibration);
        let subject = serde_json::json!({"claim":"Synthetic capacity observation only"});
        let subject_ref = reference(SUBJECT, &subject);
        let evidence = serde_json::json!({"text":"Synthetic source A"});
        let evidence_ref = reference("tos.file.fixture-a", &evidence);
        let competence = serde_json::json!({"schema_version":"tos_knowledge_assessment_competence_v1",
            "competence_id":"tos.competence.assessor-a","competence_version":1,"actor_id":"assessor-a",
            "assertion_layers":["bibliographic_assertion"],"languages":["ru","de"],
            "profile_ids":["source-observation"],"execution_profiles":[execution_ref],
            "state":"verified","valid_from":"2026-09-01T00:00:00Z","valid_until":"2099-01-01T00:00:00Z",
            "evidence_refs":[calibration_ref],"issuer_ref":"fixture:trusted-issuer"});
        let competence_ref = reference("tos.competence.assessor-a", &competence);
        let authority = serde_json::json!({"schema_version":"tos_knowledge_assessment_authority_v1",
            "authority_id":"tos.authority.assessor-a","authority_version":1,"actor_id":"assessor-a",
            "actor_kind":"agent","policy":policy_ref,"profile_ids":["source-observation"],
            "assertion_layers":["bibliographic_assertion"],"languages":["ru","de"],"uses":["research"],
            "subject_prefixes":["tos.claim."],"decisions":["admit","withdraw"],
            "competence_refs":[competence_ref],"independence_group":"assessor-a","can_supersede_others":false,
            "state":"active","valid_from":"2026-09-01T00:00:00Z","valid_until":"2099-01-01T00:00:00Z",
            "issuer_ref":"fixture:trusted-operator-grant"});
        let authority_ref = reference("tos.authority.assessor-a", &authority);
        let scope = serde_json::json!({"record":subject_ref,"assertion_layer":"bibliographic_assertion",
            "risk":"low","languages":["de"],"maker_id":"extractor","requested_use":"research","access_allowed":true});
        let config = value(
            serde_json::json!({"schema_version":"tos_local_assessment_owner_v1",
            "uid":fs::metadata(temporary.path()).unwrap().uid(),"principal_id":"assessor-a",
            "execution_profile":execution_ref,"policy":envelope("tos.policy.knowledge-assessment",policy,Value::Null),
            "authorities":[envelope("tos.authority.assessor-a",authority,Value::Null)],
            "competencies":[envelope("tos.competence.assessor-a",competence,Value::Null)],
            "records":[envelope(SUBJECT,subject,Value::Null),
                envelope("tos.file.fixture-a",evidence,serde_json::json!("source-a")),
                envelope("tos.review.fixture-calibration",calibration,Value::Null),
                envelope("tos.method.fixture-review",execution,Value::Null)],
            "journal_directory":journal,"subjects":{(SUBJECT):scope}}),
        );
        let owner_raw = cmd::canonical(&config).unwrap();
        let owner_path = temporary.path().join("owner.json");
        fs::write(&owner_path, &owner_raw).unwrap();
        fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(900);
        let owner = ProtectedAssessmentJournal::select_public_command(
            &owner_path,
            &public_root,
            deadline,
            &cancelled,
        )
        .unwrap();
        // Establish lock/home before negative membership snapshots.
        drop(
            owner
                .lock_subjects(&[SUBJECT.to_owned()], deadline, &cancelled)
                .unwrap(),
        );
        let files = support::authored(&public_root);
        let (revision, cut) =
            support::cut(&files, &temporary.path().join("cut"), deadline, &cancelled);
        let context = CommandContext {
            base_revision: revision,
            configuration_raw: owner_raw.clone(),
            request_raw: Vec::new(),
            recorded_at: NOW.to_owned(),
            effective_uid: u64::from(rustix::process::getuid().as_raw()),
            files: files
                .into_iter()
                .map(|(path, raw)| SourceFile {
                    path: RelativePath::parse(&path).unwrap(),
                    raw,
                })
                .collect(),
        };
        let event = value(
            serde_json::json!({"schema_version":"tos_knowledge_assessment_v1",
            "assessment_id":"tos.review.capacity-seed","subject":subject_ref,"policy":policy_ref,
            "profile_id":"source-observation","authority":authority_ref,"competence":competence_ref,
            "reviewer":{"actor_id":"assessor-a","kind":"agent"},"decision":"admit","rationale":"x",
            "language":"ru","evidence":[{"record":evidence_ref,"stance":"supports","locator":"synthetic"}],
            "counterevidence_search":{"status":"searched","note":"synthetic"},"limits":[],
            "method":{"procedure_ref":"fixture:source-check","invocation_ref":"fixture:synthetic",
                "model_ref":"fixture:not-a-real-model","execution_profile":execution_ref},
            "issued_at":NOW,"supersedes":[]}),
        );
        let records = cmd::array(&config, "records").unwrap().to_vec();
        let current = records
            .iter()
            .find(|v| cmd::text(v, "id").unwrap() == SUBJECT)
            .unwrap()
            .clone();
        Self {
            _temporary: temporary,
            owner,
            owner_path,
            owner_raw,
            public_root,
            journal,
            config,
            scope: value(scope),
            current,
            event,
            records,
            cut,
            context,
        }
    }
    fn resume_exact(root: &Path) -> Self {
        let admitted = PathBuf::from(std::env::var_os("TOS_D_R4_RESUME_ALLOWED_ROOT").unwrap());
        assert!(
            root.canonicalize()
                .unwrap()
                .starts_with(admitted.canonicalize().unwrap())
        );
        let mut f = Self::new();
        let deadline = Instant::now() + Duration::from_secs(900);
        let cancelled = AtomicBool::new(false);
        f.owner_path = root.join("owner.json");
        f.owner_raw = fs::read(&f.owner_path).unwrap();
        f.config = cmd::parse(&f.owner_raw).unwrap();
        f.public_root = root.join("public");
        f.journal = root.join("journal");
        assert_eq!(
            PathBuf::from(cmd::text(&f.config, "journal_directory").unwrap()),
            f.journal
        );
        f.owner = ProtectedAssessmentJournal::select_public_command(
            &f.owner_path,
            &f.public_root,
            deadline,
            &cancelled,
        )
        .unwrap();
        let files = support::authored(&f.public_root);
        let (revision, cut) = support::cut(
            &files,
            &f._temporary.path().join("resumed-cut"),
            deadline,
            &cancelled,
        );
        f.cut = cut;
        f.context = CommandContext {
            base_revision: revision,
            configuration_raw: f.owner_raw.clone(),
            request_raw: Vec::new(),
            recorded_at: NOW.to_owned(),
            effective_uid: u64::from(rustix::process::getuid().as_raw()),
            files: files
                .into_iter()
                .map(|(path, raw)| SourceFile {
                    path: RelativePath::parse(&path).unwrap(),
                    raw,
                })
                .collect(),
        };
        f.records = cmd::array(&f.config, "records").unwrap().to_vec();
        f.current = f
            .records
            .iter()
            .find(|v| cmd::text(v, "id").unwrap() == SUBJECT)
            .unwrap()
            .clone();
        assert!(
            cmd::same(
                cmd::field(cmd::field(&f.config, "subjects").unwrap(), SUBJECT).unwrap(),
                &f.scope
            )
            .unwrap()
        );
        f
    }

    fn event(&self, id: &str, rationale: usize) -> JsonValue {
        let mut v = plain(&self.event);
        v["assessment_id"] = serde_json::json!(id);
        v["rationale"] = serde_json::json!("x".repeat(rationale));
        value(v)
    }
    fn worker(&self, deadline: Instant, cancelled: &AtomicBool) -> CutWorkerSchemaExecutor {
        let path = PathBuf::from(
            std::env::var_os("TOS_SCHEMA_WORKER_PATH")
                .expect("D_R4 requires an admitted built schema worker"),
        );
        let mut budget = ExecutorBudget::laboratory();
        budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        budget.cpu_seconds = 3;
        budget.address_space_bytes = 1_073_741_824;
        CutWorkerSchemaExecutor::from_cut(
            &self.cut,
            FormatProfile::AssertedSourceCandidateV1,
            ExactWorkerIdentity {
                sha256: Digest256::of_bytes(&fs::read(&path).unwrap()),
                absolute_path: path,
            },
            budget,
            CutWorkerLimits {
                max_receipts: 128,
                max_receipt_bytes: 262_144,
            },
            deadline,
            cancelled,
        )
        .unwrap()
    }
    fn read_failure_diagnostic(
        worker: &mut CutWorkerSchemaExecutor,
        error: &SourceCommandError,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) {
        let receipts = worker.receipt_count();
        let terminal = worker.finish(deadline, cancelled);
        eprintln!(
            "D_R4_READ_FAILURE error={error:?} completed_receipts={receipts} terminal={terminal:?}"
        );
    }
    fn cold(&self) -> SourceCommandResult<(usize, usize)> {
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(900);
        let owner = ProtectedAssessmentJournal::select_public_command(
            &self.owner_path,
            &self.public_root,
            deadline,
            &cancelled,
        )?;
        let fence = owner.lock_subjects(&[SUBJECT.to_owned()], deadline, &cancelled)?;
        let mut worker = self.worker(deadline, &cancelled);
        let h = fence
            .read(SUBJECT, &self.context, &mut worker, deadline, &cancelled)
            .map_err(|error| {
                Self::read_failure_diagnostic(&mut worker, &error, deadline, &cancelled);
                error
            })?;
        worker
            .finish(deadline, &cancelled)
            .map_err(map_assessment_refusal_for_test)?;
        Ok((h.batches.len(), h.submissions.len()))
    }
    fn attempt(
        &self,
        events: Vec<JsonValue>,
        command: &str,
        expected: Option<Option<String>>,
        mode: Mode,
    ) -> Result<Attempt, Failure> {
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(900);
        let fence = self
            .owner
            .lock_subjects(&[SUBJECT.to_owned()], deadline, &cancelled)
            .map_err(|e| ("lock", e))?;
        let mut worker = self.worker(deadline, &cancelled);
        let history = fence
            .read(SUBJECT, &self.context, &mut worker, deadline, &cancelled)
            .map_err(|error| {
                Self::read_failure_diagnostic(&mut worker, &error, deadline, &cancelled);
                ("read", error)
            })?;
        let h = history
            .batches
            .iter()
            .map(|b| cmd::canonical(b).unwrap().len())
            .sum::<usize>();
        for b in &history.batches {
            let raw = cmd::canonical(b).unwrap();
            let home = self
                .journal
                .join(Digest256::of_bytes(SUBJECT.as_bytes()).to_hex());
            assert_eq!(
                fs::read(home.join(format!("{}.json", Digest256::of_bytes(&raw).to_hex())))
                    .unwrap(),
                raw
            );
        }
        let expected = expected.unwrap_or_else(|| history.head.clone());
        // This is the common owner kernel, not installed transport selection.
        // Config digest is a syntax-only snapshot marker here; whole-view
        // snapshot selection must also be covered by the separate CLI suite.
        let raw_request=serde_json::to_vec(&serde_json::json!({
            "schema_version":"tos_local_assessment_command_v1","operation":"append","subject_id":SUBJECT,
            "expected_subject":plain(&owner_reference(&self.current).unwrap()),
            "expected_snapshot":cmd::record_digest(&self.config).unwrap().to_prefixed(),
            "command_id":command,"expected_revision":expected.clone(),
            "assessments":events.iter().map(plain).collect::<Vec<_>>() })).unwrap();
        let request =
            parse_request(&raw_request, OwnerVersion::V1).map_err(|e| ("request-parser", e))?;
        let reviews = append_event_rows(
            &request.assessments,
            "assessor-a",
            cmd::field(&self.config, "execution_profile").unwrap(),
        )
        .map_err(|e| ("submission", e))?
        .iter()
        .map(|event| {
            Ok(AssessmentSubmissionInput {
                assessment: cmd::canonical(cmd::field(event, "assessment")?)?,
                principal_id: cmd::text(event, "principal_id")?.to_owned(),
                execution_profile: cmd::canonical(cmd::field(event, "execution_profile")?)?,
                committed_scope: None,
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()
        .map_err(|e| ("submission", e))?;
        let invocation = serde_json::json!({"budgets":{"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        let mut budget =
            OperationBudget::new(&invocation, deadline).map_err(|e| ("operation-budget", e))?;
        budget
            .charge_bytes(history.input_bytes().map_err(|e| ("history-bytes", e))?)
            .map_err(|e| ("history-budget", e))?;
        let mut evaluate = |reviews: &[AssessmentSubmissionInput]| {
            evaluate_selected_subject(
                &self.config,
                SUBJECT,
                &self.scope,
                &[],
                &[],
                &self.records,
                &[],
                &[],
                &history.submissions,
                reviews,
                AssessmentSourceRoute::SelectedSource,
                None,
                NOW,
                &mut worker,
                &mut budget,
                &cancelled,
            )
        };
        let current = evaluate(&[]).map_err(|e| ("evaluate-current", e))?;
        let appended = evaluate(&reviews).map_err(|e| ("evaluate-append", e))?;
        drop(evaluate);
        let all = self
            .records
            .iter()
            .map(|v| (cmd::text(v, "id").unwrap().to_owned(), v.clone()))
            .collect();
        let prepared = prepare_assessment_append(
            &request,
            NOW,
            &mut worker,
            &self.config,
            &self.scope,
            &self.current,
            &history,
            &current,
            &appended,
            &all,
            None,
            deadline,
            &cancelled,
        )
        .map_err(|e| ("prepare", e))?;
        match prepared {
            AssessmentAppendPreparation::Replayed(_) => Ok(Attempt {
                replayed: true,
                batch: None,
                h,
                new_bytes: 0,
                revision: history.head.unwrap(),
            }),
            AssessmentAppendPreparation::New {
                batch, revision, ..
            } => {
                assert_eq!(
                    appended.current_admission()["can_use"],
                    serde_json::json!(true),
                    "unqualified positive fixture"
                );
                assert_eq!(
                    appended.current_admission()["invalid_assessments"],
                    serde_json::json!([])
                );
                let new_bytes = cmd::canonical(&batch).unwrap().len();
                if !matches!(mode, Mode::Preview) {
                    let calls = std::cell::Cell::new(0usize);
                    if matches!(mode, Mode::Cancel) {
                        cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    if matches!(mode, Mode::ChangedOwner) {
                        let mut raw = self.owner_raw.clone();
                        raw.push(b' ');
                        fs::write(&self.owner_path, raw).unwrap();
                    }
                    let selected_deadline = if matches!(mode, Mode::Expired) {
                        Instant::now()
                    } else {
                        deadline
                    };
                    let result = fence.publish(
                        SUBJECT,
                        &history,
                        expected.as_deref(),
                        &batch,
                        || {
                            self.owner.verify_current(deadline, &cancelled)?;
                            calls.set(calls.get() + 1);
                            if matches!(mode, Mode::BeforeBlob) && calls.get() == 1
                                || matches!(mode, Mode::AfterBlob) && calls.get() == 2
                            {
                                return Err(SourceCommandError::Conflict(
                                    "D_R4 injected currentness interruption",
                                ));
                            }
                            Ok(())
                        },
                        selected_deadline,
                        &cancelled,
                    );
                    if matches!(mode, Mode::ChangedOwner) {
                        fs::write(&self.owner_path, &self.owner_raw).unwrap();
                    }
                    result.map_err(|e| ("publish", e))?;
                }
                Ok(Attempt {
                    replayed: false,
                    batch: Some(batch),
                    h,
                    new_bytes,
                    revision,
                })
            }
        }
    }
}
fn map_assessment_refusal_for_test(
    e: tos_validation::item_rules::ItemRefusal,
) -> SourceCommandError {
    eprintln!("D_R4_COLD_WORKER_FINISH_GATE {e:?}");
    SourceCommandError::Conflict("D_R4 reader worker finish failed")
}
#[derive(Clone, Copy)]
enum Mode {
    Normal,
    Preview,
    BeforeBlob,
    AfterBlob,
    Cancel,
    Expired,
    ChangedOwner,
}
struct Attempt {
    replayed: bool,
    batch: Option<JsonValue>,
    h: usize,
    new_bytes: usize,
    revision: String,
}

#[test]
fn d_r4_positive_qualified_echo_control() {
    let f = Fixture::new();
    let seed = f.event("tos.review.capacity-seed", 128 * 1024);
    f.attempt(vec![seed.clone()], "seed", None, Mode::Normal)
        .unwrap();
    let next = f.event("tos.review.capacity-next", 1);
    let a = f
        .attempt(vec![seed.clone(), next], "next", None, Mode::Normal)
        .unwrap();
    let b = a.batch.unwrap();
    assert_eq!(cmd::array(&b, "events").unwrap().len(), 1);
    assert_eq!(
        cmd::array(cmd::field(&b, "request").unwrap(), "events")
            .unwrap()
            .len(),
        2
    );
    let seed_bytes = cmd::canonical(&seed).unwrap();
    assert_eq!(
        cmd::array(cmd::field(&b, "request").unwrap(), "events")
            .unwrap()
            .iter()
            .filter(|event| {
                cmd::canonical(cmd::field(event, "assessment").unwrap()).unwrap() == seed_bytes
            })
            .count(),
        1
    );
    assert_eq!(f.cold().unwrap(), (2, 2));
}

#[cfg(feature = "capacity-d-diagnostics")]
fn crossing(rationale: usize, echoes: usize) {
    let f = Fixture::new();
    let seed = f.event("tos.review.capacity-seed", rationale);
    f.attempt(vec![seed.clone()], "seed", None, Mode::Normal)
        .unwrap();
    let (_, _, limit) = crate::source_assessment_journal::capacity_limits_for_test();
    for i in 1..=1025 {
        let before = snapshot(&f.journal);
        let mut submitted = vec![seed.clone(); echoes];
        submitted.push(f.event(&format!("tos.review.capacity-{i}"), 1));
        let result = f.attempt(submitted, &format!("echo-{i}"), None, Mode::Normal);
        match result {
            Ok(a) => {
                assert!(!a.replayed);
                let b = a.batch.as_ref().unwrap();
                assert_eq!(cmd::array(b, "events").unwrap().len(), 1);
                let submitted = cmd::array(cmd::field(b, "request").unwrap(), "events").unwrap();
                assert_eq!(submitted.len(), echoes + 1);
                let seed_bytes = cmd::canonical(&seed).unwrap();
                assert_eq!(
                    submitted
                        .iter()
                        .filter(|event| {
                            cmd::canonical(cmd::field(event, "assessment").unwrap()).unwrap()
                                == seed_bytes
                        })
                        .count(),
                    echoes
                );
                assert!(a.new_bytes <= 1_048_576);
                if a.h + a.new_bytes > limit {
                    match f.cold() {
                        Err(SourceCommandError::Invalid(
                            "assessment complete history byte budget",
                        )) => panic!(
                            "D_R4_REPRODUCED_UNREADABLE_HEAD H={} next={} L={} revision={}",
                            a.h, a.new_bytes, limit, a.revision
                        ),
                        Err(error) => panic!(
                            "D_R4_REOPEN_OTHER_GATE H={} next={} L={limit} error={error:?}",
                            a.h, a.new_bytes
                        ),
                        Ok(_) => {
                            panic!("D_R4 reader accepted history above its declared raw budget")
                        }
                    }
                }
                assert_eq!(f.cold().unwrap(), (i + 1, i + 1));
            }
            Err((phase, error)) => {
                assert_eq!(
                    before,
                    snapshot(&f.journal),
                    "D_R4 refusal changed head/membership phase={phase}"
                );
                assert!(f.cold().is_ok(), "D_R4 early refusal broke cold reopen");
                if matches!(phase, "prepare" | "publish")
                    && matches!(
                        &error,
                        SourceCommandError::Invalid("assessment complete history byte budget")
                    )
                {
                    eprintln!(
                        "D_R4_BOUNDED_BYTE_REFUSAL rationale={rationale} echoes={echoes} i={i} L={limit}"
                    );
                    return;
                }
                panic!(
                    "D_R4_EARLY_GATE rationale={rationale} echoes={echoes} i={i} phase={phase} error={error:?}; crossing not reached"
                );
            }
        }
    }
    panic!("D_R4 no decisive history boundary reached");
}
#[cfg(feature = "capacity-d-diagnostics")]
#[test]
fn d_r4_echo_128k_never_commits_unreadable_head() {
    crossing(128 * 1024, 1);
}
#[cfg(feature = "capacity-d-diagnostics")]
#[test]
fn d_r4_echo_256k_never_commits_unreadable_head() {
    crossing(256 * 1024, 1);
}
#[cfg(feature = "capacity-d-diagnostics")]
#[test]
fn d_r4_echo_384k_never_commits_unreadable_head() {
    crossing(384 * 1024, 1);
}

// Additional bounded hypothesis: two exact old-event echoes at 480KiB.
// The real request and batch guards remain active; failure is classified,
// never bypassed by a larger worker/record/operation budget.
#[cfg(feature = "capacity-d-diagnostics")]
#[test]
fn d_r4_echo_480k_twice_never_commits_unreadable_head() {
    crossing(480 * 1024, 2);
}

#[test]
fn d_r4_retry_conflict_currentness_cancellation_and_orphan_recovery() {
    let f = Fixture::new();
    let seed = f.event("tos.review.capacity-seed", 1);
    f.attempt(vec![seed.clone()], "seed", None, Mode::Normal)
        .unwrap();
    let e = f.event("tos.review.capacity-second", 1);
    let preview = f
        .attempt(vec![e.clone()], "second", None, Mode::Preview)
        .unwrap();
    let old_head = plain(preview.batch.as_ref().unwrap())["previous_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    for mode in [
        Mode::BeforeBlob,
        Mode::Cancel,
        Mode::Expired,
        Mode::ChangedOwner,
    ] {
        let before = snapshot(&f.journal);
        assert!(f.attempt(vec![e.clone()], "second", None, mode).is_err());
        assert_eq!(before, snapshot(&f.journal));
        assert_eq!(f.cold().unwrap(), (1, 1));
    }
    let before = snapshot(&f.journal);
    assert!(
        f.attempt(vec![e.clone()], "second", None, Mode::AfterBlob)
            .is_err()
    );
    let after = snapshot(&f.journal);
    assert_eq!(
        after.len(),
        before.len() + 1,
        "one retained immutable orphan expected"
    );
    for (name, member) in &before {
        assert_eq!(after.get(name), Some(member));
    }
    assert_eq!(f.cold().unwrap(), (1, 1));
    let recovered = f
        .attempt(vec![e.clone()], "second", None, Mode::Normal)
        .unwrap();
    assert_eq!(recovered.revision, preview.revision);
    assert_eq!(
        snapshot(&f.journal).len(),
        after.len(),
        "retry added another immutable blob"
    );
    assert_eq!(f.cold().unwrap(), (2, 2));
    let stable = snapshot(&f.journal);
    let replay = f
        .attempt(
            vec![e.clone()],
            "second",
            Some(Some(old_head.clone())),
            Mode::Normal,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(stable, snapshot(&f.journal));
    let changed = f.event("tos.review.capacity-second", 2);
    assert!(
        f.attempt(
            vec![changed],
            "second",
            Some(Some(old_head.clone())),
            Mode::Normal
        )
        .is_err()
    );
    assert!(
        f.attempt(
            vec![f.event("tos.review.capacity-third", 1)],
            "third",
            Some(Some(old_head)),
            Mode::Normal
        )
        .is_err()
    );
    assert_eq!(stable, snapshot(&f.journal));
    assert_eq!(f.cold().unwrap(), (2, 2));
}

#[test]
fn d_r4_unqualified_new_event_preserves_head_and_membership() {
    let f = Fixture::new();
    f.attempt(
        vec![f.event("tos.review.capacity-seed", 1)],
        "seed",
        None,
        Mode::Normal,
    )
    .unwrap();
    let mut bad = plain(&f.event("tos.review.capacity-unqualified", 1));
    bad["reviewer"]["actor_id"] = serde_json::json!("ungranted-actor");
    let before = snapshot(&f.journal);
    let failure = f
        .attempt(vec![value(bad)], "bad", None, Mode::Normal)
        .err()
        .expect("unqualified event accepted");
    assert_eq!(failure.0, "prepare");
    assert!(matches!(failure.1, SourceCommandError::Denied(_)));
    assert_eq!(before, snapshot(&f.journal));
    assert_eq!(f.cold().unwrap(), (1, 1));
}

#[test]
fn d_r4_canonical_batch_1mib_minus_exact_plus_boundary() {
    let f = Fixture::new();
    let base = f
        .attempt(
            vec![f.event("tos.review.capacity-size", 1)],
            "batch",
            None,
            Mode::Preview,
        )
        .unwrap();
    for target in [1_048_575usize, 1_048_576, 1_048_577] {
        // Rationale occurs twice in a first batch; command ID occurs once.
        // Both accepted sizes below independently check this exact sizing.
        let delta = target.checked_sub(base.new_bytes).unwrap();
        let command = if delta % 2 == 0 { "batch" } else { "batchx" };
        let before = snapshot(&f.journal);
        let result = f.attempt(
            vec![f.event("tos.review.capacity-size", 1 + delta / 2)],
            command,
            None,
            Mode::Preview,
        );
        if target <= 1_048_576 {
            assert_eq!(result.unwrap().new_bytes, target);
        } else {
            let (phase, error) = result.err().expect("oversize batch accepted");
            assert_eq!(phase, "prepare");
            assert!(matches!(
                error,
                SourceCommandError::Invalid("assessment batch byte budget")
            ));
        }
        assert_eq!(before, snapshot(&f.journal));
        assert_eq!(f.cold().unwrap(), (0, 0));
    }
}

#[test]
fn d_r4_unique_input_1023_1024_1025_earlier_refusals_preserve_state() {
    for total in [1023usize, 1024, 1025] {
        let f = Fixture::new();
        f.attempt(
            vec![f.event("tos.review.capacity-seed", 1)],
            "seed",
            None,
            Mode::Normal,
        )
        .unwrap();
        let before = snapshot(&f.journal);
        let events = (1..total)
            .map(|i| f.event(&format!("tos.review.capacity-count-{i}"), 1))
            .collect();
        let (phase, error) = f.attempt(events, "count", None, Mode::Normal).err().expect(
            "large single request unexpectedly committed; inspect actual batch/count limits",
        );
        eprintln!(
            "D_R4_COUNT_EARLY_REFUSAL unique_input={total} phase={phase} error={error:?}; not committed-history coverage"
        );
        assert_eq!(before, snapshot(&f.journal));
        assert_eq!(f.cold().unwrap(), (1, 1));
    }
}

#[cfg(feature = "capacity-d-diagnostics")]
fn exact_observe(
    f: &Fixture,
    target: usize,
    i: usize,
    action: &str,
    before: &Snapshot,
    result: &Result<Attempt, Failure>,
) {
    let Some(directory) = std::env::var_os("TOS_D_R4_EXACT_EVIDENCE_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    secure(&directory);
    let after = snapshot(&f.journal);
    let describe = |state: &Snapshot| {
        state
            .iter()
            .map(|(path, (dev, ino, mode, bytes, digest))| {
                serde_json::json!({"path":path,"dev":dev,"ino":ino,"mode":mode,
            "bytes":bytes,"sha256":digest.to_hex()})
            })
            .collect::<Vec<_>>()
    };
    let files = describe(&after);
    let files_before = describe(before);
    let outcome = match result {
        Ok(a) => serde_json::json!({"ok":true,"h":a.h,"new_bytes":a.new_bytes,
            "revision":a.revision,"replayed":a.replayed}),
        Err((phase, error)) => {
            serde_json::json!({"ok":false,"phase":phase,"error":format!("{error:?}"),
            "cold_after_refusal":format!("{:?}",f.cold())})
        }
    };
    let row = serde_json::json!({"target":target,"i":i,"action":action,
        "fixture":f.owner_path.parent().unwrap(),"scratch":f._temporary.path(),"journal":f.journal,
        "before_equals_after":before==&after,"files_before":files_before,
        "files_after":files,"outcome":outcome,
        "observed_unix_millis":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()});
    let path = directory.join(format!("{target}.jsonl"));
    let mut output = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    use std::io::Write;
    writeln!(output, "{}", serde_json::to_string(&row).unwrap()).unwrap();
    eprintln!("D_R4_EXACT_OBSERVATION target={target} i={i} action={action} outcome={outcome}");
}

#[cfg(feature = "capacity-d-diagnostics")]
fn copy_exact_journal(source: &Path, destination: &Path) {
    let before = snapshot(source);
    fs::create_dir(destination).unwrap();
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700)).unwrap();
    for (name, (_, _, mode, _, _)) in &before {
        let target = destination.join(name);
        secure(target.parent().unwrap());
        fs::copy(source.join(name), &target).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777)).unwrap();
    }
    assert_eq!(before, snapshot(source));
    let copied = snapshot(destination);
    assert_eq!(before.len(), copied.len());
    for (name, (_, _, mode, bytes, digest)) in before {
        let (_, _, copy_mode, copy_bytes, copy_digest) = copied.get(&name).unwrap();
        assert_eq!(
            (mode, bytes, digest),
            (*copy_mode, *copy_bytes, *copy_digest)
        );
    }
}

#[cfg(feature = "capacity-d-diagnostics")]
fn exact_history_target(target: usize) {
    let resume = std::env::var_os("TOS_D_R4_RESUME_FIXTURE").map(PathBuf::from);
    let mut f = match &resume {
        Some(root) => Fixture::resume_exact(root),
        None => Fixture::new(),
    };
    if std::env::var_os("TOS_D_R4_EXACT_EVIDENCE_DIR").is_some() {
        f._temporary.disable_cleanup(true);
        eprintln!(
            "D_R4_EXACT_RETAIN target={target} fixture={}",
            f._temporary.path().display()
        );
    }
    if std::env::var_os("TOS_D_R4_READ_ONLY_DIAGNOSTIC").is_some() {
        assert!(
            resume.is_some(),
            "read-only diagnostic requires retained fixture"
        );
        // An explicit collector, never a successful byte-boundary test. Every
        // fresh read keeps the same native worker limits and exact journal.
        for repeat in 0..3 {
            let before = snapshot(&f.journal);
            let result = f.cold();
            assert_eq!(before, snapshot(&f.journal));
            eprintln!("D_R4_READ_ONLY_DIAGNOSTIC repeat={repeat} result={result:?} unchanged=true");
        }
        panic!("D_R4_READ_ONLY_DIAGNOSTIC_COMPLETE; exact byte target not executed");
    }
    let seed = f.event("tos.review.capacity-seed", 480 * 1024);
    let start_i = if resume.is_some() {
        let state_before = snapshot(&f.journal);
        let counts = f.cold().expect("D_R4_RESUME_COLD_FAILED");
        assert_eq!(state_before, snapshot(&f.journal));
        assert_eq!(counts.0, counts.1);
        eprintln!(
            "D_R4_RESUME_COLD target={target} counts={counts:?} journal={}",
            f.journal.display()
        );
        counts.0
    } else {
        let initial = snapshot(&f.journal);
        let seeded = f.attempt(vec![seed.clone()], "seed", None, Mode::Normal);
        exact_observe(&f, target, 0, "seed", &initial, &seeded);
        seeded.unwrap();
        1
    };
    let (_, _, limit) = crate::source_assessment_journal::capacity_limits_for_test();
    for i in start_i..=96 {
        let id = format!("tos.review.capacity-exact-{i}");
        let command = format!("exact-{i}");
        let small = f.event(&id, 1);
        let before = snapshot(&f.journal);
        let raw_history = before
            .iter()
            .filter(|(name, _)| name.ends_with(".json"))
            .map(|(_, v)| v.3)
            .sum::<usize>();
        if target
            .checked_sub(raw_history)
            .is_some_and(|remaining| remaining > 2 * 1_048_576)
        {
            // Each real producer batch is still bounded by1MiB. Keep two full
            // batches of slack; do not repeat two discarded previews per prefix.
            let appended = f.attempt(
                vec![seed.clone(), seed.clone(), small],
                &command,
                None,
                Mode::Normal,
            );
            exact_observe(&f, target, i, "resumed-prefix-publish", &before, &appended);
            let appended = appended.unwrap();
            assert_eq!(appended.h, raw_history);
            assert!(appended.new_bytes <= 1_048_576);
            assert!(appended.h + appended.new_bytes < target);
            // The next real operation validates the complete chain again.
            // Mandatory fresh-owner cold read remains at resume and terminal.
            continue;
        }
        // Find the small admissible terminal layout before an optional echo
        // search probe. A final-slice target does not need an oversized echo.
        let terminal = f.attempt(vec![small.clone()], &command, None, Mode::Preview);
        exact_observe(&f, target, i, "preview-terminal", &before, &terminal);
        let terminal = terminal.unwrap_or_else(|(phase, error)| {
            panic!("D_R4_EXACT_TERMINAL_GATE target={target} i={i} phase={phase} error={error:?}")
        });
        assert_eq!(before, snapshot(&f.journal));
        let remaining = target
            .checked_sub(terminal.h)
            .expect("producer prefix exceeded target");
        // Leave room for the next admission ref/sequence digits; this is
        // fixture tiling slack, not a changed production budget.
        let full_prefix_fits = if remaining > 1_048_576 {
            let echoed = f.attempt(
                vec![seed.clone(), seed.clone(), small],
                &command,
                None,
                Mode::Preview,
            );
            exact_observe(&f, target, i, "preview-echo", &before, &echoed);
            let echoed = echoed.unwrap_or_else(|(phase, error)| {
                panic!("D_R4_EXACT_EARLY_GATE target={target} i={i} phase={phase} error={error:?}")
            });
            assert_eq!(echoed.h, terminal.h);
            assert_eq!(before, snapshot(&f.journal));
            remaining > echoed.new_bytes + terminal.new_bytes + 4096
        } else {
            false
        };
        if full_prefix_fits || remaining > 1_048_576 {
            let events = if full_prefix_fits {
                vec![seed.clone(), seed.clone(), f.event(&id, 1)]
            } else {
                vec![f.event(&id, 1)]
            };
            let appended = f.attempt(events, &command, None, Mode::Normal);
            exact_observe(&f, target, i, "prefix-publish", &before, &appended);
            appended.unwrap();
            assert!(f.cold().is_ok());
            continue;
        }
        if let Some(directory) = std::env::var_os("TOS_D_R4_EXACT_CHECKPOINT_DIR") {
            let destination = PathBuf::from(directory).join(format!("before-{target}"));
            copy_exact_journal(&f.journal, &destination);
            eprintln!(
                "D_R4_EXACT_CHECKPOINT target={target} journal={} checkpoint={}",
                f.journal.display(),
                destination.display()
            );
        }
        let delta = remaining
            .checked_sub(terminal.new_bytes)
            .expect("D_R4_TARGET_LAYOUT_GAP");
        let final_command = if delta % 2 == 0 {
            command
        } else {
            format!("{command}x")
        };
        let result = f.attempt(
            vec![f.event(&id, 1 + delta / 2)],
            &final_command,
            None,
            Mode::Normal,
        );
        exact_observe(&f, target, i, "terminal-publish", &before, &result);
        match result {
            Ok(a) => {
                assert_eq!(
                    a.h + a.new_bytes,
                    target,
                    "producer canonical size differs from exact target"
                );
                assert!(a.new_bytes <= 1_048_576);
                let reopened = f.cold();
                eprintln!(
                    "D_R4_EXACT_COLD target={target} H={} next={} result={reopened:?}",
                    a.h, a.new_bytes
                );
                match reopened {
                    Ok(counts) => {
                        assert!(
                            target <= limit,
                            "reader/writer raw-history bound unexpectedly changed"
                        );
                        assert_eq!(counts, (i + 1, i + 1));
                    }
                    Err(SourceCommandError::Invalid("assessment complete history byte budget"))
                        if target > limit =>
                    {
                        panic!(
                            "D_R4_REPRODUCED_EXACT_UNREADABLE_HEAD H={} next={} target={target} L={limit}",
                            a.h, a.new_bytes
                        )
                    }
                    Err(error) => {
                        panic!("D_R4_EXACT_REOPEN_OTHER_GATE target={target} error={error:?}")
                    }
                }
            }
            Err((phase, error)) => {
                assert_eq!(
                    before,
                    snapshot(&f.journal),
                    "exact-target refusal changed head/membership"
                );
                assert!(f.cold().is_ok());
                assert!(
                    target > limit
                        && matches!(phase, "prepare" | "publish")
                        && matches!(
                            &error,
                            SourceCommandError::Invalid("assessment complete history byte budget")
                        ),
                    "D_R4_EXACT_EARLY_GATE target={target} phase={phase} error={error:?}"
                );
            }
        }
        return;
    }
    panic!("D_R4 exact producer target not reached; do not interpret as a pass");
}
#[cfg(feature = "capacity-d-diagnostics")]
#[test]
fn d_r4_producer_history_32mib_minus_one() {
    exact_history_target(33_554_431);
}
#[cfg(feature = "capacity-d-diagnostics")]
#[test]
fn d_r4_producer_history_32mib_exact() {
    exact_history_target(33_554_432);
}
#[cfg(feature = "capacity-d-diagnostics")]
#[test]
fn d_r4_producer_history_32mib_plus_one_never_breaks_reopen() {
    exact_history_target(33_554_433);
}

#[test]
fn d_r4_withdrawal_retains_history_and_old_echo_identity() {
    let f = Fixture::new();
    let seed = f.event("tos.review.capacity-seed", 1);
    f.attempt(vec![seed.clone()], "seed", None, Mode::Normal)
        .unwrap();
    f.attempt(
        vec![f.event("tos.review.capacity-independent", 1)],
        "independent",
        None,
        Mode::Normal,
    )
    .unwrap();
    let mut withdrawal = plain(&f.event("tos.review.capacity-withdrawal", 1));
    withdrawal["decision"] = serde_json::json!("withdraw");
    withdrawal["supersedes"] =
        serde_json::json!([reference("tos.review.capacity-seed", &plain(&seed))]);
    f.attempt(vec![value(withdrawal)], "withdraw", None, Mode::Normal)
        .unwrap();
    assert_eq!(f.cold().unwrap(), (3, 3));
    let next = f
        .attempt(
            vec![seed, f.event("tos.review.capacity-after-withdrawal", 1)],
            "after-withdrawal",
            None,
            Mode::Normal,
        )
        .unwrap();
    assert_eq!(
        cmd::array(next.batch.as_ref().unwrap(), "events")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(f.cold().unwrap(), (4, 4));
}
