//! Private transaction-contract coverage for overlapping prepared PG revisions.
//! The native subprocess integration separately covers the producer request.

use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_segment_store::{SegmentLimits, SegmentStore};

use crate::source_creation_store::native_owner_test_support as fixture;

const CONTRACT: &[u8] = b"cmd2.private.shadow.contract.v1";
const JOB_ID: &str = "private-job";
const SOFTWARE_COMPONENTS: &[&str] = &[
    "rust/crates/tos-command/src/source_native_cli.rs",
    "rust/crates/tos-command/src/source_native_pg_revision_cli.rs",
    "rust/crates/tos-command/src/source_native_revisions_cli.rs",
    "rust/crates/tos-command/src/source_command.rs",
    "rust/crates/tos-command/src/source_cohort.rs",
    "rust/crates/tos-command/src/source_cohort_revision.rs",
    "rust/crates/tos-command/src/source_cohort_creation.rs",
    "rust/crates/tos-command/src/source_creation.rs",
    "rust/crates/tos-command/src/source_creation_pg.rs",
    "rust/crates/tos-command/src/source_creation_store.rs",
    "rust/crates/tos-command/src/source_revisions.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "rust/crates/tos-command/src/source_private_profile.rs",
    "rust/crates/tos-command/src/source_private_claim.rs",
    "rust/crates/tos-command/src/source_private_owner_store.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_work_transaction.rs",
    "rust/crates/tos-validation/src/assessment.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_versions.rs",
    "rust/crates/tos-compiler/src/source_witness_catalog.rs",
];
const SHARED_SOURCE: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
];

#[test]
fn family_tagged_source_revision_read_envelope_roundtrips_exact_event_cut() {
    let reads = SourceReads {
        reads: Vec::new(),
        locators: BTreeMap::new(),
        managed_original: None,
        revision: Some(SourceRevisionReadBinding {
            family_tag:
                "tos_local_native_metadata_revision_owner_v1:artifact:tos_native_artifact_v2"
                    .to_owned(),
            source_head_sequence: 41,
            predecessor_source_revision: SourceRevision(Digest256::of_bytes(b"predecessor")),
            request_canonical_digest: None,
            owner_configuration_raw: None,
            owner_configuration_raw_digest: None,
            owner_configuration_canonical_digest: None,
        }),
    };
    let encoded = reads_bytes(&reads).expect("family-tagged read envelope encodes");
    let recovered = decode_reads(&encoded).expect("family-tagged read envelope recovers");
    assert_eq!(reads_bytes(&recovered).unwrap(), encoded);
    assert_eq!(recovered.revision, reads.revision);
}

#[derive(Clone)]
struct Target {
    name: &'static str,
    source_path: String,
    record_id: String,
    forms: Vec<(String, String)>,
}

struct Fixture {
    _root: tempfile::TempDir,
    coordinator: DurablePgCoordinator,
    cohort: ManagedSourceCohort,
    original_cut: tos_source_store::CorpusCutReader,
    initial_revision: tos_foundation::SourceRevision,
    initial_membership: tos_source_store::SourceMembershipV1,
    initial_context: CommandContext,
    store: SegmentStore,
    store_root: PathBuf,
    domain: String,
    software: tos_source_store::SoftwareCaptureReader,
    components: tos_source_store::SoftwareComponentSelectionV1,
    targets: Vec<Target>,
}

fn canonical(value: &Value) -> Vec<u8> {
    let raw = serde_json::to_vec(value).unwrap();
    let parsed = crate::source_command::parse(&raw).unwrap();
    crate::source_command::canonical(&parsed).unwrap()
}

fn repository_members(repository: &Path, directories: &[&str]) -> BTreeMap<String, Vec<u8>> {
    fn insert(repository: &Path, members: &mut BTreeMap<String, Vec<u8>>, path: &str) {
        let raw = fs::read(repository.join(path)).expect("selected authored member reads");
        if let Some(prior) = members.insert(path.to_owned(), raw.clone()) {
            assert_eq!(prior, raw, "a selected path has one exact value");
        }
    }

    let mut members = BTreeMap::new();
    for directory in directories {
        let package_path = format!("ToS/source-witnesses/agents/{directory}");
        let mut entries = fs::read_dir(repository.join(&package_path))
            .expect("selected actual Agent package opens")
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        assert_eq!(entries.len(), 6, "selected Agent is the no-history fixture");
        for entry in entries {
            assert!(entry.file_type().unwrap().is_file());
            insert(
                repository,
                &mut members,
                &format!("{package_path}/{}", entry.file_name().to_str().unwrap()),
            );
        }
        let record_path = format!("{package_path}/agent.json");
        let record: Value = serde_json::from_slice(&members[&record_path]).unwrap();
        for source_ref in record["source_refs"].as_array().unwrap() {
            let Some(path) = source_ref.as_str().filter(|path| path.starts_with("ToS/")) else {
                continue;
            };
            RelativePath::parse(path).expect("local source reference is safe");
            insert(repository, &mut members, path);
        }
    }
    for path in SHARED_SOURCE {
        insert(repository, &mut members, path);
    }
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file()
            && entry
                .file_name()
                .to_str()
                .unwrap()
                .ends_with(".schema.json")
        {
            insert(
                repository,
                &mut members,
                &format!("ToS/contracts/{}", entry.file_name().to_str().unwrap()),
            );
        }
    }
    assert!(members.len() <= 512);
    assert!(members.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    members
}

fn target(repository: &Path, directory: &'static str) -> Target {
    let package = format!("ToS/source-witnesses/agents/{directory}");
    let source_path = format!("{package}/agent.json");
    let forms_path = format!("{package}/agent.human-forms.json");
    let record_raw = fs::read(repository.join(&source_path)).unwrap();
    let record: Value = serde_json::from_slice(&record_raw).unwrap();
    let source = crate::source_command::parse(&record_raw)
        .expect("the selected authored Agent parses as a strict source record");
    let source_fields = tos_validation::source_forms::source_copy_kernel::metadata_fields(&source)
        .expect("the maintained source field catalogue accepts the selected Agent");
    let forms: Value =
        serde_json::from_slice(&fs::read(repository.join(&forms_path)).unwrap()).unwrap();
    let form_selections = forms["forms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|form| {
            assert_eq!(form["content"]["kind"], "source-copy");
            assert_eq!(form["content"]["slot"], "wording");
            let pointer = form["bindings"]["wording"]["pointer"]
                .as_str()
                .expect("each real HumanForm binds its wording slot to a source pointer");
            let mut matching_fields = source_fields
                .iter()
                .filter(|field| field.pointer.as_str() == pointer);
            let field = matching_fields
                .next()
                .expect("each HumanForm wording pointer belongs to the maintained field catalogue");
            assert!(
                matching_fields.next().is_none(),
                "each HumanForm wording pointer identifies one maintained source field"
            );
            (
                form["form_id"].as_str().unwrap().to_owned(),
                field.id.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert!(!form_selections.is_empty());
    let creation_request_path = format!("{package}/source-create-request.json");
    let creation_request: Value =
        serde_json::from_slice(&fs::read(repository.join(creation_request_path)).unwrap())
            .expect("the selected actual Agent retains its maintained native creation request");
    let creation_form_selections = creation_request["forms"]
        .as_array()
        .expect("the maintained source creation request selects HumanForms")
        .iter()
        .map(|selection| {
            (
                selection["form_id"].as_str().unwrap().to_owned(),
                selection["field_id"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(creation_request["operation"], "source.create");
    assert_eq!(creation_request["record"]["record_id"], record["record_id"]);
    assert_eq!(
        form_selections, creation_form_selections,
        "field selections match the maintained producer request for this exact Agent"
    );
    Target {
        name: directory,
        source_path,
        record_id: record["record_id"].as_str().unwrap().to_owned(),
        forms: form_selections,
    }
}

fn owner_configuration(root: &Path, target: &Target) -> Vec<u8> {
    canonical(&json!({
        "schema_version":"tos_local_corpus_revision_owner_v1",
        "uid":rustix::process::getuid().as_raw(),
        "principal_id":format!("test:pg-source-revision:{}",target.name),
        "source_root":root,
        "source_path":target.source_path,
        "record_id":target.record_id,
        "authority_ref":"test:bounded-source-revision-transaction-contract",
        "allowed_form_ids":target.forms.iter().map(|(id,_)|id).collect::<Vec<_>>(),
        "allowed_fields":["preferred_label"],
        "allowed_operations":["record.revise"],
        "expires_at":"2099-01-01T00:00:00Z",
        "record_type":"agent"
    }))
}

fn revision_request(target: &Target, label: &str, operation: &str) -> Value {
    json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":operation,
        "fields":{"preferred_label":label},
        "forms":target.forms.iter().map(|(form_id,field_id)| json!({"form_id":form_id,"field_id":field_id})).collect::<Vec<_>>(),
        "reason":"Private PG transaction-contract fixture; no source, semantic, rights, or canon admission."
    })
}

fn prepare_revision(
    fixture: &mut Fixture,
    target: &Target,
    label: &str,
    command_id: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (
    crate::source_native_cli::revisions::PreparedPgRecordRevision,
    revision::SourceRevisionReadSet,
) {
    let owner_raw = owner_configuration(&fixture.store_root, target);
    let worker_path = std::env::var_os("TOS_SCHEMA_WORKER_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .map(|target| target.join("debug/tos-schema-worker"))
        })
        .expect("the pinned schema worker path is supplied to the feature test")
        .canonicalize()
        .expect("the exact selected schema worker exists");
    let invocation = json!({
        "schema_worker":{"absolute_path":worker_path,"sha256":Digest256::of_bytes(&fs::read(&worker_path).unwrap()).to_prefixed()},
        "budgets":{"worker_cpu_seconds":3,"worker_address_space_bytes":1_073_741_824,"max_schema_receipts":128,"max_schema_receipt_bytes":262_144}
    });
    let preview_request = revision_request(target, label, "prepare-revise");
    let preview_raw = canonical(&preview_request);
    let recorded_at = "2026-10-11T00:00:00+00:00";
    let (prepared, readset) = fixture
        .coordinator
        .with_source_revision_session(
            &fixture.store,
            &fixture.domain,
            JOB_ID,
            1,
            deadline,
            cancelled,
            |session| {
                let selection = session.selection().clone();
                let identity = selection.identity();
                let context =
                    crate::source_native_cli::revisions::context_flat_record_from_transport(
                        &owner_raw,
                        &preview_raw,
                        recorded_at,
                        &target.source_path,
                        selection.cohort.initial_revision(),
                        session,
                        &fixture.software,
                        &fixture.components,
                        512,
                        8 * 1024 * 1024,
                        1_048_576,
                        deadline,
                        cancelled,
                    )
                    .map_err(source_error)?;
                let preview = crate::source_native_cli::revisions::prepare_pg_record_revision(
                    &invocation,
                    context,
                    identity,
                    deadline,
                    cancelled,
                )
                .map_err(source_error)?;
                let preview_response: Value = serde_json::from_slice(
                    &crate::source_command::canonical(&preview.proposal().response)
                        .map_err(source_error)?,
                )
                .map_err(|_| DurableError::Corrupt("revision preview response JSON"))?;
                let mut request = revision_request(target, label, "record.revise");
                request["command_id"] = json!(command_id);
                for (field, prepared_field) in [
                    ("expected_configuration", "owner_configuration"),
                    ("expected_dependencies", "expected_dependencies"),
                    ("expected_source", "source"),
                    ("expected_revision", "revision"),
                ] {
                    request[field] = preview_response[prepared_field].clone();
                }
                let request_raw = canonical(&request);
                let context =
                    crate::source_native_cli::revisions::context_flat_record_from_transport(
                        &owner_raw,
                        &request_raw,
                        recorded_at,
                        &target.source_path,
                        selection.cohort.initial_revision(),
                        session,
                        &fixture.software,
                        &fixture.components,
                        512,
                        8 * 1024 * 1024,
                        1_048_576,
                        deadline,
                        cancelled,
                    )
                    .map_err(source_error)?;
                crate::source_native_cli::revisions::prepare_pg_record_revision(
                    &invocation,
                    context,
                    identity,
                    deadline,
                    cancelled,
                )
                .map_err(source_error)
            },
        )
        .expect("native revision preparation binds to the captured PG read epoch");
    (prepared, readset)
}

fn register_and_attach(
    fixture: &mut Fixture,
    prepared: &crate::source_native_cli::revisions::PreparedPgRecordRevision,
    readset: &revision::SourceRevisionReadSet,
    prepare_id: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceRevisionAttempt {
    let attempt = fixture
        .coordinator
        .register_source_revision(
            &fixture.store,
            prepared,
            readset,
            prepare_id,
            deadline,
            cancelled,
        )
        .expect("prepared source revision registers its exact PG readset");
    let identities = attempt
        .writes
        .iter()
        .enumerate()
        .map(|(slot, write)| ShadowWriteIdentity {
            member_slot: u32::try_from(slot).unwrap(),
            subject: &write.subject,
            expected_predecessor: write.expected_predecessor,
            proposed_revision: write.proposed_revision,
            exact_bytes: &write.exact_bytes,
        })
        .collect::<Vec<_>>();
    let members = seal_members(
        &fixture.store,
        &attempt.prepare_id,
        attempt.fence,
        REVISION,
        &identities,
    )
    .expect("each registered prepared revision seals its exact successor bytes");
    fixture
        .coordinator
        .attach_ready_profile_bound(
            &fixture.store,
            &attempt.domain,
            &attempt.prepare_id,
            attempt.fence,
            &members,
            REVISION,
            Some(attempt.delta),
            Some(&attempt.metadata),
            None,
        )
        .expect("each source revision is attached before either commit");
    assert_eq!(
        fixture
            .coordinator
            .resolve_attempt(&attempt.domain, &attempt.prepare_id)
            .unwrap(),
        AttemptResolution::Ready
    );
    attempt
}

fn commit(
    fixture: &mut Fixture,
    attempt: &SourceRevisionAttempt,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<DurableCommitReceipt> {
    let owner_check = || -> DurableResult<()> { Ok(()) };
    fixture
        .coordinator
        .commit_source_revision(
            &fixture.store,
            attempt,
            attempt.contract,
            &owner_check,
            deadline,
            cancelled,
        )
        .map(|(receipt, _)| receipt)
}

fn transaction_counts(fixture: &mut Fixture) -> (u64, i64, i64, i64) {
    let row = fixture
        .coordinator
        .client
        .query_one(
            "SELECT d.head_seq,
                    (SELECT count(*) FROM cmd2_receipt r WHERE r.domain=d.domain),
                    (SELECT count(*) FROM cmd2_history h WHERE h.domain=d.domain),
                    (SELECT count(*) FROM cmd2_outbox o WHERE o.domain=d.domain)
             FROM cmd2_domain d WHERE d.domain=$1",
            &[&fixture.domain],
        )
        .unwrap();
    (
        u64::try_from(row.get::<_, i64>(0)).unwrap(),
        row.get(1),
        row.get(2),
        row.get(3),
    )
}

fn history_projection_snapshot(fixture: &mut Fixture) -> Vec<(String, i64, Option<Vec<u8>>)> {
    fixture
        .coordinator
        .client
        .query(
            "SELECT subject,revision,inventory_projection FROM cmd2_history
             WHERE domain=$1 ORDER BY subject COLLATE \"C\",revision",
            &[&fixture.domain],
        )
        .unwrap()
        .into_iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect()
}

fn retained_projection_at(fixture: &mut Fixture, subject: &str, as_of_seq: i64) -> Option<Vec<u8>> {
    let query = format!(
        "SELECT inventory_projection FROM ({RETAINED_PROJECTIONS}) retained WHERE subject=$3"
    );
    fixture
        .coordinator
        .client
        .query_one(&query, &[&fixture.domain, &as_of_seq, &subject])
        .unwrap()
        .get(0)
}

#[test]
fn retained_projection_overlay_is_temporal_and_never_falls_back_when_stale() {
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let mut fixture = lab_fixture(deadline, &cancelled);
    let subject = fixture.targets[0].source_path.clone();
    let other_record = fixture.targets[1].clone();
    let metadata = fixture
        .coordinator
        .client
        .query_one(
            "SELECT revision,commit_seq,content_digest,inventory_projection
             FROM cmd2_history WHERE domain=$1 AND subject=$2
             ORDER BY revision DESC LIMIT 1",
            &[&fixture.domain, &subject],
        )
        .expect("bootstrap retained the selected real Agent metadata member");
    let source_revision: i64 = metadata.get(0);
    let source_commit_seq: i64 = metadata.get(1);
    let source_digest: String = metadata.get(2);
    let original_projection: Vec<u8> = metadata
        .get::<_, Option<Vec<u8>>>(3)
        .expect("the bootstrap Agent member has a bounded inventory projection");
    let mut overlay_projection: Value = serde_json::from_slice(&original_projection).unwrap();
    assert_eq!(canonical(&overlay_projection), original_projection);
    let events = overlay_projection
        .get_mut("events")
        .and_then(Value::as_object_mut)
        .expect("the maintained Agent projection has an events map");
    let payload = json!({"event_id":"test:projection-as-of-overlay"});
    let payload_digest = Digest256::of_bytes(&canonical(&payload)).to_hex();
    events.insert(
        "test:projection-as-of-overlay".to_owned(),
        json!({
            "payload":payload,
            "source_ref":subject,
            "source_line":1,
            "source_sha256":payload_digest,
        }),
    );
    let overlay_projection = canonical(&overlay_projection);
    assert_ne!(overlay_projection, original_projection);

    let before_seq: i64 = fixture
        .coordinator
        .client
        .query_one(
            "SELECT head_seq FROM cmd2_domain WHERE domain=$1",
            &[&fixture.domain],
        )
        .unwrap()
        .get(0);
    assert!(source_commit_seq <= before_seq);
    assert_eq!(
        retained_projection_at(&mut fixture, &subject, before_seq),
        Some(original_projection.clone()),
        "the actual bootstrap projection is visible before the overlay's effective cut"
    );

    let (prepared, readset) = prepare_revision(
        &mut fixture,
        &other_record,
        "Justus Lipsius creates a later source cut",
        "test:pg-inventory-overlay-as-of",
        deadline,
        &cancelled,
    );
    let attempt = register_and_attach(
        &mut fixture,
        &prepared,
        &readset,
        b"prepare:inventory-overlay-as-of",
        deadline,
        &cancelled,
    );
    let receipt = commit(&mut fixture, &attempt, deadline, &cancelled)
        .expect("a real revision of the independent Agent creates the later committed cut");
    verify_commit_evidence(&mut fixture, &attempt, &receipt, deadline, &cancelled);
    let later_seq = i64::try_from(receipt.commit_seq).unwrap();
    assert!(later_seq > before_seq);

    let definition_digest: String = fixture
        .coordinator
        .client
        .query_one(
            "SELECT source_definition_digest FROM cmd2_domain WHERE domain=$1",
            &[&fixture.domain],
        )
        .unwrap()
        .get(0);
    let history_before_overlay = history_projection_snapshot(&mut fixture);
    fixture
        .coordinator
        .client
        .execute(
            "INSERT INTO cmd2_source_inventory_projection(
                 domain,subject,effective_seq,source_revision,content_digest,
                 definition_digest,inventory_projection)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
            &[
                &fixture.domain,
                &subject,
                &later_seq,
                &source_revision,
                &source_digest,
                &definition_digest,
                &overlay_projection,
            ],
        )
        .expect("the overlay is keyed to existing history and a real committed log sequence");
    assert_eq!(
        retained_projection_at(&mut fixture, &subject, before_seq),
        Some(original_projection.clone()),
        "an overlay beyond the requested cut is not visible early"
    );
    assert_eq!(
        retained_projection_at(&mut fixture, &subject, later_seq),
        Some(overlay_projection.clone()),
        "the latest as-of cut uses the exact canonical overlay bytes"
    );
    assert_eq!(
        history_projection_snapshot(&mut fixture),
        history_before_overlay,
        "the overlay adds as-of projection state without rewriting retained history"
    );

    // A current projection can advance without rewriting its immutable source
    // history. Exact reads still verify every source locator/owner column and
    // the effective derived projection independently.
    fixture
        .coordinator
        .client
        .execute(
            "UPDATE cmd2_current SET inventory_projection=$3 WHERE domain=$1 AND subject=$2",
            &[&fixture.domain, &subject, &overlay_projection],
        )
        .unwrap();
    let (current_row, exact_bytes, _, _) = fixture
        .coordinator
        .read_committed_source_selected(
            &fixture.store,
            &fixture.domain,
            &subject,
            None,
            8_388_608,
            deadline,
            &cancelled,
            |_| Ok(()),
        )
        .expect("current exact read accepts the correctly bound effective projection");
    assert_eq!(Digest256::of_bytes(&exact_bytes).to_hex(), source_digest);
    assert_eq!(
        current_row.get::<_, Option<Vec<u8>>>("inventory_projection"),
        Some(overlay_projection)
    );

    let stale_digest = "0".repeat(64);
    assert_ne!(stale_digest, source_digest);
    fixture
        .coordinator
        .client
        .execute(
            "UPDATE cmd2_source_inventory_projection SET content_digest=$4
             WHERE domain=$1 AND subject=$2 AND effective_seq=$3",
            &[&fixture.domain, &subject, &later_seq, &stale_digest],
        )
        .expect("the schema allows a digest mismatch so the reader can fail closed");
    assert!(matches!(
        fixture.coordinator.read_committed_source_selected(
            &fixture.store,
            &fixture.domain,
            &subject,
            None,
            8_388_608,
            deadline,
            &cancelled,
            |_| Ok(()),
        ),
        Err(DurableError::Corrupt(
            "selected current metadata differs from history"
        ))
    ));
    assert_eq!(
        retained_projection_at(&mut fixture, &subject, before_seq),
        Some(original_projection),
        "the stale later overlay remains outside an earlier as-of cut"
    );
    assert_eq!(
        retained_projection_at(&mut fixture, &subject, later_seq),
        None,
        "the latest stale overlay is NULL rather than falling back to history bytes"
    );
    assert_eq!(
        history_projection_snapshot(&mut fixture),
        history_before_overlay,
        "rejecting a stale overlay leaves every retained history projection unchanged"
    );
}

fn verify_commit_evidence(
    fixture: &mut Fixture,
    attempt: &SourceRevisionAttempt,
    receipt: &DurableCommitReceipt,
    deadline: Instant,
    cancelled: &AtomicBool,
) {
    assert_eq!(receipt.prepare_id, attempt.prepare_id);
    assert_eq!(receipt.domain, attempt.domain);
    assert_eq!(receipt.command_id, attempt.command_id);
    assert_eq!(receipt.raw_request_digest, attempt.raw_request_digest);
    assert_eq!(receipt.delta_digest, attempt.delta);

    let receipt_row = fixture
        .coordinator
        .client
        .query_one(
            "SELECT prepare_id,commit_seq,raw_request_digest,delta_digest,members_root
             FROM cmd2_receipt WHERE domain=$1 AND command_id=$2",
            &[&attempt.domain, &attempt.command_id],
        )
        .unwrap();
    assert_eq!(receipt_row.get::<_, Vec<u8>>(0), attempt.prepare_id);
    assert_eq!(
        u64::try_from(receipt_row.get::<_, i64>(1)).unwrap(),
        receipt.commit_seq
    );
    assert_eq!(
        receipt_row.get::<_, String>(2),
        receipt.raw_request_digest.to_hex()
    );
    assert_eq!(
        receipt_row.get::<_, String>(3),
        receipt.delta_digest.to_hex()
    );
    assert_eq!(
        receipt_row.get::<_, String>(4),
        receipt.member_root.to_hex()
    );

    let log_row = fixture
        .coordinator
        .client
        .query_one(
            "SELECT event_kind,command_id,delta_digest,members_root
             FROM cmd2_log WHERE domain=$1 AND commit_seq=$2",
            &[&attempt.domain, &i64::try_from(receipt.commit_seq).unwrap()],
        )
        .unwrap();
    assert_eq!(log_row.get::<_, String>(0), "command");
    assert_eq!(log_row.get::<_, String>(1), receipt.command_id);
    assert_eq!(log_row.get::<_, String>(2), receipt.delta_digest.to_hex());
    assert_eq!(log_row.get::<_, String>(3), receipt.member_root.to_hex());

    let outbox_row = fixture
        .coordinator
        .client
        .query_one(
            "SELECT event_id FROM cmd2_outbox WHERE domain=$1 AND commit_seq=$2",
            &[&attempt.domain, &i64::try_from(receipt.commit_seq).unwrap()],
        )
        .unwrap();
    assert_eq!(
        outbox_row.get::<_, String>(0),
        format!("{}:{}", attempt.domain, receipt.commit_seq)
    );

    let history_rows = fixture
        .coordinator
        .client
        .query(
            "SELECT subject,revision,commit_seq,content_digest FROM cmd2_history
             WHERE domain=$1 AND prepare_id=$2 ORDER BY subject COLLATE \"C\"",
            &[&attempt.domain, &attempt.prepare_id],
        )
        .unwrap();
    assert_eq!(history_rows.len(), attempt.writes.len());
    for write in &attempt.writes {
        let row = history_rows
            .iter()
            .find(|row| row.get::<_, String>(0) == write.subject)
            .expect("each prepared write has its own committed history row");
        assert_eq!(
            u64::try_from(row.get::<_, i64>(1)).unwrap(),
            write.proposed_revision
        );
        assert_eq!(
            u64::try_from(row.get::<_, i64>(2)).unwrap(),
            receipt.commit_seq
        );
        assert_eq!(
            row.get::<_, String>(3),
            Digest256::of_bytes(&write.exact_bytes).to_hex()
        );

        let recovered = fixture
            .coordinator
            .cold_recover_exact(
                &fixture.store,
                &attempt.domain,
                &write.subject,
                write.proposed_revision,
            )
            .expect("committed history resolves to its sealed STO frame");
        assert_eq!(recovered.digest(), Digest256::of_bytes(&write.exact_bytes));
        assert_eq!(
            fixture
                .coordinator
                .warm_read_selected(&fixture.store, &recovered, 8_388_608)
                .unwrap(),
            write.exact_bytes,
            "historical STO bytes equal the prepared exact successor"
        );

        let path = RelativePath::parse(&write.subject).unwrap();
        let current = fixture
            .coordinator
            .read_current_source_member(
                &fixture.store,
                &fixture.cohort,
                &path,
                8_388_608,
                deadline,
                cancelled,
            )
            .expect("committed member is reachable through the current PG/STO source route");
        assert_eq!(current.raw, write.exact_bytes);
        assert_eq!(current.custody_revision, write.proposed_revision);
        assert_eq!(current.commit_seq, receipt.commit_seq);
    }
}

fn lab_fixture(deadline: Instant, cancelled: &AtomicBool) -> Fixture {
    let repository = fixture::repository();
    let root = tempfile::tempdir().unwrap();
    let source_members = repository_members(&repository, &["parmenides-of-elea", "justus-lipsius"]);
    let cut_root = root.path().join("snapshot");
    let (revision, cut) = fixture::cut(&source_members, &cut_root, deadline, cancelled);
    let targets = vec![
        target(&repository, "parmenides-of-elea"),
        target(&repository, "justus-lipsius"),
    ];
    let mut software_inputs = BTreeMap::new();
    for path in SOFTWARE_COMPONENTS {
        assert!(
            software_inputs
                .insert((*path).to_owned(), Vec::new())
                .is_none()
        );
    }
    let software_root = root.path().join("software");
    fs::create_dir(&software_root).unwrap();
    let (software, components) = fixture::software(
        &repository,
        &software_inputs,
        &software_root,
        deadline,
        cancelled,
    );
    let domain = format!(
        "tos-revision-concurrency-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let database_url = std::env::var("TOS_CMD_POSTGRES_URL")
        .expect("postgres-lab transaction contract uses its explicitly selected PostgreSQL URL");
    assert!(!database_url.trim().is_empty());
    let mut coordinator = DurablePgCoordinator::connect(&database_url).unwrap();
    coordinator.init_lab_schema().unwrap();
    coordinator
        .create_domain(&domain, Digest256::of_bytes(CONTRACT))
        .unwrap();
    coordinator.set_job_epoch(&domain, JOB_ID, 1).unwrap();
    let store_root = root.path().join("store");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&store_root)
        .expect("create the private existing root required by SegmentStore");
    let store = SegmentStore::initialize_empty(
        &store_root,
        domain.as_bytes(),
        SegmentLimits {
            max_segment_bytes: 8 * 1024 * 1024,
            max_frame_bytes: 1024 * 1024,
            max_frames: 64,
            max_journal_bytes: 1024 * 1024,
        },
    )
    .unwrap();
    let bootstrap_owner = owner_configuration(&store_root, &targets[0]);
    let bootstrap_request = canonical(&json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"describe"
    }));
    let files = components
        .members()
        .map(|member| crate::source_command::SourceFile {
            path: member.path.clone(),
            raw: software
                .read_selected_component(&components, &member.path, 2_097_152, deadline, cancelled)
                .unwrap(),
        })
        .collect();
    let context = CommandContext {
        base_revision: revision,
        configuration_raw: bootstrap_owner,
        request_raw: bootstrap_request,
        recorded_at: "2026-10-11T00:00:00+00:00".to_owned(),
        effective_uid: rustix::process::getuid().as_raw().into(),
        files,
    };
    let selected_membership = cut.stream(revision).unwrap().expectation();
    let mut bootstrap_worker = fixture::worker(&cut, deadline, cancelled);
    let bootstrap_owner_check = || -> crate::DurableResult<()> { Ok(()) };
    let reopened = coordinator
        .bootstrap_source_cohort(
            &store,
            &domain,
            &cut,
            revision,
            selected_membership,
            &context,
            &software,
            &components,
            &mut bootstrap_worker,
            Digest256::of_bytes(CONTRACT),
            JOB_ID,
            1,
            crate::source_cohort::BootstrapJobMode::RequireExisting,
            &bootstrap_owner_check,
            deadline,
            cancelled,
        )
        .expect("the selected finite source cut bootstraps through PG/STO");
    // Bootstrap's final cold reopen owns and completes the selected schema
    // worker before it publishes cohort completeness.
    Fixture {
        _root: root,
        coordinator,
        cohort: reopened.cohort,
        original_cut: cut,
        initial_revision: revision,
        initial_membership: selected_membership,
        initial_context: context,
        store,
        store_root,
        domain,
        software,
        components,
        targets,
    }
}

#[test]
fn selected_pg_payload_loan_reads_each_member_once_and_fences_body_and_success_exits() {
    use crate::source_native_cli::private_assessment::OperationBudget;
    use crate::source_native_cli::private_assessment::original_payer::with_original_pwe_session_durable;
    use crate::source_native_cli::PgGrowthCurrentInput;
    use tos_compiler::pwe_native::Limits;

    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    // Existing real source/cohort/STO fixture setup is outside this selected
    // invocation. No new issuer, profile, security grant or production data.
    let mut fixture = lab_fixture(deadline, &cancelled);
    let path = RelativePath::parse(&fixture.targets[0].source_path).unwrap();
    let paths = std::collections::BTreeSet::from([
        path.clone(), RelativePath::parse(&fixture.targets[1].source_path).unwrap(),
    ]);
    let expected = fixture.original_cut
        .read_member(fixture.initial_revision, &path, 8_388_608, deadline, &cancelled)
        .unwrap().raw;
    let expected_digest = Digest256::of_bytes(&expected);
    let inspection = expected.len().checked_mul(2)
        .and_then(|n| path.as_str().len().checked_mul(2).and_then(|p| n.checked_add(p)))
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<Digest256>() + 2 * std::mem::size_of::<u64>()))
        .unwrap();
    let database_url = std::env::var("TOS_CMD_POSTGRES_URL").unwrap();
    let mut conflicting = DurablePgCoordinator::connect(&database_url).unwrap();
    let domain = fixture.domain.clone();
    let mut job_fence = 1;
    for lane in 0..3 {
        let mut budget = OperationBudget::new(&json!({"budgets":{
            "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
        }}), deadline).unwrap();
        let limits = Limits {
            max_input_bytes: 4_194_304,
            max_nodes: 1024,
            max_work: 262_144,
            max_refs: 64,
            max_depth: 16,
            max_items: 64,
            max_state_bytes: 8_388_608,
            deadline,
        };
        let result = fixture.coordinator.with_source_revision_session_with_exit_data_admission(
            &fixture.store, &domain, JOB_ID, job_fence, deadline, &cancelled,
            &mut budget, |epoch, original| {
                let input = PgGrowthCurrentInput::new(epoch, 64, 8_388_608)
                    .map_err(|_| DurableError::Invalid("payload test selected input"))?;
                with_original_pwe_session_durable(original, limits, &cancelled, |payer, session| {
                    input.with_current_payload_loan(
                        payer, session, &paths, 8_388_608, |loan| {
                            if lane == 0 {
                                let unselected = RelativePath::parse(
                                    "ToS/contracts/semantic-entity-type-registry.schema.json"
                                ).unwrap();
                                assert!(matches!(loan.with_bytes(&unselected, |_, _| {
                                    panic!("existing but unselected payload must not be exposed")
                                }), Err(DurableError::Refused(
                                    "PG payload path outside selected loan"
                                ))));
                            }
                            let mut first_input_charge = None;
                            for round in 0..2 {
                                loan.with_bytes(&path, |payload, same| {
                                    // Assertions inspect the actual borrowed
                                    // bytes; pay that work on this SAME Session.
                                    same.admit_source_work(inspection)
                                        .map_err(|error| DurableError::Source(payer.error(error)))?;
                                    assert_eq!(payload.member().raw, expected);
                                    assert_eq!(payload.member().path, path);
                                    assert_eq!(payload.member().current_generation, payload.selected().head_seq);
                                    assert_eq!(payload.member().metadata.sha256, expected_digest);
                                    if let Some(before) = first_input_charge {
                                        assert_eq!(same.charges().0, before,
                                            "repeat borrowing is not another payload read");
                                    } else {
                                        first_input_charge = Some(same.charges().0);
                                    }
                                    if lane != 0 && round == 0 {
                                        conflicting.set_job_epoch(&domain, JOB_ID, job_fence + 1)?;
                                        if lane == 1 {
                                            return Err(DurableError::Source(
                                                SourceCommandError::Denied("payload test original body fault")
                                            ));
                                        }
                                    }
                                    Ok(())
                                })?;
                            }
                            Ok(())
                        },
                    )
                })
            },
        );
        match lane {
            0 => {
                let ((), readset) = result.unwrap();
                assert_eq!(readset.work.exact_source_members_read, paths.len() as u64);
                assert_eq!(readset.observations().len(), paths.len());
            }
            1 => assert!(matches!(result, Err(DurableError::Source(
                SourceCommandError::Denied("payload test original body fault")
            )))),
            _ => assert!(matches!(result, Err(DurableError::Conflict(
                "bounded source revision job lease changed"
            )))),
        }
        if lane != 0 { job_fence += 1; }
    }
}

// This is the positive simultaneous-borrow gate for the Common splitter,
// not a PWE role/worker/semantic admission fixture. It requires postgres-lab.
#[test]
fn selected_pg_payload_split_keeps_multiple_borrows_and_five_epoch_checks() {
    use crate::source_native_cli::private_assessment::OperationBudget;
    use crate::source_native_cli::private_assessment::original_payer::with_original_pwe_session_durable;
    use crate::source_native_cli::PgGrowthCurrentInput;
    use std::sync::atomic::Ordering;
    use tos_compiler::pwe_native::Limits;

    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let mut fixture = lab_fixture(deadline, &cancelled);
    let paths = std::collections::BTreeSet::from([
        RelativePath::parse(&fixture.targets[0].source_path).unwrap(),
        RelativePath::parse(&fixture.targets[1].source_path).unwrap(),
    ]);
    // Expected bytes come from the independent original fixture cut. Fixture
    // setup is outside the measured source invocation, as in the prior test.
    let expected: Vec<_> = paths.iter().map(|path| {
        let member = fixture.original_cut
            .read_member(fixture.initial_revision, path, 8_388_608, deadline, &cancelled)
            .unwrap();
        (path, member.raw)
    }).collect();
    let inspect = expected.iter().try_fold(0usize, |sum, (path, raw)| {
        raw.len().checked_add(path.as_str().len())
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| sum.checked_add(n))
    }).unwrap();
    let database_url = std::env::var("TOS_CMD_POSTGRES_URL").unwrap();
    let mut conflicting = DurablePgCoordinator::connect(&database_url).unwrap();
    let domain = fixture.domain.clone();
    let mut job_fence = 1;
    // Success; body fault + epoch conflict; successful body + epoch conflict;
    // swallowed mapped resource refusal; cancellation at callback exit.
    for lane in 0..5 {
        let mut budget = OperationBudget::new(&json!({"budgets":{
            "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
        }}), deadline).unwrap();
        let limits = Limits {
            max_input_bytes: 4_194_304,
            max_nodes: 1024,
            max_work: 262_144,
            max_refs: 64,
            max_depth: 16,
            max_items: 64,
            max_state_bytes: 8_388_608,
            deadline,
        };
        let result = fixture.coordinator.with_source_revision_session_with_exit_data_admission(
            &fixture.store, &domain, JOB_ID, job_fence, deadline, &cancelled,
            &mut budget, |epoch, original| {
                let input = PgGrowthCurrentInput::new(epoch, 64, 8_388_608)
                    .map_err(|_| DurableError::Invalid("split test selected input"))?;
                with_original_pwe_session_durable(original, limits, &cancelled, |payer, session| {
                    input.with_current_payload_loan(
                        payer, session, &paths, 8_388_608, |loan| {
                            loan.with_selected_payloads(|identity, members, epoch, same_payer, same| {
                                same_payer.inspect(
                                    2 * std::mem::size_of::<&crate::source_cohort::ManagedCurrentMember>()
                                        + std::mem::size_of::<crate::source_cohort::revision::PgRecordRevisionIdentity>(),
                                    0,
                                ).map_err(DurableError::Source)?;
                                assert!(std::ptr::eq(payer, same_payer));
                                assert!(same.has_source_payer(same_payer));
                                assert_eq!(members.len(), paths.len());
                                // Both member references and their raw slices
                                // stay live across all five mutable epoch and
                                // sequential SAME Session uses below.
                                let first = &members[0];
                                let second = &members[1];
                                let input_before = same.charges().0;
                                for _ in 0..5 {
                                    same.admit_source_work(inspect)
                                        .map_err(|error| DurableError::Source(same_payer.error(error)))?;
                                    assert_eq!(&first.path, expected[0].0);
                                    assert_eq!(&second.path, expected[1].0);
                                    assert_eq!(first.raw.as_slice(), expected[0].1.as_slice());
                                    assert_eq!(second.raw.as_slice(), expected[1].1.as_slice());
                                    assert_eq!(first.current_generation, identity.head_seq);
                                    assert_eq!(second.current_generation, identity.head_seq);
                                    // Same genuine epoch and original payer;
                                    // no whole-loan reentry or second Session.
                                    same_payer.reserve(
                                        std::mem::size_of::<Option<DurableError>>()
                                            + std::mem::size_of::<DurableResult<()>>(),
                                    ).map_err(DurableError::Source)?;
                                    let mut foreign = None;
                                    same_payer.with_operation(|operation| {
                                        match epoch.verify_fresh_with_query_data_admission(
                                            operation, deadline, &cancelled,
                                        ) {
                                            Ok(()) => Ok(()),
                                            Err(DurableError::Source(error)) => Err(error),
                                            Err(error) => { foreign = Some(error); Ok(()) },
                                        }
                                    }).map_err(DurableError::Source)?;
                                    if let Some(error) = foreign { return Err(error); }
                                }
                                assert_eq!(same.charges().0, input_before,
                                    "five epoch checks do not reread retained payloads");
                                if lane == 1 || lane == 2 {
                                    conflicting.set_job_epoch(&domain, JOB_ID, job_fence + 1)?;
                                    if lane == 1 {
                                        return Err(DurableError::Source(SourceCommandError::Denied(
                                            "split test original body fault",
                                        )));
                                    }
                                } else if lane == 3 {
                                    let refused = same.admit_source_work(usize::MAX).unwrap_err();
                                    // Mapping may consume the original error,
                                    // but cannot clear its sticky refusal.
                                    let _mapped = same_payer.error(refused);
                                } else if lane == 4 {
                                    cancelled.store(true, Ordering::SeqCst);
                                }
                                Ok(())
                            })
                        },
                    )
                })
            },
        );
        cancelled.store(false, Ordering::SeqCst);
        match lane {
            0 => {
                let ((), readset) = result.unwrap();
                assert_eq!(readset.work.exact_source_members_read, paths.len() as u64);
                assert_eq!(readset.observations().len(), paths.len());
            }
            1 => assert!(matches!(result, Err(DurableError::Source(
                SourceCommandError::Denied("split test original body fault")
            )))),
            2 => assert!(matches!(result, Err(DurableError::Conflict(
                "bounded source revision job lease changed"
            )))),
            3 | 4 => assert!(matches!(result, Err(DurableError::Source(_)))),
            _ => unreachable!(),
        }
        if lane == 1 || lane == 2 { job_fence += 1; }
    }
}

#[test]
fn prepared_and_attached_revisions_allow_disjoint_writes_and_conflict_on_same_record() {
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let mut fixture = lab_fixture(deadline, &cancelled);
    let parmenides = fixture.targets[0].clone();
    let lipsius = fixture.targets[1].clone();

    let (parmenides_prepared, parmenides_readset) = prepare_revision(
        &mut fixture,
        &parmenides,
        "Parmenides concurrency transaction fixture",
        "test:pg-concurrent-parmenides",
        deadline,
        &cancelled,
    );
    let (lipsius_prepared, lipsius_readset) = prepare_revision(
        &mut fixture,
        &lipsius,
        "Justus Lipsius concurrency transaction fixture",
        "test:pg-concurrent-lipsius",
        deadline,
        &cancelled,
    );
    let head_before_independent = parmenides_readset.selection.head_seq;
    assert_eq!(
        lipsius_readset.selection.head_seq, head_before_independent,
        "both independent proposals were prepared from the same source head"
    );
    let parmenides_attempt = register_and_attach(
        &mut fixture,
        &parmenides_prepared,
        &parmenides_readset,
        b"prepare:disjoint:parmenides",
        deadline,
        &cancelled,
    );
    let lipsius_attempt = register_and_attach(
        &mut fixture,
        &lipsius_prepared,
        &lipsius_readset,
        b"prepare:disjoint:lipsius",
        deadline,
        &cancelled,
    );
    let counts_before_independent = transaction_counts(&mut fixture);
    assert_eq!(
        counts_before_independent.0, head_before_independent,
        "both independent proposals are prepared and attached before the first commit"
    );
    let first = commit(&mut fixture, &parmenides_attempt, deadline, &cancelled)
        .expect("first independent source revision commits");
    verify_commit_evidence(
        &mut fixture,
        &parmenides_attempt,
        &first,
        deadline,
        &cancelled,
    );
    let second = commit(&mut fixture, &lipsius_attempt, deadline, &cancelled)
        .expect("the other prepared source revision commits after its unrelated predecessor set is unchanged");
    verify_commit_evidence(
        &mut fixture,
        &lipsius_attempt,
        &second,
        deadline,
        &cancelled,
    );
    assert_eq!(first.commit_seq, head_before_independent + 1);
    assert_eq!(second.commit_seq, head_before_independent + 2);
    assert_ne!(first.member_root, second.member_root);
    let counts_after_independent = transaction_counts(&mut fixture);
    assert_eq!(counts_after_independent.0, head_before_independent + 2);
    assert_eq!(counts_after_independent.1, counts_before_independent.1 + 2);
    assert_eq!(
        counts_after_independent.2,
        counts_before_independent.2
            + i64::try_from(parmenides_attempt.writes.len() + lipsius_attempt.writes.len())
                .unwrap()
    );
    assert_eq!(counts_after_independent.3, counts_before_independent.3 + 2);

    let (first_prepared, first_readset) = prepare_revision(
        &mut fixture,
        &parmenides,
        "Parmenides first same-object transaction fixture",
        "test:pg-same-record-first",
        deadline,
        &cancelled,
    );
    let (second_prepared, second_readset) = prepare_revision(
        &mut fixture,
        &parmenides,
        "Parmenides second same-object transaction fixture",
        "test:pg-same-record-second",
        deadline,
        &cancelled,
    );
    let head_before_same_object = first_readset.selection.head_seq;
    assert_eq!(
        second_readset.selection.head_seq, head_before_same_object,
        "both same-object proposals were prepared from the same source head"
    );
    let first_attempt = register_and_attach(
        &mut fixture,
        &first_prepared,
        &first_readset,
        b"prepare:same-record:first",
        deadline,
        &cancelled,
    );
    let second_attempt = register_and_attach(
        &mut fixture,
        &second_prepared,
        &second_readset,
        b"prepare:same-record:second",
        deadline,
        &cancelled,
    );
    let same_record_counts_before_first = transaction_counts(&mut fixture);
    assert_eq!(
        same_record_counts_before_first.0, head_before_same_object,
        "both same-object proposals are prepared and attached before the first commit"
    );
    let same_record_first = commit(&mut fixture, &first_attempt, deadline, &cancelled)
        .expect("first same-object revision commits");
    verify_commit_evidence(
        &mut fixture,
        &first_attempt,
        &same_record_first,
        deadline,
        &cancelled,
    );
    let counts_after_first = transaction_counts(&mut fixture);
    assert_eq!(counts_after_first.0, head_before_same_object + 1);
    let same_record_second = commit(&mut fixture, &second_attempt, deadline, &cancelled);
    assert!(matches!(same_record_second, Err(DurableError::Conflict(_))));
    assert_eq!(same_record_first.commit_seq, head_before_same_object + 1);
    assert_eq!(
        transaction_counts(&mut fixture),
        counts_after_first,
        "the stale losing attempt changes no current, history, receipt, or outbox rows"
    );
    verify_commit_evidence(
        &mut fixture,
        &first_attempt,
        &same_record_first,
        deadline,
        &cancelled,
    );
    assert_eq!(
        fixture
            .coordinator
            .resolve_attempt(&second_attempt.domain, &second_attempt.prepare_id)
            .unwrap(),
        AttemptResolution::Ready,
        "the losing attached attempt remains inspectable for explicit recovery"
    );

    let head_before_cold_reopen = transaction_counts(&mut fixture).0;
    let history_projection_before_cold = history_projection_snapshot(&mut fixture);
    let mut cold_reopen_worker = fixture::worker(&fixture.original_cut, deadline, &cancelled);
    let reopened = fixture
        .coordinator
        .cold_reopen_source_cohort(
            &fixture.store,
            &fixture.domain,
            &fixture.original_cut,
            fixture.initial_revision,
            fixture.initial_membership,
            &fixture.initial_context,
            &fixture.software,
            &fixture.components,
            &mut cold_reopen_worker,
            deadline,
            &cancelled,
        )
        .expect("cold reopen verifies the original source plus committed exact current/history");
    fixture.cohort = reopened.cohort;
    assert_eq!(transaction_counts(&mut fixture).0, head_before_cold_reopen);
    assert_eq!(
        history_projection_snapshot(&mut fixture),
        history_projection_before_cold,
        "cold reopen does not rewrite any retained history projection bytes"
    );

    let (after_reopen_prepared, after_reopen_readset) = prepare_revision(
        &mut fixture,
        &parmenides,
        "Parmenides revision after cold reopen",
        "test:pg-after-cold-reopen",
        deadline,
        &cancelled,
    );
    assert_eq!(
        after_reopen_readset.selection.head_seq, head_before_cold_reopen,
        "the post-reopen writer selects the reopened current head"
    );
    let after_reopen_attempt = register_and_attach(
        &mut fixture,
        &after_reopen_prepared,
        &after_reopen_readset,
        b"prepare:after-cold-reopen",
        deadline,
        &cancelled,
    );
    assert_eq!(transaction_counts(&mut fixture).0, head_before_cold_reopen);
    let after_reopen_receipt = commit(&mut fixture, &after_reopen_attempt, deadline, &cancelled)
        .expect("a new source revision commits successfully after cold reopen");
    assert_eq!(after_reopen_receipt.commit_seq, head_before_cold_reopen + 1);
    verify_commit_evidence(
        &mut fixture,
        &after_reopen_attempt,
        &after_reopen_receipt,
        deadline,
        &cancelled,
    );
}

struct PrivateOwnerReadRaceCapability {
    write: crate::durable_private_packages::PrivateOwnerPackageWriteV1,
    entered: Option<std::sync::mpsc::SyncSender<()>>,
    resume: Option<std::sync::mpsc::Receiver<()>>,
}

struct PrivateOwnerReadRaceReleaseGuard(Option<std::sync::mpsc::Sender<()>>);

impl PrivateOwnerReadRaceReleaseGuard {
    fn release(&mut self) -> Result<(), ()> {
        self.0
            .take()
            .map_or(Ok(()), |sender| sender.send(()).map_err(|_| ()))
    }
}

impl Drop for PrivateOwnerReadRaceReleaseGuard {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

impl crate::durable_private_packages::capability_seal::Sealed for PrivateOwnerReadRaceCapability {}

impl crate::durable_private_packages::PrivateOwnerPackageCapabilityV1
    for PrivateOwnerReadRaceCapability
{
    fn write(&self) -> &crate::durable_private_packages::PrivateOwnerPackageWriteV1 {
        &self.write
    }

    fn revalidate(
        &mut self,
        current: Option<&crate::durable_private_packages::PrivateOwnerPackageSnapshotV1>,
        replay: Option<&crate::durable_private_packages::PrivateOwnerPackageReplayV1>,
    ) -> crate::DurableResult<()> {
        if current != self.write.expected_current.as_ref() || replay.is_some() {
            return Err(crate::DurableError::Conflict(
                "private read-race fixture owner state changed",
            ));
        }
        if let (Some(entered), Some(resume)) = (self.entered.take(), self.resume.take()) {
            entered
                .send(())
                .map_err(|_| crate::DurableError::Refused("private read-race writer stopped"))?;
            resume
                .recv()
                .map_err(|_| crate::DurableError::Refused("private read-race release stopped"))?;
        }
        Ok(())
    }
}

fn private_read_race_source_binding(
    database_url: &str,
    domain: &str,
    private_owner_audit_generation: u64,
) -> crate::durable_private_packages::PrivateOwnerSourceBindingV1 {
    use crate::durable_private_packages::PrivateOwnerSourceBindingV1;
    use postgres::{Client, NoTls};

    let mut client = Client::connect(database_url, NoTls).unwrap();
    let row = client
        .query_one(
            "SELECT d.head_seq,d.source_epoch,d.source_generation,d.source_revision,
                    d.rights_version,d.rule_version,d.contract_digest,f.generation,
                    j.fence_epoch
             FROM cmd2_domain d
             JOIN cmd2_audit_fence f ON f.domain=d.domain
             JOIN cmd2_job j ON j.domain=d.domain AND j.job_id=$2
             WHERE d.domain=$1",
            &[&domain, &JOB_ID],
        )
        .unwrap();
    let digest = |index| {
        Digest256::from_hex(row.get::<_, String>(index).trim())
            .expect("selected source binding contains a valid digest")
    };
    PrivateOwnerSourceBindingV1 {
        head_seq: u64::try_from(row.get::<_, i64>(0)).unwrap(),
        source_epoch: u64::try_from(row.get::<_, Option<i64>>(1).unwrap()).unwrap(),
        source_generation: u64::try_from(row.get::<_, Option<i64>>(2).unwrap()).unwrap(),
        source_revision: digest(3),
        source_audit_generation: u64::try_from(row.get::<_, i64>(7)).unwrap(),
        rights_version: u64::try_from(row.get::<_, i64>(4)).unwrap(),
        rule_version: u64::try_from(row.get::<_, i64>(5)).unwrap(),
        contract_digest: digest(6),
        job_id: JOB_ID.to_owned(),
        job_fence: u64::try_from(row.get::<_, i64>(8)).unwrap(),
        readset_digest: Digest256::of_bytes(b"private lock regression readset"),
        dependency_digest: Digest256::of_bytes(b"private lock regression dependencies"),
        private_owner_audit_generation,
    }
}

fn private_read_race_write(
    scope: crate::durable_private_packages::PrivateOwnerScopeV1,
    target_ref: &str,
    command_id: &str,
    expected_current: Option<crate::durable_private_packages::PrivateOwnerPackageSnapshotV1>,
    member_bytes: &[u8],
    source_binding: crate::durable_private_packages::PrivateOwnerSourceBindingV1,
) -> crate::durable_private_packages::PrivateOwnerPackageWriteV1 {
    use crate::durable_private_packages::{
        PrivateOwnerPackageFamilyV1, PrivateOwnerPackageWriteV1,
    };

    PrivateOwnerPackageWriteV1 {
        scope,
        family: PrivateOwnerPackageFamilyV1::Text,
        target_ref: target_ref.to_owned(),
        expected_current,
        members: BTreeMap::from([("owner.txt".to_owned(), member_bytes.to_vec())]),
        archives: Vec::new(),
        command_id: command_id.to_owned(),
        request_digest: Digest256::of_bytes(command_id.as_bytes()),
        raw_request_digest: Digest256::of_bytes(format!("raw:{command_id}").as_bytes()),
        owner_configuration_digest: Digest256::of_bytes(b"private lock regression owner config"),
        source_binding,
        response_bytes: b"{\"status\":\"private fixture\"}".to_vec(),
    }
}

#[test]
fn private_owner_read_waits_for_writer_commit_before_repeatable_read_snapshot() {
    use crate::durable_private_packages::PrivateOwnerScopeV1;
    use postgres::{Client, NoTls};
    use std::sync::mpsc;

    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let fixture = lab_fixture(deadline, &cancelled);
    let database_url = std::env::var("TOS_CMD_POSTGRES_URL")
        .expect("private owner race test uses the selected PostgreSQL fixture");
    let scope = PrivateOwnerScopeV1::for_context(
        &fixture.domain,
        Digest256::of_bytes(b"private owner lock ordering regression context"),
    )
    .unwrap();
    let target_ref = "ToS/source-witnesses/private-lock-regression/owner";

    let mut writer = DurablePgCoordinator::connect(&database_url).unwrap();
    let initial = private_read_race_write(
        scope.clone(),
        target_ref,
        "test:private-lock-regression:initial",
        None,
        b"old accepted private bytes",
        private_read_race_source_binding(&database_url, &fixture.domain, 0),
    );
    let mut initial_capability = PrivateOwnerReadRaceCapability {
        write: initial,
        entered: None,
        resume: None,
    };
    writer
        .commit_private_owner_package(&mut initial_capability)
        .expect("initial owner package is committed through the private package broker");
    let old_snapshot = writer
        .read_private_owner_current(&scope, target_ref)
        .unwrap()
        .expect("initial private current package exists");
    assert_eq!(
        old_snapshot.members["owner.txt"],
        b"old accepted private bytes"
    );

    let (writer_entered_tx, writer_entered_rx) = mpsc::sync_channel(0);
    let (writer_resume_tx, writer_resume_rx) = mpsc::channel();
    let mut writer_release = PrivateOwnerReadRaceReleaseGuard(Some(writer_resume_tx));
    let update = private_read_race_write(
        scope.clone(),
        target_ref,
        "test:private-lock-regression:update",
        Some(old_snapshot.clone()),
        b"new accepted private bytes",
        private_read_race_source_binding(
            &database_url,
            &fixture.domain,
            old_snapshot.private_audit_generation,
        ),
    );
    let writer_url = database_url.clone();
    let writer_handle = thread::spawn(move || {
        let mut coordinator = DurablePgCoordinator::connect(&writer_url).unwrap();
        let mut capability = PrivateOwnerReadRaceCapability {
            write: update,
            entered: Some(writer_entered_tx),
            resume: Some(writer_resume_rx),
        };
        coordinator.commit_private_owner_package(&mut capability)
    });
    writer_entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("writer capability is paused after acquiring the private owner lock");

    let (reader_started_tx, reader_started_rx) = mpsc::sync_channel(0);
    let (reader_callback_tx, reader_callback_rx) = mpsc::channel();
    let reader_url = database_url.clone();
    let reader_scope = scope.clone();
    let reader_target = target_ref.to_owned();
    let reader_handle = thread::spawn(move || {
        let mut client = Client::connect(&reader_url, NoTls).unwrap();
        let reader_pid: i32 = client
            .query_one("SELECT pg_backend_pid()", &[])
            .unwrap()
            .get(0);
        let mut coordinator = DurablePgCoordinator::from_configured_client(client);
        let started = Instant::now();
        reader_started_tx.send(reader_pid).unwrap();
        let cancelled = AtomicBool::new(false);
        let result = coordinator.with_private_owner_read_session(
            &reader_scope,
            Instant::now() + Duration::from_secs(10),
            &cancelled,
            |session, session_deadline, session_cancelled| {
                reader_callback_tx.send(()).unwrap();
                let generation = session.private_audit_generation();
                let header = session
                    .current_header(&reader_target, session_deadline, session_cancelled)?
                    .expect("current package header exists after writer commit");
                let member = session
                    .read_current_member(
                        &reader_target,
                        "owner.txt",
                        1024,
                        session_deadline,
                        session_cancelled,
                    )?
                    .expect("current member exists after writer commit");
                Ok((generation, header, member))
            },
        );
        (started.elapsed(), result)
    });
    let reader_pid = reader_started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("reader is ready to enter the private owner read API");

    // The writer remains paused inside the real package broker after it has
    // taken the exclusive scope lock. Observe the reader's production
    // shared-lock try before allowing the writer to commit. PostgreSQL keeps
    // the last query text visible while the reader backs off, even when its
    // session state is idle between attempts.
    let mut observer = Client::connect(&database_url, NoTls).unwrap();
    let wait_deadline = Instant::now() + Duration::from_secs(5);
    let mut observed_shared_lock_try = false;
    while Instant::now() < wait_deadline {
        let shared_lock_try: bool = observer
            .query_one(
                "SELECT EXISTS (
                   SELECT 1 FROM pg_stat_activity
                   WHERE pid=$1 AND query LIKE 'SELECT pg_try_advisory_lock_shared%'
                 )",
                &[&reader_pid],
            )
            .unwrap()
            .get(0);
        if shared_lock_try {
            observed_shared_lock_try = true;
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    let callback_ran_before_commit = matches!(
        reader_callback_rx.recv_timeout(Duration::from_millis(25)),
        Ok(())
    );
    writer_release
        .release()
        .expect("paused owner writer is released to commit");
    let write_receipt = writer_handle
        .join()
        .expect("private package writer thread completes")
        .expect("replacement private package commits through the broker");
    let (reader_wait, read_result) = reader_handle
        .join()
        .expect("private owner reader thread completes");
    let (generation, header, selected_member) =
        read_result.expect("reader observes the committed current package");

    assert!(
        observed_shared_lock_try,
        "reader tried to acquire the shared owner lock while the writer still held it"
    );
    assert!(
        !callback_ran_before_commit,
        "reader did not open its repeatable-read owner view before the writer committed"
    );
    assert!(
        reader_wait >= Duration::from_millis(25),
        "read operation remained fenced until the writer committed"
    );
    assert_eq!(generation, write_receipt.private_audit_generation_after);
    assert_eq!(header.revision, 2);
    assert_eq!(header.package_digest, write_receipt.package_digest);
    assert_eq!(header.commit_seq, write_receipt.commit_seq);
    assert_eq!(selected_member.revision, 2);
    assert_eq!(selected_member.private_audit_generation, generation);
    assert_eq!(selected_member.bytes, b"new accepted private bytes");
    assert_ne!(selected_member.bytes, b"old accepted private bytes");
}
