//! Reuses actual protected frames and the maintained evaluator. No bool grant,
//! report factory, per-callback budget reset or journal writer is introduced.
use super::{MAX_BYTES,canonical,invalid,unsupported};
use crate::{
    source_assessment_journal::{AssessmentHistory,AssessmentJournalFence,ProtectedAssessmentJournal},
    source_claims::ClaimCallBudget,
    source_command::{self as cmd,CommandContext,SourceCommandError,SourceCommandResult},
    source_creation_store::active,
    source_native_cli::private_assessment::{self,AssessmentEvaluationPlan,OperationBudget},
    source_private_assessment_sources::PrivateAssessmentSources,
    source_sign::OwnerAssessmentPublicSources,
    source_text_owner::OwnerTextContext,
};
use std::{cell::RefCell,collections::BTreeMap,rc::Rc,sync::atomic::AtomicBool,time::Instant};
use tos_foundation::{CutPreparedSchemaExecutionBinding,Digest256,JsonLimits,JsonMode,JsonValue,RelativePath,SourceRevision,parse_json_with_state_budget};
use tos_source_store::{CorpusCutReader,SoftwareCaptureReader,SoftwareComponentSelectionV1};
use tos_validation::{assessment::{AssessmentMechanicsReport,evaluate_current_assessment},source_cut::{CutSchemaExecutor,CutWorkerSchemaExecutor}};

pub(crate) struct HeldFrames<'a,'scope> {
    pub(super) owner:&'a OwnerTextContext,
    pub(super) context:&'a CommandContext,
    cut:&'a CorpusCutReader,
    grant:&'a ProtectedAssessmentJournal,
    private:&'a PrivateAssessmentSources,
    public:&'a mut OwnerAssessmentPublicSources<'scope>,
    fence:&'a AssessmentJournalFence<'scope>,
    histories:&'a BTreeMap<String,AssessmentHistory>,
    software:&'a SoftwareCaptureReader,
    components:&'a SoftwareComponentSelectionV1,
    binding:CutPreparedSchemaExecutionBinding,
    pub(super) worker:&'a mut CutWorkerSchemaExecutor,
    pub(super) budget:&'a mut OperationBudget,
    pub(super) whole:&'a Rc<RefCell<ClaimCallBudget>>,
    pub(super) deadline:Instant,
    pub(super) cancelled:&'a AtomicBool,
}
impl<'a,'scope> HeldFrames<'a,'scope> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(owner:&'a OwnerTextContext,context:&'a CommandContext,cut:&'a CorpusCutReader,
        grant:&'a ProtectedAssessmentJournal,private:&'a PrivateAssessmentSources,
        public:&'a mut OwnerAssessmentPublicSources<'scope>,fence:&'a AssessmentJournalFence<'scope>,
        histories:&'a BTreeMap<String,AssessmentHistory>,software:&'a SoftwareCaptureReader,
        components:&'a SoftwareComponentSelectionV1,worker:&'a mut CutWorkerSchemaExecutor,
        budget:&'a mut OperationBudget,whole:&'a Rc<RefCell<ClaimCallBudget>>,
        deadline:Instant,cancelled:&'a AtomicBool)->SourceCommandResult<Self> {
        let binding=worker.prepared_execution_binding();
        let mut frames=Self{binding,owner,context,cut,grant,private,public,fence,histories,software,components,worker,budget,whole,deadline,cancelled};
        frames.verify()?;Ok(frames)
    }
    pub(super) fn with_roles<T>(&mut self,registry:crate::source_native_roles::RoleRegistrySelection<'_>,
        use_roles:impl FnOnce(&mut crate::source_native_roles::NativeRoleResolver<'_>)->SourceCommandResult<T>)->SourceCommandResult<T> {
        self.verify()?;
        let mut resolver=crate::source_native_roles::NativeRoleResolver::new(self.context,self.cut,self.owner,self.private,
            self.worker,self.budget,registry,self.deadline,self.cancelled)?;
        let value=use_roles(&mut resolver)?;resolver.check_current()?;drop(resolver);
        self.verify()?;Ok(value)
    }
    pub(crate) fn revision(&self)->SourceRevision{self.context.base_revision}
    pub(crate) fn source_identity(&self)->Digest256{self.revision().0}
    pub(crate) fn source_path(&self,id:&str)->SourceCommandResult<&str> {
        match (self.public.record_paths.get(id),self.private.source_paths.get(id)) {
            (Some(a),Some(b)) if a!=b=>Err(SourceCommandError::Conflict("PWE selected owner source path collision")),
            (Some(path),_)|(_,Some(path))=>Ok(path.as_str()),
            _=>Err(unsupported("PWE actual source Claim path not selected")),
        }
    }
    pub(crate) fn verify(&mut self)->SourceCommandResult<()> {
        active(self.deadline,self.cancelled)?;
        self.budget.charge_work(1)?;
        // Charge the known retained source-byte envelope on every guard.
        // Configuration/transport metadata and software/schema work keep their
        // existing native owner budgets; this is not a physical-I/O theorem.
        let mut fence_bytes=self.private.input_bytes()?.checked_add(self.public.input_bytes()?)
            .and_then(|n|n.checked_add(self.grant.configuration_raw().len())).ok_or(unsupported("PWE fence byte overflow"))?;
        for file in &self.context.files {fence_bytes=fence_bytes.checked_add(file.raw.len()).ok_or(unsupported("PWE fence byte overflow"))?;}
        for history in self.histories.values(){fence_bytes=fence_bytes.checked_add(history.input_bytes()?).ok_or(unsupported("PWE history fence byte overflow"))?;}
        self.budget.charge_bytes(fence_bytes)?;
        self.whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?
            .read(u64::try_from(fence_bytes).map_err(|_|unsupported("PWE fence byte conversion"))?)?;
        self.context.check_from_selected_captures(self.cut,self.software,self.components,self.deadline,self.cancelled)?;
        if self.cut.current().revision()!=self.revision() || self.worker.source_revision()!=self.revision() || self.worker.prepared_execution_binding()!=self.binding {
            return Err(SourceCommandError::Conflict("PWE source/schema selected revision differs"));
        }
        self.grant.verify_current(self.deadline,self.cancelled)?;
        self.public.verify_current(self.deadline,self.cancelled)?;
        self.private.verify_current(self.owner,self.context,self.cut,self.deadline,self.cancelled)?;
        self.fence.verify_current(self.deadline,self.cancelled)?;
        for history in self.histories.values(){history.verify_current(self.fence,self.deadline,self.cancelled)?;}
        self.grant.verify_current(self.deadline,self.cancelled)?;
        active(self.deadline,self.cancelled)
    }
    /// Only bytes already selected by the protected owner. The C4 resolver may
    /// not turn an arbitrary locator into a filesystem read through this host.
    pub(crate) fn selected_bytes(&mut self,path:&str)->SourceCommandResult<Vec<u8>> {
        // Outer native operations guard their whole use before/after. Repeating
        // a whole-history scan for each dependency would be quadratic I/O.
        active(self.deadline,self.cancelled)?;
        let relative=RelativePath::parse(path).map_err(|_|invalid("PWE selected source path"))?;
        let public=self.context.file(&relative)?;
        let private=self.private.retained_raw.get(path).map(Vec::as_slice);
        let raw=match (public,private) {
            (Some(a),Some(b)) if a!=b=>return Err(SourceCommandError::Conflict("PWE selected source collision")),
            (Some(a),_)|(_,Some(a))=>a,
            _=>return Err(unsupported("PWE exact source path was not selected")),
        };
        if raw.len()>MAX_BYTES{return Err(unsupported("PWE source member byte cap"));}
        self.budget.charge_bytes(raw.len())?;
        let mut whole=self.whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?;
        whole.read(u64::try_from(raw.len()).map_err(|_|invalid("PWE source size conversion"))?)?;
        whole.check_live(raw.len())?;
        let mut copy=Vec::new();copy.try_reserve_exact(raw.len()).map_err(|_|unsupported("PWE source buffer reservation"))?;
        copy.extend_from_slice(raw);Ok(copy)
    }
    pub(crate) fn parse(&mut self,raw:&[u8])->SourceCommandResult<JsonValue> {
        active(self.deadline,self.cancelled)?;
        self.budget.charge_work(raw.len())?;
        let live=self.whole.try_borrow().map_err(|_|invalid("PWE whole-call budget in use"))?.remaining_live()?
            .checked_sub(raw.len()).ok_or(unsupported("PWE parser/input simultaneous holders"))?;
        parse_json_with_state_budget(raw,JsonMode::PublishedStrict,JsonLimits::new(MAX_BYTES,96,1_000_000,4096),live)
            .map(|doc|doc.into_root()).map_err(|_|invalid("PWE strict selected JSON"))
    }
    pub(crate) fn check_schema(&mut self,path:&str,record:&JsonValue,schema:&str)->SourceCommandResult<()> {
        if !schema.starts_with("ToS/contracts/") || !schema.ends_with(".schema.json")
            || self.worker.selected_contract_digest(schema).is_none(){return Err(unsupported("PWE actual schema is not in selected worker"));}
        self.whole.try_borrow().map_err(|_|invalid("PWE whole-call budget in use"))?.check_live(MAX_BYTES)?;
        let raw=canonical(record)?;
        self.budget.charge_bytes(raw.len())?;
        self.budget.charge_work(raw.len())?;
        match self.worker.check(path,&raw,schema,self.deadline,self.cancelled) {
            Ok(true)=>Ok(()),Ok(false)=>Err(invalid("PWE selected source schema refused")),
            Err(_)=>Err(unsupported("PWE selected source schema execution incomplete")),
        }
    }
    pub(crate) fn selected_document(&mut self,path:&str,schema:&str)->SourceCommandResult<JsonValue> {
        let raw=self.selected_bytes(path)?;let value=self.parse(&raw)?;
        self.check_schema(path,&value,schema)?;self.verify()?;Ok(value)
    }
    pub(crate) fn retain(&mut self,bytes:usize)->SourceCommandResult<()> {
        self.whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?.retain(bytes)
    }
    pub(crate) fn domain_value(&mut self,value:&JsonValue)->SourceCommandResult<serde_json::Value> {
        self.whole.try_borrow().map_err(|_|invalid("PWE whole-call budget in use"))?.check_live(MAX_BYTES)?;
        let raw=canonical(value)?;
        self.budget.charge_work(raw.len())?;self.budget.charge_bytes(raw.len())?;
        self.retain(raw.len().checked_mul(64).and_then(|n|n.checked_add(256)).ok_or(unsupported("PWE conversion holder overflow"))?)?;
        serde_json::from_slice(&raw).map_err(|_|invalid("PWE native/domain conversion"))
    }
    /// Evaluate the actual protected plan under the already held histories and
    /// existing worker/budget. No caller-supplied assessment boolean is read.
    pub(crate) fn assess(&mut self,plan:&AssessmentEvaluationPlan<'_>)->SourceCommandResult<AssessmentMechanicsReport> {
        self.verify()?;
        if !cmd::same(plan.config,self.grant.configuration())? {
            return Err(SourceCommandError::Conflict("PWE assessment protected configuration differs"));
        }
        let (current,scope)=private_assessment::select_current_scope(self.grant.configuration(),plan.request,&plan.all_by_id)?;
        if !cmd::same(&current,&plan.current)? || !cmd::same(&scope,&plan.scope)?
            || plan.source_route!=tos_validation::assessment::AssessmentSourceRoute::SourceBoundClaim {
            return Err(SourceCommandError::Conflict("PWE assessment source/current scope differs"));
        }
        for subject in &plan.lock_subjects {
            if !self.histories.contains_key(subject){return Err(invalid("PWE complete held assessment history absent"));}
        }
        let revision=self.revision();
        let mut evaluator=|input:&tos_validation::assessment::AssessmentReadInput,worker:&mut CutWorkerSchemaExecutor,
            limits:tos_validation::assessment::AssessmentLimits,cancelled:&AtomicBool| {
            evaluate_current_assessment(input,worker,limits,cancelled).map_err(private_assessment::map_assessment_refusal)
        };
        let result=private_assessment::evaluate_assessment_histories(plan,self.histories,&self.context.recorded_at,
            Some(revision),revision.0,self.worker,self.budget,self.cancelled,&mut evaluator)?;
        self.verify()?;Ok(result.current_report)
    }
}
