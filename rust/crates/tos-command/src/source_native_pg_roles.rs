//! Owner-issued PG formal-role evidence over the maintained read-only Record
//! kernel. Private constructors require an ACTUAL CurrentPgRoleOwner and PWE
//! retained views. Public metadata selection is necessary current permission;
//! it is not a qualified assessment or a PWE application-admission ticket.
use super::{
    NativeRole, RoleRegistrySelection, CurrentPgRoleOwner, PgRoleKernelSchema,
    admitted_document, admitted_canonical, check_pin, check_role_rows,
    check_resolved_role, reserve_reference_copies, wire_pin,
    ENTITY_SCHEMA, ROLE_SCHEMA,
};
use super::pg_kernel_schema::PgRoleControl;
use crate::pwe_source_owner::{PgPayloadMember, PgPayloadViews};
use crate::source_cohort::revision::{PgRecordRevisionIdentity, SourceRevisionIdentityClosure};
use crate::source_command::{self as cmd, SourceCommandError as Error, SourceCommandResult as Result, SourceFile};
use crate::source_revisions::{self, RecordRead, RecordSchemaWorker, ResolvedRecordVersion};
use crate::{DurableError, DurableResult};
use std::{collections::BTreeSet, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{ExactRecordRef, JsonDocument, RelativePath};
use tos_validation::item_rules::ItemLimits;

/// Exact mechanically resolved source/history/descriptor result. PG identity
/// remains PG identity, never its embedded filesystem bootstrap revision.
/// No public constructor, Clone, can_use flag or application ticket is supplied.
pub(crate) struct NativePgRoleRecord {
    role: NativeRole,
    exact: ExactRecordRef,
    kind_binding: ExactRecordRef,
    role_registry: ExactRecordRef,
    role_registry_path: String,
    selected: PgRecordRevisionIdentity,
    unique_owner: SourceRevisionIdentityClosure,
    resolved: ResolvedRecordVersion,
}
impl NativePgRoleRecord {
    pub(crate) fn role(&self) -> NativeRole { self.role }
    pub(crate) fn exact(&self) -> &ExactRecordRef { &self.exact }
    pub(crate) fn kind_binding(&self) -> &ExactRecordRef { &self.kind_binding }
    pub(crate) fn role_registry(&self) -> &ExactRecordRef { &self.role_registry }
    pub(crate) fn role_registry_path(&self) -> &str { &self.role_registry_path }
    pub(crate) fn input_identity(&self) -> &PgRecordRevisionIdentity { &self.selected }
    pub(crate) fn evidence(&self) -> &ResolvedRecordVersion { &self.resolved }

    /// Necessary LIVE source/current-selection fence for cached role evidence.
    /// The PWE owner must independently obtain actual assessment/account/use
    /// admission before application. This method never issues that authority.
    pub(crate) fn check_current_use(
        &self, current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    ) -> DurableResult<()> {
        let body = (|| {
            current.check_current()?;
            current.with_source_epoch(|_epoch, payer, _same| {
                payer.inspect(0, size_of::<PgRecordRevisionIdentity>()).map_err(DurableError::Source)
            })?;
            if current.input_identity() != &self.selected {
                return Err(DurableError::Conflict("PG formal role belongs to another selected epoch"));
            }
            current.require_public_record(self.exact.id.as_str(), &self.resolved.source_path)?;
            current.verify_identity_closure(&self.unique_owner)
        })();
        let exit = current.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

/// Original strict selected current/retained bytes, not serialization of a
/// resolved JsonValue. The record and document are issued together below.
pub(crate) struct NativePgRoleDocument {
    record: NativePgRoleRecord,
    document: JsonDocument,
}
impl NativePgRoleDocument {
    pub(crate) fn record(&self) -> &NativePgRoleRecord { &self.record }
    pub(crate) fn document(&self) -> &JsonDocument { &self.document }
}

// Native RecordRead and schema operations share only sequential access to the
// same private owner cell. Bytes below are immutable genuine PWE member loans.
// This type is private: caller buffers/getters cannot construct an issuer.
struct SelectedRoleRead<'payload, 'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
    members: Vec<PgPayloadMember<'payload>>,
    control: PgRoleControl<'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    selected: PgRecordRevisionIdentity,
    uid: u64,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
}
impl RecordRead for SelectedRoleRead<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_> {
    type Identity = PgRecordRevisionIdentity;
    // Global identity discovery MUST use the owner's complete five-kind point
    // lookup. This reader is only passed to the maintained selected-path entry.
    fn files(&self) -> &[SourceFile] { &[] }
    fn input_identity(&self) -> Self::Identity { self.selected }
    fn effective_uid(&self) -> u64 { self.uid }
    fn schema_source_path(&self) -> Result<String> {
        self.control.with_operation(|budget| {
            budget.charge_bytes(ENTITY_SCHEMA.len())?;
            budget.charge_work(ENTITY_SCHEMA.len())?;
            Ok(ENTITY_SCHEMA.to_owned())
        })
    }
    fn check(&self) -> Result<()> {
        self.control.with_owner(|owner| owner.check_current())
    }
    fn visit_files(&self, visit: &mut dyn FnMut(&RelativePath, &[u8]) -> Result<()>) -> Result<()> {
        self.check()?;
        for member in &self.members {
            self.control.with_operation(|budget| budget.charge_work(1))?;
            visit(&member.source_member().path, member.raw())?;
        }
        Ok(())
    }
    fn file(&self, path: &RelativePath) -> Result<Option<&[u8]>> {
        self.check()?;
        for member in &self.members {
            let actual = &member.source_member().path;
            self.control.with_operation(|budget| {
                let bytes = path.as_str().len().checked_add(actual.as_str().len())
                    .ok_or(Error::Unsupported("PG role selected lookup overflow"))?;
                budget.charge_work(bytes.checked_add(1).ok_or(Error::Unsupported("PG role selected lookup overflow"))?)
            })?;
            if actual == path { return Ok(Some(member.raw())); }
        }
        // Omitted selected bytes NEVER mean absence. Query the actual owner
        // and record its absence predicate; if present, require an explicit
        // complete retained loan instead of a latest/fetch/fabricated fallback.
        let present = self.control.with_owner(|owner| owner.has_current_metadata(path))?;
        if present { Err(Error::Unsupported("PG role required current member is outside retained closure")) }
        else { Ok(None) }
    }
}

fn overflow() -> Error { Error::Unsupported("PG formal-role source accounting overflow") }

/// Execute the unchanged native exact/history/profile/descriptor kernels on
/// the real protected owner. All supplied paths must already be in its ACTUAL
/// retained loan. A subset can fail closed; it cannot certify missing history.
///
/// Full parser/package/canonical/BTree/driver activation funding remains a
/// separate acceptance obligation. This source ABI does not claim native fit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_native_pg_role<'payload, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>(
    current: &mut CurrentPgRoleOwner<'_, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    views: &PgPayloadViews<'payload, 'budget, 'cancel>,
    paths: &BTreeSet<RelativePath>,
    registry: RoleRegistrySelection<'_>,
    role: NativeRole,
    kind_binding: &ExactRecordRef,
    exact: &ExactRecordRef,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
) -> DurableResult<NativePgRoleDocument> {
    let body = (|| {
        current.check_current()?;
        if paths.is_empty() || paths.len() > 64 {
            return Err(DurableError::Refused("PG formal-role selected closure count"));
        }
        let unique_owner = current.identity_closure(exact.id.as_str())?;
        // One requested identity, complete five-kind lookup. A Claim/form/event
        // alias is not a metadata owner, even if its bare string is equal.
        current.with_source_epoch(|_epoch, payer, _same| {
            payer.inspect(size_of::<NativePgRoleDocument>() + size_of::<ResolvedRecordVersion>(),
                exact.id.as_str().len().checked_mul(2).ok_or(DurableError::Source(overflow()))?)
                .map_err(DurableError::Source)
        })?;
        let owner_path = match unique_owner.owners().get(exact.id.as_str()) {
            Some(Some((kind, path))) if kind == "metadata" => path,
            _ => return Err(DurableError::Conflict("PG formal role has no unique metadata owner")),
        };
        current.require_public_record(exact.id.as_str(), owner_path.as_str())?;
        current.verify_identity_closure(&unique_owner)?;
        let backing = paths.len().checked_mul(size_of::<PgPayloadMember<'_>>())
            .ok_or(DurableError::Source(overflow()))?;
        current.with_source_epoch(|_epoch, payer, same| {
            same.retain_source_state(backing).map_err(|error| DurableError::Source(payer.error(error)))?;
            payer.reserve(backing).map_err(DurableError::Source)
        })?;
        let mut members = Vec::new();
        members.try_reserve_exact(paths.len()).map_err(|_| DurableError::Refused("PG role borrowed member backing"))?;
        for path in paths { members.push(current.selected_member(views, path)?); }
        let selected = *current.input_identity();
        let uid = current.account_uid();
        let result = super::with_pg_role_kernel_schema(current, deadline, cancelled, |worker| {
            let ctx = SelectedRoleRead { members, control: worker.control(), selected, uid, deadline, cancelled };
            resolve_selected(&ctx, worker, &registry, role, kind_binding, exact, owner_path.as_str())
        })?;
        current.verify_identity_closure(&unique_owner)?;
        let (resolved, document) = result;
        // Source immutable loans and generic getters cannot create this type:
        // the constructor follows actual owner permission, global uniqueness,
        // full retained semantic checks and closed schema execution above.
        Ok(NativePgRoleDocument {
            record: NativePgRoleRecord {
                role, exact: exact.clone(), kind_binding: kind_binding.clone(),
                role_registry: registry.exact.clone(), role_registry_path: registry.path.to_owned(),
                selected, unique_owner, resolved,
            }, document,
        })
    })();
    let exit = current.check_current();
    match (body, exit) {
        (Err(original), _) | (Ok(_), Err(original)) => Err(original),
        (Ok(value), Ok(())) => Ok(value),
    }
}

fn resolve_selected(
    ctx: &SelectedRoleRead<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    worker: &mut PgRoleKernelSchema<'_, '_, '_, '_, '_, '_, '_, '_, '_>,
    registry: &RoleRegistrySelection<'_>, role: NativeRole,
    kind_binding: &ExactRecordRef, exact: &ExactRecordRef, owner_path: &str,
) -> Result<(ResolvedRecordVersion, JsonDocument)> {
    let entities = source_revisions::validate_source_profile_registry_for_worker(
        worker, ctx.deadline, ctx.cancelled, ctx,
    )?;
    ctx.control.with_operation(|budget| check_pin(
        &entities, kind_binding, "registry_id", "registry_version", budget, ctx.deadline, ctx.cancelled,
    ))?;
    if !registry.path.starts_with("ToS/doctrine/semantic-interchange/") || !registry.path.ends_with(".json") {
        return Err(Error::Invalid("native PG role registry source home"));
    }
    let registry_path = RelativePath::parse(registry.path).map_err(|_| Error::Invalid("PG role registry path"))?;
    let registry_raw = ctx.file(&registry_path)?.ok_or(Error::Unsupported("PG role registry not retained"))?;
    let registry_doc = ctx.control.with_operation(|budget| admitted_document(registry_raw, budget, ctx.deadline, ctx.cancelled))?;
    let bindings = registry_doc.root();
    ctx.control.with_operation(|budget| check_pin(
        bindings, registry.exact, "registry_id", "registry_version", budget, ctx.deadline, ctx.cancelled,
    ))?;
    source_revisions::schema_for_source_path_worker(worker, ctx.deadline, ctx.cancelled, ctx,
        &[ROLE_SCHEMA.into(), ENTITY_SCHEMA.into()], ROLE_SCHEMA, registry.path, bindings)?;
    ctx.control.with_operation(|budget| {
        let work = cmd::array(bindings, "role_bindings")?.len()
            .checked_mul(cmd::array(&entities, "types")?.len()).ok_or_else(overflow)?;
        budget.charge_work(work)?;
        check_role_rows(bindings, &entities)
    })?;
    // package() only visits retained members. Prove the target history's actual
    // absence, or require its genuine selected bytes, BEFORE that visit.
    let (parent, _) = owner_path.rsplit_once('/').ok_or(Error::Invalid("PG role owner parent"))?;
    let history_len = parent.len().checked_add(1 + source_revisions::HISTORY.len()).ok_or_else(overflow)?;
    ctx.control.with_operation(|budget| { budget.charge_bytes(history_len)?; budget.charge_work(history_len) })?;
    let history = RelativePath::parse(&format!("{parent}/{}", source_revisions::HISTORY))
        .map_err(|_| Error::Invalid("PG role history path"))?;
    let _history = ctx.file(&history)?;
    let (limits, pin) = ctx.control.with_operation(|budget| {
        reserve_reference_copies(budget, &[exact, kind_binding, registry.exact], registry.path)?;
        budget.charge_bytes(exact.id.as_str().len().checked_add(71).ok_or_else(overflow)?)?;
        let remaining = budget.limits()?;
        let cap = remaining.batch.max_total_raw_bytes.min(remaining.max_input_bytes).min(remaining.max_work);
        if cap == 0 { return Err(Error::Unsupported("PG role exact reader remaining allowance")); }
        Ok((ItemLimits { max_member_bytes: cap, max_total_bytes: cap as u64,
            max_state_bytes: cap, max_issues: 256, deadline: ctx.deadline.min(remaining.deadline) }, wire_pin(exact)))
    })?;
    let mut resolved = source_revisions::resolve_record_version_evidence_at_selected(
        ctx, owner_path, limits, &pin, worker, ctx.deadline, ctx.cancelled,
    )?;
    ctx.control.with_operation(|budget| check_pin(
        &resolved.record, exact, "record_id", "record_version", budget, ctx.deadline, ctx.cancelled,
    ))?;
    let descriptor = resolved.readonly_descriptor(&entities)?;
    check_resolved_role(bindings, role, &resolved.record, &descriptor)?;
    ctx.control.with_operation(|budget| {
        drop(admitted_canonical(&resolved.record, budget, ctx.deadline, ctx.cancelled)?);
        drop(admitted_canonical(&descriptor, budget, ctx.deadline, ctx.cancelled)?);
        Ok(())
    })?;
    let locator = match resolved.version_status {
        "current" => owner_path,
        "historical" => cmd::text(&resolved.source, "archive_blob_ref")?,
        _ => return Err(Error::Unsupported("PG role exact source locator route")),
    };
    let locator = RelativePath::parse(locator).map_err(|_| Error::Invalid("PG role document locator"))?;
    let raw = ctx.file(&locator)?.ok_or(Error::Unsupported("PG role exact document not retained"))?;
    let document = ctx.control.with_operation(|budget| {
        let document = admitted_document(raw, budget, ctx.deadline, ctx.cancelled)?;
        check_pin(document.root(), exact, "record_id", "record_version", budget, ctx.deadline, ctx.cancelled)?;
        Ok(document)
    })?;
    resolved.descriptor = Some(descriptor);
    Ok((resolved, document))
}
