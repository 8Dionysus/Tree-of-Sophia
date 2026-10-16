//! Record constructors preserve authored posture; measurements come from checked inputs.
use super::*;
pub(super) fn source_anchor(
    plan: &Value,
    plan_ref: &str,
    plan_digest: &str,
    event_id: &str,
) -> Result<Value> {
    let source = &plan["witness_2007"];
    let ids = &plan["opaque_ids"];
    Ok(
        json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json","schema_version":"tos_source_anchor_v2","anchor_id":ids["source_anchor_id"],"anchor_version":1,"supersedes_anchor_ref":null,"passage_id":ids["source_passage_id"],"target":{"item_id":source["item_ref"],"file_id":source["file_ref"],"file_sha256":source["file_sha256"],"media_type":"application/pdf"},"selector_payload":{"kind":"selector_expression","expression":{"mode":"single","selector":{"state":{"state_type":"digest_state","representation_ref":source["source_relative_ref"],"representation_sha256":source["file_sha256"],"media_type":"application/pdf","character_normalization":"none"},"selector":{"type":"page_region","page_identity":{"page_number":source["page_number"]},"x":0.0,"y":0.0,"width":source["page_width_points"],"height":source["page_height_points"],"coordinate_space":"points","source_width":source["page_width_points"],"source_height":source["page_height_points"]}}}},"publication_boundary":{"record_storage":"tracked","source_content_visibility":"local_only","source_text_in_record":false,"public_payload_expected":false},"selector_method":{"maker_type":"mixed","method":"frozen-sample-whole-page-region-binding","version":"1","configuration_ref":plan_ref,"configuration_digest":plan_digest},"resolution_status":"mechanically_resolved","review_status":"unreviewed","review_ref":null,"provenance_event_ref":event_id}),
    )
}

pub(super) fn source_layer(
    plan: &Value,
    plan_ref: &str,
    plan_digest: &str,
    anchor_digest: &str,
    observation_values: &Value,
    event_id: &str,
) -> Result<Value> {
    let source = &plan["witness_2007"];
    let ids = &plan["opaque_ids"];
    let ambiguity = s(&observation_values["source_damage_or_ambiguity"])?;
    Ok(
        json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json","schema_version":"tos_source_text_layer_v1","layer_id":ids["source_text_layer_id"],"layer_role":"diplomatic_transcription","source_binding":{"work_ref":source["work_ref"],"expression_ref":source["expression_ref"],"edition_ref":source["edition_ref"],"item_ref":source["item_ref"],"source_file_ref":source["file_ref"],"source_file_sha256":source["file_sha256"],"anchor_contract":"tos_source_anchor_v2","anchors":[{"anchor_id":ids["source_anchor_id"],"anchor_record_ref":plan["outputs"]["source_anchor_ref"],"anchor_record_sha256":anchor_digest}]},"representation":{"content_file_id":format!("tos.file.sha256.{}", s(&source["text_layer_sha256"])?),"content_ref":plan["outputs"]["private_text_ref"],"content_sha256":source["text_layer_sha256"],"media_type":"text/plain","charset":"UTF-8","language":source["language"],"text_scope":{"start":0,"end":source["text_layer_codepoints"],"position_unit":"unicode_code_point","interval":"half_open"},"character_normalization":"none","line_break_posture":"logical_reflow","storage":"ignored_local","content_visibility":"local_only","tracked_content":false,"publication_authorized":false,"rights_record_refs":[{"ref":source["rights_ref"],"sha256":source["rights_sha256"]}],"publication_authority_refs":[]},"derivation":{"method":"manual_transcription","input_layers":[],"maker":{"maker_type":"human","agent_ref":"human:dionysus","method":"blind-source-visible-workbench-transcription-observation","version":"1","configuration_ref":plan_ref,"configuration_digest":plan_digest},"preservation_goal":"source_near","loss_posture":"preservation_intended","silent_changes_allowed":false,"change_payload":{"kind":"none"}},"editorial_policy":{"policy_ref":plan_ref,"policy_sha256":plan_digest,"transcription_goal":"diplomatic","historical_language_preserved":true,"printing_errors_silently_corrected":false,"typography_posture":"preserve","layout_posture":"logical_reflow","unicode_normalization":"none","uncertainty_representation":"explicit-never-silent","method_declared":true},"uncertainty":{"status":"unresolved","annotations":[{"annotation_id":"tos.annotation.antonovsky-2007-p011-human-ambiguity.sid-8846af42fb4d0b3b066b42e209f99e03","anchor_ref":ids["source_anchor_id"],"kind":"ambiguous_reading","alternatives":[{"value_sha256":sha(ambiguity.as_bytes()),"value_in_record":false,"value":null,"status":"possible"}],"resolution":"unresolved"}]},"admission":{"mechanical_status":"fixity_verified","review_status":"unreviewed","review_ref":null,"human_review_performed":false,"human_language_competence":"not_assessed","language_competence_evidence_refs":[],"accepted_uses":[],"automatic_validation_complete":true,"model_output_is_ground_truth":false,"validator_proves_content_truth":false,"routine_human_task_created":false,"promotion_authorized":false},"provenance_event_ref":event_id,"authority_boundary":crate::source_text_foundation::LEGACY_LAYER_AUTHORITY,"layer_version":1,"supersedes_layer_ref":null}),
    )
}

fn unit_method(plan: &Value, plan_ref: &str, builder: &str, event_id: &str) -> Value {
    json!({"maker_kind":"software","agent_ref":SOFTWARE_AGENT,"method_name":"first-non-whitespace-after-first-full-stop-through-second-full-stop","method_version":"1","software_refs":[builder],"model_ref":null,"configuration_ref":plan_ref,"locale":"ru","unicode_version":"16.0.0","unicode_revision":null,"tailoring_ref":null,"provenance_event_ref":event_id,"made_at":plan["created_at"],"output_posture":"method_result_not_source_or_linguistic_truth"})
}
fn unit_anchor(
    source: &Value,
    private_ref: &Value,
    reference: &Value,
    ordinal: usize,
    role: &str,
    start: usize,
    end: usize,
    value: &str,
) -> Value {
    json!({"anchor_ref":reference,"ordinal":ordinal,"text_layer_ref":private_ref,"text_layer_sha256":source["text_layer_sha256"],"selector":{"type":"text_position","start":start,"end":end,"position_unit":"unicode_code_point","interval":"half_open"},"exact_sha256":sha(value.as_bytes()),"anchor_role":role,"source_return":{"required":true,"locator_ref":private_ref}})
}
pub(super) fn source_unit_packet(
    plan: &Value,
    plan_ref: &str,
    builder: &str,
    event_id: &str,
    text: &str,
    heading: &str,
    interstitial: &str,
    sentence: &str,
    remainder: &str,
) -> Result<Value> {
    let source = &plan["witness_2007"];
    let ids = &plan["opaque_ids"];
    let method = unit_method(plan, plan_ref, builder, event_id);
    let private_ref = &plan["outputs"]["private_text_ref"];
    Ok(
        json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json","schema_version":"tos_source_text_unit_packet_v1","packet_id":ids["source_unit_packet_id"],"packet_version":1,"supersedes_packet_ref":null,"content_posture":"source_bound","source_scope":{"work_ref":source["work_ref"],"expression_ref":source["expression_ref"],"edition_ref":source["edition_ref"],"item_ref":source["item_ref"],"file_ref":source["file_ref"],"file_sha256":source["file_sha256"]},"source_layer":{"text_layer_ref":private_ref,"text_layer_sha256":source["text_layer_sha256"],"language":source["language"],"media_type":"text/plain; charset=utf-8","unicode_form":"source_preserved","position_unit":"unicode_code_point","interval":"half_open","immutable":true,"visibility":"local_only","publication_authorized":false},"schemes":[{"scheme_id":ids["source_unit_scheme_id"],"scheme_version":1,"supersedes_scheme_ref":null,"identity_policy":"opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis","scheme_name":"ru first-prose-sentence boundary proposal","analysis_role":"orthographic","boundary_basis":"rule_based","unit_kinds":["sentence"],"method":method,"policies":{"normalization":"no-text-mutation-separate-successor-layer","punctuation":"included_in_neighbor","whitespace":"declared_excluded","line_break":"declared_excluded","hyphenation":"preserve_source","unreported_gaps_allowed":false,"overlap":"forbid"},"authority_limit":{"algorithmic_output_is_source_truth":false,"algorithmic_output_is_linguistic_truth":false,"model_subword_is_lexeme":false,"unit_identity_is_semantic_identity":false,"text_mutation_allowed":false}}],"anchors":[unit_anchor(source, &private_ref, &ids["source_scope_anchor_id"], 1, "scope", 0, text.chars().count(), text),unit_anchor(source, &private_ref, &ids["source_heading_anchor_id"], 2, "gap", n(&source["heading_start"])?, n(&source["heading_end"])?, heading),unit_anchor(source, &private_ref, &ids["source_interstitial_anchor_id"], 3, "whitespace", n(&source["interstitial_start"])?, n(&source["interstitial_end"])?, interstitial),unit_anchor(source, &private_ref, &ids["source_sentence_anchor_id"], 4, "content", n(&source["sentence_start"])?, n(&source["sentence_end"])?, sentence),unit_anchor(source, &private_ref, &ids["source_remainder_anchor_id"], 5, "gap", n(&source["remainder_start"])?, n(&source["remainder_end"])?, remainder)],"units":[{"unit_id":ids["source_sentence_unit_id"],"unit_version":1,"supersedes_unit_ref":null,"identity_policy":"opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis","unit_kind":"sentence","surface_posture":"source_bearing","continuity":"contiguous","ordered_anchor_refs":[ids["source_sentence_anchor_id"]],"parent_unit_refs":[],"ordered_child_unit_refs":[],"boundary_posture":"method_proposed","certainty":{"value":0.5,"meaning":"maker-declared-boundary-confidence-not-truth-probability"},"status_reason":"A deterministic punctuation-and-whitespace rule selects the first prose span after the page heading and section marker. The boundary and underlying unattested observation remain unreviewed.","source_text_mutated":false,"semantic_promotion":false}],"segmentations":[{"segmentation_id":ids["source_segmentation_id"],"segmentation_version":1,"supersedes_segmentation_ref":null,"identity_policy":"opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries","scheme_ref":ids["source_unit_scheme_id"],"ordered_unit_refs":[ids["source_sentence_unit_id"]],"coverage":{"scope_anchor_ref":ids["source_scope_anchor_id"],"coverage_posture":"declared_partial","excluded_anchor_refs":[ids["source_heading_anchor_id"],ids["source_interstitial_anchor_id"],ids["source_remainder_anchor_id"]],"unreported_gaps_allowed":false,"overlap_requires_declaration":true,"source_reconstruction_required":true},"status":"proposed","status_reason":"Only one first-prose-sentence candidate is selected; heading, interstitial whitespace, and exact remainder are digest-bound as excluded coverage. No completion attestation or linguistic review exists.","maker":method,"competing_segmentation_refs":[],"review_refs":[],"declared_uses":["source_observation"],"source_text_authority":false,"linguistic_authority":false,"semantic_authority":false}],"reviews":[],"projections":[],"rights_and_visibility":{"source_visibility":"local_only","packet_visibility":"public_metadata_only","effective_visibility":"local_only","rights_record_refs":[source["rights_ref"]],"private_source_used":true,"publication_authorized":false,"inheritance_policy":"most-restrictive-source-packet-and-destination-wins"},"authority_boundary":{"tree_role":"orientation","graph_role":"relation","source_role":"authority","validators_prove_mechanics_not_truth":true,"segmentation_is_method_result_not_source_truth":true,"source_unit_is_not_lexeme_sign_or_concept":true,"model_token_is_not_linguistic_token":true,"projection_is_owner_truth":false,"legacy_bulk_migration_authorized":false}}),
    )
}

fn witness(
    witness_id: &Value,
    ordinal: usize,
    row: &Value,
    text_layer_ref: &Value,
    text_layer_sha256: &Value,
    unit_ref: &Value,
    unit_sha256: &Value,
    anchor_ref: &Value,
    start: usize,
    end: usize,
    exact_sha256: &Value,
    source_return_ref: &Value,
) -> Value {
    json!({"witness_id":witness_id,"ordinal":ordinal,"work_ref":row["work_ref"],"expression_ref":row["expression_ref"],"edition_ref":row["edition_ref"],"item_ref":row["item_ref"],"file_ref":row["file_ref"],"file_sha256":row["file_sha256"],"text_layer_ref":text_layer_ref,"text_layer_sha256":text_layer_sha256,"text_unit_packet":{"ref":unit_ref,"sha256":unit_sha256},"language":row["language"],"anchor_ref":anchor_ref,"selector":{"type":"text_position","start":start,"end":end,"position_unit":"unicode_code_point","interval":"half_open"},"exact_sha256":exact_sha256,"source_return_ref":source_return_ref,"source_text_in_record":false,"rights_refs":[row["rights_ref"]],"visibility":"local_only","publication_authorized":false})
}
pub(super) fn collation_packet(
    plan: &Value,
    plan_ref: &str,
    plan_digest: &str,
    builder: &str,
    event_id: &str,
    source_unit_digest: &str,
    private_detail_digest: &str,
    public_views: &[Value],
) -> Result<Value> {
    let source = &plan["witness_2007"];
    let target = &plan["witness_1911"];
    let ids = &plan["opaque_ids"];
    let source_witness_id = &ids["witness_2007_id"];
    let target_witness_id = &ids["witness_1911_id"];
    let source_private_ref = &plan["outputs"]["private_text_ref"];
    let source_unit_ref = &plan["outputs"]["source_text_unit_packet_ref"];
    let detail_ref = &plan["outputs"]["private_detail_ref"];
    let artifact = &plan["human_observation"];
    Ok(
        json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/witness-text-collation-packet-v1.schema.json","schema_version":"tos_witness_text_collation_packet_v1","packet_id":ids["collation_packet_id"],"packet_version":1,"supersedes_packet_ref":null,"content_posture":"source_bound","granularity":"sentence","witnesses":[witness(&source_witness_id, 1, source, &source_private_ref, &source["text_layer_sha256"], &source_unit_ref, &json!(source_unit_digest), &ids["source_sentence_anchor_id"], n(&source["sentence_start"])?, n(&source["sentence_end"])?, &source["sentence_sha256"], &source_private_ref),witness(&target_witness_id, 2, target, &target["text_layer_ref"], &target["text_layer_sha256"], &target["text_unit_packet_ref"], &json!(target["text_unit_packet_sha256"]), &target["anchor_ref"], n(&target["sentence_start"])?, n(&target["sentence_end"])?, &target["sentence_sha256"], &target["text_layer_ref"])],"collations":[{"collation_id":ids["collation_id"],"collation_version":1,"supersedes_collation_ref":null,"identity_policy":"opaque-id-independent-of-text-label-offset-score-algorithm-and-current-correspondence","claim_id":ids["collation_claim_id"],"claim_version":1,"supersedes_claim_ref":null,"direction":"symmetric_comparison","ordered_witness_refs":[source_witness_id,target_witness_id],"relation_scope":"same_work","correspondence_shape":"one_to_one","correspondence_basis":"ordinal_and_surface_similarity","status":"proposed","status_reason":"The first prose sentence after the shared opening section marker has high character-level surface similarity in four declared views. The 2007 observation is unattested, both boundaries are unreviewed, and no textual equivalence or edition derivation is established.","comparison_method":{"algorithm":plan["comparison_method"]["name"],"algorithm_version":plan["comparison_method"]["version"],"tokenization":"unicode_code_point","autojunk":false,"source_text_mutated":false,"base_or_lemma_selected":false},"comparison_views":public_views,"private_detail":{"ref":detail_ref,"sha256":private_detail_digest,"tracked":false,"visibility":"local_only","reconstructive_detail":true},"maker":{"maker_kind":"software","agent_ref":SOFTWARE_AGENT,"made_at":plan["created_at"],"method":plan["comparison_method"]["name"],"method_version":plan["comparison_method"]["version"],"software_ref":builder,"configuration_sha256":plan_digest,"provenance_event_ref":event_id,"output_posture":"proposal_not_textual_truth"},"evidence":[{"evidence_ref":plan_ref,"sha256":plan_digest,"role":"method_input","witness_refs":[source_witness_id,target_witness_id],"authority_effect":"supports_proposal"},{"evidence_ref":format!("abyss-stack:artifact/{}", s(&artifact["autosave_relative_path"])?),"sha256":artifact["autosave_sha256"],"role":"source_observation","witness_refs":[source_witness_id],"authority_effect":"none"},{"evidence_ref":format!("abyss-stack:artifact/{}", s(&artifact["closure_relative_path"])?),"sha256":artifact["closure_sha256"],"role":"qualification","witness_refs":[source_witness_id],"authority_effect":"none"},{"evidence_ref":target["text_unit_packet_ref"],"sha256":target["text_unit_packet_sha256"],"role":"support","witness_refs":[target_witness_id],"authority_effect":"supports_proposal"},{"evidence_ref":source["rights_ref"],"sha256":source["rights_sha256"],"role":"rights","witness_refs":[source_witness_id],"authority_effect":"none"},{"evidence_ref":target["rights_ref"],"sha256":target["rights_sha256"],"role":"rights","witness_refs":[target_witness_id],"authority_effect":"none"}],"competing_collation_refs":[],"review_refs":[],"interpretive_boundary":{"preferred_reading_selected":false,"textual_equivalence_established":false,"expression_derivation_established":false,"translation_relation_inferred":false,"lexical_equivalence_inferred":false,"semantic_relation_inferred":false}}],"reviews":[],"projections":[],"rights_and_visibility":{"witness_visibility":"local_only","packet_visibility":"public_metadata_only","effective_visibility":"local_only","rights_record_refs":[source["rights_ref"],target["rights_ref"]],"private_source_used":true,"publication_authorized":false,"inheritance_policy":"most-restrictive-witness-packet-detail-and-destination-wins"},"authority_boundary":{"tree_role":"orientation","graph_role":"relation","source_role":"authority","validators_prove_mechanics_not_truth":true,"collation_is_claim_not_textual_truth":true,"machine_or_model_may_propose_not_accept":true,"human_observation_is_not_human_review":true,"projection_is_not_owner_truth":true,"preferred_reading_not_inferred":true,"equivalence_not_inferred":true,"derivation_not_inferred":true,"translation_not_inferred":true,"semantics_not_inferred":true,"canon_effect":false}}),
    )
}

pub(super) fn private_detail(
    plan: &Value,
    observation_values: &Value,
    private_views: &[Value],
) -> Value {
    json!({"schema_version":"tos_private_witness_text_collation_detail_v1","visibility":"local_only","publication_authorized":false,"sample_id":plan["human_observation"]["sample_id"],"human_observation":{"reviewer_ref":plan["human_observation"]["reviewer_ref"],"active_seconds":plan["human_observation"]["expected_active_seconds"],"attestation_status":"not_collected","pass_receipt_collected":false,"promotion_authorized":false,"values":observation_values},"witnesses":[{"ordinal":1,"text_layer_sha256":plan["witness_2007"]["text_layer_sha256"],"selector":[plan["witness_2007"]["sentence_start"],plan["witness_2007"]["sentence_end"]],"sentence_sha256":plan["witness_2007"]["sentence_sha256"]},{"ordinal":2,"text_layer_sha256":plan["witness_1911"]["text_layer_sha256"],"selector":[plan["witness_1911"]["sentence_start"],plan["witness_1911"]["sentence_end"]],"sentence_sha256":plan["witness_1911"]["sentence_sha256"]}],"comparison_views":private_views,"authority_boundary":"local reconstructive detail for an unattested observation and proposed collation; not gold, review, source acceptance, textual equivalence, Expression derivation, translation judgment, semantics, or publication"})
}
