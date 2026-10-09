//! Native CLI contract for offline prepared bootstrap and optional maintenance.
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: &str = "8388608";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tos-native-prepare-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("fresh native prepare test scratch");
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_json(root: &Path, relative: &str, value: &Value) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
    fs::write(path, serde_json::to_vec(value).expect("fixture JSON")).expect("fixture file");
}

fn write_source(root: &Path) {
    let source_ref = root
        .join("ToS/synthetic.json")
        .to_string_lossy()
        .into_owned();
    write_json(
        root,
        "ToS/derived-exports/tos_corpus_index.min.json",
        &json!({
            "schema_version":"tos_corpus_index_v1", "nodes":[], "resources":[], "manifests":[],
            "branches":[], "relation_edges":[], "relation_packs":[], "graph_views":[],
            "source_navigation":{"nodes":[],"edges":[],"rights":[]}
        }),
    );
    write_json(
        root,
        "ToS/derived-exports/philosophy_graph_projection.min.json",
        &json!({
            "schema_version":"tos_philosophy_graph_projection_v2",
            "nodes":[
                {"node_id":"a","node_type":"concept","label":"Альфа Alpha","source_ref":source_ref,
                 "properties":{"future":{"zero":0,"false":false,"null":null}}},
                {"node_id":"b","node_type":"concept","label":"Beta","source_ref":source_ref,"properties":{}}
            ],
            "edges":[{"edge_id":"r","from_id":"a","to_id":"b","predicate_id":"related_to","source_ref":"ToS/synthetic.json"}],
            "layers":[],"views":[],"clusters":[]
        }),
    );
    write_json(
        root,
        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
        &json!({
            "schema_version":"tos_source_witness_bibliographic_graph_v1", "nodes":[], "edges":[], "claim_traces":[]
        }),
    );
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for relative in [
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ] {
        let source = repository.join(relative);
        let destination = root.join(relative);
        fs::create_dir_all(destination.parent().expect("registry parent"))
            .expect("registry directory");
        fs::copy(source, destination).expect("current versioned registry");
    }
}

fn prepare(root: &Path, output: &Path, extra: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tos-access"));
    command
        .arg("prepare")
        .arg("--source-root")
        .arg(root)
        .arg("--output-dir")
        .arg(output)
        .args(["--max-seconds", "120", "--max-bytes", MAX_BYTES])
        .args(extra);
    command.output().expect("native prepare process")
}

fn completed(output: Output) -> Value {
    assert!(
        output.status.success(),
        "native prepare failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("native prepare receipt JSON")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read emitted JSON")).expect("emitted JSON")
}

fn has_table(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
            [name],
            |row| row.get(0),
        )
        .expect("prepared table lookup")
}

fn assert_artifact(output: &Path, receipt: &Value, maintenance: bool, search: &str) {
    assert_eq!(
        receipt["schema"],
        "tos_offline_prepared_bootstrap_receipt_v1"
    );
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["mode"], "full_bootstrap");
    assert_eq!(receipt["source_state_checked"], true);
    assert_eq!(receipt["ongoing_currentness_granted"], false);
    assert_eq!(receipt["normalization_cache"], "disabled");
    assert_eq!(receipt["consumer_switched"], false);
    assert_eq!(receipt["search_bootstrap"], search);
    assert_eq!(read_json(&output.join("completed.json")), *receipt);
    assert_eq!(read_json(&output.join("binding.json")), receipt["binding"]);
    let files = fs::read_dir(output)
        .expect("prepared output directory")
        .map(|entry| {
            entry
                .expect("output entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        files,
        ["binding.json", "completed.json", "snapshot.sqlite"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    let database = Connection::open(output.join("snapshot.sqlite")).expect("prepared SQLite file");
    let integrity: String = database
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .expect("prepared SQLite integrity");
    assert_eq!(integrity, "ok");
    assert!(has_table(&database, "catalog_state"));
    assert!(has_table(&database, "search_header"));
    assert_eq!(has_table(&database, "semantic_state"), maintenance);
    if maintenance {
        let attachment = &receipt["maintenance"];
        assert_eq!(attachment["status"], "attached");
        assert_eq!(attachment["mode"], "catalog_semantic_indexes");
        assert_eq!(attachment["binding"], receipt["binding"]);
        assert_eq!(attachment["publication_changed"], false);
        assert_eq!(attachment["consumer_switched"], false);
        assert_eq!(attachment["source_transition_verified"], false);
        assert_eq!(attachment["semantic_acceptance"], false);
        assert!(attachment["catalog_digest"].as_str().is_some());
        assert!(attachment["semantic_report_sha256"].as_str().is_some());
        assert!(attachment["sql_mutations"].as_u64().is_some());
    } else {
        assert!(receipt.get("maintenance").is_none());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(output).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for name in ["binding.json", "completed.json", "snapshot.sqlite"] {
            assert_eq!(
                fs::metadata(output.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}

#[test]
fn native_prepare_cli_receipt_artifact_and_maintenance_modes() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source");
    write_source(&source);

    let buffered = scratch.0.join("buffered");
    let base_receipt = completed(prepare(&source, &buffered, &[]));
    assert_artifact(&buffered, &base_receipt, false, "buffered");

    let buffered_attached = scratch.0.join("buffered-attached");
    let attached_receipt = completed(prepare(
        &source,
        &buffered_attached,
        &["--attach-maintenance"],
    ));
    assert_artifact(&buffered_attached, &attached_receipt, true, "buffered");

    let bulk_attached = scratch.0.join("bulk-attached");
    let bulk_receipt = completed(prepare(
        &source,
        &bulk_attached,
        &[
            "--attach-maintenance",
            "--bulk-search-scratch-bytes",
            "8388608",
            "--bulk-search-scratch-mutations",
            "2000000",
        ],
    ));
    assert_artifact(&bulk_attached, &bulk_receipt, true, "bulk");

    assert_eq!(
        base_receipt["source_revision"],
        attached_receipt["source_revision"]
    );
    assert_eq!(
        attached_receipt["source_revision"],
        bulk_receipt["source_revision"]
    );
    assert_eq!(base_receipt["binding"], attached_receipt["binding"]);
    assert_eq!(attached_receipt["binding"], bulk_receipt["binding"]);
    assert_eq!(
        attached_receipt["maintenance"]["catalog_digest"],
        bulk_receipt["maintenance"]["catalog_digest"]
    );
    assert_eq!(
        attached_receipt["maintenance"]["semantic_report_sha256"],
        bulk_receipt["maintenance"]["semantic_report_sha256"]
    );
}

#[test]
fn native_prepare_cli_caps_and_existing_output_fail_closed() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source");
    write_source(&source);

    let invalid = scratch.0.join("invalid-cap");
    let refused = prepare(&source, &invalid, &["--max-bytes", "0"]);
    assert!(!refused.status.success());
    assert!(
        !invalid.exists(),
        "invalid cap must refuse before output creation"
    );

    let output = scratch.0.join("completed");
    let receipt = completed(prepare(&source, &output, &[]));
    let original_marker = fs::read(output.join("completed.json")).expect("completion marker");
    assert_eq!(read_json(&output.join("completed.json")), receipt);

    let collision = prepare(&source, &output, &[]);
    assert!(!collision.status.success());
    assert_eq!(
        fs::read(output.join("completed.json")).expect("preserved completion marker"),
        original_marker
    );
}
