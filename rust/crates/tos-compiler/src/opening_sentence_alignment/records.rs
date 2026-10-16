//! Text-free record shapes from the maintained bounded sentence proposal.
use super::*;
pub(super) fn method(
    plan: &Value,
    language: &Value,
    plan_ref: &str,
    builder_ref: &str,
    event_id: &str,
    unicode_version: &str,
    made_at: &str,
) -> Value {
    json!({
    "maker_kind": "software",
    "agent_ref": "software:tos-zarathustra-opening-sentence-alignment-builder",
    "method_name": "exact first-full-stop-inclusive sentence-boundary proposal",
    "method_version": plan["method"]["version"],
    "software_refs": [builder_ref],
    "model_ref": null,
    "configuration_ref": plan_ref,
    "locale": language,
    "unicode_version": unicode_version,
    "unicode_revision": null,
    "tailoring_ref": null,
    "provenance_event_ref": event_id,
    "made_at": made_at,
    "output_posture": "method_result_not_source_or_linguistic_truth"
    })
}
pub(super) fn unit_anchor(
    side: &Value,
    anchor_ref: &Value,
    ordinal: usize,
    role: &str,
    start: usize,
    end: usize,
    digest: &str,
) -> Value {
    json!({
    "anchor_ref": anchor_ref,
    "ordinal": ordinal,
    "text_layer_ref": side["text_layer_ref"],
    "text_layer_sha256": side["text_layer_sha256"],
    "selector": {
    "type": "text_position",
    "start": start,
    "end": end,
    "position_unit": "unicode_code_point",
    "interval": "half_open"
    },
    "exact_sha256": digest,
    "anchor_role": role,
    "source_return": {
    "required": true,
    "locator_ref": side["private_content_ref"]
    }
    })
}
pub(super) fn unit_packet(
    plan: &Value,
    side_name: &str,
    text: &str,
    sentence: &str,
    remainder: &str,
    method: &Value,
) -> Value {
    let side = &plan[side_name];
    let ids = &plan["opaque_ids"];
    let prefix = side_name;
    let packet_id = &ids[format!("{prefix}_packet_id")];
    let scheme_id = &ids[format!("{prefix}_scheme_id")];
    let segmentation_id = &ids[format!("{prefix}_segmentation_id")];
    let scope_anchor_id = &ids[format!("{prefix}_scope_anchor_id")];
    let sentence_anchor_id = &ids[format!("{prefix}_sentence_anchor_id")];
    let remainder_anchor_id = &ids[format!("{prefix}_remainder_anchor_id")];
    let sentence_unit_id = &ids[format!("{prefix}_sentence_unit_id")];
    let sentence_end = sentence.chars().count();
    let source_scope = [
        "work_ref",
        "expression_ref",
        "edition_ref",
        "item_ref",
        "file_ref",
        "file_sha256",
    ]
    .into_iter()
    .map(|k| (k.to_owned(), side[k].clone()))
    .collect::<serde_json::Map<_, _>>();
    json!({
    "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
    "schema_version": "tos_source_text_unit_packet_v1",
    "packet_id": packet_id,
    "packet_version": 1,
    "supersedes_packet_ref": null,
    "content_posture": "source_bound",
    "source_scope": source_scope,
    "source_layer": {
    "text_layer_ref": side["text_layer_ref"],
    "text_layer_sha256": side["text_layer_sha256"],
    "language": side["language"],
    "media_type": "text/plain; charset=utf-8",
    "unicode_form": "source_preserved",
    "position_unit": "unicode_code_point",
    "interval": "half_open",
    "immutable": true,
    "visibility": "local_only",
    "publication_authorized": false
    },
    "schemes": [{
    "scheme_id": scheme_id,
    "scheme_version": 1,
    "supersedes_scheme_ref": null,
    "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
    "scheme_name": format!("{} exact first-full-stop-inclusive sentence proposal", s(&side["language"]).unwrap_or("")),
    "analysis_role": "orthographic",
    "boundary_basis": "rule_based",
    "unit_kinds": ["sentence"],
    "method": method,
    "policies": {
    "normalization": "no-text-mutation-separate-successor-layer",
    "punctuation": "included_in_neighbor",
    "whitespace": "declared_excluded",
    "line_break": "declared_excluded",
    "hyphenation": "preserve_source",
    "unreported_gaps_allowed": false,
    "overlap": "forbid"
    },
    "authority_limit": {
    "algorithmic_output_is_source_truth": false,
    "algorithmic_output_is_linguistic_truth": false,
    "model_subword_is_lexeme": false,
    "unit_identity_is_semantic_identity": false,
    "text_mutation_allowed": false
    }
    }],
    "anchors": [unit_anchor(side, scope_anchor_id, 1, "scope", 0, text.chars().count(), s(&side["text_layer_sha256"]).unwrap_or("")), unit_anchor(side, sentence_anchor_id, 2, "content", 0, sentence_end, &sha(sentence.as_bytes())), unit_anchor(side, remainder_anchor_id, 3, "gap", sentence_end, text.chars().count(), &sha(remainder.as_bytes()))],
    "units": [{
    "unit_id": sentence_unit_id,
    "unit_version": 1,
    "supersedes_unit_ref": null,
    "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
    "unit_kind": "sentence",
    "surface_posture": "source_bearing",
    "continuity": "contiguous",
    "ordered_anchor_refs": [sentence_anchor_id],
    "parent_unit_refs": [],
    "ordered_child_unit_refs": [],
    "boundary_posture": "method_proposed",
    "certainty": {
    "value": 0.5,
    "meaning": "maker-declared-boundary-confidence-not-truth-probability"
    },
    "status_reason": "The mechanically selected first U+002E boundary supplies a bounded alignment proposal; sentence analysis and textual acceptance retain their recorded review status.",
    "source_text_mutated": false,
    "semantic_promotion": false
    }],
    "segmentations": [{
    "segmentation_id": segmentation_id,
    "segmentation_version": 1,
    "supersedes_segmentation_ref": null,
    "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
    "scheme_ref": scheme_id,
    "ordered_unit_refs": [sentence_unit_id],
    "coverage": {
    "scope_anchor_ref": scope_anchor_id,
    "coverage_posture": "declared_partial",
    "excluded_anchor_refs": [remainder_anchor_id],
    "unreported_gaps_allowed": false,
    "overlap_requires_declaration": true,
    "source_reconstruction_required": true
    },
    "status": "proposed",
    "status_reason": "Only the first full-stop-delimited span is proposed; the exact remainder is digest-bound as excluded coverage and no linguistic review occurred.",
    "maker": method,
    "competing_segmentation_refs": [],
    "review_refs": [],
    "declared_uses": ["source_observation", "translation_alignment"],
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
    "rights_record_refs": [side["rights_ref"]],
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
pub(super) fn alignment_side(
    plan: &Value,
    side_name: &str,
    packet_ref: &str,
    packet_digest: &str,
) -> Value {
    let side = &plan[side_name];
    let anchor_ref = &plan["opaque_ids"][format!("{side_name}_sentence_anchor_id")];
    json!({
    "side_role": side_name,
    "work_ref": side["work_ref"],
    "expression_ref": side["expression_ref"],
    "edition_ref": side["edition_ref"],
    "item_ref": side["item_ref"],
    "file_ref": side["file_ref"],
    "file_sha256": side["file_sha256"],
    "text_layer_ref": side["text_layer_ref"],
    "text_layer_sha256": side["text_layer_sha256"],
    "language": side["language"],
    "segmentation": {
    "artifact_ref": packet_ref,
    "sha256": packet_digest,
    "state": "frozen"
    },
    "tokenization": null,
    "anchors": [{
    "anchor_ref": anchor_ref,
    "ordinal": 1,
    "text_layer_ref": side["text_layer_ref"],
    "text_layer_sha256": side["text_layer_sha256"],
    "selector": {
    "type": "text_position",
    "start": side["sentence_start"],
    "end": side["sentence_end"],
    "position_unit": "unicode_code_point",
    "interval": "half_open"
    },
    "exact_sha256": side["sentence_sha256"],
    "source_return": {
    "required": true,
    "locator_ref": side["private_content_ref"]
    }
    }],
    "rights_refs": [side["rights_ref"]],
    "visibility": "local_only",
    "publication_authorized": false
    })
}
pub(super) fn alignment_packet(
    plan: &Value,
    plan_ref: &str,
    builder_ref: &str,
    event_id: &str,
    made_at: &str,
    plan_digest: &str,
    source_packet_ref: &str,
    source_packet_digest: &str,
    target_packet_ref: &str,
    target_packet_digest: &str,
) -> Value {
    let ids = &plan["opaque_ids"];
    let source_anchor = &ids["source_sentence_anchor_id"];
    let target_anchor = &ids["target_sentence_anchor_id"];
    json!({
    "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json",
    "schema_version": "tos_translation_alignment_packet_v1",
    "packet_id": ids["alignment_packet_id"],
    "packet_version": 1,
    "supersedes_packet_ref": null,
    "content_posture": "source_bound",
    "granularity": "sentence",
    "source_side": alignment_side(plan, "source", source_packet_ref, source_packet_digest),
    "target_side": alignment_side(plan, "target", target_packet_ref, target_packet_digest),
    "alignments": [{
    "alignment_id": ids["alignment_id"],
    "alignment_version": 1,
    "supersedes_alignment_ref": null,
    "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
    "claim_id": ids["alignment_claim_id"],
    "claim_version": 1,
    "supersedes_claim_ref": null,
    "direction": "source_to_target",
    "correspondence_shape": "one_to_one",
    "order_posture": "monotonic",
    "ordered_source_anchor_refs": [source_anchor],
    "ordered_target_anchor_refs": [target_anchor],
    "translation_techniques": ["unresolved"],
    "epistemic_status": "inferred",
    "certainty": {
    "value": 0.5,
    "meaning": "maker_declared_uncertainty_not_truth_probability"
    },
    "status": "proposed",
    "status_reason": "The two exact spans are proposed as an ordinal structural correspondence only. Translation fidelity, wording, grammar, lexical equivalence, and semantic interpretation remain unreviewed.",
    "maker": {
    "maker_kind": "software",
    "agent_ref": "software:tos-zarathustra-opening-sentence-alignment-builder",
    "made_at": made_at,
    "method": plan["method"]["name"],
    "provenance_event_ref": event_id,
    "method_output_posture": "proposal_not_truth",
    "software_ref": builder_ref,
    "software_version": plan["method"]["version"],
    "configuration_sha256": plan_digest
    },
    "evidence": [{
    "evidence_ref": plan_ref,
    "role": "method_input",
    "source_anchor_refs": [source_anchor],
    "target_anchor_refs": [target_anchor],
    "description": "The tracked text-free plan freezes the exact selection and ordinal-pairing rule, selectors, digests, and authority limits."
    }, {
    "evidence_ref": plan["source"]["edition_reading_admission_ref"],
    "role": "qualification",
    "source_anchor_refs": [source_anchor],
    "target_anchor_refs": [],
    "description": "The source expression has bounded edition-reading admission. Sentence segmentation and alignment each require their own assessment."
    }, {
    "evidence_ref": plan["target"]["expression_record_ref"],
    "role": "support",
    "source_anchor_refs": [],
    "target_anchor_refs": [target_anchor],
    "description": "The target expression record establishes the independently modeled Russian expression identity."
    }, {
    "evidence_ref": plan["target"]["responsibility_claims_ref"],
    "role": "support",
    "source_anchor_refs": [source_anchor],
    "target_anchor_refs": [target_anchor],
    "description": "The responsibility record supplies the translated-by lineage; it does not prove this sentence correspondence or its quality."
    }, {
    "evidence_ref": plan["target"]["unresolved_spacing_annotation_ref"],
    "role": "qualification",
    "source_anchor_refs": [],
    "target_anchor_refs": [target_anchor],
    "description": "The target raw layer retains an unresolved embedded-text versus visual-spacing discrepancy; the proposal does not resolve it."
    }],
    "competing_alignment_refs": [],
    "review_refs": []
    }],
    "reviews": [],
    "projections": [],
    "rights_and_visibility": {
    "source_visibility": "local_only",
    "target_visibility": "local_only",
    "packet_visibility": "public_metadata_only",
    "effective_visibility": "local_only",
    "rights_record_refs": [plan["source"]["rights_ref"], plan["target"]["rights_ref"]],
    "private_source_used": true,
    "publication_authorized": false,
    "inheritance_policy": "most_restrictive_side_or_packet_wins"
    },
    "authority_boundary": {
    "tree_role": "orientation",
    "graph_role": "relation",
    "source_role": "authority",
    "validators_prove_mechanics_not_truth": true,
    "alignment_is_claim_not_translation_truth": true,
    "machine_or_model_may_propose_not_accept": true,
    "exports_are_not_owner_truth": true,
    "unaligned_members_are_first_class": true,
    "lexical_equivalence_not_inferred": true,
    "canon_effect": false
    }
    })
}
pub(super) struct Provenance<'a> {
    pub plan: &'a Value,
    pub plan_ref: &'a str,
    pub plan_digest: &'a str,
    pub event_id: &'a str,
    pub builder_ref: &'a str,
    pub builder_digest: &'a str,
    pub executable_digest: &'a str,
    pub argv: &'a Value,
    pub argv_digest: &'a str,
    pub inputs: &'a Vec<Value>,
    pub outputs: &'a Vec<Value>,
    pub derivations: &'a Vec<Value>,
    pub made_at: &'a str,
}
pub(super) fn provenance(v: Provenance<'_>) -> Value {
    let Provenance {
        plan,
        plan_ref,
        plan_digest,
        event_id,
        builder_ref,
        builder_digest,
        executable_digest,
        argv,
        argv_digest,
        inputs,
        outputs,
        derivations,
        made_at,
    } = v;
    let created_at = made_at;
    let runtime_name = "Tree-of-Sophia native/Rust";
    let runtime_version = env!("CARGO_PKG_VERSION");
    let uv = std::char::UNICODE_VERSION;
    let unicode_version = format!("{}.{}.{}", uv.0, uv.1, uv.2);
    json!({
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
    "event_type": "alignment",
    "started_at": created_at,
    "ended_at": created_at,
    "status": "completed_with_warnings",
    "terminal_reason": null,
    "exit_code": 0,
    "warnings": ["Both sentence segmentations and their one-to-one mapping are unreviewed proposals.", "The target layer retains an unresolved embedded-text versus visual-spacing discrepancy.", "Translation fidelity, lexical equivalence, semantics, graph assessment, canon and publication retain their corresponding owner decision routes.", "The unsigned self-recorded event proves mechanics and closure, not execution or content truth."]
    },
    "entities": {
    "inputs": inputs,
    "outputs": outputs,
    "byproducts": []
    },
    "derivations": derivations,
    "responsibility": [{
    "agent_ref": "software:tos-zarathustra-opening-sentence-alignment-builder",
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
    "name": plan["method"]["name"],
    "version": plan["method"]["version"],
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
    "name": "Tree of Sophia opening-sentence alignment builder",
    "version": plan["method"]["version"],
    "role": "sentence-selection-and-alignment-proposal-builder",
    "artifact_ref": builder_ref,
    "artifact_sha256": builder_digest,
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
    "backend": "rust-exact-code-point-selection",
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
    "statement": "No manual change was applied to either private source span; the builder emits only selectors, digests, identities, provenance, and closed authority."
    },
    "measurements": [{
    "metric": "input_bytes",
    "status": "measured",
    "value": inputs.iter().map(|row|row["size_bytes"].as_u64().unwrap_or(0)).sum::<u64>(),
    "unit": "bytes",
    "method": "sum of exact fixity-bound input entity byte counts",
    "evidence_binding": null
    }, {
    "metric": "output_bytes",
    "status": "measured",
    "value": outputs.iter().map(|row|row["size_bytes"].as_u64().unwrap_or(0)).sum::<u64>(),
    "unit": "bytes",
    "method": "sum of exact serialized tracked output byte counts",
    "evidence_binding": null
    }, {
    "metric": "human_active_seconds",
    "status": "not_applicable",
    "value": null,
    "unit": null,
    "method": "no human content review or adjudication was scheduled or performed",
    "evidence_binding": null
    }],
    "evidence_authentication": {
    "capture_posture": "tool_captured",
    "signature_status": "unsigned",
    "signature_bindings": [],
    "verification_status": "mechanically_verified",
    "producer_control_boundary": "The same local builder read the private layers and emitted this unsigned record; independent replay can verify bytes and selectors but cannot authenticate execution, language correctness, or translation fidelity."
    },
    "rights_and_visibility": {
    "rights_record_bindings": [{
    "ref": plan["source"]["rights_ref"],
    "sha256": plan["source"]["rights_sha256"]
    }, {
    "ref": plan["target"]["rights_ref"],
    "sha256": plan["target"]["rights_sha256"]
    }],
    "intended_uses": ["local_research", "indexing"],
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
    "known_gaps": ["The receipt is unsigned and self-recorded by the transformation runner.", "Neither sentence segmentation nor their correspondence has human adjudication.", "The German machine layer remains unaccepted and the Russian OCR layer retains unresolved spacing.", "Both exact private paragraph layers are intentionally absent from Git."],
    "replay_scope": "Exact private layer digests, code-point selectors, first-full-stop guards, tracked input bindings, deterministic packet serialization, native executable digest, and fail-closed authority posture."
    },
    "authority_boundary": {
    "validator_role": "mechanics_and_closure_only_not_truth",
    "claims_not_established": ["execution_truth", "content_truth", "source_fidelity", "translation_quality", "semantic_correctness", "rights_clearance", "human_review", "publication_authority", "canon_authority"]
    }
    })
}
