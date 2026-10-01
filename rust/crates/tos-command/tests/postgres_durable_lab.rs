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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postgres::{Client, NoTls};
use tos_command::{
    AttemptResolution, CancelOutcome, ColdWorkspaceLimits, CommitShadowAttempt, DurableError,
    DurablePgCoordinator, DurableShadowMember, PrivateGenerationWorkspace, RegisterShadowAttempt,
    ShadowWriteIdentity, StreamedGenerationProfile, durable_shadow_delta,
    durable_shadow_delta_prepared, lab_record_bytes,
};
use tos_foundation::Digest256;
use tos_segment_store::{
    AttemptRecovery, FrameInput, GenerationNamespaceV1, GenerationReadLimits,
    GenerationShapeLimits, KeyComparatorV1, OwnerBinding, PackedPartitionRefV1, SegmentLimits,
    SegmentStore, VerificationBudget, describe_placement_partition,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
static INIT_SCHEMA: Once = Once::new();

const PROFILE_ID: &[u8] = b"cmd2.lab.embedded-revision";
const PROFILE_VERSION: &[u8] = b"1";

struct ScratchRoot(PathBuf, bool);

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
        let configured_root = std::env::var_os("TOS_CMD2_LAB_ROOT").map(PathBuf::from);
        let retain_failure = match std::env::var_os("TOS_CMD2_LAB_RETAIN_ON_FAILURE") {
            None => false,
            Some(value) if value == "1" => {
                assert!(
                    configured_root
                        .as_ref()
                        .is_some_and(|root| root.is_absolute()),
                    "failure retention requires an explicit absolute admitted lab root"
                );
                true
            }
            Some(_) => panic!("unsupported lab failure retention setting"),
        };
        let path = configured_root
            .unwrap_or_else(std::env::temp_dir)
            .join(name);
        fs::create_dir_all(&path).expect("private STO lab root created");
        Self(path, retain_failure)
    }
}

impl Drop for ScratchRoot {
    fn drop(&mut self) {
        // Preserve only this invocation's owned fixture during unwinding.
        // The admitted batch owner accounts for and disposes of retained bytes.
        if self.1 && std::thread::panicking() {
            eprintln!(
                "retained failed private STO lab fixture: {}",
                self.0.display()
            );
            return;
        }
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

fn database_url() -> String {
    let url = std::env::var("TOS_CMD_POSTGRES_URL")
        .expect("postgres_durable_lab requires an explicitly selected PostgreSQL URL");
    assert!(
        !url.trim().is_empty(),
        "postgres_durable_lab requires a nonempty PostgreSQL URL"
    );
    url
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

// Same finite held/revocable projection-grant law as native_corpus_query.
// These exact produced private carriers are synthetic read authorization only;
// neither the managed proof nor corpus:create supplies production read admission.
struct ManagedFixtureStageOwner;
impl tos_compiler::knowledge_stage::StageOwner for ManagedFixtureStageOwner {
    fn verify_receipt(
        &self,
        _: &tos_compiler::knowledge_stage::ExactInputReceipt,
    ) -> tos_compiler::Result<()> {
        Ok(())
    }
    fn recheck_sealed_cut(
        &self,
        _: &tos_compiler::knowledge_stage::ExactInputReceipt,
    ) -> tos_compiler::Result<()> {
        Ok(())
    }
}
struct ManagedFixtureStageIsolation;
impl tos_compiler::knowledge_stage::StageIsolation for ManagedFixtureStageIsolation {
    fn verify(
        &self,
        _: &std::path::Path,
        _: tos_compiler::knowledge_stage::StageLimits,
        _: tos_compiler::knowledge_stage::WritePhase,
    ) -> tos_compiler::Result<()> {
        Ok(())
    }
}
type ManagedFixtureCarrier = tos_compiler::knowledge_full_fixture::NativeFixtureProducedCarrier;
fn fixture_search_kind(
    kind: tos_compiler::knowledge_full_fixture::NativeFixtureCarrierKind,
) -> tos_query::search_v2::SearchKind {
    match kind {
        tos_compiler::knowledge_full_fixture::NativeFixtureCarrierKind::Node => {
            tos_query::search_v2::SearchKind::Nodes
        }
        tos_compiler::knowledge_full_fixture::NativeFixtureCarrierKind::Relation => {
            tos_query::search_v2::SearchKind::Relations
        }
    }
}

#[derive(Clone)]
struct ManagedFixtureReadGrant {
    selected: tos_compiler::KnowledgeSelectedExpectation,
    original: tos_compiler::NavigationOriginalReceipt,
    carriers: std::sync::Arc<Vec<ManagedFixtureCarrier>>,
    operation: String,
    withdrawn: std::sync::Arc<AtomicBool>,
    active: std::sync::Arc<AtomicU64>,
}
impl ManagedFixtureReadGrant {
    fn new<P: tos_command::source_managed_selection::ManagedSelectedProof>(
        parent: &tos_command::source_managed_selection::ManagedAgentSelectedParent<P>,
        carriers: Vec<ManagedFixtureCarrier>,
    ) -> Self {
        Self {
            selected: parent.selection_expectation().clone(),
            original: parent.navigation_original_receipt().clone(),
            carriers: std::sync::Arc::new(carriers),
            operation: tos_query::NODE_INSPECT_OPERATION.into(),
            withdrawn: std::sync::Arc::new(AtomicBool::new(false)),
            active: std::sync::Arc::new(AtomicU64::new(0)),
        }
    }
    fn policy(&self) -> tos_query::search_v2::CurrentPolicyBinding {
        tos_query::search_v2::CurrentPolicyBinding {
            scope: "private-synthetic-produced-agent-carriers".into(),
            issuer_ref: "synthetic-test-only:no-production-read-authority".into(),
            authorization_receipt_id: "synthetic-private-agent-disclosure".into(),
            policy_epoch: "fixture-1".into(),
            withdrawal_generation: "fixture-1".into(),
        }
    }
    fn inspect_check(&self) -> Result<(), tos_query::search_v2::SearchV2Error> {
        if self.withdrawn.load(Ordering::SeqCst) {
            Err(tos_query::search_v2::SearchV2Error {
                code: tos_query::search_v2::SearchV2ErrorCode::StalePolicy,
                message: "finite projection hold withdrawn",
            })
        } else {
            Ok(())
        }
    }
    fn catalog_check(&self) -> Result<(), tos_query::CatalogError> {
        if self.withdrawn.load(Ordering::SeqCst) {
            Err(tos_query::CatalogError {
                code: tos_query::CatalogErrorCode::Unauthorized,
                message: "finite projection hold withdrawn",
            })
        } else {
            Ok(())
        }
    }
    fn inspect_scope(&self) -> tos_query::IndexedDisclosureScope {
        let policy = self.policy();
        tos_query::IndexedDisclosureScope {
            operation_id: self.operation.clone(),
            carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
            intended_use: tos_query::INSPECT_INTENDED_USE.into(),
            selected_model_receipt_id: self.selected.owner_receipt_id.clone(),
            source_cut: self.selected.source_cut.clone(),
            through_commit_seq: self.selected.through_commit_seq,
            source_membership_root: Digest256::from_hex(&self.selected.membership_root).unwrap(),
            descriptor_sha256: Digest256::from_hex(&self.selected.descriptor_sha256).unwrap(),
            selected_index_sha256: Digest256::from_hex(&self.selected.model_sha256).unwrap(),
            policy_issuer_ref: policy.issuer_ref,
            policy_receipt_id: policy.authorization_receipt_id,
            policy_scope: policy.scope,
            policy_epoch: policy.policy_epoch,
            withdrawal_generation: policy.withdrawal_generation,
        }
    }
    fn catalog_scope(&self) -> tos_query::CatalogDisclosureScope {
        let s = self.inspect_scope();
        tos_query::CatalogDisclosureScope {
            operation_id: "tos.knowledge.catalog".into(),
            carrier_layer: s.carrier_layer,
            intended_use: "read_only_public_knowledge_catalog_v1".into(),
            selected_model_receipt_id: s.selected_model_receipt_id,
            source_cut: s.source_cut,
            through_commit_seq: s.through_commit_seq,
            source_membership_root: s.source_membership_root,
            descriptor_sha256: s.descriptor_sha256,
            selected_index_sha256: s.selected_index_sha256,
            catalog_packet_sha256: Digest256::from_hex(&self.selected.catalog_packet_sha256)
                .unwrap(),
            policy_issuer_ref: s.policy_issuer_ref,
            policy_receipt_id: s.policy_receipt_id,
            policy_scope: s.policy_scope,
            policy_epoch: s.policy_epoch,
            withdrawal_generation: s.withdrawal_generation,
        }
    }
    fn hold(&self) -> ManagedFixtureReadHold {
        self.active.fetch_add(1, Ordering::SeqCst);
        ManagedFixtureReadHold {
            withdrawn: self.withdrawn.clone(),
            active: self.active.clone(),
        }
    }
}
struct ManagedFixtureReadHold {
    withdrawn: std::sync::Arc<AtomicBool>,
    active: std::sync::Arc<AtomicU64>,
}
impl Drop for ManagedFixtureReadHold {
    fn drop(&mut self) {
        assert!(self.active.fetch_sub(1, Ordering::SeqCst) > 0);
    }
}
impl tos_query::InspectDisclosureLease for ManagedFixtureReadHold {
    fn recheck(&mut self) -> Result<(), tos_query::search_v2::SearchV2Error> {
        if self.withdrawn.load(Ordering::SeqCst) {
            Err(tos_query::search_v2::SearchV2Error {
                code: tos_query::search_v2::SearchV2ErrorCode::StalePolicy,
                message: "finite projection hold withdrawn",
            })
        } else {
            Ok(())
        }
    }
}
impl tos_query::CatalogDisclosureLease for ManagedFixtureReadHold {
    fn recheck(&mut self) -> Result<(), tos_query::CatalogError> {
        if self.withdrawn.load(Ordering::SeqCst) {
            Err(tos_query::CatalogError {
                code: tos_query::CatalogErrorCode::Unauthorized,
                message: "finite projection hold withdrawn",
            })
        } else {
            Ok(())
        }
    }
}
impl<'hold> tos_query::CatalogCurrentAuthority<'hold> for ManagedFixtureReadGrant {
    fn policy_binding(&self) -> tos_query::search_v2::CurrentPolicyBinding {
        self.policy()
    }
    fn disclosure_scope(&self) -> tos_query::CatalogDisclosureScope {
        self.catalog_scope()
    }
    fn check_selected(&mut self) -> Result<(), tos_query::CatalogError> {
        self.catalog_check()
    }
    fn authorize_current(&mut self, sha: Digest256) -> Result<(), tos_query::CatalogError> {
        self.catalog_check()?;
        assert_eq!(
            sha,
            Digest256::from_hex(&self.selected.catalog_packet_sha256).unwrap()
        );
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &tos_query::CatalogDisclosureScope,
        sha: Digest256,
    ) -> Result<Box<dyn tos_query::CatalogDisclosureLease + 'hold>, tos_query::CatalogError> {
        self.catalog_check()?;
        assert_eq!(scope, &self.catalog_scope());
        assert_eq!(
            sha,
            Digest256::from_hex(&self.selected.catalog_packet_sha256).unwrap()
        );
        Ok(Box::new(self.hold()))
    }
}
impl<'hold> tos_query::InspectCurrentAuthority<'hold> for ManagedFixtureReadGrant {
    fn policy_binding(&self) -> tos_query::search_v2::CurrentPolicyBinding {
        self.policy()
    }
    fn disclosure_scope(&self) -> tos_query::IndexedDisclosureScope {
        self.inspect_scope()
    }
    fn check_selected(&mut self) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inspect_check()
    }
    fn authorize_current(
        &mut self,
        carrier: &tos_query::InspectedCarrier,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inspect_check()?;
        assert!(
            self.carriers
                .iter()
                .any(|r| fixture_search_kind(r.kind) == carrier.kind
                    && r.id == carrier.id
                    && r.source_order == carrier.position
                    && r.payload_sha256 == carrier.payload_sha256)
        );
        Ok(())
    }
    fn authorize_navigation_original_current(
        &mut self,
        receipt: &tos_compiler::NavigationOriginalReceipt,
        position: i64,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inspect_check()?;
        assert_eq!(
            serde_json::to_value(receipt).unwrap(),
            serde_json::to_value(&self.original).unwrap()
        );
        assert!(position >= -1 && position < receipt.rights as i64);
        assert_eq!(Digest256::of_bytes(raw), sha);
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &tos_query::IndexedDisclosureScope,
        consulted: &[tos_query::ObservedInspectCarrier],
    ) -> Result<
        Box<dyn tos_query::InspectDisclosureLease + 'hold>,
        tos_query::search_v2::SearchV2Error,
    > {
        self.inspect_check()?;
        assert_eq!(scope, &self.inspect_scope());
        assert!(
            consulted
                .iter()
                .all(|c| self
                    .carriers
                    .iter()
                    .any(|r| fixture_search_kind(r.kind) == c.kind
                        && r.id == c.id
                        && r.source_graph == c.source_graph
                        && r.source_order == c.position
                        && r.payload_sha256 == c.payload_sha256))
        );
        Ok(Box::new(self.hold()))
    }
}
struct ManagedFixtureOutput {
    bytes: Vec<u8>,
    active: std::sync::Arc<AtomicU64>,
    flushed: bool,
}
impl std::io::Write for ManagedFixtureOutput {
    fn write(&mut self, raw: &[u8]) -> io::Result<usize> {
        assert!(
            self.active.load(Ordering::SeqCst) > 0,
            "actual fixture read lease held during write"
        );
        self.bytes.extend_from_slice(raw);
        Ok(raw.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        assert!(
            self.active.load(Ordering::SeqCst) > 0,
            "actual fixture read lease held through final flush"
        );
        self.flushed = true;
        Ok(())
    }
}
struct ManagedFixtureNoCheckpoints;
impl tos_query::knowledge_exploration::ExplorationCheckpoints for ManagedFixtureNoCheckpoints {
    fn load(
        &mut self,
        _: &str,
        _: &str,
    ) -> Result<
        tos_query::knowledge_exploration::ExplorationCheckpoint,
        tos_query::search_v2::SearchV2Error,
    > {
        Err(tos_query::search_v2::SearchV2Error {
            code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
            message: "fixture grants catalog/node/relation only",
        })
    }
    fn prepare(
        &mut self,
        _: Option<&str>,
        _: &str,
        _: Option<&tos_query::knowledge_exploration::ExplorationState>,
        _: &tos_foundation::JsonValue,
        _: tos_query::knowledge_exploration::ExplorationBudget,
    ) -> Result<
        Box<dyn tos_query::knowledge_exploration::PreparedExplorationCheckpoint>,
        tos_query::search_v2::SearchV2Error,
    > {
        Err(tos_query::search_v2::SearchV2Error {
            code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
            message: "fixture grants catalog/node/relation only",
        })
    }
}
fn managed_fixture_query_budgets() -> tos_access::knowledge::SelectedKnowledgeBudgets {
    let read = tos_query::InspectBudget {
        max_open_vm_steps: 100_000_000,
        max_read_vm_steps: 1_000_000,
        max_matches: 64,
        max_rows: 1000,
        max_field_bytes: 8192,
        max_payload_bytes: 1_000_000,
        max_decoded_bytes: 8_000_000,
        max_response_bytes: 1_000_000,
        json: tos_foundation::JsonLimits::default(),
    };
    tos_access::knowledge::SelectedKnowledgeBudgets {
        catalog: tos_query::CatalogBudget {
            max_open_vm_steps: 100_000_000,
            max_read_vm_steps: 1_000_000,
            max_packet_bytes: 1_000_000,
            max_decoded_bytes: 1_000_032,
            json: tos_foundation::JsonLimits::default(),
        },
        inspect: read,
        lens: tos_query::knowledge_lens::LensBudget {
            inspect: read,
            max_candidates: 1000,
            max_path_steps: 1000,
            max_adjacency_rows: 1000,
            block_size: 64,
        },
        exploration: tos_query::knowledge_exploration::ExplorationBudget {
            read,
            max_work_units: 1,
            max_session_nodes: 1000,
            max_session_relations: 1000,
            max_state_bytes: 1_000_000,
            max_checkpoint_bytes: 2_000_000,
            max_checkpoints: 8,
        },
    }
}

#[test]
fn maintained_agent_creation_commits_current_indexes_and_reopens_original_bytes() {
    maintained_agent_creation_operation::<tos_compiler::managed_source::ManagedSourceProofV1>(
        false,
    );
}

#[test]
fn addressed_agent_creation_reaches_selected_reader_and_cold_restore() {
    if let Ok(phase) = std::env::var("TOS_CMD2_V2_BACKUP_PHASE") {
        addressed_backup_restore_phase(&phase);
        return;
    }
    maintained_agent_creation_operation::<tos_compiler::managed_source::ManagedSourceProofV2>(true);
}

// Transport phases execute in children of the same protected fixture image
// under the maintained 64 MiB file limit. Their receipts describe transport;
// the parent independently cold-verifies the restored source before reading.
fn addressed_backup_restore_phase(phase: &str) {
    use tos_command::backup_recovery::{
        BackupSelection, PgTool, RestoreSelection, backup_quiescent, restore_into_fresh,
    };
    let url = database_url();
    let domain = std::env::var("TOS_CMD2_V2_DOMAIN").unwrap();
    let store = PathBuf::from(std::env::var_os("TOS_CMD2_V2_STORE").unwrap());
    let backup = PathBuf::from(std::env::var_os("TOS_CMD2_V2_BACKUP").unwrap());
    let remaining: u64 = std::env::var("TOS_CMD2_V2_PHASE_MS")
        .unwrap()
        .parse()
        .unwrap();
    assert!(remaining > 0 && remaining <= 240_000);
    let deadline = Instant::now() + Duration::from_millis(remaining);
    let cancelled = AtomicBool::new(false);
    let tool_kind = match phase {
        "backup" => "DUMP",
        "restore" => "RESTORE",
        _ => panic!("unknown V2 transport phase"),
    };
    let path = PathBuf::from(std::env::var_os(format!("TOS_CMD2_PG_{tool_kind}_PATH")).unwrap());
    let sha256 = std::env::var(format!("TOS_CMD2_PG_{tool_kind}_SHA256")).unwrap();
    let tool = PgTool {
        path: &path,
        sha256: &sha256,
    };
    let result = if phase == "backup" {
        backup_quiescent(
            &BackupSelection {
                pg_url: &url,
                domain: &domain,
                store_root: &store,
                backup_root: &backup,
                tool,
                store_limits: limits(),
                quiescent_owner_confirmed: true,
            },
            deadline,
            &cancelled,
        )
    } else {
        let receipt_sha256 = std::env::var("TOS_CMD2_V2_RECEIPT_SHA256").unwrap();
        restore_into_fresh(
            &RestoreSelection {
                pg_url: &url,
                domain: &domain,
                store_root: &store,
                backup_root: &backup,
                receipt_sha256: &receipt_sha256,
                tool,
                store_limits: limits(),
                fresh_target_owner_confirmed: true,
            },
            deadline,
            &cancelled,
        )
    }
    .expect("maintained V2 DB/store transport must fit and verify its unchanged profile");
    println!(
        "CMD2_V2_TRANSPORT {}",
        serde_json::to_string(&result).unwrap()
    );
}

fn run_addressed_transport_phase(
    phase: &str,
    url: &str,
    domain: &str,
    store: &std::path::Path,
    backup: &std::path::Path,
    receipt_sha256: Option<&str>,
    deadline: Instant,
) {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .expect("whole operation deadline remains");
    let mut command = Command::new("/usr/bin/prlimit");
    command
        .args(["--fsize=67108864:67108864", "--"])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "addressed_agent_creation_reaches_selected_reader_and_cold_restore",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("TOS_CMD_POSTGRES_URL", url)
        .env("TOS_CMD2_V2_BACKUP_PHASE", phase)
        .env("TOS_CMD2_V2_DOMAIN", domain)
        .env("TOS_CMD2_V2_STORE", store)
        .env("TOS_CMD2_V2_BACKUP", backup)
        .env("TOS_CMD2_V2_PHASE_MS", remaining.as_millis().to_string());
    if let Some(sha) = receipt_sha256 {
        command.env("TOS_CMD2_V2_RECEIPT_SHA256", sha);
    }
    let mut child = command.spawn().expect("owned transport child starts");
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "V2 {phase} child refused the actual transport/profile: {status}"
            );
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("own overdue transport child stopped");
            child.wait().unwrap();
            panic!("whole V2 operation exceeded its deadline during {phase}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn addressed_journal_snapshot(url: &str, domain: &str) -> serde_json::Value {
    use postgres::fallible_iterator::FallibleIterator;
    let mut db = Client::connect(url, NoTls).unwrap();
    db.batch_execute("SET statement_timeout='15000'; SET default_transaction_read_only=on")
        .unwrap();
    let mut tx = db
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .unwrap();
    let generation: i64 = tx
        .query_one(
            "SELECT generation FROM cmd2_audit_fence WHERE domain=$1",
            &[&domain],
        )
        .unwrap()
        .get(0);
    let marker: String = tx
        .query_one(
            "SELECT row_to_json(p)::text FROM cmd2_audit_delta_v1_domain p WHERE domain=$1",
            &[&domain],
        )
        .unwrap()
        .get(0);
    let mut hash = tos_foundation::Digest256Hasher::new();
    let mut count = 0u64;
    let mut rows = tx.query_raw("SELECT sha256(convert_to(row_to_json(j)::text,'UTF8')) FROM cmd2_audit_delta_v1 j WHERE domain=$1 ORDER BY generation LIMIT 4097", [&domain]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        count += 1;
        assert!(
            count <= 4096,
            "finite synthetic journal witness stays bounded"
        );
        let digest: Vec<u8> = row.get(0);
        assert_eq!(digest.len(), 32);
        hash.update(&digest);
    }
    drop(rows);
    tx.commit().unwrap();
    serde_json::json!([generation, marker, count, hash.finalize().to_hex()])
}

fn maintained_agent_creation_operation<
    P: tos_command::source_managed_selection::ManagedSelectedProof,
>(
    addressed: bool,
) {
    use std::collections::BTreeMap;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use tos_command::source_command::{CommandContext, SourceFile};
    use tos_command::source_creation::prepare_source_creation_from_captures;
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
    use tos_compiler::managed_source::ManagedProducerProof;
    use tos_foundation::{
        CanonicalProfile, JsonLimits, JsonMode, RelativePath, canonical_bytes_v1, parse_json,
    };
    use tos_source_store::{
        CorpusCutReader, CorpusReader, CutReadLimits, ReadLimits, SoftwareCaptureReader,
        SoftwareCaptureSelectionV1,
    };
    use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
    use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor};

    fn canonical(value: &serde_json::Value) -> Vec<u8> {
        let raw = serde_json::to_vec(value).unwrap();
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        canonical_bytes_v1(
            parsed.root(),
            CanonicalProfile::CorpusSnapshotV1,
            JsonLimits::default(),
        )
        .unwrap()
    }
    fn clean(command: &mut Command) {
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_")
                || key == "PYTHONPATH"
                || key == "PYTHONHOME"
            {
                command.env_remove(key);
            }
        }
        command
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            // The real capture/restore Git children share this fixture's 1GiB
            // AS envelope. Keep their pack mmap windows bounded after scrubbing
            // inherited Git configuration; source/object semantics stay exact.
            .env("GIT_CONFIG_COUNT", "2")
            .env("GIT_CONFIG_KEY_0", "core.packedGitWindowSize")
            .env("GIT_CONFIG_VALUE_0", "16m")
            .env("GIT_CONFIG_KEY_1", "core.packedGitLimit")
            .env("GIT_CONFIG_VALUE_1", "64m");
    }
    let url = database_url();
    let mut lab = Lab::new(&url);
    if addressed {
        // Exercise the actual audit query builders and Rust bindings before
        // Git capture, schema workers, or the full cold source/model setup.
        lab.db.enable_addressed_audit_protocol().unwrap();
        println!("STO addressed audit SQL binding/decode precondition PASS");
    }
    // PostgreSQL must preserve named composite JSON and exact accepted
    // aggregates after moving the admission limit below serialization.
    let mut admission_sql = Client::connect(&url, NoTls).unwrap();
    admission_sql.batch_execute(
        "SET statement_timeout = '5s';
         CREATE TEMP TABLE admission_shape(domain text, ordinal integer, payload text, optional integer);
         INSERT INTO admission_shape VALUES('selected',1,'named \"bytes\"',NULL),
             ('selected',2,'unicode λ',7),('other',1,repeat('x',1024),NULL)"
    ).unwrap();
    let full = admission_sql
        .query_one(
            "SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0),
                coalesce(sum(octet_length(row_to_json(t)::text)),0)
         FROM admission_shape t WHERE domain=$1",
            &[&"selected"],
        )
        .unwrap();
    for limit in [2i64, 3] {
        let bounded = admission_sql
            .query_one(
                "SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0),
                    coalesce(sum(octet_length(row_to_json(t)::text)),0)
             FROM (SELECT * FROM admission_shape WHERE domain=$1 LIMIT $2) t",
                &[&"selected", &limit],
            )
            .unwrap();
        assert_eq!(bounded.get::<_, i64>(0), full.get::<_, i64>(0));
        assert_eq!(bounded.get::<_, i32>(1), full.get::<_, i32>(1));
        assert_eq!(bounded.get::<_, i64>(2), full.get::<_, i64>(2));
    }
    let named: String = admission_sql
        .query_one(
            "SELECT row_to_json(t)::text FROM (SELECT * FROM admission_shape
         WHERE domain=$1 AND ordinal=1 LIMIT $2) t",
            &[&"selected", &2i64],
        )
        .unwrap()
        .get(0);
    let named: serde_json::Value = serde_json::from_str(&named).unwrap();
    assert_eq!(
        named,
        serde_json::json!({"domain":"selected","ordinal":1,
        "payload":"named \"bytes\"","optional":null})
    );
    admission_sql
        .execute(
            "INSERT INTO admission_shape VALUES('selected',3,repeat('x',4096),NULL)",
            &[],
        )
        .unwrap();
    let plan = admission_sql
        .query(
            "EXPLAIN (ANALYZE,COSTS OFF,TIMING OFF,SUMMARY OFF)
         SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0),
                coalesce(sum(octet_length(row_to_json(t)::text)),0)
         FROM (SELECT * FROM admission_shape WHERE domain=$1 LIMIT $2) t",
            &[&"selected", &2i64],
        )
        .unwrap();
    assert!(
        plan.iter().any(|row| {
            let line: String = row.get(0);
            line.contains("Limit") && line.contains("actual rows=2 loops=1")
        }),
        "PostgreSQL must limit aggregate input to remaining+1 rows"
    );
    let overflow: i64 = admission_sql
        .query_one(
            "SELECT count(*) FROM (SELECT * FROM admission_shape WHERE domain=$1 LIMIT $2) t",
            &[&"selected", &2i64],
        )
        .unwrap()
        .get(0);
    assert!(overflow > 1, "remaining+1 proves row-budget refusal");
    eprintln!("STO cold metadata named JSON/exact accepted aggregates/overflow witness PASS");
    let root = ScratchRoot::new();
    let repository = match std::env::var_os("TOS_CMD2_LAB_SOURCE_ROOT") {
        Some(path) => {
            let path = PathBuf::from(path);
            assert!(
                path.is_absolute(),
                "selected lab source root must be absolute"
            );
            path.canonicalize().unwrap()
        }
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap(),
    };
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(240);
    // Only the two selected-model candidates need the explicitly admitted
    // fs-verity filesystem. Corpus, catalog staging, PG and generation spool
    // paths keep their existing private /srv scratch route.
    let model_root = match std::env::var_os("TOS_CMD2_LAB_NATIVE_MODEL_ROOT") {
        Some(path) => {
            let path = PathBuf::from(path);
            assert!(path.is_absolute(), "native model root must be absolute");
            let path = path.canonicalize().unwrap();
            assert!(path.is_dir());
            path
        }
        None => root.0.clone(),
    };
    let isolated = IsolatedCreationRoot::create(&root.0, deadline, &cancelled).unwrap();
    // The same actual maintained resources as the existing native creation
    // scenario. No philosophical corpus or payload discovery is performed.
    let inputs = [
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
        "scripts/source_witness_human_forms.py",
        "scripts/build_source_witness_catalog.py",
        "scripts/source_record_profiles.py",
        "scripts/native_text_binding.py",
        "scripts/source_owner_context.py",
        "scripts/source_witness_bibliographic_graph_common.py",
        "rust/crates/tos-command/src/source_creation.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "rust/crates/tos-compiler/src/knowledge_normalization.rs",
        "ToS/contracts/historical-record.schema.json",
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/historical-claim.schema.json",
        "ToS/contracts/claim-packet.schema.json",
        "ToS/contracts/knowledge-assessment.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ];
    let mut files = inputs
        .iter()
        .map(|p| (p.to_string(), fs::read(repository.join(p)).unwrap()))
        .collect::<BTreeMap<_, _>>();
    // The actual catalog constructor validates every registry-declared route,
    // including unused profiles. Reuse the source-case's complete finite
    // contract resource closure; do not fake empty routes or patch registries.
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file()
            && entry
                .file_name()
                .to_str()
                .unwrap()
                .ends_with(".schema.json")
        {
            let path = format!("ToS/contracts/{}", entry.file_name().to_str().unwrap());
            files.insert(path, fs::read(entry.path()).unwrap());
        }
    }
    assert!(files.len() <= 512);
    assert!(files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    fs::create_dir_all(isolated.path().join("ToS/source-witnesses/agents")).unwrap();
    for (path, raw) in &files {
        let target = isolated.path().join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    // Existing corpus snapshot transport, not a semantic validator verdict.
    let source_root = root.0.join("source-cut");
    fs::create_dir_all(source_root.join("objects")).unwrap();
    fs::create_dir_all(source_root.join("revisions")).unwrap();
    let members = files
        .iter()
        .filter(|(p, _)| p.starts_with("ToS/"))
        .map(|(path, raw)| {
            let sha = Digest256::of_bytes(raw).to_hex();
            fs::write(source_root.join("objects").join(&sha), raw).unwrap();
            serde_json::json!({"path":path,"sha256":sha,"size_bytes":raw.len(),"mode":420})
        })
        .collect::<Vec<_>>();
    let mut manifest = serde_json::json!({"schema_version":"tos_corpus_snapshot_v1","base_revision":null,"files":members,"identities":{},"dependencies":{},"retirements":[],"validator_sha256":Digest256::of_bytes(b"synthetic transport only, no source admission").to_hex()});
    let revision = tos_foundation::SourceRevision(Digest256::of_bytes(&canonical(&manifest)));
    manifest["revision"] = serde_json::json!(revision.0.to_hex());
    let directory = source_root.join("revisions").join(revision.0.to_hex());
    fs::create_dir(directory.clone()).unwrap();
    fs::write(directory.join("snapshot.json"), canonical(&manifest)).unwrap();
    let read_limits = ReadLimits {
        max_manifest_bytes: 4_194_304,
        max_manifest_entries: 2048,
        max_selected_object_bytes: 8_388_608,
        json: JsonLimits::default(),
    };
    let cut = CorpusReader::open_existing(&source_root, read_limits)
        .unwrap()
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 2048,
                max_total_bytes: 33_554_432,
                max_member_bytes: 8_388_608,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let membership = cut.stream(revision).unwrap().expectation();

    // Reuse the actual maintained software capture/restore program, selecting
    // exact source paths from the composed commit rather than current markers.
    let commit_output = Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .unwrap();
    assert!(commit_output.status.success());
    let commit = String::from_utf8(commit_output.stdout)
        .unwrap()
        .trim()
        .to_owned();
    let program = Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["show", &format!("{commit}:scripts/corpus_archive.py")])
        .output()
        .unwrap();
    assert!(program.status.success());
    let tool = root.0.join("corpus_archive.py");
    fs::write(&tool, program.stdout).unwrap();
    let capture = root.0.join("software-capture");
    let restored = root.0.join("software-restored");
    let mut command = Command::new("python3");
    command
        .arg(&tool)
        .arg("capture")
        .arg("--repo-root")
        .arg(&repository)
        .arg("--commit")
        .arg(&commit)
        .arg("--output")
        .arg(&capture);
    for name in files.keys().filter(|p| !p.starts_with("ToS/")) {
        command.arg("--include-prefix").arg(name);
    }
    clean(&mut command);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut command = Command::new("python3");
    command
        .arg(&tool)
        .arg("restore")
        .arg("--capture")
        .arg(&capture)
        .arg("--output")
        .arg(&restored);
    clean(&mut command);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let capture_raw = fs::read(capture.join("capture.json")).unwrap();
    let captured: serde_json::Value = serde_json::from_slice(&capture_raw).unwrap();
    let software = SoftwareCaptureReader::open(
        &capture,
        &restored,
        SoftwareCaptureSelectionV1 {
            source_git_commit: commit,
            source_git_tree: captured["source_git_tree"].as_str().unwrap().into(),
            capture_manifest_sha256: Digest256::of_bytes(&capture_raw),
        },
        read_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let paths = files
        .keys()
        .filter(|p| !p.starts_with("ToS/"))
        .map(|p| RelativePath::parse(p).unwrap())
        .collect::<Vec<_>>();
    let components = software.select_components(&paths).unwrap();
    let worker_path = PathBuf::from(
        std::env::var_os("TOS_SCHEMA_WORKER_PATH")
            .expect("OPS selects actual schema worker before this whole PG case"),
    );
    assert!(worker_path.is_absolute());
    let worker_identity = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    let new_worker = |selected_cut: &CorpusCutReader| {
        CutWorkerSchemaExecutor::from_cut(
            selected_cut,
            tos_validation::FormatProfile::LegacyPythonObserved20260923,
            worker_identity.clone(),
            ExecutorBudget::laboratory(),
            CutWorkerLimits {
                max_receipts: 128,
                max_receipt_bytes: 262_144,
            },
            deadline,
            &cancelled,
        )
        .unwrap()
    };
    let uid = fs::metadata(isolated.path()).unwrap().uid();
    let mut packages = Vec::new();
    let mut contexts = Vec::new();
    let mut owners = Vec::new();
    // B has a disjoint home; C shares A's form ID. Both original proposals
    // must be fenced after A changes the actually consumed whole inventory.
    for (name, form_name) in [("a", "a"), ("b", "b"), ("c", "a")] {
        let record = serde_json::json!({"schema_version":"tos_corpus_record_v1","record_type":"agent","record_id":format!("tos.agent.synthetic-durable-{name}"),"record_version":1,"preferred_label":"Synthetic custody subject","variant_labels":[],"identity_status":"provisional","source_refs":["synthetic-test-only:no-admission"],"external_identifiers":[],"same_as_posture":"no_equivalence_claim","notes":"Synthetic source mechanics only.","supersedes_ref":null,"field_languages":{"preferred_label":{"language":"en","script":"Latn"},"notes":{"language":"en","script":"Latn"}}});
        let config = serde_json::json!({"schema_version":"tos_local_corpus_create_owner_v2","uid":uid,"principal_id":"software:test-fixture","maker_type":"software","source_root":isolated.path(),"source_path":format!("ToS/source-witnesses/agents/synthetic-durable-{name}/agent.json"),"record_type":"agent","record_id":record["record_id"],"authority_ref":"synthetic-test-only:no-admission","allowed_form_ids":[format!("tos.form.synthetic-durable-{form_name}")],"allowed_operations":["source.create"],"expires_at":"2099-01-01T00:00:00Z","provenance_event_id":format!("tos.event.synthetic-durable-{name}")});
        let owner = isolated.path().join(format!("owner-{name}.json"));
        fs::write(&owner, canonical(&config)).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        owners.push(
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancelled).unwrap(),
        );
        let mut request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","record":record,"forms":[{"form_id":format!("tos.form.synthetic-durable-{form_name}"),"field_id":"metadata.preferred-name"}]});
        let mut context = CommandContext {
            base_revision: revision,
            configuration_raw: canonical(&config),
            request_raw: canonical(&request),
            recorded_at: "2026-09-27T00:00:00Z".into(),
            effective_uid: uid as u64,
            files: files
                .iter()
                .map(|(p, r)| SourceFile {
                    path: RelativePath::parse(p).unwrap(),
                    raw: r.clone(),
                })
                .collect(),
        };
        let mut worker = new_worker(&cut);
        let preview = prepare_source_creation_from_captures(
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancelled,
        )
        .unwrap()
        .preview()
        .unwrap();
        request["operation"] = serde_json::json!("source.create");
        request["command_id"] = serde_json::json!(format!("synthetic:durable-{name}"));
        for (key, field) in [
            ("expected_configuration", "owner_configuration"),
            ("expected_dependencies", "expected_dependencies"),
        ] {
            request[key] = serde_json::json!(preview.object_get(field).unwrap().as_str().unwrap());
        }
        request["expected_source"] = serde_json::Value::Null;
        request["expected_revision"] = serde_json::Value::Null;
        context.request_raw = canonical(&request);
        let prepared = prepare_source_creation_from_captures(
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancelled,
        )
        .unwrap();
        packages.push(
            prepared
                .serialize(&software, &components, &mut worker, deadline, &cancelled)
                .unwrap(),
        );
        worker.finish(deadline, &cancelled).unwrap();
        contexts.push(context);
    }
    let generation_profile = StreamedGenerationProfile {
        max_commit_seq: 4096,
        max_members: 512,
        max_pins: 4096,
        max_segment_bytes: 64 * 1024 * 1024,
        max_membership_key_bytes: 16 * 1024 * 1024,
        max_metadata_rows: 100_000,
        max_metadata_bytes: 64 * 1024 * 1024,
        max_elapsed: Duration::from_secs(120),
        max_pg_temp_bytes: 64 * 1024 * 1024,
        max_sql_statement_ms: 60_000,
        generation: GenerationReadLimits {
            max_descriptor_bytes: 1024 * 1024,
            shape: GenerationShapeLimits {
                max_partitions: 128,
                max_rows_per_partition: 4,
                max_key_bytes: 4096,
                max_leaf_bytes: 256 * 1024,
            },
            max_stream_rows: 512,
            max_stream_key_bytes: 16 * 1024 * 1024,
        },
        rows_per_leaf: 4,
    };
    let tree_limits = tos_segment_store::AuthenticatedTreeLimitsV1 {
        max_key_bytes: 4096,
        max_value_bytes: 1_048_576,
        max_kind_bytes: 128,
        max_node_bytes: 1_048_576,
        max_children: 16,
        max_nodes: 100_000,
        max_total_bytes: 64 * 1024 * 1024,
        max_rows: 4096,
    };
    let initial_cohort = if std::env::var_os("TOS_CMD2_STREAMED_COLD_V1").as_deref()
        == Some(std::ffi::OsStr::new("1"))
    {
        assert!(
            addressed,
            "streamed bootstrap requires the addressed consumer"
        );
        let bootstrap_scratch = ScratchRoot::new();
        let bootstrap_workspace = PrivateGenerationWorkspace::open(
            &bootstrap_scratch.0,
            ColdWorkspaceLimits {
                max_scratch_written_bytes: 64 * 1024 * 1024,
                max_run_bytes: 16 * 1024,
                max_runs: 128,
                merge_fan_in: 4,
                max_rows: 4096,
                max_key_bytes: 4096,
            },
        )
        .unwrap();
        let bootstrap_context = CommandContext {
            base_revision: revision,
            configuration_raw: contexts[0].configuration_raw.clone(),
            request_raw: contexts[0].request_raw.clone(),
            recorded_at: contexts[0].recorded_at.clone(),
            effective_uid: contexts[0].effective_uid,
            files: contexts[0]
                .files
                .iter()
                .filter(|file| !file.path.as_str().starts_with("ToS/"))
                .cloned()
                .collect(),
        };
        let mut work = tos_command::source_cohort::StreamedColdSourceWorkV1::default();
        let source_reader = CorpusReader::open_existing(&source_root, read_limits).unwrap();
        let streamed_original_result = bootstrap_workspace.open_streamed_source_cut(
            &source_reader,
            revision,
            tos_source_store::StreamedCutReadLimitsV1 {
                manifest_json: JsonLimits {
                    max_bytes: 4_194_304,
                    ..JsonLimits::default()
                },
                max_manifest_entries: 2048,
                cut: CutReadLimits {
                    max_revisions: 4,
                    max_members: 2048,
                    max_total_bytes: 33_554_432,
                    max_member_bytes: 8_388_608,
                },
                max_index_bytes: 16 * 1024 * 1024,
                max_manifest_row_bytes: 1_048_576,
                sqlite_cache_bytes: 1_048_576,
            },
            deadline,
            &cancelled,
            &mut work,
        );
        println!(
            "streamed original cut open: result={:?}; work={work:?}",
            streamed_original_result.as_ref().map(|_| ())
        );
        let streamed_original = streamed_original_result.unwrap();
        assert!(work.original_index_length_observed > 0);
        assert!(work.original_index_allocated_observed > 0);
        assert_eq!(std::fs::read_dir(&bootstrap_scratch.0).unwrap().count(), 0,
            "streamed SQLite must write only its charged unnamed inode");
        assert_eq!(
            streamed_original
                .revision_at(0)
                .unwrap()
                .unwrap()
                .membership,
            membership
        );
        let image = tos_validation::executor::VerifiedWorkerImageHandle::prepare(
            worker_identity.clone(),
            ExecutorBudget::laboratory(),
            deadline,
            &cancelled,
        )
        .unwrap();
        let mut worker = CutWorkerSchemaExecutor::from_streamed_cut_with_image(
            &streamed_original,
            tos_validation::FormatProfile::LegacyPythonObserved20260923,
            &image,
            ExecutorBudget::laboratory(),
            CutWorkerLimits {
                max_receipts: 128,
                max_receipt_bytes: 262_144,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
        let mut tree_work = tos_segment_store::AuthenticatedTreeWorkV1::default();
        let result = lab.db.bootstrap_source_cohort_streamed(
            &lab.store,
            &lab.domain,
            &streamed_original,
            revision,
            membership,
            &bootstrap_context,
            &software,
            &components,
            &mut worker,
            contract_digest(),
            2048,
            33_554_432,
            1_048_576,
            "ToS/contracts/corpus-record.schema.json",
            &bootstrap_workspace,
            generation_profile,
            tos_command::source_cohort::StreamedColdSourceLimitsV1 {
                max_rows: 100_000,
                max_logical_bytes: 64 * 1024 * 1024,
                max_key_bytes: 4096,
                max_value_bytes: 1_048_576,
                max_placement_bytes: 1_048_576,
                max_sqlite_file_bytes: 16 * 1024 * 1024,
                max_vm_steps: 200_000_000,
            },
            tos_validation::item_rules::ItemLimits {
                max_member_bytes: 8_388_608,
                max_total_bytes: 64 * 1024 * 1024,
                max_state_bytes: 16 * 1024 * 1024,
                max_issues: 256,
                deadline,
            },
            tree_limits,
            &mut work,
            &mut tree_work,
            deadline,
            std::sync::Arc::clone(&cancelled),
        );
        eprintln!(
            "CMD2_STREAMED_BOOTSTRAP result_ok={} source_work={work:?} tree_work={tree_work:?}",
            result.is_ok()
        );
        let initial = result.unwrap();
        assert!(work.derived_index.sqlite_file_len_high_water > 0);
        assert!(work.derived_index.sqlite_allocated_bytes_high_water > 0);
        assert_eq!(std::fs::read_dir(&bootstrap_scratch.0).unwrap().count(), 0,
            "assessment SQLite must retain only charged unnamed scratch");
        let cohort = initial.cohort().clone();
        for expected in cut.current().members() {
            let observed = initial
                .read_current_member(
                    &mut lab.db,
                    &lab.store,
                    &expected.path,
                    8_388_608,
                    deadline,
                    &cancelled,
                )
                .unwrap();
            assert_eq!(&observed.metadata, expected);
            assert_eq!(
                observed.dependency_claims.as_deref(),
                cut.current().indexed_dependencies(&expected.path)
            );
        }
        cohort
    } else {
        let mut worker = new_worker(&cut);
        let initial = lab
            .db
            .bootstrap_source_cohort(
                &lab.store,
                &lab.domain,
                &cut,
                revision,
                membership,
                &contexts[0],
                &software,
                &components,
                &mut worker,
                contract_digest(),
                deadline,
                &cancelled,
            )
            .unwrap();
        drop(worker);
        assert_eq!(initial.current_membership, membership);
        for original_member in cut.current().members() {
            assert_eq!(
                &initial.metadata[original_member.path.as_str()],
                original_member
            );
            assert_eq!(
                initial.dependency_claims[original_member.path.as_str()].as_deref(),
                cut.current().indexed_dependencies(&original_member.path)
            );
        }
        initial.cohort
    };
    let attempts = packages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut registration_worker = new_worker(&cut);
            let attempt = lab
                .db
                .register_source_creation(
                    &lab.store,
                    &initial_cohort,
                    format!("agent-{i}").as_bytes(),
                    p,
                    &mut registration_worker,
                    deadline,
                    &cancelled,
                )
                .unwrap();
            registration_worker.finish(deadline, &cancelled).unwrap();
            attempt
        })
        .collect::<Vec<_>>();
    // Exercise the real registered predicate set independently of the
    // whole-inventory conflict below. A changed generation or unproved
    // definition must refuse before any durable projection is published.
    let mut predicate_sql = Client::connect(&url, NoTls).unwrap();
    let registered_reads: Vec<u8> = predicate_sql
        .query_one(
            "SELECT source_reads FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&lab.domain, &b"agent-0".as_slice()],
        )
        .unwrap()
        .get(0);
    let registered_reads: serde_json::Value = serde_json::from_slice(&registered_reads).unwrap();
    let predicate_keys = registered_reads
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r[0] == "generation")
        .map(|r| {
            (
                r[1].as_str().unwrap().to_owned(),
                r[2].as_str().unwrap().to_owned(),
                r[3].as_str().unwrap().to_owned(),
                r[4].as_str().unwrap().to_owned(),
                r[5].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert!(predicate_keys.windows(2).all(|p| p[0] < p[1]));
    for scope in ["source-inventory", "source-home", "metadata", "form"] {
        assert!(predicate_keys.iter().any(|p| p.2 == scope));
    }
    let starting_head = lab.head_seq();
    let starting_counts = [
        lab.count("current"),
        lab.count("history"),
        lab.count("receipt"),
        lab.count("log"),
        lab.count("outbox"),
    ];
    for (kind, owner, scope, token, definition) in predicate_keys
        .iter()
        .filter(|p| p.2 == "source-home" || p.2 == "form")
    {
        assert_eq!(predicate_sql.execute(
            "UPDATE cmd2_predicate SET generation=generation+1 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
            &[&lab.domain, kind, owner, scope, token],
        ).unwrap(), 1);
        assert!(matches!(
            lab.db.commit_source_creation(
                &lab.store,
                &attempts[0],
                &packages[0],
                &owners[0],
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled,
            ),
            Err(DurableError::Conflict("affected source predicate changed"))
        ));
        predicate_sql.execute(
            "UPDATE cmd2_predicate SET generation=generation-1,complete=false WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
            &[&lab.domain, kind, owner, scope, token],
        ).unwrap();
        assert!(matches!(
            lab.db.commit_source_creation(
                &lab.store,
                &attempts[0],
                &packages[0],
                &owners[0],
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled,
            ),
            Err(DurableError::Refused(
                "source predicate definition/completeness invalidated"
            ))
        ));
        predicate_sql.execute(
            "UPDATE cmd2_predicate SET complete=true,definition_version=$6 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
            &[&lab.domain, kind, owner, scope, token, &Digest256::of_bytes(b"changed owner definition").to_hex()],
        ).unwrap();
        assert!(matches!(
            lab.db.commit_source_creation(
                &lab.store,
                &attempts[0],
                &packages[0],
                &owners[0],
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled,
            ),
            Err(DurableError::Refused(
                "source predicate definition/completeness invalidated"
            ))
        ));
        predicate_sql.execute(
            "UPDATE cmd2_predicate SET definition_version=$6 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
            &[&lab.domain, kind, owner, scope, token, definition],
        ).unwrap();
        assert_eq!(lab.head_seq(), starting_head);
        assert_eq!(
            [
                lab.count("current"),
                lab.count("history"),
                lab.count("receipt"),
                lab.count("log"),
                lab.count("outbox")
            ],
            starting_counts
        );
    }
    let (a, _) = lab
        .db
        .commit_source_creation(
            &lab.store,
            &attempts[0],
            &packages[0],
            &owners[0],
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            deadline,
            &cancelled,
        )
        .unwrap();
    assert!(
        matches!(
            lab.db.commit_source_creation(
                &lab.store,
                &attempts[1],
                &packages[1],
                &owners[1],
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled
            ),
            Err(DurableError::Conflict(_))
        ),
        "maintained whole inventory insertion invalidates another original proposal"
    );
    assert_eq!(a.commit_seq, starting_head + 1);
    assert!(
        matches!(
            lab.db.commit_source_creation(
                &lab.store,
                &attempts[2],
                &packages[2],
                &owners[2],
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled
            ),
            Err(DurableError::Conflict(_))
        ),
        "current owner inventory and form phantom are fenced in durable commit"
    );
    assert_eq!(lab.head_seq(), a.commit_seq);
    assert!(
        matches!(
            lab.db.reopen_committed_source_creation_attempt(
                &lab.store,
                &initial_cohort,
                b"agent-1",
                &packages[1],
                &mut new_worker(&cut),
                deadline,
                &cancelled
            ),
            Err(DurableError::Refused(_))
        ),
        "cold replay seam cannot grant a pending attempt a new write"
    );
    let (replay, _) = lab
        .db
        .commit_source_creation(
            &lab.store,
            &attempts[0],
            &packages[0],
            &owners[0],
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            deadline,
            &cancelled,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.commit_seq, a.commit_seq);
    let copied = ScratchRoot::new();
    copy_store_tree(&lab._root.0, &copied.0);
    let reopened_store = SegmentStore::open_existing(&copied.0, limits()).unwrap();
    let mut reopened_db = DurablePgCoordinator::connect(&url).unwrap();
    let reopened = reopened_db
        .cold_reopen_source_cohort(
            &reopened_store,
            &lab.domain,
            &cut,
            revision,
            membership,
            &contexts[0],
            &software,
            &components,
            &mut new_worker(&cut),
            deadline,
            &cancelled,
        )
        .unwrap();
    for package in &packages[..1] {
        for (name, raw) in package.prepared().files() {
            let path = format!("{}/{name}", package.prepared().home().as_str());
            assert_eq!(&reopened.files[&path], raw);
            let member = reopened_db
                .read_current_source_member(
                    &reopened_store,
                    &reopened.cohort,
                    &RelativePath::parse(&path).unwrap(),
                    8_388_608,
                    deadline,
                    &cancelled,
                )
                .unwrap();
            assert_eq!(&member.raw, raw);
            assert_eq!(member.current_generation, a.commit_seq);
            assert_eq!(member.metadata.mode, 0o644);
            assert_eq!(reopened.metadata[&path], member.metadata);
            assert_eq!(reopened.dependency_claims[&path], member.dependency_claims);
        }
    }
    assert!(
        reopened
            .indexes
            .iter()
            .any(|(kind, id, _)| kind == "metadata" && id == "tos.agent.synthetic-durable-a")
    );
    assert!(
        reopened
            .indexes
            .iter()
            .any(|(kind, id, _)| kind == "form" && id == "tos.form.synthetic-durable-a")
    );
    assert_eq!(
        reopened.cohort.initial_revision(),
        revision,
        "original bootstrap is not renamed current"
    );
    assert_eq!(
        reopened.current_membership.count,
        membership.count
            + packages[..1]
                .iter()
                .map(|p| p.prepared().files().len() as u64)
                .sum::<u64>()
    );

    let mut sql = Client::connect(&url, NoTls).unwrap();
    sql.execute(
        "UPDATE cmd2_audit_fence SET maintenance_state='active' WHERE domain=$1",
        &[&lab.domain],
    )
    .unwrap();
    assert!(
        reopened_db
            .read_current_source_member(
                &reopened_store,
                &reopened.cohort,
                &RelativePath::parse("ToS/source-witnesses/agents/synthetic-durable-a/agent.json")
                    .unwrap(),
                8_388_608,
                deadline,
                &cancelled
            )
            .is_err()
    );
    sql.execute(
        "UPDATE cmd2_audit_fence SET maintenance_state='normal' WHERE domain=$1",
        &[&lab.domain],
    )
    .unwrap();
    assert!(
        lab.db
            .register_source_creation(
                &lab.store,
                &initial_cohort,
                b"stale-epoch",
                &packages[2],
                &mut new_worker(&cut),
                deadline,
                &cancelled
            )
            .is_err()
    );
    let verified = reopened_db
        .cold_reopen_source_cohort(
            &reopened_store,
            &lab.domain,
            &cut,
            revision,
            membership,
            &contexts[0],
            &software,
            &components,
            &mut new_worker(&cut),
            deadline,
            &cancelled,
        )
        .unwrap();
    assert!(verified.cohort.epoch() > initial_cohort.epoch());
    let mut replay_attempt_worker = new_worker(&cut);
    let replay_attempt = reopened_db
        .reopen_committed_source_creation_attempt(
            &reopened_store,
            &verified.cohort,
            b"agent-0",
            &packages[0],
            &mut replay_attempt_worker,
            deadline,
            &cancelled,
        )
        .unwrap();
    replay_attempt_worker.finish(deadline, &cancelled).unwrap();
    drop(replay_attempt_worker);
    let (restored_replay, _) = reopened_db
        .commit_source_creation(
            &reopened_store,
            &replay_attempt,
            &packages[0],
            &owners[0],
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            deadline,
            &cancelled,
        )
        .unwrap();
    assert!(restored_replay.replayed);
    assert_eq!(restored_replay.commit_seq, a.commit_seq);
    // A genuine complete v1 export establishes the initial full producer
    // correspondence once. Warm successors do not re-export authored bodies.
    let generation_scratch = ScratchRoot::new();
    let generation_workspace = PrivateGenerationWorkspace::open(
        &generation_scratch.0,
        ColdWorkspaceLimits {
            max_scratch_written_bytes: 64 * 1024 * 1024,
            max_run_bytes: 16 * 1024,
            max_runs: 128,
            merge_fan_in: 4,
            max_rows: 4096,
            max_key_bytes: 4096,
        },
    )
    .unwrap();
    let export = tos_command::source_current_cut::select_current_source_cut(
        &mut reopened_db,
        &reopened_store,
        &lab.domain,
        &cut,
        revision,
        membership,
        &contexts[0],
        &software,
        &components,
        &mut new_worker(&cut),
        Some((&generation_workspace, generation_profile)),
        &isolated,
        None,
        read_limits,
        CutReadLimits {
            max_revisions: 4,
            max_members: 2048,
            max_total_bytes: 33_554_432,
            max_member_bytes: 8_388_608,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let export = if addressed {
        tos_command::source_current_cut::migrate_current_source_cut_addressed(
            &mut reopened_db,
            &reopened_store,
            export,
            tree_limits,
            deadline,
            &cancelled,
        )
        .unwrap()
    } else {
        export
    };
    let mut descriptor: serde_json::Value = serde_json::from_slice(
        &fs::read(
            repository.join("rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json"),
        )
        .unwrap(),
    )
    .unwrap();
    descriptor["descriptor_id"] = serde_json::json!("synthetic.private.managed-agent-query");
    descriptor["owner_ref"] = serde_json::json!("synthetic-test-only:no-production-read-authority");
    descriptor["purpose"] = serde_json::json!(
        "Private Agent-only selected-consumer conformance; no production read, source, rights or semantic admission"
    );
    descriptor["sources"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s["source_graph_id"] == "source-navigation");
    assert_eq!(descriptor["sources"].as_array().unwrap().len(), 1);
    let descriptor_raw = canonical(&descriptor);
    let descriptor_digest = Digest256::of_bytes(&descriptor_raw);
    fs::write(
        root.0.join("private-agent-descriptor.json"),
        &descriptor_raw,
    )
    .unwrap();
    let vocabulary = tos_compiler::QueryVocabulary::parse(
        &descriptor_raw,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
    )
    .unwrap();
    let processor_path =
        RelativePath::parse("rust/crates/tos-compiler/src/knowledge_normalization.rs").unwrap();
    let processor_raw = software
        .read_selected_component(
            &components,
            &processor_path,
            8_388_608,
            deadline,
            &cancelled,
        )
        .unwrap();
    let processor_digest = Digest256::of_bytes(&processor_raw);
    assert_eq!(processor_raw, files[processor_path.as_str()]);
    // Actual selected source-component/configuration pins are evidence for this
    // declared fixture profile, not an ELF/compiler closure or Python identity.
    let selected_stage_limits = tos_compiler::knowledge_stage::StageLimits {
        sqlite: tos_compiler::Limits {
            max_rows: 8192,
            max_row_bytes: 2 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_work_bytes: 128 * 1024 * 1024,
            sqlite_cache_kib: 512,
            max_sql_vm_steps: 20_000_000,
        },
        max_temp_bytes: 64 * 1024 * 1024,
        max_seek_rows: 16,
        max_seek_bytes: 32 * 1024 * 1024,
    };
    let native_limits = tos_compiler::knowledge_full_fixture::native_fixture_limits(100, 200);
    let full_limits = tos_compiler::knowledge_full_fixture::native_fixture_full_limits(1);
    let process_limits =
        tos_compiler::knowledge_full_fixture::NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS;
    let cold_limits =
        tos_compiler::knowledge_full_fixture::native_fixture_cold_limits(full_limits, 1);
    let original_limits = tos_compiler::NavigationOriginalLimits {
        max_rows: 200,
        max_row_bytes: 65536,
        max_total_bytes: 262144,
    };
    let bibliographic_limits = tos_compiler::source_bibliographic::BibliographicLimits {
        catalog: tos_compiler::source_witness_catalog::SourceCatalogLimits {
            max_files: 512,
            max_rows: 4096,
            max_file_bytes: 2 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_contract_bytes: 16 * 1024 * 1024,
            max_output_row_bytes: 1024 * 1024,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16 * 1024 * 1024,
        max_output_rows: 4096,
        max_output_bytes: 16 * 1024 * 1024,
        deadline,
    };
    let selected_owner = ManagedFixtureStageOwner;
    let selected_isolation = ManagedFixtureStageIsolation;
    let entity_raw = &files["ToS/doctrine/semantic-interchange/entity-types.v1.json"];
    let relation_raw = &files["ToS/doctrine/semantic-interchange/relation-types.v1.json"];
    let entities: serde_json::Value = serde_json::from_slice(entity_raw).unwrap();
    let binding_for = |proof: &P| {
        let installed = match proof.basis() {
            tos_compiler::KnowledgeSourceBasis::ManagedCurrent { proof } => {
                proof.generation.installed_generation_sha256
            }
            tos_compiler::KnowledgeSourceBasis::ManagedCurrentV2 { proof } => {
                proof.generation.installed_generation_sha256
            }
            _ => unreachable!(),
        };
        tos_compiler::SourceBinding {
            owner_profile: "synthetic-private-managed-agent-consumer".into(),
            source_cut: proof.initial_export().0.into(),
            through_commit_seq: proof.through_commit_seq(),
            membership_root: proof.current_root().into(),
            index_generation: installed,
            route_map_version: "private-managed-agent-v1".into(),
            reader_abi: "private-managed-agent-v1".into(),
            projection_root_sha256: proof.inventory_projection().into(),
            complete: true,
        }
    };
    let validator_for = |schema_cut: &CorpusCutReader| {
        let mut operation = tos_validation::executor::BatchStreamBudget::laboratory();
        operation.max_chunks = bibliographic_limits.max_output_rows;
        operation.max_total_units = bibliographic_limits.max_output_rows;
        operation.total_execution_wall = deadline.saturating_duration_since(Instant::now());
        operation.operation_cpu_seconds =
            operation.total_execution_wall.as_secs().saturating_add(1);
        operation.operation_address_space_bytes = ExecutorBudget::laboratory().address_space_bytes;
        tos_compiler::source_witness_catalog::SourceCatalogValidator::from_cut(
            schema_cut,
            &worker_identity,
            ExecutorBudget::laboratory(),
            CutWorkerLimits {
                max_receipts: 4096,
                max_receipt_bytes: 16 * 1024 * 1024,
            },
            operation,
            deadline,
            &cancelled,
        )
    };
    let overlay = addressed && std::env::var_os("TOS_CMD2_MODEL_OVERLAY_V2").is_some();
    let mut catalog_epoch = None;
    let mut initial_carriers = Vec::new();
    let mut cold_render_work = tos_compiler::SourceCatalogRenderWorkV1::default();
    let mut cold_source_work = tos_command::ManagedSourceWorkV1::default();
    let initial_selected_result =
        tos_command::source_managed_selection::prepare_managed_agent_selected_parent_with_work::<P>(
            &mut reopened_db,
            &reopened_store,
            export,
            &owners[0],
            &packages[0],
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            process_limits,
            cold_limits,
            deadline,
            &cancelled,
            &mut cold_source_work,
            |export, proof| {
                let selected_cut = export.cut();
                let selected_revision = selected_cut.current().revision();
                let selected_membership = selected_cut
                    .stream(selected_revision)
                    .map_err(|_| tos_compiler::Error::Invalid("fixture selected export stream"))?
                    .expectation();
                let mut binding = binding_for(&proof);
                binding.membership_root = selected_membership.digest.to_hex();
                binding.index_generation = selected_revision.0.to_hex();
                let plan = tos_compiler::plan_source_catalog_inputs(
                    selected_cut,
                    selected_revision,
                    selected_membership,
                    &binding,
                    tos_compiler::SourceCatalogInputLimits {
                        max_manifest_members: 512,
                        max_selected_members: 512,
                        max_plan_bytes: 16 * 1024 * 1024,
                        max_work_bytes: 64 * 1024 * 1024,
                    },
                    bibliographic_limits,
                    &cancelled,
                )?;
                let mut stage = tos_compiler::knowledge_stage::KnowledgeStage::create(
                    &root.0.join("initial-agent-catalog.sqlite"),
                    selected_stage_limits,
                    plan.input_receipt(),
                    &selected_owner,
                    &selected_isolation,
                )?;
                let validator = validator_for(selected_cut)?;
                let mut forms = tos_command::source_forms_compiler::NativeBibliographicForms;
                eprintln!(
                    "CMD2_MODEL_V2_COLD_PLAN members={} observed_work_bytes={} manifest_members={}",
                    plan.selected_member_count(),
                    plan.observed_work_bytes(),
                    plan.source_membership().count
                );
                let candidate = tos_compiler::render_source_bibliographic_plan_with_work(
                    &plan,
                    selected_cut,
                    selected_revision,
                    selected_membership,
                    &mut stage,
                    &validator,
                    &mut forms,
                    bibliographic_limits,
                    512,
                    16 * 1024 * 1024,
                    &mut cold_render_work,
                )?;
                let source = tos_compiler::source_bibliographic::BibliographicSourceCut {
                    cut: selected_cut,
                    expected_revision: selected_revision,
                    expected_membership: selected_membership,
                    stage_source_cut: &binding.source_cut,
                    max_read_files: 512,
                    max_read_bytes: 16 * 1024 * 1024,
                };
                let navigation =
                    tos_compiler::source_navigation_source::project_source_navigation_from_cut(
                        &mut stage,
                        &candidate.catalog,
                        &source,
                        &validator,
                        &entities,
                        &mut forms,
                        bibliographic_limits,
                    )?;
                if overlay {
                    catalog_epoch = Some(tos_compiler::VersionsCatalogEpochV2::build_with_work(
                        &mut stage,
                        &candidate.catalog,
                        &reopened_store,
                        bibliographic_limits.catalog,
                        tree_limits,
                        deadline,
                        &cancelled,
                    )?);
                }
                let basis = proof.basis();
                tos_compiler::managed_source::prepare_managed_agent_selected_model(
                    &plan,
                    &navigation,
                    &validator,
                    proof,
                    &model_root.join("initial-agent-selected.sqlite"),
                    binding,
                    selected_stage_limits,
                    &selected_owner,
                    &selected_isolation,
                    entity_raw,
                    relation_raw,
                    &descriptor_raw,
                    tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
                    native_limits,
                    original_limits,
                    full_limits,
                    &[],
                    "synthetic-agent-initial-model".into(),
                    |stage, registry| {
                        initial_carriers =
                            tos_compiler::knowledge_full_fixture::native_fixture_produced_carriers(
                                stage,
                                selected_stage_limits,
                            )?;
                        tos_compiler::knowledge_full_fixture::native_fixture_header_with_binding(
                            stage,
                            registry,
                            entity_raw,
                            &basis,
                            processor_digest,
                            descriptor_digest,
                        )
                    },
                )
            },
        );
    eprintln!(
        "CMD2_MODEL_V2_COLD_PREPARATION result_ok={} source_work={cold_source_work:?} render_work={cold_render_work:?}",
        initial_selected_result.is_ok()
    );
    let initial_selected = initial_selected_result.unwrap();
    assert!(!initial_carriers.is_empty());
    drop(initial_carriers); // the next selected model has its own finite carrier set
    let manifest_limits = tos_compiler::ManagedManifestLimitsV2 {
        max_manifest_bytes: 1_048_576,
        max_retained_generations: 16,
        max_retained_logical_bound_bytes: 64 * 1024 * 1024,
    };
    let mut initial_selected = Some(initial_selected);
    let overlay_parent = if overlay {
        Client::connect(&url, NoTls)
            .unwrap()
            .batch_execute(include_str!("../src/managed_model_selection_v2.sql"))
            .unwrap();
        let (epoch, epoch_work) = catalog_epoch.as_ref().unwrap();
        let result =
            tos_command::source_managed_selection::prepare_managed_agent_overlay_parent_v2(
                &mut reopened_db,
                &reopened_store,
                initial_selected.take().unwrap(),
                epoch,
                &owners[0],
                &packages[0],
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                tree_limits,
                manifest_limits,
                deadline,
                &cancelled,
            )
            .unwrap();
        eprintln!(
            "CMD2_MODEL_V2_COLD source_work={cold_source_work:?} render_work={cold_render_work:?} catalog_records={} catalog_entries={} epoch_work={epoch_work:?} base_digest_read={} base_transport_read={} base_validation_charged={} base_transport_work={:?} manifest_work={:?} cas_work={:?}",
            result.1.catalog_records(),
            result.1.catalog_total_entries(),
            result.1.base_digest_read_bytes(),
            result.1.base_transport_read_bytes(),
            result.1.base_validation_charged_bytes(),
            result.1.base_transport_work(),
            result.1.manifest_work(),
            result.2
        );
        Some(result.0)
    } else {
        None
    };
    let current = if let Some(parent) = &overlay_parent {
        parent.generation()
    } else {
        initial_selected.as_ref().unwrap().generation()
    };
    assert_eq!(current.commit_seq(), a.commit_seq);
    let mut current_context = contexts[1].clone();
    current_context
        .files
        .retain(|f| !f.path.as_str().starts_with("ToS/"));
    let mut current_config: serde_json::Value =
        serde_json::from_slice(&current_context.configuration_raw).unwrap();
    current_config["source_path"] =
        serde_json::json!("ToS/source-witnesses/agents/synthetic-durable-current/agent.json");
    current_config["record_id"] = serde_json::json!("tos.agent.synthetic-durable-current");
    current_config["allowed_form_ids"] = serde_json::json!(["tos.form.synthetic-durable-current"]);
    current_config["provenance_event_id"] =
        serde_json::json!("tos.event.synthetic-durable-current");
    let current_owner = isolated.path().join("owner-current.json");
    fs::write(&current_owner, canonical(&current_config)).unwrap();
    fs::set_permissions(&current_owner, fs::Permissions::from_mode(0o600)).unwrap();
    let current_filesystem =
        CreationFilesystem::select_isolated(&isolated, &current_owner, deadline, &cancelled)
            .unwrap();
    current_context.configuration_raw = canonical(&current_config);
    let mut current_request: serde_json::Value =
        serde_json::from_slice(&contexts[1].request_raw).unwrap();
    for field in [
        "command_id",
        "expected_configuration",
        "expected_dependencies",
        "expected_source",
        "expected_revision",
    ] {
        current_request.as_object_mut().unwrap().remove(field);
    }
    current_request["operation"] = serde_json::json!("prepare-create");
    current_request["record"]["record_id"] =
        serde_json::json!("tos.agent.synthetic-durable-current");
    let earlier_path = "ToS/source-witnesses/agents/synthetic-durable-a/agent.json";
    current_request["record"]["source_refs"] = serde_json::json!([earlier_path]);
    current_request["forms"][0]["form_id"] =
        serde_json::json!("tos.form.synthetic-durable-current");
    current_context.request_raw = canonical(&current_request);
    let mut current_worker = new_worker(&cut);
    let input = tos_command::source_creation::select_managed_agent_creation_input(
        &mut reopened_db,
        &reopened_store,
        &current,
        &current_context,
        &cut,
        &software,
        &components,
        deadline,
        &cancelled,
    )
    .unwrap();
    let preview = tos_command::source_creation::prepare_managed_agent_creation(
        &input,
        &cut,
        &software,
        &components,
        &mut current_worker,
        deadline,
        &cancelled,
    )
    .unwrap()
    .preview()
    .unwrap();
    current_request["operation"] = serde_json::json!("source.create");
    current_request["command_id"] = serde_json::json!("synthetic:durable-current-second");
    for (key, field) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_dependencies", "expected_dependencies"),
    ] {
        current_request[key] =
            serde_json::json!(preview.object_get(field).unwrap().as_str().unwrap());
    }
    current_request["expected_source"] = serde_json::Value::Null;
    current_request["expected_revision"] = serde_json::Value::Null;
    current_context.request_raw = canonical(&current_request);
    let mut warm_source_work = tos_command::ManagedSourceWorkV1::default();
    let successor_result = reopened_db.execute_managed_agent_creation_from_captures_with_work(
        &reopened_store,
        &current,
        &cut,
        b"agent-current-second",
        &current_filesystem,
        &current_context,
        &software,
        &components,
        &mut current_worker,
        contract_digest(),
        0,
        0,
        "private-job",
        1,
        deadline,
        &cancelled,
        &mut warm_source_work,
    );
    eprintln!(
        "CMD2_SOURCE_V1_WARM_COMMIT result_ok={} work={warm_source_work:?}",
        successor_result.is_ok()
    );
    let (current_package, second_receipt, _, successor) = successor_result.unwrap();
    drop(current_worker);
    assert_eq!(second_receipt.commit_seq, a.commit_seq + 1);
    let addressed_members = current_package
        .reads()
        .iter()
        .filter(|read| read.path.as_str().starts_with("ToS/"))
        .count();
    assert!(
        addressed_members < reopened.files.len(),
        "ordinary managed preparation does not read every current body"
    );
    assert!(
        !current_package.reads().iter().any(|read| read.path.as_str()
            == "ToS/source-witnesses/agents/synthetic-durable-a/agent.human-forms.json"),
        "prior adjacent-form contribution is consumed from exact projection, not its raw body"
    );
    assert!(
        current
            .read_current_member(
                &mut reopened_db,
                &reopened_store,
                &RelativePath::parse(earlier_path).unwrap(),
                8_388_608,
                deadline,
                &cancelled
            )
            .is_err(),
        "a stale selected generation cannot disclose unchanged current bytes"
    );
    assert!(
        current_package
            .reads()
            .iter()
            .any(|r| r.path.as_str() == earlier_path
                && r.raw_sha256 == Digest256::of_bytes(&reopened.files[earlier_path])),
        "second maintained command consumes exact first authored body"
    );
    assert_ne!(successor.digest(), current.digest());
    assert_eq!(successor.commit_seq(), second_receipt.commit_seq);
    let second_path =
        RelativePath::parse("ToS/source-witnesses/agents/synthetic-durable-current/agent.json")
            .unwrap();
    let successor_member = successor
        .read_current_member(
            &mut reopened_db,
            &reopened_store,
            &second_path,
            8_388_608,
            deadline,
            &cancelled,
        )
        .unwrap();
    assert!(
        successor_member
            .dependency_claims
            .as_ref()
            .unwrap()
            .iter()
            .any(|path| path.as_str() == earlier_path)
    );
    if let Some(parent) = &overlay_parent {
        use tos_command::backup_recovery::{
            BackupSelection, PgTool, RestoreSelection, backup_quiescent,
            restore_into_fresh_with_managed_binding,
        };
        // The genuine current owner/package must not grant an old manifest
        // current disclosure after the committed source generation advanced.
        let mut stale_seek_called = false;
        let stale_read = parent.with_current_model(
            &mut reopened_db,
            &reopened_store,
            &current_filesystem,
            &current_package,
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            manifest_limits,
            deadline,
            &cancelled,
            |_| {
                stale_seek_called = true;
                Ok(())
            },
        );
        assert!(stale_read.is_err(), "stale overlay parent is not current");
        assert!(
            !stale_seek_called,
            "stale overlay cannot enter reader callback"
        );
        eprintln!("CMD2_MODEL_V2_STALE_PARENT_REFUSAL PASS");
        let validator = validator_for(&cut).unwrap();
        let overlay_audited = reopened_store.hold_audit_root().unwrap();
        let overlay_result =
            tos_command::source_managed_selection::prepare_managed_agent_overlay_successor_v2_with_work(
                &mut reopened_db,
                &reopened_store,
                parent,
                successor,
                b"agent-current-second",
                &current_package,
                &current_filesystem,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                manifest_limits,
                deadline,
                &cancelled,
                &mut warm_source_work,
                |previous,
                 digest,
                 base,
                 source,
                 delta,
                 changed,
                 projection,
                 path,
                 raw,
                 forms_raw| {
                    let mut forms = tos_command::source_forms_compiler::NativeBibliographicForms;
                    tos_compiler::prepare_managed_overlay_initial_agent_v2(
                        previous,
                        digest,
                        base,
                        source,
                        delta,
                        changed,
                        projection,
                        path,
                        raw,
                        forms_raw,
                        &mut forms,
                        &validator,
                        revision,
                        entity_raw,
                        bibliographic_limits,
                        selected_stage_limits,
                        &reopened_store,
                        &overlay_audited,
                        tree_limits,
                        manifest_limits,
                        deadline,
                        &cancelled,
                    )
                },
            );
        eprintln!(
            "CMD2_MODEL_V2_SUCCESSOR result_ok={} source_work={warm_source_work:?}",
            overlay_result.is_ok()
        );
        let (selected, prepared, cas_work) = overlay_result.unwrap();
        assert_eq!(warm_source_work.addressed_continuations_attempted, 1);
        assert_eq!(warm_source_work.legacy_continuations_attempted, 0);
        assert_eq!(warm_source_work.projection_rows_returned, 0);
        assert_eq!(warm_source_work.catalogue_entries_delivered, 0);
        assert!(warm_source_work.audit.interval_rows_returned > 0);
        assert_eq!(current_package.files().len(), 6);
        assert_eq!(prepared.changed_source_members, 6);
        assert_eq!(warm_source_work.point_projection_rows_returned, 6);
        assert_eq!(
            prepared.source_input_bytes_consumed,
            current_package
                .files()
                .values()
                .map(|raw| raw.len() as u64)
                .sum::<u64>()
        );
        eprintln!(
            "CMD2_MODEL_V2_COST_SCOPE PG=returned-decoded-logical-payload-only PGphysical=unobserved PGprotocol=unobserved gates=excluded STO=completed-receipts-including-active-buffer cold=full-initial-and-recovery warm=source-delta-overlay-installed-reader"
        );
        let (old_entry, old_work) = catalog_epoch
            .as_ref()
            .unwrap()
            .0
            .lookup_with_work(
                "records",
                "tos.agent.synthetic-durable-a",
                tree_limits,
                deadline,
                &cancelled,
            )
            .unwrap();
        let ((new_entry, unchanged, navigation, read_work), guard_work) = selected
            .with_current_model(
                &mut reopened_db,
                &reopened_store,
                &current_filesystem,
                &current_package,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                manifest_limits,
                deadline,
                &cancelled,
                |reader| {
                    let (new_entry, new_work) = reader
                        .catalog_member(
                            "records",
                            "tos.agent.synthetic-durable-current",
                            tree_limits,
                            deadline,
                            &cancelled,
                        )
                        .map_err(
                            tos_command::source_managed_selection::ManagedSelectionError::Compiler,
                        )?;
                    let (unchanged, old_work) = reader
                        .catalog_member(
                            "records",
                            "tos.agent.synthetic-durable-a",
                            tree_limits,
                            deadline,
                            &cancelled,
                        )
                        .map_err(
                            tos_command::source_managed_selection::ManagedSelectionError::Compiler,
                        )?;
                    let (navigation, nav_work) = reader
                        .navigation_member(
                            "nodes",
                            "tos.agent.synthetic-durable-current",
                            &validator,
                            bibliographic_limits,
                            selected_stage_limits,
                            tree_limits,
                            deadline,
                            &cancelled,
                        )
                        .map_err(
                            tos_command::source_managed_selection::ManagedSelectionError::Compiler,
                        )?;
                    Ok((
                        new_entry,
                        unchanged,
                        navigation,
                        (new_work, old_work, nav_work),
                    ))
                },
            )
            .unwrap();
        assert!(new_entry.is_some());
        assert_eq!(
            unchanged, old_entry,
            "unchanged selected Versions entry/provenance must remain exact"
        );
        assert!(
            navigation.is_some(),
            "actual addressed navigation reader reaches changed Agent"
        );
        eprintln!(
            "CMD2_MODEL_V2_WARM source_work={warm_source_work:?} source_members={} source_history={} changed_record={:?} changed_forms={:?} overlay_work={:?} cas_work={cas_work:?} old_epoch_read={old_work:?} reads={read_work:?} final_guards={guard_work:?}",
            selected.source_binding().current_source_members,
            selected.source_binding().historical_source_members,
            prepared.created_record_input(),
            prepared.created_forms_input(),
            (
                prepared.catalog_work,
                prepared.navigation_work,
                prepared.manifest_work,
                prepared.base_candidate_rows,
                prepared.base_charged_bytes,
                prepared.changed_source_members,
                prepared.changed_catalog_records,
                prepared.source_input_bytes_consumed
            )
        );
        let version_ref =
            navigation.as_ref().unwrap()["properties"]["record_history"]["current_ref"].clone();
        assert!(version_ref.is_object());
        let ((version_node, version_work), version_guards) =
            selected
                .with_current_model(
                    &mut reopened_db,
                    &reopened_store,
                    &current_filesystem,
                    &current_package,
                    contract_digest(),
                    0,
                    0,
                    "private-job",
                    1,
                    manifest_limits,
                    deadline,
                    &cancelled,
                    |reader| {
                        reader.record_version_member(&version_ref, &validator, bibliographic_limits,
                selected_stage_limits, tree_limits, deadline, &cancelled)
                .map_err(tos_command::source_managed_selection::ManagedSelectionError::Compiler)
                    },
                )
                .unwrap();
        let version = version_node
            .as_ref()
            .expect("genuine Versions response reached");
        assert_eq!(
            version["properties"]["record_version_view"]["record"],
            serde_json::from_slice::<serde_json::Value>(&current_package.files()["agent.json"])
                .unwrap()
        );
        eprintln!("CMD2_MODEL_V2_VERSIONS read={version_work:?} guards={version_guards:?}");
        // Keep the real opaque restore witness in this same existing whole
        // operation. JSON from a transport subprocess cannot grant rebind.
        let restored_url = std::env::var("TOS_CMD2_V2_RESTORE_PG_URL").unwrap();
        assert_ne!(url, restored_url);
        let backup_root = ScratchRoot::new();
        let recovered_root = ScratchRoot::new();
        for directory in [&backup_root.0, &recovered_root.0, &copied.0] {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let dump_path = PathBuf::from(std::env::var_os("TOS_CMD2_PG_DUMP_PATH").unwrap());
        let dump_sha = std::env::var("TOS_CMD2_PG_DUMP_SHA256").unwrap();
        let restore_path = PathBuf::from(std::env::var_os("TOS_CMD2_PG_RESTORE_PATH").unwrap());
        let restore_sha = std::env::var("TOS_CMD2_PG_RESTORE_SHA256").unwrap();
        let backup = backup_quiescent(
            &BackupSelection {
                pg_url: &url,
                domain: &lab.domain,
                store_root: &copied.0,
                backup_root: &backup_root.0,
                tool: PgTool {
                    path: &dump_path,
                    sha256: &dump_sha,
                },
                store_limits: limits(),
                quiescent_owner_confirmed: true,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
        let receipt_sha =
            Digest256::of_bytes(&fs::read(backup_root.0.join("receipt.json")).unwrap()).to_hex();
        let old_digest = selected.manifest_digest();
        let old_binding = selected.source_binding().clone();
        drop(selected);
        drop(prepared);
        drop(overlay_parent);
        drop(catalog_epoch);
        drop(overlay_audited);
        drop(reopened_db);
        drop(reopened_store);
        let (restore_receipt, witness) = restore_into_fresh_with_managed_binding(
            &RestoreSelection {
                pg_url: &restored_url,
                domain: &lab.domain,
                store_root: &recovered_root.0,
                backup_root: &backup_root.0,
                receipt_sha256: &receipt_sha,
                tool: PgTool {
                    path: &restore_path,
                    sha256: &restore_sha,
                },
                store_limits: limits(),
                fresh_target_owner_confirmed: true,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
        let witness =
            witness.expect("recognized actual model restore must issue its opaque witness");
        let recovered_store = SegmentStore::open_existing(&recovered_root.0, limits()).unwrap();
        let mut recovered_db = DurablePgCoordinator::connect(&restored_url).unwrap();
        let current = if std::env::var_os("TOS_CMD2_STREAMED_COLD_V1").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        {
            let mut work = tos_command::source_cohort::StreamedColdSourceWorkV1::default();
            let source_reader = CorpusReader::open_existing(&source_root, read_limits).unwrap();
            let streamed_original_result = generation_workspace.open_streamed_source_cut(
                &source_reader,
                revision,
                tos_source_store::StreamedCutReadLimitsV1 {
                    manifest_json: JsonLimits {
                        max_bytes: 4_194_304,
                        ..JsonLimits::default()
                    },
                    max_manifest_entries: 2048,
                    cut: CutReadLimits {
                        max_revisions: 4,
                        max_members: 2048,
                        max_total_bytes: 33_554_432,
                        max_member_bytes: 8_388_608,
                    },
                    max_index_bytes: 16 * 1024 * 1024,
                    max_manifest_row_bytes: 1_048_576,
                    sqlite_cache_bytes: 1_048_576,
                },
                deadline,
                &cancelled,
                &mut work,
            );
            println!(
                "streamed original cut open: result={:?}; work={work:?}",
                streamed_original_result.as_ref().map(|_| ())
            );
            let streamed_original = streamed_original_result.unwrap();
            assert_eq!(
                streamed_original
                    .revision_at(0)
                    .unwrap()
                    .unwrap()
                    .membership,
                membership
            );
            let image = tos_validation::executor::VerifiedWorkerImageHandle::prepare(
                worker_identity.clone(),
                ExecutorBudget::laboratory(),
                deadline,
                &cancelled,
            )
            .unwrap();
            let mut worker = CutWorkerSchemaExecutor::from_streamed_cut_with_image(
                &streamed_original,
                tos_validation::FormatProfile::LegacyPythonObserved20260923,
                &image,
                ExecutorBudget::laboratory(),
                CutWorkerLimits {
                    max_receipts: 128,
                    max_receipt_bytes: 262_144,
                },
                deadline,
                &cancelled,
            )
            .unwrap();
            let mut tree_work = tos_segment_store::AuthenticatedTreeWorkV1::default();
            let result = recovered_db.select_current_source_generation_addressed_streaming(
                &recovered_store,
                &lab.domain,
                &streamed_original,
                revision,
                membership,
                &contexts[0],
                &software,
                &components,
                &mut worker,
                "ToS/contracts/corpus-record.schema.json",
                &generation_workspace,
                generation_profile,
                tos_command::source_cohort::StreamedColdSourceLimitsV1 {
                    max_rows: 100_000,
                    max_logical_bytes: 64 * 1024 * 1024,
                    max_key_bytes: 4096,
                    max_value_bytes: 1_048_576,
                    max_placement_bytes: 1_048_576,
                    max_sqlite_file_bytes: 16 * 1024 * 1024,
                    max_vm_steps: 200_000_000,
                },
                tos_validation::item_rules::ItemLimits {
                    max_member_bytes: 8_388_608,
                    max_total_bytes: 64 * 1024 * 1024,
                    max_state_bytes: 16 * 1024 * 1024,
                    max_issues: 256,
                    deadline,
                },
                tree_limits,
                &mut work,
                &mut tree_work,
                deadline,
                std::sync::Arc::clone(&cancelled),
            );
            eprintln!(
                "CMD2_STREAMED_COLD result_ok={} source_work={work:?} tree_work={tree_work:?}",
                result.is_ok()
            );
            result.unwrap()
        } else {
            let current = tos_command::source_current_cut::select_current_source_generation(
                &mut recovered_db,
                &recovered_store,
                &lab.domain,
                &cut,
                revision,
                membership,
                &contexts[0],
                &software,
                &components,
                &mut new_worker(&cut),
                Some((&generation_workspace, generation_profile)),
                deadline,
                &cancelled,
            )
            .unwrap();
            let current = recovered_db
                .migrate_current_source_generation_addressed(
                    &recovered_store,
                    &current,
                    tree_limits,
                    deadline,
                    &cancelled,
                )
                .unwrap();
            current
        };
        let audited = recovered_store.hold_audit_root().unwrap();
        let (cold, cold_work) = tos_compiler::ManagedManifestV2::read_retained_cold(
            &recovered_store,
            &audited,
            &[old_digest],
            manifest_limits,
            tree_limits,
            deadline,
            &cancelled,
        )
        .unwrap();
        let manifest = &cold[0].0;
        let restored_base = model_root.join("restored-overlay-base.sqlite");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .mode(0o600)
            .open(&restored_base)
            .unwrap();
        let (stage_receipt, transport_work) = manifest
            .restore_base_to_empty_file(
                &recovered_store,
                &audited,
                &mut file,
                manifest_limits,
                deadline,
                &cancelled,
            )
            .unwrap();
        drop(file);
        let measurement =
            tos_compiler::prepare_native_knowledge_artifact(&restored_base, &stage_receipt)
                .unwrap();
        let custody = tos_compiler::LinuxFsVerityCustody::new(measurement, process_limits).unwrap();
        let base = tos_compiler::open_selected_knowledge_model_owned(
            &restored_base,
            manifest.base_selection().clone(),
            std::sync::Arc::new(custody),
            cold_limits,
        )
        .unwrap();
        let mut recovery_audit_work = tos_command::AuditDeltaWork::default();
        let recovery_result =
            tos_command::source_managed_selection::rebind_restored_managed_agent_overlay_v2_with_work(
                &mut recovered_db,
                &recovered_store,
                current,
                base,
                old_digest,
                &witness,
                &current_filesystem,
                &current_package,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                manifest_limits,
                deadline,
                &cancelled,
                &mut recovery_audit_work,
            );
        eprintln!(
            "CMD2_MODEL_V2_REBIND result_ok={} audit_work={recovery_audit_work:?}",
            recovery_result.is_ok()
        );
        let (rebound, recovery, recovery_guards) = recovery_result.unwrap();
        assert_ne!(
            rebound.source_binding().database_oid,
            old_binding.database_oid
        );
        assert_eq!(
            rebound.source_binding().through_commit_seq,
            old_binding.through_commit_seq
        );
        let ((restored_entry, restored_nav, cold_lookup_work), final_guards) = rebound
            .with_current_model(
                &mut recovered_db,
                &recovered_store,
                &current_filesystem,
                &current_package,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                manifest_limits,
                deadline,
                &cancelled,
                |reader| {
                    let (entry, entry_work) = reader
                        .catalog_member(
                            "records",
                            "tos.agent.synthetic-durable-current",
                            tree_limits,
                            deadline,
                            &cancelled,
                        )
                        .map_err(
                            tos_command::source_managed_selection::ManagedSelectionError::Compiler,
                        )?;
                    let (navigation, navigation_work) = reader
                        .navigation_member(
                            "nodes",
                            "tos.agent.synthetic-durable-current",
                            &validator,
                            bibliographic_limits,
                            selected_stage_limits,
                            tree_limits,
                            deadline,
                            &cancelled,
                        )
                        .map_err(
                            tos_command::source_managed_selection::ManagedSelectionError::Compiler,
                        )?;
                    Ok((entry, navigation, (entry_work, navigation_work)))
                },
            )
            .unwrap();
        assert_eq!(restored_entry, new_entry);
        assert_eq!(restored_nav, navigation);
        let ((restored_version, version_cold_work), version_cold_guards) =
            rebound
                .with_current_model(
                    &mut recovered_db,
                    &recovered_store,
                    &current_filesystem,
                    &current_package,
                    contract_digest(),
                    0,
                    0,
                    "private-job",
                    1,
                    manifest_limits,
                    deadline,
                    &cancelled,
                    |reader| {
                        reader.record_version_member(&version_ref, &validator, bibliographic_limits,
                selected_stage_limits, tree_limits, deadline, &cancelled)
                .map_err(tos_command::source_managed_selection::ManagedSelectionError::Compiler)
                    },
                )
                .unwrap();
        assert_eq!(
            restored_version, version_node,
            "cold current Versions response/provenance exact"
        );
        eprintln!(
            "CMD2_MODEL_V2_RESTORED_VERSIONS read={version_cold_work:?} guards={version_cold_guards:?}"
        );
        eprintln!(
            "CMD2_MODEL_V2_RESTORE audit_work={recovery_audit_work:?} lookup_work={cold_lookup_work:?} backup={backup} restore={restore_receipt} cold_work={cold_work:?} transport_work={transport_work:?} recovery_manifest_work={:?} recovery_guards={recovery_guards:?} final_guards={final_guards:?}",
            recovery.manifest_work()
        );
        return;
    }
    let mut successor_carriers = Vec::new();
    let second_selected =
        tos_command::source_managed_selection::prepare_managed_agent_selected_successor(
            &mut reopened_db,
            &reopened_store,
            initial_selected.as_ref().unwrap(),
            successor,
            b"agent-current-second",
            &current_package,
            &current_filesystem,
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            process_limits,
            cold_limits,
            deadline,
            &cancelled,
            |old, parent, proof, catalogue, record_path, record_raw, forms_raw| {
                let binding = binding_for(&proof);
                let basis = proof.basis();
                let validator = validator_for(&cut)?;
                let mut forms = tos_command::source_forms_compiler::NativeBibliographicForms;
                tos_compiler::managed_source::prepare_managed_agent_selected_successor(
                    old,
                    parent,
                    proof,
                    |accept| catalogue(accept),
                    record_path,
                    record_raw,
                    forms_raw,
                    &mut forms,
                    &validator,
                    revision,
                    bibliographic_limits,
                    &model_root.join("successor-agent-selected.sqlite"),
                    binding,
                    selected_stage_limits,
                    &selected_owner,
                    &selected_isolation,
                    entity_raw,
                    relation_raw,
                    &descriptor_raw,
                    tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
                    native_limits,
                    original_limits,
                    full_limits,
                    &[],
                    "synthetic-agent-successor-model".into(),
                    |stage, registry| {
                        successor_carriers =
                            tos_compiler::knowledge_full_fixture::native_fixture_produced_carriers(
                                stage,
                                selected_stage_limits,
                            )?;
                        tos_compiler::knowledge_full_fixture::native_fixture_header_with_binding(
                            stage,
                            registry,
                            entity_raw,
                            &basis,
                            processor_digest,
                            descriptor_digest,
                        )
                    },
                )
            },
        )
        .unwrap();
    assert_eq!(
        second_selected.source_proof().through_commit_seq(),
        second_receipt.commit_seq
    );
    assert_eq!(
        second_selected.selection_expectation().descriptor_sha256,
        descriptor_digest.to_hex()
    );
    assert_ne!(
        second_selected.selection_expectation().model_sha256,
        initial_selected
            .as_ref()
            .unwrap()
            .selection_expectation()
            .model_sha256
    );
    assert_eq!(
        second_selected.source_catalog_root_sha256(),
        initial_selected
            .as_ref()
            .unwrap()
            .source_catalog_root_sha256()
    );
    let mut synthetic_read = ManagedFixtureReadGrant::new(&second_selected, successor_carriers);
    let mut no_checkpoints = ManagedFixtureNoCheckpoints;
    let query_budgets = managed_fixture_query_budgets();
    let query_profile = tos_access::AccessProfile::new(65536, 1024 * 1024, 1024 * 1024)
        .with_query_timeout(Duration::from_secs(10));
    let history_edge = synthetic_read
        .carriers
        .iter()
        .find(|row| {
            row.kind == tos_compiler::knowledge_full_fixture::NativeFixtureCarrierKind::Relation
        })
        .expect("actual retained record-history relation")
        .id
        .clone();
    for request in [
        tos_access::KnowledgeRequest::Catalog,
        tos_access::KnowledgeRequest::Node {
            node_id: "tos.agent.synthetic-durable-current".into(),
            relation_limit: 16,
        },
        tos_access::KnowledgeRequest::Relation {
            relation_id: history_edge.clone(),
        },
    ] {
        let mut catalog = synthetic_read.clone();
        let mut inspect = synthetic_read.clone();
        inspect.operation = if matches!(&request, tos_access::KnowledgeRequest::Relation { .. }) {
            tos_query::RELATION_INSPECT_OPERATION
        } else {
            tos_query::NODE_INSPECT_OPERATION
        }
        .into();
        let expected_schema = match &request {
            tos_access::KnowledgeRequest::Node { .. } => Some("tos_knowledge_node_packet_v2"),
            tos_access::KnowledgeRequest::Relation { .. } => {
                Some("tos_knowledge_relation_packet_v2")
            }
            _ => None,
        };
        let mut output = ManagedFixtureOutput {
            bytes: Vec::new(),
            active: synthetic_read.active.clone(),
            flushed: false,
        };
        let mut errors = Vec::new();
        let result = second_selected
            .write_current_knowledge(
                &mut reopened_db,
                &reopened_store,
                &current_filesystem,
                &current_package,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                &vocabulary,
                &descriptor_raw,
                &mut catalog,
                &mut inspect,
                &mut no_checkpoints,
                request,
                query_budgets,
                query_profile,
                &mut output,
                &mut errors,
                deadline,
                &cancelled,
            )
            .unwrap();
        assert_eq!(
            result,
            0,
            "actual current held selected transport: {}",
            String::from_utf8_lossy(&errors)
        );
        assert_eq!(output.bytes.last(), Some(&b'\n'));
        assert!(output.flushed);
        assert_eq!(synthetic_read.active.load(Ordering::SeqCst), 0);
        let packet: serde_json::Value = serde_json::from_slice(&output.bytes).unwrap();
        if let Some(schema) = expected_schema {
            assert_eq!(packet["schema"], schema);
            assert_eq!(
                packet["source_basis"]["kind"],
                if addressed {
                    "managed_current_v2"
                } else {
                    "managed_current"
                }
            );
            assert_eq!(
                packet["managed_source_root_sha256"],
                second_selected.source_proof().root_sha256().unwrap()
            );
            assert!(packet.get("source_revision").is_none());
            if schema == "tos_knowledge_node_packet_v2" {
                assert_eq!(packet["matches"].as_array().unwrap().len(), 1);
                let attributes = &packet["matches"][0]["attributes"];
                let record_name = second_path.as_str().rsplit('/').next().unwrap();
                let body: serde_json::Value =
                    serde_json::from_slice(&current_package.files()[record_name]).unwrap();
                let form_name =
                    record_name.strip_suffix(".json").unwrap().to_owned() + ".human-forms.json";
                let set: serde_json::Value =
                    serde_json::from_slice(&current_package.files()[&form_name]).unwrap();
                assert_eq!(attributes["source_record"], body);
                assert_eq!(
                    attributes["human_forms"],
                    serde_json::json!(
                        tos_command::source_forms_compiler::materialize_compiler_forms(
                            &body, &set, 262_144
                        )
                        .unwrap()
                    )
                );
                assert_eq!(attributes["record_history"]["status"], "available");
                assert_eq!(attributes["record_history"]["current_ref"], set["subject"]);
                assert_eq!(
                    attributes["record_history"]["refs"],
                    serde_json::json!([set["subject"].clone()])
                );
                assert_eq!(attributes["record_history"]["grants_current_use"], false);
            }
        }
    }
    // The explicit fixture read grant has a real retained withdrawal/recheck
    // through the same transport lease. It is independent of create authority.
    synthetic_read.withdrawn.store(true, Ordering::SeqCst);
    let mut catalog = synthetic_read.clone();
    let mut inspect = synthetic_read.clone();
    let mut denied_output = Vec::new();
    let mut denied_errors = Vec::new();
    let withdrawn = second_selected
        .write_current_knowledge(
            &mut reopened_db,
            &reopened_store,
            &current_filesystem,
            &current_package,
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            &vocabulary,
            &descriptor_raw,
            &mut catalog,
            &mut inspect,
            &mut no_checkpoints,
            tos_access::KnowledgeRequest::Catalog,
            query_budgets,
            query_profile,
            &mut denied_output,
            &mut denied_errors,
            deadline,
            &cancelled,
        )
        .unwrap_err();
    match withdrawn {
        tos_command::source_managed_selection::ManagedSelectionError::Access(error) => {
            assert_eq!(error.code, tos_access::AccessErrorCode::PolicyDenied);
            assert_eq!(error.message, "finite projection hold withdrawn");
        }
        other => panic!("unexpected withdrawn read boundary: {other:?}"),
    }
    assert!(denied_output.is_empty());
    assert!(
        denied_errors.is_empty(),
        "refusal precedes the packet writer"
    );
    assert_eq!(synthetic_read.active.load(Ordering::SeqCst), 0);
    synthetic_read.withdrawn.store(false, Ordering::SeqCst);
    let successor = second_selected.generation();
    // The next real command consumes the returned warm successor, without
    // cold re-auditing unchanged source bodies or exporting a v1 manifest.
    let mut warm_context = current_context.clone();
    let mut warm_config = current_config.clone();
    warm_config["source_path"] =
        serde_json::json!("ToS/source-witnesses/agents/synthetic-durable-warm/agent.json");
    warm_config["record_id"] = serde_json::json!("tos.agent.synthetic-durable-warm");
    warm_config["allowed_form_ids"] = serde_json::json!(["tos.form.synthetic-durable-warm"]);
    warm_config["provenance_event_id"] = serde_json::json!("tos.event.synthetic-durable-warm");
    let warm_owner = isolated.path().join("owner-warm.json");
    fs::write(&warm_owner, canonical(&warm_config)).unwrap();
    fs::set_permissions(&warm_owner, fs::Permissions::from_mode(0o600)).unwrap();
    let warm_filesystem =
        CreationFilesystem::select_isolated(&isolated, &warm_owner, deadline, &cancelled).unwrap();
    warm_context.configuration_raw = canonical(&warm_config);
    let mut warm_request = current_request.clone();
    for field in [
        "command_id",
        "expected_configuration",
        "expected_dependencies",
        "expected_source",
        "expected_revision",
    ] {
        warm_request.as_object_mut().unwrap().remove(field);
    }
    warm_request["operation"] = serde_json::json!("prepare-create");
    warm_request["record"]["record_id"] = serde_json::json!("tos.agent.synthetic-durable-warm");
    warm_request["record"]["source_refs"] = serde_json::json!([second_path.as_str()]);
    warm_request["forms"][0]["form_id"] = serde_json::json!("tos.form.synthetic-durable-warm");
    warm_context.request_raw = canonical(&warm_request);
    let warm_input = tos_command::source_creation::select_managed_agent_creation_input(
        &mut reopened_db,
        &reopened_store,
        &successor,
        &warm_context,
        &cut,
        &software,
        &components,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut warm_worker = new_worker(&cut);
    let warm_preview = tos_command::source_creation::prepare_managed_agent_creation(
        &warm_input,
        &cut,
        &software,
        &components,
        &mut warm_worker,
        deadline,
        &cancelled,
    )
    .unwrap()
    .preview()
    .unwrap();
    warm_request["operation"] = serde_json::json!("source.create");
    warm_request["command_id"] = serde_json::json!("synthetic:durable-current-warm");
    for (key, field) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_dependencies", "expected_dependencies"),
    ] {
        warm_request[key] =
            serde_json::json!(warm_preview.object_get(field).unwrap().as_str().unwrap());
    }
    warm_request["expected_source"] = serde_json::Value::Null;
    warm_request["expected_revision"] = serde_json::Value::Null;
    warm_context.request_raw = canonical(&warm_request);
    let (warm_package, warm_receipt, _, warm_successor) = reopened_db
        .execute_managed_agent_creation_from_captures(
            &reopened_store,
            &successor,
            &cut,
            b"agent-current-warm",
            &warm_filesystem,
            &warm_context,
            &software,
            &components,
            &mut warm_worker,
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            deadline,
            &cancelled,
        )
        .unwrap();
    assert_eq!(warm_receipt.commit_seq, second_receipt.commit_seq + 1);
    drop(warm_worker);
    assert_eq!(warm_successor.commit_seq(), warm_receipt.commit_seq);
    assert_ne!(warm_successor.digest(), successor.digest());
    assert!(
        warm_package
            .reads()
            .iter()
            .any(|r| r.path == second_path
                && r.raw_sha256 == Digest256::of_bytes(&successor_member.raw)),
        "warm preparation consumes the preceding creation's exact original body"
    );
    assert!(
        warm_package
            .reads()
            .iter()
            .filter(|r| r.path.as_str().starts_with("ToS/"))
            .count()
            < reopened.files.len(),
        "warm preparation remains addressed"
    );
    let mut catalog = synthetic_read.clone();
    let mut inspect = synthetic_read.clone();
    let mut stale_output = Vec::new();
    let mut stale_errors = Vec::new();
    assert!(
        second_selected
            .write_current_knowledge(
                &mut reopened_db,
                &reopened_store,
                &current_filesystem,
                &current_package,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                &vocabulary,
                &descriptor_raw,
                &mut catalog,
                &mut inspect,
                &mut no_checkpoints,
                tos_access::KnowledgeRequest::Catalog,
                query_budgets,
                query_profile,
                &mut stale_output,
                &mut stale_errors,
                deadline,
                &cancelled,
            )
            .is_err(),
        "the selected full successor cannot disclose after the next actual committed generation"
    );
    assert!(stale_output.is_empty());
    if addressed {
        // Journal insertion and the generation increment must disappear with
        // a rolled-back semantic mutation, leaving the selected capability live.
        let mut rollback_client = Client::connect(&url, NoTls).unwrap();
        let observation = |client: &mut Client| -> (i64, i64) {
            let r = client.query_one("SELECT f.generation,(SELECT count(*) FROM cmd2_audit_delta_v1 WHERE domain=f.domain) AS journal_rows FROM cmd2_audit_fence f WHERE domain=$1", &[&lab.domain]).unwrap();
            (r.get(0), r.get(1))
        };
        let before = observation(&mut rollback_client);
        let mut rollback = rollback_client.transaction().unwrap();
        rollback
            .execute(
                "UPDATE cmd2_domain SET rights_allowed=NOT rights_allowed WHERE domain=$1",
                &[&lab.domain],
            )
            .unwrap();
        rollback.rollback().unwrap();
        assert_eq!(observation(&mut rollback_client), before);
        let exact = warm_successor
            .read_historical_member(
                &mut reopened_db,
                &reopened_store,
                &second_path,
                1,
                8_388_608,
                deadline,
                &cancelled,
            )
            .unwrap();
        assert_eq!(exact.raw, successor_member.raw);
    }
    let expected_current_head = warm_receipt.commit_seq;
    drop(warm_package);
    drop(warm_receipt);
    drop(warm_successor);
    drop(warm_input);
    drop(warm_preview);
    drop(warm_context);
    drop(warm_request);
    drop(warm_config);
    drop(warm_filesystem);
    // Retain only independent selection references and an assertion oracle;
    // every managed input/package/generation and original process handle dies.
    let expected_original_files = current_package.files().clone();
    let expected_commit_seq = second_receipt.commit_seq;
    let recovery_domain = lab.domain.clone();
    let recovery_software_selection = software.selection().clone();
    let bootstrap_context = contexts.remove(0);
    drop(current_package);
    drop(attempts);
    drop(second_receipt);
    drop(input);
    drop(preview);
    drop(current_context);
    drop(current_request);
    drop(current_config);
    drop(current_filesystem);
    drop(successor);
    drop(current);
    drop(second_selected);
    drop(initial_selected);
    drop(successor_member);
    drop(initial_cohort);
    drop(verified);
    drop(replay_attempt);
    drop(packages);
    drop(contexts);
    drop(owners);
    drop(reopened);
    drop(reopened_db);
    drop(reopened_store);
    drop(software);
    drop(components);
    drop(cut);
    drop(sql);
    drop(lab);
    let recovered_root = ScratchRoot::new();
    let backup_root = ScratchRoot::new();
    let url = if addressed {
        let restored_url = std::env::var("TOS_CMD2_V2_RESTORE_PG_URL")
            .expect("V2 whole operation requires a distinct empty owned restore database");
        assert_ne!(url, restored_url, "restore target must be independent");
        for root in [&copied.0, &backup_root.0, &recovered_root.0] {
            fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let journal = addressed_journal_snapshot(&url, &recovery_domain);
        run_addressed_transport_phase(
            "backup",
            &url,
            &recovery_domain,
            &copied.0,
            &backup_root.0,
            None,
            deadline,
        );
        let receipt = fs::read(backup_root.0.join("receipt.json")).unwrap();
        assert!(receipt.len() <= 1024 * 1024);
        let receipt_sha256 = Digest256::of_bytes(&receipt).to_hex();
        run_addressed_transport_phase(
            "restore",
            &restored_url,
            &recovery_domain,
            &recovered_root.0,
            &backup_root.0,
            Some(&receipt_sha256),
            deadline,
        );
        assert_eq!(
            journal,
            addressed_journal_snapshot(&restored_url, &recovery_domain),
            "actual DB restore preserves exact V2 journal/profile/fence before cold selection"
        );
        restored_url
    } else {
        copy_store_tree(&copied.0, &recovered_root.0);
        url
    };
    let recovered_store = SegmentStore::open_existing(&recovered_root.0, limits()).unwrap();
    let mut recovered_db = DurablePgCoordinator::connect(&url).unwrap();
    // Every recovery-tail corruption/control must target the same database
    // that the restored coordinator reads, after all original handles died.
    let mut sql = Client::connect(&url, NoTls).unwrap();
    let cut = CorpusReader::open_existing(&source_root, read_limits)
        .unwrap()
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: 4,
                max_members: 2048,
                max_total_bytes: 33_554_432,
                max_member_bytes: 8_388_608,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let software = SoftwareCaptureReader::open(
        &capture,
        &restored,
        recovery_software_selection,
        read_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let components = software.select_components(&paths).unwrap();
    let current_filesystem =
        CreationFilesystem::select_isolated(&isolated, &current_owner, deadline, &cancelled)
            .unwrap();
    let current_reopened = tos_command::source_current_cut::select_current_source_generation(
        &mut recovered_db,
        &recovered_store,
        &recovery_domain,
        &cut,
        revision,
        membership,
        &bootstrap_context,
        &software,
        &components,
        &mut new_worker(&cut),
        Some((&generation_workspace, generation_profile)),
        deadline,
        &cancelled,
    )
    .unwrap();
    let current_reopened = if addressed {
        recovered_db
            .migrate_current_source_generation_addressed(
                &recovered_store,
                &current_reopened,
                tree_limits,
                deadline,
                &cancelled,
            )
            .unwrap()
    } else {
        current_reopened
    };
    // Record ordinary and controlled plans for the actual keyset queries.
    // These small-fixture observations do not establish whole-query capacity
    // or gate the current/cold/recovery protection checks below.
    let mut planner = Client::connect(&url, NoTls).unwrap();
    let mut planner_tx = planner.transaction().unwrap();
    let seek_queries = [
        (
            "cmd2_current_source_cold_seek",
            "SELECT c.*,a.attempt_fence AS source_attempt_fence,
                    h.inventory_projection AS retained_inventory_projection
             FROM cmd2_current c JOIN cmd2_attempt a USING(domain,prepare_id)
             LEFT JOIN cmd2_history h USING(domain,subject,revision)
             WHERE c.domain=$1 AND
               (c.prepare_id,c.member_slot) > (''::bytea,-1)
             ORDER BY c.prepare_id,c.member_slot LIMIT 8",
        ),
        (
            "cmd2_source_index_cold_seek",
            "SELECT * FROM cmd2_source_index WHERE domain=$1 AND
               (kind COLLATE \"C\",token COLLATE \"C\") >
               ('' COLLATE \"C\",'' COLLATE \"C\")
             ORDER BY kind COLLATE \"C\",token COLLATE \"C\" LIMIT 8",
        ),
        (
            "cmd2_predicate_source_cold_seek",
            "SELECT * FROM cmd2_predicate WHERE domain=$1 AND owner='native-corpus-create:agent' AND
               (kind COLLATE \"C\",scope COLLATE \"C\",token COLLATE \"C\") >
               ('' COLLATE \"C\",'' COLLATE \"C\",'' COLLATE \"C\")
             ORDER BY kind COLLATE \"C\",scope COLLATE \"C\",token COLLATE \"C\" LIMIT 8",
        ),
    ];
    for (index, query) in seek_queries {
        let plan = planner_tx
            .query(&format!("EXPLAIN (COSTS OFF) {query}"), &[&recovery_domain])
            .unwrap()
            .into_iter()
            .map(|row| row.get::<_, String>(0))
            .collect::<Vec<_>>()
            .join("\n");
        eprintln!("ordinary source seek plan {index}:\n{plan}");
    }
    planner_tx
        .batch_execute("SET LOCAL enable_seqscan=off; SET LOCAL enable_sort=off")
        .unwrap();
    for (index, query) in seek_queries {
        let plan = planner_tx
            .query(&format!("EXPLAIN (COSTS OFF) {query}"), &[&recovery_domain])
            .unwrap()
            .into_iter()
            .map(|row| row.get::<_, String>(0))
            .collect::<Vec<_>>()
            .join("\n");
        eprintln!("eligible source seek plan {index}:\n{plan}");
        let indexed_without_sort =
            plan.contains(index) && !plan.lines().any(|line| line.contains("Sort"));
        eprintln!(
            "diagnostic indexed-without-sort observation {index}: {indexed_without_sort}; whole-query capacity remains unestablished"
        );
    }
    planner_tx.commit().unwrap();
    eprintln!("STO tail begin: current member absence and original bytes");
    let absent_path =
        RelativePath::parse("ToS/source-witnesses/agents/never-created/agent.json").unwrap();
    assert!(
        current_reopened
            .member(
                &mut recovered_db,
                &recovered_store,
                &absent_path,
                deadline,
                &cancelled,
            )
            .unwrap()
            .is_none()
    );
    eprintln!("STO tail absent member PASS");
    let original_paths = expected_original_files
        .keys()
        .map(|name| {
            RelativePath::parse(&format!(
                "ToS/source-witnesses/agents/synthetic-durable-current/{name}"
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
    eprintln!("STO tail addressed original member batch begin");
    let original_members = current_reopened
        .read_current_members(
            &mut recovered_db,
            &recovered_store,
            &original_paths,
            8_388_608,
            33_554_432,
            deadline,
            &cancelled,
        )
        .unwrap();
    eprintln!("STO tail addressed original member batch ready");
    for (((name, expected), path), retained) in expected_original_files
        .iter()
        .zip(&original_paths)
        .zip(original_members)
    {
        eprintln!("STO tail original member {name} begin");
        let owned_metadata = current_reopened
            .member(
                &mut recovered_db,
                &recovered_store,
                path,
                deadline,
                &cancelled,
            )
            .unwrap()
            .unwrap();
        assert_eq!(owned_metadata, retained.metadata);
        assert_eq!(&retained.raw, expected, "process-cold original file {name}");
        assert_eq!(retained.current_generation, expected_current_head);
        eprintln!("STO tail original member {name} metadata/bytes/head PASS");
    }
    eprintln!("STO tail member metadata/original bytes PASS; prepare recovery worker");
    // A successful recovery finishes its actual worker before the commit
    // fence. Preserve that boundary; the later refusal-only calls can share
    // one new worker while retaining their independent durable checks.
    let mut recovery_worker = new_worker(&cut);
    eprintln!("STO tail recovery worker ready; committed replay begin");
    let (second_replayed, _) = recovered_db
        .recover_committed_managed_agent_creation(
            &recovered_store,
            current_reopened.cohort(),
            b"agent-current-second",
            &current_filesystem,
            &cut,
            &software,
            &components,
            &mut recovery_worker,
            contract_digest(),
            0,
            0,
            "private-job",
            1,
            deadline,
            &cancelled,
        )
        .unwrap();
    assert!(second_replayed.replayed);
    assert_eq!(second_replayed.commit_seq, expected_commit_seq);
    assert_eq!(
        recovered_db.head_seq(&recovery_domain).unwrap(),
        expected_current_head
    );
    drop(recovery_worker);
    eprintln!("STO tail committed replay/head PASS; prepare refusal worker");
    // Pending, corrupt binding, revoked rights and changed live owner refuse
    // before original-byte reconstruction or successful worker completion.
    // The unchanged positive recovery retains its independent final fence.
    let mut recovery_worker = new_worker(&cut);
    eprintln!("STO tail refusal worker ready; pending refusal begin");
    assert!(
        matches!(
            recovered_db.recover_committed_managed_agent_creation(
                &recovered_store,
                current_reopened.cohort(),
                b"agent-1",
                &current_filesystem,
                &cut,
                &software,
                &components,
                &mut recovery_worker,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled,
            ),
            Err(DurableError::Refused(_))
        ),
        "recovery cannot turn a pending attempt into a write"
    );
    let refusal_profile = StreamedGenerationProfile {
        max_metadata_rows: 1,
        ..generation_profile
    };
    assert!(matches!(
        recovered_db.cold_verify_cut_streamed(
            &recovered_store,
            &recovery_domain,
            &generation_workspace,
            refusal_profile,
            deadline,
            &cancelled,
        ),
        Err(DurableError::Refused(
            "cold metadata preadmission budget exceeded"
        ))
    ));
    assert!(matches!(
        recovered_db.cold_reopen_source_cohort_streamed(
            &recovered_store,
            &recovery_domain,
            &cut,
            revision,
            membership,
            &bootstrap_context,
            &software,
            &components,
            &mut recovery_worker,
            &generation_workspace,
            refusal_profile,
            deadline,
            &cancelled,
        ),
        Err(DurableError::Refused(
            "cold metadata preadmission budget exceeded"
        ))
    ));
    assert_eq!(
        recovered_db.head_seq(&recovery_domain).unwrap(),
        expected_current_head
    );
    eprintln!("STO cold metadata row overflow refuses both consumers without publication PASS");
    eprintln!("STO tail pending refusal PASS; original binding corruptions begin");
    let original_reads: Vec<u8> = sql
        .query_one(
            "SELECT source_reads FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&recovery_domain, &&b"agent-current-second"[..]],
        )
        .unwrap()
        .get(0);
    for corruption in [
        "original-basis",
        "registered-read-closure",
        "legacy-managed-no-companion",
    ] {
        eprintln!("STO tail corruption {corruption} begin");
        let mut corrupt: serde_json::Value = serde_json::from_slice(&original_reads).unwrap();
        match corruption {
            "original-basis" => {
                corrupt["original"]["basis"][2] = serde_json::json!(expected_commit_seq)
            }
            "registered-read-closure" => {
                let generation = corrupt["reads"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|read| read[0] == "generation")
                    .unwrap();
                generation[6] = serde_json::json!(generation[6].as_u64().unwrap() + 1);
            }
            _ => corrupt = corrupt["reads"].clone(),
        }
        let corrupt_bytes = serde_json::to_vec(&corrupt).unwrap();
        sql.execute(
            "UPDATE cmd2_attempt SET source_reads=$3 WHERE domain=$1 AND prepare_id=$2",
            &[
                &recovery_domain,
                &&b"agent-current-second"[..],
                &corrupt_bytes,
            ],
        )
        .unwrap();
        assert!(
            recovered_db
                .recover_committed_managed_agent_creation(
                    &recovered_store,
                    current_reopened.cohort(),
                    b"agent-current-second",
                    &current_filesystem,
                    &cut,
                    &software,
                    &components,
                    &mut recovery_worker,
                    contract_digest(),
                    0,
                    0,
                    "private-job",
                    1,
                    deadline,
                    &cancelled,
                )
                .is_err(),
            "corrupt or missing original managed binding cannot recover: {corruption}"
        );
        sql.execute(
            "UPDATE cmd2_attempt SET source_reads=$3 WHERE domain=$1 AND prepare_id=$2",
            &[
                &recovery_domain,
                &&b"agent-current-second"[..],
                &original_reads,
            ],
        )
        .unwrap();
        eprintln!("STO tail corruption {corruption} refusal and restoration PASS");
    }
    eprintln!("STO tail original binding corruptions PASS; rights refusal begin");
    sql.execute(
        "UPDATE cmd2_domain SET rights_allowed=false WHERE domain=$1",
        &[&recovery_domain],
    )
    .unwrap();
    assert!(
        matches!(
            recovered_db.recover_committed_managed_agent_creation(
                &recovered_store,
                current_reopened.cohort(),
                b"agent-current-second",
                &current_filesystem,
                &cut,
                &software,
                &components,
                &mut recovery_worker,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled,
            ),
            Err(DurableError::Refused(_))
        ),
        "process-cold recovery still needs current rights"
    );
    sql.execute(
        "UPDATE cmd2_domain SET rights_allowed=true WHERE domain=$1",
        &[&recovery_domain],
    )
    .unwrap();
    eprintln!("STO tail rights refusal PASS; changed current owner begin");
    let original_owner_raw = fs::read(&current_owner).unwrap();
    let mut changed_owner: serde_json::Value = serde_json::from_slice(&original_owner_raw).unwrap();
    changed_owner["principal_id"] = serde_json::json!("software:changed-owner-after-process-loss");
    fs::write(&current_owner, canonical(&changed_owner)).unwrap();
    let changed_filesystem =
        CreationFilesystem::select_isolated(&isolated, &current_owner, deadline, &cancelled)
            .unwrap();
    assert!(
        matches!(
            recovered_db.recover_committed_managed_agent_creation(
                &recovered_store,
                current_reopened.cohort(),
                b"agent-current-second",
                &changed_filesystem,
                &cut,
                &software,
                &components,
                &mut recovery_worker,
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled,
            ),
            Err(DurableError::Source(
                tos_command::source_command::SourceCommandError::Conflict(
                    "creation delegation changed before publication"
                )
            ))
        ),
        "original capture cannot override changed current protected owner"
    );
    fs::write(&current_owner, &original_owner_raw).unwrap();
    assert_eq!(
        recovered_db.head_seq(&recovery_domain).unwrap(),
        expected_current_head
    );
    eprintln!("STO tail changed owner/head PASS; finish unused refusal worker");
    recovery_worker.finish(deadline, &cancelled).unwrap();
    drop(recovery_worker);
    eprintln!("STO tail independent cold missing-index refusal begin");
    // Corruption must not be relabelled complete by a cold/open SQL marker.
    sql.execute(
        "DELETE FROM cmd2_source_index WHERE domain=$1 AND kind='metadata' AND token=$2",
        &[&recovery_domain, &"tos.agent.synthetic-durable-a"],
    )
    .unwrap();
    assert!(
        recovered_db
            .cold_reopen_source_cohort(
                &recovered_store,
                &recovery_domain,
                &cut,
                revision,
                membership,
                &bootstrap_context,
                &software,
                &components,
                &mut new_worker(&cut),
                deadline,
                &cancelled
            )
            .is_err()
    );
    eprintln!("STO tail independent cold missing-index refusal PASS; complete");
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
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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

    // These independent owner-contract fences were previously exercised only
    // by CMD.1's synthetic byte coordinator. Here each stale proposal owns
    // actual sealed STO bytes and must leave every committed projection empty.
    for rule_drift in [true, false] {
        let mut drift = Lab::new(&url);
        let members = drift.prepare(
            b"prepare-contract-drift",
            "contract-drift",
            &[MemberSpec::first("subject-contract", b"private contract")],
        );
        let mut owner = Client::connect(&url, NoTls).unwrap();
        if rule_drift {
            owner
                .execute(
                    "UPDATE cmd2_domain SET rule_version=1 WHERE domain=$1",
                    &[&drift.domain],
                )
                .unwrap();
        } else {
            owner
                .execute(
                    "UPDATE cmd2_domain SET contract_digest=$2 WHERE domain=$1",
                    &[
                        &drift.domain,
                        &Digest256::of_bytes(b"changed schema/registry/backend").to_hex(),
                    ],
                )
                .unwrap();
        }
        assert!(matches!(
            drift.commit(b"prepare-contract-drift", &members, 0, 1),
            Err(DurableError::Conflict(_))
        ));
        assert_eq!(drift.head_seq(), 0);
        for table in ["current", "history", "receipt", "log", "outbox"] {
            assert_eq!(drift.count(table), 0);
        }
    }
}

#[test]
fn compound_second_member_conflict_rolls_back_every_projection() {
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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
    let selected =
        a.db.cold_recover_exact(&a.store, &a.domain, "subject-A", 1)
            .unwrap();
    assert_eq!(
        a.db.warm_read_selected(&a.store, &selected, 1024).unwrap(),
        members[0].exact_bytes
    );
}

#[test]
fn cancel_during_fenced_seal_retries_after_late_pin_completion() {
    let url = database_url();
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
fn rights_revocation_remains_available_beyond_finite_cut_budget() {
    let url = database_url();
    for prior_head in [100_000i64, i64::MAX - 1] {
        let mut lab = Lab::new(&url);
        let mut client = Client::connect(&url, NoTls).unwrap();
        // A deliberately sparse sequence fixture isolates the rights
        // transaction boundary. It is not a valid cold cut or scale result.
        client
            .execute(
                "UPDATE cmd2_domain SET head_seq=$2 WHERE domain=$1",
                &[&lab.domain, &prior_head],
            )
            .unwrap();
        let next = prior_head + 1;
        assert_eq!(lab.db.revoke_local(&lab.domain).unwrap(), next as u64);
        let state = client
            .query_one(
                "SELECT head_seq,rights_version,rights_allowed FROM cmd2_domain WHERE domain=$1",
                &[&lab.domain],
            )
            .unwrap();
        assert_eq!(state.get::<_, i64>(0), next);
        assert_eq!(state.get::<_, i64>(1), 1);
        assert!(!state.get::<_, bool>(2));
        let events: i64 = client
            .query_one(
                "SELECT count(*) FROM cmd2_log l JOIN cmd2_outbox o USING(domain,commit_seq) \
                 WHERE l.domain=$1 AND l.commit_seq=$2 AND l.event_kind='rights'",
                &[&lab.domain, &next],
            )
            .unwrap()
            .get(0);
        assert_eq!(events, 1);
        assert_eq!(lab.count("log"), 1);
        assert_eq!(lab.count("outbox"), 1);
        if next == i64::MAX {
            assert!(lab.db.revoke_local(&lab.domain).is_err());
            let unchanged = client
                .query_one(
                    "SELECT head_seq,rights_version,rights_allowed FROM cmd2_domain WHERE domain=$1",
                    &[&lab.domain],
                )
                .unwrap();
            assert_eq!(unchanged.get::<_, i64>(0), next);
            assert_eq!(unchanged.get::<_, i64>(1), 1);
            assert!(!unchanged.get::<_, bool>(2));
            assert_eq!(lab.count("log"), 1);
            assert_eq!(lab.count("outbox"), 1);
        }
    }
}

#[test]
fn rights_revocation_blocks_pending_commit_and_committed_replay() {
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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

// Exercise the actual first publication lock while its owner remains held.
// A timeout must roll back without publishing; a subsequent exact retry uses
// the same existing candidate and succeeds after this owner releases the lock.
fn publication_lock_timeout(
    url: &str,
    domain: &str,
    publish: impl FnOnce(&mut DurablePgCoordinator) -> tos_command::DurableResult<()> + Send + 'static,
) {
    let mut blocker = Client::connect(url, NoTls).unwrap();
    let mut tx = blocker.transaction().unwrap();
    let blocker_pid: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    tx.query_one(
        "SELECT 1 FROM cmd2_audit_fence WHERE domain=$1 FOR UPDATE",
        &[&domain],
    )
    .unwrap();
    let (pid_send, pid_recv) = mpsc::channel();
    let (result_send, result_recv) = mpsc::channel();
    let worker_url = url.to_owned();
    let worker = thread::spawn(move || {
        let mut db = DurablePgCoordinator::connect(&worker_url).unwrap();
        pid_send.send(db.backend_pid().unwrap()).unwrap();
        let result = publish(&mut db);
        result_send
            .send(matches!(result,
                Err(DurableError::Database(ref error))
                    if error.code() == Some(&postgres::error::SqlState::LOCK_NOT_AVAILABLE)
            ))
            .unwrap();
    });
    let worker_pid = pid_recv.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_pg_row_block(url, worker_pid, blocker_pid);
    assert!(result_recv.recv_timeout(Duration::from_secs(8)).unwrap());
    tx.rollback().unwrap();
    worker.join().unwrap();
}

#[test]
fn rights_change_after_observed_audit_fence_wait_refuses_commit() {
    let url = database_url();
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
    let url = database_url();
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
    let url = database_url();
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
fn cold_audit_pages_log_and_preadmits_metadata_bytes() {
    let url = database_url();
    let mut lab = Lab::new(&url);
    for expected in 1..=129 {
        assert_eq!(lab.db.revoke_local(&lab.domain).unwrap(), expected);
    }
    let cut = lab.db.cold_verify_cut(&lab.store, &lab.domain).unwrap();
    assert_eq!(cut.through_commit_seq(), 129);
    assert_eq!(cut.historical_members(), 0);

    // The PK stays short; the unindexed definition makes one JSON metadata
    // row too large to enter the client even though the total row count is
    // small. The preadmission scan must refuse before materializing it.
    let mut client = Client::connect(&url, NoTls).unwrap();
    client
        .execute(
            "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version)
             VALUES($1,'unique','lab','audit','oversized',$2)",
            &[&lab.domain, &"x".repeat(1_048_577)],
        )
        .unwrap();
    assert!(matches!(
        lab.db.cold_verify_cut(&lab.store, &lab.domain),
        Err(DurableError::Refused(_))
    ));
}

#[test]
fn streamed_selected_generation_merges_private_runs_and_cold_restores() {
    let url = database_url();
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"streamed-first",
        "streamed-first",
        &[MemberSpec::first("streamed-A", b"retained bytes")],
    );
    lab.commit(b"streamed-first", &first, 0, 1).unwrap();
    let second = lab.prepare(
        b"streamed-second",
        "streamed-second",
        &[
            MemberSpec {
                subject: "streamed-A",
                revision: 2,
                predecessor: Some((1, Digest256::of_bytes(&first[0].exact_bytes))),
                payload: b"current bytes",
            },
            MemberSpec::first("streamed-B", b"another current byte string"),
        ],
    );
    lab.commit(b"streamed-second", &second, 1, 1).unwrap();
    let scratch = ScratchRoot::new();
    let workspace = PrivateGenerationWorkspace::open(
        &scratch.0,
        ColdWorkspaceLimits {
            max_scratch_written_bytes: 64 * 1024,
            max_run_bytes: 512,
            max_runs: 8,
            merge_fan_in: 2,
            max_rows: 8,
            max_key_bytes: 256,
        },
    )
    .unwrap();
    let profile = StreamedGenerationProfile {
        max_commit_seq: 8,
        max_members: 8,
        max_pins: 8,
        max_segment_bytes: 8 * 1024 * 1024,
        max_membership_key_bytes: 4096,
        max_metadata_rows: 200,
        max_metadata_bytes: 1024 * 1024,
        max_elapsed: Duration::from_secs(60),
        max_pg_temp_bytes: 8 * 1024 * 1024,
        max_sql_statement_ms: 30_000,
        generation: GenerationReadLimits {
            max_descriptor_bytes: 8192,
            shape: GenerationShapeLimits {
                max_partitions: 4,
                max_rows_per_partition: 1,
                max_key_bytes: 256,
                max_leaf_bytes: 16 * 1024,
            },
            max_stream_rows: 8,
            max_stream_key_bytes: 4096,
        },
        rows_per_leaf: 1,
    };
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let cut = lab
        .db
        .cold_verify_cut_streamed(
            &lab.store,
            &lab.domain,
            &workspace,
            profile,
            deadline,
            &cancelled,
        )
        .unwrap();
    assert_eq!((cut.historical_members(), cut.current_members()), (3, 2));
    let candidate = lab
        .db
        .build_complete_generation(&lab.store, &cut, deadline, &cancelled)
        .unwrap();
    assert_eq!(candidate.descriptor().history.partitions.len(), 3);
    lab.db.select_complete_generation(&candidate).unwrap();

    let copied_root = ScratchRoot::new();
    copy_store_tree(&lab._root.0, &copied_root.0);
    let copied_store = SegmentStore::open_existing(&copied_root.0, limits()).unwrap();
    let mut restored = DurablePgCoordinator::connect(&url).unwrap();
    let selected = restored
        .cold_open_selected_generation_streamed(
            &copied_store,
            &lab.domain,
            &workspace,
            profile,
            Instant::now() + Duration::from_secs(60),
            &cancelled,
        )
        .unwrap();
    assert_eq!(selected.digest(), candidate.digest());
    let mut history = selected.stream(GenerationNamespaceV1::History).unwrap();
    let mut observed = 0;
    while history
        .next_row(Instant::now() + Duration::from_secs(60), &cancelled)
        .unwrap()
        .is_some()
    {
        observed += 1;
    }
    assert_eq!(observed, 3);
    assert_eq!(history.coverage().unwrap().rows, 3);
    let mut observer = Client::connect(&url, NoTls).unwrap();
    observer.execute(
        "UPDATE cmd2_current SET durability_class='forged' WHERE domain=$1 AND subject='streamed-B'",
        &[&lab.domain],
    ).unwrap();
    assert!(matches!(
        lab.db.cold_verify_cut_streamed(
            &lab.store,
            &lab.domain,
            &workspace,
            profile,
            Instant::now() + Duration::from_secs(60),
            &cancelled,
        ),
        Err(DurableError::Corrupt(
            "current locator differs from latest history"
        ))
    ));
}

#[test]
fn selected_generation_binds_complete_current_and_retained_membership() {
    let url = database_url();
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"generation-first",
        "generation-first",
        &[MemberSpec::first("generation-A", b"retained bytes")],
    );
    lab.commit(b"generation-first", &first, 0, 1).unwrap();
    let second = lab.prepare(
        b"generation-second",
        "generation-second",
        &[
            MemberSpec {
                subject: "generation-A",
                revision: 2,
                predecessor: Some((1, Digest256::of_bytes(&first[0].exact_bytes))),
                payload: b"current bytes",
            },
            MemberSpec::first("generation-B", b"compound bytes"),
        ],
    );
    lab.commit(b"generation-second", &second, 1, 1).unwrap();
    let cut = lab.db.cold_verify_cut(&lab.store, &lab.domain).unwrap();
    assert_eq!(cut.historical_members(), 3);
    assert_eq!(cut.current_members(), 2);
    let cancelled = AtomicBool::new(false);

    let other_root = ScratchRoot::new();
    let other_store =
        SegmentStore::initialize_empty(&other_root.0, lab.domain.as_bytes(), limits()).unwrap();
    assert_ne!(other_store.store_id(), lab.store.store_id());
    let incomplete_root = ScratchRoot::new();
    copy_store_tree(&lab._root.0, &incomplete_root.0);
    let missing_segment = fs::read_dir(incomplete_root.0.join("segments"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::remove_file(missing_segment).unwrap();
    let incomplete_store = SegmentStore::open_existing(&incomplete_root.0, limits()).unwrap();
    assert_eq!(incomplete_store.store_id(), lab.store.store_id());
    for foreign in [&other_store, &incomplete_store] {
        assert_eq!(foreign.custody_domain(), lab.domain.as_bytes());
        assert!(matches!(
            lab.db.build_complete_generation(
                foreign,
                &cut,
                Instant::now() + Duration::from_secs(30),
                &cancelled,
            ),
            Err(DurableError::Storage(_))
        ));
    }
    assert!(
        lab.db
            .cold_verify_cut(&incomplete_store, &lab.domain)
            .is_err()
    );
    let mut observer = Client::connect(&url, NoTls).unwrap();
    let selected_before: Option<String> = observer
        .query_one(
            "SELECT selected_generation_digest FROM cmd2_domain WHERE domain=$1",
            &[&lab.domain],
        )
        .unwrap()
        .get(0);
    assert!(selected_before.is_none());

    let candidate = lab
        .db
        .build_complete_generation(
            &lab.store,
            &cut,
            Instant::now() + Duration::from_secs(30),
            &cancelled,
        )
        .unwrap();
    assert_eq!(candidate.history_coverage().rows, 3);
    assert_eq!(candidate.current_coverage().rows, 2);

    // A STO leaf and descriptor can be perfectly well-formed yet describe
    // another same-count logical universe. Only comparison with the audited
    // PostgreSQL stream may certify complete membership.
    let mut forged = candidate.descriptor().clone();
    let shape = GenerationShapeLimits {
        max_partitions: 2,
        max_rows_per_partition: 100_000,
        max_key_bytes: 4096,
        max_leaf_bytes: 64 * 1024 * 1024,
    };
    let original_leaf = &forged.history.partitions[0];
    let mut leaf = lab
        .store
        .open_packed_leaf(original_leaf.content_digest, shape)
        .unwrap();
    leaf.rows.last_mut().unwrap().key.push(0xff);
    let content_digest = lab.store.install_packed_leaf(&leaf, shape).unwrap();
    let semantic = describe_placement_partition(
        lab.store.domain_digest(),
        leaf.bounds.clone(),
        leaf.rows.iter().cloned().map(Ok),
        shape,
    )
    .unwrap();
    forged.history.partitions = vec![PackedPartitionRefV1 {
        semantic,
        content_digest,
    }];
    forged.history.catalog_root = lab
        .store
        .verify_packed_catalog_shape(
            lab.store.custody_domain(),
            b"cmd2.history.v1",
            b"all",
            forged.history.key_codec_digest,
            KeyComparatorV1::RawUnsignedBytes,
            &forged.history.partitions,
            shape,
        )
        .unwrap();
    let forged_installed = lab
        .store
        .install_generation_candidate(
            forged,
            tos_segment_store::GenerationReadLimits {
                max_descriptor_bytes: 32 * 1024,
                shape,
                max_stream_rows: 100_000,
                max_stream_key_bytes: 16 * 1024 * 1024,
            },
        )
        .unwrap();
    assert!(matches!(
        lab.db.verify_generation_candidate(
            &lab.store,
            &cut,
            forged_installed,
            Instant::now() + Duration::from_secs(30),
            &cancelled,
        ),
        Err(DurableError::Corrupt("installed membership row differs"))
    ));

    let waiting_candidate = candidate.clone();
    publication_lock_timeout(&url, &lab.domain, move |db| {
        db.select_complete_generation(&waiting_candidate)
    });
    assert_eq!(lab.db.published_seq(&lab.domain).unwrap(), 0);
    lab.db.select_complete_generation(&candidate).unwrap();
    lab.db.select_complete_generation(&candidate).unwrap();

    let copied_root = ScratchRoot::new();
    copy_store_tree(&lab._root.0, &copied_root.0);
    let copied_store = SegmentStore::open_existing(&copied_root.0, limits()).unwrap();
    let mut restored = DurablePgCoordinator::connect(&url).unwrap();
    assert!(matches!(
        restored.cold_open_selected_generation(
            &copied_store,
            &lab.domain,
            Instant::now() - Duration::from_secs(1),
            &cancelled,
        ),
        Err(DurableError::Refused("cold audit deadline exceeded"))
    ));
    let selected = restored
        .cold_open_selected_generation(
            &copied_store,
            &lab.domain,
            Instant::now() + Duration::from_secs(30),
            &cancelled,
        )
        .unwrap();
    assert_eq!(selected.digest(), candidate.digest());
    assert_eq!(selected.history_coverage().rows, 3);
    assert_eq!(selected.current_coverage().rows, 2);
    let mut current_stream = selected.stream(GenerationNamespaceV1::Current).unwrap();
    assert!(
        current_stream
            .next_row(Instant::now() + Duration::from_secs(30), &cancelled)
            .unwrap()
            .is_some()
    );
    assert!(current_stream.coverage().is_none());
    let mut observed_current = 1;
    while current_stream
        .next_row(Instant::now() + Duration::from_secs(30), &cancelled)
        .unwrap()
        .is_some()
    {
        observed_current += 1;
    }
    assert_eq!(observed_current, 2);
    assert_eq!(current_stream.coverage().unwrap().rows, 2);
    drop(selected);

    let selected_file = copied_root
        .0
        .join("generations")
        .join(candidate.digest().to_hex());
    fs::remove_file(selected_file).unwrap();
    assert!(
        restored
            .cold_open_selected_generation(
                &copied_store,
                &lab.domain,
                Instant::now() + Duration::from_secs(30),
                &cancelled,
            )
            .is_err()
    );

    // A real next commit invalidates the optimistic selected cut. The old
    // descriptor remains an immutable historical object but cannot be
    // replayed as the new complete head.
    let third = lab.prepare(
        b"generation-third",
        "generation-third",
        &[MemberSpec::first("generation-C", b"advancing bytes")],
    );
    lab.commit(b"generation-third", &third, 2, 1).unwrap();
    assert!(matches!(
        lab.db.select_complete_generation(&candidate),
        Err(DurableError::Conflict(_))
    ));
    assert!(matches!(
        lab.db.cold_open_selected_generation(
            &lab.store,
            &lab.domain,
            Instant::now() + Duration::from_secs(30),
            &cancelled,
        ),
        Err(DurableError::Conflict(_))
    ));
}

#[test]
fn cold_cut_fence_rejects_same_count_mutation_and_aba() {
    let url = database_url();
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
    let url = database_url();
    let mut lab = Lab::new(&url);
    let members = lab.prepare(
        b"prepare-seal-order",
        "seal-order",
        &[MemberSpec::first("subject-O", b"ordering bytes")],
    );
    lab.commit(b"prepare-seal-order", &members, 0, 1).unwrap();
    let cold_store = SegmentStore::open_existing(&lab._root.0, limits()).unwrap();
    let cut = lab.db.cold_verify_cut(&cold_store, &lab.domain).unwrap();
    let waiting_cut = cut.clone();
    publication_lock_timeout(&url, &lab.domain, move |db| {
        db.seal_shadow_cut(&waiting_cut)
    });
    assert_eq!(lab.db.published_seq(&lab.domain).unwrap(), 0);
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
    let url = database_url();
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

// Lab-only knobs for the existing restore consumer, not corpus/product laws.
// The absence of both knobs preserves the accepted three-member fixture bytes.
#[derive(Clone, Copy)]
struct RestoreFixtureProfile {
    revisions: u64,
    payload_bytes: Option<usize>,
}
impl RestoreFixtureProfile {
    fn selected() -> Self {
        let revisions = std::env::var_os("TOS_CMD2_RESTORE_REVISIONS")
            .map(|s| {
                s.into_string()
                    .expect("lab revision count is UTF-8")
                    .parse::<u64>()
                    .expect("lab revision count is an integer")
            })
            .unwrap_or(2);
        let payload_bytes = std::env::var_os("TOS_CMD2_RESTORE_PAYLOAD_BYTES").map(|s| {
            s.into_string()
                .expect("lab payload byte count is UTF-8")
                .parse::<usize>()
                .expect("lab payload bytes is an integer")
        });
        // Small finite lab selection; transport's existing guards remain laws.
        assert!(
            (2..=64).contains(&revisions),
            "lab profile supports 2..64 A revisions"
        );
        assert!(
            payload_bytes.is_none_or(|n| (1..=65536).contains(&n)),
            "lab profile supports 1..65536 payload bytes"
        );
        assert!(3 * revisions + 6 <= 256);
        Self {
            revisions,
            payload_bytes,
        }
    }
    fn payload(self, revision: u64) -> Vec<u8> {
        if let Some(size) = self.payload_bytes {
            // Exact revision-dependent bytes; generated one revision at a time.
            return (0..size)
                .map(|i| (i as u8).wrapping_add(revision as u8))
                .collect();
        }
        match revision {
            1 => b"retained predecessor".to_vec(),
            2 => b"new selected version".to_vec(),
            _ => format!("retained revision {revision}").into_bytes(),
        }
    }
    fn read_cap(self) -> u64 {
        self.payload_bytes
            .map(|n| n as u64 + 18 + "restore-A".len() as u64)
            .unwrap_or(1024)
    }
}

/// Run this exact ignored test alone, then copy its reported sealed store and
/// pg_dump the quiescent synthetic PostgreSQL database. The intentional leak
/// keeps this one fixture on disk after the test process exits for restore.
#[test]
#[ignore = "manual owner-managed PostgreSQL dump and independent STO copy drill"]
fn export_cold_restore_fixture() {
    let started = Instant::now();
    let profile = RestoreFixtureProfile::selected();
    let first_payload = profile.payload(1);
    let second_payload = profile.payload(2);
    let url = database_url();
    let mut lab = Lab::new(&url);
    let first = lab.prepare(
        b"restore-prepare-a1",
        "restore-command-a1",
        &[MemberSpec::first("restore-A", &first_payload)],
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
                payload: &second_payload,
            },
            MemberSpec::first("restore-B", b"second compound member"),
        ],
    );
    lab.commit(b"restore-prepare-compound", &second, 1, 1)
        .unwrap();
    let mut previous = Digest256::of_bytes(&second[0].exact_bytes);
    let mut historical_frame_bytes =
        first[0].exact_bytes.len() + second.iter().map(|m| m.exact_bytes.len()).sum::<usize>();
    for revision in 3..=profile.revisions {
        let prepare = format!("restore-prepare-a{revision}");
        let command = format!("restore-command-a{revision}");
        let payload = profile.payload(revision);
        let members = lab.prepare(
            prepare.as_bytes(),
            &command,
            &[MemberSpec {
                subject: "restore-A",
                revision,
                predecessor: Some((revision - 1, previous)),
                payload: &payload,
            }],
        );
        assert_eq!(
            lab.commit(prepare.as_bytes(), &members, revision - 1, 1)
                .unwrap()
                .commit_seq,
            revision
        );
        historical_frame_bytes += members[0].exact_bytes.len();
        previous = Digest256::of_bytes(&members[0].exact_bytes);
    }
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
    assert_eq!(lab.count("current"), 2);
    assert_eq!(lab.count("history"), (profile.revisions + 1) as i64);
    assert_eq!(lab.count("receipt"), profile.revisions as i64);
    assert_eq!(lab.count("log"), profile.revisions as i64);
    assert_eq!(lab.count("attempt"), (profile.revisions + 1) as i64);
    let original_cut = lab.db.cold_verify_cut(&lab.store, &lab.domain).unwrap();
    assert_eq!(original_cut.through_commit_seq(), profile.revisions);
    assert_eq!(original_cut.historical_members(), profile.revisions + 1);
    println!(
        "CMD2_RESTORE_FIXTURE domain={} store={} cut={} historical_members={}",
        lab.domain,
        lab._root.0.display(),
        original_cut.log_digest().to_hex(),
        original_cut.historical_members()
    );
    println!(
        "CMD2_RESTORE_PROFILE {}",
        serde_json::json!({
            "revisions": profile.revisions, "payload_bytes": profile.payload_bytes,
            "committed_attempts": profile.revisions, "sealed_attempts": profile.revisions + 1,
            "current_subjects": 2, "historical_members": profile.revisions + 1,
            "historical_frame_bytes": historical_frame_bytes,
            "export_elapsed_ms": started.elapsed().as_millis(), "source_admission": false,
        })
    );
    if let Some(backup) = std::env::var_os("TOS_CMD2_BACKUP_ROOT") {
        use tos_command::backup_recovery::{BackupSelection, PgTool, backup_quiescent};
        let backup = PathBuf::from(backup);
        let tool = PathBuf::from(std::env::var_os("TOS_CMD2_PG_DUMP_PATH").unwrap());
        let sha = std::env::var("TOS_CMD2_PG_DUMP_SHA256").unwrap();
        let result = backup_quiescent(
            &BackupSelection {
                pg_url: &url,
                domain: &lab.domain,
                store_root: &lab._root.0,
                backup_root: &backup,
                tool: PgTool {
                    path: &tool,
                    sha256: &sha,
                },
                store_limits: limits(),
                quiescent_owner_confirmed: std::env::var("TOS_CMD2_BACKUP_QUIESCENT").unwrap()
                    == "owned-fixture-only",
            },
            Instant::now() + Duration::from_secs(120),
            &AtomicBool::new(false),
        )
        .unwrap();
        println!(
            "CMD2_BACKUP_RECEIPT {}",
            serde_json::to_string(&result).unwrap()
        );
    }
    std::mem::forget(lab);
}

/// Point this at a new database restored from the export's pg_dump and at an
/// independent copy of the exported STO root, not its original directory.
#[test]
#[ignore = "manual owner-managed PostgreSQL dump and independent STO copy drill"]
fn verify_cold_restored_fixture() {
    let started = Instant::now();
    let profile = RestoreFixtureProfile::selected();
    let url = database_url();
    let domain = std::env::var("TOS_CMD2_RESTORE_DOMAIN").expect("exported domain required");
    let store_path =
        std::env::var_os("TOS_CMD2_RESTORE_STORE").expect("independent STO copy required");
    if let Some(backup) = std::env::var_os("TOS_CMD2_RESTORE_BACKUP") {
        use tos_command::backup_recovery::{PgTool, RestoreSelection, restore_into_fresh};
        let backup = PathBuf::from(backup);
        let restored = PathBuf::from(&store_path);
        let receipt_sha = std::env::var("TOS_CMD2_BACKUP_RECEIPT_SHA256").unwrap();
        let tool = PathBuf::from(std::env::var_os("TOS_CMD2_PG_RESTORE_PATH").unwrap());
        let sha = std::env::var("TOS_CMD2_PG_RESTORE_SHA256").unwrap();
        let result = restore_into_fresh(
            &RestoreSelection {
                pg_url: &url,
                domain: &domain,
                backup_root: &backup,
                receipt_sha256: &receipt_sha,
                store_root: &restored,
                tool: PgTool {
                    path: &tool,
                    sha256: &sha,
                },
                store_limits: limits(),
                fresh_target_owner_confirmed: true,
            },
            Instant::now() + Duration::from_secs(120),
            &AtomicBool::new(false),
        )
        .unwrap();
        println!(
            "CMD2_RESTORE_RECEIPT {}",
            serde_json::to_string(&result).unwrap()
        );
    }
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
    for revision in 1..=profile.revisions {
        let payload = profile.payload(revision);
        let selected = db
            .cold_recover_exact(&store, &domain, "restore-A", revision)
            .expect("restored metadata selects every exact retained revision");
        let bytes = db
            .warm_read_selected(&store, &selected, profile.read_cap())
            .unwrap();
        assert_eq!(bytes, lab_record_bytes("restore-A", revision, &payload));
    }
    let companion = db
        .cold_recover_exact(&store, &domain, "restore-B", 1)
        .unwrap();
    assert_eq!(
        db.warm_read_selected(&store, &companion, 1024).unwrap(),
        lab_record_bytes("restore-B", 1, b"second compound member")
    );
    for revision in 3..=profile.revisions {
        let prepare = format!("restore-prepare-a{revision}");
        let fence = registered_fence(&url, &domain, prepare.as_bytes());
        assert!(
            matches!(
                store
                    .recover_attempt_fenced(prepare.as_bytes(), fence, 0)
                    .unwrap(),
                Some(AttemptRecovery::Sealed { .. })
            ),
            "each extra durable prepare intent survives restore"
        );
    }
    assert_eq!(scalar_count(&url, "current", &domain), 2);
    assert_eq!(
        scalar_count(&url, "history", &domain),
        (profile.revisions + 1) as i64
    );
    let cut = db.cold_verify_cut(&store, &domain).unwrap();
    assert_eq!(cut.through_commit_seq(), profile.revisions);
    assert_eq!(cut.historical_members(), profile.revisions + 1);
    db.seal_shadow_cut(&cut).unwrap();
    assert_eq!(db.published_seq(&domain).unwrap(), profile.revisions);
    let predecessor = db
        .cold_recover_exact(&store, &domain, "restore-A", 1)
        .unwrap();
    db.revoke_local(&domain).unwrap();
    assert!(matches!(
        db.warm_read_selected(&store, &predecessor, profile.read_cap()),
        Err(DurableError::Refused(_))
    ));
    println!(
        "CMD2_RESTORE_VERIFY_PROFILE {}",
        serde_json::json!({"revisions": profile.revisions,"payload_bytes": profile.payload_bytes,"verified_historical_members": profile.revisions + 1,"verify_elapsed_ms": started.elapsed().as_millis()})
    );
    println!(
        "CMD2_RESTORED domain={} cut={} historical_members={}",
        domain,
        cut.log_digest().to_hex(),
        cut.historical_members()
    );
}

#[test]
fn sigkill_after_seal_ready_and_commit_has_distinct_recovery() {
    let url = database_url();
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
    let url = database_url();
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
