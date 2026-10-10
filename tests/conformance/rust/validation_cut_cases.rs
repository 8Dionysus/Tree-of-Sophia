//! One composed source-carrier/worker/Item contract case. Fixture selection is
//! explicit and smaller than the full authored universe; no admission follows.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_source_store::CutReadLimits;
use tos_validation::FormatProfile;
use tos_validation::executor::{
    BatchBudget, BatchCoverageExpectation, BatchStreamBudget, BatchUnit, ExactWorkerIdentity,
    ExecutorBudget,
};
use tos_validation::item_rules::ItemLimits;
use tos_validation::operation::{
    GeneralOperationLimits, OperationChange, OperationFamilyScope, OperationFamilyState,
    OperationLimits, OperationProposal, OperationRefusal, bind_operation_from_cut,
    inspect_general_operation, inspect_item_operation,
};
use tos_validation::record_rules::{RecordFamily, RecordSchema};
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor, MetadataOnlyPayloads};
use tos_validation::source_foundation_schema::{
    SourceFoundationLegacySchemaInput, SourceFoundationMixedSchemaInput,
    SourceFoundationSchemaFailure, SourceFoundationSchemaInput, SourceFoundationSchemaLimits,
    SourceFoundationSchemaOutcome, SourceFoundationSchemaSet,
    evaluate_source_foundation_mixed_schema_checks,
};

const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const ENTITY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const ITEM: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3/items/dta-sbb-corrected-tei-p5";
const INVENTORY_SCHEMA: &str = "ToS/contracts/source-resource-inventory.schema.json";

pub(super) fn repository() -> PathBuf {
    fixtures()
        .join("../../..")
        .canonicalize()
        .expect("selected source repository")
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

pub(super) fn write_cut_store_on_base(
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

pub(super) fn write_cut_store_with_optional_modes(
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

fn selected_source_foundation_contracts() -> BTreeMap<String, Vec<u8>> {
    let root = repository();
    fs::read_dir(root.join("ToS/contracts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .ends_with(".schema.json")
        })
        .map(|path| {
            let relative = path
                .strip_prefix(&root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            (relative, fs::read(path).unwrap())
        })
        .collect()
}

fn source_foundation_schema_limits(
    contracts: &BTreeMap<String, Vec<u8>>,
    max_checks: usize,
) -> SourceFoundationSchemaLimits {
    let schema_bytes = contracts
        .values()
        .try_fold(0usize, |total, raw| total.checked_add(raw.len()))
        .unwrap();
    let max_schema_resource_bytes = contracts.values().map(Vec::len).max().unwrap();
    let mut batch = BatchBudget::laboratory();
    batch.max_units = 8;
    batch.max_total_raw_bytes = 2 * 1024 * 1024;
    SourceFoundationSchemaLimits {
        max_schema_resources: contracts.len(),
        max_schema_resource_bytes,
        max_total_schema_bytes: schema_bytes,
        max_checks,
        max_chunks: 1,
        max_total_cpu_seconds: batch.cpu_seconds,
        max_instance_bytes: 1024 * 1024,
        max_total_instance_bytes: 1024 * 1024,
        max_total_issues: max_checks * 128,
        max_total_report_bytes: 1024 * 1024,
        max_total_worker_wire_bytes: BatchStreamBudget::laboratory().max_total_wire_bytes,
        batch,
    }
}

fn source_foundation_schema_set_from_fixture(
    store: &Path,
    revision: SourceRevision,
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceFoundationSchemaSet {
    let reader = CorpusReader::open_existing(
        store,
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: limits.max_schema_resources,
            max_selected_object_bytes: 1_048_576,
            json: JsonLimits::default(),
        },
    )
    .unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: limits.max_schema_resources.try_into().unwrap(),
                max_total_bytes: 2_097_152,
                max_member_bytes: 1_048_576,
            },
            deadline,
            cancelled,
        )
        .unwrap();
    SourceFoundationSchemaSet::from_cut(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        limits,
        deadline,
        cancelled,
    )
    .unwrap()
}

fn inventory_json(width_points: &str, height_points: &str, resource_kind_fields: &str) -> Vec<u8> {
    const TEMPLATE: &str = r#"{"$schema":"https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json","schema_version":"tos_source_resource_inventory_v1","item_id":"tos.item.fixture","generated_from_manifest_ref":"ToS/source-witnesses/fixture/item.manifest.json","inventory_authority":"mechanical_metadata_only","source_text_included":false,"files":[{"file_id":"tos.file.fixture","file_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","media_type":"application/pdf","profile":"pdf_pages_v1","summary":{"resource_count":1},"resources":[{"resource_id":"page-1",__RESOURCE_KIND_FIELDS__,"locator":{"page_index":1,"width_points":__WIDTH_POINTS__,"height_points":__HEIGHT_POINTS__},"label_fingerprint":{"algorithm":"sha256","normalization":"unicode-nfc-whitespace-collapse","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","character_count":1},"content_fingerprint":{"algorithm":"sha256","normalization":"unicode-nfc-whitespace-collapse","sha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","character_count":1}}]}],"generator":{"name":"build_source_resource_inventories.py","version":"2"},"provenance_event_ref":"tos.event.fixture","inventory_version":1,"authority_boundary":"quoted NaN, Infinity, and -Infinity remain text"}"#;
    TEMPLATE
        .replace("__RESOURCE_KIND_FIELDS__", resource_kind_fields)
        .replace("__WIDTH_POINTS__", width_points)
        .replace("__HEIGHT_POINTS__", height_points)
        .into_bytes()
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
    let image = super::command_form_cases::schema_image(
        tos_validation::executor::ExecutorBudget::laboratory(),
        deadline,
        &cancelled,
    );
    let worker_digest = image.identity().sha256;
    let mut schemas = CutWorkerSchemaExecutor::from_cut_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &image,
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
            "rust/crates/tos-compiler/src/provenance_event_lab.rs",
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
    let image =
        super::command_form_cases::schema_image(ExecutorBudget::laboratory(), deadline, &cancelled);
    let digest = image.identity().sha256;
    let mut schemas = CutWorkerSchemaExecutor::from_cut_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &image,
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
    // This composed fixture finishes 191 schema receipts. Its unoptimized CI
    // worker exceeds the three-second scalar-probe allowance at finalization.
    // Select one finite fixture CPU envelope; the shared under-budget refusals
    // below still derive from both selected worker envelopes.
    let worker_budget = ExecutorBudget {
        cpu_seconds: 10,
        ..ExecutorBudget::laboratory()
    };
    let stage_started = Instant::now();
    let stage = |phase: &'static str| {
        eprintln!(
            "general operation fixture phase={phase} elapsed_ms={}",
            stage_started.elapsed().as_millis()
        );
    };
    stage("selected-source-start");
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
    // Select the complete source-owned metadata packet from one exact tracked
    // source revision. The current plan is used only to name the capture; the
    // captured plan must be byte-identical before any selected bytes are used.
    // This fixture does not claim the packet binds the moving INT source head.
    let opening_plan = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-opening-sentence-alignment.plan.v1.json";
    let plan_raw = fs::read(owner.join(opening_plan)).unwrap();
    let plan: Value = serde_json::from_slice(&plan_raw).unwrap();
    let mut opening_paths = vec![(opening_plan.to_owned(), None)];
    for field in [
        "source_sentence_packet_ref",
        "target_sentence_packet_ref",
        "alignment_packet_ref",
        "provenance_event_ref",
    ] {
        let path = required(&plan["outputs"], field);
        opening_paths.push((path.to_owned(), None));
    }
    for (side, reference, digest) in [
        ("source", "text_layer_ref", "text_layer_record_sha256"),
        ("source", "layout_packet_ref", "layout_packet_sha256"),
        (
            "source",
            "edition_reading_admission_ref",
            "edition_reading_admission_sha256",
        ),
        ("source", "rights_ref", "rights_sha256"),
        ("target", "text_layer_ref", "text_layer_record_sha256"),
        ("target", "layout_packet_ref", "layout_packet_sha256"),
        (
            "target",
            "expression_record_ref",
            "expression_record_sha256",
        ),
        (
            "target",
            "responsibility_claims_ref",
            "responsibility_claims_sha256",
        ),
        ("target", "rights_ref", "rights_sha256"),
    ] {
        let path = required(&plan[side], reference);
        opening_paths.push((
            path.to_owned(),
            Some(required(&plan[side], digest).to_owned()),
        ));
    }
    assert_eq!(opening_paths.len(), 14);
    assert_eq!(
        opening_paths
            .iter()
            .map(|(path, _)| path)
            .collect::<BTreeSet<_>>()
            .len(),
        14
    );
    let prefixes = opening_paths
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>();
    let opening_source = include_str!("retained-opening-source.txt").trim();
    let opening_capture = tempfile::tempdir().unwrap();
    let captured = opening_capture.path().join("captured");
    let restored = opening_capture.path().join("restored");
    let archive_deadline = Instant::now() + Duration::from_secs(120);
    let archive_cancelled = AtomicBool::new(false);
    stage("archive-capture-start");
    let opening_selection = super::source_cut_cases::capture_software_archive(
        &owner.canonicalize().unwrap(),
        opening_source,
        &prefixes,
        &captured,
        archive_deadline,
        &archive_cancelled,
    );
    stage("archive-capture-ready");
    let captured_manifest: Value =
        serde_json::from_slice(&fs::read(captured.join("capture.json")).unwrap()).unwrap();
    assert_eq!(captured_manifest["source_git_commit"], opening_source);
    stage("archive-restore-start");
    super::source_cut_cases::restore_software_archive(
        &captured,
        &restored,
        &opening_selection,
        archive_deadline,
        &archive_cancelled,
    );
    stage("archive-restore-ready");
    assert_eq!(fs::read(restored.join(opening_plan)).unwrap(), plan_raw);
    for (path, expected_sha) in opening_paths {
        let raw = fs::read(restored.join(&path)).unwrap();
        if let Some(expected_sha) = expected_sha {
            assert_eq!(Digest256::of_bytes(&raw).to_hex(), expected_sha, "{path}");
        }
        if let Some(existing) = before.get(&path) {
            assert_eq!(existing, &raw, "selected opening-sentence source {path}");
        }
        before.insert(path, raw);
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
    stage("compound-fixture-start");
    let compound_files = crate::frozen_legacy_python_oracle("compound-source-family");
    stage("compound-fixture-loaded");
    // The captured full output includes historical generated catalog
    // companions; only the source-store's authored carriers enter both cuts.
    // Retained control/transaction/archive and human-form bytes stay selected.
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
            // Historical commands retain their own exact versioned grammar.
            // Other overlapping source items must still be identical.
            if !path.starts_with("ToS/contracts/")
                && !path.starts_with("ToS/doctrine/semantic-interchange/")
            {
                assert_eq!(existing, &raw, "selected common source {path}");
            }
        }
        before.insert(path.clone(), raw);
    }
    assert!(
        !generated_catalog_files.is_empty(),
        "frozen owner output includes generated catalog companions"
    );
    assert!(
        before
            .keys()
            .all(|path| tos_source_store::is_authored_source_path_v1(path)),
        "base/current cut contain only authored source members"
    );
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    stage("selected-store-start");
    let base = write_cut_store(&before, &root);
    let mut files = before.clone();
    let changed_path = format!("{ITEM}/forensic-report.md");
    files.get_mut(&changed_path).unwrap().push(b'\n');
    let revision = write_cut_store_on_base(&files, &root, Some(base));
    stage("selected-store-ready");
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
    stage("positive-image-start");
    let image = super::command_form_cases::schema_image(worker_budget, deadline, &cancelled);
    stage("positive-image-ready");
    let schema_limits = CutWorkerLimits {
        max_receipts: 256,
        max_receipt_bytes: 262_144,
    };
    let mut schemas = CutWorkerSchemaExecutor::from_cut_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &image,
        worker_budget,
        schema_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut record_executor = BiblioRecordExecutor::new_with_image(
        &image,
        worker_budget,
        FormatProfile::LegacyPythonObserved20260923,
        256,
        deadline,
        &cancelled,
    )
    .unwrap();
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
        max_composed_schema_cpu_seconds: 2 * worker_budget.cpu_seconds,
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
    stage("positive-inspect-start");
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
    stage("positive-inspect-ready");
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
    assert!(
        report
            .layers
            .layer_family
            .checked_predicates
            .iter()
            .any(|(path, predicate)| path == opening_plan
                && predicate == "named-zarathustra-opening-sentence-tracked-closure-v1")
    );
    assert!(
        !report
            .layers
            .layer_family
            .issues
            .iter()
            .any(|issue| issue.code.starts_with("opening-sentence-")),
        "named opening-sentence source closure: {:?}",
        report.layers.layer_family.issues
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
        10,
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
            .filter(|observed| !observed.claim_path.contains("/editions/")
                && !observed.claim_path.contains("/relations/"))
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
                "native-expression-responsibility-compound-evidence"
                    | "native-collection-work-compound-evidence"
                    | "native-work-expression-compound-evidence"
                    | "native-expression-edition-compound-evidence"
                    | "native-edition-item-compound-evidence"
            ))
    );
    assert!(
        report
            .bibliography
            .shadow
            .checked_profiles
            .contains("native-collection-work-exact-compound-plan-and-current-lineage@1")
    );
    assert_eq!(
        report
            .bibliography
            .native_compounds
            .iter()
            .filter(|observed| observed
                .claim_path
                .contains("/relations/synthetic-membership-"))
            .count(),
        2
    );
    assert!(
        !report
            .bibliography
            .shadow
            .skipped_profiles
            .contains("native-compound-owner-evidence:contains_work")
    );

    assert!(
        report
            .bibliography
            .shadow
            .checked_profiles
            .contains("native-expression-responsibility-exact-compound-plan-and-current-lineage@1")
    );
    assert_eq!(
        report
            .bibliography
            .native_compounds
            .iter()
            .filter(|observed| observed
                .claim_path
                .contains("/relations/synthetic-translator-"))
            .count(),
        2
    );
    assert!(
        !report
            .bibliography
            .shadow
            .skipped_profiles
            .contains("native-compound-owner-evidence:translated_by")
    );

    // Equal decoded JSON is insufficient: retained publication binds exact
    // receipt bytes. Inspect the changed current cut through the same actual
    // record/bibliographic route, not a caller-issued transport observation.
    let origin = report
        .bibliography
        .native_compounds
        .iter()
        .find(|observed| {
            !observed.claim_path.contains("/editions/")
                && !observed.claim_path.contains("/items/")
                && !observed.claim_path.contains("/relations/")
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
    let membership = report
        .bibliography
        .native_compounds
        .iter()
        .find(|observed| {
            observed
                .claim_path
                .contains("/relations/synthetic-membership-first/")
        })
        .unwrap();
    let membership_history = membership
        .claim_path
        .strip_suffix("source-claims.jsonl")
        .unwrap()
        .to_owned()
        + "claim-revision-history.json";
    let mut removed_history: Value = serde_json::from_slice(&files[&membership_history]).unwrap();
    removed_history["receipts"] = serde_json::json!([]);
    let translator = report
        .bibliography
        .native_compounds
        .iter()
        .find(|observed| {
            observed
                .claim_path
                .contains("/relations/synthetic-translator-first/")
        })
        .unwrap();
    let translator_receipt = translator
        .claim_path
        .strip_suffix("source-claims.jsonl")
        .unwrap()
        .to_owned()
        + "responsibility-attachment-receipt.json";
    let binding: Value = serde_json::from_slice(&files[&translator_receipt]).unwrap();
    let agent_path = binding["scope"]["agent_source_path"].as_str().unwrap();
    let agent_history =
        agent_path.strip_suffix("agent.json").unwrap().to_owned() + "source-revision-history.json";
    let mut removed_agent_history: Value = serde_json::from_slice(&files[&agent_history]).unwrap();
    removed_agent_history["receipts"] = serde_json::json!([]);
    for (target, raw, claim_path, code) in [
        (
            agent_history,
            serde_json::to_vec(&removed_agent_history).unwrap(),
            translator.claim_path.as_str(),
            "native-expression-responsibility-compound-evidence",
        ),
        (
            membership_history,
            serde_json::to_vec(&removed_history).unwrap(),
            membership.claim_path.as_str(),
            "native-collection-work-compound-evidence",
        ),
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
        stage("negative-store-start");
        let mut damaged = files.clone();
        damaged.insert(target, raw);
        let damaged_revision = write_cut_store_on_base(&damaged, &root, Some(revision));
        stage("negative-store-ready");
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
        // This negative control has its own original operation deadline;
        // admit a fresh image for it rather than renewing the positive handle.
        stage("negative-image-start");
        let negative_image =
            super::command_form_cases::schema_image(worker_budget, negative_deadline, &cancelled);
        stage("negative-image-ready");
        let mut negative_records = BiblioRecordExecutor::new_with_image(
            &negative_image,
            worker_budget,
            FormatProfile::LegacyPythonObserved20260923,
            256,
            negative_deadline,
            &cancelled,
        )
        .unwrap();
        stage("negative-records-start");
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
        stage("negative-records-ready");
        let mut negative_schemas = CutWorkerSchemaExecutor::from_cut_with_image(
            &damaged_cut,
            FormatProfile::LegacyPythonObserved20260923,
            &negative_image,
            worker_budget,
            CutWorkerLimits {
                max_receipts: 256,
                max_receipt_bytes: 262_144,
            },
            negative_deadline,
            &cancelled,
        )
        .unwrap();
        stage("negative-bibliography-start");
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
        stage("negative-bibliography-ready");
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
fn actual_native_object_link_binds_committed_origin_and_corrected_lineage() {
    use std::process::Stdio;
    use tos_validation::record_biblio_cut::BiblioRecordExecutor;
    use tos_validation::source_cut::CutSchemaExecutor;

    // One absolute case envelope includes both maintained producers, all
    // corrections, selected-cut preparation and all three validation branches.
    let deadline = Instant::now() + Duration::from_secs(360);
    let worker_path = selected_worker_path();
    let worker = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    let remaining_budget = || ExecutorBudget {
        execution_wall: deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .expect("ObjectLink whole-case deadline"),
        ..ExecutorBudget::laboratory()
    };
    // Each packet is the complete captured output of the historical owner
    // fixture. Work and Artifact retain their separate source identities.
    for kind in ["work", "artifact"] {
        let packet = crate::frozen_legacy_python_oracle(&format!("object-link-{kind}"));
        let claim_path = required(&packet, "claim_path").to_owned();
        let mut files = selected_item_sources();
        let relation = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
        files.insert(
            relation.into(),
            fs::read(repository().join(relation)).unwrap(),
        );
        for (path, hex) in packet["files"].as_object().unwrap() {
            assert!(
                tos_source_store::is_authored_source_path_v1(path),
                "unexpected non-authored ObjectLink fixture member {path}"
            );
            let hex = hex.as_str().unwrap();
            assert_eq!(hex.len() % 2, 0);
            let raw = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            if let Some(existing) = files.get(path) {
                // Historical commands retain their own exact versioned grammar.
                // Other overlapping source items must still be identical.
                if !path.starts_with("ToS/contracts/")
                    && !path.starts_with("ToS/doctrine/semantic-interchange/")
                {
                    assert_eq!(existing, &raw, "selected common source {path}");
                }
            }
            files.insert(path.clone(), raw);
        }
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("store");
        let revision = write_cut_store(&files, &root);
        let cancelled = AtomicBool::new(false);
        let reader = CorpusReader::open_existing(
            &root,
            ReadLimits {
                max_manifest_bytes: 1_048_576,
                max_manifest_entries: 1024,
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
                    max_members: 2048,
                    max_total_bytes: 32_000_000,
                    max_member_bytes: 2_097_152,
                },
                deadline,
                &cancelled,
            )
            .unwrap();
        let limits = ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline,
        };
        let mut records = BiblioRecordExecutor::new(
            worker.clone(),
            remaining_budget(),
            FormatProfile::LegacyPythonObserved20260923,
            256,
        );
        let current = tos_validation::record_biblio_cut::inspect_records_from_cut(
            &cut,
            limits,
            &cancelled,
            &mut records,
        )
        .unwrap();
        records.finish(deadline, &cancelled).unwrap();
        drop(records);
        let mut schemas = CutWorkerSchemaExecutor::from_cut(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            worker.clone(),
            remaining_budget(),
            CutWorkerLimits {
                max_receipts: 256,
                max_receipt_bytes: 262_144,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
        let report = tos_validation::biblio_rules::inspect_bibliography_from_cut(
            &cut,
            &current,
            limits,
            &cancelled,
            &mut schemas,
        )
        .unwrap();
        schemas.finish(deadline, &cancelled).unwrap();
        drop(schemas);
        assert!(
            report
                .native_compounds
                .iter()
                .any(|observed| observed.claim_path == claim_path
                    && observed.transport
                        == tos_validation::native_compound::NativeTransportState::Committed),
            "native ObjectLink source issue: {:?}",
            report.shadow.issues
        );
        assert!(
            report
                .shadow
                .checked_profiles
                .contains("native-object-link-exact-compound-plan-and-current-lineage@1")
        );
        assert!(
            !report
                .shadow
                .issues
                .iter()
                .any(|issue| issue.code == "native-object-link-compound-evidence")
        );

        if kind == "work" {
            // Removing the retained Link correction cannot be repaired by the
            // current Claim, URI or a successful schema check.
            let link_history = required(&packet, "link_history");
            let mut damaged = files.clone();
            damaged.remove(link_history).unwrap();
            let damaged_revision = write_cut_store_on_base(&damaged, &root, Some(revision));
            let damaged_cut = reader
                .open_source_cut(
                    damaged_revision,
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
            let mut negative_records = BiblioRecordExecutor::new(
                worker.clone(),
                remaining_budget(),
                FormatProfile::LegacyPythonObserved20260923,
                256,
            );
            let retained = tos_validation::record_biblio_cut::inspect_records_from_cut(
                &damaged_cut,
                limits,
                &cancelled,
                &mut negative_records,
            )
            .unwrap();
            negative_records.finish(deadline, &cancelled).unwrap();
            drop(negative_records);
            let mut negative_schemas = CutWorkerSchemaExecutor::from_cut(
                &damaged_cut,
                FormatProfile::LegacyPythonObserved20260923,
                worker.clone(),
                remaining_budget(),
                CutWorkerLimits {
                    max_receipts: 256,
                    max_receipt_bytes: 262_144,
                },
                deadline,
                &cancelled,
            )
            .unwrap();
            let refused = tos_validation::biblio_rules::inspect_bibliography_from_cut(
                &damaged_cut,
                &retained,
                limits,
                &cancelled,
                &mut negative_schemas,
            )
            .unwrap();
            negative_schemas.finish(deadline, &cancelled).unwrap();
            drop(negative_schemas);
            assert!(
                refused
                    .shadow
                    .issues
                    .iter()
                    .any(|issue| issue.code == "native-object-link-compound-evidence"
                        && issue.location.starts_with(&claim_path))
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
    let image =
        super::command_form_cases::schema_image(ExecutorBudget::laboratory(), deadline, &cancelled);
    let worker_digest = image.identity().sha256;
    let mut schemas = CutWorkerSchemaExecutor::from_cut_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &image,
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
fn actual_cut_diagnostics_bounds_residency_across_report_lifetimes() {
    use tos_validation::source_cut::{CutSchemaDiagnosticsLimits, CutSchemaExecutor};

    const CONTRACT: &str = "ToS/contracts/residency-fixture.schema.json";
    const STATE_CAP: usize = 1024 * 1024;
    let files = BTreeMap::from([(
        CONTRACT.to_owned(),
        br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://tree-of-sophia.local/residency-fixture.schema.json","type":"string"}"#.to_vec(),
    )]);
    let temporary = tempfile::tempdir().unwrap();
    let revision = write_cut_store(&files, temporary.path());
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(120);
    let reader = CorpusReader::open_existing(
        temporary.path(),
        ReadLimits {
            max_manifest_bytes: 1024 * 1024,
            max_manifest_entries: files.len(),
            max_selected_object_bytes: 1024 * 1024,
            json: JsonLimits::default(),
        },
    )
    .unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 1,
                max_members: files.len().try_into().unwrap(),
                max_total_bytes: 1024 * 1024,
                max_member_bytes: 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let image =
        super::command_form_cases::schema_image(ExecutorBudget::laboratory(), deadline, &cancelled);
    let mut schemas = CutWorkerSchemaExecutor::from_cut_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &image,
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 20,
            max_receipt_bytes: 32_768,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    schemas
        .enable_diagnostics_v2(CutSchemaDiagnosticsLimits {
            max_total_issues: 20 * 128,
            max_total_report_bytes: 1024 * 1024,
            max_total_state_bytes: STATE_CAP,
        })
        .unwrap();
    let raw = serde_json::to_vec(&"x".repeat(64 * 1024)).unwrap();
    for _ in 0..16 {
        assert!(
            schemas
                .check(
                    "fixture/resident-string",
                    &raw,
                    CONTRACT,
                    deadline,
                    &cancelled
                )
                .unwrap()
        );
    }
    let work = schemas.diagnostics_v2_cumulative_cost().unwrap();
    assert_eq!(work.completed_exchanges(), 16);
    assert!(work.request_bytes() > STATE_CAP as u64);
    assert!(work.response_bytes() > 0);

    // Invalid reports retain their allocation until the receiving owner drains
    // them; the internal Vec remains available for the next actual exchange.
    assert!(
        !schemas
            .check(
                "fixture/invalid-string",
                b"0",
                CONTRACT,
                deadline,
                &cancelled
            )
            .unwrap()
    );
    let rejected = schemas.take_schema_diagnostic_rejection().unwrap();
    assert!(rejected.is_invalid());
    assert_eq!(
        rejected.accounted_state_bytes(),
        rejected.retained_state_bytes()
    );
    drop(rejected);
    assert!(
        schemas
            .check(
                "fixture/resident-string",
                &raw,
                CONTRACT,
                deadline,
                &cancelled
            )
            .unwrap()
    );
    assert_eq!(
        schemas
            .diagnostics_v2_cumulative_cost()
            .unwrap()
            .completed_exchanges(),
        18
    );
    schemas.finish(deadline, &cancelled).unwrap();
}

#[test]
fn actual_source_foundation_inventory_profile2_binds_exceptional_schema_results() {
    use tos_validation::executor::schema_diagnostics::{PathSegment, Reason, Status};

    let mut contracts = selected_source_foundation_contracts();
    // A future source-owned declaration needs no Rust selector edit.
    const NEW_DECLARATION: &str = "ToS/contracts/future-source-owned.schema.json";
    contracts.insert(
        NEW_DECLARATION.to_owned(),
        serde_json::to_vec(&serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://tree-of-sophia.local/future-source-owned.schema.json",
            "type": "object"
        }))
        .unwrap(),
    );
    let limits = source_foundation_schema_limits(&contracts, 8);
    assert!(limits.validate());
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(120);
    let worker_path = selected_worker_path();
    let worker_sha256 = Digest256::of_bytes(&fs::read(&worker_path).unwrap());
    let worker = ExactWorkerIdentity {
        absolute_path: worker_path.clone(),
        sha256: worker_sha256,
    };
    let temporary = tempfile::tempdir().unwrap();

    // The cut owns the exact schema declarations; it never reads corpus members.
    let original_store = temporary.path().join("original-schema-cut");
    let original_revision = write_cut_store(&contracts, &original_store);
    let original_schemas = source_foundation_schema_set_from_fixture(
        &original_store,
        original_revision,
        limits,
        deadline,
        &cancelled,
    );
    assert_eq!(original_schemas.source_revision(), original_revision);
    assert_eq!(original_schemas.schema_resource_count(), contracts.len());
    for (path, raw) in &contracts {
        assert_eq!(
            original_schemas.contract_digest(path),
            Some(Digest256::of_bytes(raw)),
        );
    }

    let inventory_schema: Value = serde_json::from_slice(&contracts[INVENTORY_SCHEMA]).unwrap();
    let inventory_uri = inventory_schema["$id"].as_str().unwrap();
    let finite_raw = inventory_json("12.5", "20.5", r#""resource_kind":"pdf_page""#);
    let finite_control: Value = serde_json::from_slice(&finite_raw).unwrap();
    let raw_nan_width = inventory_json("NaN", "20", r#""resource_kind":"pdf_page""#);
    let raw_nan_height = inventory_json("10", "NaN", r#""resource_kind":"pdf_page""#);
    let raw_positive_infinity = inventory_json("Infinity", "20", r#""resource_kind":"pdf_page""#);
    let raw_negative_infinity = inventory_json("10", "-Infinity", r#""resource_kind":"pdf_page""#);
    let raw_enum_rejection = inventory_json("10", "20", r#""resource_kind":"not-a-schema-enum""#);
    let raw_last_wins = inventory_json(
        "10",
        "20",
        r#""resource_kind":"not-a-schema-enum","resource_kind":"pdf_page""#,
    );
    let locations = [
        "fixture/inventory/finite-control",
        "fixture/inventory/nan-width",
        "fixture/inventory/nan-height",
        "fixture/inventory/positive-infinity-width",
        "fixture/inventory/negative-infinity-height",
        "fixture/inventory/enum-rejection",
        "fixture/inventory/legacy-last-wins",
    ];
    let checks = [
        SourceFoundationMixedSchemaInput::Decoded(SourceFoundationSchemaInput {
            location: locations[0],
            contract: INVENTORY_SCHEMA,
            decoded_instance: &finite_control,
        }),
        SourceFoundationMixedSchemaInput::Legacy(SourceFoundationLegacySchemaInput {
            location: locations[1],
            contract: INVENTORY_SCHEMA,
            raw_instance: &raw_nan_width,
        }),
        SourceFoundationMixedSchemaInput::Legacy(SourceFoundationLegacySchemaInput {
            location: locations[2],
            contract: INVENTORY_SCHEMA,
            raw_instance: &raw_nan_height,
        }),
        SourceFoundationMixedSchemaInput::Legacy(SourceFoundationLegacySchemaInput {
            location: locations[3],
            contract: INVENTORY_SCHEMA,
            raw_instance: &raw_positive_infinity,
        }),
        SourceFoundationMixedSchemaInput::Legacy(SourceFoundationLegacySchemaInput {
            location: locations[4],
            contract: INVENTORY_SCHEMA,
            raw_instance: &raw_negative_infinity,
        }),
        SourceFoundationMixedSchemaInput::Legacy(SourceFoundationLegacySchemaInput {
            location: locations[5],
            contract: INVENTORY_SCHEMA,
            raw_instance: &raw_enum_rejection,
        }),
        SourceFoundationMixedSchemaInput::Legacy(SourceFoundationLegacySchemaInput {
            location: locations[6],
            contract: INVENTORY_SCHEMA,
            raw_instance: &raw_last_wins,
        }),
    ];
    let expected_units = checks
        .iter()
        .enumerate()
        .map(|(ordinal, check)| {
            let (location, raw_instance) = match check {
                SourceFoundationMixedSchemaInput::Decoded(input) => (
                    input.location,
                    serde_json::to_vec(input.decoded_instance).unwrap(),
                ),
                SourceFoundationMixedSchemaInput::Legacy(input) => {
                    (input.location, input.raw_instance.to_vec())
                }
            };
            BatchUnit {
                ordinal: ordinal as u64,
                member_id: format!("source-foundation-schema:{ordinal}"),
                relative_path: location.to_owned(),
                root_uri: inventory_uri.to_owned(),
                raw_instance,
            }
        })
        .collect::<Vec<_>>();
    let expected_manifest = BatchCoverageExpectation::from_units(&expected_units).unwrap();
    let original_report = match evaluate_source_foundation_mixed_schema_checks(
        &original_schemas,
        &worker,
        &checks,
        limits,
        deadline,
        &cancelled,
    ) {
        SourceFoundationSchemaOutcome::Complete(report) => report,
        SourceFoundationSchemaOutcome::Incomplete { report, reason } => {
            panic!("selected inventory profile-2 batch incomplete: {reason:?}; {report:?}")
        }
    };
    assert!(original_report.is_complete());
    assert!(!original_report.is_valid());
    assert_eq!(original_report.source_revision, original_revision);
    assert_eq!(original_report.worker_sha256, worker_sha256);
    assert_eq!(
        original_report.schema_set_sha256,
        original_schemas.schema_set_sha256()
    );
    assert_eq!(original_report.checks.len(), checks.len());
    assert_eq!(original_report.checkpoints.len(), 1);
    assert_eq!(
        original_report.checkpoints[0].completed_count,
        checks.len() as u64
    );
    assert_eq!(
        original_report.checkpoints[0].ordered_manifest_sha256,
        expected_manifest.ordered_manifest_sha256
    );
    let exceptional_usage = original_report.checkpoints[0]
        .exceptional_usage
        .expect("mixed profile-2 report carries actual exceptional work");
    assert!(exceptional_usage.schema_scan_work > 0);
    assert!(exceptional_usage.evaluation_work > 0);
    assert!(exceptional_usage.regex_checks > 0);
    for (check, location) in original_report.checks.iter().zip(locations) {
        assert_eq!(check.location, location);
        assert_eq!(check.contract, INVENTORY_SCHEMA);
        assert_eq!(check.diagnostic.worker_sha256, worker_sha256);
    }
    let issue_vector = |check_index: usize| {
        original_report.checks[check_index]
            .diagnostic
            .issues
            .iter()
            .map(|issue| {
                (
                    issue.instance_path.clone(),
                    issue.reason,
                    issue.schema_keyword.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(original_report.checks[0].diagnostic.status, Status::Valid);
    // The maintained Python validator's rejection predicate compares the
    // value with the bound using <=; NaN makes that comparison false, so both
    // geometry slots remain schema-valid and emit no exclusiveMinimum issue.
    assert_eq!(original_report.checks[1].diagnostic.status, Status::Valid);
    assert!(issue_vector(1).is_empty());
    assert_eq!(original_report.checks[2].diagnostic.status, Status::Valid);
    assert!(issue_vector(2).is_empty());
    assert_eq!(original_report.checks[3].diagnostic.status, Status::Valid);
    assert_eq!(original_report.checks[4].diagnostic.status, Status::Invalid);
    assert_eq!(
        issue_vector(4),
        vec![(
            vec![
                PathSegment::Property("files".into()),
                PathSegment::Index(0),
                PathSegment::Property("resources".into()),
                PathSegment::Index(0),
                PathSegment::Property("locator".into()),
                PathSegment::Property("height_points".into()),
            ],
            Reason::ExclusiveMinimum,
            "exclusiveMinimum".into(),
        )]
    );
    assert_eq!(original_report.checks[5].diagnostic.status, Status::Invalid);
    assert_eq!(
        issue_vector(5),
        vec![(
            vec![
                PathSegment::Property("files".into()),
                PathSegment::Index(0),
                PathSegment::Property("resources".into()),
                PathSegment::Index(0),
                PathSegment::Property("resource_kind".into()),
            ],
            Reason::Enum,
            "enum".into(),
        )]
    );
    assert_eq!(original_report.checks[6].diagnostic.status, Status::Valid);
    assert!(issue_vector(0).is_empty());
    assert!(issue_vector(3).is_empty());
    assert!(issue_vector(6).is_empty());

    // Derive a separate adversarial cut from the authentic selected contract.
    // Its one unknown assertion sits beside a real $ref inside the nested
    // pdf-profile branch, so the same worker must return Indeterminate rather
    // than laundering unsupported schema semantics into valid or invalid.
    let mut unsupported_schema: Value =
        serde_json::from_slice(&contracts[INVENTORY_SCHEMA]).unwrap();
    let nested_branch_ref = unsupported_schema
        .pointer_mut(
            "/$defs/fileInventory/allOf/1/then/properties/resources/items/properties/label_fingerprint",
        )
        .and_then(Value::as_object_mut)
        .unwrap();
    assert_eq!(
        nested_branch_ref.get("$ref").and_then(Value::as_str),
        Some("#/$defs/fingerprint")
    );
    assert!(
        nested_branch_ref
            .insert(
                "x-conformance-unsupported-assertion".into(),
                Value::Bool(true)
            )
            .is_none()
    );
    let mut derived_contracts = contracts.clone();
    derived_contracts.insert(
        INVENTORY_SCHEMA.into(),
        serde_json::to_vec(&unsupported_schema).unwrap(),
    );
    assert_ne!(
        Digest256::of_bytes(&contracts[INVENTORY_SCHEMA]),
        Digest256::of_bytes(&derived_contracts[INVENTORY_SCHEMA])
    );
    let derived_store = temporary.path().join("unsupported-schema-cut");
    let derived_revision = write_cut_store(&derived_contracts, &derived_store);
    assert_ne!(derived_revision, original_revision);
    let derived_schemas = source_foundation_schema_set_from_fixture(
        &derived_store,
        derived_revision,
        limits,
        deadline,
        &cancelled,
    );
    assert_ne!(
        derived_schemas.schema_set_sha256(),
        original_schemas.schema_set_sha256()
    );
    let indeterminate_location = "fixture/inventory/nested-branch-ref-unsupported";
    let indeterminate_check = [SourceFoundationMixedSchemaInput::Legacy(
        SourceFoundationLegacySchemaInput {
            location: indeterminate_location,
            contract: INVENTORY_SCHEMA,
            // NaN forces the exceptional evaluator, so the nested unknown
            // assertion is examined instead of being ignored by the finite
            // jsonschema backend.
            raw_instance: &raw_nan_width,
        },
    )];
    let expected_indeterminate = BatchCoverageExpectation::from_units(&[BatchUnit {
        ordinal: 0,
        member_id: "source-foundation-schema:0".into(),
        relative_path: indeterminate_location.into(),
        root_uri: inventory_uri.into(),
        raw_instance: raw_nan_width.clone(),
    }])
    .unwrap();
    let (indeterminate_report, reason) = match evaluate_source_foundation_mixed_schema_checks(
        &derived_schemas,
        &worker,
        &indeterminate_check,
        limits,
        deadline,
        &cancelled,
    ) {
        SourceFoundationSchemaOutcome::Incomplete { report, reason } => (report, reason),
        SourceFoundationSchemaOutcome::Complete(report) => {
            panic!("unsupported nested schema closure was accepted: {report:?}")
        }
    };
    assert_eq!(
        reason,
        SourceFoundationSchemaFailure::UnsupportedInputSemantics,
        "incomplete worker exchange context: {:?}",
        indeterminate_report.exchange_failure_context()
    );
    assert!(!indeterminate_report.is_complete());
    assert!(!indeterminate_report.is_valid());
    assert_eq!(indeterminate_report.source_revision, derived_revision);
    assert_eq!(
        indeterminate_report.schema_set_sha256,
        derived_schemas.schema_set_sha256()
    );
    assert_eq!(indeterminate_report.checkpoints.len(), 1);
    assert_eq!(indeterminate_report.checkpoints[0].completed_count, 1);
    assert_eq!(
        indeterminate_report.checkpoints[0].ordered_manifest_sha256,
        expected_indeterminate.ordered_manifest_sha256
    );
    assert_eq!(indeterminate_report.checks.len(), 1);
    assert_eq!(
        indeterminate_report.checks[0].diagnostic.status,
        Status::Indeterminate
    );
    assert_eq!(
        indeterminate_report.checks[0].diagnostic.failure,
        tos_validation::executor::schema_diagnostics::Failure::UnsupportedInputSemantics
    );
}

#[test]
fn maintained_assessment_whole_output_matches_native_current_view() {
    use tos_validation::assessment::{
        AssessmentLimits, AssessmentReadInput, AssessmentRecordInput, AssessmentRefusal,
        AssessmentSourceRoute, AssessmentSubmissionInput, evaluate_current_assessment,
    };
    use tos_validation::executor::BatchBudget;

    let cases = crate::frozen_legacy_python_oracle("assessment-whole-view");
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
    let image =
        super::command_form_cases::schema_image(ExecutorBudget::laboratory(), deadline, &cancelled);
    let worker_digest = image.identity().sha256;
    let mut schemas = CutWorkerSchemaExecutor::from_cut_with_image(
        &cut,
        FormatProfile::AssertedSourceCandidateV1,
        &image,
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
            layer_quality: None,
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
