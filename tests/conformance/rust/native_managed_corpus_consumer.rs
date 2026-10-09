//! Private installed continuation of the existing actual native corpus caller.
//! The separate cold/domain phase does not enter this required Linux host gate.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
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

fn run_native_corpus_build(binary: &Path, request: &serde_json::Value) -> Output {
    let address_space_bytes = request["process_limits"]["address_space_bytes"]
        .as_u64()
        .expect("finite owner address-space cap");
    let file_size_bytes = request["process_limits"]["file_size_bytes"]
        .as_u64()
        .expect("finite owner file-size cap");
    let mut command = Command::new("prlimit");
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
    let mut child = command.spawn().expect("admitted native owner binary starts");
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

/// Exercise the actual native source-cut producer under the already admitted
/// private-stage ticket, then verify only its disposable candidate through the
/// existing managed reader. This route never writes the production pointer.
pub(super) fn exercise_native_corpus_build(
    selected: &FullKnowledgeFixture,
    projection: &NativeCorpusProjection,
    repository: &Path,
    source_store: &Path,
    source_revision: tos_foundation::SourceRevision,
    software: &super::super::source_cut_cases::SoftwareCaptureFixture,
    worker_path: &Path,
) {
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

    // Read exact finite limits from the normal admitted ticket rather than
    // inventing a larger request envelope in this controller.
    let stage = tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_issued_from_environment()
        .expect("OPS must issue the native producer private tmpfs ticket");
    let (quota_bytes, inode_limit, working_ram_bytes) = stage.resource_limits();
    let persistent_store = stage
        .persistent_store()
        .expect("OPS must admit the candidate's bounded persistent store")
        .to_owned();
    drop(stage);

    let process_limits = tos_compiler::NativeProcessLimits {
        address_space_bytes: NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS
            .address_space_bytes
            .min(working_ram_bytes),
        file_size_bytes: NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS.file_size_bytes,
    };
    assert!(process_limits.address_space_bytes >= 512 * 1024 * 1024);
    let worker_source = "rust/crates/tos-validation/src/bin/tos-schema-worker.rs";
    let vocabulary_path = "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json";
    let worker_address_space_bytes = working_ram_bytes.min(1024 * 1024 * 1024);
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
        "software_components": [vocabulary_path, worker_source],
        "schema_worker_path": worker_source,
        "schema_worker_absolute_path": worker_path.display().to_string(),
        "schema_worker_sha256": hash_file(worker_path).to_hex(),
        "max_schema_receipts": 4096,
        "max_schema_receipt_bytes": 4 * 1024 * 1024,
        "worker_cpu_seconds": 60,
        "worker_address_space_bytes": worker_address_space_bytes,
    });
    let build_request = json!({
        "schema_version": "tos_native_corpus_build_request_v1",
        "mode": "build",
        "tmpfs_quota_bytes": quota_bytes,
        "tmpfs_inode_limit": inode_limit,
        "working_ram_bytes": working_ram_bytes,
        "max_state_bytes": 512 * 1024 * 1024,
        "max_json_visits": 8_000_000,
        "max_work_bytes": 8 * 1024 * 1024 * 1024u64,
        "persistent_write_cap_bytes": 512 * 1024 * 1024u64,
        "max_build_seconds": 600,
        "cold_open": serde_json::to_value(selected.cold_limits()).unwrap(),
        "process_limits": serde_json::to_value(process_limits).unwrap(),
        "data_directory": "native-corpus-build-candidate",
        "private_release_directory": "native-corpus-build-unused-release",
        "source_only": source_selection,
    });
    let built = run_native_corpus_build(&owner, &build_request);
    assert!(
        built.status.success(),
        "native corpus build refused: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(built.stdout.len() <= 1_048_576);
    let built: serde_json::Value = serde_json::from_slice(&built.stdout).unwrap();
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
    let declaration: serde_json::Value = serde_json::from_slice(
        &fs::read(repository.join("access/contracts/runtime-data.v1.json")).unwrap(),
    )
    .unwrap();
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
    assert_eq!(
        projection.receipt().source_revision,
        source_revision.0.to_hex(),
        "producer and existing five-class projection use the same source cut"
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

    // Check mode recomputes the same six products and is read-only. The
    // negative case uses a separate disposable copy so the candidate stays
    // intact for the reader admission below.
    let check_data_name = "native-corpus-check-unused-data";
    let check_release_name = "native-corpus-check-unused-release";
    let mut check_request = build_request.clone();
    check_request["mode"] = json!("check");
    check_request["comparison_root"] = json!(data.display().to_string());
    check_request["data_directory"] = json!(check_data_name);
    check_request["private_release_directory"] = json!(check_release_name);
    let checked = run_native_corpus_build(&owner, &check_request);
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
        assert_eq!(Digest256::of_bytes(&raw).to_hex(), row["sha256"].as_str().unwrap());
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
    let refused = run_native_corpus_build(&owner, &negative_request);
    assert!(!refused.status.success(), "changed product must fail parity");
    assert!(refused.stdout.is_empty());
    assert!(!persistent_store.join(check_data_name).exists());
    assert!(!persistent_store.join(check_release_name).exists());

    // Admit the disposable output through the current Rust native reader. This
    // pair lives only under the test TempDir, never at an installed current
    // pointer or production release root.
    let selection_path = data.join("native-selection.json");
    let selection_raw = fs::read(&selection_path).unwrap();
    let descriptor = fs::read(data.join(vocabulary_path)).unwrap();
    let entities = fs::read(data.join("ToS/doctrine/semantic-interchange/entity-types.v1.json"))
        .unwrap();
    let relations = fs::read(data.join("ToS/doctrine/semantic-interchange/relation-types.v1.json"))
        .unwrap();
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
    let base = software.temporary.path().join("native-corpus-build-readback");
    fs::create_dir(&base).unwrap();
    let release_root = base.join("release");
    publish_snapshot(&release_root, data_root, &consumer, &manifest, hash_file(&consumer));
    assert_eq!(
        hash_file(&data.join("manifest.json")).to_hex(),
        built["data_manifest"]["sha256"].as_str().unwrap(),
        "fixture-only release admission must preserve the native writer manifest"
    );
    let release = tos_access::release_state::ManagedRelease::open(&release_root).unwrap();
    let mut hold = release.acquire().unwrap();
    let (context, guards) = hold
        .admit_corpus_members(receipt, selected.cold_limits().max_work_bytes as usize)
        .unwrap();
    assert_eq!(context.index_path, corpus_file.to_str().unwrap());
    assert_eq!(context.tos_root, data.to_str().unwrap());
    hold.retain_member_guards(&guards).unwrap();
    hold.recheck().unwrap();
}
