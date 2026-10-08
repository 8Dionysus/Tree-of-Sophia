//! Source-owned record shapes, translated from the maintained source foundation producer.
use super::*;
pub(super) fn anchor(plan: &Value, plan_ref: &str, plan_digest: &str, event_id: &str) -> Value {
    let scope = &plan["scope"];
    let ids = &plan["opaque_ids"];
    json!({
    "$schema": "https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json",
    "schema_version": "tos_source_anchor_v2",
    "anchor_id": ids["anchor_id"],
    "passage_id": ids["passage_id"],
    "target": {
    "item_id": scope["item_ref"],
    "file_id": scope["file_ref"],
    "file_sha256": scope["file_sha256"],
    "media_type": "application/pdf"
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
    "media_type": "application/pdf",
    "character_normalization": "none"
    },
    "selector": {
    "type": "page_region",
    "page_identity": {
    "page_number": plan["selector"]["page_number"]
    },
    "x": region["x"],
    "y": region["y"],
    "width": region["width"],
    "height": region["height"],
    "coordinate_space": "points",
    "source_width": plan["selector"]["page_width_points"],
    "source_height": plan["selector"]["page_height_points"]
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
    "method": plan["extraction_policy"]["method"],
    "version": plan["extraction_policy"]["method_version"],
    "configuration_ref": plan_ref,
    "configuration_digest": plan_digest
    },
    "resolution_status": "mechanically_resolved",
    "review_status": "unreviewed",
    "provenance_event_ref": event_id,
    "anchor_version": 1,
    "supersedes_anchor_ref": null,
    "review_ref": null
    })
}
pub(super) fn layer(
    plan: &Value,
    plan_ref: &str,
    plan_digest: &str,
    anchor_digest: &str,
    rights_digest: &str,
    content: &[u8],
    text: &str,
    event_id: &str,
) -> Value {
    let scope = &plan["scope"];
    let ids = &plan["opaque_ids"];
    let content_digest = sha(content);
    json!({
    "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json",
    "schema_version": "tos_source_text_layer_v1",
    "layer_id": ids["layer_id"],
    "layer_role": "raw_ocr",
    "source_binding": {
    "work_ref": scope["work_ref"],
    "expression_ref": scope["expression_ref"],
    "edition_ref": scope["edition_ref"],
    "item_ref": scope["item_ref"],
    "source_file_ref": scope["file_ref"],
    "source_file_sha256": scope["file_sha256"],
    "anchor_contract": "tos_source_anchor_v2",
    "anchors": [{
    "anchor_id": ids["anchor_id"],
    "anchor_record_ref": plan["outputs"]["anchor_ref"],
    "anchor_record_sha256": anchor_digest
    }]
    },
    "representation": {
    "content_file_id": format!("tos.file.sha256.{}", content_digest),
    "content_ref": plan["outputs"]["private_content_ref"],
    "content_sha256": content_digest,
    "media_type": "text/plain",
    "charset": "UTF-8",
    "language": "ru",
    "text_scope": {
    "start": 0,
    "end": text.chars().count(),
    "position_unit": "unicode_code_point",
    "interval": "half_open"
    },
    "character_normalization": "none",
    "line_break_posture": "source_preserved",
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
    "derivation": {
    "method": "structural_extraction",
    "input_layers": [],
    "maker": {
    "maker_type": "software",
    "agent_ref": "software:tos-zarathustra-target-text-foundation-builder",
    "method": plan["extraction_policy"]["method"],
    "version": plan["extraction_policy"]["method_version"],
    "configuration_ref": plan_ref,
    "configuration_digest": plan_digest
    },
    "preservation_goal": "source_near",
    "loss_posture": "lossy_for_declared_use",
    "silent_changes_allowed": false,
    "change_payload": {
    "kind": "none"
    }
    },
    "editorial_policy": {
    "policy_ref": plan_ref,
    "policy_sha256": plan_digest,
    "transcription_goal": "machine_candidate",
    "historical_language_preserved": true,
    "printing_errors_silently_corrected": false,
    "typography_posture": "encode_explicitly",
    "layout_posture": "encode_explicitly",
    "unicode_normalization": "none",
    "uncertainty_representation": "explicit-never-silent",
    "method_declared": true
    },
    "uncertainty": {
    "status": "unresolved",
    "annotations": [{
    "annotation_id": ids["uncertainty_annotation_id"],
    "anchor_ref": ids["anchor_id"],
    "kind": "ambiguous_reading",
    "alternatives": [{
    "value_sha256": visual["embedded_token_sequence_sha256"],
    "value_in_record": false,
    "value": null,
    "status": "possible"
    }, {
    "value_sha256": visual["visually_joined_candidate_sha256"],
    "value_in_record": false,
    "value": null,
    "status": "possible"
    }],
    "resolution": "proposed"
    }]
    },
    "admission": {
    "mechanical_status": "fixity_verified",
    "review_status": "unreviewed",
    "review_ref": null,
    "human_review_performed": false,
    "human_language_competence": "not_assessed",
    "language_competence_evidence_refs": [],
    "accepted_uses": [],
    "automatic_validation_complete": true,
    "model_output_is_ground_truth": false,
    "validator_proves_content_truth": false,
    "routine_human_task_created": false,
    "promotion_authorized": false
    },
    "provenance_event_ref": event_id,
    "authority_boundary": "A source text layer preserves one immutable, source-returnable representation and its derivation, uncertainty, assessment, competence, rights and use scope.",
    "layer_version": 1,
    "supersedes_layer_ref": null
    })
}
pub(super) fn packet(
    plan: &Value,
    layer_record_ref: &str,
    content: &[u8],
    anchors: Vec<Value>,
    units: Vec<Value>,
    method: &Value,
) -> Value {
    let scope = &plan["scope"];
    let ids = &plan["opaque_ids"];
    let anchor_ids = &ids["anchor_ids"];
    let unit_ids = &ids["unit_ids"];
    let content_digest = sha(content);
    json!({
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
    "text_layer_ref": plan["outputs"]["text_layer_ref"],
    "text_layer_sha256": content_digest,
    "language": "ru",
    "media_type": "text/plain; charset=utf-8",
    "unicode_form": "source_preserved",
    "position_unit": "unicode_code_point",
    "interval": "half_open",
    "immutable": true,
    "visibility": "local_only",
    "publication_authorized": false
    },
    "schemes": [{
    "scheme_id": ids["scheme_id"],
    "scheme_version": 1,
    "supersedes_scheme_ref": null,
    "scheme_name": "Antonovsky 1911 embedded-PDF bbox source-layout observation",
    "analysis_role": "source_layout",
    "unit_kinds": ["physical_line", "whitespace"],
    "boundary_basis": "source_layout",
    "policies": {
    "punctuation": "included_in_neighbor",
    "whitespace": "standalone_units",
    "line_break": "standalone_units",
    "hyphenation": "preserve_source",
    "normalization": "no-text-mutation-separate-successor-layer",
    "overlap": "forbid",
    "unreported_gaps_allowed": false
    },
    "method": method,
    "authority_limit": {
    "algorithmic_output_is_source_truth": false,
    "algorithmic_output_is_linguistic_truth": false,
    "model_subword_is_lexeme": false,
    "unit_identity_is_semantic_identity": false,
    "text_mutation_allowed": false
    },
    "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis"
    }],
    "anchors": anchors,
    "units": units,
    "segmentations": [{
    "segmentation_id": ids["segmentation_id"],
    "segmentation_version": 1,
    "supersedes_segmentation_ref": null,
    "scheme_ref": ids["scheme_id"],
    "ordered_unit_refs": unit_ids,
    "coverage": {
    "scope_anchor_ref": anchor_ids[0],
    "coverage_posture": "exhaustive_nonoverlapping",
    "excluded_anchor_refs": [],
    "unreported_gaps_allowed": false,
    "overlap_requires_declaration": true,
    "source_reconstruction_required": true
    },
    "competing_segmentation_refs": [],
    "maker": method,
    "status": "observed_source_structure",
    "status_reason": "Only six exact embedded-PDF bbox lines are observed; word joining is the declared machine rule and no sentence, token, accepted text, or translation boundary is asserted.",
    "review_refs": [],
    "source_text_authority": false,
    "linguistic_authority": false,
    "semantic_authority": false,
    "declared_uses": ["navigation", "source_observation"],
    "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries"
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
    "source_role": "authority",
    "tree_role": "orientation",
    "graph_role": "relation",
    "projection_is_owner_truth": false,
    "validators_prove_mechanics_not_truth": true,
    "segmentation_is_method_result_not_source_truth": true,
    "model_token_is_not_linguistic_token": true,
    "source_unit_is_not_lexeme_sign_or_concept": true,
    "legacy_bulk_migration_authorized": false
    }
    })
}
pub(super) struct Provenance<'a> {
    pub plan: &'a Value,
    pub plan_ref: &'a str,
    pub plan_digest: &'a str,
    pub event_id: &'a str,
    pub source_digest: &'a str,
    pub source_len: usize,
    pub plan_len: usize,
    pub builder_ref: &'a str,
    pub builder_digest: &'a str,
    pub executable_digest: &'a str,
    pub argv: &'a Value,
    pub argv_digest: &'a str,
    pub outputs: &'a Vec<Value>,
    pub derivations: &'a Vec<Value>,
    pub pdftotext_version: &'a str,
    pub pdftotext_digest: &'a str,
    pub total_output_bytes: usize,
    pub rights_binding: &'a Value,
    pub observed_at: &'a str,
}
pub(super) fn provenance(v: Provenance<'_>) -> Value {
    let Provenance {
        plan,
        plan_ref,
        plan_digest,
        event_id,
        source_digest,
        source_len,
        plan_len,
        builder_ref,
        builder_digest,
        executable_digest,
        argv,
        argv_digest,
        outputs,
        derivations,
        pdftotext_version,
        pdftotext_digest,
        total_output_bytes,
        rights_binding,
        observed_at,
    } = v;
    let runtime_name = "Tree-of-Sophia native/Rust";
    let runtime_version = env!("CARGO_PKG_VERSION");
    let uv = std::char::UNICODE_VERSION;
    let unicode_version = format!("{}.{}.{}", uv.0, uv.1, uv.2);
    let mut event = json!({
    "$schema": "https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json",
    "schema_version": "tos_provenance_event_v2",
    "event_id": event_id,
    "event_version": 1,
    "supersedes_event_ref": null,
    "record_binding": {
    "manifest_ref": plan_ref,
    "digest_algorithm": "sha256",
    "digest_scope": "exact_event_record_bytes"
    },
    "activity": {
    "event_type": "native_extraction",
    "started_at": plan["created_at"],
    "ended_at": plan["created_at"],
    "status": "completed_with_warnings",
    "terminal_reason": null,
    "exit_code": 0,
    "warnings": ["Poppler emitted seventeen Invalid Font Weight warnings while returning the frozen bbox bytes.", "The embedded text is an unreviewed raw candidate and one visual letterspacing divergence remains unresolved.", "The exact PDF, bbox intermediate, and extracted text remain local-only; publication is unauthorized.", "The unsigned self-recorded event proves neither execution truth nor source fidelity."]
    },
    "entities": {
    "inputs": [{
    "entity_ref": plan["scope"]["source_relative_ref"],
    "role": "fixity-verified-local-antonovsky-1911-pdf",
    "sha256": plan["scope"]["file_sha256"],
    "size_bytes": source_len,
    "media_type": "application/pdf",
    "availability": "owner_local",
    "content_disclosure": "private_content",
    "fixity_verified": true,
    "fixity_verified_at": plan["created_at"]
    }, {
    "entity_ref": plan_ref,
    "role": "tracked-text-free-materialization-plan",
    "sha256": plan_digest,
    "size_bytes": plan_len,
    "media_type": "application/json",
    "availability": "tracked",
    "content_disclosure": "public_metadata_only",
    "fixity_verified": true,
    "fixity_verified_at": plan["created_at"]
    }],
    "outputs": outputs,
    "byproducts": []
    },
    "derivations": derivations,
    "responsibility": [{
    "agent_ref": "software:tos-zarathustra-target-text-foundation-builder",
    "agent_kind": "software",
    "role": "executor",
    "responsibility_posture": "performed",
    "evidence_binding": {
    "ref": builder_ref,
    "sha256": builder_digest
    },
    "human_evidence_status": "not_applicable"
    }, {
    "agent_ref": "model:openai-codex",
    "agent_kind": "model",
    "role": "observer",
    "responsibility_posture": "observed",
    "evidence_binding": {
    "ref": plan_ref,
    "sha256": plan_digest
    },
    "human_evidence_status": "not_applicable"
    }],
    "method": {
    "procedure": {
    "name": plan["extraction_policy"]["method"],
    "version": plan["extraction_policy"]["method_version"],
    "purpose": plan["question"]
    },
    "command_capture": {
    "disclosure": "inline",
    "argv": argv,
    "argv_sha256": argv_digest,
    "withholding_reason": null
    },
    "configuration_binding": {
    "ref": plan_ref,
    "sha256": plan_digest
    },
    "software_components": [{
    "name": "Tree of Sophia Zarathustra target-text foundation builder",
    "version": "1",
    "role": "native-extraction-and-record-builder",
    "artifact_ref": builder_ref,
    "artifact_sha256": builder_digest,
    "verification_status": "verified"
    }, {
    "name": "pdftotext",
    "version": pdftotext_version,
    "role": "pdf-embedded-text-bbox-extractor",
    "artifact_ref": "runtime:pdftotext-executable",
    "artifact_sha256": pdftotext_digest,
    "verification_status": "verified"
    }, {
    "name": runtime_name,
    "version": runtime_version,
    "role": "native-executable",
    "artifact_ref": "runtime:tos-native-executable",
    "artifact_sha256": executable_digest,
    "verification_status": "verified"
    }],
    "model_invocations": [],
    "environment": {
    "runtime": runtime_name,
    "runtime_version": runtime_version,
    "runtime_artifact_sha256": executable_digest,
    "backend": format!("poppler-pdftotext-{}-bbox-layout", pdftotext_version),
    "hardware_target": "cpu",
    "unicode_version": unicode_version,
    "environment_profile_binding": {
    "ref": plan_ref,
    "sha256": plan_digest
    }
    }
    },
    "manual_changes": {
    "status": "none_declared",
    "change_receipts": [],
    "statement": "No manual edit occurred between Poppler bbox extraction and the private raw layer; words and lines are joined only by the tracked plan's rule."
    },
    "measurements": [{
    "metric": "input_bytes",
    "status": "measured",
    "value": source_len,
    "unit": "bytes",
    "method": "filesystem stat after exact digest verification",
    "evidence_binding": null
    }, {
    "metric": "output_bytes",
    "status": "measured",
    "value": total_output_bytes,
    "unit": "bytes",
    "method": "sum of exact private and tracked output entity byte counts",
    "evidence_binding": null
    }, {
    "metric": "human_active_seconds",
    "status": "not_applicable",
    "value": null,
    "unit": null,
    "method": "no human content review or correction was scheduled or performed",
    "evidence_binding": null
    }],
    "evidence_authentication": {
    "capture_posture": "tool_captured",
    "signature_status": "unsigned",
    "signature_bindings": [],
    "verification_status": "mechanically_verified",
    "producer_control_boundary": "The same local builder executed extraction and emitted this unsigned record; independent byte checks can replay closure but cannot authenticate execution, the provider OCR process, or textual fidelity."
    },
    "rights_and_visibility": {
    "rights_record_bindings": [rights_binding],
    "intended_uses": ["local_research", "preservation", "indexing"],
    "content_visibility": "local_only",
    "publication_authorized": false,
    "publication_authority_bindings": []
    },
    "review_and_authority": {
    "mechanical_validation": "passed",
    "human_review_status": "not_performed",
    "review_bindings": [],
    "accepted_uses": [],
    "promotion_authorized": false,
    "competence_evidence_bindings": []
    },
    "reproducibility": {
    "classification": "replay_ready",
    "known_gaps": ["The receipt is unsigned and self-recorded by the transformation runner.", "No human reviewed the embedded text or source-visible line content.", "The provider's embedded-text production history remains unknown.", "The exact source, bbox intermediate, and extracted text are absent from Git."],
    "replay_scope": "Exact Antonovsky 1911 PDF digest, tracked plan and builder, pinned Poppler version and executable digest, page-region guards, private bbox/text digests, tracked record digests, and fail-closed authority posture."
    },
    "authority_boundary": {
    "validator_role": "mechanics_and_closure_only_not_truth",
    "claims_not_established": ["execution_truth", "content_truth", "source_fidelity", "translation_quality", "semantic_correctness", "rights_clearance", "human_review", "publication_authority", "canon_authority"]
    }
    });
    event["activity"]["started_at"] = json!(observed_at);
    event["activity"]["ended_at"] = json!(observed_at);
    for direction in ["inputs", "outputs"] {
        for entity in event["entities"][direction].as_array_mut().unwrap() {
            entity["fixity_verified_at"] = json!(observed_at);
        }
    }
    event
}
pub(super) fn scope_anchor(
    plan: &Value,
    layer_record_ref: &str,
    content: &[u8],
    text: &str,
) -> Value {
    let anchor_ids = &plan["opaque_ids"]["anchor_ids"];
    let content_digest = sha(content);
    json!({
    "anchor_ref": anchor_ids[0],
    "anchor_role": "scope",
    "ordinal": 1,
    "text_layer_ref": plan["outputs"]["text_layer_ref"],
    "text_layer_sha256": content_digest,
    "selector": {
    "type": "text_position",
    "start": 0,
    "end": text.chars().count(),
    "position_unit": "unicode_code_point",
    "interval": "half_open"
    },
    "exact_sha256": content_digest,
    "source_return": {
    "required": true,
    "locator_ref": plan["outputs"]["private_content_ref"]
    }
    })
}
pub(super) fn content_anchor(
    plan: &Value,
    layer_record_ref: &str,
    content_digest: &str,
    anchor_id: &Value,
    ordinal: usize,
    kind: &str,
    start: usize,
    end: usize,
    exact: &[u8],
) -> Value {
    json!({
    "anchor_ref": anchor_id,
    "anchor_role": if kind == "physical_line" { "content" } else { "whitespace" },
    "ordinal": ordinal,
    "text_layer_ref": plan["outputs"]["text_layer_ref"],
    "text_layer_sha256": content_digest,
    "selector": {
    "type": "text_position",
    "start": start,
    "end": end,
    "position_unit": "unicode_code_point",
    "interval": "half_open"
    },
    "exact_sha256": sha(exact),
    "source_return": {
    "required": true,
    "locator_ref": plan["outputs"]["private_content_ref"]
    }
    })
}
pub(super) fn unit(unit_id: &Value, anchor_id: &Value, kind: &str) -> Value {
    json!({
    "unit_id": unit_id,
    "unit_version": 1,
    "supersedes_unit_ref": null,
    "unit_kind": kind,
    "surface_posture": "source_bearing",
    "continuity": "contiguous",
    "ordered_anchor_refs": [anchor_id],
    "ordered_child_unit_refs": [],
    "parent_unit_refs": [],
    "boundary_posture": "source_attested",
    "certainty": {
    "value": 1.0,
    "meaning": "maker-declared-boundary-confidence-not-truth-probability"
    },
    "source_text_mutated": false,
    "semantic_promotion": false,
    "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
    "status_reason": "The unit records one exact Poppler bbox line or its line-break code point in the unreviewed embedded-text layer and retains its proposed source-unit status."
    })
}
pub(super) fn method(
    plan: &Value,
    plan_ref: &str,
    builder_ref: &str,
    event_id: &str,
    unicode_version: &str,
    made_at: &str,
) -> Value {
    json!({
    "maker_kind": "software",
    "agent_ref": "software:tos-zarathustra-target-text-foundation-builder",
    "method_name": "exact Poppler bbox source-layout observation",
    "method_version": "1",
    "software_refs": [builder_ref],
    "model_ref": null,
    "configuration_ref": plan_ref,
    "locale": "ru",
    "unicode_version": unicode_version,
    "unicode_revision": null,
    "tailoring_ref": null,
    "provenance_event_ref": event_id,
    "made_at": made_at,
    "output_posture": "method_result_not_source_or_linguistic_truth"
    })
}
