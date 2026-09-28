//! The maintained pure initial owner TextLayer byte recipe. The caller has
//! already verified owner grants, rights, source, File and selected member;
//! these bytes still have no publication or admission authority.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_text_layer_native::PreparedInitialLayerText;
use crate::source_text_owner::OwnerTextInitialLayerSelection;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tos_foundation::{Digest256, JsonValue};

const LAYER_SCHEMA: &str =
    "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json";
const ANCHOR_SCHEMA: &str =
    "https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json";
const AUTHORITY_BOUNDARY: &str = "A source text layer preserves one immutable, source-returnable representation and its derivation, uncertainty, assessment, competence, rights and use scope.";
const MAX_RECORD_BYTES: usize = 1_048_576;
const MAX_PACKAGE_BYTES: usize = 12 * 1024 * 1024;

pub(crate) struct InitialLayerOutput {
    pub(crate) layer: JsonValue,
    pub(crate) anchor: JsonValue,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
}

fn json(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("native TextLayer JSON conversion"))
}

fn source_value(value: &Value) -> SourceCommandResult<JsonValue> {
    let raw = serde_json::to_vec(value)
        .map_err(|_| SourceCommandError::Invalid("native TextLayer output JSON"))?;
    cmd::parse(&raw)
}

fn line(value: &Value) -> SourceCommandResult<Vec<u8>> {
    let mut bytes = cmd::canonical(&source_value(value)?)?;
    if bytes
        .len()
        .checked_add(1)
        .is_none_or(|n| n > MAX_RECORD_BYTES)
    {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer metadata byte budget",
        ));
    }
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn build_initial_layer(
    grant: &OwnerTextInitialLayerSelection,
    prepared: &PreparedInitialLayerText,
) -> SourceCommandResult<InitialLayerOutput> {
    let config = json(&grant.config)?;
    let scope = &config["source_scope"];
    let ids = &config["identities"];
    let member = &config["member"];
    let selector = &config["selector"];
    let policy = &config["policy"];
    let language = &config["language"];
    let rights = &config["derivation_access"]["rights_record_refs"];
    let source_path = cmd::text(&grant.config, "source_path")?;
    let base = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "native TextLayer source package parent",
        ))?
        .0;
    let item_ref = cmd::text(cmd::field(&grant.config, "source_record_refs")?, "item")?;
    let item_parent = item_ref
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native TextLayer Item parent"))?
        .0;
    let payload_relative = cmd::text(&prepared.source.payload_entry, "relative_path")?;
    let configuration_line = line(&config)?;
    let config_sha = Digest256::of_bytes(&configuration_line).to_hex();
    let refs = json!({
        "layer_ref": source_path,
        "anchor_ref": format!("{base}/source-anchor.v2.json"),
        "content_ref": format!("{base}/content.txt"),
        "policy_ref": format!("{base}/extraction-policy.json"),
        "configuration_ref": format!("{base}/source-create-owner-configuration.json"),
        "configuration_sha256": config_sha,
        "source_payload_ref": format!("{item_parent}/{payload_relative}"),
    });
    let mut maker = config["maker"].clone();
    let maker_obj = maker
        .as_object_mut()
        .ok_or(SourceCommandError::Invalid("native TextLayer maker object"))?;
    maker_obj.insert(
        "configuration_ref".to_owned(),
        refs["configuration_ref"].clone(),
    );
    maker_obj.insert(
        "configuration_digest".to_owned(),
        refs["configuration_sha256"].clone(),
    );
    let mut selector_method = maker.clone();
    selector_method
        .as_object_mut()
        .ok_or(SourceCommandError::Invalid(
            "native TextLayer selector method",
        ))?
        .remove("agent_ref");
    let anchor = json!({
        "$schema": ANCHOR_SCHEMA,
        "schema_version": "tos_source_anchor_v2",
        "anchor_id": ids["anchor_id"],
        "anchor_version": 1,
        "passage_id": ids["passage_id"],
        "target": {"item_id": scope["item_ref"], "file_id": scope["file_ref"],
            "file_sha256": scope["file_sha256"], "media_type": "application/epub+zip"},
        "selector_payload": {"kind": "selector_expression", "expression": {
            "mode": "refinement_chain", "steps": [
                {"state": {"state_type": "digest_state", "representation_ref": refs["source_payload_ref"],
                    "representation_sha256": scope["file_sha256"], "media_type": "application/epub+zip"},
                 "selector": {"type": "container_member", "member_path": member["member_path"],
                    "member_sha256": member["member_sha256"], "member_media_type": "application/xhtml+xml"}},
                {"state": {"state_type": "digest_state", "representation_ref": member["member_path"],
                    "representation_sha256": member["member_sha256"], "media_type": "application/xhtml+xml"},
                 "selector": selector}
            ]}},
        "publication_boundary": {"record_storage": "ignored_local", "source_content_visibility": "local_only",
            "source_text_in_record": false, "public_payload_expected": false},
        "selector_method": selector_method,
        "resolution_status": "locator_only", "review_status": "unreviewed", "review_ref": null,
        "provenance_event_ref": ids["provenance_event_id"], "supersedes_anchor_ref": null,
    });
    let anchor_bytes = line(&anchor)?;
    let content = prepared.text.as_bytes();
    if content.is_empty()
        || content.len() > 8_388_608
        || content.contains(&b'\r')
        || content.contains(&b'&')
    {
        return Err(SourceCommandError::Invalid(
            "native TextLayer exact XHTML text",
        ));
    }
    let content_sha = Digest256::of_bytes(content).to_hex();
    let policy_bytes = line(policy)?;
    let layer = json!({
        "$schema": LAYER_SCHEMA,
        "schema_version": "tos_source_text_layer_v1",
        "layer_id": ids["layer_id"], "layer_version": 1, "supersedes_layer_ref": null,
        "layer_role": "machine_transcription",
        "source_binding": {"work_ref": scope["work_ref"], "expression_ref": scope["expression_ref"],
            "edition_ref": scope["edition_ref"], "item_ref": scope["item_ref"],
            "source_file_ref": scope["file_ref"], "source_file_sha256": scope["file_sha256"],
            "anchor_contract": "tos_source_anchor_v2", "anchors": [{"anchor_id": ids["anchor_id"],
                "anchor_record_ref": refs["anchor_ref"],
                "anchor_record_sha256": Digest256::of_bytes(&anchor_bytes).to_hex()}]},
        "representation": {"content_file_id": format!("tos.file.sha256.{content_sha}"),
            "content_ref": refs["content_ref"], "content_sha256": content_sha,
            "media_type": "text/plain", "charset": "UTF-8", "language": language,
            "text_scope": {"start": 0, "end": prepared.text.chars().count(),
                "position_unit": "unicode_code_point", "interval": "half_open"},
            "character_normalization": "none", "line_break_posture": "logical_reflow",
            "storage": "ignored_local", "content_visibility": "local_only", "tracked_content": false,
            "publication_authorized": false, "rights_record_refs": rights,
            "publication_authority_refs": []},
        "derivation": {"method": "structural_extraction", "input_layers": [], "maker": maker,
            "preservation_goal": "source_near", "loss_posture": "preservation_intended",
            "silent_changes_allowed": false, "change_payload": {"kind": "none"}},
        "editorial_policy": {"policy_ref": refs["policy_ref"],
            "policy_sha256": Digest256::of_bytes(&policy_bytes).to_hex(),
            "transcription_goal": "machine_candidate", "historical_language_preserved": false,
            "printing_errors_silently_corrected": false, "typography_posture": "normalize_declared",
            "layout_posture": "logical_reflow", "unicode_normalization": "none",
            "uncertainty_representation": "explicit-never-silent", "method_declared": true},
        "uncertainty": {"status": "none", "annotations": []},
        "admission": {"mechanical_status": "materialized", "review_status": "unreviewed", "review_ref": null,
            "human_review_performed": false, "human_language_competence": "not_assessed",
            "language_competence_evidence_refs": [], "accepted_uses": [],
            "automatic_validation_complete": false, "model_output_is_ground_truth": false,
            "validator_proves_content_truth": false, "routine_human_task_created": false,
            "promotion_authorized": false},
        "provenance_event_ref": ids["provenance_event_id"],
        "authority_boundary": AUTHORITY_BOUNDARY,
    });
    let layer_bytes = line(&layer)?;
    let total = layer_bytes
        .len()
        .checked_add(anchor_bytes.len())
        .and_then(|n| n.checked_add(policy_bytes.len()))
        .and_then(|n| n.checked_add(content.len()))
        .and_then(|n| n.checked_add(configuration_line.len()))
        .ok_or(SourceCommandError::Unsupported(
            "native TextLayer package byte overflow",
        ))?;
    if total > MAX_PACKAGE_BYTES {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer package byte budget",
        ));
    }
    let mut files = BTreeMap::new();
    files.insert("source-text-layer.v1.json".to_owned(), layer_bytes);
    files.insert("source-anchor.v2.json".to_owned(), anchor_bytes);
    files.insert("extraction-policy.json".to_owned(), policy_bytes);
    files.insert("content.txt".to_owned(), content.to_vec());
    files.insert(
        "source-create-owner-configuration.json".to_owned(),
        configuration_line,
    );
    Ok(InitialLayerOutput {
        layer: source_value(&layer)?,
        anchor: source_value(&anchor)?,
        files,
    })
}
