//! First segmentation of one independently verified owner-local TextLayer.
//! The output is a source-bound proposal only; its caller owns protected
//! reading, schema checking, rights, identity absence and publication.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_text_unit_native::TextCodepointOffsets;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};

const MAX_PACKET: usize = 1_048_576;
const MAX_TEXT: usize = 8_388_608;
const SCHEMA: &str =
    "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json";
const CONFIDENCE_MEANING: &str = "maker-declared-boundary-confidence-not-truth-probability";
const UNIT_KINDS: [&str; 18] = [
    "document",
    "section",
    "paragraph",
    "physical_line",
    "verse_group",
    "verse_line",
    "sentence",
    "s_unit",
    "clause",
    "phrase",
    "surface_token",
    "syntactic_word",
    "multiword_token",
    "punctuation",
    "whitespace",
    "grapheme_cluster",
    "model_subword",
    "other",
];

fn visibility_rank(value: &str) -> SourceCommandResult<usize> {
    [
        "public",
        "public_metadata_only",
        "controlled",
        "local_only",
        "restricted",
        "unknown",
    ]
    .iter()
    .position(|candidate| *candidate == value)
    .ok_or(bad())
}

fn bad() -> SourceCommandError {
    SourceCommandError::Invalid("native TextUnit exact proposal")
}

fn value(source: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(source)?).map_err(|_| bad())
}

fn interval(row: &Value) -> SourceCommandResult<(usize, usize)> {
    let start = row["start"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(bad())?;
    let end = row["end"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(bad())?;
    if start >= end {
        return Err(bad());
    }
    Ok((start, end))
}

fn span(start: usize, end: usize) -> Value {
    json!({"start":start,"end":end,"position_unit":"unicode_code_point","interval":"half_open"})
}

fn anchor(
    identity: &str,
    role: &str,
    start: usize,
    end: usize,
    layer_ref: &str,
    layer_sha: &str,
    content_ref: &str,
    text: &str,
    offsets: &TextCodepointOffsets,
) -> SourceCommandResult<Value> {
    Ok(json!({
        "anchor_ref":identity,"text_layer_ref":layer_ref,"text_layer_sha256":layer_sha,
        "selector":{"type":"text_position","start":start,"end":end,
            "position_unit":"unicode_code_point","interval":"half_open"},
        "exact_sha256":offsets.exact_sha256(text,start,end)?.to_hex(),
        "anchor_role":role,"source_return":{"required":true,"locator_ref":content_ref},
    }))
}

pub(crate) fn build_text_unit_packet(
    existing_packet: Option<&JsonValue>,
    layer: &JsonValue,
    binding: &JsonValue,
    text: &str,
    configuration: &JsonValue,
    request: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, Vec<u8>)> {
    crate::source_creation_store::active(deadline, cancelled)?;
    if text.is_empty() || text.len() > MAX_TEXT {
        return Err(SourceCommandError::Unsupported(
            "native TextUnit exact text budget",
        ));
    }
    // The immutable file digest covers all bytes; normalization applies only
    // to the layer's declared absolute codepoint scope.
    crate::source_sign_native::verified_representation_scope(
        text,
        cmd::field(layer, "representation")?,
    )?;
    let layer = value(layer)?;
    let existing = existing_packet.map(value).transpose()?;
    let binding = value(binding)?;
    let config = value(configuration)?;
    let request = value(request)?;
    let rep = &layer["representation"];
    let target = &binding["text_layer"];
    let scope = &config["allowed_text_scope"];
    let (scope_start, scope_end) = interval(scope)?;
    let (frozen_start, frozen_end) = interval(&rep["text_scope"])?;
    let text_len = text.chars().count();
    let scheme = &config["scheme"];
    let policies = &scheme["policies"];
    let method = &config["method"];
    if ![
        "source_layout",
        "source_structure",
        "orthographic",
        "linguistic",
        "model_input",
        "interchange",
    ]
    .contains(&scheme["analysis_role"].as_str().ok_or(bad())?)
        || ![
            "source_markup",
            "source_layout",
            "unicode_default",
            "unicode_tailored",
            "rule_based",
            "statistical",
            "model",
            "imported",
            "manual",
        ]
        .contains(&scheme["boundary_basis"].as_str().ok_or(bad())?)
        || policies["normalization"] != "no-text-mutation-separate-successor-layer"
        || policies["unreported_gaps_allowed"] != false
        || policies["overlap"] != "forbid"
        || method["output_posture"] != "method_result_not_source_or_linguistic_truth"
        || !["real_human", "software", "model", "import"]
            .contains(&method["maker_kind"].as_str().ok_or(bad())?)
        || method["agent_ref"] != config["principal_id"]
        || method["provenance_event_ref"] != config["provenance_event_id"]
        || (!method["locale"].is_null() && method["locale"] != rep["language"])
    {
        return Err(SourceCommandError::Invalid(
            "native TextUnit selected method or scheme",
        ));
    }
    let content_sha = Digest256::of_bytes(text.as_bytes()).to_hex();
    if binding["schema_version"]
        != if existing.is_some() {
            "tos_native_text_unit_binding_v1"
        } else {
            "tos_native_text_layer_binding_v1"
        }
        || layer["schema_version"] != "tos_source_text_layer_v1"
        || target["layer_id"] != layer["layer_id"]
        || target["layer_version"] != layer["layer_version"]
        || !((frozen_start <= scope_start)
            && (scope_start < scope_end)
            && (scope_end <= frozen_end)
            && (frozen_end <= text_len))
        || rep["text_scope"]["position_unit"] != "unicode_code_point"
        || rep["text_scope"]["interval"] != "half_open"
        || rep["content_sha256"] != content_sha
        || rep["content_file_id"] != format!("tos.file.sha256.{content_sha}")
        || !["text/plain", "text/plain; charset=utf-8"]
            .contains(&rep["media_type"].as_str().ok_or(bad())?)
        || rep["rights_record_refs"]
            .as_array()
            .is_none_or(Vec::is_empty)
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit layer closure differs",
        ));
    }
    let form = rep["character_normalization"].as_str().ok_or(bad())?;
    let unicode_form = if form == "none" {
        "source_preserved"
    } else {
        form
    };
    if !["source_preserved", "NFC", "NFD", "NFKC", "NFKD"].contains(&unicode_form) {
        return Err(bad());
    }
    let slots = config["unit_slots"].as_array().ok_or(bad())?;
    let gap_ids = config["gap_anchor_refs"].as_array().ok_or(bad())?;
    let spans = request["spans"].as_array().ok_or(bad())?;
    let excluded = request["excluded_gaps"].as_array().ok_or(bad())?;
    if slots.is_empty()
        || slots.len() > 256
        || spans.len() != slots.len()
        || gap_ids.len() > 257
        || excluded.len() > gap_ids.len()
    {
        return Err(SourceCommandError::Unsupported(
            "native TextUnit span count",
        ));
    }
    let mut by_unit = BTreeMap::new();
    let mut all_ids = BTreeSet::new();
    for name in [
        "packet_id",
        "scheme_id",
        "segmentation_id",
        "scope_anchor_ref",
        "provenance_event_id",
    ] {
        if !all_ids.insert(config[name].as_str().ok_or(bad())?.to_owned()) {
            return Err(bad());
        }
    }
    for slot in slots {
        let unit_id = slot["unit_id"].as_str().ok_or(bad())?;
        let anchor_id = slot["anchor_ref"].as_str().ok_or(bad())?;
        if !UNIT_KINDS.contains(&slot["unit_kind"].as_str().ok_or(bad())?) {
            return Err(bad());
        }
        if !all_ids.insert(unit_id.to_owned())
            || !all_ids.insert(anchor_id.to_owned())
            || by_unit.insert(unit_id.to_owned(), slot).is_some()
        {
            return Err(bad());
        }
    }
    let allowed_gaps = gap_ids
        .iter()
        .map(|row| row.as_str().ok_or(bad()))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if allowed_gaps.len() != gap_ids.len() {
        return Err(bad());
    }
    for id in &allowed_gaps {
        if !all_ids.insert((*id).to_owned()) {
            return Err(bad());
        }
    }
    let mut endpoints = vec![scope_start, scope_end];
    let mut cursor = scope_start;
    let mut expected_gaps = Vec::new();
    let mut seen_units = BTreeSet::new();
    for row in spans {
        let id = row["unit_id"].as_str().ok_or(bad())?;
        if !by_unit.contains_key(id) || !seen_units.insert(id) {
            return Err(bad());
        }
        let (start, end) = interval(row)?;
        if start < cursor || end > scope_end {
            return Err(bad());
        }
        if cursor < start {
            expected_gaps.push((cursor, start));
        }
        endpoints.extend([start, end]);
        cursor = end;
        let certainty = &row["certainty"];
        let confidence = certainty["value"].as_f64().ok_or(bad())?;
        if !confidence.is_finite()
            || !(0.0..=1.0).contains(&confidence)
            || certainty["meaning"] != CONFIDENCE_MEANING
            || row["status_reason"].as_str().is_none_or(str::is_empty)
        {
            return Err(bad());
        }
    }
    if cursor < scope_end {
        expected_gaps.push((cursor, scope_end));
    }
    let mut seen_gaps = BTreeSet::new();
    let mut actual_gaps = Vec::new();
    for row in excluded {
        let id = row["anchor_ref"].as_str().ok_or(bad())?;
        if !allowed_gaps.contains(id) || !seen_gaps.insert(id) {
            return Err(bad());
        }
        let pair = interval(row)?;
        endpoints.extend([pair.0, pair.1]);
        actual_gaps.push(pair);
    }
    if actual_gaps != expected_gaps || seen_units.len() != slots.len() {
        return Err(bad());
    }
    let offsets = TextCodepointOffsets::select(text, &endpoints, deadline, cancelled)?;
    let source = &layer["source_binding"];
    let derived_scope = json!({"work_ref":source["work_ref"],"expression_ref":source["expression_ref"],
        "edition_ref":source["edition_ref"],"item_ref":source["item_ref"],
        "file_ref":source["source_file_ref"],"file_sha256":source["source_file_sha256"]});
    let layer_ref = target["record_ref"].as_str().ok_or(bad())?;
    let content_ref = rep["content_ref"].as_str().ok_or(bad())?;
    let derived_layer = json!({"text_layer_ref":layer_ref,"text_layer_sha256":content_sha,
        "language":rep["language"],"media_type":"text/plain; charset=utf-8",
        "unicode_form":unicode_form,"position_unit":"unicode_code_point","interval":"half_open",
        "immutable":true,"visibility":rep["content_visibility"],
        "publication_authorized":rep["publication_authorized"]});
    let visibility = rep["content_visibility"].as_str().ok_or(bad())?;
    let effective = if visibility_rank(visibility)? > visibility_rank("local_only")? {
        visibility
    } else {
        "local_only"
    };
    let rights_refs = rep["rights_record_refs"]
        .as_array()
        .ok_or(bad())?
        .iter()
        .map(|row| row["ref"].clone())
        .collect::<Vec<_>>();
    let derived_rights = json!({"source_visibility":visibility,"packet_visibility":"local_only",
        "effective_visibility":effective,"rights_record_refs":rights_refs,
        "private_source_used":true,"publication_authorized":false,
        "inheritance_policy":"most-restrictive-source-packet-and-destination-wins"});
    let (source_scope, source_layer, mut rights) = if let Some(packet) = &existing {
        if packet["schema_version"] != "tos_source_text_unit_packet_v1"
            || packet["content_posture"] != "source_bound"
            || packet["source_scope"] != derived_scope
            || packet["source_layer"] != derived_layer
            || packet["rights_and_visibility"]["source_visibility"] != visibility
            || packet["rights_and_visibility"]["rights_record_refs"]
                != derived_rights["rights_record_refs"]
        {
            return Err(SourceCommandError::Conflict(
                "native TextUnit predecessor packet closure",
            ));
        }
        let target = &binding["text_layer"];
        if binding["packet_ref"].as_str().is_none()
            || binding["packet_sha256"].as_str().is_none()
            || target["record_ref"] != layer_ref
            || packet["packet_id"] != binding["packet_id"]
            || packet["packet_version"] != binding["packet_version"]
        {
            return Err(SourceCommandError::Conflict(
                "native TextUnit packet binding",
            ));
        }
        let unit_id = binding["unit_id"].as_str().ok_or(bad())?;
        let unit = packet["units"]
            .as_array()
            .ok_or(bad())?
            .iter()
            .find(|row| row["unit_id"] == unit_id)
            .ok_or(bad())?;
        if unit["continuity"] != "contiguous" {
            return Err(bad());
        }
        let refs = unit["ordered_anchor_refs"].as_array().ok_or(bad())?;
        if refs.is_empty() {
            return Err(bad());
        }
        let anchors = packet["anchors"].as_array().ok_or(bad())?;
        let mut prior_end = None;
        let mut first = None;
        for reference in refs {
            let anchor = anchors
                .iter()
                .find(|row| row["anchor_ref"] == *reference)
                .ok_or(bad())?;
            let (left, right) = interval(&anchor["selector"])?;
            if prior_end.is_some_and(|end| end != left) {
                return Err(bad());
            }
            first.get_or_insert(left);
            prior_end = Some(right);
        }
        if first.is_none_or(|left| left > scope_start)
            || prior_end.is_none_or(|right| scope_end > right)
        {
            return Err(SourceCommandError::Denied(
                "native TextUnit scope leaves selected unit",
            ));
        }
        let mut inherited = packet["rights_and_visibility"].clone();
        if visibility_rank(inherited["packet_visibility"].as_str().ok_or(bad())?)?
            > visibility_rank("local_only")?
        {
            return Err(SourceCommandError::Denied(
                "native TextUnit predecessor packet restriction",
            ));
        }
        inherited["packet_visibility"] = json!("local_only");
        inherited["publication_authorized"] = json!(false);
        let source_visibility = inherited["source_visibility"].as_str().ok_or(bad())?;
        inherited["effective_visibility"] = json!(if visibility_rank(source_visibility)?
            > visibility_rank("local_only")?
        {
            source_visibility
        } else {
            "local_only"
        });
        (
            packet["source_scope"].clone(),
            packet["source_layer"].clone(),
            inherited,
        )
    } else {
        (derived_scope, derived_layer, derived_rights)
    };
    let mut anchors = Vec::new();
    anchors.push(anchor(
        config["scope_anchor_ref"].as_str().ok_or(bad())?,
        "scope",
        scope_start,
        scope_end,
        layer_ref,
        &content_sha,
        content_ref,
        text,
        &offsets,
    )?);
    let mut ordered = Vec::new();
    let mut units = Vec::new();
    for row in spans {
        let id = row["unit_id"].as_str().ok_or(bad())?;
        let slot = by_unit[id];
        let (start, end) = interval(row)?;
        let anchor_id = slot["anchor_ref"].as_str().ok_or(bad())?;
        ordered.push((
            start,
            anchor(
                anchor_id,
                "content",
                start,
                end,
                layer_ref,
                &content_sha,
                content_ref,
                text,
                &offsets,
            )?,
        ));
        units.push(json!({"unit_id":id,"unit_version":1,"supersedes_unit_ref":null,
            "identity_policy":"opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
            "unit_kind":slot["unit_kind"],"surface_posture":"source_bearing","continuity":"contiguous",
            "ordered_anchor_refs":[anchor_id],"parent_unit_refs":[],"ordered_child_unit_refs":[],
            "boundary_posture":"method_proposed","certainty":row["certainty"],
            "status_reason":row["status_reason"],"source_text_mutated":false,"semantic_promotion":false}));
    }
    for row in excluded {
        let (start, end) = interval(row)?;
        ordered.push((
            start,
            anchor(
                row["anchor_ref"].as_str().ok_or(bad())?,
                "gap",
                start,
                end,
                layer_ref,
                &content_sha,
                content_ref,
                text,
                &offsets,
            )?,
        ));
    }
    ordered.sort_by_key(|(start, _)| *start);
    anchors.extend(ordered.into_iter().map(|(_, row)| row));
    for (index, row) in anchors.iter_mut().enumerate() {
        row["ordinal"] = json!(index + 1);
    }
    let unit_kinds = units
        .iter()
        .map(|row| row["unit_kind"].clone())
        .collect::<Vec<_>>();
    let unit_kinds = unit_kinds.into_iter().fold(Vec::new(), |mut unique, kind| {
        if !unique.contains(&kind) {
            unique.push(kind);
        }
        unique
    });
    let mut scheme = config["scheme"].clone();
    let scheme_map = scheme.as_object_mut().ok_or(bad())?;
    scheme_map.insert("scheme_id".into(), config["scheme_id"].clone());
    scheme_map.insert("scheme_version".into(), json!(1));
    scheme_map.insert("supersedes_scheme_ref".into(), Value::Null);
    scheme_map.insert(
        "identity_policy".into(),
        json!("opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis"),
    );
    scheme_map.insert("unit_kinds".into(), json!(unit_kinds));
    scheme_map.insert("method".into(), config["method"].clone());
    scheme_map.insert(
        "authority_limit".into(),
        json!({"algorithmic_output_is_source_truth":false,
        "algorithmic_output_is_linguistic_truth":false,"model_subword_is_lexeme":false,
        "unit_identity_is_semantic_identity":false,"text_mutation_allowed":false}),
    );
    let packet = json!({"$schema":SCHEMA,"schema_version":"tos_source_text_unit_packet_v1",
        "packet_id":config["packet_id"],"packet_version":1,"supersedes_packet_ref":null,
        "content_posture":"source_bound","source_scope":source_scope,"source_layer":source_layer,
        "schemes":[scheme],"anchors":anchors,"units":units,
        "segmentations":[{"segmentation_id":config["segmentation_id"],"segmentation_version":1,
            "supersedes_segmentation_ref":null,
            "identity_policy":"opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
            "scheme_ref":config["scheme_id"],
            "ordered_unit_refs":spans.iter().map(|row|row["unit_id"].clone()).collect::<Vec<_>>(),
            "coverage":{"scope_anchor_ref":config["scope_anchor_ref"],
                "coverage_posture":if excluded.is_empty(){"exhaustive_nonoverlapping"}else{"declared_partial"},
                "excluded_anchor_refs":excluded.iter().map(|row|row["anchor_ref"].clone()).collect::<Vec<_>>(),
                "unreported_gaps_allowed":false,"overlap_requires_declaration":true,
                "source_reconstruction_required":true},
            "status":"proposed","status_reason":"Exact delegated span proposal, retaining proposed boundary and source-assessment status.",
            "maker":config["method"],"competing_segmentation_refs":[],"review_refs":[],
            "declared_uses":["source_observation"],"source_text_authority":false,
            "linguistic_authority":false,"semantic_authority":false}],
        "reviews":[],"projections":[],"rights_and_visibility":rights,
        "authority_boundary":{"tree_role":"orientation","graph_role":"relation","source_role":"authority",
            "validators_prove_mechanics_not_truth":true,"segmentation_is_method_result_not_source_truth":true,
            "source_unit_is_not_lexeme_sign_or_concept":true,"model_token_is_not_linguistic_token":true,
            "projection_is_owner_truth":false,"legacy_bulk_migration_authorized":false}});
    let raw = serde_json::to_vec(&packet).map_err(|_| bad())?;
    if raw.len() > MAX_PACKET {
        return Err(SourceCommandError::Unsupported(
            "native TextUnit packet byte budget",
        ));
    }
    let foundation = cmd::parse(&raw)?;
    let mut bytes = cmd::canonical(&foundation)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_PACKET {
        return Err(SourceCommandError::Unsupported(
            "native TextUnit packet byte budget",
        ));
    }
    Ok((foundation, bytes))
}

#[cfg(test)]
#[path = "source_text_unit_proposal_tests.rs"]
mod tests;
