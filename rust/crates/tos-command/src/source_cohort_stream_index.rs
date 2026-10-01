//! Bounded, disposable SQLite index for a complete source-cohort assessment.
//!
//! This database is only an acceleration structure for an assessment that is
//! already grounded in complete source bytes and their semantic validators.
//! It is never an authority or a source certificate. The database lives on a
//! fresh unnamed workspace inode; a missing SQLite-on-FD capability is a hard
//! refusal, with no named-file fallback.

use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, Params, Row, params};
use tos_foundation::RelativePath;

use super::cold_membership_spool::PrivateGenerationWorkspace;
use super::{DurableError, DurableResult};

const SQLITE_PAGE_BYTES: u64 = 4096;
const SQLITE_CACHE_KIB: i64 = -1024;
const MAX_SOURCE_PATH_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceAssessmentLimits {
    pub(crate) max_rows: u64,
    pub(crate) max_logical_bytes: u64,
    pub(crate) max_key_bytes: usize,
    pub(crate) max_value_bytes: usize,
    pub(crate) max_placement_bytes: usize,
    pub(crate) max_sqlite_file_bytes: u64,
    pub(crate) max_vm_steps: u64,
}

impl SourceAssessmentLimits {
    pub(crate) fn validate(self) -> DurableResult<Self> {
        if self.max_rows == 0
            || self.max_rows == u64::MAX
            || self.max_logical_bytes == 0
            || self.max_logical_bytes == u64::MAX
            || self.max_key_bytes == 0
            || self.max_key_bytes > u32::MAX as usize
            || self.max_value_bytes == 0
            || self.max_value_bytes > i32::MAX as usize
            || self.max_placement_bytes == 0
            || self.max_placement_bytes > i32::MAX as usize
            || self.max_sqlite_file_bytes < SQLITE_PAGE_BYTES
            || self.max_sqlite_file_bytes == u64::MAX
            || self.max_vm_steps == 0
            || self.max_vm_steps == u64::MAX
        {
            return Err(DurableError::Refused(
                "invalid source assessment index limits",
            ));
        }
        Ok(self)
    }
}

/// Caller-owned counters. These describe logical rows/bytes delivered to and
/// returned by this helper, SQLite progress callbacks, and observed file
/// measurements. Progress callbacks include setup/control SQL; statement
/// counts and row/byte counters do not claim protocol bytes, physical I/O, or
/// storage write amplification.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceAssessmentWork {
    pub source_input_rows_attempted: u64,
    pub source_input_logical_bytes: u64,
    pub carrier_input_rows_attempted: u64,
    pub carrier_input_logical_bytes: u64,
    pub sqlite_rows_inserted: u64,
    pub sqlite_rows_updated: u64,
    pub sqlite_rows_returned: u64,
    pub sqlite_aggregate_witnesses_returned: u64,
    pub sqlite_dependency_rows_witnessed: u64,
    pub sqlite_dependency_bytes_witnessed: u64,
    pub sqlite_logical_bytes_returned: u64,
    pub sqlite_vm_progress_callbacks: u64,
    pub sqlite_page_count_current: u64,
    pub sqlite_page_count_high_water: u64,
    pub sqlite_file_len_current: u64,
    pub sqlite_file_len_high_water: u64,
    pub sqlite_allocated_bytes_current: u64,
    pub sqlite_allocated_bytes_high_water: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceAssessmentMember<'a> {
    pub(crate) path: &'a str,
    pub(crate) sha256: [u8; 32],
    pub(crate) size_bytes: u64,
    pub(crate) mode: u32,
    /// `None` and `Some(&[])` are distinct source claims.
    pub(crate) dependencies: Option<&'a [String]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceAssessmentMemberOwned {
    pub(crate) path: String,
    pub(crate) sha256: [u8; 32],
    pub(crate) size_bytes: u64,
    pub(crate) mode: u32,
    pub(crate) dependencies: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceIdentityKind {
    Metadata,
    Form,
    Event,
    Anchor,
    Claim,
}

impl SourceIdentityKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Form => "form",
            Self::Event => "event",
            Self::Anchor => "anchor",
            Self::Claim => "claim",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceEvidenceKind {
    Event,
    Anchor,
}

impl SourceEvidenceKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Anchor => "anchor",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceAssessmentEvidenceOwned {
    pub(crate) id: String,
    pub(crate) physical_line: u64,
    pub(crate) source_sha256: [u8; 32],
    pub(crate) canonical_payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssessmentIndexRow {
    pub(crate) kind: String,
    pub(crate) token: String,
    pub(crate) path: String,
    pub(crate) definition_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssessmentPredicateRow {
    pub(crate) kind: String,
    pub(crate) owner: String,
    pub(crate) scope: String,
    pub(crate) token: String,
    pub(crate) definition_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssessmentProjectionRow {
    pub(crate) path: String,
    pub(crate) value: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceDirectoryChild {
    pub(crate) name: String,
    pub(crate) is_directory: bool,
}

/// Private bounded index. Keep `_backing_file` open for as long as SQLite is
/// using the `/proc/self/fd/N` name so the unnamed inode cannot disappear.
pub(crate) struct SourceAssessmentIndex {
    db: tos_source_store::PinnedSqliteConnection,
    _backing_file: File,
    workspace: PrivateGenerationWorkspace,
    limits: SourceAssessmentLimits,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    vm_callbacks: Arc<AtomicU64>,
    last_reported_vm_callbacks: u64,
    last_charged_pages: u64,
    member_inputs_finished: bool,
    source_inputs_finished: bool,
}

impl SourceAssessmentIndex {
    pub(crate) fn open(
        workspace: &PrivateGenerationWorkspace,
        limits: SourceAssessmentLimits,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Self> {
        let limits = limits.validate()?;
        check_active(deadline, &cancelled)?;
        let backing_file = workspace.new_private_file()?;
        validate_private_file(&backing_file)?;
        let db = tos_source_store::PinnedSqliteConnection::open_private_derived(&backing_file)
            .map_err(|error| DurableError::Refused(error.detail))?;
        let vm_callbacks = Arc::new(AtomicU64::new(0));
        let vm_observer = Arc::clone(&vm_callbacks);
        let cancelled_observer = Arc::clone(&cancelled);
        db.progress_handler(
            1,
            Some(move || {
                let step = vm_observer.fetch_add(1, Ordering::Relaxed);
                cancelled_observer.load(Ordering::Relaxed)
                    || Instant::now() >= deadline
                    || step >= limits.max_vm_steps
            }),
        );

        let mut index = Self {
            db,
            _backing_file: backing_file,
            workspace: workspace.clone(),
            limits,
            deadline,
            cancelled,
            vm_callbacks,
            last_reported_vm_callbacks: 0,
            last_charged_pages: 0,
            member_inputs_finished: false,
            source_inputs_finished: false,
        };
        index.run(true, work, |db, _work| {
            db.execute_batch(
                "PRAGMA page_size=4096;
                 PRAGMA journal_mode=OFF;
                 PRAGMA synchronous=OFF;
                 PRAGMA locking_mode=EXCLUSIVE;
                 PRAGMA temp_store=MEMORY;
                 PRAGMA mmap_size=0;
                 PRAGMA cache_size=-1024;
                 PRAGMA trusted_schema=OFF;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE members(
                   path TEXT PRIMARY KEY COLLATE BINARY,
                   sha256 BLOB NOT NULL CHECK(length(sha256)=32),
                   size_bytes INTEGER NOT NULL CHECK(size_bytes>=0),
                   mode INTEGER NOT NULL CHECK(mode>=0),
                   dependencies_present INTEGER NOT NULL CHECK(dependencies_present IN (0,1)),
                   seen INTEGER NOT NULL DEFAULT 0 CHECK(seen IN (0,1))
                 ) WITHOUT ROWID;
                 CREATE TABLE member_dependencies(
                   source_path TEXT NOT NULL COLLATE BINARY,
                   target_path TEXT NOT NULL COLLATE BINARY,
                   PRIMARY KEY(source_path,target_path)
                 ) WITHOUT ROWID;
                 CREATE INDEX member_dependencies_by_target
                   ON member_dependencies(target_path COLLATE BINARY,source_path COLLATE BINARY);
                 CREATE TABLE identity_path(
                   id TEXT PRIMARY KEY COLLATE BINARY,
                   path TEXT NOT NULL COLLATE BINARY
                 ) WITHOUT ROWID;
                 CREATE TABLE identity_kinds(
                   kind TEXT NOT NULL COLLATE BINARY,
                   id TEXT NOT NULL COLLATE BINARY,
                   path TEXT NOT NULL COLLATE BINARY,
                   PRIMARY KEY(kind,id)
                 ) WITHOUT ROWID;
                 CREATE TABLE evidence(
                   kind TEXT NOT NULL COLLATE BINARY,
                   source_ref TEXT NOT NULL COLLATE BINARY,
                   id TEXT NOT NULL COLLATE BINARY,
                   physical_line INTEGER NOT NULL CHECK(physical_line>0),
                   source_sha256 BLOB NOT NULL CHECK(length(source_sha256)=32),
                   canonical_payload BLOB NOT NULL,
                   PRIMARY KEY(kind,id)
                 ) WITHOUT ROWID;
                 CREATE INDEX evidence_source_kind_id
                   ON evidence(source_ref COLLATE BINARY,kind COLLATE BINARY,id COLLATE BINARY);
                 CREATE TABLE expected_index(
                   kind TEXT NOT NULL COLLATE BINARY,
                   token TEXT NOT NULL COLLATE BINARY,
                   path TEXT NOT NULL COLLATE BINARY,
                   definition_digest TEXT NOT NULL COLLATE BINARY,
                   seen INTEGER NOT NULL DEFAULT 0 CHECK(seen IN (0,1)),
                   PRIMARY KEY(kind,token)
                 ) WITHOUT ROWID;
                 CREATE TABLE expected_predicate(
                   kind TEXT NOT NULL COLLATE BINARY,
                   owner TEXT NOT NULL COLLATE BINARY,
                   scope TEXT NOT NULL COLLATE BINARY,
                   token TEXT NOT NULL COLLATE BINARY,
                   definition_version TEXT NOT NULL COLLATE BINARY,
                   seen INTEGER NOT NULL DEFAULT 0 CHECK(seen IN (0,1)),
                   PRIMARY KEY(kind,owner,scope,token)
                 ) WITHOUT ROWID;
                 CREATE TABLE observed_predicate(
                   kind TEXT NOT NULL COLLATE BINARY,
                   owner TEXT NOT NULL COLLATE BINARY,
                   scope TEXT NOT NULL COLLATE BINARY,
                   token TEXT NOT NULL COLLATE BINARY,
                   definition_version TEXT NOT NULL COLLATE BINARY,
                   PRIMARY KEY(kind,owner,scope,token)
                 ) WITHOUT ROWID;
                 CREATE TABLE expected_projection(
                   path TEXT PRIMARY KEY COLLATE BINARY,
                   is_present INTEGER NOT NULL CHECK(is_present IN (0,1)),
                   value BLOB,
                   seen INTEGER NOT NULL DEFAULT 0 CHECK(seen IN (0,1)),
                   CHECK((is_present=0 AND value IS NULL) OR (is_present=1 AND value IS NOT NULL))
                 ) WITHOUT ROWID;
                 CREATE TABLE current_placement(
                   path TEXT PRIMARY KEY COLLATE BINARY,
                   encoded_row BLOB NOT NULL
                 ) WITHOUT ROWID;",
            )
            .map_err(sqlite_error)?;
            Ok(())
        })?;
        index.snapshot_usage(work)?;
        Ok(index)
    }

    pub(crate) fn add_member(
        &mut self,
        member: SourceAssessmentMember<'_>,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_member_open()?;
        let dependency_rows = member.dependencies.map_or(0, |rows| rows.len());
        self.account_source_input_rows(
            work,
            member_logical_bytes(member.path, member.dependencies)?,
            1usize
                .checked_add(dependency_rows)
                .ok_or(DurableError::Refused(
                    "source assessment row count overflow",
                ))?,
        )?;
        validate_path(member.path, self.limits.max_key_bytes)?;
        check_i64(member.size_bytes, "source member size exceeds SQLite range")?;
        if member.mode > 0o7777 {
            return Err(DurableError::Corrupt("source member mode malformed"));
        }
        validate_dependencies(member.dependencies, self.limits.max_key_bytes)?;
        let dependencies_present = i64::from(member.dependencies.is_some());
        let deadline = self.deadline;
        let cancelled = Arc::clone(&self.cancelled);
        self.run(true, work, |db, work| {
            let changed = db
                .execute(
                    "INSERT INTO members(path,sha256,size_bytes,mode,dependencies_present)
                     VALUES(?1,?2,?3,?4,?5)",
                    params![
                        member.path,
                        member.sha256.as_slice(),
                        as_i64(member.size_bytes)?,
                        i64::from(member.mode),
                        dependencies_present
                    ],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt("source member insert count differs"));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            if let Some(dependencies) = member.dependencies {
                for dependency in dependencies {
                    check_active(deadline, &cancelled)?;
                    let changed = db
                        .execute(
                            "INSERT INTO member_dependencies(source_path,target_path) VALUES(?1,?2)",
                            params![member.path, dependency],
                        )
                        .map_err(sqlite_error)?;
                    if changed != 1 {
                        return Err(DurableError::Corrupt(
                            "source dependency insert count differs",
                        ));
                    }
                    add_counter(&mut work.sqlite_rows_inserted, 1)?;
                }
            }
            Ok(())
        })
    }

    pub(crate) fn add_identity(
        &mut self,
        kind: SourceIdentityKind,
        id: &str,
        path: &str,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_open()?;
        self.account_source_input(work, string_row_bytes(&[kind.as_str(), id, path])?)?;
        validate_key_text(id, self.limits.max_key_bytes, "source identity malformed")?;
        validate_path(path, self.limits.max_key_bytes)?;
        self.run(true, work, |db, work| {
            let existing_path = query_optional_text(
                db,
                "SELECT path FROM identity_path WHERE id=?1",
                params![id],
                work,
            )?;
            if existing_path.as_deref().is_some_and(|prior| prior != path) {
                return Err(DurableError::Conflict(
                    "stable identity occurs on different source members",
                ));
            }
            let existing_kind_path = query_optional_text(
                db,
                "SELECT path FROM identity_kinds WHERE kind=?1 AND id=?2",
                params![kind.as_str(), id],
                work,
            )?;
            if let Some(existing) = existing_kind_path {
                if existing != path {
                    return Err(DurableError::Conflict(
                        "owner identity occurs on different source members",
                    ));
                }
                return Ok(());
            }
            if existing_path.is_none() {
                let changed = db
                    .execute(
                        "INSERT INTO identity_path(id,path) VALUES(?1,?2)",
                        params![id, path],
                    )
                    .map_err(sqlite_error)?;
                if changed != 1 {
                    return Err(DurableError::Corrupt("identity path insert count differs"));
                }
                add_counter(&mut work.sqlite_rows_inserted, 1)?;
            }
            let changed = db
                .execute(
                    "INSERT INTO identity_kinds(kind,id,path) VALUES(?1,?2,?3)",
                    params![kind.as_str(), id, path],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt("identity kind insert count differs"));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            Ok(())
        })
    }

    pub(crate) fn add_evidence_row(
        &mut self,
        kind: SourceEvidenceKind,
        source_member_path: &str,
        id: &str,
        physical_line: u64,
        source_sha256: [u8; 32],
        canonical_payload: &[u8],
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_open()?;
        let bytes = string_row_bytes(&[kind.as_str(), source_member_path, id])?
            .checked_add(8 + 32)
            .and_then(|sum| sum.checked_add(canonical_payload.len()))
            .ok_or(DurableError::Refused(
                "source assessment input size overflow",
            ))?;
        self.account_source_input(work, bytes)?;
        validate_path(source_member_path, self.limits.max_key_bytes)?;
        validate_key_text(
            id,
            self.limits.max_key_bytes,
            "source evidence identity malformed",
        )?;
        check_i64(physical_line, "source evidence line exceeds SQLite range")?;
        if physical_line == 0 || canonical_payload.len() > self.limits.max_value_bytes {
            return Err(DurableError::Corrupt("source evidence row malformed"));
        }
        if tos_foundation::Digest256::of_bytes(canonical_payload).as_bytes() != &source_sha256 {
            return Err(DurableError::Corrupt(
                "source evidence payload digest differs",
            ));
        }
        self.run(true, work, |db, work| {
            let changed = db
                .execute(
                    "INSERT INTO evidence(kind,source_ref,id,physical_line,source_sha256,canonical_payload)
                     VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        kind.as_str(),
                        source_member_path,
                        id,
                        as_i64(physical_line)?,
                        source_sha256.as_slice(),
                        canonical_payload
                    ],
                )
                .map_err(|_| DurableError::Corrupt("duplicate or invalid source evidence row"))?;
            if changed != 1 {
                return Err(DurableError::Corrupt("source evidence insert count differs"));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            Ok(())
        })
    }

    pub(crate) fn add_index_row(
        &mut self,
        row: &AssessmentIndexRow,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_open()?;
        let bytes = string_row_bytes(&[&row.kind, &row.token, &row.path, &row.definition_digest])?;
        self.account_source_input(work, bytes)?;
        validate_index_row(row, self.limits)?;
        self.run(true, work, |db, work| {
            let existing = query_optional_decoded(
                db,
                "SELECT path,definition_digest FROM expected_index WHERE kind=?1 AND token=?2",
                params![row.kind, row.token],
                work,
                |r| {
                    let path: String = r.get(0).map_err(sqlite_error)?;
                    let digest: String = r.get(1).map_err(sqlite_error)?;
                    let bytes = path
                        .len()
                        .checked_add(digest.len())
                        .ok_or(DurableError::Refused("source index read size overflow"))?;
                    Ok(((path, digest), bytes))
                },
            )?;
            if let Some((path, digest)) = existing {
                if path == row.path && digest == row.definition_digest {
                    return Ok(());
                }
                return Err(DurableError::Conflict(
                    "source index key has conflicting path or digest",
                ));
            }
            let changed = db
                .execute(
                    "INSERT INTO expected_index(kind,token,path,definition_digest)
                     VALUES(?1,?2,?3,?4)",
                    params![row.kind, row.token, row.path, row.definition_digest],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt("source index insert count differs"));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            Ok(())
        })
    }

    pub(crate) fn add_predicate_row(
        &mut self,
        row: &AssessmentPredicateRow,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_open()?;
        self.account_source_input(
            work,
            string_row_bytes(&[
                &row.kind,
                &row.owner,
                &row.scope,
                &row.token,
                &row.definition_version,
            ])?,
        )?;
        validate_predicate_row(row, self.limits)?;
        self.run(true, work, |db, work| {
            let existing = query_optional_decoded(
                db,
                "SELECT definition_version FROM expected_predicate
                     WHERE kind=?1 AND owner=?2 AND scope=?3 AND token=?4",
                params![row.kind, row.owner, row.scope, row.token],
                work,
                |r| {
                    let version: String = r.get(0).map_err(sqlite_error)?;
                    let bytes = version.len();
                    Ok((version, bytes))
                },
            )?;
            if let Some(version) = existing {
                if version == row.definition_version {
                    return Ok(());
                }
                return Err(DurableError::Conflict(
                    "source predicate key has conflicting definition version",
                ));
            }
            let changed = db
                .execute(
                    "INSERT INTO expected_predicate(kind,owner,scope,token,definition_version)
                     VALUES(?1,?2,?3,?4,?5)",
                    params![
                        row.kind,
                        row.owner,
                        row.scope,
                        row.token,
                        row.definition_version
                    ],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt(
                    "source predicate insert count differs",
                ));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            Ok(())
        })
    }

    pub(crate) fn add_projection(
        &mut self,
        row: &AssessmentProjectionRow,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_open()?;
        let bytes = string_row_bytes(&[&row.path])?
            .checked_add(row.value.as_ref().map_or(0, Vec::len))
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(DurableError::Refused(
                "source assessment input size overflow",
            ))?;
        self.account_source_input(work, bytes)?;
        validate_path(&row.path, self.limits.max_key_bytes)?;
        if row
            .value
            .as_ref()
            .is_some_and(|value| value.len() > self.limits.max_value_bytes)
        {
            return Err(DurableError::Refused("source projection exceeds bound"));
        }
        let max_value_bytes = self.limits.max_value_bytes;
        self.run(true, work, |db, work| {
            let existing = query_optional_decoded(
                db,
                "SELECT is_present,
                        CASE WHEN is_present=1 AND length(value)<=?2 THEN value ELSE NULL END,
                        length(value)
                 FROM expected_projection WHERE path=?1",
                params![row.path, max_value_bytes as i64],
                work,
                |r| {
                    let present: i64 = r.get(0).map_err(sqlite_error)?;
                    let length: Option<i64> = r.get(2).map_err(sqlite_error)?;
                    if present == 1
                        && length
                            .is_none_or(|value| value < 0 || value as u64 > max_value_bytes as u64)
                    {
                        return Err(DurableError::Corrupt(
                            "stored source projection exceeds assessment bound",
                        ));
                    }
                    let value: Option<Vec<u8>> = r.get(1).map_err(sqlite_error)?;
                    if present == 1 && value.is_none() {
                        return Err(DurableError::Corrupt(
                            "bounded source projection value absent",
                        ));
                    }
                    let bytes = value.as_ref().map_or(0, Vec::len).checked_add(16).ok_or(
                        DurableError::Refused("source projection read size overflow"),
                    )?;
                    Ok(((present, value), bytes))
                },
            )?;
            if let Some((present, value)) = existing {
                if (present != 0) == row.value.is_some() && value == row.value {
                    return Ok(());
                }
                return Err(DurableError::Conflict(
                    "source projection path has conflicting value",
                ));
            }
            let changed = db
                .execute(
                    "INSERT INTO expected_projection(path,is_present,value)
                     VALUES(?1,?2,?3)",
                    params![row.path, i64::from(row.value.is_some()), row.value],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt(
                    "source projection insert count differs",
                ));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            Ok(())
        })
    }

    /// Stream evidence rows owned by exactly one physical JSONL member. The
    /// index order is `(source_ref,kind,id)` and only one payload is retained
    /// at a time. The callback receives counters before any later row can fail.
    pub(crate) fn for_each_evidence<F>(
        &mut self,
        kind: SourceEvidenceKind,
        source_member_path: &str,
        mut visit: F,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<u64>
    where
        F: FnMut(SourceAssessmentEvidenceOwned, &mut SourceAssessmentWork) -> DurableResult<()>,
    {
        self.require_member_finished()?;
        validate_path(source_member_path, self.limits.max_key_bytes)?;
        let deadline = self.deadline;
        let cancelled = Arc::clone(&self.cancelled);
        let limits = self.limits;
        self.run(false, work, |db, work| {
            let mut statement = db
                .prepare(
                    "SELECT id,physical_line,source_sha256,
                            CASE WHEN length(canonical_payload)<=?3
                                 THEN canonical_payload ELSE NULL END,
                            length(canonical_payload)
                     FROM evidence
                     WHERE source_ref=?1 AND kind=?2 ORDER BY id COLLATE BINARY",
                )
                .map_err(sqlite_error)?;
            let mut rows = statement
                .query(params![
                    source_member_path,
                    kind.as_str(),
                    limits.max_value_bytes as i64
                ])
                .map_err(sqlite_error)?;
            let mut count = 0u64;
            while let Some(row) = rows.next().map_err(sqlite_error)? {
                count = count
                    .checked_add(1)
                    .ok_or(DurableError::Refused("source evidence count overflow"))?;
                add_counter(&mut work.sqlite_rows_returned, 1)?;
                charge_returned_row_bytes(row, work)?;
                check_active(deadline, &cancelled)?;
                if count > limits.max_rows {
                    return Err(DurableError::Refused("source evidence row budget exceeded"));
                }
                check_logical_limit(work, limits)?;
                check_row_limit(work, limits)?;
                let id: String = row.get(0).map_err(sqlite_error)?;
                let line: i64 = row.get(1).map_err(sqlite_error)?;
                let sha: Vec<u8> = row.get(2).map_err(sqlite_error)?;
                let payload: Option<Vec<u8>> = row.get(3).map_err(sqlite_error)?;
                let payload_len: i64 = row.get(4).map_err(sqlite_error)?;
                let payload_len = usize::try_from(payload_len)
                    .map_err(|_| DurableError::Corrupt("stored evidence length malformed"))?;
                if payload_len > limits.max_value_bytes {
                    return Err(DurableError::Corrupt(
                        "stored source evidence exceeds assessment bound",
                    ));
                }
                let payload = payload.ok_or(DurableError::Corrupt(
                    "bounded source evidence payload absent",
                ))?;
                if payload.len() > limits.max_value_bytes || sha.len() != 32 {
                    return Err(DurableError::Corrupt(
                        "stored source evidence row malformed",
                    ));
                }
                let mut digest = [0; 32];
                digest.copy_from_slice(&sha);
                if tos_foundation::Digest256::of_bytes(&payload).as_bytes() != &digest {
                    return Err(DurableError::Corrupt(
                        "stored source evidence digest differs",
                    ));
                }
                let physical_line = u64::try_from(line).ok().filter(|line| *line > 0).ok_or(
                    DurableError::Corrupt("stored source evidence line malformed"),
                )?;
                visit(
                    SourceAssessmentEvidenceOwned {
                        id,
                        physical_line,
                        source_sha256: digest,
                        canonical_payload: payload,
                    },
                    work,
                )?;
            }
            Ok(count)
        })
    }

    /// Match one actual PostgreSQL current-member carrier to the complete
    /// source assessment and mark the expected path seen. The caller counts
    /// the PostgreSQL row before invoking this method.
    pub(crate) fn observe_member(
        &mut self,
        actual: SourceAssessmentMember<'_>,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        validate_path(actual.path, self.limits.max_key_bytes)?;
        validate_dependencies(actual.dependencies, self.limits.max_key_bytes)?;
        let Some(expected) = self.member(actual.path, work)? else {
            return Err(DurableError::Corrupt(
                "current PostgreSQL member is absent from source assessment",
            ));
        };
        if expected.sha256 != actual.sha256
            || expected.size_bytes != actual.size_bytes
            || expected.mode != actual.mode
            || expected.dependencies.as_deref() != actual.dependencies
        {
            return Err(DurableError::Corrupt(
                "current PostgreSQL member differs from source assessment",
            ));
        }
        self.run(true, work, |db, work| {
            let changed = db
                .execute(
                    "UPDATE members SET seen=1 WHERE path=?1 AND seen=0",
                    params![actual.path],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt(
                    "duplicate current PostgreSQL source member",
                ));
            }
            add_counter(&mut work.sqlite_rows_updated, 1)?;
            Ok(())
        })
    }

    pub(crate) fn finish_member_comparison(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        self.run(false, work, |db, work| {
            if let Some(path) = query_optional_text(
                db,
                "SELECT path FROM members WHERE seen=0 ORDER BY path COLLATE BINARY LIMIT 1",
                [],
                work,
            )? {
                return Err(DurableError::Corrupt(
                    "current PostgreSQL source member coverage is incomplete",
                ));
            }
            Ok(())
        })
    }

    pub(crate) fn observe_index_row(
        &mut self,
        row: &AssessmentIndexRow,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        validate_index_row(row, self.limits)?;
        self.run(true, work, |db, work| {
            let expected = query_optional_decoded(
                db,
                "SELECT path,definition_digest,seen FROM expected_index
                     WHERE kind=?1 AND token=?2",
                params![row.kind, row.token],
                work,
                |r| {
                    let path: String = r.get(0).map_err(sqlite_error)?;
                    let digest: String = r.get(1).map_err(sqlite_error)?;
                    let seen: i64 = r.get(2).map_err(sqlite_error)?;
                    let bytes = path
                        .len()
                        .checked_add(digest.len() + 1)
                        .ok_or(DurableError::Refused("source index read size overflow"))?;
                    Ok(((path, digest, seen), bytes))
                },
            )?;
            let Some((path, digest, seen)) = expected else {
                return Err(DurableError::Corrupt(
                    "database index contains an unexpected source row",
                ));
            };
            if seen != 0 || path != row.path || digest != row.definition_digest {
                return Err(DurableError::Corrupt(
                    "database source index differs or repeats a key",
                ));
            }
            let changed = db
                .execute(
                    "UPDATE expected_index SET seen=1 WHERE kind=?1 AND token=?2 AND seen=0",
                    params![row.kind, row.token],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt("source index seen marker differs"));
            }
            add_counter(&mut work.sqlite_rows_updated, 1)?;
            Ok(())
        })
    }

    pub(crate) fn finish_index_comparison(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        self.run(false, work, |db, work| {
            if query_optional_i64(
                db,
                "SELECT 1 FROM expected_index WHERE seen=0 LIMIT 1",
                [],
                work,
            )?
            .is_some()
            {
                return Err(DurableError::Corrupt(
                    "database source index omitted an expected row",
                ));
            }
            Ok(())
        })
    }

    /// Return the next source-owned index row by its exact BINARY primary key.
    /// Text lengths are checked in SQL before the values are materialized; the
    /// bounded one-row result is charged before decoding or semantic checks.
    pub(crate) fn expected_index_after(
        &mut self,
        after: Option<(&str, &str)>,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Option<AssessmentIndexRow>> {
        self.require_source_finished()?;
        if let Some((kind, token)) = after {
            validate_key_text(
                kind,
                self.limits.max_key_bytes,
                "source index cursor kind malformed",
            )?;
            validate_key_text(
                token,
                self.limits.max_key_bytes,
                "source index cursor token malformed",
            )?;
        }
        let limits = self.limits;
        let key_bytes = i64::try_from(limits.max_key_bytes)
            .map_err(|_| DurableError::Refused("source index key bound exceeds SQLite range"))?;
        self.run(false, work, |db, work| {
            let decode = |row: &Row<'_>| {
                let kind = read_bounded_text(row, 0, 1, limits.max_key_bytes)?;
                let token = read_bounded_text(row, 2, 3, limits.max_key_bytes)?;
                let path = read_bounded_text(row, 4, 5, limits.max_key_bytes)?;
                let definition_digest = read_bounded_text(row, 6, 7, 64)?;
                Ok((
                    AssessmentIndexRow {
                        kind,
                        token,
                        path,
                        definition_digest,
                    },
                    0,
                ))
            };
            let result = if let Some((kind, token)) = after {
                query_optional_decoded(
                    db,
                    "SELECT length(CAST(kind AS BLOB)),
                            CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=?3 THEN kind END,
                            length(CAST(token AS BLOB)),
                            CASE WHEN typeof(token)='text' AND length(CAST(token AS BLOB))<=?3 THEN token END,
                            length(CAST(path AS BLOB)),
                            CASE WHEN typeof(path)='text' AND length(CAST(path AS BLOB))<=?3 THEN path END,
                            length(CAST(definition_digest AS BLOB)),
                            CASE WHEN typeof(definition_digest)='text' AND length(CAST(definition_digest AS BLOB))=64 THEN definition_digest END
                     FROM expected_index
                     WHERE (kind COLLATE BINARY,token COLLATE BINARY)>
                           (?1 COLLATE BINARY,?2 COLLATE BINARY)
                     ORDER BY kind COLLATE BINARY,token COLLATE BINARY LIMIT 1",
                    params![kind, token, key_bytes],
                    work,
                    decode,
                )?
            } else {
                query_optional_decoded(
                    db,
                    "SELECT length(CAST(kind AS BLOB)),
                            CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=?1 THEN kind END,
                            length(CAST(token AS BLOB)),
                            CASE WHEN typeof(token)='text' AND length(CAST(token AS BLOB))<=?1 THEN token END,
                            length(CAST(path AS BLOB)),
                            CASE WHEN typeof(path)='text' AND length(CAST(path AS BLOB))<=?1 THEN path END,
                            length(CAST(definition_digest AS BLOB)),
                            CASE WHEN typeof(definition_digest)='text' AND length(CAST(definition_digest AS BLOB))=64 THEN definition_digest END
                     FROM expected_index
                     ORDER BY kind COLLATE BINARY,token COLLATE BINARY LIMIT 1",
                    params![key_bytes],
                    work,
                    decode,
                )?
            };
            if let Some(row) = result.as_ref() {
                validate_index_row(row, limits)?;
                if after.is_some_and(|(kind, token)| {
                    (row.kind.as_bytes(), row.token.as_bytes())
                        <= (kind.as_bytes(), token.as_bytes())
                }) {
                    return Err(DurableError::Corrupt(
                        "source index keyset cursor did not advance",
                    ));
                }
            }
            Ok(result)
        })
    }

    /// Observe a predicate row after the caller has validated its owner-local
    /// schema/kind/version and complete flag. Extra retained absence keys are
    /// allowed; every expected source predicate must still be observed.
    pub(crate) fn observe_predicate_row(
        &mut self,
        row: &AssessmentPredicateRow,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        validate_predicate_row(row, self.limits)?;
        self.run(true, work, |db, work| {
            let changed = db
                .execute(
                    "INSERT INTO observed_predicate(kind,owner,scope,token,definition_version)
                     VALUES(?1,?2,?3,?4,?5)",
                    params![
                        row.kind,
                        row.owner,
                        row.scope,
                        row.token,
                        row.definition_version
                    ],
                )
                .map_err(|_| DurableError::Corrupt("duplicate observed source predicate"))?;
            if changed != 1 {
                return Err(DurableError::Corrupt(
                    "observed predicate insert count differs",
                ));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            let expected = query_optional_decoded(
                db,
                "SELECT definition_version FROM expected_predicate
                     WHERE kind=?1 AND owner=?2 AND scope=?3 AND token=?4",
                params![row.kind, row.owner, row.scope, row.token],
                work,
                |r| {
                    let version: String = r.get(0).map_err(sqlite_error)?;
                    let bytes = version.len();
                    Ok((version, bytes))
                },
            )?;
            if let Some(version) = expected {
                if version != row.definition_version {
                    return Err(DurableError::Corrupt(
                        "database source predicate definition differs",
                    ));
                }
                let changed = db
                    .execute(
                        "UPDATE expected_predicate SET seen=1
                         WHERE kind=?1 AND owner=?2 AND scope=?3 AND token=?4 AND seen=0",
                        params![row.kind, row.owner, row.scope, row.token],
                    )
                    .map_err(sqlite_error)?;
                if changed != 1 {
                    return Err(DurableError::Corrupt(
                        "source predicate seen marker differs",
                    ));
                }
                add_counter(&mut work.sqlite_rows_updated, 1)?;
            }
            Ok(())
        })
    }

    pub(crate) fn finish_predicate_comparison(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        self.run(false, work, |db, work| {
            if query_optional_i64(
                db,
                "SELECT 1 FROM expected_predicate WHERE seen=0 LIMIT 1",
                [],
                work,
            )?
            .is_some()
            {
                return Err(DurableError::Corrupt(
                    "database omitted an expected source predicate",
                ));
            }
            Ok(())
        })
    }

    /// Return the next source-owned predicate by its exact BINARY primary key.
    /// The cursor is strict (`>`); extra retained absence rows stay available
    /// for comparison without allocating a full expected-row vector.
    pub(crate) fn expected_predicate_after(
        &mut self,
        after: Option<(&str, &str, &str, &str)>,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Option<AssessmentPredicateRow>> {
        self.require_source_finished()?;
        if let Some((kind, owner, scope, token)) = after {
            for value in [kind, owner, scope, token] {
                validate_key_text(
                    value,
                    self.limits.max_key_bytes,
                    "source predicate cursor key malformed",
                )?;
            }
        }
        let limits = self.limits;
        let key_bytes = i64::try_from(limits.max_key_bytes).map_err(|_| {
            DurableError::Refused("source predicate key bound exceeds SQLite range")
        })?;
        self.run(false, work, |db, work| {
            let decode = |row: &Row<'_>| {
                let kind = read_bounded_text(row, 0, 1, limits.max_key_bytes)?;
                let owner = read_bounded_text(row, 2, 3, limits.max_key_bytes)?;
                let scope = read_bounded_text(row, 4, 5, limits.max_key_bytes)?;
                let token = read_bounded_text(row, 6, 7, limits.max_key_bytes)?;
                let definition_version = read_bounded_text(row, 8, 9, 64)?;
                Ok((
                    AssessmentPredicateRow {
                        kind,
                        owner,
                        scope,
                        token,
                        definition_version,
                    },
                    0,
                ))
            };
            let result = if let Some((kind, owner, scope, token)) = after {
                query_optional_decoded(
                    db,
                    "SELECT length(CAST(kind AS BLOB)),
                            CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=?5 THEN kind END,
                            length(CAST(owner AS BLOB)),
                            CASE WHEN typeof(owner)='text' AND length(CAST(owner AS BLOB))<=?5 THEN owner END,
                            length(CAST(scope AS BLOB)),
                            CASE WHEN typeof(scope)='text' AND length(CAST(scope AS BLOB))<=?5 THEN scope END,
                            length(CAST(token AS BLOB)),
                            CASE WHEN typeof(token)='text' AND length(CAST(token AS BLOB))<=?5 THEN token END,
                            length(CAST(definition_version AS BLOB)),
                            CASE WHEN typeof(definition_version)='text' AND length(CAST(definition_version AS BLOB))=64 THEN definition_version END
                     FROM expected_predicate
                     WHERE (kind COLLATE BINARY,owner COLLATE BINARY,scope COLLATE BINARY,token COLLATE BINARY)>
                           (?1 COLLATE BINARY,?2 COLLATE BINARY,?3 COLLATE BINARY,?4 COLLATE BINARY)
                     ORDER BY kind COLLATE BINARY,owner COLLATE BINARY,scope COLLATE BINARY,token COLLATE BINARY LIMIT 1",
                    params![kind, owner, scope, token, key_bytes],
                    work,
                    decode,
                )?
            } else {
                query_optional_decoded(
                    db,
                    "SELECT length(CAST(kind AS BLOB)),
                            CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=?1 THEN kind END,
                            length(CAST(owner AS BLOB)),
                            CASE WHEN typeof(owner)='text' AND length(CAST(owner AS BLOB))<=?1 THEN owner END,
                            length(CAST(scope AS BLOB)),
                            CASE WHEN typeof(scope)='text' AND length(CAST(scope AS BLOB))<=?1 THEN scope END,
                            length(CAST(token AS BLOB)),
                            CASE WHEN typeof(token)='text' AND length(CAST(token AS BLOB))<=?1 THEN token END,
                            length(CAST(definition_version AS BLOB)),
                            CASE WHEN typeof(definition_version)='text' AND length(CAST(definition_version AS BLOB))=64 THEN definition_version END
                     FROM expected_predicate
                     ORDER BY kind COLLATE BINARY,owner COLLATE BINARY,scope COLLATE BINARY,token COLLATE BINARY LIMIT 1",
                    params![key_bytes],
                    work,
                    decode,
                )?
            };
            if let Some(row) = result.as_ref() {
                validate_predicate_row(row, limits)?;
                if after.is_some_and(|(kind, owner, scope, token)| {
                    (
                        row.kind.as_bytes(),
                        row.owner.as_bytes(),
                        row.scope.as_bytes(),
                        row.token.as_bytes(),
                    ) <= (
                        kind.as_bytes(),
                        owner.as_bytes(),
                        scope.as_bytes(),
                        token.as_bytes(),
                    )
                }) {
                    return Err(DurableError::Corrupt(
                        "source predicate keyset cursor did not advance",
                    ));
                }
            }
            Ok(result)
        })
    }

    pub(crate) fn observe_projection(
        &mut self,
        row: &AssessmentProjectionRow,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        validate_path(&row.path, self.limits.max_key_bytes)?;
        if row
            .value
            .as_ref()
            .is_some_and(|value| value.len() > self.limits.max_value_bytes)
        {
            return Err(DurableError::Refused("database projection exceeds bound"));
        }
        let max_value_bytes = self.limits.max_value_bytes;
        self.run(true, work, |db, work| {
            let expected = query_optional_decoded(
                db,
                "SELECT is_present,
                        CASE WHEN is_present=1 AND length(value)<=?2 THEN value ELSE NULL END,
                        seen,length(value)
                 FROM expected_projection WHERE path=?1",
                params![row.path, max_value_bytes as i64],
                work,
                |r| {
                    let present: i64 = r.get(0).map_err(sqlite_error)?;
                    let length: Option<i64> = r.get(3).map_err(sqlite_error)?;
                    if present == 1
                        && length
                            .is_none_or(|value| value < 0 || value as u64 > max_value_bytes as u64)
                    {
                        return Err(DurableError::Corrupt(
                            "stored source projection exceeds assessment bound",
                        ));
                    }
                    let value: Option<Vec<u8>> = r.get(1).map_err(sqlite_error)?;
                    let seen: i64 = r.get(2).map_err(sqlite_error)?;
                    if present == 1 && value.is_none() {
                        return Err(DurableError::Corrupt(
                            "bounded source projection value absent",
                        ));
                    }
                    let bytes = value.as_ref().map_or(0, Vec::len).checked_add(24).ok_or(
                        DurableError::Refused("source projection read size overflow"),
                    )?;
                    Ok(((present, value, seen), bytes))
                },
            )?;
            let Some((is_present, value, seen)) = expected else {
                return Err(DurableError::Corrupt(
                    "database has an unexpected source projection",
                ));
            };
            if seen != 0 || (is_present != 0) != row.value.is_some() || value != row.value {
                return Err(DurableError::Corrupt(
                    "database source projection differs or repeats a path",
                ));
            }
            let changed = db
                .execute(
                    "UPDATE expected_projection SET seen=1 WHERE path=?1 AND seen=0",
                    params![row.path],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt(
                    "source projection seen marker differs",
                ));
            }
            add_counter(&mut work.sqlite_rows_updated, 1)?;
            Ok(())
        })
    }

    pub(crate) fn finish_projection_comparison(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_source_finished()?;
        self.run(false, work, |db, work| {
            if query_optional_i64(
                db,
                "SELECT 1 FROM expected_projection WHERE seen=0 LIMIT 1",
                [],
                work,
            )?
            .is_some()
            {
                return Err(DurableError::Corrupt(
                    "database omitted an expected source projection",
                ));
            }
            Ok(())
        })
    }

    /// Return the next source projection by its BINARY path key. Oversized
    /// payloads are excluded by SQL before rusqlite materializes the value;
    /// `None` remains distinct from a present empty value.
    pub(crate) fn projection_after(
        &mut self,
        after_path: Option<&str>,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Option<AssessmentProjectionRow>> {
        self.require_source_finished()?;
        if let Some(path) = after_path {
            validate_path(path, self.limits.max_key_bytes)?;
        }
        let max_value_bytes = self.limits.max_value_bytes;
        let limits = self.limits;
        self.run(false, work, |db, work| {
            let sql = if after_path.is_some() {
                "SELECT path,is_present,
                        CASE WHEN is_present=1 AND length(value)<=?2 THEN value ELSE NULL END,
                        length(value)
                 FROM expected_projection
                 WHERE path>?1 COLLATE BINARY
                 ORDER BY path COLLATE BINARY LIMIT 1"
            } else {
                "SELECT path,is_present,
                        CASE WHEN is_present=1 AND length(value)<=?1 THEN value ELSE NULL END,
                        length(value)
                 FROM expected_projection
                 ORDER BY path COLLATE BINARY LIMIT 1"
            };
            let mut statement = db.prepare(sql).map_err(sqlite_error)?;
            let mut rows = if let Some(after_path) = after_path {
                statement
                    .query(params![after_path, max_value_bytes as i64])
                    .map_err(sqlite_error)?
            } else {
                statement
                    .query(params![max_value_bytes as i64])
                    .map_err(sqlite_error)?
            };
            let Some(row) = rows.next().map_err(sqlite_error)? else {
                return Ok(None);
            };

            // Record the observed row and actual returned field sizes before
            // interpreting presence, key validity, or payload consistency.
            add_counter(&mut work.sqlite_rows_returned, 1)?;
            let path_value = row.get_ref(0).map_err(sqlite_error)?;
            let present_value = row.get_ref(1).map_err(sqlite_error)?;
            let payload_value = row.get_ref(2).map_err(sqlite_error)?;
            let length_value = row.get_ref(3).map_err(sqlite_error)?;
            let logical_bytes = sqlite_value_logical_bytes(&path_value)
                .checked_add(sqlite_value_logical_bytes(&present_value))
                .and_then(|bytes| bytes.checked_add(sqlite_value_logical_bytes(&payload_value)))
                .and_then(|bytes| bytes.checked_add(sqlite_value_logical_bytes(&length_value)))
                .ok_or(DurableError::Refused(
                    "source projection result size overflow",
                ))?;
            add_read_bytes(work, logical_bytes)?;
            check_row_limit(work, limits)?;
            check_logical_limit(work, limits)?;

            let path = match path_value {
                ValueRef::Text(bytes) => std::str::from_utf8(bytes)
                    .map_err(|_| DurableError::Corrupt("stored source projection path UTF-8"))?
                    .to_owned(),
                _ => return Err(DurableError::Corrupt("stored source projection path type")),
            };
            validate_path(&path, limits.max_key_bytes)?;
            let is_present = match present_value {
                ValueRef::Integer(0) => false,
                ValueRef::Integer(1) => true,
                _ => return Err(DurableError::Corrupt("stored source projection presence")),
            };
            let expected_length = match length_value {
                ValueRef::Null => None,
                ValueRef::Integer(value) if value >= 0 => Some(value as u64),
                _ => return Err(DurableError::Corrupt("stored source projection length")),
            };
            if is_present
                && expected_length.is_some_and(|length| length > limits.max_value_bytes as u64)
            {
                return Err(DurableError::Refused(
                    "source projection payload exceeds assessment bound",
                ));
            }
            let value = match (is_present, payload_value, expected_length) {
                (false, ValueRef::Null, None) => None,
                (true, ValueRef::Blob(bytes), Some(length)) => {
                    if length != bytes.len() as u64 {
                        return Err(DurableError::Corrupt(
                            "source projection payload length differs",
                        ));
                    }
                    Some(bytes.to_vec())
                }
                (false, _, _) => {
                    return Err(DurableError::Corrupt(
                        "absent source projection has a payload",
                    ));
                }
                (true, _, _) => {
                    return Err(DurableError::Corrupt(
                        "present source projection payload is absent",
                    ));
                }
            };
            Ok(Some(AssessmentProjectionRow { path, value }))
        })
    }

    pub(crate) fn identity_path(
        &mut self,
        id: &str,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Option<String>> {
        self.require_source_finished()?;
        validate_key_text(id, self.limits.max_key_bytes, "source identity malformed")?;
        self.run(false, work, |db, work| {
            query_optional_text(
                db,
                "SELECT path FROM identity_path WHERE id=?1",
                params![id],
                work,
            )
        })
    }

    pub(crate) fn contains_member(
        &mut self,
        path: &str,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<bool> {
        self.require_member_finished()?;
        validate_path(path, self.limits.max_key_bytes)?;
        self.run(false, work, |db, work| {
            let value = query_optional_i64(
                db,
                "SELECT 1 FROM members WHERE path=?1",
                params![path],
                work,
            )?;
            Ok(value.is_some())
        })
    }

    pub(crate) fn member(
        &mut self,
        path: &str,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Option<SourceAssessmentMemberOwned>> {
        self.require_member_finished()?;
        validate_path(path, self.limits.max_key_bytes)?;
        let limits = self.limits;
        self.run(false, work, |db, work| read_member(db, path, limits, work))
    }

    /// Return the first source member whose exact UTF-8/BINARY path is after
    /// `after_path`, or the first member when it is `None`.
    pub(crate) fn member_after(
        &mut self,
        after_path: Option<&str>,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Option<SourceAssessmentMemberOwned>> {
        self.require_member_finished()?;
        if let Some(path) = after_path {
            validate_path(path, self.limits.max_key_bytes)?;
        }
        let limits = self.limits;
        self.run(false, work, |db, work| {
            let path = if let Some(after_path) = after_path {
                query_optional_text(
                    db,
                    "SELECT path FROM members WHERE path>?1 COLLATE BINARY
                     ORDER BY path COLLATE BINARY LIMIT 1",
                    params![after_path],
                    work,
                )?
            } else {
                query_optional_text(
                    db,
                    "SELECT path FROM members ORDER BY path COLLATE BINARY LIMIT 1",
                    [],
                    work,
                )?
            };
            let Some(path) = path else {
                return Ok(None);
            };
            let member = read_member(db, &path, limits, work)?.ok_or(DurableError::Corrupt(
                "source assessment cursor member absent",
            ))?;
            Ok(Some(member))
        })
    }

    pub(crate) fn add_current_placement(
        &mut self,
        path: &str,
        encoded_row: &[u8],
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_member_open()?;
        self.account_carrier_input(
            work,
            string_row_bytes(&[path])?
                .checked_add(encoded_row.len())
                .ok_or(DurableError::Refused(
                    "source assessment input size overflow",
                ))?,
        )?;
        validate_path(path, self.limits.max_key_bytes)?;
        if encoded_row.len() > self.limits.max_placement_bytes {
            return Err(DurableError::Refused(
                "current placement exceeds source assessment bound",
            ));
        }
        let max_placement_bytes = self.limits.max_placement_bytes;
        self.run(true, work, |db, work| {
            if let Some(existing) = query_optional_decoded(
                db,
                "SELECT length(encoded_row),
                        CASE WHEN length(encoded_row)<=?2 THEN encoded_row ELSE NULL END
                 FROM current_placement WHERE path=?1",
                params![path, max_placement_bytes as i64],
                work,
                |row| {
                    let length: i64 = row.get(0).map_err(sqlite_error)?;
                    if length < 0 || length as u64 > max_placement_bytes as u64 {
                        return Err(DurableError::Corrupt(
                            "stored current placement exceeds source assessment bound",
                        ));
                    }
                    let value: Option<Vec<u8>> = row.get(1).map_err(sqlite_error)?;
                    let value = value.ok_or(DurableError::Corrupt(
                        "bounded current placement value absent",
                    ))?;
                    let bytes = value.len().checked_add(8).ok_or(DurableError::Refused(
                        "current placement byte count overflow",
                    ))?;
                    Ok((value, bytes))
                },
            )? {
                if existing == encoded_row {
                    return Ok(());
                }
                return Err(DurableError::Conflict(
                    "current placement changed within source assessment",
                ));
            }
            let changed = db
                .execute(
                    "INSERT INTO current_placement(path,encoded_row) VALUES(?1,?2)",
                    params![path, encoded_row],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(DurableError::Corrupt(
                    "current placement insert count differs",
                ));
            }
            add_counter(&mut work.sqlite_rows_inserted, 1)?;
            Ok(())
        })
    }

    pub(crate) fn lookup_current_placement(
        &mut self,
        path: &str,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Option<Vec<u8>>> {
        self.require_member_finished()?;
        validate_path(path, self.limits.max_key_bytes)?;
        let max_placement_bytes = self.limits.max_placement_bytes;
        self.run(false, work, |db, work| {
            query_optional_decoded(
                db,
                "SELECT length(encoded_row),
                        CASE WHEN length(encoded_row)<=?2 THEN encoded_row ELSE NULL END
                 FROM current_placement WHERE path=?1",
                params![path, max_placement_bytes as i64],
                work,
                |row| {
                    let length: i64 = row.get(0).map_err(sqlite_error)?;
                    if length < 0 || length as u64 > max_placement_bytes as u64 {
                        return Err(DurableError::Corrupt(
                            "stored current placement exceeds source assessment bound",
                        ));
                    }
                    let value: Option<Vec<u8>> = row.get(1).map_err(sqlite_error)?;
                    let value = value.ok_or(DurableError::Corrupt(
                        "bounded current placement value absent",
                    ))?;
                    let bytes = value.len().checked_add(8).ok_or(DurableError::Refused(
                        "current placement byte count overflow",
                    ))?;
                    Ok((value, bytes))
                },
            )
        })
    }

    /// Require the selected cold-cut placements to cover exactly the complete
    /// source member set. Call after all placement rows have been added.
    pub(crate) fn finish_placement_coverage(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.require_member_finished()?;
        let limits = self.limits;
        self.run(false, work, |db, work| {
            check_placement_coverage(db, work, limits)
        })
    }

    /// Return the bounded immediate children of a directory. The query is a
    /// BINARY primary-key range scan, grouped with O(1) state. It refuses a
    /// source tree where one immediate child is both a file and directory.
    pub(crate) fn list_immediate_children(
        &mut self,
        path: &str,
        max_children: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<Vec<SourceDirectoryChild>> {
        self.require_member_finished()?;
        validate_path(path, self.limits.max_key_bytes)?;
        if max_children == 0 || max_children as u64 > self.limits.max_rows {
            return Err(DurableError::Refused(
                "invalid immediate source child limit",
            ));
        }
        let requested_deadline = deadline.min(self.deadline);
        if cancelled.load(Ordering::Relaxed) {
            return Err(DurableError::Refused("source assessment cancelled"));
        }
        let limits = self.limits;
        let internal_cancelled = Arc::clone(&self.cancelled);
        let mut lower = String::with_capacity(path.len() + 1);
        lower.push_str(path);
        lower.push('/');
        let mut upper = String::with_capacity(path.len() + 1);
        upper.push_str(path);
        upper.push('0');
        self.run(false, work, |db, work| {
            if query_optional_i64(
                db,
                "SELECT 1 FROM members WHERE path=?1",
                params![path],
                work,
            )?
            .is_some()
            {
                return Err(DurableError::Conflict(
                    "source path is both a file and a directory",
                ));
            }
            let mut statement = db
                .prepare(
                    "SELECT path FROM members
                     WHERE path>=?1 COLLATE BINARY AND path<?2 COLLATE BINARY
                     ORDER BY path COLLATE BINARY",
                )
                .map_err(sqlite_error)?;
            let mut rows = statement
                .query(params![lower, upper])
                .map_err(sqlite_error)?;
            let mut children = Vec::new();
            let mut current: Option<SourceDirectoryChild> = None;
            let mut scanned = 0u64;
            while let Some(row) = rows.next().map_err(sqlite_error)? {
                check_active(requested_deadline, &internal_cancelled)?;
                if cancelled.load(Ordering::Relaxed) {
                    return Err(DurableError::Refused("source assessment cancelled"));
                }
                scanned = scanned
                    .checked_add(1)
                    .ok_or(DurableError::Refused("source child row count overflow"))?;
                if scanned > limits.max_rows {
                    return Err(DurableError::Refused("source child row budget exceeded"));
                }
                add_counter(&mut work.sqlite_rows_returned, 1)?;
                check_row_limit(work, limits)?;
                let full_path: String = row.get(0).map_err(sqlite_error)?;
                add_read_bytes(work, full_path.len())?;
                check_logical_limit(work, limits)?;
                let suffix = full_path
                    .strip_prefix(&lower)
                    .ok_or(DurableError::Corrupt("source child prefix differs"))?;
                let (name, is_directory) = match suffix.split_once('/') {
                    Some((name, _)) => (name, true),
                    None => (suffix, false),
                };
                if name.is_empty() {
                    return Err(DurableError::Corrupt("source child name is empty"));
                }
                match current.as_mut() {
                    Some(child) if child.name == name => {
                        if child.is_directory != is_directory {
                            return Err(DurableError::Conflict(
                                "source child is both a file and directory",
                            ));
                        }
                    }
                    Some(_) => {
                        children
                            .try_reserve(1)
                            .map_err(|_| DurableError::Refused("source child allocation failed"))?;
                        children.push(current.take().expect("present"));
                        if children.len() >= max_children {
                            return Err(DurableError::Refused(
                                "immediate source child count exceeds limit",
                            ));
                        }
                        current = Some(SourceDirectoryChild {
                            name: name.to_owned(),
                            is_directory,
                        });
                    }
                    None => {
                        current = Some(SourceDirectoryChild {
                            name: name.to_owned(),
                            is_directory,
                        });
                    }
                }
            }
            if let Some(child) = current {
                children
                    .try_reserve(1)
                    .map_err(|_| DurableError::Refused("source child allocation failed"))?;
                children.push(child);
            }
            if children.len() > max_children {
                return Err(DurableError::Refused(
                    "immediate source child count exceeds limit",
                ));
            }
            Ok(children)
        })
    }

    /// Close membership and placement inputs after exact current membership
    /// has been streamed. Until this check succeeds no membership or
    /// placement read APIs are available, and after it succeeds neither input
    /// can be extended.
    pub(crate) fn finish_member_inputs(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        if self.member_inputs_finished || self.source_inputs_finished {
            return Err(DurableError::Conflict(
                "source assessment member inputs already finished",
            ));
        }
        let limits = self.limits;
        self.run(false, work, |db, work| {
            reject_text_witness(
                db,
                "SELECT d.target_path FROM member_dependencies d
                 LEFT JOIN members m ON m.path=d.target_path COLLATE BINARY
                 WHERE m.path IS NULL LIMIT 1",
                work,
                limits,
                "source assessment dependency target is absent",
            )?;
            check_placement_coverage(db, work, limits)
        })?;
        self.member_inputs_finished = true;
        Ok(())
    }

    /// Close semantic rows after source-owned parsing and projection have
    /// completed. Calling this directly retains the original convenience
    /// behavior by sealing member inputs first when necessary.
    pub(crate) fn finish_source_inputs(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        if self.source_inputs_finished {
            return Err(DurableError::Conflict(
                "source assessment inputs already finished",
            ));
        }
        if !self.member_inputs_finished {
            self.finish_member_inputs(work)?;
        }
        let limits = self.limits;
        self.run(false, work, |db, work| {
            // These checks are limited to the direct references represented by
            // this derived index. Semantic source-owner laws remain with the
            // parser and its exact source-byte validation.
            for (sql, reason) in [
                (
                    "SELECT p.path FROM identity_path p
                     LEFT JOIN members m ON m.path=p.path COLLATE BINARY
                     WHERE m.path IS NULL LIMIT 1",
                    "source identity path is absent from membership",
                ),
                (
                    "SELECT k.path FROM identity_kinds k
                     LEFT JOIN members m ON m.path=k.path COLLATE BINARY
                     WHERE m.path IS NULL LIMIT 1",
                    "source identity owner is absent from membership",
                ),
                (
                    "SELECT k.id FROM identity_kinds k
                     LEFT JOIN identity_path p ON p.id=k.id COLLATE BINARY
                       AND p.path=k.path COLLATE BINARY
                     WHERE p.id IS NULL LIMIT 1",
                    "source identity path index differs",
                ),
                (
                    "SELECT p.id FROM identity_path p
                     LEFT JOIN identity_kinds k ON k.id=p.id COLLATE BINARY
                     WHERE k.id IS NULL LIMIT 1",
                    "source identity path has no owner identity",
                ),
                (
                    "SELECT e.source_ref FROM evidence e
                     LEFT JOIN members m ON m.path=e.source_ref COLLATE BINARY
                     LEFT JOIN identity_kinds k ON k.kind=e.kind COLLATE BINARY
                       AND k.id=e.id COLLATE BINARY
                       AND k.path=e.source_ref COLLATE BINARY
                     WHERE m.path IS NULL OR k.id IS NULL LIMIT 1",
                    "source evidence owner or identity reference is absent",
                ),
                (
                    "SELECT i.path FROM expected_index i
                     LEFT JOIN members m ON m.path=i.path COLLATE BINARY
                     LEFT JOIN identity_kinds k ON k.kind=i.kind COLLATE BINARY
                       AND k.id=i.token COLLATE BINARY
                       AND k.path=i.path COLLATE BINARY
                     WHERE m.path IS NULL OR (i.kind!='path' AND k.id IS NULL)
                     LIMIT 1",
                    "source index path or owner identity is absent",
                ),
                (
                    "SELECT k.path FROM identity_kinds k
                     LEFT JOIN expected_index i ON i.kind=k.kind COLLATE BINARY
                       AND i.token=k.id COLLATE BINARY
                       AND i.path=k.path COLLATE BINARY
                     WHERE i.kind IS NULL LIMIT 1",
                    "source owner identity is absent from source index",
                ),
                (
                    "SELECT p.path FROM expected_projection p
                     LEFT JOIN members m ON m.path=p.path COLLATE BINARY
                     WHERE m.path IS NULL LIMIT 1",
                    "source projection path is absent from membership",
                ),
            ] {
                reject_text_witness(db, sql, work, limits, reason)?;
            }
            Ok(())
        })?;
        self.source_inputs_finished = true;
        Ok(())
    }

    pub(crate) fn snapshot_workspace_usage(
        &mut self,
        work: &mut SourceAssessmentWork,
    ) -> DurableResult<()> {
        self.snapshot_usage(work)
    }

    fn require_source_finished(&self) -> DurableResult<()> {
        if self.source_inputs_finished {
            Ok(())
        } else {
            Err(DurableError::Conflict(
                "source assessment inputs are not finished",
            ))
        }
    }

    fn require_source_open(&self) -> DurableResult<()> {
        if self.source_inputs_finished {
            Err(DurableError::Conflict(
                "source assessment inputs are already finished",
            ))
        } else {
            Ok(())
        }
    }

    fn require_member_finished(&self) -> DurableResult<()> {
        if self.member_inputs_finished {
            Ok(())
        } else {
            Err(DurableError::Conflict(
                "source assessment member inputs are not finished",
            ))
        }
    }

    fn require_member_open(&self) -> DurableResult<()> {
        if self.member_inputs_finished || self.source_inputs_finished {
            Err(DurableError::Conflict(
                "source assessment member inputs are already finished",
            ))
        } else {
            Ok(())
        }
    }

    fn account_source_input(
        &self,
        work: &mut SourceAssessmentWork,
        bytes: usize,
    ) -> DurableResult<()> {
        self.account_source_input_rows(work, bytes, 1)
    }

    fn account_source_input_rows(
        &self,
        work: &mut SourceAssessmentWork,
        bytes: usize,
        rows: usize,
    ) -> DurableResult<()> {
        add_counter(
            &mut work.source_input_rows_attempted,
            u64::try_from(rows)
                .map_err(|_| DurableError::Refused("source assessment row count overflow"))?,
        )?;
        let bytes = u64::try_from(bytes)
            .map_err(|_| DurableError::Refused("source assessment input size overflow"))?;
        add_counter(&mut work.source_input_logical_bytes, bytes)?;
        check_input_limits(work, self.limits)?;
        self.workspace.charge_private_bytes(bytes)?;
        Ok(())
    }

    fn account_carrier_input(
        &self,
        work: &mut SourceAssessmentWork,
        bytes: usize,
    ) -> DurableResult<()> {
        add_counter(&mut work.carrier_input_rows_attempted, 1)?;
        let bytes = u64::try_from(bytes)
            .map_err(|_| DurableError::Refused("source assessment carrier size overflow"))?;
        add_counter(&mut work.carrier_input_logical_bytes, bytes)?;
        check_input_limits(work, self.limits)?;
        self.workspace.charge_private_bytes(bytes)?;
        Ok(())
    }

    fn run<T>(
        &mut self,
        write: bool,
        work: &mut SourceAssessmentWork,
        operation: impl FnOnce(&Connection, &mut SourceAssessmentWork) -> DurableResult<T>,
    ) -> DurableResult<T> {
        check_active(self.deadline, &self.cancelled)?;
        let capacity = if write {
            self.apply_page_limit()
        } else {
            Ok(())
        };
        if let Err(error) = capacity {
            let _ = self.snapshot_usage(work);
            return Err(error);
        }
        let result = operation(&self.db, work);
        let measured = self.snapshot_usage(work);
        match result {
            Err(error) => Err(error),
            Ok(value) => {
                measured?;
                check_work_limits(work, self.limits)?;
                Ok(value)
            }
        }
    }

    fn apply_page_limit(&mut self) -> DurableResult<()> {
        let page_count: i64 = self
            .db
            .query_row("PRAGMA page_count", [], |row| row.get(0))
            .map_err(sqlite_error)?;
        let page_count = u64::try_from(page_count)
            .map_err(|_| DurableError::Corrupt("SQLite page count malformed"))?;
        let current_bytes = page_count
            .checked_mul(SQLITE_PAGE_BYTES)
            .ok_or(DurableError::Refused("SQLite size overflow"))?;
        if current_bytes > self.limits.max_sqlite_file_bytes {
            return Err(DurableError::Refused(
                "source assessment SQLite file limit exceeded",
            ));
        }
        let remaining = self.workspace.remaining_private_bytes()?;
        let additional_pages = remaining / SQLITE_PAGE_BYTES;
        let workspace_pages = page_count
            .checked_add(additional_pages)
            .ok_or(DurableError::Refused("SQLite page budget overflow"))?;
        let file_pages = self.limits.max_sqlite_file_bytes / SQLITE_PAGE_BYTES;
        let max_pages = workspace_pages.min(file_pages).max(page_count);
        let max_pages = i64::try_from(max_pages)
            .map_err(|_| DurableError::Refused("SQLite page limit overflow"))?;
        self.db
            .pragma_update(None, "max_page_count", max_pages)
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn snapshot_usage(&mut self, work: &mut SourceAssessmentWork) -> DurableResult<()> {
        let observed_callbacks = self.vm_callbacks.load(Ordering::Relaxed);
        let callbacks = observed_callbacks
            .checked_sub(self.last_reported_vm_callbacks)
            .ok_or(DurableError::Corrupt(
                "SQLite progress count moved backwards",
            ))?;
        self.last_reported_vm_callbacks = observed_callbacks;
        add_counter(&mut work.sqlite_vm_progress_callbacks, callbacks)?;
        if observed_callbacks > self.limits.max_vm_steps {
            return Err(DurableError::Refused(
                "source assessment SQLite step budget exceeded",
            ));
        }
        let metadata = self
            ._backing_file
            .metadata()
            .map_err(|_| DurableError::Refused("source assessment SQLite stat failed"))?;
        let file_len = metadata.len();
        let allocated = metadata
            .blocks()
            .checked_mul(512)
            .ok_or(DurableError::Refused("SQLite allocation count overflow"))?;
        work.sqlite_file_len_current = file_len;
        work.sqlite_file_len_high_water = work.sqlite_file_len_high_water.max(file_len);
        work.sqlite_allocated_bytes_current = allocated;
        work.sqlite_allocated_bytes_high_water =
            work.sqlite_allocated_bytes_high_water.max(allocated);
        let page_count: i64 = self
            .db
            .query_row("PRAGMA page_count", [], |row| row.get(0))
            .map_err(sqlite_error)?;
        let page_count = u64::try_from(page_count)
            .map_err(|_| DurableError::Corrupt("SQLite page count malformed"))?;
        if page_count < self.last_charged_pages {
            return Err(DurableError::Corrupt("SQLite page count moved backwards"));
        }
        let new_pages = page_count - self.last_charged_pages;
        if new_pages > 0 {
            let bytes = new_pages
                .checked_mul(SQLITE_PAGE_BYTES)
                .ok_or(DurableError::Refused("SQLite page allocation overflow"))?;
            self.workspace.charge_private_bytes(bytes)?;
            self.last_charged_pages = page_count;
        }
        work.sqlite_page_count_current = page_count;
        work.sqlite_page_count_high_water = work.sqlite_page_count_high_water.max(page_count);
        if file_len > self.limits.max_sqlite_file_bytes
            || allocated > self.limits.max_sqlite_file_bytes
        {
            return Err(DurableError::Refused(
                "source assessment SQLite file limit exceeded",
            ));
        }
        Ok(())
    }
}

fn validate_private_file(file: &File) -> DurableResult<()> {
    let metadata = file
        .metadata()
        .map_err(|_| DurableError::Refused("source assessment workspace stat failed"))?;
    if !metadata.is_file()
        || metadata.nlink() != 0
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.len() != 0
    {
        return Err(DurableError::Refused(
            "source assessment requires a fresh private unnamed file",
        ));
    }
    Ok(())
}

fn check_active(deadline: Instant, cancelled: &AtomicBool) -> DurableResult<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(DurableError::Refused("source assessment cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(DurableError::Refused("source assessment deadline exceeded"));
    }
    Ok(())
}

fn sqlite_error(_: rusqlite::Error) -> DurableError {
    DurableError::Refused("source assessment SQLite operation failed")
}

fn sqlite_value_logical_bytes(value: &ValueRef<'_>) -> usize {
    match value {
        ValueRef::Null => 0,
        ValueRef::Integer(_) | ValueRef::Real(_) => 8,
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => bytes.len(),
    }
}

fn reject_text_witness(
    db: &Connection,
    sql: &str,
    work: &mut SourceAssessmentWork,
    limits: SourceAssessmentLimits,
    reason: &'static str,
) -> DurableResult<()> {
    if query_optional_text(db, sql, [], work)?.is_some() {
        check_row_limit(work, limits)?;
        check_logical_limit(work, limits)?;
        return Err(DurableError::Corrupt(reason));
    }
    Ok(())
}

fn check_placement_coverage(
    db: &Connection,
    work: &mut SourceAssessmentWork,
    limits: SourceAssessmentLimits,
) -> DurableResult<()> {
    reject_text_witness(
        db,
        "SELECT m.path FROM members m
         LEFT JOIN current_placement p ON p.path=m.path COLLATE BINARY
         WHERE p.path IS NULL ORDER BY m.path COLLATE BINARY LIMIT 1",
        work,
        limits,
        "cold placement omits an assessed source member",
    )?;
    reject_text_witness(
        db,
        "SELECT p.path FROM current_placement p
         LEFT JOIN members m ON m.path=p.path COLLATE BINARY
         WHERE m.path IS NULL ORDER BY p.path COLLATE BINARY LIMIT 1",
        work,
        limits,
        "cold placement has an unassessed source member",
    )
}

fn add_counter(counter: &mut u64, amount: u64) -> DurableResult<()> {
    *counter = counter.checked_add(amount).ok_or(DurableError::Refused(
        "source assessment work counter overflow",
    ))?;
    Ok(())
}

fn check_i64(value: u64, reason: &'static str) -> DurableResult<()> {
    i64::try_from(value)
        .map(|_| ())
        .map_err(|_| DurableError::Refused(reason))
}

fn as_i64(value: u64) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Refused("source assessment integer overflow"))
}

fn string_row_bytes(values: &[&str]) -> DurableResult<usize> {
    values.iter().try_fold(0usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(DurableError::Refused("source assessment row size overflow"))
    })
}

fn member_logical_bytes(path: &str, dependencies: Option<&[String]>) -> DurableResult<usize> {
    let mut bytes = string_row_bytes(&[path])?
        .checked_add(32 + 8 + 4 + 1)
        .ok_or(DurableError::Refused(
            "source assessment member size overflow",
        ))?;
    if let Some(dependencies) = dependencies {
        for dependency in dependencies {
            bytes = bytes
                .checked_add(dependency.len())
                .ok_or(DurableError::Refused(
                    "source assessment member size overflow",
                ))?;
        }
    }
    Ok(bytes)
}

fn validate_key_text(value: &str, max: usize, reason: &'static str) -> DurableResult<()> {
    if value.is_empty() || value.len() > max || value.as_bytes().contains(&0) {
        return Err(DurableError::Corrupt(reason));
    }
    Ok(())
}

fn validate_path(path: &str, max_key_bytes: usize) -> DurableResult<()> {
    if path.len() > MAX_SOURCE_PATH_BYTES || path.len() > max_key_bytes {
        return Err(DurableError::Refused(
            "source path exceeds assessment bound",
        ));
    }
    RelativePath::parse(path).map_err(|_| DurableError::Corrupt("source path malformed"))?;
    Ok(())
}

fn validate_dependencies(
    dependencies: Option<&[String]>,
    max_key_bytes: usize,
) -> DurableResult<()> {
    let Some(dependencies) = dependencies else {
        return Ok(());
    };
    let mut bytes = 0usize;
    for (index, path) in dependencies.iter().enumerate() {
        validate_path(path, max_key_bytes)?;
        bytes = bytes
            .checked_add(path.len())
            .ok_or(DurableError::Refused("source dependency size overflow"))?;
        if index > 0 && dependencies[index - 1].as_bytes() >= path.as_bytes() {
            return Err(DurableError::Corrupt(
                "source dependencies are not sorted and unique",
            ));
        }
    }
    if dependencies.len() as u64 > u64::MAX / 2 || bytes > i32::MAX as usize {
        return Err(DurableError::Refused("source dependencies exceed bound"));
    }
    Ok(())
}

fn validate_index_row(
    row: &AssessmentIndexRow,
    limits: SourceAssessmentLimits,
) -> DurableResult<()> {
    if !matches!(
        row.kind.as_str(),
        "metadata" | "form" | "event" | "anchor" | "claim" | "path"
    ) {
        return Err(DurableError::Corrupt("source index kind is unknown"));
    }
    validate_key_text(
        &row.kind,
        limits.max_key_bytes,
        "source index kind malformed",
    )?;
    validate_key_text(
        &row.token,
        limits.max_key_bytes,
        "source index token malformed",
    )?;
    validate_path(&row.path, limits.max_key_bytes)?;
    if row.kind == "path" && row.token != row.path {
        return Err(DurableError::Corrupt("source path index key differs"));
    }
    if row.definition_digest.len() != 64
        || !row
            .definition_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DurableError::Corrupt("source index digest malformed"));
    }
    Ok(())
}

fn validate_predicate_row(
    row: &AssessmentPredicateRow,
    limits: SourceAssessmentLimits,
) -> DurableResult<()> {
    for value in [&row.kind, &row.owner, &row.scope, &row.token] {
        validate_key_text(
            value,
            limits.max_key_bytes,
            "source predicate key malformed",
        )?;
    }
    if row.definition_version.len() != 64
        || !row
            .definition_version
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DurableError::Corrupt(
            "source predicate definition version malformed",
        ));
    }
    Ok(())
}

fn add_read_bytes(work: &mut SourceAssessmentWork, bytes: usize) -> DurableResult<()> {
    let bytes = u64::try_from(bytes)
        .map_err(|_| DurableError::Refused("SQLite returned byte count overflow"))?;
    add_counter(&mut work.sqlite_logical_bytes_returned, bytes)
}

fn charge_returned_row_bytes(row: &Row<'_>, work: &mut SourceAssessmentWork) -> DurableResult<()> {
    for column in 0..row.as_ref().column_count() {
        let value = row.get_ref(column).map_err(sqlite_error)?;
        add_read_bytes(work, sqlite_value_logical_bytes(&value))?;
    }
    Ok(())
}

fn check_logical_limit(
    work: &SourceAssessmentWork,
    limits: SourceAssessmentLimits,
) -> DurableResult<()> {
    let total = work
        .source_input_logical_bytes
        .checked_add(work.carrier_input_logical_bytes)
        .ok_or(DurableError::Refused(
            "source assessment logical bytes overflow",
        ))?
        .checked_add(work.sqlite_logical_bytes_returned)
        .ok_or(DurableError::Refused(
            "source assessment logical bytes overflow",
        ))?;
    if total > limits.max_logical_bytes {
        return Err(DurableError::Refused(
            "source assessment logical byte budget exceeded",
        ));
    }
    Ok(())
}

fn check_input_limits(
    work: &SourceAssessmentWork,
    limits: SourceAssessmentLimits,
) -> DurableResult<()> {
    let rows = work
        .source_input_rows_attempted
        .checked_add(work.carrier_input_rows_attempted)
        .ok_or(DurableError::Refused(
            "source assessment input rows overflow",
        ))?;
    let bytes = work
        .source_input_logical_bytes
        .checked_add(work.carrier_input_logical_bytes)
        .ok_or(DurableError::Refused(
            "source assessment input bytes overflow",
        ))?;
    if rows > limits.max_rows || bytes > limits.max_logical_bytes {
        return Err(DurableError::Refused(
            "source assessment input budget exceeded",
        ));
    }
    Ok(())
}

fn check_row_limit(
    work: &SourceAssessmentWork,
    limits: SourceAssessmentLimits,
) -> DurableResult<()> {
    if work.sqlite_rows_returned > limits.max_rows {
        return Err(DurableError::Refused(
            "source assessment SQLite row budget exceeded",
        ));
    }
    Ok(())
}

fn check_work_limits(
    work: &SourceAssessmentWork,
    limits: SourceAssessmentLimits,
) -> DurableResult<()> {
    check_input_limits(work, limits)?;
    check_row_limit(work, limits)?;
    check_logical_limit(work, limits)
}

fn query_optional_text<P: rusqlite::Params>(
    db: &Connection,
    sql: &str,
    params: P,
    work: &mut SourceAssessmentWork,
) -> DurableResult<Option<String>> {
    let mut statement = db.prepare(sql).map_err(sqlite_error)?;
    let mut rows = statement.query(params).map_err(sqlite_error)?;
    let Some(row) = rows.next().map_err(sqlite_error)? else {
        return Ok(None);
    };
    add_counter(&mut work.sqlite_rows_returned, 1)?;
    let value: String = row.get(0).map_err(sqlite_error)?;
    add_read_bytes(work, value.len())?;
    Ok(Some(value))
}

fn query_optional_decoded<P, T, F>(
    db: &Connection,
    sql: &str,
    params: P,
    work: &mut SourceAssessmentWork,
    decode: F,
) -> DurableResult<Option<T>>
where
    P: Params,
    F: FnOnce(&Row<'_>) -> DurableResult<(T, usize)>,
{
    let mut statement = db.prepare(sql).map_err(sqlite_error)?;
    let mut rows = statement.query(params).map_err(sqlite_error)?;
    let Some(row) = rows.next().map_err(sqlite_error)? else {
        return Ok(None);
    };
    add_counter(&mut work.sqlite_rows_returned, 1)?;
    // Charge actual SQLite fields before type/semantic decoding. Failed row
    // validation must retain its returned work without charging estimates a
    // second time.
    charge_returned_row_bytes(row, work)?;
    let (value, _decoded_byte_estimate) = decode(row)?;
    Ok(Some(value))
}

fn read_bounded_text(
    row: &Row<'_>,
    length_column: usize,
    value_column: usize,
    max_bytes: usize,
) -> DurableResult<String> {
    let length = row
        .get::<_, Option<i64>>(length_column)
        .map_err(sqlite_error)?
        .filter(|length| *length >= 0)
        .ok_or(DurableError::Corrupt(
            "stored source assessment text length malformed",
        ))? as u64;
    if length > max_bytes as u64 {
        return Err(DurableError::Corrupt(
            "stored source assessment text exceeds key bound",
        ));
    }
    let value = row
        .get::<_, Option<String>>(value_column)
        .map_err(sqlite_error)?
        .ok_or(DurableError::Corrupt(
            "bounded source assessment text absent",
        ))?;
    if value.len() as u64 != length {
        return Err(DurableError::Corrupt(
            "stored source assessment text length differs",
        ));
    }
    Ok(value)
}

fn query_optional_i64<P: rusqlite::Params>(
    db: &Connection,
    sql: &str,
    params: P,
    work: &mut SourceAssessmentWork,
) -> DurableResult<Option<i64>> {
    let mut statement = db.prepare(sql).map_err(sqlite_error)?;
    let mut rows = statement.query(params).map_err(sqlite_error)?;
    let Some(row) = rows.next().map_err(sqlite_error)? else {
        return Ok(None);
    };
    add_counter(&mut work.sqlite_rows_returned, 1)?;
    let value: i64 = row.get(0).map_err(sqlite_error)?;
    add_read_bytes(work, 8)?;
    Ok(Some(value))
}

fn read_member(
    db: &Connection,
    path: &str,
    limits: SourceAssessmentLimits,
    work: &mut SourceAssessmentWork,
) -> DurableResult<Option<SourceAssessmentMemberOwned>> {
    let (sha, size, mode, dependencies_present, seen) = {
        let mut statement = db
            .prepare(
                "SELECT sha256,size_bytes,mode,dependencies_present,seen
                 FROM members WHERE path=?1",
            )
            .map_err(sqlite_error)?;
        let mut rows = statement.query(params![path]).map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Ok(None);
        };
        add_counter(&mut work.sqlite_rows_returned, 1)?;
        check_row_limit(work, limits)?;
        let sha: Vec<u8> = row.get(0).map_err(sqlite_error)?;
        let size: i64 = row.get(1).map_err(sqlite_error)?;
        let mode: i64 = row.get(2).map_err(sqlite_error)?;
        let dependencies_present: i64 = row.get(3).map_err(sqlite_error)?;
        let seen: i64 = row.get(4).map_err(sqlite_error)?;
        count_row_read(work, sha.len() + 8 + 8 + 1 + 1)?;
        check_logical_limit(work, limits)?;
        (sha, size, mode, dependencies_present, seen)
    };
    if sha.len() != 32
        || size < 0
        || mode < 0
        || mode > 0o7777
        || !(0..=1).contains(&dependencies_present)
        || !(0..=1).contains(&seen)
    {
        return Err(DurableError::Corrupt("stored source member malformed"));
    }
    let mut digest = [0; 32];
    digest.copy_from_slice(&sha);
    let (dependency_count, dependency_bytes, max_dependency_bytes) = {
        let mut statement = db
            .prepare(
                "SELECT count(*),coalesce(sum(length(CAST(target_path AS BLOB))),0),
                        coalesce(max(length(CAST(target_path AS BLOB))),0)
                 FROM member_dependencies WHERE source_path=?1",
            )
            .map_err(sqlite_error)?;
        let mut rows = statement.query(params![path]).map_err(sqlite_error)?;
        let row = rows
            .next()
            .map_err(sqlite_error)?
            .ok_or(DurableError::Corrupt("dependency aggregate witness absent"))?;
        add_counter(&mut work.sqlite_rows_returned, 1)?;
        check_row_limit(work, limits)?;
        add_counter(&mut work.sqlite_aggregate_witnesses_returned, 1)?;
        let count: i64 = row.get(0).map_err(sqlite_error)?;
        let bytes: i64 = row.get(1).map_err(sqlite_error)?;
        let max_bytes: i64 = row.get(2).map_err(sqlite_error)?;
        count_row_read(work, 24)?;
        check_logical_limit(work, limits)?;
        let count = u64::try_from(count)
            .map_err(|_| DurableError::Corrupt("dependency count malformed"))?;
        let bytes = u64::try_from(bytes)
            .map_err(|_| DurableError::Corrupt("dependency byte count malformed"))?;
        let max_bytes = u64::try_from(max_bytes)
            .map_err(|_| DurableError::Corrupt("dependency maximum malformed"))?;
        add_counter(&mut work.sqlite_dependency_rows_witnessed, count)?;
        add_counter(&mut work.sqlite_dependency_bytes_witnessed, bytes)?;
        if count > limits.max_rows
            || bytes > limits.max_logical_bytes
            || max_bytes > limits.max_key_bytes as u64
        {
            return Err(DurableError::Refused(
                "stored source dependencies exceed assessment bound",
            ));
        }
        (count, bytes, max_bytes)
    };
    let dependencies = if dependencies_present == 0 {
        if dependency_count != 0 {
            return Err(DurableError::Corrupt(
                "absent dependency claim has indexed targets",
            ));
        }
        None
    } else {
        let capacity = usize::try_from(dependency_count)
            .map_err(|_| DurableError::Refused("source dependency count exceeds memory"))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| DurableError::Refused("source dependency allocation failed"))?;
        let mut statement = db
            .prepare(
                "SELECT target_path FROM member_dependencies
                 WHERE source_path=?1 ORDER BY target_path COLLATE BINARY",
            )
            .map_err(sqlite_error)?;
        let mut rows = statement.query(params![path]).map_err(sqlite_error)?;
        let mut returned = 0u64;
        let mut returned_bytes = 0u64;
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            if returned >= dependency_count {
                return Err(DurableError::Corrupt(
                    "dependency rows exceed aggregate witness",
                ));
            }
            add_counter(&mut work.sqlite_rows_returned, 1)?;
            check_row_limit(work, limits)?;
            let value: String = row.get(0).map_err(sqlite_error)?;
            count_row_read(work, value.len())?;
            check_logical_limit(work, limits)?;
            validate_path(&value, limits.max_key_bytes)?;
            returned = returned
                .checked_add(1)
                .ok_or(DurableError::Refused("dependency row count overflow"))?;
            returned_bytes = returned_bytes
                .checked_add(value.len() as u64)
                .ok_or(DurableError::Refused("dependency byte count overflow"))?;
            values.push(value);
        }
        if returned != dependency_count || returned_bytes != dependency_bytes {
            return Err(DurableError::Corrupt(
                "dependency rows differ from aggregate witness",
            ));
        }
        Some(values)
    };
    let _ = max_dependency_bytes;
    check_logical_limit(work, limits)?;
    Ok(Some(SourceAssessmentMemberOwned {
        path: path.to_owned(),
        sha256: digest,
        size_bytes: size as u64,
        mode: mode as u32,
        dependencies,
    }))
}

fn count_row_read(work: &mut SourceAssessmentWork, bytes: usize) -> DurableResult<()> {
    add_read_bytes(work, bytes)
}
