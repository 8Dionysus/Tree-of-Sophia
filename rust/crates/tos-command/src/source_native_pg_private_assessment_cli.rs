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
    commit_private_assessment_batch_with_expected_heads,
    with_private_assessment_and_source_histories_guard_selected_schema,
};
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, SourceRevisionReadSession, SourceRevisionReadSet,
};
use crate::source_command::{
    self as cmd, CommandContext, SourceCommandError, SourceCommandResult, SourceFile,
};
use crate::source_native_cli::pg_claims as pg;
use crate::source_native_cli::private_assessment as assessment;
use crate::source_native_cli::private_assessment::AssessmentAppendPreparation;
use crate::source_native_pg_growth_input::PgGrowthCurrentInput;
use crate::source_private_assessment_layers::{OwnerLocalLayerPreflight, PrivateAssessmentLayers};
use crate::source_revisions::ReadonlyRecordFiles;
use crate::source_sign::pg::PgOwnerAssessmentPublicSources;
use crate::source_text_owner::{OwnerTextContext, read_absolute};
use crate::{DurableError, DurablePgCoordinator};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath, SourceRevision};
use tos_segment_store::{SegmentLimits, SegmentStore};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::assessment::{
    AssessmentMechanicsReport, AssessmentReadInput, AssessmentRecordInput, AssessmentRefusal,
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
    public_sources: &mut PgOwnerAssessmentPublicSources<'_>,
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
                let public_maps = assessment::AssessmentFormMaps {
                    form_paths: &public_sources.form_paths,
                    form_sets: &public_sources.form_sets,
                    source_paths: &public_sources.record_paths,
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
    public_sources: &PgOwnerAssessmentPublicSources<'_>,
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
            identity,
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
    let _request = assessment::parse_request(&request_raw, version)?;
    if !matches!(
        version,
        assessment::OwnerVersion::V5 | assessment::OwnerVersion::V6
    ) || selected_context.is_null()
    {
        return Err(SourceCommandError::Denied(
            "private PostgreSQL Assessment owner selection",
        ));
    }
    owner.verify_current(deadline, cancelled)?;
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

    // The selected PG source, protected v5/v6 grant, and candidate schema
    // profile are now bound to one source epoch. Operation-history evaluation
    // is deliberately fail-closed until its complete multi-target guard and
    // private-broker writer are attached to this callable route.
    let _ = (
        max_members,
        max_total_bytes,
        max_member_bytes,
        components,
        software,
        owner,
        scope,
        config_raw,
        entity_raw,
        relation_raw,
        source_targets,
        request_raw,
    );
    Err(SourceCommandError::Unsupported(
        "private PostgreSQL Assessment history-bound operation runtime",
    ))
}
