//! Whole initial Metadata prepared publication. Creation has already committed.
//! The owned SQLite transaction rolls back on any failure; immutable COW parts
//! remain unselected. This never writes authored source or grants admission.
use crate::{
    source_agent_publication_profile::NativeAgentExecution,
    source_claim_publication::{
        ClaimPublicationLimits, ClaimPublicationProgress, catalog_after, source_vector,
    },
    source_claim_publication_bytes as bytes,
    source_claim_publication_context::{Context, keys},
    source_claim_publication_dependencies::{Dependencies, Limits as DependencyLimits},
    source_claim_publication_graph::Graph,
    source_claim_publication_normalize::{
        self as normalization, CandidateNode, CandidateRelation, ClaimCandidateInput,
        ClaimCandidateLimits, ClaimCandidateRegistries,
    },
    source_claim_publication_roots::{Change, MutationLimits, Roots, Snapshot},
    source_command::CommandContext,
    source_creation::SerializedCreation,
    source_creation_store::{CommittedMetadataCreationObservation, CreationFilesystem},
    source_metadata_publication_assembly as assembly,
};
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_compiler::{
    Error, QueryVocabulary, Result,
    knowledge_normalization::SourceRow,
    knowledge_stage::SeekRow,
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::CatalogInputs,
    prepared_semantic_index::SemanticMaintenanceLimits,
    prepared_source_binding::{
        PreparedSourceInputs, apply_source_bound_prepared_delta_transaction,
        read_prepared_source_inputs_transaction,
    },
    source_bibliographic::BibliographicLimits,
};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json,
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CutWorkerSchemaExecutor;
const META: usize = 1_048_576;
const PROFILE: &str = "metadata-addition-publication-profile";
fn owner<T>(r: crate::source_command::SourceCommandResult<T>) -> Result<T> {
    r.map_err(|e| Error::Source(format!("Metadata owner: {e:?}")))
}
fn view(v: &JsonValue) -> Result<Value> {
    bytes::parse(&encoded(v)?, META)
}
fn encoded(v: &JsonValue) -> Result<Vec<u8>> {
    emit_value_preserved_json(
        v,
        JsonLimits::new(META, 128, 1_000_000, 4300)
            .map_err(|_| Error::Budget("Metadata detached limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))
}
fn typed(v: &Value) -> Result<JsonValue> {
    Ok(parse_json(
        &bytes::canonical(v, META)?,
        JsonMode::PublishedStrict,
        JsonLimits::new(META, 128, 1_000_000, 4300)
            .map_err(|_| Error::Budget("Metadata typed limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?
    .root()
    .clone())
}
fn seek(g: &str, id: &str, raw: Vec<u8>) -> SeekRow {
    SeekRow {
        id: id.to_owned(),
        source_graph: g.to_owned(),
        source_order: None,
        payload_sha256: bytes::digest(&raw),
        payload: raw,
    }
}
fn role(g: &str) -> Result<&'static str> {
    match g {
        "source-navigation" => Ok("source-navigation"),
        "source-claims" => Ok("bibliographic-claims"),
        _ => Err(Error::Invalid("Metadata raw graph")),
    }
}
/// Independently held executing image, shared native normalization/declaration
/// identities, and this exact whole publication implementation descriptor.
pub struct NativeMetadataExecution {
    common: NativeAgentExecution,
    profile: String,
}
impl NativeMetadataExecution {
    pub fn observe(deadline: Instant, cancelled: &AtomicBool) -> Result<Self> {
        let common = NativeAgentExecution::observe(deadline, cancelled)?;
        let profile = bytes::row_digest(
            &json!({"schema":"tos_native_metadata_addition_publication_v1","executing_artifact_sha256":common.processor()}),
            META,
        )?;
        Ok(Self { common, profile })
    }
    pub fn profile(&self) -> &str {
        &self.profile
    }
    pub fn processor(&self) -> &str {
        self.common.processor()
    }
    /// Reuse this same held image for the explicit native predecessor bootstrap.
    pub fn common_execution(&self) -> &NativeAgentExecution {
        &self.common
    }
    fn verify(&self, source: &PreparedSourceInputs, catalog: &CatalogInputs) -> Result<()> {
        self.common.verify_source(source, catalog)?;
        let v = view(&source.value()?)?;
        if v["dependencies"]
            .get(PROFILE)
            .is_some_and(|value| value.as_str() != Some(self.profile()))
        {
            return Err(Error::Invalid(
                "Metadata execution profile requires explicit reviewed transition",
            ));
        }
        Ok(())
    }
}
/// Exact compatible implementation-only pair. It does not attest the review.
pub struct ReviewedMetadataProfileTransition {
    pub before_sha256: String,
    pub after_sha256: String,
    pub review_ref: String,
}

/// Pair one named Metadata implementation transition with dependency/context
/// current selections under the existing caller transaction. No domain rows,
/// root bytes, publication token, normalization or declarations change.
#[allow(clippy::too_many_arguments)]
pub fn transition_reviewed_metadata_profile_transaction(
    tx: &Transaction<'_>,
    progress: &ClaimPublicationProgress<'_>,
    source: &PreparedSourceInputs,
    expected: &JsonValue,
    catalog: &CatalogInputs,
    execution: &NativeMetadataExecution,
    review: &ReviewedMetadataProfileTransition,
    publication: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    operation: ClaimPublicationLimits,
) -> Result<Value> {
    operation.validate()?;
    let dependencies = DependencyLimits {
        max_claims: operation.max_claims,
        max_read_bytes: operation.max_bytes,
        max_writes: publication.max_mutations,
        ..DependencyLimits::default()
    };
    execution.common.verify_source(source, catalog)?;
    progress.verify(tx)?;
    publication.validate()?;
    let mut value = view(&source.value()?)?;
    bytes::sha(&review.before_sha256)?;
    bytes::sha(&review.after_sha256)?;
    if value["dependencies"][PROFILE] != review.before_sha256
        || execution.profile() != review.after_sha256
        || review.before_sha256 == review.after_sha256
        || review.review_ref.trim().is_empty()
        || review.review_ref.len() > 4096
    {
        return Err(Error::Invalid("Metadata exact reviewed profile pair"));
    }
    let start = tx.total_changes();
    let binding = view(expected)?;
    let mut dependency = Dependencies::new(tx, dependencies)?;
    let state = dependency.state(
        &binding,
        source.digest(),
        execution.common.declaration(),
        execution.common.dependency_implementation(),
    )?;
    let mut context = Context::new(tx, dependencies);
    let mut context_state = context.state(&binding, source.digest())?;
    value["dependencies"][PROFILE] = json!(execution.profile());
    let after_source = source_vector(value, publication)?;
    let normalization = view(&catalog.header)?["normalization_binding"].clone();
    let after = catalog_after(catalog, &after_source, &normalization)?;
    let finalizers = dependency.stage(
        state,
        &[],
        after_source.digest(),
        after_source.source_revision(),
    )?;
    let used = tx.total_changes() - start;
    let mut limits = publication;
    limits.max_mutations = publication
        .max_mutations
        .checked_sub(used)
        .and_then(|n| n.checked_sub(finalizers as u64 + 1))
        .filter(|n| *n >= 2)
        .ok_or(Error::Budget("Metadata profile finalizers"))?;
    let paired = apply_source_bound_prepared_delta_transaction(
        tx,
        expected,
        source,
        &after_source,
        catalog,
        &after,
        std::iter::empty(),
        limits,
        catalog_limits,
        semantic_limits,
        execution.processor(),
    )?;
    let binding = view(&paired.publication.binding)?;
    dependency.finalize(
        &binding,
        after_source.digest(),
        execution.common.declaration(),
        execution.common.dependency_implementation(),
    )?;
    context.append(
        context_state.clone(),
        &binding,
        after_source.digest(),
        &[],
        &BTreeMap::new(),
    )?;
    context_state["binding"] = binding.clone();
    context_state["source_inputs_sha256"] = json!(after_source.digest());
    if context.state(&binding, after_source.digest())? != context_state {
        return Err(Error::Invalid("Metadata profile context readback"));
    }
    if tx.total_changes() - start > publication.max_mutations {
        return Err(Error::Budget("Metadata profile cumulative mutations"));
    }
    execution.verify(&after_source, &after)?;
    progress.verify(tx)?;
    Ok(
        json!({"binding":binding,"source_inputs_sha256":after_source.digest(),"source_revision":after_source.source_revision(),"sql_mutations":tx.total_changes()-start,"before_execution_profile_sha256":review.before_sha256,"after_execution_profile_sha256":review.after_sha256,"compatibility_review_ref":review.review_ref,"compatibility_verified_by_helper":false,"normalized_row_changes_supplied":0,"agent_context_membership_changed":false}),
    )
}

/// Actual whole caller: authenticate committed creation and current cut, stage
/// its bounded cohort/catalog, normalize complete incidence, publish paired
/// state and finalizers, reauthenticate currentness, then commit the same DB.
/// Source creation remains committed when this operation fails.
#[allow(clippy::too_many_arguments)]
pub fn publish_committed_initial_metadata(
    db: &Connection,
    owner_configuration: &Path,
    package: &SerializedCreation,
    original: &CorpusCutReader,
    current: &CorpusCutReader,
    current_context: &CommandContext,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    expected_receipt: Digest256,
    expected_request: Digest256,
    source: &PreparedSourceInputs,
    expected: &JsonValue,
    catalog: &CatalogInputs,
    metadata_profile_review: Option<&ReviewedMetadataProfileTransition>,
    worker: &mut CutWorkerSchemaExecutor,
    vocabulary: &QueryVocabulary,
    descriptor: &[u8],
    execution: &NativeMetadataExecution,
    operation: ClaimPublicationLimits,
    bibliographic: BibliographicLimits,
    publication: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    cancelled: Arc<AtomicBool>,
) -> Result<Value> {
    publish_committed_initial_metadata_with_precommit(
        db,
        owner_configuration,
        package,
        original,
        current,
        current_context,
        software,
        components,
        expected_receipt,
        expected_request,
        source,
        expected,
        catalog,
        metadata_profile_review,
        worker,
        vocabulary,
        descriptor,
        execution,
        operation,
        bibliographic,
        publication,
        catalog_limits,
        semantic_limits,
        cancelled,
        &mut || Ok(()),
    )
}

/// Protected transports revalidate their held database selection immediately
/// before the same owned transaction commits. The callback grants no source authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_committed_initial_metadata_with_precommit(
    db: &Connection,
    owner_configuration: &Path,
    package: &SerializedCreation,
    original: &CorpusCutReader,
    current: &CorpusCutReader,
    current_context: &CommandContext,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    expected_receipt: Digest256,
    expected_request: Digest256,
    source: &PreparedSourceInputs,
    expected: &JsonValue,
    catalog: &CatalogInputs,
    metadata_profile_review: Option<&ReviewedMetadataProfileTransition>,
    worker: &mut CutWorkerSchemaExecutor,
    vocabulary: &QueryVocabulary,
    descriptor: &[u8],
    execution: &NativeMetadataExecution,
    operation: ClaimPublicationLimits,
    bibliographic: BibliographicLimits,
    publication: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    cancelled: Arc<AtomicBool>,
    precommit: &mut dyn FnMut() -> Result<()>,
) -> Result<Value> {
    operation.validate()?;
    publication.validate()?;
    catalog.validate()?;
    if !db.is_autocommit() || descriptor.len() > META {
        return Err(Error::Invalid("Metadata idle connection and descriptor"));
    }
    let deadline = bibliographic.deadline;
    execution.common.verify_source(source, catalog)?;
    let environment = package
        .prepared()
        .files()
        .get("source-create-environment.json")
        .ok_or(Error::Invalid(
            "Metadata native creation environment absent",
        ))?;
    if bytes::parse(environment, META)?["runtime_artifact_sha256"] != execution.processor() {
        return Err(Error::Invalid(
            "Metadata creation must use the selected executing owner image",
        ));
    }
    let before_profile = view(&source.value()?)?["dependencies"]
        .get(PROFILE)
        .cloned();
    if before_profile
        .as_ref()
        .is_some_and(|v| v.as_str() != Some(execution.profile()))
    {
        let review = metadata_profile_review.ok_or(Error::Invalid(
            "Metadata named compatibility review required",
        ))?;
        if before_profile.as_ref().and_then(Value::as_str) != Some(review.before_sha256.as_str())
            || review.after_sha256 != execution.profile()
            || review.review_ref.trim().is_empty()
            || review.review_ref.len() > 4096
        {
            return Err(Error::Invalid("Metadata exact compatibility pair"));
        }
    } else if metadata_profile_review.is_some() {
        return Err(Error::Invalid("Metadata unsolicited profile transition"));
    }
    let (fs, configuration) = owner(CreationFilesystem::select_creation_owner(
        owner_configuration,
        deadline,
        &cancelled,
    ))?;
    if configuration != current_context.configuration_raw {
        return Err(Error::Invalid("Metadata selected owner differs"));
    }
    let observation = owner(CommittedMetadataCreationObservation::select(
        &fs,
        package,
        original,
        current,
        current_context,
        software,
        components,
        expected_receipt,
        expected_request,
        deadline,
        &cancelled,
    ))?;
    let entity = encoded(&catalog.entity_registry)?;
    let relation = encoded(&catalog.relation_registry)?;
    let header = view(&catalog.header)?;
    let normalization = header["normalization_binding"].clone();
    let registries = ClaimCandidateRegistries {
        entity_bytes: &entity,
        relation_bytes: &relation,
        descriptor_bytes: descriptor,
        vocabulary,
        expected_normalization_binding: &normalization,
    };
    let mutation = MutationLimits {
        input: operation.max_bytes,
        decoded: operation.max_bytes,
        result: operation.max_bytes,
        ..MutationLimits::default()
    };
    let mut snapshots = BTreeMap::new();
    for (role, root) in source.roots() {
        snapshots.insert(
            role.clone(),
            Snapshot::parse(
                root.root_bytes.clone(),
                root.namespace_path.clone().into(),
                &root.snapshot_sha256,
            )?,
        );
    }
    let mut roots = Roots::new(snapshots, mutation)?;
    let assembled =
        assembly::assemble(&observation, &mut roots, worker, bibliographic, &cancelled)?;
    let progress =
        ClaimPublicationProgress::install(db, cancelled.clone(), deadline, operation.max_vm_steps)?;
    let tx = db.unchecked_transaction()?;
    progress.verify(&tx)?;
    if tx.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))? != "wal"
        || publication.max_mutations <= 2
    {
        return Err(Error::Invalid(
            "Metadata WAL transaction finalizer allowance",
        ));
    }
    if read_prepared_source_inputs_transaction(&tx, expected, catalog, publication)?.raw()
        != source.raw()
    {
        return Err(Error::Invalid("Metadata captured predecessor advanced"));
    }
    let start = tx.total_changes();
    let (selected_source, selected_binding, selected_catalog) =
        if let Some(review) = metadata_profile_review {
            let result = transition_reviewed_metadata_profile_transaction(
                &tx,
                &progress,
                source,
                expected,
                catalog,
                execution,
                review,
                publication,
                catalog_limits,
                semantic_limits,
                operation,
            )?;
            let mut value = view(&source.value()?)?;
            value["dependencies"][PROFILE] = json!(execution.profile());
            let after_source = source_vector(value, publication)?;
            let after = catalog_after(catalog, &after_source, &normalization)?;
            (after_source, typed(&result["binding"])?, after)
        } else {
            (
                PreparedSourceInputs::parse(source.raw(), publication)?,
                expected.clone(),
                catalog_after(catalog, source, &normalization)?,
            )
        };
    let source = &selected_source;
    let expected = &selected_binding;
    let catalog = &selected_catalog;
    execution.verify(source, catalog)?;
    let binding = view(expected)?;
    let dependencies = DependencyLimits {
        max_claims: operation.max_claims,
        max_read_bytes: operation.max_bytes,
        max_writes: publication.max_mutations,
        ..DependencyLimits::default()
    };
    let mut dependency = Dependencies::new(&tx, dependencies)?;
    let state = dependency.state(
        &binding,
        source.digest(),
        execution.common.declaration(),
        execution.common.dependency_implementation(),
    )?;
    for (kind, reference) in [
        ("unresolved", Value::Null),
        ("unresolved", json!(assembled.id)),
        ("identity", json!(assembled.id)),
    ] {
        if !dependency.lookup(kind, &reference)?.is_empty() {
            return Err(Error::Invalid(
                "Metadata broader existing dependency closure",
            ));
        }
    }
    let mut context = Context::new(&tx, dependencies);
    let mut context_state = context.state(&binding, source.digest())?;
    let mut dossiers: BTreeSet<String> = context_state["dossier_refs"]
        .as_array()
        .ok_or(Error::Invalid("Metadata dossier state"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(Error::Invalid("Metadata dossier identity"))
        })
        .collect::<Result<_>>()?;
    for (collection, kind, rows) in [
        ("nodes", "node", &assembled.nodes),
        ("edges", "relation", &assembled.edges),
    ] {
        for ((g, id), raw) in rows {
            if roots.get(role(g)?, collection, id)?.is_some()
                || tx
                    .query_row(
                        "SELECT id FROM prepared_documents WHERE kind=?1 AND id=?2 LIMIT 1",
                        [kind, &format!("{g}:{id}")],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                    .is_some()
            {
                return Err(Error::Invalid("Metadata new carrier exists"));
            }
            if collection == "nodes" && g == "source-navigation" {
                if let Some(d) =
                    normalization::declared_dossier(&bytes::parse(raw, operation.max_bytes)?, g)
                {
                    dossiers.insert(d);
                }
            }
        }
    }
    let candidate_limits = ClaimCandidateLimits {
        max_nodes: operation.max_nodes,
        max_retained_nodes: operation.max_nodes,
        max_relations: operation.max_relations,
        max_traces: operation.max_claims,
        max_contexts: operation.max_contexts,
        max_row_bytes: bibliographic
            .catalog
            .max_row_bytes
            .min(operation.max_row_bytes)
            .min(publication.max_row_bytes),
        max_input_bytes: operation.max_bytes,
        max_output_bytes: operation.max_bytes,
    };
    let nodes = assembled
        .nodes
        .iter()
        .map(|((g, id), raw)| {
            let value = bytes::parse(raw, candidate_limits.max_row_bytes)?;
            Ok(CandidateNode {
                raw: seek(g, id, raw.clone()),
                dossier_ref: normalization::declared_dossier(&value, g)
                    .filter(|d| dossiers.contains(d)),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut input = ClaimCandidateInput {
        nodes,
        relations: assembled
            .edges
            .iter()
            .map(|((g, id), raw)| CandidateRelation {
                raw: seek(g, id, raw.clone()),
                identity_id: None,
            })
            .collect(),
        retained_nodes: Vec::new(),
        traces: Vec::new(),
        dossier_refs: dossiers.iter().cloned().collect(),
        context_node_order: Vec::new(),
        normalization_binding: normalization.clone(),
    };
    let base = normalization::normalize_claim_candidate(&input, registries, candidate_limits)?;
    // Join only this complete new cohort. The shared compiler pair renderer
    // owns the persistent-ID rule; no whole graph or fabricated stage exists.
    let mut joins = Vec::new();
    for left in base
        .nodes
        .iter()
        .filter(|n| n["source_graph"] == "source-claims")
    {
        for right in base
            .nodes
            .iter()
            .filter(|n| n["source_graph"] == "source-navigation")
        {
            if let Some(raw) = tos_compiler::render_supplied_shared_identity_pair(
                &SourceRow::parse(
                    &bytes::canonical(left, candidate_limits.max_row_bytes)?,
                    candidate_limits.max_row_bytes,
                )?,
                &SourceRow::parse(
                    &bytes::canonical(right, candidate_limits.max_row_bytes)?,
                    candidate_limits.max_row_bytes,
                )?,
                vocabulary,
                descriptor,
                candidate_limits.max_row_bytes,
            )? {
                joins.push(raw);
            }
        }
    }
    let join_graph = vocabulary
        .sources
        .iter()
        .find(|s| s.adapter_profile == "declared-identity-and-source-ref-joins-v1")
        .map(|s| s.source_graph_id.as_str())
        .ok_or(Error::Invalid("Metadata shared identity join source"))?;
    for raw in joins {
        let id = bytes::text(&raw, "edge_id")?.to_owned();
        input.relations.push(CandidateRelation {
            raw: seek(
                join_graph,
                &id,
                bytes::canonical(&raw, candidate_limits.max_row_bytes)?,
            ),
            identity_id: None,
        });
    }
    let normalized =
        normalization::normalize_claim_candidate(&input, registries, candidate_limits)?;
    let selected: BTreeSet<_> = assembled
        .nodes
        .keys()
        .map(|(g, id)| format!("{g}:{id}"))
        .collect();
    if normalized
        .nodes
        .iter()
        .map(|n| bytes::text(n, "id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?
        != selected
        || normalized
            .nodes
            .iter()
            .any(|n| keys(n).map_or(true, |k| !k.is_empty()))
        || normalized.relations.iter().any(|r| {
            !selected.contains(r["from_id"].as_str().unwrap_or(""))
                || !selected.contains(r["to_id"].as_str().unwrap_or(""))
        })
    {
        return Err(Error::Invalid(
            "Metadata complete independent normalized incidence",
        ));
    }
    let mut graph = Graph::new(&tx, dependencies);
    let changes = graph.changes(
        &BTreeMap::new(),
        &BTreeMap::new(),
        &normalized.nodes,
        &normalized.relations,
    )?;
    let mut value = view(&source.value()?)?;
    value["dependencies"][PROFILE] = json!(execution.profile());
    for g in ["source-navigation", "source-claims"] {
        let mut changes = Vec::new();
        for (collection, rows) in [("nodes", &assembled.nodes), ("edges", &assembled.edges)] {
            for ((graph, id), raw) in rows {
                if graph == g {
                    changes.push(Change {
                        collection: collection.into(),
                        key: id.clone(),
                        before_sha256: None,
                        after: bytes::parse(raw, candidate_limits.max_row_bytes)?,
                    });
                }
            }
        }
        let root = roots.stage(role(g)?, changes, None, operation.cow_target_bytes)?;
        value["roots"][role(g)?] = json!({"namespace_path":root.path.to_str().ok_or(Error::Invalid("Metadata root path"))?,"root_json":std::str::from_utf8(&root.raw).map_err(|_|Error::Invalid("Metadata root UTF8"))?,"snapshot_sha256":bytes::digest(&root.raw)});
    }
    let root = roots.stage(
        "source-catalog",
        assembled.catalog_changes,
        Some(assembled.header),
        operation.cow_target_bytes,
    )?;
    value["roots"]["source-catalog"] = json!({"namespace_path":root.path.to_str().ok_or(Error::Invalid("Metadata catalog path"))?,"root_json":std::str::from_utf8(&root.raw).map_err(|_|Error::Invalid("Metadata catalog UTF8"))?,"snapshot_sha256":bytes::digest(&root.raw)});
    let after_source = source_vector(value, publication)?;
    let after = catalog_after(catalog, &after_source, &normalization)?;
    let finalizers = dependency.stage(
        state,
        &[],
        after_source.digest(),
        after_source.source_revision(),
    )?;
    let used = tx.total_changes() - start;
    let mut paired_limits = publication;
    paired_limits.max_mutations = publication
        .max_mutations
        .checked_sub(used)
        .and_then(|n| n.checked_sub(finalizers as u64 + 1))
        .filter(|n| *n >= 2)
        .ok_or(Error::Budget("Metadata combined finalizers"))?;
    let paired = apply_source_bound_prepared_delta_transaction(
        &tx,
        expected,
        source,
        &after_source,
        catalog,
        &after,
        changes.into_iter().map(Ok),
        paired_limits,
        catalog_limits,
        semantic_limits,
        execution.processor(),
    )?;
    let binding = view(&paired.publication.binding)?;
    dependency.finalize(
        &binding,
        after_source.digest(),
        execution.common.declaration(),
        execution.common.dependency_implementation(),
    )?;
    context_state["dossier_refs"] = json!(dossiers);
    context.append(
        context_state.clone(),
        &binding,
        after_source.digest(),
        &[],
        &BTreeMap::new(),
    )?;
    context_state["binding"] = binding.clone();
    context_state["source_inputs_sha256"] = json!(after_source.digest());
    if context.state(&binding, after_source.digest())? != context_state {
        return Err(Error::Invalid("Metadata context finalizer readback"));
    }
    let total = tx.total_changes() - start;
    if total > publication.max_mutations {
        return Err(Error::Budget("Metadata cumulative mutations"));
    }
    let selected_changes = tx.total_changes();
    let schema: u64 = tx.query_row("PRAGMA main.schema_version", [], |r| r.get(0))?;
    let mut receipt = json!({"schema":"tos_initial_metadata_publication_v1","record_id":assembled.id,"creation_request_digest":assembled.request_digest,"binding":binding,"source_inputs_sha256":after_source.digest(),"sql_mutations":total,"changed_nodes":normalized.nodes.len(),"changed_relations":normalized.relations.len(),"source_header":paired.publication.source_header.as_ref().map(view).transpose()?,"semantic_report_sha256":paired.publication.semantic_report_sha256,"catalog_digest":paired.publication.catalog_digest,"source_command_committed":true,"prepared_committed":false,"initial_metadata_closure_verified":true,"global_source_currentness_verified":false,"consumer_switched":false,"is_semantic_acceptance":false});
    bytes::canonical(&receipt, publication.max_metadata_bytes.min(META))?;
    execution.verify(&after_source, &after)?;
    owner(observation.verify_current(deadline, &cancelled))?;
    progress.verify(&tx)?;
    if selected_changes != tx.total_changes()
        || schema != tx.query_row("PRAGMA main.schema_version", [], |r| r.get::<_, u64>(0))?
        || read_prepared_source_inputs_transaction(
            &tx,
            &typed(&binding)?,
            &paired.final_catalog,
            publication,
        )?
        .raw()
            != after_source.raw()
    {
        return Err(Error::Invalid("Metadata selection changed before commit"));
    }
    precommit()?;
    progress.verify(&tx)?;
    if selected_changes != tx.total_changes()
        || schema != tx.query_row("PRAGMA main.schema_version", [], |r| r.get::<_, u64>(0))?
    {
        return Err(Error::Invalid(
            "Metadata database guard changed the selected transaction",
        ));
    }
    tx.commit()?;
    receipt["prepared_committed"] = json!(true);
    Ok(receipt)
}
