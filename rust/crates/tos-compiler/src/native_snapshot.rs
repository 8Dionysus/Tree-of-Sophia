//! Full immutable runtime-data snapshot production. This produces derived
//! selection facts; it neither admits authored source nor issues a live grant.
use crate::{
    Error, ExpectedSourceScope, FullKnowledgeLimits, KnowledgeSelectedExpectation,
    NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeFamilyInputs, NativeProducerLimits, QueryVocabulary,
    Result, SourceBinding,
    d1_public_capture::{PublicCapture, PublicCaptureLimits},
    d1_public_graph::{
        PublicRepositoryRoot, PublicStageOwner, captured_input_roots, ingest_family_rows,
        prepare_family_rows,
    },
    d1_public_header::build_native_snapshot_header,
    d1_public_lens_specs::saved_lenses,
    d1_public_semantics::{validate_native_snapshot_semantics, validate_public_current_registries},
    knowledge_source_navigation_prepare::NavigationHeaderClaim,
    knowledge_stage::{ExactInputReceipt, KnowledgeStage, StageIsolation, StageLimits, WritePhase},
};
use std::path::{Path, PathBuf};
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

/// Issued only after the complete captured families, components and final
/// source check succeed. No constructor accepts an arbitrary candidate receipt.
pub struct CompletedNativeSnapshot {
    path: PathBuf,
    stage: crate::knowledge_stage::StageReceipt,
    full: crate::FullKnowledgeReceipt,
    expectation: KnowledgeSelectedExpectation,
    producer: crate::knowledge_native::NativeProducerReceipt,
    declaration_sha256: Digest256,
    source_revision: String,
    descriptor: Vec<u8>,
    entity: Vec<u8>,
    relation: Vec<u8>,
}
impl CompletedNativeSnapshot {
    pub fn artifact_path(&self) -> &Path {
        &self.path
    }
    pub fn stage(&self) -> &crate::knowledge_stage::StageReceipt {
        &self.stage
    }
    pub fn components(&self) -> &crate::FullKnowledgeReceipt {
        &self.full
    }
    pub fn expectation(&self) -> &KnowledgeSelectedExpectation {
        &self.expectation
    }
    pub fn producer(&self) -> &crate::knowledge_native::NativeProducerReceipt {
        &self.producer
    }
    pub fn declaration_sha256(&self) -> Digest256 {
        self.declaration_sha256
    }
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    /// After the host copies the finished model once to its fresh filesystem
    /// destination, seal that exact copy and serialize independently produced
    /// roots. No fields are inferred by opening the candidate SQLite model.
    pub fn selection_for_copied_model(
        &self,
        copied_model: &Path,
        paths: crate::NativeSelectionPaths,
        cold: crate::ColdOpenLimits,
        process: crate::NativeProcessLimits,
        max_bytes: usize,
    ) -> Result<crate::NativeKnowledgeSelection> {
        let measurement = crate::prepare_native_knowledge_artifact(copied_model, &self.stage)?;
        crate::NativeKnowledgeSelection::from_producer(
            paths,
            crate::NativeSelectionProducer {
                stage: self.stage.clone(),
                seal: self.full.seal.clone(),
                navigation_original: self.producer.navigation_original.clone(),
                philosophy_original: self.producer.philosophy_original.clone(),
                corpus_original: self.producer.corpus_original.clone(),
                managed_source: None,
                managed_source_v2: None,
            },
            self.expectation.clone(),
            measurement,
            cold,
            process,
            &self.descriptor,
            &self.entity,
            &self.relation,
            NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
            max_bytes,
        )
    }

    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }
    pub fn entity_registry(&self) -> &[u8] {
        &self.entity
    }
    pub fn relation_registry(&self) -> &[u8] {
        &self.relation
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeSnapshotLimits {
    pub capture: PublicCaptureLimits,
    pub stage: StageLimits,
    pub native: NativeProducerLimits,
    pub full: FullKnowledgeLimits,
    pub originals: crate::CorpusOriginalSourceLimits,
    pub max_transfer_work_bytes: u64,
    pub max_declaration_bytes: usize,
}
fn input(capture: &PublicCapture, path: &str) -> Result<Vec<u8>> {
    capture
        .read_input(path, 4 * 1024 * 1024)?
        .ok_or(Error::Invalid("native snapshot required captured input"))
}

/// Reuse the actual maintained capture and full component pipeline. The host
/// retains its kernel-backed StageIsolation for the capture, stage and spill
/// lifetime; caller retains the exact declaration bytes through disclosure.
/// The declaration is the software-owned runtime-data allowlist, never a
/// source-owner grant. Candidate must be a fresh private path.
pub fn build_native_snapshot_from_capture(
    capture: &PublicCapture,
    candidate: &Path,
    declaration_raw: &[u8],
    isolation: &dyn StageIsolation,
    limits: NativeSnapshotLimits,
) -> Result<CompletedNativeSnapshot> {
    if limits.max_declaration_bytes == 0
        || limits.max_declaration_bytes > 1024 * 1024
        || declaration_raw.is_empty()
        || declaration_raw.len() > limits.max_declaration_bytes
        || limits.max_transfer_work_bytes == 0
    {
        return Err(Error::Budget("native snapshot declaration/transfer limits"));
    }
    let declaration = parse_json(
        declaration_raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(limits.max_declaration_bytes, 64, 65536, 4096)
            .map_err(|_| Error::Budget("native snapshot declaration JSON"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if declaration
        .root()
        .object_get("schema_version")
        .and_then(|v| v.as_str())
        != Some("tos_access_runtime_data_allowlist_v1")
    {
        return Err(Error::Invalid("native snapshot runtime declaration"));
    }
    capture.check_custody()?;
    let declaration_sha256 = Digest256::of_bytes(declaration_raw);
    let source_revision = if capture.partitioned() {
        capture.partitioned_source_revision()?
    } else {
        capture.legacy_source_revision()?
    };
    let entity = input(
        capture,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    )?;
    let relation = input(
        capture,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    )?;
    let descriptor = input(
        capture,
        "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
    )?;
    let registry = validate_public_current_registries(capture, &entity, &relation)?;
    let vocabulary = QueryVocabulary::parse(&descriptor, NATIVE_KNOWLEDGE_ADAPTER_PROFILES)?;
    prepare_family_rows(capture, limits.capture)?;
    let (collections, membership_root, projection_root) =
        captured_input_roots(capture, &vocabulary)?;
    // The chosen original corpus/phi connector selects the existing V5 ABI.
    // Original receipts remain distinct from normalized projections.
    let abi = crate::KNOWLEDGE_CORPUS_MODEL_ABI;
    let binding = SourceBinding {
        owner_profile: "tos-native-projection-snapshot-v1".into(),
        source_cut: format!("native-projection:{source_revision}"),
        through_commit_seq: 0,
        membership_root,
        index_generation: format!("{abi}:{}", declaration_sha256.to_hex()),
        route_map_version: "tos-access-runtime-data-v1".into(),
        reader_abi: abi.into(),
        projection_root_sha256: projection_root.to_hex(),
        complete: true,
    };
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let originals = crate::native_snapshot_originals::prepare(
        capture,
        &binding,
        &vocabulary,
        limits.originals,
        capture.deadline(),
        &cancelled,
    )?;
    if originals.expected_model_abi() != abi {
        return Err(Error::Invalid("native snapshot original component ABI"));
    }
    let receipt = ExactInputReceipt {
        binding: binding.clone(),
        collections,
    };
    let owner = PublicStageOwner {
        capture,
        receipt: receipt.clone(),
    };
    let mut stage = KnowledgeStage::create_captured_native_snapshot(
        candidate,
        limits.stage,
        receipt,
        &owner,
        isolation,
        capture.vm_counter(),
        capture.work_counter(),
        capture.max_work_bytes(),
        capture.deadline(),
    )?;
    ingest_family_rows(&mut stage, capture, limits.max_transfer_work_bytes)?;
    let nav_raw =
        serde_json::to_vec(&capture.header_object("corpus", "source_navigation", 1024 * 1024)?)
            .map_err(|e| Error::Source(e.to_string()))?;
    let nav = NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&nav_raw).to_hex(),
        raw_json: nav_raw,
    };
    let repository =
        PublicRepositoryRoot::captured_native_projection(capture, &stage, &source_revision)?;
    let borrowed_originals = originals.borrowed();
    let mut families = borrowed_originals.family_inputs(limits.native);
    families.repository_root = Some(repository.input());
    let producer = crate::materialize_native_sources_with_inputs(
        &mut stage,
        &registry,
        &entity,
        &relation,
        &vocabulary,
        &descriptor,
        &nav,
        limits.native,
        families,
    )?;
    let semantics =
        validate_native_snapshot_semantics(&mut stage, capture, &registry, &entity, &relation)?;
    let (processor, configuration) =
        crate::d1_public_build::processor_binding(&repository, &descriptor, &entity, &relation)?;
    let header = build_native_snapshot_header(
        &mut stage,
        capture,
        &registry,
        &entity,
        &source_revision,
        processor,
        configuration,
        &semantics,
    )?;
    let full = crate::compile_full_knowledge_components(
        &mut stage,
        &header,
        &registry,
        &entity,
        &relation,
        &saved_lenses(capture)?,
        &vocabulary,
        &descriptor,
        limits.full,
    )?;
    if full.seal.model_abi != abi || full.seal.managed_source_root_sha256.is_some() {
        return Err(Error::Invalid("native snapshot actual component ABI"));
    }
    let source_scopes = stage.with_connection(WritePhase::Finalize, |db| {
        let mut statement = db.prepare("SELECT source_graph,input_role,adapter_profile,expected_node_count,expected_relation_count,lower(hex(node_root_sha256)),lower(hex(relation_root_sha256)) FROM source_scope ORDER BY source_graph")?;
        let result = statement.query_map([], |r| Ok(ExpectedSourceScope { source_graph:r.get(0)?, input_role:r.get(1)?, adapter_profile:r.get(2)?, node_count:r.get(3)?, relation_count:r.get(4)?, node_root_sha256:r.get(5)?, relation_root_sha256:r.get(6)? }))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(result)
    })?;
    let output = stage.finish()?;
    capture.verify_inputs(limits.capture)?;
    let expectation = KnowledgeSelectedExpectation {
        model_sha256: output.sqlite_sha256.clone(),
        model_size_bytes: output.sqlite_size_bytes,
        owner_receipt_id: format!(
            "native-snapshot:{}:{}",
            source_revision,
            declaration_sha256.to_hex()
        ),
        model_abi: full.seal.model_abi.clone(),
        managed_source_root_sha256: None,
        descriptor_sha256: vocabulary.descriptor_sha256.clone(),
        descriptor_version: vocabulary.descriptor_version,
        semantic_primitive_profile: vocabulary.semantic_primitive_profile.clone(),
        source_cut: output.source_cut.clone(),
        through_commit_seq: binding.through_commit_seq,
        membership_root: output.membership_root.clone(),
        entity_registry_id: registry.entity_registry_id.clone(),
        entity_registry_version: registry.entity_registry_version.to_string(),
        entity_registry_sha256: registry.entity_sha256.clone(),
        relation_registry_id: registry.relation_registry_id.clone(),
        relation_registry_version: registry.relation_registry_version.to_string(),
        relation_registry_sha256: registry.relation_sha256.clone(),
        graph_root_sha256: full.seal.graph_root_sha256.clone(),
        navigation_original_root_sha256: full.seal.navigation_original_root_sha256.clone(),
        philosophy_original_root_sha256: full.seal.philosophy_original_root_sha256.clone(),
        corpus_original_root_sha256: full.seal.corpus_original_root_sha256.clone(),
        catalog_packet_sha256: full.catalog.catalog_packet_sha256.clone(),
        catalog_index_root_sha256: full.catalog.catalog_index_root_sha256.clone(),
        source_scope_root_sha256: full.source_scope.source_scope_root_sha256.clone(),
        search_index_root_sha256: full.search.search_index_root_sha256.clone(),
        node_count: output.node_rows,
        relation_count: output.relation_rows,
        index_generation: binding.index_generation,
        route_map_version: binding.route_map_version,
        reader_abi: binding.reader_abi,
        authority_boundary: serde_json::to_string(&header["authority_boundary"])
            .map_err(|e| Error::Source(e.to_string()))?,
        source_scopes,
        complete: true,
    };
    Ok(CompletedNativeSnapshot {
        path: candidate.to_owned(),
        stage: output,
        full,
        expectation,
        producer,
        declaration_sha256,
        source_revision,
        descriptor,
        entity,
        relation,
    })
}
