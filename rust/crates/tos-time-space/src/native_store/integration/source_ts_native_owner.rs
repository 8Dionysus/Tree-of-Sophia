//! Included by the common command crate through the separately supplied hunk.
//! This file is TS-owned; the real document reader/constructors stay common-owned.
//! No public wire constructor, source grant, completion flag or fake host exists.
use crate::{
    source_command::{
        self as cmd, CommandContext, SourceCommandError as Error, SourceCommandResult as Result,
    },
    source_native_cli::private_assessment::OperationBudget,
    source_native_roles::{
        NativeRole, NativeRoleDocument, NativeRoleResolver, RoleRegistrySelection,
        admitted_document, check_pin,
    },
    source_private_assessment_sources::PrivateAssessmentSources,
    source_revisions,
    source_text_owner::OwnerTextContext,
};
use std::{
    cell::{Ref, RefCell},
    mem::size_of,
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::{
    Digest256, ExactRecordRef, FoundationError, FoundationErrorCode, JsonDocument, JsonValue,
    RelativePath,
};
use tos_source_store::CorpusCutReader;
use tos_time_space::{
    DomainError,
    native_store::{
        NativeTsDescriptor, NativeTsRequest, NativeTsRole, NativeTsSelection, NativeTsSession,
        NativeTsSourceOwner, SelectedNativeTsRecord,
    },
};
use tos_validation::{item_rules::ItemLimits, source_cut::CutWorkerSchemaExecutor};

const ENTITY_PATH: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const REGISTRY_SCHEMA: &str = "ToS/contracts/ts-native-bindings.schema.json";
const ROLE_SCHEMA: &str = "ToS/contracts/native-role-bindings.schema.json";
const ENTITY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const PATHS: [&str; 3] = [
    "ToS/contracts/ts-native-definition.schema.json",
    "ToS/contracts/ts-native-value-profile.schema.json",
    "ToS/contracts/ts-native-operation-result.schema.json",
];

struct SchemaInput {
    document: JsonDocument,
    raw_digest: Digest256,
    lookup_work: usize,
}
struct HeldDocument {
    native: NativeRoleDocument,
    role: NativeTsRole,
    resource: usize,
    scan_work: usize,
}

/// Owns genuinely resolved original documents and their actual live owner
/// frames. The temporary resolver releases mutable worker/ledger borrows after
/// preparation; later phases borrow the SAME parent ledger, never a fresh one.
pub(crate) struct NativeTsHeldOwner<'frames, 'ts> {
    ctx: &'frames CommandContext,
    cut: &'frames CorpusCutReader,
    owner: &'frames OwnerTextContext,
    sources: &'frames PrivateAssessmentSources,
    parent: &'frames RefCell<OperationBudget>,
    deadline: Instant,
    cancelled: &'frames AtomicBool,
    selection: NativeTsSelection,
    selection_document: JsonDocument,
    selection_evidence: source_revisions::ResolvedRecordVersion,
    registry_document: JsonDocument,
    schemas: Vec<SchemaInput>,
    documents: Vec<HeldDocument>,
    ledger: &'ts RefCell<NativeTsSession<'ts>>,
    last_refusal: RefCell<Option<Error>>,
}
fn role_index(role: NativeTsRole) -> usize {
    match role {
        NativeTsRole::Definition => 0,
        NativeTsRole::ValueProfile => 1,
        NativeTsRole::OperationResult => 2,
    }
}
fn native_role(role: NativeTsRole) -> NativeRole {
    match role {
        NativeTsRole::Definition => NativeRole::TsDefinition,
        NativeTsRole::ValueProfile => NativeRole::TsValueProfile,
        NativeTsRole::OperationResult => NativeRole::TsOperationResult,
    }
}
fn ts_error(_: DomainError) -> Error {
    Error::Unsupported("original TS allowance or exact binding refused")
}
fn source_raw<'a>(
    ctx: &'a CommandContext,
    path: &str,
    budget: &mut OperationBudget,
) -> Result<&'a [u8]> {
    if !path.starts_with("ToS/") {
        return Err(Error::Invalid("TS authored input namespace"));
    }
    budget.charge_work(
        ctx.files
            .len()
            .checked_add(1)
            .ok_or(Error::Unsupported("TS source file count overflow"))?
            .checked_mul(
                path.len()
                    .checked_add(1)
                    .ok_or(Error::Unsupported("TS source path work overflow"))?,
            )
            .ok_or(Error::Unsupported("TS selected source scan overflow"))?,
    )?;
    budget.charge_bytes(path.len())?;
    let path = RelativePath::parse(path).map_err(|_| Error::Invalid("TS selected input path"))?;
    ctx.file(&path)?.ok_or(Error::Unsupported(
        "TS required actual source input is not selected",
    ))
}
fn strict_input(
    ctx: &CommandContext,
    path: &str,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<JsonDocument> {
    let raw = source_raw(ctx, path, budget)?;
    admitted_document(raw, budget, deadline, cancelled)
}
fn schema_input(
    ctx: &CommandContext,
    worker: &CutWorkerSchemaExecutor,
    path: &str,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SchemaInput> {
    let raw = source_raw(ctx, path, budget)?;
    budget.charge_work(raw.len())?;
    let digest = Digest256::of_bytes(raw);
    let mut lookup_error = None;
    let selected = {
        let mut admit = |bytes: usize, visits: usize| {
            let work = bytes.checked_add(visits).ok_or_else(|| {
                FoundationError::new(
                    FoundationErrorCode::BudgetExceeded,
                    "TS schema lookup work overflow",
                )
            })?;
            if let Err(error) = budget.charge_work(work) {
                lookup_error = Some(error);
                return Err(FoundationError::new(
                    FoundationErrorCode::BudgetExceeded,
                    "TS schema lookup allowance",
                ));
            }
            Ok(())
        };
        worker.contract_digest_with_admission(path, &mut admit)
    };
    let selected = selected
        .map_err(|_| lookup_error.unwrap_or(Error::Unsupported("TS admitted schema lookup")))?;
    if selected != Some(digest) {
        return Err(Error::Conflict(
            "TS schema is not the actual prepared resource",
        ));
    }
    let document = admitted_document(raw, budget, deadline, cancelled)?;
    budget.charge_work(raw.len())?;
    let uri = cmd::text(document.root(), "$id")?;
    // Compare with the agreed resource home; RETURN the actual source $id.
    budget.charge_work(path.len())?;
    budget.charge_bytes(path.len() + 32)?;
    if uri != format!("https://tree-of-sophia.local/{path}") {
        return Err(Error::Conflict("TS native schema source resource identity"));
    }
    Ok(SchemaInput {
        document,
        raw_digest: digest,
        lookup_work: raw.len(),
    })
}
impl<'frames, 'ts> NativeTsHeldOwner<'frames, 'ts> {
    /// The protected caller supplies its ACTUAL source selection record/path.
    /// Its meaning/requested use must already be selected by that caller. This
    /// adapter verifies existing source content; it invents no selection record.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        ctx: &'frames CommandContext,
        cut: &'frames CorpusCutReader,
        owner: &'frames OwnerTextContext,
        sources: &'frames PrivateAssessmentSources,
        worker: &mut CutWorkerSchemaExecutor,
        parent: &'frames RefCell<OperationBudget>,
        selection_path: &RelativePath,
        selection_exact: &ExactRecordRef,
        kind_binding: &ExactRecordRef,
        registry: RoleRegistrySelection<'_>,
        requests: &[NativeTsRequest<'_>],
        ledger: &'ts RefCell<NativeTsSession<'ts>>,
        deadline: Instant,
        cancelled: &'frames AtomicBool,
    ) -> Result<Self> {
        let result = match parent.try_borrow_mut() {
            Ok(mut budget) => Self::prepare_inner(
                ctx,
                cut,
                owner,
                sources,
                worker,
                &mut budget,
                parent,
                selection_path,
                selection_exact,
                kind_binding,
                registry,
                requests,
                ledger,
                deadline,
                cancelled,
            ),
            Err(_) => Err(Error::Unsupported(
                "original native TS parent ledger borrowed",
            )),
        };
        // Actual after-Err fence, including setup failures; preserve first error.
        let fenced = sources.verify_current(owner, ctx, cut, deadline, cancelled);
        match (result, fenced) {
            (Err(e), _) | (Ok(_), Err(e)) => Err(e),
            (Ok(v), Ok(())) => Ok(v),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_inner(
        ctx: &'frames CommandContext,
        cut: &'frames CorpusCutReader,
        owner: &'frames OwnerTextContext,
        sources: &'frames PrivateAssessmentSources,
        worker: &mut CutWorkerSchemaExecutor,
        parent: &mut OperationBudget,
        parent_cell: &'frames RefCell<OperationBudget>,
        selection_path: &RelativePath,
        selection_exact: &ExactRecordRef,
        kind_binding: &ExactRecordRef,
        registry: RoleRegistrySelection<'_>,
        requests: &[NativeTsRequest<'_>],
        ledger: &'ts RefCell<NativeTsSession<'ts>>,
        deadline: Instant,
        cancelled: &'frames AtomicBool,
    ) -> Result<Self> {
        sources.verify_current(owner, ctx, cut, deadline, cancelled)?;
        // Reserve from the EXISTING parent before any TS work, not after return.
        // No new OperationBudget, cap increase or refund. Native reader/worker
        // work remains additionally paid by that same parent through C4.
        let remaining = {
            let ts = ledger
                .try_borrow()
                .map_err(|_| Error::Unsupported("TS ledger borrowed"))?;
            if !std::ptr::eq(ts.cancellation_flag(), cancelled) {
                return Err(Error::Conflict(
                    "TS and native original cancellation identities differ",
                ));
            }
            ts.remaining_allowance().map_err(ts_error)?
        };
        let deadline = deadline.min(remaining.deadline);
        parent.charge_work(remaining.domain.max_work)?;
        parent.charge_bytes(remaining.domain.max_requested_bytes)?;
        if worker.source_revision() != ctx.base_revision
            || cut.current().revision() != ctx.base_revision
        {
            return Err(Error::Conflict("TS actual cut/worker selection differs"));
        }
        let selection_document =
            strict_input(ctx, selection_path.as_str(), parent, deadline, cancelled)?;
        check_pin(
            selection_document.root(),
            selection_exact,
            "record_id",
            "record_version",
            parent,
            deadline,
            cancelled,
        )?;
        let wire_cost = selection_exact
            .id
            .as_str()
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(2048))
            .ok_or(Error::Unsupported("TS selection pin state overflow"))?;
        parent.charge_work(wire_cost)?;
        parent.charge_bytes(wire_cost)?;
        let pin = cmd::object(vec![
            ("id", cmd::string(selection_exact.id.as_str())),
            ("version", cmd::number(selection_exact.version.get())),
            ("digest", cmd::string(&selection_exact.digest.to_prefixed())),
        ]);
        let available = parent.limits()?;
        let read_cap = available
            .batch
            .max_total_raw_bytes
            .min(available.max_work / 4);
        if read_cap == 0 {
            return Err(Error::Unsupported("TS selection exact reader allowance"));
        }
        let selection_evidence = source_revisions::resolve_role_record_version_from_cut(
            ctx,
            cut,
            &pin,
            ItemLimits {
                max_member_bytes: available.max_input_bytes.min(read_cap),
                max_total_bytes: read_cap as u64,
                max_state_bytes: available.max_input_bytes.min(read_cap),
                max_issues: 256,
                deadline: deadline.min(available.deadline),
            },
            worker,
            cancelled,
        )?;
        parent.charge_bytes(
            usize::try_from(selection_evidence.bytes_read)
                .map_err(|_| Error::Unsupported("TS selection read count overflow"))?,
        )?;
        parent.charge_work(
            usize::try_from(selection_evidence.bytes_read)
                .ok()
                .and_then(|n| n.checked_mul(4))
                .ok_or(Error::Unsupported("TS selection read work overflow"))?,
        )?;
        parent.charge_bytes(selection_evidence.returned_state_bytes)?;
        // The selection input is supported only as the ACTUAL current Record in
        // this cut. Historical formal documents use C4 below. No latest fallback.
        parent.charge_work(selection_path.as_str().len())?;
        if selection_evidence.version_status != "current"
            || selection_evidence.source_path != selection_path.as_str()
            || cut.current().member(selection_path).is_none()
        {
            return Err(Error::Unsupported(
                "TS current selection Record is not in the actual cut",
            ));
        }
        let entities = strict_input(ctx, ENTITY_PATH, parent, deadline, cancelled)?;
        check_pin(
            entities.root(),
            kind_binding,
            "registry_id",
            "registry_version",
            parent,
            deadline,
            cancelled,
        )?;
        let registry_document = strict_input(ctx, registry.path, parent, deadline, cancelled)?;
        check_pin(
            registry_document.root(),
            registry.exact,
            "registry_id",
            "registry_version",
            parent,
            deadline,
            cancelled,
        )?;
        let registry_schema =
            schema_input(ctx, worker, REGISTRY_SCHEMA, parent, deadline, cancelled)?;
        source_revisions::schema_for_source_path_worker(
            worker,
            deadline,
            cancelled,
            ctx,
            &[
                REGISTRY_SCHEMA.into(),
                ROLE_SCHEMA.into(),
                ENTITY_SCHEMA.into(),
            ],
            REGISTRY_SCHEMA,
            registry.path,
            registry_document.root(),
        )?;
        drop(registry_schema);
        let mut schemas = Vec::new();
        parent.charge_bytes(3 * size_of::<SchemaInput>())?;
        schemas
            .try_reserve_exact(3)
            .map_err(|_| Error::Unsupported("TS schema holder state"))?;
        for path in PATHS {
            schemas.push(schema_input(
                ctx, worker, path, parent, deadline, cancelled,
            )?);
        }
        let copy_bytes = selection_exact
            .id
            .as_str()
            .len()
            .checked_add(kind_binding.id.as_str().len())
            .and_then(|n| n.checked_add(registry.exact.id.as_str().len()))
            .ok_or(Error::Unsupported("TS selected reference copy overflow"))?;
        parent.charge_bytes(copy_bytes)?;
        parent.charge_work(copy_bytes)?;
        let selection = NativeTsSelection {
            selection_ref: selection_exact.clone(),
            source_revision: ctx.base_revision,
            input_identity: ctx.base_revision.0,
            entity_registry_ref: kind_binding.clone(),
            role_registry_ref: registry.exact.clone(),
            schema_execution: worker.prepared_execution_binding(),
        };
        let mut resolver = NativeRoleResolver::new(
            ctx, cut, owner, sources, worker, parent, registry, deadline, cancelled,
        )?;
        let mut documents: Vec<HeldDocument> = Vec::new();
        for request in requests {
            let mut ts = ledger
                .try_borrow_mut()
                .map_err(|_| Error::Unsupported("TS ledger borrowed"))?;
            ts.item().map_err(ts_error)?;
            ts.work(request.exact.id.as_str().len() + 40)
                .map_err(ts_error)?;
            for prior in &documents {
                ts.work(prior.native.record().exact().id.as_str().len() + 40)
                    .map_err(ts_error)?;
                if prior.native.record().exact() == request.exact {
                    return Err(Error::Invalid("duplicate TS held exact request"));
                }
            }
            // Actual C4 original raw/current/archive/schema/history route.
            let native = resolver.resolve_document(
                native_role(request.role),
                kind_binding,
                request.exact,
            )?;
            if native.record().source_revision() != selection.source_revision
                || native.record().kind_binding() != &selection.entity_registry_ref
                || native.record().role_registry() != &selection.role_registry_ref
            {
                return Err(Error::Conflict(
                    "TS resolved native role/source pins differ",
                ));
            }
            let resource = role_index(request.role);
            let evidence = native.record().evidence();
            let raw_size = usize::try_from(cmd::integer(&evidence.source, "record_bytes")?)
                .map_err(|_| Error::Unsupported("TS native original byte count"))?;
            // TS source counts describe retained input only. C4's strict raw
            // reread pays parent prefixes; the maintained foreign-reader C3-W
            // debt remains separate and is not justified by this return fee.
            ts.source_read(1, raw_size).map_err(ts_error)?;
            let scan_work = raw_size
                .checked_mul(8)
                .ok_or(Error::Unsupported("TS native metadata work overflow"))?;
            ts.work(scan_work).map_err(ts_error)?;
            let descriptor = evidence
                .descriptor
                .as_ref()
                .ok_or(Error::Unsupported("TS native descriptor absent"))?;
            if cmd::text(descriptor, "source_schema_ref")? != PATHS[resource]
                || cmd::text(native.document().root(), "record_type")?
                    != [
                        "ts-native-definition",
                        "ts-native-value-profile",
                        "ts-native-operation-result",
                    ][resource]
            {
                return Err(Error::Conflict(
                    "TS actual role descriptor/resource differs",
                ));
            }
            ts.requested_bytes(size_of::<HeldDocument>())
                .map_err(ts_error)?;
            ts.work(documents.len() + 1).map_err(ts_error)?;
            documents
                .try_reserve_exact(1)
                .map_err(|_| Error::Unsupported("TS native document cache state"))?;
            documents.push(HeldDocument {
                native,
                role: request.role,
                resource,
                scan_work,
            });
        }
        resolver.check_current()?;
        drop(resolver);
        Ok(Self {
            ctx,
            cut,
            owner,
            sources,
            parent: parent_cell,
            deadline,
            cancelled,
            selection,
            selection_document,
            selection_evidence,
            registry_document,
            schemas,
            documents,
            ledger,
            last_refusal: RefCell::new(None),
        })
    }
    pub(crate) fn selection(&self) -> &NativeTsSelection {
        &self.selection
    }
    pub(crate) fn selection_document(&self) -> &JsonDocument {
        &self.selection_document
    }
    pub(crate) fn selection_evidence(&self) -> &source_revisions::ResolvedRecordVersion {
        &self.selection_evidence
    }
    pub(crate) fn role_registry_document(&self) -> &JsonDocument {
        &self.registry_document
    }
    pub(crate) fn last_source_refusal(&self) -> Result<Ref<'_, Option<Error>>> {
        self.last_refusal
            .try_borrow()
            .map_err(|_| Error::Unsupported("TS source refusal currently borrowed"))
    }
    pub(crate) fn schema_raw_digest(&self, role: NativeTsRole) -> Digest256 {
        self.schemas[role_index(role)].raw_digest
    }
    fn native_fence(&self) -> std::result::Result<(), DomainError> {
        let result = (|| {
            self.parent
                .try_borrow()
                .map_err(|_| Error::Unsupported("original native TS parent ledger borrowed"))?
                .limits()?;
            if self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
                || Instant::now() >= self.deadline
            {
                return Err(Error::Denied("native TS operation cancelled or expired"));
            }
            self.sources.verify_current(
                self.owner,
                self.ctx,
                self.cut,
                self.deadline,
                self.cancelled,
            )
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                let mut first = self
                    .last_refusal
                    .try_borrow_mut()
                    .map_err(|_| DomainError::ResourceLimit)?;
                if first.is_none() {
                    *first = Some(error);
                }
                Err(DomainError::NeedsBinding(
                    "actual native TS source fence refused; original error retained",
                ))
            }
        }
    }
    fn same_selection(
        &self,
        selection: &NativeTsSelection,
        ts: &mut NativeTsSession<'_>,
    ) -> std::result::Result<(), DomainError> {
        if !std::ptr::eq(ts as *const _, self.ledger.as_ptr() as *const _) {
            return Err(DomainError::Incompatible("TS original ledger identity"));
        }
        ts.check()?;
        for r in [
            &selection.selection_ref,
            &selection.entity_registry_ref,
            &selection.role_registry_ref,
            &self.selection.selection_ref,
            &self.selection.entity_registry_ref,
            &self.selection.role_registry_ref,
        ] {
            ts.work(r.id.as_str().len() + 40)?;
        }
        if selection != &self.selection {
            return Err(DomainError::Incompatible(
                "TS held exact selection/schema binding",
            ));
        }
        Ok(())
    }
}
impl NativeTsSourceOwner for NativeTsHeldOwner<'_, '_> {
    fn verify_current(
        &self,
        selection: &NativeTsSelection,
        ts: &mut NativeTsSession<'_>,
    ) -> std::result::Result<(), DomainError> {
        let before = self.native_fence();
        let result = before.and_then(|_| self.same_selection(selection, ts));
        let after = self.native_fence();
        match (result, after) {
            (Err(e), _) | (Ok(_), Err(e)) => Err(e),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
    fn resolve<'a>(
        &'a self,
        selection: &NativeTsSelection,
        request: NativeTsRequest<'_>,
        ts: &mut NativeTsSession<'_>,
    ) -> std::result::Result<SelectedNativeTsRecord<'a>, DomainError> {
        let result = (|| {
            self.verify_current(selection, ts)?;
            ts.work(request.exact.id.as_str().len() + 40)?;
            for held in &self.documents {
                ts.work(held.native.record().exact().id.as_str().len() + 40)?;
                let exact = held.native.record().exact();
                if exact.id != request.exact.id || exact.version != request.exact.version {
                    continue;
                }
                if exact.digest != request.exact.digest {
                    return Err(DomainError::Incompatible("TS held exact digest"));
                }
                if held.role != request.role {
                    return Err(DomainError::Incompatible("TS held exact role"));
                }
                ts.work(held.scan_work)?;
                ts.work(self.schemas[held.resource].lookup_work)?;
                let root = held.native.document().root();
                let string =
                    |v: &'a JsonValue, key: &str| -> std::result::Result<&'a str, DomainError> {
                        cmd::text(v, key)
                            .map_err(|_| DomainError::Invalid("TS native held descriptor string"))
                    };
                return Ok(SelectedNativeTsRecord {
                    document: held.native.document(),
                    selection: &self.selection,
                    descriptor: NativeTsDescriptor {
                        record_type: string(root, "record_type")?,
                        schema_version: string(root, "schema_version")?,
                        schema_resource: string(
                            self.schemas[held.resource].document.root(),
                            "$id",
                        )?,
                    },
                });
            }
            Err(DomainError::NeedsBinding(
                "exact native TS document not prepared; no absence inference",
            ))
        })();
        let after = self.native_fence();
        match (result, after) {
            (Err(e), _) | (Ok(_), Err(e)) => Err(e),
            (Ok(v), Ok(())) => Ok(v),
        }
    }
}
