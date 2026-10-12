//! Addressed current-source input for maintained Growth preparers.
//!
//! This adapter intentionally exposes exact files and owner-local ordered
//! prefixes only. Whole-source membership remains with the source cohort
//! bootstrap/cold verifier; the enclosing PG readset is validated before the
//! prepared package can enter the shared revision writer.

use crate::source_cohort::revision::{PgRecordRevisionIdentity, SourceRevisionReadSession};
use crate::source_revisions::ReadonlyRecordFiles;
use std::cell::RefCell;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::RelativePath;
use tos_source_store::SourcePresenceV1;
use tos_validation::item_rules::ItemRefusal;
use tos_validation::record_biblio_cut::{
    SourceCutInput, SourceCutInputCoverage, SourceCutInputWithIdentity, SourceCutMemberMeta,
    SourceCutPrefixCoverage,
};

const MAX_PREFIX_MEMBERS: usize = 4_096;
const MAX_PREFIX_TOTAL_BYTES: usize = 33_554_432;
const MAX_PREFIX_MEMBER_BYTES: usize = 8_388_608;
const MAX_PREFIX_DEPTH: usize = 64;

/// A borrowed view of one fenced PG source epoch. Reads are recorded by the
/// underlying `SourceRevisionReadSession`; `SourceRevisionReadSet` remains the
/// authority used by the owner package's precommit validator.
pub(crate) struct PgGrowthCurrentInput<'session, 'store> {
    session: RefCell<&'session mut SourceRevisionReadSession<'store>>,
    identity: PgRecordRevisionIdentity,
    max_prefix_members: usize,
    max_member_bytes: usize,
}

impl<'session, 'store> PgGrowthCurrentInput<'session, 'store> {
    pub(crate) fn new(
        session: &'session mut SourceRevisionReadSession<'store>,
        max_prefix_members: usize,
        max_member_bytes: usize,
    ) -> Result<Self, ItemRefusal> {
        if max_prefix_members == 0
            || max_prefix_members > MAX_PREFIX_MEMBERS
            || max_member_bytes == 0
            || max_member_bytes > MAX_PREFIX_MEMBER_BYTES
        {
            return Err(ItemRefusal::Unsupported(
                "PG Growth selected-input budget".into(),
            ));
        }
        let identity = session.selection().identity();
        Ok(Self {
            session: RefCell::new(session),
            identity,
            max_prefix_members,
            max_member_bytes,
        })
    }

    fn refusal() -> ItemRefusal {
        ItemRefusal::Source("PG Growth selected source read failed".into())
    }

    /// Resolve annotation owner IDs through the selected PG index while this
    /// input still owns the borrowed read session. Exact packet/member
    /// validation remains the caller's responsibility.
    pub(crate) fn lookup_annotation_identity_owners(
        &self,
        tokens: &std::collections::BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> crate::DurableResult<
        std::collections::BTreeMap<
            String,
            std::collections::BTreeSet<
                crate::source_annotation_identity_index::AnnotationEntityOwner,
            >,
        >,
    > {
        self.session
            .borrow_mut()
            .lookup_annotation_identity_owners(tokens, deadline, cancelled)
    }

    /// Observe the candidate's dedicated uniqueness namespace. Sign IDs are
    /// also existing Claim IDs, so this must not use the cross-kind identity
    /// probe used by the other Growth owners.
    pub(crate) fn lookup_sign_promotion_candidate(
        &self,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> crate::DurableResult<Option<RelativePath>> {
        self.session
            .borrow_mut()
            .lookup_sign_promotion_candidate(token, deadline, cancelled)
    }

    /// Record an owner-selected output-home absence without releasing this
    /// exact input's mutable borrow of the source revision session.
    pub(crate) fn observe_home_absence(
        &self,
        home: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> crate::DurableResult<crate::source_cohort::revision::PgSourceHomeObservation> {
        self.session
            .borrow_mut()
            .observe_home_absence(home, deadline, cancelled)
    }

    /// Record a semantic unique-index predicate in the same read session
    /// used for the selected owner inputs.
    pub(crate) fn observe_unique_index(
        &self,
        kind: &str,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> crate::DurableResult<crate::source_cohort::revision::PgSourceUniqueIndexObservation> {
        self.session
            .borrow_mut()
            .observe_unique_index(kind, token, deadline, cancelled)
    }

    /// Enumerate one selected PG directory through its bounded source index.
    /// Callers must keep this to an owner-selected subtree and let the session
    /// readset bind the exact listing.
    pub(crate) fn list_directory(
        &self,
        directory: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> crate::source_command::SourceCommandResult<Vec<(String, bool)>> {
        self.session
            .borrow_mut()
            .list_directory(directory, deadline, cancelled)
    }

    fn member_exists(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.session
            .borrow_mut()
            .has_file(path, deadline, cancelled)
            .map_err(|_| Self::refusal())
    }

    fn visit_prefix(
        &self,
        directory: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
        depth: usize,
        seen: &mut usize,
        total_bytes: &mut usize,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        if depth > MAX_PREFIX_DEPTH {
            return Err(ItemRefusal::BudgetCheck {
                check: "PG Growth selected prefix depth",
                used: Some(depth as u64),
                limit: Some(MAX_PREFIX_DEPTH as u64),
            });
        }
        let parsed = RelativePath::parse(directory)
            .map_err(|_| ItemRefusal::Source("invalid PG Growth source prefix".into()))?;
        if parsed.as_str() != directory {
            return Err(ItemRefusal::Source(
                "noncanonical PG Growth source prefix".into(),
            ));
        }
        let children = self
            .session
            .borrow_mut()
            .list_directory(directory, deadline, cancelled)
            .map_err(|_| Self::refusal())?;
        for (name, is_directory) in children {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(ItemRefusal::Source(
                    "PG Growth source prefix cancelled".into(),
                ));
            }
            if Instant::now() >= deadline {
                return Err(ItemRefusal::Deadline);
            }
            let path = format!("{directory}/{name}");
            if is_directory {
                self.visit_prefix(
                    &path,
                    deadline,
                    cancelled,
                    depth + 1,
                    seen,
                    total_bytes,
                    visit,
                )?;
                continue;
            }
            *seen = seen.checked_add(1).ok_or(ItemRefusal::Budget)?;
            if *seen > self.max_prefix_members {
                return Err(ItemRefusal::BudgetCheck {
                    check: "PG Growth selected prefix members",
                    used: Some(*seen as u64),
                    limit: Some(self.max_prefix_members as u64),
                });
            }
            let raw = self
                .session
                .borrow_mut()
                .read(&path, self.max_member_bytes, deadline, cancelled)
                .map_err(|_| Self::refusal())?;
            *total_bytes = total_bytes
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= MAX_PREFIX_TOTAL_BYTES)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "PG Growth selected prefix bytes",
                    used: None,
                    limit: Some(MAX_PREFIX_TOTAL_BYTES as u64),
                })?;
            let size_bytes = u64::try_from(raw.len()).map_err(|_| ItemRefusal::Budget)?;
            visit(SourceCutMemberMeta {
                path: &path,
                size_bytes,
            })?;
        }
        Ok(())
    }
}

impl SourceCutInput for PgGrowthCurrentInput<'_, '_> {
    fn for_each_current_member_meta(
        &self,
        _deadline: Instant,
        _cancelled: &AtomicBool,
        _visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "PG Growth input has no whole-source member walk".into(),
        ))
    }

    fn current_member_size(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<u64>, ItemRefusal> {
        if !self.member_exists(path, deadline, cancelled)? {
            return Ok(None);
        }
        let raw = self
            .session
            .borrow_mut()
            .read(path, self.max_member_bytes, deadline, cancelled)
            .map_err(|_| Self::refusal())?;
        u64::try_from(raw.len())
            .map(Some)
            .map_err(|_| ItemRefusal::Budget)
    }

    fn for_each_current_member_meta_under(
        &self,
        directory: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<SourceCutPrefixCoverage, ItemRefusal> {
        let mut seen = 0usize;
        let mut total_bytes = 0usize;
        self.visit_prefix(
            directory,
            deadline,
            cancelled,
            0,
            &mut seen,
            &mut total_bytes,
            visit,
        )?;
        SourceCutPrefixCoverage::after_verified_prefix_eof(
            directory,
            u64::try_from(seen).map_err(|_| ItemRefusal::Budget)?,
        )
    }

    fn with_current_member(
        &self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        if !self.member_exists(path, deadline, cancelled)? {
            return Ok(());
        }
        let max_bytes = max_bytes.min(self.max_member_bytes);
        let raw = self
            .session
            .borrow_mut()
            .read(path, max_bytes, deadline, cancelled)
            .map_err(|_| Self::refusal())?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Source("invalid PG Growth member path".into()))?;
        if relative.as_str() != path {
            return Err(ItemRefusal::Source(
                "noncanonical PG Growth member path".into(),
            ));
        }
        visit(
            SourceCutMemberMeta {
                path: relative.as_str(),
                size_bytes: u64::try_from(raw.len()).map_err(|_| ItemRefusal::Budget)?,
            },
            &raw,
        )
    }

    fn for_each_current_member(
        &self,
        _deadline: Instant,
        _cancelled: &AtomicBool,
        _visit: &mut dyn FnMut(SourceCutMemberMeta<'_>, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<SourceCutInputCoverage, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "PG Growth input has no whole-source membership walk".into(),
        ))
    }

    fn path_presence(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourcePresenceV1>, ItemRefusal> {
        Ok(self
            .member_exists(path, deadline, cancelled)?
            .then_some(SourcePresenceV1::File))
    }

    fn verify_current_fence(
        &self,
        _coverage: &SourceCutInputCoverage,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "PG Growth exact-read fence is verified by its SourceRevisionReadSet".into(),
        ))
    }
}

impl SourceCutInputWithIdentity<PgRecordRevisionIdentity> for PgGrowthCurrentInput<'_, '_> {
    fn input_identity(&self) -> &PgRecordRevisionIdentity {
        &self.identity
    }

    fn source_input(&self) -> &dyn SourceCutInput {
        self
    }
}
