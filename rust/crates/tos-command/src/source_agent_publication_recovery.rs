//! Read-only reconciliation of an explicitly selected Agent publication.
//! The CLI owns the protected DB anchor and final disclosure fence. This never
//! discovers a current binding, retries a command or grants source authority.
use crate::{
    source_agent_publication::NativeAgentExecution,
    source_claim_publication::{ClaimPublicationLimits, ClaimPublicationProgress},
    source_claim_publication_bytes as bytes,
    source_claim_publication_context::Context,
    source_claim_publication_dependencies::{Dependencies, Limits},
    source_claim_publication_roots::{MutationLimits, Roots, Snapshot},
    source_creation_store::CommittedRecordObservation,
};
use rusqlite::Transaction;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::atomic::AtomicBool, time::Instant};
use tos_compiler::{
    Error, Result,
    local_prepared::PublicationLimits,
    prepared_catalog_semantics::CatalogInputs,
    prepared_source_binding::{PreparedSourceInputs, read_prepared_source_inputs_transaction},
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json};

fn view(value: &JsonValue) -> Result<Value> {
    let limits = JsonLimits::new(1_048_576, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("Agent recovery JSON limits"))?;
    let raw = emit_value_preserved_json(value, limits)
        .map_err(|_| Error::Budget("Agent recovery JSON bytes"))?;
    bytes::parse(&raw, 1_048_576)
}

/// Called within one read transaction while the authenticated source lock is
/// held. The caller must end that transaction and verify its held DB anchor
/// before disclosing the result. No SQL mutation or COW staging occurs here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reconcile(
    tx: &Transaction<'_>,
    progress: &ClaimPublicationProgress<'_>,
    observation: &CommittedRecordObservation<'_>,
    selected: &JsonValue,
    source: &PreparedSourceInputs,
    catalog: &CatalogInputs,
    execution: &NativeAgentExecution,
    operation: ClaimPublicationLimits,
    publication: PublicationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value> {
    operation.validate()?;
    publication.validate()?;
    progress.verify(tx)?;
    let query_only: i64 = tx.query_row("PRAGMA query_only", [], |row| row.get(0))?;
    if query_only != 1 || tx.total_changes() != 0 {
        return Err(Error::Invalid(
            "Agent recovery requires fresh read-only selection",
        ));
    }
    let changes = tx.total_changes();
    execution.verify_source(source, catalog)?;
    if read_prepared_source_inputs_transaction(tx, selected, catalog, publication)?.raw()
        != source.raw()
    {
        return Err(Error::Invalid("Agent recovery paired selection"));
    }
    let binding = view(selected)?;
    let limits = Limits {
        max_claims: operation.max_claims,
        max_read_bytes: operation.max_bytes,
        ..Limits::default()
    };
    Dependencies::new(tx, limits)?.state(
        &binding,
        source.digest(),
        execution.declaration(),
        execution.dependency_implementation(),
    )?;
    Context::new(tx, limits).state(&binding, source.digest())?;
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
    let snapshot = snapshots
        .get("source-catalog")
        .ok_or(Error::Invalid("Agent recovery source catalog"))?;
    let header = &snapshot.manifest["header"];
    let transition = json!({"transaction_id": observation.transaction_id(),
        "manifest_sha256": observation.manifest_sha256().to_prefixed(),
        "record_id": observation.record_id()});
    if header["last_transition"] != transition
        || header["source_publication"] != view(observation.source_publication())?
    {
        return Err(Error::Invalid("Agent recovery selected transition"));
    }
    let mut roots = Roots::new(
        snapshots,
        MutationLimits {
            input: operation.max_bytes,
            decoded: operation.max_bytes,
            result: operation.max_bytes,
            ..MutationLimits::default()
        },
    )?;
    let row = roots
        .get("source-catalog", "records", observation.record_id())?
        .ok_or(Error::Invalid("Agent recovery current record absent"))?;
    let current = observation.binding();
    if row["source"]["raw_sha256"] != current.current.sha256.to_hex()
        || row["source"]["raw_bytes"].as_u64() != Some(current.current.raw.len() as u64)
        || row["source"]["record_ref"] != view(&current.current.subject)?
        || row["source"]["source_ref"] != current.source_path.as_str()
    {
        return Err(Error::Invalid("Agent recovery current record differs"));
    }
    observation
        .verify_current(deadline, cancelled)
        .map_err(|e| Error::Source(format!("Agent recovery source fence: {e:?}")))?;
    execution.verify()?;
    progress.verify(tx)?;
    if tx.total_changes() != changes {
        return Err(Error::Invalid("Agent recovery mutated transaction"));
    }
    Ok(json!({"schema":"tos_agent_publication_reconciliation_v1",
        "binding":binding,"source_inputs_sha256":source.digest(),
        "transaction":transition,"prepared_committed":true,
        "reconciled":true,"replayed":false,"is_semantic_acceptance":false}))
}

/// Recover only a candidate header from the existing prepared descriptor.
/// This is not admission: the caller must pass it through `reconcile`, which
/// checks the descriptor digest and complete selected source/catalog pairing.
pub(crate) fn candidate_header(tx: &Transaction<'_>, cap: usize) -> Result<JsonValue> {
    let cap = cap.min(1_048_576);
    let mut statement = tx.prepare("SELECT CASE WHEN typeof(descriptor)='text' AND length(CAST(descriptor AS BLOB))<=? THEN descriptor END FROM prepared_state WHERE singleton=1 LIMIT 2")?;
    let mut rows = statement.query([cap])?;
    let raw: Option<String> = rows
        .next()?
        .ok_or(Error::Invalid("Agent recovery descriptor absent"))?
        .get(0)?;
    let raw = raw.ok_or(Error::Budget("Agent recovery descriptor bytes"))?;
    if rows.next()?.is_some() {
        return Err(Error::Invalid("Agent recovery descriptor cardinality"));
    }
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("Agent recovery descriptor limits"))?;
    let document = parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    document
        .root()
        .object_get("header")
        .filter(|value| value.as_object().is_some())
        .cloned()
        .ok_or(Error::Invalid("Agent recovery descriptor header"))
}
