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
    fn new(
        parent: &tos_command::source_managed_selection::ManagedAgentSelectedParent,
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
    use std::collections::BTreeMap;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_command::{CommandContext, SourceFile};
    use tos_command::source_creation::prepare_source_creation_from_captures;
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
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
            .env("GIT_NO_REPLACE_OBJECTS", "1");
    }
    let url = database_url();
    let mut lab = Lab::new(&url);
    let root = ScratchRoot::new();
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
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
    let attempts = packages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut registration_worker = new_worker(&cut);
            let attempt = lab
                .db
                .register_source_creation(
                    &lab.store,
                    &initial.cohort,
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
                &initial.cohort,
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
                &initial.cohort,
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
    assert!(verified.cohort.epoch() > initial.cohort.epoch());
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
    let binding_for = |proof: &tos_compiler::ManagedSourceProofV1| tos_compiler::SourceBinding {
        owner_profile: "synthetic-private-managed-agent-consumer".into(),
        source_cut: proof.initial_export_source_revision.clone(),
        through_commit_seq: proof.generation.through_commit_seq,
        membership_root: proof.generation.current_membership_sha256.clone(),
        index_generation: proof.generation.installed_generation_sha256.clone(),
        route_map_version: "private-managed-agent-v1".into(),
        reader_abi: "private-managed-agent-v1".into(),
        projection_root_sha256: proof.generation.inventory_projection_sha256.clone(),
        complete: true,
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
    let mut initial_carriers = Vec::new();
    let initial_selected =
        tos_command::source_managed_selection::prepare_managed_agent_selected_parent(
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
                let candidate = tos_compiler::render_source_bibliographic_plan(
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
                let basis = tos_compiler::KnowledgeSourceBasis::ManagedCurrent {
                    proof: proof.clone(),
                };
                tos_compiler::managed_source::prepare_managed_agent_selected_model(
                    &plan,
                    &navigation,
                    &validator,
                    proof,
                    &root.0.join("initial-agent-selected.sqlite"),
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
        )
        .unwrap();
    assert!(!initial_carriers.is_empty());
    drop(initial_carriers); // the next selected model has its own finite carrier set
    let current = initial_selected.generation();
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
    let (current_package, second_receipt, _, successor) = reopened_db
        .execute_managed_agent_creation_from_captures(
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
        )
        .unwrap();
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
    let mut successor_carriers = Vec::new();
    let second_selected =
        tos_command::source_managed_selection::prepare_managed_agent_selected_successor(
            &mut reopened_db,
            &reopened_store,
            &initial_selected,
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
                let basis = tos_compiler::KnowledgeSourceBasis::ManagedCurrent {
                    proof: proof.clone(),
                };
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
                    &root.0.join("successor-agent-selected.sqlite"),
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
        second_selected.source_proof().generation.through_commit_seq,
        second_receipt.commit_seq
    );
    assert_eq!(
        second_selected.selection_expectation().descriptor_sha256,
        descriptor_digest.to_hex()
    );
    assert_ne!(
        second_selected.selection_expectation().model_sha256,
        initial_selected.selection_expectation().model_sha256
    );
    assert_eq!(
        second_selected.source_catalog_root_sha256(),
        initial_selected.source_catalog_root_sha256()
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
            assert_eq!(packet["source_basis"]["kind"], "managed_current");
            assert_eq!(
                packet["managed_source_root_sha256"],
                second_selected.source_proof().root_sha256().unwrap()
            );
            assert!(packet.get("source_revision").is_none());
            if schema == "tos_knowledge_node_packet_v2" {
                assert_eq!(packet["matches"].as_array().unwrap().len(), 1);
                let attributes = &packet["matches"][0]["attributes"];
                let body: serde_json::Value =
                    serde_json::from_slice(&current_package.files()[second_path.as_str()]).unwrap();
                let form_path = second_path
                    .as_str()
                    .strip_suffix(".json")
                    .unwrap()
                    .to_owned()
                    + ".human-forms.json";
                let set: serde_json::Value =
                    serde_json::from_slice(&current_package.files()[&form_path]).unwrap();
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
    drop(initial);
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
    drop(lab);
    let recovered_root = ScratchRoot::new();
    copy_store_tree(&copied.0, &recovered_root.0);
    let recovered_store = SegmentStore::open_existing(&recovered_root.0, limits()).unwrap();
    let mut recovered_db = DurablePgCoordinator::connect(&url).unwrap();
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
    // Real consumed keyset queries must have a matching physical path.
    // First record ordinary plans. Then disable sequential scan/sort only
    // for an eligibility check: this is no runtime performance measurement.
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
        assert!(
            plan.contains(index),
            "the exact source seek has no matching index: {plan}"
        );
        assert!(
            !plan.lines().any(|line| line.contains("Sort")),
            "the source seek needs a whole sort: {plan}"
        );
    }
    planner_tx.commit().unwrap();
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
    for (name, expected) in &expected_original_files {
        let path = RelativePath::parse(&format!(
            "ToS/source-witnesses/agents/synthetic-durable-current/{name}"
        ))
        .unwrap();
        let retained = current_reopened
            .read_current_member(
                &mut recovered_db,
                &recovered_store,
                &path,
                8_388_608,
                deadline,
                &cancelled,
            )
            .unwrap();
        let owned_metadata = current_reopened
            .member(
                &mut recovered_db,
                &recovered_store,
                &path,
                deadline,
                &cancelled,
            )
            .unwrap()
            .unwrap();
        assert_eq!(owned_metadata, retained.metadata);
        assert_eq!(&retained.raw, expected, "process-cold original file {name}");
        assert_eq!(retained.current_generation, expected_current_head);
    }
    let (second_replayed, _) = recovered_db
        .recover_committed_managed_agent_creation(
            &recovered_store,
            current_reopened.cohort(),
            b"agent-current-second",
            &current_filesystem,
            &cut,
            &software,
            &components,
            &mut new_worker(&cut),
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
                &mut new_worker(&cut),
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
                    &mut new_worker(&cut),
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
    }
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
                &mut new_worker(&cut),
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
    let original_owner_raw = fs::read(&current_owner).unwrap();
    let mut changed_owner: serde_json::Value = serde_json::from_slice(&original_owner_raw).unwrap();
    changed_owner["principal_id"] = serde_json::json!("software:changed-owner-after-process-loss");
    fs::write(&current_owner, canonical(&changed_owner)).unwrap();
    let changed_filesystem =
        CreationFilesystem::select_isolated(&isolated, &current_owner, deadline, &cancelled)
            .unwrap();
    assert!(
        recovered_db
            .recover_committed_managed_agent_creation(
                &recovered_store,
                current_reopened.cohort(),
                b"agent-current-second",
                &changed_filesystem,
                &cut,
                &software,
                &components,
                &mut new_worker(&cut),
                contract_digest(),
                0,
                0,
                "private-job",
                1,
                deadline,
                &cancelled,
            )
            .is_err(),
        "original capture cannot override changed current protected owner"
    );
    fs::write(&current_owner, &original_owner_raw).unwrap();
    assert_eq!(
        recovered_db.head_seq(&recovery_domain).unwrap(),
        expected_current_head
    );
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

/// Run this exact ignored test alone, then copy its reported sealed store and
/// pg_dump the quiescent synthetic PostgreSQL database. The intentional leak
/// keeps this one fixture on disk after the test process exits for restore.
#[test]
#[ignore = "manual owner-managed PostgreSQL dump and independent STO copy drill"]
fn export_cold_restore_fixture() {
    let url = database_url();
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
    let url = database_url();
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
