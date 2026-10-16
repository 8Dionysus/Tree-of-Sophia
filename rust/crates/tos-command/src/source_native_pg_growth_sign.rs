//! PostgreSQL route for the v2 Sign owner.
//!
//! Sign is a source mutation whose proposal carries a protected Assessment
//! decision. The PG route keeps both the Assessment and owner-local source
//! package histories held through revision registration and commit. A retry
//! validates the already sealed basis under those same holds and never
//! replaces it with a new Assessment result.

use crate::durable_private_packages::{
    MAX_PACKAGE_HISTORY_BYTES, MAX_PACKAGE_HISTORY_ROWS, MAX_PACKAGE_HISTORY_TARGETS,
    PrivateOwnerScopeV1,
};
use crate::source_assessment_journal::{
    ProtectedAssessmentPgGrant, with_private_assessment_and_source_histories_guard_selected_schema,
};
use crate::source_cohort::revision::{PgRecordRevisionIdentity, SourceRevisionReadSession};
use crate::source_command::{
    self as cmd, CommandContext, SourceCommandError, SourceCommandResult, SourceFile,
};
use crate::source_creation_store::CreationFilesystem;
use crate::source_native_cli::PgGrowthCurrentInput;
use crate::source_native_cli::private_assessment as assessment;
use crate::source_revisions::ReadonlyRecordFiles;
use crate::source_text_owner::{OwnerTextContext, normalized_absolute, read_absolute};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_segment_store::SegmentStore;
use tos_validation::record_biblio_cut::SourceCutInput;
use tos_validation::record_biblio_cut::SourceCutInputWithIdentity;
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const BATCH_SCHEMA: &str = "ToS/contracts/knowledge-assessment-batch.schema.json";
const ENTITY_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION_REGISTRY: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const SIGN_SCHEMA_INPUTS: &[&str] = &[
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/source-metadata-record.schema.json",
    "ToS/contracts/sign-description-record.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];
const MAX_SCHEMA_PATHS: usize = 128;
const MAX_SCHEMA_BYTES: usize = 4_194_304;
const MAX_SELECTED_MEMBERS: usize = 4096;

/// The Sign public-source preparer needs only addressed member reads. Keep it
/// on the same PG read session; this adapter does not expose whole-source
/// enumeration or a synthetic cut.
struct PgGrowthFiles<'a, 'session, 'store> {
    input: &'a PgGrowthCurrentInput<'session, 'store>,
}

impl ReadonlyRecordFiles for PgGrowthFiles<'_, '_, '_> {
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let mut selected = None;
        self.input
            .source_input()
            .with_current_member(path, max_bytes, deadline, cancelled, &mut |meta, raw| {
                if meta.path != path || selected.is_some() {
                    return Err(tos_validation::item_rules::ItemRefusal::Source(
                        "repeated Sign PG selected member".into(),
                    ));
                }
                selected = Some(raw.to_vec());
                Ok(())
            })
            .map_err(|_| SourceCommandError::Conflict("Sign PG selected source read"))?;
        selected.ok_or(SourceCommandError::Conflict(
            "Sign PG selected source member absent",
        ))
    }

    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        self.input.list_directory(path, deadline, cancelled)
    }

    fn has_file(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<bool> {
        self.input
            .path_presence(path, deadline, cancelled)
            .map(|presence| presence.is_some())
            .map_err(|_| SourceCommandError::Conflict("Sign PG selected path probe"))
    }
}

struct SignPreflight {
    identity: PgRecordRevisionIdentity,
    owner: OwnerTextContext,
    assessment_config: JsonValue,
    assessment_config_raw: Vec<u8>,
    context_schema: Vec<u8>,
    entity_registry: Vec<u8>,
    relation_registry: Vec<u8>,
    scope: PrivateOwnerScopeV1,
    subject_targets: Vec<(String, String)>,
    source_targets: Vec<String>,
}

enum SignPrepared {
    Preview(JsonValue),
    Mutation(super::PreparedPgGrowthRevision),
    Replay,
}

fn package_head(
    package: &crate::durable_private_packages::PrivateOwnerPackageHistoryV1,
) -> SourceCommandResult<Option<String>> {
    let Some(current) = &package.current else {
        if package.family.is_some() || !package.history.is_empty() {
            return Err(SourceCommandError::Conflict(
                "Sign PG package history has no current head",
            ));
        }
        return Ok(None);
    };
    let raw = current
        .members
        .get("head")
        .ok_or(SourceCommandError::Conflict(
            "Sign PG package head member absent",
        ))?;
    let head = std::str::from_utf8(raw)
        .map_err(|_| SourceCommandError::Invalid("Sign PG package head UTF-8"))?;
    if head.is_empty() || head.len() > 65 {
        return Err(SourceCommandError::Invalid("Sign PG package head length"));
    }
    Ok(Some(head.to_owned()))
}

fn merge_member(
    members: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    raw: Vec<u8>,
) -> SourceCommandResult<()> {
    if members
        .insert(path, raw.clone())
        .is_some_and(|previous| previous != raw)
    {
        return Err(SourceCommandError::Conflict(
            "Sign PG selected dependency bytes differ",
        ));
    }
    if members.len() > MAX_SELECTED_MEMBERS {
        return Err(SourceCommandError::Invalid("Sign PG selected member count"));
    }
    Ok(())
}

fn source_files(members: BTreeMap<String, Vec<u8>>) -> SourceCommandResult<Vec<SourceFile>> {
    members
        .into_iter()
        .map(|(path, raw)| {
            let relative = RelativePath::parse(&path)
                .map_err(|_| SourceCommandError::Invalid("Sign PG selected member path"))?;
            if relative.as_str() != path {
                return Err(SourceCommandError::Invalid(
                    "Sign PG noncanonical member path",
                ));
            }
            Ok(SourceFile {
                path: relative,
                raw,
            })
        })
        .collect()
}

fn preflight_sign_owner(
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &Value,
    selection: &super::GrowthSourceSelection,
    assessment_path: &Path,
    context_path: &Path,
    owner_config: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SignPreflight> {
    let identity = session.selection().identity();
    if identity.contract != selection.contract {
        return Err(SourceCommandError::Conflict(
            "Sign PG source contract differs from selected domain",
        ));
    }
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_SCHEMA_BYTES)
        .map_err(|_| SourceCommandError::Invalid("Sign PG selected input budget"))?;
    let mut files = PgGrowthFiles { input: &input };
    let context_schema = files.read(CONTEXT_SCHEMA, MAX_SCHEMA_BYTES, deadline, cancelled)?;
    let mut worker = assessment::selected_assessment_candidate_worker(
        invocation,
        &mut files,
        identity,
        &[CONTEXT_SCHEMA.to_owned()],
        deadline,
        cancelled,
    )?;
    let (owner, selected_context) = OwnerTextContext::select_pg(
        context_path,
        &context_schema,
        &mut worker,
        deadline,
        cancelled,
    )?;
    let grant = ProtectedAssessmentPgGrant::select(
        assessment_path,
        context_path,
        &owner,
        deadline,
        cancelled,
    )?;
    let assessment_config = grant.configuration().clone();
    let assessment_config_raw = grant.configuration_raw().to_vec();
    let version = assessment::OwnerVersion::from_config(&assessment_config)?;
    if !matches!(
        version,
        assessment::OwnerVersion::V5 | assessment::OwnerVersion::V6
    ) {
        return Err(SourceCommandError::Denied(
            "PG Sign requires protected Assessment owner v5/v6",
        ));
    }
    assessment::preflight_configuration(&assessment_config, version)?;
    let binding = owner.private_owner_scope_binding(deadline, cancelled)?;
    let scope = PrivateOwnerScopeV1::for_context(&selection.domain, binding)
        .map_err(|_| SourceCommandError::Invalid("Sign PG private owner scope"))?;
    let subject_map = cmd::field(&assessment_config, "subjects")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("Sign PG Assessment subjects"))?;
    if subject_map.is_empty() || subject_map.len() > MAX_PACKAGE_HISTORY_TARGETS {
        return Err(SourceCommandError::Invalid(
            "Sign PG Assessment subject history budget",
        ));
    }
    let candidate = cmd::text(owner_config, "promotion_candidate_id")?;
    let mut subject_targets = subject_map
        .iter()
        .map(|(subject, _)| {
            let subject = subject
                .as_str()
                .ok_or(SourceCommandError::Invalid("Sign PG Assessment subject ID"))?;
            Ok((subject.to_owned(), grant.target_ref(&owner, subject)?))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    subject_targets.sort_by(|left, right| left.0.cmp(&right.0));
    if !subject_targets
        .iter()
        .any(|(subject, _)| subject == candidate)
    {
        return Err(SourceCommandError::Denied(
            "Sign candidate is outside protected Assessment subjects",
        ));
    }
    let source_targets = crate::source_private_assessment_pg_sources::private_source_target_refs(
        &owner,
        &assessment_config,
    )?;
    let entity_registry = files.read(ENTITY_REGISTRY, MAX_SCHEMA_BYTES, deadline, cancelled)?;
    let relation_registry = files.read(RELATION_REGISTRY, MAX_SCHEMA_BYTES, deadline, cancelled)?;
    owner.install_pg_selected_members(BTreeMap::from([(
        CONTEXT_SCHEMA.to_owned(),
        context_schema.clone(),
    )]))?;
    owner.snapshot(deadline, cancelled)?;
    grant.verify_current(deadline, cancelled)?;
    crate::source_sign::finish_worker(&mut worker, deadline, cancelled)?;
    drop(selected_context);
    drop(files);
    drop(input);
    Ok(SignPreflight {
        identity,
        owner,
        assessment_config,
        assessment_config_raw,
        context_schema,
        entity_registry,
        relation_registry,
        scope,
        subject_targets,
        source_targets,
    })
}

fn selected_schema_members(
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &Value,
    preflight: &SignPreflight,
    initial_packages: &BTreeMap<
        String,
        crate::durable_private_packages::PrivateOwnerPackageHistoryV1,
    >,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    BTreeMap<String, Vec<u8>>,
    CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
)> {
    let identity = session.selection().identity();
    if identity != preflight.identity {
        return Err(SourceCommandError::Conflict(
            "Sign PG source identity changed during schema preflight",
        ));
    }
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_SCHEMA_BYTES)
        .map_err(|_| SourceCommandError::Invalid("Sign PG selected input budget"))?;
    let mut files = PgGrowthFiles { input: &input };
    let mut paths = crate::source_private_assessment_pg_sources::private_source_schema_paths(
        &preflight.owner,
        &preflight.assessment_config,
        initial_packages,
        &preflight.entity_registry,
        &preflight.relation_registry,
    )?
    .into_iter()
    .collect::<BTreeSet<_>>();
    paths.insert(CONTEXT_SCHEMA.to_owned());
    paths.insert(BATCH_SCHEMA.to_owned());
    paths.insert("ToS/contracts/semantic-entity-type-registry.schema.json".to_owned());
    for path in crate::source_private_assessment_layers::selected_pg_contract_paths() {
        paths.insert((*path).to_owned());
    }
    paths.extend(SIGN_SCHEMA_INPUTS.iter().map(|path| (*path).to_owned()));
    paths.extend(crate::source_sign::pg::candidate_schema_paths_from_config(
        &mut files,
        &preflight.assessment_config,
        deadline,
        cancelled,
    )?);
    if paths.is_empty() || paths.len() > MAX_SCHEMA_PATHS {
        return Err(SourceCommandError::Invalid("Sign PG schema closure budget"));
    }
    let mut schema_members = BTreeMap::new();
    for path in paths {
        let raw = files.read(&path, MAX_SCHEMA_BYTES, deadline, cancelled)?;
        merge_member(&mut schema_members, path, raw)?;
    }
    if schema_members.get(CONTEXT_SCHEMA) != Some(&preflight.context_schema) {
        return Err(SourceCommandError::Conflict(
            "Sign PG owner context schema changed",
        ));
    }
    let paths = schema_members.keys().cloned().collect::<Vec<_>>();
    let guard_worker = assessment::selected_assessment_candidate_worker(
        invocation, &mut files, identity, &paths, deadline, cancelled,
    )?;
    let operation_worker = assessment::selected_assessment_candidate_worker(
        invocation, &mut files, identity, &paths, deadline, cancelled,
    )?;
    drop(files);
    drop(input);
    Ok((schema_members, guard_worker, operation_worker))
}

fn current_package_members(
    session: &mut SourceRevisionReadSession<'_>,
    schema_members: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for (path, expected) in schema_members {
        let relative = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("Sign PG schema path"))?;
        let current = session
            .read_current_member(&relative, MAX_SCHEMA_BYTES as u64, deadline, cancelled)
            .map_err(super::growth_source_error)?;
        if &current.raw != expected {
            return Err(SourceCommandError::Conflict(
                "Sign PG selected schema bytes changed",
            ));
        }
    }
    Ok(())
}

fn selected_target_exists(
    input: &PgGrowthCurrentInput<'_, '_>,
    owner_config: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    let source = cmd::text(owner_config, "source_path")?;
    let home = source
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(SourceCommandError::Invalid("Sign PG source home"))?;
    let (parent, name) = home
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Sign PG source parent"))?;
    Ok(input
        .list_directory(parent, deadline, cancelled)?
        .iter()
        .any(|(child, _)| child == name))
}

fn evaluate_sign_view(
    invocation: &Value,
    config: &JsonValue,
    config_raw: &[u8],
    public: &crate::source_sign::pg::PgOwnerAssessmentPublicSources,
    private: &crate::source_private_assessment_pg_sources::PgPrivateAssessmentSources,
    owner: &mut OwnerTextContext,
    selected_context: &JsonValue,
    identity: PgRecordRevisionIdentity,
    schemas: &BTreeMap<String, Vec<u8>>,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    candidate: &str,
    histories: &BTreeMap<String, crate::source_assessment_journal::AssessmentHistory>,
    require_ready: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, assessment::OperationBudget)> {
    let version = assessment::OwnerVersion::from_config(config)?;
    let request_raw = cmd::canonical(&cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_assessment_command_v1"),
        ),
        ("operation", cmd::string("describe")),
        ("subject_id", cmd::string(candidate)),
    ]))?;
    let request = assessment::parse_request(&request_raw, version)?;
    let mut budget = assessment::OperationBudget::new(invocation, deadline)?;
    budget.charge_bytes(private.input_bytes()?)?;
    budget.charge_bytes(public.input_bytes()?)?;
    let mut resolved = assessment::AssessmentResolvedSources::from_selected_rows(
        &public.rows,
        &private.records,
        &public.native_rows,
        &private.native_records,
        &mut budget,
    )?;
    let records = resolved
        .source_records
        .iter()
        .map(assessment::record_input)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let layers = crate::source_private_assessment_layers::select_owner_local_layers_selected_pg(
        owner,
        selected_context,
        config,
        &records,
        &identity,
        schemas,
        worker,
        deadline,
        cancelled,
    )?;
    budget.charge_bytes(
        usize::try_from(layers.input_bytes())
            .map_err(|_| SourceCommandError::Invalid("Sign PG layer input byte count"))?,
    )?;
    resolved.add_layer_records(&layers.records(), &mut budget)?;
    let layer_preflight = crate::source_private_assessment_layers::preflight_owner_local_layers(
        config,
        cmd::field(config, "subjects")?,
        cmd::text(config, "schema_version")?,
    )?;
    let plan = assessment::plan_assessment_evaluation(
        assessment::AssessmentEvaluationInputs {
            config,
            configuration_raw: config_raw,
            request: &request,
            version,
            resolved,
            public_fixity: &public.fixity,
            public_identity_snapshots: &public.identity_snapshots,
            public_claim_dependencies: &public.claim_dependencies,
            private_source_snapshot: &private.owner_local_sources_snapshot,
            private_native_snapshots: &private.native_snapshots,
            private_required_source_refs: &private.required_source_refs,
            private_required_languages: &private.required_languages,
            native_summaries: &private.native_summaries,
            layers: Some(&layers),
            layer_preflight: Some(&layer_preflight),
        },
        &mut budget,
        deadline,
        cancelled,
    )?;
    if plan.source_route != tos_validation::assessment::AssessmentSourceRoute::SourceBoundClaim
        || !plan.lock_subjects.iter().all(|subject| {
            cmd::field(config, "subjects")
                .ok()
                .and_then(JsonValue::as_object)
                .is_some_and(|subjects| {
                    subjects
                        .iter()
                        .any(|(candidate, _)| candidate.as_str() == Some(subject.as_str()))
                })
        })
    {
        return Err(SourceCommandError::Conflict(
            "Sign PG Assessment history closure differs from protected grant",
        ));
    }
    let (view, _) = crate::source_sign::pg::evaluate_promotion_view_selected_pg(
        &plan,
        histories,
        public,
        candidate,
        &crate::source_serialization::instant()?,
        identity,
        worker,
        &mut budget,
        require_ready,
        cancelled,
    )?;
    Ok((view, budget))
}

fn check_invocation_and_owner(
    invocation_path: &Path,
    invocation_raw: &[u8],
    uid: u32,
    owner_path: &Path,
    owner_raw: &[u8],
    selection: &super::GrowthSourceSelection,
    software: &tos_source_store::SoftwareCaptureReader,
    components: &tos_source_store::SoftwareComponentSelectionV1,
    expected_software_files: &[cmd::SourceFile],
    expected_executable: Digest256,
    runtime: &crate::source_serialization::ExecutableObservation,
    owner: &OwnerTextContext,
    grant: &ProtectedAssessmentPgGrant,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> crate::DurableResult<()> {
    if read_absolute(
        invocation_path,
        uid,
        true,
        invocation_raw.len(),
        deadline,
        cancelled,
    )
    .map_err(crate::DurableError::Source)?
        != invocation_raw
    {
        return Err(crate::DurableError::Conflict(
            "PG Sign invocation changed before commit",
        ));
    }
    let (_, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )
    .map_err(crate::DurableError::Source)?;
    if current_raw != owner_raw
        || runtime
            .current_digest(deadline, cancelled)
            .map_err(crate::DurableError::Source)?
            != expected_executable
    {
        return Err(crate::DurableError::Conflict(
            "PG Sign owner or executable changed before commit",
        ));
    }
    if super::selected_software_files(software, components, deadline, cancelled)
        .map_err(crate::DurableError::Source)?
        != expected_software_files
    {
        return Err(crate::DurableError::Conflict(
            "PG Sign selected software changed before commit",
        ));
    }
    owner
        .snapshot(deadline, cancelled)
        .map_err(crate::DurableError::Source)?;
    grant
        .verify_current(deadline, cancelled)
        .map_err(crate::DurableError::Source)?;
    Ok(())
}

/// Run the v2 private-Assessment Sign route. Previews and mutation use the
/// same selected PG source and guarded Assessment basis; only mutation enters
/// the common source revision writer.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    request: &JsonValue,
    uid: u32,
    selection: &super::GrowthSourceSelection,
    owner_path: &Path,
    owner_raw: &[u8],
    owner_config: &JsonValue,
    expected_executable: Digest256,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if cmd::text(owner_config, "schema_version")? != "tos_local_sign_promote_owner_v2" {
        return Err(SourceCommandError::Unsupported(
            "PG Sign requires protected owner v2; legacy filesystem Assessment remains local",
        ));
    }
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported("PG Sign request profile"));
    }
    let operation = cmd::text(request, "operation")?;
    if !matches!(
        operation,
        "describe" | "prepare" | "prepare-create" | "sign.promote"
    ) {
        return Err(SourceCommandError::Unsupported("PG Sign operation"));
    }
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native PG Sign executable identity",
        ));
    }
    let command_id = if operation == "sign.promote" {
        Some(cmd::text(request, "command_id")?)
    } else {
        None
    };
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected PG Sign STO"))?;
    let (software, components) = super::growth_software_selection(invocation, deadline, cancelled)?;
    let expected_software_files =
        super::selected_software_files(&software, &components, deadline, cancelled)?;
    let mut source_db = crate::DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PG Sign PostgreSQL"))?,
    );
    let mut history_db = crate::DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("PG Sign history PostgreSQL"))?,
    );
    let mut commit_db = crate::DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("PG Sign commit PostgreSQL"))?,
    );

    // Resolve only an already sealed attempt. Basis validation is deferred
    // until the selected Assessment and source package histories are held.
    let recovered = if let Some(command_id) = command_id {
        let prepare_id = super::growth_prepare_id(&selection.domain, command_id);
        match commit_db.recover_source_revision_attempt(
            &store,
            &selection.domain,
            &prepare_id,
            command_id,
            Digest256::of_bytes(request_raw),
            Digest256::of_bytes(owner_raw),
            cmd::record_digest(owner_config)?,
            super::PgGrowthFamily::SignPromotion.family_tag(),
            false,
            &selection.job_id,
            selection.job_fence,
            None,
            deadline,
            cancelled,
        ) {
            Ok(attempt) => {
                if attempt.contract != selection.contract {
                    return Err(SourceCommandError::Conflict(
                        "recovered PG Sign source contract changed",
                    ));
                }
                Some(attempt)
            }
            Err(crate::DurableError::Refused("source revision attempt absent")) => None,
            Err(error) => return Err(super::growth_source_error(error)),
        }
    } else {
        None
    };

    let context_path = normalized_absolute(cmd::text(
        owner_config,
        "promotion_assessment_private_owner_config",
    )?)?;
    let assessment_context_path =
        ProtectedAssessmentPgGrant::source_context_ref(&context_path, deadline, cancelled)?;
    let (preflight, _) = source_db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                preflight_sign_owner(
                    session,
                    invocation,
                    selection,
                    &context_path,
                    &assessment_context_path,
                    owner_config,
                    deadline,
                    cancelled,
                )
                .map_err(crate::DurableError::Source)
            },
        )
        .map_err(super::growth_source_error)?;

    let mut all_targets = preflight
        .subject_targets
        .iter()
        .map(|(_, target)| target.clone())
        .collect::<BTreeSet<_>>();
    for target in &preflight.source_targets {
        if !all_targets.insert(target.clone()) {
            return Err(SourceCommandError::Conflict(
                "Sign Assessment and source package target overlap",
            ));
        }
    }
    if all_targets.is_empty() || all_targets.len() > MAX_PACKAGE_HISTORY_TARGETS {
        return Err(SourceCommandError::Invalid(
            "Sign PG package history target budget",
        ));
    }
    let all_targets = all_targets.into_iter().collect::<Vec<_>>();
    let baseline_packages = history_db
        .with_private_owner_package_histories_guard(
            &preflight.scope,
            &all_targets,
            MAX_PACKAGE_HISTORY_ROWS,
            MAX_PACKAGE_HISTORY_BYTES,
            deadline,
            cancelled,
            |histories, _, _| Ok(histories.clone()),
        )
        .map_err(super::growth_source_error)?;
    let mut expected_heads = BTreeMap::new();
    for (subject, target) in &preflight.subject_targets {
        let package = baseline_packages
            .get(target)
            .ok_or(SourceCommandError::Conflict(
                "Sign PG Assessment history target absent",
            ))?;
        expected_heads.insert(subject.clone(), package_head(package)?);
    }

    let ((schema_members, mut guard_worker, mut operation_worker), _) = source_db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().identity() != preflight.identity
                    || session.selection().contract != selection.contract
                {
                    return Err(crate::DurableError::Conflict(
                        "Sign PG source changed during schema selection",
                    ));
                }
                selected_schema_members(
                    session,
                    invocation,
                    &preflight,
                    &baseline_packages,
                    deadline,
                    cancelled,
                )
                .map_err(crate::DurableError::Source)
            },
        )
        .map_err(super::growth_source_error)?;
    let batch_schema = schema_members
        .get(BATCH_SCHEMA)
        .ok_or(SourceCommandError::Conflict("Sign PG batch schema absent"))?;
    let runtime_check = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime_check.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native PG Sign executable changed",
        ));
    }
    let candidate = cmd::text(owner_config, "promotion_candidate_id")?.to_owned();

    let held = with_private_assessment_and_source_histories_guard_selected_schema(
        &mut history_db,
        &preflight.scope,
        &preflight.subject_targets,
        &preflight.source_targets,
        &expected_heads,
        batch_schema,
        &preflight.identity,
        &mut guard_worker,
        MAX_PACKAGE_HISTORY_ROWS,
        MAX_PACKAGE_HISTORY_BYTES,
        deadline,
        cancelled,
        |histories, _, source_packages, _guarded_worker, held_deadline, held_cancelled| {
            let ((outcome, owner, grant), readset) = source_db
                .with_source_revision_session(
                    &store,
                    &selection.domain,
                    &selection.job_id,
                    selection.job_fence,
                    held_deadline,
                    held_cancelled,
                    |session| {
                        if session.selection().identity() != preflight.identity
                            || session.selection().contract != selection.contract
                        {
                            return Err(crate::DurableError::Conflict(
                                "Sign PG source identity changed under history guard",
                            ));
                        }
                        current_package_members(
                            session,
                            &schema_members,
                            held_deadline,
                            held_cancelled,
                        )
                        .map_err(crate::DurableError::Source)?;
                        let input = PgGrowthCurrentInput::new(
                            session,
                            4096,
                            usize::try_from(max_member_bytes)
                                .unwrap_or(MAX_SCHEMA_BYTES),
                        )
                        .map_err(|_| {
                            crate::DurableError::Source(SourceCommandError::Invalid(
                                "Sign PG selected input budget",
                            ))
                        })?;
                        let context_schema = schema_members.get(CONTEXT_SCHEMA).ok_or(
                            crate::DurableError::Conflict("Sign PG context schema absent"),
                        )?;
                        let (mut owner, selected_context) = OwnerTextContext::select_pg(
                            &assessment_context_path,
                            context_schema,
                            &mut operation_worker,
                            held_deadline,
                            held_cancelled,
                        )
                        .map_err(crate::DurableError::Source)?;
                        owner.install_pg_selected_members(BTreeMap::from([(
                            CONTEXT_SCHEMA.to_owned(),
                            context_schema.clone(),
                        )]))
                        .map_err(crate::DurableError::Source)?;
                        let grant = ProtectedAssessmentPgGrant::select(
                            &context_path,
                            &assessment_context_path,
                            &owner,
                            held_deadline,
                            held_cancelled,
                        )
                        .map_err(crate::DurableError::Source)?;
                        if grant.configuration_raw() != preflight.assessment_config_raw
                            || grant.configuration() != &preflight.assessment_config
                        {
                            return Err(crate::DurableError::Conflict(
                                "Sign PG Assessment owner configuration changed",
                            ));
                        }
                        let mut initial_files = schema_members.clone();
                        merge_member(
                            &mut initial_files,
                            ENTITY_REGISTRY.to_owned(),
                            preflight.entity_registry.clone(),
                        )
                        .map_err(crate::DurableError::Source)?;
                        merge_member(
                            &mut initial_files,
                            RELATION_REGISTRY.to_owned(),
                            preflight.relation_registry.clone(),
                        )
                        .map_err(crate::DurableError::Source)?;
                        let software_files = super::selected_software_files(
                            &software,
                            &components,
                            held_deadline,
                            held_cancelled,
                        )
                        .map_err(crate::DurableError::Source)?;
                        if software_files != expected_software_files {
                            return Err(crate::DurableError::Conflict(
                                "PG Sign selected software changed during preparation",
                            ));
                        }
                        for file in &software_files {
                            merge_member(
                                &mut initial_files,
                                file.path.as_str().to_owned(),
                                file.raw.clone(),
                            )
                            .map_err(crate::DurableError::Source)?;
                        }
                        let base_context = CommandContext {
                            base_revision: preflight.identity.bootstrap_revision,
                            configuration_raw: owner_raw.to_vec(),
                            request_raw: request_raw.to_vec(),
                            recorded_at: crate::source_serialization::instant()
                                .map_err(crate::DurableError::Source)?,
                            effective_uid: u64::from(uid),
                            files: source_files(initial_files)
                                .map_err(crate::DurableError::Source)?,
                        };
                        base_context.check().map_err(crate::DurableError::Source)?;
                        if operation == "prepare" {
                            let target_exists = selected_target_exists(
                                &input,
                                owner_config,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(crate::DurableError::Source)?;
                            let preview = crate::source_creation::sign_pg::selected_information(
                                &base_context,
                                preflight.identity,
                                &mut operation_worker,
                                target_exists,
                                None,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(crate::DurableError::Source)?;
                            drop(input);
                            crate::source_sign::finish_worker(
                                &mut operation_worker,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(crate::DurableError::Source)?;
                            drop(selected_context);
                            owner
                                .snapshot(held_deadline, held_cancelled)
                                .map_err(crate::DurableError::Source)?;
                            grant
                                .verify_current(held_deadline, held_cancelled)
                                .map_err(crate::DurableError::Source)?;
                            return Ok((SignPrepared::Preview(preview), owner, grant));
                        }
                        let private_sources =
                            crate::source_private_assessment_pg_sources::select_owner_local_sources_pg(
                                &mut owner,
                                &selected_context,
                                &preflight.assessment_config,
                                &base_context,
                                &input,
                                source_packages,
                                &mut operation_worker,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(crate::DurableError::Source)?;
                        if private_sources.target_refs != preflight.source_targets {
                            return Err(crate::DurableError::Conflict(
                                "Sign PG private source target set changed",
                            ));
                        }
                        let public_sources =
                            crate::source_sign::pg::select_owner_assessment_public_sources_pg(
                                &input,
                                &preflight.assessment_config,
                                &mut operation_worker,
                                Some(&owner),
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(crate::DurableError::Source)?;
                        private_sources
                            .verify_current(
                                &owner,
                                &input,
                                source_packages,
                                held_deadline,
                                held_cancelled,
                            )
                            .map_err(crate::DurableError::Source)?;
                        public_sources
                            .verify_current(&input, Some(&owner), held_deadline, held_cancelled)
                            .map_err(crate::DurableError::Source)?;
                        let history = histories.get(&candidate).ok_or(
                            crate::DurableError::Conflict(
                                "Sign PG candidate Assessment history absent",
                            ),
                        )?;
                        let owner_snapshot = public_sources.owner_snapshot;
                        let outcome = if let Some(attempt) = recovered.as_ref() {
                            let basis = crate::source_sign::pg::PgSignRecoverableBasis::from_writes(
                                &attempt.writes,
                            )
                            .map_err(crate::DurableError::Source)?;
                            crate::source_sign::pg::verify_recoverable_basis_current(
                                &basis,
                                history,
                                &public_sources,
                                &preflight.assessment_config,
                            )
                            .map_err(crate::DurableError::Source)?;
                            drop(public_sources);
                            drop(private_sources);
                            drop(input);
                            SignPrepared::Replay
                        } else {
                            let (view, _budget) = evaluate_sign_view(
                                    invocation,
                                    &preflight.assessment_config,
                                    &preflight.assessment_config_raw,
                                    &public_sources,
                                    &private_sources,
                                    &mut owner,
                                    &selected_context,
                                    preflight.identity,
                                    &schema_members,
                                    &mut operation_worker,
                                    &candidate,
                                    histories,
                                    operation != "describe",
                                    held_deadline,
                                    held_cancelled,
                                )
                                .map_err(crate::DurableError::Source)?;
                            public_sources
                                .verify_current(
                                    &input,
                                    Some(&owner),
                                    held_deadline,
                                    held_cancelled,
                                )
                                .map_err(crate::DurableError::Source)?;
                            let mut members = schema_members.clone();
                            merge_member(
                                &mut members,
                                ENTITY_REGISTRY.to_owned(),
                                preflight.entity_registry.clone(),
                            )
                            .map_err(crate::DurableError::Source)?;
                            merge_member(
                                &mut members,
                                RELATION_REGISTRY.to_owned(),
                                preflight.relation_registry.clone(),
                            )
                            .map_err(crate::DurableError::Source)?;
                            for (path, raw) in private_sources.dependency_members() {
                                merge_member(&mut members, path, raw)
                                    .map_err(crate::DurableError::Source)?;
                            }
                            for (path, raw) in &public_sources.selected_members {
                                merge_member(&mut members, path.clone(), raw.clone())
                                    .map_err(crate::DurableError::Source)?;
                            }
                            for file in &software_files {
                                merge_member(
                                    &mut members,
                                    file.path.as_str().to_owned(),
                                    file.raw.clone(),
                                )
                                .map_err(crate::DurableError::Source)?;
                            }
                            let context = CommandContext {
                                base_revision: preflight.identity.bootstrap_revision,
                                configuration_raw: owner_raw.to_vec(),
                                request_raw: request_raw.to_vec(),
                                recorded_at: crate::source_serialization::instant()
                                    .map_err(crate::DurableError::Source)?,
                                effective_uid: u64::from(uid),
                                files: source_files(members)
                                    .map_err(crate::DurableError::Source)?,
                            };
                            context.check().map_err(crate::DurableError::Source)?;
                            if operation == "describe" {
                                let target_exists = selected_target_exists(
                                    &input,
                                    owner_config,
                                    held_deadline,
                                    held_cancelled,
                                )
                                .map_err(crate::DurableError::Source)?;
                                let preview =
                                    crate::source_creation::sign_pg::selected_information(
                                        &context,
                                        preflight.identity,
                                        &mut operation_worker,
                                        target_exists,
                                        Some(&view),
                                        held_deadline,
                                        held_cancelled,
                                    )
                                    .map_err(crate::DurableError::Source)?;
                                drop(public_sources);
                                drop(private_sources);
                                drop(input);
                                SignPrepared::Preview(preview)
                            } else {
                            let basis = cmd::field(&view, "basis")
                                .map_err(crate::DurableError::Source)?
                                .clone();
                            let requirements =
                                crate::source_creation::sign_pg::requirements(&context)
                                    .map_err(crate::DurableError::Source)?;
                            let identities =
                                crate::source_sign::pg::observe_sign_creation_absences(
                                    &input,
                                    &requirements.annotation_identity_token,
                                    &requirements.candidate_token,
                                    held_deadline,
                                    held_cancelled,
                                )
                                .map_err(crate::DurableError::Source)?;
                            drop(public_sources);
                            drop(private_sources);
                            drop(input);
                            let home = session
                                .observe_home_absence(
                                    &requirements.home,
                                    held_deadline,
                                    held_cancelled,
                                )
                                .map_err(super::growth_source_error)
                                .map_err(crate::DurableError::Source)?;
                            let unique = requirements
                                .unique_tokens
                                .iter()
                                .map(|item| {
                                    session
                                        .observe_unique_index(
                                            &item.kind,
                                            &item.token,
                                            held_deadline,
                                            held_cancelled,
                                        )
                                        .map_err(super::growth_source_error)
                                        .map_err(crate::DurableError::Source)
                                })
                                .collect::<crate::DurableResult<Vec<_>>>()?;
                            let observations =
                                crate::source_creation::sign_pg::PgSignCreationObservations {
                                    home,
                                    unique,
                                    identities,
                                };
                            let prepared =
                                crate::source_creation::sign_pg::prepare_pg_sign_creation(
                                    context,
                                    preflight.identity,
                                    observations,
                                    basis,
                                    owner_snapshot,
                                    &software,
                                    &components,
                                    &mut operation_worker,
                                    held_deadline,
                                    held_cancelled,
                                )
                                .map_err(crate::DurableError::Source)?;
                            if operation == "prepare-create" {
                                SignPrepared::Preview(
                                    prepared
                                        .preview()
                                        .map_err(crate::DurableError::Source)?,
                                )
                            } else {
                                let serialized = prepared
                                    .serialize(
                                        &software,
                                        &mut operation_worker,
                                        held_deadline,
                                        held_cancelled,
                                    )
                                    .map_err(crate::DurableError::Source)?;
                                let (final_context, _) =
                                    super::context_with_pg_exact_reads(
                                        &serialized.context,
                                        session,
                                        held_deadline,
                                        held_cancelled,
                                    )
                                    .map_err(crate::DurableError::Source)?;
                                super::observe_growth_semantic_indexes(
                                    session,
                                    &serialized.indexes,
                                    held_deadline,
                                    held_cancelled,
                                )?;
                                let binding = super::PgGrowthReadBinding::capture(session);
                                let proposal = final_context
                                    .plan(
                                        &serialized.proposal.handler_id,
                                        serialized.proposal.response.clone(),
                                        serialized.proposal.changes.clone(),
                                        serialized.proposal.replayed,
                                    )
                                    .map_err(crate::DurableError::Source)?;
                                super::check_growth_context_budget(
                                    &final_context,
                                    max_members,
                                    max_total_bytes,
                                    max_member_bytes,
                                )
                                .map_err(crate::DurableError::Source)?;
                                let prepared = super::PreparedPgGrowthRevision::new(
                                    final_context,
                                    proposal,
                                    super::PgGrowthFamily::SignPromotion,
                                    binding,
                                    serialized.indexes,
                                )?;
                                SignPrepared::Mutation(prepared)
                            }
                            }
                        };
                        crate::source_sign::finish_worker(
                            &mut operation_worker,
                            held_deadline,
                            held_cancelled,
                        )
                        .map_err(crate::DurableError::Source)?;
                        drop(selected_context);
                        owner
                            .snapshot(held_deadline, held_cancelled)
                            .map_err(crate::DurableError::Source)?;
                        grant
                            .verify_current(held_deadline, held_cancelled)
                            .map_err(crate::DurableError::Source)?;
                        Ok((outcome, owner, grant))
                    },
                )
                .map_err(|error| error)?;
            match outcome {
                SignPrepared::Mutation(prepared) => {
                    let result = super::growth_value(&prepared.proposal().response)
                        .map_err(crate::DurableError::Source)?;
                    let attempt = commit_db.register_source_revision(
                        &store,
                        &prepared,
                        &readset,
                        &super::growth_prepare_id(
                            &selection.domain,
                            command_id.ok_or(crate::DurableError::Invalid(
                                "PG Sign command identity absent",
                            ))?,
                        ),
                        held_deadline,
                        held_cancelled,
                    )?;
                    let owner_check = || {
                        check_invocation_and_owner(
                            invocation_path,
                            invocation_raw,
                            uid,
                            owner_path,
                            owner_raw,
                            selection,
                            &software,
                            &components,
                            &expected_software_files,
                            expected_executable,
                            &runtime,
                            &owner,
                            &grant,
                            held_deadline,
                            held_cancelled,
                        )
                    };
                    let (receipt, _) = commit_db.commit_source_revision(
                        &store,
                        &attempt,
                        selection.contract,
                        &owner_check,
                        held_deadline,
                        held_cancelled,
                    )?;
                    Ok(json!({
                        "schema_version":"tos_local_pg_growth_result_v1",
                        "result":result,
                        "source_read_work":super::growth_source_read_work(&readset.work),
                        "replayed":receipt.replayed,
                        "durable_commit":super::growth_commit_result(&receipt),
                        "grants_admission":false
                    }))
                }
                SignPrepared::Preview(preview) => {
                    check_invocation_and_owner(
                        invocation_path,
                        invocation_raw,
                        uid,
                        owner_path,
                        owner_raw,
                        selection,
                        &software,
                        &components,
                        &expected_software_files,
                        expected_executable,
                        &runtime,
                        &owner,
                        &grant,
                        held_deadline,
                        held_cancelled,
                    )?;
                    Ok(json!({
                        "schema_version":"tos_local_pg_growth_result_v1",
                        "result":super::growth_value(&preview)
                            .map_err(crate::DurableError::Source)?,
                        "source_read_work":super::growth_source_read_work(&readset.work),
                        "grants_admission":false
                    }))
                }
                SignPrepared::Replay => {
                    let attempt = recovered.as_ref().ok_or(crate::DurableError::Conflict(
                        "PG Sign recovery attempt absent",
                    ))?;
                    let owner_check = || {
                        check_invocation_and_owner(
                            invocation_path,
                            invocation_raw,
                            uid,
                            owner_path,
                            owner_raw,
                            selection,
                            &software,
                            &components,
                            &expected_software_files,
                            expected_executable,
                            &runtime,
                            &owner,
                            &grant,
                            held_deadline,
                            held_cancelled,
                        )
                    };
                    let (receipt, _) = commit_db.commit_source_revision(
                        &store,
                        attempt,
                        selection.contract,
                        &owner_check,
                        held_deadline,
                        held_cancelled,
                    )?;
                    Ok(json!({
                        "schema_version":"tos_local_pg_growth_result_v1",
                        "operation":"sign.promote",
                        "source_read_work":super::growth_source_read_work(&readset.work),
                        "replayed":receipt.replayed,
                        "durable_commit":super::growth_commit_result(&receipt),
                        "grants_admission":false
                    }))
                }
            }
        },
    )
    .map_err(super::growth_source_error)?;
    crate::source_sign::finish_worker(&mut guard_worker, deadline, cancelled)?;
    Ok(held)
}
