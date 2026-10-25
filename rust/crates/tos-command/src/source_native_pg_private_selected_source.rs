//! SAME protected owner/source/worker adapter. The public metadata census
//! enumerates real physical directories and excludes maintained hidden/payload
//! namespaces; it is NOT whole-corpus or virtual-publication EOF. Retained
//! byte subsets are never interpreted as coverage or absence.
use super::super::with_pg_role_kernel_schema;
use super::private_schema::{admitted_context_is_tos, context_identity, run_private};
use super::{CurrentPgRoleOwner, PgRoleControl, SelectedRoleRead};
use crate::pwe_source_owner::{PgPayloadMember, PgPayloadViews};
use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_command::{
    CommandContext, SourceCommandError as Error, SourceCommandResult as CommandResult,
};
use crate::source_native_cli::private_assessment::{
    OperationBudget, original_payer::OriginalAssessmentPayer,
};
use crate::source_native_cli::private_owner::profile::{
    PrivateSelectedSchemaExecutor, PrivateSelectedSourceIdentity, PrivateSelectedSourceInput,
};
use crate::source_revisions::RecordRead;
use crate::{DurableError, DurableResult};
use std::{
    cell::RefCell,
    collections::BTreeSet,
    mem::{size_of, size_of_val},
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::RelativePath;
use tos_validation::{item_rules::ItemRefusal, record_biblio_cut::SourceCutPrefixCoverage};

#[derive(Debug)]
pub(crate) enum PgPrivateSelectedSourceCause {
    Owner(DurableError),
    Visit(ItemRefusal),
}
#[derive(Debug)]
pub(crate) struct PgPrivateSelectedSourceFailure {
    pub(crate) original: PgPrivateSelectedSourceCause,
    /// A subsequent kernel/owner failure, not a fabricated second original.
    pub(crate) kernel_completion: Option<DurableError>,
}
type Output<T> = std::result::Result<T, PgPrivateSelectedSourceFailure>;
type NativeResult<T> = std::result::Result<T, ItemRefusal>;
const PUBLIC_ROOT: &str = "ToS/source-witnesses";
const MAX_PUBLIC_ENTRIES: usize = 32_768;
const MAX_PUBLIC_PATHS: usize = 2_048;
// Same hidden/payload/catalog exclusions as the maintained public metadata
// route. This is a metadata candidate superset: its consumers still apply
// exact profile/Claim/form/annotation/provenance basename predicates.
fn public_metadata_skip(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "payload" | "local-content" | "catalog")
}
fn public_metadata_reserved(parent: &str, name: &str) -> bool {
    parent == PUBLIC_ROOT && name == "owner-local"
}
fn public_metadata_step_work(parent_bytes: usize, name_bytes: usize) -> Option<usize> {
    // Four skip comparisons plus both reserved-path operands and six events.
    name_bytes
        .checked_mul(5)?
        .checked_add(parent_bytes)?
        .checked_add(PUBLIC_ROOT.len())?
        .checked_add("owner-local".len())?
        .checked_add("payload".len() + "local-content".len() + "catalog".len() + 1 + 6)
}
fn admitted_public_metadata_skip(
    parent: &str,
    name: &str,
    budget: &mut OperationBudget,
) -> CommandResult<bool> {
    let work = public_metadata_step_work(parent.len(), name.len()).ok_or_else(overflow)?;
    budget.charge_work(work)?;
    budget.charge_bytes(size_of::<CommandResult<bool>>())?;
    if public_metadata_skip(name) {
        return Ok(true);
    }
    if public_metadata_reserved(parent, name) {
        return Err(Error::Denied(
            "reserved owner-local namespace in selected public identity prefix",
        ));
    }
    Ok(false)
}
fn overflow() -> Error {
    Error::Unsupported("PG private selected source accounting overflow")
}
fn owned(error: DurableError) -> PgPrivateSelectedSourceFailure {
    PgPrivateSelectedSourceFailure {
        original: PgPrivateSelectedSourceCause::Owner(error),
        kernel_completion: None,
    }
}
fn complete<T>(
    body: DurableResult<Option<T>>,
    first: Option<PgPrivateSelectedSourceCause>,
) -> Output<T> {
    match (first, body) {
        (Some(original), body) => Err(PgPrivateSelectedSourceFailure {
            original,
            kernel_completion: body.err(),
        }),
        (None, Err(original)) => Err(owned(original)),
        (None, Ok(Some(value))) => Ok(value),
        (None, Ok(None)) => Err(owned(DurableError::Corrupt(
            "private selected source owned outcome missing",
        ))),
    }
}
// Private implementation detail, constructed below only with the concrete
// retained reader and control closures from the SAME real protected owner.
struct SelectedInput<'read, 'pay, 'budget, 'cancel, R: ?Sized> {
    reader: &'read R,
    payer: &'pay OriginalAssessmentPayer<'budget, 'cancel>,
    selected: PgRecordRevisionIdentity,
    children: &'read dyn Fn(&RelativePath) -> CommandResult<Vec<(String, bool)>>,
    owner_failed: &'read dyn Fn() -> bool,
    first: &'read RefCell<Option<PgPrivateSelectedSourceCause>>,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
}
impl<R: RecordRead<Identity = PgRecordRevisionIdentity> + ?Sized> SelectedInput<'_, '_, '_, '_, R> {
    fn command(&self, error: Error) -> ItemRefusal {
        if !(self.owner_failed)() && self.first.borrow().is_none() {
            *self.first.borrow_mut() = Some(PgPrivateSelectedSourceCause::Owner(
                DurableError::Source(error),
            ));
        }
        // Allocation-free INTERNAL relay, never a new payer.error(Budget).
        ItemRefusal::Budget
    }
    fn visit_error(&self, original: ItemRefusal) -> ItemRefusal {
        if !(self.owner_failed)() && self.first.borrow().is_none() {
            // Immutable payer borrow was pointer-bound to the actual host
            // before construction. Latch before ANY later paid owner exit,
            // without a budget loan, mapping or consuming this owned cause.
            self.payer.latch_native_schema_refusal(&original);
            *self.first.borrow_mut() = Some(PgPrivateSelectedSourceCause::Visit(original));
        }
        ItemRefusal::Budget
    }
    fn inspect(&self, bytes: usize, work: usize) -> NativeResult<()> {
        self.payer
            .inspect(bytes, work)
            .map_err(|error| self.command(error))
    }
    fn ready(&self, d: Instant, c: &AtomicBool) -> NativeResult<()> {
        if self.first.borrow().is_some() {
            return Err(ItemRefusal::Budget);
        }
        if d != self.deadline || !std::ptr::eq(c, self.cancelled) {
            return Err(self.command(Error::Conflict(
                "private selected source cutoff differs from actual owner",
            )));
        }
        self.reader.check().map_err(|error| self.command(error))
    }
    fn finish_source<T>(&self, body: NativeResult<T>) -> NativeResult<T> {
        // Bypass the LOCAL retained-error short circuit ONLY to attempt the
        // actual paid owner exit on ordinary Err too. Existing actual owner
        // failure/refusal can itself prevent physical I/O. The owned first
        // cause stays in its original cell/reader control, never overwritten.
        let exit = self.reader.check().map_err(|error| self.command(error));
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    fn path(&self, path: &str) -> NativeResult<RelativePath> {
        let bytes = path
            .len()
            .checked_add(size_of::<RelativePath>() + size_of::<CommandResult<RelativePath>>())
            .ok_or_else(|| self.command(overflow()))?;
        let work = path
            .len()
            .checked_mul(5)
            .and_then(|n| n.checked_add(4))
            .ok_or_else(|| self.command(overflow()))?;
        self.inspect(bytes, work)?;
        RelativePath::parse(path)
            .map_err(|_| self.command(Error::Invalid("private selected source path")))
    }
    fn push(&self, pending: &mut Vec<String>, path: String) -> NativeResult<()> {
        let needed = pending
            .len()
            .checked_add(1)
            .ok_or_else(|| self.command(overflow()))?;
        if needed > pending.capacity() {
            let capacity = pending
                .capacity()
                .checked_mul(2)
                .ok_or_else(|| self.command(overflow()))?
                .max(needed)
                .max(4);
            let allocation = capacity
                .checked_mul(size_of::<String>())
                .ok_or_else(|| self.command(overflow()))?;
            let relocation = pending
                .capacity()
                .checked_mul(size_of::<String>())
                .ok_or_else(|| self.command(overflow()))?;
            self.inspect(
                allocation,
                relocation
                    .checked_add(1)
                    .ok_or_else(|| self.command(overflow()))?,
            )?;
        }
        // Exact requested backing growth is admitted above; no implicit
        // allocator-growth claim is used as proof of whole native fit.
        if needed > pending.capacity() {
            let capacity = pending
                .capacity()
                .checked_mul(2)
                .ok_or_else(|| self.command(overflow()))?
                .max(needed)
                .max(4);
            pending
                .try_reserve_exact(capacity - pending.len())
                .map_err(|_| self.command(overflow()))?;
        }
        pending.push(path);
        Ok(())
    }
    fn child(&self, parent: &str, name: &str) -> NativeResult<String> {
        let bytes = parent
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_add(name.len()))
            .ok_or_else(|| self.command(overflow()))?;
        self.inspect(
            bytes
                .checked_add(size_of::<String>())
                .ok_or_else(|| self.command(overflow()))?,
            bytes,
        )?;
        let mut path = String::new();
        path.try_reserve_exact(bytes)
            .map_err(|_| self.command(overflow()))?;
        path.push_str(parent);
        path.push('/');
        path.push_str(name);
        // Same actual canonical path grammar before pending push or visitor.
        let _validated = self.path(&path)?;
        Ok(path)
    }
}
impl<R: RecordRead<Identity = PgRecordRevisionIdentity> + ?Sized> PrivateSelectedSourceInput
    for SelectedInput<'_, '_, '_, '_, R>
{
    fn private_selection_identity(&self) -> PrivateSelectedSourceIdentity {
        PrivateSelectedSourceIdentity::PgRecord(self.selected)
    }
    fn selected_public_identity_paths(
        &self,
        d: Instant,
        c: &AtomicBool,
    ) -> NativeResult<Option<Vec<String>>> {
        let body = (|| {
            self.ready(d, c)?;
            self.inspect(
                2 * size_of::<Vec<String>>()
                    + size_of::<usize>()
                    + size_of::<NativeResult<Option<Vec<String>>>>(),
                1,
            )?;
            let root = self.path(PUBLIC_ROOT)?.into_string();
            let mut pending = Vec::new();
            self.push(&mut pending, root)?;
            let mut paths = Vec::new();
            let mut entries = 0usize;
            while let Some(parent) = pending.pop() {
                self.ready(d, c)?;
                let parsed = self.path(&parent)?;
                // This physical query reaches actual None and records the
                // actual source-home generation. Hidden publication virtuals
                // are OUTSIDE this deliberately narrower metadata census.
                let children = (self.children)(&parsed).map_err(|error| self.command(error))?;
                for (name, directory) in children {
                    self.inspect(0, size_of::<usize>() + 1)?;
                    entries = entries
                        .checked_add(1)
                        .ok_or_else(|| self.command(overflow()))?;
                    if entries > MAX_PUBLIC_ENTRIES {
                        return Err(self.command(Error::Unsupported(
                            "PG private public identity entry budget",
                        )));
                    }
                    // Every encountered name is admitted BEFORE skips. This
                    // short original budget loan ends before the next owner
                    // query, source callback or schema-worker use.
                    if self
                        .payer
                        .with_operation(|budget| {
                            admitted_public_metadata_skip(&parent, &name, budget)
                        })
                        .map_err(|error| self.command(error))?
                    {
                        continue;
                    }
                    let child = self.child(&parent, &name)?;
                    if directory {
                        self.push(&mut pending, child)?;
                    } else {
                        if paths.len() == MAX_PUBLIC_PATHS {
                            return Err(self.command(Error::Unsupported(
                                "PG private public identity path budget",
                            )));
                        }
                        self.push(&mut paths, child)?;
                    }
                }
            }
            self.ready(d, c)?;
            // Complete metadata path superset only. Neither a caller list nor
            // retained byte membership can reach Some without the real scan.
            Ok(Some(paths))
        })();
        self.finish_source(body)
    }
    // No fabricated annotation index. Its maintained source-bound fallback
    // consumes the complete metadata census above and filters native packets.
    fn for_each_current_member_path_under(
        &self,
        directory: &str,
        d: Instant,
        c: &AtomicBool,
        visit: &mut dyn FnMut(&str) -> NativeResult<()>,
    ) -> NativeResult<SourceCutPrefixCoverage> {
        let body = (|| {
            self.ready(d, c)?;
            // Before classifying even the unsupported prefix, price the complete
            // bounded string comparisons. This is NOT a virtual absence witness.
            self.inspect(
                0,
                crate::source_cohort::revision::publication_prefix_discriminator_work(
                    directory.len(),
                )
                .ok_or_else(|| self.command(overflow()))?,
            )?;
            if !crate::source_cohort::revision::physical_prefix_excludes_publication_virtuals(
                directory,
            ) {
                return Err(self.command(Error::Unsupported("private selected source prefix requires admitted virtual publication enumeration")));
            }
            let root = self.path(directory)?.into_string();
            self.inspect(size_of::<Vec<String>>() + size_of::<u64>(), 1)?;
            let mut pending = Vec::new();
            self.push(&mut pending, root)?;
            let mut count = 0u64;
            while let Some(parent) = pending.pop() {
                self.ready(d, c)?;
                let parsed = self.path(&parent)?;
                // Source/OperationBudget/host loans END with this call, before
                // invoking a visitor that may use the SAME private schema worker.
                let children = (self.children)(&parsed).map_err(|error| self.command(error))?;
                for (name, directory) in children {
                    self.inspect(0, size_of::<(String, bool)>() + 1)?;
                    let child = self.child(&parent, &name)?;
                    if directory {
                        self.push(&mut pending, child)?;
                    } else {
                        self.inspect(0, 1)?;
                        count = count
                            .checked_add(1)
                            .ok_or_else(|| self.command(overflow()))?;
                        visit(&child).map_err(|original| self.visit_error(original))?;
                    }
                }
            }
            // Only every real directory query reaching None AND every visitor
            // succeeding AND this real paid current-owner fence can reach here.
            self.ready(d, c)?;
            let bytes = directory
                .len()
                .checked_mul(2)
                .and_then(|n| {
                    n.checked_add(
                        size_of::<SourceCutPrefixCoverage>()
                            + size_of::<NativeResult<SourceCutPrefixCoverage>>()
                            + size_of::<RelativePath>(),
                    )
                })
                .ok_or_else(|| self.command(overflow()))?;
            let work = directory
                .len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(4))
                .ok_or_else(|| self.command(overflow()))?;
            self.inspect(bytes, work)?;
            SourceCutPrefixCoverage::after_verified_prefix_eof(directory, count)
                .map_err(|original| self.visit_error(original))
        })();
        self.finish_source(body)
    }
    fn with_current_member(
        &self,
        path: &str,
        max_bytes: usize,
        d: Instant,
        c: &AtomicBool,
        visit: &mut dyn FnMut(&str, &[u8]) -> NativeResult<()>,
    ) -> NativeResult<()> {
        let body = (|| {
            self.ready(d, c)?;
            let path_owned = self.path(path)?;
            if let Some(raw) = self
                .reader
                .file(&path_owned)
                .map_err(|error| self.command(error))?
            {
                self.inspect(0, size_of::<usize>() + 1)?;
                if raw.len() > max_bytes {
                    return Err(self.command(Error::Unsupported(
                        "private selected source member byte bound",
                    )));
                }
                // Logical loan work only. Subsequent visitor decoding/content
                // scans/Record copies retain their OWN prospective payment law.
                self.inspect(0, raw.len())?;
                visit(path, raw).map_err(|original| self.visit_error(original))?;
            }
            self.ready(d, c)
        })();
        self.finish_source(body)
    }
}

/// This factory provides no target/request/permission/NativeAccount/report.
/// Its source and worker can coexist; neither retains another mutable owner
/// loan. All physical source navigation records actual epoch predicates.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_pg_private_selected_source_kernel<
    'payload,
    'owner,
    'view,
    'store,
    'budget,
    'cancel,
    'pure,
    T,
>(
    current: &mut CurrentPgRoleOwner<'_, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    views: &PgPayloadViews<'payload, 'budget, 'cancel>,
    paths: &BTreeSet<RelativePath>,
    ctx: &CommandContext,
    payer: &OriginalAssessmentPayer<'budget, 'cancel>,
    d: Instant,
    c: &'cancel AtomicBool,
    run: impl FnOnce(
        &dyn PrivateSelectedSourceInput,
        &mut dyn PrivateSelectedSchemaExecutor,
    ) -> CommandResult<T>,
) -> Output<T> {
    let prepared = (|| {
        current.check_current()?;
        if paths.is_empty() || paths.len() > 64 {
            return Err(DurableError::Refused(
                "private PG kernel selected closure count",
            ));
        }
        let backing = paths
            .len()
            .checked_mul(size_of::<PgPayloadMember<'_>>())
            .ok_or(DurableError::Source(overflow()))?;
        let fixed = size_of::<SelectedRoleRead<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>>()
            .checked_add(size_of::<
                SelectedInput<'_, '_, '_, '_, dyn RecordRead<Identity = PgRecordRevisionIdentity>>,
            >())
            .and_then(|n| {
                n.checked_add(2 * size_of::<PgRoleControl<'_, '_, '_, '_, '_, '_, '_, '_, '_>>())
            })
            .and_then(|n| n.checked_add(size_of::<Instant>() + size_of::<&AtomicBool>()))
            .and_then(|n| n.checked_add(size_of::<RefCell<Option<PgPrivateSelectedSourceCause>>>()))
            .and_then(|n| {
                n.checked_add(
                    size_of::<CommandResult<Option<T>>>()
                        + size_of::<Output<T>>()
                        + size_of::<DurableResult<Option<T>>>()
                        + size_of_val(&run),
                )
            })
            .and_then(|n| n.checked_add(backing))
            .ok_or(DurableError::Source(overflow()))?;
        current.with_source_epoch(|_, actual, same| {
            actual
                .charge_sto_source_measures(same, 0, fixed, 0, 0, 1)
                .map_err(DurableError::Source)?;
            actual
                .inspect(0, 2 * size_of::<usize>())
                .map_err(DurableError::Source)?;
            if !std::ptr::eq(actual, payer) {
                return Err(DurableError::Conflict(
                    "private selected source supplied a different original payer",
                ));
            }
            let (deadline, cancelled) = actual.source_cutoff();
            if deadline != d || !std::ptr::eq(cancelled, c) {
                return Err(DurableError::Conflict(
                    "private selected source cutoff differs from original payer",
                ));
            }
            Ok(())
        })?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(paths.len())
            .map_err(|_| DurableError::Source(overflow()))?;
        for path in paths {
            members.push(current.selected_member(views, path)?);
        }
        Ok((members, *current.input_identity(), current.account_uid()))
    })();
    let (members, selected, uid) = match prepared {
        Ok(value) => value,
        Err(original) => {
            return Err(PgPrivateSelectedSourceFailure {
                original: PgPrivateSelectedSourceCause::Owner(original),
                kernel_completion: current.check_current().err(),
            });
        }
    };
    let first = RefCell::new(None);
    let body = with_pg_role_kernel_schema(current, d, c, |worker| {
        let control = worker.control();
        let reader = SelectedRoleRead {
            members,
            control,
            selected,
            uid,
            deadline: d,
            cancelled: c,
        };
        reader.check()?;
        control.with_operation(|budget| {
            budget.charge_work(size_of::<u64>() + size_of::<tos_foundation::SourceRevision>())
        })?;
        context_identity(ctx, selected.bootstrap_revision, uid)?;
        for file in &ctx.files {
            if !control.with_operation(|budget| admitted_context_is_tos(file, budget, d, c))? {
                continue;
            }
            control.with_operation(|budget| {
                budget.charge_work(file.raw.len().checked_add(1).ok_or_else(overflow)?)
            })?;
            if reader.file(&file.path)? != Some(file.raw.as_slice()) {
                return Err(Error::Conflict(
                    "private PG context bytes differ from retained source",
                ));
            }
        }
        let children = |parent: &RelativePath| {
            control.with_owner(|owner| {
                owner.with_source_epoch(|epoch, payer, _same| {
                    let mut foreign = None;
                    let result = payer.with_operation(|budget| {
                        match epoch
                            .list_immediate_children_with_data_admission(parent, budget, d, c)
                        {
                            Ok(value) => Ok(Some(value)),
                            Err(DurableError::Source(original)) => Err(original),
                            Err(original) => {
                                foreign = Some(original);
                                Ok(None)
                            }
                        }
                    });
                    match (result, foreign) {
                        (_, Some(original)) => Err(original),
                        (Err(original), None) => Err(DurableError::Source(original)),
                        (Ok(Some(value)), None) => Ok(value),
                        (Ok(None), None) => Err(DurableError::Corrupt(
                            "private source directory owned outcome missing",
                        )),
                    }
                })
            })
        };
        let owner_failed = || control.has_retained_failure();
        let input = SelectedInput {
            reader: &reader as &dyn RecordRead<Identity = PgRecordRevisionIdentity>,
            payer,
            selected,
            children: &children,
            owner_failed: &owner_failed,
            first: &first,
            deadline: d,
            cancelled: c,
        };
        let result = run_private(worker, &reader, |worker| run(&input, worker));
        // Retain actual visitor error outside the generic command kernel;
        // its internal relay can be swallowed/mapped but cannot become Ok(T).
        if first.borrow().is_some() {
            Ok(None)
        } else {
            result.map(Some)
        }
    });
    // The kernel constructor can itself refuse before entering its callback.
    // Attempt the actual outer current-use fence on those Result exits too;
    // its established body-first projection cannot turn failure into success.
    let exited = current.check_current();
    let body = match (body, exited) {
        (Err(original), _) | (Ok(_), Err(original)) => Err(original),
        (Ok(value), Ok(())) => Ok(value),
    };
    complete(body, first.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_metadata_filter_excludes_only_maintained_hidden_payload_names() {
        // Pure name law only; no owner, census or EOF token is fabricated.
        for name in [
            ".metadata-publication.json",
            ".metadata-transactions",
            ".hidden",
            "payload",
            "local-content",
            "catalog",
        ] {
            assert!(public_metadata_skip(name));
        }
        for name in [
            "agents",
            "semantic-annotation.json",
            "x.human-forms.json",
            "catalogue",
            "payload.json",
            "é",
        ] {
            assert!(!public_metadata_skip(name));
        }
        assert!(public_metadata_reserved(PUBLIC_ROOT, "owner-local"));
        assert!(!public_metadata_reserved(
            "ToS/source-witnesses/agents",
            "owner-local"
        ));
        assert!(!public_metadata_reserved(PUBLIC_ROOT, "owner-local-other"));
    }
    #[test]
    fn public_metadata_admission_precedes_hidden_skip_and_reserved_denial() {
        use std::time::Duration;
        fn budget() -> OperationBudget {
            let d = Instant::now() + Duration::from_secs(30);
            OperationBudget::new(
                &serde_json::json!({"budgets":{
                    "worker_cpu_seconds":1,"worker_address_space_bytes":1_048_576
                }}),
                d,
            )
            .unwrap()
        }
        // Actual posted OperationBudget currency, not an owner/census fixture.
        let mut b = budget();
        let quote = public_metadata_step_work(PUBLIC_ROOT.len(), ".hidden".len()).unwrap();
        b.lower_remaining(1_048_576, quote + 7).unwrap();
        b.charge_work(7).unwrap();
        assert!(admitted_public_metadata_skip(PUBLIC_ROOT, ".hidden", &mut b).unwrap());
        assert_eq!(b.test_work_remaining(), 0);
        assert!(matches!(
            admitted_public_metadata_skip(PUBLIC_ROOT, ".hidden", &mut b),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(b.test_work_remaining(), 0);
        let mut b = budget();
        let quote = public_metadata_step_work(PUBLIC_ROOT.len(), "owner-local".len()).unwrap();
        b.lower_remaining(1_048_576, quote - 1).unwrap();
        // Too little admission refuses BEFORE the reserved-name branch; it
        // cannot report a semantic denial as if inspection had been paid.
        assert!(matches!(
            admitted_public_metadata_skip(PUBLIC_ROOT, "owner-local", &mut b),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(b.test_work_remaining(), quote - 1);
        let mut b = budget();
        assert!(matches!(
            admitted_public_metadata_skip(PUBLIC_ROOT, "owner-local", &mut b),
            Err(Error::Denied(
                "reserved owner-local namespace in selected public identity prefix"
            ))
        ));
        assert!(public_metadata_step_work(usize::MAX, 1).is_none());
        assert!(public_metadata_step_work(1, usize::MAX).is_none());
    }
    #[test]
    fn selected_source_completion_keeps_owned_visitor_before_later_kernel_error() {
        // Error-transport DATA only; this constructs no owner/source/worker
        // adapter, selected path, readset, current permission or EOF carrier.
        let detail = String::from("first owned visitor detail");
        let pointer = detail.as_ptr();
        let result = complete::<()>(
            Err(DurableError::Conflict("later actual kernel exit")),
            Some(PgPrivateSelectedSourceCause::Visit(ItemRefusal::Source(
                detail,
            ))),
        );
        match result {
            Err(PgPrivateSelectedSourceFailure {
                original: PgPrivateSelectedSourceCause::Visit(ItemRefusal::Source(actual)),
                kernel_completion: Some(DurableError::Conflict("later actual kernel exit")),
            }) => assert_eq!(actual.as_ptr(), pointer),
            other => panic!("owned visitor detail was lost: {other:?}"),
        }
    }
    #[test]
    fn selected_source_completion_cannot_turn_retained_visitor_refusal_into_success() {
        let result = complete(
            Ok(Some(17)),
            Some(PgPrivateSelectedSourceCause::Visit(
                ItemRefusal::BudgetCheck {
                    check: "actual source visitor work",
                    used: Some(11),
                    limit: Some(10),
                },
            )),
        );
        assert!(matches!(
            result,
            Err(PgPrivateSelectedSourceFailure {
                original: PgPrivateSelectedSourceCause::Visit(ItemRefusal::BudgetCheck {
                    check: "actual source visitor work",
                    used: Some(11),
                    limit: Some(10),
                }),
                kernel_completion: None,
            })
        ));
    }
    #[test]
    fn selected_source_completion_requires_an_owned_outcome_and_keeps_actual_owner_error() {
        assert_eq!(complete(Ok(Some(17)), None).unwrap(), 17);
        assert!(matches!(
            complete::<()>(Ok(None), None),
            Err(PgPrivateSelectedSourceFailure {
                original: PgPrivateSelectedSourceCause::Owner(DurableError::Corrupt(
                    "private selected source owned outcome missing"
                )),
                ..
            })
        ));
        assert!(matches!(
            complete::<()>(
                Err(DurableError::Refused("first actual source refusal")),
                None
            ),
            Err(PgPrivateSelectedSourceFailure {
                original: PgPrivateSelectedSourceCause::Owner(DurableError::Refused(
                    "first actual source refusal"
                )),
                kernel_completion: None
            })
        ));
    }
}
