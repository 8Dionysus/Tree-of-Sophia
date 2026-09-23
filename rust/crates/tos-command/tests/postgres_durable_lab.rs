//! Real PostgreSQL + STO private shadow laboratory. The fixture only writes
//! synthetic records in an isolated database/domain and temporary byte store.
//! Set TOS_CMD_POSTGRES_URL for a dedicated ephemeral PostgreSQL instance.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Cursor, Read};
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postgres::{Client, NoTls};
use tos_command::{
    AttemptResolution, CancelOutcome, CommitShadowAttempt, DurableError, DurablePgCoordinator,
    DurableShadowMember, RegisterShadowAttempt, ShadowWriteIdentity, durable_shadow_delta,
    durable_shadow_delta_prepared, lab_record_bytes,
};
use tos_foundation::Digest256;
use tos_segment_store::{
    AttemptRecovery, FrameInput, OwnerBinding, SegmentLimits, SegmentStore, VerificationBudget,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
static INIT_SCHEMA: Once = Once::new();

const PROFILE_ID: &[u8] = b"cmd2.lab.embedded-revision";
const PROFILE_VERSION: &[u8] = b"1";

struct ScratchRoot(PathBuf);

impl ScratchRoot {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let name = format!(
            "tos-cmd2-sto-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::var_os("TOS_CMD2_LAB_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(name);
        fs::create_dir_all(&path).expect("private STO lab root created");
        Self(path)
    }
}

impl Drop for ScratchRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("private STO lab root removed");
    }
}

fn limits() -> SegmentLimits {
    SegmentLimits {
        max_segment_bytes: 8 * 1024 * 1024,
        max_frame_bytes: 1024 * 1024,
        max_frames: 64,
        max_journal_bytes: 1024 * 1024,
    }
}

fn database_url() -> Option<String> {
    std::env::var("TOS_CMD_POSTGRES_URL").ok()
}

fn unique_domain() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    format!(
        "cmd2-private-{}-{timestamp}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    )
}

fn seal(
    store: &SegmentStore,
    prepare_id: &[u8],
    attempt_fence: u64,
    records: &[(&str, Vec<u8>)],
) -> Vec<tos_segment_store::ByteDurabilityReceipt> {
    let mut readers: Vec<_> = records
        .iter()
        .map(|(_, bytes)| Cursor::new(bytes.as_slice()))
        .collect();
    let mut frames: Vec<_> = records
        .iter()
        .zip(readers.iter_mut())
        .enumerate()
        .map(|(slot, ((subject, bytes), reader))| FrameInput {
            binding: OwnerBinding {
                profile_id: PROFILE_ID.to_vec(),
                profile_version: PROFILE_VERSION.to_vec(),
                subject_key: subject.as_bytes().to_vec(),
                member_slot: slot as u32,
            },
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(bytes),
            reader,
        })
        .collect();
    store
        .seal_segment_fenced(prepare_id, attempt_fence, 0, &mut frames)
        .expect("exact private frames sealed")
}

fn scalar_count(url: &str, table: &str, domain: &str) -> i64 {
    let mut client = Client::connect(url, NoTls).expect("lab PostgreSQL connects");
    let statement = match table {
        "attempt" => "SELECT count(*) FROM cmd2_attempt WHERE domain=$1",
        "member" => "SELECT count(*) FROM cmd2_member WHERE domain=$1",
        "current" => "SELECT count(*) FROM cmd2_current WHERE domain=$1",
        "history" => "SELECT count(*) FROM cmd2_history WHERE domain=$1",
        "receipt" => "SELECT count(*) FROM cmd2_receipt WHERE domain=$1",
        "log" => "SELECT count(*) FROM cmd2_log WHERE domain=$1",
        "outbox" => "SELECT count(*) FROM cmd2_outbox WHERE domain=$1",
        _ => panic!("unknown CMD.2 table"),
    };
    client.query_one(statement, &[&domain]).unwrap().get(0)
}

fn registered_fence(url: &str, domain: &str, prepare_id: &[u8]) -> u64 {
    let mut client = Client::connect(url, NoTls).expect("lab PostgreSQL connects");
    let value: i64 = client
        .query_one(
            "SELECT attempt_fence FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &prepare_id],
        )
        .expect("durable attempt exists")
        .get(0);
    value.try_into().expect("positive attempt fence")
}

fn copy_store_tree(source: &std::path::Path, target: &std::path::Path) {
    for entry in fs::read_dir(source).expect("source store directory opens") {
        let entry = entry.expect("source store entry reads");
        let destination = target.join(entry.file_name());
        let kind = entry.file_type().expect("source store entry type reads");
        if kind.is_dir() {
            fs::create_dir(&destination).expect("copied store directory created");
            copy_store_tree(&entry.path(), &destination);
        } else if kind.is_file() {
            fs::copy(entry.path(), destination).expect("exact store file copied");
        } else {
            panic!("unexpected non-regular store entry");
        }
    }
}

struct PausingReader {
    cursor: Cursor<Vec<u8>>,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    paused: bool,
}

impl Read for PausingReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if !self.paused {
            self.paused = true;
            self.entered
                .send(())
                .map_err(|_| io::Error::other("seal checkpoint observer gone"))?;
            self.release
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| io::Error::other("seal checkpoint release timed out"))?;
        }
        self.cursor.read(buffer)
    }
}

fn contract_digest() -> Digest256 {
    Digest256::of_bytes(b"cmd2.private.shadow.contract.v1")
}

struct Lab {
    db: DurablePgCoordinator,
    store: SegmentStore,
    _root: ScratchRoot,
    domain: String,
    url: String,
    attempt_fences: HashMap<Vec<u8>, u64>,
}

impl Lab {
    fn new(url: &str) -> Self {
        let mut db = DurablePgCoordinator::connect(url).expect("lab PostgreSQL connects");
        INIT_SCHEMA.call_once(|| db.init_lab_schema().expect("CMD.2 schema initializes"));
        let domain = unique_domain();
        db.create_domain(&domain, contract_digest())
            .expect("private domain created");
        db.set_job_epoch(&domain, "private-job", 1)
            .expect("private job fence initialized");
        let root = ScratchRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, domain.as_bytes(), limits())
            .expect("private STO store initialized");
        Self {
            db,
            store,
            _root: root,
            domain,
            url: url.to_owned(),
            attempt_fences: HashMap::new(),
        }
    }

    fn head_seq(&self) -> u64 {
        let mut client = Client::connect(&self.url, NoTls).unwrap();
        let value: i64 = client
            .query_one(
                "SELECT head_seq FROM cmd2_domain WHERE domain=$1",
                &[&self.domain],
            )
            .unwrap()
            .get(0);
        value.try_into().unwrap()
    }

    fn count(&self, table: &str) -> i64 {
        scalar_count(&self.url, table, &self.domain)
    }

    fn prepare(
        &mut self,
        prepare_id: &[u8],
        command_id: &str,
        specs: &[MemberSpec<'_>],
    ) -> Vec<DurableShadowMember> {
        let records: Vec<_> = specs
            .iter()
            .map(|spec| {
                (
                    spec.subject,
                    lab_record_bytes(spec.subject, spec.revision, spec.payload),
                )
            })
            .collect();
        let identities: Vec<_> = specs
            .iter()
            .zip(records.iter())
            .enumerate()
            .map(|(slot, (spec, (_, bytes)))| ShadowWriteIdentity {
                member_slot: slot as u32,
                subject: spec.subject,
                expected_predecessor: spec.predecessor,
                proposed_revision: spec.revision,
                exact_bytes: bytes,
            })
            .collect();
        let attempt_fence = self
            .db
            .register_attempt(&RegisterShadowAttempt {
                domain: &self.domain,
                prepare_id,
                command_id,
                raw_request_digest: Digest256::of_bytes(command_id.as_bytes()),
                delta_digest: durable_shadow_delta_prepared(&identities),
            })
            .expect("private attempt registered before STO seal");
        self.attempt_fences
            .insert(prepare_id.to_vec(), attempt_fence);
        let receipts = seal(&self.store, prepare_id, attempt_fence, &records);
        let members: Vec<_> = specs
            .iter()
            .zip(records)
            .zip(receipts)
            .enumerate()
            .map(
                |(slot, ((spec, (_, exact_bytes)), receipt))| DurableShadowMember {
                    member_slot: slot as u32,
                    subject: spec.subject.to_owned(),
                    expected_predecessor: spec.predecessor,
                    proposed_revision: spec.revision,
                    exact_bytes,
                    receipt,
                },
            )
            .collect();
        self.db
            .attach_ready(
                &self.store,
                &self.domain,
                prepare_id,
                attempt_fence,
                &members,
            )
            .expect("exact STO locators attached");
        members
    }

    fn commit(
        &mut self,
        prepare_id: &[u8],
        members: &[DurableShadowMember],
        full_base_seq: u64,
        job_fence: u64,
    ) -> tos_command::DurableResult<tos_command::DurableCommitReceipt> {
        let receipts: Vec<_> = members
            .iter()
            .map(|member| member.receipt.clone())
            .collect();
        let request = CommitShadowAttempt {
            domain: &self.domain,
            prepare_id,
            attempt_fence: *self
                .attempt_fences
                .get(prepare_id)
                .expect("registered attempt fence known"),
            receipts: &receipts,
            expected_contract_digest: contract_digest(),
            expected_rule_version: 0,
            expected_rights_version: 0,
            job_id: "private-job",
            job_fence,
            full_base_seq,
        };
        self.db
            .commit_shadow(&self.store, &request)
            .map(|(receipt, _)| receipt)
    }
}

struct MemberSpec<'a> {
    subject: &'a str,
    revision: u64,
    predecessor: Option<(u64, Digest256)>,
    payload: &'a [u8],
}

impl<'a> MemberSpec<'a> {
    fn first(subject: &'a str, payload: &'a [u8]) -> Self {
        Self {
            subject,
            revision: 1,
            predecessor: None,
            payload,
        }
    }
}

#[test]
fn exact_locator_commit_replay_and_command_collision() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-A",
        "command-A",
        &[MemberSpec::first("subject-A", b"private alpha")],
    );
    assert_eq!(lab.count("attempt"), 1);
    assert_eq!(lab.count("member"), 1);
    assert_eq!(lab.count("current"), 0);
    assert_eq!(lab.count("receipt"), 0);
    let committed = lab.commit(b"prepare-A", &members, 0, 1).unwrap();
    assert_eq!(committed.commit_seq, 1);
    assert!(!committed.replayed);
    let replay = lab.commit(b"prepare-A", &members, 0, 1).unwrap();
    assert_eq!(replay.commit_seq, 1);
    assert_eq!(replay.member_root, committed.member_root);
    assert!(replay.replayed);
    assert_eq!(lab.head_seq(), 1);
    for table in ["current", "history", "receipt", "log", "outbox"] {
        assert_eq!(lab.count(table), 1, "{table} did not commit exactly once");
    }
    let mut client = Client::connect(&url, NoTls).unwrap();
    let row = client
        .query_one(
            "SELECT c.store_id,c.custody_domain,c.pin_id,c.pin_fence,
                    c.segment_digest,c.segment_size,c.frame_index,c.frame_header_offset,
                    c.frame_digest,c.frame_length,c.sto_receipt_id,
                    h.sto_receipt_id,a.state,a.commit_seq
             FROM cmd2_current c
             JOIN cmd2_history h ON h.domain=c.domain AND h.subject=c.subject AND h.revision=c.revision
             JOIN cmd2_attempt a ON a.domain=c.domain AND a.prepare_id=c.prepare_id
             WHERE c.domain=$1 AND c.subject='subject-A'",
            &[&lab.domain],
        )
        .unwrap();
    let receipt = &members[0].receipt;
    let coordinate = receipt.coordinate();
    assert_eq!(row.get::<_, Vec<u8>>(0), receipt.store_id());
    assert_eq!(row.get::<_, Vec<u8>>(1), lab.domain.as_bytes());
    assert_eq!(row.get::<_, Vec<u8>>(2), receipt.pin_id());
    assert_eq!(row.get::<_, i64>(3), receipt.fence_epoch() as i64);
    assert_eq!(row.get::<_, String>(4), receipt.segment_digest().to_hex());
    assert_eq!(row.get::<_, i64>(5), receipt.segment_size() as i64);
    assert_eq!(row.get::<_, i32>(6), receipt.frame_index() as i32);
    assert_eq!(row.get::<_, i64>(7), coordinate.header_offset as i64);
    assert_eq!(row.get::<_, String>(8), coordinate.sha256.to_hex());
    assert_eq!(row.get::<_, i64>(9), coordinate.size_bytes as i64);
    assert_eq!(row.get::<_, String>(10), receipt.receipt_id().to_hex());
    assert_eq!(row.get::<_, String>(11), receipt.receipt_id().to_hex());
    assert_eq!(row.get::<_, String>(12), "committed");
    assert_eq!(row.get::<_, i64>(13), 1);

    let collision = lab.db.register_attempt(&RegisterShadowAttempt {
        domain: &lab.domain,
        prepare_id: b"prepare-A",
        command_id: "command-A",
        raw_request_digest: Digest256::of_bytes(b"different request"),
        delta_digest: durable_shadow_delta(&members),
    });
    assert!(matches!(collision, Err(DurableError::Conflict(_))));
    assert_eq!(lab.count("outbox"), 1);
}

#[test]
fn replay_rejects_forged_outbox_and_receipt_identity() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-replay-integrity",
        "replay-integrity",
        &[MemberSpec::first("subject-I", b"identity bytes")],
    );
    lab.commit(b"prepare-replay-integrity", &members, 0, 1)
        .unwrap();
    let mut corrupter = Client::connect(&url, NoTls).unwrap();
    let original_event: String = corrupter
        .query_one(
            "SELECT event_id FROM cmd2_outbox WHERE domain=$1 AND commit_seq=1",
            &[&lab.domain],
        )
        .unwrap()
        .get(0);
    corrupter
        .execute(
            "UPDATE cmd2_outbox SET event_id='forged-replay-event' WHERE domain=$1 AND commit_seq=1",
            &[&lab.domain],
        )
        .unwrap();
    assert!(matches!(
        lab.commit(b"prepare-replay-integrity", &members, 0, 1),
        Err(DurableError::Corrupt(_))
    ));
    corrupter
        .execute(
            "UPDATE cmd2_outbox SET event_id=$2 WHERE domain=$1 AND commit_seq=1",
            &[&lab.domain, &original_event],
        )
        .unwrap();
    let forged = Digest256::of_bytes(b"forged-receipt").to_hex();
    corrupter
        .execute(
            "UPDATE cmd2_receipt SET receipt_digest=$2 WHERE domain=$1 AND command_id='replay-integrity'",
            &[&lab.domain, &forged],
        )
        .unwrap();
    corrupter
        .execute(
            "UPDATE cmd2_attempt SET receipt_digest=$2 WHERE domain=$1 AND prepare_id=$3",
            &[
                &lab.domain,
                &forged,
                &b"prepare-replay-integrity".as_slice(),
            ],
        )
        .unwrap();
    assert!(matches!(
        lab.commit(b"prepare-replay-integrity", &members, 0, 1),
        Err(DurableError::Corrupt(_))
    ));
}

#[test]
fn stale_job_and_full_base_refuse_without_partial_commit() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let stale_lease = lab.prepare(
        b"prepare-lease",
        "stale-lease",
        &[MemberSpec::first("subject-L", b"private lease")],
    );
    lab.db.set_job_epoch(&lab.domain, "private-job", 2).unwrap();
    assert!(matches!(
        lab.commit(b"prepare-lease", &stale_lease, 0, 1),
        Err(DurableError::Refused(_))
    ));
    assert_eq!(lab.count("current"), 0);
    assert_eq!(lab.count("receipt"), 0);
    assert_eq!(lab.count("outbox"), 0);
    let first = lab.prepare(
        b"prepare-first",
        "first",
        &[MemberSpec::first("subject-A", b"private first")],
    );
    lab.commit(b"prepare-first", &first, 0, 2).unwrap();
    assert!(matches!(
        lab.commit(b"prepare-lease", &stale_lease, 0, 2),
        Err(DurableError::Conflict(_))
    ));
    assert_eq!(lab.head_seq(), 1);
    assert_eq!(lab.count("history"), 1);
}

#[test]
fn compound_second_member_conflict_rolls_back_every_projection() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"prepare-existing",
        "existing",
        &[MemberSpec::first("subject-A", b"original")],
    );
    lab.commit(b"prepare-existing", &first, 0, 1).unwrap();
    let compound = lab.prepare(
        b"prepare-compound",
        "compound",
        &[
            MemberSpec::first("subject-B", b"would be first insert"),
            MemberSpec::first("subject-A", b"conflicts at second member"),
        ],
    );
    assert_eq!(compound[0].receipt.pin_id(), compound[1].receipt.pin_id());
    assert!(matches!(
        lab.commit(b"prepare-compound", &compound, 1, 1),
        Err(DurableError::Conflict(_))
    ));
    assert_eq!(lab.head_seq(), 1);
    assert_eq!(lab.count("current"), 1);
    assert_eq!(lab.count("history"), 1);
    assert_eq!(lab.count("receipt"), 1);
    assert_eq!(lab.count("log"), 1);
    assert_eq!(lab.count("outbox"), 1);
    let mut client = Client::connect(&url, NoTls).unwrap();
    let escaped: i64 = client
        .query_one(
            "SELECT count(*) FROM cmd2_current WHERE domain=$1 AND subject='subject-B'",
            &[&lab.domain],
        )
        .unwrap()
        .get(0);
    assert_eq!(escaped, 0);
}

#[test]
fn different_prepare_cannot_reuse_command_identity() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"prepare-original",
        "same-command",
        &[MemberSpec::first("subject-A", b"original")],
    );
    lab.commit(b"prepare-original", &first, 0, 1).unwrap();
    let collision = lab.db.register_attempt(&RegisterShadowAttempt {
        domain: &lab.domain,
        prepare_id: b"prepare-different",
        command_id: "same-command",
        raw_request_digest: Digest256::of_bytes(b"different request"),
        delta_digest: Digest256::of_bytes(b"different delta"),
    });
    assert!(matches!(collision, Err(DurableError::Conflict(_))));
    assert_eq!(lab.count("attempt"), 1);
    assert_eq!(lab.count("receipt"), 1);
}

#[test]
fn embedded_revision_and_owner_binding_fail_before_attachment() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let bytes = lab_record_bytes("subject-A", 1, b"private payload");
    let attempt_fence = lab
        .db
        .register_attempt(&RegisterShadowAttempt {
            domain: &lab.domain,
            prepare_id: b"prepare-mismatch",
            command_id: "mismatch",
            raw_request_digest: Digest256::of_bytes(b"mismatch"),
            delta_digest: durable_shadow_delta_prepared(&[ShadowWriteIdentity {
                member_slot: 0,
                subject: "subject-A",
                expected_predecessor: Some((1, Digest256::of_bytes(b"predecessor"))),
                proposed_revision: 2,
                exact_bytes: &bytes,
            }]),
        })
        .unwrap();
    let receipts = seal(
        &lab.store,
        b"prepare-mismatch",
        attempt_fence,
        &[("subject-A", bytes.clone())],
    );
    let wrong_revision = DurableShadowMember {
        member_slot: 0,
        subject: "subject-A".to_owned(),
        expected_predecessor: Some((1, Digest256::of_bytes(b"predecessor"))),
        proposed_revision: 2,
        exact_bytes: bytes.clone(),
        receipt: receipts[0].clone(),
    };
    assert!(matches!(
        lab.db.attach_ready(
            &lab.store,
            &lab.domain,
            b"prepare-mismatch",
            attempt_fence,
            &[wrong_revision]
        ),
        Err(DurableError::Invalid(_))
    ));
    assert_eq!(lab.count("member"), 0);
    assert_eq!(lab.count("current"), 0);
    let wrong_subject = DurableShadowMember {
        member_slot: 0,
        subject: "subject-B".to_owned(),
        expected_predecessor: None,
        proposed_revision: 1,
        exact_bytes: bytes,
        receipt: receipts[0].clone(),
    };
    assert!(matches!(
        lab.db.attach_ready(
            &lab.store,
            &lab.domain,
            b"prepare-mismatch",
            attempt_fence,
            &[wrong_subject]
        ),
        Err(DurableError::Invalid(_))
    ));
}

#[test]
fn fenced_attempt_and_slot_mismatch_cannot_attach_or_commit() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let prepare_id = b"prepare-fence-negative";
    let members = lab.prepare(
        prepare_id,
        "fence-negative",
        &[MemberSpec::first("subject-F", b"fenced exact bytes")],
    );
    let fence = lab.attempt_fences[prepare_id.as_slice()];
    assert!(
        lab.store
            .recover_attempt_fenced(prepare_id, fence + 1, 0)
            .is_err()
    );
    assert!(
        lab.store
            .recover_attempt_fenced(prepare_id, fence, 1)
            .is_err()
    );
    let receipts = [members[0].receipt.clone()];
    let budget = VerificationBudget {
        max_receipts: 1,
        max_segments: 1,
        max_total_segment_bytes: 8 * 1024 * 1024,
    };
    assert!(
        lab.store
            .verify_and_hold_fenced(prepare_id, fence + 1, 0, &receipts, budget)
            .is_err()
    );
    assert!(
        lab.store
            .verify_and_hold_fenced(prepare_id, fence, 1, &receipts, budget)
            .is_err()
    );
    assert!(
        lab.db
            .attach_ready(&lab.store, &lab.domain, prepare_id, fence + 1, &members)
            .is_err()
    );
    let mut wrong_slot = members.clone();
    wrong_slot[0].member_slot = 1;
    assert!(matches!(
        lab.db
            .attach_ready(&lab.store, &lab.domain, prepare_id, fence, &wrong_slot),
        Err(DurableError::Invalid(_))
    ));
    assert!(
        lab.db
            .commit_shadow(
                &lab.store,
                &CommitShadowAttempt {
                    domain: &lab.domain,
                    prepare_id,
                    attempt_fence: fence + 1,
                    receipts: &receipts,
                    expected_contract_digest: contract_digest(),
                    expected_rule_version: 0,
                    expected_rights_version: 0,
                    job_id: "private-job",
                    job_fence: 1,
                    full_base_seq: 0,
                },
            )
            .is_err()
    );
    assert_eq!(lab.count("receipt"), 0);
    assert_eq!(
        lab.commit(prepare_id, &members, 0, 1).unwrap().commit_seq,
        1
    );
}

#[test]
fn cancel_wins_and_commit_wins_preserve_durable_attempt_decision() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let cancelled = lab.prepare(
        b"prepare-cancel",
        "cancel-wins",
        &[MemberSpec::first("subject-C", b"cancelled bytes")],
    );
    let pin = cancelled[0].receipt.pin_id();
    assert!(matches!(
        lab.db
            .resolve_attempt(&lab.domain, b"prepare-cancel")
            .unwrap(),
        AttemptResolution::Ready
    ));
    assert!(matches!(
        lab.db
            .cancel_attempt(&lab.store, &lab.domain, b"prepare-cancel")
            .unwrap(),
        CancelOutcome::Cancelled { pin_fenced: true }
    ));
    assert!(matches!(
        lab.db
            .resolve_attempt(&lab.domain, b"prepare-cancel")
            .unwrap(),
        AttemptResolution::Aborted
    ));
    let mut client = Client::connect(&url, NoTls).unwrap();
    let fence: i64 = client
        .query_one(
            "SELECT attempt_fence FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&lab.domain, &b"prepare-cancel".as_slice()],
        )
        .unwrap()
        .get(0);
    assert_eq!(fence, 2);
    assert!(lab.store.recover_sealed(pin).is_err());
    assert!(lab.commit(b"prepare-cancel", &cancelled, 0, 1).is_err());
    assert_eq!(lab.count("current"), 0);

    let winner = lab.prepare(
        b"prepare-commit",
        "commit-wins",
        &[MemberSpec::first("subject-W", b"committed bytes")],
    );
    let original = lab.commit(b"prepare-commit", &winner, 0, 1).unwrap();
    assert!(matches!(
        lab.db
            .cancel_attempt(&lab.store, &lab.domain, b"prepare-commit")
            .unwrap(),
        CancelOutcome::AlreadyCommitted(receipt) if receipt.commit_seq == original.commit_seq
    ));
    lab.store
        .verify_receipt(&winner[0].receipt)
        .expect("committed pin was never aborted");
    assert_eq!(lab.count("current"), 1);
    assert_eq!(lab.count("receipt"), 1);
}

#[test]
fn registered_cancel_survives_without_attached_pin() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    lab.db
        .register_attempt(&RegisterShadowAttempt {
            domain: &lab.domain,
            prepare_id: b"registered-only",
            command_id: "registered-only",
            raw_request_digest: Digest256::of_bytes(b"registered-only"),
            delta_digest: Digest256::of_bytes(b"no seal yet"),
        })
        .unwrap();
    assert!(matches!(
        lab.db
            .resolve_attempt(&lab.domain, b"registered-only")
            .unwrap(),
        AttemptResolution::Registered
    ));
    assert!(matches!(
        lab.db
            .cancel_attempt(&lab.store, &lab.domain, b"registered-only")
            .unwrap(),
        CancelOutcome::Cancelled { pin_fenced: false }
    ));
    assert!(matches!(
        lab.db
            .resolve_attempt(&lab.domain, b"registered-only")
            .unwrap(),
        AttemptResolution::Aborted
    ));
}

#[test]
fn cancel_refuses_a_different_domains_store_before_fencing_either_attempt() {
    let Some(url) = database_url() else { return };
    let mut a = Lab::new(&url);
    let mut b = Lab::new(&url);
    let prepare_id = b"same-prepare-different-domains";
    let members = a.prepare(
        prepare_id,
        "domain-a-commit",
        &[MemberSpec::first("subject-A", b"A's committed bytes")],
    );
    a.commit(prepare_id, &members, 0, 1).unwrap();
    b.db.register_attempt(&RegisterShadowAttempt {
        domain: &b.domain,
        prepare_id,
        command_id: "domain-b-registered",
        raw_request_digest: Digest256::of_bytes(b"domain-b-registered"),
        delta_digest: Digest256::of_bytes(b"domain-b-no-seal"),
    })
    .unwrap();

    assert!(matches!(
        b.db.cancel_attempt(&a.store, &b.domain, prepare_id),
        Err(DurableError::Conflict(_))
    ));
    assert!(matches!(
        b.db.resolve_attempt(&b.domain, prepare_id).unwrap(),
        AttemptResolution::Registered
    ));
    assert!(matches!(
        a.db.resolve_attempt(&a.domain, prepare_id).unwrap(),
        AttemptResolution::Committed(_)
    ));
    a.store.verify_receipt(&members[0].receipt).unwrap();
}

#[test]
fn cancel_during_fenced_seal_retries_after_late_pin_completion() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let prepare_id = b"cancel-during-seal";
    let bytes = lab_record_bytes("late-subject", 1, b"late sealed bytes");
    let attempt_fence = lab
        .db
        .register_attempt(&RegisterShadowAttempt {
            domain: &lab.domain,
            prepare_id,
            command_id: "cancel-during-seal",
            raw_request_digest: Digest256::of_bytes(b"cancel-during-seal"),
            delta_digest: durable_shadow_delta_prepared(&[ShadowWriteIdentity {
                member_slot: 0,
                subject: "late-subject",
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: &bytes,
            }]),
        })
        .unwrap();
    let (entered_send, entered_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let worker_store = lab.store.clone();
    let worker_bytes = bytes.clone();
    let sealing = thread::spawn(move || {
        let mut reader = PausingReader {
            cursor: Cursor::new(worker_bytes.clone()),
            entered: entered_send,
            release: release_recv,
            paused: false,
        };
        let mut frames = [FrameInput {
            binding: OwnerBinding {
                profile_id: PROFILE_ID.to_vec(),
                profile_version: PROFILE_VERSION.to_vec(),
                subject_key: b"late-subject".to_vec(),
                member_slot: 0,
            },
            declared_size: worker_bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&worker_bytes),
            reader: &mut reader,
        }];
        worker_store
            .seal_segment_fenced(prepare_id, attempt_fence, 0, &mut frames)
            .expect("late seal completes after DB abort")
    });
    entered_recv
        .recv_timeout(Duration::from_secs(5))
        .expect("seal reached input read after intent/pin sync");
    assert!(matches!(
        lab.store
            .recover_attempt_fenced(prepare_id, attempt_fence, 0)
            .unwrap(),
        Some(AttemptRecovery::Preparing { .. })
    ));
    assert!(matches!(
        lab.db
            .cancel_attempt(&lab.store, &lab.domain, prepare_id)
            .unwrap(),
        CancelOutcome::Cancelled { pin_fenced: false }
    ));
    assert!(matches!(
        lab.db.resolve_attempt(&lab.domain, prepare_id).unwrap(),
        AttemptResolution::Aborted
    ));
    release_send.send(()).unwrap();
    let receipts = sealing.join().unwrap();
    assert!(matches!(
        lab.store
            .recover_attempt_fenced(prepare_id, attempt_fence, 0)
            .unwrap(),
        Some(AttemptRecovery::Sealed { .. })
    ));
    let members = [DurableShadowMember {
        member_slot: 0,
        subject: "late-subject".to_owned(),
        expected_predecessor: None,
        proposed_revision: 1,
        exact_bytes: bytes,
        receipt: receipts[0].clone(),
    }];
    assert!(
        lab.db
            .attach_ready(&lab.store, &lab.domain, prepare_id, attempt_fence, &members)
            .is_err()
    );
    assert!(matches!(
        lab.db
            .cancel_attempt(&lab.store, &lab.domain, prepare_id)
            .unwrap(),
        CancelOutcome::Cancelled { pin_fenced: true }
    ));
    assert!(matches!(
        lab.store
            .recover_attempt_fenced(prepare_id, attempt_fence, 0)
            .unwrap(),
        Some(AttemptRecovery::Aborted { .. })
    ));
    assert_eq!(lab.count("member"), 0);
    assert_eq!(lab.count("receipt"), 0);
}

#[test]
fn rights_revocation_blocks_pending_commit_and_committed_replay() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let committed = lab.prepare(
        b"prepare-committed",
        "committed-before-revoke",
        &[MemberSpec::first("subject-R", b"rights-gated")],
    );
    let original = lab.commit(b"prepare-committed", &committed, 0, 1).unwrap();
    let pending = lab.prepare(
        b"prepare-pending",
        "pending-under-revoke",
        &[MemberSpec::first("subject-P", b"pending")],
    );
    assert_eq!(lab.db.revoke_local(&lab.domain).unwrap(), 2);
    assert!(matches!(
        lab.commit(b"prepare-pending", &pending, 1, 1),
        Err(DurableError::Refused(_))
    ));
    assert!(matches!(
        lab.commit(b"prepare-committed", &committed, 0, 1),
        Err(DurableError::Refused(_))
    ));
    assert!(matches!(
        lab.db.resolve_attempt(&lab.domain, b"prepare-committed").unwrap(),
        AttemptResolution::Committed(receipt) if receipt.commit_seq == original.commit_seq
    ));
    assert_eq!(lab.count("current"), 1);
    assert_eq!(lab.count("receipt"), 1);
    assert_eq!(lab.count("log"), 2);
    assert_eq!(lab.count("outbox"), 2);
}

#[test]
fn cold_reopen_retains_predecessor_bytes_and_detects_locator_tamper() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let original = lab.prepare(
        b"prepare-v1",
        "version-one",
        &[MemberSpec::first("subject-V", b"first private value")],
    );
    lab.commit(b"prepare-v1", &original, 0, 1).unwrap();
    let first_digest = Digest256::of_bytes(&original[0].exact_bytes);
    let revised = lab.prepare(
        b"prepare-v2",
        "version-two",
        &[MemberSpec {
            subject: "subject-V",
            revision: 2,
            predecessor: Some((1, first_digest)),
            payload: b"second private value",
        }],
    );
    lab.commit(b"prepare-v2", &revised, 1, 1).unwrap();
    assert_eq!(lab.count("current"), 1);
    assert_eq!(lab.count("history"), 2);

    let cold_store = SegmentStore::open_existing(&lab._root.0, limits())
        .expect("exact byte store reopens after fresh handle loss");
    let mut cold_db = DurablePgCoordinator::connect(&url).expect("fresh DB session connects");
    let historical = cold_db
        .cold_recover_exact(&cold_store, &lab.domain, "subject-V", 1)
        .expect("retained predecessor locator recovers");
    let current = cold_db
        .cold_recover_exact(&cold_store, &lab.domain, "subject-V", 2)
        .expect("current locator recovers");
    assert_eq!(historical.digest(), first_digest);
    assert_eq!(
        current.digest(),
        Digest256::of_bytes(&revised[0].exact_bytes)
    );
    assert_eq!(
        cold_db
            .warm_read_selected(&cold_store, &historical, 1024)
            .unwrap(),
        original[0].exact_bytes
    );
    assert_eq!(
        cold_db
            .warm_read_selected(&cold_store, &current, 1024)
            .unwrap(),
        revised[0].exact_bytes
    );

    let mut corrupter = Client::connect(&url, NoTls).unwrap();
    assert_eq!(
        corrupter
            .execute(
                "UPDATE cmd2_history SET frame_digest=$3
                 WHERE domain=$1 AND subject='subject-V' AND revision=$2",
                &[
                    &lab.domain,
                    &1i64,
                    &Digest256::of_bytes(b"forged frame").to_hex(),
                ],
            )
            .unwrap(),
        1
    );
    assert!(matches!(
        cold_db.cold_recover_exact(&cold_store, &lab.domain, "subject-V", 1),
        Err(DurableError::Corrupt(_))
    ));
    assert!(matches!(
        cold_db.warm_read_selected(&cold_store, &historical, 1024),
        Err(DurableError::Corrupt(_))
    ));
}

#[test]
fn selected_history_obeys_current_rights_after_cold_reopen() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-rights-read",
        "rights-read",
        &[MemberSpec::first("subject-R", b"retained but gated")],
    );
    lab.commit(b"prepare-rights-read", &members, 0, 1).unwrap();
    let cold_store = SegmentStore::open_existing(&lab._root.0, limits()).unwrap();
    let recovered = lab
        .db
        .cold_recover_exact(&cold_store, &lab.domain, "subject-R", 1)
        .unwrap();
    lab.db.revoke_local(&lab.domain).unwrap();
    assert!(matches!(
        lab.db.warm_read_selected(&cold_store, &recovered, 1024),
        Err(DurableError::Refused(_))
    ));
}

#[test]
fn independent_store_copy_requires_exact_v2_attempt_intents() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"copy-prepare-v1",
        "copy-version-one",
        &[MemberSpec::first("copy-subject", b"retained original")],
    );
    lab.commit(b"copy-prepare-v1", &first, 0, 1).unwrap();
    let predecessor = Digest256::of_bytes(&first[0].exact_bytes);
    let second = lab.prepare(
        b"copy-prepare-v2",
        "copy-version-two",
        &[MemberSpec {
            subject: "copy-subject",
            revision: 2,
            predecessor: Some((1, predecessor)),
            payload: b"selected successor",
        }],
    );
    lab.commit(b"copy-prepare-v2", &second, 1, 1).unwrap();

    let copied_root = ScratchRoot::new();
    copy_store_tree(&lab._root.0, &copied_root.0);
    assert_ne!(copied_root.0, lab._root.0);
    let copied_store = SegmentStore::open_existing(&copied_root.0, limits()).unwrap();
    let mut cold_db = DurablePgCoordinator::connect(&url).unwrap();
    for (prepare, revision, expected) in [
        (b"copy-prepare-v1".as_slice(), 1, &first[0].exact_bytes),
        (b"copy-prepare-v2".as_slice(), 2, &second[0].exact_bytes),
    ] {
        let fence = registered_fence(&url, &lab.domain, prepare);
        assert!(matches!(
            copied_store
                .recover_attempt_fenced(prepare, fence, 0)
                .unwrap(),
            Some(AttemptRecovery::Sealed { .. })
        ));
        let selected = cold_db
            .cold_recover_exact(&copied_store, &lab.domain, "copy-subject", revision)
            .unwrap();
        let selected_bytes = cold_db
            .warm_read_selected(&copied_store, &selected, 1024)
            .unwrap();
        assert_eq!(selected_bytes.as_slice(), expected.as_slice());
    }
    let cut = cold_db.cold_verify_cut(&copied_store, &lab.domain).unwrap();
    assert_eq!(cut.through_commit_seq(), 2);
    assert_eq!(cut.historical_members(), 2);

    let incomplete_root = ScratchRoot::new();
    copy_store_tree(&lab._root.0, &incomplete_root.0);
    let omitted = incomplete_root
        .0
        .join("attempts")
        .join(Digest256::of_bytes(b"copy-prepare-v1").to_hex());
    fs::remove_file(&omitted).expect("one v2 intent intentionally omitted from copy");
    let incomplete_store = SegmentStore::open_existing(&incomplete_root.0, limits()).unwrap();
    assert!(
        incomplete_store
            .recover_attempt_fenced(
                b"copy-prepare-v1",
                registered_fence(&url, &lab.domain, b"copy-prepare-v1"),
                0
            )
            .unwrap()
            .is_none()
    );
    assert!(
        cold_db
            .cold_recover_exact(&incomplete_store, &lab.domain, "copy-subject", 1)
            .is_err()
    );
    assert!(
        cold_db
            .cold_verify_cut(&incomplete_store, &lab.domain)
            .is_err()
    );
}

fn wait_for_pg_row_block(url: &str, worker_pid: i32, blocker_pid: i32) {
    let mut observer = Client::connect(url, NoTls).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let row = observer
            .query_one(
                "SELECT wait_event_type,pg_blocking_pids(pid)
                 FROM pg_stat_activity WHERE pid=$1",
                &[&worker_pid],
            )
            .unwrap();
        let wait_type: Option<String> = row.get(0);
        let blockers: Vec<i32> = row.get(1);
        if wait_type.as_deref() == Some("Lock") && blockers.contains(&blocker_pid) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker {worker_pid} never waited on audit fence; blockers={blockers:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn rights_change_after_observed_audit_fence_wait_refuses_commit() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-waiting",
        "waiting-under-rights",
        &[MemberSpec::first("subject-W", b"private waiting")],
    );
    let receipts: Vec<_> = members
        .iter()
        .map(|member| member.receipt.clone())
        .collect();
    let mut blocker = Client::connect(&url, NoTls).unwrap();
    let mut tx = blocker.transaction().unwrap();
    let blocker_pid: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    tx.query_one(
        "SELECT 1 FROM cmd2_audit_fence WHERE domain=$1 FOR UPDATE",
        &[&lab.domain],
    )
    .unwrap();
    tx.query_one(
        "SELECT 1 FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
        &[&lab.domain],
    )
    .unwrap();
    let (pid_send, pid_recv) = mpsc::channel();
    let (result_send, result_recv) = mpsc::channel();
    let worker_url = url.clone();
    let worker_domain = lab.domain.clone();
    let worker_store = lab.store.clone();
    let worker_fence = lab.attempt_fences[b"prepare-waiting".as_slice()];
    let worker = thread::spawn(move || {
        let mut db = DurablePgCoordinator::connect(&worker_url).unwrap();
        pid_send.send(db.backend_pid().unwrap()).unwrap();
        let result = db.commit_shadow(
            &worker_store,
            &CommitShadowAttempt {
                domain: &worker_domain,
                prepare_id: b"prepare-waiting",
                attempt_fence: worker_fence,
                receipts: &receipts,
                expected_contract_digest: contract_digest(),
                expected_rule_version: 0,
                expected_rights_version: 0,
                job_id: "private-job",
                job_fence: 1,
                full_base_seq: 0,
            },
        );
        result_send
            .send(matches!(result, Err(DurableError::Refused(_))))
            .unwrap();
    });
    let worker_pid = pid_recv.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_pg_row_block(&url, worker_pid, blocker_pid);
    tx.execute(
        "UPDATE cmd2_domain SET head_seq=1,rights_version=1,rights_allowed=false
         WHERE domain=$1",
        &[&lab.domain],
    )
    .unwrap();
    let empty = Digest256::of_bytes(b"").to_hex();
    tx.execute(
        "INSERT INTO cmd2_log(domain,commit_seq,event_kind,command_id,delta_digest,members_root)
         VALUES($1,1,'rights','rights.revoke:1',$2,$2)",
        &[&lab.domain, &empty],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO cmd2_outbox(domain,commit_seq,event_id)
         VALUES($1,1,'rights.revoke:1')",
        &[&lab.domain],
    )
    .unwrap();
    tx.commit().unwrap();
    assert!(result_recv.recv_timeout(Duration::from_secs(5)).unwrap());
    worker.join().unwrap();
    assert_eq!(lab.count("receipt"), 0);
    assert_eq!(lab.count("current"), 0);
    assert_eq!(lab.head_seq(), 1);
}

#[test]
fn verified_guard_blocks_adversarial_pin_abort_until_release() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-guard",
        "guarded",
        &[MemberSpec::first("subject-G", b"guarded bytes")],
    );
    let receipts = [members[0].receipt.clone()];
    let attempt_fence = lab.attempt_fences[b"prepare-guard".as_slice()];
    let guard = lab
        .store
        .verify_and_hold_fenced(
            b"prepare-guard",
            attempt_fence,
            0,
            &receipts,
            VerificationBudget {
                max_receipts: 1,
                max_segments: 1,
                max_total_segment_bytes: 8 * 1024 * 1024,
            },
        )
        .unwrap();
    let pin_id = receipts[0].pin_id();
    let pin_fence = receipts[0].fence_epoch();
    assert!(
        lab.store
            .abort_uncommitted_fenced(pin_id, b"prepare-guard", attempt_fence, 0, pin_fence)
            .is_err(),
        "exclusive STO abort passed a live same-process shared guard"
    );
    lab.store.verify_receipt(&members[0].receipt).unwrap();
    drop(guard);
    assert!(
        lab.store
            .abort_uncommitted(pin_id, b"prepare-guard", pin_fence)
            .is_err(),
        "v1 abort must not bypass a TOSINT2 attempt fence"
    );
    assert!(
        lab.store
            .abort_uncommitted_fenced(pin_id, b"prepare-guard", attempt_fence + 1, 0, pin_fence)
            .is_err()
    );
    assert!(
        lab.store
            .abort_uncommitted_fenced(pin_id, b"prepare-guard", attempt_fence, 1, pin_fence)
            .is_err()
    );
    lab.store.verify_receipt(&members[0].receipt).unwrap();
    lab.store
        .abort_uncommitted_fenced(pin_id, b"prepare-guard", attempt_fence, 0, pin_fence)
        .expect("abort fences pin after guard release");
    assert!(lab.store.verify_receipt(&members[0].receipt).is_err());
}

#[test]
fn cold_cut_seal_is_monotone_and_rejects_forged_digest() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"prepare-cut-a",
        "cut-A",
        &[MemberSpec::first("subject-A", b"cut alpha")],
    );
    lab.commit(b"prepare-cut-a", &first, 0, 1).unwrap();
    let cold_store = SegmentStore::open_existing(&lab._root.0, limits()).unwrap();
    let first_cut = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    assert_eq!(first_cut.through_commit_seq(), 1);
    assert_eq!(first_cut.historical_members(), 1);
    lab.db.seal_shadow_cut(&first_cut).unwrap();
    lab.db.seal_shadow_cut(&first_cut).unwrap();
    let audited_again = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    assert_eq!(audited_again.state_digest(), first_cut.state_digest());
    lab.db.seal_shadow_cut(&audited_again).unwrap();
    assert_eq!(lab.db.published_seq(&lab.domain).unwrap(), 1);
    let mut corrupter = Client::connect(&url, NoTls).unwrap();
    let old_delta: String = corrupter
        .query_one(
            "SELECT delta_digest FROM cmd2_log WHERE domain=$1 AND commit_seq=1",
            &[&lab.domain],
        )
        .unwrap()
        .get(0);
    corrupter
        .execute(
            "UPDATE cmd2_log SET delta_digest=$2 WHERE domain=$1 AND commit_seq=1",
            &[&lab.domain, &Digest256::of_bytes(b"forged cut").to_hex()],
        )
        .unwrap();
    assert!(matches!(
        lab.db.seal_shadow_cut(&first_cut),
        Err(DurableError::Conflict(_))
    ));
    assert_eq!(lab.db.published_seq(&lab.domain).unwrap(), 1);
    corrupter
        .execute(
            "UPDATE cmd2_log SET delta_digest=$2 WHERE domain=$1 AND commit_seq=1",
            &[&lab.domain, &old_delta],
        )
        .unwrap();

    let second = lab.prepare(
        b"prepare-cut-b",
        "cut-B",
        &[MemberSpec::first("subject-B", b"cut beta")],
    );
    lab.commit(b"prepare-cut-b", &second, 1, 1).unwrap();
    let second_cut = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    assert_eq!(second_cut.through_commit_seq(), 2);
    assert_eq!(second_cut.historical_members(), 2);
    lab.db.seal_shadow_cut(&second_cut).unwrap();
    assert!(matches!(
        lab.db.seal_shadow_cut(&first_cut),
        Err(DurableError::Conflict(_))
    ));
    assert_eq!(lab.db.published_seq(&lab.domain).unwrap(), 2);
    corrupter
        .execute(
            "UPDATE cmd2_log SET delta_digest=$2 WHERE domain=$1 AND commit_seq=1",
            &[
                &lab.domain,
                &Digest256::of_bytes(b"forged old prefix").to_hex(),
            ],
        )
        .unwrap();
    assert!(matches!(
        lab.db.seal_shadow_cut(&first_cut),
        Err(DurableError::Conflict(_))
    ));
    assert_eq!(lab.db.published_seq(&lab.domain).unwrap(), 2);
}

#[test]
fn cold_cut_fence_rejects_same_count_mutation_and_aba() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-cut-fence",
        "cut-fence",
        &[MemberSpec::first("subject-F", b"fenced bytes")],
    );
    lab.commit(b"prepare-cut-fence", &members, 0, 1).unwrap();
    let cold_store = SegmentStore::open_existing(&lab._root.0, limits()).unwrap();
    let mut admin = Client::connect(&url, NoTls).unwrap();
    let mutations = [
        "UPDATE cmd2_history SET durability_class='forged' WHERE domain=$1",
        "UPDATE cmd2_current SET durability_class='forged' WHERE domain=$1",
        "UPDATE cmd2_member SET durability_class='forged' WHERE domain=$1",
        "UPDATE cmd2_receipt SET raw_request_digest=repeat('a',64) WHERE domain=$1",
        "UPDATE cmd2_outbox SET event_id='forged-event' WHERE domain=$1",
    ];
    let restorations = [
        "UPDATE cmd2_history SET durability_class='fsync-reopened' WHERE domain=$1",
        "UPDATE cmd2_current SET durability_class='fsync-reopened' WHERE domain=$1",
        "UPDATE cmd2_member SET durability_class='fsync-reopened' WHERE domain=$1",
        "UPDATE cmd2_receipt SET raw_request_digest=(SELECT raw_request_digest FROM cmd2_attempt WHERE domain=$1) WHERE domain=$1",
        "UPDATE cmd2_outbox SET event_id=domain || ':1' WHERE domain=$1",
    ];
    // Read the exact original class rather than assuming a store enum spelling.
    let original_class: String = admin
        .query_one(
            "SELECT durability_class FROM cmd2_history WHERE domain=$1",
            &[&lab.domain],
        )
        .unwrap()
        .get(0);
    for (index, (mutation, restoration)) in mutations.iter().zip(restorations).enumerate() {
        let cut = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
        assert_eq!(
            admin.execute(*mutation, &[&lab.domain]).unwrap(),
            1,
            "mutation {index}"
        );
        if index < 3 {
            let table = ["cmd2_history", "cmd2_current", "cmd2_member"][index];
            let query = format!("UPDATE {table} SET durability_class=$2 WHERE domain=$1");
            assert_eq!(
                admin
                    .execute(&query, &[&lab.domain, &original_class])
                    .unwrap(),
                1
            );
        } else {
            assert_eq!(admin.execute(restoration, &[&lab.domain]).unwrap(), 1);
        }
        assert!(
            matches!(lab.db.seal_shadow_cut(&cut), Err(DurableError::Conflict(_))),
            "same-count or ABA metadata mutation {index} passed a stale cut"
        );
    }
    let profile_cut = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    let original_profile: String = admin
        .query_one(
            "SELECT schema_profile_digest FROM cmd2_domain WHERE domain=$1",
            &[&lab.domain],
        )
        .unwrap()
        .get(0);
    admin
        .execute(
            "UPDATE cmd2_domain SET schema_profile_digest=repeat('0',64) WHERE domain=$1",
            &[&lab.domain],
        )
        .unwrap();
    assert!(matches!(
        lab.db.cold_verify_cut(&cold_store, &lab.domain),
        Err(DurableError::Conflict(_))
    ));
    assert!(matches!(
        lab.db.seal_shadow_cut(&profile_cut),
        Err(DurableError::Conflict(_))
    ));
    admin
        .execute(
            "UPDATE cmd2_domain SET schema_profile_digest=$2 WHERE domain=$1",
            &[&lab.domain, &original_profile],
        )
        .unwrap();
    assert!(matches!(
        lab.db.seal_shadow_cut(&profile_cut),
        Err(DurableError::Conflict(_))
    ));
    let fresh = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    admin
        .execute(
            "UPDATE cmd2_audit_fence SET maintenance_state='active' WHERE domain=$1",
            &[&lab.domain],
        )
        .unwrap();
    assert!(matches!(
        lab.db.cold_verify_cut(&cold_store, &lab.domain),
        Err(DurableError::Refused(_))
    ));
    assert!(matches!(
        lab.db.seal_shadow_cut(&fresh),
        Err(DurableError::Refused(_))
    ));
    admin
        .execute(
            "UPDATE cmd2_audit_fence SET maintenance_state='normal' WHERE domain=$1",
            &[&lab.domain],
        )
        .unwrap();
    assert!(matches!(
        lab.db.seal_shadow_cut(&fresh),
        Err(DurableError::Conflict(_))
    ));
    let after_maintenance = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    lab.db.seal_shadow_cut(&after_maintenance).unwrap();
}

#[test]
fn seal_waits_on_trigger_ordered_fence_then_refuses_changed_cut() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-seal-order",
        "seal-order",
        &[MemberSpec::first("subject-O", b"ordering bytes")],
    );
    lab.commit(b"prepare-seal-order", &members, 0, 1).unwrap();
    let cold_store = SegmentStore::open_existing(&lab._root.0, limits()).unwrap();
    let cut = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    let mut blocker = Client::connect(&url, NoTls).unwrap();
    let mut tx = blocker.transaction().unwrap();
    let blocker_pid: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    tx.query_one(
        "SELECT 1 FROM cmd2_audit_fence WHERE domain=$1 FOR UPDATE",
        &[&lab.domain],
    )
    .unwrap();
    // This UPDATE fires the implicit audit trigger while the transaction
    // already owns its first lock. The publisher must wait on that lock,
    // never take the domain first and deadlock with the trigger.
    tx.execute(
        "UPDATE cmd2_member SET durability_class=durability_class WHERE domain=$1",
        &[&lab.domain],
    )
    .unwrap();
    let (pid_send, pid_recv) = mpsc::channel();
    let (result_send, result_recv) = mpsc::channel();
    let worker_url = url.clone();
    let worker = thread::spawn(move || {
        let mut db = DurablePgCoordinator::connect(&worker_url).unwrap();
        pid_send.send(db.backend_pid().unwrap()).unwrap();
        result_send
            .send(matches!(
                db.seal_shadow_cut(&cut),
                Err(DurableError::Conflict(_))
            ))
            .unwrap();
    });
    let worker_pid = pid_recv.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_pg_row_block(&url, worker_pid, blocker_pid);
    tx.commit().unwrap();
    assert!(result_recv.recv_timeout(Duration::from_secs(5)).unwrap());
    worker.join().unwrap();
    let fresh = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    lab.db.seal_shadow_cut(&fresh).unwrap();
}

#[test]
fn cold_cut_rejects_locator_and_outbox_tampering() {
    let Some(url) = database_url() else { return };
    let mut lab = Lab::new(&url);
    let member = lab.prepare(
        b"prepare-cut-tamper",
        "cut-tamper",
        &[MemberSpec::first("subject-T", b"cut tamper bytes")],
    );
    lab.commit(b"prepare-cut-tamper", &member, 0, 1).unwrap();
    let cold_store = SegmentStore::open_existing(&lab._root.0, limits()).unwrap();
    lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();

    let mut corrupter = Client::connect(&url, NoTls).unwrap();
    let original_pin: Vec<u8> = corrupter
        .query_one(
            "SELECT pin_id FROM cmd2_current WHERE domain=$1 AND subject='subject-T'",
            &[&lab.domain],
        )
        .unwrap()
        .get(0);
    corrupter
        .execute(
            "UPDATE cmd2_current SET pin_id=$2 WHERE domain=$1 AND subject='subject-T'",
            &[&lab.domain, &vec![7u8; 16]],
        )
        .unwrap();
    assert!(matches!(
        lab.db.cold_verify_cut(&cold_store, &lab.domain),
        Err(DurableError::Corrupt(_))
    ));
    corrupter
        .execute(
            "UPDATE cmd2_current SET pin_id=$2 WHERE domain=$1 AND subject='subject-T'",
            &[&lab.domain, &original_pin],
        )
        .unwrap();

    corrupter
        .execute(
            "UPDATE cmd2_outbox SET event_id='forged-event' WHERE domain=$1 AND commit_seq=1",
            &[&lab.domain],
        )
        .unwrap();
    assert!(matches!(
        lab.db.cold_verify_cut(&cold_store, &lab.domain),
        Err(DurableError::Corrupt(_))
    ));
}

/// Run this exact ignored test alone, then copy its reported sealed store and
/// pg_dump the quiescent synthetic PostgreSQL database. The intentional leak
/// keeps this one fixture on disk after the test process exits for restore.
#[test]
#[ignore = "manual owner-managed PostgreSQL dump and independent STO copy drill"]
fn export_cold_restore_fixture() {
    let url = database_url().expect("dedicated synthetic PostgreSQL URL required");
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"restore-prepare-a1",
        "restore-command-a1",
        &[MemberSpec::first("restore-A", b"retained predecessor")],
    );
    lab.commit(b"restore-prepare-a1", &first, 0, 1).unwrap();
    let predecessor = Digest256::of_bytes(&first[0].exact_bytes);
    let second = lab.prepare(
        b"restore-prepare-compound",
        "restore-command-compound",
        &[
            MemberSpec {
                subject: "restore-A",
                revision: 2,
                predecessor: Some((1, predecessor)),
                payload: b"new selected version",
            },
            MemberSpec::first("restore-B", b"second compound member"),
        ],
    );
    lab.commit(b"restore-prepare-compound", &second, 1, 1)
        .unwrap();
    // Sealed-before-attach is discoverable only through the synced STO
    // prepare intent. A backup must copy attempts/ with pins/ and segments/.
    let orphan_bytes = lab_record_bytes("restore-unattached", 1, b"orphan forensic bytes");
    let orphan_fence = lab
        .db
        .register_attempt(&RegisterShadowAttempt {
            domain: &lab.domain,
            prepare_id: b"restore-prepare-unattached",
            command_id: "restore-command-unattached",
            raw_request_digest: Digest256::of_bytes(b"restore-command-unattached"),
            delta_digest: durable_shadow_delta_prepared(&[ShadowWriteIdentity {
                member_slot: 0,
                subject: "restore-unattached",
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: &orphan_bytes,
            }]),
        })
        .unwrap();
    seal(
        &lab.store,
        b"restore-prepare-unattached",
        orphan_fence,
        &[("restore-unattached", orphan_bytes)],
    );
    assert!(matches!(
        lab.store
            .recover_attempt_fenced(b"restore-prepare-unattached", orphan_fence, 0)
            .unwrap(),
        Some(AttemptRecovery::Sealed { .. })
    ));
    let original_cut = lab.db.cold_verify_cut(&lab.store, &lab.domain).unwrap();
    assert_eq!(original_cut.through_commit_seq(), 2);
    assert_eq!(original_cut.historical_members(), 3);
    println!(
        "CMD2_RESTORE_FIXTURE domain={} store={} cut={} historical_members={}",
        lab.domain,
        lab._root.0.display(),
        original_cut.log_digest().to_hex(),
        original_cut.historical_members()
    );
    std::mem::forget(lab);
}

/// Point this at a new database restored from the export's pg_dump and at an
/// independent copy of the exported STO root, not its original directory.
#[test]
#[ignore = "manual owner-managed PostgreSQL dump and independent STO copy drill"]
fn verify_cold_restored_fixture() {
    let url = database_url().expect("restored synthetic PostgreSQL URL required");
    let domain = std::env::var("TOS_CMD2_RESTORE_DOMAIN").expect("exported domain required");
    let store_path =
        std::env::var_os("TOS_CMD2_RESTORE_STORE").expect("independent STO copy required");
    let store = SegmentStore::open_existing(&PathBuf::from(store_path), limits())
        .expect("copied STO store cold-opens");
    let mut db = DurablePgCoordinator::connect(&url).unwrap();
    for prepare in [
        b"restore-prepare-a1".as_slice(),
        b"restore-prepare-compound".as_slice(),
        b"restore-prepare-unattached".as_slice(),
    ] {
        let fence = registered_fence(&url, &domain, prepare);
        assert!(
            matches!(
                store.recover_attempt_fenced(prepare, fence, 0).unwrap(),
                Some(AttemptRecovery::Sealed { .. })
            ),
            "restored STO root must retain exact durable prepare intent"
        );
    }
    assert!(matches!(
        db.resolve_attempt(&domain, b"restore-prepare-unattached")
            .unwrap(),
        AttemptResolution::Registered
    ));
    assert!(matches!(
        db.cancel_attempt(&store, &domain, b"restore-prepare-unattached")
            .unwrap(),
        CancelOutcome::Cancelled { pin_fenced: true }
    ));
    assert!(matches!(
        store
            .recover_attempt_fenced(
                b"restore-prepare-unattached",
                registered_fence(&url, &domain, b"restore-prepare-unattached") - 1,
                0
            )
            .unwrap(),
        Some(AttemptRecovery::Aborted { .. })
    ));
    let expected = [
        ("restore-A", 1, b"retained predecessor".as_slice()),
        ("restore-A", 2, b"new selected version".as_slice()),
        ("restore-B", 1, b"second compound member".as_slice()),
    ];
    for (subject, revision, payload) in expected {
        let selected = db
            .cold_recover_exact(&store, &domain, subject, revision)
            .expect("restored metadata selects exact sealed bytes");
        let bytes = db.warm_read_selected(&store, &selected, 1024).unwrap();
        assert_eq!(bytes, lab_record_bytes(subject, revision, payload));
    }
    let cut = db.cold_verify_cut(&store, &domain).unwrap();
    assert_eq!(cut.through_commit_seq(), 2);
    assert_eq!(cut.historical_members(), 3);
    db.seal_shadow_cut(&cut).unwrap();
    assert_eq!(db.published_seq(&domain).unwrap(), 2);
    let predecessor = db
        .cold_recover_exact(&store, &domain, "restore-A", 1)
        .unwrap();
    db.revoke_local(&domain).unwrap();
    assert!(matches!(
        db.warm_read_selected(&store, &predecessor, 1024),
        Err(DurableError::Refused(_))
    ));
    println!(
        "CMD2_RESTORED domain={} cut={} historical_members={}",
        domain,
        cut.log_digest().to_hex(),
        cut.historical_members()
    );
}

#[test]
fn sigkill_after_seal_ready_and_commit_has_distinct_recovery() {
    let Some(url) = database_url() else { return };
    for phase in ["sealed", "ready", "committed"] {
        let mut lab = Lab::new(&url);
        let status = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("cmd2_process_kill_child")
            .arg("--ignored")
            .env("TOS_CMD2_KILL_PHASE", phase)
            .env("TOS_CMD2_KILL_DOMAIN", &lab.domain)
            .env("TOS_CMD2_KILL_ROOT", &lab._root.0)
            .status()
            .expect("child test starts");
        assert_eq!(status.signal(), Some(9), "{phase} did not receive SIGKILL");
        let cold_store = SegmentStore::open_existing(&lab._root.0, limits())
            .expect("exact STO instance reopens after killed process");
        let attempt_fence = registered_fence(&url, &lab.domain, b"child-prepare");
        let pin_id = match cold_store
            .recover_attempt_fenced(b"child-prepare", attempt_fence, 0)
            .unwrap()
            .unwrap()
        {
            AttemptRecovery::Sealed { receipts } => receipts[0].pin_id(),
            other => {
                panic!("{phase}: durable prepare intent did not recover sealed pin: {other:?}")
            }
        };
        let recovered = cold_store
            .recover_sealed(pin_id)
            .expect("sealed pin remains recoverable after process kill");
        assert_eq!(recovered.len(), 1);
        match phase {
            "sealed" => {
                assert!(matches!(
                    lab.db
                        .resolve_attempt(&lab.domain, b"child-prepare")
                        .unwrap(),
                    AttemptResolution::Registered
                ));
                assert_eq!(lab.count("member"), 0);
                assert_eq!(lab.count("receipt"), 0);
                assert!(matches!(
                    lab.db
                        .cancel_attempt(&cold_store, &lab.domain, b"child-prepare")
                        .unwrap(),
                    CancelOutcome::Cancelled { pin_fenced: true }
                ));
                assert!(
                    cold_store.recover_sealed(pin_id).is_err(),
                    "unattached sealed pin must be fenced after DB abort decision"
                );
                assert!(matches!(
                    cold_store
                        .recover_attempt_fenced(b"child-prepare", attempt_fence, 0)
                        .unwrap(),
                    Some(AttemptRecovery::Aborted { .. })
                ));
            }
            "ready" => {
                assert!(matches!(
                    lab.db
                        .resolve_attempt(&lab.domain, b"child-prepare")
                        .unwrap(),
                    AttemptResolution::Ready
                ));
                assert_eq!(lab.count("receipt"), 0);
                assert!(matches!(
                    lab.db
                        .cancel_attempt(&cold_store, &lab.domain, b"child-prepare")
                        .unwrap(),
                    CancelOutcome::Cancelled { pin_fenced: true }
                ));
                assert!(cold_store.recover_sealed(pin_id).is_err());
            }
            "committed" => {
                assert!(matches!(
                    lab.db.resolve_attempt(&lab.domain, b"child-prepare").unwrap(),
                    AttemptResolution::Committed(receipt) if receipt.commit_seq == 1
                ));
                let selected = lab
                    .db
                    .cold_recover_exact(&cold_store, &lab.domain, "child-subject", 1)
                    .unwrap();
                assert_eq!(
                    lab.db
                        .warm_read_selected(&cold_store, &selected, 1024)
                        .unwrap(),
                    lab_record_bytes("child-subject", 1, b"child private bytes")
                );
                assert_eq!(lab.count("receipt"), 1);
                assert_eq!(lab.count("outbox"), 1);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
#[ignore = "run only as a child of sigkill_after_seal_ready_and_commit_has_distinct_recovery"]
fn cmd2_process_kill_child() {
    let phase = std::env::var("TOS_CMD2_KILL_PHASE").expect("parent supplies kill phase");
    let url = database_url().expect("parent supplies ephemeral PostgreSQL URL");
    let domain = std::env::var("TOS_CMD2_KILL_DOMAIN").expect("parent supplies domain");
    let root = PathBuf::from(std::env::var_os("TOS_CMD2_KILL_ROOT").expect("parent supplies root"));
    let store = SegmentStore::open_existing(&root, limits()).unwrap();
    let mut db = DurablePgCoordinator::connect(&url).unwrap();
    let bytes = lab_record_bytes("child-subject", 1, b"child private bytes");
    let attempt_fence = db
        .register_attempt(&RegisterShadowAttempt {
            domain: &domain,
            prepare_id: b"child-prepare",
            command_id: "child-command",
            raw_request_digest: Digest256::of_bytes(b"child-command"),
            delta_digest: durable_shadow_delta_prepared(&[ShadowWriteIdentity {
                member_slot: 0,
                subject: "child-subject",
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: &bytes,
            }]),
        })
        .unwrap();
    let receipts = seal(
        &store,
        b"child-prepare",
        attempt_fence,
        &[("child-subject", bytes.clone())],
    );
    if phase != "sealed" {
        let members = [DurableShadowMember {
            member_slot: 0,
            subject: "child-subject".to_owned(),
            expected_predecessor: None,
            proposed_revision: 1,
            exact_bytes: bytes,
            receipt: receipts[0].clone(),
        }];
        db.attach_ready(&store, &domain, b"child-prepare", attempt_fence, &members)
            .unwrap();
        if phase == "committed" {
            db.commit_shadow(
                &store,
                &CommitShadowAttempt {
                    domain: &domain,
                    prepare_id: b"child-prepare",
                    attempt_fence,
                    receipts: &receipts,
                    expected_contract_digest: contract_digest(),
                    expected_rule_version: 0,
                    expected_rights_version: 0,
                    job_id: "private-job",
                    job_fence: 1,
                    full_base_seq: 0,
                },
            )
            .unwrap();
        }
    }
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    // This child has completed the named barrier. No cleanup/destructor runs.
    assert_eq!(unsafe { kill(std::process::id() as i32, 9) }, 0);
    unreachable!("SIGKILL must terminate the child");
}
