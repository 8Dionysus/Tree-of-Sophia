//! Explicit, read-only V2 source selection for one protected native
//! invocation. The profile is a finite resource envelope, not a source grant.

use crate::source_admission::AdmissionWorkBudget;
use crate::source_admission_segment_v2::{
    SourceRevisionRootsV2, SourceRootSetV2, decode_workspace_upper_bound,
};
use crate::source_admission_v2_reader::{
    V2CurrentSelectionObservation, V2PointReadLimits, V2ReadSession, V2RootKind,
};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_revisions::ReadonlyRecordFiles;
use serde_json::Value;
use std::fs::File;
use std::io;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, RelativePath, SourceRevision};
use tos_segment_store::{AuthenticatedTreeLimitsV1, SegmentLimits};
use tos_source_store::{
    MemberMetadata, PinnedSqliteIoBudget, ReadLimits, RetiredSourceMemberV1, RetirementMetadata,
    SoftwareCaptureReader, SoftwareComponentSelectionV1, SourceCutDependencyWitness,
    SourceCutDirectoryWitness, SourceCutFormat, SourceCutIdentityWitness, SourceCutMemberStream,
    SourceCutMemberTuple, SourceCutMemberWitness, SourceCutMembership, SourceCutMetadataStream,
    SourceCutObjectRefcountWitness, SourceCutRead, SourceCutReadsetV1, SourceCutRevision,
    SourceCutRevisionStream, SourceCutSelection, SourceMemberV1, SourceMembershipV1,
    SourcePresenceV1, StoreError, StoreErrorCode, feed_source_membership_v1,
};

const ROOTSET_BYTES: usize = SourceRevisionRootsV2::MAX_ENCODED_BYTES;
const ROOT_GUARD_BYTES: u64 = 65_537;
const MAX_V2_SCHEMA_OBJECT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TREE_KEY_BYTES: usize = 4096;
const MAX_TREE_KIND_BYTES: usize = 64;
const MAX_TREE_CHILDREN: usize = 16;

/// A selected finite source-read envelope. It has no admission or publication
/// authority and is deliberately unavailable to V1 readers.
#[derive(Clone, Copy, Debug)]
pub(super) struct SourceV2ReadProfile {
    pub(super) max_revisions: usize,
    pub(super) max_members: u64,
    pub(super) max_total_bytes: u64,
    pub(super) max_member_bytes: u64,
    pub(super) max_read_io_bytes: u64,
    pub(super) max_state_bytes: usize,
    pub(super) segment: SegmentLimits,
    pub(super) tree: AuthenticatedTreeLimitsV1,
    pub(super) max_object_bytes: usize,
    pub(super) max_work_units: u64,
}

impl SourceV2ReadProfile {
    pub(super) fn parse(value: &Value, selected_budgets: &Value) -> SourceCommandResult<Self> {
        super::exact(
            value,
            &[
                "schema_version",
                "max_read_io_bytes",
                "max_state_bytes",
                "max_segment_bytes",
                "max_frame_bytes",
                "max_frames",
                "max_tree_nodes",
                "max_tree_bytes",
                "max_tree_node_bytes",
                "max_tree_rows",
                "max_object_bytes",
                "max_work_units",
            ],
        )?;
        if super::text(value, "schema_version")? != "tos_native_source_v2_read_profile_v1" {
            return Err(SourceCommandError::Invalid(
                "native V2 source profile version",
            ));
        }
        let max_read_io_bytes = super::number(value, "max_read_io_bytes")?;
        let max_state_bytes = usize::try_from(super::number(value, "max_state_bytes")?)
            .map_err(|_| SourceCommandError::Invalid("native V2 source state range"))?;
        let max_segment_bytes = super::number(value, "max_segment_bytes")?;
        let max_frame_bytes = super::number(value, "max_frame_bytes")?;
        let max_frames = u32::try_from(super::number(value, "max_frames")?)
            .map_err(|_| SourceCommandError::Invalid("native V2 source frame count"))?;
        let max_tree_nodes = super::number(value, "max_tree_nodes")?;
        let max_tree_bytes = super::number(value, "max_tree_bytes")?;
        let max_tree_node_bytes = usize::try_from(super::number(value, "max_tree_node_bytes")?)
            .map_err(|_| SourceCommandError::Invalid("native V2 source tree node range"))?;
        let max_tree_rows = super::number(value, "max_tree_rows")?;
        let max_object_bytes = usize::try_from(super::number(value, "max_object_bytes")?)
            .map_err(|_| SourceCommandError::Invalid("native V2 source object range"))?;
        let max_work_units = super::number(value, "max_work_units")?;
        let selected_max_members = super::capped(selected_budgets, "max_members", 2048)?;
        let selected_max_source_bytes =
            super::capped(selected_budgets, "max_total_bytes", 33_554_432)?;
        let selected_max_member_bytes =
            super::capped(selected_budgets, "max_member_bytes", 8_388_608)?;
        let selected_max_revisions = super::capped(selected_budgets, "max_revisions", 4)?;

        if max_state_bytes == 0
            || max_state_bytes == usize::MAX
            || max_read_io_bytes == u64::MAX
            || max_segment_bytes <= 48
            || max_segment_bytes > max_read_io_bytes
            || max_frame_bytes == 0
            || max_frame_bytes > max_segment_bytes
            || max_tree_nodes == u64::MAX
            || max_tree_bytes == 0
            || max_tree_bytes > max_read_io_bytes
            || max_tree_node_bytes < 128
            || u64::try_from(max_tree_node_bytes).map_or(true, |bytes| bytes > max_frame_bytes)
            || max_tree_rows == 0
            || max_tree_rows > selected_max_members
            || max_object_bytes == 0
            || u64::try_from(max_object_bytes).map_or(true, |bytes| {
                bytes > selected_max_member_bytes || bytes > MAX_V2_SCHEMA_OBJECT_BYTES
            })
            || max_object_bytes as u64 > max_frame_bytes
            || max_frames == 0
            || max_state_bytes / 8 < 128
            || max_work_units == 0
            || max_work_units == u64::MAX
        {
            return Err(SourceCommandError::Invalid(
                "native V2 source profile exceeds the selected operation envelope",
            ));
        }
        // The authenticated tree and source member count share the original
        // selected logical cut ceiling; source bytes remain separately capped
        // by the same invocation's existing logical budget.
        if max_tree_rows > selected_max_members || selected_max_source_bytes == 0 {
            return Err(SourceCommandError::Invalid(
                "native V2 source logical cut cap differs",
            ));
        }
        let segment = SegmentLimits {
            max_segment_bytes,
            max_frame_bytes,
            max_frames,
            // SegmentLimits requires a finite journal bound even for this
            // reader. Derive it from the already-selected state slice; never
            // raise a too-small profile to its structural minimum.
            max_journal_bytes: max_state_bytes / 8,
        };
        segment
            .validate()
            .map_err(|_| SourceCommandError::Invalid("native V2 source segment profile"))?;
        let tree = AuthenticatedTreeLimitsV1 {
            max_key_bytes: MAX_TREE_KEY_BYTES,
            max_value_bytes: ROOTSET_BYTES,
            max_kind_bytes: MAX_TREE_KIND_BYTES,
            max_node_bytes: max_tree_node_bytes,
            max_children: MAX_TREE_CHILDREN,
            max_nodes: max_tree_nodes,
            max_total_bytes: max_tree_bytes,
            max_rows: max_tree_rows,
        };
        Ok(Self {
            max_revisions: usize::try_from(selected_max_revisions)
                .map_err(|_| SourceCommandError::Invalid("native V2 revision count range"))?,
            max_members: selected_max_members,
            max_total_bytes: selected_max_source_bytes,
            max_member_bytes: selected_max_member_bytes,
            max_read_io_bytes,
            max_state_bytes,
            segment,
            tree,
            max_object_bytes,
            max_work_units,
        })
    }

    pub(super) fn point_limits(self, caller_retained_state_bytes: usize) -> V2PointReadLimits {
        V2PointReadLimits {
            pointer: ReadLimits {
                max_manifest_bytes: 4096,
                max_manifest_entries: 64,
                max_selected_object_bytes: self.max_object_bytes as u64,
                json: JsonLimits {
                    max_bytes: 4096,
                    max_depth: 8,
                    max_visits: 64,
                    max_integer_digits: 64,
                },
            },
            segment: self.segment,
            tree: self.tree,
            max_object_bytes: self.max_object_bytes,
            max_state_bytes: self.max_state_bytes,
            caller_retained_state_bytes,
        }
    }
}

/// One held V2 current selection and its original read-only cumulative ledger.
/// All pointer, tree, history and member reads go through this same session.
pub(super) struct NativeSourceV2Session {
    _held_root: File,
    path: PathBuf,
    reader: V2ReadSession,
    opened_selection: SourceCutSelection,
    io: PinnedSqliteIoBudget,
    work: AdmissionWorkBudget,
    cancelled: Arc<AtomicBool>,
    profile: SourceV2ReadProfile,
    caller_retained_state_bytes: usize,
}

impl NativeSourceV2Session {
    pub(super) fn open(
        path: &Path,
        expected_current: SourceRevision,
        expected_rootset_sha256: Digest256,
        profile: SourceV2ReadProfile,
        work: AdmissionWorkBudget,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> SourceCommandResult<Self> {
        let path_len = path.as_os_str().as_encoded_bytes().len();
        let root_guard = u64::try_from(path_len)
            .ok()
            .and_then(|bytes| bytes.checked_add(ROOT_GUARD_BYTES))
            .ok_or(SourceCommandError::Invalid(
                "native V2 source root guard overflow",
            ))?;
        if !path.is_absolute()
            || path_len > 4096
            || path.components().count() > 128
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(SourceCommandError::Invalid("native V2 source root path"));
        }
        let caller_state = caller_retained_state_bytes
            .checked_add(size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(size_of::<V2ReadSession>()))
            .and_then(|bytes| bytes.checked_add(path_len.checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(8192))
            .ok_or(SourceCommandError::Invalid(
                "native V2 caller state overflow",
            ))?;
        let limits = profile.point_limits(caller_state);
        let preflight = point_base_state_upper_bound(limits)
            .and_then(|bytes| bytes.checked_add(caller_state))
            .ok_or(SourceCommandError::Invalid(
                "native V2 point state overflow",
            ))?;
        if preflight > profile.max_state_bytes {
            return Err(SourceCommandError::Invalid(
                "native V2 source whole-operation state profile is too small",
            ));
        }
        let io = PinnedSqliteIoBudget::new_read_only(profile.max_read_io_bytes)
            .map_err(|_| SourceCommandError::Invalid("native V2 source read profile"))?;
        io.charge_read_upper_bound(root_guard)
            .map_err(|_| SourceCommandError::Conflict("native V2 source root guard budget"))?;
        let held_root = tos_fd_open::open_absolute_directory(path)
            .map_err(|_| SourceCommandError::Conflict("native V2 source root"))?;
        let mut reader = V2ReadSession::open_at_named_with_work(
            path,
            &held_root,
            limits,
            io.clone(),
            work.clone(),
            deadline,
            cancelled.clone(),
        )
        .map_err(|_| SourceCommandError::Conflict("native V2 source selection"))?;
        if reader.selected_revision() != expected_current {
            return Err(SourceCommandError::Conflict(
                "native V2 selected source revision changed",
            ));
        }
        if reader.selected_selection().rootset_sha256 != Some(expected_rootset_sha256) {
            return Err(SourceCommandError::Conflict(
                "native V2 selected rootset changed",
            ));
        }
        let opened = reader.selected_selection();
        let (retained, _) = reader
            .declared_retained_state_bytes()
            .map_err(|_| SourceCommandError::Invalid("native V2 reader retained state"))?;
        if retained
            .checked_add(caller_state)
            .is_none_or(|bytes| bytes > profile.max_state_bytes)
        {
            return Err(SourceCommandError::Invalid(
                "native V2 source whole-operation state allowance exceeded",
            ));
        }
        let current_roots = reader
            .roots_for_revision_with_caller_state(expected_current, caller_state)
            .map_err(|_| SourceCommandError::Conflict("native V2 current source roots"))?
            .ok_or(SourceCommandError::Conflict(
                "native V2 current source revision absent",
            ))?;
        if current_roots.identity_paths.is_none() {
            return Err(SourceCommandError::Unsupported(
                "native V2 source revision lacks exact path identity coverage",
            ));
        }
        Ok(Self {
            _held_root: held_root,
            path: path.to_path_buf(),
            reader,
            opened_selection: SourceCutSelection {
                format: SourceCutFormat::NativeAdmissionV2,
                current_revision: opened.revision,
                rootset_sha256: opened.rootset_sha256,
            },
            io,
            work,
            cancelled,
            profile,
            caller_retained_state_bytes: caller_state,
        })
    }

    pub(super) fn selected_revision(&self) -> SourceRevision {
        self.reader.selected_revision()
    }

    pub(super) fn opened_selection(&self) -> SourceCutSelection {
        self.opened_selection
    }

    pub(super) fn reader_mut(&mut self) -> &mut V2ReadSession {
        &mut self.reader
    }

    pub(super) fn io_budget(&self) -> &PinnedSqliteIoBudget {
        &self.io
    }

    pub(super) fn work_budget(&self) -> AdmissionWorkBudget {
        self.work.clone()
    }

    pub(super) fn profile(&self) -> SourceV2ReadProfile {
        self.profile
    }

    pub(super) fn caller_retained_state_bytes(&self) -> usize {
        self.caller_retained_state_bytes
    }

    pub(super) fn ensure_state_capacity(&self, additional: usize) -> SourceCommandResult<()> {
        ensure_reader_state(
            &self.reader,
            self.caller_retained_state_bytes,
            self.profile.max_state_bytes,
            additional,
        )
        .map_err(|_| {
            SourceCommandError::Invalid("native V2 source cursor state allowance exceeded")
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn verify_current_fence(&self) -> SourceCommandResult<()> {
        self.reader
            .verify_current_fence()
            .map_err(|_| SourceCommandError::Conflict("native V2 source current fence"))
    }
}

/// SourceEntry's point-only projection over one authenticated V2 selection.
/// It accumulates the exact read predicates that must be rechecked before a
/// caller can hand a proposal to its owning transaction route.
pub(super) struct NativeSourceV2RecordFiles<'a> {
    session: &'a mut NativeSourceV2Session,
    revision: SourceRevision,
    retained_file_state_bytes: usize,
    payload_read_bytes: u64,
    readset: SourceCutReadsetV1,
}

impl<'a> NativeSourceV2RecordFiles<'a> {
    pub(super) fn new(
        session: &'a mut NativeSourceV2Session,
        revision: SourceRevision,
    ) -> SourceCommandResult<Self> {
        let mut files = Self {
            session,
            revision,
            retained_file_state_bytes: 0,
            payload_read_bytes: 0,
            readset: SourceCutReadsetV1::new(revision),
        };
        files.ensure_state(0, 0)?;
        Ok(files)
    }

    fn readset_state(&self) -> SourceCommandResult<usize> {
        source_cut_readset_state_upper_bound(
            &self.readset.members,
            &self.readset.identities,
            &self.readset.directories,
            &self.readset.dependencies,
            &self.readset.object_refcounts,
        )
        .map_err(|_| SourceCommandError::Invalid("native V2 record readset state"))
    }

    fn live_state(&self) -> SourceCommandResult<usize> {
        size_of::<Self>()
            .checked_add(self.retained_file_state_bytes)
            .and_then(|bytes| bytes.checked_add(self.readset_state().ok()?))
            .ok_or(SourceCommandError::Invalid(
                "native V2 record live state overflow",
            ))
    }

    fn readset_caller_state(&self) -> SourceCommandResult<usize> {
        size_of::<Self>()
            .checked_add(self.retained_file_state_bytes)
            .ok_or(SourceCommandError::Invalid(
                "native V2 record live state overflow",
            ))
    }

    fn ensure_state(
        &self,
        transient_bytes: usize,
        additional_bytes: usize,
    ) -> SourceCommandResult<()> {
        let state = self
            .live_state()?
            .checked_add(transient_bytes)
            .and_then(|bytes| bytes.checked_add(additional_bytes))
            .ok_or(SourceCommandError::Invalid(
                "native V2 record live state overflow",
            ))?;
        self.session.ensure_state_capacity(state)
    }

    fn identity_state_bound(id: &str, path: &RelativePath) -> SourceCommandResult<usize> {
        size_of::<SourceCutIdentityWitness>()
            .checked_add(id.len().saturating_mul(2))
            .and_then(|bytes| bytes.checked_add(path.as_str().len().saturating_mul(4)))
            .ok_or(SourceCommandError::Invalid(
                "native V2 identity witness state overflow",
            ))
    }

    fn member_state_bound(
        path: &RelativePath,
        indexed_ids: &[String],
    ) -> SourceCommandResult<usize> {
        let mut bytes = size_of::<SourceCutMemberWitness>()
            .checked_add(path.as_str().len().saturating_mul(4))
            .and_then(|bytes| {
                bytes.checked_add(indexed_ids.len().checked_mul(size_of::<String>())?)
            })
            .ok_or(SourceCommandError::Invalid(
                "native V2 member witness state overflow",
            ))?;
        for id in indexed_ids {
            bytes = bytes
                .checked_add(id.capacity())
                .ok_or(SourceCommandError::Invalid(
                    "native V2 member witness state overflow",
                ))?;
        }
        Ok(bytes)
    }

    fn capture_file_witness(
        &mut self,
        path: &RelativePath,
        member: Option<MemberMetadata>,
        indexed_ids: Option<Vec<String>>,
        presence: Option<SourcePresenceV1>,
        transient_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let indexed_ids = indexed_ids.unwrap_or_default();
        let expected_member = member.as_ref().map(|member| SourceCutMemberTuple {
            sha256: member.sha256,
            size_bytes: member.size_bytes,
            mode: member.mode,
        });
        let member_bound = Self::member_state_bound(path, &indexed_ids)?;
        let indexed_ids_state = indexed_ids
            .capacity()
            .checked_mul(size_of::<String>())
            .and_then(|bytes| {
                indexed_ids
                    .iter()
                    .try_fold(bytes, |total, id| total.checked_add(id.capacity()))
            })
            .ok_or(SourceCommandError::Invalid(
                "native V2 identity result state overflow",
            ))?;
        let identity_bounds = indexed_ids.iter().try_fold(0usize, |total, id| {
            total
                .checked_add(Self::identity_state_bound(id, path)?)
                .ok_or(SourceCommandError::Invalid(
                    "native V2 identity witness state overflow",
                ))
        })?;
        self.ensure_state(
            transient_bytes,
            member_bound
                .checked_add(indexed_ids_state)
                .and_then(|bytes| bytes.checked_add(identity_bounds))
                .ok_or(SourceCommandError::Invalid(
                    "native V2 readset state overflow",
                ))?,
        )?;
        if let Some(prior) = self
            .readset
            .members
            .iter()
            .find(|witness| witness.path == *path)
        {
            if prior.expected_presence != presence
                || prior.expected_member != expected_member
                || prior.expected_indexed_ids.as_deref()
                    != member.as_ref().map(|_| indexed_ids.as_slice())
            {
                return Err(SourceCommandError::Conflict(
                    "native V2 selected path changed within invocation",
                ));
            }
        } else {
            if self.readset.members.len() as u64 >= self.session.profile.max_members {
                return Err(SourceCommandError::Unsupported(
                    "native V2 record readset member budget",
                ));
            }
            self.readset
                .members
                .try_reserve_exact(1)
                .map_err(|_| SourceCommandError::Invalid("native V2 member witness allocation"))?;
            self.readset.members.push(SourceCutMemberWitness {
                path: path.clone(),
                expected_presence: presence,
                expected_member,
                expected_indexed_ids: member.as_ref().map(|_| indexed_ids.clone()),
            });
        }
        for id in indexed_ids {
            let observed_path = self
                .session
                .identity_path(self.revision, &id, self.live_state()?, deadline, cancelled)
                .map_err(|_| SourceCommandError::Conflict("native V2 source identity lookup"))?;
            if observed_path.as_ref() != Some(path) {
                return Err(SourceCommandError::Conflict(
                    "native V2 path identity inverse differs",
                ));
            }
            let witness = SourceCutIdentityWitness {
                id: id.clone(),
                expected_path: Some(path.clone()),
            };
            if let Some(prior) = self
                .readset
                .identities
                .iter()
                .find(|prior| prior.id == witness.id)
            {
                if prior != &witness {
                    return Err(SourceCommandError::Conflict(
                        "native V2 identity path changed within invocation",
                    ));
                }
            } else {
                self.ensure_state(transient_bytes, Self::identity_state_bound(&id, path)?)?;
                self.readset.identities.try_reserve_exact(1).map_err(|_| {
                    SourceCommandError::Invalid("native V2 identity witness allocation")
                })?;
                self.readset.identities.push(witness);
            }
        }
        self.ensure_state(transient_bytes, 0)
    }

    /// Retain only a genuine provider observation. Repeated identical reads
    /// deduplicate; contradictory counts refuse within the same invocation.
    pub(super) fn object_refcount(
        &mut self,
        digest: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<u64>> {
        let count = self
            .session
            .object_refcount(
                self.revision,
                digest,
                self.live_state()?,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Unsupported(
                    "native V2 authenticated object refcount unavailable",
                )
            })?;
        if count == Some(0) {
            return Err(SourceCommandError::Conflict(
                "native V2 zero object refcount is not absence",
            ));
        }
        let witness = SourceCutObjectRefcountWitness {
            digest,
            expected_count: count,
        };
        if let Some(prior) = self
            .readset
            .object_refcounts
            .iter()
            .find(|prior| prior.digest == digest)
        {
            if prior != &witness {
                return Err(SourceCommandError::Conflict(
                    "native V2 object refcount changed within invocation",
                ));
            }
        } else {
            self.ensure_state(0, size_of::<SourceCutObjectRefcountWitness>())?;
            self.readset
                .object_refcounts
                .try_reserve_exact(1)
                .map_err(|_| {
                    SourceCommandError::Invalid("native V2 object refcount witness allocation")
                })?;
            self.readset.object_refcounts.push(witness);
            self.readset
                .object_refcounts
                .sort_by_key(|entry| entry.digest);
        }
        self.ensure_state(0, 0)?;
        Ok(count)
    }

    pub(super) fn verify_readset_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<SourceCutSelection> {
        self.session
            .verify_readset_current(
                self.readset.base_revision,
                &self.readset.members,
                &self.readset.identities,
                &self.readset.directories,
                &self.readset.dependencies,
                &self.readset.object_refcounts,
                self.readset_caller_state()?,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("native V2 source readset changed"))
    }

    pub(super) fn readset(&self) -> &SourceCutReadsetV1 {
        &self.readset
    }
}

impl ReadonlyRecordFiles for NativeSourceV2RecordFiles<'_> {
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SourceCommandError::Unsupported(
                "native V2 record read deadline or cancellation",
            ));
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("native V2 selected record path"))?;
        let caller_state = self.live_state()?;
        let metadata = self
            .session
            .member(self.revision, &relative, caller_state, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("native V2 selected record member"))?
            .ok_or(SourceCommandError::Conflict(
                "native V2 selected record member is absent",
            ))?;
        let cap = u64::try_from(max_bytes)
            .map_err(|_| SourceCommandError::Invalid("native V2 record read cap"))?;
        if metadata.size_bytes > cap {
            return Err(SourceCommandError::Unsupported(
                "native V2 selected record member exceeds read cap",
            ));
        }
        let next_source_bytes = self
            .payload_read_bytes
            .checked_add(metadata.size_bytes)
            .ok_or(SourceCommandError::Unsupported(
                "native V2 selected source byte budget",
            ))?;
        if next_source_bytes > self.session.profile.max_total_bytes {
            return Err(SourceCommandError::Unsupported(
                "native V2 selected source byte budget",
            ));
        }
        let member = self
            .session
            .read_member(
                self.revision,
                &relative,
                cap,
                self.live_state()?,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("native V2 selected record bytes"))?;
        if member.raw.len() as u64 != metadata.size_bytes
            || Digest256::of_bytes(&member.raw) != metadata.sha256
            || member.path != relative
            || member.revision != self.revision
        {
            return Err(SourceCommandError::Conflict(
                "native V2 selected record bytes differ from member index",
            ));
        }
        let output_bytes = member.raw.len();
        let ids = member.stable_ids;
        self.capture_file_witness(
            &relative,
            Some(metadata),
            Some(ids),
            Some(SourcePresenceV1::File),
            output_bytes,
            deadline,
            cancelled,
        )?;
        self.payload_read_bytes = next_source_bytes;
        Ok(member.raw)
    }

    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        let relative = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("native V2 selected directory path"))?;
        let caller_state = self.live_state()?;
        let presence = self
            .session
            .presence(self.revision, &relative, caller_state, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("native V2 selected directory presence"))?;
        let max_entries = usize::try_from(self.session.profile.tree.max_rows)
            .map_err(|_| SourceCommandError::Invalid("native V2 directory entry range"))?;
        let children = self
            .session
            .list_directory(
                self.revision,
                &relative,
                max_entries,
                self.live_state()?,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("native V2 selected directory children"))?
            .unwrap_or_default();
        if presence == Some(SourcePresenceV1::File) && !children.is_empty() {
            return Err(SourceCommandError::Conflict(
                "native V2 source path is both file and directory",
            ));
        }
        if let Some(prior) = self
            .readset
            .directories
            .iter()
            .find(|prior| prior.path == relative)
        {
            if prior.expected_presence != presence || prior.expected_children != children {
                return Err(SourceCommandError::Conflict(
                    "native V2 directory changed within invocation",
                ));
            }
        } else {
            let bytes = children.iter().try_fold(
                size_of::<SourceCutDirectoryWitness>()
                    .checked_add(relative.as_str().len().saturating_mul(4))
                    .ok_or(SourceCommandError::Invalid(
                        "native V2 directory witness state overflow",
                    ))?,
                |total, (name, _)| {
                    total
                        .checked_add(name.len().saturating_mul(2))
                        .and_then(|value| value.checked_add(size_of::<(String, bool)>()))
                        .ok_or(SourceCommandError::Invalid(
                            "native V2 directory witness state overflow",
                        ))
                },
            )?;
            self.ensure_state(
                0,
                bytes.checked_mul(2).ok_or(SourceCommandError::Invalid(
                    "native V2 directory witness state overflow",
                ))?,
            )?;
            self.readset.directories.try_reserve_exact(1).map_err(|_| {
                SourceCommandError::Invalid("native V2 directory witness allocation")
            })?;
            self.readset.directories.push(SourceCutDirectoryWitness {
                path: relative,
                expected_presence: presence,
                expected_children: children.clone(),
            });
        }
        self.ensure_state(0, 0)?;
        Ok(children)
    }

    fn set_retained_state_bytes(
        &mut self,
        bytes: usize,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let prior = self.retained_file_state_bytes;
        self.retained_file_state_bytes = bytes;
        if let Err(error) = self.ensure_state(0, 0) {
            self.retained_file_state_bytes = prior;
            return Err(error);
        }
        Ok(())
    }

    fn retained_state_bytes(&self) -> usize {
        self.retained_file_state_bytes
    }

    fn has_file(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<bool> {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SourceCommandError::Unsupported(
                "native V2 record probe deadline or cancellation",
            ));
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("native V2 selected record path"))?;
        let metadata = self
            .session
            .member(
                self.revision,
                &relative,
                self.live_state()?,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("native V2 selected record probe"))?;
        let (ids, presence) = match metadata.as_ref() {
            Some(_) => {
                let ids = self
                    .session
                    .indexed_ids_for_path(
                        self.revision,
                        &relative,
                        self.live_state()?,
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| SourceCommandError::Conflict("native V2 indexed record member"))?
                    .ok_or(SourceCommandError::Conflict(
                        "native V2 indexed record member is absent",
                    ))?;
                (Some(ids), Some(SourcePresenceV1::File))
            }
            None => {
                let presence = self
                    .session
                    .presence(
                        self.revision,
                        &relative,
                        self.live_state()?,
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| {
                        SourceCommandError::Conflict("native V2 selected record presence")
                    })?;
                (None, presence)
            }
        };
        self.capture_file_witness(&relative, metadata, ids, presence, 0, deadline, cancelled)?;
        Ok(presence == Some(SourcePresenceV1::File))
    }
}

impl SourceCutRead for NativeSourceV2Session {
    fn selection(&self) -> SourceCutSelection {
        let selection = self.reader.selected_selection();
        SourceCutSelection {
            format: SourceCutFormat::NativeAdmissionV2,
            current_revision: selection.revision,
            rootset_sha256: selection.rootset_sha256,
        }
    }

    fn revision_stream<'a>(
        &'a mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Box<dyn SourceCutRevisionStream + 'a>> {
        check_cut_clock(deadline, cancelled)?;
        ensure_cut_state(self, caller_retained_state_bytes)?;
        let next_revision = self.reader.selected_revision();
        let max_revisions = self.profile.max_revisions;
        let mut seen = Vec::new();
        seen.try_reserve_exact(max_revisions).map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 revision cursor allocation",
            )
        })?;
        ensure_cut_state(
            self,
            caller_retained_state_bytes
                .checked_add(
                    seen.capacity()
                        .checked_mul(size_of::<SourceRevision>())
                        .ok_or_else(|| {
                            cut_refusal(
                                StoreErrorCode::BudgetExceeded,
                                "V2 revision cursor state overflow",
                            )
                        })?,
                )
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 revision cursor state overflow",
                    )
                })?,
        )?;
        Ok(Box::new(V2RevisionStream {
            session: self,
            next_revision: Some(next_revision),
            seen,
            max_revisions,
            complete: false,
            failed: false,
        }))
    }

    fn revision(
        &mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<SourceCutRevision>> {
        check_cut_clock(deadline, cancelled)?;
        match self.cut_revision(revision, caller_retained_state_bytes) {
            Ok(revision) => Ok(Some(revision)),
            Err(error) if error.code == StoreErrorCode::MissingRevision => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<MemberMetadata>> {
        check_cut_clock(deadline, cancelled)?;
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        let root_state = roots
            .retained_state_bytes()
            .map_err(|error| cut_io_error("V2 member root state unavailable", error))?;
        ensure_cut_state(
            self,
            caller_retained_state_bytes
                .checked_add(root_state)
                .and_then(|bytes| bytes.checked_add(path.as_str().len().checked_mul(4)?))
                .and_then(|bytes| bytes.checked_add(size_of::<SourceCutRevision>() + 256))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 member lookup state overflow",
                    )
                })?,
        )?;
        let Some(tuple) = self
            .reader
            .member_tuple(revision, path)
            .map_err(|error| cut_io_error("V2 source member lookup failed", error))?
        else {
            return Ok(None);
        };
        if roots.revision != tuple.revision {
            return Err(cut_refusal(
                StoreErrorCode::DescriptorMismatch,
                "V2 source member revision differs",
            ));
        }
        ensure_cut_state(
            self,
            caller_retained_state_bytes
                .checked_add(root_state)
                .and_then(|bytes| bytes.checked_add(path.as_str().len().checked_mul(8)?))
                .and_then(|bytes| bytes.checked_add(size_of::<MemberMetadata>() + 256))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 member result state overflow",
                    )
                })?,
        )?;
        check_cut_clock(deadline, cancelled)?;
        Ok(Some(MemberMetadata {
            path: tuple.path,
            sha256: tuple.sha256,
            size_bytes: tuple.size_bytes,
            mode: tuple.source_mode,
        }))
    }

    fn list_directory(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        max_entries: usize,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<Vec<(String, bool)>>> {
        check_cut_clock(deadline, cancelled)?;
        if max_entries == 0 || max_entries == usize::MAX {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 source directory child budget is not finite",
            ));
        }
        self.cut_roots(revision, caller_retained_state_bytes)?;
        let mut lower = Vec::new();
        lower
            .try_reserve_exact(path.as_str().len().saturating_add(1))
            .map_err(|_| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 directory prefix allocation",
                )
            })?;
        lower.extend_from_slice(path.as_str().as_bytes());
        lower.push(b'/');
        let upper = prefix_successor(&lower).ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::UnsafePath,
                "V2 directory prefix range overflow",
            )
        })?;
        let mut after: Option<Vec<u8>> = None;
        let mut children = Vec::<(String, bool)>::new();
        let mut child_bytes = 0usize;
        loop {
            let cursor_state = caller_retained_state_bytes
                .checked_add(lower.capacity())
                .and_then(|bytes| bytes.checked_add(upper.capacity()))
                .and_then(|bytes| bytes.checked_add(after.as_ref().map_or(0, Vec::capacity)))
                .and_then(|bytes| {
                    bytes.checked_add(
                        children
                            .capacity()
                            .checked_mul(size_of::<(String, bool)>())?,
                    )
                })
                .and_then(|bytes| bytes.checked_add(child_bytes))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 directory cursor state overflow",
                    )
                })?;
            ensure_cut_state(self, cursor_state)?;
            let row = self
                .reader
                .next_row_after_with_caller_state(
                    revision,
                    V2RootKind::Members,
                    Some(&lower),
                    Some(&upper),
                    after.as_deref(),
                    cursor_state,
                )
                .map_err(|error| cut_io_error("V2 source directory cursor failed", error))?;
            let Some(row) = row else {
                break;
            };
            let tail = row
                .key
                .strip_prefix(lower.as_slice())
                .filter(|tail| !tail.is_empty())
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::DescriptorMismatch,
                        "V2 directory key differs",
                    )
                })?;
            let (name, is_directory) = match tail.iter().position(|byte| *byte == b'/') {
                Some(slash) => (&tail[..slash], true),
                None => (tail, false),
            };
            let name = std::str::from_utf8(name).map_err(|_| {
                cut_refusal(
                    StoreErrorCode::UnsafePath,
                    "V2 directory child is not UTF-8",
                )
            })?;
            if name.is_empty() || name == "." || name == ".." {
                return Err(cut_refusal(
                    StoreErrorCode::UnsafePath,
                    "V2 directory child path is invalid",
                ));
            }
            if let Some((_, prior)) = children.iter().find(|(child, _)| child == name) {
                if *prior != is_directory {
                    return Err(cut_refusal(
                        StoreErrorCode::DescriptorMismatch,
                        "V2 source path is both file and directory",
                    ));
                }
            } else {
                children.try_reserve_exact(1).map_err(|_| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 directory result allocation",
                    )
                })?;
                child_bytes = child_bytes.checked_add(name.len()).ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 directory child state overflow",
                    )
                })?;
                children.push((name.to_owned(), is_directory));
                if children.len() > max_entries {
                    return Err(cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 source directory child budget exceeded",
                    ));
                }
            }
            let mut next_after = Vec::new();
            if is_directory {
                let skip_len = lower
                    .len()
                    .checked_add(name.len())
                    .and_then(|bytes| bytes.checked_add(2))
                    .ok_or_else(|| {
                        cut_refusal(
                            StoreErrorCode::BudgetExceeded,
                            "V2 directory seek key overflow",
                        )
                    })?;
                next_after.try_reserve_exact(skip_len).map_err(|_| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 directory seek allocation",
                    )
                })?;
                next_after.extend_from_slice(&lower);
                next_after.extend_from_slice(name.as_bytes());
                next_after.push(b'/');
                next_after.push(u8::MAX);
            } else {
                next_after.try_reserve_exact(row.key.len()).map_err(|_| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 directory cursor allocation",
                    )
                })?;
                next_after.extend_from_slice(&row.key);
            }
            after = Some(next_after);
            check_cut_clock(deadline, cancelled)?;
        }
        check_cut_clock(deadline, cancelled)?;
        if children.is_empty() {
            Ok(None)
        } else {
            Ok(Some(children))
        }
    }

    fn identity_path(
        &mut self,
        revision: SourceRevision,
        id: &str,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<RelativePath>> {
        check_cut_clock(deadline, cancelled)?;
        self.cut_roots(revision, caller_retained_state_bytes)?;
        let lookup_state = self
            .reader
            .identity_lookup_result_state_upper_bound()
            .map_err(|error| cut_io_error("V2 identity lookup state unavailable", error))?;
        ensure_reader_cut_state(
            &self.reader,
            self.caller_retained_state_bytes,
            self.profile.max_state_bytes,
            caller_retained_state_bytes
                .checked_add(lookup_state)
                .ok_or_else(|| {
                    cut_refusal(StoreErrorCode::BudgetExceeded, "V2 identity state overflow")
                })?,
        )?;
        let path = self
            .reader
            .identity_path(revision, id)
            .map_err(|error| cut_io_error("V2 identity path lookup failed", error))?;
        check_cut_clock(deadline, cancelled)?;
        Ok(path)
    }

    fn indexed_ids_for_path(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<Vec<String>>> {
        check_cut_clock(deadline, cancelled)?;
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        if self
            .reader
            .member_tuple(revision, path)
            .map_err(|error| cut_io_error("V2 source member lookup failed", error))?
            .is_none()
        {
            return Ok(None);
        }
        if roots.identity_paths.is_none() {
            return Err(cut_refusal(
                StoreErrorCode::UnsupportedFormat,
                "V2 source revision lacks exact path identity coverage",
            ));
        }
        drop(roots);
        let live = caller_retained_state_bytes
            .checked_add(path.as_str().len().checked_mul(4).ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity path state overflow",
                )
            })?)
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity path state overflow",
                )
            })?;
        let ids = v2_indexed_ids_for_path(
            &mut self.reader,
            self.caller_retained_state_bytes,
            self.profile.max_state_bytes,
            self.profile.tree.max_key_bytes,
            revision,
            path,
            self.profile.max_members,
            live,
            deadline,
            cancelled,
        )?;
        Ok(Some(ids))
    }

    fn indexed_dependencies(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<Vec<RelativePath>>> {
        check_cut_clock(deadline, cancelled)?;
        self.cut_roots(revision, caller_retained_state_bytes)?;
        if self
            .reader
            .member_tuple(revision, path)
            .map_err(|error| cut_io_error("V2 source member lookup failed", error))?
            .is_none()
        {
            return Ok(None);
        }
        let mut lower = path.as_str().as_bytes().to_vec();
        lower.push(0);
        let upper = prefix_successor(&lower).ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::InvalidDependencyIndex,
                "V2 dependency range overflow",
            )
        })?;
        let mut after: Option<Vec<u8>> = None;
        let mut targets = Vec::new();
        let mut target_bytes = 0usize;
        loop {
            let live = caller_retained_state_bytes
                .checked_add(lower.capacity())
                .and_then(|bytes| bytes.checked_add(upper.capacity()))
                .and_then(|bytes| bytes.checked_add(after.as_ref().map_or(0, Vec::capacity)))
                .and_then(|bytes| {
                    bytes.checked_add(targets.capacity().checked_mul(size_of::<RelativePath>())?)
                })
                .and_then(|bytes| bytes.checked_add(target_bytes))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 dependency result state overflow",
                    )
                })?;
            ensure_cut_state(self, live)?;
            let row = self
                .reader
                .next_row_after_with_caller_state(
                    revision,
                    V2RootKind::Dependencies,
                    Some(&lower),
                    Some(&upper),
                    after.as_deref(),
                    live,
                )
                .map_err(|error| cut_io_error("V2 source dependency cursor failed", error))?;
            let Some(row) = row else {
                break;
            };
            let (source, target) = decode_dependency_row(&row.key, &row.value)?;
            if source != *path {
                return Err(cut_refusal(
                    StoreErrorCode::InvalidDependencyIndex,
                    "V2 source dependency key differs",
                ));
            }
            let target_state = live
                .checked_add(row.key.capacity())
                .and_then(|bytes| bytes.checked_add(row.value.capacity()))
                .and_then(|bytes| bytes.checked_add(target.as_str().len().checked_mul(4)?))
                .and_then(|bytes| bytes.checked_add(size_of::<RelativePath>() + 64))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 dependency row state overflow",
                    )
                })?;
            ensure_cut_state(self, target_state)?;
            let next_bytes = target_bytes
                .checked_add(target.as_str().len())
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 dependency state overflow",
                    )
                })?;
            targets.try_reserve_exact(1).map_err(|_| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 dependency result allocation",
                )
            })?;
            targets.push(target);
            target_bytes = next_bytes;
            after = Some(row.key);
            if targets.len() as u64 > self.profile.max_members {
                return Err(cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 source dependency count exceeds selected member budget",
                ));
            }
            check_cut_clock(deadline, cancelled)?;
        }
        Ok(Some(targets))
    }

    fn presence(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<SourcePresenceV1>> {
        check_cut_clock(deadline, cancelled)?;
        self.cut_roots(revision, caller_retained_state_bytes)?;
        if self
            .reader
            .member_tuple(revision, path)
            .map_err(|error| cut_io_error("V2 source member lookup failed", error))?
            .is_some()
        {
            return Ok(Some(SourcePresenceV1::File));
        }
        let lower = format!("{}/", path.as_str()).into_bytes();
        let upper = format!("{}0", path.as_str()).into_bytes();
        let mut live = caller_retained_state_bytes
            .checked_add(lower.capacity())
            .and_then(|bytes| bytes.checked_add(upper.capacity()))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 source presence state overflow",
                )
            })?;
        ensure_cut_state(self, live)?;
        let row = self
            .reader
            .next_row_after_with_caller_state(
                revision,
                V2RootKind::Members,
                Some(&lower),
                Some(&upper),
                None,
                live,
            )
            .map_err(|error| cut_io_error("V2 source presence range failed", error))?;
        live = live
            .checked_add(
                row.as_ref()
                    .map_or(0, |row| row.key.capacity() + row.value.capacity()),
            )
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 source presence row overflow",
                )
            })?;
        ensure_cut_state(self, live)?;
        check_cut_clock(deadline, cancelled)?;
        Ok(row.map(|_| SourcePresenceV1::MaterializedDirectory))
    }

    fn read_member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: u64,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<SourceMemberV1> {
        check_cut_clock(deadline, cancelled)?;
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        let root_state = roots
            .retained_state_bytes()
            .map_err(|error| cut_io_error("V2 member root state unavailable", error))?;
        ensure_cut_state(
            self,
            caller_retained_state_bytes
                .checked_add(root_state)
                .and_then(|bytes| bytes.checked_add(path.as_str().len().checked_mul(8)?))
                .and_then(|bytes| bytes.checked_add(size_of::<SourceMemberV1>() + 512))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 member lookup state overflow",
                    )
                })?,
        )?;
        let tuple = self
            .reader
            .member_tuple(revision, path)
            .map_err(|error| cut_io_error("V2 source member lookup failed", error))?
            .ok_or_else(|| cut_refusal(StoreErrorCode::MissingMember, "source member is absent"))?;
        if roots.revision != tuple.revision || roots.member_count != roots.members.entries {
            return Err(cut_refusal(
                StoreErrorCode::DescriptorMismatch,
                "V2 source member revision or count differs",
            ));
        }
        let cap = max_bytes.min(self.profile.max_member_bytes);
        if tuple.size_bytes > cap {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 source member exceeds selected byte cap",
            ));
        }
        drop(roots);
        let tuple_state = tuple
            .path
            .as_str()
            .len()
            .checked_mul(4)
            .and_then(|bytes| {
                bytes.checked_add(size_of::<tos_source_store::MemberMetadata>() + 128)
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member tuple state overflow",
                )
            })?;
        let ids = self
            .indexed_ids_for_path(
                revision,
                path,
                caller_retained_state_bytes
                    .checked_add(tuple_state)
                    .ok_or_else(|| {
                        cut_refusal(
                            StoreErrorCode::BudgetExceeded,
                            "V2 member identity state overflow",
                        )
                    })?,
                deadline,
                cancelled,
            )?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::InvalidIdentityIndex,
                    "V2 member identity rows are absent",
                )
            })?;
        let ids_state = ids
            .capacity()
            .checked_mul(size_of::<String>())
            .and_then(|bytes| {
                ids.iter()
                    .try_fold(bytes, |total, id| total.checked_add(id.capacity()))
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member identity state overflow",
                )
            })?;
        let output_bytes = usize::try_from(tuple.size_bytes).map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 source member size range",
            )
        })?;
        let live = caller_retained_state_bytes
            .checked_add(tuple_state)
            .and_then(|bytes| bytes.checked_add(ids_state))
            .and_then(|bytes| bytes.checked_add(output_bytes))
            .and_then(|bytes| bytes.checked_add(path.as_str().len().checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(size_of::<SourceMemberV1>() + 256))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 source member output state overflow",
                )
            })?;
        ensure_cut_state(self, live)?;
        let observation = self
            .reader
            .read_member_with_caller_state(revision, path, live)
            .map_err(|error| cut_io_error("V2 source member object read failed", error))?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::MissingMember,
                    "source member object is absent",
                )
            })?;
        if observation.sha256 != tuple.sha256
            || observation.size_bytes != tuple.size_bytes
            || observation.path != *path
            || observation.revision != revision
            || Digest256::of_bytes(&observation.bytes) != tuple.sha256
        {
            return Err(cut_refusal(
                StoreErrorCode::CorruptSelectedObject,
                "V2 source member observation differs",
            ));
        }
        let raw = observation.bytes.clone();
        check_cut_clock(deadline, cancelled)?;
        Ok(SourceMemberV1 {
            path: path.clone(),
            raw,
            revision,
            stable_ids: ids,
        })
    }

    fn read_retirement(
        &mut self,
        revision: SourceRevision,
        index: usize,
        max_bytes: u64,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<RetiredSourceMemberV1> {
        check_cut_clock(deadline, cancelled)?;
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        let ordinal = u64::try_from(index).map_err(|_| {
            cut_refusal(
                StoreErrorCode::InvalidRetirementIndex,
                "V2 retirement ordinal range",
            )
        })?;
        if ordinal >= roots.retirement_count {
            return Err(cut_refusal(
                StoreErrorCode::MissingMember,
                "V2 retirement ordinal is absent",
            ));
        }
        let lower = ordinal.to_be_bytes();
        let row = self
            .reader
            .next_row_after_with_caller_state(
                revision,
                V2RootKind::Retirements,
                Some(&lower),
                None,
                None,
                caller_retained_state_bytes,
            )
            .map_err(|error| cut_io_error("V2 retirement row lookup failed", error))?
            .filter(|row| row.key.as_slice() == lower)
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::InvalidRetirementIndex,
                    "V2 retirement ordinal differs",
                )
            })?;
        let metadata = decode_retirement_row(&row.key, &row.value)?;
        if !tos_source_store::is_authored_source_path_v1(metadata.path.as_str())
            || !tos_source_store::is_authored_source_path_v1(metadata.event_ref.as_str())
        {
            return Err(cut_refusal(
                StoreErrorCode::InvalidRetirementIndex,
                "V2 retirement is outside authored source",
            ));
        }
        let cap = usize::try_from(max_bytes.min(self.profile.max_member_bytes)).map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 retirement object cap range",
            )
        })?;
        drop(roots);
        let row_state = row
            .key
            .capacity()
            .checked_add(row.value.capacity())
            .and_then(|bytes| bytes.checked_add(metadata.path.as_str().len().checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(metadata.event_ref.as_str().len().checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(size_of::<RetirementMetadata>() + 256))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 retirement row state overflow",
                )
            })?;
        let lookup_state = caller_retained_state_bytes
            .checked_add(row_state)
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 retirement lookup state overflow",
                )
            })?;
        ensure_cut_state(self, lookup_state)?;
        let target_location = self
            .reader
            .object_location_by_digest(revision, metadata.sha256, lookup_state)
            .map_err(|error| cut_io_error("V2 retired source object location failed", error))?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::InvalidRetirementIndex,
                    "V2 retired source object is absent",
                )
            })?;
        if target_location.size > cap as u64 || metadata.event_size_bytes > cap as u64 {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 retirement object exceeds selected cap",
            ));
        }
        let target_size = usize::try_from(target_location.size).map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 retired object size range",
            )
        })?;
        let event_size = usize::try_from(metadata.event_size_bytes).map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 retirement event size range",
            )
        })?;
        let output_state = target_size
            .checked_add(event_size)
            .and_then(|bytes| bytes.checked_add(size_of::<RetiredSourceMemberV1>() + 512))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 retirement output state overflow",
                )
            })?;
        let live = lookup_state.checked_add(output_state).ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 retirement retained state overflow",
            )
        })?;
        ensure_cut_state(self, live)?;
        let raw = self
            .reader
            .read_object_by_digest(
                revision,
                metadata.sha256,
                Some(target_location.size),
                cap,
                live,
            )
            .map_err(|error| cut_io_error("V2 retired source object read failed", error))?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::InvalidRetirementIndex,
                    "V2 retired source object is absent",
                )
            })?;
        let event_raw = self
            .reader
            .read_object_by_digest(
                revision,
                metadata.event_sha256,
                Some(metadata.event_size_bytes),
                cap,
                live.checked_add(raw.capacity()).ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 retirement event state overflow",
                    )
                })?,
            )
            .map_err(|error| cut_io_error("V2 retirement event read failed", error))?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::InvalidRetirementIndex,
                    "V2 retirement event is absent",
                )
            })?;
        if raw.len() > cap || event_raw.len() > cap {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 retirement object exceeds selected cap",
            ));
        }
        check_cut_clock(deadline, cancelled)?;
        Ok(RetiredSourceMemberV1 {
            revision,
            metadata,
            raw,
            event_raw,
        })
    }

    fn metadata_stream<'a>(
        &'a mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Box<dyn SourceCutMetadataStream + 'a>> {
        check_cut_clock(deadline, cancelled)?;
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        let expected = v2_membership(&roots)?;
        drop(roots);
        let max_members = self.profile.max_members;
        let max_total_bytes = self.profile.max_total_bytes;
        let max_state_bytes = self.profile.max_state_bytes;
        let max_tree_key_bytes = self.profile.tree.max_key_bytes;
        let baseline_caller_retained_state_bytes = self.caller_retained_state_bytes;
        Ok(Box::new(V2MetadataStream {
            reader: &mut self.reader,
            revision,
            max_members,
            max_total_bytes,
            max_state_bytes,
            max_tree_key_bytes,
            baseline_caller_retained_state_bytes,
            expected,
            after: None,
            count: 0,
            bytes: 0,
            actual: membership_hasher_v1(),
            complete: false,
            failed: false,
        }))
    }

    fn member_stream<'a>(
        &'a mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Box<dyn SourceCutMemberStream + 'a>> {
        check_cut_clock(deadline, cancelled)?;
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        let expected = v2_membership(&roots)?;
        drop(roots);
        let max_members = self.profile.max_members;
        let max_total_bytes = self.profile.max_total_bytes;
        let max_member_bytes = self.profile.max_member_bytes;
        let max_state_bytes = self.profile.max_state_bytes;
        let max_tree_key_bytes = self.profile.tree.max_key_bytes;
        let baseline_caller_retained_state_bytes = self.caller_retained_state_bytes;
        Ok(Box::new(V2MemberStream {
            reader: &mut self.reader,
            revision,
            max_members,
            max_total_bytes,
            max_member_bytes,
            max_state_bytes,
            max_tree_key_bytes,
            baseline_caller_retained_state_bytes,
            expected,
            after: None,
            count: 0,
            bytes: 0,
            actual: membership_hasher_v1(),
            complete: false,
            failed: false,
        }))
    }

    fn object_refcount(
        &mut self,
        revision: SourceRevision,
        digest: Digest256,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<u64>> {
        check_cut_clock(deadline, cancelled)?;
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        if roots.objects.is_none() {
            return Err(cut_refusal(
                StoreErrorCode::UnsupportedFormat,
                "selected V2 source cut has no object index",
            ));
        }
        drop(roots);
        ensure_cut_state(self, caller_retained_state_bytes)?;
        let location = self
            .reader
            .object_location_by_digest(revision, digest, caller_retained_state_bytes)
            .map_err(|error| cut_io_error("V2 object index lookup failed", error))?;
        check_cut_clock(deadline, cancelled)?;
        if location.is_some() {
            // PackedObjectLocationV2 authenticates location and size, but
            // does not encode reference counts. Presence is not count one.
            return Err(cut_refusal(
                StoreErrorCode::UnsupportedFormat,
                "selected V2 object value has no authenticated refcount",
            ));
        }
        Ok(None)
    }

    fn verify_current_fence(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<()> {
        check_cut_clock(deadline, cancelled)?;
        self.reader
            .verify_current_fence()
            .map_err(|error| cut_io_error("V2 source current fence failed", error))?;
        check_cut_clock(deadline, cancelled)
    }

    fn verify_readset_current(
        &mut self,
        base_revision: SourceRevision,
        members: &[SourceCutMemberWitness],
        identities: &[SourceCutIdentityWitness],
        directories: &[SourceCutDirectoryWitness],
        dependencies: &[SourceCutDependencyWitness],
        object_refcounts: &[SourceCutObjectRefcountWitness],
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<SourceCutSelection> {
        check_cut_clock(deadline, cancelled)?;
        let readset_state = source_cut_readset_state_upper_bound(
            members,
            identities,
            directories,
            dependencies,
            object_refcounts,
        )?;
        let caller_state = caller_retained_state_bytes
            .checked_add(readset_state)
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 source readset state overflow",
                )
            })?;
        ensure_cut_state(self, caller_state)?;

        // A moved current selector is not itself a whole-source conflict. Open
        // the latest root under the same held directory, IO ledger, work
        // budget, deadline and cancellation handle, then re-prove every exact
        // operation witness before replacing the reader. The opened selection
        // remains available separately as invocation provenance.
        let needs_reopen = matches!(
            self.reader
                .observe_current_selection()
                .map_err(|error| cut_io_error("V2 current selector observation failed", error))?,
            V2CurrentSelectionObservation::Advanced
        );
        let prior_state = if needs_reopen {
            self.reader
                .declared_retained_state_bytes()
                .map_err(|error| cut_io_error("V2 prior reader state unavailable", error))?
                .0
        } else {
            0
        };
        if needs_reopen {
            let reopen_caller_state = self
                .caller_retained_state_bytes
                .checked_add(prior_state)
                .and_then(|bytes| bytes.checked_add(caller_state))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 readset rebase state overflow",
                    )
                })?;
            ensure_reader_state(
                &self.reader,
                self.caller_retained_state_bytes,
                self.profile.max_state_bytes,
                caller_state,
            )
            .map_err(|_| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 readset rebase state allowance exceeded",
                )
            })?;
            let candidate = V2ReadSession::open_at_named_with_work(
                &self.path,
                &self._held_root,
                self.profile.point_limits(reopen_caller_state),
                self.io.clone(),
                self.work.clone(),
                deadline,
                self.cancelled.clone(),
            )
            .map_err(|error| cut_io_error("V2 current readset reselect failed", error))?;
            let candidate_state = candidate
                .declared_retained_state_bytes()
                .map_err(|error| cut_io_error("V2 current reader state unavailable", error))?
                .0;
            if candidate_state
                .checked_add(reopen_caller_state)
                .is_none_or(|bytes| bytes > self.profile.max_state_bytes)
            {
                return Err(cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 current readset reselect exceeds whole-operation state",
                ));
            }
            let prior = std::mem::replace(&mut self.reader, candidate);
            let selected = match self.verify_readset_on_selected_current(
                base_revision,
                members,
                identities,
                directories,
                dependencies,
                object_refcounts,
                caller_state.checked_add(prior_state).ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 prior reader overlap overflow",
                    )
                })?,
                deadline,
                cancelled,
            ) {
                Ok(selection) => selection,
                Err(error) => {
                    self.reader = prior;
                    return Err(error);
                }
            };
            drop(prior);
            Ok(selected)
        } else {
            self.verify_readset_on_selected_current(
                base_revision,
                members,
                identities,
                directories,
                dependencies,
                object_refcounts,
                caller_state,
                deadline,
                cancelled,
            )
        }
    }
}

impl NativeSourceV2Session {
    fn verify_readset_on_selected_current(
        &mut self,
        base_revision: SourceRevision,
        members: &[SourceCutMemberWitness],
        identities: &[SourceCutIdentityWitness],
        directories: &[SourceCutDirectoryWitness],
        dependencies: &[SourceCutDependencyWitness],
        object_refcounts: &[SourceCutObjectRefcountWitness],
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<SourceCutSelection> {
        check_cut_clock(deadline, cancelled)?;
        if !self.revision_is_in_selected_chain(base_revision, caller_retained_state_bytes)? {
            return Err(cut_refusal(
                StoreErrorCode::MissingRevision,
                "V2 rebased current no longer retains the invocation base",
            ));
        }
        let current = self.reader.selected_revision();
        for witness in members {
            check_cut_clock(deadline, cancelled)?;
            if self.presence(
                current,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_presence
            {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset path presence changed",
                ));
            }
            let actual = self.member(
                current,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )?;
            let expected = witness.expected_member.map(|tuple| MemberMetadata {
                path: witness.path.clone(),
                sha256: tuple.sha256,
                size_bytes: tuple.size_bytes,
                mode: tuple.mode,
            });
            if actual != expected {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset member binding changed",
                ));
            }
            if self.indexed_ids_for_path(
                current,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_indexed_ids
            {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset inverse identity coverage changed",
                ));
            }
        }
        for witness in identities {
            check_cut_clock(deadline, cancelled)?;
            if self.identity_path(
                current,
                &witness.id,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_path
            {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset identity binding changed",
                ));
            }
        }
        for witness in directories {
            check_cut_clock(deadline, cancelled)?;
            if self.presence(
                current,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_presence
            {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset directory presence changed",
                ));
            }
            let max_entries = witness
                .expected_children
                .len()
                .checked_add(1)
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 directory witness cap overflow",
                    )
                })?;
            let actual = self
                .list_directory(
                    current,
                    &witness.path,
                    max_entries,
                    caller_retained_state_bytes,
                    deadline,
                    cancelled,
                )?
                .unwrap_or_default();
            if actual != witness.expected_children {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset directory selection changed",
                ));
            }
        }
        for witness in dependencies {
            check_cut_clock(deadline, cancelled)?;
            if self.indexed_dependencies(
                current,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != Some(witness.expected_targets.clone())
            {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset dependency closure changed",
                ));
            }
        }
        for witness in object_refcounts {
            check_cut_clock(deadline, cancelled)?;
            if witness.expected_count == Some(0)
                || self.object_refcount(
                    current,
                    witness.digest,
                    caller_retained_state_bytes,
                    deadline,
                    cancelled,
                )? != witness.expected_count
            {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 readset object refcount changed",
                ));
            }
        }
        match self
            .reader
            .observe_current_selection()
            .map_err(|error| cut_io_error("V2 readset selector observation failed", error))?
        {
            V2CurrentSelectionObservation::StillSelected => (),
            V2CurrentSelectionObservation::Advanced => {
                return Err(cut_refusal(
                    StoreErrorCode::RevisionMismatch,
                    "V2 readset current selection advanced during verification",
                ));
            }
        }
        let selection = self.reader.selected_selection();
        Ok(SourceCutSelection {
            format: SourceCutFormat::NativeAdmissionV2,
            current_revision: selection.revision,
            rootset_sha256: selection.rootset_sha256,
        })
    }
}

impl NativeSourceV2Session {
    fn cut_roots(
        &mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
    ) -> tos_source_store::Result<SourceRevisionRootsV2> {
        if !self.revision_is_in_selected_chain(revision, caller_retained_state_bytes)? {
            return Err(cut_refusal(
                StoreErrorCode::MissingRevision,
                "revision is outside opened V2 source chain",
            ));
        }
        ensure_cut_state(self, caller_retained_state_bytes)?;
        let roots = self
            .reader
            .roots_for_revision_with_caller_state(revision, caller_retained_state_bytes)
            .map_err(|error| cut_io_error("V2 source revision roots unavailable", error))?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::MissingRevision,
                    "V2 source revision is absent",
                )
            })?;
        if roots.identity_paths.is_none() {
            return Err(cut_refusal(
                StoreErrorCode::UnsupportedFormat,
                "V2 source revision lacks exact path identity coverage",
            ));
        }
        let roots_state = roots
            .retained_state_bytes()
            .map_err(|error| cut_io_error("V2 source root state unavailable", error))?;
        ensure_cut_state(
            self,
            caller_retained_state_bytes
                .checked_add(roots_state)
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 source root state overflow",
                    )
                })?,
        )?;
        Ok(roots)
    }

    fn revision_is_in_selected_chain(
        &mut self,
        target: SourceRevision,
        caller_retained_state_bytes: usize,
    ) -> tos_source_store::Result<bool> {
        let mut next = Some(self.reader.selected_revision());
        let mut seen = Vec::new();
        seen.try_reserve_exact(self.profile.max_revisions)
            .map_err(|_| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 revision cursor allocation",
                )
            })?;
        while let Some(revision) = next {
            let live = caller_retained_state_bytes
                .checked_add(size_of::<SourceRevision>())
                .and_then(|bytes| {
                    bytes.checked_add(seen.capacity().checked_mul(size_of::<SourceRevision>())?)
                })
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 revision cursor state overflow",
                    )
                })?;
            ensure_cut_state(self, live)?;
            if revision == target {
                return Ok(true);
            }
            if seen.len() >= self.profile.max_revisions || seen.contains(&revision) {
                return Err(cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 selected revision chain exceeds profile or cycles",
                ));
            }
            seen.push(revision);
            let roots = self
                .reader
                .roots_for_revision_with_caller_state(revision, live)
                .map_err(|error| cut_io_error("V2 revision chain lookup failed", error))?
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::MissingRevision,
                        "V2 revision chain root is absent",
                    )
                })?;
            next = roots.base_revision;
        }
        Ok(false)
    }

    fn cut_revision(
        &mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
    ) -> tos_source_store::Result<SourceCutRevision> {
        let roots = self.cut_roots(revision, caller_retained_state_bytes)?;
        v2_cut_revision(&roots)
    }
}

struct V2RevisionStream<'a> {
    session: &'a mut NativeSourceV2Session,
    next_revision: Option<SourceRevision>,
    seen: Vec<SourceRevision>,
    max_revisions: usize,
    complete: bool,
    failed: bool,
}

impl SourceCutRevisionStream for V2RevisionStream<'_> {
    fn complete(&self) -> bool {
        self.complete
    }

    fn next_revision(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<SourceCutRevision>> {
        if self.failed {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 revision stream already refused",
            ));
        }
        let result = self.next_inner(caller_retained_state_bytes, deadline, cancelled);
        if result.is_err() {
            self.failed = true;
            self.complete = false;
        }
        result
    }
}

impl V2RevisionStream<'_> {
    fn next_inner(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<SourceCutRevision>> {
        check_cut_clock(deadline, cancelled)?;
        let Some(revision) = self.next_revision else {
            self.complete = true;
            return Ok(None);
        };
        if self.seen.len() >= self.max_revisions || self.seen.contains(&revision) {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 selected revision chain exceeds profile or cycles",
            ));
        }
        let live = caller_retained_state_bytes
            .checked_add(size_of::<Self>())
            .and_then(|bytes| {
                bytes.checked_add(
                    self.seen
                        .capacity()
                        .checked_mul(size_of::<SourceRevision>())?,
                )
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 revision cursor state overflow",
                )
            })?;
        ensure_cut_state(self.session, live)?;
        let roots = self
            .session
            .reader
            .roots_for_revision_with_caller_state(revision, live)
            .map_err(|error| cut_io_error("V2 revision stream root read failed", error))?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::MissingRevision,
                    "V2 revision stream root is absent",
                )
            })?;
        if roots.identity_paths.is_none() {
            return Err(cut_refusal(
                StoreErrorCode::UnsupportedFormat,
                "V2 retained revision lacks exact path identity coverage",
            ));
        }
        let roots_state = roots
            .retained_state_bytes()
            .map_err(|error| cut_io_error("V2 revision stream root state unavailable", error))?;
        ensure_cut_state(
            self.session,
            live.checked_add(roots_state)
                .and_then(|bytes| bytes.checked_add(size_of::<SourceCutRevision>()))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 revision output state overflow",
                    )
                })?,
        )?;
        let locator = v2_cut_revision(&roots)?;
        self.seen.push(revision);
        self.next_revision = roots.base_revision;
        check_cut_clock(deadline, cancelled)?;
        Ok(Some(locator))
    }
}

struct V2MetadataStream<'a> {
    reader: &'a mut V2ReadSession,
    revision: SourceRevision,
    max_members: u64,
    max_total_bytes: u64,
    max_state_bytes: usize,
    max_tree_key_bytes: usize,
    baseline_caller_retained_state_bytes: usize,
    expected: SourceCutMembership,
    after: Option<Vec<u8>>,
    count: u64,
    bytes: u64,
    actual: Digest256Hasher,
    complete: bool,
    failed: bool,
}

impl SourceCutMetadataStream for V2MetadataStream<'_> {
    fn expectation(&self) -> SourceCutMembership {
        self.expected
    }

    fn coverage(&self) -> Option<SourceCutMembership> {
        self.complete.then_some(self.expected)
    }

    fn next_metadata(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<MemberMetadata>> {
        if self.failed {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 metadata stream already refused",
            ));
        }
        let result = self.next_inner(caller_retained_state_bytes, deadline, cancelled);
        if result.is_err() {
            self.failed = true;
            self.complete = false;
        }
        result
    }
}

impl V2MetadataStream<'_> {
    fn next_inner(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<MemberMetadata>> {
        check_cut_clock(deadline, cancelled)?;
        if self.complete {
            return Ok(None);
        }
        let dynamic = caller_retained_state_bytes
            .checked_add(size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(self.after.as_ref().map_or(0, Vec::capacity)))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 metadata cursor state overflow",
                )
            })?;
        ensure_reader_cut_state(
            self.reader,
            self.baseline_caller_retained_state_bytes,
            self.max_state_bytes,
            dynamic,
        )?;
        let next = self
            .reader
            .next_member_tuple_after(self.revision, self.after.as_deref(), dynamic)
            .map_err(|error| cut_io_error("V2 metadata cursor failed", error))?;
        let Some(tuple) = next else {
            let rows = self.expected.rows();
            if self.count != rows.count || self.actual.clone().finalize() != rows.digest {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 metadata membership differs",
                ));
            }
            self.complete = true;
            return Ok(None);
        };
        if self.count >= self.max_members {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 metadata member budget exceeded",
            ));
        }
        let next_bytes = self
            .bytes
            .checked_add(tuple.size_bytes)
            .filter(|bytes| *bytes <= self.max_total_bytes)
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 metadata byte budget exceeded",
                )
            })?;
        let result_state = dynamic
            .checked_add(tuple.path.as_str().len().checked_mul(8).ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 metadata path state overflow",
                )
            })?)
            .and_then(|bytes| bytes.checked_add(size_of::<MemberMetadata>() + 256))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 metadata result state overflow",
                )
            })?;
        ensure_reader_cut_state(
            self.reader,
            self.baseline_caller_retained_state_bytes,
            self.max_state_bytes,
            result_state,
        )?;
        feed_source_membership_v1(
            &mut self.actual,
            tuple.path.as_str(),
            tuple.size_bytes,
            tuple.sha256,
        );
        let mut after = Vec::new();
        after
            .try_reserve_exact(tuple.path.as_str().len())
            .map_err(|_| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 metadata cursor path allocation",
                )
            })?;
        after.extend_from_slice(tuple.path.as_str().as_bytes());
        self.after = Some(after);
        self.count += 1;
        self.bytes = next_bytes;
        check_cut_clock(deadline, cancelled)?;
        Ok(Some(MemberMetadata {
            path: tuple.path,
            sha256: tuple.sha256,
            size_bytes: tuple.size_bytes,
            mode: tuple.source_mode,
        }))
    }
}

struct V2MemberStream<'a> {
    reader: &'a mut V2ReadSession,
    revision: SourceRevision,
    max_members: u64,
    max_total_bytes: u64,
    max_member_bytes: u64,
    max_state_bytes: usize,
    max_tree_key_bytes: usize,
    baseline_caller_retained_state_bytes: usize,
    expected: SourceCutMembership,
    after: Option<Vec<u8>>,
    count: u64,
    bytes: u64,
    actual: Digest256Hasher,
    complete: bool,
    failed: bool,
}

impl SourceCutMemberStream for V2MemberStream<'_> {
    fn expectation(&self) -> SourceCutMembership {
        self.expected
    }

    fn coverage(&self) -> Option<SourceCutMembership> {
        self.complete.then_some(self.expected)
    }

    fn next_member(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<SourceMemberV1>> {
        if self.failed {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 member stream already refused",
            ));
        }
        let result = self.next_inner(caller_retained_state_bytes, deadline, cancelled);
        if result.is_err() {
            self.failed = true;
            self.complete = false;
        }
        result
    }
}

impl V2MemberStream<'_> {
    fn next_inner(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_source_store::Result<Option<SourceMemberV1>> {
        check_cut_clock(deadline, cancelled)?;
        if self.complete {
            return Ok(None);
        }
        let cursor_state = caller_retained_state_bytes
            .checked_add(size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(self.after.as_ref().map_or(0, Vec::capacity)))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member cursor state overflow",
                )
            })?;
        ensure_reader_cut_state(
            self.reader,
            self.baseline_caller_retained_state_bytes,
            self.max_state_bytes,
            cursor_state,
        )?;
        let next = self
            .reader
            .next_member_tuple_after(self.revision, self.after.as_deref(), cursor_state)
            .map_err(|error| cut_io_error("V2 member cursor failed", error))?;
        let Some(tuple) = next else {
            let rows = self.expected.rows();
            if self.count != rows.count || self.actual.clone().finalize() != rows.digest {
                return Err(cut_refusal(
                    StoreErrorCode::DescriptorMismatch,
                    "V2 member stream membership differs",
                ));
            }
            self.complete = true;
            return Ok(None);
        };
        if self.count >= self.max_members || tuple.size_bytes > self.max_member_bytes {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 member stream profile exceeded",
            ));
        }
        let next_bytes = self
            .bytes
            .checked_add(tuple.size_bytes)
            .filter(|bytes| *bytes <= self.max_total_bytes)
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member stream byte cap exceeded",
                )
            })?;
        let tuple_state = tuple
            .path
            .as_str()
            .len()
            .checked_mul(8)
            .and_then(|bytes| bytes.checked_add(size_of::<MemberMetadata>() + 256))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member tuple state overflow",
                )
            })?;
        let tuple_live = cursor_state.checked_add(tuple_state).ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 member tuple state overflow",
            )
        })?;
        ensure_reader_cut_state(
            self.reader,
            self.baseline_caller_retained_state_bytes,
            self.max_state_bytes,
            tuple_live,
        )?;
        let stable_ids = v2_indexed_ids_for_path(
            self.reader,
            self.baseline_caller_retained_state_bytes,
            self.max_state_bytes,
            self.max_tree_key_bytes,
            self.revision,
            &tuple.path,
            self.max_members,
            tuple_live,
            deadline,
            cancelled,
        )?;
        let ids_state = stable_ids
            .capacity()
            .checked_mul(size_of::<String>())
            .and_then(|bytes| {
                stable_ids
                    .iter()
                    .try_fold(bytes, |total, id| total.checked_add(id.capacity()))
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member stream identity state overflow",
                )
            })?;
        let output_bytes = usize::try_from(tuple.size_bytes).map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 member stream size range",
            )
        })?;
        let read_state = tuple_live
            .checked_add(ids_state)
            .and_then(|bytes| bytes.checked_add(output_bytes))
            .and_then(|bytes| bytes.checked_add(size_of::<SourceMemberV1>() + 256))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member stream read state overflow",
                )
            })?;
        ensure_reader_cut_state(
            self.reader,
            self.baseline_caller_retained_state_bytes,
            self.max_state_bytes,
            read_state,
        )?;
        let roots_bytes = self
            .reader
            .read_member_with_caller_state(self.revision, &tuple.path, read_state)
            .map_err(|error| cut_io_error("V2 source member read failed", error))?
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::MissingMember,
                    "V2 source member disappeared",
                )
            })?;
        if roots_bytes.sha256 != tuple.sha256
            || roots_bytes.size_bytes != tuple.size_bytes
            || Digest256::of_bytes(&roots_bytes.bytes) != tuple.sha256
        {
            return Err(cut_refusal(
                StoreErrorCode::CorruptSelectedObject,
                "V2 source member digest differs",
            ));
        }
        let raw = roots_bytes.bytes.clone();
        feed_source_membership_v1(
            &mut self.actual,
            tuple.path.as_str(),
            roots_bytes.bytes.len() as u64,
            Digest256::of_bytes(&roots_bytes.bytes),
        );
        let mut after = Vec::new();
        after
            .try_reserve_exact(tuple.path.as_str().len())
            .map_err(|_| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member cursor path allocation",
                )
            })?;
        after.extend_from_slice(tuple.path.as_str().as_bytes());
        self.after = Some(after);
        self.count += 1;
        self.bytes = next_bytes;
        check_cut_clock(deadline, cancelled)?;
        Ok(Some(SourceMemberV1 {
            path: tuple.path,
            raw,
            revision: self.revision,
            stable_ids,
        }))
    }
}

fn v2_cut_revision(roots: &SourceRevisionRootsV2) -> tos_source_store::Result<SourceCutRevision> {
    Ok(SourceCutRevision {
        format: SourceCutFormat::NativeAdmissionV2,
        revision: roots.revision,
        base_revision: roots.base_revision,
        validator_sha256: roots.validator_sha256,
        member_count: roots.member_count,
        source_bytes: roots.source_bytes,
        identity_count: roots.identity_count,
        dependency_source_count: roots.dependency_source_count,
        dependency_count: roots.dependency_count,
        retirement_count: roots.retirement_count,
        membership: v2_membership(roots)?,
    })
}

fn v2_membership(roots: &SourceRevisionRootsV2) -> tos_source_store::Result<SourceCutMembership> {
    Ok(SourceCutMembership::NativeAdmissionV2 {
        rows: roots.membership_v1.ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::UnsupportedFormat,
                "V2 ordered row witness is absent from this selected revision",
            )
        })?,
        members_tree_commitment: roots.members.commitment,
    })
}

fn membership_hasher_v1() -> Digest256Hasher {
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-val-full-membership-v1\0");
    hasher
}

fn v2_indexed_ids_for_path(
    reader: &mut V2ReadSession,
    baseline_caller_retained_state_bytes: usize,
    max_state_bytes: usize,
    max_key_bytes: usize,
    revision: SourceRevision,
    path: &RelativePath,
    max_rows: u64,
    caller_retained_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_source_store::Result<Vec<String>> {
    check_cut_clock(deadline, cancelled)?;
    let mut lower = Vec::new();
    lower
        .try_reserve_exact(path.as_str().len().saturating_add(1))
        .map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 identity prefix allocation",
            )
        })?;
    lower.extend_from_slice(path.as_str().as_bytes());
    lower.push(0);
    let mut upper = Vec::new();
    upper
        .try_reserve_exact(lower.len().saturating_add(1))
        .map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 identity range allocation",
            )
        })?;
    upper.extend_from_slice(&lower);
    upper.push(u8::MAX);
    let mut after: Option<Vec<u8>> = None;
    let mut ids = Vec::new();
    let mut owned_id_bytes = 0usize;
    let identity_lookup_state = reader
        .identity_lookup_result_state_upper_bound()
        .map_err(|error| cut_io_error("V2 identity lookup state unavailable", error))?;
    let row_result_state = max_key_bytes.checked_add(256).ok_or_else(|| {
        cut_refusal(
            StoreErrorCode::BudgetExceeded,
            "V2 identity row state overflow",
        )
    })?;
    loop {
        let live = caller_retained_state_bytes
            .checked_add(lower.capacity())
            .and_then(|bytes| bytes.checked_add(upper.capacity()))
            .and_then(|bytes| bytes.checked_add(after.as_ref().map_or(0, Vec::capacity)))
            .and_then(|bytes| bytes.checked_add(ids.capacity().checked_mul(size_of::<String>())?))
            .and_then(|bytes| bytes.checked_add(owned_id_bytes))
            .and_then(|bytes| bytes.checked_add(identity_lookup_state))
            .and_then(|bytes| bytes.checked_add(row_result_state))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity result state overflow",
                )
            })?;
        ensure_reader_cut_state(
            reader,
            baseline_caller_retained_state_bytes,
            max_state_bytes,
            live,
        )?;
        let row = reader
            .next_row_after_with_caller_state(
                revision,
                V2RootKind::IdentityPaths,
                Some(&lower),
                Some(&upper),
                after.as_deref(),
                live,
            )
            .map_err(|error| cut_io_error("V2 path identity cursor failed", error))?;
        let Some(row) = row else {
            break;
        };
        let suffix = row
            .key
            .strip_prefix(lower.as_slice())
            .filter(|suffix| !suffix.is_empty())
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::InvalidIdentityIndex,
                    "V2 path identity key differs",
                )
            })?;
        if !row.value.is_empty() || suffix.contains(&0) {
            return Err(cut_refusal(
                StoreErrorCode::InvalidIdentityIndex,
                "V2 path identity row shape differs",
            ));
        }
        let id = std::str::from_utf8(suffix).map_err(|_| {
            cut_refusal(
                StoreErrorCode::InvalidIdentityIndex,
                "V2 identity ID is not UTF-8",
            )
        })?;
        if id.is_empty() || id.len() > 4096 || ids.len() as u64 >= max_rows {
            return Err(cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 path identity row cap exceeded",
            ));
        }
        let identity_check_state = live
            .checked_add(row.key.capacity())
            .and_then(|bytes| bytes.checked_add(row.value.capacity()))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity row state overflow",
                )
            })?;
        ensure_reader_cut_state(
            reader,
            baseline_caller_retained_state_bytes,
            max_state_bytes,
            identity_check_state,
        )?;
        let id_path = reader
            .identity_path(revision, id)
            .map_err(|error| cut_io_error("V2 reverse identity verification failed", error))?;
        if id_path.as_ref() != Some(path) {
            return Err(cut_refusal(
                StoreErrorCode::InvalidIdentityIndex,
                "V2 identity indexes disagree",
            ));
        }
        let projected_owned = owned_id_bytes.checked_add(id.len()).ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 identity result state overflow",
            )
        })?;
        let next_live = caller_retained_state_bytes
            .checked_add(lower.capacity())
            .and_then(|bytes| bytes.checked_add(upper.capacity()))
            .and_then(|bytes| bytes.checked_add(row.key.capacity()))
            .and_then(|bytes| {
                bytes.checked_add(
                    ids.capacity()
                        .checked_add(1)?
                        .checked_mul(size_of::<String>())?,
                )
            })
            .and_then(|bytes| bytes.checked_add(projected_owned))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity result state overflow",
                )
            })?;
        ensure_reader_cut_state(
            reader,
            baseline_caller_retained_state_bytes,
            max_state_bytes,
            next_live,
        )?;
        let mut owned = String::new();
        owned.try_reserve_exact(id.len()).map_err(|_| {
            cut_refusal(StoreErrorCode::BudgetExceeded, "V2 identity ID allocation")
        })?;
        owned.push_str(id);
        let next_owned = owned_id_bytes
            .checked_add(owned.capacity())
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity result state overflow",
                )
            })?;
        ids.try_reserve_exact(1).map_err(|_| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 identity result allocation",
            )
        })?;
        let reserved_live = caller_retained_state_bytes
            .checked_add(lower.capacity())
            .and_then(|bytes| bytes.checked_add(upper.capacity()))
            .and_then(|bytes| bytes.checked_add(row.key.capacity()))
            .and_then(|bytes| bytes.checked_add(ids.capacity().checked_mul(size_of::<String>())?))
            .and_then(|bytes| bytes.checked_add(next_owned))
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity result state overflow",
                )
            })?;
        ensure_reader_cut_state(
            reader,
            baseline_caller_retained_state_bytes,
            max_state_bytes,
            reserved_live,
        )?;
        ids.push(owned);
        owned_id_bytes = next_owned;
        after = Some(row.key);
        check_cut_clock(deadline, cancelled)?;
    }
    Ok(ids)
}

fn decode_dependency_row(
    key: &[u8],
    value: &[u8],
) -> tos_source_store::Result<(RelativePath, RelativePath)> {
    let split = key.iter().position(|byte| *byte == 0).ok_or_else(|| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency key lacks separator",
        )
    })?;
    if split == 0 || split + 1 >= key.len() {
        return Err(cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency key shape differs",
        ));
    }
    let source = RelativePath::parse(std::str::from_utf8(&key[..split]).map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency source is not UTF-8",
        )
    })?)
    .map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency source path differs",
        )
    })?;
    let target = RelativePath::parse(std::str::from_utf8(&key[split + 1..]).map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency target is not UTF-8",
        )
    })?)
    .map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency target path differs",
        )
    })?;
    let mut cursor = 0usize;
    let value_source = take_prefixed_path(value, &mut cursor)?;
    let value_target = take_prefixed_path(value, &mut cursor)?;
    if cursor != value.len() || value_source != source || value_target != target || source == target
    {
        return Err(cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency key/value differ",
        ));
    }
    Ok((source, target))
}

fn take_prefixed_path(value: &[u8], cursor: &mut usize) -> tos_source_store::Result<RelativePath> {
    let end_len = cursor
        .checked_add(4)
        .filter(|end| *end <= value.len())
        .ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::InvalidDependencyIndex,
                "V2 dependency tuple is truncated",
            )
        })?;
    let len = u32::from_be_bytes(value[*cursor..end_len].try_into().map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency path length differs",
        )
    })?) as usize;
    let end = end_len
        .checked_add(len)
        .filter(|end| *end <= value.len())
        .ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::InvalidDependencyIndex,
                "V2 dependency path is truncated",
            )
        })?;
    let path = RelativePath::parse(std::str::from_utf8(&value[end_len..end]).map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency path is not UTF-8",
        )
    })?)
    .map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidDependencyIndex,
            "V2 dependency path is invalid",
        )
    })?;
    *cursor = end;
    Ok(path)
}

fn decode_retirement_row(key: &[u8], value: &[u8]) -> tos_source_store::Result<RetirementMetadata> {
    if key.len() != 8 {
        return Err(cut_refusal(
            StoreErrorCode::InvalidRetirementIndex,
            "V2 retirement key width differs",
        ));
    }
    let mut cursor = 0usize;
    let path = take_prefixed_path(value, &mut cursor)?;
    let digest = take_digest(value, &mut cursor)?;
    let event_ref = take_prefixed_path(value, &mut cursor)?;
    let event_sha256 = take_digest(value, &mut cursor)?;
    let size_end = cursor
        .checked_add(8)
        .filter(|end| *end == value.len())
        .ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::InvalidRetirementIndex,
                "V2 retirement tuple width differs",
            )
        })?;
    let event_size_bytes =
        u64::from_be_bytes(value[cursor..size_end].try_into().map_err(|_| {
            cut_refusal(
                StoreErrorCode::InvalidRetirementIndex,
                "V2 retirement event size differs",
            )
        })?);
    Ok(RetirementMetadata {
        path,
        sha256: digest,
        event_ref,
        event_sha256,
        event_size_bytes,
    })
}

fn take_digest(value: &[u8], cursor: &mut usize) -> tos_source_store::Result<Digest256> {
    let end = cursor
        .checked_add(32)
        .filter(|end| *end <= value.len())
        .ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::InvalidRetirementIndex,
                "V2 retirement digest is truncated",
            )
        })?;
    let digest = Digest256::from_bytes(value[*cursor..end].try_into().map_err(|_| {
        cut_refusal(
            StoreErrorCode::InvalidRetirementIndex,
            "V2 retirement digest width differs",
        )
    })?);
    *cursor = end;
    Ok(digest)
}

fn prefix_successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut upper = prefix.to_vec();
    while let Some(last) = upper.pop() {
        if last != u8::MAX {
            upper.push(last + 1);
            return Some(upper);
        }
    }
    None
}

fn ensure_cut_state(
    session: &NativeSourceV2Session,
    caller_retained_state_bytes: usize,
) -> tos_source_store::Result<()> {
    ensure_reader_cut_state(
        &session.reader,
        session.caller_retained_state_bytes,
        session.profile.max_state_bytes,
        caller_retained_state_bytes,
    )
}

fn source_cut_readset_state_upper_bound(
    members: &[SourceCutMemberWitness],
    identities: &[SourceCutIdentityWitness],
    directories: &[SourceCutDirectoryWitness],
    dependencies: &[SourceCutDependencyWitness],
    object_refcounts: &[SourceCutObjectRefcountWitness],
) -> tos_source_store::Result<usize> {
    let mut state = members
        .len()
        .checked_mul(size_of::<SourceCutMemberWitness>())
        .and_then(|bytes| {
            bytes.checked_add(
                identities
                    .len()
                    .checked_mul(size_of::<SourceCutIdentityWitness>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                directories
                    .len()
                    .checked_mul(size_of::<SourceCutDirectoryWitness>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                dependencies
                    .len()
                    .checked_mul(size_of::<SourceCutDependencyWitness>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                object_refcounts
                    .len()
                    .checked_mul(size_of::<SourceCutObjectRefcountWitness>())?,
            )
        })
        .ok_or_else(|| {
            cut_refusal(
                StoreErrorCode::BudgetExceeded,
                "V2 source readset state overflow",
            )
        })?;
    for witness in members {
        state = state
            .checked_add(witness.path.as_str().len().saturating_mul(4))
            .and_then(|bytes| {
                bytes.checked_add(
                    witness
                        .expected_indexed_ids
                        .as_ref()
                        .map_or(0, |ids| ids.capacity().saturating_mul(size_of::<String>())),
                )
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 member witness state overflow",
                )
            })?;
        if let Some(ids) = &witness.expected_indexed_ids {
            for id in ids {
                state = state.checked_add(id.capacity()).ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 identity witness state overflow",
                    )
                })?;
            }
        }
    }
    for witness in identities {
        state = state
            .checked_add(witness.id.capacity())
            .and_then(|bytes| {
                bytes.checked_add(
                    witness
                        .expected_path
                        .as_ref()
                        .map_or(0, |path| path.as_str().len().saturating_mul(4)),
                )
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 identity witness state overflow",
                )
            })?;
    }
    for witness in directories {
        state = state
            .checked_add(witness.path.as_str().len().saturating_mul(4))
            .and_then(|bytes| {
                bytes.checked_add(
                    witness
                        .expected_children
                        .capacity()
                        .checked_mul(size_of::<(String, bool)>())?,
                )
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 directory witness state overflow",
                )
            })?;
        for (name, _) in &witness.expected_children {
            state = state.checked_add(name.capacity()).ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 directory name state overflow",
                )
            })?;
        }
    }
    for witness in dependencies {
        state = state
            .checked_add(witness.path.as_str().len().saturating_mul(4))
            .and_then(|bytes| {
                bytes.checked_add(
                    witness
                        .expected_targets
                        .capacity()
                        .checked_mul(size_of::<RelativePath>())?,
                )
            })
            .ok_or_else(|| {
                cut_refusal(
                    StoreErrorCode::BudgetExceeded,
                    "V2 dependency witness state overflow",
                )
            })?;
        for path in &witness.expected_targets {
            state = state
                .checked_add(path.as_str().len().saturating_mul(4))
                .ok_or_else(|| {
                    cut_refusal(
                        StoreErrorCode::BudgetExceeded,
                        "V2 dependency target state overflow",
                    )
                })?;
        }
    }
    Ok(state)
}

fn ensure_reader_cut_state(
    reader: &V2ReadSession,
    baseline_caller_retained_state_bytes: usize,
    max_state_bytes: usize,
    additional_live_state_bytes: usize,
) -> tos_source_store::Result<()> {
    ensure_reader_state(
        reader,
        baseline_caller_retained_state_bytes,
        max_state_bytes,
        additional_live_state_bytes,
    )
    .map_err(|_| {
        cut_refusal(
            StoreErrorCode::BudgetExceeded,
            "V2 source cursor state allowance exceeded",
        )
    })
}

fn ensure_reader_state(
    reader: &V2ReadSession,
    baseline_caller_retained_state_bytes: usize,
    max_state_bytes: usize,
    additional_live_state_bytes: usize,
) -> io::Result<()> {
    let (reader_state, _) = reader.declared_retained_state_bytes()?;
    if reader_state
        .checked_add(baseline_caller_retained_state_bytes)
        .and_then(|bytes| bytes.checked_add(additional_live_state_bytes))
        .is_none_or(|bytes| bytes > max_state_bytes)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "V2 source cursor state allowance exceeded",
        ));
    }
    Ok(())
}

fn check_cut_clock(deadline: Instant, cancelled: &AtomicBool) -> tos_source_store::Result<()> {
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) || Instant::now() >= deadline {
        Err(cut_refusal(
            StoreErrorCode::BudgetExceeded,
            "V2 source cut cancelled or expired",
        ))
    } else {
        Ok(())
    }
}

fn cut_io_error(detail: &'static str, error: io::Error) -> StoreError {
    StoreError::io(detail, error)
}

fn cut_refusal(code: StoreErrorCode, detail: &'static str) -> StoreError {
    StoreError::new(code, detail)
}

/// Bound live CLI and selected software state retained concurrently with the
/// V2 reader. Public Text supplies its owner-specific closure reservation.
pub(super) fn native_live_state_upper_bound(
    invocation_raw_bytes: usize,
    invocation: &Value,
    request_raw_bytes: usize,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
) -> SourceCommandResult<usize> {
    let invocation_state = serde_value_state_upper_bound(invocation, 0)?;
    let request_parse = request_json_state_upper_bound(request_raw_bytes)?;
    let mut software_state = size_of::<SoftwareCaptureReader>();
    for member in software.members() {
        software_state = software_state
            .checked_add(size_of::<tos_source_store::MemberMetadata>())
            .and_then(|bytes| bytes.checked_add(member.path.as_str().len().checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(192))
            .ok_or(SourceCommandError::Invalid(
                "native software state overflow",
            ))?;
    }
    for prefix in software
        .include_prefixes()
        .iter()
        .chain(software.exclude_prefixes())
        .chain(software.exclude_path_parts())
    {
        software_state = software_state
            .checked_add(
                prefix
                    .len()
                    .checked_mul(2)
                    .ok_or(SourceCommandError::Invalid(
                        "native software path state overflow",
                    ))?,
            )
            .and_then(|bytes| bytes.checked_add(64))
            .ok_or(SourceCommandError::Invalid(
                "native software state overflow",
            ))?;
    }
    let mut component_state = size_of::<SoftwareComponentSelectionV1>();
    let mut selected_component_bytes = 0usize;
    for member in components.members() {
        selected_component_bytes =
            selected_component_bytes
                .checked_add(usize::try_from(member.size_bytes).map_err(|_| {
                    SourceCommandError::Invalid("native software component size range")
                })?)
                .ok_or(SourceCommandError::Invalid(
                    "native software component size overflow",
                ))?;
        component_state = component_state
            .checked_add(size_of::<tos_source_store::MemberMetadata>())
            .and_then(|bytes| bytes.checked_add(member.path.as_str().len().checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(192))
            .ok_or(SourceCommandError::Invalid(
                "native component state overflow",
            ))?;
    }
    // The existing selected-component read profile caps all producer bytes at
    // 8 MiB. Reserve that transient output with the still-live capture/index.
    component_state = component_state
        .checked_add(selected_component_bytes)
        .and_then(|bytes| bytes.checked_add(8 * 1024 * 1024))
        .ok_or(SourceCommandError::Invalid(
            "native component state overflow",
        ))?;
    invocation_state
        .checked_add(invocation_raw_bytes)
        .and_then(|bytes| bytes.checked_add(invocation_raw_bytes.checked_mul(4)?))
        .and_then(|bytes| bytes.checked_add(request_raw_bytes))
        .and_then(|bytes| bytes.checked_add(request_parse))
        .and_then(|bytes| bytes.checked_add(software_state))
        .and_then(|bytes| bytes.checked_add(component_state))
        .ok_or(SourceCommandError::Invalid("native live state overflow"))
}

fn point_base_state_upper_bound(limits: V2PointReadLimits) -> Option<usize> {
    // Keep the pre-open admission calculation aligned with V2PointReadLimits'
    // maintained exact profile so an undersized protected selection refuses
    // before opening the physical store or materializing current roots.
    let rootset_result =
        SourceRootSetV2::retained_state_upper_bound_for_value(ROOTSET_BYTES).ok()?;
    let history_result =
        SourceRevisionRootsV2::retained_state_upper_bound_for_value(ROOTSET_BYTES).ok()?;
    let history_decode = decode_workspace_upper_bound(ROOTSET_BYTES).ok()?;
    let rootset_peak = ROOTSET_BYTES
        .checked_add(decode_workspace_upper_bound(ROOTSET_BYTES).ok()?)?
        .checked_add(rootset_result)?;
    let history_peak = rootset_result
        .checked_add(ROOTSET_BYTES)?
        .checked_add(history_decode)?
        .checked_add(history_result.checked_mul(2)?)?;
    let auxiliary = 8usize.checked_mul(1024 * 1024)?.checked_add(65_536)?;
    let tree_stack = limits.tree.max_node_bytes.checked_mul(64)?;
    let pointer_state = limits.pointer.max_manifest_bytes.checked_mul(64)?;
    let object_overlap = limits.max_object_bytes.checked_mul(2)?;
    pointer_state
        .checked_add(tree_stack)?
        .checked_add(object_overlap)?
        .checked_add(rootset_peak.max(history_peak))?
        .checked_add(auxiliary)
}

fn serde_value_state_upper_bound(value: &Value, depth: usize) -> SourceCommandResult<usize> {
    if depth > 128 {
        return Err(SourceCommandError::Invalid("native invocation state depth"));
    }
    let mut bytes = size_of::<Value>()
        .checked_add(64)
        .ok_or(SourceCommandError::Invalid(
            "native invocation state overflow",
        ))?;
    match value {
        Value::String(string) => {
            bytes = bytes
                .checked_add(string.capacity())
                .ok_or(SourceCommandError::Invalid(
                    "native invocation state overflow",
                ))?;
        }
        Value::Array(values) => {
            bytes = bytes
                .checked_add(values.capacity().checked_mul(size_of::<Value>()).ok_or(
                    SourceCommandError::Invalid("native invocation state overflow"),
                )?)
                .ok_or(SourceCommandError::Invalid(
                    "native invocation state overflow",
                ))?;
            for child in values {
                bytes = bytes
                    .checked_add(serde_value_state_upper_bound(child, depth + 1)?)
                    .ok_or(SourceCommandError::Invalid(
                        "native invocation state overflow",
                    ))?;
            }
        }
        Value::Object(values) => {
            for (key, child) in values {
                let child_bytes = serde_value_state_upper_bound(child, depth + 1)?;
                bytes = bytes
                    .checked_add(key.capacity())
                    .and_then(|value| value.checked_add(128))
                    .and_then(|value| value.checked_add(child_bytes))
                    .ok_or(SourceCommandError::Invalid(
                        "native invocation state overflow",
                    ))?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => (),
    }
    Ok(bytes)
}

fn request_json_state_upper_bound(raw_bytes: usize) -> SourceCommandResult<usize> {
    let visits = raw_bytes
        .saturating_add(1)
        .min(JsonLimits::default().max_visits);
    raw_bytes
        .checked_mul(4)
        .and_then(|bytes| {
            visits
                .checked_mul(size_of::<tos_foundation::JsonValue>().checked_add(256)?)
                .and_then(|parser| bytes.checked_add(parser))
        })
        .ok_or(SourceCommandError::Invalid(
            "native request parser state overflow",
        ))
}
