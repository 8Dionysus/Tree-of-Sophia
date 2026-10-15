//! Protected native entry for the bounded streamed PostgreSQL/STO bootstrap.
//!
//! This is a separate invocation family from the legacy materializing import.
//! It anchors one exact source revision, retains only the authenticated streamed
//! cut index plus one command-sized batch, and keeps the protected owner and
//! executable observation fences active through every durable batch edge.

use super::pg_bootstrap::{
    SoftwareInputLimits, prepare_protected_bootstrap, selected_source, selected_store, source_error,
};
use super::{absolute, capped, digest, exact, number, text};
use crate::source_cohort::BootstrapJobMode;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::{ColdWorkspaceLimits, DurablePgCoordinator, PrivateGenerationWorkspace};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::JsonLimits;
use tos_segment_store::{AuthenticatedTreeLimitsV1, GenerationReadLimits, GenerationShapeLimits};
use tos_source_store::{
    CorpusReader, CutReadLimits, ReadLimits, SourceMembershipV1, StreamedCutReadLimitsV1,
};
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget, VerifiedWorkerImageHandle};
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};

const INVOCATION_SCHEMA: &str = "tos_local_pg_source_bootstrap_streamed_invocation_v1";
const RESULT_SCHEMA: &str = "tos_local_pg_source_bootstrap_streamed_result_v1";
const MAX_SOURCE_MEMBERS: u64 = 1_000_000;
const MAX_SOURCE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_SOURCE_MEMBER_BYTES: u64 = 32 * 1024 * 1024;
const MAX_DEPENDENCY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GENERATION_ELAPSED_SECONDS: u64 = 86_400;

fn object<'a>(value: &'a Value, key: &str) -> SourceCommandResult<&'a Value> {
    value
        .get(key)
        .filter(|value| value.is_object())
        .ok_or(SourceCommandError::Invalid("streamed bootstrap object"))
}

fn field(value: &Value, key: &str, maximum: u64) -> SourceCommandResult<u64> {
    let selected = number(value, key)?;
    if selected > maximum {
        return Err(SourceCommandError::Invalid("streamed bootstrap budget"));
    }
    Ok(selected)
}

/// Select the streamed route's cooperative whole-invocation deadline before
/// any source, STO or PostgreSQL side effect. The full budget parser below
/// repeats the same bounded field validation for the operation itself.
pub(super) fn invocation_duration(value: &Value) -> SourceCommandResult<Duration> {
    let budgets = object(value, "streamed_budgets")?;
    let generation = object(budgets, "generation")?;
    let seconds = field(
        generation,
        "max_elapsed_seconds",
        MAX_GENERATION_ELAPSED_SECONDS,
    )?;
    Ok(Duration::from_secs(seconds))
}

fn usize_field(value: &Value, key: &str, maximum: u64) -> SourceCommandResult<usize> {
    usize::try_from(field(value, key, maximum)?)
        .map_err(|_| SourceCommandError::Invalid("streamed bootstrap budget"))
}

struct StreamedBudgets {
    cut: CutReadLimits,
    max_source_bytes: u64,
    max_member_bytes: u64,
    max_dependency_bytes_per_member: usize,
    cut_read: StreamedCutReadLimitsV1,
    source_limits: crate::source_cohort::StreamedColdSourceLimitsV1,
    workspace: ColdWorkspaceLimits,
    generation: crate::StreamedGenerationProfile,
    item: tos_validation::item_rules::ItemLimits,
    tree: AuthenticatedTreeLimitsV1,
}

fn streamed_budgets(value: &Value, deadline: Instant) -> SourceCommandResult<StreamedBudgets> {
    exact(
        value,
        &[
            "max_revisions",
            "max_members",
            "max_source_bytes",
            "max_member_bytes",
            "max_dependency_bytes_per_member",
            "max_manifest_bytes",
            "max_manifest_entries",
            "max_manifest_row_bytes",
            "max_index_bytes",
            "max_sqlite_cache_bytes",
            "index",
            "workspace",
            "generation",
            "tree",
            "item",
        ],
    )?;
    let max_revisions = usize_field(value, "max_revisions", 64)?;
    let max_members = field(value, "max_members", MAX_SOURCE_MEMBERS)?;
    let max_source_bytes = field(value, "max_source_bytes", MAX_SOURCE_BYTES)?;
    let max_member_bytes = field(value, "max_member_bytes", MAX_SOURCE_MEMBER_BYTES)?;
    let max_dependency_bytes_per_member = usize_field(
        value,
        "max_dependency_bytes_per_member",
        MAX_DEPENDENCY_BYTES,
    )?;
    let max_manifest_bytes = field(value, "max_manifest_bytes", MAX_MANIFEST_BYTES)?;
    let max_manifest_entries = usize_field(value, "max_manifest_entries", 1_000_000)?;
    let max_manifest_row_bytes = usize_field(value, "max_manifest_row_bytes", 64 * 1024 * 1024)?;
    let max_index_bytes = field(value, "max_index_bytes", 4 * 1024 * 1024 * 1024)?;
    let max_sqlite_cache_bytes = usize_field(value, "max_sqlite_cache_bytes", 512 * 1024 * 1024)?;

    let index = object(value, "index")?;
    exact(
        index,
        &[
            "max_rows",
            "max_logical_bytes",
            "max_key_bytes",
            "max_value_bytes",
            "max_placement_bytes",
            "max_sqlite_file_bytes",
            "max_vm_steps",
        ],
    )?;
    let source_limits = crate::source_cohort::StreamedColdSourceLimitsV1 {
        max_member_bytes,
        max_rows: field(index, "max_rows", MAX_SOURCE_MEMBERS)?,
        max_logical_bytes: field(index, "max_logical_bytes", MAX_SOURCE_BYTES)?,
        max_key_bytes: usize_field(index, "max_key_bytes", 1_048_576)?,
        max_value_bytes: usize_field(index, "max_value_bytes", 64 * 1024 * 1024)?,
        max_placement_bytes: usize_field(index, "max_placement_bytes", 64 * 1024 * 1024)?,
        max_sqlite_file_bytes: field(index, "max_sqlite_file_bytes", MAX_SOURCE_BYTES)?,
        max_vm_steps: field(index, "max_vm_steps", 2_000_000_000)?,
    };

    let workspace_value = object(value, "workspace")?;
    exact(
        workspace_value,
        &[
            "max_scratch_written_bytes",
            "max_run_bytes",
            "max_runs",
            "merge_fan_in",
            "max_rows",
            "max_key_bytes",
        ],
    )?;
    let workspace = ColdWorkspaceLimits {
        max_scratch_written_bytes: field(
            workspace_value,
            "max_scratch_written_bytes",
            MAX_SOURCE_BYTES,
        )?,
        max_run_bytes: usize_field(workspace_value, "max_run_bytes", 256 * 1024 * 1024)?,
        max_runs: usize_field(workspace_value, "max_runs", 65_536)?,
        merge_fan_in: usize_field(workspace_value, "merge_fan_in", 64)?,
        max_rows: field(workspace_value, "max_rows", MAX_SOURCE_MEMBERS)?,
        max_key_bytes: usize_field(workspace_value, "max_key_bytes", 1_048_576)?,
    };

    let generation_value = object(value, "generation")?;
    exact(
        generation_value,
        &[
            "max_commit_seq",
            "max_members",
            "max_pins",
            "max_segment_bytes",
            "max_membership_key_bytes",
            "max_metadata_rows",
            "max_metadata_bytes",
            "max_elapsed_seconds",
            "max_pg_temp_bytes",
            "max_sql_statement_ms",
            "generation",
            "rows_per_leaf",
        ],
    )?;
    let read_generation = object(generation_value, "generation")?;
    exact(
        read_generation,
        &[
            "max_descriptor_bytes",
            "max_partitions",
            "max_rows_per_partition",
            "max_key_bytes",
            "max_leaf_bytes",
            "max_stream_rows",
            "max_stream_key_bytes",
        ],
    )?;
    let shape = GenerationShapeLimits {
        max_partitions: usize_field(read_generation, "max_partitions", 1_000_000)?,
        max_rows_per_partition: field(
            read_generation,
            "max_rows_per_partition",
            MAX_SOURCE_MEMBERS,
        )?,
        max_key_bytes: usize_field(read_generation, "max_key_bytes", 1_048_576)?,
        max_leaf_bytes: field(read_generation, "max_leaf_bytes", MAX_SOURCE_BYTES)?,
    };
    let generation = crate::StreamedGenerationProfile {
        max_commit_seq: field(generation_value, "max_commit_seq", i64::MAX as u64)?,
        max_members: field(generation_value, "max_members", MAX_SOURCE_MEMBERS)?,
        max_pins: usize_field(generation_value, "max_pins", 1_000_000)?,
        max_segment_bytes: field(generation_value, "max_segment_bytes", MAX_SOURCE_BYTES)?,
        max_membership_key_bytes: field(
            generation_value,
            "max_membership_key_bytes",
            MAX_SOURCE_BYTES,
        )?,
        max_metadata_rows: usize_field(generation_value, "max_metadata_rows", 1_000_000)?,
        max_metadata_bytes: usize_field(generation_value, "max_metadata_bytes", MAX_SOURCE_BYTES)?,
        max_elapsed: Duration::from_secs(field(
            generation_value,
            "max_elapsed_seconds",
            MAX_GENERATION_ELAPSED_SECONDS,
        )?),
        max_pg_temp_bytes: field(generation_value, "max_pg_temp_bytes", MAX_SOURCE_BYTES)?,
        max_sql_statement_ms: field(generation_value, "max_sql_statement_ms", 300_000)?,
        generation: GenerationReadLimits {
            max_descriptor_bytes: usize_field(
                read_generation,
                "max_descriptor_bytes",
                64 * 1024 * 1024,
            )?,
            shape,
            max_stream_rows: field(read_generation, "max_stream_rows", MAX_SOURCE_MEMBERS)?,
            max_stream_key_bytes: field(read_generation, "max_stream_key_bytes", MAX_SOURCE_BYTES)?,
        },
        rows_per_leaf: usize_field(generation_value, "rows_per_leaf", 1_000_000)?,
    };

    let tree_value = object(value, "tree")?;
    exact(
        tree_value,
        &[
            "max_key_bytes",
            "max_value_bytes",
            "max_kind_bytes",
            "max_node_bytes",
            "max_children",
            "max_nodes",
            "max_total_bytes",
            "max_rows",
        ],
    )?;
    let tree = AuthenticatedTreeLimitsV1 {
        max_key_bytes: usize_field(tree_value, "max_key_bytes", 1_048_576)?,
        max_value_bytes: usize_field(tree_value, "max_value_bytes", 64 * 1024 * 1024)?,
        max_kind_bytes: usize_field(tree_value, "max_kind_bytes", 65_536)?,
        max_node_bytes: usize_field(tree_value, "max_node_bytes", 64 * 1024 * 1024)?,
        max_children: usize_field(tree_value, "max_children", 256)?,
        max_nodes: field(tree_value, "max_nodes", MAX_SOURCE_MEMBERS)?,
        max_total_bytes: field(tree_value, "max_total_bytes", MAX_SOURCE_BYTES)?,
        max_rows: field(tree_value, "max_rows", MAX_SOURCE_MEMBERS)?,
    }
    .validate()
    .map_err(|_| SourceCommandError::Invalid("streamed bootstrap tree limits"))?;

    let item_value = object(value, "item")?;
    exact(
        item_value,
        &[
            "max_member_bytes",
            "max_total_bytes",
            "max_state_bytes",
            "max_issues",
        ],
    )?;
    let item = tos_validation::item_rules::ItemLimits {
        max_member_bytes: usize_field(item_value, "max_member_bytes", 64 * 1024 * 1024)?,
        max_total_bytes: field(item_value, "max_total_bytes", MAX_SOURCE_BYTES)?,
        max_state_bytes: usize_field(item_value, "max_state_bytes", MAX_SOURCE_BYTES)?,
        max_issues: usize_field(item_value, "max_issues", 1_000_000)?,
        deadline,
    };
    let cut_read = StreamedCutReadLimitsV1 {
        cut: CutReadLimits {
            max_revisions,
            max_members,
            max_total_bytes: max_source_bytes,
            max_member_bytes,
        },
        manifest_json: JsonLimits {
            max_bytes: usize::try_from(max_manifest_bytes)
                .map_err(|_| SourceCommandError::Invalid("streamed manifest budget"))?,
            ..JsonLimits::default()
        },
        max_manifest_entries,
        max_index_bytes,
        max_manifest_row_bytes,
        sqlite_cache_bytes: max_sqlite_cache_bytes,
    };
    Ok(StreamedBudgets {
        cut: CutReadLimits {
            max_revisions,
            max_members,
            max_total_bytes: max_source_bytes,
            max_member_bytes,
        },
        max_source_bytes,
        max_member_bytes,
        max_dependency_bytes_per_member,
        cut_read,
        source_limits,
        workspace,
        generation,
        item,
        tree,
    })
}

fn software_limits(invocation: &Value) -> SourceCommandResult<SoftwareInputLimits> {
    let budgets = object(invocation, "budgets")?;
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
    Ok(SoftwareInputLimits {
        max_members: capped(budgets, "max_members", 4_096)?,
        max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
        max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
    })
}

fn worker_limits_and_image(
    invocation: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(VerifiedWorkerImageHandle, ExecutorBudget, CutWorkerLimits)> {
    let worker = object(invocation, "schema_worker")?;
    exact(worker, &["absolute_path", "sha256"])?;
    let budgets = object(invocation, "budgets")?;
    let mut worker_budget = ExecutorBudget::laboratory();
    worker_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    worker_budget.cpu_seconds = capped(budgets, "worker_cpu_seconds", 3)?;
    worker_budget.address_space_bytes =
        capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    let image = VerifiedWorkerImageHandle::prepare(
        ExactWorkerIdentity {
            absolute_path: absolute(text(worker, "absolute_path")?)?,
            sha256: digest(text(worker, "sha256")?)?,
        },
        worker_budget,
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Denied("streamed schema worker image"))?;
    let limits = CutWorkerLimits {
        max_receipts: usize::try_from(capped(budgets, "max_schema_receipts", 128)?)
            .map_err(|_| SourceCommandError::Invalid("streamed schema receipt budget"))?,
        max_receipt_bytes: usize::try_from(capped(budgets, "max_schema_receipt_bytes", 262_144)?)
            .map_err(|_| {
            SourceCommandError::Invalid("streamed schema receipt budget")
        })?,
    };
    Ok((image, worker_budget, limits))
}

pub(super) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
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
            "streamed_budgets",
            "workspace_root",
        ],
    )?;
    if text(invocation, "schema_version")? != INVOCATION_SCHEMA {
        return Err(SourceCommandError::Unsupported(
            "streamed PostgreSQL source bootstrap invocation",
        ));
    }
    let selection = selected_source(invocation)?;
    if selection.job_id != "source-bootstrap" || selection.job_fence != 1 {
        return Err(SourceCommandError::Denied(
            "streamed bootstrap requires the exact source-bootstrap lease",
        ));
    }
    if digest(text(invocation, "source_revision")?)? != selection.source_revision.0 {
        return Err(SourceCommandError::Conflict(
            "streamed source revision selection differs",
        ));
    }
    let budgets = streamed_budgets(object(invocation, "streamed_budgets")?, deadline)?;
    let protected = prepare_protected_bootstrap(
        invocation,
        input,
        uid,
        software_limits(invocation)?,
        deadline,
        cancelled.as_ref(),
    )?;
    let selection = &protected.selection;
    let workspace_root = absolute(text(invocation, "workspace_root")?)?;
    if workspace_root == selection.store_root {
        return Err(SourceCommandError::Denied(
            "streamed workspace must be separate from source store",
        ));
    }
    let corpus = CorpusReader::open_existing(
        &absolute(text(invocation, "corpus_store")?)?,
        ReadLimits {
            max_manifest_bytes: usize::try_from(budgets.cut_read.manifest_json.max_bytes)
                .map_err(|_| SourceCommandError::Invalid("streamed manifest budget"))?,
            max_manifest_entries: budgets.cut_read.max_manifest_entries,
            max_selected_object_bytes: budgets.max_member_bytes,
            json: JsonLimits::default(),
        },
    )
    .map_err(|_| SourceCommandError::Conflict("selected immutable streamed source store"))?;
    let workspace = PrivateGenerationWorkspace::open(&workspace_root, budgets.workspace)
        .map_err(source_error)?;
    let mut body_work = crate::source_cohort::StreamedColdSourceWorkV1::default();
    let streamed = workspace
        .open_streamed_source_cut(
            &corpus,
            selection.source_revision,
            budgets.cut_read,
            deadline,
            cancelled.as_ref(),
            &mut body_work,
        )
        .map_err(source_error)?;
    let selected = streamed
        .revision_at(0)
        .map_err(|_| SourceCommandError::Conflict("streamed selected revision"))?
        .ok_or(SourceCommandError::Conflict(
            "streamed selected revision absent",
        ))?;
    let membership: SourceMembershipV1 = selected.membership;
    if selected.revision != selection.source_revision || membership != selection.membership {
        return Err(SourceCommandError::Conflict(
            "streamed exact revision membership differs",
        ));
    }
    let (image, worker_budget, worker_limits) =
        worker_limits_and_image(invocation, deadline, cancelled.as_ref())?;
    let mut worker = CutWorkerSchemaExecutor::from_streamed_cut_with_image(
        &streamed,
        FormatProfile::LegacyPythonObserved20260923,
        &image,
        worker_budget,
        worker_limits,
        deadline,
        cancelled.as_ref(),
    )
    .map_err(|_| SourceCommandError::Denied("streamed schema worker"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let owner_check = || {
        protected.owner_check(
            invocation_path,
            invocation_raw,
            uid,
            deadline,
            cancelled.as_ref(),
        )
    };
    owner_check().map_err(source_error)?;
    if selection.initialize_schema {
        db.initialize_empty_schema()
            .map_err(|_| SourceCommandError::Conflict("initialize absent CMD.2 schema"))?;
        owner_check().map_err(source_error)?;
    }
    db.require_compatible_schema()
        .map_err(|_| SourceCommandError::Conflict("selected CMD.2 schema is not provisioned"))?;
    owner_check().map_err(source_error)?;
    if selection.initialize_schema {
        db.enable_addressed_audit_protocol().map_err(|_| {
            SourceCommandError::Conflict("initialize addressed audit companion schema")
        })?;
    } else {
        db.require_addressed_audit_protocol(deadline, cancelled.as_ref())
            .map_err(|_| {
                SourceCommandError::Conflict(
                    "selected addressed audit companion schema is not provisioned",
                )
            })?;
    }
    owner_check().map_err(source_error)?;
    let store = match selection.job_mode {
        BootstrapJobMode::Initialize => selected_store(&protected.owner_fs, selection)?,
        BootstrapJobMode::RequireExisting => tos_segment_store::SegmentStore::open_existing_at(
            protected.owner_fs.protected_root_file(),
            selection.store_limits,
        )
        .map_err(|_| SourceCommandError::Conflict("open retained streamed bootstrap store"))?,
    };
    owner_check().map_err(source_error)?;
    if store.custody_domain() != selection.domain.as_bytes() {
        return Err(SourceCommandError::Conflict(
            "streamed bootstrap store identity changed",
        ));
    }
    let mut tree_work = tos_segment_store::AuthenticatedTreeWorkV1::default();
    let owner_check = &owner_check;
    let reopened = db
        .bootstrap_source_cohort_streamed(
            &store,
            &selection.domain,
            &streamed,
            selection.source_revision,
            selection.membership,
            &protected.context,
            &protected.software,
            &protected.components,
            &mut worker,
            selection.contract,
            selection.job_mode,
            budgets.cut.max_members,
            budgets.max_source_bytes,
            budgets.max_member_bytes,
            budgets.max_dependency_bytes_per_member,
            "ToS/contracts/corpus-record.schema.json",
            &workspace,
            budgets.generation,
            budgets.source_limits,
            budgets.item,
            budgets.tree,
            &mut body_work,
            &mut tree_work,
            deadline,
            Arc::clone(cancelled),
            owner_check,
        )
        .map_err(source_error)?;
    let (current_membership_digest, selected_state_digest) = match reopened.selected() {
        crate::durable_adapter::SelectedSourceGeneration::Addressed(selected) => {
            (selected.cut.current.commitment, selected.state_digest)
        }
        _ => {
            return Err(SourceCommandError::Conflict(
                "streamed bootstrap returned a non-addressed generation",
            ));
        }
    };
    Ok(json!({
        "schema_version": RESULT_SCHEMA,
        "domain": selection.domain,
        "source_revision_sha256": selection.source_revision.0.to_prefixed(),
        "source_membership_sha256": selection.membership.digest.to_prefixed(),
        "source_membership_count": selection.membership.count,
        "current_membership_sha256": current_membership_digest.to_prefixed(),
        "current_membership_count": reopened.member_count(),
        "source_complete": true,
        "schema_initialized": selection.initialize_schema,
        "selected_generation_sha256": reopened.digest().to_prefixed(),
        "selected_head_seq": reopened.commit_seq(),
        "selected_state_sha256": selected_state_digest.to_prefixed(),
        "streamed": true,
        "bootstrap_resume_supported": true,
        "bootstrap_resume_scope": "complete-original-write-before-assessment",
        "bootstrap_resumed": selection.job_mode == BootstrapJobMode::RequireExisting,
        "logical_source_work": {
            "original_bodies_returned": body_work.original_bodies_returned,
            "original_body_bytes_returned": body_work.original_body_bytes_returned,
            "current_bodies_returned": body_work.current_bodies_returned,
            "current_body_bytes_returned": body_work.current_body_bytes_returned,
            "current_projection_query_rows_returned": body_work.current_projection_query_rows_returned,
            "current_and_history_projection_bytes_returned": body_work.current_and_history_projection_bytes_returned,
            "software_bodies_returned": body_work.software_bodies_returned,
            "software_body_bytes_returned": body_work.software_body_bytes_returned,
            "fresh_pg_field_bytes_attempted": body_work.fresh_pg_field_bytes_attempted,
            "fresh_pg_rows_written": body_work.fresh_pg_rows_written,
            "tree": {
                "read_bytes": tree_work.read_bytes,
                "written_bytes": tree_work.written_bytes,
                "allocated_bytes": tree_work.allocated_bytes,
            },
            "derived_index": {
                "sqlite_file_len_high_water": body_work.derived_index.sqlite_file_len_high_water,
                "sqlite_allocated_bytes_high_water": body_work.derived_index.sqlite_allocated_bytes_high_water,
            },
        },
        "grants_admission": false,
        "grants_publication": false
    }))
}
