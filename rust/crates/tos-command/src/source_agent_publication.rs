//! Agent source addressing paired with prepared rows, dependencies and context.
//! The caller owns source membership/locks and the complete transaction rollback.
//! Extending an address never updates an execution profile or admits a source.
pub use crate::source_agent_publication_commit::publish_committed_agent_correction;
pub use crate::source_agent_publication_profile::{
    NativeAgentExecution, bootstrap_reviewed_agent_execution_profile_transaction,
};

use crate::{
    source_claim_publication::ClaimPublicationProgress,
    source_claim_publication_bytes as bytes,
    source_claim_publication_context::Context,
    source_claim_publication_dependencies::{Dependencies, Limits},
};
use rusqlite::Transaction;
use serde_json::{Value, json};
use tos_compiler::{
    Error, Result,
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::CatalogInputs,
    prepared_semantic_index::SemanticMaintenanceLimits,
    prepared_source_binding::{
        PreparedSourceInputs, SourceMaintenanceReceipt,
        bootstrap_prepared_source_root_extension_transaction,
        read_prepared_source_inputs_transaction,
    },
};
use tos_foundation::{JsonLimits, JsonValue, emit_value_preserved_json};

fn view(value: &JsonValue, cap: usize) -> Result<Value> {
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("Agent addressing JSON limits"))?;
    let raw = emit_value_preserved_json(value, limits)
        .map_err(|_| Error::Budget("Agent addressing JSON bytes"))?;
    bytes::parse(&raw, cap)
}

pub(super) fn require_vector(inputs: &PreparedSourceInputs, cap: usize) -> Result<Value> {
    let value = bytes::parse(inputs.raw(), cap)?;
    for role in [
        "source-catalog",
        "source-navigation",
        "bibliographic-claims",
    ] {
        if !inputs.roots().contains_key(role) {
            return Err(Error::Invalid("Agent independent source roots required"));
        }
    }
    let mut digests = serde_json::Map::new();
    for (role, root) in inputs.roots() {
        let body = bytes::parse(&root.root_bytes, 262_144)?;
        let header = body["header"]
            .as_object()
            .ok_or(Error::Invalid("Agent raw root header"))?;
        if header.contains_key("source_revision") {
            return Err(Error::Invalid("Agent raw root contains derived revision"));
        }
        let schema = match role.as_str() {
            "source-navigation" => Some("tos_agent_source_navigation_rows_v1"),
            "bibliographic-claims" => Some("tos_agent_bibliographic_rows_v1"),
            _ => None,
        };
        if let Some(schema) = schema {
            if body["header"] != json!({"schema_version":schema}) {
                return Err(Error::Invalid("Agent independent raw row profile"));
            }
            let fields: &[(&str, &str)] = if role == "source-navigation" {
                &[("nodes", "node_id"), ("edges", "edge_id")]
            } else {
                &[
                    ("nodes", "node_id"),
                    ("edges", "edge_id"),
                    ("claim_traces", "claim_ref"),
                ]
            };
            if body["logical_schema"] != schema
                || body["collections"].as_object().map(|v| v.len()) != Some(fields.len())
                || fields.iter().any(|(name, key)| {
                    body["collections"][*name]["key_field"] != *key
                        || body["collections"][*name]["order_fields"] != json!([key])
                })
            {
                return Err(Error::Invalid("Agent raw collection identity/order"));
            }
        }
        digests.insert(role.clone(), json!(root.snapshot_sha256));
    }
    let revision = bytes::row_digest(
        &json!({
            "schema":"tos_agent_source_root_vector_v1", "roots":digests,
            "source_publication":value["source_publication"], "dependencies":value["dependencies"],
        }),
        cap,
    )?;
    if inputs.source_revision() != revision {
        return Err(Error::Invalid(
            "Agent source vector revision requires bootstrap",
        ));
    }
    Ok(value)
}

/// Result remains uncommitted, and leaves normalized rows, declarations and
/// context membership unchanged. All counts include the dependent finalizers.
pub struct AgentSourceExtensionReceipt {
    pub paired: SourceMaintenanceReceipt,
    pub added_root: String,
    pub source_dependencies_paired: bool,
    pub agent_context_selection_paired: bool,
    pub agent_context_membership_changed: bool,
    pub source_root_admission_verified: bool,
    pub normalized_row_changes_supplied: u64,
}

/// Native equivalent of bootstrap_agent_source_addressing_extension_transaction.
/// Reuses the same caller-owned progress handler as whole Claim publication.
/// Neither successful execution nor a named root proves source membership.
pub fn bootstrap_agent_source_addressing_extension_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before_source: &PreparedSourceInputs,
    after_source: &PreparedSourceInputs,
    added_root: &str,
    before: &CatalogInputs,
    after: &CatalogInputs,
    progress: &ClaimPublicationProgress<'_>,
    publication_limits: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    dependency_implementation_sha256: &str,
    normalization_processor_sha256: &str,
) -> Result<AgentSourceExtensionReceipt> {
    publication_limits.validate()?;
    progress.verify(tx)?;
    if publication_limits.max_mutations <= 4 {
        return Err(Error::Budget(
            "Agent extension combined finalizer allowance",
        ));
    }
    let journal: String = tx.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    if journal != "wal" {
        return Err(Error::Invalid(
            "Agent publication requires explicit WAL profile",
        ));
    }
    let cap = publication_limits.max_metadata_bytes.min(1_048_576);
    let source = require_vector(before_source, cap)?;
    require_vector(after_source, cap)?;
    let declaration_profile = bytes::text(&source["dependencies"], "declaration-profile")?;
    let before_binding = view(expected, cap)?;
    if read_prepared_source_inputs_transaction(tx, expected, before, publication_limits)?.raw()
        != before_source.raw()
    {
        return Err(Error::Invalid("Agent extension source predecessor CAS"));
    }
    let start = tx.total_changes();
    let limits = Limits {
        max_state: cap,
        max_read_bytes: usize::try_from(publication_limits.max_bytes)
            .unwrap_or(usize::MAX)
            .min(Limits::default().max_read_bytes),
        // Dependencies counts all writes since construction, including other
        // lanes. The combined caller budget, not only dependency writes, applies.
        max_writes: publication_limits.max_mutations,
        ..Limits::default()
    };
    let mut dependencies = Dependencies::new(tx, limits)?;
    let state = dependencies.state(
        &before_binding,
        before_source.digest(),
        declaration_profile,
        dependency_implementation_sha256,
    )?;
    let mut context = Context::new(tx, limits);
    let context_state = context.state(&before_binding, before_source.digest())?;
    let finalizer = dependencies.stage(
        state,
        &[],
        after_source.digest(),
        after_source.source_revision(),
    )?;
    let reserve = (finalizer as u64)
        .checked_add(1)
        .ok_or(Error::Budget("Agent extension finalizers"))?;
    let used = tx
        .total_changes()
        .checked_sub(start)
        .ok_or(Error::Invalid("Agent extension mutation counter"))?;
    let mut paired_limits = publication_limits;
    paired_limits.max_mutations = publication_limits
        .max_mutations
        .checked_sub(used)
        .and_then(|left| left.checked_sub(reserve))
        .filter(|left| *left >= 2)
        .ok_or(Error::Budget("Agent extension finalizer reservation"))?;
    let mut paired = bootstrap_prepared_source_root_extension_transaction(
        tx,
        expected,
        before_source,
        after_source,
        added_root,
        before,
        after,
        paired_limits,
        catalog_limits,
        semantic_limits,
        normalization_processor_sha256,
    )?;
    let binding = view(&paired.publication.binding, cap)?;
    dependencies.finalize(
        &binding,
        after_source.digest(),
        declaration_profile,
        dependency_implementation_sha256,
    )?;
    context.rebind(context_state, &binding, after_source.digest())?;
    let total = tx
        .total_changes()
        .checked_sub(start)
        .filter(|total| *total <= publication_limits.max_mutations)
        .ok_or(Error::Budget("Agent extension cumulative mutations"))?;
    progress.verify(tx)?;
    paired.sql_mutations = total;
    Ok(AgentSourceExtensionReceipt {
        paired,
        added_root: added_root.to_owned(),
        source_dependencies_paired: true,
        agent_context_selection_paired: true,
        agent_context_membership_changed: false,
        source_root_admission_verified: false,
        normalized_row_changes_supplied: 0,
    })
}
