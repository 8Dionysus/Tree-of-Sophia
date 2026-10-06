//! Maintained weighted capacity-fixture producer entry.
//!
//! The declared recipe selects packed-only output from the authenticated typed
//! cursor; the legacy route also retains a standalone raw mirror. Its receipt
//! records mechanical production only. Native admission validates the selected
//! indexed input independently.

use crate::source_admission::AdmissionWorkBudget;
use crate::source_admission_packed_objects::{MAX_PACKED_OBJECT_FRAMES_V2, PackedObjectLimitsV2};
use crate::source_capacity_workload::{
    PackedScaleInputReceiptV1, WeightedScaleAuthoredAuxMemberV1,
    WeightedScaleAuthoredAuxSelectionV1, WeightedScaleProducerRequestV1, WeightedScaleProfileV1,
    WeightedScaleRepresentationV1, load_declared_fixture_templates_accounted,
    produce_weighted_scale_input_v1,
    produce_weighted_scale_input_with_authored_aux_and_templates_v1,
    produce_weighted_scale_input_with_authored_aux_v1,
    weighted_scale_composed_producer_envelope_v1,
    weighted_scale_composed_producer_envelope_with_templates_v2,
    weighted_scale_producer_envelope_v1,
};
use crate::source_command::{SourceCommandError, public_io_reason};
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

pub const HELP: &str = "usage: tos-native-owner-command capacity-fixture --store PATH --seed-sha256 LOWERHEX64 --work-units N [--target-records N] [--record-selection-manifest ABS] [--fixture-recipe ABS --fixture-recipe-sha256 LOWERHEX64] -- --repo-root ABS --invocation ABS [native validator selections]\n\nCreate deterministic private scale input for the declared weighted record count (default 100000) under the protected artifact root. A declared recipe with authored selection produces packed-only input; the legacy route also emits a standalone raw mirror. The five-class 5/40/5/15/35 distribution is priced against the exact selected Native limits before writing. Then run corpus-admit with the explicit validation profile and printed indexed input. Recipe selection does not itself validate semantics. This fixture does not grant source, review, rights, canon, or admission authority.\n";

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        SourceCommandError::Invalid(reason),
    )
}

struct Arguments {
    store: PathBuf,
    seed: Digest256,
    work_units: u64,
    target_records: u64,
    repository_root: PathBuf,
    record_selection_manifest: Option<PathBuf>,
    fixture_recipe: Option<PathBuf>,
    fixture_recipe_sha256: Option<Digest256>,
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
    let mut target_records = None;
    let mut record_selection_manifest = None;
    let mut fixture_recipe = None;
    let mut fixture_recipe_sha256 = None;
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
                seed = Some(
                    Digest256::from_hex(text)
                        .map_err(|_| invalid("capacity fixture seed digest refused"))?,
                );
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
            "--record-selection-manifest" if record_selection_manifest.is_none() => {
                record_selection_manifest = Some(parse_path(
                    value,
                    "capacity fixture record selection manifest",
                )?);
            }
            "--fixture-recipe" if fixture_recipe.is_none() => {
                fixture_recipe = Some(parse_path(value, "capacity fixture recipe path")?);
            }
            "--fixture-recipe-sha256" if fixture_recipe_sha256.is_none() => {
                let text = value
                    .to_str()
                    .ok_or_else(|| invalid("capacity fixture recipe digest encoding"))?;
                if text.len() != 64 || text.bytes().any(|byte| byte.is_ascii_uppercase()) {
                    return Err(invalid(
                        "capacity fixture recipe digest must be lowercase SHA-256",
                    ));
                }
                fixture_recipe_sha256 = Some(
                    Digest256::from_hex(text)
                        .map_err(|_| invalid("capacity fixture recipe digest refused"))?,
                );
            }
            "--target-records" if target_records.is_none() => {
                let text = value
                    .to_str()
                    .ok_or_else(|| invalid("capacity fixture target-record encoding"))?;
                let records = text
                    .parse::<u64>()
                    .map_err(|_| invalid("capacity fixture target-record value"))?;
                if records < 20 || records == u64::MAX {
                    return Err(invalid(
                        "capacity fixture target records must be finite and represent all five classes",
                    ));
                }
                target_records = Some(records);
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
    if fixture_recipe.is_some() != fixture_recipe_sha256.is_some() {
        return Err(invalid(
            "capacity fixture recipe path and digest must be selected together",
        ));
    }
    if fixture_recipe.is_some() && record_selection_manifest.is_none() {
        return Err(invalid(
            "declared capacity fixture recipe requires authored record selection",
        ));
    }
    Ok(Some(Arguments {
        store: store.ok_or_else(|| invalid("capacity fixture store absent"))?,
        seed: seed.ok_or_else(|| invalid("capacity fixture seed absent"))?,
        work_units: work_units.ok_or_else(|| invalid("capacity fixture work units absent"))?,
        target_records: target_records.unwrap_or(100_000),
        record_selection_manifest,
        fixture_recipe,
        fixture_recipe_sha256,
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
    if let Err(error) = result.as_ref() {
        let mut output = SelectedOutput {
            writer: stderr,
            bytes: &output_bytes,
            stopped: &output_stopped,
            max_bytes: output_cap.get(),
            deadline: output_deadline.get(),
            cancelled,
        };
        let _ = writeln!(
            output,
            "Native capacity fixture refused: {}",
            public_io_reason(error)
        )
        .and_then(|_| output.flush());
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
    let execution = validator.prepared_execution_resources()?;
    let mut resources = match execution {
        PreparedAdmissionExecution::Spooled(resources) => resources,
        PreparedAdmissionExecution::Resident => {
            return Err(invalid("capacity fixture requires selected Native V2 case"));
        }
    };
    // Resource preparation issues the selected V2 profile and narrows the
    // candidate envelope against the same remaining operation budget.
    let candidate = resources.candidate_limits.candidate;
    let v2_profile = validator.prepared_v2_read_case_profile()?.clone();
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
    if case.object_bytes == 0
        || case.source_store_bytes == 0
        || case.target_store_bytes == 0
        || case.state_bytes != v2_profile.max_working_state_bytes
    {
        return Err(invalid(
            "selected Native V2 case has no finite capacity-fixture limits",
        ));
    }
    let mut profile = WeightedScaleProfileV1::weighted_for_records(args.seed, args.target_records)?;
    let target_records = profile.target_records;

    let total_store_bytes = case
        .source_store_bytes
        .checked_add(case.target_store_bytes)
        .and_then(|bytes| bytes.checked_add(case.sqlite_store_bytes))
        .ok_or_else(|| invalid("capacity fixture selected store sum overflow"))?;
    let seed_text = args.seed.to_hex();
    let count_label = if profile.target_records == 100_000 {
        "100k".to_owned()
    } else {
        profile.target_records.to_string()
    };
    let raw_root = artifact
        .path
        .join(format!("scale-raw-{count_label}-{seed_text}"));
    let packed_root = artifact
        .path
        .join(format!("scale-packed-{count_label}-{seed_text}"));
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
    let mut caller_live_state_bytes = size_of::<Arguments>()
        .checked_add(size_of_val(&validator))
        .and_then(|bytes| bytes.checked_add(size_of_val(&resources)))
        .and_then(|bytes| bytes.checked_add(size_of_val(&v2_profile)))
        .and_then(|bytes| bytes.checked_add(size_of_val(&candidate)))
        .and_then(|bytes| bytes.checked_add(args.store.as_os_str().len()))
        .and_then(|bytes| bytes.checked_add(args.repository_root.as_os_str().len()))
        .and_then(|bytes| {
            bytes.checked_add(
                args.record_selection_manifest
                    .as_ref()
                    .map_or(0, |path| path.as_os_str().len()),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                args.fixture_recipe
                    .as_ref()
                    .map_or(0, |path| path.as_os_str().len()),
            )
        })
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
    let validator_deadline = validator.deadline();
    let io_before = v2_profile.io.snapshot();
    let started = Instant::now();
    let authored = if let Some(path) = args.record_selection_manifest.as_deref() {
        Some(select_authored_aux_v1(
            path,
            &mut validator,
            &v2_profile.io,
            case.object_bytes,
            case.files
                .checked_sub(target_records)
                .ok_or_else(|| invalid("capacity fixture auxiliary file cap absent"))?,
            candidate.max_read_bytes,
            v2_profile.max_working_state_bytes,
            caller_live_state_bytes,
            validator_deadline,
            cancelled.as_ref(),
        )?)
    } else {
        None
    };
    if let Some(selection) = &authored {
        caller_live_state_bytes = caller_live_state_bytes
            .checked_add(selection.retained_state_bytes)
            .ok_or_else(|| invalid("capacity fixture authored caller state overflow"))?;
    }
    let templates = if let Some(recipe) = &args.fixture_recipe {
        let relative_recipe = recipe
            .strip_prefix(&args.repository_root)
            .ok()
            .and_then(Path::to_str)
            .ok_or_else(|| {
                invalid("capacity fixture recipe must belong to selected source root")
            })?;
        let templates = load_declared_fixture_templates_accounted(
            &args.repository_root,
            relative_recipe,
            args.fixture_recipe_sha256
                .ok_or_else(|| invalid("capacity fixture recipe digest absent"))?,
            &profile,
            &v2_profile.io,
            validator_deadline,
            cancelled.as_ref(),
            &work,
            v2_profile.max_working_state_bytes,
            caller_live_state_bytes,
        )?;
        profile = templates.profile_with_selected_dimensions(profile)?;
        Some(templates)
    } else {
        None
    };
    let representation = if templates.is_some() {
        WeightedScaleRepresentationV1::PackedOnlyV2
    } else {
        WeightedScaleRepresentationV1::RawAndPackedV1
    };
    let selected_raw_root = if templates.is_some() {
        None
    } else {
        Some(raw_root.as_path())
    };
    let (forecast, envelope) = if let Some(templates) = &templates {
        let selection = authored
            .as_ref()
            .ok_or_else(|| invalid("declared capacity fixture authored selection absent"))?;
        let price = weighted_scale_composed_producer_envelope_with_templates_v2(
            &profile,
            templates,
            &selection.auxiliary,
            representation,
            tree_io.selected_allocation_unit_bytes(),
            selected_raw_root,
            v2_profile.max_working_state_bytes,
            caller_live_state_bytes,
        )?;
        (price.forecast, price.envelope)
    } else if let Some(selection) = &authored {
        let price = weighted_scale_composed_producer_envelope_v1(
            &profile,
            &selection.auxiliary,
            tree_io.selected_allocation_unit_bytes(),
            &raw_root,
            v2_profile.max_working_state_bytes,
            caller_live_state_bytes,
        )?;
        (price.forecast, price.envelope)
    } else {
        (
            profile.forecast_inputs()?,
            weighted_scale_producer_envelope_v1(
                &profile,
                tree_io.selected_allocation_unit_bytes(),
            )?,
        )
    };
    let member_bytes = u64::try_from(case.object_bytes)
        .map_err(|_| invalid("capacity fixture member cap exceeds u64"))?;
    let max_source_bytes = case.tree_bytes.min(candidate.max_read_bytes);
    let producer_deadline = validator.deadline();
    let mut finalize = || {
        if let Some(selection) = &authored {
            crate::source_text_owner::verify_held_file_with_io(
                &selection.held,
                unsafe { libc::geteuid() },
                producer_deadline,
                cancelled.as_ref(),
                &v2_profile.io,
            )
            .map_err(|_| invalid("capacity fixture authored manifest custody changed"))?;
        }
        validator.verify_store_authority(&args.store)?;
        validator.finalize_without_evaluation()
    };
    let producer_request = WeightedScaleProducerRequestV1 {
        repository_root: &args.repository_root,
        representation,
        raw_input_root: selected_raw_root,
        output_root: &packed_root,
        profile,
        segment_limits: resources
            .v2_base_read_limits
            .as_ref()
            .ok_or_else(|| invalid("capacity fixture segment limits absent"))?
            .segment,
        member_tree_limits: v2_profile.tree_limits,
        object_limits: PackedObjectLimitsV2 {
            segment_limits: resources
                .v2_base_read_limits
                .as_ref()
                .ok_or_else(|| invalid("capacity fixture segment limits absent"))?
                .segment,
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
    };
    let class_counts = producer_request.profile.classes.map(|row| row.count);
    let generated = if let Some(templates) = templates {
        let selection = authored
            .as_ref()
            .ok_or_else(|| invalid("declared capacity fixture authored selection absent"))?;
        produce_weighted_scale_input_with_authored_aux_and_templates_v1(
            producer_request,
            &selection.auxiliary,
            templates,
            &mut finalize,
        )
    } else if let Some(selection) = &authored {
        produce_weighted_scale_input_with_authored_aux_v1(
            producer_request,
            &selection.auxiliary,
            &mut finalize,
        )
    } else {
        produce_weighted_scale_input_v1(producer_request, &mut finalize)
    };
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
    let mut selected_profile = serde_json::json!({
        "target_records": target_records,
        "class_counts": class_counts,
        "source_store_bytes": case.source_store_bytes,
        "target_store_bytes": case.target_store_bytes,
        "total_store_bytes": total_store_bytes,
        "producer_prewrite_price": {
            "source_upper_bytes": envelope.maximum_source_bytes,
            "raw_allocated_upper_bytes": envelope.raw_input_allocated_bytes,
            "scratch_logical_upper_bytes": envelope.temporary_logical_bytes,
            "scratch_allocated_upper_bytes": envelope.temporary_allocated_bytes,
            "scratch_file_inode_upper": envelope.temporary_file_inodes
        },
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
        "max_total_write_bytes_remaining": candidate.max_write_bytes,
        "profile_forecast": {
            "target_records": forecast.target_records,
            "raw_input_file_count": forecast.raw_input_file_count,
            "raw_input_directory_count": forecast.raw_input_directory_count,
            "raw_input_inode_count": forecast.raw_input_inode_count,
            "p50_logical_source_bytes_at_profile_size": forecast.p50_logical_source_bytes,
            "selected_quantile_logical_source_bytes_at_profile_size": forecast
                .selected_quantile_scenario_logical_source_bytes,
            "selected_quantile_logical_source_bytes_at_1b": forecast
                .selected_quantile_scenario_logical_source_bytes_at_1b,
            "external_sort_logical_bytes": forecast.external_sort_logical_bytes,
            "temporary_payload_spool_peak_bytes": forecast.temporary_payload_spool_peak_bytes,
            "temporary_digest_sort_peak_bytes": forecast.temporary_digest_sort_peak_bytes,
            "temporary_scratch_peak_bytes": forecast.temporary_scratch_peak_bytes,
            "temporary_scratch_blocks_4k_assumption": forecast.temporary_scratch_blocks_4k_assumption,
            "temporary_file_inode_peak": forecast.temporary_file_inode_peak,
            "history_change_rows": forecast.history_change_rows,
            "history_change_payload_scenario_bytes": forecast
                .history_change_payload_scenario_bytes,
            "physical_fit_established": forecast.physical_fit_established
        }
    });
    if case.sqlite_store_bytes != 0 {
        selected_profile["consumer_sqlite_store_bytes_selected"] =
            serde_json::json!(case.sqlite_store_bytes);
        selected_profile["producer_storage_bytes_selected"] = serde_json::json!(
            case.source_store_bytes
                .checked_add(case.target_store_bytes)
                .ok_or_else(|| invalid("capacity fixture producer selected store sum overflow"))?
        );
    }
    if let Some(composition) = &receipt.composition {
        selected_profile["generated_record_count"] =
            serde_json::json!(composition.generated_record_count);
        selected_profile["auxiliary_member_count"] =
            serde_json::json!(composition.auxiliary_member_count);
        selected_profile["auxiliary_source_bytes"] =
            serde_json::json!(composition.auxiliary_source_bytes);
        selected_profile["physical_member_count"] = serde_json::json!(receipt.member_count);
    }
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

struct AuthoredAuxCustodyV1 {
    held: crate::source_text_owner::HeldOwnerFile,
    // Retain the exact finite record/slot selection beside its held manifest.
    // The producer copies selected physical bytes; semantic admission is later.
    _selection: tos_validation::source_record_selection::SourceRecordSelection,
    auxiliary: WeightedScaleAuthoredAuxSelectionV1,
    retained_state_bytes: usize,
}

#[allow(clippy::too_many_arguments)]
fn select_authored_aux_v1(
    path: &Path,
    validator: &mut NativeSourceValidator<'_>,
    io: &tos_source_store::PinnedSqliteIoBudget,
    max_member_bytes: usize,
    max_auxiliary_members: u64,
    max_read_bytes: u64,
    max_working_state_bytes: usize,
    caller_live_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> io::Result<AuthoredAuxCustodyV1> {
    use tos_validation::source_record_selection::{
        SelectionLimits, SourceRecordSelection, selection_state_upper_bound,
    };
    if max_auxiliary_members == 0 || max_auxiliary_members == u64::MAX {
        return Err(invalid("capacity fixture auxiliary member cap absent"));
    }
    let custody_state = path
        .as_os_str()
        .len()
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(size_of::<crate::source_text_owner::HeldOwnerFile>()))
        .and_then(|bytes| bytes.checked_add(size_of::<AuthoredAuxCustodyV1>()))
        .ok_or_else(|| invalid("capacity fixture authored custody state overflow"))?;
    let model_state = max_working_state_bytes
        .checked_sub(caller_live_state_bytes)
        .and_then(|bytes| bytes.checked_sub(custody_state))
        .ok_or_else(|| invalid("capacity fixture authored state slice absent"))?;
    let mut low = 0usize;
    // Selection and final custody each read the held/named file: four bounded
    // EOF reads, each with the owner reader's one-byte overflow check.
    let read_cap = max_read_bytes
        .checked_div(4)
        .and_then(|bytes| bytes.checked_sub(1))
        .ok_or_else(|| invalid("capacity fixture authored read envelope absent"))?;
    let mut high = max_member_bytes.min(usize::try_from(read_cap).unwrap_or(usize::MAX));
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if selection_state_upper_bound(middle).is_ok_and(|bytes| bytes < model_state) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    if low == 0 {
        return Err(invalid("capacity fixture authored manifest bound absent"));
    }
    let reserve = selection_state_upper_bound(low)
        .map_err(|_| invalid("capacity fixture authored model state refused"))?
        .checked_add(custody_state)
        .ok_or_else(|| invalid("capacity fixture authored state overflow"))?;
    validator.reserve_spooled_external_state(reserve, io)?;
    let (held, raw) = crate::source_text_owner::select_held_file_with_io(
        path,
        unsafe { libc::geteuid() },
        false,
        low,
        deadline,
        cancelled,
        io,
    )
    .map_err(|_| invalid("capacity fixture authored manifest custody refused"))?;
    let model_upper = selection_state_upper_bound(raw.len())
        .map_err(|_| invalid("capacity fixture authored model state refused"))?;
    let verify_state = model_state
        .checked_sub(model_upper)
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| invalid("capacity fixture authored verification state absent"))?;
    let selection = SourceRecordSelection::parse(
        &raw,
        SelectionLimits {
            max_manifest_bytes: low,
            max_records: raw.len(),
            max_slots: raw.len(),
            max_roots: raw.len(),
            max_owned_state_bytes: model_state,
            max_row_bytes: max_member_bytes,
            max_verify_state_bytes: verify_state,
        },
    )
    .map_err(|_| invalid("capacity fixture authored record selection refused"))?;
    if selection.member_count() as u64 > max_auxiliary_members {
        return Err(invalid(
            "capacity fixture auxiliary selection exceeds raw file cap",
        ));
    }
    let authored_manifest_sha256 = selection.digest();
    if authored_manifest_sha256 != Digest256::of_bytes(&raw) {
        return Err(invalid("capacity fixture authored manifest digest differs"));
    }
    let retained_state_bytes = selection
        .charged_state_bytes()
        .checked_add(
            held.retained_state_bytes()
                .ok_or_else(|| invalid("capacity fixture authored held state overflow"))?,
        )
        .and_then(|bytes| bytes.checked_add(size_of::<AuthoredAuxCustodyV1>()))
        .ok_or_else(|| invalid("capacity fixture authored retained state overflow"))?;
    drop(raw);
    let available_aux_state = max_working_state_bytes
        .checked_sub(caller_live_state_bytes)
        .and_then(|bytes| bytes.checked_sub(retained_state_bytes))
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| invalid("capacity fixture authored descriptor state absent"))?;
    let descriptor_state = selection.members().try_fold(
        selection
            .member_count()
            .checked_mul(size_of::<WeightedScaleAuthoredAuxMemberV1>())
            .ok_or_else(|| invalid("capacity fixture authored descriptor state overflow"))?,
        |bytes, member| {
            bytes
                .checked_add(member.source_ref.len())
                .ok_or_else(|| invalid("capacity fixture authored path state overflow"))
        },
    )?;
    if descriptor_state
        .checked_add(size_of::<WeightedScaleAuthoredAuxSelectionV1>())
        .is_none_or(|bytes| bytes > available_aux_state)
    {
        return Err(invalid(
            "capacity fixture authored descriptors exceed selected state",
        ));
    }
    let mut members = Vec::new();
    members
        .try_reserve_exact(selection.member_count())
        .map_err(|_| invalid("capacity fixture authored descriptor allocation refused"))?;
    for member in selection.members() {
        if !crate::source_current_cut::foundation_capture::selected(&member.source_ref, false) {
            continue;
        }
        members.push(WeightedScaleAuthoredAuxMemberV1 {
            path: member.source_ref.clone(),
            raw_sha256: Digest256::from_hex(&member.raw_sha256)
                .map_err(|_| invalid("capacity fixture authored member digest differs"))?,
            raw_bytes: member.raw_bytes,
        });
    }
    let auxiliary = WeightedScaleAuthoredAuxSelectionV1::from_owner_selection(
        authored_manifest_sha256,
        members,
        selection.member_count(),
        max_read_bytes,
        available_aux_state,
    )?;
    Ok(AuthoredAuxCustodyV1 {
        held,
        _selection: selection,
        auxiliary,
        retained_state_bytes,
    })
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
    let generated_source_bytes =
        receipt
            .class_source_bytes
            .iter()
            .try_fold(0u64, |total, bytes| {
                total
                    .checked_add(*bytes)
                    .ok_or_else(|| invalid("capacity fixture generated source total overflow"))
            })?
            .checked_add(receipt.generated_support_source_bytes)
            .ok_or_else(|| invalid("capacity fixture generated support total overflow"))?;
    let (
        measured_source_at_1b,
        measured_source_ten_copies_at_1b,
        measured_unique_at_1b,
        measured_unique_ten_copies_at_1b,
    ) = if let Some(composition) = &receipt.composition {
        if generated_source_bytes.checked_add(composition.auxiliary_source_bytes)
            != Some(receipt.source_bytes)
            || composition
                .generated_record_count
                .checked_add(composition.generated_support_member_count)
                .and_then(|count| count.checked_add(composition.auxiliary_member_count))
                != Some(receipt.member_count)
        {
            return Err(invalid("capacity fixture composed receipt totals differ"));
        }
        let one = project_measured_bytes(
            generated_source_bytes,
            composition.generated_record_count,
            1_000_000_000,
        )?
        .checked_add(composition.auxiliary_source_bytes)
        .ok_or_else(|| invalid("capacity fixture composed projection overflow"))?;
        let ten = project_measured_bytes(
            generated_source_bytes,
            composition.generated_record_count,
            10_000_000_000,
        )?
        .checked_add(
            composition
                .auxiliary_source_bytes
                .checked_mul(10)
                .ok_or_else(|| invalid("capacity fixture composed projection overflow"))?,
        )
        .ok_or_else(|| invalid("capacity fixture composed projection overflow"))?;
        // Shared deduplication has no measured generated/auxiliary split.
        // A unique 1B projection cannot be derived from aggregate objects.
        (one, ten, None, None)
    } else {
        (
            project_measured_bytes(receipt.source_bytes, receipt.member_count, 1_000_000_000)?,
            project_measured_bytes(receipt.source_bytes, receipt.member_count, 10_000_000_000)?,
            Some(project_measured_bytes(
                receipt.unique_payload_bytes,
                receipt.member_count,
                1_000_000_000,
            )?),
            Some(project_measured_bytes(
                receipt.unique_payload_bytes,
                receipt.member_count,
                10_000_000_000,
            )?),
        )
    };
    let forecast_report = serde_json::json!({
        "target_records": forecast.target_records,
        "p50_logical_source_bytes_at_profile_size": forecast.p50_logical_source_bytes,
        "selected_quantile_logical_source_bytes_at_profile_size": forecast
            .selected_quantile_scenario_logical_source_bytes,
        "p50_logical_source_bytes_at_1b": forecast.p50_logical_source_bytes_at_1b,
        "selected_quantile_logical_source_bytes_at_1b": forecast
            .selected_quantile_scenario_logical_source_bytes_at_1b,
        "measured_profile_scaled_logical_source_bytes_at_1b": measured_source_at_1b,
        "measured_profile_scaled_unique_payload_bytes_at_1b": measured_unique_at_1b,
        "ten_full_copies_without_dedup_bytes_at_1b": forecast
            .ten_full_copy_no_dedup_scenario_bytes_at_1b,
        "ten_full_copies_without_dedup_from_measured_profile_bytes_at_1b": measured_source_ten_copies_at_1b,
        "ten_full_copies_with_measured_unique_payload_bytes_at_1b": measured_unique_ten_copies_at_1b,
        "history_change_rows": forecast.history_change_rows,
        "history_change_payload_scenario_bytes": forecast
            .history_change_payload_scenario_bytes,
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
        "current_snapshot_three_copy_p50_bytes_at_profile_size": forecast
            .current_snapshot_three_copy_p50_bytes,
        "three_pins_backup_restore_no_dedup_p50_bytes_at_profile_size": forecast
            .three_pins_with_backup_and_restore_no_dedup_p50_bytes,
        "selected_native_state_bytes_per_client": forecast.selected_native_state_bytes_per_client,
        "read_clients_state_upper_bytes": forecast.read_clients_state_upper_bytes,
        "write_clients_staging_upper_bytes": forecast.write_clients_staging_upper_bytes,
        "writer_callback_state_upper_bytes_per_client": forecast.writer_callback_state_upper_bytes_per_client,
        "write_clients_callback_state_upper_bytes": forecast.write_clients_callback_state_upper_bytes,
        "measured_unique_payload_ratio_at_profile_size": if receipt.source_bytes == 0 {
            None
        } else {
            Some(receipt.unique_payload_bytes as f64 / receipt.source_bytes as f64)
        },
        "full_256_peak_established": forecast.full_256_peak_established,
        "physical_fit_established": false,
        "authored_route_bridge_records": forecast.authored_route_bridge_records,
        "authored_route_bridge_coverage": forecast.authored_route_bridge_coverage
    });
    let serde_json::Value::Object(mut report_fields) = serde_json::json!({
        "schema": "tos_native_weighted_capacity_fixture_receipt_v2",
        "source_status": "synthetic_private_fixture",
        "semantic_admission": false,
        "rights_change": false,
        "review_authority": false,
        "canon_authority": false,
        "selected_profile": selected_profile,
        "indexed_input_root": receipt.named_root.to_string_lossy(),
        "seed_sha256": args.seed.to_hex(),
        "input_representation": match receipt.representation {
            WeightedScaleRepresentationV1::RawAndPackedV1 => "raw_and_packed_v1",
            WeightedScaleRepresentationV1::PackedOnlyV2 => "packed_only_v2",
        },
        "standalone_raw_mirror": receipt.raw_input.is_some(),
        "member_count": receipt.member_count,
        "source_bytes": receipt.source_bytes,
        "generated_support_source_bytes": receipt.generated_support_source_bytes,
        "deduplicated_payload_bytes": receipt.source_bytes.saturating_sub(receipt.unique_payload_bytes),
        "class_source_bytes": {
            "artifact": receipt.class_source_bytes[0],
            "claim": receipt.class_source_bytes[1],
            "evidence_packet": receipt.class_source_bytes[2],
            "text_unit": receipt.class_source_bytes[3],
            "work": receipt.class_source_bytes[4]
        }
    }) else {
        return Err(invalid(
            "capacity fixture identity receipt must be an object",
        ));
    };
    if let Some(raw) = &receipt.raw_input {
        report_fields.insert(
            "raw_input_root".into(),
            serde_json::json!(raw.raw_input_root.to_string_lossy()),
        );
        report_fields.insert(
            "raw_file_count".into(),
            serde_json::json!(raw.raw_input_file_count),
        );
        report_fields.insert(
            "raw_directory_count".into(),
            serde_json::json!(raw.raw_input_directory_count),
        );
        report_fields.insert(
            "raw_inode_count".into(),
            serde_json::json!(raw.raw_input_inode_count),
        );
        report_fields.insert(
            "raw_source_bytes".into(),
            serde_json::json!(raw.raw_input_source_bytes),
        );
        report_fields.insert(
            "raw_allocated_bytes".into(),
            serde_json::json!(raw.raw_input_allocated_bytes),
        );
    }
    if let Some(recipe) = receipt.source_recipe() {
        report_fields.insert(
            "fixture_recipe_source".into(),
            serde_json::json!({
                "source_path": recipe.source_path(),
                "source_sha256": recipe.source_sha256().to_hex(),
                "source_bytes": recipe.source_bytes(),
                "binding": "authenticated_selected_input_copy",
            }),
        );
    } else {
        report_fields.insert(
            "template_source_commit".into(),
            serde_json::json!(crate::source_capacity_workload::SCALE_TEMPLATE_SOURCE_COMMIT_V1),
        );
    }
    if let Some(composition) = &receipt.composition {
        report_fields.insert("authored_aux_composition".into(), serde_json::json!({
            "coverage": "authenticated_byte_composition_pending_semantic_admission",
            "authored_manifest_sha256": composition.authored_manifest_sha256.to_hex(),
            "generated_declaration_sha256": composition.generated_declaration_sha256.to_hex(),
            "auxiliary_members_sha256": composition.auxiliary_members_sha256.to_hex(),
            "generated_record_count": composition.generated_record_count,
            "generated_support_member_count": composition.generated_support_member_count,
            "auxiliary_member_count": composition.auxiliary_member_count,
            "auxiliary_source_bytes": composition.auxiliary_source_bytes,
            "members_descriptor_sha256": composition.members_descriptor_sha256.to_hex(),
            "unique_payload_1b_projection": "unavailable_without_generated_auxiliary_dedup_split"
        }));
    }
    let serde_json::Value::Object(measurement_fields) = serde_json::json!({
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
        "forecast": forecast_report
    }) else {
        return Err(invalid("capacity fixture measurements must be an object"));
    };
    report_fields.extend(measurement_fields);
    let report = serde_json::Value::Object(report_fields);
    validator.write_receipt(&report, output)
}

fn project_measured_bytes(bytes: u64, source_records: u64, target_records: u64) -> io::Result<u64> {
    if source_records == 0 {
        return Err(invalid(
            "capacity fixture measured projection source is empty",
        ));
    }
    let denominator = source_records as u128;
    let numerator = (bytes as u128)
        .checked_mul(target_records as u128)
        .ok_or_else(|| invalid("capacity fixture measured projection overflows u128"))?;
    let rounded = numerator
        .checked_add(denominator - 1)
        .ok_or_else(|| invalid("capacity fixture measured projection rounding overflows"))?
        / denominator;
    u64::try_from(rounded).map_err(|_| invalid("capacity fixture measured projection exceeds u64"))
}
