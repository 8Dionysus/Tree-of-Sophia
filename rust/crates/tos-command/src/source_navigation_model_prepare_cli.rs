//! Protected one-time preparation of an initial managed Source Navigation
//! model. The preparation owner selects all inputs and finite work envelopes;
//! the command derives the retained V2 proof and selected expectation only
//! from the current addressed PG cut and the existing compiler producer.

use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Cursor, Read, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tos_compiler::SourceBinding;
use tos_compiler::managed_source::ManagedProducerProof;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_segment_store::SegmentStore;
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, ReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};

use super::source_navigation_model_compile::INITIAL_MAX_TOTAL_MS;
use crate::{
    source_command::{self as cmd, SourceCommandError, SourceCommandResult},
    source_creation_store::active,
};
use tos_validation::executor::ExactWorkerIdentity;

const HOST_FINALIZATION_RESERVE_MS: u64 = 60_000;

fn stage_file_identity(parent: &File, leaf: &str) -> SourceCommandResult<(u64, u64)> {
    let file = tos_fd_open::open_regular_at(parent, Path::new(leaf))
        .map_err(|_| SourceCommandError::Conflict("private compiler candidate unavailable"))?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("private compiler candidate metadata"))?;
    if !metadata.is_file() || metadata.uid() != rustix::process::geteuid().as_raw() {
        return Err(SourceCommandError::Conflict(
            "private compiler candidate ownership differs",
        ));
    }
    Ok((metadata.dev(), metadata.ino()))
}

fn remove_stage_file(
    isolated: &crate::source_creation_store::IsolatedCreationRoot,
    leaf: &str,
    expected_identity: (u64, u64),
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let parent = match isolated.verify_current(deadline, cancelled) {
        Ok(parent) => parent,
        Err(_error)
            if std::fs::symlink_metadata(isolated.path())
                .is_err_and(|metadata| metadata.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let file = tos_fd_open::open_regular_at(&parent, Path::new(leaf))
        .map_err(|_| SourceCommandError::Conflict("private compiler candidate changed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("private compiler candidate metadata"))?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || (metadata.dev(), metadata.ino()) != expected_identity
    {
        return Err(SourceCommandError::Conflict(
            "private compiler candidate identity changed",
        ));
    }
    drop(file);
    rustix::fs::unlinkat(&parent, leaf, rustix::fs::AtFlags::empty())
        .map_err(|_| SourceCommandError::Conflict("private compiler candidate cleanup failed"))?;
    parent
        .sync_all()
        .map_err(|_| SourceCommandError::Invalid("private compiler stage directory fsync"))?;
    Ok(())
}

fn cleanup_failed_child_stage(
    isolated: &crate::source_creation_store::IsolatedCreationRoot,
    leaf: &str,
    max_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let parent = isolated.verify_current(deadline, cancelled)?;
    match tos_fd_open::open_regular_at(&parent, Path::new(leaf)) {
        Ok(file) => {
            let metadata = file
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("failed child candidate metadata"))?;
            if !metadata.is_file()
                || metadata.uid() != rustix::process::geteuid().as_raw()
                || metadata.mode() & 0o7777 != 0o600
                || metadata.nlink() != 1
                || metadata.len() == 0
                || metadata.len() > max_bytes
            {
                return Err(SourceCommandError::Conflict(
                    "failed child candidate is unsafe to clean",
                ));
            }
            let identity = (metadata.dev(), metadata.ino());
            drop(file);
            remove_stage_file(isolated, leaf, identity, deadline, cancelled)?;
        }
        Err(_) => {}
    }
    isolated.cleanup_empty(deadline, cancelled)
}

fn preserve_cleanup_failures(
    primary: SourceCommandError,
    export_cleanup: Option<SourceCommandError>,
    stage_cleanup: Option<SourceCommandError>,
) -> SourceCommandError {
    let mut details = Vec::new();
    if let Some(error) = export_cleanup {
        details.push(format!("host export: {}", error.public_reason()));
    }
    if let Some(error) = stage_cleanup {
        details.push(format!("child stage: {}", error.public_reason()));
    }
    if details.is_empty() {
        primary
    } else {
        SourceCommandError::DeniedWithReason(format!(
            "{}; cleanup failed: {}",
            primary.public_reason(),
            details.join("; ")
        ))
    }
}

fn cleanup_partial_source_export(
    isolated: &crate::source_creation_store::IsolatedCreationRoot,
    expected_normalization: Option<
        super::source_navigation_model_cleanup::ExpectedNormalizationInputV1,
    >,
    limits: super::source_navigation_model_cleanup::ManagedSourceCutCleanupLimitsV1,
) -> SourceCommandResult<()> {
    let cleanup_deadline = Instant::now() + Duration::from_millis(HOST_FINALIZATION_RESERVE_MS);
    let cleanup_cancelled = AtomicBool::new(false);
    let manifest =
        super::source_navigation_model_cleanup::capture_partial_managed_source_cut_cleanup_v1(
            isolated,
            expected_normalization,
            limits,
            cleanup_deadline,
            &cleanup_cancelled,
        )?;
    super::source_navigation_model_cleanup::cleanup_managed_source_cut_export_v1(
        isolated,
        manifest,
        cleanup_deadline,
        &cleanup_cancelled,
    )?;
    isolated.cleanup_empty(cleanup_deadline, &cleanup_cancelled)
}

fn cleanup_captured_source_export(
    isolated: &crate::source_creation_store::IsolatedCreationRoot,
    manifest: super::source_navigation_model_cleanup::ManagedSourceCutCleanupManifestV1,
) -> SourceCommandResult<()> {
    let cleanup_deadline = Instant::now() + Duration::from_millis(HOST_FINALIZATION_RESERVE_MS);
    let cleanup_cancelled = AtomicBool::new(false);
    super::source_navigation_model_cleanup::cleanup_managed_source_cut_export_v1(
        isolated,
        manifest,
        cleanup_deadline,
        &cleanup_cancelled,
    )?;
    isolated.cleanup_empty(cleanup_deadline, &cleanup_cancelled)
}

fn preserve_partial_source_cleanup_failure(
    primary: SourceCommandError,
    isolated: &crate::source_creation_store::IsolatedCreationRoot,
    expected_normalization: Option<
        super::source_navigation_model_cleanup::ExpectedNormalizationInputV1,
    >,
    limits: super::source_navigation_model_cleanup::ManagedSourceCutCleanupLimitsV1,
) -> SourceCommandError {
    preserve_cleanup_failures(
        primary,
        cleanup_partial_source_export(isolated, expected_normalization, limits).err(),
        None,
    )
}

fn record_export_object(
    objects: &mut BTreeMap<String, u64>,
    digest: &str,
    size: u64,
) -> SourceCommandResult<()> {
    match objects.get(digest) {
        Some(previous) if *previous != size => Err(SourceCommandError::Conflict(
            "retained source object size conflicts",
        )),
        Some(_) => Ok(()),
        None => {
            objects.insert(digest.to_owned(), size);
            Ok(())
        }
    }
}

fn record_export_object_with_unknown_size(
    objects: &mut BTreeMap<String, u64>,
    digest: &str,
    maximum_size: u64,
) {
    objects.entry(digest.to_owned()).or_insert(maximum_size);
}

/// Preflight the selected explicit source export and its retained history.
/// The current-generation envelope comes from the protected model owner;
/// ordinary command batch ceilings do not define this export. Retirement payloads are
/// charged at their protected selected-object ceiling because v1 metadata
/// does not carry their exact byte length.
fn preflight_host_export(
    original: &CorpusCutReader,
    limits: ModelLimits,
    normalization_bytes: u64,
    manifest_cap: u64,
) -> SourceCommandResult<(u64, u64)> {
    let mut objects = BTreeMap::new();
    let mut revisions = 0u64;
    let mut retained_ids = 0u64;
    for snapshot in original.revisions() {
        revisions = revisions.checked_add(1).ok_or(SourceCommandError::Invalid(
            "host export revision count overflow",
        ))?;
        for member in snapshot.members() {
            record_export_object(&mut objects, &member.sha256.to_hex(), member.size_bytes)?;
            retained_ids = retained_ids
                .checked_add(1)
                .ok_or(SourceCommandError::Invalid(
                    "host export retained object count overflow",
                ))?;
        }
        for retired in snapshot.retirements() {
            record_export_object_with_unknown_size(
                &mut objects,
                &retired.sha256.to_hex(),
                limits.max_source_total_bytes,
            );
            record_export_object(
                &mut objects,
                &retired.event_sha256.to_hex(),
                retired.event_size_bytes,
            )?;
            retained_ids = retained_ids
                .checked_add(2)
                .ok_or(SourceCommandError::Invalid(
                    "host export retirement count overflow",
                ))?;
        }
    }
    let retained_bytes = objects
        .values()
        .try_fold(0u64, |total, size| total.checked_add(*size))
        .ok_or(SourceCommandError::Invalid(
            "host export retained byte count overflow",
        ))?;
    let source_writer_bytes = limits.max_source_total_bytes;
    let source_writer_members = limits.max_source_members;
    // Include one concurrent same-size temporary used by atomic object
    // installation, each copied/derived manifest, the compiler input, and
    // conservative directory/inode overhead.
    let manifest_bytes = revisions
        .checked_add(1)
        .and_then(|count| count.checked_mul(manifest_cap))
        .ok_or(SourceCommandError::Invalid(
            "host export manifest byte count overflow",
        ))?;
    let bytes = retained_bytes
        .checked_add(
            source_writer_bytes
                .checked_mul(2)
                .ok_or(SourceCommandError::Invalid(
                    "host export staging byte count overflow",
                ))?,
        )
        .and_then(|n| n.checked_add(manifest_bytes))
        .and_then(|n| n.checked_add(normalization_bytes))
        .ok_or(SourceCommandError::Invalid(
            "host export byte bound overflow",
        ))?;
    let inodes = u64::try_from(objects.len())
        .ok()
        .and_then(|n| n.checked_add(source_writer_members))
        .and_then(|n| n.checked_add(retained_ids))
        .and_then(|n| n.checked_add(revisions.saturating_mul(2)))
        .and_then(|n| n.checked_add(8))
        .ok_or(SourceCommandError::Invalid(
            "host export inode bound overflow",
        ))?;
    if bytes > limits.host_export_bytes || inodes > limits.host_export_inodes {
        return Err(SourceCommandError::Unsupported(
            "selected source history exceeds host export reservation",
        ));
    }
    Ok((bytes, inodes))
}

fn stage_normalization_input(
    isolated: &crate::source_creation_store::IsolatedCreationRoot,
    raw: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PathBuf> {
    active(deadline, cancelled)?;
    let root = isolated.verify_current(deadline, cancelled)?;
    rustix::fs::mkdirat(&root, "inputs", rustix::fs::Mode::from_raw_mode(0o700))
        .map_err(|_| SourceCommandError::Conflict("source-navigation compiler input directory"))?;
    let inputs = tos_fd_open::open_directory_at(&root, Path::new("inputs"))
        .map_err(|_| SourceCommandError::Conflict("source-navigation compiler input directory"))?;
    let directory = inputs
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation compiler input metadata"))?;
    if !directory.is_dir()
        || directory.uid() != rustix::process::geteuid().as_raw()
        || directory.mode() & 0o7777 != 0o700
    {
        return Err(SourceCommandError::Conflict(
            "source-navigation compiler input directory changed",
        ));
    }
    let raw_fd = rustix::fs::openat(
        &inputs,
        "knowledge_normalization.rs",
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o400),
    )
    .map_err(|_| {
        SourceCommandError::Conflict("source-navigation normalization input already exists")
    })?;
    let mut file = File::from(raw_fd);
    let initial = file.metadata().map_err(|_| {
        SourceCommandError::Invalid("source-navigation normalization input metadata")
    })?;
    let identity = (initial.dev(), initial.ino());
    let write_result = (|| -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        file.write_all(raw)
            .and_then(|()| file.sync_all())
            .map_err(|_| {
                SourceCommandError::Invalid("source-navigation normalization input write")
            })?;
        let metadata = file.metadata().map_err(|_| {
            SourceCommandError::Invalid("source-navigation normalization input metadata")
        })?;
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o7777 != 0o400
            || metadata.nlink() != 1
            || metadata.len() != raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "source-navigation normalization input differs",
            ));
        }
        inputs
            .sync_all()
            .and_then(|()| root.sync_all())
            .map_err(|_| {
                SourceCommandError::Invalid("source-navigation compiler input directory sync")
            })?;
        Ok(())
    })();
    if let Err(primary) = write_result {
        let cleanup = remove_staged_normalization_input(&inputs, &file, identity);
        return Err(preserve_cleanup_failures(primary, cleanup.err(), None));
    }
    Ok(isolated.path().join("inputs/knowledge_normalization.rs"))
}

fn remove_staged_normalization_input(
    inputs: &File,
    held_file: &File,
    expected_identity: (u64, u64),
) -> SourceCommandResult<()> {
    let uid = rustix::process::geteuid().as_raw();
    let held = held_file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("normalization cleanup held metadata"))?;
    let named = tos_fd_open::open_regular_at(inputs, Path::new("knowledge_normalization.rs"))
        .map_err(|_| SourceCommandError::Conflict("normalization cleanup file replaced"))?;
    let metadata = named
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("normalization cleanup named metadata"))?;
    if !held.is_file()
        || !metadata.is_file()
        || (held.dev(), held.ino()) != expected_identity
        || (metadata.dev(), metadata.ino()) != expected_identity
        || held.uid() != uid
        || metadata.uid() != uid
        || held.mode() & 0o7777 != 0o400
        || metadata.mode() & 0o7777 != 0o400
        || held.nlink() != 1
        || metadata.nlink() != 1
    {
        return Err(SourceCommandError::Conflict(
            "normalization cleanup file identity changed",
        ));
    }
    drop(named);
    rustix::fs::unlinkat(
        inputs,
        "knowledge_normalization.rs",
        rustix::fs::AtFlags::empty(),
    )
    .map_err(|_| SourceCommandError::Conflict("normalization cleanup unlink failed"))?;
    inputs
        .sync_all()
        .map_err(|_| SourceCommandError::Invalid("normalization cleanup directory fsync"))
}

const OWNER_SCHEMA: &str = "tos_local_source_navigation_model_prepare_owner_v1";
const REQUEST_SCHEMA: &str = "tos_local_source_navigation_model_prepare_request_v1";
const RESULT_SCHEMA: &str = "tos_local_source_navigation_model_prepare_result_v1";
const SELECTION_SCHEMA: &str = "tos_local_pg_source_navigation_selection_v1";
const FINAL_OWNER_SCHEMA: &str = "tos_local_source_navigation_owner_v1";
const MAX_OWNER_BYTES: usize = 1_048_576;
const NORMALIZATION_PROCESSOR_PATH: &str =
    "rust/crates/tos-compiler/src/knowledge_normalization.rs";
const ENTITY_REGISTRY_PATH: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION_REGISTRY_PATH: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const QUERY_VOCABULARY_PATH: &str = "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareOwner {
    schema_version: String,
    uid: u64,
    source_root: String,
    expires_at: String,
    host_export_stage_parent: String,
    bootstrap_invocation_path: String,
    bootstrap_request_path: String,
    model_leaf: String,
    final_owner_leaf: String,
    source_navigation: Value,
    policy_template: PolicyTemplate,
    model_limits: ModelLimits,
    launcher: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyTemplate {
    schema_version: String,
    issuer_ref: String,
    receipt_id: String,
    carrier_layer: String,
    intended_use: String,
    model_abi: String,
    selection_profile: String,
    policy_epoch: u64,
    withdrawal_generation: u64,
    expires_at: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelLimits {
    max_total_ms: u64,
    host_export_bytes: u64,
    host_export_inodes: u64,
    tmpfs_bytes: u64,
    tmpfs_inodes: u64,
    working_ram_bytes: u64,
    max_source_members: u64,
    max_source_total_bytes: u64,
    max_source_member_bytes: u64,
    max_plan_bytes: usize,
    max_plan_work_bytes: u64,
    max_model_bytes: u64,
}

fn invalid<T>(message: &'static str) -> SourceCommandResult<T> {
    Err(SourceCommandError::Invalid(message))
}

fn exact_keys(value: &Value, keys: &[&str], message: &'static str) -> SourceCommandResult<()> {
    let object = value
        .as_object()
        .ok_or(SourceCommandError::Invalid(message))?;
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return invalid(message);
    }
    Ok(())
}

fn safe_leaf(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn validate_model_limits(limits: ModelLimits) -> SourceCommandResult<()> {
    if !(1_000..=INITIAL_MAX_TOTAL_MS).contains(&limits.max_total_ms)
        || !(16 * 1024 * 1024..=2 * 1024 * 1024 * 1024).contains(&limits.host_export_bytes)
        || !(32..=1_000_000).contains(&limits.host_export_inodes)
        || !(64 * 1024 * 1024..=8 * 1024 * 1024 * 1024).contains(&limits.tmpfs_bytes)
        || !(128..=1_000_000).contains(&limits.tmpfs_inodes)
        || !(512 * 1024 * 1024..=16 * 1024 * 1024 * 1024).contains(&limits.working_ram_bytes)
        || limits.max_source_members == 0
        || limits.max_source_members > limits.host_export_inodes
        || limits.max_source_total_bytes == 0
        || limits.max_source_total_bytes > limits.host_export_bytes
        || limits.max_source_member_bytes == 0
        || limits.max_source_member_bytes > limits.max_source_total_bytes
        || limits.max_source_member_bytes > limits.working_ram_bytes / 8
        || limits.max_plan_bytes == 0
        || limits.max_plan_bytes > 64 * 1024 * 1024
        || limits.max_plan_work_bytes == 0
        || limits.max_plan_work_bytes > 512 * 1024 * 1024
        || limits.max_model_bytes == 0
        || limits.max_model_bytes > 512 * 1024 * 1024
    {
        return invalid("source-navigation model limits outside supported envelope");
    }
    Ok(())
}

fn initial_source_read_limits(limits: ModelLimits) -> SourceCommandResult<ReadLimits> {
    validate_model_limits(limits)?;
    let manifest_bytes = usize::try_from(limits.max_source_members.saturating_mul(1024))
        .map_err(|_| SourceCommandError::Invalid("initial export manifest bound"))?
        .clamp(
            1_048_576,
            crate::source_current_cut::INITIAL_EXPORT_MANIFEST_BYTES,
        );
    Ok(ReadLimits {
        max_manifest_bytes: manifest_bytes,
        max_manifest_entries: crate::source_current_cut::INITIAL_EXPORT_MANIFEST_ENTRIES,
        max_selected_object_bytes: limits.max_source_member_bytes,
        json: tos_foundation::JsonLimits {
            max_bytes: manifest_bytes,
            max_visits: crate::source_current_cut::INITIAL_EXPORT_JSON_VISITS,
            ..tos_foundation::JsonLimits::default()
        },
    })
}

/// Only the explicit local initial-export route uses these protected owner
/// bounds. This preflight grants no mutation authority; execute reselects the
/// protected filesystem owner and validates its full scope before doing work.
pub(super) fn initial_invocation_limits(
    owner_raw: &[u8],
    uid: u32,
) -> SourceCommandResult<(ReadLimits, CutReadLimits, u64)> {
    let owner_value = cmd::parse(owner_raw)?;
    let owner: PrepareOwner = serde_json::from_slice(&cmd::canonical(&owner_value)?)
        .map_err(|_| SourceCommandError::Invalid("source-navigation preparation owner"))?;
    if owner.schema_version != OWNER_SCHEMA || owner.uid != u64::from(uid) {
        return Err(SourceCommandError::Denied(
            "source-navigation preparation owner scope",
        ));
    }
    let limits = owner.model_limits;
    let read = initial_source_read_limits(limits)?;
    let revisions = super::source_navigation_model_compile::MAX_SOURCE_HISTORY_REVISIONS;
    let cut = CutReadLimits {
        max_revisions: revisions,
        max_members: limits
            .max_source_members
            .saturating_mul(revisions as u64)
            .min(crate::source_current_cut::INITIAL_EXPORT_MANIFEST_ENTRIES as u64),
        max_total_bytes: limits
            .max_source_total_bytes
            .saturating_mul(revisions as u64)
            .min(crate::source_current_cut::INITIAL_EXPORT_HISTORY_BYTES),
        max_member_bytes: limits.max_source_member_bytes,
    };
    Ok((read, cut, limits.max_total_ms))
}

fn validate_launcher(value: &Value) -> SourceCommandResult<()> {
    exact_keys(
        value,
        &[
            "tos_access_path",
            "tos_access_sha256",
            "unshare_path",
            "unshare_sha256",
            "setup_cgroup",
            "consumer_cgroup",
            "scratch_parent",
            "persistent_staging_root",
            "shutdown_grace_ms",
        ],
        "source-navigation model launcher fields",
    )?;
    for key in [
        "tos_access_path",
        "unshare_path",
        "scratch_parent",
        "persistent_staging_root",
        "setup_cgroup",
        "consumer_cgroup",
    ] {
        let path = PathBuf::from(super::text(value, key)?);
        if !path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(SourceCommandError::Invalid(
                "source-navigation model launcher path",
            ));
        }
    }
    for key in ["tos_access_sha256", "unshare_sha256"] {
        Digest256::from_hex(super::text(value, key)?).map_err(|_| {
            SourceCommandError::Invalid("source-navigation launcher executable digest")
        })?;
    }
    selected_positive(value, "shutdown_grace_ms", 60_000)?;
    Ok(())
}

fn validate_selection_input(value: &Value) -> SourceCommandResult<()> {
    exact_keys(
        value,
        &[
            "schema_version",
            "domain",
            "source_contract_sha256",
            "rule_version",
            "rights_version",
            "job_id",
            "job_fence",
            "consumer_id",
            "profile_id",
            "profile_version",
            "store_root",
            "store_limits",
            "tree_limits",
            "process_limits",
            "cold_limits",
            "producer",
        ],
        "source-navigation model selection fields",
    )?;
    if value["schema_version"].as_str() != Some(SELECTION_SCHEMA)
        || value["domain"].as_str().is_none_or(str::is_empty)
        || value["job_id"].as_str().is_none_or(str::is_empty)
        || value["consumer_id"].as_str().is_none_or(str::is_empty)
        || value["profile_id"].as_str() != Some(tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE)
        || value["producer"].is_null()
    {
        return Err(SourceCommandError::Denied(
            "source-navigation model selection scope",
        ));
    }
    let producer = &value["producer"];
    exact_keys(
        producer,
        &[
            "schema_version",
            "event_path",
            "requested_paths",
            "schema_paths",
            "schema_worker_path",
            "schema_worker_sha256",
            "schema_execution_wall_ms",
            "schema_cleanup_grace_ms",
            "schema_cpu_seconds",
            "schema_address_space_bytes",
            "schema_max_receipts",
            "schema_max_receipt_bytes",
            "max_events",
            "event_limits",
            "aggregate_limits",
        ],
        "source-navigation producer fields",
    )?;
    exact_keys(
        &producer["event_limits"],
        &[
            "max_dependencies",
            "max_cohort_rows",
            "max_current_cohorts",
            "max_current_rows",
            "max_adjacency_rows",
            "max_adjacency_bytes",
            "max_row_bytes",
            "max_total_bytes",
            "statement_timeout_ms",
        ],
        "source-navigation producer event limits",
    )?;
    exact_keys(
        &producer["aggregate_limits"],
        &[
            "max_total_rows",
            "max_total_encoded_bytes",
            "max_total_dependency_bytes",
            "max_total_work_units",
        ],
        "source-navigation producer aggregate limits",
    )?;
    if producer["schema_version"] != "tos_local_pg_source_navigation_producer_v1" {
        return Err(SourceCommandError::Denied(
            "source-navigation producer profile",
        ));
    }
    Ok(())
}

fn validate_policy_template(
    template: &PolicyTemplate,
    expires_at: &str,
) -> SourceCommandResult<()> {
    if template.schema_version != "tos_source_navigation_public_metadata_policy_template_v1"
        || template.issuer_ref.is_empty()
        || template.issuer_ref.len() > 4096
        || !template.issuer_ref.starts_with("ToS/")
        || template.receipt_id.is_empty()
        || template.receipt_id.len() > 512
        || template
            .receipt_id
            .bytes()
            .any(|byte| byte.is_ascii_control())
        || template.carrier_layer != crate::source_navigation_policy::CARRIER_LAYER
        || template.intended_use != crate::source_navigation_policy::INTENDED_USE
        || template.model_abi != tos_query::SOURCE_NAVIGATION_MODEL_ABI
        || template.selection_profile != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
        || template.policy_epoch == 0
        || template.expires_at != expires_at
    {
        return Err(SourceCommandError::Denied(
            "source-navigation explicit policy template scope",
        ));
    }
    Ok(())
}

fn selected_positive(value: &Value, key: &str, cap: u64) -> SourceCommandResult<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|number| *number > 0 && *number <= cap)
        .ok_or(SourceCommandError::Invalid(
            "selected source-navigation producer budget",
        ))
}

fn hash_configuration(parts: &[&[u8]]) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-source-navigation-initial-model-configuration-v1\0");
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize()
}

fn require_compiled_generation_current(
    lease: &crate::source_managed_selection::SourceNavigationCurrentLeaseV1<'_>,
    generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
    proof: &tos_compiler::managed_source::ManagedSourceProofV2,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
) -> std::result::Result<(), crate::DurableError> {
    // V2 publication stores the authenticated addressed inventory root in
    // cmd2_domain. The separately retained catalogue transcript has its own
    // inventory_projection_sha256 and remains part of the compiler proof.
    let expected_projection = Digest256::from_hex(proof.native_inventory_root())
        .map_err(|_| crate::DurableError::Invalid("compiled addressed inventory digest"))?;
    if lease.head_sequence() != generation.commit_seq()
        || lease.source_generation() != generation.commit_seq()
        || lease.audit_generation() != generation.audit_generation()
        || lease.audit_generation() != proof.generation.selected_audit_generation
        || lease.database_oid() != proof.generation.database_oid
        || !lease.rights_allowed()
        || lease.rights_version() != rights_version
        || lease.rule_version() != rule_version
        || lease.source_contract() != contract
        || lease.selected_generation_digest() != Some(generation.digest())
        || lease.source_projection_digest() != Some(expected_projection)
    {
        return Err(crate::DurableError::Conflict(
            "compiled source-navigation generation is no longer current",
        ));
    }
    lease
        .require_managed_v2_basis(proof)
        .map_err(|_| crate::DurableError::Conflict("compiled source-navigation proof changed"))
}

/// Native owner entry. This handler is intentionally separate from regular
/// Source commands: its request has no paths, proof, policy, worker or budget.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    uid: u32,
    original_store: &CorpusReader,
    original_cut: &CorpusCutReader,
    original_software: &SoftwareCaptureReader,
    original_components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> SourceCommandResult<Value> {
    if super::text(invocation, "schema_version")? != "tos_local_native_source_invocation_v1" {
        return Err(SourceCommandError::Denied(
            "source-navigation model invocation",
        ));
    }
    let request_source = cmd::parse(request_raw)?;
    let request = super::pg_bootstrap::value(&request_source)?;
    exact_keys(
        &request,
        &["schema_version", "operation"],
        "model preparation request",
    )?;
    if super::text(&request, "schema_version")? != REQUEST_SCHEMA
        || super::text(&request, "operation")? != "prepare-initial"
    {
        return Err(SourceCommandError::Unsupported(
            "source-navigation model operation",
        ));
    }
    let owner_path = super::absolute(super::text(invocation, "owner_config")?)?;
    let (filesystem, owner_raw) =
        crate::source_creation_store::CreationFilesystem::select_protected_native_owner_bounded(
            &owner_path,
            MAX_OWNER_BYTES,
            deadline,
            cancelled,
        )?;
    let owner_value = cmd::parse(&owner_raw)?;
    let owner: PrepareOwner = serde_json::from_slice(&cmd::canonical(&owner_value)?)
        .map_err(|_| SourceCommandError::Invalid("source-navigation preparation owner"))?;
    if owner.schema_version != OWNER_SCHEMA
        || owner.uid != u64::from(uid)
        || filesystem.protected_uid() != uid
        || super::absolute(&owner.source_root)? != filesystem.protected_root_path()
        || !safe_leaf(&owner.model_leaf)
        || !safe_leaf(&owner.final_owner_leaf)
        || owner.model_leaf == owner.final_owner_leaf
        || owner_path.parent() != Some(filesystem.protected_root_path())
    {
        return Err(SourceCommandError::Denied(
            "source-navigation preparation owner scope",
        ));
    }
    validate_model_limits(owner.model_limits)?;
    validate_selection_input(&owner.source_navigation)?;
    validate_policy_template(&owner.policy_template, &owner.expires_at)?;
    validate_launcher(&owner.launcher)?;
    let host_export_parent = super::absolute(&owner.host_export_stage_parent)?;
    let source_root = filesystem.protected_root_path();
    if host_export_parent == source_root
        || !host_export_parent.starts_with(source_root)
        || host_export_parent.starts_with(source_root.join("ToS"))
    {
        return Err(SourceCommandError::Denied(
            "source-navigation host export stage scope",
        ));
    }
    let persistent_stage = PathBuf::from(super::text(&owner.launcher, "persistent_staging_root")?);
    if persistent_stage.starts_with(&host_export_parent)
        || host_export_parent.starts_with(&persistent_stage)
    {
        return Err(SourceCommandError::Denied(
            "host export and child artifact staging overlap",
        ));
    }
    let persistent_parent = tos_fd_open::open_absolute_directory(&persistent_stage)
        .map_err(|_| SourceCommandError::Denied("source-navigation persistent stage parent"))?;
    let persistent_metadata = persistent_parent
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation persistent stage metadata"))?;
    if !persistent_metadata.is_dir()
        || persistent_metadata.uid() != uid
        || persistent_metadata.mode() & 0o7777 != 0o700
    {
        return Err(SourceCommandError::Denied(
            "source-navigation persistent stage owner",
        ));
    }
    let export_parent = tos_fd_open::open_absolute_directory(&host_export_parent)
        .map_err(|_| SourceCommandError::Denied("source-navigation host export stage parent"))?;
    let export_parent_metadata = export_parent
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation host export stage metadata"))?;
    if !export_parent_metadata.is_dir()
        || export_parent_metadata.uid() != uid
        || export_parent_metadata.mode() & 0o7777 != 0o700
    {
        return Err(SourceCommandError::Denied(
            "source-navigation host export stage owner",
        ));
    }
    // Validate expiry and selected owner once before doing source or compiler
    // work. The same package-free fence is reacquired for final publication.
    drop(filesystem.hold_source_navigation_owner(deadline, cancelled)?);

    prepare_initial(
        invocation_path,
        invocation_raw,
        invocation,
        request_raw,
        owner_path,
        owner_raw,
        owner,
        filesystem,
        uid,
        original_store,
        original_cut,
        original_software,
        original_components,
        deadline,
        cancelled,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_initial(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    owner_path: PathBuf,
    owner_raw: Vec<u8>,
    owner: PrepareOwner,
    filesystem: crate::source_creation_store::CreationFilesystem,
    uid: u32,
    original_store: &CorpusReader,
    original_cut: &CorpusCutReader,
    original_software: &SoftwareCaptureReader,
    original_components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> SourceCommandResult<Value> {
    let selection = PreparationSelection::parse(&owner.source_navigation)?;
    // Rule and rights versions belong to the selected CMD domain. Fresh
    // domains start at zero; the publication lease checks exact currentness.
    if selection.profile_version != 1
        || selection.profile_id != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
    {
        return Err(SourceCommandError::Denied(
            "source-navigation initial profile",
        ));
    }
    let bootstrap_invocation_path = super::absolute(&owner.bootstrap_invocation_path)?;
    let bootstrap_request_path = super::absolute(&owner.bootstrap_request_path)?;
    let bootstrap_invocation_raw = super::read_absolute(
        &bootstrap_invocation_path,
        uid,
        true,
        MAX_OWNER_BYTES,
        deadline,
        cancelled,
    )?;
    let bootstrap_invocation_source = cmd::parse(&bootstrap_invocation_raw)?;
    let bootstrap_invocation = super::pg_bootstrap::value(&bootstrap_invocation_source)?;
    exact_keys(
        &bootstrap_invocation,
        &[
            "budgets",
            "corpus_store",
            "native_executable",
            "native_executable_sha256",
            "owner_config",
            "schema_version",
            "schema_worker",
            "software_capture",
            "software_components",
            "software_restored_root",
            "software_selection",
            "source_revision",
            "source_selection",
            "streamed_budgets",
            "workspace_root",
        ],
        "retained streamed bootstrap invocation fields",
    )?;
    if super::text(&bootstrap_invocation, "schema_version")?
        != "tos_local_pg_source_bootstrap_streamed_invocation_v1"
    {
        return Err(SourceCommandError::Denied(
            "retained streamed bootstrap invocation schema",
        ));
    }
    let bootstrap_request_raw = super::read_absolute(
        &bootstrap_request_path,
        uid,
        true,
        64 * 1024,
        deadline,
        cancelled,
    )?;
    let software_total = original_components
        .members()
        .try_fold(0u64, |total, member| total.checked_add(member.size_bytes))
        .ok_or(SourceCommandError::Invalid(
            "selected software size overflow",
        ))?;
    let software_member_cap = original_components
        .members()
        .map(|member| member.size_bytes)
        .max()
        .ok_or(SourceCommandError::Invalid(
            "selected software components absent",
        ))?;
    let bootstrap_start = Instant::now();
    let bootstrap = super::pg_bootstrap::prepare_protected_bootstrap(
        &bootstrap_invocation,
        Cursor::new(&bootstrap_request_raw),
        uid,
        super::pg_bootstrap::SoftwareInputLimits {
            max_members: original_components.members().count() as u64,
            max_total_bytes: software_total,
            max_member_bytes: software_member_cap,
        },
        deadline,
        cancelled,
    )?;
    let bootstrap_source_revision = original_cut.current().revision();
    let source_membership = original_cut
        .stream(bootstrap_source_revision)
        .map_err(|_| SourceCommandError::Conflict("selected bootstrap source cut"))?
        .expectation();
    let outer_owner_context = super::absolute(super::text(invocation, "owner_context")?)?;
    let bootstrap_owner_path =
        super::absolute(super::text(&bootstrap_invocation, "owner_config")?)?;
    // These operations may select separate job leases: the bootstrap owner is
    // checked below and navigation at publication. Compare their source binding only.
    if bootstrap.selection.source_revision != bootstrap_source_revision
        || Digest256::from_prefixed(super::text(&bootstrap_invocation, "source_revision")?)
            .map_err(|_| SourceCommandError::Invalid("retained bootstrap source revision"))?
            != bootstrap_source_revision.0
        || bootstrap.selection.membership != source_membership
        || bootstrap.selection.domain != selection.domain
        || bootstrap.selection.contract.to_hex() != selection.source_contract_sha256
        || bootstrap.selection.store_root != selection.store_root
        || outer_owner_context != bootstrap_owner_path
        || bootstrap.software.selection() != original_software.selection()
        || bootstrap
            .components
            .members()
            .ne(original_components.members())
    {
        return Err(SourceCommandError::Conflict(
            "retained bootstrap selection differs",
        ));
    }
    bootstrap
        .owner_check(
            &bootstrap_invocation_path,
            &bootstrap_invocation_raw,
            uid,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("retained bootstrap owner changed"))?;

    let normalization_relative = RelativePath::parse(NORMALIZATION_PROCESSOR_PATH)
        .map_err(|_| SourceCommandError::Invalid("normalization processor path"))?;
    let normalization_raw = original_software
        .read_selected_component(
            original_components,
            &normalization_relative,
            8 * 1024 * 1024,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("selected normalization processor absent"))?;
    let normalization_member = original_components
        .member(&normalization_relative)
        .filter(|member| {
            member.size_bytes == normalization_raw.len() as u64
                && member.sha256 == Digest256::of_bytes(&normalization_raw)
        })
        .ok_or(SourceCommandError::Conflict(
            "selected normalization processor changed",
        ))?;
    let export_read_limits = initial_source_read_limits(owner.model_limits)?;
    let (host_export_bytes, host_export_inodes) = preflight_host_export(
        original_cut,
        owner.model_limits,
        normalization_member.size_bytes,
        export_read_limits.max_manifest_bytes as u64,
    )?;
    let cleanup_max_entries = usize::try_from(host_export_inodes.min(
        super::source_navigation_model_cleanup::MAX_MANAGED_SOURCE_CUT_CLEANUP_ENTRIES_V1 as u64,
    ))
    .map_err(|_| SourceCommandError::Invalid("source-navigation cleanup entry cap"))?;
    if usize::try_from(host_export_inodes)
        .map_err(|_| SourceCommandError::Invalid("source-navigation cleanup entry cap"))?
        > super::source_navigation_model_cleanup::MAX_MANAGED_SOURCE_CUT_CLEANUP_ENTRIES_V1
    {
        return Err(SourceCommandError::Unsupported(
            "selected source history exceeds cleanup entry reservation",
        ));
    }
    let host_export_parent = super::absolute(&owner.host_export_stage_parent)?;
    let invocation_worker = &invocation["schema_worker"];
    let producer_worker = &owner.source_navigation["producer"];
    if super::absolute(super::text(invocation_worker, "absolute_path")?)?
        != super::absolute(super::text(producer_worker, "schema_worker_path")?)?
        || Digest256::from_prefixed(super::text(invocation_worker, "sha256")?)
            .map_err(|_| SourceCommandError::Invalid("native schema worker identity"))?
            != Digest256::from_hex(super::text(producer_worker, "schema_worker_sha256")?)
                .map_err(|_| SourceCommandError::Invalid("navigation schema worker identity"))?
    {
        return Err(SourceCommandError::Denied(
            "initial and navigation schema worker identities differ",
        ));
    }
    let store =
        SegmentStore::open_existing(&selection.store_root, selection.store_limits.into_limits())
            .map_err(|_| SourceCommandError::Conflict("selected source-navigation store"))?;
    if store.custody_domain() != selection.domain.as_bytes() {
        return Err(SourceCommandError::Conflict(
            "selected source-navigation domain",
        ));
    }
    let mut coordinator = crate::DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );

    // Validate all owner-controlled compile settings before creating the
    // random host export root, whose first nonempty state is source-owned data.
    let producer = &owner.source_navigation["producer"];
    let event_limits = &producer["event_limits"];
    let aggregate_limits = &producer["aggregate_limits"];
    let max_events = selected_positive(producer, "max_events", 256)?;
    let event_dependencies = selected_positive(event_limits, "max_dependencies", 4096)?;
    let event_rows = selected_positive(event_limits, "max_current_rows", 65_536)?;
    let event_row_bytes = selected_positive(event_limits, "max_row_bytes", 8 * 1024 * 1024)?;
    let event_total_bytes = selected_positive(event_limits, "max_total_bytes", 512 * 1024 * 1024)?;
    let schema_wall_ms = selected_positive(producer, "schema_execution_wall_ms", 300_000)?;
    let schema_cleanup_ms = selected_positive(producer, "schema_cleanup_grace_ms", 60_000)?;
    let schema_cpu_seconds = selected_positive(producer, "schema_cpu_seconds", 60)?;
    let schema_address_space = selected_positive(
        producer,
        "schema_address_space_bytes",
        8 * 1024 * 1024 * 1024,
    )?;
    let schema_max_receipts = selected_positive(producer, "schema_max_receipts", 100_000)?;
    let schema_max_receipt_bytes =
        selected_positive(producer, "schema_max_receipt_bytes", 64 * 1024 * 1024)?;
    let max_total_rows = selected_positive(aggregate_limits, "max_total_rows", u64::MAX - 1)?;
    let max_total_encoded_bytes =
        selected_positive(aggregate_limits, "max_total_encoded_bytes", u64::MAX - 1)?;
    let max_total_dependency_bytes =
        selected_positive(aggregate_limits, "max_total_dependency_bytes", u64::MAX - 1)?;
    let max_total_work_units =
        selected_positive(aggregate_limits, "max_total_work_units", u64::MAX - 1)?;
    let schema_worker_identity = ExactWorkerIdentity {
        absolute_path: super::absolute(super::text(
            &invocation["schema_worker"],
            "absolute_path",
        )?)?,
        sha256: Digest256::from_prefixed(super::text(&invocation["schema_worker"], "sha256")?)
            .map_err(|_| SourceCommandError::Invalid("source-navigation schema worker digest"))?,
    };
    let selected_worker_path = super::absolute(super::text(producer, "schema_worker_path")?)?;
    let selected_worker_digest =
        Digest256::from_hex(super::text(producer, "schema_worker_sha256")?).map_err(|_| {
            SourceCommandError::Invalid("selected source-navigation schema worker digest")
        })?;
    if schema_worker_identity.absolute_path != selected_worker_path
        || schema_worker_identity.sha256 != selected_worker_digest
    {
        return Err(SourceCommandError::Denied(
            "source-navigation producer worker differs",
        ));
    }
    selection
        .process_limits
        .verify_current()
        .map_err(|_| SourceCommandError::Denied("selected native process limits differ"))?;
    let launcher = super::source_navigation_model_process::SourceNavigationModelLauncherV1 {
        tos_access_path: PathBuf::from(super::text(&owner.launcher, "tos_access_path")?),
        tos_access_sha256: super::text(&owner.launcher, "tos_access_sha256")?.to_owned(),
        unshare_path: PathBuf::from(super::text(&owner.launcher, "unshare_path")?),
        unshare_sha256: super::text(&owner.launcher, "unshare_sha256")?.to_owned(),
        setup_cgroup: PathBuf::from(super::text(&owner.launcher, "setup_cgroup")?),
        consumer_cgroup: PathBuf::from(super::text(&owner.launcher, "consumer_cgroup")?),
        scratch_parent: PathBuf::from(super::text(&owner.launcher, "scratch_parent")?),
        persistent_staging_root: PathBuf::from(super::text(
            &owner.launcher,
            "persistent_staging_root",
        )?),
        shutdown_grace_ms: selected_positive(&owner.launcher, "shutdown_grace_ms", 60_000)?,
    };
    let process_limits =
        super::source_navigation_model_process::SourceNavigationModelProcessLimits {
            max_total_ms: owner.model_limits.max_total_ms,
            tmpfs_bytes: owner.model_limits.tmpfs_bytes,
            tmpfs_inodes: owner.model_limits.tmpfs_inodes,
            working_ram_bytes: owner.model_limits.working_ram_bytes,
            max_model_bytes: owner.model_limits.max_model_bytes,
        };

    let export_start = Instant::now();
    let isolated = crate::source_creation_store::IsolatedCreationRoot::create(
        &host_export_parent,
        deadline,
        cancelled,
    )?;
    let cleanup_limits = super::source_navigation_model_cleanup::ManagedSourceCutCleanupLimitsV1 {
        max_entries: cleanup_max_entries,
        max_total_bytes: host_export_bytes,
        max_file_bytes: host_export_bytes,
    };
    let expected_normalization =
        super::source_navigation_model_cleanup::ExpectedNormalizationInputV1 {
            size_bytes: normalization_member.size_bytes,
            sha256: normalization_member.sha256,
        };
    let staged_source = (|| {
        let generation = match coordinator
            .try_reopen_current_addressed_source_selection(
                &store,
                &selection.domain,
                bootstrap_source_revision,
                source_membership,
                selection.tree_limits.into_limits(),
                deadline,
                cancelled,
            )
            .map_err(crate::source_current_cut::durable)?
        {
            Some(generation) => generation,
            None => {
                // Initial preparation explicitly owns this full assessment.
                // Ordinary navigation updates never enter this route.
                let owner_check = || {
                    bootstrap.owner_check(
                        &bootstrap_invocation_path,
                        &bootstrap_invocation_raw,
                        uid,
                        deadline,
                        cancelled,
                    )?;
                    if super::read_absolute(
                        &owner_path,
                        uid,
                        true,
                        MAX_OWNER_BYTES,
                        deadline,
                        cancelled,
                    )
                    .map_err(crate::DurableError::Source)?
                        != owner_raw
                    {
                        return Err(crate::DurableError::Conflict(
                            "navigation preparation owner changed",
                        ));
                    }
                    drop(
                        filesystem
                            .hold_source_navigation_owner(deadline, cancelled)
                            .map_err(crate::DurableError::Source)?,
                    );
                    Ok(())
                };
                super::pg_bootstrap_streamed::reassess_existing_for_initial_model(
                    &bootstrap_invocation,
                    &bootstrap,
                    original_store,
                    &mut coordinator,
                    &store,
                    deadline,
                    cancelled,
                    &owner_check,
                )?
            }
        };
        let export = crate::source_current_cut::export_current_source_generation_addressed(
            &mut coordinator,
            &store,
            generation,
            original_cut,
            &isolated,
            export_read_limits,
            CutReadLimits {
                max_revisions: super::source_navigation_model_compile::MAX_SOURCE_HISTORY_REVISIONS,
                max_members: owner.model_limits.max_source_members,
                max_total_bytes: owner.model_limits.max_source_total_bytes,
                max_member_bytes: owner.model_limits.max_source_member_bytes,
            },
            deadline,
            cancelled,
        )?;
        bootstrap
            .owner_check(
                &bootstrap_invocation_path,
                &bootstrap_invocation_raw,
                uid,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Conflict("bootstrap changed during source selection")
            })?;
        let revision = export.cut.current().revision();
        let membership = export.membership;
        let projection = coordinator
            .visit_managed_model_catalogue_with_work(
                &export.generation,
                deadline,
                cancelled,
                &mut crate::ManagedSourceWorkV1::default(),
                |_| Ok(()),
            )
            .map_err(|_| SourceCommandError::Conflict("managed source projection incomplete"))?;
        let proof = <tos_compiler::managed_source::ManagedSourceProofV2 as crate::source_managed_selection::ManagedSelectedProof>::from_selected(
            &export.generation, projection, (&revision.0.to_hex(), &membership.digest.to_hex(), membership.count), None,
        ).map_err(|_| SourceCommandError::Conflict("managed source proof unavailable"))?;
        let current_source_bytes = export.source_bytes;
        if export.membership.count > owner.model_limits.max_source_members
            || current_source_bytes > owner.model_limits.max_source_total_bytes
        {
            return Err(SourceCommandError::Conflict(
                "selected source export exceeds configured limits",
            ));
        }
        let normalization_input_path =
            stage_normalization_input(&isolated, &normalization_raw, deadline, cancelled)?;
        let cleanup_manifest =
            super::source_navigation_model_cleanup::capture_managed_source_cut_cleanup_v1(
                &isolated,
                Some(expected_normalization),
                cleanup_limits,
                deadline,
                cancelled,
            )?;
        Ok((
            export,
            revision,
            membership,
            proof,
            current_source_bytes,
            normalization_input_path,
            cleanup_manifest,
        ))
    })();
    let (
        export,
        revision,
        membership,
        proof,
        current_source_bytes,
        normalization_input_path,
        cleanup_manifest,
    ) = match staged_source {
        Ok(staged) => staged,
        Err(primary) => {
            return Err(preserve_partial_source_cleanup_failure(
                primary,
                &isolated,
                Some(expected_normalization),
                cleanup_limits,
            ));
        }
    };
    let source_read_limits =
        super::source_navigation_model_compile::SourceNavigationCompileReadLimitsV1::from(
            export.read_limits,
        );
    let source_cut_limits =
        super::source_navigation_model_compile::SourceNavigationCompileCutLimitsV1::from(
            export.cut.limits(),
        );
    let source_binding = SourceBinding {
        owner_profile: "tos-managed-source-navigation-v2".into(),
        source_cut: revision.0.to_hex(),
        through_commit_seq: export.generation.commit_seq(),
        membership_root: membership.digest.to_hex(),
        index_generation: proof.generation.installed_generation_sha256.clone(),
        route_map_version: "tos-source-navigation-managed-current-v2".into(),
        reader_abi: tos_foundation::KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2.into(),
        projection_root_sha256: proof.generation.inventory_projection_sha256.clone(),
        complete: true,
    };

    let source_read_limits_raw = serde_json::to_vec(&source_read_limits).map_err(|_| {
        SourceCommandError::Invalid("source-navigation retained read limits encoding")
    })?;
    let source_cut_limits_raw = serde_json::to_vec(&source_cut_limits).map_err(|_| {
        SourceCommandError::Invalid("source-navigation retained cut limits encoding")
    })?;
    let configuration_digest = hash_configuration(&[
        owner_raw.as_slice(),
        invocation_raw,
        request_raw,
        &normalization_raw,
        proof.generation.installed_generation_sha256.as_bytes(),
        proof.generation.inventory_projection_sha256.as_bytes(),
        &source_read_limits_raw,
        &source_cut_limits_raw,
        revision.0.as_bytes(),
        membership.digest.as_bytes(),
    ]);
    let model_receipt_id = format!("sha256:{}", configuration_digest.to_hex());
    let child_stage = match crate::source_creation_store::IsolatedCreationRoot::create(
        &launcher.persistent_staging_root,
        deadline,
        cancelled,
    ) {
        Ok(stage) => stage,
        Err(primary) => {
            return Err(preserve_cleanup_failures(
                primary,
                cleanup_captured_source_export(&isolated, cleanup_manifest).err(),
                None,
            ));
        }
    };
    let mut compile_request = super::source_navigation_model_compile::SourceNavigationModelCompileRequestV2 {
        schema_version: "tos_source_navigation_model_compile_request_v2".into(),
        read_only_input_root: isolated.path().to_path_buf(),
        source_cut_root: isolated.path().join(".managed-source-cut"),
        bootstrap_source_revision: bootstrap_source_revision.0.to_hex(),
        bootstrap_membership_count: source_membership.count,
        bootstrap_membership_sha256: source_membership.digest.to_hex(),
        export_revision: revision.0.to_hex(),
        source_read_limits,
        source_cut_limits,
        membership_count: membership.count,
        membership_sha256: membership.digest.to_hex(),
        source_binding: source_binding.clone(),
        source_proof_v2: proof.clone(),
        schema_worker: super::source_navigation_model_compile::SourceNavigationCompileWorkerV1 {
            absolute_path: selected_worker_path,
            sha256: selected_worker_digest.to_hex(),
        },
        normalization_processor: super::source_navigation_model_compile::SourceNavigationCompileProcessorV1 {
            absolute_path: normalization_input_path,
            sha256: normalization_member.sha256.to_hex(),
            size_bytes: normalization_member.size_bytes,
        },
        artifact_staging_root: child_stage.path().to_path_buf(),
        configuration_sha256: configuration_digest.to_hex(),
        native_executable_sha256: String::new(),
        operation_budget_ms: 1,
        model_limits: super::source_navigation_model_compile::SourceNavigationCompileModelLimitsV1 {
            max_total_ms: owner.model_limits.max_total_ms,
            host_export_bytes: owner.model_limits.host_export_bytes,
            host_export_inodes: owner.model_limits.host_export_inodes,
            tmpfs_bytes: owner.model_limits.tmpfs_bytes,
            tmpfs_inodes: owner.model_limits.tmpfs_inodes,
            working_ram_bytes: owner.model_limits.working_ram_bytes,
            max_source_members: owner.model_limits.max_source_members,
            max_source_total_bytes: owner.model_limits.max_source_total_bytes,
            max_source_member_bytes: owner.model_limits.max_source_member_bytes,
            max_plan_bytes: owner.model_limits.max_plan_bytes,
            max_plan_work_bytes: owner.model_limits.max_plan_work_bytes,
            max_model_bytes: owner.model_limits.max_model_bytes,
        },
        producer_limits: super::source_navigation_model_compile::SourceNavigationCompileProducerLimitsV1 {
            max_events,
            schema_execution_wall_ms: schema_wall_ms,
            schema_cleanup_grace_ms: schema_cleanup_ms,
            schema_cpu_seconds,
            schema_address_space_bytes: schema_address_space,
            schema_max_receipts,
            schema_max_receipt_bytes,
            event_limits: super::source_navigation_model_compile::SourceNavigationCompileEventLimitsV1 {
                max_dependencies: event_dependencies,
                max_current_rows: event_rows,
                max_row_bytes: event_row_bytes,
                max_total_bytes: event_total_bytes,
            },
            aggregate_limits: super::source_navigation_model_compile::SourceNavigationCompileAggregateLimitsV1 {
                max_total_rows,
                max_total_encoded_bytes,
                max_total_dependency_bytes,
                max_total_work_units,
            },
        },
        cold_limits: selection.cold_limits,
        process_limits: selection.process_limits,
    };
    let remaining_ms = u64::try_from(
        deadline
            .saturating_duration_since(Instant::now())
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    let child_budget_ms = remaining_ms
        .saturating_sub(HOST_FINALIZATION_RESERVE_MS)
        .min(owner.model_limits.max_total_ms);
    let model_start = Instant::now();
    let child_result =
        super::source_navigation_model_process::launch_source_navigation_model_compile(
            &launcher,
            process_limits,
            child_budget_ms,
            &mut compile_request,
            cancelled,
        );
    let model_ms = model_start.elapsed().as_millis();
    drop(export);
    let export_cleanup = cleanup_captured_source_export(&isolated, cleanup_manifest);
    let cleanup_deadline = Instant::now() + Duration::from_millis(HOST_FINALIZATION_RESERVE_MS);
    let cleanup_cancelled = AtomicBool::new(false);
    let child_stage_cleanup = if child_result.is_err() || export_cleanup.is_err() {
        Some(cleanup_failed_child_stage(
            &child_stage,
            "source-navigation-model.sqlite",
            owner.model_limits.max_model_bytes,
            cleanup_deadline,
            &cleanup_cancelled,
        ))
    } else {
        None
    };
    let completed = match (child_result, export_cleanup, child_stage_cleanup) {
        (Ok(completed), Ok(()), _) => completed,
        (Err(primary), export_cleanup, stage_cleanup) => {
            return Err(preserve_cleanup_failures(
                primary,
                export_cleanup.err(),
                stage_cleanup.and_then(Result::err),
            ));
        }
        (Ok(_), Err(primary), stage_cleanup) => {
            return Err(preserve_cleanup_failures(
                primary,
                None,
                stage_cleanup.and_then(Result::err),
            ));
        }
    };
    let finalization = (|| -> SourceCommandResult<Value> {
        let child_stage_dir = child_stage.verify_current(deadline, cancelled)?;
        let candidate_leaf = "source-navigation-model.sqlite";
        let model_candidate_identity = stage_file_identity(&child_stage_dir, candidate_leaf)?;
        let expectation = completed.expectation.clone();
        if expectation.model_size_bytes == 0
            || expectation.model_size_bytes > owner.model_limits.max_model_bytes
            || selection.cold_limits.max_file_bytes < expectation.model_size_bytes
            || selection.cold_limits.max_file_bytes > owner.model_limits.max_model_bytes
            || completed.stage.sqlite_sha256 != expectation.model_sha256
            || completed.stage.sqlite_size_bytes != expectation.model_size_bytes
            || expectation.owner_receipt_id != model_receipt_id
        {
            return Err(SourceCommandError::Conflict(
                "selected cold model envelope does not fit compiled model",
            ));
        }
        let generation = coordinator
            .reopen_persisted_addressed_navigation_anchor(
                &store,
                &proof,
                selection.tree_limits.into_limits(),
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Conflict("compiled source-navigation anchor unavailable")
            })?;
        if generation.commit_seq() != proof.generation.through_commit_seq
            || generation.audit_generation() != proof.generation.selected_audit_generation
            || generation.digest().to_hex() != proof.generation.installed_generation_sha256
            || generation.epoch() != proof.generation.epoch
            || generation.agent_definition_digest().to_hex() != proof.generation.definition_sha256
        {
            return Err(SourceCommandError::Conflict(
                "compiled source-navigation anchor differs",
            ));
        }
        let model_destination = filesystem.protected_root_path().join(&owner.model_leaf);
        let policy_receipt = crate::source_navigation_policy::SourceNavigationPolicyReceiptV1 {
            schema_version: crate::source_navigation_policy::RECEIPT_SCHEMA.into(),
            issuer_ref: owner.policy_template.issuer_ref.clone(),
            receipt_id: owner.policy_template.receipt_id.clone(),
            export_receipt_id: crate::source_navigation_policy::selected_export_receipt_id(
                &completed.navigation_original.component_root_sha256,
            )
            .map_err(|_| SourceCommandError::Invalid("selected source export receipt id"))?,
            model_receipt_id: expectation.owner_receipt_id.clone(),
            carrier_layer: owner.policy_template.carrier_layer.clone(),
            intended_use: owner.policy_template.intended_use.clone(),
            model_abi: expectation.model_abi.clone(),
            selection_profile: owner.policy_template.selection_profile.clone(),
            policy_epoch: owner.policy_template.policy_epoch,
            withdrawal_generation: owner.policy_template.withdrawal_generation,
        };
        policy_receipt
            .validate()
            .map_err(|_| SourceCommandError::Denied("source-navigation policy receipt invalid"))?;
        let phase_publish_start = Instant::now();
        let source_contract = Digest256::from_hex(&selection.source_contract_sha256)
            .map_err(|_| SourceCommandError::Invalid("source-navigation contract digest"))?;
        let publication = coordinator.with_current_source_navigation_lease(
        &store,
        &generation,
        &proof,
        &filesystem,
        source_contract,
        selection.rule_version,
        selection.rights_version,
        &selection.job_id,
        selection.job_fence,
        deadline,
        cancelled,
        |lease, _tx| {
            let check_final_inputs = || -> std::result::Result<(), crate::DurableError> {
                filesystem.verify_selected_configuration(deadline, cancelled)
                    .map_err(|_| crate::DurableError::Conflict("preparation owner changed before publication"))?;
                if filesystem.protected_configuration_raw() != owner_raw.as_slice()
                    || super::read_absolute(&owner_path, uid, true, MAX_OWNER_BYTES, deadline, cancelled)
                .map_err(|_| crate::DurableError::Conflict("preparation owner changed before model publication"))? != owner_raw
                    || super::read_absolute(invocation_path, uid, true, MAX_OWNER_BYTES, deadline, cancelled)
                        .map_err(|_| crate::DurableError::Conflict("native invocation changed before publication"))? != invocation_raw
                    || super::read_absolute(&bootstrap_request_path, uid, true, 64 * 1024, deadline, cancelled)
                        .map_err(|_| crate::DurableError::Conflict("bootstrap request changed before publication"))? != bootstrap_request_raw
                {
                    return Err(crate::DurableError::Conflict("source-navigation preparation inputs changed"));
                }
                bootstrap.owner_check(&bootstrap_invocation_path, &bootstrap_invocation_raw, uid, deadline, cancelled)
                    .map_err(|_| crate::DurableError::Conflict("bootstrap owner changed before publication"))?;
                let expected_native_executable = Digest256::from_prefixed(
                    super::text(invocation, "native_executable_sha256")
                        .map_err(|_| crate::DurableError::Invalid("native executable digest"))?,
                )
                    .map_err(|_| crate::DurableError::Invalid("native executable digest"))?;
                let executable = crate::source_serialization::observe_executable(deadline, cancelled)
                    .and_then(|observation| observation.current_digest(deadline, cancelled))
                    .map_err(|_| crate::DurableError::Conflict("native executable changed before publication"))?;
                if executable != expected_native_executable {
                    return Err(crate::DurableError::Conflict("native executable changed before publication"));
                }
                let selected_schema = super::selected_schema(invocation, original_cut, deadline, cancelled)
                    .map_err(|_| crate::DurableError::Conflict("selected schema worker changed before publication"))?;
                drop(selected_schema);
                let normalization_now = original_software.read_selected_component(
                    original_components, &normalization_relative, 8 * 1024 * 1024, deadline, cancelled,
                ).map_err(|_| crate::DurableError::Conflict("normalization processor changed before publication"))?;
                if normalization_now != normalization_raw {
                    return Err(crate::DurableError::Conflict("normalization processor changed before publication"));
                }
                selection.process_limits.verify_current()
                    .map_err(|_| crate::DurableError::Conflict("selected process profile changed before publication"))?;
                require_compiled_generation_current(
                    lease,
                    &generation,
                    &proof,
                    source_contract,
                    selection.rule_version,
                    selection.rights_version,
                )?;
                Ok(())
            };
            check_final_inputs()?;
            let (verity, model_reused) = match std::fs::symlink_metadata(&model_destination) {
                Ok(metadata) if metadata.file_type().is_file() => {
                    let existing = tos_fd_open::open_regular_at(filesystem.protected_root_file(), Path::new(&owner.model_leaf))
                        .map_err(|_| crate::DurableError::Conflict("existing model destination unsafe"))?;
                    let pinned = existing.metadata().map_err(|_| crate::DurableError::Conflict("existing model metadata unavailable"))?;
                    if !pinned.is_file() || pinned.uid() != uid || pinned.mode() & 0o7777 != 0o600
                        || pinned.len() != expectation.model_size_bytes {
                        return Err(crate::DurableError::Conflict("existing model destination differs"));
                    }
                    let measurement = tos_compiler::measure_existing_native_knowledge_artifact_custody(
                        &model_destination, &expectation,
                    ).map_err(|_| crate::DurableError::Conflict("existing model custody differs"))?;
                    let custody = std::sync::Arc::new(
                        tos_compiler::LinuxFsVerityCustody::new(measurement.clone(), selection.process_limits)
                            .map_err(|_| crate::DurableError::Conflict("existing model fs-verity custody invalid"))?,
                    );
                    let selected = tos_compiler::open_selected_knowledge_model_owned(
                        &model_destination, expectation.clone(), custody, selection.cold_limits,
                    ).map_err(|_| crate::DurableError::Conflict("existing model exact content differs"))?;
                    selected.check_pin().map_err(|_| crate::DurableError::Conflict("existing model pin changed"))?;
                    (measurement, true)
                }
                Ok(_) => return Err(crate::DurableError::Conflict("existing model destination is not a regular file")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let measurement = super::source_navigation_model_artifact::publish_completed_native_model_artifact(
                        &compile_request.artifact_staging_root.join(candidate_leaf),
                        filesystem.protected_root_path(), &owner.model_leaf,
                        &completed.stage, owner.model_limits.max_model_bytes, deadline, cancelled,
                    ).map_err(|_| crate::DurableError::Conflict("create-only source-navigation model publication failed"))?;
                    let custody = std::sync::Arc::new(
                        tos_compiler::LinuxFsVerityCustody::new(measurement.clone(), selection.process_limits)
                            .map_err(|_| crate::DurableError::Conflict("published model fs-verity custody invalid"))?,
                    );
                    let selected = tos_compiler::open_selected_knowledge_model_owned(
                        &model_destination, expectation.clone(), custody, selection.cold_limits,
                    ).map_err(|_| crate::DurableError::Conflict("published model exact content differs"))?;
                    selected.check_pin().map_err(|_| crate::DurableError::Conflict("published model pin changed"))?;
                    (measurement, false)
                }
                Err(_) => return Err(crate::DurableError::Conflict("model destination state unavailable")),
            };
            if verity != completed.artifact_fs_verity {
                return Err(crate::DurableError::Conflict("published source-navigation artifact custody differs"));
            }
            remove_stage_file(&child_stage, candidate_leaf, model_candidate_identity, deadline, cancelled)
                .map_err(|_| crate::DurableError::Conflict("source-navigation child candidate cleanup failed"))?;
            child_stage.cleanup_empty(deadline, cancelled)
                .map_err(|_| crate::DurableError::Conflict("source-navigation child stage cleanup incomplete"))?;
            let mut source_navigation_value = owner.source_navigation.clone();
            source_navigation_value["model_path"] = Value::String(model_destination.to_string_lossy().into_owned());
            source_navigation_value["source_proof_v2"] = serde_json::to_value(&proof)
                .map_err(|_| crate::DurableError::Invalid("source-navigation proof encoding"))?;
            source_navigation_value["expectation"] = serde_json::to_value(&expectation)
                .map_err(|_| crate::DurableError::Invalid("source-navigation model expectation encoding"))?;
            source_navigation_value["policy_receipt"] = policy_receipt
                .to_value()
                .map_err(|_| crate::DurableError::Invalid("source-navigation policy encoding"))?;
            let final_owner_value = json!({
                "schema_version": FINAL_OWNER_SCHEMA,
                "uid": owner.uid,
                "source_root": owner.source_root,
                "expires_at": owner.expires_at,
                "source_navigation": source_navigation_value,
            });
            if final_owner_value.get("schema_version").and_then(Value::as_str) != Some(FINAL_OWNER_SCHEMA) {
                return Err(crate::DurableError::Invalid("final source-navigation owner schema"));
            }
            let final_owner_source = cmd::parse(&serde_json::to_vec(&final_owner_value)
                .map_err(|_| crate::DurableError::Invalid("final source-navigation owner encoding"))?)
                .map_err(|_| crate::DurableError::Invalid("final source-navigation owner JSON"))?;
            let final_owner_bytes = cmd::canonical(&final_owner_source)
                .map_err(|_| crate::DurableError::Invalid("final source-navigation owner canonicalization"))?;
            let final_owner_path = filesystem.protected_root_path().join(&owner.final_owner_leaf);
            // Recheck every independent source/owner input after model
            // publication and immediately before the final owner receipt.
            check_final_inputs()?;
            let (final_owner_metadata, owner_reused) = match tos_fd_open::open_regular_at(
                filesystem.protected_root_file(), Path::new(&owner.final_owner_leaf),
            ) {
                Ok(mut existing_file) => {
                    let existing_metadata = existing_file.metadata()
                        .map_err(|_| crate::DurableError::Conflict("existing final owner metadata unavailable"))?;
                    if !existing_metadata.is_file() || existing_metadata.uid() != uid
                        || existing_metadata.mode() & 0o7777 != 0o600
                        || existing_metadata.len() > MAX_OWNER_BYTES as u64
                    {
                        return Err(crate::DurableError::Conflict("existing final source-navigation owner is unsafe"));
                    }
                    let mut existing = Vec::new();
                    existing_file.take((MAX_OWNER_BYTES + 1) as u64)
                        .read_to_end(&mut existing)
                        .map_err(|_| crate::DurableError::Conflict("existing final owner readback failed"))?;
                    if existing != final_owner_bytes {
                        return Err(crate::DurableError::Conflict("existing final source-navigation owner differs"));
                    }
                    (super::source_navigation_model_artifact::PublishedCanonicalJson {
                        sha256: Digest256::of_bytes(&existing),
                        size_bytes: existing.len() as u64,
                    }, true)
                }
                Err(_) => {
                    if std::fs::symlink_metadata(&final_owner_path).is_ok() {
                        return Err(crate::DurableError::Conflict("final source-navigation owner destination unsafe"));
                    }
                    (super::source_navigation_model_artifact::publish_canonical_owner_json(
                        filesystem.protected_root_file(), &owner.final_owner_leaf, &final_owner_bytes,
                        MAX_OWNER_BYTES, deadline, cancelled,
                    ).map_err(|_| crate::DurableError::Conflict("create-only final source-navigation owner publication failed"))?, false)
                }
            };
            check_final_inputs()?;
            Ok(json!({
                "schema_version": RESULT_SCHEMA,
                "operation": "prepare-initial",
                "admission_granted": false,
                "status": if model_reused && owner_reused { "retained_pair_recovered" } else { "prepared_and_published" },
                "model_reused": model_reused,
                "final_owner_reused": owner_reused,
                "source": {"revision": revision.0.to_hex(), "membership_sha256": membership.digest.to_hex(), "members": membership.count, "bytes": current_source_bytes},
                "catalog_plan": {
                    "selected_members": completed.catalog_work.planned_selected_members,
                    "observed_work_bytes": completed.catalog_work.plan_observed_work_bytes,
                    "plan_bytes_cap": owner.model_limits.max_plan_bytes,
                    "plan_work_bytes_cap": owner.model_limits.max_plan_work_bytes,
                },
                "catalog_render": {
                    "source_members_returned": completed.catalog_work.rendered_source_members,
                    "source_payload_bytes_returned": completed.catalog_work.rendered_source_payload_bytes,
                    "input_rows_staged": completed.catalog_work.staged_input_rows,
                    "input_payload_bytes_staged": completed.catalog_work.staged_input_payload_bytes,
                },
                "model": {
                    "path": model_destination.to_string_lossy(),
                    "sha256": expectation.model_sha256,
                    "size_bytes": expectation.model_size_bytes,
                    "fs_verity": verity,
                    "expectation": expectation,
                    "navigation_original": completed.navigation_original,
                    "stage": {"input_rows": completed.stage.input_rows, "node_rows": completed.stage.node_rows, "relation_rows": completed.stage.relation_rows}
                },
                "proof": proof,
                "final_owner_configuration": {"path": filesystem.protected_root_path().join(&owner.final_owner_leaf).to_string_lossy(), "sha256": final_owner_metadata.sha256.to_hex(), "size_bytes": final_owner_metadata.size_bytes},
                "phase_ms": {
                    "bootstrap": bootstrap_start.elapsed().as_millis(),
                    "selected_cut_export": export_start.elapsed().as_millis(),
                    "catalog_and_plan": completed.catalog_work.planning_elapsed_ms,
                    "catalog_render": completed.catalog_work.rendering_elapsed_ms,
                    "model": model_ms,
                    "publish": phase_publish_start.elapsed().as_millis(),
                }
            }))
        },
    ).map_err(|error| match error {
        crate::DurableError::Source(error) => error,
        crate::DurableError::Conflict(reason) => SourceCommandError::Conflict(reason),
        crate::DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        crate::DurableError::Invalid(reason) | crate::DurableError::Corrupt(reason) => {
            SourceCommandError::Invalid(reason)
        }
        _ => SourceCommandError::Conflict("source-navigation current-owner publication lease failed"),
    })?;
        let _ = (invocation, outer_owner_context);
        Ok(publication)
    })();
    match finalization {
        Ok(publication) => Ok(publication),
        Err(primary) => {
            let stage_cleanup = cleanup_failed_child_stage(
                &child_stage,
                "source-navigation-model.sqlite",
                owner.model_limits.max_model_bytes,
                deadline,
                cancelled,
            );
            Err(preserve_cleanup_failures(
                primary,
                None,
                stage_cleanup.err(),
            ))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparationSelection {
    schema_version: String,
    domain: String,
    source_contract_sha256: String,
    rule_version: u64,
    rights_version: u64,
    job_id: String,
    job_fence: u64,
    consumer_id: String,
    profile_id: String,
    profile_version: u32,
    store_root: PathBuf,
    store_limits: StoreLimits,
    tree_limits: TreeLimits,
    process_limits: tos_compiler::NativeProcessLimits,
    cold_limits: tos_compiler::ColdOpenLimits,
    producer: Value,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreLimits {
    max_segment_bytes: u64,
    max_frame_bytes: u64,
    max_frames: u32,
    max_journal_bytes: usize,
}
impl StoreLimits {
    fn into_limits(self) -> tos_segment_store::SegmentLimits {
        tos_segment_store::SegmentLimits {
            max_segment_bytes: self.max_segment_bytes,
            max_frame_bytes: self.max_frame_bytes,
            max_frames: self.max_frames,
            max_journal_bytes: self.max_journal_bytes,
        }
    }
}
#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeLimits {
    max_key_bytes: usize,
    max_value_bytes: usize,
    max_kind_bytes: usize,
    max_node_bytes: usize,
    max_children: usize,
    max_nodes: u64,
    max_total_bytes: u64,
    max_rows: u64,
}
impl TreeLimits {
    fn into_limits(self) -> tos_segment_store::AuthenticatedTreeLimitsV1 {
        tos_segment_store::AuthenticatedTreeLimitsV1 {
            max_key_bytes: self.max_key_bytes,
            max_value_bytes: self.max_value_bytes,
            max_kind_bytes: self.max_kind_bytes,
            max_node_bytes: self.max_node_bytes,
            max_children: self.max_children,
            max_nodes: self.max_nodes,
            max_total_bytes: self.max_total_bytes,
            max_rows: self.max_rows,
        }
    }
}
impl PreparationSelection {
    fn parse(value: &Value) -> SourceCommandResult<Self> {
        validate_selection_input(value)?;
        serde_json::from_value(value.clone())
            .map_err(|_| SourceCommandError::Invalid("source-navigation model selection"))
    }
}

#[cfg(test)]
mod initial_envelope_tests {
    use super::*;

    #[test]
    fn bootstrap_contract_initial_invocation_uses_explicit_owner_bounds() {
        let mut owner = json!({
            "schema_version": OWNER_SCHEMA, "uid": 1000,
            "source_root": "/selected", "expires_at": "2026-10-15T12:00:00Z",
            "host_export_stage_parent": "/selected/export",
            "bootstrap_invocation_path": "/selected/bootstrap.json",
            "bootstrap_request_path": "/selected/request.json",
            "model_leaf": "model.sqlite", "final_owner_leaf": "owner.json",
            "source_navigation": {}, "launcher": {},
            "policy_template": {
                "schema_version": "policy", "issuer_ref": "issuer",
                "receipt_id": "receipt", "carrier_layer": "layer",
                "intended_use": "use", "model_abi": "abi", "selection_profile": "profile",
                "policy_epoch": 1, "withdrawal_generation": 0, "expires_at": "expiry"
            },
            "model_limits": {
                "max_total_ms": 600000, "host_export_bytes": 2147483648u64,
                "host_export_inodes": 100000, "tmpfs_bytes": 1073741824,
                "tmpfs_inodes": 100000, "working_ram_bytes": 1073741824,
                "max_source_members": 10000, "max_source_total_bytes": 300000000,
                "max_source_member_bytes": 33554432, "max_plan_bytes": 1048576,
                "max_plan_work_bytes": 33554432, "max_model_bytes": 536870912
            }
        });
        let (read, cut, ms) =
            initial_invocation_limits(&serde_json::to_vec(&owner).unwrap(), 1000).unwrap();
        assert!(read.max_manifest_bytes > 2_600_000);
        assert!(read.json.max_bytes >= read.max_manifest_bytes);
        assert!(read.max_manifest_entries >= 9517);
        assert!(read.max_selected_object_bytes >= 16_734_381);
        assert!(cut.max_members >= 19_034 && cut.max_total_bytes >= 549_726_458);
        assert_eq!(ms, 600000);
        owner["model_limits"]["max_total_ms"] = json!(INITIAL_MAX_TOTAL_MS);
        assert_eq!(
            initial_invocation_limits(&serde_json::to_vec(&owner).unwrap(), 1000)
                .unwrap()
                .2,
            INITIAL_MAX_TOTAL_MS
        );
        owner["model_limits"]["max_total_ms"] = json!(INITIAL_MAX_TOTAL_MS + 1);
        assert!(initial_invocation_limits(&serde_json::to_vec(&owner).unwrap(), 1000).is_err());
        owner["model_limits"]["max_total_ms"] = json!(600000);
        assert!(initial_invocation_limits(&serde_json::to_vec(&owner).unwrap(), 1001).is_err());
        owner["model_limits"]["max_source_total_bytes"] = json!(2147483649u64);
        assert!(initial_invocation_limits(&serde_json::to_vec(&owner).unwrap(), 1000).is_err());
        owner["model_limits"]["max_source_total_bytes"] = json!(300000000);
        owner["schema_version"] = json!("tos_local_source_owner_v1");
        assert!(initial_invocation_limits(&serde_json::to_vec(&owner).unwrap(), 1000).is_err());
    }
}
