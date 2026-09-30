//! The maintained initial Metadata oracle observes a real native creation.
//! Creation, profile transition and publication use the SAME protected owner C;
//! its committed database/binding then continue through the real access lanes.
use super::command_claim_publication_cases::{
    AGENT_RECORD_COMPONENTS, agent_authored, agent_catalog, agent_native_call, canonical_lf,
    fixture_physical_bytes, read_packet, typed,
};
use super::*;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::{MetadataExt, PermissionsExt},
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
use tos_source_store::{ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1};
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
fn python(command: &mut Command, deadline: Instant) {
    command
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME");
    let output = native_child::bounded_output_before(
        command,
        4096,
        deadline.min(Instant::now() + Duration::from_secs(60)),
    );
    assert!(
        output.status.success(),
        "Metadata fixture/capture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn maintained_initial_metadata_whole_transaction_and_access() {
    // Whole deadline precedes image hashes, fixture/capture scans and all writes.
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = Arc::new(AtomicBool::new(false));
    let repository = super::validation_cut_cases::repository();
    let consumer = PathBuf::from(
        std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN").expect("protected native owner C"),
    );
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let e = PathBuf::from("/proc/self/exe");
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
    let packet_path = workspace.path().join("metadata-fixture.json");
    let fixture = repository.join("tests/conformance/rust/source_metadata_publication_fixture.py");
    python(
        Command::new("/usr/bin/python3")
            .arg(&fixture)
            .arg(workspace.path())
            .arg(&packet_path),
        deadline,
    );
    let packet = read_packet(&packet_path);
    let root = PathBuf::from(required(&packet, "source_root"))
        .canonicalize()
        .unwrap();
    assert!(root.starts_with(workspace.path().canonicalize().unwrap()));
    let owner = PathBuf::from(required(&packet, "owner_config"));
    let db_path = PathBuf::from(required(&packet, "db_path"));
    assert!(owner.starts_with(&root) && db_path.starts_with(&root));
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
    let original_files = agent_authored(&root, deadline);
    let original_store = workspace.path().join("original-cut");
    let original_revision =
        super::validation_cut_cases::write_cut_store(&original_files, &original_store);
    let source_catalog: Value = serde_json::from_str(required(
        &packet["source_inputs"]["roots"]["source-catalog"],
        "root_json",
    ))
    .unwrap();
    let mut names: BTreeSet<String> = AGENT_RECORD_COMPONENTS
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
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
    let mut archive = Command::new("/usr/bin/python3");
    archive
        .arg(repository.join("scripts/corpus_archive.py"))
        .arg("capture")
        .arg("--repo-root")
        .arg(&repository)
        .arg("--commit")
        .arg(&commit)
        .arg("--output")
        .arg(&capture);
    for name in &names {
        archive.arg("--include-prefix").arg(name);
    }
    python(&mut archive, deadline);
    python(
        Command::new("/usr/bin/python3")
            .arg(repository.join("scripts/corpus_archive.py"))
            .arg("restore")
            .arg("--capture")
            .arg(&capture)
            .arg("--output")
            .arg(&restored),
        deadline,
    );
    let capture_raw = fs::read(capture.join("capture.json")).unwrap();
    assert!(capture_raw.len() <= 1_048_576);
    let manifest: Value = serde_json::from_slice(&capture_raw).unwrap();
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree: required(&manifest, "source_git_tree").to_owned(),
        capture_manifest_sha256: Digest256::of_bytes(&capture_raw),
    };
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
    let current_files = agent_authored(&root, deadline);
    // One corpus store retains the original authenticated revision while the
    // new current revision is appended; the fixed CLI opens BOTH through it.
    let current_store = original_store.clone();
    let current_revision = super::validation_cut_cases::write_cut_store_on_base(
        &current_files,
        &current_store,
        Some(original_revision),
    );
    // Genuine native creation is observed by the exact maintained full oracle
    // BEFORE any native prepared profile transition changes Python identities.
    let oracle_path = workspace.path().join("metadata-independent-oracle.json");
    python(
        Command::new("/usr/bin/python3")
            .arg(&fixture)
            .arg("oracle")
            .arg(&packet_path)
            .arg(&oracle_path),
        deadline,
    );
    let oracle = read_packet(&oracle_path);
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
        assert_eq!(report, typed(&packet["baseline_semantic_report"]));
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
