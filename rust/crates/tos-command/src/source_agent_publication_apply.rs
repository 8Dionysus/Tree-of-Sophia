//! Whole prepared half of the source-locked descriptive Agent transaction.
//! Source creation has already committed; errors roll back the entire caller DB
//! transaction and leave immutable unselected COW parts for owner recovery.
use crate::{
    source_agent_publication_assembly as assembly, source_agent_publication_closure as closure,
    source_claim_publication::{ClaimPublicationProgress, catalog_after, source_vector},
    source_claim_publication_bytes as bytes,
    source_claim_publication_context::Context,
    source_claim_publication_dependencies::{Dependencies, Limits},
    source_claim_publication_normalize::{ClaimCandidateLimits, ClaimCandidateRegistries},
    source_claim_publication_roots::{MutationLimits, Roots, Snapshot},
    source_creation_store::CommittedRecordObservation,
};
use rusqlite::{Connection, Transaction};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::atomic::AtomicBool, time::Instant};
use tos_compiler::source_bibliographic::BibliographicLimits;
use tos_compiler::{
    Error, Result,
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::CatalogInputs,
    prepared_semantic_index::SemanticMaintenanceLimits,
    prepared_source_binding::{
        PreparedSourceInputs, apply_source_bound_prepared_delta_transaction,
        read_prepared_source_inputs_transaction,
    },
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json};
use tos_validation::source_cut::CutWorkerSchemaExecutor;
const META: usize = 1_048_576;
fn view(value: &JsonValue) -> Result<Value> {
    let raw = emit_value_preserved_json(
        value,
        JsonLimits::new(META, 128, 1_000_000, 4300)
            .map_err(|_| Error::Budget("Agent publication JSON limits"))?,
    )
    .map_err(|_| Error::Budget("Agent publication JSON bytes"))?;
    bytes::parse(&raw, META)
}
fn typed(value: &Value) -> Result<JsonValue> {
    Ok(parse_json(
        &bytes::canonical(value, META)?,
        JsonMode::PublishedStrict,
        JsonLimits::new(META, 128, 1_000_000, 4300)
            .map_err(|_| Error::Budget("Agent typed JSON"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?
    .root()
    .clone())
}
fn verify(
    observation: &CommittedRecordObservation<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    observation
        .verify_current(deadline, cancelled)
        .map_err(crate::source_agent_publication_assembly::source_failure)
}

pub(super) struct Applied {
    connection: usize,
    total_changes: u64,
    schema: u64,
    source: PreparedSourceInputs,
    catalog: CatalogInputs,
    receipt: Value,
    limits: PublicationLimits,
}
impl Applied {
    pub fn commit(
        self,
        tx: Transaction<'_>,
        progress: &ClaimPublicationProgress<'_>,
        observation: &CommittedRecordObservation<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
        precommit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Value> {
        progress.verify(&tx)?;
        let schema: u64 = tx.query_row("PRAGMA main.schema_version", [], |r| r.get(0))?;
        if self.connection != (&*tx as *const Connection) as usize
            || tx.is_autocommit()
            || self.total_changes != tx.total_changes()
            || self.schema != schema
        {
            return Err(Error::Invalid(
                "Agent applied transaction changed before commit",
            ));
        }
        let binding = typed(&self.receipt["binding"])?;
        if read_prepared_source_inputs_transaction(&tx, &binding, &self.catalog, self.limits)?.raw()
            != self.source.raw()
        {
            return Err(Error::Invalid(
                "Agent paired selection changed before commit",
            ));
        }
        verify(observation, deadline, cancelled)?;
        progress.verify(&tx)?;
        precommit()?;
        tx.commit()?;
        let mut result = self.receipt;
        result["prepared_committed"] = json!(true);
        Ok(result)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    tx: &Transaction<'_>,
    progress: &ClaimPublicationProgress<'_>,
    observation: &CommittedRecordObservation<'_>,
    before_source: &PreparedSourceInputs,
    expected: &JsonValue,
    before: &CatalogInputs,
    original_worker: &mut CutWorkerSchemaExecutor,
    current_worker: &mut CutWorkerSchemaExecutor,
    bibliographic_limits: BibliographicLimits,
    mutation_limits: MutationLimits,
    candidate_limits: ClaimCandidateLimits,
    dependency_limits: Limits,
    registries: &ClaimCandidateRegistries<'_>,
    limits: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    declaration_profile: &str,
    dependency_implementation: &str,
    processor: &str,
    cow_target_bytes: usize,
    cancelled: &AtomicBool,
) -> Result<Applied> {
    limits.validate()?;
    progress.verify(tx)?;
    // The sole caller just authenticated and acquired this source guard.
    // Reauthenticate once immediately before commit, not at every phase.
    crate::source_agent_publication::require_vector(before_source, META)?;
    if before.source_order_profile
        != tos_compiler::prepared_catalog_semantics::SourceOrderProfile::SourceGraphId
    {
        return Err(Error::Invalid(
            "Agent publication requires canonical source order",
        ));
    }
    let journal: String = tx.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    if journal != "wal" {
        return Err(Error::Invalid(
            "Agent publication requires explicit WAL profile",
        ));
    }
    if limits.max_mutations <= 4 || tx.is_autocommit() {
        return Err(Error::Budget("Agent publication finalizer reservation"));
    }
    if read_prepared_source_inputs_transaction(tx, expected, before, limits)?.raw()
        != before_source.raw()
    {
        return Err(Error::Invalid("Agent captured predecessor advanced"));
    }
    let start = tx.total_changes();
    let binding = view(expected)?;
    let mut dependencies = Dependencies::new(tx, dependency_limits)?;
    let state = dependencies.state(
        &binding,
        before_source.digest(),
        declaration_profile,
        dependency_implementation,
    )?;
    // Resolve only indexed affected declarations under this exact predecessor.
    // Unknown dependencies prevent a bounded correction; never scan the graph.
    if !dependencies.lookup("unresolved", &Value::Null)?.is_empty() {
        return Err(Error::Invalid("Agent unresolved source dependencies"));
    }
    let ids = dependencies.lookup("identity", &json!(observation.record_id()))?;
    let mut reverse = Vec::with_capacity(ids.len());
    for id in ids {
        reverse.push(
            dependencies
                .claim(&id)?
                .ok_or(Error::Invalid("Agent reverse declaration disappeared"))?,
        );
    }
    let mut snapshots = BTreeMap::new();
    for (role, root) in before_source.roots() {
        snapshots.insert(
            role.clone(),
            Snapshot::parse(
                root.root_bytes.clone(),
                root.namespace_path.clone().into(),
                &root.snapshot_sha256,
            )?,
        );
    }
    let mut roots = Roots::new(snapshots, mutation_limits)?;
    let assembled = assembly::assemble(
        observation,
        &mut roots,
        &reverse,
        original_worker,
        current_worker,
        bibliographic_limits,
        cancelled,
    )?;
    let source_publication = view(observation.source_publication())?;
    if assembled.catalog_header["source_publication"] != source_publication
        || assembled.reverse_dependent_claims != reverse.len()
    {
        return Err(Error::Invalid(
            "Agent catalog/dependency successor mismatch",
        ));
    }
    let mut context = Context::new(tx, dependency_limits);
    let context_state = context.state(&binding, before_source.digest())?;
    let result = closure::normalize(
        tx,
        &mut roots,
        &assembled.old,
        &assembled.new,
        &context_state,
        candidate_limits,
        dependency_limits,
        registries,
    )?;
    progress.verify(tx)?;
    let mut source = view(&before_source.value()?)?;
    source["source_publication"] = json!(bytes::text(&source_publication, "token")?);
    for (role, changes) in result.raw_changes {
        let root = roots.stage(&role, changes, None, cow_target_bytes)?;
        source["roots"][&role] = json!({"namespace_path":root.path.to_str().ok_or(Error::Invalid("Agent root path"))?,
            "root_json":std::str::from_utf8(&root.raw).map_err(|_|Error::Invalid("Agent root UTF8"))?,
            "snapshot_sha256":bytes::digest(&root.raw)});
    }
    let root = roots.stage(
        "source-catalog",
        assembled.catalog_changes,
        Some(assembled.catalog_header),
        cow_target_bytes,
    )?;
    source["roots"]["source-catalog"] = json!({"namespace_path":root.path.to_str().ok_or(Error::Invalid("Agent catalog path"))?,
        "root_json":std::str::from_utf8(&root.raw).map_err(|_|Error::Invalid("Agent catalog UTF8"))?,
        "snapshot_sha256":bytes::digest(&root.raw)});
    let after_source = source_vector(source, limits)?;
    let normalization = view(&before.header)?["normalization_binding"].clone();
    let after = catalog_after(before, &after_source, &normalization)?;
    let finalizer = dependencies.stage(
        state,
        &[],
        after_source.digest(),
        after_source.source_revision(),
    )?;
    let used = tx
        .total_changes()
        .checked_sub(start)
        .ok_or(Error::Invalid("Agent cumulative writes"))?;
    let reserve = (finalizer as u64)
        .checked_add(1)
        .ok_or(Error::Budget("Agent finalizers"))?;
    let mut paired_limits = limits;
    paired_limits.max_mutations = limits
        .max_mutations
        .checked_sub(used)
        .and_then(|n| n.checked_sub(reserve))
        .filter(|n| *n >= 2)
        .ok_or(Error::Budget("Agent publication remaining mutations"))?;
    let paired = apply_source_bound_prepared_delta_transaction(
        tx,
        expected,
        before_source,
        &after_source,
        before,
        &after,
        result.changes.into_iter().map(Ok),
        paired_limits,
        catalog_limits,
        semantic_limits,
        processor,
    )?;
    let binding = view(&paired.publication.binding)?;
    dependencies.finalize(
        &binding,
        after_source.digest(),
        declaration_profile,
        dependency_implementation,
    )?;
    context.rebind(context_state, &binding, after_source.digest())?;
    let total = tx
        .total_changes()
        .checked_sub(start)
        .filter(|n| *n <= limits.max_mutations)
        .ok_or(Error::Budget("Agent publication combined mutations"))?;
    progress.verify(tx)?;
    let receipt = json!({"schema":"tos_agent_correction_publication_v1","binding":binding,
        "source_inputs_sha256":after_source.digest(),"sql_mutations":total,
        "source_header":paired.publication.source_header.as_ref().map(view).transpose()?,
        "catalog":paired.publication.catalog.as_ref().map(view).transpose()?,
        "catalog_digest":paired.publication.catalog_digest,
        "semantic_report":paired.publication.semantic_report.as_ref().map(view).transpose()?,
        "semantic_report_sha256":paired.publication.semantic_report_sha256,
        "publication_changed":paired.publication.publication_changed,

        "transaction_id":observation.transaction_id(),"source_manifest_sha256":observation.manifest_sha256().to_hex(),
        "affected_nodes":result.affected_nodes,"normalized_node_count":result.normalized_nodes,"normalized_relation_count":result.normalized_relations,
        "changed_nodes":result.changed_nodes,"changed_relations":result.changed_relations,
        "reverse_dependent_claims":assembled.reverse_dependent_claims,
        "source_command_committed":true,"prepared_committed":false,"source_transition_verified":true,
        "descriptive_closure_verified":true,"cross_filesystem_atomic":false,
        "global_source_currentness_verified":false,"is_semantic_acceptance":false});
    bytes::canonical(&receipt, limits.max_metadata_bytes.min(META))?;
    Ok(Applied {
        connection: (&**tx as *const Connection) as usize,
        total_changes: tx.total_changes(),
        schema: tx.query_row("PRAGMA main.schema_version", [], |r| r.get(0))?,
        source: after_source,
        catalog: after,
        receipt,
        limits,
    })
}
