//! Independent, small conformance vectors for the Rust migration foundation.
//! OPS registers this as an integration test with serde_json and tempfile.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::TempDir;
use tos_foundation::{
    CanonicalProfile, CodePointSpan, Digest256, JsonLimits, JsonMode, JsonNumberKind, JsonValue,
    RelativePath, SourceRevision, canonical_bytes_v1, emit_preserved_json, parse_json,
};
use tos_source_store::{CorpusReader, ReadLimits, Selector, StoreErrorCode};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lines(name: &str) -> Vec<Value> {
    fs::read_to_string(fixtures().join(name))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn required<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing {key}: {value}"))
}

fn decode_hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn source_bytes(case: &Value) -> Vec<u8> {
    match case.get("input_hex") {
        Some(hex) => decode_hex(hex.as_str().unwrap()),
        None => required(case, "input_utf8").as_bytes().to_vec(),
    }
}

#[test]
fn independent_foundation_vectors() {
    let cases = lines("foundation.jsonl");
    assert_eq!(cases.len(), 21, "a dropped vector is a contract change");
    for case in cases {
        let id = required(&case, "case_id");
        let expected = &case["expected"];
        match required(&case, "operation") {
            "relative_path" => {
                let actual = RelativePath::parse(required(&case, "input_utf8"));
                if let Some(code) = expected.get("reject") {
                    assert_eq!(
                        actual.unwrap_err().code.as_str(),
                        code.as_str().unwrap(),
                        "{id}"
                    );
                } else {
                    assert_eq!(
                        actual.unwrap().as_str(),
                        required(expected, "accept"),
                        "{id}"
                    );
                }
            }
            "span" => {
                let text = required(&case, "input_utf8");
                assert_eq!(
                    text.chars().count() as u64,
                    expected["code_points"].as_u64().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    text.len() as u64,
                    expected["utf8_bytes"].as_u64().unwrap(),
                    "{id}"
                );
                let range = expected["code_point_range"].as_array().unwrap();
                let span = CodePointSpan::new(
                    range[0].as_u64().unwrap(),
                    range[1].as_u64().unwrap(),
                    Digest256::of_bytes(text.as_bytes()),
                    "source-exact-v1",
                )
                .unwrap();
                let bytes = span.byte_span_in(text).unwrap();
                let result = expected["corresponding_byte_range"].as_array().unwrap();
                assert_eq!(
                    [bytes.start, bytes.end],
                    [result[0].as_u64().unwrap(), result[1].as_u64().unwrap()],
                    "{id}"
                );
                assert!(
                    span.byte_span_in("AéO").is_err(),
                    "{id}: representation digest must bind offsets"
                );
            }
            operation @ ("parse" | "parse_preserve" | "canonical") => {
                let mode = match case.get("mode").and_then(Value::as_str) {
                    Some("RequestLastWins") => JsonMode::RequestLastWins,
                    _ => JsonMode::PublishedStrict,
                };
                let parsed = parse_json(&source_bytes(&case), mode, JsonLimits::default());
                if operation != "canonical" && expected.get("reject").is_some() {
                    assert_eq!(
                        parsed.unwrap_err().code.as_str(),
                        required(expected, "reject"),
                        "{id}"
                    );
                    continue;
                }
                let parsed =
                    parsed.unwrap_or_else(|error| panic!("{id}: unexpected parse error {error}"));
                if operation == "canonical" {
                    assert_eq!(required(&case, "profile"), "CorpusSnapshotV1", "{id}");
                    let canonical = canonical_bytes_v1(
                        parsed.root(),
                        CanonicalProfile::CorpusSnapshotV1,
                        JsonLimits::default(),
                    );
                    if expected.get("reject").is_some() {
                        assert_eq!(
                            canonical.unwrap_err().code.as_str(),
                            required(expected, "reject"),
                            "{id}"
                        );
                    } else {
                        let bytes = canonical.unwrap();
                        assert_eq!(
                            bytes.as_slice(),
                            required(expected, "canonical_utf8").as_bytes(),
                            "{id}"
                        );
                        assert_eq!(
                            Digest256::of_bytes(&bytes).to_hex(),
                            required(expected, "digest_hex"),
                            "{id}"
                        );
                    }
                    continue;
                }
                if let Some(order) = expected.get("member_order") {
                    let observed: Vec<&str> = parsed
                        .root()
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(key, _)| key.as_str().unwrap())
                        .collect();
                    let wanted: Vec<&str> = order
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|key| key.as_str().unwrap())
                        .collect();
                    assert_eq!(observed, wanted, "{id}");
                }
                if let Some(numbers) = expected.get("numbers").and_then(Value::as_object) {
                    for (key, wanted) in numbers {
                        let JsonValue::Number(number) = parsed.root().object_get(key).unwrap()
                        else {
                            panic!("{id}: {key} lost numeric context");
                        };
                        let kind = match number.kind {
                            JsonNumberKind::Int => "Int",
                            JsonNumberKind::Float => "Float",
                        };
                        assert_eq!(kind, required(wanted, "kind"), "{id}/{key}");
                        assert_eq!(number.lexeme, required(wanted, "lexeme"), "{id}/{key}");
                    }
                }
                if let Some(wanted) = expected.get("preserved_utf8") {
                    let actual = emit_preserved_json(&parsed, JsonLimits::default()).unwrap();
                    assert_eq!(
                        actual.as_slice(),
                        wanted.as_str().unwrap().as_bytes(),
                        "{id}"
                    );
                }
            }
            other => panic!("{id}: unhandled operation {other}"),
        }
    }
}

fn read_limits() -> ReadLimits {
    ReadLimits {
        max_manifest_bytes: 8192,
        max_manifest_entries: 32,
        max_selected_object_bytes: 1024,
        json: JsonLimits::default(),
    }
}

fn revision(hex: &str) -> SourceRevision {
    SourceRevision(Digest256::from_hex(hex).unwrap())
}

fn fixture_store() -> PathBuf {
    fixtures().join("corpus-v1/store")
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let destination = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&source, &destination)?;
        } else {
            fs::copy(source, destination)?;
        }
    }
    Ok(())
}

fn working_store() -> (TempDir, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    copy_tree(&fixture_store(), &root).unwrap();
    (temporary, root)
}

fn fixture_manifest() -> Value {
    serde_json::from_slice(&fs::read(fixtures().join("corpus-v1/fixture.json")).unwrap()).unwrap()
}

fn selected_object(root: &Path, sha256: &str) -> PathBuf {
    root.join("objects").join(sha256)
}

fn canonical_json(value: &Value) -> Vec<u8> {
    // This setup encodes only the ASCII v1 manifest; its expected digest and
    // original bytes come from the checked-in independent fixture.
    let mut raw = serde_json::to_vec(value).unwrap();
    raw.push(b'\n');
    raw
}

#[test]
fn independent_corpus_v1_positive_and_missing() {
    let fixture = fixture_manifest();
    let reader = CorpusReader::open_existing(&fixture_store(), read_limits()).unwrap();
    assert_eq!(
        reader.select_current().unwrap().unwrap().0.to_hex(),
        required(&fixture, "revision_current")
    );
    for case in fixture["selected_cases"].as_array().unwrap() {
        let id = required(case, "case_id");
        let snapshot = reader
            .load_exact(revision(required(case, "revision")))
            .unwrap();
        let result = if let Some(source_id) = case.get("source_id").and_then(Value::as_str) {
            reader.resolve(&snapshot, Selector::SourceId(source_id))
        } else {
            let path = RelativePath::parse(required(case, "path")).unwrap();
            reader.resolve(&snapshot, Selector::Path(&path))
        };
        if case.get("expected_error").is_some() {
            assert_eq!(
                format!("{:?}", result.unwrap_err().code),
                required(case, "expected_error"),
                "{id}"
            );
            continue;
        }
        let actual = result.unwrap();
        let expected = &case["expected_descriptor"];
        assert_eq!(
            actual.revision.0.to_hex(),
            required(expected, "revision"),
            "{id}"
        );
        assert_eq!(actual.path.as_str(), required(expected, "path"), "{id}");
        assert_eq!(actual.sha256.to_hex(), required(expected, "sha256"), "{id}");
        assert_eq!(
            actual.size_bytes,
            expected["size_bytes"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(
            actual.mode as u64,
            expected["mode"].as_u64().unwrap(),
            "{id}"
        );
        let mut selected = Vec::new();
        let count = reader
            .read_selected(&snapshot, &actual, 1024, &mut selected)
            .unwrap();
        assert_eq!(count as usize, selected.len(), "{id}");
        assert_eq!(
            selected,
            decode_hex(required(case, "expected_bytes_hex")),
            "{id}"
        );
    }
}

#[test]
fn independent_corpus_v1_negative_boundaries() {
    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");
    let old_sha = required(
        &fixture["selected_cases"][0]["expected_descriptor"],
        "sha256",
    );
    for case in fixture["negative_cases"].as_array().unwrap() {
        let id = required(case, "case_id");
        if matches!(id, "traversal" | "git-component") {
            assert_eq!(
                RelativePath::parse(required(case, "selector_path"))
                    .unwrap_err()
                    .code
                    .as_str(),
                "unsafe_path",
                "{id}"
            );
            continue;
        }
        let (_temporary, root) = working_store();
        let old_object = selected_object(&root, old_sha);
        let old_snapshot = root.join("revisions").join(old).join("snapshot.json");
        match id {
            "wrong-selected-digest" => fs::write(&old_object, b"other").unwrap(),
            "wrong-selected-size" => fs::write(&old_object, b"tiny").unwrap(),
            "unrelated-corrupt-object" => {
                let current = required(&fixture, "revision_current");
                let manifest: Value = serde_json::from_slice(
                    &fs::read(root.join("revisions").join(current).join("snapshot.json")).unwrap(),
                )
                .unwrap();
                let beta_sha = manifest["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|entry| entry["path"] == "records/beta.txt")
                    .unwrap()["sha256"]
                    .as_str()
                    .unwrap();
                fs::write(
                    selected_object(&root, beta_sha),
                    b"damaged unrelated object",
                )
                .unwrap();
            }
            "symlink-selected-object" => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::symlink;
                    let outside = root.parent().unwrap().join("outside");
                    fs::write(&outside, b"other").unwrap();
                    fs::remove_file(&old_object).unwrap();
                    symlink(outside, &old_object).unwrap();
                }
                #[cfg(not(unix))]
                {
                    continue;
                }
            }
            "corrupt-ancestor-manifest" => {
                let mut raw = fs::read(&old_snapshot).unwrap();
                assert_eq!(raw[0], b'{');
                raw.insert(1, b' ');
                fs::write(&old_snapshot, raw).unwrap();
            }
            "duplicate-identity-json-field" => {
                let original = fs::read_to_string(&old_snapshot).unwrap();
                let one = "\"identities\":{\"tos.work.synthetic.alpha\":\"records/alpha.txt\"}";
                let two = "\"identities\":{\"tos.work.synthetic.alpha\":\"records/alpha.txt\",\"tos.work.synthetic.alpha\":\"records/alpha.txt\"}";
                assert!(original.contains(one));
                fs::write(&old_snapshot, original.replacen(one, two, 1)).unwrap();
            }
            "canonical-manifest-revision-mismatch" => {
                let mut manifest: Value =
                    serde_json::from_slice(&fs::read(&old_snapshot).unwrap()).unwrap();
                manifest["validator_sha256"] = Value::String("b".repeat(64));
                fs::write(&old_snapshot, canonical_json(&manifest)).unwrap();
            }
            "duplicate-member-path" => {
                let mut manifest: Value =
                    serde_json::from_slice(&fs::read(&old_snapshot).unwrap()).unwrap();
                let duplicated = manifest["files"][0].clone();
                manifest["files"].as_array_mut().unwrap().push(duplicated);
                let mut body = manifest.clone();
                body.as_object_mut().unwrap().remove("revision");
                let new_revision = Digest256::of_bytes(&canonical_json(&body)).to_hex();
                manifest["revision"] = Value::String(new_revision.clone());
                let new_home = root.join("revisions").join(&new_revision);
                fs::create_dir(&new_home).unwrap();
                fs::write(new_home.join("snapshot.json"), canonical_json(&manifest)).unwrap();
                let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
                assert_eq!(
                    reader.load_exact(revision(&new_revision)).unwrap_err().code,
                    StoreErrorCode::InvalidMemberIndex,
                    "{id}"
                );
                continue;
            }
            "missing-old-revision"
            | "bounded-read"
            | "declared-selected-size-over-cap"
            | "actual-object-growth-after-resolve"
            | "manifest-byte-cap"
            | "aggregate-index-entry-cap" => {}
            other => panic!("unhandled corpus negative {other}"),
        }
        let mut limits = read_limits();
        if id == "declared-selected-size-over-cap" {
            limits.max_selected_object_bytes = 4;
        }
        if id == "manifest-byte-cap" {
            limits.max_manifest_bytes = 488;
        }
        if id == "aggregate-index-entry-cap" {
            limits.max_manifest_entries = 2;
        }
        let reader = CorpusReader::open_existing(&root, limits).unwrap();
        let result_code = if id == "missing-old-revision" {
            reader
                .load_exact(revision(required(case, "selected_revision")))
                .unwrap_err()
                .code
        } else if id == "manifest-byte-cap"
            || id == "aggregate-index-entry-cap"
            || id == "corrupt-ancestor-manifest"
            || id == "duplicate-identity-json-field"
            || id == "canonical-manifest-revision-mismatch"
        {
            reader.load_exact(revision(old)).unwrap_err().code
        } else {
            let snapshot = reader.load_exact(revision(old)).unwrap();
            if id == "actual-object-growth-after-resolve" {
                let descriptor = reader
                    .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
                    .unwrap();
                fs::write(&old_object, b"larger").unwrap();
                let mut stage = Vec::new();
                let error = reader
                    .read_selected(&snapshot, &descriptor, 5, &mut stage)
                    .unwrap_err();
                assert!(
                    stage.is_empty(),
                    "{id}: preflight refusal must not expose bytes"
                );
                error.code
            } else if id == "bounded-read" {
                let descriptor = reader
                    .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
                    .unwrap();
                let mut stage = Vec::new();
                let error = reader
                    .read_selected(&snapshot, &descriptor, 4, &mut stage)
                    .unwrap_err();
                assert!(
                    stage.is_empty(),
                    "{id}: declared size refusal must not expose bytes"
                );
                error.code
            } else {
                let result =
                    reader.resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"));
                if id == "unrelated-corrupt-object" {
                    assert!(result.is_ok(), "{id}: unrelated object must not be hashed");
                    continue;
                }
                result.unwrap_err().code
            }
        };
        assert_eq!(
            format!("{result_code:?}"),
            required(case, "expected_error"),
            "{id}"
        );
    }
}

#[cfg(target_os = "linux")]
fn old_alpha_bytes(reader: &CorpusReader, old: &str) -> Vec<u8> {
    let snapshot = reader.load_exact(revision(old)).unwrap();
    let descriptor = reader
        .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
        .unwrap();
    let mut bytes = Vec::new();
    reader
        .read_selected(&snapshot, &descriptor, 1024, &mut bytes)
        .unwrap();
    bytes
}

#[cfg(target_os = "linux")]
#[test]
fn corpus_reader_keeps_opened_root_and_object_directory() {
    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");
    let old_sha = required(
        &fixture["selected_cases"][0]["expected_descriptor"],
        "sha256",
    );
    let expected = decode_hex(required(
        &fixture["selected_cases"][0],
        "expected_bytes_hex",
    ));

    // Rename the opened root and put a valid but corrupt decoy at its former
    // pathname. A reader rooted in path strings would follow the decoy.
    let (_temporary, root) = working_store();
    let decoy = root.parent().unwrap().join("decoy-store");
    copy_tree(&root, &decoy).unwrap();
    fs::write(selected_object(&decoy, old_sha), b"other").unwrap();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    fs::rename(&root, root.parent().unwrap().join("original-store")).unwrap();
    fs::rename(&decoy, &root).unwrap();
    assert_eq!(
        old_alpha_bytes(&reader, old),
        expected,
        "root path replacement"
    );

    // Replacing the objects directory inside an otherwise stable root must
    // likewise not redirect an already-open reader to new object bytes.
    let (_temporary, root) = working_store();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    fs::rename(root.join("objects"), root.join("objects-original")).unwrap();
    fs::create_dir(root.join("objects")).unwrap();
    fs::write(selected_object(&root, old_sha), b"other").unwrap();
    assert_eq!(
        old_alpha_bytes(&reader, old),
        expected,
        "objects directory replacement"
    );

    // A replaced revisions directory must not change which historical
    // manifest this already-open reader validates.
    let (_temporary, root) = working_store();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    fs::rename(root.join("revisions"), root.join("revisions-original")).unwrap();
    copy_tree(&root.join("revisions-original"), &root.join("revisions")).unwrap();
    fs::write(
        root.join("revisions").join(old).join("snapshot.json"),
        b"{}",
    )
    .unwrap();
    assert_eq!(
        old_alpha_bytes(&reader, old),
        expected,
        "revisions directory replacement"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn corpus_reader_refuses_symlinked_revision() {
    use std::os::unix::fs::symlink;

    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");

    let (_temporary, root) = working_store();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    let revision_dir = root.join("revisions").join(old);
    let real_dir = root.join("revisions").join("saved-revision");
    fs::rename(&revision_dir, &real_dir).unwrap();
    symlink(&real_dir, &revision_dir).unwrap();
    assert_eq!(
        reader.load_exact(revision(old)).unwrap_err().code,
        StoreErrorCode::UnsafePath,
        "symlinked intermediate revision"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn corpus_reader_refuses_fifo_object() {
    use std::process::Command;
    use std::time::{Duration, Instant};

    let fixture = fixture_manifest();
    let old = required(&fixture, "revision_old");
    if let Some(root) = std::env::var_os("TOS_ASS_FIFO_PROBE_ROOT") {
        fs::write(Path::new(&root).join(".fifo-probe-ran"), b"started").unwrap();
        let reader = CorpusReader::open_existing(Path::new(&root), read_limits()).unwrap();
        let snapshot = reader.load_exact(revision(old)).unwrap();
        assert_eq!(
            reader
                .resolve(&snapshot, Selector::SourceId("tos.work.synthetic.alpha"))
                .unwrap_err()
                .code,
            StoreErrorCode::UnsafePath,
            "opened FIFO must be refused as a non-regular object"
        );
        return;
    }

    let (_temporary, root) = working_store();
    let old_sha = required(
        &fixture["selected_cases"][0]["expected_descriptor"],
        "sha256",
    );
    let object = selected_object(&root, old_sha);
    fs::remove_file(&object).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(&object)
            .status()
            .unwrap()
            .success()
    );
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("corpus_reader_refuses_fifo_object")
        .env("TOS_ASS_FIFO_PROBE_ROOT", &root)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("FIFO object probe blocked past its 10-second watchdog");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "FIFO object probe failed");
    assert_eq!(
        fs::read(root.join(".fifo-probe-ran")).unwrap(),
        b"started",
        "child test filter did not execute the FIFO probe"
    );
}
