//! Reuses actual protected frames and the maintained evaluator. No bool grant,
//! report factory, per-callback budget reset or journal writer is introduced.
use super::access::BorrowedAccess;
use super::budget::{CanonicalLedger, FrameBudget};
use super::{MAX_BYTES, invalid, unsupported};
use crate::{
    source_assessment_journal::{
        AssessmentHistory, AssessmentJournalFence, ProtectedAssessmentJournal,
    },
    source_claims::ClaimCallBudget,
    source_command::{CommandContext, SourceCommandError, SourceCommandResult},
    source_creation_store::active,
    source_native_cli::private_assessment::{self, AssessmentEvaluationPlan, OperationBudget},
    source_private_assessment_sources::PrivateAssessmentSources,
    source_sign::OwnerAssessmentPublicSources,
    source_text_owner::OwnerTextContext,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{
    CutPreparedSchemaExecutionBinding, Digest256, JsonValue, RelativePath, SourceRevision,
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::{
    assessment::{AssessmentMechanicsReport, evaluate_current_assessment},
    source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor},
};

pub(super) struct FrameLease { _private: u8 }

pub(crate) struct HeldFrames<'a, 'scope> {
    pub(super) lease: Rc<FrameLease>,
    pub(super) owner: &'a OwnerTextContext,
    pub(super) context: &'a CommandContext,
    cut: &'a CorpusCutReader,
    grant: &'a ProtectedAssessmentJournal,
    private: &'a PrivateAssessmentSources,
    public: &'a mut OwnerAssessmentPublicSources<'scope>,
    fence: &'a AssessmentJournalFence<'scope>,
    histories: &'a BTreeMap<String, AssessmentHistory>,
    software: &'a SoftwareCaptureReader,
    components: &'a SoftwareComponentSelectionV1,
    binding: CutPreparedSchemaExecutionBinding,
    pub(super) worker: &'a mut CutWorkerSchemaExecutor,
    pub(super) budget: FrameBudget<'a>,
    pub(super) whole: Option<&'a Rc<RefCell<ClaimCallBudget>>>,
    pub(super) deadline: Instant,
    pub(super) cancelled: &'a AtomicBool,
}
impl<'a, 'scope> HeldFrames<'a, 'scope> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        owner: &'a OwnerTextContext,
        context: &'a CommandContext,
        cut: &'a CorpusCutReader,
        grant: &'a ProtectedAssessmentJournal,
        private: &'a PrivateAssessmentSources,
        public: &'a mut OwnerAssessmentPublicSources<'scope>,
        fence: &'a AssessmentJournalFence<'scope>,
        histories: &'a BTreeMap<String, AssessmentHistory>,
        software: &'a SoftwareCaptureReader,
        components: &'a SoftwareComponentSelectionV1,
        worker: &'a mut CutWorkerSchemaExecutor,
        budget: &'a mut OperationBudget,
        whole: &'a Rc<RefCell<ClaimCallBudget>>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::build(owner, context, cut, grant, private, public, fence, histories, software, components, worker, FrameBudget::Claim(budget), Some(whole), deadline, cancelled)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_assessment<'budget, 'cancel>(
        owner: &'a OwnerTextContext,
        context: &'a CommandContext,
        cut: &'a CorpusCutReader,
        grant: &'a ProtectedAssessmentJournal,
        private: &'a PrivateAssessmentSources,
        public: &'a mut OwnerAssessmentPublicSources<'scope>,
        fence: &'a AssessmentJournalFence<'scope>,
        histories: &'a BTreeMap<String, AssessmentHistory>,
        software: &'a SoftwareCaptureReader,
        components: &'a SoftwareComponentSelectionV1,
        worker: &'a mut CutWorkerSchemaExecutor,
        payer: &'a crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer<'budget, 'cancel>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::build(owner, context, cut, grant, private, public, fence, histories, software, components, worker, FrameBudget::Assessment(payer), None, deadline, cancelled)
    }
    #[allow(clippy::too_many_arguments)]
    fn build(
        owner: &'a OwnerTextContext,
        context: &'a CommandContext,
        cut: &'a CorpusCutReader,
        grant: &'a ProtectedAssessmentJournal,
        private: &'a PrivateAssessmentSources,
        public: &'a mut OwnerAssessmentPublicSources<'scope>,
        fence: &'a AssessmentJournalFence<'scope>,
        histories: &'a BTreeMap<String, AssessmentHistory>,
        software: &'a SoftwareCaptureReader,
        components: &'a SoftwareComponentSelectionV1,
        worker: &'a mut CutWorkerSchemaExecutor,
        mut budget: FrameBudget<'a>,
        whole: Option<&'a Rc<RefCell<ClaimCallBudget>>>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        if deadline > budget.limits()?.deadline { return Err(invalid("PWE frame exceeds original operation deadline")); }
        let holders = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<FrameLease>())
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
            .ok_or(unsupported("PWE protected frame lease holder"))?;
        budget.charge_work(holders)?;
        budget.charge_bytes(holders)?;
        if let Some(whole) = whole { whole.try_borrow_mut().map_err(|_| invalid("PWE whole-call budget in use"))?.retain(holders)?; }
        let binding = worker.prepared_execution_binding();
        let mut frames = Self {
            lease: Rc::new(FrameLease { _private: 0 }),
            binding,
            owner,
            context,
            cut,
            grant,
            private,
            public,
            fence,
            histories,
            software,
            components,
            worker,
            budget,
            whole,
            deadline,
            cancelled,
        };
        frames.verify()?;
        Ok(frames)
    }
    fn with_ledger<T>(&mut self, run: impl FnOnce(&mut CanonicalLedger<'_>) -> SourceCommandResult<T>) -> SourceCommandResult<T> {
        let (whole, deadline, cancelled) = (self.whole, self.deadline, self.cancelled);
        let mut ledger = CanonicalLedger::new_selected(&mut self.budget, whole, deadline, cancelled);
        run(&mut ledger)
    }
    pub(super) fn remaining_live(&mut self) -> SourceCommandResult<usize> {
        self.with_ledger(|ledger| ledger.remaining_live())
    }
    pub(super) fn check_live(&mut self, bytes: usize) -> SourceCommandResult<()> {
        if bytes > self.remaining_live()? { return Err(unsupported("PWE original live allowance")); }
        Ok(())
    }
    pub(super) fn check_session_payer(&mut self, session: &tos_compiler::pwe_native::Session<'_>, payer: &dyn super::source_budget::OriginalPayer) -> SourceCommandResult<()> {
        self.inspect_access(0, 3)?;
        payer.check_cutoff(self.deadline, self.cancelled)?;
        let matches = match (self.whole, payer.claim_wallet()) {
            (Some(expected), Some(actual)) => Rc::ptr_eq(expected, actual),
            (None, None) => self.budget.matches_assessment(payer),
            _ => false,
        };
        if !matches || !session.has_source_payer(payer.admission()) {
            return Err(SourceCommandError::Conflict("PWE frame/Session original payer differs"));
        }
        Ok(())
    }
    pub(super) fn with_roles<T>(
        &mut self,
        registry: crate::source_native_roles::RoleRegistrySelection<'_>,
        use_roles: impl FnOnce(
            &mut crate::source_native_roles::NativeRoleResolver<'_>,
        ) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        self.verify()?;
        let (context, cut, owner, private, worker, deadline, cancelled) =
            (self.context, self.cut, self.owner, self.private, &mut *self.worker, self.deadline, self.cancelled);
        let result = self.budget.with_operation(|budget| {
            let mut resolver = crate::source_native_roles::NativeRoleResolver::new(
                context, cut, owner, private, worker, budget, registry, deadline, cancelled,
            )?;
            let result = use_roles(&mut resolver);
            let fenced = resolver.check_current();
            match (result, fenced) {
                (Err(error), _) | (Ok(_), Err(error)) => Err(error),
                (Ok(value), Ok(())) => Ok(value),
            }
        });
        let fenced = self.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    /// Lend the original exact-source owner frames for one maintained read.
    /// The callback must retain its real inventory/history/readset and spend
    /// these same ledgers; it may not construct a worker, authority or budget.
    pub(crate) fn with_exact_dependency<T>(
        &mut self,
        read: impl FnOnce(
            &CommandContext,
            &CorpusCutReader,
            &mut CutWorkerSchemaExecutor,
            &mut OperationBudget,
            &Rc<RefCell<ClaimCallBudget>>,
            Instant,
            &AtomicBool,
        ) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        self.verify()?;
        let (context, cut, worker, whole, deadline, cancelled) =
            (self.context, self.cut, &mut *self.worker, self.whole, self.deadline, self.cancelled);
        let result = match whole {
            Some(whole) => self.budget.with_operation(|budget| read(context, cut, worker, budget, whole, deadline, cancelled)),
            None => Err(unsupported("PWE dependency reader requires the actual Assessment loan, not a Claim wallet")),
        };
        // Attempt the original fence even on callback failure. A successful
        // value is never released after a source/worker/currentness failure.
        // Neither refusal path refunds previously spent ledger prefixes.
        let fenced = self.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    /// Actual FS Assessment source loan. There is deliberately no Claim wallet.
    pub(crate) fn with_assessment_dependency<T>(
        &mut self,
        read: impl FnOnce(&CommandContext, &CorpusCutReader, &mut CutWorkerSchemaExecutor,
            &mut OperationBudget, Instant, &AtomicBool) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        self.verify()?;
        let (context, cut, worker, deadline, cancelled) =
            (self.context, self.cut, &mut *self.worker, self.deadline, self.cancelled);
        let result = if self.whole.is_none() {
            self.budget.with_operation(|budget| read(context, cut, worker, budget, deadline, cancelled))
        } else { Err(unsupported("PWE actual Assessment source loan selected for a Claim route")) };
        let fenced = self.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    /// Source reads need the actual payer WITHOUT holding its OperationBudget
    /// RefCell loan. This supports the Common original immutable read session.
    pub(crate) fn with_assessment_source_dependency<T>(
        &mut self,
        read: impl FnOnce(&CommandContext, &CorpusCutReader, &mut CutWorkerSchemaExecutor,
            &crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer<'_, '_>,
            Instant, &AtomicBool) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        self.verify()?;
        let (context, cut, worker, deadline, cancelled) =
            (self.context, self.cut, &mut *self.worker, self.deadline, self.cancelled);
        let result = self.budget.with_assessment_payer(|payer| read(context, cut, worker, payer, deadline, cancelled));
        let fenced = self.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    pub(super) fn canonical(&mut self, value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
        self.with_ledger(|ledger| ledger.canonical(value))
    }
    pub(super) fn hash_bytes(&mut self, bytes: &[u8]) -> SourceCommandResult<Digest256> {
        self.with_ledger(|ledger| ledger.hash_bytes(bytes))
    }
    pub(super) fn same(
        &mut self,
        left: &JsonValue,
        right: &JsonValue,
    ) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.same(left, right))
    }
    pub(super) fn exact_match(
        &mut self,
        reference: &serde_json::Value,
        value: &JsonValue,
        identity: &str,
        version: &str,
    ) -> SourceCommandResult<()> {
        self.with_ledger(|ledger| super::exact_match(reference, value, identity, version, ledger))
    }
    /// Same maintained assessment-envelope projection and whole source digest;
    /// unlike owner_reference(), its canonical/hash passes use held admission.
    pub(super) fn claim_reference(
        &mut self,
        row: &JsonValue,
    ) -> SourceCommandResult<serde_json::Value> {
        let (id, version, digest) = self.with_ledger(|ledger| {
        ledger.inspect_access(0, 1)?;
        if row.as_object().map(|o| o.len()) != Some(4) {
            return Err(invalid("PWE selected Claim envelope fields"));
        }
        let id = ledger.native_text(row, "id")?;
        let version = ledger
            .native_field(row, "version")?
            .as_u64()
            .ok_or(invalid("PWE selected Claim envelope version"))?;
        let payload = ledger.native_field(row, "payload")?;
        ledger.native_field(row, "origin_id")?;
        let digest = ledger.digest(payload)?;
        Ok((id, version, digest))
        })?;
        self.budget.charge_work(id.len())?;
        self.retain(
            id.len()
                .checked_mul(64)
                .and_then(|n| n.checked_add(1024))
                .ok_or(unsupported("PWE Claim reference holder overflow"))?,
        )?;
        Ok(serde_json::json!({"id":id,"version":version,"digest":digest.to_prefixed()}))
    }
    pub(super) fn encoded_len(&mut self, value: &serde_json::Value) -> SourceCommandResult<usize> {
        self.with_ledger(|ledger| ledger.encoded_len(value))
    }
    pub(super) fn reserve_domain_copy(
        &mut self,
        value: &serde_json::Value,
    ) -> SourceCommandResult<()> {
        let bytes = self.encoded_len(value)?;
        self.budget.charge_work(bytes)?;
        self.retain(
            bytes
                .checked_mul(64)
                .and_then(|n| n.checked_add(256))
                .ok_or(unsupported("PWE retained value copy overflow"))?,
        )
    }
    pub(super) fn get<'v>(
        &mut self,
        value: &'v serde_json::Value,
        key: &str,
    ) -> SourceCommandResult<Option<&'v serde_json::Value>> {
        self.with_ledger(|ledger| ledger.get(value, key))
    }
    pub(super) fn field<'v>(
        &mut self,
        value: &'v serde_json::Value,
        key: &str,
    ) -> SourceCommandResult<&'v serde_json::Value> {
        self.with_ledger(|ledger| ledger.field(value, key))
    }
    pub(super) fn pointer<'v>(
        &mut self,
        value: &'v serde_json::Value,
        pointer: &str,
    ) -> SourceCommandResult<Option<&'v serde_json::Value>> {
        self.with_ledger(|ledger| ledger.pointer(value, pointer))
    }
    pub(super) fn native_get<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> SourceCommandResult<Option<&'v JsonValue>> {
        self.with_ledger(|ledger| ledger.native_get(value, key))
    }
    pub(super) fn native_field<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> SourceCommandResult<&'v JsonValue> {
        self.with_ledger(|ledger| ledger.native_field(value, key))
    }
    pub(super) fn native_text<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> SourceCommandResult<&'v str> {
        self.with_ledger(|ledger| ledger.native_text(value, key))
    }
    pub(super) fn native_array<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> SourceCommandResult<&'v [JsonValue]> {
        self.with_ledger(|ledger| ledger.native_array(value, key))
    }
    pub(super) fn equal(
        &mut self,
        left: &serde_json::Value,
        right: &serde_json::Value,
    ) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.equal(left, right))
    }
    pub(super) fn optional_equal(
        &mut self,
        left: Option<&serde_json::Value>,
        right: Option<&serde_json::Value>,
    ) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.optional_equal(left, right))
    }
    pub(super) fn contains(
        &mut self,
        values: &[serde_json::Value],
        target: &serde_json::Value,
    ) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.contains(values, target))
    }
    pub(super) fn text_equal(&mut self, left: &str, right: &str) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.text_equal(left, right))
    }
    pub(super) fn text_is(
        &mut self,
        value: &serde_json::Value,
        key: &str,
        expected: &str,
    ) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.text_is(value, key, expected))
    }
    pub(super) fn inspect_access(
        &mut self,
        bytes: usize,
        visits: usize,
    ) -> SourceCommandResult<()> {
        self.with_ledger(|ledger| ledger.inspect_access(bytes, visits))
    }
    pub(super) fn string_map_get<'v, T>(
        &mut self,
        map: &'v BTreeMap<String, T>,
        key: &str,
    ) -> SourceCommandResult<Option<&'v T>> {
        self.with_ledger(|ledger| ledger.string_map_get(map, key))
    }
    pub(super) fn exact(
        &mut self,
        value: &serde_json::Value,
    ) -> SourceCommandResult<tos_foundation::ExactRecordRef> {
        self.with_ledger(|ledger| ledger.exact(value))
    }
    pub(super) fn exact_equal(
        &mut self,
        left: &tos_foundation::ExactRecordRef,
        right: &tos_foundation::ExactRecordRef,
    ) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.exact_equal(left, right))
    }
    pub(super) fn reference_matches(
        &mut self,
        value: &serde_json::Value,
        reference: tos_compiler::pwe_native::PweRef<'_>,
    ) -> SourceCommandResult<bool> {
        self.with_ledger(|ledger| ledger.reference_matches(value, reference))
    }
    pub(super) fn field_equal(
        &mut self,
        value: &serde_json::Value,
        key: &str,
        target: &serde_json::Value,
    ) -> SourceCommandResult<bool> {
        let found = self.get(value, key)?;
        self.optional_equal(found, Some(target))
    }
    pub(super) fn pointer_equal(
        &mut self,
        value: &serde_json::Value,
        pointer: &str,
        target: &serde_json::Value,
    ) -> SourceCommandResult<bool> {
        let found = self.pointer(value, pointer)?;
        self.optional_equal(found, Some(target))
    }
    pub(super) fn fields_equal(
        &mut self,
        left: &serde_json::Value,
        right: &serde_json::Value,
        key: &str,
    ) -> SourceCommandResult<bool> {
        let left = self.get(left, key)?;
        let right = self.get(right, key)?;
        self.optional_equal(left, right)
    }
    pub(super) fn pointers_equal(
        &mut self,
        left: &serde_json::Value,
        lpath: &str,
        right: &serde_json::Value,
        rpath: &str,
    ) -> SourceCommandResult<bool> {
        let left = self.pointer(left, lpath)?;
        let right = self.pointer(right, rpath)?;
        self.optional_equal(left, right)
    }
    pub(super) fn pointer_contains(
        &mut self,
        value: &serde_json::Value,
        pointer: &str,
        target: &serde_json::Value,
    ) -> SourceCommandResult<bool> {
        match self
            .pointer(value, pointer)?
            .and_then(serde_json::Value::as_array)
        {
            Some(values) => self.contains(values, target),
            None => Ok(false),
        }
    }
    pub(super) fn string_arguments(
        &mut self,
        values: &serde_json::Value,
        arguments: &[String],
    ) -> SourceCommandResult<bool> {
        let Some(values) = values.as_array() else {
            return Ok(false);
        };
        let mut index = 0;
        for value in values {
            self.inspect_access(0, 1)?;
            if let Some(value) = value.as_str() {
                let Some(expected) = arguments.get(index) else {
                    return Ok(false);
                };
                if !self.text_equal(value, expected)? {
                    return Ok(false);
                }
                index += 1;
            }
        }
        Ok(index == arguments.len())
    }
    pub(super) fn pointer_text_is(
        &mut self,
        value: &serde_json::Value,
        pointer: &str,
        expected: &str,
    ) -> SourceCommandResult<bool> {
        match self
            .pointer(value, pointer)?
            .and_then(serde_json::Value::as_str)
        {
            Some(actual) => self.text_equal(actual, expected),
            None => Ok(false),
        }
    }
    /// Requires the common-owned admitted lookup on the actual held worker.
    /// No guessed resource count or descriptor supplies this cost boundary.
    pub(super) fn contract_digest(
        &mut self,
        contract: &str,
    ) -> SourceCommandResult<Option<Digest256>> {
        let (worker, whole, deadline, cancelled) = (&mut *self.worker, self.whole, self.deadline, self.cancelled);
        {
            let mut ledger = CanonicalLedger::new_selected(&mut self.budget, whole, deadline, cancelled);
        let mut admission_error = None;
        let result = worker
            .contract_digest_with_admission(contract, &mut |bytes, visits| {
                ledger.inspect_access(bytes, visits).map_err(|error| {
                    admission_error = Some(error);
                    tos_foundation::FoundationError::new(
                        tos_foundation::FoundationErrorCode::BudgetExceeded,
                        "PWE schema lookup admission",
                    )
                })
            });
        result.map_err(|_| {
            admission_error.unwrap_or(unsupported("PWE held schema lookup incomplete"))
        })
        }
    }
    pub(crate) fn revision(&self) -> SourceRevision {
        self.context.base_revision
    }
    pub(crate) fn source_identity(&self) -> Digest256 {
        self.revision().0
    }
    pub(crate) fn source_path(&mut self, id: &str) -> SourceCommandResult<String> {
        let public = self.public.record_paths.iter();
        let private = &self.private.source_paths;
        let (whole, deadline, cancelled) = (self.whole, self.deadline, self.cancelled);
        {
            let mut ledger = CanonicalLedger::new_selected(&mut self.budget, whole, deadline, cancelled);
        let mut selected_public = None;
        for (candidate, path) in public {
            if ledger.text_equal(candidate, id)? {
                selected_public = Some(path.as_str());
                break;
            }
        }
        let selected_private = ledger.string_map_get(private, id)?.map(String::as_str);
        match (selected_public, selected_private) {
            (Some(a), Some(b)) => {
                if !ledger.text_equal(a, b)? {
                    return Err(SourceCommandError::Conflict(
                        "PWE selected owner source path collision",
                    ));
                }
                ledger.inspect_access(a.len(), 1)?;
                ledger.reserve_access(a.len())?;
                Ok(a.to_owned())
            }
            (Some(path), _) | (_, Some(path)) => {
                ledger.inspect_access(path.len(), 1)?;
                ledger.reserve_access(path.len())?;
                Ok(path.to_owned())
            }
            _ => Err(unsupported("PWE actual source Claim path not selected")),
        }
        }
    }
    pub(crate) fn verify(&mut self) -> SourceCommandResult<()> {
        active(self.deadline, self.cancelled)?;
        let summaries = self
            .private
            .retained_raw
            .len()
            .checked_add(self.context.files.len())
            .and_then(|n| n.checked_add(self.histories.len()))
            .and_then(|n| n.checked_add(1))
            .ok_or(unsupported("PWE fence summary work overflow"))?;
        self.budget.charge_work(summaries)?;
        // Charge the known retained source-byte envelope on every guard.
        // Configuration/transport metadata and software/schema work keep their
        // existing native owner budgets; this is not a physical-I/O theorem.
        let mut fence_bytes = self
            .private
            .input_bytes()?
            .checked_add(self.public.input_bytes()?)
            .and_then(|n| {
                self.grant
                    .configuration_raw()
                    .len()
                    .checked_mul(2)
                    .and_then(|twice| n.checked_add(twice))
            })
            .ok_or(unsupported("PWE fence byte overflow"))?;
        for file in &self.context.files {
            fence_bytes = fence_bytes
                .checked_add(file.raw.len())
                .ok_or(unsupported("PWE fence byte overflow"))?;
        }
        for history in self.histories.values() {
            fence_bytes = fence_bytes
                .checked_add(history.input_bytes()?)
                .ok_or(unsupported("PWE history fence byte overflow"))?;
        }
        // Both with_exact_dependency fences enter here independently. Fund
        // their actual source-byte inspection work BEFORE executing either
        // currentness traversal, including both grant checks within one fence.
        self.budget.charge_work(fence_bytes)?;
        self.budget.charge_bytes(fence_bytes)?;
        if let Some(whole) = self.whole {
            whole.try_borrow_mut().map_err(|_| invalid("PWE whole-call budget in use"))?
                .read(u64::try_from(fence_bytes).map_err(|_| unsupported("PWE fence byte conversion"))?)?;
        }
        self.context.check_from_selected_captures(
            self.cut,
            self.software,
            self.components,
            self.deadline,
            self.cancelled,
        )?;
        if self.cut.current().revision() != self.revision()
            || self.worker.source_revision() != self.revision()
            || self.worker.prepared_execution_binding() != self.binding
        {
            return Err(SourceCommandError::Conflict(
                "PWE source/schema selected revision differs",
            ));
        }
        self.grant.verify_current(self.deadline, self.cancelled)?;
        self.public.verify_current(self.deadline, self.cancelled)?;
        self.private.verify_current(
            self.owner,
            self.context,
            self.cut,
            self.deadline,
            self.cancelled,
        )?;
        self.fence.verify_current(self.deadline, self.cancelled)?;
        for history in self.histories.values() {
            history.verify_current(self.fence, self.deadline, self.cancelled)?;
        }
        self.grant.verify_current(self.deadline, self.cancelled)?;
        active(self.deadline, self.cancelled)
    }
    /// Only bytes already selected by the protected owner. The C4 resolver may
    /// not turn an arbitrary locator into a filesystem read through this host.
    pub(crate) fn selected_bytes(&mut self, path: &str) -> SourceCommandResult<Vec<u8>> {
        // Pay path validation/copy, each actual path comparison, collision
        // bytes and final source copying before the corresponding operation.
        active(self.deadline, self.cancelled)?;
        let context = self.context;
        let private = self.private;
        self.with_ledger(|ledger| {
        ledger.inspect_access(
            path.len()
                .checked_mul(16)
                .ok_or(unsupported("PWE path work overflow"))?,
            1,
        )?;
        ledger.reserve_access(
            path.len()
                .checked_add(std::mem::size_of::<RelativePath>())
                .ok_or(unsupported("PWE path holder overflow"))?,
        )?;
        let relative =
            RelativePath::parse(path).map_err(|_| invalid("PWE selected source path"))?;
        let mut public = None;
        for file in &context.files {
            if ledger.text_equal(file.path.as_str(), relative.as_str())? {
                if public.is_some() {
                    return Err(invalid("duplicate selected source path"));
                }
                public = Some(file.raw.as_slice());
            }
        }
        let private = ledger
            .string_map_get(&private.retained_raw, path)?
            .map(Vec::as_slice);
        let raw = match (public, private) {
            (Some(a), Some(b)) => {
                if a.len() > MAX_BYTES || b.len() > MAX_BYTES {
                    return Err(unsupported("PWE source member byte cap"));
                }
                if !ledger.bytes_equal(a, b)? {
                    return Err(SourceCommandError::Conflict(
                        "PWE selected source collision",
                    ));
                }
                a
            }
            (Some(a), _) | (_, Some(a)) => a,
            _ => return Err(unsupported("PWE exact source path was not selected")),
        };
        ledger.inspect_access(raw.len(), 1)?;
        if raw.len() > MAX_BYTES { return Err(unsupported("PWE source member byte cap")); }
        ledger.account_copy_input(raw.len())?;
        let available = ledger.remaining_live()?;
        ledger.reserve_access(raw.len())?;
        let mut copy = Vec::new();
        copy.try_reserve_exact(raw.len()).map_err(|_| unsupported("PWE source buffer reservation"))?;
        if copy.capacity() > available { return Err(unsupported("PWE source buffer live allowance")); }
        ledger.reserve_access(copy.capacity().saturating_sub(raw.len()))?;
        copy.extend_from_slice(raw);
        Ok(copy)
        })
    }
    pub(crate) fn parse(&mut self, raw: &[u8]) -> SourceCommandResult<JsonValue> {
        self.with_ledger(|ledger| ledger.parse_selected(raw))
    }
    pub(crate) fn check_schema(
        &mut self,
        path: &str,
        record: &JsonValue,
        schema: &str,
    ) -> SourceCommandResult<()> {
        self.inspect_access(schema.len(), 1)?;
        if !schema.starts_with("ToS/contracts/")
            || !schema.ends_with(".schema.json")
            || self.contract_digest(schema)?.is_none()
        {
            return Err(unsupported("PWE actual schema is not in selected worker"));
        }
        let raw = self.canonical(record)?;
        match self
            .worker
            .check(path, &raw, schema, self.deadline, self.cancelled)
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(invalid("PWE selected source schema refused")),
            Err(_) => Err(unsupported(
                "PWE selected source schema execution incomplete",
            )),
        }
    }
    pub(crate) fn selected_document(
        &mut self,
        path: &str,
        schema: &str,
    ) -> SourceCommandResult<JsonValue> {
        let raw = self.selected_bytes(path)?;
        let value = self.parse(&raw)?;
        self.check_schema(path, &value, schema)?;
        self.verify()?;
        Ok(value)
    }
    pub(crate) fn retain(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.with_ledger(|ledger| ledger.reserve_access(bytes))
    }
    pub(crate) fn domain_value(
        &mut self,
        value: &JsonValue,
    ) -> SourceCommandResult<serde_json::Value> {
        let raw = self.canonical(value)?;
        self.decode_domain(&raw)
    }
    pub(super) fn decode_domain(&mut self, raw: &[u8]) -> SourceCommandResult<serde_json::Value> {
        self.with_ledger(|ledger| ledger.decode_domain(raw))
    }
    /// Evaluate the actual protected plan under the already held histories and
    /// existing worker/budget. No caller-supplied assessment boolean is read.
    pub(crate) fn assess(
        &mut self,
        plan: &AssessmentEvaluationPlan<'_>,
    ) -> SourceCommandResult<AssessmentMechanicsReport> {
        self.verify()?;
        let result = self.assess_current(plan);
        // Fence the protected resources after every attempted assessment,
        // including selection, history and evaluator refusals. Preserve the
        // original error and every spent prefix if the final fence also fails.
        let fenced = self.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(report), Ok(())) => Ok(report),
        }
    }
    fn assess_current(
        &mut self,
        plan: &AssessmentEvaluationPlan<'_>,
    ) -> SourceCommandResult<AssessmentMechanicsReport> {
        let grant = self.grant;
        if !self.same(plan.config, grant.configuration())? {
            return Err(SourceCommandError::Conflict(
                "PWE assessment protected configuration differs",
            ));
        }
        let (current, scope) = self.selected_scope(plan)?;
        if !self.same(&current, &plan.current)?
            || !self.same(&scope, &plan.scope)?
            || plan.source_route
                != tos_validation::assessment::AssessmentSourceRoute::SourceBoundClaim
        {
            return Err(SourceCommandError::Conflict(
                "PWE assessment source/current scope differs",
            ));
        }
        for subject in &plan.lock_subjects {
            let histories = self.histories;
            if self.string_map_get(histories, subject)?.is_none() {
                return Err(invalid("PWE complete held assessment history absent"));
            }
        }
        let revision = self.revision();
        let mut evaluator = |input: &tos_validation::assessment::AssessmentReadInput,
                             worker: &mut CutWorkerSchemaExecutor,
                             limits: tos_validation::assessment::AssessmentLimits,
                             cancelled: &AtomicBool| {
            evaluate_current_assessment(input, worker, limits, cancelled)
                .map_err(private_assessment::map_assessment_refusal)
        };
        let result = self.budget.with_operation(|budget| private_assessment::evaluate_assessment_histories(
            plan,
            self.histories,
            &self.context.recorded_at,
            Some(revision),
            revision.0,
            self.worker,
            budget,
            self.cancelled,
            &mut evaluator,
        ))?;
        Ok(result.current_report)
    }
}

// Closed, borrowed native resources. Only the two checked constructors below
// can create this loan. Its existence never authorizes unrelated subjects.
pub(crate) enum HeldEvaluationPosition<'checked> {
    Account(&'checked super::owner::ApplicationCheckedTicket<'checked>),
    FormWithCheckedParent,
}
pub(crate) struct HeldPweEvaluation<'checked> {
    position: HeldEvaluationPosition<'checked>,
    target: &'checked private_assessment::AssessmentEvaluationTarget<'checked, 'checked>,
    parent: Option<&'checked super::owner::CheckedPweParent<'checked, 'checked>>,
    histories: &'checked BTreeMap<String, AssessmentHistory>,
    observed_now: &'checked str,
    source_revision: SourceRevision,
    source_identity: Digest256,
    worker: &'checked mut CutWorkerSchemaExecutor,
    budget: &'checked mut dyn super::budget::OperationAccess,
    whole: Option<&'checked Rc<RefCell<ClaimCallBudget>>>,
    deadline: Instant,
    cancelled: &'checked AtomicBool,
}
impl HeldPweEvaluation<'_> {
    pub(crate) fn position(&self) -> &HeldEvaluationPosition<'_> { &self.position }
    pub(crate) fn target(&self) -> &private_assessment::AssessmentEvaluationTarget<'_, '_> { self.target }
    pub(crate) fn parent(&self) -> Option<&super::owner::CheckedPweParent<'_, '_>> { self.parent }
    /// Lexical resource loan to Common's maintained evaluator. The private
    /// checked constructor, not this callback, binds application permission.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_filesystem<T>(
        &mut self,
        run: impl FnOnce(
            &HeldEvaluationPosition<'_>,
            &private_assessment::AssessmentEvaluationTarget<'_, '_>,
            Option<&super::owner::CheckedPweParent<'_, '_>>,
            &BTreeMap<String, AssessmentHistory>, &str, SourceRevision, Digest256,
            &mut CutWorkerSchemaExecutor, &mut OperationBudget,
            Option<&Rc<RefCell<ClaimCallBudget>>>, Instant, &AtomicBool,
        ) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        let (position, target, parent, histories, observed, revision, identity, worker, whole, deadline, cancelled) =
            (&self.position, self.target, self.parent, self.histories, self.observed_now,
             self.source_revision, self.source_identity, &mut *self.worker, self.whole, self.deadline, self.cancelled);
        super::budget::with_operation_access(self.budget, |budget| run(position, target, parent, histories, observed,
            revision, identity, worker, budget, whole, deadline, cancelled))
    }
}
impl HeldFrames<'_, '_> {
    fn selected_scope(&mut self, plan: &AssessmentEvaluationPlan<'_>) -> SourceCommandResult<(JsonValue, JsonValue)> {
        let (config, whole, deadline, cancelled) = (self.grant.configuration(), self.whole, self.deadline, self.cancelled);
        self.budget.with_operation(|budget| match whole {
            Some(whole) => private_assessment::select_current_scope_with_admission(
                config, plan.request, &plan.all_by_id, budget, whole, deadline, cancelled),
            None => private_assessment::select_current_scope_with_operation_admission(
                config, plan.request, &plan.all_by_id, budget, deadline, cancelled),
        })
    }
    pub(super) fn reserve_scoped(
        &mut self,
        session: &mut tos_compiler::pwe_native::Session<'_>,
        payer: &dyn super::source_budget::OriginalPayer,
        bytes: usize,
    ) -> SourceCommandResult<()> {
        self.check_session_payer(session, payer)?;
        self.budget.charge_work(bytes)?;
        self.budget.charge_bytes(bytes)?;
        session.admit_source_prefix(0, 8).map_err(|e| payer.error(e, "PWE lease work"))?;
        session.retain_source_state(bytes).map_err(|e| payer.error(e, "PWE scoped holder"))
    }
    fn check_selected_plan(&mut self, plan: &AssessmentEvaluationPlan<'_>) -> SourceCommandResult<()> {
        let grant = self.grant;
        if !self.same(plan.config, grant.configuration())? {
            return Err(SourceCommandError::Conflict("PWE checked evaluation protected config differs"));
        }
        let (current, scope) = self.selected_scope(plan)?;
        if !self.same(&current, &plan.current)? || !self.same(&scope, &plan.scope)? {
            return Err(SourceCommandError::Conflict("PWE checked evaluation selected plan differs"));
        }
        for subject in &plan.lock_subjects {
            if self.string_map_get(self.histories, subject)?.is_none() {
                return Err(invalid("PWE checked evaluation held history absent"));
            }
        }
        Ok(())
    }
    fn check_report_target(
        &mut self,
        target: &private_assessment::AssessmentEvaluationTarget<'_, '_>,
        report: &AssessmentMechanicsReport,
    ) -> SourceCommandResult<()> {
        self.inspect_access(0, 4)?;
        let binding = tos_validation::assessment::AssessmentSchemaBindingV1::SourceCut {
            source_revision: self.revision(), worker: self.worker.prepared_execution_binding(),
        };
        let recorded_at = self.context.recorded_at.as_str();
        if report.source_revision() != Some(self.revision())
            || report.source_input_identity() != self.source_identity()
            || report.schema_binding() != binding
            || !self.text_equal(report.observed_now(), recorded_at)?
        {
            return Err(SourceCommandError::Conflict("PWE checked evaluation source/worker/time differs"));
        }
        let reference = self.claim_reference(target.record())?;
        let scope = self.domain_value(target.scope())?;
        if !self.field_equal(report.current_admission(), "subject", &reference)?
            || !self.equal(report.scope(), &scope)?
        {
            return Err(SourceCommandError::Conflict("PWE checked evaluation exact subject/scope differs"));
        }
        Ok(())
    }
    pub(super) fn assess_checked(
        &mut self,
        ticket: super::owner::ApplicationCheckedTicket<'_>,
        parent: Option<super::owner::CheckedPweParent<'_, '_>>,
    ) -> SourceCommandResult<AssessmentMechanicsReport> {
        self.verify()?;
        let result = (|| {
            self.check_selected_plan(ticket.target.plan())?;
            if ticket.target.source_route() != tos_validation::assessment::AssessmentSourceRoute::SourceBoundClaim {
                return Err(invalid("PWE ticket target is not an actual source Claim"));
            }
            ticket.check_frame(self)?;
            if let Some(parent) = parent.as_ref() { ticket.check_parent(parent, self)?; }
            let revision = self.revision();
            let identity = self.source_identity();
            let mut evaluation = HeldPweEvaluation {
                position: HeldEvaluationPosition::Account(&ticket), target: &ticket.target,
                parent: parent.as_ref(), histories: self.histories,
                observed_now: &self.context.recorded_at, source_revision: revision, source_identity: identity,
                worker: &mut *self.worker, budget: &mut self.budget, whole: self.whole,
                deadline: self.deadline, cancelled: self.cancelled,
            };
            // Common owns this agreed closed entry and its private dispatch
            // implementation. No raw generic/PWE exemption or report factory.
            let result = private_assessment::evaluate_held_pwe_assessment(&mut evaluation);
            drop(evaluation);
            let report = result?.current_report;
            self.check_report_target(&ticket.target, &report)?;
            Ok(report)
        })();
        let fenced = self.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(report), Ok(())) => Ok(report),
        }
    }
    pub(super) fn assess_form_with_parent(
        &mut self,
        plan: &AssessmentEvaluationPlan<'_>,
        parent: super::owner::CheckedPweParent<'_, '_>,
        host_lease: &Rc<super::owner::HostLease>,
    ) -> SourceCommandResult<AssessmentMechanicsReport> {
        self.verify()?;
        let result = (|| {
            self.check_selected_plan(plan)?;
            parent.check_host(self, host_lease)?;
            parent.check_frame(self, plan)?;
            let payload = self.native_field(&plan.current, "payload")?;
            let schema = self.native_text(payload, "schema_version")?;
            if !self.text_equal(schema, "tos_human_form_v1")? {
                return Err(invalid("PWE parent-only loan requires actual ordinary human form"));
            }
            let target = plan.current_target();
            let revision = self.revision();
            let identity = self.source_identity();
            let mut evaluation = HeldPweEvaluation {
                position: HeldEvaluationPosition::FormWithCheckedParent, target: &target,
                parent: Some(&parent), histories: self.histories,
                observed_now: &self.context.recorded_at, source_revision: revision, source_identity: identity,
                worker: &mut *self.worker, budget: &mut self.budget, whole: self.whole,
                deadline: self.deadline, cancelled: self.cancelled,
            };
            let result = private_assessment::evaluate_held_pwe_assessment(&mut evaluation);
            drop(evaluation);
            let report = result?.current_report;
            self.check_report_target(&target, &report)?;
            Ok(report)
        })();
        let fenced = self.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(report), Ok(())) => Ok(report),
        }
    }
}
