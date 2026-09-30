//! Actual maintained Claim fixture enters the native whole caller before BEGIN.
//! The same committed DB/binding continues to native CLI, HTTP and MCP readers.
use super::*;
use std::{
    collections::BTreeMap,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tos_command::source_claim_publication::{
    ClaimAdditionPublication, ClaimPublicationLimits, ClaimPublicationProgress,
    ReviewedClaimProfileTransition,
};
use tos_compiler::{
    QueryVocabulary,
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::{CatalogInputs, SourceOrderProfile},
    prepared_semantic_index::{self, SemanticMaintenanceLimits},
    prepared_source_binding::PreparedSourceInputs,
    source_bibliographic::BibliographicLimits,
    source_witness_catalog::SourceCatalogLimits,
};
#[path = "claim_publication_access.rs"]
mod claim_publication_access;
#[path = "../../../rust/crates/tos-access/tests/support/native_child.rs"]
mod native_child;
fn typed(value: &Value) -> JsonValue {
    parse_json(
        &canonical_json(value),
        JsonMode::PublishedStrict,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap()
    .root()
    .clone()
}
fn canonical_lf(value: &Value) -> Vec<u8> {
    canonical_raw_bytes_v1(
        &serde_json::to_vec(value).unwrap(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap()
}
fn read_packet(path: &Path) -> Value {
    assert!(fs::metadata(path).unwrap().len() <= 16_777_216);
    let raw = fs::read(path).unwrap();
    parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap();
    serde_json::from_slice(&raw).unwrap()
}
fn fixture_physical_bytes(roots: &[PathBuf], deadline: Instant) -> (u64, usize) {
    use std::os::unix::fs::MetadataExt;
    let mut pending: Vec<_> = roots.iter().cloned().map(|p| (p, 0usize)).collect();
    let mut entries = 0usize;
    let mut physical = 0u64;
    while let Some((path, depth)) = pending.pop() {
        assert!(Instant::now() < deadline);
        entries += 1;
        assert!(
            entries <= 16_384 && depth <= 64,
            "finite synthetic fixture inventory"
        );
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "synthetic fixture symlink"
        );
        assert!(
            metadata.is_file() || metadata.is_dir(),
            "synthetic fixture member type"
        );
        physical = physical
            .checked_add(metadata.blocks().checked_mul(512).unwrap())
            .unwrap();
        assert!(
            physical <= 512 * 1024 * 1024,
            "synthetic baseline physical budget"
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                assert!(pending.len() + entries < 16_384);
                pending.push((entry.unwrap().path(), depth + 1));
            }
        }
    }
    (physical, entries)
}

#[test]
fn maintained_claim_addition_whole_transaction_and_access() {
    let deadline = Instant::now() + Duration::from_secs(240);
    // Observe exact products before fixture or material writes. These reads
    // do not copy binaries into the disposable workspace.
    let executable = Path::new("/proc/self/exe");
    let consumer = PathBuf::from(std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN").unwrap());
    assert!(consumer.is_absolute());
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let executable_bytes = fs::metadata(executable).unwrap().len();
    let consumer_bytes = fs::metadata(&consumer).unwrap().len();
    let worker_bytes = fs::metadata(&worker_path).unwrap().len();
    assert!(executable_bytes <= 512 * 1024 * 1024);
    assert!(consumer_bytes <= 512 * 1024 * 1024);
    assert!(worker_bytes <= 128 * 1024 * 1024);
    let executable_sha = native_child::bounded_sha_before(executable, 512 * 1024 * 1024, deadline);
    let consumer_sha = native_child::bounded_sha_before(&consumer, 512 * 1024 * 1024, deadline);
    let worker_sha = native_child::bounded_sha_before(&worker_path, 128 * 1024 * 1024, deadline);
    assert_eq!(
        executable_sha.to_hex(),
        std::env::var("TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256").unwrap()
    );
    assert_eq!(
        consumer_sha.to_hex(),
        std::env::var("TOS_NATIVE_PREPARED_CONSUMER_SHA256").unwrap()
    );
    eprintln!(
        "Claim whole preflight E={executable_bytes}:{} C={consumer_bytes}:{} W={worker_bytes}:{}",
        executable_sha.to_hex(),
        consumer_sha.to_hex(),
        worker_sha.to_hex()
    );
    assert!(Instant::now() < deadline);
    let repository = super::validation_cut_cases::repository();
    let workspace = tempfile::tempdir().unwrap();
    let packet_path = if let Some(path) = std::env::var_os("TOS_NATIVE_CLAIM_PUBLICATION_FIXTURE") {
        PathBuf::from(path)
    } else {
        let path = workspace.path().join("claim-fixture.json");
        let mut command = Command::new(
            std::env::var_os("TOS_MAINTAINED_PYTHON").unwrap_or_else(|| "python3".into()),
        );
        command
            .arg(repository.join("tests/conformance/rust/source_claim_publication_fixture.py"))
            .arg(workspace.path())
            .arg(&path)
            .env("PYTHONDONTWRITEBYTECODE", "1");
        let output = native_child::bounded_output_before(
            &mut command,
            4096,
            deadline.min(Instant::now() + Duration::from_secs(30)),
        );
        assert!(
            output.status.success(),
            "maintained fixture export refused: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        path
    };
    assert!(packet_path.is_absolute());
    let packet = read_packet(&packet_path);
    let db_path = PathBuf::from(required(&packet, "db_path"))
        .canonicalize()
        .unwrap();
    let owner_config = PathBuf::from(required(&packet, "owner_config"))
        .canonicalize()
        .unwrap();
    assert!(fs::metadata(&owner_config).unwrap().len() <= 1_048_576);
    let config_raw = fs::read(&owner_config).unwrap();
    let config: Value = serde_json::from_slice(&config_raw).unwrap();
    let fixture_root = PathBuf::from(required(&config, "source_root"));
    assert!(fixture_root.is_absolute());
    // The optional Python branch supplies its actual disposable fixture, never
    // an authored repository or arbitrary host tree for this metadata walk.
    let temporary_base = std::env::temp_dir().canonicalize().unwrap();
    assert!(
        !fs::symlink_metadata(&fixture_root)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let fixture_root = fixture_root.canonicalize().unwrap();
    let workspace_root = workspace.path().canonicalize().unwrap();
    assert!(fixture_root.starts_with(&temporary_base) && fixture_root != temporary_base);
    assert!(owner_config.starts_with(&fixture_root) && db_path.starts_with(&fixture_root));
    let inventory_roots = if fixture_root.starts_with(&workspace_root) {
        vec![workspace_root]
    } else {
        vec![workspace_root, fixture_root]
    };
    let selected_packet = packet_path.canonicalize().unwrap();
    assert!(
        inventory_roots
            .iter()
            .any(|root| selected_packet.starts_with(root))
    );
    for output in ["binding_path", "receipt_path"] {
        let parent = Path::new(required(&packet, output))
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        assert!(inventory_roots.iter().any(|root| parent.starts_with(root)));
    }
    let (fixture_bytes, fixture_entries) = fixture_physical_bytes(&inventory_roots, deadline);
    eprintln!("Claim whole preflight F={fixture_bytes} physical bytes entries={fixture_entries}");
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.busy_timeout(Duration::from_secs(2)).unwrap();
    assert!(connection.is_autocommit());
    let cancel = Arc::new(AtomicBool::new(false));
    let progress =
        ClaimPublicationProgress::install(&connection, cancel.clone(), deadline, 100_000_000)
            .unwrap();
    connection
        .execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF")
        .unwrap();
    // Finite maintained consumer envelope; library defaults remain portable.
    let publication_limits = PublicationLimits {
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
    let binding = typed(&packet["binding"]);
    let old_binding_path = workspace.path().join("old-binding.json");
    fs::write(&old_binding_path, canonical_lf(&packet["binding"])).unwrap();
    let catalog = CatalogInputs {
        header: typed(&packet["header"]),
        entity_registry: typed(&packet["entities"]),
        relation_registry: typed(&packet["relations"]),
        lenses: packet["lenses"]
            .as_array()
            .unwrap()
            .iter()
            .map(typed)
            .collect(),
        source_order_profile: SourceOrderProfile::SourceGraphId,
    };
    let source =
        PreparedSourceInputs::parse(&canonical_lf(&packet["source_inputs"]), publication_limits)
            .unwrap();
    // Explicit native auxiliary bootstrap over the SAME immutable predecessor.
    // Preserve the original Python semantic rows in a bounded reference packet;
    // no executable hash is relabeled on the imported index.
    let reference = workspace.path().join("python-semantic-reference.json");
    let mut semantic_reference = BTreeMap::new();
    let mut reference_bytes = 0usize;
    for table in [
        "semantic_state",
        "semantic_rows",
        "semantic_edges",
        "semantic_deps",
        "semantic_cardinality",
        "semantic_cardinality_errors",
        "semantic_diagnostics",
        "semantic_pending",
    ] {
        let mut stmt = connection
            .prepare(&format!("SELECT * FROM {table} LIMIT 16385"))
            .unwrap();
        let count = stmt.column_count();
        let rows: Vec<Vec<Value>> = stmt
            .query_map([], |r| {
                let mut values = Vec::new();
                for i in 0..count {
                    let value = match r.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(n) => Value::from(n),
                        rusqlite::types::ValueRef::Text(v) => {
                            Value::String(std::str::from_utf8(v).unwrap().to_owned())
                        }
                        _ => panic!("semantic fixture scalar ABI"),
                    };
                    reference_bytes += match &value {
                        Value::String(s) => s.len(),
                        _ => 32,
                    };
                    assert!(reference_bytes <= 16_777_216);
                    values.push(value);
                }
                Ok(values)
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(rows.len() <= 16384);
        semantic_reference.insert(table, rows);
    }
    let raw = serde_json::to_vec(&semantic_reference).unwrap();
    assert!(raw.len() <= 16_777_216);
    fs::write(reference, raw).unwrap();
    drop(semantic_reference);
    {
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
            &binding,
            &catalog.entity_registry,
            &catalog.relation_registry,
            None,
            required(
                &packet["header"]["normalization_binding"],
                "processor_digest",
            ),
            semantic_limits,
        )
        .unwrap();
        assert_eq!(typed(&packet["baseline_semantic_report"]), report);
        tx.commit().unwrap();
    }
    // Real bounded schema worker over an actual sealed synthetic source cut.
    let mut files = BTreeMap::new();
    let mut bytes = 0usize;
    for (path, hex) in packet["source_files"].as_object().unwrap() {
        let raw = decode_hex(hex.as_str().unwrap());
        bytes += raw.len();
        assert!(bytes <= 16_777_216);
        files.insert(path.clone(), raw);
    }
    assert!(files.len() <= 2048);
    // Existing helper emits one fixed record per member. Bound its exact
    // escaped path contribution plus conservative256-byte framing BEFORE write.
    let manifest_upper = files
        .keys()
        .try_fold(512usize, |total, path| {
            total.checked_add(serde_json::to_vec(path).unwrap().len() + 256)
        })
        .unwrap();
    assert!(manifest_upper <= 4_194_304);
    let cut_root = workspace.path().join("schema-cut");
    let revision = super::validation_cut_cases::write_cut_store(&files, &cut_root);
    drop(files);
    let cut = super::command_form_cases::open_cut(&cut_root, revision, deadline, &cancel);
    let mut worker = super::command_form_cases::schemas(&cut, deadline, &cancel);
    let descriptor = serde_json::to_vec(&packet["descriptor"]).unwrap();
    let vocabulary =
        QueryVocabulary::parse(&descriptor, tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES)
            .unwrap();
    let artifact = std::env::var("TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256")
        .expect("OPS exact current conformance artifact SHA required");
    Digest256::from_hex(&artifact).unwrap();
    let before = packet["header"]["normalization_binding"].clone();
    let mut after = before.clone();
    after["processor_digest"] = Value::String(artifact.clone());
    let dependencies = &packet["source_inputs"]["dependencies"];
    let review = ReviewedClaimProfileTransition {
        dependency_implementation_before: required(&packet, "dependency_implementation_before")
            .to_owned(),
        declaration_before: required(dependencies, "declaration-profile").to_owned(),
        agent_publication_before: required(dependencies, "agent-publication-profile").to_owned(),
        claim_publication_before: dependencies
            .get("claim-publication-profile")
            .and_then(Value::as_str)
            .map(str::to_owned),
        normalization_before: before,
        normalization_after: after,
        native_normalization_processor_sha256: artifact,
        review_ref: "test:maintained-claim-whole-source-compatibility-review".to_owned(),
    };
    let bibliographic = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 2048,
            max_rows: 4096,
            max_file_bytes: 16_777_216,
            max_row_bytes: 1_048_576,
            max_contract_bytes: 4_194_304,
            max_output_row_bytes: 1_048_576,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16_777_216,
        max_output_rows: 4096,
        max_output_bytes: 16_777_216,
        deadline,
    };
    let mut operation = ClaimAdditionPublication::prepare(
        Path::new(required(&packet, "owner_config")),
        source,
        binding,
        catalog,
        Digest256::from_hex(required(&packet, "expected_receipt_sha256")).unwrap(),
        Digest256::from_hex(required(&packet, "expected_request_digest")).unwrap(),
        review,
        vocabulary,
        descriptor,
        ClaimPublicationLimits::default(),
        bibliographic,
        &mut worker,
        cancel.clone(),
    )
    .unwrap();
    let tx =
        rusqlite::Transaction::new_unchecked(&connection, rusqlite::TransactionBehavior::Immediate)
            .unwrap();
    let result = operation
        .apply_transaction(
            &tx,
            &progress,
            publication_limits,
            catalog_limits,
            semantic_limits,
        )
        .unwrap();
    assert_eq!(result["prepared_committed"], false);
    let result = operation.commit_transaction(tx, &progress).unwrap();
    drop(worker);
    drop(cut);
    assert_eq!(result["prepared_committed"], true);
    let expected = &packet["expected"];
    for kind in ["node", "relation"] {
        let mut stmt = connection
            .prepare(&format!(
                "SELECT id,json FROM knowledge_{kind}s ORDER BY id LIMIT 16385"
            ))
            .unwrap();
        let actual: BTreeMap<String, Value> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|r| {
                let (id, raw) = r.unwrap();
                (id, serde_json::from_str(&raw).unwrap())
            })
            .collect();
        assert!(actual.len() <= 16384);
        let expected: BTreeMap<String, Value> = expected[format!("{kind}s")]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (required(r, "id").to_owned(), r.clone()))
            .collect();
        assert!(
            actual == expected,
            "maintained full union {kind} oracle mismatch"
        );
    }
    assert_eq!(
        result["semantic_report"],
        expected["counts"]["semantic_validation"]
    );
    let binding_path = Path::new(required(&packet, "binding_path"));
    fs::write(binding_path, canonical_lf(&result["binding"])).unwrap();
    fs::write(required(&packet, "receipt_path"), canonical_lf(&result)).unwrap();
    drop(progress);
    drop(connection);
    let old_executor = tos_access::prepared_local::PreparedLocalExecutor::open(
        db_path.clone(),
        old_binding_path,
        None,
    )
    .unwrap();
    let stale = tos_access::http::handle_get(
        &old_executor,
        "GET",
        "/api/knowledge/catalog",
        tos_access::prepared_local::profile().with_query_timeout(
            Duration::from_secs(5).min(deadline.checked_duration_since(Instant::now()).unwrap()),
        ),
    );
    assert_ne!(
        stale.status, 200,
        "old selected binding must refuse the successor"
    );
    drop(old_executor);
    claim_publication_access::verify_published_access_until(
        &db_path,
        binding_path,
        required(&packet, "new_node_id"),
        required(&packet, "new_relation_id"),
        deadline,
    );
}
