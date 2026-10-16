//! Text-free records for exact DTA TEI technical structure.
use super::*;
pub(super) fn method(plan_ref: &str, builder: &str, event_id: &str, made_at: &str) -> Value {
    json!({
    "maker_kind": "software",
    "agent_ref": "software:tos-zarathustra-technical-markup-builder",
    "method_name": "exact DTA TEI source-structure observation",
    "method_version": "1",
    "software_refs": [builder],
    "model_ref": null,
    "configuration_ref": plan_ref,
    "locale": "de",
    "unicode_version": null,
    "unicode_revision": null,
    "tailoring_ref": null,
    "provenance_event_ref": event_id,
    "made_at": made_at,
    "output_posture": "method_result_not_source_or_linguistic_truth"
    })
}
pub(super) fn anchor(
    identity: &Value,
    ordinal: usize,
    private_ref: &str,
    content_digest: &str,
    observation: &Unit,
    payload_ref: &str,
    exact: &str,
    role: &str,
) -> Value {
    json!({
    "anchor_ref": identity["anchor_ref"],
    "ordinal": ordinal,
    "text_layer_ref": private_ref,
    "text_layer_sha256": content_digest,
    "selector": {
    "type": "text_position",
    "start": observation.start,
    "end": observation.end,
    "position_unit": "unicode_code_point",
    "interval": "half_open"
    },
    "exact_sha256": sha(exact.as_bytes()),
    "anchor_role": role,
    "source_return": {
    "required": true,
    "locator_ref": format!("{}#{}", payload_ref, observation.locator)
    }
    })
}
pub(super) fn unit(
    identity: &Value,
    observation: &Unit,
    parent_refs: &Value,
    child_refs: &Value,
) -> Value {
    json!({
    "unit_id": identity["unit_id"],
    "unit_version": 1,
    "supersedes_unit_ref": null,
    "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
    "unit_kind": observation.unit_kind,
    "surface_posture": "source_bearing",
    "continuity": "contiguous",
    "ordered_anchor_refs": [identity["anchor_ref"]],
    "parent_unit_refs": parent_refs,
    "ordered_child_unit_refs": child_refs,
    "boundary_posture": "source_attested",
    "certainty": {
    "value": 1.0,
    "meaning": "maker-declared-boundary-confidence-not-truth-probability"
    },
    "status_reason": "The unit records a boundary explicitly present in the exact selected DTA TEI representation. German text acceptance, editorial hierarchy, linguistic analysis, translation and semantics retain their corresponding assessment routes.",
    "source_text_mutated": false,
    "semantic_promotion": false
    })
}
pub(super) fn packet(
    plan: &Value,
    config: &Value,
    part_ids: &Value,
    content_digest: &str,
    private_ref: &str,
    method: &Value,
    anchors: &[Value],
    units: &[Value],
    citation_ref: &str,
    citation_digest: &str,
) -> Value {
    let ordered_unit_refs = units
        .iter()
        .map(|r| r["unit_id"].clone())
        .collect::<Vec<_>>();
    json!({
    "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
    "schema_version": "tos_source_text_unit_packet_v1",
    "packet_id": part_ids["packet_id"],
    "packet_version": 1,
    "supersedes_packet_ref": null,
    "content_posture": "source_bound",
    "source_scope": {
    "work_ref": plan["work_ref"],
    "expression_ref": config["expression_ref"],
    "edition_ref": config["edition_ref"],
    "item_ref": config["item_ref"],
    "file_ref": config["file_ref"],
    "file_sha256": config["file_sha256"]
    },
    "source_layer": {
    "text_layer_ref": private_ref,
    "text_layer_sha256": content_digest,
    "language": "de",
    "media_type": "text/plain; charset=utf-8",
    "unicode_form": "source_preserved",
    "position_unit": "unicode_code_point",
    "interval": "half_open",
    "immutable": true,
    "visibility": "local_only",
    "publication_authorized": false
    },
    "schemes": [{
    "scheme_id": part_ids["scheme_id"],
    "scheme_version": 1,
    "supersedes_scheme_ref": null,
    "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
    "scheme_name": "DTA TEI non-semantic technical source structure v1",
    "analysis_role": "source_structure",
    "boundary_basis": "source_markup",
    "unit_kinds": ["document", "section", "paragraph", "verse_group", "verse_line", "milestone", "other"],
    "method": method,
    "policies": {
    "normalization": "no-text-mutation-separate-successor-layer",
    "punctuation": "included_in_neighbor",
    "whitespace": "included_in_neighbor",
    "line_break": "included_in_neighbor",
    "hyphenation": "preserve_source",
    "unreported_gaps_allowed": false,
    "overlap": "nested_only"
    },
    "authority_limit": {
    "algorithmic_output_is_source_truth": false,
    "algorithmic_output_is_linguistic_truth": false,
    "model_subword_is_lexeme": false,
    "unit_identity_is_semantic_identity": false,
    "text_mutation_allowed": false
    }
    }],
    "anchors": anchors,
    "units": units,
    "segmentations": [{
    "segmentation_id": part_ids["segmentation_id"],
    "segmentation_version": 1,
    "supersedes_segmentation_ref": null,
    "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
    "scheme_ref": part_ids["scheme_id"],
    "ordered_unit_refs": ordered_unit_refs,
    "coverage": {
    "scope_anchor_ref": anchors[0]["anchor_ref"],
    "coverage_posture": "exhaustive_nested",
    "excluded_anchor_refs": [],
    "unreported_gaps_allowed": false,
    "overlap_requires_declaration": true,
    "source_reconstruction_required": true
    },
    "status": "observed_source_structure",
    "status_reason": "The hierarchy reproduces only selected TEI body element boundaries; the separately declared auxiliary Part-IV container subtree is outside the work-level source layer.",
    "maker": method,
    "competing_segmentation_refs": [],
    "review_refs": [],
    "declared_uses": ["navigation", "source_observation", "interchange"],
    "source_text_authority": false,
    "linguistic_authority": false,
    "semantic_authority": false
    }],
    "reviews": [],
    "projections": [{
    "projection_id": part_ids["projection_id"],
    "projection_kind": "other",
    "source_segmentation_refs": [part_ids["segmentation_id"]],
    "artifact_ref": citation_ref,
    "artifact_sha256": citation_digest,
    "admission_posture": "proposal_preserving",
    "preserves_unit_ids": true,
    "preserves_status": true,
    "preserves_source_return": true,
    "runtime_authority": false,
    "source_text_authority": false,
    "linguistic_authority": false,
    "semantic_authority": false,
    "visibility": "local_only"
    }],
    "rights_and_visibility": {
    "source_visibility": "local_only",
    "packet_visibility": "public_metadata_only",
    "effective_visibility": "local_only",
    "rights_record_refs": [config["rights_ref"]],
    "private_source_used": true,
    "publication_authorized": false,
    "inheritance_policy": "most-restrictive-source-packet-and-destination-wins"
    },
    "authority_boundary": {
    "tree_role": "orientation",
    "graph_role": "relation",
    "source_role": "authority",
    "validators_prove_mechanics_not_truth": true,
    "segmentation_is_method_result_not_source_truth": true,
    "source_unit_is_not_lexeme_sign_or_concept": true,
    "model_token_is_not_linguistic_token": true,
    "projection_is_owner_truth": false,
    "legacy_bulk_migration_authorized": false
    }
    })
}
pub(super) fn event(
    plan_ref: &str,
    event_id: &str,
    made_at: &str,
    part_count: usize,
    builder_digest: &str,
    inputs: &[Value],
    outputs: &[Value],
    output_refs: &[String],
    rights_ref: &Value,
) -> Value {
    json!({
    "schema_version": "tos_provenance_event_v1",
    "event_id": event_id,
    "event_type": "segmentation",
    "started_at": made_at,
    "ended_at": made_at,
    "agent_refs": ["software:tos-zarathustra-technical-markup-builder", "model:codex"],
    "inputs": inputs,
    "outputs": outputs,
    "method": {
    "maker_type": "software",
    "name": "exact-dta-tei-non-semantic-technical-structure",
    "version": "1",
    "artifact_digest": builder_digest,
    "runtime": null,
    "device": "cpu",
    "configuration": {
    "parts": part_count,
    "source_text_tracked": false,
    "opaque_identity_issuance_reused": true,
    "source_markup_status": "observed_source_structure",
    "human_review_status": "unreviewed",
    "semantic_fields_materialized": false,
    "russian_parallel_structure_materialized": false
    },
    "prompt_or_instruction_ref": plan_ref
    },
    "status": "completed_with_warnings",
    "warnings": ["TEI source markup is observed but not accepted as a final editorial hierarchy.", "The sequential German layers remain ignored local-only mode-0600 files.", "The Part-IV auxiliary sequence is excluded only from this work-level projection and remains intact in its source container.", "This event records German technical markup. Russian structure, translation, linguistic and semantic assessment, graph, canon, rights and publication follow their corresponding owner routes."],
    "receipt_refs": output_refs,
    "rights_basis_ref": rights_ref,
    "event_version": 1,
    "supersedes_event_ref": null
    })
}
