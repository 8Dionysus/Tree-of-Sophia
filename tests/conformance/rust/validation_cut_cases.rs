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
use tos_validation::record_rules::{RecordFamily, RecordSchema};
use tos_validation::source_cut::{
    CutWorkerLimits, CutWorkerSchemaExecutor, MetadataOnlyPayloads, inspect_items_from_cut,
};

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
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("revisions")).unwrap();
    let members: Vec<_> = files
        .iter()
        .map(|(path, raw)| {
            let sha = Digest256::of_bytes(raw).to_hex();
            fs::write(root.join("objects").join(&sha), raw).unwrap();
            serde_json::json!({"path":path,"sha256":sha,"size_bytes":raw.len(),"mode":420})
        })
        .collect();
    let mut manifest = serde_json::json!({"schema_version":"tos_corpus_snapshot_v1",
        "base_revision":null,"files":members,"identities":{},"dependencies":{},
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
        "ToS/contracts/artifact-v2.schema.json",
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
    let files = selected_item_sources();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    let revision = write_cut_store(&files, &root);
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
                max_members: 512,
                max_total_bytes: 8_388_608,
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
    let report = inspect_items_from_cut(
        &cut,
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
        report.item_family.issues.is_empty(),
        "{:?}",
        report.item_family.issues
    );
    assert_eq!(report.item_family.unavailable_payloads, 1);
    assert!(!report.item_family.source_admission_complete);
    assert_eq!(report.carrier_membership.count, files.len() as u64);
    assert!(!schemas.receipts().is_empty());
    for receipt in schemas.receipts() {
        assert_eq!(receipt.source_revision, revision);
        assert_eq!(
            receipt.source_raw_sha256,
            Digest256::of_bytes(&files[&receipt.path])
        );
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
        &["scripts/build_provenance_event_v2_lab.py"],
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
    assert!(schemas.receipts().iter().all(|receipt| receipt.valid
        && receipt.execution.worker_sha256 == digest
        && receipt.source_revision == revision));
}
