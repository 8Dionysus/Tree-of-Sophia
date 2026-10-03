//! Bind a cold-verified CMP knowledge model to its exact authored vocabulary.
//! This establishes query semantics, not current policy or disclosure rights.

use tos_compiler::{
    KNOWLEDGE_CORPUS_MODEL_ABI, KNOWLEDGE_MANAGED_MODEL_ABI, KNOWLEDGE_MODEL_ABI,
    KNOWLEDGE_NAVIGATION_MODEL_ABI, KNOWLEDGE_PHILOSOPHY_MODEL_ABI, KnowledgeSourceBasis,
    ManagedSourceProofV1, QueryVocabulary, VerifiedKnowledgeModel,
};
use tos_foundation::Digest256;

use crate::search_v2::{
    QUERY_PRIMITIVE_PROFILE, QueryVocabularyBinding, SEARCH_UNICODE_PROFILE,
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
    source_basis: KnowledgeSourceBasis,
    authority_boundary: String,
    owner_receipt_id: String,
    descriptor: tos_foundation::JsonValue,
}

impl BoundCmpKnowledge<'_> {
    pub(crate) fn vocabulary(&self) -> &QueryVocabulary {
        self.vocabulary
    }
    pub(crate) fn descriptor(&self) -> &tos_foundation::JsonValue {
        &self.descriptor
    }
    pub fn selection(&self) -> &SearchSelectionBinding {
        &self.selection
    }
    pub fn source_basis(&self) -> &KnowledgeSourceBasis {
        &self.source_basis
    }
    pub fn source_revision(&self) -> Option<&str> {
        self.source_basis.source_revision()
    }
    pub fn require_source_revision(&self) -> Result<&str, SearchV2Error> {
        self.source_revision().ok_or(SearchV2Error {
            code: SearchV2ErrorCode::UnsupportedModel,
            message: "query requires a cut source revision",
        })
    }
    pub fn authority_boundary(&self) -> &str {
        &self.authority_boundary
    }
    pub fn owner_receipt_id(&self) -> &str {
        &self.owner_receipt_id
    }
    pub(crate) fn validate_catalog_identity(
        &self,
        packet: &tos_foundation::JsonValue,
        limits: tos_foundation::JsonLimits,
    ) -> Result<(), SearchV2Error> {
        use tos_foundation::JsonValue;
        let invalid = || SearchV2Error {
            code: SearchV2ErrorCode::CorruptSelectedCarrier,
            message: "selected catalog identity differs",
        };
        match &self.source_basis {
            KnowledgeSourceBasis::V1Cut { source_revision } => {
                if packet.object_get("schema").and_then(JsonValue::as_str)
                    != Some("tos_knowledge_catalog_v1")
                    || packet
                        .object_get("source_revision")
                        .and_then(JsonValue::as_str)
                        != Some(source_revision.as_str())
                    || packet.object_get("source_basis").is_some()
                {
                    return Err(invalid());
                }
            }
            KnowledgeSourceBasis::ManagedCurrent { .. }
            | KnowledgeSourceBasis::ManagedCurrentV2 { .. } => {
                if packet.object_get("schema").and_then(JsonValue::as_str)
                    != Some(tos_compiler::managed_source::MANAGED_CATALOG_SCHEMA)
                    || packet.object_get("source_revision").is_some()
                {
                    return Err(invalid());
                }
                self.validate_managed_basis(
                    packet.object_get("source_basis").ok_or_else(invalid)?,
                    limits,
                )?;
            }
        }
        Ok(())
    }
    /// Compare the retained tagged basis with the one owner proof already
    /// bound to the cold model. Descriptive equality is never a read grant.
    pub(crate) fn validate_managed_basis(
        &self,
        basis: &tos_foundation::JsonValue,
        limits: tos_foundation::JsonLimits,
    ) -> Result<String, SearchV2Error> {
        use tos_foundation::{CanonicalProfile, JsonValue, canonical_bytes_v1};
        let invalid = || SearchV2Error {
            code: SearchV2ErrorCode::CorruptSelectedCarrier,
            message: "selected managed source basis differs",
        };
        let (kind, expected) = match &self.source_basis {
            KnowledgeSourceBasis::ManagedCurrent { proof } => {
                ("managed_current", proof.root_sha256())
            }
            KnowledgeSourceBasis::ManagedCurrentV2 { proof } => {
                ("managed_current_v2", proof.root_sha256())
            }
            _ => return Err(invalid()),
        };
        let expected = expected.map_err(|_| invalid())?;
        if basis.as_object().map(|fields| fields.len()) != Some(2)
            || basis.object_get("kind").and_then(JsonValue::as_str) != Some(kind)
        {
            return Err(invalid());
        }
        let actual = canonical_bytes_v1(
            basis.object_get("proof").ok_or_else(invalid)?,
            CanonicalProfile::SourceRecordDigestV1,
            limits,
        )
        .map_err(|reason| SearchV2Error {
            code: if reason.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                SearchV2ErrorCode::BudgetExceeded
            } else {
                SearchV2ErrorCode::CorruptSelectedCarrier
            },
            message: "selected managed source basis cannot be checked",
        })?;
        if Digest256::of_bytes(&actual) != digest(&expected)? {
            return Err(invalid());
        }
        Ok(expected)
    }

    pub(crate) fn source_for_adapter(&self, adapter: &str) -> Option<&str> {
        let mut sources = self
            .vocabulary
            .sources
            .iter()
            .filter(|source| source.adapter_profile == adapter);
        let source = sources.next()?;
        if sources.next().is_some() {
            return None;
        }
        Some(&source.source_graph_id)
    }

    /// Refuse a different pinned selected file before running a query with
    /// this semantic binding. The source owner separately checks its pin and
    /// current policy through the full disclosure lifetime.
    pub fn check_model(&self, model: &VerifiedKnowledgeModel<'_>) -> Result<(), SearchV2Error> {
        model
            .check_pin()
            .map_err(|_| stale("selected knowledge pin changed"))?;
        if digest(&model.selection().model_sha256)? != self.selection.index_root_sha256
            || model.source_basis() != &self.source_basis
            || model.search_index_profile() != self.selection.search_unicode_profile
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
    if !matches!(model.source_basis(), KnowledgeSourceBasis::V1Cut { .. }) {
        return Err(stale("managed source requires its current owner binding"));
    }
    bind_knowledge(model, vocabulary, authored_descriptor)
}

/// Bind a managed selected model only inside its command owner's held current
/// read. The callback compares the proof to the private parent; every actual
/// read still requires the matching current authority and disclosure lease.
pub fn bind_managed_verified_knowledge<'a>(
    model: &VerifiedKnowledgeModel<'_>,
    vocabulary: &'a QueryVocabulary,
    authored_descriptor: &[u8],
    authorize_current: impl FnOnce(&ManagedSourceProofV1) -> Result<(), SearchV2Error>,
) -> Result<BoundCmpKnowledge<'a>, SearchV2Error> {
    let bound = bind_knowledge(model, vocabulary, authored_descriptor)?;
    let proof = bound
        .source_basis
        .managed_source()
        .ok_or_else(|| stale("managed binding requires a managed selected source"))?;
    authorize_current(proof)?;
    bound.check_model(model)?;
    Ok(bound)
}

/// Exact typed managed proof, accepted only under the caller's existing held source authority.
pub fn bind_managed_proof_verified_knowledge<'a, P: tos_compiler::ManagedProducerProof>(
    model: &VerifiedKnowledgeModel<'_>,
    vocabulary: &'a QueryVocabulary,
    authored_descriptor: &[u8],
    proof: &P,
    authorize_current: impl FnOnce(&P) -> Result<(), SearchV2Error>,
) -> Result<BoundCmpKnowledge<'a>, SearchV2Error> {
    let bound = bind_knowledge(model, vocabulary, authored_descriptor)?;
    if bound.source_basis != proof.basis() {
        return Err(stale("managed binding typed source proof differs"));
    }
    proof
        .validate()
        .map_err(|_| stale("managed binding typed source proof invalid"))?;
    authorize_current(proof)?;
    bound.check_model(model)?;
    Ok(bound)
}

fn bind_knowledge<'a>(
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
    model
        .source_basis()
        .validate()
        .map_err(|_| stale("selected knowledge source basis is invalid"))?;
    if matches!(
        model.source_basis(),
        KnowledgeSourceBasis::ManagedCurrent { .. } | KnowledgeSourceBasis::ManagedCurrentV2 { .. }
    ) != (selected.model_abi == KNOWLEDGE_MANAGED_MODEL_ABI)
    {
        return Err(stale(
            "selected knowledge source basis and model ABI differ",
        ));
    }
    if !selected.complete
        || (selected.model_abi != KNOWLEDGE_MODEL_ABI
            && selected.model_abi != KNOWLEDGE_NAVIGATION_MODEL_ABI
            && selected.model_abi != KNOWLEDGE_PHILOSOPHY_MODEL_ABI
            && selected.model_abi != KNOWLEDGE_CORPUS_MODEL_ABI
            && selected.model_abi != KNOWLEDGE_MANAGED_MODEL_ABI)
        || selected.semantic_primitive_profile != QUERY_PRIMITIVE_PROFILE
        || selected.semantic_primitive_profile != vocabulary.semantic_primitive_profile
        || model.search_index_profile() != SEARCH_UNICODE_PROFILE
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
        search_unicode_profile: model.search_index_profile().into(),
        source_cut: selected.source_cut.clone(),
        through_commit_seq: selected.through_commit_seq,
        source_membership_root: digest(&selected.membership_root)?,
        history_root_sha256: None, // selected envelope has no independent history-root field; managed proof is retained separately
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
        source_basis: model.source_basis().clone(),
        authority_boundary: selected.authority_boundary.clone(),
        owner_receipt_id: selected.owner_receipt_id.clone(),
        descriptor: tos_foundation::parse_json(
            authored_descriptor,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits::default(),
        )
        .map_err(|_| stale("authored query vocabulary JSON invalid"))?
        .into_root(),
    })
}
