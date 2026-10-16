//! Candidate-intake adapter uses the same exact pack/proposal machinery.
//! No candidate nodes or canon admission are invented by this relation family.
use crate::knowledge_canon_materialize::CanonNormalizer;
pub use crate::knowledge_canon_materialize::{
    CanonMaterializeLimits as CandidateMaterializeLimits,
    CanonMaterializeReceipt as CandidateMaterializeReceipt,
    materialize_canon_relations as materialize_candidate_relations,
};
use crate::knowledge_canon_prepare::{CANDIDATE_PROFILE, prepare_family};
pub use crate::knowledge_canon_prepare::{
    CanonPrepareLimits as CandidatePrepareLimits, CanonPrepareReceipt as CandidatePrepareReceipt,
};
use crate::knowledge_stage::KnowledgeStage;
use crate::{KnowledgeRegistry, QueryVocabulary, Result};
pub fn prepare_candidate_inputs(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: CandidatePrepareLimits,
) -> Result<CandidatePrepareReceipt> {
    let result = prepare_family(stage, vocabulary, CANDIDATE_PROFILE, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}
pub fn candidate_normalizer<'a>(
    registry: &'a KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    vocabulary: &QueryVocabulary,
    descriptor_bytes: &[u8],
    limits: CandidateMaterializeLimits,
) -> Result<CanonNormalizer<'a>> {
    CanonNormalizer::family(
        registry,
        entity_bytes,
        relation_bytes,
        vocabulary,
        descriptor_bytes,
        limits,
        CANDIDATE_PROFILE,
    )
}
