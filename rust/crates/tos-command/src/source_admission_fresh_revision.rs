//! Bridge an already committed, protected `record.revise` Work transaction to
//! a canonical native admission proposal. The Work journal supplies the exact
//! owner proposal and old/new bytes; the held V2 view supplies the immutable
//! base; a complete fd-rooted census supplies the live selected source cut.
//! None of these observations is an admission witness. Native validation and
//! the terminal fence remain required before the ordinary V2 publisher runs.

use super::source_admission::{
    AdmissionBatch, AdmissionLimits, SourceUpdate, active as active_io, invalid,
};
use super::source_admission_segment_v2::SourceRevisionRootsV2;
use super::source_admission_source_census::{
    SourceCensusLimits, SourceCensusScan, SourceCensusSummary, SourceCensusWorkKind,
    census_selected_to_scratch, working_state_upper_bound,
};
use super::source_admission_v2_reader::{V2MemberTupleObservation, V2ReadSession};
use super::source_admission_v2_seen_pack::configure_db;
use super::source_creation_store::{CreationFilesystem, active as active_owner};
use super::source_foundation_admission::NativeSourceValidator;
use super::source_work_transaction::{
    RecordRevisionInspectionLimits, WorkArchive, WorkCorpusFence,
};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceFile};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io;
use std::mem::size_of;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Instant;
use tos_foundation::{Digest256, SourceRevision};
use tos_source_store::{
    PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteConnection, PinnedSqliteIoBudget,
};

#[derive(Clone, Copy)]
pub(crate) struct FreshRevisionProfile {
    pub(crate) inspection: RecordRevisionInspectionLimits,
    pub(crate) census: SourceCensusLimits,
    pub(crate) admission: AdmissionLimits,
    /// Sum of these slices is checked against the already selected source
    /// operation state allowance. This is a partition, not a new grant.
    pub(crate) max_state_slice_bytes: usize,
    pub(crate) work_state_slice_bytes: usize,
    /// Retained bounded Work facts plus the expected/update maps live while
    /// the ordered census/base merge constructs the canonical batch.
    pub(crate) derived_rows_state_bytes: usize,
    pub(crate) cursor_caller_state_bytes: usize,
    pub(crate) batch_builder_state_bytes: usize,
    pub(crate) retained_fence_state_bytes: usize,
    /// SQLite page-cache limit selected from the same original operation
    /// budget. The matching native-overhead allowance is a finite caller
    /// partition, not an empirical allocator measurement.
    pub(crate) sqlite_cache_bytes: usize,
    pub(crate) sqlite_native_overhead_bytes: usize,
    /// Shared monotonic unit ceiling for both censuses and the authenticated
    /// base merge. Every unit is charged before its corresponding operation.
    pub(crate) max_work_units: u64,
}

impl FreshRevisionProfile {
    /// Partition an already selected source-operation allowance using only
    /// its original finite row, path, parser, and work limits. This creates
    /// no additional state authority and refuses if the full simultaneous
    /// bridge envelope does not fit.
    pub(crate) fn from_original_limits(
        original_state_slice_bytes: usize,
        inspection: RecordRevisionInspectionLimits,
        mut census: SourceCensusLimits,
        admission: AdmissionLimits,
        sqlite_cache_bytes: usize,
        sqlite_native_overhead_bytes: usize,
        max_work_units: u64,
    ) -> io::Result<Self> {
        if original_state_slice_bytes == 0 || original_state_slice_bytes == usize::MAX {
            return Err(invalid("fresh revision original state slice is not finite"));
        }
        census.state_slice_bytes = working_state_upper_bound(census.max_path_bytes)?;
        let mut profile = Self {
            inspection,
            census,
            admission,
            max_state_slice_bytes: original_state_slice_bytes,
            work_state_slice_bytes: 1,
            derived_rows_state_bytes: 1,
            cursor_caller_state_bytes: 1,
            batch_builder_state_bytes: 1,
            retained_fence_state_bytes: 1,
            sqlite_cache_bytes,
            sqlite_native_overhead_bytes,
            max_work_units,
        };
        profile.work_state_slice_bytes = minimum_work_state_upper_bound(profile)?;
        profile.derived_rows_state_bytes = minimum_derived_rows_state_upper_bound(profile)?;
        profile.cursor_caller_state_bytes = profile
            .census
            .max_path_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<V2MemberTupleObservation>() + 512))
            .ok_or_else(|| invalid("fresh revision cursor state upper overflow"))?;
        profile.batch_builder_state_bytes = minimum_batch_builder_state_upper_bound(profile)?;
        profile.retained_fence_state_bytes = minimum_retained_fence_state_upper_bound(profile)?;
        state_partition(profile)?;
        Ok(profile)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FreshRevisionDisposition {
    /// The original base is still selected and a complete live-source census
    /// matched that base plus exactly the committed Work changes.
    NewPublication,
    /// The pointer already advanced. The caller may only look up this exact
    /// canonical batch in authenticated history; it must not publish it anew.
    RetryLookupOnly,
}

pub(crate) struct FreshRevisionPrepared<'a> {
    batch: Option<AdmissionBatch>,
    disposition: FreshRevisionDisposition,
    fence: FreshRevisionFence<'a>,
}

impl FreshRevisionPrepared<'_> {
    pub(crate) fn disposition(&self) -> FreshRevisionDisposition {
        self.disposition
    }

    pub(crate) fn take_batch(&mut self) -> io::Result<AdmissionBatch> {
        self.batch
            .take()
            .ok_or_else(|| invalid("fresh revision batch already consumed"))
    }

    /// Rust-side fence plus the SQLite cache/native terms remain live after
    /// the batch is moved to the normal candidate/validator path.
    pub(crate) fn retained_state_upper_bound_bytes(&self) -> usize {
        self.fence
            .profile
            .retained_fence_state_bytes
            .checked_add(self.fence.profile.sqlite_cache_bytes)
            .and_then(|bytes| bytes.checked_add(self.fence.profile.sqlite_native_overhead_bytes))
            .unwrap_or(usize::MAX)
    }

    pub(crate) fn work_units_used(&self) -> u64 {
        self.fence.work.used
    }

    pub(crate) fn proposal_census(&self) -> Option<SourceCensusSummary> {
        (self.disposition == FreshRevisionDisposition::NewPublication)
            .then_some(self.fence.proposal_census)
    }

    /// The only publication path: Work owner/configuration, exact source
    /// census, and selected-current V2 fence are rechecked after Native
    /// validation. A retry candidate deliberately cannot use this method.
    pub(crate) fn verify_before_publish(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        if self.disposition != FreshRevisionDisposition::NewPublication {
            return Err(invalid("retry-only fresh revision cannot publish"));
        }
        self.fence.verify_before_publish(base)
    }

    /// A recovered result is only returned after the caller has matched the
    /// canonical batch against V2 retained history. This rechecks the Work
    /// proposal and current pointer without claiming that today's filesystem
    /// is a fresh admitted cut. The caller must invoke it only after an exact
    /// `find_accepted_batch` result.
    pub(crate) fn verify_retry_outcome(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        self.fence.verify_retry_outcome(base)
    }
}

struct FreshRevisionFence<'a> {
    fs: &'a CreationFilesystem,
    work_fence: WorkCorpusFence<'a>,
    archive: WorkArchive,
    transaction_id: String,
    expected_record_id: String,
    manifest_sha256: Digest256,
    original_base: SourceRevision,
    selected_revision: SourceRevision,
    proposal_census: SourceCensusSummary,
    profile: FreshRevisionProfile,
    // Keep the original held workspace, quota, and shared IO/state owner alive
    // for both proposal and terminal census passes.
    _scope: PinnedSqliteAuxScope,
    db: PinnedSqliteConnection,
    io: PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    work: CensusWork,
}

#[derive(Clone, Copy)]
struct CensusWork {
    used: u64,
    maximum: u64,
}

impl CensusWork {
    fn new(maximum: u64) -> io::Result<Self> {
        if maximum == 0 || maximum == u64::MAX {
            return Err(invalid(
                "fresh revision requires finite shared work ceiling",
            ));
        }
        Ok(Self { used: 0, maximum })
    }

    fn charge(&mut self, _kind: SourceCensusWorkKind) -> io::Result<()> {
        self.used = self
            .used
            .checked_add(1)
            .filter(|used| *used <= self.maximum)
            .ok_or_else(|| invalid("fresh revision shared work ceiling exhausted"))?;
        Ok(())
    }

    fn charge_many(&mut self, count: u64) -> io::Result<()> {
        self.used = self
            .used
            .checked_add(count)
            .filter(|used| *used <= self.maximum)
            .ok_or_else(|| invalid("fresh revision shared work ceiling exhausted"))?;
        Ok(())
    }
}

#[derive(Clone)]
struct ExpectedChange {
    before_sha256: Digest256,
    before_size: u64,
    after_sha256: Digest256,
    after_size: u64,
    seen: bool,
}

struct WorkFacts {
    manifest_sha256: Digest256,
    archive: WorkArchive,
    expected: BTreeMap<String, ExpectedChange>,
}

fn charge_read_upper(io: &PinnedSqliteIoBudget, bytes: u64) -> io::Result<()> {
    io.charge_read_upper_bound(bytes).map_err(invalid)
}

fn owner_configuration_upper(fs: &CreationFilesystem) -> io::Result<u64> {
    let path_bytes = u64::try_from(fs.protected_root_path().as_os_str().len())
        .map_err(|_| invalid("fresh revision protected-root path length"))?
        .checked_add(
            u64::try_from(fs.protected_configuration_path().as_os_str().len())
                .map_err(|_| invalid("fresh revision configuration path length"))?,
        )
        .ok_or_else(|| invalid("fresh revision protected path length overflow"))?;
    1_048_576u64
        .checked_add(65_536)
        .and_then(|bytes| bytes.checked_add(path_bytes.checked_mul(4096)?))
        .ok_or_else(|| invalid("fresh revision owner-configuration read upper overflow"))
}

fn work_owner_read_upper(profile: FreshRevisionProfile) -> io::Result<u64> {
    let l = profile.inspection;
    let manifests = u64::try_from(l.max_manifest_bytes)
        .map_err(|_| invalid("fresh revision Work manifest bound"))?
        .checked_mul(3)
        .ok_or_else(|| invalid("fresh revision Work manifest read upper overflow"))?;
    let blobs = u64::try_from(l.max_total_blob_bytes)
        .map_err(|_| invalid("fresh revision Work blob bound"))?
        .checked_mul(3)
        .ok_or_else(|| invalid("fresh revision Work blob read upper overflow"))?;
    let sides = u64::try_from(l.max_total_side_bytes)
        .map_err(|_| invalid("fresh revision Work side bound"))?
        .checked_mul(2)
        .ok_or_else(|| invalid("fresh revision Work side read upper overflow"))?;
    let files = u64::try_from(l.max_files)
        .map_err(|_| invalid("fresh revision Work file count"))?
        .checked_mul(4096)
        .ok_or_else(|| invalid("fresh revision Work metadata read upper overflow"))?;
    manifests
        .checked_add(blobs)
        .and_then(|bytes| bytes.checked_add(sides))
        .and_then(|bytes| bytes.checked_add(files))
        .and_then(|bytes| bytes.checked_add(131_072))
        .ok_or_else(|| invalid("fresh revision Work read upper overflow"))
}

fn charge_owner_reads(
    fs: &CreationFilesystem,
    profile: FreshRevisionProfile,
    io: &PinnedSqliteIoBudget,
) -> io::Result<()> {
    charge_read_upper(io, owner_configuration_upper(fs)?)?;
    charge_read_upper(io, work_owner_read_upper(profile)?)
}

fn work_owner_work_upper(profile: FreshRevisionProfile) -> io::Result<u64> {
    u64::try_from(profile.inspection.max_files)
        .map_err(|_| invalid("fresh revision Work file count"))?
        .checked_mul(16)
        .and_then(|units| units.checked_add(64))
        .ok_or_else(|| invalid("fresh revision Work operation upper overflow"))
}

fn minimum_work_state_upper_bound(profile: FreshRevisionProfile) -> io::Result<usize> {
    let limits = profile.inspection;
    let manifest = limits
        .max_manifest_bytes
        .checked_mul(16)
        .ok_or_else(|| invalid("fresh revision Work manifest state overflow"))?;
    let blobs = limits
        .max_total_blob_bytes
        .checked_mul(24)
        .ok_or_else(|| invalid("fresh revision Work blob state overflow"))?;
    let side = limits
        .max_total_side_bytes
        .checked_mul(12)
        .ok_or_else(|| invalid("fresh revision Work side state overflow"))?;
    let config = 1_048_576usize
        .checked_mul(16)
        .ok_or_else(|| invalid("fresh revision owner configuration state overflow"))?;
    manifest
        .checked_add(blobs)
        .and_then(|n| n.checked_add(side))
        .and_then(|n| n.checked_add(config))
        .and_then(|n| n.checked_add(131_072))
        .ok_or_else(|| invalid("fresh revision Work state upper overflow"))
}

fn minimum_derived_rows_state_upper_bound(profile: FreshRevisionProfile) -> io::Result<usize> {
    let path = profile.census.max_path_bytes;
    let maps = path
        .checked_mul(2)
        .and_then(|n| n.checked_add(512))
        .and_then(|n| n.checked_mul(6))
        .and_then(|n| n.checked_add(8192))
        .ok_or_else(|| invalid("fresh revision derived row state upper overflow"))?;
    let roots = SourceRevisionRootsV2::retained_state_upper_bound_for_value(
        SourceRevisionRootsV2::MAX_ENCODED_BYTES,
    )?;
    maps.checked_add(roots)
        .ok_or_else(|| invalid("fresh revision root and row state overflow"))
}

fn minimum_batch_builder_state_upper_bound(profile: FreshRevisionProfile) -> io::Result<usize> {
    profile
        .census
        .max_path_bytes
        .checked_mul(16)
        .and_then(|n| n.checked_add(2048))
        .and_then(|n| n.checked_mul(3))
        .and_then(|n| n.checked_add(8192))
        .ok_or_else(|| invalid("fresh revision batch builder state upper overflow"))
}

fn minimum_retained_fence_state_upper_bound(profile: FreshRevisionProfile) -> io::Result<usize> {
    let path_bytes = profile
        .census
        .max_path_bytes
        .checked_mul(10)
        .ok_or_else(|| invalid("fresh revision retained path state overflow"))?;
    size_of::<FreshRevisionFence<'static>>()
        .checked_add(size_of::<FreshRevisionPrepared<'static>>())
        .and_then(|n| n.checked_add(path_bytes))
        // The retained transaction id and selected record id are separate
        // input strings; the latter may be as long as its validated bound.
        .and_then(|n| n.checked_add(80 + 1024))
        .and_then(|n| n.checked_add(3 * 512))
        .and_then(|n| n.checked_add(8192))
        .ok_or_else(|| invalid("fresh revision retained fence state upper overflow"))
}

fn state_partition(profile: FreshRevisionProfile) -> io::Result<()> {
    let census = working_state_upper_bound(profile.census.max_path_bytes)?;
    let census_limits = profile.census;
    let minimum_work = minimum_work_state_upper_bound(profile)?;
    let minimum_rows = minimum_derived_rows_state_upper_bound(profile)?;
    let minimum_fence = minimum_retained_fence_state_upper_bound(profile)?;
    if census > profile.census.state_slice_bytes
        || profile.work_state_slice_bytes < minimum_work
        || profile.work_state_slice_bytes == usize::MAX
        || profile.derived_rows_state_bytes < minimum_rows
        || profile.derived_rows_state_bytes == usize::MAX
        || profile.cursor_caller_state_bytes == 0
        || profile.cursor_caller_state_bytes == usize::MAX
        || profile.batch_builder_state_bytes == 0
        || profile.batch_builder_state_bytes == usize::MAX
        || profile.retained_fence_state_bytes < minimum_fence
        || profile.retained_fence_state_bytes == usize::MAX
        || profile.sqlite_cache_bytes < 1024
        || profile.sqlite_cache_bytes / 1024 > i32::MAX as usize
        || profile.sqlite_native_overhead_bytes == 0
        || profile.sqlite_native_overhead_bytes == usize::MAX
        || profile.max_state_slice_bytes == 0
        || profile.max_state_slice_bytes == usize::MAX
        || profile.inspection.max_files != 3
        || census_limits.max_files == 0
        || census_limits.max_files == u64::MAX
        || census_limits.max_files > i64::MAX as u64
        || census_limits.max_directories == 0
        || census_limits.max_directories == u64::MAX
        || census_limits.max_entries == 0
        || census_limits.max_entries == u64::MAX
        || census_limits.max_path_bytes < 3
        || census_limits.max_path_bytes == usize::MAX
        || census_limits.max_depth == usize::MAX
        || i64::try_from(census_limits.max_depth).is_err()
        || census_limits.max_member_bytes == 0
        || census_limits.max_member_bytes == u64::MAX
        || census_limits.max_member_bytes > i64::MAX as u64
        || census_limits.max_source_bytes == 0
        || census_limits.max_source_bytes == u64::MAX
        || census_limits.max_source_bytes > i64::MAX as u64
        || census_limits.max_member_bytes > census_limits.max_source_bytes
        || profile.max_work_units == 0
        || profile.max_work_units == u64::MAX
    {
        return Err(invalid("fresh revision finite state partition differs"));
    }
    let required = profile
        .census
        .state_slice_bytes
        .checked_add(profile.work_state_slice_bytes)
        .and_then(|n| n.checked_add(profile.derived_rows_state_bytes))
        .and_then(|n| n.checked_add(profile.cursor_caller_state_bytes))
        .and_then(|n| n.checked_add(profile.batch_builder_state_bytes))
        .and_then(|n| n.checked_add(profile.retained_fence_state_bytes))
        .and_then(|n| n.checked_add(profile.sqlite_cache_bytes))
        .and_then(|n| n.checked_add(profile.sqlite_native_overhead_bytes))
        .ok_or_else(|| invalid("fresh revision state partition overflow"))?;
    if required > profile.max_state_slice_bytes {
        return Err(invalid(
            "fresh revision exceeds selected original state slice",
        ));
    }
    profile.admission.validate()?;
    Ok(())
}

fn work_error(_: SourceCommandError) -> io::Error {
    invalid("protected record revision Work evidence refused")
}

fn inspect_work_facts(
    fs: &CreationFilesystem,
    transaction_id: &str,
    expected_record_id: &str,
    original_base: SourceRevision,
    profile: FreshRevisionProfile,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
    work: &mut CensusWork,
) -> io::Result<WorkFacts> {
    active_owner(deadline, cancel).map_err(work_error)?;
    work.charge_many(work_owner_work_upper(profile)?)?;
    charge_owner_reads(fs, profile, io)?;
    fs.verify_selected_configuration(deadline, cancel)
        .map_err(work_error)?;
    let (manifest, plan, base_publication, _terminal) =
        super::source_work_transaction::inspect_committed_record_revision(
            fs,
            transaction_id,
            profile.inspection,
            deadline,
            cancel,
        )
        .map_err(work_error)?;
    if plan.transaction_id != transaction_id
        || expected_record_id.is_empty()
        || expected_record_id.len() > 1024
        || plan.item_path_profile.is_some()
        || !plan.new_directories.is_empty()
        || plan.files.len() != 3
    {
        return Err(invalid(
            "committed Work transaction is not exact record.revise",
        ));
    }
    let manifest_sha256 = Digest256::from_prefixed(&manifest).map_err(invalid)?;
    let auth = &plan.authorization;
    cmd::exact_keys(
        auth,
        &[
            "schema_version",
            "principal_id",
            "authority_ref",
            "source_path",
            "record_id",
            "record_type",
            "request",
        ],
    )
    .map_err(work_error)?;
    if cmd::text(auth, "schema_version").map_err(work_error)?
        != "tos_selected_metadata_revision_authorization_v1"
        || cmd::text(auth, "record_id").map_err(work_error)? != expected_record_id
    {
        return Err(invalid(
            "committed Work transaction has another owner family",
        ));
    }
    let request = cmd::field(auth, "request").map_err(work_error)?;
    if cmd::text(request, "schema_version").map_err(work_error)? != "tos_local_source_command_v1"
        || cmd::text(request, "operation").map_err(work_error)? != "record.revise"
        || crate::source_revisions::transaction_id(request).map_err(work_error)? != transaction_id
    {
        return Err(invalid("committed Work request identity differs"));
    }

    let request_raw = cmd::canonical(request).map_err(work_error)?;
    let mut before = BTreeMap::<String, Vec<u8>>::new();
    for file in &plan.files {
        let name = file
            .path
            .as_str()
            .rsplit('/')
            .next()
            .ok_or_else(|| invalid("committed Work selected member basename"))?;
        if before
            .insert(
                name.to_owned(),
                file.before
                    .as_ref()
                    .ok_or_else(|| invalid("committed Work predecessor bytes absent"))?
                    .clone(),
            )
            .is_some()
        {
            return Err(invalid("duplicate committed Work selected basename"));
        }
        if file.after.is_none() {
            return Err(invalid("committed Work successor bytes absent"));
        }
    }
    let source_path = cmd::text(auth, "source_path").map_err(work_error)?;
    let names = crate::source_revisions::names(source_path).map_err(work_error)?;
    if before.len() != names.len() || names.iter().any(|name| !before.contains_key(name)) {
        return Err(invalid("committed Work selected file set differs"));
    }
    let parent = source_path
        .rsplit_once('/')
        .ok_or_else(|| invalid("committed Work source path parent absent"))?
        .0;
    let expected_paths = names
        .iter()
        .map(|name| format!("{parent}/{name}"))
        .collect::<BTreeSet<_>>();
    if plan
        .files
        .iter()
        .map(|file| file.path.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        != expected_paths
    {
        return Err(invalid("committed Work selected paths differ"));
    }

    let mut context_files = Vec::with_capacity(plan.files.len());
    for file in &plan.files {
        let path = file.path.clone();
        let raw = file
            .before
            .as_ref()
            .ok_or_else(|| invalid("committed Work predecessor bytes absent"))?
            .clone();
        context_files.push(SourceFile { path, raw });
    }
    let context = CommandContext {
        base_revision: original_base,
        configuration_raw: fs.protected_configuration_raw().to_vec(),
        request_raw,
        recorded_at: crate::source_serialization::instant().map_err(work_error)?,
        effective_uid: u64::from(fs.protected_uid()),
        files: context_files,
    };
    let (config, family) = crate::source_revisions::configuration(&context).map_err(work_error)?;
    if !family.selected()
        || cmd::text(&config, "source_path").map_err(work_error)? != source_path
        || cmd::text(&config, "record_id").map_err(work_error)?
            != cmd::text(auth, "record_id").map_err(work_error)?
        || cmd::field(&config, "record_type").map_err(work_error)?
            != cmd::field(auth, "record_type").map_err(work_error)?
    {
        return Err(invalid(
            "committed Work is outside current selected owner scope",
        ));
    }
    let configured_root = crate::source_text_owner::normalized_absolute(
        cmd::text(&config, "source_root").map_err(work_error)?,
    )
    .map_err(work_error)?;
    if configured_root != fs.protected_root_path() {
        return Err(invalid(
            "committed Work owner root differs from held source root",
        ));
    }
    for name in [
        "principal_id",
        "authority_ref",
        "source_path",
        "record_id",
        "record_type",
    ] {
        if cmd::field(auth, name).map_err(work_error)?
            != cmd::field(&config, name).map_err(work_error)?
        {
            return Err(invalid(
                "committed Work owner binding differs from current configuration",
            ));
        }
    }

    // This is the selected metadata package revision, distinct from the V2
    // corpus SourceRevision supplied by the caller. The exact V2 base is
    // established later by the ordered full-member tuple comparison.
    let old_revision = crate::source_revisions::revision(&before).map_err(work_error)?;
    let before_record = cmd::parse(
        before
            .get(&names[0])
            .ok_or_else(|| invalid("committed Work original Record absent"))?,
    )
    .map_err(work_error)?;
    let before_subject =
        crate::source_forms::metadata_subject(&before_record).map_err(work_error)?;
    if cmd::text(&before_subject, "id").map_err(work_error)?
        != cmd::text(&config, "record_id").map_err(work_error)?
        || cmd::text(request, "expected_configuration").map_err(work_error)?
            != cmd::record_digest(&config)
                .map_err(work_error)?
                .to_prefixed()
        || !cmd::same(
            cmd::field(request, "expected_source").map_err(work_error)?,
            &before_subject,
        )
        .map_err(work_error)?
        || cmd::text(request, "expected_revision").map_err(work_error)? != old_revision
        || cmd::field(request, "expected_publication").map_err(work_error)?
            != cmd::field(&base_publication, "token").map_err(work_error)?
    {
        return Err(invalid(
            "committed Work request is not bound to its exact predecessor",
        ));
    }
    let after = plan
        .files
        .iter()
        .map(|file| {
            let name = file
                .path
                .as_str()
                .rsplit('/')
                .next()
                .ok_or_else(|| invalid("committed Work successor basename"))?;
            Ok((
                name.to_owned(),
                file.after
                    .as_ref()
                    .ok_or_else(|| invalid("committed Work successor bytes absent"))?
                    .clone(),
            ))
        })
        .collect::<io::Result<BTreeMap<_, _>>>()?;
    let after_record = cmd::parse(
        after
            .get(&names[0])
            .ok_or_else(|| invalid("committed Work successor Record absent"))?,
    )
    .map_err(work_error)?;
    let after_subject = crate::source_forms::metadata_subject(&after_record).map_err(work_error)?;
    if cmd::text(&after_subject, "id").map_err(work_error)?
        != cmd::text(&before_subject, "id").map_err(work_error)?
        || cmd::integer(&after_subject, "version").map_err(work_error)?
            != cmd::integer(&before_subject, "version")
                .map_err(work_error)?
                .checked_add(1)
                .ok_or_else(|| invalid("committed Work Record version overflow"))?
    {
        return Err(invalid(
            "committed Work successor identity or version differs",
        ));
    }
    let after_history =
        crate::source_revisions::history(&after, &after_record).map_err(work_error)?;
    let receipts = cmd::array(&after_history, "receipts").map_err(work_error)?;
    let receipt = receipts
        .last()
        .ok_or_else(|| invalid("committed Work successor receipt absent"))?;
    if !cmd::same(cmd::field(receipt, "request").map_err(work_error)?, request)
        .map_err(work_error)?
    {
        return Err(invalid(
            "committed Work receipt differs from protected request",
        ));
    }

    let mut expected = BTreeMap::new();
    for file in &plan.files {
        let before_raw = file
            .before
            .as_ref()
            .ok_or_else(|| invalid("committed Work predecessor bytes absent"))?;
        let after_raw = file
            .after
            .as_ref()
            .ok_or_else(|| invalid("committed Work successor bytes absent"))?;
        let key = file.path.as_str().to_owned();
        if expected
            .insert(
                key,
                ExpectedChange {
                    before_sha256: Digest256::of_bytes(before_raw),
                    before_size: u64::try_from(before_raw.len())
                        .map_err(|_| invalid("committed Work predecessor size"))?,
                    after_sha256: Digest256::of_bytes(after_raw),
                    after_size: u64::try_from(after_raw.len())
                        .map_err(|_| invalid("committed Work successor size"))?,
                    seen: false,
                },
            )
            .is_some()
        {
            return Err(invalid("duplicate committed Work selected path"));
        }
    }
    let archive = super::source_work_transaction::record_revision_archive(
        fs,
        &context,
        &before_record,
        &before,
        &old_revision,
        deadline,
        cancel,
        false,
    )
    .map_err(work_error)?;
    if archive.path
        != crate::source_revisions::archive_path(&config, &old_revision).map_err(work_error)?
    {
        return Err(invalid("committed Work predecessor archive path differs"));
    }
    archive
        .verify_current(fs, deadline, cancel)
        .map_err(work_error)?;
    Ok(WorkFacts {
        manifest_sha256,
        archive,
        expected,
    })
}

struct ScratchMember {
    path: String,
    sha256: Digest256,
    size: u64,
    mode: u32,
}

fn read_scratch_member(row: &rusqlite::Row<'_>) -> io::Result<ScratchMember> {
    let path = row
        .get::<_, String>(0)
        .map_err(|_| invalid("fresh revision census path row"))?;
    let digest = row
        .get::<_, Vec<u8>>(1)
        .map_err(|_| invalid("fresh revision census digest row"))?;
    let digest: [u8; 32] = digest
        .try_into()
        .map_err(|_| invalid("fresh revision census digest length"))?;
    let size = row
        .get::<_, i64>(2)
        .map_err(|_| invalid("fresh revision census size row"))?;
    let mode = row
        .get::<_, i64>(3)
        .map_err(|_| invalid("fresh revision census mode row"))?;
    Ok(ScratchMember {
        path,
        sha256: Digest256::from_bytes(digest),
        size: u64::try_from(size).map_err(|_| invalid("fresh revision census size range"))?,
        mode: u32::try_from(mode).map_err(|_| invalid("fresh revision census mode range"))?,
    })
}

fn merge_census_with_base(
    db: &PinnedSqliteConnection,
    scan: SourceCensusScan,
    summary: SourceCensusSummary,
    base: &mut V2ReadSession,
    original_base: SourceRevision,
    roots: &SourceRevisionRootsV2,
    expected: &mut BTreeMap<String, ExpectedChange>,
    work: &mut CensusWork,
    profile: FreshRevisionProfile,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<BTreeMap<String, SourceUpdate>> {
    if roots.revision != original_base
        || roots.member_count != roots.membership_v1.count
        || roots.member_count != roots.members.entries
        || roots.source_bytes > profile.admission.max_source_bytes
    {
        return Err(invalid("fresh revision V2 base root counts differ"));
    }
    work.charge_many(2)?;
    let mut statement = db
        .prepare(
            "SELECT path,sha256,size,mode FROM source_member_census \
             WHERE scan_label=?1 ORDER BY path COLLATE BINARY",
        )
        .map_err(|_| invalid("fresh revision census ordered query prepare"))?;
    let mut rows = statement
        .query([scan.label()])
        .map_err(|_| invalid("fresh revision census ordered query"))?;
    let mut prior: Option<V2MemberTupleObservation> = None;
    let mut updates = BTreeMap::<String, SourceUpdate>::new();
    let mut base_members = 0u64;
    let mut base_bytes = 0u64;
    let mut actual_source_bytes = 0u64;
    loop {
        active_io(deadline, cancel)?;
        work.charge(SourceCensusWorkKind::SqlRow)?;
        let actual = rows
            .next()
            .map_err(|_| invalid("fresh revision census row read"))?;
        let actual = actual.map(read_scratch_member).transpose()?;
        let after = prior.as_ref().map(|member| member.path.as_str().as_bytes());
        work.charge(SourceCensusWorkKind::Entry)?;
        let caller_state = profile
            .cursor_caller_state_bytes
            .checked_add(after.map_or(0, <[u8]>::len))
            .and_then(|n| n.checked_add(actual.as_ref().map_or(0, |row| row.path.len())))
            .ok_or_else(|| invalid("fresh revision cursor caller state overflow"))?;
        let authenticated = base
            .next_member_tuple_after(original_base, after, caller_state)
            .map_err(invalid)?;
        match (actual, authenticated) {
            (None, None) => break,
            (Some(_), None) | (None, Some(_)) => {
                return Err(invalid(
                    "fresh revision full selected census membership differs",
                ));
            }
            (Some(actual), Some(authenticated)) => {
                if actual.path != authenticated.path.as_str()
                    || actual.mode != authenticated.source_mode
                    || !matches!(actual.mode, 0o600 | 0o644 | 0o755)
                {
                    return Err(invalid("fresh revision full selected census tuple differs"));
                }
                base_members = base_members
                    .checked_add(1)
                    .ok_or_else(|| invalid("fresh revision base member count overflow"))?;
                base_bytes = base_bytes
                    .checked_add(authenticated.size_bytes)
                    .ok_or_else(|| invalid("fresh revision base byte count overflow"))?;
                actual_source_bytes = actual_source_bytes
                    .checked_add(actual.size)
                    .ok_or_else(|| invalid("fresh revision census byte count overflow"))?;
                if let Some(change) = expected.get_mut(&actual.path) {
                    if change.seen
                        || authenticated.sha256 != change.before_sha256
                        || authenticated.size_bytes != change.before_size
                        || actual.sha256 != change.after_sha256
                        || actual.size != change.after_size
                    {
                        return Err(invalid("fresh revision planned member sides differ"));
                    }
                    change.seen = true;
                    if actual.sha256 != authenticated.sha256
                        || actual.size != authenticated.size_bytes
                    {
                        updates.insert(
                            actual.path.clone(),
                            SourceUpdate {
                                sha256: actual.sha256,
                                size_bytes: actual.size,
                                mode: actual.mode,
                            },
                        );
                    }
                } else if actual.sha256 != authenticated.sha256
                    || actual.size != authenticated.size_bytes
                {
                    return Err(invalid(
                        "fresh revision undeclared selected-source difference",
                    ));
                }
                prior = Some(authenticated);
            }
        }
    }
    drop(rows);
    drop(statement);
    if base_members != roots.member_count
        || base_members != summary.member_count
        || base_bytes != roots.source_bytes
        || actual_source_bytes != summary.source_bytes
        || expected.values().any(|change| !change.seen)
        || updates.is_empty()
        || updates.len() > profile.admission.max_members
    {
        return Err(invalid("fresh revision census/base closure totals differ"));
    }
    Ok(updates)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_record_revision<'a>(
    fs: &'a CreationFilesystem,
    invocation: &NativeSourceValidator<'_>,
    base: &mut V2ReadSession,
    original_base: SourceRevision,
    transaction_id: &str,
    expected_record_id: &str,
    profile: FreshRevisionProfile,
    workspace: File,
    aux: PinnedSqliteAuxRequest,
    validator_sha256: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<FreshRevisionPrepared<'a>> {
    state_partition(profile)?;
    active_io(deadline, cancel)?;
    let retained_cancel = Arc::clone(&aux.cancelled);
    if transaction_id.is_empty()
        || transaction_id.len() > 80
        || !transaction_id.starts_with("sha256:")
        || expected_record_id.is_empty()
        || expected_record_id.len() > 1024
        || aux.deadline != deadline
        || !std::ptr::eq(Arc::as_ptr(&aux.cancelled), cancel)
    {
        return Err(invalid(
            "fresh revision caller custody or original ledger differs",
        ));
    }
    // Spool/source inspection and the authenticated V2 base each retain their
    // independently selected original ledger. The protected Native owner is
    // the only place that can establish this exact pair for the invocation.
    invocation.verify_spooled_v2_io(&aux.io_budget, base.io_budget(), deadline, cancel)?;
    let io = aux.io_budget.clone();
    charge_read_upper(&io, 65_536)?;
    let mut work = CensusWork::new(profile.max_work_units)?;
    work.charge_many(8)?;
    let work_fence = WorkCorpusFence::hold_existing(fs, deadline, cancel).map_err(work_error)?;
    let facts = inspect_work_facts(
        fs,
        transaction_id,
        expected_record_id,
        original_base,
        profile,
        &io,
        deadline,
        cancel,
        &mut work,
    )?;
    let WorkFacts {
        manifest_sha256,
        archive,
        expected: mut expected,
    } = facts;
    let disposition = if base.selected_revision() == original_base {
        FreshRevisionDisposition::NewPublication
    } else {
        FreshRevisionDisposition::RetryLookupOnly
    };
    let mut scope = PinnedSqliteAuxScope::new(workspace, aux)
        .map_err(|_| invalid("fresh revision held census scratch selection refused"))?;
    let db = scope
        .open_connection()
        .map_err(|_| invalid("fresh revision held census scratch connection refused"))?;
    configure_db(&db, profile.sqlite_cache_bytes)?;
    let progress_deadline = deadline;
    let progress_cancel = Arc::clone(&retained_cancel);
    db.progress_handler(
        1000,
        Some(move || active_io(progress_deadline, &progress_cancel).is_err()),
    );
    let mut callback = |kind| work.charge(kind);
    let proposal_census = if disposition == FreshRevisionDisposition::NewPublication {
        census_selected_to_scratch(
            fs.protected_root_file(),
            &db,
            &io,
            SourceCensusScan::Proposal,
            profile.census,
            &mut callback,
            deadline,
            cancel,
        )?
    } else {
        // Retry matching is history-only. The committed owner plan is checked
        // against the original immutable base below; a later live tree is not
        // promoted into a new candidate cut or compared as fresh admission.
        SourceCensusSummary {
            member_count: 0,
            source_bytes: 0,
            directory_count: 0,
            entry_count: 0,
            payload_read_bytes: 0,
            metadata_read_upper_bytes: 0,
            sql_row_operations: 0,
            digest: Digest256::from_bytes([0; 32]),
        }
    };
    drop(callback);
    let roots = base
        .roots_for_revision(original_base)
        .map_err(invalid)?
        .ok_or_else(|| invalid("fresh revision exact original V2 base absent"))?;
    let updates = if disposition == FreshRevisionDisposition::NewPublication {
        merge_census_with_base(
            &db,
            SourceCensusScan::Proposal,
            proposal_census,
            base,
            original_base,
            &roots,
            &mut expected,
            &mut work,
            profile,
            deadline,
            cancel,
        )?
    } else {
        updates_from_work_base(
            &mut expected,
            &mut work,
            profile,
            &mut *base,
            original_base,
            &roots,
            deadline,
            cancel,
        )?
    };
    drop(expected);
    let batch = AdmissionBatch::from_verified_rows(
        Some(original_base.0),
        validator_sha256,
        updates,
        fs.protected_root_file()
            .try_clone()
            .map_err(|_| invalid("fresh revision selected source root clone"))?,
        profile.admission,
        profile.batch_builder_state_bytes,
        &io,
        deadline,
        cancel,
    )?;
    Ok(FreshRevisionPrepared {
        batch: Some(batch),
        disposition,
        fence: FreshRevisionFence {
            fs,
            work_fence,
            archive,
            transaction_id: transaction_id.to_owned(),
            expected_record_id: expected_record_id.to_owned(),
            manifest_sha256,
            original_base,
            selected_revision: base.selected_revision(),
            proposal_census,
            profile,
            _scope: scope,
            db,
            io,
            deadline,
            cancel: retained_cancel,
            work,
        },
    })
}

fn updates_from_work_base(
    expected: &mut BTreeMap<String, ExpectedChange>,
    work: &mut CensusWork,
    profile: FreshRevisionProfile,
    base: &mut V2ReadSession,
    original_base: SourceRevision,
    roots: &SourceRevisionRootsV2,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<BTreeMap<String, SourceUpdate>> {
    if roots.revision != original_base
        || roots.member_count != roots.membership_v1.count
        || roots.member_count != roots.members.entries
    {
        return Err(invalid("fresh retry V2 base root counts differ"));
    }
    let mut result = BTreeMap::new();
    let mut previous: Option<V2MemberTupleObservation> = None;
    let mut count = 0u64;
    let mut bytes = 0u64;
    loop {
        active_io(deadline, cancel)?;
        work.charge(SourceCensusWorkKind::Entry)?;
        let after = previous
            .as_ref()
            .map(|member| member.path.as_str().as_bytes());
        let member = base
            .next_member_tuple_after(original_base, after, profile.cursor_caller_state_bytes)
            .map_err(invalid)?;
        let Some(member) = member else { break };
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid("fresh retry base member count overflow"))?;
        bytes = bytes
            .checked_add(member.size_bytes)
            .ok_or_else(|| invalid("fresh retry base byte count overflow"))?;
        if let Some(change) = expected.get_mut(member.path.as_str()) {
            if change.seen
                || member.sha256 != change.before_sha256
                || member.size_bytes != change.before_size
            {
                return Err(invalid(
                    "fresh retry original Work predecessor differs from V2 base",
                ));
            }
            change.seen = true;
            if change.after_sha256 != member.sha256 || change.after_size != member.size_bytes {
                result.insert(
                    member.path.as_str().to_owned(),
                    SourceUpdate {
                        sha256: change.after_sha256,
                        size_bytes: change.after_size,
                        mode: member.source_mode,
                    },
                );
            }
        }
        previous = Some(member);
    }
    if count != roots.member_count
        || bytes != roots.source_bytes
        || expected.values().any(|change| !change.seen)
        || result.is_empty()
        || result.len() > profile.admission.max_members
    {
        return Err(invalid("fresh retry original V2 base closure differs"));
    }
    Ok(result)
}

impl FreshRevisionFence<'_> {
    fn verify_common(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        active_io(self.deadline, &self.cancel)?;
        // The lock/name fence has a distinct finite metadata envelope. The
        // owner/configuration and retained Work payload windows are debited
        // inside `inspect_work_facts` before those reads occur.
        charge_read_upper(&self.io, 65_536)?;
        self.work.charge_many(8)?;
        self.work_fence
            .verify(self.deadline, &self.cancel)
            .map_err(work_error)?;
        let facts = inspect_work_facts(
            self.fs,
            &self.transaction_id,
            &self.expected_record_id,
            self.original_base,
            self.profile,
            &self.io,
            self.deadline,
            &self.cancel,
            &mut self.work,
        )?;
        if facts.manifest_sha256 != self.manifest_sha256 || facts.archive.path != self.archive.path
        {
            return Err(invalid("fresh revision committed Work binding changed"));
        }
        if base.selected_revision() != self.selected_revision {
            return Err(invalid("fresh revision selected V2 pointer changed"));
        }
        Ok(())
    }

    fn verify_before_publish(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        self.verify_common(base)?;
        if self.selected_revision != self.original_base {
            return Err(invalid("fresh revision original V2 base no longer current"));
        }
        let mut callback = |kind| self.work.charge(kind);
        let terminal = census_selected_to_scratch(
            self.fs.protected_root_file(),
            &self.db,
            &self.io,
            SourceCensusScan::Terminal,
            self.profile.census,
            &mut callback,
            self.deadline,
            &self.cancel,
        )?;
        if terminal != self.proposal_census {
            return Err(invalid("fresh revision terminal source census changed"));
        }
        // Fence the named root/configuration after the full held-FD walk as
        // well as before it; otherwise the walk could finish on a detached
        // input root while the pointer check succeeds.
        charge_read_upper(&self.io, owner_configuration_upper(self.fs)?)?;
        self.fs
            .verify_selected_configuration(self.deadline, &self.cancel)
            .map_err(work_error)?;
        charge_read_upper(&self.io, 65_536)?;
        self.work.charge_many(8)?;
        self.work_fence
            .verify(self.deadline, &self.cancel)
            .map_err(work_error)?;
        base.verify_current_fence().map_err(invalid)
    }

    fn verify_retry_outcome(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        self.verify_common(base)?;
        base.verify_current_fence().map_err(invalid)
    }
}
