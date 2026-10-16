//! Bounded PG preparation for maintained Historical and PublicProfile creates.
//!
//! The owner validates only its exact selected source closure. Global ID
//! absence is supplied separately by the PG identity-index read epoch.

use super::forms;
use super::{
    CreationFamily, PreparedCreation, configuration, historical_claims_with_schema,
    initial_with_candidate_schema, prepare_source_forms,
};
use crate::source_cohort::creation::{self, PgCreationInput};
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, PgSourceHomeObservation, PgSourceUniqueIndexObservation,
    SourceRevisionIdentityClosure,
};
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{
    self as cmd, CommandContext, CommandPlan, SourceCommandError, SourceCommandResult,
};
use crate::source_revisions::{self as revisions, PgRecordRevisionCommandContext};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

pub(crate) const PG_HISTORICAL_V1_FAMILY_TAG: &str =
    "tos-pg-historical-create-v1:tos_local_historical_create_owner_v1:historical.create";
pub(crate) const PG_HISTORICAL_V2_FAMILY_TAG: &str =
    "tos-pg-historical-create-v2:tos_local_historical_create_owner_v2:historical.create";
pub(crate) const PG_PUBLIC_PROFILE_FAMILY_TAG: &str =
    "tos-pg-public-profile-create-v1:tos_local_profile_create_owner_v1:source.create";

const NATIVE_TEXT_CONTENT_SCHEMAS: &[&str] = &[
    "ToS/contracts/source-text-unit-packet-v1.schema.json",
    "ToS/contracts/source-text-layer.schema.json",
    "ToS/contracts/source-anchor-v2.schema.json",
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/owner-local-source-context.schema.json",
    "ToS/contracts/native-pg-private-text-read.schema.json",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgFamilyCreationRequirement {
    pub(crate) kind: String,
    pub(crate) token: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PgFamilyCreationRequirements {
    pub(crate) family: CreationFamily,
    pub(crate) home: RelativePath,
    pub(crate) record_type: String,
    /// Exact authored paths directly named by the request.
    pub(crate) source_refs: Vec<RelativePath>,
    /// Profile and family contract paths known before registry expansion.
    pub(crate) required_source_inputs: Vec<RelativePath>,
    /// IDs that must be absent globally before this creation can proceed.
    pub(crate) unique_tokens: Vec<PgFamilyCreationRequirement>,
    /// Existing identity references whose exact current owner must be selected.
    pub(crate) reference_tokens: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PgFamilyCreationObservations {
    pub(crate) family: CreationFamily,
    pub(crate) record_type: String,
    pub(crate) home: PgSourceHomeObservation,
    pub(crate) unique: Vec<PgSourceUniqueIndexObservation>,
    pub(crate) identity_closure: SourceRevisionIdentityClosure,
    pub(crate) dependency_generation: u64,
}

pub(crate) fn family_tag(family: CreationFamily) -> SourceCommandResult<&'static str> {
    match family {
        CreationFamily::HistoricalV1 => Ok(PG_HISTORICAL_V1_FAMILY_TAG),
        CreationFamily::HistoricalV2 => Ok(PG_HISTORICAL_V2_FAMILY_TAG),
        CreationFamily::PublicProfile => Ok(PG_PUBLIC_PROFILE_FAMILY_TAG),
        _ => Err(SourceCommandError::Unsupported(
            "PG Historical/Profile creation family",
        )),
    }
}

fn base_schema_inputs(family: CreationFamily) -> Vec<&'static str> {
    let mut paths = vec![
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/source-metadata-record.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
    ];
    if family.historical() {
        paths.extend([
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            "ToS/contracts/historical-record.schema.json",
            "ToS/contracts/historical-claim.schema.json",
            "ToS/contracts/claim-packet.schema.json",
            "ToS/contracts/knowledge-assessment.schema.json",
        ]);
        if family == CreationFamily::HistoricalV2 {
            paths.push("ToS/contracts/provenance-event-v2.schema.json");
        }
    }
    paths
}

/// Exact profile route resources are read only after the registry member is
/// selected. Until then this returns the bounded family schema roots needed
/// to safely interpret that registry.
pub(crate) fn required_source_inputs(
    context: &CommandContext,
) -> SourceCommandResult<Vec<RelativePath>> {
    let (family, config, _) = configuration(context)?;
    let mut inputs = base_schema_inputs(family)
        .into_iter()
        .map(|path| {
            RelativePath::parse(path)
                .map_err(|_| SourceCommandError::Invalid("creation schema input"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if family == CreationFamily::PublicProfile {
        let registry_path =
            RelativePath::parse("ToS/doctrine/semantic-interchange/entity-types.v1.json")
                .map_err(|_| SourceCommandError::Invalid("profile registry path"))?;
        let Some(raw) = context.file(&registry_path)? else {
            return Ok(inputs.into_iter().collect());
        };
        let registry = cmd::parse(raw)?;
        let profile_id = cmd::text(&config, "profile_type_id")?;
        let request = cmd::parse(&context.request_raw)?;
        let record = cmd::field(&request, "record")?;
        let schema_version = cmd::text(record, "schema_version")?;
        let entries = cmd::array(&registry, "types")?
            .iter()
            .filter(|entry| cmd::text(entry, "type_id").ok() == Some(profile_id))
            .collect::<Vec<_>>();
        if entries.len() != 1 {
            return Err(SourceCommandError::Conflict(
                "profile type registry identity",
            ));
        }
        let profile = cmd::field(entries[0], "source_record_profile")?;
        let routes = cmd::array(profile, "schemas")?
            .iter()
            .filter(|route| cmd::text(route, "schema_version").ok() == Some(schema_version))
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "profile exact schema version unavailable",
            ));
        }
        for dependency in cmd::array(routes[0], "schema_dependencies")? {
            let value = dependency
                .as_str()
                .ok_or(SourceCommandError::Invalid("profile schema dependency"))?;
            inputs.insert(
                RelativePath::parse(value)
                    .map_err(|_| SourceCommandError::Invalid("profile schema dependency path"))?,
            );
        }
        let root = cmd::text(routes[0], "schema_ref")?;
        inputs.insert(
            RelativePath::parse(root)
                .map_err(|_| SourceCommandError::Invalid("profile schema root path"))?,
        );
        inputs.insert(RelativePath::parse("ToS/contracts/corpus-record.schema.json").unwrap());
        if profile.object_get("native_binding_adapter").is_some() {
            for path in NATIVE_TEXT_CONTENT_SCHEMAS {
                inputs.insert(
                    RelativePath::parse(path)
                        .map_err(|_| SourceCommandError::Invalid("native content schema path"))?,
                );
            }
        }
    }
    if family.historical() {
        let request = cmd::parse(&context.request_raw)?;
        for claim in cmd::array(&request, "claims")? {
            if claim
                .object_get("qualifiers")
                .and_then(|value| value.object_get("display_fields"))
                .and_then(|value| value.object_get("schema_version"))
                .and_then(JsonValue::as_str)
                == Some("tos_claim_display_fields_v1")
            {
                inputs.insert(
                    RelativePath::parse("ToS/contracts/claim-display-fields.schema.json")
                        .map_err(|_| SourceCommandError::Invalid("Claim display schema path"))?,
                );
            }
        }
    }
    Ok(inputs.into_iter().collect())
}

/// Select bounded observations for the two remaining public creation families.
pub(crate) fn requirements(
    context: &CommandContext,
) -> SourceCommandResult<PgFamilyCreationRequirements> {
    context.check()?;
    let (family, config, home) = configuration(context)?;
    if !matches!(
        family,
        CreationFamily::HistoricalV1 | CreationFamily::HistoricalV2 | CreationFamily::PublicProfile
    ) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Historical/Profile creation owner",
        ));
    }
    let request = cmd::parse(&context.request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let mut keys = vec!["schema_version", "operation", "record", "forms"];
    if family.historical() {
        keys.push("claims");
    }
    if operation == family.operation() {
        keys.extend([
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]);
    } else if operation != "prepare-create" {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Historical/Profile creation operation",
        ));
    }
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !cmd::array(&config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == Some(family.operation()))
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Historical/Profile operation grant",
        ));
    }
    let record = cmd::field(&request, "record")?;
    let record_type = cmd::text(record, "record_type")?.to_owned();
    let source_path = cmd::text(&config, "source_path")?;
    if family.historical() && source_path != format!("{}/{record_type}.json", home.as_str())
        || !family.historical()
            && !source_path
                .strip_prefix(&format!("{}/", home.as_str()))
                .is_some_and(|name| {
                    !name.is_empty() && !name.contains('/') && name.ends_with(".json")
                })
    {
        return Err(SourceCommandError::Denied(
            "PG Historical/Profile source home is not exact",
        ));
    }
    let mut paths = BTreeSet::new();
    for reference in cmd::array(record, "source_refs")? {
        let value = reference
            .as_str()
            .ok_or(SourceCommandError::Invalid("creation source reference"))?;
        if value.starts_with("ToS/") {
            if value
                .split('/')
                .any(|part| ["owner-local", "payload", "local-content"].contains(&part))
            {
                return Err(SourceCommandError::Denied(
                    "creation source reference addresses private content",
                ));
            }
            paths.insert(
                RelativePath::parse(value)
                    .map_err(|_| SourceCommandError::Invalid("creation source reference path"))?,
            );
        }
    }
    if let Some(binding) = record.object_get("native_text_binding") {
        let refs = cmd::field(binding, "source_record_refs")?;
        let packet_ref = cmd::text(binding, "packet_ref")?.to_owned();
        let layer_ref = cmd::text(cmd::field(binding, "text_layer")?, "record_ref")?.to_owned();
        for (value, private_package_allowed) in [
            (packet_ref, true),
            (layer_ref, true),
            (cmd::text(refs, "work")?.to_owned(), false),
            (cmd::text(refs, "expression")?.to_owned(), false),
            (cmd::text(refs, "edition")?.to_owned(), false),
            (cmd::text(refs, "item")?.to_owned(), false),
        ] {
            if !value.starts_with("ToS/source-witnesses/")
                || value.split('/').any(|part| {
                    ["payload", "local-content", "catalog"].contains(&part)
                        || part == "owner-local" && !private_package_allowed
                })
                || value.contains("/owner-local/")
                    && (!private_package_allowed
                        || !value.starts_with("ToS/source-witnesses/owner-local/"))
            {
                return Err(SourceCommandError::Denied(
                    "PG creation native binding addresses an unselected private source member",
                ));
            }
            let path = RelativePath::parse(&value)
                .map_err(|_| SourceCommandError::Invalid("native binding reference path"))?;
            if !value.starts_with("ToS/source-witnesses/owner-local/") {
                paths.insert(path);
            }
        }
    }
    let mut unique = BTreeSet::new();
    unique.insert((
        "metadata".to_owned(),
        cmd::text(record, "record_id")?.to_owned(),
    ));
    if family != CreationFamily::HistoricalV1 {
        unique.insert((
            "event".to_owned(),
            cmd::text(&config, "provenance_event_id")?.to_owned(),
        ));
    }
    let forms = cmd::array(&request, "forms")?;
    if forms.is_empty() || forms.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "creation form selections budget",
        ));
    }
    for selection in forms {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        unique.insert((
            "form".to_owned(),
            cmd::text(selection, "form_id")?.to_owned(),
        ));
    }
    let mut references = BTreeSet::new();
    let record_id = cmd::text(record, "record_id")?;
    collect_claim_dependencies(
        record,
        record_id,
        config
            .object_get("provenance_event_id")
            .and_then(JsonValue::as_str),
        &BTreeSet::new(),
        family == CreationFamily::PublicProfile,
        &mut references,
        &mut paths,
    );
    if family.historical() {
        let claims = cmd::array(&request, "claims")?;
        if claims.len() > 32 {
            return Err(SourceCommandError::Invalid(
                "initial historical Claims budget",
            ));
        }
        let new_claim_ids = claims
            .iter()
            .map(|claim| cmd::text(claim, "claim_id").map(str::to_owned))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        let record_id = cmd::text(record, "record_id")?;
        let new_event_id = config
            .object_get("provenance_event_id")
            .and_then(JsonValue::as_str);
        for claim in claims {
            let claim_id = cmd::text(claim, "claim_id")?.to_owned();
            unique.insert(("claim".to_owned(), claim_id));
            collect_claim_dependencies(
                claim,
                record_id,
                new_event_id,
                &new_claim_ids,
                false,
                &mut references,
                &mut paths,
            );
        }
    }
    let claim_count = if family.historical() {
        cmd::array(&request, "claims")?.len()
    } else {
        0
    };
    let expected_unique_count =
        forms.len() + claim_count + usize::from(family != CreationFamily::HistoricalV1) + 1;
    if unique.len() != expected_unique_count {
        return Err(SourceCommandError::Invalid(
            "duplicate PG creation identity token",
        ));
    }
    Ok(PgFamilyCreationRequirements {
        family,
        home,
        record_type,
        source_refs: paths.into_iter().collect(),
        required_source_inputs: required_source_inputs(context)?,
        unique_tokens: unique
            .into_iter()
            .map(|(kind, token)| PgFamilyCreationRequirement { kind, token })
            .collect(),
        reference_tokens: references,
    })
}

fn collect_claim_dependencies(
    claim: &JsonValue,
    new_record_id: &str,
    new_event_id: Option<&str>,
    new_claim_ids: &BTreeSet<String>,
    separate_native_binding: bool,
    tokens: &mut BTreeSet<String>,
    paths: &mut BTreeSet<RelativePath>,
) {
    fn visit(
        value: &JsonValue,
        key: Option<&str>,
        new_record_id: &str,
        new_event_id: Option<&str>,
        new_claim_ids: &BTreeSet<String>,
        separate_native_binding: bool,
        tokens: &mut BTreeSet<String>,
        paths: &mut BTreeSet<RelativePath>,
    ) {
        match value {
            JsonValue::String(text) => {
                let Some(text) = text.as_str() else {
                    return;
                };
                if key.is_some_and(|key| key == "evidence_refs" || key == "counterevidence_refs")
                    && text.starts_with("ToS/")
                {
                    if let Ok(path) = RelativePath::parse(text) {
                        paths.insert(path);
                    }
                } else if key.is_some_and(|key| key.ends_with("_ref") || key.ends_with("_refs"))
                    && text.starts_with("tos.")
                {
                    let own_record = key == Some("subject_ref") && text == new_record_id;
                    let own_event =
                        key == Some("provenance_event_ref") && Some(text) == new_event_id;
                    if !own_record && !own_event && !new_claim_ids.contains(text) {
                        tokens.insert(text.to_owned());
                    }
                }
            }
            JsonValue::Array(values) => {
                for value in values {
                    visit(
                        value,
                        key,
                        new_record_id,
                        new_event_id,
                        new_claim_ids,
                        false,
                        tokens,
                        paths,
                    );
                }
            }
            JsonValue::Object(values) => {
                for (name, value) in values {
                    if let Some(name) = name.as_str() {
                        // Root native bindings have their own exact packet,
                        // anchor and rights verification. Private anchors are
                        // not members of the public source identity index.
                        if separate_native_binding && name == "native_text_binding" {
                            continue;
                        }
                        visit(
                            value,
                            Some(name),
                            new_record_id,
                            new_event_id,
                            new_claim_ids,
                            false,
                            tokens,
                            paths,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    visit(
        claim,
        None,
        new_record_id,
        new_event_id,
        new_claim_ids,
        separate_native_binding,
        tokens,
        paths,
    );
}

/// Owner preparation over the exact source members selected by PG. No full
/// inventory projection is built; global absence and reference ownership
/// remain bound by the point-index predicates carried in `observations`.
pub(crate) fn prepare_pg_family_creation(
    context: CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: PgFamilyCreationObservations,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    replay_binding: Digest256,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    native_reader: Option<&mut dyn crate::source_sign_native::SignNativeRead>,
    private_selection: Option<&crate::source_native_text_read::LocalTextReadSelection>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgFamilyCreation> {
    if context.base_revision != identity.bootstrap_revision
        || worker.input_identity() != &identity
        || components.capture() != software.selection()
        || observations.identity_closure.selection_identity() != identity
    {
        return Err(SourceCommandError::Conflict(
            "PG Historical/Profile source epoch or software selection differs",
        ));
    }
    context.check()?;
    let requirements = requirements(&context)?;
    for path in &requirements.required_source_inputs {
        if context.file(path)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "required selected Historical/Profile creation bytes absent",
            ));
        }
    }
    for path in &requirements.source_refs {
        if path.as_str().starts_with("ToS/") && context.file(path)?.is_none() {
            return Err(SourceCommandError::Conflict(
                "Historical/Profile exact bound source member absent",
            ));
        }
    }
    if observations.family != requirements.family
        || observations.record_type != requirements.record_type
        || observations.home.parent != requirements.home
        || !observations.home.children.is_empty()
        || !observations_match(&context, &requirements, &observations)?
    {
        return Err(SourceCommandError::Conflict(
            "PG Historical/Profile source home or identity observations differ",
        ));
    }
    for token in &requirements.reference_tokens {
        let owner = observations
            .identity_closure
            .owners()
            .get(token)
            .and_then(Option::as_ref)
            .ok_or(SourceCommandError::Conflict(
                "Historical/Profile source identity reference is unresolved",
            ))?;
        let path = &owner.1;
        if context.file(path)?.is_none() {
            return Err(SourceCommandError::Conflict(
                "Historical/Profile identity owner member was not selected",
            ));
        }
        let selected = selected_identity_paths(&context, token)?;
        if selected != BTreeSet::from([path.clone()]) {
            return Err(SourceCommandError::Conflict(
                "Historical/Profile point identity owner differs from selected source bytes",
            ));
        }
    }

    let (family, config, home) = configuration(&context)?;
    if home != requirements.home {
        return Err(SourceCommandError::Conflict(
            "PG Historical/Profile home changed during preparation",
        ));
    }
    let schema_context = PgRecordRevisionCommandContext::new(context.clone(), identity);
    revisions::validate_source_profile_registry_candidate(
        worker,
        deadline,
        cancelled,
        &schema_context,
    )?;
    let request = cmd::parse(&cmd::canonical(&cmd::parse(&context.request_raw)?)?)?;
    let operation = cmd::text(&request, "operation")?;
    if !["prepare-create", family.operation()].contains(&operation) {
        return Err(SourceCommandError::Invalid(
            "Historical/Profile creation package operation",
        ));
    }
    let mut keys = vec!["schema_version", "operation", "record", "forms"];
    if family.historical() {
        keys.push("claims");
    }
    if operation == family.operation() {
        keys.extend([
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]);
    }
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !cmd::array(&config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == Some(family.operation()))
    {
        return Err(SourceCommandError::Denied(
            "Historical/Profile creation operation not delegated",
        ));
    }
    let record = cmd::field(&request, "record")?;
    let source_path = cmd::text(&config, "source_path")?;
    let source_name = source_path
        .strip_prefix(&format!("{}/", home.as_str()))
        .filter(|name| !name.is_empty() && !name.contains('/') && name.ends_with(".json"));
    let expected_historical_name = format!("{}.json", requirements.record_type);
    if cmd::text(record, "record_type")? != requirements.record_type
        || source_name.is_none()
        || family.historical() && source_name != Some(expected_historical_name.as_str())
    {
        return Err(SourceCommandError::Denied(
            "PG Historical/Profile source path or record type is not exact",
        ));
    }
    initial_with_candidate_schema(
        &context,
        identity,
        worker,
        &config,
        family,
        record,
        native_reader,
        private_selection,
        deadline,
        cancelled,
    )?;
    let selections = cmd::array(&request, "forms")?;
    let mut validate_forms = |refs: &[String], root: &str, instance: &JsonValue| {
        revisions::schema_candidate(
            worker,
            deadline,
            cancelled,
            &schema_context,
            refs,
            root,
            instance,
        )
    };
    let forms = prepare_source_forms(&config, record, selections, &mut validate_forms)?;
    let mut files = BTreeMap::from([
        (
            cmd::text(&config, "source_path")?
                .rsplit('/')
                .next()
                .ok_or(SourceCommandError::Invalid("creation source filename"))?
                .to_owned(),
            cmd::published(record)?,
        ),
        (
            format!(
                "{}.human-forms.json",
                source_name
                    .and_then(|name| name.strip_suffix(".json"))
                    .ok_or(SourceCommandError::Invalid("creation source basename"))?
            ),
            cmd::published(&forms.set)?,
        ),
    ]);
    if family.historical() {
        let authored_members = context
            .files
            .iter()
            .filter(|file| file.path.as_str().starts_with("ToS/"))
            .cloned()
            .collect::<Vec<_>>();
        let closure = crate::source_claims::maintained_selected_identity_closure_candidate(
            &schema_context,
            &authored_members,
            worker,
            deadline,
            cancelled,
        )?;
        let mut evidence = Vec::new();
        let mut validate_claim_schema = |refs: &[String], root: &str, instance: &JsonValue| {
            revisions::schema_candidate(
                worker,
                deadline,
                cancelled,
                &schema_context,
                refs,
                root,
                instance,
            )
        };
        let rows = cmd::array(&request, "claims")?;
        let claim_bytes = historical_claims_with_schema(
            &context,
            &config,
            record,
            rows,
            &closure.objects,
            &closure.claims,
            &closure.events,
            &closure.anchors,
            &mut validate_claim_schema,
            &mut evidence,
        )?;
        files.insert("historical-claims.jsonl".into(), claim_bytes);
    }
    let dependency_commitment = family_dependency_commitment(
        &context,
        identity,
        &observations,
        &config,
        &request,
        components,
    )?;
    let prepared = PreparedCreation {
        context,
        managed_basis: None,
        managed_observations: None,
        managed_inventory: None,
        family,
        home,
        subject: forms.subject,
        dependencies: dependency_commitment.to_prefixed(),
        files,
        components: components.clone(),
    };
    Ok(PreparedPgFamilyCreation {
        prepared,
        identity,
        observations,
        family_tag: family_tag(family)?.to_owned(),
        record_type: requirements.record_type,
        dependency_commitment,
        replay_binding,
    })
}

fn observations_match(
    context: &CommandContext,
    required: &PgFamilyCreationRequirements,
    observations: &PgFamilyCreationObservations,
) -> SourceCommandResult<bool> {
    let expected = required
        .unique_tokens
        .iter()
        .map(|item| (item.kind.clone(), item.token.clone()))
        .collect::<BTreeSet<_>>();
    let actual_references = observations
        .identity_closure
        .owners()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual = observations
        .unique
        .iter()
        .map(|item| (item.kind.clone(), item.token.clone()))
        .collect::<BTreeSet<_>>();
    if actual != expected
        || observations.unique.len() != expected.len()
        || observations.unique.iter().any(|item| !item.rows.is_empty())
        || actual_references != required.reference_tokens
        || observations
            .identity_closure
            .selection_identity()
            .bootstrap_revision
            != context.base_revision
    {
        return Ok(false);
    }
    let fresh = requirements(context)?;
    Ok(fresh.family == required.family
        && fresh.home == required.home
        && fresh.record_type == required.record_type
        && fresh.source_refs == required.source_refs
        && fresh.unique_tokens == required.unique_tokens
        && fresh.reference_tokens == required.reference_tokens)
}

fn selected_identity_paths(
    context: &CommandContext,
    token: &str,
) -> SourceCommandResult<BTreeSet<RelativePath>> {
    fn has_identity(value: &JsonValue, token: &str) -> bool {
        match value {
            JsonValue::Object(fields) => fields.iter().any(|(name, value)| {
                (name.as_str().is_some_and(|name| name.ends_with("_id"))
                    && value.as_str() == Some(token))
                    || has_identity(value, token)
            }),
            JsonValue::Array(values) => values.iter().any(|value| has_identity(value, token)),
            _ => false,
        }
    }
    let mut paths = BTreeSet::new();
    for file in context
        .files
        .iter()
        .filter(|file| file.path.as_str().starts_with("ToS/"))
    {
        let path = file.path.as_str();
        if path.ends_with(".jsonl") {
            for line in file.raw.split(|byte| *byte == b'\n') {
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                let value = cmd::parse(line)?;
                if has_identity(&value, token) {
                    paths.insert(file.path.clone());
                }
            }
        } else if path.ends_with(".json") {
            let value = cmd::parse(&file.raw)?;
            if has_identity(&value, token) {
                paths.insert(file.path.clone());
            }
        }
    }
    Ok(paths)
}

fn family_dependency_commitment(
    context: &CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: &PgFamilyCreationObservations,
    config: &JsonValue,
    request: &JsonValue,
    components: &SoftwareComponentSelectionV1,
) -> SourceCommandResult<Digest256> {
    let mut inputs = BTreeMap::new();
    for file in &context.files {
        if inputs
            .insert(
                file.path.as_str().to_owned(),
                (file.raw.len(), Digest256::of_bytes(&file.raw)),
            )
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate PG Historical/Profile selected input path",
            ));
        }
    }
    let selected_inputs = inputs
        .into_iter()
        .map(|(path, (bytes, digest))| {
            cmd::object(vec![
                ("path", cmd::string(&path)),
                ("bytes", cmd::number(bytes as u64)),
                ("sha256", cmd::string(&digest.to_prefixed())),
            ])
        })
        .collect();
    let capture = components.capture();
    let software_members = components
        .members()
        .map(|member| {
            cmd::object(vec![
                ("path", cmd::string(member.path.as_str())),
                ("bytes", cmd::number(member.size_bytes)),
                ("sha256", cmd::string(&member.sha256.to_prefixed())),
            ])
        })
        .collect();
    let unique_observations = observations
        .unique
        .iter()
        .map(|item| {
            cmd::object(vec![
                ("kind", cmd::string(&item.kind)),
                ("token", cmd::string(&item.token)),
                ("generation", cmd::number(item.generation)),
            ])
        })
        .collect();
    let requirements = requirements(context)?;
    // Preparation and commit carry the same proposal. Commit-only receipt
    // expectations cannot enter the value they are required to equal.
    let mut proposal = vec![
        ("record", cmd::field(request, "record")?.clone()),
        ("forms", cmd::field(request, "forms")?.clone()),
    ];
    if requirements.family.historical() {
        proposal.push(("claims", cmd::field(request, "claims")?.clone()));
    }
    let binding = cmd::object(vec![
        (
            "profile",
            cmd::string("tos_initial_historical_profile_dependency_v2"),
        ),
        ("family_tag", cmd::string(family_tag(requirements.family)?)),
        ("identity", cmd::string(&identity.digest().to_prefixed())),
        (
            "proposal_sha256",
            cmd::string(&cmd::record_digest(&cmd::object(proposal))?.to_prefixed()),
        ),
        (
            "configuration_sha256",
            cmd::string(&cmd::record_digest(config)?.to_prefixed()),
        ),
        ("selected_inputs", JsonValue::Array(selected_inputs)),
        (
            "software_capture",
            cmd::object(vec![
                ("source_git_commit", cmd::string(&capture.source_git_commit)),
                ("source_git_tree", cmd::string(&capture.source_git_tree)),
                (
                    "capture_manifest_sha256",
                    cmd::string(&capture.capture_manifest_sha256.to_prefixed()),
                ),
            ]),
        ),
        ("software_components", JsonValue::Array(software_members)),
        (
            "home",
            cmd::object(vec![
                ("path", cmd::string(requirements.home.as_str())),
                ("generation", cmd::number(observations.home.generation)),
                (
                    "children",
                    cmd::number(observations.home.children.len() as u64),
                ),
            ]),
        ),
        ("unique_observations", JsonValue::Array(unique_observations)),
        (
            "reference_closure",
            cmd::string(&observations.identity_closure.closure_root().to_prefixed()),
        ),
        (
            "dependency_generation",
            cmd::number(observations.dependency_generation),
        ),
    ]);
    cmd::record_digest(&binding)
}

pub(crate) struct PreparedPgFamilyCreation {
    prepared: PreparedCreation,
    identity: PgRecordRevisionIdentity,
    observations: PgFamilyCreationObservations,
    family_tag: String,
    record_type: String,
    dependency_commitment: Digest256,
    replay_binding: Digest256,
}

impl PreparedPgFamilyCreation {
    pub(crate) fn preview(&self) -> SourceCommandResult<JsonValue> {
        self.prepared.preview()
    }

    pub(crate) fn serialize(
        self,
        software: &SoftwareCaptureReader,
        worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PgSerializedFamilyCreation> {
        if worker.input_identity() != &self.identity {
            return Err(SourceCommandError::Conflict(
                "PG Historical/Profile schema worker identity changed",
            ));
        }
        let schema_context =
            PgRecordRevisionCommandContext::new(self.prepared.context().clone(), self.identity);
        let components = self.prepared.components().clone();
        let (prepared, plan) = self.prepared.serialize_content_with_schema(
            software,
            &components,
            deadline,
            cancelled,
            None,
            |_, refs, root, instance| {
                revisions::schema_candidate(
                    worker,
                    deadline,
                    cancelled,
                    &schema_context,
                    refs,
                    root,
                    instance,
                )
            },
        )?;
        let family = prepared.family();
        if plan.operation != family.operation()
            || plan.handler_id != family.handler_id()
            || family_tag(family)? != self.family_tag
        {
            return Err(SourceCommandError::Conflict(
                "PG Historical/Profile serializer family changed",
            ));
        }
        let config = cmd::parse(&prepared.context.configuration_raw)?;
        let source_path = cmd::text(&config, "source_path")?;
        let record_name = source_path
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Invalid("creation source filename"))?;
        let expected_files = if family == CreationFamily::HistoricalV1 {
            BTreeSet::from([
                record_name.to_owned(),
                format!("{}.human-forms.json", record_name.trim_end_matches(".json")),
                "historical-claims.jsonl".to_owned(),
                "source-create-receipt.json".to_owned(),
            ])
        } else if family.historical() {
            BTreeSet::from([
                record_name.to_owned(),
                format!("{}.human-forms.json", record_name.trim_end_matches(".json")),
                "historical-claims.jsonl".to_owned(),
                "source-create-request.json".to_owned(),
                "source-create-environment.json".to_owned(),
                "source-create-provenance.jsonl".to_owned(),
                "source-create-receipt.json".to_owned(),
            ])
        } else {
            BTreeSet::from([
                record_name.to_owned(),
                format!("{}.human-forms.json", record_name.trim_end_matches(".json")),
                "source-create-request.json".to_owned(),
                "source-create-environment.json".to_owned(),
                "source-create-provenance.jsonl".to_owned(),
                "source-create-receipt.json".to_owned(),
            ])
        };
        if prepared.files.keys().cloned().collect::<BTreeSet<_>>() != expected_files {
            return Err(SourceCommandError::Conflict(
                "PG Historical/Profile serialized package closure changed",
            ));
        }
        let home = prepared.home.clone();
        let writes = prepared
            .files
            .iter()
            .map(|(name, raw)| SourceRevisionWrite {
                subject: format!("{}/{}", home.as_str(), name),
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: raw.clone(),
            })
            .collect::<Vec<_>>();
        let indexes =
            recovery_package_contract(&self.family_tag, &self.record_type, home.as_str(), &writes)?;
        let dependency_commitment = creation::PgDependencyCommitment::new(
            creation::COMMITMENT_VERSION,
            self.dependency_commitment,
        )
        .map_err(|_| SourceCommandError::Invalid("PG Historical/Profile dependency commitment"))?;
        Ok(PgSerializedFamilyCreation {
            context: prepared.context.clone(),
            plan,
            family_tag: self.family_tag,
            record_type: self.record_type,
            home,
            files: prepared.files.clone(),
            indexes,
            dependency_commitment,
            identity: self.identity,
            observations: self.observations,
            replay_binding: self.replay_binding,
        })
    }
}

pub(crate) struct PgSerializedFamilyCreation {
    context: CommandContext,
    plan: CommandPlan,
    family_tag: String,
    record_type: String,
    home: RelativePath,
    files: BTreeMap<String, Vec<u8>>,
    indexes: IndexRows,
    dependency_commitment: creation::PgDependencyCommitment,
    replay_binding: Digest256,
    identity: PgRecordRevisionIdentity,
    observations: PgFamilyCreationObservations,
}

impl PgCreationInput for PgSerializedFamilyCreation {
    fn context(&self) -> &CommandContext {
        &self.context
    }
    fn plan(&self) -> &CommandPlan {
        &self.plan
    }
    fn family_tag(&self) -> &str {
        &self.family_tag
    }
    fn replay_binding(&self) -> Option<Digest256> {
        Some(self.replay_binding)
    }
    fn record_type(&self) -> &str {
        &self.record_type
    }
    fn home(&self) -> &RelativePath {
        &self.home
    }
    fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
    fn indexes(&self) -> &IndexRows {
        &self.indexes
    }
    fn dependency_commitment(&self) -> creation::PgDependencyCommitment {
        self.dependency_commitment
    }
    fn identity(&self) -> PgRecordRevisionIdentity {
        self.identity
    }
    fn home_observation(&self) -> &PgSourceHomeObservation {
        &self.observations.home
    }
    fn unique_observations(&self) -> &[PgSourceUniqueIndexObservation] {
        &self.observations.unique
    }
}

/// Derive owner identity indexes from exact staged bytes. The shared writer
/// calls this during both fresh registration and recovery, so package closure
/// and projection admission use one owner implementation.
pub(crate) fn recovery_package_contract(
    family_tag: &str,
    record_type: &str,
    home: &str,
    writes: &[SourceRevisionWrite],
) -> SourceCommandResult<IndexRows> {
    let family = match family_tag {
        PG_HISTORICAL_V1_FAMILY_TAG => CreationFamily::HistoricalV1,
        PG_HISTORICAL_V2_FAMILY_TAG => CreationFamily::HistoricalV2,
        PG_PUBLIC_PROFILE_FAMILY_TAG => CreationFamily::PublicProfile,
        _ => return Err(SourceCommandError::Denied("Historical/Profile family tag")),
    };
    let home =
        RelativePath::parse(home).map_err(|_| SourceCommandError::Invalid("creation home"))?;
    if family.historical()
        && !["historical-event", "historical-process", "historical-state"].contains(&record_type)
        || record_type.is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "Historical/Profile record type",
        ));
    }
    if writes.is_empty() || writes.len() > 40 {
        return Err(SourceCommandError::Invalid(
            "Historical/Profile package member count",
        ));
    }
    let prefix = format!("{}/", home.as_str());
    let mut members = BTreeMap::new();
    for write in writes {
        if write.expected_predecessor.is_some() || write.proposed_revision != 1 {
            return Err(SourceCommandError::Conflict(
                "initial creation member revision",
            ));
        }
        let name = write
            .subject
            .strip_prefix(&prefix)
            .filter(|name| !name.is_empty() && !name.contains('/'))
            .ok_or(SourceCommandError::Conflict(
                "creation package member escapes home",
            ))?;
        if members
            .insert(name.to_owned(), write.exact_bytes.as_slice())
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "duplicate creation package member",
            ));
        }
    }
    let receipt_raw = *members
        .get("source-create-receipt.json")
        .ok_or(SourceCommandError::Conflict("creation receipt absent"))?;
    let receipt = cmd::parse(receipt_raw)?;
    let source_path = cmd::text(&receipt, "source_path")?;
    let source_name = source_path
        .strip_prefix(&prefix)
        .filter(|name| !name.is_empty() && !name.contains('/') && name.ends_with(".json"))
        .ok_or(SourceCommandError::Conflict(
            "creation source path escapes home",
        ))?;
    if family.historical() && source_name != format!("{record_type}.json") {
        return Err(SourceCommandError::Conflict(
            "Historical source basename differs",
        ));
    }
    let forms_name = format!("{}.human-forms.json", source_name.trim_end_matches(".json"));
    let expected_members = if family == CreationFamily::HistoricalV1 {
        BTreeSet::from([
            source_name.to_owned(),
            forms_name.clone(),
            "historical-claims.jsonl".to_owned(),
            "source-create-receipt.json".to_owned(),
        ])
    } else if family.historical() {
        BTreeSet::from([
            source_name.to_owned(),
            forms_name.clone(),
            "historical-claims.jsonl".to_owned(),
            "source-create-request.json".to_owned(),
            "source-create-environment.json".to_owned(),
            "source-create-provenance.jsonl".to_owned(),
            "source-create-receipt.json".to_owned(),
        ])
    } else {
        BTreeSet::from([
            source_name.to_owned(),
            forms_name.clone(),
            "source-create-request.json".to_owned(),
            "source-create-environment.json".to_owned(),
            "source-create-provenance.jsonl".to_owned(),
            "source-create-receipt.json".to_owned(),
        ])
    };
    if members.keys().cloned().collect::<BTreeSet<_>>() != expected_members {
        return Err(SourceCommandError::Conflict(
            "Historical/Profile package closure differs",
        ));
    }
    let metadata_raw = members[source_name];
    let metadata = cmd::parse(metadata_raw)?;
    let record_id = cmd::text(&metadata, "record_id")?;
    let record_id_prefix = if family.historical() {
        format!("tos.{record_type}.")
    } else {
        "tos.".to_owned()
    };
    if cmd::text(&metadata, "record_type")? != record_type
        || cmd::integer(&metadata, "record_version")? != 1
        || cmd::text(&metadata, "identity_status")? != "provisional"
        || cmd::text(&metadata, "same_as_posture")? != "no_equivalence_claim"
        || !revisions::valid_id(record_id, &record_id_prefix, false)
        || cmd::published(&metadata)? != metadata_raw
    {
        return Err(SourceCommandError::Conflict(
            "Historical/Profile source identity differs",
        ));
    }
    let expected_receipt_schema = if family.historical() {
        "tos_local_historical_create_receipt_v1"
    } else {
        "tos_local_source_create_receipt_v1"
    };
    let mut receipt_keys = vec![
        "schema_version",
        "command_id",
        "request_digest",
        "principal_id",
        "authority_ref",
        "owner_configuration",
        "recorded_at",
        "source_path",
        "source",
        "dependencies",
        "files",
        "grants_admission",
    ];
    receipt_keys.sort_unstable();
    let mut observed_keys = receipt
        .as_object()
        .ok_or(SourceCommandError::Invalid("creation receipt object"))?
        .iter()
        .map(|(key, _)| key.as_str().unwrap_or(""))
        .collect::<Vec<_>>();
    observed_keys.sort_unstable();
    if observed_keys != receipt_keys
        || cmd::text(&receipt, "schema_version")? != expected_receipt_schema
        || cmd::text(&receipt, "source_path")? != source_path
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
        || cmd::text(&receipt, "command_id")?.is_empty()
        || cmd::field(&receipt, "source")?
            .object_get("id")
            .and_then(JsonValue::as_str)
            != Some(record_id)
        || cmd::field(&receipt, "source")?
            .object_get("version")
            .and_then(JsonValue::as_u64)
            != Some(1)
        || cmd::field(&receipt, "source")?
            .object_get("digest")
            .and_then(JsonValue::as_str)
            != Some(cmd::record_digest(&metadata)?.to_prefixed().as_str())
    {
        return Err(SourceCommandError::Conflict(
            "creation receipt binding differs",
        ));
    }
    validate_receipt_files(&receipt, &members)?;
    let forms_raw = members[forms_name.as_str()];
    let forms_set = cmd::parse(forms_raw)?;
    if cmd::published(&forms_set)? != forms_raw
        || !forms_set
            .object_get("prior_forms")
            .and_then(JsonValue::as_array)
            .is_some_and(|values| values.is_empty())
        || forms::apply_form_changes(Some(&forms_set), cmd::field(&forms_set, "subject")?, &[])
            .is_err()
        || cmd::field(&forms_set, "subject")? != &forms::metadata_subject(&metadata)?
    {
        return Err(SourceCommandError::Conflict(
            "creation form set binding differs",
        ));
    }
    let views = forms::materialize_source_forms(&metadata, &forms_set)?;
    if views.is_empty()
        || !views
            .iter()
            .any(|view| cmd::text(view, "role").ok() == Some("name"))
        || views
            .iter()
            .any(|view| cmd::text(view, "state").ok() != Some("ready"))
    {
        return Err(SourceCommandError::Conflict(
            "creation form set is not ready",
        ));
    }
    let mut indexes = IndexRows::new();
    indexes.insert((
        "metadata".to_owned(),
        record_id.to_owned(),
        source_path.to_owned(),
    ));
    let form_ids = cmd::array(&forms_set, "forms")?
        .iter()
        .map(|form| cmd::text(form, "form_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if form_ids.is_empty() || form_ids.len() != cmd::array(&forms_set, "forms")?.len() {
        return Err(SourceCommandError::Conflict(
            "creation form identity set differs",
        ));
    }
    for form_id in form_ids {
        indexes.insert((
            "form".to_owned(),
            form_id,
            format!("{}/{}", home.as_str(), forms_name),
        ));
    }
    if family.historical() {
        let claim_raw = members["historical-claims.jsonl"];
        let claim_ids = validate_historical_claim_stream(claim_raw, record_id, family, &members)?;
        for claim_id in claim_ids {
            indexes.insert((
                "claim".to_owned(),
                claim_id,
                format!("{}/historical-claims.jsonl", home.as_str()),
            ));
        }
    }
    if family != CreationFamily::HistoricalV1 {
        let event_path = format!("{}/source-create-provenance.jsonl", home.as_str());
        let event = cmd::parse(members["source-create-provenance.jsonl"])?;
        let event_id = cmd::text(&event, "event_id")?.to_owned();
        validate_creation_event(&event, &event_id, &home, &members)?;
        if family.historical() {
            for line in members["historical-claims.jsonl"]
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
            {
                let claim = cmd::parse(line)?;
                if cmd::text(&claim, "provenance_event_ref")? != event_id {
                    return Err(SourceCommandError::Conflict(
                        "Historical Claim event binding differs",
                    ));
                }
            }
        }
        indexes.insert(("event".to_owned(), event_id, event_path));
    }
    Ok(indexes)
}

fn validate_receipt_files(
    receipt: &JsonValue,
    members: &BTreeMap<String, &[u8]>,
) -> SourceCommandResult<()> {
    let files = cmd::field(receipt, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("creation receipt file map"))?;
    if files.len() + 1 != members.len()
        || files
            .iter()
            .any(|(name, _)| name.as_str() == Some("source-create-receipt.json"))
    {
        return Err(SourceCommandError::Conflict(
            "creation receipt package member set differs",
        ));
    }
    for (name, reference) in files {
        let name = name
            .as_str()
            .ok_or(SourceCommandError::Invalid("creation receipt filename"))?;
        cmd::exact_keys(reference, &["sha256", "bytes"])?;
        let raw = members.get(name).ok_or(SourceCommandError::Conflict(
            "creation receipt member absent",
        ))?;
        if cmd::integer(reference, "bytes")? != raw.len() as u64
            || cmd::text(reference, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
        {
            return Err(SourceCommandError::Conflict(
                "creation receipt member fixity differs",
            ));
        }
    }
    Ok(())
}

fn validate_creation_event(
    event: &JsonValue,
    event_id: &str,
    home: &RelativePath,
    members: &BTreeMap<String, &[u8]>,
) -> SourceCommandResult<()> {
    let entities = cmd::field(event, "entities")?;
    let mut observed = BTreeSet::new();
    for group in ["inputs", "outputs", "byproducts"] {
        for entity in cmd::array(entities, group)? {
            let reference = cmd::text(entity, "entity_ref")?;
            let name = reference
                .strip_prefix(&format!("{}/", home.as_str()))
                .ok_or(SourceCommandError::Conflict(
                    "creation event entity home differs",
                ))?;
            let raw = members
                .get(name)
                .ok_or(SourceCommandError::Conflict("creation event entity absent"))?;
            if name == "source-create-provenance.jsonl"
                || !observed.insert(name.to_owned())
                || cmd::integer(entity, "size_bytes")? != raw.len() as u64
                || cmd::text(entity, "sha256")? != Digest256::of_bytes(raw).to_hex()
                || cmd::field(entity, "fixity_verified")? != &JsonValue::Bool(false)
            {
                return Err(SourceCommandError::Conflict(
                    "creation event entity fixity differs",
                ));
            }
        }
    }
    let expected = members
        .keys()
        .filter(|name| {
            ![
                "source-create-provenance.jsonl",
                "source-create-receipt.json",
            ]
            .contains(&name.as_str())
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    if event.object_get("event_id").and_then(JsonValue::as_str) != Some(event_id)
        || observed != expected
    {
        return Err(SourceCommandError::Conflict(
            "creation provenance package closure differs",
        ));
    }
    Ok(())
}

fn validate_historical_claim_stream(
    raw: &[u8],
    record_id: &str,
    family: CreationFamily,
    members: &BTreeMap<String, &[u8]>,
) -> SourceCommandResult<BTreeSet<String>> {
    if raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid(
            "historical Claim stream budget",
        ));
    }
    let mut ids = BTreeSet::new();
    for line in raw
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
    {
        let claim = cmd::parse(line)?;
        if cmd::canonical(&claim)? != line
            || cmd::text(&claim, "subject_ref")? != record_id
            || cmd::integer(&claim, "claim_version")? != 1
            || !revisions::valid_id(cmd::text(&claim, "claim_id")?, "tos.claim.", false)
            || !ids.insert(cmd::text(&claim, "claim_id")?.to_owned())
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim stream identity differs",
            ));
        }
    }
    if family == CreationFamily::HistoricalV2
        && members.contains_key("source-create-provenance.jsonl")
    {
        // The exact event ID is checked by the caller once the provenance row
        // has been parsed; here the stream shape and ID set are sealed.
    }
    Ok(ids)
}
