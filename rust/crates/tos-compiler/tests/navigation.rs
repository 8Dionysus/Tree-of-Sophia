use rusqlite::Connection;
use std::{
    fs,
    fs::File,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tos_compiler::{
    CandidateReceipt, Collection, ImmutableModelCustody, LegacyPartitionedNavigation, Limits,
    MODEL_ABI, NavigationInput, PublicationAuthority, SELECTION_PROFILE, SelectedExpectation,
    SelectionFence, SourceBinding, compile_navigation, open_selected_model, publish_candidate,
};

const ROOT_SHA: &str = "2d9edbd88ebee606fc6fc4b06e23b43d4c73ccaed15e2ffb171d7518ead2f3e3";
const ASSESSED_AUTHORITY: &str = "generated read-only navigation; authored branch manifests, source records, claims, item manifests, and rights records retain authority; local assessed research candidate, not public clearance or a current runtime grant";

struct FixtureCustody;
impl ImmutableModelCustody for FixtureCustody {
    fn verify_held(&self, pinned: &File, _: &str, size_bytes: u64) -> tos_compiler::Result<()> {
        if pinned.metadata()?.len() != size_bytes {
            return Err(tos_compiler::Error::Invalid("fixture custody size"));
        }
        Ok(())
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tos_corpus_index.min.json")
}
fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
fn binding() -> SourceBinding {
    SourceBinding {
        owner_profile: "fixture_owner_v1".into(),
        source_cut: "fixture_sealed_cut".into(),
        through_commit_seq: 7,
        membership_root: "e92c74487c3cdcc852cc761711fcd2c826a527321f806da82b936e630b1613b7".into(),
        index_generation: "fixture_generation".into(),
        route_map_version: "fixture_route".into(),
        reader_abi: "fixture_reader".into(),
        projection_root_sha256: ROOT_SHA.into(),
        complete: true,
    }
}
fn build() -> (PathBuf, CandidateReceipt) {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-cmp-test-{}-{tick}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("model.sqlite3");
    let mut input =
        LegacyPartitionedNavigation::open(&fixture(), ROOT_SHA, Limits::default()).unwrap();
    let receipt = compile_navigation(&mut input, &binding(), &path, Limits::default()).unwrap();
    (path, receipt)
}

#[test]
fn compiles_exact_partitioned_oracle_with_visible_adjacency() {
    let (path, receipt) = build();
    assert_eq!(
        (receipt.node_count, receipt.edge_count, receipt.rights_count),
        (3, 2, 1)
    );
    assert_eq!(
        (receipt.visible_node_count, receipt.visible_edge_count),
        (2, 1)
    );
    let db = Connection::open(&path).unwrap();
    let profile: String = db
        .query_row(
            "SELECT value FROM metadata WHERE key='selection_profile'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(profile, SELECTION_PROFILE);
    let abi: String = db
        .query_row(
            "SELECT value FROM metadata WHERE key='model_abi'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(abi, MODEL_ABI);
    let authority: String = db
        .query_row(
            "SELECT value FROM metadata WHERE key='authority_boundary'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(authority, "fixture");
    let selected: Vec<String> = {
        let mut stmt = db
            .prepare("SELECT node_id FROM nodes WHERE visible=1 ORDER BY node_id")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(selected, ["id.alpha", "id.beta"]);
    let edges: Vec<String> = {
        let mut stmt = db
            .prepare(
                "SELECT edge_id FROM edges WHERE visible=1 AND from_id='id.alpha' ORDER BY edge_id",
            )
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(edges, ["edge.visible"]);
    let (count, digest): (u64, String) = db
        .query_row(
            "SELECT edge_count,edges_sha256 FROM adjacency WHERE from_id='id.alpha'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        digest,
        "eb593d8115b0a4145e61c02f8f887d24685cf0d2730b2e0fe903aa56efc508a7"
    );
    let (empty_count, empty_digest): (u64, String) = db
        .query_row(
            "SELECT edge_count,edges_sha256 FROM adjacency WHERE from_id='id.beta'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(empty_count, 0);
    assert_eq!(
        empty_digest,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let carrier: Vec<u8> = db
        .query_row(
            "SELECT carrier FROM nodes WHERE node_id='id.alpha'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(String::from_utf8(carrier).unwrap().contains("ToS/a.json"));
    drop(db);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn refuses_untrusted_root_incomplete_cut_and_tight_budget() {
    assert!(
        LegacyPartitionedNavigation::open(&fixture(), &"0".repeat(64), Limits::default()).is_err()
    );
    let mut incomplete = binding();
    incomplete.complete = false;
    let mut reader =
        LegacyPartitionedNavigation::open(&fixture(), ROOT_SHA, Limits::default()).unwrap();
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-cmp-refuse-{}-{tick}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let candidate = dir.join("candidate.sqlite3");
    assert!(compile_navigation(&mut reader, &incomplete, &candidate, Limits::default()).is_err());
    assert!(!candidate.exists());
    let mut wrong_binding = binding();
    wrong_binding.projection_root_sha256 = "0".repeat(64);
    assert!(
        compile_navigation(&mut reader, &wrong_binding, &candidate, Limits::default()).is_err()
    );
    assert!(!candidate.exists());
    let limits = Limits {
        max_rows: 1,
        ..Limits::default()
    };
    let mut reader = LegacyPartitionedNavigation::open(&fixture(), ROOT_SHA, limits).unwrap();
    assert!(compile_navigation(&mut reader, &binding(), &candidate, limits).is_err());
    assert!(!candidate.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn changed_partition_byte_refuses_and_removes_candidate() {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-cmp-tamper-{}-{tick}", std::process::id()));
    copy_tree(fixture().parent().unwrap(), &dir);
    let copied = dir.join("tos_corpus_index.min.json");
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(&copied).unwrap()).unwrap();
    let relative = manifest["collections"]["source_navigation/rights"]["root"]["path"]
        .as_str()
        .unwrap();
    let part = dir.join(relative);
    let mut bytes = fs::read(&part).unwrap();
    bytes[0] ^= 1;
    fs::write(&part, bytes).unwrap();
    let candidate = dir.join("candidate.sqlite3");
    let mut input =
        LegacyPartitionedNavigation::open(&copied, ROOT_SHA, Limits::default()).unwrap();
    assert!(compile_navigation(&mut input, &binding(), &candidate, Limits::default()).is_err());
    assert!(!candidate.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_root_binds_exact_assessed_authority_string() {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-cmp-header-{}-{tick}", std::process::id()));
    copy_tree(fixture().parent().unwrap(), &dir);
    let root = dir.join("tos_corpus_index.min.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&root).unwrap()).unwrap();
    manifest["header"]["source_navigation"]["authority_boundary"] = ASSESSED_AUTHORITY.into();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(&root, &bytes).unwrap();
    let mut owner = binding();
    owner.projection_root_sha256 = tos_foundation::Digest256::of_bytes(&bytes).to_hex();
    let mut input =
        LegacyPartitionedNavigation::open(&root, &owner.projection_root_sha256, Limits::default())
            .unwrap();
    let selected = dir.join("assessed.sqlite3");
    compile_navigation(&mut input, &owner, &selected, Limits::default()).unwrap();
    let db = Connection::open(&selected).unwrap();
    let value: String = db
        .query_row(
            "SELECT value FROM metadata WHERE key='authority_boundary'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(value, ASSESSED_AUTHORITY);
    drop(db);
    fs::remove_dir_all(dir).unwrap();
}

struct DanglingInput;
impl NavigationInput for DanglingInput {
    fn verify_binding(&self, _: &SourceBinding) -> tos_compiler::Result<()> {
        Ok(())
    }
    fn authority_boundary(&self) -> tos_compiler::Result<String> {
        Ok("exact authored note".to_owned())
    }
    fn visit(
        &mut self,
        collection: Collection,
        sink: &mut dyn FnMut(&[u8]) -> tos_compiler::Result<()>,
    ) -> tos_compiler::Result<()> {
        match collection {
            Collection::Nodes => sink(br#"{"node_id":"id.alpha","source_ref":"ToS/a"}"#)?,
            Collection::Edges => sink(br#"{"edge_id":"dangling","from_id":"id.alpha","to_id":"id.absent","source_refs":["ToS/a"]}"#)?,
            Collection::Rights => {}
        }
        Ok(())
    }
    fn verify_sealed_cut(&mut self) -> tos_compiler::Result<()> {
        Ok(())
    }
}

#[test]
fn dangling_raw_edge_is_not_visible_bibliographic_adjacency() {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-cmp-dangling-{}-{tick}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("model.sqlite3");
    let receipt =
        compile_navigation(&mut DanglingInput, &binding(), &path, Limits::default()).unwrap();
    assert_eq!((receipt.edge_count, receipt.visible_edge_count), (1, 0));
    let db = Connection::open(&path).unwrap();
    let (count, digest): (u64, String) = db
        .query_row(
            "SELECT edge_count,edges_sha256 FROM adjacency WHERE from_id='id.alpha'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(digest, tos_foundation::Digest256::of_bytes(b"").to_hex());
    drop(db);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_vm_and_emitted_input_budgets_fail_private() {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-cmp-budgets-{}-{tick}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    for (label, limits, expected) in [
        (
            "vm",
            Limits {
                max_sql_vm_steps: 1,
                ..Limits::default()
            },
            "SQLite VM steps",
        ),
        (
            "input",
            Limits {
                max_work_bytes: 1,
                ..Limits::default()
            },
            "emitted input bytes",
        ),
        (
            "output",
            Limits {
                max_output_bytes: 1024,
                ..Limits::default()
            },
            "output smaller than SQLite page",
        ),
    ] {
        let candidate = dir.join(format!("{label}.sqlite3"));
        let err = compile_navigation(&mut DanglingInput, &binding(), &candidate, limits)
            .expect_err("tight budget must refuse");
        assert!(err.to_string().contains(expected), "{label}: {err}");
        assert!(!candidate.exists(), "{label} left a private candidate");
    }
    let page_limited = dir.join("page-limited.sqlite3");
    let err = compile_navigation(
        &mut DanglingInput,
        &binding(),
        &page_limited,
        Limits {
            max_output_bytes: 4096,
            ..Limits::default()
        },
    )
    .expect_err("SQLite page cap must stop the schema or data build");
    assert!(
        !page_limited.exists(),
        "page cap left a private candidate after {err}"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn legacy_reader_refuses_symlink_and_fifo_roots_without_opening_target() {
    use std::{ffi::CString, os::unix::fs::symlink};
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tos-cmp-path-{}-{tick}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let link = dir.join("root-link.json");
    symlink(fixture(), &link).unwrap();
    assert!(LegacyPartitionedNavigation::open(&link, ROOT_SHA, Limits::default()).is_err());
    let fifo = dir.join("root-fifo.json");
    let name = CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: name is an owned, NUL-terminated filesystem path.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(LegacyPartitionedNavigation::open(&fifo, ROOT_SHA, Limits::default()).is_err());
    fs::remove_dir_all(dir).unwrap();
}

struct FixtureOwner;
struct FixtureFence;
impl SelectionFence for FixtureFence {
    fn receipt_id(&self) -> &str {
        "fixture-owner-selection"
    }
    fn recheck_held(&self) -> tos_compiler::Result<()> {
        Ok(())
    }
}
impl PublicationAuthority for FixtureOwner {
    type Fence = FixtureFence;
    fn acquire_selection_fence(
        &mut self,
        source: &SourceBinding,
        receipt: &CandidateReceipt,
    ) -> tos_compiler::Result<Self::Fence> {
        assert_eq!(source.source_cut, receipt.source_cut);
        Ok(FixtureFence)
    }
}

#[test]
fn selection_is_atomic_and_requires_exact_previous_pointer() {
    let (path, receipt) = build();
    let parent = path.parent().unwrap().to_path_buf();
    let selected = publish_candidate(
        &path,
        &binding(),
        &receipt,
        &parent,
        None,
        &mut FixtureOwner,
    )
    .unwrap();
    assert!(path.exists());
    assert!(selected.selected_path.exists());
    let pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(&selected.pointer_path).unwrap()).unwrap();
    assert_eq!(pointer["model_sha256"], receipt.sqlite_sha256);
    assert_eq!(
        pointer["owner_authority_receipt_id"],
        "fixture-owner-selection"
    );
    let mut verified = open_selected_model(
        &parent,
        &SelectedExpectation {
            source: &binding(),
            model_sha256: &receipt.sqlite_sha256,
            model_size_bytes: receipt.sqlite_size_bytes,
            owner_receipt_id: "fixture-owner-selection",
            custody: Arc::new(FixtureCustody),
            max_cold_open_bytes: receipt.sqlite_size_bytes,
            max_cold_open_vm_steps: 1_000_000,
        },
    )
    .unwrap();
    assert_eq!(verified.selection().authority_boundary, "fixture");
    assert!(verified.open_vm_steps() > 0);
    assert!(verified.open_vm_steps() < 1_000_000);
    verified.check_pin().unwrap();
    let visible: u64 = verified
        .connection_mut()
        .query_row("SELECT count(*) FROM nodes WHERE visible=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(visible, 2);
    assert!(matches!(
        open_selected_model(
            &parent,
            &SelectedExpectation {
                source: &binding(),
                model_sha256: &receipt.sqlite_sha256,
                model_size_bytes: receipt.sqlite_size_bytes,
                owner_receipt_id: "fixture-owner-selection",
                custody: Arc::new(FixtureCustody),
                max_cold_open_bytes: receipt.sqlite_size_bytes - 1,
                max_cold_open_vm_steps: 1_000_000,
            },
        ),
        Err(tos_compiler::Error::Budget("cold-open model bytes"))
    ));
    assert!(
        open_selected_model(
            &parent,
            &SelectedExpectation {
                source: &binding(),
                model_sha256: &receipt.sqlite_sha256,
                model_size_bytes: receipt.sqlite_size_bytes,
                owner_receipt_id: "fixture-owner-selection",
                custody: Arc::new(FixtureCustody),
                max_cold_open_bytes: receipt.sqlite_size_bytes,
                max_cold_open_vm_steps: 1,
            },
        )
        .is_err()
    );
    let selected_backup = parent.join("selected-backup.sqlite3");
    fs::rename(&selected.selected_path, &selected_backup).unwrap();
    fs::write(&selected.selected_path, b"replaced path").unwrap();
    let still_visible: u64 = verified
        .connection_mut()
        .query_row("SELECT count(*) FROM nodes WHERE visible=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(still_visible, 2);
    let mut sibling = verified.fork_reader().unwrap();
    assert!(sibling.open_vm_steps() > 0);
    assert!(sibling.open_vm_steps() < 1_000_000);
    assert!(verified.fork_reader_with_vm_budget(1).is_err());
    assert!(verified.fork_reader_with_vm_budget(1_000_001).is_err());
    let bounded_sibling = verified.fork_reader_with_vm_budget(100_000).unwrap();
    assert!(bounded_sibling.open_vm_steps() < 100_000);
    let sibling_visible: u64 = sibling
        .connection_mut()
        .query_row("SELECT count(*) FROM nodes WHERE visible=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(sibling_visible, 2);
    assert!(
        open_selected_model(
            &parent,
            &SelectedExpectation {
                source: &binding(),
                model_sha256: &receipt.sqlite_sha256,
                model_size_bytes: receipt.sqlite_size_bytes,
                owner_receipt_id: "fixture-owner-selection",
                custody: Arc::new(FixtureCustody),
                max_cold_open_bytes: receipt.sqlite_size_bytes,
                max_cold_open_vm_steps: 1_000_000,
            },
        )
        .is_err()
    );

    let second = parent.join("second.sqlite3");
    let mut input =
        LegacyPartitionedNavigation::open(&fixture(), ROOT_SHA, Limits::default()).unwrap();
    let next = compile_navigation(&mut input, &binding(), &second, Limits::default()).unwrap();
    assert!(
        publish_candidate(&second, &binding(), &next, &parent, None, &mut FixtureOwner,).is_err()
    );
    assert!(second.exists());
    fs::remove_dir_all(parent).unwrap();
}

struct SwappingOwner {
    candidate: PathBuf,
}
impl PublicationAuthority for SwappingOwner {
    type Fence = FixtureFence;
    fn acquire_selection_fence(
        &mut self,
        _source: &SourceBinding,
        _receipt: &CandidateReceipt,
    ) -> tos_compiler::Result<Self::Fence> {
        // Full immutable installation has completed before owner authority is
        // acquired; swapping the private candidate path cannot rebind it.
        let installed = self
            .candidate
            .parent()
            .unwrap()
            .join(format!("{}.sqlite3", _receipt.sqlite_sha256));
        assert!(installed.is_file());
        assert_eq!(
            tos_foundation::Digest256::of_bytes(&fs::read(installed)?).to_hex(),
            _receipt.sqlite_sha256
        );
        fs::rename(&self.candidate, self.candidate.with_extension("original"))?;
        fs::write(&self.candidate, b"attacker replacement")?;
        Ok(FixtureFence)
    }
}

#[test]
fn callback_path_swap_cannot_rebind_selected_bytes() {
    let (path, receipt) = build();
    let parent = path.parent().unwrap().to_path_buf();
    let selected = publish_candidate(
        &path,
        &binding(),
        &receipt,
        &parent,
        None,
        &mut SwappingOwner {
            candidate: path.clone(),
        },
    )
    .unwrap();
    let selected_bytes = fs::read(selected.selected_path).unwrap();
    assert_eq!(
        tos_foundation::Digest256::of_bytes(&selected_bytes).to_hex(),
        receipt.sqlite_sha256
    );
    assert_eq!(fs::read(&path).unwrap(), b"attacker replacement");
    fs::remove_dir_all(parent).unwrap();
}
