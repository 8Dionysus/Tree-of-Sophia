use super::*;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use tempfile::TempDir;

const FIXTURE_TIME: &str = "2026-09-21T12:00:00Z";

struct Fixture {
    _temp: TempDir,
    repo: PathBuf,
    metadata: PathBuf,
    manifest_path: PathBuf,
    output: PathBuf,
    bodies: BTreeMap<String, Vec<u8>>,
    manifest_sha: String,
}

impl Fixture {
    fn new(count: usize) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let metadata = root.join("metadata");
        fs::create_dir(&metadata).unwrap();
        let manifest_path = root.join("selection.json");
        let output = root.join("handoff");
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap();

        let mut selections = Vec::new();
        let mut all_record_refs = BTreeSet::new();
        let mut all_payload_refs = BTreeSet::new();
        let mut bodies = BTreeMap::new();
        for index in 0..count {
            let slug = format!("fixture-{index}");
            let item_ref = format!("tos.item.fixture.{index}");
            let item_root = format!(
                "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/{slug}"
            );
            let item_record_ref = format!("{item_root}/item.json");
            let item_manifest_ref = format!("{item_root}/item.manifest.json");
            let rights_ref = format!("{item_root}/rights.json");
            let provenance_ref = format!("{item_root}/provenance.jsonl");
            let forensic_ref = format!("{item_root}/forensic-report.md");
            let inventory_ref = format!("{item_root}/resource-inventory.json");
            let fixity_ref = format!("{item_root}/fixity.sha256");
            let relative_payload = format!("payload/{slug}.txt");
            let payload_body = format!("payload {index}\n").into_bytes();
            let payload_sha = sha(&payload_body);
            let file_ref = format!("tos.file.sha256.{payload_sha}");
            let acquisition_event = format!("tos.event.acquisition.fixture-{index}");
            let inventory_event = format!("tos.event.inventory.fixture-{index}");

            let rights = json!({
                "schema_version":"tos_rights_record_v1",
                "rights_id":format!("tos.rights.fixture.{index}"),
                "scope_refs":[item_ref,file_ref],
                "assessment_status":"not_assessed",
                "jurisdictions_reviewed":[],
                "source_refs":[format!("https://provider.example/{slug}/rights")],
                "permissions":[],
                "restrictions":[],
                "visibility":"local_only",
                "redistribution_posture":"not_authorized",
                "derivative_posture":"local_research_only",
                "assessed_by":{"maker_type":"imported_source","agent_ref":"software:fixture-acquisition"},
                "assessed_at":FIXTURE_TIME,
                "rationale":"Fixture rights metadata only.",
                "review_status":"unreviewed",
                "record_version":1
            });
            let item_manifest = json!({
                "schema_version":"tos_source_item_manifest_v1",
                "item_id":item_ref,
                "item_kind":"born_digital",
                "embodiment_ref":format!("tos.edition.fixture.{index}"),
                "storage_posture":"local_gitignored_payload",
                "payload_files":[{
                    "file_id":file_ref,"relative_path":relative_payload,"original_basename":format!("{slug}.txt"),
                    "media_type":"text/plain","byte_size":payload_body.len(),"sha256":payload_sha,
                    "fixity_verified_at":FIXTURE_TIME
                }],
                "acquisition_event_ref":acquisition_event,
                "rights_ref":rights_ref,
                "provenance_ref":provenance_ref,
                "forensic_report_ref":forensic_ref,
                "resource_inventory_ref":inventory_ref,
                "visibility":"local_only",
                "manifest_version":1
            });
            let inventory = json!({
                "$schema":"https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json",
                "schema_version":"tos_source_resource_inventory_v1",
                "item_id":item_ref,
                "generated_from_manifest_ref":item_manifest_ref,
                "inventory_authority":"mechanical_metadata_only",
                "source_text_included":false,
                "files":[{
                    "file_id":file_ref,"file_sha256":payload_sha,"media_type":"text/plain","profile":"plain_text_v1",
                    "summary":{"resource_count":1},
                    "resources":[{
                        "resource_id":"fixture-resource","resource_kind":"plain_text_file",
                        "locator":{"container_order":1},"structural_role":"member",
                        "content_fingerprint":{"algorithm":"sha256","normalization":"unicode-codepoints-preserved",
                            "sha256":payload_sha,"character_count":payload_body.len()}
                    }]
                }],
                "generator":{"name":"build_source_resource_inventories.py","version":"1"},
                "provenance_event_ref":inventory_event,
                "inventory_version":1,
                "authority_boundary":"Fixture metadata only; no source text is included."
            });
            let item = json!({
                "schema_version":"tos_corpus_record_v1","record_type":"item","record_id":item_ref,
                "preferred_label":format!("Fixture Item {index}"),"identity_status":"provisional",
                "source_refs":[item_manifest_ref],"external_identifiers":[],"same_as_posture":"no_equivalence_claim",
                "item_manifest_ref":item_manifest_ref,"record_version":1
            });
            let acquisition = json!({
                "schema_version":"tos_provenance_event_v1","event_id":acquisition_event,
                "event_type":"acquisition","status":"completed","event_version":1,
                "started_at":FIXTURE_TIME,"ended_at":FIXTURE_TIME,"agent_refs":["software:fixture-acquisition"],
                "inputs":[],"method":{"maker_type":"software","name":"fixture-acquisition","version":"1","configuration":{}},
                "rights_basis_ref":rights_ref,
                "outputs":[{"ref":format!("{item_root}/{relative_payload}"),"role":"immutable-acquired-source-file","sha256":payload_sha}]
            });
            let inventory_provenance = json!({
                "schema_version":"tos_provenance_event_v1","event_id":inventory_event,
                "event_type":"forensic_inspection","started_at":FIXTURE_TIME,"ended_at":FIXTURE_TIME,
                "agent_refs":["software:tos-source-item-commands"],"inputs":[],
                "outputs":[{"ref":inventory_ref,"role":"tracked_text_free_resource_inventory","sha256":sha(&json_line(&inventory))}],
                "method":{"maker_type":"software","name":"build_source_resource_inventories.py","version":"1","configuration":{}},
                "status":"completed","event_version":1
            });
            let mut provenance = json_line(&acquisition);
            provenance.extend(json_line(&inventory_provenance));
            let forensic = b"Fixture forensic report; no interpretation was accepted.\n".to_vec();
            let fixity = format!("{payload_sha}  {relative_payload}\n").into_bytes();
            let mut record_rows = Vec::new();
            for (reference, kind, bytes) in [
                (item_record_ref.as_str(), "item", json_bytes(&item)),
                (
                    item_manifest_ref.as_str(),
                    "manifest",
                    json_line(&item_manifest),
                ),
                (rights_ref.as_str(), "rights", json_bytes(&rights)),
                (provenance_ref.as_str(), "provenance", provenance),
                (forensic_ref.as_str(), "discovery", forensic),
                (inventory_ref.as_str(), "discovery", json_line(&inventory)),
                (fixity_ref.as_str(), "discovery", fixity),
            ] {
                write_metadata(&metadata, reference, &bytes);
                record_rows.push(json!({"ref":reference,"kind":kind,"sha256":sha(&bytes)}));
                all_record_refs.insert(reference.to_owned());
            }
            let provider_url = format!("https://provider.example/{slug}/r1/{slug}.txt");
            bodies.insert(file_ref.clone(), payload_body.clone());
            all_payload_refs.insert(file_ref.clone());
            selections.push(json!({
                "item_ref":item_ref,"item_root_ref":item_root,
                "provider":{"name":"fixture-provider","revision":"r1","source_id":"fixture-collection-v1","source_url":"https://provider.example/fixture-collection-v1"},
                "records":record_rows,
                "rights":{"ref":rights_ref,"sha256":sha(&json_bytes(&rights)),"posture":"local_only"},
                "payload_files":[{
                    "item_ref":item_ref,"file_ref":file_ref,"item_root_ref":item_root,"relative_path":relative_payload,
                    "byte_size":payload_body.len(),"sha256":payload_sha,"provider_url":provider_url,
                    "provider_revision":"r1","provider_source_id":"fixture-collection-v1","media_type":"text/plain"
                }]
            }));
        }
        let base_revision = "a".repeat(64);
        let manifest = json!({
            "$schema":"https://tree-of-sophia.local/ToS/contracts/acquisition-batch.schema.json",
            "schema_version":"tos_acquisition_batch_v1","batch_id":"tos.acquisition-batch.fixture-20260921",
            "batch_revision":1,"prepared_at":FIXTURE_TIME,"base_revision":base_revision,
            "selection":selections,
            "provenance_delta":{
                "event_ref":"tos.event.acquisition-batch.fixture-20260921","event_version":1,
                "change_kind":"batch_delta","base_revision":base_revision,
                "record_refs":all_record_refs.into_iter().collect::<Vec<_>>(),
                "payload_file_refs":all_payload_refs.into_iter().collect::<Vec<_>>(),
                "supersedes_event_ref":null
            }
        });
        let manifest_bytes = pretty_line(&manifest);
        fs::write(&manifest_path, &manifest_bytes).unwrap();
        Self {
            _temp: temp,
            repo,
            metadata,
            manifest_path,
            output,
            bodies,
            manifest_sha: sha(&manifest_bytes),
        }
    }

    fn manifest(&self) -> Value {
        parse(&read_bytes(&self.manifest_path, None, false, false, 16 * 1024 * 1024).unwrap())
            .unwrap()
    }

    fn write_manifest(&mut self, manifest: &Value) {
        let bytes = pretty_line(manifest);
        fs::write(&self.manifest_path, &bytes).unwrap();
        self.manifest_sha = sha(&bytes);
    }

    fn add_claim(&mut self) -> (String, Value) {
        let mut manifest = self.manifest();
        let selection = &mut manifest["selection"][0];
        let item_root = selection["item_root_ref"].as_str().unwrap().to_owned();
        let claim_ref = format!("{item_root}/source-claims.jsonl");
        let claim = json!({
            "schema_version":"tos_source_relation_claim_v1",
            "claim_id":"tos.claim.acquisition-profile-fixture",
            "claim_type":"relation",
            "assertion_layer":"bibliographic_assertion",
            "predicate":"has_expression",
            "subject_ref":"tos.work.acquisition-profile-fixture",
            "object":"tos.expression.acquisition-profile-fixture",
            "claim_version":1,
            "review_status":"unreviewed",
            "visibility":"public_metadata_only",
            "epistemic_status":"observed",
            "polarity":"positive",
            "evidence_refs":[format!("{item_root}/item.json"),format!("{item_root}/item.manifest.json")],
            "assessment_refs":[],
            "maker":{"maker_type":"model","agent_ref":"model:fixture"},
            "provenance_event_ref":"tos.event.acquisition-profile-fixture",
            "qualifiers":{"statement":"Synthetic profile fixture only.","statement_language":"en","statement_script":"Latn","limits":"No claim is admitted."}
        });
        let claim_bytes = json_line(&claim);
        write_metadata(&self.metadata, &claim_ref, &claim_bytes);
        selection["records"].as_array_mut().unwrap().push(json!({
            "ref":claim_ref,"kind":"claim","sha256":sha(&claim_bytes)
        }));
        let mut refs = manifest["selection"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|selected| selected["records"].as_array().unwrap())
            .map(|record| record["ref"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        refs.sort();
        manifest["provenance_delta"]["record_refs"] = json!(refs);
        self.write_manifest(&manifest);
        (claim_ref, claim)
    }

    fn acquire(
        &self,
        output: &Path,
        fetch: &mut impl FnMut(&Value) -> Result<Vec<u8>>,
    ) -> Result<Value> {
        let context = load_manifest(&self.manifest_path, &self.repo, Some(&self.manifest_sha))?;
        acquire(&context, &self.metadata, output, 1, fetch)
    }
}

fn json_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn json_line(value: &Value) -> Vec<u8> {
    let mut bytes = json_bytes(value);
    bytes.push(b'\n');
    bytes
}

fn pretty_line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn write_metadata(root: &Path, reference: &str, bytes: &[u8]) {
    let path = root.join(reference);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn native_batch_failure_resume_and_immutable_custody() {
    let fixture = Fixture::new(2);
    let manifest = fixture.manifest();
    let urls = manifest["selection"]
        .as_array()
        .unwrap()
        .iter()
        .map(|selection| {
            selection["payload_files"][0]["file_ref"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    let mut calls = Vec::new();
    let first = fixture
        .acquire(&fixture.output, &mut |payload| {
            let file_ref = payload["file_ref"].as_str().unwrap().to_owned();
            calls.push(file_ref.clone());
            if file_ref == urls[0] {
                Err("isolated fixture failure".into())
            } else {
                Ok(fixture.bodies[&file_ref].clone())
            }
        })
        .unwrap();
    assert_eq!(first["status"], "partially-acquired-not-admitted");
    assert_eq!(calls.len(), 2);
    assert_eq!(verify_local_shape(&fixture).unwrap(), "incomplete");

    calls.clear();
    let result = fixture
        .acquire(&fixture.output, &mut |payload| {
            let file_ref = payload["file_ref"].as_str().unwrap().to_owned();
            calls.push(file_ref.clone());
            Ok(fixture.bodies[&file_ref].clone())
        })
        .unwrap();
    assert_eq!(result["status"], "acquired-not-admitted");
    assert_eq!(calls, vec![urls[0].clone()]);
    assert_eq!(verify_local_shape(&fixture).unwrap(), "verified");
    let handoff: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .output
                .join(&result["handoff_ref"].as_str().unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(handoff["admission_status"], "not-admitted");
    assert_eq!(handoff["publication_status"], "not-published");
    for path in payload_paths(&fixture) {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o7777,
            0o444
        );
    }
    for name in ["", "source", "payload", "receipts"] {
        assert_eq!(
            fs::metadata(fixture.output.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o700
        );
    }
    let payload = payload_paths(&fixture).remove(0);
    fs::set_permissions(&payload, fs::Permissions::from_mode(0o644)).unwrap();
    calls.clear();
    let conflict = fixture
        .acquire(&fixture.output, &mut |payload| {
            calls.push(payload["file_ref"].as_str().unwrap().to_owned());
            Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone())
        })
        .unwrap();
    assert_eq!(conflict["status"], "partially-acquired-not-admitted");
    assert!(calls.is_empty());
}

fn verify_local_shape(fixture: &Fixture) -> Result<String> {
    let context = load_manifest(
        &fixture.manifest_path,
        &fixture.repo,
        Some(&fixture.manifest_sha),
    )?;
    let result = invoke(
        &json!({"repo_root":fixture.repo,"operation":"verify_local","output_root":fixture.output}),
    )?;
    assert_eq!(
        result["status"],
        if result["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["status"] == "verified")
        {
            "verified"
        } else {
            "incomplete"
        }
    );
    assert_eq!(result["manifest_sha256"], context.manifest_sha256);
    Ok(result["status"].as_str().unwrap().to_owned())
}

fn payload_paths(fixture: &Fixture) -> Vec<PathBuf> {
    let context = load_manifest(
        &fixture.manifest_path,
        &fixture.repo,
        Some(&fixture.manifest_sha),
    )
    .unwrap();
    payloads(&context)
        .unwrap()
        .iter()
        .map(|payload| payload_path(&fixture.output.join("payload"), payload).unwrap())
        .collect()
}

#[test]
fn native_batch_rechecks_selection_roots_and_interrupted_recovery_before_fetch() {
    let fixture = Fixture::new(1);
    let mut calls = Vec::new();
    let bad_attempts = json!({
        "repo_root":fixture.repo,"operation":"acquire","manifest_path":fixture.manifest_path,
        "metadata_root":fixture.metadata,"output_root":fixture.output,
        "expected_manifest_sha256":fixture.manifest_sha,"max_attempts":0
    });
    assert!(invoke(&bad_attempts).is_err());
    assert!(!fixture.output.exists());

    let prepared = invoke(&json!({
        "repo_root":fixture.repo,"operation":"prepare","manifest_path":fixture.manifest_path,
        "metadata_root":fixture.metadata,"output_root":fixture.output,
        "expected_manifest_sha256":fixture.manifest_sha
    }))
    .unwrap();
    assert_eq!(prepared["status"], "prepared-not-acquired");
    let selected_rights = fixture.output.join("source").join(
        fixture.manifest()["selection"][0]["rights"]["ref"]
            .as_str()
            .unwrap(),
    );
    let original = fs::read(&selected_rights).unwrap();
    fs::write(&selected_rights, [original.as_slice(), b" "].concat()).unwrap();
    assert!(
        fixture
            .acquire(&fixture.output, &mut |payload| {
                calls.push(payload["file_ref"].as_str().unwrap().to_owned());
                Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone())
            })
            .is_err()
    );
    assert!(calls.is_empty());
    fs::write(&selected_rights, original).unwrap();
    fs::set_permissions(&fixture.output, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        fixture
            .acquire(&fixture.output, &mut |payload| {
                calls.push(payload["file_ref"].as_str().unwrap().to_owned());
                Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone())
            })
            .is_err()
    );
    assert!(calls.is_empty());
    fs::set_permissions(&fixture.output, fs::Permissions::from_mode(0o700)).unwrap();

    fs::remove_file(fixture.output.join("receipts/preparation.json")).unwrap();
    let result = fixture
        .acquire(&fixture.output, &mut |payload| {
            calls.push(payload["file_ref"].as_str().unwrap().to_owned());
            Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone())
        })
        .unwrap();
    assert_eq!(result["status"], "acquired-not-admitted");
    fs::remove_file(fixture.output.join("receipts/preparation.json")).unwrap();
    calls.clear();
    assert!(
        fixture
            .acquire(&fixture.output, &mut |payload| {
                calls.push(payload["file_ref"].as_str().unwrap().to_owned());
                Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone())
            })
            .is_err()
    );
    assert!(calls.is_empty());
    assert!(payload_paths(&fixture).iter().any(|path| path.is_file()));
}

#[test]
fn native_batch_refuses_hardlinked_and_symlink_payloads_without_following() {
    let fixture = Fixture::new(1);
    let mut fetch =
        |payload: &Value| Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone());
    fixture.acquire(&fixture.output, &mut fetch).unwrap();
    let payload = payload_paths(&fixture).remove(0);
    let outside = fixture._temp.path().join("outside.bin");
    fs::hard_link(&payload, &outside).unwrap();
    let checked = invoke(
        &json!({"repo_root":fixture.repo,"operation":"verify_local","output_root":fixture.output}),
    )
    .unwrap();
    assert_eq!(checked["status"], "incomplete");
    fs::remove_file(&outside).unwrap();
    let body = fs::read(&payload).unwrap();
    fs::remove_file(&payload).unwrap();
    fs::write(&outside, body).unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o444)).unwrap();
    symlink(&outside, &payload).unwrap();
    let checked = invoke(
        &json!({"repo_root":fixture.repo,"operation":"verify_local","output_root":fixture.output}),
    )
    .unwrap();
    assert_eq!(checked["status"], "incomplete");
}

#[test]
fn selected_claim_profile_is_checked_before_fetch_and_handoff() {
    let mut fixture = Fixture::new(1);
    let (claim_ref, mut claim) = fixture.add_claim();
    let mut calls = Vec::new();
    let valid = fixture
        .acquire(&fixture.output, &mut |payload| {
            calls.push(payload["file_ref"].as_str().unwrap().to_owned());
            Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone())
        })
        .unwrap();
    assert_eq!(valid["status"], "acquired-not-admitted");
    assert_eq!(calls.len(), 1);

    claim["predicate"] = json!("not_a_declared_source_relation");
    let claim_bytes = json_line(&claim);
    write_metadata(&fixture.metadata, &claim_ref, &claim_bytes);
    let mut manifest = fixture.manifest();
    let record = manifest["selection"][0]["records"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|record| record["ref"] == claim_ref)
        .unwrap();
    record["sha256"] = json!(sha(&claim_bytes));
    fixture.write_manifest(&manifest);

    let output = fixture._temp.path().join("invalid-selected-source-claim");
    calls.clear();
    let result = fixture.acquire(&output, &mut |payload| {
        calls.push(payload["file_ref"].as_str().unwrap().to_owned());
        Ok(fixture.bodies[payload["file_ref"].as_str().unwrap()].clone())
    });
    let error = result.unwrap_err();
    assert!(error.contains("source Claim carrier violates its profile"));
    assert!(calls.is_empty());
    assert!(
        fs::read_dir(output.join("payload"))
            .unwrap()
            .next()
            .is_none()
    );
    let receipt_names = fs::read_dir(output.join("receipts"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        !receipt_names
            .iter()
            .any(|name| name.starts_with("handoff-"))
    );
}

#[test]
fn native_batch_keeps_exact_manifest_bytes_after_wire_expansion() {
    let mut fixture = Fixture::new(1);
    let mut raw = fs::read(&fixture.manifest_path).unwrap();
    raw.extend(std::iter::repeat_n(b' ', 7 * 1024 * 1024));
    fs::write(&fixture.manifest_path, &raw).unwrap();
    fixture.manifest_sha = sha(&raw);
    let loaded = load_manifest(
        &fixture.manifest_path,
        &fixture.repo,
        Some(&fixture.manifest_sha),
    )
    .unwrap();
    assert_eq!(loaded.raw_manifest, raw);
    assert_eq!(
        loaded.manifest["schema_version"],
        "tos_acquisition_batch_v1"
    );
}
