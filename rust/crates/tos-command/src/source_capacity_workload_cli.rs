//! Maintained two-root 100K capacity-fixture producer entry.
//!
//! The producer emits a raw census root and a packed indexed root from one
//! typed cursor under the selected Native invocation. Its receipt is
//! measurement only; `corpus-admit` still reopens and validates both roots.

use crate::source_admission::{AdmissionWorkBudget, invalid};
use crate::source_admission_packed_objects::{MAX_PACKED_OBJECT_FRAMES_V2, PackedObjectLimitsV2};
use crate::source_capacity_workload::{
    PackedScaleInputReceiptV1, WeightedScaleProducerRequestV1, WeightedScaleProfileV1,
    produce_weighted_scale_input_v1,
};
use crate::source_current_cut::{
    foundation_command::SelectedOutput, foundation_entry::FoundationBootstrapClock,
};
use crate::source_foundation_admission::{NativeSourceValidator, PreparedAdmissionExecution};
use std::cell::Cell;
use std::ffi::OsString;
use std::io::{self, Write};
use std::mem::{size_of, size_of_val};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32},
};
use std::time::Instant;
use tos_foundation::Digest256;

pub const HELP: &str = "usage: tos-native-owner-command capacity-fixture --store PATH --seed-sha256 LOWERHEX64 --work-units N -- --repo-root ABS --invocation ABS [native validator selections]\n\nCreate the deterministic private 100K raw and packed scale-input roots under the protected artifact root. Then run corpus-admit with the printed --input-root and --indexed-input-root. This fixture does not grant source, review, rights, canon, or admission authority.\n";

struct Arguments {
    store: PathBuf,
    seed: Digest256,
    work_units: u64,
    repository_root: PathBuf,
    validator: Vec<OsString>,
}

fn parse_path(value: &OsString, label: &'static str) -> io::Result<PathBuf> {
    let text = value
        .to_str()
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or_else(|| invalid(label))?;
    let path = Path::new(text);
    if !path.is_absolute()
        || path == Path::new("/")
        || path.to_str() != Some(text)
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        || text[1..]
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid(label));
    }
    Ok(path.to_path_buf())
}

fn parse(args: &[OsString]) -> io::Result<Option<Arguments>> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        return Ok(None);
    }
    if args.len() > 64
        || args
            .iter()
            .try_fold(0usize, |sum, arg| sum.checked_add(arg.len()))
            .is_none_or(|bytes| bytes > 32 * 1024)
    {
        return Err(invalid("capacity fixture argument envelope"));
    }
    let mut store = None;
    let mut seed = None;
    let mut work_units = None;
    let mut validator = None;
    let mut at = 0;
    while at < args.len() {
        if args[at] == "--" {
            if validator.is_some() {
                return Err(invalid("duplicate capacity fixture validator separator"));
            }
            let tail = args.get(at + 1..).unwrap_or_default();
            if tail.is_empty() {
                return Err(invalid(
                    "capacity fixture requires protected validator selections",
                ));
            }
            validator = Some(tail.to_vec());
            break;
        }
        let option = args[at]
            .to_str()
            .ok_or_else(|| invalid("capacity fixture option encoding"))?;
        let value = args
            .get(at + 1)
            .ok_or_else(|| invalid("capacity fixture option value"))?;
        at += 2;
        match option {
            "--store" if store.is_none() => {
                store = Some(parse_path(value, "capacity fixture store path")?)
            }
            "--seed-sha256" if seed.is_none() => {
                let text = value
                    .to_str()
                    .ok_or_else(|| invalid("capacity fixture seed encoding"))?;
                if text.len() != 64 || text.bytes().any(|byte| byte.is_ascii_uppercase()) {
                    return Err(invalid("capacity fixture seed must be lowercase SHA-256"));
                }
                seed = Some(Digest256::from_hex(text).map_err(invalid)?);
            }
            "--work-units" if work_units.is_none() => {
                let text = value
                    .to_str()
                    .ok_or_else(|| invalid("capacity fixture work-unit encoding"))?;
                let units = text
                    .parse::<u64>()
                    .map_err(|_| invalid("capacity fixture work-unit value"))?;
                if units == 0 || units == u64::MAX {
                    return Err(invalid("capacity fixture work units must be finite"));
                }
                work_units = Some(units);
            }
            _ => return Err(invalid("capacity fixture option set")),
        }
    }
    let validator =
        validator.ok_or_else(|| invalid("capacity fixture validator separator absent"))?;
    let mut repository_root = None;
    let mut invocation = false;
    let mut index = 0;
    while index < validator.len() {
        let option = validator[index].to_str();
        if option == Some("--repo-root") {
            if repository_root.is_some() {
                return Err(invalid("duplicate capacity fixture repository root"));
            }
            let value = validator
                .get(index + 1)
                .ok_or_else(|| invalid("capacity fixture repository root value"))?;
            repository_root = Some(parse_path(value, "capacity fixture repository root")?);
            index += 2;
        } else if option == Some("--invocation") {
            if invocation {
                return Err(invalid("duplicate capacity fixture invocation"));
            }
            let _ = validator
                .get(index + 1)
                .ok_or_else(|| invalid("capacity fixture invocation value"))?;
            invocation = true;
            index += 2;
        } else {
            index += 1;
        }
    }
    if !invocation {
        return Err(invalid("capacity fixture invocation path absent"));
    }
    Ok(Some(Arguments {
        store: store.ok_or_else(|| invalid("capacity fixture store absent"))?,
        seed: seed.ok_or_else(|| invalid("capacity fixture seed absent"))?,
        work_units: work_units.ok_or_else(|| invalid("capacity fixture work units absent"))?,
        repository_root: repository_root
            .ok_or_else(|| invalid("capacity fixture repository root absent"))?,
        validator,
    }))
}

pub fn run_shared_cancel(
    args: &[OsString],
    cancelled: &Arc<AtomicBool>,
    git_signal: &AtomicI32,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<i32> {
    let clock = FoundationBootstrapClock::begin()
        .map_err(|_| invalid("capacity fixture bootstrap refused"))?;
    let output_bytes = Cell::new(0usize);
    let output_stopped = Cell::new(false);
    let output_cap = Cell::new(4096usize);
    let output_deadline = Cell::new(clock.hard_deadline());
    let result = run_selected(
        args,
        cancelled,
        git_signal,
        stdout,
        clock,
        &output_bytes,
        &output_stopped,
        &output_cap,
        &output_deadline,
    );
    if result.is_err() {
        let mut output = SelectedOutput {
            writer: stderr,
            bytes: &output_bytes,
            stopped: &output_stopped,
            max_bytes: output_cap.get(),
            deadline: output_deadline.get(),
            cancelled,
        };
        let _ = writeln!(output, "Native capacity fixture refused").and_then(|_| output.flush());
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_selected(
    raw_args: &[OsString],
    cancelled: &Arc<AtomicBool>,
    git_signal: &AtomicI32,
    stdout: &mut dyn Write,
    clock: FoundationBootstrapClock,
    output_bytes: &Cell<usize>,
    output_stopped: &Cell<bool>,
    output_cap: &Cell<usize>,
    output_deadline: &Cell<Instant>,
) -> io::Result<i32> {
    let Some(args) = parse(raw_args)? else {
        let mut output = SelectedOutput {
            writer: stdout,
            bytes: output_bytes,
            stopped: output_stopped,
            max_bytes: output_cap.get(),
            deadline: output_deadline.get(),
            cancelled,
        };
        output.write_all(HELP.as_bytes())?;
        output.flush()?;
        return Ok(0);
    };
    let select_output = |cap, deadline| {
        output_cap.set(usize::try_from(cap).unwrap_or(usize::MAX));
        output_deadline.set(deadline);
    };
    let protected_operation_started = clock.started();
    let mut validator = NativeSourceValidator::prepare_shared_cancel(
        clock,
        &args.validator,
        cancelled,
        git_signal,
        select_output,
    )?;
    let mut output = SelectedOutput {
        writer: stdout,
        bytes: output_bytes,
        stopped: output_stopped,
        max_bytes: output_cap.get(),
        deadline: output_deadline.get(),
        cancelled,
    };
    validator.bind_store_authority(&args.store)?;
    let candidate = validator.candidate_limits()?;
    let v2_profile = validator.prepared_v2_read_case_profile()?.clone();
    let execution = validator.prepared_execution_resources()?;
    let mut resources = match execution {
        PreparedAdmissionExecution::Spooled(resources) => resources,
        PreparedAdmissionExecution::Resident => {
            return Err(invalid("capacity fixture requires selected Native V2 case"));
        }
    };
    let case = resources
        .v2_case
        .as_ref()
        .ok_or_else(|| invalid("capacity fixture V2 case absent"))?;
    let artifact = resources
        .v2_target_root
        .as_ref()
        .ok_or_else(|| invalid("capacity fixture artifact root absent"))?;
    let tree_io = resources
        .v2_allocation_accountant
        .as_ref()
        .cloned()
        .ok_or_else(|| invalid("capacity fixture V2 IO ledger absent"))?;
    if case.files < 100_000
        || case.directories < 100_019
        || case.tree_rows < 100_000
        || case.object_bytes == 0
        || case.source_store_bytes == 0
        || case.target_store_bytes == 0
        || case.state_bytes != v2_profile.max_working_state_bytes
    {
        return Err(invalid(
            "selected Native V2 case is below the finite 100K fixture envelope",
        ));
    }
    let seed_text = args.seed.to_hex();
    let raw_root = artifact.path.join(format!("scale-raw-100k-{seed_text}"));
    let packed_root = artifact.path.join(format!("scale-packed-100k-{seed_text}"));
    for root in [&raw_root, &packed_root] {
        if root.starts_with(&case.target)
            || case.target.starts_with(root)
            || root.starts_with(&args.repository_root)
            || args.repository_root.starts_with(root)
        {
            return Err(invalid(
                "capacity fixture roots overlap protected source or target",
            ));
        }
    }
    let work = AdmissionWorkBudget::new(args.work_units)?;
    let caller_live_state_bytes = size_of::<Arguments>()
        .checked_add(size_of_val(&validator))
        .and_then(|bytes| bytes.checked_add(size_of_val(&resources)))
        .and_then(|bytes| bytes.checked_add(size_of_val(&v2_profile)))
        .and_then(|bytes| bytes.checked_add(size_of_val(&candidate)))
        .and_then(|bytes| bytes.checked_add(args.store.as_os_str().len()))
        .and_then(|bytes| bytes.checked_add(args.repository_root.as_os_str().len()))
        .and_then(|bytes| {
            args.validator
                .iter()
                .try_fold(bytes, |total, arg| total.checked_add(arg.len()))
        })
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .ok_or_else(|| invalid("capacity fixture caller-state estimate overflow"))?;
    if caller_live_state_bytes >= v2_profile.max_working_state_bytes {
        return Err(invalid(
            "capacity fixture caller state exceeds selected V2 state",
        ));
    }
    let member_bytes = u64::try_from(case.object_bytes)
        .map_err(|_| invalid("capacity fixture member cap exceeds u64"))?;
    let max_source_bytes = case.tree_bytes.min(candidate.max_read_bytes);
    let io_before = v2_profile.io.snapshot();
    let started = Instant::now();
    let producer_deadline = validator.deadline();
    let mut finalize = || {
        validator.verify_store_authority(&args.store)?;
        validator.finalize_without_evaluation()
    };
    let generated = produce_weighted_scale_input_v1(
        WeightedScaleProducerRequestV1 {
            repository_root: &args.repository_root,
            raw_input_root: &raw_root,
            output_root: &packed_root,
            profile: WeightedScaleProfileV1::fixed_100k(args.seed),
            segment_limits: resources
                .v2_base_read_limits
                .as_ref()
                .ok_or_else(|| invalid("capacity fixture segment limits absent"))?
                .segment,
            member_tree_limits: v2_profile.tree_limits,
            object_limits: PackedObjectLimitsV2 {
                tree_limits: v2_profile.tree_limits,
                max_working_state_bytes: v2_profile.max_working_state_bytes,
                caller_live_state_bytes,
                max_work_units: args.work_units,
                max_objects: case.tree_rows,
                max_delta_rows: case.tree_rows,
                max_pack_frames: MAX_PACKED_OBJECT_FRAMES_V2,
            },
            max_member_bytes: member_bytes,
            max_source_bytes,
            max_raw_input_files: case.files,
            max_raw_input_directories: case.directories,
            max_raw_input_allocated_bytes: case.source_store_bytes,
            max_temporary_logical_bytes: case.target_store_bytes,
            max_temporary_allocated_bytes: case.target_store_bytes,
            max_temporary_inodes: case.files,
            max_working_state_bytes: v2_profile.max_working_state_bytes,
            caller_live_state_bytes,
            deadline: producer_deadline,
            cancelled: cancelled.as_ref(),
            work: work.clone(),
            space: v2_profile.allocation_space.clone(),
            tree_io: Arc::clone(&tree_io),
        },
        &mut finalize,
    );
    drop(finalize);
    drop(resources.workspace);
    let verify_store = validator.verify_store_authority(&args.store);
    let cleanup =
        validator.cleanup_spooled_workspace(&resources.workspace_root, cancelled.as_ref());
    let receipt = match generated {
        Ok(receipt) => receipt,
        Err(error) => {
            verify_store?;
            cleanup?;
            return Err(error);
        }
    };
    verify_store?;
    cleanup?;
    if artifact.held.metadata()?.ino() != artifact.identity.1
        || artifact.held.metadata()?.dev() != artifact.identity.0
    {
        return Err(invalid("capacity fixture artifact root custody changed"));
    }
    let io_after = v2_profile.io.snapshot();
    let space = v2_profile.allocation_space.snapshot();
    let selected_profile = serde_json::json!({
        "source_store_bytes": case.source_store_bytes,
        "target_store_bytes": case.target_store_bytes,
        "total_store_bytes": case.source_store_bytes.saturating_add(case.target_store_bytes),
        "tree_bytes": case.tree_bytes,
        "point_tree_bytes": case.point_tree_bytes,
        "tree_nodes": case.tree_nodes,
        "tree_rows": case.tree_rows,
        "max_member_bytes": member_bytes,
        "max_source_bytes": max_source_bytes,
        "raw_file_cap": case.files,
        "raw_directory_cap": case.directories,
        "working_state_bytes": v2_profile.max_working_state_bytes,
        "allocation_unit_bytes": case.allocation_unit_bytes,
        "max_frames_per_pack": MAX_PACKED_OBJECT_FRAMES_V2,
        "work_units": args.work_units,
        "max_total_read_bytes_remaining": candidate.max_read_bytes,
        "max_total_write_bytes_remaining": candidate.max_write_bytes
    });
    write_receipt(
        &mut validator,
        &mut output,
        &args,
        &receipt,
        &selected_profile,
        work.used(),
        args.work_units,
        tree_io.actual_allocated_bytes(),
        space.reserved_high_water_bytes,
        space.actual_observed_high_water_bytes,
        io_after
            .read_attempted_bytes
            .saturating_sub(io_before.read_attempted_bytes),
        io_after
            .read_returned_bytes
            .saturating_sub(io_before.read_returned_bytes),
        io_after
            .write_attempted_bytes
            .saturating_sub(io_before.write_attempted_bytes),
        io_after
            .write_returned_bytes
            .saturating_sub(io_before.write_returned_bytes),
        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        u64::try_from(protected_operation_started.elapsed().as_millis()).unwrap_or(u64::MAX),
    )?;
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
fn write_receipt(
    validator: &mut NativeSourceValidator<'_>,
    output: &mut dyn Write,
    args: &Arguments,
    receipt: &PackedScaleInputReceiptV1,
    selected_profile: &serde_json::Value,
    work_used: u64,
    work_selected: u64,
    tree_allocated_bytes: u64,
    persistent_reserved_high_water: u64,
    persistent_actual_high_water: u64,
    read_attempted: u64,
    read_returned: u64,
    write_attempted: u64,
    write_returned: u64,
    producer_elapsed_ms: u64,
    protected_operation_elapsed_ms: u64,
) -> io::Result<()> {
    let forecast = &receipt.forecast;
    let report = serde_json::json!({
        "schema": "tos_native_weighted_capacity_fixture_receipt_v1",
        "source_status": "synthetic_private_fixture",
        "semantic_admission": false,
        "rights_change": false,
        "review_authority": false,
        "canon_authority": false,
        "selected_profile": selected_profile,
        "raw_input_root": receipt.raw_input_root.to_string_lossy(),
        "indexed_input_root": receipt.named_root.to_string_lossy(),
        "seed_sha256": args.seed.to_hex(),
        "template_source_commit": crate::source_capacity_workload::SCALE_TEMPLATE_SOURCE_COMMIT_V1,
        "member_count": receipt.member_count,
        "source_bytes": receipt.source_bytes,
        "deduplicated_payload_bytes": receipt.source_bytes.saturating_sub(receipt.unique_payload_bytes),
        "class_source_bytes": {
            "artifact": receipt.class_source_bytes[0],
            "claim": receipt.class_source_bytes[1],
            "evidence_packet": receipt.class_source_bytes[2],
            "text_unit": receipt.class_source_bytes[3],
            "work": receipt.class_source_bytes[4]
        },
        "raw_file_count": receipt.raw_input_file_count,
        "raw_directory_count": receipt.raw_input_directory_count,
        "raw_inode_count": receipt.raw_input_inode_count,
        "raw_source_bytes": receipt.raw_input_source_bytes,
        "raw_allocated_bytes": receipt.raw_input_allocated_bytes,
        "unique_object_count": receipt.unique_object_count,
        "unique_payload_bytes": receipt.unique_payload_bytes,
        "max_frames_per_pack": receipt.max_frames_per_pack,
        "members_descriptor_sha256": receipt.members_descriptor_sha256.to_hex(),
        "objects_descriptor_sha256": receipt.objects_descriptor_sha256.to_hex(),
        "manifest_sha256": receipt.manifest_sha256.to_hex(),
        "profile_sha256": receipt.profile_sha256.to_hex(),
        "dependency_closure_sha256": receipt.dependency_closure_sha256.to_hex(),
        "generated_dependency_edges": receipt.generated_dependency_edges,
        "pinned_external_dependency_edges": receipt.pinned_external_dependency_edges,
        "unresolved_dependency_edges": receipt.unresolved_dependency_edges,
        "authored_route_bridge_coverage": "unsupported_maintained_owner",
        "measured_temporary_logical_bytes": receipt.measured_temporary_logical_bytes,
        "measured_temporary_allocated_peak_bytes": receipt.measured_temporary_allocated_peak_bytes,
        "measured_temporary_file_inode_peak": receipt.measured_temporary_file_inode_peak,
        "member_tree_work": {
            "read_nodes": receipt.member_tree_work.read_nodes,
            "read_bytes": receipt.member_tree_work.read_bytes,
            "written_nodes": receipt.member_tree_work.written_nodes,
            "written_bytes": receipt.member_tree_work.written_bytes,
            "allocated_bytes": receipt.member_tree_work.allocated_bytes
        },
        "object_tree_work": {
            "read_nodes": receipt.object_build_work.tree_work.read_nodes,
            "read_bytes": receipt.object_build_work.tree_work.read_bytes,
            "written_nodes": receipt.object_build_work.tree_work.written_nodes,
            "written_bytes": receipt.object_build_work.tree_work.written_bytes,
            "allocated_bytes": receipt.object_build_work.tree_work.allocated_bytes
        },
        "packed_segment_work": {
            "read_bytes": receipt.object_build_work.segment_work.read_bytes,
            "read_upper_bound_bytes": receipt.object_build_work.segment_work.read_upper_bound_bytes,
            "write_bytes": receipt.object_build_work.segment_work.write_bytes,
            "allocation_reserved_bytes": receipt.object_build_work.segment_work.allocation_reserved_bytes,
            "allocated_bytes": receipt.object_build_work.segment_work.allocated_bytes,
            "work_units": receipt.object_build_work.segment_work.work_units,
            "work_bytes": receipt.object_build_work.segment_work.work_bytes
        },
        "tree_io_allocated_bytes": tree_allocated_bytes,
        "persistent_reserved_high_water_bytes": persistent_reserved_high_water,
        "persistent_actual_high_water_bytes": persistent_actual_high_water,
        "native_io_delta": {
            "read_attempted_bytes": read_attempted,
            "read_returned_bytes": read_returned,
            "write_attempted_bytes": write_attempted,
            "write_returned_bytes": write_returned
        },
        "work_units_used": work_used,
        "work_units_selected": work_selected,
        "producer_elapsed_ms": producer_elapsed_ms,
        "protected_operation_elapsed_ms": protected_operation_elapsed_ms,
        "forecast": {
            "p50_logical_source_bytes_100k": forecast.p50_logical_source_bytes_100k,
            "selected_quantile_logical_source_bytes_100k": forecast.selected_quantile_scenario_logical_source_bytes_100k,
            "selected_quantile_logical_source_bytes_1b": forecast.selected_quantile_scenario_logical_source_bytes_1b,
            "measured_100k_scaled_logical_source_bytes_1b": receipt.source_bytes.checked_mul(10_000),
            "measured_100k_scaled_unique_payload_bytes_1b": receipt.unique_payload_bytes.checked_mul(10_000),
            "ten_full_copies_without_dedup_bytes_1b": forecast.ten_full_copy_no_dedup_scenario_bytes_1b,
            "ten_full_copies_without_dedup_from_measured_100k_bytes_1b": receipt.source_bytes.checked_mul(100_000),
            "ten_full_copies_with_measured_unique_payload_bytes_1b": receipt.unique_payload_bytes.checked_mul(100_000),
            "history_change_rows_100k": forecast.history_change_rows_100k,
            "history_change_payload_scenario_bytes_100k": forecast.history_change_payload_scenario_bytes_100k,
            "revisions": forecast.revisions,
            "changed_rows_per_revision": forecast.changed_rows_per_revision,
            "policy_churn_rows_per_revision": forecast.policy_churn_rows_per_revision,
            "retained_pinned_revisions": forecast.retained_pinned_revisions,
            "normal_out_degree": forecast.normal_out_degree,
            "hot_hub_out_degree": forecast.hot_hub_out_degree,
            "expected_read_clients": forecast.expected_read_clients,
            "expected_write_clients": forecast.expected_write_clients,
            "concurrent_clients": forecast.concurrent_clients,
            "read_percent": forecast.read_percent,
            "write_percent": forecast.write_percent,
            "three_pins_backup_restore_no_dedup_p50_bytes_100k": forecast.three_pins_with_backup_and_restore_no_dedup_p50_bytes_100k,
            "selected_native_state_bytes_per_client": forecast.selected_native_state_bytes_per_client,
            "read_clients_state_upper_bytes": forecast.read_clients_state_upper_bytes,
            "write_clients_staging_upper_bytes": forecast.write_clients_staging_upper_bytes,
            "writer_callback_state_upper_bytes_per_client": forecast.writer_callback_state_upper_bytes_per_client,
            "write_clients_callback_state_upper_bytes": forecast.write_clients_callback_state_upper_bytes,
            "measured_unique_payload_ratio_100k": if receipt.source_bytes == 0 { None } else { Some(receipt.unique_payload_bytes as f64 / receipt.source_bytes as f64) },
            "full_256_peak_established": forecast.full_256_peak_established,
            "physical_fit_established": false,
            "authored_route_bridge_records_100k": forecast.authored_route_bridge_records_100k,
            "authored_route_bridge_coverage": forecast.authored_route_bridge_coverage
        }
    });
    validator.write_receipt(&report, output)
}
