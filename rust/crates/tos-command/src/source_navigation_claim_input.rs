//! Exact selected-file assembler for one event-local Claim contribution.
//!
//! The PG owner selects the bounded source closure; this module binds those
//! bytes to the existing maintained Claim renderer without inventing a source
//! revision or walking the authored corpus.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tos_compiler::{
    SelectedBibliographicFilesV1, resolve_selected_claim_version_v1,
    resolve_selected_metadata_history_v1, selected_metadata_record_schema_path_v1,
    source_bibliographic::{
        BibliographicDocumentLimits, BibliographicForms, BibliographicLimits,
        SuppliedBibliographicClaimInputs, SuppliedEvidenceResolution, materialize_checked_form_set,
        supplied_bibliographic_endpoint_matches, supplied_bibliographic_event,
        supplied_bibliographic_evidence, supplied_bibliographic_identity,
        supplied_bibliographic_literal, supplied_bibliographic_maker_with_fallback,
        supplied_bibliographic_path_evidence, supplied_claim_navigation_descriptor,
        supplied_claim_reference_members, supplied_retained_object_link_context,
        validate_supplied_catalogue_attribution,
    },
    source_navigation_claim_event::{
        NavigationClaimEventBudgetCostV1, NavigationClaimEventCommonInputV1,
        NavigationClaimEventOwnerInventoryV1, NavigationClaimEventRangeManifestV1,
        NavigationClaimEventRangeOwnerCohortV1, NavigationClaimEventRangeOwnerInputV1,
        NavigationClaimEventSourceFileV1, NavigationClaimIdentitySourceV1,
        project_navigation_claim_event_range_owner_with_inventory_v1,
    },
    source_navigation_event::{
        NavigationDependencyRoleV1, NavigationDependencyStateV1, NavigationDependencyV1,
    },
    source_witness_catalog::{
        CandidateEventSourceIdentityV1, SourceCatalogValidator, render_catalog_claim,
        render_catalog_record,
    },
};
use tos_foundation::{Digest256, RelativePath};

use crate::{
    DurableError, DurableNavigationEventContextV1, DurableResult,
    source_navigation_claim_selection::{
        ClaimEventIdentityOwnerV1, canonical_serde_source_record_v1, relation_profile,
    },
    source_navigation_producer::{NavigationEventSourceCutIdentityV1, compiler_refusal},
};

const MAX_PATHS: usize = 4096;
const MAX_DOC_BYTES: usize = 2 * 1024 * 1024;
const CLAIM_SCHEMA: &str = "ToS/contracts/source-claim-record.schema.json";
const HISTORICAL_CLAIM_SCHEMA: &str = "ToS/contracts/historical-claim.schema.json";
const EVENT_SCHEMA_V1: &str = "ToS/contracts/provenance-event.schema.json";
const EVENT_SCHEMA_V2: &str = "ToS/contracts/provenance-event-v2.schema.json";
const ENTITY_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const ENTITY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const RELATION_REGISTRY: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const RELATION_SCHEMA: &str = "ToS/contracts/semantic-relation-type-registry.schema.json";
const FORMS_SCHEMA: &str = "ToS/contracts/human-form-set.schema.json";
const ANCHOR_SCHEMA: &str = "ToS/contracts/source-anchor.schema.json";
const CLAIM_DISPLAY_FIELDS_SCHEMA: &str = "ToS/contracts/claim-display-fields.schema.json";

struct OwnedIdentitySource<'a> {
    path: String,
    schema_path: String,
    forms_path: String,
    schema_bytes: &'a [u8],
    source_bytes: &'a [u8],
    forms_bytes: Option<&'a [u8]>,
    forms_schema_bytes: Option<&'a [u8]>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble_selected_claim_event_range_owner_v1<'a, 'validator>(
    context: &'a DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    validator: &'a SourceCatalogValidator<'validator>,
    files: &'a SelectedBibliographicFilesV1,
    inventory: &'a NavigationClaimEventOwnerInventoryV1,
    manifest: &'a NavigationClaimEventRangeManifestV1,
    identity_owners: &'a [ClaimEventIdentityOwnerV1],
    owner_stream_path: &'a str,
    claim_id: &'a str,
    predecessor_cohort_root: Option<Digest256>,
    forms_materializer: &'a mut dyn BibliographicForms,
    limits: BibliographicLimits,
) -> DurableResult<(
    NavigationClaimEventRangeOwnerCohortV1,
    NavigationClaimEventBudgetCostV1,
)> {
    manifest
        .verify_integrity()
        .map_err(|_| DurableError::Conflict("Claim range manifest differs"))?;
    if context.event_sequence() != identity.event_sequence()
        || context.source_head_sequence() != identity.source_head_sequence()
        || manifest.coverage().through_sequence() != context.event_sequence()
        || manifest.coverage().terminal_event_id() != context.event_id()
        || manifest.coverage().cursor_before() != context.consumer_cursor().sequence
        || identity.range_coverage_binding() != Some(manifest.coverage().coverage_root())
    {
        return Err(DurableError::Conflict(
            "selected Claim event identity differs",
        ));
    }
    let predecessor_source_revision =
        context
            .consumer_cursor()
            .source_revision
            .ok_or(DurableError::Conflict(
                "navigation Claim consumer source revision absent",
            ))?;
    let owner = inventory
        .owner(owner_stream_path, claim_id)
        .ok_or(DurableError::Conflict(
            "selected Claim owner is absent from exact inventory",
        ))?;
    let owner_ids = inventory
        .owner_ids(owner_stream_path)
        .ok_or(DurableError::Conflict(
            "selected Claim stream is absent from exact inventory",
        ))?;
    let source_path = owner.source_path;
    let source_bytes = files
        .required(source_path)
        .map_err(|_| DurableError::Invalid("selected Claim stream bytes absent"))?;
    let claim = owner.claim;
    let claim_schema_bytes = files
        .required(CLAIM_SCHEMA)
        .map_err(|_| DurableError::Invalid("selected Claim schema absent"))?;
    let entity_bytes = files
        .required(ENTITY_REGISTRY)
        .map_err(|_| DurableError::Invalid("selected Claim entity registry absent"))?;
    let relation_bytes = files
        .required(RELATION_REGISTRY)
        .map_err(|_| DurableError::Invalid("selected Claim relation registry absent"))?;
    let entity_schema_bytes = files
        .required(ENTITY_SCHEMA)
        .map_err(|_| DurableError::Invalid("selected Claim entity schema absent"))?;
    let relation_schema_bytes = files
        .required(RELATION_SCHEMA)
        .map_err(|_| DurableError::Invalid("selected Claim relation schema absent"))?;
    let entities = parse_value(entity_bytes, limits.catalog.max_row_bytes)?;
    let relations = parse_value(relation_bytes, limits.catalog.max_row_bytes)?;
    let entry = render_catalog_claim(
        claim,
        source_path,
        owner.source_line,
        Some(CLAIM_SCHEMA),
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)?;
    let history_path = sibling(source_path, "claim-revision-history.json")?;
    let history_bytes = files
        .optional(&history_path)
        .map_err(|_| DurableError::Invalid("selected Claim history observation absent"))?;
    let provenance_event_path = sibling(source_path, "source-create-provenance.jsonl")?;
    let provenance_receipt_path = sibling(source_path, "source-create-receipt.json")?;
    let provenance_event_bytes = files
        .required(&provenance_event_path)
        .map_err(|_| DurableError::Invalid("selected Claim create event absent"))?;
    let provenance_receipt_bytes = files
        .required(&provenance_receipt_path)
        .map_err(|_| DurableError::Invalid("selected Claim create receipt absent"))?;
    let (provenance_event, provenance_location) = find_jsonl(
        provenance_event_bytes,
        &provenance_event_path,
        "event_id",
        claim
            .get("provenance_event_ref")
            .and_then(Value::as_str)
            .ok_or(DurableError::Invalid(
                "selected Claim provenance event reference absent",
            ))?,
        limits.catalog.max_row_bytes,
    )?;
    let event_schema_path = if provenance_event
        .get("schema_version")
        .and_then(Value::as_str)
        == Some("tos_provenance_event_v2")
    {
        EVENT_SCHEMA_V2
    } else {
        EVENT_SCHEMA_V1
    };
    let event_schema_bytes = files
        .required(event_schema_path)
        .map_err(|_| DurableError::Invalid("selected Claim provenance schema absent"))?;
    let event = supplied_bibliographic_event(
        &provenance_event,
        &provenance_location,
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)?;

    let claim_forms_path = claim_form_path(source_path, claim_id)?;
    let claim_forms_bytes = files
        .optional(&claim_forms_path)
        .map_err(|_| DurableError::Invalid("selected Claim forms observation absent"))?;
    let forms_schema_bytes = if claim_forms_bytes.is_some() {
        Some(
            files
                .required(FORMS_SCHEMA)
                .map_err(|_| DurableError::Invalid("selected Claim forms schema absent"))?,
        )
    } else {
        None
    };
    let claim_forms = match claim_forms_bytes {
        Some(raw) => {
            materialize_checked_form_set(raw, &claim_forms_path, claim, forms_materializer, limits)
                .map_err(compiler_refusal)?
        }
        None => None,
    };

    let mut required_identity_ids = BTreeSet::new();
    required_identity_ids.insert(text(claim, "subject_ref")?.to_owned());
    if let Some(value) = claim.get("object").and_then(Value::as_str)
        && value.starts_with("tos.")
    {
        required_identity_ids.insert(value.to_owned());
    }
    if let Some(value) = claim.pointer("/maker/agent_ref").and_then(Value::as_str) {
        required_identity_ids.insert(value.to_owned());
    }
    let profiled = source_path.ends_with("/source-claims.jsonl");
    let relation_profile = if profiled {
        relation_profile(&relations, text(claim, "predicate")?)
            .map_err(|_| DurableError::Conflict("Claim relation profile is ambiguous"))?
    } else {
        None
    };
    let transition = relation_profile
        .and_then(|profile| profile.get("reader").and_then(Value::as_str))
        .is_some_and(|reader| {
            matches!(reader, "identity-transition-v1" | "identity-transition-v2")
        });
    if let Some(profile) = relation_profile {
        for reference in
            supplied_claim_reference_members(claim, profile, limits.catalog.max_row_bytes)
                .map_err(compiler_refusal)?
        {
            required_identity_ids.insert(reference.to_owned());
        }
    }
    add_normalized_refs(claim, relation_profile, &mut required_identity_ids)?;
    for reference in claim
        .get("evidence_refs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            claim
                .get("counterevidence_refs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .filter_map(Value::as_str)
    {
        if identity_owners
            .iter()
            .any(|owner| owner.id == reference && owner.kind.as_deref() == Some("metadata"))
        {
            required_identity_ids.insert(reference.to_owned());
        }
    }
    let mut identity_sources_owned = Vec::<OwnedIdentitySource<'_>>::new();
    let mut identity_nodes = BTreeMap::<String, Value>::new();
    for id in &required_identity_ids {
        if let Some((node, source)) = resolve_metadata_identity(
            id,
            identity_owners,
            files,
            validator,
            identity.event_source_revision(),
            forms_materializer,
            limits,
        )? {
            identity_nodes.insert(id.clone(), node);
            identity_sources_owned.push(source);
        }
    }
    let subject_ref = text(claim, "subject_ref")?;
    let subject = identity_nodes
        .get(subject_ref)
        .cloned()
        .ok_or(DurableError::Conflict(
            "selected Claim subject identity absent",
        ))?;
    let object = match claim.get("object") {
        Some(Value::String(reference)) => match identity_nodes.get(reference) {
            Some(node) => node.clone(),
            None if reference.starts_with("tos.") => {
                return Err(DurableError::Conflict(
                    "Claim identity-like object is unresolved",
                ));
            }
            None => supplied_bibliographic_literal(
                &entry,
                claim,
                BibliographicDocumentLimits {
                    input_document_bytes: limits.catalog.max_row_bytes,
                    output_row_bytes: limits.catalog.max_output_row_bytes,
                },
            )
            .map_err(compiler_refusal)?,
        },
        _ => supplied_bibliographic_literal(
            &entry,
            claim,
            BibliographicDocumentLimits {
                input_document_bytes: limits.catalog.max_row_bytes,
                output_row_bytes: limits.catalog.max_output_row_bytes,
            },
        )
        .map_err(compiler_refusal)?,
    };

    let predicate = text(claim, "predicate")?;
    if !matches!(
        claim.get("claim_type").and_then(Value::as_str),
        Some("bibliographic" | "relation")
    ) {
        return Err(DurableError::Invalid("selected Claim type differs"));
    }
    let legacy_link =
        source_path == "ToS/source-witnesses/relations/object-link/object-link-claims.jsonl";
    let historical = matches!(
        predicate,
        "historical_participant" | "historical_place" | "historical_work" | "historical_dating"
    );
    if !profiled
        && !legacy_link
        && claim.get("claim_type").and_then(Value::as_str) == Some("relation")
        && predicate != "is_derivative_of"
        && !historical
    {
        return Err(DurableError::Refused(
            "legacy relation is outside selected Claim profile",
        ));
    }
    if !profiled
        && !legacy_link
        && !matches!(
            claim.get("assertion_layer").and_then(Value::as_str),
            Some("bibliographic_assertion" | "scholarly_report")
        )
    {
        return Err(DurableError::Refused(
            "legacy Claim assertion layer differs",
        ));
    }
    if claim
        .pointer("/qualifiers/display_fields/schema_version")
        .and_then(Value::as_str)
        == Some("tos_claim_display_fields_v1")
    {
        let display_schema = files
            .required(CLAIM_DISPLAY_FIELDS_SCHEMA)
            .map_err(|_| DurableError::Invalid("selected Claim display-fields schema absent"))?;
        let display_bytes = canonical_serde_source_record_v1(&claim["qualifiers"], MAX_DOC_BYTES)?;
        validator
            .check_selected_event_document_exact(
                identity.event_source_revision(),
                source_path,
                CLAIM_DISPLAY_FIELDS_SCHEMA,
                display_schema,
                &display_bytes,
            )
            .map_err(compiler_refusal)?;
    }
    if profiled || historical {
        let relation = relation_entry(&relations, predicate)?;
        if !transition
            && !supplied_bibliographic_endpoint_matches(
                &subject,
                &relation["domain_type_ids"],
                &entities,
                BibliographicDocumentLimits {
                    input_document_bytes: limits.catalog.max_row_bytes,
                    output_row_bytes: limits.catalog.max_output_row_bytes,
                },
            )
            .map_err(compiler_refusal)?
        {
            return Err(DurableError::Conflict("Claim subject domain violation"));
        }
        if object.get("node_kind").and_then(Value::as_str) == Some("identity")
            && !transition
            && !supplied_bibliographic_endpoint_matches(
                &object,
                &relation["range_type_ids"],
                &entities,
                BibliographicDocumentLimits {
                    input_document_bytes: limits.catalog.max_row_bytes,
                    output_row_bytes: limits.catalog.max_output_row_bytes,
                },
            )
            .map_err(compiler_refusal)?
        {
            return Err(DurableError::Conflict("Claim object range violation"));
        }
    }

    let mut members = Vec::new();
    if let Some(profile) = relation_profile {
        for reference in
            supplied_claim_reference_members(claim, profile, limits.catalog.max_row_bytes)
                .map_err(compiler_refusal)?
        {
            let node = identity_nodes
                .get(reference)
                .cloned()
                .ok_or(DurableError::Conflict(
                    "selected Claim member identity absent",
                ))?;
            if let Some(types) = profile.pointer("/object_reference_set/member_type_ids")
                && !transition
                && !supplied_bibliographic_endpoint_matches(
                    &node,
                    types,
                    &entities,
                    BibliographicDocumentLimits {
                        input_document_bytes: limits.catalog.max_row_bytes,
                        output_row_bytes: limits.catalog.max_output_row_bytes,
                    },
                )
                .map_err(compiler_refusal)?
            {
                return Err(DurableError::Conflict("Claim member domain violation"));
            }
            if transition && !transition_participant_eligible(&node, profile, &entities) {
                return Err(DurableError::Conflict(
                    "Claim transition participant role differs",
                ));
            }
            if members.len() >= limits.max_claim_cohort_rows {
                return Err(DurableError::Refused("selected Claim member count budget"));
            }
            members.push(node);
        }
    }

    validate_supplied_catalogue_attribution(claim).map_err(compiler_refusal)?;
    let maker_identity = claim
        .pointer("/maker/agent_ref")
        .and_then(Value::as_str)
        .and_then(|reference| identity_nodes.get(reference).cloned());
    let (maker, maker_identity) = supplied_bibliographic_maker_with_fallback(
        claim,
        maker_identity,
        entry
            .get("source_claim_file_ref")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("Claim entry source path absent"))?,
        entry
            .get("claim_sha256")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("Claim entry digest absent"))?,
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)?;
    let mut evidence = Vec::new();
    let mut counterevidence = Vec::new();
    for (field, output) in [
        ("evidence_refs", &mut evidence),
        ("counterevidence_refs", &mut counterevidence),
    ] {
        if let Some(values) = claim.get(field).and_then(Value::as_array) {
            for value in values {
                if output.len() >= limits.max_claim_cohort_rows {
                    return Err(DurableError::Refused(
                        "selected Claim evidence count budget",
                    ));
                }
                let reference = value
                    .as_str()
                    .ok_or(DurableError::Invalid("Claim evidence reference type"))?;
                output.push(resolve_evidence(
                    reference,
                    claim,
                    &entry,
                    &provenance_event_path,
                    provenance_event_bytes,
                    &identity_nodes,
                    identity_owners,
                    files,
                    validator,
                    identity.event_source_revision(),
                    limits,
                )?);
            }
        }
    }
    let normalized = normalized_nodes(claim, relation_profile, &identity_nodes, &entities)?;
    let legacy_context =
        if source_path == "ToS/source-witnesses/relations/object-link/object-link-claims.jsonl" {
            Some(
                supplied_retained_object_link_context(
                    claim,
                    source_path,
                    predicate,
                    &subject,
                    &object,
                    &entry["source_claim_line"],
                    &entry["claim_sha256"],
                )
                .map_err(compiler_refusal)?,
            )
        } else {
            None
        };
    let collection_order_basis = if relation_profile
        .and_then(|profile| profile.pointer("/object_reference_set/basis_adapter"))
        .and_then(Value::as_str)
        == Some("collection-membership-versions-v1")
    {
        Some(collection_order_basis(
            claim,
            identity_owners,
            inventory,
            files,
            validator,
            identity.event_source_revision(),
            forms_materializer,
            limits,
        )?)
    } else {
        None
    };
    let descriptor = supplied_claim_navigation_descriptor(
        claim,
        &subject,
        &object,
        &relations,
        &entities,
        BibliographicDocumentLimits {
            input_document_bytes: limits.catalog.max_row_bytes,
            output_row_bytes: limits.catalog.max_output_row_bytes,
        },
    )
    .map_err(compiler_refusal)?;
    let claim_inputs = SuppliedBibliographicClaimInputs {
        entry: &entry,
        claim,
        subject,
        object,
        event,
        maker,
        maker_identity,
        evidence,
        counterevidence,
        members,
        normalized,
        descriptor,
        forms: claim_forms,
        collection_order_basis,
        legacy_context,
    };
    let mut identity_sources = identity_sources_owned
        .iter()
        .map(|owned| NavigationClaimIdentitySourceV1 {
            path: &owned.path,
            schema_path: &owned.schema_path,
            schema_bytes: owned.schema_bytes,
            source_bytes: owned.source_bytes,
            forms_path: &owned.forms_path,
            forms_schema_path: owned.forms_schema_bytes.map(|_| FORMS_SCHEMA),
            forms_schema_bytes: owned.forms_schema_bytes,
            forms_bytes: owned.forms_bytes,
        })
        .collect::<Vec<_>>();
    identity_sources.sort_by(|a, b| a.path.cmp(b.path));
    let (source_files, dependencies) = selected_files_and_dependencies(
        context,
        files,
        source_path,
        &provenance_event_path,
        event_schema_path,
        &provenance_receipt_path,
        &claim_forms_path,
        history_path.as_str(),
        identity_owners,
    )?;
    let common = NavigationClaimEventCommonInputV1 {
        validator,
        event_id: context.event_id(),
        event_sequence: context.event_sequence(),
        // The range renderer validates its event-cut input at the prefix;
        // `identity` still binds the distinct held current source head.
        source_head_sequence: context.event_sequence(),
        predecessor_source_revision,
        predecessor_cohort_root,
        source_path,
        source_line: owner.source_line,
        source_bytes,
        owner_claim_ids: &owner_ids,
        claim_schema_path: CLAIM_SCHEMA,
        claim_schema_bytes,
        forms_path: &claim_forms_path,
        forms_schema_path: FORMS_SCHEMA,
        forms_schema_bytes,
        forms_bytes: claim_forms_bytes,
        history_path: &history_path,
        history_bytes,
        provenance_event_path: &provenance_event_path,
        provenance_event_bytes,
        provenance_event_schema_path: event_schema_path,
        provenance_event_schema_bytes: event_schema_bytes,
        provenance_receipt_path: &provenance_receipt_path,
        provenance_receipt_bytes,
        entity_registry_path: ENTITY_REGISTRY,
        entity_registry_bytes: entity_bytes,
        entity_registry_schema_path: ENTITY_SCHEMA,
        entity_registry_schema_bytes: entity_schema_bytes,
        relation_registry_path: RELATION_REGISTRY,
        relation_registry_bytes: relation_bytes,
        relation_registry_schema_path: RELATION_SCHEMA,
        relation_registry_schema_bytes: relation_schema_bytes,
        identity_sources: &identity_sources,
        source_files: &source_files,
        dependencies: &dependencies,
        selected_files: files,
        forms_materializer,
        claim_inputs,
    };
    let input = NavigationClaimEventRangeOwnerInputV1 { common };
    let cohort = project_navigation_claim_event_range_owner_with_inventory_v1(
        input, manifest, inventory, limits,
    )
    .map_err(compiler_refusal)?;
    let cost = cohort.budget_cost().map_err(compiler_refusal)?;
    Ok((cohort, cost))
}

fn resolve_metadata_identity<'a>(
    id: &str,
    identity_owners: &[ClaimEventIdentityOwnerV1],
    files: &'a SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    event_revision: Digest256,
    materializer: &mut dyn BibliographicForms,
    limits: BibliographicLimits,
) -> DurableResult<Option<(Value, OwnedIdentitySource<'a>)>> {
    let Some(owner) = identity_owners.iter().find(|owner| owner.id == id) else {
        return Ok(None);
    };
    if owner.kind.as_deref() != Some("metadata") {
        return Ok(None);
    }
    let source_path = owner.source_path.as_deref().ok_or(DurableError::Conflict(
        "metadata identity owner path absent",
    ))?;
    let source_bytes = files
        .required(source_path)
        .map_err(|_| DurableError::Invalid("selected metadata identity bytes absent"))?;
    if source_bytes.len() > MAX_DOC_BYTES {
        return Err(DurableError::Refused(
            "selected metadata identity byte budget",
        ));
    }
    let source = parse_value(source_bytes, limits.catalog.max_row_bytes)?;
    let identity = source
        .get("record_id")
        .or_else(|| source.get("artifact_id"))
        .or_else(|| source.get("composite_id"))
        .and_then(Value::as_str);
    if identity != Some(id) {
        return Err(DurableError::Conflict(
            "selected metadata identity ID differs",
        ));
    }
    let entities = parse_value(
        files
            .required(ENTITY_REGISTRY)
            .map_err(|_| DurableError::Invalid("selected entity registry absent"))?,
        limits.catalog.max_row_bytes,
    )?;
    let record_type = source
        .get("record_type")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if source.get("artifact_id").is_some() {
                "artifact"
            } else if source.get("composite_id").is_some() {
                "composite"
            } else {
                ""
            }
        });
    if record_type.is_empty() {
        return Err(DurableError::Invalid(
            "selected metadata record type absent",
        ));
    }
    let schema_path =
        selected_metadata_record_schema_path_v1(record_type, source_path, &source, &entities)
            .map_err(compiler_refusal)?;
    let schema_path = schema_path.to_owned();
    let schema_bytes = files
        .required(&schema_path)
        .map_err(|_| DurableError::Invalid("selected metadata schema absent"))?;
    validator
        .check_selected_event_document_exact(
            event_revision,
            source_path,
            &schema_path,
            schema_bytes,
            source_bytes,
        )
        .map_err(compiler_refusal)?;
    let entry = render_catalog_record(
        &source,
        source_path,
        Some(&schema_path),
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)?;
    let forms_path = identity_forms_path(source_path)?;
    let forms_bytes = files
        .optional(&forms_path)
        .map_err(|_| DurableError::Invalid("selected metadata forms observation absent"))?;
    let forms_schema_bytes = if let Some(raw) = forms_bytes {
        let schema = files
            .required(FORMS_SCHEMA)
            .map_err(|_| DurableError::Invalid("selected forms schema absent"))?;
        validator
            .check_selected_event_document_exact(
                event_revision,
                &forms_path,
                FORMS_SCHEMA,
                schema,
                raw,
            )
            .map_err(compiler_refusal)?;
        Some(schema)
    } else {
        None
    };
    let materialized = match forms_bytes {
        Some(raw) => materialize_checked_form_set(raw, &forms_path, &source, materializer, limits)
            .map_err(compiler_refusal)?,
        None => None,
    };
    let node = supplied_bibliographic_identity(
        &entry,
        &source,
        materialized
            .as_ref()
            .map(|(path, value)| (path.as_str(), value)),
        BibliographicDocumentLimits {
            input_document_bytes: limits.catalog.max_row_bytes,
            output_row_bytes: limits.catalog.max_output_row_bytes,
        },
    )
    .map_err(compiler_refusal)?;
    Ok(Some((
        node,
        OwnedIdentitySource {
            path: source_path.to_owned(),
            schema_path,
            forms_path,
            schema_bytes,
            source_bytes,
            forms_bytes,
            forms_schema_bytes,
        },
    )))
}

fn identity_forms_path(source_path: &str) -> DurableResult<String> {
    let stem = source_path
        .strip_suffix(".json")
        .or_else(|| source_path.strip_suffix(".jsonl"))
        .ok_or(DurableError::Invalid(
            "selected metadata identity source extension",
        ))?;
    Ok(format!("{stem}.human-forms.json"))
}

fn find_jsonl(
    raw: &[u8],
    path: &str,
    id_field: &str,
    id: &str,
    max_bytes: usize,
) -> DurableResult<(Value, Value)> {
    if raw.len() > 16 * 1024 * 1024 {
        return Err(DurableError::Refused(
            "selected provenance file byte budget",
        ));
    }
    let mut found = None;
    for (index, line) in raw.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let bytes = line.strip_suffix(b"\r").unwrap_or(line);
        let value = parse_value(bytes, max_bytes)?;
        if value.get(id_field).and_then(Value::as_str) != Some(id) {
            continue;
        }
        let canonical = canonical_serde_source_record_v1(&value, max_bytes)?;
        let location = json!({
            "source_ref":path,
            "source_line":index as u64 + 1,
            "canonical_sha256":Digest256::of_bytes(&canonical).to_hex()
        });
        if found.replace((value, location)).is_some() {
            return Err(DurableError::Conflict(
                "selected source identity is ambiguous",
            ));
        }
    }
    found.ok_or(DurableError::Conflict("selected source identity is absent"))
}

fn relation_entry<'a>(registry: &'a Value, predicate: &str) -> DurableResult<&'a Value> {
    let relations = registry
        .get("relations")
        .and_then(Value::as_array)
        .ok_or(DurableError::Invalid("selected relation registry shape"))?;
    let mut found = None;
    for relation in relations {
        let matches = relation
            .get("source_mappings")
            .and_then(Value::as_array)
            .is_some_and(|mappings| {
                mappings.iter().any(|mapping| {
                    mapping.get("source_graph").and_then(Value::as_str) == Some("source-claims")
                        && mapping.get("source_predicate_id").and_then(Value::as_str)
                            == Some(predicate)
                        && mapping.get("scope").and_then(Value::as_str) == Some("claim-predicate")
                })
            });
        if matches && found.replace(relation).is_some() {
            return Err(DurableError::Conflict(
                "Claim relation mapping is ambiguous",
            ));
        }
    }
    found.ok_or(DurableError::Conflict("Claim relation mapping is absent"))
}

fn transition_participant_eligible(node: &Value, profile: &Value, entities: &Value) -> bool {
    let Some(kind) = node
        .pointer("/properties/identity_kind")
        .and_then(Value::as_str)
    else {
        return false;
    };
    let Some(types) = entities.get("types").and_then(Value::as_array) else {
        return false;
    };
    types.iter().any(|entity| {
        entity.get("abstract").and_then(Value::as_bool) == Some(false)
            && entity
                .get("source_mappings")
                .and_then(Value::as_array)
                .is_some_and(|mappings| {
                    mappings.iter().any(|mapping| {
                        mapping.get("source_graph").and_then(Value::as_str) == Some("source-claims")
                            && mapping.get("source_kind_id").and_then(Value::as_str) == Some(kind)
                    })
                })
            && (entity.get("object_role").and_then(Value::as_str) == Some("identity")
                || profile.get("reader").and_then(Value::as_str) == Some("identity-transition-v2")
                    && entity.get("object_role").and_then(Value::as_str) == Some("semantic")
                    && entity
                        .pointer("/source_record_profile/identity_proposal_adapter")
                        .and_then(Value::as_str)
                        == Some("exact-semantic-metadata-v1"))
    })
}

fn add_normalized_refs(
    claim: &Value,
    profile: Option<&Value>,
    refs: &mut BTreeSet<String>,
) -> DurableResult<()> {
    if claim.get("predicate").and_then(Value::as_str) == Some("provision_activity") {
        for (group, field) in [
            ("places", "normalized_place_ref"),
            ("agents", "normalized_agent_ref"),
        ] {
            if let Some(items) = claim
                .pointer(&format!("/object/{group}"))
                .and_then(Value::as_array)
            {
                for item in items {
                    if let Some(value) = item.get(field).filter(|value| !value.is_null()) {
                        refs.insert(
                            value
                                .as_str()
                                .ok_or(DurableError::Invalid("normalized Claim reference type"))?
                                .to_owned(),
                        );
                    }
                }
            }
        }
    }
    let predicate = claim.get("predicate").and_then(Value::as_str);
    let temporal_profile = profile
        .and_then(|profile| profile.get("reader").and_then(Value::as_str))
        .is_some_and(|reader| {
            matches!(
                reader,
                "historical-temporal-v1" | "document-catalogue-temporal-v1"
            )
        });
    if (predicate == Some("historical_dating") || temporal_profile)
        && claim
            .pointer("/object/relative")
            .is_some_and(|value| !value.is_null())
    {
        if let Some(reference) = claim
            .pointer("/object/relative/anchor_ref")
            .and_then(Value::as_str)
        {
            refs.insert(reference.to_owned());
        }
    }
    Ok(())
}

fn normalized_nodes(
    claim: &Value,
    profile: Option<&Value>,
    identities: &BTreeMap<String, Value>,
    entities: &Value,
) -> DurableResult<Vec<(String, Value)>> {
    let mut values = BTreeSet::new();
    if claim.get("predicate").and_then(Value::as_str) == Some("provision_activity") {
        for (group, field, edge, allowed) in [
            (
                "places",
                "normalized_place_ref",
                "has_normalized_place",
                &["place"][..],
            ),
            (
                "agents",
                "normalized_agent_ref",
                "has_normalized_agent",
                &["agent", "organization"][..],
            ),
        ] {
            if let Some(items) = claim
                .pointer(&format!("/object/{group}"))
                .and_then(Value::as_array)
            {
                for item in items {
                    if let Some(reference) = item.get(field).filter(|v| !v.is_null()) {
                        let reference = reference
                            .as_str()
                            .ok_or(DurableError::Invalid("normalized Claim reference type"))?;
                        let node = identities
                            .get(reference)
                            .ok_or(DurableError::Conflict("normalized Claim identity absent"))?;
                        if !allowed.contains(
                            &node
                                .pointer("/properties/identity_kind")
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                        ) {
                            return Err(DurableError::Conflict(
                                "normalized Claim identity kind differs",
                            ));
                        }
                        values.insert((edge.to_owned(), reference.to_owned()));
                    }
                }
            }
        }
    }
    let predicate = claim.get("predicate").and_then(Value::as_str);
    let temporal_profile = profile
        .and_then(|profile| profile.get("reader").and_then(Value::as_str))
        .is_some_and(|reader| {
            matches!(
                reader,
                "historical-temporal-v1" | "document-catalogue-temporal-v1"
            )
        });
    if (predicate == Some("historical_dating") || temporal_profile)
        && claim
            .pointer("/object/relative")
            .is_some_and(|value| !value.is_null())
    {
        if let Some(reference) = claim
            .pointer("/object/relative/anchor_ref")
            .and_then(Value::as_str)
        {
            let node = identities.get(reference).ok_or(DurableError::Conflict(
                "relative date anchor identity absent",
            ))?;
            if !supplied_bibliographic_endpoint_matches(
                node,
                &json!(["tos.entity.historical-situation"]),
                entities,
                BibliographicDocumentLimits {
                    input_document_bytes: 2 * 1024 * 1024,
                    output_row_bytes: 2 * 1024 * 1024,
                },
            )
            .map_err(compiler_refusal)?
            {
                return Err(DurableError::Conflict(
                    "relative date anchor domain differs",
                ));
            }
            values.insert((
                "has_historical_date_anchor".to_owned(),
                reference.to_owned(),
            ));
        }
    }
    values
        .into_iter()
        .map(|(edge, reference)| {
            Ok((
                edge,
                identities
                    .get(&reference)
                    .cloned()
                    .ok_or(DurableError::Conflict("normalized identity absent"))?,
            ))
        })
        .collect()
}

fn resolve_evidence(
    reference: &str,
    claim: &Value,
    entry: &Value,
    event_path: &str,
    event_bytes: &[u8],
    identity_nodes: &BTreeMap<String, Value>,
    identity_owners: &[ClaimEventIdentityOwnerV1],
    files: &SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    event_revision: Digest256,
    limits: BibliographicLimits,
) -> DurableResult<Value> {
    if reference.starts_with("ToS/") {
        let raw = files
            .required(reference)
            .map_err(|_| DurableError::Invalid("selected path evidence absent"))?;
        return supplied_bibliographic_path_evidence(
            reference,
            raw,
            limits.catalog.max_output_row_bytes,
        )
        .map_err(compiler_refusal);
    }
    if let Some(owner) = identity_owners.iter().find(|owner| owner.id == reference) {
        if let Some(path) = owner.source_path.as_deref() {
            match owner.kind.as_deref() {
                Some("anchor") => {
                    let raw = files.required(path).map_err(|_| {
                        DurableError::Invalid("selected Claim anchor source absent")
                    })?;
                    let (anchor, location) = find_jsonl(
                        raw,
                        path,
                        "anchor_id",
                        reference,
                        limits.catalog.max_row_bytes,
                    )?;
                    let schema = files.required(ANCHOR_SCHEMA).map_err(|_| {
                        DurableError::Invalid("selected Claim anchor schema absent")
                    })?;
                    let anchor_raw =
                        canonical_serde_source_record_v1(&anchor, limits.catalog.max_row_bytes)?;
                    validator
                        .check_selected_event_document_exact(
                            event_revision,
                            path,
                            ANCHOR_SCHEMA,
                            schema,
                            &anchor_raw,
                        )
                        .map_err(|_| {
                            DurableError::Refused("selected Claim anchor schema rejected")
                        })?;
                    return supplied_bibliographic_evidence(
                        reference,
                        claim,
                        entry,
                        SuppliedEvidenceResolution::Anchor {
                            anchor: &anchor,
                            location: &location,
                        },
                        limits.catalog.max_output_row_bytes,
                    )
                    .map_err(compiler_refusal);
                }
                Some("event") => {
                    let raw = files.required(path).map_err(|_| {
                        DurableError::Invalid("selected Claim evidence event absent")
                    })?;
                    let (event, location) = find_jsonl(
                        raw,
                        path,
                        "event_id",
                        reference,
                        limits.catalog.max_row_bytes,
                    )?;
                    return resolve_provenance_evidence(
                        reference,
                        claim,
                        entry,
                        &event,
                        &location,
                        path,
                        files,
                        validator,
                        event_revision,
                        limits,
                    );
                }
                _ => {}
            }
        } else if matches!(owner.kind.as_deref(), Some("anchor" | "event")) {
            return Err(DurableError::Conflict(
                "selected Claim evidence owner path absent",
            ));
        }
    }
    if let Some(identity) = identity_nodes.get(reference) {
        return supplied_bibliographic_evidence(
            reference,
            claim,
            entry,
            SuppliedEvidenceResolution::Identity(identity),
            limits.catalog.max_output_row_bytes,
        )
        .map_err(compiler_refusal);
    }
    if let Ok((event, location)) = find_jsonl(
        event_bytes,
        event_path,
        "event_id",
        reference,
        limits.catalog.max_row_bytes,
    ) {
        return resolve_provenance_evidence(
            reference,
            claim,
            entry,
            &event,
            &location,
            event_path,
            files,
            validator,
            event_revision,
            limits,
        );
    }
    supplied_bibliographic_evidence(
        reference,
        claim,
        entry,
        SuppliedEvidenceResolution::ExternalCitation,
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)
}

fn resolve_provenance_evidence(
    reference: &str,
    claim: &Value,
    entry: &Value,
    event: &Value,
    location: &Value,
    event_path: &str,
    files: &SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    event_revision: Digest256,
    limits: BibliographicLimits,
) -> DurableResult<Value> {
    let schema_path = match event.get("schema_version").and_then(Value::as_str) {
        Some("tos_provenance_event_v1") => EVENT_SCHEMA_V1,
        Some("tos_provenance_event_v2") => EVENT_SCHEMA_V2,
        _ => {
            return Err(DurableError::Invalid(
                "selected Claim evidence event schema",
            ));
        }
    };
    let schema = files
        .required(schema_path)
        .map_err(|_| DurableError::Invalid("selected Claim evidence event schema absent"))?;
    let event_raw = canonical_serde_source_record_v1(event, limits.catalog.max_row_bytes)?;
    validator
        .check_selected_event_document_exact(
            event_revision,
            event_path,
            schema_path,
            schema,
            &event_raw,
        )
        .map_err(compiler_refusal)?;
    if event.get("schema_version").and_then(Value::as_str) == Some("tos_provenance_event_v2")
        && (!matches!(
            event
                .pointer("/rights_and_visibility/content_visibility")
                .and_then(Value::as_str),
            Some("tracked_public_metadata" | "public_content" | "public_synthetic"),
        ) || !tos_validation::provenance_rules::semantic_issues(event, 4096, limits.deadline)
            .map_err(|_| {
                DurableError::Refused("selected Claim evidence event semantics unavailable")
            })?
            .is_empty())
    {
        return Err(DurableError::Refused(
            "selected Claim evidence event is not public and valid",
        ));
    }
    let node = supplied_bibliographic_event(event, location, limits.catalog.max_output_row_bytes)
        .map_err(|_| {
        DurableError::Refused("selected Claim evidence event projection refused")
    })?;
    let event_location = json!({
        "source_ref":location["source_ref"],
        "source_line":location["source_line"],
        "canonical_sha256":location["canonical_sha256"],
        "event_node_id":node["node_id"]
    });
    supplied_bibliographic_evidence(
        reference,
        claim,
        entry,
        SuppliedEvidenceResolution::ProvenanceEvent(&event_location),
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)
}

fn collection_order_basis(
    claim: &Value,
    identity_owners: &[ClaimEventIdentityOwnerV1],
    inventory: &NavigationClaimEventOwnerInventoryV1,
    files: &SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    event_revision: Digest256,
    materializer: &mut dyn BibliographicForms,
    limits: BibliographicLimits,
) -> DurableResult<Value> {
    let object = claim.get("object").ok_or(DurableError::Invalid(
        "collection-order Claim object absent",
    ))?;
    let collection_ref = object
        .get("collection_version")
        .ok_or(DurableError::Invalid(
            "collection-order exact Collection ref absent",
        ))?;
    let collection_id =
        collection_ref
            .get("id")
            .and_then(Value::as_str)
            .ok_or(DurableError::Invalid(
                "collection-order Collection ID absent",
            ))?;
    let collection_owner = identity_owners
        .iter()
        .find(|owner| owner.id == collection_id && owner.kind.as_deref() == Some("metadata"))
        .ok_or(DurableError::Conflict(
            "collection-order Collection owner absent",
        ))?;
    let collection_path = collection_owner
        .source_path
        .as_deref()
        .ok_or(DurableError::Conflict(
            "collection-order Collection path absent",
        ))?;
    let collection_raw = files
        .required(collection_path)
        .map_err(|_| DurableError::Invalid("collection-order Collection bytes absent"))?;
    let collection_record = parse_value(collection_raw, limits.catalog.max_row_bytes)?;
    let entities = parse_value(
        files
            .required(ENTITY_REGISTRY)
            .map_err(|_| DurableError::Invalid("collection-order entity registry absent"))?,
        limits.catalog.max_row_bytes,
    )?;
    let collection_type = collection_record
        .get("record_type")
        .and_then(Value::as_str)
        .ok_or(DurableError::Invalid(
            "collection-order Collection type absent",
        ))?;
    if collection_type != "collection"
        || collection_record.get("record_id").and_then(Value::as_str) != Some(collection_id)
    {
        return Err(DurableError::Conflict(
            "collection-order Collection identity/type differs",
        ));
    }
    let collection_schema_path = selected_metadata_record_schema_path_v1(
        collection_type,
        collection_path,
        &collection_record,
        &entities,
    )
    .map_err(compiler_refusal)?;
    let collection_schema_path = collection_schema_path.to_owned();
    let collection_schema_bytes = files
        .required(&collection_schema_path)
        .map_err(|_| DurableError::Invalid("collection-order Collection schema absent"))?;
    validator
        .check_selected_event_document_exact(
            event_revision,
            collection_path,
            &collection_schema_path,
            collection_schema_bytes,
            collection_raw,
        )
        .map_err(compiler_refusal)?;
    let collection_entry = render_catalog_record(
        &collection_record,
        collection_path,
        Some(&collection_schema_path),
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)?;
    let selected_collection = resolve_selected_metadata_history_v1(
        &collection_entry,
        &entities,
        files,
        validator,
        event_revision,
        limits,
    )
    .map_err(compiler_refusal)?;
    let collection_version = selected_collection
        .versions
        .iter()
        .find(|(reference, _)| reference == collection_ref)
        .map(|(_, resolution)| resolution)
        .ok_or(DurableError::Conflict(
            "exact Collection version is not retained",
        ))?;
    if collection_version.get("status").and_then(Value::as_str) != Some("available")
        || collection_version.get("record").is_none_or(|record| {
            record.get("record_type").and_then(Value::as_str) != Some("collection")
                || record.get("record_id") != claim.get("subject_ref")
        })
    {
        return Err(DurableError::Conflict(
            "exact Collection version does not ground Claim subject",
        ));
    }
    let collection_record = collection_version
        .get("record")
        .ok_or(DurableError::Invalid(
            "selected Collection version record absent",
        ))?;
    let declared = collection_record
        .get("membership_claim_refs")
        .and_then(Value::as_array)
        .ok_or(DurableError::Invalid(
            "selected Collection membership refs absent",
        ))?;
    let expected = object
        .get("members")
        .and_then(Value::as_array)
        .ok_or(DurableError::Invalid("Claim Collection members absent"))?;
    let membership_versions = object
        .get("membership_versions")
        .and_then(Value::as_array)
        .ok_or(DurableError::Invalid(
            "Claim exact membership versions absent",
        ))?;
    let mut seen_members = BTreeSet::new();
    let mut basis_memberships = Vec::new();
    let mut input_digests = BTreeMap::<String, String>::new();
    bind_grounding_digests(
        collection_version
            .get("provenance")
            .ok_or(DurableError::Invalid("Collection provenance absent"))?,
        files,
        &mut input_digests,
    )?;
    for exact in membership_versions {
        let id = exact
            .get("id")
            .and_then(Value::as_str)
            .ok_or(DurableError::Invalid("membership Claim exact ID absent"))?;
        let indexed = inventory.claim_by_id(id).ok_or(DurableError::Conflict(
            "exact membership Claim endpoint absent",
        ))?;
        let schema_path = if indexed.source_path.ends_with("/historical-claims.jsonl") {
            HISTORICAL_CLAIM_SCHEMA
        } else {
            CLAIM_SCHEMA
        };
        let entry = render_catalog_claim(
            indexed.claim,
            indexed.source_path,
            indexed.source_line,
            Some(schema_path),
            limits.catalog.max_output_row_bytes,
        )
        .map_err(compiler_refusal)?;
        let resolved = resolve_selected_claim_version_v1(
            &entry,
            exact,
            files,
            validator,
            event_revision,
            materializer,
            limits,
        )
        .map_err(compiler_refusal)?;
        let member = resolved.record.as_ref().ok_or(DurableError::Conflict(
            "exact membership Claim version is not retained",
        ))?;
        if resolved.status != "available" {
            return Err(DurableError::Conflict(
                "exact membership Claim version unavailable",
            ));
        }
        let legacy = member.get("schema_version").and_then(Value::as_str)
            == Some("tos_claim_packet_v1")
            && member.get("claim_type").and_then(Value::as_str) == Some("bibliographic");
        let native = member.get("schema_version").and_then(Value::as_str)
            == Some("tos_source_relation_claim_v1")
            && member.get("claim_type").and_then(Value::as_str) == Some("relation");
        let work = member
            .get("object")
            .and_then(Value::as_str)
            .ok_or(DurableError::Invalid("membership Claim Work ref absent"))?;
        let polarity = member
            .get("polarity")
            .and_then(Value::as_str)
            .or(if legacy { Some("positive") } else { None });
        if !(legacy || native)
            || !matches!(
                member.get("assertion_layer").and_then(Value::as_str),
                Some("bibliographic_assertion" | "scholarly_report")
            )
            || !declared.iter().any(|candidate| candidate == &json!(id))
            || member.get("subject_ref") != claim.get("subject_ref")
            || member.get("predicate").and_then(Value::as_str) != Some("contains_work")
            || polarity != Some("positive")
            || !expected
                .iter()
                .any(|candidate| candidate.as_str() == Some(work))
            || !seen_members.insert(work.to_owned())
        {
            return Err(DurableError::Conflict(
                "exact positive Collection membership differs",
            ));
        }
        bind_grounding_digests(
            resolved
                .provenance
                .as_ref()
                .ok_or(DurableError::Invalid("membership Claim provenance absent"))?,
            files,
            &mut input_digests,
        )?;
        basis_memberships.push(json!({
            "ref": exact,
            "provenance": resolved.provenance,
            "version_status": resolved.version_status,
        }));
    }
    let expected_work_refs = expected
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(DurableError::Invalid("Collection Work reference type"))
        })
        .collect::<DurableResult<BTreeSet<_>>>()?;
    if seen_members
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_work_refs
    {
        return Err(DurableError::Conflict(
            "exact Collection membership set is incomplete",
        ));
    }
    Ok(json!({
        "collection": {"ref": collection_ref, "provenance": collection_version["provenance"], "version_status": collection_version["version_status"]},
        "memberships": basis_memberships,
        "input_digests": input_digests,
        "establishes_membership": false,
        "grants_admission": false
    }))
}

fn bind_grounding_digests(
    provenance: &Value,
    files: &SelectedBibliographicFilesV1,
    digests: &mut BTreeMap<String, String>,
) -> DurableResult<()> {
    let mut bind = |path: &Value, digest: &Value| -> DurableResult<()> {
        if path.is_null() || digest.is_null() {
            return Ok(());
        }
        let path = path
            .as_str()
            .ok_or(DurableError::Invalid("grounding input path type"))?;
        let digest = digest
            .as_str()
            .ok_or(DurableError::Invalid("grounding input digest type"))?
            .strip_prefix("sha256:")
            .ok_or(DurableError::Invalid("grounding input digest prefix"))?;
        Digest256::from_hex(digest)
            .map_err(|_| DurableError::Invalid("grounding input digest format"))?;
        files
            .optional(path)
            .map_err(|_| DurableError::Invalid("grounding selected input observation absent"))?
            .ok_or(DurableError::Invalid(
                "grounding selected input bytes absent",
            ))?;
        if let Some(previous) = digests.insert(path.to_owned(), digest.to_owned())
            && previous != digest
        {
            return Err(DurableError::Conflict("grounding input digest changed"));
        }
        Ok(())
    };
    for section in ["catalog", "history"] {
        let value = provenance.get(section).unwrap_or(&Value::Null);
        bind(&value["source_ref"], &value["sha256"])?;
    }
    let source = provenance.get("source").unwrap_or(&Value::Null);
    let source_path = source
        .get("archive_blob_ref")
        .filter(|value| !value.is_null())
        .unwrap_or(&source["source_ref"]);
    let source_digest = source
        .get("record_sha256")
        .filter(|value| !value.is_null())
        .unwrap_or(&source["stream_sha256"]);
    bind(source_path, source_digest)?;
    bind(
        &source["archive_manifest_ref"],
        &source["archive_manifest_sha256"],
    )?;
    Ok(())
}

fn selected_files_and_dependencies<'a>(
    context: &DurableNavigationEventContextV1,
    files: &'a SelectedBibliographicFilesV1,
    owner_source_path: &str,
    provenance_event_path: &str,
    provenance_event_schema_path: &str,
    provenance_receipt_path: &str,
    forms_path: &str,
    history_path: &str,
    _identity_owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<(
    Vec<NavigationClaimEventSourceFileV1<'a>>,
    Vec<NavigationDependencyV1>,
)> {
    let mut paths = BTreeSet::new();
    // Bind only this Claim's complete selected closure. A batch may hold
    // unrelated streams and metadata; they remain fenced by the range owner
    // without being hashed and persisted again in every Claim contribution.
    for path in files.observed_paths() {
        paths.insert(path);
        let source = context
            .selected_sources()
            .iter()
            .find(|source| source.source_path == path)
            .ok_or(DurableError::Conflict(
                "Claim dependency left selected event context",
            ))?;
        let selected = files
            .optional(&source.source_path)
            .map_err(|_| DurableError::Invalid("selected Claim file observation is incomplete"))?;
        match (&source.exact_source_bytes, selected) {
            (Some(bytes), Some(selected)) if bytes.as_slice() == selected => {}
            (None, None) => {}
            _ => {
                return Err(DurableError::Conflict(
                    "Claim file differs from selected event context",
                ));
            }
        }
    }
    for member in context.members() {
        paths.insert(member.source_path.as_str());
        let selected = files
            .optional(&member.source_path)
            .map_err(|_| DurableError::Invalid("Claim event member observation is incomplete"))?;
        match (&member.exact_source_bytes, selected) {
            (Some(bytes), Some(selected)) if bytes.as_slice() == selected => {}
            (None, None) => {}
            _ => {
                return Err(DurableError::Conflict(
                    "Claim event member differs from selected context",
                ));
            }
        }
    }
    if paths.len() > MAX_PATHS {
        return Err(DurableError::Refused(
            "selected Claim dependency path budget",
        ));
    }
    let source_files = files
        .paths()
        .map(|path| {
            Ok(NavigationClaimEventSourceFileV1 {
                path,
                bytes: files
                    .required(path)
                    .map_err(|_| DurableError::Invalid("selected Claim source file absent"))?,
            })
        })
        .collect::<DurableResult<Vec<_>>>()?;
    let mut dependencies = Vec::with_capacity(paths.len());
    for path in paths {
        let observed = files
            .optional(path)
            .map_err(|_| DurableError::Invalid("selected Claim dependency observation absent"))?;
        let state = match observed {
            Some(bytes) => NavigationDependencyStateV1::Present {
                sha256: Digest256::of_bytes(bytes),
                size_bytes: bytes.len() as u64,
            },
            None => NavigationDependencyStateV1::Absent {
                reason: "path is absent in the exact selected CMD source cut".into(),
            },
        };
        let role = dependency_role(
            path,
            owner_source_path,
            provenance_event_path,
            provenance_event_schema_path,
            provenance_receipt_path,
            forms_path,
            history_path,
        );
        dependencies.push(NavigationDependencyV1 {
            path: RelativePath::parse(path)
                .map_err(|_| DurableError::Invalid("Claim dependency path"))?,
            role,
            state,
        });
    }
    dependencies.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((source_files, dependencies))
}

fn dependency_role(
    path: &str,
    owner_source_path: &str,
    provenance_event_path: &str,
    provenance_event_schema_path: &str,
    provenance_receipt_path: &str,
    forms_path: &str,
    history_path: &str,
) -> NavigationDependencyRoleV1 {
    if path == owner_source_path || is_current_claim_stream(path) {
        return NavigationDependencyRoleV1::Record;
    }
    if path == CLAIM_SCHEMA {
        return NavigationDependencyRoleV1::RecordSchema;
    }
    if path == provenance_event_path {
        return NavigationDependencyRoleV1::Event;
    }
    if path == provenance_receipt_path {
        return NavigationDependencyRoleV1::EventReceipt;
    }
    if path == provenance_event_schema_path {
        return NavigationDependencyRoleV1::EventSchema;
    }
    if path == ENTITY_REGISTRY {
        return NavigationDependencyRoleV1::EntityRegistry;
    }
    if path == ENTITY_SCHEMA {
        return NavigationDependencyRoleV1::EntityRegistrySchema;
    }
    if path == RELATION_REGISTRY || path == RELATION_SCHEMA {
        return NavigationDependencyRoleV1::Additional;
    }
    if path == FORMS_SCHEMA {
        return NavigationDependencyRoleV1::FormsSchema;
    }
    if path == forms_path || path.ends_with(".human-forms.json") {
        return NavigationDependencyRoleV1::HumanForms;
    }
    if path == history_path
        || path.ends_with("/claim-revision-history.json")
        || path.ends_with("/source-revision-history.json")
    {
        return NavigationDependencyRoleV1::RecordHistory;
    }
    if path.contains("/.record-revisions/") {
        return NavigationDependencyRoleV1::VersionSource;
    }
    NavigationDependencyRoleV1::Additional
}

fn is_current_claim_stream(path: &str) -> bool {
    path.starts_with("ToS/source-witnesses/relations/")
        && path.ends_with("/source-claims.jsonl")
        && !path.contains("/owner-local/")
        && !path.contains("/private/")
}

fn claim_form_path(source_path: &str, claim_id: &str) -> DurableResult<String> {
    let stem = source_path
        .strip_suffix(".jsonl")
        .ok_or(DurableError::Invalid("Claim stream extension"))?;
    Ok(format!(
        "{stem}.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    ))
}

fn sibling(path: &str, name: &str) -> DurableResult<String> {
    let home = path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Invalid("Claim owner home absent"))?;
    Ok(format!("{home}/{name}"))
}

fn text<'a>(value: &'a Value, key: &str) -> DurableResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or(DurableError::Invalid(
            "selected Claim required text field absent",
        ))
}

fn parse_value(raw: &[u8], max_bytes: usize) -> DurableResult<Value> {
    if raw.is_empty() || raw.len() > max_bytes {
        return Err(DurableError::Refused("selected Claim JSON byte budget"));
    }
    let value = crate::source_command::parse(raw)
        .map_err(|_| DurableError::Invalid("selected Claim strict JSON"))?;
    let canonical = crate::source_command::canonical(&value)
        .map_err(|_| DurableError::Invalid("selected Claim canonical JSON"))?;
    serde_json::from_slice(&canonical)
        .map_err(|_| DurableError::Invalid("selected Claim JSON conversion"))
}
