//! Private installed continuation of the existing actual native corpus caller.
//! The separate cold/domain phase does not enter this required Linux host gate.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use tos_compiler::{
    NativeKnowledgeSelection, NativeSelectionPaths, NativeSelectionProducer,
    knowledge_full_fixture::{FullKnowledgeFixture, NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS},
    source_corpus::NativeCorpusProjection,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonString, JsonValue,
    canonical_bytes_v1, parse_json,
};
use tos_query::corpus_read::CorpusReadRequest as R;

const VOCABULARY_PATH: &str = "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json";
const PRODUCER_SOURCE_MEMBERS: u64 = 4090;
const PRODUCER_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const PRODUCER_SOURCE_MEMBER_BYTES: u64 = 32 * 1024 * 1024;
const RECORD_MAX_BYTES: usize = 16 * 1024 * 1024;

fn run_native_corpus_build(binary: &Path, request: &serde_json::Value, preflight: bool) -> Output {
    let address_space_bytes = request["process_limits"]["address_space_bytes"]
        .as_u64()
        .expect("finite owner address-space cap");
    let file_size_bytes = request["process_limits"]["file_size_bytes"]
        .as_u64()
        .expect("finite owner file-size cap");
    let mut command = if let Some(launcher) = std::env::var_os("TOS_NATIVE_CORPUS_STAGE_LAUNCHER") {
        let launcher = Path::new(&launcher);
        assert!(launcher.is_absolute() && launcher.is_file());
        let mut command = Command::new(launcher);
        command.arg("/usr/bin/prlimit");
        command
    } else {
        Command::new("prlimit")
    };
    command
        .args([
            format!("--as={address_space_bytes}"),
            format!("--fsize={file_size_bytes}"),
            "--".into(),
        ])
        .arg(binary)
        .arg("corpus-build")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("TOS_RELEASE_ROOT");
    if preflight {
        command.arg("--preflight");
    }
    let mut child = command
        .spawn()
        .expect("admitted native owner binary starts");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(request).unwrap())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn canonical(value: &serde_json::Value) -> Vec<u8> {
    let raw = serde_json::to_vec(value).unwrap();
    // Expanded checkpoint inventories use the same finite record envelope
    // for hashing, persistence and reopen. Production readers keep their caps.
    let limits = JsonLimits {
        max_bytes: RECORD_MAX_BYTES,
        ..JsonLimits::default()
    };
    let doc = parse_json(&raw, JsonMode::PublishedStrict, limits).unwrap();
    canonical_bytes_v1(
        doc.root(),
        CanonicalProfile::CorpusSnapshotV1,
        limits,
    )
    .unwrap()
}
fn hash_file(path: &Path) -> Digest256 {
    let mut file = File::open(path).unwrap();
    let mut hash = Digest256Hasher::new();
    let mut block = [0u8; 65536];
    loop {
        let n = file.read(&mut block).unwrap();
        if n == 0 {
            break;
        }
        hash.update(&block[..n]);
    }
    hash.finalize()
}
fn encoded(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(b) {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
struct OwnedServer(Child);
impl Drop for OwnedServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn command(binary: &Path, root: &Path) -> Command {
    let limits = NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS;
    let mut cmd = Command::new("prlimit");
    cmd.args([
        format!("--as={}", limits.address_space_bytes),
        format!("--fsize={}", limits.file_size_bytes),
        "--".into(),
    ])
    .arg(binary)
    .arg("--release-root")
    .arg(root)
    .env_remove("TOS_RELEASE_ROOT");
    cmd
}
fn http(address: &str, method: &str, target: &str) -> (u16, usize, Vec<u8>) {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "{method} {target} HTTP/1.1\r\nHost: {address}\r\n\r\n"
    )
    .unwrap();
    let mut wire = Vec::new();
    stream
        .take(1_048_576 + 65536 + 1)
        .read_to_end(&mut wire)
        .unwrap();
    assert!(wire.len() <= 1_048_576 + 65536);
    let end = wire.windows(4).position(|v| v == b"\r\n\r\n").unwrap() + 4;
    let header = std::str::from_utf8(&wire[..end]).unwrap();
    let status = header.split_whitespace().nth(1).unwrap().parse().unwrap();
    let length = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .unwrap()
        .parse()
        .unwrap();
    (status, length, wire[end..].to_vec())
}
fn arguments(request: &R) -> serde_json::Value {
    use serde_json::json;
    match request {
        R::Status | R::Summary | R::GraphViews => json!({}),
        R::Search {
            query,
            limit,
            resource_kind,
        } => json!({"query":query,"limit":limit,"resource_kind":resource_kind}),
        R::Resources {
            resource_kind,
            owner_branch,
            limit,
        } => json!({"resource_kind":resource_kind,"owner_branch":owner_branch,"limit":limit}),
        R::Node { node_id } => json!({"node_id":node_id}),
        R::RelationPack { pack_id } => json!({"pack_id":pack_id}),
        R::GraphView { view_id, limit } => json!({"view_id":view_id,"limit":limit}),
        R::Packet {
            query,
            view_id,
            limit,
        } => json!({"query":query,"view_id":view_id,"limit":limit}),
    }
}
fn http_target(request: &R, path: &str) -> Option<String> {
    Some(match request {
        R::Status | R::Summary | R::GraphViews => path.to_owned(),
        R::Search {
            query,
            limit,
            resource_kind: None,
        } => format!("{path}?query={}&limit={limit}", encoded(query)),
        R::Node { node_id } => format!("{}{}", path.split('{').next().unwrap(), encoded(node_id)),
        R::RelationPack { pack_id } => {
            format!("{}{}", path.split('{').next().unwrap(), encoded(pack_id))
        }
        R::GraphView { view_id, limit } => format!(
            "{}{}?limit={limit}",
            path.split('{').next().unwrap(),
            encoded(view_id)
        ),
        R::Search {
            resource_kind: Some(_),
            ..
        }
        | R::Resources { .. }
        | R::Packet { .. } => return None,
    })
}
fn display_context(value: &mut JsonValue, root: &Path, index: &Path) {
    let JsonValue::Object(fields) = value else {
        panic!("status object required")
    };
    for (key, value) in fields {
        match key.as_str() {
            Some("tos_root") => {
                *value = JsonValue::String(JsonString::from_utf8(root.to_str().unwrap()))
            }
            Some("index_path") => {
                *value = JsonValue::String(JsonString::from_utf8(index.to_str().unwrap()))
            }
            _ => {}
        }
    }
}

fn publish_snapshot(
    root: &Path,
    data: &Path,
    binary: &Path,
    body: &serde_json::Value,
    binary_sha: Digest256,
) {
    use serde_json::json;
    let mut manifest = body.clone();
    manifest.as_object_mut().unwrap().remove("data_revision");
    let revision = Digest256::of_bytes(&canonical(&manifest)).to_hex();
    manifest["data_revision"] = json!(revision);
    let raw = canonical(&manifest);
    let manifest_path = data.join("data/manifest.json");
    if fs::read(&manifest_path).ok().as_deref() != Some(raw.as_slice()) {
        fs::write(&manifest_path, &raw).unwrap();
    }
    for dir in [
        "pairs",
        "bindings",
        "revocations/data",
        "revocations/corpus",
        "revocations/software",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    if !root.join(".release.lock").exists() {
        fs::write(root.join(".release.lock"), []).unwrap();
    }
    let pair = json!({"schema_version":"tos_access_release_pair_v1","software_sha256":binary_sha.to_hex(),"data_revision":revision,"data_manifest_sha256":Digest256::of_bytes(&raw).to_hex(),"corpus_revision":manifest["corpus_revision"],"query_schema":manifest["compiler"]["schema"],"compiler_version":tos_compiler::COMPILER_VERSION});
    let raw = canonical(&pair);
    let id = Digest256::of_bytes(&raw).to_hex();
    fs::write(root.join(format!("pairs/{id}.json")), raw).unwrap();
    fs::write(
        root.join(format!("bindings/{id}.json")),
        canonical(&json!({"data_root":data,"software_archive":binary})),
    )
    .unwrap();
    fs::write(
        root.join("current.json"),
        canonical(
            &json!({"schema_version":"tos_access_release_pointer_v1","current":id,"previous":null}),
        ),
    )
    .unwrap();
}

pub(super) fn prepare_managed_native_corpus(
    selected: &FullKnowledgeFixture,
    projection: &NativeCorpusProjection,
    repository: &Path,
    output_path: &str,
    files: &BTreeMap<String, Vec<u8>>,
    packets: &[(R, JsonValue)],
    case_root: &Path,
) -> serde_json::Value {
    use serde_json::json;
    assert_eq!(
        packets.len(),
        13,
        "one retained whole oracle, bounded native responses"
    );
    // This mandatory product is supplied only by OPS after source/build/hash
    // admission. There is no target discovery, copied binary or old-product fallback.
    let binary = std::env::var_os("TOS_NATIVE_MANAGED_CONSUMER_BIN")
        .map(std::path::PathBuf::from)
        .expect("OPS must supply the exact admitted native consumer binary");
    assert!(binary.is_absolute() && binary.is_file());
    let data = case_root.join("snapshot");
    fs::create_dir_all(data.join("data")).unwrap();
    let output = data.join("data").join(output_path);
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(&output, projection.output_bytes()).unwrap();
    let paths = NativeSelectionPaths {
        model: "data/model.sqlite3".into(),
        descriptor: "data/descriptor.json".into(),
        entity_registry: "data/entity-registry.json".into(),
        relation_registry: "data/relation-registry.json".into(),
    };
    fs::copy(&selected.path, data.join(&paths.model)).unwrap();
    fs::write(data.join(&paths.descriptor), &selected.descriptor_bytes).unwrap();
    let registries = selected.registry_originals();
    fs::write(data.join(&paths.entity_registry), registries[0]).unwrap();
    fs::write(data.join(&paths.relation_registry), registries[1]).unwrap();
    // Required concrete custody, never a fabricated measurement or skipped PASS.
    // Only the host-admitted installed phase calls this function.
    let measurement = tos_compiler::prepare_native_knowledge_artifact(
        &data.join(&paths.model),
        &selected.stage_receipt,
    )
    .expect("isolated native fs-verity custody prerequisite must succeed");
    let selection = NativeKnowledgeSelection::from_producer(
        paths.clone(),
        NativeSelectionProducer {
            stage: selected.stage_receipt.clone(),
            seal: selected.seal_receipt.clone(),
            navigation_original: selected.navigation_original.clone(),
            philosophy_original: selected.philosophy_original.clone(),
            corpus_original: selected.corpus_original.clone(),
            managed_source: None,
            managed_source_v2: None,
        },
        selected.expectation.clone(),
        measurement,
        selected.cold_limits(),
        NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS,
        &selected.descriptor_bytes,
        registries[0],
        registries[1],
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
        JsonLimits::default().max_bytes,
    )
    .unwrap();
    fs::write(
        data.join("data/native-selection.json"),
        selection.encode(JsonLimits::default().max_bytes).unwrap(),
    )
    .unwrap();
    let proof = projection.receipt();
    let program = "rust/crates/tos-compiler/src/source_corpus.rs";
    let schema = "ToS/contracts/tos-corpus-index.schema.json";
    let compiler_inputs = BTreeMap::from([
        (program, hash_file(&repository.join(program)).to_hex()),
        (schema, hash_file(&repository.join(schema)).to_hex()),
    ]);
    assert_eq!(compiler_inputs[program], proof.owner_program_sha256);
    assert_eq!(compiler_inputs[schema], proof.owner_schema_sha256);
    let mut inputs = files
        .iter()
        .map(|(path, raw)| (path.clone(), Digest256::of_bytes(raw).to_hex()))
        .collect::<BTreeMap<_, _>>();
    assert!(
        !inputs.contains_key(output_path),
        "produced output cannot be relabeled as source input"
    );
    inputs.insert(
        tos_access::release_state::RUNTIME_DATA_DECLARATION_PATH.to_owned(),
        Digest256::of_bytes(tos_access::release_state::RUNTIME_DATA_DECLARATION).to_hex(),
    );
    let mut members = vec![
        paths.model.clone(),
        paths.descriptor.clone(),
        paths.entity_registry.clone(),
        paths.relation_registry.clone(),
        "data/native-selection.json".into(),
        format!("data/{output_path}"),
    ];
    members.sort();
    let members=members.iter().map(|path|json!({"path":path,"size_bytes":fs::metadata(data.join(path)).unwrap().len(),"sha256":hash_file(&data.join(path)).to_hex()})).collect::<Vec<_>>();
    let manifest = json!({"schema_version":tos_access::release_state::NATIVE_DATA_SCHEMA,"corpus_revision":proof.source_revision,"input_bindings":inputs,"compiler":{"schema":selected.expectation.model_abi,"compiler_version":tos_compiler::COMPILER_VERSION,"compiler_sha256":hash_file(&std::env::current_exe().unwrap()).to_hex(),"compiler_paths":compiler_inputs.keys().collect::<Vec<_>>(),"input_bindings":compiler_inputs},"members":members,"native_selection":"data/native-selection.json"});
    let packets = packets
        .iter()
        .map(|(request, oracle)| {
            let route = tos_access::registered_operations()
                .unwrap()
                .iter()
                .find(|row| row.operation_id == request.operation_id())
                .unwrap();
            json!({"operation":request.operation_id(), "tool":route.mcp_tool,
            "arguments":arguments(request), "http":http_target(request, &route.http_path),
            "expected":String::from_utf8(canonical_bytes_v1(oracle,
                CanonicalProfile::SourceRecordDigestV1, JsonLimits::default()).unwrap()).unwrap()})
        })
        .collect::<Vec<_>>();
    json!({"data":data, "manifest":manifest, "output_path":output_path,
        "program":program, "proof":proof, "packets":packets})
}

fn selection(data: &Path) -> NativeKnowledgeSelection {
    NativeKnowledgeSelection::decode(
        &fs::read(data.join("data/native-selection.json")).unwrap(),
        &fs::read(data.join("data/descriptor.json")).unwrap(),
        &fs::read(data.join("data/entity-registry.json")).unwrap(),
        &fs::read(data.join("data/relation-registry.json")).unwrap(),
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
        1_048_576,
    )
    .unwrap()
}

fn exercise_managed_native_corpus(case: &serde_json::Value, phase: &str) {
    use serde_json::json;
    let binary = PathBuf::from(
        std::env::var_os("TOS_NATIVE_MANAGED_CONSUMER_BIN")
            .expect("OPS must supply the admitted consumer"),
    );
    let temporary = tempfile::tempdir().unwrap();
    let data = temporary.path().join("snapshot");
    let root = temporary.path().join("release");
    copy_tree(Path::new(case["data"].as_str().unwrap()), &data);
    let selection = selection(&data);
    tos_compiler::prepare_native_knowledge_artifact(
        &data.join("data/model.sqlite3"),
        &selection.producer().stage,
    )
    .unwrap();
    let manifest = case["manifest"].clone();
    let output_path = case["output_path"].as_str().unwrap();
    let output = data.join("data").join(output_path);
    let program = case["program"].as_str().unwrap();
    let proof = &case["proof"];
    let packets = case["packets"].as_array().unwrap();
    let binary_sha = hash_file(&binary);
    publish_snapshot(&root, &data, &binary, &manifest, binary_sha);
    let release = tos_access::release_state::ManagedRelease::open(&root).unwrap();
    let receipt = selection.producer().corpus_original.as_ref().unwrap();
    let mut hold = release.acquire().unwrap();
    let (context, guards) = hold
        .admit_corpus_members(receipt, selection.cold_limits().max_work_bytes as usize)
        .unwrap();
    assert_eq!(context.index_path, output.to_str().unwrap());
    assert_eq!(context.tos_root, data.join("data").to_str().unwrap());
    let exclusive = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(".release.lock"))
        .unwrap();
    assert!(
        exclusive.try_lock().is_err(),
        "real shared ReleaseLease holds selected publication"
    );
    hold.retain_member_guards(&guards).unwrap();
    hold.recheck().unwrap();
    drop(hold);
    exclusive.try_lock().unwrap();
    exclusive.unlock().unwrap();
    let mut expected = vec![];
    let mut input = String::from(
        "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":-1,\"method\":\"tools/list\"}\n",
    );
    for (index, row) in packets.iter().enumerate() {
        let mut packet = parse_json(
            row["expected"].as_str().unwrap().as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        if row["operation"] == R::Status.operation_id() {
            display_context(&mut packet, &data.join("data"), &output);
        } else if row["operation"] == R::Summary.operation_id() {
            let JsonValue::Object(fields) = &mut packet else {
                unreachable!()
            };
            display_context(
                &mut fields
                    .iter_mut()
                    .find(|(key, _)| key.as_str() == Some("status"))
                    .unwrap()
                    .1,
                &data.join("data"),
                &output,
            );
        }
        expected.push(
            canonical_bytes_v1(
                &packet,
                CanonicalProfile::SourceRecordDigestV1,
                JsonLimits::default(),
            )
            .unwrap(),
        );
        input.push_str(
            &serde_json::to_string(&json!({"jsonrpc":"2.0","id":index+1,
            "method":"tools/call","params":{"name":row["tool"],"arguments":row["arguments"]}}))
            .unwrap(),
        );
        input.push('\n');
    }
    if matches!(phase, "consumer" | "mcp") {
        let mut child = command(&binary, &root);
        child
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = child.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let response = child.wait_with_output().unwrap();
        assert!(
            response.status.success(),
            "{}",
            String::from_utf8_lossy(&response.stderr)
        );
        let lines = response
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), packets.len() + 2);
        let frame_cap = tos_access::mcp::tool_result_frame_byte_bound(1_048_576, 65_536).unwrap();
        assert!(response.stdout.len() <= (packets.len() + 2) * frame_cap);
        let advertised =
            parse_json(lines[1], JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        let tools = advertised
            .root()
            .object_get("result")
            .unwrap()
            .object_get("tools")
            .unwrap()
            .as_array()
            .unwrap();
        for row in packets {
            assert!(
                tools
                    .iter()
                    .any(|tool| tool.object_get("name").and_then(JsonValue::as_str)
                        == row["tool"].as_str()),
                "actual managed corpus tool must be advertised"
            );
        }
        for (index, body) in expected.iter().enumerate() {
            let frame = parse_json(
                lines[index + 2],
                JsonMode::PublishedStrict,
                JsonLimits {
                    max_bytes: frame_cap,
                    ..JsonLimits::default()
                },
            )
            .unwrap();
            let result = frame.root().object_get("result").unwrap();
            assert_eq!(
                result.object_get("content").unwrap().as_array().unwrap()[0]
                    .object_get("text")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .as_bytes(),
                body
            );
            assert_eq!(
                canonical_bytes_v1(
                    result.object_get("structuredContent").unwrap(),
                    CanonicalProfile::SourceRecordDigestV1,
                    JsonLimits::default()
                )
                .unwrap(),
                *body
            );
        }
    }
    if phase == "mcp" {
        return;
    }
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address = format!("127.0.0.1:{port}");
    let mut server = OwnedServer(
        command(&binary, &root)
            .args(["serve", &address])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if TcpStream::connect(&address).is_ok() {
            break;
        }
        if let Some(status) = server.0.try_wait().unwrap() {
            panic!("actual managed native server exited before ready: {status}");
        }
        assert!(Instant::now() < deadline, "bounded native server startup");
        thread::sleep(Duration::from_millis(10));
    }
    let mut http_operations = BTreeSet::new();
    for (index, row) in packets.iter().enumerate() {
        let Some(target) = row["http"].as_str() else {
            continue;
        };
        http_operations.insert(row["operation"].as_str().unwrap());
        for method in ["GET", "HEAD"] {
            let (status, length, body) = http(&address, method, &target);
            assert_eq!(status, 200, "{target}: {}", String::from_utf8_lossy(&body));
            assert_eq!(length, expected[index].len());
            if method == "GET" {
                assert_eq!(body, expected[index]);
            } else {
                assert!(body.is_empty());
            }
        }
    }
    assert_eq!(http_operations.len(), 6);
    if phase == "http" {
        return;
    }
    // Existing DataGuard/current-release failure paths protect this new origin.
    let status_route = tos_access::registered_operations()
        .unwrap()
        .iter()
        .find(|row| row.operation_id == R::Status.operation_id())
        .unwrap();
    exclusive.try_lock().unwrap();
    let retained = output.with_extension("retained-output");
    fs::rename(&output, &retained).unwrap();
    exclusive.unlock().unwrap();
    let (status, _, body) = http(&address, "GET", &status_route.http_path);
    assert_eq!(status, 503);
    assert!(!String::from_utf8_lossy(&body).contains("\"index_exists\":true"));
    drop(server);
    exclusive.try_lock().unwrap();
    fs::rename(retained, &output).unwrap();
    exclusive.unlock().unwrap();
    // Valid release metadata cannot admit a false native source/code binding or
    // conceal produced output in the input vector. Recompute legitimate pair
    // digests so refusals reach the actual factory provenance boundary.
    for fault in ["source-revision", "compiler-pin", "output-as-input"] {
        let mut changed = manifest.clone();
        match fault {
            "source-revision" => changed["corpus_revision"] = json!("0".repeat(64)),
            "compiler-pin" => {
                changed["compiler"]["input_bindings"][program] = json!("0".repeat(64))
            }
            "output-as-input" => {
                changed["input_bindings"][output_path] = proof["output_sha256"].clone()
            }
            _ => unreachable!(),
        }
        exclusive.try_lock().unwrap();
        publish_snapshot(&root, &data, &binary, &changed, binary_sha);
        exclusive.unlock().unwrap();
        let refusal = command(&binary, &root)
            .arg("mcp")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(refusal.status.code(), Some(3), "{fault}");
        assert!(refusal.stdout.is_empty(), "{fault}");
        let expected_boundary = if fault == "output-as-input" {
            "corpus source member binding differs"
        } else {
            "native corpus"
        };
        assert!(
            String::from_utf8_lossy(&refusal.stderr).contains(expected_boundary),
            "{fault}: {}",
            String::from_utf8_lossy(&refusal.stderr)
        );
    }
    exclusive.try_lock().unwrap();
    publish_snapshot(&root, &data, &binary, &manifest, binary_sha);
    let record = json!({"schema_version":"tos_access_release_revocation_v1","kind":"corpus","digest":proof["source_revision"],"reason":"isolated native corpus withdrawal"});
    fs::write(
        root.join(format!(
            "revocations/corpus/{}.json",
            proof["source_revision"].as_str().unwrap()
        )),
        canonical(&record),
    )
    .unwrap();
    exclusive.unlock().unwrap();
    let refusal = command(&binary, &root)
        .arg("mcp")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(refusal.status.code(), Some(3));
    assert!(refusal.stdout.is_empty());
}

/// Exercise the actual native source-cut producer under the already admitted
/// private-stage ticket, then verify only its disposable candidate through the
/// existing managed reader. This route never writes the production pointer.
fn stage_resources() -> (u64, u64, u64, PathBuf) {
    // OPS can issue the real ticket at each producer exec. This keeps the
    // consumer's fs-verity files and HTTP host outside the private tmpfs/network
    // namespace. The production owner still verifies the sealed ticket and
    // kernel boundaries against every requested limit before writing.
    if std::env::var_os("TOS_NATIVE_CORPUS_STAGE_LAUNCHER").is_some() {
        let path = std::env::var_os("TOS_NATIVE_CORPUS_STAGE_CONFIG")
            .expect("OPS must supply the exact launcher resource selection");
        let path = Path::new(&path);
        assert!(path.is_absolute() && path.is_file());
        let mut raw = Vec::new();
        File::open(path)
            .unwrap()
            .take(8193)
            .read_to_end(&mut raw)
            .unwrap();
        assert!(raw.len() <= 8192);
        let limits: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let keys = limits
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys,
            BTreeSet::from([
                "quota_bytes",
                "inode_limit",
                "working_ram_bytes",
                "persistent_store"
            ])
        );
        let persistent_store =
            std::path::PathBuf::from(limits["persistent_store"].as_str().unwrap());
        assert!(persistent_store.is_absolute() && persistent_store.is_dir());
        (
            limits["quota_bytes"].as_u64().unwrap(),
            limits["inode_limit"].as_u64().unwrap(),
            limits["working_ram_bytes"].as_u64().unwrap(),
            persistent_store,
        )
    } else {
        let stage = tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_issued_from_environment()
                .expect("OPS must issue the native producer private tmpfs ticket");
        let (quota_bytes, inode_limit, working_ram_bytes) = stage.resource_limits();
        let persistent_store = stage
            .persistent_store()
            .expect("OPS must admit the candidate's bounded persistent store")
            .to_owned();
        (
            quota_bytes,
            inode_limit,
            working_ram_bytes,
            persistent_store,
        )
    }
}

fn build_request(
    cold: tos_compiler::ColdOpenLimits,
    mut source_selection: serde_json::Value,
) -> serde_json::Value {
    use serde_json::json;
    let (quota_bytes, inode_limit, working_ram_bytes, _) = stage_resources();
    let process_limits = tos_compiler::NativeProcessLimits {
        // The complete philosophy producer has its own admitted working set;
        // the small consumer fixture's 1 GiB process cap does not describe it.
        address_space_bytes: working_ram_bytes,
        // Permit the full declared model file, including philosophy products.
        file_size_bytes: cold.max_file_bytes,
    };
    for (key, value) in [
        ("max_revisions", 1u64),
        ("max_members", PRODUCER_SOURCE_MEMBERS),
        ("max_total_bytes", PRODUCER_SOURCE_BYTES),
        ("max_member_bytes", PRODUCER_SOURCE_MEMBER_BYTES),
        ("max_schema_receipts", 4096),
        ("max_schema_receipt_bytes", 4 * 1024 * 1024),
        ("worker_cpu_seconds", 60),
        (
            "worker_address_space_bytes",
            working_ram_bytes.min(1024 * 1024 * 1024),
        ),
    ] {
        source_selection[key] = json!(value);
    }
    // Full philosophy materialization alone previously measured over nine
    // minutes. Its independent producer needs the measured full-data work
    // envelope; this changes no process-memory or physical-file ceiling.
    json!({
       "schema_version": "tos_native_corpus_build_request_v1",
       "mode": "build",
       "tmpfs_quota_bytes": quota_bytes,
       "tmpfs_inode_limit": inode_limit,
       "working_ram_bytes": working_ram_bytes,
       "max_state_bytes": 512 * 1024 * 1024,
       "max_json_visits": 8_000_000,
       "max_work_bytes": 32 * 1024 * 1024 * 1024u64,
       "persistent_write_cap_bytes": 512 * 1024 * 1024u64,
       "max_build_seconds": 1800,
       "cold_open": serde_json::to_value(cold).unwrap(),
       "process_limits": serde_json::to_value(process_limits).unwrap(),
       "data_directory": "native-corpus-build-candidate",
       "private_release_directory": "native-corpus-build-unused-release",
       "source_only": source_selection,
    })
}

pub(super) fn prepare_native_corpus_build(
    cold: tos_compiler::ColdOpenLimits,
    repository: &Path,
    source_store: &Path,
    source_revision: tos_foundation::SourceRevision,
    software: &super::super::source_cut_cases::SoftwareCaptureFixture,
    worker_path: &Path,
) -> serde_json::Value {
    use serde_json::json;
    let owner = std::env::var_os("TOS_NATIVE_OWNER_COMMAND_BIN")
        .map(std::path::PathBuf::from)
        .expect("OPS must provide the exact admitted native owner binary");
    assert!(owner.is_absolute() && owner.is_file());
    let consumer = std::env::var_os("TOS_NATIVE_MANAGED_CONSUMER_BIN")
        .map(std::path::PathBuf::from)
        .expect("OPS must provide the exact admitted native consumer binary");
    assert!(consumer.is_absolute() && consumer.is_file());
    assert!(worker_path.is_absolute() && worker_path.is_file());

    let worker_source = "rust/crates/tos-validation/src/bin/tos-schema-worker.rs";
    let source_selection = json!({
        "corpus_store": source_store.display().to_string(),
        "source_revision": source_revision.0.to_hex(),
        "max_revisions": 1,
        "max_members": 512,
        "max_total_bytes": 16 * 1024 * 1024,
        "max_member_bytes": 2 * 1024 * 1024,
        "software_capture": software.capture.display().to_string(),
        "software_restored_root": software.restored.display().to_string(),
        "source_git_commit": software.selection.source_git_commit,
        "source_git_tree": software.selection.source_git_tree,
        "capture_manifest_sha256": software.selection.capture_manifest_sha256.to_hex(),
        "software_components": [VOCABULARY_PATH, worker_source],
        "schema_worker_path": worker_source,
        "schema_worker_absolute_path": worker_path.display().to_string(),
        "schema_worker_sha256": hash_file(worker_path).to_hex(),
        "max_schema_receipts": 4096,
        "max_schema_receipt_bytes": 4 * 1024 * 1024,
        "worker_cpu_seconds": 60,
    });
    let build_request = build_request(cold, source_selection);
    json!({"request":build_request,"source_revision":source_revision.0.to_hex(),
        "runtime_declaration":serde_json::from_slice::<serde_json::Value>(
            &fs::read(repository.join("access/contracts/runtime-data.v1.json")).unwrap()).unwrap()})
}

/// The small retained consumer cut intentionally has no philosophy family.
/// The six-product writer needs that family's actual authored inputs. Keep the
/// old cut intact and declare a separate complete producer cut, preserving all
/// its original members byte-for-byte. No generated projection is an input.
pub(super) fn prepare_producer_source(
    base: &serde_json::Value,
    repository: &Path,
    target: &Path,
) -> serde_json::Value {
    use tos_source_store::{CorpusReader, CutReadLimits, ReadLimits};
    let source = &base["request"]["source_only"];
    assert!(
        base.get("source_cohort").is_none(),
        "producer source is already prepared"
    );
    assert!(!target.exists());
    let deadline = Instant::now() + Duration::from_secs(120);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let revision = tos_foundation::SourceRevision(
        Digest256::from_hex(source["source_revision"].as_str().unwrap()).unwrap(),
    );
    let reader = CorpusReader::open_existing(
        Path::new(source["corpus_store"].as_str().unwrap()),
        ReadLimits {
            max_manifest_bytes: 4 * 1024 * 1024,
            max_manifest_entries: PRODUCER_SOURCE_MEMBERS as usize,
            max_selected_object_bytes: PRODUCER_SOURCE_MEMBER_BYTES,
            json: JsonLimits::default(),
        },
    )
    .unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 1,
                max_members: PRODUCER_SOURCE_MEMBERS,
                max_total_bytes: PRODUCER_SOURCE_BYTES,
                max_member_bytes: PRODUCER_SOURCE_MEMBER_BYTES,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let mut stream = cut.stream(revision).unwrap();
    let membership = stream.expectation();
    let mut files = BTreeMap::new();
    while let Some(member) = stream.next_member(deadline, &cancelled).unwrap() {
        assert!(
            files
                .insert(member.path.as_str().to_owned(), member.raw)
                .is_none()
        );
    }
    assert_eq!(stream.coverage(), Some(membership));
    let original = files
        .iter()
        .map(|(path, bytes)| (path.clone(), Digest256::of_bytes(bytes)))
        .collect::<BTreeMap<_, _>>();
    fn add_philosophy(
        repository: &Path,
        root: &Path,
        files: &mut BTreeMap<String, Vec<u8>>,
        deadline: Instant,
        bytes: &mut u64,
    ) {
        assert!(Instant::now() < deadline);
        let mut children = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        children.sort();
        for path in children {
            assert!(Instant::now() < deadline);
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                add_philosophy(repository, &path, files, deadline, bytes);
                continue;
            }
            assert!(metadata.is_file() && metadata.len() <= PRODUCER_SOURCE_MEMBER_BYTES);
            let relative = path.strip_prefix(repository).unwrap().to_str().unwrap();
            let raw = fs::read(&path).unwrap();
            assert_eq!(raw.len() as u64, metadata.len());
            if let Some(existing) = files.get(relative) {
                assert_eq!(
                    existing, &raw,
                    "overlapping source member changed: {relative}"
                );
            } else {
                *bytes = bytes.checked_add(raw.len() as u64).unwrap();
                assert!(*bytes <= PRODUCER_SOURCE_BYTES);
                assert!(files.len() < PRODUCER_SOURCE_MEMBERS as usize);
                files.insert(relative.to_owned(), raw);
            }
        }
    }
    let mut bytes = files.values().map(|raw| raw.len() as u64).sum::<u64>();
    add_philosophy(
        repository,
        &repository.join("ToS/philosophy"),
        &mut files,
        deadline,
        &mut bytes,
    );
    // Check the actual owner's entry inputs before capture/index preparation.
    // Further dependency/semantic checks remain with its normal producer.
    for path in [
        tos_compiler::source_philosophy_multilingual::LABEL_LEDGER,
        tos_compiler::source_philosophy_atlas::ATLAS_SOURCE,
        tos_compiler::source_philosophy_atlas::DOSSIERS_SOURCE,
        tos_compiler::source_philosophy_atlas::DOSSIERS_MANIFEST,
        tos_compiler::source_philosophy_atlas::GRAPH_SHAPE,
        tos_compiler::source_philosophy_views::VIEW_CONTRACT,
        tos_compiler::source_philosophy_views::LENS_CONTRACT,
        tos_compiler::source_philosophy_views::LAYERS_SOURCE,
        tos_compiler::source_philosophy_graph::CLUSTER_CONTRACT,
        tos_compiler::source_philosophy_graph::REVIEW_CONTRACT,
    ] {
        assert!(files.contains_key(path), "producer input absent: {path}");
    }
    for (path, sha) in &original {
        assert_eq!(Digest256::of_bytes(&files[path]), *sha);
    }
    // Repository topology compares the selected source cut with its exact
    // capture inventory. Extend both from the same bytes, retaining every old
    // companion through the capture reader rather than an ambient checkout.
    let inventory = tos_source_store::SoftwareCaptureReader::open(
        Path::new(source["software_capture"].as_str().unwrap()),
        Path::new(source["software_restored_root"].as_str().unwrap()),
        tos_source_store::SoftwareCaptureSelectionV1 {
            source_git_commit: source["source_git_commit"].as_str().unwrap().into(),
            source_git_tree: source["source_git_tree"].as_str().unwrap().into(),
            capture_manifest_sha256: Digest256::from_hex(
                source["capture_manifest_sha256"].as_str().unwrap(),
            )
            .unwrap(),
        },
        ReadLimits {
            max_manifest_bytes: 4 * 1024 * 1024,
            max_manifest_entries: PRODUCER_SOURCE_MEMBERS as usize,
            max_selected_object_bytes: PRODUCER_SOURCE_MEMBER_BYTES,
            json: JsonLimits::default(),
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    assert!(inventory.exclude_prefixes().is_empty());
    assert!(inventory.exclude_path_parts().is_empty());
    let mut captured = files.clone();
    for member in inventory.members() {
        assert_eq!(member.mode, 0o644, "fixture companions are ordinary files");
        if let Some(raw) = captured.get(member.path.as_str()) {
            assert_eq!(raw.len() as u64, member.size_bytes);
            assert_eq!(Digest256::of_bytes(raw), member.sha256);
        } else {
            let selected = inventory
                .select_components(std::slice::from_ref(&member.path))
                .unwrap();
            let raw = inventory
                .read_selected_component(
                    &selected,
                    &member.path,
                    PRODUCER_SOURCE_MEMBER_BYTES,
                    deadline,
                    &cancelled,
                )
                .unwrap();
            captured.insert(member.path.as_str().to_owned(), raw);
        }
    }
    assert!(captured.len() <= PRODUCER_SOURCE_MEMBERS as usize);
    assert!(captured.values().map(|raw| raw.len() as u64).sum::<u64>() <= PRODUCER_SOURCE_BYTES);
    let git_root = target.with_file_name("producer-git");
    fs::create_dir(&git_root).unwrap();
    super::git(&git_root, &["init", "-q"]);
    for (path, raw) in &captured {
        let output = git_root.join(path);
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(output, raw).unwrap();
    }
    super::git(&git_root, &["add", "--force", "--all"]);
    super::git(
        &git_root,
        &[
            "-c", "user.name=Fixture", "-c", "user.email=fixture@invalid",
            "-c", "commit.gpgsign=false", "commit", "-qm",
            "Retained source cut with complete philosophy inputs",
        ],
    );
    let commit = String::from_utf8(super::git(&git_root, &["rev-parse", "HEAD^{commit}"]))
        .unwrap().trim().to_owned();
    let capture = target.with_file_name("producer-software-capture");
    let restored = target.with_file_name("producer-software-restored");
    let prefixes = inventory.include_prefixes().iter().map(String::as_str).collect::<Vec<_>>();
    let selection = super::super::source_cut_cases::capture_software_archive(
        &git_root, &commit, &prefixes, &capture, deadline, &cancelled,
    );
    super::super::source_cut_cases::restore_software_archive(
        &capture, &restored, &selection, deadline, &cancelled,
    );
    let producer_revision = super::super::validation_cut_cases::write_cut_store(&files, target);
    let mut source = source.clone();
    source["corpus_store"] = serde_json::json!(target);
    source["source_revision"] = serde_json::json!(producer_revision.0.to_hex());
    source["software_capture"] = serde_json::json!(capture);
    source["software_restored_root"] = serde_json::json!(restored);
    source["source_git_commit"] = serde_json::json!(selection.source_git_commit);
    source["source_git_tree"] = serde_json::json!(selection.source_git_tree);
    source["capture_manifest_sha256"] = serde_json::json!(selection.capture_manifest_sha256.to_hex());
    let mut cold: tos_compiler::ColdOpenLimits =
        serde_json::from_value(base["request"]["cold_open"].clone()).unwrap();
    cold.max_file_bytes = 512 * 1024 * 1024;
    cold.max_vm_steps = 50_000_000_000;
    // The cold reader counts decoded postings as well as graph rows. Use the
    // same finite envelope as the full-data qualification, not the small
    // consumer fixture's row allowance.
    cold.max_rows = 256_000_000;
    cold.max_work_bytes = 32 * 1024 * 1024 * 1024;
    cold.max_row_bytes = 8 * 1024 * 1024;
    cold.max_metadata_bytes = tos_compiler::ColdOpenLimits::MAX_METADATA_BYTES;
    cold.validate().unwrap();
    let mut prepared = base.clone();
    prepared
        .as_object_mut()
        .unwrap()
        .remove("projection_source_revision");
    prepared["request"] = build_request(cold, source);
    prepared["consumer_source_revision"] = serde_json::json!(revision.0.to_hex());
    prepared["source_revision"] = serde_json::json!(producer_revision.0.to_hex());
    prepared["source_cohort"] = serde_json::json!({
        "selection":"retained consumer sources plus complete authored ToS/philosophy branch",
        "original_members":original.len(),"producer_members":files.len(),"producer_bytes":bytes,
        "original_members_unchanged":true,
        "capture_members":captured.len(),"original_companions_unchanged":true,
    });
    prepared
}

fn exercise_native_corpus_build(case: &serde_json::Value, phase: &str, case_root: &Path) {
    use serde_json::json;
    assert!(
        case.get("source_cohort").is_some(),
        "the retained consumer cut needs producer-prepare before a six-product build"
    );
    let owner = PathBuf::from(std::env::var_os("TOS_NATIVE_OWNER_COMMAND_BIN").unwrap());
    let consumer = PathBuf::from(std::env::var_os("TOS_NATIVE_MANAGED_CONSUMER_BIN").unwrap());
    let build_request = &case["request"];
    let (_, _, _, persistent_store) = stage_resources();
    let result_path = case_root.join("producer-result.json");
    let built = if phase == "producer" {
        assert!(
            !result_path.exists(),
            "a completed producer result is immutable"
        );
        let preflight = run_native_corpus_build(&owner, build_request, true);
        assert!(
            preflight.status.success(),
            "native preflight refused: {}",
            String::from_utf8_lossy(&preflight.stderr)
        );
        let built = run_native_corpus_build(&owner, build_request, false);
        assert!(
            built.status.success(),
            "native corpus build refused: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        assert!(built.stdout.len() <= 1_048_576);
        let receipt: serde_json::Value = serde_json::from_slice(&built.stdout).unwrap();
        receipt
    } else {
        let result = read_record(&result_path);
        verify_pins(&result["inputs"]);
        assert_eq!(
            result["producer_sha256"],
            hash_file(&owner).to_hex(),
            "changed producer requires a fresh producer result"
        );
        assert_eq!(
            result["request_sha256"],
            Digest256::of_bytes(&canonical(build_request)).to_hex()
        );
        result["receipt"].clone()
    };
    assert_eq!(built["current_release_promoted"], false);
    assert_eq!(built["private_release_root_created"], false);
    assert_eq!(built["installed_access_mcp_accepted"], false);
    assert_eq!(built["cold_witness"]["actual_cold_open_completed"], true);
    assert_eq!(built["authority"]["source_admission"], false);
    assert_eq!(built["authority"]["publication"], false);
    assert_eq!(built["native_model_reused"], false);
    let data_root = Path::new(built["data_root"].as_str().unwrap());
    assert!(data_root.is_absolute() && data_root.is_dir());
    assert!(
        !persistent_store
            .join("native-corpus-build-unused-release")
            .exists()
    );

    let data = data_root.join("data");
    let manifest_raw = fs::read(data.join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_raw).unwrap();
    let members = manifest["members"].as_array().unwrap();
    let declaration = &case["runtime_declaration"];
    let product_path = |subject: &str| -> String {
        let rows = declaration["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["subject_id"] == subject)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1, "runtime product {subject}");
        rows[0]["source_path"].as_str().unwrap().to_owned()
    };
    let corpus_path = product_path("tos-corpus-index");
    let claims_path = product_path("tos-source-witness-bibliographic-claim-graph");
    let corpus_file = data.join(&corpus_path);
    let claims_file = data.join(&claims_path);
    let corpus_bytes = fs::read(&corpus_file).unwrap();
    let claims_bytes = fs::read(&claims_file).unwrap();
    assert_eq!(case["source_cohort"]["original_members_unchanged"], true);
    assert_eq!(
        case["request"]["source_only"]["source_revision"], case["source_revision"],
        "writer uses the independently declared complete producer source cut"
    );
    for (path, raw) in [(&corpus_path, &corpus_bytes), (&claims_path, &claims_bytes)] {
        let member = members
            .iter()
            .find(|row| row["path"] == format!("data/{path}"))
            .expect("generated runtime product is admitted as a data member");
        assert_eq!(member["size_bytes"].as_u64(), Some(raw.len() as u64));
        let expected_sha = Digest256::of_bytes(raw).to_hex();
        assert_eq!(member["sha256"].as_str(), Some(expected_sha.as_str()));
    }
    assert!(!claims_bytes.is_empty());

    if phase == "producer" {
        let data = Path::new(built["data_root"].as_str().unwrap());
        let mut inputs = Vec::new();
        pin_tree(data, &mut inputs);
        write_record(
            &result_path,
            &json!({"receipt":built,"inputs":inputs,
            "producer_sha256":hash_file(&owner).to_hex(),
            "request_sha256":Digest256::of_bytes(&canonical(build_request)).to_hex()}),
        );
        return;
    }
    if phase == "check" {
        // Check mode recomputes the same six products and is read-only. The
        // negative case uses a separate disposable copy so the candidate stays
        // intact for the reader admission below.
        let check_data_name = "native-corpus-check-unused-data";
        let check_release_name = "native-corpus-check-unused-release";
        let mut check_request = (*build_request).clone();
        check_request["mode"] = json!("check");
        check_request["comparison_root"] = json!(data.display().to_string());
        check_request["data_directory"] = json!(check_data_name);
        check_request["private_release_directory"] = json!(check_release_name);
        let checked = run_native_corpus_build(&owner, &check_request, false);
        assert!(
            checked.status.success(),
            "native corpus parity check refused: {}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let checked: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
        assert_eq!(checked["mode"], "check");
        assert_eq!(checked["persistent_write_performed"], false);
        let products = checked["products"].as_array().unwrap();
        assert_eq!(products.len(), 6);
        assert!(products.iter().any(|row| row["path"] == corpus_path));
        assert!(products.iter().any(|row| row["path"] == claims_path));
        for row in products {
            assert_eq!(row["matches"], true);
        }
        assert!(!persistent_store.join(check_data_name).exists());
        assert!(!persistent_store.join(check_release_name).exists());

        let compare = tempfile::tempdir().unwrap();
        for row in products {
            let relative = row["path"].as_str().unwrap();
            let raw = fs::read(data.join(relative)).unwrap();
            assert_eq!(
                Digest256::of_bytes(&raw).to_hex(),
                row["sha256"].as_str().unwrap()
            );
            let target = compare.path().join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, raw).unwrap();
        }
        let mut negative_request = check_request.clone();
        negative_request["comparison_root"] = json!(compare.path().display().to_string());
        let damaged = compare.path().join(&claims_path);
        let mut damaged_bytes = fs::read(&damaged).unwrap();
        damaged_bytes[0] ^= 1;
        fs::write(&damaged, damaged_bytes).unwrap();
        let refused = run_native_corpus_build(&owner, &negative_request, false);
        assert!(
            !refused.status.success(),
            "changed product must fail parity"
        );
        assert!(refused.stdout.is_empty());
        assert!(!persistent_store.join(check_data_name).exists());
        assert!(!persistent_store.join(check_release_name).exists());

        return;
    }
    assert_eq!(phase, "producer-read");
    // Admit the disposable output through the current Rust native reader. This
    // pair lives only under the test TempDir, never at an installed current
    // pointer or production release root.
    let selection_path = data.join("native-selection.json");
    let selection_raw = fs::read(&selection_path).unwrap();
    let descriptor = fs::read(data.join(VOCABULARY_PATH)).unwrap();
    let entities =
        fs::read(data.join("ToS/doctrine/semantic-interchange/entity-types.v1.json")).unwrap();
    let relations =
        fs::read(data.join("ToS/doctrine/semantic-interchange/relation-types.v1.json")).unwrap();
    let selection = NativeKnowledgeSelection::decode(
        &selection_raw,
        &descriptor,
        &entities,
        &relations,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
        1_048_576,
    )
    .unwrap();
    let receipt = selection
        .producer()
        .corpus_original
        .as_ref()
        .expect("native producer must retain corpus Original receipt");
    assert_eq!(
        receipt.source_cut,
        built["selected_source"]["native_projection_source_cut"]
            .as_str()
            .unwrap()
    );
    let base = tempfile::tempdir().unwrap();
    let release_root = base.path().join("release");
    publish_snapshot(
        &release_root,
        data_root,
        &consumer,
        &manifest,
        hash_file(&consumer),
    );
    assert_eq!(
        hash_file(&data.join("manifest.json")).to_hex(),
        built["data_manifest"]["sha256"].as_str().unwrap(),
        "fixture-only release admission must preserve the native writer manifest"
    );
    let release = tos_access::release_state::ManagedRelease::open(&release_root).unwrap();
    let mut hold = release.acquire().unwrap();
    let (context, guards) = hold
        .admit_corpus_members(receipt, selection.cold_limits().max_work_bytes as usize)
        .unwrap();
    assert_eq!(context.index_path, corpus_file.to_str().unwrap());
    assert_eq!(context.tos_root, data.to_str().unwrap());
    hold.retain_member_guards(&guards).unwrap();
    hold.recheck().unwrap();
}

// These are disposable conformance checkpoints, not production manifests or
// grants. The existing producer/reader still admits every real use.
fn copy_tree(source: &Path, target: &Path) {
    assert!(!source.is_symlink());
    fs::create_dir_all(target).unwrap();
    let mut entries = fs::read_dir(source)
        .unwrap()
        .map(|row| row.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        let output = target.join(path.file_name().unwrap());
        if metadata.is_dir() {
            copy_tree(&path, &output);
        } else {
            assert!(metadata.is_file());
            fs::copy(path, output).unwrap();
        }
    }
}
fn pin_tree(path: &Path, rows: &mut Vec<serde_json::Value>) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(!metadata.is_symlink());
    if metadata.is_dir() {
        let mut children = fs::read_dir(path)
            .unwrap()
            .map(|row| row.unwrap().path())
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            pin_tree(&child, rows);
        }
    } else {
        assert!(metadata.is_file() && path.is_absolute());
        assert!(rows.len() < 4096);
        assert!(metadata.len() <= 512 * 1024 * 1024);
        rows.push(serde_json::json!({"path":path,"bytes":metadata.len(),"sha256":hash_file(path).to_hex()}));
    }
}
fn verify_pins(value: &serde_json::Value) {
    let rows = value.as_array().unwrap();
    assert!(!rows.is_empty() && rows.len() <= 4096);
    let mut total = 0u64;
    for row in rows {
        let path = Path::new(row["path"].as_str().unwrap());
        assert!(path.is_absolute());
        let meta = fs::symlink_metadata(path).unwrap();
        assert!(meta.is_file() && meta.uid() == fs::metadata("/proc/self").unwrap().uid());
        total = total.checked_add(meta.len()).unwrap();
        assert!(total <= 512 * 1024 * 1024);
        assert_eq!(Some(meta.len()), row["bytes"].as_u64());
        assert_eq!(hash_file(path).to_hex(), row["sha256"].as_str().unwrap());
    }
}
fn read_record(path: &Path) -> serde_json::Value {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(
        path.is_absolute()
            && metadata.is_file()
            && metadata.uid() == fs::metadata("/proc/self").unwrap().uid()
    );
    assert!(metadata.len() <= RECORD_MAX_BYTES as u64);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn write_record(path: &Path, value: &serde_json::Value) {
    use std::os::unix::fs::OpenOptionsExt;
    let bytes = serde_json::to_vec(value).unwrap();
    assert!(bytes.len() <= RECORD_MAX_BYTES);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(path)
        .unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
}
pub(super) fn selected_phase() -> String {
    let phase = std::env::var("TOS_NATIVE_CORPUS_PHASE").unwrap_or_else(|_| "all".into());
    assert!(matches!(
        phase.as_str(),
        "all"
            | "prepare"
            | "producer-prepare"
            | "consumer"
            | "mcp"
            | "http"
            | "producer"
            | "check"
            | "producer-read"
    ));
    phase
}
pub(super) fn resume_selected_phase() -> bool {
    let phase = selected_phase();
    if matches!(phase.as_str(), "all" | "prepare") {
        return false;
    }
    let path = PathBuf::from(
        std::env::var_os("TOS_NATIVE_CORPUS_CHECKPOINT")
            .expect("resume requires the completed preparation checkpoint"),
    );
    let sha = std::env::var("TOS_NATIVE_CORPUS_CHECKPOINT_SHA256")
        .expect("resume requires its exact checkpoint digest");
    assert_eq!(hash_file(&path).to_hex(), sha);
    let case = read_record(&path);
    assert_eq!(case["preparation_complete"], true);
    verify_pins(&case["inputs"]);
    if phase == "producer-prepare" {
        let parent = path.parent().unwrap();
        let temporary = tempfile::Builder::new()
            .prefix("producer-inputs-")
            .tempdir_in(parent)
            .unwrap();
        let repository = super::super::validation_cut_cases::repository()
            .canonicalize()
            .unwrap();
        let producer = prepare_producer_source(
            &case["producer"],
            &repository,
            &temporary.path().join("source-store"),
        );
        let prepared = prepared_record(case["managed"].clone(), producer);
        let output = parent.join(format!(
            "producer-prepared-{}.json",
            Digest256::of_bytes(&canonical(&prepared)).to_hex()
        ));
        assert!(
            !output.exists(),
            "completed producer preparation is immutable"
        );
        write_record(&output, &prepared);
        temporary.keep();
        eprintln!(
            "native corpus checkpoint={} sha256={}",
            output.display(),
            hash_file(&output).to_hex()
        );
        eprintln!("native corpus completed phase=producer-prepare");
    } else {
        run_phase(&case, &phase, path.parent().unwrap());
    }
    verify_pins(&case["inputs"]);
    true
}
pub(super) fn finish_preparation(
    managed: serde_json::Value,
    producer: serde_json::Value,
    default_root: &Path,
) -> (serde_json::Value, PathBuf, bool) {
    let selected = std::env::var_os("TOS_NATIVE_CORPUS_CHECKPOINT").map(PathBuf::from);
    let path = selected
        .clone()
        .unwrap_or_else(|| default_root.join("prepared.json"));
    assert!(path.is_absolute() && !path.exists());
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent).unwrap();
    let case = prepared_record(managed, producer);
    write_record(&path, &case);
    eprintln!(
        "native corpus checkpoint={} sha256={}",
        path.display(),
        hash_file(&path).to_hex()
    );
    (case, parent.to_owned(), selected.is_some())
}
fn prepared_record(managed: serde_json::Value, producer: serde_json::Value) -> serde_json::Value {
    let mut inputs = Vec::new();
    pin_tree(Path::new(managed["data"].as_str().unwrap()), &mut inputs);
    let source = &producer["request"]["source_only"];
    for key in [
        "corpus_store",
        "software_capture",
        "software_restored_root",
        "schema_worker_absolute_path",
    ] {
        pin_tree(Path::new(source[key].as_str().unwrap()), &mut inputs);
    }
    let case = serde_json::json!({"preparation_complete":true,"managed":managed,"producer":producer,"inputs":inputs});
    verify_pins(&case["inputs"]);
    case
}
pub(super) fn run_phase(case: &serde_json::Value, phase: &str, root: &Path) {
    verify_pins(&case["inputs"]);
    match phase {
        "prepare" => (),
        "all" => {
            for stage in ["consumer", "producer", "check", "producer-read"] {
                run_phase(case, stage, root);
            }
        }
        "consumer" | "mcp" | "http" => exercise_managed_native_corpus(&case["managed"], phase),
        "producer" | "check" | "producer-read" => {
            exercise_native_corpus_build(&case["producer"], phase, root)
        }
        _ => unreachable!(),
    }
    verify_pins(&case["inputs"]);
    eprintln!("native corpus completed phase={phase}");
}
pub(super) fn preflight(cold: tos_compiler::ColdOpenLimits) {
    let owner = PathBuf::from(
        std::env::var_os("TOS_NATIVE_OWNER_COMMAND_BIN").expect("native producer required"),
    );
    let consumer = PathBuf::from(
        std::env::var_os("TOS_NATIVE_MANAGED_CONSUMER_BIN").expect("native consumer required"),
    );
    let worker = super::super::validation_cut_cases::selected_worker_path();
    let mut errors = Vec::new();
    for (name, path) in [
        ("producer", &owner),
        ("consumer", &consumer),
        ("worker", &worker),
    ] {
        if !path.is_absolute() || !path.is_file() {
            errors.push(format!("{name}: absolute executable absent"));
        }
    }
    // A tiny existing completed fixture checks the actual fs-verity owner
    // route on TMPDIR before the expensive corpus recipe is prepared.
    let probe = tos_compiler::knowledge_full_fixture::build_fixture();
    if let Err(error) =
        tos_compiler::prepare_native_knowledge_artifact(&probe.path, &probe.stage_receipt)
    {
        errors.push(format!("filesystem custody: {error}"));
    }
    if owner.is_absolute() && owner.is_file() && worker.is_absolute() && worker.is_file() {
        let worker_source = "rust/crates/tos-validation/src/bin/tos-schema-worker.rs";
        // Preflight validates shapes and live host resources only. These
        // explicitly unopened future source paths are never admission evidence.
        let source = serde_json::json!({"corpus_store":"/preflight/unopened-source",
            "source_revision":"0".repeat(64),"software_capture":"/preflight/unopened-capture",
            "software_restored_root":"/preflight/unopened-restored","source_git_commit":"0".repeat(40),
            "source_git_tree":"0".repeat(40),"capture_manifest_sha256":"0".repeat(64),
            "software_components":[VOCABULARY_PATH,worker_source],
            "schema_worker_path":worker_source,"schema_worker_absolute_path":worker,
            "schema_worker_sha256":hash_file(&worker).to_hex()});
        let request = build_request(cold, source);
        let checked = run_native_corpus_build(&owner, &request, true);
        if !checked.status.success() {
            errors.push(format!(
                "producer admission: {}",
                String::from_utf8_lossy(&checked.stderr)
            ));
        }
    }
    assert!(
        errors.is_empty(),
        "native corpus preflight: {}",
        errors.join("; ")
    );
}
