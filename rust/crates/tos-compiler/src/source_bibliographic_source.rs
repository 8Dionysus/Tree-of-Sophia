//! Cold current authored source-cut recipe for the maintained catalog and graph.
//! A private bounded plan freezes exact original-file roots before the caller
//! creates its independently receipted stage. Generated catalog artifacts and
//! retained archives are not relabelled as current catalog source files.
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::{
    CandidateExactInputReceipt, CandidateValidationBinding, ColdAuthoredBinding,
    ColdExactInputReceipt, ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage,
};
use crate::source_bibliographic::{
    self as graph, BibliographicForms, BibliographicLimits, BibliographicReceipt,
    BibliographicSourceCut,
};
use crate::source_witness_catalog::{
    self as catalog, BIBLIOGRAPHIC_FILES, CATALOG_SOURCE, CONTRACT_FILES, NATIVE_IDENTITIES,
    NATIVE_TEXT, SOURCE_FILES, SourceCatalogReceipt, SourceCatalogValidator,
};
use crate::{Error, Result, SourceBinding};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};
use tos_validation::record_biblio_cut::{
    SourceCutInput, SourceCutInputCoverage, SourceCutInputWithIdentity,
};

const ENTITY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const COLLECTIONS: [&str; 5] = [
    SOURCE_FILES,
    CONTRACT_FILES,
    NATIVE_IDENTITIES,
    NATIVE_TEXT,
    BIBLIOGRAPHIC_FILES,
];

#[derive(Clone, Copy, Debug)]
pub struct SourceCatalogInputLimits {
    pub max_manifest_members: u64,
    pub max_selected_members: usize,
    /// Includes selected locator/index/queue storage. Absent paths allocate no
    /// retained state and cannot become fabricated raw input rows.
    pub max_plan_bytes: usize,
    /// Original bytes observed while planning or transferring, with repeats.
    pub max_work_bytes: u64,
}
impl SourceCatalogInputLimits {
    pub fn validate(self, l: BibliographicLimits) -> Result<()> {
        l.validate()?;
        l.catalog.validate()?;
        if self.max_manifest_members == 0
            || self.max_selected_members == 0
            || self.max_selected_members > 4096
            || self.max_plan_bytes == 0
            || self.max_plan_bytes > 64 * 1024 * 1024
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("cold source catalog input limits"));
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Member {
    size: u64,
    sha: Digest256,
    collections: BTreeSet<&'static str>,
}
/// Only the producer can create the selected member plan. The receipt is a
/// clone for final-stage creation; changing it cannot change this private seal.
struct PlanInput<B> {
    binding: B,
    collections: Vec<InputCollectionReceipt>,
}
pub struct SourceCatalogInputPlan<B = SourceBinding> {
    receipt: PlanInput<B>,
    revision: Option<SourceRevision>,
    membership: SourceMembershipV1,
    members: BTreeMap<String, Member>,
    limits: SourceCatalogInputLimits,
    work_bytes: u64,
    retained_plan_bytes: usize,
    raw_input_max_bytes: usize,
    workspace_limit: Option<usize>,
}
impl SourceCatalogInputPlan {
    /// A projection plan must retain the independently selected real revision.
    pub fn source_revision(&self) -> Result<SourceRevision> {
        self.revision
            .ok_or(Error::Invalid("projection plan revision missing"))
    }
    pub fn input_receipt(&self) -> ExactInputReceipt {
        ExactInputReceipt {
            binding: self.receipt.binding.clone(),
            collections: self.receipt.collections.clone(),
        }
    }
}
pub type ColdSourceCatalogInputPlan = SourceCatalogInputPlan<ColdAuthoredBinding>;
impl SourceCatalogInputPlan<ColdAuthoredBinding> {
    pub fn source_revision(&self) -> SourceRevision {
        self.receipt.binding.revision()
    }
    pub fn input_receipt(&self) -> ColdExactInputReceipt {
        ColdExactInputReceipt {
            binding: self.receipt.binding.clone(),
            collections: self.receipt.collections.clone(),
        }
    }
}
impl<B> SourceCatalogInputPlan<B> {
    pub fn selected_member_count(&self) -> usize {
        self.members.len()
    }
    pub fn source_membership(&self) -> SourceMembershipV1 {
        self.membership
    }
    pub fn observed_work_bytes(&self) -> u64 {
        self.work_bytes
    }
    /// Declared temporary parser/plan ceiling, not observed RSS or retained plan bytes.
    pub fn temporary_workspace_limit(&self) -> Option<usize> {
        self.workspace_limit
    }
    pub(crate) fn verify_selected_member_bytes(&self, path: &str, raw: &[u8]) -> Result<()> {
        let member = self
            .members
            .get(path)
            .ok_or(Error::Invalid("source plan selected member absent"))?;
        if member.size != raw.len() as u64 || member.sha != Digest256::of_bytes(raw) {
            return Err(Error::Invalid("source plan exact selected member bytes"));
        }
        Ok(())
    }
}
pub struct SourceBibliographicCandidate<B = SourceBinding> {
    pub catalog: SourceCatalogReceipt<B>,
    pub bibliographic: BibliographicReceipt,
}
/// Preserve the source adapter refusal category without requiring Display or
/// inventing observations absent from the adapter's actual error.
#[track_caller]
pub(crate) fn candidate_input_refusal(error: tos_validation::item_rules::ItemRefusal) -> Error {
    use tos_validation::item_rules::ItemRefusal;
    let origin = std::panic::Location::caller();
    let source_cause = |detail: &str| {
        let district = if origin.file().ends_with("source_bibliographic_versions.rs") {
            "biblio-versions"
        } else {
            "biblio-source"
        };
        Error::Source(format!(
            "source-cause:candidate-input:{district}-{}:{}",
            origin.line(),
            Digest256::of_bytes(detail.as_bytes()).to_hex(),
        ))
    };
    match error.compatibility_category() {
        ItemRefusal::Budget => Error::Budget("candidate source input"),
        ItemRefusal::BudgetCheck { check, .. } => Error::Budget(check),
        ItemRefusal::Deadline => Error::Budget("candidate source input deadline"),
        ItemRefusal::Source(detail) => source_cause(&detail),
        ItemRefusal::Executor(evidence) => Error::Source(evidence.summary()),
        ItemRefusal::Unsupported(detail) => source_cause(&detail),
    }
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("cold source catalog cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("cold source catalog deadline"));
    }
    Ok(())
}
fn selected_cut(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    expected: SourceMembershipV1,
    limits: SourceCatalogInputLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    check(deadline, cancelled)?;
    if cut.current().revision() != revision
        || cut
            .stream(revision)
            .map_err(|_| Error::Invalid("cold catalog selected revision"))?
            .expectation()
            != expected
    {
        return Err(Error::Invalid("cold catalog independent source selection"));
    }
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-val-full-membership-v1\0");
    let mut count = 0u64;
    for member in cut.current().members() {
        check(deadline, cancelled)?;
        count = count
            .checked_add(1)
            .filter(|n| *n <= limits.max_manifest_members)
            .ok_or(Error::Budget("cold catalog manifest EOF"))?;
        let path = member.path.as_str();
        hash.update(&(path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
        hash.update(&member.size_bytes.to_be_bytes());
        hash.update(member.sha256.as_bytes());
    }
    if (SourceMembershipV1 {
        count,
        digest: hash.finalize(),
    }) != expected
    {
        return Err(Error::Invalid("cold catalog complete metadata membership"));
    }
    Ok(())
}
fn public_path(path: &str) -> bool {
    path.starts_with("ToS/")
        && path.len() <= 4096
        && !path.contains(['\\', '\0'])
        && path.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !matches!(
                    part,
                    "catalog" | "payload" | "private" | "owner-local" | "local-content"
                )
        })
}
/// Host-held kernel quota/custody for the selected locator database, including
/// its connection/cache lifetime. There is no default permissive owner.
#[derive(Clone, Copy, Debug)]
pub enum ColdSourceCatalogSpoolPhase {
    Create,
    Grow,
    Read,
    Finish,
}
pub trait ColdSourceCatalogSpoolIsolation {
    fn verify(
        &self,
        file: &std::fs::File,
        limits: ColdSourceCatalogSpoolLimits,
        phase: ColdSourceCatalogSpoolPhase,
    ) -> Result<()>;
}
#[derive(Clone, Copy, Debug)]
pub struct ColdSourceCatalogSpoolLimits {
    pub max_selected_members: u64,
    pub max_locator_bytes: u64,
    pub max_sqlite_bytes: u64,
    pub max_work_bytes: u64,
    pub sqlite_cache_kib: u32,
    pub max_sql_vm_steps: u64,
    pub max_workspace_bytes: usize,
}
enum PlanningDatabase {
    Owned(tos_source_store::PinnedSqliteConnection),
    Shared(std::rc::Rc<tos_source_store::PinnedSqliteConnection>),
}
impl std::ops::Deref for PlanningDatabase {
    type Target = tos_source_store::PinnedSqliteConnection;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(db) => db,
            Self::Shared(db) => db,
        }
    }
}
struct DiskEntries<'a> {
    db: PlanningDatabase,
    file: Option<std::fs::File>,
    isolation: Option<&'a dyn ColdSourceCatalogSpoolIsolation>,
    limits: ColdSourceCatalogSpoolLimits,
    rows: u64,
    logical_bytes: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    cleanup_complete: std::cell::Cell<bool>,
}
impl DiskEntries<'_> {
    fn guard(&self, phase: ColdSourceCatalogSpoolPhase) -> Result<()> {
        check(self.deadline, self.cancelled)?;
        if let (Some(file), Some(isolation)) = (&self.file, self.isolation) {
            isolation.verify(file, self.limits, phase)?;
        } else {
            // SAME pinned native connection: its VFS owns prewrite whole IO,
            // original allocation, deadline/cancellation and auxiliary caps.
            let page: u64 = self.db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
            let count: u64 = self.db.query_row("PRAGMA page_count", [], |r| r.get(0))?;
            if page
                .checked_mul(count)
                .is_none_or(|bytes| bytes > self.limits.max_sqlite_bytes)
            {
                return Err(Error::Budget("candidate catalog shared SQLite ceiling"));
            }
        }
        check(self.deadline, self.cancelled)
    }
    fn finish_candidate_cleanup(&self) -> Result<()> {
        if !matches!(self.db, PlanningDatabase::Shared(_)) || self.cleanup_complete.get() {
            return Ok(());
        }
        self.guard(ColdSourceCatalogSpoolPhase::Finish)?;
        let selected: u64 = self
            .db
            .query_row("SELECT COUNT(*) FROM selected", [], |r| r.get(0))?;
        let pending: u64 = self
            .db
            .query_row("SELECT COUNT(*) FROM pending", [], |r| r.get(0))?;
        if selected != self.rows || pending != 0 {
            return Err(Error::Invalid("candidate catalog planner cleanup EOF"));
        }
        self.db
            .execute_batch("DROP TABLE selected; DROP TABLE pending; DROP TABLE scanned;")?;
        let remaining: u64 = self.db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('selected','pending','scanned')", [], |r| r.get(0))?;
        if remaining != 0 {
            return Err(Error::Invalid("candidate catalog planner cleanup closure"));
        }
        self.guard(ColdSourceCatalogSpoolPhase::Finish)?;
        self.cleanup_complete.set(true);
        Ok(())
    }
}
impl Drop for DiskEntries<'_> {
    fn drop(&mut self) {
        if matches!(self.db, PlanningDatabase::Shared(_)) && !self.cleanup_complete.get() {
            // Failure remains in the original pinned VFS/IO owner; this does
            // not refund a cumulative byte or physical reservation.
            let _ = self.db.execute_batch("DROP TABLE IF EXISTS selected; DROP TABLE IF EXISTS pending; DROP TABLE IF EXISTS scanned;");
        }
    }
}
enum PlanningEntries<'a> {
    Resident {
        members: BTreeMap<String, Member>,
        pending: BTreeSet<String>,
        scanned: BTreeSet<String>,
    },
    Disk(DiskEntries<'a>),
}
fn collection_flag(collection: &str) -> Result<u32> {
    COLLECTIONS
        .iter()
        .position(|c| *c == collection)
        .map(|i| 1u32 << i)
        .ok_or(Error::Invalid("cold catalog collection flag"))
}
impl PlanningEntries<'_> {
    fn get(&self, path: &str) -> Result<Option<Member>> {
        use rusqlite::OptionalExtension;
        match self {
            Self::Resident { members, .. } => Ok(members.get(path).cloned()),
            Self::Disk(d) => {
                d.guard(ColdSourceCatalogSpoolPhase::Read)?;
                let row =
                    d.db.query_row(
                        "SELECT size,sha,flags FROM selected WHERE path=?1",
                        [path],
                        |r| {
                            Ok((
                                r.get::<_, Vec<u8>>(0)?,
                                r.get::<_, Vec<u8>>(1)?,
                                r.get::<_, u32>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                d.guard(ColdSourceCatalogSpoolPhase::Read)?;
                row.map(|(size, sha, flags)| {
                    let size = u64::from_be_bytes(
                        size.try_into()
                            .map_err(|_| Error::Invalid("cold spool size"))?,
                    );
                    Ok(Member {
                        size,
                        sha: Digest256::from_bytes(
                            sha.try_into()
                                .map_err(|_| Error::Invalid("cold spool digest"))?,
                        ),
                        collections: COLLECTIONS
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| flags & (1 << i) != 0)
                            .map(|(_, c)| *c)
                            .collect(),
                    })
                })
                .transpose()
            }
        }
    }
    fn queue(&mut self, path: &str) -> Result<()> {
        match self {
            Self::Resident {
                pending, scanned, ..
            } => {
                if !scanned.contains(path) {
                    pending.insert(path.to_owned());
                }
            }
            Self::Disk(d) => {
                d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
                d.db.execute("INSERT OR IGNORE INTO pending SELECT ?1 WHERE NOT EXISTS(SELECT 1 FROM scanned WHERE path=?1)",[path])?;
                d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
            }
        }
        Ok(())
    }
    fn queued(&self, path: &str) -> Result<bool> {
        match self {
            Self::Resident {
                pending, scanned, ..
            } => Ok(pending.contains(path) || scanned.contains(path)),
            Self::Disk(d) => {
                d.guard(ColdSourceCatalogSpoolPhase::Read)?;
                let result = d.db.query_row("SELECT EXISTS(SELECT 1 FROM pending WHERE path=?1) OR EXISTS(SELECT 1 FROM scanned WHERE path=?1)", [path], |r| r.get(0));
                d.guard(ColdSourceCatalogSpoolPhase::Read)?;
                Ok(result?)
            }
        }
    }
    fn pop(&mut self) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        match self {
            Self::Resident {
                pending, scanned, ..
            } => {
                let path = pending.pop_first();
                if let Some(path) = &path {
                    scanned.insert(path.clone());
                }
                Ok(path)
            }
            Self::Disk(d) => {
                d.guard(ColdSourceCatalogSpoolPhase::Read)?;
                let path: Option<String> =
                    d.db.query_row("SELECT path FROM pending ORDER BY path LIMIT 1", [], |r| {
                        r.get(0)
                    })
                    .optional()?;
                d.guard(ColdSourceCatalogSpoolPhase::Read)?;
                if let Some(path) = &path {
                    d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
                    d.db.execute("INSERT INTO scanned VALUES (?1)", [path])?;
                    d.db.execute("DELETE FROM pending WHERE path=?1", [path])?;
                    d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
                }
                Ok(path)
            }
        }
    }
    fn len(&self) -> u64 {
        match self {
            Self::Resident { members, .. } => members.len() as u64,
            Self::Disk(d) => d.rows,
        }
    }
    fn set(&mut self, path: &str, member: Member) -> Result<()> {
        match self {
            Self::Resident { members, .. } => {
                members.insert(path.to_owned(), member);
            }
            Self::Disk(d) => {
                d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
                let mut flags = 0u32;
                for c in &member.collections {
                    flags |= collection_flag(c)?;
                }
                let added=d.db.execute("INSERT INTO selected VALUES (?1,?2,?3,?4) ON CONFLICT(path) DO UPDATE SET flags=excluded.flags",rusqlite::params![path,member.size.to_be_bytes().as_slice(),member.sha.as_bytes().as_slice(),flags])?;
                let _ = added;
                d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
            }
        }
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> Result<()> {
        if let Self::Disk(d) = self {
            d.logical_bytes = d
                .logical_bytes
                .checked_add(bytes as u64)
                .filter(|n| *n <= d.limits.max_locator_bytes)
                .ok_or(Error::Budget("cold spool locator bytes"))?;
            d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
        }
        Ok(())
    }
    fn next(&self, after: Option<&str>) -> Result<Option<(String, Member)>> {
        use rusqlite::OptionalExtension;
        if let Self::Disk(d) = self {
            d.guard(ColdSourceCatalogSpoolPhase::Read)?;
        }
        let path = match self {
            Self::Resident { members, .. } => members
                .keys()
                .find(|p| after.is_none_or(|a| p.as_str() > a))
                .cloned(),
            Self::Disk(d) => match after {
                Some(after) => {
                    d.db.query_row(
                        "SELECT path FROM selected WHERE path>?1 ORDER BY path LIMIT 1",
                        [after],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                }
                None => {
                    d.db.query_row("SELECT path FROM selected ORDER BY path LIMIT 1", [], |r| {
                        r.get::<_, String>(0)
                    })
                    .optional()?
                }
            },
        };
        if let Self::Disk(d) = self {
            d.guard(ColdSourceCatalogSpoolPhase::Read)?;
        }
        path.map(|p| {
            self.get(&p)?
                .map(|m| (p, m))
                .ok_or(Error::Invalid("cold spool selected row lost"))
        })
        .transpose()
    }
    fn new_member(&mut self) -> Result<()> {
        if let Self::Disk(d) = self {
            d.rows = d
                .rows
                .checked_add(1)
                .filter(|n| *n <= d.limits.max_selected_members)
                .ok_or(Error::Budget("cold spool selected members"))?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum PlanningCut<'a> {
    Candidate(&'a dyn SourceCutInput),
    Resident(&'a CorpusCutReader),
    Streamed(&'a tos_source_store::StreamedCorpusCutReaderV1),
}
impl PlanningCut<'_> {
    fn record_selection(
        &self,
    ) -> Option<std::sync::Arc<tos_validation::source_record_selection::SourceRecordSelection>>
    {
        match self {
            Self::Candidate(input) => input.record_selection(),
            _ => None,
        }
    }
    fn generated_selection(
        &self,
    ) -> Option<std::sync::Arc<dyn tos_validation::record_biblio_cut::GeneratedSourceSelection>>
    {
        match self {
            Self::Candidate(input) => input.generated_selection(),
            _ => None,
        }
    }
    fn selects_semantic_member(&self, path: &str) -> Result<bool> {
        match self {
            Self::Candidate(input) => input
                .selects_semantic_member(path)
                .map_err(|error| candidate_input_refusal(error)),
            _ => Ok(true),
        }
    }
    fn selects_catalog_member(&self, path: &str) -> Result<bool> {
        let finite = self.record_selection();
        let generated = self.generated_selection();
        if finite.is_none() && generated.is_none() {
            return Ok(true);
        }
        if finite
            .as_ref()
            .is_some_and(|selection| selection.selects_semantic_member(path))
        {
            return Ok(true);
        }
        generated
            .as_ref()
            .map_or(Ok(false), |selection| {
                selection.selects_catalog_member(path)
            })
            .map_err(|error| candidate_input_refusal(error))
    }
    fn selects_required_member(&self, path: &str) -> Result<bool> {
        match self {
            Self::Candidate(input) => input
                .selects_required_member(path)
                .map_err(|error| candidate_input_refusal(error)),
            _ => Ok(true),
        }
    }
    fn file_present(
        &self,
        revision: Option<SourceRevision>,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool> {
        check(deadline, cancelled)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| Error::Invalid("catalog dependency relative path"))?;
        match self {
            Self::Candidate(input) => input
                .path_presence(path, deadline, cancelled)
                .map(|presence| presence == Some(tos_source_store::SourcePresenceV1::File))
                .map_err(|error| candidate_input_refusal(error)),
            Self::Resident(cut) => Ok(cut.current().member(&relative).is_some()),
            Self::Streamed(cut) => cut
                .member(
                    revision.ok_or(Error::Invalid("streamed plan revision missing"))?,
                    &relative,
                )
                .map(|member| member.is_some())
                .map_err(|error| Error::Source(error.to_string())),
        }
    }
    fn member(
        &self,
        revision: Option<SourceRevision>,
        path: &RelativePath,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut u64,
        max_work: u64,
    ) -> Result<Option<(u64, Digest256)>> {
        match self {
            Self::Candidate(input) => {
                check(deadline, cancelled)?;
                if input
                    .path_presence(path.as_str(), deadline, cancelled)
                    .map_err(|error| candidate_input_refusal(error))?
                    != Some(tos_source_store::SourcePresenceV1::File)
                {
                    return Ok(None);
                }
                let mut facts = None;
                input
                    .with_current_member(
                        path.as_str(),
                        cap,
                        deadline,
                        cancelled,
                        &mut |meta, raw| {
                            if meta.path != path.as_str()
                                || meta.size_bytes != raw.len() as u64
                                || raw.len() > cap
                            {
                                return Err(tos_validation::item_rules::ItemRefusal::Source(
                                    "candidate member facts".into(),
                                ));
                            }
                            *work = work
                                .checked_add(meta.size_bytes)
                                .filter(|n| *n <= max_work)
                                .ok_or(tos_validation::item_rules::ItemRefusal::Budget)?;
                            facts = Some((meta.size_bytes, Digest256::of_bytes(raw)));
                            Ok(())
                        },
                    )
                    .map_err(|error| candidate_input_refusal(error))?;
                check(deadline, cancelled)?;
                Ok(facts)
            }
            Self::Resident(cut) => Ok(cut.current().member(path).map(|m| (m.size_bytes, m.sha256))),
            Self::Streamed(cut) => cut
                .member(
                    revision.ok_or(Error::Invalid("streamed plan revision missing"))?,
                    path,
                )
                .map(|m| m.map(|m| (m.size_bytes, m.sha256)))
                .map_err(|e| Error::Source(e.to_string())),
        }
    }
    fn read(
        &self,
        revision: Option<SourceRevision>,
        path: &RelativePath,
        cap: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        match self {
            Self::Candidate(input) => {
                let cap =
                    usize::try_from(cap).map_err(|_| Error::Budget("candidate read byte range"))?;
                let mut result = None;
                input
                    .with_current_member(
                        path.as_str(),
                        cap,
                        deadline,
                        cancelled,
                        &mut |meta, raw| {
                            if meta.path != path.as_str()
                                || meta.size_bytes != raw.len() as u64
                                || raw.len() > cap
                            {
                                return Err(tos_validation::item_rules::ItemRefusal::Source(
                                    "candidate read identity".into(),
                                ));
                            }
                            result = Some(raw.to_vec());
                            Ok(())
                        },
                    )
                    .map_err(|error| candidate_input_refusal(error))?;
                check(deadline, cancelled)?;
                return result.ok_or(Error::Invalid("candidate source member absent"));
            }
            Self::Resident(cut) => cut
                .read_member(
                    revision.ok_or(Error::Invalid("resident plan revision missing"))?,
                    path,
                    cap,
                    deadline,
                    cancelled,
                )
                .map(|m| m.raw),
            Self::Streamed(cut) => cut
                .read_member(
                    revision.ok_or(Error::Invalid("resident plan revision missing"))?,
                    path,
                    cap,
                    deadline,
                    cancelled,
                )
                .map(|m| m.raw),
        }
        .map_err(|e| Error::Source(e.to_string()))
    }
}
struct Planning<'a, 's> {
    cut: PlanningCut<'a>,
    revision: Option<SourceRevision>,
    limits: SourceCatalogInputLimits,
    graph_limits: BibliographicLimits,
    cancelled: &'a AtomicBool,
    entries: PlanningEntries<'s>,
    plan_bytes: usize,
    work_bytes: u64,
    raw_input_max_bytes: usize,
    json_input_max_bytes: usize,
    cold_raw_parser: bool,
    workspace_limit: Option<usize>,
    live_raw_bytes: usize,
    live_parser_upper: usize,
}
impl Planning<'_, '_> {
    fn charge(&mut self, bytes: usize) -> Result<()> {
        if matches!(self.entries, PlanningEntries::Resident { .. }) {
            self.plan_bytes = self
                .plan_bytes
                .checked_add(bytes)
                .filter(|n| *n <= self.limits.max_plan_bytes)
                .ok_or(Error::Budget("cold catalog plan state"))?;
        }
        self.entries.charge(bytes)?;
        self.check_workspace()
    }
    fn check_workspace(&self) -> Result<()> {
        if let Some(limit) = self.workspace_limit {
            self.plan_bytes
                .checked_add(self.live_raw_bytes)
                .and_then(|n| n.checked_add(self.live_parser_upper))
                .filter(|n| *n <= limit)
                .ok_or(Error::Budget("cold catalog temporary workspace"))?;
        }
        Ok(())
    }
    fn parse_packet(&mut self, raw: &[u8], cap: usize) -> Result<SourceRow> {
        if self.cold_raw_parser {
            if let Some(limit) = self.workspace_limit {
                let available = limit
                    .checked_sub(self.plan_bytes)
                    .and_then(|n| n.checked_sub(self.live_raw_bytes))
                    .ok_or(Error::Budget("cold catalog parser workspace"))?;
                let (parsed, upper) =
                    SourceRow::parse_raw_input_with_state_budget(raw, cap, available)?;
                self.live_parser_upper = upper;
                check(self.graph_limits.deadline, self.cancelled)?;
                return Ok(parsed);
            }
            SourceRow::parse_raw_input(raw, cap)
        } else {
            SourceRow::parse(raw, cap)
        }
    }
    fn queue(&mut self, path: &str) -> Result<()> {
        if !self.entries.queued(path)? {
            self.charge(
                path.len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(128))
                    .ok_or(Error::Budget("cold catalog locator state"))?,
            )?;
            self.entries.queue(path)?;
        }
        Ok(())
    }
    fn select(&mut self, path: &str, collection: &'static str) -> Result<bool> {
        if !public_path(path) {
            return Err(Error::Invalid("cold catalog public dependency path"));
        }
        let relative =
            RelativePath::parse(path).map_err(|_| Error::Invalid("cold catalog relative path"))?;
        let Some((size, sha)) = self.cut.member(
            self.revision,
            &relative,
            self.raw_input_max_bytes,
            self.graph_limits.deadline,
            self.cancelled,
            &mut self.work_bytes,
            self.limits.max_work_bytes,
        )?
        else {
            return Ok(false);
        };
        if size > self.raw_input_max_bytes as u64 {
            return Err(Error::Budget(
                "cold catalog original source exceeds raw stage row cap",
            ));
        }
        if self.entries.get(path)?.is_none() {
            if matches!(self.entries, PlanningEntries::Resident { .. })
                && self.entries.len() >= self.limits.max_selected_members as u64
            {
                return Err(Error::Budget("cold catalog selected files"));
            }
            self.charge(
                path.len()
                    .checked_add(256)
                    .ok_or(Error::Budget("cold catalog member state"))?,
            )?;
            self.entries.new_member()?;
            self.entries.set(
                path,
                Member {
                    size,
                    sha,
                    collections: BTreeSet::new(),
                },
            )?;
            self.queue(path)?;
        }
        let mut member = self
            .entries
            .get(path)?
            .ok_or(Error::Invalid("cold catalog member lost"))?;
        if !member.collections.contains(collection) {
            self.charge(128)?;
        }
        member.collections.insert(collection);
        // The maintained readers search these unions and reject duplicate raw
        // paths. A primary file remains primary when later referenced.
        if member.collections.contains(SOURCE_FILES) {
            member.collections.remove(NATIVE_TEXT);
            member.collections.remove(BIBLIOGRAPHIC_FILES);
        }
        if member.collections.contains(CONTRACT_FILES) {
            member.collections.remove(BIBLIOGRAPHIC_FILES);
        }
        self.entries.set(path, member)?;
        Ok(true)
    }
    fn raw(&mut self, path: &str) -> Result<Vec<u8>> {
        check(self.graph_limits.deadline, self.cancelled)?;
        let member = self
            .entries
            .get(path)?
            .ok_or(Error::Invalid("cold catalog raw plan absent"))?;
        self.work_bytes = self
            .work_bytes
            .checked_add(member.size)
            .filter(|n| *n <= self.limits.max_work_bytes)
            .ok_or(Error::Budget("cold catalog raw work"))?;
        let size = member.size;
        let sha = member.sha;
        self.live_raw_bytes =
            usize::try_from(size).map_err(|_| Error::Budget("cold catalog raw workspace range"))?;
        if self.workspace_limit.is_some() {
            // TimedStage grows a Vec geometrically: reserve old/new buffer
            // overlap and loader scratch before read, not raw length as RSS.
            self.live_raw_bytes = self
                .live_raw_bytes
                .max(8)
                .checked_mul(4)
                .and_then(|n| n.checked_add(128 * 1024))
                .ok_or(Error::Budget("cold catalog raw workspace range"))?;
            let relative =
                RelativePath::parse(path).map_err(|_| Error::Invalid("cold catalog raw path"))?;
            if let PlanningCut::Resident(cut) = &self.cut {
                for id in cut.current().indexed_ids_for_path(&relative) {
                    self.live_raw_bytes = self
                        .live_raw_bytes
                        .checked_add(id.len())
                        .and_then(|n| n.checked_add(4 * std::mem::size_of::<String>()))
                        .ok_or(Error::Budget("cold catalog source IDs workspace"))?;
                }
            }
        }
        self.live_parser_upper = 0;
        self.check_workspace()?;
        let raw = self.cut.read(
            self.revision,
            &RelativePath::parse(path).map_err(|_| Error::Invalid("cold catalog raw path"))?,
            self.raw_input_max_bytes as u64,
            self.graph_limits.deadline,
            self.cancelled,
        )?;
        if raw.len() as u64 != size || Digest256::of_bytes(&raw) != sha {
            return Err(Error::Invalid("cold catalog original bytes binding"));
        }
        Ok(raw)
    }
    fn dependency(&mut self, path: &str) -> Result<()> {
        // Only actual positive public metadata members join the raw recipe.
        // Payload/corpus bytes and generated catalog carriers remain outside.
        if public_path(path) {
            // The renderer also probes optional companions and directory
            // references. A strict generated-record selector only accepts real
            // member paths; absence must be decided by the held input first.
            if !self.cut.file_present(
                self.revision,
                path,
                self.graph_limits.deadline,
                self.cancelled,
            )? {
                return Ok(());
            }
            if !self.cut.selects_required_member(path)? {
                // Optional producer companions do not enlarge the
                // declared closure. Required-reference validity remains
                // with the already executed Records/Discovery/Closure owners.
                return Ok(());
            }
            self.select(path, NATIVE_TEXT)?;
            self.select(path, BIBLIOGRAPHIC_FILES)?;
        }
        Ok(())
    }
    fn value(&mut self, value: &Value, depth: usize) -> Result<()> {
        check(self.graph_limits.deadline, self.cancelled)?;
        if depth > 96 {
            return Err(Error::Budget("cold catalog dependency depth"));
        }
        match value {
            Value::String(path) if path.starts_with("ToS/") => self.dependency(path)?,
            Value::Array(values) => {
                for value in values {
                    self.value(value, depth + 1)?;
                }
            }
            Value::Object(values) => {
                for value in values.values() {
                    self.value(value, depth + 1)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    fn initial(&mut self, path: &str, basenames: &BTreeSet<String>) -> Result<()> {
        let basename = path.rsplit('/').next().unwrap_or("");
        if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") && public_path(path)
        {
            self.select(path, CONTRACT_FILES)?;
        }
        if path.starts_with("ToS/source-witnesses/") {
            if !self.cut.selects_catalog_member(path)? {
                return Ok(());
            }
            // Native packets retain their taxonomy and one collection root;
            // a profile basename must not count the same packet as a record.
            if basename != "source-text-unit-packet.v1.json"
                && (basenames.contains(basename)
                    || basename.ends_with(".jsonl")
                        && (basename.contains("provenance") || basename.contains("anchor")))
            {
                catalog::source_ref(path)?;
                self.select(path, SOURCE_FILES)?;
            }
            if (basename.starts_with("semantic-annotation") && basename.ends_with(".json")
                || basename == "source-text-unit-packet.v1.json")
                && !path
                    .split('/')
                    .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
            {
                catalog::source_ref(path)?;
                self.select(path, NATIVE_IDENTITIES)?;
            }
        }
        Ok(())
    }
    fn packet(&mut self, path: &str, raw: &[u8]) -> Result<()> {
        let selection = self.cut.record_selection();
        let generated = self.cut.generated_selection();
        let generated_member = generated
            .as_ref()
            .map_or(Ok(false), |selection| {
                selection.selects_required_member(path)
            })
            .map_err(|error| candidate_input_refusal(error))?;
        if !path.starts_with("ToS/contracts/") && path != ENTITY && path != RELATION {
            if generated_member {
                let caller_state = self
                    .plan_bytes
                    .checked_add(self.live_raw_bytes)
                    .ok_or(Error::Budget("generated catalog verifier overlap"))?;
                generated
                    .as_ref()
                    .ok_or(Error::Invalid("generated catalog selection absent"))?
                    .verify_member(
                        path,
                        raw,
                        caller_state,
                        self.graph_limits.deadline,
                        self.cancelled,
                    )
                    .map_err(|error| candidate_input_refusal(error))?;
            } else if let Some(selection) = &selection {
                selection
                    .verify_metadata_member(path, raw)
                    .map_err(|error| candidate_input_refusal(error))?;
            }
            if !self.cut.selects_semantic_member(path)? {
                return Ok(());
            }
        }
        let member = self
            .entries
            .get(path)?
            .ok_or(Error::Invalid("cold catalog packet absent"))?;
        let source = member.collections.contains(SOURCE_FILES);
        let required = source
            || member.collections.contains(NATIVE_IDENTITIES)
            || member.collections.contains(CONTRACT_FILES);
        let cap = self.graph_limits.catalog.max_row_bytes;
        if path.ends_with(".json") {
            let parsed = self.parse_packet(raw, self.json_input_max_bytes);
            if required {
                self.value(parsed?.value(), 0)?;
            } else {
                match parsed {
                    Ok(parsed) => self.value(parsed.value(), 0)?,
                    Err(error @ Error::Budget(_)) if self.cold_raw_parser => return Err(error),
                    Err(_) => (),
                }
            }
            if source {
                self.dependency(&format!(
                    "{}.human-forms.json",
                    path.strip_suffix(".json")
                        .ok_or(Error::Invalid("cold catalog JSON suffix"))?
                ))?;
            }
        } else if path.ends_with(".jsonl") {
            if let Some(selection) = selection.as_ref().filter(|_| !generated_member) {
                let mut scratch = 0;
                for slot in selection.file_slots(path) {
                    scratch = scratch.max(
                        slot.verification_state_upper_bound()
                            .map_err(|error| candidate_input_refusal(error))?,
                    );
                }
                if let Some(limit) = self.workspace_limit {
                    self.plan_bytes
                        .checked_add(self.live_raw_bytes)
                        .and_then(|n| n.checked_add(scratch))
                        .filter(|n| *n <= limit)
                        .ok_or(Error::Budget("selected catalog row verification workspace"))?;
                }
                let verified = selection
                    .verify_file(path, raw, self.graph_limits.deadline, self.cancelled)
                    .map_err(|error| candidate_input_refusal(error))?;
                let mut cursor = verified.row_cursor();
                while let Some(row) =
                    cursor.next_checked(self.graph_limits.deadline, self.cancelled)
                {
                    let (_, row, _) = row.map_err(|error| candidate_input_refusal(error))?;
                    let parsed = self.parse_packet(row, cap)?;
                    self.value(parsed.value(), 0)?;
                }
                return Ok(());
            }
            // Original bytes stay intact. This pass discovers dependencies;
            // the owner producer verifies physical line/slot semantics later.
            let text =
                std::str::from_utf8(raw).map_err(|_| Error::Invalid("cold catalog JSONL UTF-8"))?;
            for line in text
                .split(['\r', '\n'])
                .filter(|line| !line.trim().is_empty())
            {
                let parsed = self.parse_packet(line.as_bytes(), cap);
                if let Ok(parsed) = parsed {
                    self.value(parsed.value(), 0)?;
                    if source
                        && matches!(
                            path.rsplit('/').next(),
                            Some("source-claims.jsonl" | "historical-claims.jsonl")
                        )
                    {
                        if let Some(id) = parsed.value().get("claim_id").and_then(Value::as_str) {
                            self.dependency(&format!(
                                "{}.{}.human-forms.json",
                                path.strip_suffix(".jsonl")
                                    .ok_or(Error::Invalid("cold catalog JSONL suffix"))?,
                                Digest256::of_bytes(id.as_bytes()).to_hex()
                            ))?;
                        }
                    }
                } else {
                    match parsed {
                        Err(error @ Error::Budget(_)) if self.cold_raw_parser => return Err(error),
                        Err(_) if required => {
                            return Err(Error::Invalid("cold catalog source JSONL row"));
                        }
                        _ => (),
                    }
                }
            }
        }
        Ok(())
    }
}
fn frame(hash: &mut Digest256Hasher, path: &str, sha: &Digest256) {
    hash.update(&(path.len() as u64).to_be_bytes());
    hash.update(path.as_bytes());
    hash.update(sha.as_bytes());
}
fn collection_receipts(
    members: &BTreeMap<String, Member>,
    l: BibliographicLimits,
) -> Result<Vec<InputCollectionReceipt>> {
    let mut result = Vec::new();
    for collection in COLLECTIONS {
        let mut hash = Digest256Hasher::new();
        let mut count = 0u64;
        let mut annotation_count = 0u64;
        for (path, member) in members {
            if member.collections.contains(collection) {
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= l.catalog.max_files)
                    .ok_or(Error::Budget("cold catalog collection files"))?;
                if collection == NATIVE_IDENTITIES
                    && path
                        .rsplit('/')
                        .next()
                        .is_some_and(|name| name.starts_with("semantic-annotation"))
                {
                    annotation_count = annotation_count
                        .checked_add(1)
                        .filter(|n| *n <= 1024)
                        .ok_or(Error::Budget(
                            "cold catalog native identity inventory packets",
                        ))?;
                }
                frame(&mut hash, path, &member.sha);
            }
        }
        let (role, profile) = catalog::input_role(collection)?;
        result.push(InputCollectionReceipt {
            source_graph: CATALOG_SOURCE.into(),
            collection: collection.into(),
            input_role: role.into(),
            adapter_profile: profile.into(),
            expected_count: count,
            expected_root_sha256: hash.finalize().to_hex(),
        });
    }
    Ok(result)
}
/// A private selected-member spool, never an arbitrary caller-supplied iterator.
/// The host reservation and source reader must outlive its transfer/final fence.
pub struct ColdSourceCatalogInputSpool<'a> {
    entries: PlanningEntries<'a>,
    receipt: ColdExactInputReceipt,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    raw_input_max_bytes: usize,
    work_bytes: u64,
    consumed: bool,
}
impl ColdSourceCatalogInputSpool<'_> {
    pub fn input_receipt(&self) -> ColdExactInputReceipt {
        self.receipt.clone()
    }
    pub fn selected_member_count(&self) -> u64 {
        self.entries.len()
    }
    pub fn observed_work_bytes(&self) -> u64 {
        self.work_bytes
    }
}
/// Actual catalog→bibliographic preparation over the same selected spool and
/// authenticated current/history reader. The sink renderer remains unchanged.
pub fn render_streamed_source_bibliographic_spool(
    plan: &mut ColdSourceCatalogInputSpool<'_>,
    cut: &tos_source_store::StreamedCorpusCutReaderV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    mut l: BibliographicLimits,
    observer: &mut impl catalog::SourceCatalogProfileObserver,
    observed: &mut SourceCatalogRenderWorkV1,
    read_ledger: &std::cell::RefCell<
        crate::source_bibliographic_versions::StreamedBibliographicReadLedger,
    >,
    max_workspace_bytes: usize,
) -> Result<SourceBibliographicCandidate<ColdAuthoredBinding>> {
    if let PlanningEntries::Disk(d) = &plan.entries {
        l.deadline = l.deadline.min(d.deadline);
    }
    let result = (|| {
        let catalog = prepare_streamed_cold_source_catalog_spool(
            plan, cut, target, validator, l, observer, observed,
        )?;
        let source = crate::source_bibliographic_versions::StreamedBibliographicSourceCut {
            cut,
            expected_revision: plan.revision,
            expected_membership: plan.membership,
            stage_source_cut: plan.receipt.binding.source_cut(),
            read_ledger,
            max_workspace_bytes,
        };
        let bibliographic = graph::prepare_streamed_cold_bibliographic_graph_from_cut(
            target, &catalog, validator, forms, l, &source,
        )?;
        streamed_selection(
            cut,
            plan.revision,
            plan.membership,
            l.deadline,
            validator.cancelled,
        )?;
        if let PlanningEntries::Disk(d) = &plan.entries {
            d.guard(ColdSourceCatalogSpoolPhase::Finish)?;
        }
        check(l.deadline, validator.cancelled)?;
        Ok(SourceBibliographicCandidate {
            catalog,
            bibliographic,
        })
    })();
    if result.is_err() {
        target.poison();
    }
    result
}
pub fn prepare_streamed_cold_source_catalog_spool(
    plan: &mut ColdSourceCatalogInputSpool<'_>,
    cut: &tos_source_store::StreamedCorpusCutReaderV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    mut l: BibliographicLimits,
    observer: &mut impl catalog::SourceCatalogProfileObserver,
    observed: &mut SourceCatalogRenderWorkV1,
) -> Result<SourceCatalogReceipt<ColdAuthoredBinding>> {
    if plan.consumed {
        target.poison();
        return Err(Error::Invalid("streamed catalog spool already consumed"));
    }
    plan.consumed = true;
    if let PlanningEntries::Disk(d) = &plan.entries {
        if !std::ptr::eq(d.cancelled, validator.cancelled) {
            target.poison();
            return Err(Error::Invalid(
                "streamed catalog cancellation owner differs",
            ));
        }
        l.deadline = l.deadline.min(d.deadline);
    }
    let result = (|| {
        streamed_selection(
            cut,
            plan.revision,
            plan.membership,
            l.deadline,
            validator.cancelled,
        )?;
        validator.verify_schema_binding(plan.revision)?;
        if plan.receipt.binding.value() != target.cold_receipt()?.binding.value() {
            return Err(Error::Invalid("streamed catalog target binding"));
        }
        let actual = spool_receipts(&plan.entries, l, validator.cancelled)?;
        let expected = target.input_collections();
        if actual.len() != expected.len() {
            return Err(Error::Invalid("streamed catalog collection closure"));
        }
        for a in &actual {
            if !expected.iter().any(|e| {
                e.source_graph == a.source_graph
                    && e.collection == a.collection
                    && e.input_role == a.input_role
                    && e.adapter_profile == a.adapter_profile
                    && e.expected_count == a.expected_count
                    && e.expected_root_sha256 == a.expected_root_sha256
            }) {
                return Err(Error::Invalid("streamed catalog independent target roots"));
            }
        }
        let mut after = None;

        while let Some((path, member)) =
            next_spool_member(&plan.entries, after.as_deref(), l, validator.cancelled)?
        {
            check(l.deadline, validator.cancelled)?;
            if let PlanningEntries::Disk(d) = &plan.entries {
                d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
                plan.work_bytes = plan
                    .work_bytes
                    .checked_add(member.size)
                    .filter(|n| *n <= d.limits.max_work_bytes)
                    .ok_or(Error::Budget("streamed catalog cumulative work"))?;
            }
            let relative = RelativePath::parse(&path)
                .map_err(|_| Error::Invalid("streamed catalog transfer path"))?;
            let current = cut
                .member(plan.revision, &relative)
                .map_err(|e| Error::Source(e.to_string()))?
                .ok_or(Error::Invalid("streamed catalog transfer member absent"))?;
            if current.size_bytes != member.size || current.sha256 != member.sha {
                return Err(Error::Invalid("streamed catalog transfer metadata changed"));
            }
            if let PlanningEntries::Disk(d) = &plan.entries {
                let buffer_upper = usize::try_from(member.size)
                    .ok()
                    .and_then(|n| n.max(8).checked_mul(4))
                    .and_then(|n| n.checked_add(128 * 1024))
                    .and_then(|n| {
                        n.checked_add((d.limits.sqlite_cache_kib as usize) * 1024 + 64 * 1024)
                    })
                    .filter(|n| *n <= d.limits.max_workspace_bytes)
                    .ok_or(Error::Budget("streamed catalog transfer workspace"))?;
                let _ = buffer_upper;
            }
            let raw = cut
                .read_member(
                    plan.revision,
                    &relative,
                    plan.raw_input_max_bytes.min(l.catalog.max_file_bytes) as u64,
                    l.deadline,
                    validator.cancelled,
                )
                .map_err(|e| Error::Source(e.to_string()))?
                .raw;
            charge_render_work(&mut observed.source_members_returned, 1)?;
            charge_render_work(
                &mut observed.source_payload_bytes_returned,
                raw.len() as u64,
            )?;
            if raw.len() as u64 != member.size || Digest256::of_bytes(&raw) != member.sha {
                return Err(Error::Invalid("streamed catalog transfer exact bytes"));
            }
            for collection in &member.collections {
                check(l.deadline, validator.cancelled)?;
                target.ingest_input(InputRow {
                    source_graph: CATALOG_SOURCE,
                    collection,
                    id: &path,
                    payload: &raw,
                })?;
                charge_render_work(&mut observed.input_rows_staged, 1)?;
                charge_render_work(&mut observed.input_payload_bytes_staged, raw.len() as u64)?;
            }
            after = Some(path);
        }
        let receipt =
            catalog::prepare_catalog_receipt_observed(target, validator, l.catalog, observer)?;
        streamed_selection(
            cut,
            plan.revision,
            plan.membership,
            l.deadline,
            validator.cancelled,
        )?;
        if let PlanningEntries::Disk(d) = &plan.entries {
            d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
        }
        check(l.deadline, validator.cancelled)?;
        Ok(receipt)
    })();
    if result.is_err() {
        target.poison();
    }
    result
}
fn streamed_selection(
    cut: &tos_source_store::StreamedCorpusCutReaderV1,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    check(deadline, cancelled)?;
    let actual = cut
        .revision(revision)
        .map_err(|e| Error::Source(e.to_string()))?
        .ok_or(Error::Invalid("streamed catalog revision absent"))?;
    if cut.current_revision() != revision || actual.membership != membership {
        return Err(Error::Invalid(
            "streamed catalog independently selected cut",
        ));
    }
    Ok(())
}
fn next_spool_member(
    entries: &PlanningEntries<'_>,
    after: Option<&str>,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<Option<(String, Member)>> {
    check(l.deadline, cancelled)?;
    let next = entries.next(after)?;
    check(l.deadline, cancelled)?;
    Ok(next)
}
fn spool_receipts(
    entries: &PlanningEntries<'_>,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<Vec<InputCollectionReceipt>> {
    let mut receipts = Vec::with_capacity(COLLECTIONS.len());
    for collection in COLLECTIONS {
        let mut hash = Digest256Hasher::new();
        let mut count = 0u64;
        let mut annotation_count = 0u64;
        let mut after = None;
        while let Some((path, member)) = next_spool_member(entries, after.as_deref(), l, cancelled)?
        {
            check(l.deadline, cancelled)?;
            if member.collections.contains(collection) {
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= l.catalog.max_files)
                    .ok_or(Error::Budget("cold catalog collection files"))?;
                if collection == NATIVE_IDENTITIES
                    && path
                        .rsplit('/')
                        .next()
                        .is_some_and(|name| name.starts_with("semantic-annotation"))
                {
                    annotation_count = annotation_count
                        .checked_add(1)
                        .filter(|n| *n <= 1024)
                        .ok_or(Error::Budget(
                            "cold catalog native identity inventory packets",
                        ))?;
                }
                frame(&mut hash, &path, &member.sha);
            }
            after = Some(path);
        }
        let (role, profile) = catalog::input_role(collection)?;
        receipts.push(InputCollectionReceipt {
            source_graph: CATALOG_SOURCE.into(),
            collection: collection.into(),
            input_role: role.into(),
            adapter_profile: profile.into(),
            expected_count: count,
            expected_root_sha256: hash.finalize().to_hex(),
        });
    }
    Ok(receipts)
}
/// Plan the same source/dependency law into a quota-owned disk locator index.
/// No manifest/member Vec or caller-authored receipt grants source membership.
pub fn plan_streamed_cold_source_catalog_inputs<'a>(
    cut: &tos_source_store::StreamedCorpusCutReaderV1,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    epoch: &tos_source_store::MetadataPublicationEpoch,
    file: std::fs::File,
    isolation: &'a dyn ColdSourceCatalogSpoolIsolation,
    limits: ColdSourceCatalogSpoolLimits,
    l: BibliographicLimits,
    cancelled: &'a AtomicBool,
) -> Result<ColdSourceCatalogInputSpool<'a>> {
    l.validate()?;
    l.catalog.validate()?;
    if limits.max_selected_members == 0
        || limits.max_selected_members == u64::MAX
        || limits.max_locator_bytes == 0
        || limits.max_locator_bytes == u64::MAX
        || limits.max_sqlite_bytes < 4096
        || limits.max_sqlite_bytes == u64::MAX
        || limits.max_work_bytes == 0
        || limits.max_work_bytes == u64::MAX
        || limits.sqlite_cache_kib == 0
        || limits.sqlite_cache_kib > 64 * 1024
        || limits.max_sql_vm_steps == 0
        || limits.max_workspace_bytes == 0
    {
        return Err(Error::Budget("streamed catalog spool limits"));
    }
    streamed_selection(cut, revision, membership, l.deadline, cancelled)?;
    let binding = ColdAuthoredBinding::from_streamed_cut(cut, revision, membership, epoch)?;
    let baseline = (limits.sqlite_cache_kib as usize)
        .checked_mul(1024)
        .and_then(|n| n.checked_add(64 * 1024))
        .filter(|n| *n <= limits.max_workspace_bytes)
        .ok_or(Error::Budget("streamed catalog spool workspace"))?;
    isolation.verify(&file, limits, ColdSourceCatalogSpoolPhase::Create)?;
    let db = tos_source_store::PinnedSqliteConnection::open_private_derived(&file)
        .map_err(|e| Error::Source(e.to_string()))?;
    let sql_limits = crate::Limits {
        max_rows: limits.max_selected_members,
        max_row_bytes: 4096,
        max_output_bytes: limits.max_sqlite_bytes,
        max_work_bytes: limits.max_work_bytes,
        sqlite_cache_kib: limits.sqlite_cache_kib,
        max_sql_vm_steps: limits.max_sql_vm_steps,
    };
    crate::sqlite_budget::install_progress_until(
        &db,
        sql_limits,
        std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        l.deadline,
    );
    db.pragma_update(None, "cache_size", -(limits.sqlite_cache_kib as i64))?;
    let page: u64 = db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    if page == 0 || page > limits.max_sqlite_bytes {
        return Err(Error::Budget("streamed catalog spool page"));
    }
    db.pragma_update(None, "max_page_count", limits.max_sqlite_bytes / page)?;
    let effective: u64 = db.query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
    if effective > limits.max_sqlite_bytes / page {
        return Err(Error::Invalid("streamed catalog spool page cap"));
    }
    db.execute_batch("CREATE TABLE selected(path TEXT PRIMARY KEY,size BLOB NOT NULL,sha BLOB NOT NULL,flags INTEGER NOT NULL) WITHOUT ROWID; CREATE TABLE pending(path TEXT PRIMARY KEY) WITHOUT ROWID; CREATE TABLE scanned(path TEXT PRIMARY KEY) WITHOUT ROWID;")?;
    isolation.verify(&file, limits, ColdSourceCatalogSpoolPhase::Grow)?;
    let mut plan = Planning {
        cut: PlanningCut::Streamed(cut),
        revision: Some(revision),
        limits: SourceCatalogInputLimits {
            max_manifest_members: membership.count,
            max_selected_members: 4096,
            max_plan_bytes: 64 * 1024 * 1024,
            max_work_bytes: limits.max_work_bytes,
        },
        graph_limits: l,
        cancelled,
        entries: PlanningEntries::Disk(DiskEntries {
            db: PlanningDatabase::Owned(db),
            file: Some(file),
            isolation: Some(isolation),
            limits,
            rows: 0,
            cleanup_complete: std::cell::Cell::new(false),
            logical_bytes: 0,
            deadline: l.deadline,
            cancelled,
        }),
        plan_bytes: baseline,
        work_bytes: 0,
        raw_input_max_bytes: l.catalog.max_file_bytes.min(64 * 1024 * 1024),
        json_input_max_bytes: l.catalog.max_file_bytes.min(64 * 1024 * 1024),
        cold_raw_parser: true,
        workspace_limit: Some(limits.max_workspace_bytes),
        live_raw_bytes: 0,
        live_parser_upper: 0,
    };
    for registry in [ENTITY, RELATION] {
        if !plan.select(registry, CONTRACT_FILES)? {
            return Err(Error::Invalid("cold catalog source registry absent"));
        }
    }
    let raw = plan.raw(ENTITY)?;
    let entities = plan.parse_packet(&raw, l.catalog.max_row_bytes)?;
    plan.live_parser_upper = plan
        .live_parser_upper
        .checked_mul(2)
        .ok_or(Error::Budget("cold catalog registry workspace"))?;
    plan.check_workspace()?;
    let basenames = catalog::source_basenames(entities.value())?;
    let mut after = None;
    let mut membership_hash = Digest256Hasher::new();
    membership_hash.update(b"tos-val-full-membership-v1\0");
    let mut member_count = 0u64;
    loop {
        check(l.deadline, cancelled)?;
        let Some(member) = cut
            .member_after(revision, after.as_ref())
            .map_err(|e| Error::Source(e.to_string()))?
        else {
            break;
        };
        check(l.deadline, cancelled)?;
        let reference = member.path.as_str();
        membership_hash.update(&(reference.len() as u64).to_be_bytes());
        membership_hash.update(reference.as_bytes());
        membership_hash.update(&member.size_bytes.to_be_bytes());
        membership_hash.update(member.sha256.as_bytes());
        member_count = member_count
            .checked_add(1)
            .filter(|n| *n <= membership.count)
            .ok_or(Error::Budget("streamed catalog full membership"))?;
        plan.initial(member.path.as_str(), &basenames)?;
        after = Some(member.path);
    }
    if (SourceMembershipV1 {
        count: member_count,
        digest: membership_hash.finalize(),
    }) != membership
    {
        return Err(Error::Invalid("streamed catalog full membership EOF"));
    }
    drop(basenames);
    drop(entities);
    drop(raw);
    plan.live_raw_bytes = 0;
    plan.live_parser_upper = 0;
    while let Some(path) = plan.entries.pop()? {
        check(l.deadline, cancelled)?;
        let raw = plan.raw(&path)?;
        plan.packet(&path, &raw)?;
        drop(raw);
        plan.live_raw_bytes = 0;
        plan.live_parser_upper = 0;
    }
    let collections = spool_receipts(&plan.entries, l, cancelled)?;
    if let PlanningEntries::Disk(d) = &plan.entries {
        d.guard(ColdSourceCatalogSpoolPhase::Grow)?;
    }
    streamed_selection(cut, revision, membership, l.deadline, cancelled)?;
    Ok(ColdSourceCatalogInputSpool {
        entries: plan.entries,
        receipt: ColdExactInputReceipt {
            binding,
            collections,
        },
        revision,
        membership,
        raw_input_max_bytes: plan.raw_input_max_bytes,
        work_bytes: plan.work_bytes,
        consumed: false,
    })
}
/// Discover maintained current source families, complete native identities and
/// their actual public metadata dependency closure. No callback or worker can
/// turn this observation plan into catalog/schema/semantic acceptance.
pub fn plan_source_catalog_inputs(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    binding: &SourceBinding,
    limits: SourceCatalogInputLimits,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<SourceCatalogInputPlan> {
    plan_inputs(
        cut,
        expected_revision,
        expected_membership,
        binding,
        limits,
        l,
        cancelled,
        l.catalog.max_file_bytes,
        l.catalog.max_row_bytes,
        false,
        None,
    )
}
pub fn plan_cold_source_catalog_inputs(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    binding: &ColdAuthoredBinding,
    limits: SourceCatalogInputLimits,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<ColdSourceCatalogInputPlan> {
    if binding.revision() != expected_revision || binding.membership() != expected_membership {
        return Err(Error::Invalid(
            "cold catalog binding differs from selected manifest",
        ));
    }
    plan_inputs(
        cut,
        expected_revision,
        expected_membership,
        binding,
        limits,
        l,
        cancelled,
        l.catalog.max_file_bytes,
        l.catalog.max_file_bytes,
        true,
        None,
    )
}
/// Cold dependency discovery with a separate complete temporary workspace.
/// Locator format capacity remains governed by SourceCatalogInputLimits.
pub fn plan_cold_source_catalog_inputs_with_workspace(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    binding: &ColdAuthoredBinding,
    limits: SourceCatalogInputLimits,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
    max_workspace_bytes: usize,
) -> Result<ColdSourceCatalogInputPlan> {
    if binding.revision() != expected_revision || binding.membership() != expected_membership {
        return Err(Error::Invalid(
            "cold catalog binding differs from selected manifest",
        ));
    }
    if max_workspace_bytes == 0 {
        return Err(Error::Budget("cold catalog temporary workspace"));
    }
    plan_inputs(
        cut,
        expected_revision,
        expected_membership,
        binding,
        limits,
        l,
        cancelled,
        l.catalog.max_file_bytes,
        l.catalog.max_file_bytes,
        true,
        Some(max_workspace_bytes),
    )
}
fn plan_inputs<B: catalog::CatalogInputBinding>(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    binding: &B,
    limits: SourceCatalogInputLimits,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
    raw_input_max_bytes: usize,
    json_input_max_bytes: usize,
    cold_raw_parser: bool,
    workspace_limit: Option<usize>,
) -> Result<SourceCatalogInputPlan<B>> {
    limits.validate(l)?;
    if raw_input_max_bytes == 0 || raw_input_max_bytes > 64 * 1024 * 1024 {
        return Err(Error::Budget("cold catalog raw input ceiling"));
    }
    binding.validate_plan()?;
    selected_cut(
        cut,
        expected_revision,
        expected_membership,
        limits,
        l.deadline,
        cancelled,
    )?;
    plan_inputs_kernel(
        PlanningCut::Resident(cut),
        Some(expected_revision),
        expected_membership,
        binding,
        limits,
        l,
        cancelled,
        raw_input_max_bytes,
        json_input_max_bytes,
        cold_raw_parser,
        workspace_limit,
        0,
    )
}
fn plan_inputs_kernel<B: catalog::CatalogInputBinding>(
    cut: PlanningCut<'_>,
    revision: Option<SourceRevision>,
    expected_membership: SourceMembershipV1,
    binding: &B,
    limits: SourceCatalogInputLimits,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
    raw_input_max_bytes: usize,
    json_input_max_bytes: usize,
    cold_raw_parser: bool,
    workspace_limit: Option<usize>,
    initial_work: u64,
) -> Result<SourceCatalogInputPlan<B>> {
    let mut plan = Planning {
        cut,
        revision,
        limits,
        graph_limits: l,
        cancelled,
        entries: PlanningEntries::Resident {
            members: BTreeMap::new(),
            pending: BTreeSet::new(),
            scanned: BTreeSet::new(),
        },
        plan_bytes: 0,
        work_bytes: initial_work,
        raw_input_max_bytes,
        json_input_max_bytes,
        cold_raw_parser,
        workspace_limit,
        live_raw_bytes: 0,
        live_parser_upper: 0,
    };
    for registry in [ENTITY, RELATION] {
        if !plan.select(registry, CONTRACT_FILES)? {
            return Err(Error::Invalid("cold catalog source registry absent"));
        }
    }
    let raw = plan.raw(ENTITY)?;
    let entities = plan.parse_packet(&raw, l.catalog.max_row_bytes)?;
    // This small registry's derived basename set coexists with its DOM. A
    // second complete DOM/container envelope conservatively covers that set
    // before the existing selector allocates it (no duplicate selector).
    if plan.workspace_limit.is_some() {
        plan.live_parser_upper = plan
            .live_parser_upper
            .checked_mul(2)
            .ok_or(Error::Budget("cold catalog registry workspace"))?;
        plan.check_workspace()?;
    }
    let basenames = catalog::source_basenames(entities.value())?;
    match cut {
        PlanningCut::Resident(cut) => {
            for member in cut.current().members() {
                check(l.deadline, cancelled)?;
                plan.initial(member.path.as_str(), &basenames)?;
            }
        }
        PlanningCut::Candidate(input) => {
            input
                .for_each_current_member_meta(l.deadline, cancelled, &mut |meta| {
                    plan.initial(meta.path, &basenames).map_err(|e| match e {
                        Error::Budget(_) | Error::SqliteVmBudget { .. } => {
                            tos_validation::item_rules::ItemRefusal::Budget
                        }
                        other => tos_validation::item_rules::ItemRefusal::Source(other.to_string()),
                    })
                })
                .map_err(|error| candidate_input_refusal(error))?;
            check(l.deadline, cancelled)?;
        }
        PlanningCut::Streamed(_) => return Err(Error::Invalid("resident recipe storage kind")),
    }
    drop(basenames);
    drop(entities);
    drop(raw);
    plan.live_parser_upper = 0;
    plan.live_raw_bytes = 0;
    while let Some(path) = plan.entries.pop()? {
        check(l.deadline, cancelled)?;
        let raw = plan.raw(&path)?;
        plan.packet(&path, &raw)?;
        drop(raw);
        plan.live_parser_upper = 0;
        plan.live_raw_bytes = 0;
    }
    if plan.workspace_limit.is_some() {
        // Five fixed collection receipts plus the returned plan/binding and
        // transient source-path framing. Reserve before constructing receipts.
        plan.charge(COLLECTIONS.len() * 1024 + std::mem::size_of::<SourceCatalogInputPlan<B>>())?;
    }
    let PlanningEntries::Resident { members, .. } = plan.entries else {
        return Err(Error::Invalid("resident source plan storage"));
    };
    let receipt = PlanInput {
        binding: binding.clone(),
        collections: collection_receipts(&members, l)?,
    };
    Ok(SourceCatalogInputPlan {
        receipt,
        revision,
        membership: expected_membership,
        members,
        limits,
        work_bytes: plan.work_bytes,
        retained_plan_bytes: plan.plan_bytes,
        raw_input_max_bytes,
        workspace_limit,
    })
}
/// Transfer the authenticated planned cut and prepare only catalog parity.
/// The maintained foundation validator selects its optional bibliographic
/// continuation independently; this function never reads generated catalogs.
/// Actual cold-plan transfer observations; excludes protocol/page I/O and renderer-wide CPU.
/// Caller-retained counters survive later failure; failed calls without a completed
/// result are not inferred as successful reads or staged input rows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceCatalogRenderWorkV1 {
    pub source_members_returned: u64,
    pub source_payload_bytes_returned: u64,
    pub input_rows_staged: u64,
    pub input_payload_bytes_staged: u64,
}
fn charge_render_work(field: &mut u64, value: u64) -> Result<()> {
    *field = field
        .checked_add(value)
        .ok_or(Error::Budget("cold render work overflow"))?;
    Ok(())
}

pub fn prepare_source_catalog_plan(
    plan: &SourceCatalogInputPlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
) -> Result<SourceCatalogReceipt> {
    prepare_catalog_plan(
        plan,
        cut,
        expected_revision,
        expected_membership,
        target,
        validator,
        l,
        &mut catalog::IgnoreCatalogProfiles,
        &mut SourceCatalogRenderWorkV1::default(),
    )
}
pub fn prepare_cold_source_catalog_plan(
    plan: &ColdSourceCatalogInputPlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
) -> Result<catalog::ColdSourceCatalogReceipt> {
    prepare_catalog_plan(
        plan,
        cut,
        expected_revision,
        expected_membership,
        target,
        validator,
        l,
        &mut catalog::IgnoreCatalogProfiles,
        &mut SourceCatalogRenderWorkV1::default(),
    )
}
pub fn prepare_cold_source_catalog_plan_observed(
    plan: &ColdSourceCatalogInputPlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
    observer: &mut impl catalog::SourceCatalogProfileObserver,
) -> Result<catalog::ColdSourceCatalogReceipt> {
    prepare_catalog_plan(
        plan,
        cut,
        expected_revision,
        expected_membership,
        target,
        validator,
        l,
        observer,
        &mut SourceCatalogRenderWorkV1::default(),
    )
}
fn prepare_catalog_plan<B: catalog::CatalogInputBinding>(
    plan: &SourceCatalogInputPlan<B>,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
    observer: &mut impl catalog::SourceCatalogProfileObserver,
    observed: &mut SourceCatalogRenderWorkV1,
) -> Result<SourceCatalogReceipt<B>> {
    let result = (|| {
        plan.limits.validate(l)?;
        drop(validator.schemas(expected_revision)?);
        selected_cut(
            cut,
            expected_revision,
            expected_membership,
            plan.limits,
            l.deadline,
            validator.cancelled,
        )?;
        prepare_catalog_plan_kernel(
            plan,
            PlanningCut::Resident(cut),
            Some(expected_revision),
            expected_membership,
            target,
            validator,
            l,
            observer,
            observed,
            0,
            &mut 0,
        )
    })();
    if result.is_err() {
        target.poison();
    }
    result
}
fn prepare_catalog_plan_kernel<B: catalog::CatalogInputBinding>(
    plan: &SourceCatalogInputPlan<B>,
    cut: PlanningCut<'_>,
    revision: Option<SourceRevision>,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
    observer: &mut impl catalog::SourceCatalogProfileObserver,
    observed: &mut SourceCatalogRenderWorkV1,
    initial_work: u64,
    work_after: &mut u64,
) -> Result<SourceCatalogReceipt<B>> {
    let result = (|| {
        plan.limits.validate(l)?;
        if plan.revision != revision
            || plan.membership != expected_membership
            || !plan.receipt.binding.matches_source(&B::selected(target)?)
        {
            return Err(Error::Invalid("cold catalog plan source binding"));
        }
        let actual = collection_receipts(&plan.members, l)?;
        let target_collections = target.input_collections();
        if target_collections.len() != actual.len() {
            return Err(Error::Invalid("cold catalog target collection closure"));
        }
        for expected in &actual {
            let entry = target_collections
                .iter()
                .find(|entry| {
                    entry.source_graph == expected.source_graph
                        && entry.collection == expected.collection
                })
                .ok_or(Error::Invalid("cold catalog target collection missing"))?;
            if entry.input_role != expected.input_role
                || entry.adapter_profile != expected.adapter_profile
                || entry.expected_count != expected.expected_count
                || entry.expected_root_sha256 != expected.expected_root_sha256
            {
                return Err(Error::Invalid("cold catalog independent target roots"));
            }
        }
        *work_after = initial_work;
        let work = &mut *work_after;
        for (path, member) in &plan.members {
            check(l.deadline, validator.cancelled)?;
            let relative = RelativePath::parse(path)
                .map_err(|_| Error::Invalid("cold catalog transfer path"))?;
            if let Some(limit) = plan.workspace_limit {
                let retained = plan.retained_plan_bytes;
                usize::try_from(member.size)
                    .ok()
                    .and_then(|n| n.max(8).checked_mul(4))
                    .and_then(|n| n.checked_add(128 * 1024))
                    .and_then(|n| n.checked_add(retained))
                    .filter(|n| *n <= limit)
                    .ok_or(Error::Budget("candidate transfer workspace"))?;
            }
            let (metadata_size, metadata_sha) = cut
                .member(
                    revision,
                    &relative,
                    plan.raw_input_max_bytes.min(l.catalog.max_file_bytes),
                    l.deadline,
                    validator.cancelled,
                    work,
                    plan.limits.max_work_bytes,
                )?
                .ok_or(Error::Invalid("cold catalog planned member absent"))?;
            if metadata_sha != member.sha || metadata_size != member.size {
                return Err(Error::Invalid("cold catalog planned member changed"));
            }
            *work = work
                .checked_add(member.size)
                .filter(|n| *n <= plan.limits.max_work_bytes)
                .ok_or(Error::Budget("cold catalog transfer work"))?;
            let raw = cut.read(
                revision,
                &relative,
                plan.raw_input_max_bytes.min(l.catalog.max_file_bytes) as u64,
                l.deadline,
                validator.cancelled,
            )?;
            charge_render_work(&mut observed.source_members_returned, 1)?;
            charge_render_work(
                &mut observed.source_payload_bytes_returned,
                raw.len() as u64,
            )?;
            if raw.len() as u64 != member.size || Digest256::of_bytes(&raw) != member.sha {
                return Err(Error::Invalid("cold catalog transfer exact bytes"));
            }
            let (max_rows, max_bytes) = target.input_batch_limits();
            let rows_per_chunk = if raw.is_empty() {
                max_rows
            } else {
                max_rows.min((max_bytes / raw.len() as u64) as usize)
            };
            if rows_per_chunk == 0 {
                for collection in &member.collections {
                    check(l.deadline, validator.cancelled)?;
                    target.ingest_input(InputRow {
                        source_graph: CATALOG_SOURCE,
                        collection,
                        id: path,
                        payload: &raw,
                    })?;
                    charge_render_work(&mut observed.input_rows_staged, 1)?;
                    charge_render_work(&mut observed.input_payload_bytes_staged, raw.len() as u64)?;
                }
            } else {
                let mut collections = member.collections.iter();
                loop {
                    let rows = collections
                        .by_ref()
                        .take(rows_per_chunk)
                        .map(|collection| {
                            check(l.deadline, validator.cancelled)?;
                            Ok(InputRow {
                                source_graph: CATALOG_SOURCE,
                                collection,
                                id: path,
                                payload: &raw,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    if rows.is_empty() {
                        break;
                    }
                    target.ingest_input_batch(&rows)?;
                    charge_render_work(&mut observed.input_rows_staged, rows.len() as u64)?;
                    charge_render_work(
                        &mut observed.input_payload_bytes_staged,
                        (raw.len() as u64)
                            .checked_mul(rows.len() as u64)
                            .ok_or(Error::Budget("cold render work overflow"))?,
                    )?;
                    check(l.deadline, validator.cancelled)?;
                }
            }
        }
        let record_selection = cut.record_selection();
        let generated_selection = cut.generated_selection();
        let receipt = catalog::prepare_catalog_receipt_observed_with_selections(
            target,
            validator,
            l.catalog,
            observer,
            record_selection.as_deref(),
            generated_selection.as_deref(),
            plan.temporary_workspace_limit(),
        )?;

        Ok(receipt)
    })();
    if result.is_err() {
        target.poison();
    }
    result
}

/// Transfer actual original bytes into an independently created exact stage,
/// then execute the existing catalog and selected-cut bibliographic producers.
/// The caller supplies the actual native forms adapter from its owning crate;
/// compiler has no dependency on command execution or publication authority.
pub fn render_source_bibliographic_plan(
    plan: &SourceCatalogInputPlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    max_version_read_files: usize,
    max_version_read_bytes: usize,
) -> Result<SourceBibliographicCandidate> {
    render_source_bibliographic_plan_with_work(
        plan,
        cut,
        expected_revision,
        expected_membership,
        target,
        validator,
        forms,
        l,
        max_version_read_files,
        max_version_read_bytes,
        &mut SourceCatalogRenderWorkV1::default(),
    )
}

pub fn render_source_bibliographic_plan_with_work(
    plan: &SourceCatalogInputPlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    max_version_read_files: usize,
    max_version_read_bytes: usize,
    observed: &mut SourceCatalogRenderWorkV1,
) -> Result<SourceBibliographicCandidate> {
    let result = (|| {
        plan.limits.validate(l)?;
        if max_version_read_files == 0
            || max_version_read_files > 4096
            || max_version_read_bytes == 0
            || max_version_read_bytes > 64 * 1024 * 1024
        {
            return Err(Error::Budget("cold catalog selected version read limits"));
        }
        let receipt = prepare_catalog_plan(
            plan,
            cut,
            expected_revision,
            expected_membership,
            target,
            validator,
            l,
            &mut catalog::IgnoreCatalogProfiles,
            observed,
        )?;
        let source = BibliographicSourceCut {
            cut,
            expected_revision,
            expected_membership,
            stage_source_cut: &plan.receipt.binding.source_cut,
            max_read_files: max_version_read_files,
            max_read_bytes: max_version_read_bytes,
        };
        let bibliographic = graph::prepare_bibliographic_graph_from_cut(
            target, &receipt, validator, forms, l, &source,
        )?;
        Ok(SourceBibliographicCandidate {
            catalog: receipt,
            bibliographic,
        })
    })();
    if result.is_err() {
        target.poison();
    }
    result
}

fn verify_candidate_input_eof<I: Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    identity: &I,
    coverage: &SourceCutInputCoverage,
    deadline: Instant,
    cancelled: &AtomicBool,
    max_members: u64,
    max_file: usize,
    work: &mut u64,
    max_work: u64,
) -> Result<()> {
    check(deadline, cancelled)?;
    if input.input_identity() != identity {
        return Err(Error::Invalid("candidate EOF identity"));
    }
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-val-full-membership-v1\0");
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut previous = String::new();
    let observed = input
        .for_each_current_member(deadline, cancelled, &mut |meta, raw| {
            if meta.path.len() > 4096
                || (!previous.is_empty() && meta.path <= previous.as_str())
                || meta.size_bytes != raw.len() as u64
                || raw.len() > max_file
            {
                return Err(tos_validation::item_rules::ItemRefusal::Source(
                    "candidate full member stream".into(),
                ));
            }
            RelativePath::parse(meta.path).map_err(|_| {
                tos_validation::item_rules::ItemRefusal::Source("candidate EOF path".into())
            })?;
            count = count
                .checked_add(1)
                .filter(|n| *n <= max_members)
                .ok_or(tos_validation::item_rules::ItemRefusal::Budget)?;
            bytes = bytes
                .checked_add(meta.size_bytes)
                .ok_or(tos_validation::item_rules::ItemRefusal::Budget)?;
            *work = work
                .checked_add(meta.size_bytes)
                .filter(|n| *n <= max_work)
                .ok_or(tos_validation::item_rules::ItemRefusal::Budget)?;
            hash.update(&(meta.path.len() as u64).to_be_bytes());
            hash.update(meta.path.as_bytes());
            hash.update(&meta.size_bytes.to_be_bytes());
            hash.update(Digest256::of_bytes(raw).as_bytes());
            previous.clear();
            previous.push_str(meta.path);
            Ok(())
        })
        .map_err(|error| candidate_input_refusal(error))?;
    if &observed != coverage
        || count != coverage.member_count()
        || bytes != coverage.source_bytes_read()
        || (SourceMembershipV1 {
            count,
            digest: hash.finalize(),
        }) != coverage.membership()
    {
        return Err(Error::Invalid("candidate complete input coverage"));
    }
    input
        .verify_current_fence(&observed, deadline, cancelled)
        .map_err(|error| candidate_input_refusal(error))?;
    check(deadline, cancelled)?;
    if input.input_identity() != identity {
        return Err(Error::Invalid("candidate EOF final identity"));
    }
    Ok(())
}
/// Unpublished input recipe. It carries opaque source identity and verified EOF,
/// never a source revision, metadata publication epoch or cold-publication receipt.
enum CandidateCatalogPlanning<'a, I: Copy + Eq + 'static> {
    Resident(SourceCatalogInputPlan<CandidateValidationBinding<I>>),
    Disk {
        entries: PlanningEntries<'a>,
        recipe: PlanInput<CandidateValidationBinding<I>>,
        limits: SourceCatalogInputLimits,
        raw_input_max_bytes: usize,
        workspace: usize,
    },
}
impl<I: Copy + Eq + 'static> CandidateCatalogPlanning<'_, I> {
    fn recipe(&self) -> &PlanInput<CandidateValidationBinding<I>> {
        match self {
            Self::Resident(plan) => &plan.receipt,
            Self::Disk { recipe, .. } => recipe,
        }
    }
    fn limits(&self) -> SourceCatalogInputLimits {
        match self {
            Self::Resident(plan) => plan.limits,
            Self::Disk { limits, .. } => *limits,
        }
    }
}
pub struct CandidateSourceCatalogInputPlan<'a, I: Copy + Eq + 'static> {
    plan: CandidateCatalogPlanning<'a, I>,
    consumed: bool,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    work_bytes: u64,
}
impl<I: Copy + Eq + 'static> CandidateSourceCatalogInputPlan<'_, I> {
    /// Bound the fixed five-collection receipt before a planner constructs it.
    /// The bound is derived from the same owned constants and role/profile law.
    pub fn receipt_state_upper_bound(identity_heap_bytes: usize) -> Result<usize> {
        let mut bytes = std::mem::size_of::<CandidateExactInputReceipt<I>>()
            .checked_add(identity_heap_bytes)
            .and_then(|n| n.checked_add(128))
            .ok_or(Error::Budget("candidate receipt state"))?;
        for collection in COLLECTIONS {
            let (role, profile) = catalog::input_role(collection)?;
            bytes = std::mem::size_of::<InputCollectionReceipt>()
                .checked_mul(2)
                .and_then(|n| bytes.checked_add(n))
                .ok_or(Error::Budget("candidate receipt state"))?;
            for length in [
                CATALOG_SOURCE.len(),
                collection.len(),
                role.len(),
                profile.len(),
                64,
            ] {
                bytes = length
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(128))
                    .and_then(|n| bytes.checked_add(n))
                    .ok_or(Error::Budget("candidate receipt state"))?;
            }
        }
        Ok(bytes)
    }
    /// Reserve a clone BEFORE allocating its receipt/vector/string storage.
    /// Inline identity/coverage bytes are included; a generic identity owner
    /// supplies its own additional deep-clone heap bound (zero for fixed Copy
    /// identities). This observation does not clone or admit any source bytes.
    pub fn input_receipt_clone_state_bytes(&self, identity_heap_bytes: usize) -> Result<usize> {
        let mut bytes = std::mem::size_of::<CandidateExactInputReceipt<I>>()
            .checked_add(identity_heap_bytes)
            .and_then(|n| n.checked_add(128))
            .ok_or(Error::Budget("candidate receipt clone state"))?;
        for collection in &self.plan.recipe().collections {
            bytes = std::mem::size_of::<InputCollectionReceipt>()
                .checked_mul(2)
                .and_then(|n| bytes.checked_add(n))
                .ok_or(Error::Budget("candidate receipt clone state"))?;
            for value in [
                &collection.source_graph,
                &collection.collection,
                &collection.input_role,
                &collection.adapter_profile,
                &collection.expected_root_sha256,
            ] {
                // Conservative allocator/string framing, beyond logical bytes.
                bytes = value
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(128))
                    .and_then(|n| bytes.checked_add(n))
                    .ok_or(Error::Budget("candidate receipt clone state"))?;
            }
        }
        Ok(bytes)
    }
    pub fn input_receipt(&self) -> CandidateExactInputReceipt<I> {
        CandidateExactInputReceipt {
            binding: self.plan.recipe().binding.clone(),
            collections: self.plan.recipe().collections.clone(),
        }
    }
    pub fn observed_work_bytes(&self) -> u64 {
        self.work_bytes
    }
}
pub fn plan_candidate_source_catalog_inputs_with_workspace<'a, I: Copy + Eq + 'static>(
    input: &dyn SourceCutInputWithIdentity<I>,
    expected_identity: &I,
    coverage: &SourceCutInputCoverage,
    limits: SourceCatalogInputLimits,
    l: BibliographicLimits,
    cancelled: &'a AtomicBool,
    max_workspace_bytes: usize,
) -> Result<CandidateSourceCatalogInputPlan<'a, I>> {
    limits.validate(l)?;
    // EOF path validation retains one bounded previous key and transient path framing.
    if max_workspace_bytes < 16 * 1024 {
        return Err(Error::Budget("candidate catalog workspace"));
    }
    check(l.deadline, cancelled)?;
    let binding = CandidateValidationBinding::from_verified_input(
        input,
        expected_identity,
        coverage.clone(),
        l.deadline,
        cancelled,
    )?;
    let mut initial_work = 0;
    verify_candidate_input_eof(
        input,
        expected_identity,
        coverage,
        l.deadline,
        cancelled,
        limits.max_manifest_members,
        l.catalog.max_file_bytes,
        &mut initial_work,
        limits.max_work_bytes,
    )?;
    let plan = plan_inputs_kernel(
        PlanningCut::Candidate(input.source_input()),
        None,
        coverage.membership(),
        &binding,
        limits,
        l,
        cancelled,
        l.catalog.max_file_bytes.min(64 * 1024 * 1024),
        l.catalog.max_file_bytes.min(64 * 1024 * 1024),
        true,
        Some(max_workspace_bytes),
        initial_work,
    )?;
    input
        .verify_current_fence(coverage, l.deadline, cancelled)
        .map_err(|error| candidate_input_refusal(error))?;
    check(l.deadline, cancelled)?;
    if input.input_identity() != expected_identity {
        return Err(Error::Invalid("candidate catalog input identity"));
    }
    let work_bytes = plan.work_bytes;
    Ok(CandidateSourceCatalogInputPlan {
        plan: CandidateCatalogPlanning::Resident(plan),
        consumed: false,
        deadline: l.deadline,
        cancelled,
        work_bytes,
    })
}
pub fn prepare_candidate_source_catalog_plan_observed<I: Copy + Eq + 'static>(
    plan: &mut CandidateSourceCatalogInputPlan<'_, I>,
    input: &dyn SourceCutInputWithIdentity<I>,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    mut l: BibliographicLimits,
    observer: &mut impl catalog::SourceCatalogProfileObserver,
    observed: &mut SourceCatalogRenderWorkV1,
) -> Result<catalog::CandidateSourceCatalogReceipt<I>> {
    if plan.consumed {
        return Err(Error::Invalid("candidate catalog plan consumed"));
    }
    plan.consumed = true;
    let result = (|| {
        if !std::ptr::eq(plan.cancelled, validator.cancelled) {
            return Err(Error::Invalid("candidate catalog cancellation identity"));
        }
        l.deadline = l.deadline.min(plan.deadline);
        check(l.deadline, validator.cancelled)?;
        let binding = &plan.plan.recipe().binding;
        if input.input_identity() != binding.input_identity() {
            return Err(Error::Invalid("candidate catalog input identity"));
        }
        input
            .verify_current_fence(binding.coverage(), l.deadline, validator.cancelled)
            .map_err(|error| candidate_input_refusal(error))?;
        validator.verify_candidate_schema_binding(binding.input_identity())?;
        let receipt = match &plan.plan {
            CandidateCatalogPlanning::Resident(resident) => prepare_catalog_plan_kernel(
                resident,
                PlanningCut::Candidate(input.source_input()),
                None,
                binding.coverage().membership(),
                target,
                validator,
                l,
                observer,
                observed,
                plan.work_bytes,
                &mut plan.work_bytes,
            )?,
            CandidateCatalogPlanning::Disk {
                entries,
                raw_input_max_bytes,
                workspace,
                ..
            } => {
                let actual = spool_receipts(entries, l, validator.cancelled)?;
                let expected = target.input_collections();
                if actual.len() != expected.len()
                    || actual.iter().any(|a| {
                        !expected.iter().any(|e| {
                            a.source_graph == e.source_graph
                                && a.collection == e.collection
                                && a.input_role == e.input_role
                                && a.adapter_profile == e.adapter_profile
                                && a.expected_count == e.expected_count
                                && a.expected_root_sha256 == e.expected_root_sha256
                        })
                    })
                {
                    return Err(Error::Invalid(
                        "candidate catalog independent disk recipe roots",
                    ));
                }
                let cut = PlanningCut::Candidate(input.source_input());
                let mut after = None;
                while let Some((path, member)) =
                    next_spool_member(entries, after.as_deref(), l, validator.cancelled)?
                {
                    let read_workspace = usize::try_from(member.size)
                        .ok()
                        .and_then(|n| n.checked_mul(4))
                        .and_then(|n| n.checked_add(128 * 1024))
                        .ok_or(Error::Budget("candidate disk transfer state"))?;
                    if read_workspace > *workspace {
                        return Err(Error::Budget("candidate disk transfer state"));
                    }
                    let raw = cut.read(
                        None,
                        &RelativePath::parse(&path)
                            .map_err(|_| Error::Invalid("candidate disk transfer path"))?,
                        *raw_input_max_bytes as u64,
                        l.deadline,
                        validator.cancelled,
                    )?;
                    if raw.len() as u64 != member.size || Digest256::of_bytes(&raw) != member.sha {
                        return Err(Error::Invalid("candidate disk transfer exact bytes"));
                    }
                    plan.work_bytes = plan
                        .work_bytes
                        .checked_add(raw.len() as u64)
                        .filter(|n| *n <= plan.plan.limits().max_work_bytes)
                        .ok_or(Error::Budget("candidate disk cumulative work"))?;
                    charge_render_work(&mut observed.source_members_returned, 1)?;
                    charge_render_work(
                        &mut observed.source_payload_bytes_returned,
                        raw.len() as u64,
                    )?;
                    for collection in &member.collections {
                        target.ingest_input(InputRow {
                            source_graph: CATALOG_SOURCE,
                            collection,
                            id: &path,
                            payload: &raw,
                        })?;
                        charge_render_work(&mut observed.input_rows_staged, 1)?;
                        charge_render_work(
                            &mut observed.input_payload_bytes_staged,
                            raw.len() as u64,
                        )?;
                    }
                    after = Some(path);
                }
                let finite = cut.record_selection();
                let generated = cut.generated_selection();
                catalog::prepare_catalog_receipt_observed_with_selections(
                    target,
                    validator,
                    l.catalog,
                    observer,
                    finite.as_deref(),
                    generated.as_deref(),
                    Some(*workspace),
                )?
            }
        };
        verify_candidate_input_eof(
            input,
            binding.input_identity(),
            binding.coverage(),
            l.deadline,
            validator.cancelled,
            plan.plan.limits().max_manifest_members,
            l.catalog.max_file_bytes,
            &mut plan.work_bytes,
            plan.plan.limits().max_work_bytes,
        )?;
        target.verify_candidate_inputs::<I>()?;
        validator.verify_candidate_schema_binding(binding.input_identity())?;
        check(l.deadline, validator.cancelled)?;
        if let CandidateCatalogPlanning::Disk {
            entries: PlanningEntries::Disk(entries),
            ..
        } = &plan.plan
        {
            entries.finish_candidate_cleanup()?;
        }
        Ok(receipt)
    })();
    if result.is_err() {
        target.poison();
    }
    result
}

/// Storage-backed candidate plan on the SAME native index connection. The
/// caller supplies that existing pinned authority and its configured ceiling;
/// no additional SQLite family, inode, source revision or allowance is issued.
pub fn plan_candidate_source_catalog_inputs_spooled<'a, I: Copy + Eq + 'static>(
    input: &dyn SourceCutInputWithIdentity<I>,
    expected_identity: &I,
    coverage: &SourceCutInputCoverage,
    limits: SourceCatalogInputLimits,
    storage: std::rc::Rc<tos_source_store::PinnedSqliteConnection>,
    disk_limits: ColdSourceCatalogSpoolLimits,
    l: BibliographicLimits,
    cancelled: &'a AtomicBool,
) -> Result<CandidateSourceCatalogInputPlan<'a, I>> {
    l.validate()?;
    l.catalog.validate()?;
    if limits.max_manifest_members == 0
        || limits.max_manifest_members == u64::MAX
        || limits.max_work_bytes == 0
        || limits.max_work_bytes == u64::MAX
        || disk_limits.max_selected_members == 0
        || disk_limits.max_selected_members > limits.max_manifest_members
        || disk_limits.max_selected_members > l.catalog.max_files
        || disk_limits.max_locator_bytes == 0
        || disk_limits.max_locator_bytes == u64::MAX
        || disk_limits.max_sqlite_bytes < 4096
        || disk_limits.max_sqlite_bytes == u64::MAX
        || disk_limits.max_workspace_bytes < 16 * 1024
        || disk_limits.max_work_bytes != limits.max_work_bytes
    {
        return Err(Error::Budget("candidate disk catalog explicit limits"));
    }
    let binding = CandidateValidationBinding::from_verified_input(
        input,
        expected_identity,
        coverage.clone(),
        l.deadline,
        cancelled,
    )?;
    let mut initial_work = 0;
    verify_candidate_input_eof(
        input,
        expected_identity,
        coverage,
        l.deadline,
        cancelled,
        limits.max_manifest_members,
        l.catalog.max_file_bytes,
        &mut initial_work,
        limits.max_work_bytes,
    )?;

    let baseline = std::mem::size_of::<CandidateSourceCatalogInputPlan<'a, I>>()
        .checked_add(CandidateSourceCatalogInputPlan::<I>::receipt_state_upper_bound(0)?)
        .and_then(|n| n.checked_add(4096))
        .filter(|n| *n < disk_limits.max_workspace_bytes)
        .ok_or(Error::Budget("candidate disk catalog fixed state"))?;
    storage.execute_batch("CREATE TABLE selected(path TEXT PRIMARY KEY,size BLOB NOT NULL,sha BLOB NOT NULL,flags INTEGER NOT NULL) WITHOUT ROWID; CREATE TABLE pending(path TEXT PRIMARY KEY) WITHOUT ROWID; CREATE TABLE scanned(path TEXT PRIMARY KEY) WITHOUT ROWID;")?;
    let mut plan = Planning {
        cut: PlanningCut::Candidate(input.source_input()),
        revision: None,
        limits,
        graph_limits: l,
        cancelled,
        entries: PlanningEntries::Disk(DiskEntries {
            db: PlanningDatabase::Shared(storage),
            file: None,
            isolation: None,
            limits: disk_limits,
            rows: 0,
            cleanup_complete: std::cell::Cell::new(false),
            logical_bytes: 0,
            deadline: l.deadline,
            cancelled,
        }),
        plan_bytes: baseline,
        work_bytes: initial_work,
        raw_input_max_bytes: l.catalog.max_file_bytes,
        json_input_max_bytes: l.catalog.max_file_bytes,
        cold_raw_parser: true,
        workspace_limit: Some(disk_limits.max_workspace_bytes),
        live_raw_bytes: 0,
        live_parser_upper: 0,
    };
    for registry in [ENTITY, RELATION] {
        if !plan.select(registry, CONTRACT_FILES)? {
            return Err(Error::Invalid("candidate catalog source registry absent"));
        }
    }
    let raw = plan.raw(ENTITY)?;
    let entities = plan.parse_packet(&raw, l.catalog.max_row_bytes)?;
    plan.live_parser_upper = plan
        .live_parser_upper
        .checked_mul(2)
        .ok_or(Error::Budget("candidate catalog registry state"))?;
    plan.check_workspace()?;
    let basenames = catalog::source_basenames(entities.value())?;
    let observed = input
        .for_each_current_member(l.deadline, cancelled, &mut |meta, raw| {
            if raw.len() > l.catalog.max_file_bytes {
                return Err(tos_validation::item_rules::ItemRefusal::BudgetCheck {
                    check: "candidate disk catalog physical member bytes",
                    used: u64::try_from(raw.len()).ok(),
                    limit: u64::try_from(l.catalog.max_file_bytes).ok(),
                });
            }
            plan.work_bytes = plan
                .work_bytes
                .checked_add(meta.size_bytes)
                .filter(|n| *n <= limits.max_work_bytes)
                .ok_or(tos_validation::item_rules::ItemRefusal::Budget)?;
            if raw.len() as u64 != meta.size_bytes {
                return Err(tos_validation::item_rules::ItemRefusal::Source(
                    "candidate disk full membership binding".into(),
                ));
            }
            plan.initial(meta.path, &basenames)
                .map_err(|e| tos_validation::item_rules::ItemRefusal::Source(e.to_string()))
        })
        .map_err(|error| candidate_input_refusal(error))?;
    if &observed != coverage {
        return Err(Error::Invalid("candidate disk planning physical EOF"));
    }
    drop(basenames);
    drop(entities);
    drop(raw);
    plan.live_raw_bytes = 0;
    plan.live_parser_upper = 0;
    while let Some(path) = plan.entries.pop()? {
        let raw = plan.raw(&path)?;
        plan.packet(&path, &raw)?;
        drop(raw);
        plan.live_raw_bytes = 0;
        plan.live_parser_upper = 0;
    }
    let collections = spool_receipts(&plan.entries, l, cancelled)?;
    input
        .verify_current_fence(coverage, l.deadline, cancelled)
        .map_err(|error| candidate_input_refusal(error))?;
    if input.input_identity() != expected_identity {
        return Err(Error::Invalid("candidate disk final identity"));
    }
    let work_bytes = plan.work_bytes;
    Ok(CandidateSourceCatalogInputPlan {
        plan: CandidateCatalogPlanning::Disk {
            entries: plan.entries,
            recipe: PlanInput {
                binding,
                collections,
            },
            limits,
            raw_input_max_bytes: plan.raw_input_max_bytes,
            workspace: disk_limits.max_workspace_bytes,
        },
        consumed: false,
        deadline: l.deadline,
        cancelled,
        work_bytes,
    })
}
