//! Invocation-derived, finite ceilings for the first native source phase.
//!
//! These are admission ceilings, not observations of the selected checkout.
//! Bootstrap intersects them with the live whole-operation ticket after it
//! has charged invocation custody and the selected roots.

use super::foundation_bootstrap::FoundationBootstrapConfig;
use super::foundation_capture::FoundationCapturedCut;
use super::foundation_entry::{FoundationInvocation, FoundationSelectedRoots};
use super::foundation_execution_limits::{FoundationExecutionLimits, FoundationRemainingBudget};
use super::foundation_payload::PhysicalPayloadLimits;
use super::foundation_physical::PhysicalSourceLimits;
use super::foundation_selection::{FoundationPhysicalSelection, FoundationSelectionLimits};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_creation_store::{MAX_BYTES, MAX_FILES};
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use tos_compiler::private_tmpfs_stage::{PRIVATE_TMPFS_SELECT_COST, PRIVATE_TMPFS_VERIFY_COST};
use tos_foundation::{JsonLimits, JsonString, JsonValue, RelativePath};
use tos_ops_mechanics_plan::route_cards::RouteSources;
use tos_source_store::{CorpusReader, CutReadLimits, MemberMetadata, ReadLimits};
use tos_validation::source_foundation_discovery::SourcePhysicalFacts;

const PUBLICATION_CONTROL_READ_CAP_BYTES: u64 = 8_192;
const CAPTURE_CONTROL_READS: u64 = 2;
const CAPTURE_MEMBER_READ_PASSES: u64 = 3;
const CAPTURE_TMPFS_FIXED_INODES: u64 = 8;
// Keep this aligned with the protected launch parser's selected-path cap.
const MAX_RELATIVE_PATH_BYTES: usize = 4_096;
const MIN_CAPTURE_MANIFEST_ENVELOPE_BYTES: usize = 512;
const MIN_CAPTURE_MEMBER_ROW_BYTES: usize = 112;
const MIN_CAPTURE_MANIFEST_BYTES: usize =
    MIN_CAPTURE_MANIFEST_ENVELOPE_BYTES + MIN_CAPTURE_MEMBER_ROW_BYTES;
const CAPTURE_MANIFEST_FIXED_VISITS: usize = 9;
const CAPTURE_MANIFEST_VISITS_PER_MEMBER: usize = 5;
const DEFAULT_PAYLOAD_SUFFIX: &str = "ToS/source-witnesses";

/// Convert the selected protected invocation into concrete first-phase caps.
/// No current paths, source bytes, or physical observations are inferred here.
pub(crate) fn from_invocation(
    invocation: &FoundationInvocation<'_>,
) -> Result<FoundationBootstrapConfig> {
    from_invocation_profile(invocation, false)
}

/// Candidate capture uses the same protected invocation-derived limits as the
/// ordinary source route, but does not inherit the unrelated creation helper's
/// per-call file-count and aggregate-byte ceilings.
pub(crate) fn from_candidate_invocation(
    invocation: &FoundationInvocation<'_>,
) -> Result<FoundationBootstrapConfig> {
    from_invocation_profile(invocation, true)
}

fn from_invocation_profile(
    invocation: &FoundationInvocation<'_>,
    candidate_profile: bool,
) -> Result<FoundationBootstrapConfig> {
    let budgets = invocation.budgets;
    if invocation.deadline() <= std::time::Instant::now() {
        return Err(Error::Unsupported("foundation invocation deadline expired"));
    }
    if invocation
        .cancellation_flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err(Error::Unsupported("foundation invocation cancelled"));
    }

    let state_cap = finite_usize(budgets.max_state_bytes, "foundation state limit range")?;
    let member_cap = budgets.max_member_bytes.min(u64::MAX - 1);
    let source_cap = budgets.max_total_read_bytes.min(u64::MAX - 1);
    let tmpfs_cap = budgets.tmpfs_quota_bytes.min(u64::MAX - 1);
    if member_cap == 0 || source_cap == 0 || tmpfs_cap == 0 {
        return Err(Error::Unsupported(
            "foundation bootstrap source and tmpfs limits must be positive",
        ));
    }

    // Invocation and self-image reads are already real source-read charges in
    // the ledger. Reserve the two bounded publication-control reads first,
    // then retain the established three-pass member allowance. Live-source
    // capture uses all three passes; candidate capture uses its copy and
    // immutable EOF passes within that same bounded allowance.
    let entry_reads = invocation
        .cost
        .invocation_read_bytes
        .checked_add(invocation.cost.self_image_read_bytes)
        .ok_or(Error::Unsupported(
            "foundation entry read accounting overflow",
        ))?;
    let member_read_budget = source_cap
        .checked_sub(entry_reads)
        .and_then(|remaining| remaining.checked_sub(capture_control_read_upper_bound_bytes().ok()?))
        .ok_or(Error::Unsupported(
            "foundation source budget cannot reserve capture controls",
        ))?
        / CAPTURE_MEMBER_READ_PASSES;
    // The capture helper independently bounds the aggregate authored-object
    // pass and the generated manifest. Its cumulative write cap checks the
    // actual object bytes plus the actual encoded manifest before installation,
    // so do not divide tmpfs between two independent maxima here.
    // `max_member_bytes` is per authored file, not an aggregate or manifest cap.
    let mut capture_member_cap = member_read_budget.min(tmpfs_cap);
    if !candidate_profile {
        capture_member_cap = capture_member_cap.min(MAX_BYTES as u64);
    }
    let capture_member_bytes_per_pass =
        usize::try_from(capture_member_cap.min((usize::MAX - 1) as u64))
            .map_err(|_| Error::Unsupported("foundation capture member limit range"))?;
    if capture_member_bytes_per_pass == 0 {
        return Err(Error::Unsupported(
            "foundation source budget cannot admit a capture member",
        ));
    }
    let object_bytes_per_file = member_cap.min(capture_member_bytes_per_pass as u64);

    // Capture creates at most one source-store revision. Its member count is
    // bounded by the explicit current-member cap, fixed directory/file inode
    // overhead, the generated-manifest parser's real visit ceiling, and the
    // state envelope below. The ordinary source route keeps its creation
    // helper's additional per-call file ceiling.
    let inode_members = budgets
        .tmpfs_inode_limit
        .checked_sub(CAPTURE_TMPFS_FIXED_INODES)
        .ok_or(Error::Unsupported(
            "foundation tmpfs inode budget cannot admit capture structure",
        ))?;
    if inode_members == 0 {
        return Err(Error::Unsupported(
            "foundation tmpfs inode budget cannot admit a capture member",
        ));
    }
    let mut initial_entry_cap = budgets.max_current_members.min(inode_members);
    if !candidate_profile {
        initial_entry_cap = initial_entry_cap.min(MAX_FILES as u64);
    }
    let initial_entry_cap = usize::try_from(initial_entry_cap.min((usize::MAX - 1) as u64))
        .map_err(|_| Error::Unsupported("foundation current-member limit range"))?;
    if initial_entry_cap == 0 {
        return Err(Error::Unsupported(
            "foundation current-member limit cannot admit capture",
        ));
    }

    // Account startup retention and the exact selected-root wrapper upper
    // bound before assigning capture state. This mirrors the retained-state
    // terms charged by FoundationRemainingBudget and bootstrap.
    let worker_path_bytes = invocation.schema_worker.path.as_os_str().as_bytes().len();
    let plan_state = size_of::<FoundationRemainingBudget<'_>>()
        .checked_add(size_of::<FoundationExecutionLimits>())
        .and_then(|bytes| bytes.checked_add(worker_path_bytes))
        .and_then(|bytes| bytes.checked_add(2 * size_of::<usize>()))
        .ok_or(Error::Unsupported(
            "foundation execution-plan state overflow",
        ))?;
    let startup_state = invocation
        .cost
        .retained_input_bytes
        .checked_add(plan_state)
        .ok_or(Error::Unsupported("foundation startup state overflow"))?;
    let root_state = selected_roots_upper_bound()?;
    let selection_minimum = size_of::<FoundationPhysicalSelection>()
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(size_of::<String>() + 32 * size_of::<usize>()))
        .ok_or(Error::Unsupported("foundation selection state overflow"))?;
    let capture_state_budget = state_cap
        .checked_sub(startup_state)
        .and_then(|bytes| bytes.checked_sub(root_state))
        .and_then(|bytes| bytes.checked_sub(selection_minimum))
        .ok_or(Error::Unsupported(
            "foundation state budget cannot reserve startup, roots, and selection",
        ))?;

    let control_state = control_parse_state()?;
    let capture_fixed_state = size_of::<FoundationCapturedCut>()
        .checked_add(size_of::<CorpusReader>())
        .and_then(|bytes| bytes.checked_add(control_state))
        .and_then(|bytes| bytes.checked_add(PRIVATE_TMPFS_SELECT_COST.retained_bytes))
        .ok_or(Error::Unsupported(
            "foundation capture fixed state overflow",
        ))?;
    let per_member_state = capture_member_state_upper_bound()?;
    let per_json_visit_state = 2usize
        .checked_mul(size_of::<JsonValue>() + size_of::<JsonString>())
        .ok_or(Error::Unsupported("foundation JSON state limit overflow"))?;
    // This internal aggregate metadata file is governed by tmpfs, parser, and
    // retained-state limits; the protected per-authored-member cap does not
    // apply to it. Manifest and object maxima remain independent, while the
    // capture helper checks their actual cumulative writes against tmpfs.
    let maximum_manifest_bytes = finite_usize(tmpfs_cap, "foundation manifest limit range")?;
    let manifest_shape = choose_manifest_shape(
        maximum_manifest_bytes,
        initial_entry_cap,
        capture_object_workspace(object_bytes_per_file as usize)?,
        capture_fixed_state,
        per_member_state,
        per_json_visit_state,
        capture_state_budget,
    )?;
    let read_limits = ReadLimits {
        max_manifest_bytes: manifest_shape.manifest_bytes,
        max_manifest_entries: manifest_shape.entries,
        max_selected_object_bytes: object_bytes_per_file,
        json: JsonLimits {
            max_bytes: manifest_shape.manifest_bytes,
            max_depth: JsonLimits::default().max_depth,
            max_visits: manifest_shape.visits,
            max_integer_digits: JsonLimits::default()
                .max_integer_digits
                .min(manifest_shape.manifest_bytes),
        },
    };
    let capture_tmpfs_write_upper_bound_bytes = tmpfs_cap;
    let capture_state_upper_bound_bytes = manifest_shape.state_bytes;
    if capture_state_upper_bound_bytes > capture_state_budget {
        return Err(Error::Unsupported(
            "foundation state budget cannot reserve capture metadata",
        ));
    }

    let cut_limits = CutReadLimits {
        max_revisions: 1,
        max_members: manifest_shape.entries as u64,
        max_total_bytes: capture_member_bytes_per_pass as u64,
        max_member_bytes: object_bytes_per_file,
    };

    let selection_state_cap = state_cap
        .checked_sub(startup_state)
        .and_then(|bytes| bytes.checked_sub(root_state))
        .ok_or(Error::Unsupported(
            "foundation selection state exceeds invocation",
        ))?;
    // A nominal selector ceiling is narrowed by the live shared ledger after
    // capture. Subtracting the hypothetical largest capture here would reject
    // a small actual capture followed by a larger valid selector phase.
    let selector_row_bytes = size_of::<String>()
        .checked_add(32 * size_of::<usize>())
        .ok_or(Error::Unsupported("foundation selector row limit overflow"))?;
    let selection_entries = selection_state_cap / selector_row_bytes;
    let selection_limits = FoundationSelectionLimits {
        max_selector_documents: finite_usize(
            budgets.max_current_members,
            "foundation selector document limit range",
        )?,
        max_output_entries: selection_entries,
        max_private_prefixes: selection_entries,
        max_payload_paths: selection_entries,
        max_member_bytes: member_cap.min(source_cap),
        max_total_read_bytes: source_cap,
        max_state_bytes: selection_state_cap,
        deadline: invocation.deadline(),
    };
    if selection_limits.max_selector_documents == 0
        || selection_limits.max_output_entries == 0
        || selection_limits.max_member_bytes == 0
        || selection_limits.max_total_read_bytes == 0
        || selection_limits.max_state_bytes == 0
    {
        return Err(Error::Unsupported(
            "foundation selection limits cannot admit a source document",
        ));
    }

    let physical_min_path_state = size_of::<String>()
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
        .ok_or(Error::Unsupported(
            "foundation physical path limit overflow",
        ))?;
    let physical_inventory_path_state =
        physical_min_path_state
            .checked_mul(3)
            .ok_or(Error::Unsupported(
                "foundation inventory path limit overflow",
            ))?;
    let physical_max_paths = selection_state_cap / physical_min_path_state;
    let physical_max_observations = selection_state_cap / size_of::<usize>();
    let physical_max_inventory_paths = selection_state_cap / physical_inventory_path_state;
    let physical_file_cap = finite_usize(
        member_cap.min(source_cap),
        "foundation physical file limit range",
    )?;
    let physical_total_cap = finite_usize(source_cap, "foundation physical read limit range")?;
    if physical_max_paths == 0
        || physical_max_observations == 0
        || physical_max_inventory_paths == 0
        || physical_file_cap == 0
        || physical_total_cap == 0
        || selection_state_cap < size_of::<SourcePhysicalFacts>()
    {
        return Err(Error::Unsupported(
            "foundation physical limits cannot admit source observations",
        ));
    }
    let physical_limits = PhysicalSourceLimits {
        max_paths: physical_max_paths,
        max_private_prefixes: physical_max_paths,
        max_inventory_paths: physical_max_inventory_paths,
        max_path_observations: physical_max_observations,
        max_git_path_queries: physical_max_observations,
        max_file_bytes: physical_file_cap,
        max_total_bytes: physical_total_cap,
        max_state_bytes: selection_state_cap,
        git_output_bytes: selection_state_cap.min(65_536),
        git_cleanup_grace: Duration::from_millis(budgets.operation_wall_ms.min(1_000)),
    };

    let payload_row_state = size_of::<String>()
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
        .ok_or(Error::Unsupported("foundation payload row limit overflow"))?;
    let payload_limits = PhysicalPayloadLimits {
        max_files: selection_state_cap / payload_row_state,
        max_observations: selection_state_cap / size_of::<usize>(),
        max_file_bytes: member_cap.min(source_cap),
        max_total_bytes: source_cap,
        max_state_bytes: selection_state_cap,
    };
    if physical_limits.max_paths == 0
        || physical_limits.max_inventory_paths == 0
        || physical_limits.max_path_observations == 0
        || physical_limits.max_git_path_queries == 0
        || physical_limits.git_output_bytes == 0
        || physical_limits.git_cleanup_grace.is_zero()
        || payload_limits.max_files == 0
        || payload_limits.max_observations == 0
        || payload_limits.max_file_bytes == 0
        || payload_limits.max_total_bytes == 0
        || payload_limits.max_state_bytes == 0
    {
        return Err(Error::Unsupported(
            "foundation snapshot limits cannot admit selected physical inputs",
        ));
    }

    Ok(FoundationBootstrapConfig {
        read_limits,
        cut_limits,
        capture_member_bytes_per_pass,
        capture_state_upper_bound_bytes,
        capture_tmpfs_write_upper_bound_bytes,
        selection_limits,
        physical_limits,
        payload_limits,
    })
}

/// Tighten capture's shape against the already charged shared ledger. This
/// does not replenish startup, identity-selection or candidate I/O charges.
pub(crate) fn narrow_capture_config(
    mut config: FoundationBootstrapConfig,
    remaining: super::foundation_execution_limits::FoundationPhaseReservation,
) -> Result<FoundationBootstrapConfig> {
    let state_cap = remaining
        .state_bytes
        .min(config.capture_state_upper_bound_bytes);
    let tmpfs_cap = remaining
        .tmpfs_bytes
        .min(config.capture_tmpfs_write_upper_bound_bytes);
    let fixed = size_of::<FoundationCapturedCut>()
        .checked_add(size_of::<CorpusReader>())
        .and_then(|bytes| bytes.checked_add(control_parse_state().ok()?))
        .and_then(|bytes| bytes.checked_add(PRIVATE_TMPFS_SELECT_COST.retained_bytes))
        .ok_or(Error::Unsupported("capture narrowed fixed state overflow"))?;
    let shape = choose_manifest_shape(
        config
            .read_limits
            .max_manifest_bytes
            .min(finite_usize(tmpfs_cap, "capture narrowed tmpfs range")?),
        config.read_limits.max_manifest_entries,
        capture_object_workspace(
            usize::try_from(config.read_limits.max_selected_object_bytes)
                .map_err(|_| Error::Unsupported("capture narrowed member range"))?,
        )?,
        fixed,
        capture_member_state_upper_bound()?,
        2 * (size_of::<JsonValue>() + size_of::<JsonString>()),
        state_cap,
    )?;
    config.capture_state_upper_bound_bytes = shape.state_bytes;
    config.capture_tmpfs_write_upper_bound_bytes = tmpfs_cap;
    config.read_limits.max_manifest_bytes = shape.manifest_bytes;
    config.read_limits.max_manifest_entries = shape.entries;
    config.read_limits.json.max_bytes = shape.manifest_bytes;
    config.read_limits.json.max_visits = shape.visits;
    config.read_limits.json.max_integer_digits = config
        .read_limits
        .json
        .max_integer_digits
        .min(shape.manifest_bytes);
    config.cut_limits.max_members = config.cut_limits.max_members.min(shape.entries as u64);
    Ok(config)
}

struct ManifestShape {
    manifest_bytes: usize,
    entries: usize,
    visits: usize,
    state_bytes: usize,
}

fn choose_manifest_shape(
    maximum_manifest_bytes: usize,
    maximum_entries: usize,
    member_transient_workspace_bytes: usize,
    fixed_state_bytes: usize,
    per_member_state_bytes: usize,
    per_visit_state_bytes: usize,
    state_budget: usize,
) -> Result<ManifestShape> {
    let default_visits = JsonLimits::default().max_visits;
    let mut low = MIN_CAPTURE_MANIFEST_BYTES;
    let mut high = maximum_manifest_bytes.min(usize::MAX - 1);
    let mut selected = None;
    while low <= high {
        let middle = low + (high - low) / 2;
        if let Some(shape) = manifest_shape(
            middle,
            maximum_entries,
            member_transient_workspace_bytes,
            fixed_state_bytes,
            per_member_state_bytes,
            per_visit_state_bytes,
            default_visits,
        )? && shape.state_bytes <= state_budget
        {
            selected = Some(shape);
            low = middle.saturating_add(1);
        } else {
            high = middle.saturating_sub(1);
        }
    }
    selected.ok_or(Error::Unsupported(
        "foundation invocation cannot reserve a bounded one-member capture",
    ))
}

fn manifest_shape(
    manifest_bytes: usize,
    maximum_entries: usize,
    member_transient_workspace_bytes: usize,
    fixed_state_bytes: usize,
    per_member_state_bytes: usize,
    per_visit_state_bytes: usize,
    default_visits: usize,
) -> Result<Option<ManifestShape>> {
    if manifest_bytes < MIN_CAPTURE_MANIFEST_BYTES {
        return Ok(None);
    }
    let maximum_visits = default_visits.min(manifest_bytes);
    let entries_by_manifest = manifest_bytes
        .checked_sub(MIN_CAPTURE_MANIFEST_ENVELOPE_BYTES)
        .map(|bytes| bytes / MIN_CAPTURE_MEMBER_ROW_BYTES)
        .unwrap_or(0);
    let entries_by_visits = maximum_visits
        .checked_sub(CAPTURE_MANIFEST_FIXED_VISITS)
        .map(|visits| visits / CAPTURE_MANIFEST_VISITS_PER_MEMBER)
        .unwrap_or(0);
    let entries = maximum_entries
        .min(entries_by_manifest)
        .min(entries_by_visits);
    if entries == 0 {
        return Ok(None);
    }
    let visits = CAPTURE_MANIFEST_FIXED_VISITS
        .checked_add(
            entries
                .checked_mul(CAPTURE_MANIFEST_VISITS_PER_MEMBER)
                .ok_or(Error::Unsupported("foundation JSON visit limit overflow"))?,
        )
        .ok_or(Error::Unsupported("foundation JSON visit limit overflow"))?;
    if visits > maximum_visits {
        return Ok(None);
    }
    let string_state = manifest_bytes
        .checked_mul(8)
        .ok_or(Error::Unsupported("foundation manifest state overflow"))?;
    let manifest_workspace = visits
        .checked_mul(per_visit_state_bytes)
        .ok_or(Error::Unsupported("foundation JSON visit state overflow"))?;
    // Object bytes are read one member at a time before manifest encoding and
    // parsing begin. Retained metadata is additive, but these two transient
    // workspaces do not coexist, so reserve their larger peak.
    let transient_workspace = member_transient_workspace_bytes.max(
        string_state
            .checked_add(manifest_workspace)
            .ok_or(Error::Unsupported("foundation manifest state overflow"))?,
    );
    let member_state = entries
        .checked_mul(per_member_state_bytes)
        .ok_or(Error::Unsupported(
            "foundation capture member state overflow",
        ))?;
    let state_bytes = fixed_state_bytes
        .checked_add(member_state)
        .and_then(|bytes| bytes.checked_add(transient_workspace))
        .ok_or(Error::Unsupported("foundation capture state overflow"))?;
    Ok(Some(ManifestShape {
        manifest_bytes,
        entries,
        visits,
        state_bytes,
    }))
}

pub(crate) struct ObservedCaptureState {
    pub retained_upper_bound_bytes: usize,
    pub peak_upper_bound_bytes: usize,
}

/// Publication controls and this capture's separate guard selection/recheck.
/// Guard reads are admitted upper bounds; sequential scratch is modeled by max.
pub(crate) fn capture_control_read_upper_bound_bytes() -> Result<u64> {
    PUBLICATION_CONTROL_READ_CAP_BYTES
        .checked_mul(CAPTURE_CONTROL_READS)
        .and_then(|bytes| bytes.checked_add(PRIVATE_TMPFS_SELECT_COST.read_bytes))
        .and_then(|bytes| bytes.checked_add(PRIVATE_TMPFS_VERIFY_COST.read_bytes))
        .ok_or(Error::Unsupported("capture guard/control read overflow"))
}

fn capture_object_workspace(largest_member: usize) -> Result<usize> {
    largest_member
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(65_536))
        .map(|bytes| bytes.max(PRIVATE_TMPFS_VERIFY_COST.workspace_bytes))
        .ok_or(Error::Unsupported("capture object workspace overflow"))
}

/// Logical structural retention and separate sequential transient peak for
/// the observed capture. Neither is an allocator or physical RSS claim.
pub(crate) fn observed_capture_state_upper_bound(
    captured: &FoundationCapturedCut,
) -> Result<ObservedCaptureState> {
    let mut entries = 0usize;
    let mut largest_member = 0usize;
    for (_, member) in captured.observed_members() {
        entries = entries
            .checked_add(1)
            .ok_or(Error::Unsupported("capture member count overflow"))?;
        largest_member = largest_member.max(
            usize::try_from(member.size_bytes)
                .map_err(|_| Error::Unsupported("capture observed member size range"))?,
        );
    }
    let visits = entries
        .checked_mul(CAPTURE_MANIFEST_VISITS_PER_MEMBER)
        .and_then(|count| count.checked_add(CAPTURE_MANIFEST_FIXED_VISITS))
        .ok_or(Error::Unsupported("capture observed JSON visits overflow"))?;
    let manifest = captured.cost().manifest_write_bytes;
    let manifest_workspace = manifest
        .checked_mul(8)
        .and_then(|bytes| {
            visits
                .checked_mul(2 * (size_of::<JsonValue>() + size_of::<JsonString>()))
                .and_then(|nodes| bytes.checked_add(nodes))
        })
        .ok_or(Error::Unsupported(
            "capture observed manifest state overflow",
        ))?;
    let control_state = control_parse_state()?;
    let fixed = size_of::<FoundationCapturedCut>()
        .checked_add(size_of::<CorpusReader>())
        .and_then(|bytes| bytes.checked_add(control_state))
        .and_then(|bytes| bytes.checked_add(PRIVATE_TMPFS_SELECT_COST.retained_bytes))
        .ok_or(Error::Unsupported("capture observed fixed state overflow"))?;
    let retained_upper_bound_bytes = entries
        .checked_mul(capture_member_state_upper_bound()?)
        .and_then(|bytes| bytes.checked_add(fixed))
        .ok_or(Error::Unsupported(
            "capture observed retained state overflow",
        ))?;
    let peak_upper_bound_bytes = retained_upper_bound_bytes
        .checked_add(capture_object_workspace(largest_member)?.max(manifest_workspace))
        .ok_or(Error::Unsupported("capture observed peak state overflow"))?;
    Ok(ObservedCaptureState {
        retained_upper_bound_bytes,
        peak_upper_bound_bytes,
    })
}

fn capture_member_state_upper_bound() -> Result<usize> {
    let path_bytes = MAX_RELATIVE_PATH_BYTES
        .checked_mul(6)
        .ok_or(Error::Unsupported("foundation capture path state overflow"))?;
    let metadata = size_of::<MemberMetadata>()
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(size_of::<String>() * 2))
        .and_then(|bytes| bytes.checked_add(size_of::<RelativePath>()))
        .and_then(|bytes| bytes.checked_add(64 * size_of::<usize>()))
        .and_then(|bytes| bytes.checked_add(path_bytes))
        .ok_or(Error::Unsupported("foundation capture row state overflow"))?;
    Ok(metadata)
}

fn control_parse_state() -> Result<usize> {
    let visits = JsonLimits::default()
        .max_visits
        .min(PUBLICATION_CONTROL_READ_CAP_BYTES as usize);
    let parser_nodes = visits
        .checked_mul(
            2usize
                .checked_mul(size_of::<JsonValue>() + size_of::<JsonString>())
                .ok_or(Error::Unsupported("foundation control JSON state overflow"))?,
        )
        .ok_or(Error::Unsupported("foundation control JSON state overflow"))?;
    (PUBLICATION_CONTROL_READ_CAP_BYTES as usize)
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(parser_nodes))
        .ok_or(Error::Unsupported("foundation control state overflow"))
}

fn selected_roots_upper_bound() -> Result<usize> {
    let root_count = 3usize;
    let selected_path_bytes =
        root_count
            .checked_mul(MAX_RELATIVE_PATH_BYTES)
            .ok_or(Error::Unsupported(
                "foundation selected-root path state overflow",
            ))?;
    let default_payload_path_bytes = MAX_RELATIVE_PATH_BYTES
        .checked_add(DEFAULT_PAYLOAD_SUFFIX.len())
        .ok_or(Error::Unsupported(
            "foundation payload-root path state overflow",
        ))?;
    let route_wrappers =
        size_of::<RouteSources>()
            .checked_mul(root_count)
            .ok_or(Error::Unsupported(
                "foundation selected-root route state overflow",
            ))?;
    let shared_operation_arc =
        size_of::<AtomicUsize>()
            .checked_mul(3)
            .ok_or(Error::Unsupported(
                "foundation selected-root route state overflow",
            ))?;
    let custody_arc_headers = size_of::<AtomicUsize>()
        .checked_mul(root_count)
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(Error::Unsupported(
            "foundation selected-root custody state overflow",
        ))?;
    size_of::<FoundationSelectedRoots>()
        .checked_add(route_wrappers)
        .and_then(|bytes| bytes.checked_add(selected_path_bytes.checked_mul(2)?))
        .and_then(|bytes| bytes.checked_add(default_payload_path_bytes))
        .and_then(|bytes| bytes.checked_add(size_of::<std::path::PathBuf>()))
        .and_then(|bytes| bytes.checked_add(shared_operation_arc))
        .and_then(|bytes| bytes.checked_add(custody_arc_headers))
        .ok_or(Error::Unsupported(
            "foundation selected-root state overflow",
        ))
}

fn finite_usize(value: u64, label: &'static str) -> Result<usize> {
    let finite = value.min((usize::MAX - 1) as u64);
    usize::try_from(finite).map_err(|_| Error::Unsupported(label))
}
