//! One complete protected Artifact CLI consumer. Synthetic metadata and exact
//! input bindings provide no acquisition, rights clearance or witness admission.
use super::command_form_cases::successor;
use super::command_text_cases::{
    alignment_image_digest, alignment_native_cli, authored_text_files, native_owner_cli_observation,
};
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_creation_store::IsolatedCreationRoot;

// Consumer-only physical scratch envelope; library budgets stay unchanged.
// Sixteen full fixture allocations cover source/oracle roots, archive capture,
// compressed archive, restored software, cut object copies and transaction
// staging/retained packages. The fixed allowances cover manifests, child I/O,
// owner-tool copy and growth outputs, plus explicit filesystem headroom.
fn physical_fixture_budget(files: &BTreeMap<String, Vec<u8>>) -> u64 {
    const BLOCK: u64 = 4096;
    assert!(files.len() <= 256);
    let mut logical = 0u64;
    let mut allocated = 0u64;
    for (path, raw) in files {
        assert!(path.len() <= 512 && path.split('/').count() <= 16);
        assert!(
            !path.starts_with('/')
                && !path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
        );
        logical = logical.checked_add(raw.len() as u64).unwrap();
        // Round every file separately; allow a block per path directory and
        // two further blocks for file/directory inode and metadata allocation.
        allocated = allocated
            .checked_add(
                (raw.len() as u64).div_ceil(BLOCK) * BLOCK
                    + (path.split('/').count() as u64 + 2) * BLOCK,
            )
            .unwrap();
    }
    assert!(logical <= 8_388_608);
    let total = allocated
        .checked_mul(16)
        .unwrap()
        .checked_add(128 * 1_048_576)
        .unwrap()
        .checked_add(256 * 1_048_576)
        .unwrap();
    assert!(total <= 1_073_741_824);
    total
}

#[test]
fn native_artifact_cli_describes_prepares_creates_and_cold_replays_exact_bytes() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    let sample_path = "ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/artifact-witness.json";
    let mut record: Value =
        serde_json::from_slice(&fs::read(repository.join(sample_path)).unwrap()).unwrap();
    let rights_path = record["rights_ref"].as_str().unwrap().to_owned();
    let discovery_path = record["discovery_ref"].as_str().unwrap().to_owned();
    record["$schema"] = serde_json::json!(
        "https://tree-of-sophia.local/ToS/contracts/artifact-source-witness-v2.schema.json"
    );
    record["schema_version"] = serde_json::json!("tos_artifact_source_witness_v2");
    record["artifact_id"] = serde_json::json!("tos.artifact.synthetic-created");
    record["record_version"] = serde_json::json!(1);
    record["provenance_event_ref"] = serde_json::json!("tos.event.synthetic-artifact-created");
    record["philosophy_planting_refs"] = serde_json::json!([]);
    record["created_at"] = serde_json::json!("2026-09-09T12:00:00Z");
    record["maker"] = serde_json::json!({"maker_type":"model","agent_ref":"model:codex","human_review_performed":false});
    record["authority"]["review_status"] = serde_json::json!("unreviewed");
    record["custody"]["inventory_numbers"] = serde_json::json!(["SYNTHETIC-CREATION-ONLY"]);
    record["path_identity"]["note"] =
        serde_json::json!("Synthetic physical identity, no historical assessment.");
    let mut rights: Value =
        serde_json::from_slice(&fs::read(repository.join(rights_path)).unwrap()).unwrap();
    rights["rights_id"] = serde_json::json!("tos.rights.synthetic-created-artifact");
    rights["scope_refs"] = serde_json::json!([record["artifact_id"]]);
    let mut discovery: Value =
        serde_json::from_slice(&fs::read(repository.join(discovery_path)).unwrap()).unwrap();
    discovery["target"]["known_tos_refs"] = serde_json::json!([record["artifact_id"]]);
    let mut files = BTreeMap::new();
    // The existing protected harness uses the actual authored contract directory,
    // not a handwritten schema or a fabricated catalog/assessment projection.
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
    for name in ["entity-types.v1.json", "relation-types.v1.json"] {
        let path = format!("ToS/doctrine/semantic-interchange/{name}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    let inputs = [
        (
            "rights_ref",
            "ToS/source-witnesses/rights/synthetic-artifact.json",
            serde_json::to_vec_pretty(&rights).unwrap(),
        ),
        (
            "discovery_ref",
            "ToS/source-witnesses/discovery/runs/synthetic-artifact.json",
            serde_json::to_vec_pretty(&discovery).unwrap(),
        ),
        (
            "research_ref",
            "ToS/research-packets/synthetic-artifact.md",
            b"Synthetic test research input; not a real rights or discovery decision.\n".to_vec(),
        ),
    ];
    let mut bindings = serde_json::json!({});
    for (field, path, raw) in inputs {
        record[field] = serde_json::json!(path);
        bindings[field] =
            serde_json::json!({"ref":path,"sha256":Digest256::of_bytes(&raw).to_hex()});
        files.insert(path.into(), raw);
    }
    for path in [
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_artifact_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
        "scripts/build_source_witness_catalog.py",
        "scripts/source_record_profiles.py",
        "scripts/source_witness_human_forms.py",
        "scripts/source_witness_bibliographic_graph_common.py",
        "rust/crates/tos-command/src/source_artifact_native.rs",
        "rust/crates/tos-command/src/source_creation.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
    ] {
        files.insert(path.into(), fs::read(repository.join(path)).unwrap());
    }
    let fixture_bytes = files
        .values()
        .try_fold(0u64, |n, raw| n.checked_add(raw.len() as u64))
        .unwrap();
    let scratch_bound = physical_fixture_budget(&files);
    assert!(fixture_bytes <= 8_388_608 && files.len() <= 256);
    eprintln!(
        "Artifact physical scratch <={} B including 256MiB headroom; F<=8MiB entries<=256 path<=512B depth<=16; images are supplied outside scratch",
        scratch_bound
    );
    let native = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
            .expect("OPS must select the protected native Artifact image"),
    );
    assert!(native.is_absolute());
    let worker = super::validation_cut_cases::selected_worker_path();
    let native_bytes = fs::metadata(&native).unwrap().len();
    let worker_bytes = fs::metadata(&worker).unwrap().len();
    let consumer_bytes = fs::metadata(std::env::current_exe().unwrap())
        .unwrap()
        .len();
    assert!(
        native_bytes <= 536_870_912 && worker_bytes <= 536_870_912 && consumer_bytes <= 536_870_912
    );
    assert!(Instant::now() < deadline);
    eprintln!(
        "Artifact CLI F_fixture={} E_native={} C_consumer={} W_worker={} native_children=6 schema_workers<=6 deadline_s=240 per_child_s<=60; existing capture harness has one git selection, one git owner-tool read and two Python capture/restore children",
        fixture_bytes, native_bytes, consumer_bytes, worker_bytes
    );
    let (capture, software, components) =
        super::command_record_cases::captured_components(&files, deadline, &cancelled);
    let temporary = tempfile::tempdir().unwrap();
    let store = temporary.path().join("selected-store");
    let authored = files
        .iter()
        .filter(|(path, _)| path.starts_with("ToS/"))
        .map(|(path, raw)| (path.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let base = super::validation_cut_cases::write_cut_store(&authored, &store);
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
    for (path, raw) in &files {
        let path = isolated.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let source_path = "ToS/source-witnesses/artifacts/synthetic/uncertain/new-native-artifact/artifact-witness.json";
    let home = Path::new(source_path).parent().unwrap();
    fs::create_dir_all(isolated.path().join(home).parent().unwrap()).unwrap();
    let private=isolated.path().join("ToS/source-witnesses/artifacts/synthetic/existing/representations/private/payload/untouched.bin");
    fs::create_dir_all(private.parent().unwrap()).unwrap();
    let private_bytes = b"Private synthetic payload: no inspection, acquisition or publication.";
    fs::write(&private, private_bytes).unwrap();
    let config = serde_json::json!({"schema_version":"tos_local_artifact_create_owner_v1","uid":fs::metadata(isolated.path()).unwrap().uid(),"principal_id":"model:codex","maker_type":"model","source_root":isolated.path(),"source_path":source_path,"authority_ref":"synthetic-test-only:artifact-creation-not-assessment","expires_at":"2099-01-01T00:00:00Z","record_id":record["artifact_id"],"provenance_event_id":record["provenance_event_ref"],"allowed_operations":["source.create"],"allowed_form_ids":["tos.form.synthetic-artifact-name","tos.form.synthetic-artifact-note"],"source_bindings":bindings});
    let owner = isolated.path().join("owner.json");
    fs::write(&owner, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let invocation_path = temporary.path().join("artifact-invocation.json");
    let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,"owner_context":null,"assessment_schema_worker":null,"native_executable":native,"native_executable_sha256":alignment_image_digest(&native).to_prefixed(),"corpus_store":store,"source_revision":base.0.to_prefixed(),"original_source_revision":base.0.to_prefixed(),"software_capture":capture.capture,"software_restored_root":capture.restored,"software_selection":{"source_git_commit":capture.selection.source_git_commit,"source_git_tree":capture.selection.source_git_tree,"capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()},"software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),"schema_worker":{"absolute_path":worker,"sha256":alignment_image_digest(&worker).to_prefixed()},"budgets":{"max_revisions":4,"max_members":256,"max_total_bytes":8388608,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    let write_invocation = |value: &Value| {
        fs::write(&invocation_path, serde_json::to_vec(value).unwrap()).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    };
    write_invocation(&invocation);
    for operation in ["describe", "prepare"] {
        let mut information_request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":operation});
        // The maintained prepare contract inspects the delegated record shape.
        if operation == "prepare" {
            information_request["record"] = record.clone();
        }
        eprintln!("Artifact actual information operation={operation}");
        let result = alignment_native_cli(
            &repository,
            &owner,
            &invocation_path,
            &information_request,
            deadline,
        );
        assert_eq!(
            result["schema_version"],
            "tos_local_native_source_result_v1"
        );
        assert_eq!(result["grants_admission"], false);
        assert_eq!(result["result"]["record_type"], "artifact");
    }
    let preview_request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","record":record,"source_bindings":bindings,"forms":[{"form_id":"tos.form.synthetic-artifact-name","field_id":"metadata.preferred-name"},{"form_id":"tos.form.synthetic-artifact-note","field_id":"metadata.source-note"}]});
    let preview = alignment_native_cli(
        &repository,
        &owner,
        &invocation_path,
        &preview_request,
        deadline,
    );
    assert_eq!(preview["grants_admission"], false);
    let mut request = preview_request;
    request["operation"] = serde_json::json!("source.create");
    request["command_id"] = serde_json::json!("synthetic:whole-native-artifact-cli");
    request["expected_configuration"] = preview["result"]["owner_configuration"].clone();
    request["expected_source"] = Value::Null;
    request["expected_revision"] = Value::Null;
    request["expected_dependencies"] = preview["result"]["expected_dependencies"].clone();
    let research = isolated
        .path()
        .join(record["research_ref"].as_str().unwrap());
    let original_research = fs::read(&research).unwrap();
    fs::write(
        &research,
        [original_research.as_slice(), b"Changed exact input.\n"].concat(),
    )
    .unwrap();
    let (status, _, _) =
        native_owner_cli_observation(&repository, &owner, &invocation_path, &request, deadline);
    assert!(!status.success());
    assert!(!isolated.path().join(home).exists());
    assert_eq!(fs::read(&private).unwrap(), private_bytes);
    fs::write(&research, &original_research).unwrap();
    let created = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(created["result"]["replayed"], false);
    assert_eq!(created["result"]["grants_admission"], false);
    let original_files = fs::read_dir(isolated.path().join(home))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(original_files.len(), 6);
    let published: Value =
        serde_json::from_slice(&original_files["artifact-witness.json"]).unwrap();
    assert_eq!(published["artifact_id"], record["artifact_id"]);
    assert!(published.get("record_id").is_none());
    assert_eq!(published["maker"]["human_review_performed"], false);
    assert_eq!(published["authority"]["publication_authority"], false);
    assert_eq!(published["authority"]["source_text_admitted"], false);
    let event: Value =
        serde_json::from_slice(&original_files["source-create-provenance.jsonl"]).unwrap();
    assert_eq!(
        event["method"]["procedure"]["name"],
        "native-artifact-metadata-serialization"
    );
    assert_eq!(
        event["review_and_authority"]["human_review_status"],
        "not_performed"
    );
    assert_eq!(
        event["rights_and_visibility"]["publication_authorized"],
        false
    );
    assert_eq!(
        event["authority_boundary"]["validator_role"],
        "mechanics_and_closure_only_not_truth"
    );
    assert!(
        event["method"]["software_components"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["artifact_ref"] == "runtime:tos-native-executable")
    );
    assert!(
        event["method"]["software_components"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["artifact_ref"] != "runtime:python-executable")
    );
    let mut current_files = authored_text_files(isolated.path());
    current_files.remove("ToS/source-witnesses/.historical-create.writer.lock");
    physical_fixture_budget(&current_files);
    let current = successor(&current_files, &store, base);
    invocation["source_revision"] = serde_json::json!(current.0.to_prefixed());
    write_invocation(&invocation);
    let cold = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
    assert_eq!(cold["result"]["replayed"], true);
    assert_eq!(cold["result"]["receipt"], created["result"]["receipt"]);
    for (name, raw) in original_files {
        assert_eq!(
            fs::read(isolated.path().join(home).join(name)).unwrap(),
            raw
        );
    }
    for field in ["rights_ref", "discovery_ref", "research_ref"] {
        let path = bindings[field]["ref"].as_str().unwrap();
        assert_eq!(fs::read(isolated.path().join(path)).unwrap(), files[path]);
    }
    assert_eq!(fs::read(&private).unwrap(), private_bytes);
    // Python's existing fault fixture interrupts its own directory-publish edge;
    // no corresponding native hook exists. This case adds no kill race or fake
    // pending recovery state. Exact native cold replay is the supported seam.
    drop(software);
}
