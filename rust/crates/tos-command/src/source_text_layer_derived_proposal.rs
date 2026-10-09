//! Exact additive TextLayer byte construction after the owner has separately
//! verified current source, predecessor or supplied material, rights and IDs.
//! These buffers are proposals until the protected publisher commits them.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_text_layer_normalize::normalize_whole_predecessor;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};

const MAX_TEXT: usize = 131_072;
const MAX_EDITS: usize = 128;
const MAX_RECORD: usize = 1_048_576;
const LAYER_SCHEMA: &str =
    "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json";
const AUTHORITY: &str = "A source text layer preserves one immutable, source-returnable representation and its derivation, uncertainty, assessment, competence, rights and use scope.";

pub(crate) struct DerivedLayerOutput {
    pub(crate) layer: JsonValue,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
}

fn bad() -> SourceCommandError {
    SourceCommandError::Invalid("native TextLayer derived byte recipe")
}
fn serde(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?).map_err(|_| bad())
}
fn line(value: &Value) -> SourceCommandResult<Vec<u8>> {
    let raw = serde_json::to_vec(value).map_err(|_| bad())?;
    let mut out = cmd::canonical(&cmd::parse(&raw)?)?;
    if out.len().checked_add(1).is_none_or(|n| n > MAX_RECORD) {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer derived metadata budget",
        ));
    }
    out.push(b'\n');
    Ok(out)
}
fn bounded(text: &str) -> SourceCommandResult<()> {
    if text.is_empty() || text.len() > MAX_TEXT || text.contains('\0') {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer derived UTF-8 budget",
        ));
    }
    Ok(())
}
fn span(start: usize, end: usize) -> Value {
    json!({"start":start,"end":end,"position_unit":"unicode_code_point","interval":"half_open"})
}

fn policy(operation: &str, form: &str, transcription: &str) -> SourceCommandResult<Value> {
    let supplied = matches!(
        operation,
        "text-layer.record-transcription"
            | "text-layer.record-ocr"
            | "text-layer.record-owner-ocr"
            | "text-layer.record-owner-page-ocr"
    );
    let observed = matches!(
        operation,
        "text-layer.record-owner-ocr" | "text-layer.record-owner-page-ocr"
    );
    let normalize = operation == "text-layer.normalize";
    if !matches!(
        operation,
        "text-layer.correct"
            | "text-layer.normalize"
            | "text-layer.record-transcription"
            | "text-layer.record-ocr"
            | "text-layer.record-owner-ocr"
            | "text-layer.record-owner-page-ocr"
    ) || normalize && !matches!(form, "NFC" | "NFD" | "NFKC" | "NFKD")
        || !normalize && form != "none"
        || operation == "text-layer.record-transcription"
            && !matches!(
                transcription,
                "manual_transcription" | "model_transcription"
            )
    {
        return Err(bad());
    }
    let method = if operation == "text-layer.record-transcription" {
        transcription
    } else if observed {
        "ocr"
    } else {
        match operation {
            "text-layer.correct" => "correction",
            "text-layer.normalize" => "unicode_normalization",
            "text-layer.record-ocr" => "ocr",
            _ => return Err(bad()),
        }
    };
    Ok(
        json!({"schema_version":"tos_native_text_layer_derivation_policy_v1",
        "operation":operation,"method":method,"encoding":"UTF-8-strict",
        "text_max_bytes":MAX_TEXT,"edits_max_count":MAX_EDITS,
        "input_scope":if supplied {"exact-source-anchor"} else {"whole-exact-representation"},
        "unicode_normalization":form,
        "unicode_database_version":if normalize {Some("16.0.0")} else {None},
        "edits":if supplied {"not-applicable"} else {"ordered-explicit-half-open-code-point-proposals-no-diff"},
        "whitespace":"unchanged-except-explicit-edits-or-selected-Unicode-form",
        "result_origin":if observed {"authenticated-owner-execution-receipt"}
            else if supplied {"supplied-result-not-provider-execution"}
            else if normalize {"executed-Unicode-transform"}
            else {"applied-supplied-edit-proposals"},
        "provider_execution_verified":observed,
        "source_layout_fidelity":"not-assessed","quality_assessment":"not-performed",
        "inherited_quality":"not-transferred",
        "uncertainty":if supplied {"supplied-none-recorded-is-not-reviewed-absence"}
            else {"source-annotations-retained-without-resolution"}}),
    )
}

fn edited(
    input: &str,
    edits: &[Value],
    maker: &Value,
    anchors: &[Value],
    normalize: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, Value)> {
    if edits.is_empty() || edits.len() > MAX_EDITS {
        return Err(bad());
    }
    let mut output = String::new();
    let mut operations = Vec::with_capacity(edits.len());
    let mut cursor = 0usize;
    let mut output_cursor = 0usize;
    let positions = input
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(input.len()))
        .collect::<Vec<_>>();
    for (index, edit) in edits.iter().enumerate() {
        active(deadline, cancelled)?;
        let fields = edit.as_object().ok_or(bad())?;
        if fields.len() != 7
            || [
                "start",
                "end",
                "input_exact",
                "input_sha256",
                "output_exact",
                "reason",
                "confidence",
            ]
            .iter()
            .any(|name| !fields.contains_key(*name))
        {
            return Err(bad());
        }
        let start = edit["start"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(bad())?;
        let end = edit["end"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(bad())?;
        if !(cursor <= start && start <= end && end < positions.len()) {
            return Err(bad());
        }
        let exact = edit["input_exact"].as_str().ok_or(bad())?;
        let replacement = edit["output_exact"].as_str().ok_or(bad())?;
        let before = &input[positions[start]..positions[end]];
        let reason = edit["reason"].as_str().ok_or(bad())?;
        let confidence = edit["confidence"].as_f64().ok_or(bad())?;
        if exact != before
            || edit["input_sha256"] != Digest256::of_bytes(exact.as_bytes()).to_hex()
            || !confidence.is_finite()
            || !(0.0..=1.0).contains(&confidence)
            || reason.is_empty()
            || reason.len() > 2048
            || replacement.contains('\0')
            || !normalize && replacement == exact
        {
            return Err(bad());
        }
        let unchanged = &input[positions[cursor]..positions[start]];
        let next = output
            .len()
            .checked_add(unchanged.len())
            .and_then(|n| n.checked_add(replacement.len()))
            .ok_or(bad())?;
        if next > MAX_TEXT {
            return Err(SourceCommandError::Unsupported(
                "native TextLayer edited output budget",
            ));
        }
        output.push_str(unchanged);
        output.push_str(replacement);
        output_cursor += unchanged.chars().count();
        let operation = if normalize {
            "unicode_normalization"
        } else if start == end {
            "insert"
        } else if replacement.is_empty() {
            "delete"
        } else {
            "replace"
        };
        let replacement_end = output_cursor + replacement.chars().count();
        operations.push(
            json!({"edit_id":format!("edit-{}",index+1),"operation":operation,
            "input_span":span(start,end),"output_span":span(output_cursor,replacement_end),
            "input_exact":exact,"input_sha256":edit["input_sha256"],"output_exact":replacement,
            "output_sha256":Digest256::of_bytes(replacement.as_bytes()).to_hex(),
            "reason":reason,"responsibility":maker,"confidence":edit["confidence"],
            "evidence_anchor_refs":anchors,"status":"proposed"}),
        );
        output_cursor = replacement_end;
        cursor = end;
    }
    let tail = &input[positions[cursor]..];
    if output
        .len()
        .checked_add(tail.len())
        .is_none_or(|n| n > MAX_TEXT)
    {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer edited tail budget",
        ));
    }
    output.push_str(tail);
    bounded(&output)?;
    Ok((
        output,
        json!({"kind":"explicit_operations","operations":operations}),
    ))
}

pub(crate) fn build_derived_layer(
    configuration: &JsonValue,
    source_binding: &JsonValue,
    predecessor: Option<&JsonValue>,
    input_text: Option<&str>,
    supplied_text: Option<&str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<DerivedLayerOutput> {
    active(deadline, cancelled)?;
    let config = serde(configuration)?;
    let binding = serde(source_binding)?;
    let operation = config["allowed_operations"]
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(Value::as_str)
        .ok_or(bad())?;
    let supplied = matches!(
        operation,
        "text-layer.record-transcription"
            | "text-layer.record-ocr"
            | "text-layer.record-owner-ocr"
            | "text-layer.record-owner-page-ocr"
    );
    let normalize = operation == "text-layer.normalize";
    let form = config["policy"]["unicode_normalization"]
        .as_str()
        .ok_or(bad())?;
    let transcription = config["policy"]["method"]
        .as_str()
        .unwrap_or("manual_transcription");
    let selected_policy = policy(operation, form, transcription)?;
    if config["policy"] != selected_policy {
        return Err(SourceCommandError::Conflict(
            "native TextLayer policy changed",
        ));
    }
    let source_path = cmd::text(configuration, "source_path")?;
    let home = source_path.rsplit_once('/').ok_or(bad())?.0;
    let config_raw = line(&config)?;
    let config_sha = Digest256::of_bytes(&config_raw).to_hex();
    let content_ref = format!("{home}/content.txt");
    let policy_ref = format!("{home}/derivation-policy.json");
    let configuration_ref = format!("{home}/source-create-owner-configuration.json");
    let mut maker = config["maker"].clone();
    let maker_object = maker.as_object_mut().ok_or(bad())?;
    maker_object.insert("configuration_ref".into(), json!(configuration_ref));
    maker_object.insert("configuration_digest".into(), json!(config_sha));
    let anchors = binding["anchors"]
        .as_array()
        .ok_or(bad())?
        .iter()
        .map(|row| row["anchor_id"].clone())
        .collect::<Vec<_>>();
    if anchors.is_empty() || anchors.len() > 16 {
        return Err(bad());
    }
    let mut unique = std::collections::BTreeSet::new();
    if anchors
        .iter()
        .any(|row| row.as_str().is_none_or(|id| !unique.insert(id)))
    {
        return Err(bad());
    }
    let (text, change, inputs, supersedes, version, uncertainty, line_break) = if supplied {
        if predecessor.is_some() || input_text.is_some() {
            return Err(bad());
        }
        let text = supplied_text.ok_or(bad())?;
        bounded(text)?;
        (
            text.to_owned(),
            json!({"kind":"none"}),
            Vec::new(),
            Value::Null,
            1usize,
            json!({"status":"none","annotations":[]}),
            json!("logical_reflow"),
        )
    } else {
        if supplied_text.is_some() {
            return Err(bad());
        }
        let prior = serde(predecessor.ok_or(bad())?)?;
        let input = input_text.ok_or(bad())?;
        bounded(input)?;
        let target = &config["input"]["binding"]["text_layer"];
        let rep = &prior["representation"];
        if prior["layer_id"] != target["layer_id"]
            || prior["layer_version"] != target["layer_version"]
            || prior["source_binding"] != binding
            || rep["content_sha256"] != Digest256::of_bytes(input.as_bytes()).to_hex()
            || rep["text_scope"] != span(0, input.chars().count())
            || prior["layer_id"] == config["identities"]["layer_id"]
            || !normalize
                && (prior["layer_role"] == "normalized_text"
                    || rep["character_normalization"] != "none")
        {
            return Err(SourceCommandError::Conflict(
                "native TextLayer predecessor differs",
            ));
        }
        let edits = if normalize {
            let result = normalize_whole_predecessor(input, form, "16.0.0", deadline, cancelled)?;
            vec![
                json!({"start":0,"end":input.chars().count(),"input_exact":input,
                "input_sha256":Digest256::of_bytes(input.as_bytes()).to_hex(),"output_exact":result,
                "reason":format!("Explicit {form} under Unicode 16.0.0"),"confidence":1}),
            ]
        } else {
            config["material"]["edits"].as_array().ok_or(bad())?.clone()
        };
        let (text, change) = edited(
            input, &edits, &maker, &anchors, normalize, deadline, cancelled,
        )?;
        let input_binding = json!({"layer_id":target["layer_id"],"record_ref":target["record_ref"],
            "record_sha256":target["record_sha256"],"content_sha256":rep["content_sha256"]});
        let version = prior["layer_version"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| n.checked_add(1))
            .ok_or(bad())?;
        (
            text,
            change,
            vec![input_binding],
            prior["layer_id"].clone(),
            version,
            prior["uncertainty"].clone(),
            rep["line_break_posture"].clone(),
        )
    };
    if text.len() > cmd::integer(cmd::field(configuration, "limits")?, "max_output_bytes")? as usize
    {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer delegated output budget",
        ));
    }
    let content_sha = Digest256::of_bytes(text.as_bytes()).to_hex();
    let policy_raw = line(&selected_policy)?;
    let role = if normalize {
        "normalized_text"
    } else if matches!(
        operation,
        "text-layer.record-ocr"
            | "text-layer.record-owner-ocr"
            | "text-layer.record-owner-page-ocr"
    ) {
        "raw_ocr"
    } else if selected_policy["method"] == "model_transcription" {
        "machine_transcription"
    } else {
        "diplomatic_transcription"
    };
    let layer = json!({"$schema":LAYER_SCHEMA,"schema_version":"tos_source_text_layer_v1",
        "layer_id":config["identities"]["layer_id"],"layer_version":version,
        "supersedes_layer_ref":supersedes,"layer_role":role,"source_binding":binding,
        "representation":{"content_file_id":format!("tos.file.sha256.{content_sha}"),
            "content_ref":content_ref,"content_sha256":content_sha,"media_type":"text/plain",
            "charset":"UTF-8","language":config["language"],
            "text_scope":span(0,text.chars().count()),"character_normalization":form,
            "line_break_posture":line_break,"storage":"ignored_local","content_visibility":"local_only",
            "tracked_content":false,"publication_authorized":false,
            "rights_record_refs":config["derivation_access"]["rights_record_refs"],
            "publication_authority_refs":[]},
        "derivation":{"method":selected_policy["method"],"input_layers":inputs,"maker":maker,
            "preservation_goal":if normalize {"normalized_for_search"}else{"source_near"},
            "loss_posture":if normalize {"normalization_intended"}else if supplied {"unknown"}else{"preservation_intended"},
            "silent_changes_allowed":false,"change_payload":change},
        "editorial_policy":{"policy_ref":policy_ref,
            "policy_sha256":Digest256::of_bytes(&policy_raw).to_hex(),
            "transcription_goal":if normalize {"normalized_access"}else if selected_policy["method"] == "ocr"
                || selected_policy["method"] == "model_transcription" {"machine_candidate"}else{"diplomatic"},
            "historical_language_preserved":false,"printing_errors_silently_corrected":false,
            "typography_posture":if normalize {"normalize_declared"}else{"encode_explicitly"},
            "layout_posture":"encode_explicitly","unicode_normalization":form,
            "uncertainty_representation":"explicit-never-silent","method_declared":true},
        "uncertainty":uncertainty,
        "admission":{"mechanical_status":"materialized","review_status":"unreviewed",
            "review_ref":null,"human_review_performed":false,"human_language_competence":"not_assessed",
            "language_competence_evidence_refs":[],"accepted_uses":[],"automatic_validation_complete":false,
            "model_output_is_ground_truth":false,"validator_proves_content_truth":false,
            "routine_human_task_created":false,"promotion_authorized":false},
        "provenance_event_ref":config["identities"]["provenance_event_id"],"authority_boundary":AUTHORITY});
    let layer_raw = line(&layer)?;
    let converted = cmd::parse(&layer_raw)?;
    let mut files = BTreeMap::new();
    files.insert("source-text-layer.v1.json".into(), layer_raw);
    files.insert("derivation-policy.json".into(), policy_raw);
    files.insert("content.txt".into(), text.into_bytes());
    files.insert("source-create-owner-configuration.json".into(), config_raw);
    Ok(DerivedLayerOutput {
        layer: converted,
        files,
    })
}

#[cfg(test)]
#[path = "source_text_layer_derived_proposal_tests.rs"]
mod tests;
