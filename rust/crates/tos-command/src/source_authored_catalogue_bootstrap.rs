//! Mechanical initial metadata bootstrap over a genuinely completed native
//! candidate. The existing transaction owner issues the epoch; neither this
//! receipt nor its serialized form grants semantics, rights, or catalogue use.
//! The derived catalogue must subsequently be rendered under that ready epoch.
use super::{CreationFilesystem, owned, raw, walk, work_transaction as tx};
use crate::source_admission_spooled_candidate::{SpoolCandidate, SpoolLimits};
use crate::source_admission_spooled_index::{IndexView, SpoolIndexLimits};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use std::collections::BTreeSet;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::{io, path::Path, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue, RelativePath};

const OWNER_CAP: usize = 65_536;
const RECEIPT_CAP: usize = 65_536;
const RECEIPT: &str = "ToS/source-witnesses/bootstrap/native-catalogue-bootstrap.json";
// One absent->present file, no directories, no recovery/replay branch. The
// existing mover's hard raw/manifest caps remain unchanged. For this ONE file
// route there are six guarded mover edges, five retained-manifest checks and
// no directory/replay loops. A conservative 64 reads at (512KiB+64KiB sentinel)
// plus sixteen protected-owner rereads at (1MiB+64KiB) is <56MiB. Eight writes
// at 512KiB covers its six immutable/control/output writes. Reserve once in
// the original ledger; these are admitted upper bounds, not measured returns.
const TRANSPORT_READ_ENVELOPE: u64 = 56 * 1_048_576;
const TRANSPORT_WRITE_ENVELOPE: u64 = 4 * 1_048_576;
const TRANSPORT_STATE_ENVELOPE: usize = 32 * 1_048_576;

/// A refusal after a real installed bootstrap receipt preserves its physical
/// identity and observed publication head instead of reporting an empty root.
#[derive(Debug)]
pub(crate) struct BootstrapRefusal {
    cause: SourceCommandError,
    terminal_cause: Option<io::Error>,
    packet: Option<serde_json::Value>,
}
impl From<SourceCommandError> for BootstrapRefusal {
    fn from(cause: SourceCommandError) -> Self {
        Self {
            cause,
            terminal_cause: None,
            packet: None,
        }
    }
}
impl std::fmt::Display for BootstrapRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "authored catalogue bootstrap refused: {:?}", self.cause)
    }
}
impl std::error::Error for BootstrapRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.terminal_cause
            .as_ref()
            .map(|cause| cause as &dyn std::error::Error)
    }
}
impl BootstrapRefusal {
    pub(crate) fn with_terminal_checks(mut self, io_accounted: bool, cleanup: bool) -> Self {
        if let Some(packet) = self.packet.as_mut() {
            packet["terminal_io_accounting"] =
                serde_json::json!(if io_accounted { "passed" } else { "refused" });
            packet["workspace_cleanup"] =
                serde_json::json!(if cleanup { "passed" } else { "refused" });
        }
        self
    }
    pub(crate) fn with_candidate_io(
        mut self,
        usage: tos_source_store::PinnedSqliteIoSnapshot,
    ) -> Self {
        let packet = self.packet.get_or_insert_with(|| {
            serde_json::json!({
                "schema_version":"tos_authored_bootstrap_refusal_v1",
                "status":"BOOTSTRAP_REFUSED_BEFORE_CONFIRMED_PUBLICATION",
                "catalogue_complete":false, "grants_admission":false,
            })
        });
        packet["candidate_shared_io"] = serde_json::json!({
            "read_attempted_bytes":usage.read_attempted_bytes,
            "read_permitted_bytes":usage.read_permitted_bytes,
            "read_returned_bytes":usage.read_returned_bytes,
            "write_attempted_bytes":usage.write_attempted_bytes,
            "write_permitted_bytes":usage.write_permitted_bytes,
            "write_returned_bytes":usage.write_returned_bytes,
        });
        self
    }
    pub(crate) fn with_output_refused(mut self) -> Self {
        if let Some(packet) = self.packet.as_mut() {
            packet["terminal_output"] = serde_json::json!("refused");
        }
        self
    }
    pub(crate) fn after_publication(publication: serde_json::Value, cause: io::Error) -> Self {
        Self {
            cause: error("authored bootstrap committed before terminal refusal"),
            terminal_cause: Some(cause),
            packet: Some(serde_json::json!({
                "schema_version":"tos_authored_bootstrap_committed_refusal_v1",
                "status":"BOOTSTRAP_PUBLICATION_COMMITTED_TERMINAL_REFUSAL",
                "publication":publication,
                "catalogue_complete":false, "grants_admission":false,
            })),
        }
    }
    pub(crate) fn packet(&self) -> Option<&serde_json::Value> {
        self.packet.as_ref()
    }
}

fn error(reason: &'static str) -> SourceCommandError {
    SourceCommandError::Conflict(reason)
}
fn native(error: io::Error) -> SourceCommandError {
    let _ = error;
    SourceCommandError::Conflict("authored bootstrap completed candidate refused")
}

/// Construction is private to the genuine publisher below. This proves one
/// technical source transaction, not a generated catalogue or managed grant.
pub(crate) struct AuthoredBootstrapPublication {
    receipt_sha256: String,
    members: u64,
    source_bytes: u64,
    transaction: tx::WorkTransportResult,
}
impl AuthoredBootstrapPublication {
    pub(crate) fn value(&self) -> SourceCommandResult<serde_json::Value> {
        let value = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_authored_catalogue_bootstrap_publication_v1"),
            ),
            ("receipt_sha256", cmd::string(&self.receipt_sha256)),
            ("members", cmd::number(self.members)),
            ("source_bytes", cmd::number(self.source_bytes)),
            ("receipt_path", cmd::string(RECEIPT)),
            (
                "publication_token",
                cmd::field(&self.transaction.publication, "token")?.clone(),
            ),
            (
                "publication_generation",
                cmd::field(&self.transaction.publication, "generation")?.clone(),
            ),
            (
                "transaction_id",
                cmd::string(&self.transaction.transaction_id),
            ),
            (
                "manifest_sha256",
                cmd::string(&self.transaction.manifest_sha256),
            ),
            ("catalogue_complete", JsonValue::Bool(false)),
            ("grants_admission", JsonValue::Bool(false)),
        ]);
        serde_json::from_slice(&cmd::canonical(&value)?)
            .map_err(|_| error("authored bootstrap publication encoding"))
    }
}

/// Compare the exact selected raw/dependency closure against the actual held
/// destination namespace. Every body read reserves its size plus one EOF byte
/// before IO, and a limited reader refuses growth inside that same reservation.
/// Unselected destination bytes acquire no proof from this selected receipt.
fn verify_destination(
    fs: &CreationFilesystem,
    candidate: &SpoolCandidate<'_>,
    index: &IndexView<'_>,
    journal_members: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    index.verify_candidate().map_err(native)?;
    verify_private_root(fs)?;
    fs.verify_protected_configuration(deadline, cancelled)?;
    let fence = index.fence();
    let mut after = None;
    let mut count = 0u64;
    let mut total = 0u64;
    let mut present = [false; 11];
    let mut identity_after = None;
    const BASENAMES: [&str; 11] = [
        "agent.json",
        "place.json",
        "organization.json",
        "work.json",
        "expression.json",
        "edition.json",
        "collection.json",
        "item.json",
        "link.json",
        "artifact-witness.json",
        "composite-witness.json",
    ];
    while let Some(member) = candidate
        .member_after_bounded(after.as_ref(), 65_536)
        .map_err(native)?
    {
        super::active(deadline, cancelled)?;
        let path = member.path.as_str();
        if path == RECEIPT || path == "ToS/source-witnesses/.metadata-publication.json" {
            return Err(error(
                "authored bootstrap requires a new unretagged candidate",
            ));
        }
        let (parent, leaf) = path
            .rsplit_once('/')
            .ok_or_else(|| error("authored bootstrap selected relative member"))?;

        let directory = walk(&fs.root, parent, fs.uid)?;
        candidate
            .debit_read(
                member
                    .size_bytes
                    .checked_add(1)
                    .ok_or_else(|| error("authored bootstrap read reservation overflow"))?,
            )
            .map_err(native)?;
        let mut fd = tos_fd_open::open_regular_at(&directory, Path::new(leaf))
            .map_err(|_| error("authored bootstrap selected member unavailable"))?;
        let metadata = owned(&fd, fs.uid, false)?;
        if metadata.len() != member.size_bytes || metadata.mode() & 0o7777 != member.mode {
            return Err(error("authored bootstrap destination member size differs"));
        }
        let mut digest = Digest256Hasher::new();
        let mut returned = 0u64;
        let mut limited = (&mut fd).take(member.size_bytes + 1);
        let mut buffer = [0u8; 65_536];
        loop {
            super::active(deadline, cancelled)?;
            let n = limited
                .read(&mut buffer)
                .map_err(|_| error("authored bootstrap bounded destination read"))?;
            candidate
                .record_bootstrap_read_returned(n as u64)
                .map_err(native)?;
            if n == 0 {
                break;
            }
            returned = returned
                .checked_add(n as u64)
                .ok_or_else(|| error("authored bootstrap returned byte overflow"))?;
            if returned > member.size_bytes {
                return Err(error(
                    "authored bootstrap destination grew beyond reservation",
                ));
            }
            digest.update(&buffer[..n]);
        }
        let after_read = owned(&fd, fs.uid, false)?;
        if super::stamp(&metadata) != super::stamp(&after_read)
            || metadata.mode() != after_read.mode()
        {
            return Err(error("authored bootstrap destination changed during read"));
        }
        if returned != member.size_bytes || digest.finalize() != member.sha256 {
            return Err(error("authored bootstrap destination raw closure differs"));
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| error("authored bootstrap member count overflow"))?;
        total = total
            .checked_add(member.size_bytes)
            .ok_or_else(|| error("authored bootstrap raw total overflow"))?;
        after = Some(member.path);
    }
    // Presence comes from the genuine completed identity index, not arbitrary
    // similarly named files. Its producer evaluate_spooled_admission already
    // ran compare_spooled_candidate -> prepare_candidate_source_catalog_plan_observed
    // with the actual schema worker, required issues.empty/profiles.complete and
    // authenticated fresh EOF before NativeAdmissionComplete could be created.
    // This original candidate proof is not a catalogue bound to the NEW epoch.
    while let Some((id, path)) = index
        .identities_after(identity_after.as_deref())
        .map_err(native)?
    {
        if path.as_str().starts_with("ToS/source-witnesses/") {
            if let Some(leaf) = path.as_str().rsplit('/').next() {
                if let Some(i) = BASENAMES.iter().position(|name| *name == leaf) {
                    present[i] = true;
                }
            }
        }
        identity_after = Some(id);
    }
    let mut census = DestinationCensus::default();
    let tos = super::child(&fs.root, "ToS")?;
    census_directory(
        &tos,
        "ToS",
        fs,
        candidate,
        0,
        &mut census,
        journal_members,
        deadline,
        cancelled,
    )?;
    if census.members != fence.membership.count {
        return Err(error("authored bootstrap exhaustive destination differs"));
    }
    if count != fence.membership.count
        || total != fence.source_bytes
        || present.iter().any(|value| !value)
    {
        return Err(error(
            "authored bootstrap destination selected closure incomplete",
        ));
    }
    index.verify_candidate().map_err(native)?;
    verify_private_root(fs)?;
    fs.verify_protected_configuration(deadline, cancelled)
}

/// Only the native-v4 caller can supply IndexView: its retained private
/// NativeAdmissionComplete exists only after every FND phase and terminal fence.
/// The protected owner selects a private destination and this fixed new metadata
/// path, never caller-supplied transaction plans or serialized completion tokens.
pub(crate) fn publish(
    owner_path: &Path,
    candidate: &SpoolCandidate<'_>,
    index: &IndexView<'_>,
    candidate_limits: SpoolLimits,
    index_limits: SpoolIndexLimits,
    output_cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    invocation_fence: &mut dyn FnMut() -> SourceCommandResult<()>,
) -> Result<AuthoredBootstrapPublication, BootstrapRefusal> {
    check_terminal_output(output_cap)?;
    index
        .verify_publication_receiver(candidate, index_limits)
        .map_err(native)?;
    if index.fence().base_revision.is_some() || index.fence().retirement_count != 0 {
        return Err(error("authored bootstrap requires an initial complete candidate").into());
    }
    candidate
        .check_state(TRANSPORT_STATE_ENVELOPE)
        .map_err(native)?;
    candidate
        .debit_read((OWNER_CAP + 65_536) as u64)
        .map_err(native)?;
    let (fs, configuration) = CreationFilesystem::select_protected_native_owner_bounded(
        owner_path, OWNER_CAP, deadline, cancelled,
    )?;
    let config = cmd::parse(&configuration)?;
    cmd::exact_keys(
        &config,
        &[
            "schema_version",
            "uid",
            "expires_at",
            "source_root",
            "principal_id",
            "authority_ref",
            "candidate_batch_sha256",
            "candidate_validator_sha256",
            "candidate_membership_sha256",
            "candidate_members",
            "expected_publication_token",
            "expected_publication_generation",
        ],
    )?;
    let fence = index.fence();
    if cmd::text(&config, "schema_version")? != "tos_authored_catalogue_bootstrap_owner_v1"
        || cmd::text(&config, "principal_id")?.trim().is_empty()
        || cmd::text(&config, "authority_ref")?.trim().is_empty()
        || cmd::text(&config, "candidate_batch_sha256")? != fence.batch_sha256.to_prefixed()
        || cmd::text(&config, "candidate_validator_sha256")? != fence.validator_sha256.to_prefixed()
        || cmd::text(&config, "candidate_membership_sha256")?
            != fence.membership.digest.to_prefixed()
        || cmd::integer(&config, "candidate_members")? != fence.membership.count
        || cmd::field(&config, "expected_publication_token")? != &JsonValue::Null
        || cmd::integer(&config, "expected_publication_generation")? != 0
    {
        return Err(error("authored bootstrap exact protected initial owner selection").into());
    }
    // Constructor and parsing overlap, candidate/index native state and this
    // transport reserve remain bounded by the original source admission state.
    if TRANSPORT_STATE_ENVELOPE > candidate_limits.candidate.max_state_bytes {
        return Err(error("authored bootstrap original state reservation").into());
    }
    candidate
        .debit_read(TRANSPORT_READ_ENVELOPE)
        .map_err(native)?;
    candidate
        .debit_write(TRANSPORT_WRITE_ENVELOPE)
        .map_err(native)?;
    let lock = tx::WorkCorpusFence::hold(&fs, deadline, cancelled)?;
    let epoch = tx::PublicationSnapshot::select(&fs, deadline, cancelled)?;
    if epoch.token.is_some() || epoch.generation != 0 {
        return Err(error("authored bootstrap destination already has a publication").into());
    }
    invocation_fence()?;
    verify_destination(&fs, candidate, index, &BTreeSet::new(), deadline, cancelled)?;
    let authorization = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_authored_catalogue_bootstrap_authorization_v1"),
        ),
        ("principal_id", cmd::field(&config, "principal_id")?.clone()),
        (
            "authority_ref",
            cmd::field(&config, "authority_ref")?.clone(),
        ),
        (
            "owner_configuration_sha256",
            cmd::string(&Digest256::of_bytes(&configuration).to_prefixed()),
        ),
        (
            "candidate_batch_sha256",
            cmd::string(&fence.batch_sha256.to_prefixed()),
        ),
        (
            "candidate_validator_sha256",
            cmd::string(&fence.validator_sha256.to_prefixed()),
        ),
        (
            "candidate_membership_sha256",
            cmd::string(&fence.membership.digest.to_prefixed()),
        ),
    ]);
    let receipt = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_authored_catalogue_bootstrap_receipt_v1"),
        ),
        ("origin", authorization.clone()),
        ("members", cmd::number(fence.membership.count)),
        ("source_bytes", cmd::number(fence.source_bytes)),
        ("scope", cmd::string("validated_candidate_members_only")),
        ("catalogue_complete", JsonValue::Bool(false)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let mut receipt_bytes = cmd::canonical(&receipt)?;
    receipt_bytes.push(b'\n');
    if receipt_bytes.len() > RECEIPT_CAP {
        return Err(error("authored bootstrap receipt byte reservation").into());
    }
    let transaction_id = Digest256::of_bytes(&receipt_bytes).to_prefixed();
    let retained_id = transaction_id.clone();
    let expected_receipt_sha = transaction_id.clone();
    let plan = tx::WorkPlan {
        transaction_id,
        authorization: authorization.clone(),
        item_path_profile: None,
        files: vec![tx::SelectedFile {
            path: RelativePath::parse(RECEIPT)
                .map_err(|_| error("authored bootstrap fixed path"))?,
            before: None,
            after: Some(receipt_bytes),
        }],
        new_directories: Vec::new(),
        source_readset: None,
        source_successor: None,
    };
    tx::validate_plan(&plan)?;
    let transaction_result = lock.apply_initial(
        plan,
        &epoch,
        |summary, guard| {
            // Reauthenticate actual protected authority and every selected body at
            // each original mover edge; callback data are never a serialized grant.
            if !cmd::same(cmd::field(summary, "authorization")?, &authorization)? {
                return Err(error("authored bootstrap retained authority differs").into());
            }
            cmd::validate_expiry(
                cmd::text(&config, "expires_at")?,
                &crate::source_serialization::instant()?,
            )?;
            invocation_fence()?;
            verify_destination(
                &fs,
                candidate,
                index,
                guard.journal_members,
                deadline,
                cancelled,
            )
        },
        deadline,
        cancelled,
    );
    let transaction = match transaction_result {
        Ok(transaction) => transaction,
        Err(cause) => {
            let observation = (|| -> SourceCommandResult<serde_json::Value> {
                let parent = walk(&fs.root, "ToS/source-witnesses/bootstrap", fs.uid)?;
                let mut fd = tos_fd_open::open_regular_at(
                    &parent,
                    Path::new("native-catalogue-bootstrap.json"),
                )
                .map_err(|_| error("authored bootstrap receipt unavailable after refusal"))?;
                let metadata = owned(&fd, fs.uid, false)?;
                let bytes = raw(&mut fd, RECEIPT_CAP, deadline, cancelled)?;
                if Digest256::of_bytes(&bytes).to_prefixed() != plan_receipt_sha(&receipt)? {
                    return Err(error("authored bootstrap post-refusal receipt differs").into());
                }
                let witness = walk(&fs.root, "ToS/source-witnesses", fs.uid)?;
                let head = tx::read_at(
                    &witness,
                    ".metadata-publication.json",
                    fs.uid,
                    8192,
                    deadline,
                    cancelled,
                )?
                .map(|body| cmd::parse(&body))
                .transpose()?;
                if let Some(value) = &head {
                    tx::state(value)?;
                }
                let head = head
                    .map(|value| cmd::canonical(&value))
                    .transpose()?
                    .map(|body| serde_json::from_slice::<serde_json::Value>(&body))
                    .transpose()
                    .map_err(|_| error("authored bootstrap refusal head encoding"))?;
                Ok(serde_json::json!({
                    "schema_version":"tos_authored_bootstrap_installed_refusal_v1",
                    "status":"BOOTSTRAP_RECEIPT_INSTALLED_PUBLICATION_REFUSED",
                    "receipt_path":RECEIPT,
                    "receipt_sha256":Digest256::of_bytes(&bytes).to_prefixed(),
                    "receipt_dev":metadata.dev(), "receipt_ino":metadata.ino(),
                    "publication_token":head.as_ref().and_then(|h| h.get("token")),
                    "publication_generation":head.as_ref().and_then(|h| h.get("generation")),
                    "catalogue_complete":false, "grants_admission":false,
                }))
            })()
            .ok();
            let packet = observation.unwrap_or_else(|| {
                serde_json::json!({
                    "schema_version":"tos_authored_bootstrap_progress_refusal_v1",
                    "status":"BOOTSTRAP_TRANSACTION_PROGRESS_UNCONFIRMED",
                    "transaction_id":retained_id,
                    "receipt_path":RECEIPT,
                    "expected_receipt_sha256":expected_receipt_sha,
                    "catalogue_complete":false, "grants_admission":false,
                })
            });
            return Err(BootstrapRefusal {
                cause,
                terminal_cause: None,
                packet: Some(packet),
            });
        }
    };
    let journal_home = format!(
        "ToS/source-witnesses/.metadata-transactions/{}",
        &retained_id[7..]
    );
    let terminal_journal = BTreeSet::from([
        format!("{journal_home}/manifest.json"),
        format!("{journal_home}/{}.blob", &expected_receipt_sha[7..]),
        format!("{journal_home}/completion.json"),
    ]);
    let terminal = invocation_fence().and_then(|_| {
        verify_destination(
            &fs,
            candidate,
            index,
            &terminal_journal,
            deadline,
            cancelled,
        )?;
        let current = tx::PublicationSnapshot::select(&fs, deadline, cancelled)?;
        if current.token.as_deref() != Some(cmd::text(&transaction.publication, "token")?)
            || current.generation != cmd::integer(&transaction.publication, "generation")?
        {
            return Err(error("authored bootstrap terminal publication changed"));
        }
        Ok(())
    });
    if let Err(cause) = terminal {
        let completed = AuthoredBootstrapPublication {
            receipt_sha256: expected_receipt_sha.clone(),
            members: fence.membership.count,
            source_bytes: fence.source_bytes,
            transaction,
        };
        let packet = completed.value().map_err(BootstrapRefusal::from)?;
        return Err(BootstrapRefusal {
            cause,
            terminal_cause: None,
            packet: Some(serde_json::json!({
                "schema_version":"tos_authored_bootstrap_committed_refusal_v1",
                "status":"BOOTSTRAP_PUBLICATION_COMMITTED_TERMINAL_REFUSAL",
                "publication":packet, "catalogue_complete":false, "grants_admission":false,
            })),
        });
    }
    Ok(AuthoredBootstrapPublication {
        receipt_sha256: expected_receipt_sha,
        members: fence.membership.count,
        source_bytes: fence.source_bytes,
        transaction,
    })
}

fn plan_receipt_sha(receipt: &JsonValue) -> SourceCommandResult<String> {
    let mut raw = cmd::canonical(receipt)?;
    raw.push(b'\n');
    Ok(Digest256::of_bytes(&raw).to_prefixed())
}

fn verify_private_root(fs: &CreationFilesystem) -> SourceCommandResult<()> {
    let named = tos_fd_open::open_absolute_directory(&fs.root_path)
        .map_err(|_| error("authored bootstrap private root unavailable"))?;
    if owned(&named, fs.uid, true)?.mode() & 0o7777 != 0o700
        || owned(&fs.root, fs.uid, true)?.mode() & 0o7777 != 0o700
    {
        return Err(error("authored bootstrap private root protection changed"));
    }
    Ok(())
}

#[derive(Default)]
struct DestinationCensus {
    entries: usize,
    directories: usize,
    members: u64,
}

// Exact cooperating transaction protocol paths are separate from authored
// input membership. The retained transaction owner verifies its own journal;
// every other physical file, including unexpected dotfiles, must be selected.
fn census_directory(
    directory: &std::fs::File,
    prefix: &str,
    fs: &CreationFilesystem,
    candidate: &SpoolCandidate<'_>,
    depth: usize,
    census: &mut DestinationCensus,
    journal_members: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    super::active(deadline, cancelled)?;
    if depth > 64 || prefix.len() > 4096 || census.directories >= 8192 {
        return Err(error("authored bootstrap destination traversal bound"));
    }
    census.directories += 1;
    let before = owned(directory, fs.uid, true)?;
    // RawDir uses the actual held FD and this caller-owned fixed buffer. It
    // cannot allocate/grow a hidden directory buffer or reopen the pathname.
    let mut directory_buffer = [std::mem::MaybeUninit::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(directory, &mut directory_buffer);
    loop {
        if entries.is_buffer_empty() {
            candidate.debit_read(8192).map_err(native)?;
        }
        let Some(entry) = entries.next() else {
            break;
        };
        let entry = entry.map_err(|_| error("authored bootstrap held directory enumeration"))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| error("authored bootstrap non UTF8 destination entry"))?;
        if matches!(name, "." | "..") {
            continue;
        }
        if name.len() > 255 {
            return Err(error("authored bootstrap destination name bound"));
        }
        let kind = entry.file_type();
        let name = name.to_owned();
        super::active(deadline, cancelled)?;
        if census.entries >= 12288 {
            return Err(error("authored bootstrap destination entry bound"));
        }
        census.entries += 1;
        let path = format!("{prefix}/{name}");
        let relative = RelativePath::parse(&path)
            .map_err(|_| error("authored bootstrap unsafe destination path"))?;
        if kind == rustix::fs::FileType::Directory {
            let held = super::child(directory, &name)?;
            if path == "ToS/source-witnesses/.metadata-transactions"
                || path.starts_with("ToS/source-witnesses/.metadata-transactions/")
            {
                if owned(&held, fs.uid, true)?.mode() & 0o7777 != 0o700
                    || !journal_members
                        .iter()
                        .any(|member| member.starts_with(&format!("{path}/")))
                {
                    return Err(error("authored bootstrap unselected transaction directory"));
                }
            }
            census_directory(
                &held,
                &path,
                fs,
                candidate,
                depth + 1,
                census,
                journal_members,
                deadline,
                cancelled,
            )?;
        } else if kind == rustix::fs::FileType::RegularFile {
            if path == RECEIPT
                || path == "ToS/source-witnesses/.metadata-publication.json"
                || path == "ToS/source-witnesses/.historical-create.writer.lock"
            {
                // The transaction holds and authenticates these exact protocol
                // files. They grant no authored candidate membership.
                continue;
            }
            if journal_members.contains(&path) {
                continue;
            }
            if candidate
                .member_bounded(&relative, 65_536)
                .map_err(native)?
                .is_none()
            {
                return Err(error("authored bootstrap unselected destination file"));
            }
            census.members = census
                .members
                .checked_add(1)
                .ok_or_else(|| error("authored bootstrap census overflow"))?;
        } else {
            return Err(error(
                "authored bootstrap destination special file or symlink",
            ));
        }
    }
    let after = owned(directory, fs.uid, true)?;
    if super::stamp(&before) != super::stamp(&after) || before.mode() != after.mode() {
        return Err(error("authored bootstrap destination directory changed"));
    }
    Ok(())
}

// Compute the largest compact wire form before any publication. Hashes are
// fixed sha256 strings, counters are u64, and owner-controlled strings never
// enter the wire. Installed/unconfirmed refusal forms are strictly smaller.
fn check_terminal_output(cap: usize) -> SourceCommandResult<()> {
    let hash = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let success = serde_json::json!({
        "schema_version":"tos_authored_catalogue_bootstrap_publication_v1",
        "receipt_sha256":hash, "members":u64::MAX, "source_bytes":u64::MAX,
        "receipt_path":RECEIPT, "publication_token":hash,
        "publication_generation":u64::MAX, "transaction_id":hash,
        "manifest_sha256":hash, "catalogue_complete":false, "grants_admission":false,
    });
    let refusal = serde_json::json!({
        "schema_version":"tos_authored_bootstrap_committed_refusal_v1",
        "status":"BOOTSTRAP_PUBLICATION_COMMITTED_TERMINAL_REFUSAL",
        "publication":success, "catalogue_complete":false, "grants_admission":false,
        "terminal_io_accounting":"refused", "workspace_cleanup":"refused",
        "terminal_output":"refused",
        "candidate_shared_io": {
            "read_attempted_bytes":u64::MAX, "read_permitted_bytes":u64::MAX,
            "read_returned_bytes":u64::MAX, "write_attempted_bytes":u64::MAX,
            "write_permitted_bytes":u64::MAX, "write_returned_bytes":u64::MAX,
        },
    });
    let bytes = serde_json::to_vec(&refusal)
        .map_err(|_| error("authored bootstrap compact output encoding"))?;
    if bytes.len().checked_add(1).is_none_or(|n| n > cap) {
        return Err(error(
            "authored bootstrap compact terminal output reservation",
        ));
    }
    Ok(())
}
