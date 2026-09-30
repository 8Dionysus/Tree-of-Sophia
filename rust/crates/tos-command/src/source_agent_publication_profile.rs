//! Actual executing-image profile and explicit compatible Agent transition.
//! Only an explicitly reviewed before/after implementation pair can bootstrap
//! the native profiles. Domain rows, declarations and membership do not change;
//! configuration and descriptor provenance remain with the selected caller.
use super::{
    source_claim_publication::{
        self as claim, ClaimPublicationLimits, ClaimPublicationProgress, ExecutingImage,
        NativeClaimProfiles, ReviewedClaimProfileTransition,
    },
    source_claim_publication_bytes as bytes,
    source_claim_publication_context::Context,
    source_claim_publication_dependencies::{self as deps, Dependencies},
};
use rusqlite::Transaction;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::atomic::AtomicBool, time::Instant};
use tos_compiler::{
    Error, KnowledgeRegistry, Result,
    knowledge_normalization::SourceRow,
    local_prepared::{PreparedChange, PublicationLimits},
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::{CatalogInputs, SourceOrderProfile},
    prepared_semantic_index::SemanticMaintenanceLimits,
    prepared_source_binding::{
        PreparedSourceInputs, apply_source_bound_prepared_delta_transaction,
        read_prepared_source_inputs_transaction,
    },
};
use tos_foundation::{JsonLimits, JsonValue, emit_value_preserved_json};
const META: usize = 1_048_576;
fn view(value: &JsonValue) -> Result<Value> {
    let raw = emit_value_preserved_json(
        value,
        JsonLimits::new(META, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Agent profile detached view"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    bytes::parse(&raw, META)
}
fn stable(value: &Value) -> Result<String> {
    SourceRow::parse(&bytes::canonical(value, META)?, META)?.stable_digest()
}
pub struct NativeAgentExecution {
    image: ExecutingImage,
    profiles: NativeClaimProfiles,
    agent_profile: String,
}
impl NativeAgentExecution {
    pub fn observe(deadline: Instant, cancelled: &AtomicBool) -> Result<Self> {
        let image = ExecutingImage::observe(deadline, cancelled)?;
        let profiles = claim::profiles_for_image(image.digest())?;
        let agent_profile = bytes::row_digest(
            &json!({
                "schema":"tos_native_agent_descriptive_publication_v1",
                "executing_artifact_sha256":image.digest()
            }),
            META,
        )?;
        Ok(Self {
            image,
            profiles,
            agent_profile,
        })
    }
    pub fn verify(&self) -> Result<()> {
        self.image.verify()
    }
    pub fn dependency_implementation(&self) -> &str {
        &self.profiles.dependency_implementation
    }
    pub fn declaration(&self) -> &str {
        &self.profiles.declaration
    }
    pub fn processor(&self) -> &str {
        self.image.digest()
    }
    pub fn agent_publication(&self) -> &str {
        &self.agent_profile
    }
    fn verify_baseline(
        &self,
        source: &PreparedSourceInputs,
        catalog: &CatalogInputs,
    ) -> Result<()> {
        self.verify()?;
        let selected = view(&source.value()?)?;
        if claim::source_vector(selected.clone(), PublicationLimits::default())?.raw()
            != source.raw()
            || catalog.source_order_profile != SourceOrderProfile::SourceGraphId
        {
            return Err(Error::Invalid(
                "Agent explicit canonical source vector requires bootstrap",
            ));
        }
        let header = view(&catalog.header)?;
        if header["source_revision"] != source.source_revision() {
            return Err(Error::Invalid("Agent catalog/source revision differs"));
        }
        let binding = header
            .get("normalization_binding")
            .ok_or(Error::Invalid("Agent normalization binding absent"))?;
        let object = binding
            .as_object()
            .ok_or(Error::Invalid("Agent normalization binding object"))?;
        if object.len() != 5
            || object.get("schema").and_then(Value::as_str)
                != Some("tos_knowledge_graph_normalization_binding_v1")
        {
            return Err(Error::Invalid("Agent normalization binding profile"));
        }
        for key in [
            "processor_digest",
            "configuration_digest",
            "entity_registry_digest",
            "relation_registry_digest",
        ] {
            bytes::sha(bytes::text(binding, key)?)?;
        }
        let registry = KnowledgeRegistry::parse(
            &bytes::canonical(&view(&catalog.entity_registry)?, META)?,
            &bytes::canonical(&view(&catalog.relation_registry)?, META)?,
        )?;
        if bytes::text(binding, "processor_digest")? != self.processor()
            || bytes::text(binding, "entity_registry_digest")? != registry.entity_semantic_digest
            || bytes::text(binding, "relation_registry_digest")?
                != registry.relation_semantic_digest
        {
            return Err(Error::Invalid(
                "Agent normalization processor/registry bootstrap required",
            ));
        }
        let dependencies = selected["dependencies"]
            .as_object()
            .ok_or(Error::Invalid("Agent dependency profile"))?;
        if dependencies
            .get("declaration-profile")
            .and_then(Value::as_str)
            != Some(self.declaration())
            || dependencies.get("normalization").and_then(Value::as_str)
                != Some(stable(binding)?.as_str())
            || dependencies.get("entity-registry").and_then(Value::as_str)
                != Some(stable(&view(&catalog.entity_registry)?)?.as_str())
            || dependencies
                .get("relation-registry")
                .and_then(Value::as_str)
                != Some(stable(&view(&catalog.relation_registry)?)?.as_str())
            || !dependencies.contains_key("nonparticipating-profile")
        {
            return Err(Error::Invalid(
                "Agent explicit source/normalization/declaration profile requires bootstrap",
            ));
        }
        Ok(())
    }
    /// Verify the exact selected configuration binding and registry identities.
    /// This does not derive or attest descriptor/configuration provenance.
    pub fn verify_source(
        &self,
        source: &PreparedSourceInputs,
        catalog: &CatalogInputs,
    ) -> Result<()> {
        self.verify_baseline(source, catalog)?;
        let selected = view(&source.value()?)?;
        if selected["dependencies"]["agent-publication-profile"] != self.agent_profile {
            return Err(Error::Invalid(
                "Agent execution profile explicit bootstrap required",
            ));
        }
        Ok(())
    }
}

/// Explicit reviewed implementation-only transition in the caller transaction.
/// Domain rows, root snapshots, publication token, declarations and membership
/// are unchanged. A failed check requires rollback of the whole caller tx.
#[allow(clippy::too_many_arguments)]
pub fn bootstrap_reviewed_agent_execution_profile_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before_source: &PreparedSourceInputs,
    before_catalog: &CatalogInputs,
    execution: &NativeAgentExecution,
    review: &ReviewedClaimProfileTransition,
    reviewed_after_agent_sha256: &str,
    progress: &ClaimPublicationProgress<'_>,
    operation: ClaimPublicationLimits,
    publication_limits: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
) -> Result<Value> {
    operation.validate()?;
    publication_limits.validate()?;
    before_catalog.validate()?;
    progress.verify(tx)?;
    execution.verify()?;
    for digest in [
        &review.dependency_implementation_before,
        &review.declaration_before,
        &review.agent_publication_before,
        &review.native_normalization_processor_sha256,
    ] {
        bytes::sha(digest)?;
    }
    bytes::sha(reviewed_after_agent_sha256)?;
    let mut selected = view(&before_source.value()?)?;
    if claim::source_vector(selected.clone(), publication_limits)?.raw() != before_source.raw()
        || before_catalog.source_order_profile != SourceOrderProfile::SourceGraphId
        || review.review_ref.trim().is_empty()
        || review.review_ref.len() > 4096
        || selected["dependencies"]["agent-publication-profile"] != review.agent_publication_before
        || selected["dependencies"]["declaration-profile"] != review.declaration_before
        || selected["dependencies"]["normalization"] != stable(&review.normalization_before)?
        || view(&before_catalog.header)?["normalization_binding"] != review.normalization_before
        || selected["dependencies"]
            .get("claim-publication-profile")
            .and_then(Value::as_str)
            != review.claim_publication_before.as_deref()
        || reviewed_after_agent_sha256 != execution.agent_publication()
        || review.agent_publication_before == reviewed_after_agent_sha256
        || review.native_normalization_processor_sha256 != execution.processor()
        || bytes::text(&review.normalization_after, "processor_digest")? != execution.processor()
    {
        return Err(Error::Invalid(
            "Agent exact reviewed implementation transition required",
        ));
    }
    if let Some(digest) = &review.claim_publication_before {
        bytes::sha(digest)?;
    }
    if operation.max_claims == 0 || operation.max_claims > 512 {
        return Err(Error::Budget("Agent profile dependency claim bound"));
    }
    let start = tx.total_changes();
    if read_prepared_source_inputs_transaction(tx, expected, before_catalog, publication_limits)?
        .raw()
        != before_source.raw()
    {
        return Err(Error::Invalid("Agent profile selected predecessor CAS"));
    }
    let binding = view(expected)?;
    let mut dependency_limits = deps::Limits::default();
    dependency_limits.max_claims = operation.max_claims;
    let mut dependencies = Dependencies::new(tx, dependency_limits)?;
    let dependency_state = dependencies.state(
        &binding,
        before_source.digest(),
        &review.declaration_before,
        &review.dependency_implementation_before,
    )?;
    let mut context = Context::new(tx, dependency_limits);
    let context_state = context.state(&binding, before_source.digest())?;
    selected["dependencies"]["declaration-profile"] = json!(execution.declaration());
    selected["dependencies"]["agent-publication-profile"] = json!(execution.agent_publication());
    selected["dependencies"]["normalization"] = json!(stable(&review.normalization_after)?);
    let after = claim::source_vector(selected, publication_limits)?;
    let after_catalog = claim::catalog_after(before_catalog, &after, &review.normalization_after)?;
    execution.verify_source(&after, &after_catalog)?;
    let mut paired_limits = publication_limits;
    paired_limits.max_mutations = paired_limits
        .max_mutations
        .checked_sub(2)
        .ok_or(Error::Budget("Agent profile finalizer allowance"))?;
    let paired = if review.normalization_before == review.normalization_after {
        apply_source_bound_prepared_delta_transaction(
            tx,
            expected,
            before_source,
            &after,
            before_catalog,
            &after_catalog,
            std::iter::empty::<Result<PreparedChange>>(),
            paired_limits,
            catalog_limits,
            semantic_limits,
            execution.processor(),
        )?
    } else {
        let mut named = BTreeMap::new();
        for (key, old, new) in [
            (
                "declaration-profile",
                review.declaration_before.clone(),
                execution.declaration().to_owned(),
            ),
            (
                "agent-publication-profile",
                review.agent_publication_before.clone(),
                execution.agent_publication().to_owned(),
            ),
            (
                "normalization",
                stable(&review.normalization_before)?,
                stable(&review.normalization_after)?,
            ),
        ] {
            if old != new {
                named.insert(key.to_owned(), (old, new));
            }
        }
        let transition = tos_compiler::local_prepared::ReviewedNormalizationTransition {
            before_processor: bytes::text(&review.normalization_before, "processor_digest")?,
            after_processor: execution.processor(),
            review_ref: &review.review_ref,
        };
        tos_compiler::prepared_source_binding::transition_prepared_source_profiles_transaction(
            tx,
            expected,
            before_source,
            &after,
            before_catalog,
            &after_catalog,
            &transition,
            &named,
            paired_limits,
            catalog_limits,
            semantic_limits,
        )?
    };
    let successor_binding = view(&paired.publication.binding)?;
    dependencies.rebind_reviewed(
        dependency_state,
        &successor_binding,
        after.digest(),
        execution.declaration(),
        execution.dependency_implementation(),
    )?;
    context.rebind(context_state, &successor_binding, after.digest())?;
    let total = tx
        .total_changes()
        .checked_sub(start)
        .filter(|n| *n <= publication_limits.max_mutations)
        .ok_or(Error::Budget(
            "Agent execution profile combined pairing mutations",
        ))?;
    progress.verify(tx)?;
    execution.verify()?;
    let result = json!({"binding":successor_binding,"source_inputs_sha256":after.digest(),"sql_mutations":total,
        "source_header":paired.publication.source_header.as_ref().map(view).transpose()?,
        "catalog":paired.publication.catalog.as_ref().map(view).transpose()?,"catalog_digest":paired.publication.catalog_digest,
        "semantic_report":paired.publication.semantic_report.as_ref().map(view).transpose()?,"semantic_report_sha256":paired.publication.semantic_report_sha256,
        "publication_changed":paired.publication.publication_changed,"roots_paired_in_caller_transaction":paired.roots_paired_in_caller_transaction,
        "source_transition_verified":false,"target_closure_verified":false,"semantic_acceptance":false,"consumer_switched":false,
        "agent_context_selection_paired":true,"agent_context_membership_changed":false,"normalized_row_changes_supplied":0,
        "execution_profile_current":true,"compatibility_review_ref":review.review_ref,"compatibility_verified_by_helper":false,"descriptor_configuration_provenance_verified":false,
        "before_execution_profile_sha256":review.agent_publication_before,"after_execution_profile_sha256":execution.agent_publication()});
    bytes::canonical(&result, publication_limits.max_metadata_bytes)?;
    Ok(result)
}
