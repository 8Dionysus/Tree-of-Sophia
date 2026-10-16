//! Maintained private Assessment operations after exact PG owner selection.
//!
//! Source and history transport stays in the caller. This module reuses the
//! same planning, evaluation, form-materialization, append/replay and result
//! semantics as the filesystem route; the PG caller owns readset and package
//! currentness around these operations.

use crate::durable_private_packages::{
    PrivateOwnerPackageFamilyV1, PrivateOwnerPackageHistoryV1, PrivateOwnerPackageSnapshotV1,
    PrivateOwnerPackageWriteV1, PrivateOwnerScopeV1, PrivateOwnerSourceBindingV1,
    private_owner_dependency_digest, private_owner_readset_digest,
};
use crate::source_assessment_journal::{
    AssessmentHistory, ProtectedAssessmentPgGrant,
    commit_private_assessment_batch_with_expected_heads_and_currentness,
    with_private_assessment_and_source_histories_guard_selected_schema,
};
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, SourceRevisionReadSession, SourceRevisionReadSet,
};
use crate::source_command::{
    self as cmd, CommandContext, SourceCommandError, SourceCommandResult, SourceFile,
};
use crate::source_native_cli::PgGrowthCurrentInput;
use crate::source_native_cli::pg_claims as pg;
use crate::source_native_cli::private_assessment as assessment;
use crate::source_native_cli::private_assessment::AssessmentAppendPreparation;
use crate::source_private_assessment_layers::PrivateAssessmentLayers;
use crate::source_revisions::ReadonlyRecordFiles;
use crate::source_sign::pg::PgOwnerAssessmentPublicSources;
use crate::source_sign_native::SignNativeRead;
use crate::source_text_owner::{OwnerTextContext, read_absolute};
use crate::{DurableError, DurablePgCoordinator, DurableResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_segment_store::{SegmentLimits, SegmentStore};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::assessment::{
    AssessmentMechanicsReport, AssessmentReadInput, AssessmentRecordInput,
    evaluate_current_assessment_selected,
};
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget, VerifiedWorkerImageHandle};
use tos_validation::record_biblio_cut::SourceCutInputWithIdentity;
use tos_validation::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutSchemaExecutor, CutWorkerLimits,
};

const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const ENTITY_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION_REGISTRY: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const MAX_SCHEMA_FILES: usize = 128;
const MAX_SCHEMA_BYTES: u64 = 33_554_432;
const MAX_SCHEMA_MEMBER_BYTES: u64 = 1_048_576;
const MAX_REQUEST: usize = 1_048_576;
const MAX_PG_ASSESSMENT_MEMBERS: u64 = 4_096;
const MAX_PG_ASSESSMENT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PG_ASSESSMENT_MEMBER_BYTES: u64 = 32 * 1024 * 1024;

/// Output from one already-selected PG Assessment operation. A new append is
/// only prepared here; the caller must release shared history guards and
/// publish it through the private package broker with expected-head CAS.
pub(crate) enum PgAssessmentOperation {
    Read(JsonValue),
    Append(AssessmentAppendPreparation),
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_selected_operation<W: CutSchemaExecutor>(
    request: &assessment::ParsedRequest,
    version: assessment::OwnerVersion,
    config: &JsonValue,
    context: &CommandContext,
    plan: &assessment::AssessmentEvaluationPlan<'_>,
    history: &AssessmentHistory,
    evaluation: &assessment::AssessmentEvaluationResult,
    private_records: &[AssessmentRecordInput],
    private_native_summaries: &[JsonValue],
    private_form_paths: &BTreeMap<String, String>,
    private_form_sets: &BTreeMap<String, JsonValue>,
    private_source_paths: &BTreeMap<String, String>,
    public_sources: &mut PgOwnerAssessmentPublicSources,
    layers: Option<&PrivateAssessmentLayers>,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgAssessmentOperation> {
    let current = &plan.current;
    let scope = &plan.scope;
    let subject_id = request.subject_id.as_str();
    let current_report = &evaluation.current_report;
    let admission_bases = &evaluation.admission_bases;
    let required_sources = &plan.required_sources;
    let all_by_id = &plan.all_by_id;
    let history_revision = history
        .head
        .as_deref()
        .map(cmd::string)
        .unwrap_or(JsonValue::Null);
    let current_admission =
        || assessment::current_admission_with_limits(current_report, layers, subject_id);

    let result =
        match request.operation.as_str() {
            "describe" | "inspect" | "read-layer-comparison" => {
                let mut result = cmd::object(vec![
                    ("revision", history_revision),
                    ("batch_count", cmd::number(history.batches.len() as u64)),
                    ("current_admission", current_admission()?),
                ]);
                if request.operation == "read-layer-comparison" {
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
                    let comparison = assessment::parse_record_input(comparison)?;
                    cmd::set(
                        &mut result,
                        "source_comparison",
                        cmd::object(vec![
                            ("record", assessment::owner_reference(&comparison)?),
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
                if request.operation == "describe" {
                    // The shared append parser requires this exact snapshot
                    // in `expected_snapshot`; the filesystem route returns it
                    // in its envelope, so expose the PG evaluation result too.
                    cmd::set(
                        &mut result,
                        "owner_snapshot",
                        cmd::string(&evaluation.current_snapshot),
                    )?;
                    cmd::set(
                        &mut result,
                        "command_context",
                        describe_context(
                            version,
                            config,
                            scope,
                            current,
                            current_report,
                            admission_bases,
                            evaluation.parent_assessment.as_ref(),
                            required_sources,
                            evaluation.source_read_ready,
                            plan.private_required_source_refs,
                            private_records,
                            private_native_summaries,
                            private_form_paths,
                            public_sources,
                            layers,
                        )?,
                    )?;
                }
                crate::source_sign::finish_worker(worker, deadline, cancelled)?;
                PgAssessmentOperation::Read(result)
            }
            "materialize-form" => {
                if !current_report.source_read_ready() {
                    return Err(SourceCommandError::Denied(
                        "native-bound form materialization requires exact current source read",
                    ));
                }
                let private_maps = assessment::AssessmentFormMaps {
                    form_paths: private_form_paths,
                    form_sets: private_form_sets,
                    source_paths: private_source_paths,
                };
                let public_form_paths = public_sources.form_paths.clone();
                let public_form_sets = public_sources.form_sets.clone();
                let public_source_paths = public_sources.record_paths.clone();
                let public_maps = assessment::AssessmentFormMaps {
                    form_paths: &public_form_paths,
                    form_sets: &public_form_sets,
                    source_paths: &public_source_paths,
                };
                let materialization = assessment::materialize_assessed_form_from_selected_maps(
                    Some(&private_maps),
                    &public_maps,
                    true,
                    subject_id,
                    worker,
                    current,
                    scope,
                    history,
                    current_report,
                    admission_bases,
                    evaluation.parent_assessment.as_ref(),
                    all_by_id,
                    required_sources,
                    layers,
                    |path, worker, deadline, cancelled| {
                        public_sources.validate_form_set(path, worker, deadline, cancelled)
                    },
                    deadline,
                    cancelled,
                )?;
                crate::source_sign::finish_worker(worker, deadline, cancelled)?;
                PgAssessmentOperation::Read(cmd::object(vec![
                    ("revision", history_revision),
                    ("batch_count", cmd::number(history.batches.len() as u64)),
                    ("current_admission", current_admission()?),
                    ("materialization", materialization),
                ]))
            }
            "append" => {
                if !current_report.source_read_ready() {
                    return Err(SourceCommandError::Denied(
                        "native-bound assessment append requires exact current source read",
                    ));
                }
                if !assessment::native_append_ready(
                    layers
                        .and_then(|layers| layers.layers.get(subject_id))
                        .is_some(),
                    plan.public_claim_dependencies.contains_key(subject_id)
                        || plan.private_required_source_refs.contains_key(subject_id),
                    required_sources,
                    private_native_summaries,
                )? {
                    return Err(SourceCommandError::Denied(
                        "native assessment append requires every selected current source read",
                    ));
                }
                let append_report =
                    evaluation
                        .append_report
                        .as_ref()
                        .ok_or(SourceCommandError::Invalid(
                            "assessment append evaluation absent",
                        ))?;
                let prepared = assessment::prepare_assessment_append(
                    request,
                    &context.recorded_at,
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
                PgAssessmentOperation::Append(prepared)
            }
            _ => {
                return Err(SourceCommandError::Unsupported(
                    "private assessment operation",
                ));
            }
        };

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn describe_context(
    version: assessment::OwnerVersion,
    config: &JsonValue,
    scope: &JsonValue,
    current: &JsonValue,
    current_report: &AssessmentMechanicsReport,
    admission_bases: &[(
        assessment::QualityRequirement,
        AssessmentRecordInput,
        bool,
        Vec<JsonValue>,
    )],
    parent_assessment: Option<&JsonValue>,
    required_sources: &[JsonValue],
    source_read_ready: bool,
    private_required_source_refs: &BTreeMap<String, Vec<JsonValue>>,
    private_records: &[AssessmentRecordInput],
    private_native_summaries: &[JsonValue],
    private_form_paths: &BTreeMap<String, String>,
    public_sources: &PgOwnerAssessmentPublicSources,
    layers: Option<&PrivateAssessmentLayers>,
) -> SourceCommandResult<JsonValue> {
    let current_body = cmd::field(current, "payload")?;
    let current_id = cmd::text(current, "id")?;
    let layer = layers.and_then(|layers| layers.layers.get(current_id));
    let is_source_form = (private_form_paths.contains_key(current_id)
        || public_sources.form_paths.contains_key(current_id))
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
    let claim_source_bound = private_required_source_refs.contains_key(current_id)
        || public_sources.claim_dependencies.contains_key(current_id);
    let required_ids = required_sources
        .iter()
        .map(|row| cmd::text(row, "id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let summaries_ready = private_native_summaries.iter().all(|summary| {
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
    let mut supported = vec!["describe", "inspect"];
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
        ("subject", assessment::owner_reference(current)?),
        (
            "policy",
            assessment::owner_reference(&cmd::field(config, "policy")?.clone())?,
        ),
        (
            "scope",
            if version.is_private() {
                assessment::scope_context(scope)?
            } else {
                scope.clone()
            },
        ),
        (
            "supported_operations",
            JsonValue::Array(supported.into_iter().map(cmd::string).collect()),
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
                    .map(assessment::owner_reference)
                    .collect::<SourceCommandResult<Vec<_>>>()?,
            ),
        )?;
    }
    if version.has_layer_quality() {
        let rows = admission_bases
            .iter()
            .map(|(_, basis, can_use, limits)| {
                let row = assessment::parse_record_input(basis)?;
                let body = cmd::field(&row, "payload")?;
                Ok(cmd::object(vec![
                    ("basis", assessment::owner_reference(&row)?),
                    ("can_use", JsonValue::Bool(*can_use)),
                    ("limits", JsonValue::Array(limits.clone())),
                    ("layer", cmd::field(body, "layer")?.clone()),
                    ("use", cmd::field(body, "use")?.clone()),
                ]))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        cmd::set(&mut context, "required_admissions", JsonValue::Array(rows))?;
        if let Some(layers) = layers {
            cmd::set(
                &mut context,
                "layer_comparison_contracts",
                JsonValue::Array(
                    layers
                        .profile_contract_digests
                        .iter()
                        .map(|(path, digest)| {
                            cmd::object(vec![
                                ("path", cmd::string(path)),
                                ("digest", cmd::string(digest)),
                            ])
                        })
                        .collect(),
                ),
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
                        .map(assessment::parse_record_input)
                        .transpose()?
                        .map(|row| assessment::owner_reference(&row))
                        .transpose()?
                        .unwrap_or(JsonValue::Null),
                ),
            ]),
        )?;
    }
    if current_report.source_read_required() || version.has_layer_quality() {
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
    let private_rows = private_records
        .iter()
        .map(|input| {
            let row = assessment::parse_record_input(input)?;
            Ok(cmd::object(vec![
                ("record", assessment::owner_reference(&row)?),
                (
                    "origin_id",
                    row.object_get("origin_id")
                        .cloned()
                        .unwrap_or(JsonValue::Null),
                ),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if !private_rows.is_empty() {
        cmd::set(
            &mut context,
            "owner_local_source_records",
            JsonValue::Array(private_rows),
        )?;
    }
    if !private_native_summaries.is_empty() {
        cmd::set(
            &mut context,
            "native_text_units",
            JsonValue::Array(private_native_summaries.to_vec()),
        )?;
    }
    Ok(context)
}

const INVOCATION_SCHEMA: &str = "tos_local_pg_private_assessment_invocation_v1";
const BATCH_SCHEMA: &str = "ToS/contracts/knowledge-assessment-batch.schema.json";

fn pg_read_member(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("private assessment PG source path"))?;
    session
        .read_current_member(&path, max_bytes as u64, deadline, cancelled)
        .map(|member| member.raw)
        .map_err(pg::source_error)
}

fn source_files(files: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<Vec<SourceFile>> {
    files
        .iter()
        .map(|(path, raw)| {
            Ok(SourceFile {
                path: RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("private assessment selected path"))?,
                raw: raw.clone(),
            })
        })
        .collect()
}

fn command_context(
    identity: PgRecordRevisionIdentity,
    config_raw: &[u8],
    request_raw: &[u8],
    uid: u32,
    files: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<CommandContext> {
    let context = CommandContext {
        base_revision: identity.bootstrap_revision,
        configuration_raw: config_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(uid),
        files: source_files(files)?,
    };
    context.check()?;
    Ok(context)
}

fn insert_dependency(
    members: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    raw: Vec<u8>,
) -> SourceCommandResult<()> {
    if members
        .insert(path, raw.clone())
        .is_some_and(|previous| previous != raw)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment dependency bytes differ",
        ));
    }
    Ok(())
}

fn selected_component_members(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let mut members = BTreeMap::new();
    for member in components.members() {
        let raw = software
            .read_selected_component(
                components,
                &member.path,
                max_member_bytes,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("private Assessment software changed"))?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "private Assessment software member identity changed",
            ));
        }
        insert_dependency(&mut members, member.path.as_str().to_owned(), raw)?;
    }
    Ok(members)
}

fn package_head(history: &PrivateOwnerPackageHistoryV1) -> SourceCommandResult<Option<String>> {
    let Some(current) = &history.current else {
        if history.family.is_some() || !history.history.is_empty() {
            return Err(SourceCommandError::Conflict(
                "private Assessment package has history without a current head",
            ));
        }
        return Ok(None);
    };
    let head = current
        .members
        .get("head")
        .ok_or(SourceCommandError::Conflict(
            "private Assessment package head member absent",
        ))?;
    let head = std::str::from_utf8(head)
        .map_err(|_| SourceCommandError::Invalid("private Assessment package head UTF-8"))?;
    if head.is_empty() || head.len() > 65 {
        return Err(SourceCommandError::Invalid(
            "private Assessment package head length",
        ));
    }
    Ok(Some(head.to_owned()))
}

fn source_binding(
    readset: &SourceRevisionReadSet,
    dependencies: &BTreeMap<String, Vec<u8>>,
    private_owner_audit_generation: u64,
) -> SourceCommandResult<PrivateOwnerSourceBindingV1> {
    let selected = readset.selection();
    let reads = dependencies
        .iter()
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect::<Vec<_>>();
    Ok(PrivateOwnerSourceBindingV1 {
        head_seq: selected.head_seq,
        source_epoch: selected.epoch,
        source_generation: selected.source_generation,
        source_revision: selected.cohort.initial_revision().0,
        source_audit_generation: selected.audit_generation,
        rights_version: selected.rights_version,
        rule_version: selected.rule_version,
        contract_digest: selected.contract,
        job_id: selected.job_id.clone(),
        job_fence: selected.job_fence,
        readset_digest: private_owner_readset_digest(readset).map_err(pg::source_error)?,
        dependency_digest: private_owner_dependency_digest(&reads).map_err(pg::source_error)?,
        private_owner_audit_generation,
    })
}

struct DeferredFinish<'a, W>(&'a mut W);

impl<W: CutSchemaExecutor> CutSchemaExecutor for DeferredFinish<'_, W> {
    fn selected_contract_digest(&self, contract: &str) -> Option<Digest256> {
        self.0.selected_contract_digest(contract)
    }

    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, tos_validation::item_rules::ItemRefusal> {
        self.0.check(path, raw, contract, deadline, cancelled)
    }

    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, tos_validation::item_rules::ItemRefusal> {
        self.0
            .check_reusing_scalar(path, raw, contract, deadline, cancelled)
    }

    fn finish(
        &mut self,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<(), tos_validation::item_rules::ItemRefusal> {
        Ok(())
    }
}

struct PgAssessmentAttempt {
    operation: PgAssessmentOperation,
    owner: OwnerTextContext,
    grant: ProtectedAssessmentPgGrant,
    config_raw: Vec<u8>,
    target_ref: String,
    expected_current: Option<PrivateOwnerPackageSnapshotV1>,
    dependencies: BTreeMap<String, Vec<u8>>,
    package_histories: BTreeMap<String, PrivateOwnerPackageHistoryV1>,
    source_binding: PrivateOwnerSourceBindingV1,
}

struct AssessmentPreflight {
    identity: PgRecordRevisionIdentity,
    worker: CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    schema_paths: Vec<String>,
    schema_members: BTreeMap<String, Vec<u8>>,
    lock_subjects: Vec<String>,
    subject_targets: Vec<(String, String)>,
    expected_heads: BTreeMap<String, Option<String>>,
}

/// Native PG counterpart for the protected v5/v6 Assessment owner. The
/// maintained parser, planner, evaluator, and operation preparation remain
/// shared with the filesystem journal; only selection and publication use
/// the selected PG epoch and private-package broker.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    super::exact(
        invocation,
        &[
            "schema_version",
            "owner_context",
            "owner_config",
            "native_executable",
            "native_executable_sha256",
            "source_selection",
            "software_capture",
            "software_restored_root",
            "software_selection",
            "software_components",
            "schema_worker",
            "budgets",
        ],
    )?;
    if super::text(invocation, "schema_version")? != INVOCATION_SCHEMA {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private Assessment invocation profile",
        ));
    }
    let selection = pg::selected_source(invocation)?;
    let budgets = invocation
        .get("budgets")
        .ok_or(SourceCommandError::Invalid(
            "PostgreSQL private Assessment budgets",
        ))?;
    super::exact(
        budgets,
        &[
            "max_members",
            "max_total_bytes",
            "max_member_bytes",
            "max_schema_receipts",
            "max_schema_receipt_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
        ],
    )?;
    let max_members = usize::try_from(super::capped(
        budgets,
        "max_members",
        MAX_PG_ASSESSMENT_MEMBERS,
    )?)
    .map_err(|_| SourceCommandError::Invalid("private Assessment member budget"))?;
    let max_total_bytes = super::capped(budgets, "max_total_bytes", MAX_PG_ASSESSMENT_BYTES)?;
    let max_member_bytes =
        super::capped(budgets, "max_member_bytes", MAX_PG_ASSESSMENT_MEMBER_BYTES)?;
    if max_members == 0 || max_total_bytes == 0 || max_member_bytes == 0 {
        return Err(SourceCommandError::Invalid(
            "private Assessment empty budget",
        ));
    }
    let _ = super::capped(budgets, "max_schema_receipts", 128)?;
    let _ = super::capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let executable_path = super::absolute(super::text(invocation, "native_executable")?)?;
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let executable_digest = super::digest(super::text(invocation, "native_executable_sha256")?)?;
    if runtime.current_digest(deadline, cancelled)? != executable_digest {
        return Err(SourceCommandError::Conflict(
            "private Assessment executable identity",
        ));
    }
    let (software, components) = pg::software_selection(invocation, deadline, cancelled)?;
    let component_members = components.members().collect::<Vec<_>>();
    if component_members.len() > max_members
        || component_members
            .iter()
            .any(|member| member.size_bytes > max_member_bytes)
        || component_members
            .iter()
            .try_fold(0u64, |n, member| n.checked_add(member.size_bytes))
            .is_none_or(|n| n > max_total_bytes)
    {
        return Err(SourceCommandError::Invalid(
            "private Assessment software budget",
        ));
    }
    let request_raw = pg::read_request(input)?;
    let context_path = super::absolute(super::text(invocation, "owner_context")?)?;
    let grant_path = super::absolute(super::text(invocation, "owner_config")?)?;
    if ProtectedAssessmentPgGrant::source_context_ref(&grant_path, deadline, cancelled)?
        != context_path
    {
        return Err(SourceCommandError::Denied(
            "private Assessment invocation context differs",
        ));
    }
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected private Assessment source store"))?;
    let mut source_db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected private Assessment PostgreSQL"))?,
    );
    let mut history_db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("private Assessment package PostgreSQL"))?,
    );
    let mut commit_db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("private Assessment commit PostgreSQL"))?,
    );

    let (
        (
            initial_identity,
            mut owner,
            selected_context,
            grant,
            scope,
            config,
            config_raw,
            entity_raw,
            relation_raw,
            source_targets,
        ),
        _,
    ) = source_db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let identity = session.selection().identity();
                if identity.contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "private Assessment source contract differs",
                    ));
                }
                let context_schema =
                    pg_read_member(session, CONTEXT_SCHEMA, 1_048_576, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                let mut context_worker = assessment::selected_assessment_candidate_worker(
                    invocation,
                    session,
                    identity,
                    &[CONTEXT_SCHEMA.to_owned()],
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let (mut owner, selected_context) = OwnerTextContext::select_pg(
                    &context_path,
                    &context_schema,
                    &mut context_worker,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let grant = ProtectedAssessmentPgGrant::select(
                    &grant_path,
                    &context_path,
                    &owner,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let config = grant.configuration().clone();
                let config_raw = grant.configuration_raw().to_vec();
                let version =
                    assessment::OwnerVersion::from_config(&config).map_err(DurableError::Source)?;
                if !matches!(
                    version,
                    assessment::OwnerVersion::V5 | assessment::OwnerVersion::V6
                ) {
                    return Err(DurableError::Refused(
                        "private PG Assessment requires owner v5/v6",
                    ));
                }
                let request = assessment::parse_request(&request_raw, version)
                    .map_err(DurableError::Source)?;
                assessment::preflight_configuration(&config, version)
                    .map_err(DurableError::Source)?;
                let binding = owner
                    .private_owner_scope_binding(deadline, cancelled)
                    .map_err(DurableError::Source)?;
                let scope = PrivateOwnerScopeV1::for_context(&selection.domain, binding)
                    .map_err(|_| DurableError::Invalid("private Assessment scope"))?;
                let source_targets =
                    crate::source_private_assessment_pg_sources::private_source_target_refs(
                        &owner, &config,
                    )
                    .map_err(DurableError::Source)?;
                let entity_raw =
                    pg_read_member(session, ENTITY_REGISTRY, 4_194_304, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                let relation_raw =
                    pg_read_member(session, RELATION_REGISTRY, 4_194_304, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                owner
                    .install_pg_selected_members(BTreeMap::from([(
                        CONTEXT_SCHEMA.to_owned(),
                        context_schema,
                    )]))
                    .map_err(DurableError::Source)?;
                owner
                    .verify_current(deadline, cancelled)
                    .map_err(DurableError::Source)?;
                grant
                    .verify_current(deadline, cancelled)
                    .map_err(DurableError::Source)?;
                crate::source_sign::finish_worker(&mut context_worker, deadline, cancelled)
                    .map_err(DurableError::Source)?;
                Ok((
                    identity,
                    owner,
                    selected_context,
                    grant,
                    scope,
                    config,
                    config_raw,
                    entity_raw,
                    relation_raw,
                    source_targets,
                ))
            },
        )
        .map_err(pg::source_error)?;
    let version = assessment::OwnerVersion::from_config(&config)?;
    let request = assessment::parse_request(&request_raw, version)?;
    if !matches!(
        version,
        assessment::OwnerVersion::V5 | assessment::OwnerVersion::V6
    ) {
        return Err(SourceCommandError::Denied(
            "private PostgreSQL Assessment owner selection",
        ));
    }
    if !matches!(
        request.operation.as_str(),
        "describe" | "inspect" | "append" | "materialize-form" | "read-layer-comparison"
    ) {
        return Err(SourceCommandError::Unsupported(
            "private PostgreSQL Assessment operation",
        ));
    }
    owner.snapshot(deadline, cancelled)?;
    grant.verify_current(deadline, cancelled)?;
    if read_absolute(
        invocation_path,
        uid,
        true,
        invocation_raw.len(),
        deadline,
        cancelled,
    )? != invocation_raw
        || runtime.current_digest(deadline, cancelled)? != executable_digest
    {
        return Err(SourceCommandError::Conflict(
            "private Assessment invocation or executable changed",
        ));
    }

    let software_members = selected_component_members(
        &software,
        &components,
        max_member_bytes,
        deadline,
        cancelled,
    )?;
    let initial_source_packages = if source_targets.is_empty() {
        BTreeMap::new()
    } else {
        history_db
            .with_private_owner_package_histories_guard(
                &scope,
                &source_targets,
                crate::durable_private_packages::MAX_PACKAGE_HISTORY_ROWS,
                crate::durable_private_packages::MAX_PACKAGE_HISTORY_BYTES,
                deadline,
                cancelled,
                |histories, _, _| Ok(histories.clone()),
            )
            .map_err(pg::source_error)?
    };
    let mut schema_paths =
        crate::source_private_assessment_pg_sources::private_source_schema_paths(
            &owner,
            &config,
            &initial_source_packages,
            &entity_raw,
            &relation_raw,
        )?
        .into_iter()
        .collect::<BTreeSet<_>>();
    schema_paths.insert(CONTEXT_SCHEMA.to_owned());
    schema_paths.insert(BATCH_SCHEMA.to_owned());
    for path in crate::source_private_assessment_layers::selected_pg_contract_paths() {
        schema_paths.insert((*path).to_owned());
    }
    for path in software_members.keys() {
        if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
            schema_paths.insert(path.clone());
        }
    }
    if schema_paths.len() > MAX_SCHEMA_FILES {
        return Err(SourceCommandError::Invalid(
            "private Assessment schema path budget",
        ));
    }
    let schema_paths = schema_paths.into_iter().collect::<Vec<_>>();
    let layer_preflight = Some(
        crate::source_private_assessment_layers::preflight_owner_local_layers(
            &config,
            cmd::field(&config, "subjects")?,
            cmd::text(&config, "schema_version")?,
        )?,
    );

    let ((mut preflight_worker, schema_members, lock_subjects, subject_targets), _) = source_db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let identity = session.selection().identity();
                if identity != initial_identity || identity.contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "private Assessment source identity changed",
                    ));
                }
                let mut schemas = BTreeMap::new();
                let mut schema_total = 0u64;
                for path in &schema_paths {
                    let raw = pg_read_member(
                        session,
                        path,
                        MAX_SCHEMA_MEMBER_BYTES as usize,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    schema_total = schema_total
                        .checked_add(raw.len() as u64)
                        .filter(|total| *total <= MAX_SCHEMA_BYTES)
                        .ok_or(DurableError::Invalid(
                            "private Assessment selected schema byte budget",
                        ))?;
                    schemas.insert(path.clone(), raw);
                }
                let current_entity =
                    pg_read_member(session, ENTITY_REGISTRY, 4_194_304, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                let current_relation =
                    pg_read_member(session, RELATION_REGISTRY, 4_194_304, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                if current_entity != entity_raw || current_relation != relation_raw {
                    return Err(DurableError::Conflict(
                        "private Assessment semantic registries changed",
                    ));
                }
                let mut authored = schemas.clone();
                insert_dependency(&mut authored, ENTITY_REGISTRY.to_owned(), current_entity)
                    .map_err(DurableError::Source)?;
                insert_dependency(
                    &mut authored,
                    RELATION_REGISTRY.to_owned(),
                    current_relation,
                )
                .map_err(DurableError::Source)?;
                for (path, raw) in &software_members {
                    if !path.starts_with("ToS/") {
                        continue;
                    }
                    if path.starts_with("ToS/source-witnesses/owner-local/") {
                        return Err(DurableError::Refused(
                            "private Assessment software contains owner-local material",
                        ));
                    }
                    let selected = pg_read_member(
                        session,
                        path,
                        usize::try_from(max_member_bytes)
                            .unwrap_or(MAX_PG_ASSESSMENT_MEMBER_BYTES as usize),
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    if &selected != raw {
                        return Err(DurableError::Conflict(
                            "private Assessment selected software differs from PG source",
                        ));
                    }
                    insert_dependency(&mut authored, path.clone(), selected)
                        .map_err(DurableError::Source)?;
                }
                let context_schema = schemas.get(CONTEXT_SCHEMA).ok_or(DurableError::Conflict(
                    "private Assessment context schema absent",
                ))?;
                let identity = session.selection().identity();
                let mut worker = assessment::selected_assessment_candidate_worker(
                    invocation,
                    session,
                    identity,
                    &schema_paths,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let (mut owner, selected_context) = OwnerTextContext::select_pg(
                    &context_path,
                    context_schema,
                    &mut worker,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                owner
                    .install_pg_selected_members(authored.clone())
                    .map_err(DurableError::Source)?;
                let mut command_files = authored;
                for (path, raw) in &software_members {
                    insert_dependency(&mut command_files, path.clone(), raw.clone())
                        .map_err(DurableError::Source)?;
                }
                let context =
                    command_context(identity, &config_raw, &request_raw, uid, &command_files)
                        .map_err(DurableError::Source)?;
                let mut input = PgGrowthCurrentInput::new(
                    session,
                    max_members,
                    usize::try_from(max_member_bytes)
                        .unwrap_or(MAX_PG_ASSESSMENT_MEMBER_BYTES as usize),
                )
                .map_err(|_| {
                    DurableError::Source(SourceCommandError::Invalid(
                        "private Assessment PG input budget",
                    ))
                })?;
                let private_sources =
                    crate::source_private_assessment_pg_sources::select_owner_local_sources_pg(
                        &mut owner,
                        &selected_context,
                        &config,
                        &context,
                        &input,
                        &initial_source_packages,
                        &mut worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                if private_sources.target_refs != source_targets {
                    return Err(DurableError::Conflict(
                        "private Assessment source package target set changed",
                    ));
                }
                let mut public_sources =
                    crate::source_sign::pg::select_owner_assessment_public_sources_pg(
                        &input,
                        &config,
                        &mut worker,
                        Some(&owner),
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                let mut budget = assessment::OperationBudget::new(invocation, deadline)
                    .map_err(DurableError::Source)?;
                budget
                    .charge_bytes(
                        private_sources
                            .input_bytes()
                            .map_err(DurableError::Source)?,
                    )
                    .map_err(DurableError::Source)?;
                budget
                    .charge_bytes(public_sources.input_bytes().map_err(DurableError::Source)?)
                    .map_err(DurableError::Source)?;
                let mut resolved = assessment::AssessmentResolvedSources::from_selected_rows(
                    &public_sources.rows,
                    &private_sources.records,
                    &public_sources.native_rows,
                    &private_sources.native_records,
                    &mut budget,
                )
                .map_err(DurableError::Source)?;
                let layers =
                    crate::source_private_assessment_layers::select_owner_local_layers_selected_pg(
                        &mut owner,
                        &selected_context,
                        &config,
                        &resolved
                            .source_records
                            .iter()
                            .map(assessment::record_input)
                            .collect::<SourceCommandResult<Vec<_>>>()
                            .map_err(DurableError::Source)?,
                        &identity,
                        &schemas,
                        &mut worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                budget
                    .charge_bytes(usize::try_from(layers.input_bytes()).map_err(|_| {
                        DurableError::Source(SourceCommandError::Invalid(
                            "private Assessment layer input byte conversion",
                        ))
                    })?)
                    .map_err(DurableError::Source)?;
                resolved
                    .add_layer_records(&layers.records(), &mut budget)
                    .map_err(DurableError::Source)?;
                let plan = assessment::plan_assessment_evaluation(
                    assessment::AssessmentEvaluationInputs {
                        config: &config,
                        configuration_raw: &config_raw,
                        request: &request,
                        version,
                        resolved,
                        public_fixity: &public_sources.fixity,
                        public_identity_snapshots: &public_sources.identity_snapshots,
                        public_claim_dependencies: &public_sources.claim_dependencies,
                        private_source_snapshot: &private_sources.owner_local_sources_snapshot,
                        private_native_snapshots: &private_sources.native_snapshots,
                        private_required_source_refs: &private_sources.required_source_refs,
                        private_required_languages: &private_sources.required_languages,
                        native_summaries: &private_sources.native_summaries,
                        layers: Some(&layers),
                        layer_preflight: layer_preflight.as_ref(),
                    },
                    &mut budget,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let lock_subjects = plan.lock_subjects.clone();
                let mut subject_targets = lock_subjects
                    .iter()
                    .map(|subject| {
                        Ok((
                            subject.clone(),
                            grant
                                .target_ref(&owner, subject)
                                .map_err(DurableError::Source)?,
                        ))
                    })
                    .collect::<DurableResult<Vec<_>>>()?;
                subject_targets.sort_by(|left, right| left.0.cmp(&right.0));
                Ok((worker, schemas, lock_subjects, subject_targets))
            },
        )
        .map_err(pg::source_error)?;
    let mut all_targets = subject_targets
        .iter()
        .map(|(_, target)| target.clone())
        .chain(source_targets.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if all_targets.is_empty()
        || all_targets.len() > crate::durable_private_packages::MAX_PACKAGE_HISTORY_TARGETS
    {
        return Err(SourceCommandError::Invalid(
            "private Assessment package history target budget",
        ));
    }
    let baseline_packages = history_db
        .with_private_owner_package_histories_guard(
            &scope,
            &all_targets,
            crate::durable_private_packages::MAX_PACKAGE_HISTORY_ROWS,
            crate::durable_private_packages::MAX_PACKAGE_HISTORY_BYTES,
            deadline,
            cancelled,
            |histories, _, _| Ok(histories.clone()),
        )
        .map_err(pg::source_error)?;
    let expected_heads = subject_targets
        .iter()
        .map(|(subject, target)| {
            Ok((
                subject.clone(),
                package_head(baseline_packages.get(target).ok_or(
                    SourceCommandError::Conflict("private Assessment baseline history absent"),
                )?)?,
            ))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    let preflight = AssessmentPreflight {
        identity: initial_identity,
        worker: preflight_worker,
        schema_paths,
        schema_members,
        lock_subjects,
        subject_targets,
        expected_heads,
    };

    let mut worker = preflight.worker;
    let attempt = with_private_assessment_and_source_histories_guard_selected_schema(
            &mut history_db,
            &scope,
            &preflight.subject_targets,
            &source_targets,
            &preflight.expected_heads,
            preflight
                .schema_members
                .get(BATCH_SCHEMA)
                .ok_or(SourceCommandError::Conflict(
                    "private Assessment batch schema absent",
                ))?,
            &preflight.identity,
            &mut worker,
            crate::durable_private_packages::MAX_PACKAGE_HISTORY_ROWS,
            crate::durable_private_packages::MAX_PACKAGE_HISTORY_BYTES,
            deadline,
            cancelled,
            |histories, assessment_packages, source_packages, guarded_worker, held_deadline, held_cancelled| {
                let ((operation, owner, grant, current_config_raw, target_ref, expected_current, dependencies, private_generation, package_histories), readset) =
                    source_db.with_source_revision_session(
                        &store,
                        &selection.domain,
                        &selection.job_id,
                        selection.job_fence,
                        held_deadline,
                        held_cancelled,
                        |session| {
                            let identity = session.selection().identity();
                            if identity != preflight.identity || identity.contract != selection.contract {
                                return Err(DurableError::Conflict(
                                    "private Assessment source identity changed during history guard",
                                ));
                            }
                            let mut schemas = BTreeMap::new();
                            for path in &preflight.schema_paths {
                                schemas.insert(
                                    path.clone(),
                                    pg_read_member(
                                        session,
                                        path,
                                        MAX_SCHEMA_MEMBER_BYTES as usize,
                                        held_deadline,
                                        held_cancelled,
                                    )
                                    .map_err(DurableError::Source)?,
                                );
                            }
                            if schemas != preflight.schema_members {
                                return Err(DurableError::Conflict(
                                    "private Assessment selected schema bytes changed",
                                ));
                            }
                            let current_entity = pg_read_member(
                                session,
                                ENTITY_REGISTRY,
                                4_194_304,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            let current_relation = pg_read_member(
                                session,
                                RELATION_REGISTRY,
                                4_194_304,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            if current_entity != entity_raw || current_relation != relation_raw {
                                return Err(DurableError::Conflict(
                                    "private Assessment semantic registries changed",
                                ));
                            }
                            let mut authored = schemas.clone();
                            insert_dependency(
                                &mut authored,
                                ENTITY_REGISTRY.to_owned(),
                                current_entity,
                            )
                            .map_err(DurableError::Source)?;
                            insert_dependency(
                                &mut authored,
                                RELATION_REGISTRY.to_owned(),
                                current_relation,
                            )
                            .map_err(DurableError::Source)?;
                            for (path, raw) in &software_members {
                                if !path.starts_with("ToS/") {
                                    continue;
                                }
                                let selected = pg_read_member(
                                    session,
                                    path,
                                    usize::try_from(max_member_bytes).unwrap_or(
                                        MAX_PG_ASSESSMENT_MEMBER_BYTES as usize,
                                    ),
                                    held_deadline,
                                    held_cancelled,
                                )
                                .map_err(DurableError::Source)?;
                                if &selected != raw {
                                    return Err(DurableError::Conflict(
                                        "private Assessment selected software differs from PG source",
                                    ));
                                }
                                insert_dependency(&mut authored, path.clone(), selected)
                                    .map_err(DurableError::Source)?;
                            }
                            let context_schema = schemas
                                .get(CONTEXT_SCHEMA)
                                .ok_or(DurableError::Conflict(
                                    "private Assessment context schema absent",
                                ))?;
                            let mut selected_worker = DeferredFinish(guarded_worker);
                            let (mut owner, selected_context) = OwnerTextContext::select_pg(
                                &context_path,
                                context_schema,
                                &mut selected_worker,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            owner
                                .install_pg_selected_members(authored.clone())
                                .map_err(DurableError::Source)?;
                            let grant = ProtectedAssessmentPgGrant::select(
                                &grant_path,
                                &context_path,
                                &owner,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            let config = grant.configuration();
                            let current_config_raw = grant.configuration_raw().to_vec();
                            if current_config_raw != config_raw
                                || PrivateOwnerScopeV1::for_context(
                                    &selection.domain,
                                    owner
                                        .private_owner_scope_binding(held_deadline, held_cancelled)
                                        .map_err(DurableError::Source)?,
                                )
                                .map_err(|_| DurableError::Invalid("private Assessment scope"))?
                                    != scope
                            {
                                return Err(DurableError::Conflict(
                                    "private Assessment owner configuration or scope changed",
                                ));
                            }
                            let version = assessment::OwnerVersion::from_config(config)
                                .map_err(DurableError::Source)?;
                            let request = assessment::parse_request(&request_raw, version)
                                .map_err(DurableError::Source)?;
                            let mut command_files = authored.clone();
                            for (path, raw) in &software_members {
                                insert_dependency(&mut command_files, path.clone(), raw.clone())
                                    .map_err(DurableError::Source)?;
                            }
                            let command = command_context(
                                identity,
                                &current_config_raw,
                                &request_raw,
                                uid,
                                &command_files,
                            )
                            .map_err(DurableError::Source)?;
                            let mut input = PgGrowthCurrentInput::new(
                                session,
                                max_members,
                                usize::try_from(max_member_bytes).unwrap_or(
                                    MAX_PG_ASSESSMENT_MEMBER_BYTES as usize,
                                ),
                            )
                            .map_err(|_| {
                                DurableError::Source(SourceCommandError::Invalid(
                                    "private Assessment PG input budget",
                                ))
                            })?;
                            let private = crate::source_private_assessment_pg_sources::select_owner_local_sources_pg(
                                &mut owner,
                                &selected_context,
                                config,
                                &command,
                                &input,
                                source_packages,
                                &mut selected_worker.0,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            if private.target_refs != source_targets {
                                return Err(DurableError::Conflict(
                                    "private Assessment source package target set changed",
                                ));
                            }
                            let mut public = crate::source_sign::pg::select_owner_assessment_public_sources_pg(
                                &input,
                                config,
                                &mut selected_worker.0,
                                Some(&owner),
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            let mut budget = assessment::OperationBudget::new(invocation, held_deadline)
                                .map_err(DurableError::Source)?;
                            budget
                                .charge_bytes(
                                    private
                                        .input_bytes()
                                        .map_err(DurableError::Source)?,
                                )
                                .map_err(DurableError::Source)?;
                            budget
                                .charge_bytes(
                                    public
                                        .input_bytes()
                                        .map_err(DurableError::Source)?,
                                )
                                .map_err(DurableError::Source)?;
                            let mut resolved = assessment::AssessmentResolvedSources::from_selected_rows(
                                &public.rows,
                                &private.records,
                                &public.native_rows,
                                &private.native_records,
                                &mut budget,
                            )
                            .map_err(DurableError::Source)?;
                            let layers = crate::source_private_assessment_layers::select_owner_local_layers_selected_pg(
                                &mut owner,
                                &selected_context,
                                config,
                                &resolved
                                    .source_records
                                    .iter()
                                    .map(assessment::record_input)
                                    .collect::<SourceCommandResult<Vec<_>>>()
                                    .map_err(DurableError::Source)?,
                                &identity,
                                &schemas,
                                &mut selected_worker.0,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            budget
                                .charge_bytes(
                                    usize::try_from(layers.input_bytes()).map_err(|_| {
                                        DurableError::Source(SourceCommandError::Invalid(
                                            "private Assessment layer input byte conversion",
                                        ))
                                    })?,
                                )
                                .map_err(DurableError::Source)?;
                            resolved
                                .add_layer_records(&layers.records(), &mut budget)
                                .map_err(DurableError::Source)?;
                            let public_fixity = public.fixity.clone();
                            let public_identity_snapshots = public.identity_snapshots.clone();
                            let public_claim_dependencies = public.claim_dependencies.clone();
                            let plan = assessment::plan_assessment_evaluation(
                                assessment::AssessmentEvaluationInputs {
                                    config,
                                    configuration_raw: &current_config_raw,
                                    request: &request,
                                    version,
                                    resolved,
                                    public_fixity: &public_fixity,
                                    public_identity_snapshots: &public_identity_snapshots,
                                    public_claim_dependencies: &public_claim_dependencies,
                                    private_source_snapshot: &private.owner_local_sources_snapshot,
                                    private_native_snapshots: &private.native_snapshots,
                                    private_required_source_refs: &private.required_source_refs,
                                    private_required_languages: &private.required_languages,
                                    native_summaries: &private.native_summaries,
                                    layers: Some(&layers),
                                    layer_preflight: layer_preflight.as_ref(),
                                },
                                &mut budget,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            if plan.lock_subjects != preflight.lock_subjects {
                                return Err(DurableError::Conflict(
                                    "private Assessment history closure changed",
                                ));
                            }
                            let mut current_targets = plan
                                .lock_subjects
                                .iter()
                                .map(|subject| {
                                    Ok((
                                        subject.clone(),
                                        grant
                                            .target_ref(&owner, subject)
                                            .map_err(DurableError::Source)?,
                                    ))
                                })
                                .collect::<DurableResult<Vec<_>>>()?;
                            current_targets.sort_by(|left, right| left.0.cmp(&right.0));
                            if current_targets != preflight.subject_targets {
                                return Err(DurableError::Conflict(
                                    "private Assessment target mapping changed",
                                ));
                            }
                            let private_generation = assessment_packages
                                .values()
                                .chain(source_packages.values())
                                .map(|package| package.private_audit_generation)
                                .next()
                                .ok_or(DurableError::Invalid(
                                    "private Assessment audit generation absent",
                                ))?;
                            if assessment_packages
                                .values()
                                .chain(source_packages.values())
                                .any(|package| package.private_audit_generation != private_generation)
                            {
                                return Err(DurableError::Conflict(
                                    "private Assessment package audit generations differ",
                                ));
                            }
                            private
                                .verify_current(&owner, &input, source_packages, held_deadline, held_cancelled)
                                .map_err(DurableError::Source)?;
                            public
                                .verify_current(&input, Some(&owner), held_deadline, held_cancelled)
                                .map_err(DurableError::Source)?;
                            layers
                                .verify_current(&owner, held_deadline, held_cancelled)
                                .map_err(DurableError::Source)?;
                            owner
                                .verify_current(held_deadline, held_cancelled)
                                .map_err(DurableError::Source)?;
                            grant
                                .verify_current(held_deadline, held_cancelled)
                                .map_err(DurableError::Source)?;
                            let subject = &request.subject_id;
                            for lock_subject in &plan.lock_subjects {
                                let history = histories.get(lock_subject).ok_or(
                                    DurableError::Invalid(
                                        "private Assessment locked subject history absent",
                                    ),
                                )?;
                                budget
                                    .charge_work(1)
                                    .map_err(DurableError::Source)?;
                                budget
                                    .charge_bytes(
                                        history
                                            .input_bytes()
                                            .map_err(DurableError::Source)?,
                                    )
                                    .map_err(DurableError::Source)?;
                                budget
                                    .charge_work(
                                        history
                                            .submissions
                                            .len()
                                            .saturating_add(history.batches.len()),
                                    )
                                    .map_err(DurableError::Source)?;
                            }
                            let history = histories.get(subject).ok_or(DurableError::Invalid(
                                "private Assessment subject history absent",
                            ))?;
                            let mut evaluate = |read_input: &AssessmentReadInput,
                                                schema_worker: &mut DeferredFinish<'_, CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>>,
                                                limits: tos_validation::assessment::AssessmentLimits,
                                                cancelled: &AtomicBool| {
                                evaluate_current_assessment_selected(
                                    read_input,
                                    &identity,
                                    identity.digest(),
                                    &mut schema_worker.0,
                                    limits,
                                    cancelled,
                                )
                                .map_err(assessment::map_assessment_refusal)
                            };
                            let evaluation = assessment::evaluate_assessment_histories(
                                &plan,
                                histories,
                                &command.recorded_at,
                                None,
                                identity.digest(),
                                &mut selected_worker,
                                &mut budget,
                                held_cancelled,
                                &mut evaluate,
                            )
                            .map_err(DurableError::Source)?;
                            let operation = execute_selected_operation(
                                &request,
                                version,
                                config,
                                &command,
                                &plan,
                                history,
                                &evaluation,
                                &private.records,
                                &private.native_summaries,
                                &private.form_paths,
                                &private.form_sets,
                                &private.source_paths,
                                &mut public,
                                Some(&layers),
                                &mut selected_worker,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            let mut dependencies = authored;
                            for (path, raw) in &software_members {
                                insert_dependency(&mut dependencies, path.clone(), raw.clone())
                                    .map_err(DurableError::Source)?;
                            }
                            for (path, raw) in private.dependency_members() {
                                insert_dependency(&mut dependencies, path, raw)
                                    .map_err(DurableError::Source)?;
                            }
                            for (path, raw) in &public.selected_members {
                                insert_dependency(&mut dependencies, path.clone(), raw.clone())
                                    .map_err(DurableError::Source)?;
                            }
                            let context_snapshot = owner
                                .snapshot(held_deadline, held_cancelled)
                                .map_err(DurableError::Source)?
                                .to_prefixed();
                            insert_dependency(
                                &mut dependencies,
                                "private-owner/context-snapshot.sha256".to_owned(),
                                context_snapshot.as_bytes().to_vec(),
                            )
                            .map_err(DurableError::Source)?;
                            if let Some(layers) = Some(&layers) {
                                insert_dependency(
                                    &mut dependencies,
                                    "private-owner/native-layer-assessment-snapshot.sha256".to_owned(),
                                    layers.source_snapshot.as_bytes().to_vec(),
                                )
                                .map_err(DurableError::Source)?;
                            }
                            let target_ref = grant
                                .target_ref(&owner, subject)
                                .map_err(DurableError::Source)?;
                            let expected_current = assessment_packages
                                .get(subject)
                                .and_then(|package| package.current.clone());
                            let mut package_histories = BTreeMap::new();
                            for (subject_id, assessment_target) in &preflight.subject_targets {
                                let history = assessment_packages.get(subject_id).ok_or(
                                    DurableError::Invalid(
                                        "private Assessment subject package history absent",
                                    ),
                                )?;
                                if package_histories
                                    .insert(assessment_target.clone(), history.clone())
                                    .is_some()
                                {
                                    return Err(DurableError::Conflict(
                                        "private Assessment package target collision",
                                    ));
                                }
                            }
                            for (target, history) in source_packages {
                                if package_histories.insert(target.clone(), history.clone()).is_some() {
                                    return Err(DurableError::Conflict(
                                        "private Assessment history target collision",
                                    ));
                                }
                            }
                            if read_absolute(
                                invocation_path,
                                uid,
                                true,
                                invocation_raw.len(),
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(DurableError::Source)?
                                != invocation_raw
                                || runtime
                                    .current_digest(held_deadline, held_cancelled)
                                    .map_err(DurableError::Source)?
                                    != executable_digest
                            {
                                return Err(DurableError::Conflict(
                                    "private Assessment invocation or executable changed",
                                ));
                            }
                            Ok((
                                operation,
                                owner,
                                grant,
                                current_config_raw,
                                target_ref,
                                expected_current,
                                dependencies,
                                private_generation,
                                package_histories,
                            ))
                        },
                    )?;
                let binding = source_binding(&readset, &dependencies, private_generation)
                    .map_err(DurableError::Source)?;
                Ok(PgAssessmentAttempt {
                    operation,
                    owner,
                    grant,
                    config_raw: current_config_raw,
                    target_ref,
                    expected_current,
                    dependencies,
                    package_histories,
                    source_binding: binding,
                })
            },
        )
        .map_err(pg::source_error)?;

    let context_snapshot = attempt.owner.snapshot(deadline, cancelled)?.to_prefixed();
    if attempt
        .dependencies
        .get("private-owner/context-snapshot.sha256")
        .map(Vec::as_slice)
        != Some(context_snapshot.as_bytes())
    {
        return Err(SourceCommandError::Conflict(
            "private Assessment owner context changed after selection",
        ));
    }
    attempt.grant.verify_current(deadline, cancelled)?;
    if attempt.grant.configuration_raw() != attempt.config_raw
        || read_absolute(
            invocation_path,
            uid,
            true,
            invocation_raw.len(),
            deadline,
            cancelled,
        )? != invocation_raw
        || runtime.current_digest(deadline, cancelled)? != executable_digest
        || selected_component_members(
            &software,
            &components,
            max_member_bytes,
            deadline,
            cancelled,
        )? != software_members
    {
        return Err(SourceCommandError::Conflict(
            "private Assessment invocation, owner grant or software changed",
        ));
    }
    all_targets = attempt.package_histories.keys().cloned().collect();
    let current_package_histories = history_db
        .with_private_owner_package_histories_guard(
            &scope,
            &all_targets,
            crate::durable_private_packages::MAX_PACKAGE_HISTORY_ROWS,
            crate::durable_private_packages::MAX_PACKAGE_HISTORY_BYTES,
            deadline,
            cancelled,
            |histories, _, _| Ok(histories.clone()),
        )
        .map_err(pg::source_error)?;
    if current_package_histories != attempt.package_histories {
        return Err(SourceCommandError::Conflict(
            "private Assessment source or history packages changed after selection",
        ));
    }

    match attempt.operation {
        PgAssessmentOperation::Read(result) => {
            serde_json::from_slice(&cmd::canonical(&cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_native_pg_private_assessment_result_v1"),
                ),
                ("authentication", cmd::string("local-unix-account")),
                ("family", cmd::string("assessment")),
                (
                    "command_id",
                    request
                        .command_id
                        .as_ref()
                        .map(|value| cmd::string(value))
                        .unwrap_or(JsonValue::Null),
                ),
                ("replayed", JsonValue::Bool(false)),
                ("grants_admission", JsonValue::Bool(false)),
                ("result", result),
            ]))?)
            .map_err(|_| SourceCommandError::Invalid("private Assessment result JSON"))
        }
        PgAssessmentOperation::Append(AssessmentAppendPreparation::Replayed(result)) => {
            serde_json::from_slice(&cmd::canonical(&cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_native_pg_private_assessment_result_v1"),
                ),
                ("authentication", cmd::string("local-unix-account")),
                ("family", cmd::string("assessment")),
                (
                    "command_id",
                    request
                        .command_id
                        .as_ref()
                        .map(|value| cmd::string(value))
                        .unwrap_or(JsonValue::Null),
                ),
                ("replayed", JsonValue::Bool(true)),
                ("grants_admission", JsonValue::Bool(false)),
                ("result", result),
            ]))?)
            .map_err(|_| SourceCommandError::Invalid("private Assessment result JSON"))
        }
        PgAssessmentOperation::Append(AssessmentAppendPreparation::New {
            batch,
            request_digest,
            result,
            ..
        }) => {
            let command_id = request
                .command_id
                .as_deref()
                .ok_or(SourceCommandError::Invalid(
                    "private Assessment append command identity absent",
                ))?;
            let response_bytes = cmd::canonical(&result)?;
            let expected_package_heads = preflight
                .subject_targets
                .iter()
                .map(|(subject_id, target)| {
                    Ok((
                        target.clone(),
                        preflight.expected_heads.get(subject_id).cloned().ok_or(
                            SourceCommandError::Conflict(
                                "private Assessment expected subject head absent",
                            ),
                        )?,
                    ))
                })
                .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
            let write = PrivateOwnerPackageWriteV1 {
                scope: scope.clone(),
                family: PrivateOwnerPackageFamilyV1::Assessment,
                target_ref: attempt.target_ref.clone(),
                expected_current: attempt.expected_current.clone(),
                members: BTreeMap::new(),
                archives: Vec::new(),
                command_id: command_id.to_owned(),
                request_digest,
                raw_request_digest: Digest256::of_bytes(&request_raw),
                owner_configuration_digest: Digest256::of_bytes(&attempt.config_raw),
                source_binding: attempt.source_binding,
                response_bytes,
            };
            let revalidate_currentness = || -> SourceCommandResult<()> {
                let context_snapshot = attempt.owner.snapshot(deadline, cancelled)?.to_prefixed();
                if attempt
                    .dependencies
                    .get("private-owner/context-snapshot.sha256")
                    .map(Vec::as_slice)
                    != Some(context_snapshot.as_bytes())
                {
                    return Err(SourceCommandError::Conflict(
                        "private Assessment owner context changed before commit",
                    ));
                }
                attempt.grant.verify_current(deadline, cancelled)?;
                if attempt.grant.configuration_raw() != attempt.config_raw
                    || read_absolute(
                        invocation_path,
                        uid,
                        true,
                        invocation_raw.len(),
                        deadline,
                        cancelled,
                    )? != invocation_raw
                    || runtime.current_digest(deadline, cancelled)? != executable_digest
                    || selected_component_members(
                        &software,
                        &components,
                        max_member_bytes,
                        deadline,
                        cancelled,
                    )? != software_members
                {
                    return Err(SourceCommandError::Conflict(
                        "private Assessment commit inputs changed",
                    ));
                }
                Ok(())
            };
            let replay = commit_private_assessment_batch_with_expected_heads_and_currentness(
                &mut commit_db,
                write,
                &batch,
                expected_package_heads,
                revalidate_currentness,
            )
            .map_err(pg::source_error)?;
            serde_json::from_slice(&cmd::canonical(&cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_native_pg_private_assessment_result_v1"),
                ),
                ("authentication", cmd::string("local-unix-account")),
                ("family", cmd::string("assessment")),
                ("command_id", cmd::string(command_id)),
                ("commit_seq", cmd::number(replay.commit_seq)),
                (
                    "package_sha256",
                    cmd::string(&replay.package_digest.to_prefixed()),
                ),
                (
                    "receipt_sha256",
                    cmd::string(&Digest256::of_bytes(&replay.response_bytes).to_prefixed()),
                ),
                ("replayed", JsonValue::Bool(replay.replayed)),
                ("grants_admission", JsonValue::Bool(false)),
                ("result", result),
            ]))?)
            .map_err(|_| SourceCommandError::Invalid("private Assessment result JSON"))
        }
    }
}
