//! Non-OCR owner-local initial TextLayer through the maintained default CLI.
//! Recovery reconstructs its genuine completed native stage, without a crash claim.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OWNER_TEXT_NATIVE_OWNER_PATHS: &[&str] = &[
    "rust/crates/tos-command/src/source_text_layer_native.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_text_unit_native.rs",
    "rust/crates/tos-command/src/source_text_layer_entry.rs",
    "rust/crates/tos-command/src/source_text_unit_entry.rs",
];

fn assert_authored_text_unchanged(root: &Path, expected: &BTreeMap<String, Vec<u8>>) {
    let actual = super::command_text_cases::authored_text_files(root);
    let paths = actual
        .keys()
        .chain(expected.keys())
        .collect::<std::collections::BTreeSet<_>>();
    let changed = paths
        .into_iter()
        .filter(|path| actual.get(*path) != expected.get(*path))
        .collect::<Vec<_>>();
    assert!(
        changed.is_empty(),
        "authored Text bytes changed at: {changed:?}"
    );
}

fn text_fixture(root: &Path) -> Value {
    let script = r#"
import json,sys
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts'),str(repository/'tests')]
import test_source_text_unit_commands as unit
import test_source_text_layer_commands as layer
class ExistingRoot:
    def __init__(self,*args,**kwargs): self.name=str(root)
    def cleanup(self): pass
original=unit.tempfile.TemporaryDirectory
unit.tempfile.TemporaryDirectory=ExistingRoot
try:
    case=layer.NativeLayerCommandTests(methodName='runTest')
    case.setUp()
finally:
    unit.tempfile.TemporaryDirectory=original
print(json.dumps({'public':str(case.public),'private':str(case.store),
    'context':str(case.context_path),'owner':str(case.owner),
    'source_ref':case.source_ref,'payload':str(case.payload),'content_sha256':layer.source._digest(case.content)[7:],
    'content_bytes':len(case.content),'config':case.config,
    'unit_config':case.seed.config,'unit_proposal':case.seed.proposal,
    'unit_layer_schema':unit.native.LAYER_CONFIG,
    'implementations':sorted(set(layer.layers.IMPLEMENTATIONS))},ensure_ascii=False,separators=(',',':')))
"#;
    let native_owner_paths = OWNER_TEXT_NATIVE_OWNER_PATHS;
    let captured = super::native_python_fixture(
        "owner-text-base",
        &[("source-root", root)],
        &native_owner_paths,
    );
    super::assert_native_python_fixture(&captured, script, &native_owner_paths);
    captured.packets.get("factory").unwrap().clone()
}

const PAGE_OCR_LAYER_ID: &str = "tos.text-layer.sid-cccccccccccccccccccccccccccccccc";
const PAGE_OCR_NOTICE: &str =
    "Synthetic test fixture only; no historical, linguistic or rights judgment.";
const PAGE_OCR_STORE_ID: &str = "sid-99999999999999999999999999999999";

struct OwnerOCRFixture {
    state: Value,
    signed_evidence: BTreeMap<String, Vec<u8>>,
}

fn page_fixture_sha256(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

fn page_fixture_read(path: &Path, max_bytes: u64) -> Vec<u8> {
    let metadata = path.symlink_metadata().unwrap_or_else(|error| {
        panic!(
            "selected page fixture input is unavailable: {} ({error})",
            path.display()
        )
    });
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert!(
        metadata.len() <= max_bytes,
        "selected page fixture input exceeds its bound"
    );
    let raw = fs::read(path).unwrap();
    assert_eq!(raw.len() as u64, metadata.len());
    raw
}

fn page_fixture_write(path: &Path, raw: &[u8], mode: u32) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, raw).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn page_fixture_write_json(path: &Path, value: &Value, mode: u32) -> Vec<u8> {
    let mut raw = serde_json::to_vec(value).unwrap();
    raw.push(b'\n');
    page_fixture_write(path, &raw, mode);
    raw
}

fn page_fixture_private_dir(private_root: &Path, relative: &Path) -> PathBuf {
    let uid = private_root.metadata().unwrap().uid();
    let mut current = private_root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            panic!("private fixture path must contain only ordinary components")
        };
        current.push(component);
        if !current.exists() {
            fs::create_dir(&current).unwrap();
        }
        let metadata = current.symlink_metadata().unwrap();
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        assert_eq!(metadata.uid(), uid);
        fs::set_permissions(&current, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(current.metadata().unwrap().mode() & 0o777, 0o700);
    }
    current
}

fn page_fixture_expiry_one_hour() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3_600;
    let days = (seconds / 86_400) as i64;
    let daytime = (seconds % 86_400) as i64;
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    let hour = daytime / 3_600;
    let minute = daytime % 3_600 / 60;
    let second = daytime % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn page_fixture_copy_tree(source: &Path, destination: &Path, budget: &mut (usize, u64)) {
    let metadata = source.symlink_metadata().unwrap();
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        if !destination.exists() {
            fs::create_dir(destination).unwrap();
        }
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            page_fixture_copy_tree(&entry.path(), &destination.join(entry.file_name()), budget);
        }
        fs::set_permissions(destination, metadata.permissions()).unwrap();
    } else {
        assert!(metadata.is_file() && metadata.len() <= 2_097_152);
        budget.0 += 1;
        budget.1 = budget.1.checked_add(metadata.len()).unwrap();
        assert!(budget.0 <= 512 && budget.1 <= 8_388_608);
        fs::copy(source, destination).unwrap();
        let copied = page_fixture_read(destination, 2_097_152);
        assert_eq!(copied, page_fixture_read(source, 2_097_152));
        fs::set_permissions(destination, metadata.permissions()).unwrap();
    }
}

fn page_fixture_copy_contracts(repository: &Path, public: &Path) {
    let source = repository.join("ToS/contracts");
    let mut entries: Vec<_> = fs::read_dir(&source)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".schema.json"))
        })
        .collect();
    entries.sort();
    for path in entries {
        assert!(path.is_file() && !path.symlink_metadata().unwrap().file_type().is_symlink());
        let relative = path.strip_prefix(repository).unwrap();
        let target = public.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::copy(path, target).unwrap();
    }
    for relative in [
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ] {
        let source = repository.join(relative);
        let target = public.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::copy(source, target).unwrap();
    }
}

fn page_owner_ocr_policy() -> Value {
    serde_json::json!({
        "schema_version":"tos_native_text_layer_derivation_policy_v1",
        "operation":"text-layer.record-owner-page-ocr",
        "method":"ocr",
        "encoding":"UTF-8-strict",
        "text_max_bytes":131072,
        "edits_max_count":128,
        "input_scope":"exact-source-anchor",
        "unicode_normalization":"none",
        "unicode_database_version":null,
        "edits":"not-applicable",
        "whitespace":"unchanged-except-explicit-edits-or-selected-Unicode-form",
        "result_origin":"authenticated-owner-execution-receipt",
        "provider_execution_verified":true,
        "source_layout_fidelity":"not-assessed",
        "quality_assessment":"not-performed",
        "inherited_quality":"not-transferred",
        "uncertainty":"supplied-none-recorded-is-not-reviewed-absence"
    })
}

fn page_owner_ocr_fixture(repository: &Path, root: &Path) -> OwnerOCRFixture {
    let public = root.join("page-public");
    let private = root.join("page-private");
    fs::create_dir(&public).unwrap();
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&public, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    page_fixture_copy_contracts(repository, &public);

    let descriptor_path = PathBuf::from(
        std::env::var_os("TOS_PRIVATE_JOURNAL_PAGE_INPUTS_JSON")
            .expect("retained genuine signed PageOCR input descriptor"),
    );
    assert!(descriptor_path.is_absolute());
    let descriptor_raw = page_fixture_read(&descriptor_path, 131_072);
    let descriptor: Value = serde_json::from_slice(&descriptor_raw).unwrap();
    assert_eq!(
        descriptor["schema"],
        "journal_page_native_consumer_inputs_v1"
    );
    let mut material = descriptor["material"].clone();
    let receipt_root = PathBuf::from(material["receipt_root"].as_str().unwrap());
    let mut signed_evidence = BTreeMap::new();
    let evidence_files = [
        (
            "receipt.json",
            "receipt_sha256",
            "owner-ocr-receipt.json",
            131_072,
        ),
        (
            "signature.sigstore.json",
            "signature_sha256",
            "owner-ocr-signature.sigstore.json",
            65_536,
        ),
        (
            "signer.pub",
            "public_key_sha256",
            "owner-ocr-signer.pub",
            4_096,
        ),
    ];
    for (source_name, digest_field, package_name, cap) in evidence_files {
        let raw = page_fixture_read(&receipt_root.join(source_name), cap);
        assert_eq!(
            page_fixture_sha256(&raw),
            material[digest_field].as_str().unwrap()
        );
        signed_evidence.insert(package_name.to_owned(), raw);
    }
    let receipt_raw = signed_evidence.get("owner-ocr-receipt.json").unwrap();
    let receipt: Value = serde_json::from_slice(receipt_raw).unwrap();
    assert_eq!(
        receipt["schema_version"],
        "tos_retained_pdf_page_ocr_execution_v1"
    );
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["returncode"], 0);
    assert_eq!(receipt["owner"]["source_ref"], material["owner_source_ref"]);
    assert_eq!(
        receipt["owner"]["adapter_sha256"],
        material["adapter_sha256"]
    );
    let binding = receipt["input_representation"].clone();
    assert_eq!(binding, material["input_representation"]);
    assert_ne!(binding["source_file_sha256"], binding["input_sha256"]);
    let scope = receipt["source_scope"].clone();
    assert_eq!(scope["item_ref"], "tos.item.synthetic.native-binding");
    assert_eq!(scope["work_ref"], "tos.work.synthetic.native-binding");
    assert_eq!(
        scope["expression_ref"],
        "tos.expression.synthetic.native-binding"
    );
    assert_eq!(scope["edition_ref"], "tos.edition.synthetic.native-binding");
    assert_eq!(binding["source_file_ref"], scope["file_ref"]);
    assert_eq!(binding["source_file_sha256"], scope["file_sha256"]);

    let source_pdf_path = PathBuf::from(descriptor["source_pdf"].as_str().unwrap());
    let source_pdf = page_fixture_read(&source_pdf_path, 131_072);
    assert!(source_pdf.len() <= 131_072 && source_pdf.starts_with(b"%PDF-"));
    assert_eq!(
        page_fixture_sha256(&source_pdf),
        scope["file_sha256"].as_str().unwrap()
    );
    let image_path = PathBuf::from(descriptor["image_path"].as_str().unwrap());
    assert!(image_path.is_absolute());
    let image = page_fixture_read(&image_path, 10 * 1024 * 1024);
    assert_eq!(image.len() as u64, binding["input_bytes"].as_u64().unwrap());
    assert_eq!(
        page_fixture_sha256(&image),
        binding["input_sha256"].as_str().unwrap()
    );
    assert_eq!(&image[..8], b"\x89PNG\r\n\x1a\n");

    let work_home = "ToS/source-witnesses/works/synthetic-native-binding";
    let expression_home = format!("{work_home}/expressions/und-synthetic");
    let edition_home = format!("{expression_home}/editions/synthetic-edition");
    let item_home = format!("{edition_home}/items/synthetic-item");
    let native_home = format!("{work_home}/technical-markup/synthetic-binding");
    let refs = BTreeMap::from([
        ("work", format!("{work_home}/work.json")),
        ("expression", format!("{expression_home}/expression.json")),
        ("edition", format!("{edition_home}/edition.json")),
        ("item", format!("{item_home}/item.json")),
    ]);
    let ids = BTreeMap::from([
        ("work", scope["work_ref"].as_str().unwrap()),
        ("expression", scope["expression_ref"].as_str().unwrap()),
        ("edition", scope["edition_ref"].as_str().unwrap()),
        ("item", scope["item_ref"].as_str().unwrap()),
    ]);
    let manifest_ref = format!("{item_home}/item.manifest.json");
    let rights_ref = format!("{item_home}/rights.json");
    let policy_ref = format!("{native_home}/synthetic-editorial-policy.json");
    let authority_ref = format!("{native_home}/synthetic-publication-authority.json");
    page_fixture_write_json(
        &public.join(&policy_ref),
        &serde_json::json!({"notice":PAGE_OCR_NOTICE}),
        0o644,
    );
    page_fixture_write_json(
        &public.join(&authority_ref),
        &serde_json::json!({"notice":PAGE_OCR_NOTICE}),
        0o644,
    );

    let original = b"<synthetic>Not a historical witness.</synthetic>\r\n";
    let original_digest = page_fixture_sha256(original);
    let original_id = format!("tos.file.sha256.{original_digest}");
    let original_ref = format!("{item_home}/payload/synthetic-original.xml");
    page_fixture_write(&public.join(&original_ref), original, 0o644);

    for (kind, reference) in &refs {
        let mut record = serde_json::json!({
            "schema_version":"tos_corpus_record_v1",
            "record_type":kind,
            "record_id":ids[*kind],
            "preferred_label":PAGE_OCR_NOTICE,
            "identity_status":"provisional",
            "source_refs":[policy_ref],
            "external_identifiers":[],
            "same_as_posture":"no_equivalence_claim",
            "record_version":1,
            "notes":PAGE_OCR_NOTICE
        });
        match *kind {
            "work" => record["expression_claim_refs"] = serde_json::json!([]),
            "expression" => {
                record["work_ref"] = serde_json::json!(ids["work"]);
                record["language"] = serde_json::json!("und");
                record["expression_role"] = serde_json::json!("source_language");
                record["responsibility_claim_refs"] = serde_json::json!([]);
                record["embodiment_claim_refs"] = serde_json::json!([]);
            }
            "edition" => {
                record["embodies_expression_refs"] = serde_json::json!([ids["expression"]]);
                record["publication_claim_refs"] = serde_json::json!([]);
                record["exemplar_claim_refs"] = serde_json::json!([]);
            }
            "item" => record["item_manifest_ref"] = serde_json::json!(manifest_ref),
            _ => panic!("unexpected page fixture source record kind"),
        }
        page_fixture_write_json(&public.join(reference), &record, 0o644);
    }

    let source_file_ref = scope["file_ref"].as_str().unwrap();
    let source_file_sha256 = scope["file_sha256"].as_str().unwrap();
    let mut rights = serde_json::json!({
        "schema_version":"tos_rights_record_v1",
        "rights_id":"tos.rights.synthetic.native-binding",
        "scope_refs":[ids["item"],original_id,source_file_ref],
        "assessment_status":"not_assessed",
        "jurisdictions_reviewed":[],
        "source_refs":[policy_ref],
        "permissions":[],
        "restrictions":[PAGE_OCR_NOTICE],
        "visibility":"local_only",
        "redistribution_posture":"not_authorized",
        "derivative_posture":"local_research_only",
        "assessed_by":{"maker_type":"model","agent_ref":"model:synthetic-fixture"},
        "assessed_at":"2026-09-08T00:00:00Z",
        "rationale":PAGE_OCR_NOTICE,
        "review_status":"unreviewed",
        "record_version":1
    });
    let pdf_relative = Path::new(&item_home)
        .strip_prefix("ToS/source-witnesses")
        .unwrap()
        .join("payload/source-page.pdf");
    let payload_root = root.join("page-payload-owner");
    fs::create_dir(&payload_root).unwrap();
    fs::set_permissions(&payload_root, fs::Permissions::from_mode(0o700)).unwrap();
    let selected_pdf = payload_root.join(&pdf_relative);
    page_fixture_write(&selected_pdf, &source_pdf, 0o600);
    let mut manifest = serde_json::json!({
        "schema_version":"tos_source_item_manifest_v1",
        "item_id":ids["item"],
        "item_kind":"born_digital",
        "embodiment_ref":ids["edition"],
        "storage_posture":"local_gitignored_payload",
        "payload_files":[
            {"file_id":original_id,"relative_path":"payload/synthetic-original.xml",
             "original_basename":"synthetic-original.xml","media_type":"application/xml",
             "byte_size":original.len(),"sha256":original_digest,"fixity_verified_at":"2026-09-08T00:00:00Z"},
            {"file_id":source_file_ref,"relative_path":"payload/source-page.pdf",
             "original_basename":"source-page.pdf","media_type":"application/pdf",
             "byte_size":source_pdf.len(),"sha256":source_file_sha256,
             "fixity_verified_at":"2026-09-08T00:00:00Z"}
        ],
        "acquisition_event_ref":"tos.event.synthetic.native-binding",
        "rights_ref":rights_ref,
        "provenance_ref":policy_ref,
        "forensic_report_ref":policy_ref,
        "resource_inventory_ref":policy_ref,
        "visibility":"local_only",
        "manifest_version":1
    });
    page_fixture_write_json(&public.join(&rights_ref), &rights, 0o644);
    page_fixture_write_json(&public.join(&manifest_ref), &manifest, 0o644);

    let store_prefix = format!("ToS/source-witnesses/owner-local/{PAGE_OCR_STORE_ID}/");
    let store_id = PAGE_OCR_STORE_ID;
    let context_path = root.join("page-context.json");
    let context = serde_json::json!({
        "schema_version":"tos_owner_local_source_context_v1",
        "public_root":public.to_str().unwrap(),
        "private_root":private.to_str().unwrap(),
        "private_prefix":store_prefix,
        "store_id":store_id
    });
    page_fixture_write_json(&context_path, &context, 0o600);
    let package_reference =
        format!("{store_prefix}layers/journal-page-current/source-text-layer.v1.json");
    page_fixture_private_dir(&private, Path::new(&package_reference).parent().unwrap());
    let rights_reference = format!("{store_prefix}rights/journal-page-current-layer.json");
    let rights_target = private.join(&rights_reference);
    page_fixture_private_dir(&private, Path::new(&rights_reference).parent().unwrap());
    rights["rights_id"] = serde_json::json!("tos.rights.synthetic.journal-page-current-layer");
    rights["scope_refs"] = serde_json::json!([PAGE_OCR_LAYER_ID]);
    page_fixture_write_json(&rights_target, &rights, 0o600);
    let rights_layer_digest = page_fixture_sha256(&fs::read(&rights_target).unwrap());

    let expiry = page_fixture_expiry_one_hour();
    let source_record_refs = serde_json::json!({
        "work":refs["work"],"expression":refs["expression"],
        "edition":refs["edition"],"item":refs["item"]
    });
    let mut source_record_sha256 = serde_json::Map::new();
    for (kind, reference) in &refs {
        source_record_sha256.insert(
            kind.to_string(),
            serde_json::json!(page_fixture_sha256(
                &fs::read(public.join(reference)).unwrap()
            )),
        );
    }
    let anchor_ref = format!("{store_prefix}source-page-anchor.current.json");
    let anchor_path = private.join(&anchor_ref);
    let mut anchor: Value = serde_json::from_slice(&page_fixture_read(
        &repository
            .join("tests/fixtures/native-text-binding/source-anchor-v2-abc/variant-b.anchor.json"),
        131_072,
    ))
    .unwrap();
    anchor["anchor_id"] = serde_json::json!("tos.anchor.sid-cccccccccccccccccccccccccccccccc");
    anchor["passage_id"] = Value::Null;
    anchor["resolution_status"] = serde_json::json!("locator_only");
    anchor["review_status"] = serde_json::json!("unreviewed");
    anchor["review_ref"] = Value::Null;
    anchor["target"]["item_id"] = serde_json::json!(scope["item_ref"]);
    anchor["target"]["file_id"] = serde_json::json!(scope["file_ref"]);
    anchor["target"]["file_sha256"] = serde_json::json!(scope["file_sha256"]);
    anchor["target"]["media_type"] = serde_json::json!("application/pdf");
    anchor["selector_payload"] = serde_json::json!({
        "kind":"selector_expression",
        "expression":{"mode":"single","selector":{
            "state":{"state_type":"digest_state",
                "representation_ref":format!("{item_home}/payload/source-page.pdf"),
                "representation_sha256":scope["file_sha256"],"media_type":"application/pdf"},
            "selector":{"type":"page_region","page_identity":{"page_number":binding["page_number"]},
                "x":0,"y":0,"width":1,"height":1,"coordinate_space":"normalized_0_1"}
        }}
    });
    page_fixture_write_json(&anchor_path, &anchor, 0o600);
    let anchor_digest = page_fixture_sha256(&fs::read(&anchor_path).unwrap());
    let source_access = serde_json::json!({
        "read_scope":"exact_acquired_file","access_allowed":true,
        "payload_root":payload_root.to_str().unwrap(),"byte_size":source_pdf.len(),
        "expires_at":expiry,"authority_ref":"test:owned-synthetic-exact-page-pdf"
    });
    let derivation_rights = serde_json::json!([
        {"ref":rights_ref,"sha256":page_fixture_sha256(&fs::read(public.join(&rights_ref)).unwrap())},
        {"ref":rights_reference,"sha256":rights_layer_digest}
    ]);
    material["authority_ref"] = serde_json::json!("test:current-authenticated-page-material");
    material["expires_at"] = serde_json::json!(expiry);
    let config = serde_json::json!({
        "schema_version":"tos_local_text_layer_record_owner_page_ocr_v1",
        "uid":private.metadata().unwrap().uid(),
        "principal_id":"test:journal-current-page-ocr",
        "authority_ref":"test:owned-synthetic-page-ocr-layer",
        "expires_at":expiry,
        "source_context_ref":context_path.to_str().unwrap(),
        "source_path":package_reference,
        "source_record_refs":source_record_refs,
        "source_record_sha256":source_record_sha256,
        "manifest_sha256":page_fixture_sha256(&fs::read(public.join(&manifest_ref)).unwrap()),
        "allowed_operations":["text-layer.record-owner-page-ocr"],
        "source_scope":scope,
        "identities":{"layer_id":PAGE_OCR_LAYER_ID,
            "provenance_event_id":"tos.event.sid-cccccccccccccccccccccccccccccccc"},
        "policy":page_owner_ocr_policy(),
        "language":"de",
        "source_access":source_access,
        "derivation_access":{"derivation_allowed":true,"content_visibility":"local_only",
            "operation":"ocr","expires_at":expiry,
            "authority_ref":"test:owned-synthetic-page-derivation",
            "rights_record_refs":derivation_rights},
        "maker":{"maker_type":"software","agent_ref":"test:journal-current-page-ocr",
            "method":"tos.owner-retained-page-ocr-record.v1","version":"1"},
        "limits":{"max_output_bytes":131072,"max_seconds":60},
        "input":{"kind":"retained_pdf_page","anchor":{"anchor_id":anchor["anchor_id"],
            "record_ref":anchor_ref,"record_sha256":anchor_digest}},
        "material":material
    });
    let owner = root.join("page-create-owner.json");
    page_fixture_write_json(&owner, &config, 0o600);
    let state = serde_json::json!({
        "public":public.to_str().unwrap(),"private":private.to_str().unwrap(),
        "context":context_path.to_str().unwrap(),"owner":owner.to_str().unwrap(),
        "assessment_owner":root.join("page-assessment-owner.json").to_str().unwrap(),
        "source_ref":package_reference,"expiry":expiry,"source_record_refs":source_record_refs,
        "payload_access":source_access,"image_path":image_path.to_str().unwrap(),
        "source_pdf_path":selected_pdf.to_str().unwrap(),"input_representation":binding,
        "original_receipt_sha256":material["receipt_sha256"],
        "original_signature_sha256":material["signature_sha256"],
        "original_owner_source_ref":material["owner_source_ref"]
    });
    OwnerOCRFixture {
        state,
        signed_evidence,
    }
}

fn page_assessment_fixture(repository: &Path, state: &Value) -> Value {
    let private = PathBuf::from(state["private"].as_str().unwrap());
    let source_reference = state["source_ref"].as_str().unwrap();
    let layer_path = private.join(source_reference);
    let layer_raw = fs::read(&layer_path).unwrap();
    let layer: Value = serde_json::from_slice(&layer_raw).unwrap();
    let layer_record_digest = page_fixture_sha256(&layer_raw);
    let canonical_layer = serde_json::to_vec(&layer).unwrap();
    let subject = serde_json::json!({
        "id":PAGE_OCR_LAYER_ID,
        "version":layer["layer_version"],
        "digest":Digest256::of_bytes(&canonical_layer).to_prefixed()
    });
    let selected = serde_json::json!({
        "record_ref":source_reference,"record_sha256":layer_record_digest,
        "layer_id":PAGE_OCR_LAYER_ID,"layer_version":layer["layer_version"]
    });
    let image = &state["input_representation"];
    let selection = serde_json::json!({
        "binding":{"schema_version":"tos_native_text_layer_binding_v1",
            "text_layer":selected,"source_record_refs":state["source_record_refs"]},
        "origin_id":"synthetic-current-retained-page-ocr",
        "source_access":{"read_scope":"exact_owner_local","access_allowed":true,
            "authority_ref":"test:owned-synthetic-page-source-metadata"},
        "payload_access":state["payload_access"],
        "comparison_profile":"tos_retained_page_ocr_image_comparison_v1",
        "image_access":{"read_scope":"exact_retained_page","access_allowed":true,
            "authority_ref":"test:owned-synthetic-exact-page-image","expires_at":state["expiry"],
            "path":state["image_path"],"byte_size":image["input_bytes"],
            "sha256":image["input_sha256"],"page_number":image["page_number"],
            "source_file_ref":image["source_file_ref"],
            "source_file_sha256":image["source_file_sha256"],"processing_boundary":"local_only",
            "width_pixels":image["width_pixels"],"height_pixels":image["height_pixels"]},
        "disclosure_access":null
    });
    let policy: Value = serde_json::from_slice(&page_fixture_read(
        &repository.join("ToS/doctrine/semantic-interchange/assessment-policy.v3.json"),
        131_072,
    ))
    .unwrap();
    let journal = private.join("page-new-journal");
    fs::create_dir(&journal).unwrap();
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o700)).unwrap();
    let owner = serde_json::json!({
        "schema_version":"tos_local_assessment_owner_v6",
        "uid":private.metadata().unwrap().uid(),
        "principal_id":"test:journal-current-page-comparison",
        "execution_profile":null,
        "policy":{"id":"tos.policy.knowledge-assessment","version":3,
            "payload":policy,"origin_id":null},
        "authorities":[],"competencies":[],"records":[],
        "subjects":{PAGE_OCR_LAYER_ID:{
            "record":subject,"assertion_layer":"textual_observation","risk":"low",
            "languages":["de"],"maker_id":layer["derivation"]["maker"]["agent_ref"],
            "requested_use":"text-layer:citation","access_allowed":true
        }},
        "journal_directory":journal.to_str().unwrap(),
        "source_context_ref":state["context"],
        "source_records":[],"owner_local_source_records":[],"native_text_units":[],
        "native_text_layers":[selection],"quality_dependencies":{}
    });
    let assessment_owner = PathBuf::from(state["assessment_owner"].as_str().unwrap());
    page_fixture_write_json(&assessment_owner, &owner, 0o600);
    let mut result = state.clone();
    result["subject"] = subject;
    result["image_sha256"] = serde_json::json!(image["input_sha256"]);
    result["layer_sha256"] = serde_json::json!(layer_record_digest);
    result["package"] = serde_json::json!(layer_path.parent().unwrap().to_str().unwrap());
    result
}

fn v6_owner_ocr_fixture(repository: &Path, root: &Path) -> OwnerOCRFixture {
    let base = PathBuf::from(
        std::env::var_os("TOS_PRIVATE_JOURNAL_V6_FIXTURE_ROOT")
            .expect("retained genuine signed synthetic OCR fixture root"),
    );
    let base = base.canonicalize().unwrap();
    let prefix = "ToS/source-witnesses/owner-local/sid-77777777777777777777777777777777/";
    let old_package = format!("{prefix}layers/raw-ocr/");
    let new_package = format!("{prefix}layers/journal-v6-retained-ocr/");
    let public = root.join("v6-public");
    let private = root.join("v6-private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let mut budget = (0usize, 0u64);
    page_fixture_copy_tree(&base.join("source"), &public, &mut budget);
    page_fixture_copy_contracts(repository, &public);
    let rights_relative = PathBuf::from(format!("{prefix}rights"));
    let rights_destination = page_fixture_private_dir(&private, &rights_relative);
    page_fixture_copy_tree(
        &base.join("private").join(prefix).join("rights"),
        &rights_destination,
        &mut (0, 0),
    );
    page_fixture_private_dir(&private, Path::new(&new_package).parent().unwrap());
    let package = private.join(&new_package);
    assert!(!package.exists() && !package.is_symlink());

    let context = root.join("v6-context.json");
    let context_value = serde_json::json!({
        "schema_version":"tos_owner_local_source_context_v1",
        "public_root":public.to_str().unwrap(),"private_root":private.to_str().unwrap(),
        "private_prefix":prefix,"store_id":"sid-77777777777777777777777777777777"
    });
    page_fixture_write_json(&context, &context_value, 0o600);
    let old_config_path = base.join("private").join(format!(
        "{old_package}source-create-owner-configuration.json"
    ));
    let mut config: Value =
        serde_json::from_slice(&page_fixture_read(&old_config_path, 131_072)).unwrap();
    let expiry = page_fixture_expiry_one_hour();
    let principal = "test:journal-v6-retained-evidence";
    config["uid"] = serde_json::json!(private.metadata().unwrap().uid());
    config["principal_id"] = serde_json::json!(principal);
    config["authority_ref"] = serde_json::json!("test:journal-v6-new-layer-selection");
    config["expires_at"] = serde_json::json!(expiry);
    config["source_context_ref"] = serde_json::json!(context.to_str().unwrap());
    config["source_path"] = serde_json::json!(format!("{new_package}source-text-layer.v1.json"));
    config["identities"] = serde_json::json!({
        "layer_id":"tos.text-layer.sid-99999999999999999999999999999999",
        "provenance_event_id":"tos.event.sid-99999999999999999999999999999999"
    });
    for name in ["source_access", "derivation_access", "material"] {
        config[name]["expires_at"] = serde_json::json!(expiry);
        config[name]["authority_ref"] = serde_json::json!(format!("test:journal-v6-{name}"));
    }
    config["maker"]["agent_ref"] = serde_json::json!(principal);

    let rights_ref = format!("{prefix}rights/journal-v6-synthetic-layer.json");
    let old_rights_path = base
        .join("private")
        .join(format!("{prefix}rights/synthetic-new-ocr-layer.json"));
    let mut rights: Value =
        serde_json::from_slice(&page_fixture_read(&old_rights_path, 131_072)).unwrap();
    rights["rights_id"] = serde_json::json!("tos.rights.synthetic.journal-v6-retained-ocr");
    rights["scope_refs"] =
        serde_json::json!(["tos.text-layer.sid-99999999999999999999999999999999"]);
    rights["assessed_by"] =
        serde_json::json!({"maker_type":"model","agent_ref":"test:journal-v6-fixture"});
    rights["assessment_status"] = serde_json::json!("not_assessed");
    rights["review_status"] = serde_json::json!("unreviewed");
    let rights_path = private.join(&rights_ref);
    let rights_raw = page_fixture_write_json(&rights_path, &rights, 0o600);
    for binding in config["derivation_access"]["rights_record_refs"]
        .as_array_mut()
        .unwrap()
    {
        if binding["ref"].as_str().unwrap_or("").starts_with(prefix) {
            binding["ref"] = serde_json::json!(rights_ref);
            binding["sha256"] = serde_json::json!(page_fixture_sha256(&rights_raw));
        }
    }
    let material = config["material"].clone();
    let receipt_root = PathBuf::from(material["receipt_root"].as_str().unwrap());
    let evidence_files = [
        (
            "receipt.json",
            "receipt_sha256",
            "owner-ocr-receipt.json",
            131_072,
        ),
        (
            "signature.sigstore.json",
            "signature_sha256",
            "owner-ocr-signature.sigstore.json",
            65_536,
        ),
        (
            "signer.pub",
            "public_key_sha256",
            "owner-ocr-signer.pub",
            4_096,
        ),
    ];
    let mut signed_evidence = BTreeMap::new();
    for (source_name, digest_name, package_name, cap) in evidence_files {
        let raw = page_fixture_read(&receipt_root.join(source_name), cap);
        assert_eq!(
            page_fixture_sha256(&raw),
            material[digest_name].as_str().unwrap()
        );
        signed_evidence.insert(package_name.to_owned(), raw);
    }
    let image_owner_path = base.join("synthetic-own-disclosed-owner.json");
    let image_owner: Value =
        serde_json::from_slice(&page_fixture_read(&image_owner_path, 1_048_576)).unwrap();
    let image_path = image_owner["native_text_layers"][0]["image_access"]["path"]
        .as_str()
        .unwrap();
    let owner = root.join("v6-create-owner.json");
    page_fixture_write_json(&owner, &config, 0o600);
    let state = serde_json::json!({
        "fixture_root":base.to_str().unwrap(),
        "public":public.to_str().unwrap(),"private":private.to_str().unwrap(),
        "context":context.to_str().unwrap(),"owner":owner.to_str().unwrap(),
        "assessment_owner":root.join("v6-assessment-owner.json").to_str().unwrap(),
        "source_ref":config["source_path"],"expiry":expiry,
        "original_receipt_sha256":material["receipt_sha256"],
        "original_signature_sha256":material["signature_sha256"],
        "original_owner_source_ref":material["owner_source_ref"],
        "image_path":image_path.to_owned()
    });
    OwnerOCRFixture {
        state,
        signed_evidence,
    }
}

fn v6_assessment_fixture(state: &Value, base: &Path) -> Value {
    let private = PathBuf::from(state["private"].as_str().unwrap());
    let source_ref = state["source_ref"].as_str().unwrap();
    let layer_path = private.join(source_ref);
    let layer_raw = fs::read(&layer_path).unwrap();
    let layer: Value = serde_json::from_slice(&layer_raw).unwrap();
    let canonical_layer = serde_json::to_vec(&layer).unwrap();
    let record = serde_json::json!({
        "id":"tos.text-layer.sid-99999999999999999999999999999999",
        "version":layer["layer_version"],
        "digest":Digest256::of_bytes(&canonical_layer).to_prefixed()
    });
    let mut owner: Value = serde_json::from_slice(&page_fixture_read(
        &base.join("synthetic-own-disclosed-owner.json"),
        1_048_576,
    ))
    .unwrap();
    owner["uid"] = serde_json::json!(private.metadata().unwrap().uid());
    owner["principal_id"] = serde_json::json!("test:journal-v6-read-only");
    let journal = private.join("v6-new-journal");
    fs::create_dir(&journal).unwrap();
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o700)).unwrap();
    owner["journal_directory"] = serde_json::json!(journal.to_str().unwrap());
    owner["source_context_ref"] = state["context"].clone();
    let image_sha256 = {
        let selection = &mut owner["native_text_layers"][0];
        let selected = &mut selection["binding"]["text_layer"];
        selected["layer_id"] = serde_json::json!(record["id"]);
        selected["layer_version"] = serde_json::json!(record["version"]);
        selected["record_ref"] = serde_json::json!(state["source_ref"]);
        selected["record_sha256"] = serde_json::json!(page_fixture_sha256(&layer_raw));
        for name in ["image_access", "payload_access"] {
            selection[name]["expires_at"] = state["expiry"].clone();
            selection[name]["authority_ref"] = serde_json::json!(format!("test:journal-v6-{name}"));
        }
        selection["source_access"]["authority_ref"] =
            serde_json::json!("test:journal-v6-private-source");
        selection["disclosure_access"] = Value::Null;
        selection["image_access"]["sha256"].clone()
    };
    let mut subject = owner["subjects"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    subject["record"] = record.clone();
    subject["maker_id"] = layer["derivation"]["maker"]["agent_ref"].clone();
    owner["subjects"] =
        serde_json::json!({"tos.text-layer.sid-99999999999999999999999999999999":subject});
    let assessment_owner = PathBuf::from(state["assessment_owner"].as_str().unwrap());
    page_fixture_write_json(&assessment_owner, &owner, 0o600);
    let mut selected_state = state.clone();
    selected_state["subject"] = record;
    selected_state["image_sha256"] = image_sha256;
    selected_state["layer_sha256"] = serde_json::json!(page_fixture_sha256(&layer_raw));
    selected_state["package"] = serde_json::json!(layer_path.parent().unwrap().to_str().unwrap());
    selected_state
}

fn package_files(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut total = 0usize;
    let files: BTreeMap<_, _> = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.path().symlink_metadata().unwrap();
            assert!(metadata.is_file() && metadata.len() <= 8_388_608);
            assert_eq!(metadata.mode() & 0o777, 0o600);
            let raw = fs::read(entry.path()).unwrap();
            total = total.checked_add(raw.len()).unwrap();
            assert!(total <= 12_582_912);
            (entry.file_name().to_str().unwrap().to_owned(), raw)
        })
        .collect();
    assert!(!files.is_empty() && files.len() <= 12);
    files
}

#[test]
fn native_owner_text_cli_extracts_replays_and_recovers_completed_stage() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    // Admitted E/C/W stay in their immutable locations; no image copies.
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let before_images: Vec<_> = images.iter().map(|p| custody(p)).collect();
    let fixture = text_fixture(temporary.path());
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let private = PathBuf::from(fixture["private"].as_str().unwrap());
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let source_ref = fixture["source_ref"].as_str().unwrap();
    let payload = PathBuf::from(fixture["payload"].as_str().unwrap());
    assert!(fs::metadata(&payload).unwrap().len() <= 8_388_608);
    let payload_raw = fs::read(&payload).unwrap();
    let context_raw = fs::read(&context).unwrap();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in OWNER_TEXT_NATIVE_OWNER_PATHS {
        let reference = *reference;
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 8_388_608);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,"assessment_schema_worker":null,
        "native_executable":images[1],"native_executable_sha256":before_images[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),
        "original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":before_images[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
            "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("native-text-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let observe = |request: &Value| {
        super::command_text_cases::native_owner_cli_observation(
            &repository,
            &owner,
            &invocation_path,
            request,
            deadline,
        )
    };
    let invoke = |request: &Value| -> Value {
        let (status, raw, errors) = observe(request);
        assert!(
            status.success(),
            "Text CLI: {}",
            String::from_utf8_lossy(&errors)
        );
        let envelope: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(
            envelope["schema_version"],
            "tos_local_native_source_result_v1"
        );
        assert_eq!(envelope["authentication"], "local-unix-account");
        assert_eq!(envelope["grants_admission"], false);
        let result = envelope["result"].clone();
        assert_eq!(
            result["schema_version"],
            "tos_local_text_layer_create_result_v1"
        );
        assert_eq!(result["content_disclosure"], "withheld");
        assert_eq!(result["grants_admission"], false);
        result
    };
    let described = invoke(&serde_json::json!({"operation":"describe"}));
    assert_eq!(described["target_exists"], false);
    assert_eq!(described["receipt_sha256"], Value::Null);
    let preview = invoke(&serde_json::json!({"operation":"prepare-create"}));
    assert_eq!(
        preview["owner_configuration"],
        described["owner_configuration"]
    );
    let request = serde_json::json!({"schema_version":"tos_local_source_command_v1",
        "operation":"text-layer.create","command_id":"native-owner-text-completed-stage",
        "expected_configuration":preview["owner_configuration"],
        "expected_dependencies":preview["expected_dependencies"],
        "expected_source":null,"expected_revision":null});
    let created = invoke(&request);
    assert_eq!(created["replayed"], false);
    let package = private.join(source_ref).parent().unwrap().to_path_buf();
    assert_eq!(fs::metadata(&package).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(&package).unwrap().uid(),
        fs::metadata(&private).unwrap().uid()
    );
    let retained = package_files(&package);
    let content = &retained["content.txt"];
    assert_eq!(
        content.len() as u64,
        fixture["content_bytes"].as_u64().unwrap()
    );
    assert_eq!(
        Digest256::of_bytes(content).to_hex(),
        fixture["content_sha256"].as_str().unwrap()
    );
    assert_eq!(
        created["receipt_sha256"],
        Digest256::of_bytes(
            retained["source-create-receipt.json"]
                .strip_suffix(b"\n")
                .expect("canonical receipt line")
        )
        .to_prefixed()
    );
    assert!(!public.join(source_ref).exists());
    assert_authored_text_unchanged(&public, &authored);
    // Fresh CLI process, same absolute protected root/config and same producer.
    let replay = invoke(&request);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["receipt_sha256"], created["receipt_sha256"]);
    assert_eq!(package_files(&package), retained);
    let after_description = invoke(&serde_json::json!({"operation":"describe"}));
    assert_eq!(after_description["target_exists"], true);
    assert_eq!(after_description["receipt_sha256"], Value::Null);
    let controls: Vec<_> = fs::read_dir(&private)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(".native-construction-")
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .ends_with(".pending")
        })
        .collect();
    assert_eq!(controls.len(), 1);
    let control = &controls[0];
    let plan = fs::read(control.join("plan.json")).unwrap();
    assert!(plan.len() <= 18_874_368);
    let output = control.join("output");
    assert!(!output.exists());
    // Genuine completed native package and original plan are moved back to
    // the existing pre-rename boundary; no provenance/control is fabricated.
    fs::rename(&package, &output).unwrap();
    fs::File::open(package.parent().unwrap())
        .unwrap()
        .sync_all()
        .unwrap();
    fs::File::open(control).unwrap().sync_all().unwrap();
    let owner_raw = fs::read(&owner).unwrap();
    let mut revoked: Value = serde_json::from_slice(&owner_raw).unwrap();
    revoked["allowed_operations"] = serde_json::json!([]);
    fs::write(&owner, serde_json::to_vec(&revoked).unwrap()).unwrap();
    assert!(!observe(&request).0.success());
    assert_eq!(package_files(&output), retained);
    assert_eq!(fs::read(control.join("plan.json")).unwrap(), plan);
    assert!(!package.exists());
    assert_authored_text_unchanged(&public, &authored);
    assert_eq!(fs::read(&context).unwrap(), context_raw);
    assert_eq!(fs::read(&payload).unwrap(), payload_raw);
    fs::write(&owner, &owner_raw).unwrap();
    fs::write(output.join("content.txt"), b"changed staged content").unwrap();
    let changed = package_files(&output);
    assert!(!observe(&request).0.success());
    assert_eq!(package_files(&output), changed);
    assert_eq!(fs::read(control.join("plan.json")).unwrap(), plan);
    assert!(!package.exists());
    assert_authored_text_unchanged(&public, &authored);
    assert_eq!(fs::read(&context).unwrap(), context_raw);
    assert_eq!(fs::read(&payload).unwrap(), payload_raw);
    fs::write(output.join("content.txt"), content).unwrap();
    let recovered = invoke(&request);
    assert_eq!(recovered["replayed"], true);
    assert_eq!(recovered["receipt_sha256"], created["receipt_sha256"]);
    assert_eq!(package_files(&package), retained);
    assert!(!output.exists());
    assert_eq!(fs::read(control.join("plan.json")).unwrap(), plan);
    assert_authored_text_unchanged(&public, &authored);
    assert_eq!(fs::read(&owner).unwrap(), owner_raw);
    assert_eq!(fs::read(&context).unwrap(), context_raw);
    assert_eq!(fs::read(&payload).unwrap(), payload_raw);
    assert_eq!(
        images.iter().map(|p| custody(p)).collect::<Vec<_>>(),
        before_images
    );
    assert!(Instant::now() < deadline);
}

/// The maintained Python fixture supplies synthetic authored input only.
/// Assessment/journal operations below always cross the captured native CLI.
#[test]
fn native_private_assessment_v4_append_replay_and_revocation_preserve_native_bytes() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    // This is a Rust-owned v4 setup derived from the captured native text-journal
    // fixture. The archive remains v5 evidence: only its in-memory owner config
    // is explicitly downgraded for the v4 compatibility route.
    let mut fixture = native_layer_journal_fixture(temporary.path(), false);
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let captured_v5: Value = serde_json::from_slice(&fs::read(&owner).unwrap()).unwrap();
    assert_eq!(
        captured_v5["schema_version"],
        "tos_local_assessment_owner_v5"
    );
    let mut config = captured_v5;
    config["schema_version"] = Value::from("tos_local_assessment_owner_v4");
    assert!(
        config
            .as_object_mut()
            .unwrap()
            .remove("quality_dependencies")
            .is_some()
    );
    // V4 has no layer-quality selection; the captured V5 seed remains intact.
    assert!(config.as_object_mut().unwrap().remove("native_text_layers").is_some());
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(config["schema_version"], "tos_local_assessment_owner_v4");
    assert!(config.get("quality_dependencies").is_none());
    assert_eq!(config["native_text_units"].as_array().unwrap().len(), 1);
    assert_eq!(
        config["owner_local_source_records"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let before_images: Vec<_> = images.iter().map(|path| custody(path)).collect();
    // The portable capture normalizes constructor paths. Build a new synthetic
    // V4 input from that relocated configuration, rebinding its exact byte
    // dependencies before the preservation baseline; the captured V5 archive
    // and its historical references remain unchanged.
    let context_value: Value = serde_json::from_slice(&fs::read(&context).unwrap()).unwrap();
    let private_root = PathBuf::from(context_value["private_root"].as_str().unwrap());
    let selection = &mut config["native_text_units"][0]["binding"];
    let layer_path = private_root.join(selection["text_layer"]["record_ref"].as_str().unwrap());
    let mut layer: Value = serde_json::from_slice(&fs::read(&layer_path).unwrap()).unwrap();
    let configuration = private_root.join(layer["derivation"]["maker"]["configuration_ref"].as_str().unwrap());
    let configuration_digest = Digest256::of_bytes(&fs::read(configuration).unwrap()).to_hex();
    layer["derivation"]["maker"]["configuration_digest"] = Value::from(configuration_digest.clone());
    for anchor_ref in layer["source_binding"]["anchors"].as_array_mut().unwrap() {
        let anchor_path = private_root.join(anchor_ref["anchor_record_ref"].as_str().unwrap());
        let mut anchor: Value = serde_json::from_slice(&fs::read(&anchor_path).unwrap()).unwrap();
        anchor["selector_method"]["configuration_digest"] = Value::from(configuration_digest.clone());
        let raw = canonical_json(&anchor);
        fs::write(&anchor_path, &raw).unwrap();
        anchor_ref["anchor_record_sha256"] = Value::from(Digest256::of_bytes(&raw).to_hex());
    }
    let layer_raw = canonical_json(&layer);
    fs::write(&layer_path, &layer_raw).unwrap();
    selection["text_layer"]["record_sha256"] = Value::from(Digest256::of_bytes(&layer_raw).to_hex());
    let setup_authored = super::command_text_cases::authored_text_files(&public);
    let setup_store = temporary.path().join("v4-setup-cut");
    let setup_revision = super::validation_cut_cases::write_cut_store(&setup_authored, &setup_store);
    let setup_cut = super::command_form_cases::open_cut(&setup_store, setup_revision, deadline, &cancelled);
    let selected_units = tos_foundation::parse_json(&serde_json::to_vec(&config["native_text_units"]).unwrap(),
        tos_foundation::JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let resolved = tos_command::resolve_native_text_units_for_conformance(
        &context, selected_units.root(), &setup_cut, &images[2], before_images[2].4, deadline, &cancelled,
    ).unwrap();
    for record in resolved.native_records {
        let record: Value = serde_json::from_slice(&tos_foundation::canonical_bytes_v1(
            &record, CanonicalProfile::SourceCommandInputV1, JsonLimits::default()).unwrap()).unwrap();
        let id = record["id"].as_str().unwrap();
        let reference = serde_json::json!({"id":id,"version":record["version"],
            "digest":Digest256::of_bytes(&command_binding_bytes(&record["payload"])).to_prefixed()});
        if let Some(subject) = config["subjects"].get_mut(id) { subject["record"] = reference.clone(); }
        if fixture["layer_subject"]["id"] == id {
            fixture["unit_template"]["assessments"][0]["evidence"] = serde_json::json!([
                {"record":reference,"stance":"supports","locator":"Exact synthetic source layer; no substantive assessment."}
            ]);
        }
        if fixture["unit_subject"]["id"] == id {
            fixture["unit_subject"] = reference.clone();
            fixture["unit_template"]["expected_subject"] = reference.clone();
            fixture["unit_template"]["assessments"][0]["subject"] = reference;
        }
    }
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();

    let preserved: BTreeMap<PathBuf, Vec<u8>> = fixture["preserved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| {
            let path = PathBuf::from(path.as_str().unwrap());
            assert!(path.symlink_metadata().unwrap().is_file());
            assert!(fs::metadata(&path).unwrap().len() <= 8_388_608);
            let raw = fs::read(&path).unwrap();
            (path, raw)
        })
        .collect();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    // Bounded changed feature sources, not a handler-mandated component schema.
    // The selected executable digest independently binds the actual native code.
    for reference in assessment_feature_sources() {
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 2_097_152);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
        assert!(Instant::now() < deadline);
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("assessment-source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,
        "native_executable":images[1],"native_executable_sha256":before_images[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),
        "original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":before_images[2].4.to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":before_images[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
            "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("assessment-native-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let unit = &fixture["unit_subject"];
    let unit_id = unit["id"].as_str().unwrap();
    let binding = &config["native_text_units"][0]["binding"];
    let private_marker = fixture["content"]
        .as_str()
        .unwrap()
        .chars()
        .take(8)
        .collect::<String>();
    let forbidden = [
        private_marker,
        fixture["source_member"].as_str().unwrap().to_owned(),
        binding["packet_ref"].as_str().unwrap().to_owned(),
        binding["packet_sha256"].as_str().unwrap().to_owned(),
        binding["text_layer"]["record_sha256"]
            .as_str()
            .unwrap()
            .to_owned(),
        "ordered_anchor_refs".to_owned(),
        "exact_sha256".to_owned(),
        "source_record_refs".to_owned(),
    ];
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
            "private assessment CLI: {}",
            String::from_utf8_lossy(&errors)
        );
        let response: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(response["schema_version"], "tos_local_assessment_result_v1");
        assert_eq!(response["visibility"], "local_only");
        assert_eq!(response["publication_authorized"], false);
        for marker in &forbidden {
            assert!(
                !String::from_utf8_lossy(&raw).contains(marker),
                "private native evidence disclosed in result"
            );
        }
        assert!(Instant::now() < deadline);
        response
    };
    let describe = serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
        "operation":"describe","subject_id":unit_id});
    let described = invoke(&describe);
    let mut request = fixture["unit_template"].clone();
    // Currentness comes from the selected native owner, not fixture/oracle output.
    request["expected_snapshot"] = described["owner_snapshot"].clone();
    request["expected_revision"] = Value::Null;
    let committed = invoke(&request);
    assert_eq!(committed["result"]["current_admission"]["can_use"], true);
    assert_eq!(
        committed["result"]["receipt"]["events"][0]["assessment"]["reviewer"]["kind"],
        "agent"
    );
    let replay = invoke(&request);
    assert_eq!(replay["result"]["replayed"], true);
    assert_eq!(replay["result"]["receipt"], committed["result"]["receipt"]);
    let mut config: Value = serde_json::from_slice(&fs::read(&owner).unwrap()).unwrap();
    let grant = &mut config["authorities"][0];
    grant["version"] = Value::from(grant["version"].as_u64().unwrap().checked_add(1).unwrap());
    grant["payload"]["authority_version"] = Value::from(
        grant["payload"]["authority_version"]
            .as_u64()
            .unwrap()
            .checked_add(1)
            .unwrap(),
    );
    grant["payload"]["state"] = Value::from("revoked");
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    request["expected_snapshot"] = invoke(&describe)["owner_snapshot"].clone();
    let revoked = invoke(&request);
    assert_eq!(revoked["result"]["replayed"], true);
    assert_eq!(
        revoked["result"]["receipt"]["admission_at_commit"]["can_use"],
        true
    );
    assert_eq!(revoked["result"]["current_admission"]["can_use"], false);
    for (path, raw) in preserved {
        assert_eq!(fs::read(path).unwrap(), raw);
    }
    assert_authored_text_unchanged(&public, &authored);
    for (index, path) in images.iter().enumerate() {
        assert_eq!(custody(path), before_images[index]);
    }
    assert!(Instant::now() < deadline);
}

fn assessment_feature_sources() -> [&'static str; 12] {
    [
        "rust/crates/tos-command/src/lib.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_native_private_cli.rs",
        "rust/crates/tos-command/src/source_native_private_assessment_cli.rs",
        "rust/crates/tos-command/src/source_assessment_journal.rs",
        "rust/crates/tos-command/src/source_private_assessment_sources.rs",
        "rust/crates/tos-command/src/source_private_assessment_layers.rs",
        "rust/crates/tos-command/src/source_private_claim.rs",
        "rust/crates/tos-command/src/source_sign.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-validation/src/assessment.rs",
    ]
}

fn native_layer_journal_fixture(root: &Path, derived: bool) -> Value {
    let template = r#"
import copy,json,sys,tempfile,unittest
from pathlib import Path
repository,root=map(Path,sys.argv[1:])
sys.dont_write_bytecode=True
sys.path[:0]=[str(repository/'mechanics/growth-cycle/tests'),str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts'),str(repository/'tests')]
from datetime import datetime,timedelta,timezone
import test_knowledge_assessment as policy_fixture
# Native commands use real time; retain a finite synthetic grant window.
policy_fixture.END=(datetime.now(timezone.utc)+timedelta(days=7)).isoformat()
class ExistingRoot:
    serial=0
    def __init__(self,*args,**kwargs):
        type(self).serial+=1
        path=root/('synthetic-layer-'+str(type(self).serial))
        path.mkdir(mode=0o700)
        self.name=str(path)
    def cleanup(self): pass
    def __enter__(self): return self.name
    def __exit__(self,*args): pass
original=tempfile.TemporaryDirectory
try:
    tempfile.TemporaryDirectory=ExistingRoot
    test=unittest.TestCase(methodName='runTest')
    import test_native_layer_quality_journal as maintained
    if JOURNAL_DERIVED_METHOD:
        from test_native_text_layer_assessment import NativeDerivedLayerAssessmentFixture
        # Late handler imports must use this exact fixture source tree.
        sys.path[:0]=[str(repository/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'),str(repository/'scripts')]
        layer=NativeDerivedLayerAssessmentFixture(test)
        layer.member=layer.seed.member
        layer.payload=layer.seed.payload
        fx=maintained.QualityJournalFixture(test,layer_fixture=layer)
    else:
        fx=maintained.QualityJournalFixture(test)
    # Keep these synthetic native grants finite and independent of Python's fixed NOW.
    for grant in fx.config['authorities']+fx.config['competencies']:
        grant['payload']['valid_until']='2099-01-01T00:00:00Z'
    for index,competency in enumerate(fx.config['competencies']):
        fx.config['authorities'][index]['payload']['competence_refs']=[maintained.Record.from_payload(**competency).ref]
    fx.save()
    assert fx.fx.layer['admission']['human_review_performed'] is False
    index=next(i for i,g in enumerate(fx.config['authorities']) if g['payload']['actor_id']==fx.config['principal_id'])
    def template(record,profile,name):
        review=copy.deepcopy(fx.policy_fixture.review(index).assessment)
        review.update(assessment_id='tos.review.'+name,subject=record.ref,policy=fx.policy.ref,profile_id=profile,
            decision='admit',limits=[],supersedes=[],authority=maintained.Record.from_payload(**fx.config['authorities'][index]).ref,
            competence=maintained.Record.from_payload(**fx.config['competencies'][index]).ref,evidence=[])
        return {'schema_version':'tos_local_assessment_command_v1','operation':'append','subject_id':record.id,
            'expected_subject':record.ref,'command_id':name,'assessments':[review]}
    result={'public':str(fx.fx.public),'context':str(fx.fx.context_path),'owner':str(fx.owner),
        'layer_subject':fx.layer_record.ref,'unit_subject':fx.unit.ref,
        'layer_template':template(fx.layer_record,'text-layer-quality','synthetic-native-layer-admit'),
        'unit_template':template(fx.unit,'source-observation','synthetic-native-unit-admit'),
        'source_member':fx.fx.member.decode(),'content':fx.fx.content.decode(),
        'derived':JOURNAL_DERIVED_METHOD,
        'preserved':([str(path) for path in sorted(fx.fx.store.rglob('*')) if path.is_file() and fx.journal not in path.parents] if JOURNAL_DERIVED_METHOD else [str(fx.fx.store/fx.fx.source_ref),str(fx.fx.store/fx.packet_ref),str(fx.fx.payload)])}
finally:
    tempfile.TemporaryDirectory=original
print(json.dumps(result,ensure_ascii=False,separators=(',',':')))
"#;
    let script = template.replace(
        "JOURNAL_DERIVED_METHOD",
        if derived { "True" } else { "False" },
    );
    let native_owner_paths = [
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-command/src/source_assessment_journal.rs",
        "rust/crates/tos-command/src/source_private_assessment_layers.rs",
        "rust/crates/tos-command/src/source_private_assessment_sources.rs",
        "rust/crates/tos-command/src/source_text_layer_native.rs",
    ];
    let id = if derived {
        "owner-text-derived-journal"
    } else {
        "owner-text-journal"
    };
    let captured = super::native_python_fixture(id, &[("journal-root", root)], &native_owner_paths);
    super::assert_native_python_fixture(&captured, &script, &native_owner_paths);
    captured.packets.get("factory").unwrap().clone()
}

fn native_layer_journal_case(derived: bool) {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let fixture = native_layer_journal_fixture(temporary.path(), derived);
    eprintln!(
        "native layer journal cost: phase=fixture elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let image_before: Vec<_> = images.iter().map(|path| custody(path)).collect();
    eprintln!(
        "native layer journal cost: phase=image-custody elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let preserved: BTreeMap<PathBuf, Vec<u8>> = fixture["preserved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let path = PathBuf::from(value.as_str().unwrap());
            assert!(path.symlink_metadata().unwrap().is_file());
            assert!(fs::metadata(&path).unwrap().len() <= 8_388_608);
            (path.clone(), fs::read(path).unwrap())
        })
        .collect();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in assessment_feature_sources() {
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 2_097_152);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
        assert!(Instant::now() < deadline);
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    eprintln!(
        "native layer journal cost: phase=software-capture elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let store = temporary.path().join("layer-journal-source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    eprintln!(
        "native layer journal cost: phase=captured elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,"native_executable":images[1],"native_executable_sha256":image_before[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":image_before[2].4.to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":image_before[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("layer-journal-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    // An unselected layer in a unit's closure remains supporting evidence.
    let configuration_raw = fs::read(&owner).unwrap();
    let mut supporting_only: Value = serde_json::from_slice(&configuration_raw).unwrap();
    supporting_only["native_text_layers"] = serde_json::json!([]);
    supporting_only["quality_dependencies"] = serde_json::json!({});
    fs::write(&owner, serde_json::to_vec(&supporting_only).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let (denied_status, _, denied_error) = super::command_text_cases::native_owner_cli_observation(
        &repository,
        &owner,
        &invocation_path,
        &serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"describe","subject_id":fixture["layer_subject"]["id"]}),
        deadline,
    );
    let denied_prefix = String::from_utf8_lossy(&denied_error[..denied_error.len().min(16_384)]);
    assert!(
        !denied_status.success(),
        "supporting-only native refusal unexpectedly succeeded: status={denied_status:?} stderr_bytes={} prefix={denied_prefix}",
        denied_error.len()
    );
    assert!(
        String::from_utf8_lossy(&denied_error)
            .contains("native supporting evidence is not an assessment target"),
        "supporting-only native refusal differs: status={denied_status:?} stderr_bytes={} prefix={denied_prefix}",
        denied_error.len()
    );
    fs::write(&owner, configuration_raw).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    eprintln!(
        "native layer journal cost: phase=supporting-refusal elapsed_ms={}",
        started.elapsed().as_millis()
    );
    let ordinal = std::cell::Cell::new(0usize);
    let invoke = |request: &Value| -> Value {
        let number = ordinal.get() + 1;
        ordinal.set(number);
        let step_started = Instant::now();
        eprintln!(
            "native layer journal cost: call={number} phase=start operation={} elapsed_ms={} remaining_ms={}",
            request["operation"].as_str().unwrap_or("<absent>"),
            started.elapsed().as_millis(),
            deadline.saturating_duration_since(step_started).as_millis()
        );
        let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
            &repository,
            &owner,
            &invocation_path,
            request,
            deadline,
        );
        eprintln!(
            "native layer journal cost: call={number} phase=terminal operation={} elapsed_ms={} child_ms={} success={}",
            request["operation"].as_str().unwrap_or("<absent>"),
            started.elapsed().as_millis(),
            step_started.elapsed().as_millis(),
            status.success()
        );
        assert!(
            status.success(),
            "native layer journal: {}",
            String::from_utf8_lossy(&errors)
        );
        let result: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(result["schema_version"], "tos_local_assessment_result_v1");
        assert_eq!(result["visibility"], "local_only");
        assert_eq!(result["publication_authorized"], false);
        assert!(Instant::now() < deadline);
        result
    };
    let describe = |subject: &Value| {
        invoke(
            &serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"describe","subject_id":subject["id"]}),
        )
    };
    let layer = describe(&fixture["layer_subject"]);
    assert!(
        layer["result"]["command_context"]["supported_operations"]
            .as_array()
            .unwrap()
            .contains(&Value::from("read-layer-comparison"))
    );
    let comparison = invoke(
        &serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"read-layer-comparison","subject_id":fixture["layer_subject"]["id"],"expected_subject":fixture["layer_subject"],"expected_snapshot":layer["owner_snapshot"]}),
    );
    assert!(
        !serde_json::to_string(&layer)
            .unwrap()
            .contains(fixture["content"].as_str().unwrap())
    );
    let comparison_payload = &comparison["result"]["source_comparison"]["payload"];
    if derived {
        assert_eq!(
            comparison_payload["schema_version"],
            "tos_native_text_layer_derivation_comparison_v1"
        );
        assert_eq!(
            comparison_payload["source_view"]["source_member_utf8"],
            fixture["source_member"]
        );
        assert_eq!(comparison_payload["inherited_quality"], "not-transferred");
        assert_eq!(comparison_payload["lineage"].as_array().unwrap().len(), 2);
        assert_eq!(
            comparison_payload["lineage"][1]["record_payload"]["admission"]["review_status"],
            "unreviewed"
        );
        assert_eq!(comparison_payload["performs_semantic_assessment"], false);
    } else {
        assert_eq!(
            comparison_payload["source_member_utf8"],
            fixture["source_member"]
        );
    }
    let append_request = |template: &Value, described: &Value| {
        let mut request = template.clone();
        request["expected_snapshot"] = described["owner_snapshot"].clone();
        request["expected_revision"] = described["result"]["revision"].clone();
        let command = &described["result"]["command_context"];
        let sources = command.get("required_sources").and_then(Value::as_array);
        let admissions = command.get("required_admissions").and_then(Value::as_array);
        request["assessments"][0]["evidence"] = Value::Array(
            sources.into_iter().flatten().cloned()
                .chain(admissions.into_iter().flatten().map(|row| row["basis"].clone()))
                .map(|reference| serde_json::json!({"record":reference,"stance":"supports","locator":"Synthetic explicit comparison or quality context."}))
                .collect());
        request
    };
    let request = append_request(&fixture["layer_template"], &layer);
    let quality = invoke(&request);
    assert_eq!(quality["result"]["current_admission"]["can_use"], true);
    assert_eq!(invoke(&request)["result"]["replayed"], true);
    let unit = describe(&fixture["unit_subject"]);
    let mut dependent_request = append_request(&fixture["unit_template"], &unit);
    let dependent = invoke(&dependent_request);
    assert_eq!(dependent["result"]["current_admission"]["can_use"], true);
    let mut withdrawal = fixture["layer_template"].clone();
    withdrawal["command_id"] = Value::from("synthetic-native-layer-withdraw");
    withdrawal["assessments"][0]["assessment_id"] =
        Value::from("tos.review.synthetic-native-layer-withdraw");
    withdrawal["assessments"][0]["decision"] = Value::from("withdraw");
    withdrawal["assessments"][0]["supersedes"] =
        serde_json::json!([quality["result"]["current_admission"]["assessment_refs"][0]]);
    let withdrawn = invoke(&append_request(
        &withdrawal,
        &describe(&fixture["layer_subject"]),
    ));
    assert_eq!(withdrawn["result"]["current_admission"]["can_use"], false);
    let closed_unit = describe(&fixture["unit_subject"]);
    assert_eq!(closed_unit["result"]["current_admission"]["can_use"], false);
    dependent_request["expected_snapshot"] = closed_unit["owner_snapshot"].clone();
    let replay = invoke(&dependent_request);
    assert_eq!(replay["result"]["replayed"], true);
    assert_eq!(
        replay["result"]["receipt"]["admission_at_commit"]["can_use"],
        true
    );
    assert_eq!(replay["result"]["current_admission"]["can_use"], false);
    let mut renewal = fixture["layer_template"].clone();
    renewal["command_id"] = Value::from("synthetic-native-layer-renewed");
    renewal["assessments"][0]["assessment_id"] =
        Value::from("tos.review.synthetic-native-layer-renewed");
    let renewed_quality = invoke(&append_request(
        &renewal,
        &describe(&fixture["layer_subject"]),
    ));
    assert_eq!(
        renewed_quality["result"]["current_admission"]["can_use"],
        true
    );
    let stale_unit = describe(&fixture["unit_subject"]);
    assert_eq!(stale_unit["result"]["current_admission"]["can_use"], false);
    dependent_request["expected_snapshot"] = stale_unit["owner_snapshot"].clone();
    let stale_replay = invoke(&dependent_request);
    assert_eq!(stale_replay["result"]["replayed"], true);
    assert_eq!(
        stale_replay["result"]["receipt"]["admission_at_commit"]["can_use"],
        true
    );
    assert_eq!(
        stale_replay["result"]["current_admission"]["can_use"],
        false
    );
    let mut reassessed = fixture["unit_template"].clone();
    reassessed["command_id"] = Value::from("synthetic-native-unit-reassessed");
    reassessed["assessments"][0]["assessment_id"] =
        Value::from("tos.review.synthetic-native-unit-reassessed");
    let new_dependent = invoke(&append_request(&reassessed, &stale_unit));
    assert_eq!(
        new_dependent["result"]["current_admission"]["can_use"],
        true
    );
    for (path, raw) in preserved {
        assert_eq!(fs::read(path).unwrap(), raw);
    }
    for (index, path) in images.iter().enumerate() {
        assert_eq!(custody(path), image_before[index]);
    }
    assert!(Instant::now() < deadline);
}

#[test]
fn native_private_assessment_v5_quality_dependency_withdrawal_preserves_source() {
    native_layer_journal_case(false);
}

#[test]
fn native_derived_layer_assessment_lineage_quality_withdrawal_preserves_source() {
    native_layer_journal_case(true);
}

#[test]
#[ignore = "requires retained signed synthetic OCR evidence and admitted native owner/worker"]
fn native_private_assessment_v6_retained_signed_ocr_comparison_preserves_source() {
    native_owner_ocr_comparison_case(false);
}

#[test]
#[ignore = "requires separately retained genuine current synthetic PageOCR producer and exact native products"]
fn native_retained_page_ocr_assessment_comparison_preserves_original_and_signed_capture() {
    native_owner_ocr_comparison_case(true);
}

fn native_owner_ocr_comparison_case(retained_page: bool) {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(if retained_page { 480 } else { 240 });
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let owner_fixture = if retained_page {
        page_owner_ocr_fixture(&repository, temporary.path())
    } else {
        v6_owner_ocr_fixture(&repository, temporary.path())
    };
    let fixture = owner_fixture.state.clone();
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let public = PathBuf::from(fixture["public"].as_str().unwrap());
    let context = PathBuf::from(fixture["context"].as_str().unwrap());
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("worker required")),
    ];
    let custody = |path: &Path| {
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
        assert_eq!(metadata.mode() & 0o022, 0);
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            super::command_text_cases::alignment_image_digest(path),
        )
    };
    let image_before_native: Vec<_> = images.iter().map(|path| custody(path)).collect();
    let authored = super::command_text_cases::authored_text_files(&public);
    let mut captured = authored.clone();
    for reference in assessment_feature_sources() {
        let path = repository.join(reference);
        assert!(path.symlink_metadata().unwrap().is_file());
        assert!(fs::metadata(&path).unwrap().len() <= 2_097_152);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
        assert!(Instant::now() < deadline);
    }
    assert!(captured.len() <= 2048);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("layer-journal-source-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":owner,"owner_context":context,"native_executable":images[1],"native_executable_sha256":image_before_native[1].4.to_prefixed(),
        "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,"software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":image_before_native[2].4.to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":image_before_native[2].4.to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let invocation_path = temporary.path().join("layer-journal-invocation.json");
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let observe = |selected_owner: &Path, request: &Value| {
        let mut selected_invocation = invocation.clone();
        selected_invocation["owner_config"] = Value::from(selected_owner.to_str().unwrap());
        if request["schema_version"] != "tos_local_assessment_command_v1" {
            selected_invocation["assessment_schema_worker"] = Value::Null;
        }
        fs::write(
            &invocation_path,
            serde_json::to_vec(&selected_invocation).unwrap(),
        )
        .unwrap();
        super::command_text_cases::native_owner_cli_observation(
            &repository,
            selected_owner,
            &invocation_path,
            request,
            deadline,
        )
    };
    let invocation_count = std::cell::Cell::new(0u32);
    let invoke = |selected_owner: &Path, request: &Value| -> Value {
        let ordinal = invocation_count.get() + 1;
        invocation_count.set(ordinal);
        let (status, raw, errors) = observe(selected_owner, request);
        assert!(
            status.success(),
            "native retained OCR invocation {} operation {}: {}",
            ordinal,
            request["operation"].as_str().unwrap_or("<absent>"),
            String::from_utf8_lossy(&errors)
        );
        let envelope: Value = serde_json::from_slice(&raw).unwrap();
        if request["schema_version"] == "tos_local_assessment_command_v1" {
            envelope
        } else {
            assert_eq!(
                envelope["schema_version"],
                "tos_local_native_source_result_v1"
            );
            assert_eq!(envelope["authentication"], "local-unix-account");
            assert_eq!(envelope["grants_admission"], false);
            let result = envelope["result"].clone();
            assert!(result.is_object());
            assert_eq!(
                result["schema_version"],
                "tos_local_text_layer_derive_result_v1"
            );
            assert_eq!(result["content_disclosure"], "withheld");
            assert_eq!(result["grants_admission"], false);
            assert!(result["owner_configuration"].is_string());
            if request["operation"] == "prepare-create" {
                assert!(result["expected_dependencies"].is_string());
            }
            result
        }
    };
    let prepared = invoke(
        &owner,
        &serde_json::json!({
        "operation":"prepare-create"}),
    );
    let record_request = serde_json::json!({
        "schema_version":"tos_local_source_command_v1","operation":if retained_page { "text-layer.record-owner-page-ocr" } else { "text-layer.record-owner-ocr" },
        "command_id":if retained_page { "synthetic-current-page-authenticated-ocr-record" } else { "synthetic-v6-authenticated-ocr-record" },
        "expected_configuration":prepared["owner_configuration"],
        "expected_dependencies":prepared["expected_dependencies"],
        "expected_source":null,"expected_revision":null});
    let created = invoke(&owner, &record_request);
    assert_eq!(created["replayed"], false);
    if retained_page {
        let replayed = invoke(&owner, &record_request);
        assert_eq!(replayed["replayed"], true);
        assert_eq!(replayed["receipt_sha256"], created["receipt_sha256"]);
    }
    let selected = if retained_page {
        page_assessment_fixture(&repository, &fixture)
    } else {
        v6_assessment_fixture(
            &fixture,
            Path::new(fixture["fixture_root"].as_str().unwrap()),
        )
    };
    let assessment_owner = PathBuf::from(selected["assessment_owner"].as_str().unwrap());
    let package = PathBuf::from(selected["package"].as_str().unwrap());
    let retained = package_files(&package);
    assert_eq!(retained.len(), 12);
    for (name, raw) in &owner_fixture.signed_evidence {
        assert_eq!(retained.get(name), Some(raw));
    }
    assert_eq!(
        Digest256::of_bytes(&retained["owner-ocr-signature.sigstore.json"]).to_hex(),
        selected["original_signature_sha256"].as_str().unwrap()
    );
    let receipt: Value = serde_json::from_slice(&retained["owner-ocr-receipt.json"]).unwrap();
    assert_eq!(
        receipt["owner"]["source_ref"],
        selected["original_owner_source_ref"]
    );
    assert_eq!(
        Digest256::of_bytes(&retained["owner-ocr-receipt.json"]).to_hex(),
        selected["original_receipt_sha256"].as_str().unwrap()
    );
    let image = PathBuf::from(selected["image_path"].as_str().unwrap());
    let image_before = custody(&image);
    let original_pdf =
        retained_page.then(|| PathBuf::from(selected["source_pdf_path"].as_str().unwrap()));
    let original_before = original_pdf.as_ref().map(|path| custody(path));
    let described = invoke(
        &assessment_owner,
        &serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1","operation":"describe",
        "subject_id":selected["subject"]["id"]}),
    );
    let request = serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1","operation":"read-layer-comparison",
        "subject_id":selected["subject"]["id"],"expected_subject":selected["subject"],
        "expected_snapshot":described["owner_snapshot"]});
    let compared = invoke(&assessment_owner, &request);
    assert_eq!(compared["schema_version"], "tos_local_assessment_result_v1");
    assert_eq!(compared["visibility"], "local_only");
    assert_eq!(compared["publication_authorized"], false);
    assert_eq!(compared["result"]["current_admission"]["can_use"], false);
    assert_eq!(compared["result"]["revision"], Value::Null);
    assert_eq!(
        compared["result"]["source_comparison"]["payload"]["source_image"]["sha256"],
        selected["image_sha256"]
    );
    if retained_page {
        let comparison = &compared["result"]["source_comparison"]["payload"];
        assert_eq!(
            comparison["owner_execution"]["input_verification"]["render_execution"],
            "not_performed"
        );
        assert_eq!(
            comparison["owner_execution"]["input_verification"]["historical_receipt_signature"],
            "absent"
        );
        assert_ne!(
            comparison["source_scope"]["file_sha256"],
            comparison["source_image"]["sha256"]
        );
        assert_eq!(
            comparison["input_representation"],
            fixture["input_representation"]
        );
    }
    assert_eq!(
        compared["result"]["source_comparison"]["payload"]["performs_semantic_assessment"],
        false
    );

    assert_eq!(
        compared["result"]["source_comparison"]["payload"]["source_image"]["model_disclosure_authorized"],
        false
    );
    let mut expired: Value = serde_json::from_slice(&fs::read(&assessment_owner).unwrap()).unwrap();
    expired["native_text_layers"][0]["image_access"]["expires_at"] =
        Value::from("2000-01-01T00:00:00Z");
    fs::write(&assessment_owner, serde_json::to_vec(&expired).unwrap()).unwrap();
    assert!(!observe(&assessment_owner, &request).0.success());
    assert_eq!(package_files(&package), retained);
    assert_eq!(custody(&image), image_before);
    if let Some(path) = original_pdf.as_ref() {
        assert_eq!(custody(path), original_before.unwrap());
    }
    assert_authored_text_unchanged(&public, &authored);
    for (index, path) in images.iter().enumerate() {
        assert_eq!(custody(path), image_before_native[index]);
    }
    assert!(Instant::now() < deadline);
}

#[test]
fn native_public_assessment_v1_v2_v3_append_replay_and_revocation_preserve_source() {
    native_public_assessment_versions(&[1, 2, 3]);
}

#[test]
fn native_public_assessment_v3_append_replay_revocation_and_metadata_scope_preserve_source() {
    native_public_assessment_versions(&[3]);
}

#[test]
fn native_public_v2_assessed_form_batch_matches_builder_and_rechecks_drift() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let fixture =
        super::native_public_assessment_fixture::native_public_v2_assessed_form_batch_fixture(
            &repository,
            temporary.path(),
        )
        .unwrap();
    let owner = fixture.owner_config_path.clone();
    let source_root = fixture.public_root.clone();
    let invocation_path = temporary
        .path()
        .join("native-assessment-read-invocation.json");
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("native worker required")),
    ];
    let before: Vec<_> = images
        .iter()
        .map(|path| super::command_text_cases::alignment_image_digest(path))
        .collect();
    let authored = super::command_text_cases::authored_text_files(&source_root);
    let mut captured = authored.clone();
    for reference in assessment_feature_sources().into_iter().chain([
        "rust/crates/tos-ops-mechanics-plan/src/tree_nodes.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-validation/src/source_forms/source_copy_kernel.rs",
    ]) {
        let path = repository.join(reference);
        assert!(path.is_file() && fs::metadata(&path).unwrap().len() <= 2_097_152);
        assert!(
            captured
                .insert(reference.to_owned(), fs::read(path).unwrap())
                .is_none()
        );
    }
    assert!(captured.len() <= 2048 && captured.values().map(Vec::len).sum::<usize>() <= 33_554_432);
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("public-assessment-read-cut");
    let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
    let invocation = serde_json::json!({
        "schema_version":"tos_local_native_assessment_read_invocation_v1",
        "owner_config":owner,
        "native_executable":images[1],
        "native_executable_sha256":before[1].to_prefixed(),
        "corpus_store":store,
        "source_revision":selected.0.to_prefixed(),
        "original_source_revision":selected.0.to_prefixed(),
        "software_capture":capture.capture,
        "software_restored_root":capture.restored,
        "software_selection":{"source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":before[2].to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":before[2].to_prefixed()},
        "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
            "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}
    });
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();

    let mut command_invocation = invocation.clone();
    command_invocation["schema_version"] = Value::from("tos_local_native_source_invocation_v1");
    command_invocation["owner_context"] = serde_json::json!(fixture.owner_context_path);
    let command_path = temporary
        .path()
        .join("native-assessment-command-invocation.json");
    fs::write(
        &command_path,
        serde_json::to_vec(&command_invocation).unwrap(),
    )
    .unwrap();
    fs::set_permissions(&command_path, fs::Permissions::from_mode(0o600)).unwrap();
    let observe = |path: &Path, request: &Value| {
        super::command_text_cases::native_owner_cli_observation(
            &repository,
            &owner,
            path,
            request,
            deadline,
        )
    };
    let invoke = |path: &Path, request: &Value| -> Value {
        let (status, raw, errors) = observe(path, request);
        assert!(
            status.success(),
            "native batch {}: {}",
            request["operation"],
            String::from_utf8_lossy(&errors)
        );
        let outer: Value = serde_json::from_slice(&raw).unwrap();
        if request["operation"] == "materialize_assessed_forms" {
            assert_eq!(outer["schema_version"], "tos_local_native_source_result_v1");
            assert_eq!(outer["grants_admission"], false);
            outer["result"].clone()
        } else {
            outer
        }
    };
    let describe = |index: usize| serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"describe","subject_id":fixture.subject_ids[index]});
    for &index in &fixture.ready_selections {
        let current = invoke(&command_path, &describe(index));
        let request = serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"append",
            "subject_id":fixture.subject_ids[index],"expected_subject":fixture.subject_refs[index],
            "expected_snapshot":current["owner_snapshot"],"expected_revision":current["result"]["revision"],
            "command_id":format!("native-batch-{index}"),"assessments":[fixture.append_assessments[index]]});
        assert_eq!(
            invoke(&command_path, &request)["result"]["current_admission"]["can_use"],
            true
        );
    }
    fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut result = BTreeMap::new();
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            let metadata = path.symlink_metadata().unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                result.extend(files(&path));
            } else {
                assert!(metadata.is_file());
                result.insert(path.clone(), fs::read(path).unwrap());
            }
        }
        result
    }
    // Establish each subject read lock before comparing journal bytes.
    // Subsequent batch and single reads must preserve this complete baseline.
    for index in 0..fixture.subject_ids.len() {
        invoke(&command_path, &describe(index));
    }
    let journal_before = files(&fixture.journal_directory);
    let source_path = fixture.owner_config["source_records"][0]["path"].clone();
    let form_path = fixture
        .form_set_path
        .as_ref()
        .unwrap()
        .strip_prefix(&source_root)
        .unwrap()
        .to_str()
        .unwrap();
    let selections=fixture.subject_refs.iter().map(|form_ref|serde_json::json!({
        "form_ref":form_ref,"subject_ref":fixture.source_ref,"source_path":source_path,"form_path":form_path
    })).collect::<Vec<_>>();
    let request = serde_json::json!({"schema_version":"tos_local_assessed_forms_materialization_request_v1","operation":"materialize_assessed_forms","selections":selections});
    let actual = invoke(&invocation_path, &request);
    assert_eq!(actual["schema_version"], "tos_local_assessed_forms_materialization_result_v1");
    let replies = actual["replies"].as_array().unwrap();
    assert_eq!(replies.len(), 7);
    for (index, reply) in replies.iter().enumerate() {
        let current = invoke(&command_path, &describe(index));
        let single = invoke(
            &command_path,
            &serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"materialize-form","subject_id":fixture.subject_ids[index],"expected_subject":fixture.subject_refs[index],
            "expected_snapshot":current["owner_snapshot"]}),
        );
        let mut expected = single.clone();
        expected["owner_snapshot"] = reply["owner_snapshot"].clone();
        assert_eq!(reply, &expected, "whole native batch/single parity {index}");
        assert_eq!(
            reply["result"]["materialization"]["state"],
            if index == 0 {
                "needs-assessment"
            } else {
                "ready"
            }
        );
    }
    assert_eq!(files(&fixture.journal_directory), journal_before);
    assert_authored_text_unchanged(&source_root, &authored);
    for mutation in [
        "version",
        "subject",
        "source-path",
        "form-path",
        "duplicate",
    ] {
        let mut bad = request.clone();
        let rows = bad["selections"].as_array_mut().unwrap();
        let last = rows.last_mut().unwrap();
        match mutation {
            "version" => last["form_ref"]["version"] = serde_json::json!(2),
            "subject" => last["subject_ref"] = last["form_ref"].clone(),
            "source-path" => {
                last["source_path"] = Value::from("ToS/source-witnesses/another/source.json")
            }
            "form-path" => {
                last["form_path"] =
                    Value::from("ToS/source-witnesses/another/source.human-forms.json")
            }
            _ => {
                let duplicate = last.clone();
                rows.push(duplicate);
            }
        }
        assert!(
            !observe(&invocation_path, &bad).0.success(),
            "bad final selection {mutation}"
        );
        assert_eq!(files(&fixture.journal_directory), journal_before);
        assert_authored_text_unchanged(&source_root, &authored);
    }
    let grammar_raw =
        fs::read(source_root.join("ToS/contracts/knowledge-assessment.schema.json")).unwrap();
    let grammar = store
        .join("objects")
        .join(Digest256::of_bytes(&grammar_raw).to_hex());
    assert_eq!(fs::read(&grammar).unwrap(), grammar_raw);
    fs::write(&grammar, b"{}").unwrap();
    assert!(!observe(&invocation_path, &request).0.success());
    fs::write(&grammar, &grammar_raw).unwrap();
    assert_eq!(invoke(&invocation_path, &request), actual);
    let owner_before = fs::read(&owner).unwrap();
    let control = source_root.join("ToS/source-witnesses/.metadata-publication.json");
    let control_before = fs::read(&control).ok();
    for mutation in ["grant", "pending", "ready-epoch"] {
        let target = temporary
            .path()
            .join(format!("native-post-fsync-{mutation}.json"));
        let changed = std::cell::Cell::new(false);
        let result = tos_command::managed_native_original_cli::conformance_write_assessed_candidate(
            &target,
            b"{}",
            || {
                changed.set(true);
                if mutation == "grant" {
                    let mut config: Value = serde_json::from_slice(&owner_before).unwrap();
                    config["subjects"][&fixture.subject_ids[0]]["access_allowed"] =
                        Value::Bool(false);
                    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
                } else {
                    let mut state = serde_json::json!({"schema_version":"tos_source_metadata_publication_v1","generation":1,
                    "transition_id":"11111111111111111111111111111111","phase":if mutation=="pending"{"pending"}else{"ready"},
                    "transaction_id":format!("sha256:{}","1".repeat(64)),"manifest_sha256":format!("sha256:{}","2".repeat(64)),
                    "outcome":if mutation=="pending"{Value::Null}else{Value::from("rolled-back")},"recovery_authorization":null});
                    state["token"] = Value::from(
                        Digest256::of_bytes(&super::command_text_cases::alignment_owner_bytes(
                            &state,
                        ))
                        .to_prefixed(),
                    );
                    fs::write(&control, serde_json::to_vec(&state).unwrap()).unwrap();
                }
                let (status, raw, _) = observe(&invocation_path, &request);
                if !status.success() || serde_json::from_slice::<Value>(&raw).unwrap()["result"] != actual {
                    Err(std::io::Error::other(
                        "selected assessment or publication changed after fsync",
                    )
                    .into())
                } else {
                    Ok(())
                }
            },
        );
        assert!(
            changed.get() && result.is_err() && !target.exists(),
            "post-fsync mutation {mutation}"
        );
        assert!(fs::read_dir(temporary.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tos-assessed-")
        }));
        fs::write(&owner, &owner_before).unwrap();
        match &control_before {
            Some(raw) => fs::write(&control, raw).unwrap(),
            None => {
                if control.exists() {
                    fs::remove_file(&control).unwrap();
                }
            }
        }
        assert_eq!(invoke(&invocation_path, &request), actual);
        assert_eq!(files(&fixture.journal_directory), journal_before);
    }
    let mut expired: Value = serde_json::from_slice(&owner_before).unwrap();
    for authority in expired["authorities"].as_array_mut().unwrap() {
        authority["payload"]["valid_until"] = Value::from("2026-09-02T00:00:00Z");
    }
    fs::write(&owner, serde_json::to_vec(&expired).unwrap()).unwrap();
    let result = invoke(&invocation_path, &request);
    assert_ne!(result["owner_snapshot"], actual["owner_snapshot"]);
    for reply in result["replies"].as_array().unwrap() {
        assert_eq!(
            reply["result"]["materialization"]["state"],
            "needs-assessment"
        );
        assert!(reply["result"]["materialization"]["display_text"].is_null());
        assert_eq!(reply["result"]["current_admission"]["can_use"], false);
    }
    fs::write(&owner, &owner_before).unwrap();
    let index = fixture.ready_selections[0];
    let current = invoke(&command_path, &describe(index));
    let withdrawn = invoke(
        &command_path,
        &serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"append",
        "subject_id":fixture.subject_ids[index],"expected_subject":fixture.subject_refs[index],"expected_snapshot":current["owner_snapshot"],
        "expected_revision":current["result"]["revision"],"command_id":"native-batch-withdrawal","assessments":[fixture.withdrawal_assessments[index]]}),
    );
    assert_eq!(withdrawn["result"]["current_admission"]["can_use"], false);
    let journal_after = files(&fixture.journal_directory);
    let result = invoke(&invocation_path, &request);
    assert_ne!(
        result["replies"][index]["result"]["revision"],
        actual["replies"][index]["result"]["revision"]
    );
    for (i, reply) in result["replies"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            reply["result"]["materialization"]["state"],
            if i == 0 || i == index {
                "needs-assessment"
            } else {
                "ready"
            }
        );
    }
    assert_eq!(files(&fixture.journal_directory), journal_after);
    assert_authored_text_unchanged(&source_root, &authored);
    for (path, expected) in images.iter().zip(&before) {
        assert_eq!(
            super::command_text_cases::alignment_image_digest(path),
            *expected
        );
    }
}

fn native_public_assessment_versions(versions: &[u8]) {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(600);
    let cancelled = AtomicBool::new(false);
    for &version in versions {
        let temporary = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let native_fixture =
            super::native_public_assessment_fixture::native_public_assessment_fixture(
                &repository,
                temporary.path(),
                version,
            )
            .unwrap();
        let fixture = serde_json::json!({
            "owner": native_fixture.owner_config_path,
            "public": native_fixture.public_root,
            "context": native_fixture.owner_context_path,
            "subject_id": native_fixture.subject_ids[0],
            "request": {
                "schema_version":"tos_local_assessment_command_v1",
                "operation":"append",
                "subject_id":native_fixture.subject_ids[0],
                "expected_subject":native_fixture.subject_refs[0],
                "expected_snapshot":null,"expected_revision":null,
                "command_id":"native-public-assessment-append",
                "assessments":[native_fixture.append_assessments[0]]
            },
            "preserved":native_fixture.preserved,
            "metadata_subject":native_fixture.metadata_subject,
        });
        let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
        let public = PathBuf::from(fixture["public"].as_str().unwrap());
        let context = PathBuf::from(fixture["context"].as_str().unwrap());
        let images = [
            std::env::current_exe().unwrap(),
            PathBuf::from(
                std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
            ),
            PathBuf::from(
                std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("native worker required"),
            ),
        ];
        let custody = |path: &Path| {
            let metadata = path.symlink_metadata().unwrap();
            assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= 536_870_912);
            assert_eq!(metadata.mode() & 0o022, 0);
            (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mode(),
                super::command_text_cases::alignment_image_digest(path),
            )
        };
        let before: Vec<_> = images.iter().map(|path| custody(path)).collect();
        let authored = super::command_text_cases::authored_text_files(&public);
        let mut captured = authored.clone();
        for reference in assessment_feature_sources() {
            let path = repository.join(reference);
            assert!(
                path.symlink_metadata().unwrap().is_file()
                    && fs::metadata(&path).unwrap().len() <= 2_097_152
            );
            assert!(
                captured
                    .insert(reference.to_owned(), fs::read(path).unwrap())
                    .is_none()
            );
        }
        assert!(
            captured.len() <= 2048 && captured.values().map(Vec::len).sum::<usize>() <= 33_554_432
        );
        let (capture, _software, components) =
            super::command_record_cases::captured_components(&captured, deadline, &cancelled);
        let store = temporary.path().join("public-journal-cut");
        let selected = super::validation_cut_cases::write_cut_store(&authored, &store);
        let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
            "owner_config":owner,"owner_context":context,"native_executable":images[1],"native_executable_sha256":before[1].4.to_prefixed(),
            "corpus_store":store,"source_revision":selected.0.to_prefixed(),"original_source_revision":selected.0.to_prefixed(),
            "software_capture":capture.capture,"software_restored_root":capture.restored,
            "software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":images[2],"sha256":before[2].4.to_prefixed()},
            "assessment_schema_worker":{"absolute_path":images[2],"sha256":before[2].4.to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        let invocation_path = temporary.path().join("public-journal-invocation.json");
        fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
        let invocation_count = std::cell::Cell::new(0u32);
        let observe = |request: &Value| {
            invocation_count.set(invocation_count.get().checked_add(1).unwrap());
            super::command_text_cases::native_owner_cli_observation(
                &repository,
                &owner,
                &invocation_path,
                request,
                deadline,
            )
        };
        let invoke = |request: &Value| -> Value {
            let (status, raw, errors) = observe(request);
            assert!(
                status.success(),
                "public Journal v{version} call {} operation {}: {}",
                invocation_count.get(),
                request["operation"].as_str().unwrap_or("missing"),
                String::from_utf8_lossy(&errors)
            );
            serde_json::from_slice(&raw).unwrap()
        };
        let describe = serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"describe","subject_id":fixture["subject_id"]});
        let described = invoke(&describe);
        assert_eq!(
            described["schema_version"],
            "tos_local_assessment_result_v1"
        );
        assert_eq!(described["authentication"], "local-unix-account");
        assert_eq!(
            described["result"]["command_context"]["subject"],
            native_fixture.subject_refs[0]
        );
        assert_eq!(
            described["result"]["command_context"]["grants_authority"],
            false
        );
        assert!(
            described["owner_snapshot"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert_eq!(described["result"]["batch_count"], 0);
        assert!(described["result"]["revision"].is_null());
        let mut request = fixture["request"].clone();
        request["expected_snapshot"] = described["owner_snapshot"].clone();
        let mut inspect = serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"inspect","subject_id":fixture["subject_id"],
            "expected_subject":request["expected_subject"],"expected_snapshot":request["expected_snapshot"]});
        if version == 2 {
            inspect["operation"] = Value::from("materialize-form");
            let pending = invoke(&inspect);
            assert_eq!(
                pending["result"]["materialization"]["state"],
                "needs-assessment"
            );
            assert!(pending["result"]["materialization"]["display_text"].is_null());
        }
        let committed = invoke(&request);
        assert_eq!(committed["result"]["current_admission"]["can_use"], true);
        if version == 2 {
            let ready = invoke(&inspect);
            assert_eq!(ready["result"]["materialization"]["state"], "ready");
            assert!(
                ready["result"]["materialization"]["display_text"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty())
            );
        } else {
            assert_eq!(invoke(&inspect)["result"]["batch_count"], 1);
        }
        let replay = invoke(&request);
        assert_eq!(replay["result"]["replayed"], true);
        assert_eq!(replay["result"]["receipt"], committed["result"]["receipt"]);
        let mut config: Value = serde_json::from_slice(&fs::read(&owner).unwrap()).unwrap();
        let original_authority = config["authorities"][0].clone();
        let authority = &mut config["authorities"][0];
        authority["version"] = Value::from(
            authority["version"]
                .as_u64()
                .unwrap()
                .checked_add(1)
                .unwrap(),
        );
        authority["payload"]["authority_version"] = Value::from(
            authority["payload"]["authority_version"]
                .as_u64()
                .unwrap()
                .checked_add(1)
                .unwrap(),
        );
        authority["payload"]["state"] = Value::from("revoked");
        fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        request["expected_snapshot"] = invoke(&describe)["owner_snapshot"].clone();
        inspect["expected_snapshot"] = request["expected_snapshot"].clone();
        if version == 2 {
            let closed = invoke(&inspect);
            assert_eq!(
                closed["result"]["materialization"]["state"],
                "needs-assessment"
            );
            assert!(closed["result"]["materialization"]["display_text"].is_null());
        }
        let revoked = invoke(&request);
        assert_eq!(revoked["result"]["replayed"], true);
        assert_eq!(
            revoked["result"]["receipt"]["admission_at_commit"]["can_use"],
            true
        );
        assert_eq!(revoked["result"]["current_admission"]["can_use"], false);
        if version == 3 {
            config["authorities"][0] = original_authority;
            config["native_text_units"][0]["read_scope"] = Value::from("metadata_only");
            // The genuine metadata-only resolver view has a new subject digest.
            // Keep request.expected_subject unchanged for the stale downgrade refusal.
            config["subjects"][fixture["subject_id"].as_str().unwrap()]["record"] =
                fixture["metadata_subject"].clone();
            fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
            fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
            let metadata = invoke(&describe);
            assert_eq!(
                metadata["result"]["command_context"]["supported_operations"],
                serde_json::json!(["describe", "inspect"])
            );
            request["expected_snapshot"] = metadata["owner_snapshot"].clone();
            assert!(!observe(&request).0.success());
        }
        let mut unknown = describe.clone();
        unknown["subject_id"] = Value::from("tos.subject.outside-public-selection");
        assert!(!observe(&unknown).0.success());
        for preserved in fixture["preserved"].as_array().unwrap() {
            let path = Path::new(preserved["path"].as_str().unwrap());
            let metadata = path.symlink_metadata().unwrap();
            assert!(metadata.is_file() && metadata.len() <= 8_388_608);
            let raw = fs::read(path).unwrap();
            assert_eq!(raw.len() as u64, preserved["bytes"].as_u64().unwrap());
            assert_eq!(Digest256::of_bytes(&raw).to_hex(), preserved["sha256"].as_str().unwrap());
        }
        for (index, path) in images.iter().enumerate() {
            assert_eq!(custody(path), before[index]);
        }
        assert!(Instant::now() < deadline);
    }
}
