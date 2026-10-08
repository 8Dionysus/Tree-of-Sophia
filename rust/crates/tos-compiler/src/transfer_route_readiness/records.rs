//! Source-owned text-free route projection shapes.
use super::*;
pub(super) fn route(
    target: &Value,
    source: &Value,
    route_id: &str,
    target_status: &str,
    readiness_class: &str,
    intersection: bool,
) -> Value {
    json!({
    "route_readiness_id": route_id,
    "target_passage_candidate_id": target["passage_candidate_id"],
    "source_passage_candidate_id": source["source_passage_candidate_id"],
    "frozen_page_candidate_id": target["frozen_page_candidate_id"],
    "work_ref": target["work_ref"],
    "qualified_unit_key": target["qualified_unit_key"],
    "source_candidate_status": source["status"],
    "source_boundary_layer": source["boundary_layer"],
    "target_candidate_status": target_status,
    "target_boundary_layer": target["source_layer"],
    "readiness_class": readiness_class,
    "source_candidate_available": true,
    "target_candidate_available": true,
    "frozen_page_intersection": intersection,
    "accepted_source_or_target_text": false,
    "source_to_target_passage_alignment": false,
    "eligible_for_variant_execution": false,
    "target_gold": false,
    "human_review_performed": false
    })
}
pub(super) fn page(page_id: &Value) -> Value {
    json!({
    "frozen_page_candidate_id": page_id,
    "route_readiness_refs": [],
    "dual_intersecting_route_count": 0,
    "dual_target_nonintersecting_route_count": 0,
    "status": "has-dual-private-intersecting-route"
    })
}
pub(super) fn projection(
    target_binding: &Value,
    source_binding: &Value,
    routes: Vec<Value>,
    frozen_pages: Vec<Value>,
    intersecting: usize,
    nonintersecting: usize,
    event_id: &str,
) -> Value {
    json!({
    "$schema": SCHEMA_URI,
    "schema_version": "tos_transfer_route_readiness_projection_v1",
    "projection_id": PROJECTION_ID,
    "target_candidate_set": target_binding,
    "source_candidate_set": source_binding,
    "routes": routes,
    "frozen_pages": frozen_pages,
    "summary": {
    "conservative_route_count": 35,
    "source_candidate_available_route_count": 35,
    "target_candidate_available_route_count": 35,
    "dual_candidate_available_route_count": 35,
    "dual_candidate_frozen_page_intersection_count": intersecting,
    "dual_candidate_target_nonintersection_count": nonintersecting,
    "frozen_page_count": 20,
    "frozen_pages_with_dual_intersecting_route_count": 20,
    "source_to_target_alignment_count": 0,
    "eligible_target_unit_count": 0,
    "target_gold_count": 0,
    "human_review_count": 0
    },
    "effects": {
    "readiness_projection_created": true,
    "tracked_source_or_target_text_created": false,
    "source_to_target_passage_alignment_created": false,
    "translation_alignment_created": false,
    "target_unit_eligible": false,
    "target_gold_created": false,
    "semantic_work_opened": false,
    "human_work_scheduled": false,
    "publication_authorized": false,
    "canon_effect": false
    },
    "provenance_event_ref": event_id,
    "status": "mechanically-prepared-ineligible",
    "authority_boundary": "This text-free projection records co-availability of independently materialized source and target candidates, using their shared structural route and frozen-page intersections.",
    "does_not_establish": ["accepted_german", "accepted_russian", "diplomatic_transcription", "source_to_target_passage_alignment", "translation_correspondence", "translation_equivalence", "translation_quality", "eligible_target_unit", "target_gold", "human_review", "semantic_relation", "publication_permission", "canon_promotion"]
    })
}
pub(super) fn event(
    event_inputs: &[Value],
    rendered: &[u8],
    summary: &Value,
    output_ref: &str,
    event_id: &str,
    event_at: &str,
    builder_digest: &str,
) -> Value {
    let mut event = json!({
    "schema_version": "tos_provenance_event_v1",
    "event_id": event_id,
    "event_type": "export",
    "started_at": event_at,
    "ended_at": event_at,
    "agent_refs": ["model:codex", "software:python-standard-library"],
    "inputs": event_inputs,
    "outputs": [{
    "ref": output_ref,
    "role": "tracked-text-free-transfer-route-readiness-projection",
    "sha256": sha(rendered)
    }],
    "method": {
    "maker_type": "software",
    "name": "candidate-identity-and-mechanical-status-coavailability",
    "version": "1",
    "artifact_digest": builder_digest,
    "runtime": "Python standard library",
    "device": "abyss-machine",
    "configuration": summary,
    "prompt_or_instruction_ref": "ToS/research-packets/foundation-laboratory-2026-07/GOLDEN_KERNEL_TRANSFER_REPORT.md"
    },
    "status": "completed_with_warnings",
    "warnings": ["Co-availability records independently materialized source and target candidates; passage alignment retains its recorded status.", "Frozen-page intersection records overlap between independently proposed candidates; target-text assessment and target-gold status remain as recorded.", "no private source or target content was read or copied", "This event records candidate co-availability. Eligibility, human work, semantic assessment, publication and canon retain their existing states."],
    "receipt_refs": [output_ref],
    "rights_basis_ref": null,
    "event_version": 1,
    "supersedes_event_ref": null
    });
    event["agent_refs"] = json!(["software:tos-native-transfer-route-readiness"]);
    event["method"]["runtime"] = json!(format!(
        "Tree-of-Sophia native/Rust {}",
        env!("CARGO_PKG_VERSION")
    ));
    event["method"]["device"] = Value::Null;
    event
}
