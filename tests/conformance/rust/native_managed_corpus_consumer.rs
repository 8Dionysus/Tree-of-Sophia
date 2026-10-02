//! Private installed continuation of the existing actual native corpus caller.
//! The separate cold/domain phase does not enter this required Linux host gate.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
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
fn canonical(value: &serde_json::Value) -> Vec<u8> {
    let raw = serde_json::to_vec(value).unwrap();
    let doc = parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    canonical_bytes_v1(
        doc.root(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
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
    fs::write(data.join("data/manifest.json"), &raw).unwrap();
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

pub(super) fn exercise_managed_native_corpus(
    selected: &FullKnowledgeFixture,
    projection: &NativeCorpusProjection,
    repository: &Path,
    output_path: &str,
    files: &BTreeMap<String, Vec<u8>>,
    packets: &[(R, JsonValue)],
) {
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
    let base = selected
        .path
        .parent()
        .unwrap()
        .join("managed-native-corpus");
    let data = base.join("snapshot");
    let root = base.join("release");
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
    let program = "scripts/tos_corpus_index_common.py";
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
    let binary_sha = hash_file(&binary);
    publish_snapshot(&root, &data, &binary, &manifest, binary_sha);
    let release = tos_access::release_state::ManagedRelease::open(&root).unwrap();
    let receipt = selected.corpus_original.as_ref().unwrap();
    let mut hold = release.acquire().unwrap();
    let (context, guards) = hold
        .admit_corpus_members(receipt, selected.cold_limits().max_work_bytes as usize)
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
    for (index, (request, oracle)) in packets.iter().enumerate() {
        let mut packet = oracle.clone();
        match request {
            R::Status => display_context(&mut packet, &data.join("data"), &output),
            R::Summary => {
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
            R::GraphViews
            | R::Search { .. }
            | R::Resources { .. }
            | R::Node { .. }
            | R::RelationPack { .. }
            | R::GraphView { .. }
            | R::Packet { .. } => {}
        }
        expected.push(
            canonical_bytes_v1(
                &packet,
                CanonicalProfile::SourceRecordDigestV1,
                JsonLimits::default(),
            )
            .unwrap(),
        );
        let route = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|row| row.operation_id == request.operation_id())
            .unwrap();
        input.push_str(&serde_json::to_string(&json!({"jsonrpc":"2.0","id":index+1,"method":"tools/call","params":{"name":route.mcp_tool,"arguments":arguments(request)}})).unwrap());
        input.push('\n');
    }
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
    for (request, _) in packets {
        let route = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|row| row.operation_id == request.operation_id())
            .unwrap();
        assert!(
            tools
                .iter()
                .any(|tool| tool.object_get("name").and_then(JsonValue::as_str)
                    == Some(route.mcp_tool.as_str())),
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
    for (index, (request, _)) in packets.iter().enumerate() {
        let route = tos_access::registered_operations()
            .unwrap()
            .iter()
            .find(|row| row.operation_id == request.operation_id())
            .unwrap();
        let Some(target) = http_target(request, &route.http_path) else {
            continue;
        };
        http_operations.insert(request.operation_id());
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
                changed["input_bindings"][output_path] = json!(proof.output_sha256)
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
        assert!(
            String::from_utf8_lossy(&refusal.stderr).contains("native corpus"),
            "{fault}: {}",
            String::from_utf8_lossy(&refusal.stderr)
        );
    }
    exclusive.try_lock().unwrap();
    publish_snapshot(&root, &data, &binary, &manifest, binary_sha);
    let record = json!({"schema_version":"tos_access_release_revocation_v1","kind":"corpus","digest":proof.source_revision,"reason":"isolated native corpus withdrawal"});
    fs::write(
        root.join(format!("revocations/corpus/{}.json", proof.source_revision)),
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
