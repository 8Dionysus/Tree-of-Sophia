//! Bind a cold-verified CMP knowledge model to its exact authored vocabulary.
//! This establishes query semantics, not current policy or disclosure rights.

use tos_compiler::{QueryVocabulary, VerifiedKnowledgeModel};
use tos_foundation::Digest256;

use crate::search_v2::{
    QueryVocabularyBinding, SEARCH_READ_MODEL_ABI_V1, SEARCH_UNICODE_PROFILE,
    SearchSelectionBinding, SearchV2Error, SearchV2ErrorCode, SelectedQueryVocabulary,
};

fn stale(message: &'static str) -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::StaleSelection,
        message,
    }
}

fn digest(raw: &str) -> Result<Digest256, SearchV2Error> {
    Digest256::from_hex(raw).map_err(|_| stale("selected knowledge digest is invalid"))
}

/// The exact typed selection/descriptor pairing after CMP's cold admission.
/// This carries no source pin, current-policy decision or public lease.
pub struct BoundCmpKnowledge<'a> {
    selection: SearchSelectionBinding,
    vocabulary: &'a QueryVocabulary,
    source_revision: String,
    authority_boundary: String,
    owner_receipt_id: String,
}

impl BoundCmpKnowledge<'_> {
    pub fn selection(&self) -> &SearchSelectionBinding {
        &self.selection
    }
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    pub fn authority_boundary(&self) -> &str {
        &self.authority_boundary
    }
    pub fn owner_receipt_id(&self) -> &str {
        &self.owner_receipt_id
    }

    /// Refuse a different pinned selected file before running a query with
    /// this semantic binding. The source owner separately checks its pin and
    /// current policy through the full disclosure lifetime.
    pub fn check_model(&self, model: &VerifiedKnowledgeModel<'_>) -> Result<(), SearchV2Error> {
        model
            .check_pin()
            .map_err(|_| stale("selected knowledge pin changed"))?;
        if digest(&model.selection().model_sha256)? != self.selection.index_root_sha256
            || model.source_revision() != self.source_revision
        {
            return Err(stale("selected knowledge model differs from query binding"));
        }
        Ok(())
    }
}

impl SelectedQueryVocabulary for BoundCmpKnowledge<'_> {
    fn binding(&self) -> &QueryVocabularyBinding {
        &self.selection.vocabulary
    }
    fn registered_source_ids(&self) -> &[String] {
        self.vocabulary.registered_source_ids()
    }
}

/// CMP must already have verified the immutable selected SQLite bytes,
/// component roots, source scopes and owner expectation at cold admission.
/// This second seam confirms the exact owner-authored descriptor bytes still
/// produce the supplied vocabulary and are the ones named by that selection,
/// including every zero-row registered source. `QueryVocabulary` has public
/// fields, so its cached SHA alone cannot authenticate its current semantics.
pub fn bind_verified_knowledge<'a>(
    model: &VerifiedKnowledgeModel<'_>,
    vocabulary: &'a QueryVocabulary,
    authored_descriptor: &[u8],
) -> Result<BoundCmpKnowledge<'a>, SearchV2Error> {
    vocabulary
        .verify_authored_bytes(authored_descriptor)
        .map_err(|_| stale("authored query vocabulary bytes differ"))?;
    model
        .check_pin()
        .map_err(|_| stale("selected knowledge pin changed"))?;
    let selected = model.selection();
    if !selected.complete
        || selected.model_abi != SEARCH_READ_MODEL_ABI_V1
        || selected.semantic_primitive_profile != SEARCH_UNICODE_PROFILE
        || selected.semantic_primitive_profile != vocabulary.semantic_primitive_profile
        || selected.descriptor_sha256 != vocabulary.descriptor_sha256
        || selected.descriptor_version != vocabulary.descriptor_version
        || selected.entity_registry_id != vocabulary.entity_registry_id
        || selected.relation_registry_id != vocabulary.relation_registry_id
        || selected.source_scopes.len() != vocabulary.registered_source_ids().len()
    {
        return Err(stale(
            "selected knowledge and authored query vocabulary differ",
        ));
    }
    let mut authored: Vec<_> = vocabulary.sources.iter().collect();
    authored.sort_by(|left, right| left.source_graph_id.cmp(&right.source_graph_id));
    for ((source, expected_id), scope) in authored
        .iter()
        .zip(vocabulary.registered_source_ids())
        .zip(&selected.source_scopes)
    {
        if source.source_graph_id.as_str() != expected_id.as_str()
            || scope.source_graph.as_str() != expected_id.as_str()
            || scope.input_role != source.input_role
            || scope.adapter_profile != source.adapter_profile
        {
            return Err(stale(
                "selected source scope differs from authored registration",
            ));
        }
    }
    let selection = SearchSelectionBinding {
        model_abi: selected.model_abi.clone(),
        vocabulary: QueryVocabularyBinding {
            descriptor_sha256: digest(&selected.descriptor_sha256)?,
            descriptor_version: selected.descriptor_version,
        },
        semantic_primitive_profile: selected.semantic_primitive_profile.clone(),
        source_cut: selected.source_cut.clone(),
        through_commit_seq: selected.through_commit_seq,
        source_membership_root: digest(&selected.membership_root)?,
        history_root_sha256: None, // no history-root field in CMP selected v1
        entity_registry_id: selected.entity_registry_id.clone(),
        entity_registry_version: selected.entity_registry_version.clone(),
        entity_registry_sha256: digest(&selected.entity_registry_sha256)?,
        relation_registry_id: selected.relation_registry_id.clone(),
        relation_registry_version: selected.relation_registry_version.clone(),
        relation_registry_sha256: digest(&selected.relation_registry_sha256)?,
        graph_root_sha256: digest(&selected.graph_root_sha256)?,
        catalog_packet_sha256: digest(&selected.catalog_packet_sha256)?,
        catalog_index_root_sha256: digest(&selected.catalog_index_root_sha256)?,
        source_scope_root_sha256: digest(&selected.source_scope_root_sha256)?,
        search_index_root_sha256: digest(&selected.search_index_root_sha256)?,
        index_root_sha256: digest(&selected.model_sha256)?,
        index_generation: selected.index_generation.clone(),
        route_map_version: selected.route_map_version.clone(),
        reader_abi: selected.reader_abi.clone(),
        complete: selected.complete,
    };
    Ok(BoundCmpKnowledge {
        selection,
        vocabulary,
        source_revision: model.source_revision().to_owned(),
        authority_boundary: selected.authority_boundary.clone(),
        owner_receipt_id: selected.owner_receipt_id.clone(),
    })
}
