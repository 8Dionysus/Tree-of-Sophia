//! Protected native entry for the initial PostgreSQL/STO source import.
//!
//! The owner selects one immutable source cut and software capture. Optional
//! schema initialization is restricted to an absent CMD.2 namespace; this route
//! creates only an explicitly selected empty store/domain and never grants
//! source admission or publication authority.

use super::{absolute, capped, digest, exact, number, read_absolute, text};
use crate::source_cohort::BootstrapJobMode;
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use crate::source_serialization::ExecutableObservation;
use crate::{DurableError, DurablePgCoordinator, DurableResult};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonLimits, RelativePath, SourceRevision};
use tos_segment_store::{SegmentLimits, SegmentStore};
use tos_source_store::{
    CorpusReader, CutReadLimits, ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
    SoftwareComponentSelectionV1, SourceMembershipV1,
};

const MAX_REQUEST: usize = 1_048_576;
const MAX_DOMAIN_BYTES: usize = 1_024;
const MAX_JOB_ID_BYTES: usize = 256;
const MAX_IMPORT_MEMBERS: u64 = 4_096;
const MAX_IMPORT_BYTES: u64 = 33_554_432;
const MAX_IMPORT_MEMBER_BYTES: u64 = 8_388_608;
const OWNER_SCHEMA: &str = "tos_local_pg_source_bootstrap_owner_v1";
const INVOCATION_SCHEMA: &str = "tos_local_pg_source_bootstrap_invocation_v1";
const RESULT_SCHEMA: &str = "tos_local_pg_source_bootstrap_result_v1";

fn exact_optional(value: &Value, required: &[&str], optional: &[&str]) -> SourceCommandResult<()> {
    let object = value
        .as_object()
        .ok_or(SourceCommandError::Invalid("PostgreSQL bootstrap object"))?;
    if object.len() < required.len()
        || object.len() > required.len() + optional.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err(SourceCommandError::Invalid("PostgreSQL bootstrap fields"));
    }
    Ok(())
}

fn optional_bool(value: &Value, key: &str) -> SourceCommandResult<bool> {
    match value.get(key) {
        None => Ok(false),
        Some(Value::Bool(selected)) => Ok(*selected),
        Some(_) => Err(SourceCommandError::Invalid("PostgreSQL bootstrap choice")),
    }
}

pub(super) struct SourceSelection {
    pub(super) domain: String,
    pub(super) contract: Digest256,
    pub(super) store_root: PathBuf,
    pub(super) store_limits: SegmentLimits,
    pub(super) job_id: String,
    pub(super) job_fence: u64,
    pub(super) job_mode: BootstrapJobMode,
    pub(super) initialize_schema: bool,
    pub(super) source_revision: SourceRevision,
    pub(super) membership: SourceMembershipV1,
}

struct ImportBudgets {
    cut: CutReadLimits,
    max_members: u64,
    max_total_bytes: u64,
    max_member_bytes: u64,
}

#[derive(Clone, Copy)]
pub(super) struct SoftwareInputLimits {
    pub max_members: u64,
    pub max_total_bytes: u64,
    pub max_member_bytes: u64,
}

/// Common protected preparation shared by legacy and streamed bootstrap.
/// The source cut remains outside this context: streamed callers anchor it by
/// exact revision and retain their own bounded reader instead of materializing
/// every source member here.
pub(super) struct ProtectedBootstrapContext {
    pub selection: SourceSelection,
    pub owner_path: PathBuf,
    pub owner_raw: Vec<u8>,
    pub owner_fs: CreationFilesystem,
    pub request_raw: Vec<u8>,
    pub context: cmd::CommandContext,
    pub software: SoftwareCaptureReader,
    pub components: SoftwareComponentSelectionV1,
    pub expected_executable: Digest256,
    pub runtime_observation: ExecutableObservation,
}

impl ProtectedBootstrapContext {
    pub(super) fn owner_check(
        &self,
        invocation_path: &Path,
        invocation_raw: &[u8],
        uid: u32,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<()> {
        if read_absolute(
            invocation_path,
            uid,
            true,
            invocation_raw.len(),
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)?
            != invocation_raw
        {
            return Err(DurableError::Conflict(
                "bootstrap invocation changed during import",
            ));
        }
        let (current_owner, current_raw) =
            CreationFilesystem::select_protected_native_owner_at_root(
                &self.owner_path,
                &self.selection.store_root,
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?;
        if current_raw != self.owner_raw {
            return Err(DurableError::Conflict(
                "bootstrap owner changed during import",
            ));
        }
        if current_owner.protected_root_identity() != self.owner_fs.protected_root_identity() {
            return Err(DurableError::Conflict(
                "bootstrap owner root changed during import",
            ));
        }
        current_owner
            .current_context(&self.context, deadline, cancelled)
            .map_err(DurableError::Source)?;
        if self
            .runtime_observation
            .current_digest(deadline, cancelled)
            .map_err(DurableError::Source)?
            != self.expected_executable
        {
            return Err(DurableError::Conflict(
                "bootstrap executable changed during import",
            ));
        }
        Ok(())
    }
}

pub(super) fn prepare_protected_bootstrap(
    invocation: &Value,
    input: impl Read,
    uid: u32,
    software_limits: SoftwareInputLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ProtectedBootstrapContext> {
    let selection = selected_source(invocation)?;
    let _ = absolute(text(invocation, "native_executable")?)?;
    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let expected_executable = digest(text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native bootstrap executable identity",
        ));
    }

    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    let owner_value = value(&cmd::parse(&owner_raw)?)?;
    check_owner_scope(&owner_value, &selection.store_root, &selection)?;

    let request_raw = request_bytes(input)?;
    let request_value = value(&cmd::parse(&request_raw)?)?;
    exact(&request_value, &["schema_version", "operation"])?;
    if text(&request_value, "schema_version")? != "tos_local_source_command_v1"
        || text(&request_value, "operation")? != "source.bootstrap"
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL bootstrap request operation",
        ));
    }

    let (software, components) = selected_software(
        invocation,
        software_limits.max_total_bytes,
        deadline,
        cancelled,
    )?;
    let mut files = Vec::with_capacity(components.members().count());
    let mut context_bytes = 0u64;
    for member in components.members() {
        if files.len() >= usize::try_from(software_limits.max_members).unwrap_or(usize::MAX)
            || member.size_bytes > software_limits.max_member_bytes
        {
            return Err(SourceCommandError::Invalid(
                "selected bootstrap software member budget",
            ));
        }
        context_bytes = context_bytes
            .checked_add(member.size_bytes)
            .filter(|bytes| *bytes <= software_limits.max_total_bytes)
            .ok_or(SourceCommandError::Invalid(
                "selected bootstrap software byte budget",
            ))?;
        let raw = software
            .read_selected_component(
                &components,
                &member.path,
                software_limits.max_member_bytes,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("selected bootstrap software component"))?;
        if raw.len() as u64 != member.size_bytes {
            return Err(SourceCommandError::Conflict(
                "selected bootstrap software component size changed",
            ));
        }
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let context = cmd::CommandContext {
        base_revision: selection.source_revision,
        configuration_raw: owner_raw.clone(),
        request_raw: request_raw.clone(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(uid),
        files,
    };
    context.check_selected_software_inputs(&software, &components, deadline, cancelled)?;
    owner_fs.current_context(&context, deadline, cancelled)?;

    Ok(ProtectedBootstrapContext {
        selection,
        owner_path,
        owner_raw,
        owner_fs,
        request_raw,
        context,
        software,
        components,
        expected_executable,
        runtime_observation,
    })
}

pub(super) fn selected_source(invocation: &Value) -> SourceCommandResult<SourceSelection> {
    let value = &invocation["source_selection"];
    exact_optional(
        value,
        &[
            "domain",
            "source_contract_sha256",
            "store_root",
            "store_limits",
            "job_id",
            "job_fence",
            "initialization_mode",
            "source_revision_sha256",
            "source_membership_sha256",
            "source_membership_count",
        ],
        &["initialize_schema"],
    )?;
    let domain = text(value, "domain")?;
    if domain.is_empty() || domain.len() > MAX_DOMAIN_BYTES || domain.contains('\0') {
        return Err(SourceCommandError::Invalid("PostgreSQL bootstrap domain"));
    }
    let job_id = text(value, "job_id")?;
    if job_id.is_empty() || job_id.len() > MAX_JOB_ID_BYTES || job_id.contains('\0') {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL bootstrap job identity",
        ));
    }
    let job_fence = number(value, "job_fence")?;
    if job_fence > i64::MAX as u64 {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL bootstrap job fence",
        ));
    }
    let job_mode = match text(value, "initialization_mode")? {
        "initialize" => BootstrapJobMode::Initialize,
        "existing" => BootstrapJobMode::RequireExisting,
        _ => return Err(SourceCommandError::Invalid("PostgreSQL bootstrap mode")),
    };
    let initialize_schema = optional_bool(value, "initialize_schema")?;
    if initialize_schema && job_mode != BootstrapJobMode::Initialize {
        return Err(SourceCommandError::Denied(
            "new CMD.2 schema requires initial domain and job selection",
        ));
    }
    if job_mode == BootstrapJobMode::Initialize && job_fence != 1 {
        return Err(SourceCommandError::Denied(
            "bootstrap initialization requires the exact initial job fence",
        ));
    }
    let limits = &value["store_limits"];
    exact(
        limits,
        &[
            "max_segment_bytes",
            "max_frame_bytes",
            "max_frames",
            "max_journal_bytes",
        ],
    )?;
    let max_frames = u32::try_from(number(limits, "max_frames")?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL bootstrap store limits"))?;
    let max_journal_bytes = usize::try_from(number(limits, "max_journal_bytes")?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL bootstrap store limits"))?;
    Ok(SourceSelection {
        domain: domain.to_owned(),
        contract: digest(text(value, "source_contract_sha256")?)?,
        store_root: absolute(text(value, "store_root")?)?,
        store_limits: SegmentLimits {
            max_segment_bytes: number(limits, "max_segment_bytes")?,
            max_frame_bytes: number(limits, "max_frame_bytes")?,
            max_frames,
            max_journal_bytes,
        },
        job_id: job_id.to_owned(),
        job_fence,
        job_mode,
        initialize_schema,
        source_revision: SourceRevision(digest(text(value, "source_revision_sha256")?)?),
        membership: SourceMembershipV1 {
            digest: digest(text(value, "source_membership_sha256")?)?,
            count: number(value, "source_membership_count")?,
        },
    })
}

fn import_budgets(invocation: &Value) -> SourceCommandResult<ImportBudgets> {
    let budgets = &invocation["budgets"];
    exact(
        budgets,
        &[
            "max_revisions",
            "max_members",
            "max_total_bytes",
            "max_member_bytes",
            "max_schema_receipts",
            "max_schema_receipt_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
        ],
    )?;
    let max_revisions = usize::try_from(capped(budgets, "max_revisions", 4)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL bootstrap revision budget"))?;
    let max_members = capped(budgets, "max_members", MAX_IMPORT_MEMBERS)?;
    let max_total_bytes = capped(budgets, "max_total_bytes", MAX_IMPORT_BYTES)?;
    let max_member_bytes = capped(budgets, "max_member_bytes", MAX_IMPORT_MEMBER_BYTES)?;
    let _ = capped(budgets, "max_schema_receipts", 128)?;
    let _ = capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let _ = capped(budgets, "worker_cpu_seconds", 3)?;
    let _ = capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    Ok(ImportBudgets {
        cut: CutReadLimits {
            max_revisions,
            max_members,
            max_total_bytes,
            max_member_bytes,
        },
        max_members,
        max_total_bytes,
        max_member_bytes,
    })
}

pub(super) fn selected_software(
    invocation: &Value,
    max_total_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(SoftwareCaptureReader, SoftwareComponentSelectionV1)> {
    let selection = &invocation["software_selection"];
    exact(
        selection,
        &[
            "source_git_commit",
            "source_git_tree",
            "capture_manifest_sha256",
        ],
    )?;
    let read_limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 512,
        max_selected_object_bytes: max_total_bytes,
        json: JsonLimits::default(),
    };
    let software = SoftwareCaptureReader::open(
        &absolute(text(invocation, "software_capture")?)?,
        &absolute(text(invocation, "software_restored_root")?)?,
        SoftwareCaptureSelectionV1 {
            source_git_commit: text(selection, "source_git_commit")?.to_owned(),
            source_git_tree: text(selection, "source_git_tree")?.to_owned(),
            capture_manifest_sha256: digest(text(selection, "capture_manifest_sha256")?)?,
        },
        read_limits,
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Conflict("selected bootstrap software capture"))?;
    let paths = invocation["software_components"]
        .as_array()
        .filter(|paths| !paths.is_empty() && paths.len() <= 128)
        .ok_or(SourceCommandError::Invalid(
            "selected bootstrap software components",
        ))?
        .iter()
        .map(|path| {
            let path = path.as_str().ok_or(SourceCommandError::Invalid(
                "selected bootstrap software component path",
            ))?;
            RelativePath::parse(path)
                .map_err(|_| SourceCommandError::Invalid("selected bootstrap software path"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if paths.iter().any(|path| path.as_str().starts_with("ToS/"))
        || paths
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != paths.len()
    {
        return Err(SourceCommandError::Denied(
            "bootstrap source and software namespaces overlap",
        ));
    }
    let components = software
        .select_components(&paths)
        .map_err(|_| SourceCommandError::Conflict("selected bootstrap software components"))?;
    Ok((software, components))
}

pub(super) fn request_bytes(input: impl Read) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_REQUEST + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL bootstrap request read"))?;
    if raw.len() > MAX_REQUEST {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL bootstrap request budget",
        ));
    }
    Ok(raw)
}

pub(super) fn value(value: &tos_foundation::JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL bootstrap JSON"))
}

pub(super) fn check_owner_scope(
    owner: &Value,
    owner_root: &Path,
    selection: &SourceSelection,
) -> SourceCommandResult<()> {
    exact_optional(
        owner,
        &[
            "schema_version",
            "uid",
            "expires_at",
            "source_root",
            "principal_id",
            "authority_ref",
            "allowed_operations",
            "domain",
            "source_contract_sha256",
            "source_revision_sha256",
            "source_membership_sha256",
            "source_membership_count",
            "initialization_mode",
            "job_id",
            "job_fence",
        ],
        &["initialize_schema"],
    )?;
    let root = owner_root
        .to_str()
        .ok_or(SourceCommandError::Invalid("bootstrap owner root UTF-8"))?;
    let operations = owner["allowed_operations"]
        .as_array()
        .filter(|operations| operations.len() == 1)
        .ok_or(SourceCommandError::Denied(
            "bootstrap owner operation scope",
        ))?;
    if text(owner, "schema_version")? != OWNER_SCHEMA
        || text(owner, "source_root")? != root
        || text(owner, "domain")? != selection.domain
        || digest(text(owner, "source_contract_sha256")?)? != selection.contract
        || SourceRevision(digest(text(owner, "source_revision_sha256")?)?)
            != selection.source_revision
        || digest(text(owner, "source_membership_sha256")?)? != selection.membership.digest
        || number(owner, "source_membership_count")? != selection.membership.count
        || text(owner, "initialization_mode")?
            != match selection.job_mode {
                BootstrapJobMode::Initialize => "initialize",
                BootstrapJobMode::RequireExisting => "existing",
            }
        || text(owner, "job_id")? != selection.job_id
        || number(owner, "job_fence")? != selection.job_fence
        || optional_bool(owner, "initialize_schema")? != selection.initialize_schema
        || operations[0].as_str() != Some("source.bootstrap")
    {
        return Err(SourceCommandError::Denied(
            "bootstrap owner does not authorize the exact selected import",
        ));
    }
    for field in ["principal_id", "authority_ref"] {
        let value = text(owner, field)?;
        if value.is_empty() || value.len() > 512 || value.contains('\0') {
            return Err(SourceCommandError::Invalid("bootstrap owner reference"));
        }
    }
    Ok(())
}

fn directory_empty(directory: &std::fs::File) -> SourceCommandResult<bool> {
    let mut entries = fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| SourceCommandError::Denied("private bootstrap store enumeration"))?;
    match entries.next() {
        None => Ok(true),
        Some(Ok(_)) => Ok(false),
        Some(Err(_)) => Err(SourceCommandError::Denied("private bootstrap store entry")),
    }
}

fn initialized_store_empty(directory: &std::fs::File) -> SourceCommandResult<bool> {
    let mut entries = fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| SourceCommandError::Denied("private bootstrap store enumeration"))?;
    let mut names = std::collections::BTreeSet::new();
    let mut metadata_is_file = false;
    for _ in 0..7 {
        let Some(entry) = entries.next() else { break };
        let entry =
            entry.map_err(|_| SourceCommandError::Denied("private bootstrap store entry"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| SourceCommandError::Conflict("private bootstrap store name"))?;
        let kind = entry
            .file_type()
            .map_err(|_| SourceCommandError::Conflict("private bootstrap store type"))?;
        if name == "store.meta" {
            metadata_is_file = kind.is_file();
        } else if kind.is_dir()
            && [
                "staging",
                "segments",
                "pins",
                "attempts",
                "leaves",
                "generations",
            ]
            .contains(&name.as_str())
        {
            names.insert(name);
        } else {
            return Ok(false);
        }
    }
    if entries
        .next()
        .transpose()
        .map_err(|_| SourceCommandError::Denied("private bootstrap store entry"))?
        .is_some()
    {
        return Ok(false);
    }
    if !metadata_is_file
        || names.len() != 6
        || names
            != [
                "attempts",
                "generations",
                "leaves",
                "pins",
                "segments",
                "staging",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
    {
        return Ok(false);
    }
    for name in [
        "staging",
        "segments",
        "pins",
        "attempts",
        "leaves",
        "generations",
    ] {
        let child = tos_fd_open::open_directory_at(directory, Path::new(name))
            .map_err(|_| SourceCommandError::Conflict("private bootstrap store directory"))?;
        if !directory_empty(&child)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn selected_store(
    owner: &CreationFilesystem,
    selection: &SourceSelection,
) -> SourceCommandResult<SegmentStore> {
    let root = owner.protected_root_file();
    let store = if directory_empty(root)? {
        if selection.job_mode != BootstrapJobMode::Initialize {
            return Err(SourceCommandError::Conflict(
                "selected PostgreSQL bootstrap store is absent",
            ));
        }
        SegmentStore::initialize_empty_at(root, selection.domain.as_bytes(), selection.store_limits)
            .map_err(|_| SourceCommandError::Conflict("initialize private bootstrap store"))?
    } else {
        if !initialized_store_empty(root)? {
            return Err(SourceCommandError::Conflict(
                "selected PostgreSQL bootstrap store is not unused",
            ));
        }
        SegmentStore::open_existing_at(root, selection.store_limits)
            .map_err(|_| SourceCommandError::Conflict("open private bootstrap store"))?
    };
    if store.custody_domain() != selection.domain.as_bytes() {
        return Err(SourceCommandError::Conflict(
            "private bootstrap store custody differs",
        ));
    }
    if !initialized_store_empty(root)? {
        return Err(SourceCommandError::Conflict(
            "private bootstrap store changed during selection",
        ));
    }
    Ok(store)
}

pub(super) fn source_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        // Conflict reasons are static owner labels, not database/source text.
        DurableError::Conflict(reason) => SourceCommandError::Conflict(reason),
        DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        // These owner labels are static strings, never database messages or
        // source values. Preserve the exact refusal so a failed import can be
        // diagnosed without repeating it merely to discover the failed check.
        DurableError::Invalid(reason) | DurableError::Corrupt(reason) => {
            SourceCommandError::Invalid(reason)
        }
        DurableError::Database(error) => {
            // PostgreSQL messages may contain private source values. Expose only
            // the fixed protocol code, never Display/Debug or server detail.
            let code = error
                .code()
                .map(|code| code.code())
                .filter(|code| {
                    code.len() == 5
                        && code
                            .bytes()
                            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
                })
                .unwrap_or("unclassified");
            eprintln!("PostgreSQL bootstrap database failure SQLSTATE={code}");
            SourceCommandError::Conflict("PostgreSQL bootstrap database operation failed")
        }
        DurableError::Storage(error) => {
            // SegmentError detail is a static owner label; the attached IO
            // error and its possibly private path are deliberately not emitted.
            eprintln!(
                "PostgreSQL bootstrap storage failure category={:?}",
                error.code
            );
            SourceCommandError::Conflict(error.detail)
        }
        DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("PostgreSQL bootstrap completion is indeterminate")
        }
    }
}

/// Execute one exactly scoped initial PostgreSQL source import.
pub(super) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    exact(
        invocation,
        &[
            "schema_version",
            "owner_config",
            "native_executable",
            "native_executable_sha256",
            "source_selection",
            "corpus_store",
            "source_revision",
            "software_capture",
            "software_restored_root",
            "software_selection",
            "software_components",
            "schema_worker",
            "budgets",
        ],
    )?;
    if text(invocation, "schema_version")? != INVOCATION_SCHEMA {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL source bootstrap invocation",
        ));
    }
    let budgets = import_budgets(invocation)?;
    let protected = prepare_protected_bootstrap(
        invocation,
        input,
        uid,
        SoftwareInputLimits {
            max_members: budgets.max_members,
            max_total_bytes: budgets.max_total_bytes,
            max_member_bytes: budgets.max_member_bytes,
        },
        deadline,
        cancelled,
    )?;
    let selection = &protected.selection;
    if digest(text(invocation, "source_revision")?)? != selection.source_revision.0 {
        return Err(SourceCommandError::Conflict(
            "source revision selection differs",
        ));
    }

    let source_read_limits = ReadLimits {
        max_manifest_bytes: 4_194_304,
        max_manifest_entries: 4_096,
        max_selected_object_bytes: MAX_IMPORT_MEMBER_BYTES,
        json: JsonLimits::default(),
    };
    let corpus = CorpusReader::open_existing(
        &absolute(text(invocation, "corpus_store")?)?,
        source_read_limits,
    )
    .map_err(|_| SourceCommandError::Conflict("selected immutable source store"))?;
    let revision = selection.source_revision;
    if corpus
        .select_current()
        .map_err(|_| SourceCommandError::Conflict("source current pointer"))?
        .is_some_and(|current| current != revision)
    {
        return Err(SourceCommandError::Conflict(
            "selected immutable source is no longer current",
        ));
    }
    let cut = corpus
        .open_source_cut(revision, budgets.cut, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("selected immutable source cut"))?;
    let membership = cut
        .stream(revision)
        .map_err(|_| SourceCommandError::Conflict("selected source membership"))?
        .expectation();
    if revision != selection.source_revision || membership != selection.membership {
        return Err(SourceCommandError::Conflict(
            "source revision or complete membership differs from protected owner",
        ));
    }

    let context_bytes = protected
        .context
        .files
        .iter()
        .try_fold(0u64, |sum, file| sum.checked_add(file.raw.len() as u64))
        .ok_or(SourceCommandError::Invalid(
            "selected bootstrap software byte budget",
        ))?;
    let selected_source_bytes = cut
        .current()
        .members()
        .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
        .ok_or(SourceCommandError::Invalid(
            "source cut byte budget overflow",
        ))?;
    let selected_count = protected
        .context
        .files
        .len()
        .checked_add(cut.current().member_count())
        .and_then(|count| u64::try_from(count).ok())
        .ok_or(SourceCommandError::Invalid(
            "source cut member budget overflow",
        ))?;
    if selected_count > budgets.max_members
        || context_bytes
            .checked_add(selected_source_bytes)
            .is_none_or(|bytes| bytes > budgets.max_total_bytes)
        || cut
            .current()
            .members()
            .any(|member| member.size_bytes > budgets.max_member_bytes)
    {
        return Err(SourceCommandError::Invalid(
            "combined selected bootstrap context budget",
        ));
    }
    protected.context.check_from_selected_captures(
        &cut,
        &protected.software,
        &protected.components,
        deadline,
        cancelled,
    )?;

    let mut worker = super::selected_schema(invocation, &cut, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let owner_check =
        || protected.owner_check(invocation_path, invocation_raw, uid, deadline, cancelled);
    owner_check().map_err(source_error)?;

    if selection.initialize_schema {
        db.initialize_empty_schema()
            .map_err(|_| SourceCommandError::Conflict("initialize absent CMD.2 schema"))?;
    }
    db.require_compatible_schema()
        .map_err(|_| SourceCommandError::Conflict("selected CMD.2 schema is not provisioned"))?;

    owner_check().map_err(source_error)?;

    // Schema readiness is established before any local store initialization.
    // Only the authorized initialization path can create an empty physical
    // store; existing stores are descriptor-anchored and accepted only when
    // all six data directories are empty.
    let store = selected_store(&protected.owner_fs, selection)?;
    owner_check().map_err(source_error)?;
    if store.custody_domain() != selection.domain.as_bytes() {
        return Err(SourceCommandError::Conflict(
            "private bootstrap store identity changed",
        ));
    }
    owner_check().map_err(source_error)?;

    let reopened = db
        .bootstrap_source_cohort(
            &store,
            &selection.domain,
            &cut,
            selection.source_revision,
            selection.membership,
            &protected.context,
            &protected.software,
            &protected.components,
            &mut worker,
            selection.contract,
            &selection.job_id,
            selection.job_fence,
            selection.job_mode,
            &owner_check,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
    Ok(json!({
        "schema_version":RESULT_SCHEMA,
        "domain":selection.domain,
        "source_revision_sha256":selection.source_revision.0.to_prefixed(),
        "source_membership_sha256":selection.membership.digest.to_prefixed(),
        "source_membership_count":selection.membership.count,
        "current_membership_sha256":reopened.current_membership.digest.to_prefixed(),
        "current_membership_count":reopened.current_membership.count,
        "source_complete":true,
        "schema_initialized":selection.initialize_schema,
        "selected_generation_sha256":reopened.selected.digest().to_prefixed(),
        "selected_head_seq":reopened.selected.cut().through_commit_seq(),
        "selected_state_sha256":reopened.selected.cut().state_digest().to_prefixed(),
        "bootstrap_resume_supported":false,
        "grants_admission":false,
        "grants_publication":false
    }))
}
