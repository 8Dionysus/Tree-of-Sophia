//! Family-local source-text-unit v1 mechanics from the frozen Python source.
//!
//! This is a FullOnly observation of one exact packet and text layer. Schema
//! evaluation, current owner authority, complete corpus membership and CMD
//! attestation are external gates. No result from this module admits source.

use std::collections::{BTreeMap, BTreeSet};

use tos_foundation::{Digest256, JsonLimits, JsonMode, JsonValue, parse_json};

use crate::{Coverage, KeyState, PredicateRead};

pub const TEXT_UNIT_RULE_ID: &str = "tos.val.text-unit.v1@1";
pub const TEXT_UNIT_PROFILE: &str = "tos_source_text_unit_packet_v1";
pub const TEXT_LAYER_RULE_ID: &str = "tos.val.text-layer.v1@1";
pub const TEXT_LAYER_PROFILE: &str = "tos_source_text_layer_v1";
pub const ANCHOR_V2_RULE_ID: &str = "tos.val.anchor-selector.v2@1";
pub const ANCHOR_V2_PROFILE: &str = "tos_source_anchor_v2";
const MAX_LAYER_RESOURCE_BYTES: usize = 67_108_864;
const MAX_TOTAL_LAYER_RESOURCE_BYTES: usize = 134_217_728;
const MAX_PACKET_BYTES: usize = 2_097_152;
const MAX_TEXT_BYTES: usize = 8_388_608;
const MAX_ROWS: usize = 4096;
const MAX_ISSUES: usize = 4096;
static NULL_JSON: JsonValue = JsonValue::Null;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRuleContext {
    pub packet_path: String,
    pub frozen_text_locator: String,
    pub schema_checked: bool,
    pub requested_profiles: Vec<String>,
    /// Complete packet-local interval generation supplied by the snapshot reader.
    pub interval_generation: String,
    /// Complete packet-local reverse-reference generation supplied by the reader.
    pub reverse_generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextRuleState {
    Checked,
    InvalidInput,
    Unsupported,
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextIssue {
    pub code: &'static str,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalFact {
    pub scope: String,
    pub member: String,
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseFact {
    pub target: String,
    pub relation: &'static str,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRuleReport {
    pub rule_id: &'static str,
    pub packet_digest: Option<String>,
    pub coverage: Coverage,
    pub state: TextRuleState,
    pub checked_profiles: Vec<String>,
    pub unsupported_profiles: Vec<String>,
    pub issues: Vec<TextIssue>,
    pub reads: Vec<PredicateRead>,
    pub intervals: Vec<IntervalFact>,
    pub reverse_facts: Vec<ReverseFact>,
}

impl TextRuleReport {
    fn new_for(rule_id: &'static str) -> Self {
        Self {
            rule_id,
            packet_digest: None,
            coverage: Coverage::FullOnly,
            state: TextRuleState::Unsupported,
            checked_profiles: Vec::new(),
            unsupported_profiles: Vec::new(),
            issues: Vec::new(),
            reads: Vec::new(),
            intervals: Vec::new(),
            reverse_facts: Vec::new(),
        }
    }

    fn issue(&mut self, code: &'static str, subject: impl Into<String>) {
        if self.issues.len() >= MAX_ISSUES {
            self.state = TextRuleState::BudgetExceeded;
            return;
        }
        self.issues.push(TextIssue {
            code,
            subject: subject.into(),
        });
    }

    fn edge(&mut self, target: &str, relation: &'static str, source: &str) {
        self.reverse_facts.push(ReverseFact {
            target: target.to_owned(),
            relation,
            source: source.to_owned(),
        });
    }

    fn endpoint(&mut self, target: &str, relation: &'static str, source: &str, present: bool) {
        self.edge(target, relation, source);
        let namespace = format!(
            "text-unit/packet-local/{}/{relation}",
            self.packet_digest.as_deref().unwrap_or("unbound")
        );
        self.reads.push(PredicateRead::RefEndpoint {
            endpoint_type: namespace.clone(),
            id: target.into(),
            observed: if present {
                KeyState::Present
            } else {
                KeyState::Absent
            },
        });
        if !present {
            self.reads.push(PredicateRead::AbsentKey {
                namespace,
                key: target.into(),
            });
        }
    }
}

fn field<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn string<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    field(value, key)?.as_str()
}
fn boolean(value: &JsonValue, key: &str) -> Option<bool> {
    field(value, key)?.as_bool()
}
fn rows<'a>(value: &'a JsonValue, key: &str) -> &'a [JsonValue] {
    field(value, key)
        .and_then(JsonValue::as_array)
        .unwrap_or(&[])
}
fn strings<'a>(value: &'a JsonValue, key: &str) -> Vec<&'a str> {
    rows(value, key)
        .iter()
        .filter_map(JsonValue::as_str)
        .collect()
}
fn nullable_same(value: &JsonValue, key: &str, id: &str) -> bool {
    string(value, key) == Some(id)
}
fn index<'a>(
    items: &'a [JsonValue],
    key: &str,
    label: &'static str,
    packet_digest: &str,
    report: &mut TextRuleReport,
) -> BTreeMap<String, &'a JsonValue> {
    let mut result = BTreeMap::new();
    for row in items {
        if let Some(id) = string(row, key) {
            report.reads.push(PredicateRead::UniqueKey {
                namespace: format!("text-unit/packet-local/{packet_digest}/{key}"),
                key: id.to_owned(),
                owner: packet_digest.into(),
            });
            if result.insert(id.to_owned(), row).is_some() {
                report.issue(label, id);
            }
        }
    }
    result
}
fn refs<'a>(
    value: &'a JsonValue,
    key: &str,
    known: &BTreeMap<String, &'a JsonValue>,
    code: &'static str,
    relation: &'static str,
    source: &str,
    report: &mut TextRuleReport,
) {
    for id in strings(value, key) {
        let present = known.contains_key(id);
        report.endpoint(id, relation, source, present);
        if !present {
            report.issue(code, id);
        }
    }
}
fn interval(value: &JsonValue) -> Option<(u64, u64)> {
    let selector = field(value, "selector")?;
    Some((
        field(selector, "start")?.as_u64()?,
        field(selector, "end")?.as_u64()?,
    ))
}

fn span(value: &JsonValue) -> Option<(usize, usize)> {
    Some((
        usize::try_from(field(value, "start")?.as_u64()?).ok()?,
        usize::try_from(field(value, "end")?.as_u64()?).ok()?,
    ))
}

/// One exact snapshot resource. The caller must bind these bytes to the same
/// serializable cut as the layer; the helper never reopens a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerResource<'a> {
    pub locator: &'a str,
    pub raw: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerRuleContext {
    pub layer_path: String,
    pub schema_checked: bool,
    pub requested_profiles: Vec<String>,
    pub interval_generation: String,
    pub reverse_generation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorRuleContext {
    pub anchor_path: String,
    pub target_locator: String,
    pub method_configuration_locator: String,
    pub schema_checked: bool,
    pub requested_profiles: Vec<String>,
    pub interval_generation: String,
    pub reverse_generation: String,
}

fn anchor_selection(
    report: &mut TextRuleReport,
    anchor_id: &str,
    context: &AnchorRuleContext,
    bytes: &[u8],
    start: u64,
    end: u64,
) {
    let digest = Digest256::of_bytes(bytes).to_hex();
    report.reads.push(PredicateRead::ExactBytes {
        locator: format!("anchor-selection:{anchor_id}"),
        digest,
    });
    report.reads.push(PredicateRead::Interval {
        scope: context.target_locator.clone(),
        start,
        end,
        generation: context.interval_generation.clone(),
    });
    report.intervals.push(IntervalFact {
        scope: context.target_locator.clone(),
        member: anchor_id.into(),
        start,
        end,
    });
}

/// Exact single-selector source-anchor v2 mechanics. The Unicode boundary
/// proof is deliberately conservative: an ASCII code point at either edge
/// cannot be a combining mark. Non-ASCII edges, normalization, structural
/// traversal and compound expressions remain Unsupported without pinned
/// Unicode/selector implementations and their complete resource closure.
pub fn inspect_source_anchor_v2_single(
    raw_anchor: &[u8],
    target_bytes: &[u8],
    method_configuration: &[u8],
    context: &AnchorRuleContext,
) -> TextRuleReport {
    let mut out = TextRuleReport::new_for(ANCHOR_V2_RULE_ID);
    for profile in &context.requested_profiles {
        if profile != ANCHOR_V2_PROFILE {
            out.unsupported_profiles.push(profile.clone());
        }
    }
    if !context.schema_checked
        || context.requested_profiles.is_empty()
        || !out.unsupported_profiles.is_empty()
        || context.anchor_path.is_empty()
        || context.target_locator.is_empty()
        || context.method_configuration_locator.is_empty()
        || context.interval_generation.is_empty()
        || context.reverse_generation.is_empty()
    {
        if !context.schema_checked {
            out.unsupported_profiles.push("schema-unchecked".into());
        }
        if context.requested_profiles.is_empty() {
            out.unsupported_profiles.push("no-profile".into());
        }
        if context.anchor_path.is_empty()
            || context.target_locator.is_empty()
            || context.method_configuration_locator.is_empty()
            || context.interval_generation.is_empty()
            || context.reverse_generation.is_empty()
        {
            out.unsupported_profiles
                .push("missing-snapshot-binding".into());
        }
        return out;
    }
    if raw_anchor.len() > MAX_PACKET_BYTES
        || target_bytes.len() > MAX_TEXT_BYTES
        || method_configuration.len() > MAX_PACKET_BYTES
    {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    let limits = match JsonLimits::new(MAX_PACKET_BYTES, 64, 300_000, 4_300) {
        Ok(v) => v,
        Err(_) => {
            out.state = TextRuleState::BudgetExceeded;
            return out;
        }
    };
    let document = match parse_json(raw_anchor, JsonMode::PublishedStrict, limits) {
        Ok(v) => v,
        Err(_) => {
            out.state = TextRuleState::InvalidInput;
            out.issue("invalid_published_json", &context.anchor_path);
            return out;
        }
    };
    let anchor = document.root();
    if string(anchor, "schema_version") != Some(ANCHOR_V2_PROFILE) {
        out.unsupported_profiles.push(
            string(anchor, "schema_version")
                .unwrap_or("missing-schema-version")
                .into(),
        );
        return out;
    }
    let anchor_digest = Digest256::of_bytes(raw_anchor).to_hex();
    let target_digest = Digest256::of_bytes(target_bytes).to_hex();
    let method_digest = Digest256::of_bytes(method_configuration).to_hex();
    out.packet_digest = Some(anchor_digest.clone());
    out.reads.extend([
        PredicateRead::ExactPath {
            path: context.anchor_path.clone(),
            digest: anchor_digest,
        },
        PredicateRead::ExactBytes {
            locator: context.target_locator.clone(),
            digest: target_digest.clone(),
        },
        PredicateRead::ExactBytes {
            locator: context.method_configuration_locator.clone(),
            digest: method_digest.clone(),
        },
    ]);
    let id = string(anchor, "anchor_id").unwrap_or("");
    out.reads.push(PredicateRead::UniqueKey {
        namespace: "source-anchor-v2/id".into(),
        key: id.into(),
        owner: context.anchor_path.clone(),
    });
    out.reads.push(PredicateRead::Range {
        namespace: "source-anchor-v2/id".into(),
        lower: id.into(),
        upper: id.into(),
        generation: context.reverse_generation.clone(),
    });
    if string(anchor, "supersedes_anchor_ref") == Some(id) {
        out.issue("anchor_self_supersession", id);
    }
    let target = field(anchor, "target").unwrap_or(&NULL_JSON);
    let file_id = string(target, "file_id").unwrap_or("");
    if string(target, "file_sha256") != Some(target_digest.as_str())
        || file_id != format!("tos.file.sha256.{target_digest}")
    {
        out.issue("anchor_target_digest_drift", id);
    }
    out.reads.push(PredicateRead::ReverseRefs {
        target: file_id.into(),
        relation: "source-anchor-v2/target-file".into(),
        generation: context.reverse_generation.clone(),
    });
    out.edge(file_id, "target_file", id);
    let method = field(anchor, "selector_method").unwrap_or(&NULL_JSON);
    if string(method, "configuration_ref").is_none() {
        out.unsupported_profiles
            .push("method-configuration-without-ref".into());
    } else if string(method, "configuration_ref")
        != Some(context.method_configuration_locator.as_str())
        || string(method, "configuration_digest") != Some(method_digest.as_str())
    {
        out.issue("anchor_method_configuration_drift", id);
    }
    let publication = field(anchor, "publication_boundary").unwrap_or(&NULL_JSON);
    let payload = field(anchor, "selector_payload").unwrap_or(&NULL_JSON);
    if string(payload, "kind") == Some("withheld_selector_receipt") {
        if boolean(publication, "source_text_in_record") == Some(true) {
            out.issue("anchor_withheld_text_exposure", id);
        }
        out.unsupported_profiles
            .push("withheld-selector-receipt".into());
        if !out.issues.is_empty() {
            out.state = TextRuleState::InvalidInput;
        }
        return out;
    }
    let expression = field(payload, "expression").unwrap_or(&NULL_JSON);
    if string(expression, "mode") != Some("single") {
        out.unsupported_profiles.push(format!(
            "selector-mode:{}",
            string(expression, "mode").unwrap_or("missing")
        ));
        return out;
    }
    let envelope = field(expression, "selector").unwrap_or(&NULL_JSON);
    let state = field(envelope, "state").unwrap_or(&NULL_JSON);
    let selector = field(envelope, "selector").unwrap_or(&NULL_JSON);
    let selector_type = string(selector, "type").unwrap_or("");
    if string(state, "representation_ref") != Some(context.target_locator.as_str())
        || string(state, "representation_sha256") != Some(target_digest.as_str())
        || string(state, "representation_sha256") != string(target, "file_sha256")
        || string(state, "media_type") != string(target, "media_type")
    {
        out.issue("anchor_selector_state_drift", id);
    }
    if string(state, "version_ref").is_some() {
        out.unsupported_profiles
            .push("versioned-representation-state".into());
    }
    let text_selector = selector_type == "text_quote" || selector_type == "text_position";
    if text_selector && string(state, "character_normalization").is_none() {
        out.issue("anchor_text_normalization_unspecified", id);
    }
    if text_selector && string(state, "character_normalization") != Some("none") {
        out.unsupported_profiles.push(format!(
            "unicode-normalization:{}",
            string(state, "character_normalization").unwrap_or("missing")
        ));
    }
    if string(publication, "record_storage") == Some("tracked")
        && string(publication, "source_content_visibility") != Some("public")
        && selector_type == "text_quote"
    {
        out.issue("anchor_tracked_nonpublic_quote", id);
    }
    if boolean(publication, "source_text_in_record") == Some(false) && selector_type == "text_quote"
    {
        out.issue("anchor_quote_exposure_conflict", id);
    }
    if boolean(publication, "source_text_in_record") == Some(true) && selector_type != "text_quote"
    {
        out.issue("anchor_declared_text_absent", id);
    }
    if !out.unsupported_profiles.is_empty() {
        if !out.issues.is_empty() {
            out.state = TextRuleState::InvalidInput;
        }
        return out;
    }
    match selector_type {
        "byte_position" => {
            if let Some((start, end)) = span(selector) {
                if start < end && end <= target_bytes.len() {
                    anchor_selection(
                        &mut out,
                        id,
                        context,
                        &target_bytes[start..end],
                        start as u64,
                        end as u64,
                    );
                } else {
                    out.issue("anchor_byte_span_outside_target", id);
                }
            } else {
                out.issue("anchor_byte_span_invalid", id);
            }
        }
        "text_position" => match std::str::from_utf8(target_bytes) {
            Ok(text) => {
                let points: Vec<usize> = text
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(text.len()))
                    .collect();
                if let Some((start, end)) = span(selector) {
                    if start < end && end < points.len() {
                        let start_char = text[points[start]..].chars().next();
                        let end_char = text[points[end]..].chars().next();
                        if start_char.is_some_and(|c| c.is_ascii())
                            && end_char.is_none_or(|c| c.is_ascii())
                        {
                            anchor_selection(
                                &mut out,
                                id,
                                context,
                                &target_bytes[points[start]..points[end]],
                                start as u64,
                                end as u64,
                            );
                        } else {
                            out.unsupported_profiles
                                .push("non-ascii-selector-boundary".into());
                        }
                    } else {
                        out.issue("anchor_text_span_outside_target", id);
                    }
                } else {
                    out.issue("anchor_text_span_invalid", id);
                }
            }
            Err(_) => out.issue("anchor_target_not_utf8", id),
        },
        "text_quote" => match std::str::from_utf8(target_bytes) {
            Ok(text) => {
                let exact = string(selector, "exact").unwrap_or("");
                let prefix = string(selector, "prefix");
                let suffix = string(selector, "suffix");
                if exact.is_empty() {
                    out.issue("anchor_quote_empty", id);
                    out.state = TextRuleState::InvalidInput;
                    return out;
                }
                let mut found = text.match_indices(exact).filter(|(start, _)| {
                    let end = *start + exact.len();
                    prefix.is_none_or(|p| text[..*start].ends_with(p))
                        && suffix.is_none_or(|s| text[end..].starts_with(s))
                });
                let first = found.next();
                if first.is_none() || found.next().is_some() {
                    out.issue("anchor_quote_not_unique", id);
                } else {
                    let start = first.unwrap().0;
                    let end = start + exact.len();
                    let start_char = text[start..].chars().next();
                    let end_char = text[end..].chars().next();
                    if start_char.is_some_and(|c| c.is_ascii())
                        && end_char.is_none_or(|c| c.is_ascii())
                    {
                        anchor_selection(
                            &mut out,
                            id,
                            context,
                            exact.as_bytes(),
                            text[..start].chars().count() as u64,
                            text[..end].chars().count() as u64,
                        );
                    } else {
                        out.unsupported_profiles
                            .push("non-ascii-selector-boundary".into());
                    }
                }
            }
            Err(_) => out.issue("anchor_target_not_utf8", id),
        },
        _ => out
            .unsupported_profiles
            .push(format!("selector-type:{selector_type}")),
    }
    if !out.issues.is_empty() {
        out.state = TextRuleState::InvalidInput;
    } else if out.unsupported_profiles.is_empty() {
        out.state = TextRuleState::Checked;
        out.checked_profiles.push(ANCHOR_V2_PROFILE.into());
    }
    out
}

fn layer_bound_resource(
    owner: &str,
    reference: &str,
    expected_digest: &str,
    resources: &[LayerResource<'_>],
    report: &mut TextRuleReport,
) -> bool {
    let matches: Vec<_> = resources
        .iter()
        .filter(|r| r.locator == reference)
        .collect();
    report.reads.push(PredicateRead::RefEndpoint {
        endpoint_type: "text-layer/exact-resource".into(),
        id: reference.into(),
        observed: if matches.len() == 1 {
            KeyState::Present
        } else {
            KeyState::Absent
        },
    });
    report.edge(reference, "layer_resource", owner);
    if matches.len() != 1 {
        if matches.is_empty() {
            report
                .unsupported_profiles
                .push(format!("unavailable-resource:{reference}"));
            report.reads.push(PredicateRead::AbsentKey {
                namespace: "text-layer/exact-resource".into(),
                key: reference.into(),
            });
        } else {
            report.issue("layer_resource_not_unique", reference);
        }
        return false;
    }
    let resource = matches[0];
    if resource.raw.len() > MAX_LAYER_RESOURCE_BYTES {
        report.state = TextRuleState::BudgetExceeded;
        return false;
    }
    let actual = Digest256::of_bytes(resource.raw).to_hex();
    report.reads.push(PredicateRead::ExactBytes {
        locator: reference.into(),
        digest: actual.clone(),
    });
    if actual != expected_digest {
        report.issue("layer_resource_digest_drift", reference);
        return false;
    }
    true
}

fn layer_binding_refs(
    owner: &str,
    bindings: &[JsonValue],
    reference_key: &str,
    digest_key: &str,
    resources: &[LayerResource<'_>],
    report: &mut TextRuleReport,
) {
    for binding in bindings {
        if let (Some(reference), Some(digest)) =
            (string(binding, reference_key), string(binding, digest_key))
        {
            layer_bound_resource(owner, reference, digest, resources, report);
        }
    }
}

fn layer_maker_configuration(
    owner: &str,
    maker: &JsonValue,
    resources: &[LayerResource<'_>],
    seen: &mut BTreeSet<(String, String)>,
    report: &mut TextRuleReport,
) {
    if let (Some(reference), Some(digest)) = (
        string(maker, "configuration_ref"),
        string(maker, "configuration_digest"),
    ) {
        if seen.insert((reference.into(), digest.into())) {
            layer_bound_resource(owner, reference, digest, resources, report);
        }
    } else if string(maker, "configuration_ref").is_none() {
        report
            .unsupported_profiles
            .push("maker-configuration-without-ref".into());
    }
}

/// Mechanical source-text-layer v1 observation over exact snapshot bytes.
/// This probe is not an AuditRule while schema_checked is caller asserted.
/// Every declared external reference must be present in `resources`; otherwise
/// the result is Unsupported. Unicode normalization profiles remain explicit
/// Unsupported until a pinned Unicode algorithm is available.
pub fn inspect_source_text_layer_v1(
    raw_layer: &[u8],
    resources: &[LayerResource<'_>],
    context: &LayerRuleContext,
) -> TextRuleReport {
    let mut out = TextRuleReport::new_for(TEXT_LAYER_RULE_ID);
    for profile in &context.requested_profiles {
        if profile != TEXT_LAYER_PROFILE {
            out.unsupported_profiles.push(profile.clone());
        }
    }
    if !context.schema_checked
        || context.requested_profiles.is_empty()
        || !out.unsupported_profiles.is_empty()
        || context.layer_path.is_empty()
        || context.interval_generation.is_empty()
        || context.reverse_generation.is_empty()
    {
        if !context.schema_checked {
            out.unsupported_profiles.push("schema-unchecked".into());
        }
        if context.requested_profiles.is_empty() {
            out.unsupported_profiles.push("no-profile".into());
        }
        if context.layer_path.is_empty()
            || context.interval_generation.is_empty()
            || context.reverse_generation.is_empty()
        {
            out.unsupported_profiles
                .push("missing-snapshot-binding".into());
        }
        return out;
    }
    if raw_layer.len() > MAX_PACKET_BYTES
        || resources.len() > MAX_ROWS
        || resources
            .iter()
            .any(|r| r.raw.len() > MAX_LAYER_RESOURCE_BYTES)
        || resources.iter().map(|r| r.raw.len()).sum::<usize>() > MAX_TOTAL_LAYER_RESOURCE_BYTES
    {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    let limits = match JsonLimits::new(MAX_PACKET_BYTES, 64, 300_000, 4_300) {
        Ok(v) => v,
        Err(_) => {
            out.state = TextRuleState::BudgetExceeded;
            return out;
        }
    };
    let document = match parse_json(raw_layer, JsonMode::PublishedStrict, limits) {
        Ok(v) => v,
        Err(_) => {
            out.state = TextRuleState::InvalidInput;
            out.issue("invalid_published_json", &context.layer_path);
            return out;
        }
    };
    let layer = document.root();
    if string(layer, "schema_version") != Some(TEXT_LAYER_PROFILE) {
        out.unsupported_profiles.push(
            string(layer, "schema_version")
                .unwrap_or("missing-schema-version")
                .into(),
        );
        return out;
    }
    let layer_digest = Digest256::of_bytes(raw_layer).to_hex();
    out.packet_digest = Some(layer_digest.clone());
    out.reads.push(PredicateRead::ExactPath {
        path: context.layer_path.clone(),
        digest: layer_digest.clone(),
    });
    let id = string(layer, "layer_id").unwrap_or("");
    out.reads.push(PredicateRead::UniqueKey {
        namespace: "source-text-layer/id".into(),
        key: id.into(),
        owner: context.layer_path.clone(),
    });
    out.reads.push(PredicateRead::Range {
        namespace: format!("source-text-layer/{id}/bindings"),
        lower: String::new(),
        upper: "\u{10ffff}".into(),
        generation: context.reverse_generation.clone(),
    });
    if string(layer, "supersedes_layer_ref") == Some(id) {
        out.issue("layer_self_supersession", id);
    }
    let binding = field(layer, "source_binding").unwrap_or(&NULL_JSON);
    let anchors = rows(binding, "anchors");
    if anchors.len() > MAX_ROWS {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    let anchor_ids: BTreeSet<&str> = anchors
        .iter()
        .filter_map(|a| string(a, "anchor_id"))
        .collect();
    layer_binding_refs(
        id,
        anchors,
        "anchor_record_ref",
        "anchor_record_sha256",
        resources,
        &mut out,
    );
    if let (Some(source_ref), Some(digest)) = (
        string(binding, "source_file_ref"),
        string(binding, "source_file_sha256"),
    ) {
        layer_bound_resource(id, source_ref, digest, resources, &mut out);
        if source_ref != format!("tos.file.sha256.{digest}") {
            out.issue("layer_source_identity_drift", source_ref);
        }
    }
    let representation = field(layer, "representation").unwrap_or(&NULL_JSON);
    let content_ref = string(representation, "content_ref").unwrap_or("");
    let content_digest = string(representation, "content_sha256").unwrap_or("");
    let content_ok = layer_bound_resource(id, content_ref, content_digest, resources, &mut out);
    let content_resource = resources.iter().find(|r| r.locator == content_ref);
    if content_resource.is_some_and(|r| r.raw.len() > MAX_TEXT_BYTES) {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    let content = content_resource.and_then(|r| std::str::from_utf8(r.raw).ok());
    if content_resource.is_some() && content.is_none() {
        out.issue("layer_content_not_utf8", content_ref);
    }
    let text_scope = field(representation, "text_scope").unwrap_or(&NULL_JSON);
    let selected = match (content, span(text_scope)) {
        (Some(text), Some((start, end))) if start <= end => {
            let points: Vec<usize> = text
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
                .collect();
            out.reads.push(PredicateRead::Interval {
                scope: content_ref.into(),
                start: start as u64,
                end: end as u64,
                generation: context.interval_generation.clone(),
            });
            out.intervals.push(IntervalFact {
                scope: content_ref.into(),
                member: id.into(),
                start: start as u64,
                end: end as u64,
            });
            if end > points.len().saturating_sub(1) {
                out.issue("layer_scope_outside_content", id);
                None
            } else {
                Some(&text[points[start]..points[end]])
            }
        }
        (None, _) if content_resource.is_none() => None,
        _ => {
            out.issue("layer_scope_reversed_or_missing", id);
            None
        }
    };
    if content_ok {
        if let Some(normalization) = string(representation, "character_normalization") {
            if normalization != "none" {
                out.unsupported_profiles
                    .push(format!("unicode-normalization:{normalization}"));
            }
        }
    }
    let storage = string(representation, "storage");
    let tracked = boolean(representation, "tracked_content");
    let visibility = string(representation, "content_visibility");
    if (storage == Some("tracked")) != (tracked == Some(true)) {
        out.issue("layer_storage_tracking_disagree", id);
    }
    if tracked == Some(true) && visibility != Some("public") {
        out.issue("layer_tracked_nonpublic", id);
    }
    let published = boolean(representation, "publication_authorized") == Some(true);
    let publication_refs = rows(representation, "publication_authority_refs");
    if rows(representation, "rights_record_refs").len() > MAX_ROWS
        || publication_refs.len() > MAX_ROWS
    {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    if published && visibility != Some("public") {
        out.issue("layer_nonpublic_publication", id);
    }
    if published && publication_refs.is_empty() {
        out.issue("layer_publication_authority_missing", id);
    }
    if !published && !publication_refs.is_empty() {
        out.issue("layer_closed_publication_gate_conflict", id);
    }
    layer_binding_refs(
        id,
        rows(representation, "rights_record_refs"),
        "ref",
        "sha256",
        resources,
        &mut out,
    );
    layer_binding_refs(id, publication_refs, "ref", "sha256", resources, &mut out);
    let editorial = field(layer, "editorial_policy").unwrap_or(&NULL_JSON);
    if let (Some(reference), Some(digest)) = (
        string(editorial, "policy_ref"),
        string(editorial, "policy_sha256"),
    ) {
        layer_bound_resource(id, reference, digest, resources, &mut out);
    }
    let derivation = field(layer, "derivation").unwrap_or(&NULL_JSON);
    let mut maker_configurations = BTreeSet::new();
    layer_maker_configuration(
        id,
        field(derivation, "maker").unwrap_or(&NULL_JSON),
        resources,
        &mut maker_configurations,
        &mut out,
    );
    let inputs = rows(derivation, "input_layers");
    if inputs.len() > MAX_ROWS {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    let mut input_text = None;
    for input in inputs {
        let input_id = string(input, "layer_id").unwrap_or("");
        if input_id == id {
            out.issue("layer_self_derivation", id);
        }
        let record_ref = string(input, "record_ref").unwrap_or("");
        let record_digest = string(input, "record_sha256").unwrap_or("");
        if layer_bound_resource(id, record_ref, record_digest, resources, &mut out) {
            if let Some(record) = resources.iter().find(|r| r.locator == record_ref) {
                let parsed = JsonLimits::new(MAX_PACKET_BYTES, 64, 300_000, 4_300)
                    .ok()
                    .and_then(|l| parse_json(record.raw, JsonMode::PublishedStrict, l).ok());
                if let Some(parsed) = parsed {
                    let predecessor = parsed.root();
                    if string(predecessor, "layer_id") != Some(input_id) {
                        out.issue("layer_predecessor_identity_drift", record_ref);
                    }
                    let rep = field(predecessor, "representation").unwrap_or(&NULL_JSON);
                    if string(rep, "content_sha256") != string(input, "content_sha256") {
                        out.issue("layer_predecessor_content_digest_drift", record_ref);
                    }
                    if inputs.len() == 1 {
                        let predecessor_ref = string(rep, "content_ref").unwrap_or("");
                        let predecessor_digest = string(rep, "content_sha256").unwrap_or("");
                        if layer_bound_resource(
                            id,
                            predecessor_ref,
                            predecessor_digest,
                            resources,
                            &mut out,
                        ) {
                            if let Some(raw) =
                                resources.iter().find(|r| r.locator == predecessor_ref)
                            {
                                if raw.raw.len() > MAX_TEXT_BYTES {
                                    out.state = TextRuleState::BudgetExceeded;
                                    return out;
                                }
                                if let Ok(text) = std::str::from_utf8(raw.raw) {
                                    if let Some((s, e)) = field(rep, "text_scope").and_then(span) {
                                        let points: Vec<usize> = text
                                            .char_indices()
                                            .map(|(i, _)| i)
                                            .chain(std::iter::once(text.len()))
                                            .collect();
                                        if s <= e && e < points.len() {
                                            input_text =
                                                Some(text[points[s]..points[e]].to_owned());
                                        } else {
                                            out.issue(
                                                "layer_predecessor_scope_outside_content",
                                                input_id,
                                            );
                                        }
                                    }
                                } else {
                                    out.issue("layer_predecessor_not_utf8", predecessor_ref);
                                }
                            }
                        }
                    }
                } else {
                    out.issue("layer_predecessor_invalid_json", record_ref);
                }
            }
        }
        out.reads.push(PredicateRead::ReverseRefs {
            target: input_id.into(),
            relation: "source-text-layer/input".into(),
            generation: context.reverse_generation.clone(),
        });
        out.edge(input_id, "input_layer", id);
    }
    if let Some(last) = inputs.last() {
        if string(layer, "supersedes_layer_ref") != string(last, "layer_id") {
            out.issue("layer_immediate_supersession_drift", id);
        }
    }
    let payload = field(derivation, "change_payload").unwrap_or(&NULL_JSON);
    if string(payload, "kind") == Some("withheld_operations_receipt") {
        out.unsupported_profiles
            .push("withheld-operations-receipt".into());
    }
    if inputs.len() > 1 {
        out.unsupported_profiles
            .push("multiple-input-layers".into());
    }
    let operations = if string(payload, "kind") == Some("explicit_operations") {
        rows(payload, "operations")
    } else {
        &[]
    };
    if operations.len() > MAX_ROWS {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    if string(payload, "kind") == Some("explicit_operations") && inputs.len() != 1 {
        out.unsupported_profiles
            .push("explicit-operations-multiple-inputs".into());
    }
    let mut in_end = 0usize;
    let mut out_end = 0usize;
    let mut output_cursor = 0usize;
    let mut replay = String::new();
    let input_points: Option<Vec<usize>> = input_text.as_deref().map(|text| {
        text.char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .collect()
    });
    for operation in operations {
        let edit_id = string(operation, "edit_id").unwrap_or("");
        layer_maker_configuration(
            id,
            field(operation, "responsibility").unwrap_or(&NULL_JSON),
            resources,
            &mut maker_configurations,
            &mut out,
        );
        if string(operation, "operation") == Some("unicode_normalization") {
            out.unsupported_profiles
                .push("unicode-normalization-edit".into());
        }
        let input_span = field(operation, "input_span").and_then(span);
        let output_span = field(operation, "output_span").and_then(span);
        let input_exact = string(operation, "input_exact").unwrap_or("");
        let output_exact = string(operation, "output_exact").unwrap_or("");
        if string(operation, "input_sha256")
            != Some(
                Digest256::of_bytes(input_exact.as_bytes())
                    .to_hex()
                    .as_str(),
            )
        {
            out.issue("layer_edit_input_digest_drift", edit_id);
        }
        if string(operation, "output_sha256")
            != Some(
                Digest256::of_bytes(output_exact.as_bytes())
                    .to_hex()
                    .as_str(),
            )
        {
            out.issue("layer_edit_output_digest_drift", edit_id);
        }
        let (Some((ins, ine)), Some((outs, oute))) = (input_span, output_span) else {
            out.issue("layer_edit_span_missing", edit_id);
            continue;
        };
        if ine < ins || oute < outs || ins < in_end || outs < out_end {
            out.issue("layer_edit_span_order_drift", edit_id);
        }
        if ine.saturating_sub(ins) != input_exact.chars().count()
            || oute.saturating_sub(outs) != output_exact.chars().count()
        {
            out.issue("layer_edit_span_length_drift", edit_id);
        }
        for anchor in strings(operation, "evidence_anchor_refs") {
            if !anchor_ids.contains(anchor) {
                out.issue("layer_edit_anchor_outside_binding", edit_id);
            }
        }
        if rows(operation, "evidence_anchor_refs").is_empty() {
            out.issue("layer_edit_anchor_missing", edit_id);
        }
        if let (Some(text), Some(points)) = (input_text.as_deref(), input_points.as_deref()) {
            if in_end < points.len()
                && ins < points.len()
                && ine < points.len()
                && in_end <= ins
                && ins <= ine
            {
                let unchanged = &text[points[in_end]..points[ins]];
                replay.push_str(unchanged);
                output_cursor += ins - in_end;
                if &text[points[ins]..points[ine]] != input_exact {
                    out.issue("layer_edit_input_text_drift", edit_id);
                }
                if outs != output_cursor
                    || outs.checked_add(output_exact.chars().count()) != Some(oute)
                {
                    out.issue("layer_edit_output_alignment_drift", edit_id);
                }
                replay.push_str(output_exact);
                output_cursor += output_exact.chars().count();
            } else {
                out.issue("layer_edit_input_outside_predecessor", edit_id);
            }
        }
        in_end = in_end.max(ine);
        out_end = out_end.max(oute);
    }
    if let (Some(input), Some(output)) = (input_text.as_deref(), selected) {
        if !operations.is_empty() {
            if let Some(points) = input_points.as_deref() {
                if in_end < points.len() {
                    replay.push_str(&input[points[in_end]..]);
                }
            }
            if replay != output {
                out.issue("layer_edit_replay_output_drift", id);
            }
        }
        if string(derivation, "method") == Some("identity_copy") && input != output {
            out.issue("layer_identity_copy_output_drift", id);
        }
    }
    if string(derivation, "method") == Some("unicode_normalization") {
        out.unsupported_profiles.push("unicode-derivation".into());
    }
    if string(derivation, "method") == Some("editorial_normalization") {
        out.unsupported_profiles
            .push("editorial-normalization-derivation".into());
    }
    let uncertainty = field(layer, "uncertainty").unwrap_or(&NULL_JSON);
    if rows(uncertainty, "annotations").len() > MAX_ROWS {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    for annotation in rows(uncertainty, "annotations") {
        if let Some(anchor) = string(annotation, "anchor_ref") {
            if !anchor_ids.contains(anchor) {
                out.issue("layer_uncertainty_anchor_outside_binding", anchor);
            }
        }
        for alternative in rows(annotation, "alternatives") {
            let value = string(alternative, "value");
            let in_record = boolean(alternative, "value_in_record");
            if in_record == Some(true) && value.is_none() {
                out.issue("layer_uncertainty_value_missing", id);
            }
            if in_record == Some(false) && value.is_some() {
                out.issue("layer_uncertainty_withheld_value_exposed", id);
            }
            if let Some(value) = value {
                if string(alternative, "value_sha256")
                    != Some(Digest256::of_bytes(value.as_bytes()).to_hex().as_str())
                {
                    out.issue("layer_uncertainty_digest_drift", id);
                }
            }
        }
    }
    if out.reads.len() > 100_000
        || out.reverse_facts.len() > 25_000
        || out.intervals.len() > MAX_ROWS * 2
    {
        out.state = TextRuleState::BudgetExceeded;
    }
    if out.state != TextRuleState::BudgetExceeded {
        let reverse: BTreeSet<_> = out
            .reverse_facts
            .iter()
            .map(|fact| (fact.target.clone(), fact.relation.to_owned()))
            .collect();
        for (target, relation) in reverse {
            out.reads.push(PredicateRead::ReverseRefs {
                target,
                relation,
                generation: context.reverse_generation.clone(),
            });
        }
        if !out.issues.is_empty() {
            out.state = TextRuleState::InvalidInput;
        } else if out.unsupported_profiles.is_empty() {
            out.state = TextRuleState::Checked;
            out.checked_profiles.push(TEXT_LAYER_PROFILE.into());
        }
    }
    out
}
fn visibility_rank(value: Option<&str>) -> Option<u8> {
    match value? {
        "public" => Some(0),
        "public_metadata_only" => Some(1),
        "controlled" => Some(2),
        "local_only" => Some(3),
        "restricted" => Some(4),
        "unknown" => Some(5),
        _ => None,
    }
}
fn has(items: &[&str], target: &str) -> bool {
    items.contains(&target)
}

/// Inspect the complete packet-local v1 mechanic after an independently
/// completed schema check. FND PublishedStrict rejects duplicate decoded keys;
/// text is exact UTF-8 supplied by the snapshot reader. The explicit bounds
/// are family-local and do not replace the bounded schema worker.
pub fn inspect_source_text_unit_v1(
    raw_packet: &[u8],
    frozen_text: &[u8],
    context: &TextRuleContext,
) -> TextRuleReport {
    let mut out = TextRuleReport::new_for(TEXT_UNIT_RULE_ID);
    for profile in &context.requested_profiles {
        if profile != TEXT_UNIT_PROFILE {
            out.unsupported_profiles.push(profile.clone());
        }
    }
    if !context.schema_checked
        || out.unsupported_profiles.len() != 0
        || context.requested_profiles.is_empty()
        || context.packet_path.is_empty()
        || context.frozen_text_locator.is_empty()
        || context.interval_generation.is_empty()
        || context.reverse_generation.is_empty()
    {
        if !context.schema_checked {
            out.unsupported_profiles.push("schema-unchecked".into());
        }
        if context.requested_profiles.is_empty() {
            out.unsupported_profiles.push("no-profile".into());
        }
        if context.packet_path.is_empty()
            || context.frozen_text_locator.is_empty()
            || context.interval_generation.is_empty()
            || context.reverse_generation.is_empty()
        {
            out.unsupported_profiles
                .push("missing-snapshot-binding".into());
        }
        return out;
    }
    if raw_packet.len() > MAX_PACKET_BYTES || frozen_text.len() > MAX_TEXT_BYTES {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    let limits = match JsonLimits::new(MAX_PACKET_BYTES, 64, 300_000, 4_300) {
        Ok(limits) => limits,
        Err(_) => {
            out.state = TextRuleState::BudgetExceeded;
            return out;
        }
    };
    let document = match parse_json(raw_packet, JsonMode::PublishedStrict, limits) {
        Ok(value) => value,
        Err(_) => {
            out.state = TextRuleState::InvalidInput;
            out.issue("invalid_published_json", context.packet_path.clone());
            return out;
        }
    };
    let text = match std::str::from_utf8(frozen_text) {
        Ok(value) => value,
        Err(_) => {
            out.state = TextRuleState::InvalidInput;
            out.issue("invalid_frozen_utf8", context.frozen_text_locator.clone());
            return out;
        }
    };
    let packet = document.root();
    if string(packet, "schema_version") != Some(TEXT_UNIT_PROFILE) {
        out.unsupported_profiles.push(
            string(packet, "schema_version")
                .unwrap_or("missing-schema-version")
                .to_owned(),
        );
        return out;
    }
    let schemes = rows(packet, "schemes");
    let anchors = rows(packet, "anchors");
    let units = rows(packet, "units");
    let segmentations = rows(packet, "segmentations");
    let reviews = rows(packet, "reviews");
    let projections = rows(packet, "projections");
    if [
        schemes.len(),
        anchors.len(),
        units.len(),
        segmentations.len(),
        reviews.len(),
        projections.len(),
    ]
    .iter()
    .any(|n| *n > MAX_ROWS)
    {
        out.state = TextRuleState::BudgetExceeded;
        return out;
    }
    let packet_digest = Digest256::of_bytes(raw_packet).to_hex();
    out.packet_digest = Some(packet_digest.clone());
    let text_digest = Digest256::of_bytes(frozen_text).to_hex();
    out.reads.extend([
        PredicateRead::ExactPath {
            path: context.packet_path.clone(),
            digest: packet_digest.clone(),
        },
        PredicateRead::ExactBytes {
            locator: context.frozen_text_locator.clone(),
            digest: text_digest.clone(),
        },
        PredicateRead::Range {
            namespace: format!("text-unit/{packet_digest}/identities"),
            lower: String::new(),
            upper: "\u{10ffff}".into(),
            generation: packet_digest.clone(),
        },
        PredicateRead::Range {
            namespace: format!("text-unit/{packet_digest}/coverage"),
            lower: String::new(),
            upper: "\u{10ffff}".into(),
            generation: context.interval_generation.clone(),
        },
        PredicateRead::ReverseRefs {
            target: packet_digest.clone(),
            relation: "packet-local-text-unit-refs".into(),
            generation: context.reverse_generation.clone(),
        },
    ]);
    out.checked_profiles.push(TEXT_UNIT_PROFILE.into());
    out.state = TextRuleState::Checked;

    let source_layer = field(packet, "source_layer").unwrap_or(&NULL_JSON);
    let layer_ref = string(source_layer, "text_layer_ref").unwrap_or("");
    let layer_sha = string(source_layer, "text_layer_sha256").unwrap_or("");
    if layer_sha != text_digest {
        out.issue("source_layer_text_digest_mismatch", layer_ref);
    }
    if string(packet, "packet_id") == string(packet, "supersedes_packet_ref") {
        out.issue(
            "packet_self_supersession",
            string(packet, "packet_id").unwrap_or(""),
        );
    }

    let schemes_by = index(
        schemes,
        "scheme_id",
        "duplicate_scheme_id",
        &packet_digest,
        &mut out,
    );
    let anchors_by = index(
        anchors,
        "anchor_ref",
        "duplicate_anchor_id",
        &packet_digest,
        &mut out,
    );
    let units_by = index(
        units,
        "unit_id",
        "duplicate_unit_id",
        &packet_digest,
        &mut out,
    );
    let segmentations_by = index(
        segmentations,
        "segmentation_id",
        "duplicate_segmentation_id",
        &packet_digest,
        &mut out,
    );
    let reviews_by = index(
        reviews,
        "review_id",
        "duplicate_review_id",
        &packet_digest,
        &mut out,
    );
    let _projections_by = index(
        projections,
        "projection_id",
        "duplicate_projection_id",
        &packet_digest,
        &mut out,
    );

    let mut ordinals = BTreeSet::new();
    let mut anchor_intervals = BTreeMap::<String, (u64, u64)>::new();
    let codepoints: Vec<usize> = text
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect();
    let text_len = codepoints.len().saturating_sub(1) as u64;
    for anchor in anchors {
        let id = string(anchor, "anchor_ref").unwrap_or("");
        if let Some(ordinal) = field(anchor, "ordinal").and_then(JsonValue::as_u64) {
            out.reads.push(PredicateRead::UniqueKey {
                namespace: format!("text-unit/{packet_digest}/anchor-ordinal"),
                key: ordinal.to_string(),
                owner: TEXT_UNIT_RULE_ID.into(),
            });
            if !ordinals.insert(ordinal) {
                out.issue("duplicate_anchor_ordinal", id);
            }
        }
        if string(anchor, "text_layer_ref") != Some(layer_ref) {
            out.issue("anchor_layer_ref_mismatch", id);
        }
        if string(anchor, "text_layer_sha256") != Some(layer_sha) {
            out.issue("anchor_layer_digest_mismatch", id);
        }
        let Some((start, end)) = interval(anchor) else {
            continue;
        };
        if start > end {
            out.issue("anchor_reversed", id);
            continue;
        }
        if start == end && string(anchor, "anchor_role") != Some("milestone") {
            out.issue("anchor_empty_nonmilestone", id);
        }
        anchor_intervals.insert(id.to_owned(), (start, end));
        out.intervals.push(IntervalFact {
            scope: layer_ref.into(),
            member: id.into(),
            start,
            end,
        });
        out.reads.push(PredicateRead::Interval {
            scope: layer_ref.into(),
            start,
            end,
            generation: context.interval_generation.clone(),
        });
        if end > text_len {
            out.issue("anchor_outside_frozen_text", id);
        } else {
            let selected = &frozen_text[codepoints[start as usize]..codepoints[end as usize]];
            if string(anchor, "exact_sha256")
                != Some(Digest256::of_bytes(selected).to_hex().as_str())
            {
                out.issue("anchor_span_digest_mismatch", id);
            }
        }
    }

    for scheme in schemes {
        let id = string(scheme, "scheme_id").unwrap_or("");
        if nullable_same(scheme, "supersedes_scheme_ref", id) {
            out.issue("scheme_self_supersession", id);
        }
        let method = field(scheme, "method").unwrap_or(&NULL_JSON);
        if string(packet, "content_posture") != Some("public_synthetic_contract_exercise")
            && string(method, "maker_kind") == Some("synthetic_fixture")
        {
            out.issue("synthetic_scheme_outside_lab", id);
        }
    }

    for unit in units {
        let id = string(unit, "unit_id").unwrap_or("");
        if nullable_same(unit, "supersedes_unit_ref", id) {
            out.issue("unit_self_supersession", id);
        }
        refs(
            unit,
            "ordered_anchor_refs",
            &anchors_by,
            "missing_unit_anchor",
            "unit_anchor",
            id,
            &mut out,
        );
        let spans: Vec<(u64, u64)> = strings(unit, "ordered_anchor_refs")
            .into_iter()
            .filter_map(|r| anchor_intervals.get(r).copied())
            .collect();
        for pair in spans.windows(2) {
            if pair[1].0 < pair[0].1 {
                out.issue("unit_anchor_overlap_or_reverse", id);
            }
        }
        if string(unit, "continuity") == Some("contiguous")
            && spans.windows(2).any(|pair| pair[1].0 != pair[0].1)
        {
            out.issue("contiguous_unit_gap", id);
        }
        if string(unit, "continuity") == Some("discontinuous")
            && spans.len() > 1
            && spans.windows(2).all(|pair| pair[1].0 == pair[0].1)
        {
            out.issue("discontinuous_unit_no_gap", id);
        }
        refs(
            unit,
            "parent_unit_refs",
            &units_by,
            "missing_parent_unit",
            "unit_parent",
            id,
            &mut out,
        );
        refs(
            unit,
            "ordered_child_unit_refs",
            &units_by,
            "missing_child_unit",
            "unit_child",
            id,
            &mut out,
        );
        let parents = strings(unit, "parent_unit_refs");
        let children = strings(unit, "ordered_child_unit_refs");
        if has(&parents, id) {
            out.issue("unit_own_parent", id);
        }
        if has(&children, id) {
            out.issue("unit_own_child", id);
        }
        for parent in parents {
            if let Some(row) = units_by.get(parent) {
                if !has(&strings(row, "ordered_child_unit_refs"), id) {
                    out.issue("unit_parent_nonreciprocal", format!("{id} -> {parent}"));
                }
            }
        }
        for child in children {
            if let Some(row) = units_by.get(child) {
                if !has(&strings(row, "parent_unit_refs"), id) {
                    out.issue("unit_child_nonreciprocal", format!("{id} -> {child}"));
                }
            }
        }
    }

    for review in reviews {
        let id = string(review, "review_id").unwrap_or("");
        refs(
            review,
            "segmentation_refs",
            &segmentations_by,
            "missing_review_segmentation",
            "review_segmentation",
            id,
            &mut out,
        );
        refs(
            review,
            "reviewed_unit_refs",
            &units_by,
            "missing_reviewed_unit",
            "review_unit",
            id,
            &mut out,
        );
        if string(review, "source_layer_ref") != Some(layer_ref) {
            out.issue("review_layer_ref_mismatch", id);
        }
        if string(review, "source_layer_sha256") != Some(layer_sha) {
            out.issue("review_layer_digest_mismatch", id);
        }
        let competence = field(review, "language_competence").unwrap_or(&NULL_JSON);
        if string(competence, "language") != string(source_layer, "language") {
            out.issue("review_language_mismatch", id);
        }
        for seg in strings(review, "segmentation_refs") {
            if let Some(row) = segmentations_by.get(seg) {
                if !has(&strings(row, "review_refs"), id) {
                    out.issue(
                        "review_segmentation_nonreciprocal",
                        format!("{id} -> {seg}"),
                    );
                }
            }
        }
    }

    for segmentation in segmentations {
        let id = string(segmentation, "segmentation_id").unwrap_or("");
        if nullable_same(segmentation, "supersedes_segmentation_ref", id) {
            out.issue("segmentation_self_supersession", id);
        }
        let scheme_ref = string(segmentation, "scheme_ref").unwrap_or("");
        let scheme = schemes_by.get(scheme_ref).copied();
        out.endpoint(scheme_ref, "segmentation_scheme", id, scheme.is_some());
        if scheme.is_none() {
            out.issue("missing_segmentation_scheme", scheme_ref);
        }
        refs(
            segmentation,
            "ordered_unit_refs",
            &units_by,
            "missing_segmentation_unit",
            "segmentation_unit",
            id,
            &mut out,
        );
        let members = strings(segmentation, "ordered_unit_refs");
        if let Some(scheme) = scheme {
            let kinds = strings(scheme, "unit_kinds");
            for member in &members {
                if let Some(unit) = units_by.get(*member) {
                    if !has(&kinds, string(unit, "unit_kind").unwrap_or("")) {
                        out.issue("unit_kind_outside_scheme", format!("{id} -> {member}"));
                    }
                }
            }
        }
        refs(
            segmentation,
            "competing_segmentation_refs",
            &segmentations_by,
            "missing_competing_segmentation",
            "segmentation_competitor",
            id,
            &mut out,
        );
        for competitor in strings(segmentation, "competing_segmentation_refs") {
            if competitor == id {
                out.issue("segmentation_competes_self", id);
            } else if let Some(row) = segmentations_by.get(competitor) {
                if !has(&strings(row, "competing_segmentation_refs"), id) {
                    out.issue(
                        "segmentation_competition_nonreciprocal",
                        format!("{id} -> {competitor}"),
                    );
                }
            }
        }
        refs(
            segmentation,
            "review_refs",
            &reviews_by,
            "missing_segmentation_review",
            "segmentation_review",
            id,
            &mut out,
        );
        let review_refs = strings(segmentation, "review_refs");
        for review_ref in &review_refs {
            if let Some(row) = reviews_by.get(*review_ref) {
                if !has(&strings(row, "segmentation_refs"), id) {
                    out.issue(
                        "segmentation_review_nonreciprocal",
                        format!("{id} -> {review_ref}"),
                    );
                }
            }
        }
        let status = string(segmentation, "status").unwrap_or("");
        if ["partially_reviewed", "accepted"].contains(&status) && review_refs.is_empty() {
            out.issue("reviewed_segmentation_without_review", id);
        }
        if status == "accepted" {
            let accepted_reviews: Vec<&JsonValue> = review_refs
                .iter()
                .filter_map(|r| reviews_by.get(*r).copied())
                .filter(|r| string(r, "outcome") == Some("accepted"))
                .collect();
            let member_set: BTreeSet<&str> = members.iter().copied().collect();
            let full = accepted_reviews.iter().any(|r| {
                let reviewed: BTreeSet<&str> =
                    strings(r, "reviewed_unit_refs").into_iter().collect();
                string(r, "review_scope") == Some("all_units")
                    && reviewed == member_set
                    && string(r, "reviewer_kind") == Some("real_human")
                    && boolean(r, "source_visible") == Some(true)
                    && boolean(
                        r,
                        "independent_boundary_decision_recorded_before_assistance",
                    ) == Some(true)
                    && field(r, "language_competence").and_then(|c| boolean(c, "declared"))
                        == Some(true)
            });
            if !full {
                out.issue("accepted_segmentation_without_full_legacy_review", id);
            }
            for member in &members {
                if let Some(unit) = units_by.get(*member) {
                    if string(unit, "boundary_posture") != Some("reviewed_accepted") {
                        out.issue(
                            "accepted_segmentation_unaccepted_unit",
                            format!("{id} -> {member}"),
                        );
                    }
                }
            }
        }
        if status == "observed_source_structure" {
            let source_scheme = scheme
                .map(|row| {
                    ["source_layout", "source_structure"]
                        .contains(&string(row, "analysis_role").unwrap_or(""))
                        && ["source_layout", "source_markup"]
                            .contains(&string(row, "boundary_basis").unwrap_or(""))
                })
                .unwrap_or(false);
            if !source_scheme {
                out.issue("source_structure_scheme_mismatch", id);
            }
            for member in &members {
                if let Some(unit) = units_by.get(*member) {
                    if string(unit, "boundary_posture") != Some("source_attested") {
                        out.issue(
                            "source_structure_proposed_unit",
                            format!("{id} -> {member}"),
                        );
                    }
                }
            }
        }
        let coverage = field(segmentation, "coverage").unwrap_or(&NULL_JSON);
        let scope_ref = string(coverage, "scope_anchor_ref").unwrap_or("");
        out.endpoint(
            scope_ref,
            "coverage_scope",
            id,
            anchors_by.contains_key(scope_ref),
        );
        if !anchors_by.contains_key(scope_ref) {
            out.issue("missing_coverage_scope", scope_ref);
            continue;
        }
        let Some(scope) = anchor_intervals.get(scope_ref).copied() else {
            continue;
        };
        refs(
            coverage,
            "excluded_anchor_refs",
            &anchors_by,
            "missing_excluded_anchor",
            "coverage_excluded",
            id,
            &mut out,
        );
        let excluded = strings(coverage, "excluded_anchor_refs");
        let mut spans: Vec<(u64, u64, String)> = Vec::new();
        for member in &members {
            if let Some(unit) = units_by.get(*member) {
                if string(unit, "surface_posture") == Some("source_bearing") {
                    for anchor in strings(unit, "ordered_anchor_refs") {
                        if let Some((start, end)) = anchor_intervals.get(anchor).copied() {
                            spans.push((start, end, (*member).into()));
                        }
                    }
                }
            }
        }
        for anchor in &excluded {
            if let Some((start, end)) = anchor_intervals.get(*anchor).copied() {
                spans.push((start, end, format!("excluded:{anchor}")));
            }
        }
        for (start, end, member) in &spans {
            if *start < scope.0 || *end > scope.1 {
                out.issue("coverage_member_outside_scope", format!("{id} -> {member}"));
            }
        }
        spans.sort_by(|a, b| a.cmp(b));
        let overlap = spans.windows(2).any(|pair| pair[1].0 < pair[0].1);
        let posture = string(coverage, "coverage_posture").unwrap_or("");
        if ["exhaustive_nonoverlapping", "declared_partial"].contains(&posture) && overlap {
            out.issue("coverage_undeclared_overlap", id);
        }
        if posture == "overlap_declared"
            && scheme.is_some()
            && scheme
                .and_then(|s| field(s, "policies"))
                .and_then(|p| string(p, "overlap"))
                != Some("allow_declared")
        {
            out.issue("coverage_overlap_policy_mismatch", id);
        }
        let mut merged: Vec<(u64, u64)> = Vec::new();
        for (start, end, _) in &spans {
            if let Some(last) = merged.last_mut() {
                if *start <= last.1 {
                    last.1 = last.1.max(*end);
                    continue;
                }
            }
            merged.push((*start, *end));
        }
        if merged != vec![scope] {
            out.issue("coverage_hidden_gap_or_extent", id);
        }
        if posture != "declared_partial" && !excluded.is_empty() {
            out.issue("coverage_nonpartial_exclusion", id);
        }
        if posture == "declared_partial" && excluded.is_empty() {
            out.issue("coverage_partial_without_exclusion", id);
        }
        out.reads.push(PredicateRead::Interval {
            scope: format!("{layer_ref}/{id}"),
            start: scope.0,
            end: scope.1,
            generation: context.interval_generation.clone(),
        });
    }

    let referenced: BTreeSet<&str> = segmentations
        .iter()
        .flat_map(|s| strings(s, "ordered_unit_refs"))
        .collect();
    for id in units_by.keys() {
        if !referenced.contains(id.as_str()) {
            out.issue("unit_without_segmentation", id);
        }
    }
    for (items, id_key, predecessor_key, code) in [
        (
            schemes,
            "scheme_id",
            "supersedes_scheme_ref",
            "scheme_supersession_cycle",
        ),
        (
            units,
            "unit_id",
            "supersedes_unit_ref",
            "unit_supersession_cycle",
        ),
        (
            segmentations,
            "segmentation_id",
            "supersedes_segmentation_ref",
            "segmentation_supersession_cycle",
        ),
    ] {
        let by: BTreeMap<&str, &JsonValue> = items
            .iter()
            .filter_map(|row| Some((string(row, id_key)?, row)))
            .collect();
        for row in items {
            let id = string(row, id_key).unwrap_or("");
            let mut cursor = string(row, predecessor_key);
            let mut seen = BTreeSet::from([id]);
            while let Some(parent) = cursor {
                if !by.contains_key(parent) {
                    break;
                }
                if !seen.insert(parent) {
                    out.issue(code, id);
                    break;
                }
                out.edge(parent, "supersedes", id);
                cursor = by
                    .get(parent)
                    .and_then(|next| string(next, predecessor_key));
            }
        }
    }

    let rights = field(packet, "rights_and_visibility").unwrap_or(&NULL_JSON);
    if string(rights, "source_visibility") != string(source_layer, "visibility") {
        out.issue("source_packet_visibility_mismatch", layer_ref);
    }
    let source_rank = visibility_rank(string(rights, "source_visibility"));
    let packet_rank = visibility_rank(string(rights, "packet_visibility"));
    let effective_rank = visibility_rank(string(rights, "effective_visibility"));
    if let (Some(a), Some(b), Some(e)) = (source_rank, packet_rank, effective_rank) {
        if e != a.max(b) {
            out.issue("effective_visibility_not_most_restrictive", layer_ref);
        }
    }
    if boolean(rights, "publication_authorized") == Some(true)
        && (string(rights, "effective_visibility") != Some("public")
            || boolean(rights, "private_source_used") == Some(true)
            || boolean(source_layer, "publication_authorized") != Some(true))
    {
        out.issue("publication_boundary_widened", layer_ref);
    }
    for projection in projections {
        let id = string(projection, "projection_id").unwrap_or("");
        refs(
            projection,
            "source_segmentation_refs",
            &segmentations_by,
            "missing_projection_segmentation",
            "projection_segmentation",
            id,
            &mut out,
        );
        for seg in strings(projection, "source_segmentation_refs") {
            if (string(projection, "projection_kind") == Some("graph")
                || string(projection, "admission_posture") == Some("accepted_only"))
                && segmentations_by
                    .get(seg)
                    .is_some_and(|row| string(row, "status") != Some("accepted"))
            {
                out.issue(
                    "projection_unaccepted_segmentation",
                    format!("{id} -> {seg}"),
                );
            }
        }
        if let (Some(e), Some(p)) = (
            effective_rank,
            visibility_rank(string(projection, "visibility")),
        ) {
            if p < e {
                out.issue("projection_visibility_widened", id);
            }
        }
    }
    if out.issues.len() > MAX_ISSUES
        || out.intervals.len() > MAX_ROWS * 2
        || out.reverse_facts.len() > MAX_ROWS * 40
    {
        out.state = TextRuleState::BudgetExceeded;
        out.issues.truncate(MAX_ISSUES);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn fixture(name: &str) -> (Vec<u8>, Vec<u8>) {
        let lab = root()
            .join("ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc");
        (
            std::fs::read(lab.join(name)).unwrap(),
            std::fs::read(lab.join("public-synthetic-source.x-tos-unit.txt")).unwrap(),
        )
    }

    fn context(schema_checked: bool) -> TextRuleContext {
        TextRuleContext {
            packet_path: "fixture/packet.json".into(),
            frozen_text_locator: "fixture/source.txt".into(),
            schema_checked,
            requested_profiles: vec![TEXT_UNIT_PROFILE.into()],
            interval_generation: "fixture-interval-generation".into(),
            reverse_generation: "fixture-reverse-generation".into(),
        }
    }

    fn case(value: &Value, text: &[u8]) -> TextRuleReport {
        inspect_source_text_unit_v1(&serde_json::to_vec(value).unwrap(), text, &context(true))
    }

    fn has_issue(report: &TextRuleReport, code: &str) -> bool {
        report.issues.iter().any(|issue| issue.code == code)
    }

    #[test]
    fn public_source_layout_and_competing_segmentations_are_mechanically_clean() {
        for file in [
            "variant-a-source-layout-observation.json",
            "variant-b-competing-segmentations.json",
        ] {
            let (raw, text) = fixture(file);
            let report = inspect_source_text_unit_v1(&raw, &text, &context(true));
            assert_eq!(
                report.state,
                TextRuleState::Checked,
                "{file}: {:?}",
                report.issues
            );
            assert!(report.issues.is_empty(), "{file}: {:?}", report.issues);
            assert_eq!(report.checked_profiles, vec![TEXT_UNIT_PROFILE]);
            assert!(!report.intervals.is_empty());
            assert!(!report.reverse_facts.is_empty());
        }
    }

    #[test]
    fn independent_python_oracle_mutations_have_same_mechanical_classes() {
        let (raw, text) = fixture("variant-b-competing-segmentations.json");
        let base: Value = serde_json::from_slice(&raw).unwrap();
        let mut duplicate = base.clone();
        let first = duplicate["units"][0].clone();
        duplicate["units"].as_array_mut().unwrap().push(first);
        assert!(has_issue(&case(&duplicate, &text), "duplicate_unit_id"));

        let mut absent_scheme = base.clone();
        absent_scheme["segmentations"][0]["scheme_ref"] =
            Value::String("unallocated-scheme".into());
        let report = case(&absent_scheme, &text);
        assert!(has_issue(&report, "missing_segmentation_scheme"));
        assert!(
            report.reads.iter().any(
                |r| matches!(r, PredicateRead::AbsentKey { key, .. } if key == "unallocated-scheme")
            ) || report
                .reverse_facts
                .iter()
                .any(|f| f.target == "unallocated-scheme")
        );

        let mut absent_anchor = base.clone();
        absent_anchor["units"][0]["ordered_anchor_refs"][0] =
            Value::String("unallocated-anchor".into());
        assert!(has_issue(
            &case(&absent_anchor, &text),
            "missing_unit_anchor"
        ));

        let mut span_digest = base.clone();
        span_digest["anchors"][1]["exact_sha256"] = Value::String("1".repeat(64));
        assert!(has_issue(
            &case(&span_digest, &text),
            "anchor_span_digest_mismatch"
        ));

        let mut layer_digest = base.clone();
        layer_digest["source_layer"]["text_layer_sha256"] = Value::String("0".repeat(64));
        assert!(has_issue(
            &case(&layer_digest, &text),
            "source_layer_text_digest_mismatch"
        ));

        let mut orphan = base.clone();
        let mut other = orphan["units"][0].clone();
        other["unit_id"] = Value::String("new-unallocated-unit".into());
        orphan["units"].as_array_mut().unwrap().push(other);
        assert!(has_issue(
            &case(&orphan, &text),
            "unit_without_segmentation"
        ));

        let mut rights = base;
        rights["rights_and_visibility"]["private_source_used"] = Value::Bool(true);
        assert!(has_issue(
            &case(&rights, &text),
            "publication_boundary_widened"
        ));
    }

    #[test]
    fn unchecked_schema_unknown_profile_and_duplicate_keys_never_pass() {
        let (raw, text) = fixture("variant-a-source-layout-observation.json");
        let unchecked = inspect_source_text_unit_v1(&raw, &text, &context(false));
        assert_eq!(unchecked.state, TextRuleState::Unsupported);
        assert!(unchecked.checked_profiles.is_empty());

        let mut unknown = context(true);
        unknown
            .requested_profiles
            .push("tos_named_transfer_bridge_v1".into());
        let report = inspect_source_text_unit_v1(&raw, &text, &unknown);
        assert_eq!(report.state, TextRuleState::Unsupported);
        assert_eq!(
            report.unsupported_profiles,
            vec!["tos_named_transfer_bridge_v1"]
        );

        let bad = br#"{"schema_version":"tos_source_text_unit_packet_v1","schema_version":"tos_source_text_unit_packet_v1"}"#;
        let report = inspect_source_text_unit_v1(bad, &text, &context(true));
        assert_eq!(report.state, TextRuleState::InvalidInput);
        assert!(has_issue(&report, "invalid_published_json"));
    }

    fn layer_context(schema_checked: bool) -> LayerRuleContext {
        LayerRuleContext {
            layer_path: "fixture/layer.json".into(),
            schema_checked,
            requested_profiles: vec![TEXT_LAYER_PROFILE.into()],
            interval_generation: "fixture-layer-interval-generation".into(),
            reverse_generation: "fixture-layer-reverse-generation".into(),
        }
    }

    fn layer_fixture(name: &str) -> (Vec<u8>, Vec<(String, Vec<u8>)>) {
        let prefix = "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc";
        let raw = std::fs::read(root().join(format!("{prefix}/{name}"))).unwrap();
        let value: Value = serde_json::from_slice(&raw).unwrap();
        let mut refs = Vec::new();
        let source_ref = value["source_binding"]["source_file_ref"].as_str().unwrap();
        refs.push((source_ref.to_owned(), std::fs::read(root().join("ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-b-unicode.txt")).unwrap()));
        for anchor in value["source_binding"]["anchors"].as_array().unwrap() {
            let path = anchor["anchor_record_ref"].as_str().unwrap();
            refs.push((path.into(), std::fs::read(root().join(path)).unwrap()));
        }
        let path = value["representation"]["content_ref"].as_str().unwrap();
        refs.push((path.into(), std::fs::read(root().join(path)).unwrap()));
        let path = value["editorial_policy"]["policy_ref"].as_str().unwrap();
        refs.push((path.into(), std::fs::read(root().join(path)).unwrap()));
        for input in value["derivation"]["input_layers"].as_array().unwrap() {
            let path = input["record_ref"].as_str().unwrap();
            let record = std::fs::read(root().join(path)).unwrap();
            let predecessor: Value = serde_json::from_slice(&record).unwrap();
            refs.push((path.into(), record));
            let path = predecessor["representation"]["content_ref"]
                .as_str()
                .unwrap();
            refs.push((path.into(), std::fs::read(root().join(path)).unwrap()));
        }
        (raw, refs)
    }

    fn layer_case(raw: &[u8], owned: &[(String, Vec<u8>)]) -> TextRuleReport {
        let resources: Vec<_> = owned
            .iter()
            .map(|(locator, raw)| LayerResource { locator, raw })
            .collect();
        inspect_source_text_layer_v1(raw, &resources, &layer_context(true))
    }

    #[test]
    fn text_layer_raw_ocr_closed_and_unicode_declared_unsupported() {
        let (raw, resources) = layer_fixture("variant-a.layer.json");
        let report = layer_case(&raw, &resources);
        assert_eq!(report.state, TextRuleState::Checked, "{:?}", report.issues);
        assert!(report.issues.is_empty());
        assert!(!report.reads.is_empty());
        assert_eq!(report.intervals.len(), 1);

        let (raw, resources) = layer_fixture("variant-b.layer.json");
        let report = layer_case(&raw, &resources);
        assert_eq!(
            report.state,
            TextRuleState::Unsupported,
            "{:?}",
            report.issues
        );
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        assert!(
            report
                .unsupported_profiles
                .iter()
                .any(|p| p == "unicode-normalization:NFD")
        );
    }

    #[test]
    fn text_layer_oracle_mutations_and_missing_snapshot_fail_closed() {
        let (raw, mut resources) = layer_fixture("variant-a.layer.json");
        let mut value: Value = serde_json::from_slice(&raw).unwrap();
        value["representation"]["content_sha256"] = Value::String("0".repeat(64));
        let report = layer_case(&serde_json::to_vec(&value).unwrap(), &resources);
        assert!(has_issue(&report, "layer_resource_digest_drift"));

        let mut value: Value = serde_json::from_slice(&raw).unwrap();
        value["representation"]["content_visibility"] = Value::String("local_only".into());
        let report = layer_case(&serde_json::to_vec(&value).unwrap(), &resources);
        assert!(has_issue(&report, "layer_tracked_nonpublic"));

        resources.pop();
        let report = layer_case(&raw, &resources);
        assert_eq!(report.state, TextRuleState::Unsupported);
        assert!(!report.unsupported_profiles.is_empty());

        let (raw_b, resources_b) = layer_fixture("variant-b.layer.json");
        let mut b: Value = serde_json::from_slice(&raw_b).unwrap();
        b["derivation"]["change_payload"]["operations"][0]["input_span"]["start"] = Value::from(5);
        let report = layer_case(&serde_json::to_vec(&b).unwrap(), &resources_b);
        assert!(has_issue(&report, "layer_edit_span_order_drift"));

        let (raw_a, mut resources_a) = layer_fixture("variant-a.layer.json");
        resources_a.retain(|(reference, _)| !reference.ends_with("variant-a-raw-ocr.txt"));
        let report = layer_case(&raw_a, &resources_a);
        assert_eq!(report.state, TextRuleState::Unsupported);
        assert!(!has_issue(&report, "layer_scope_reversed_or_missing"));

        let complete = layer_fixture("variant-a.layer.json");
        let borrowed: Vec<_> = complete
            .1
            .iter()
            .map(|(locator, raw)| LayerResource { locator, raw })
            .collect();
        let report = inspect_source_text_layer_v1(&complete.0, &borrowed, &layer_context(false));
        assert_eq!(report.state, TextRuleState::Unsupported);
    }

    fn anchor_fixture() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let lab =
            root().join("ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc");
        (
            std::fs::read(lab.join("variant-b.anchor.json")).unwrap(),
            std::fs::read(lab.join("variant-b-unicode.txt")).unwrap(),
            std::fs::read(lab.join("lab.manifest.json")).unwrap(),
        )
    }

    fn anchor_context() -> AnchorRuleContext {
        AnchorRuleContext {
            anchor_path: "fixture/variant-b.anchor.json".into(),
            target_locator: "fixture:variant-b-unicode".into(),
            method_configuration_locator: "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/lab.manifest.json".into(),
            schema_checked: true,
            requested_profiles: vec![ANCHOR_V2_PROFILE.into()],
            interval_generation: "fixture-anchor-interval".into(),
            reverse_generation: "fixture-anchor-reverse".into(),
        }
    }

    fn anchor_case(value: &Value, target: &[u8], config: &[u8]) -> TextRuleReport {
        inspect_source_anchor_v2_single(
            &serde_json::to_vec(value).unwrap(),
            target,
            config,
            &anchor_context(),
        )
    }

    fn selection_digest(report: &TextRuleReport) -> Option<&str> {
        report.reads.iter().find_map(|read| match read {
            PredicateRead::ExactBytes { locator, digest }
                if locator.starts_with("anchor-selection:") =>
            {
                Some(digest.as_str())
            }
            _ => None,
        })
    }

    #[test]
    fn anchor_v2_frozen_unicode_and_byte_oracle_selections() {
        let (raw, target, config) = anchor_fixture();
        let base: Value = serde_json::from_slice(&raw).unwrap();
        let report = anchor_case(&base, &target, &config);
        assert_eq!(report.state, TextRuleState::Checked, "{:?}", report.issues);
        assert_eq!(
            selection_digest(&report),
            Some("81ef060bcd98adc7824eb5c1ada83c32491b16018e11e79f00ab9d09e04b015a")
        );
        assert_eq!(report.intervals[0].start, 4);
        assert_eq!(report.intervals[0].end, 9);

        let mut bytes = base.clone();
        bytes["selector_payload"]["expression"]["selector"]["selector"] = serde_json::json!({
            "type": "byte_position", "start": 0, "end": 5,
            "position_unit": "byte", "interval": "half_open"
        });
        let report = anchor_case(&bytes, &target, &config);
        assert_eq!(report.state, TextRuleState::Checked, "{:?}", report.issues);
        assert_eq!(
            selection_digest(&report),
            Some("3c94a0388b52318853918f13698b0677f047c0f298aba9732f8f3f637f652084")
        );

        let mut quote = base;
        quote["selector_payload"]["expression"]["selector"]["selector"] =
            serde_json::json!({"type": "text_quote", "exact": "cafe\u{301}"});
        quote["publication_boundary"]["source_text_in_record"] = Value::Bool(true);
        let report = anchor_case(&quote, &target, &config);
        assert_eq!(report.state, TextRuleState::Checked, "{:?}", report.issues);
        assert_eq!(
            selection_digest(&report),
            Some("81ef060bcd98adc7824eb5c1ada83c32491b16018e11e79f00ab9d09e04b015a")
        );
    }

    #[test]
    fn anchor_v2_negative_oracle_and_unsupported_profiles() {
        let (raw, target, config) = anchor_fixture();
        let base: Value = serde_json::from_slice(&raw).unwrap();
        let mut drift = base.clone();
        drift["selector_payload"]["expression"]["selector"]["state"]["representation_sha256"] =
            Value::String("0".repeat(64));
        assert!(has_issue(
            &anchor_case(&drift, &target, &config),
            "anchor_selector_state_drift"
        ));

        let mut reverse = base.clone();
        reverse["selector_payload"]["expression"]["selector"]["selector"]["start"] =
            Value::from(10);
        assert!(has_issue(
            &anchor_case(&reverse, &target, &config),
            "anchor_text_span_outside_target"
        ));

        let mut utf16 = base.clone();
        utf16["selector_payload"]["expression"]["selector"]["selector"]["start"] = Value::from(5);
        utf16["selector_payload"]["expression"]["selector"]["selector"]["end"] = Value::from(10);
        let report = anchor_case(&utf16, &target, &config);
        assert_eq!(report.state, TextRuleState::Checked);
        assert_eq!(
            selection_digest(&report),
            Some("db9950f9a1575823de691f2152032a848d3a498b905a46d22c278eae0ff08cff")
        );

        let mut quote = base.clone();
        quote["selector_payload"]["expression"]["selector"]["selector"] =
            serde_json::json!({"type": "text_quote", "exact": "cafe\u{301}"});
        quote["publication_boundary"]["source_text_in_record"] = Value::Bool(true);
        quote["publication_boundary"]["source_content_visibility"] =
            Value::String("local_only".into());
        assert!(has_issue(
            &anchor_case(&quote, &target, &config),
            "anchor_tracked_nonpublic_quote"
        ));

        let mut normalized = base.clone();
        normalized["selector_payload"]["expression"]["selector"]["state"]["character_normalization"] =
            Value::String("NFC".into());
        assert_eq!(
            anchor_case(&normalized, &target, &config).state,
            TextRuleState::Unsupported
        );

        let mut combining_edge = base.clone();
        combining_edge["selector_payload"]["expression"]["selector"]["selector"]["start"] =
            Value::from(8);
        assert!(
            anchor_case(&combining_edge, &target, &config)
                .unsupported_profiles
                .iter()
                .any(|p| p == "non-ascii-selector-boundary")
        );

        let mut alternatives = base.clone();
        alternatives["selector_payload"]["expression"] = serde_json::json!({
            "mode": "alternatives", "alternatives": []
        });
        assert_eq!(
            anchor_case(&alternatives, &target, &config).state,
            TextRuleState::Unsupported
        );

        let mut context = anchor_context();
        context.schema_checked = false;
        assert_eq!(
            inspect_source_anchor_v2_single(&raw, &target, &config, &context).state,
            TextRuleState::Unsupported
        );
    }
}
