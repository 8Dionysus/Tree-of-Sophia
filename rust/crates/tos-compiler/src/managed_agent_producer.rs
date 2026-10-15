//! Initial full managed Agent catalog/navigation selection. Current authority
//! and immutable installed custody remain with the actual command owner.
use crate::{
    Error, ExpectedSourceScope, FullKnowledgeLimits, KnowledgeRegistry, KnowledgeSealReceipt,
    KnowledgeSelectedExpectation, ManagedSourceProofV1, NativeFamilyInputs, NativeProducerLimits,
    NavigationHeaderClaim, NavigationOriginalInput, NavigationOriginalLimits,
    NavigationOriginalReceipt, QueryVocabulary, Result, SourceBinding,
    knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage, StageIsolation,
        StageLimits, StageOwner, StageReceipt, WritePhase,
    },
    managed_source::ManagedProducerProof,
    source_bibliographic_source::SourceCatalogInputPlan,
    source_navigation_source::NavigationSourceProjection,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tos_foundation::{Digest256, Digest256Hasher, SourceRevision};
use tos_source_store::SourceMembershipV1;

/// Derive the exact managed-model stage receipt binding shared by the producer
/// and its source owner. This is a descriptive input binding, not admission
/// authority.
pub fn managed_model_receipt_binding<P: ManagedProducerProof>(
    export_binding: &SourceBinding,
    proof: &P,
    collections: &[InputCollectionReceipt],
    max_header_bytes: usize,
) -> Result<SourceBinding> {
    let collection_value =
        serde_json::to_value(collections).map_err(|error| Error::Source(error.to_string()))?;
    let receipt_bytes =
        crate::source_bibliographic_render::encode(&collection_value, max_header_bytes)?;
    let mut binding = export_binding.clone();
    binding.source_cut = proof.stage_source_cut()?;
    binding.membership_root = proof.current_root().to_owned();
    binding.through_commit_seq = proof.through_commit_seq();
    binding.projection_root_sha256 = Digest256::of_bytes(&receipt_bytes).to_hex();
    Ok(binding)
}

/// Only the actual full invocation below can construct this outcome. Public
/// descriptive receipts and arbitrary cold models cannot be converted into it.
pub struct CompletedManagedAgentProducer<P: ManagedProducerProof = ManagedSourceProofV1> {
    path: PathBuf,
    stage: StageReceipt,
    seal: KnowledgeSealReceipt,
    expectation: KnowledgeSelectedExpectation,
    navigation: NavigationOriginalReceipt,
    source: P,
    export_revision: SourceRevision,
    export_membership: SourceMembershipV1,
    source_catalog_root: String,
    schema_worker_sha256: String,
    schema_set_sha256: String,
    successor_catalogue_input: Option<(String, u64)>,
    created_record_input: Option<(String, String, u64)>,
    created_forms_input: Option<(String, String, u64)>,
}
impl<P: ManagedProducerProof> CompletedManagedAgentProducer<P> {
    pub fn artifact_path(&self) -> &Path {
        &self.path
    }
    pub fn stage(&self) -> &StageReceipt {
        &self.stage
    }
    pub fn seal(&self) -> &KnowledgeSealReceipt {
        &self.seal
    }
    pub fn expectation(&self) -> &KnowledgeSelectedExpectation {
        &self.expectation
    }
    pub fn navigation_original(&self) -> &NavigationOriginalReceipt {
        &self.navigation
    }
    pub fn source_proof(&self) -> &P {
        &self.source
    }
    pub fn export_source_revision(&self) -> SourceRevision {
        self.export_revision
    }
    pub fn export_membership(&self) -> SourceMembershipV1 {
        self.export_membership
    }
    /// Genuine initial full catalogue root retained through the managed chain.
    /// Current catalogue inventory is separately bound by source_proof().
    pub fn source_catalog_root_sha256(&self) -> &str {
        &self.source_catalog_root
    }
    pub fn schema_worker_sha256(&self) -> &str {
        &self.schema_worker_sha256
    }
    pub fn schema_set_sha256(&self) -> &str {
        &self.schema_set_sha256
    }
    /// Actual successor catalogue packets accepted in strict record_id order.
    /// SHA binds SourceRecordDigestV1 canonical entry bytes plus one LF per row.
    /// Initial export correspondence is separate and returns None here.
    pub fn successor_catalogue_input(&self) -> Option<(&str, u64)> {
        self.successor_catalogue_input
            .as_ref()
            .map(|(sha, count)| (sha.as_str(), *count))
    }
    /// Exact addressed source bytes consumed by the successful invocation.
    /// SHA is hexadecimal without a prefix; these facts issue no source authority.
    pub fn created_record_input(&self) -> Option<(&str, &str, u64)> {
        self.created_record_input
            .as_ref()
            .map(|(path, sha, size)| (path.as_str(), sha.as_str(), *size))
    }
    pub fn created_forms_input(&self) -> Option<(&str, &str, u64)> {
        self.created_forms_input
            .as_ref()
            .map(|(path, sha, size)| (path.as_str(), sha.as_str(), *size))
    }
}
fn charge(work: &mut u64, bytes: usize, cap: u64) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .filter(|n| *n <= cap)
        .ok_or(Error::Budget("managed selected input preparation work"))?;
    Ok(())
}
/// The opaque source plan and projection must come from the existing complete
/// catalog/Versions/navigation invocation on the genuine initial export. This
/// function owns new raw ingestion, native joins, all selected components and
/// finish; it never accepts caller-created normalized rows or producer receipts.
/// Non-navigation selected families require their real complete reconstruction,
/// and are explicitly refused rather than replaced by empty collections.
#[allow(clippy::too_many_arguments)]
pub fn prepare_managed_agent_selected_model<P: ManagedProducerProof>(
    source_plan: &SourceCatalogInputPlan,
    projection: &NavigationSourceProjection,
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    proof: P,
    candidate: &Path,
    binding: SourceBinding,
    stage_limits: StageLimits,
    owner: &dyn StageOwner,
    isolation: &dyn StageIsolation,
    entity_raw: &[u8],
    relation_raw: &[u8],
    descriptor_raw: &[u8],
    supported_profiles: &[&str],
    native_limits: NativeProducerLimits,
    original_limits: NavigationOriginalLimits,
    full_limits: FullKnowledgeLimits,
    saved_lenses: &[Value],
    owner_receipt_id: String,
    build_header: impl FnOnce(&mut KnowledgeStage<'_>, &KnowledgeRegistry) -> Result<Value>,
) -> Result<CompletedManagedAgentProducer<P>> {
    proof.validate()?;
    let export_revision = source_plan.source_revision()?;
    let export_membership = source_plan.source_membership();
    let original_binding = source_plan.input_receipt().binding;
    if proof.delta().is_some()
        || proof.initial_export().0 != export_revision.0.to_hex()
        || proof.initial_export().1 != export_membership.digest.to_hex()
        || proof.initial_export().2 != export_membership.count
        || projection.source_binding["source_revision"] != export_revision.0.to_hex()
        || projection.source_binding["membership_sha256"] != export_membership.digest.to_hex()
        || projection.source_binding["membership_count"] != export_membership.count
        || projection.source_binding["stage_source_cut"] != original_binding.source_cut
    {
        return Err(Error::Invalid(
            "managed initial full source invocation correspondence",
        ));
    }
    source_plan.verify_selected_member_bytes(
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        entity_raw,
    )?;
    source_plan.verify_selected_member_bytes(
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        relation_raw,
    )?;
    let schema_set_sha256 = validator
        .schemas(export_revision)?
        .execution_binding()
        .schema_set_sha256
        .to_hex();
    // Complete the genuine catalog/Versions/navigation schema operation before
    // issuing a successful independently sealed selected outcome.
    validator.finish()?;
    build_selected(
        &projection.value,
        &projection.catalog_root_sha256,
        export_revision,
        export_membership,
        validator.worker.sha256.to_hex(),
        schema_set_sha256,
        proof,
        candidate,
        binding,
        stage_limits,
        owner,
        isolation,
        entity_raw,
        relation_raw,
        descriptor_raw,
        supported_profiles,
        native_limits,
        original_limits,
        full_limits,
        saved_lenses,
        owner_receipt_id,
        0,
        None,
        None,
        None,
        build_header,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_selected<P: ManagedProducerProof>(
    projection: &Value,
    source_catalog_root: &str,
    export_revision: SourceRevision,
    export_membership: SourceMembershipV1,
    schema_worker_sha256: String,
    schema_set_sha256: String,
    proof: P,
    candidate: &Path,
    binding: SourceBinding,
    stage_limits: StageLimits,
    owner: &dyn StageOwner,
    isolation: &dyn StageIsolation,
    entity_raw: &[u8],
    relation_raw: &[u8],
    descriptor_raw: &[u8],
    supported_profiles: &[&str],
    native_limits: NativeProducerLimits,
    original_limits: NavigationOriginalLimits,
    full_limits: FullKnowledgeLimits,
    saved_lenses: &[Value],
    owner_receipt_id: String,
    preparation_work: u64,
    successor_catalogue_input: Option<(String, u64)>,
    created_record_input: Option<(String, String, u64)>,
    created_forms_input: Option<(String, String, u64)>,
    build_header: impl FnOnce(&mut KnowledgeStage<'_>, &KnowledgeRegistry) -> Result<Value>,
) -> Result<CompletedManagedAgentProducer<P>> {
    let vocabulary = QueryVocabulary::parse(descriptor_raw, supported_profiles)?;
    if vocabulary.sources.len() != 1
        || vocabulary.sources[0].adapter_profile != "source-navigation-node-edge-v1"
    {
        return Err(Error::ManagedSourceUnsupported(
            "unsupported managed selected source family closure",
        ));
    }
    let registered = &vocabulary.sources[0];
    let mut work = preparation_work;
    let mut collections = Vec::new();
    for (name, id_field) in [("nodes", "node_id"), ("edges", "edge_id")] {
        let values = projection[name]
            .as_array()
            .ok_or(Error::Invalid("managed original navigation collection"))?;
        if values.len() as u64 > stage_limits.sqlite.max_rows {
            return Err(Error::Budget("managed original navigation rows"));
        }
        let mut root = Digest256Hasher::new();
        let mut previous: Option<&str> = None;
        for value in values {
            let id = value[id_field]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(Error::Invalid("managed original navigation identity"))?;
            if previous.is_some_and(|old| old >= id) {
                return Err(Error::Invalid(
                    "managed original navigation order/duplicate",
                ));
            }
            previous = Some(id);
            let raw = crate::source_bibliographic_render::encode(
                value,
                stage_limits.sqlite.max_row_bytes,
            )?;
            charge(&mut work, raw.len(), stage_limits.sqlite.max_work_bytes)?;
            root.update(&(id.len() as u64).to_be_bytes());
            root.update(id.as_bytes());
            root.update(Digest256::of_bytes(&raw).as_bytes());
        }
        collections.push(InputCollectionReceipt {
            source_graph: registered.source_graph_id.clone(),
            collection: name.into(),
            input_role: registered.input_role.clone(),
            adapter_profile: registered.adapter_profile.clone(),
            expected_count: values.len() as u64,
            expected_root_sha256: root.finalize().to_hex(),
        });
    }
    original_limits.validate()?;
    // Detach header fields directly; never clone the complete row arrays.
    let header = Value::Object(
        projection
            .as_object()
            .ok_or(Error::Invalid("managed navigation header"))?
            .iter()
            .filter(|(key, _)| !["nodes", "edges", "rights"].contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    let raw_header = crate::source_bibliographic_render::encode(
        &header,
        native_limits.navigation_prepare.max_header_bytes,
    )?;
    charge(
        &mut work,
        raw_header.len(),
        stage_limits.sqlite.max_work_bytes,
    )?;
    let rights = projection["rights"]
        .as_array()
        .ok_or(Error::Invalid("managed original rights collection"))?;
    if rights.len() as u64 > original_limits.max_rows {
        return Err(Error::Budget("managed original rights rows"));
    }
    let mut rights_raw = Vec::new();
    let mut original_bytes = raw_header.len() as u64;
    for right in rights {
        let raw = crate::source_bibliographic_render::encode(right, original_limits.max_row_bytes)?;
        charge(&mut work, raw.len(), stage_limits.sqlite.max_work_bytes)?;
        original_bytes = original_bytes
            .checked_add(raw.len() as u64)
            .filter(|n| *n <= original_limits.max_total_bytes)
            .ok_or(Error::Budget("managed original rights/header bytes"))?;
        rights_raw.push(raw);
    }
    if original_bytes > original_limits.max_total_bytes {
        return Err(Error::Budget("managed original header bytes"));
    }
    let binding = managed_model_receipt_binding(
        &binding,
        &proof,
        &collections,
        full_limits.seal.max_header_bytes,
    )?;
    let mut stage = KnowledgeStage::create_managed_projection(
        candidate,
        stage_limits,
        ExactInputReceipt {
            binding,
            collections,
        },
        owner,
        isolation,
    )?;
    stage.charge_materialized(0, work)?;
    for (name, id_field) in [("nodes", "node_id"), ("edges", "edge_id")] {
        for value in projection[name]
            .as_array()
            .ok_or(Error::Invalid("managed navigation collection"))?
        {
            let raw = crate::source_bibliographic_render::encode(
                value,
                stage_limits.sqlite.max_row_bytes,
            )?;
            stage.ingest_input(InputRow {
                source_graph: &registered.source_graph_id,
                collection: name,
                id: value[id_field]
                    .as_str()
                    .ok_or(Error::Invalid("managed navigation ID"))?,
                payload: &raw,
            })?;
        }
    }
    if entity_raw.len() > full_limits.max_registry_bytes
        || relation_raw.len() > full_limits.max_registry_bytes
    {
        return Err(Error::Budget("managed selected registry bytes"));
    }
    let registry = KnowledgeRegistry::parse(entity_raw, relation_raw)?;
    let claim = NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&raw_header).to_hex(),
        raw_json: raw_header,
    };
    let rights_slices: Vec<&[u8]> = rights_raw.iter().map(Vec::as_slice).collect();
    let rights_root = crate::navigation_original_rights_root(&rights_slices);
    let mut additional = NativeFamilyInputs::bounded_from(native_limits);
    additional.navigation_original = Some(NavigationOriginalInput {
        rights: &rights_slices,
        expected_rights_root_sha256: &rights_root,
        limits: original_limits,
    });
    let navigation = if stage.exact_receipt()?.binding.reader_abi
        == tos_foundation::KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2
    {
        let prepared = crate::prepare_source_navigation(
            &mut stage,
            &vocabulary,
            &claim,
            native_limits.navigation_prepare,
        )?;
        let original = crate::retain_navigation_original(
            &mut stage,
            &vocabulary,
            &prepared,
            &claim,
            &rights_slices,
            &rights_root,
            original_limits,
        )?;
        crate::knowledge_source_navigation_prepare::clear_original_anchor_prepare(
            &mut stage,
            &vocabulary,
            &claim,
            &prepared,
            &original,
            native_limits.navigation_prepare,
        )?;
        original
    } else {
        crate::materialize_native_sources_with_inputs(
            &mut stage,
            &registry,
            entity_raw,
            relation_raw,
            &vocabulary,
            descriptor_raw,
            &claim,
            native_limits,
            additional,
        )?
        .navigation_original
        .ok_or(Error::Invalid("managed selected original receipt"))?
    };
    let normalized = stage.core_roots()?;
    let mut header = build_header(&mut stage, &registry)?;
    let after_header = stage.core_roots()?;
    if normalized.nodes != after_header.nodes
        || normalized.relations != after_header.relations
        || normalized.node_sha256 != after_header.node_sha256
        || normalized.relation_sha256 != after_header.relation_sha256
    {
        return Err(Error::Invalid(
            "managed header callback changed normalized core",
        ));
    }
    let object = header
        .as_object_mut()
        .ok_or(Error::Invalid("managed graph header object"))?;
    object.remove("source_revision");
    object.insert(
        "schema".into(),
        Value::String(crate::managed_source::MANAGED_GRAPH_SCHEMA.into()),
    );
    object.insert(
        "source_basis".into(),
        serde_json::to_value(proof.basis()).map_err(|e| Error::Source(e.to_string()))?,
    );
    let source_binding = stage.exact_receipt()?.binding.clone();
    let full = if tos_foundation::is_managed_navigation_model_abi(&source_binding.reader_abi) {
        crate::knowledge_full::compile_managed_navigation_components(
            &mut stage,
            &header,
            &registry,
            entity_raw,
            relation_raw,
            saved_lenses,
            &vocabulary,
            descriptor_raw,
            full_limits,
        )?
    } else {
        crate::compile_full_knowledge_components(
            &mut stage,
            &header,
            &registry,
            entity_raw,
            relation_raw,
            saved_lenses,
            &vocabulary,
            descriptor_raw,
            full_limits,
        )?
        .into()
    };
    let scopes = stage.with_connection(WritePhase::Finalize, |db| {
        let mut rows = db.prepare("SELECT source_graph,input_role,adapter_profile,expected_node_count,expected_relation_count,lower(hex(node_root_sha256)),lower(hex(relation_root_sha256)) FROM source_scope ORDER BY source_graph")?;
        let scopes = rows.query_map([], |r| Ok(ExpectedSourceScope { source_graph:r.get(0)?, input_role:r.get(1)?,
            adapter_profile:r.get(2)?, node_count:r.get(3)?, relation_count:r.get(4)?, node_root_sha256:r.get(5)?, relation_root_sha256:r.get(6)? }))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(scopes)
    })?;
    let output = stage.finish()?;
    let expectation = KnowledgeSelectedExpectation {
        model_sha256: output.sqlite_sha256.clone(),
        model_size_bytes: output.sqlite_size_bytes,
        owner_receipt_id,
        model_abi: full.seal.model_abi.clone(),
        managed_source_root_sha256: full.seal.managed_source_root_sha256.clone(),
        descriptor_sha256: vocabulary.descriptor_sha256.clone(),
        descriptor_version: vocabulary.descriptor_version,
        semantic_primitive_profile: vocabulary.semantic_primitive_profile.clone(),
        source_cut: output.source_cut.clone(),
        through_commit_seq: source_binding.through_commit_seq,
        membership_root: output.membership_root.clone(),
        entity_registry_id: registry.entity_registry_id.clone(),
        entity_registry_version: registry.entity_registry_version.to_string(),
        entity_registry_sha256: registry.entity_sha256.clone(),
        relation_registry_id: registry.relation_registry_id.clone(),
        relation_registry_version: registry.relation_registry_version.to_string(),
        relation_registry_sha256: registry.relation_sha256.clone(),
        graph_root_sha256: full.seal.graph_root_sha256.clone(),
        navigation_original_root_sha256: full.seal.navigation_original_root_sha256.clone(),
        philosophy_original_root_sha256: None,
        corpus_original_root_sha256: None,
        catalog_packet_sha256: full.catalog.catalog_packet_sha256,
        catalog_index_root_sha256: full.catalog.catalog_index_root_sha256,
        source_scope_root_sha256: full.source_scope.source_scope_root_sha256,
        search_index_root_sha256: full.seal.search_index_root_sha256.clone(),
        node_count: output.node_rows,
        relation_count: output.relation_rows,
        index_generation: source_binding.index_generation,
        route_map_version: source_binding.route_map_version,
        reader_abi: source_binding.reader_abi,
        authority_boundary: String::from_utf8(crate::knowledge_seal::canonical(
            &header["authority_boundary"],
            full_limits.seal.max_header_bytes,
        )?)
        .map_err(|e| Error::Source(e.to_string()))?,
        source_scopes: scopes,
        complete: true,
    };
    Ok(CompletedManagedAgentProducer {
        path: candidate.to_owned(),
        stage: output,
        seal: full.seal,
        expectation,
        navigation,
        source: proof,
        export_revision,
        export_membership,
        source_catalog_root: source_catalog_root.to_owned(),
        schema_worker_sha256,
        schema_set_sha256,
        successor_catalogue_input,
        created_record_input,
        created_forms_input,
    })
}

#[cfg(test)]
mod original_anchor_tests {
    use super::*;
    use crate::managed_source::{ManagedSourceGenerationV2, ManagedSourceProofV2};
    use crate::{ColdOpenLimits, ImmutableKnowledgeCustody, knowledge_stage::WritePhase};
    use serde_json::json;

    // Synthetic custody and source ownership only, matching the existing full
    // software fixture. This test issues no runtime admission or source proof.
    struct Fixture;
    impl StageOwner for Fixture {
        fn verify_receipt(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
        fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
    }
    impl StageIsolation for Fixture {
        fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> Result<()> {
            Ok(())
        }
    }
    impl ImmutableKnowledgeCustody for Fixture {
        fn verify(
            &self,
            file: &std::fs::File,
            expected: &KnowledgeSelectedExpectation,
        ) -> Result<()> {
            if file.metadata()?.len() != expected.model_size_bytes {
                return Err(Error::Invalid("fixture model length"));
            }
            Ok(())
        }
        fn verify_cold_resources(&self, _: ColdOpenLimits) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn managed_navigation_original_anchor_builds_finishes_and_cold_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor.sqlite");
        let hash = Digest256::of_bytes(b"Original anchor software fixture").to_hex();
        let proof = ManagedSourceProofV2 {
            schema: crate::managed_source::MANAGED_SOURCE_V2_SCHEMA.into(),
            generation: ManagedSourceGenerationV2 {
                domain: "test-only".into(),
                store_id: [0; 16],
                installed_generation_sha256: hash.clone(),
                through_commit_seq: 1,
                selected_audit_generation: 1,
                epoch: 1,
                definition_sha256: hash.clone(),
                bootstrap_source_revision: hash.clone(),
                bootstrap_membership_sha256: hash.clone(),
                bootstrap_members: 1,
                domain_sha256: hash.clone(),
                database_oid: 1,
                schema_profile_sha256: hash.clone(),
                addressed_metadata_tree_sha256: hash.clone(),
                addressed_metadata_members: 1,
                state_profile_sha256: hash.clone(),
                addressed_inventory_root_sha256: hash.clone(),
                log_sha256: hash.clone(),
                addressed_current_tree_sha256: hash.clone(),
                current_members: 1,
                addressed_history_tree_sha256: hash.clone(),
                history_members: 1,
                inventory_projection_sha256: hash.clone(),
            },
            initial_export_source_revision: hash.clone(),
            initial_export_membership_sha256: hash.clone(),
            initial_export_members: 1,
            delta: None,
        };
        let binding = SourceBinding {
            owner_profile: "tos-managed-source-navigation-v2".into(),
            source_cut: hash.clone(),
            through_commit_seq: 1,
            membership_root: hash.clone(),
            index_generation: hash.clone(),
            route_map_version: "test-only".into(),
            reader_abi: tos_foundation::KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2.into(),
            projection_root_sha256: hash.clone(),
            complete: true,
        };
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let mut descriptor: Value = serde_json::from_slice(include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        ))
        .unwrap();
        descriptor["sources"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v["source_graph_id"] == "source-navigation");
        let descriptor = serde_json::to_vec(&descriptor).unwrap();
        let node = json!({"node_id":"n:a","node_kind":"work","label":"Alpha","source_ref":"ToS/a.json","identity_status":"not_applicable","properties":{}});
        let edge = json!({"edge_id":"e:a","from_id":"n:a","to_id":"n:a","predicate_id":"references","edge_kind":"source_record_link","review_status":"not_applicable","source_refs":["ToS/a.json"]});
        let projection = json!({"schema_version":"tos_source_navigation_v1","authority_boundary":"software fixture, no source authority","counts":{"nodes":1,"edges":1,"rights":1},"nodes":[node],"edges":[edge],"rights":[{"rights_id":"test-only"}]});
        let mut limits =
            crate::native_snapshot_manifest::portable_native_snapshot_limits(60).unwrap();
        limits.stage.sqlite.max_output_bytes = 8 * 1024 * 1024;
        limits.stage.max_temp_bytes = 16 * 1024 * 1024;
        limits.stage.sqlite.max_work_bytes = 128 * 1024 * 1024;
        let original_limits = NavigationOriginalLimits {
            max_rows: 16,
            max_row_bytes: 1024 * 1024,
            max_total_bytes: 16 * 1024 * 1024,
        };
        let header_proof = proof.clone();
        let completed = build_selected(
            &projection,
            &hash,
            SourceRevision(Digest256::from_hex(&hash).unwrap()),
            SourceMembershipV1 {
                count: 1,
                digest: Digest256::from_hex(&hash).unwrap(),
            },
            hash.clone(),
            hash.clone(),
            proof,
            &path,
            binding,
            limits.stage,
            &Fixture,
            &Fixture,
            entity,
            relation,
            &descriptor,
            crate::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
            limits.native,
            original_limits,
            limits.full,
            &[],
            "test-only".into(),
            0,
            None,
            None,
            None,
            |stage, registry| {
                crate::managed_navigation_header_with_binding(
                    stage,
                    registry,
                    entity,
                    &header_proof,
                    Digest256::of_bytes(b"fixture processor"),
                    Digest256::of_bytes(b"fixture configuration"),
                )
            },
        )
        .unwrap();
        assert_eq!(
            completed.expectation().model_abi,
            tos_foundation::KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2
        );
        assert_eq!(
            (completed.stage().node_rows, completed.stage().relation_rows),
            (0, 0)
        );
        assert_eq!(
            (
                completed.navigation_original().nodes,
                completed.navigation_original().edges
            ),
            (1, 1)
        );
        assert!(completed.expectation().search_index_root_sha256.is_none());
        let cold = ColdOpenLimits {
            max_file_bytes: 8 * 1024 * 1024,
            max_vm_steps: 100_000_000,
            sqlite_cache_kib: 1024,
            max_rows: 100_000,
            max_work_bytes: 128 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_metadata_bytes: ColdOpenLimits::MAX_METADATA_BYTES,
            max_sources: 1,
        };
        let model = crate::open_selected_knowledge_model(
            completed.artifact_path(),
            completed.expectation().clone(),
            &Fixture,
            cold,
        )
        .unwrap();
        for (collection, key) in [("nodes", "node_id"), ("edges", "edge_id")] {
            let value = &projection[collection][0];
            let actual = managed_navigation_original_member_bytes(
                &model,
                collection,
                value[key].as_str().unwrap(),
                original_limits,
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                actual.raw,
                crate::source_bibliographic_render::encode(value, original_limits.max_row_bytes)
                    .unwrap()
            );
            assert_eq!(actual.visited_rows, 1);
        }
    }
}

fn preparation_check(
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    limits: crate::source_bibliographic::BibliographicLimits,
    parent: &crate::VerifiedKnowledgeModel<'_>,
) -> Result<()> {
    if validator
        .cancelled
        .load(std::sync::atomic::Ordering::Relaxed)
        || std::time::Instant::now() >= limits.deadline
    {
        return Err(Error::Budget(
            "managed successor preparation cancelled/deadline",
        ));
    }
    parent.check_pin()
}

fn managed_catalog_filename(entry: &Value, entities: &Value) -> Result<String> {
    let kind = crate::source_bibliographic_render::text(entry, "record_type")?;
    let source = crate::source_bibliographic_render::text(entry, "source_record_ref")?;
    let filename = match kind {
        "agent" => "agents.jsonl",
        "place" => "places.jsonl",
        "organization" => "organizations.jsonl",
        "work" => "works.jsonl",
        "expression" => "expressions.jsonl",
        "edition" => "editions.jsonl",
        "collection" => "collections.jsonl",
        "item" => "items.jsonl",
        "link" => "links.jsonl",
        "artifact" if source.ends_with("/artifact-witness.json") => "artifacts.jsonl",
        "composite" if source.ends_with("/composite-witness.json") => "composites.jsonl",
        _ => {
            let profiles = entities["types"]
                .as_array()
                .ok_or(Error::Invalid("managed entity registry types"))?
                .iter()
                .filter(|item| item["source_record_profile"]["record_type"] == kind)
                .collect::<Vec<_>>();
            if profiles.len() != 1 {
                return Err(Error::ManagedSourceUnsupported(
                    "managed retained catalogue family has no unique owner profile; FullOnly required",
                ));
            }
            let filename = crate::source_bibliographic_render::text(
                &profiles[0]["source_record_profile"],
                "catalog_filename",
            )?;
            if filename.contains('/') || !filename.ends_with(".jsonl") {
                return Err(Error::Invalid("managed catalogue family filename"));
            }
            return Ok(filename.to_owned());
        }
    };
    Ok(filename.to_owned())
}

fn contains_exact_string(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value == expected,
        Value::Array(values) => values
            .iter()
            .any(|value| contains_exact_string(value, expected)),
        Value::Object(values) => values
            .values()
            .any(|value| contains_exact_string(value, expected)),
        _ => false,
    }
}

#[derive(Clone)]
struct ManagedCatalogFact {
    source_ref: String,
    source_record_ref: String,
    old_line: u64,
    line: u64,
    old_sha256: String,
    sha256: String,
}

fn refresh_managed_catalog_provenance(
    catalog: &mut Value,
    id: &str,
    fact: &ManagedCatalogFact,
) -> Result<()> {
    if catalog["current_record_ref"]["id"].as_str() != Some(id)
        || catalog["source_ref"].as_str() != Some(fact.source_ref.as_str())
        || catalog["source_record_ref"].as_str() != Some(fact.source_record_ref.as_str())
        || catalog["line"].as_u64() != Some(fact.old_line)
        || catalog["sha256"].as_str() != Some(fact.old_sha256.as_str())
    {
        return Err(Error::Invalid(
            "managed retained catalogue provenance changed",
        ));
    }
    catalog["line"] = Value::from(fact.line);
    catalog["sha256"] = Value::String(fact.sha256.clone());
    Ok(())
}

/// One genuine original navigation member from the pinned immutable base.
/// The existing original-member PK and native-ID index bound the seek; the
/// canonical/raw/semantic seals still own exact source return.
pub(crate) fn parent_navigation_member(
    parent: &crate::VerifiedKnowledgeModel<'_>,
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    collection: &str,
    id: &str,
    l: crate::source_bibliographic::BibliographicLimits,
    limits: StageLimits,
    work: &mut u64,
    visited_rows: &mut u64,
) -> Result<Option<Value>> {
    limits.validate()?;
    l.validate()?;
    preparation_check(validator, l, parent)?;
    let original = managed_navigation_original_member_bytes(
        parent,
        collection,
        id,
        NavigationOriginalLimits {
            max_rows: limits.max_seek_rows as u64,
            max_row_bytes: limits.sqlite.max_row_bytes,
            max_total_bytes: limits.sqlite.max_work_bytes.saturating_sub(*work),
        },
    )?;
    preparation_check(validator, l, parent)?;
    let Some(original) = original else {
        return Ok(None);
    };
    charge(
        work,
        original.work_bytes as usize,
        limits.sqlite.max_work_bytes,
    )?;
    *visited_rows = visited_rows
        .checked_add(original.visited_rows)
        .ok_or(Error::Budget("managed base candidate rows"))?;
    Ok(Some(
        crate::knowledge_normalization::SourceRow::parse(
            &original.raw,
            limits.sqlite.max_row_bytes,
        )?
        .value()
        .clone(),
    ))
}

/// Exact original bytes recovered from authenticated membership and either
/// the V2 direct carrier or the older normalized carrier, plus bounded work.
/// This is immutable custody only; the caller retains current owner authority.
pub struct ManagedNavigationOriginalCarrier {
    pub raw: Vec<u8>,
    pub visited_rows: u64,
    pub work_bytes: u64,
}

/// Read one original member through the existing managed model format. The
/// caller retains its SQL cancellation/VM hook. Placeholder carriers cannot
/// substitute for the raw, canonical and semantic seals in the original index.
pub fn managed_navigation_original_member_bytes(
    parent: &crate::VerifiedKnowledgeModel<'_>,
    collection: &str,
    id: &str,
    limits: NavigationOriginalLimits,
) -> Result<Option<ManagedNavigationOriginalCarrier>> {
    use crate::source_bibliographic_render::encode;
    limits.validate()?;
    parent.check_pin()?;
    if !tos_foundation::is_managed_knowledge_model_abi(&parent.selection().model_abi) {
        return Err(Error::ManagedSourceUnsupported(
            "managed original member model ABI",
        ));
    }
    let mut work = 0u64;
    let mut visited_rows = 0u64;
    let Some(member) = parent.navigation_original_member_under_caller_budget(
        collection,
        id,
        limits.max_total_bytes.min(64 * 1024 * 1024),
    )?
    else {
        return Ok(None);
    };
    if parent.selection().model_abi == tos_foundation::KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2 {
        let carrier = crate::knowledge_navigation_original::exact_carrier(
            parent.connection(),
            &member,
            limits,
        )?;
        parent.check_pin()?;
        return Ok(Some(carrier));
    }
    let receipt = parent.navigation_original_receipt()?;
    let (table, id_field) = match collection {
        "nodes" => ("knowledge_nodes", "node_id"),
        "edges" => ("knowledge_relations", "edge_id"),
        _ => return Err(Error::Invalid("managed base member collection")),
    };
    let sql = format!(
        "SELECT payload_len,CASE WHEN typeof(payload)='blob' AND payload_len=length(payload) AND payload_len<=?3 THEN payload ELSE NULL END,payload_sha256 FROM {table} WHERE native_id=?1 AND source_graph=?2 ORDER BY source_order LIMIT ?4"
    );
    let mut stmt = parent.connection().prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params![
        id,
        receipt.source_graph,
        limits.max_row_bytes as i64,
        limits
            .max_rows
            .checked_add(1)
            .ok_or(Error::Budget("managed base seek row overflow"))? as i64
    ])?;
    let mut selected = None;
    let mut visits = 0usize;
    while let Some(row) = rows.next()? {
        parent.check_pin()?;
        visits += 1;
        visited_rows = visited_rows
            .checked_add(1)
            .ok_or(Error::Budget("managed base candidate rows"))?;
        if visits > limits.max_rows as usize {
            return Err(Error::Budget("managed base identity candidates"));
        }
        let len: i64 = row.get(0)?;
        let raw: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(1)?
            .ok_or(Error::Budget("managed base normalized row bound"))?;
        if len <= 0 || raw.len() as i64 != len {
            return Err(Error::Invalid("managed base row length"));
        }
        let expected: Vec<u8> = row.get(2)?;
        if expected.as_slice() != Digest256::of_bytes(&raw).as_bytes() {
            return Err(Error::Invalid("managed base normalized payload digest"));
        }
        charge(&mut work, raw.len(), limits.max_total_bytes)?;
        let carrier = crate::knowledge_normalization::SourceRow::parse(&raw, limits.max_row_bytes)?;
        let payload = &carrier.value()["source_record"]["payload"];
        let canonical = encode(payload, limits.max_row_bytes)?;
        charge(&mut work, canonical.len(), limits.max_total_bytes)?;
        if Digest256::of_bytes(&canonical).to_hex() != member.canonical_original_sha256 {
            continue;
        }
        let parsed =
            crate::knowledge_normalization::SourceRow::parse(&canonical, limits.max_row_bytes)?;
        if payload[id_field] != member.id
            || parsed.stable_digest()? != member.semantic_sha256
            || carrier.value()["source_record"]["digest"] != member.semantic_sha256
            || canonical.len() as u64 != member.raw_bytes
            || Digest256::of_bytes(&canonical).to_hex() != member.raw_sha256
        {
            return Err(Error::ManagedSourceUnsupported(
                "managed base exact original closure unavailable",
            ));
        }
        if selected.replace(canonical).is_some() {
            return Err(Error::Invalid("managed base duplicate admitted carrier"));
        }
    }
    parent.check_pin()?;
    selected
        .map(|raw| {
            Some(ManagedNavigationOriginalCarrier {
                raw,
                visited_rows,
                work_bytes: work,
            })
        })
        .ok_or(Error::ManagedSourceUnsupported(
            "managed base original carrier unavailable; FullOnly required",
        ))
}

/// Reconstruct only cold-verified original membership, never all normalized
/// carriers or placeholder keys. The initial producer emits canonical original
/// bytes; a parent with only semantic equality cannot enter this update path.
fn parent_navigation(
    parent: &crate::VerifiedKnowledgeModel<'_>,
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    l: crate::source_bibliographic::BibliographicLimits,
    limits: StageLimits,
    work: &mut u64,
) -> Result<Value> {
    let receipt = parent.navigation_original_receipt()?;
    let mut object = None;
    let mut rights = Vec::new();
    let mut after = None;
    let mut packets = 0u64;
    loop {
        preparation_check(validator, l, parent)?;
        let page = parent.navigation_original_page_under_caller_budget(
            after,
            limits.max_seek_rows,
            limits.sqlite.max_row_bytes,
            limits.max_seek_bytes,
        )?;
        for (ordinal, raw) in page.rows {
            charge(work, raw.len(), limits.sqlite.max_work_bytes)?;
            let value = crate::knowledge_normalization::SourceRow::parse(
                &raw,
                limits.sqlite.max_row_bytes,
            )?
            .value()
            .clone();
            if ordinal == -1 && object.is_none() {
                if Digest256::of_bytes(&raw).to_hex() != receipt.header_sha256 {
                    return Err(Error::Invalid("managed parent original header SHA"));
                }
                object = Some(value);
            } else if ordinal == rights.len() as i64 {
                rights.push(value);
            } else {
                return Err(Error::Invalid("managed parent original packet order"));
            }
            packets = packets
                .checked_add(1)
                .ok_or(Error::Budget("managed parent original packet count"))?;
        }
        after = page.next_ordinal;
        if after.is_none() {
            break;
        }
    }
    if packets
        != receipt
            .rights
            .checked_add(1)
            .ok_or(Error::Budget("managed parent rights count"))?
    {
        return Err(Error::Invalid("managed parent original packet EOF"));
    }
    let mut header = object.ok_or(Error::Invalid("managed parent original header absent"))?;
    if !header.is_object() {
        return Err(Error::Invalid("managed parent header object"));
    }
    if header.get("nodes").is_some()
        || header.get("edges").is_some()
        || header.get("rights").is_some()
    {
        return Err(Error::Invalid("managed parent detached original header"));
    }
    for (collection, id_field, table, expected, expected_root) in [
        (
            "nodes",
            "node_id",
            "knowledge_nodes",
            receipt.nodes,
            &receipt.node_input_root_sha256,
        ),
        (
            "edges",
            "edge_id",
            "knowledge_relations",
            receipt.edges,
            &receipt.edge_input_root_sha256,
        ),
    ] {
        let mut values = Vec::new();
        let mut cursor = None;
        let mut root = Digest256Hasher::new();
        let sql = format!(
            "SELECT payload_len,payload FROM {table} WHERE native_id=?1 AND source_graph=?2 ORDER BY source_order"
        );
        let mut statement = parent.connection().prepare(&sql)?;
        loop {
            preparation_check(validator, l, parent)?;
            let page = parent.navigation_original_members_under_caller_budget(
                collection,
                cursor.as_deref(),
                limits.max_seek_rows,
                limits.max_seek_bytes,
            )?;
            for member in page.rows {
                let mut rows =
                    statement.query(rusqlite::params![member.id, receipt.source_graph])?;
                let mut selected = None;
                let mut visited = 0usize;
                while let Some(row) = rows.next()? {
                    preparation_check(validator, l, parent)?;
                    visited += 1;
                    if visited > limits.max_seek_rows {
                        return Err(Error::Budget("managed parent original identity candidates"));
                    }
                    let len: u64 = row.get(0)?;
                    if len > limits.sqlite.max_row_bytes as u64 {
                        return Err(Error::Budget("managed parent normalized row"));
                    }
                    let raw: Vec<u8> = row.get(1)?;
                    if raw.len() as u64 != len {
                        return Err(Error::Invalid("managed parent normalized row bytes"));
                    }
                    charge(work, raw.len(), limits.sqlite.max_work_bytes)?;
                    let carrier = crate::knowledge_normalization::SourceRow::parse(
                        &raw,
                        limits.sqlite.max_row_bytes,
                    )?;
                    let payload = &carrier.value()["source_record"]["payload"];
                    let canonical = crate::source_bibliographic_render::encode(
                        payload,
                        limits.sqlite.max_row_bytes,
                    )?;
                    charge(work, canonical.len(), limits.sqlite.max_work_bytes)?;
                    if Digest256::of_bytes(&canonical).to_hex() != member.canonical_original_sha256
                    {
                        continue;
                    }
                    let parsed = crate::knowledge_normalization::SourceRow::parse(
                        &canonical,
                        limits.sqlite.max_row_bytes,
                    )?;
                    if payload[id_field] != member.id
                        || parsed.stable_digest()? != member.semantic_sha256
                        || carrier.value()["source_record"]["digest"] != member.semantic_sha256
                        || canonical.len() as u64 != member.raw_bytes
                        || Digest256::of_bytes(&canonical).to_hex() != member.raw_sha256
                    {
                        return Err(Error::ManagedSourceUnsupported(
                            "managed parent canonical original byte closure unavailable",
                        ));
                    }
                    if selected.is_some() {
                        return Err(Error::Invalid(
                            "managed parent duplicate admitted original carrier",
                        ));
                    }
                    selected = Some(payload.clone());
                }
                let value = selected.ok_or(Error::ManagedSourceUnsupported(
                    "managed parent original carrier unavailable; FullOnly required",
                ))?;
                root.update(&(member.id.len() as u64).to_be_bytes());
                root.update(member.id.as_bytes());
                root.update(
                    Digest256::from_hex(&member.raw_sha256)
                        .map_err(|_| Error::Invalid("managed parent original SHA"))?
                        .as_bytes(),
                );
                values.push(value);
                if values.len() as u64 > expected || values.len() as u64 > limits.sqlite.max_rows {
                    return Err(Error::Budget("managed parent original rows"));
                }
            }
            cursor = page.next_id;
            if cursor.is_none() {
                break;
            }
        }
        if values.len() as u64 != expected || root.finalize().to_hex() != *expected_root {
            return Err(Error::Invalid(
                "managed parent complete original count/root/EOF",
            ));
        }
        header[collection] = Value::Array(values);
    }
    header["rights"] = Value::Array(rights);
    preparation_check(validator, l, parent)?;
    Ok(header)
}

/// Complete selected successor for the proved managed Agent creation scope.
/// CMD reconstructs and verifies the committed delta before calling this
/// function; the old immutable model is construction input, not a current-head
/// read grant. Missing original/global dependency closure refuses before output.
/// The callback streams the ACTUAL retained catalogue and returns its owner
/// inventory projection root after complete metadata coverage/EOF.
#[allow(clippy::too_many_arguments)]
pub fn prepare_managed_agent_selected_successor<P: ManagedProducerProof>(
    old: &CompletedManagedAgentProducer<P>,
    parent: &crate::VerifiedKnowledgeModel<'_>,
    proof: P,
    catalogue: impl FnOnce(&mut dyn FnMut(&Value) -> Result<()>) -> Result<String>,
    record_path: &str,
    record_raw: &[u8],
    forms_raw: Option<&[u8]>,
    forms: &mut dyn crate::source_bibliographic::BibliographicForms,
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    schema_revision: SourceRevision,
    l: crate::source_bibliographic::BibliographicLimits,
    candidate: &Path,
    binding: SourceBinding,
    stage_limits: StageLimits,
    owner: &dyn StageOwner,
    isolation: &dyn StageIsolation,
    entity_raw: &[u8],
    relation_raw: &[u8],
    descriptor_raw: &[u8],
    supported_profiles: &[&str],
    native_limits: NativeProducerLimits,
    original_limits: NavigationOriginalLimits,
    full_limits: FullKnowledgeLimits,
    saved_lenses: &[Value],
    owner_receipt_id: String,
    build_header: impl FnOnce(&mut KnowledgeStage<'_>, &KnowledgeRegistry) -> Result<Value>,
) -> Result<CompletedManagedAgentProducer<P>> {
    use crate::source_bibliographic_render::{encode, text};
    use tos_validation::source_cut::CutSchemaExecutor;
    proof.validate()?;
    l.validate()?;
    original_limits.validate()?;
    let delta = proof
        .delta()
        .ok_or(Error::Invalid("managed successor committed delta absent"))?;
    let previous = &old.source;
    if parent.source_basis() != &(previous.basis())
        || encode(
            &serde_json::to_value(parent.selection()).map_err(|e| Error::Source(e.to_string()))?,
            full_limits.seal.max_header_bytes,
        )? != encode(
            &serde_json::to_value(&old.expectation).map_err(|e| Error::Source(e.to_string()))?,
            full_limits.seal.max_header_bytes,
        )?
        || delta.parent_model_sha256 != old.expectation.model_sha256
        || delta.parent_model_size_bytes != old.expectation.model_size_bytes
        || delta.parent_source_proof_sha256 != previous.root_sha256()?
        || delta.parent_through_commit_seq != previous.through_commit_seq()
        || !previous.same_immutable_context(&proof)
        || previous.initial_export() != proof.initial_export()
        || Digest256::of_bytes(entity_raw).to_hex() != old.expectation.entity_registry_sha256
        || Digest256::of_bytes(relation_raw).to_hex() != old.expectation.relation_registry_sha256
        || Digest256::of_bytes(descriptor_raw).to_hex() != old.expectation.descriptor_sha256
        || validator.worker.sha256.to_hex() != old.schema_worker_sha256
    {
        return Err(Error::Invalid(
            "managed successor immutable parent/software/current chain",
        ));
    }
    let schemas = validator.schemas(schema_revision)?;
    let schema_set_sha256 = schemas.execution_binding().schema_set_sha256.to_hex();
    drop(schemas);
    if schema_set_sha256 != old.schema_set_sha256 {
        return Err(Error::ManagedSourceUnsupported(
            "managed successor schema closure changed; FullOnly required",
        ));
    }
    let vocabulary = QueryVocabulary::parse(descriptor_raw, supported_profiles)?;
    if vocabulary.sources.len() != 1
        || vocabulary.sources[0].adapter_profile != "source-navigation-node-edge-v1"
    {
        return Err(Error::ManagedSourceUnsupported(
            "managed successor full source family closure unavailable",
        ));
    }
    let parent = parent.fork_reader_with_vm_budget(stage_limits.sqlite.max_sql_vm_steps)?;
    let mut work = 0;
    let mut projection = parent_navigation(&parent, validator, l, stage_limits, &mut work)?;
    let catalog_len: u64 = parent.connection().query_row(
        "SELECT packet_len FROM catalog_index_meta WHERE descriptor_sha256=?1",
        [&old.expectation.descriptor_sha256],
        |row| row.get(0),
    )?;
    if catalog_len > full_limits.catalog.max_catalog_bytes as u64 {
        return Err(Error::Budget("managed parent catalogue packet"));
    }
    charge(
        &mut work,
        usize::try_from(catalog_len)
            .map_err(|_| Error::Budget("managed parent catalogue length"))?,
        stage_limits.sqlite.max_work_bytes,
    )?;
    let catalog_raw: Vec<u8> = parent.connection().query_row(
        "SELECT packet FROM catalog_index_meta WHERE descriptor_sha256=?1",
        [&old.expectation.descriptor_sha256],
        |row| row.get(0),
    )?;
    if catalog_raw.len() as u64 != catalog_len
        || Digest256::of_bytes(&catalog_raw).to_hex() != old.expectation.catalog_packet_sha256
    {
        return Err(Error::Invalid("managed immutable parent catalogue changed"));
    }
    drop(
        tos_foundation::parse_json(
            &catalog_raw,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits::new(
                full_limits.catalog.max_catalog_bytes,
                96,
                1_000_000,
                4096,
            )
            .map_err(|_| Error::Budget("managed parent catalogue JSON limits"))?,
        )
        .map_err(|e| Error::Source(e.to_string()))?,
    );
    let catalog: Value = serde_json::from_slice(&catalog_raw)
        .map_err(|_| Error::Invalid("managed parent catalogue JSON"))?;
    let retained = catalog["lenses"]
        .as_array()
        .ok_or(Error::Invalid("managed parent saved-lens array"))?;
    if retained.len() != saved_lenses.len() {
        return Err(Error::ManagedSourceUnsupported(
            "managed successor saved-lens input changed; FullOnly required",
        ));
    }
    for (previous, current) in retained.iter().zip(saved_lenses) {
        let previous = encode(previous, full_limits.catalog.max_catalog_bytes)?;
        let current = encode(current, full_limits.catalog.max_catalog_bytes)?;
        charge(
            &mut work,
            previous
                .len()
                .checked_add(current.len())
                .ok_or(Error::Budget("managed saved-lens preparation bytes"))?,
            stage_limits.sqlite.max_work_bytes,
        )?;
        if previous != current {
            return Err(Error::ManagedSourceUnsupported(
                "managed successor saved-lens input changed; FullOnly required",
            ));
        }
    }
    drop(catalog);
    drop(catalog_raw);
    let header_len: u64 = parent.connection().query_row(
        "SELECT packet_len FROM graph_header WHERE singleton=1",
        [],
        |row| row.get(0),
    )?;
    if header_len > full_limits.seal.max_header_bytes as u64 {
        return Err(Error::Budget("managed parent graph header"));
    }
    charge(
        &mut work,
        usize::try_from(header_len).map_err(|_| Error::Budget("managed parent header length"))?,
        stage_limits.sqlite.max_work_bytes,
    )?;
    let header_raw: Vec<u8> = parent.connection().query_row(
        "SELECT packet FROM graph_header WHERE singleton=1",
        [],
        |row| row.get(0),
    )?;
    if header_raw.len() as u64 != header_len
        || Digest256::of_bytes(&header_raw).to_hex() != old.seal.graph_header_sha256
    {
        return Err(Error::Invalid(
            "managed immutable parent graph header changed",
        ));
    }
    let parent_header = crate::knowledge_normalization::SourceRow::parse(
        &header_raw,
        full_limits.seal.max_header_bytes,
    )?
    .value()
    .clone();

    if record_raw.len() > l.catalog.max_row_bytes {
        return Err(Error::Budget("managed addressed Agent source bytes"));
    }
    charge(
        &mut work,
        record_raw.len(),
        stage_limits.sqlite.max_work_bytes,
    )?;
    let record =
        crate::knowledge_normalization::SourceRow::parse(record_raw, l.catalog.max_row_bytes)?
            .value()
            .clone();
    let new_id = text(&record, "record_id")?;
    if record["record_version"] != 1 || record["record_type"] != "agent" {
        return Err(Error::Invalid("managed addressed initial Agent source"));
    }
    let entities = crate::knowledge_normalization::SourceRow::parse(
        entity_raw,
        full_limits.max_registry_bytes,
    )?
    .value()
    .clone();
    let mut entries = std::collections::BTreeMap::<String, Value>::new();
    let mut families = std::collections::BTreeMap::<String, String>::new();
    let mut family_types = std::collections::BTreeMap::<String, String>::new();
    let mut family_lines = std::collections::BTreeMap::<String, u64>::new();
    let mut family_hashers = std::collections::BTreeMap::<String, Digest256Hasher>::new();
    let mut lines = std::collections::BTreeMap::<String, u64>::new();
    let mut old_agent_lines = std::collections::BTreeMap::<String, u64>::new();
    let mut old_agent_line = 0u64;
    let mut old_agent_hash = Digest256Hasher::new();
    let mut catalogue_hash = Digest256Hasher::new();
    let mut last = String::new();
    let observed = catalogue(&mut |entry| {
        preparation_check(validator, l, &parent)?;
        let id = text(entry, "record_id")?;
        let kind = text(entry, "record_type")?;
        let family = managed_catalog_filename(entry, &entities)?;
        if id <= last.as_str()
            || entry["schema_version"] != "tos_source_witness_catalog_entry_v1"
            || entries.len() as u64 >= l.catalog.max_rows
            || entry
                .get("source_record_ref")
                .and_then(Value::as_str)
                .is_none()
        {
            return Err(Error::Invalid(
                "managed complete ordered mixed catalogue scope",
            ));
        }
        if family_types
            .get(&family)
            .is_some_and(|existing| existing != kind)
        {
            return Err(Error::ManagedSourceUnsupported(
                "managed catalogue filename combines record families; FullOnly required",
            ));
        }
        family_types.insert(family.clone(), kind.to_owned());
        let raw = encode(entry, l.catalog.max_output_row_bytes)?;
        charge(
            &mut work,
            raw.len()
                .checked_add(1)
                .ok_or(Error::Budget("managed catalogue line bytes"))?,
            stage_limits.sqlite.max_work_bytes,
        )?;
        catalogue_hash.update(&raw);
        catalogue_hash.update(b"\n");
        let line = family_lines
            .get(&family)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(Error::Budget("managed family catalogue line"))?;
        family_lines.insert(family.clone(), line);
        let hasher = family_hashers
            .entry(family.clone())
            .or_insert_with(Digest256Hasher::new);
        hasher.update(&raw);
        hasher.update(b"\n");
        if family == "agents.jsonl" && id != new_id {
            old_agent_line = old_agent_line
                .checked_add(1)
                .ok_or(Error::Budget("managed old Agent catalogue line"))?;
            old_agent_lines.insert(id.to_owned(), old_agent_line);
            old_agent_hash.update(&raw);
            old_agent_hash.update(b"\n");
        }
        last = id.to_owned();
        lines.insert(id.to_owned(), line);
        if entries.contains_key(id) || families.contains_key(id) {
            return Err(Error::Invalid("managed duplicate mixed catalogue identity"));
        }
        entries.insert(id.to_owned(), entry.clone());
        families.insert(id.to_owned(), family);
        Ok(())
    })?;
    if observed != proof.inventory_projection() {
        return Err(Error::Invalid(
            "managed current catalogue owner projection EOF/root",
        ));
    }
    let catalogue_sha = format!("sha256:{}", catalogue_hash.finalize().to_hex());
    let old_agent_sha = format!("sha256:{}", old_agent_hash.finalize().to_hex());
    let family_shas = family_hashers
        .into_iter()
        .map(|(family, hasher)| (family, format!("sha256:{}", hasher.finalize().to_hex())))
        .collect::<std::collections::BTreeMap<_, _>>();
    let new_entry = entries.get(new_id).ok_or(Error::Invalid(
        "managed new Agent absent from complete catalogue",
    ))?;
    if families.get(new_id).map(String::as_str) != Some("agents.jsonl")
        || new_entry["source_record_ref"] != record_path
        || crate::source_witness_catalog::render_catalog_record(
            &record,
            record_path,
            new_entry.get("source_schema_ref").and_then(Value::as_str),
            l.catalog.max_row_bytes,
        )? != *new_entry
    {
        return Err(Error::Invalid(
            "managed addressed initial Agent/catalog correspondence",
        ));
    }
    let mut nodes = std::mem::take(
        projection["nodes"]
            .as_array_mut()
            .ok_or(Error::Invalid("managed parent nodes"))?,
    );
    let mut edges = std::mem::take(
        projection["edges"]
            .as_array_mut()
            .ok_or(Error::Invalid("managed parent edges"))?,
    );
    let mut facts = std::collections::BTreeMap::<String, ManagedCatalogFact>::new();
    for (id, entry) in &entries {
        let family = families
            .get(id)
            .ok_or(Error::Invalid("managed catalogue family index"))?;
        let sha256 = family_shas
            .get(family)
            .ok_or(Error::Invalid("managed catalogue family hash"))?;
        let line = lines[id];
        let (old_line, old_sha256) = match (family.as_str(), id.as_str() == new_id) {
            ("agents.jsonl", false) => (
                *old_agent_lines
                    .get(id)
                    .ok_or(Error::Invalid("managed old Agent catalogue line index"))?,
                old_agent_sha.clone(),
            ),
            _ => (line, sha256.clone()),
        };
        facts.insert(
            id.clone(),
            ManagedCatalogFact {
                source_ref: format!("ToS/source-witnesses/catalog/{family}"),
                source_record_ref: text(entry, "source_record_ref")?.to_owned(),
                old_line,
                line,
                old_sha256,
                sha256: sha256.clone(),
            },
        );
    }
    let mut parent_node_ids = std::collections::BTreeSet::new();
    let mut retained_record_ids = std::collections::BTreeSet::new();
    let mut old_agents = std::collections::BTreeSet::new();
    for node in &mut nodes {
        let id = text(node, "node_id")?.to_owned();
        let kind = text(node, "node_kind")?.to_owned();
        let source_ref = text(node, "source_ref")?.to_owned();
        if !node["label"].is_string()
            || !node["properties"].is_object()
            || !parent_node_ids.insert(id.clone())
            || id == new_id
            || contains_exact_string(node, new_id)
            || contains_exact_string(node, record_path)
        {
            return Err(Error::Invalid(
                "managed parent navigation node shape/dependency",
            ));
        }
        if node["properties"].get("source_record").is_some() {
            let entry = entries.get(&id).ok_or(Error::Invalid(
                "managed parent source record absent from current catalogue",
            ))?;
            let source = &node["properties"]["source_record"];
            if kind != text(entry, "record_type")?
                || source_ref != text(entry, "source_record_ref")?
                || crate::source_witness_catalog::render_catalog_record(
                    source,
                    &source_ref,
                    entry.get("source_schema_ref").and_then(Value::as_str),
                    l.catalog.max_row_bytes,
                )? != *entry
                || !retained_record_ids.insert(id.to_owned())
            {
                return Err(Error::Invalid(
                    "managed retained mixed catalogue/source correspondence",
                ));
            }
            if kind == "agent" {
                old_agents.insert(id.clone());
            }
            let catalog = node
                .pointer_mut("/properties/record_history/provenance/catalog")
                .ok_or(Error::Invalid(
                    "managed retained catalogue provenance absent",
                ))?;
            refresh_managed_catalog_provenance(
                catalog,
                &id,
                facts
                    .get(&id)
                    .ok_or(Error::Invalid("managed retained catalogue fact"))?,
            )?;
        } else if kind == "record-version" {
            let view = node
                .pointer_mut("/properties/record_version_view")
                .ok_or(Error::Invalid("managed record-version view absent"))?;
            if view["schema_version"] != "tos_record_version_view_v1"
                || !matches!(view["record_kind"].as_str(), Some("claim" | "metadata"))
                || id
                    != format!(
                        "record-version:{}",
                        crate::source_bibliographic_render::digest(
                            &view["record_ref"],
                            l.catalog.max_row_bytes,
                        )?
                    )
            {
                return Err(Error::Invalid("managed retained record-version identity"));
            }
            if view["record_kind"] == "metadata" {
                let record_id = view["record_ref"]["id"].as_str().map(str::to_owned);
                let catalog = view
                    .pointer_mut("/provenance/catalog")
                    .ok_or(Error::Invalid(
                        "managed metadata version catalogue provenance absent",
                    ))?;
                let current_id = catalog["current_record_ref"]["id"]
                    .as_str()
                    .ok_or(Error::Invalid("managed metadata version current identity"))?
                    .to_owned();
                if record_id.as_deref() != Some(current_id.as_str()) {
                    return Err(Error::Invalid(
                        "managed metadata version/catalog current identity mismatch",
                    ));
                }
                refresh_managed_catalog_provenance(
                    catalog,
                    &current_id,
                    facts
                        .get(&current_id)
                        .ok_or(Error::Invalid("managed metadata version catalogue target"))?,
                )?;
            }
        } else if matches!(kind.as_str(), "tradition" | "region" | "era" | "branch") {
            if !source_ref.ends_with("/branch.manifest.json")
                || node
                    .pointer("/properties/branch_path")
                    .and_then(Value::as_str)
                    .is_none()
                || node
                    .pointer("/properties/role")
                    .and_then(Value::as_str)
                    .is_none()
            {
                return Err(Error::Invalid("managed retained branch navigation node"));
            }
        } else if kind == "source_planting" {
            if !source_ref.ends_with("/source-planting.json")
                || node.pointer("/properties/discovery_ref").is_none()
                || node.pointer("/properties/research_ref").is_none()
            {
                return Err(Error::Invalid("managed retained source-planting node"));
            }
        } else if kind == "source_witness" {
            if node["identity_status"] != "provisional"
                || node["properties"] != Value::Object(Default::default())
                || entries.contains_key(&id)
            {
                return Err(Error::Invalid("managed retained source-witness node"));
            }
        } else if kind == "file" {
            let sha = text(&node["properties"], "sha256")?;
            let digest = sha.strip_prefix("sha256:").unwrap_or(sha);
            let refs = node["source_refs"]
                .as_array()
                .ok_or(Error::Invalid("managed retained file source refs"))?;
            if id != format!("tos.file.sha256.{sha}")
                || Digest256::from_hex(digest).is_err()
                || refs.is_empty()
                || refs.iter().any(|r| !r.is_string())
                || node["properties"]["byte_size"].as_u64().is_none()
                || node["properties"]["media_type"].as_str().is_none()
            {
                return Err(Error::Invalid("managed retained file navigation node"));
            }
        } else if node["identity_status"] == "source-declared-versioned-record" {
            let record_id = text(&node["properties"], "record_id")?;
            let packet_id = text(&node["properties"], "packet_id")?;
            let version = match node["properties"].get("packet_version") {
                None | Some(Value::Null) => "None".to_owned(),
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => return Err(Error::Invalid("managed retained packet version")),
            };
            let namespace =
                Digest256::of_bytes(format!("{source_ref}:{packet_id}:{version}").as_bytes())
                    .to_hex();
            if id != format!("{record_id}@{}", &namespace[..20])
                || node["properties"]["content_available"].as_bool().is_none()
                || node["properties"]["publication_posture"].as_str().is_none()
                || node["properties"].get("content_posture").is_none()
                || node["properties"].get("review_status").is_none()
            {
                return Err(Error::Invalid("managed retained stand-off packet node"));
            }
        } else {
            return Err(Error::ManagedSourceUnsupported(
                "managed parent navigation node family unsupported; FullOnly required",
            ));
        }
    }
    if retained_record_ids.len()
        != entries
            .len()
            .checked_sub(1)
            .ok_or(Error::Budget("managed mixed catalogue row count"))?
        || retained_record_ids.contains(new_id)
        || old_agents.len()
            != families
                .iter()
                .filter(|(id, family)| id.as_str() != new_id && family.as_str() == "agents.jsonl")
                .count()
    {
        return Err(Error::Invalid(
            "managed retained mixed catalogue node coverage",
        ));
    }
    for id in entries.keys().filter(|id| id.as_str() != new_id) {
        if !retained_record_ids.contains(id) {
            return Err(Error::Invalid("managed current catalogue node missing"));
        }
    }
    let mut edge_ids = std::collections::BTreeSet::new();
    for edge in &edges {
        let id = text(edge, "edge_id")?;
        let from = text(edge, "from_id")?;
        let to = text(edge, "to_id")?;
        let kind = text(edge, "edge_kind")?;
        let refs = edge["source_refs"]
            .as_array()
            .ok_or(Error::Invalid("managed retained relation source refs"))?;
        if !edge_ids.insert(id.to_owned())
            || !parent_node_ids.contains(from)
            || !parent_node_ids.contains(to)
            || !edge["predicate_id"].is_string()
            || !edge["review_status"].is_string()
            || refs.iter().any(|r| !r.is_string())
            || !matches!(
                kind,
                "exact_historical_record_reference"
                    | "authored_branch_hierarchy"
                    | "authored_source_planting"
                    | "evidence_claim"
                    | "authored_item_manifest"
                    | "stand-off-source-structure"
            )
            || contains_exact_string(edge, new_id)
            || contains_exact_string(edge, record_path)
        {
            return Err(Error::ManagedSourceUnsupported(
                "managed parent navigation relation closure unavailable; FullOnly required",
            ));
        }
    }
    // Every retained chain/source/form stays exact. Only catalogue provenance
    // is refreshed; unchanged family hashes/lines must already match the parent.
    for node in &mut nodes {
        charge(
            &mut work,
            encode(node, l.catalog.max_output_row_bytes)?.len(),
            stage_limits.sqlite.max_work_bytes,
        )?;
    }
    let form_path = format!(
        "{}.human-forms.json",
        record_path
            .strip_suffix(".json")
            .ok_or(Error::Invalid("managed Agent basename"))?
    );
    let materialized = {
        let mut schemas = validator.schemas(schema_revision)?;
        let valid = schemas
            .check(
                record_path,
                record_raw,
                "ToS/contracts/corpus-record.schema.json",
                l.deadline,
                validator.cancelled,
            )
            .map_err(|e| Error::Source(format!("managed Agent schema:{e:?}")))?;
        if !valid {
            return Err(Error::Invalid("managed Agent owner schema invalid"));
        }
        if let Some(raw) = forms_raw {
            charge(&mut work, raw.len(), stage_limits.sqlite.max_work_bytes)?;
            if !schemas
                .check(
                    &form_path,
                    raw,
                    "ToS/contracts/human-form-set.schema.json",
                    l.deadline,
                    validator.cancelled,
                )
                .map_err(|e| Error::Source(format!("managed Agent form schema:{e:?}")))?
            {
                return Err(Error::Invalid("managed Agent form owner schema invalid"));
            }
        }
        drop(schemas);
        forms_raw
            .map(|raw| {
                crate::source_bibliographic::materialize_checked_form_set(
                    raw, &form_path, &record, forms, l,
                )
            })
            .transpose()?
            .flatten()
    };
    let new = crate::source_bibliographic_navigation::project_managed_initial_agent(
        new_entry,
        &record,
        record_raw,
        &entities,
        (
            lines[new_id],
            family_shas
                .get("agents.jsonl")
                .ok_or(Error::Invalid("managed current Agent catalogue hash"))?
                .clone(),
        ),
        materialized.as_ref().map(|(r, v)| (r.as_str(), v)),
        l,
    )?;
    if !new.diagnostics.is_empty() {
        return Err(Error::Invalid(
            "managed initial Agent unresolved navigation diagnostics",
        ));
    }
    nodes.extend(new.nodes);
    edges.extend(new.edges);
    nodes.sort_by(|a, b| a["node_id"].as_str().cmp(&b["node_id"].as_str()));
    edges.sort_by(|a, b| a["edge_id"].as_str().cmp(&b["edge_id"].as_str()));
    let mut final_node_ids = std::collections::BTreeSet::new();
    for node in &nodes {
        if !final_node_ids.insert(text(node, "node_id")?.to_owned()) {
            return Err(Error::Invalid(
                "managed successor duplicate navigation node",
            ));
        }
    }
    let mut final_edge_ids = std::collections::BTreeSet::new();
    for edge in &edges {
        if !final_edge_ids.insert(text(edge, "edge_id")?.to_owned())
            || !final_node_ids.contains(text(edge, "from_id")?)
            || !final_node_ids.contains(text(edge, "to_id")?)
        {
            return Err(Error::Invalid(
                "managed successor navigation endpoint closure",
            ));
        }
    }
    projection["counts"]["nodes"] = Value::from(nodes.len());
    projection["counts"]["edges"] = Value::from(edges.len());
    projection["nodes"] = Value::Array(nodes);
    projection["edges"] = Value::Array(edges);
    preparation_check(validator, l, &parent)?;
    validator.finish()?;
    build_selected(
        &projection,
        &old.source_catalog_root,
        old.export_revision,
        old.export_membership,
        validator.worker.sha256.to_hex(),
        schema_set_sha256,
        proof,
        candidate,
        binding,
        stage_limits,
        owner,
        isolation,
        entity_raw,
        relation_raw,
        descriptor_raw,
        supported_profiles,
        native_limits,
        original_limits,
        full_limits,
        saved_lenses,
        owner_receipt_id,
        work,
        Some((catalogue_sha, entries.len() as u64)),
        Some((
            record_path.to_owned(),
            Digest256::of_bytes(record_raw).to_hex(),
            record_raw.len() as u64,
        )),
        forms_raw.map(|raw| {
            (
                form_path,
                Digest256::of_bytes(raw).to_hex(),
                raw.len() as u64,
            )
        }),
        |stage, registry| {
            let header = build_header(stage, registry)?;
            for field in [
                "normalization_binding",
                "query_properties",
                "authority_boundary",
            ] {
                if encode(&header[field], full_limits.seal.max_header_bytes)?
                    != encode(&parent_header[field], full_limits.seal.max_header_bytes)?
                {
                    return Err(Error::ManagedSourceUnsupported(
                        "managed successor software/registry/authority header changed; FullOnly required",
                    ));
                }
            }
            Ok(header)
        },
    )
}
