//! Initial full managed Agent catalog/navigation selection. Current authority
//! and immutable installed custody remain with the actual command owner.
use crate::{
    Error, ExpectedSourceScope, FullKnowledgeLimits, KnowledgeRegistry, KnowledgeSealReceipt,
    KnowledgeSelectedExpectation, KnowledgeSourceBasis, ManagedSourceProofV1, NativeFamilyInputs,
    NativeProducerLimits, NavigationHeaderClaim, NavigationOriginalInput, NavigationOriginalLimits,
    NavigationOriginalReceipt, QueryVocabulary, Result, SourceBinding,
    knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage, StageIsolation,
        StageLimits, StageOwner, StageReceipt, WritePhase,
    },
    source_bibliographic_source::SourceCatalogInputPlan,
    source_navigation_source::NavigationSourceProjection,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tos_foundation::{Digest256, Digest256Hasher, SourceRevision};
use tos_source_store::SourceMembershipV1;

/// Only the actual full invocation below can construct this outcome. Public
/// descriptive receipts and arbitrary cold models cannot be converted into it.
pub struct CompletedManagedAgentProducer {
    path: PathBuf,
    stage: StageReceipt,
    seal: KnowledgeSealReceipt,
    expectation: KnowledgeSelectedExpectation,
    navigation: NavigationOriginalReceipt,
    source: ManagedSourceProofV1,
    export_revision: SourceRevision,
    export_membership: SourceMembershipV1,
    source_catalog_root: String,
    schema_worker_sha256: String,
    schema_set_sha256: String,
    successor_catalogue_input: Option<(String, u64)>,
    created_record_input: Option<(String, String, u64)>,
    created_forms_input: Option<(String, String, u64)>,
}
impl CompletedManagedAgentProducer {
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
    pub fn source_proof(&self) -> &ManagedSourceProofV1 {
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
pub fn prepare_managed_agent_selected_model(
    source_plan: &SourceCatalogInputPlan,
    projection: &NavigationSourceProjection,
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    proof: ManagedSourceProofV1,
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
) -> Result<CompletedManagedAgentProducer> {
    proof.validate()?;
    let export_revision = source_plan.source_revision();
    let export_membership = source_plan.source_membership();
    let original_binding = source_plan.input_receipt().binding;
    if proof.delta.is_some()
        || proof.initial_export_source_revision != export_revision.0.to_hex()
        || proof.initial_export_membership_sha256 != export_membership.digest.to_hex()
        || proof.initial_export_members != export_membership.count
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
fn build_selected(
    projection: &Value,
    source_catalog_root: &str,
    export_revision: SourceRevision,
    export_membership: SourceMembershipV1,
    schema_worker_sha256: String,
    schema_set_sha256: String,
    proof: ManagedSourceProofV1,
    candidate: &Path,
    mut binding: SourceBinding,
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
) -> Result<CompletedManagedAgentProducer> {
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
    binding.source_cut = proof.stage_source_cut()?;
    binding.membership_root = proof.generation.current_membership_sha256.clone();
    binding.through_commit_seq = proof.generation.through_commit_seq;
    let receipt_bytes = crate::source_bibliographic_render::encode(
        &serde_json::to_value(&collections).map_err(|e| Error::Source(e.to_string()))?,
        full_limits.seal.max_header_bytes,
    )?;
    binding.projection_root_sha256 = Digest256::of_bytes(&receipt_bytes).to_hex();
    let mut stage = KnowledgeStage::create(
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
    let native = crate::materialize_native_sources_with_inputs(
        &mut stage,
        &registry,
        entity_raw,
        relation_raw,
        &vocabulary,
        descriptor_raw,
        &claim,
        native_limits,
        additional,
    )?;
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
        serde_json::to_value(KnowledgeSourceBasis::ManagedCurrent {
            proof: proof.clone(),
        })
        .map_err(|e| Error::Source(e.to_string()))?,
    );
    let source_binding = stage.exact_receipt().binding.clone();
    let full = crate::compile_full_knowledge_components(
        &mut stage,
        &header,
        &registry,
        entity_raw,
        relation_raw,
        saved_lenses,
        &vocabulary,
        descriptor_raw,
        full_limits,
    )?;
    let scopes = stage.with_connection(WritePhase::Finalize, |db| {
        let mut rows = db.prepare("SELECT source_graph,input_role,adapter_profile,expected_node_count,expected_relation_count,lower(hex(node_root_sha256)),lower(hex(relation_root_sha256)) FROM source_scope ORDER BY source_graph")?;
        let scopes = rows.query_map([], |r| Ok(ExpectedSourceScope { source_graph:r.get(0)?, input_role:r.get(1)?,
            adapter_profile:r.get(2)?, node_count:r.get(3)?, relation_count:r.get(4)?, node_root_sha256:r.get(5)?, relation_root_sha256:r.get(6)? }))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(scopes)
    })?;
    let output = stage.finish()?;
    let navigation = native
        .navigation_original
        .ok_or(Error::Invalid("managed selected original receipt"))?;
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
        search_index_root_sha256: full.search.search_index_root_sha256,
        node_count: output.node_rows,
        relation_count: output.relation_rows,
        index_generation: source_binding.index_generation,
        route_map_version: source_binding.route_map_version,
        reader_abi: source_binding.reader_abi,
        authority_boundary: serde_json::to_string(&header["authority_boundary"])
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
pub fn prepare_managed_agent_selected_successor(
    old: &CompletedManagedAgentProducer,
    parent: &crate::VerifiedKnowledgeModel<'_>,
    proof: ManagedSourceProofV1,
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
) -> Result<CompletedManagedAgentProducer> {
    use crate::source_bibliographic_render::{encode, text};
    use tos_validation::source_cut::CutSchemaExecutor;
    proof.validate()?;
    l.validate()?;
    original_limits.validate()?;
    let delta = proof
        .delta
        .as_ref()
        .ok_or(Error::Invalid("managed successor committed delta absent"))?;
    let previous = &old.source;
    let a = &previous.generation;
    let b = &proof.generation;
    if parent.source_basis()
        != &(KnowledgeSourceBasis::ManagedCurrent {
            proof: previous.clone(),
        })
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
        || delta.parent_through_commit_seq != a.through_commit_seq
        || a.domain != b.domain
        || a.store_id != b.store_id
        || a.epoch != b.epoch
        || a.definition_sha256 != b.definition_sha256
        || a.database_oid != b.database_oid
        || a.schema_profile_sha256 != b.schema_profile_sha256
        || a.bootstrap_source_revision != b.bootstrap_source_revision
        || a.bootstrap_membership_sha256 != b.bootstrap_membership_sha256
        || a.bootstrap_members != b.bootstrap_members
        || previous.initial_export_source_revision != proof.initial_export_source_revision
        || previous.initial_export_membership_sha256 != proof.initial_export_membership_sha256
        || previous.initial_export_members != proof.initial_export_members
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

    let mut entries = std::collections::BTreeMap::<String, Value>::new();
    let mut last = String::new();
    let mut catalogue_hash = Digest256Hasher::new();
    let observed = catalogue(&mut |entry| {
        preparation_check(validator, l, &parent)?;
        let id = text(entry, "record_id")?;
        if id <= last.as_str()
            || entry["schema_version"] != "tos_source_witness_catalog_entry_v1"
            || entry["record_type"] != "agent"
            || entries.len() as u64 >= l.catalog.max_rows
        {
            return Err(Error::Invalid(
                "managed complete ordered Agent catalogue scope",
            ));
        }
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
        last = id.to_owned();
        entries.insert(id.to_owned(), entry.clone());
        Ok(())
    })?;
    if observed != proof.generation.inventory_projection_sha256 {
        return Err(Error::Invalid(
            "managed current catalogue owner projection EOF/root",
        ));
    }
    let catalogue_sha = format!("sha256:{}", catalogue_hash.finalize().to_hex());
    let lines = entries
        .keys()
        .enumerate()
        .map(|(n, id)| (id.clone(), n as u64 + 1))
        .collect::<std::collections::BTreeMap<_, _>>();
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
    let new_entry = entries.get(new_id).ok_or(Error::Invalid(
        "managed new Agent absent from complete catalogue",
    ))?;
    if new_entry["source_record_ref"] != record_path
        || record["record_version"] != 1
        || record["record_type"] != "agent"
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
    let mut old_agents = std::collections::BTreeSet::new();
    for node in &nodes {
        match node["node_kind"].as_str() {
            Some("agent") => {
                let id = text(node, "node_id")?;
                if id == new_id || !old_agents.insert(id.to_owned()) {
                    return Err(Error::Invalid("managed creation existing/duplicate Agent"));
                }
                let entry = entries.get(id).ok_or(Error::Invalid(
                    "managed parent Agent missing current catalogue",
                ))?;
                let source = &node["properties"]["source_record"];
                if crate::source_witness_catalog::render_catalog_record(
                    source,
                    text(node, "source_ref")?,
                    entry.get("source_schema_ref").and_then(Value::as_str),
                    l.catalog.max_row_bytes,
                )? != *entry
                {
                    return Err(Error::Invalid(
                        "managed unchanged Agent catalogue/source changed",
                    ));
                }
            }
            Some("record-version") => {}
            _ => {
                return Err(Error::ManagedSourceUnsupported(
                    "managed parent external navigation dependency closure unavailable; FullOnly required",
                ));
            }
        }
    }
    if entries.len()
        != old_agents
            .len()
            .checked_add(1)
            .ok_or(Error::Budget("managed catalogue row count"))?
        || edges
            .iter()
            .any(|e| e["edge_kind"] != "exact_historical_record_reference")
    {
        return Err(Error::ManagedSourceUnsupported(
            "managed complete catalogue/global incidence closure unavailable; FullOnly required",
        ));
    }
    // Every retained chain/source/form stays exact. Only current catalogue
    // line/SHA changes, as the maintained Versions provenance requires.
    for node in &mut nodes {
        let provenance = if node["node_kind"] == "agent" {
            &mut node["properties"]["record_history"]["provenance"]
        } else {
            &mut node["properties"]["record_version_view"]["provenance"]
        };
        let catalog = provenance.get_mut("catalog").ok_or(Error::Invalid(
            "managed retained Versions catalogue provenance absent",
        ))?;
        let id = catalog["current_record_ref"]["id"]
            .as_str()
            .ok_or(Error::Invalid("managed retained current version identity"))?
            .to_owned();
        if !old_agents.contains(&id)
            || catalog["source_record_ref"] != entries[&id]["source_record_ref"]
            || catalog["source_ref"] != "ToS/source-witnesses/catalog/agents.jsonl"
        {
            return Err(Error::Invalid(
                "managed retained Agent Versions/catalog closure",
            ));
        }
        catalog["line"] = Value::from(lines[&id]);
        catalog["sha256"] = Value::String(catalogue_sha.clone());
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
    let entities = crate::knowledge_normalization::SourceRow::parse(
        entity_raw,
        full_limits.max_registry_bytes,
    )?
    .value()
    .clone();
    let new = crate::source_bibliographic_navigation::project_managed_initial_agent(
        new_entry,
        &record,
        record_raw,
        &entities,
        (lines[new_id], catalogue_sha.clone()),
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
