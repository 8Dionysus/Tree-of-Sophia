//! Installed private v4-v6 native assessment caller.
//!
//! This is deliberately separate from the Sign v2/v3 promotion reader. It
//! selects the confidential owner configuration, current source context,
//! exact source closures and common append/replay journal before asking the
//! shared assessment policy engine for a current view.

use super::{absolute, capped, digest, selected_schema_with_profile, text};
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
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath};
use tos_source_store::{
    CorpusCutReader, CorpusReader, SoftwareCaptureReader, SoftwareComponentSelectionV1,
};
use tos_validation::assessment::{
    AssessmentLayerQualityObservation, AssessmentLimits, AssessmentMechanicsReport,
    AssessmentReadInput, AssessmentRecordInput, AssessmentRefusal, AssessmentSourceRoute,
    AssessmentSubmissionInput, MAX_ASSESSMENTS, MAX_RECORD_BYTES, evaluate_current_assessment,
};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const OWNER_CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const BATCH_SCHEMA: &str = "ToS/contracts/knowledge-assessment-batch.schema.json";
const QUALITY_BASIS_SCHEMA: &str = "ToS/contracts/native-text-layer-quality-basis.schema.json";
const MAX_OPERATION_BYTES: usize = 234_881_024;
const MAX_RESOLVED_BYTES: usize = 33_554_432;
const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_CONFIG_BYTES: usize = 8_388_608;
const MAX_QUALITY_SUBJECTS: usize = 64;

#[derive(Clone)]
struct ParsedRequest {
    operation: String,
    subject_id: String,
    expected_subject: Option<JsonValue>,
    expected_snapshot: Option<String>,
    command_id: Option<String>,
    expected_revision: Option<Option<String>>,
    assessments: Vec<JsonValue>,
}

struct SelectedOperation<'a> {
    invocation: &'a Value,
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

#[derive(Debug)]
struct OperationBudget {
    bytes_remaining: usize,
    work_remaining: usize,
    worker_cpu_seconds: u64,
    worker_address_space_bytes: u64,
    deadline: Instant,
}

impl OperationBudget {
    fn new(invocation: &Value, deadline: Instant) -> SourceCommandResult<Self> {
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

    fn charge_bytes(&mut self, bytes: usize) -> SourceCommandResult<()> {
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

    fn limits(&self) -> SourceCommandResult<AssessmentLimits> {
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

    fn charge_report(&mut self, report: &AssessmentMechanicsReport) -> SourceCommandResult<()> {
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

    fn charge_work(&mut self, work: usize) -> SourceCommandResult<()> {
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
enum OwnerVersion {
    V4,
    V5,
    V6,
}

impl OwnerVersion {
    fn from_config(config: &JsonValue) -> SourceCommandResult<Self> {
        match cmd::text(config, "schema_version")? {
            "tos_local_assessment_owner_v4" => Ok(Self::V4),
            "tos_local_assessment_owner_v5" => Ok(Self::V5),
            "tos_local_assessment_owner_v6" => Ok(Self::V6),
            _ => Err(SourceCommandError::Denied(
                "private assessment owner v4/v5/v6 required",
            )),
        }
    }

    fn has_layer_quality(self) -> bool {
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
    if cmd::canonical(&hint)?.len() > MAX_RECORD_BYTES {
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
    let owner = ProtectedAssessmentJournal::select_owner_local(
        &configuration_path,
        &context_path,
        owner_context.private_root(),
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(owner.configuration_raw().len())?;
    let config = owner.configuration();
    if owner.configuration_raw() != hint_raw.as_slice()
        || OwnerVersion::from_config(config)? != version
    {
        return Err(SourceCommandError::Conflict(
            "assessment owner configuration changed during preflight",
        ));
    }
    if cmd::canonical(config)?.len() > MAX_RECORD_BYTES {
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
    let private_sources = crate::source_private_assessment_sources::select_owner_local_sources(
        &owner_context,
        &context_configuration,
        cmd::field(config, "owner_local_source_records")?,
        cmd::field(config, "native_text_units")?,
        config.object_get("owner_local_source_claims"),
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
        invocation,
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

fn parse_request(raw: &[u8], version: OwnerVersion) -> SourceCommandResult<ParsedRequest> {
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
        OwnerVersion::V4 => matches!(
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
                if digest(text).is_err() {
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

fn preflight_configuration(config: &JsonValue, version: OwnerVersion) -> SourceCommandResult<()> {
    if cmd::array(config, "authorities")?.len() > MAX_ASSESSMENTS
        || cmd::array(config, "competencies")?.len() > MAX_ASSESSMENTS
        || cmd::array(config, "records")?.len() > MAX_ASSESSMENTS
        || cmd::array(config, "source_records")?.len() > MAX_ASSESSMENTS
        || cmd::array(config, "owner_local_source_records")?.len() > 64
        || cmd::array(config, "native_text_units")?.len() > 64
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
        "ToS/contracts/provenance-event-v2.schema.json",
        "ToS/contracts/source-anchor-v2.schema.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
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
    let source_inputs = selected
        .private_sources
        .records
        .iter()
        .cloned()
        .collect::<Vec<_>>();
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
    let mut source_records = Vec::new();
    let mut source_index = BTreeMap::<String, usize>::new();
    let mut resolved_rows = Vec::new();
    let mut resolved_index = BTreeMap::<String, usize>::new();
    for row in &public_sources.rows {
        budget.charge_work(1)?;
        push_resolved(row.clone(), &mut source_records, &mut source_index)?;
        push_resolved(row.clone(), &mut resolved_rows, &mut resolved_index)?;
    }
    for input in &selected.private_sources.records {
        budget.charge_work(1)?;
        let row = parse_record_input(input)?;
        push_resolved(row.clone(), &mut source_records, &mut source_index)?;
        push_resolved(row, &mut resolved_rows, &mut resolved_index)?;
    }
    let mut native_records = Vec::new();
    let mut native_index = BTreeMap::<String, usize>::new();
    for input in &selected.private_sources.native_records {
        budget.charge_work(1)?;
        let row = parse_record_input(input)?;
        push_resolved(row.clone(), &mut native_records, &mut native_index)?;
        push_resolved(row, &mut resolved_rows, &mut resolved_index)?;
    }
    let mut layers = if selected.version.has_layer_quality() {
        Some(
            crate::source_private_assessment_layers::select_owner_local_layers(
                &mut selected.owner_context,
                &selected.selected_context,
                &config,
                &source_records
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
        for input in layer_sources.records() {
            budget.charge_work(1)?;
            let row = parse_record_input(&input)?;
            let id = cmd::text(&row, "id")?.to_owned();
            if let Some(index) = native_index.get(&id).copied() {
                if !cmd::same(&native_records[index], &row)? {
                    return Err(SourceCommandError::Conflict(
                        "assessment layer identity shadows different native evidence",
                    ));
                }
            }
            push_resolved(row.clone(), &mut source_records, &mut source_index)?;
            push_resolved(row, &mut resolved_rows, &mut resolved_index)?;
        }
    }
    if source_records
        .len()
        .checked_add(native_records.len())
        .is_none_or(|count| count > MAX_ASSESSMENTS)
        || resolved_rows.len() > MAX_ASSESSMENTS
    {
        return Err(SourceCommandError::Invalid(
            "assessment combined owner/source record budget",
        ));
    }
    let resolved_bytes = cmd::canonical(&JsonValue::Array(resolved_rows.clone()))?.len();
    budget.charge_bytes(resolved_bytes)?;
    if resolved_bytes > MAX_RESOLVED_BYTES {
        return Err(SourceCommandError::Invalid(
            "assessment resolved source byte budget",
        ));
    }
    let base_snapshot = owner_snapshot(
        &config,
        selected.owner.configuration_raw(),
        &public_sources.fixity,
        &resolved_rows,
        &public_sources.identity_snapshots,
        &public_sources.claim_dependencies,
        &selected.private_sources.owner_local_sources_snapshot,
        &selected.private_sources.native_snapshots,
        layers.as_ref(),
    )?;
    if !selected.version.has_layer_quality()
        && selected.request.operation != "describe"
        && selected.request.expected_snapshot.as_deref() != Some(&base_snapshot)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment owner/source snapshot is stale",
        ));
    }
    let mut all_by_id = BTreeMap::<String, JsonValue>::new();
    for row in resolved_rows.iter().chain(cmd::array(&config, "records")?) {
        let id = cmd::text(row, "id")?.to_owned();
        if let Some(old) = all_by_id.get(&id) {
            if !cmd::same(old, row)? {
                return Err(SourceCommandError::Conflict(
                    "assessment owner/source identity has conflicting current bodies",
                ));
            }
        } else {
            all_by_id.insert(id, row.clone());
        }
    }
    let (current, scope) = select_current_scope(&config, &selected.request, &all_by_id)?;
    if selected.request.operation == "read-layer-comparison"
        && layers
            .as_ref()
            .and_then(|layers| layers.layers.get(&selected.request.subject_id))
            .is_none()
    {
        return Err(SourceCommandError::Denied(
            "assessment comparison reading requires an exact selected layer",
        ));
    }
    let selected_layer = layers
        .as_ref()
        .and_then(|layers| layers.layers.get(&selected.request.subject_id));
    let subject_is_native = native_index.contains_key(&selected.request.subject_id);
    let subject_is_sourced = source_index.contains_key(&selected.request.subject_id);
    if subject_is_native
        && selected_layer.is_none()
        && !cmd::array(&config, "native_text_units")?
            .iter()
            .any(|selection| {
                selection
                    .object_get("binding")
                    .and_then(|binding| binding.object_get("unit_id"))
                    .and_then(JsonValue::as_str)
                    == Some(selected.request.subject_id.as_str())
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
    let subject_required_source_refs = required_source_refs(
        &selected.request.subject_id,
        &public_sources.claim_dependencies,
        selected.private_sources.required_source_refs(),
        selected_layer,
    )?;
    let required_sources = resolve_required_sources(&subject_required_source_refs, &all_by_id)?;
    let source_route = if selected_layer.is_some() {
        AssessmentSourceRoute::LayerQuality
    } else if public_sources
        .claim_dependencies
        .contains_key(&selected.request.subject_id)
        || selected
            .private_sources
            .required_source_refs()
            .contains_key(&selected.request.subject_id)
    {
        AssessmentSourceRoute::SourceBoundClaim
    } else {
        AssessmentSourceRoute::SelectedSource
    };
    let mut current_snapshot = base_snapshot.clone();
    let mut subject_quality_requirements = Vec::new();
    if selected.version.has_layer_quality() {
        subject_quality_requirements = quality_requirements(
            &current,
            &required_sources,
            &all_by_id,
            &config,
            layers.as_ref().ok_or(SourceCommandError::Invalid(
                "selected native layer source snapshot absent",
            ))?,
            cmd::text(&scope, "assertion_layer")?,
            &mut budget,
            cancelled,
        )?;
    }
    let parent_context = if selected.version.has_layer_quality()
        && subject_is_sourced
        && cmd::text(&current, "id").is_ok()
        && cmd::text(cmd::field(&current, "payload")?, "schema_version")? == "tos_human_form_v1"
    {
        parent_claim_context(
            &current,
            &scope,
            &config,
            &all_by_id,
            &required_sources,
            &public_sources.claim_dependencies,
            selected.private_sources.required_source_refs(),
            selected.private_sources.required_languages(),
            &selected.private_sources.native_summaries,
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
            &config,
            layers.as_ref().ok_or(SourceCommandError::Invalid(
                "selected native layer source snapshot absent",
            ))?,
            cmd::text(&parent.scope, "assertion_layer")?,
            &mut budget,
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
    let mut lock_subjects = vec![selected.request.subject_id.clone()];
    let roots = subject_quality_requirements
        .iter()
        .chain(parent_quality_requirements.iter())
        .map(|requirement| requirement.layer_id.clone())
        .collect::<Vec<_>>();
    let quality_closure = collect_quality_lock_closure(
        &roots,
        selected.layer_preflight.as_ref(),
        &mut budget,
        deadline,
        cancelled,
    )?;
    lock_subjects.extend(quality_closure);
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
    let fence = selected
        .owner
        .lock_subjects(&lock_subjects, deadline, cancelled)?;
    let now = selected.context.recorded_at.clone();
    let mut quality_bases = Vec::new();
    let mut histories = BTreeMap::new();
    for subject_id in &lock_subjects {
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
    let owner_record_rows = record_rows(&config, &[])?;
    for requirement in &subject_quality_requirements {
        let layer = layers
            .as_ref()
            .and_then(|layers| layers.layers.get(&requirement.layer_id))
            .ok_or(SourceCommandError::Denied(
                "assessment quality dependency has no exact selected source layer",
            ))?;
        let dependency_scope = configured_scope(&config, &requirement.layer_id)?;
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
            &public_sources.claim_dependencies,
            selected.private_sources.required_source_refs(),
            Some(layer),
        )?;
        let report = evaluate_selected_subject(
            &config,
            &requirement.layer_id,
            &dependency_scope,
            &source_records,
            &native_records,
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
            &now,
            &mut selected.worker,
            &mut budget,
            cancelled,
        )?;
        let basis = quality_basis(
            layer,
            &report,
            &requirement.use_name,
            &mut selected.worker,
            deadline,
            cancelled,
        )?;
        if all_by_id.contains_key(cmd::text(&basis, "id")?) {
            return Err(SourceCommandError::Denied(
                "derived quality basis conflicts with an owner source record",
            ));
        }
        quality_bases.push((requirement.clone(), basis));
    }
    if selected.version.has_layer_quality() {
        current_snapshot = quality_snapshot(&base_snapshot, &quality_bases)?;
    }
    let subject_history =
        histories
            .get(&selected.request.subject_id)
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
        &config,
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
    if selected.request.operation != "describe"
        && parent_context.is_none()
        && selected.request.expected_snapshot.as_deref() != Some(&current_snapshot)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment current owner/source snapshot is stale",
        ));
    }
    let trusted_history = subject_history.submissions.clone();
    let reviews = if selected.request.operation == "append" {
        append_submissions(&config, &scope, &selected.request.assessments)?
    } else {
        Vec::new()
    };
    let current_report = evaluate_selected_subject(
        &config,
        &selected.request.subject_id,
        &scope,
        &source_records,
        &native_records,
        &record_rows,
        &subject_required_source_refs,
        &admission_bases
            .iter()
            .map(|(_, basis, _, _)| basis.clone())
            .collect::<Vec<_>>(),
        &trusted_history,
        &[],
        source_route,
        route_layer_quality,
        &now,
        &mut selected.worker,
        &mut budget,
        cancelled,
    )?;
    let append_report = if selected.request.operation == "append" {
        Some(evaluate_selected_subject(
            &config,
            &selected.request.subject_id,
            &scope,
            &source_records,
            &native_records,
            &record_rows,
            &subject_required_source_refs,
            &admission_bases
                .iter()
                .map(|(_, basis, _, _)| basis.clone())
                .collect::<Vec<_>>(),
            &trusted_history,
            &reviews,
            source_route,
            route_layer_quality,
            &now,
            &mut selected.worker,
            &mut budget,
            cancelled,
        )?)
    } else {
        None
    };
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
    let parent_assessment = if let Some(parent) = &parent_context {
        let id = cmd::text(&parent.record, "id")?;
        let parent_history = histories.get(id).ok_or(SourceCommandError::Invalid(
            "assessment parent Claim history absent",
        ))?;
        let parent_bases = quality_bases
            .iter()
            .filter(|(requirement, _)| parent_quality_requirements.contains(requirement))
            .map(|(_, basis)| record_input(basis))
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let parent_report = evaluate_selected_subject(
            &config,
            id,
            &parent.scope,
            &source_records,
            &native_records,
            &record_rows,
            &parent
                .required_sources
                .iter()
                .map(owner_reference)
                .collect::<SourceCommandResult<Vec<_>>>()?,
            &parent_bases,
            &parent_history.submissions,
            &[],
            AssessmentSourceRoute::SourceBoundClaim,
            None,
            &now,
            &mut selected.worker,
            &mut budget,
            cancelled,
        )?;
        let packet = parent_assessment_packet(
            &parent.record,
            &parent_report,
            parent_history,
            &parent.scope,
        )?;
        if selected.version.has_layer_quality() {
            current_snapshot = extend_subject_assessment_snapshot(&current_snapshot, &packet)?;
        }
        Some(packet)
    } else {
        None
    };
    if selected.version.has_layer_quality()
        && selected.request.operation != "describe"
        && selected.request.expected_snapshot.as_deref() != Some(&current_snapshot)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment current quality basis snapshot is stale",
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
struct QualityRequirement {
    layer_id: String,
    use_name: String,
}

struct ParentClaimContext {
    record: JsonValue,
    scope: JsonValue,
    required_sources: Vec<JsonValue>,
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    crate::source_creation_store::active(deadline, cancelled)
}

fn parse_record_input(input: &AssessmentRecordInput) -> SourceCommandResult<JsonValue> {
    cmd::parse(&input.envelope)
}

fn record_input(row: &JsonValue) -> SourceCommandResult<AssessmentRecordInput> {
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

fn append_submissions(
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

fn owner_reference(row: &JsonValue) -> SourceCommandResult<JsonValue> {
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
    if scope.object_get("form_language_context").is_some() {
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

fn select_current_scope(
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
    } else if cmd::text(body, "schema_version")? == "tos_human_form_v1"
        && (cmd::text(&scope, "assertion_layer")? != "human_projection"
            || cmd::field(&scope, "maker_id")? != cmd::field(body, "creator_id")?)
    {
        return Err(SourceCommandError::Denied(
            "assessment source form scope differs from its creator",
        ));
    }
    Ok((current, scope))
}

fn resolve_required_sources(
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

fn required_source_refs(
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

fn scope_context(scope: &JsonValue) -> SourceCommandResult<JsonValue> {
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
    let input = AssessmentReadInput {
        source_revision: worker.source_revision(),
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
    };
    active(budget.deadline, cancelled)?;
    let report = evaluate_current_assessment(&input, worker, evaluation_limits(budget)?, cancelled)
        .map_err(map_assessment_refusal)?;
    budget.charge_report(&report)?;
    Ok(report)
}

fn map_assessment_refusal(error: AssessmentRefusal) -> SourceCommandError {
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
        AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Unsupported(_))
        | AssessmentRefusal::Unsupported(_) => {
            SourceCommandError::Unsupported("private assessment evaluation")
        }
    }
}

fn owner_snapshot(
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
    let mut identity_rows = public_identity_snapshots.clone();
    identity_rows.insert(
        "native_text_snapshots".to_owned(),
        JsonValue::Array(
            private_native_snapshots
                .iter()
                .map(|value| cmd::string(value))
                .collect(),
        ),
    );
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
    entries.insert(
        "owner_local_sources".to_owned(),
        cmd::string(private_source_snapshot),
    );
    entries.insert(
        "configuration_bytes".to_owned(),
        cmd::string(&Digest256::of_bytes(configuration_raw).to_hex()),
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

fn quality_snapshot(
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

fn extend_subject_assessment_snapshot(
    source_snapshot: &str,
    subject_assessment: &JsonValue,
) -> SourceCommandResult<String> {
    let value = cmd::object(vec![
        ("source_snapshot", cmd::string(source_snapshot)),
        ("subject_assessment", subject_assessment.clone()),
    ]);
    Ok(format!("sha256:{}", cmd::record_digest(&value)?.to_hex()))
}

fn record_rows(config: &JsonValue, additions: &[JsonValue]) -> SourceCommandResult<Vec<JsonValue>> {
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

fn quality_requirements(
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

fn collect_quality_lock_closure(
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

fn quality_basis(
    layer: &crate::source_private_assessment_layers::SelectedOwnerLayer,
    report: &AssessmentMechanicsReport,
    use_name: &str,
    worker: &mut CutWorkerSchemaExecutor,
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
    let scope = layer.scope.clone();
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

fn report_admission(report: &AssessmentMechanicsReport) -> SourceCommandResult<JsonValue> {
    let raw = serde_json::to_vec(report.current_admission())
        .map_err(|_| SourceCommandError::Invalid("assessment admission serialization"))?;
    cmd::parse(&raw)
}

fn basis_observation(basis: &JsonValue) -> SourceCommandResult<(bool, Vec<JsonValue>)> {
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

fn native_append_ready(
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

fn parent_claim_context(
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

fn parent_assessment_packet(
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
    let envelope = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_assessment_result_v1"),
        ),
        ("owner_snapshot", cmd::string(snapshot)),
        ("authentication", cmd::string("local-unix-account")),
        ("result", result),
        ("visibility", cmd::string("local_only")),
        ("publication_authorized", JsonValue::Bool(false)),
    ]);
    Ok(envelope)
}

fn current_admission_with_limits(
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
        ("scope", scope_context(scope)?),
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
    Ok(context)
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
    let command_id = selected
        .request
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
        selected
            .request
            .expected_revision
            .as_ref()
            .ok_or(SourceCommandError::Invalid(
                "assessment expected revision absent",
            ))?;
    let submitted = append_event_rows(
        &selected.request.assessments,
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
    let request_digest = cmd::record_digest(&request)?.to_hex();
    for batch in &history.batches {
        if cmd::text(cmd::field(batch, "request")?, "command_id")? == command_id {
            if cmd::text(batch, "request_digest")? != request_digest {
                return Err(SourceCommandError::Conflict(
                    "assessment command ID is bound to another request",
                ));
            }
            crate::source_sign::finish_worker(worker, deadline, cancelled)?;
            return Ok((
                cmd::object(vec![
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
                        current_admission_with_limits(
                            current_report,
                            layers,
                            &selected.request.subject_id,
                        )?,
                    ),
                ]),
                false,
            ));
        }
    }
    if history.head.as_ref() != expected_revision.as_ref() {
        return Err(SourceCommandError::Conflict(
            "assessment expected journal revision is stale",
        ));
    }
    if let Some(previous) = history.batches.last() {
        if tos_validation::retirement_rules::observed_instant_order(
            &selected.context.recorded_at,
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
        ("subject_id", cmd::string(&selected.request.subject_id)),
        ("sequence", cmd::number(history.batches.len() as u64 + 1)),
        ("previous_revision", previous_revision),
        ("recorded_at", cmd::string(&selected.context.recorded_at)),
        ("request", request),
        ("request_digest", cmd::string(&request_digest)),
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
    if cmd::canonical(&batch)?.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid("assessment batch byte budget"));
    }
    if !worker
        .check(
            "assessment/native-journal-batch",
            &cmd::canonical(&batch)?,
            BATCH_SCHEMA,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Invalid("assessment batch schema execution"))?
    {
        return Err(SourceCommandError::Invalid("assessment batch schema"));
    }
    crate::source_sign::finish_worker(worker, deadline, cancelled)?;
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
    let revision = fence.publish(
        &selected.request.subject_id,
        history,
        expected_revision.as_deref(),
        &batch,
        || currentness(),
        deadline,
        cancelled,
    )?;
    Ok((
        cmd::object(vec![
            ("revision", cmd::string(&revision)),
            ("receipt", batch),
            ("replayed", JsonValue::Bool(false)),
            (
                "current_admission",
                current_admission_with_limits(append_report, layers, &selected.request.subject_id)?,
            ),
        ]),
        true,
    ))
}

fn append_event_rows(
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
    let (form_set_path, form_set, _public_form) =
        if let Some(path) = selected.private_sources.form_paths().get(form_id) {
            let form_set = selected.private_sources.form_sets().get(path).ok_or(
                SourceCommandError::Conflict(
                    "materialization form set is absent from the current selected source closure",
                ),
            )?;
            (path.clone(), form_set, false)
        } else if let Some(path) = public_sources.form_paths.get(form_id).cloned() {
            if public_sources.record_paths.get(form_id) != Some(&path) {
                return Err(SourceCommandError::Denied(
                    "materialization public form path is not exact-selected",
                ));
            }
            public_sources.validate_form_set(&path, worker, deadline, cancelled)?;
            let form_set =
                public_sources
                    .form_sets
                    .get(&path)
                    .ok_or(SourceCommandError::Conflict(
                        "materialization public form set is absent from current selected sources",
                    ))?;
            (path, form_set, true)
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
    let subject_path = selected
        .private_sources
        .source_paths
        .get(subject_id)
        .or_else(|| public_sources.record_paths.get(subject_id));
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
        || !cmd::same(cmd::field(form_set, "subject")?, subject_ref)?
    {
        return Err(SourceCommandError::Conflict(
            "materialization form binds another current source subject",
        ));
    }
    let forms = cmd::array(form_set, "forms")?;
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
    let mut wording = String::new();
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
        let fields = crate::source_forms::metadata_fields(cmd::field(subject, "payload")?)?;
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
    let admission =
        current_admission_with_limits(current_report, layers, &selected.request.subject_id)?;
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
