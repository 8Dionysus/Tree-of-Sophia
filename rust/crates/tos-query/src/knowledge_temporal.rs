//! Selected exact-ID temporal adapter; shares inspect's current held carrier
//! disclosure and caller-selected read/transfer admission.
use crate::{
    knowledge_binding::BoundCmpKnowledge,
    knowledge_inspect::{
        DisclosableInspect, InspectBudget, InspectCurrentAuthority, execute_selected_carrier_packet,
    },
    search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode},
    temporal_comparison::{TEMPORAL_INTENDED_USE, TEMPORAL_OPERATION, compare_temporal_operands},
};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::JsonValue;
pub fn execute_selected_temporal<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &JsonValue,
    budget: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    bound.require_source_revision()?;
    let claim_source = bound
        .source_for_adapter("reified-bibliographic-claims-v1")
        .ok_or(SearchV2Error {
            code: SearchV2ErrorCode::Unavailable,
            message: "selected source Claim adapter unavailable",
        })?;
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        TEMPORAL_OPERATION,
        TEMPORAL_INTENDED_USE,
        budget,
        |read| {
            compare_temporal_operands(
                bound.require_source_revision()?,
                request,
                claim_source,
                |id| read.items(SearchKind::Nodes, "id", id, 1, false),
                budget.json,
            )
        },
    )
}
