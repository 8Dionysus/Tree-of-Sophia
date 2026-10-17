//! PostgreSQL owner preparation for selected Collection membership append.
//!
//! The PG path uses the complete selected witness catalog for membership
//! identity/topology and the selected compound kernel for Claim grammar,
//! schema custody, and retained native output. Existing membership claims are
//! replayed through the exact selected-input kernel, and their transaction
//! manifests are tied to the Collection history receipt.

use super::*;
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::SelectedSourceWitnessCatalog;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::record_biblio_cut::{
    SourceCutInput, SourceCutInputWithIdentity, SourceCutMemberMeta,
};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

const COLLECTION_CONTRACTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

/// Owner result and physical corpus fence. The common Growth writer keeps this
/// package alive through source-revision registration and commit.
pub(crate) struct PgCollectionMembershipOwnerPackage<'a> {
    pub(crate) context: CommandContext,
    pub(crate) proposal: cmd::PreparedCommand,
    pub(crate) semantic_indexes: IndexRows,
    pub(crate) fence: work_transaction::WorkCorpusFence<'a>,
}

/// Prepare one exact Collection/Work membership append from selected PG bytes.
/// Every retained membership Claim and native archive is replayed before the
/// successor set is admitted.
pub(crate) fn prepare_pg_collection_membership<'a, I: Copy + Eq>(
    fs: &'a CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    selected_catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgCollectionMembershipOwnerPackage<'a>> {
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    for path in COLLECTION_CONTRACTS {
        if context.file(&relative(path)?)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "selected Collection source registry/schema set incomplete",
            ));
        }
    }

    let fence = work_transaction::WorkCorpusFence::hold_existing(fs, limits.deadline, cancelled)?;
    let owner = CollectionOwner::select(fs, context, false, limits.deadline, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    check_requested_publication(&owner, &snapshot)?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    let prior_id = selected_prior_publication(input, &snapshot, limits.deadline, cancelled)?;
    if let Some(prior_id) = prior_id.as_deref() {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            prior_id,
            snapshot.token.as_deref(),
            limits.deadline,
            cancelled,
        )?;
    }
    let before = selected_before_from_input(
        &owner,
        input.source_input(),
        &snapshot,
        limits.deadline,
        cancelled,
    )?;
    let catalog = selected_membership_catalog(
        &owner,
        &before,
        input,
        selected_catalog,
        worker,
        limits,
        cancelled,
    )?;
    let directories = owner.new_directories(fs, limits.deadline, cancelled)?;
    verify_empty_prefix(
        input.source_input(),
        owner
            .claim_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "Collection selected Claim home",
            ))?
            .0,
        limits.deadline,
        cancelled,
    )?;

    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_selected_collection_membership_bytes(
        input,
        worker,
        &scope,
        &request_raw,
        &before,
        &context.recorded_at,
        remaining(&catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, context)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "Collection current dependency closure changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("Collection owner authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "Collection transaction identity differs from request",
        ));
    }
    let authorization = foundation_value(core.authorization())?;
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_collection_membership(
        &owner.request,
        cmd::text(&owner.configuration, "provenance_event_id")?,
        owner
            .claim_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "Collection selected Claim home",
            ))?
            .0,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_selected_collection_membership_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Collection finished byte recipe differs from selected transaction",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let old_collection = cmd::parse(before.get("collection.json").ok_or(
        SourceCommandError::Conflict("Collection selected parent absent"),
    )?)?;
    let expected_revision = cmd::text(&owner.request, "expected_revision")?;
    let archive = work_transaction::collection_archive_package(
        owner.collection_path.as_str(),
        &old_collection,
        &before,
        expected_revision,
    )?;
    if archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Collection retained archive locator differs from recipe",
        ));
    }
    let mut plan = owner.plan(
        authorization.clone(),
        &before,
        finished.parent,
        finished.child,
        directories,
    )?;
    append_archive_package(input.source_input(), &mut plan, &archive, limits, cancelled)?;
    include_missing_output_directories(input.source_input(), &mut plan, limits, cancelled)?;
    let commit_members = work_transaction::pg_selected_commit_members(
        input.source_input(),
        &snapshot,
        &plan,
        limits.deadline,
        cancelled,
    )?;
    for (path, after) in commit_members {
        let before = selected_optional_member(
            input.source_input(),
            &path,
            16_777_216,
            limits.deadline,
            cancelled,
        )?;
        if path != "ToS/source-witnesses/.metadata-publication.json" && before.is_some() {
            return Err(SourceCommandError::Conflict(
                "Collection PG transaction journal path already occupied",
            ));
        }
        plan.files.push(work_transaction::SelectedFile {
            path: relative(&path)?,
            before,
            after: Some(after),
        });
    }
    verify_new_paths_absent(input.source_input(), &plan, limits.deadline, cancelled)?;

    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    software_current(fs, context, limits.deadline, cancelled)?;
    fs.current_context(context, limits.deadline, cancelled)?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    fence.verify(limits.deadline, cancelled)?;

    let mut projected_outputs = BTreeMap::new();
    for file in &plan.files {
        if let Some(after) = &file.after {
            projected_outputs.insert(file.path.as_str().to_owned(), after.clone());
        }
    }
    let prepared = CollectionMembershipPreparation {
        request: owner.request.clone(),
        projected_outputs: projected_outputs.clone(),
    };
    let mut response = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_collection_membership_result_v1"),
        ),
        ("authentication", cmd::string("local-unix-account")),
        (
            "owner_configuration",
            cmd::string(&owner.configuration_digest),
        ),
        ("operation", cmd::string(OPERATION)),
        (
            "command_operations",
            JsonValue::Array(
                ["describe", "prepare-attach", OPERATION, RECOVERY]
                    .into_iter()
                    .map(cmd::string)
                    .collect(),
            ),
        ),
        (
            "allowed_operations",
            cmd::field(&owner.configuration, "allowed_operations")?.clone(),
        ),
        (
            "collection_source_path",
            cmd::string(owner.collection_path.as_str()),
        ),
        ("claim_source_path", cmd::string(owner.claim_path.as_str())),
        (
            "source",
            cmd::reference(&old_collection, "record_id", "record_version")?,
        ),
        (
            "revision",
            cmd::string(&crate::source_revisions::revision(&before)?),
        ),
        (
            "publication_snapshot",
            snapshot
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&old_collection)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
        ("scope", owner.scope()?),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(false)),
        ("recovery", JsonValue::Null),
        ("materializations", JsonValue::Null),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    for (key, value) in prepared
        .result_fields(&owner.configuration)?
        .as_object()
        .ok_or(SourceCommandError::Invalid("Collection PG result fields"))?
    {
        cmd::set(
            &mut response,
            key.as_str().ok_or(SourceCommandError::Invalid(
                "Collection PG result field key",
            ))?,
            value.clone(),
        )?;
    }
    let changes = plan
        .files
        .iter()
        .map(|file| {
            Ok(cmd::SourceChange {
                path: file.path.clone(),
                before: file.before.as_ref().map(|raw| Digest256::of_bytes(raw)),
                after: file.after.clone(),
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let writes = projected_writes(&plan)?;
    let semantic_indexes = expected_collection_membership_revision_indexes(&writes)
        .map_err(|_| SourceCommandError::Invalid("Collection PG semantic index projection"))?;
    let proposal = context.plan("native-collection-membership", response, changes, false)?;
    expected_collection_inventory_contributions(&writes, context, &proposal.response)
        .map_err(|_| SourceCommandError::Invalid("Collection PG inventory projection"))?;

    Ok(PgCollectionMembershipOwnerPackage {
        context: context.clone(),
        proposal,
        semantic_indexes,
        fence,
    })
}

/// Build the same descriptive Collection attachment materialization from the
/// selected PG source epoch without making a transaction plan or admission
/// package. The caller binds all reads to the held PG readset before returning
/// the preview to the CLI.
pub(crate) fn prepare_pg_collection_membership_preview<I: Copy + Eq>(
    fs: &CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    selected_catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<super::CollectionMembershipPreparation> {
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    for path in COLLECTION_CONTRACTS {
        if context.file(&relative(path)?)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "selected Collection source registry/schema set incomplete",
            ));
        }
    }

    let fence = work_transaction::WorkCorpusFence::hold_existing(fs, limits.deadline, cancelled)?;
    let mut owner = CollectionOwner::select(fs, context, true, limits.deadline, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    let prior_id = selected_prior_publication(input, &snapshot, limits.deadline, cancelled)?;
    if let Some(prior_id) = prior_id.as_deref() {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            prior_id,
            snapshot.token.as_deref(),
            limits.deadline,
            cancelled,
        )?;
    }
    let before = selected_before_from_input(
        &owner,
        input.source_input(),
        &snapshot,
        limits.deadline,
        cancelled,
    )?;
    let catalog = selected_membership_catalog(
        &owner,
        &before,
        input,
        selected_catalog,
        worker,
        limits,
        cancelled,
    )?;
    owner.new_directories(fs, limits.deadline, cancelled)?;
    verify_empty_prefix(
        input.source_input(),
        owner
            .claim_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "Collection selected Claim home",
            ))?
            .0,
        limits.deadline,
        cancelled,
    )?;

    let collection = cmd::parse(before.get("collection.json").ok_or(
        SourceCommandError::Conflict("Collection preview selected record absent"),
    )?)?;
    let mut refs = cmd::array(&collection, "membership_claim_refs")?.to_vec();
    if refs.len() >= 128
        || refs
            .iter()
            .any(|value| value.as_str() == Some(owner.claim_id.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "Collection preview Claim append invalid",
        ));
    }
    refs.push(cmd::string(&owner.claim_id));
    let claim = cmd::field(&owner.request, "claim")?;
    let mut claim_raw = cmd::canonical(claim)?;
    claim_raw.push(b'\n');
    let scope = serde_value(&owner.scope()?)?;
    let mut prepared_request = None;
    let mut authorization_error = None;
    let core =
        tos_validation::native_compound::prepare_selected_collection_membership_preview_bytes(
            input,
            worker,
            &scope,
            &claim_raw,
            &before,
            &context.recorded_at,
            remaining(&catalog, limits)?,
            cancelled,
            |grammar| {
                let result = (|| {
                    let dependencies = dependency_bindings(&catalog, grammar, context)?;
                    let mut request = owner.request.clone();
                    cmd::set(&mut request, "operation", cmd::string(OPERATION))?;
                    cmd::set(
                        &mut request,
                        "command_id",
                        cmd::string("preview:uncommitted"),
                    )?;
                    cmd::set(
                        &mut request,
                        "fields",
                        cmd::object(vec![("membership_claim_refs", JsonValue::Array(refs))]),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_configuration",
                        cmd::string(&owner.configuration_digest),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_source",
                        cmd::reference(&collection, "record_id", "record_version")?,
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_revision",
                        cmd::string(&crate::source_revisions::revision(&before)?),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_dependencies",
                        cmd::string(&cmd::record_digest(&dependencies)?.to_prefixed()),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_publication",
                        snapshot
                            .token
                            .as_ref()
                            .map_or(JsonValue::Null, |token| cmd::string(token)),
                    )?;
                    owner.request = request.clone();
                    let authorization = serde_value(&owner.authorization(dependencies)?)?;
                    let mut raw = cmd::canonical(&request)?;
                    raw.push(b'\n');
                    prepared_request = Some(request);
                    Ok((raw, authorization))
                })();
                result.map_err(|error| {
                    authorization_error = Some(error);
                    ItemRefusal::Source("Collection preview authorization rejected".to_owned())
                })
            },
        );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    let request = prepared_request.ok_or(SourceCommandError::Invalid(
        "Collection preview full request absent",
    ))?;
    let authorization = foundation_value(core.authorization())?;
    let outputs = core.outputs().map_err(item_error)?;
    drop(outputs);
    let projected_outputs = core.into_prepared_outputs().map_err(item_error)?;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    fs.current_context(context, limits.deadline, cancelled)?;
    software_current(fs, context, limits.deadline, cancelled)?;
    work_dependencies_current(
        fs,
        cmd::field(&authorization, "dependency_bindings")?,
        limits.deadline,
        cancelled,
    )?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    fence.verify(limits.deadline, cancelled)?;
    Ok(super::CollectionMembershipPreparation {
        request,
        projected_outputs,
    })
}

/// Build the Collection owner's current-state `describe` fields from the
/// selected PG source epoch. It reads only the configured Collection, its
/// current Work endpoint, publication state, and the owner's schema/registry
/// inputs; it does not construct an attachment proposal or materialize a
/// Claim.
pub(crate) fn current_result_fields_from_selected_pg<I: Copy + Eq>(
    fs: &CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let deadline = limits.deadline;
    context.check_selected_software_inputs(software, components, deadline, cancelled)?;
    let request = cmd::parse(&context.request_raw)?;
    cmd::exact_keys(&request, &["schema_version", "operation"])?;
    if cmd::text(&request, "schema_version")? != REQUEST
        || cmd::text(&request, "operation")? != "describe"
    {
        return Err(SourceCommandError::Invalid(
            "Collection selected describe request",
        ));
    }
    for path in COLLECTION_CONTRACTS {
        let relative_path = relative(path)?;
        let expected = context
            .file(&relative_path)?
            .ok_or(SourceCommandError::Unsupported(
                "selected Collection source registry/schema set incomplete",
            ))?;
        let selected = read_selected_source_member(
            input.source_input(),
            path,
            2_097_152,
            deadline,
            cancelled,
        )?;
        if selected.as_slice() != expected {
            return Err(SourceCommandError::Conflict(
                "Collection describe schema differs from selected context",
            ));
        }
    }

    let grant = CollectionGrant::select(fs, context, None, deadline, cancelled)?;
    let config = grant.configuration;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        deadline,
        cancelled,
    )?;
    if let Some(id) = selected_prior_publication(input, &snapshot, deadline, cancelled)? {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            &id,
            snapshot.token.as_deref(),
            deadline,
            cancelled,
        )?;
    }

    let collection_path = grant.collection_path.as_str();
    let collection_id = cmd::text(&config, "collection_id")?;
    let collection_entry = unique_catalog_entry(catalog, "collection", collection_id)?;
    if cmd::text(collection_entry, "source_record_ref")? != collection_path {
        return Err(SourceCommandError::Conflict(
            "Collection describe current endpoint differs from catalog",
        ));
    }
    let collection_raw = read_selected_source_member(
        input.source_input(),
        collection_path,
        2_097_152,
        deadline,
        cancelled,
    )?;
    let collection = cmd::parse(&collection_raw)?;
    if cmd::text(&collection, "record_type")? != "collection"
        || cmd::text(&collection, "record_id")? != collection_id
        || cmd::text(collection_entry, "record_sha256")?
            != cmd::record_digest(&collection)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "Collection describe current record differs from catalog",
        ));
    }
    let collection_home = collection_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(SourceCommandError::Invalid(
            "Collection describe current parent path",
        ))?;
    let mut package = BTreeMap::from([("collection.json".to_owned(), collection_raw.clone())]);
    for name in [
        "collection.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = format!("{collection_home}/{name}");
        if let Some(raw) =
            selected_optional_member(input.source_input(), &path, 2_097_152, deadline, cancelled)?
        {
            package.insert(name.to_owned(), raw);
        }
    }
    tos_validation::native_compound::inspect_record_history(
        &package,
        &collection_raw,
        deadline,
        cancelled,
    )
    .map_err(item_error)?;

    let work_path = grant.work_path.as_str();
    let work_id = cmd::text(&config, "work_id")?;
    let work_entry = unique_catalog_entry(catalog, "work", work_id)?;
    if cmd::text(work_entry, "source_record_ref")? != work_path {
        return Err(SourceCommandError::Conflict(
            "Collection describe Work endpoint differs from catalog",
        ));
    }
    let work_raw = read_selected_source_member(
        input.source_input(),
        work_path,
        2_097_152,
        deadline,
        cancelled,
    )?;
    let work = cmd::parse(&work_raw)?;
    if cmd::text(&work, "record_type")? != "work"
        || cmd::text(&work, "record_id")? != work_id
        || cmd::text(work_entry, "record_sha256")? != cmd::record_digest(&work)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "Collection describe current Work differs from catalog",
        ));
    }

    let entity = selected_context_descriptor(
        context,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    )?;
    let relation = selected_context_descriptor(
        context,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    )?;
    let corpus = selected_context_descriptor(context, "ToS/contracts/corpus-record.schema.json")?;
    let mut profiles = Vec::new();
    for kind in ["collection", "work"] {
        let matching = entity["types"]
            .as_array()
            .ok_or(SourceCommandError::Invalid("Collection entity registry"))?
            .iter()
            .filter(|entry| {
                entry["source_mappings"].as_array().is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row["source_graph"] == "source-claims" && row["source_kind_id"] == kind
                    })
                })
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "Collection descriptor mapping ambiguous",
            ));
        }
        let value = serde_json::json!({
            "type_id": matching[0]["type_id"],
            "record_type": kind,
            "schema_ref": "ToS/contracts/corpus-record.schema.json",
            "schema_version": corpus["properties"]["schema_version"]["const"],
            "source_basename": format!("{kind}.json")
        });
        profiles.push((kind, foundation_value(&value)?));
    }
    let matching = relation["relations"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("Collection relation registry"))?
        .iter()
        .filter(|entry| {
            entry["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-claims"
                        && row["source_predicate_id"] == "contains_work"
                        && row["scope"] == "claim-predicate"
                })
            })
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "Collection relation descriptor ambiguous",
        ));
    }
    let profile = &matching[0]["source_claim_profile"];
    let routes = profile["schemas"]
        .as_array()
        .ok_or(SourceCommandError::Invalid(
            "Collection relation schema routes",
        ))?
        .iter()
        .filter(|row| row["schema_version"] == "tos_source_relation_claim_v1")
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "Collection Claim descriptor ambiguous",
        ));
    }
    profiles.push((
        "contains_work",
        foundation_value(&serde_json::json!({
            "relation_type_id": matching[0]["relation_type_id"],
            "predicate": "contains_work",
            "reader": profile["reader"],
            "schema_ref": routes[0]["schema_ref"],
            "schema_version": routes[0]["schema_version"],
            "assertion_layers": profile["assertion_layers"],
            "source_basename": "source-claims.jsonl"
        }))?,
    ));

    let scope = JsonValue::Object(
        config
            .as_object()
            .ok_or(SourceCommandError::Invalid("Collection result grant"))?
            .iter()
            .filter(|(key, _)| key.as_str().is_some_and(|key| SCOPE_KEYS.contains(&key)))
            .cloned()
            .collect(),
    );
    let result = cmd::object(vec![
        (
            "source",
            cmd::reference(&collection, "record_id", "record_version")?,
        ),
        (
            "revision",
            cmd::string(&crate::source_revisions::revision(&package)?),
        ),
        (
            "publication_snapshot",
            snapshot
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("work_record", work.clone()),
        (
            "work_source_binding",
            cmd::object(vec![
                (
                    "source_path",
                    cmd::field(&config, "work_source_path")?.clone(),
                ),
                (
                    "source",
                    cmd::reference(&work, "record_id", "record_version")?,
                ),
                (
                    "source_sha256",
                    cmd::string(&Digest256::of_bytes(&work_raw).to_prefixed()),
                ),
                ("source_bytes", cmd::number(work_raw.len() as u64)),
            ]),
        ),
        ("scope", scope),
        ("materializations", JsonValue::Null),
        ("source_profiles", cmd::object(profiles)),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&collection)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
    ]);
    worker.finish(deadline, cancelled).map_err(item_error)?;
    software_current(fs, context, deadline, cancelled)?;
    fs.current_context(context, deadline, cancelled)?;
    snapshot.verify_from_input(input.source_input(), deadline, cancelled)?;
    snapshot.verify_current(fs, deadline, cancelled)?;
    Ok(result)
}

fn selected_context_descriptor(
    context: &CommandContext,
    path: &str,
) -> SourceCommandResult<serde_json::Value> {
    let raw = context
        .file(&relative(path)?)?
        .ok_or(SourceCommandError::Unsupported(
            "selected Collection descriptor absent",
        ))?;
    serde_json::from_slice(raw)
        .map_err(|_| SourceCommandError::Invalid("Collection descriptor JSON"))
}

fn projected_writes(
    plan: &work_transaction::WorkPlan,
) -> SourceCommandResult<Vec<SourceRevisionWrite>> {
    plan.files
        .iter()
        .filter_map(|file| {
            file.after.as_ref().map(|raw| {
                Ok(SourceRevisionWrite {
                    subject: file.path.as_str().to_owned(),
                    expected_predecessor: file
                        .before
                        .as_ref()
                        .map(|before| (1, Digest256::of_bytes(before))),
                    proposed_revision: 1,
                    exact_bytes: raw.clone(),
                })
            })
        })
        .collect()
}

/// Compare exact PG source reads and absence predicates with the protected
/// physical corpus while the retained owner fence is held by the caller.
pub(crate) fn verify_pg_collection_context(
    fs: &CreationFilesystem,
    context: &CommandContext,
    absences: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    fs.current_context(context, deadline, cancelled)?;
    software_current(fs, context, deadline, cancelled)?;
    for path in absences {
        let (parent_path, name) = path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Collection exact absence path"))?;
        let Some(parent) = work_transaction::read_existing_parent(fs, parent_path)? else {
            continue;
        };
        if work_transaction::read_at(&parent, name, fs.uid, 16_777_216, deadline, cancelled)?
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "Collection exact path absence differs from protected corpus",
            ));
        }
    }
    Ok(())
}

fn check_requested_publication(
    owner: &CollectionOwner,
    snapshot: &work_transaction::PublicationSnapshot,
) -> SourceCommandResult<()> {
    let requested = cmd::field(&owner.request, "expected_publication")?;
    if requested.as_str() != snapshot.token.as_deref()
        && !(requested == &JsonValue::Null && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "Collection requested publication snapshot changed",
        ));
    }
    Ok(())
}

fn selected_before_from_input(
    owner: &CollectionOwner,
    input: &dyn SourceCutInput,
    snapshot: &work_transaction::PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent = owner
        .collection_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Collection parent"))?
        .0;
    let mut before = BTreeMap::new();
    let mut total = 0usize;
    for name in [
        "collection.json",
        "collection.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = format!("{parent}/{name}");
        let raw = selected_optional_member(input, &path, 2_097_152, deadline, cancelled)?;
        if name == "collection.json" && raw.is_none() {
            return Err(SourceCommandError::Conflict(
                "Collection selected parent record absent",
            ));
        }
        if let Some(raw) = raw {
            total = total
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= 8_388_608)
                .ok_or(SourceCommandError::Invalid(
                    "selected Collection package byte budget",
                ))?;
            before.insert(name.to_owned(), raw);
        }
    }
    snapshot.verify_from_input(input, deadline, cancelled)?;
    Ok(before)
}

fn selected_prior_publication<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    snapshot: &work_transaction::PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<String>> {
    let path = "ToS/source-witnesses/.metadata-publication.json";
    let Some(_) = input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("Collection prior publication presence"))?
    else {
        if snapshot.token.is_some() {
            return Err(SourceCommandError::Conflict(
                "Collection prior publication absent",
            ));
        }
        return Ok(None);
    };
    let raw = read_selected_source_member(input.source_input(), path, 8192, deadline, cancelled)?;
    let state = cmd::parse(&raw)?;
    work_transaction::state(&state)?;
    if cmd::text(&state, "phase")? != "ready"
        || Some(cmd::text(&state, "token")?) != snapshot.token.as_deref()
    {
        return Err(SourceCommandError::Conflict(
            "Collection prior publication differs from protected head",
        ));
    }
    Ok(Some(cmd::text(&state, "transaction_id")?.to_owned()))
}

fn selected_prior_completion(
    input: &dyn SourceCutInput,
    snapshot: &work_transaction::PublicationSnapshot,
    id: &str,
    expected_token: Option<&str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let Some(token) = expected_token else {
        return Err(SourceCommandError::Conflict(
            "Collection prior publication token absent",
        ));
    };
    let journal = format!("ToS/source-witnesses/.metadata-transactions/{id}");
    let manifest_raw = read_selected_source_member(
        input,
        &format!("{journal}/manifest.json"),
        1_048_576,
        deadline,
        cancelled,
    )?;
    let manifest = cmd::parse(&manifest_raw)?;
    cmd::exact_keys(
        &manifest,
        &[
            "schema_version",
            "transaction_id",
            "manifest_sha256",
            "base_publication",
            "plan",
            "parents",
            "parent_binding_profile",
        ],
    )?;
    if cmd::text(&manifest, "parent_binding_profile")? != "tos_source_revision_parent_bindings_v1" {
        return Err(SourceCommandError::Conflict(
            "Collection prior PG parent binding profile differs",
        ));
    }
    let base = cmd::field(&manifest, "base_publication")?;
    cmd::exact_keys(base, &["token", "generation"])?;
    let generation = cmd::integer(base, "generation")?;
    let previous_token = match cmd::field(base, "token")? {
        JsonValue::Null => generation == 0,
        JsonValue::String(value) => {
            generation > 0 && value.as_str().is_some_and(work_transaction::is_hash)
        }
        _ => false,
    };
    let state = snapshot
        .selected_state()
        .ok_or(SourceCommandError::Conflict(
            "Collection prior publication state absent",
        ))?;
    if cmd::text(&manifest, "transaction_id")? != id
        || Digest256::of_bytes(&manifest_raw).to_prefixed() != cmd::text(state, "manifest_sha256")?
        || generation.checked_add(2) != Some(cmd::integer(state, "generation")?)
        || !previous_token
        || cmd::text(state, "token")? != token
        || cmd::text(state, "transaction_id")? != id
    {
        return Err(SourceCommandError::Conflict(
            "Collection prior publication manifest differs",
        ));
    }
    let completion_raw = read_selected_source_member(
        input,
        &format!("{journal}/completion.json"),
        8192,
        deadline,
        cancelled,
    )?;
    let completion = cmd::parse(&completion_raw)?;
    cmd::exact_keys(&completion, &["schema_version", "publication"])?;
    if cmd::text(&completion, "schema_version")? != "tos_selected_metadata_completion_v1"
        || !cmd::same(
            cmd::field(&completion, "publication")?,
            snapshot
                .selected_state()
                .ok_or(SourceCommandError::Conflict(
                    "Collection prior publication state absent",
                ))?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "Collection prior completion differs from selected publication",
        ));
    }
    Ok(())
}

fn selected_membership_catalog<I: Copy + Eq>(
    owner: &CollectionOwner,
    before: &BTreeMap<String, Vec<u8>>,
    input: &dyn SourceCutInputWithIdentity<I>,
    selected: &SelectedSourceWitnessCatalog,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionCatalog> {
    let deadline = limits.deadline;
    let collection_raw = before
        .get("collection.json")
        .ok_or(SourceCommandError::Conflict(
            "Collection predecessor absent",
        ))?;
    let collection = cmd::parse(collection_raw)?;
    let history = tos_validation::native_compound::inspect_record_history(
        before,
        collection_raw,
        deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let collection_entry = unique_catalog_entry(selected, "collection", &owner.collection_id)?;
    if cmd::text(&collection, "record_type")? != "collection"
        || cmd::text(&collection, "record_id")? != owner.collection_id
        || cmd::text(collection_entry, "source_record_ref")? != owner.collection_path.as_str()
        || cmd::text(collection_entry, "record_sha256")?
            != cmd::record_digest(&collection)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "Collection selected catalog parent differs",
        ));
    }
    if selected.record_ids.contains(&owner.claim_id)
        || selected.claim_ids.contains(&owner.claim_id)
        || selected.entries.iter().any(|(kind, entry)| {
            kind == "claim"
                && entry
                    .object_get("provenance_event_ref")
                    .and_then(JsonValue::as_str)
                    == owner
                        .configuration
                        .object_get("provenance_event_id")
                        .and_then(JsonValue::as_str)
        })
    {
        return Err(SourceCommandError::Conflict(
            "Collection new Claim/event identity occupied",
        ));
    }
    let mut records = BTreeMap::<String, &JsonValue>::new();
    let mut claims = BTreeMap::<String, &JsonValue>::new();
    for (kind, entry) in &selected.entries {
        let (index, id) = if kind == "claim" {
            (&mut claims, cmd::text(entry, "claim_id")?)
        } else {
            (&mut records, cmd::text(entry, "record_id")?)
        };
        if index.insert(id.to_owned(), entry).is_some() {
            return Err(SourceCommandError::Conflict(
                "Collection selected catalog identity duplicated",
            ));
        }
    }
    let work_entry = records
        .get(&owner.work_id)
        .copied()
        .ok_or(SourceCommandError::Conflict(
            "Collection selected Work catalog endpoint absent",
        ))?;
    if cmd::text(work_entry, "source_record_ref")? != owner.work_path.as_str() {
        return Err(SourceCommandError::Denied(
            "Collection exact Work catalog endpoint differs",
        ));
    }
    let work_raw = read_selected_source_member(
        input,
        owner.work_path.as_str(),
        2_097_152,
        deadline,
        cancelled,
    )?;
    let work = cmd::parse(&work_raw)?;
    if cmd::text(&work, "record_type")? != "work"
        || cmd::text(&work, "record_id")? != owner.work_id
        || cmd::text(work_entry, "record_sha256")? != cmd::record_digest(&work)?.to_hex()
        || !cmd::same(&work, cmd::field(&owner.request, "work")?)?
    {
        return Err(SourceCommandError::Conflict(
            "Collection caller Work differs from selected catalog bytes",
        ));
    }
    let mut digests = selected.digests.clone();
    insert_digest(&mut digests, owner.work_path.as_str(), &work_raw)?;
    let mut source_bytes =
        selected
            .bytes_read
            .checked_add(work_raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Collection selected source byte overflow",
            ))?;
    let mut works = BTreeMap::from([(owner.work_id.clone(), work)]);
    let mut selected_claims = BTreeSet::new();
    let mut retained_transactions = BTreeMap::new();
    let mut native_reads = Vec::new();
    let mut native_bytes = 0u64;
    let mut native_state = 0usize;
    let mut legacy_events: Option<Vec<JsonValue>> = None;
    for (id, entry) in claims.iter().filter(|(_, entry)| {
        entry.object_get("predicate").and_then(JsonValue::as_str) == Some("contains_work")
            && entry.object_get("subject_ref").and_then(JsonValue::as_str)
                == Some(owner.collection_id.as_str())
    }) {
        active(deadline, cancelled)?;
        let claim = selected_catalog_claim(
            input.source_input(),
            entry,
            &mut digests,
            &mut source_bytes,
            deadline,
            cancelled,
        )?;
        let target = cmd::text(&claim, "object")?;
        let endpoint = records
            .get(target)
            .copied()
            .ok_or(SourceCommandError::Conflict(
                "Collection member Work absent from catalog",
            ))?;
        if cmd::text(endpoint, "record_type")? != "work" {
            return Err(SourceCommandError::Conflict(
                "Collection member endpoint is not Work",
            ));
        }
        if !works.contains_key(target) {
            let path = cmd::text(endpoint, "source_record_ref")?;
            let raw = selected_dependency(
                input.source_input(),
                path,
                &mut digests,
                &mut source_bytes,
                2_097_152,
                deadline,
                cancelled,
            )?;
            let value = cmd::parse(&raw)?;
            if cmd::field(&value, "record_id")? != cmd::field(endpoint, "record_id")?
                || cmd::field(&value, "record_type")? != cmd::field(endpoint, "record_type")?
                || cmd::record_digest(&value)?.to_hex() != cmd::text(endpoint, "record_sha256")?
            {
                return Err(SourceCommandError::Conflict(
                    "Collection member Work catalog row is stale",
                ));
            }
            works.insert(target.to_owned(), value);
        }
        let claim_path = cmd::text(entry, "source_claim_file_ref")?;
        if claim_path.ends_with("/source-claims.jsonl") {
            let consumed = u64::try_from(source_bytes)
                .ok()
                .and_then(|bytes| bytes.checked_add(native_bytes))
                .ok_or(SourceCommandError::Invalid(
                    "Collection cumulative selected source bytes",
                ))?;
            let mut remaining = limits;
            remaining.max_total_bytes =
                limits
                    .max_total_bytes
                    .checked_sub(consumed)
                    .ok_or(SourceCommandError::Invalid(
                        "Collection cumulative native bytes",
                    ))?;
            remaining.max_state_bytes = limits.max_state_bytes.checked_sub(native_state).ok_or(
                SourceCommandError::Invalid("Collection cumulative native state"),
            )?;
            let observation =
                tos_validation::native_compound::verify_collection_membership_from_selected_input(
                    input,
                    worker,
                    claim_path,
                    &serde_value(&claim)?,
                    remaining,
                    cancelled,
                )
                .map_err(item_error)?;
            let parent_receipts = history["receipts"]
                .as_array()
                .ok_or(SourceCommandError::Invalid("Collection history"))?;
            let history_receipt = parent_receipts.iter().find(|receipt| {
                receipt["publication"]["transaction_id"].as_str()
                    == Some(observation.transaction_id.as_str())
            });
            let receipt_manifest = history_receipt
                .and_then(|receipt| receipt["publication"]["manifest_sha256"].as_str());
            if observation.transport
                != tos_validation::native_compound::NativeTransportState::Committed
                || receipt_manifest.is_some_and(|sha| sha != observation.manifest_sha256)
                || history_receipt.is_none()
            {
                return Err(SourceCommandError::Conflict(
                    "Collection member lacks exact selected PG receipt/history lineage",
                ));
            }
            if retained_transactions
                .insert(
                    observation.transaction_id.clone(),
                    observation.manifest_sha256.clone(),
                )
                .is_some_and(|prior| prior != observation.manifest_sha256)
            {
                return Err(SourceCommandError::Conflict(
                    "Collection retained selected transaction changed",
                ));
            }
            native_bytes = native_bytes.checked_add(observation.bytes_read).ok_or(
                SourceCommandError::Invalid("Collection native byte overflow"),
            )?;
            native_state = native_state
                .checked_add(observation.returned_state_bytes)
                .ok_or(SourceCommandError::Invalid(
                    "Collection native state overflow",
                ))?;
            native_reads.extend(observation.reads);
        } else if claim_path.ends_with("/membership-claims.jsonl") {
            if legacy_events.is_none() {
                let mut events = Vec::new();
                for reference in
                    cmd::array(&owner.configuration, "retained_membership_provenance_refs")?
                {
                    let reference = reference
                        .as_str()
                        .ok_or(SourceCommandError::Invalid("Collection provenance path"))?;
                    let raw = selected_dependency(
                        input.source_input(),
                        reference,
                        &mut digests,
                        &mut source_bytes,
                        2_097_152,
                        deadline,
                        cancelled,
                    )?;
                    for line in catalog_lines(&raw) {
                        if events.len() >= 8192 {
                            return Err(SourceCommandError::Invalid(
                                "Collection retained provenance event budget",
                            ));
                        }
                        events.push(cmd::parse(line)?);
                    }
                }
                legacy_events = Some(events);
            }
            let matches = legacy_events
                .as_ref()
                .ok_or(SourceCommandError::Invalid("Collection legacy event cache"))?
                .iter()
                .filter(|event| {
                    event.object_get("event_id") == claim.object_get("provenance_event_ref")
                })
                .collect::<Vec<_>>();
            if cmd::text(&claim, "claim_type")? != "bibliographic"
                || !matches!(
                    cmd::text(&claim, "assertion_layer")?,
                    "bibliographic_assertion" | "scholarly_report"
                )
                || matches.len() != 1
                || !cmd::array(matches[0], "outputs")?.iter().any(|output| {
                    output.object_get("ref").and_then(JsonValue::as_str) == Some(claim_path)
                        && output.object_get("sha256").and_then(JsonValue::as_str)
                            == digests.get(claim_path).map(String::as_str)
                })
            {
                return Err(SourceCommandError::Conflict(
                    "Collection legacy membership retained batch differs",
                ));
            }
        } else {
            return Err(SourceCommandError::Denied(
                "Collection member has no declared evidence carrier",
            ));
        }
        selected_claims.insert(id.clone());
    }
    let declared = cmd::array(&collection, "membership_claim_refs")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid(
                    "Collection membership reference",
                ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if declared.len() != selected_claims.len()
        || declared.iter().cloned().collect::<BTreeSet<_>>() != selected_claims
    {
        return Err(SourceCommandError::Conflict(
            "Collection complete selected membership union differs",
        ));
    }
    let claim = cmd::field(&owner.request, "claim")?;
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(values) = claim.object_get(key) {
            for reference in values
                .as_array()
                .ok_or(SourceCommandError::Invalid("Collection evidence list"))?
            {
                let reference = reference
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("Collection evidence reference"))?;
                if reference.starts_with("ToS/") && reference != owner.collection_path.as_str() {
                    selected_dependency(
                        input.source_input(),
                        reference,
                        &mut digests,
                        &mut source_bytes,
                        8_388_608,
                        deadline,
                        cancelled,
                    )?;
                }
            }
        }
    }
    if let Some(rows) = cmd::field(&owner.request, "claim")?.object_get("alternative_claim_refs") {
        if rows
            .as_array()
            .ok_or(SourceCommandError::Invalid(
                "Collection alternative Claim list",
            ))?
            .iter()
            .any(|row| !row.as_str().is_some_and(|id| claims.contains_key(id)))
        {
            return Err(SourceCommandError::Conflict(
                "Collection alternative Claim lacks catalog identity",
            ));
        }
    }
    if source_bytes as u64 > limits.max_total_bytes as u64 {
        return Err(SourceCommandError::Invalid(
            "Collection selected source byte budget",
        ));
    }
    Ok(CollectionCatalog {
        digests,
        retained_transactions,
        native_reads,
        native_bytes,
        native_state,
        source_bytes: source_bytes as u64,
    })
}

fn selected_dependency(
    input: &dyn SourceCutInput,
    path: &str,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let raw = read_selected_source_member(input, path, max_bytes, deadline, cancelled)?;
    *total = total
        .checked_add(raw.len())
        .filter(|bytes| *bytes <= 33_554_432)
        .ok_or(SourceCommandError::Invalid(
            "Collection selected current dependency byte budget",
        ))?;
    if !digests.contains_key(path) && digests.len() >= 512 {
        return Err(SourceCommandError::Invalid(
            "Collection selected current dependency count budget",
        ));
    }
    insert_digest(digests, path, &raw)?;
    Ok(raw)
}

fn selected_catalog_claim(
    input: &dyn SourceCutInput,
    entry: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let path = cmd::text(entry, "source_claim_file_ref")?;
    let raw = selected_dependency(input, path, digests, total, 2_097_152, deadline, cancelled)?;
    let line = usize::try_from(cmd::integer(entry, "source_claim_line")?)
        .map_err(|_| SourceCommandError::Invalid("Collection Claim line"))?
        .checked_sub(1)
        .ok_or(SourceCommandError::Invalid("Collection Claim line"))?;
    let claim = cmd::parse(
        catalog_lines(&raw)
            .nth(line)
            .ok_or(SourceCommandError::Conflict("Collection Claim line absent"))?,
    )?;
    for key in [
        "claim_id",
        "predicate",
        "object",
        "subject_ref",
        "provenance_event_ref",
    ] {
        if cmd::field(&claim, key)? != cmd::field(entry, key)? {
            return Err(SourceCommandError::Conflict(
                "Collection selected catalog Claim endpoint stale",
            ));
        }
    }
    if cmd::record_digest(&claim)?.to_hex() != cmd::text(entry, "claim_sha256")? {
        return Err(SourceCommandError::Conflict(
            "Collection selected catalog Claim digest stale",
        ));
    }
    Ok(claim)
}

fn unique_catalog_entry<'a>(
    catalog: &'a SelectedSourceWitnessCatalog,
    kind: &str,
    id: &str,
) -> SourceCommandResult<&'a JsonValue> {
    let mut entries = catalog.entries.iter().filter(|(entry_kind, entry)| {
        entry_kind == kind && entry.object_get("record_id").and_then(JsonValue::as_str) == Some(id)
    });
    let entry = entries
        .next()
        .map(|(_, entry)| entry)
        .ok_or(SourceCommandError::Conflict(
            "Collection catalog endpoint absent",
        ))?;
    if entries.next().is_some() {
        return Err(SourceCommandError::Conflict(
            "Collection catalog endpoint duplicated",
        ));
    }
    Ok(entry)
}

fn insert_digest(
    digests: &mut BTreeMap<String, String>,
    path: &str,
    raw: &[u8],
) -> SourceCommandResult<()> {
    let digest = Digest256::of_bytes(raw).to_hex();
    if digests
        .insert(path.to_owned(), digest.clone())
        .is_some_and(|old| old != digest)
    {
        return Err(SourceCommandError::Conflict(
            "Collection selected dependency changed",
        ));
    }
    Ok(())
}

fn read_selected_source_member(
    input: &dyn SourceCutInput,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path_relative = relative(path)?;
    if path_relative.as_str() != path {
        return Err(SourceCommandError::Invalid(
            "Collection selected path canonicality",
        ));
    }
    if input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("Collection selected member presence"))?
        .is_none()
    {
        return Err(SourceCommandError::Conflict(
            "Collection selected member absent",
        ));
    }
    let mut selected = None;
    input
        .with_current_member(
            path,
            max_bytes.min(8_388_608),
            deadline,
            cancelled,
            &mut |meta: SourceCutMemberMeta<'_>, raw| {
                if meta.path != path || raw.len() as u64 != meta.size_bytes {
                    return Err(ItemRefusal::Source(
                        "Collection selected member binding differs".into(),
                    ));
                }
                selected = Some(raw.to_vec());
                Ok(())
            },
        )
        .map_err(|_| SourceCommandError::Conflict("Collection selected member read"))?;
    selected.ok_or(SourceCommandError::Conflict(
        "Collection selected member absent",
    ))
}

fn selected_optional_member(
    input: &dyn SourceCutInput,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    if input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("Collection selected member presence"))?
        .is_some()
    {
        Ok(Some(read_selected_source_member(
            input, path, max_bytes, deadline, cancelled,
        )?))
    } else {
        Ok(None)
    }
}

fn verify_empty_prefix(
    input: &dyn SourceCutInput,
    prefix: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut found = None;
    input
        .for_each_current_member_meta_under(prefix, deadline, cancelled, &mut |meta| {
            found = Some(meta.path.to_owned());
            Err(ItemRefusal::Source(
                "Collection new output home is not empty".into(),
            ))
        })
        .map_err(|_| {
            if found.is_some() {
                SourceCommandError::Conflict("Collection new output home is not empty")
            } else {
                SourceCommandError::Conflict("Collection output home enumeration incomplete")
            }
        })?;
    Ok(())
}

fn append_archive_package(
    input: &dyn SourceCutInput,
    plan: &mut work_transaction::WorkPlan,
    archive: &work_transaction::WorkArchivePackage,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let expected_paths = archive
        .members
        .keys()
        .map(|name| format!("{}/{name}", archive.path))
        .collect::<BTreeSet<_>>();
    let mut selected_paths = BTreeSet::new();
    input
        .for_each_current_member_meta_under(
            &archive.path,
            limits.deadline,
            cancelled,
            &mut |meta| {
                selected_paths.insert(meta.path.to_owned());
                Ok(())
            },
        )
        .map_err(|_| SourceCommandError::Conflict("Collection archive prefix incomplete"))?;
    if expected_paths.is_empty() || !selected_paths.is_empty() && selected_paths != expected_paths {
        return Err(SourceCommandError::Conflict(
            "Collection archive member set incomplete or unexpected",
        ));
    }
    for (name, expected) in &archive.members {
        let path = format!("{}/{name}", archive.path);
        if let Some(selected) =
            selected_optional_member(input, &path, 16_777_216, limits.deadline, cancelled)?
        {
            if &selected != expected {
                return Err(SourceCommandError::Conflict(
                    "Collection archive bytes changed",
                ));
            }
        } else {
            plan.files.push(work_transaction::SelectedFile {
                path: relative(&path)?,
                before: None,
                after: Some(expected.clone()),
            });
        }
    }
    Ok(())
}

fn prefix_has_member(
    input: &dyn SourceCutInput,
    prefix: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    let mut found = false;
    input
        .for_each_current_member_meta_under(prefix, limits.deadline, cancelled, &mut |_| {
            found = true;
            Ok(())
        })
        .map_err(|_| SourceCommandError::Conflict("Collection output prefix incomplete"))?;
    Ok(found)
}

fn include_missing_output_directories(
    input: &dyn SourceCutInput,
    plan: &mut work_transaction::WorkPlan,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let existing = plan
        .files
        .iter()
        .filter(|file| file.before.is_some())
        .map(|file| file.path.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut directories = plan
        .new_directories
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    for path in plan
        .files
        .iter()
        .filter(|file| file.before.is_none())
        .map(|file| file.path.as_str().to_owned())
    {
        let mut parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_owned())
            .ok_or(SourceCommandError::Invalid("Collection output parent"))?;
        while parent.starts_with("ToS/source-witnesses/") {
            if existing
                .iter()
                .any(|member| member.starts_with(&format!("{parent}/")))
            {
                break;
            }
            if !prefix_has_member(input, &parent, limits, cancelled)? {
                directories.insert(parent.clone());
            } else {
                break;
            }
            if parent == "ToS/source-witnesses" {
                break;
            }
            parent = parent
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_owned())
                .ok_or(SourceCommandError::Invalid("Collection output ancestor"))?;
        }
    }
    plan.new_directories = directories
        .into_iter()
        .map(|path| relative(&path))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(())
}

fn verify_new_paths_absent(
    input: &dyn SourceCutInput,
    plan: &work_transaction::WorkPlan,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for file in &plan.files {
        if file.before.is_some() {
            continue;
        }
        if input
            .path_presence(file.path.as_str(), deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Collection output presence read"))?
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "Collection output path occupied",
            ));
        }
    }
    Ok(())
}

/// Re-derive Collection membership indexes from the exact sealed successor
/// writes. Recovery must not trust the preparation-time index snapshot.
pub(crate) fn expected_collection_membership_revision_indexes(
    writes: &[SourceRevisionWrite],
) -> crate::DurableResult<IndexRows> {
    use crate::DurableError;

    if writes.is_empty() || writes.len() > 512 {
        return Err(DurableError::Corrupt(
            "PG Collection membership write count differs",
        ));
    }
    let mut outputs = BTreeMap::<String, &[u8]>::new();
    let mut indexes = writes
        .iter()
        .filter(|write| write.expected_predecessor.is_none())
        .map(|write| {
            (
                "path".to_owned(),
                write.subject.clone(),
                write.subject.clone(),
            )
        })
        .collect::<IndexRows>();
    for write in writes {
        if !write.subject.starts_with("ToS/source-witnesses/")
            || outputs
                .insert(write.subject.clone(), write.exact_bytes.as_slice())
                .is_some()
        {
            return Err(DurableError::Corrupt(
                "PG Collection membership writes are duplicated or outside source homes",
            ));
        }
    }

    let collection_rows = outputs
        .iter()
        .filter(|(path, _)| {
            path.starts_with("ToS/source-witnesses/collections/")
                && path.ends_with("/collection.json")
        })
        .collect::<Vec<_>>();
    if collection_rows.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG Collection membership record write coverage differs",
        ));
    }
    let (collection_path, collection_raw) = collection_rows[0];
    let collection = cmd::parse(collection_raw).map_err(DurableError::Source)?;
    if cmd::text(&collection, "record_type").map_err(DurableError::Source)? != "collection"
        || writes
            .iter()
            .find(|write| write.subject.as_str() == collection_path.as_str())
            .is_none_or(|write| write.expected_predecessor.is_none())
    {
        return Err(DurableError::Corrupt(
            "PG Collection membership predecessor write differs",
        ));
    }
    let collection_id = cmd::text(&collection, "record_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let collection_home = collection_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Corrupt(
            "PG Collection membership home is absent",
        ))?;

    let claim_rows = outputs
        .iter()
        .filter(|(path, _)| {
            path.starts_with("ToS/source-witnesses/relations/")
                && path.ends_with("/source-claims.jsonl")
        })
        .collect::<Vec<_>>();
    if claim_rows.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG Collection membership Claim stream coverage differs",
        ));
    }
    let (claim_path, claim_raw) = claim_rows[0];
    let claim_path_string = claim_path.to_owned();
    let claim_home =
        claim_path
            .rsplit_once('/')
            .map(|(home, _)| home)
            .ok_or(DurableError::Corrupt(
                "PG Collection membership Claim home is absent",
            ))?;
    let claim_write = writes
        .iter()
        .find(|write| write.subject == claim_path_string)
        .ok_or(DurableError::Corrupt(
            "PG Collection membership Claim stream write absent",
        ))?;
    if claim_write.expected_predecessor.is_some() {
        return Err(DurableError::Corrupt(
            "PG Collection membership Claim owner write differs",
        ));
    }
    let claims = catalog_lines(claim_raw)
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if claims.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG Collection membership Claim cardinality differs",
        ));
    }
    let claim = &claims[0];
    let claim_id = cmd::text(claim, "claim_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let event_id = cmd::text(claim, "provenance_event_ref")
        .map_err(DurableError::Source)?
        .to_owned();
    if cmd::text(claim, "predicate").map_err(DurableError::Source)? != "contains_work"
        || cmd::text(claim, "subject_ref").map_err(DurableError::Source)? != collection_id
    {
        return Err(DurableError::Corrupt(
            "PG Collection membership Claim identity differs",
        ));
    }
    let membership_refs =
        cmd::array(&collection, "membership_claim_refs").map_err(DurableError::Source)?;
    let mut unique_membership_refs = BTreeSet::new();
    for reference in membership_refs {
        let reference = reference.as_str().ok_or(DurableError::Corrupt(
            "PG Collection membership reference differs",
        ))?;
        if !unique_membership_refs.insert(reference) {
            return Err(DurableError::Corrupt(
                "PG Collection membership reference is duplicated",
            ));
        }
    }
    if !unique_membership_refs.contains(claim_id.as_str()) {
        return Err(DurableError::Corrupt(
            "PG Collection successor omits its membership Claim",
        ));
    }

    let expected_event_path = format!("{claim_home}/source-create-provenance.jsonl");
    let event_rows = outputs
        .iter()
        .filter(|(path, _)| path.ends_with("/source-create-provenance.jsonl"))
        .collect::<Vec<_>>();
    if event_rows.len() != 1 || event_rows[0].0.as_str() != expected_event_path {
        return Err(DurableError::Corrupt(
            "PG Collection membership event write coverage differs",
        ));
    }
    let event_raw = event_rows[0].1;
    let event_write = writes
        .iter()
        .find(|write| write.subject == expected_event_path)
        .ok_or(DurableError::Corrupt(
            "PG Collection membership event write absent",
        ))?;
    if event_write.expected_predecessor.is_some() {
        return Err(DurableError::Corrupt(
            "PG Collection membership event predecessor differs",
        ));
    }
    let events = catalog_lines(event_raw)
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if events.len() != 1
        || cmd::text(&events[0], "event_id").map_err(DurableError::Source)? != event_id
    {
        return Err(DurableError::Corrupt(
            "PG Collection membership event identity differs",
        ));
    }

    indexes.insert((
        "metadata".to_owned(),
        collection_id.clone(),
        (*collection_path).clone(),
    ));
    indexes.insert(("claim".to_owned(), claim_id.clone(), claim_path_string));
    indexes.insert(("event".to_owned(), event_id, expected_event_path));

    let expected_collection_forms = format!("{collection_home}/collection.human-forms.json");
    let expected_claim_forms = format!(
        "{claim_home}/source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    );
    let form_paths = outputs
        .iter()
        .filter(|(path, _)| path.ends_with(".human-forms.json"))
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    if form_paths.len() != 2
        || !form_paths
            .iter()
            .any(|(path, _)| *path == expected_collection_forms)
        || !form_paths
            .iter()
            .any(|(path, _)| *path == expected_claim_forms)
    {
        return Err(DurableError::Corrupt(
            "PG Collection membership form write coverage differs",
        ));
    }
    for (path, raw) in form_paths {
        let subject_id = if path == expected_collection_forms {
            collection_id.as_str()
        } else {
            claim_id.as_str()
        };
        let set = cmd::parse(raw).map_err(DurableError::Source)?;
        for section in ["forms", "prior_forms"] {
            if let Some(values) = set.object_get(section) {
                let values = values
                    .as_array()
                    .ok_or(DurableError::Corrupt("PG Collection form list differs"))?;
                for form in values {
                    if cmd::text(
                        cmd::field(form, "subject").map_err(DurableError::Source)?,
                        "id",
                    )
                    .map_err(DurableError::Source)?
                        != subject_id
                    {
                        return Err(DurableError::Corrupt(
                            "PG Collection membership form subject differs",
                        ));
                    }
                    indexes.insert((
                        "form".to_owned(),
                        cmd::text(form, "form_id")
                            .map_err(DurableError::Source)?
                            .to_owned(),
                        path.to_owned(),
                    ));
                }
            }
        }
    }
    Ok(indexes)
}

/// Rebuild the Collection owner projection for registration and recovery from
/// the sealed bytes, exact owner context, and the recorded command response.
pub(crate) fn expected_collection_inventory_contributions(
    writes: &[SourceRevisionWrite],
    context: &CommandContext,
    response: &JsonValue,
) -> crate::DurableResult<BTreeMap<String, JsonValue>> {
    use crate::DurableError;

    expected_collection_membership_revision_indexes(writes)?;
    let scope = cmd::field(response, "scope").map_err(DurableError::Source)?;
    let collection_path =
        cmd::text(response, "collection_source_path").map_err(DurableError::Source)?;
    let claim_path = cmd::text(response, "claim_source_path").map_err(DurableError::Source)?;
    let collection_id = cmd::text(scope, "collection_id").map_err(DurableError::Source)?;
    let claim_id = cmd::text(scope, "claim_id").map_err(DurableError::Source)?;
    let work_id = cmd::text(scope, "work_id").map_err(DurableError::Source)?;
    let event_id = cmd::text(scope, "provenance_event_id").map_err(DurableError::Source)?;

    let collection_write = writes
        .iter()
        .find(|write| write.subject == collection_path)
        .ok_or(DurableError::Corrupt(
            "PG Collection owner record write absent",
        ))?;
    let collection = cmd::parse(&collection_write.exact_bytes).map_err(DurableError::Source)?;
    if cmd::text(&collection, "record_type").map_err(DurableError::Source)? != "collection"
        || cmd::text(&collection, "record_id").map_err(DurableError::Source)? != collection_id
    {
        return Err(DurableError::Corrupt(
            "PG Collection owner record identity differs",
        ));
    }
    let claim_write = writes
        .iter()
        .find(|write| write.subject == claim_path)
        .ok_or(DurableError::Corrupt(
            "PG Collection owner Claim write absent",
        ))?;
    let claims = catalog_lines(&claim_write.exact_bytes)
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if claims.len() != 1
        || cmd::text(&claims[0], "claim_id").map_err(DurableError::Source)? != claim_id
        || cmd::text(&claims[0], "subject_ref").map_err(DurableError::Source)? != collection_id
        || cmd::text(&claims[0], "object").map_err(DurableError::Source)? != work_id
        || cmd::text(&claims[0], "predicate").map_err(DurableError::Source)? != "contains_work"
        || cmd::text(&claims[0], "provenance_event_ref").map_err(DurableError::Source)? != event_id
    {
        return Err(DurableError::Corrupt(
            "PG Collection owner Claim identity differs",
        ));
    }
    let claim_home =
        claim_path
            .rsplit_once('/')
            .map(|(home, _)| home)
            .ok_or(DurableError::Corrupt(
                "PG Collection owner Claim home absent",
            ))?;
    let event_path = format!("{claim_home}/source-create-provenance.jsonl");
    let record = crate::source_creation_store::PgGrowthRecordProjection {
        kind: "collection".to_owned(),
        path: collection_path.to_owned(),
        record: collection,
        schema_ref: Some("ToS/contracts/corpus-record.schema.json".to_owned()),
    };
    if !context
        .files
        .iter()
        .any(|file| file.path.as_str() == "ToS/contracts/corpus-record.schema.json")
    {
        return Err(DurableError::Corrupt(
            "PG Collection owner record schema absent from context",
        ));
    }
    crate::source_creation_store::pg_growth_owner_contributions(
        writes,
        &context.files,
        &[record],
        &event_path,
        claim_path,
        "tos_managed_collection_membership_claim_owner_inventory_v1",
    )
}
