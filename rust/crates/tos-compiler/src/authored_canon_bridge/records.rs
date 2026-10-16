//! Source-owned record constructors; measured fields are explicit inputs.
use super::*;
pub(super) fn source_anchor(
    plan: &Value,
    plan_ref: &str,
    plan_digest: &str,
    event_id: &str,
) -> Result<Value> {
    let scope = &plan["scope"];
    let ids = &plan["opaque_ids"];
    Ok(json!({
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json",
        "schema_version": "tos_source_anchor_v2",
        "anchor_id": ids["anchor_id"],
        "passage_id": ids["passage_id"],
        "target": {
            "item_id": scope["item_ref"],
            "file_id": scope["file_ref"],
            "file_sha256": scope["file_sha256"],
            "media_type": "application/xml"
        },
        "selector_payload": {
            "kind": "selector_expression",
            "expression": {
                "mode": "single",
                "selector": {
                    "state": {
                        "state_type": "digest_state",
                        "representation_ref": scope["source_relative_ref"],
                        "representation_sha256": scope["file_sha256"],
                        "media_type": "application/xml",
                        "character_normalization": "none"
                    },
                    "selector": {
                        "type": "structural",
                        "scheme": plan["source_selector"]["scheme"],
                        "value": plan["source_selector"]["value"],
                        "conforms_to": "https://www.w3.org/TR/1999/REC-xpath-19991116/"
                    }
                }
            }
        },
        "publication_boundary": {
            "record_storage": "tracked",
            "source_content_visibility": "local_only",
            "source_text_in_record": false,
            "public_payload_expected": false
        },
        "selector_method": {
            "maker_type": "software",
            "method": "exact-tei-section-selection",
            "version": "1",
            "configuration_ref": plan_ref,
            "configuration_digest": plan_digest
        },
        "resolution_status": "mechanically_resolved",
        "review_status": "unreviewed",
        "provenance_event_ref": event_id,
        "anchor_version": 1,
        "supersedes_anchor_ref": null,
        "review_ref": null
    }))
}
pub(super) fn common_layer(
    plan: &Value,
    plan_ref: &str,
    plan_digest: &str,
    event_id: &str,
    layer_id: &Value,
    layer_role: &str,
    content_ref: &Value,
    content: &[u8],
    anchor_digest: &str,
    rights_digest: &str,
    derivation: Value,
    character_normalization: &str,
    line_break_posture: &str,
    transcription_goal: &str,
    layout_posture: &str,
) -> Result<Value> {
    let scope = &plan["scope"];
    let text = utf8(content)?;
    let content_digest = sha(content);
    Ok(json!({
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json",
        "schema_version": "tos_source_text_layer_v1",
        "layer_id": layer_id,
        "layer_role": layer_role,
        "source_binding": {
            "work_ref": scope["work_ref"],
            "expression_ref": scope["expression_ref"],
            "edition_ref": scope["edition_ref"],
            "item_ref": scope["item_ref"],
            "source_file_ref": scope["file_ref"],
            "source_file_sha256": scope["file_sha256"],
            "anchor_contract": "tos_source_anchor_v2",
            "anchors": [{
                "anchor_id": plan["opaque_ids"]["anchor_id"],
                "anchor_record_ref": plan["outputs"]["anchor_ref"],
                "anchor_record_sha256": anchor_digest
            }]
        },
        "representation": {
            "content_file_id": format!("tos.file.sha256.{}", content_digest),
            "content_ref": content_ref,
            "content_sha256": content_digest,
            "media_type": "text/plain",
            "charset": "UTF-8",
            "language": "de",
            "text_scope": {
                "start": 0,
                "end": text.chars().count(),
                "position_unit": "unicode_code_point",
                "interval": "half_open"
            },
            "character_normalization": character_normalization,
            "line_break_posture": line_break_posture,
            "storage": "ignored_local",
            "content_visibility": "local_only",
            "tracked_content": false,
            "publication_authorized": false,
            "rights_record_refs": [{
                "ref": scope["rights_ref"],
                "sha256": rights_digest
            }],
            "publication_authority_refs": []
        },
        "derivation": derivation,
        "editorial_policy": {
            "policy_ref": plan_ref,
            "policy_sha256": plan_digest,
            "transcription_goal": transcription_goal,
            "historical_language_preserved": true,
            "printing_errors_silently_corrected": false,
            "typography_posture": (if character_normalization == "none" { "preserve" } else { "normalize_declared" }),
            "layout_posture": layout_posture,
            "unicode_normalization": character_normalization,
            "uncertainty_representation": "explicit-never-silent",
            "method_declared": true
        },
        "uncertainty": {
            "status": "none",
            "annotations": []
        },
        "admission": {
            "mechanical_status": "fixity_verified",
            "review_status": "unreviewed",
            "review_ref": null,
            "human_review_performed": false,
            "human_language_competence": "blocked",
            "language_competence_evidence_refs": [],
            "accepted_uses": [],
            "automatic_validation_complete": true,
            "model_output_is_ground_truth": false,
            "validator_proves_content_truth": false,
            "routine_human_task_created": false,
            "promotion_authorized": false
        },
        "provenance_event_ref": event_id,
        "authority_boundary": crate::source_text_foundation::LEGACY_LAYER_AUTHORITY,
        "layer_version": 1,
        "supersedes_layer_ref": null
    }))
}
pub(super) fn unit_anchor(
    anchor_ref: &Value,
    ordinal: usize,
    layer_ref: &Value,
    layer_digest: &str,
    content: &str,
    start: usize,
    end: usize,
    role: &str,
    locator_ref: &Value,
) -> Result<Value> {
    Ok(json!({
        "anchor_ref": anchor_ref,
        "ordinal": ordinal,
        "text_layer_ref": layer_ref,
        "text_layer_sha256": layer_digest,
        "selector": {
            "type": "text_position",
            "start": start,
            "end": end,
            "position_unit": "unicode_code_point",
            "interval": "half_open"
        },
        "exact_sha256": sha(slice(content, start, end)?.as_bytes()),
        "anchor_role": role,
        "source_return": {
            "required": true,
            "locator_ref": locator_ref
        }
    }))
}
pub(super) fn unit(
    unit_id: &Value,
    anchor_ref: &Value,
    kind: &str,
    boundary_posture: &str,
    status_reason: &str,
) -> Result<Value> {
    Ok(json!({
        "unit_id": unit_id,
        "unit_version": 1,
        "supersedes_unit_ref": null,
        "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
        "unit_kind": kind,
        "surface_posture": "source_bearing",
        "continuity": "contiguous",
        "ordered_anchor_refs": [anchor_ref],
        "parent_unit_refs": [],
        "ordered_child_unit_refs": [],
        "boundary_posture": boundary_posture,
        "certainty": {
            "value": 1.0,
            "meaning": "maker-declared-boundary-confidence-not-truth-probability"
        },
        "status_reason": status_reason,
        "source_text_mutated": false,
        "semantic_promotion": false
    }))
}
pub(super) fn unit_packet(
    plan: &Value,
    plan_ref: &str,
    builder: &str,
    event_id: &str,
    normalized: &str,
    source_parts: &[String],
    authored_parts: &[String],
) -> Result<Value> {
    let ids = &plan["opaque_ids"];
    let scope = &plan["scope"];
    let layer_ref = &plan["outputs"]["normalized_layer_ref"];
    let layer_content_digest = sha(normalized.as_bytes());
    let (source_spans, source_gaps) = spans(source_parts, normalized)?;
    let (authored_spans, authored_gaps) = spans(authored_parts, normalized)?;
    let locator = &plan["outputs"]["private_normalized_content_ref"];
    let mut anchors = vec![unit_anchor(
        &ids["scope_anchor_id"],
        1,
        layer_ref,
        &layer_content_digest,
        normalized,
        0,
        normalized.chars().count(),
        "scope",
        locator,
    )?];
    for (key, selected, role) in [
        ("source_anchor_ids", &source_spans, "content"),
        ("source_gap_anchor_ids", &source_gaps, "gap"),
        ("authored_anchor_ids", &authored_spans, "content"),
        ("authored_gap_anchor_ids", &authored_gaps, "gap"),
    ] {
        let names = a(&ids[key])?;
        ensure(names.len() == selected.len(), "unit anchor membership")?;
        for (name, &(start, end)) in names.iter().zip(selected) {
            anchors.push(unit_anchor(
                name,
                anchors.len() + 1,
                layer_ref,
                &layer_content_digest,
                normalized,
                start,
                end,
                role,
                locator,
            )?);
        }
    }
    let mut units = vec![];
    for (unit_key, anchor_key, kind, boundary, reason) in [
        (
            "source_unit_ids",
            "source_anchor_ids",
            "paragraph",
            "source_attested",
            "The boundary derives from one exact direct TEI p element after a separate declared comparison normalization; it is not accepted German or semantics.",
        ),
        (
            "authored_unit_ids",
            "authored_anchor_ids",
            "other",
            "method_proposed",
            "The boundary is imported from the pre-existing authored twelve-segment route and mechanically crosswalked only; older authored/review posture is not a modern source, linguistic, translation, semantic, or human-review decision.",
        ),
    ] {
        let names = a(&ids[unit_key])?;
        let refs = a(&ids[anchor_key])?;
        ensure(
            names.len() == refs.len() && names.len() == 12,
            "unit membership",
        )?;
        for (name, reference) in names.iter().zip(refs) {
            units.push(unit(name, reference, kind, boundary, reason)?);
        }
    }
    let common_method = json!({
        "agent_ref": SOFTWARE_AGENT, "method_version":"1", "software_refs":[builder],
        "model_ref":null, "configuration_ref":plan_ref, "locale":"de", "unicode_version":"16.0.0",
        "unicode_revision":null, "tailoring_ref":null, "provenance_event_ref":event_id,
        "made_at":plan["created_at"], "output_posture":"method_result_not_source_or_linguistic_truth"
    });
    let mut source_method = common_method.clone();
    source_method["maker_kind"] = json!("software");
    source_method["method_name"] =
        json!("exact TEI paragraph boundaries on declared normalized layer");
    let mut authored_method = common_method;
    authored_method["maker_kind"] = json!("import");
    authored_method["agent_ref"] = json!("record:tos-source-thus-spoke-zarathustra-prologue");
    authored_method["method_name"] =
        json!("digest-bound import of legacy authored segment boundaries");
    let source_policy = json!({"normalization":"no-text-mutation-separate-successor-layer", "punctuation":"included_in_neighbor", "whitespace":"declared_excluded", "line_break":"not_applicable", "hyphenation":"separate_successor_layer", "unreported_gaps_allowed":false, "overlap":"forbid"});
    let authority_limit = json!({"algorithmic_output_is_source_truth":false, "algorithmic_output_is_linguistic_truth":false, "model_subword_is_lexeme":false, "unit_identity_is_semantic_identity":false, "text_mutation_allowed":false});
    Ok(json!({
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
        "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": ids["packet_id"],
        "packet_version": 1,
        "supersedes_packet_ref": null,
        "content_posture": "source_bound",
        "source_scope": {
            "work_ref": scope["work_ref"],
            "expression_ref": scope["expression_ref"],
            "edition_ref": scope["edition_ref"],
            "item_ref": scope["item_ref"],
            "file_ref": scope["file_ref"],
            "file_sha256": scope["file_sha256"]
        },
        "source_layer": {
            "text_layer_ref": layer_ref,
            "text_layer_sha256": layer_content_digest,
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
            "scheme_id": ids["source_scheme_id"],
            "scheme_version": 1,
            "supersedes_scheme_ref": null,
            "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
            "scheme_name": "DTA source paragraph structure on comparison layer",
            "analysis_role": "source_structure",
            "boundary_basis": "source_markup",
            "unit_kinds": ["paragraph"],
            "method": source_method,
            "policies": source_policy,
            "authority_limit": authority_limit
        }, {
            "scheme_id": ids["authored_scheme_id"],
            "scheme_version": 1,
            "supersedes_scheme_ref": null,
            "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
            "scheme_name": "pre-existing authored prologue route segment crosswalk",
            "analysis_role": "interchange",
            "boundary_basis": "imported",
            "unit_kinds": ["other"],
            "method": authored_method,
            "policies": source_policy,
            "authority_limit": authority_limit
        }],
        "anchors": anchors,
        "units": units,
        "segmentations": [{
            "segmentation_id": ids["source_segmentation_id"],
            "segmentation_version": 1,
            "supersedes_segmentation_ref": null,
            "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
            "scheme_ref": ids["source_scheme_id"],
            "ordered_unit_refs": ids["source_unit_ids"],
            "coverage": {
                "scope_anchor_ref": ids["scope_anchor_id"],
                "coverage_posture": "declared_partial",
                "excluded_anchor_refs": ids["source_gap_anchor_ids"],
                "unreported_gaps_allowed": false,
                "overlap_requires_declaration": true,
                "source_reconstruction_required": true
            },
            "status": "observed_source_structure",
            "status_reason": "Twelve exact TEI paragraph boundaries are mechanically observed on the separate normalized layer; excluded spaces represent declared inter-paragraph separators.",
            "maker": source_method,
            "competing_segmentation_refs": [ids["authored_segmentation_id"]],
            "review_refs": [],
            "declared_uses": ["navigation", "source_observation"],
            "source_text_authority": false,
            "linguistic_authority": false,
            "semantic_authority": false
        }, {
            "segmentation_id": ids["authored_segmentation_id"],
            "segmentation_version": 1,
            "supersedes_segmentation_ref": null,
            "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
            "scheme_ref": ids["authored_scheme_id"],
            "ordered_unit_refs": ids["authored_unit_ids"],
            "coverage": {
                "scope_anchor_ref": ids["scope_anchor_id"],
                "coverage_posture": "declared_partial",
                "excluded_anchor_refs": ids["authored_gap_anchor_ids"],
                "unreported_gaps_allowed": false,
                "overlap_requires_declaration": true,
                "source_reconstruction_required": true
            },
            "status": "proposed",
            "status_reason": "The twelve authored boundaries match the complete normalized source sequence but retain legacy authored rather than modern source, language, translation, semantic, or review assurance.",
            "maker": authored_method,
            "competing_segmentation_refs": [ids["source_segmentation_id"]],
            "review_refs": [],
            "declared_uses": ["navigation", "interchange"],
            "source_text_authority": false,
            "linguistic_authority": false,
            "semantic_authority": false
        }],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "source_visibility": "local_only",
            "packet_visibility": "public_metadata_only",
            "effective_visibility": "local_only",
            "rights_record_refs": [scope["rights_ref"]],
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
    }))
}
