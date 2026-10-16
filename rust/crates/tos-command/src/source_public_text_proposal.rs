//! Pure literal public UTF-8 capture and first segmentation assembly.
//! The caller owns source authentication, rights/authority review, schemas,
//! file plans and publication. These builders perform no I/O or grants.
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};

pub(crate) struct PublicLayerProposal {
    pub(crate) layer: JsonValue,
    pub(crate) anchor: JsonValue,
    pub(crate) policy: JsonValue,
    pub(crate) content: Vec<u8>,
}
fn bad() -> SourceCommandError {
    SourceCommandError::Invalid("public literal UTF-8 proposal")
}
fn value(v: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(v)?).map_err(|_| bad())
}
fn foundation(v: &Value) -> SourceCommandResult<JsonValue> {
    cmd::parse(&serde_json::to_vec(v).map_err(|_| bad())?)
}
fn record(v: &Value) -> SourceCommandResult<Vec<u8>> {
    let mut bytes = cmd::canonical(&foundation(v)?)?;
    bytes.push(b'\n');
    if bytes.len() > 1_048_576 {
        return Err(bad());
    }
    Ok(bytes)
}
fn keys(v: &Value, names: &[&str]) -> SourceCommandResult<()> {
    let object = v.as_object().ok_or(bad())?;
    if object.len() != names.len() || names.iter().any(|n| !object.contains_key(*n)) {
        return Err(bad());
    }
    Ok(())
}
fn digest(v: &Value) -> SourceCommandResult<&str> {
    let s = v.as_str().ok_or(bad())?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad());
    }
    Ok(s)
}
fn nonempty(v: &Value) -> SourceCommandResult<&str> {
    let s = v.as_str().ok_or(bad())?;
    if s.trim().is_empty() {
        return Err(bad());
    }
    Ok(s)
}
fn reference(v: &Value) -> SourceCommandResult<&str> {
    let s = nonempty(v)?;
    if s.chars().count() > 4096
        || s.starts_with(['/', '~'])
        || s.contains(['\\', ':'])
        || s.chars().any(|c| (c as u32) < 32)
        || s.split('/').any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(bad());
    }
    Ok(s)
}
fn identity(v: &Value, kind: &str, opaque: bool) -> SourceCommandResult<()> {
    let s = v.as_str().ok_or(bad())?;
    let tail = s.strip_prefix(&format!("tos.{kind}.")).ok_or(bad())?;
    if kind == "file" {
        digest(&json!(tail.strip_prefix("sha256.").ok_or(bad())?))?;
    } else if opaque {
        let hex = tail.strip_prefix("sid-").ok_or(bad())?;
        if hex.len() != 32
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(bad());
        }
    } else if tail.is_empty()
        || !tail.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
    {
        return Err(bad());
    }
    Ok(())
}
fn evidence(v: &Value) -> SourceCommandResult<()> {
    let rows = v.as_array().ok_or(bad())?;
    if rows.is_empty() || rows.len() > 16 {
        return Err(bad());
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        keys(row, &["ref", "sha256"])?;
        if !seen.insert(reference(&row["ref"])?) {
            return Err(bad());
        }
        digest(&row["sha256"])?;
    }
    Ok(())
}

pub(crate) fn build_public_layer(
    original_utf8: &str,
    configuration: &JsonValue,
    refs: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PublicLayerProposal> {
    crate::source_creation_store::active(deadline, cancelled)?;
    let config = value(configuration)?;
    let refs = value(refs)?;
    let scope = &config["source_scope"];
    let ids = &config["identities"];
    let selector = &config["source"]["selector"];
    keys(
        scope,
        &[
            "work_ref",
            "expression_ref",
            "edition_ref",
            "item_ref",
            "file_ref",
            "file_sha256",
        ],
    )?;
    keys(selector, &["start", "end"])?;
    keys(
        &refs,
        &[
            "layer_ref",
            "anchor_ref",
            "content_ref",
            "policy_ref",
            "configuration_ref",
            "configuration_sha256",
            "source_ref",
        ],
    )?;
    for (name, v) in refs.as_object().ok_or(bad())? {
        if name == "configuration_sha256" {
            digest(v)?;
        } else {
            reference(v)?;
        }
    }
    let start = selector["start"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(bad())?;
    let end = selector["end"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(bad())?;
    let media = nonempty(&config["source"]["media_type"])?;
    let original_sha = Digest256::of_bytes(original_utf8.as_bytes()).to_hex();
    if original_utf8.is_empty()
        || original_utf8.len() > 131072
        || start >= end
        || !["text/plain", "text/markdown"].contains(&media)
        || scope["file_sha256"] != original_sha
        || scope["file_ref"] != format!("tos.file.sha256.{original_sha}")
    {
        return Err(bad());
    }
    for kind in ["work", "expression", "edition", "item", "file"] {
        identity(&scope[format!("{kind}_ref")], kind, false)?;
    }
    for (key, kind) in [
        ("layer_id", "text-layer"),
        ("anchor_id", "anchor"),
        ("passage_id", "passage"),
        ("provenance_event_id", "event"),
    ] {
        identity(&ids[key], kind, true)?;
    }
    nonempty(&config["principal_id"])?;
    let rights = &config["rights_record_refs"];
    let authorities = json!([config["publication_authority"]]);
    evidence(rights)?;
    evidence(&authorities)?;
    // Convert code-point coordinates once; never normalize or rewrite bytes.
    let mut left = None;
    let mut right = None;
    let mut count = 0;
    for (ordinal, (byte, _)) in original_utf8.char_indices().enumerate() {
        if ordinal == start {
            left = Some(byte);
        }
        if ordinal == end {
            right = Some(byte);
        }
        count = ordinal + 1;
    }
    if end == count {
        right = Some(original_utf8.len());
    }
    let text = original_utf8
        .get(left.ok_or(bad())?..right.ok_or(bad())?)
        .ok_or(bad())?;
    let content = text.as_bytes().to_vec();
    let content_sha = Digest256::of_bytes(&content).to_hex();
    let policy = json!({"schema_version":"tos_project_utf8_range_policy_v1","method":"tos.project-authored.utf8-range.v1",
        "encoding":"UTF-8-strict","position_unit":"unicode_code_point","interval":"half_open","unicode_normalization":"none",
        "whitespace":"preserve-including-CRLF-and-leading-or-trailing-space","markup":"literal-source-characters-not-rendered",
        "selection":"one-exact-contiguous-range-no-rewrite","source_posture":"independently-authorized-project-authored-text","quality_assessment":"not-performed"});
    let maker = json!({"maker_type":"software","agent_ref":config["principal_id"],"method":policy["method"],"version":"1",
        "configuration_ref":refs["configuration_ref"],"configuration_digest":refs["configuration_sha256"]});
    let mut selector_method = maker.clone();
    selector_method
        .as_object_mut()
        .ok_or(bad())?
        .remove("agent_ref");
    let anchor = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json",
        "schema_version":"tos_source_anchor_v2","anchor_id":ids["anchor_id"],"anchor_version":1,"passage_id":ids["passage_id"],
        "target":{"item_id":scope["item_ref"],"file_id":scope["file_ref"],"file_sha256":scope["file_sha256"],"media_type":media},
        "selector_payload":{"kind":"selector_expression","expression":{"mode":"single","selector":{
            "state":{"state_type":"digest_state","representation_ref":refs["source_ref"],"representation_sha256":scope["file_sha256"],"media_type":media,"character_normalization":"none"},
            "selector":{"type":"text_position","start":start,"end":end,"position_unit":"unicode_code_point","interval":"half_open"}}}},
        "publication_boundary":{"record_storage":"tracked","source_content_visibility":"public","source_text_in_record":false,"public_payload_expected":false},
        "selector_method":selector_method,"resolution_status":"locator_only","review_status":"unreviewed","review_ref":null,
        "provenance_event_ref":ids["provenance_event_id"],"supersedes_anchor_ref":null});
    let layer = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json",
        "schema_version":"tos_source_text_layer_v1","layer_id":ids["layer_id"],"layer_version":1,"supersedes_layer_ref":null,"layer_role":"machine_transcription",
        "source_binding":{"work_ref":scope["work_ref"],"expression_ref":scope["expression_ref"],"edition_ref":scope["edition_ref"],"item_ref":scope["item_ref"],
            "source_file_ref":scope["file_ref"],"source_file_sha256":scope["file_sha256"],"anchor_contract":"tos_source_anchor_v2",
            "anchors":[{"anchor_id":ids["anchor_id"],"anchor_record_ref":refs["anchor_ref"],"anchor_record_sha256":Digest256::of_bytes(&record(&anchor)?).to_hex()}]},
        "representation":{"content_file_id":format!("tos.file.sha256.{content_sha}"),"content_ref":refs["content_ref"],"content_sha256":content_sha,
            "media_type":"text/plain","charset":"UTF-8","language":config["language"],"text_scope":{"start":0,"end":text.chars().count(),"position_unit":"unicode_code_point","interval":"half_open"},
            "character_normalization":"none","line_break_posture":"source_preserved","storage":"tracked","content_visibility":"public","tracked_content":true,"publication_authorized":true,
            "rights_record_refs":rights,"publication_authority_refs":authorities},
        "derivation":{"method":"structural_extraction","input_layers":[],"maker":maker,"preservation_goal":"source_near","loss_posture":"preservation_intended","silent_changes_allowed":false,"change_payload":{"kind":"none"}},
        "editorial_policy":{"policy_ref":refs["policy_ref"],"policy_sha256":Digest256::of_bytes(&record(&policy)?).to_hex(),"transcription_goal":"machine_candidate","historical_language_preserved":false,
            "printing_errors_silently_corrected":false,"typography_posture":"preserve","layout_posture":"preserve","unicode_normalization":"none","uncertainty_representation":"explicit-never-silent","method_declared":true},
        "uncertainty":{"status":"none","annotations":[]},"admission":{"mechanical_status":"materialized","review_status":"unreviewed","review_ref":null,"human_review_performed":false,
            "human_language_competence":"not_assessed","language_competence_evidence_refs":[],"accepted_uses":[],"automatic_validation_complete":false,"model_output_is_ground_truth":false,"validator_proves_content_truth":false,"routine_human_task_created":false,"promotion_authorized":false},
        "provenance_event_ref":ids["provenance_event_id"],"authority_boundary":"A source text layer preserves one immutable, source-returnable representation and its derivation, uncertainty, assessment, competence, rights and use scope."});
    record(&layer)?;
    crate::source_creation_store::active(deadline, cancelled)?;
    Ok(PublicLayerProposal {
        layer: foundation(&layer)?,
        anchor: foundation(&anchor)?,
        policy: foundation(&policy)?,
        content,
    })
}

pub(crate) fn build_public_text_unit_packet(
    layer: &JsonValue,
    binding: &JsonValue,
    text: &str,
    configuration: &JsonValue,
    request: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, Vec<u8>)> {
    let declared = value(layer)?;
    let rep = &declared["representation"];
    if rep["content_visibility"] != "public"
        || rep["publication_authorized"] != true
        || rep["tracked_content"] != true
        || rep["storage"] != "tracked"
        || rep["publication_authority_refs"]
            .as_array()
            .is_none_or(Vec::is_empty)
    {
        return Err(bad());
    }
    let (packet, _) = crate::source_text_unit_proposal::build_text_unit_packet(
        None,
        layer,
        binding,
        text,
        configuration,
        request,
        deadline,
        cancelled,
    )?;
    let mut packet = value(&packet)?;
    let rights = packet["rights_and_visibility"]
        .as_object_mut()
        .ok_or(bad())?;
    for name in [
        "source_visibility",
        "packet_visibility",
        "effective_visibility",
    ] {
        rights.insert(name.into(), json!("public"));
    }
    rights.insert("publication_authorized".into(), json!(true));
    rights.insert("private_source_used".into(), json!(false));
    let bytes = record(&packet)?;
    Ok((foundation(&packet)?, bytes))
}
