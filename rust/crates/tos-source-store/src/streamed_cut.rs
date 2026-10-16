//! Bounded, caller-workspace-backed reader for an exact V1 corpus cut.
//!
//! The private SQLite file is only a disposable index decoded from verified
//! manifests. Every returned member is still read from the held source root
//! and checked against its V1 digest and length.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use rusqlite::{Connection, ErrorCode as SqliteErrorCode, OptionalExtension, params};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, FoundationError, FoundationErrorCode, JsonLimits,
    JsonMode, JsonValue, RelativePath, SourceRevision, canonical_bytes_v1, parse_json,
};

use crate::object::{verify_digest_object, verify_selected_object};
use crate::secure_open::StoreRoot;
use crate::{
    CorpusReader, CutReadLimits, MemberMetadata, ReadLimits, Result, RetirementMetadata,
    SourceMembershipV1, SourcePresenceV1, StoreError, StoreErrorCode,
    cut::is_authored_source_path_v1, manifest::digest_field, manifest::exact_keys,
    manifest::path_field, manifest::source_mode_field, manifest::uint_field,
};

const PAGE_BYTES: u64 = 4096;
const SNAPSHOT_SCHEMA: &str = "tos_corpus_snapshot_v1";
const TOP_LEVEL_KEYS: [&str; 8] = [
    "base_revision",
    "dependencies",
    "files",
    "identities",
    "retirements",
    "revision",
    "schema_version",
    "validator_sha256",
];

/// Finite resource limits for one streamed V1 cut.
#[derive(Clone, Copy, Debug)]
pub struct StreamedCutReadLimitsV1 {
    pub cut: CutReadLimits,
    /// Whole manifest work budget for the additive streamed decoder. Each
    /// retained JSON value still obeys max_manifest_row_bytes below. This does
    /// not change CorpusReader's compatibility full-materialization limits.
    pub manifest_json: JsonLimits,
    pub max_manifest_entries: usize,
    /// Maximum allocated SQLite index file size, including its schema and indexes.
    pub max_index_bytes: u64,
    /// Maximum raw bytes retained for one JSON value or manifest row.
    pub max_manifest_row_bytes: usize,
    /// Maximum SQLite page-cache budget. This does not claim to bound process RSS.
    pub sqlite_cache_bytes: usize,
}

impl StreamedCutReadLimitsV1 {
    fn validate(self, _read: ReadLimits) -> Result<Self> {
        JsonLimits::new(
            self.manifest_json.max_bytes,
            self.manifest_json.max_depth,
            self.manifest_json.max_visits,
            self.manifest_json.max_integer_digits,
        )
        .map_err(|_| refusal("invalid streamed manifest JSON limits"))?;
        if self.cut.max_revisions == 0
            || self.cut.max_revisions == usize::MAX
            || self.cut.max_members == 0
            || self.cut.max_members == u64::MAX
            || self.cut.max_total_bytes == 0
            || self.cut.max_total_bytes == u64::MAX
            || self.cut.max_member_bytes == 0
            || self.cut.max_member_bytes == u64::MAX
            || self.max_index_bytes < PAGE_BYTES * 16
            || self.max_index_bytes == u64::MAX
            || self.max_manifest_row_bytes == 0
            || self.max_manifest_row_bytes == usize::MAX
            || self.manifest_json.max_bytes == 0
            || self.manifest_json.max_bytes == usize::MAX
            || self.manifest_json.max_visits == 0
            || self.manifest_json.max_visits == usize::MAX
            || self.max_manifest_entries == 0
            || self.max_manifest_entries == usize::MAX
            || self.max_manifest_row_bytes > self.manifest_json.max_bytes
            || self.sqlite_cache_bytes == 0
            || self.sqlite_cache_bytes == usize::MAX
        {
            return Err(refusal("invalid streamed source-cut limits"));
        }
        Ok(self)
    }
}

/// Small per-revision metadata retained by the reader. Member/index rows stay
/// in the private bounded database.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamedRevisionV1 {
    pub ordinal: u64,
    pub revision: SourceRevision,
    pub base_revision: Option<SourceRevision>,
    /// The manifest's validator/schema image digest. Its bytes remain a separate
    /// caller-owned, verified worker image.
    pub validator_sha256: Digest256,
    pub member_count: u64,
    pub identity_count: u64,
    /// Number of keys in the dependency object, including empty target lists.
    pub dependency_source_count: u64,
    /// Number of individual dependency targets.
    pub dependency_count: u64,
    pub retirement_count: u64,
    pub membership: SourceMembershipV1,
}

/// One verified content member. Unlike `SourceMemberV1`, this carries no
/// materialized stable-ID vector; callers enumerate ID claims by keyset query.
#[derive(Debug)]
pub struct StreamedSourceMemberV1 {
    pub path: RelativePath,
    pub raw: Vec<u8>,
    pub revision: SourceRevision,
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub mode: u32,
}

#[derive(Debug)]
pub struct StreamedRetiredSourceMemberV1 {
    pub revision: SourceRevision,
    pub metadata: RetirementMetadata,
    pub raw: Vec<u8>,
    pub event_raw: Vec<u8>,
}

// Close the caller backing FD before its shared reservation on error paths.
struct IndexBacking {
    file: File,
    policy: Option<Arc<dyn crate::pinned_sqlite::FdIoPolicy>>,
}

struct BudgetedReadRequest {
    io: crate::PinnedSqliteIoBudget,
    space: crate::PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

/// Exact current revision and retained bases; manifest rows live in a private index.
pub struct StreamedCorpusCutReaderV1 {
    reader: CorpusReader,
    index: crate::PinnedSqliteConnection,
    limits: StreamedCutReadLimitsV1,
    current: SourceRevision,
    revision_count: u64,
    // Declared after the connection so SQLite closes before its backing FD.
    _index_file: File,
    // After every retained backing FD: the same reservation cannot release early.
    _index_policy: Option<Arc<dyn crate::pinned_sqlite::FdIoPolicy>>,
    budgeted_request: Option<BudgetedReadRequest>,
}

impl CorpusReader {
    /// Open a complete current-to-base V1 chain into a fresh private index.
    /// The supplied file must be a distinct, empty, owner-private O_TMPFILE
    /// from the caller's workspace; it is consumed and retained for the reader's
    /// entire lifetime. No partially decoded cut is returned.
    pub fn open_source_cut_streamed(
        &self,
        current: SourceRevision,
        limits: StreamedCutReadLimitsV1,
        index_file: File,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<StreamedCorpusCutReaderV1> {
        self.open_source_cut_streamed_inner(
            current, limits, index_file, None, None, deadline, cancelled,
        )
    }

    /// Keep the strict MAIN-only index while charging manifest reads and actual
    /// pager I/O to the caller's shared ledgers. Allocated bytes are a separate
    /// finite reservation, not an I/O cap or filesystem-fit proof.
    pub fn open_source_cut_streamed_budgeted(
        &self,
        current: SourceRevision,
        limits: StreamedCutReadLimitsV1,
        index_file: File,
        io_budget: crate::PinnedSqliteIoBudget,
        space_budget: crate::PinnedSqliteSpaceBudget,
        max_index_allocated_bytes: u64,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<StreamedCorpusCutReaderV1> {
        let limits = limits.validate(self.streamed_cut_root_and_limits().1)?;
        check_time_budgeted(deadline, &cancelled, Some(&io_budget))?;
        verify_private_index_file(&index_file, true, None)?;
        let policy = crate::pinned_sqlite_aux::strict_main_policy(
            &index_file,
            io_budget.clone(),
            space_budget.clone(),
            limits.max_index_bytes,
            max_index_allocated_bytes,
            deadline,
            cancelled.clone(),
        )?;
        let mut reader = self.open_source_cut_streamed_inner(
            current,
            limits,
            index_file,
            Some(policy),
            Some(io_budget.clone()),
            deadline,
            &cancelled,
        )?;
        reader.budgeted_request = Some(BudgetedReadRequest {
            io: io_budget,
            space: space_budget,
            deadline,
            cancelled,
        });
        Ok(reader)
    }

    fn open_source_cut_streamed_inner(
        &self,
        current: SourceRevision,
        limits: StreamedCutReadLimitsV1,
        index_file: File,
        index_policy: Option<Arc<dyn crate::pinned_sqlite::FdIoPolicy>>,
        io_budget: Option<crate::PinnedSqliteIoBudget>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<StreamedCorpusCutReaderV1> {
        let backing = IndexBacking {
            file: index_file,
            policy: index_policy,
        };
        let (root, read_limits) = self.streamed_cut_root_and_limits();
        let limits = limits.validate(read_limits)?;
        check_time_budgeted(deadline, cancelled, io_budget.as_ref())?;
        let index_identity = verify_private_index_file(&backing.file, true, None)?;
        let mut index = open_index(&backing.file, limits, backing.policy.clone())?;
        verify_private_index_file(&backing.file, false, Some(index_identity))?;
        create_index_schema(&index, limits)?;

        let mut next = Some(current);
        let mut ordinal = 0u64;
        let mut aggregate = CutAggregate::default();
        while let Some(revision) = next {
            check_time_budgeted(deadline, cancelled, io_budget.as_ref())?;
            let already_loaded = index
                .query_row(
                    "SELECT 1 FROM revisions WHERE revision=?1",
                    params![revision.0.as_bytes().as_slice()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(sql_error)?
                .is_some();
            if ordinal >= limits.cut.max_revisions as u64 || already_loaded {
                return Err(refusal("source history chain exceeds budget or cycles"));
            }
            next = Some(read_exact_manifest(
                root,
                &mut index,
                &backing.file,
                revision,
                ordinal,
                read_limits,
                limits,
                &mut aggregate,
                io_budget.as_ref(),
                deadline,
                cancelled,
            )?)
            .and_then(|loaded| loaded.base_revision);
            ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| refusal("source revision count overflow"))?;
        }
        check_time_budgeted(deadline, cancelled, io_budget.as_ref())?;
        verify_private_index_file(&backing.file, false, Some(index_identity))?;
        check_index_budget(&index, &backing.file, limits)?;
        index
            .execute_batch("PRAGMA query_only=ON;")
            .map_err(sql_error)?;
        check_time_budgeted(deadline, cancelled, io_budget.as_ref())?;
        if backing
            .policy
            .as_ref()
            .is_some_and(|policy| !policy.check_operation())
            || io_budget
                .as_ref()
                .is_some_and(|budget| budget.snapshot().failure.is_some())
        {
            return Err(refusal(
                "streamed source index ended with a shared-budget failure",
            ));
        }
        Ok(StreamedCorpusCutReaderV1 {
            reader: self.clone(),
            _index_file: backing.file,
            index,
            limits,
            current,
            revision_count: ordinal,
            _index_policy: backing.policy,
            budgeted_request: None,
        })
    }
}

impl StreamedCorpusCutReaderV1 {
    /// Declared retained terms: (fixed Rust/custody bytes, selected SQLite page
    /// cache budget). These are not a complete heap bound. Opaque rusqlite,
    /// SQLite/VFS allocations, statement caches, page pins, allocator overhead
    /// and process RSS require the caller's separately admitted whole runtime
    /// envelope across all live connections. Shared request ledgers are charged
    /// by their original owner, not again here. The shared StoreRoot is included
    /// conservatively; a whole owner may deduplicate that exact held allocation.
    pub fn declared_retained_state_bytes(&self) -> Result<(usize, usize)> {
        if let Some(request) = &self.budgeted_request {
            check_time_budgeted(request.deadline, &request.cancelled, Some(&request.io))?;
        }
        let mut fixed = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<crate::secure_open::StoreRoot>())
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or_else(|| refusal("streamed declared custody state overflow"))?;
        if self._index_policy.is_some() {
            fixed = fixed
                .checked_add(crate::pinned_sqlite_aux::strict_main_declared_custody_bytes()?)
                .ok_or_else(|| refusal("streamed declared custody state overflow"))?;
        }
        Ok((fixed, self.limits.sqlite_cache_bytes))
    }

    /// Compare the constructor's resource identities, not limits or freshness.
    /// Legacy readers are never associated with a budgeted request.
    pub fn shares_budgeted_request(
        &self,
        io: &crate::PinnedSqliteIoBudget,
        space: &crate::PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
    ) -> bool {
        self.budgeted_request.as_ref().is_some_and(|request| {
            request.io.shares_with(io)
                && request.space.shares_with(space)
                && request.deadline == deadline
                && Arc::ptr_eq(&request.cancelled, cancelled)
        })
    }

    /// Validated raw manifest-row ceiling for precharging owned locator state.
    pub fn manifest_row_byte_limit(&self) -> usize {
        self.limits.max_manifest_row_bytes
    }

    /// Content ceilings verified while constructing the complete derived index.
    pub fn content_limits(&self) -> CutReadLimits {
        self.limits.cut
    }

    pub fn current_revision(&self) -> SourceRevision {
        self.current
    }

    pub fn revision_count(&self) -> u64 {
        self.revision_count
    }

    /// Current is ordinal zero; retained bases follow in exact chain order.
    pub fn revision_at(&self, ordinal: u64) -> Result<Option<StreamedRevisionV1>> {
        if ordinal >= self.revision_count {
            return Ok(None);
        }
        let key = ordinal.to_be_bytes();
        let row = self
            .index
            .query_row(
                "SELECT revision,base_revision,validator_sha256,member_count,identity_count,dependency_source_count,dependency_count,retirement_count,membership_digest FROM revisions WHERE ordinal=?1",
                params![key.as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                        row.get::<_, Vec<u8>>(5)?,
                        row.get::<_, Vec<u8>>(6)?,
                        row.get::<_, Vec<u8>>(7)?,
                        row.get::<_, Vec<u8>>(8)?,
                    ))
                },
            )
            .map_err(sql_error)?;
        let (
            revision,
            base,
            validator,
            members,
            identities,
            dependency_sources,
            dependencies,
            retirements,
            membership,
        ) = row;
        let revision = SourceRevision(decode_digest(&revision)?);
        let base_revision = base
            .as_deref()
            .map(decode_digest)
            .transpose()?
            .map(SourceRevision);
        Ok(Some(StreamedRevisionV1 {
            ordinal,
            revision,
            base_revision,
            validator_sha256: decode_digest(&validator)?,
            member_count: decode_u64(&members)?,
            identity_count: decode_u64(&identities)?,
            dependency_source_count: decode_u64(&dependency_sources)?,
            dependency_count: decode_u64(&dependencies)?,
            retirement_count: decode_u64(&retirements)?,
            membership: SourceMembershipV1 {
                count: decode_u64(&members)?,
                digest: decode_digest(&membership)?,
            },
        }))
    }

    pub fn revision(&self, revision: SourceRevision) -> Result<Option<StreamedRevisionV1>> {
        let row = self
            .index
            .query_row(
                "SELECT ordinal FROM revisions WHERE revision=?1",
                params![revision.0.as_bytes().as_slice()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(sql_error)?;
        match row {
            Some(ordinal) => self.revision_at(decode_u64(&ordinal)?),
            None => Ok(None),
        }
    }

    pub fn member(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
    ) -> Result<Option<MemberMetadata>> {
        self.require_revision(revision)?;
        let row = self
            .index
            .query_row(
                "SELECT sha256,size_bytes,mode FROM members WHERE revision=?1 AND path=?2",
                params![revision.0.as_bytes().as_slice(), path.as_str()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sql_error)?;
        row.map(|(digest, size, mode)| {
            let mode =
                u32::try_from(mode).map_err(|_| mismatch("private source index mode changed"))?;
            if !matches!(mode, 0o600 | 0o644 | 0o755) {
                return Err(mismatch("private source index mode changed"));
            }
            Ok(MemberMetadata {
                path: path.clone(),
                sha256: decode_digest(&digest)?,
                size_bytes: decode_u64(&size)?,
                mode,
            })
        })
        .transpose()
    }

    pub fn member_after(
        &self,
        revision: SourceRevision,
        after: Option<&RelativePath>,
    ) -> Result<Option<MemberMetadata>> {
        self.require_revision(revision)?;
        let row = match after {
            Some(after) => self
                .index
                .query_row(
                    "SELECT path,sha256,size_bytes,mode FROM members WHERE revision=?1 AND path>?2 ORDER BY path LIMIT 1",
                    params![revision.0.as_bytes().as_slice(), after.as_str()],
                    member_row,
                )
                .optional()
                .map_err(sql_error)?,
            None => self
                .index
                .query_row(
                    "SELECT path,sha256,size_bytes,mode FROM members WHERE revision=?1 ORDER BY path LIMIT 1",
                    params![revision.0.as_bytes().as_slice()],
                    member_row,
                )
                .optional()
                .map_err(sql_error)?,
        };
        row.map(decode_member_row).transpose()
    }

    pub fn identity_path(
        &self,
        revision: SourceRevision,
        id: &str,
    ) -> Result<Option<RelativePath>> {
        self.require_revision(revision)?;
        let path = self
            .index
            .query_row(
                "SELECT path FROM identities WHERE revision=?1 AND id=?2",
                params![revision.0.as_bytes().as_slice(), id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sql_error)?;
        path.map(decode_path).transpose()
    }

    /// Look up an identity without allocating an owned path before checking
    /// its caller allowance. This covers the requested path bytes and inline
    /// RelativePath state, not SQLite pager or allocator/RSS overhead.
    pub fn identity_path_bounded(
        &self,
        revision: SourceRevision,
        id: &str,
        max_owned_state_bytes: usize,
    ) -> Result<Option<RelativePath>> {
        self.require_revision(revision)?;
        let path = self
            .index
            .query_row(
                "SELECT path FROM identities WHERE revision=?1 AND id=?2",
                params![revision.0.as_bytes().as_slice(), id],
                |row| {
                    let value = row.get_ref(0)?;
                    Ok(match value {
                        rusqlite::types::ValueRef::Text(bytes) => {
                            let state =
                                bytes.len().checked_add(std::mem::size_of::<RelativePath>());
                            if state.is_none_or(|state| state > max_owned_state_bytes) {
                                Err(refusal(
                                    "private source identity path exceeds owned-state allowance",
                                ))
                            } else {
                                std::str::from_utf8(bytes)
                                    .map_err(|_| mismatch("private source index path changed"))
                                    .and_then(|path| {
                                        RelativePath::parse(path).map_err(|_| {
                                            mismatch("private source index path changed")
                                        })
                                    })
                            }
                        }
                        _ => Err(mismatch("private source index path changed")),
                    })
                },
            )
            .optional()
            .map_err(sql_error)?;
        path.transpose()
    }

    pub fn identity_after(
        &self,
        revision: SourceRevision,
        after_id: Option<&str>,
    ) -> Result<Option<(String, RelativePath)>> {
        self.require_revision(revision)?;
        let row = match after_id {
            Some(after) => self
                .index
                .query_row(
                    "SELECT id,path FROM identities WHERE revision=?1 AND id>?2 ORDER BY id LIMIT 1",
                    params![revision.0.as_bytes().as_slice(), after],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(sql_error)?,
            None => self
                .index
                .query_row(
                    "SELECT id,path FROM identities WHERE revision=?1 ORDER BY id LIMIT 1",
                    params![revision.0.as_bytes().as_slice()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(sql_error)?,
        };
        row.map(|(id, path)| Ok((id, decode_path(path)?)))
            .transpose()
    }

    pub fn identity_for_path_after(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
        after_id: Option<&str>,
    ) -> Result<Option<String>> {
        self.require_revision(revision)?;
        let id = match after_id {
            Some(after) => self
                .index
                .query_row(
                    "SELECT id FROM identities WHERE revision=?1 AND path=?2 AND id>?3 ORDER BY id LIMIT 1",
                    params![revision.0.as_bytes().as_slice(), path.as_str(), after],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sql_error)?,
            None => self
                .index
                .query_row(
                    "SELECT id FROM identities WHERE revision=?1 AND path=?2 ORDER BY id LIMIT 1",
                    params![revision.0.as_bytes().as_slice(), path.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sql_error)?,
        };
        Ok(id)
    }

    pub fn dependency_after(
        &self,
        revision: SourceRevision,
        source: &RelativePath,
        after_target: Option<&RelativePath>,
    ) -> Result<Option<RelativePath>> {
        self.require_revision(revision)?;
        let target = match after_target {
            Some(after) => self
                .index
                .query_row(
                    "SELECT target FROM dependencies WHERE revision=?1 AND source=?2 AND target>?3 ORDER BY target LIMIT 1",
                    params![revision.0.as_bytes().as_slice(), source.as_str(), after.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sql_error)?,
            None => self
                .index
                .query_row(
                    "SELECT target FROM dependencies WHERE revision=?1 AND source=?2 ORDER BY target LIMIT 1",
                    params![revision.0.as_bytes().as_slice(), source.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sql_error)?,
        };
        target.map(decode_path).transpose()
    }

    /// Iterate declared dependency-index keys in lexical order, retaining
    /// explicit empty target lists from the V1 manifest.
    pub fn dependency_source_after(
        &self,
        revision: SourceRevision,
        after_source: Option<&RelativePath>,
    ) -> Result<Option<RelativePath>> {
        self.require_revision(revision)?;
        let source = match after_source {
            Some(after) => self
                .index
                .query_row(
                    "SELECT source FROM dependency_sources WHERE revision=?1 AND source>?2 ORDER BY source LIMIT 1",
                    params![revision.0.as_bytes().as_slice(), after.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sql_error)?,
            None => self
                .index
                .query_row(
                    "SELECT source FROM dependency_sources WHERE revision=?1 ORDER BY source LIMIT 1",
                    params![revision.0.as_bytes().as_slice()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sql_error)?,
        };
        source.map(decode_path).transpose()
    }

    pub fn has_dependency_index(
        &self,
        revision: SourceRevision,
        source: &RelativePath,
    ) -> Result<bool> {
        self.require_revision(revision)?;
        self.index
            .query_row(
                "SELECT 1 FROM dependency_sources WHERE revision=?1 AND source=?2",
                params![revision.0.as_bytes().as_slice(), source.as_str()],
                |_| Ok(()),
            )
            .optional()
            .map(|row| row.is_some())
            .map_err(sql_error)
    }

    pub fn retirement_at(
        &self,
        revision: SourceRevision,
        ordinal: u64,
    ) -> Result<Option<RetirementMetadata>> {
        self.require_revision(revision)?;
        let ordinal = ordinal.to_be_bytes();
        let row = self
            .index
            .query_row(
                "SELECT path,sha256,event_ref,event_sha256,event_size_bytes FROM retirements WHERE revision=?1 AND ordinal=?2",
                params![revision.0.as_bytes().as_slice(), ordinal.as_slice()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(sql_error)?;
        row.map(|(path, sha, event_ref, event_sha, event_size)| {
            Ok(RetirementMetadata {
                path: decode_path(path)?,
                sha256: decode_digest(&sha)?,
                event_ref: decode_path(event_ref)?,
                event_sha256: decode_digest(&event_sha)?,
                event_size_bytes: decode_u64(&event_size)?,
            })
        })
        .transpose()
    }

    pub fn presence(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
    ) -> Result<Option<SourcePresenceV1>> {
        self.require_revision(revision)?;
        if self.member(revision, path)?.is_some() {
            return Ok(Some(SourcePresenceV1::File));
        }
        let lower = format!("{}/", path.as_str());
        let upper = format!("{}0", path.as_str());
        let found = self
            .index
            .query_row(
                "SELECT 1 FROM members WHERE revision=?1 AND path>=?2 AND path<?3 LIMIT 1",
                params![revision.0.as_bytes().as_slice(), lower, upper],
                |_| Ok(()),
            )
            .optional()
            .map_err(sql_error)?
            .is_some();
        Ok(found.then_some(SourcePresenceV1::MaterializedDirectory))
    }

    /// Read one selected object from the securely held source root and verify
    /// exact manifest digest and size before returning its bytes.
    pub fn read_member(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<StreamedSourceMemberV1> {
        check_time(deadline, cancelled)?;
        let metadata = self.member(revision, path)?.ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::MissingMember,
                "path is absent from exact source revision",
            )
        })?;
        let cap = max_bytes.min(self.limits.cut.max_member_bytes).min(
            self.reader
                .streamed_cut_root_and_limits()
                .1
                .max_selected_object_bytes,
        );
        if metadata.size_bytes > cap {
            return Err(refusal("selected source member exceeds read limit"));
        }
        let mut stage = TimedStage::new(cap, deadline, cancelled);
        verify_selected_object(
            self.reader.streamed_cut_root_and_limits().0,
            metadata.sha256,
            metadata.size_bytes,
            cap,
            &mut stage,
        )?;
        check_time(deadline, cancelled)?;
        Ok(StreamedSourceMemberV1 {
            path: metadata.path,
            raw: stage.raw,
            revision,
            sha256: metadata.sha256,
            size_bytes: metadata.size_bytes,
            mode: metadata.mode,
        })
    }

    pub fn read_retirement(
        &self,
        revision: SourceRevision,
        ordinal: u64,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<StreamedRetiredSourceMemberV1> {
        check_time(deadline, cancelled)?;
        let metadata = self.retirement_at(revision, ordinal)?.ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::MissingMember,
                "retirement is outside exact source revision",
            )
        })?;
        let cap = max_bytes.min(self.limits.cut.max_member_bytes).min(
            self.reader
                .streamed_cut_root_and_limits()
                .1
                .max_selected_object_bytes,
        );
        let root = self.reader.streamed_cut_root_and_limits().0;
        let mut raw = TimedStage::new(cap, deadline, cancelled);
        verify_digest_object(root, metadata.sha256, None, cap, &mut raw)?;
        let mut event = TimedStage::new(cap, deadline, cancelled);
        verify_selected_object(
            root,
            metadata.event_sha256,
            metadata.event_size_bytes,
            cap,
            &mut event,
        )?;
        check_time(deadline, cancelled)?;
        Ok(StreamedRetiredSourceMemberV1 {
            revision,
            metadata,
            raw: raw.raw,
            event_raw: event.raw,
        })
    }

    pub fn stream(&self, revision: SourceRevision) -> Result<StreamedSourceMemberStreamV1<'_>> {
        self.stream_with_member_limit(revision, self.limits.cut.max_member_bytes)
    }

    /// Stream the same exact membership with a narrower per-member read cap.
    /// The caller cap is intersected with both cut and selected-object limits,
    /// so oversized metadata is rejected before any member bytes are allocated.
    pub fn stream_with_member_limit(
        &self,
        revision: SourceRevision,
        max_member_bytes: u64,
    ) -> Result<StreamedSourceMemberStreamV1<'_>> {
        if max_member_bytes == 0 {
            return Err(refusal("source stream member limit must be positive"));
        }
        let member_limit = max_member_bytes.min(self.limits.cut.max_member_bytes).min(
            self.reader
                .streamed_cut_root_and_limits()
                .1
                .max_selected_object_bytes,
        );
        if member_limit == 0 {
            return Err(refusal("source stream member limit must be positive"));
        }
        let expected = self
            .revision(revision)?
            .ok_or_else(|| {
                StoreError::new(
                    StoreErrorCode::MissingRevision,
                    "revision is outside opened source cut",
                )
            })?
            .membership;
        let mut actual = Digest256Hasher::new();
        actual.update(b"tos-val-full-membership-v1\0");
        Ok(StreamedSourceMemberStreamV1 {
            cut: self,
            revision,
            last: None,
            count: 0,
            bytes: 0,
            actual,
            expected,
            member_limit,
            complete: false,
            failed: false,
        })
    }

    fn require_revision(&self, revision: SourceRevision) -> Result<()> {
        if self.revision(revision)?.is_some() {
            Ok(())
        } else {
            Err(StoreError::new(
                StoreErrorCode::MissingRevision,
                "revision is outside opened source cut",
            ))
        }
    }
}

pub struct StreamedSourceMemberStreamV1<'a> {
    cut: &'a StreamedCorpusCutReaderV1,
    revision: SourceRevision,
    last: Option<RelativePath>,
    count: u64,
    bytes: u64,
    actual: Digest256Hasher,
    expected: SourceMembershipV1,
    member_limit: u64,
    complete: bool,
    failed: bool,
}

impl StreamedSourceMemberStreamV1<'_> {
    pub fn expectation(&self) -> SourceMembershipV1 {
        self.expected
    }

    /// Coverage is available only after the caller reads through exact EOF.
    pub fn coverage(&self) -> Option<SourceMembershipV1> {
        self.complete.then_some(self.expected)
    }

    pub fn next_member(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<StreamedSourceMemberV1>> {
        if self.failed {
            return Err(refusal("source stream already refused"));
        }
        let result = self.next_inner(deadline, cancelled);
        if result.is_err() {
            self.failed = true;
            self.complete = false;
        }
        result
    }

    fn next_inner(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<StreamedSourceMemberV1>> {
        check_time(deadline, cancelled)?;
        if self.complete {
            return Ok(None);
        }
        let Some(metadata) = self.cut.member_after(self.revision, self.last.as_ref())? else {
            if self.count != self.expected.count
                || self.actual.clone().finalize() != self.expected.digest
            {
                return Err(mismatch("source stream membership differs"));
            }
            self.complete = true;
            return Ok(None);
        };
        if metadata.size_bytes > self.member_limit {
            return Err(refusal("source stream member exceeds per-member limit"));
        }
        let next_bytes = self
            .bytes
            .checked_add(metadata.size_bytes)
            .ok_or_else(|| refusal("source stream byte count overflow"))?;
        if self.count >= self.cut.limits.cut.max_members
            || next_bytes > self.cut.limits.cut.max_total_bytes
        {
            return Err(refusal("source stream exceeds declared budget"));
        }
        let member = self.cut.read_member(
            self.revision,
            &metadata.path,
            self.member_limit,
            deadline,
            cancelled,
        )?;
        feed_member(
            &mut self.actual,
            member.path.as_str(),
            member.raw.len() as u64,
            Digest256::of_bytes(&member.raw),
        );
        self.last = Some(member.path.clone());
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| refusal("source stream count overflow"))?;
        self.bytes = next_bytes;
        Ok(Some(member))
    }
}

#[derive(Default)]
struct CutAggregate {
    members: u64,
    bytes: u64,
}

fn read_exact_manifest(
    root: &StoreRoot,
    index: &mut Connection,
    index_file: &File,
    revision: SourceRevision,
    ordinal: u64,
    _read_limits: ReadLimits,
    limits: StreamedCutReadLimitsV1,
    aggregate: &mut CutAggregate,
    io_budget: Option<&crate::PinnedSqliteIoBudget>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<StreamedRevisionV1> {
    let dir = root.open_revision(&revision.0.to_hex()).map_err(|error| {
        if is_not_found(&error) {
            StoreError::new(
                StoreErrorCode::MissingRevision,
                "exact corpus revision is absent",
            )
        } else {
            error
        }
    })?;
    let manifest = root.open_manifest(&dir).map_err(|error| {
        if is_not_found(&error) {
            StoreError::new(
                StoreErrorCode::MissingRevision,
                "exact corpus snapshot is absent",
            )
        } else {
            error
        }
    })?;
    let manifest_cap = limits.manifest_json.max_bytes;
    let row_cap = limits.max_manifest_row_bytes.min(manifest_cap);
    let mut cursor = ManifestCursor::new(
        manifest,
        manifest_cap,
        row_cap,
        io_budget,
        deadline,
        cancelled,
    );
    cursor.expect_byte(b'{')?;

    let ordinal_key = ordinal.to_be_bytes();
    let zero = 0u64.to_be_bytes();
    index
        .execute(
            "INSERT INTO revisions(ordinal,revision,base_revision,validator_sha256,member_count,identity_count,dependency_source_count,dependency_count,retirement_count,membership_digest) VALUES(?1,?2,NULL,NULL,?3,?3,?3,?3,?3,?4)",
            params![
                ordinal_key.as_slice(),
                revision.0.as_bytes().as_slice(),
                zero.as_slice(),
                Digest256::of_bytes(b"tos-val-full-membership-v1\0").as_bytes().as_slice()
            ],
        )
        .map_err(sql_error)?;

    let mut body = Digest256Hasher::new();
    body.update(b"{");
    let mut body_fields = 0usize;
    let mut visits = 0usize;
    record_visits(&mut visits, 1, 0, limits.manifest_json)?; // top-level object
    let mut base_revision = None;
    let mut validator_sha256 = None;
    let mut manifest_revision = None;
    let mut member_count = 0u64;
    let mut identity_count = 0u64;
    let mut dependency_source_count = 0u64;
    let mut dependency_count = 0u64;
    let mut retirement_count = 0u64;
    let mut remaining_entries = limits.max_manifest_entries;
    let mut membership = Digest256Hasher::new();
    membership.update(b"tos-val-full-membership-v1\0");

    for (top_index, expected_key) in TOP_LEVEL_KEYS.iter().enumerate() {
        check_time_budgeted(deadline, cancelled, io_budget)?;
        if top_index != 0 {
            cursor.expect_byte(b',')?;
        }
        let raw_key = cursor.read_value(row_cap)?;
        let key = parse_fragment_string(
            &raw_key,
            row_cap,
            limits.manifest_json,
            0,
            &mut visits,
            false,
        )?;
        if key != *expected_key {
            return Err(StoreError::new(
                StoreErrorCode::InvalidCanonicalSnapshot,
                "corpus snapshot top-level keys are missing, duplicated or unsorted",
            ));
        }
        cursor.expect_byte(b':')?;
        if key != "revision" {
            if body_fields != 0 {
                body.update(b",");
            }
            body.update(&raw_key);
            body.update(b":");
            body_fields += 1;
        }

        match key.as_str() {
            "base_revision" => {
                let raw = cursor.read_value(row_cap)?;
                let value = parse_fragment(&raw, row_cap, limits.manifest_json, 1, &mut visits)?;
                base_revision = if matches!(&value, JsonValue::Null) {
                    None
                } else {
                    Some(SourceRevision(parse_digest_value(
                        &value,
                        StoreErrorCode::UnsupportedFormat,
                    )?))
                };
                body.update(&raw);
            }
            "dependencies" => {
                record_visits(&mut visits, 1, 1, limits.manifest_json)?;
                cursor.expect_byte(b'{')?;
                body.update(b"{");
                let mut previous_source: Option<String> = None;
                let mut first = true;
                loop {
                    check_time_budgeted(deadline, cancelled, io_budget)?;
                    if cursor.peek_byte()? == Some(b'}') {
                        cursor.expect_byte(b'}')?;
                        body.update(b"}");
                        break;
                    }
                    if !first {
                        cursor.expect_byte(b',')?;
                        body.update(b",");
                    }
                    first = false;
                    let raw_source = cursor.read_value(row_cap)?;
                    let source_text = parse_fragment_string(
                        &raw_source,
                        row_cap,
                        limits.manifest_json,
                        1,
                        &mut visits,
                        false,
                    )?;
                    if previous_source
                        .as_ref()
                        .is_some_and(|previous| source_text <= *previous)
                    {
                        return Err(StoreError::new(
                            StoreErrorCode::InvalidDependencyIndex,
                            "dependency sources must be strictly sorted and unique",
                        ));
                    }
                    let source = RelativePath::parse(&source_text).map_err(|_| {
                        StoreError::new(
                            StoreErrorCode::InvalidDependencyIndex,
                            "dependency source is unsafe",
                        )
                    })?;
                    consume_entry(&mut remaining_entries)?;
                    index
                        .execute(
                            "INSERT INTO dependency_sources(revision,source) VALUES(?1,?2)",
                            params![revision.0.as_bytes().as_slice(), source.as_str()],
                        )
                        .map_err(sql_error)?;
                    dependency_source_count =
                        checked_inc(dependency_source_count, "dependency source count overflow")?;
                    cursor.expect_byte(b':')?;
                    body.update(&raw_source);
                    body.update(b":");
                    record_visits(&mut visits, 1, 2, limits.manifest_json)?;
                    cursor.expect_byte(b'[')?;
                    body.update(b"[");
                    let mut previous_target: Option<RelativePath> = None;
                    let mut first_target = true;
                    loop {
                        check_time_budgeted(deadline, cancelled, io_budget)?;
                        if cursor.peek_byte()? == Some(b']') {
                            cursor.expect_byte(b']')?;
                            body.update(b"]");
                            break;
                        }
                        if !first_target {
                            cursor.expect_byte(b',')?;
                            body.update(b",");
                        }
                        first_target = false;
                        let raw_target = cursor.read_value(row_cap)?;
                        let target_text = parse_fragment_string(
                            &raw_target,
                            row_cap,
                            limits.manifest_json,
                            3,
                            &mut visits,
                            true,
                        )?;
                        let target = RelativePath::parse(&target_text).map_err(|_| {
                            StoreError::new(
                                StoreErrorCode::InvalidDependencyIndex,
                                "dependency target is unsafe",
                            )
                        })?;
                        if previous_target
                            .as_ref()
                            .is_some_and(|prior| target <= *prior)
                        {
                            return Err(StoreError::new(
                                StoreErrorCode::InvalidDependencyIndex,
                                "dependency targets are absent, duplicate or unsorted",
                            ));
                        }
                        consume_entry(&mut remaining_entries)?;
                        index
                            .execute(
                                "INSERT INTO dependencies(revision,source,target) VALUES(?1,?2,?3)",
                                params![
                                    revision.0.as_bytes().as_slice(),
                                    source.as_str(),
                                    target.as_str()
                                ],
                            )
                            .map_err(sql_error)?;
                        dependency_count =
                            checked_inc(dependency_count, "dependency count overflow")?;
                        previous_target = Some(target);
                        body.update(&raw_target);
                    }
                    previous_source = Some(source_text);
                    if dependency_source_count & 0xff == 0 {
                        check_index_budget(index, index_file, limits)?;
                    }
                }
            }
            "files" => {
                record_visits(&mut visits, 1, 1, limits.manifest_json)?;
                cursor.expect_byte(b'[')?;
                body.update(b"[");
                let mut previous: Option<RelativePath> = None;
                let mut first = true;
                loop {
                    check_time_budgeted(deadline, cancelled, io_budget)?;
                    if cursor.peek_byte()? == Some(b']') {
                        cursor.expect_byte(b']')?;
                        body.update(b"]");
                        break;
                    }
                    if !first {
                        cursor.expect_byte(b',')?;
                        body.update(b",");
                    }
                    first = false;
                    let raw = cursor.read_value(row_cap)?;
                    let value =
                        parse_fragment(&raw, row_cap, limits.manifest_json, 2, &mut visits)?;
                    exact_keys(
                        &value,
                        &["path", "sha256", "size_bytes", "mode"],
                        StoreErrorCode::InvalidMemberIndex,
                    )?;
                    let path = path_field(&value, "path", StoreErrorCode::InvalidMemberIndex)?;
                    if previous.as_ref().is_some_and(|prior| path <= *prior) {
                        return Err(StoreError::new(
                            StoreErrorCode::InvalidMemberIndex,
                            "snapshot members must be strictly sorted and unique",
                        ));
                    }
                    if !is_authored_source_path_v1(path.as_str()) {
                        return Err(StoreError::new(
                            StoreErrorCode::InvalidMemberIndex,
                            "member is outside the source admission carrier",
                        ));
                    }
                    let metadata = MemberMetadata {
                        path: path.clone(),
                        sha256: digest_field(&value, "sha256", StoreErrorCode::InvalidMemberIndex)?,
                        size_bytes: uint_field(
                            &value,
                            "size_bytes",
                            StoreErrorCode::InvalidMemberIndex,
                        )?,
                        mode: source_mode_field(
                            &value,
                            "mode",
                            StoreErrorCode::InvalidMemberIndex,
                        )?,
                    };
                    consume_entry(&mut remaining_entries)?;
                    aggregate.members = aggregate
                        .members
                        .checked_add(1)
                        .ok_or_else(|| refusal("source member count overflow"))?;
                    aggregate.bytes = aggregate
                        .bytes
                        .checked_add(metadata.size_bytes)
                        .ok_or_else(|| refusal("source byte count overflow"))?;
                    if aggregate.members > limits.cut.max_members
                        || aggregate.bytes > limits.cut.max_total_bytes
                        || metadata.size_bytes > limits.cut.max_member_bytes
                    {
                        return Err(refusal("source cut exceeds declared budget"));
                    }
                    let size = metadata.size_bytes.to_be_bytes();
                    index
                        .execute(
                            "INSERT INTO members(revision,path,sha256,size_bytes,mode) VALUES(?1,?2,?3,?4,?5)",
                            params![
                                revision.0.as_bytes().as_slice(),
                                metadata.path.as_str(),
                                metadata.sha256.as_bytes().as_slice(),
                                size.as_slice(),
                                i64::from(metadata.mode)
                            ],
                        )
                        .map_err(sql_error)?;
                    member_count = checked_inc(member_count, "source member count overflow")?;
                    feed_member(
                        &mut membership,
                        path.as_str(),
                        metadata.size_bytes,
                        metadata.sha256,
                    );
                    previous = Some(path);
                    body.update(&raw);
                    if member_count & 0xff == 0 {
                        check_index_budget(index, index_file, limits)?;
                    }
                }
                validate_dependencies(index, revision, io_budget, deadline, cancelled)?;
            }
            "identities" => {
                record_visits(&mut visits, 1, 1, limits.manifest_json)?;
                cursor.expect_byte(b'{')?;
                body.update(b"{");
                let mut previous_id: Option<String> = None;
                let mut first = true;
                loop {
                    check_time_budgeted(deadline, cancelled, io_budget)?;
                    if cursor.peek_byte()? == Some(b'}') {
                        cursor.expect_byte(b'}')?;
                        body.update(b"}");
                        break;
                    }
                    if !first {
                        cursor.expect_byte(b',')?;
                        body.update(b",");
                    }
                    first = false;
                    let raw_id = cursor.read_value(row_cap)?;
                    let id = parse_fragment_string(
                        &raw_id,
                        row_cap,
                        limits.manifest_json,
                        1,
                        &mut visits,
                        false,
                    )?;
                    if id.trim().is_empty()
                        || previous_id.as_ref().is_some_and(|previous| id <= *previous)
                    {
                        return Err(StoreError::new(
                            StoreErrorCode::InvalidIdentityIndex,
                            "identities must be nonempty, strictly sorted and unique",
                        ));
                    }
                    cursor.expect_byte(b':')?;
                    body.update(&raw_id);
                    body.update(b":");
                    let raw_path = cursor.read_value(row_cap)?;
                    let value =
                        parse_fragment(&raw_path, row_cap, limits.manifest_json, 2, &mut visits)?;
                    let path_text = match value {
                        JsonValue::String(string) => {
                            string.as_str().map(str::to_owned).ok_or_else(|| {
                                StoreError::new(
                                    StoreErrorCode::InvalidIdentityIndex,
                                    "identity path is invalid",
                                )
                            })?
                        }
                        _ => {
                            return Err(StoreError::new(
                                StoreErrorCode::InvalidIdentityIndex,
                                "identity path is invalid",
                            ));
                        }
                    };
                    let path = RelativePath::parse(&path_text).map_err(|_| {
                        StoreError::new(
                            StoreErrorCode::InvalidIdentityIndex,
                            "identity path is unsafe",
                        )
                    })?;
                    if !sqlite_member_exists(index, revision, &path)? {
                        return Err(StoreError::new(
                            StoreErrorCode::InvalidIdentityIndex,
                            "identity points outside snapshot",
                        ));
                    }
                    consume_entry(&mut remaining_entries)?;
                    index
                        .execute(
                            "INSERT INTO identities(revision,id,path) VALUES(?1,?2,?3)",
                            params![revision.0.as_bytes().as_slice(), id, path.as_str()],
                        )
                        .map_err(sql_error)?;
                    identity_count = checked_inc(identity_count, "identity count overflow")?;
                    previous_id = Some(id);
                    body.update(&raw_path);
                }
            }
            "retirements" => {
                record_visits(&mut visits, 1, 1, limits.manifest_json)?;
                cursor.expect_byte(b'[')?;
                body.update(b"[");
                let mut first = true;
                loop {
                    check_time_budgeted(deadline, cancelled, io_budget)?;
                    if cursor.peek_byte()? == Some(b']') {
                        cursor.expect_byte(b']')?;
                        body.update(b"]");
                        break;
                    }
                    if !first {
                        cursor.expect_byte(b',')?;
                        body.update(b",");
                    }
                    first = false;
                    let raw = cursor.read_value(row_cap)?;
                    let value =
                        parse_fragment(&raw, row_cap, limits.manifest_json, 2, &mut visits)?;
                    exact_keys(
                        &value,
                        &[
                            "path",
                            "sha256",
                            "event_ref",
                            "event_sha256",
                            "event_size_bytes",
                        ],
                        StoreErrorCode::InvalidRetirementIndex,
                    )?;
                    let path = path_field(&value, "path", StoreErrorCode::InvalidRetirementIndex)?;
                    let sha =
                        digest_field(&value, "sha256", StoreErrorCode::InvalidRetirementIndex)?;
                    let event_ref =
                        path_field(&value, "event_ref", StoreErrorCode::InvalidRetirementIndex)?;
                    let event_sha = digest_field(
                        &value,
                        "event_sha256",
                        StoreErrorCode::InvalidRetirementIndex,
                    )?;
                    let event_size = uint_field(
                        &value,
                        "event_size_bytes",
                        StoreErrorCode::InvalidRetirementIndex,
                    )?;
                    if path == event_ref
                        || !is_authored_source_path_v1(path.as_str())
                        || !is_authored_source_path_v1(event_ref.as_str())
                    {
                        return Err(StoreError::new(
                            StoreErrorCode::InvalidRetirementIndex,
                            "retirement is self-referential or outside source carrier",
                        ));
                    }
                    consume_entry(&mut remaining_entries)?;
                    aggregate.members = aggregate
                        .members
                        .checked_add(2)
                        .ok_or_else(|| refusal("retirement member count overflow"))?;
                    aggregate.bytes = aggregate
                        .bytes
                        .checked_add(limits.cut.max_member_bytes)
                        .and_then(|value| value.checked_add(event_size))
                        .ok_or_else(|| refusal("retirement byte count overflow"))?;
                    if aggregate.members > limits.cut.max_members
                        || aggregate.bytes > limits.cut.max_total_bytes
                        || event_size > limits.cut.max_member_bytes
                    {
                        return Err(refusal("retirement cut exceeds declared budget"));
                    }
                    let entry_ordinal = retirement_count.to_be_bytes();
                    let event_size_bytes = event_size.to_be_bytes();
                    let inserted = index.execute(
                        "INSERT OR IGNORE INTO retirements(revision,ordinal,path,sha256,event_ref,event_sha256,event_size_bytes) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                        params![
                            revision.0.as_bytes().as_slice(),
                            entry_ordinal.as_slice(),
                            path.as_str(),
                            sha.as_bytes().as_slice(),
                            event_ref.as_str(),
                            event_sha.as_bytes().as_slice(),
                            event_size_bytes.as_slice()
                        ],
                    ).map_err(sql_error)?;
                    if inserted != 1 {
                        return Err(StoreError::new(
                            StoreErrorCode::InvalidRetirementIndex,
                            "retirement is duplicate",
                        ));
                    }
                    retirement_count = checked_inc(retirement_count, "retirement count overflow")?;
                    body.update(&raw);
                    if retirement_count & 0xff == 0 {
                        check_index_budget(index, index_file, limits)?;
                    }
                }
            }
            "revision" => {
                let raw = cursor.read_value(row_cap)?;
                let value = parse_fragment(&raw, row_cap, limits.manifest_json, 1, &mut visits)?;
                manifest_revision = Some(SourceRevision(parse_digest_value(
                    &value,
                    StoreErrorCode::RevisionMismatch,
                )?));
            }
            "schema_version" => {
                let raw = cursor.read_value(row_cap)?;
                let value = parse_fragment(&raw, row_cap, limits.manifest_json, 1, &mut visits)?;
                if value.as_str() != Some(SNAPSHOT_SCHEMA) {
                    return Err(StoreError::new(
                        StoreErrorCode::UnsupportedFormat,
                        "unsupported corpus snapshot",
                    ));
                }
                body.update(&raw);
            }
            "validator_sha256" => {
                let raw = cursor.read_value(row_cap)?;
                let value = parse_fragment(&raw, row_cap, limits.manifest_json, 1, &mut visits)?;
                validator_sha256 = Some(parse_digest_value(
                    &value,
                    StoreErrorCode::UnsupportedFormat,
                )?);
                body.update(&raw);
            }
            _ => unreachable!("top-level key array is closed"),
        }
    }
    cursor.expect_byte(b'}')?;
    cursor.expect_byte(b'\n')?;
    cursor.ensure_eof()?;
    body.update(b"}\n");
    let computed_revision = body.finalize();
    if manifest_revision != Some(revision) || computed_revision != revision.0 {
        return Err(StoreError::new(
            StoreErrorCode::RevisionMismatch,
            "corpus revision digest differs",
        ));
    }
    let validator_sha256 = validator_sha256.ok_or_else(|| {
        StoreError::new(
            StoreErrorCode::UnsupportedFormat,
            "validator digest is absent",
        )
    })?;
    let membership = SourceMembershipV1 {
        count: member_count,
        digest: membership.finalize(),
    };
    let base_blob = base_revision.map(|base| base.0.as_bytes().to_vec());
    let member_blob = member_count.to_be_bytes();
    let identity_blob = identity_count.to_be_bytes();
    let dependency_blob = dependency_count.to_be_bytes();
    let dependency_source_blob = dependency_source_count.to_be_bytes();
    let retirement_blob = retirement_count.to_be_bytes();
    index
        .execute(
            "UPDATE revisions SET base_revision=?1,validator_sha256=?2,member_count=?3,identity_count=?4,dependency_source_count=?5,dependency_count=?6,retirement_count=?7,membership_digest=?8 WHERE revision=?9",
            params![
                base_blob,
                validator_sha256.as_bytes().as_slice(),
                member_blob.as_slice(),
                identity_blob.as_slice(),
                dependency_source_blob.as_slice(),
                dependency_blob.as_slice(),
                retirement_blob.as_slice(),
                membership.digest.as_bytes().as_slice(),
                revision.0.as_bytes().as_slice()
            ],
        )
        .map_err(sql_error)?;
    check_index_budget(index, index_file, limits)?;
    Ok(StreamedRevisionV1 {
        ordinal,
        revision,
        base_revision,
        validator_sha256,
        member_count,
        identity_count,
        dependency_source_count,
        dependency_count,
        retirement_count,
        membership,
    })
}

pub(crate) struct ManifestRead<'a> {
    file: File,
    io_budget: Option<&'a crate::PinnedSqliteIoBudget>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl Read for ManifestRead<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if let Some(budget) = self.io_budget {
            budget
                .charge_read(output.len() as u64)
                .map_err(|_| io::Error::other("source manifest read budget exceeded"))?;
        }
        self.check_running()?;
        let result = self.file.read(output);
        match result {
            Ok(count) => {
                if let Some(budget) = self.io_budget {
                    budget
                        .record_read_returned(count as u64)
                        .map_err(|_| io::Error::other("source manifest read accounting failed"))?;
                }
                self.check_running()?;
                Ok(count)
            }
            Err(error) => {
                if error.kind() != io::ErrorKind::Interrupted {
                    if let Some(budget) = self.io_budget {
                        budget.fail(crate::PinnedSqliteIoFailure::Io);
                    }
                }
                Err(error)
            }
        }
    }
}

impl<'a> ManifestRead<'a> {
    pub(crate) fn new(
        file: File,
        io_budget: Option<&'a crate::PinnedSqliteIoBudget>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self {
            file,
            io_budget,
            deadline,
            cancelled,
        }
    }
    fn check_running(&self) -> io::Result<()> {
        let failure = if Instant::now() >= self.deadline {
            Some(crate::PinnedSqliteIoFailure::Deadline)
        } else if self.cancelled.load(Ordering::Acquire) {
            Some(crate::PinnedSqliteIoFailure::Cancelled)
        } else {
            None
        };
        if let Some(failure) = failure {
            if let Some(budget) = self.io_budget {
                budget.fail(failure);
            }
            return Err(io::Error::other("source manifest read stopped"));
        }
        Ok(())
    }
}

struct ManifestCursor<'a> {
    reader: BufReader<ManifestRead<'a>>,
    consumed: usize,
    max_bytes: usize,
    max_row_bytes: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl<'a> ManifestCursor<'a> {
    fn new(
        file: File,
        max_bytes: usize,
        max_row_bytes: usize,
        io_budget: Option<&'a crate::PinnedSqliteIoBudget>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self {
            reader: BufReader::with_capacity(
                max_bytes.min(64 * 1024).max(1),
                ManifestRead {
                    file,
                    io_budget,
                    deadline,
                    cancelled,
                },
            ),
            consumed: 0,
            max_bytes,
            max_row_bytes,
            deadline,
            cancelled,
        }
    }

    fn peek_byte(&mut self) -> Result<Option<u8>> {
        check_time_budgeted(
            self.deadline,
            self.cancelled,
            self.reader.get_ref().io_budget,
        )?;
        self.peek_byte_unchecked()
    }

    fn peek_byte_unchecked(&mut self) -> Result<Option<u8>> {
        self.reader
            .fill_buf()
            .map(|bytes| bytes.first().copied())
            .map_err(|error| StoreError::io("cannot read corpus manifest", error))
    }

    fn next_byte(&mut self) -> Result<u8> {
        if self.consumed & 0x0fff == 0 {
            check_time_budgeted(
                self.deadline,
                self.cancelled,
                self.reader.get_ref().io_budget,
            )?;
        }
        let byte = self.peek_byte_unchecked()?.ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::InvalidCanonicalSnapshot,
                "truncated corpus manifest",
            )
        })?;
        self.reader.consume(1);
        self.consumed = self
            .consumed
            .checked_add(1)
            .ok_or_else(|| refusal("manifest byte count overflow"))?;
        if self.consumed > self.max_bytes {
            return Err(refusal("corpus manifest exceeds read limit"));
        }
        Ok(byte)
    }

    fn expect_byte(&mut self, expected: u8) -> Result<()> {
        if self.next_byte()? != expected {
            return Err(StoreError::new(
                StoreErrorCode::InvalidCanonicalSnapshot,
                "corpus manifest is not canonical v1 JSON",
            ));
        }
        Ok(())
    }

    fn read_value(&mut self, cap: usize) -> Result<Vec<u8>> {
        let mut raw = Vec::new();
        let first = self.peek_byte()?.ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::InvalidCanonicalSnapshot,
                "truncated corpus manifest",
            )
        })?;
        match first {
            b'"' => self.read_string_into(&mut raw, cap)?,
            b'{' | b'[' => {
                let mut stack = Vec::with_capacity(128);
                let mut in_string = false;
                let mut escaped = false;
                loop {
                    let byte = self.next_byte()?;
                    push_capped(&mut raw, byte, cap)?;
                    if in_string {
                        if escaped {
                            escaped = false;
                        } else if byte == b'\\' {
                            escaped = true;
                        } else if byte == b'"' {
                            in_string = false;
                        }
                        continue;
                    }
                    match byte {
                        b'"' => in_string = true,
                        b'{' | b'[' => {
                            stack.push(byte);
                            if stack.len() > 128 {
                                return Err(refusal("corpus JSON depth exceeds safe limit"));
                            }
                        }
                        b'}' | b']' => {
                            let opening = stack.pop().ok_or_else(|| {
                                StoreError::new(
                                    StoreErrorCode::InvalidCanonicalSnapshot,
                                    "invalid JSON container",
                                )
                            })?;
                            if !matches!((opening, byte), (b'{', b'}') | (b'[', b']')) {
                                return Err(StoreError::new(
                                    StoreErrorCode::InvalidCanonicalSnapshot,
                                    "invalid JSON container",
                                ));
                            }
                            if stack.is_empty() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => loop {
                match self.peek_byte()? {
                    Some(b',' | b']' | b'}') | None => break,
                    Some(_) => push_capped(&mut raw, self.next_byte()?, cap)?,
                }
            },
        }
        if raw.is_empty() {
            return Err(StoreError::new(
                StoreErrorCode::InvalidCanonicalSnapshot,
                "empty JSON value",
            ));
        }
        Ok(raw)
    }

    fn read_string_into(&mut self, out: &mut Vec<u8>, cap: usize) -> Result<()> {
        // Consume the opening delimiter separately: only a later unescaped
        // quote closes the string (including an empty string).
        self.expect_byte(b'"')?;
        push_capped(out, b'"', cap)?;
        let mut escaped = false;
        loop {
            let byte = self.next_byte()?;
            push_capped(out, byte, cap)?;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                return Ok(());
            }
        }
    }

    fn ensure_eof(&mut self) -> Result<()> {
        if self.peek_byte()?.is_some() {
            if self.consumed >= self.max_bytes {
                return Err(refusal("corpus manifest exceeds read limit"));
            }
            return Err(StoreError::new(
                StoreErrorCode::InvalidCanonicalSnapshot,
                "corpus manifest has trailing bytes",
            ));
        }
        Ok(())
    }
}

fn parse_fragment(
    raw: &[u8],
    max_bytes: usize,
    limits: JsonLimits,
    depth_offset: usize,
    visits: &mut usize,
) -> Result<JsonValue> {
    let value = parse_fragment_unaccounted(raw, max_bytes, limits)?;
    let (nodes, depth) = json_shape(&value);
    if depth.saturating_add(depth_offset) > limits.max_depth {
        return Err(refusal("corpus JSON depth budget exceeded"));
    }
    record_visits(visits, nodes, depth_offset, limits)?;
    Ok(value)
}

fn parse_fragment_string(
    raw: &[u8],
    max_bytes: usize,
    limits: JsonLimits,
    depth_offset: usize,
    visits: &mut usize,
    value_fragment: bool,
) -> Result<String> {
    let value = parse_fragment_unaccounted(raw, max_bytes, limits)?;
    let (nodes, depth) = json_shape(&value);
    if depth.saturating_add(depth_offset) > limits.max_depth {
        return Err(refusal("corpus JSON depth budget exceeded"));
    }
    if value_fragment {
        record_visits(visits, nodes, depth_offset, limits)?;
    }
    match value {
        JsonValue::String(value) => value.as_str().map(str::to_owned).ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::InvalidCanonicalSnapshot,
                "JSON string is not a Unicode scalar string",
            )
        }),
        _ => Err(StoreError::new(
            StoreErrorCode::InvalidCanonicalSnapshot,
            "expected JSON string",
        )),
    }
}

fn parse_fragment_unaccounted(
    raw: &[u8],
    max_bytes: usize,
    mut limits: JsonLimits,
) -> Result<JsonValue> {
    limits.max_bytes = limits.max_bytes.min(max_bytes);
    let parsed = parse_json(raw, JsonMode::PublishedStrict, limits).map_err(canonical_error)?;
    let value = parsed.into_root();
    let canonical = canonical_bytes_v1(&value, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(canonical_error)?;
    if canonical != raw {
        return Err(StoreError::new(
            StoreErrorCode::InvalidCanonicalSnapshot,
            "corpus manifest is not canonical v1 JSON",
        ));
    }
    Ok(value)
}

fn json_shape(value: &JsonValue) -> (usize, usize) {
    match value {
        JsonValue::Array(values) => {
            let mut nodes = 1usize;
            let mut depth = 0usize;
            for value in values {
                let (child_nodes, child_depth) = json_shape(value);
                nodes = nodes.saturating_add(child_nodes);
                depth = depth.max(child_depth.saturating_add(1));
            }
            (nodes, depth)
        }
        JsonValue::Object(values) => {
            let mut nodes = 1usize;
            let mut depth = 0usize;
            for (_, value) in values {
                let (child_nodes, child_depth) = json_shape(value);
                nodes = nodes.saturating_add(child_nodes);
                depth = depth.max(child_depth.saturating_add(1));
            }
            (nodes, depth)
        }
        _ => (1, 0),
    }
}

fn record_visits(visits: &mut usize, add: usize, depth: usize, limits: JsonLimits) -> Result<()> {
    if depth > limits.max_depth {
        return Err(refusal("corpus JSON depth budget exceeded"));
    }
    *visits = visits
        .checked_add(add)
        .filter(|used| *used <= limits.max_visits)
        .ok_or_else(|| refusal("corpus JSON structural budget exceeded"))?;
    Ok(())
}

fn parse_digest_value(value: &JsonValue, code: StoreErrorCode) -> Result<Digest256> {
    let text = value
        .as_str()
        .ok_or_else(|| StoreError::new(code, "corpus digest field is invalid"))?;
    Digest256::from_hex(text).map_err(|_| StoreError::new(code, "corpus digest field is invalid"))
}

fn consume_entry(remaining: &mut usize) -> Result<()> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| refusal("corpus index exceeds entry limit"))?;
    Ok(())
}

fn sqlite_member_exists(
    index: &Connection,
    revision: SourceRevision,
    path: &RelativePath,
) -> Result<bool> {
    index
        .query_row(
            "SELECT 1 FROM members WHERE revision=?1 AND path=?2",
            params![revision.0.as_bytes().as_slice(), path.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(sql_error)
}

fn validate_dependencies(
    index: &Connection,
    revision: SourceRevision,
    io_budget: Option<&crate::PinnedSqliteIoBudget>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let mut source_statement = index
        .prepare("SELECT source FROM dependency_sources WHERE revision=?1 ORDER BY source")
        .map_err(sql_error)?;
    let mut sources = source_statement
        .query(params![revision.0.as_bytes().as_slice()])
        .map_err(sql_error)?;
    while let Some(row) = sources.next().map_err(sql_error)? {
        check_time_budgeted(deadline, cancelled, io_budget)?;
        let source = decode_path(row.get::<_, String>(0).map_err(sql_error)?)?;
        if !sqlite_member_exists(index, revision, &source)? {
            return Err(StoreError::new(
                StoreErrorCode::InvalidDependencyIndex,
                "dependency source is absent",
            ));
        }
    }
    drop(sources);
    drop(source_statement);

    let mut target_statement = index
        .prepare("SELECT target FROM dependencies WHERE revision=?1 ORDER BY source,target")
        .map_err(sql_error)?;
    let mut targets = target_statement
        .query(params![revision.0.as_bytes().as_slice()])
        .map_err(sql_error)?;
    while let Some(row) = targets.next().map_err(sql_error)? {
        check_time_budgeted(deadline, cancelled, io_budget)?;
        let target = decode_path(row.get::<_, String>(0).map_err(sql_error)?)?;
        if !sqlite_member_exists(index, revision, &target)? {
            return Err(StoreError::new(
                StoreErrorCode::InvalidDependencyIndex,
                "dependency target is absent",
            ));
        }
    }
    Ok(())
}

fn open_index(
    file: &File,
    limits: StreamedCutReadLimitsV1,
    policy: Option<Arc<dyn crate::pinned_sqlite::FdIoPolicy>>,
) -> Result<crate::PinnedSqliteConnection> {
    let index = crate::PinnedSqliteConnection::open_private_derived_with_policy(file, policy)?;
    let cache_kib = limits
        .sqlite_cache_bytes
        .div_ceil(1024)
        .min(i64::MAX as usize);
    index
        .pragma_update(None, "page_size", PAGE_BYTES as i64)
        .map_err(sql_error)?;
    index
        .pragma_update(None, "journal_mode", "OFF")
        .map_err(sql_error)?;
    index
        .pragma_update(None, "synchronous", "OFF")
        .map_err(sql_error)?;
    index
        .pragma_update(None, "temp_store", "MEMORY")
        .map_err(sql_error)?;
    index
        .pragma_update(None, "mmap_size", 0i64)
        .map_err(sql_error)?;
    index
        .pragma_update(None, "cache_size", -(cache_kib as i64))
        .map_err(sql_error)?;
    index
        .pragma_update(None, "locking_mode", "EXCLUSIVE")
        .map_err(sql_error)?;
    index
        .pragma_update(None, "trusted_schema", "OFF")
        .map_err(sql_error)?;
    index
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(sql_error)?;
    let max_pages = i64::try_from(limits.max_index_bytes / PAGE_BYTES)
        .map_err(|_| refusal("source index page limit overflow"))?;
    index
        .pragma_update(None, "max_page_count", max_pages)
        .map_err(sql_error)?;
    let page_size: u64 = index
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(sql_error)?;
    let actual_max_pages: i64 = index
        .query_row("PRAGMA max_page_count", [], |row| row.get(0))
        .map_err(sql_error)?;
    let cache_setting: i64 = index
        .query_row("PRAGMA cache_size", [], |row| row.get(0))
        .map_err(sql_error)?;
    let temp_store: i64 = index
        .query_row("PRAGMA temp_store", [], |row| row.get(0))
        .map_err(sql_error)?;
    let cache_bytes = cache_setting.unsigned_abs().checked_mul(1024);
    if page_size != PAGE_BYTES
        || actual_max_pages > max_pages
        || cache_setting >= 0
        || cache_bytes.map_or(true, |bytes| bytes > limits.sqlite_cache_bytes as u64)
        || temp_store != 2
    {
        return Err(refusal("private source index limits could not be enforced"));
    }
    let journal_mode: String = index
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(sql_error)?;
    if !journal_mode.eq_ignore_ascii_case("off") {
        return Err(StoreError::new(
            StoreErrorCode::UnsupportedPlatform,
            "private source index cannot disable journal sidecars",
        ));
    }
    Ok(index)
}

fn create_index_schema(index: &Connection, limits: StreamedCutReadLimitsV1) -> Result<()> {
    index
        .execute_batch(
            "CREATE TABLE revisions(ordinal BLOB PRIMARY KEY CHECK(length(ordinal)=8),revision BLOB NOT NULL UNIQUE CHECK(length(revision)=32),base_revision BLOB CHECK(base_revision IS NULL OR length(base_revision)=32),validator_sha256 BLOB CHECK(validator_sha256 IS NULL OR length(validator_sha256)=32),member_count BLOB NOT NULL CHECK(length(member_count)=8),identity_count BLOB NOT NULL CHECK(length(identity_count)=8),dependency_source_count BLOB NOT NULL CHECK(length(dependency_source_count)=8),dependency_count BLOB NOT NULL CHECK(length(dependency_count)=8),retirement_count BLOB NOT NULL CHECK(length(retirement_count)=8),membership_digest BLOB NOT NULL CHECK(length(membership_digest)=32)) WITHOUT ROWID;\
             CREATE TABLE members(revision BLOB NOT NULL,path TEXT NOT NULL,sha256 BLOB NOT NULL CHECK(length(sha256)=32),size_bytes BLOB NOT NULL CHECK(length(size_bytes)=8),mode INTEGER NOT NULL,PRIMARY KEY(revision,path)) WITHOUT ROWID;\
             CREATE TABLE identities(revision BLOB NOT NULL,id TEXT NOT NULL,path TEXT NOT NULL,PRIMARY KEY(revision,id)) WITHOUT ROWID;\
             CREATE INDEX identities_by_path ON identities(revision,path,id);\
             CREATE TABLE dependency_sources(revision BLOB NOT NULL,source TEXT NOT NULL,PRIMARY KEY(revision,source)) WITHOUT ROWID;\
             CREATE TABLE dependencies(revision BLOB NOT NULL,source TEXT NOT NULL,target TEXT NOT NULL,PRIMARY KEY(revision,source,target)) WITHOUT ROWID;\
             CREATE TABLE retirements(revision BLOB NOT NULL,ordinal BLOB NOT NULL CHECK(length(ordinal)=8),path TEXT NOT NULL,sha256 BLOB NOT NULL CHECK(length(sha256)=32),event_ref TEXT NOT NULL,event_sha256 BLOB NOT NULL CHECK(length(event_sha256)=32),event_size_bytes BLOB NOT NULL CHECK(length(event_size_bytes)=8),PRIMARY KEY(revision,ordinal),UNIQUE(revision,path,sha256,event_ref,event_sha256)) WITHOUT ROWID;",
        )
        .map_err(sql_error)?;
    check_index_budget_from_conn_only(index, limits)
}

fn check_index_budget(
    index: &Connection,
    file: &File,
    limits: StreamedCutReadLimitsV1,
) -> Result<()> {
    check_index_budget_from_conn_only(index, limits)?;
    let length = file
        .metadata()
        .map_err(|error| StoreError::io("cannot stat private source index", error))?
        .len();
    if length > limits.max_index_bytes {
        return Err(refusal("private source index exceeds byte budget"));
    }
    Ok(())
}

fn check_index_budget_from_conn_only(
    index: &Connection,
    limits: StreamedCutReadLimitsV1,
) -> Result<()> {
    let pages: u64 = index
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .map_err(sql_error)?;
    let bytes = pages
        .checked_mul(PAGE_BYTES)
        .ok_or_else(|| refusal("source index byte count overflow"))?;
    if bytes > limits.max_index_bytes {
        return Err(refusal("private source index exceeds byte budget"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateIndexIdentity {
    device: u64,
    inode: u64,
    uid: u32,
}

fn verify_private_index_file(
    file: &File,
    require_empty: bool,
    expected: Option<PrivateIndexIdentity>,
) -> Result<PrivateIndexIdentity> {
    let metadata = file
        .metadata()
        .map_err(|error| StoreError::io("cannot inspect private source index", error))?;
    let identity = PrivateIndexIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        uid: metadata.uid(),
    };
    if !metadata.file_type().is_file()
        || metadata.nlink() != 0
        || (require_empty && metadata.len() != 0)
        || metadata.mode() & 0o777 != 0o600
        || identity.uid != crate::pinned_sqlite::current_fs_uid()?
        || expected.is_some_and(|prior| prior != identity)
    {
        return Err(StoreError::new(
            StoreErrorCode::UnsafePath,
            "source index must be a fresh owner-private unnamed regular file",
        ));
    }
    Ok(identity)
}

fn member_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, Vec<u8>, Vec<u8>, i64)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn decode_member_row(row: (String, Vec<u8>, Vec<u8>, i64)) -> Result<MemberMetadata> {
    let (path, digest, size, mode) = row;
    let mode = u32::try_from(mode).map_err(|_| mismatch("private source index mode changed"))?;
    if !matches!(mode, 0o600 | 0o644 | 0o755) {
        return Err(mismatch("private source index mode changed"));
    }
    Ok(MemberMetadata {
        path: decode_path(path)?,
        sha256: decode_digest(&digest)?,
        size_bytes: decode_u64(&size)?,
        mode,
    })
}

fn decode_path(path: String) -> Result<RelativePath> {
    RelativePath::parse(&path).map_err(|_| mismatch("private source index path changed"))
}

fn decode_digest(bytes: &[u8]) -> Result<Digest256> {
    let digest: [u8; 32] = bytes
        .try_into()
        .map_err(|_| mismatch("private source index digest changed"))?;
    Ok(Digest256::from_bytes(digest))
}

fn decode_u64(bytes: &[u8]) -> Result<u64> {
    let value: [u8; 8] = bytes
        .try_into()
        .map_err(|_| mismatch("private source index integer changed"))?;
    Ok(u64::from_be_bytes(value))
}

fn push_capped(out: &mut Vec<u8>, byte: u8, cap: usize) -> Result<()> {
    if out.len() >= cap {
        return Err(refusal("corpus manifest row exceeds read limit"));
    }
    out.try_reserve(1)
        .map_err(|_| refusal("corpus manifest row allocation failed"))?;
    out.push(byte);
    Ok(())
}

fn feed_member(hasher: &mut Digest256Hasher, path: &str, length: u64, digest: Digest256) {
    hasher.update(&(path.len() as u64).to_be_bytes());
    hasher.update(path.as_bytes());
    hasher.update(&length.to_be_bytes());
    hasher.update(digest.as_bytes());
}

struct TimedStage<'a> {
    raw: Vec<u8>,
    cap: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl<'a> TimedStage<'a> {
    fn new(cap: u64, deadline: Instant, cancelled: &'a AtomicBool) -> Self {
        Self {
            raw: Vec::new(),
            cap,
            deadline,
            cancelled,
        }
    }
}

impl Write for TimedStage<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "source read cancelled or expired",
            ));
        }
        let next = self.raw.len().checked_add(bytes.len()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "selected source member length overflow",
            )
        })?;
        if next as u64 > self.cap {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "selected source member exceeds limit",
            ));
        }
        self.raw.try_reserve(bytes.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "selected source member allocation failed",
            )
        })?;
        self.raw.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn check_time_budgeted(
    deadline: Instant,
    cancelled: &AtomicBool,
    io_budget: Option<&crate::PinnedSqliteIoBudget>,
) -> Result<()> {
    if let Some(budget) = io_budget {
        if cancelled.load(Ordering::Acquire) {
            budget.fail(crate::PinnedSqliteIoFailure::Cancelled);
        } else if Instant::now() >= deadline {
            budget.fail(crate::PinnedSqliteIoFailure::Deadline);
        }
        if budget.snapshot().failure.is_some() {
            return Err(refusal("source read stopped by shared I/O ledger"));
        }
        Ok(())
    } else {
        check_time(deadline, cancelled)
    }
}

fn check_time(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(refusal("source read cancelled or expired"))
    } else {
        Ok(())
    }
}

fn checked_inc(value: u64, detail: &'static str) -> Result<u64> {
    value.checked_add(1).ok_or_else(|| refusal(detail))
}

fn canonical_error(error: FoundationError) -> StoreError {
    let code = match error.code {
        FoundationErrorCode::BudgetExceeded => StoreErrorCode::BudgetExceeded,
        FoundationErrorCode::UnsupportedCanonicalNumber => StoreErrorCode::UnsupportedFormat,
        _ => StoreErrorCode::InvalidCanonicalSnapshot,
    };
    StoreError::new(code, "invalid canonical corpus JSON")
}

fn sql_error(error: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(sqlite, _) = &error {
        if matches!(
            sqlite.code,
            SqliteErrorCode::DiskFull | SqliteErrorCode::OutOfMemory | SqliteErrorCode::TooBig
        ) {
            return refusal("private source index exceeds declared resource budget");
        }
    }
    StoreError::io(
        "private source index operation failed",
        io::Error::new(io::ErrorKind::Other, error),
    )
}

fn is_not_found(error: &StoreError) -> bool {
    error.code == StoreErrorCode::Io
        && error
            .source
            .as_ref()
            .is_some_and(|source| source.kind() == io::ErrorKind::NotFound)
}

fn refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn mismatch(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::DescriptorMismatch, detail)
}
