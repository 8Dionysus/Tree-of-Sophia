//! Selected disclosure scope shared by knowledge operations.

use crate::{
    knowledge_binding::BoundCmpKnowledge,
    search::{CurrentPolicyBinding, SearchError, SearchErrorCode},
};
use tos_foundation::Digest256;

pub(crate) const INDEXED_SEARCH_CARRIER_LAYER: &str = "tos_knowledge_public_graph_projection_v1";

fn error(code: SearchErrorCode, message: &'static str) -> SearchError {
    SearchError { code, message }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedDisclosureScope {
    pub operation_id: String,
    pub carrier_layer: String,
    pub intended_use: String,
    pub selected_model_receipt_id: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub source_membership_root: Digest256,
    pub descriptor_sha256: Digest256,
    pub selected_index_sha256: Digest256,
    pub policy_issuer_ref: String,
    pub policy_receipt_id: String,
    pub policy_scope: String,
    pub policy_epoch: String,
    pub withdrawal_generation: String,
}

impl IndexedDisclosureScope {
    pub(crate) fn validate_for(
        &self,
        bound: &BoundCmpKnowledge<'_>,
        policy: &CurrentPolicyBinding,
        operation_id: &str,
        intended_use: &str,
    ) -> Result<(), SearchError> {
        if self.operation_id != operation_id
            || self.carrier_layer != INDEXED_SEARCH_CARRIER_LAYER
            || self.intended_use != intended_use
            || self.selected_model_receipt_id != bound.owner_receipt_id()
            || self.source_cut != bound.selection().source_cut
            || self.through_commit_seq != bound.selection().through_commit_seq
            || self.source_membership_root != bound.selection().source_membership_root
            || self.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256
            || self.selected_index_sha256 != bound.selection().index_root_sha256
            || self.policy_scope != policy.scope
            || self.policy_issuer_ref != policy.issuer_ref
            || self.policy_receipt_id != policy.authorization_receipt_id
            || self.policy_epoch != policy.policy_epoch
            || self.withdrawal_generation != policy.withdrawal_generation
            || policy.scope.is_empty()
            || policy.issuer_ref.is_empty()
            || policy.authorization_receipt_id.is_empty()
            || policy.policy_epoch.is_empty()
        {
            return Err(error(
                SearchErrorCode::PolicyBindingUnavailable,
                "indexed public-projection disclosure scope unavailable",
            ));
        }
        Ok(())
    }
}
