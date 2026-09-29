//! One composed native form lifecycle over existing independent Python oracle
//! bytes, actual anchored source cuts and the selected disposable schema worker.
//! Carrier/schema green still ends in the explicit full-admission refusal.
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_command::{CommandContext, SourceCommandError, SourceFile};
use tos_command::source_forms::{run_form_command, run_form_command_from_captures};
use tos_command::source_operation::{SourceOperationError, bind_selected_candidate};
use tos_source_store::{CorpusCutReader, CutReadLimits, SoftwareCaptureReader};
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::operation::OperationLimits;
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};

fn fixture_files(profile: &str) -> (BTreeMap<String, Vec<u8>>, Vec<u8>, String) {
    let repository = super::validation_cut_cases::repository();
    let packet = repository
        .join("rust/crates/tos-command/tests/fixtures/source_forms_shadow")
        .join(profile);
    let independent_work = profile == "de_constantia";
    let claim_profile = matches!(profile, "claim_v1" | "claim_v2");
    let config = fs::read(packet.join(if independent_work || claim_profile {
        "owner.json"
    } else {
        "owner.synthetic.json"
    }))
    .unwrap();
    let owner: Value = serde_json::from_slice(&config).unwrap();
    let path = owner["source_path"].as_str().unwrap().to_owned();
    let target = if claim_profile {
        format!(
            "{}.{}.human-forms.json",
            path.strip_suffix(".jsonl").unwrap(),
            Digest256::of_bytes(owner["claim_id"].as_str().unwrap().as_bytes()).to_hex()
        )
    } else {
        format!("{}.human-forms.json", path.strip_suffix(".json").unwrap())
    };
    let mut files = BTreeMap::new();
    // Existing exact selected schema resources; no production corpus traversal.
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            files.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    // Native oracle pins its exact source schema, independently of current
    // schema evolution. The fixture worker receives these selected bytes.
    if !profile.is_empty() && !independent_work && !claim_profile {
        let source: Value =
            serde_json::from_slice(&fs::read(packet.join("source.initial.json")).unwrap()).unwrap();
        let schema = match source["schema_version"].as_str().unwrap() {
            "tos_artifact_source_witness_v1" => "artifact-source-witness.schema.json",
            "tos_artifact_source_witness_v2" => "artifact-source-witness-v2.schema.json",
            "tos_scholarly_composite_witness_v1" => "scholarly-composite-witness.schema.json",
            _ => panic!("unknown selected oracle source"),
        };
        files.insert(
            format!("ToS/contracts/{schema}"),
            fs::read(packet.join("source-schema.initial.json")).unwrap(),
        );
    }
    files.insert(
        path,
        fs::read(packet.join(if independent_work || claim_profile {
            "source.json"
        } else {
            "source.initial.json"
        }))
        .unwrap(),
    );
    files.insert(
        target.clone(),
        fs::read(packet.join(if independent_work || claim_profile {
            "initial.json"
        } else {
            "form-set.initial.json"
        }))
        .unwrap(),
    );
    if claim_profile {
        let expected: BTreeMap<String, String> =
            serde_json::from_slice(&fs::read(packet.join("source_contracts.json")).unwrap())
                .unwrap();
        for (path, digest) in expected {
            let raw = fs::read(repository.join(&path)).unwrap();
            assert_eq!(
                Digest256::of_bytes(&raw).to_prefixed(),
                digest,
                "pinned Claim oracle source resource: {path}"
            );
            files.insert(path, raw);
        }
    }
    (files, config, target)
}
pub(super) fn open_cut(
    root: &Path,
    revision: SourceRevision,
    deadline: Instant,
    cancel: &AtomicBool,
) -> CorpusCutReader {
    CorpusReader::open_existing(
        root,
        ReadLimits {
            max_manifest_bytes: 4_194_304,
            max_manifest_entries: 2048,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
    )
    .unwrap()
    .open_source_cut(
        revision,
        CutReadLimits {
            max_revisions: 4,
            max_members: 2048,
            max_total_bytes: 33_554_432,
            max_member_bytes: 8_388_608,
        },
        deadline,
        cancel,
    )
    .unwrap()
}
pub(super) fn schemas(
    cut: &CorpusCutReader,
    deadline: Instant,
    cancel: &AtomicBool,
) -> CutWorkerSchemaExecutor {
    schemas_for_profile(
        cut,
        FormatProfile::LegacyPythonObserved20260923,
        deadline,
        cancel,
    )
}
pub(super) fn schemas_for_profile(
    cut: &CorpusCutReader,
    profile: FormatProfile,
    deadline: Instant,
    cancel: &AtomicBool,
) -> CutWorkerSchemaExecutor {
    schemas_for_profile_with_budget(cut, profile, ExecutorBudget::laboratory(), deadline, cancel)
}
pub(super) fn schemas_for_profile_with_budget(
    cut: &CorpusCutReader,
    profile: FormatProfile,
    budget: ExecutorBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> CutWorkerSchemaExecutor {
    let absolute_path = super::validation_cut_cases::selected_worker_path();
    let sha256 = Digest256::of_bytes(&fs::read(&absolute_path).unwrap());
    CutWorkerSchemaExecutor::from_cut(
        cut,
        profile,
        ExactWorkerIdentity {
            absolute_path,
            sha256,
        },
        budget,
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 262_144,
        },
        deadline,
        cancel,
    )
    .unwrap()
}
pub(super) fn context(
    files: &BTreeMap<String, Vec<u8>>,
    configuration_raw: Vec<u8>,
    request_raw: Vec<u8>,
    base_revision: SourceRevision,
) -> CommandContext {
    CommandContext {
        base_revision,
        configuration_raw,
        request_raw,
        recorded_at: "2026-01-01T12:34:56+00:00".into(),
        effective_uid: 1000,
        files: files
            .iter()
            .map(|(path, raw)| SourceFile {
                path: RelativePath::parse(path).unwrap(),
                raw: raw.clone(),
            })
            .collect(),
    }
}
pub(super) fn successor(
    files: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    base: SourceRevision,
) -> SourceRevision {
    let unlinked = super::validation_cut_cases::write_cut_store(files, root);
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(
            root.join("revisions")
                .join(unlinked.0.to_hex())
                .join("snapshot.json"),
        )
        .unwrap(),
    )
    .unwrap();
    manifest["base_revision"] = Value::String(base.0.to_hex());
    manifest.as_object_mut().unwrap().remove("revision");
    let revision = SourceRevision(Digest256::of_bytes(&canonical_json(&manifest)));
    manifest["revision"] = Value::String(revision.0.to_hex());
    let directory = root.join("revisions").join(revision.0.to_hex());
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("snapshot.json"), canonical_json(&manifest)).unwrap();
    revision
}

fn assert_form_oracle_response(response: &tos_foundation::JsonValue, expected: &Path) {
    let actual: Value = serde_json::from_slice(
        &tos_foundation::canonical_bytes_v1(
            response,
            tos_foundation::CanonicalProfile::SourceCommandInputV1,
            JsonLimits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let oracle: Value = serde_json::from_slice(&fs::read(expected).unwrap()).unwrap();
    assert_eq!(actual, oracle, "{}", expected.display());
}

#[test]
fn maintained_forms_propose_exact_bytes_bind_real_cut_and_refuse_unissued_admission() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let commit_output = std::process::Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .unwrap();
    assert!(commit_output.status.success());
    assert!(commit_output.stdout.len() <= 41);
    let commit = String::from_utf8(commit_output.stdout).unwrap();
    let component_path =
        RelativePath::parse("rust/crates/tos-command/src/source_forms.rs").unwrap();
    // One explicit captured rule-input file is byte evidence only: this does
    // not claim running executable identity or complete producer provenance.
    let capture = super::source_cut_cases::captured_software_fixture(
        &repository,
        commit.trim(),
        &[component_path.as_str()],
    );
    let other_capture = super::source_cut_cases::captured_software_fixture(
        &repository,
        commit.trim(),
        &[
            component_path.as_str(),
            "rust/crates/tos-command/src/source_command.rs",
        ],
    );
    for profile in [
        "",
        "artifact-v1",
        "artifact-v2",
        "composite-v1",
        "de_constantia",
        "claim_v1",
        "claim_v2",
    ] {
        let (mut files, config, target) = fixture_files(profile);
        let packet = super::validation_cut_cases::repository()
            .join("rust/crates/tos-command/tests/fixtures/source_forms_shadow")
            .join(profile);
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("store");
        let base = super::validation_cut_cases::write_cut_store(&files, &root);
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(180);
        let software = SoftwareCaptureReader::open(
            &capture.capture,
            &capture.restored,
            capture.selection.clone(),
            ReadLimits {
                max_manifest_bytes: 1_048_576,
                max_manifest_entries: 128,
                max_selected_object_bytes: 8_388_608,
                json: JsonLimits::default(),
            },
            deadline,
            &cancel,
        )
        .unwrap();
        let components = software
            .select_components(&[component_path.clone()])
            .unwrap();
        let software_raw = software
            .read_selected_component(&components, &component_path, 8_388_608, deadline, &cancel)
            .unwrap();
        let cut = open_cut(&root, base, deadline, &cancel);
        let mut worker = schemas(&cut, deadline, &cancel);
        let independent_work = profile == "de_constantia";
        let claim_profile = matches!(profile, "claim_v1" | "claim_v2");
        let request = fs::read(packet.join(if independent_work || claim_profile {
            "apply_request.json"
        } else {
            "apply.request.json"
        }))
        .unwrap();
        let mut ctx = context(&files, config.clone(), request.clone(), base);
        ctx.files.push(SourceFile {
            path: component_path.clone(),
            raw: software_raw.clone(),
        });
        if independent_work {
            // Transfer the shadow's independent ru/Cyrl Work oracle through
            // the real selected-capture handler. Both revise and create preview
            // must preserve the source-owned language/context bindings.
            for (input, expected) in [
                ("describe_request.json", "describe.json"),
                ("prepare_request.json", "prepare.json"),
                ("create_request.json", "create.json"),
            ] {
                let mut observed = ctx.clone();
                observed.request_raw = fs::read(packet.join(input)).unwrap();
                let result = run_form_command_from_captures(
                    &observed,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel,
                )
                .unwrap();
                assert!(result.changes.is_empty());
                let actual: Value = serde_json::from_slice(
                    &tos_foundation::canonical_bytes_v1(
                        &result.response,
                        tos_foundation::CanonicalProfile::SourceCommandInputV1,
                        JsonLimits::default(),
                    )
                    .unwrap(),
                )
                .unwrap();
                let oracle: Value =
                    serde_json::from_slice(&fs::read(packet.join(expected)).unwrap()).unwrap();
                assert_eq!(actual, oracle, "{profile}: {input}");
            }
        }
        if profile.is_empty() || matches!(profile, "artifact-v1" | "artifact-v2" | "composite-v1") {
            // Remaining independent shadow goldens now call the real handler.
            let previews = if profile.is_empty() {
                vec![
                    (
                        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
                        "describe.response.json",
                    ),
                    (
                        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare","form_id":"tos.form.jenseits-von-gut-und-boese.name-original","field_id":"metadata.preferred-name"}),
                        "prepare.preferred.response.json",
                    ),
                    (
                        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare","form_id":"tos.form.oracle.jgb-name-ru-copy","field_id":"metadata.variant-name:0"}),
                        "prepare.russian.response.json",
                    ),
                ]
            } else {
                [
                    ("describe.request.json", "describe.response.json"),
                    ("prepare.note.request.json", "prepare.note.response.json"),
                    ("prepare.name.request.json", "prepare.name.response.json"),
                ]
                .into_iter()
                .map(|(input, expected)| {
                    (
                        serde_json::from_slice(&fs::read(packet.join(input)).unwrap()).unwrap(),
                        expected,
                    )
                })
                .collect()
            };
            for (input, expected) in previews {
                let mut observed = ctx.clone();
                observed.request_raw = serde_json::to_vec(&input).unwrap();
                let result = run_form_command_from_captures(
                    &observed,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel,
                )
                .unwrap();
                assert!(result.changes.is_empty());
                assert_form_oracle_response(&result.response, &packet.join(expected));
            }
        }
        if claim_profile {
            let mut operations = vec![
                ("describe_request.json", "describe.json"),
                (
                    "prepare-claim-statement.request.json",
                    "prepare-claim-statement.response.json",
                ),
            ];
            if profile == "claim_v2" {
                operations.extend([
                    (
                        "prepare-claim-name.request.json",
                        "prepare-claim-name.response.json",
                    ),
                    (
                        "prepare-claim-caption.request.json",
                        "prepare-claim-caption.response.json",
                    ),
                    (
                        "prepare-claim-hover.request.json",
                        "prepare-claim-hover.response.json",
                    ),
                ]);
            }
            for (input, expected) in operations {
                let mut observed = ctx.clone();
                observed.request_raw = fs::read(packet.join(input)).unwrap();
                let result = run_form_command_from_captures(
                    &observed,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel,
                )
                .unwrap();
                assert!(result.changes.is_empty());
                assert_form_oracle_response(&result.response, &packet.join(expected));
            }
            if profile == "claim_v1" {
                let mut forbidden = ctx.clone();
                forbidden.request_raw = fs::read(
                    packet
                        .parent()
                        .unwrap()
                        .join("claim_v2/prepare-claim-name.request.json"),
                )
                .unwrap();
                assert!(matches!(
                    run_form_command_from_captures(
                        &forbidden,
                        &cut,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        &cancel,
                    ),
                    Err(SourceCommandError::Denied(_))
                ));
            }
        }
        if independent_work {
            let mut wrong_guard = ctx.clone();
            let request = String::from_utf8(wrong_guard.request_raw.clone()).unwrap();
            assert!(request.contains("/field_languages/notes"));
            wrong_guard.request_raw = request
                .replace("/field_languages/notes", "/absent-language-guard")
                .into_bytes();
            assert!(
                run_form_command_from_captures(
                    &wrong_guard,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel,
                )
                .is_err(),
                "source-owned language context cannot be replaced"
            );
            let mut unknown_field = ctx.clone();
            let request = fs::read_to_string(packet.join("prepare_request.json")).unwrap();
            assert!(request.contains("metadata.source-note"));
            unknown_field.request_raw = request
                .replace("metadata.source-note", "metadata.unknown")
                .into_bytes();
            assert!(matches!(
                run_form_command_from_captures(
                    &unknown_field,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel,
                ),
                Err(SourceCommandError::Invalid(_))
            ));
        }
        if profile == "claim_v1" {
            let owner: Value = serde_json::from_slice(&config).unwrap();
            let source_path = owner["source_path"].as_str().unwrap();
            for (name, duplicated) in [("absent", false), ("duplicate", true)] {
                let mut invalid_files = files.clone();
                let raw = if duplicated {
                    let mut raw = invalid_files[source_path].clone();
                    raw.push(b'\n');
                    raw.extend_from_slice(&invalid_files[source_path]);
                    raw
                } else {
                    Vec::new()
                };
                invalid_files.insert(source_path.to_owned(), raw);
                let invalid_root = temporary.path().join(name);
                let invalid_base =
                    super::validation_cut_cases::write_cut_store(&invalid_files, &invalid_root);
                let invalid_cut = open_cut(&invalid_root, invalid_base, deadline, &cancel);
                let mut invalid_worker = schemas(&invalid_cut, deadline, &cancel);
                let mut invalid_ctx = context(
                    &invalid_files,
                    config.clone(),
                    fs::read(packet.join("describe_request.json")).unwrap(),
                    invalid_base,
                );
                invalid_ctx.files.push(SourceFile {
                    path: component_path.clone(),
                    raw: software_raw.clone(),
                });
                assert!(
                    matches!(
                        run_form_command_from_captures(
                            &invalid_ctx,
                            &invalid_cut,
                            &software,
                            &components,
                            &mut invalid_worker,
                            deadline,
                            &cancel,
                        ),
                        Err(SourceCommandError::Invalid(_))
                    ),
                    "selected Claim must occur exactly once: {name}"
                );
            }
        }
        if profile == "claim_v2" {
            let mut narrowed_ctx = ctx.clone();
            let mut narrowed: Value = serde_json::from_slice(&config).unwrap();
            narrowed["allowed_field_ids"] = serde_json::json!(["claim.statement"]);
            narrowed_ctx.configuration_raw = serde_json::to_vec(&narrowed).unwrap();
            assert!(
                matches!(
                    run_form_command_from_captures(
                        &narrowed_ctx,
                        &cut,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        &cancel,
                    ),
                    Err(SourceCommandError::Denied(_))
                ),
                "field revocation applies before new Claim apply"
            );
        }
        if profile.is_empty() {
            // Preserve distinct request, local identity and immutable-read risks
            // on the actual consumer, without prototype-only error strings.
            let original: Value = serde_json::from_slice(&request).unwrap();
            for mutation in ["stale-source", "creator", "duplicate-form", "context"] {
                let mut bad = original.clone();
                match mutation {
                    "stale-source" => {
                        bad["expected_source"]["digest"] = serde_json::json!(
                            "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        )
                    }
                    "creator" => {
                        bad["changes"][0]["form"]["creator_id"] = serde_json::json!("impostor")
                    }
                    "duplicate-form" => {
                        bad["changes"][1]["form"]["form_id"] =
                            bad["changes"][0]["form"]["form_id"].clone()
                    }
                    "context" => {
                        let raw = serde_json::to_string(&bad).unwrap();
                        assert!(raw.contains("/identity_status"));
                        bad = serde_json::from_str(
                            &raw.replace("/identity_status", "/forged-context"),
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
                let mut bad_ctx = ctx.clone();
                bad_ctx.request_raw = serde_json::to_vec(&bad).unwrap();
                assert!(
                    run_form_command_from_captures(
                        &bad_ctx,
                        &cut,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        &cancel
                    )
                    .is_err(),
                    "actual request guard: {mutation}"
                );
            }
            let mut wrong_uid = ctx.clone();
            wrong_uid.effective_uid += 1;
            assert!(matches!(
                run_form_command_from_captures(
                    &wrong_uid,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel
                ),
                Err(SourceCommandError::Denied(_))
            ));
            for raw in [
                b"{\"schema_version\":\"x\",\"schema_version\":\"y\"}".to_vec(),
                vec![b' '; 1_048_577],
            ] {
                let mut malformed = ctx.clone();
                malformed.request_raw = raw;
                assert!(matches!(
                    run_form_command_from_captures(
                        &malformed,
                        &cut,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        &cancel
                    ),
                    Err(SourceCommandError::Invalid(_))
                ));
            }
        }
        if matches!(profile, "artifact-v1" | "artifact-v2" | "composite-v1") {
            let owner: Value = serde_json::from_slice(&config).unwrap();
            let source_path = owner["source_path"].as_str().unwrap();
            // Payload-law denials use the actual proposal engine and selected
            // schemas. Substituted payloads have no authenticated cut claim;
            // the separate capture wrapper must reject them as well.
            for mutation in ["recast", "private", "version"] {
                let mut substituted = ctx.clone();
                let input = substituted
                    .files
                    .iter_mut()
                    .find(|f| f.path.as_str() == source_path)
                    .unwrap();
                let mut source: Value = serde_json::from_slice(&input.raw).unwrap();
                match mutation {
                    "recast" => {
                        source["schema_version"] = serde_json::json!("tos_corpus_record_v1")
                    }
                    "private" => {
                        assert_eq!(source["authority"]["visibility"], "public_metadata_only");
                        source["authority"]["visibility"] = serde_json::json!("local_only");
                    }
                    "version" => {
                        source["record_version"] =
                            serde_json::json!(source["record_version"].as_u64().unwrap() + 1)
                    }
                    _ => unreachable!(),
                }
                input.raw = serde_json::to_vec(&source).unwrap();
                assert!(
                    run_form_command(&substituted, &cut, &mut worker, deadline, &cancel).is_err(),
                    "{profile}: payload owner law {mutation}"
                );
                assert!(matches!(
                    run_form_command_from_captures(
                        &substituted,
                        &cut,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        &cancel
                    ),
                    Err(SourceCommandError::Conflict(_))
                ));
            }
            let schema_path = match profile {
                "artifact-v1" => "ToS/contracts/artifact-source-witness.schema.json",
                "artifact-v2" => "ToS/contracts/artifact-source-witness-v2.schema.json",
                "composite-v1" => "ToS/contracts/scholarly-composite-witness.schema.json",
                _ => unreachable!(),
            };
            let mut drifted_schema = ctx.clone();
            drifted_schema
                .files
                .iter_mut()
                .find(|f| f.path.as_str() == schema_path)
                .unwrap()
                .raw
                .push(b'\n');
            assert!(matches!(
                run_form_command_from_captures(
                    &drifted_schema,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel
                ),
                Err(SourceCommandError::Conflict(_))
            ));
            let mut wrong_route = ctx.clone();
            let mut owner = owner.clone();
            owner["source_path"] = serde_json::json!(format!(
                "ToS/source-witnesses/works/native/{}",
                source_path.rsplit('/').next().unwrap()
            ));
            wrong_route.configuration_raw = serde_json::to_vec(&owner).unwrap();
            assert!(
                run_form_command_from_captures(
                    &wrong_route,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel
                )
                .is_err()
            );
        }
        let prepared = run_form_command_from_captures(
            &ctx,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel,
        )
        .unwrap();
        assert_eq!(prepared.changes.len(), 1, "{profile}");
        assert_form_oracle_response(
            &prepared.response,
            &packet.join(if independent_work || claim_profile {
                "apply.json"
            } else {
                "apply.response.json"
            }),
        );
        let raw = prepared.changes[0].after.as_ref().unwrap();
        assert_eq!(
            *raw,
            fs::read(packet.join(if independent_work || claim_profile {
                "published.json"
            } else {
                "form-set.published.json"
            }))
            .unwrap(),
            "{profile}: maintained exact Python publication bytes"
        );
        assert_eq!(
            prepared.commit(),
            Err(SourceCommandError::MissingProductionAdmission)
        );
        files.insert(target.clone(), raw.clone());
        let candidate = successor(&files, &root, base);
        let candidate_cut = open_cut(&root, candidate, deadline, &cancel);
        if profile.is_empty() {
            // Mutate an unchanged dependency and re-derive the public proposal
            // from the same substituted context. Self-equivalence cannot pass
            // the independently selected base's real byte/fixity checks.
            for selected_path in [
                "ToS/contracts/human-form.schema.json",
                component_path.as_str(),
            ] {
                let mut substituted = ctx.clone();
                substituted
                    .files
                    .iter_mut()
                    .find(|input| input.path.as_str() == selected_path)
                    .unwrap()
                    .raw
                    .push(b' ');
                let substituted_proposal = substituted
                    .plan(
                        &prepared.handler_id,
                        prepared.response.clone(),
                        prepared.changes.clone(),
                        prepared.replayed,
                    )
                    .unwrap();
                assert!(matches!(
                    bind_selected_candidate(
                        &substituted,
                        substituted_proposal,
                        &cut,
                        &software,
                        &components,
                        &candidate_cut,
                        RelativePath::parse("protected-owner/form-command.json").unwrap(),
                        OperationLimits {
                            max_member_bytes: 8_388_608,
                            max_total_bytes: 33_554_432,
                            max_state_bytes: 33_554_432,
                            max_reads: 8192,
                            max_changes: 64,
                            deadline
                        },
                        &cancel,
                    ),
                    Err(SourceOperationError::Command(SourceCommandError::Conflict(
                        _
                    )))
                ));
                assert!(matches!(
                    run_form_command_from_captures(
                        &substituted,
                        &cut,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        &cancel
                    ),
                    Err(SourceCommandError::Conflict(_))
                ));
            }
            let other_software = SoftwareCaptureReader::open(
                &other_capture.capture,
                &other_capture.restored,
                other_capture.selection.clone(),
                ReadLimits {
                    max_manifest_bytes: 1_048_576,
                    max_manifest_entries: 128,
                    max_selected_object_bytes: 8_388_608,
                    json: JsonLimits::default(),
                },
                deadline,
                &cancel,
            )
            .unwrap();
            let other_components = other_software
                .select_components(&[component_path.clone()])
                .unwrap();
            assert!(matches!(
                run_form_command_from_captures(
                    &ctx,
                    &cut,
                    &software,
                    &other_components,
                    &mut worker,
                    deadline,
                    &cancel,
                ),
                Err(SourceCommandError::Conflict(_))
            ));
            assert!(matches!(
                bind_selected_candidate(
                    &ctx,
                    prepared.clone(),
                    &cut,
                    &software,
                    &other_components,
                    &candidate_cut,
                    RelativePath::parse("protected-owner/form-command.json").unwrap(),
                    OperationLimits {
                        max_member_bytes: 8_388_608,
                        max_total_bytes: 33_554_432,
                        max_state_bytes: 33_554_432,
                        max_reads: 8192,
                        max_changes: 64,
                        deadline
                    },
                    &cancel,
                ),
                Err(SourceOperationError::Command(SourceCommandError::Conflict(
                    _
                )))
            ));
            assert!(
                run_form_command_from_captures(
                    &ctx,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    Instant::now(),
                    &cancel,
                )
                .is_err()
            );
            assert!(
                run_form_command_from_captures(
                    &ctx,
                    &cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &AtomicBool::new(true),
                )
                .is_err()
            );
        }
        let bound = bind_selected_candidate(
            &ctx,
            prepared,
            &cut,
            &software,
            &components,
            &candidate_cut,
            RelativePath::parse("protected-owner/form-command.json").unwrap(),
            OperationLimits {
                max_member_bytes: 8_388_608,
                max_total_bytes: 33_554_432,
                max_state_bytes: 33_554_432,
                max_reads: 8192,
                max_changes: 64,
                deadline,
            },
            &cancel,
        )
        .unwrap();
        assert_eq!(bound.binding().base_revision(), base);
        assert_eq!(bound.binding().candidate_revision(), candidate);
        assert!(matches!(
            bound.commit(),
            Err(SourceOperationError::MissingFullSourceAdmission)
        ));
        let mut replay_ctx = context(&files, config.clone(), request.clone(), candidate);
        replay_ctx.files.push(SourceFile {
            path: component_path.clone(),
            raw: software_raw.clone(),
        });
        let mut worker = schemas(&candidate_cut, deadline, &cancel);
        let replay = run_form_command_from_captures(
            &replay_ctx,
            &candidate_cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel,
        )
        .unwrap();
        assert!(replay.replayed);
        assert!(replay.changes.is_empty());
        assert_form_oracle_response(
            &replay.response,
            &packet.join(if independent_work || claim_profile {
                "replay.json"
            } else {
                "replay.response.json"
            }),
        );
        if profile.is_empty() {
            // Reusing a command ID for a different request must not replay.
            let mut reused = replay_ctx.clone();
            let mut changed: Value = serde_json::from_slice(&request).unwrap();
            changed["expected_revision"] = serde_json::json!(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            );
            reused.request_raw = serde_json::to_vec(&changed).unwrap();
            assert!(matches!(
                run_form_command_from_captures(
                    &reused,
                    &candidate_cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel
                ),
                Err(SourceCommandError::Conflict(_))
            ));
            // Even re-derived public context cannot replace retained bytes.
            let mut corrupted = replay_ctx.clone();
            let retained = corrupted
                .files
                .iter_mut()
                .find(|f| f.path.as_str() == target.as_str())
                .unwrap();
            let mut set: Value = serde_json::from_slice(&retained.raw).unwrap();
            set["prior_forms"] = serde_json::json!([]);
            retained.raw = serde_json::to_vec(&set).unwrap();
            assert!(
                run_form_command(&corrupted, &candidate_cut, &mut worker, deadline, &cancel)
                    .is_err(),
                "retained lineage must reconstruct, even before custody binding"
            );
            assert!(matches!(
                run_form_command_from_captures(
                    &corrupted,
                    &candidate_cut,
                    &software,
                    &components,
                    &mut worker,
                    deadline,
                    &cancel
                ),
                Err(SourceCommandError::Conflict(_))
            ));
        }
        if profile == "claim_v2" {
            let mut narrowed: Value = serde_json::from_slice(&config).unwrap();
            narrowed["allowed_field_ids"] = serde_json::json!(["claim.statement"]);
            let mut narrowed_ctx = context(
                &files,
                serde_json::to_vec(&narrowed).unwrap(),
                request.clone(),
                candidate,
            );
            narrowed_ctx.files.push(SourceFile {
                path: component_path.clone(),
                raw: software_raw.clone(),
            });
            assert!(
                matches!(
                    run_form_command_from_captures(
                        &narrowed_ctx,
                        &candidate_cut,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        &cancel,
                    ),
                    Err(SourceCommandError::Denied(_))
                ),
                "field revocation applies before Claim replay"
            );
        }
        // Current revocation must apply before historical receipt replay.
        let mut revoked: Value = serde_json::from_slice(&config).unwrap();
        revoked["allowed_operations"] = serde_json::json!([]);
        let mut revoked_ctx = context(
            &files,
            serde_json::to_vec(&revoked).unwrap(),
            request,
            candidate,
        );
        revoked_ctx.files.push(SourceFile {
            path: component_path.clone(),
            raw: software_raw.clone(),
        });
        assert!(matches!(
            run_form_command_from_captures(
                &revoked_ctx,
                &candidate_cut,
                &software,
                &components,
                &mut worker,
                deadline,
                &cancel
            ),
            Err(SourceCommandError::Denied(_))
        ));
    }
}

// Link-v1 retains its own schema and record identity. The existing public
// source-copy kernel already guards its URL/provider/observation context;
// Forms publishes only the adjacent carrier and never revises Link bytes.
#[test]
fn retained_link_v1_forms_prepare_apply_and_cold_replay_through_native_owner() {
    use std::os::unix::fs::PermissionsExt;
    use tos_command::source_creation_store::IsolatedCreationRoot;
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("OPS immutable native owner CLI"),
    );
    assert!(native.is_absolute());
    let worker = super::validation_cut_cases::selected_worker_path();
    let native_digest = super::command_text_cases::alignment_image_digest(&native);
    let worker_digest = super::command_text_cases::alignment_image_digest(&worker);
    let temporary = tempfile::tempdir().unwrap();
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
    let source_path =
        "ToS/source-witnesses/links/internet-archive/onfoursongsconta00good/landing/link.json";
    let target = source_path.strip_suffix(".json").unwrap().to_owned() + ".human-forms.json";
    let source_raw = fs::read(repository.join(source_path)).unwrap();
    let retained_raw = fs::read(repository.join(&target)).unwrap();
    let source: Value = serde_json::from_slice(&source_raw).unwrap();
    let retained: Value = serde_json::from_slice(&retained_raw).unwrap();
    assert_eq!(
        source["schema_version"],
        serde_json::json!("tos_source_link_v1")
    );
    let selected_form = retained["forms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| {
            form["content"]["kind"] == "source-copy"
                && form["bindings"]["wording"]["pointer"] == "/preferred_label"
        })
        .unwrap();
    let form_id = selected_form["form_id"].as_str().unwrap();
    let mut files = BTreeMap::from([
        (source_path.to_owned(), source_raw.clone()),
        (target.clone(), retained_raw.clone()),
    ]);
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            files.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    assert!(files.len() <= 2048 && files.values().map(Vec::len).sum::<usize>() <= 8 * 1024 * 1024);
    for (path, raw) in &files {
        let destination = isolated.path().join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(&destination, raw).unwrap();
        fs::set_permissions(destination, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let component = "rust/crates/tos-command/src/source_forms.rs";
    let component_raw = fs::read(repository.join(component)).unwrap();
    let destination = isolated.path().join(component);
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::write(&destination, &component_raw).unwrap();
    fs::set_permissions(destination, fs::Permissions::from_mode(0o644)).unwrap();
    let mut capture_inputs = files.clone();
    capture_inputs.insert(component.into(), component_raw);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&capture_inputs, deadline, &cancelled);
    let owner = temporary.path().join("link-form-owner.json");
    let config = serde_json::json!({"schema_version":"tos_local_source_command_owner_v1","uid":rustix::process::getuid().as_raw(),
        "principal_id":"model:retained-link-forms","source_root":isolated.path(),"source_path":source_path,
        "authority_ref":"test-only:retained-link-form-revision","allowed_form_ids":[form_id],
        "allowed_operations":["form.revise"],"expires_at":"2099-01-01T00:00:00Z"});
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let store = temporary.path().join("link-form-cut");
    let base = super::validation_cut_cases::write_cut_store(&files, &store);
    let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,
        "owner_context":null,"assessment_schema_worker":null,
        "native_executable":native,"native_executable_sha256":native_digest.to_prefixed(),
        "corpus_store":store,"source_revision":base.0.to_prefixed(),"original_source_revision":base.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":worker,"sha256":worker_digest.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("link-form-invocation.json");
    let freeze = |value: &Value| {
        fs::write(&invocation_path, serde_json::to_vec(value).unwrap()).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    };
    freeze(&invocation);
    let invoke = |request: &Value| -> Value {
        let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
            &repository,
            &owner,
            &invocation_path,
            request,
            deadline,
        );
        assert!(
            status.success(),
            "Link-v1 native Forms: {}",
            String::from_utf8_lossy(&errors)
        );
        let result: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(result["grants_admission"], serde_json::json!(false));
        result["result"].clone()
    };
    let preview = invoke(
        &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare","form_id":form_id,"field_id":"metadata.preferred-name"}),
    );
    assert_eq!(preview["source"]["id"], source["record_id"]);
    assert_eq!(preview["source"]["version"], source["record_version"]);
    assert_eq!(
        preview["prepared_materialization"]["state"],
        serde_json::json!("ready")
    );
    let pointers = preview["prepared_change"]["form"]["bindings"]
        .as_object()
        .unwrap()
        .values()
        .map(|binding| binding["pointer"].as_str().unwrap())
        .collect::<Vec<_>>();
    for pointer in [
        "/uri",
        "/provider_label",
        "/link_kind",
        "/access_status",
        "/observed_at",
        "/observation_ref",
        "/association_claim_refs",
        "/provenance_event_ref",
    ] {
        assert!(pointers.contains(&pointer));
    }
    assert_eq!(
        fs::read(isolated.path().join(&target)).unwrap(),
        retained_raw
    );
    let request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"apply","command_id":"retained-link-native-form-revise",
        "expected_source":preview["source"],"expected_revision":preview["revision"],"expected_configuration":preview["owner_configuration"],"changes":[preview["prepared_change"]]});
    let applied = invoke(&request);
    assert_eq!(applied["replayed"], serde_json::json!(false));
    let after_raw = fs::read(isolated.path().join(&target)).unwrap();
    let after: Value = serde_json::from_slice(&after_raw).unwrap();
    assert!(
        after["prior_forms"]
            .as_array()
            .unwrap()
            .contains(selected_form)
    );
    assert_eq!(
        fs::read(isolated.path().join(source_path)).unwrap(),
        source_raw
    );
    files.insert(target.clone(), after_raw.clone());
    // The publication carrier is source transport, not a semantic revision.
    let publication = "ToS/source-witnesses/.metadata-publication.json";
    files.insert(
        publication.into(),
        fs::read(isolated.path().join(publication)).unwrap(),
    );
    let current = super::validation_cut_cases::write_cut_store_on_base(&files, &store, Some(base));
    invocation["source_revision"] = serde_json::json!(current.0.to_prefixed());
    freeze(&invocation);
    assert_eq!(invoke(&request)["replayed"], serde_json::json!(true));
    assert_eq!(fs::read(isolated.path().join(&target)).unwrap(), after_raw);
    assert_eq!(
        fs::read(isolated.path().join(source_path)).unwrap(),
        source_raw
    );
}
