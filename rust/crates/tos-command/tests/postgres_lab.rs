//! Run against a dedicated ephemeral database with TOS_CMD_POSTGRES_URL set.
//! No test opens or modifies the authored ToS corpus.

use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postgres::{Client, IsolationLevel, NoTls};
use tos_command::{
    AuthorityFence, Candidate, CommitValidationAttestation, Error, PgCoordinator, PredicateKind,
    PredicateRead, PredicateToken, RecordWrite, SyntheticByteRef, synthetic_delta_digest,
};
use tos_foundation::Digest256;

static NEXT_DOMAIN: AtomicU64 = AtomicU64::new(0);
static INIT_LAB_SCHEMA: Once = Once::new();

fn database_url() -> Option<String> {
    std::env::var("TOS_CMD_POSTGRES_URL").ok()
}

fn setup(url: &str) -> (PgCoordinator, String) {
    let mut db = PgCoordinator::connect(url).expect("lab PostgreSQL connects");
    INIT_LAB_SCHEMA.call_once(|| db.init_lab_schema().expect("lab schema initializes"));
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let domain = format!(
        "cmd1-{}-{time}-{}",
        std::process::id(),
        NEXT_DOMAIN.fetch_add(1, Ordering::Relaxed)
    );
    db.create_domain(&domain, Digest256::of_bytes(b"synthetic-contract-v1"))
        .expect("domain created");
    db.set_job_epoch(&domain, "job", 1)
        .expect("job initialized");
    (db, domain)
}

fn candidate(domain: &str, command_id: &str, key: &str) -> Candidate {
    let writes = vec![write(key, vec![])];
    let delta_digest = synthetic_delta_digest(&writes);
    Candidate {
        domain: domain.to_owned(),
        command_id: command_id.to_owned(),
        raw_request_digest: Digest256::of_bytes(command_id.as_bytes()),
        input_profile_id: "raw-lab-v1".to_owned(),
        delta_digest,
        reads: vec![PredicateRead::Absent {
            namespace: "synthetic".to_owned(),
            key: key.to_owned(),
        }],
        writes,
        authority: AuthorityFence::Local {
            expected_version: 0,
        },
        job_id: "job".to_owned(),
        fence_epoch: 1,
        expected_rule_version: 0,
        expected_contract_digest: Digest256::of_bytes(b"synthetic-contract-v1"),
        full_base_seq: None,
        attestation: CommitValidationAttestation {
            prepare_base_revision: Digest256::of_bytes(b"synthetic base"),
            prepare_overlay_id: format!("overlay-{command_id}"),
            prepare_delta_digest: delta_digest,
            trace_digest: Digest256::of_bytes(b"synthetic trace"),
            checked_predicates_digest: Digest256::of_bytes(b"synthetic predicates"),
            checked_rule_versions_digest: Digest256::of_bytes(b"synthetic versions"),
            owner_fences_digest: Digest256::of_bytes(b"synthetic local fence"),
            schema_profile_id: "synthetic-schema-v1".to_owned(),
            schema_backend_digest: Digest256::of_bytes(b"synthetic backend"),
        },
    }
}

fn rebind(candidate: &mut Candidate) {
    candidate.delta_digest = synthetic_delta_digest(&candidate.writes);
    candidate.attestation.prepare_delta_digest = candidate.delta_digest;
}

fn write(key: &str, invalidates: Vec<PredicateToken>) -> RecordWrite {
    RecordWrite {
        namespace: "synthetic".to_owned(),
        key: key.to_owned(),
        expected_version: None,
        bytes: SyntheticByteRef {
            digest: Digest256::of_bytes(key.as_bytes()),
            length: key.len() as u64,
            lab_marker: "synthetic-not-durable".to_owned(),
        },
        invalidates,
    }
}

fn token(kind: PredicateKind) -> PredicateToken {
    PredicateToken {
        kind,
        owner: "lab-rule".to_owned(),
        scope: "synthetic".to_owned(),
        token: "selected-scope".to_owned(),
        definition_version: "v1".to_owned(),
    }
}

fn must_conflict(result: tos_command::Result<impl Sized>) {
    assert!(
        matches!(result, Err(Error::Conflict(_))),
        "expected conflict"
    );
}

fn wait_for_sequencer_block(url: &str, worker_pid: i32, blocker_pid: i32) {
    let mut observer = Client::connect(url, NoTls).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let row = observer
            .query_one(
                "SELECT wait_event_type, pg_blocking_pids(pid)
                 FROM pg_stat_activity WHERE pid=$1",
                &[&worker_pid],
            )
            .unwrap();
        let wait_type: Option<String> = row.get(0);
        let blocking_pids: Vec<i32> = row.get(1);
        if wait_type.as_deref() == Some("Lock") && blocking_pids.contains(&blocker_pid) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker {worker_pid} never waited on sequencer owner {blocker_pid}; last wait={wait_type:?}, blockers={blocking_pids:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn disjoint_commit_order_closed_cut_and_observed_lock_wait() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    let a = candidate(&domain, "prepared-A", "A");
    let b = candidate(&domain, "prepared-B", "B");
    let (b_receipt, _) = db.commit(&b).unwrap();
    let (a_receipt, _) = db.commit(&a).unwrap();
    assert_eq!((b_receipt.commit_seq, a_receipt.commit_seq), (1, 2));
    let cut = db.read_cut(&domain).unwrap();
    assert_eq!(cut.command_ids, ["prepared-B", "prepared-A"]);
    db.seal_cut(&cut).unwrap();
    assert_eq!(db.published_seq(&domain).unwrap(), 2);
    db.seal_cut(&cut).unwrap();
    let mut forged = cut.clone();
    forged.log_digest = Digest256::of_bytes(b"forged publication digest");
    must_conflict(db.seal_cut(&forged));
    assert_eq!(db.published_seq(&domain).unwrap(), 2);

    let mut blocker = Client::connect(&url, NoTls).unwrap();
    let mut tx = blocker.transaction().unwrap();
    let blocker_pid: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    tx.query_one(
        "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1 FOR UPDATE",
        &[&domain],
    )
    .unwrap();
    let (send, recv) = mpsc::channel();
    let (pid_send, pid_recv) = mpsc::channel();
    let worker_url = url.clone();
    let worker_domain = domain.clone();
    let handle = thread::spawn(move || {
        let mut worker = PgCoordinator::connect(&worker_url).unwrap();
        pid_send.send(worker.backend_pid().unwrap()).unwrap();
        let value = worker.commit(&candidate(&worker_domain, "waiter", "C"));
        send.send(value).unwrap();
    });
    let worker_pid = pid_recv.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_sequencer_block(&url, worker_pid, blocker_pid);
    tx.commit().unwrap();
    let (receipt, timing) = recv.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    handle.join().unwrap();
    assert_eq!(receipt.commit_seq, 3);
    assert!(timing.lock_wait > Duration::ZERO);
}

#[test]
fn commit_reads_current_rights_after_waiting_for_sequencer() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    let prepared = candidate(&domain, "waiting-under-revoke", "X");
    let mut owner = Client::connect(&url, NoTls).unwrap();
    let mut owner_tx = owner.transaction().unwrap();
    let owner_pid: i32 = owner_tx
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    owner_tx
        .query_one(
            "SELECT 1 FROM cmd1_coordinator WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )
        .unwrap();
    let (send, recv) = mpsc::channel();
    let (pid_send, pid_recv) = mpsc::channel();
    let worker_url = url.clone();
    let worker = thread::spawn(move || {
        let mut client = PgCoordinator::connect(&worker_url).unwrap();
        pid_send.send(client.backend_pid().unwrap()).unwrap();
        send.send(client.commit(&prepared)).unwrap();
    });
    let worker_pid = pid_recv.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_sequencer_block(&url, worker_pid, owner_pid);
    owner_tx
        .execute(
            "UPDATE cmd1_coordinator SET head_seq=1,rights_version=1,rights_allowed=false
             WHERE domain=$1",
            &[&domain],
        )
        .unwrap();
    let empty: Vec<String> = vec![];
    owner_tx
        .execute(
            "INSERT INTO cmd1_commit_log(domain,commit_seq,event_kind,command_id,delta_digest,members)
             VALUES($1,1,'rights','rights.revoke:1',$2,$3)",
            &[&domain, &Digest256::of_bytes(b"").to_hex(), &empty],
        )
        .unwrap();
    owner_tx
        .execute(
            "INSERT INTO cmd1_outbox(domain,commit_seq,event_id)
             VALUES($1,1,'rights.revoke:1')",
            &[&domain],
        )
        .unwrap();
    owner_tx.commit().unwrap();
    assert!(matches!(
        recv.recv_timeout(Duration::from_secs(5)).unwrap(),
        Err(Error::Refused(_))
    ));
    worker.join().unwrap();
    assert_eq!(db.head_seq(&domain).unwrap(), 1);
    assert_eq!(db.receipt_count(&domain).unwrap(), 0);
}

#[test]
fn absent_and_typed_predicate_phantoms() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    let stale = candidate(&domain, "stale-absence", "X");
    db.commit(&candidate(&domain, "creates-X", "X")).unwrap();
    must_conflict(db.commit(&stale));
    for (index, kind) in [
        PredicateKind::Unique,
        PredicateKind::Range,
        PredicateKind::Prefix,
        PredicateKind::ReverseRefs,
        PredicateKind::Interval,
    ]
    .into_iter()
    .enumerate()
    {
        let token = token(kind);
        db.register_predicate(&domain, &token, true).unwrap();
        let observed = db.predicate_generation(&domain, &token).unwrap();
        let mut reader = candidate(&domain, &format!("reader-{index}"), &format!("R{index}"));
        reader.reads.push(PredicateRead::Generation {
            predicate: token.clone(),
            observed_generation: observed,
        });
        let mut writer = candidate(&domain, &format!("writer-{index}"), &format!("W{index}"));
        writer.writes[0].invalidates.push(token);
        rebind(&mut writer);
        db.commit(&writer).unwrap();
        must_conflict(db.commit(&reader));
    }
}

#[test]
fn revocation_rule_drift_external_refusal_and_current_read_gate() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    let prepared = candidate(&domain, "prepared", "X");
    let revoked_seq = db.revoke_local(&domain).unwrap();
    assert_eq!(revoked_seq, 1);
    assert!(matches!(db.commit(&prepared), Err(Error::Refused(_))));
    assert!(matches!(
        db.read_record(&domain, "synthetic", "X"),
        Err(Error::Refused(_))
    ));
    assert_eq!(
        db.read_cut(&domain).unwrap().command_ids,
        ["rights.revoke:1"]
    );

    let (mut db2, domain2) = setup(&url);
    let before = candidate(&domain2, "before-revoke", "Y");
    db2.commit(&before).unwrap();
    assert!(
        db2.read_record(&domain2, "synthetic", "Y")
            .unwrap()
            .is_some()
    );
    db2.revoke_local(&domain2).unwrap();
    assert!(matches!(
        db2.read_record(&domain2, "synthetic", "Y"),
        Err(Error::Refused(_))
    ));

    let (mut db3, domain3) = setup(&url);
    let rule_stale = candidate(&domain3, "rule-stale", "Z");
    db3.set_rule_version(&domain3, 1).unwrap();
    must_conflict(db3.commit(&rule_stale));
    let mut external = candidate(&domain3, "external", "E");
    external.authority = AuthorityFence::ExternalUnsupported {
        owner: "outside".to_owned(),
        scope: "E".to_owned(),
    };
    assert!(matches!(
        db3.commit(&external),
        Err(Error::UnsupportedExternalAuthority)
    ));

    let (mut db4, domain4) = setup(&url);
    let schema_stale = candidate(&domain4, "schema-stale", "S");
    db4.set_contract_digest(
        &domain4,
        Digest256::of_bytes(b"changed schema/registry/backend"),
    )
    .unwrap();
    must_conflict(db4.commit(&schema_stale));
}

#[test]
fn retained_versions_obey_current_rights() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    db.commit(&candidate(&domain, "create-A", "A")).unwrap();
    let original = Digest256::of_bytes(b"A");
    let mut revision = candidate(&domain, "revise-A", "A");
    revision.reads = vec![PredicateRead::Exact {
        namespace: "synthetic".to_owned(),
        key: "A".to_owned(),
        expected_version: Some(1),
        expected_digest: Some(original),
    }];
    revision.writes[0].expected_version = Some(1);
    revision.writes[0].bytes.digest = Digest256::of_bytes(b"A-v2");
    revision.writes[0].bytes.length = 4;
    rebind(&mut revision);
    db.commit(&revision).unwrap();
    assert_eq!(db.history_count(&domain).unwrap(), 2);
    assert_eq!(
        db.read_record_version(&domain, "synthetic", "A", 1)
            .unwrap(),
        Some(original)
    );
    assert_eq!(
        db.read_record_version(&domain, "synthetic", "A", 2)
            .unwrap(),
        Some(Digest256::of_bytes(b"A-v2"))
    );
    db.revoke_local(&domain).unwrap();
    assert!(matches!(
        db.read_record_version(&domain, "synthetic", "A", 1),
        Err(Error::Refused(_))
    ));
}

#[test]
fn closed_cut_detects_missing_committed_history() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    db.commit(&candidate(&domain, "create-A", "A")).unwrap();
    assert_eq!(db.read_cut(&domain).unwrap().through_commit_seq, 1);
    let mut corrupter = Client::connect(&url, NoTls).unwrap();
    corrupter
        .execute(
            "DELETE FROM cmd1_record_history WHERE domain=$1 AND commit_seq=1",
            &[&domain],
        )
        .unwrap();
    assert!(matches!(db.read_cut(&domain), Err(Error::Corrupt(_))));
}

#[test]
fn closed_cut_rejects_same_count_history_member_tamper() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    db.commit(&candidate(&domain, "create-A", "A")).unwrap();
    let cut = db.read_cut(&domain).unwrap();
    let mut corrupter = Client::connect(&url, NoTls).unwrap();
    assert_eq!(
        corrupter
            .execute(
                "UPDATE cmd1_record_history SET digest=$2
                 WHERE domain=$1 AND commit_seq=1",
                &[&domain, &Digest256::of_bytes(b"different bytes").to_hex()],
            )
            .unwrap(),
        1
    );
    assert_eq!(db.history_count(&domain).unwrap(), 1);
    assert!(matches!(db.read_cut(&domain), Err(Error::Corrupt(_))));
    assert!(matches!(db.seal_cut(&cut), Err(Error::Corrupt(_))));
    assert_eq!(db.published_seq(&domain).unwrap(), 0);
}

#[test]
fn seal_rejects_malformed_cut_event_counts_before_publication() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    db.commit(&candidate(&domain, "first", "A")).unwrap();
    db.commit(&candidate(&domain, "second", "B")).unwrap();
    let cut = db.read_cut(&domain).unwrap();
    assert_eq!(cut.through_commit_seq, 2);

    let mut short = cut.clone();
    short.command_ids.pop();
    assert!(matches!(db.seal_cut(&short), Err(Error::InvalidInput(_))));
    let mut long = cut.clone();
    long.command_ids.push("fabricated".to_owned());
    assert!(matches!(db.seal_cut(&long), Err(Error::InvalidInput(_))));
    let mut overflow = cut.clone();
    overflow.through_commit_seq = u64::MAX;
    assert!(matches!(
        db.seal_cut(&overflow),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(db.published_seq(&domain).unwrap(), 0);

    db.seal_cut(&cut).unwrap();
    assert_eq!(db.published_seq(&domain).unwrap(), 2);
}

#[test]
fn replay_lease_compound_rollback_and_mvcc_prefix() {
    let Some(url) = database_url() else { return };
    let (mut db, domain) = setup(&url);
    let a = candidate(&domain, "A", "A");
    let first = db.commit(&a).unwrap().0;
    let replay = db.commit(&a).unwrap().0;
    assert_eq!(first.commit_seq, replay.commit_seq);
    assert!(replay.replayed);
    assert_eq!(
        (
            db.receipt_count(&domain).unwrap(),
            db.outbox_count(&domain).unwrap()
        ),
        (1, 1)
    );
    let old_cut = db.read_cut(&domain).unwrap();
    let mut collision = a.clone();
    collision.raw_request_digest = Digest256::of_bytes(b"different");
    must_conflict(db.commit(&collision));

    let stale = candidate(&domain, "stale-job", "B");
    db.set_job_epoch(&domain, "job", 2).unwrap();
    assert!(matches!(db.commit(&stale), Err(Error::Refused(_))));
    let mut compound = candidate(&domain, "compound", "C");
    compound.fence_epoch = 2;
    compound.writes.push(write("C", vec![])); // second row collides after first insert
    rebind(&mut compound);
    must_conflict(db.commit(&compound));
    assert_eq!(
        (
            db.head_seq(&domain).unwrap(),
            db.record_count(&domain).unwrap()
        ),
        (1, 1)
    );
    assert_eq!(db.history_count(&domain).unwrap(), 1);
    assert_eq!(
        (
            db.receipt_count(&domain).unwrap(),
            db.outbox_count(&domain).unwrap()
        ),
        (1, 1)
    );

    // A real REPEATABLE READ snapshot observes head=1 before B commits and
    // retains that closed prefix after the concurrent committed successor.
    let mut snapshot_client = Client::connect(&url, NoTls).unwrap();
    let mut snapshot = snapshot_client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .unwrap();
    let head: i64 = snapshot
        .query_one(
            "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1",
            &[&domain],
        )
        .unwrap()
        .get(0);
    assert_eq!(head, 1);
    let mut b = candidate(&domain, "B", "B");
    b.fence_epoch = 2;
    db.commit(&b).unwrap();
    let visible: i64 = snapshot
        .query_one(
            "SELECT count(*) FROM cmd1_commit_log WHERE domain=$1",
            &[&domain],
        )
        .unwrap()
        .get(0);
    assert_eq!(visible, 1);
    snapshot.commit().unwrap();
    db.seal_cut(&old_cut).unwrap();
    assert_eq!(db.published_seq(&domain).unwrap(), 1);
    let new_cut = db.read_cut(&domain).unwrap();
    assert_eq!(new_cut.through_commit_seq, 2);
    db.seal_cut(&new_cut).unwrap();
    assert_eq!(db.published_seq(&domain).unwrap(), 2);
    let mut forged_old_cut = old_cut.clone();
    forged_old_cut.log_digest = Digest256::of_bytes(b"forged old prefix");
    must_conflict(db.seal_cut(&forged_old_cut));
    db.seal_cut(&old_cut).unwrap();

    let mut compound_ok = candidate(&domain, "compound-ok", "C");
    compound_ok.fence_epoch = 2;
    compound_ok.writes.push(write("D", vec![]));
    rebind(&mut compound_ok);
    let receipt = db.commit(&compound_ok).unwrap().0;
    assert_eq!(receipt.commit_seq, 3);
    assert_eq!(db.record_count(&domain).unwrap(), 4);
    assert_eq!(db.history_count(&domain).unwrap(), 4);
}
