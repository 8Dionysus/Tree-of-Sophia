//! Installed v1-v6 native assessment caller.
//!
//! Direct public v1-v3 commands and confidential v4-v6 commands share the
//! existing append/replay journal and assessment engine. Public v2/v3 source
//! selection reuses the Sign resolver; Sign promotion keeps its separate
//! protected v2/v3 selector and does not acquire new command authority.

use super::{absolute, capped, digest, exact, selected_schema_with_profile, text};
use crate::source_assessment_journal::{
    AssessmentHistory, AssessmentJournalFence, ProtectedAssessmentJournal,
};
use crate::source_command::{
    self as cmd, CommandContext, SourceCommandError, SourceCommandResult, SourceFile,
};
use crate::source_creation_store::protected_configuration_parents;
use crate::source_private_assessment_sources::PrivateAssessmentSources;
use crate::source_text_owner::OwnerTextContext;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, SoftwareCaptureReader, SoftwareComponentSelectionV1,
};
use tos_validation::assessment::{
    AssessmentLayerQualityObservation, AssessmentLimits, AssessmentMechanicsReport,
    AssessmentReadInput, AssessmentRecordInput, AssessmentRefusal, AssessmentSourceRoute,
    AssessmentSubmissionInput, MAX_ASSESSMENTS, MAX_RECORD_BYTES, evaluate_current_assessment,
};
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget, VerifiedWorkerImageHandle};
use tos_validation::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor,
};

#[path = "source_native_private_assessment_scope.rs"]
mod scope_admission;
use scope_admission::ScopeAccess;

#[path = "source_native_assessment_payer.rs"]
pub(crate) mod original_payer;

const OWNER_CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const BATCH_SCHEMA: &str = "ToS/contracts/knowledge-assessment-batch.schema.json";
const QUALITY_BASIS_SCHEMA: &str = "ToS/contracts/native-text-layer-quality-basis.schema.json";

const MAX_OPERATION_BYTES: usize = 234_881_024;
const MAX_RESOLVED_BYTES: usize = 33_554_432;
const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_CONFIG_BYTES: usize = 8_388_608;
const MAX_QUALITY_SUBJECTS: usize = 64;

#[derive(Clone)]
pub(crate) struct ParsedRequest {
    pub(crate) operation: String,
    pub(crate) subject_id: String,
    pub(crate) expected_subject: Option<JsonValue>,
    pub(crate) expected_snapshot: Option<String>,
    pub(crate) command_id: Option<String>,
    pub(crate) expected_revision: Option<Option<String>>,
    pub(crate) assessments: Vec<JsonValue>,
}

struct SelectedOperation<'a> {
    cut: &'a CorpusCutReader,
    owner: ProtectedAssessmentJournal,
    owner_context: OwnerTextContext,
    selected_context: JsonValue,
    context: CommandContext,
    worker: CutWorkerSchemaExecutor,
    request: ParsedRequest,
    version: OwnerVersion,
    private_sources: PrivateAssessmentSources,
    layer_preflight: Option<crate::source_private_assessment_layers::OwnerLocalLayerPreflight>,
    budget: Option<OperationBudget>,
}

struct SelectedOperationView<'a> {
    cut: &'a CorpusCutReader,
    owner: &'a ProtectedAssessmentJournal,
    owner_context: &'a OwnerTextContext,
    context: &'a CommandContext,
    request: &'a ParsedRequest,
    version: OwnerVersion,
    private_sources: &'a PrivateAssessmentSources,
}

/// Source-neutral inputs after each transport has completed its own exact
/// source selection. This is the shared semantic boundary for the maintained
/// filesystem journal and the PG private-package adapter.
pub(crate) struct AssessmentEvaluationInputs<'a> {
    pub(crate) config: &'a JsonValue,
    pub(crate) configuration_raw: &'a [u8],
    pub(crate) request: &'a ParsedRequest,
    pub(crate) version: OwnerVersion,
    pub(crate) resolved: AssessmentResolvedSources,
    pub(crate) public_fixity: &'a [JsonValue],
    pub(crate) public_identity_snapshots: &'a BTreeMap<String, JsonValue>,
    pub(crate) public_claim_dependencies: &'a BTreeMap<String, Vec<JsonValue>>,
    pub(crate) private_source_snapshot: &'a str,
    pub(crate) private_native_snapshots: &'a [String],
    pub(crate) private_required_source_refs: &'a BTreeMap<String, Vec<JsonValue>>,
    pub(crate) private_required_languages: &'a BTreeMap<String, Vec<String>>,
    pub(crate) native_summaries: &'a [JsonValue],
    pub(crate) layers: Option<&'a crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    pub(crate) layer_preflight:
        Option<&'a crate::source_private_assessment_layers::OwnerLocalLayerPreflight>,
}

pub(crate) struct AssessmentEvaluationPlan<'a> {
    pub(crate) config: &'a JsonValue,
    pub(crate) request: &'a ParsedRequest,
    pub(crate) version: OwnerVersion,
    pub(crate) resolved: AssessmentResolvedSources,
    pub(crate) all_by_id: BTreeMap<String, JsonValue>,
    pub(crate) base_snapshot: String,
    pub(crate) current: JsonValue,
    pub(crate) scope: JsonValue,
    pub(crate) subject_required_source_refs: Vec<JsonValue>,
    pub(crate) required_sources: Vec<JsonValue>,
    pub(crate) source_route: AssessmentSourceRoute,
    pub(crate) subject_quality_requirements: Vec<QualityRequirement>,
    pub(crate) parent_context: Option<ParentClaimContext>,
    pub(crate) parent_quality_requirements: Vec<QualityRequirement>,
    pub(crate) lock_subjects: Vec<String>,
    pub(crate) public_claim_dependencies: &'a BTreeMap<String, Vec<JsonValue>>,
    pub(crate) private_required_source_refs: &'a BTreeMap<String, Vec<JsonValue>>,
    pub(crate) layers: Option<&'a crate::source_private_assessment_layers::PrivateAssessmentLayers>,
}

/// A borrowed position within the original protected plan, not another user
/// request or an authorization ticket. Native account preparation must still
/// check its exact application, source, scope, history and held host lease.
pub(crate) struct AssessmentEvaluationTarget<'plan, 'data> {
    plan: &'plan AssessmentEvaluationPlan<'data>,
    parent: Option<&'plan ParentClaimContext>,
}

impl<'data> AssessmentEvaluationPlan<'data> {
    pub(crate) fn current_target(&self) -> AssessmentEvaluationTarget<'_, 'data> {
        AssessmentEvaluationTarget {
            plan: self,
            parent: None,
        }
    }

    pub(crate) fn parent_target(&self) -> Option<AssessmentEvaluationTarget<'_, 'data>> {
        self.parent_context
            .as_ref()
            .map(|parent| AssessmentEvaluationTarget {
                plan: self,
                parent: Some(parent),
            })
    }
}

impl<'plan, 'data> AssessmentEvaluationTarget<'plan, 'data> {
    pub(crate) fn plan(&self) -> &'plan AssessmentEvaluationPlan<'data> {
        self.plan
    }

    pub(crate) fn record(&self) -> &'plan JsonValue {
        self.parent
            .map_or(&self.plan.current, |parent| &parent.record)
    }

    pub(crate) fn scope(&self) -> &'plan JsonValue {
        self.parent.map_or(&self.plan.scope, |parent| &parent.scope)
    }

    pub(crate) fn required_sources(&self) -> &'plan [JsonValue] {
        self.parent
            .map_or(self.plan.required_sources.as_slice(), |parent| {
                parent.required_sources.as_slice()
            })
    }

    pub(crate) fn quality_requirements(&self) -> &'plan [QualityRequirement] {
        if self.parent.is_some() {
            &self.plan.parent_quality_requirements
        } else {
            &self.plan.subject_quality_requirements
        }
    }

    pub(crate) fn source_route(&self) -> AssessmentSourceRoute {
        if self.parent.is_some() {
            AssessmentSourceRoute::SourceBoundClaim
        } else {
            self.plan.source_route
        }
    }

    pub(crate) fn is_parent(&self) -> bool {
        self.parent.is_some()
    }
}

pub(crate) struct AssessmentEvaluationResult {
    pub(crate) current_snapshot: String,
    pub(crate) quality_bases: Vec<(QualityRequirement, JsonValue)>,
    pub(crate) admission_bases: Vec<(
        QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )>,
    pub(crate) current_report: AssessmentMechanicsReport,
    pub(crate) append_report: Option<AssessmentMechanicsReport>,
    pub(crate) parent_assessment: Option<JsonValue>,
    pub(crate) source_read_ready: bool,
}

#[derive(Debug)]
pub(crate) struct OperationBudget {
    bytes_remaining: usize,
    work_remaining: usize,
    worker_cpu_seconds: u64,
    worker_address_space_bytes: u64,
    deadline: Instant,
}

/// Transport-neutral, collision-checked owner/source rows used by the
/// maintained Assessment engine. Native filesystem and PG-selected source
/// adapters can both feed their exact selected envelopes through this one
/// bounded closure builder before evaluation.
pub(crate) struct AssessmentResolvedSources {
    pub(crate) source_records: Vec<JsonValue>,
    pub(crate) source_index: BTreeMap<String, usize>,
    pub(crate) native_records: Vec<JsonValue>,
    pub(crate) native_index: BTreeMap<String, usize>,
    pub(crate) resolved_rows: Vec<JsonValue>,
    resolved_index: BTreeMap<String, usize>,
}

/// Exact form and source-path selections shared by filesystem and PG
/// Assessment adapters. Bytes and currentness proofs remain with the adapter;
/// this view carries only the already-selected semantic records.
pub(crate) struct AssessmentFormMaps<'a> {
    pub(crate) form_paths: &'a BTreeMap<String, String>,
    pub(crate) form_sets: &'a BTreeMap<String, JsonValue>,
    pub(crate) source_paths: &'a BTreeMap<String, String>,
}

impl AssessmentResolvedSources {
    pub(crate) fn from_selected_rows(
        public_rows: &[JsonValue],
        private_rows: &[AssessmentRecordInput],
        public_native_rows: &[JsonValue],
        private_native_rows: &[AssessmentRecordInput],
        budget: &mut OperationBudget,
    ) -> SourceCommandResult<Self> {
        let mut selected = Self {
            source_records: Vec::new(),
            source_index: BTreeMap::new(),
            native_records: Vec::new(),
            native_index: BTreeMap::new(),
            resolved_rows: Vec::new(),
            resolved_index: BTreeMap::new(),
        };
        for row in public_rows {
            budget.charge_work(1)?;
            push_resolved(
                row.clone(),
                &mut selected.source_records,
                &mut selected.source_index,
            )?;
            push_resolved(
                row.clone(),
                &mut selected.resolved_rows,
                &mut selected.resolved_index,
            )?;
        }
        for input in private_rows {
            budget.charge_work(1)?;
            let row = parse_record_input(input)?;
            push_resolved(
                row.clone(),
                &mut selected.source_records,
                &mut selected.source_index,
            )?;
            push_resolved(
                row,
                &mut selected.resolved_rows,
                &mut selected.resolved_index,
            )?;
        }
        for row in public_native_rows {
            budget.charge_work(1)?;
            let id = cmd::text(row, "id")?;
            if selected.source_index.contains_key(id) {
                return Err(SourceCommandError::Denied(
                    "public source selection shadows a native assessment identity",
                ));
            }
            push_resolved(
                row.clone(),
                &mut selected.native_records,
                &mut selected.native_index,
            )?;
            push_resolved(
                row.clone(),
                &mut selected.resolved_rows,
                &mut selected.resolved_index,
            )?;
        }
        for input in private_native_rows {
            budget.charge_work(1)?;
            let row = parse_record_input(input)?;
            push_resolved(
                row.clone(),
                &mut selected.native_records,
                &mut selected.native_index,
            )?;
            push_resolved(
                row,
                &mut selected.resolved_rows,
                &mut selected.resolved_index,
            )?;
        }
        Ok(selected)
    }

    pub(crate) fn add_layer_records(
        &mut self,
        records: &[AssessmentRecordInput],
        budget: &mut OperationBudget,
    ) -> SourceCommandResult<()> {
        for input in records {
            budget.charge_work(1)?;
            let row = parse_record_input(input)?;
            let id = cmd::text(&row, "id")?.to_owned();
            if let Some(index) = self.native_index.get(&id).copied() {
                if !cmd::same(&self.native_records[index], &row)? {
                    return Err(SourceCommandError::Conflict(
                        "assessment layer identity shadows different native evidence",
                    ));
                }
            }
            push_resolved(
                row.clone(),
                &mut self.source_records,
                &mut self.source_index,
            )?;
            push_resolved(row, &mut self.resolved_rows, &mut self.resolved_index)?;
        }
        Ok(())
    }

    pub(crate) fn check_budgets(&self, budget: &mut OperationBudget) -> SourceCommandResult<()> {
        if self
            .source_records
            .len()
            .checked_add(self.native_records.len())
            .is_none_or(|count| count > MAX_ASSESSMENTS)
            || self.resolved_rows.len() > MAX_ASSESSMENTS
        {
            return Err(SourceCommandError::Invalid(
                "assessment combined owner/source record budget",
            ));
        }
        let resolved_bytes = cmd::canonical(&JsonValue::Array(self.resolved_rows.clone()))?.len();
        budget.charge_bytes(resolved_bytes)?;
        if resolved_bytes > MAX_RESOLVED_BYTES {
            return Err(SourceCommandError::Invalid(
                "assessment resolved source byte budget",
            ));
        }
        Ok(())
    }

    pub(crate) fn all_by_id(
        &self,
        config: &JsonValue,
    ) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
        let mut all = BTreeMap::<String, JsonValue>::new();
        for row in self
            .resolved_rows
            .iter()
            .chain(cmd::array(config, "records")?)
        {
            let id = cmd::text(row, "id")?.to_owned();
            if let Some(old) = all.get(&id) {
                if !cmd::same(old, row)? {
                    return Err(SourceCommandError::Conflict(
                        "assessment owner/source identity has conflicting current bodies",
                    ));
                }
            } else {
                all.insert(id, row.clone());
            }
        }
        Ok(all)
    }
}

impl OperationBudget {
    pub(crate) fn new(invocation: &Value, deadline: Instant) -> SourceCommandResult<Self> {
        let budgets = invocation
            .get("budgets")
            .ok_or(SourceCommandError::Invalid("assessment invocation budgets"))?;
        Ok(Self {
            bytes_remaining: MAX_OPERATION_BYTES,
            work_remaining: 4_194_304,
            worker_cpu_seconds: capped(budgets, "worker_cpu_seconds", 3)?,
            worker_address_space_bytes: capped(
                budgets,
                "worker_address_space_bytes",
                1_073_741_824,
            )?,
            deadline,
        })
    }

    /// Lower this same operation's actual remainder; never replenish consumed work.
    pub(crate) fn lower_remaining(&mut self, input: usize, work: usize) -> SourceCommandResult<()> {
        if input == 0 || work == 0 || input > self.bytes_remaining || work > self.work_remaining {
            return Err(SourceCommandError::Invalid(
                "assessment lower input/work budget",
            ));
        }
        self.bytes_remaining = input;
        self.work_remaining = work;
        Ok(())
    }

    /// Select lower per-session resources, without changing the operation deadline.
    /// This setter alone does not account aggregate CPU across multiple sessions.
    pub(crate) fn lower_worker_resources(
        &mut self,
        cpu_seconds: u64,
        address_bytes: u64,
    ) -> SourceCommandResult<()> {
        if cpu_seconds == 0
            || address_bytes == 0
            || cpu_seconds > self.worker_cpu_seconds
            || address_bytes > self.worker_address_space_bytes
        {
            return Err(SourceCommandError::Invalid(
                "assessment lower worker resources",
            ));
        }
        self.worker_cpu_seconds = cpu_seconds;
        self.worker_address_space_bytes = address_bytes;
        Ok(())
    }

    pub(crate) fn charge_bytes(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.bytes_remaining =
            self.bytes_remaining
                .checked_sub(bytes)
                .ok_or(SourceCommandError::Invalid(
                    "assessment cumulative byte budget",
                ))?;
        if Instant::now() >= self.deadline {
            return Err(SourceCommandError::Denied("assessment operation deadline"));
        }
        Ok(())
    }

    pub(crate) fn limits(&self) -> SourceCommandResult<AssessmentLimits> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || self.work_remaining == 0 || self.bytes_remaining == 0 {
            return Err(SourceCommandError::Denied(
                "assessment operation budget expired",
            ));
        }
        let mut batch = tos_validation::executor::BatchBudget::laboratory();
        batch.total_execution_wall = remaining;
        batch.startup_wall = batch.startup_wall.min(remaining);
        batch.per_unit_wall = batch.per_unit_wall.min(remaining);
        batch.cpu_seconds = self.worker_cpu_seconds;
        batch.address_space_bytes = self.worker_address_space_bytes;
        batch.max_total_raw_bytes = batch.max_total_raw_bytes.min(self.bytes_remaining);
        Ok(AssessmentLimits {
            max_input_bytes: self.bytes_remaining.min(8_388_608),
            max_work: self.work_remaining.min(4_194_304),
            batch,
            deadline: self.deadline,
        })
    }

    pub(crate) fn charge_report(
        &mut self,
        report: &AssessmentMechanicsReport,
    ) -> SourceCommandResult<()> {
        self.bytes_remaining = self
            .bytes_remaining
            .checked_sub(report.input_bytes_used())
            .ok_or(SourceCommandError::Invalid(
                "assessment cumulative input budget",
            ))?;
        self.work_remaining = self.work_remaining.checked_sub(report.work_used()).ok_or(
            SourceCommandError::Invalid("assessment cumulative work budget"),
        )?;
        Ok(())
    }

    pub(crate) fn charge_work(&mut self, work: usize) -> SourceCommandResult<()> {
        self.work_remaining =
            self.work_remaining
                .checked_sub(work)
                .ok_or(SourceCommandError::Invalid(
                    "assessment cumulative work budget",
                ))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerVersion {
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
}

impl OwnerVersion {
    pub(crate) fn from_config(config: &JsonValue) -> SourceCommandResult<Self> {
        Self::from_schema(cmd::text(config, "schema_version")?)
    }

    fn from_schema(schema: &str) -> SourceCommandResult<Self> {
        match schema {
            "tos_local_assessment_owner_v1" => Ok(Self::V1),
            "tos_local_assessment_owner_v2" => Ok(Self::V2),
            "tos_local_assessment_owner_v3" => Ok(Self::V3),
            "tos_local_assessment_owner_v4" => Ok(Self::V4),
            "tos_local_assessment_owner_v5" => Ok(Self::V5),
            "tos_local_assessment_owner_v6" => Ok(Self::V6),
            _ => Err(SourceCommandError::Denied(
                "private assessment owner v4/v5/v6 required",
            )),
        }
    }

    pub(crate) fn is_private(self) -> bool {
        matches!(self, Self::V4 | Self::V5 | Self::V6)
    }

    pub(crate) fn has_source(self) -> bool {
        !matches!(self, Self::V1)
    }

    pub(crate) fn has_native(self) -> bool {
        !matches!(self, Self::V1 | Self::V2)
    }

    pub(crate) fn has_layer_quality(self) -> bool {
        matches!(self, Self::V5 | Self::V6)
    }
}

/// Exact entry point for the source-native dispatcher. The handler itself
/// reselects every confidential boundary; invocation and request bytes only
/// select the already installed operation family.
pub(super) fn run(
    invocation: &Value,
    request_raw: &[u8],
    store: &CorpusReader,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    active(deadline, cancelled)?;
    let mut budget = OperationBudget::new(invocation, deadline)?;
    budget.charge_bytes(request_raw.len())?;
    let configuration_path = absolute(text(invocation, "owner_config")?)?;
    protected_configuration_parents(&configuration_path, rustix::process::getuid().as_raw())?;

    // The bounded protected hint routes code and performs pure profile
    // preflight before any owner context or selected source is read. The
    // protected selector below rereads and holds these exact bytes through
    // return; a change between preflight and selection conflicts.
    let hint_raw = crate::source_text_owner::read_absolute(
        &configuration_path,
        rustix::process::getuid().as_raw(),
        true,
        MAX_CONFIG_BYTES,
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(hint_raw.len())?;
    let hint = cmd::parse(&hint_raw)?;
    let version = OwnerVersion::from_config(&hint)?;
    let request = parse_request(request_raw, version)?;
    if cmd::canonical(&hint)?.len() > MAX_CONFIG_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment canonical owner configuration budget",
        ));
    }
    preflight_configuration(&hint, version)?;
    let layer_preflight = if version.has_layer_quality() {
        Some(
            crate::source_private_assessment_layers::preflight_owner_local_layers(
                &hint,
                cmd::field(&hint, "subjects")?,
                cmd::text(&hint, "schema_version")?,
            )?,
        )
    } else {
        None
    };

    let context_path = absolute(text(invocation, "owner_context")?)?;
    let mut worker = selected_schema_with_profile(
        invocation,
        cut,
        "assessment_schema_worker",
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
        deadline,
        cancelled,
    )?;
    crate::source_sign::require_assessment_profile(&worker)?;
    let context_schema_path = RelativePath::parse(OWNER_CONTEXT_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("assessment context schema path"))?;
    let context_schema = cut
        .read_member(
            cut.current().revision(),
            &context_schema_path,
            1_048_576,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("assessment context schema selection"))?;
    if worker.contract_digest(OWNER_CONTEXT_SCHEMA)
        != Some(Digest256::of_bytes(&context_schema.raw))
    {
        return Err(SourceCommandError::Conflict(
            "assessment context schema worker differs from the current cut",
        ));
    }
    let (owner_context, context_configuration) = OwnerTextContext::select(
        &context_path,
        &context_schema.raw,
        &mut worker,
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(cmd::canonical(&context_configuration)?.len())?;
    let owner = if version.is_private() {
        ProtectedAssessmentJournal::select_owner_local(
            &configuration_path,
            &context_path,
            owner_context.private_root(),
            deadline,
            cancelled,
        )?
    } else {
        ProtectedAssessmentJournal::select_public_command(
            &configuration_path,
            owner_context.public_root(),
            deadline,
            cancelled,
        )?
    };
    budget.charge_bytes(owner.configuration_raw().len())?;
    let config = owner.configuration();
    if owner.configuration_raw() != hint_raw.as_slice()
        || OwnerVersion::from_config(config)? != version
    {
        return Err(SourceCommandError::Conflict(
            "assessment owner configuration changed during preflight",
        ));
    }
    if cmd::canonical(config)?.len() > MAX_CONFIG_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment canonical owner configuration budget",
        ));
    }
    let context = selected_command_context(
        cut,
        &owner.configuration_raw(),
        request_raw,
        context_configuration.clone(),
        &mut budget,
        deadline,
        cancelled,
    )?;
    let empty = JsonValue::Array(Vec::new());
    let private_sources = crate::source_private_assessment_sources::select_owner_local_sources(
        &owner_context,
        &context_configuration,
        if version.is_private() {
            cmd::field(config, "owner_local_source_records")?
        } else {
            &empty
        },
        if version.is_private() {
            cmd::field(config, "native_text_units")?
        } else {
            &empty
        },
        if version.is_private() {
            config.object_get("owner_local_source_claims")
        } else {
            None
        },
        &context,
        cut,
        &mut worker,
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(private_sources.input_bytes()?)?;
    if private_sources.records.len() + private_sources.native_records.len() > MAX_ASSESSMENTS {
        return Err(SourceCommandError::Invalid(
            "assessment private source record budget",
        ));
    }
    let operation = SelectedOperation {
        cut,
        owner,
        owner_context,
        selected_context: context_configuration,
        context,
        worker,
        request,
        version,
        private_sources,
        layer_preflight,
        budget: Some(budget),
    };
    run_selected(operation, store, software, components, deadline, cancelled)
}

#[derive(Clone)]
struct PublicV2FormSelection {
    form_id: String,
    form_ref: JsonValue,
    subject_id: String,
    subject_ref: JsonValue,
    source_path: String,
    form_path: String,
}

/// Dedicated native consumer for one caller-owned set of public-v2 forms.
/// This intentionally bypasses the v1-v6 command/fence route: it selects only
/// public v2, owns a no-lock read batch, and returns nothing until both passes
/// and all source, grammar, publication, configuration, and head guards hold.
pub(crate) fn run_public_v2_batch(
    invocation: &Value,
    request_raw: &[u8],
    _store: &CorpusReader,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    expected_source_root: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    active(deadline, cancelled)?;
    let selections = parse_public_v2_form_request(request_raw)?;
    let mut budget = OperationBudget::new(invocation, deadline)?;
    budget.charge_bytes(request_raw.len())?;

    let configuration_path = absolute(text(invocation, "owner_config")?)?;
    protected_configuration_parents(&configuration_path, rustix::process::getuid().as_raw())?;
    let hint_raw = crate::source_text_owner::read_absolute(
        &configuration_path,
        rustix::process::getuid().as_raw(),
        true,
        MAX_CONFIG_BYTES,
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(hint_raw.len())?;
    let hint = cmd::parse(&hint_raw)?;
    if OwnerVersion::from_config(&hint)? != OwnerVersion::V2 {
        return Err(SourceCommandError::Denied(
            "assessed-form batch requires public assessment owner v2",
        ));
    }
    preflight_configuration(&hint, OwnerVersion::V2)?;
    if cmd::canonical(&hint)?.len() > MAX_CONFIG_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment canonical owner configuration budget",
        ));
    }

    let source_root = absolute(cmd::text(&hint, "source_root")?)?;
    if expected_source_root.is_some_and(|expected| expected != source_root) {
        return Err(SourceCommandError::Denied(
            "assessed candidate owner source root differs from captured repository",
        ));
    }
    let owner =
        ProtectedAssessmentJournal::select(&configuration_path, &source_root, deadline, cancelled)?;
    budget.charge_bytes(owner.configuration_raw().len())?;
    if owner.configuration_raw() != hint_raw.as_slice()
        || OwnerVersion::from_config(owner.configuration())? != OwnerVersion::V2
    {
        return Err(SourceCommandError::Conflict(
            "public assessment owner changed during batch preflight",
        ));
    }
    let config = owner.configuration().clone();
    if cmd::canonical(&config)?.len() > MAX_CONFIG_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment canonical owner configuration budget",
        ));
    }

    let subject_ids = selections
        .iter()
        .map(|selection| selection.form_id.clone())
        .collect::<Vec<_>>();
    let journal = owner.read_only_subjects(&subject_ids, deadline, cancelled)?;

    let mut worker = selected_schema_with_profile(
        invocation,
        cut,
        "assessment_schema_worker",
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
        deadline,
        cancelled,
    )?;
    crate::source_sign::require_assessment_profile(&worker)?;
    let context = selected_command_context(
        cut,
        owner.configuration_raw(),
        request_raw,
        JsonValue::Null,
        &mut budget,
        deadline,
        cancelled,
    )?;
    context.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    owner.verify_current(deadline, cancelled)?;

    let mut public_sources = crate::source_sign::select_owner_assessment_public_sources(
        &source_root,
        cut,
        &context,
        &config,
        &mut worker,
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(public_sources.input_bytes()?)?;
    let mut source_records = Vec::new();
    let mut source_index = BTreeMap::<String, usize>::new();
    let mut resolved_rows = Vec::new();
    let mut resolved_index = BTreeMap::<String, usize>::new();
    for row in &public_sources.rows {
        budget.charge_work(1)?;
        push_resolved(row.clone(), &mut source_records, &mut source_index)?;
        push_resolved(row.clone(), &mut resolved_rows, &mut resolved_index)?;
    }
    if source_records
        .len()
        .checked_add(cmd::array(&config, "records")?.len())
        .is_none_or(|count| count > MAX_ASSESSMENTS)
        || resolved_rows.len() > MAX_ASSESSMENTS
    {
        return Err(SourceCommandError::Invalid(
            "assessment combined public source record budget",
        ));
    }
    let resolved_bytes = cmd::canonical(&JsonValue::Array(resolved_rows.clone()))?.len();
    budget.charge_bytes(resolved_bytes)?;
    if resolved_bytes > MAX_RESOLVED_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment resolved source byte budget",
        ));
    }
    let snapshot = public_v2_owner_snapshot(
        &config,
        owner.configuration_raw(),
        &public_sources.fixity,
        &resolved_rows,
        &public_sources.identity_snapshots,
        &public_sources.claim_dependencies,
        &context,
        &worker,
    )?;

    let mut all_by_id = BTreeMap::<String, JsonValue>::new();
    for row in &resolved_rows {
        all_by_id.insert(cmd::text(row, "id")?.to_owned(), row.clone());
    }
    for row in cmd::array(&config, "records")? {
        let id = cmd::text(row, "id")?.to_owned();
        if all_by_id.contains_key(&id) {
            return Err(SourceCommandError::Invalid(
                "inline assessment record shadows a public selected source",
            ));
        }
        all_by_id.insert(id, row.clone());
    }
    validate_public_v2_form_selections(&selections, &source_index, &all_by_id, &public_sources)?;

    let record_rows = record_rows(&config, &[])?;
    let mut first_histories = Vec::with_capacity(selections.len());
    let mut first_replies = Vec::with_capacity(selections.len());
    for selection in &selections {
        active(deadline, cancelled)?;
        let history = journal.read(
            &selection.form_id,
            &context,
            &mut worker,
            deadline,
            cancelled,
        )?;
        budget.charge_bytes(history.input_bytes()?)?;
        first_replies.push(materialize_public_v2_selection(
            selection,
            &snapshot,
            &config,
            &source_records,
            &source_index,
            &record_rows,
            &all_by_id,
            &history,
            &mut public_sources,
            &mut worker,
            &mut budget,
            cancelled,
            deadline,
        )?);
        first_histories.push(history);
    }

    owner.verify_current(deadline, cancelled)?;
    context.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    public_sources.verify_current(deadline, cancelled)?;
    journal.verify_current(deadline, cancelled)?;

    let mut replay_histories = Vec::with_capacity(selections.len());
    let mut replay_replies = Vec::with_capacity(selections.len());
    for selection in &selections {
        active(deadline, cancelled)?;
        let history = journal.read(
            &selection.form_id,
            &context,
            &mut worker,
            deadline,
            cancelled,
        )?;
        budget.charge_bytes(history.input_bytes()?)?;
        replay_replies.push(materialize_public_v2_selection(
            selection,
            &snapshot,
            &config,
            &source_records,
            &source_index,
            &record_rows,
            &all_by_id,
            &history,
            &mut public_sources,
            &mut worker,
            &mut budget,
            cancelled,
            deadline,
        )?);
        replay_histories.push(history);
    }
    for (first, replay) in first_replies.iter().zip(&replay_replies) {
        if cmd::canonical(first)? != cmd::canonical(replay)? {
            return Err(SourceCommandError::Conflict(
                "public assessment journal or current admission changed during read batch",
            ));
        }
    }

    for history in first_histories.iter().chain(&replay_histories) {
        history.verify_current(&journal, deadline, cancelled)?;
    }
    owner.verify_current(deadline, cancelled)?;
    public_sources.verify_current(deadline, cancelled)?;
    context.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    journal.verify_current(deadline, cancelled)?;
    crate::source_sign::finish_worker(&mut worker, deadline, cancelled)?;

    let result = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_assessed_forms_materialization_result_v1"),
        ),
        ("owner_snapshot", cmd::string(&snapshot)),
        ("replies", JsonValue::Array(first_replies)),
    ]);
    let result_raw = cmd::canonical(&result)?;
    budget.charge_bytes(result_raw.len())?;
    if result_raw.len() > 16 * MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessed-form batch output byte budget",
        ));
    }
    serde_json::from_slice(&result_raw)
        .map_err(|_| SourceCommandError::Invalid("assessed-form batch result transport JSON"))
}

fn parse_public_v2_form_request(raw: &[u8]) -> SourceCommandResult<Vec<PublicV2FormSelection>> {
    if raw.len() > MAX_REQUEST_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessed-form batch request byte budget",
        ));
    }
    let request = cmd::parse(raw)?;
    if cmd::canonical(&request)?.len() > MAX_REQUEST_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessed-form canonical request byte budget",
        ));
    }
    cmd::exact_keys(&request, &["schema_version", "operation", "selections"])?;
    if cmd::text(&request, "schema_version")?
        != "tos_local_assessed_forms_materialization_request_v1"
        || cmd::text(&request, "operation")? != "materialize_assessed_forms"
    {
        return Err(SourceCommandError::Unsupported(
            "public-v2 assessed-form batch request profile",
        ));
    }
    let rows = cmd::array(&request, "selections")?;
    if rows.is_empty() || rows.len() > 256 {
        return Err(SourceCommandError::Invalid(
            "assessed-form batch selection count",
        ));
    }
    let mut selected = BTreeSet::new();
    rows.iter()
        .map(|row| {
            cmd::exact_keys(
                row,
                &["form_ref", "subject_ref", "source_path", "form_path"],
            )?;
            let form_ref = cmd::field(row, "form_ref")?.clone();
            let subject_ref = cmd::field(row, "subject_ref")?.clone();
            validate_public_record_ref(&form_ref)?;
            validate_public_record_ref(&subject_ref)?;
            let form_id = cmd::text(&form_ref, "id")?.to_owned();
            if !selected.insert(form_id.clone()) {
                return Err(SourceCommandError::Invalid(
                    "assessed-form batch identities must be distinct",
                ));
            }
            let subject_id = cmd::text(&subject_ref, "id")?.to_owned();
            let source_path = cmd::text(row, "source_path")?.to_owned();
            let form_path = cmd::text(row, "form_path")?.to_owned();
            if source_path.is_empty()
                || source_path.len() > MAX_RECORD_BYTES
                || form_path.is_empty()
                || form_path.len() > MAX_RECORD_BYTES
                || RelativePath::parse(&source_path).is_err()
                || RelativePath::parse(&form_path).is_err()
            {
                return Err(SourceCommandError::Invalid(
                    "assessed-form batch source path",
                ));
            }
            Ok(PublicV2FormSelection {
                form_id,
                form_ref,
                subject_id,
                subject_ref,
                source_path,
                form_path,
            })
        })
        .collect()
}

fn validate_public_record_ref(reference: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(reference, &["id", "version", "digest"])?;
    if cmd::text(reference, "id")?.is_empty()
        || cmd::integer(reference, "version")? == 0
        || Digest256::from_prefixed(cmd::text(reference, "digest")?).is_err()
    {
        return Err(SourceCommandError::Invalid(
            "assessed-form batch source reference",
        ));
    }
    Ok(())
}

fn validate_public_v2_form_selections(
    selections: &[PublicV2FormSelection],
    source_index: &BTreeMap<String, usize>,
    all_by_id: &BTreeMap<String, JsonValue>,
    public_sources: &crate::source_sign::OwnerAssessmentPublicSources<'_>,
) -> SourceCommandResult<()> {
    for selection in selections {
        if !source_index.contains_key(&selection.form_id)
            || !source_index.contains_key(&selection.subject_id)
            || public_sources.record_paths.get(&selection.form_id) != Some(&selection.form_path)
            || public_sources.form_paths.get(&selection.form_id) != Some(&selection.form_path)
            || public_sources.record_paths.get(&selection.subject_id)
                != Some(&selection.source_path)
        {
            return Err(SourceCommandError::Denied(
                "assessed-form batch paths are outside selected public source records",
            ));
        }
        let form = all_by_id
            .get(&selection.form_id)
            .ok_or(SourceCommandError::Conflict(
                "selected source form is absent",
            ))?;
        let subject = all_by_id
            .get(&selection.subject_id)
            .ok_or(SourceCommandError::Conflict(
                "selected form subject is absent",
            ))?;
        if !cmd::same(&owner_reference(form)?, &selection.form_ref)?
            || !cmd::same(&owner_reference(subject)?, &selection.subject_ref)?
            || cmd::text(cmd::field(form, "payload")?, "schema_version")? != "tos_human_form_v1"
            || !cmd::same(
                cmd::field(cmd::field(form, "payload")?, "subject")?,
                &selection.subject_ref,
            )?
        {
            return Err(SourceCommandError::Conflict(
                "assessed-form batch references differ from current selected records",
            ));
        }
        let body = cmd::field(form, "payload")?;
        if cmd::text(body, "form_id")? != selection.form_id
            || cmd::integer(body, "form_version")? != cmd::integer(form, "version")?
        {
            return Err(SourceCommandError::Conflict(
                "assessed-form identity differs from its selected record",
            ));
        }
        let bindings = cmd::field(body, "bindings")?
            .as_object()
            .ok_or(SourceCommandError::Invalid("assessed-form bindings"))?;
        if bindings.iter().any(|(_, binding)| {
            binding
                .object_get("record")
                .and_then(|record| record.object_get("id"))
                .and_then(JsonValue::as_str)
                .is_none_or(|identity| !source_index.contains_key(identity))
        }) {
            return Err(SourceCommandError::Denied(
                "assessed-form binding is not explicitly source-selected",
            ));
        }
    }
    Ok(())
}

fn public_v2_owner_snapshot(
    config: &JsonValue,
    configuration_raw: &[u8],
    fixity: &[JsonValue],
    resolved_rows: &[JsonValue],
    identity_snapshots: &BTreeMap<String, JsonValue>,
    claim_dependencies: &BTreeMap<String, Vec<JsonValue>>,
    context: &CommandContext,
    worker: &CutWorkerSchemaExecutor,
) -> SourceCommandResult<String> {
    let mut entries = BTreeMap::<String, JsonValue>::new();
    entries.insert("configuration".to_owned(), config.clone());
    entries.insert("source_files".to_owned(), JsonValue::Array(fixity.to_vec()));
    entries.insert(
        "resolved_records".to_owned(),
        JsonValue::Array(resolved_rows.to_vec()),
    );
    entries.extend(identity_snapshots.clone());
    if !claim_dependencies.is_empty() {
        entries.insert(
            "public_claim_dependencies".to_owned(),
            JsonValue::Object(
                claim_dependencies
                    .iter()
                    .map(|(identity, rows)| {
                        (
                            JsonString::from_utf8(identity),
                            JsonValue::Array(rows.clone()),
                        )
                    })
                    .collect(),
            ),
        );
    }
    entries.insert(
        "configuration_bytes".to_owned(),
        cmd::string(&Digest256::of_bytes(configuration_raw).to_hex()),
    );
    entries.insert(
        "assessment_contract_files".to_owned(),
        JsonValue::Array(
            context
                .files
                .iter()
                .map(|file| {
                    cmd::object(vec![
                        ("path", cmd::string(file.path.as_str())),
                        (
                            "sha256",
                            cmd::string(&Digest256::of_bytes(&file.raw).to_prefixed()),
                        ),
                    ])
                })
                .collect(),
        ),
    );
    let execution = worker.execution_binding();
    entries.insert(
        "assessment_schema_execution".to_owned(),
        cmd::object(vec![
            (
                "profile",
                cmd::string(&format!("{:?}", execution.schema_profile)),
            ),
            (
                "schema_set_sha256",
                cmd::string(&execution.schema_set_sha256.to_prefixed()),
            ),
            (
                "worker_sha256",
                cmd::string(&execution.worker_sha256.to_prefixed()),
            ),
        ]),
    );
    let snapshot = JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(&key), value))
            .collect(),
    );
    Ok(format!(
        "sha256:{}",
        cmd::record_digest(&snapshot)?.to_hex()
    ))
}

#[allow(clippy::too_many_arguments)]
fn materialize_public_v2_selection(
    selection: &PublicV2FormSelection,
    snapshot: &str,
    config: &JsonValue,
    source_records: &[JsonValue],
    source_index: &BTreeMap<String, usize>,
    record_rows: &[JsonValue],
    all_by_id: &BTreeMap<String, JsonValue>,
    history: &AssessmentHistory,
    public_sources: &mut crate::source_sign::OwnerAssessmentPublicSources<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> SourceCommandResult<JsonValue> {
    let parsed = ParsedRequest {
        operation: "materialize-form".to_owned(),
        subject_id: selection.form_id.clone(),
        expected_subject: Some(selection.form_ref.clone()),
        expected_snapshot: Some(snapshot.to_owned()),
        command_id: None,
        expected_revision: None,
        assessments: Vec::new(),
    };
    let (current, scope) = select_current_scope(config, &parsed, all_by_id)?;
    if !source_index.contains_key(&selection.form_id)
        || !source_index.contains_key(&selection.subject_id)
        || scope
            .object_get("form_language_context")
            .is_some_and(|value| !value.is_null())
            && cmd::text(cmd::field(&current, "payload")?, "schema_version")? != "tos_human_form_v1"
    {
        return Err(SourceCommandError::Denied(
            "assessed form is outside the public source-form owner scope",
        ));
    }
    if let Some(language_context) = scope
        .object_get("form_language_context")
        .filter(|value| !value.is_null())
    {
        let identity = cmd::text(cmd::field(language_context, "record")?, "id")?;
        if !source_index.contains_key(identity) {
            return Err(SourceCommandError::Denied(
                "form linguistic context is outside selected public sources",
            ));
        }
    }
    let required_refs = public_sources
        .claim_dependencies
        .get(&selection.form_id)
        .cloned()
        .unwrap_or_default();
    let required_sources = resolve_required_sources(&required_refs, all_by_id)?;
    let native_subjects = if public_sources
        .claim_dependencies
        .contains_key(&selection.form_id)
    {
        required_sources.iter().collect::<Vec<_>>()
    } else {
        source_records.iter().collect::<Vec<_>>()
    };
    let needs_native_read = native_subjects.iter().any(|row| {
        row.object_get("payload").is_some_and(|payload| {
            payload.object_get("native_text_binding").is_some()
                || payload
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                    == Some("tos_occurrence_description_record_v1")
        })
    }) || cmd::field(&current, "payload")?
        .object_get("native_text_binding")
        .is_some()
        || cmd::text(cmd::field(&current, "payload")?, "schema_version")?
            == "tos_occurrence_description_record_v1";
    if needs_native_read {
        return Err(SourceCommandError::Denied(
            "native-bound form materialization requires an exact public text reader",
        ));
    }
    let source_route = if public_sources
        .claim_dependencies
        .contains_key(&selection.form_id)
    {
        AssessmentSourceRoute::SourceBoundClaim
    } else {
        AssessmentSourceRoute::SelectedSource
    };
    let now = crate::source_serialization::instant()?;
    let report = evaluate_selected_subject(
        config,
        &selection.form_id,
        &scope,
        source_records,
        &[],
        record_rows,
        &required_refs,
        &[],
        &history.submissions,
        &[],
        source_route,
        None,
        &now,
        worker,
        budget,
        cancelled,
    )?;
    let materialization = materialize_assessed_form(
        None,
        &selection.form_id,
        worker,
        &current,
        &scope,
        history,
        &report,
        &[],
        None,
        all_by_id,
        &required_sources,
        public_sources,
        None,
        deadline,
        cancelled,
    )?;
    let admission = cmd::field(&materialization, "admission")?.clone();
    let result = cmd::object(vec![
        (
            "revision",
            history
                .head
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("batch_count", cmd::number(history.batches.len() as u64)),
        ("current_admission", admission),
        ("materialization", materialization),
    ]);
    Ok(cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_assessment_result_v1"),
        ),
        ("owner_snapshot", cmd::string(snapshot)),
        ("authentication", cmd::string("local-unix-account")),
        ("result", result),
    ]))
}

pub(crate) fn parse_request(
    raw: &[u8],
    version: OwnerVersion,
) -> SourceCommandResult<ParsedRequest> {
    if raw.len() > MAX_REQUEST_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment request byte budget",
        ));
    }
    let request = cmd::parse(raw)?;
    if cmd::canonical(&request)?.len() > MAX_REQUEST_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment canonical request byte budget",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    let allowed = match version {
        OwnerVersion::V1 | OwnerVersion::V2 | OwnerVersion::V3 | OwnerVersion::V4 => matches!(
            operation,
            "describe" | "inspect" | "append" | "materialize-form"
        ),
        OwnerVersion::V5 | OwnerVersion::V6 => matches!(
            operation,
            "describe" | "inspect" | "append" | "materialize-form" | "read-layer-comparison"
        ),
    };
    if !allowed {
        return Err(SourceCommandError::Unsupported(
            "assessment operation is outside the selected owner version",
        ));
    }
    if cmd::text(&request, "schema_version")? != "tos_local_assessment_command_v1" {
        return Err(SourceCommandError::Invalid(
            "assessment request schema version",
        ));
    }
    let mut keys = vec!["schema_version", "operation", "subject_id"];
    if operation != "describe" {
        keys.extend(["expected_subject", "expected_snapshot"]);
    }
    if operation == "append" {
        keys.extend(["command_id", "expected_revision", "assessments"]);
    }
    cmd::exact_keys(&request, &keys)?;
    let subject_id = cmd::text(&request, "subject_id")?.to_owned();
    if subject_id.is_empty() || subject_id.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment subject identity budget",
        ));
    }
    let expected_subject = if operation == "describe" {
        None
    } else {
        Some(cmd::field(&request, "expected_subject")?.clone())
    };
    let expected_snapshot = if operation == "describe" {
        None
    } else {
        let value = cmd::text(&request, "expected_snapshot")?;
        if !value.starts_with("sha256:") || Digest256::from_prefixed(value).is_err() {
            return Err(SourceCommandError::Invalid("assessment expected snapshot"));
        }
        Some(value.to_owned())
    };
    let (command_id, expected_revision, assessments) = if operation == "append" {
        let command_id = cmd::text(&request, "command_id")?;
        if !cmd::nonblank(command_id) || command_id.len() > 256 {
            return Err(SourceCommandError::Invalid("assessment command identity"));
        }
        let revision = cmd::field(&request, "expected_revision")?;
        let expected_revision = match revision {
            JsonValue::Null => None,
            JsonValue::String(value) => {
                let text = value.as_str().ok_or(SourceCommandError::Invalid(
                    "assessment expected journal revision UTF-8",
                ))?;
                if Digest256::from_hex(text).is_err() {
                    return Err(SourceCommandError::Invalid(
                        "assessment expected journal revision",
                    ));
                }
                Some(text.to_owned())
            }
            _ => {
                return Err(SourceCommandError::Invalid(
                    "assessment expected journal revision",
                ));
            }
        };
        let rows = cmd::array(&request, "assessments")?;
        if rows.is_empty() || rows.len() > MAX_ASSESSMENTS {
            return Err(SourceCommandError::Invalid("assessment event count budget"));
        }
        if rows.iter().any(|row| row.as_object().is_none()) {
            return Err(SourceCommandError::Invalid("assessment event object"));
        }
        (
            Some(command_id.to_owned()),
            Some(expected_revision),
            rows.to_vec(),
        )
    } else {
        (None, None, Vec::new())
    };
    Ok(ParsedRequest {
        operation: operation.to_owned(),
        subject_id,
        expected_subject,
        expected_snapshot,
        command_id,
        expected_revision,
        assessments,
    })
}

pub(crate) fn preflight_configuration(
    config: &JsonValue,
    version: OwnerVersion,
) -> SourceCommandResult<()> {
    if cmd::array(config, "authorities")?.len() > MAX_ASSESSMENTS
        || cmd::array(config, "competencies")?.len() > MAX_ASSESSMENTS
        || cmd::array(config, "records")?.len() > MAX_ASSESSMENTS
        || version.has_source() && cmd::array(config, "source_records")?.len() > MAX_ASSESSMENTS
        || version.is_private() && cmd::array(config, "owner_local_source_records")?.len() > 64
        || version.has_native() && cmd::array(config, "native_text_units")?.len() > 64
    {
        return Err(SourceCommandError::Invalid(
            "assessment configured input count",
        ));
    }
    let subjects = cmd::field(config, "subjects")?
        .as_object()
        .filter(|subjects| !subjects.is_empty() && subjects.len() <= MAX_ASSESSMENTS)
        .ok_or(SourceCommandError::Invalid(
            "assessment configured subjects",
        ))?;
    let mut subject_ids = BTreeSet::new();
    for (identity, scope) in subjects {
        let identity = identity
            .as_str()
            .filter(|identity| !identity.is_empty())
            .ok_or(SourceCommandError::Invalid(
                "assessment configured subject identity",
            ))?;
        if scope.as_object().is_none() {
            return Err(SourceCommandError::Invalid(
                "assessment configured subject row",
            ));
        }
        subject_ids.insert(identity.to_owned());
    }
    if version.has_layer_quality() {
        let dependencies = cmd::field(config, "quality_dependencies")?
            .as_object()
            .filter(|rows| rows.len() <= MAX_ASSESSMENTS)
            .ok_or(SourceCommandError::Invalid(
                "assessment quality dependency map",
            ))?;
        if dependencies.iter().any(|(subject, _)| {
            subject
                .as_str()
                .is_none_or(|subject| !subject_ids.contains(subject))
        }) {
            return Err(SourceCommandError::Invalid(
                "assessment quality dependency subject is not configured",
            ));
        }
        for (subject, entries) in dependencies {
            let subject = subject.as_str().ok_or(SourceCommandError::Invalid(
                "assessment quality subject identity",
            ))?;
            let entries = entries
                .as_array()
                .filter(|entries| entries.len() <= 8)
                .ok_or(SourceCommandError::Invalid(
                    "assessment quality dependency count",
                ))?;
            let mut seen = BTreeSet::new();
            for entry in entries {
                cmd::exact_keys(entry, &["layer_id", "use"])?;
                let id = cmd::text(entry, "layer_id")?;
                let use_name = cmd::text(entry, "use")?;
                if !seen.insert(id.to_owned())
                    || !matches!(
                        use_name,
                        "text-layer:citation"
                            | "text-layer:linguistic-analysis"
                            | "text-layer:semantic-analysis"
                            | "text-layer:search-projection"
                    )
                    || !subject_ids.contains(id)
                {
                    return Err(SourceCommandError::Invalid(
                        "assessment quality dependency identity or purpose",
                    ));
                }
            }
            let _ = subject;
        }
    }
    Ok(())
}

fn selected_command_context(
    cut: &CorpusCutReader,
    configuration_raw: &[u8],
    request_raw: &[u8],
    context_configuration: JsonValue,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CommandContext> {
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    let required = [
        OWNER_CONTEXT_SCHEMA,
        BATCH_SCHEMA,
        "ToS/contracts/knowledge-assessment.schema.json",
        "ToS/contracts/knowledge-assessment-policy.schema.json",
        "ToS/contracts/knowledge-assessment-authority.schema.json",
        "ToS/contracts/knowledge-assessment-competence.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/native-text-unit-assessment-subject.schema.json",
        "ToS/contracts/native-text-layer.schema.json",
        "ToS/contracts/native-text-layer-comparison.schema.json",
        "ToS/contracts/native-text-layer-quality-basis.schema.json",
        "ToS/contracts/source-metadata-record.schema.json",
        "ToS/contracts/source-claim-record.schema.json",
        "ToS/contracts/claim-packet.schema.json",
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        // Source-cut schema resources for the actual held TS/C4 consumer.
        // Presence selects bytes only; the caller still needs exact native
        // selection/profile/registry pins and its protected owner frames.
        "ToS/contracts/native-role-bindings.schema.json",
        "ToS/contracts/ts-native-bindings.schema.json",
        "ToS/contracts/ts-native-definition.schema.json",
        "ToS/contracts/ts-native-value-profile.schema.json",
        "ToS/contracts/ts-native-operation-result.schema.json",
        "ToS/contracts/semantic-description-record.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
        "ToS/contracts/source-anchor-v2.schema.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        "ToS/doctrine/semantic-interchange/ts-bindings.v1.json",
    ];
    let mut total = 0usize;
    for name in required {
        active(deadline, cancelled)?;
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("assessment context input path"))?;
        let Some(member) = cut.current().member(&path) else {
            continue;
        };
        let file = cut
            .read_member(
                cut.current().revision(),
                &path,
                1_048_576,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("assessment context source read"))?;
        if file.raw.len() as u64 != member.size_bytes
            || Digest256::of_bytes(&file.raw) != member.sha256
        {
            return Err(SourceCommandError::Conflict(
                "assessment context source member changed",
            ));
        }
        budget.charge_bytes(file.raw.len())?;
        total = total
            .checked_add(file.raw.len())
            .filter(|bytes| *bytes <= MAX_OPERATION_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "assessment context byte budget",
            ))?;
        files.insert(name.to_owned(), file.raw);
    }
    let batch = files
        .get(BATCH_SCHEMA)
        .ok_or(SourceCommandError::Unsupported(
            "assessment batch schema absent",
        ))?;
    let _ = batch;
    let _ = context_configuration;
    Ok(CommandContext {
        base_revision: cut.current().revision(),
        configuration_raw: configuration_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files: files
            .into_iter()
            .map(|(name, raw)| {
                Ok(SourceFile {
                    path: RelativePath::parse(&name)
                        .map_err(|_| SourceCommandError::Invalid("assessment selected path"))?,
                    raw,
                })
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
    })
}

/// Build the maintained Assessment source closure and quality-history lock
/// set from already-selected transport-neutral owner inputs. The caller must
/// acquire exactly these histories under its own journal transport before
/// asking the history evaluator to calculate any admission basis.
pub(crate) fn plan_assessment_evaluation<'a>(
    inputs: AssessmentEvaluationInputs<'a>,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AssessmentEvaluationPlan<'a>> {
    let AssessmentEvaluationInputs {
        config,
        configuration_raw,
        request,
        version,
        resolved,
        public_fixity,
        public_identity_snapshots,
        public_claim_dependencies,
        private_source_snapshot,
        private_native_snapshots,
        private_required_source_refs,
        private_required_languages,
        native_summaries,
        layers,
        layer_preflight,
    } = inputs;
    resolved.check_budgets(budget)?;
    let all_by_id = resolved.all_by_id(config)?;
    let base_snapshot = owner_snapshot(
        config,
        configuration_raw,
        public_fixity,
        &resolved.resolved_rows,
        public_identity_snapshots,
        public_claim_dependencies,
        private_source_snapshot,
        private_native_snapshots,
        layers,
    )?;
    if !version.has_layer_quality()
        && request.operation != "describe"
        && request.expected_snapshot.as_deref() != Some(&base_snapshot)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment owner/source snapshot is stale",
        ));
    }
    let (current, scope) = select_current_scope(config, request, &all_by_id)?;
    if request.operation == "read-layer-comparison"
        && layers
            .and_then(|layers| layers.layers.get(&request.subject_id))
            .is_none()
    {
        return Err(SourceCommandError::Denied(
            "assessment comparison reading requires an exact selected layer",
        ));
    }
    let selected_layer = layers.and_then(|layers| layers.layers.get(&request.subject_id));
    let subject_is_native = resolved.native_index.contains_key(&request.subject_id);
    let subject_is_sourced = resolved.source_index.contains_key(&request.subject_id);
    if subject_is_native
        && selected_layer.is_none()
        && !cmd::array(config, "native_text_units")?
            .iter()
            .any(|selection| {
                selection
                    .object_get("binding")
                    .and_then(|binding| binding.object_get("unit_id"))
                    .and_then(JsonValue::as_str)
                    == Some(request.subject_id.as_str())
            })
    {
        return Err(SourceCommandError::Denied(
            "native supporting evidence is not an assessment target",
        ));
    }
    if selected_layer.is_some() && !subject_is_sourced {
        return Err(SourceCommandError::Denied(
            "selected native layer assessment target is absent from source closure",
        ));
    }
    let mut subject_required_source_refs = if version == OwnerVersion::V3 {
        crate::source_sign::required_sources(
            &request.subject_id,
            public_claim_dependencies,
            &resolved.resolved_rows,
            native_summaries,
        )?
    } else {
        required_source_refs(
            &request.subject_id,
            public_claim_dependencies,
            private_required_source_refs,
            selected_layer,
        )?
    };
    append_manual_authoring_sources(
        &current,
        &scope,
        &resolved.source_records,
        &mut subject_required_source_refs,
        budget,
        deadline,
        cancelled,
    )?;
    let required_sources = resolve_required_sources(&subject_required_source_refs, &all_by_id)?;
    let source_route = if selected_layer.is_some() {
        AssessmentSourceRoute::LayerQuality
    } else if public_claim_dependencies.contains_key(&request.subject_id)
        || private_required_source_refs.contains_key(&request.subject_id)
    {
        AssessmentSourceRoute::SourceBoundClaim
    } else {
        AssessmentSourceRoute::SelectedSource
    };
    let mut subject_quality_requirements = Vec::new();
    if version.has_layer_quality() {
        subject_quality_requirements = quality_requirements(
            &current,
            &required_sources,
            &all_by_id,
            config,
            layers.ok_or(SourceCommandError::Invalid(
                "selected native layer source snapshot absent",
            ))?,
            cmd::text(&scope, "assertion_layer")?,
            budget,
            cancelled,
        )?;
    }
    let parent_context = if version.has_layer_quality()
        && subject_is_sourced
        && cmd::text(&current, "id").is_ok()
        && cmd::text(cmd::field(&current, "payload")?, "schema_version")? == "tos_human_form_v1"
    {
        parent_claim_context(
            &current,
            &scope,
            config,
            &all_by_id,
            &required_sources,
            public_claim_dependencies,
            private_required_source_refs,
            private_required_languages,
            native_summaries,
        )?
    } else {
        None
    };
    let mut parent_quality_requirements = Vec::new();
    if let Some(parent) = &parent_context {
        parent_quality_requirements = quality_requirements(
            &parent.record,
            &parent.required_sources,
            &all_by_id,
            config,
            layers.ok_or(SourceCommandError::Invalid(
                "selected native layer source snapshot absent",
            ))?,
            cmd::text(&parent.scope, "assertion_layer")?,
            budget,
            cancelled,
        )?;
        if parent_quality_requirements
            .iter()
            .any(|entry| !subject_quality_requirements.contains(entry))
        {
            return Err(SourceCommandError::Denied(
                "parent Claim quality is outside the selected form closure",
            ));
        }
    }
    let mut lock_subjects = vec![request.subject_id.clone()];
    let roots = subject_quality_requirements
        .iter()
        .chain(parent_quality_requirements.iter())
        .map(|requirement| requirement.layer_id.clone())
        .collect::<Vec<_>>();
    lock_subjects.extend(collect_quality_lock_closure(
        &roots,
        layer_preflight,
        budget,
        deadline,
        cancelled,
    )?);
    if let Some(parent) = &parent_context {
        lock_subjects.push(cmd::text(&parent.record, "id")?.to_owned());
    }
    let mut distinct_locks = BTreeSet::new();
    lock_subjects.retain(|identity| distinct_locks.insert(identity.clone()));
    if lock_subjects.len() > 65 {
        return Err(SourceCommandError::Invalid(
            "assessment quality lock subject budget",
        ));
    }
    Ok(AssessmentEvaluationPlan {
        config,
        request,
        version,
        resolved,
        all_by_id,
        base_snapshot,
        current,
        scope,
        subject_required_source_refs,
        required_sources,
        source_route,
        subject_quality_requirements,
        parent_context,
        parent_quality_requirements,
        lock_subjects,
        public_claim_dependencies,
        private_required_source_refs,
        layers,
    })
}

/// Evaluate a source-neutral plan against an already-held, exact subject
/// history closure. The transport owns locking, history selection, and final
/// source-currentness checks; this function owns the maintained assessment,
/// quality-basis, and parent-Claim semantics.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_assessment_histories<W, F>(
    plan: &AssessmentEvaluationPlan<'_>,
    histories: &BTreeMap<String, AssessmentHistory>,
    observed_now: &str,
    source_revision: Option<SourceRevision>,
    source_identity: Digest256,
    worker: &mut W,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
    evaluate: &mut F,
) -> SourceCommandResult<AssessmentEvaluationResult>
where
    W: CutSchemaExecutor,
    F: FnMut(
        &AssessmentReadInput,
        &mut W,
        AssessmentLimits,
        &AtomicBool,
    ) -> SourceCommandResult<AssessmentMechanicsReport>,
{
    // The generic assessment lane has no held PWE exact-basis host. Shape,
    // source presence and a general review must never stand in for mandatory
    // PWE role/dependency/application dispatch. Actual PWE hosts assess only
    // through their private NativeConsumerOwner after those checks.
    for subject in std::iter::once(&plan.current)
        .chain(plan.parent_context.as_ref().map(|parent| &parent.record))
    {
        refuse_generic_pwe_subject(subject, budget, cancelled)?;
    }
    let config = plan.config;
    let request = plan.request;
    let version = plan.version;
    let source_records = &plan.resolved.source_records;
    let native_records = &plan.resolved.native_records;
    let all_by_id = &plan.all_by_id;
    let selected_layer = plan
        .layers
        .and_then(|layers| layers.layers.get(&request.subject_id));
    let mut current_snapshot = plan.base_snapshot.clone();
    let owner_record_rows = record_rows(config, &[])?;
    let mut quality_bases = Vec::new();
    for requirement in &plan.subject_quality_requirements {
        let layer = plan
            .layers
            .and_then(|layers| layers.layers.get(&requirement.layer_id))
            .ok_or(SourceCommandError::Denied(
                "assessment quality dependency has no exact selected source layer",
            ))?;
        let dependency_scope = configured_scope(config, &requirement.layer_id)?;
        if cmd::text(&dependency_scope, "requested_use")? != requirement.use_name {
            return Err(SourceCommandError::Denied(
                "assessment quality dependency purpose differs from selected scope",
            ));
        }
        let history = histories
            .get(&requirement.layer_id)
            .ok_or(SourceCommandError::Invalid(
                "assessment quality history is outside held closure",
            ))?;
        let comparison_refs = required_source_refs(
            &requirement.layer_id,
            plan.public_claim_dependencies,
            plan.private_required_source_refs,
            Some(layer),
        )?;
        let report = evaluate_selected_subject_with_executor(
            config,
            &requirement.layer_id,
            &dependency_scope,
            source_records,
            native_records,
            &owner_record_rows,
            &comparison_refs,
            &[],
            &history.submissions,
            &[],
            AssessmentSourceRoute::LayerQuality,
            Some(AssessmentLayerQualityObservation {
                source_comparison_required: true,
                source_comparison_present: layer.comparison_record.is_some(),
                positive_use_allowed: layer.positive_use_allowed,
            }),
            observed_now,
            source_revision.clone(),
            source_identity,
            worker,
            budget,
            cancelled,
            evaluate,
        )?;
        let basis = quality_basis(
            layer,
            &report,
            &requirement.use_name,
            worker,
            budget.deadline,
            cancelled,
        )?;
        if all_by_id.contains_key(cmd::text(&basis, "id")?) {
            return Err(SourceCommandError::Denied(
                "derived quality basis conflicts with an owner source record",
            ));
        }
        quality_bases.push((requirement.clone(), basis));
    }
    if version.has_layer_quality() {
        current_snapshot = quality_snapshot(&plan.base_snapshot, &quality_bases)?;
    }
    let subject_history = histories
        .get(&request.subject_id)
        .ok_or(SourceCommandError::Invalid(
            "assessment subject history absent",
        ))?;
    let admission_bases = quality_bases
        .iter()
        .map(|(requirement, basis)| {
            let (can_use, limits) = basis_observation(basis)?;
            Ok((requirement.clone(), record_input(basis)?, can_use, limits))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let record_rows = record_rows(
        config,
        &quality_bases
            .iter()
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>(),
    )?;
    let route_layer_quality = selected_layer.map(|layer| AssessmentLayerQualityObservation {
        source_comparison_required: true,
        source_comparison_present: layer.comparison_record.is_some(),
        positive_use_allowed: layer.positive_use_allowed,
    });
    if request.operation != "describe"
        && plan.parent_context.is_none()
        && request.expected_snapshot.as_deref() != Some(&current_snapshot)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment current owner/source snapshot is stale",
        ));
    }
    let trusted_history = subject_history.submissions.clone();
    let reviews = if request.operation == "append" {
        append_submissions(config, &plan.scope, &request.assessments)?
    } else {
        Vec::new()
    };
    let admission_records = admission_bases
        .iter()
        .map(|(_, basis, _, _)| basis.clone())
        .collect::<Vec<_>>();
    let current_report = evaluate_selected_subject_with_executor(
        config,
        &request.subject_id,
        &plan.scope,
        source_records,
        native_records,
        &record_rows,
        &plan.subject_required_source_refs,
        &admission_records,
        &trusted_history,
        &[],
        plan.source_route,
        route_layer_quality,
        observed_now,
        source_revision.clone(),
        source_identity,
        worker,
        budget,
        cancelled,
        evaluate,
    )?;
    let append_report = if request.operation == "append" {
        Some(evaluate_selected_subject_with_executor(
            config,
            &request.subject_id,
            &plan.scope,
            source_records,
            native_records,
            &record_rows,
            &plan.subject_required_source_refs,
            &admission_records,
            &trusted_history,
            &reviews,
            plan.source_route,
            route_layer_quality,
            observed_now,
            source_revision.clone(),
            source_identity,
            worker,
            budget,
            cancelled,
            evaluate,
        )?)
    } else {
        None
    };
    if !current_report.source_read_ready() && request.operation == "append" {
        return Err(SourceCommandError::Denied(
            "native-bound assessment append requires exact current source read",
        ));
    }
    let parent_assessment = if let Some(parent) = &plan.parent_context {
        let id = cmd::text(&parent.record, "id")?;
        let parent_history = histories.get(id).ok_or(SourceCommandError::Invalid(
            "assessment parent Claim history absent",
        ))?;
        let parent_bases = quality_bases
            .iter()
            .filter(|(requirement, _)| plan.parent_quality_requirements.contains(requirement))
            .map(|(_, basis)| record_input(basis))
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let parent_required_refs = parent
            .required_sources
            .iter()
            .map(owner_reference)
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let parent_report = evaluate_selected_subject_with_executor(
            config,
            id,
            &parent.scope,
            source_records,
            native_records,
            &record_rows,
            &parent_required_refs,
            &parent_bases,
            &parent_history.submissions,
            &[],
            AssessmentSourceRoute::SourceBoundClaim,
            None,
            observed_now,
            source_revision,
            source_identity,
            worker,
            budget,
            cancelled,
            evaluate,
        )?;
        let packet = parent_assessment_packet(
            &parent.record,
            &parent_report,
            parent_history,
            &parent.scope,
        )?;
        if version.has_layer_quality() {
            current_snapshot = extend_subject_assessment_snapshot(&current_snapshot, &packet)?;
        }
        Some(packet)
    } else {
        None
    };
    if version.has_layer_quality()
        && request.operation != "describe"
        && request.expected_snapshot.as_deref() != Some(&current_snapshot)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment current quality basis snapshot is stale",
        ));
    }
    Ok(AssessmentEvaluationResult {
        current_snapshot,
        quality_bases,
        admission_bases,
        source_read_ready: current_report.source_read_ready(),
        current_report,
        append_report,
        parent_assessment,
    })
}

fn run_selected(
    mut selected: SelectedOperation<'_>,
    _store: &CorpusReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    active(deadline, cancelled)?;
    let mut budget = selected.budget.take().ok_or(SourceCommandError::Invalid(
        "assessment operation budget absent",
    ))?;
    selected.context.check_from_selected_captures(
        selected.cut,
        software,
        components,
        deadline,
        cancelled,
    )?;
    selected.owner.verify_current(deadline, cancelled)?;
    let config = selected.owner.configuration().clone();
    let mut public_sources = crate::source_sign::select_owner_assessment_public_sources(
        selected.owner_context.public_root(),
        selected.cut,
        &selected.context,
        &config,
        &mut selected.worker,
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(public_sources.input_bytes()?)?;
    let mut resolved = AssessmentResolvedSources::from_selected_rows(
        &public_sources.rows,
        &selected.private_sources.records,
        &public_sources.native_rows,
        &selected.private_sources.native_records,
        &mut budget,
    )?;
    if !selected.version.is_private() {
        selected.private_sources.native_summaries = public_sources.native_summaries.clone();
    }
    let layers = if selected.version.has_layer_quality() {
        Some(
            crate::source_private_assessment_layers::select_owner_local_layers(
                &mut selected.owner_context,
                &selected.selected_context,
                &config,
                &resolved
                    .source_records
                    .iter()
                    .map(record_input)
                    .collect::<SourceCommandResult<Vec<_>>>()?,
                selected.cut,
                &mut selected.worker,
                deadline,
                cancelled,
            )?,
        )
    } else {
        None
    };
    if let Some(layer_sources) = &layers {
        budget.charge_bytes(usize::try_from(layer_sources.input_bytes()).map_err(|_| {
            SourceCommandError::Invalid("assessment layer input byte conversion")
        })?)?;
        resolved.add_layer_records(&layer_sources.records(), &mut budget)?;
    }
    let public_fixity = public_sources.fixity.clone();
    let public_identity_snapshots = public_sources.identity_snapshots.clone();
    let public_claim_dependencies = public_sources.claim_dependencies.clone();
    let plan = plan_assessment_evaluation(
        AssessmentEvaluationInputs {
            config: &config,
            configuration_raw: selected.owner.configuration_raw(),
            request: &selected.request,
            version: selected.version,
            resolved,
            public_fixity: &public_fixity,
            public_identity_snapshots: &public_identity_snapshots,
            public_claim_dependencies: &public_claim_dependencies,
            private_source_snapshot: &selected.private_sources.owner_local_sources_snapshot,
            private_native_snapshots: &selected.private_sources.native_snapshots,
            private_required_source_refs: selected.private_sources.required_source_refs(),
            private_required_languages: selected.private_sources.required_languages(),
            native_summaries: &selected.private_sources.native_summaries,
            layers: layers.as_ref(),
            layer_preflight: selected.layer_preflight.as_ref(),
        },
        &mut budget,
        deadline,
        cancelled,
    )?;
    let now = selected.context.recorded_at.clone();
    let fence = selected
        .owner
        .lock_subjects(&plan.lock_subjects, deadline, cancelled)?;
    let mut histories = BTreeMap::new();
    for subject_id in &plan.lock_subjects {
        active(deadline, cancelled)?;
        budget.charge_work(1)?;
        let history = fence.read(
            subject_id,
            &selected.context,
            &mut selected.worker,
            deadline,
            cancelled,
        )?;
        budget.charge_bytes(history.input_bytes()?)?;
        budget.charge_work(
            history
                .submissions
                .len()
                .saturating_add(history.batches.len()),
        )?;
        histories.insert(subject_id.clone(), history);
    }
    verify_selected_sources(
        &SelectedOperationView {
            cut: selected.cut,
            owner: &selected.owner,
            owner_context: &selected.owner_context,
            context: &selected.context,
            request: &selected.request,
            version: selected.version,
            private_sources: &selected.private_sources,
        },
        &mut public_sources,
        layers.as_ref(),
        software,
        components,
        deadline,
        cancelled,
    )?;
    let source_revision = selected.worker.source_revision();
    let mut evaluate = |input: &AssessmentReadInput,
                        worker: &mut CutWorkerSchemaExecutor,
                        limits: AssessmentLimits,
                        cancelled: &AtomicBool| {
        evaluate_current_assessment(input, worker, limits, cancelled)
            .map_err(map_assessment_refusal)
    };
    let evaluation = evaluate_assessment_histories(
        &plan,
        &histories,
        &now,
        Some(source_revision.clone()),
        source_revision.0,
        &mut selected.worker,
        &mut budget,
        cancelled,
        &mut evaluate,
    )?;
    let current_snapshot = evaluation.current_snapshot;
    let quality_bases = evaluation.quality_bases;
    let admission_bases = evaluation.admission_bases;
    let current_report = evaluation.current_report;
    let append_report = evaluation.append_report;
    let parent_assessment = evaluation.parent_assessment;
    let current = &plan.current;
    let scope = &plan.scope;
    let all_by_id = &plan.all_by_id;
    let required_sources = &plan.required_sources;
    let subject_quality_requirements = &plan.subject_quality_requirements;
    let selected_layer = plan
        .layers
        .and_then(|layers| layers.layers.get(&selected.request.subject_id));
    let subject_history =
        histories
            .get(&selected.request.subject_id)
            .ok_or(SourceCommandError::Invalid(
                "assessment subject history absent",
            ))?;
    if !current_report.source_read_ready() && selected.request.operation == "append" {
        return Err(SourceCommandError::Denied(
            "native-bound assessment append requires exact current source read",
        ));
    }
    if selected.request.operation == "append"
        && !native_append_ready(
            selected_layer.is_some(),
            public_sources
                .claim_dependencies
                .contains_key(&selected.request.subject_id)
                || selected
                    .private_sources
                    .required_source_refs()
                    .contains_key(&selected.request.subject_id),
            &required_sources,
            &selected.private_sources.native_summaries,
        )?
    {
        return Err(SourceCommandError::Denied(
            "native assessment append requires every selected current source read",
        ));
    }
    verify_selected_sources(
        &SelectedOperationView {
            cut: selected.cut,
            owner: &selected.owner,
            owner_context: &selected.owner_context,
            context: &selected.context,
            request: &selected.request,
            version: selected.version,
            private_sources: &selected.private_sources,
        },
        &mut public_sources,
        layers.as_ref(),
        software,
        components,
        deadline,
        cancelled,
    )?;
    if selected.request.operation == "materialize-form" && !current_report.source_read_ready() {
        return Err(SourceCommandError::Denied(
            "native-bound form materialization requires exact current source read",
        ));
    }
    let selected_view = SelectedOperationView {
        cut: selected.cut,
        owner: &selected.owner,
        owner_context: &selected.owner_context,
        context: &selected.context,
        request: &selected.request,
        version: selected.version,
        private_sources: &selected.private_sources,
    };
    let result = execute_journal_operation(
        &selected_view,
        &mut selected.worker,
        &config,
        &scope,
        &current,
        subject_history,
        &fence,
        &current_report,
        append_report.as_ref(),
        &admission_bases,
        &quality_bases,
        &subject_quality_requirements,
        &parent_assessment,
        &all_by_id,
        &required_sources,
        &current_snapshot,
        current_report.source_read_ready(),
        &mut public_sources,
        layers.as_ref(),
        software,
        components,
        deadline,
        cancelled,
    )?;
    verify_selected_sources(
        &selected_view,
        &mut public_sources,
        layers.as_ref(),
        software,
        components,
        deadline,
        cancelled,
    )?;
    let append_committed = selected.request.operation == "append"
        && result
            .object_get("result")
            .and_then(|inner| inner.object_get("replayed"))
            == Some(&JsonValue::Bool(false));
    for (id, history) in &histories {
        if id == &selected.request.subject_id && append_committed {
            history.verify_batches_current(&fence, deadline, cancelled)?;
        } else {
            history.verify_current(&fence, deadline, cancelled)?;
        }
    }
    if append_committed {
        let result_revision = result
            .object_get("result")
            .and_then(|inner| inner.object_get("revision"))
            .and_then(JsonValue::as_str)
            .ok_or(SourceCommandError::Invalid(
                "assessment append revision absent",
            ))?;
        if fence.head(&selected.request.subject_id, deadline, cancelled)?
            != Some(result_revision.to_owned())
        {
            return Err(SourceCommandError::Conflict(
                "assessment published head changed before return",
            ));
        }
    } else if fence.head(&selected.request.subject_id, deadline, cancelled)? != subject_history.head
    {
        return Err(SourceCommandError::Conflict(
            "assessment held journal head changed before return",
        ));
    }
    fence.verify_current(deadline, cancelled)?;
    let result_bytes = cmd::canonical(&result)?;
    budget.charge_bytes(result_bytes.len())?;
    serde_json::from_slice(&result_bytes)
        .map_err(|_| SourceCommandError::Invalid("assessment result transport JSON"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QualityRequirement {
    pub(crate) layer_id: String,
    pub(crate) use_name: String,
}

pub(crate) struct ParentClaimContext {
    pub(crate) record: JsonValue,
    pub(crate) scope: JsonValue,
    pub(crate) required_sources: Vec<JsonValue>,
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    crate::source_creation_store::active(deadline, cancelled)
}

pub(crate) fn parse_record_input(input: &AssessmentRecordInput) -> SourceCommandResult<JsonValue> {
    cmd::parse(&input.envelope)
}

pub(crate) fn record_input(row: &JsonValue) -> SourceCommandResult<AssessmentRecordInput> {
    let fields = if row.object_get("origin_id").is_some() {
        &["id", "version", "payload", "origin_id"][..]
    } else {
        &["id", "version", "payload"][..]
    };
    cmd::exact_keys(row, fields)?;
    Ok(AssessmentRecordInput {
        envelope: cmd::canonical(&cmd::object(vec![
            ("id", cmd::field(row, "id")?.clone()),
            ("version", cmd::field(row, "version")?.clone()),
            ("payload", cmd::field(row, "payload")?.clone()),
            (
                "origin_id",
                row.object_get("origin_id")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
            ),
        ]))?,
    })
}

pub(crate) fn append_submissions(
    config: &JsonValue,
    scope: &JsonValue,
    assessments: &[JsonValue],
) -> SourceCommandResult<Vec<AssessmentSubmissionInput>> {
    let principal_id = cmd::text(config, "principal_id")?.to_owned();
    let execution_profile = cmd::canonical(cmd::field(config, "execution_profile")?)?;
    let committed_scope = cmd::canonical(&cmd::object(vec![
        (
            "assertion_layer",
            cmd::field(scope, "assertion_layer")?.clone(),
        ),
        ("risk", cmd::field(scope, "risk")?.clone()),
        ("languages", cmd::field(scope, "languages")?.clone()),
        ("maker_id", cmd::field(scope, "maker_id")?.clone()),
        ("requested_use", cmd::field(scope, "requested_use")?.clone()),
    ]))?;
    assessments
        .iter()
        .map(|assessment| {
            if assessment.as_object().is_none() {
                return Err(SourceCommandError::Invalid(
                    "assessment submitted record must be an object",
                ));
            }
            Ok(AssessmentSubmissionInput {
                assessment: cmd::canonical(assessment)?,
                principal_id: principal_id.clone(),
                execution_profile: execution_profile.clone(),
                committed_scope: Some(committed_scope.clone()),
            })
        })
        .collect()
}

fn push_resolved(
    row: JsonValue,
    rows: &mut Vec<JsonValue>,
    index: &mut BTreeMap<String, usize>,
) -> SourceCommandResult<()> {
    let id = cmd::text(&row, "id")?.to_owned();
    if let Some(previous) = index.get(&id).copied() {
        if !cmd::same(&rows[previous], &row)? {
            return Err(SourceCommandError::Conflict(
                "assessment source identity has conflicting current records",
            ));
        }
        return Ok(());
    }
    index.insert(id, rows.len());
    rows.push(row);
    Ok(())
}

pub(crate) fn owner_reference(row: &JsonValue) -> SourceCommandResult<JsonValue> {
    cmd::exact_keys(row, &["id", "version", "payload", "origin_id"])?;
    Ok(cmd::object(vec![
        ("id", cmd::field(row, "id")?.clone()),
        ("version", cmd::field(row, "version")?.clone()),
        (
            "digest",
            cmd::string(&cmd::record_digest(cmd::field(row, "payload")?)?.to_prefixed()),
        ),
    ]))
}

fn configured_scope(config: &JsonValue, subject_id: &str) -> SourceCommandResult<JsonValue> {
    let subjects = cmd::field(config, "subjects")?;
    let scope = subjects
        .object_get(subject_id)
        .ok_or(SourceCommandError::Denied(
            "assessment subject is outside configured scope",
        ))?
        .clone();
    let mut keys = vec![
        "record",
        "assertion_layer",
        "risk",
        "languages",
        "maker_id",
        "requested_use",
        "access_allowed",
    ];
    if OwnerVersion::from_config(config)? != OwnerVersion::V1
        && scope.object_get("form_language_context").is_some()
    {
        keys.push("form_language_context");
    }
    cmd::exact_keys(&scope, &keys)?;
    let languages = cmd::array(&scope, "languages")?;
    if languages.is_empty()
        || languages
            .iter()
            .any(|value| value.as_str().is_none_or(|language| language.is_empty()))
        || ["assertion_layer", "risk", "maker_id", "requested_use"]
            .iter()
            .any(|field| cmd::text(&scope, field).is_err_and(|_| true))
        || cmd::field(&scope, "access_allowed")? != &JsonValue::Bool(true)
    {
        return Err(SourceCommandError::Denied(
            "assessment configured access scope is incomplete",
        ));
    }
    Ok(scope)
}

fn configured_scope_using(
    config: &JsonValue,
    subject_id: &str,
    access: &mut ScopeAccess<'_>,
) -> SourceCommandResult<JsonValue> {
    access.inspect(0, 128)?; // fixed selector controls, not variable key/language work
    let subjects = access.field(config, "subjects")?;
    let selected = access
        .get(subjects, subject_id)?
        .ok_or(SourceCommandError::Denied(
            "assessment subject is outside configured scope",
        ))?;
    let scope = access.copied(selected)?;
    const KEYS: [&str; 8] = [
        "record",
        "assertion_layer",
        "risk",
        "languages",
        "maker_id",
        "requested_use",
        "access_allowed",
        "form_language_context",
    ];
    let extended = access.version(config)? != OwnerVersion::V1
        && access.get(&scope, "form_language_context")?.is_some();
    access.exact_keys(&scope, &KEYS[..if extended { 8 } else { 7 }])?;
    let languages = access
        .field(&scope, "languages")?
        .as_array()
        .ok_or(SourceCommandError::Invalid("expected array field"))?;
    if languages.is_empty() {
        return Err(SourceCommandError::Denied(
            "assessment configured access scope is incomplete",
        ));
    }
    for language in languages {
        access.inspect(0, 1)?;
        if language.as_str().is_none_or(str::is_empty) {
            return Err(SourceCommandError::Denied(
                "assessment configured access scope is incomplete",
            ));
        }
    }
    for key in ["assertion_layer", "risk", "maker_id", "requested_use"] {
        if access
            .get(&scope, key)?
            .and_then(JsonValue::as_str)
            .is_none()
        {
            return Err(SourceCommandError::Denied(
                "assessment configured access scope is incomplete",
            ));
        }
    }
    let allowed = access.field(&scope, "access_allowed")?;
    if !access.optional_equal(Some(allowed), Some(&JsonValue::Bool(true)))? {
        return Err(SourceCommandError::Denied(
            "assessment configured access scope is incomplete",
        ));
    }
    Ok(scope)
}

pub(crate) fn select_current_scope(
    config: &JsonValue,
    request: &ParsedRequest,
    all_by_id: &BTreeMap<String, JsonValue>,
) -> SourceCommandResult<(JsonValue, JsonValue)> {
    let current = all_by_id
        .get(&request.subject_id)
        .ok_or(SourceCommandError::Denied(
            "assessment subject has no exact selected current record",
        ))?
        .clone();
    let current_ref = owner_reference(&current)?;
    let scope = configured_scope(config, &request.subject_id)?;
    if !cmd::same(&current_ref, cmd::field(&scope, "record")?)?
        || request.operation != "describe"
            && !cmd::same(
                &current_ref,
                request
                    .expected_subject
                    .as_ref()
                    .ok_or(SourceCommandError::Invalid(
                        "assessment expected subject is absent",
                    ))?,
            )?
    {
        return Err(SourceCommandError::Conflict(
            "assessment expected subject or owner scope is stale",
        ));
    }
    let body = cmd::field(&current, "payload")?;
    if body.object_get("claim_id").is_some() {
        let maker = body
            .object_get("maker")
            .and_then(|value| value.object_get("agent_ref"));
        if cmd::field(&scope, "assertion_layer")?
            != body
                .object_get("assertion_layer")
                .unwrap_or(&JsonValue::Null)
            || maker != Some(cmd::field(&scope, "maker_id")?)
        {
            return Err(SourceCommandError::Denied(
                "assessment source Claim scope differs from its layer or maker",
            ));
        }
    } else if body
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        == Some("tos_human_form_v1")
        && (cmd::text(&scope, "assertion_layer")? != "human_projection"
            || cmd::field(&scope, "maker_id")? != cmd::field(body, "creator_id")?)
    {
        return Err(SourceCommandError::Denied(
            "assessment source form scope differs from its creator",
        ));
    }
    Ok((current, scope))
}

/// Same maintained selection rules with the caller's original operation/whole
/// ledgers. This returns selected data, never an access grant or admission.
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_current_scope_with_admission(
    config: &JsonValue,
    request: &ParsedRequest,
    all_by_id: &BTreeMap<String, JsonValue>,
    budget: &mut OperationBudget,
    whole: &std::rc::Rc<std::cell::RefCell<crate::source_claims::ClaimCallBudget>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, JsonValue)> {
    select_current_scope_using(
        config,
        request,
        all_by_id,
        &mut ScopeAccess::admitted(budget, whole, deadline, cancelled),
    )
}

fn select_current_scope_using(
    config: &JsonValue,
    request: &ParsedRequest,
    all_by_id: &BTreeMap<String, JsonValue>,
    access: &mut ScopeAccess<'_>,
) -> SourceCommandResult<(JsonValue, JsonValue)> {
    access.inspect(0, 64)?;
    let selected = access.current(all_by_id, &request.subject_id)?;
    let current = access.copied(selected)?;
    let current_ref = access.reference(&current)?;
    let scope = configured_scope_using(config, &request.subject_id, access)?;
    let scope_record = access.field(&scope, "record")?;
    if !access.same(&current_ref, scope_record)?
        || !access.describe(request)?
            && !access.same(
                &current_ref,
                request
                    .expected_subject
                    .as_ref()
                    .ok_or(SourceCommandError::Invalid(
                        "assessment expected subject is absent",
                    ))?,
            )?
    {
        return Err(SourceCommandError::Conflict(
            "assessment expected subject or owner scope is stale",
        ));
    }
    let body = access.field(&current, "payload")?;
    if access.get(body, "claim_id")?.is_some() {
        let maker_object = access.get(body, "maker")?;
        let maker = match maker_object {
            Some(maker) => access.get(maker, "agent_ref")?,
            None => None,
        };
        let expected_layer = access.field(&scope, "assertion_layer")?;
        let actual_layer = access
            .get(body, "assertion_layer")?
            .unwrap_or(&JsonValue::Null);
        let layer_matches = access.optional_equal(Some(expected_layer), Some(actual_layer))?;
        let maker_matches = if layer_matches {
            let expected_maker = access.field(&scope, "maker_id")?;
            access.optional_equal(maker, Some(expected_maker))?
        } else {
            false
        };
        if !layer_matches || !maker_matches {
            return Err(SourceCommandError::Denied(
                "assessment source Claim scope differs from its layer or maker",
            ));
        }
    } else {
        let schema = access
            .get(body, "schema_version")?
            .and_then(JsonValue::as_str);
        let human_form = match schema {
            Some(schema) => access.text_equal(schema, "tos_human_form_v1")?,
            None => false,
        };
        if human_form {
            let layer = access.text(&scope, "assertion_layer")?;
            let layer_matches = access.text_equal(layer, "human_projection")?;
            let maker_matches = if layer_matches {
                let maker = access.field(&scope, "maker_id")?;
                let creator = access.field(body, "creator_id")?;
                access.optional_equal(Some(maker), Some(creator))?
            } else {
                false
            };
            if !layer_matches || !maker_matches {
                return Err(SourceCommandError::Denied(
                    "assessment source form scope differs from its creator",
                ));
            }
        }
    }
    Ok((current, scope))
}

// New manual-use closure is derived ONLY from already source-owned rows, never
// from config.records, request fields or a claimed complete/read bool. The
// selected basis targets an exact existing Claim; the Claim need not acquire a
// circular dependency on its later assessment artifact. Existing uses bypass it.
fn append_manual_authoring_sources(
    current: &JsonValue,
    scope: &JsonValue,
    sources: &[JsonValue],
    required: &mut Vec<JsonValue>,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    struct Access<'a> {
        budget: &'a mut OperationBudget,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    }
    impl Access<'_> {
        fn pay(&mut self, bytes: usize) -> SourceCommandResult<()> {
            active(self.deadline, self.cancelled)?;
            self.budget
                .charge_work(bytes.checked_add(1).ok_or(SourceCommandError::Unsupported(
                    "manual source work overflow",
                ))?)
        }
        fn equal(&mut self, a: &str, b: &str) -> SourceCommandResult<bool> {
            self.pay(
                a.len()
                    .checked_add(b.len())
                    .ok_or(SourceCommandError::Unsupported(
                        "manual source comparison overflow",
                    ))?,
            )?;
            Ok(a == b)
        }
        fn get<'v>(
            &mut self,
            value: &'v JsonValue,
            key: &str,
        ) -> SourceCommandResult<Option<&'v JsonValue>> {
            self.pay(0)?;
            let Some(fields) = value.as_object() else {
                return Err(SourceCommandError::Invalid("manual source object required"));
            };
            for (name, value) in fields {
                self.pay(0)?;
                if let Some(name) = name.as_str() {
                    if self.equal(name, key)? {
                        return Ok(Some(value));
                    }
                }
            }
            Ok(None)
        }
        fn field<'v>(
            &mut self,
            value: &'v JsonValue,
            key: &str,
        ) -> SourceCommandResult<&'v JsonValue> {
            self.get(value, key)?
                .ok_or(SourceCommandError::Invalid("manual source required field"))
        }
        fn text<'v>(&mut self, value: &'v JsonValue, key: &str) -> SourceCommandResult<&'v str> {
            self.field(value, key)?
                .as_str()
                .ok_or(SourceCommandError::Invalid("manual source text field"))
        }
        fn canonical(&mut self, value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
            crate::source_native_roles::admitted_canonical(
                value,
                self.budget,
                self.deadline,
                self.cancelled,
            )
        }
        fn same(&mut self, a: &JsonValue, b: &JsonValue) -> SourceCommandResult<bool> {
            self.pay(0)?;
            if a.as_object().is_none_or(|v| v.len() != 3)
                || b.as_object().is_none_or(|v| v.len() != 3)
            {
                return Err(SourceCommandError::Invalid("manual exact reference shape"));
            }
            for key in ["id", "digest"] {
                let av = self.text(a, key)?;
                let bv = self.text(b, key)?;
                if !self.equal(av, bv)? {
                    return Ok(false);
                }
            }
            let av = self.field(a, "version")?;
            let bv = self.field(b, "version")?;
            let (JsonValue::Number(an), JsonValue::Number(bn)) = (av, bv) else {
                return Err(SourceCommandError::Invalid(
                    "manual exact version is not numeric",
                ));
            };
            let tokens = an.lexeme.len().checked_add(bn.lexeme.len()).ok_or(
                SourceCommandError::Unsupported("manual version work overflow"),
            )?;
            self.pay(
                tokens
                    .checked_mul(16)
                    .ok_or(SourceCommandError::Unsupported(
                        "manual version work overflow",
                    ))?,
            )?;
            // Only the two numeric leaves enter the maintained Python helper;
            // it cannot sort an arbitrary object or apply bool/int coercion.
            // All token scans/copies and integral-float formatting holders are
            // reserved prospectively, preserving 1==1.0 without a u64 cast.
            let holders = tokens
                .checked_add(640)
                .and_then(|n| n.checked_mul(8))
                .ok_or(SourceCommandError::Unsupported(
                    "manual version holder overflow",
                ))?;
            self.budget.charge_bytes(holders)?;
            for number in [an, bn] {
                if number.kind == tos_foundation::JsonNumberKind::Float {
                    let n = number.lexeme.parse::<f64>().map_err(|_| {
                        SourceCommandError::Unsupported("manual numeric representation")
                    })?;
                    if !n.is_finite() {
                        return Err(SourceCommandError::Unsupported("manual nonfinite version"));
                    }
                    let exponent = ((n.to_bits() >> 52) & 0x7ff) as i32;
                    let passes = (exponent - 1023 - 52).max(0) as usize;
                    self.pay((passes + 1) * 3 * 640 + 1024)?;
                }
            }
            tos_foundation::python_equals(av, bv, 1)
                .map_err(|_| SourceCommandError::Unsupported("manual exact numeric equality"))
        }
        fn reference(&mut self, row: &JsonValue) -> SourceCommandResult<JsonValue> {
            let id = self.text(row, "id")?;
            let version = self.field(row, "version")?;
            // A source envelope's existing version is preserved verbatim. No
            // fresh scalar bound or source instance is invented by this port.
            let version_raw = self.canonical(version)?;
            let payload = self.field(row, "payload")?;
            let raw = self.canonical(payload)?;
            self.pay(raw.len())?;
            let digest = Digest256::of_bytes(&raw);
            let holder = id
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(version_raw.len().checked_mul(16)?))
                .and_then(|n| n.checked_add(2048))
                .ok_or(SourceCommandError::Unsupported(
                    "manual exact holder overflow",
                ))?;
            self.budget.charge_bytes(holder)?;
            self.pay(holder)?;
            Ok(cmd::object(vec![
                ("id", cmd::string(id)),
                ("version", version.clone()),
                ("digest", cmd::string(&digest.to_prefixed())),
            ]))
        }
        fn add(
            &mut self,
            reference: &JsonValue,
            sources: &[JsonValue],
            required: &mut Vec<JsonValue>,
        ) -> SourceCommandResult<()> {
            let id = self.text(reference, "id")?;
            let mut found = None;
            for row in sources {
                self.pay(0)?;
                let candidate = self.text(row, "id")?;
                if self.equal(candidate, id)? {
                    found = Some(row);
                    break;
                }
            }
            let row = found.ok_or(SourceCommandError::Unsupported(
                "manual dependency absent from actual source selection",
            ))?;
            let actual = self.reference(row)?;
            if !self.same(&actual, reference)? {
                return Err(SourceCommandError::Conflict(
                    "manual source dependency exact pin differs",
                ));
            }
            for prior in required.iter() {
                self.pay(0)?;
                if self.same(prior, &actual)? {
                    return Ok(());
                }
                let prior_id = self.text(prior, "id")?;
                if self.equal(prior_id, id)? {
                    return Err(SourceCommandError::Conflict(
                        "manual source dependency versions conflict",
                    ));
                }
            }
            let vector_bytes = required
                .len()
                .checked_add(1)
                .and_then(|n| n.checked_mul(std::mem::size_of::<JsonValue>()))
                .and_then(|n| n.checked_mul(2))
                .ok_or(SourceCommandError::Unsupported(
                    "manual reference vector overflow",
                ))?;
            self.budget.charge_bytes(vector_bytes)?;
            self.pay(vector_bytes)?;
            required
                .try_reserve_exact(1)
                .map_err(|_| SourceCommandError::Unsupported("manual source reference holder"))?;
            required.push(actual);
            Ok(())
        }
        fn walk(
            &mut self,
            value: &JsonValue,
            sources: &[JsonValue],
            required: &mut Vec<JsonValue>,
            depth: usize,
        ) -> SourceCommandResult<()> {
            self.pay(0)?;
            if depth > tos_foundation::JsonLimits::default().max_depth {
                return Err(SourceCommandError::Unsupported(
                    "manual source closure depth",
                ));
            }
            match value {
                JsonValue::Object(fields) => {
                    if fields.len() == 3
                        && self.get(value, "id")?.is_some()
                        && self.get(value, "version")?.is_some()
                        && self.get(value, "digest")?.is_some()
                    {
                        return self.add(value, sources, required);
                    }
                    for (_, child) in fields {
                        self.walk(child, sources, required, depth + 1)?;
                    }
                }
                JsonValue::Array(values) => {
                    for child in values {
                        self.walk(child, sources, required, depth + 1)?;
                    }
                }
                _ => (),
            }
            Ok(())
        }
    }
    let mut access = Access {
        budget,
        deadline,
        cancelled,
    };
    let requested_use = access.text(scope, "requested_use")?;
    if !access.equal(requested_use, "registry-manual-authoring-v1")? {
        return Ok(());
    }
    let target = access.reference(current)?;
    let mut source_target = false;
    let target_id = access.text(&target, "id")?;
    for row in sources {
        let row_id = access.text(row, "id")?;
        if access.equal(row_id, target_id)? {
            let actual = access.reference(row)?;
            source_target = access.same(&actual, &target)?;
            break;
        }
    }
    if !source_target {
        return Err(SourceCommandError::Denied(
            "manual target is not the actual selected source Claim",
        ));
    }
    let mut basis = None;
    for row in sources {
        access.pay(0)?;
        let payload = access.field(row, "payload")?;
        let Some(kind) = access
            .get(payload, "record_type")?
            .and_then(JsonValue::as_str)
        else {
            continue;
        };
        if !access.equal(kind, "manual-authoring-basis")? {
            continue;
        }
        let body = access.field(payload, "body")?;
        let target_ref = access.field(body, "target")?;
        if !access.same(target_ref, &target)? {
            continue;
        }
        if basis.replace(row).is_some() {
            return Err(SourceCommandError::Conflict(
                "multiple selected manual bases target the exact Claim",
            ));
        }
    }
    let basis = basis.ok_or(SourceCommandError::Unsupported(
        "exact source manual basis is not selected",
    ))?;
    let basis_ref = access.reference(basis)?;
    access.add(&basis_ref, sources, required)?;
    let payload = access.field(basis, "payload")?;
    let body = access.field(payload, "body")?;
    let procedure = access.field(body, "procedure")?;
    access.add(procedure, sources, required)?;
    // Reserve the fixed borrowed walker stack BEFORE recursion. The closed
    // registered body grammar determines which leaves can be exact refs.
    let fixed = std::mem::size_of::<JsonValue>()
        .checked_add(8 * std::mem::size_of::<usize>())
        .and_then(|n| n.checked_mul(tos_foundation::JsonLimits::default().max_depth + 1))
        .and_then(|n| n.checked_mul(16))
        .ok_or(SourceCommandError::Unsupported(
            "manual closure fixed state overflow",
        ))?;
    access.budget.charge_bytes(fixed)?;
    let finding = access.field(body, "basis")?;
    access.walk(finding, sources, required, 0)
}

pub(crate) fn resolve_required_sources(
    references: &[JsonValue],
    all_by_id: &BTreeMap<String, JsonValue>,
) -> SourceCommandResult<Vec<JsonValue>> {
    let mut selected = BTreeMap::<String, JsonValue>::new();
    for reference in references {
        let id = cmd::text(reference, "id")?.to_owned();
        let source = all_by_id.get(&id).ok_or(SourceCommandError::Conflict(
            "assessment required current source is absent",
        ))?;
        let actual = owner_reference(source)?;
        if !cmd::same(reference, &actual)? {
            return Err(SourceCommandError::Conflict(
                "assessment required source reference is stale",
            ));
        }
        if let Some(previous) = selected.insert(id, source.clone()) {
            if !cmd::same(&previous, source)? {
                return Err(SourceCommandError::Conflict(
                    "assessment required source identity changed",
                ));
            }
        }
    }
    Ok(selected.into_values().collect())
}

pub(crate) fn required_source_refs(
    subject_id: &str,
    public_claim_dependencies: &BTreeMap<String, Vec<JsonValue>>,
    private_claim_dependencies: &BTreeMap<String, Vec<JsonValue>>,
    selected_layer: Option<&crate::source_private_assessment_layers::SelectedOwnerLayer>,
) -> SourceCommandResult<Vec<JsonValue>> {
    if let Some(layer) = selected_layer {
        return layer
            .comparison_record
            .as_ref()
            .map(|record| parse_record_input(record).and_then(|row| owner_reference(&row)))
            .transpose()
            .map(|reference| reference.into_iter().collect());
    }
    if let Some(references) = public_claim_dependencies.get(subject_id) {
        return Ok(references.clone());
    }
    if let Some(references) = private_claim_dependencies.get(subject_id) {
        return Ok(references.clone());
    }
    Ok(Vec::new())
}

pub(crate) fn scope_context(scope: &JsonValue) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("layer", cmd::field(scope, "assertion_layer")?.clone()),
        ("risk", cmd::field(scope, "risk")?.clone()),
        ("languages", cmd::field(scope, "languages")?.clone()),
        ("maker_id", cmd::field(scope, "maker_id")?.clone()),
        ("use", cmd::field(scope, "requested_use")?.clone()),
    ]))
}

fn evaluation_limits(budget: &OperationBudget) -> SourceCommandResult<AssessmentLimits> {
    budget.limits()
}

/// Borrowed dispatch lookup, paid before field and string comparisons.
fn pwe_dispatch_field<'v>(
    value: &'v JsonValue,
    key: &str,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<&'v JsonValue>> {
    active(budget.deadline, cancelled)?;
    budget.charge_work(1)?;
    let Some(fields) = value.as_object() else {
        return Ok(None);
    };
    budget.charge_work(
        fields
            .len()
            .checked_add(1)
            .ok_or(SourceCommandError::Unsupported(
                "PWE dispatch field iteration overflow",
            ))?,
    )?;
    for (name, value) in fields {
        active(budget.deadline, cancelled)?;
        if let Some(name) = name.as_str() {
            budget.charge_work(name.len().checked_add(key.len()).ok_or(
                SourceCommandError::Unsupported("PWE dispatch field comparison overflow"),
            )?)?;
            if name == key {
                return Ok(Some(value));
            }
        }
    }
    Ok(None)
}

/// Exact registered PWE account families cannot fall through to the generic
/// assessment engine while the protected native consumer is not installed.
/// This is a refusal gate, never a report/authority/completeness constructor.
fn requires_held_pwe_dispatch(
    current: &JsonValue,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    let payload = pwe_dispatch_field(current, "payload", budget, cancelled)?.unwrap_or(current);
    // These exact constants are the three declared source families in the
    // maintained pwe-bindings.v1.json, not inferred from a similar object body.
    for (field, expected) in [
        ("schema_version", "tos_pwe_name_assignment_claim_v1"),
        ("schema_version", "tos_pwe_responsibility_account_claim_v1"),
        ("schema_version", "tos_pwe_identity_correspondence_claim_v1"),
        ("predicate", "pwe_name_assignment"),
        ("predicate", "pwe_responsibility_account"),
        ("predicate", "pwe_identity_correspondence"),
    ] {
        let actual = pwe_dispatch_field(payload, field, budget, cancelled)?;
        if let Some(actual) = actual.and_then(JsonValue::as_str) {
            budget.charge_work(actual.len().checked_add(expected.len()).ok_or(
                SourceCommandError::Unsupported("PWE dispatch family comparison overflow"),
            )?)?;
            if actual == expected {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// The same boundary protects direct, form-parent and quality-dependency
/// evaluation. Only the actual selected subject is checked; an unrelated PWE
/// record in the shared source set must not infect another subject's result.
fn refuse_generic_pwe_subject(
    subject: &JsonValue,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if requires_held_pwe_dispatch(subject, budget, cancelled)? {
        return Err(SourceCommandError::Unsupported(
            "PWE source Claim requires its held exact-basis consumer dispatch",
        ));
    }
    Ok(())
}

fn refuse_generic_pwe_subject_id(
    subject_id: &str,
    sources: [&[JsonValue]; 3],
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(budget.deadline, cancelled)?;
    for records in sources {
        budget.charge_work(records.len().checked_add(1).ok_or(
            SourceCommandError::Unsupported("PWE subject dispatch iteration overflow"),
        )?)?;
        for record in records {
            if let Some(id) =
                pwe_dispatch_field(record, "id", budget, cancelled)?.and_then(JsonValue::as_str)
            {
                budget.charge_work(id.len().checked_add(subject_id.len()).ok_or(
                    SourceCommandError::Unsupported("PWE subject identity comparison overflow"),
                )?)?;
                if id == subject_id {
                    refuse_generic_pwe_subject(record, budget, cancelled)?;
                }
            }
        }
    }
    Ok(())
}

fn evaluate_selected_subject_with_executor<W, F>(
    config: &JsonValue,
    subject_id: &str,
    scope: &JsonValue,
    source_records: &[JsonValue],
    native_records: &[JsonValue],
    record_rows: &[JsonValue],
    required_refs: &[JsonValue],
    required_admission_bases: &[AssessmentRecordInput],
    trusted_history: &[AssessmentSubmissionInput],
    reviews: &[AssessmentSubmissionInput],
    source_route: AssessmentSourceRoute,
    layer_quality: Option<AssessmentLayerQualityObservation>,
    observed_now: &str,
    source_revision: Option<SourceRevision>,
    source_identity: Digest256,
    worker: &mut W,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
    evaluate: &mut F,
) -> SourceCommandResult<AssessmentMechanicsReport>
where
    F: FnMut(
        &AssessmentReadInput,
        &mut W,
        AssessmentLimits,
        &AtomicBool,
    ) -> SourceCommandResult<AssessmentMechanicsReport>,
{
    refuse_generic_pwe_subject_id(
        subject_id,
        [source_records, native_records, record_rows],
        budget,
        cancelled,
    )?;
    let input = selected_assessment_input(
        config,
        subject_id,
        scope,
        source_records,
        native_records,
        record_rows,
        required_refs,
        required_admission_bases,
        trusted_history,
        reviews,
        source_route,
        layer_quality,
        observed_now,
        source_revision,
        source_identity,
    )?;
    active(budget.deadline, cancelled)?;
    let report = evaluate(&input, worker, evaluation_limits(budget)?, cancelled)?;
    budget.charge_report(&report)?;
    Ok(report)
}

fn evaluate_selected_subject(
    config: &JsonValue,
    subject_id: &str,
    scope: &JsonValue,
    source_records: &[JsonValue],
    native_records: &[JsonValue],
    record_rows: &[JsonValue],
    required_refs: &[JsonValue],
    required_admission_bases: &[AssessmentRecordInput],
    trusted_history: &[AssessmentSubmissionInput],
    reviews: &[AssessmentSubmissionInput],
    source_route: AssessmentSourceRoute,
    layer_quality: Option<AssessmentLayerQualityObservation>,
    observed_now: &str,
    worker: &mut CutWorkerSchemaExecutor,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AssessmentMechanicsReport> {
    let source_revision = worker.source_revision();
    evaluate_selected_subject_with_executor(
        config,
        subject_id,
        scope,
        source_records,
        native_records,
        record_rows,
        required_refs,
        required_admission_bases,
        trusted_history,
        reviews,
        source_route,
        layer_quality,
        observed_now,
        Some(source_revision),
        source_revision.0,
        worker,
        budget,
        cancelled,
        &mut |input, worker, limits, cancelled| {
            evaluate_current_assessment(input, worker, limits, cancelled)
                .map_err(map_assessment_refusal)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn selected_assessment_input(
    config: &JsonValue,
    subject_id: &str,
    scope: &JsonValue,
    source_records: &[JsonValue],
    native_records: &[JsonValue],
    record_rows: &[JsonValue],
    required_refs: &[JsonValue],
    required_admission_bases: &[AssessmentRecordInput],
    trusted_history: &[AssessmentSubmissionInput],
    reviews: &[AssessmentSubmissionInput],
    source_route: AssessmentSourceRoute,
    layer_quality: Option<AssessmentLayerQualityObservation>,
    observed_now: &str,
    source_revision: Option<tos_foundation::SourceRevision>,
    source_input_identity: Digest256,
) -> SourceCommandResult<AssessmentReadInput> {
    Ok(AssessmentReadInput {
        source_revision,
        source_input_identity,
        policy: record_input(cmd::field(config, "policy")?)?,
        authorities: cmd::array(config, "authorities")?
            .iter()
            .map(record_input)
            .collect::<SourceCommandResult<Vec<_>>>()?,
        competencies: cmd::array(config, "competencies")?
            .iter()
            .map(record_input)
            .collect::<SourceCommandResult<Vec<_>>>()?,
        records: record_rows
            .iter()
            .map(record_input)
            .collect::<SourceCommandResult<Vec<_>>>()?,
        source_records: source_records
            .iter()
            .map(record_input)
            .collect::<SourceCommandResult<Vec<_>>>()?,
        native_records: native_records
            .iter()
            .map(record_input)
            .collect::<SourceCommandResult<Vec<_>>>()?,
        source_route,
        subject_id: subject_id.to_owned(),
        configured_scope: cmd::canonical(scope)?,
        required_source_refs: required_refs
            .iter()
            .map(cmd::canonical)
            .collect::<SourceCommandResult<Vec<_>>>()?,
        required_admission_bases: required_admission_bases.to_vec(),
        layer_quality,
        reviews: reviews.to_vec(),
        trusted_history: trusted_history.to_vec(),
        observed_now: observed_now.to_owned(),
    })
}

/// Prepare the asserted candidate worker for a PG-selected Assessment source
/// identity. Schema bytes are read through the caller's bounded readset, and
/// the worker carries that opaque identity without claiming a SourceRevision.
pub(crate) fn selected_assessment_candidate_worker<I: Copy + Eq>(
    invocation: &Value,
    readset: &mut impl crate::source_revisions::ReadonlyRecordFiles,
    input_identity: I,
    schema_paths: &[String],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CandidateCutWorkerSchemaExecutor<I>> {
    let selected_worker = invocation
        .get("schema_worker")
        .ok_or(SourceCommandError::Invalid("assessment PG schema worker"))?;
    exact(selected_worker, &["absolute_path", "sha256"])?;
    let budgets = invocation
        .get("budgets")
        .ok_or(SourceCommandError::Invalid("assessment PG budgets"))?;
    let mut worker_budget = ExecutorBudget::laboratory();
    worker_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    worker_budget.cpu_seconds = capped(budgets, "worker_cpu_seconds", 3)?;
    worker_budget.address_space_bytes =
        capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    let worker_identity = ExactWorkerIdentity {
        absolute_path: absolute(text(selected_worker, "absolute_path")?)?,
        sha256: digest(text(selected_worker, "sha256")?)?,
    };
    let image =
        VerifiedWorkerImageHandle::prepare(worker_identity, worker_budget, deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("assessment PG schema worker image"))?;
    let limits = CutWorkerLimits {
        max_receipts: usize::try_from(capped(budgets, "max_schema_receipts", 128)?)
            .map_err(|_| SourceCommandError::Invalid("assessment PG schema receipt budget"))?,
        max_receipt_bytes: usize::try_from(capped(budgets, "max_schema_receipt_bytes", 262_144)?)
            .map_err(|_| {
            SourceCommandError::Invalid("assessment PG schema receipt budget")
        })?,
    };
    crate::source_sign::candidate_schema_worker_from_readset(
        readset,
        input_identity,
        schema_paths,
        &image,
        worker_budget,
        limits,
        deadline,
        cancelled,
    )
}

pub(crate) fn map_assessment_refusal(error: AssessmentRefusal) -> SourceCommandError {
    let error = match error {
        AssessmentRefusal::Schema(error) => {
            AssessmentRefusal::Schema(error.compatibility_category())
        }
        other => other,
    };
    match error {
        AssessmentRefusal::InvalidInput(_) => {
            SourceCommandError::Invalid("private current assessment input")
        }
        AssessmentRefusal::Budget => SourceCommandError::Invalid("private assessment budget"),
        AssessmentRefusal::Cancelled | AssessmentRefusal::Deadline => {
            SourceCommandError::Denied("private assessment cancelled or expired")
        }
        AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Source(_)) => {
            SourceCommandError::Invalid("private assessment schema execution")
        }
        AssessmentRefusal::Schema(
            tos_validation::item_rules::ItemRefusal::Budget
            | tos_validation::item_rules::ItemRefusal::BudgetCheck { .. },
        ) => SourceCommandError::Invalid("private assessment schema budget"),
        AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Deadline) => {
            SourceCommandError::Denied("private assessment schema deadline")
        }
        AssessmentRefusal::Schema(
            tos_validation::item_rules::ItemRefusal::Unsupported(_)
            | tos_validation::item_rules::ItemRefusal::Executor(_),
        )
        | AssessmentRefusal::Unsupported(_) => {
            SourceCommandError::Unsupported("private assessment evaluation")
        }
    }
}

pub(crate) fn owner_snapshot(
    config: &JsonValue,
    configuration_raw: &[u8],
    public_fixity: &[JsonValue],
    resolved_rows: &[JsonValue],
    public_identity_snapshots: &BTreeMap<String, JsonValue>,
    public_claim_dependencies: &BTreeMap<String, Vec<JsonValue>>,
    private_source_snapshot: &str,
    private_native_snapshots: &[String],
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
) -> SourceCommandResult<String> {
    let version = OwnerVersion::from_config(config)?;
    if version == OwnerVersion::V1 {
        return Ok(format!("sha256:{}", cmd::record_digest(config)?.to_hex()));
    }
    let mut identity_rows = public_identity_snapshots.clone();
    if version.is_private() {
        identity_rows.insert(
            "native_text_snapshots".to_owned(),
            JsonValue::Array(
                private_native_snapshots
                    .iter()
                    .map(|value| cmd::string(value))
                    .collect(),
            ),
        );
    }
    if let Some(layers) = layers {
        identity_rows.insert(
            "native_layer_assessment_snapshot".to_owned(),
            cmd::string(&layers.source_snapshot),
        );
    }
    let mut entries = BTreeMap::<String, JsonValue>::new();
    entries.insert("configuration".to_owned(), config.clone());
    entries.insert(
        "source_files".to_owned(),
        JsonValue::Array(public_fixity.to_vec()),
    );
    entries.insert(
        "resolved_records".to_owned(),
        JsonValue::Array(resolved_rows.to_vec()),
    );
    entries.extend(identity_rows);
    if !public_claim_dependencies.is_empty() {
        let dependencies = JsonValue::Object(
            public_claim_dependencies
                .iter()
                .map(|(identity, rows)| {
                    (
                        JsonString::from_utf8(identity),
                        JsonValue::Array(rows.clone()),
                    )
                })
                .collect(),
        );
        entries.insert("public_claim_dependencies".to_owned(), dependencies);
    }
    if version.is_private() {
        entries.insert(
            "owner_local_sources".to_owned(),
            cmd::string(private_source_snapshot),
        );
        entries.insert(
            "configuration_bytes".to_owned(),
            cmd::string(&Digest256::of_bytes(configuration_raw).to_hex()),
        );
    }
    let snapshot = JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(&key), value))
            .collect(),
    );
    Ok(format!(
        "sha256:{}",
        cmd::record_digest(&snapshot)?.to_hex()
    ))
}

pub(crate) fn quality_snapshot(
    source_snapshot: &str,
    bases: &[(QualityRequirement, JsonValue)],
) -> SourceCommandResult<String> {
    let references = bases
        .iter()
        .map(|(_, basis)| owner_reference(basis))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let value = cmd::object(vec![
        ("source_snapshot", cmd::string(source_snapshot)),
        ("quality_bases", JsonValue::Array(references)),
    ]);
    Ok(format!("sha256:{}", cmd::record_digest(&value)?.to_hex()))
}

pub(crate) fn extend_subject_assessment_snapshot(
    source_snapshot: &str,
    subject_assessment: &JsonValue,
) -> SourceCommandResult<String> {
    let value = cmd::object(vec![
        ("source_snapshot", cmd::string(source_snapshot)),
        ("subject_assessment", subject_assessment.clone()),
    ]);
    Ok(format!("sha256:{}", cmd::record_digest(&value)?.to_hex()))
}

pub(crate) fn record_rows(
    config: &JsonValue,
    additions: &[JsonValue],
) -> SourceCommandResult<Vec<JsonValue>> {
    let base = cmd::array(config, "records")?;
    if base
        .len()
        .checked_add(additions.len())
        .is_none_or(|n| n > MAX_ASSESSMENTS)
    {
        return Err(SourceCommandError::Invalid(
            "assessment owner record budget",
        ));
    }
    let mut rows = base.to_vec();
    let mut seen = base
        .iter()
        .map(|row| cmd::text(row, "id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    for row in additions {
        let id = cmd::text(row, "id")?.to_owned();
        if !seen.insert(id) {
            return Err(SourceCommandError::Denied(
                "derived assessment basis shadows an owner record",
            ));
        }
        rows.push(row.clone());
    }
    Ok(rows)
}

pub(crate) fn quality_requirements(
    current: &JsonValue,
    required_sources: &[JsonValue],
    records: &BTreeMap<String, JsonValue>,
    config: &JsonValue,
    layers: &crate::source_private_assessment_layers::PrivateAssessmentLayers,
    assertion_layer: &str,
    budget: &mut OperationBudget,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<QualityRequirement>> {
    let current_id = cmd::text(current, "id")?;
    let mut pending = vec![current.clone()];
    pending.extend(required_sources.iter().cloned());
    let mut seen_records = BTreeSet::new();
    let mut needed = BTreeSet::new();
    while let Some(record) = pending.pop() {
        active(budget.deadline, cancelled)?;
        budget.charge_work(1)?;
        let identity = cmd::text(&record, "id")?.to_owned();
        if !seen_records.insert(identity.clone()) {
            continue;
        }
        if seen_records.len() > MAX_ASSESSMENTS {
            return Err(SourceCommandError::Invalid(
                "assessment quality source closure budget",
            ));
        }
        let body = cmd::field(&record, "payload")?;
        let binding = body
            .object_get("native_text_binding")
            .or_else(|| body.object_get("native_binding"));
        if let Some(binding) = binding.filter(|value| !value.is_null()) {
            let layer_binding = cmd::field(binding, "text_layer")?;
            let layer_id = cmd::text(layer_binding, "layer_id")?;
            let selected = layers
                .layers
                .get(layer_id)
                .ok_or(SourceCommandError::Denied(
                    "quality grounding lacks its exact selected native layer",
                ))?;
            if !cmd::same(layer_binding, cmd::field(&selected.binding, "text_layer")?)?
                || !cmd::same(
                    cmd::field(binding, "source_record_refs")?,
                    cmd::field(&selected.binding, "source_record_refs")?,
                )?
            {
                return Err(SourceCommandError::Denied(
                    "quality grounding differs from its exact selected layer binding",
                ));
            }
            needed.insert(layer_id.to_owned());
        }
        if body
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            == Some("tos_source_text_layer_v1")
            && identity != current_id
        {
            let selected = layers
                .layers
                .get(&identity)
                .ok_or(SourceCommandError::Denied(
                    "quality grounding lacks its exact selected current layer",
                ))?;
            let selected_row = parse_record_input(&selected.layer_record)?;
            if !cmd::same(&owner_reference(&record)?, &owner_reference(&selected_row)?)? {
                return Err(SourceCommandError::Conflict(
                    "quality grounding text layer is not the selected current record",
                ));
            }
            needed.insert(identity.clone());
        }
        if body
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            == Some("tos_human_form_v1")
        {
            let mut references = Vec::new();
            references.push(cmd::field(body, "subject")?.clone());
            let bindings = cmd::field(body, "bindings")?
                .as_object()
                .ok_or(SourceCommandError::Invalid("assessment form bindings"))?;
            for (_, value) in bindings {
                references.push(cmd::field(value, "record")?.clone());
            }
            for reference in references {
                let source_id = cmd::text(&reference, "id")?;
                if source_id.starts_with("tos.quality-basis.sha256.") {
                    continue;
                }
                let source = records.get(source_id).ok_or(SourceCommandError::Conflict(
                    "quality-bound form source is absent from the selected snapshot",
                ))?;
                if !cmd::same(&reference, &owner_reference(source)?)? {
                    return Err(SourceCommandError::Conflict(
                        "quality-bound form source snapshot is stale",
                    ));
                }
                pending.push(source.clone());
            }
        }
    }

    // The layer adapter authenticates the exact v5/v6 dependency map. Its
    // preflight is retained on the selected operation and is read separately
    // by the lock-closure walk; direct requirements are resolved below from
    // the independently pinned owner configuration.
    let entries = cmd::field(config, "quality_dependencies")?
        .object_get(current_id)
        .and_then(JsonValue::as_array)
        .unwrap_or(&[]);
    let configured_ids = entries
        .iter()
        .map(|entry| cmd::text(entry, "layer_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if configured_ids != needed || needed.contains(current_id) {
        return Err(SourceCommandError::Denied(
            "native quality dependencies omit, add or self-ground a direct source",
        ));
    }

    let effective_layer = if assertion_layer == "human_projection" {
        let subject_id = cmd::field(current, "payload")?
            .object_get("subject")
            .and_then(|value| value.object_get("id"))
            .and_then(JsonValue::as_str);
        if let Some(parent) = subject_id.and_then(|identity| records.get(identity)) {
            if let Some(layer) = cmd::field(parent, "payload")?
                .object_get("assertion_layer")
                .and_then(JsonValue::as_str)
            {
                layer.to_owned()
            } else if cmd::field(parent, "payload")?
                .object_get("schema_version")
                .and_then(JsonValue::as_str)
                == Some("tos_native_text_unit_assessment_subject_v1")
            {
                "textual_observation".to_owned()
            } else {
                "semantic_interpretation".to_owned()
            }
        } else {
            "semantic_interpretation".to_owned()
        }
    } else {
        assertion_layer.to_owned()
    };
    let required_use = if matches!(
        effective_layer.as_str(),
        "linguistic_analysis" | "translation_alignment" | "translation_judgment"
    ) {
        "text-layer:linguistic-analysis"
    } else if matches!(
        effective_layer.as_str(),
        "textual_observation"
            | "forensic_observation"
            | "bibliographic_assertion"
            | "scholarly_report"
    ) {
        "text-layer:citation"
    } else {
        "text-layer:semantic-analysis"
    };
    let mut requirements = Vec::new();
    for entry in entries {
        let layer_id = cmd::text(entry, "layer_id")?.to_owned();
        let use_name = cmd::text(entry, "use")?.to_owned();
        if use_name != required_use {
            return Err(SourceCommandError::Denied(
                "native quality purpose does not cover this assertion layer",
            ));
        }
        requirements.push(QualityRequirement { layer_id, use_name });
    }
    requirements.sort_by(|left, right| left.layer_id.cmp(&right.layer_id));
    budget.charge_work(requirements.len())?;
    Ok(requirements)
}

pub(crate) fn collect_quality_lock_closure(
    roots: &[String],
    preflight: Option<&crate::source_private_assessment_layers::OwnerLocalLayerPreflight>,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<String>> {
    let Some(preflight) = preflight else {
        if roots.is_empty() {
            return Ok(Vec::new());
        }
        return Err(SourceCommandError::Invalid(
            "assessment quality lock closure lacks owner preflight",
        ));
    };
    let mut queue = std::collections::VecDeque::<String>::new();
    let mut scheduled = BTreeSet::new();
    for root in roots {
        if scheduled.insert(root.clone()) {
            queue.push_back(root.clone());
        }
    }
    let mut visited = BTreeSet::new();
    while let Some(subject_id) = queue.pop_front() {
        active(deadline, cancelled)?;
        if !visited.insert(subject_id.clone()) {
            continue;
        }
        if visited.len() > MAX_QUALITY_SUBJECTS {
            return Err(SourceCommandError::Invalid(
                "assessment quality history closure budget",
            ));
        }
        budget.charge_work(1)?;
        if let Some(dependencies) = preflight.quality_dependencies.get(&subject_id) {
            for dependency in dependencies {
                budget.charge_work(1)?;
                budget.charge_bytes(
                    dependency
                        .layer_id
                        .len()
                        .saturating_add(dependency.use_name.len()),
                )?;
                if !visited.contains(&dependency.layer_id)
                    && scheduled.insert(dependency.layer_id.clone())
                {
                    queue.push_back(dependency.layer_id.clone());
                }
            }
        }
        if scheduled.len() > MAX_QUALITY_SUBJECTS {
            return Err(SourceCommandError::Invalid(
                "assessment quality history closure budget",
            ));
        }
    }
    Ok(visited.into_iter().collect())
}

pub(crate) fn quality_basis<W: CutSchemaExecutor>(
    layer: &crate::source_private_assessment_layers::SelectedOwnerLayer,
    report: &AssessmentMechanicsReport,
    use_name: &str,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let layer_row = parse_record_input(&layer.layer_record)?;
    let layer_ref = owner_reference(&layer_row)?;
    let comparison_ref = layer
        .comparison_record
        .as_ref()
        .map(parse_record_input)
        .transpose()?
        .as_ref()
        .map(owner_reference)
        .transpose()?
        .unwrap_or(JsonValue::Null);
    let representation = cmd::field(cmd::field(&layer_row, "payload")?, "representation")?;
    let scope = cmd::object(
        ["content_file_id", "content_sha256", "text_scope"]
            .iter()
            .map(|key| Ok((*key, cmd::field(representation, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    let basis_identity = cmd::object(vec![
        ("layer_id", cmd::field(&layer_ref, "id")?.clone()),
        ("use", cmd::string(use_name)),
        ("scope", scope.clone()),
    ]);
    let identity = format!(
        "tos.quality-basis.sha256.{}",
        cmd::record_digest(&basis_identity)?.to_hex()
    );
    let admission = report_admission(report)?;
    let mut limits = Vec::<JsonValue>::new();
    for value in cmd::array(&admission, "limits")? {
        if !limits.iter().any(|existing| existing == value) {
            limits.push(value.clone());
        }
    }
    for value in &layer.comparison_limits {
        let value = cmd::string(value);
        if !limits.iter().any(|existing| existing == &value) {
            limits.push(value);
        }
    }
    let basis_body = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_native_text_layer_quality_basis_v1"),
        ),
        ("basis_id", cmd::string(&identity)),
        ("basis_version", cmd::number(1)),
        ("layer", layer_ref),
        ("comparison", comparison_ref),
        ("use", cmd::string(use_name)),
        ("scope", scope),
        ("policy", cmd::field(&admission, "policy")?.clone()),
        (
            "assessment_refs",
            cmd::field(&admission, "assessment_refs")?.clone(),
        ),
        ("status", cmd::field(&admission, "status")?.clone()),
        (
            "can_use",
            JsonValue::Bool(
                cmd::field(&admission, "can_use")? == &JsonValue::Bool(true) && layer.read_ready,
            ),
        ),
        ("limits", JsonValue::Array(limits)),
        ("visibility", cmd::string("local_only")),
        ("publication_authorized", JsonValue::Bool(false)),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
    ]);
    if !worker
        .check(
            "assessment/native-text-layer-quality-basis",
            &cmd::canonical(&basis_body)?,
            QUALITY_BASIS_SCHEMA,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Invalid("assessment quality basis schema execution"))?
    {
        return Err(SourceCommandError::Invalid(
            "assessment quality basis schema",
        ));
    }
    let origin = layer_row
        .object_get("origin_id")
        .cloned()
        .unwrap_or(JsonValue::Null);
    Ok(cmd::object(vec![
        ("id", cmd::string(&identity)),
        ("version", cmd::number(1)),
        ("payload", basis_body),
        ("origin_id", origin),
    ]))
}

pub(crate) fn report_admission(
    report: &AssessmentMechanicsReport,
) -> SourceCommandResult<JsonValue> {
    let raw = serde_json::to_vec(report.current_admission())
        .map_err(|_| SourceCommandError::Invalid("assessment admission serialization"))?;
    cmd::parse(&raw)
}

pub(crate) fn basis_observation(basis: &JsonValue) -> SourceCommandResult<(bool, Vec<JsonValue>)> {
    Ok((
        cmd::field(cmd::field(basis, "payload")?, "can_use")? == &JsonValue::Bool(true),
        cmd::array(cmd::field(basis, "payload")?, "limits")?.to_vec(),
    ))
}

fn verify_selected_sources(
    selected: &SelectedOperationView<'_>,
    public_sources: &mut crate::source_sign::OwnerAssessmentPublicSources<'_>,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    selected.context.check_from_selected_captures(
        selected.cut,
        software,
        components,
        deadline,
        cancelled,
    )?;
    selected.owner.verify_current(deadline, cancelled)?;
    public_sources.verify_current(deadline, cancelled)?;
    selected.private_sources.verify_current(
        &selected.owner_context,
        &selected.context,
        selected.cut,
        deadline,
        cancelled,
    )?;
    if let Some(layers) = layers {
        layers.verify_current(&selected.owner_context, deadline, cancelled)?;
    }
    selected.owner.verify_current(deadline, cancelled)?;
    Ok(())
}

pub(crate) fn native_append_ready(
    selected_layer: bool,
    claim_source_bound: bool,
    required_sources: &[JsonValue],
    native_summaries: &[JsonValue],
) -> SourceCommandResult<bool> {
    if selected_layer {
        return Ok(true);
    }
    let required_ids = required_sources
        .iter()
        .map(|reference| cmd::text(reference, "id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    for summary in native_summaries {
        let relevant = if claim_source_bound {
            let identity = summary
                .object_get("unit_id")
                .and_then(JsonValue::as_str)
                .ok_or(SourceCommandError::Invalid(
                    "private native summary unit identity",
                ))?;
            required_ids.contains(identity)
        } else {
            true
        };
        if relevant && summary.object_get("content_verified") != Some(&JsonValue::Bool(true)) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn parent_claim_context(
    current_form: &JsonValue,
    form_scope: &JsonValue,
    config: &JsonValue,
    records: &BTreeMap<String, JsonValue>,
    form_required_sources: &[JsonValue],
    public_claim_dependencies: &BTreeMap<String, Vec<JsonValue>>,
    private_claim_dependencies: &BTreeMap<String, Vec<JsonValue>>,
    private_required_languages: &BTreeMap<String, Vec<String>>,
    native_summaries: &[JsonValue],
) -> SourceCommandResult<Option<ParentClaimContext>> {
    let form_body = cmd::field(current_form, "payload")?;
    let parent_ref = cmd::field(form_body, "subject")?;
    let parent_id = cmd::text(parent_ref, "id")?;
    let parent = records.get(parent_id).ok_or(SourceCommandError::Conflict(
        "assessed form parent is absent from its exact selected source snapshot",
    ))?;
    if !cmd::same(parent_ref, &owner_reference(parent)?)? {
        return Err(SourceCommandError::Conflict(
            "assessed form binds another current parent source snapshot",
        ));
    }
    let parent_body = cmd::field(parent, "payload")?;
    if parent_body.object_get("claim_id").is_none()
        || parent_body.object_get("claim_version").is_none()
    {
        return Ok(None);
    }
    let scope = configured_scope(config, parent_id)?;
    if !cmd::same(cmd::field(&scope, "record")?, parent_ref)?
        || cmd::field(&scope, "access_allowed")? != &JsonValue::Bool(true)
        || cmd::field(&scope, "requested_use")? != cmd::field(form_scope, "requested_use")?
        || cmd::field(&scope, "assertion_layer")?
            != parent_body
                .object_get("assertion_layer")
                .unwrap_or(&JsonValue::Null)
        || cmd::field(&scope, "maker_id")?
            != cmd::field(cmd::field(parent_body, "maker")?, "agent_ref")?
    {
        return Err(SourceCommandError::Denied(
            "parent Claim scope disagrees with its source, use or access",
        ));
    }
    if matches!(
        parent_body
            .object_get("predicate")
            .and_then(JsonValue::as_str),
        Some("identity_transition_proposal" | "subject_identity_transition_proposal")
    ) && (cmd::text(&scope, "risk")? != "high"
        || cmd::text(&scope, "requested_use")? != "research")
    {
        return Err(SourceCommandError::Denied(
            "identity proposal assessment requires high-risk research scope",
        ));
    }
    let claim_references = if let Some(references) = public_claim_dependencies.get(parent_id) {
        references
    } else if let Some(references) = private_claim_dependencies.get(parent_id) {
        references
    } else {
        return Err(SourceCommandError::Denied(
            "form parent Claim is not an explicitly source-selected Claim",
        ));
    };
    let required_sources = resolve_required_sources(claim_references, records)?;
    let mut selected_refs = BTreeMap::<String, JsonValue>::new();
    selected_refs.insert(parent_id.to_owned(), parent_ref.clone());
    for source in &required_sources {
        let reference = owner_reference(source)?;
        selected_refs.insert(cmd::text(source, "id")?.to_owned(), reference);
    }
    let form_refs = form_required_sources
        .iter()
        .map(|reference| Ok((cmd::text(reference, "id")?.to_owned(), reference.clone())))
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    for (identity, reference) in &selected_refs {
        if form_refs
            .get(identity)
            .is_none_or(|current| current != reference)
        {
            return Err(SourceCommandError::Denied(
                "parent Claim grounding is outside the selected form closure",
            ));
        }
    }
    let scope_languages = cmd::array(&scope, "languages")?;
    if scope_languages.is_empty()
        || scope_languages
            .iter()
            .any(|language| language.as_str().is_none_or(|language| language.is_empty()))
    {
        return Err(SourceCommandError::Invalid(
            "parent Claim assessment language scope is incomplete",
        ));
    }
    let mut required_languages = BTreeSet::new();
    if let Some(languages) = private_required_languages.get(parent_id) {
        for language in languages {
            required_languages.insert(
                tos_foundation::python_casefold_unicode16_v1(language, language.len(), 512, 2048)
                    .map_err(|_| SourceCommandError::Invalid("parent Claim language budget"))?,
            );
        }
    }
    let dependency_ids = required_sources
        .iter()
        .map(|source| cmd::text(source, "id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    for summary in native_summaries {
        if summary
            .object_get("unit_id")
            .and_then(JsonValue::as_str)
            .is_some_and(|identity| dependency_ids.contains(identity))
        {
            if let Some(language) = summary.object_get("language").and_then(JsonValue::as_str) {
                required_languages.insert(
                    tos_foundation::python_casefold_unicode16_v1(
                        language,
                        language.len(),
                        512,
                        2048,
                    )
                    .map_err(|_| SourceCommandError::Invalid("parent Claim language budget"))?,
                );
            }
        }
    }
    let scope_language_set = scope_languages
        .iter()
        .map(|language| {
            let language = language
                .as_str()
                .ok_or(SourceCommandError::Invalid("parent Claim language"))?;
            tos_foundation::python_casefold_unicode16_v1(language, language.len(), 512, 2048)
                .map_err(|_| SourceCommandError::Invalid("parent Claim language budget"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if !required_languages.is_subset(&scope_language_set) {
        return Err(SourceCommandError::Denied(
            "parent Claim assessment scope omits selected source languages",
        ));
    }
    Ok(Some(ParentClaimContext {
        record: parent.clone(),
        scope,
        required_sources,
    }))
}

pub(crate) fn parent_assessment_packet(
    record: &JsonValue,
    report: &AssessmentMechanicsReport,
    history: &AssessmentHistory,
    scope: &JsonValue,
) -> SourceCommandResult<JsonValue> {
    let subject_ref = owner_reference(record)?;
    let committed_scope = scope_context(scope)?;
    let mut withdrawals = Vec::new();
    for submission in &history.submissions {
        let assessment = cmd::parse(&submission.assessment)?;
        if cmd::text(&assessment, "decision")? != "withdraw"
            || !cmd::same(cmd::field(&assessment, "subject")?, &subject_ref)?
        {
            continue;
        }
        let Some(raw_scope) = &submission.committed_scope else {
            continue;
        };
        let old_scope = cmd::parse(raw_scope)?;
        if !committed_scopes_match(&old_scope, &committed_scope)? {
            continue;
        }
        withdrawals.push(cmd::object(vec![
            ("id", cmd::field(&assessment, "assessment_id")?.clone()),
            ("version", cmd::number(1)),
            (
                "digest",
                cmd::string(&cmd::record_digest(&assessment)?.to_prefixed()),
            ),
        ]));
    }
    let revision = history
        .head
        .as_deref()
        .map(cmd::string)
        .unwrap_or(JsonValue::Null);
    Ok(cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_human_form_subject_assessment_v1"),
        ),
        ("subject", subject_ref),
        ("admission", report_admission(report)?),
        ("journal_revision", revision),
        ("journal_batches", cmd::number(history.batches.len() as u64)),
        ("historical_withdrawals", JsonValue::Array(withdrawals)),
        (
            "form_admission_is_parent_endorsement",
            JsonValue::Bool(false),
        ),
    ]))
}

fn committed_scopes_match(a: &JsonValue, b: &JsonValue) -> SourceCommandResult<bool> {
    for key in ["assertion_layer", "risk", "maker_id", "requested_use"] {
        if cmd::field(a, key)? != cmd::field(b, key)? {
            return Ok(false);
        }
    }
    let fold = |value: &JsonValue| -> SourceCommandResult<BTreeSet<String>> {
        cmd::array(value, "languages")?
            .iter()
            .map(|language| {
                let text = language
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("assessment scope language"))?;
                tos_foundation::python_casefold_unicode16_v1(text, text.len(), 512, 2048)
                    .map_err(|_| SourceCommandError::Invalid("assessment scope language budget"))
            })
            .collect()
    };
    Ok(fold(a)? == fold(b)?)
}

#[allow(clippy::too_many_arguments)]
fn execute_journal_operation(
    selected: &SelectedOperationView<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    config: &JsonValue,
    scope: &JsonValue,
    current: &JsonValue,
    history: &AssessmentHistory,
    fence: &AssessmentJournalFence<'_>,
    current_report: &AssessmentMechanicsReport,
    append_report: Option<&AssessmentMechanicsReport>,
    admission_bases: &[(
        QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )],
    _quality_bases: &[(QualityRequirement, JsonValue)],
    _quality_requirements: &[QualityRequirement],
    parent_assessment: &Option<JsonValue>,
    all_by_id: &BTreeMap<String, JsonValue>,
    required_sources: &[JsonValue],
    snapshot: &str,
    source_read_ready: bool,
    public_sources: &mut crate::source_sign::OwnerAssessmentPublicSources<'_>,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let subject_id = selected.request.subject_id.as_str();
    let history_revision = history
        .head
        .as_deref()
        .map(cmd::string)
        .unwrap_or(JsonValue::Null);
    let current_admission = || current_admission_with_limits(current_report, layers, subject_id);
    let result =
        match selected.request.operation.as_str() {
            "describe" | "inspect" | "read-layer-comparison" => {
                let mut result = cmd::object(vec![
                    ("revision", history_revision),
                    ("batch_count", cmd::number(history.batches.len() as u64)),
                    ("current_admission", current_admission()?),
                ]);
                if selected.request.operation == "read-layer-comparison" {
                    let layer = layers
                        .and_then(|layers| layers.layers.get(subject_id))
                        .ok_or(SourceCommandError::Denied(
                            "assessment comparison reading requires an exact selected layer",
                        ))?;
                    let comparison = layer.comparison_record.as_ref().ok_or(
                        SourceCommandError::Denied(
                            "assessment comparison reading requires an exact current comparison",
                        ),
                    )?;
                    let comparison = parse_record_input(comparison)?;
                    cmd::set(
                        &mut result,
                        "source_comparison",
                        cmd::object(vec![
                            ("record", owner_reference(&comparison)?),
                            ("payload", cmd::field(&comparison, "payload")?.clone()),
                            (
                                "origin_id",
                                comparison
                                    .object_get("origin_id")
                                    .cloned()
                                    .unwrap_or(JsonValue::Null),
                            ),
                        ]),
                    )?;
                }
                if selected.request.operation == "describe" {
                    cmd::set(
                        &mut result,
                        "command_context",
                        describe_context(
                            selected,
                            config,
                            scope,
                            current,
                            current_report,
                            admission_bases,
                            parent_assessment,
                            required_sources,
                            source_read_ready,
                            public_sources,
                            layers,
                        )?,
                    )?;
                }
                crate::source_sign::finish_worker(worker, deadline, cancelled)?;
                result
            }
            "materialize-form" => {
                let materialization = materialize_selected_form(
                    selected,
                    worker,
                    current,
                    scope,
                    history,
                    current_report,
                    admission_bases,
                    parent_assessment.as_ref(),
                    all_by_id,
                    required_sources,
                    public_sources,
                    layers,
                    deadline,
                    cancelled,
                )?;
                let result = cmd::object(vec![
                    ("revision", history_revision),
                    ("batch_count", cmd::number(history.batches.len() as u64)),
                    (
                        "current_admission",
                        current_admission_with_limits(current_report, layers, subject_id)?,
                    ),
                    ("materialization", materialization),
                ]);
                crate::source_sign::finish_worker(worker, deadline, cancelled)?;
                result
            }
            "append" => {
                let append_report = append_report.ok_or(SourceCommandError::Invalid(
                    "assessment append evaluation absent",
                ))?;
                let (result, committed) = append_assessment_batch(
                    selected,
                    worker,
                    config,
                    scope,
                    current,
                    history,
                    fence,
                    current_report,
                    append_report,
                    admission_bases,
                    all_by_id,
                    layers,
                    public_sources,
                    software,
                    components,
                    deadline,
                    cancelled,
                )?;
                let _ = committed;
                result
            }
            _ => {
                return Err(SourceCommandError::Unsupported(
                    "private assessment operation",
                ));
            }
        };
    let mut envelope = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_assessment_result_v1"),
        ),
        ("owner_snapshot", cmd::string(snapshot)),
        ("authentication", cmd::string("local-unix-account")),
        ("result", result),
    ]);
    if selected.version.is_private() {
        cmd::set(&mut envelope, "visibility", cmd::string("local_only"))?;
        cmd::set(
            &mut envelope,
            "publication_authorized",
            JsonValue::Bool(false),
        )?;
    }
    Ok(envelope)
}

pub(crate) fn current_admission_with_limits(
    report: &AssessmentMechanicsReport,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    subject_id: &str,
) -> SourceCommandResult<JsonValue> {
    let mut admission = report_admission(report)?;
    if let Some(layer) = layers.and_then(|layers| layers.layers.get(subject_id)) {
        let mut limits = cmd::array(&admission, "limits")?.to_vec();
        for value in &layer.comparison_limits {
            let value = cmd::string(value);
            if !limits.iter().any(|item| item == &value) {
                limits.push(value);
            }
        }
        cmd::set(&mut admission, "limits", JsonValue::Array(limits))?;
    }
    Ok(admission)
}

fn describe_context(
    selected: &SelectedOperationView<'_>,
    config: &JsonValue,
    scope: &JsonValue,
    current: &JsonValue,
    current_report: &AssessmentMechanicsReport,
    admission_bases: &[(
        QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )],
    parent_assessment: &Option<JsonValue>,
    required_sources: &[JsonValue],
    source_read_ready: bool,
    public_sources: &crate::source_sign::OwnerAssessmentPublicSources<'_>,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
) -> SourceCommandResult<JsonValue> {
    let current_body = cmd::field(current, "payload")?;
    let current_id = cmd::text(current, "id")?;
    let layer = layers.and_then(|layers| layers.layers.get(current_id));
    let is_source_form = selected
        .private_sources
        .form_paths()
        .contains_key(current_id)
        || public_sources.form_paths.contains_key(current_id);
    let is_source_form = is_source_form
        && current_body
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            == Some("tos_human_form_v1");
    let materializable_form = is_source_form
        && current_body
            .object_get("content")
            .and_then(|content| content.object_get("kind"))
            .and_then(JsonValue::as_str)
            .is_some_and(|kind| matches!(kind, "source-copy" | "freeform"));
    let mut supported = vec!["describe", "inspect"];
    let claim_source_bound = selected
        .private_sources
        .required_source_refs()
        .contains_key(current_id)
        || public_sources.claim_dependencies.contains_key(current_id);
    let required_ids = required_sources
        .iter()
        .map(|row| cmd::text(row, "id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let summaries_ready = selected
        .private_sources
        .native_summaries
        .iter()
        .all(|summary| {
            if claim_source_bound {
                summary
                    .object_get("unit_id")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|unit| !required_ids.contains(unit))
                    || summary.object_get("content_verified") == Some(&JsonValue::Bool(true))
            } else {
                summary.object_get("content_verified") == Some(&JsonValue::Bool(true))
            }
        });
    let can_append = current_report.source_read_ready()
        && summaries_ready
        && layer.is_none_or(|layer| layer.comparison_record.is_some());
    if can_append {
        supported.push("append");
    }
    if materializable_form && source_read_ready {
        supported.push("materialize-form");
    }
    if layer.is_some_and(|layer| layer.comparison_record.is_some()) {
        supported.push("read-layer-comparison");
    }
    let mut context = cmd::object(vec![
        ("subject", owner_reference(current)?),
        (
            "policy",
            owner_reference(&cmd::field(config, "policy")?.clone())?,
        ),
        (
            "scope",
            if selected.version.is_private() {
                scope_context(scope)?
            } else {
                scope.clone()
            },
        ),
        (
            "supported_operations",
            JsonValue::Array(supported.iter().map(|value| cmd::string(value)).collect()),
        ),
        ("grants_authority", JsonValue::Bool(false)),
    ]);
    if !required_sources.is_empty() {
        cmd::set(
            &mut context,
            "required_sources",
            JsonValue::Array(
                required_sources
                    .iter()
                    .map(owner_reference)
                    .collect::<SourceCommandResult<Vec<_>>>()?,
            ),
        )?;
    }
    if selected.version.has_layer_quality() {
        let rows = admission_bases
            .iter()
            .map(|(_, basis, can_use, limits)| {
                let row = parse_record_input(basis)?;
                let body = cmd::field(&row, "payload")?;
                Ok(cmd::object(vec![
                    ("basis", owner_reference(&row)?),
                    ("can_use", JsonValue::Bool(*can_use)),
                    ("limits", JsonValue::Array(limits.clone())),
                    ("layer", cmd::field(body, "layer")?.clone()),
                    ("use", cmd::field(body, "use")?.clone()),
                ]))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        cmd::set(&mut context, "required_admissions", JsonValue::Array(rows))?;
        if let Some(layers) = layers {
            let contracts = layers
                .profile_contract_digests
                .iter()
                .map(|(path, digest)| {
                    cmd::object(vec![
                        ("path", cmd::string(path)),
                        ("digest", cmd::string(digest)),
                    ])
                })
                .collect();
            cmd::set(
                &mut context,
                "layer_comparison_contracts",
                JsonValue::Array(contracts),
            )?;
        }
    }
    if let Some(layer) = layer {
        cmd::set(
            &mut context,
            "source_comparison",
            cmd::object(vec![
                ("required", JsonValue::Bool(true)),
                ("ready", JsonValue::Bool(layer.comparison_record.is_some())),
                (
                    "positive_use_allowed",
                    JsonValue::Bool(layer.positive_use_allowed),
                ),
                (
                    "record",
                    layer
                        .comparison_record
                        .as_ref()
                        .map(parse_record_input)
                        .transpose()?
                        .map(|row| owner_reference(&row))
                        .transpose()?
                        .unwrap_or(JsonValue::Null),
                ),
            ]),
        )?;
    }
    if current_report.source_read_required() || selected.version.has_layer_quality() {
        cmd::set(
            &mut context,
            "source_read",
            cmd::object(vec![
                ("required", JsonValue::Bool(true)),
                ("ready", JsonValue::Bool(source_read_ready)),
            ]),
        )?;
    }
    if let Some(packet) = parent_assessment {
        cmd::set(&mut context, "subject_assessment", packet.clone())?;
    }
    let private_records = selected
        .private_sources
        .records
        .iter()
        .map(|input| {
            let row = parse_record_input(input)?;
            Ok(cmd::object(vec![
                ("record", owner_reference(&row)?),
                (
                    "origin_id",
                    row.object_get("origin_id")
                        .cloned()
                        .unwrap_or(JsonValue::Null),
                ),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if !private_records.is_empty() {
        cmd::set(
            &mut context,
            "owner_local_source_records",
            JsonValue::Array(private_records),
        )?;
    }
    if !selected.private_sources.native_summaries.is_empty() {
        cmd::set(
            &mut context,
            "native_text_units",
            JsonValue::Array(selected.private_sources.native_summaries.clone()),
        )?;
    }
    if !selected.version.is_private() && selected.version.has_source() {
        let fixity = public_sources
            .fixity
            .iter()
            .map(|row| Ok((cmd::text(row, "path")?, cmd::field(row, "digest")?.clone())))
            .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
        let bindings = cmd::array(config, "source_records")?;
        let paths = bindings
            .iter()
            .map(|row| cmd::text(row, "path"))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        let records = bindings
            .iter()
            .map(|binding| {
                let path = cmd::text(binding, "path")?;
                let id = cmd::text(binding, "record_id")?;
                let row = public_sources
                    .rows
                    .iter()
                    .find(|row| row.object_get("id").and_then(JsonValue::as_str) == Some(id))
                    .ok_or(SourceCommandError::Conflict(
                        "public assessment source record absent",
                    ))?;
                Ok(cmd::object(vec![
                    ("record", owner_reference(row)?),
                    ("path", cmd::string(path)),
                    (
                        "file_digest",
                        fixity
                            .get(path)
                            .cloned()
                            .ok_or(SourceCommandError::Conflict(
                                "public assessment source fixity absent",
                            ))?,
                    ),
                    ("origin_id", cmd::field(row, "origin_id")?.clone()),
                ]))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        cmd::set(&mut context, "source_records", JsonValue::Array(records))?;
        let contracts = public_sources
            .fixity
            .iter()
            .filter(|row| {
                row.object_get("path")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|path| !paths.contains(path))
            })
            .cloned()
            .collect();
        cmd::set(
            &mut context,
            "source_contracts",
            JsonValue::Array(contracts),
        )?;
        if selected.version.has_native() {
            cmd::set(
                &mut context,
                "native_text_units",
                JsonValue::Array(public_sources.native_summaries.clone()),
            )?;
            let contracts = public_sources
                .native_contracts
                .iter()
                .map(|(path, digest)| {
                    cmd::object(vec![
                        ("path", cmd::string(path)),
                        ("digest", cmd::string(digest)),
                    ])
                })
                .collect();
            cmd::set(
                &mut context,
                "native_contracts",
                JsonValue::Array(contracts),
            )?;
        }
    }
    Ok(context)
}

pub(crate) enum AssessmentAppendPreparation {
    Replayed(JsonValue),
    New {
        batch: JsonValue,
        request_digest: Digest256,
        revision: String,
        result: JsonValue,
    },
}

/// Reuse the maintained append/replay, qualification, and immutable batch
/// preparation rules independently of the journal transport. The filesystem
/// journal and PG private-package broker publish this same prepared batch
/// through their own owner-specific currentness and commit boundaries.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_assessment_append<W: CutSchemaExecutor>(
    request_row: &ParsedRequest,
    recorded_at: &str,
    worker: &mut W,
    config: &JsonValue,
    scope: &JsonValue,
    current: &JsonValue,
    history: &AssessmentHistory,
    current_report: &AssessmentMechanicsReport,
    append_report: &AssessmentMechanicsReport,
    all_by_id: &BTreeMap<String, JsonValue>,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AssessmentAppendPreparation> {
    let command_id = request_row
        .command_id
        .as_deref()
        .ok_or(SourceCommandError::Invalid(
            "assessment append command identity absent",
        ))?;
    let execution = cmd::field(config, "execution_profile")?;
    let profile_id = cmd::text(execution, "id")?;
    let profile = all_by_id
        .get(profile_id)
        .or_else(|| {
            std::iter::once(cmd::field(config, "policy").ok()?)
                .chain(cmd::array(config, "authorities").ok()?.iter())
                .chain(cmd::array(config, "competencies").ok()?.iter())
                .find(|row| row.object_get("id").and_then(JsonValue::as_str) == Some(profile_id))
        })
        .ok_or(SourceCommandError::Denied(
            "configured assessment execution profile is not current",
        ))?;
    if !cmd::same(execution, &owner_reference(profile)?)? {
        return Err(SourceCommandError::Denied(
            "configured assessment execution profile is stale",
        ));
    }
    let expected_revision =
        request_row
            .expected_revision
            .as_ref()
            .ok_or(SourceCommandError::Invalid(
                "assessment expected revision absent",
            ))?;
    let submitted = append_event_rows(
        &request_row.assessments,
        cmd::text(config, "principal_id")?,
        execution,
    )?;
    let request = cmd::object(vec![
        ("command_id", cmd::string(command_id)),
        ("subject", owner_reference(current)?),
        ("events", JsonValue::Array(submitted.clone())),
        (
            "expected_revision",
            expected_revision
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("layer", cmd::field(scope, "assertion_layer")?.clone()),
        ("risk", cmd::field(scope, "risk")?.clone()),
        ("languages", cmd::field(scope, "languages")?.clone()),
        ("maker_id", cmd::field(scope, "maker_id")?.clone()),
        ("use", cmd::field(scope, "requested_use")?.clone()),
    ]);
    let request_digest = cmd::record_digest(&request)?;
    let request_digest_hex = request_digest.to_hex();
    for batch in &history.batches {
        if cmd::text(cmd::field(batch, "request")?, "command_id")? == command_id {
            if cmd::text(batch, "request_digest")? != request_digest_hex {
                return Err(SourceCommandError::Conflict(
                    "assessment command ID is bound to another request",
                ));
            }
            crate::source_sign::finish_worker(worker, deadline, cancelled)?;
            return Ok(AssessmentAppendPreparation::Replayed(cmd::object(vec![
                (
                    "revision",
                    history
                        .head
                        .as_deref()
                        .map(cmd::string)
                        .unwrap_or(JsonValue::Null),
                ),
                ("receipt", batch.clone()),
                ("replayed", JsonValue::Bool(true)),
                (
                    "current_admission",
                    current_admission_with_limits(current_report, layers, &request_row.subject_id)?,
                ),
            ])));
        }
    }
    if history.head.as_ref() != expected_revision.as_ref() {
        return Err(SourceCommandError::Conflict(
            "assessment expected journal revision is stale",
        ));
    }
    if let Some(previous) = history.batches.last() {
        if tos_validation::retirement_rules::observed_instant_order(
            recorded_at,
            cmd::text(previous, "recorded_at")?,
        )
        .map_err(|_| SourceCommandError::Invalid("assessment append chronology"))?
            == std::cmp::Ordering::Less
        {
            return Err(SourceCommandError::Conflict(
                "assessment journal time cannot move backward",
            ));
        }
    }
    let append_admission = report_admission(append_report)?;
    let submitted_ids = submitted
        .iter()
        .map(|event| {
            cmd::text(cmd::field(event, "assessment")?, "assessment_id").map(str::to_owned)
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let invalid = cmd::array(&append_admission, "invalid_assessments")?;
    if invalid.iter().any(|entry| {
        entry
            .object_get("assessment_id")
            .and_then(JsonValue::as_str)
            .is_some_and(|identity| submitted_ids.contains(identity))
    }) {
        return Err(SourceCommandError::Denied(
            "assessment append includes an unqualified new review",
        ));
    }
    let old_ids = history
        .submissions
        .iter()
        .map(|submission| {
            let assessment = cmd::parse(&submission.assessment)?;
            Ok(cmd::text(&assessment, "assessment_id")?.to_owned())
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let mut unique = BTreeMap::<Vec<u8>, JsonValue>::new();
    for event in submitted {
        let id = cmd::text(cmd::field(&event, "assessment")?, "assessment_id")?;
        if !old_ids.contains(id) {
            unique.entry(cmd::canonical(&event)?).or_insert(event);
        }
    }
    if unique.is_empty() {
        return Err(SourceCommandError::Conflict(
            "assessment batch contains no new review event",
        ));
    }
    let committed_events = unique.into_values().collect::<Vec<_>>();
    let previous_revision = expected_revision
        .as_deref()
        .map(cmd::string)
        .unwrap_or(JsonValue::Null);
    let batch = cmd::object(vec![
        ("schema_version", cmd::string("tos_assessment_batch_v1")),
        ("subject_id", cmd::string(&request_row.subject_id)),
        ("sequence", cmd::number(history.batches.len() as u64 + 1)),
        ("previous_revision", previous_revision),
        ("recorded_at", cmd::string(recorded_at)),
        ("request", request),
        ("request_digest", cmd::string(&request_digest_hex)),
        ("events", JsonValue::Array(committed_events)),
        (
            "qualification",
            cmd::object(vec![
                ("all_new_events_qualified", JsonValue::Bool(true)),
                ("policy", owner_reference(cmd::field(config, "policy")?)?),
                ("is_semantic_evaluation", JsonValue::Bool(false)),
            ]),
        ),
        ("admission_at_commit", append_admission),
    ]);
    let payload = history.checked_successor_payload(&batch)?;
    if payload.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid("assessment batch byte budget"));
    }
    if !worker
        .check(
            "assessment/native-journal-batch",
            &payload,
            BATCH_SCHEMA,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Invalid("assessment batch schema execution"))?
    {
        return Err(SourceCommandError::Invalid("assessment batch schema"));
    }
    crate::source_sign::finish_worker(worker, deadline, cancelled)?;
    let revision = cmd::record_digest(&batch)?.to_hex();
    let result = cmd::object(vec![
        ("revision", cmd::string(&revision)),
        ("receipt", batch.clone()),
        ("replayed", JsonValue::Bool(false)),
        (
            "current_admission",
            current_admission_with_limits(append_report, layers, &request_row.subject_id)?,
        ),
    ]);
    Ok(AssessmentAppendPreparation::New {
        batch,
        request_digest,
        revision,
        result,
    })
}

#[allow(clippy::too_many_arguments)]
fn append_assessment_batch(
    selected: &SelectedOperationView<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    config: &JsonValue,
    scope: &JsonValue,
    current: &JsonValue,
    history: &AssessmentHistory,
    fence: &AssessmentJournalFence<'_>,
    current_report: &AssessmentMechanicsReport,
    append_report: &AssessmentMechanicsReport,
    _admission_bases: &[(
        QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )],
    all_by_id: &BTreeMap<String, JsonValue>,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    public_sources: &mut crate::source_sign::OwnerAssessmentPublicSources<'_>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, bool)> {
    let preparation = prepare_assessment_append(
        selected.request,
        &selected.context.recorded_at,
        worker,
        config,
        scope,
        current,
        history,
        current_report,
        append_report,
        all_by_id,
        layers,
        deadline,
        cancelled,
    )?;
    match preparation {
        AssessmentAppendPreparation::Replayed(result) => Ok((result, false)),
        AssessmentAppendPreparation::New {
            batch,
            revision,
            result,
            ..
        } => {
            let mut currentness = || {
                verify_selected_sources(
                    selected,
                    public_sources,
                    layers,
                    software,
                    components,
                    deadline,
                    cancelled,
                )
            };
            let committed_revision = fence.publish(
                &selected.request.subject_id,
                history,
                selected
                    .request
                    .expected_revision
                    .as_ref()
                    .and_then(|revision| revision.as_deref()),
                &batch,
                || currentness(),
                deadline,
                cancelled,
            )?;
            if committed_revision != revision {
                return Err(SourceCommandError::Conflict(
                    "assessment journal published a different batch revision",
                ));
            }
            Ok((result, true))
        }
    }
}

pub(crate) fn append_event_rows(
    assessments: &[JsonValue],
    principal_id: &str,
    execution_profile: &JsonValue,
) -> SourceCommandResult<Vec<JsonValue>> {
    let mut events = Vec::with_capacity(assessments.len());
    for assessment in assessments {
        events.push(cmd::object(vec![
            ("assessment", assessment.clone()),
            ("principal_id", cmd::string(principal_id)),
            (
                "execution_profile",
                cmd::canonical(execution_profile).and_then(|raw| cmd::parse(&raw))?,
            ),
        ]));
    }
    let mut keyed = events
        .into_iter()
        .map(|event| Ok((cmd::canonical(&event)?, event)))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, event)| event).collect())
}

fn expected_source_form_set_path(
    source_path: &str,
    subject_id: &str,
) -> SourceCommandResult<String> {
    let leaf = source_path.rsplit('/').next().unwrap_or(source_path);
    if matches!(leaf, "source-claims.jsonl" | "historical-claims.jsonl") {
        let stem = source_path
            .strip_suffix(".jsonl")
            .ok_or(SourceCommandError::Invalid(
                "materialization Claim stream path",
            ))?;
        let suffix = Digest256::of_bytes(subject_id.as_bytes()).to_hex();
        return Ok(format!("{stem}.{suffix}.human-forms.json"));
    }
    let stem = source_path
        .strip_suffix(".json")
        .ok_or(SourceCommandError::Denied(
            "materialization subject source path is not a current metadata record",
        ))?;
    Ok(format!("{stem}.human-forms.json"))
}

#[allow(clippy::too_many_arguments)]
fn materialize_selected_form(
    selected: &SelectedOperationView<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    current: &JsonValue,
    scope: &JsonValue,
    history: &AssessmentHistory,
    current_report: &AssessmentMechanicsReport,
    admission_bases: &[(
        QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )],
    parent_assessment: Option<&JsonValue>,
    all_by_id: &BTreeMap<String, JsonValue>,
    required_sources: &[JsonValue],
    public_sources: &mut crate::source_sign::OwnerAssessmentPublicSources<'_>,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    materialize_assessed_form(
        Some(&selected.private_sources),
        &selected.request.subject_id,
        worker,
        current,
        scope,
        history,
        current_report,
        admission_bases,
        parent_assessment,
        all_by_id,
        required_sources,
        public_sources,
        layers,
        deadline,
        cancelled,
    )
}

/// Materialize one selected public or owner-local form from exact records and
/// current admission. Source and journal selection stay with the caller; this
/// adapter grants no authority and performs no filesystem operations.
#[allow(clippy::too_many_arguments)]
fn materialize_assessed_form(
    private_sources: Option<&PrivateAssessmentSources>,
    selected_subject_id: &str,
    worker: &mut CutWorkerSchemaExecutor,
    current: &JsonValue,
    scope: &JsonValue,
    history: &AssessmentHistory,
    current_report: &AssessmentMechanicsReport,
    admission_bases: &[(
        QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )],
    parent_assessment: Option<&JsonValue>,
    all_by_id: &BTreeMap<String, JsonValue>,
    required_sources: &[JsonValue],
    public_sources: &mut crate::source_sign::OwnerAssessmentPublicSources<'_>,
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let private_maps = private_sources.map(|sources| AssessmentFormMaps {
        form_paths: sources.form_paths(),
        form_sets: sources.form_sets(),
        source_paths: &sources.source_paths,
    });
    let public_form_paths = public_sources.form_paths.clone();
    let public_form_sets = public_sources.form_sets.clone();
    let public_source_paths = public_sources.record_paths.clone();
    let public_maps = AssessmentFormMaps {
        form_paths: &public_form_paths,
        form_sets: &public_form_sets,
        source_paths: &public_source_paths,
    };
    materialize_assessed_form_from_selected_maps(
        private_maps.as_ref(),
        &public_maps,
        private_sources.is_some(),
        selected_subject_id,
        worker,
        current,
        scope,
        history,
        current_report,
        admission_bases,
        parent_assessment,
        all_by_id,
        required_sources,
        layers,
        |path, worker, deadline, cancelled| {
            public_sources.validate_form_set(path, worker, deadline, cancelled)
        },
        deadline,
        cancelled,
    )
}

/// Transport-neutral form materializer used after public/private source
/// adapters have selected exact forms and subject paths. The supplied
/// validator binds a public form set to the caller's selected contract bytes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize_assessed_form_from_selected_maps<W: CutSchemaExecutor>(
    private_maps: Option<&AssessmentFormMaps<'_>>,
    public_maps: &AssessmentFormMaps<'_>,
    has_private_sources: bool,
    selected_subject_id: &str,
    worker: &mut W,
    current: &JsonValue,
    scope: &JsonValue,
    history: &AssessmentHistory,
    current_report: &AssessmentMechanicsReport,
    admission_bases: &[(
        QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )],
    parent_assessment: Option<&JsonValue>,
    all_by_id: &BTreeMap<String, JsonValue>,
    required_sources: &[JsonValue],
    layers: Option<&crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    mut validate_public_form_set: impl FnMut(
        &str,
        &mut W,
        Instant,
        &AtomicBool,
    ) -> SourceCommandResult<()>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    active(deadline, cancelled)?;
    let form_id = cmd::text(current, "id")?;
    let form_ref = owner_reference(current)?;
    let form_body = cmd::field(current, "payload")?;
    if cmd::text(form_body, "schema_version")? != "tos_human_form_v1" {
        return Err(SourceCommandError::Denied(
            "materialization requires an explicitly selected source form",
        ));
    }
    let subject_ref = cmd::field(form_body, "subject")?;
    let subject_id = cmd::text(subject_ref, "id")?;
    let (form_set_path, form_set) =
        if let Some(path) = private_maps.and_then(|maps| maps.form_paths.get(form_id)) {
            let form_set = private_maps
                .and_then(|maps| maps.form_sets.get(path))
                .ok_or(SourceCommandError::Conflict(
                    "materialization form set is absent from the current selected source closure",
                ))?;
            (path.clone(), form_set.clone())
        } else if let Some(path) = public_maps.form_paths.get(form_id).cloned() {
            if public_maps.source_paths.get(form_id) != Some(&path) {
                return Err(SourceCommandError::Denied(
                    "materialization public form path is not exact-selected",
                ));
            }
            validate_public_form_set(&path, worker, deadline, cancelled)?;
            let form_set = public_maps
                .form_sets
                .get(&path)
                .ok_or(SourceCommandError::Conflict(
                    "materialization public form set is absent from current selected sources",
                ))?
                .clone();
            (path, form_set)
        } else {
            return Err(SourceCommandError::Denied(
                "materialization form path is not source-selected",
            ));
        };
    let Some(subject) = all_by_id.get(subject_id) else {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "unavailable",
            "subject.unavailable",
            None,
            parent_assessment,
        ));
    };
    let subject_path = private_maps
        .and_then(|maps| maps.source_paths.get(subject_id))
        .or_else(|| public_maps.source_paths.get(subject_id));
    if let Some(subject_path) = subject_path {
        if expected_source_form_set_path(subject_path, subject_id)? != form_set_path {
            return Err(SourceCommandError::Denied(
                "materialization form set is not adjacent to its exact selected subject",
            ));
        }
    } else {
        return Err(SourceCommandError::Denied(
            "materialization subject has no exact selected source path",
        ));
    }
    if subject_id == form_id {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "invalid",
            "form.identity-or-subject",
            None,
            parent_assessment,
        ));
    }
    if cmd::text(form_body, "form_id")? != form_id
        || cmd::integer(form_body, "form_version")? != cmd::integer(current, "version")?
    {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "invalid",
            "form.identity-or-subject",
            None,
            parent_assessment,
        ));
    }
    if !cmd::same(subject_ref, &owner_reference(subject)?)?
        || !cmd::same(cmd::field(&form_set, "subject")?, subject_ref)?
    {
        return Err(SourceCommandError::Conflict(
            "materialization form binds another current source subject",
        ));
    }
    let forms = cmd::array(&form_set, "forms")?;
    let mut matches = 0usize;
    for candidate in forms {
        if cmd::same(candidate, form_body)? {
            matches += 1;
        }
    }
    if matches != 1 {
        return Err(SourceCommandError::Conflict(
            "current materialization form is absent or repeated in its exact source set",
        ));
    }
    let content = cmd::field(form_body, "content")?;
    let kind = cmd::text(content, "kind")?;
    if !matches!(kind, "source-copy" | "freeform") {
        return Err(SourceCommandError::Denied(
            "assessed form materialization supports only source-copy and freeform",
        ));
    }
    if cmd::text(form_body, "creator_id")? != cmd::text(scope, "maker_id")? {
        return Err(SourceCommandError::Denied(
            "materialization form creator differs from configured assessment maker",
        ));
    }
    if form_body.object_get("language") != Some(&JsonValue::Null)
        && form_body.object_get("language").is_some()
    {
        let language = cmd::text(form_body, "language")?;
        let folded =
            tos_foundation::python_casefold_unicode16_v1(language, language.len(), 512, 2048)
                .map_err(|_| SourceCommandError::Invalid("materialization language budget"))?;
        let allowed = cmd::array(scope, "languages")?
            .iter()
            .filter_map(JsonValue::as_str)
            .map(|value| {
                tos_foundation::python_casefold_unicode16_v1(value, value.len(), 512, 2048)
                    .map_err(|_| SourceCommandError::Invalid("materialization language budget"))
            })
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        if !allowed.contains(&folded) {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "invalid",
                "form.language-outside-scope",
                None,
                parent_assessment,
            ));
        }
    }
    let configured_language_context = scope
        .object_get("form_language_context")
        .filter(|value| !value.is_null());
    let authored_language_context = form_body.object_get("language_context");
    let expected_language_context = configured_language_context
        .map(|value| source_binding_reference(value))
        .transpose()?;
    if match (&expected_language_context, authored_language_context) {
        (None, None | Some(JsonValue::Null)) => false,
        (Some(expected), Some(actual)) => !cmd::same(expected, actual)?,
        (None, Some(actual)) => !actual.is_null(),
        (Some(_), None) => true,
    } {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "invalid",
            "language-context.outside-owner-scope",
            None,
            parent_assessment,
        ));
    }

    let mut current_rows = all_by_id.clone();
    for (_, basis, _, _) in admission_bases {
        let row = parse_record_input(basis)?;
        let identity = cmd::text(&row, "id")?.to_owned();
        if let Some(previous) = current_rows.get(&identity) {
            if !cmd::same(previous, &row)? {
                return Err(SourceCommandError::Denied(
                    "current quality basis shadows another source identity",
                ));
            }
        } else {
            current_rows.insert(identity, row);
        }
    }
    let mut dependencies = Vec::new();
    dependencies.push(subject_ref.clone());
    for reference in required_sources {
        let Some(source) = current_rows.get(cmd::text(reference, "id")?) else {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "unavailable",
                "required-source.unavailable",
                None,
                parent_assessment,
            ));
        };
        if !cmd::same(reference, &owner_reference(source)?)? {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "stale",
                "required-source.changed",
                None,
                parent_assessment,
            ));
        }
        dependencies.push(reference.clone());
    }
    for (_, basis, _, _) in admission_bases {
        let row = parse_record_input(basis)?;
        dependencies.push(owner_reference(&row)?);
    }
    let binding_rows = cmd::field(form_body, "bindings")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("materialization form bindings"))?;
    let mut values = BTreeMap::<String, JsonValue>::new();
    for (slot, binding) in binding_rows {
        active(deadline, cancelled)?;
        let slot = slot
            .as_str()
            .ok_or(SourceCommandError::Invalid("materialization form slot"))?;
        let reference = cmd::field(binding, "record")?;
        let identity = cmd::text(reference, "id")?;
        let Some(source) = current_rows.get(identity) else {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "unavailable",
                &format!("binding.unavailable:{slot}"),
                None,
                parent_assessment,
            ));
        };
        if !cmd::same(reference, &owner_reference(source)?)? {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "stale",
                &format!("binding.changed:{slot}"),
                None,
                parent_assessment,
            ));
        }
        let pointer = cmd::text(binding, "pointer")?;
        let value = match pointer_value(cmd::field(source, "payload")?, pointer) {
            Some(value) => value,
            None => {
                return Ok(materialization_stop(
                    &form_ref,
                    subject_ref,
                    "invalid",
                    &format!("binding.pointer:{slot}"),
                    None,
                    parent_assessment,
                ));
            }
        };
        values.insert(slot.to_owned(), value);
        dependencies.push(reference.clone());
    }
    let binding_map = binding_rows
        .iter()
        .map(|(slot, binding)| {
            Ok((
                cmd::canonical(&source_binding_reference(binding)?)?,
                slot.as_str()
                    .ok_or(SourceCommandError::Invalid("materialization form slot"))?
                    .to_owned(),
            ))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;

    let mut required_context = Vec::<JsonValue>::new();
    let mut source_copy_binding = None;
    let wording;
    if kind == "source-copy" {
        let slot = cmd::text(content, "slot")?;
        let binding = binding_rows
            .iter()
            .find(|(key, _)| key.as_str() == Some(slot))
            .map(|(_, value)| value)
            .ok_or(SourceCommandError::Invalid(
                "source-copy wording slot is not bound",
            ))?;
        if !cmd::same(cmd::field(binding, "record")?, subject_ref)? {
            return Err(SourceCommandError::Denied(
                "source-copy form must bind the exact selected source subject",
            ));
        }
        let pointer = cmd::text(binding, "pointer")?;
        let subject_payload = cmd::field(subject, "payload")?;
        let fields = if !has_private_sources && subject_payload.object_get("claim_id").is_some() {
            public_claim_form_fields(subject_payload, worker, deadline, cancelled)?
        } else {
            crate::source_forms::metadata_fields(subject_payload)?
        };
        let role = cmd::text(form_body, "role")?;
        let matches = fields
            .into_iter()
            .filter(|field| field.role == role && field.pointer == pointer)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(SourceCommandError::Denied(
                "source-copy form field is not an exact source-owned field and role",
            ));
        }
        let field = matches
            .into_iter()
            .next()
            .ok_or(SourceCommandError::Invalid("source-copy field selection"))?;
        for pointer in &field.context {
            required_context.push(source_binding_ref(subject, pointer)?);
        }
        let value = values.get(slot).ok_or(SourceCommandError::Invalid(
            "source-copy wording binding is absent",
        ))?;
        wording = value
            .as_str()
            .ok_or(SourceCommandError::Invalid("source-copy requires text"))?
            .to_owned();
        if !cmd::nonblank(&wording) {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "invalid",
                "source-copy.requires-complete-nonempty-string",
                None,
                parent_assessment,
            ));
        }
        if cmd::field(form_body, "language")? != &field.language
            || cmd::field(form_body, "script")? != &field.script
        {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "invalid",
                "source-copy.language-not-bound-to-source",
                None,
                parent_assessment,
            ));
        }
        source_copy_binding = Some(source_binding_ref(subject, pointer)?);
    } else {
        required_context.push(source_binding_ref(subject, "")?);
        wording = cmd::text(content, "text")?.to_owned();
    }

    let mut language_context_value = None;
    if let Some(language_context) = configured_language_context {
        let language_ref = source_binding_reference(language_context)?;
        let slot_key = cmd::canonical(&language_ref)?;
        let Some(slot) = binding_map.get(&slot_key) else {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "invalid",
                "context.omitted",
                None,
                parent_assessment,
            ));
        };
        let metadata = values
            .get(slot)
            .ok_or(SourceCommandError::Invalid("language context value absent"))?;
        let metadata_raw = cmd::canonical(metadata)?;
        match worker.check_reusing_scalar(
            "assessment/form-language-context",
            &metadata_raw,
            "ToS/contracts/human-form.schema.json#/$defs/languageContext",
            deadline,
            cancelled,
        ) {
            Ok(true) => (),
            Ok(false) => {
                return Ok(materialization_stop(
                    &form_ref,
                    subject_ref,
                    "invalid",
                    "language-context.schema",
                    None,
                    parent_assessment,
                ));
            }
            Err(reason) => {
                return Err(SourceCommandError::SchemaExecution {
                    path: "assessment/form-language-context".to_owned(),
                    root: "ToS/contracts/human-form.schema.json#/$defs/languageContext".into(),
                    reason,
                });
            }
        }
        if cmd::field(metadata, "language")? != cmd::field(form_body, "language")?
            || cmd::field(metadata, "script")? != cmd::field(form_body, "script")?
        {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "invalid",
                "language-context.language-or-script-mismatch",
                None,
                parent_assessment,
            ));
        }
        required_context.push(language_ref.clone());
        if let Some(source) = metadata
            .object_get("source")
            .filter(|source| !source.is_null())
        {
            let source_binding = source_binding_reference(source)?;
            let source_id = cmd::text(cmd::field(&source_binding, "record")?, "id")?;
            let source_copy_self = if kind == "source-copy" {
                if let Some(copy) = source_copy_binding.as_ref() {
                    cmd::same(copy, &source_binding)?
                } else {
                    false
                }
            } else {
                false
            };
            if source_id == form_id || source_copy_self {
                return Ok(materialization_stop(
                    &form_ref,
                    subject_ref,
                    "invalid",
                    "language-context.self-derivation",
                    None,
                    parent_assessment,
                ));
            }
            let source_key = cmd::canonical(&source_binding)?;
            let Some(source_slot) = binding_map.get(&source_key) else {
                return Ok(materialization_stop(
                    &form_ref,
                    subject_ref,
                    "invalid",
                    "context.omitted",
                    None,
                    parent_assessment,
                ));
            };
            if values
                .get(source_slot)
                .and_then(JsonValue::as_str)
                .is_none_or(|value| !cmd::nonblank(value))
            {
                return Ok(materialization_stop(
                    &form_ref,
                    subject_ref,
                    "invalid",
                    "language-context.source-requires-complete-nonempty-string",
                    None,
                    parent_assessment,
                ));
            }
            required_context.push(source_binding);
        }
        language_context_value = Some(cmd::object(vec![
            ("binding", language_ref),
            ("value", metadata.clone()),
        ]));
    }
    let root_subject_binding = source_binding_ref(subject, "")?;
    let mut root_context_required = false;
    for reference in &required_context {
        root_context_required |= cmd::same(reference, &root_subject_binding)?;
    }
    let owner_subject_context = kind == "source-copy" && !root_context_required;
    let mut context = Vec::new();
    for reference in &required_context {
        let key = cmd::canonical(reference)?;
        let Some(slot) = binding_map.get(&key) else {
            return Ok(materialization_stop(
                &form_ref,
                subject_ref,
                "invalid",
                "context.omitted",
                None,
                parent_assessment,
            ));
        };
        context.push(cmd::object(vec![
            ("slot", cmd::string(slot)),
            ("binding", reference.clone()),
            (
                "value",
                values
                    .get(slot)
                    .cloned()
                    .ok_or(SourceCommandError::Invalid("form context value absent"))?,
            ),
        ]));
    }
    if owner_subject_context {
        context.push(cmd::object(vec![
            ("slot", cmd::string("owner:subject")),
            ("binding", root_subject_binding),
            ("value", cmd::field(subject, "payload")?.clone()),
        ]));
    }
    for (index, (_, basis, _, _)) in admission_bases.iter().enumerate() {
        let row = parse_record_input(basis)?;
        context.push(cmd::object(vec![
            ("slot", cmd::string(&format!("owner:quality:{index}"))),
            (
                "binding",
                cmd::object(vec![
                    ("record", owner_reference(&row)?),
                    ("pointer", cmd::string("")),
                ]),
            ),
            ("value", cmd::field(&row, "payload")?.clone()),
        ]));
    }
    let admission = current_admission_with_limits(current_report, layers, selected_subject_id)?;
    if admission_bases.iter().any(|(_, _, can_use, _)| !can_use) {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "needs-assessment",
            "source-quality.not-admitted",
            Some(admission),
            parent_assessment,
        ));
    }
    if cmd::field(&admission, "can_use")? != &JsonValue::Bool(true) {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "needs-assessment",
            "form.not-admitted",
            Some(admission),
            parent_assessment,
        ));
    }
    if !cmd::nonblank(&wording) {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "invalid",
            "form.empty",
            Some(admission),
            parent_assessment,
        ));
    }
    let mut seen_dependencies = BTreeSet::<Vec<u8>>::new();
    let mut unique = Vec::<JsonValue>::new();
    for reference in dependencies {
        if seen_dependencies.insert(cmd::canonical(&reference)?) {
            unique.push(reference);
        }
    }
    let mut materialization = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_human_form_materialization_v1"),
        ),
        ("form", form_ref.clone()),
        ("subject", subject_ref.clone()),
        ("state", cmd::string("ready")),
        ("display_text", cmd::string(&wording)),
        ("context", JsonValue::Array(context.clone())),
        ("issues", JsonValue::Array(Vec::new())),
        ("admission", admission),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
        ("role", cmd::field(form_body, "role")?.clone()),
        ("language", cmd::field(form_body, "language")?.clone()),
        ("script", cmd::field(form_body, "script")?.clone()),
        ("derivation", cmd::string(kind)),
        ("dependencies", JsonValue::Array(unique)),
        (
            "standalone_reading",
            JsonValue::Bool(context.is_empty() && parent_assessment.is_none()),
        ),
    ]);
    if let Some(packet) = parent_assessment {
        cmd::set(&mut materialization, "subject_assessment", packet.clone())?;
    }
    if let Some(language_context) = language_context_value {
        cmd::set(&mut materialization, "language_context", language_context)?;
    }
    if cmd::canonical(&materialization)?.len() > 65_536 {
        return Ok(materialization_stop(
            &form_ref,
            subject_ref,
            "over-budget",
            "form.output-budget-exceeded-do-not-truncate",
            None,
            parent_assessment,
        ));
    }
    Ok(materialization)
}

fn public_claim_form_fields<W: CutSchemaExecutor>(
    payload: &JsonValue,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<crate::source_forms::FormField>> {
    let qualifiers = cmd::field(payload, "qualifiers")?;
    let display = qualifiers.object_get("display_fields");
    let understood_display = display.is_some_and(|value| {
        value
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            == Some("tos_claim_display_fields_v1")
    });
    if understood_display {
        let raw = cmd::canonical(qualifiers)?;
        match worker.check_reusing_scalar(
            "assessment/claim-display-fields",
            &raw,
            "ToS/contracts/claim-display-fields.schema.json",
            deadline,
            cancelled,
        ) {
            Ok(true) => (),
            Ok(false) => {
                return Err(SourceCommandError::Invalid(
                    "public Claim display fields violate their contract",
                ));
            }
            Err(reason) => {
                return Err(SourceCommandError::SchemaExecution {
                    path: "assessment/claim-display-fields".to_owned(),
                    root: "ToS/contracts/claim-display-fields.schema.json".into(),
                    reason,
                });
            }
        }
    }
    let Some(_statement) = qualifiers
        .object_get("statement")
        .and_then(JsonValue::as_str)
        .filter(|value| cmd::nonblank(value))
    else {
        return crate::source_forms::metadata_fields(payload);
    };
    let language = cmd::field(qualifiers, "statement_language")?.clone();
    let script = cmd::field(qualifiers, "statement_script")?.clone();
    let language_input = cmd::object(vec![(
        "notes",
        cmd::object(vec![
            ("language", language.clone()),
            ("script", script.clone()),
        ]),
    )]);
    let raw = cmd::canonical(&language_input)?;
    match worker.check_reusing_scalar(
        "assessment/claim-statement-language",
        &raw,
        "ToS/contracts/corpus-record.schema.json#/properties/field_languages",
        deadline,
        cancelled,
    ) {
        Ok(true) => (),
        Ok(false) => {
            return Err(SourceCommandError::Invalid(
                "public Claim statement language/script violates the source-form contract",
            ));
        }
        Err(reason) => {
            return Err(SourceCommandError::SchemaExecution {
                path: "assessment/claim-statement-language".to_owned(),
                root: "ToS/contracts/corpus-record.schema.json#/properties/field_languages".into(),
                reason,
            });
        }
    }
    crate::source_forms::metadata_fields(payload)
}

fn source_binding_ref(record: &JsonValue, pointer: &str) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("record", owner_reference(record)?),
        ("pointer", cmd::string(pointer)),
    ]))
}

fn source_binding_reference(binding: &JsonValue) -> SourceCommandResult<JsonValue> {
    cmd::exact_keys(binding, &["record", "pointer"])?;
    let reference = cmd::field(binding, "record")?.clone();
    let pointer = cmd::text(binding, "pointer")?;
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return Err(SourceCommandError::Invalid(
            "materialization source pointer",
        ));
    }
    Ok(cmd::object(vec![
        ("record", reference),
        ("pointer", cmd::string(pointer)),
    ]))
}

fn pointer_value(value: &JsonValue, pointer: &str) -> Option<JsonValue> {
    if pointer.is_empty() {
        return Some(value.clone());
    }
    let mut current = value;
    for raw_token in pointer.strip_prefix('/')?.split('/') {
        let token = raw_token.replace("~1", "/").replace("~0", "~");
        current = match current {
            JsonValue::Object(_) => current.object_get(&token)?,
            JsonValue::Array(rows)
                if token.is_ascii()
                    && token.bytes().all(|byte| byte.is_ascii_digit())
                    && (token == "0" || !token.starts_with('0')) =>
            {
                rows.get(token.parse::<usize>().ok()?)?
            }
            _ => return None,
        };
    }
    Some(current.clone())
}

fn materialization_stop(
    form_ref: &JsonValue,
    subject_ref: &JsonValue,
    state: &str,
    issue: &str,
    admission: Option<JsonValue>,
    subject_assessment: Option<&JsonValue>,
) -> JsonValue {
    let mut result = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_human_form_materialization_v1"),
        ),
        ("form", form_ref.clone()),
        ("subject", subject_ref.clone()),
        ("state", cmd::string(state)),
        ("display_text", JsonValue::Null),
        ("context", JsonValue::Array(Vec::new())),
        ("issues", JsonValue::Array(vec![cmd::string(issue)])),
        ("admission", admission.unwrap_or(JsonValue::Null)),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
    ]);
    if let Some(packet) = subject_assessment {
        let _ = cmd::set(&mut result, "subject_assessment", packet.clone());
    }
    result
}

#[cfg(test)]
#[path = "../tests/fixtures/capacity-d-v1/assessment_operation_tests.rs"]
mod capacity_d_r4;

use crate::temporal_research_crosswalk::c01_each;

/// Borrow-only resolver inputs: no resolved holder crosses the C01 boundary.
pub(crate) struct AssessmentBorrowedPlanInputs<'a> {
    pub(crate) config: &'a JsonValue,
    pub(crate) configuration_raw: &'a [u8],
    pub(crate) request: &'a ParsedRequest,
    pub(crate) version: OwnerVersion,
    pub(crate) public_rows: &'a [JsonValue],
    pub(crate) private_rows: &'a [AssessmentRecordInput],
    pub(crate) public_native_rows: &'a [JsonValue],
    pub(crate) private_native_rows: &'a [AssessmentRecordInput],
    pub(crate) public_fixity: &'a [JsonValue],
    pub(crate) public_identity_snapshots: &'a BTreeMap<String, JsonValue>,
    pub(crate) public_claim_dependencies: &'a BTreeMap<String, Vec<JsonValue>>,
    pub(crate) private_source_snapshot: &'a str,
    pub(crate) private_native_snapshots: &'a [String],
    pub(crate) private_required_source_refs: &'a BTreeMap<String, Vec<JsonValue>>,
    pub(crate) private_required_languages: &'a BTreeMap<String, Vec<String>>,
    pub(crate) native_summaries: &'a [JsonValue],
    pub(crate) layers: Option<&'a crate::source_private_assessment_layers::PrivateAssessmentLayers>,
    pub(crate) layer_preflight:
        Option<&'a crate::source_private_assessment_layers::OwnerLocalLayerPreflight>,
}

pub(crate) fn selected_c01_command_context_with_return_limit(
    cut: &CorpusCutReader,
    configuration_raw: &[u8],
    request_raw: &[u8],
    operation: &mut OperationBudget,
    adapter: &mut crate::temporal_research_crosswalk::C01AdapterPayer<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CommandContext> {
    use crate::temporal_research_crosswalk::{C01Counter, C01Row};
    let mut a = adapter.reserve(C01Row::Context)?;
    operation.c01_interface_profile(&mut a)?;
    // The original builder never inspects context_configuration (let _ = ...).
    // Keep actual config/request BYTES and full23-file selection unchanged;
    // do not import a caller-owned unused DOM before its accounting boundary.
    let value = match selected_command_context(
        cut,
        configuration_raw,
        request_raw,
        JsonValue::Null,
        operation,
        deadline,
        cancelled,
    ) {
        Ok(value) => value,
        Err(error) => return a.error(error),
    };
    {
        let mut c = C01Counter::new(&mut a)?;
        let CommandContext {
            base_revision,
            configuration_raw,
            request_raw,
            recorded_at,
            effective_uid,
            files,
        } = &value;
        c.root::<SourceCommandResult<CommandContext>>()?;
        c.visit(2)?;
        let _ = (base_revision, effective_uid);
        c.bytes(configuration_raw)?;
        c.bytes(request_raw)?;
        c.string(recorded_at)?;
        c.visit(1)?;
        c.slots::<SourceFile>(files.capacity())?;
        c01_each!(c, files, file, {
            c.visit(1)?;
            let SourceFile { path, raw } = file;
            c.relative_path(path)?;
            c.bytes(raw)?;
        });
        c.visit(1)?;
    }
    Ok(value)
}

pub(crate) fn resolve_and_plan_c01_with_return_limit<'a>(
    inputs: AssessmentBorrowedPlanInputs<'a>,
    operation: &mut OperationBudget,
    adapter: &mut crate::temporal_research_crosswalk::C01AdapterPayer<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AssessmentEvaluationPlan<'a>> {
    use crate::temporal_research_crosswalk::{C01Counter, C01Row};
    let mut a = adapter.reserve(C01Row::Plan)?;
    operation.c01_interface_profile(&mut a)?;
    let AssessmentBorrowedPlanInputs {
        config,
        configuration_raw,
        request,
        version,
        public_rows,
        private_rows,
        public_native_rows,
        private_native_rows,
        public_fixity,
        public_identity_snapshots,
        public_claim_dependencies,
        private_source_snapshot,
        private_native_snapshots,
        private_required_source_refs,
        private_required_languages,
        native_summaries,
        layers,
        layer_preflight,
    } = inputs;
    a.public_v2(config)?;
    a.visit(64)?;
    if version != OwnerVersion::V2
        || request.operation != "inspect"
        || public_rows.len() != 1
        || !private_rows.is_empty()
        || !public_native_rows.is_empty()
        || !private_native_rows.is_empty()
        || !public_claim_dependencies.is_empty()
        || !private_native_snapshots.is_empty()
        || !private_required_source_refs.is_empty()
        || !private_required_languages.is_empty()
        || !native_summaries.is_empty()
        || layers.is_some()
        || layer_preflight.is_some()
    {
        return Err(SourceCommandError::Unsupported(
            "C01 V2 inspect selected-source profile",
        ));
    }
    a.six_envelopes(config)?;
    let resolved = match AssessmentResolvedSources::from_selected_rows(
        public_rows,
        private_rows,
        public_native_rows,
        private_native_rows,
        operation,
    ) {
        Ok(value) => value,
        Err(error) => return a.error(error),
    };
    let plan = match plan_assessment_evaluation(
        AssessmentEvaluationInputs {
            config,
            configuration_raw,
            request,
            version,
            resolved,
            public_fixity,
            public_identity_snapshots,
            public_claim_dependencies,
            private_source_snapshot,
            private_native_snapshots,
            private_required_source_refs,
            private_required_languages,
            native_summaries,
            layers,
            layer_preflight,
        },
        operation,
        deadline,
        cancelled,
    ) {
        Ok(value) => value,
        Err(error) => return a.error(error),
    };
    {
        let mut c = C01Counter::new(&mut a)?;
        plan.c01_count(&mut c)?;
    }
    Ok(plan)
}
impl AssessmentEvaluationPlan<'_> {
    fn c01_count<'v>(
        &'v self,
        c: &mut crate::temporal_research_crosswalk::C01Counter<'v, '_, '_>,
    ) -> SourceCommandResult<()> {
        let Self {
            config,
            request,
            version,
            resolved,
            all_by_id,
            base_snapshot,
            current,
            scope,
            subject_required_source_refs,
            required_sources,
            source_route,
            subject_quality_requirements,
            parent_context,
            parent_quality_requirements,
            lock_subjects,
            public_claim_dependencies,
            private_required_source_refs,
            layers,
        } = self;
        c.root::<SourceCommandResult<Self>>()?;
        c.visit(6)?;
        let _ = (
            config,
            request,
            version,
            public_claim_dependencies,
            private_required_source_refs,
            layers,
        );
        c.visit(64)?;
        if !matches!(source_route, AssessmentSourceRoute::SelectedSource)
            || !subject_quality_requirements.is_empty()
            || parent_context.is_some()
            || !parent_quality_requirements.is_empty()
            || layers.is_some()
        {
            return Err(SourceCommandError::Unsupported(
                "C01 no-quality/parent/layer plan",
            ));
        }
        let AssessmentResolvedSources {
            source_records,
            source_index,
            native_records,
            native_index,
            resolved_rows,
            resolved_index,
        } = resolved;
        c.json_vec(source_records)?;
        c.json_vec(native_records)?;
        c.json_vec(resolved_rows)?;
        macro_rules! index {
            ($map:expr) => {{
                c.visit(1)?;
                c.visit(1)?;
                c.btree::<String, usize>($map.len())?;
                c01_each!(c, $map, (key, value), {
                    c.visit(1)?;
                    c.string(key)?;
                    c.visit(1)?;
                    let _ = value;
                });
                c.visit(1)?;
            }};
        }
        index!(source_index);
        index!(native_index);
        index!(resolved_index);
        c.visit(1)?;
        c.btree::<String, JsonValue>(all_by_id.len())?;
        c01_each!(c, all_by_id, (key, value), {
            c.visit(1)?;
            c.string(key)?;
            c.json(value)?;
        });
        c.visit(1)?;
        c.string(base_snapshot)?;
        c.json(current)?;
        c.json(scope)?;
        c.json_vec(subject_required_source_refs)?;
        c.json_vec(required_sources)?;
        c.visit(1)?;
        c.slots::<QualityRequirement>(subject_quality_requirements.capacity())?;
        c.visit(1)?;
        c.slots::<QualityRequirement>(parent_quality_requirements.capacity())?;
        c.strings(lock_subjects)
    }
}

pub(crate) fn evaluate_assessment_histories_c01_with_return_limit<W, F>(
    plan: &AssessmentEvaluationPlan<'_>,
    histories: &BTreeMap<String, AssessmentHistory>,
    observed_now: &str,
    source_revision: Option<SourceRevision>,
    source_identity: Digest256,
    worker: &mut W,
    operation: &mut OperationBudget,
    adapter: &mut crate::temporal_research_crosswalk::C01AdapterPayer<'_>,
    cancelled: &AtomicBool,
    evaluate: &mut F,
) -> SourceCommandResult<AssessmentEvaluationResult>
where
    W: CutSchemaExecutor,
    F: FnMut(
        &AssessmentReadInput,
        &mut W,
        AssessmentLimits,
        &AtomicBool,
    ) -> SourceCommandResult<AssessmentMechanicsReport>,
{
    use crate::temporal_research_crosswalk::{C01Counter, C01Row};
    let mut a = adapter.reserve(C01Row::Result)?;
    operation.c01_interface_profile(&mut a)?;
    a.visit(64)?;
    if plan.version != OwnerVersion::V2
        || plan.request.operation != "inspect"
        || histories.len() != 1
        || plan.layers.is_some()
        || plan.parent_context.is_some()
        || !plan.subject_quality_requirements.is_empty()
        || !plan.parent_quality_requirements.is_empty()
        || !matches!(plan.source_route, AssessmentSourceRoute::SelectedSource)
    {
        return Err(SourceCommandError::Unsupported(
            "C01 closed inspect evaluation",
        ));
    }
    let result = match evaluate_assessment_histories(
        plan,
        histories,
        observed_now,
        source_revision,
        source_identity,
        worker,
        operation,
        cancelled,
        evaluate,
    ) {
        Ok(value) => value,
        Err(error) => return a.error(error),
    };
    {
        let mut c = C01Counter::new(&mut a)?;
        let AssessmentEvaluationResult {
            current_snapshot,
            quality_bases,
            admission_bases,
            current_report,
            append_report,
            parent_assessment,
            source_read_ready,
        } = &result;
        c.root::<SourceCommandResult<AssessmentEvaluationResult>>()?;
        c.visit(64)?;
        if !quality_bases.is_empty()
            || !admission_bases.is_empty()
            || append_report.is_some()
            || parent_assessment.is_some()
        {
            return Err(SourceCommandError::Unsupported("C01 closed inspect result"));
        }
        c.string(current_snapshot)?;
        c.visit(1)?;
        c.slots::<(QualityRequirement, JsonValue)>(quality_bases.capacity())?;
        c.visit(1)?;
        c.slots::<(
            QualityRequirement,
            AssessmentRecordInput,
            bool,
            Vec<JsonValue>,
        )>(admission_bases.capacity())?;
        c.report(current_report)?;
        c.visit(3)?;
        let _ = (append_report, parent_assessment, source_read_ready);
    }
    Ok(result)
}

impl OperationBudget {
    fn c01_interface_profile(
        &self,
        a: &mut crate::temporal_research_crosswalk::C01Admission<'_>,
    ) -> SourceCommandResult<()> {
        a.visit(64)?;
        if self.bytes_remaining > 33554432 || self.work_remaining > 4194304 {
            return Err(SourceCommandError::Invalid(
                "C01 delegated lower input/work profile",
            ));
        }
        Ok(())
    }
}
