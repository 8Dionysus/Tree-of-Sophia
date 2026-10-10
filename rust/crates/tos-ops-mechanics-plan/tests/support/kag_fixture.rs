// Shared accepted-corpus fixture for native export and CLI publication tests.
static FIXTURES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) struct CorpusFixture(pub PathBuf);
impl Drop for CorpusFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub(crate) fn fixture() -> (CorpusFixture, PathBuf, String) {
    let base = std::env::temp_dir().join(format!(
        "tos-kag-corpus-test-{}-{}",
        std::process::id(),
        FIXTURES.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&base).unwrap();
    let store = base.join("store");
    fs::create_dir_all(store.join("revisions")).unwrap();
    fs::create_dir_all(store.join("objects")).unwrap();
    let mut files = Vec::new();
    for path in SOURCE_PATHS {
        let raw = match path {
                PRIMARY => canonical(&json!({"node_id":"corpus-only-node"})).unwrap(),
                "ToS/public-compatibility/source_node.example.json" => canonical(&json!({"node_id":"corpus-only-node","interpretation_layers":["corpus-only-layer"],"relations":[{"relation_type":"bounded_hop","target_ref":"corpus-only-concept"}]})).unwrap(),
                "ToS/public-compatibility/concept_node.example.json" => canonical(&json!({"node_id":"corpus-only-concept"})).unwrap(),
                _ => b"# exact corpus bytes\n".to_vec(),
            };
        let sha = digest(&raw);
        let object = store.join("objects").join(&sha);
        if object.exists() {
            assert_eq!(fs::read(&object).unwrap(), raw);
        } else {
            write_new(&object, &raw).unwrap();
        }
        files.push(json!({"path":path,"sha256":sha,"size_bytes":raw.len(),"mode":420}));
    }
    files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let mut snapshot = json!({"schema_version":"tos_corpus_snapshot_v1","base_revision":null,"validator_sha256":"1".repeat(64),"files":files,"identities":{"corpus-only-node":PRIMARY},"dependencies":{},"retirements":[]});
    let revision = digest(&canonical(&snapshot).unwrap());
    snapshot["revision"] = json!(revision);
    write_new(
        &store
            .join("revisions")
            .join(&revision)
            .join("snapshot.json"),
        &canonical(&snapshot).unwrap(),
    )
    .unwrap();
    (CorpusFixture(base), store, revision)
}
pub(crate) fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_owned()
}
