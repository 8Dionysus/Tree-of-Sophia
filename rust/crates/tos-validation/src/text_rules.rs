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
    fn new() -> Self {
        Self {
            rule_id: TEXT_UNIT_RULE_ID,
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
fn strings(value: &JsonValue, key: &str) -> Vec<&str> {
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
    let mut out = TextRuleReport::new();
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
}
