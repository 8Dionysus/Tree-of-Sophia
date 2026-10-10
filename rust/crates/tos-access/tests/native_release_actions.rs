#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, Write},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::AtomicBool,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, canonical_bytes_v1,
    native_software_roles, parse_json,
};
use zip::{ZipWriter, write::SimpleFileOptions};

const CLI: &str = env!("CARGO_BIN_EXE_tos-access");
// The selected relation registry is over 256 KiB; all vocabulary members
// must fit the same explicit metadata envelope used by release preparation.
const MAX_METADATA: usize = 524_288;
const MAX_ARCHIVE_MEMBERS: usize = native_software_roles::COMMANDS.len() + 6;

struct Scratch(PathBuf);
impl Scratch {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tos-native-release-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        // Cargo's bin and deps entries may be hard links to one inode. Release
        // preparation requires custody of a single-link running image, just
        // like the independently extracted executable used after installation.
        let program = path.join("release-tool");
        fs::copy(CLI, &program).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn canonical(value: &serde_json::Value) -> Vec<u8> {
    let raw = serde_json::to_vec(value).unwrap();
    let document = parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    canonical_bytes_v1(
        &document.into_root(),
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
    )
    .unwrap()
}

fn number(value: usize) -> serde_json::Value {
    serde_json::json!(value)
}

fn write_private(path: &Path, raw: &[u8]) {
    fs::write(path, raw).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn create_archive(path: &Path, marker: &str) -> String {
    let binary = fs::read(CLI).unwrap();
    let lock = include_bytes!("../../../../Cargo.lock").to_vec();
    let pin = include_bytes!("../../../../rust-toolchain.toml").to_vec();
    // Pair admission includes the complete shared delivery descriptor.
    // These integrity-only command images are deliberately never executed;
    // the real Access image runs installation, promotion, rollback and status.
    // Genuine command execution is covered by the installed conformance route.
    let mut files: BTreeMap<String, Vec<u8>> = [
        ("Cargo.lock", lock),
        ("access/src/tos_access/tos-access", binary),
        (
            "access/src/tos_access/web_dist/assets/tos-graph.css",
            format!("body{{margin:0}}/*{marker}*/\n").into_bytes(),
        ),
        (
            "access/src/tos_access/web_dist/assets/tos-graph.js",
            b"export const softwareOwned=true;\n".to_vec(),
        ),
        ("rust-toolchain.toml", pin),
    ]
    .into_iter()
    .map(|(path, bytes)| (path.to_owned(), bytes))
    .collect();
    for role in native_software_roles::COMMANDS {
        let mut image = vec![0u8; 64];
        image[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        image[18..20].copy_from_slice(&[0x3e, 0]);
        image.extend_from_slice(role.as_bytes());
        assert!(files.insert(format!("native/bin/{role}"), image).is_none());
    }
    let source = Digest256::of_bytes(b"native release action integration fixture").to_hex();
    let pin_text = std::str::from_utf8(&files["rust-toolchain.toml"]).unwrap();
    let toolchain = pin_text
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("channel = ")
                .and_then(|raw| raw.strip_prefix('"'))
                .and_then(|raw| raw.strip_suffix('"'))
        })
        .unwrap();
    let member_rows = files
        .iter()
        .map(|(name, bytes)| {
            serde_json::json!({
                "path": name,
                "size_bytes": bytes.len(),
                "sha256": Digest256::of_bytes(bytes).to_hex(),
            })
        })
        .collect::<Vec<_>>();
    let lock = &files["Cargo.lock"];
    let image = &files["access/src/tos_access/tos-access"];
    let native_commands: serde_json::Map<String, serde_json::Value> = native_software_roles::COMMANDS.iter().map(|role| {
        let image = &files[&format!("native/bin/{role}")];
        ((*role).to_owned(), serde_json::json!({
            "schema_version":"tos_native_software_command_build_v1", "target":"x86_64-unknown-linux-gnu",
            "source_commit":source, "source_tree":source, "profile":"debug",
            "lock_sha256":Digest256::of_bytes(lock).to_hex(), "toolchain":toolchain,
            "features":native_software_roles::features(role).unwrap(),
            "sha256":Digest256::of_bytes(image).to_hex(), "size_bytes":image.len(),
        }))
    }).collect();
    let embedded = serde_json::json!({
        "schema_version":"tos_software_bundle_manifest_v1",
        "software_ref":source,
        "data_included":false,
        "source_dirty":false,
        "native_access":{
            "schema_version":"tos_native_access_build_v1",
            "target":"x86_64-unknown-linux-gnu",
            "source_commit":source,
            "source_tree":source,
            "profile":"debug",
            "lock_sha256":Digest256::of_bytes(lock).to_hex(),
            "toolchain":toolchain,
            "sha256":Digest256::of_bytes(image).to_hex(),
            "size_bytes":image.len(),
        },
        "native_commands":native_commands,
        "members":member_rows,
    });
    let mut writer = ZipWriter::new(
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap(),
    );
    for (name, bytes) in &files {
        writer
            .start_file(
                name.as_str(),
                SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored)
                    .unix_permissions(
                        if name == "access/src/tos_access/tos-access"
                            || name.starts_with("native/bin/")
                        {
                            0o755
                        } else {
                            0o644
                        },
                    ),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer
        .start_file(
            "software.manifest.json",
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(0o644),
        )
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(&embedded).unwrap())
        .unwrap();
    let mut archive = writer.finish().unwrap();
    archive
        .set_permissions(fs::Permissions::from_mode(0o600))
        .unwrap();
    archive.sync_all().unwrap();
    let size = archive.metadata().unwrap().len();
    archive.rewind().unwrap();
    let mut hasher = Digest256Hasher::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let read = archive.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize().to_hex();
    let mut sidecar = embedded;
    sidecar
        .as_object_mut()
        .unwrap()
        .insert("archive_size_bytes".into(), number(size as usize));
    sidecar
        .as_object_mut()
        .unwrap()
        .insert("archive_sha256".into(), serde_json::json!(digest));
    let mut sidecar_path = path.as_os_str().to_os_string();
    sidecar_path.push(".manifest.json");
    write_private(
        &PathBuf::from(sidecar_path),
        &serde_json::to_vec(&sidecar).unwrap(),
    );
    digest
}

fn write_snapshot(
    root: &Path,
    fixture: &tos_compiler::knowledge_full_fixture::FullKnowledgeFixture,
    corpus_root: &Path,
    query_schema: &str,
    corrupt_model: bool,
    fs_verity: bool,
    process_limits: tos_compiler::NativeProcessLimits,
) -> (String, String) {
    fs::create_dir(root).unwrap();
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    let data = root.join("data");
    fs::create_dir(&data).unwrap();
    fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
    let model_path = data.join("model.sqlite");
    fs::copy(&fixture.path, &model_path).unwrap();
    // Candidate custody is explicit and cannot depend on the CI runner umask.
    fs::set_permissions(&model_path, fs::Permissions::from_mode(0o600)).unwrap();
    let measurement = if fs_verity {
        tos_compiler::prepare_native_knowledge_artifact(&model_path, &fixture.stage_receipt)
            .unwrap()
    } else {
        tos_compiler::NativeFsVerityMeasurement {
            algorithm: "sha256".into(),
            digest: fixture.expectation.model_sha256.clone(),
        }
    };
    let descriptor = fixture.descriptor_bytes.clone();
    let entity = fixture.entity_registry_bytes().to_vec();
    let relation = fixture.relation_registry_bytes().to_vec();
    write_private(&data.join("descriptor.json"), &descriptor);
    write_private(&data.join("entity.json"), &entity);
    write_private(&data.join("relation.json"), &relation);
    let selection = tos_compiler::NativeKnowledgeSelection::from_producer(
        tos_compiler::NativeSelectionPaths {
            model: "data/model.sqlite".into(),
            descriptor: "data/descriptor.json".into(),
            entity_registry: "data/entity.json".into(),
            relation_registry: "data/relation.json".into(),
        },
        tos_compiler::NativeSelectionProducer {
            stage: fixture.stage_receipt.clone(),
            seal: fixture.seal_receipt.clone(),
            navigation_original: fixture.navigation_original.clone(),
            philosophy_original: fixture.philosophy_original.clone(),
            corpus_original: fixture.corpus_original.clone(),
            managed_source: None,
            managed_source_v2: None,
        },
        fixture.expectation.clone(),
        measurement,
        fixture.cold_limits(),
        process_limits,
        &descriptor,
        &entity,
        &relation,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
        MAX_METADATA,
    )
    .unwrap();
    let selection = selection.encode(MAX_METADATA).unwrap();
    write_private(&data.join("native-selection.json"), &selection);
    let mut members: BTreeMap<String, Vec<u8>> = [
        ("data/descriptor.json", descriptor),
        ("data/entity.json", entity),
        ("data/model.sqlite", fs::read(&model_path).unwrap()),
        ("data/native-selection.json", selection.clone()),
        ("data/relation.json", relation),
    ]
    .into_iter()
    .map(|(name, bytes)| (name.to_owned(), bytes))
    .collect();
    let source_declaration = include_bytes!("../../../../access/contracts/runtime-data.v1.json");
    let compiler_program =
        include_bytes!("../../../../rust/crates/tos-compiler/src/source_corpus.rs");
    let source_binding = Digest256::of_bytes(source_declaration).to_hex();
    let compiler_binding = Digest256::of_bytes(compiler_program).to_hex();
    let mut input_bindings = BTreeMap::from([(
        "access/contracts/runtime-data.v1.json".to_owned(),
        source_binding,
    )]);
    // Carry the complete captured corpus closure and its exact source bindings
    // alongside the model. Promotion must verify the same real origin receipt.
    for member in &fixture.corpus_original.as_ref().unwrap().origin.members {
        let raw = fs::read(corpus_root.join(&member.path)).unwrap();
        assert_eq!(raw.len() as u64, member.size_bytes);
        assert_eq!(Digest256::of_bytes(&raw).to_hex(), member.sha256);
        let selected = format!("data/{}", member.path);
        let selected_path = root.join(&selected);
        let parent = selected_path.parent().unwrap();
        fs::create_dir_all(parent).unwrap();
        for directory in parent
            .ancestors()
            .take_while(|path| path.starts_with(&data))
        {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        write_private(&root.join(&selected), &raw);
        assert!(members.insert(selected, raw).is_none());
        assert!(
            input_bindings
                .insert(member.path.clone(), member.sha256.clone())
                .is_none()
        );
    }
    // Bind the same verified source revision carried by the complete model.
    let corpus_revision = fixture
        .open()
        .unwrap()
        .source_revision()
        .unwrap()
        .to_owned();
    let mut manifest = serde_json::json!({
        "schema_version":"tos_access_native_data_snapshot_v1",
        "corpus_revision":corpus_revision,
        "input_bindings":input_bindings,
        "compiler":{
            "schema":query_schema,
            "compiler_version":tos_compiler::COMPILER_VERSION,
            "compiler_sha256":Digest256::of_bytes(b"fixture compiler").to_hex(),
            "compiler_paths":["rust/crates/tos-compiler/src/source_corpus.rs"],
            "input_bindings":{"rust/crates/tos-compiler/src/source_corpus.rs":compiler_binding},
        },
        "members":members.iter().map(|(path, raw)| serde_json::json!({
            "path":path,
            "size_bytes":raw.len(),
            "sha256":Digest256::of_bytes(raw).to_hex(),
        })).collect::<Vec<_>>(),
        "native_selection":"data/native-selection.json",
    });
    let revision = Digest256::of_bytes(&canonical(&manifest)).to_hex();
    manifest
        .as_object_mut()
        .unwrap()
        .insert("data_revision".into(), serde_json::json!(revision));
    let manifest_raw = canonical(&manifest);
    write_private(&data.join("manifest.json"), &manifest_raw);
    if corrupt_model {
        fs::write(&model_path, b"corrupt after authenticated member census").unwrap();
    }
    (
        Digest256::of_bytes(&manifest_raw).to_hex(),
        Digest256::of_bytes(&selection).to_hex(),
    )
}

fn deadline_ns() -> u64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) },
        0
    );
    (now.tv_sec as u64) * 1_000_000_000 + now.tv_nsec as u64 + 120_000_000_000
}

fn bounded_command_output(mut command: Command) -> Output {
    // The verified synthetic selection itself declares these finite owner limits.
    unsafe {
        command.pre_exec(|| {
            let address = libc::rlimit {
                rlim_cur: 2 * 1024 * 1024 * 1024,
                rlim_max: 2 * 1024 * 1024 * 1024,
            };
            let file_size = libc::rlimit {
                rlim_cur: 512 * 1024 * 1024,
                rlim_max: 512 * 1024 * 1024,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &address) != 0
                || libc::setrlimit(libc::RLIMIT_FSIZE, &file_size) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.output().unwrap()
}

fn install_archive(archive: &Path, prefix: &Path) -> Output {
    let mut command = Command::new(CLI);
    command
        .args(["software", "install", "--archive"])
        .arg(archive)
        .arg("--prefix")
        .arg(prefix)
        .args([
            "--max-total-bytes",
            "536870912",
            "--max-archive-bytes",
            "536870912",
            "--max-members",
            &MAX_ARCHIVE_MEMBERS.to_string(),
            "--max-metadata-bytes",
            "262144",
        ]);
    bounded_command_output(command)
}

fn action(action: &str, root: &Path, value: serde_json::Value) -> Output {
    action_with(&root.join("release-tool"), action, root, value)
}

fn action_with(program: &Path, action: &str, root: &Path, value: serde_json::Value) -> Output {
    let request_path = root.join(format!("{action}.request.json"));
    let request = canonical(&value);
    write_private(&request_path, &request);
    let deadline = value["work_deadline_ns"].as_u64().unwrap();
    let request_sha = Digest256::of_bytes(&request).to_hex();
    let deadline = deadline.to_string();
    let mut command = Command::new(program);
    command.args([
        action,
        "--request",
        request_path.to_str().unwrap(),
        "--request-sha256",
        &request_sha,
        "--work-deadline-ns",
        &deadline,
    ]);
    bounded_command_output(command)
}

fn action_limits() -> serde_json::Value {
    serde_json::json!({
        "max_metadata_bytes":MAX_METADATA,
        "max_state_bytes":536_870_912usize,
        "max_io_bytes":8_000_000_000u64,
        "max_installed_io_bytes":4_000_000_000u64,
        "max_installed_state_bytes":268_435_456usize,
        "max_held_fds":64,
        "max_archive_bytes":536_870_912u64,
        "max_archive_expanded_bytes":536_870_912u64,
        "max_archive_members":MAX_ARCHIVE_MEMBERS,
        "max_image_bytes":536_870_912u64,
        "max_candidate_bytes":67_108_864u64,
        "max_candidate_members":32,
    })
}

fn prepare_request(archive: &Path, data_root: &Path, deadline: u64) -> serde_json::Value {
    serde_json::json!({
        "schema_version":"tos_access_native_release_prepare_request_v1",
        "work_deadline_ns":deadline,
        "software_archive":archive,
        "data_root":data_root,
        "limits":action_limits(),
    })
}

fn promotion_request(
    receipt: &serde_json::Value,
    release: &Path,
    prefix: &Path,
    expected_current: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version":"tos_access_native_release_promote_request_v1",
        "work_deadline_ns":deadline_ns(),
        "release_root":release,
        "software_prefix":prefix,
        "candidate_pair":receipt["pair"],
        "bindings":receipt["bindings"],
        "expected_current":expected_current,
        "candidate_manifest_sha256":receipt["candidate_manifest_sha256"],
        "candidate_selection_sha256":receipt["candidate_selection_sha256"],
        "limits":action_limits(),
    })
}

fn rollback_request(release: &Path, prefix: &Path, expected_current: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version":"tos_access_native_release_rollback_request_v1",
        "work_deadline_ns":deadline_ns(),
        "release_root":release,
        "software_prefix":prefix,
        "expected_current":expected_current,
        "limits":action_limits(),
    })
}

fn revoke_request(release: &Path, kind: &str, digest: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version":"tos_access_native_release_revoke_request_v1",
        "work_deadline_ns":deadline_ns(),
        "release_root":release,
        "kind":kind,
        "digest":digest,
        "reason":"superseded synthetic release candidate",
        "owner_ref":"test:native-release-actions",
    })
}

fn native_action_process_limits() -> tos_compiler::NativeProcessLimits {
    tos_compiler::NativeProcessLimits {
        address_space_bytes: 2 * 1024 * 1024 * 1024,
        file_size_bytes: 512 * 1024 * 1024,
    }
}

fn capture_release_corpus(root: &Path) -> (tos_source_store::SoftwareCaptureReader, PathBuf) {
    use tos_source_store::{
        CaptureGitRequest, CaptureRestoreLimits, GitCaptureLimits, ReadLimits,
        SoftwareCaptureSelectionV1, capture_git, restore_capture,
    };
    const SOURCE: &str = "ToS/derived-exports/tos_corpus_index.min.json";
    let source = root.join("corpus-source");
    fs::create_dir_all(source.join("ToS/derived-exports")).unwrap();
    let index = serde_json::json!({
        "schema_version":"tos_corpus_index_v1","owner_repo":"Tree-of-Sophia","surface_kind":"derived",
        "counts":{"nodes":2,"relation_edges":1,"relation_packs":1},
        "nodes":[{"node_id":"a","label":"Alpha","source_ref":"ToS/canon/a.json"},{"node_id":"b","label":"Beta","source_ref":"ToS/canon/b.json"}],
        "resources":[],"manifests":[],"branches":[],
        "relation_packs":[{"pack_id":"canon/fixture","owner_branch":"ToS/canon","path":"ToS/canon/fixture/edges.csv"}],
        "relation_edges":[{"edge_id":"canon-edge","owner_branch":"ToS/canon","pack_id":"canon/fixture","from_id":"a","to_id":"b"}],
        "graph_views":[{"view_id":"corpus-topology","title":"Corpus"},{"view_id":"route-graph","title":"Routes"},{"view_id":"promotion-flow","title":"Promotion"}],
        "authority_order":["ToS/canon"],"runtime_projection_boundary":{"runtime_owner":"abyss-stack"}
    });
    write_private(&source.join(SOURCE), &canonical(&index));
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .current_dir(&source)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    git(&["init", "-q"]);
    git(&["add", "--", SOURCE]);
    git(&[
        "-c",
        "user.name=ToS Native Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "-qm",
        "captured release corpus fixture",
    ]);
    let commit = git(&["rev-parse", "HEAD^{commit}"]);
    let tree = git(&["rev-parse", "HEAD^{tree}"]);
    let capture = root.join("corpus-capture");
    let restored = root.join("corpus-restored");
    let includes = vec![SOURCE.to_owned()];
    let excludes = Vec::new();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let captured = capture_git(
        CaptureGitRequest {
            repository: &source,
            commit: &commit,
            include_prefixes: &includes,
            exclude_prefixes: &excludes,
            exclude_path_parts: &excludes,
            output: &capture,
        },
        GitCaptureLimits::default(),
        deadline,
        &cancelled,
    )
    .unwrap();
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: commit,
        source_git_tree: tree,
        capture_manifest_sha256: captured.manifest_sha256,
    };
    let limits = || ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 128,
        max_selected_object_bytes: 8_388_608,
        json: JsonLimits::default(),
    };
    restore_capture(
        &capture,
        &restored,
        &selection,
        CaptureRestoreLimits {
            metadata: limits(),
            max_archive_bytes: 8_388_608,
            max_decoded_bytes: 8_388_608,
            max_source_bytes: 8_388_608,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let reader = tos_source_store::SoftwareCaptureReader::open(
        &capture,
        &restored,
        selection,
        limits(),
        deadline,
        &cancelled,
    )
    .unwrap();
    (reader, restored)
}

#[test]
fn native_release_prepare_and_status_cli_bind_exact_pair_and_refuse_bad_candidates() {
    let scratch = Scratch::new("actions");
    let (corpus_capture, corpus_root) = capture_release_corpus(scratch.path());
    let corpus_path =
        tos_foundation::RelativePath::parse("ToS/derived-exports/tos_corpus_index.min.json")
            .unwrap();
    let cancelled = AtomicBool::new(false);
    let fixture_for = |variant| {
        tos_compiler::knowledge_full_fixture::build_native_fixture_with_philosophy_and_captured_corpus(
        variant, &corpus_capture, &corpus_path, Instant::now() + Duration::from_secs(120), &cancelled)
    };
    let archive = scratch.path().join("software.zip");
    let archive_sha = create_archive(&archive, "first");
    let fixture = fixture_for(
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::ReferencesV2,
    );
    let candidate = scratch.path().join("candidate");
    let (manifest_sha, selection_sha) = write_snapshot(
        &candidate,
        &fixture,
        &corpus_root,
        &fixture.expectation.model_abi,
        false,
        true,
        native_action_process_limits(),
    );
    let deadline = deadline_ns();
    let prepared = action(
        "native-release-prepare",
        scratch.path(),
        prepare_request(&archive, &candidate, deadline),
    );
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert_eq!(
        receipt["schema_version"],
        "tos_access_native_release_preparation_receipt_v1"
    );
    assert_eq!(receipt["admission"], "candidate-preparation-only");
    assert_eq!(receipt["candidate_manifest_sha256"], manifest_sha);
    assert_eq!(receipt["candidate_selection_sha256"], selection_sha);
    let pair = &receipt["pair"];
    let pair_raw = canonical(pair);
    assert_eq!(receipt["pair_id"], Digest256::of_bytes(&pair_raw).to_hex());
    assert_eq!(pair["software_sha256"], archive_sha);
    assert_eq!(pair["data_manifest_sha256"], manifest_sha);
    assert_eq!(pair["query_schema"], fixture.expectation.model_abi);
    assert_eq!(pair["compiler_version"], tos_compiler::COMPILER_VERSION);

    let bad_abi = scratch.path().join("bad-abi");
    let (bad_manifest_sha, _) = write_snapshot(
        &bad_abi,
        &fixture,
        &corpus_root,
        "unsupported-fixture-abi",
        false,
        false,
        tos_compiler::knowledge_full_fixture::NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS,
    );
    let refused_abi = action(
        "native-release-prepare",
        scratch.path(),
        prepare_request(&archive, &bad_abi, deadline_ns()),
    );
    assert!(!refused_abi.status.success());
    assert!(
        String::from_utf8_lossy(&refused_abi.stderr)
            .contains("compiler/ABI/source profile mismatch")
    );
    assert_ne!(bad_manifest_sha, manifest_sha);

    let corrupt = scratch.path().join("corrupt");
    write_snapshot(
        &corrupt,
        &fixture,
        &corpus_root,
        &fixture.expectation.model_abi,
        true,
        false,
        tos_compiler::knowledge_full_fixture::NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS,
    );
    let refused_corrupt = action(
        "native-release-prepare",
        scratch.path(),
        prepare_request(&archive, &corrupt, deadline_ns()),
    );
    assert!(!refused_corrupt.status.success());

    let release = scratch.path().join("release");
    for directory in [
        release.clone(),
        release.join("pairs"),
        release.join("bindings"),
        release.join("revocations"),
        release.join("revocations/data"),
        release.join("revocations/corpus"),
        release.join("revocations/software"),
    ] {
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    write_private(&release.join(".release.lock"), b"");
    let pair_id = receipt["pair_id"].as_str().unwrap();
    write_private(&release.join(format!("pairs/{pair_id}.json")), &pair_raw);
    let bindings_raw = canonical(&receipt["bindings"]);
    write_private(
        &release.join(format!("bindings/{pair_id}.json")),
        &bindings_raw,
    );
    write_private(
        &release.join("current.json"),
        &canonical(&serde_json::json!({
            "schema_version":"tos_access_release_pointer_v1",
            "current":pair_id,
            "previous":null,
        })),
    );
    let status = action(
        "native-release-status",
        scratch.path(),
        serde_json::json!({
            "schema_version":"tos_access_native_release_status_request_v1",
            "work_deadline_ns":deadline_ns(),
            "release_root":release,
        }),
    );
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(
        status["schema_version"],
        "tos_access_native_release_status_receipt_v1"
    );
    assert_eq!(status["pair_id"], pair_id);
    assert_eq!(canonical(&status["pair"]), pair_raw);
    assert_eq!(canonical(&status["bindings"]), bindings_raw);
    assert!(status["previous"].is_null());

    // Exercise the actual installed native CLI and writer protocol with two
    // independently verified software/data pairs. Their corpus source revision
    // is shared, while the view variant changes the selected data identity.
    let second_archive = scratch.path().join("software-second.zip");
    let second_archive_sha = create_archive(&second_archive, "second");
    let second_fixture = fixture_for(
        tos_compiler::knowledge_full_fixture::PhilosophyFixtureViewVariant::InlineBothV1,
    );
    let second_candidate = scratch.path().join("candidate-second");
    let (second_manifest_sha, second_selection_sha) = write_snapshot(
        &second_candidate,
        &second_fixture,
        &corpus_root,
        &second_fixture.expectation.model_abi,
        false,
        true,
        native_action_process_limits(),
    );
    let second_prepared = action(
        "native-release-prepare",
        scratch.path(),
        prepare_request(&second_archive, &second_candidate, deadline_ns()),
    );
    assert!(
        second_prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&second_prepared.stderr)
    );
    let second_receipt: serde_json::Value =
        serde_json::from_slice(&second_prepared.stdout).unwrap();
    assert_eq!(
        second_receipt["candidate_manifest_sha256"],
        second_manifest_sha
    );
    assert_eq!(
        second_receipt["candidate_selection_sha256"],
        second_selection_sha
    );
    assert_eq!(
        second_receipt["pair"]["software_sha256"],
        second_archive_sha
    );
    assert_ne!(
        second_receipt["pair"]["data_revision"],
        receipt["pair"]["data_revision"]
    );
    assert_eq!(
        second_receipt["pair"]["corpus_revision"],
        receipt["pair"]["corpus_revision"]
    );

    let prefix_first = scratch.path().join("prefix-first");
    let installed_first = install_archive(&archive, &prefix_first);
    assert!(
        installed_first.status.success(),
        "{}",
        String::from_utf8_lossy(&installed_first.stderr)
    );
    let prefix_second = scratch.path().join("prefix-second");
    let installed_second = install_archive(&second_archive, &prefix_second);
    assert!(
        installed_second.status.success(),
        "{}",
        String::from_utf8_lossy(&installed_second.stderr)
    );
    let release_actions = scratch.path().join("release-actions");
    let first_program = prefix_first.join("bin/tos");
    let second_program = prefix_second.join("bin/tos");
    let promoted_first = action_with(
        &first_program,
        "native-release-promote",
        scratch.path(),
        promotion_request(&receipt, &release_actions, &prefix_first, None),
    );
    assert!(
        promoted_first.status.success(),
        "{}",
        String::from_utf8_lossy(&promoted_first.stderr)
    );
    let promoted_first: serde_json::Value = serde_json::from_slice(&promoted_first.stdout).unwrap();
    assert!(promoted_first["committed"].as_bool().unwrap());
    assert_eq!(promoted_first["pair_id"], receipt["pair_id"]);
    assert!(promoted_first["previous"].is_null());

    let first_pair_id = receipt["pair_id"].as_str().unwrap();
    let promoted_second = action_with(
        &second_program,
        "native-release-promote",
        scratch.path(),
        promotion_request(
            &second_receipt,
            &release_actions,
            &prefix_second,
            Some(first_pair_id),
        ),
    );
    assert!(
        promoted_second.status.success(),
        "{}",
        String::from_utf8_lossy(&promoted_second.stderr)
    );
    let promoted_second: serde_json::Value =
        serde_json::from_slice(&promoted_second.stdout).unwrap();
    assert!(promoted_second["committed"].as_bool().unwrap());
    assert_eq!(promoted_second["pair_id"], second_receipt["pair_id"]);
    assert_eq!(promoted_second["previous"], receipt["pair_id"]);

    // Roll back by the exact current CAS and verify that the genuine earlier
    // candidate becomes current while the second pair remains the rollback slot.
    let second_pair_id = second_receipt["pair_id"].as_str().unwrap();
    let rolled_back = action_with(
        &first_program,
        "native-release-rollback",
        scratch.path(),
        rollback_request(&release_actions, &prefix_first, second_pair_id),
    );
    assert!(
        rolled_back.status.success(),
        "{}",
        String::from_utf8_lossy(&rolled_back.stderr)
    );
    let rolled_back: serde_json::Value = serde_json::from_slice(&rolled_back.stdout).unwrap();
    assert!(rolled_back["committed"].as_bool().unwrap());
    assert_eq!(rolled_back["pair_id"], receipt["pair_id"]);
    assert_eq!(rolled_back["previous"], second_receipt["pair_id"]);

    // An undeclared candidate member refuses a switch before pointer mutation.
    let undeclared = second_candidate.join("data/undeclared.json");
    fs::write(&undeclared, b"must not be admitted").unwrap();
    let failed_switch = action_with(
        &second_program,
        "native-release-promote",
        scratch.path(),
        promotion_request(
            &second_receipt,
            &release_actions,
            &prefix_second,
            Some(first_pair_id),
        ),
    );
    assert!(!failed_switch.status.success());
    fs::remove_file(undeclared).unwrap();
    let stable = action_with(
        &first_program,
        "native-release-status",
        scratch.path(),
        serde_json::json!({
            "schema_version":"tos_access_native_release_status_request_v1",
            "work_deadline_ns":deadline_ns(),
            "release_root":release_actions,
        }),
    );
    assert!(
        stable.status.success(),
        "{}",
        String::from_utf8_lossy(&stable.stderr)
    );
    let stable: serde_json::Value = serde_json::from_slice(&stable.stdout).unwrap();
    assert_eq!(stable["pair_id"], receipt["pair_id"]);
    assert_eq!(stable["previous"], second_receipt["pair_id"]);

    // Native CLI revocation writes each immutable component record. Neither
    // rollback nor repromotion may resurrect the revoked previous pair.
    for (kind, field) in [
        ("data", "data_revision"),
        ("corpus", "corpus_revision"),
        ("software", "software_sha256"),
    ] {
        let revoked = action_with(
            &second_program,
            "native-release-revoke",
            scratch.path(),
            revoke_request(
                &release_actions,
                kind,
                second_receipt["pair"][field].as_str().unwrap(),
            ),
        );
        assert!(
            revoked.status.success(),
            "{}",
            String::from_utf8_lossy(&revoked.stderr)
        );
        let revoked: serde_json::Value = serde_json::from_slice(&revoked.stdout).unwrap();
        assert_eq!(revoked["kind"], kind);
        assert!(revoked["committed"].as_bool().unwrap());
        assert!(revoked["durable"].as_bool().unwrap());
    }
    let refused_promotion = action_with(
        &second_program,
        "native-release-promote",
        scratch.path(),
        promotion_request(
            &second_receipt,
            &release_actions,
            &prefix_second,
            Some(first_pair_id),
        ),
    );
    assert!(!refused_promotion.status.success());
    let refused_rollback = action_with(
        &second_program,
        "native-release-rollback",
        scratch.path(),
        rollback_request(&release_actions, &prefix_second, first_pair_id),
    );
    assert!(!refused_rollback.status.success());
    let stable = action_with(
        &first_program,
        "native-release-status",
        scratch.path(),
        serde_json::json!({
            "schema_version":"tos_access_native_release_status_request_v1",
            "work_deadline_ns":deadline_ns(),
            "release_root":release_actions,
        }),
    );
    assert!(
        stable.status.success(),
        "{}",
        String::from_utf8_lossy(&stable.stderr)
    );
    let stable: serde_json::Value = serde_json::from_slice(&stable.stdout).unwrap();
    assert_eq!(stable["pair_id"], receipt["pair_id"]);
    assert_eq!(stable["previous"], second_receipt["pair_id"]);
}
