//! Build the first native V2 admission batch from one complete held source cut.
//!
//! This is a bounded proposal producer only. The normal candidate, Native
//! validator and initial V2 publisher remain the only admission path.

use super::source_admission::{
    AdmissionBatch, AdmissionLimits, AdmissionWorkBudget, active, invalid,
};
use super::source_admission_segment_v2::NativeV2TreeIo;
use super::source_admission_source_census::{
    SourceCensusLimits, SourceCensusScan, SourceCensusSummary, SourceCensusWorkKind,
    census_selected_to_scratch, working_state_upper_bound,
};
use super::source_admission_store::AdmissionStore;
use super::source_admission_v2_seen_pack::configure_db;
use super::source_foundation_admission::NativeSourceValidator;
use rusqlite::params;
use rustix::fs::{AtFlags, FileType, RawDir};
use std::{
    cell::RefCell,
    fs::File,
    io,
    mem::{MaybeUninit, size_of},
    os::unix::fs::MetadataExt,
    path::{Component, Path},
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::{
    PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteConnection, PinnedSqliteIoBudget,
    ReadLimits,
};

const ROOT_PATH_BYTES: usize = 4096;
const ROOT_PATH_COMPONENTS: usize = 128;
const ROOT_METADATA_GUARD_BYTES: u64 = 4096;
const OBJECT_VERIFY_BLOCK_BYTES: usize = 65_536;
const INDEXED_MANIFEST_PARSE_STATE_BYTES: usize = 128 * 1024;
// The census helper performs two CREATE TABLE statements, a SAVEPOINT, and a
// RELEASE on success. Reserving six control units also covers a failed RELEASE
// followed by its bounded ROLLBACK TO + RELEASE cleanup path.
const CENSUS_CONTROL_SQL_WORK_UNITS: u64 = 6;

/// Finite slices derived from the invocation's already-selected source
/// limits. This profile cannot raise any selected file, member, byte, state,
/// cache or work ceiling.
#[derive(Clone, Copy)]
pub(crate) struct InitialCutProfile {
    pub(crate) census: SourceCensusLimits,
    pub(crate) admission: AdmissionLimits,
    pub(crate) max_state_slice_bytes: usize,
    pub(crate) census_state_bytes: usize,
    pub(crate) update_rows_state_bytes: usize,
    pub(crate) batch_builder_state_bytes: usize,
    pub(crate) store_namespace_state_bytes: usize,
    pub(crate) retained_fence_state_bytes: usize,
    pub(crate) sqlite_cache_bytes: usize,
    pub(crate) sqlite_native_overhead_bytes: usize,
    pub(crate) max_work_units: u64,
}

impl InitialCutProfile {
    /// Derive the simultaneous initial-cut envelope from the caller's
    /// original selections. SQLite native overhead is an admitted finite
    /// allowance from that same state budget, not an empirical RSS claim.
    pub(crate) fn from_original_limits(
        original_state_slice_bytes: usize,
        mut census: SourceCensusLimits,
        admission: AdmissionLimits,
        sqlite_cache_bytes: usize,
        sqlite_native_overhead_bytes: usize,
        indexed_input: bool,
        // A finite slice selected by the caller from the original invocation
        // budget. The locally derived operation bound must fit inside it.
        selected_work_units: u64,
    ) -> io::Result<Self> {
        let admission = admission.validate()?;
        if original_state_slice_bytes == 0
            || original_state_slice_bytes == usize::MAX
            || sqlite_cache_bytes == 0
            || sqlite_cache_bytes == usize::MAX
            || sqlite_native_overhead_bytes == 0
            || sqlite_native_overhead_bytes == usize::MAX
            || selected_work_units == 0
            || selected_work_units == u64::MAX
            || census.max_files == 0
            || census.max_files == u64::MAX
            || u64::try_from(admission.max_members)
                .ok()
                .is_none_or(|members| census.max_files > members)
            || census.max_directories == 0
            || census.max_directories == u64::MAX
            || census.max_entries == 0
            || census.max_entries == u64::MAX
            || census.max_depth == usize::MAX
            || census.max_member_bytes == 0
            || census.max_member_bytes == u64::MAX
            || census.max_source_bytes == 0
            || census.max_source_bytes == u64::MAX
            || census.max_files > i64::MAX as u64
            || census.max_directories > i64::MAX as u64
            || census.max_entries > i64::MAX as u64
            || census.max_member_bytes > i64::MAX as u64
            || census.max_source_bytes > i64::MAX as u64
            || i64::try_from(census.max_depth).is_err()
            || census.max_member_bytes > census.max_source_bytes
            || census.max_member_bytes > admission.max_member_bytes
            || census.max_source_bytes > admission.max_source_bytes
            || census.max_path_bytes > admission.max_batch_bytes
        {
            return Err(invalid(
                "initial source cut limits exceed the original profile",
            ));
        }
        let census_state_bytes = working_state_upper_bound(census.max_path_bytes)?;
        census.state_slice_bytes = census_state_bytes;
        let update_rows_state_bytes = update_rows_state_upper_bound(census)?;
        let batch_builder_state_bytes = batch_builder_state_upper_bound(census)?;
        let store_namespace_state_bytes = store_namespace_state_upper_bound()?;
        let retained_fence_state_bytes = retained_fence_state_upper_bound(indexed_input)?;
        let max_work_units = maximum_work_units(census, indexed_input)?;
        if max_work_units > selected_work_units {
            return Err(invalid(
                "initial source cut work bound exceeds the caller-selected ceiling",
            ));
        }
        let profile = Self {
            census,
            admission,
            max_state_slice_bytes: original_state_slice_bytes,
            census_state_bytes,
            update_rows_state_bytes,
            batch_builder_state_bytes,
            store_namespace_state_bytes,
            retained_fence_state_bytes,
            sqlite_cache_bytes,
            sqlite_native_overhead_bytes,
            max_work_units,
        };
        check_state_partition(profile)?;
        Ok(profile)
    }
}

pub(crate) struct InitialCutPrepared<'a> {
    batch: Option<AdmissionBatch>,
    fence: InitialCutFence<'a>,
}

impl InitialCutPrepared<'_> {
    pub(crate) fn take_batch(&mut self) -> io::Result<AdmissionBatch> {
        self.batch
            .take()
            .ok_or_else(|| invalid("initial source cut batch already consumed"))
    }

    /// The entire selected bridge slice remains charged on the original
    /// Native state ledger through candidate validation and the terminal
    /// census. The V2 reader and candidate retain their separately selected
    /// slices from that same invocation.
    pub(crate) fn retained_state_upper_bound_bytes(&self) -> usize {
        self.fence.profile.max_state_slice_bytes
    }

    pub(crate) fn work_units_used(&self) -> u64 {
        self.fence.work.used()
    }

    pub(crate) fn proposal_census(&self) -> SourceCensusSummary {
        self.fence.proposal
    }

    /// After exact accepted-publication lookup misses, prove the target is
    /// still a genuinely empty initial store before candidate ingestion writes
    /// any objects. Accepted retries skip this check and recover the retained
    /// publication instead of attempting another initial CAS.
    pub(crate) fn verify_initial_miss_baseline(
        &mut self,
        invocation: &NativeSourceValidator<'_>,
        store: &AdmissionStore,
        limits: ReadLimits,
        accountant: &Arc<NativeV2TreeIo>,
    ) -> io::Result<()> {
        if self.batch.is_some() {
            return Err(invalid(
                "initial miss baseline follows exact accepted-publication lookup",
            ));
        }
        self.fence
            .verify_initial_miss_baseline(invocation, store, limits, accountant)
    }

    /// Recheck the complete selected filesystem cut and exact store object
    /// closure after Native validation. The initial publisher must call this
    /// as its in-lock pre-publication fence before it builds or installs V2
    /// state.
    pub(crate) fn verify_before_publish(
        &mut self,
        invocation: &NativeSourceValidator<'_>,
        store: &AdmissionStore,
        limits: ReadLimits,
        accountant: &Arc<NativeV2TreeIo>,
    ) -> io::Result<()> {
        if self.batch.is_some() {
            return Err(invalid(
                "initial source cut must enter the candidate before its terminal fence",
            ));
        }
        self.fence
            .verify_before_publish(invocation, store, limits, accountant)
    }
}

struct InitialCutFence<'a> {
    root_name: &'a Path,
    root: File,
    root_identity: (u64, u64),
    root_stamp: (u64, u64, u64, i64, i64, i64, i64),
    proposal: SourceCensusSummary,
    profile: InitialCutProfile,
    _scope: PinnedSqliteAuxScope,
    db: Rc<RefCell<PinnedSqliteConnection>>,
    spool_io: PinnedSqliteIoBudget,
    v2_io: PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    work: InitialCutWork,
    miss_baseline_verified: bool,
}

// This is a local meter for the initial-cut producer, bounded by a work slice
// selected by its caller. Native currently exposes no shared mutable work
// ledger, so this counter does not claim to debit global invocation work.
type InitialCutWork = AdmissionWorkBudget;

fn update_rows_state_upper_bound(census: SourceCensusLimits) -> io::Result<usize> {
    census
        .max_path_bytes
        .checked_mul(24)
        .and_then(|bytes| bytes.checked_add(16_384))
        .ok_or_else(|| invalid("initial source update-row state overflow"))
}

fn batch_builder_state_upper_bound(census: SourceCensusLimits) -> io::Result<usize> {
    census
        .max_path_bytes
        .checked_mul(24)
        .and_then(|bytes| bytes.checked_add(32_768))
        .ok_or_else(|| invalid("initial canonical batch state overflow"))
}

fn retained_fence_state_upper_bound(indexed_input: bool) -> io::Result<usize> {
    size_of::<InitialCutPrepared<'static>>()
        .checked_add(AdmissionWorkBudget::retained_allocation_upper_bound_bytes())
        .and_then(|bytes| bytes.checked_add(4 * size_of::<usize>()))
        .and_then(|bytes| bytes.checked_add(16_384))
        .and_then(|bytes| {
            bytes.checked_add(if indexed_input {
                INDEXED_MANIFEST_PARSE_STATE_BYTES
            } else {
                0
            })
        })
        .ok_or_else(|| invalid("initial source cut retained fence state overflow"))
}

fn store_namespace_state_upper_bound() -> io::Result<usize> {
    // One getdents buffer is reused across fixed store namespaces. The SQL
    // comparison keeps at most two statements/rows and two 64-byte names live.
    8192usize
        .checked_add(OBJECT_VERIFY_BLOCK_BYTES)
        .and_then(|bytes| bytes.checked_add(8192))
        .and_then(|bytes| bytes.checked_add(size_of::<File>() * 2))
        .and_then(|bytes| {
            size_of::<rusqlite::Statement<'static>>()
                .checked_mul(2)
                .and_then(|workspace| bytes.checked_add(workspace))
        })
        .and_then(|bytes| {
            size_of::<rusqlite::Rows<'static>>()
                .checked_mul(2)
                .and_then(|workspace| bytes.checked_add(workspace))
        })
        .and_then(|bytes| bytes.checked_add(2048))
        .ok_or_else(|| invalid("initial store namespace state bound overflow"))
}

fn check_state_partition(profile: InitialCutProfile) -> io::Result<()> {
    let required = profile
        .census_state_bytes
        .checked_add(profile.update_rows_state_bytes)
        .and_then(|bytes| bytes.checked_add(profile.batch_builder_state_bytes))
        .and_then(|bytes| bytes.checked_add(profile.store_namespace_state_bytes))
        .and_then(|bytes| bytes.checked_add(profile.retained_fence_state_bytes))
        .and_then(|bytes| bytes.checked_add(profile.sqlite_cache_bytes))
        .and_then(|bytes| bytes.checked_add(profile.sqlite_native_overhead_bytes))
        .ok_or_else(|| invalid("initial source cut state partition overflow"))?;
    if required > profile.max_state_slice_bytes {
        return Err(invalid(
            "initial source cut exceeds the original state slice",
        ));
    }
    Ok(())
}

fn maximum_work_units(census: SourceCensusLimits, indexed_input: bool) -> io::Result<u64> {
    // The census owner charges at most entries + 4*directories + 4*members +
    // 4 row-probe/aggregate/EOF units per pass. The caller separately
    // precharges six control units for its two schema DDLs, savepoint, and
    // success/refusal cleanup. The canonical feed plus the candidate's
    // membership and ingestion cursors share this same local work meter. The
    // object GROUP BY also scans every proposal row, charged before query.
    // Root-name and selector fences have a separate bounded allowance for
    // both preparation and terminal verification.
    let per_scan = census
        .max_entries
        .checked_add(
            census
                .max_directories
                .checked_mul(4)
                .ok_or_else(|| invalid("initial source cut directory work bound overflow"))?,
        )
        .and_then(|units| units.checked_add(census.max_files.checked_mul(4)?))
        .and_then(|units| units.checked_add(4 + CENSUS_CONTROL_SQL_WORK_UNITS))
        .ok_or_else(|| invalid("initial source cut scan work bound overflow"))?;
    let object_verify_reads = census
        .max_source_bytes
        .checked_div(OBJECT_VERIFY_BLOCK_BYTES as u64)
        .and_then(|blocks| {
            blocks.checked_add(u64::from(
                census.max_source_bytes % OBJECT_VERIFY_BLOCK_BYTES as u64 != 0,
            ))
        })
        .and_then(|blocks| blocks.checked_add(census.max_files.checked_mul(2)?))
        .ok_or_else(|| invalid("initial source object-read work bound overflow"))?;
    let indexed_pack_scan_work = if indexed_input {
        // Each unique payload appears in at most one pack and total unique
        // payload bytes cannot exceed the source cut. A pack adds a 48-byte
        // segment header, a 40-byte frame header per object and an 8-byte
        // trailer. In the worst case there is one pack per source member.
        // The verifier reads frame payloads in 64 KiB chunks, plus one read
        // for each frame header and each segment header/trailer. The extra
        // per-member terms cover per-frame chunk rounding and verifier calls
        // without treating manifest object counts as a grant. This prices the
        // normal regular-file read shape; every actual short read still
        // debits the same shared meter and can refuse safely if fragmented.
        let framing_bytes = census
            .max_files
            .checked_mul(96)
            .ok_or_else(|| invalid("initial indexed pack framing bound overflow"))?;
        let scan_bytes = census
            .max_source_bytes
            .checked_add(framing_bytes)
            .ok_or_else(|| invalid("initial indexed pack scan byte bound overflow"))?;
        let blocks = scan_bytes
            .checked_div(OBJECT_VERIFY_BLOCK_BYTES as u64)
            .and_then(|blocks| {
                blocks.checked_add(u64::from(
                    scan_bytes % OBJECT_VERIFY_BLOCK_BYTES as u64 != 0,
                ))
            })
            .ok_or_else(|| invalid("initial indexed pack scan block bound overflow"))?;
        blocks
            .checked_add(
                census
                    .max_files
                    .checked_mul(6)
                    .ok_or_else(|| invalid("initial indexed pack scan operation bound overflow"))?,
            )
            .ok_or_else(|| invalid("initial indexed pack scan work bound overflow"))?
    } else {
        0
    };
    let file_reopen_work = if indexed_input {
        0
    } else {
        u64::try_from(census.max_depth)
            .map_err(|_| invalid("initial source reopen depth range differs"))?
            .checked_add(2)
            .and_then(|components| census.max_files.checked_mul(components))
            .ok_or_else(|| invalid("initial source reopen work bound overflow"))?
    };
    let selector_fence_work = if indexed_input {
        let rounded_blocks = |bytes: usize| -> io::Result<u64> {
            let bytes = u64::try_from(bytes)
                .map_err(|_| invalid("indexed-input evidence size range differs"))?;
            bytes
                .checked_div(OBJECT_VERIFY_BLOCK_BYTES as u64)
                .and_then(|blocks| {
                    blocks.checked_add(u64::from(bytes % OBJECT_VERIFY_BLOCK_BYTES as u64 != 0))
                })
                .ok_or_else(|| invalid("indexed-input evidence block bound overflow"))
        };
        let descriptor_blocks =
            rounded_blocks(crate::source_admission_indexed_input::DESCRIPTOR_MAX_BYTES)?
                .checked_mul(3)
                .ok_or_else(|| invalid("indexed-input descriptor work bound overflow"))?;
        let one_pass =
            rounded_blocks(crate::source_admission_indexed_input::PROFILE_SIDECAR_MAX_BYTES_V1)?
                .checked_add(rounded_blocks(
                    crate::source_admission_indexed_input::DEPENDENCY_CLOSURE_MAX_BYTES_V1,
                )?)
                .and_then(|blocks| blocks.checked_add(descriptor_blocks))
                .ok_or_else(|| invalid("indexed-input evidence work bound overflow"))?;
        (ROOT_PATH_COMPONENTS as u64 * 3)
            .checked_add(160)
            .and_then(|units| {
                one_pass
                    .checked_mul(3)
                    .and_then(|passes| units.checked_add(passes))
            })
            .ok_or_else(|| invalid("indexed-input selector fence work bound overflow"))?
    } else {
        (ROOT_PATH_COMPONENTS * 2 + 160) as u64
    };
    per_scan
        .checked_mul(2)
        // Canonical row streaming replaces the prior map import/builder work;
        // candidate membership and ingestion each revisit every update row
        // through a charged one-row keyset cursor.
        .and_then(|units| units.checked_add(census.max_files.checked_mul(13)?))
        // The explicit packed-input selector compares producer deduplication
        // evidence with the exact source census by a bounded digest GROUP BY.
        // Reserve one input-row scan plus at most one digest output row per
        // input member before that query is issued.
        .and_then(|units| units.checked_add(census.max_files.checked_mul(2)?))
        // The file fallback reopens each selected path from the held
        // repository descriptor. Indexed mode reads the same selected
        // members from its authenticated tree and pack, so it reserves no
        // redundant filesystem reopen traversal here.
        .and_then(|units| units.checked_add(file_reopen_work))
        // Input rows consumed by the expected-object GROUP BY are charged
        // before SQLite starts that aggregate; output rows are charged below.
        .and_then(|units| units.checked_add(census.max_files))
        // The indexed-input reader records each authenticated unique extent
        // in the owned AUX ledger, then streams that bounded table once for
        // whole-pack closure. Two units per input row cover the extent-row
        // write/check, and one covers the final closure cursor row.
        .and_then(|units| units.checked_add(census.max_files.checked_mul(3)?))
        .and_then(|units| units.checked_add(2))
        .and_then(|units| units.checked_add(object_verify_reads))
        .and_then(|units| units.checked_add(indexed_pack_scan_work))
        // Includes bounded query setup and root/selector fences, plus the
        // repeated maximum-leaf hash passes used to open and recheck indexed
        // evidence files.
        .and_then(|units| units.checked_add(selector_fence_work))
        .ok_or_else(|| invalid("initial source cut work bound overflow"))
}

/// Charge the maintained source-admission upper bound for one descriptor-
/// relative name-resolution/metadata window.
pub(crate) fn charge_name_guard(io: &PinnedSqliteIoBudget, name: &str) -> io::Result<()> {
    let bytes = u64::try_from(name.len())
        .map_err(|_| invalid("initial store namespace name exceeds range"))?
        .checked_add(1 + ROOT_METADATA_GUARD_BYTES)
        .ok_or_else(|| invalid("initial store namespace guard overflow"))?;
    charge_read_upper(io, bytes)
}

fn verify_store_root_entries(
    root: &File,
    io: &PinnedSqliteIoBudget,
    work: &mut InitialCutWork,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let mut seen = [false; 3];
    let mut lock_seen = false;
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = RawDir::new(root, &mut buffer);
    loop {
        active(deadline, cancel)?;
        if entries.is_buffer_empty() {
            charge_read_upper(io, 8192)?;
        }
        work.charge_many(1)?;
        let entry = match entries.next() {
            None => break,
            Some(Err(error)) => return Err(error.into()),
            Some(Ok(entry)) => entry,
        };
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| invalid("initial store root contains a non-UTF8 name"))?;
        if name == "." || name == ".." {
            continue;
        }
        let expected_kind = match name {
            "objects" => {
                if std::mem::replace(&mut seen[0], true) {
                    return Err(invalid("initial store objects namespace is duplicated"));
                }
                0
            }
            "revisions" => {
                if std::mem::replace(&mut seen[1], true) {
                    return Err(invalid("initial store revisions namespace is duplicated"));
                }
                0
            }
            "staging" => {
                if std::mem::replace(&mut seen[2], true) {
                    return Err(invalid("initial store staging namespace is duplicated"));
                }
                0
            }
            ".admission.lock" => {
                if std::mem::replace(&mut lock_seen, true) {
                    return Err(invalid("initial store lock entry is duplicated"));
                }
                1
            }
            _ => return Err(invalid("initial store root contains an unexpected entry")),
        };
        charge_name_guard(io, name)?;
        let stat = rustix::fs::statat(root, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let kind = FileType::from_raw_mode(stat.st_mode);
        if (expected_kind == 0 && !kind.is_dir()) || (expected_kind == 1 && !kind.is_file()) {
            return Err(invalid("initial store root entry has the wrong type"));
        }
    }
    if seen != [true; 3] {
        return Err(invalid("initial store fixed namespaces are incomplete"));
    }
    Ok(())
}

fn require_empty_directory(
    directory: &File,
    io: &PinnedSqliteIoBudget,
    work: &mut InitialCutWork,
    deadline: Instant,
    cancel: &AtomicBool,
    reason: &'static str,
) -> io::Result<()> {
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = RawDir::new(directory, &mut buffer);
    loop {
        active(deadline, cancel)?;
        if entries.is_buffer_empty() {
            charge_read_upper(io, 8192)?;
        }
        work.charge_many(1)?;
        match entries.next() {
            None => return Ok(()),
            Some(Err(error)) => return Err(error.into()),
            Some(Ok(entry)) => {
                let name = entry
                    .file_name()
                    .to_str()
                    .map_err(|_| invalid("initial store namespace contains a non-UTF8 name"))?;
                if name != "." && name != ".." {
                    return Err(invalid(reason));
                }
            }
        }
    }
}

fn initial_store_namespaces(
    store: &AdmissionStore,
    io: &PinnedSqliteIoBudget,
    work: &mut InitialCutWork,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<(File, File, File)> {
    work.charge_many(16)?;
    if store.has_v2_segments()? {
        return Err(invalid("initial V2 store already has a segment namespace"));
    }
    let (root, objects, revisions) = store.backup_namespaces()?;
    verify_store_root_entries(root, io, work, deadline, cancel)?;
    charge_name_guard(io, "staging")?;
    let staging = tos_fd_open::open_directory_at(root, Path::new("staging")).map_err(invalid)?;
    Ok((objects.try_clone()?, revisions.try_clone()?, staging))
}

fn verify_initial_store_baseline(
    store: &AdmissionStore,
    io: &PinnedSqliteIoBudget,
    work: &mut InitialCutWork,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let (objects, revisions, staging) =
        initial_store_namespaces(store, io, work, deadline, cancel)?;
    require_empty_directory(
        &objects,
        io,
        work,
        deadline,
        cancel,
        "initial V2 object namespace contains pre-existing objects",
    )?;
    require_empty_directory(
        &revisions,
        io,
        work,
        deadline,
        cancel,
        "initial V2 revision namespace contains retained state",
    )?;
    require_empty_directory(
        &staging,
        io,
        work,
        deadline,
        cancel,
        "initial V2 staging namespace contains retained state",
    )?;
    Ok(())
}

fn create_object_inventory_table(
    db: &PinnedSqliteConnection,
    work: &mut InitialCutWork,
) -> io::Result<()> {
    work.charge_many(1)?;
    db.execute_batch(
        "CREATE TABLE initial_cut_actual_objects(\
            name TEXT NOT NULL PRIMARY KEY COLLATE BINARY,\
            size INTEGER NOT NULL CHECK(size >= 0)\
         ) WITHOUT ROWID;",
    )
    .map_err(|_| invalid("initial V2 object inventory scratch table refused"))
}

fn verify_indexed_unique_totals(
    db: &PinnedSqliteConnection,
    scan: SourceCensusScan,
    proposal: SourceCensusSummary,
    expected_objects: u64,
    expected_unique_bytes: u64,
    work: &mut InitialCutWork,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    active(deadline, cancel)?;
    let bounded_rows = proposal
        .member_count
        .checked_mul(2)
        .and_then(|units| units.checked_add(1))
        .ok_or_else(|| invalid("indexed input deduplication work overflow"))?;
    work.charge_many(bounded_rows)?;
    let (objects, unique_bytes, conflicting_sizes): (i64, i64, i64) = db
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(min_size),0), \
             COALESCE(SUM(CASE WHEN min_size != max_size THEN 1 ELSE 0 END),0) \
             FROM (SELECT MIN(size) AS min_size, MAX(size) AS max_size \
                   FROM source_member_census WHERE scan_label=?1 GROUP BY sha256)",
            params![scan.label()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| invalid("indexed input census deduplication query refused"))?;
    active(deadline, cancel)?;
    if conflicting_sizes != 0
        || u64::try_from(objects).ok() != Some(expected_objects)
        || u64::try_from(unique_bytes).ok() != Some(expected_unique_bytes)
        || expected_objects == 0
        || expected_unique_bytes > proposal.source_bytes
    {
        return Err(invalid(
            "indexed input deduplication totals differ from the exact census",
        ));
    }
    work.charge_many(2)?;
    db.execute_batch(
        "CREATE TABLE source_indexed_pack_extent_v1(\
            digest BLOB NOT NULL PRIMARY KEY CHECK(length(digest)=32),\
            size INTEGER NOT NULL CHECK(size >= 0),\
            segment_digest BLOB NOT NULL CHECK(length(segment_digest)=32),\
            segment_size INTEGER NOT NULL CHECK(segment_size > 0),\
            frame_index INTEGER NOT NULL CHECK(frame_index >= 0),\
            frame_count INTEGER NOT NULL CHECK(frame_count > 0),\
            header_offset INTEGER NOT NULL CHECK(header_offset >= 0),\
            UNIQUE(segment_digest, frame_index)\
         ) WITHOUT ROWID;",
    )
    .map_err(|_| invalid("indexed input extent closure scratch table refused"))?;
    Ok(())
}

fn verify_object_inventory(
    store: &AdmissionStore,
    db: &PinnedSqliteConnection,
    objects: &File,
    namespace_io: &PinnedSqliteIoBudget,
    payload_io: &PinnedSqliteIoBudget,
    profile: InitialCutProfile,
    proposal: SourceCensusSummary,
    work: &mut InitialCutWork,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = RawDir::new(objects, &mut buffer);
    let mut actual_count = 0u64;
    loop {
        active(deadline, cancel)?;
        if entries.is_buffer_empty() {
            charge_read_upper(namespace_io, 8192)?;
        }
        work.charge_many(1)?;
        let entry = match entries.next() {
            None => break,
            Some(Err(error)) => return Err(error.into()),
            Some(Ok(entry)) => entry,
        };
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| invalid("initial V2 object name is not UTF-8"))?;
        if name == "." || name == ".." {
            continue;
        }
        let digest = Digest256::from_hex(name).map_err(invalid)?;
        if digest.to_hex() != name {
            return Err(invalid("initial V2 object name is not canonical"));
        }
        charge_name_guard(namespace_io, name)?;
        let stat = rustix::fs::statat(objects, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let kind = FileType::from_raw_mode(stat.st_mode);
        if !kind.is_file()
            || stat.st_uid != rustix::process::geteuid().as_raw()
            || stat.st_mode & 0o777 != 0o444
        {
            return Err(invalid("initial V2 object has unexpected custody"));
        }
        let size = u64::try_from(stat.st_size)
            .map_err(|_| invalid("initial V2 object size is negative"))?;
        if size > profile.census.max_member_bytes
            || size > profile.admission.max_member_bytes
            || actual_count >= profile.census.max_files
        {
            return Err(invalid("initial V2 object inventory exceeds its profile"));
        }
        actual_count += 1;
        work.charge_many(object_verify_read_work_units(size)?)?;
        let charge_read = |bytes| payload_io.charge_read(bytes).map_err(invalid);
        let record_read = |bytes| payload_io.record_read_returned(bytes).map_err(invalid);
        let no_write = |_bytes: u64| Ok(());
        store.verify_object_accounted(
            digest,
            size,
            deadline,
            cancel,
            &charge_read,
            &no_write,
            &record_read,
            &no_write,
        )?;
        work.charge_many(1)?;
        let inserted = db
            .execute(
                "INSERT INTO initial_cut_actual_objects(name,size) VALUES(?1,?2)",
                params![
                    name,
                    i64::try_from(size).map_err(|_| {
                        invalid("initial V2 object size exceeds SQLite integer range")
                    })?
                ],
            )
            .map_err(|_| invalid("initial V2 object inventory row refused"))?;
        if inserted != 1 {
            return Err(invalid("initial V2 object inventory row was not unique"));
        }
    }

    // Reserve the complete aggregate input scan before SQLite starts the
    // GROUP BY. Output rows and the paired EOF comparison are charged below.
    work.charge_many(4)?;
    work.charge_many(proposal.member_count)?;
    let mut expected = db
        .prepare(
            "SELECT lower(hex(sha256)),MIN(size),MAX(size) \
             FROM source_member_census WHERE scan_label=?1 \
             GROUP BY sha256 ORDER BY sha256",
        )
        .map_err(|_| invalid("initial V2 expected-object cursor refused"))?;
    let mut expected_rows = expected
        .query(params![SourceCensusScan::Proposal.label()])
        .map_err(|_| invalid("initial V2 expected-object rows refused"))?;
    let mut actual = db
        .prepare("SELECT name,size FROM initial_cut_actual_objects ORDER BY name COLLATE BINARY")
        .map_err(|_| invalid("initial V2 actual-object cursor refused"))?;
    let mut actual_rows = actual
        .query([])
        .map_err(|_| invalid("initial V2 actual-object rows refused"))?;
    let mut compared = 0u64;
    loop {
        active(deadline, cancel)?;
        work.charge_many(2)?;
        let expected = expected_rows
            .next()
            .map_err(|_| invalid("initial V2 expected-object row read refused"))?;
        let actual = actual_rows
            .next()
            .map_err(|_| invalid("initial V2 actual-object row read refused"))?;
        match (expected, actual) {
            (None, None) => break,
            (Some(expected), Some(actual)) => {
                compared = compared
                    .checked_add(1)
                    .filter(|count| *count <= profile.census.max_files)
                    .ok_or_else(|| invalid("initial V2 object comparison count overflow"))?;
                let expected_name: String = expected
                    .get(0)
                    .map_err(|_| invalid("initial V2 expected-object name decode refused"))?;
                let min_size: i64 = expected
                    .get(1)
                    .map_err(|_| invalid("initial V2 expected-object size decode refused"))?;
                let max_size: i64 = expected
                    .get(2)
                    .map_err(|_| invalid("initial V2 expected-object size decode refused"))?;
                let actual_name: String = actual
                    .get(0)
                    .map_err(|_| invalid("initial V2 actual-object name decode refused"))?;
                let actual_size: i64 = actual
                    .get(1)
                    .map_err(|_| invalid("initial V2 actual-object size decode refused"))?;
                if min_size != max_size || expected_name != actual_name || min_size != actual_size {
                    return Err(invalid(
                        "initial V2 object namespace differs from the proposed source cut",
                    ));
                }
            }
            _ => {
                return Err(invalid(
                    "initial V2 object namespace is incomplete or has an orphan",
                ));
            }
        }
    }
    if compared == 0 || compared != actual_count {
        return Err(invalid(
            "initial V2 object namespace does not match the nonempty source cut",
        ));
    }
    Ok(())
}

fn object_verify_read_work_units(size: u64) -> io::Result<u64> {
    let block_bytes = OBJECT_VERIFY_BLOCK_BYTES as u64;
    let full_blocks = size / block_bytes;
    let rounded_block = full_blocks
        .checked_add(u64::from(size % block_bytes != 0))
        .ok_or_else(|| invalid("initial V2 object read work overflow"))?;
    rounded_block
        .checked_add(2)
        .ok_or_else(|| invalid("initial V2 object read work overflow"))
}

fn verify_terminal_store_namespaces(
    store: &AdmissionStore,
    db: &PinnedSqliteConnection,
    namespace_io: &PinnedSqliteIoBudget,
    payload_io: &PinnedSqliteIoBudget,
    profile: InitialCutProfile,
    proposal: SourceCensusSummary,
    work: &mut InitialCutWork,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let (objects, revisions, staging) =
        initial_store_namespaces(store, namespace_io, work, deadline, cancel)?;
    require_empty_directory(
        &revisions,
        namespace_io,
        work,
        deadline,
        cancel,
        "initial V2 revision namespace gained retained state",
    )?;
    require_empty_directory(
        &staging,
        namespace_io,
        work,
        deadline,
        cancel,
        "initial V2 staging namespace retained temporary state",
    )?;
    verify_object_inventory(
        store,
        db,
        &objects,
        namespace_io,
        payload_io,
        profile,
        proposal,
        work,
        deadline,
        cancel,
    )
}

fn charge_read_upper(io: &PinnedSqliteIoBudget, bytes: u64) -> io::Result<()> {
    io.charge_read_upper_bound(bytes).map_err(invalid)
}

fn narrow_current_limits(
    mut limits: ReadLimits,
    accountant: &NativeV2TreeIo,
) -> io::Result<ReadLimits> {
    limits.max_manifest_bytes = limits.max_manifest_bytes.min(4 * 1024);
    limits.max_manifest_entries = limits.max_manifest_entries.min(64);
    limits.json.max_bytes = limits.json.max_bytes.min(4 * 1024);
    limits.json.max_depth = limits.json.max_depth.min(8);
    limits.json.max_visits = limits.json.max_visits.min(64);
    limits.json.max_integer_digits = limits.json.max_integer_digits.min(64);
    limits = limits.validate().map_err(invalid)?;
    let state_upper = limits
        .max_manifest_bytes
        .checked_mul(64)
        .and_then(|bytes| {
            bytes.checked_add(size_of::<Option<tos_source_store::CorpusCurrentSelection>>())
        })
        .and_then(|bytes| bytes.checked_add(4096))
        .ok_or_else(|| invalid("initial V2 selector state bound overflow"))?;
    if state_upper > accountant.max_working_state_bytes() {
        return Err(invalid(
            "initial V2 selector exceeds the original V2 state slice",
        ));
    }
    Ok(limits)
}

fn path_guard_upper(path: &Path) -> io::Result<u64> {
    let path_bytes = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || path_bytes.len() > ROOT_PATH_BYTES
        || path.components().count() > ROOT_PATH_COMPONENTS
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid(
            "initial source root path is not bounded and normalized",
        ));
    }
    path.components()
        .try_fold(ROOT_METADATA_GUARD_BYTES, |total, component| {
            let name_bytes = u64::try_from(component.as_os_str().as_encoded_bytes().len())
                .map_err(|_| invalid("initial source root component length exceeds range"))?;
            total
                .checked_add(name_bytes)
                .and_then(|bytes| bytes.checked_add(1 + ROOT_METADATA_GUARD_BYTES))
                .ok_or_else(|| invalid("initial source root path guard overflow"))
        })
}

fn root_path_component_count(path: &Path) -> io::Result<u64> {
    u64::try_from(path.components().count())
        .map_err(|_| invalid("initial source root component count exceeds range"))
}

fn file_stamp(metadata: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn verify_named_root(
    root_name: &Path,
    root: &File,
    identity: (u64, u64),
    stamp: (u64, u64, u64, i64, i64, i64, i64),
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    active(deadline, cancel)?;
    charge_read_upper(io, path_guard_upper(root_name)?)?;
    // The path envelope covers name resolution and one descriptor metadata
    // read; precharge the second held-root metadata comparison before opening.
    charge_read_upper(io, ROOT_METADATA_GUARD_BYTES)?;
    let named = open_root_nofollow(root_name)?;
    let metadata = named.metadata()?;
    if !metadata.is_dir()
        || (metadata.dev(), metadata.ino()) != identity
        || file_stamp(&root.metadata()?) != stamp
        || file_stamp(&metadata) != stamp
    {
        return Err(invalid(
            "initial source root named identity or stamp changed",
        ));
    }
    Ok(())
}

fn open_selected_root(
    root_name: &Path,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<(File, (u64, u64), (u64, u64, u64, i64, i64, i64, i64))> {
    active(deadline, cancel)?;
    charge_read_upper(io, path_guard_upper(root_name)?)?;
    let root = open_root_nofollow(root_name)?;
    let metadata = root.metadata()?;
    if !metadata.is_dir() {
        return Err(invalid("initial source root path is not a directory"));
    }
    Ok((
        root,
        (metadata.dev(), metadata.ino()),
        file_stamp(&metadata),
    ))
}

fn open_root_nofollow(path: &Path) -> io::Result<File> {
    let mut current = File::open("/")?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current =
                    tos_fd_open::open_directory_at(&current, Path::new(name)).map_err(invalid)?;
            }
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(invalid(
                    "initial source root path component is not normalized",
                ));
            }
        }
    }
    Ok(current)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_initial_cut<'a>(
    root_name: &'a Path,
    invocation: &mut NativeSourceValidator<'_>,
    store: &AdmissionStore,
    _read_limits: ReadLimits,
    accountant: &Arc<NativeV2TreeIo>,
    profile: InitialCutProfile,
    workspace: File,
    aux: PinnedSqliteAuxRequest,
    indexed_input: Option<crate::source_admission_indexed_input::IndexedInputRequestV1>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<InitialCutPrepared<'a>> {
    check_state_partition(profile)?;
    active(deadline, cancel)?;
    if aux.deadline != deadline || !std::ptr::eq(Arc::as_ptr(&aux.cancelled), cancel) {
        return Err(invalid(
            "initial source cut original invocation binding differs",
        ));
    }
    let spool_io = aux.io_budget.clone();
    let v2_io = accountant.io_budget().clone();
    invocation.verify_spooled_v2_io(&spool_io, &v2_io, deadline, cancel)?;
    if !store.has_v2_allocation_accountant(accountant) {
        return Err(invalid(
            "initial V2 store is not bound to the original accountant",
        ));
    }
    invocation.reserve_spooled_external_state(profile.max_state_slice_bytes, &spool_io)?;
    let mut work = InitialCutWork::new(profile.max_work_units)?;
    work.charge_many(root_path_component_count(root_name)?.saturating_add(1))?;
    let (root, root_identity, root_stamp) =
        open_selected_root(root_name, &spool_io, deadline, cancel)?;
    let retained_cancel = Arc::clone(&aux.cancelled);
    work.charge_many(32)?;
    let mut scope = PinnedSqliteAuxScope::new(workspace, aux)
        .map_err(|_| invalid("initial source cut held AUX scope refused"))?;
    let db =
        Rc::new(RefCell::new(scope.open_connection().map_err(|_| {
            invalid("initial source cut AUX connection refused")
        })?));
    work.charge_many(16)?;
    let progress_deadline = deadline;
    let progress_cancel = Arc::clone(&retained_cancel);
    {
        let db_guard = db.borrow_mut();
        configure_db(&db_guard, profile.sqlite_cache_bytes)?;
        create_object_inventory_table(&db_guard, &mut work)?;
        db_guard.progress_handler(
            1000,
            Some(move || active(progress_deadline, &progress_cancel).is_err()),
        );
    }
    work.charge_many(CENSUS_CONTROL_SQL_WORK_UNITS)?;
    let mut callback = |kind| work.charge(kind);
    let proposal = {
        let db_guard = db.borrow_mut();
        census_selected_to_scratch(
            &root,
            &db_guard,
            &spool_io,
            SourceCensusScan::Proposal,
            profile.census,
            &mut callback,
            deadline,
            cancel,
        )?
    };
    drop(callback);
    let indexed_reader = if let Some(request) = indexed_input {
        let selection = crate::source_admission_indexed_input::open_selection_from_manifest_v1(
            &request.named_root,
            request.segment_limits,
            request.reader_limits.max_descriptor_bytes,
            request.max_profile_bytes,
            request.max_dependency_closure_bytes,
            &spool_io,
            deadline,
            cancel,
            &work,
        )?;
        if selection.member_count != proposal.member_count
            || selection.source_bytes != proposal.source_bytes
        {
            return Err(invalid(
                "indexed input totals differ from the held source census",
            ));
        }
        {
            let db_guard = db.borrow_mut();
            verify_indexed_unique_totals(
                &db_guard,
                SourceCensusScan::Proposal,
                proposal,
                selection.unique_object_count,
                selection.unique_payload_bytes,
                &mut work,
                deadline,
                cancel,
            )?;
        }
        Some(
            crate::source_admission_indexed_input::IndexedInputReaderV1::open(
                selection,
                request.reader_limits,
                db.clone(),
                spool_io.clone(),
                deadline,
                Arc::clone(&retained_cancel),
                work.clone(),
            )?,
        )
    } else {
        None
    };
    let mut batch = AdmissionBatch::from_verified_census_rows(
        db.clone(),
        SourceCensusScan::Proposal.label(),
        proposal.member_count,
        proposal.source_bytes,
        invocation.identity(),
        root.try_clone()
            .map_err(|_| invalid("initial source root descriptor clone refused"))?,
        profile.admission,
        profile.census.max_path_bytes,
        profile.update_rows_state_bytes,
        profile.batch_builder_state_bytes,
        work.clone(),
        &spool_io,
        deadline,
        cancel,
    )?;
    if let Some(reader) = indexed_reader {
        batch.attach_indexed_input(reader, proposal.member_count, proposal.source_bytes)?;
    }
    Ok(InitialCutPrepared {
        batch: Some(batch),
        fence: InitialCutFence {
            root_name,
            root: root
                .try_clone()
                .map_err(|_| invalid("initial source root fence clone refused"))?,
            root_identity,
            root_stamp,
            proposal,
            profile,
            _scope: scope,
            db,
            spool_io,
            v2_io,
            deadline,
            cancel: retained_cancel,
            work,
            miss_baseline_verified: false,
        },
    })
}

impl InitialCutFence<'_> {
    fn verify_initial_miss_baseline(
        &mut self,
        invocation: &NativeSourceValidator<'_>,
        store: &AdmissionStore,
        limits: ReadLimits,
        accountant: &Arc<NativeV2TreeIo>,
    ) -> io::Result<()> {
        active(self.deadline, &self.cancel)?;
        invocation.verify_spooled_v2_io(
            &self.spool_io,
            &self.v2_io,
            self.deadline,
            &self.cancel,
        )?;
        if !accountant.io_budget().shares_with(&self.v2_io)
            || !store.has_v2_allocation_accountant(accountant)
        {
            return Err(invalid("initial miss baseline original binding changed"));
        }
        self.work.charge_many(8)?;
        if store
            .current_selection(
                narrow_current_limits(limits, accountant)?,
                self.deadline,
                &self.cancel,
                Some(&self.v2_io),
            )?
            .is_some()
        {
            return Err(invalid("initial V2 source cut requires an empty selector"));
        }
        verify_initial_store_baseline(
            store,
            &self.v2_io,
            &mut self.work,
            self.deadline,
            &self.cancel,
        )?;
        self.miss_baseline_verified = true;
        Ok(())
    }

    fn verify_before_publish(
        &mut self,
        invocation: &NativeSourceValidator<'_>,
        store: &AdmissionStore,
        limits: ReadLimits,
        accountant: &Arc<NativeV2TreeIo>,
    ) -> io::Result<()> {
        active(self.deadline, &self.cancel)?;
        if !self.miss_baseline_verified {
            return Err(invalid(
                "initial publication requires an empty-baseline miss check",
            ));
        }
        invocation.verify_spooled_v2_io(
            &self.spool_io,
            &self.v2_io,
            self.deadline,
            &self.cancel,
        )?;
        if !accountant.io_budget().shares_with(&self.v2_io)
            || !store.has_v2_allocation_accountant(accountant)
        {
            return Err(invalid("initial V2 publisher original binding changed"));
        }
        self.work.charge_many(CENSUS_CONTROL_SQL_WORK_UNITS)?;
        let mut callback = |kind| self.work.charge(kind);
        let terminal = {
            let db_guard = self.db.borrow_mut();
            census_selected_to_scratch(
                &self.root,
                &db_guard,
                &self.spool_io,
                SourceCensusScan::Terminal,
                self.profile.census,
                &mut callback,
                self.deadline,
                &self.cancel,
            )?
        };
        drop(callback);
        if terminal != self.proposal {
            return Err(invalid(
                "initial terminal source cut differs from its proposal",
            ));
        }
        self.work.charge_many(8)?;
        let limits = narrow_current_limits(limits, accountant)?;
        if store
            .current_selection(limits, self.deadline, &self.cancel, Some(&self.v2_io))?
            .is_some()
        {
            return Err(invalid("initial V2 selector appeared before publication"));
        }
        {
            let db_guard = self.db.borrow_mut();
            verify_terminal_store_namespaces(
                store,
                &db_guard,
                &self.v2_io,
                &self.spool_io,
                self.profile,
                self.proposal,
                &mut self.work,
                self.deadline,
                &self.cancel,
            )?;
        }
        self.work
            .charge_many(root_path_component_count(self.root_name)?.saturating_add(1))?;
        verify_named_root(
            self.root_name,
            &self.root,
            self.root_identity,
            self.root_stamp,
            &self.spool_io,
            self.deadline,
            &self.cancel,
        )?;
        Ok(())
    }
}
