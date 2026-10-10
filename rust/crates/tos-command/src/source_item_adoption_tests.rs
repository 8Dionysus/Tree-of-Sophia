//! Item owner recovery from an actual retained plan with no publication head.
use super::*;
use crate::source_command::SourceFile;
use crate::source_creation_store::IsolatedCreationRoot;
use crate::source_creation_store::native_owner_test_support as support;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tos_foundation::RelativePath;

fn context_with_request(context: &CommandContext, request_raw: Vec<u8>) -> CommandContext {
    CommandContext {
        base_revision: context.base_revision.clone(),
        configuration_raw: context.configuration_raw.clone(),
        request_raw,
        recorded_at: context.recorded_at.clone(),
        effective_uid: context.effective_uid,
        files: context.files.clone(),
    }
}

#[test]
fn item_retained_orphan_recovers_through_owner_for_resume_and_rollback() {
    let repository = support::repository();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    for (decision, expect_commit) in [("resume", true), ("rollback", false)] {
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let root = isolated.path();
        let fixture =
            crate::source_creation_store::source_native_test_fixtures::item_orphan_recovery(
                &repository,
                root,
            );
        let owner_path = Path::new(fixture["owner"].as_str().unwrap());
        let owner_raw = fs::read(owner_path).unwrap();
        let authored = support::authored(root);
        let mut files = authored.clone();
        for name in IMPLEMENTATIONS {
            let raw = fs::read(repository.join(name)).unwrap();
            let target = root.join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, &raw).unwrap();
            assert!(files.insert((*name).to_owned(), raw).is_none());
        }
        let (software, components) =
            support::software(&repository, &files, temporary.path(), deadline, &cancelled);
        let (revision, cut) = support::cut(
            &authored,
            &temporary.path().join("source-cut"),
            deadline,
            &cancelled,
        );
        let proposal_raw = serde_json::to_vec(&fixture["proposal"]).unwrap();
        let context = CommandContext {
            base_revision: revision,
            configuration_raw: owner_raw,
            request_raw: proposal_raw,
            recorded_at: "2026-09-09T12:00:00+00:00".to_owned(),
            effective_uid: u64::from(fs::metadata(root).unwrap().uid()),
            files: files
                .iter()
                .map(|(path, raw)| SourceFile {
                    path: RelativePath::parse(path).unwrap(),
                    raw: raw.clone(),
                })
                .collect(),
        };
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, owner_path, deadline, &cancelled)
                .unwrap();
        let limits = ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline,
        };
        let mut preview_worker = support::worker(&cut, deadline, &cancelled);
        let prepared = prepare_isolated_item_adoption_from_proposal(
            &filesystem,
            &context,
            &cut,
            &software,
            &components,
            &mut preview_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(preview_worker);
        let mut prepared_request: serde_json::Value =
            serde_json::from_slice(&cmd::canonical(prepared.request()).unwrap()).unwrap();
        prepared_request["command_id"] =
            serde_json::json!(format!("native-item-orphan-{decision}"));
        let prepared_context =
            context_with_request(&context, serde_json::to_vec(&prepared_request).unwrap());
        let owner =
            ItemOwner::select(&filesystem, &prepared_context, deadline, &cancelled).unwrap();
        let transaction_id = owner.transaction_id().unwrap();
        let item_path = owner.item_path.as_str().to_owned();
        let publication_path = root.join("ToS/source-witnesses/.metadata-publication.json");
        let publication_before = fs::read(&publication_path).unwrap();
        assert!(!root.join(&item_path).exists());

        // Fail the actual owner guard's second WorkCorpusFence call, after
        // retain has fsynced the plan but before the pending publication head.
        work_transaction::interrupt_item_second_guard_once_for_test();
        let mut worker = support::worker(&cut, deadline, &cancelled);
        let interrupted = execute_isolated_item_adoption_from_captures(
            &filesystem,
            &prepared_context,
            &cut,
            &software,
            &components,
            &mut worker,
            limits,
            &cancelled,
        );
        drop(worker);
        assert!(matches!(interrupted, Err(SourceCommandError::Conflict(_))));

        let orphan = work_transaction::retained_item_orphan(
            &filesystem,
            &transaction_id,
            deadline,
            &cancelled,
        )
        .unwrap()
        .expect("retained Item plan is an orphan before publication head");
        assert_eq!(orphan.0.transaction_id, transaction_id);
        let selected_files = orphan
            .0
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str().to_owned(),
                    file.before.clone(),
                    file.after.clone(),
                )
            })
            .collect::<Vec<_>>();
        assert!(selected_files.len() > 1);
        assert!(
            work_transaction::read_pending(&filesystem, deadline, &cancelled)
                .unwrap()
                .is_none()
        );
        assert!(!root.join(&item_path).exists());

        let grant = cmd::parse(&prepared_context.configuration_raw).unwrap();
        let recovery_request = serde_json::json!({
            "schema_version": REQUEST,
            "operation": RECOVERY,
            "transaction_id": transaction_id,
            "decision": decision,
            "expected_configuration": cmd::record_digest(&grant).unwrap().to_prefixed(),
        });
        let recovery_context = context_with_request(
            &prepared_context,
            serde_json::to_vec(&recovery_request).unwrap(),
        );
        let mut recovery_worker = support::worker(&cut, deadline, &cancelled);
        let recovered = recover_isolated_item_adoption_from_captures(
            &filesystem,
            &recovery_context,
            &cut,
            &software,
            &components,
            &mut recovery_worker,
            limits,
            &cancelled,
        )
        .unwrap();
        drop(recovery_worker);
        assert_eq!(recovered.transaction_id(), transaction_id);
        assert!(!recovered.replayed());
        assert_eq!(recovered.receipt().is_some(), expect_commit);
        assert_eq!(root.join(&item_path).exists(), expect_commit);
        assert_eq!(
            cmd::field(recovered.deposit(), "metadata_committed")
                .unwrap()
                .as_bool(),
            Some(expect_commit)
        );
        for (path, before, after) in &selected_files {
            assert_eq!(
                fs::read(root.join(path)).ok(),
                if expect_commit {
                    after.clone()
                } else {
                    before.clone()
                },
                "Item recovery did not restore the exact selected side: {path}",
            );
        }
        if expect_commit {
            let terminal: serde_json::Value =
                serde_json::from_slice(&fs::read(&publication_path).unwrap()).unwrap();
            assert_eq!(terminal["phase"], "ready");
            assert_eq!(terminal["transaction_id"], transaction_id);
            assert_eq!(terminal["outcome"], "committed");
            // Resuming selects the retained transaction as the durable head.
            assert!(matches!(
                work_transaction::retained_item_orphan(
                    &filesystem,
                    &transaction_id,
                    deadline,
                    &cancelled,
                ),
                Err(SourceCommandError::Conflict(
                    "Item transaction is head selected; orphan recovery refused"
                ))
            ));
        } else {
            // This orphan never published a pending metadata head. Rolling
            // back its byte deposit preserves the previous head and retained
            // plan, rather than publishing a fictitious metadata transaction.
            assert_eq!(fs::read(&publication_path).unwrap(), publication_before);
            let retained = work_transaction::retained_item_orphan(
                &filesystem,
                &transaction_id,
                deadline,
                &cancelled,
            )
            .unwrap()
            .expect("unpublished Item plan remains retained after byte rollback");
            assert_eq!(retained.0.transaction_id, transaction_id);
            assert_eq!(
                retained
                    .0
                    .files
                    .iter()
                    .map(|file| (
                        file.path.as_str().to_owned(),
                        file.before.clone(),
                        file.after.clone(),
                    ))
                    .collect::<Vec<_>>(),
                selected_files,
            );
        }
        assert!(
            work_transaction::read_pending(&filesystem, deadline, &cancelled)
                .unwrap()
                .is_none()
        );
    }
}
