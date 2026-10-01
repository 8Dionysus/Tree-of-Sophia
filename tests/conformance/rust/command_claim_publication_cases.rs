//! Actual maintained Claim fixture enters the native whole caller before BEGIN.
//! The same committed DB/binding continues to native CLI, HTTP and MCP readers.
use super::*;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
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
pub(super) fn typed(value: &Value) -> JsonValue {
    parse_json(
        &canonical_json(value),
        JsonMode::PublishedStrict,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap()
    .root()
    .clone()
}
// Compare the full semantic report independently of object encounter order.
// Preserve every scalar (including number kind/lexeme) and array position.
pub(super) fn report_object_order(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(entries) => {
            let mut entries = entries
                .iter()
                .map(|(key, value)| (key.clone(), report_object_order(value)))
                .collect::<Vec<_>>();
            entries.sort_by(|a, b| a.0.units().cmp(b.0.units()));
            JsonValue::Object(entries)
        }
        JsonValue::Array(entries) => {
            JsonValue::Array(entries.iter().map(report_object_order).collect())
        }
        scalar => scalar.clone(),
    }
}
pub(super) fn canonical_lf(value: &Value) -> Vec<u8> {
    canonical_raw_bytes_v1(
        &serde_json::to_vec(value).unwrap(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(16_777_216, 128, 2_000_000, 4096).unwrap(),
    )
    .unwrap()
}
pub(super) fn read_packet(path: &Path) -> Value {
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
pub(super) fn fixture_physical_bytes(roots: &[PathBuf], deadline: Instant) -> (u64, usize) {
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

// Same local failure-custody pattern as Item/PublicText; no source copy.
// Only the explicit narrow launcher opt-in retains a failed fixture.
struct PublicationFailureFixture {
    directory: tempfile::TempDir,
    label: &'static str,
    retain_failure: bool,
}
impl PublicationFailureFixture {
    fn new(label: &'static str) -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
            label,
            retain_failure: std::env::var_os("TOS_NATIVE_CLAIM_PUBLICATION_RETAIN_FIXTURE")
                .as_deref()
                == Some(std::ffi::OsStr::new("1")),
        }
    }
    fn path(&self) -> &Path {
        self.directory.path()
    }
}
impl Drop for PublicationFailureFixture {
    fn drop(&mut self) {
        if self.retain_failure && std::thread::panicking() {
            self.directory.disable_cleanup(true);
            eprintln!(
                "{} failed fixture retained at {}",
                self.label,
                self.path().display()
            );
        }
    }
}

#[test]
fn maintained_claim_addition_whole_transaction_and_access() {
    let deadline = Instant::now() + Duration::from_secs(240);
    // Observe exact products before fixture or material writes. These reads
    // do not copy binaries into the disposable workspace.
    let executable_path = std::env::current_exe().expect("actual protected conformance ELF path");
    let executable = executable_path.as_path();
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
    let workspace = PublicationFailureFixture::new("Claim addition");
    let packet_path = if let Some(path) = std::env::var_os("TOS_NATIVE_CLAIM_PUBLICATION_FIXTURE") {
        PathBuf::from(path)
    } else {
        let path = workspace.path().join("claim-fixture.json");
        let mut command = Command::new(
            std::env::var_os("TOS_MAINTAINED_PYTHON")
                .expect("explicit maintained fixture interpreter"),
        );
        command
            .arg(repository.join("tests/conformance/rust/source_claim_publication_fixture.py"))
            .arg(workspace.path())
            .arg(&path)
            .env("PYTHONDONTWRITEBYTECODE", "1");
        let output = native_child::bounded_output_before_diagnostic(
            &mut command,
            4096,
            deadline.min(Instant::now() + Duration::from_secs(30)),
            "claim.fixture-export",
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
    fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600)).unwrap();
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
        assert_eq!(
            report_object_order(&typed(&packet["baseline_semantic_report"])),
            report_object_order(&report)
        );
        // Import the independent Python catalog through the genuine native
        // bootstrap, which reproduces its full catalog/header before binding
        // the native auxiliary index. Never relabel the Python projector.
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
                &binding,
                &catalog,
                publication_limits,
                catalog_limits,
            )
            .unwrap();
        assert_eq!(catalog_receipt.binding, binding);
        assert!(!catalog_receipt.publication_changed && !catalog_receipt.consumer_switched);
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
        Digest256::from_prefixed(required(&packet, "expected_request_digest")).unwrap(),
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

pub(super) const AGENT_RECORD_COMPONENTS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "scripts/source_record_profiles.py",
    "scripts/native_text_binding.py",
    "scripts/source_owner_context.py",
    "scripts/source_witness_human_forms.py",
    "scripts/source_metadata_snapshot.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_selected_revisions.py",
];
pub(super) fn agent_authored(root: &Path, deadline: Instant) -> BTreeMap<String, Vec<u8>> {
    let mut pending = vec![root.join("ToS")];
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    while let Some(directory) = pending.pop() {
        assert!(Instant::now() < deadline);
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let metadata = entry.file_type().unwrap();
            assert!(!metadata.is_symlink());
            if metadata.is_dir() {
                pending.push(entry.path());
                continue;
            }
            assert!(metadata.is_file());
            let path = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            if path == "ToS/source-witnesses/.historical-create.writer.lock"
                || !tos_source_store::is_authored_source_path_v1(&path)
            {
                continue;
            }
            assert!(fs::metadata(entry.path()).unwrap().len() <= 8_388_608);
            let raw = fs::read(entry.path()).unwrap();
            total = total.checked_add(raw.len()).unwrap();
            assert!(total <= 33_554_432 && files.len() < 2048);
            files.insert(path, raw);
        }
    }
    files
}
pub(super) fn agent_catalog(packet: &Value, header: &Value) -> CatalogInputs {
    CatalogInputs {
        header: typed(header),
        entity_registry: typed(&packet["entities"]),
        relation_registry: typed(&packet["relations"]),
        lenses: packet["lenses"]
            .as_array()
            .unwrap()
            .iter()
            .map(typed)
            .collect(),
        source_order_profile: SourceOrderProfile::SourceGraphId,
    }
}
pub(super) fn agent_native_call(
    repository: &Path,
    owner: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> Value {
    use std::io::Write;
    let mut input = tempfile::NamedTempFile::new().unwrap();
    let input_raw = serde_json::to_vec(request).unwrap();
    assert!(input_raw.len() <= 1_048_576);
    input.write_all(&input_raw).unwrap();
    let mut command = Command::new(
        std::env::var_os("TOS_MAINTAINED_PYTHON").expect("explicit maintained fixture interpreter"),
    );
    command
        .arg(
            repository.join(
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
            ),
        )
        .arg("--owner-config")
        .arg(owner)
        .arg("--native-invocation")
        .arg(invocation)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(std::process::Stdio::from(input.reopen().unwrap()));
    let output = native_child::bounded_output_before(
        &mut command,
        1_048_576,
        deadline.min(Instant::now() + Duration::from_secs(60)),
    );
    assert!(
        output.status.success(),
        "actual Agent Record caller: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn agent_publication_cli_output(
    executable: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> std::process::Output {
    use std::io::Write;
    let mut input = tempfile::NamedTempFile::new().unwrap();
    let raw = serde_json::to_vec(request).unwrap();
    assert!(raw.len() <= 1_048_576);
    input.write_all(&raw).unwrap();
    let mut command = Command::new(executable);
    command
        .arg("--invocation")
        .arg(invocation)
        .stdin(std::process::Stdio::from(input.reopen().unwrap()));
    native_child::bounded_output_before(
        &mut command,
        1_048_576,
        deadline.min(Instant::now() + Duration::from_secs(60)),
    )
}
fn agent_publication_cli(
    executable: &Path,
    invocation: &Path,
    request: &Value,
    deadline: Instant,
) -> Value {
    let output = agent_publication_cli_output(executable, invocation, request, deadline);
    assert!(
        output.status.success(),
        "actual Agent publication CLI action={} status={}: {}",
        request["action"]
            .as_str()
            .expect("fixed Agent publication action"),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["grants_admission"], false);
    envelope["result"].clone()
}

fn agent_sql_snapshot(
    connection: &rusqlite::Connection,
    deadline: Instant,
) -> BTreeMap<String, Vec<Vec<rusqlite::types::Value>>> {
    let mut names=connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 129").unwrap();
    let names = names
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert!(names.len() <= 128);
    let mut result = BTreeMap::new();
    let mut bytes = 0usize;
    for name in names {
        assert!(
            Instant::now() < deadline
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
        let probe = connection
            .prepare(&format!("SELECT * FROM \"{name}\" LIMIT 0"))
            .unwrap();
        let columns = probe.column_count();
        assert!((1..=64).contains(&columns));
        drop(probe);
        let order = (1..=columns)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut statement = connection
            .prepare(&format!(
                "SELECT * FROM \"{name}\" ORDER BY {order} LIMIT 16385"
            ))
            .unwrap();
        let values = statement
            .query_map([], |r| {
                (0..columns)
                    .map(|column| r.get::<_, rusqlite::types::Value>(column))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert!(values.len() <= 16384);
        for row in &values {
            for value in row {
                let size = match value {
                    rusqlite::types::Value::Text(v) => v.len(),
                    rusqlite::types::Value::Blob(v) => v.len(),
                    _ => 16,
                };
                assert!(size <= 16_777_216);
                bytes = bytes.checked_add(size).unwrap();
                assert!(bytes <= 33_554_432);
            }
        }
        result.insert(name, values);
    }
    result
}

fn agent_physical(workspace: &Path, deadline: Instant) {
    let (bytes, _) = fixture_physical_bytes(&[workspace.to_owned()], deadline);
    // Reserve4MiB for the one sequential anonymous CLI input/stdout/stderr
    // framing, including allocated-block rounding; all named copies are here.
    assert!(bytes <= 508 * 1024 * 1024);
}

#[test]
fn maintained_agent_record_correction_whole_transaction_and_access() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_compiler::prepared_source_binding::read_prepared_source_inputs_transaction;
    use tos_source_store::{ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1};
    let deadline = Instant::now() + Duration::from_secs(600);
    let cancel = Arc::new(AtomicBool::new(false));
    let repository = super::validation_cut_cases::repository();
    let owner_command = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("exact native owner CLI required"),
    );
    let prepared_consumer = PathBuf::from(
        std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
            .expect("exact native prepared-access CLI required"),
    );
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let image_paths = [
        (
            std::env::current_exe().expect("actual protected conformance ELF path"),
            512 * 1024 * 1024,
        ),
        (owner_command.clone(), 512 * 1024 * 1024),
        (worker_path.clone(), 128 * 1024 * 1024),
        (prepared_consumer, 512 * 1024 * 1024),
    ];
    for (path, maximum) in &image_paths {
        assert!(path.is_absolute());
        assert!(fs::metadata(path).unwrap().len() <= *maximum as u64);
    }
    let images = image_paths
        .iter()
        .map(|(p, maximum)| native_child::bounded_sha_before(p, *maximum, deadline))
        .collect::<Vec<_>>();
    let workspace = PublicationFailureFixture::new("Agent correction");
    let packet_path = workspace.path().join("agent-fixture.json");
    let mut export = Command::new(
        std::env::var_os("TOS_MAINTAINED_PYTHON").expect("explicit maintained fixture interpreter"),
    );
    export
        .arg(repository.join("tests/conformance/rust/source_claim_publication_fixture.py"))
        .arg("agent")
        .arg(workspace.path())
        .arg(&packet_path)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    let output = native_child::bounded_output_before(
        &mut export,
        4096,
        deadline.min(Instant::now() + Duration::from_secs(60)),
    );
    assert!(
        output.status.success(),
        "Agent fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packet = read_packet(&packet_path);
    let root = PathBuf::from(required(&packet, "source_root"))
        .canonicalize()
        .unwrap();
    assert!(root.starts_with(workspace.path().canonicalize().unwrap()));
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let owner = PathBuf::from(required(&packet, "owner_config"));
    let config_raw = fs::read(&owner).unwrap();
    assert_eq!(
        fs::metadata(&owner).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let db_path = PathBuf::from(required(&packet, "db_path"));
    assert!(owner.starts_with(&root) && db_path.starts_with(&root));
    // The native DB fence requires its immediate parent to be fixture-private.
    let db_parent = db_path.parent().unwrap();
    assert_eq!(db_parent.canonicalize().unwrap(), db_parent);
    assert_eq!(
        fs::metadata(db_parent).unwrap().uid(),
        fs::metadata(&root).unwrap().uid()
    );
    fs::set_permissions(db_parent, fs::Permissions::from_mode(0o700)).unwrap();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.busy_timeout(Duration::from_secs(2)).unwrap();
    connection
        .execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF")
        .unwrap();
    let publication = PublicationLimits {
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
    let operation_limits = ClaimPublicationLimits::default();
    let initial_binding = typed(&packet["binding"]);
    let initial_catalog = agent_catalog(&packet, &packet["header"]);
    // Rebuild native auxiliary indexes from this fixture's exact prepared rows.
    // Keep the independent Python catalog, header, and semantic report unchanged.
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
    let original_files = agent_authored(&root, deadline);
    let original_store = workspace.path().join("original-cut");
    let original_revision =
        super::validation_cut_cases::write_cut_store(&original_files, &original_store);
    let _original =
        super::command_form_cases::open_cut(&original_store, original_revision, deadline, &cancel);
    let mut combined = original_files.clone();
    for name in AGENT_RECORD_COMPONENTS {
        let raw = fs::read(repository.join(name)).unwrap();
        assert!(raw.len() <= 2_097_152);
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &raw).unwrap();
        combined.insert((*name).to_owned(), raw);
    }
    assert!(combined.values().map(Vec::len).sum::<usize>() <= 33_554_432 && combined.len() <= 2048);
    let capture = workspace.path().join("software-capture");
    let restored = workspace.path().join("software-restored");
    let mut commit = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            commit.env_remove(name);
        }
    }
    commit
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    commit
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"]);
    let commit = native_child::bounded_output_before(&mut commit, 4096, deadline);
    assert!(commit.status.success());
    let commit = String::from_utf8(commit.stdout).unwrap().trim().to_owned();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    for restore in [false, true] {
        let mut command = Command::new(
            std::env::var_os("TOS_MAINTAINED_PYTHON")
                .expect("explicit maintained fixture interpreter"),
        );
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
        command
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        command.arg(repository.join("scripts/corpus_archive.py"));
        if restore {
            command
                .arg("restore")
                .arg("--capture")
                .arg(&capture)
                .arg("--output")
                .arg(&restored);
        } else {
            command
                .arg("capture")
                .arg("--repo-root")
                .arg(&repository)
                .arg("--commit")
                .arg(&commit)
                .arg("--output")
                .arg(&capture);
            for name in AGENT_RECORD_COMPONENTS {
                command.arg("--include-prefix").arg(name);
            }
        }
        command.env("PYTHONDONTWRITEBYTECODE", "1");
        let output = native_child::bounded_output_before(
            &mut command,
            4096,
            deadline.min(Instant::now() + Duration::from_secs(60)),
        );
        assert!(
            output.status.success(),
            "capture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let capture_raw = fs::read(capture.join("capture.json")).unwrap();
    assert!(capture_raw.len() <= 1_048_576);
    let capture_manifest: Value = serde_json::from_slice(&capture_raw).unwrap();
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree: required(&capture_manifest, "source_git_tree").to_owned(),
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
        &cancel,
    )
    .unwrap();
    let component_paths = AGENT_RECORD_COMPONENTS
        .iter()
        .map(|p| RelativePath::parse(p).unwrap())
        .collect::<Vec<_>>();
    let _components = software.select_components(&component_paths).unwrap();
    let invocation_path = workspace.path().join("agent-native-invocation.json");
    let invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1","owner_config":owner,"owner_context":null,"assessment_schema_worker":null,"native_executable":owner_command,"native_executable_sha256":images[1].to_prefixed(),"corpus_store":original_store,"source_revision":original_revision.0.to_prefixed(),"original_source_revision":original_revision.0.to_prefixed(),"software_capture":capture,"software_restored_root":restored,"software_selection":{"source_git_commit":selection.source_git_commit,"source_git_tree":selection.source_git_tree,"capture_manifest_sha256":selection.capture_manifest_sha256.to_prefixed()},"software_components":AGENT_RECORD_COMPONENTS,"schema_worker":{"absolute_path":worker_path,"sha256":images[2].to_prefixed()},"budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
    fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
    fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
    let agent_invocation_path = root.join("agent-publication-invocation.json");
    let binding_companion = root.join("agent-selected-binding.json");
    let source_companion = root.join("agent-selected-source.json");
    let catalog_companion = root.join("agent-selected-catalog.json");
    let descriptor_companion = root.join("agent-selected-descriptor.json");
    let write_protected = |path: &Path, value: &Value| {
        fs::write(path, canonical_lf(value)).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    };
    let selected_catalog_value = |header: &Value| serde_json::json!({"header":header,"entity_registry":packet["entities"],"relation_registry":packet["relations"],"lenses":packet["lenses"],"source_order_profile":"source-graph-id-v1"});
    write_protected(&binding_companion, &packet["binding"]);
    write_protected(&source_companion, &packet["source_inputs"]);
    write_protected(
        &catalog_companion,
        &selected_catalog_value(&packet["header"]),
    );
    write_protected(&descriptor_companion, &packet["descriptor"]);
    let file_sha = |path: &Path| Digest256::of_bytes(&fs::read(path).unwrap()).to_prefixed();
    let mut agent_invocation = invocation.clone();
    agent_invocation["schema_version"] = json!("tos_local_native_agent_publication_invocation_v1");
    agent_invocation["prepared_database"] = json!(db_path);
    agent_invocation["expected_binding_path"] = json!(binding_companion);
    agent_invocation["source_inputs_path"] = json!(source_companion);
    agent_invocation["source_inputs_sha256"] = json!(file_sha(&source_companion));
    agent_invocation["catalog_path"] = json!(catalog_companion);
    agent_invocation["catalog_sha256"] = json!(file_sha(&catalog_companion));
    agent_invocation["descriptor_path"] = json!(descriptor_companion);
    agent_invocation["descriptor_sha256"] = json!(file_sha(&descriptor_companion));
    agent_invocation["reviewed_execution_transition"] = Value::Null;
    agent_invocation["publication_limits"] = json!({
        "operation":{"max_nodes":operation_limits.max_nodes,"max_relations":operation_limits.max_relations,"max_claims":operation_limits.max_claims,"max_bytes":operation_limits.max_bytes,"max_row_bytes":operation_limits.max_row_bytes,"max_contexts":operation_limits.max_contexts,"max_vm_steps":operation_limits.max_vm_steps,"cow_target_bytes":operation_limits.cow_target_bytes},
        "publication":publication,"catalog":catalog_limits,"semantic":semantic_limits,
        "bibliographic":{"max_claim_cohort_rows":256,"max_claim_cohort_bytes":16777216,"max_output_rows":16384,"max_output_bytes":16777216}
    });
    write_protected(&agent_invocation_path, &agent_invocation);
    let execution = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &json!({"action":"describe-agent-execution"}),
        deadline,
    );
    let mut after_normalization = packet["header"]["normalization_binding"].clone();
    after_normalization["processor_digest"] = execution["processor"].clone();
    let deps = &packet["source_inputs"]["dependencies"];
    agent_invocation["reviewed_execution_transition"] = json!({
        "dependency_implementation_before":packet["dependency_implementation_before"],"declaration_before":deps["declaration-profile"],"agent_publication_before":deps["agent-publication-profile"],"claim_publication_before":deps.get("claim-publication-profile").cloned().unwrap_or(Value::Null),
        "normalization_before":packet["header"]["normalization_binding"],"normalization_after":after_normalization,"native_normalization_processor_sha256":execution["processor"],"review_ref":"test:real-Agent-whole-predecessor-profile-review","reviewed_after_agent_sha256":execution["agent_publication"]
    });
    write_protected(&agent_invocation_path, &agent_invocation);
    let profile_result = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &json!({"action":"reviewed-agent-execution-bootstrap"}),
        deadline,
    );
    let binding = typed(&profile_result["binding"]);
    let catalog = agent_catalog(&packet, &profile_result["source_header"]);
    let source = {
        let tx = connection.unchecked_transaction().unwrap();
        let source =
            read_prepared_source_inputs_transaction(&tx, &binding, &catalog, publication).unwrap();
        tx.rollback().unwrap();
        source
    };
    let predecessor_binding = workspace.path().join("Agent-predecessor-binding.json");
    fs::write(
        &predecessor_binding,
        canonical_lf(&profile_result["binding"]),
    )
    .unwrap();
    write_protected(&binding_companion, &profile_result["binding"]);
    write_protected(
        &source_companion,
        &serde_json::from_slice::<Value>(source.raw()).unwrap(),
    );
    write_protected(
        &catalog_companion,
        &selected_catalog_value(&profile_result["source_header"]),
    );
    agent_invocation["source_inputs_sha256"] = json!(file_sha(&source_companion));
    agent_invocation["catalog_sha256"] = json!(file_sha(&catalog_companion));
    agent_invocation["reviewed_execution_transition"] = Value::Null;
    write_protected(&agent_invocation_path, &agent_invocation);
    // Agent's selected Record owner returns a native transport envelope around
    // the source_revisions selected-family response. Other callers keep the
    // outer envelope, so unwrap only these two Agent observations.
    let checked_agent_record_result = |mut envelope: Value| {
        assert!(envelope.is_object(), "Agent native source envelope object");
        assert_eq!(envelope.as_object().unwrap().len(), 4);
        assert_eq!(
            envelope["schema_version"],
            "tos_local_native_source_result_v1"
        );
        assert_eq!(envelope["authentication"], "local-unix-account");
        assert_eq!(envelope["grants_admission"], false);
        let result = envelope.as_object_mut().unwrap().remove("result").unwrap();
        assert!(result.is_object(), "Agent selected Record result object");
        assert_eq!(
            result["schema_version"],
            "tos_local_source_revision_result_v2"
        );
        assert_eq!(result["authentication"], "local-unix-account");
        assert_eq!(result["grants_admission"], false);
        result
    };
    let preview = checked_agent_record_result(agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &packet["proposal"],
        deadline,
    ));
    for field in [
        "owner_configuration",
        "source",
        "revision",
        "expected_dependencies",
    ] {
        assert_eq!(preview[field], packet["python_preview"][field]);
    }
    let mut request = packet["proposal"].clone();
    request["operation"] = Value::String("record.revise".into());
    request["command_id"] = Value::String("synthetic:actual-native-Agent-correction".into());
    for (field, prepared) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
        ("expected_dependencies", "expected_dependencies"),
        ("expected_publication", "expected_publication"),
    ] {
        request[field] = preview[prepared].clone();
    }
    agent_physical(workspace.path(), deadline);
    let source_result = checked_agent_record_result(agent_native_call(
        &repository,
        &owner,
        &invocation_path,
        &request,
        deadline,
    ));
    assert_eq!(source_result["replayed"], false);
    let source_receipt = workspace.path().join("native-Record-receipt.json");
    fs::write(&source_receipt, canonical_lf(&source_result)).unwrap();
    let current_files = agent_authored(&root, deadline);
    let current_store = original_store.clone();
    let current_revision = super::validation_cut_cases::write_cut_store_on_base(
        &current_files,
        &current_store,
        Some(original_revision),
    );
    let _current =
        super::command_form_cases::open_cut(&current_store, current_revision, deadline, &cancel);
    agent_invocation["source_revision"] = json!(current_revision.0.to_prefixed());
    write_protected(&agent_invocation_path, &agent_invocation);
    let publication_request = json!({"action":"publish-agent-correction","recorded_at":source_result["receipt"]["recorded_at"],"record_request":request});
    let result = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &publication_request,
        deadline,
    );
    assert_eq!(result["prepared_committed"], true);
    // Actual protected production CLI stale predecessor refusal preserves all SQL/source bytes.
    let prepared_before = agent_sql_snapshot(&connection, deadline);
    let refusal = agent_publication_cli_output(
        &owner_command,
        &agent_invocation_path,
        &publication_request,
        deadline,
    );
    assert!(!refusal.status.success());
    assert_eq!(prepared_before, agent_sql_snapshot(&connection, deadline));
    assert_eq!(agent_authored(&root, deadline), current_files);
    // Discard publication stdout as an authority: recover only from the persisted
    // paired state under the authenticated source observation and read-only DB.
    let recovered = agent_publication_cli(
        &owner_command,
        &agent_invocation_path,
        &json!({"action":"inspect-agent-publication","recorded_at":source_result["receipt"]["recorded_at"],"record_request":request}),
        deadline,
    );
    assert_eq!(recovered["binding"], result["binding"]);
    assert_eq!(
        recovered["transaction"]["transaction_id"],
        result["transaction_id"]
    );
    assert_eq!(prepared_before, agent_sql_snapshot(&connection, deadline));
    assert_eq!(agent_authored(&root, deadline), current_files);
    let oracle_path = workspace.path().join("Agent-independent-oracle.json");
    let mut oracle = Command::new(
        std::env::var_os("TOS_MAINTAINED_PYTHON").expect("explicit maintained fixture interpreter"),
    );
    oracle
        .arg(repository.join("tests/conformance/rust/source_claim_publication_fixture.py"))
        .arg("agent-oracle")
        .arg(&packet_path)
        .arg(&source_receipt)
        .arg(&oracle_path)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    let output = native_child::bounded_output_before(
        &mut oracle,
        4096,
        deadline.min(Instant::now() + Duration::from_secs(60)),
    );
    assert!(
        output.status.success(),
        "Agent oracle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = read_packet(&oracle_path);
    assert_eq!(agent_authored(&root, deadline), current_files);
    for kind in ["node", "relation"] {
        let actual: BTreeMap<String, Value> = connection
            .prepare(&format!(
                "SELECT id,json FROM knowledge_{kind}s ORDER BY id LIMIT 16385"
            ))
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|r| {
                let (id, raw) = r.unwrap();
                (id, serde_json::from_str(&raw).unwrap())
            })
            .collect();
        let expected: BTreeMap<String, Value> = expected[format!("{kind}s")]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (required(r, "id").to_owned(), r.clone()))
            .collect();
        assert!(actual.len() <= 16384);
        assert_eq!(actual, expected);
    }
    assert_eq!(
        result["semantic_report"],
        expected["counts"]["semantic_validation"]
    );
    let binding_path = workspace.path().join("Agent-binding.json");
    fs::write(&binding_path, canonical_lf(&result["binding"])).unwrap();
    drop(connection);
    if std::env::var_os("TOS_NATIVE_INSTALLED_SOFTWARE_SITE").as_deref()
        == Some(std::ffi::OsStr::new("1"))
    {
        let installed_access = &image_paths[3].0;
        assert_eq!(
            native_child::bounded_sha_before(installed_access, 512 * 1024 * 1024, deadline),
            images[3]
        );
        let stale = native_child::bounded_output_before(
            Command::new(installed_access)
                .arg("--prepared-read-model")
                .arg(&db_path)
                .arg("--prepared-binding")
                .arg(&predecessor_binding)
                .args(["knowledge", "catalog"]),
            tos_access::prepared_local::PREPARED_RESPONSE_BYTES,
            deadline.min(Instant::now() + Duration::from_secs(5)),
        );
        assert!(
            !stale.status.success(),
            "installed access accepted stale Agent predecessor"
        );
        assert!(
            stale.stdout.is_empty(),
            "stale installed query disclosed rows"
        );
        assert!(
            String::from_utf8_lossy(&stale.stderr).contains("stale_selection"),
            "unexpected installed stale refusal: {}",
            String::from_utf8_lossy(&stale.stderr)
        );
        assert_eq!(
            native_child::bounded_sha_before(installed_access, 512 * 1024 * 1024, deadline),
            images[3]
        );
    }
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
    let agent_id = required(&packet, "record_id");
    let node = expected["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["properties"]["source_record"]["record_id"] == agent_id
                && r["properties"]["source_record"]["record_version"] == 2
        })
        .expect("independent current Agent node");
    let relation = expected["relations"]
        .as_array()
        .unwrap()
        .first()
        .expect("maintained Agent related claim relation");
    claim_publication_access::verify_published_access_until(
        &db_path,
        &binding_path,
        required(node, "id"),
        required(relation, "id"),
        deadline,
    );
    agent_physical(workspace.path(), deadline);
    for ((path, maximum), expected) in image_paths.iter().zip(images) {
        assert_eq!(
            native_child::bounded_sha_before(path, *maximum, deadline),
            expected
        );
    }
    assert!(Instant::now() < deadline);
}
