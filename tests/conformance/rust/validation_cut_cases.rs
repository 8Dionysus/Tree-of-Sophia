//! One composed source-carrier/worker/Item contract case. Fixture selection is
//! explicit and smaller than the full authored universe; no admission follows.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_source_store::CutReadLimits;
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::item_rules::ItemLimits;
use tos_validation::operation::{
    GeneralOperationLimits, OperationChange, OperationFamilyScope, OperationFamilyState,
    OperationLimits, OperationProposal, OperationRefusal, bind_operation_from_cut,
    inspect_general_operation, inspect_item_operation,
};
use tos_validation::record_rules::{RecordFamily, RecordSchema};
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor, MetadataOnlyPayloads};

const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const ENTITY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const ITEM: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3/items/dta-sbb-corrected-tei-p5";

pub(super) fn repository() -> PathBuf {
    fixtures().join("../../..")
}

fn selected_item_sources() -> BTreeMap<String, Vec<u8>> {
    let root = repository();
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            files.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    for name in [
        "item.manifest.json",
        "item.json",
        "rights.json",
        "provenance.jsonl",
        "resource-inventory.json",
        "fixity.sha256",
        "forensic-report.md",
        "source-metadata-snapshot.json",
    ] {
        let path = format!("{ITEM}/{name}");
        files.insert(path.clone(), fs::read(root.join(path)).unwrap());
    }
    for path in [
        REGISTRY,
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3/edition.json",
        "ToS/research-packets/foundation-laboratory-2026-07/DTA_ZARATHUSTRA_PARTS_1_4_LAYERED_RIGHTS_ASSESSMENT.md",
    ] {
        files.insert(path.into(), fs::read(root.join(path)).unwrap());
    }
    files
}

// The corpus carrier is disposable transport. These actual source bytes and
// empty routing claims do not certify source enumeration or semantic identity.
pub(super) fn write_cut_store(files: &BTreeMap<String, Vec<u8>>, root: &Path) -> SourceRevision {
    write_cut_store_on_base(files, root, None)
}

fn write_cut_store_on_base(
    files: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    base: Option<SourceRevision>,
) -> SourceRevision {
    write_cut_store_with_optional_modes(files, root, base, None)
}

pub(super) fn write_cut_store_with_modes(
    files: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    modes: &BTreeMap<String, u32>,
) -> SourceRevision {
    assert!(files.keys().eq(modes.keys()));
    write_cut_store_with_optional_modes(files, root, None, Some(modes))
}

fn write_cut_store_with_optional_modes(
    files: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    base: Option<SourceRevision>,
    modes: Option<&BTreeMap<String, u32>>,
) -> SourceRevision {
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("revisions")).unwrap();
    let members: Vec<_> = files
        .iter()
        .map(|(path, raw)| {
            let sha = Digest256::of_bytes(raw).to_hex();
            fs::write(root.join("objects").join(&sha), raw).unwrap();
            let mode = modes.map_or(0o644, |values| values[path]);
            serde_json::json!({"path":path,"sha256":sha,"size_bytes":raw.len(),"mode":mode})
        })
        .collect();
    let mut manifest = serde_json::json!({"schema_version":"tos_corpus_snapshot_v1",
        "base_revision":base.map(|revision| revision.0.to_hex()),"files":members,"identities":{},"dependencies":{},
        "retirements":[],"validator_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"});
    let revision = SourceRevision(Digest256::of_bytes(&canonical_json(&manifest)));
    manifest["revision"] = Value::String(revision.0.to_hex());
    let dir = root.join("revisions").join(revision.0.to_hex());
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("snapshot.json"), canonical_json(&manifest)).unwrap();
    revision
}

fn record_routes(files: &BTreeMap<String, Vec<u8>>) -> RecordFamily {
    let registry: Value = serde_json::from_slice(&files[REGISTRY]).unwrap();
    let mut closure: BTreeSet<String> = [
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/source-link.schema.json",
        "ToS/contracts/artifact-source-witness.schema.json",
        "ToS/contracts/artifact-source-witness-v2.schema.json",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for entry in registry["types"].as_array().unwrap() {
        if let Some(routes) = entry["source_record_profile"]["schemas"].as_array() {
            for route in routes {
                closure.insert(required(route, "schema_ref").into());
                if let Some(dependencies) = route["schema_dependencies"].as_array() {
                    closure.extend(
                        dependencies
                            .iter()
                            .map(|path| path.as_str().unwrap().to_owned()),
                    );
                }
            }
        }
    }
    RecordFamily::new(
        &files[REGISTRY],
        &files[ENTITY_SCHEMA],
        closure.iter().map(|path| RecordSchema {
            path,
            raw: &files[path],
        }),
        FormatProfile::LegacyPythonObserved20260923,
    )
    .unwrap()
}

// Only explicitly selected executable or Cargo target custody; no default
// target/path search and no silent skip in a workspace validation lane.
pub(super) fn selected_worker_path() -> PathBuf {
    let path = if let Some(path) = std::env::var_os("TOS_SCHEMA_WORKER_PATH") {
        PathBuf::from(path)
    } else {
        let target = PathBuf::from(
            std::env::var_os("CARGO_TARGET_DIR")
                .expect("OPS must supply TOS_SCHEMA_WORKER_PATH or absolute CARGO_TARGET_DIR"),
        );
        assert!(
            target.is_absolute(),
            "selected Cargo target must be absolute"
        );
        target.join("debug/tos-schema-worker")
    };
    assert!(path.is_absolute(), "selected worker path must be absolute");
    assert!(
        path.is_file(),
        "selected worker must be built before conformance"
    );
    path
}

#[test]
fn actual_cut_worker_and_item_companions_preserve_metadata_only_outcome() {
    let before = selected_item_sources();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    let base = write_cut_store(&before, &root);
    let mut files = before.clone();
    let changed_path = format!("{ITEM}/forensic-report.md");
    files.get_mut(&changed_path).unwrap().push(b'\n');
    let revision = write_cut_store_on_base(&files, &root, Some(base));
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2_097_152,
        json: JsonLimits::default(),
    };
    let reader = CorpusReader::open_existing(&root, limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 1024,
                max_total_bytes: 16_777_216,
                max_member_bytes: 2_097_152,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    // OPS supplies the separately built exact worker; absence is a failure.
    let worker_path = selected_worker_path();
    assert!(worker_path.is_absolute());
    let worker_digest = Digest256::of_bytes(&fs::read(&worker_path).unwrap());
    let mut schemas = CutWorkerSchemaExecutor::from_cut(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            absolute_path: worker_path,
            sha256: worker_digest,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 131_072,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let request_raw = br#"{"operation":"fixture-item-metadata"}"#.to_vec();
    let configuration_raw = br#"{"owner":"private-fixture","scope":"item-companions"}"#.to_vec();
    let canonical_digest = |raw: &[u8]| {
        Digest256::of_bytes(
            &tos_foundation::canonical_raw_bytes_v1(
                raw,
                tos_foundation::CanonicalProfile::SourceCommandInputV1,
                JsonLimits::default(),
            )
            .unwrap(),
        )
    };
    let proposal = OperationProposal {
        handler_id: "fixture-item-handler".into(),
        operation: "fixture-item-metadata".into(),
        base_revision: base,
        candidate_revision: revision,
        request_canonical_sha256: canonical_digest(&request_raw),
        request_raw,
        // This protected namespace is deliberately absent from authored members.
        configuration_path: RelativePath::parse("protected/owner-operation.json").unwrap(),
        configuration_raw_sha256: Digest256::of_bytes(&configuration_raw),
        configuration_canonical_sha256: canonical_digest(&configuration_raw),
        configuration_raw,
        changes: vec![OperationChange {
            path: RelativePath::parse(&changed_path).unwrap(),
            before: Some(Digest256::of_bytes(&before[&changed_path])),
            after: Some(Digest256::of_bytes(&files[&changed_path])),
        }],
    };
    let operation_limits = OperationLimits {
        max_member_bytes: 2_097_152,
        max_total_bytes: 16_777_216,
        max_state_bytes: 4_194_304,
        max_reads: 2048,
        max_changes: 128,
        deadline,
    };
    // Reuse the selected carrier; no schema worker runs for these rejected
    // exact-proposal controls. Each protects a distinct command boundary.
    let mut control = proposal.clone();
    control.changes.clear();
    assert!(matches!(
        bind_operation_from_cut(&cut, &control, operation_limits, &cancelled),
        Err(OperationRefusal::InvalidProposal(
            "undeclared source change"
        ))
    ));
    control = proposal.clone();
    control.changes[0].before = None;
    assert!(matches!(
        bind_operation_from_cut(&cut, &control, operation_limits, &cancelled),
        Err(OperationRefusal::InvalidProposal(
            "source change digest mismatch"
        ))
    ));
    control = proposal.clone();
    control.configuration_raw.push(b' ');
    assert!(matches!(
        bind_operation_from_cut(&cut, &control, operation_limits, &cancelled),
        Err(OperationRefusal::InvalidProposal(
            "request or configuration digest mismatch"
        ))
    ));
    control = proposal.clone();
    control.changes.push(control.changes[0].clone());
    assert!(matches!(
        bind_operation_from_cut(&cut, &control, operation_limits, &cancelled),
        Err(OperationRefusal::InvalidProposal(
            "duplicate or empty change"
        ))
    ));
    let report = inspect_item_operation(
        &cut,
        &proposal,
        operation_limits,
        ItemLimits {
            max_member_bytes: 1_048_576,
            max_total_bytes: 16_777_216,
            max_state_bytes: 8_388_608,
            max_issues: 128,
            deadline,
        },
        false,
        &cancelled,
        &record_routes(&files),
        &mut schemas,
        &mut MetadataOnlyPayloads,
    )
    .unwrap();
    assert!(
        report.item_family().unwrap().issues.is_empty(),
        "{:?}",
        report.item_family().unwrap().issues
    );
    assert_eq!(report.item_family().unwrap().unavailable_payloads, 1);
    assert!(!report.item_family().unwrap().source_admission_complete);
    assert_eq!(
        report.binding().candidate_carrier().count,
        files.len() as u64
    );
    assert_eq!(report.scope(), OperationFamilyScope::ItemCompanions);
    assert_eq!(report.state(), &OperationFamilyState::MechanicsComplete);
    assert!(!report.general_source_missing_rules().is_empty());
    assert_eq!(report.binding().base_revision(), base);
    assert_eq!(
        report.binding().configuration().raw_sha256,
        proposal.configuration_raw_sha256
    );
    assert_eq!(report.worker().source_revision, revision);
    assert_eq!(report.schema_receipts().len(), schemas.receipts().len());
    assert!(!schemas.receipts().is_empty());
    for receipt in schemas.receipts() {
        assert_eq!(receipt.source_revision, revision);
        let selected_raw = if let Some(raw) = files.get(&receipt.path) {
            raw.as_slice()
        } else {
            let (path, ordinal) = receipt
                .path
                .rsplit_once(':')
                .expect("only a selected JSONL row may have a logical receipt path");
            assert_eq!(path, format!("{ITEM}/provenance.jsonl"));
            let ordinal: usize = ordinal.parse().unwrap();
            assert!(ordinal > 0);
            files[path]
                .split(|byte| *byte == b'\n')
                .nth(ordinal - 1)
                .expect("receipt must identify an existing selected provenance row")
        };
        assert_eq!(receipt.source_raw_sha256, Digest256::of_bytes(selected_raw));
        assert_eq!(receipt.execution.worker_sha256, worker_digest);
        assert_eq!(
            receipt.execution.instance_sha256,
            receipt.decoded_instance_sha256
        );
        assert!(receipt.valid);
    }
}

#[test]
fn actual_cut_worker_and_pinned_software_preserve_provenance_lab_limits() {
    use tos_source_store::SoftwareCaptureReader;
    use tos_validation::provenance_rules::LAB_MANIFEST;
    use tos_validation::source_cut::inspect_provenance_lab_from_cut;
    let root = repository().canonicalize().unwrap();
    let commit_output = std::process::Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .unwrap();
    assert!(commit_output.status.success());
    let commit = String::from_utf8(commit_output.stdout)
        .unwrap()
        .trim()
        .to_owned();
    assert_eq!(commit.len(), 40);
    assert!(commit.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let capture = super::source_cut_cases::captured_software_fixture(
        &root,
        &commit,
        &[
            "scripts/build_provenance_event_v2_lab.py",
            "rust/crates/tos-validation/src/source_cut.rs",
        ],
    );
    let mut files = selected_item_sources();
    let lab = LAB_MANIFEST.rsplit_once('/').unwrap().0;
    for entry in fs::read_dir(root.join(lab)).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            files.insert(
                format!("{lab}/{}", entry.file_name().to_str().unwrap()),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    let manifest: Value = serde_json::from_slice(&files[LAB_MANIFEST]).unwrap();
    // Owner-defined archived paths preserve original recorded inputs while
    // the separately pinned software capture remains the current builder.
    for field in ["contract", "builder"] {
        let digest = required(&manifest[field], "sha256");
        let archive = if field == "contract" {
            format!("ToS/contracts/history/{digest}.json")
        } else {
            format!(
                "ToS/research-packets/retained-builder-inputs/build_provenance_event_v2_lab/{digest}.py"
            )
        };
        let path = root.join(&archive);
        if path.is_file() {
            files.insert(archive, fs::read(path).unwrap());
        }
    }
    let temporary = tempfile::tempdir().unwrap();
    let store = temporary.path().join("store");
    let revision = write_cut_store(&files, &store);
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let read = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2_097_152,
        json: JsonLimits::default(),
    };
    let reader = CorpusReader::open_existing(&store, read).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 512,
                max_total_bytes: 8_388_608,
                max_member_bytes: 2_097_152,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let software = SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        read,
        deadline,
        &cancelled,
    )
    .unwrap();
    let unselected = software
        .read_current(
            &RelativePath::parse("scripts/validate_source_witness_foundation.py").unwrap(),
            1_048_576,
            deadline,
            &cancelled,
        )
        .unwrap_err();
    assert_eq!(
        unselected.code,
        StoreErrorCode::UnsupportedFormat,
        "an uncaptured software owner path is not proven absent"
    );
    let worker_path = selected_worker_path();
    assert!(worker_path.is_absolute());
    let digest = Digest256::of_bytes(&fs::read(&worker_path).unwrap());
    let mut schemas = CutWorkerSchemaExecutor::from_cut(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            absolute_path: worker_path,
            sha256: digest,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 131_072,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    {
        use tos_validation::provenance_rules::ProvenanceSource;
        use tos_validation::source_cut::CutProvenanceSource;
        let component_path =
            RelativePath::parse("rust/crates/tos-validation/src/source_cut.rs").unwrap();
        // Capture membership alone does not enable arbitrary software reads.
        assert!(
            software
                .read_current(&component_path, 1_048_576, deadline, &cancelled)
                .unwrap()
                .is_none()
        );
        let components = software
            .select_components(&[component_path.clone()])
            .unwrap();
        let mut source = CutProvenanceSource {
            cut: &cut,
            software: &software,
            components: None,
            schemas: &mut schemas,
            cancelled: &cancelled,
        };
        assert!(matches!(
            source.current(
                "scripts/validate_source_witness_foundation.py",
                1_048_576,
                deadline
            ),
            Err(tos_validation::item_rules::ItemRefusal::Unsupported(_))
        ));
        assert!(matches!(
            source.current(component_path.as_str(), 1_048_576, deadline),
            Err(tos_validation::item_rules::ItemRefusal::Unsupported(_))
        ));
        source.components = Some(&components);
        let raw = source
            .current(component_path.as_str(), 1_048_576, deadline)
            .unwrap()
            .unwrap();
        assert_eq!(
            Digest256::of_bytes(&raw),
            components.member(&component_path).unwrap().sha256
        );
        assert_eq!(
            source
                .recorded_input(
                    component_path.as_str(),
                    &Digest256::of_bytes(&raw).to_hex(),
                    1_048_576,
                    deadline
                )
                .unwrap(),
            Some(raw)
        );
        assert!(
            source
                .recorded_input(
                    component_path.as_str(),
                    &"0".repeat(64),
                    1_048_576,
                    deadline
                )
                .unwrap()
                .is_none()
        );
    }
    let report = inspect_provenance_lab_from_cut(
        &cut,
        &software,
        ItemLimits {
            max_member_bytes: 1_048_576,
            max_total_bytes: 16_777_216,
            max_state_bytes: 8_388_608,
            max_issues: 128,
            deadline,
        },
        &cancelled,
        &mut schemas,
    )
    .unwrap();
    assert_eq!(report.source_revision, revision);
    assert_eq!(report.software_selection, capture.selection);
    assert!(
        report.provenance_family.issues.is_empty(),
        "{:?}",
        report.provenance_family.issues
    );
    assert!(!report.provenance_family.source_admission_complete);
    assert!(
        report
            .provenance_family
            .negative_controls
            .values()
            .all(|passed| *passed)
    );
    assert!(!schemas.receipts().is_empty());
    assert!(
        schemas
            .receipts()
            .iter()
            .all(|receipt| receipt.execution.worker_sha256 == digest
                && receipt.source_revision == revision)
    );
    assert_eq!(
        schemas
            .receipts()
            .iter()
            .filter(|receipt| receipt.valid)
            .count(),
        3
    );
    assert_eq!(
        schemas
            .receipts()
            .iter()
            .filter(|receipt| !receipt.valid)
            .count(),
        6
    );
}

#[test]
fn actual_general_operation_keeps_selected_family_coverage_below_source_admission() {
    use tos_validation::record_biblio_cut::BiblioRecordExecutor;
    use tos_validation::source_cut::CutSchemaExecutor;
    let mut before = selected_item_sources();
    let owner = repository();
    let relation = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
    before.insert(relation.into(), fs::read(owner.join(relation)).unwrap());
    // Existing maintained owner examples; this deliberately selected fixture
    // has no accepted full-source membership or complete bibliographic graph.
    // Source omissions must remain observable in the composed result.
    let ladder = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/initial-sign-packet.v5.json";
    let crosswalk = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/transfer-candidate-page-crosswalk.v1.json";
    for path in [ladder, crosswalk] {
        before.insert(path.into(), fs::read(owner.join(path)).unwrap());
    }
    let packet: Value = serde_json::from_slice(&before[crosswalk]).unwrap();
    for input in packet["inputs"].as_object().unwrap().values() {
        let path = required(input, "ref");
        let raw = fs::read(owner.join(path)).unwrap();
        assert_eq!(
            Digest256::of_bytes(&raw).to_hex(),
            required(input, "sha256")
        );
        before.insert(path.into(), raw);
    }
    // Reuse the maintained native compound fixture's real isolated writer.
    // Two siblings at each consumed tier exercise parent archive prefixes;
    // descriptive Expression lineage and exact older commitments survive.
    let oracle=std::process::Command::new("python3").arg("-c").arg(r#"
import copy,json,sys
from pathlib import Path
root=Path(sys.argv[1])
sys.path.insert(0,str(root/'mechanics/growth-cycle/tests'))
from test_source_item_commands import NativeItemTests,commands,item,edition_fixture
c=NativeItemTests();c.setUp()
try:
    c.origin.origin.correct_expression();c.rebuild()
    c.origin.origin.select_child('second');second_work=c.origin.origin.request()
    commands.run_local_command(c.origin.origin.owner,second_work)
    work_child=Path(c.origin.origin.config['expression_source_path'])
    c.origin.extra_records.append(c.root/work_child)
    c.origin.extra_claims.append((c.root/work_child).with_name('source-claims.jsonl'));c.rebuild()
    c.origin.select_child('second');second_edition=c.origin.request()
    commands.run_local_command(c.origin.owner,second_edition);c.rebuild()
    first=c.request();commands.run_local_command(c.owner,first);c.rebuild()
    c.select_item('second');second=c.request();commands.run_local_command(c.owner,second);c.rebuild()
    for request in (c.origin.origin_request,second_work):
        child=request['claim']['evidence_refs'][1]
        edition_fixture.expression_fixture.compound.verify_compound(c.root,str(Path(child).with_name('source-claims.jsonl')),request['claim'])
    for request in (c.edition_request,second_edition):
        child=request['claim']['evidence_refs'][1]
        edition_fixture.edition.verify_compound(c.root,str(Path(child).with_name('source-claims.jsonl')),request['claim'])
    for request in (first,second):
        child=request['claim']['evidence_refs'][1]
        item.verify_compound(c.root,str(Path(child).with_name('source-claims.jsonl')),request['claim'])
    files={p.relative_to(c.root).as_posix():p.read_bytes().hex()
           for p in sorted((c.root/'ToS').rglob('*')) if p.is_file()}
    print(json.dumps(files))
finally:c.doCleanups()
"#).arg(&owner).env("PYTHONDONTWRITEBYTECODE","1").output().unwrap();
    assert!(
        oracle.status.success(),
        "maintained compound oracle: {}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let compound_files: Value = serde_json::from_slice(&oracle.stdout).unwrap();
    // The maintained preparation oracle rebuilds its generated catalog in the
    // same directory. Keep those bytes available to that oracle, but use the
    // source-store's authored carrier law for both immutable source revisions.
    // All retained control/transaction/archive and human-form bytes stay in
    // the selected cut; generated catalogs are not read by this verifier.
    let mut generated_catalog_files = Vec::new();
    for (path, hex) in compound_files.as_object().unwrap() {
        if !tos_source_store::is_authored_source_path_v1(path) {
            assert!(
                path.starts_with("ToS/source-witnesses/catalog/")
                    && (path.ends_with(".json") || path.ends_with(".jsonl")),
                "unexpected non-authored fixture member {path}"
            );
            generated_catalog_files.push(path.as_str());
            continue;
        }
        let hex = hex.as_str().unwrap();
        assert_eq!(hex.len() % 2, 0);
        let raw = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        if let Some(existing) = before.get(path) {
            assert_eq!(existing, &raw, "selected common source {path}");
        }
        before.insert(path.clone(), raw);
    }
    assert!(
        !generated_catalog_files.is_empty(),
        "maintained oracle generated catalog present"
    );
    assert!(
        before
            .keys()
            .all(|path| tos_source_store::is_authored_source_path_v1(path)),
        "base/current cut contain only authored source members"
    );
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    let base = write_cut_store(&before, &root);
    let mut files = before.clone();
    let changed_path = format!("{ITEM}/forensic-report.md");
    files.get_mut(&changed_path).unwrap().push(b'\n');
    let revision = write_cut_store_on_base(&files, &root, Some(base));
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(120);
    let read = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 1024,
        max_selected_object_bytes: 2_097_152,
        json: JsonLimits::default(),
    };
    let reader = CorpusReader::open_existing(&root, read).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 2048,
                max_total_bytes: 32_000_000,
                max_member_bytes: 2_097_152,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let worker_path = selected_worker_path();
    let worker = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    let schema_limits = CutWorkerLimits {
        max_receipts: 256,
        max_receipt_bytes: 262_144,
    };
    let mut schemas = CutWorkerSchemaExecutor::from_cut(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        worker.clone(),
        ExecutorBudget::laboratory(),
        schema_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut record_executor = BiblioRecordExecutor::new(
        worker,
        ExecutorBudget::laboratory(),
        FormatProfile::LegacyPythonObserved20260923,
        256,
    );
    let request_raw = br#"{"operation":"fixture-general-metadata"}"#.to_vec();
    let configuration_raw = br#"{"owner":"private-fixture","scope":"general-mechanics"}"#.to_vec();
    let canonical_digest = |raw: &[u8]| {
        Digest256::of_bytes(
            &tos_foundation::canonical_raw_bytes_v1(
                raw,
                tos_foundation::CanonicalProfile::SourceCommandInputV1,
                JsonLimits::default(),
            )
            .unwrap(),
        )
    };
    let proposal = OperationProposal {
        handler_id: "fixture-general-handler".into(),
        operation: "fixture-general-metadata".into(),
        base_revision: base,
        candidate_revision: revision,
        request_canonical_sha256: canonical_digest(&request_raw),
        request_raw,
        configuration_path: RelativePath::parse("protected/owner-operation.json").unwrap(),
        configuration_raw_sha256: Digest256::of_bytes(&configuration_raw),
        configuration_canonical_sha256: canonical_digest(&configuration_raw),
        configuration_raw,
        changes: vec![OperationChange {
            path: RelativePath::parse(&changed_path).unwrap(),
            before: Some(Digest256::of_bytes(&before[&changed_path])),
            after: Some(Digest256::of_bytes(&files[&changed_path])),
        }],
    };
    let composed_limits = GeneralOperationLimits {
        operation: OperationLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 32_000_000,
            max_state_bytes: 4_194_304,
            max_reads: 4096,
            max_changes: 128,
            deadline,
        },
        family: ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline,
        },
        max_composed_state_bytes: 134_217_728,
        max_composed_read_bytes: 500_000_000,
        max_composed_schema_cpu_seconds: 2 * ExecutorBudget::laboratory().cpu_seconds,
        max_composed_schema_wire_bytes: 2
            * tos_validation::executor::BatchStreamBudget::laboratory().max_total_wire_bytes,
    };
    // Both already selected envelopes must fit before either child starts.
    for refused in [
        GeneralOperationLimits {
            max_composed_schema_cpu_seconds: composed_limits.max_composed_schema_cpu_seconds - 1,
            ..composed_limits
        },
        GeneralOperationLimits {
            max_composed_schema_wire_bytes: composed_limits.max_composed_schema_wire_bytes - 1,
            ..composed_limits
        },
    ] {
        assert!(matches!(
            inspect_general_operation(
                &cut,
                &proposal,
                refused,
                &cancelled,
                &record_routes(&files),
                &mut record_executor,
                &mut schemas,
                &mut MetadataOnlyPayloads,
                false
            ),
            Err(OperationRefusal::BudgetCheck { .. })
        ));
        assert!(schemas.receipts().is_empty());
    }
    let report = inspect_general_operation(
        &cut,
        &proposal,
        composed_limits,
        &cancelled,
        &record_routes(&files),
        &mut record_executor,
        &mut schemas,
        &mut MetadataOnlyPayloads,
        false,
    )
    .unwrap_or_else(|error| {
        let receipts = schemas.receipts();
        let locator_bytes = receipts.iter().map(|receipt| receipt.path.len() + receipt.contract.len()).sum::<usize>();
        let last = receipts.last().map(|receipt| (receipt.path.as_str(), receipt.contract.as_str()));
        panic!("actual General refusal {error:?}; schema receipts {}/{}, retained locator bytes {locator_bytes}, declared receipt-byte cap {}, last supplied locator/root {last:?}",
            receipts.len(), schema_limits.max_receipts, schema_limits.max_receipt_bytes);
    });
    assert_eq!(
        report.operation().scope(),
        OperationFamilyScope::GeneralSource
    );
    assert!(matches!(
        report.operation().state(),
        OperationFamilyState::Rejected { .. } | OperationFamilyState::MissingRules { .. }
    ));
    assert!(!report.operation().general_source_missing_rules().is_empty());
    assert_eq!(
        report.records.current_membership,
        report.operation().binding().candidate_carrier()
    );
    assert_eq!(report.records.retained_memberships.len(), 1);
    assert_eq!(
        report.source_shapes.carrier_membership,
        report.operation().binding().candidate_carrier()
    );
    assert!(report.source_shapes.checked_instances > 0);
    assert_eq!(report.retirement.revision, revision);
    assert_eq!(report.retirement.base_revision, Some(base));
    assert!(!report.retirement.source_admission_complete);
    assert!(report.retirement.membership_transition.is_none());
    assert!(report.rights.rights_record_count > 0);
    assert!(!report.rights.missing_authority.is_empty());
    assert!(
        report
            .layers
            .layer_family
            .checked_predicates
            .iter()
            .any(|(path, predicate)| path == ladder
                && predicate == "_semantic_ladder_identity_issues/v4")
    );
    assert!(
        report
            .layers
            .layer_family
            .checked_predicates
            .iter()
            .any(|(path, predicate)| path == crosswalk
                && predicate.starts_with("_transfer_candidate_crosswalk_issues/v1"))
    );
    assert!(!report.operation().schema_receipts().is_empty());
    assert!(
        report
            .operation()
            .schema_receipts()
            .iter()
            .all(|receipt| receipt.source_revision == revision)
    );
    assert_eq!(
        report.bibliography.native_compounds.len(),
        6,
        "native compound coverage: observations {:?}; bibliography issues {:?}; checked {:?}; skipped {:?}",
        report.bibliography.native_compounds,
        report.bibliography.shadow.issues,
        report.bibliography.shadow.checked_profiles,
        report.bibliography.shadow.skipped_profiles
    );
    assert!(
        report
            .bibliography
            .native_compounds
            .iter()
            .all(|observed| observed.transport
                == tos_validation::native_compound::NativeTransportState::Committed)
    );
    assert!(
        report
            .bibliography
            .shadow
            .checked_profiles
            .contains("native-work-expression-exact-compound-plan-and-current-lineage@1")
    );
    assert_eq!(
        report
            .bibliography
            .native_compounds
            .iter()
            .filter(|observed| !observed.claim_path.contains("/editions/"))
            .count(),
        2
    );
    assert!(
        report
            .bibliography
            .shadow
            .checked_profiles
            .contains("native-expression-edition-exact-compound-plan-and-current-lineage@1")
    );
    assert!(
        report
            .bibliography
            .shadow
            .checked_profiles
            .contains("native-edition-item-exact-compound-plan-and-current-lineage@1")
    );
    assert_eq!(
        report
            .bibliography
            .native_compounds
            .iter()
            .filter(|observed| observed.claim_path.contains("/items/"))
            .count(),
        2
    );
    assert_eq!(
        report
            .bibliography
            .native_compounds
            .iter()
            .filter(|observed| observed.claim_path.contains("/editions/")
                && !observed.claim_path.contains("/items/"))
            .count(),
        2
    );
    assert!(
        !report
            .bibliography
            .shadow
            .issues
            .iter()
            .any(|issue| matches!(
                issue.code,
                "native-work-expression-compound-evidence"
                    | "native-expression-edition-compound-evidence"
                    | "native-edition-item-compound-evidence"
            ))
    );

    // Equal decoded JSON is insufficient: retained publication binds exact
    // receipt bytes. Inspect the changed current cut through the same actual
    // record/bibliographic route, not a caller-issued transport observation.
    let origin = report
        .bibliography
        .native_compounds
        .iter()
        .find(|observed| {
            !observed.claim_path.contains("/editions/") && !observed.claim_path.contains("/items/")
        })
        .unwrap();
    let receipt_path = origin
        .claim_path
        .strip_suffix("source-claims.jsonl")
        .unwrap()
        .to_owned()
        + "work-expression-receipt.json";
    let edition = report
        .bibliography
        .native_compounds
        .iter()
        .find(|observed| {
            observed.claim_path.contains("/editions/") && !observed.claim_path.contains("/items/")
        })
        .unwrap();
    let edition_path = edition
        .claim_path
        .strip_suffix("source-claims.jsonl")
        .unwrap()
        .to_owned()
        + "edition.json";
    let mut rewritten: Value = serde_json::from_slice(&files[&edition_path]).unwrap();
    rewritten["notes"] = Value::String("Unretained rewrite".into());
    let mut rewritten_raw = serde_json::to_vec(&rewritten).unwrap();
    rewritten_raw.push(b'\n');
    let item = report
        .bibliography
        .native_compounds
        .iter()
        .find(|observed| observed.claim_path.contains("/items/"))
        .unwrap();
    let fixity_path = item
        .claim_path
        .strip_suffix("source-claims.jsonl")
        .unwrap()
        .to_owned()
        + "fixity.sha256";
    for (target, raw, claim_path, code) in [
        (
            receipt_path.clone(),
            {
                let mut raw = files[&receipt_path].clone();
                raw.push(b'\n');
                raw
            },
            origin.claim_path.as_str(),
            "native-work-expression-compound-evidence",
        ),
        (
            edition_path,
            rewritten_raw,
            edition.claim_path.as_str(),
            "native-expression-edition-compound-evidence",
        ),
        (
            fixity_path.clone(),
            {
                let mut raw = files[&fixity_path].clone();
                raw.push(b'\n');
                raw
            },
            item.claim_path.as_str(),
            "native-edition-item-compound-evidence",
        ),
    ] {
        let mut damaged = files.clone();
        damaged.insert(target, raw);
        let damaged_revision = write_cut_store_on_base(&damaged, &root, Some(revision));
        let negative_deadline = Instant::now() + Duration::from_secs(120);
        let damaged_cut = reader
            .open_source_cut(
                damaged_revision,
                CutReadLimits {
                    max_revisions: 4,
                    max_members: 2048,
                    max_total_bytes: 32_000_000,
                    max_member_bytes: 2_097_152,
                },
                negative_deadline,
                &cancelled,
            )
            .unwrap();
        let negative_limits = ItemLimits {
            deadline: negative_deadline,
            ..composed_limits.family
        };
        let mut negative_records = BiblioRecordExecutor::new(
            record_executor.worker.clone(),
            ExecutorBudget::laboratory(),
            FormatProfile::LegacyPythonObserved20260923,
            256,
        );
        let retained = tos_validation::record_biblio_cut::inspect_records_from_cut(
            &damaged_cut,
            negative_limits,
            &cancelled,
            &mut negative_records,
        )
        .unwrap();
        negative_records
            .finish(negative_deadline, &cancelled)
            .unwrap();
        let mut negative_schemas = CutWorkerSchemaExecutor::from_cut(
            &damaged_cut,
            FormatProfile::LegacyPythonObserved20260923,
            record_executor.worker.clone(),
            ExecutorBudget::laboratory(),
            CutWorkerLimits {
                max_receipts: 256,
                max_receipt_bytes: 262_144,
            },
            negative_deadline,
            &cancelled,
        )
        .unwrap();
        let refused = tos_validation::biblio_rules::inspect_bibliography_from_cut(
            &damaged_cut,
            &retained,
            negative_limits,
            &cancelled,
            &mut negative_schemas,
        )
        .unwrap();
        negative_schemas
            .finish(negative_deadline, &cancelled)
            .unwrap();
        assert!(
            refused
                .shadow
                .issues
                .iter()
                .any(|issue| issue.code == code && issue.location.starts_with(claim_path))
        );
        assert!(
            !refused
                .native_compounds
                .iter()
                .any(|observed| observed.claim_path == claim_path
                    && observed.transport
                        == tos_validation::native_compound::NativeTransportState::Committed)
        );
    }
}

#[test]
fn actual_cut_schema_batch_binds_ordered_units_and_refuses_partial_receipts() {
    use tos_validation::executor::BatchBudget;
    use tos_validation::source_cut::{CutSchemaCheck, CutSchemaExecutor};

    let files = selected_item_sources();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    let revision = write_cut_store(&files, &root);
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let reader = CorpusReader::open_existing(
        &root,
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 2_097_152,
            json: JsonLimits::default(),
        },
    )
    .unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 1024,
                max_total_bytes: 16_777_216,
                max_member_bytes: 2_097_152,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let worker_path = selected_worker_path();
    let worker_digest = Digest256::of_bytes(&fs::read(&worker_path).unwrap());
    let mut schemas = CutWorkerSchemaExecutor::from_cut(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            absolute_path: worker_path,
            sha256: worker_digest,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 16,
            max_receipt_bytes: 32_768,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let binding = schemas.execution_binding();
    let item_path = format!("{ITEM}/item.json");
    let item: Value = serde_json::from_slice(&files[&item_path]).unwrap();
    let version = serde_json::to_vec(&item["record_version"]).unwrap();
    let label = serde_json::to_vec(&item["preferred_label"]).unwrap();
    let version_contract = "ToS/contracts/corpus-record.schema.json#/properties/record_version";
    // These are field-instance probes and deliberate negative mutations, not
    // whole-record validation, source enumeration or owner admission.
    let mut spaced_version = vec![b' '];
    spaced_version.extend_from_slice(&version);
    spaced_version.push(b'\n');
    let inputs = vec![
        CutSchemaCheck {
            path: format!("{item_path}#/record_version/first"),
            raw: spaced_version,
            contract: version_contract.into(),
        },
        CutSchemaCheck {
            path: format!("{item_path}#/record_version/negative"),
            raw: b"0".to_vec(),
            contract: version_contract.into(),
        },
        CutSchemaCheck {
            path: format!("{item_path}#/record_version/repeated"),
            raw: version,
            contract: version_contract.into(),
        },
        CutSchemaCheck {
            path: format!("{item_path}#/preferred_label"),
            raw: label,
            contract: "ToS/contracts/corpus-record.schema.json#/properties/preferred_label".into(),
        },
    ];
    let mut budget = BatchBudget::laboratory();
    budget.max_units = inputs.len();
    budget.max_total_raw_bytes = inputs.iter().map(|input| input.raw.len()).sum();
    assert_eq!(
        schemas
            .check_batch(&inputs, budget, deadline, &cancelled)
            .unwrap(),
        vec![true, false, true, true]
    );
    assert_eq!(schemas.receipts().len(), inputs.len());
    let checkpoint = schemas.receipts()[0].batch.unwrap().checkpoint;
    assert_eq!(checkpoint.completed_count, inputs.len() as u64);
    assert_eq!(checkpoint.worker_sha256, worker_digest);
    assert_eq!(checkpoint.schema_set_sha256, binding.schema_set_sha256);
    assert_eq!(checkpoint.profile, binding.schema_profile);
    let mut unit_digests = BTreeSet::new();
    for (ordinal, (input, receipt)) in inputs.iter().zip(schemas.receipts()).enumerate() {
        let batch = receipt.batch.unwrap();
        assert_eq!(batch.checkpoint, checkpoint);
        assert_eq!(batch.ordinal, ordinal as u64);
        assert!(unit_digests.insert(batch.unit_sha256));
        assert_eq!(receipt.path, input.path);
        assert_eq!(receipt.contract, input.contract);
        assert_eq!(receipt.source_revision, revision);
        assert_eq!(receipt.source_raw_sha256, Digest256::of_bytes(&input.raw));
        let decoded: Value = serde_json::from_slice(&input.raw).unwrap();
        let decoded_digest = Digest256::of_bytes(&serde_json::to_vec(&decoded).unwrap());
        assert_eq!(receipt.decoded_instance_sha256, decoded_digest);
        assert_eq!(receipt.execution.instance_sha256, decoded_digest);
        assert_eq!(receipt.execution.request_sha256, checkpoint.request_sha256);
        assert_eq!(receipt.execution.worker_sha256, worker_digest);
        assert_eq!(
            receipt.execution.schema_set_sha256,
            binding.schema_set_sha256
        );
    }
    assert_ne!(
        schemas.receipts()[0].source_raw_sha256,
        schemas.receipts()[0].decoded_instance_sha256
    );
    // A later request in the same operation keeps the selected closure while
    // receiving a fresh nonce/sequence binding and an independent verdict.
    let prior = schemas.receipts().len();
    let repeat = [inputs[0].clone()];
    assert_eq!(
        schemas
            .check_batch(&repeat, BatchBudget::laboratory(), deadline, &cancelled)
            .unwrap(),
        vec![true]
    );
    let later = schemas.receipts()[prior].batch.unwrap().checkpoint;
    assert_ne!(later.request_sha256, checkpoint.request_sha256);
    assert_eq!(later.schema_set_sha256, checkpoint.schema_set_sha256);
    assert_eq!(later.worker_sha256, checkpoint.worker_sha256);
    assert_eq!(later.completed_count, 1);
    let accepted = schemas.receipts().len();
    // The first valid unit is emitted before this second decoded instance
    // exceeds the strict worker's depth budget. Complete coverage must fail;
    // the adapter may not append the provisional first receipt.
    let mut deep = vec![b'['; 65];
    deep.push(b'0');
    deep.extend(std::iter::repeat_n(b']', 65));
    let incomplete = vec![
        inputs[0].clone(),
        CutSchemaCheck {
            path: format!("{item_path}#/record_version/depth-budget"),
            raw: deep,
            contract: version_contract.into(),
        },
        inputs[2].clone(),
    ];
    budget.max_units = incomplete.len();
    budget.max_total_raw_bytes = incomplete.iter().map(|input| input.raw.len()).sum();
    let refusal = schemas
        .check_batch(&incomplete, budget, deadline, &cancelled)
        .unwrap_err();
    match refusal {
        tos_validation::item_rules::ItemRefusal::Unsupported(reason) => {
            // The existing owner diagnostic retains the typed transport
            // outcome: this must be a partial worker result, not an earlier
            // adapter budget/deadline refusal that happened to leave no rows.
            assert!(reason.contains("reason: InputBudget"), "{reason}");
            assert!(reason.contains("completed_count: 1"), "{reason}");
        }
        other => panic!("expected actual partial worker refusal, got {other:?}"),
    }
    assert_eq!(schemas.receipts().len(), accepted);
    assert!(
        schemas.receipts()[..prior]
            .iter()
            .all(|receipt| receipt.batch.unwrap().checkpoint == checkpoint)
    );
    assert!(
        schemas.finish(deadline, &cancelled).is_err(),
        "partial exchange poisons finalization"
    );
    assert!(
        schemas
            .check_batch(&inputs, BatchBudget::laboratory(), deadline, &cancelled)
            .is_err(),
        "no transparent retry after failed exchange"
    );
    assert!(
        schemas
            .check_batch(
                &inputs,
                BatchBudget::laboratory(),
                deadline,
                &AtomicBool::new(true)
            )
            .is_err()
    );
    assert!(
        schemas
            .check_batch(
                &inputs,
                BatchBudget::laboratory(),
                Instant::now(),
                &cancelled
            )
            .is_err()
    );
    assert_eq!(schemas.receipts().len(), accepted);
}

#[test]
fn maintained_assessment_whole_output_matches_native_current_view() {
    use tos_validation::assessment::{
        AssessmentLimits, AssessmentReadInput, AssessmentRecordInput, AssessmentRefusal,
        AssessmentSourceRoute, AssessmentSubmissionInput, evaluate_current_assessment,
    };
    use tos_validation::executor::BatchBudget;

    // Reuse the maintained pure owner fixture factory and implementation.
    // No protected journal, public source, semantic review or grant is written.
    // The oracle is the entire current-view output, not a hand-authored bool.
    let oracle = std::process::Command::new("python3").arg("-c").arg(r#"
import copy,json,sys
from dataclasses import replace
from pathlib import Path
root=Path(sys.argv[1])
sys.path.insert(0,str(root/'mechanics/growth-cycle/tests'))
from test_knowledge_assessment import AssessmentPolicyTests,NOW
from knowledge_assessment import Record,CommittedScope,RequiredAdmission
def fresh():
    c=AssessmentPolicyTests();c.setUp();return c
def envelope(r):
    return dict(id=r.id,version=r.version,payload=r.payload,origin_id=r.origin_id)
def submission(s):
    scope=None if s.committed_scope is None else dict(assertion_layer=s.committed_scope.assertion_layer,
        risk=s.committed_scope.risk,languages=list(s.committed_scope.languages),
        maker_id=s.committed_scope.maker_id,requested_use=s.committed_scope.requested_use)
    return dict(assessment=s.assessment,principal_id=s.principal_id,
        execution_profile=s.execution_profile.ref,committed_scope=scope)
rows=[]
def emit(name,c,reviews=(),history=(),context=None,now=NOW,sourced=(),native=(),claim=False,source_read=None):
    ctx=context or c.context
    source_ids={r.id for r in (*sourced,*native)}
    rows.append(dict(name=name,policy=envelope(c.policy),authorities=list(map(envelope,c.authorities)),
        competencies=list(map(envelope,c.competencies)),records=[envelope(r) for r in c.records if r.id not in source_ids],
        source_records=list(map(envelope,sourced)),native_records=list(map(envelope,native)),claim=claim,
        subject_id=ctx.record.id,configured_scope=dict(record=ctx.record.ref,assertion_layer=ctx.assertion_layer,
            risk=ctx.risk,languages=list(ctx.languages),maker_id=ctx.maker_id,requested_use=ctx.requested_use,
            access_allowed=ctx.access_allowed),required_source_refs=[r.ref for r in ctx.required_sources],
        required_admission_bases=[envelope(a.basis) for a in ctx.required_admissions],
        reviews=list(map(submission,reviews)),trusted_history=list(map(submission,history)),observed_now=now,source_read=source_read,
        expected=c.engine().evaluate(ctx,reviews,now=now,trusted_history=history)))
c=fresh();emit('positive-current',c,[c.review()])
c=fresh();emit('competing-reject',c,[c.review(),c.review(1,decision='reject')])
c=fresh();s=c.review();s.assessment['counterevidence_search']['status']='not-searched';emit('countersearch',c,[s])
c=fresh();s=c.review();s.assessment['rationale']=17;emit('schema-invalid',c,[s])
c=fresh();s=c.review();t=copy.deepcopy(s);t.assessment['rationale']='conflicting fixture';emit('identity-collision',c,[s,t])
c=fresh();s=c.review();s.assessment['subject']['version']=1.0;s.assessment['policy']['version']=1.0
s.assessment['authority']['version']=1.0;s.assessment['competence']['version']=1.0
emit('python-numeric-exactrefs',c,[s])
c=fresh();body=c.policy.payload;body['profiles'][0]['min_reviewers']=9007199254740993
c.policy=Record.from_payload(c.policy.id,1,body)
for i,r in enumerate(c.authorities):
    body=r.payload;body['policy']=c.policy.ref;c.authorities[i]=Record.from_payload(r.id,1,body)
emit('large-integer-quorum',c,[c.review()])
c=fresh();s=c.review();body=c.authorities[0].payload;body['state']='revoked'
c.authorities[0]=Record.from_payload(c.authorities[0].id,2,{**body,'authority_version':2})
emit('current-revocation',c,history=[replace(s,committed_scope=CommittedScope.from_context(c.context))])
c=fresh();emit('current-time-expired',c,[c.review()],now='2026-10-02T00:00:00Z')
c=fresh();s=c.review();w=c.review(decision='withdraw',name='tos.review.fixture-withdrawal')
w.assessment['supersedes']=[Record.from_payload(s.assessment['assessment_id'],1,s.assessment).ref]
scope=CommittedScope.from_context(c.context)
emit('committed-withdrawal',c,history=[replace(s,committed_scope=scope),replace(w,committed_scope=scope)])
body=c.authorities[0].payload;body['state']='revoked';body['authority_version']=2
c.authorities[0]=Record.from_payload(c.authorities[0].id,2,body)
emit('historical-suppression-after-revocation',c,history=[replace(s,committed_scope=scope),replace(w,committed_scope=scope)])
c=fresh();s=replace(c.review(),committed_scope=CommittedScope.from_context(c.context))
emit('purpose-changed',c,history=[s],context=replace(c.context,risk='moderate'))
c=fresh();body=c.authorities[0].payload;body['languages']=['STRASSE']
c.authorities[0]=Record.from_payload(c.authorities[0].id,1,body)
body=c.competencies[0].payload;body['languages']=['strasse']
c.competencies[0]=Record.from_payload(c.competencies[0].id,1,body)
body=c.authorities[0].payload;body['competence_refs']=[c.competencies[0].ref]
c.authorities[0]=Record.from_payload(c.authorities[0].id,1,body)
s=c.review();s.assessment['language']='strasse'
ctx=replace(c.context,languages=('Straße',))
s=replace(s,committed_scope=CommittedScope.from_context(replace(ctx,languages=('STRASSE',))))
emit('unicode-casefold-purpose',c,history=[s],context=ctx)
c=fresh();s=c.review(profile='identity');t=c.review(1,profile='identity')
for r in (s,t):r.assessment['evidence'].append(dict(record=c.source_b.ref,stance='supports',locator='fixture B'))
emit('independent-quorum',c,[s,t],context=replace(c.context,assertion_layer='identity_assertion',risk='high'))
body=c.authorities[1].payload;body['independence_group']='assessor-a'
c.authorities[1]=Record.from_payload(c.authorities[1].id,1,body)
t=c.review(1,profile='identity');t.assessment['evidence'].append(dict(record=c.source_b.ref,stance='supports',locator='fixture B'))
emit('shared-independence-group',c,[s,t],context=replace(c.context,assertion_layer='identity_assertion',risk='high'))
c=fresh();body=c.policy.payload;body['profiles'][0].update(min_reviewers=3,min_independence_groups=3)
c.policy=Record.from_payload(c.policy.id,1,body)
for i,r in enumerate(c.authorities):
    body=r.payload;body['policy']=c.policy.ref;body['independence_group']='group-one' if i==0 else 'group-shared'
    c.authorities[i]=Record.from_payload(r.id,1,body)
body=c.competencies[1].payload;body.update(competence_id='tos.competence.assessor-c',actor_id='assessor-c')
third=Record.from_payload(body['competence_id'],1,body);c.competencies.append(third)
body=c.authorities[1].payload;body.update(authority_id='tos.authority.assessor-c',actor_id='assessor-c',competence_refs=[third.ref])
c.authorities.append(Record.from_payload(body['authority_id'],1,body))
c.competencies.append(c.competencies[0])
body=c.authorities[0].payload;body.update(authority_id='tos.authority.assessor-a-alt',independence_group='group-two')
c.authorities.append(Record.from_payload(body['authority_id'],1,body))
voters=[c.review(i,name=f'tos.review.matching-{i}') for i in range(4)]
emit('actor-group-maximum-matching',c,voters)
body=c.authorities[2].payload;body['independence_group']='group-third'
c.authorities[2]=Record.from_payload(c.authorities[2].id,1,body)
voters[2]=c.review(2,name='tos.review.matching-2')
emit('actor-group-augmenting-path',c,voters)
c=fresh();same=Record.from_payload(c.source_b.id,1,c.source.payload,origin_id='source-b')
c.records[2]=same;s=c.review(profile='identity');t=c.review(1,profile='identity')
for r in (s,t):r.assessment['evidence'].append(dict(record=same.ref,stance='supports',locator='same exact source bytes'))
emit('same-bytes-no-extra-origin',c,[s,t],context=replace(c.context,assertion_layer='identity_assertion',risk='high'))
c=fresh();basis=Record.from_payload('tos.quality-basis.fixture',1,dict(synthetic=True,can_use=True,
    limits=['fixture quality limitation']),origin_id='source-a');c.records.append(basis)
ctx=replace(c.context,required_sources=(c.source,),required_admissions=(RequiredAdmission(basis,True,('fixture quality limitation',)),))
s=c.review();s.assessment['evidence'].append(dict(record=basis.ref,stance='context',locator='whole fixture basis'))
emit('exact-dependency-and-limits',c,[s],context=ctx)
missing=copy.deepcopy(s);missing.assessment['evidence'][0]['record']['version']=1.0
emit('canonical-evidence-not-numeric-ref',c,[missing],context=ctx)
c=fresh();binding=dict(unit_id='tos.text-unit.fixture',version=1,sha256='0'*64)
subject=Record.from_payload(c.subject.id,1,dict(synthetic=True,native_text_binding=binding))
c.subject=subject;c.records[0]=subject;ctx=replace(c.context,record=subject,source_read_ready=False)
native=Record.from_payload('tos.native.fixture',1,dict(native_binding=binding,content_verified=False),origin_id='fixture-native')
c.records.append(native)
emit('metadata-only-is-not-content-read',c,[c.review()],context=ctx,sourced=[subject],native=[native],claim=True,
    source_read=dict(required=True,ready=False))
print(json.dumps(rows,ensure_ascii=False,allow_nan=False,separators=(',',':')))
"#).arg(repository()).output().unwrap();
    assert!(
        oracle.status.success(),
        "maintained owner oracle: {}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let cases: Value = serde_json::from_slice(&oracle.stdout).unwrap();
    let cases = cases.as_array().unwrap();
    assert!(!cases.is_empty());
    let files = selected_item_sources();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    let revision = write_cut_store(&files, &root);
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(300);
    let reader = CorpusReader::open_existing(
        &root,
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 2_097_152,
            json: JsonLimits::default(),
        },
    )
    .unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 1024,
                max_total_bytes: 16_777_216,
                max_member_bytes: 2_097_152,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let worker_path = selected_worker_path();
    let worker_digest = Digest256::of_bytes(&fs::read(&worker_path).unwrap());
    let mut schemas = CutWorkerSchemaExecutor::from_cut(
        &cut,
        FormatProfile::AssertedSourceCandidateV1,
        ExactWorkerIdentity {
            absolute_path: worker_path,
            sha256: worker_digest,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 512,
            max_receipt_bytes: 524_288,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let record = |value: &Value| AssessmentRecordInput {
        envelope: serde_json::to_vec(value).unwrap(),
    };
    let records = |value: &Value| {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(record)
            .collect::<Vec<_>>()
    };
    let submissions = |value: &Value| {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|s| AssessmentSubmissionInput {
                assessment: serde_json::to_vec(&s["assessment"]).unwrap(),
                principal_id: required(s, "principal_id").into(),
                execution_profile: serde_json::to_vec(&s["execution_profile"]).unwrap(),
                committed_scope: (!s["committed_scope"].is_null())
                    .then(|| serde_json::to_vec(&s["committed_scope"]).unwrap()),
            })
            .collect::<Vec<_>>()
    };
    let limits = AssessmentLimits {
        max_input_bytes: 8_388_608,
        max_work: 4_194_304,
        batch: BatchBudget::laboratory(),
        deadline,
    };
    let mut hashes = BTreeSet::new();
    let mut last_input = None;
    for case in cases {
        let name = required(case, "name");
        let input = AssessmentReadInput {
            source_revision: revision,
            policy: record(&case["policy"]),
            authorities: records(&case["authorities"]),
            competencies: records(&case["competencies"]),
            records: records(&case["records"]),
            source_records: records(&case["source_records"]),
            native_records: records(&case["native_records"]),
            source_route: if case["claim"].as_bool().unwrap() {
                AssessmentSourceRoute::SourceBoundClaim
            } else {
                AssessmentSourceRoute::SelectedSource
            },
            subject_id: required(case, "subject_id").into(),
            configured_scope: serde_json::to_vec(&case["configured_scope"]).unwrap(),
            required_source_refs: case["required_source_refs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| serde_json::to_vec(r).unwrap())
                .collect(),
            required_admission_bases: records(&case["required_admission_bases"]),
            reviews: submissions(&case["reviews"]),
            trusted_history: submissions(&case["trusted_history"]),
            observed_now: required(case, "observed_now").into(),
        };
        let start = schemas.receipts().len();
        let report = evaluate_current_assessment(&input, &mut schemas, limits, &cancelled)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            report.current_admission(),
            &case["expected"],
            "entire maintained output: {name}"
        );
        assert_eq!(report.schema_binding().source_revision, revision);
        assert_eq!(report.schema_binding().worker_sha256, worker_digest);
        assert_eq!(report.observed_now(), required(case, "observed_now"));
        if case["source_read"].is_null() {
            assert!(
                !report.source_read_required(),
                "empty native closure must not create a present Sign read gate: {name}"
            );
            assert!(report.source_read_ready());
        } else {
            assert_eq!(
                report.source_read_required(),
                case["source_read"]["required"].as_bool().unwrap()
            );
            assert_eq!(
                report.source_read_ready(),
                case["source_read"]["ready"].as_bool().unwrap()
            );
        }
        assert!(
            hashes.insert(report.input_sha256()),
            "each owner observation must bind a distinct input: {name}"
        );
        assert!(schemas.receipts().len() > start);
        assert!(
            schemas.receipts()[start..]
                .iter()
                .all(|r| r.source_revision == revision
                    && r.execution.worker_sha256 == worker_digest
                    && r.execution.profile == FormatProfile::AssertedSourceCandidateV1
                    && r.batch.is_some())
        );
        last_input = Some(input);
    }
    let mut input = last_input.unwrap();
    let start = schemas.receipts().len();
    let current_revision = input.source_revision;
    input.source_revision = SourceRevision(Digest256::of_bytes(b"different selected source"));
    assert!(matches!(
        evaluate_current_assessment(&input, &mut schemas, limits, &cancelled),
        Err(AssessmentRefusal::InvalidInput(_))
    ));
    input.source_revision = current_revision;
    assert!(matches!(
        evaluate_current_assessment(&input, &mut schemas, limits, &AtomicBool::new(true)),
        Err(AssessmentRefusal::Cancelled)
    ));
    assert!(matches!(
        evaluate_current_assessment(
            &input,
            &mut schemas,
            AssessmentLimits {
                deadline: Instant::now(),
                ..limits
            },
            &cancelled
        ),
        Err(AssessmentRefusal::Deadline)
    ));
    assert!(matches!(
        evaluate_current_assessment(
            &input,
            &mut schemas,
            AssessmentLimits {
                max_input_bytes: 1,
                ..limits
            },
            &cancelled
        ),
        Err(AssessmentRefusal::Budget)
    ));
    input.source_route = AssessmentSourceRoute::LayerQuality;
    assert!(matches!(
        evaluate_current_assessment(&input, &mut schemas, limits, &cancelled),
        Err(AssessmentRefusal::Unsupported(_))
    ));
    assert_eq!(schemas.receipts().len(), start);
    // Every true fixture value above remains a pure synthetic mechanics
    // observation. No journal, current source admission or Sign issuer exists.
}
