//! Native ObjectLink transaction interruption and recovery from its owner guard.
use super::*;
use crate::source_command::SourceFile;
use crate::source_creation_store::IsolatedCreationRoot;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::item_rules::ItemLimits;

type Side = Option<(usize, Digest256)>;

struct SelectedWitness {
    path: String,
    before: Side,
    after: Side,
}

const FIXTURE_DIR: &str = "rust/crates/tos-command/tests/fixtures/source-native-object-link-v1";
const FIXTURE_MANIFEST_SHA256: &str =
    "49cc6efdf222a04f268769c31d88afc6ff8389351a2d8036aa02abe83f5a5f81";

fn digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("read frozen ObjectLink tree") {
        let entry = entry.expect("read frozen ObjectLink tree entry");
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).expect("inspect frozen ObjectLink member");
        assert!(
            !metadata.file_type().is_symlink(),
            "fixture contains a symlink"
        );
        if metadata.is_dir() {
            collect_files(root, &path, output);
        } else {
            assert!(metadata.is_file(), "fixture contains a non-file member");
            output.push(
                path.strip_prefix(root)
                    .expect("fixture member beneath isolated root")
                    .to_str()
                    .expect("UTF-8 fixture path")
                    .to_owned(),
            );
        }
    }
}

/// Recreate the captured non-payload ObjectLink tree from the shared frozen Work
/// baseline plus only the byte-different/missing members in this fixture.
fn object_link_fixture(repository: &Path, root: &Path) -> (PathBuf, Vec<u8>) {
    let _baseline = crate::source_creation_store::source_native_test_fixtures::work_expression(
        repository, root,
    );
    let fixture = repository.join(FIXTURE_DIR);
    let manifest_raw =
        fs::read(fixture.join("manifest.json")).expect("ObjectLink fixture manifest");
    assert_eq!(
        digest(&manifest_raw),
        FIXTURE_MANIFEST_SHA256,
        "ObjectLink fixture manifest drift"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_raw).expect("valid ObjectLink fixture manifest");
    assert_eq!(
        manifest["schema_version"],
        "tos_source_object_link_fixture_v1"
    );
    assert_eq!(
        manifest["provenance"]["capture_id"],
        "object-link-base.call-00"
    );
    assert_eq!(
        manifest["provenance"]["capture_stdout_sha256"],
        "0d5181854bef3a55732863a65c3fa23774eae38010e89edaf9b419b5729fa751"
    );
    assert_eq!(
        manifest["provenance"]["source_git_commit"],
        "1d33e8f3dcc360c7e39a3b72db13d97a0d188f23"
    );
    assert_eq!(
        manifest["provenance"]["fixture_basis_manifest_sha256"],
        "06d9dc44671c508caaf69ab58a15fb1e0757eb2bf37c26b6bcbc8f94514c2b14"
    );

    // The shared Work snapshot contains four unrelated authored entries. The
    // captured ObjectLink root does not, so remove them before the exact check.
    for path in [
        "ToS/contracts/source-relation-claim.schema.json",
        "ToS/source-witnesses/works/synthetic/parent/expressions/untouched/editions/child/item.txt",
        "ToS/source-witnesses/works/synthetic/parent/work.human-forms.json",
        "ToS/source-witnesses/works/synthetic/parent/work.json",
    ] {
        let path = root.join(path);
        if path.exists() {
            fs::remove_file(path).expect("remove Work-only fixture member");
        }
    }

    for row in manifest["extension_files"]
        .as_array()
        .expect("ObjectLink fixture extension")
    {
        let relative = row["path"].as_str().expect("ObjectLink source path");
        let artifact = row["artifact_path"]
            .as_str()
            .expect("ObjectLink extension artifact");
        assert!(safe_relative(relative) && relative.starts_with("ToS/"));
        assert!(safe_relative(artifact));
        assert_eq!(artifact, format!("extra/{relative}"));
        let source = fixture.join(artifact);
        let metadata = fs::symlink_metadata(&source).expect("captured ObjectLink extension file");
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        let raw = fs::read(&source).expect("read captured ObjectLink extension file");
        assert_eq!(raw.len(), row["size_bytes"].as_u64().unwrap() as usize);
        assert_eq!(digest(&raw), row["sha256"].as_str().unwrap());
        let target = root.join(relative);
        fs::create_dir_all(target.parent().unwrap()).expect("create ObjectLink fixture parent");
        let mut directory = target.parent().unwrap();
        while directory != root {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).unwrap();
            directory = directory.parent().unwrap();
        }
        fs::write(&target, raw).expect("materialize ObjectLink extension");
        let mode = u32::from_str_radix(row["mode"].as_str().unwrap(), 8).unwrap();
        assert_eq!(mode, 0o600);
        fs::set_permissions(&target, fs::Permissions::from_mode(mode))
            .expect("restore ObjectLink source mode");
    }

    let mut actual = Vec::new();
    collect_files(root, &root.join("ToS"), &mut actual);
    actual.sort();
    let rows = manifest["source_files"]
        .as_array()
        .expect("non-payload ObjectLink source inventory");
    let mut expected = rows
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected, "ObjectLink fixture file set drift");
    for row in rows {
        let relative = row["path"].as_str().unwrap();
        assert!(safe_relative(relative));
        let source = root.join(relative);
        let metadata = fs::symlink_metadata(&source).expect("materialized ObjectLink source");
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert_eq!(metadata.mode() & 0o777, 0o600);
        let raw = fs::read(source).expect("read materialized ObjectLink source");
        assert_eq!(raw.len(), row["size_bytes"].as_u64().unwrap() as usize);
        assert_eq!(digest(&raw), row["sha256"].as_str().unwrap());
    }

    let owner_row = &manifest["owner"];
    let owner_raw = fs::read(fixture.join(owner_row["artifact_path"].as_str().unwrap()))
        .expect("captured ObjectLink owner config");
    assert_eq!(
        owner_raw.len(),
        owner_row["size_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(digest(&owner_raw), owner_row["sha256"].as_str().unwrap());
    let mut owner: serde_json::Value =
        serde_json::from_slice(&owner_raw).expect("valid ObjectLink owner config");
    assert_eq!(owner["source_root"], "$TOS_FIXTURE_ROOT");
    assert_eq!(owner["uid"], owner_row["captured_uid"]);
    owner["source_root"] = serde_json::Value::String(
        root.canonicalize()
            .expect("canonical isolated root")
            .to_string_lossy()
            .into_owned(),
    );
    owner["uid"] = serde_json::Value::from(fs::metadata(root).unwrap().uid());
    let owner_path = root.join("link-owner.json");
    fs::write(&owner_path, serde_json::to_vec_pretty(&owner).unwrap())
        .expect("write isolated ObjectLink owner config");
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600))
        .expect("protect isolated ObjectLink owner config");

    let proposal_row = &manifest["proposal"];
    let proposal_path = fixture.join(proposal_row["artifact_path"].as_str().unwrap());
    let proposal_raw = fs::read(proposal_path).expect("captured ObjectLink proposal");
    assert_eq!(
        proposal_raw.len(),
        proposal_row["size_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(
        digest(&proposal_raw),
        proposal_row["sha256"].as_str().unwrap()
    );
    let proposal: serde_json::Value =
        serde_json::from_slice(&proposal_raw).expect("valid ObjectLink proposal");
    assert_eq!(proposal["operation"], "prepare-create");
    assert_eq!(proposal["schema_version"], REQUEST);
    (owner_path, proposal_raw)
}

fn side(raw: Option<&[u8]>) -> Side {
    raw.map(|raw| (raw.len(), Digest256::of_bytes(raw)))
}

fn current_side(root: &Path, reference: &str) -> Side {
    match fs::read(root.join(reference)) {
        Ok(raw) => side(Some(&raw)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("selected ObjectLink member read: {error}"),
    }
}

fn mixed_selected(root: &Path, selected: &[SelectedWitness]) -> bool {
    let mut before = false;
    let mut after = false;
    for member in selected {
        let actual = current_side(root, &member.path);
        assert!(
            actual == member.before || actual == member.after,
            "selected ObjectLink member entered a third state: {}",
            member.path
        );
        if member.before == member.after {
            continue;
        }
        before |= actual == member.before;
        after |= actual == member.after;
    }
    before && after
}

#[test]
fn object_link_pending_guard_refusal_resumes_or_rolls_back_natively() {
    use crate::source_creation_store::native_owner_test_support as support;

    let repository = support::repository();
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    for decision in [
        ObjectLinkRecoveryDecision::Resume,
        ObjectLinkRecoveryDecision::Rollback,
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let root = isolated.path();
        let (owner_path, proposal_raw) = object_link_fixture(&repository, root);
        let owner_raw = fs::read(&owner_path).unwrap();
        let authored = support::authored(root);
        let mut files = authored.clone();
        for name in IMPLEMENTATIONS {
            let raw = fs::read(repository.join(name)).unwrap();
            let target = root.join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, &raw).unwrap();
            assert!(files.insert((*name).to_owned(), raw).is_none());
        }
        let (software, components) =
            support::software(&repository, &files, temporary.path(), deadline, &cancelled);
        let (revision, selected) = support::cut(
            &authored,
            &temporary.path().join("cut"),
            deadline,
            &cancelled,
        );
        let mut command_context = CommandContext {
            base_revision: revision,
            configuration_raw: owner_raw,
            request_raw: proposal_raw,
            recorded_at: "2026-01-01T12:34:56+00:00".to_owned(),
            effective_uid: fs::metadata(root).unwrap().uid().into(),
            files: files
                .iter()
                .map(|(path, raw)| SourceFile {
                    path: RelativePath::parse(path).unwrap(),
                    raw: raw.clone(),
                })
                .collect(),
        };
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner_path, deadline, &cancelled)
                .unwrap();
        let limits = ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline,
        };
        let mut preview_worker = support::worker(&selected, deadline, &cancelled);
        let prepared = prepare_isolated_object_link_from_proposal(
            &filesystem,
            &command_context,
            &selected,
            &software,
            &components,
            &mut preview_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(preview_worker);
        let mut request = prepared.request().clone();
        cmd::set(
            &mut request,
            "command_id",
            cmd::string("test:object-link-recovery"),
        )
        .unwrap();
        command_context.request_raw = cmd::canonical(&request).unwrap();

        command_context
            .check_from_selected_captures(&selected, &software, &components, deadline, &cancelled)
            .unwrap();
        let owner =
            Owner::select(&filesystem, &command_context, false, limits, &cancelled).unwrap();
        let snapshot =
            work_transaction::PublicationSnapshot::select(&filesystem, deadline, &cancelled)
                .unwrap();
        assert_eq!(
            cmd::field(&owner.request, "expected_publication")
                .unwrap()
                .as_str(),
            snapshot.token.as_deref()
        );
        shared::complete_current_cut(&filesystem, &selected, &snapshot, deadline, &cancelled)
            .unwrap();
        owner.directories(&filesystem, limits, &cancelled).unwrap();
        let mut compose_worker = support::worker(&selected, deadline, &cancelled);
        let dependencies = context(
            &filesystem,
            &command_context,
            &owner,
            &selected,
            &mut compose_worker,
            snapshot.token.as_deref(),
            limits,
            &cancelled,
        )
        .unwrap();
        assert_eq!(
            cmd::record_digest(&dependencies).unwrap().to_prefixed(),
            cmd::field(&owner.request, "expected_dependencies")
                .unwrap()
                .as_str()
                .unwrap()
        );
        let (plan, _receipt, reads) = compose(
            &filesystem,
            &command_context,
            &owner,
            &selected,
            &software,
            &components,
            &mut compose_worker,
            dependencies,
            limits,
            &cancelled,
        )
        .unwrap();
        compose_worker.finish(deadline, &cancelled).unwrap();
        let selected_witnesses = plan
            .files
            .iter()
            .map(|file| SelectedWitness {
                path: file.path.as_str().to_owned(),
                before: side(file.before.as_deref()),
                after: side(file.after.as_deref()),
            })
            .collect::<Vec<_>>();
        assert!(selected_witnesses.len() > 1);
        let dependency = root.join("ToS/contracts/corpus-record.schema.json");
        let original = fs::read(&dependency).unwrap();
        let changed = [original.as_slice(), b"\n"].concat();
        let fence =
            work_transaction::WorkCorpusFence::hold(&filesystem, deadline, &cancelled).unwrap();
        let guard_plan = plan.clone();
        let mut switched = false;
        let result = fence.apply(
            plan,
            &snapshot,
            |summary, extent| {
                if extent.pending_state.is_some()
                    && !switched
                    && mixed_selected(root, &selected_witnesses)
                {
                    fs::write(&dependency, &changed).unwrap();
                    switched = true;
                }
                guard(
                    &filesystem,
                    &command_context,
                    &selected,
                    &guard_plan,
                    &reads,
                    Some(&snapshot),
                    summary,
                    extent,
                    limits,
                    &cancelled,
                )
            },
            deadline,
            &cancelled,
        );
        drop(fence);
        assert!(
            switched,
            "the owner guard reached the interrupted pending edge"
        );
        assert!(matches!(result, Err(SourceCommandError::Conflict(_))));
        assert_eq!(fs::read(&dependency).unwrap(), changed);
        assert!(mixed_selected(root, &selected_witnesses));
        let control = root.join("ToS/source-witnesses/.metadata-publication.json");
        let pending: serde_json::Value =
            serde_json::from_slice(&fs::read(&control).unwrap()).unwrap();
        assert_eq!(pending["phase"], "pending");
        assert_eq!(pending["outcome"], serde_json::Value::Null);
        fs::write(&dependency, original).unwrap();

        let mut renewal_config: serde_json::Value =
            serde_json::from_slice(&command_context.configuration_raw).unwrap();
        renewal_config["principal_id"] = serde_json::json!("model:synthetic-recoverer");
        renewal_config["allowed_operations"] = serde_json::json!([RECOVERY]);
        let renewal_raw = serde_json::to_vec_pretty(&renewal_config).unwrap();
        fs::write(&owner_path, &renewal_raw).unwrap();
        fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();
        let renewal_document = cmd::parse(&renewal_raw).unwrap();
        let decision_name = if matches!(decision, ObjectLinkRecoveryDecision::Rollback) {
            "rollback"
        } else {
            "resume"
        };
        let recovery_request = serde_json::json!({
            "schema_version": REQUEST,
            "operation": RECOVERY,
            "transaction_id": pending["transaction_id"],
            "decision": decision_name,
            "expected_configuration": cmd::record_digest(&renewal_document).unwrap().to_prefixed(),
        });
        let mut renewal_context = command_context.clone();
        renewal_context.configuration_raw = renewal_raw;
        renewal_context.request_raw = serde_json::to_vec(&recovery_request).unwrap();
        let renewal_filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner_path, deadline, &cancelled)
                .unwrap();
        let mut recovery_worker = support::worker(&selected, deadline, &cancelled);
        let recovered = recover_isolated_object_link_from_captures(
            &renewal_filesystem,
            &renewal_context,
            &selected,
            &software,
            &components,
            &mut recovery_worker,
            decision,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(recovery_worker);
        assert_eq!(
            recovered.transaction_id(),
            pending["transaction_id"].as_str().unwrap()
        );
        assert!(!recovered.replayed());
        let rollback = matches!(decision, ObjectLinkRecoveryDecision::Rollback);
        for member in &selected_witnesses {
            assert_eq!(
                current_side(root, &member.path),
                if rollback {
                    member.before
                } else {
                    member.after
                },
                "selected ObjectLink member was not restored: {}",
                member.path,
            );
        }
        let terminal: serde_json::Value =
            serde_json::from_slice(&fs::read(&control).unwrap()).unwrap();
        assert_eq!(terminal["phase"], "ready");
        assert_eq!(terminal["transaction_id"], pending["transaction_id"]);
        assert_eq!(terminal["manifest_sha256"], pending["manifest_sha256"]);
        assert_eq!(
            terminal["outcome"],
            if rollback { "rolled-back" } else { "committed" }
        );
        assert_eq!(recovered.receipt().is_some(), !rollback);
        assert_eq!(recovered.materializations().is_some(), !rollback);
        let link_home = root
            .join(renewal_config["link_source_path"].as_str().unwrap())
            .parent()
            .unwrap()
            .to_path_buf();
        let claim_home = root
            .join(renewal_config["claim_source_path"].as_str().unwrap())
            .parent()
            .unwrap()
            .to_path_buf();
        assert_eq!(link_home.exists(), !rollback);
        assert_eq!(claim_home.exists(), !rollback);
    }
}
