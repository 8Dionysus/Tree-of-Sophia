//! Read continuation of the actual Claim publication case. The caller supplies
//! its committed database and receipt binding; this module never builds rows,
//! copies a database, issues authority, or substitutes an independent fixture.
use std::{
    io::Cursor,
    path::Path,
    time::{Duration, Instant},
};

#[path = "../../../rust/crates/tos-access/tests/support/native_child.rs"]
mod native_child;

use serde_json::{Value, json};
use tos_access::{cli, http::handle_get, mcp::run_io, prepared_local};

fn segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(out, "%{byte:02X}").unwrap();
        }
    }
    out
}

fn remaining(deadline: Instant) -> Duration {
    let left = deadline
        .checked_duration_since(Instant::now())
        .expect("whole Claim access deadline elapsed");
    assert!(!left.is_zero(), "whole Claim access deadline elapsed");
    left.min(Duration::from_secs(5))
}
fn profile_until(deadline: Instant) -> tos_access::AccessProfile {
    prepared_local::profile().with_query_timeout(remaining(deadline))
}

fn read_three_wires(
    executor: &prepared_local::PreparedLocalExecutor,
    args: &[String],
    path: &str,
    tool: &str,
    arguments: Value,
    deadline: Instant,
) -> Value {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(
        cli::run_cli(
            args,
            executor,
            profile_until(deadline),
            &mut stdout,
            &mut stderr
        ),
        0,
        "committed Claim CLI: {}",
        String::from_utf8_lossy(&stderr)
    );
    let expected: Value = serde_json::from_slice(&stdout).unwrap();
    let http = handle_get(executor, "GET", path, profile_until(deadline));
    assert_eq!(http.status, 200, "{}", String::from_utf8_lossy(&http.body));
    let mut wire = Vec::new();
    tos_access::http::write_response(&mut wire, http).unwrap();
    assert!(
        wire.starts_with(b"HTTP/1.1 200 "),
        "{}",
        String::from_utf8_lossy(&wire)
    );
    let body = wire
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap()
        + 4;
    assert_eq!(
        expected,
        serde_json::from_slice::<Value>(&wire[body..]).unwrap()
    );
    remaining(deadline);

    let mut input = Vec::new();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-11-25"}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":tool,"arguments":arguments}}),
    ] {
        serde_json::to_writer(&mut input, &message).unwrap();
        input.push(b'\n');
    }
    let mut output = Vec::new();
    run_io(
        Cursor::new(input),
        &mut output,
        executor,
        profile_until(deadline),
    )
    .unwrap();
    let replies: Vec<Value> = output
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[1]["id"], 2);
    assert!(replies[1].get("error").is_none(), "{}", replies[1]);
    assert_ne!(replies[1]["result"]["isError"], true);
    assert_eq!(expected, replies[1]["result"]["structuredContent"]);
    remaining(deadline);
    expected
}

/// Installed-only continuation. The real server owns SoftwareSite selection;
/// this observer compares HTTP bytes with its immutable installed declaration.
fn verify_installed_site(
    binary: &Path,
    database: &Path,
    receipt_binding: &Path,
    node_id: &str,
    deadline: Instant,
) {
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        os::unix::fs::MetadataExt,
        process::{Command, Stdio},
    };
    use tos_foundation::Digest256;
    const STATIC_PREFIX: &str = "access/src/tos_access/web_dist/";
    const ASSET_BYTES: usize = 16 * 1024 * 1024;
    const HEADER_BYTES: usize = 64 * 1024;
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let executable = binary.canonicalize().unwrap();
    let software_root = executable
        .ancestors()
        .nth(4)
        .expect("installed native layout");
    assert_eq!(
        executable.strip_prefix(software_root).unwrap(),
        Path::new("access/src/tos_access/tos-access")
    );
    let manifest_path = software_root.join("software.manifest.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).unwrap();
    assert!(manifest_metadata.is_file() && !manifest_metadata.file_type().is_symlink());
    assert!(manifest_metadata.len() <= 1_048_576);
    let mut manifest_raw = Vec::new();
    fs::File::open(&manifest_path)
        .unwrap()
        .take(1_048_577)
        .read_to_end(&mut manifest_raw)
        .unwrap();
    assert!(manifest_raw.len() <= 1_048_576);
    let manifest_hash = Digest256::of_bytes(&manifest_raw);
    let manifest: Value = serde_json::from_slice(&manifest_raw).unwrap();
    assert_eq!(
        manifest["schema_version"],
        "tos_software_bundle_manifest_v1"
    );
    assert_eq!(manifest["data_included"], false);
    let members = manifest["members"].as_array().unwrap();
    let mut assets = Vec::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for member in members {
        let path = member["path"].as_str().unwrap();
        if let Some(relative) = path.strip_prefix(STATIC_PREFIX) {
            tos_foundation::RelativePath::parse(relative).unwrap();
            assert!(
                paths.insert(relative.to_owned()),
                "duplicate installed static declaration"
            );
            let bytes = member["size_bytes"].as_u64().unwrap();
            assert!(bytes <= ASSET_BYTES as u64 && assets.len() < 512);
            total = total
                .checked_add(bytes)
                .filter(|n| *n <= 64 * 1024 * 1024)
                .unwrap();
            assets.push((
                relative.to_owned(),
                bytes,
                member["sha256"].as_str().unwrap().to_owned(),
            ));
        }
    }
    assert!(paths.contains("assets/tos-graph.js") && paths.contains("assets/tos-graph.css"));
    assert!(
        paths.iter().any(|p| p.ends_with(".wasm")),
        "installed WASM not declared"
    );
    drop(manifest_raw);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut server = native_child::OwnedChild(
        Command::new(binary)
            .arg("--prepared-read-model")
            .arg(database)
            .arg("--prepared-binding")
            .arg(receipt_binding)
            .arg("serve")
            .arg(address.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let startup = deadline.min(Instant::now() + Duration::from_secs(15));
    let first = loop {
        assert!(
            Instant::now() < startup,
            "installed SoftwareSite startup deadline"
        );
        assert!(
            server.try_wait().unwrap().is_none(),
            "installed SoftwareSite exited before listening"
        );
        match TcpStream::connect_timeout(&address, remaining(startup)) {
            Ok(stream) => break stream,
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let http = |mut stream: TcpStream, path: &str, maximum: usize| {
        assert!(path.starts_with('/') && !path.contains('\r') && !path.contains('\n'));
        stream.set_write_timeout(Some(remaining(deadline))).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let limit = maximum.checked_add(HEADER_BYTES).unwrap();
        let mut response = Vec::with_capacity(limit);
        let mut buffer = [0u8; 65536];
        loop {
            stream.set_read_timeout(Some(remaining(deadline))).unwrap();
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            assert!(
                response
                    .len()
                    .checked_add(count)
                    .is_some_and(|n| n <= limit)
            );
            response.extend_from_slice(&buffer[..count]);
        }
        assert!(
            response.starts_with(b"HTTP/1.1 200 "),
            "installed HTTP refusal: {}",
            String::from_utf8_lossy(&response[..response.len().min(1024)])
        );
        let start = response.windows(4).position(|p| p == b"\r\n\r\n").unwrap() + 4;
        assert!(start <= HEADER_BYTES && response.len() - start <= maximum);
        let header = std::str::from_utf8(&response[..start]).unwrap();
        let content_length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .trim()
            .parse::<usize>()
            .unwrap();
        assert_eq!(content_length, response.len() - start);
        response.drain(..start);
        response
    };
    let index = http(first, "/", 1_048_576);
    let html = std::str::from_utf8(&index).unwrap();
    assert!(
        html.contains("window.__TOS_GRAPH_BOOT__=")
            && html.contains("/static/assets/tos-graph.js")
            && html.contains("/static/assets/tos-graph.css")
    );
    drop(index);
    let node = http(
        TcpStream::connect_timeout(&address, remaining(deadline)).unwrap(),
        &format!("/api/knowledge/nodes/{}", segment(node_id)),
        prepared_local::PREPARED_RESPONSE_BYTES,
    );
    let node: Value = serde_json::from_slice(&node).unwrap();
    assert!(
        node["matches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == node_id)
    );
    drop(node);
    for (path, bytes, sha) in assets {
        let route = format!(
            "/static/{}",
            path.split('/').map(segment).collect::<Vec<_>>().join("/")
        );
        let body = http(
            TcpStream::connect_timeout(&address, remaining(deadline)).unwrap(),
            &route,
            bytes as usize,
        );
        assert_eq!(body.len() as u64, bytes);
        assert_eq!(
            Digest256::of_bytes(&body).to_hex(),
            sha,
            "installed static HTTP digest {path}"
        );
        if path.ends_with(".wasm") {
            assert!(body.starts_with(b"\0asm"));
        }
    }
    server.kill().unwrap();
    server.wait().unwrap();
    remaining(deadline);
    let after = fs::symlink_metadata(&manifest_path).unwrap();
    assert_eq!(
        (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        ),
        (
            manifest_metadata.dev(),
            manifest_metadata.ino(),
            manifest_metadata.len(),
            manifest_metadata.mtime(),
            manifest_metadata.mtime_nsec(),
            manifest_metadata.ctime(),
            manifest_metadata.ctime_nsec()
        )
    );
    assert_eq!(
        native_child::bounded_sha_before(&manifest_path, 1_048_576, deadline),
        manifest_hash
    );
}

/// Invoke only after the native publication's guarded commit. Both identifiers
/// must come from its normalized addition, not from the predecessor fixture.
/// The same caller deadline covers setup, hashing and every transport/child.
pub(super) fn verify_published_access_until(
    database: &Path,
    receipt_binding: &Path,
    expected_node_id: &str,
    expected_relation_id: &str,
    deadline: Instant,
) {
    // OPS selects and protects the coherent access product (which may be an
    // installed prefix). No copy or separate data fixture is made here.
    let binary = std::path::PathBuf::from(
        std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
            .expect("exact protected access executable required"),
    );
    assert!(binary.is_absolute());
    let expected_sha = std::env::var("TOS_NATIVE_PREPARED_CONSUMER_SHA256").unwrap();
    assert_eq!(
        native_child::bounded_sha_before(&binary, 512 * 1024 * 1024, deadline).to_hex(),
        expected_sha
    );
    let actual = |args: &[String], expected: &Value| {
        let child_deadline = Instant::now() + remaining(deadline);
        let output = native_child::bounded_output_before(
            std::process::Command::new(&binary)
                .arg("--prepared-read-model")
                .arg(database)
                .arg("--prepared-binding")
                .arg(receipt_binding)
                .args(args),
            prepared_local::PREPARED_RESPONSE_BYTES,
            child_deadline.min(deadline),
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            *expected,
            serde_json::from_slice::<Value>(&output.stdout).unwrap()
        );
    };
    {
        remaining(deadline);
        let executor = prepared_local::PreparedLocalExecutor::open(
            database.to_owned(),
            receipt_binding.to_owned(),
            None,
        )
        .unwrap();
        for (kind, identifier, argument, tool) in [
            ("node", expected_node_id, "node_id", "tos_knowledge_node"),
            (
                "relation",
                expected_relation_id,
                "relation_id",
                "tos_knowledge_relation",
            ),
        ] {
            let args = ["knowledge".into(), kind.into(), identifier.into()];
            let packet = read_three_wires(
                &executor,
                &args,
                &format!("/api/knowledge/{kind}s/{}", segment(identifier)),
                tool,
                json!({argument: identifier}),
                deadline,
            );
            assert!(
                packet["matches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == identifier),
                "newly published {kind} {identifier} absent: {packet}"
            );
            actual(&args, &packet);
        }
        let args = ["knowledge".into(), "catalog".into()];
        let catalog = read_three_wires(
            &executor,
            &args,
            "/api/knowledge/catalog",
            "tos_knowledge_catalog",
            json!({}),
            deadline,
        );
        drop(executor);
        actual(&args, &catalog);
        remaining(deadline);
    }
    if std::env::var_os("TOS_NATIVE_INSTALLED_SOFTWARE_SITE").as_deref()
        == Some(std::ffi::OsStr::new("1"))
    {
        verify_installed_site(
            &binary,
            database,
            receipt_binding,
            expected_node_id,
            deadline,
        );
        assert_eq!(
            native_child::bounded_sha_before(&binary, 512 * 1024 * 1024, deadline).to_hex(),
            expected_sha
        );
    }
}
