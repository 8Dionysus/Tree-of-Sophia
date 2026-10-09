//! The maintained initial Metadata oracle observes a real native creation.
//! Creation, profile transition and publication use the SAME protected owner C;
//! its committed database/binding then continue through the real access lanes.
use super::command_claim_publication_cases::{
    AGENT_RECORD_COMPONENTS, agent_authored, agent_authored_capture, agent_catalog,
    agent_native_call, canonical_lf, fixture_physical_bytes, read_packet, report_object_order,
    typed,
};
use super::*;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Component,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tos_command::source_claim_publication::ClaimPublicationLimits;
use tos_compiler::{
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_semantic_index::{self, SemanticMaintenanceLimits},
};
use tos_source_store::{
    CaptureGitRequest, CaptureRestoreLimits, GitCaptureLimits, ReadLimits, SoftwareCaptureReader,
    SoftwareCaptureSelectionV1, capture_git, restore_capture,
};
#[path = "claim_publication_access.rs"]
mod access;
#[path = "../../../rust/crates/tos-access/tests/support/native_child.rs"]
mod native_child;

fn private_json(path: &Path, value: &Value) {
    let raw = canonical_lf(value);
    assert!(raw.len() <= 1_048_576);
    fs::write(path, raw).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn decode_fixture_hex(raw: &str) -> Vec<u8> {
    assert!(raw.len().is_multiple_of(2));
    raw.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => panic!("invalid frozen Metadata source hex"),
            };
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect()
}
fn rebase_fixture_paths(value: &mut Value, replacements: &[(String, String)]) {
    match value {
        Value::String(text) => {
            for (from, to) in replacements {
                *text = text.replace(from, to);
            }
        }
        Value::Array(values) => {
            for value in values {
                rebase_fixture_paths(value, replacements);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                rebase_fixture_paths(value, replacements);
            }
        }
        _ => {}
    }
}
fn materialize_metadata_predecessor(
    fixture_dir: &Path,
    packet: &mut Value,
    workspace: &Path,
) -> (PathBuf, PathBuf, PathBuf) {
    let old_root = required(packet, "source_root").to_owned();
    let old_db = required(packet, "db_path").to_owned();
    let old_owner = required(packet, "owner_config").to_owned();
    let source_root = workspace.join("metadata-source");
    fs::create_dir(&source_root).unwrap();
    fs::set_permissions(&source_root, fs::Permissions::from_mode(0o700)).unwrap();
    let source_root = source_root.canonicalize().unwrap();
    let db_path = source_root.join("derived/prepared.sqlite");
    let owner_path = source_root.join("metadata-addition-owner.json");
    let replacements = [
        (old_db, db_path.display().to_string()),
        (old_owner, owner_path.display().to_string()),
        (old_root, source_root.display().to_string()),
    ];
    rebase_fixture_paths(packet, &replacements);

    let source_files = packet["source_files"].as_object().unwrap();
    assert!(source_files.len() <= 2048);
    let mut total = 0usize;
    for (relative, encoded) in source_files {
        assert!(relative.starts_with("ToS/"));
        let relative_path = Path::new(relative);
        assert!(relative_path.components().all(|part| matches!(part, Component::Normal(_))));
        let bytes = decode_fixture_hex(encoded.as_str().unwrap());
        total = total.checked_add(bytes.len()).unwrap();
        assert!(bytes.len() <= 8_388_608 && total <= 16_777_216);
        let target = source_root.join(relative_path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, bytes).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let db_parent = db_path.parent().unwrap();
    fs::create_dir_all(db_parent).unwrap();
    fs::set_permissions(db_parent, fs::Permissions::from_mode(0o700)).unwrap();
    fs::copy(fixture_dir.join("prepared-before.sqlite"), &db_path).unwrap();
    fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut owner_document = packet["owner_config_document"].clone();
    owner_document["uid"] = json!(fs::metadata(&source_root).unwrap().uid());
    private_json(&owner_path, &owner_document);
    packet["source_root"] = json!(source_root);
    packet["db_path"] = json!(db_path);
    packet["owner_config"] = json!(owner_path);
    (source_root, db_path, owner_path)
}
fn verify_frozen_metadata_evidence(fixture_dir: &Path, deadline: Instant) {
    let provenance = read_packet(&fixture_dir.join("PROVENANCE.json"));
    assert_eq!(
        required(&provenance, "schema_version"),
        "tos_legacy_python_metadata_oracle_evidence_v1"
    );
    assert!(required(&provenance, "classification").contains("oracle-evidence-only"));
    assert!(required(&provenance, "classification").contains("not source-runtime acceptance"));
    let artifacts = provenance["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 7);
    for artifact in artifacts {
        assert!(Instant::now() < deadline);
        let relative = Path::new(required(artifact, "path"));
        assert!(relative.components().all(|part| matches!(part, Component::Normal(_))));
        let path = fixture_dir.join(relative);
        assert!(path.starts_with(fixture_dir));
        let cap = match relative.file_name().unwrap().to_str().unwrap() {
            "prepared-before.sqlite" => 629_145_600,
            "prepared-before.packet.json" | "full-union-oracle.json" => 16_777_216,
            "owner-before.json" | "source-create-receipt.json" => 1_048_576,
            "export-capture-manifest.json" | "oracle-capture-manifest.json" => 1_048_576,
            _ => panic!("unexpected frozen Metadata evidence file"),
        };
        assert_eq!(
            native_child::bounded_sha_before(&path, cap, deadline).to_hex(),
            required(artifact, "sha256")
        );
    }
}

#[test]
fn maintained_initial_metadata_whole_transaction_and_access() {
    // Whole deadline precedes image hashes, fixture/capture scans and all writes.
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = Arc::new(AtomicBool::new(false));
    let repository = super::validation_cut_cases::repository();
    let consumer = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("protected native owner C"),
    );
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let e = std::env::current_exe().expect("current conformance executable E");
    for (path, cap) in [
        (&e, 512u64 * 1024 * 1024),
        (&consumer, 512 * 1024 * 1024),
        (&worker_path, 128 * 1024 * 1024),
    ] {
        assert!(fs::metadata(path).unwrap().len() <= cap);
    }
    let e_sha = native_child::bounded_sha_before(&e, 512 * 1024 * 1024, deadline);
    let c_sha = native_child::bounded_sha_before(&consumer, 512 * 1024 * 1024, deadline);
    let w_sha = native_child::bounded_sha_before(&worker_path, 128 * 1024 * 1024, deadline);
    let workspace = tempfile::tempdir().unwrap();
    let fixture_dir = repository.join("tests/conformance/rust/legacy-python-oracles-v1/metadata");
    verify_frozen_metadata_evidence(&fixture_dir, deadline);
    let mut packet = read_packet(&fixture_dir.join("prepared-before.packet.json"));
    let (root, db_path, owner) =
        materialize_metadata_predecessor(&fixture_dir, &mut packet, workspace.path());
    let root = root.canonicalize().unwrap();
    assert!(root.starts_with(workspace.path().canonicalize().unwrap()));
    assert!(owner.starts_with(&root) && db_path.starts_with(&root));
    // The native DB fence requires its immediate parent to be fixture-private.
    let db_parent = db_path.parent().unwrap();
    assert_eq!(db_parent.canonicalize().unwrap(), db_parent);
    assert_eq!(
        fs::metadata(db_parent).unwrap().uid(),
        fs::metadata(&root).unwrap().uid()
    );
    fs::set_permissions(db_parent, fs::Permissions::from_mode(0o700)).unwrap();
    // Only this synthetic fixture is inventoried. No authored host scan or
    // estimation run; reserve the bounded static increments below before DB work.
    let (baseline, _) = fixture_physical_bytes(&[workspace.path().to_owned()], deadline);
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(!PathBuf::from(format!("{}{suffix}", db_path.display())).exists());
    }
    let baseline_db = fs::metadata(&db_path)
        .unwrap()
        .blocks()
        .checked_mul(512)
        .unwrap();
    // Replace the DB component of F with the full DB/WAL reserve; no baseline
    // database double counting or second auxiliary database copy.
    assert!(baseline.checked_sub(baseline_db).unwrap() + 480 * 1024 * 1024 <= 1024 * 1024 * 1024);
    fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600)).unwrap();
    let (original_files, original_modes) = agent_authored_capture(&root, deadline, 33_554_432);
    let original_store = workspace.path().join("original-cut");
    let original_revision = super::validation_cut_cases::write_cut_store_with_modes(
        &original_files,
        &original_store,
        &original_modes,
    );
    let source_catalog: Value = serde_json::from_str(required(
        &packet["source_inputs"]["roots"]["source-catalog"],
        "root_json",
    ))
    .unwrap();
    let mut names: BTreeSet<String> = AGENT_RECORD_COMPONENTS
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    // Fixed initial-creation rule inputs not carried by the catalog execution profile.
    names.extend([
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py"
            .to_owned(),
        "scripts/source_witness_bibliographic_graph_common.py".to_owned(),
    ]);
    for reference in source_catalog["header"]["profile_bindings"]["execution"]
        .as_object()
        .unwrap()
        .keys()
    {
        if !reference.starts_with("ToS/") {
            names.insert(reference.clone());
        }
    }
    assert!(names.len() <= 64);
    let mut software_bytes = 0usize;
    for name in &names {
        assert!(Instant::now() < deadline);
        let path = repository.join(name);
        let size = fs::metadata(&path).unwrap().len();
        assert!(size <= 2_097_152);
        software_bytes = software_bytes.checked_add(size as usize).unwrap();
        assert!(software_bytes <= 33_554_432);
        let target = root.join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, fs::read(path).unwrap()).unwrap();
    }
    let capture = workspace.path().join("software-capture");
    let restored = workspace.path().join("software-restored");
    let mut revision = Command::new("git");
    revision
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"]);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            revision.env_remove(name);
        }
    }
    revision
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    let output = native_child::bounded_output_before(&mut revision, 4096, deadline);
    assert!(output.status.success());
    let commit = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    let include_prefixes = names.iter().cloned().collect::<Vec<_>>();
    let captured = capture_git(
        CaptureGitRequest {
            repository: &repository,
            commit: &commit,
            include_prefixes: &include_prefixes,
            exclude_prefixes: &[],
            exclude_path_parts: &[],
            output: &capture,
        },
        GitCaptureLimits {
            max_members: 512,
            max_member_bytes: 2_097_152,
            max_source_bytes: 33_554_432,
            max_metadata_bytes: 4_194_304,
            max_tree_bytes: 4_194_304,
            max_archive_bytes: 33_554_432,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let capture_raw = fs::read(capture.join("capture.json")).unwrap();
    assert!(capture_raw.len() <= 1_048_576);
    assert_eq!(Digest256::of_bytes(&capture_raw), captured.manifest_sha256);
    let manifest: Value = serde_json::from_slice(&capture_raw).unwrap();
    let source_git_tree = captured
        .manifest
        .object_get("source_git_tree")
        .and_then(|value| value.as_str())
        .expect("native capture returns its exact Git tree")
        .to_owned();
    assert_eq!(manifest["source_git_commit"].as_str().unwrap(), commit);
    assert_eq!(manifest["source_git_tree"].as_str().unwrap(), source_git_tree);
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree,
        capture_manifest_sha256: captured.manifest_sha256,
    };
    let read_limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2_097_152,
        json: JsonLimits::default(),
    };
    restore_capture(
        &capture,
        &restored,
        &selection,
        CaptureRestoreLimits {
            metadata: read_limits,
            max_archive_bytes: 33_554_432,
            max_decoded_bytes: 33_554_432,
            max_source_bytes: 33_554_432,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let software = SoftwareCaptureReader::open(
        &capture,
        &restored,
        selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: 2_097_152,
            json: JsonLimits::default(),
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let paths = names
        .iter()
        .map(|p| RelativePath::parse(p).unwrap())
        .collect::<Vec<_>>();
    software.select_components(&paths).unwrap(); // Selected capture validates the concrete component union.
    let invocation_path = workspace.path().join("metadata-native-invocation.json");
    let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,"owner_context":null,"assessment_schema_worker":null,"native_executable":consumer,"native_executable_sha256":c_sha.to_prefixed(),"corpus_store":original_store,"source_revision":original_revision.0.to_prefixed(),"original_source_revision":original_revision.0.to_prefixed(),"software_capture":capture,"software_restored_root":restored,"software_selection":{"source_git_commit":selection.source_git_commit,"source_git_tree":selection.source_git_tree,"capture_manifest_sha256":selection.capture_manifest_sha256.to_prefixed()},"software_components":names,"schema_worker":{"absolute_path":worker_path,"sha256":w_sha.to_prefixed()},"budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    private_json(&invocation_path, &invocation);
    let mut request = packet["creation_request"].clone();
    request["operation"] = json!("prepare-create");
    for key in [
        "command_id",
        "expected_configuration",
        "expected_dependencies",
        "expected_revision",
        "expected_source",
    ] {
        request.as_object_mut().unwrap().remove(key);
    }
    let preview =
        agent_native_call(&repository, &owner, &invocation_path, &request, deadline)["result"]
            .clone();
    request["operation"] = json!("source.create");
    request["command_id"] = json!("synthetic:actual-native-Metadata-addition");
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    request["expected_revision"] = Value::Null;
    request["expected_source"] = Value::Null;
    let created =
        agent_native_call(&repository, &owner, &invocation_path, &request, deadline)["result"]
            .clone();
    assert_eq!(created["replayed"], false);
    let (current_files, current_modes) = agent_authored_capture(&root, deadline, 33_554_432);
    // One corpus store retains the original authenticated revision while the
    // new current revision is appended; the fixed CLI opens BOTH through it.
    let current_store = original_store.clone();
    let current_revision = super::validation_cut_cases::write_cut_store_with_optional_modes(
        &current_files,
        &current_store,
        Some(original_revision),
        Some(&current_modes),
    );
    // Genuine native creation is observed by the exact maintained full oracle
    // BEFORE any native prepared profile transition changes Python identities.
    let oracle = read_packet(&fixture_dir.join("full-union-oracle.json"));
    assert_eq!(agent_authored(&root, deadline), current_files);
    let publication = PublicationLimits {
        max_bytes: 67_108_864,
        max_mutations: 100_000,
        ..PublicationLimits::default()
    };
    let catalog_limits = CatalogMaintenanceLimits {
        max_delta_bytes: 16_777_216,
        max_catalog_entries: 16_384,
        max_aggregate_bytes: 16_777_216,
        max_catalog_bytes: 8_388_608,
        max_index_bytes: 67_108_864,
        ..CatalogMaintenanceLimits::default()
    };
    let semantic_limits = SemanticMaintenanceLimits {
        max_rows: 100_000,
        max_queries: 100_000,
        max_writes: 100_000,
        max_read_bytes: 33_554_432,
        max_input_bytes: 16_777_216,
        max_input_values: 1_000_000,
        max_output_items: 16_384,
        max_bytes: 67_108_864,
        ..SemanticMaintenanceLimits::default()
    };
    let operation = ClaimPublicationLimits::default();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.busy_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA page_size", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        4096
    );
    connection
        .execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF; PRAGMA max_page_count=16384")
        .unwrap();
    let initial_binding = typed(&packet["binding"]);
    let initial_catalog = agent_catalog(&packet, &packet["header"]);
    {
        // Fresh native auxiliary index from the SAME immutable normalized
        // predecessor. No Python projector digest is relabelled.
        let tx = connection.unchecked_transaction().unwrap();
        for table in [
            "semantic_pending",
            "semantic_diagnostics",
            "semantic_cardinality_errors",
            "semantic_cardinality",
            "semantic_deps",
            "semantic_edges",
            "semantic_rows",
            "semantic_state",
        ] {
            tx.execute(&format!("DROP TABLE {table}"), []).unwrap();
        }
        let report = prepared_semantic_index::bootstrap_semantic_index_transaction(
            &tx,
            &initial_binding,
            &initial_catalog.entity_registry,
            &initial_catalog.relation_registry,
            None,
            required(
                &packet["header"]["normalization_binding"],
                "processor_digest",
            ),
            semantic_limits,
        )
        .unwrap();
        assert_eq!(
            report_object_order(&report),
            report_object_order(&typed(&packet["baseline_semantic_report"]))
        );
        // Import the maintained Python catalog through the same native bootstrap
        // used by Claim publication. Reproduce its full catalog/header before
        // binding the native auxiliary index; never relabel its projector.
        for table in [
            "catalog_heads",
            "catalog_occurrences",
            "catalog_contributors",
            "catalog_totals",
            "catalog_atoms",
            "catalog_state",
        ] {
            tx.execute(&format!("DROP TABLE {table}"), []).unwrap();
        }
        let catalog_receipt =
            tos_compiler::prepared_maintenance::bootstrap_prepared_catalog_transaction(
                &tx,
                &initial_binding,
                &initial_catalog,
                publication,
                catalog_limits,
            )
            .unwrap();
        assert_eq!(catalog_receipt.binding, initial_binding);
        assert!(!catalog_receipt.publication_changed && !catalog_receipt.consumer_switched);
        tx.commit().unwrap();
    }
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    let companions = root.join("metadata-prepared-companions");
    fs::create_dir(&companions).unwrap();
    fs::set_permissions(&companions, fs::Permissions::from_mode(0o700)).unwrap();
    let binding_path = companions.join("binding.json");
    let source_path = companions.join("source.json");
    let catalog_path = companions.join("catalog.json");
    let descriptor_path = companions.join("descriptor.json");
    private_json(&binding_path, &packet["binding"]);
    private_json(&source_path, &packet["source_inputs"]);
    private_json(
        &catalog_path,
        &json!({"header":packet["header"],"entity_registry":packet["entities"],"relation_registry":packet["relations"],"lenses":packet["lenses"],"source_order_profile":"source-graph-id-v1"}),
    );
    private_json(&descriptor_path, &packet["descriptor"]);
    invocation["schema_version"] = json!("tos_local_native_metadata_publication_invocation_v1");
    invocation["corpus_store"] = json!(current_store);
    invocation["source_revision"] = json!(current_revision.0.to_prefixed());
    invocation["prepared_database"] = json!(db_path);
    invocation["expected_binding_path"] = json!(binding_path);
    invocation["source_inputs_path"] = json!(source_path);
    invocation["source_inputs_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&source_path).unwrap()).to_prefixed());
    invocation["catalog_path"] = json!(catalog_path);
    invocation["catalog_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&catalog_path).unwrap()).to_prefixed());
    invocation["descriptor_path"] = json!(descriptor_path);
    invocation["descriptor_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&descriptor_path).unwrap()).to_prefixed());
    invocation["publication_limits"] = json!({"operation":{"max_nodes":operation.max_nodes,"max_relations":operation.max_relations,"max_claims":operation.max_claims,"max_bytes":operation.max_bytes,"max_row_bytes":operation.max_row_bytes,"max_contexts":operation.max_contexts,"max_vm_steps":operation.max_vm_steps,"cow_target_bytes":operation.cow_target_bytes},"publication":publication,"catalog":catalog_limits,"semantic":semantic_limits,"bibliographic":{"max_claim_cohort_rows":16,"max_claim_cohort_bytes":16_777_216,"max_output_rows":4096,"max_output_bytes":16_777_216}});
    invocation["reviewed_execution_transition"] = Value::Null;
    invocation["reviewed_metadata_transition"] = Value::Null;
    invocation["expected_creation_receipt_sha256"] = Value::Null;
    invocation["expected_creation_request_digest"] = Value::Null;
    private_json(&invocation_path, &invocation);
    let identity = agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &json!({"action":"describe-metadata-execution"}),
        deadline,
    )["result"]
        .clone();
    assert_eq!(identity["processor"], c_sha.to_hex());
    let deps = &packet["source_inputs"]["dependencies"];
    let mut after_normalization = packet["header"]["normalization_binding"].clone();
    after_normalization["processor_digest"] = identity["processor"].clone();
    invocation["reviewed_execution_transition"] = json!({"dependency_implementation_before":packet["dependency_implementation_before"],"declaration_before":deps["declaration-profile"],"agent_publication_before":deps["agent-publication-profile"],"claim_publication_before":deps.get("claim-publication-profile").unwrap_or(&Value::Null),"normalization_before":packet["header"]["normalization_binding"],"normalization_after":after_normalization,"native_normalization_processor_sha256":identity["processor"],"review_ref":"test:Metadata-native-predecessor-execution-review","reviewed_after_agent_sha256":identity["agent_publication"]});
    private_json(&invocation_path, &invocation);
    let bootstrap = agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &json!({"action":"reviewed-metadata-execution-bootstrap"}),
        deadline,
    )["result"]
        .clone();
    private_json(&binding_path, &bootstrap["binding"]);
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    let source_raw: String = connection
        .query_row(
            "SELECT inputs FROM prepared_source_state WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(source_raw.len() <= 1_048_576);
    let selected_source: Value = serde_json::from_str(&source_raw).unwrap();
    private_json(&source_path, &selected_source);
    private_json(
        &catalog_path,
        &json!({"header":bootstrap["source_header"],"entity_registry":packet["entities"],"relation_registry":packet["relations"],"lenses":packet["lenses"],"source_order_profile":"source-graph-id-v1"}),
    );
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    let predecessor_binding = companions.join("predecessor-binding.json");
    private_json(&predecessor_binding, &bootstrap["binding"]);
    invocation["source_inputs_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&source_path).unwrap()).to_prefixed());
    invocation["catalog_sha256"] =
        json!(Digest256::of_bytes(&fs::read(&catalog_path).unwrap()).to_prefixed());
    invocation["reviewed_execution_transition"] = Value::Null;
    let receipt_path = root
        .join(required(&packet, "source_path"))
        .with_file_name("source-create-receipt.json");
    invocation["expected_creation_receipt_sha256"] =
        json!(native_child::bounded_sha_before(&receipt_path, 1_048_576, deadline).to_prefixed());
    invocation["expected_creation_request_digest"] = created["receipt"]["request_digest"].clone();
    if let Some(before) =
        selected_source["dependencies"].get("metadata-addition-publication-profile")
    {
        invocation["reviewed_metadata_transition"] = json!({"before_sha256":before,"after_sha256":identity["metadata_publication"],"review_ref":"test:Metadata-specific-compatible-execution-review"});
    }
    private_json(&invocation_path, &invocation);
    let published = agent_native_call(&repository,&owner,&invocation_path,&json!({"action":"publish-initial-metadata","recorded_at":created["receipt"]["recorded_at"],"creation_request":request}),deadline)["result"].clone();
    assert_eq!(published["prepared_committed"], true);
    assert_eq!(published["changed_nodes"], 3);
    assert_eq!(published["changed_relations"], 2);
    assert_eq!(agent_authored(&root, deadline), current_files);
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    for kind in ["node", "relation"] {
        let mut statement = connection
            .prepare(&format!(
                "SELECT id,json FROM knowledge_{kind}s ORDER BY id LIMIT 16385"
            ))
            .unwrap();
        let actual: BTreeMap<String, Value> = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|r| {
                let (id, raw) = r.unwrap();
                assert!(raw.len() <= 1_048_576);
                (id, serde_json::from_str(&raw).unwrap())
            })
            .collect();
        let expected: BTreeMap<String, Value> = oracle["expected"][format!("{kind}s")]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (required(r, "id").to_owned(), r.clone()))
            .collect();
        assert!(actual.len() <= 16384);
        assert_eq!(actual, expected);
    }
    assert_eq!(
        published["source_header"]["counts"]["semantic_validation"],
        oracle["expected"]["counts"]["semantic_validation"]
    );
    drop(connection);
    private_json(&binding_path, &published["binding"]);
    let old = tos_access::prepared_local::PreparedLocalExecutor::open(
        db_path.clone(),
        predecessor_binding,
        None,
    )
    .unwrap();
    let stale = tos_access::http::handle_get(
        &old,
        "GET",
        "/api/knowledge/catalog",
        tos_access::prepared_local::profile().with_query_timeout(
            Duration::from_secs(5).min(deadline.saturating_duration_since(Instant::now())),
        ),
    );
    assert_ne!(stale.status, 200);
    drop(old);
    access::verify_published_access_until(
        &db_path,
        &binding_path,
        required(&oracle, "new_node_id"),
        required(&oracle, "new_relation_id"),
        deadline,
    );
    let (physical, _) = fixture_physical_bytes(&[workspace.path().to_owned()], deadline);
    assert!(physical <= 1024 * 1024 * 1024);
    for (path, sha, cap) in [
        (&e, e_sha, 512 * 1024 * 1024),
        (&consumer, c_sha, 512 * 1024 * 1024),
        (&worker_path, w_sha, 128 * 1024 * 1024),
    ] {
        assert_eq!(native_child::bounded_sha_before(path, cap, deadline), sha);
    }
    assert!(Instant::now() < deadline);
}
