//! Native bootstrap and captured-cut transport for the maintained source-
//! foundation command. Selection and byte custody do not establish owner
//! findings, source validity, rights, or admission.

use super::foundation_cli::{self, FoundationArguments};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_revisions::ReadonlyRecordFiles;
use crate::source_serialization::executable;
use crate::source_text_owner::{normalized_absolute, read_absolute};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, RelativePath, SourceRevision, parse_json_with_state_budget,
};
use tos_source_store::{CorpusCutReader, SourceMemberV1, SourcePresenceV1};

const BOOTSTRAP_WALL_MS: u64 = 60_000;
const INVOCATION_BYTES: usize = 1_048_576;
const INVOCATION_JSON_WORKSPACE: usize = 4 * 1_048_576;
const HASH_WORKSPACE_BYTES: usize = 65_536;
const MAX_CLI_ARGS: usize = 64;
const MAX_CLI_BYTES: usize = 32 * 1024;
const MAX_RELATIVE_PATH_BYTES: usize = 4096;

// A separate, conservative accounting envelope for the existing
// collect_readonly_record_files implementation. Its JSON parser has a fixed
// visit cap but no caller-provided state budget, so the transport admits a
// ratio bound before making each member available to it.
const JSON_PARSE_STATE_PER_INPUT_BYTE: usize = 128;
const RECORD_COLLECTOR_FIXED_STATE: usize = 1_048_576;
const RECORD_MAP_ENTRY_STATE: usize = 256;
const DIRECTORY_ENTRY_STATE: usize = 128;

/// Starts the one original clock and establishes the process boundary before
/// any protected invocation or source read. The caller keeps and passes the
/// one cancellation flag alongside this original deadline.
pub(crate) struct FoundationBootstrapClock {
    started: Instant,
    hard_deadline: Instant,
    selected_deadline: Cell<Option<Instant>>,
    uid: u32,
}

impl FoundationBootstrapClock {
    pub(crate) fn begin() -> Result<Self> {
        let started = Instant::now();
        let hard_deadline = started
            .checked_add(Duration::from_millis(BOOTSTRAP_WALL_MS))
            .ok_or(Error::Invalid("foundation bootstrap clock range"))?;
        let uid = rustix::process::getuid().as_raw();
        if rustix::process::geteuid().as_raw() != uid {
            return Err(Error::Denied("foundation setuid invocation refused"));
        }
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .map_err(|_| Error::Denied("foundation dumpable process boundary"))?;
        if rustix::process::dumpable_behavior()
            .map_err(|_| Error::Denied("foundation dumpable process boundary"))?
            != rustix::process::DumpableBehavior::NotDumpable
        {
            return Err(Error::Denied("foundation dumpable process boundary"));
        }
        Ok(Self {
            started,
            hard_deadline,
            selected_deadline: Cell::new(None),
            uid,
        })
    }

    pub(crate) fn started(&self) -> Instant {
        self.started
    }

    pub(crate) fn hard_deadline(&self) -> Instant {
        self.selected_deadline.get().unwrap_or(self.hard_deadline)
    }

    // Bootstrap reads retain their fixed short ceiling. The protected finite
    // invocation selects the whole operation once, always from the ORIGINAL
    // start: parsing, hashing and setup never receive a fresh operation clock.
    fn select_operation_deadline(&self, wall_ms: u64, cancelled: &AtomicBool) -> Result<Instant> {
        active(self.hard_deadline, cancelled)?;
        if self.selected_deadline.get().is_some() || wall_ms == 0 || wall_ms == u64::MAX {
            return Err(Error::Invalid("foundation operation clock selection"));
        }
        let deadline = self
            .started
            .checked_add(Duration::from_millis(wall_ms))
            .ok_or(Error::Invalid("foundation operation deadline range"))?;
        active(deadline, cancelled)?;
        self.selected_deadline.set(Some(deadline));
        Ok(deadline)
    }

    pub(crate) fn uid(&self) -> u32 {
        self.uid
    }
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::Denied("foundation invocation cancelled"))
    } else if Instant::now() >= deadline {
        Err(Error::Denied("foundation invocation deadline"))
    } else {
        Ok(())
    }
}

/// The native `--invocation` selector is stripped before the maintained CLI
/// parser sees its original arguments. No installed-root or cwd fallback is
/// supplied; `--repo-root` remains an explicit user-selected path.
pub(crate) struct FoundationLaunchArguments {
    pub arguments: FoundationArguments,
    pub invocation_path: Option<PathBuf>,
    /// Conservative peak bound for the filtered argv copies and parsed path
    /// copies made while constructing this retained launch description.
    pub state_upper_bound_bytes: usize,
}

pub(crate) fn parse_launch_arguments(args: &[OsString]) -> Result<FoundationLaunchArguments> {
    let argument_bytes = args.iter().try_fold(0usize, |sum, arg| {
        sum.checked_add(arg.as_os_str().as_bytes().len())
    });
    if args.len() > MAX_CLI_ARGS || argument_bytes.is_none_or(|n| n > MAX_CLI_BYTES) {
        return Err(Error::Unsupported("foundation argument limits"));
    }

    let mut filtered = Vec::with_capacity(args.len());
    let mut invocation_path = None;
    let mut index = 0;
    while index < args.len() {
        if args[index].to_str() == Some("--invocation") {
            if invocation_path.is_some() {
                return Err(Error::Invalid("duplicate foundation invocation option"));
            }
            index += 1;
            let value = args
                .get(index)
                .and_then(|arg| arg.to_str())
                .ok_or(Error::Invalid("foundation invocation path must be UTF-8"))?;
            invocation_path = Some(PathBuf::from(value));
        } else {
            filtered.push(args[index].clone());
        }
        index += 1;
    }

    let mut arguments = foundation_cli::parse_arguments(&filtered, None).map_err(Error::Invalid)?;
    if !arguments.help && invocation_path.is_none() {
        return Err(Error::Denied("foundation invocation file must be explicit"));
    }
    if !arguments.help && arguments.repo_root.is_none() {
        return Err(Error::Denied("foundation repository root must be explicit"));
    }
    if !arguments.help
        && let Some(root) = arguments.repo_root.as_deref()
    {
        arguments.repo_root = Some(normalize_selected_path(root)?);
    }
    if !arguments.help
        && let Some(root) = arguments.payload_source_root.as_deref()
    {
        arguments.payload_source_root = Some(normalize_selected_path(root)?);
    }
    let launch_state_bytes = argument_bytes
        .and_then(|bytes| bytes.checked_mul(3))
        .and_then(|bytes| {
            args.len()
                .checked_mul(size_of::<OsString>())
                .and_then(|headers| bytes.checked_add(headers))
        })
        .and_then(|bytes| bytes.checked_add(size_of::<FoundationLaunchArguments>()))
        .ok_or(Error::Unsupported("foundation launch state overflow"))?;
    Ok(FoundationLaunchArguments {
        arguments,
        invocation_path,
        state_upper_bound_bytes: launch_state_bytes,
    })
}

/// Resolve only paths explicitly selected by the command or protected
/// invocation. Existence and held-root custody belong to the route opener.
pub(crate) struct FoundationSelectedRoots {
    pub repo_root: PathBuf,
    pub payload_source_root: Option<PathBuf>,
    pub artifact_root: Option<PathBuf>,
}

fn normalize_selected_path(path: &Path) -> Result<PathBuf> {
    let text = path
        .to_str()
        .ok_or(Error::Invalid("foundation selected path must be UTF-8"))?;
    if text.len() > MAX_RELATIVE_PATH_BYTES {
        return Err(Error::Unsupported("foundation selected path byte cap"));
    }
    normalized_absolute(text)
}

fn normalize_selected_text(text: &str) -> Result<PathBuf> {
    if text.len() > MAX_RELATIVE_PATH_BYTES {
        return Err(Error::Unsupported("foundation selected path byte cap"));
    }
    normalized_absolute(text)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FoundationInvocationBudgets {
    pub operation_wall_ms: u64,
    pub tmpfs_quota_bytes: u64,
    pub tmpfs_inode_limit: u64,
    pub working_ram_bytes: u64,
    pub worker_cpu_seconds: u64,
    pub worker_address_space_bytes: u64,
    pub sqlite_max_vm_steps: u64,
    pub max_member_bytes: u64,
    pub max_total_read_bytes: u64,
    /// Persistent admission-store IO; absent for existing read-only FND profiles.
    /// This is independent of private-stage tmpfs allocation.
    pub max_admission_write_bytes: Option<u64>,
    /// Incremental allocated-byte ceiling for the opt-in V2 segment writer.
    /// This is a separately selected persistent-store profile, not tmpfs.
    pub max_admission_store_bytes: Option<u64>,
    pub max_total_worker_wire_bytes: u64,
    pub max_current_members: u64,
    pub max_state_bytes: u64,
    pub max_issues: u64,
    pub max_output_bytes: u64,
    pub max_readonly_record_files: u64,
    pub max_readonly_record_bytes: u64,
    pub max_readonly_record_directory_entries: u64,
    pub max_readonly_record_read_calls: u64,
}

impl FoundationInvocationBudgets {
    fn parse(value: &tos_foundation::JsonValue) -> Result<Self> {
        const KEYS: &[&str] = &[
            "operation_wall_ms",
            "tmpfs_quota_bytes",
            "tmpfs_inode_limit",
            "working_ram_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
            "sqlite_max_vm_steps",
            "max_member_bytes",
            "max_total_read_bytes",
            "max_total_worker_wire_bytes",
            "max_current_members",
            "max_state_bytes",
            "max_issues",
            "max_output_bytes",
            "max_readonly_record_files",
            "max_readonly_record_bytes",
            "max_readonly_record_directory_entries",
            "max_readonly_record_read_calls",
        ];
        let admission_write_selected = value.object_get("max_admission_write_bytes").is_some();
        let admission_store_selected = value.object_get("max_admission_store_bytes").is_some();
        let mut selected_keys = KEYS.to_vec();
        if admission_write_selected {
            selected_keys.push("max_admission_write_bytes");
        }
        if admission_store_selected {
            selected_keys.push("max_admission_store_bytes");
        }
        crate::source_command::exact_keys(value, &selected_keys)
            .map_err(|_| Error::Invalid("foundation invocation budget fields"))?;
        let number = |key: &str| -> Result<u64> {
            crate::source_command::integer(value, key)
                .map_err(|_| Error::Invalid("foundation invocation budget value"))
                .and_then(|n| {
                    (n > 0).then_some(n).ok_or(Error::Invalid(
                        "foundation invocation budget must be positive",
                    ))
                })
        };
        Ok(Self {
            operation_wall_ms: number("operation_wall_ms")?,
            tmpfs_quota_bytes: number("tmpfs_quota_bytes")?,
            tmpfs_inode_limit: number("tmpfs_inode_limit")?,
            working_ram_bytes: number("working_ram_bytes")?,
            worker_cpu_seconds: number("worker_cpu_seconds")?,
            worker_address_space_bytes: number("worker_address_space_bytes")?,
            sqlite_max_vm_steps: number("sqlite_max_vm_steps")?,
            max_member_bytes: number("max_member_bytes")?,
            max_total_read_bytes: number("max_total_read_bytes")?,
            max_admission_write_bytes: if admission_write_selected {
                let bytes = number("max_admission_write_bytes")?;
                if bytes == u64::MAX {
                    return Err(Error::Invalid("admission write budget must be finite"));
                }
                Some(bytes)
            } else {
                None
            },
            max_admission_store_bytes: if admission_store_selected {
                let bytes = number("max_admission_store_bytes")?;
                if bytes == u64::MAX {
                    return Err(Error::Invalid("admission store budget must be finite"));
                }
                Some(bytes)
            } else {
                None
            },
            max_total_worker_wire_bytes: number("max_total_worker_wire_bytes")?,
            max_current_members: number("max_current_members")?,
            max_state_bytes: number("max_state_bytes")?,
            max_issues: number("max_issues")?,
            max_output_bytes: number("max_output_bytes")?,
            max_readonly_record_files: number("max_readonly_record_files")?,
            max_readonly_record_bytes: number("max_readonly_record_bytes")?,
            max_readonly_record_directory_entries: number("max_readonly_record_directory_entries")?,
            max_readonly_record_read_calls: number("max_readonly_record_read_calls")?,
        })
    }

    fn as_usize(value: u64, label: &'static str) -> Result<usize> {
        usize::try_from(value).map_err(|_| Error::Invalid(label))
    }

    pub(crate) fn readonly_record_limits(
        self,
        source_bytes_already_read: u64,
        state_already_reserved: usize,
    ) -> Result<CapturedReadonlyRecordLimits> {
        if source_bytes_already_read > self.max_total_read_bytes {
            return Err(Error::Unsupported(
                "foundation source read budget exhausted",
            ));
        }
        Ok(CapturedReadonlyRecordLimits {
            max_current_members: Self::as_usize(
                self.max_current_members,
                "foundation current-member limit range",
            )?,
            max_files: Self::as_usize(
                self.max_readonly_record_files,
                "foundation readonly record-file limit range",
            )?,
            max_read_calls: Self::as_usize(
                self.max_readonly_record_read_calls,
                "foundation readonly record-read limit range",
            )?,
            max_directory_entries: Self::as_usize(
                self.max_readonly_record_directory_entries,
                "foundation readonly directory limit range",
            )?,
            max_record_bytes: self.max_readonly_record_bytes,
            remaining_source_bytes: self.max_total_read_bytes - source_bytes_already_read,
            max_member_bytes: self.max_member_bytes,
            max_state_bytes: Self::as_usize(self.max_state_bytes, "foundation state limit range")?,
            state_already_reserved,
        })
    }
}

pub(crate) struct FoundationSchemaWorkerSelection {
    pub path: PathBuf,
    pub sha256: Digest256,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FoundationBootstrapCost {
    pub launch_state_upper_bound_bytes: usize,
    pub invocation_read_bytes: u64,
    pub invocation_read_calls: usize,
    pub self_image_read_bytes: u64,
    pub self_image_read_calls: usize,
    pub hash_workspace_bytes: usize,
    pub retained_input_bytes: usize,
    pub peak_state_upper_bound_bytes: usize,
}

/// Retains the exact protected launch bytes and the original deadline. It is
/// configuration custody only; the selected worker's executor still owns
/// executable pinning and schema semantics.
/// Authenticated carrier request from the exact protected invocation. This
/// selects representation only; NativeV4 still needs the complete native
/// validator and the original shared cancellation/store/profile custody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FoundationAdmissionRepresentation {
    ResidentV1,
    NativeV4,
    NativeV4SegmentV2,
}

impl FoundationAdmissionRepresentation {
    fn parse(value: &tos_foundation::JsonValue) -> Result<Self> {
        if value.object_get("admission_representation").is_none() {
            return Ok(Self::ResidentV1);
        }
        match crate::source_command::text(value, "admission_representation")
            .map_err(|_| Error::Invalid("foundation admission representation value"))?
        {
            "resident-v1" => Ok(Self::ResidentV1),
            "native-v4" => Ok(Self::NativeV4),
            "native-v4-segment-v2" => Ok(Self::NativeV4SegmentV2),
            _ => Err(Error::Invalid(
                "foundation admission representation profile",
            )),
        }
    }
}

/// Protected catalogue delivery representation. The default still verifies
/// authored-root persisted outputs; owned cold output stays private and temporary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FoundationCatalogueRepresentation {
    PersistedRoot,
    OwnedCold,
}
impl FoundationCatalogueRepresentation {
    fn parse(value: &tos_foundation::JsonValue) -> Result<Self> {
        if value.object_get("catalogue_representation").is_none() {
            return Ok(Self::PersistedRoot);
        }
        match crate::source_command::text(value, "catalogue_representation")
            .map_err(|_| Error::Invalid("foundation catalogue representation value"))?
        {
            "persisted-root" => Ok(Self::PersistedRoot),
            "owned-cold" => Ok(Self::OwnedCold),
            _ => Err(Error::Invalid(
                "foundation catalogue representation profile",
            )),
        }
    }
}
/// Opt-in mechanical V2 consumer selection inside the existing protected
/// invocation. These are slices of its original cap, never new grants.
#[derive(Clone, Debug)]
pub(crate) struct FoundationV2CaseSelection {
    pub target: PathBuf,
    pub identity: String,
    pub member_path: RelativePath,
    pub source_store_bytes: u64,
    pub target_store_bytes: u64,
    /// Optional original persistent slice for the existing SQLite provider.
    pub sqlite_store_bytes: u64,
    pub tree_nodes: u64,
    pub tree_bytes: u64,
    pub point_tree_nodes: u64,
    pub point_tree_bytes: u64,
    pub tree_rows: u64,
    pub history_roots: usize,
    pub files: u64,
    pub directories: u64,
    pub depth: usize,
    pub state_bytes: usize,
    pub pointer_bytes: usize,
    pub object_bytes: usize,
    pub snapshot_bytes: u64,
    pub allocation_unit_bytes: u64,
    pub aux_bytes: u64,
    pub sqlite_cache_bytes: usize,
    pub sqlite_native_overhead_bytes: usize,
}
impl FoundationV2CaseSelection {
    fn parse(
        value: &tos_foundation::JsonValue,
        budgets: FoundationInvocationBudgets,
        representation: FoundationAdmissionRepresentation,
        artifact_root: Option<&Path>,
    ) -> Result<Self> {
        crate::source_command::exact_keys(value, &["target", "identity", "member_path", "limits"])
            .map_err(|_| Error::Invalid("V2 case fields"))?;
        if representation != FoundationAdmissionRepresentation::NativeV4SegmentV2 {
            return Err(Error::Invalid(
                "V2 case requires explicit segment V2 selection",
            ));
        }
        let target = normalize_selected_text(crate::source_command::text(value, "target")?)?;
        if artifact_root.is_none_or(|root| target == root || !target.starts_with(root)) {
            return Err(Error::Invalid(
                "V2 case target outside selected artifact root",
            ));
        }
        let identity = crate::source_command::text(value, "identity")?;
        if identity.is_empty()
            || identity.len() > MAX_RELATIVE_PATH_BYTES
            || identity.chars().any(char::is_control)
        {
            return Err(Error::Invalid("V2 case identity shape"));
        }
        let member_path_text = crate::source_command::text(value, "member_path")?;
        if member_path_text.len() > MAX_RELATIVE_PATH_BYTES {
            return Err(Error::Invalid("V2 case member path length"));
        }
        let member_path = RelativePath::parse(member_path_text)
            .map_err(|_| Error::Invalid("V2 case member path"))?;
        let limits = crate::source_command::field(value, "limits")?;
        const KEYS: &[&str] = &[
            "source_store_bytes",
            "target_store_bytes",
            "tree_nodes",
            "tree_bytes",
            "point_tree_nodes",
            "point_tree_bytes",
            "tree_rows",
            "history_roots",
            "files",
            "directories",
            "depth",
            "state_bytes",
            "pointer_bytes",
            "object_bytes",
            "snapshot_bytes",
            "allocation_unit_bytes",
            "aux_bytes",
            "sqlite_cache_bytes",
            "sqlite_native_overhead_bytes",
        ];
        let mut keys = KEYS.to_vec();
        if limits.object_get("sqlite_store_bytes").is_some() {
            keys.push("sqlite_store_bytes");
        }
        crate::source_command::exact_keys(limits, &keys)
            .map_err(|_| Error::Invalid("V2 case limit fields"))?;
        let number = |key: &str| -> Result<u64> {
            let n = crate::source_command::integer(limits, key)?;
            if n == 0 || n == u64::MAX {
                return Err(Error::Invalid("V2 case limits must be positive finite"));
            }
            Ok(n)
        };
        let size = |key: &str| -> Result<usize> {
            let n =
                usize::try_from(number(key)?).map_err(|_| Error::Invalid("V2 case limit range"))?;
            if n == usize::MAX {
                return Err(Error::Invalid("V2 case limit must be finite"));
            }
            Ok(n)
        };
        let selected = Self {
            target,
            identity: identity.to_owned(),
            member_path,
            source_store_bytes: number("source_store_bytes")?,
            target_store_bytes: number("target_store_bytes")?,
            sqlite_store_bytes: if limits.object_get("sqlite_store_bytes").is_some() {
                number("sqlite_store_bytes")?
            } else {
                0
            },
            tree_nodes: number("tree_nodes")?,
            tree_bytes: number("tree_bytes")?,
            point_tree_nodes: number("point_tree_nodes")?,
            point_tree_bytes: number("point_tree_bytes")?,
            tree_rows: number("tree_rows")?,
            history_roots: size("history_roots")?,
            files: number("files")?,
            directories: number("directories")?,
            depth: size("depth")?,
            state_bytes: size("state_bytes")?,
            pointer_bytes: size("pointer_bytes")?,
            object_bytes: size("object_bytes")?,
            snapshot_bytes: number("snapshot_bytes")?,
            allocation_unit_bytes: number("allocation_unit_bytes")?,
            aux_bytes: number("aux_bytes")?,
            sqlite_cache_bytes: size("sqlite_cache_bytes")?,
            sqlite_native_overhead_bytes: size("sqlite_native_overhead_bytes")?,
        };
        if selected
            .source_store_bytes
            .checked_add(selected.target_store_bytes)
            .and_then(|bytes| bytes.checked_add(selected.sqlite_store_bytes))
            != budgets.max_admission_store_bytes
            || selected
                .point_tree_nodes
                .checked_mul(2)
                .and_then(|n| n.checked_add(selected.tree_nodes))
                .is_none_or(|n| n == u64::MAX)
            || selected.depth > 64
            || selected.state_bytes as u64 > budgets.max_state_bytes
            || selected.object_bytes as u64 > budgets.max_member_bytes
            || selected.pointer_bytes > INVOCATION_BYTES
            || selected
                .tree_bytes
                .checked_add(
                    selected
                        .point_tree_bytes
                        .checked_mul(2)
                        .ok_or(Error::Invalid("V2 case tree byte partition overflow"))?,
                )
                .is_none_or(|n| n > budgets.max_total_read_bytes)
            || selected
                .aux_bytes
                .checked_mul(10)
                .is_none_or(|n| n > budgets.tmpfs_quota_bytes)
            || selected
                .sqlite_cache_bytes
                .checked_add(selected.sqlite_native_overhead_bytes)
                .is_none_or(|n| n >= selected.state_bytes)
        {
            return Err(Error::Invalid("V2 case slices exceed original invocation"));
        }
        Ok(selected)
    }
    pub(crate) fn retained_state_bytes(&self) -> Result<usize> {
        size_of::<Self>()
            .checked_add(self.target.as_os_str().as_bytes().len())
            .and_then(|n| n.checked_add(self.identity.len()))
            .and_then(|n| n.checked_add(self.member_path.as_str().len()))
            .ok_or(Error::Unsupported("V2 case retained-state overflow"))
    }
}

pub(crate) struct FoundationInvocation<'cancel> {
    path: PathBuf,
    raw: Vec<u8>,
    raw_sha256: Digest256,
    expected_executable_sha256: Digest256,
    admission_representation: FoundationAdmissionRepresentation,
    catalogue_representation: FoundationCatalogueRepresentation,
    pub schema_worker: FoundationSchemaWorkerSelection,
    pub artifact_root: Option<PathBuf>,
    v2_case: Option<FoundationV2CaseSelection>,
    pub budgets: FoundationInvocationBudgets,
    started: Instant,
    deadline: Instant,
    uid: u32,
    cancelled: &'cancel AtomicBool,
    pub cost: FoundationBootstrapCost,
}

impl<'cancel> FoundationInvocation<'cancel> {
    pub(crate) fn catalogue_representation(&self) -> FoundationCatalogueRepresentation {
        self.catalogue_representation
    }
    pub(crate) fn v2_case(&self) -> Option<&FoundationV2CaseSelection> {
        self.v2_case.as_ref()
    }
    pub(crate) fn source_store_allocation_bytes(&self) -> Option<u64> {
        self.v2_case
            .as_ref()
            .map(|case| case.source_store_bytes)
            .or(self.budgets.max_admission_store_bytes)
    }

    pub(crate) fn admission_representation(&self) -> FoundationAdmissionRepresentation {
        self.admission_representation
    }

    pub(crate) fn uid(&self) -> u32 {
        self.uid
    }

    pub(crate) fn cancellation_flag(&self) -> &'cancel AtomicBool {
        self.cancelled
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn started(&self) -> Instant {
        self.started
    }

    pub(crate) fn invocation_sha256(&self) -> Digest256 {
        self.raw_sha256
    }

    pub(crate) fn invocation_len(&self) -> usize {
        self.raw.len()
    }

    pub(crate) fn executable_sha256(&self) -> Digest256 {
        self.expected_executable_sha256
    }

    pub(crate) fn selected_roots(
        &self,
        launch: &FoundationLaunchArguments,
    ) -> Result<FoundationSelectedRoots> {
        let repo_root = launch
            .arguments
            .repo_root
            .as_deref()
            .ok_or(Error::Denied("foundation repository root must be explicit"))?;
        Ok(FoundationSelectedRoots {
            repo_root: normalize_selected_path(repo_root)?,
            payload_source_root: launch
                .arguments
                .payload_source_root
                .as_deref()
                .map(normalize_selected_path)
                .transpose()?,
            artifact_root: self.artifact_root.clone(),
        })
    }

    /// Select the caller-provided, quota-bound private stage before capture
    /// writes. The ticket values are matched to the selected envelope, not
    /// treated as host admission evidence.
    pub(crate) fn select_stage(
        &self,
    ) -> std::result::Result<
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
        &'static str,
    > {
        active(self.deadline, self.cancelled)
            .map_err(|_| "foundation invocation expired before private stage selection")?;
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            self.budgets.tmpfs_quota_bytes,
            self.budgets.tmpfs_inode_limit,
            self.budgets.working_ram_bytes,
        )
        .map_err(|_| "foundation private stage selection refused")
    }

    /// Invoke after owner controls and the last worker have completed, but
    /// before the final source-capture EOF recheck. No child may start after it.
    pub(crate) fn verify_before_source_eof(&mut self) -> Result<()> {
        active(self.deadline, self.cancelled)?;
        let required_state = self
            .raw
            .len()
            .checked_add(INVOCATION_BYTES)
            .and_then(|n| n.checked_add(HASH_WORKSPACE_BYTES))
            .and_then(|n| {
                n.checked_add(
                    self.cost
                        .retained_input_bytes
                        .saturating_sub(self.raw.len()),
                )
            })
            .ok_or(Error::Unsupported("foundation invocation state overflow"))?;
        let max_state = FoundationInvocationBudgets::as_usize(
            self.budgets.max_state_bytes,
            "foundation state limit range",
        )?;
        if required_state > max_state {
            return Err(Error::Unsupported(
                "foundation invocation recheck state budget",
            ));
        }

        let current = read_absolute(
            &self.path,
            self.uid,
            true,
            INVOCATION_BYTES,
            self.deadline,
            self.cancelled,
        )?;
        self.cost.invocation_read_calls =
            self.cost
                .invocation_read_calls
                .checked_add(1)
                .ok_or(Error::Unsupported(
                    "foundation invocation read count overflow",
                ))?;
        self.cost.invocation_read_bytes = self
            .cost
            .invocation_read_bytes
            .checked_add(current.len() as u64)
            .ok_or(Error::Unsupported(
                "foundation invocation read size overflow",
            ))?;
        let same = current.len() == self.raw.len()
            && Digest256::of_bytes(&current) == self.raw_sha256
            && current == self.raw;
        drop(current);
        if !same {
            return Err(Error::Conflict("foundation invocation changed before EOF"));
        }
        let (actual, image_bytes) = executable_with_size(self.deadline, self.cancelled)?;
        self.cost.self_image_read_calls =
            self.cost
                .self_image_read_calls
                .checked_add(1)
                .ok_or(Error::Unsupported(
                    "foundation executable read count overflow",
                ))?;
        self.cost.self_image_read_bytes = self
            .cost
            .self_image_read_bytes
            .checked_add(image_bytes)
            .ok_or(Error::Unsupported(
                "foundation executable read size overflow",
            ))?;
        if actual != self.expected_executable_sha256 {
            return Err(Error::Conflict("foundation running executable changed"));
        }
        active(self.deadline, self.cancelled)
    }
}

fn executable_with_size(deadline: Instant, cancelled: &AtomicBool) -> Result<(Digest256, u64)> {
    let before = std::fs::metadata("/proc/self/exe")
        .map_err(|_| Error::Unsupported("running foundation executable unavailable"))?;
    if !before.is_file() || before.len() == 0 {
        return Err(Error::Invalid("running foundation executable metadata"));
    }
    let digest = executable(deadline, cancelled)?;
    let after = std::fs::metadata("/proc/self/exe")
        .map_err(|_| Error::Unsupported("running foundation executable unavailable"))?;
    if before.len() != after.len() {
        return Err(Error::Conflict(
            "running foundation executable size changed",
        ));
    }
    Ok((digest, before.len()))
}

pub(crate) fn read_invocation<'cancel>(
    clock: &FoundationBootstrapClock,
    launch: &FoundationLaunchArguments,
    cancelled: &'cancel AtomicBool,
) -> Result<FoundationInvocation<'cancel>> {
    active(clock.hard_deadline, cancelled)?;
    let selected = launch
        .invocation_path
        .as_deref()
        .ok_or(Error::Denied("foundation invocation file must be explicit"))?;
    let path = normalize_selected_path(selected)?;
    let raw = read_absolute(
        &path,
        clock.uid,
        true,
        INVOCATION_BYTES,
        clock.hard_deadline,
        cancelled,
    )?;
    if raw.is_empty() {
        return Err(Error::Invalid("empty foundation invocation"));
    }
    let raw_sha256 = Digest256::of_bytes(&raw);
    let parsed = parse_json_with_state_budget(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: INVOCATION_BYTES,
            ..JsonLimits::default()
        },
        INVOCATION_JSON_WORKSPACE,
    )
    .map_err(|_| Error::Invalid("foundation invocation JSON or state budget"))?;
    let value = parsed.into_root();
    let artifact_root_selected = value.as_object().is_some_and(|fields| {
        fields
            .iter()
            .any(|(key, _)| key.as_str() == Some("artifact_root"))
    });
    // At most eight static keys; retain strict optional shapes without a heap
    // allocation or Vec growth before the invocation state is accounted.
    let mut invocation_keys = [
        "schema_version",
        "native_executable_sha256",
        "schema_worker",
        "budgets",
        "artifact_root",
        "admission_representation",
        "catalogue_representation",
        "v2_case",
    ];
    let mut invocation_key_count = 4;
    if artifact_root_selected {
        invocation_key_count += 1;
    }
    if value.object_get("admission_representation").is_some() {
        invocation_keys[invocation_key_count] = "admission_representation";
        invocation_key_count += 1;
    }
    if value.object_get("catalogue_representation").is_some() {
        invocation_keys[invocation_key_count] = "catalogue_representation";
        invocation_key_count += 1;
    }
    if value.object_get("v2_case").is_some() {
        invocation_keys[invocation_key_count] = "v2_case";
        invocation_key_count += 1;
    }
    crate::source_command::exact_keys(&value, &invocation_keys[..invocation_key_count])
        .map_err(|_| Error::Invalid("foundation invocation fields"))?;
    if crate::source_command::text(&value, "schema_version")?
        != "tos_local_native_foundation_invocation_v1"
    {
        return Err(Error::Invalid("foundation invocation profile"));
    }
    let admission_representation = FoundationAdmissionRepresentation::parse(&value)?;
    let catalogue_representation = FoundationCatalogueRepresentation::parse(&value)?;
    let expected_executable_sha256 = Digest256::from_prefixed(crate::source_command::text(
        &value,
        "native_executable_sha256",
    )?)
    .map_err(|_| Error::Invalid("foundation executable digest"))?;

    let worker = crate::source_command::field(&value, "schema_worker")?;
    crate::source_command::exact_keys(worker, &["absolute_path", "sha256"])
        .map_err(|_| Error::Invalid("foundation schema-worker fields"))?;
    let worker_path =
        normalize_selected_text(crate::source_command::text(worker, "absolute_path")?)?;
    let worker_sha256 = Digest256::from_prefixed(crate::source_command::text(worker, "sha256")?)
        .map_err(|_| Error::Invalid("foundation schema-worker digest"))?;

    let artifact_root = match value.object_get("artifact_root") {
        None | Some(tos_foundation::JsonValue::Null) => None,
        Some(value) => Some(normalize_selected_text(
            value
                .as_str()
                .ok_or(Error::Invalid("foundation artifact-root field"))?,
        )?),
    };
    let budgets =
        FoundationInvocationBudgets::parse(crate::source_command::field(&value, "budgets")?)?;
    match (
        admission_representation,
        budgets.max_admission_write_bytes,
        budgets.max_admission_store_bytes,
    ) {
        (FoundationAdmissionRepresentation::NativeV4SegmentV2, Some(write), Some(store))
            if store <= write => {}
        (FoundationAdmissionRepresentation::NativeV4SegmentV2, _, _) | (_, _, Some(_)) => {
            return Err(Error::Invalid(
                "V2 segment admission requires a finite paired persistent-store profile",
            ));
        }
        _ => {}
    }
    let v2_case = value
        .object_get("v2_case")
        .map(|selection| {
            FoundationV2CaseSelection::parse(
                selection,
                budgets,
                admission_representation,
                artifact_root.as_deref(),
            )
        })
        .transpose()?;
    drop(value);

    let deadline = clock.select_operation_deadline(budgets.operation_wall_ms, cancelled)?;
    let max_state = FoundationInvocationBudgets::as_usize(
        budgets.max_state_bytes,
        "foundation state limit range",
    )?;
    let selected_path_state = (if v2_case.is_some() { 6usize } else { 3usize })
        .checked_mul(MAX_RELATIVE_PATH_BYTES)
        .ok_or(Error::Unsupported(
            "foundation invocation path state overflow",
        ))?;
    let parse_state = raw
        .len()
        .checked_add(INVOCATION_JSON_WORKSPACE)
        .and_then(|n| n.checked_add(HASH_WORKSPACE_BYTES))
        .and_then(|n| n.checked_add(selected_path_state))
        .and_then(|n| n.checked_add(launch.state_upper_bound_bytes))
        .ok_or(Error::Unsupported("foundation invocation state overflow"))?;
    if parse_state > max_state {
        return Err(Error::Unsupported("foundation invocation state budget"));
    }
    let (actual_executable, executable_bytes) = executable_with_size(deadline, cancelled)?;
    if actual_executable != expected_executable_sha256 {
        return Err(Error::Conflict("foundation native executable identity"));
    }
    let raw_len = raw.len();
    let case_retained = v2_case
        .as_ref()
        .map(|case| case.retained_state_bytes())
        .transpose()?
        .unwrap_or(0);
    let retained_input_bytes = raw_len
        .checked_add(path.as_os_str().as_bytes().len())
        .and_then(|n| n.checked_add(worker_path.as_os_str().as_bytes().len()))
        .and_then(|n| {
            artifact_root.as_ref().map_or(Some(n), |root| {
                n.checked_add(root.as_os_str().as_bytes().len())
            })
        })
        .and_then(|n| n.checked_add(case_retained))
        .and_then(|n| n.checked_add(launch.state_upper_bound_bytes))
        .and_then(|n| n.checked_add(size_of::<FoundationInvocation<'_>>()))
        .ok_or(Error::Unsupported(
            "foundation invocation retained-state overflow",
        ))?;
    let mut cost = FoundationBootstrapCost {
        launch_state_upper_bound_bytes: launch.state_upper_bound_bytes,
        invocation_read_bytes: raw_len as u64,
        invocation_read_calls: 1,
        self_image_read_bytes: executable_bytes,
        self_image_read_calls: 1,
        hash_workspace_bytes: HASH_WORKSPACE_BYTES,
        retained_input_bytes,
        peak_state_upper_bound_bytes: parse_state,
    };
    cost.peak_state_upper_bound_bytes = cost
        .peak_state_upper_bound_bytes
        .max(retained_input_bytes)
        .max(
            raw_len
                .checked_add(HASH_WORKSPACE_BYTES)
                .ok_or(Error::Unsupported("foundation invocation state overflow"))?,
        );

    Ok(FoundationInvocation {
        path,
        raw,
        raw_sha256,
        expected_executable_sha256,
        admission_representation,
        catalogue_representation,
        schema_worker: FoundationSchemaWorkerSelection {
            path: worker_path,
            sha256: worker_sha256,
        },
        artifact_root,
        v2_case,
        budgets,
        started: clock.started,
        deadline,
        uid: clock.uid,
        cancelled,
        cost,
    })
}

pub(crate) struct CapturedReadonlyRecordLimits {
    pub max_current_members: usize,
    pub max_files: usize,
    pub max_read_calls: usize,
    pub max_directory_entries: usize,
    pub max_record_bytes: u64,
    pub remaining_source_bytes: u64,
    pub max_member_bytes: u64,
    pub max_state_bytes: usize,
    /// Already-retained capture, route, launch and caller state; it is included
    /// before each transport allocation, never added after the fact only.
    pub state_already_reserved: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CapturedReadonlyRecordCost {
    pub current_members_checked: usize,
    pub read_calls: usize,
    pub read_bytes: u64,
    pub directory_calls: usize,
    pub directory_entries: usize,
    pub retained_state_upper_bound_bytes: usize,
    pub peak_state_upper_bound_bytes: usize,
}

#[derive(Clone, Copy, Default)]
struct StableIdCopyBound {
    count: usize,
    text_bytes: usize,
}

/// ReadonlyRecordFiles over the exact immutable current revision selected by
/// `FoundationCapturedCut`. It does not route back to a live checkout and does
/// not treat cut absence as evidence about external payload or artifact roots.
pub(crate) struct CapturedReadonlyRecordFiles<'cut, 'cancel> {
    cut: &'cut CorpusCutReader,
    revision: SourceRevision,
    current_paths: &'cut [String],
    limits: CapturedReadonlyRecordLimits,
    original_deadline: Instant,
    cancelled: &'cancel AtomicBool,
    id_copy_bounds: BTreeMap<String, StableIdCopyBound>,
    id_index_state_upper: usize,
    retained_read_bytes: u64,
    retained_path_state_upper: usize,
    read_calls: usize,
    directory_calls: usize,
    directory_entries: usize,
    cost: CapturedReadonlyRecordCost,
}

impl<'cut, 'cancel> CapturedReadonlyRecordFiles<'cut, 'cancel> {
    pub(crate) fn new(
        cut: &'cut CorpusCutReader,
        revision: SourceRevision,
        current_paths: &'cut [String],
        limits: CapturedReadonlyRecordLimits,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
    ) -> Result<Self> {
        active(deadline, cancelled)?;
        if cut.current().revision() != revision {
            return Err(Error::Conflict("foundation record transport revision"));
        }
        if current_paths.len() > limits.max_current_members
            || cut.current().member_count() != current_paths.len()
            || limits.max_files == 0
            || limits.max_read_calls == 0
            || limits.max_record_bytes == 0
            || limits.max_member_bytes == 0
            || limits.max_state_bytes == 0
        {
            return Err(Error::Unsupported("foundation record transport bounds"));
        }
        let mut member_path_bytes_upper = 0usize;
        let mut members = cut.current().members();
        for path in current_paths {
            active(deadline, cancelled)?;
            let member = members.next().ok_or(Error::Conflict(
                "foundation current inventory differs from cut",
            ))?;
            if member.path.as_str() != path {
                return Err(Error::Conflict(
                    "foundation current inventory differs from cut",
                ));
            }
            member_path_bytes_upper = member_path_bytes_upper
                .checked_add(path.len())
                .ok_or(Error::Unsupported("foundation current path state overflow"))?;
        }
        if members.next().is_some() {
            return Err(Error::Conflict(
                "foundation current inventory differs from cut",
            ));
        }

        // read_member returns copied stable IDs with each raw object. Retain a
        // bounded per-path copy-size index so each such transient vector can be
        // admitted before the source object is read.
        let identity_count = cut.current().identity_count();
        let path_index_upper = cut
            .current()
            .member_count()
            .checked_mul(RECORD_MAP_ENTRY_STATE)
            .and_then(|n| n.checked_add(member_path_bytes_upper))
            .ok_or(Error::Unsupported("foundation record index state overflow"))?;
        let id_row_scan_upper = identity_count
            .checked_mul(size_of::<(&str, &RelativePath)>())
            .ok_or(Error::Unsupported(
                "foundation identity-index state overflow",
            ))?;
        let id_index_state_upper = path_index_upper
            .checked_add(id_row_scan_upper)
            .ok_or(Error::Unsupported("foundation record index state overflow"))?;
        let retained_preflight = limits
            .state_already_reserved
            .checked_add(id_index_state_upper)
            .and_then(|n| n.checked_add(RECORD_COLLECTOR_FIXED_STATE))
            .ok_or(Error::Unsupported("foundation readonly state overflow"))?;
        if retained_preflight > limits.max_state_bytes {
            return Err(Error::Unsupported(
                "foundation readonly record state budget",
            ));
        }

        let mut id_copy_bounds = BTreeMap::<String, StableIdCopyBound>::new();
        for (id, path) in cut.current().indexed_identities() {
            active(deadline, cancelled)?;
            if current_paths
                .binary_search_by(|candidate| candidate.as_str().cmp(path.as_str()))
                .is_err()
            {
                return Err(Error::Conflict(
                    "foundation identity points outside captured inventory",
                ));
            }
            if let Some(bound) = id_copy_bounds.get_mut(path.as_str()) {
                bound.count = bound
                    .count
                    .checked_add(1)
                    .ok_or(Error::Unsupported("foundation identity count overflow"))?;
                bound.text_bytes = bound
                    .text_bytes
                    .checked_add(id.len())
                    .ok_or(Error::Unsupported("foundation identity text overflow"))?;
            } else {
                id_copy_bounds.insert(
                    path.as_str().to_owned(),
                    StableIdCopyBound {
                        count: 1,
                        text_bytes: id.len(),
                    },
                );
            }
        }
        active(deadline, cancelled)?;

        let mut result = Self {
            cut,
            revision,
            current_paths,
            limits,
            original_deadline: deadline,
            cancelled,
            id_copy_bounds,
            id_index_state_upper,
            retained_read_bytes: 0,
            retained_path_state_upper: 0,
            read_calls: 0,
            directory_calls: 0,
            directory_entries: 0,
            cost: CapturedReadonlyRecordCost {
                current_members_checked: current_paths.len(),
                retained_state_upper_bound_bytes: retained_preflight,
                peak_state_upper_bound_bytes: retained_preflight,
                ..CapturedReadonlyRecordCost::default()
            },
        };
        let actual_index_state = result
            .id_copy_bounds
            .len()
            .checked_mul(RECORD_MAP_ENTRY_STATE)
            .and_then(|n| n.checked_add(member_path_bytes_upper))
            .and_then(|n| n.checked_add(id_row_scan_upper))
            .ok_or(Error::Unsupported("foundation record index state overflow"))?;
        if actual_index_state > id_index_state_upper {
            return Err(Error::Unsupported(
                "foundation record index accounting changed",
            ));
        }
        result.id_index_state_upper = id_index_state_upper;
        Ok(result)
    }

    pub(crate) fn cost(&self) -> CapturedReadonlyRecordCost {
        self.cost
    }

    fn verify_context(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        if !std::ptr::eq(cancelled, self.cancelled) || deadline > self.original_deadline {
            return Err(Error::Denied(
                "foundation readonly record operation context differs",
            ));
        }
        active(deadline, self.cancelled)
    }

    fn retained_state_after_read(&self, path: &str, next_bytes: u64) -> Result<usize> {
        let bytes = usize::try_from(next_bytes)
            .map_err(|_| Error::Unsupported("foundation readonly byte state range"))?;
        self.limits
            .state_already_reserved
            .checked_add(self.id_index_state_upper)
            .and_then(|n| n.checked_add(RECORD_COLLECTOR_FIXED_STATE))
            .and_then(|n| n.checked_add(bytes))
            .and_then(|n| n.checked_add(self.retained_path_state_upper))
            .and_then(|n| n.checked_add(path.len().checked_mul(3)?))
            .and_then(|n| n.checked_add(RECORD_MAP_ENTRY_STATE))
            .ok_or(Error::Unsupported(
                "foundation readonly record state overflow",
            ))
    }

    fn check_path_member(&self, path: &str) -> Result<(RelativePath, u64)> {
        if path.is_empty() || path.len() > MAX_RELATIVE_PATH_BYTES {
            return Err(Error::Invalid("foundation readonly record path bounds"));
        }
        if self
            .current_paths
            .binary_search_by(|candidate| candidate.as_str().cmp(path))
            .is_err()
        {
            return Err(Error::Denied(
                "foundation readonly record path outside captured cut",
            ));
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| Error::Invalid("foundation readonly record relative path"))?;
        let metadata = self.cut.current().member(&relative).ok_or(Error::Conflict(
            "foundation captured member metadata absent",
        ))?;
        Ok((relative, metadata.size_bytes))
    }
}

impl ReadonlyRecordFiles for CapturedReadonlyRecordFiles<'_, '_> {
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        self.verify_context(deadline, cancelled)?;
        if self.read_calls >= self.limits.max_read_calls || self.read_calls >= self.limits.max_files
        {
            return Err(Error::Unsupported(
                "foundation readonly record read-call budget",
            ));
        }
        let (relative, metadata_bytes) = self.check_path_member(path)?;
        let request_cap = u64::try_from(max_bytes)
            .map_err(|_| Error::Unsupported("foundation readonly member cap range"))?;
        if metadata_bytes > request_cap || metadata_bytes > self.limits.max_member_bytes {
            return Err(Error::Unsupported(
                "foundation readonly record member byte budget",
            ));
        }
        let next_read_bytes = self
            .retained_read_bytes
            .checked_add(metadata_bytes)
            .ok_or(Error::Unsupported("foundation readonly read-byte overflow"))?;
        if next_read_bytes > self.limits.max_record_bytes
            || next_read_bytes > self.limits.remaining_source_bytes
        {
            return Err(Error::Unsupported(
                "foundation readonly aggregate read budget",
            ));
        }
        let id_bound = self.id_copy_bounds.get(path).copied().unwrap_or_default();
        let copied_ids_state = id_bound
            .count
            .checked_mul(size_of::<String>() * 2 + 2)
            .and_then(|n| n.checked_add(id_bound.text_bytes.checked_mul(2)?))
            .ok_or(Error::Unsupported(
                "foundation stable-ID copy state overflow",
            ))?;
        let next_path_state = self
            .retained_path_state_upper
            .checked_add(
                path.len()
                    .checked_mul(3)
                    .ok_or(Error::Unsupported("foundation record path state overflow"))?,
            )
            .and_then(|n| n.checked_add(RECORD_MAP_ENTRY_STATE))
            .ok_or(Error::Unsupported("foundation record path state overflow"))?;
        let next_total_state = self
            .retained_state_after_read(path, next_read_bytes)?
            .checked_add(copied_ids_state)
            .and_then(|n| n.checked_add(size_of::<SourceMemberV1>()))
            .ok_or(Error::Unsupported(
                "foundation readonly record state overflow",
            ))?;
        let parser_workspace = if path.ends_with(".blob") {
            0
        } else {
            usize::try_from(metadata_bytes)
                .ok()
                .and_then(|n| n.checked_mul(JSON_PARSE_STATE_PER_INPUT_BYTE))
                .and_then(|n| n.checked_add(64 * 1024))
                .ok_or(Error::Unsupported(
                    "foundation readonly JSON state overflow",
                ))?
        };
        let peak_state =
            next_total_state
                .checked_add(parser_workspace)
                .ok_or(Error::Unsupported(
                    "foundation readonly peak state overflow",
                ))?;
        if peak_state > self.limits.max_state_bytes {
            return Err(Error::Unsupported(
                "foundation readonly record state budget",
            ));
        }

        let member = self
            .cut
            .read_member(
                self.revision,
                &relative,
                request_cap.min(self.limits.max_member_bytes),
                deadline,
                self.cancelled,
            )
            .map_err(|_| Error::Denied("foundation captured record member read refused"))?;
        if member.revision != self.revision
            || member.path != relative
            || member.raw.len() as u64 != metadata_bytes
        {
            return Err(Error::Conflict("foundation captured record member changed"));
        }
        let SourceMemberV1 {
            raw, stable_ids, ..
        } = member;
        drop(stable_ids);
        self.read_calls = self
            .read_calls
            .checked_add(1)
            .ok_or(Error::Unsupported("foundation readonly read-call overflow"))?;
        self.retained_read_bytes = next_read_bytes;
        self.retained_path_state_upper = next_path_state;
        self.cost.read_calls = self.read_calls;
        self.cost.read_bytes = self.retained_read_bytes;
        self.cost.retained_state_upper_bound_bytes = next_total_state
            .checked_sub(copied_ids_state)
            .and_then(|n| n.checked_sub(size_of::<SourceMemberV1>()))
            .ok_or(Error::Unsupported(
                "foundation readonly retained-state underflow",
            ))?;
        self.cost.peak_state_upper_bound_bytes =
            self.cost.peak_state_upper_bound_bytes.max(peak_state);
        Ok(raw)
    }

    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<(String, bool)>> {
        self.verify_context(deadline, cancelled)?;
        if self.directory_calls >= self.limits.max_files {
            return Err(Error::Unsupported(
                "foundation readonly directory-call budget",
            ));
        }
        if path.is_empty() || path.len() > MAX_RELATIVE_PATH_BYTES {
            return Err(Error::Invalid("foundation readonly directory path bounds"));
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| Error::Invalid("foundation readonly directory relative path"))?;
        let prefix_bytes = path
            .len()
            .checked_add(1)
            .ok_or(Error::Unsupported("foundation directory path overflow"))?;
        let state_before = self
            .limits
            .state_already_reserved
            .checked_add(self.id_index_state_upper)
            .and_then(|n| n.checked_add(RECORD_COLLECTOR_FIXED_STATE))
            .and_then(|n| {
                usize::try_from(self.retained_read_bytes)
                    .ok()
                    .and_then(|b| n.checked_add(b))
            })
            .and_then(|n| n.checked_add(self.retained_path_state_upper))
            .and_then(|n| n.checked_add(prefix_bytes))
            .ok_or(Error::Unsupported("foundation directory state overflow"))?;
        if state_before > self.limits.max_state_bytes {
            return Err(Error::Unsupported(
                "foundation readonly directory state budget",
            ));
        }

        let presence = self.cut.presence(self.revision, &relative);
        match presence {
            Some(SourcePresenceV1::File) => {
                return Err(Error::Conflict(
                    "foundation record directory is a captured file",
                ));
            }
            Some(SourcePresenceV1::MaterializedDirectory) | None => (),
        }
        let mut prefix = String::with_capacity(prefix_bytes);
        prefix.push_str(path);
        prefix.push('/');
        let start = self
            .current_paths
            .partition_point(|candidate| candidate.as_str() < prefix.as_str());

        let mut unique_entries = 0usize;
        let mut unique_name_bytes = 0usize;
        let mut previous_name: Option<&str> = None;
        let mut previous_is_dir = false;
        let mut scan = start;
        while let Some(candidate) = self.current_paths.get(scan) {
            active(deadline, self.cancelled)?;
            let Some(rest) = candidate.strip_prefix(&prefix) else {
                break;
            };
            if rest.is_empty() {
                scan += 1;
                continue;
            }
            let name = rest.split('/').next().unwrap_or(rest);
            let is_dir = rest.contains('/');
            if previous_name == Some(name) {
                if previous_is_dir != is_dir {
                    return Err(Error::Conflict(
                        "foundation captured directory child type conflict",
                    ));
                }
            } else {
                unique_entries = unique_entries.checked_add(1).ok_or(Error::Unsupported(
                    "foundation directory entry count overflow",
                ))?;
                unique_name_bytes =
                    unique_name_bytes
                        .checked_add(name.len())
                        .ok_or(Error::Unsupported(
                            "foundation directory entry state overflow",
                        ))?;
                previous_name = Some(name);
                previous_is_dir = is_dir;
            }
            scan += 1;
        }
        let next_directory_entries =
            self.directory_entries
                .checked_add(unique_entries)
                .ok_or(Error::Unsupported(
                    "foundation directory entry count overflow",
                ))?;
        if next_directory_entries > self.limits.max_directory_entries {
            return Err(Error::Unsupported(
                "foundation readonly directory-entry budget",
            ));
        }
        let output_state = unique_entries
            .checked_mul(size_of::<(String, bool)>() + DIRECTORY_ENTRY_STATE)
            .and_then(|n| n.checked_add(unique_name_bytes.checked_mul(2)?))
            .and_then(|n| state_before.checked_add(n))
            .ok_or(Error::Unsupported("foundation directory state overflow"))?;
        if output_state > self.limits.max_state_bytes {
            return Err(Error::Unsupported(
                "foundation readonly directory state budget",
            ));
        }

        let mut children = Vec::new();
        children
            .try_reserve_exact(unique_entries)
            .map_err(|_| Error::Unsupported("foundation directory allocation refused"))?;
        previous_name = None;
        scan = start;
        while let Some(candidate) = self.current_paths.get(scan) {
            active(deadline, self.cancelled)?;
            let Some(rest) = candidate.strip_prefix(&prefix) else {
                break;
            };
            if rest.is_empty() {
                scan += 1;
                continue;
            }
            let name = rest.split('/').next().unwrap_or(rest);
            if previous_name != Some(name) {
                children.push((name.to_owned(), rest.contains('/')));
                previous_name = Some(name);
            }
            scan += 1;
        }
        self.directory_calls = self
            .directory_calls
            .checked_add(1)
            .ok_or(Error::Unsupported("foundation directory-call overflow"))?;
        self.directory_entries = next_directory_entries;
        self.cost.directory_calls = self.directory_calls;
        self.cost.directory_entries = self.directory_entries;
        self.cost.peak_state_upper_bound_bytes =
            self.cost.peak_state_upper_bound_bytes.max(output_state);
        Ok(children)
    }
}

#[cfg(test)]
mod clock_selection_tests {
    use super::*;

    #[test]
    fn protected_operation_uses_original_start_and_cannot_renew_bootstrap() {
        let cancelled = AtomicBool::new(false);
        let started = Instant::now();
        let clock = FoundationBootstrapClock {
            started,
            hard_deadline: started + Duration::from_millis(BOOTSTRAP_WALL_MS),
            selected_deadline: Cell::new(None),
            uid: 0,
        };
        assert_eq!(clock.hard_deadline(), clock.hard_deadline);
        assert!(clock.select_operation_deadline(0, &cancelled).is_err());
        let selected = clock
            .select_operation_deadline(120_000, &cancelled)
            .unwrap();
        assert_eq!(selected, started + Duration::from_millis(120_000));
        assert_eq!(clock.hard_deadline(), selected);
        assert!(
            clock
                .select_operation_deadline(180_000, &cancelled)
                .is_err()
        );
        assert_eq!(clock.hard_deadline(), selected);
        let old = started - Duration::from_millis(BOOTSTRAP_WALL_MS + 1);
        let expired = FoundationBootstrapClock {
            started: old,
            hard_deadline: old + Duration::from_millis(BOOTSTRAP_WALL_MS),
            selected_deadline: Cell::new(None),
            uid: 0,
        };
        assert!(
            expired
                .select_operation_deadline(120_000, &cancelled)
                .is_err()
        );
        assert_eq!(expired.selected_deadline.get(), None);
    }

    #[test]
    fn protected_admission_representation_defaults_resident_and_refuses_unknown_shapes() {
        use FoundationAdmissionRepresentation::{NativeV4, ResidentV1};
        for (raw, expected) in [
            (br#"{}"#.as_slice(), ResidentV1),
            (
                br#"{"admission_representation":"resident-v1"}"#.as_slice(),
                ResidentV1,
            ),
            (
                br#"{"admission_representation":"native-v4"}"#.as_slice(),
                NativeV4,
            ),
        ] {
            let value = crate::source_command::parse(raw).unwrap();
            assert_eq!(
                FoundationAdmissionRepresentation::parse(&value).unwrap(),
                expected
            );
        }
        for raw in [
            br#"{"admission_representation":null}"#.as_slice(),
            br#"{"admission_representation":true}"#.as_slice(),
            br#"{"admission_representation":4}"#.as_slice(),
            br#"{"admission_representation":"spooled"}"#.as_slice(),
            br#"{"admission_representation":"native-v5"}"#.as_slice(),
        ] {
            let value = crate::source_command::parse(raw).unwrap();
            assert!(FoundationAdmissionRepresentation::parse(&value).is_err());
        }
    }
}
