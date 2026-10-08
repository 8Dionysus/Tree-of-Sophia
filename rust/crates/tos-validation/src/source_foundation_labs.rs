//! Source-foundation laboratory routes and the three default authored bridges.
//!
//! This module is an adapter over an authenticated `LayerFamilySource`. It
//! does not open a checkout, discover an alternate corpus, or create source
//! admission. Individual lab selection preserves the Python validator's
//! first-flag-wins order; the default route preserves its ordered concatenation.

use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::layer_family_rules::{LayerFamilyReport, LayerFamilyRules, LayerFamilySource};
use crate::source_foundation_default_rules::{SliceDefaultPaths, SourceFoundationDefaultPaths};
use crate::source_foundation_discovery::SourcePhysicalFacts;
use serde_json::{Value, json};
use std::io::{self, Write};
use std::sync::Arc;
use tos_foundation::{Digest256, RelativePath};
use unicode_normalization::UnicodeNormalization;

struct JsonSizeCounter {
    written: usize,
    limit: usize,
}

struct BoundedVecWriter<'a> {
    output: &'a mut Vec<u8>,
    limit: usize,
}

impl Write for JsonSizeCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .filter(|written| *written <= self.limit)
            .ok_or_else(|| io::Error::other("bounded JSON output exceeded"))?;
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for BoundedVecWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .output
            .len()
            .checked_add(bytes.len())
            .filter(|written| *written <= self.limit)
            .ok_or_else(|| io::Error::other("bounded JSON output exceeded"))?;
        self.output.extend_from_slice(bytes);
        debug_assert_eq!(self.output.len(), next);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bounded_json_encoded_len(value: &Value, limit: usize) -> Result<usize, ItemRefusal> {
    let mut counter = JsonSizeCounter { written: 0, limit };
    serde_json::to_writer(&mut counter, value).map_err(|_| ItemRefusal::Budget)?;
    Ok(counter.written)
}

fn write_bounded_json(
    value: &Value,
    limit: usize,
    expected_len: usize,
) -> Result<Vec<u8>, ItemRefusal> {
    let mut output = Vec::with_capacity(expected_len);
    serde_json::to_writer(
        BoundedVecWriter {
            output: &mut output,
            limit,
        },
        value,
    )
    .map_err(|_| ItemRefusal::Budget)?;
    if output.len() != expected_len {
        return Err(ItemRefusal::Budget);
    }
    Ok(output)
}

fn estimate_value_storage(value: &Value) -> Result<usize, ItemRefusal> {
    fn add(total: &mut usize, amount: usize) -> Result<(), ItemRefusal> {
        *total = total.checked_add(amount).ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn walk(value: &Value, total: &mut usize, depth: usize) -> Result<(), ItemRefusal> {
        if depth > 128 {
            return Err(ItemRefusal::Budget);
        }
        add(total, std::mem::size_of::<Value>())?;
        match value {
            Value::String(text) => {
                add(
                    total,
                    text.len()
                        .checked_mul(2)
                        .and_then(|bytes| bytes.checked_add(32))
                        .ok_or(ItemRefusal::Budget)?,
                )?;
            }
            Value::Array(rows) => {
                let slots = rows
                    .len()
                    .checked_mul(std::mem::size_of::<Value>())
                    .and_then(|bytes| bytes.checked_mul(2))
                    .and_then(|bytes| bytes.checked_add(64))
                    .ok_or(ItemRefusal::Budget)?;
                add(total, slots)?;
                for row in rows {
                    walk(row, total, depth + 1)?;
                }
            }
            Value::Object(map) => {
                for (key, row) in map {
                    add(
                        total,
                        key.len()
                            .checked_mul(2)
                            .and_then(|bytes| bytes.checked_add(192))
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    walk(row, total, depth + 1)?;
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
        Ok(())
    }

    let mut estimate = 0;
    walk(value, &mut estimate, 0)?;
    Ok(estimate)
}

fn estimate_text_storage(text: &str) -> Result<usize, ItemRefusal> {
    text.len()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(32))
        .ok_or(ItemRefusal::Budget)
}

fn schema_check_storage(check: &SourceFoundationSchemaCheck) -> Result<usize, ItemRefusal> {
    let mut bytes = std::mem::size_of::<SourceFoundationSchemaCheck>();
    for text in [
        Some(check.location.as_str()),
        Some(check.contract.as_str()),
        check.negative_control.as_deref(),
        check.report_slot.as_deref(),
        check.rejection_reasons_slot.as_deref(),
        check.schema_message_prefix.as_deref(),
        check.mismatch_message.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        bytes = bytes
            .checked_add(estimate_text_storage(text)?)
            .ok_or(ItemRefusal::Budget)?;
    }
    bytes = bytes
        .checked_add(estimate_value_storage(&check.instance)?)
        .ok_or(ItemRefusal::Budget)?;
    Ok(bytes)
}

fn json_parse_owner_message(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Syntax => "cannot read JSON: invalid JSON syntax",
        serde_json::error::Category::Data => "cannot read JSON: invalid JSON data",
        serde_json::error::Category::Eof => "cannot read JSON: incomplete JSON",
        serde_json::error::Category::Io => "cannot read JSON: input failure",
    }
}

fn synthetic_resolution_reason(error: &str) -> &'static str {
    match error {
        "selector state is absent" => "selector state is absent",
        "selector state representation is absent from the laboratory manifest" => {
            "selector state representation is absent from the laboratory manifest"
        }
        "selector state digest differs from the represented bytes" => {
            "selector state digest differs from the represented bytes"
        }
        "selector state media type differs from the represented bytes" => {
            "selector state media type differs from the represented bytes"
        }
        "selector expression is absent" => "selector expression is absent",
        "selector is absent" => "selector is absent",
        "selector type is absent" => "selector type is absent",
        "selector state" => "invalid selector state",
        "text quote exact text is absent" => "text quote exact text is absent",
        "text quote does not resolve exactly once with its context" => {
            "text quote does not resolve exactly once with its context"
        }
        "text quote start splits a combining sequence" => {
            "text quote start splits a combining sequence"
        }
        "text quote end splits a combining sequence" => {
            "text quote end splits a combining sequence"
        }
        "text-position interval is outside the selected representation" => {
            "text-position interval is outside the selected representation"
        }
        "text-position start splits a combining sequence" => {
            "text-position start splits a combining sequence"
        }
        "text-position end splits a combining sequence" => {
            "text-position end splits a combining sequence"
        }
        "byte-position interval is outside the selected representation" => {
            "byte-position interval is outside the selected representation"
        }
        "container member is not declared by the selected container" => {
            "container member is not declared by the selected container"
        }
        "container member is absent from the laboratory manifest" => {
            "container member is absent from the laboratory manifest"
        }
        "container member digest differs from selector" => {
            "container member digest differs from selector"
        }
        "container member media type differs from selector" => {
            "container member media type differs from selector"
        }
        "alternative selector set is absent" => "alternative selector set is absent",
        "alternative selector set is empty" => "alternative selector set is empty",
        "alternative selectors resolve to different segments" => {
            "alternative selectors resolve to different segments"
        }
        "refinement chain is absent" => "refinement chain is absent",
        "refinement chain is empty" => "refinement chain is empty",
        "refinement step state differs from the prior selected representation" => {
            "refinement step state differs from the prior selected representation"
        }
        "container refinement does not bind the selected member state" => {
            "container refinement does not bind the selected member state"
        }
        "synthetic laboratory does not implement arbitrary fragment standards" => {
            "synthetic laboratory does not implement arbitrary fragment standards"
        }
        "JSON Pointer must begin with /" => "JSON Pointer must begin with /",
        "text-layer representation is not UTF-8 text" => {
            "text-layer representation is not UTF-8 text"
        }
        value if value.starts_with("unsupported character normalization:") => {
            "unsupported character normalization"
        }
        value if value.starts_with("unsupported synthetic structural scheme:") => {
            "unsupported synthetic structural scheme"
        }
        value if value.starts_with("unsupported selector type:") => "unsupported selector type",
        value if value.starts_with("unsupported selector expression mode:") => {
            "unsupported selector expression mode"
        }
        _ => "invalid synthetic selector expression",
    }
}

fn text_replay_owner_reason(error: &str) -> &'static str {
    let code = error.split_once(':').map_or(error, |(code, _)| code);
    match code {
        "layer_edit_input_digest_drift" => "input text digest drifted",
        "layer_edit_output_digest_drift" => "output text digest drifted",
        "layer_edit_span_missing" => "edit span is absent",
        "layer_edit_span_order_drift" => "edit spans are out of order",
        "layer_edit_span_length_drift" => "edit span length drifted",
        "layer_edit_anchor_outside_binding" => "edit anchor is outside source binding",
        "layer_edit_anchor_missing" => "edit anchor is absent",
        "layer_edit_input_text_drift" => "input text differs from selected predecessor",
        "layer_edit_output_alignment_drift" => "output alignment drifted",
        "layer_edit_input_outside_predecessor" => "input span is outside predecessor",
        "layer_edit_replay_output_drift" => "replay output drifted",
        "layer_edit_required_fields" => "edit is missing required fields",
        "source text layer predecessor text missing" => {
            "source text layer predecessor text missing"
        }
        _ => "source text replay could not be verified",
    }
}

/// The six public synthetic lab routes and the three mandatory default bridges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceFoundationLab {
    SourceAnchorV2,
    SourceTextLayer,
    ProvenanceV2,
    SemanticAnnotationV2,
    TranslationAlignmentV1,
    SourceTextUnitV1,
    ZarathustraOpeningSentence,
    AntonovskyCollation,
    ZarathustraAuthoredCanonBridge,
}

impl SourceFoundationLab {
    /// Existing CLI branch order. The first selected lab flag wins.
    pub const LAB_ONLY_ORDER: [Self; 6] = [
        Self::SourceAnchorV2,
        Self::SourceTextLayer,
        Self::ProvenanceV2,
        Self::SemanticAnnotationV2,
        Self::TranslationAlignmentV1,
        Self::SourceTextUnitV1,
    ];

    /// Existing maintained-foundation concatenation order.
    pub const DEFAULT_ORDER: [Self; 9] = [
        Self::SourceAnchorV2,
        Self::SourceTextLayer,
        Self::ProvenanceV2,
        Self::SemanticAnnotationV2,
        Self::TranslationAlignmentV1,
        Self::SourceTextUnitV1,
        Self::ZarathustraOpeningSentence,
        Self::AntonovskyCollation,
        Self::ZarathustraAuthoredCanonBridge,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::SourceAnchorV2 => "source_anchor_v2_lab",
            Self::SourceTextLayer => "source_text_layer_lab",
            Self::ProvenanceV2 => "provenance_v2_lab",
            Self::SemanticAnnotationV2 => "semantic_annotation_v2_lab",
            Self::TranslationAlignmentV1 => "translation_alignment_v1_lab",
            Self::SourceTextUnitV1 => "source_text_unit_v1_lab",
            Self::ZarathustraOpeningSentence => "zarathustra_opening_sentence_alignment",
            Self::AntonovskyCollation => "antonovsky_2007_1911_collation",
            Self::ZarathustraAuthoredCanonBridge => "zarathustra_authored_canon_evidence_bridge",
        }
    }
}

/// A scheduled exact schema diagnostic. In a lab result, `before_issue` is its
/// local direct-issue ordinal so the caller can interleave diagnostic-v2
/// findings without a speculative boolean schema verdict or a second parse.
/// The aggregate report carries separate copies with aggregate ordinals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationSchemaCheck {
    pub before_issue: usize,
    pub location: String,
    pub contract: String,
    /// Already decoded once by the source adapter. The executor normalizes
    /// this owner-local DTO into its bounded diagnostic unit.
    pub instance: Value,
    /// Optional local report-control identity for a schema-only negative.
    pub negative_control: Option<String>,
    /// Expected schema validity for reports that assert the result.
    pub expected_valid: Option<bool>,
    /// Expected combined rejection result for a maintained negative control.
    pub expected_rejected: Option<bool>,
    /// Direct semantic rejection result for controls whose owner contract
    /// rejects on schema OR semantic findings.
    pub semantic_rejected: Option<bool>,
    /// JSON pointer in the route report that receives the resolved verdict.
    pub report_slot: Option<String>,
    /// JSON pointer for maintained rejection prose, merged with exact schema
    /// diagnostics before the caller sorts and deduplicates that field.
    pub rejection_reasons_slot: Option<String>,
    /// Fixed maintained document-label prefix for bridge schema issues.
    pub schema_message_prefix: Option<String>,
    /// Exact owner message emitted only if the frozen result expectation fails.
    pub mismatch_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationLabResult {
    pub lab: SourceFoundationLab,
    /// The route-native Python report shape, where available.
    pub report: Value,
    /// Direct owner diagnostics in source order.
    pub ordered_issues: Vec<(String, String)>,
    /// Explicitly names any source-owner behavior not yet represented by the
    /// Rust route. An empty list is required before this can be called whole-lab.
    pub unimplemented: Vec<String>,
    pub schema_checks: Vec<SourceFoundationSchemaCheck>,
    /// Bytes explicitly read by this adapter; reused family kernels can make
    /// additional reads through the same cut owner.
    pub direct_read_bytes: u64,
    /// Retained-state estimate charged by this adapter.
    pub retained_state_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationLabsCost {
    /// The caller's per-member and aggregate source-read ceilings.
    pub max_member_bytes: usize,
    pub max_total_bytes: u64,
    /// The caller's retained-state and diagnostic ceilings.
    pub max_state_bytes: usize,
    pub max_issues: usize,
    pub direct_read_bytes: u64,
    pub retained_state_bytes: usize,
    /// This adapter cannot observe the concrete read counter of every source
    /// implementation; the cut owner remains authoritative for observed I/O.
    pub observed_source_read_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationLabsReport {
    pub results: Vec<SourceFoundationLabResult>,
    /// Ordered concatenation, not a set or sorted diagnostic view.
    pub ordered_issues: Vec<(String, String)>,
    /// Schema requests in traversal order, with ordinals adjusted to the
    /// aggregate issue list. Each per-lab result retains local ordinals.
    pub schema_checks: Vec<SourceFoundationSchemaCheck>,
    pub cost: SourceFoundationLabsCost,
    pub unimplemented: Vec<String>,
}

/// Borrow the exact bridge-private output refs that should be included in the
/// host's physical pre-observation. The caller retains its own budget charge
/// when turning these borrowed refs into the `String` path list used by the
/// physical snapshot builder.
pub fn foundation_lab_private_output_refs<'a>(
    collation_plan: Option<&'a Value>,
    authored_plan: Option<&'a Value>,
) -> impl Iterator<Item = &'a str> + 'a {
    [
        (
            collation_plan,
            "private_text_ref",
            "local-content/witness-text-collation/",
        ),
        (
            collation_plan,
            "private_detail_ref",
            "local-content/witness-text-collation/",
        ),
        (
            authored_plan,
            "private_raw_content_ref",
            "local-content/authored-canon-evidence-bridge/",
        ),
        (
            authored_plan,
            "private_normalized_content_ref",
            "local-content/authored-canon-evidence-bridge/",
        ),
        (
            authored_plan,
            "private_operations_ref",
            "local-content/authored-canon-evidence-bridge/",
        ),
        (
            authored_plan,
            "private_comparison_ref",
            "local-content/authored-canon-evidence-bridge/",
        ),
    ]
    .into_iter()
    .filter_map(|(plan, field, prefix)| {
        let path = plan?.get("outputs")?.get(field)?.as_str()?;
        (path.contains(prefix) && RelativePath::parse(path).is_ok()).then_some(path)
    })
}

fn selected_private_git_ignored(
    physical: Option<&SourcePhysicalFacts>,
    path: &str,
) -> Option<bool> {
    let physical = physical?;
    // `_git_ignored` returns None whenever the selected checkout cannot
    // provide a Git check-ignore result. The owner predicate accepts only
    // literal True, so known Git unavailability is a known mismatch.
    if physical.git_available == Some(false) {
        return Some(false);
    }
    let path_was_observed =
        physical.authored_git.contains_key(path) || physical.private_paths.contains_key(path);
    let ignored = physical
        .authored_git
        .get(path)
        .and_then(|facts| facts.ignored)
        .or_else(|| {
            physical
                .private_paths
                .get(path)
                .and_then(|facts| facts.git_ignored)
        });
    match ignored {
        Some(ignored) => Some(ignored),
        // The Python predicate treats a known Git-unavailable or failed
        // check-ignore result as `None`, which fails its `is not True` test.
        // A path entry plus known Git availability is therefore a known miss.
        None if path_was_observed && physical.git_available.is_some() => Some(false),
        None => None,
    }
}

fn check_private_git_ignore(
    state: &mut DirectState,
    physical: Option<&SourcePhysicalFacts>,
    path: &str,
    field: &str,
    location: &str,
    issue_prefix: &str,
    gap_prefix: &str,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    match selected_private_git_ignored(physical, path) {
        Some(true) => Ok(()),
        Some(false) => state.issue(location, format!("{issue_prefix}: {field}"), limits),
        None => state.gap(format!("{gap_prefix}: {field}"), limits),
    }
}

/// Run exactly one lab-only route. This is the selection entry point for a
/// caller with multiple flags; selection precedence is applied by the caller
/// using `SourceFoundationLab::LAB_ONLY_ORDER` before calling this function.
pub fn inspect_source_foundation_lab(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    lab: SourceFoundationLab,
    current_paths: &[String],
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let current_paths = SliceDefaultPaths(current_paths);
    inspect_source_foundation_lab_inner(source, limits, lab, &current_paths, None)
}

/// Run one lab against caller-owned current-path membership without requiring a
/// materialized copy of the complete member list.
pub fn inspect_source_foundation_lab_from_paths(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    lab: SourceFoundationLab,
    current_paths: &dyn SourceFoundationDefaultPaths,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    inspect_source_foundation_lab_inner(source, limits, lab, current_paths, None)
}

/// Run one lab route with physical path and Git observations from the selected
/// host snapshot. These observations close only bridge-local private-output
/// ignore predicates; they do not make source membership or publication claims.
pub fn inspect_source_foundation_lab_with_physical(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    lab: SourceFoundationLab,
    current_paths: &[String],
    physical: &SourcePhysicalFacts,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let current_paths = SliceDefaultPaths(current_paths);
    inspect_source_foundation_lab_inner(source, limits, lab, &current_paths, Some(physical))
}

/// Run one lab with exact physical observations and caller-owned current-path
/// membership.
pub fn inspect_source_foundation_lab_with_physical_from_paths(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    lab: SourceFoundationLab,
    current_paths: &dyn SourceFoundationDefaultPaths,
    physical: &SourcePhysicalFacts,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    inspect_source_foundation_lab_inner(source, limits, lab, current_paths, Some(physical))
}

fn inspect_source_foundation_lab_inner(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    lab: SourceFoundationLab,
    current_paths: &dyn SourceFoundationDefaultPaths,
    physical: Option<&SourcePhysicalFacts>,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    source.checkpoint(limits.deadline)?;
    inspect_one(source, limits, lab, current_paths, physical)
}

/// Run all six lab-only routes and then the three default authored bridges in
/// the exact maintained Python concatenation order.
pub fn inspect_source_foundation_labs(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    current_paths: &[String],
) -> Result<SourceFoundationLabsReport, ItemRefusal> {
    let current_paths = SliceDefaultPaths(current_paths);
    inspect_source_foundation_labs_inner(source, limits, &current_paths, None)
}

/// Run all lab and authored routes against caller-owned current-path
/// membership without materializing the complete member list.
pub fn inspect_source_foundation_labs_from_paths(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    current_paths: &dyn SourceFoundationDefaultPaths,
) -> Result<SourceFoundationLabsReport, ItemRefusal> {
    inspect_source_foundation_labs_inner(source, limits, current_paths, None)
}

/// Run all six labs and three authored bridges against one selected physical
/// snapshot. Bridge-private Git predicates use only the supplied exact facts.
pub fn inspect_source_foundation_labs_with_physical(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    current_paths: &[String],
    physical: &SourcePhysicalFacts,
) -> Result<SourceFoundationLabsReport, ItemRefusal> {
    let current_paths = SliceDefaultPaths(current_paths);
    inspect_source_foundation_labs_inner(source, limits, &current_paths, Some(physical))
}

/// Run the maintained lab order using the supplied physical snapshot and a
/// caller-owned current-path lookup.
pub fn inspect_source_foundation_labs_with_physical_from_paths(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    current_paths: &dyn SourceFoundationDefaultPaths,
    physical: &SourcePhysicalFacts,
) -> Result<SourceFoundationLabsReport, ItemRefusal> {
    inspect_source_foundation_labs_inner(source, limits, current_paths, Some(physical))
}

fn inspect_source_foundation_labs_inner(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    current_paths: &dyn SourceFoundationDefaultPaths,
    physical: Option<&SourcePhysicalFacts>,
) -> Result<SourceFoundationLabsReport, ItemRefusal> {
    let fixed_output_bytes = SourceFoundationLab::DEFAULT_ORDER
        .len()
        .checked_mul(std::mem::size_of::<SourceFoundationLabResult>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<SourceFoundationLabsReport>()))
        .ok_or(ItemRefusal::Budget)?;
    if fixed_output_bytes > limits.max_state_bytes {
        return Err(ItemRefusal::Budget);
    }
    let mut results = Vec::with_capacity(SourceFoundationLab::DEFAULT_ORDER.len());
    let mut issues = Vec::new();
    let mut unimplemented = Vec::new();
    let mut schema_checks = Vec::new();
    let mut direct_read_bytes = 0u64;
    let mut retained_state_bytes = fixed_output_bytes;
    for lab in SourceFoundationLab::DEFAULT_ORDER {
        source.checkpoint(limits.deadline)?;
        let issue_base = issues.len();
        let result = inspect_one(source, limits, lab, current_paths, physical)?;
        let issue_end = issues
            .len()
            .checked_add(result.ordered_issues.len())
            .filter(|count| *count <= limits.max_issues)
            .ok_or(ItemRefusal::Budget)?;
        for check in &result.schema_checks {
            check
                .before_issue
                .checked_add(issue_base)
                .ok_or(ItemRefusal::Budget)?;
        }
        direct_read_bytes = direct_read_bytes
            .checked_add(result.direct_read_bytes)
            .filter(|bytes| *bytes <= limits.max_total_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let issue_copy_bytes =
            result
                .ordered_issues
                .iter()
                .try_fold(0usize, |bytes, (location, message)| {
                    bytes
                        .checked_add(location.len())
                        .and_then(|bytes| bytes.checked_add(message.len()))
                        .and_then(|bytes| {
                            bytes.checked_add(std::mem::size_of::<(String, String)>() + 32)
                        })
                        .ok_or(ItemRefusal::Budget)
                })?;
        let schema_copy_bytes = result
            .schema_checks
            .iter()
            .try_fold(0usize, |bytes, check| {
                schema_check_storage(check).and_then(|size| {
                    bytes
                        .checked_add(size)
                        .and_then(|bytes| bytes.checked_add(32))
                        .ok_or(ItemRefusal::Budget)
                })
            })?;
        retained_state_bytes = retained_state_bytes
            .checked_add(result.retained_state_bytes)
            .and_then(|bytes| bytes.checked_add(issue_copy_bytes))
            .and_then(|bytes| bytes.checked_add(schema_copy_bytes))
            .filter(|bytes| *bytes <= limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        issues
            .try_reserve_exact(result.ordered_issues.len())
            .map_err(|_| ItemRefusal::Budget)?;
        schema_checks
            .try_reserve_exact(result.schema_checks.len())
            .map_err(|_| ItemRefusal::Budget)?;
        for check in &result.schema_checks {
            let before_issue = check
                .before_issue
                .checked_add(issue_base)
                .ok_or(ItemRefusal::Budget)?;
            let mut aggregate_check = check.clone();
            aggregate_check.before_issue = before_issue;
            schema_checks.push(aggregate_check);
        }
        issues.extend(result.ordered_issues.iter().cloned());
        debug_assert_eq!(issues.len(), issue_end);
        let prefixed_gap_bytes = result
            .unimplemented
            .iter()
            .try_fold(0usize, |bytes, item| {
                bytes
                    .checked_add(lab.name().len())
                    .and_then(|bytes| bytes.checked_add(item.len()))
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>() + 32))
                    .ok_or(ItemRefusal::Budget)
            })?;
        retained_state_bytes = retained_state_bytes
            .checked_add(prefixed_gap_bytes)
            .filter(|bytes| *bytes <= limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        for item in &result.unimplemented {
            unimplemented.push(format!("{}: {item}", lab.name()));
        }
        results.push(result);
    }
    Ok(SourceFoundationLabsReport {
        results,
        ordered_issues: issues,
        schema_checks,
        cost: SourceFoundationLabsCost {
            max_member_bytes: limits.max_member_bytes,
            max_total_bytes: limits.max_total_bytes,
            max_state_bytes: limits.max_state_bytes,
            max_issues: limits.max_issues,
            direct_read_bytes,
            retained_state_bytes,
            observed_source_read_bytes: None,
        },
        unimplemented,
    })
}

fn inspect_one(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    lab: SourceFoundationLab,
    current_paths: &dyn SourceFoundationDefaultPaths,
    physical: Option<&SourcePhysicalFacts>,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    match lab {
        SourceFoundationLab::SourceAnchorV2 => inspect_source_anchor_v2(source, limits),
        SourceFoundationLab::SourceTextLayer => inspect_source_text_layer(source, limits),
        SourceFoundationLab::ProvenanceV2 => inspect_provenance_v2(source, limits),
        SourceFoundationLab::SemanticAnnotationV2 => inspect_semantic_annotation_v2(source, limits),
        SourceFoundationLab::TranslationAlignmentV1 => {
            inspect_translation_alignment_v1(source, limits)
        }
        SourceFoundationLab::SourceTextUnitV1 => inspect_source_text_unit_v1(source, limits),
        SourceFoundationLab::ZarathustraOpeningSentence => inspect_opening_sentence(source, limits),
        SourceFoundationLab::AntonovskyCollation => {
            inspect_antonovsky_collation(source, limits, physical)
        }
        SourceFoundationLab::ZarathustraAuthoredCanonBridge => {
            inspect_authored_canon_bridge(source, limits, current_paths, physical)
        }
    }
}

fn inspect_source_anchor_v2(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let manifest_path =
        "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/lab.manifest.json";
    let mut state = DirectState {
        report: json!({"variants": [], "negative_controls": {}}),
        ..Default::default()
    };
    let expected_limits = json!({"source_payload_used":false,"human_review_performed":false,"source_text_accepted":false,"translation_created":false,"semantic_claim_created":false,"canon_effect":false});
    let Some(manifest) = manifest(
        &mut state,
        source,
        manifest_path,
        "tos_source_anchor_v2_lab_v1",
        "synthetic_mechanical_fixture_only",
        &expected_limits,
        limits,
    )?
    else {
        return result(
            SourceFoundationLab::SourceAnchorV2,
            state,
            Vec::new(),
            limits,
        );
    };
    if manifest.get("contract_ref").and_then(Value::as_str)
        != Some("ToS/contracts/source-anchor-v2.schema.json")
    {
        state.issue(
            manifest_path,
            "laboratory contract reference drifted",
            limits,
        )?;
    }
    manifest_authority(
        &mut state,
        manifest_path,
        &manifest,
        "tos_source_anchor_v2_lab_v1",
        "synthetic_mechanical_fixture_only",
        &expected_limits,
        limits,
    )?;
    let lab_root = "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc";
    let mut resources_by_ref = std::collections::BTreeMap::<String, AnchorResource>::new();
    let mut resources_by_path = std::collections::BTreeMap::<String, AnchorResource>::new();
    if let Some(resources) = manifest.get("resources").and_then(Value::as_array) {
        for (index, resource) in resources.iter().enumerate() {
            let location = format!("{manifest_path} resources[{}]", index + 1);
            if !resource.is_object() {
                state.issue(&location, "resource is not an object", limits)?;
                continue;
            }
            let Some(relative) = resource.get("path").and_then(Value::as_str) else {
                state.issue(
                    &location,
                    "resource path is not a bounded relative path",
                    limits,
                )?;
                continue;
            };
            if RelativePath::parse(relative).is_err() {
                state.issue(
                    &location,
                    "resource path is not a bounded relative path",
                    limits,
                )?;
                continue;
            }
            let path = format!("{lab_root}/{relative}");
            let Some(raw) = state.read(source, &path, limits)? else {
                state.issue(
                    &location,
                    format!("resource is missing: {relative}"),
                    limits,
                )?;
                continue;
            };
            let actual_digest = Digest256::of_bytes(&raw).to_hex();
            if resource.get("sha256").and_then(Value::as_str) != Some(actual_digest.as_str()) {
                state.issue(
                    &location,
                    format!("resource digest drifted: {relative}"),
                    limits,
                )?;
            }
            let Some(representation_ref) =
                resource.get("representation_ref").and_then(Value::as_str)
            else {
                state.issue(
                    &location,
                    "representation_ref is missing or duplicated",
                    limits,
                )?;
                continue;
            };
            if resources_by_ref.contains_key(representation_ref) {
                state.issue(
                    &location,
                    "representation_ref is missing or duplicated",
                    limits,
                )?;
                continue;
            }
            if resources_by_path.contains_key(relative) {
                state.issue(&location, "resource path is duplicated", limits)?;
                continue;
            }
            let sha256 = resource.get("sha256").and_then(Value::as_str).unwrap_or("");
            let media_type = resource
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("");
            let mut entry_bytes = std::mem::size_of::<(String, AnchorResource)>()
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(128))
                .ok_or(ItemRefusal::Budget)?;
            for text in [
                representation_ref,
                sha256,
                media_type,
                representation_ref,
                representation_ref,
                sha256,
                media_type,
                relative,
            ] {
                entry_bytes = entry_bytes
                    .checked_add(estimate_text_storage(text)?)
                    .ok_or(ItemRefusal::Budget)?;
            }
            state.reserve(entry_bytes, limits)?;
            let entry = AnchorResource {
                representation_ref: representation_ref.to_owned(),
                sha256: sha256.to_owned(),
                media_type: media_type.to_owned(),
                bytes: Arc::new(raw),
            };
            resources_by_ref.insert(representation_ref.to_owned(), entry.clone());
            resources_by_path.insert(relative.to_owned(), entry);
        }
    }
    let mut variant_rows = Vec::new();
    let mut anchors_by_variant = std::collections::BTreeMap::<String, Value>::new();
    let mut anchor_ids = std::collections::BTreeSet::<String>::new();
    if let Some(variants) = manifest.get("variants").and_then(Value::as_array) {
        let ids: Vec<_> = variants
            .iter()
            .filter_map(|row| row.get("variant_id").and_then(Value::as_str))
            .collect();
        if ids != ["A", "B", "C"] {
            state.issue(
                manifest_path,
                "anchor variants must be ordered exactly A, B, C",
                limits,
            )?;
        }
        let mut seen = std::collections::BTreeSet::new();
        for variant in variants {
            if !variant.is_object() {
                continue;
            }
            let (Some(id), Some(anchor_ref)) = (
                variant.get("variant_id").and_then(Value::as_str),
                variant.get("anchor_ref").and_then(Value::as_str),
            ) else {
                state.issue(
                    manifest_path,
                    "variant identity or anchor_ref is invalid",
                    limits,
                )?;
                continue;
            };
            if !seen.insert(id.to_owned()) || RelativePath::parse(anchor_ref).is_err() {
                state.issue(
                    manifest_path,
                    "variant identity is duplicated or leaves the laboratory root",
                    limits,
                )?;
                continue;
            }
            let path = format!("{lab_root}/{anchor_ref}");
            let Some(raw) = state.read(source, &path, limits)? else {
                state.issue(&path, "file is missing", limits)?;
                continue;
            };
            let anchor: Value = match serde_json::from_slice::<Value>(&raw) {
                Ok(value) if value.is_object() => value,
                Ok(_) => {
                    state.issue(&path, "JSON root must be an object", limits)?;
                    continue;
                }
                Err(error) => {
                    state.issue(&path, json_parse_owner_message(&error), limits)?;
                    continue;
                }
            };
            state.reserve_decoded(&anchor, limits)?;
            state.schema_check(
                &path,
                "ToS/contracts/source-anchor-v2.schema.json",
                &anchor,
                limits,
            )?;
            append_text_metadata(
                &mut state,
                source,
                &path,
                &raw,
                crate::text_rules::ANCHOR_V2_PROFILE,
                limits,
            )?;
            if let Some(anchor_id) = anchor.get("anchor_id").and_then(Value::as_str) {
                if !anchor_ids.insert(anchor_id.to_owned()) {
                    state.issue(
                        &path,
                        format!("anchor identity is duplicated: {anchor_id}"),
                        limits,
                    )?;
                }
            }
            for message in anchor_v2_semantic_issues(&anchor) {
                state.issue(&path, message, limits)?;
            }
            let selector_method = anchor.get("selector_method").unwrap_or(&Value::Null);
            if selector_method
                .get("configuration_ref")
                .and_then(Value::as_str)
                != Some(manifest_path)
            {
                state.issue(
                    &path,
                    "selector method does not cite the laboratory manifest",
                    limits,
                )?;
            }
            let manifest_digest = state
                .member_digests
                .get(manifest_path)
                .cloned()
                .unwrap_or_default();
            if selector_method
                .get("configuration_digest")
                .and_then(Value::as_str)
                != Some(manifest_digest.as_str())
            {
                state.issue(
                    &path,
                    "selector method configuration digest drifted",
                    limits,
                )?;
            }
            let (mode, selection) = match resolve_anchor_expression(
                &anchor,
                &resources_by_ref,
                &resources_by_path,
                limits.max_member_bytes,
            ) {
                Ok(result) => result,
                Err(error) => {
                    state.issue(
                        &path,
                        format!(
                            "synthetic resolution failed: {}",
                            synthetic_resolution_reason(&error)
                        ),
                        limits,
                    )?;
                    continue;
                }
            };
            let selection_digest = Digest256::of_bytes(selection.as_bytes()).to_hex();
            if selection
                != variant
                    .get("expected_selection")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            {
                state.issue(
                    &path,
                    "resolved selection differs from the frozen expected selection",
                    limits,
                )?;
            }
            if selection_digest
                != variant
                    .get("expected_selection_sha256")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            {
                state.issue(
                    &path,
                    "resolved selection digest differs from the frozen expectation",
                    limits,
                )?;
            }
            variant_rows.push(json!({"variant_id":id,"mode":mode,"selection":selection,"selection_sha256":selection_digest,"resolution_status":anchor.get("resolution_status").cloned().unwrap_or(Value::Null),"review_status":anchor.get("review_status").cloned().unwrap_or(Value::Null)}));
            state.reserve(
                estimate_text_storage(id)?
                    .checked_add(std::mem::size_of::<(String, Value)>() + 64)
                    .ok_or(ItemRefusal::Budget)?,
                limits,
            )?;
            anchors_by_variant.insert(id.to_owned(), anchor);
        }
    } else {
        state.issue(manifest_path, "variants are not a list", limits)?;
    }
    state.report["variants"] = Value::Array(variant_rows);
    let controls = run_anchor_negative_controls(
        &anchors_by_variant,
        &resources_by_ref,
        &resources_by_path,
        &manifest,
        manifest_path,
        limits,
        &mut state,
    )?;
    state.report["negative_controls"] = Value::Object(controls);
    if let Some(declared) = manifest.get("negative_controls").and_then(Value::as_array) {
        let declared: std::collections::BTreeSet<_> =
            declared.iter().filter_map(Value::as_str).collect();
        let observed: std::collections::BTreeSet<_> = state.report["negative_controls"]
            .as_object()
            .map(|rows| rows.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if declared != observed {
            state.issue(
                manifest_path,
                "negative-control coverage differs from manifest",
                limits,
            )?;
        }
    }
    result(
        SourceFoundationLab::SourceAnchorV2,
        state,
        Vec::new(),
        limits,
    )
}

#[derive(Debug, Clone)]
struct AnchorResource {
    representation_ref: String,
    sha256: String,
    media_type: String,
    bytes: Arc<Vec<u8>>,
}

#[derive(Debug, Clone)]
enum AnchorValue {
    Bytes(Arc<Vec<u8>>),
    OwnedBytes(Vec<u8>),
    Json(Value),
    Text(String),
}

#[derive(Debug, Clone)]
struct AnchorScope {
    value: AnchorValue,
    representation_sha256: String,
}

fn anchor_state_scope(
    envelope: &Value,
    resources_by_ref: &std::collections::BTreeMap<String, AnchorResource>,
) -> Result<AnchorScope, String> {
    let state = envelope
        .get("state")
        .and_then(Value::as_object)
        .ok_or_else(|| "selector state is absent".to_owned())?;
    let reference = state
        .get("representation_ref")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "selector state representation is absent from the laboratory manifest".to_owned()
        })?;
    let resource = resources_by_ref.get(reference).ok_or_else(|| {
        "selector state representation is absent from the laboratory manifest".to_owned()
    })?;
    if state.get("representation_sha256").and_then(Value::as_str) != Some(resource.sha256.as_str())
    {
        return Err("selector state digest differs from the represented bytes".into());
    }
    if state.get("media_type").and_then(Value::as_str) != Some(resource.media_type.as_str()) {
        return Err("selector state media type differs from the represented bytes".into());
    }
    Ok(AnchorScope {
        value: AnchorValue::Bytes(resource.bytes.clone()),
        representation_sha256: resource.sha256.clone(),
    })
}

fn anchor_value_bytes(value: &AnchorValue, max_output_bytes: usize) -> Result<Vec<u8>, String> {
    match value {
        AnchorValue::Bytes(bytes) if bytes.len() <= max_output_bytes => Ok(bytes.as_ref().clone()),
        AnchorValue::OwnedBytes(bytes) if bytes.len() <= max_output_bytes => Ok(bytes.clone()),
        AnchorValue::Text(text) if text.len() <= max_output_bytes => Ok(text.as_bytes().to_vec()),
        AnchorValue::Json(value) => {
            let size = bounded_json_encoded_len(value, max_output_bytes)
                .map_err(|_| "selector JSON serialization exceeded its bound")?;
            write_bounded_json(value, max_output_bytes, size)
                .map_err(|_| "selector JSON serialization exceeded its bound".to_owned())
        }
        AnchorValue::Bytes(_) | AnchorValue::OwnedBytes(_) | AnchorValue::Text(_) => {
            Err("selector representation exceeded its bound".into())
        }
    }
}

fn anchor_value_json(value: &AnchorValue) -> Result<Value, String> {
    match value {
        AnchorValue::Json(value) => Ok(value.clone()),
        AnchorValue::Bytes(bytes) => {
            serde_json::from_slice(bytes).map_err(|_| "selector state is not valid JSON".into())
        }
        AnchorValue::OwnedBytes(bytes) => {
            serde_json::from_slice(bytes).map_err(|_| "selector state is not valid JSON".into())
        }
        AnchorValue::Text(text) => {
            serde_json::from_str(text).map_err(|_| "selector state is not valid JSON".into())
        }
    }
}

fn anchor_value_text(value: &AnchorValue, normalization: &str) -> Result<String, String> {
    let text = match value {
        AnchorValue::Text(text) => text.clone(),
        AnchorValue::Bytes(bytes) => std::str::from_utf8(bytes)
            .map_err(|_| "text-layer representation is not UTF-8 text")?
            .to_owned(),
        AnchorValue::OwnedBytes(bytes) => std::str::from_utf8(bytes)
            .map_err(|_| "text-layer representation is not UTF-8 text")?
            .to_owned(),
        AnchorValue::Json(value) => python_compact_sorted_json(value)?,
    };
    match normalization {
        "none" => Ok(text),
        "NFC" => Ok(text.nfc().collect()),
        "NFD" => Ok(text.nfd().collect()),
        "NFKC" => Ok(text.nfkc().collect()),
        "NFKD" => Ok(text.nfkd().collect()),
        _ => Err(format!(
            "unsupported character normalization: {normalization}"
        )),
    }
}

fn apply_anchor_selector(
    envelope: &Value,
    scope: AnchorScope,
    resources_by_path: &std::collections::BTreeMap<String, AnchorResource>,
    max_output_bytes: usize,
) -> Result<AnchorScope, String> {
    let state = envelope
        .get("state")
        .and_then(Value::as_object)
        .ok_or_else(|| "selector state is absent".to_owned())?;
    let selector = envelope
        .get("selector")
        .and_then(Value::as_object)
        .ok_or_else(|| "selector is absent".to_owned())?;
    let selector_type = selector
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "selector type is absent".to_owned())?;
    match selector_type {
        "container_member" => {
            let container = anchor_value_json(&scope.value)?;
            let member_path = selector
                .get("member_path")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    "container member is not declared by the selected container".to_owned()
                })?;
            let declared = container
                .get("members")
                .and_then(Value::as_array)
                .is_some_and(|members| {
                    members
                        .iter()
                        .any(|member| member.as_str() == Some(member_path))
                });
            if !declared {
                return Err("container member is not declared by the selected container".into());
            }
            let member = resources_by_path.get(member_path).ok_or_else(|| {
                "container member is absent from the laboratory manifest".to_owned()
            })?;
            if selector.get("member_sha256").and_then(Value::as_str) != Some(member.sha256.as_str())
            {
                return Err("container member digest differs from selector".into());
            }
            if selector.get("member_media_type").and_then(Value::as_str)
                != Some(member.media_type.as_str())
            {
                return Err("container member media type differs from selector".into());
            }
            Ok(AnchorScope {
                value: AnchorValue::Bytes(member.bytes.clone()),
                representation_sha256: member.sha256.clone(),
            })
        }
        "structural" => {
            let scheme = selector
                .get("scheme")
                .and_then(Value::as_str)
                .ok_or_else(|| "unsupported synthetic structural scheme: missing".to_owned())?;
            let selected = match scheme {
                "json_pointer" => {
                    let document = anchor_value_json(&scope.value)?;
                    let pointer = selector.get("value").and_then(Value::as_str).unwrap_or("");
                    anchor_json_value(anchor_json_pointer(&document, pointer)?)
                }
                "xml_id" => {
                    let raw = anchor_value_bytes(&scope.value, max_output_bytes)?;
                    let text = std::str::from_utf8(&raw)
                        .map_err(|_| "text-layer representation is not UTF-8 text")?;
                    let id = selector.get("value").and_then(Value::as_str).unwrap_or("");
                    AnchorValue::Text(anchor_xml_id_text(text, id)?)
                }
                _ => return Err(format!("unsupported synthetic structural scheme: {scheme}")),
            };
            Ok(AnchorScope {
                value: selected,
                ..scope
            })
        }
        "text_quote" => {
            let normalization = state
                .get("character_normalization")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let text = anchor_value_text(&scope.value, normalization)?;
            let exact = selector
                .get("exact")
                .and_then(Value::as_str)
                .ok_or_else(|| "text quote exact text is absent".to_owned())?;
            let text_chars: Vec<char> = text.chars().collect();
            let exact_chars: Vec<char> = exact.chars().collect();
            let prefix: Option<Vec<char>> = selector
                .get("prefix")
                .and_then(Value::as_str)
                .map(|value| value.chars().collect());
            let suffix: Option<Vec<char>> = selector
                .get("suffix")
                .and_then(Value::as_str)
                .map(|value| value.chars().collect());
            let mut starts = Vec::new();
            if !exact_chars.is_empty() && exact_chars.len() <= text_chars.len() {
                for start in 0..=text_chars.len() - exact_chars.len() {
                    let end = start + exact_chars.len();
                    if text_chars[start..end] != exact_chars {
                        continue;
                    }
                    let prefix_ok = prefix.as_ref().is_none_or(|prefix| {
                        let prefix_start = start.saturating_sub(prefix.len());
                        text_chars[prefix_start..start] == prefix[..]
                    });
                    let suffix_ok = suffix.as_ref().is_none_or(|suffix| {
                        text_chars.get(end..end.saturating_add(suffix.len())) == Some(&suffix[..])
                    });
                    if prefix_ok && suffix_ok {
                        starts.push(start);
                    }
                }
            }
            if starts.len() != 1 {
                return Err("text quote does not resolve exactly once with its context".into());
            }
            let start = starts[0];
            let end = start + exact_chars.len();
            if unicode_normalization::char::is_combining_mark(text_chars[start]) {
                return Err("text quote start splits a combining sequence".into());
            }
            if end < text_chars.len()
                && unicode_normalization::char::is_combining_mark(text_chars[end])
            {
                return Err("text quote end splits a combining sequence".into());
            }
            Ok(AnchorScope {
                value: AnchorValue::Text(exact.to_owned()),
                ..scope
            })
        }
        "text_position" => {
            let normalization = state
                .get("character_normalization")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let text = anchor_value_text(&scope.value, normalization)?;
            let chars: Vec<char> = text.chars().collect();
            let start = selector
                .get("start")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    "text-position interval is outside the selected representation".to_owned()
                })?;
            let end = selector.get("end").and_then(Value::as_i64).ok_or_else(|| {
                "text-position interval is outside the selected representation".to_owned()
            })?;
            if start < 0 || start >= end || end as usize > chars.len() {
                return Err("text-position interval is outside the selected representation".into());
            }
            if unicode_normalization::char::is_combining_mark(chars[start as usize]) {
                return Err("text-position start splits a combining sequence".into());
            }
            if (end as usize) < chars.len()
                && unicode_normalization::char::is_combining_mark(chars[end as usize])
            {
                return Err("text-position end splits a combining sequence".into());
            }
            let selected = chars[start as usize..end as usize].iter().collect();
            Ok(AnchorScope {
                value: AnchorValue::Text(selected),
                ..scope
            })
        }
        "byte_position" => {
            let bytes = anchor_value_bytes(&scope.value, max_output_bytes)?;
            let start = selector
                .get("start")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    "byte-position interval is outside the selected representation".to_owned()
                })?;
            let end = selector.get("end").and_then(Value::as_i64).ok_or_else(|| {
                "byte-position interval is outside the selected representation".to_owned()
            })?;
            if start < 0 || start >= end || end as usize > bytes.len() {
                return Err("byte-position interval is outside the selected representation".into());
            }
            Ok(AnchorScope {
                value: AnchorValue::OwnedBytes(bytes[start as usize..end as usize].to_vec()),
                ..scope
            })
        }
        "page_region" => {
            let mut region = serde_json::Map::new();
            for key in [
                "page_identity",
                "x",
                "y",
                "width",
                "height",
                "coordinate_space",
            ] {
                if let Some(value) = selector.get(key) {
                    region.insert(key.to_owned(), value.clone());
                }
            }
            Ok(AnchorScope {
                value: AnchorValue::Json(Value::Object(region)),
                ..scope
            })
        }
        "fragment" => {
            Err("synthetic laboratory does not implement arbitrary fragment standards".into())
        }
        _ => Err(format!("unsupported selector type: {selector_type}")),
    }
}

fn resolve_anchor_expression(
    anchor: &Value,
    resources_by_ref: &std::collections::BTreeMap<String, AnchorResource>,
    resources_by_path: &std::collections::BTreeMap<String, AnchorResource>,
    max_output_bytes: usize,
) -> Result<(String, String), String> {
    let expression = anchor
        .pointer("/selector_payload/expression")
        .and_then(Value::as_object)
        .ok_or_else(|| "selector expression is absent".to_owned())?;
    let mode = expression
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| "unsupported selector expression mode: missing".to_owned())?;
    let selected = match mode {
        "single" => {
            let envelope = expression
                .get("selector")
                .ok_or_else(|| "selector expression is absent".to_owned())?;
            let scope = anchor_state_scope(envelope, resources_by_ref)?;
            apply_anchor_selector(envelope, scope, resources_by_path, max_output_bytes)?.value
        }
        "alternatives" => {
            let alternatives = expression
                .get("alternatives")
                .and_then(Value::as_array)
                .ok_or_else(|| "alternative selector set is absent".to_owned())?;
            let mut values = Vec::with_capacity(alternatives.len());
            for envelope in alternatives {
                let scope = anchor_state_scope(envelope, resources_by_ref)?;
                values.push(
                    apply_anchor_selector(envelope, scope, resources_by_path, max_output_bytes)?
                        .value,
                );
            }
            let canonical: Vec<_> = values
                .iter()
                .map(anchor_value_comparison_text)
                .collect::<Result<_, _>>()?;
            if canonical
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != 1
            {
                return Err("alternative selectors resolve to different segments".into());
            }
            values
                .into_iter()
                .next()
                .ok_or_else(|| "alternative selector set is empty".to_owned())?
        }
        "refinement_chain" => {
            let steps = expression
                .get("steps")
                .and_then(Value::as_array)
                .ok_or_else(|| "refinement chain is absent".to_owned())?;
            let mut current: Option<AnchorScope> = None;
            for (index, envelope) in steps.iter().enumerate() {
                let base = anchor_state_scope(envelope, resources_by_ref)?;
                let scope = if let Some(previous) = current.take() {
                    let next_digest = envelope
                        .pointer("/state/representation_sha256")
                        .and_then(Value::as_str);
                    if Some(previous.representation_sha256.as_str()) != next_digest {
                        return Err(
                            "refinement step state differs from the prior selected representation"
                                .into(),
                        );
                    }
                    previous
                } else {
                    base
                };
                let selected =
                    apply_anchor_selector(envelope, scope, resources_by_path, max_output_bytes)?;
                if index == 0
                    && envelope.pointer("/selector/type").and_then(Value::as_str)
                        == Some("container_member")
                {
                    let next_digest = steps
                        .get(1)
                        .and_then(|step| step.pointer("/state/representation_sha256"))
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            "container refinement does not bind the selected member state"
                                .to_owned()
                        })?;
                    if selected.representation_sha256 != next_digest {
                        return Err(
                            "container refinement does not bind the selected member state".into(),
                        );
                    }
                }
                current = Some(selected);
            }
            current
                .ok_or_else(|| "refinement chain is empty".to_owned())?
                .value
        }
        _ => return Err(format!("unsupported selector expression mode: {mode}")),
    };
    Ok((mode.to_owned(), anchor_value_result_text(&selected)?))
}

fn anchor_value_comparison_text(value: &AnchorValue) -> Result<String, String> {
    match value {
        AnchorValue::Text(text) => Ok(text.clone()),
        AnchorValue::Json(value) => python_sorted_json(value),
        AnchorValue::Bytes(_) | AnchorValue::OwnedBytes(_) => {
            Err("Object of type bytes is not JSON serializable".into())
        }
    }
}

fn anchor_value_result_text(value: &AnchorValue) -> Result<String, String> {
    anchor_value_comparison_text(value)
}

fn anchor_json_pointer(document: &Value, pointer: &str) -> Result<Value, String> {
    if !pointer.starts_with('/') {
        return Err("JSON Pointer must begin with /".into());
    }
    let mut current = document;
    for raw in pointer[1..].split('/') {
        let part = raw.replace("~1", "/").replace("~0", "~");
        current = match current {
            Value::Object(map) => map
                .get(&part)
                .ok_or_else(|| "JSON Pointer key is absent".to_owned())?,
            Value::Array(rows) => {
                let index = part
                    .parse::<usize>()
                    .map_err(|_| "JSON Pointer array index is invalid".to_owned())?;
                rows.get(index)
                    .ok_or_else(|| "JSON Pointer array index is absent".to_owned())?
            }
            _ => return Err("JSON Pointer traverses a scalar".into()),
        };
    }
    Ok(current.clone())
}

fn anchor_json_value(value: Value) -> AnchorValue {
    match value {
        Value::String(text) => AnchorValue::Text(text),
        value => AnchorValue::Json(value),
    }
}

fn anchor_xml_id_text(xml: &str, id: &str) -> Result<String, String> {
    let mut match_span = None;
    let mut selected_tag = String::new();
    for (start, _) in xml.match_indices('<') {
        let Some(relative_end) = xml[start..].find('>') else {
            continue;
        };
        let end = start + relative_end;
        let tag = &xml[start + 1..end];
        if tag.starts_with('/') || tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        let tag_name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/');
        let has_id = ["id", "xml:id"]
            .iter()
            .any(|name| xml_attribute(tag, name).is_some_and(|value| value == id));
        if has_id {
            if match_span.is_some() {
                return Err("xml_id selector does not resolve exactly one element".into());
            }
            match_span = Some(end + 1);
            selected_tag = tag_name.to_owned();
        }
    }
    let content_start = match_span
        .ok_or_else(|| "xml_id selector does not resolve exactly one element".to_owned())?;
    let close = format!("</{selected_tag}");
    let content_end = xml[content_start..]
        .find(&close)
        .map(|offset| content_start + offset)
        .ok_or_else(|| "xml_id selector does not resolve exactly one element".to_owned())?;
    let mut text = String::new();
    let mut in_tag = false;
    for ch in xml[content_start..content_end].chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    Ok(text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&"))
}

fn xml_attribute(tag: &str, name: &str) -> Option<String> {
    let mut cursor = 0;
    while cursor < tag.len() {
        while cursor < tag.len() && tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let key_start = cursor;
        while cursor < tag.len()
            && !tag.as_bytes()[cursor].is_ascii_whitespace()
            && tag.as_bytes()[cursor] != b'='
            && tag.as_bytes()[cursor] != b'/'
        {
            cursor += 1;
        }
        if key_start == cursor {
            cursor += 1;
            continue;
        }
        let key = &tag[key_start..cursor];
        while cursor < tag.len() && tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag.len() || tag.as_bytes()[cursor] != b'=' {
            continue;
        }
        cursor += 1;
        while cursor < tag.len() && tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag.len() || !matches!(tag.as_bytes()[cursor], b'\'' | b'"') {
            return None;
        }
        let quote = tag.as_bytes()[cursor];
        cursor += 1;
        let value_start = cursor;
        while cursor < tag.len() && tag.as_bytes()[cursor] != quote {
            cursor += 1;
        }
        if cursor >= tag.len() {
            return None;
        }
        if key == name {
            return Some(tag[value_start..cursor].to_owned());
        }
        cursor += 1;
    }
    None
}

fn anchor_v2_expression_envelopes(expression: &Value) -> Vec<&Value> {
    match expression.get("mode").and_then(Value::as_str) {
        Some("single") => expression.get("selector").into_iter().collect(),
        Some("alternatives") => expression
            .get("alternatives")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().collect())
            .unwrap_or_default(),
        Some("refinement_chain") => expression
            .get("steps")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn anchor_v2_semantic_issues(anchor: &Value) -> Vec<String> {
    let mut messages = Vec::new();
    if !anchor
        .get("supersedes_anchor_ref")
        .is_none_or(Value::is_null)
        && anchor.get("supersedes_anchor_ref") == anchor.get("anchor_id")
    {
        messages.push("source anchor cannot supersede itself".into());
    }
    let Some(selector_payload) = anchor.get("selector_payload").and_then(Value::as_object) else {
        return messages;
    };
    let publication = anchor
        .get("publication_boundary")
        .and_then(Value::as_object);
    if selector_payload.get("kind").and_then(Value::as_str) == Some("withheld_selector_receipt") {
        if publication
            .and_then(|row| row.get("source_text_in_record"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            messages.push("withheld selector receipt cannot claim source text in record".into());
        }
        return messages;
    }
    let Some(expression) = selector_payload
        .get("expression")
        .and_then(Value::as_object)
    else {
        return messages;
    };
    let expression_value = Value::Object(expression.clone());
    let envelopes = anchor_v2_expression_envelopes(&expression_value);
    let selectors: Vec<_> = envelopes
        .iter()
        .filter_map(|envelope| envelope.get("selector").and_then(Value::as_object))
        .collect();
    let selector_types: std::collections::BTreeSet<_> = selectors
        .iter()
        .filter_map(|selector| selector.get("type").and_then(Value::as_str))
        .collect();
    let target_digest = anchor.pointer("/target/file_sha256");
    let mode = expression.get("mode").and_then(Value::as_str);
    if matches!(mode, Some("single" | "refinement_chain")) && !envelopes.is_empty() {
        if envelopes[0].pointer("/state/representation_sha256") != target_digest {
            messages.push("first selector state is not the exact target file state".into());
        }
    }
    if mode == Some("alternatives") {
        let mut first_digests = std::collections::BTreeSet::<String>::new();
        for envelope in &envelopes {
            if let Some(value) = envelope.pointer("/state/representation_sha256") {
                first_digests.insert(value.to_string());
            }
        }
        let expected = target_digest
            .map(Value::to_string)
            .unwrap_or_else(|| "null".into());
        if first_digests != [expected].into_iter().collect() {
            messages
                .push("alternatives do not independently begin from the exact target state".into());
        }
    }
    for envelope in &envelopes {
        let Some(selector) = envelope.get("selector").and_then(Value::as_object) else {
            continue;
        };
        let Some(state) = envelope.get("state").and_then(Value::as_object) else {
            continue;
        };
        let selector_type = selector.get("type").and_then(Value::as_str).unwrap_or("");
        if matches!(selector_type, "text_quote" | "text_position")
            && !state.contains_key("character_normalization")
        {
            messages.push(format!(
                "{selector_type} lacks explicit character normalization"
            ));
        }
        if matches!(selector_type, "text_position" | "byte_position") {
            if let (Some(start), Some(end)) = (
                selector.get("start").and_then(Value::as_i64),
                selector.get("end").and_then(Value::as_i64),
            ) {
                if start >= end {
                    messages.push(format!("{selector_type} interval is empty or reversed"));
                }
            }
        }
        if selector_type == "page_region" {
            let space = selector.get("coordinate_space").and_then(Value::as_str);
            if space == Some("normalized_0_1") {
                if let (Some(x), Some(y), Some(width), Some(height)) = (
                    selector.get("x").and_then(Value::as_f64),
                    selector.get("y").and_then(Value::as_f64),
                    selector.get("width").and_then(Value::as_f64),
                    selector.get("height").and_then(Value::as_f64),
                ) {
                    if x + width > 1.0 || y + height > 1.0 {
                        messages.push("normalized page region exceeds the unit square".into());
                    }
                }
            } else if matches!(space, Some("pixels" | "points")) {
                if let (
                    Some(x),
                    Some(y),
                    Some(width),
                    Some(height),
                    Some(source_width),
                    Some(source_height),
                ) = (
                    selector.get("x").and_then(Value::as_f64),
                    selector.get("y").and_then(Value::as_f64),
                    selector.get("width").and_then(Value::as_f64),
                    selector.get("height").and_then(Value::as_f64),
                    selector.get("source_width").and_then(Value::as_f64),
                    selector.get("source_height").and_then(Value::as_f64),
                ) {
                    if x + width > source_width || y + height > source_height {
                        messages
                            .push("page region exceeds the declared representation extent".into());
                    }
                }
            }
        }
    }
    let tracked_nonpublic = publication
        .and_then(|row| row.get("record_storage"))
        .and_then(Value::as_str)
        == Some("tracked")
        && publication
            .and_then(|row| row.get("source_content_visibility"))
            .and_then(Value::as_str)
            != Some("public");
    if tracked_nonpublic && selector_types.contains("text_quote") {
        messages.push("tracked nonpublic anchor cannot carry a text quote".into());
    }
    if publication
        .and_then(|row| row.get("source_text_in_record"))
        .and_then(Value::as_bool)
        == Some(false)
        && selector_types.contains("text_quote")
    {
        messages.push("text quote contradicts source_text_in_record=false".into());
    }
    if publication
        .and_then(|row| row.get("source_text_in_record"))
        .and_then(Value::as_bool)
        == Some(true)
        && !selector_types.contains("text_quote")
    {
        messages.push("source_text_in_record=true has no text-bearing selector".into());
    }
    if anchor.get("resolution_status").and_then(Value::as_str) == Some("mechanically_resolved")
        && envelopes.is_empty()
    {
        messages.push("mechanically resolved anchor has no selector expression".into());
    }
    messages
}

fn python_sorted_json(value: &Value) -> Result<String, String> {
    fn write(value: &Value, output: &mut String, depth: usize) -> Result<(), String> {
        if depth > 128 {
            return Err("JSON value exceeds canonicalization depth".into());
        }
        match value {
            Value::Null => output.push_str("null"),
            Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            Value::Number(value) => output.push_str(&value.to_string()),
            Value::String(value) => output.push_str(
                &serde_json::to_string(value)
                    .map_err(|_| "selector JSON serialization failed".to_owned())?,
            ),
            Value::Array(rows) => {
                output.push('[');
                for (index, row) in rows.iter().enumerate() {
                    if index > 0 {
                        output.push_str(", ");
                    }
                    write(row, output, depth + 1)?;
                }
                output.push(']');
            }
            Value::Object(map) => {
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort_unstable();
                output.push('{');
                for (index, key) in keys.iter().enumerate() {
                    if index > 0 {
                        output.push_str(", ");
                    }
                    output.push_str(
                        &serde_json::to_string(*key)
                            .map_err(|_| "selector JSON serialization failed".to_owned())?,
                    );
                    output.push_str(": ");
                    write(
                        map.get(*key)
                            .ok_or_else(|| "JSON key disappeared".to_owned())?,
                        output,
                        depth + 1,
                    )?;
                }
                output.push('}');
            }
        }
        Ok(())
    }
    let mut output = String::new();
    write(value, &mut output, 0)?;
    Ok(output)
}

fn python_compact_sorted_json(value: &Value) -> Result<String, String> {
    let bytes = canonical_json_bytes(value, usize::MAX)
        .map_err(|_| "JSON value exceeds canonicalization bounds".to_owned())?;
    String::from_utf8(bytes).map_err(|_| "text-layer representation is not UTF-8 text".into())
}

fn run_anchor_negative_controls(
    anchors: &std::collections::BTreeMap<String, Value>,
    resources_by_ref: &std::collections::BTreeMap<String, AnchorResource>,
    resources_by_path: &std::collections::BTreeMap<String, AnchorResource>,
    manifest: &Value,
    manifest_path: &str,
    limits: ItemLimits,
    state: &mut DirectState,
) -> Result<serde_json::Map<String, Value>, ItemRefusal> {
    let mut result = serde_json::Map::new();
    if anchors
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        != ["A", "B", "C"].into_iter().collect()
    {
        return Ok(result);
    }
    let expected = manifest
        .get("variants")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("variant_id").and_then(Value::as_str) == Some("B"))
        })
        .and_then(|row| row.get("expected_selection"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut controls = Vec::<(&str, bool)>::new();

    let mut utf16 = state.clone_value_charged(&anchors["B"], limits)?;
    set_json_pointer(
        &mut utf16,
        "/selector_payload/expression/selector/selector/start",
        json!(5),
    );
    set_json_pointer(
        &mut utf16,
        "/selector_payload/expression/selector/selector/end",
        json!(10),
    );
    controls.push((
        "utf16_offset_mislabeled_as_unicode_code_point",
        resolve_anchor_expression(
            &utf16,
            resources_by_ref,
            resources_by_path,
            limits.max_member_bytes,
        )
        .is_ok_and(|(_, value)| value != expected),
    ));

    let mut alternatives = state.clone_value_charged(&anchors["A"], limits)?;
    set_json_pointer(
        &mut alternatives,
        "/selector_payload/expression/alternatives/1/selector/exact",
        json!("A tree begins in evidence."),
    );
    controls.push((
        "alternatives_resolve_to_different_passages",
        resolve_anchor_expression(
            &alternatives,
            resources_by_ref,
            resources_by_path,
            limits.max_member_bytes,
        )
        .is_err(),
    ));

    let mut digest_drift = state.clone_value_charged(&anchors["B"], limits)?;
    set_json_pointer(
        &mut digest_drift,
        "/selector_payload/expression/selector/selector/state/representation_sha256",
        json!("0".repeat(64)),
    );
    controls.push((
        "representation_digest_drift",
        resolve_anchor_expression(
            &digest_drift,
            resources_by_ref,
            resources_by_path,
            limits.max_member_bytes,
        )
        .is_err(),
    ));

    let mut reversed = state.clone_value_charged(&anchors["C"], limits)?;
    if let Some(steps) = reversed
        .pointer_mut("/selector_payload/expression/steps")
        .and_then(Value::as_array_mut)
    {
        steps.reverse();
    }
    controls.push((
        "refinement_steps_reversed",
        resolve_anchor_expression(
            &reversed,
            resources_by_ref,
            resources_by_path,
            limits.max_member_bytes,
        )
        .is_err(),
    ));

    let mut nonpublic = state.clone_value_charged(&anchors["A"], limits)?;
    set_json_pointer(
        &mut nonpublic,
        "/publication_boundary/source_content_visibility",
        json!("local_only"),
    );
    controls.push((
        "tracked_nonpublic_text_quote",
        anchor_v2_semantic_issues(&nonpublic)
            .iter()
            .any(|message| message == "tracked nonpublic anchor cannot carry a text quote"),
    ));

    let mut region = state.clone_value_charged(&anchors["B"], limits)?;
    set_json_pointer(
        &mut region,
        "/selector_payload/expression/selector/selector",
        json!({
            "type":"page_region",
            "page_identity":{"page_number":1},
            "x":0.8,
            "y":0.1,
            "width":0.4,
            "height":0.2,
            "coordinate_space":"normalized_0_1"
        }),
    );
    controls.push((
        "normalized_page_region_overflow",
        anchor_v2_semantic_issues(&region)
            .iter()
            .any(|message| message == "normalized page region exceeds the unit square"),
    ));

    for (name, rejected) in controls {
        result.insert(
            name.to_owned(),
            Value::String(if rejected { "rejected" } else { "not_rejected" }.into()),
        );
        if !rejected {
            state.issue(
                manifest_path,
                format!("negative control was not rejected: {name}"),
                limits,
            )?;
        }
    }
    Ok(result)
}

fn set_json_pointer(document: &mut Value, pointer: &str, value: Value) {
    if let Some(target) = document.pointer_mut(pointer) {
        *target = value;
    }
}

fn python_equal(left: &Value, right: &Value) -> Result<bool, ItemRefusal> {
    crate::assessment::py_equal(left, right)
        .map_err(|_| ItemRefusal::Unsupported("bounded Python JSON comparison failed".into()))
}

fn python_optional_equal(left: Option<&Value>, right: Option<&Value>) -> Result<bool, ItemRefusal> {
    match (left, right) {
        (Some(left), Some(right)) => python_equal(left, right),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

fn python_integer(value: &Value) -> Option<i128> {
    value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
        .or_else(|| value.as_bool().map(|value| if value { 1 } else { 0 }))
}

fn inspect_source_text_layer(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let path = "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/lab.manifest.json";
    let expected_limits = json!({"real_source_payload_used":false,"human_review_performed":false,"source_text_accepted":false,"translation_created":false,"linguistic_claim_created":false,"semantic_claim_created":false,"graph_effect_created":false,"canon_effect":false,"routine_human_task_created":false});
    let mut state = DirectState {
        report: json!({"source_anchor_selection":null,"variants":[],"negative_controls":{}}),
        ..Default::default()
    };
    let Some(manifest) = manifest(
        &mut state,
        source,
        path,
        "tos_source_text_layer_lab_v1",
        "public_synthetic_mechanics_only",
        &expected_limits,
        limits,
    )?
    else {
        return result(
            SourceFoundationLab::SourceTextLayer,
            state,
            Vec::new(),
            limits,
        );
    };
    if manifest.get("contract_ref").and_then(Value::as_str)
        != Some("ToS/contracts/source-text-layer.schema.json")
    {
        state.issue(path, "laboratory contract reference drifted", limits)?;
    }
    manifest_authority(
        &mut state,
        path,
        &manifest,
        "tos_source_text_layer_lab_v1",
        "public_synthetic_mechanics_only",
        &expected_limits,
        limits,
    )?;
    let policy = manifest
        .get("editorial_policy")
        .cloned()
        .unwrap_or(Value::Null);
    let policy_ref = policy.get("ref").and_then(Value::as_str);
    let policy_digest = policy.get("sha256").and_then(Value::as_str);
    if let (Some(ref_path), Some(digest)) = (policy_ref, policy_digest) {
        match state.read(source, ref_path, limits)? {
            Some(raw) if Digest256::of_bytes(&raw).to_hex() == digest => {}
            _ => state.issue(path, "editorial policy closure drifted", limits)?,
        }
    } else {
        state.issue(path, "editorial policy closure drifted", limits)?;
    }
    let anchor_result = inspect_source_anchor_v2(source, limits)?;
    let anchor_issue_base = state.issues.len();
    state.read_bytes = state
        .read_bytes
        .checked_add(anchor_result.direct_read_bytes)
        .filter(|bytes| *bytes <= limits.max_total_bytes)
        .ok_or(ItemRefusal::Budget)?;
    state.retained_bytes = state
        .retained_bytes
        .checked_add(anchor_result.retained_state_bytes)
        .filter(|bytes| *bytes <= limits.max_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    for mut check in anchor_result.schema_checks.iter().cloned() {
        check.before_issue = check
            .before_issue
            .checked_add(anchor_issue_base)
            .ok_or(ItemRefusal::Budget)?;
        state.schema_checks.push(check);
    }
    for (location, message) in &anchor_result.ordered_issues {
        state.issue(
            location,
            format!("source-text-layer anchor dependency: {message}"),
            limits,
        )?;
    }
    for gap in &anchor_result.unimplemented {
        state.gap(format!("source-anchor dependency: {gap}"), limits)?;
    }
    let anchor_expectation = manifest.get("source_anchor").unwrap_or(&Value::Null);
    let anchor_row = anchor_result
        .report
        .get("variants")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("variant_id").and_then(Value::as_str) == Some("B"))
        });
    if let Some(anchor_row) = anchor_row {
        state.report["source_anchor_selection"] =
            anchor_row.get("selection").cloned().unwrap_or(Value::Null);
        if !python_optional_equal(
            anchor_row.get("selection"),
            anchor_expectation.get("expected_selection"),
        )? || !python_optional_equal(
            anchor_row.get("selection_sha256"),
            anchor_expectation.get("expected_selection_sha256"),
        )? || !python_optional_equal(
            anchor_row.get("review_status"),
            anchor_expectation.get("review_status"),
        )? {
            state.issue(path, "source anchor expectation drifted", limits)?;
        }
    } else {
        state.issue(path, "source anchor B did not resolve", limits)?;
    }
    let anchor_record_ref = anchor_expectation
        .get("anchor_record_ref")
        .and_then(Value::as_str);
    let anchor_record_digest = anchor_expectation
        .get("anchor_record_sha256")
        .and_then(Value::as_str);
    if let (Some(anchor_ref), Some(expected_digest)) = (anchor_record_ref, anchor_record_digest) {
        match state.read(source, anchor_ref, limits)? {
            Some(raw) if Digest256::of_bytes(&raw).to_hex() == expected_digest => {}
            _ => state.issue(path, "source anchor record closure drifted", limits)?,
        }
    } else {
        state.issue(path, "source anchor record closure drifted", limits)?;
    }
    let Some(variants) = manifest.get("variants").and_then(Value::as_array) else {
        state.issue(path, "variants are not a list", limits)?;
        return result(
            SourceFoundationLab::SourceTextLayer,
            state,
            Vec::new(),
            limits,
        );
    };
    let ids: Vec<_> = variants
        .iter()
        .filter_map(|row| row.get("variant_id").and_then(Value::as_str))
        .collect();
    if ids != ["A", "B", "C"] {
        state.issue(
            path,
            "source-text-layer variants must be ordered exactly A, B, C",
            limits,
        )?;
    }
    let mut report_rows = Vec::new();
    let mut layers_by_id = std::collections::BTreeMap::<String, Value>::new();
    let mut record_ref_by_id = std::collections::BTreeMap::<String, String>::new();
    let mut selected_text_by_id = std::collections::BTreeMap::<String, String>::new();
    let mut layers_by_variant = std::collections::BTreeMap::<String, Value>::new();
    for row in variants {
        source.checkpoint(limits.deadline)?;
        let (Some(id), Some(layer_ref), Some(content_ref)) = (
            row.get("variant_id").and_then(Value::as_str),
            row.get("layer_ref").and_then(Value::as_str),
            row.get("content_ref").and_then(Value::as_str),
        ) else {
            state.issue(path, "variant references are invalid", limits)?;
            continue;
        };
        let Some(raw) = state.read(source, layer_ref, limits)? else {
            state.issue(layer_ref, "layer record fixity drifted", limits)?;
            continue;
        };
        if Digest256::of_bytes(&raw).to_hex()
            != row
                .get("layer_sha256")
                .and_then(Value::as_str)
                .unwrap_or("")
        {
            state.issue(layer_ref, "layer record fixity drifted", limits)?;
        }
        let Some(content) = check_ref(
            &mut state,
            source,
            layer_ref,
            content_ref,
            row.get("content_sha256").and_then(Value::as_str),
            limits,
        )?
        else {
            continue;
        };
        let layer: Value = match serde_json::from_slice::<Value>(&raw) {
            Ok(value) if value.is_object() => value,
            Ok(_) => {
                state.issue(layer_ref, "JSON root must be an object", limits)?;
                continue;
            }
            Err(error) => {
                state.issue(layer_ref, json_parse_owner_message(&error), limits)?;
                continue;
            }
        };
        state.reserve_decoded(&layer, limits)?;
        state.schema_check(
            layer_ref,
            "ToS/contracts/source-text-layer.schema.json",
            &layer,
            limits,
        )?;
        append_text_metadata(
            &mut state,
            source,
            layer_ref,
            &raw,
            crate::text_rules::TEXT_LAYER_PROFILE,
            limits,
        )?;
        for message in source_text_layer_semantic_issues(&layer) {
            state.issue(layer_ref, message, limits)?;
        }
        let selected = match selected_layer_text(&layer, &content) {
            Ok(text) => text,
            Err(message) => {
                state.issue(layer_ref, message, limits)?;
                continue;
            }
        };
        if layer
            .pointer("/representation/content_ref")
            .and_then(Value::as_str)
            != Some(content_ref)
        {
            state.issue(
                layer_ref,
                "layer representation and manifest content paths differ",
                limits,
            )?;
        }
        if Some(selected.as_str()) != row.get("expected_text").and_then(Value::as_str) {
            state.issue(
                layer_ref,
                "selected text differs from the frozen expectation",
                limits,
            )?;
        }
        let selected_digest = Digest256::of_bytes(selected.as_bytes()).to_hex();
        if Some(selected_digest.as_str()) != row.get("expected_text_sha256").and_then(Value::as_str)
        {
            state.issue(
                layer_ref,
                "selected text digest differs from the frozen expectation",
                limits,
            )?;
        }
        if layer
            .pointer("/representation/content_sha256")
            .and_then(Value::as_str)
            != row.get("content_sha256").and_then(Value::as_str)
        {
            state.issue(
                layer_ref,
                "layer and manifest content digests differ",
                limits,
            )?;
        }
        let editorial = layer.get("editorial_policy");
        if !python_optional_equal(
            editorial.and_then(|value| value.get("policy_ref")),
            policy.get("ref"),
        )? || !python_optional_equal(
            editorial.and_then(|value| value.get("policy_sha256")),
            policy.get("sha256"),
        )? {
            state.issue(layer_ref, "layer editorial policy closure drifted", limits)?;
        }
        if layer
            .pointer("/source_binding/source_file_sha256")
            .and_then(Value::as_str)
            != Some("0e80d33931ca968af97942d1a91c82d4ab4c2ed9c417b21a05186ef85493dfce")
        {
            state.issue(
                layer_ref,
                "layer source file digest left the exact anchor target",
                limits,
            )?;
        }
        let expected_anchors = json!([{
            "anchor_id": anchor_expectation.get("anchor_id"),
            "anchor_record_ref": anchor_record_ref,
            "anchor_record_sha256": anchor_record_digest,
        }]);
        if !python_equal(
            layer
                .pointer("/source_binding/anchors")
                .unwrap_or(&Value::Null),
            &expected_anchors,
        )? {
            state.issue(layer_ref, "layer source-anchor binding drifted", limits)?;
        }

        let Some(layer_id) = layer.get("layer_id").and_then(Value::as_str) else {
            state.issue(layer_ref, "layer identity is invalid or duplicated", limits)?;
            continue;
        };
        if layers_by_id.contains_key(layer_id) {
            state.issue(layer_ref, "layer identity is invalid or duplicated", limits)?;
            continue;
        }
        let input_layers = layer
            .pointer("/derivation/input_layers")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for input_binding in input_layers {
            source.checkpoint(limits.deadline)?;
            let input_id = input_binding.get("layer_id").and_then(Value::as_str);
            let predecessor = input_id.and_then(|value| layers_by_id.get(value));
            let (Some(input_id), Some(predecessor)) = (input_id, predecessor) else {
                state.issue(
                    layer_ref,
                    "input layer is absent or occurs after its successor",
                    limits,
                )?;
                continue;
            };
            let predecessor_ref = record_ref_by_id
                .get(input_id)
                .expect("predecessor reference recorded with its layer");
            let predecessor_digest = state.member_digests.get(predecessor_ref);
            if input_binding.get("record_ref").and_then(Value::as_str)
                != Some(predecessor_ref.as_str())
                || input_binding.get("record_sha256").and_then(Value::as_str)
                    != predecessor_digest.map(String::as_str)
                || !python_optional_equal(
                    input_binding.get("content_sha256"),
                    predecessor.pointer("/representation/content_sha256"),
                )?
            {
                state.issue(
                    layer_ref,
                    "input layer digest-bound closure drifted",
                    limits,
                )?;
                continue;
            }
            let payload = layer.pointer("/derivation/change_payload");
            if payload
                .and_then(|value| value.get("kind"))
                .and_then(Value::as_str)
                == Some("explicit_operations")
            {
                let operations = payload
                    .and_then(|value| value.get("operations"))
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let replay = selected_text_by_id
                    .get(input_id)
                    .ok_or_else(|| "source text layer predecessor text missing".to_owned())
                    .and_then(|input_text| {
                        replay_source_text_layer_edits(input_text, &selected, operations)
                    });
                if let Err(error) = replay {
                    state.issue(
                        layer_ref,
                        format!(
                            "explicit change replay failed: {}",
                            text_replay_owner_reason(&error)
                        ),
                        limits,
                    )?;
                }
            }
        }
        if !input_layers.is_empty()
            && !python_optional_equal(
                layer.get("supersedes_layer_ref"),
                input_layers
                    .last()
                    .and_then(|binding| binding.get("layer_id")),
            )?
        {
            state.issue(
                layer_ref,
                "successor does not name its immediate input as superseded",
                limits,
            )?;
        }
        if layer.pointer("/derivation/method").and_then(Value::as_str)
            == Some("unicode_normalization")
            && let Some(input_id) = input_layers
                .last()
                .and_then(|binding| binding.get("layer_id"))
                .and_then(Value::as_str)
            && let Some(predecessor_text) = selected_text_by_id.get(input_id)
            && predecessor_text.nfc().collect::<String>() != selected
        {
            state.issue(
                layer_ref,
                "declared NFC derivation does not reproduce the output",
                limits,
            )?;
        }
        if id == "B" && Some(selected.as_str()) != state.report["source_anchor_selection"].as_str()
        {
            state.issue(
                layer_ref,
                "diplomatic candidate differs from the exact source-anchor selection",
                limits,
            )?;
        }

        for field in [
            layer.get("layer_id"),
            layer.get("layer_role"),
            layer.pointer("/admission/review_status"),
            layer.pointer("/admission/accepted_uses"),
            layer.pointer("/representation/publication_authorized"),
            layer.pointer("/representation/rights_record_refs"),
        ]
        .into_iter()
        .flatten()
        {
            state.reserve_decoded(field, limits)?;
        }
        let report_keys = [
            "variant_id",
            "layer_id",
            "layer_role",
            "text",
            "text_sha256",
            "review_status",
            "accepted_uses",
            "publication_authorized",
            "rights_record_refs",
        ];
        let report_scaffold = report_keys.into_iter().try_fold(0usize, |bytes, key| {
            bytes
                .checked_add(std::mem::size_of::<Value>() + 192)
                .and_then(|bytes| bytes.checked_add(key.len().saturating_mul(2)))
                .ok_or(ItemRefusal::Budget)
        })?;
        let map_overhead = 4usize
            .checked_mul(std::mem::size_of::<(String, Value)>() + 64)
            .ok_or(ItemRefusal::Budget)?;
        let retained = report_scaffold
            .checked_add(map_overhead)
            .and_then(|bytes| bytes.checked_add(estimate_text_storage(id).ok()?))
            .and_then(|bytes| {
                bytes.checked_add(estimate_text_storage(&selected).ok()?.checked_mul(2)?)
            })
            .and_then(|bytes| bytes.checked_add(estimate_text_storage(&selected_digest).ok()?))
            .and_then(|bytes| {
                bytes.checked_add(estimate_text_storage(layer_id).ok()?.checked_mul(3)?)
            })
            .and_then(|bytes| bytes.checked_add(estimate_text_storage(layer_ref).ok()?))
            .ok_or(ItemRefusal::Budget)?;
        state.reserve(retained, limits)?;
        let indexed_layer = state.clone_value_charged(&layer, limits)?;
        report_rows.push(json!({
            "variant_id": id,
            "layer_id": layer.get("layer_id").cloned().unwrap_or(Value::Null),
            "layer_role": layer.get("layer_role").cloned().unwrap_or(Value::Null),
            "text": selected.clone(),
            "text_sha256": selected_digest,
            "review_status": layer.pointer("/admission/review_status").cloned().unwrap_or(Value::Null),
            "accepted_uses": layer.pointer("/admission/accepted_uses").cloned().unwrap_or(Value::Null),
            "publication_authorized": layer.pointer("/representation/publication_authorized").cloned().unwrap_or(Value::Null),
            "rights_record_refs": layer.pointer("/representation/rights_record_refs").cloned().unwrap_or(Value::Null),
        }));
        layers_by_id.insert(layer_id.into(), indexed_layer);
        record_ref_by_id.insert(layer_id.into(), layer_ref.into());
        selected_text_by_id.insert(layer_id.into(), selected);
        layers_by_variant.insert(id.into(), layer);
    }
    state.report["variants"] = Value::Array(report_rows);
    if let (Some(layer_a), Some(layer_b), Some(layer_c)) = (
        layers_by_variant.get("A"),
        layers_by_variant.get("B"),
        layers_by_variant.get("C"),
    ) {
        const CONTRACT: &str = "ToS/contracts/source-text-layer.schema.json";
        let layer_b_ref = variants
            .iter()
            .find(|row| row.get("variant_id").and_then(Value::as_str) == Some("B"))
            .and_then(|row| row.get("layer_ref"))
            .and_then(Value::as_str)
            .unwrap_or(path);
        let layer_c_ref = variants
            .iter()
            .find(|row| row.get("variant_id").and_then(Value::as_str) == Some("C"))
            .and_then(|row| row.get("layer_ref"))
            .and_then(Value::as_str)
            .unwrap_or(path);
        let mut silent_correction = state.clone_value_charged(&layer_b, limits)?;
        set_json_pointer(
            &mut silent_correction,
            "/derivation/change_payload",
            json!({"kind":"none"}),
        );
        schedule_text_layer_schema_control(
            &mut state,
            "silent_correction_without_operations",
            layer_b_ref,
            CONTRACT,
            &silent_correction,
            limits,
        )?;

        let input_record_drift_rejected = layer_b
            .pointer("/derivation/input_layers/0")
            .and_then(|binding| binding.get("record_ref"))
            .and_then(Value::as_str)
            .zip(
                layer_b
                    .pointer("/derivation/input_layers/0/record_sha256")
                    .and_then(Value::as_str),
            )
            .is_none_or(|(record_ref, recorded_digest)| {
                state
                    .member_digests
                    .get(record_ref)
                    .is_none_or(|actual| actual != recorded_digest)
                    || "0".repeat(64)
                        != state
                            .member_digests
                            .get(record_ref)
                            .map(String::as_str)
                            .unwrap_or("")
            });
        record_text_layer_control(
            &mut state,
            path,
            "input_record_digest_drift",
            input_record_drift_rejected,
            limits,
        )?;

        let mut edit_mismatch = state.clone_value_charged(&layer_b, limits)?;
        set_json_pointer(
            &mut edit_mismatch,
            "/derivation/change_payload/operations/0/output_sha256",
            Value::String("0".repeat(64)),
        );
        let edit_mismatch_rejected = source_text_layer_semantic_issues(&edit_mismatch)
            .iter()
            .any(|message| message.contains("output text digest drifted"));
        record_text_layer_control(
            &mut state,
            path,
            "edit_span_or_digest_mismatch",
            edit_mismatch_rejected,
            limits,
        )?;

        let mut unreviewed_use = state.clone_value_charged(&layer_b, limits)?;
        set_json_pointer(
            &mut unreviewed_use,
            "/admission/accepted_uses",
            json!(["citation"]),
        );
        schedule_text_layer_schema_control(
            &mut state,
            "unreviewed_layer_claims_accepted_use",
            layer_b_ref,
            CONTRACT,
            &unreviewed_use,
            limits,
        )?;

        let mut normalized_diplomatic_use = state.clone_value_charged(&layer_c, limits)?;
        set_json_pointer(
            &mut normalized_diplomatic_use,
            "/admission/review_status",
            json!("accepted"),
        );
        set_json_pointer(
            &mut normalized_diplomatic_use,
            "/admission/review_ref",
            json!("tos.review.synthetic.source-text-layer"),
        );
        set_json_pointer(
            &mut normalized_diplomatic_use,
            "/admission/human_review_performed",
            json!(true),
        );
        set_json_pointer(
            &mut normalized_diplomatic_use,
            "/admission/accepted_uses",
            json!(["diplomatic_transcription"]),
        );
        schedule_text_layer_schema_control(
            &mut state,
            "normalized_layer_claims_diplomatic_use",
            layer_c_ref,
            CONTRACT,
            &normalized_diplomatic_use,
            limits,
        )?;

        let mut language_use = state.clone_value_charged(&layer_b, limits)?;
        set_json_pointer(
            &mut language_use,
            "/admission/review_status",
            json!("accepted_with_limits"),
        );
        set_json_pointer(
            &mut language_use,
            "/admission/review_ref",
            json!("tos.review.synthetic.source-text-layer"),
        );
        set_json_pointer(
            &mut language_use,
            "/admission/human_review_performed",
            json!(true),
        );
        set_json_pointer(
            &mut language_use,
            "/admission/human_language_competence",
            json!("blocked"),
        );
        set_json_pointer(
            &mut language_use,
            "/admission/accepted_uses",
            json!(["translation_source"]),
        );
        schedule_text_layer_schema_control(
            &mut state,
            "language_sensitive_use_without_competence",
            layer_b_ref,
            CONTRACT,
            &language_use,
            limits,
        )?;

        let mut tracked_nonpublic = state.clone_value_charged(&layer_b, limits)?;
        set_json_pointer(
            &mut tracked_nonpublic,
            "/representation/content_visibility",
            json!("local_only"),
        );
        set_json_pointer(
            &mut tracked_nonpublic,
            "/representation/publication_authorized",
            json!(false),
        );
        let tracked_nonpublic_rejected = source_text_layer_semantic_issues(&tracked_nonpublic)
            .iter()
            .any(|message| message == "tracked source text layer must be public content");
        record_text_layer_control(
            &mut state,
            path,
            "tracked_nonpublic_explicit_text",
            tracked_nonpublic_rejected,
            limits,
        )?;

        let mut publication_without_authority = state.clone_value_charged(&layer_b, limits)?;
        set_json_pointer(
            &mut publication_without_authority,
            "/representation/publication_authorized",
            json!(false),
        );
        set_json_pointer(
            &mut publication_without_authority,
            "/admission/review_status",
            json!("accepted"),
        );
        set_json_pointer(
            &mut publication_without_authority,
            "/admission/review_ref",
            json!("tos.review.synthetic.source-text-layer"),
        );
        set_json_pointer(
            &mut publication_without_authority,
            "/admission/human_review_performed",
            json!(true),
        );
        set_json_pointer(
            &mut publication_without_authority,
            "/admission/accepted_uses",
            json!(["publication"]),
        );
        schedule_text_layer_schema_control(
            &mut state,
            "publication_use_without_publication_authority",
            layer_b_ref,
            CONTRACT,
            &publication_without_authority,
            limits,
        )?;

        let mut self_supersession = state.clone_value_charged(&layer_b, limits)?;
        let self_id = self_supersession
            .get("layer_id")
            .cloned()
            .unwrap_or(Value::Null);
        set_json_pointer(&mut self_supersession, "/supersedes_layer_ref", self_id);
        let self_supersession_rejected = source_text_layer_semantic_issues(&self_supersession)
            .iter()
            .any(|message| message == "source text layer cannot supersede itself");
        record_text_layer_control(
            &mut state,
            path,
            "self_supersession",
            self_supersession_rejected,
            limits,
        )?;
    }
    let declared_controls: std::collections::BTreeSet<String> = manifest
        .get("negative_controls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let reported_controls: std::collections::BTreeSet<String> = state.report["negative_controls"]
        .as_object()
        .map(|controls| controls.keys().cloned().collect())
        .unwrap_or_default();
    if declared_controls != reported_controls {
        state.issue(
            path,
            "negative-control coverage differs from manifest",
            limits,
        )?;
    }
    result(
        SourceFoundationLab::SourceTextLayer,
        state,
        Vec::new(),
        limits,
    )
}

fn schedule_text_layer_schema_control(
    state: &mut DirectState,
    control: &str,
    location: &str,
    contract: &str,
    instance: &Value,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    state.report["negative_controls"][control] = json!("pending_schema_diagnostic");
    state.schema_check_with_control(location, contract, instance, Some(control), limits)
}

fn record_direct_control(
    state: &mut DirectState,
    controls: &mut Value,
    location: &str,
    name: &str,
    rejected: bool,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    controls[name] = Value::String(if rejected { "rejected" } else { "not_rejected" }.into());
    if !rejected {
        state.issue(
            location,
            format!("negative control was not rejected: {name}"),
            limits,
        )?;
    }
    Ok(())
}

fn schedule_schema_negative_control(
    state: &mut DirectState,
    controls: &mut Value,
    location: &str,
    contract: &str,
    name: &str,
    payload: &Value,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    let slot = format!("/negative_controls/{name}");
    state.schema_check_with_result(
        location,
        contract,
        payload,
        Some(name),
        Some(false),
        None,
        Some(&slot),
        Some(format!("negative control was not rejected: {name}")),
        limits,
    )?;
    controls[name] = Value::String("pending_schema_diagnostic".into());
    Ok(())
}

fn record_text_layer_control(
    state: &mut DirectState,
    location: &str,
    control: &str,
    rejected: bool,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    if rejected {
        state.report["negative_controls"][control] = json!("rejected");
    } else {
        state.issue(
            location,
            format!("negative control was not rejected: {control}"),
            limits,
        )?;
        state.report["negative_controls"][control] = json!("not_rejected");
    }
    Ok(())
}

fn selected_layer_text(layer: &Value, raw_content: &[u8]) -> Result<String, String> {
    let representation = layer
        .get("representation")
        .and_then(Value::as_object)
        .ok_or_else(|| "text-layer representation is absent".to_owned())?;
    let expected_digest = representation
        .get("content_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "text-layer representation digest is absent".to_owned())?;
    if Digest256::of_bytes(raw_content).to_hex() != expected_digest {
        return Err("text-layer representation digest drifted".into());
    }
    let text = std::str::from_utf8(raw_content)
        .map_err(|_| "text-layer representation is not UTF-8 text".to_owned())?;
    let scope = representation
        .get("text_scope")
        .and_then(Value::as_object)
        .ok_or_else(|| "text-layer representation scope leaves the content".to_owned())?;
    let start = scope
        .get("start")
        .and_then(python_integer)
        .and_then(|value| u64::try_from(value).ok())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "text-layer representation scope leaves the content".to_owned())?;
    let end = scope
        .get("end")
        .and_then(python_integer)
        .and_then(|value| u64::try_from(value).ok())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "text-layer representation scope leaves the content".to_owned())?;
    let points = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    if end < start || end >= points.len() {
        return Err("text-layer representation scope leaves the content".into());
    }
    let selected = &text[points[start]..points[end]];
    let normalized = match representation
        .get("character_normalization")
        .and_then(Value::as_str)
    {
        Some("NFC") => Some(selected.nfc().collect::<String>()),
        Some("NFD") => Some(selected.nfd().collect::<String>()),
        Some("NFKC") => Some(selected.nfkc().collect::<String>()),
        Some("NFKD") => Some(selected.nfkd().collect::<String>()),
        _ => None,
    };
    if normalized.as_deref().is_some_and(|value| value != selected) {
        return Err("text-layer representation contradicts its Unicode normalization".into());
    }
    Ok(selected.to_owned())
}

fn source_text_layer_semantic_issues(layer: &Value) -> Vec<String> {
    let mut messages = Vec::new();
    let layer_id = layer.get("layer_id").and_then(Value::as_str);
    if layer.get("supersedes_layer_ref").and_then(Value::as_str) == layer_id {
        messages.push("source text layer cannot supersede itself".into());
    }

    let representation = layer.get("representation").unwrap_or(&Value::Null);
    let scope = representation.get("text_scope").unwrap_or(&Value::Null);
    if let (Some(start), Some(end)) = (
        scope.get("start").and_then(python_integer),
        scope.get("end").and_then(python_integer),
    ) {
        if end < start {
            messages.push("representation text scope is reversed".into());
        }
    }
    let storage = representation.get("storage").and_then(Value::as_str);
    let tracked_content = representation
        .get("tracked_content")
        .and_then(Value::as_bool);
    let visibility = representation
        .get("content_visibility")
        .and_then(Value::as_str);
    if (storage == Some("tracked")) != (tracked_content == Some(true)) {
        messages.push("representation storage and tracked-content posture disagree".into());
    }
    if tracked_content == Some(true) && visibility != Some("public") {
        messages.push("tracked source text layer must be public content".into());
    }
    let publication_authorized = representation
        .get("publication_authorized")
        .and_then(Value::as_bool);
    let publication_authority_refs = representation
        .get("publication_authority_refs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if publication_authorized == Some(true) && visibility != Some("public") {
        messages.push("nonpublic source text layer cannot authorize publication".into());
    }
    if publication_authorized == Some(true) && publication_authority_refs.is_empty() {
        messages.push("publication authorization has no authority reference".into());
    }
    if publication_authorized != Some(true) && !publication_authority_refs.is_empty() {
        messages.push("publication authority references contradict the closed gate".into());
    }

    let anchor_ids: std::collections::BTreeSet<&str> = layer
        .pointer("/source_binding/anchors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|binding| binding.get("anchor_id").and_then(Value::as_str))
        .collect();
    let derivation = layer.get("derivation").unwrap_or(&Value::Null);
    let input_layers = derivation
        .get("input_layers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if input_layers
        .iter()
        .any(|item| item.get("layer_id").and_then(Value::as_str) == layer_id)
    {
        messages.push("source text layer cannot derive from itself".into());
    }

    let change_payload = derivation.get("change_payload").unwrap_or(&Value::Null);
    let operations =
        if change_payload.get("kind").and_then(Value::as_str) == Some("explicit_operations") {
            change_payload
                .get("operations")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        } else {
            &[]
        };
    let mut last_input_end = -1i128;
    let mut last_output_end = -1i128;
    for operation in operations {
        let Some(operation) = operation.as_object() else {
            continue;
        };
        let operation_value = Value::Object(operation.clone());
        let edit_id = operation_value
            .get("edit_id")
            .and_then(Value::as_str)
            .unwrap_or("None");
        let input_span = operation_value.get("input_span").unwrap_or(&Value::Null);
        let output_span = operation_value.get("output_span").unwrap_or(&Value::Null);
        let input_start = input_span.get("start").and_then(python_integer);
        let input_end = input_span.get("end").and_then(python_integer);
        let output_start = output_span.get("start").and_then(python_integer);
        let output_end = output_span.get("end").and_then(python_integer);
        if let (Some(start), Some(end)) = (input_start, input_end) {
            if end < start {
                messages.push(format!("{edit_id} input span is reversed"));
            }
            if start < last_input_end {
                messages.push("explicit input edit spans overlap or are out of order".into());
            }
            last_input_end = last_input_end.max(end);
        }
        if let (Some(start), Some(end)) = (output_start, output_end) {
            if end < start {
                messages.push(format!("{edit_id} output span is reversed"));
            }
            if start < last_output_end {
                messages.push("explicit output edit spans overlap or are out of order".into());
            }
            last_output_end = last_output_end.max(end);
        }
        let input_exact = operation_value.get("input_exact").and_then(Value::as_str);
        let output_exact = operation_value.get("output_exact").and_then(Value::as_str);
        if let Some(input_exact) = input_exact {
            let expected = Digest256::of_bytes(input_exact.as_bytes()).to_hex();
            if operation_value.get("input_sha256").and_then(Value::as_str)
                != Some(expected.as_str())
            {
                messages.push(format!("{edit_id} input text digest drifted"));
            }
            if let (Some(start), Some(end)) = (input_start, input_end) {
                let span_len = end.checked_sub(start).and_then(|n| usize::try_from(n).ok());
                if span_len != Some(input_exact.chars().count()) {
                    messages.push(format!("{edit_id} input span length differs from text"));
                }
            }
        }
        if let Some(output_exact) = output_exact {
            let expected = Digest256::of_bytes(output_exact.as_bytes()).to_hex();
            if operation_value.get("output_sha256").and_then(Value::as_str)
                != Some(expected.as_str())
            {
                messages.push(format!("{edit_id} output text digest drifted"));
            }
            if let (Some(start), Some(end)) = (output_start, output_end) {
                let span_len = end.checked_sub(start).and_then(|n| usize::try_from(n).ok());
                if span_len != Some(output_exact.chars().count()) {
                    messages.push(format!("{edit_id} output span length differs from text"));
                }
            }
        }
        let evidence: std::collections::BTreeSet<&str> = operation_value
            .get("evidence_anchor_refs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if evidence.is_empty() || !evidence.is_subset(&anchor_ids) {
            messages.push(format!(
                "{edit_id} evidence anchors leave the source binding"
            ));
        }
    }

    let uncertainty = layer.get("uncertainty").unwrap_or(&Value::Null);
    for annotation in uncertainty
        .get("annotations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if annotation
            .get("anchor_ref")
            .and_then(Value::as_str)
            .is_none_or(|reference| !anchor_ids.contains(reference))
        {
            messages.push("uncertainty annotation leaves the source binding".into());
        }
        for alternative in annotation
            .get("alternatives")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let value = alternative.get("value");
            let in_record = alternative.get("value_in_record").and_then(Value::as_bool);
            if in_record == Some(true) && !value.is_some_and(Value::is_string) {
                messages.push("in-record uncertainty alternative omits its value".into());
            }
            if in_record == Some(false) && value.is_some_and(|item| !item.is_null()) {
                messages.push("withheld uncertainty alternative exposes a value".into());
            }
            if let Some(value) = value.and_then(Value::as_str) {
                let expected = Digest256::of_bytes(value.as_bytes()).to_hex();
                if alternative.get("value_sha256").and_then(Value::as_str)
                    != Some(expected.as_str())
                {
                    messages.push("uncertainty alternative digest drifted".into());
                }
            }
        }
    }
    messages
}

fn replay_source_text_layer_edits(
    input_text: &str,
    output_text: &str,
    operations: &[Value],
) -> Result<(), String> {
    let input: Vec<char> = input_text.chars().collect();
    let mut pieces = String::new();
    let mut input_cursor = 0usize;
    let mut output_cursor = 0usize;
    for operation in operations {
        let input_span = operation.get("input_span").unwrap_or(&Value::Null);
        let output_span = operation.get("output_span").unwrap_or(&Value::Null);
        let start = input_span
            .get("start")
            .and_then(python_integer)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                "explicit input edits overlap, reverse, or leave the input".to_owned()
            })?;
        let end = input_span
            .get("end")
            .and_then(python_integer)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                "explicit input edits overlap, reverse, or leave the input".to_owned()
            })?;
        let output_start = output_span
            .get("start")
            .and_then(python_integer)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "explicit output edit start is not replay-aligned".to_owned())?;
        let output_end = output_span
            .get("end")
            .and_then(python_integer)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "explicit output edit end is not replay-aligned".to_owned())?;
        if start < input_cursor || end < start || end > input.len() {
            return Err("explicit input edits overlap, reverse, or leave the input".into());
        }
        let unchanged: String = input[input_cursor..start].iter().collect();
        output_cursor += unchanged.chars().count();
        pieces.push_str(&unchanged);
        let input_exact = operation
            .get("input_exact")
            .and_then(Value::as_str)
            .ok_or_else(|| "explicit input edit text does not match the input bytes".to_owned())?;
        let actual_input: String = input[start..end].iter().collect();
        if actual_input != input_exact {
            return Err("explicit input edit text does not match the input bytes".into());
        }
        let input_digest = Digest256::of_bytes(input_exact.as_bytes()).to_hex();
        if operation.get("input_sha256").and_then(Value::as_str) != Some(input_digest.as_str()) {
            return Err("explicit input edit digest drifted".into());
        }
        if output_start != output_cursor {
            return Err("explicit output edit start is not replay-aligned".into());
        }
        let output_exact = operation
            .get("output_exact")
            .and_then(Value::as_str)
            .ok_or_else(|| "explicit output edit end is not replay-aligned".to_owned())?;
        if output_end != output_cursor + output_exact.chars().count() {
            return Err("explicit output edit end is not replay-aligned".into());
        }
        let output_digest = Digest256::of_bytes(output_exact.as_bytes()).to_hex();
        if operation.get("output_sha256").and_then(Value::as_str) != Some(output_digest.as_str()) {
            return Err("explicit output edit digest drifted".into());
        }
        pieces.push_str(output_exact);
        output_cursor = output_end;
        input_cursor = end;
    }
    pieces.extend(input[input_cursor..].iter().copied());
    if pieces != output_text {
        return Err("explicit edit replay does not reproduce the output layer".into());
    }
    Ok(())
}

fn inspect_provenance_v2(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let path = "ToS/research-packets/foundation-laboratory-2026-07/provenance-event-v2-abc/lab.manifest.json";
    let limit_value = json!({"private_source_used":false,"model_invoked":false,"human_evidence_created":false,"execution_truth_established":false,"content_truth_established":false,"rights_clearance_established":false,"semantic_claim_created":false,"canon_effect":false});
    let mut state = DirectState {
        report: json!({"variants":[],"negative_controls":{}}),
        ..Default::default()
    };
    let Some(manifest) = manifest(
        &mut state,
        source,
        path,
        "tos_provenance_event_v2_lab_v1",
        "public_synthetic_mechanics_only",
        &limit_value,
        limits,
    )?
    else {
        return result(SourceFoundationLab::ProvenanceV2, state, Vec::new(), limits);
    };
    manifest_authority(
        &mut state,
        path,
        &manifest,
        "tos_provenance_event_v2_lab_v1",
        "public_synthetic_mechanics_only",
        &limit_value,
        limits,
    )?;
    let mut input_fixture_ref = None;
    for field in [
        "contract",
        "plan",
        "builder",
        "environment_profile",
        "input_fixture",
    ] {
        let bound = binding(
            &mut state,
            source,
            path,
            &manifest,
            field,
            matches!(field, "contract" | "builder"),
            limits,
        )?;
        if field == "input_fixture" {
            input_fixture_ref = bound;
        }
    }
    if manifest.pointer("/contract/ref").and_then(Value::as_str)
        != Some("ToS/contracts/provenance-event-v2.schema.json")
    {
        state.issue(path, "provenance v2 contract reference drifted", limits)?;
    }
    let mut events_by_variant = std::collections::BTreeMap::<String, Value>::new();
    let mut variant_by_id = std::collections::BTreeMap::<String, Value>::new();
    let report_rows = check_packet_variants(
        &mut state,
        source,
        path,
        &manifest,
        "event_ref",
        "event_sha256",
        "ToS/contracts/provenance-event-v2.schema.json",
        None,
        "provenance",
        |state, source, id, event_path, event, _raw, row| {
            source.checkpoint(limits.deadline)?;
            let semantic = crate::provenance_rules::semantic_issues(
                event,
                limits.max_issues.saturating_sub(state.issues.len()),
                limits.deadline,
            )?;
            for message in &semantic {
                state.issue(event_path, *message, limits)?;
            }
            if !python_equal(
                event.get("record_binding").unwrap_or(&Value::Null),
                &json!({
                    "manifest_ref": path,
                    "digest_algorithm": "sha256",
                    "digest_scope": "exact_event_record_bytes"
                }),
            )? {
                state.issue(
                    event_path,
                    "event-to-manifest record binding drifted",
                    limits,
                )?;
            }
            let activity = event.get("activity").unwrap_or(&Value::Null);
            let expected_status = row.get("expected_status").unwrap_or(&Value::Null);
            let expected_exit = row.get("expected_exit_code").unwrap_or(&Value::Null);
            if !python_equal(
                activity.get("status").unwrap_or(&Value::Null),
                expected_status,
            )? || !python_equal(
                activity.get("exit_code").unwrap_or(&Value::Null),
                expected_exit,
            )? {
                state.issue(
                    event_path,
                    "event terminal state differs from the frozen expectation",
                    limits,
                )?;
            }
            let expected_command = json!([
                "python",
                "scripts/build_provenance_event_v2_lab.py",
                "--variant",
                id
            ]);
            let command = event.pointer("/method/command_capture/argv");
            if !python_optional_equal(command, Some(&expected_command))?
                || !python_equal(
                    row.get("captured_command").unwrap_or(&Value::Null),
                    &expected_command,
                )?
            {
                state.issue(
                    event_path,
                    "captured argv differs from the executed laboratory command",
                    limits,
                )?;
            }
            let input_ref = row.get("input_ref").and_then(Value::as_str);
            let input_digest = row.get("input_sha256").and_then(Value::as_str);
            let input_bytes = input_ref
                .map(|input_path| state.read(source, input_path, limits))
                .transpose()?
                .flatten();
            if input_bytes
                .as_ref()
                .is_none_or(|raw| Some(Digest256::of_bytes(raw).to_hex().as_str()) != input_digest)
            {
                state.issue(event_path, "variant input fixity drifted", limits)?;
            }
            let inputs = event.pointer("/entities/inputs").and_then(Value::as_array);
            if !inputs.is_some_and(|items| {
                items.len() == 1
                    && items[0].get("entity_ref").and_then(Value::as_str) == input_ref
                    && items[0].get("sha256").and_then(Value::as_str) == input_digest
            }) {
                state.issue(
                    event_path,
                    "event input binding differs from the manifest",
                    limits,
                )?;
            }
            let output_ref = row.get("output_ref").and_then(Value::as_str);
            let output_digest = row.get("output_sha256").and_then(Value::as_str);
            let output_bytes = output_ref
                .map(|output_path| state.read(source, output_path, limits))
                .transpose()?
                .flatten();
            let outputs = event.pointer("/entities/outputs").and_then(Value::as_array);
            if let Some(output_ref) = output_ref {
                if output_bytes.as_ref().is_none_or(|raw| {
                    Some(Digest256::of_bytes(raw).to_hex().as_str()) != output_digest
                }) {
                    state.issue(event_path, "variant output fixity drifted", limits)?;
                }
                if !outputs.is_some_and(|items| {
                    items.len() == 1
                        && items[0].get("entity_ref").and_then(Value::as_str) == Some(output_ref)
                        && items[0].get("sha256").and_then(Value::as_str) == output_digest
                }) {
                    state.issue(
                        event_path,
                        "event output binding differs from the manifest",
                        limits,
                    )?;
                }
            } else if outputs.is_some_and(|items| !items.is_empty()) {
                state.issue(
                    event_path,
                    "failed variant exposes an authoritative output",
                    limits,
                )?;
            }
            let byproduct_ref = row.get("byproduct_ref").and_then(Value::as_str);
            let byproduct_digest = row.get("byproduct_sha256").and_then(Value::as_str);
            let byproduct_bytes = byproduct_ref
                .map(|byproduct_path| state.read(source, byproduct_path, limits))
                .transpose()?
                .flatten();
            let byproducts = event
                .pointer("/entities/byproducts")
                .and_then(Value::as_array);
            if let Some(byproduct_ref) = byproduct_ref {
                if byproduct_bytes.as_ref().is_none_or(|raw| {
                    Some(Digest256::of_bytes(raw).to_hex().as_str()) != byproduct_digest
                }) {
                    state.issue(event_path, "variant byproduct fixity drifted", limits)?;
                }
                if !byproducts.is_some_and(|items| {
                    items.len() == 1
                        && items[0].get("entity_ref").and_then(Value::as_str) == Some(byproduct_ref)
                        && items[0].get("sha256").and_then(Value::as_str) == byproduct_digest
                }) {
                    state.issue(
                        event_path,
                        "event byproduct binding differs from the manifest",
                        limits,
                    )?;
                }
            } else if byproducts.is_some_and(|items| !items.is_empty()) {
                state.issue(
                    event_path,
                    "successful variant unexpectedly carries a failure byproduct",
                    limits,
                )?;
            }
            let relation = row.get("expected_relation").cloned().unwrap_or(Value::Null);
            let relations = event
                .get("derivations")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.get("relation").cloned().unwrap_or(Value::Null))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let expected_relations = if relation.is_null() {
                json!([])
            } else {
                json!([relation])
            };
            if !python_equal(&json!(relations), &expected_relations)? {
                state.issue(
                    event_path,
                    "event derivation relation differs from the frozen expectation",
                    limits,
                )?;
            }
            if !python_equal(
                event
                    .pointer("/method/model_invocations")
                    .unwrap_or(&Value::Null),
                &json!([]),
            )? {
                state.issue(
                    event_path,
                    "public synthetic provenance lab invoked a model",
                    limits,
                )?;
            }
            if event
                .get("responsibility")
                .and_then(Value::as_array)
                .is_some_and(|actors| {
                    actors.iter().any(|actor| {
                        actor.get("agent_kind").and_then(Value::as_str) == Some("human")
                    })
                })
            {
                state.issue(
                    event_path,
                    "public synthetic provenance lab fabricated human evidence",
                    limits,
                )?;
            }
            if event
                .pointer("/review_and_authority/human_review_status")
                .and_then(Value::as_str)
                != Some("not_performed")
            {
                state.issue(
                    event_path,
                    "public synthetic provenance lab claims human review",
                    limits,
                )?;
            }
            if event
                .pointer("/rights_and_visibility/publication_authorized")
                .and_then(Value::as_bool)
                != Some(false)
            {
                state.issue(
                    event_path,
                    "public synthetic provenance lab widened publication authority",
                    limits,
                )?;
            }
            state.reserve_decoded(event, limits)?;
            state.reserve_decoded(row, limits)?;
            events_by_variant.insert(id.to_owned(), event.clone());
            variant_by_id.insert(id.to_owned(), row.clone());
            let relation = row.get("expected_relation").cloned().unwrap_or(Value::Null);
            let report_row = json!({"variant_id":id,"status":activity.get("status").cloned().unwrap_or(Value::Null),"exit_code":activity.get("exit_code").cloned().unwrap_or(Value::Null),"input_sha256":row.get("input_sha256").cloned().unwrap_or(Value::Null),"output_sha256":row.get("output_sha256").cloned().unwrap_or(Value::Null),"byproduct_sha256":row.get("byproduct_sha256").cloned().unwrap_or(Value::Null),"relation":relation,"signature_status":event.pointer("/evidence_authentication/signature_status").cloned().unwrap_or(Value::Null),"human_review_status":event.pointer("/review_and_authority/human_review_status").cloned().unwrap_or(Value::Null)});
            state.reserve_decoded(&report_row, limits)?;
            Ok(report_row)
        },
        limits,
    )?;
    state.report["variants"] = Value::Array(report_rows);
    if events_by_variant
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        == std::collections::BTreeSet::from(["A", "B", "C"])
    {
        let input_raw = input_fixture_ref
            .as_deref()
            .map(|input| state.read(source, input, limits))
            .transpose()?
            .flatten();
        let a_output = variant_by_id["A"]
            .get("output_ref")
            .and_then(Value::as_str)
            .map(|output| state.read(source, output, limits))
            .transpose()?
            .flatten();
        let b_output = variant_by_id["B"]
            .get("output_ref")
            .and_then(Value::as_str)
            .map(|output| state.read(source, output, limits))
            .transpose()?
            .flatten();
        let c_byproduct = variant_by_id["C"]
            .get("byproduct_ref")
            .and_then(Value::as_str)
            .map(|byproduct| state.read(source, byproduct, limits))
            .transpose()?
            .flatten();
        if a_output.as_ref() != input_raw.as_ref() {
            state.issue(path, "variant A is not a byte-identical copy", limits)?;
        }
        if let (Some(input_raw), Some(b_output)) = (input_raw.as_deref(), b_output.as_deref()) {
            match (
                std::str::from_utf8(input_raw),
                std::str::from_utf8(b_output),
            ) {
                (Ok(source_text), Ok(b_text)) => {
                    let normalized = source_text.nfc().collect::<String>();
                    if b_text != normalized || b_output == input_raw {
                        state.issue(
                            path,
                            "variant B does not preserve the exact NFC transformation",
                            limits,
                        )?;
                    }
                }
                _ => state.issue(path, "synthetic text is not UTF-8", limits)?,
            }
        } else {
            state.issue(path, "synthetic text is not UTF-8", limits)?;
        }
        if c_byproduct.as_deref() != Some(b"status=failed\nreason=non_ascii_input\nexit_code=7\n") {
            state.issue(path, "variant C failure byproduct drifted", limits)?;
        }

        let mut controls = Value::Object(serde_json::Map::new());
        let a = &events_by_variant["A"];
        let b = &events_by_variant["B"];
        let c = &events_by_variant["C"];
        let a_row = &variant_by_id["A"];
        let a_event_path = a_row
            .get("event_ref")
            .and_then(Value::as_str)
            .unwrap_or(path);
        let a_event_digest = state.member_digests.get(a_event_path).map(String::as_str);
        let zero_digest = "0".repeat(64);
        let event_record_digest_drift = a_event_digest != Some(zero_digest.as_str());
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "event_record_digest_drift",
            event_record_digest_drift,
            limits,
        )?;

        let input_ref = a
            .pointer("/entities/inputs/0/entity_ref")
            .and_then(Value::as_str);
        let input_file_digest =
            input_ref.and_then(|input| state.member_digests.get(input).map(String::as_str));
        let input_fixity_drift = input_file_digest.is_some_and(|digest| {
            Some(digest)
                != a.pointer("/entities/inputs/0/sha256")
                    .and_then(Value::as_str)
        });
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "input_fixity_drift",
            input_fixity_drift,
            limits,
        )?;

        let semantic_rejected =
            |event: &Value, expected: &str| -> Result<bool, ItemRefusal> {
                Ok(crate::provenance_rules::semantic_issues(
                    event,
                    limits.max_issues,
                    limits.deadline,
                )?
                .iter()
                .any(|message| *message == expected))
            };
        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(
            &mut mutated,
            "/method/command_capture/argv_sha256",
            json!("0".repeat(64)),
        );
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "command_digest_drift",
            semantic_rejected(&mutated, "inline command argv digest drifted")?,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(&mut mutated, "/entities/outputs", json!([]));
        set_json_pointer(&mut mutated, "/derivations", json!([]));
        schedule_schema_negative_control(
            &mut state,
            &mut controls,
            path,
            "ToS/contracts/provenance-event-v2.schema.json",
            "completed_without_output",
            &mutated,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&c, limits)?;
        set_json_pointer(
            &mut mutated,
            "/entities/outputs",
            a.pointer("/entities/outputs")
                .cloned()
                .unwrap_or_else(|| json!([])),
        );
        schedule_schema_negative_control(
            &mut state,
            &mut controls,
            path,
            "ToS/contracts/provenance-event-v2.schema.json",
            "failed_with_authoritative_output",
            &mutated,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(
            &mut mutated,
            "/entities/outputs/0/sha256",
            json!("0".repeat(64)),
        );
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "identity_copy_content_drift",
            semantic_rejected(
                &mutated,
                "identity copy does not preserve exact bytes and size",
            )?,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&b, limits)?;
        set_json_pointer(
            &mut mutated,
            "/derivations/0/output_entity_ref",
            json!("outside:event-output"),
        );
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "derivation_endpoint_escape",
            semantic_rejected(
                &mutated,
                "derivation output leaves the authoritative output entities",
            )?,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(
            &mut mutated,
            "/activity/event_type",
            json!("model_inference"),
        );
        schedule_schema_negative_control(
            &mut state,
            &mut controls,
            path,
            "ToS/contracts/provenance-event-v2.schema.json",
            "model_event_without_invocation",
            &mutated,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(
            &mut mutated,
            "/method/command_capture/disclosure",
            json!("withheld_digest_only"),
        );
        set_json_pointer(&mut mutated, "/method/command_capture/argv", Value::Null);
        set_json_pointer(
            &mut mutated,
            "/method/command_capture/withholding_reason",
            json!("synthetic negative"),
        );
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "replay_ready_with_withheld_command",
            semantic_rejected(&mutated, "replay-ready provenance withholds its command")?,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(
            &mut mutated,
            "/evidence_authentication/verification_status",
            json!("signature_verified"),
        );
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "unsigned_claimed_signature_verification",
            semantic_rejected(
                &mutated,
                "unsigned provenance event claims signature verification",
            )?,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/publication_authorized",
            json!(true),
        );
        schedule_schema_negative_control(
            &mut state,
            &mut controls,
            path,
            "ToS/contracts/provenance-event-v2.schema.json",
            "publication_without_authority",
            &mutated,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(
            &mut mutated,
            "/review_and_authority/human_review_status",
            json!("performed"),
        );
        set_json_pointer(
            &mut mutated,
            "/review_and_authority/review_bindings",
            json!([manifest.get("plan").cloned().unwrap_or(Value::Null)]),
        );
        schedule_schema_negative_control(
            &mut state,
            &mut controls,
            path,
            "ToS/contracts/provenance-event-v2.schema.json",
            "unattested_human_review",
            &mutated,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        set_json_pointer(&mut mutated, "/manual_changes/status", json!("recorded"));
        schedule_schema_negative_control(
            &mut state,
            &mut controls,
            path,
            "ToS/contracts/provenance-event-v2.schema.json",
            "manual_change_without_receipt",
            &mutated,
            limits,
        )?;

        let mut mutated = state.clone_value_charged(&a, limits)?;
        let event_id = mutated.pointer("/event_id").cloned().unwrap_or(Value::Null);
        set_json_pointer(&mut mutated, "/supersedes_event_ref", event_id);
        record_direct_control(
            &mut state,
            &mut controls,
            path,
            "self_supersession",
            semantic_rejected(&mutated, "provenance event cannot supersede itself")?,
            limits,
        )?;
        let expected_controls: std::collections::BTreeSet<String> = manifest
            .get("negative_controls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let reported_controls: std::collections::BTreeSet<String> = controls
            .as_object()
            .map(|object| object.keys().cloned().collect())
            .unwrap_or_default();
        if expected_controls != reported_controls {
            state.issue(
                path,
                "negative-control coverage differs from manifest",
                limits,
            )?;
        }
        state.reserve_decoded(&controls, limits)?;
        state.report["negative_controls"] = controls;
    } else if manifest
        .get("negative_controls")
        .and_then(Value::as_array)
        .is_some_and(|rows| !rows.is_empty())
    {
        state.issue(
            path,
            "negative-control coverage differs from manifest",
            limits,
        )?;
    }
    result(SourceFoundationLab::ProvenanceV2, state, Vec::new(), limits)
}

fn inspect_semantic_annotation_v2(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let path = "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/lab.manifest.json";
    let limit_value = json!({"private_source_used":false,"model_invoked":false,"human_review_performed":false,"stable_sign_established":false,"concept_established":false,"graph_truth_established":false,"semantic_truth_established":false,"canon_effect":false});
    let mut state = DirectState {
        report: json!({"variants":[],"negative_controls":{}}),
        ..Default::default()
    };
    let Some(manifest) = manifest(
        &mut state,
        source,
        path,
        "tos_semantic_annotation_v2_lab_manifest_v1",
        "public_synthetic_contract_mechanics_only",
        &limit_value,
        limits,
    )?
    else {
        return result(
            SourceFoundationLab::SemanticAnnotationV2,
            state,
            Vec::new(),
            limits,
        );
    };
    manifest_authority(
        &mut state,
        path,
        &manifest,
        "tos_semantic_annotation_v2_lab_manifest_v1",
        "public_synthetic_contract_mechanics_only",
        &limit_value,
        limits,
    )?;
    let mut plan_ref = None;
    let mut input_ref = None;
    for field in ["contract", "research", "plan", "builder", "input_fixture"] {
        let bound = binding(&mut state, source, path, &manifest, field, false, limits)?;
        match field {
            "plan" => plan_ref = bound,
            "input_fixture" => input_ref = bound,
            _ => {}
        }
    }
    if manifest.pointer("/contract/ref").and_then(Value::as_str)
        != Some("ToS/contracts/semantic-annotation-packet-v2.schema.json")
    {
        state.issue(
            path,
            "semantic annotation contract reference drifted",
            limits,
        )?;
    }
    let plan_path = plan_ref.as_deref().unwrap_or(path);
    let plan = match state.read_json(source, plan_path, limits)? {
        Some((value, _)) if value.is_object() => value,
        Some(_) => {
            state.issue(plan_path, "JSON root must be an object", limits)?;
            Value::Null
        }
        None => Value::Null,
    };
    if !python_optional_equal(plan.get("authority_limits"), Some(&limit_value))? {
        state.issue(
            path,
            "semantic annotation plan authority limits drifted",
            limits,
        )?;
    }
    let expected_controls: std::collections::BTreeSet<String> = plan
        .get("negative_controls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let input_path = input_ref.as_deref().unwrap_or(path);
    let input_raw = match state.read(source, input_path, limits)? {
        Some(raw) => raw,
        None => {
            state.issue(input_path, "file is missing", limits)?;
            Vec::new()
        }
    };
    state.reserve(input_raw.len(), limits)?;
    let source_text = match std::str::from_utf8(&input_raw) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            state.issue(
                path,
                "cannot read public synthetic source: invalid UTF-8",
                limits,
            )?;
            String::new()
        }
    };
    let input_digest = Digest256::of_bytes(&input_raw).to_hex();
    let mut packets_by_variant = std::collections::BTreeMap::<String, Value>::new();
    let report_rows = check_packet_variants(
        &mut state,
        source,
        path,
        &manifest,
        "packet_ref",
        "packet_sha256",
        "ToS/contracts/semantic-annotation-packet-v2.schema.json",
        None,
        "semantic",
        |state, source, id, packet_path, packet, _raw, row| {
            let report_index = state.report["variants"].as_array().map_or(0, Vec::len);
            let family = crate::layer_family_rules::inspect_supplied_semantic_annotation(
                packet_path,
                packet,
                limits,
            )?;
            let semantic_errors = semantic_annotation_owner_messages(packet, &family)?;
            for gap in &family.unsupported {
                state.gap(
                    format!("unsupported {} profile at {}", gap.profile, gap.path),
                    limits,
                )?;
            }
            let semantic_valid = semantic_errors.is_empty();
            if !python_optional_equal(
                row.get("expected_semantic_valid"),
                Some(&Value::Bool(semantic_valid)),
            )? {
                state.issue(
                    packet_path,
                    "semantic closure result differs from frozen A/B/C expectation",
                    limits,
                )?;
            }
            if packet
                .pointer("/source_scope/file_sha256")
                .and_then(Value::as_str)
                != Some(input_digest.as_str())
            {
                state.issue(
                    packet_path,
                    "packet source fixity differs from the manifest input",
                    limits,
                )?;
            }
            for anchor in packet
                .pointer("/source_scope/source_anchors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|anchor| anchor.is_object())
            {
                let anchor_ref = anchor
                    .get("anchor_ref")
                    .map(python_value_string)
                    .unwrap_or_else(|| "None".into());
                let selector = anchor.get("selector").unwrap_or(&Value::Null);
                let start = selector.get("start").and_then(python_integer);
                let end = selector.get("end").and_then(python_integer);
                let (Some(start), Some(end)) = (start, end) else {
                    state.issue(
                        packet_path,
                        format!("anchor selector is outside synthetic source: {anchor_ref}"),
                        limits,
                    )?;
                    continue;
                };
                let char_len = source_text.chars().count();
                if start < 0 || start >= end || end > char_len as i128 {
                    state.issue(
                        packet_path,
                        format!("anchor selector is outside synthetic source: {anchor_ref}"),
                        limits,
                    )?;
                    continue;
                }
                let Some(exact) = python_char_slice(&source_text, start as usize, end as usize)
                else {
                    state.issue(
                        packet_path,
                        format!("anchor selector is outside synthetic source: {anchor_ref}"),
                        limits,
                    )?;
                    continue;
                };
                if anchor.get("exact_sha256").and_then(Value::as_str)
                    != Some(Digest256::of_bytes(exact.as_bytes()).to_hex().as_str())
                {
                    state.issue(
                        packet_path,
                        format!("anchor exact digest does not resolve: {anchor_ref}"),
                        limits,
                    )?;
                }
                if anchor.get("source_sha256").and_then(Value::as_str)
                    != Some(input_digest.as_str())
                {
                    state.issue(
                        packet_path,
                        format!("anchor source fixity drifted: {anchor_ref}"),
                        limits,
                    )?;
                }
            }
            state.reserve_decoded(packet, limits)?;
            packets_by_variant.insert(id.to_owned(), packet.clone());
            let mut report =
                semantic_annotation_variant_report(id, packet, semantic_valid, semantic_errors);
            report["schema_valid"] = Value::Null;
            state.reserve_decoded(&report, limits)?;
            Ok(report)
        },
        limits,
    )?;
    state.report["variants"] = Value::Array(report_rows);
    if packets_by_variant
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        == ["A", "B", "C"].into_iter().collect()
    {
        let a = &packets_by_variant["A"];
        let b = &packets_by_variant["B"];
        let c = &packets_by_variant["C"];
        let a_entities = a
            .get("entities")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if a_entities.is_empty()
            || a_entities.iter().any(|entity| {
                entity.get("entity_kind").and_then(Value::as_str) != Some("occurrence")
            })
        {
            state.issue(path, "variant A contains a higher semantic entity", limits)?;
        }
        if a.get("claims")
            .and_then(Value::as_array)
            .is_some_and(|claims| {
                claims
                    .iter()
                    .any(|claim| claim.get("stage").and_then(Value::as_str) != Some("exact_form"))
            })
        {
            state.issue(path, "variant A leaves exact-form observation", limits)?;
        }
        let b_signs: Vec<_> = b
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|entity| entity.get("entity_kind").and_then(Value::as_str) == Some("sign"))
            .collect();
        let b_sign_claims: Vec<_> = b
            .get("claims")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|claim| {
                claim.get("claim_type").and_then(Value::as_str) == Some("sign_identity")
            })
            .collect();
        if b_signs.len() != 2
            || b_signs.iter().any(|entity| {
                entity.get("admission_status").and_then(Value::as_str) != Some("proposed")
            })
        {
            state.issue(
                path,
                "variant B must preserve exactly two proposed sign identities",
                limits,
            )?;
        }
        if b_sign_claims.len() != 2
            || b_sign_claims.iter().any(|claim| {
                claim
                    .get("competing_claim_refs")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len)
                    != 1
            })
        {
            state.issue(
                path,
                "variant B must preserve two reciprocal competing claims",
                limits,
            )?;
        }
        for (packet, variant) in [(a, "A"), (b, "B"), (c, "C")] {
            if packet
                .get("reviews")
                .and_then(Value::as_array)
                .is_some_and(|rows| !rows.is_empty())
                || packet
                    .pointer("/graph_projection/edges")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| !rows.is_empty())
            {
                state.issue(
                    path,
                    format!("variant {variant} fabricates review or graph effect"),
                    limits,
                )?;
            }
        }
        let mut controls = json!({});
        let negative_location = manifest
            .get("variants")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|row| row.get("variant_id").and_then(Value::as_str) == Some("B"))
            .and_then(|row| row.get("packet_ref"))
            .and_then(Value::as_str)
            .unwrap_or(path)
            .to_owned();
        state.reserve_value_clones(b, 12, limits)?;
        // The hand-authored relation and review controls below clone small
        // synthetic fragments in addition to the twelve packet mutations.
        state.reserve(16 * 1024, limits)?;
        let mut control = |name: &str, payload: Value| -> Result<(), ItemRefusal> {
            let semantic = semantic_annotation_owner_messages(
                &payload,
                &crate::layer_family_rules::inspect_supplied_semantic_annotation(
                    &negative_location,
                    &payload,
                    limits,
                )?,
            )?;
            let semantic_rejected = !semantic.is_empty();
            let report_slot = format!("/negative_controls/{name}");
            state.schema_check_with_result(
                &negative_location,
                "ToS/contracts/semantic-annotation-packet-v2.schema.json",
                &payload,
                Some(name),
                None,
                Some(semantic_rejected),
                Some(&report_slot),
                Some(format!("negative control was not rejected: {name}")),
                limits,
            )?;
            controls[name] = json!("pending_schema_diagnostic");
            Ok(())
        };
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/entities/2/entity_id",
            json!("tos.sign.flame"),
        );
        control("label-derived-id", mutated)?;
        let mut mutated = b.clone();
        if let Some(first) = mutated.pointer("/entities/0").cloned() {
            if let Some(entities) = mutated.get_mut("entities").and_then(Value::as_array_mut) {
                entities.push(first);
            }
        }
        control("duplicate-entity-id", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/claims/0/target_anchor_refs", json!([]));
        control("missing-target-anchor", mutated)?;
        let mut mutated = b.clone();
        if let Some(refs) = mutated
            .pointer_mut("/claims/2/proposition/object/entity_refs")
            .and_then(Value::as_array_mut)
        {
            refs.push(json!("tos.sign.sid-ffffffffffffffffffffffffffffffff"));
        }
        control("unresolved-entity-reference", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/claims/2/competing_claim_refs", json!([]));
        control("one-way-competing-claim", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/claims/2/claim_status", json!("accepted"));
        control("accepted-model-claim-without-review", mutated)?;
        let review_id = "tos.review.sid-11111111111111111111111111111111";
        let weak_review = json!({
            "review_id":review_id,
            "reviewer_kind":"human",
            "reviewer_ref":"human:synthetic-negative-control",
            "review_kind":"sign_promotion",
            "competence":[
                {"scope":"source_reading","status":"not_claimed","evidence_refs":[]},
                {"scope":"semantic_interpretation","status":"not_claimed","evidence_refs":[]}
            ],
            "review_mode":"source_visible_unassisted",
            "decision":"accept",
            "rationale":"Synthetic negative control; not evidence of an actual review.",
            "reviewed_at":"2026-08-11T12:00:00Z",
            "unassisted_baseline":{
                "required":true,"status":"frozen","frozen_before_model_suggestions":true,
                "evidence_ref":"synthetic:missing-real-baseline"
            }
        });
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/reviews", json!([weak_review]));
        set_json_pointer(
            &mut mutated,
            "/entities/2/admission_status",
            json!("accepted"),
        );
        set_json_pointer(
            &mut mutated,
            "/entities/2/admission_review_refs",
            json!([review_id]),
        );
        control("sign-promotion-without-baseline-or-competence", mutated)?;
        let relation_claim_id = "tos.claim.sid-22222222222222222222222222222222";
        let relation_id = "tos.relation.sid-33333333333333333333333333333333";
        let subject_ref = b
            .pointer("/entities/2/entity_id")
            .cloned()
            .unwrap_or(Value::Null);
        let object_ref = b
            .pointer("/entities/3/entity_id")
            .cloned()
            .unwrap_or(Value::Null);
        let mut relation_claim = b.pointer("/claims/2").cloned().unwrap_or(Value::Null);
        if let Some(object) = relation_claim.as_object_mut() {
            object.insert("claim_id".into(), json!(relation_claim_id));
            object.insert("claim_type".into(), json!("relation"));
            object.insert("stage".into(), json!("relations_between_signs"));
            object.insert("proposition".into(), json!({"subject_ref":subject_ref,"predicate":"related_to","object":{"kind":"entity_ref","entity_ref":object_ref}}));
            object.insert("claim_status".into(), json!("proposed"));
            object.insert(
                "status_reason".into(),
                json!("Synthetic proposed relation."),
            );
            object.insert("competing_claim_refs".into(), json!([]));
            object.insert("review_refs".into(), json!([]));
        }
        let relation_anchors = relation_claim
            .get("target_anchor_refs")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let accepted_relation = json!({
            "relation_id":relation_id,"relation_type":"related_to","subject_ref":subject_ref,
            "object_ref":object_ref,"claim_ref":relation_claim_id,
            "target_anchor_refs":relation_anchors,"relation_status":"accepted","review_refs":[review_id]
        });
        let mut interpretive_review = weak_review.clone();
        set_json_pointer(
            &mut interpretive_review,
            "/review_kind",
            json!("interpretive"),
        );
        set_json_pointer(
            &mut interpretive_review,
            "/review_mode",
            json!("evidence_packet_review"),
        );
        set_json_pointer(
            &mut interpretive_review,
            "/competence",
            json!([
                {"scope":"semantic_interpretation","status":"self_attested","evidence_refs":[]}
            ]),
        );
        set_json_pointer(
            &mut interpretive_review,
            "/unassisted_baseline",
            json!({
                "required":false,"status":"not_applicable","frozen_before_model_suggestions":false,"evidence_ref":null
            }),
        );
        let mut mutated = b.clone();
        if let Some(claims) = mutated.get_mut("claims").and_then(Value::as_array_mut) {
            claims.push(relation_claim.clone());
        }
        set_json_pointer(&mut mutated, "/relations", json!([accepted_relation]));
        set_json_pointer(&mut mutated, "/reviews", json!([interpretive_review]));
        control("accepted-relation-without-accepted-claim", mutated)?;
        let mut proposed_relation = accepted_relation.clone();
        set_json_pointer(
            &mut proposed_relation,
            "/relation_status",
            json!("proposed"),
        );
        set_json_pointer(&mut proposed_relation, "/review_refs", json!([]));
        let mut mutated = b.clone();
        if let Some(claims) = mutated.get_mut("claims").and_then(Value::as_array_mut) {
            claims.push(relation_claim.clone());
        }
        set_json_pointer(
            &mut mutated,
            "/relations",
            json!([proposed_relation.clone()]),
        );
        if let Some(graph) = mutated
            .get_mut("graph_projection")
            .and_then(Value::as_object_mut)
        {
            graph.insert(
                "projection_event_refs".into(),
                json!(["synthetic:negative-control"]),
            );
            graph.insert("node_refs".into(), json!([subject_ref, object_ref]));
            graph.insert(
                "edges".into(),
                json!([{"relation_ref":relation_id,"claim_ref":relation_claim_id,"subject_ref":subject_ref,"object_ref":object_ref,"source_return_anchor_refs":relation_anchors}]),
            );
        }
        control("graph-projection-of-proposed-claim", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut relation_claim,
            "/proposition/predicate",
            json!("contrasts_with"),
        );
        if let Some(claims) = mutated.get_mut("claims").and_then(Value::as_array_mut) {
            claims.push(relation_claim);
        }
        set_json_pointer(&mut mutated, "/relations", json!([proposed_relation]));
        control("relation-proposition-mismatch", mutated)?;
        let mut mutated = b.clone();
        let annotation_id = mutated.get("annotation_id").cloned().unwrap_or(Value::Null);
        set_json_pointer(&mut mutated, "/supersedes_annotation_ref", annotation_id);
        control("self-supersession", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/source_content_visibility",
            json!("local_only"),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/record_visibility",
            json!("public_metadata_only"),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/publication_authorized",
            json!(true),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/private_source_used",
            json!(true),
        );
        control("publication-boundary-widening", mutated)?;
        drop(control);
        state.reserve_decoded(&controls, limits)?;
        state.report["negative_controls"] = controls;
    }
    let reported_controls: std::collections::BTreeSet<String> = state.report["negative_controls"]
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    if expected_controls != reported_controls {
        state.issue(
            path,
            "negative-control coverage differs from the plan",
            limits,
        )?;
    }
    result(
        SourceFoundationLab::SemanticAnnotationV2,
        state,
        Vec::new(),
        limits,
    )
}

fn inspect_translation_alignment_v1(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let path = "ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/lab.manifest.json";
    let limit_value = json!({"private_source_used":false,"translation_performed":false,"model_invoked":false,"aligner_invoked":false,"human_review_performed":false,"accepted_alignment_established":false,"lexical_equivalence_established":false,"semantic_truth_established":false,"graph_truth_established":false,"canon_effect":false});
    let mut state = DirectState {
        report: json!({"variants":[],"negative_controls":{}}),
        ..Default::default()
    };
    let Some(manifest) = manifest(
        &mut state,
        source,
        path,
        "tos_translation_alignment_v1_lab_manifest_v1",
        "public_synthetic_contract_mechanics_only",
        &limit_value,
        limits,
    )?
    else {
        return result(
            SourceFoundationLab::TranslationAlignmentV1,
            state,
            Vec::new(),
            limits,
        );
    };
    manifest_authority(
        &mut state,
        path,
        &manifest,
        "tos_translation_alignment_v1_lab_manifest_v1",
        "public_synthetic_contract_mechanics_only",
        &limit_value,
        limits,
    )?;
    let mut plan_ref = None;
    for field in ["contract", "research", "plan", "builder"] {
        let bound = binding(&mut state, source, path, &manifest, field, false, limits)?;
        if field == "plan" {
            plan_ref = bound;
        }
    }
    let mut input_paths = Vec::<String>::new();
    for (field, label) in [
        ("inputs", "input"),
        ("analysis_artifacts", "analysis_artifact"),
    ] {
        if let Some(bindings) = manifest.get(field).and_then(Value::as_array) {
            for (index, binding_value) in bindings.iter().enumerate() {
                let Some(ref_path) = binding_value.get("ref").and_then(Value::as_str) else {
                    state.issue(path, format!("{label}[{index}] binding is absent"), limits)?;
                    continue;
                };
                let Some(expected_digest) = binding_value.get("sha256").and_then(Value::as_str)
                else {
                    state.issue(path, format!("{label}[{index}] binding is absent"), limits)?;
                    if field == "inputs" {
                        state.reserve(ref_path.len(), limits)?;
                        input_paths.push(ref_path.to_owned());
                    }
                    continue;
                };
                let raw = state.read(source, ref_path, limits)?;
                if raw
                    .as_ref()
                    .is_none_or(|raw| Digest256::of_bytes(raw).to_hex() != expected_digest)
                {
                    state.issue(path, format!("{label}[{index}] binding drifted"), limits)?;
                }
                if field == "inputs" {
                    state.reserve(ref_path.len(), limits)?;
                    input_paths.push(ref_path.to_owned());
                }
            }
        }
    }
    if manifest.pointer("/contract/ref").and_then(Value::as_str)
        != Some("ToS/contracts/translation-alignment-packet-v1.schema.json")
    {
        state.issue(
            path,
            "translation alignment contract reference drifted",
            limits,
        )?;
    }
    let plan_path = plan_ref.as_deref().unwrap_or(path);
    let plan = match state.read_json(source, plan_path, limits)? {
        Some((value, _)) if value.is_object() => value,
        Some(_) => {
            state.issue(plan_path, "JSON root must be an object", limits)?;
            Value::Null
        }
        None => Value::Null,
    };
    if !python_optional_equal(plan.get("authority_limits"), Some(&limit_value))? {
        state.issue(
            path,
            "translation alignment plan authority limits drifted",
            limits,
        )?;
    }
    let expected_controls: std::collections::BTreeSet<String> = plan
        .get("negative_controls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let mut input_texts = std::collections::BTreeMap::<String, String>::new();
    for ref_path in input_paths {
        let raw = state.read(source, &ref_path, limits)?;
        match raw.as_deref().map(std::str::from_utf8) {
            Some(Ok(text)) => {
                state.reserve(text.len(), limits)?;
                input_texts.insert(ref_path, text.to_owned());
            }
            Some(Err(_)) => state.issue(
                path,
                "cannot read public synthetic alignment input: invalid UTF-8",
                limits,
            )?,
            None => state.issue(
                path,
                "cannot read public synthetic alignment input: file is missing",
                limits,
            )?,
        }
    }
    let mut packets_by_variant = std::collections::BTreeMap::<String, Value>::new();
    let report_rows = check_packet_variants(
        &mut state,
        source,
        path,
        &manifest,
        "packet_ref",
        "packet_sha256",
        "ToS/contracts/translation-alignment-packet-v1.schema.json",
        None,
        "translation alignment",
        |state, source, id, packet_path, packet, _raw, row| {
            let family =
                crate::layer_family_rules::inspect_supplied_translation_alignment(packet, limits)?;
            let semantic_errors = translation_alignment_owner_messages(packet, &family);
            let semantic_valid = semantic_errors.is_empty();
            if !python_optional_equal(
                row.get("expected_semantic_valid"),
                Some(&Value::Bool(semantic_valid)),
            )? {
                state.issue(
                    packet_path,
                    "semantic result differs from frozen A/B/C expectation",
                    limits,
                )?;
            }
            for gap in &family.unsupported {
                state.gap(
                    format!("unsupported {} profile at {}", gap.profile, gap.path),
                    limits,
                )?;
            }
            for side_name in ["source_side", "target_side"] {
                let side = packet.get(side_name).unwrap_or(&Value::Null);
                if !side.is_object() {
                    continue;
                }
                let text_ref = side.get("text_layer_ref").and_then(Value::as_str);
                let Some(text) = text_ref.and_then(|reference| input_texts.get(reference)) else {
                    state.issue(
                        packet_path,
                        format!("{side_name} does not resolve to a manifest input"),
                        limits,
                    )?;
                    continue;
                };
                let text_digest = Digest256::of_bytes(text.as_bytes()).to_hex();
                if side.get("file_sha256").and_then(Value::as_str) != Some(text_digest.as_str()) {
                    state.issue(
                        packet_path,
                        format!("{side_name} file fixity differs from its input"),
                        limits,
                    )?;
                }
                if side.get("text_layer_sha256").and_then(Value::as_str)
                    != Some(text_digest.as_str())
                {
                    state.issue(
                        packet_path,
                        format!("{side_name} text-layer fixity differs from its input"),
                        limits,
                    )?;
                }
                for analysis_field in ["segmentation", "tokenization"] {
                    let binding = side.get(analysis_field).unwrap_or(&Value::Null);
                    if !binding.is_object() {
                        continue;
                    }
                    let artifact_ref = binding
                        .get("artifact_ref")
                        .and_then(Value::as_str)
                        .unwrap_or(packet_path);
                    let expected = binding.get("sha256").and_then(Value::as_str);
                    let raw = state.read(source, artifact_ref, limits)?;
                    if raw.as_ref().is_none_or(|raw| {
                        Some(Digest256::of_bytes(raw).to_hex().as_str()) != expected
                    }) {
                        state.issue(
                            packet_path,
                            format!("{side_name} {analysis_field} binding drifted"),
                            limits,
                        )?;
                    }
                }
                for anchor in side
                    .get("anchors")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|anchor| anchor.is_object())
                {
                    let anchor_ref = anchor
                        .get("anchor_ref")
                        .map(python_value_string)
                        .unwrap_or_else(|| "None".into());
                    let selector = anchor.get("selector").unwrap_or(&Value::Null);
                    let start = selector.get("start").and_then(python_integer);
                    let end = selector.get("end").and_then(python_integer);
                    let Some((start, end)) = start.zip(end) else {
                        state.issue(
                            packet_path,
                            format!(
                                "anchor selector is outside synthetic {side_name}: {anchor_ref}"
                            ),
                            limits,
                        )?;
                        continue;
                    };
                    let char_len = text.chars().count();
                    if start < 0 || start >= end || end > char_len as i128 {
                        state.issue(
                            packet_path,
                            format!(
                                "anchor selector is outside synthetic {side_name}: {anchor_ref}"
                            ),
                            limits,
                        )?;
                        continue;
                    }
                    let Some(exact) = python_char_slice(text, start as usize, end as usize) else {
                        state.issue(
                            packet_path,
                            format!(
                                "anchor selector is outside synthetic {side_name}: {anchor_ref}"
                            ),
                            limits,
                        )?;
                        continue;
                    };
                    if anchor.get("exact_sha256").and_then(Value::as_str)
                        != Some(Digest256::of_bytes(exact.as_bytes()).to_hex().as_str())
                    {
                        state.issue(
                            packet_path,
                            format!("anchor exact digest does not resolve: {anchor_ref}"),
                            limits,
                        )?;
                    }
                }
            }
            state.reserve_decoded(packet, limits)?;
            packets_by_variant.insert(id.to_owned(), packet.clone());
            let mut reasons = semantic_errors;
            reasons.sort();
            reasons.dedup();
            let report = json!({
                "variant_id":id,
                "schema_valid":null,
                "semantic_valid":semantic_valid,
                "shapes":packet.get("alignments").and_then(Value::as_array).into_iter().flatten().map(|item|item.get("correspondence_shape").cloned().unwrap_or(Value::Null)).collect::<Vec<_>>(),
                "statuses":packet.get("alignments").and_then(Value::as_array).into_iter().flatten().map(|item|item.get("status").cloned().unwrap_or(Value::Null)).collect::<Vec<_>>(),
                "review_count":packet.get("reviews").and_then(Value::as_array).map_or(0,Vec::len),
                "projection_count":packet.get("projections").and_then(Value::as_array).map_or(0,Vec::len),
                "rejection_reasons":reasons,
            });
            state.reserve_decoded(&report, limits)?;
            Ok(report)
        },
        limits,
    )?;
    state.report["variants"] = Value::Array(report_rows);
    if packets_by_variant
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        == ["A", "B", "C"].into_iter().collect()
    {
        let a = &packets_by_variant["A"];
        let b = &packets_by_variant["B"];
        let c = &packets_by_variant["C"];
        let shapes = |packet: &Value| -> Vec<Value> {
            packet
                .get("alignments")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|row| {
                    row.get("correspondence_shape")
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .collect()
        };
        if shapes(a) != vec![json!("one_to_one")] {
            state.issue(
                path,
                "variant A must contain exactly one one-to-one proposal",
                limits,
            )?;
        }
        let b_alignments = b
            .get("alignments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let b_shapes = shapes(b);
        if b_shapes != vec![json!("one_to_one"), json!("one_to_many")]
            || b_alignments
                .iter()
                .any(|row| row.get("status").and_then(Value::as_str) != Some("proposed"))
        {
            state.issue(
                path,
                "variant B must preserve the two proposed competing shapes",
                limits,
            )?;
        }
        for (packet, variant) in [(a, "A"), (b, "B"), (c, "C")] {
            if packet
                .get("reviews")
                .and_then(Value::as_array)
                .is_some_and(|rows| !rows.is_empty())
                || packet
                    .get("projections")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| !rows.is_empty())
            {
                state.issue(
                    path,
                    format!("variant {variant} fabricates review or projection evidence"),
                    limits,
                )?;
            }
        }
        let mut controls = json!({});
        let negative_location = manifest
            .get("variants")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|row| row.get("variant_id").and_then(Value::as_str) == Some("B"))
            .and_then(|row| row.get("packet_ref"))
            .and_then(Value::as_str)
            .unwrap_or(path)
            .to_owned();
        state.reserve_value_clones(b, 17, limits)?;
        // One synthetic projection clone is retained while its paired
        // negative control is scheduled.
        state.reserve(8 * 1024, limits)?;
        let mut control = |name: &str, payload: Value| -> Result<(), ItemRefusal> {
            let family = crate::layer_family_rules::inspect_supplied_translation_alignment(
                &payload, limits,
            )?;
            let semantic_rejected =
                !translation_alignment_owner_messages(&payload, &family).is_empty();
            let report_slot = format!("/negative_controls/{name}");
            state.schema_check_with_result(
                &negative_location,
                "ToS/contracts/translation-alignment-packet-v1.schema.json",
                &payload,
                Some(name),
                None,
                Some(semantic_rejected),
                Some(&report_slot),
                Some(format!("negative control was not rejected: {name}")),
                limits,
            )?;
            controls[name] = json!("pending_schema_diagnostic");
            Ok(())
        };
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/0/identity_policy",
            json!("text-derived-from-current-source-and-target-content"),
        );
        control("text-derived-alignment-id", mutated)?;
        let mut mutated = b.clone();
        if let Some(first) = mutated.pointer("/alignments/0").cloned() {
            if let Some(rows) = mutated.get_mut("alignments").and_then(Value::as_array_mut) {
                rows.push(first);
            }
        }
        control("duplicate-alignment-id", mutated)?;
        let mut mutated = b.clone();
        if let (Some(first), Some(second)) = (
            mutated.pointer("/alignments/0/claim_id").cloned(),
            mutated.pointer("/alignments/1/claim_id").cloned(),
        ) {
            set_json_pointer(&mut mutated, "/alignments/1/claim_id", first);
            let _ = second;
        }
        control("duplicate-claim-id", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/0/ordered_source_anchor_refs",
            json!([]),
        );
        control("missing-source-anchor", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/0/ordered_target_anchor_refs",
            json!([]),
        );
        control("missing-target-anchor", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/source_side/text_layer_sha256",
            json!("0".repeat(64)),
        );
        control("source-layer-digest-drift", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/target_side/text_layer_sha256",
            json!("1".repeat(64)),
        );
        control("target-layer-digest-drift", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/0/ordered_source_anchor_refs",
            json!(["tos.anchor.translation-alignment-v1.synthetic-naked-offset"]),
        );
        control("naked-unresolved-anchor", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/1/correspondence_shape",
            json!("one_to_one"),
        );
        control("shape-cardinality-mismatch", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/0/correspondence_shape",
            json!("source_omission"),
        );
        set_json_pointer(
            &mut mutated,
            "/alignments/0/order_posture",
            json!("not_applicable"),
        );
        control("source-omission-with-target-members", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/0/correspondence_shape",
            json!("target_addition"),
        );
        set_json_pointer(
            &mut mutated,
            "/alignments/0/order_posture",
            json!("not_applicable"),
        );
        control("target-addition-with-source-members", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/alignments/0/competing_alignment_refs",
            json!([]),
        );
        control("one-way-competing-mapping", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/alignments/0/status", json!("accepted"));
        control("accepted-model-proposal-without-review", mutated)?;
        let weak_review_id =
            "tos.translation-alignment-review.sid-11111111111111111111111111111111";
        let weak_review = json!({
            "review_id":weak_review_id,
            "reviewer_kind":"real_human",
            "reviewer_ref":"human:synthetic-negative-control",
            "reviewed_alignment_refs":[b.pointer("/alignments/0/alignment_id").cloned().unwrap_or(Value::Null)],
            "review_kind":"translation_alignment_adjudication",
            "competence":[
                {"scope":"source_language_reading","status":"not_claimed","evidence_refs":[]},
                {"scope":"target_language_reading","status":"not_claimed","evidence_refs":[]},
                {"scope":"translation_analysis","status":"not_claimed","evidence_refs":[]}
            ],
            "source_visible":true,"target_visible":true,"decision":"accept",
            "rationale":"Synthetic negative control; not evidence of a real review.",
            "reviewed_at":"2026-08-11T14:00:00Z",
            "review_provenance_event_ref":"synthetic:negative-control",
            "unassisted_baseline":{"required":true,"status":"frozen","frozen_before_machine_or_model_suggestions":true,"evidence_ref":"synthetic:negative-control-baseline"}
        });
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(&mut mutated, "/reviews", json!([weak_review]));
        set_json_pointer(&mut mutated, "/alignments/0/status", json!("accepted"));
        set_json_pointer(
            &mut mutated,
            "/alignments/0/review_refs",
            json!([weak_review_id]),
        );
        control("accepted-without-language-competence", mutated)?;
        let projection = json!({
            "projection_id":"tos.translation-alignment-projection.sid-22222222222222222222222222222222",
            "projection_kind":"tei_linking","artifact_ref":"synthetic:negative-control-projection",
            "artifact_sha256":"2".repeat(64),
            "source_alignment_refs":[b.pointer("/alignments/0/alignment_id").cloned().unwrap_or(Value::Null)],
            "projection_event_ref":"synthetic:negative-control","derived_only":true,
            "source_return_required":true,
            "authority_posture":"non_authoritative_reproducible_projection","effective_visibility":"public"
        });
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(&mut mutated, "/projections", json!([projection]));
        control("projection-of-proposed-alignment", mutated)?;
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(&mut mutated, "/source_side/visibility", json!("local_only"));
        set_json_pointer(
            &mut mutated,
            "/source_side/publication_authorized",
            json!(false),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/source_visibility",
            json!("local_only"),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/effective_visibility",
            json!("local_only"),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/publication_authorized",
            json!(false),
        );
        set_json_pointer(&mut mutated, "/projections", json!([projection]));
        control("projection-visibility-widening", mutated)?;
        let mut mutated = b.clone();
        let packet_id = mutated.get("packet_id").cloned().unwrap_or(Value::Null);
        set_json_pointer(&mut mutated, "/supersedes_packet_ref", packet_id);
        control("self-supersession", mutated)?;
        drop(control);
        state.reserve_decoded(&controls, limits)?;
        state.report["negative_controls"] = controls;
    }
    let reported_controls: std::collections::BTreeSet<String> = state.report["negative_controls"]
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    if expected_controls != reported_controls {
        state.issue(
            path,
            "negative-control coverage differs from the plan",
            limits,
        )?;
    }
    result(
        SourceFoundationLab::TranslationAlignmentV1,
        state,
        Vec::new(),
        limits,
    )
}

fn translation_alignment_owner_messages(packet: &Value, family: &LayerFamilyReport) -> Vec<String> {
    let mut messages = Vec::new();
    let mut duplicate_ordinal_seen = std::collections::BTreeMap::<String, usize>::new();
    for issue in &family.issues {
        let message = match issue.code {
            "duplicate-local-identity" => {
                let (label, identity) = issue.subject.split_once(':').unwrap_or(("", ""));
                let label = match label {
                    "alignment" => "alignment",
                    "alignment-claim" => "alignment claim",
                    "alignment-review" => "alignment review",
                    "alignment-projection" => "alignment projection",
                    "source-anchor" => "source anchor",
                    "target-anchor" => "target anchor",
                    _ => label,
                };
                format!("duplicate {label} identity: {identity}")
            }
            "alignment-duplicate-anchor-ordinal" => {
                let label = issue.subject.as_str();
                let side_name = if label == "source" {
                    "source_side"
                } else {
                    "target_side"
                };
                let mut seen = std::collections::BTreeSet::<i128>::new();
                let mut duplicate_values = Vec::<String>::new();
                for anchor in packet
                    .pointer(&format!("/{side_name}/anchors"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|anchor| anchor.is_object())
                {
                    if let Some(ordinal) = anchor.get("ordinal").and_then(python_integer) {
                        if !seen.insert(ordinal) {
                            duplicate_values.push(ordinal.to_string());
                        }
                    }
                }
                let index = duplicate_ordinal_seen.entry(label.to_owned()).or_default();
                let value = duplicate_values.get(*index).cloned().unwrap_or_default();
                *index += 1;
                format!("duplicate {label} anchor ordinal: {value}")
            }
            "alignment-anchor-layer-escape" => {
                format!(
                    "{} anchor escapes the frozen text layer: {}",
                    translation_anchor_side(packet, &issue.subject),
                    issue.subject
                )
            }
            "alignment-anchor-layer-digest-drift" => {
                let label = translation_anchor_side(packet, &issue.subject);
                format!(
                    "{label} anchor text-layer digest drifted: {}",
                    issue.subject
                )
            }
            "alignment-anchor-selector-reversed" => {
                let label = translation_anchor_side(packet, &issue.subject);
                format!(
                    "{label} anchor selector is reversed or empty: {}",
                    issue.subject
                )
            }
            "alignment-shared-side-anchor-identity" => {
                format!(
                    "source and target sides reuse one anchor identity: {}",
                    issue.subject
                )
            }
            "alignment-packet-self-supersession" => {
                "translation alignment packet cannot supersede itself".into()
            }
            "alignment-identical-expressions" => {
                "translation alignment source and target expressions must differ".into()
            }
            "alignment-analysis-not-frozen" => {
                format!("{} analysis binding is not frozen", issue.subject)
            }
            "alignment-review-competence-scope-drift" => {
                format!(
                    "alignment review competence scopes are incomplete or duplicated: {}",
                    issue.subject
                )
            }
            "unresolved-local-reference" => {
                let (label, reference) = issue.subject.split_once(':').unwrap_or(("", ""));
                let label = match label {
                    "reviewed-alignment" => "reviewed alignment",
                    "alignment-source-anchor" => "source anchor",
                    "alignment-target-anchor" => "target anchor",
                    "alignment-review" => "alignment review",
                    "competing-alignment" => "competing alignment",
                    "alignment-evidence-source" => "evidence source anchor",
                    "alignment-evidence-target" => "evidence target anchor",
                    "projected-alignment" => "projected alignment",
                    _ => label,
                };
                format!("unresolved {label} reference: {reference}")
            }
            "review-alignment-not-reciprocal" => {
                let left = issue
                    .subject
                    .split_once("->")
                    .map(|(left, _)| left)
                    .unwrap_or("");
                let review_ids = packet
                    .get("reviews")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|row| row.get("review_id").and_then(Value::as_str))
                    .collect::<std::collections::BTreeSet<_>>();
                let prose = if review_ids.contains(left) {
                    "review-to-alignment relation"
                } else {
                    "alignment-to-review relation"
                };
                format!(
                    "{prose} is not reciprocal: {}",
                    issue.subject.replace("->", " -> ")
                )
            }
            "alignment-self-supersession" => {
                format!("alignment cannot supersede itself: {}", issue.subject)
            }
            "alignment-claim-self-supersession" => {
                format!("alignment claim cannot supersede itself: {}", issue.subject)
            }
            "alignment-shape-cardinality-drift" => {
                let alignment_id = &issue.subject;
                let shape = packet
                    .get("alignments")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|row| {
                        row.get("alignment_id").and_then(Value::as_str) == Some(alignment_id)
                    })
                    .and_then(|row| row.get("correspondence_shape"))
                    .map(python_value_string)
                    .unwrap_or_else(|| "None".into());
                format!("alignment member cardinality does not match {shape}: {alignment_id}")
            }
            "alignment-unaligned-order-posture" => {
                format!(
                    "unaligned member uses an inapplicable order posture: {}",
                    issue.subject
                )
            }
            "alignment-mixed-unresolved-techniques" => {
                format!(
                    "unresolved translation technique is mixed with decided techniques: {}",
                    issue.subject
                )
            }
            "alignment-self-competition" => {
                format!("alignment competes with itself: {}", issue.subject)
            }
            "alignment-competition-not-reciprocal" => {
                format!(
                    "competing-alignment relation is not reciprocal: {}",
                    issue.subject.replace("->", " -> ")
                )
            }
            "decided-alignment-review-absent" => {
                format!("decided alignment lacks review: {}", issue.subject)
            }
            "alignment-direct-machine-acceptance" => {
                format!(
                    "machine, imported, or synthetic alignment cannot be accepted directly: {}",
                    issue.subject
                )
            }
            "accepted-alignment-matching-decision-absent" => {
                format!(
                    "accepted alignment lacks a matching real-human decision: {}",
                    issue.subject
                )
            }
            "accepted-alignment-review-competence-absent" => {
                let (review_id, scope) = issue.subject.split_once('/').unwrap_or(("", ""));
                format!(
                    "accepted alignment review lacks declared competence for {scope}: {review_id}"
                )
            }
            "alignment-synthetic-maker-outside-lab" => {
                format!(
                    "synthetic fixture maker escaped the synthetic laboratory: {}",
                    issue.subject
                )
            }
            "alignment-source-evidence-incomplete" => {
                format!(
                    "alignment evidence does not cover every source member: {}",
                    issue.subject
                )
            }
            "alignment-target-evidence-incomplete" => {
                format!(
                    "alignment evidence does not cover every target member: {}",
                    issue.subject
                )
            }
            "alignment-lineage-cycle" => {
                format!("alignment supersession cycle reaches: {}", issue.subject)
            }
            "alignment-lineage-predecessor-unresolved" => {
                format!(
                    "unresolved superseded alignment reference: {}",
                    issue.subject
                )
            }
            "alignment-side-visibility-drift" => {
                format!(
                    "{}-side and packet visibility differ",
                    issue
                        .subject
                        .strip_suffix("_visibility")
                        .unwrap_or(&issue.subject)
                )
            }
            "alignment-effective-visibility-drift" => {
                "effective visibility is not the most restrictive component".into()
            }
            "alignment-publication-boundary-widened" => {
                "alignment publication authority widens a source or target boundary".into()
            }
            "projection-alignment-not-accepted" => {
                let (projection, alignment) = issue.subject.split_once("->").unwrap_or(("", ""));
                format!(
                    "projection uses an alignment that is not accepted: {projection} -> {alignment}"
                )
            }
            "alignment-projection-visibility-widened" => {
                format!(
                    "projection visibility widens packet boundary: {}",
                    issue.subject
                )
            }
            _ => format!("{}: {}", issue.code, issue.subject),
        };
        messages.push(message);
    }
    messages
}

fn translation_anchor_side<'a>(packet: &'a Value, anchor_ref: &str) -> &'static str {
    if packet
        .pointer("/source_side/anchors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|row| row.get("anchor_ref").and_then(Value::as_str) == Some(anchor_ref))
    {
        "source"
    } else {
        "target"
    }
}

fn semantic_annotation_variant_report(
    id: &str,
    packet: &Value,
    semantic_valid: bool,
    mut rejection_reasons: Vec<String>,
) -> Value {
    rejection_reasons.sort();
    rejection_reasons.dedup();
    let values = |field: &str, key: &str| -> Vec<Value> {
        packet
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|row| row.is_object())
            .map(|row| row.get(key).cloned().unwrap_or(Value::Null))
            .collect()
    };
    json!({
        "variant_id":id,
        "schema_valid":null,
        "semantic_valid":semantic_valid,
        "entity_kinds":values("entities","entity_kind"),
        "claim_statuses":values("claims","claim_status"),
        "review_count":packet.get("reviews").and_then(Value::as_array).map_or(0,Vec::len),
        "graph_edge_count":packet.pointer("/graph_projection/edges").and_then(Value::as_array).map_or(0,Vec::len),
        "rejection_reasons":rejection_reasons,
    })
}

fn semantic_annotation_owner_messages(
    packet: &Value,
    family: &LayerFamilyReport,
) -> Result<Vec<String>, ItemRefusal> {
    let mut messages = Vec::new();
    for issue in &family.issues {
        if issue.code == "semantic-publication-boundary-widened" {
            let rights = packet.get("rights_and_visibility").unwrap_or(&Value::Null);
            if rights.get("private_source_used").and_then(Value::as_bool) == Some(true)
                && rights
                    .get("publication_authorized")
                    .and_then(Value::as_bool)
                    == Some(true)
            {
                messages.push("private source use cannot widen publication authority".into());
            }
            if rights
                .get("source_content_visibility")
                .and_then(Value::as_str)
                .is_some_and(|value| matches!(value, "local_only" | "restricted" | "unknown"))
                && rights
                    .get("publication_authorized")
                    .and_then(Value::as_bool)
                    == Some(true)
            {
                messages.push(
                    "non-public or unknown source visibility cannot authorize publication".into(),
                );
            }
            continue;
        }
        let message = match issue.code {
            "duplicate-local-identity" => {
                let (label, identity) = issue.subject.split_once(':').unwrap_or(("", ""));
                let label = match label {
                    "entity" => "entity",
                    "claim" => "claim",
                    "relation" => "relation",
                    "review" => "review",
                    "anchor" => "source anchor",
                    _ => label,
                };
                format!("duplicate {label} identity: {identity}")
            }
            "unresolved-local-reference" => {
                let (label, reference) = issue.subject.split_once(':').unwrap_or(("", ""));
                let label = match label {
                    "entity-anchor" => "entity anchor",
                    "entity-claim" => "entity claim",
                    "entity-parent" => "parent entity",
                    "entity-review" => "entity review",
                    "claim-subject" => "claim subject",
                    "claim-object" => "claim object entity",
                    "claim-anchor" => "claim target anchor",
                    "evidence-anchor" => "evidence anchor",
                    "competing-claim" => "competing claim",
                    "claim-review" => "claim review",
                    "relation-endpoint" => "relation endpoint",
                    "relation-claim" => "relation claim",
                    "relation-anchor" => "relation target anchor",
                    "relation-review" => "relation review",
                    "graph-node" => "graph node",
                    "graph-relation" => "graph relation",
                    "graph-claim" => "graph claim",
                    "graph-endpoint" => "graph endpoint",
                    "graph-source-return-anchor" => "graph source-return anchor",
                    _ => label,
                };
                if label == "claim subject" {
                    format!("claim subject is unresolved: {reference}")
                } else {
                    format!("unresolved {label} reference: {reference}")
                }
            }
            "annotation-self-supersession" => {
                "semantic annotation packet cannot supersede itself".into()
            }
            "entity-kind-namespace-drift" => {
                format!(
                    "entity identity namespace does not match entity kind: {}",
                    issue.subject
                )
            }
            "entity-self-supersession" => {
                format!("entity cannot supersede itself: {}", issue.subject)
            }
            "decided-entity-review-absent" => {
                format!("decided entity lacks review: {}", issue.subject)
            }
            "accepted-entity-review-kind-absent" => {
                let kind = packet
                    .get("entities")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|entity| {
                        entity.get("entity_id").and_then(Value::as_str) == Some(&issue.subject)
                    })
                    .and_then(|entity| entity.get("entity_kind"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if kind == "sign" {
                    format!(
                        "accepted sign lacks an accepting sign-promotion review: {}",
                        issue.subject
                    )
                } else {
                    format!(
                        "accepted concept lacks an accepting interpretive review: {}",
                        issue.subject
                    )
                }
            }
            "sign-baseline-not-frozen" => {
                format!(
                    "sign-promotion review lacks a frozen unassisted baseline: {}",
                    issue.subject
                )
            }
            "entity-review-competence-absent" => {
                let (review_id, scope) = issue.subject.split_once('/').unwrap_or(("", ""));
                let kind = packet
                    .get("entities")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|entity| {
                        entity
                            .get("admission_status")
                            .and_then(Value::as_str)
                            .is_some_and(|status| {
                                matches!(status, "accepted" | "accepted_with_limits")
                            })
                            && entity
                                .get("admission_review_refs")
                                .and_then(Value::as_array)
                                .is_some_and(|refs| {
                                    refs.iter()
                                        .any(|reference| reference.as_str() == Some(review_id))
                                })
                    })
                    .and_then(|entity| entity.get("entity_kind"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if kind == "sign" {
                    format!(
                        "sign-promotion review lacks declared competence for {scope}: {review_id}"
                    )
                } else {
                    format!("concept review lacks declared semantic competence: {review_id}")
                }
            }
            "semantic-claim-self-supersession" => {
                format!("claim cannot supersede itself: {}", issue.subject)
            }
            "semantic-claim-self-competition" => {
                format!("claim competes with itself: {}", issue.subject)
            }
            "semantic-claim-competition-not-reciprocal" => {
                format!(
                    "competing-claim relation is not reciprocal: {}",
                    issue.subject.replace("->", " -> ")
                )
            }
            "decided-semantic-claim-review-absent" => {
                format!("decided claim lacks review: {}", issue.subject)
            }
            "accepted-semantic-claim-review-absent" => {
                format!(
                    "accepted claim lacks an accepting real-human review: {}",
                    issue.subject
                )
            }
            "accepted-synthetic-semantic-claim" => {
                format!(
                    "synthetic fixture cannot establish an accepted claim: {}",
                    issue.subject
                )
            }
            "semantic-synthetic-maker-outside-lab" => {
                format!(
                    "synthetic fixture maker escaped the synthetic laboratory: {}",
                    issue.subject
                )
            }
            "semantic-relation-collapsed-endpoints" => {
                format!(
                    "relation cannot collapse subject and object: {}",
                    issue.subject
                )
            }
            "relation-supporting-proposition-drift" => {
                format!(
                    "relation and supporting claim proposition differ: {}",
                    issue.subject
                )
            }
            "accepted-relation-supporting-claim-not-accepted" => {
                format!(
                    "accepted relation lacks an accepted supporting claim: {}",
                    issue.subject
                )
            }
            "accepted-semantic-relation-review-absent" => {
                format!(
                    "accepted relation lacks an accepting real-human review: {}",
                    issue.subject
                )
            }
            "graph-semantic-relation-not-accepted" => {
                format!(
                    "graph edge projects a relation that is not accepted: {}",
                    issue.subject
                )
            }
            "graph-semantic-claim-not-accepted" => {
                format!(
                    "graph edge projects a claim that is not accepted: {}",
                    issue.subject
                )
            }
            "graph-semantic-relation-binding-drift" => {
                format!(
                    "graph edge differs from its admitted relation: {}",
                    issue.subject
                )
            }
            _ => format!("{}: {}", issue.code, issue.subject),
        };
        messages.push(message);
    }
    let estimated = messages.iter().try_fold(0usize, |total, message| {
        total.checked_add(message.len()).ok_or(ItemRefusal::Budget)
    })?;
    if estimated > usize::MAX / 2 {
        return Err(ItemRefusal::Budget);
    }
    Ok(messages)
}

fn python_char_slice(text: &str, start: usize, end: usize) -> Option<&str> {
    let byte_start = text.char_indices().nth(start).map_or_else(
        || (start == text.chars().count()).then_some(text.len()),
        |(offset, _)| Some(offset),
    )?;
    let byte_end = text.char_indices().nth(end).map_or_else(
        || (end == text.chars().count()).then_some(text.len()),
        |(offset, _)| Some(offset),
    )?;
    text.get(byte_start..byte_end)
}

fn python_value_string(value: &Value) -> String {
    match value {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn inspect_source_text_unit_v1(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let path = "ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/lab.manifest.json";
    let limit_value = json!({"private_source_used":false,"real_language_boundary_accepted":false,"model_invoked":false,"human_review_performed":false,"linguistic_word_established":false,"lexeme_established":false,"semantic_truth_established":false,"graph_truth_established":false,"legacy_migration_performed":false,"canon_effect":false});
    let mut state = DirectState {
        report: json!({"variants":[],"negative_controls":{}}),
        ..Default::default()
    };
    let Some(manifest) = manifest(
        &mut state,
        source,
        path,
        "tos_source_text_unit_v1_lab_manifest_v1",
        "public_synthetic_contract_mechanics_only",
        &limit_value,
        limits,
    )?
    else {
        return result(
            SourceFoundationLab::SourceTextUnitV1,
            state,
            Vec::new(),
            limits,
        );
    };
    manifest_authority(
        &mut state,
        path,
        &manifest,
        "tos_source_text_unit_v1_lab_manifest_v1",
        "public_synthetic_contract_mechanics_only",
        &limit_value,
        limits,
    )?;
    let mut plan_ref = None;
    let mut source_ref = None;
    for field in ["contract", "research", "plan", "builder", "source"] {
        let bound = binding(&mut state, source, path, &manifest, field, false, limits)?;
        match field {
            "plan" => plan_ref = bound,
            "source" => source_ref = bound,
            _ => {}
        }
    }
    if manifest.pointer("/contract/ref").and_then(Value::as_str)
        != Some("ToS/contracts/source-text-unit-packet-v1.schema.json")
    {
        state.issue(path, "source text unit contract reference drifted", limits)?;
    }
    let plan = match plan_ref.as_deref() {
        Some(plan_path) => state
            .read_json(source, plan_path, limits)?
            .map(|(plan, _)| plan),
        None => None,
    };
    let expected_controls: std::collections::BTreeSet<String> = plan
        .as_ref()
        .and_then(|value| value.get("negative_controls"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let plan_limits_match = match plan
        .as_ref()
        .and_then(|value| value.get("authority_limits"))
    {
        Some(actual) => python_equal(actual, &limit_value)?,
        None => false,
    };
    if !plan_limits_match {
        state.issue(
            path,
            "source text unit plan authority limits drifted",
            limits,
        )?;
    }

    let text = match source_ref.as_deref() {
        Some(source_path) => match state.read(source, source_path, limits)? {
            Some(raw) => match String::from_utf8(raw) {
                Ok(text) => text,
                Err(_) => {
                    state.issue(
                        path,
                        "cannot read public synthetic source text: invalid UTF-8",
                        limits,
                    )?;
                    String::new()
                }
            },
            None => {
                state.issue(
                    path,
                    "cannot read public synthetic source text: file is missing",
                    limits,
                )?;
                String::new()
            }
        },
        None => String::new(),
    };
    let generation = source.generation();
    state.reserve(generation.len(), limits)?;
    let mut packets_by_variant = std::collections::BTreeMap::<String, Value>::new();
    let mut rows_out = check_packet_variants(
        &mut state,
        source,
        path,
        &manifest,
        "packet_ref",
        "packet_sha256",
        "ToS/contracts/source-text-unit-packet-v1.schema.json",
        Some(crate::text_rules::TEXT_UNIT_PROFILE),
        "source text unit",
        |state, source, id, packet_path, packet, raw, row| {
            source.checkpoint(limits.deadline)?;
            let semantic = crate::layer_family_rules::inspect_supplied_source_text_unit_semantics(
                packet_path,
                raw,
                source_ref.as_deref().unwrap_or(""),
                text.as_bytes(),
                &generation,
                limits,
            )?;
            source.checkpoint(limits.deadline)?;
            let semantic_messages = text_unit_owner_messages(&semantic.issues);
            let semantic_valid = semantic_messages.is_empty();
            if semantic.unsupported_profiles.len() > 0 {
                state.gap(
                    format!("text-unit semantic profile unavailable for {packet_path}"),
                    limits,
                )?;
            }
            if row.get("expected_semantic_valid").and_then(Value::as_bool) != Some(semantic_valid) {
                state.issue(
                    packet_path,
                    "semantic result differs from frozen A/B/C expectation",
                    limits,
                )?;
            }
            let text_sha = Digest256::of_bytes(text.as_bytes()).to_hex();
            if packet
                .pointer("/source_layer/text_layer_ref")
                .and_then(Value::as_str)
                != source_ref.as_deref()
            {
                state.issue(
                    packet_path,
                    "packet source layer does not resolve to manifest source",
                    limits,
                )?;
            }
            if packet
                .pointer("/source_layer/text_layer_sha256")
                .and_then(Value::as_str)
                != Some(text_sha.as_str())
            {
                state.issue(
                    packet_path,
                    "packet text_layer_sha256 differs from source input",
                    limits,
                )?;
            }
            if packet
                .pointer("/source_scope/file_sha256")
                .and_then(Value::as_str)
                != Some(text_sha.as_str())
            {
                state.issue(
                    packet_path,
                    "packet file digest differs from source input",
                    limits,
                )?;
            }
            state.reserve_decoded(packet, limits)?;
            packets_by_variant.insert(id.to_owned(), packet.clone());
            let statuses = packet
                .get("segmentations")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.get("status").cloned().unwrap_or(Value::Null))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let rejection_reasons = semantic_messages
                .iter()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let report_row = json!({
                "variant_id": id,
                "schema_valid": null,
                "semantic_valid": semantic_valid,
                "scheme_count": packet.get("schemes").and_then(Value::as_array).map_or(0, Vec::len),
                "segmentation_count": packet.get("segmentations").and_then(Value::as_array).map_or(0, Vec::len),
                "unit_count": packet.get("units").and_then(Value::as_array).map_or(0, Vec::len),
                "statuses": statuses,
                "review_count": packet.get("reviews").and_then(Value::as_array).map_or(0, Vec::len),
                "projection_count": packet.get("projections").and_then(Value::as_array).map_or(0, Vec::len),
                "rejection_reasons": rejection_reasons
            });
            state.reserve_decoded(&report_row, limits)?;
            Ok(report_row)
        },
        limits,
    )?;
    if packets_by_variant
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        == std::collections::BTreeSet::from(["A", "B", "C"])
    {
        let a = &packets_by_variant["A"];
        let b = &packets_by_variant["B"];
        let c = &packets_by_variant["C"];
        let a_statuses = a
            .get("segmentations")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| row.get("status").cloned().unwrap_or(Value::Null))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if a_statuses != vec![json!("observed_source_structure")] {
            state.issue(
                path,
                "variant A must contain exactly one source-layout observation",
                limits,
            )?;
        }
        let b_segmentations = b
            .get("segmentations")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let b_statuses: std::collections::BTreeSet<&str> = b_segmentations
            .iter()
            .filter_map(|row| row.get("status").and_then(Value::as_str))
            .collect();
        if b_segmentations.len() != 4
            || b_statuses != std::collections::BTreeSet::from(["ambiguous"])
        {
            state.issue(
                path,
                "variant B must preserve four unresolved competing segmentations",
                limits,
            )?;
        }
        for (packet, variant_id) in [(a, "A"), (b, "B"), (c, "C")] {
            if packet
                .get("reviews")
                .and_then(Value::as_array)
                .is_some_and(|rows| !rows.is_empty())
                || packet
                    .get("projections")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| !rows.is_empty())
            {
                state.issue(
                    path,
                    format!("variant {variant_id} fabricates review or projection evidence"),
                    limits,
                )?;
            }
        }
        let mut controls = Value::Object(serde_json::Map::new());
        let first_seg_id = b
            .pointer("/segmentations/0/segmentation_id")
            .cloned()
            .unwrap_or(Value::Null);
        let first_unit_id = b
            .pointer("/segmentations/0/ordered_unit_refs/0")
            .cloned()
            .unwrap_or(Value::Null);
        let second_unit_id = b
            .pointer("/segmentations/0/ordered_unit_refs/1")
            .cloned()
            .unwrap_or(Value::Null);
        let plan_ref_value = plan_ref.clone().map(Value::String).unwrap_or(Value::Null);
        let segmentation_ref = first_seg_id.clone();
        let source_layer = b.get("source_layer").cloned().unwrap_or_else(|| json!({}));

        state.reserve_value_clones(b, 25, limits)?;
        // Reviews, projections, and copied list members are small authored
        // control fragments in addition to the twenty-five packet mutations.
        state.reserve(16 * 1024, limits)?;
        let mut schedule_control = |name: &str, payload: Value| -> Result<(), ItemRefusal> {
            let serialized = state.serialize_json(&payload, limits)?;
            let semantic = crate::layer_family_rules::inspect_supplied_source_text_unit_semantics(
                "source-text-unit negative control",
                &serialized,
                source_ref.as_deref().unwrap_or(""),
                text.as_bytes(),
                &generation,
                limits,
            )?;
            let semantic_rejected = !text_unit_owner_messages(&semantic.issues).is_empty();
            let slot = format!("/negative_controls/{name}");
            state.schema_check_with_result(
                path,
                "ToS/contracts/source-text-unit-packet-v1.schema.json",
                &payload,
                Some(name),
                None,
                Some(semantic_rejected),
                Some(&slot),
                Some(format!("negative control was not rejected: {name}")),
                limits,
            )?;
            controls[name] = Value::String("pending_schema_diagnostic".into());
            Ok(())
        };

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/units/0/unit_id",
            json!("tos.text-unit.derived-from-current-text"),
        );
        schedule_control("text-derived-unit-id", mutated)?;

        let mut mutated = b.clone();
        if let Some(units) = mutated.get_mut("units").and_then(Value::as_array_mut) {
            if let Some(first) = units.first().cloned() {
                units.push(first);
            }
        }
        schedule_control("duplicate-unit-id", mutated)?;

        let mut mutated = b.clone();
        if let Some(schemes) = mutated.get_mut("schemes").and_then(Value::as_array_mut) {
            if let Some(first) = schemes.first().cloned() {
                schemes.push(first);
            }
        }
        schedule_control("duplicate-scheme-id", mutated)?;

        let mut mutated = b.clone();
        if let Some(segmentations) = mutated
            .get_mut("segmentations")
            .and_then(Value::as_array_mut)
        {
            if let Some(first) = segmentations.first().cloned() {
                segmentations.push(first);
            }
        }
        schedule_control("duplicate-segmentation-id", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/segmentations/0/scheme_ref",
            json!("tos.text-unit-scheme.sid-00000000000000000000000000000000"),
        );
        schedule_control("missing-scheme-ref", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/segmentations/0/ordered_unit_refs/0",
            json!("tos.text-unit.sid-00000000000000000000000000000000"),
        );
        schedule_control("missing-unit-ref", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/units/0/ordered_anchor_refs/0",
            json!("tos.anchor.source-text-unit-v1.synthetic-missing"),
        );
        schedule_control("missing-anchor-ref", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/source_layer/text_layer_sha256",
            json!("0".repeat(64)),
        );
        schedule_control("source-layer-digest-drift", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/anchors/1/exact_sha256",
            json!("1".repeat(64)),
        );
        schedule_control("anchor-exact-digest-drift", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/anchors/1/selector/end",
            json!(text.chars().count() + 1),
        );
        schedule_control("anchor-out-of-bounds", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/anchors/1/selector/start", json!(5));
        set_json_pointer(&mut mutated, "/anchors/1/selector/end", json!(4));
        schedule_control("anchor-reversed-range", mutated)?;

        let mut mutated = b.clone();
        let second_anchor = mutated
            .get("units")
            .and_then(Value::as_array)
            .and_then(|rows| {
                rows.iter()
                    .find(|unit| unit.get("unit_id") == Some(&second_unit_id))
            })
            .and_then(|unit| unit.pointer("/ordered_anchor_refs/0"))
            .cloned();
        let first_unit = mutated
            .get_mut("units")
            .and_then(Value::as_array_mut)
            .and_then(|rows| {
                rows.iter_mut()
                    .find(|unit| unit.get("unit_id") == Some(&first_unit_id))
            });
        if let (Some(unit), Some(anchor)) = (first_unit, second_anchor) {
            if let Some(anchors) = unit
                .get_mut("ordered_anchor_refs")
                .and_then(Value::as_array_mut)
            {
                anchors.push(anchor);
            }
        }
        schedule_control("unit-anchor-order-overlap", mutated)?;

        let mut mutated = b.clone();
        let parent_id = mutated
            .pointer("/units/0/unit_id")
            .cloned()
            .unwrap_or(Value::Null);
        set_json_pointer(
            &mut mutated,
            "/units/1/parent_unit_refs",
            json!([parent_id.clone()]),
        );
        schedule_control("one-way-parent-child", mutated)?;

        let mut mutated = b.clone();
        let self_id = mutated
            .pointer("/units/0/unit_id")
            .cloned()
            .unwrap_or(Value::Null);
        set_json_pointer(&mut mutated, "/units/0/parent_unit_refs", json!([self_id]));
        schedule_control("self-parent-unit", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/segmentations/0/competing_segmentation_refs",
            json!([]),
        );
        schedule_control("one-way-competing-segmentation", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(
            &mut mutated,
            "/segmentations/0/competing_segmentation_refs",
            json!([first_seg_id.clone()]),
        );
        schedule_control("self-competing-segmentation", mutated)?;

        let mut mutated = b.clone();
        if let Some(units) = mutated
            .pointer_mut("/segmentations/0/ordered_unit_refs")
            .and_then(Value::as_array_mut)
        {
            units.pop();
        }
        schedule_control("hidden-exhaustive-coverage-gap", mutated)?;

        let mut mutated = b.clone();
        let other_unit_id = b
            .pointer("/segmentations/1/ordered_unit_refs/0")
            .cloned()
            .unwrap_or(Value::Null);
        if let Some(units) = mutated
            .pointer_mut("/segmentations/0/ordered_unit_refs")
            .and_then(Value::as_array_mut)
        {
            units.push(other_unit_id);
        }
        schedule_control("undeclared-overlap", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/segmentations/0/status", json!("accepted"));
        schedule_control("accepted-machine-result-without-review", mutated)?;

        let weak_review_id = "tos.text-unit-review.sid-11111111111111111111111111111111";
        let weak_review = json!({
            "review_id": weak_review_id,
            "segmentation_refs": [segmentation_ref],
            "reviewer_kind": "real_human",
            "reviewer_ref": "human:synthetic-negative-control",
            "reviewed_at": "2026-08-11T18:00:00Z",
            "review_scope": "sample",
            "reviewed_unit_refs": [first_unit_id],
            "source_layer_ref": source_layer.get("text_layer_ref").cloned().unwrap_or(Value::Null),
            "source_layer_sha256": source_layer.get("text_layer_sha256").cloned().unwrap_or(Value::Null),
            "source_visible": true,
            "independent_boundary_decision_recorded_before_assistance": true,
            "unassisted_baseline_ref": plan_ref_value,
            "language_competence": {"language":"x-tos-unit","declared":true,"scope":"synthetic negative control only"},
            "outcome": "accepted",
            "notes_ref": plan_ref_value,
            "provenance_event_ref": "synthetic:negative-control",
            "sample_does_not_accept_unreviewed_units": true
        });
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(&mut mutated, "/reviews", json!([weak_review.clone()]));
        set_json_pointer(&mut mutated, "/segmentations/0/status", json!("accepted"));
        set_json_pointer(
            &mut mutated,
            "/segmentations/0/review_refs",
            json!([weak_review_id]),
        );
        schedule_control("accepted-with-sample-only-review", mutated)?;

        let mut mutated = b.clone();
        let mut weak_no_competence = weak_review.clone();
        set_json_pointer(&mut weak_no_competence, "/review_scope", json!("all_units"));
        set_json_pointer(
            &mut weak_no_competence,
            "/reviewed_unit_refs",
            mutated
                .pointer("/segmentations/0/ordered_unit_refs")
                .cloned()
                .unwrap_or(Value::Null),
        );
        set_json_pointer(
            &mut weak_no_competence,
            "/language_competence/declared",
            json!(false),
        );
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(&mut mutated, "/reviews", json!([weak_no_competence]));
        set_json_pointer(&mut mutated, "/segmentations/0/status", json!("accepted"));
        set_json_pointer(
            &mut mutated,
            "/segmentations/0/review_refs",
            json!([weak_review_id]),
        );
        schedule_control("accepted-without-language-competence", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/units/0/unit_kind", json!("model_subword"));
        set_json_pointer(
            &mut mutated,
            "/segmentations/0/linguistic_authority",
            json!(true),
        );
        schedule_control("model-subword-promoted-to-linguistic-authority", mutated)?;

        let projection = json!({
            "projection_id":"tos.text-unit-projection.sid-22222222222222222222222222222222",
            "projection_kind":"graph",
            "source_segmentation_refs":[segmentation_ref],
            "artifact_ref":plan_ref_value,
            "artifact_sha256":"2".repeat(64),
            "admission_posture":"accepted_only",
            "preserves_unit_ids":true,
            "preserves_status":true,
            "preserves_source_return":true,
            "runtime_authority":false,
            "source_text_authority":false,
            "linguistic_authority":false,
            "semantic_authority":false,
            "visibility":"public"
        });
        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(&mut mutated, "/projections", json!([projection.clone()]));
        schedule_control("graph-projection-of-proposed-segmentation", mutated)?;

        let mut mutated = b.clone();
        set_json_pointer(&mut mutated, "/content_posture", json!("source_bound"));
        set_json_pointer(
            &mut mutated,
            "/source_layer/visibility",
            json!("local_only"),
        );
        set_json_pointer(
            &mut mutated,
            "/source_layer/publication_authorized",
            json!(false),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/source_visibility",
            json!("local_only"),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/effective_visibility",
            json!("local_only"),
        );
        set_json_pointer(
            &mut mutated,
            "/rights_and_visibility/publication_authorized",
            json!(false),
        );
        set_json_pointer(&mut mutated, "/projections", json!([projection]));
        schedule_control("projection-visibility-widening", mutated)?;

        let mut mutated = b.clone();
        let packet_id = mutated
            .pointer("/packet_id")
            .cloned()
            .unwrap_or(Value::Null);
        set_json_pointer(&mut mutated, "/supersedes_packet_ref", packet_id);
        schedule_control("self-supersession", mutated)?;

        drop(schedule_control);
        let observed_controls: std::collections::BTreeSet<String> = controls
            .as_object()
            .map(|object| object.keys().cloned().collect())
            .unwrap_or_default();
        if expected_controls != observed_controls {
            state.issue(
                path,
                "negative-control coverage differs from the plan",
                limits,
            )?;
        }
        rows_out.shrink_to_fit();
        state.reserve_decoded(&controls, limits)?;
        state.report["negative_controls"] = controls;
    } else if !expected_controls.is_empty() {
        state.issue(
            path,
            "negative-control coverage differs from the plan",
            limits,
        )?;
    }
    state.report["variants"] = Value::Array(rows_out);
    result(
        SourceFoundationLab::SourceTextUnitV1,
        state,
        Vec::new(),
        limits,
    )
}

fn text_unit_owner_messages(issues: &[crate::text_rules::TextIssue]) -> Vec<String> {
    issues
        .iter()
        .filter_map(|issue| {
            let subject = issue.subject.as_str();
            let message = match issue.code {
                "source_layer_text_digest_mismatch"
                | "invalid_published_json"
                | "invalid_frozen_utf8" => return None,
                "packet_self_supersession" => {
                    "source-text-unit packet cannot supersede itself".to_owned()
                }
                "duplicate_scheme_id" => format!("duplicate text-unit scheme identity: {subject}"),
                "duplicate_anchor_id" => format!("duplicate text-unit anchor identity: {subject}"),
                "duplicate_unit_id" => format!("duplicate text unit identity: {subject}"),
                "duplicate_segmentation_id" => {
                    format!("duplicate text segmentation identity: {subject}")
                }
                "duplicate_review_id" => format!("duplicate text-unit review identity: {subject}"),
                "duplicate_projection_id" => {
                    format!("duplicate text-unit projection identity: {subject}")
                }
                "duplicate_anchor_ordinal" => {
                    format!("duplicate text-unit anchor ordinal: {subject}")
                }
                "anchor_layer_ref_mismatch" => {
                    format!("anchor escapes frozen source layer: {subject}")
                }
                "anchor_layer_digest_mismatch" => {
                    format!("anchor source-layer digest drifted: {subject}")
                }
                "anchor_reversed" => format!("anchor selector is reversed: {subject}"),
                "anchor_empty_nonmilestone" => {
                    format!("non-milestone anchor selector is empty: {subject}")
                }
                "anchor_outside_frozen_text" => {
                    format!("anchor selector is outside frozen text: {subject}")
                }
                "anchor_span_digest_mismatch" => {
                    format!("anchor exact digest does not resolve: {subject}")
                }
                "scheme_self_supersession" => {
                    format!("text-unit scheme cannot supersede itself: {subject}")
                }
                "synthetic_scheme_outside_lab" => {
                    format!("synthetic scheme maker escaped the synthetic laboratory: {subject}")
                }
                "unit_self_supersession" => format!("text unit cannot supersede itself: {subject}"),
                "missing_unit_anchor" => format!("unresolved unit anchor reference: {subject}"),
                "unit_anchor_overlap_or_reverse" => {
                    format!("unit anchor members overlap or reverse: {subject}")
                }
                "contiguous_unit_gap" => {
                    format!("contiguous unit has a gap between members: {subject}")
                }
                "discontinuous_unit_no_gap" => {
                    format!("discontinuous unit has no discontinuity: {subject}")
                }
                "missing_parent_unit" => format!("unresolved parent unit reference: {subject}"),
                "missing_child_unit" => format!("unresolved child unit reference: {subject}"),
                "unit_own_parent" => format!("text unit is its own parent: {subject}"),
                "unit_own_child" => format!("text unit is its own child: {subject}"),
                "unit_parent_nonreciprocal" => {
                    format!("unit parent relation is not reciprocal: {subject}")
                }
                "unit_child_nonreciprocal" => {
                    format!("unit child relation is not reciprocal: {subject}")
                }
                "missing_review_segmentation" => {
                    format!("unresolved reviewed segmentation reference: {subject}")
                }
                "missing_reviewed_unit" => format!("unresolved reviewed unit reference: {subject}"),
                "review_layer_ref_mismatch" => {
                    format!("review escapes frozen source layer: {subject}")
                }
                "review_layer_digest_mismatch" => {
                    format!("review source-layer digest drifted: {subject}")
                }
                "review_language_mismatch" => {
                    format!("review language competence does not match source: {subject}")
                }
                "review_segmentation_nonreciprocal" => {
                    format!("review-to-segmentation relation is not reciprocal: {subject}")
                }
                "segmentation_self_supersession" => {
                    format!("text segmentation cannot supersede itself: {subject}")
                }
                "missing_segmentation_scheme" => {
                    format!("unresolved segmentation scheme reference: {subject}")
                }
                "missing_segmentation_unit" => {
                    format!("unresolved segmentation unit reference: {subject}")
                }
                "unit_kind_outside_scheme" => {
                    format!("segmentation unit kind is outside its scheme: {subject}")
                }
                "missing_competing_segmentation" => {
                    format!("unresolved competing segmentation reference: {subject}")
                }
                "segmentation_competes_self" => {
                    format!("text segmentation competes with itself: {subject}")
                }
                "segmentation_competition_nonreciprocal" => {
                    format!("competing-segmentation relation is not reciprocal: {subject}")
                }
                "missing_segmentation_review" => {
                    format!("unresolved segmentation review reference: {subject}")
                }
                "segmentation_review_nonreciprocal" => {
                    format!("segmentation-to-review relation is not reciprocal: {subject}")
                }
                "reviewed_segmentation_without_review" => {
                    format!("reviewed segmentation lacks review: {subject}")
                }
                "accepted_segmentation_without_full_legacy_review" => format!(
                    "accepted segmentation lacks full competent real-human review: {subject}"
                ),
                "accepted_segmentation_unaccepted_unit" => {
                    format!("accepted segmentation contains an unaccepted unit: {subject}")
                }
                "source_structure_scheme_mismatch" => format!(
                    "source-observed segmentation is not source-layout/markup based: {subject}"
                ),
                "source_structure_proposed_unit" => {
                    format!("source-observed segmentation contains a proposed unit: {subject}")
                }
                "missing_coverage_scope" => format!("unresolved coverage scope anchor: {subject}"),
                "missing_excluded_anchor" => {
                    format!("unresolved excluded coverage anchor reference: {subject}")
                }
                "coverage_member_outside_scope" => {
                    format!("coverage member escapes scope: {subject}")
                }
                "coverage_undeclared_overlap" => {
                    format!("coverage overlaps without declaration: {subject}")
                }
                "coverage_overlap_policy_mismatch" => {
                    format!("declared overlap conflicts with scheme policy: {subject}")
                }
                "coverage_hidden_gap_or_extent" => format!(
                    "coverage does not reconstruct exact scope without hidden gaps: {subject}"
                ),
                "coverage_nonpartial_exclusion" => {
                    format!("non-partial segmentation declares excluded ranges: {subject}")
                }
                "coverage_partial_without_exclusion" => {
                    format!("partial segmentation lacks explicit excluded ranges: {subject}")
                }
                "unit_without_segmentation" => {
                    format!("text unit is not owned by any segmentation: {subject}")
                }
                "scheme_supersession_cycle" => {
                    format!("scheme supersession contains a cycle: {subject}")
                }
                "unit_supersession_cycle" => {
                    format!("unit supersession contains a cycle: {subject}")
                }
                "segmentation_supersession_cycle" => {
                    format!("segmentation supersession contains a cycle: {subject}")
                }
                "source_packet_visibility_mismatch" => {
                    "source-layer and packet visibility differ".to_owned()
                }
                "effective_visibility_not_most_restrictive" => {
                    "effective visibility is not the most restrictive source or packet component"
                        .to_owned()
                }
                "publication_boundary_widened" => {
                    "text-unit publication authority widens source boundary".to_owned()
                }
                "missing_projection_segmentation" => {
                    format!("unresolved projected segmentation reference: {subject}")
                }
                "projection_unaccepted_segmentation" => {
                    format!("accepted-only projection uses unaccepted segmentation: {subject}")
                }
                "projection_visibility_widened" => {
                    format!("text-unit projection visibility widens packet boundary: {subject}")
                }
                _ => return Some(format!("{}: {subject}", issue.code)),
            };
            Some(message)
        })
        .collect()
}

fn assessment_refusal_to_item(error: crate::assessment::AssessmentRefusal) -> ItemRefusal {
    match error {
        crate::assessment::AssessmentRefusal::Budget => ItemRefusal::Budget,
        crate::assessment::AssessmentRefusal::Cancelled => {
            ItemRefusal::Source("assessment predicate cancelled".into())
        }
        crate::assessment::AssessmentRefusal::Deadline => ItemRefusal::Deadline,
        crate::assessment::AssessmentRefusal::Schema(error) => error,
        crate::assessment::AssessmentRefusal::InvalidInput(_) => {
            ItemRefusal::Source("invalid assessment predicate input".into())
        }
        crate::assessment::AssessmentRefusal::Unsupported(_) => {
            ItemRefusal::Unsupported("assessment predicate".into())
        }
    }
}

fn owner_values_equal(left: &Value, right: &Value) -> Result<bool, ItemRefusal> {
    crate::assessment::py_equal(left, right).map_err(assessment_refusal_to_item)
}

fn opening_sentence_plan_binding_messages(
    plan: &Value,
    source_packet: &Value,
    target_packet: &Value,
    alignment: &Value,
) -> Result<Vec<String>, ItemRefusal> {
    let mut messages = Vec::new();
    for (label, packet, side_key) in [
        ("source", source_packet, "source_side"),
        ("target", target_packet, "target_side"),
    ] {
        let side_plan = plan.get(label).unwrap_or(&Value::Null);
        let anchor_ref = plan
            .pointer(&format!("/opaque_ids/{label}_sentence_anchor_id"))
            .unwrap_or(&Value::Null);
        let expected_selector = json!({
            "type":"text_position",
            "start":side_plan.get("sentence_start").cloned().unwrap_or(Value::Null),
            "end":side_plan.get("sentence_end").cloned().unwrap_or(Value::Null),
            "position_unit":"unicode_code_point",
            "interval":"half_open"
        });
        let mut packet_anchor = None;
        for row in packet
            .get("anchors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .rev()
            .filter(|row| row.is_object())
        {
            if let Some(value) = row.get("anchor_ref") {
                if owner_values_equal(value, anchor_ref)? {
                    packet_anchor = Some(row);
                    break;
                }
            }
        }
        let alignment_anchors: Vec<_> = alignment
            .pointer(&format!("/{side_key}/anchors"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|row| row.is_object())
            .collect();
        let alignment_anchor = (alignment_anchors.len() == 1).then(|| alignment_anchors[0]);
        for (owner, anchor) in [
            ("sentence-unit packet", packet_anchor),
            ("alignment side", alignment_anchor),
        ] {
            let Some(anchor) = anchor else {
                messages.push(format!("{label} {owner} sentence anchor is absent"));
                continue;
            };
            for (field, expected, text) in [
                ("anchor_ref", anchor_ref, "sentence anchor identity drifted"),
                (
                    "selector",
                    &expected_selector,
                    "sentence selector drifted from plan",
                ),
                (
                    "exact_sha256",
                    side_plan.get("sentence_sha256").unwrap_or(&Value::Null),
                    "sentence digest drifted from plan",
                ),
                (
                    "text_layer_ref",
                    side_plan.get("text_layer_ref").unwrap_or(&Value::Null),
                    "text-layer ref drifted from plan",
                ),
                (
                    "text_layer_sha256",
                    side_plan.get("text_layer_sha256").unwrap_or(&Value::Null),
                    "text-layer digest drifted from plan",
                ),
            ] {
                if !owner_values_equal(anchor.get(field).unwrap_or(&Value::Null), expected)? {
                    messages.push(format!("{label} {owner} {text}"));
                }
            }
            if !owner_values_equal(
                anchor
                    .pointer("/source_return/locator_ref")
                    .unwrap_or(&Value::Null),
                side_plan.get("private_content_ref").unwrap_or(&Value::Null),
            )? {
                messages.push(format!(
                    "{label} {owner} source-return locator drifted from plan"
                ));
            }
        }
    }
    Ok(messages)
}

/// Reuse the existing text-unit semantic kernel without opening private text.
/// The maintained bridge calls `_source_text_unit_v1_issues(packet)` without
/// `text`, so byte-span findings are excluded from this metadata-only route.
fn source_text_unit_metadata_messages(
    packet_path: &str,
    raw_packet: &[u8],
    limits: ItemLimits,
    generation: String,
) -> Result<Vec<String>, ItemRefusal> {
    let report = crate::text_rules::inspect_source_text_unit_v1(
        raw_packet,
        b"",
        &crate::text_rules::TextRuleContext {
            packet_path: packet_path.into(),
            frozen_text_locator: "metadata-only:no-private-text".into(),
            schema_checked: true,
            requested_profiles: vec![crate::text_rules::TEXT_UNIT_PROFILE.into()],
            interval_generation: generation.clone(),
            reverse_generation: generation,
        },
    );
    if report.state == crate::text_rules::TextRuleState::BudgetExceeded {
        return Err(ItemRefusal::Budget);
    }
    if std::time::Instant::now() >= limits.deadline {
        return Err(ItemRefusal::Deadline);
    }
    let semantic: Vec<_> = report
        .issues
        .iter()
        .filter(|issue| {
            !matches!(
                issue.code,
                "source_layer_text_digest_mismatch"
                    | "anchor_outside_frozen_text"
                    | "anchor_span_digest_mismatch"
            )
        })
        .cloned()
        .collect();
    Ok(text_unit_owner_messages(&semantic))
}

fn opening_sentence_family_issues(
    family: LayerFamilyReport,
) -> (Vec<(String, String)>, Vec<String>) {
    let mut result = Vec::new();
    let mut after_output_closure = Vec::new();
    let mut unimplemented = Vec::new();
    for issue in family.issues {
        let subject = issue.subject.as_str();
        let message = match issue.code {
            "opening-sentence-output-ref-absent" => {
                let field = match subject {
                    "source_sentence_packet_ref" => "source",
                    "target_sentence_packet_ref" => "target",
                    "alignment_packet_ref" => "alignment",
                    "provenance_event_ref" => "event",
                    _ => "opening-sentence",
                };
                format!("{field} output reference is absent")
            }
            "opening-sentence-source-scope" => format!("{subject} source scope drifted from plan"),
            "opening-sentence-frozen-layer" => format!("{subject} frozen source layer drifted"),
            "opening-sentence-fabricated-review-projection" => {
                format!("{subject} proposal fabricates review or projection")
            }
            "opening-sentence-segmentation-posture" => {
                format!("{subject} sentence segmentation is not one proposal")
            }
            "opening-sentence-segmentation-coverage" => {
                format!("{subject} proposal coverage or review posture drifted")
            }
            "opening-sentence-unit-posture" => format!("{subject} sentence unit posture drifted"),
            "opening-sentence-anchor" => format!("{subject} sentence anchor drifted"),
            "opening-sentence-excluded-remainder" => {
                format!("{subject} excluded remainder anchor drifted")
            }
            "opening-sentence-remainder-coverage" => {
                format!("{subject} excluded remainder is not closed")
            }
            "opening-sentence-alignment-side" => format!("alignment {subject} side drifted"),
            "opening-sentence-segmentation-binding" => {
                format!("alignment {subject} segmentation binding drifted")
            }
            "opening-sentence-side-anchor-count" => {
                format!("alignment {subject} anchor count drifted")
            }
            "opening-sentence-side-anchor" => {
                format!("alignment {subject} anchor drifted from unit packet")
            }
            "opening-sentence-fabricated-tokenization" => {
                format!("alignment {subject} fabricates tokenization")
            }
            // The maintained route emits plan authority and method findings
            // before it starts the two side checks. The caller performs those
            // exact comparisons at that point rather than inheriting the
            // family predicate's later traversal position.
            "opening-sentence-authority-boundary"
            | "opening-sentence-authority-boundary-shape"
            | "opening-sentence-method-widened" => continue,
            "opening-sentence-alignment-count" => {
                "real opening-sentence packet must contain one alignment".into()
            }
            "opening-sentence-alignment-posture" => {
                "opening-sentence alignment authority posture widened".into()
            }
            "opening-sentence-alignment-review-projection" => {
                "opening-sentence alignment fabricates review or projection".into()
            }
            "opening-sentence-rights-visibility" => {
                "opening-sentence rights or visibility drifted".into()
            }
            // Python checks event authority only after the event output
            // closure. Defer this mapped issue until the caller has inserted
            // the preceding tracked-input checks at that closure boundary.
            "opening-sentence-event-authority" => {
                after_output_closure
                    .push((issue.path, "alignment provenance authority widened".into()));
                continue;
            }
            "opening-sentence-event-output-closure" => {
                "alignment provenance output closure drifted".into()
            }
            // These are handled by exact document or retained-input adapters.
            "opening-sentence-plan-anchor-binding"
            | "opening-sentence-tracked-ref"
            | "opening-sentence-tracked-binding"
            | "opening-sentence-event-semantic"
            | "opening-sentence-output-profile"
            | "opening-sentence-event-profile" => continue,
            _ => {
                unimplemented.push(format!(
                    "opening-sentence family predicate {} lacks owner-prose mapping",
                    issue.code
                ));
                continue;
            }
        };
        result.push((issue.path, message));
    }
    result.extend(after_output_closure);
    for gap in family.unsupported {
        unimplemented.push(format!(
            "unsupported {} profile at {}",
            gap.profile, gap.path
        ));
    }
    (result, unimplemented)
}

fn opening_sentence_recorded_bindings(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    plan: &Value,
    plan_path: &str,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    let bindings = [
        ("source", "text_layer_ref", "text_layer_record_sha256"),
        ("source", "layout_packet_ref", "layout_packet_sha256"),
        (
            "source",
            "edition_reading_admission_ref",
            "edition_reading_admission_sha256",
        ),
        ("source", "rights_ref", "rights_sha256"),
        ("target", "text_layer_ref", "text_layer_record_sha256"),
        ("target", "layout_packet_ref", "layout_packet_sha256"),
        (
            "target",
            "expression_record_ref",
            "expression_record_sha256",
        ),
        (
            "target",
            "responsibility_claims_ref",
            "responsibility_claims_sha256",
        ),
        ("target", "rights_ref", "rights_sha256"),
    ];
    for (side, ref_field, digest_field) in bindings {
        source.checkpoint(limits.deadline)?;
        let row = plan.get(side).unwrap_or(&Value::Null);
        let path = row.get(ref_field).and_then(Value::as_str);
        let digest = row.get(digest_field).and_then(Value::as_str);
        let mut matched = false;
        if let (Some(path), Some(digest)) = (path, digest) {
            if RelativePath::parse(path).is_ok() {
                if let Some(raw) =
                    source.recorded(path, digest, limits.max_member_bytes, limits.deadline)?
                {
                    if raw.len() > limits.max_member_bytes {
                        return Err(ItemRefusal::Budget);
                    }
                    state.read_bytes = state
                        .read_bytes
                        .checked_add(raw.len() as u64)
                        .filter(|total| *total <= limits.max_total_bytes)
                        .ok_or(ItemRefusal::Budget)?;
                    state.reserve(raw.len(), limits)?;
                    matched = Digest256::of_bytes(&raw).to_hex() == digest;
                }
            }
        }
        if !matched {
            state.issue(
                plan_path,
                format!("tracked {side} binding drifted: {ref_field}"),
                limits,
            )?;
        }
    }
    source.checkpoint(limits.deadline)
}

fn inspect_opening_sentence(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let plan_path = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-opening-sentence-alignment.plan.v1.json";
    let mut direct = DirectState {
        report: Value::Null,
        ..Default::default()
    };
    let Some((plan, _)) = direct.read_json(source, plan_path, limits)? else {
        return result(
            SourceFoundationLab::ZarathustraOpeningSentence,
            direct,
            Vec::new(),
            limits,
        );
    };
    if plan.get("schema_version").and_then(Value::as_str)
        != Some("tos_zarathustra_opening_sentence_alignment_plan_v1")
    {
        direct.issue(
            plan_path,
            "unexpected opening-sentence alignment plan version",
            limits,
        )?;
    }
    let fields = [
        (
            "source_sentence_packet_ref",
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
            "source-text-unit schema: ",
        ),
        (
            "target_sentence_packet_ref",
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
            "source-text-unit schema: ",
        ),
        (
            "alignment_packet_ref",
            "ToS/contracts/translation-alignment-packet-v1.schema.json",
            "alignment schema: ",
        ),
        (
            "provenance_event_ref",
            "ToS/contracts/provenance-event-v2.schema.json",
            "provenance schema: ",
        ),
    ];
    let expected_output_count = fields.len();
    direct.reserve(
        std::mem::size_of::<Vec<(String, Value, Vec<u8>)>>()
            .checked_add(
                expected_output_count
                    .checked_mul(std::mem::size_of::<(String, Value, Vec<u8>)>())
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?,
        limits,
    )?;
    let mut output_rows = Vec::with_capacity(expected_output_count);
    for (field, contract, prefix) in fields {
        let Some(output_path) = plan
            .pointer("/outputs")
            .and_then(|v| v.get(field))
            .and_then(Value::as_str)
        else {
            direct.issue(
                plan_path,
                format!(
                    "{} output reference is absent",
                    field.strip_suffix("_packet_ref").unwrap_or(field)
                ),
                limits,
            )?;
            continue;
        };
        let Some((output, raw)) = direct.read_json(source, output_path, limits)? else {
            continue;
        };
        direct.schema_check_with_result(
            output_path,
            contract,
            &output,
            None,
            None,
            None,
            None,
            None,
            limits,
        )?;
        direct.set_last_schema_message_prefix(prefix, limits)?;
        direct.reserve(estimate_text_storage(output_path)?, limits)?;
        output_rows.push((field, output_path.to_owned(), output, raw));
    }
    // The maintained validator returns after loading if any of the four
    // documents is absent or malformed; no semantic findings are added then.
    if output_rows.len() != expected_output_count {
        direct.schema_checks.clear();
        return result(
            SourceFoundationLab::ZarathustraOpeningSentence,
            direct,
            Vec::new(),
            limits,
        );
    }
    let output = |index: usize| output_rows.get(index).ok_or(ItemRefusal::Budget);
    let source_row = output(0)?;
    let target_row = output(1)?;
    let alignment_row = output(2)?;
    let event_row = output(3)?;
    let source_packet = &source_row.2;
    let source_raw = &source_row.3;
    let target_packet = &target_row.2;
    let target_raw = &target_row.3;
    let alignment = &alignment_row.2;
    let event_path = &event_row.1;
    let event = &event_row.2;

    for message in
        opening_sentence_plan_binding_messages(&plan, source_packet, target_packet, alignment)?
    {
        direct.issue(plan_path, message, limits)?;
    }
    let check_base = direct.schema_checks.len().saturating_sub(output_rows.len());
    for (check_offset, (packet_path, packet, raw)) in [
        (&source_row.1, source_packet, source_raw),
        (&target_row.1, target_packet, target_raw),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(check) = direct.schema_checks.get_mut(check_base + check_offset) {
            check.before_issue = direct.issues.len();
        }
        for message in
            source_text_unit_metadata_messages(packet_path, raw, limits, source.generation())?
        {
            direct.issue(packet_path, message, limits)?;
        }
    }
    if let Some(check) = direct.schema_checks.get_mut(check_base + 2) {
        check.before_issue = direct.issues.len();
    }
    let alignment_family =
        crate::layer_family_rules::inspect_supplied_translation_alignment(alignment, limits)?;
    for message in translation_alignment_owner_messages(alignment, &alignment_family) {
        direct.issue(&alignment_row.1, message, limits)?;
    }
    if let Some(check) = direct.schema_checks.get_mut(check_base + 3) {
        check.before_issue = direct.issues.len();
    }
    for message in crate::provenance_rules::semantic_issues(
        event,
        limits.max_issues.saturating_sub(direct.issues.len()),
        limits.deadline,
    )? {
        direct.issue(event_path, message, limits)?;
    }
    let expected_authority = json!({
        "source_sentence_segmentation_status":"proposed",
        "target_sentence_segmentation_status":"proposed",
        "alignment_status":"proposed",
        "accepted_german":false,
        "accepted_russian":false,
        "accepted_translation":false,
        "translation_fidelity_established":false,
        "lexical_equivalence_established":false,
        "etymology_or_semantics_established":false,
        "human_task_created":false,
        "projection_created":false,
        "graph_or_canon_effect":false
    });
    if !owner_values_equal(
        plan.get("authority_boundary").unwrap_or(&Value::Null),
        &expected_authority,
    )? {
        direct.issue(
            plan_path,
            "opening-sentence plan authority boundary widened",
            limits,
        )?;
    }
    for field in [
        "dehyphenation",
        "tokenization",
        "model_invoked",
        "translation_performed",
        "recognized_translation_used_for_text_decision",
        "human_review_performed",
    ] {
        if plan
            .pointer(&format!("/method/{field}"))
            .and_then(Value::as_bool)
            != Some(false)
        {
            direct.issue(
                plan_path,
                format!("opening-sentence method widened {field}"),
                limits,
            )?;
        }
    }
    let mut rules = LayerFamilyRules::new(limits);
    rules.inspect_zarathustra_opening_sentence_without_schema(source)?;
    let family = rules.finish();
    let (family_issues, unimplemented) = opening_sentence_family_issues(family);
    let family_issue_count = family_issues.len();
    let binding_insert = family_issues
        .iter()
        .position(|(_, message)| message == "alignment provenance output closure drifted")
        .unwrap_or(family_issue_count);
    let mut ordered_issues = std::mem::take(&mut direct.issues);
    for (index, issue) in family_issues.into_iter().enumerate() {
        if index == binding_insert {
            opening_sentence_recorded_bindings(&mut direct, source, &plan, plan_path, limits)?;
            ordered_issues.append(&mut direct.issues);
        }
        ordered_issues.push(issue);
    }
    if binding_insert == family_issue_count {
        opening_sentence_recorded_bindings(&mut direct, source, &plan, plan_path, limits)?;
        ordered_issues.append(&mut direct.issues);
    }
    let schema_checks = direct.schema_checks;
    Ok(SourceFoundationLabResult {
        lab: SourceFoundationLab::ZarathustraOpeningSentence,
        report: direct.report,
        ordered_issues,
        // The owner validator explicitly leaves private-span replay to its
        // builder/manual route; this adapter covers the maintained validator.
        unimplemented,
        schema_checks,
        direct_read_bytes: direct.read_bytes,
        retained_state_bytes: direct.retained_bytes,
    })
}

fn json_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Object(value)) => !value.is_empty(),
    }
}

fn python_duplicate_count(values: &[Value]) -> Result<bool, ItemRefusal> {
    for right in 0..values.len() {
        for left in 0..right {
            if owner_values_equal(&values[left], &values[right])? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn python_contains(values: &[Value], needle: &Value) -> Result<bool, ItemRefusal> {
    for value in values {
        if owner_values_equal(value, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Shared semantic closure for an already bounded, schema-checked collation packet.
pub fn witness_text_collation_semantic_messages(packet: &Value) -> Result<Vec<String>, ItemRefusal> {
    let rows = |field: &str| -> Vec<&Value> {
        packet
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|row| row.is_object())
            .collect()
    };
    let witnesses = rows("witnesses");
    let collations = rows("collations");
    let reviews = rows("reviews");
    let witness_ids: Vec<_> = witnesses
        .iter()
        .map(|row| row.get("witness_id").cloned().unwrap_or(Value::Null))
        .collect();
    let witness_ordinals: Vec<_> = witnesses
        .iter()
        .map(|row| row.get("ordinal").cloned().unwrap_or(Value::Null))
        .collect();
    let expressions: Vec<_> = witnesses
        .iter()
        .map(|row| row.get("expression_ref").cloned().unwrap_or(Value::Null))
        .collect();
    let review_ids: Vec<_> = reviews
        .iter()
        .map(|row| row.get("review_id").cloned().unwrap_or(Value::Null))
        .collect();
    let mut messages = Vec::new();
    if python_duplicate_count(&witness_ids)? {
        messages.push("duplicate witness identity".into());
    }
    if python_duplicate_count(&witness_ordinals)? {
        messages.push("duplicate witness ordinal".into());
    }
    if python_duplicate_count(&expressions)? {
        messages.push("collation witnesses must retain distinct Expression identities".into());
    }
    let known_witnesses = witness_ids;
    let known_reviews = review_ids;
    let known_collations: Vec<_> = collations
        .iter()
        .filter_map(|row| row.get("collation_id").filter(|value| value.is_string()))
        .cloned()
        .collect();
    let mut accepted_ids = Vec::<Value>::new();
    for row in &collations {
        let status = row.get("status").and_then(Value::as_str);
        let decided = matches!(
            status,
            Some("accepted" | "accepted_with_limits" | "rejected" | "superseded")
        );
        let accepted = matches!(status, Some("accepted" | "accepted_with_limits"));
        for reference in row
            .get("ordered_witness_refs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if !python_contains(&known_witnesses, reference)? {
                messages.push("collation references an unknown witness".into());
            }
        }
        let review_refs = row
            .get("review_refs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for reference in &review_refs {
            if !python_contains(&known_reviews, reference)? {
                messages.push("collation references an unknown review".into());
            }
        }
        if decided && review_refs.is_empty() {
            messages.push("decided collation lacks human review".into());
        }
        if accepted && row.pointer("/maker/maker_kind").and_then(Value::as_str) != Some("human") {
            messages.push("non-human collation acceptance".into());
        }
        if owner_values_equal(
            row.get("collation_id").unwrap_or(&Value::Null),
            row.get("supersedes_collation_ref").unwrap_or(&Value::Null),
        )? {
            messages.push("collation cannot supersede itself".into());
        }
        if owner_values_equal(
            row.get("claim_id").unwrap_or(&Value::Null),
            row.get("supersedes_claim_ref").unwrap_or(&Value::Null),
        )? {
            messages.push("collation claim cannot supersede itself".into());
        }
        if let Some(boundary) = row.get("interpretive_boundary").and_then(Value::as_object) {
            if boundary.values().any(|value| value != &Value::Bool(false)) {
                messages.push("collation interpretive boundary widened".into());
            }
        }
        if accepted {
            if let Some(id) = row.get("collation_id") {
                accepted_ids.push(id.clone());
            }
        }
    }
    for projection in rows("projections") {
        let references = projection
            .get("source_collation_refs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        let mut unknown = false;
        let mut unaccepted = false;
        for reference in references {
            if !python_contains(&known_collations, reference)? {
                unknown = true;
            }
            if !python_contains(&accepted_ids, reference)? {
                unaccepted = true;
            }
        }
        if unknown {
            messages.push("projection references an unknown collation".into());
        }
        if unaccepted {
            messages.push("projection uses a collation that is not accepted".into());
        }
    }
    let rights = packet.get("rights_and_visibility");
    if json_truthy(rights.and_then(|value| value.get("private_source_used")))
        && json_truthy(rights.and_then(|value| value.get("publication_authorized")))
    {
        messages.push("private-source collation authorizes publication".into());
    }
    Ok(messages)
}

fn inspect_antonovsky_collation(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    physical: Option<&SourcePhysicalFacts>,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let plan = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/antonovsky-2007-1911-opening-sentence-collation.plan.v1.json";
    let mut state = DirectState {
        report: Value::Null,
        ..Default::default()
    };
    let Some((plan_value, _)) = state.read_json(source, plan, limits)? else {
        return result(
            SourceFoundationLab::AntonovskyCollation,
            state,
            Vec::new(),
            limits,
        );
    };
    if plan_value.get("schema_version").and_then(Value::as_str)
        != Some("tos_antonovsky_2007_1911_opening_sentence_collation_plan_v1")
    {
        state.issue(
            plan,
            "unexpected Antonovsky witness-collation plan version",
            limits,
        )?;
    }
    let output_specs = [
        (
            "source_anchor_ref",
            "ToS/contracts/source-anchor-v2.schema.json",
        ),
        (
            "source_text_layer_ref",
            "ToS/contracts/source-text-layer.schema.json",
        ),
        (
            "source_text_unit_packet_ref",
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
        ),
        (
            "collation_packet_ref",
            "ToS/contracts/witness-text-collation-packet-v1.schema.json",
        ),
        (
            "provenance_event_ref",
            "ToS/contracts/provenance-event-v2.schema.json",
        ),
    ];
    let output_map = named_outputs(&mut state, source, plan, &plan_value, &output_specs, limits)?;
    if output_map.len() != output_specs.len() {
        // The maintained bridge stops after the five output records fail to
        // load; schema and semantic checks are not run on a partial set.
        state.schema_checks.clear();
        return result(
            SourceFoundationLab::AntonovskyCollation,
            state,
            Vec::new(),
            limits,
        );
    }
    let schema_base = state
        .schema_checks
        .len()
        .checked_sub(output_specs.len())
        .ok_or(ItemRefusal::Budget)?;
    for (index, (label, _)) in output_specs.iter().enumerate() {
        let Some((path, value, raw)) = output_map.get(*label) else {
            continue;
        };
        if let Some(check) = state.schema_checks.get_mut(schema_base + index) {
            check.before_issue = state.issues.len();
        }
        let messages = match *label {
            "source_anchor_ref" => anchor_v2_semantic_issues(value),
            "source_text_layer_ref" => source_text_layer_semantic_issues(value),
            "source_text_unit_packet_ref" => {
                source_text_unit_metadata_messages(path, raw, limits, source.generation())?
            }
            "collation_packet_ref" => witness_text_collation_semantic_messages(value)?,
            "provenance_event_ref" => crate::provenance_rules::semantic_issues(
                value,
                limits.max_issues.saturating_sub(state.issues.len()),
                limits.deadline,
            )?
            .into_iter()
            .map(str::to_owned)
            .collect(),
            _ => Vec::new(),
        };
        for message in messages {
            state.issue(path, message, limits)?;
        }
    }
    exact_value_issue(
        &mut state,
        plan,
        plan_value.get("authority_boundary"),
        json!({"human_observation_preserved":true,"human_attestation_collected":false,"human_review_performed":false,"gold_created":false,"source_text_accepted":false,"sentence_boundary_accepted":false,"collation_status":"proposed","preferred_reading_selected":false,"textual_equivalence_established":false,"expression_derivation_established":false,"translation_relation_created":false,"lexical_or_semantic_relation_created":false,"projection_created":false,"graph_or_canon_effect":false,"routine_human_task_created":false,"publication_authorized":false}),
        "Antonovsky witness-collation plan authority widened",
        limits,
    )?;
    let observation = plan_value.get("human_observation").unwrap_or(&Value::Null);
    if [
        "pass_receipt_collected",
        "source_review_created",
        "gold_created",
        "promotion_authorized",
    ]
    .iter()
    .any(|field| observation.get(*field).and_then(Value::as_bool) != Some(false))
        || observation
            .get("attestation_status")
            .and_then(Value::as_str)
            != Some("not_collected")
    {
        state.issue(
            plan,
            "unattested Workbench observation was promoted",
            limits,
        )?;
    }
    let source_side = plan_value.get("witness_2007").unwrap_or(&Value::Null);
    let target_side = plan_value.get("witness_1911").unwrap_or(&Value::Null);
    let mut deps = vec![
        (plan_value.get("research_ref"), None),
        (plan_value.get("contract_ref"), None),
        (
            observation.get("assurance_ref"),
            observation.get("assurance_sha256"),
        ),
        (
            observation.get("method_research_ref"),
            observation.get("method_research_sha256"),
        ),
        (source_side.get("item_manifest_ref"), None),
        (source_side.get("resource_inventory_ref"), None),
        (
            source_side.get("rights_ref"),
            source_side.get("rights_sha256"),
        ),
        (
            target_side.get("text_layer_record_ref"),
            target_side.get("text_layer_record_sha256"),
        ),
        (
            target_side.get("text_unit_packet_ref"),
            target_side.get("text_unit_packet_sha256"),
        ),
        (
            target_side.get("rights_ref"),
            target_side.get("rights_sha256"),
        ),
    ];
    for (ref_value, digest_value) in deps.drain(..) {
        let Some(ref_path) = ref_value.and_then(Value::as_str) else {
            state.issue(
                plan,
                "tracked collation dependency reference is absent",
                limits,
            )?;
            continue;
        };
        let Some(raw) = state.read(source, ref_path, limits)? else {
            state.issue(
                plan,
                format!("tracked collation dependency is absent: {ref_path}"),
                limits,
            )?;
            continue;
        };
        if digest_value
            .and_then(Value::as_str)
            .is_some_and(|expected| Digest256::of_bytes(&raw).to_hex() != expected)
        {
            state.issue(
                plan,
                format!("tracked collation dependency digest drifted: {ref_path}"),
                limits,
            )?;
        }
    }
    let outputs = plan_value.get("outputs").unwrap_or(&Value::Null);
    for field in ["private_text_ref", "private_detail_ref"] {
        let Some(ref_path) = outputs.get(field).and_then(Value::as_str) else {
            state.issue(
                plan,
                format!("unsafe private collation output: {field}"),
                limits,
            )?;
            continue;
        };
        if !ref_path.contains("local-content/witness-text-collation/") {
            state.issue(
                plan,
                format!("unsafe private collation output: {field}"),
                limits,
            )?;
            continue;
        }
        source.checkpoint(limits.deadline)?;
        check_private_git_ignore(
            &mut state,
            physical,
            ref_path,
            field,
            plan,
            "private collation output is not Git-ignored",
            "physical Git-ignore fact unavailable for private collation output",
            limits,
        )?;
    }
    let ids = plan_value.get("opaque_ids").unwrap_or(&Value::Null);
    let anchor = output_map.get("source_anchor_ref").map(|row| &row.1);
    let layer = output_map.get("source_text_layer_ref").map(|row| &row.1);
    let unit = output_map
        .get("source_text_unit_packet_ref")
        .map(|row| &row.1);
    let collation = output_map.get("collation_packet_ref").map(|row| &row.1);
    let event = output_map.get("provenance_event_ref").map(|row| &row.1);
    if let Some(anchor) = anchor {
        let selector = anchor
            .pointer("/selector_payload/expression/selector")
            .unwrap_or(&Value::Null);
        let target = anchor.get("target").unwrap_or(&Value::Null);
        let expected_anchor_target = json!({
            "item_id":source_side.get("item_ref"),
            "file_id":source_side.get("file_ref"),
            "file_sha256":source_side.get("file_sha256"),
            "media_type":"application/pdf"
        });
        if anchor.get("anchor_id") != ids.get("source_anchor_id")
            || anchor.get("passage_id") != ids.get("source_passage_id")
            || !owner_values_equal(target, &expected_anchor_target)?
            || anchor.get("review_status").and_then(Value::as_str) != Some("unreviewed")
            || anchor.get("review_ref").is_some_and(|v| !v.is_null())
            || anchor
                .pointer("/publication_boundary/source_text_in_record")
                .and_then(Value::as_bool)
                != Some(false)
            || anchor
                .pointer("/publication_boundary/public_payload_expected")
                .and_then(Value::as_bool)
                != Some(false)
            || !owner_values_equal(
                selector
                    .pointer("/state/representation_sha256")
                    .unwrap_or(&Value::Null),
                source_side.get("file_sha256").unwrap_or(&Value::Null),
            )?
            || !owner_values_equal(
                selector
                    .pointer("/selector/page_identity/page_number")
                    .unwrap_or(&Value::Null),
                source_side.get("page_number").unwrap_or(&Value::Null),
            )?
        {
            state.issue(
                output_map
                    .get("source_anchor_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "2007 source-anchor binding or authority drifted",
                limits,
            )?;
        }
    }
    if let Some(layer) = layer {
        let rep = layer.get("representation").unwrap_or(&Value::Null);
        let admission = layer.get("admission").unwrap_or(&Value::Null);
        if layer.get("layer_id") != ids.get("source_text_layer_id")
            || layer.get("layer_role").and_then(Value::as_str) != Some("diplomatic_transcription")
            || rep.get("content_ref") != outputs.get("private_text_ref")
            || rep.get("content_sha256") != source_side.get("text_layer_sha256")
            || rep.get("tracked_content").and_then(Value::as_bool) != Some(false)
            || rep.get("content_visibility").and_then(Value::as_str) != Some("local_only")
            || rep.get("publication_authorized").and_then(Value::as_bool) != Some(false)
            || admission.get("review_status").and_then(Value::as_str) != Some("unreviewed")
            || admission
                .get("human_review_performed")
                .and_then(Value::as_bool)
                != Some(false)
            || admission.get("accepted_uses") != Some(&json!([]))
            || admission
                .get("promotion_authorized")
                .and_then(Value::as_bool)
                != Some(false)
            || admission
                .get("routine_human_task_created")
                .and_then(Value::as_bool)
                != Some(false)
        {
            state.issue(
                output_map
                    .get("source_text_layer_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "2007 observation layer authority or private-content posture drifted",
                limits,
            )?;
        }
        let expected_anchor_binding = json!([{
            "anchor_id": ids.get("source_anchor_id").cloned().unwrap_or(Value::Null),
            "anchor_record_ref": output_map
                .get("source_anchor_ref")
                .map(|row| Value::String(row.0.clone()))
                .unwrap_or(Value::Null),
            "anchor_record_sha256": output_map
                .get("source_anchor_ref")
                .map(|row| Digest256::of_bytes(&row.2).to_hex())
        }]);
        if !owner_values_equal(
            layer
                .pointer("/source_binding/anchors")
                .unwrap_or(&Value::Null),
            &expected_anchor_binding,
        )? {
            state.issue(
                output_map
                    .get("source_text_layer_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "2007 layer-to-anchor fixity closure drifted",
                limits,
            )?;
        }
    }
    if let Some(unit) = unit {
        let scope = source_side;
        let fields = [
            "work_ref",
            "expression_ref",
            "edition_ref",
            "item_ref",
            "file_ref",
            "file_sha256",
        ];
        let mut expected_scope = serde_json::Map::new();
        for field in fields {
            expected_scope.insert(
                field.into(),
                scope.get(field).cloned().unwrap_or(Value::Null),
            );
        }
        if !owner_values_equal(
            unit.get("packet_id").unwrap_or(&Value::Null),
            ids.get("source_unit_packet_id").unwrap_or(&Value::Null),
        )? || !owner_values_equal(
            unit.get("source_scope").unwrap_or(&Value::Null),
            &Value::Object(expected_scope),
        )? {
            state.issue(
                output_map
                    .get("source_text_unit_packet_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "2007 source-text-unit identity or scope drifted",
                limits,
            )?;
        }
        let anchors = unit
            .get("anchors")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut by_id = std::collections::BTreeMap::<String, &Value>::new();
        for row in anchors {
            if let Some(id) = row.get("anchor_ref").and_then(Value::as_str) {
                by_id.insert(id.to_owned(), row);
            }
        }
        let anchor_expectations = [
            (
                "source_scope_anchor_id",
                Value::from(0),
                source_side
                    .get("text_layer_codepoints")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side.get("text_layer_sha256"),
            ),
            (
                "source_heading_anchor_id",
                source_side
                    .get("heading_start")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side
                    .get("heading_end")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side.get("heading_sha256"),
            ),
            (
                "source_interstitial_anchor_id",
                source_side
                    .get("interstitial_start")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side
                    .get("interstitial_end")
                    .cloned()
                    .unwrap_or(Value::Null),
                None,
            ),
            (
                "source_sentence_anchor_id",
                source_side
                    .get("sentence_start")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side
                    .get("sentence_end")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side.get("sentence_sha256"),
            ),
            (
                "source_remainder_anchor_id",
                source_side
                    .get("remainder_start")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side
                    .get("remainder_end")
                    .cloned()
                    .unwrap_or(Value::Null),
                source_side.get("remainder_sha256"),
            ),
        ];
        for (id_field, start, end, digest) in anchor_expectations {
            let id = ids.get(id_field).and_then(Value::as_str).unwrap_or("");
            let row = by_id.get(id).copied().unwrap_or(&Value::Null);
            let selector = row.get("selector").unwrap_or(&Value::Null);
            if !owner_values_equal(selector.get("start").unwrap_or(&Value::Null), &start)?
                || !owner_values_equal(selector.get("end").unwrap_or(&Value::Null), &end)?
                || !owner_values_equal(
                    row.get("text_layer_ref").unwrap_or(&Value::Null),
                    outputs.get("private_text_ref").unwrap_or(&Value::Null),
                )?
                || !owner_values_equal(
                    row.get("text_layer_sha256").unwrap_or(&Value::Null),
                    source_side.get("text_layer_sha256").unwrap_or(&Value::Null),
                )?
                || (digest.is_some_and(|expected| row.get("exact_sha256") != Some(expected)))
            {
                state.issue(
                    output_map
                        .get("source_text_unit_packet_ref")
                        .map(|row| row.0.as_str())
                        .unwrap_or(plan),
                    format!("2007 unit anchor drifted: {id_field}"),
                    limits,
                )?;
            }
        }
        let segmentations = unit
            .get("segmentations")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let units = unit
            .get("units")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let expected_ordered_units = json!([ids.get("source_sentence_unit_id")]);
        if segmentations.len() != 1
            || segmentations[0].get("status").and_then(Value::as_str) != Some("proposed")
            || segmentations[0].get("review_refs") != Some(&json!([]))
            || !owner_values_equal(
                segmentations[0]
                    .get("ordered_unit_refs")
                    .unwrap_or(&Value::Null),
                &expected_ordered_units,
            )?
            || units.len() != 1
            || units[0].get("unit_id") != ids.get("source_sentence_unit_id")
            || units[0].get("boundary_posture").and_then(Value::as_str) != Some("method_proposed")
            || units[0].get("semantic_promotion").and_then(Value::as_bool) != Some(false)
            || unit.get("reviews") != Some(&json!([]))
            || unit.get("projections") != Some(&json!([]))
        {
            state.issue(
                output_map
                    .get("source_text_unit_packet_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "2007 sentence proposal was widened or lost closure",
                limits,
            )?;
        }
    }
    if let Some(collation) = collation {
        let witnesses = collation
            .get("witnesses")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let proposals = collation
            .get("collations")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if collation.get("packet_id") != ids.get("collation_packet_id")
            || witnesses.len() != 2
            || proposals.len() != 1
        {
            state.issue(
                output_map
                    .get("collation_packet_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "collation packet identity or cardinality drifted",
                limits,
            )?;
        }
        let witness_by_id: std::collections::BTreeMap<_, _> = witnesses
            .iter()
            .filter_map(|row| Some((row.get("witness_id")?.as_str()?, row)))
            .collect();
        let source_unit_row = output_map.get("source_text_unit_packet_ref");
        let expected_witnesses = [
            (
                ids.get("witness_2007_id").and_then(Value::as_str),
                source_side,
                source_unit_row.map(|row| row.0.as_str()),
                source_unit_row.map(|row| Digest256::of_bytes(&row.2).to_hex()),
                ids.get("source_sentence_anchor_id").and_then(Value::as_str),
            ),
            (
                ids.get("witness_1911_id").and_then(Value::as_str),
                target_side,
                target_side
                    .get("text_unit_packet_ref")
                    .and_then(Value::as_str),
                target_side
                    .get("text_unit_packet_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                target_side.get("anchor_ref").and_then(Value::as_str),
            ),
        ];
        for (witness_id, side, packet_ref, packet_sha, anchor_ref) in expected_witnesses {
            let id = witness_id.unwrap_or("");
            let row = witness_by_id.get(id).copied().unwrap_or(&Value::Null);
            let selector = row.get("selector").unwrap_or(&Value::Null);
            let expected_packet = json!({
                "ref":packet_ref,
                "sha256":packet_sha
            });
            let selector_start = side.get("sentence_start").unwrap_or(&Value::Null);
            let selector_end = side.get("sentence_end").unwrap_or(&Value::Null);
            if row.get("work_ref") != side.get("work_ref")
                || row.get("expression_ref") != side.get("expression_ref")
                || row.get("edition_ref") != side.get("edition_ref")
                || row.get("item_ref") != side.get("item_ref")
                || row.get("anchor_ref").and_then(Value::as_str) != anchor_ref
                || row.get("exact_sha256") != side.get("sentence_sha256")
                || !owner_values_equal(
                    selector.get("start").unwrap_or(&Value::Null),
                    selector_start,
                )?
                || !owner_values_equal(selector.get("end").unwrap_or(&Value::Null), selector_end)?
                || !owner_values_equal(
                    row.get("text_unit_packet").unwrap_or(&Value::Null),
                    &expected_packet,
                )?
                || row.get("visibility").and_then(Value::as_str) != Some("local_only")
                || row.get("publication_authorized").and_then(Value::as_bool) != Some(false)
                || row.get("source_text_in_record").and_then(Value::as_bool) != Some(false)
            {
                state.issue(
                    output_map
                        .get("collation_packet_ref")
                        .map(|row| row.0.as_str())
                        .unwrap_or(plan),
                    format!("collation witness binding drifted: {id}"),
                    limits,
                )?;
            }
        }
        let proposal = proposals.first().unwrap_or(&Value::Null);
        let mut expected_views = Vec::new();
        if let Some(views) = plan_value
            .pointer("/comparison_method/views")
            .and_then(Value::as_array)
        {
            for view in views {
                let mut expected = state.clone_value_charged(&view, limits)?;
                if let Some(object) = expected.as_object_mut() {
                    object.insert(
                        "similarity_metric".into(),
                        Value::String("sequence_matcher_ratio".into()),
                    );
                }
                expected_views.push(expected);
            }
        }
        if proposal.get("collation_id") != ids.get("collation_id")
            || proposal.get("claim_id") != ids.get("collation_claim_id")
            || proposal.get("status").and_then(Value::as_str) != Some("proposed")
            || proposal.get("review_refs") != Some(&json!([]))
            || proposal.get("correspondence_shape").and_then(Value::as_str) != Some("one_to_one")
            || !owner_values_equal(
                proposal.get("comparison_views").unwrap_or(&Value::Null),
                &Value::Array(expected_views),
            )?
            || proposal
                .pointer("/maker/maker_kind")
                .and_then(Value::as_str)
                != Some("software")
            || proposal
                .pointer("/interpretive_boundary")
                .and_then(Value::as_object)
                .is_some_and(|object| object.values().any(|value| value != &Value::Bool(false)))
            || proposal
                .pointer("/private_detail/tracked")
                .and_then(Value::as_bool)
                != Some(false)
            || proposal
                .pointer("/private_detail/visibility")
                .and_then(Value::as_str)
                != Some("local_only")
            || collation.get("reviews") != Some(&json!([]))
            || collation.get("projections") != Some(&json!([]))
        {
            state.issue(
                output_map
                    .get("collation_packet_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "collation proposal, metrics, or non-promotion boundary drifted",
                limits,
            )?;
        }
        let expected_rights = json!({
            "witness_visibility":"local_only",
            "packet_visibility":"public_metadata_only",
            "effective_visibility":"local_only",
            "rights_record_refs":[source_side.get("rights_ref"),target_side.get("rights_ref")],
            "private_source_used":true,
            "publication_authorized":false,
            "inheritance_policy":"most-restrictive-witness-packet-detail-and-destination-wins"
        });
        if !owner_values_equal(
            collation
                .get("rights_and_visibility")
                .unwrap_or(&Value::Null),
            &expected_rights,
        )? {
            state.issue(
                output_map
                    .get("collation_packet_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "collation rights or visibility drifted",
                limits,
            )?;
        }
    }
    if let Some(event) = event {
        let expected: std::collections::BTreeMap<_, _> = output_map
            .iter()
            .filter(|(key, _)| key.as_str() != "provenance_event_ref")
            .map(|(_, (path, _, raw))| (path.as_str(), Digest256::of_bytes(raw).to_hex()))
            .collect();
        let actual: std::collections::BTreeMap<_, _> = event
            .pointer("/entities/outputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|row| {
                Some((
                    row.get("entity_ref")?.as_str()?,
                    row.get("sha256")?.as_str()?.to_owned(),
                ))
            })
            .collect();
        if expected != actual {
            state.issue(
                output_map
                    .get("provenance_event_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "collation provenance output closure drifted",
                limits,
            )?;
        }
        let byproducts = event
            .pointer("/entities/byproducts")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut byproduct_map = std::collections::BTreeMap::<String, &Value>::new();
        let mut missing_byproduct_reference = false;
        for row in byproducts.iter().filter(|row| row.is_object()) {
            if let Some(reference) = row.get("entity_ref").and_then(Value::as_str) {
                byproduct_map.insert(reference.to_owned(), row);
            } else {
                missing_byproduct_reference = true;
            }
        }
        let mut actual_private = std::collections::BTreeSet::new();
        let mut byproducts_ok = true;
        for (reference, row) in &byproduct_map {
            if row.get("availability").and_then(Value::as_str) == Some("ignored_local") {
                actual_private.insert(reference.clone());
            }
            if row.get("availability").and_then(Value::as_str) != Some("ignored_local")
                || row.get("content_disclosure").and_then(Value::as_str) != Some("private_content")
            {
                byproducts_ok = false;
            }
        }
        let expected_private: std::collections::BTreeSet<_> = [
            outputs.get("private_text_ref").and_then(Value::as_str),
            outputs.get("private_detail_ref").and_then(Value::as_str),
        ]
        .into_iter()
        .flatten()
        .map(str::to_owned)
        .collect();
        if missing_byproduct_reference || !byproducts_ok || actual_private != expected_private {
            state.issue(
                output_map
                    .get("provenance_event_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "private collation byproduct closure drifted",
                limits,
            )?;
        }
        if event
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            != Some(plan)
            || event
                .pointer("/activity/event_type")
                .and_then(Value::as_str)
                != Some("alignment")
            || event
                .pointer("/review_and_authority/human_review_status")
                .and_then(Value::as_str)
                != Some("not_performed")
            || event.pointer("/review_and_authority/accepted_uses") != Some(&json!([]))
            || event
                .pointer("/review_and_authority/promotion_authorized")
                .and_then(Value::as_bool)
                != Some(false)
            || event
                .pointer("/rights_and_visibility/content_visibility")
                .and_then(Value::as_str)
                != Some("local_only")
            || event
                .pointer("/rights_and_visibility/publication_authorized")
                .and_then(Value::as_bool)
                != Some(false)
        {
            state.issue(
                output_map
                    .get("provenance_event_ref")
                    .map(|row| row.0.as_str())
                    .unwrap_or(plan),
                "collation provenance authority widened",
                limits,
            )?;
        }
    }
    if metadata_leak(
        &plan_value,
        &[
            "diplomatic_transcription",
            "opcodes",
            "source_text",
            "target_text",
        ],
        &[],
    )
    .is_some()
    {
        state.issue(
            plan,
            "tracked collation record contains private-content field",
            limits,
        )?;
    }
    for (field, _) in output_specs {
        if let Some(value) = output_map.get(field) {
            if metadata_leak(
                &value.1,
                &[
                    "diplomatic_transcription",
                    "opcodes",
                    "source_text",
                    "target_text",
                ],
                &[],
            )
            .is_some()
            {
                state.issue(
                    &value.0,
                    "tracked collation record contains private-content field",
                    limits,
                )?;
            }
        }
    }
    result(
        SourceFoundationLab::AntonovskyCollation,
        state,
        Vec::new(),
        limits,
    )
}

fn inspect_authored_canon_bridge(
    source: &mut impl LayerFamilySource,
    limits: ItemLimits,
    current_paths: &dyn SourceFoundationDefaultPaths,
    physical: Option<&SourcePhysicalFacts>,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    let plan = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/authored-canon-evidence-bridge.plan.v1.json";
    let mut state = DirectState {
        report: Value::Null,
        ..Default::default()
    };
    let Some((plan_value, plan_raw)) = state.read_json(source, plan, limits)? else {
        return result(
            SourceFoundationLab::ZarathustraAuthoredCanonBridge,
            state,
            Vec::new(),
            limits,
        );
    };
    if plan_value.get("schema_version").and_then(Value::as_str)
        != Some("tos_authored_canon_evidence_bridge_plan_v1")
    {
        state.issue(
            plan,
            "unexpected authored-canon evidence-bridge plan version",
            limits,
        )?;
    }
    exact_value_issue(
        &mut state,
        plan,
        plan_value.get("authority_boundary"),
        json!({"legacy_canon_retained":true,"bulk_migration_authorized":false,"human_review_performed":false,"german_competence_attested":false,"accepted_german":false,"accepted_translation":false,"semantic_or_sign_promotion":false,"graph_admission":false,"canon_revision":false,"human_task_created":false,"new_publication_authorized":false,"server_transfer_authorized":false}),
        "authored-canon bridge plan authority widened",
        limits,
    )?;
    let output_specs = [
        ("anchor_ref", "ToS/contracts/source-anchor-v2.schema.json"),
        (
            "raw_layer_ref",
            "ToS/contracts/source-text-layer.schema.json",
        ),
        (
            "normalized_layer_ref",
            "ToS/contracts/source-text-layer.schema.json",
        ),
        (
            "unit_packet_ref",
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
        ),
        (
            "bridge_ref",
            "ToS/contracts/authored-route-evidence-bridge-v1.schema.json",
        ),
        (
            "provenance_event_ref",
            "ToS/contracts/provenance-event-v2.schema.json",
        ),
    ];
    let output_map = named_outputs(&mut state, source, plan, &plan_value, &output_specs, limits)?;
    if output_map.len() != output_specs.len() {
        state.schema_checks.clear();
        return result(
            SourceFoundationLab::ZarathustraAuthoredCanonBridge,
            state,
            Vec::new(),
            limits,
        );
    }
    let outputs = plan_value.get("outputs").unwrap_or(&Value::Null);
    let schema_base = state
        .schema_checks
        .len()
        .checked_sub(output_specs.len())
        .ok_or(ItemRefusal::Budget)?;
    for (index, (field, _)) in output_specs.iter().enumerate() {
        let Some((path, value, raw)) = output_map.get(*field) else {
            continue;
        };
        if let Some(check) = state.schema_checks.get_mut(schema_base + index) {
            check.before_issue = state.issues.len();
        }
        source.checkpoint(limits.deadline)?;
        let messages = match *field {
            "anchor_ref" => anchor_v2_semantic_issues(value),
            "raw_layer_ref" | "normalized_layer_ref" => source_text_layer_semantic_issues(value),
            "unit_packet_ref" => {
                source_text_unit_metadata_messages(path, raw, limits, source.generation())?
            }
            "provenance_event_ref" => crate::provenance_rules::semantic_issues(
                value,
                limits.max_issues.saturating_sub(state.issues.len()),
                limits.deadline,
            )?
            .into_iter()
            .map(str::to_owned)
            .collect(),
            _ => Vec::new(),
        };
        for message in messages {
            state.issue(path, message, limits)?;
        }
    }
    for field in [
        "private_raw_content_ref",
        "private_normalized_content_ref",
        "private_operations_ref",
        "private_comparison_ref",
    ] {
        let Some(path) = outputs.get(field).and_then(Value::as_str) else {
            state.issue(
                plan,
                format!("unsafe authored-canon private output: {field}"),
                limits,
            )?;
            continue;
        };
        if !path.contains("local-content/authored-canon-evidence-bridge/") {
            state.issue(
                plan,
                format!("unsafe authored-canon private output: {field}"),
                limits,
            )?;
            continue;
        }
        source.checkpoint(limits.deadline)?;
        check_private_git_ignore(
            &mut state,
            physical,
            path,
            field,
            plan,
            "authored-canon private output is not Git-ignored",
            "physical Git-ignore fact unavailable for authored-canon private output",
            limits,
        )?;
    }
    let scope = plan_value.get("scope").unwrap_or(&Value::Null);
    let ids = plan_value.get("opaque_ids").unwrap_or(&Value::Null);
    let plan_digest = Digest256::of_bytes(&plan_raw).to_hex();
    let scope_fields = [
        "route_ref",
        "work_ref",
        "expression_ref",
        "edition_ref",
        "item_ref",
        "file_ref",
        "file_sha256",
    ];
    if let Some((anchor_path, anchor, _)) = output_map.get("anchor_ref") {
        let selector = anchor
            .pointer("/selector_payload/expression/selector")
            .unwrap_or(&Value::Null);
        if !owner_values_equal(
            anchor.get("anchor_id").unwrap_or(&Value::Null),
            ids.get("anchor_id").unwrap_or(&Value::Null),
        )? || !owner_values_equal(
            anchor.get("passage_id").unwrap_or(&Value::Null),
            ids.get("passage_id").unwrap_or(&Value::Null),
        )? || !owner_values_equal(
            anchor
                .pointer("/target/file_sha256")
                .unwrap_or(&Value::Null),
            scope.get("file_sha256").unwrap_or(&Value::Null),
        )? || !owner_values_equal(
            selector
                .pointer("/state/representation_sha256")
                .unwrap_or(&Value::Null),
            scope.get("file_sha256").unwrap_or(&Value::Null),
        )? || !owner_values_equal(
            selector.pointer("/selector/value").unwrap_or(&Value::Null),
            plan_value
                .pointer("/source_selector/value")
                .unwrap_or(&Value::Null),
        )? || anchor.get("resolution_status").and_then(Value::as_str)
            != Some("mechanically_resolved")
            || anchor.get("review_status").and_then(Value::as_str) != Some("unreviewed")
            || anchor
                .pointer("/publication_boundary/source_text_in_record")
                .and_then(Value::as_bool)
                != Some(false)
            || anchor
                .pointer("/publication_boundary/public_payload_expected")
                .and_then(Value::as_bool)
                != Some(false)
            || anchor
                .pointer("/selector_method/configuration_digest")
                .and_then(Value::as_str)
                != Some(plan_digest.as_str())
        {
            state.issue(
                anchor_path,
                "authored-canon source anchor or authority drifted",
                limits,
            )?;
        }
    }
    let raw_layer = output_map.get("raw_layer_ref").map(|r| &r.1);
    let normalized = output_map.get("normalized_layer_ref").map(|r| &r.1);
    for (field, layer, role, content_ref) in [
        (
            "raw_layer_ref",
            raw_layer,
            "machine_transcription",
            "private_raw_content_ref",
        ),
        (
            "normalized_layer_ref",
            normalized,
            "normalized_text",
            "private_normalized_content_ref",
        ),
    ] {
        if let (Some((path, _, _)), Some(layer)) = (output_map.get(field), layer) {
            let rep = layer.get("representation").unwrap_or(&Value::Null);
            let admission = layer.get("admission").unwrap_or(&Value::Null);
            let anchors = layer
                .pointer("/source_binding/anchors")
                .and_then(Value::as_array);
            if layer.get("layer_role").and_then(Value::as_str) != Some(role)
                || rep.get("content_ref") != outputs.get(content_ref)
                || rep.get("storage").and_then(Value::as_str) != Some("ignored_local")
                || rep.get("tracked_content").and_then(Value::as_bool) != Some(false)
                || rep.get("content_visibility").and_then(Value::as_str) != Some("local_only")
                || rep.get("publication_authorized").and_then(Value::as_bool) != Some(false)
                || admission.get("review_status").and_then(Value::as_str) != Some("unreviewed")
                || admission
                    .get("human_review_performed")
                    .and_then(Value::as_bool)
                    != Some(false)
                || admission
                    .get("human_language_competence")
                    .and_then(Value::as_str)
                    != Some("blocked")
                || admission.get("accepted_uses") != Some(&json!([]))
                || admission
                    .get("routine_human_task_created")
                    .and_then(Value::as_bool)
                    != Some(false)
                || admission
                    .get("promotion_authorized")
                    .and_then(Value::as_bool)
                    != Some(false)
                || anchors.map_or(true, |rows| rows.len() != 1)
                || !owner_values_equal(
                    anchors
                        .and_then(|rows| rows.first())
                        .and_then(|row| row.get("anchor_record_ref"))
                        .unwrap_or(&Value::Null),
                    &output_map
                        .get("anchor_ref")
                        .map(|row| json!(row.0))
                        .unwrap_or(Value::Null),
                )?
                || !owner_values_equal(
                    anchors
                        .and_then(|rows| rows.first())
                        .and_then(|row| row.get("anchor_record_sha256"))
                        .unwrap_or(&Value::Null),
                    &output_map
                        .get("anchor_ref")
                        .map(|row| json!(Digest256::of_bytes(&row.2).to_hex()))
                        .unwrap_or(Value::Null),
                )?
            {
                state.issue(
                    path,
                    format!("{field} private-content or authority drifted"),
                    limits,
                )?;
            }
        }
    }
    if let (Some((normalized_path, normalized_layer, _)), Some((_, raw_layer, raw_bytes))) = (
        output_map.get("normalized_layer_ref"),
        output_map.get("raw_layer_ref"),
    ) {
        let inputs = normalized_layer
            .pointer("/derivation/input_layers")
            .and_then(Value::as_array);
        let input = inputs.and_then(|rows| rows.first());
        let expected_raw_path = output_map
            .get("raw_layer_ref")
            .map(|row| json!(row.0))
            .unwrap_or(Value::Null);
        let expected_raw_digest = Digest256::of_bytes(raw_bytes).to_hex();
        let expected_operations_ref = outputs
            .get("private_operations_ref")
            .unwrap_or(&Value::Null);
        if !owner_values_equal(
            raw_layer.get("layer_id").unwrap_or(&Value::Null),
            ids.get("raw_layer_id").unwrap_or(&Value::Null),
        )? || !owner_values_equal(
            normalized_layer.get("layer_id").unwrap_or(&Value::Null),
            ids.get("normalized_layer_id").unwrap_or(&Value::Null),
        )? || inputs.map_or(0, Vec::len) != 1
            || !owner_values_equal(
                input
                    .and_then(|row| row.get("layer_id"))
                    .unwrap_or(&Value::Null),
                ids.get("raw_layer_id").unwrap_or(&Value::Null),
            )?
            || !owner_values_equal(
                input
                    .and_then(|row| row.get("record_ref"))
                    .unwrap_or(&Value::Null),
                &expected_raw_path,
            )?
            || input
                .and_then(|row| row.get("record_sha256"))
                .and_then(Value::as_str)
                != Some(expected_raw_digest.as_str())
            || !owner_values_equal(
                normalized_layer
                    .pointer("/derivation/change_payload/private_ref")
                    .unwrap_or(&Value::Null),
                expected_operations_ref,
            )?
        {
            state.issue(
                normalized_path,
                "normalized layer derivation closure drifted",
                limits,
            )?;
        }
    }
    if let Some((path, unit, _)) = output_map.get("unit_packet_ref") {
        let mut expected_packet_scope = serde_json::Map::new();
        for field in scope_fields
            .into_iter()
            .filter(|field| *field != "route_ref")
        {
            expected_packet_scope.insert(
                field.into(),
                scope.get(field).cloned().unwrap_or(Value::Null),
            );
        }
        let schemes = unit.get("schemes").and_then(Value::as_array);
        let actual_scheme_ids = Value::Array(
            schemes
                .into_iter()
                .flatten()
                .map(|row| row.get("scheme_id").cloned().unwrap_or(Value::Null))
                .collect(),
        );
        let expected_scheme_ids = json!([
            ids.get("source_scheme_id").unwrap_or(&Value::Null),
            ids.get("authored_scheme_id").unwrap_or(&Value::Null)
        ]);
        if !owner_values_equal(
            unit.get("packet_id").unwrap_or(&Value::Null),
            ids.get("packet_id").unwrap_or(&Value::Null),
        )? || !owner_values_equal(
            unit.get("source_scope").unwrap_or(&Value::Null),
            &Value::Object(expected_packet_scope),
        )? || !owner_values_equal(
            unit.pointer("/source_layer/text_layer_ref")
                .unwrap_or(&Value::Null),
            &output_map
                .get("normalized_layer_ref")
                .map(|row| json!(row.0))
                .unwrap_or(Value::Null),
        )? || !owner_values_equal(
            unit.pointer("/source_layer/text_layer_sha256")
                .unwrap_or(&Value::Null),
            normalized
                .and_then(|layer| layer.pointer("/representation/content_sha256"))
                .unwrap_or(&Value::Null),
        )? || !owner_values_equal(&actual_scheme_ids, &expected_scheme_ids)?
            || unit
                .get("schemes")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
                != 2
            || unit
                .get("anchors")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
                != 47
            || unit
                .get("units")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
                != 24
            || unit
                .get("segmentations")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
                != 2
            || unit.get("reviews") != Some(&json!([]))
            || unit.get("projections") != Some(&json!([]))
            || unit
                .pointer("/rights_and_visibility/publication_authorized")
                .and_then(Value::as_bool)
                != Some(false)
            || unit
                .pointer("/authority_boundary/legacy_bulk_migration_authorized")
                .and_then(Value::as_bool)
                != Some(false)
        {
            state.issue(path, "authored/source segmentation closure drifted", limits)?;
        }
        if let Some(segmentations) = unit.get("segmentations").and_then(Value::as_array)
            && segmentations.len() == 2
        {
            let source = &segmentations[0];
            let authored = &segmentations[1];
            let source_competing =
                json!([ids.get("authored_segmentation_id").unwrap_or(&Value::Null)]);
            let authored_competing =
                json!([ids.get("source_segmentation_id").unwrap_or(&Value::Null)]);
            let authority_widened = segmentations.iter().any(|row| {
                [
                    "source_text_authority",
                    "linguistic_authority",
                    "semantic_authority",
                ]
                .iter()
                .any(|field| row.get(*field).and_then(Value::as_bool) != Some(false))
                    || row.get("review_refs") != Some(&json!([]))
            });
            if !owner_values_equal(
                source.get("segmentation_id").unwrap_or(&Value::Null),
                ids.get("source_segmentation_id").unwrap_or(&Value::Null),
            )? || source.get("status").and_then(Value::as_str)
                != Some("observed_source_structure")
                || !owner_values_equal(
                    source.get("ordered_unit_refs").unwrap_or(&Value::Null),
                    ids.get("source_unit_ids").unwrap_or(&Value::Null),
                )?
                || !owner_values_equal(
                    source
                        .get("competing_segmentation_refs")
                        .unwrap_or(&Value::Null),
                    &source_competing,
                )?
                || !owner_values_equal(
                    authored.get("segmentation_id").unwrap_or(&Value::Null),
                    ids.get("authored_segmentation_id").unwrap_or(&Value::Null),
                )?
                || authored.get("status").and_then(Value::as_str) != Some("proposed")
                || !owner_values_equal(
                    authored.get("ordered_unit_refs").unwrap_or(&Value::Null),
                    ids.get("authored_unit_ids").unwrap_or(&Value::Null),
                )?
                || !owner_values_equal(
                    authored
                        .get("competing_segmentation_refs")
                        .unwrap_or(&Value::Null),
                    &authored_competing,
                )?
                || authority_widened
            {
                state.issue(path, "competing segmentation posture widened", limits)?;
            }
        }
    }
    if let Some((path, bridge, _)) = output_map.get("bridge_ref") {
        let effects = json!({"source_text_accepted":false,"german_competence_attested":false,"translation_accepted":false,"sign_or_concept_promoted":false,"legacy_relation_migrated":false,"graph_projection_admitted":false,"canon_revised":false,"human_task_created":false,"new_publication_authorized":false,"server_transfer_authorized":false});
        let assurance = json!({"source_comparison":"complete-mechanical-normalized-match","authored_review_posture":"legacy-review-records-without-machine-readable-human-attestation","modern_semantic_packet_posture":"not-materialized-by-this-bridge","claim_evidence_closure":false,"graph_admission":false,"current_canon_retained":true,"bulk_migration_authorized":false});
        let mut route_scope = serde_json::Map::new();
        for field in [
            "route_ref",
            "work_ref",
            "expression_ref",
            "edition_ref",
            "item_ref",
            "file_ref",
            "file_sha256",
        ] {
            route_scope.insert(
                field.into(),
                scope.get(field).cloned().unwrap_or(Value::Null),
            );
        }
        if !owner_values_equal(
            bridge.get("route_scope").unwrap_or(&Value::Null),
            &Value::Object(route_scope),
        )? || !owner_values_equal(bridge.get("effects").unwrap_or(&Value::Null), &effects)?
            || !owner_values_equal(bridge.get("assurance").unwrap_or(&Value::Null), &assurance)?
        {
            state.issue(path, "authored-canon bridge authority widened", limits)?;
        }
        for (bridge_field, plan_field) in [
            ("source_anchor", "anchor_ref"),
            ("raw_layer", "raw_layer_ref"),
            ("normalized_layer", "normalized_layer_ref"),
            ("unit_packet", "unit_packet_ref"),
        ] {
            let expected=output_map.get(plan_field).map(|(ref_path,_,raw)|json!({"ref":ref_path,"sha256":Digest256::of_bytes(raw).to_hex()}));
            if !owner_values_equal(
                bridge
                    .pointer(&format!("/source_foundation/{bridge_field}"))
                    .unwrap_or(&Value::Null),
                &expected.unwrap_or(Value::Null),
            )? {
                state.issue(
                    path,
                    format!("bridge source binding drifted: {bridge_field}"),
                    limits,
                )?;
            }
        }
    }
    authored_canon_inventory_and_closure(
        &mut state,
        source,
        plan,
        &plan_value,
        &plan_raw,
        &output_map,
        current_paths,
        limits,
    )?;
    let forbidden_content_keys = [
        "text",
        "source_text",
        "target_text",
        "diplomatic_transcription",
        "dta_normalized",
        "legacy_normalized",
        "operations",
        "opcodes",
    ];
    for (label, value) in [
        (
            "anchor",
            output_map
                .get("anchor_ref")
                .map(|row| &row.1)
                .unwrap_or(&Value::Null),
        ),
        (
            "raw_layer",
            output_map
                .get("raw_layer_ref")
                .map(|row| &row.1)
                .unwrap_or(&Value::Null),
        ),
        (
            "normalized_layer",
            output_map
                .get("normalized_layer_ref")
                .map(|row| &row.1)
                .unwrap_or(&Value::Null),
        ),
        (
            "unit",
            output_map
                .get("unit_packet_ref")
                .map(|row| &row.1)
                .unwrap_or(&Value::Null),
        ),
        (
            "bridge",
            output_map
                .get("bridge_ref")
                .map(|row| &row.1)
                .unwrap_or(&Value::Null),
        ),
        (
            "event",
            output_map
                .get("provenance_event_ref")
                .map(|row| &row.1)
                .unwrap_or(&Value::Null),
        ),
    ] {
        let location = if label == "plan" {
            plan
        } else {
            output_map
                .get(match label {
                    "anchor" => "anchor_ref",
                    "raw_layer" => "raw_layer_ref",
                    "normalized_layer" => "normalized_layer_ref",
                    "unit" => "unit_packet_ref",
                    "bridge" => "bridge_ref",
                    _ => "provenance_event_ref",
                })
                .map(|row| row.0.as_str())
                .unwrap_or(plan)
        };
        if let Some(keys) = private_content_keys(value, &forbidden_content_keys) {
            state.issue(
                location,
                format!("tracked bridge record exposes private content fields: {keys}"),
                limits,
            )?;
        } else if has_absolute_owner_path(value) {
            state.issue(
                location,
                "tracked bridge record exposes an absolute owner-local path",
                limits,
            )?;
        }
    }
    result(
        SourceFoundationLab::ZarathustraAuthoredCanonBridge,
        state,
        Vec::new(),
        limits,
    )
}

#[derive(Debug, Default)]
struct DirectState {
    issues: Vec<(String, String)>,
    schema_checks: Vec<SourceFoundationSchemaCheck>,
    unimplemented: Vec<String>,
    report: Value,
    read_bytes: u64,
    retained_bytes: usize,
    member_digests: std::collections::BTreeMap<String, String>,
}

impl DirectState {
    fn gap(&mut self, message: impl Into<String>, limits: ItemLimits) -> Result<(), ItemRefusal> {
        let message = message.into();
        self.reserve(
            message
                .len()
                .checked_add(std::mem::size_of::<String>() + 32)
                .ok_or(ItemRefusal::Budget)?,
            limits,
        )?;
        self.unimplemented.push(message);
        Ok(())
    }

    fn issue(
        &mut self,
        path: &str,
        message: impl Into<String>,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        if self.issues.len() >= limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        let message = message.into();
        let retained = path
            .len()
            .checked_add(message.len())
            .and_then(|bytes| bytes.checked_add(32))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve(retained, limits)?;
        self.issues.push((path.into(), message));
        Ok(())
    }

    fn reserve(&mut self, bytes: usize, limits: ItemLimits) -> Result<(), ItemRefusal> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .filter(|total| *total <= limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn read(
        &mut self,
        source: &mut (impl LayerFamilySource + ?Sized),
        path: &str,
        limits: ItemLimits,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        source.checkpoint(limits.deadline)?;
        RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("unsafe source-foundation lab path".into()))?;
        let Some(raw) = source.current(path, limits.max_member_bytes, limits.deadline)? else {
            return Ok(None);
        };
        if raw.len() > limits.max_member_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.read_bytes = self
            .read_bytes
            .checked_add(raw.len() as u64)
            .filter(|total| *total <= limits.max_total_bytes)
            .ok_or(ItemRefusal::Budget)?;
        self.reserve(raw.len(), limits)?;
        self.record_digest(path, Digest256::of_bytes(&raw).to_hex(), limits)?;
        source.checkpoint(limits.deadline)?;
        Ok(Some(raw))
    }

    fn record_digest(
        &mut self,
        path: &str,
        digest: String,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        if !self.member_digests.contains_key(path) {
            self.reserve(
                path.len()
                    .checked_add(digest.len())
                    .and_then(|bytes| bytes.checked_add(32))
                    .ok_or(ItemRefusal::Budget)?,
                limits,
            )?;
        }
        self.member_digests.insert(path.to_owned(), digest);
        Ok(())
    }

    fn read_json(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        limits: ItemLimits,
    ) -> Result<Option<(Value, Vec<u8>)>, ItemRefusal> {
        let Some(raw) = self.read(source, path, limits)? else {
            self.issue(path, "file is missing", limits)?;
            return Ok(None);
        };
        match serde_json::from_slice::<Value>(&raw) {
            Ok(value) if value.is_object() => {
                self.reserve_decoded(&value, limits)?;
                Ok(Some((value, raw)))
            }
            Ok(_) => {
                self.issue(path, "JSON root must be an object", limits)?;
                Ok(None)
            }
            Err(error) => {
                self.issue(path, json_parse_owner_message(&error), limits)?;
                Ok(None)
            }
        }
    }

    fn reserve_decoded(&mut self, value: &Value, limits: ItemLimits) -> Result<(), ItemRefusal> {
        self.reserve(estimate_value_storage(value)?, limits)
    }

    fn clone_value_charged(
        &mut self,
        value: &Value,
        limits: ItemLimits,
    ) -> Result<Value, ItemRefusal> {
        self.reserve_decoded(value, limits)?;
        Ok(value.clone())
    }

    fn reserve_value_clones(
        &mut self,
        value: &Value,
        copies: usize,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        let per_copy = estimate_value_storage(value)?;
        let retained = per_copy.checked_mul(copies).ok_or(ItemRefusal::Budget)?;
        self.reserve(retained, limits)
    }

    fn serialize_json(
        &mut self,
        value: &Value,
        limits: ItemLimits,
    ) -> Result<Vec<u8>, ItemRefusal> {
        let remaining_state = limits
            .max_state_bytes
            .checked_sub(self.retained_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let bounded_cap = remaining_state.min(limits.max_member_bytes);
        let encoded_len = bounded_json_encoded_len(value, bounded_cap)?;
        self.reserve(encoded_len, limits)?;
        write_bounded_json(value, bounded_cap, encoded_len)
    }

    fn canonical_digest(
        &mut self,
        value: &Value,
        limits: ItemLimits,
    ) -> Result<String, ItemRefusal> {
        if std::time::Instant::now() >= limits.deadline {
            return Err(ItemRefusal::Deadline);
        }
        let remaining_state = limits
            .max_state_bytes
            .checked_sub(self.retained_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let cap = remaining_state.min(limits.max_member_bytes);
        let encoded_len = canonical_json_encoded_len(value, cap)?;
        self.reserve(encoded_len, limits)?;
        let bytes = canonical_json_bytes(value, encoded_len)?;
        if std::time::Instant::now() >= limits.deadline {
            return Err(ItemRefusal::Deadline);
        }
        self.reserve(
            estimate_text_storage(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )?,
            limits,
        )?;
        Ok(Digest256::of_bytes(&bytes).to_hex())
    }

    fn set_last_schema_message_prefix(
        &mut self,
        prefix: &str,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        if self.schema_checks.is_empty() {
            return Err(ItemRefusal::Budget);
        }
        self.reserve(estimate_text_storage(prefix)?, limits)?;
        if let Some(check) = self.schema_checks.last_mut() {
            check.schema_message_prefix = Some(prefix.to_owned());
        }
        Ok(())
    }

    fn set_last_rejection_reasons_slot(
        &mut self,
        variant_index: usize,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        let digits = if variant_index == 0 {
            1
        } else {
            variant_index.ilog10() as usize + 1
        };
        let estimated = "/variants/"
            .len()
            .checked_add(digits)
            .and_then(|bytes| bytes.checked_add("/rejection_reasons".len()))
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(32))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve(estimated, limits)?;
        let Some(check) = self.schema_checks.last_mut() else {
            return Err(ItemRefusal::Budget);
        };
        check.rejection_reasons_slot = Some(format!("/variants/{variant_index}/rejection_reasons"));
        Ok(())
    }

    fn schema_check(
        &mut self,
        location: &str,
        contract: &str,
        instance: &Value,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        self.schema_check_with_result(
            location, contract, instance, None, None, None, None, None, limits,
        )
    }

    fn schema_check_with_control(
        &mut self,
        location: &str,
        contract: &str,
        instance: &Value,
        negative_control: Option<&str>,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        let report_slot = negative_control.map(|name| format!("/negative_controls/{name}"));
        self.schema_check_with_result(
            location,
            contract,
            instance,
            negative_control,
            negative_control.map(|_| false),
            None,
            report_slot.as_deref(),
            negative_control.map(|name| format!("negative control was not rejected: {name}")),
            limits,
        )
    }

    fn schema_check_with_result(
        &mut self,
        location: &str,
        contract: &str,
        instance: &Value,
        negative_control: Option<&str>,
        expected_valid: Option<bool>,
        semantic_rejected: Option<bool>,
        report_slot: Option<&str>,
        mismatch_message: Option<String>,
        limits: ItemLimits,
    ) -> Result<(), ItemRefusal> {
        let remaining_state = limits
            .max_state_bytes
            .checked_sub(self.retained_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let bounded_cap = remaining_state.min(limits.max_member_bytes);
        let _encoded_len = bounded_json_encoded_len(instance, bounded_cap)?;
        let decoded_clone = estimate_value_storage(instance)?;
        let mut retained = estimate_text_storage(location)?
            .checked_add(estimate_text_storage(contract)?)
            .ok_or(ItemRefusal::Budget)?;
        for text in [negative_control, report_slot]
            .into_iter()
            .flatten()
            .chain(mismatch_message.as_deref())
        {
            retained = retained
                .checked_add(estimate_text_storage(text)?)
                .ok_or(ItemRefusal::Budget)?;
        }
        retained = retained
            .checked_add(decoded_clone)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<SourceFoundationSchemaCheck>()))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve(retained, limits)?;
        self.schema_checks.push(SourceFoundationSchemaCheck {
            before_issue: self.issues.len(),
            location: location.into(),
            contract: contract.into(),
            instance: instance.clone(),
            negative_control: negative_control.map(str::to_owned),
            expected_valid,
            expected_rejected: negative_control.map(|_| true),
            semantic_rejected,
            report_slot: report_slot.map(str::to_owned),
            rejection_reasons_slot: None,
            schema_message_prefix: None,
            mismatch_message,
        });
        Ok(())
    }
}

fn checked_binding(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    manifest_path: &str,
    value: &Value,
    label: &str,
    retained_ok: bool,
    limits: ItemLimits,
) -> Result<Option<String>, ItemRefusal> {
    let Some(binding) = value.get(label).and_then(Value::as_object) else {
        state.issue(manifest_path, format!("{label} binding is absent"), limits)?;
        return Ok(None);
    };
    let Some(path) = binding.get("ref").and_then(Value::as_str) else {
        state.issue(manifest_path, format!("{label} binding is absent"), limits)?;
        return Ok(None);
    };
    let Some(expected) = binding.get("sha256").and_then(Value::as_str) else {
        state.issue(manifest_path, format!("{label} binding is absent"), limits)?;
        return Ok(Some(path.to_owned()));
    };
    let actual = if retained_ok {
        RelativePath::parse(path).map_err(|_| {
            ItemRefusal::Unsupported("unsafe source-foundation binding path".into())
        })?;
        source.checkpoint(limits.deadline)?;
        let raw = source.recorded(path, expected, limits.max_member_bytes, limits.deadline)?;
        if let Some(raw) = raw.as_ref() {
            if raw.len() > limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            state.read_bytes = state
                .read_bytes
                .checked_add(raw.len() as u64)
                .filter(|total| *total <= limits.max_total_bytes)
                .ok_or(ItemRefusal::Budget)?;
            state.reserve(raw.len(), limits)?;
            state.record_digest(path, Digest256::of_bytes(raw).to_hex(), limits)?;
        }
        source.checkpoint(limits.deadline)?;
        raw
    } else {
        state.read(source, path, limits)?
    };
    match actual {
        Some(raw) if Digest256::of_bytes(&raw).to_hex() == expected => {}
        _ => state.issue(manifest_path, format!("{label} binding drifted"), limits)?,
    }
    Ok(Some(path.to_owned()))
}

fn manifest(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    path: &str,
    schema_version: &str,
    _authority_posture: &str,
    _authority_limits: &Value,
    limits: ItemLimits,
) -> Result<Option<Value>, ItemRefusal> {
    let Some((value, _)) = state.read_json(source, path, limits)? else {
        return Ok(None);
    };
    if !value.is_object() {
        state.issue(path, "JSON root must be an object", limits)?;
        return Ok(None);
    }
    if value.get("schema_version").and_then(Value::as_str) != Some(schema_version) {
        state.issue(path, manifest_version_message(schema_version), limits)?;
    }
    Ok(Some(value))
}

fn manifest_authority(
    state: &mut DirectState,
    path: &str,
    manifest: &Value,
    schema_version: &str,
    authority_posture: &str,
    authority_limits: &Value,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    if manifest.get("authority_posture").and_then(Value::as_str) != Some(authority_posture) {
        state.issue(path, manifest_posture_message(schema_version), limits)?;
    }
    let limits_match = match manifest.get("authority_limits") {
        Some(actual) => python_equal(actual, authority_limits)?,
        None => false,
    };
    if !limits_match {
        state.issue(path, manifest_limits_message(schema_version), limits)?;
    }
    Ok(())
}

fn manifest_version_message(schema_version: &str) -> &'static str {
    match schema_version {
        "tos_provenance_event_v2_lab_v1" => "unexpected provenance v2 lab manifest version",
        "tos_semantic_annotation_v2_lab_manifest_v1" => {
            "unexpected semantic annotation lab manifest version"
        }
        "tos_translation_alignment_v1_lab_manifest_v1" => {
            "unexpected translation alignment lab manifest version"
        }
        "tos_source_text_unit_v1_lab_manifest_v1" => {
            "unexpected source text unit lab manifest version"
        }
        _ => "unexpected laboratory manifest version",
    }
}

fn manifest_posture_message(schema_version: &str) -> &'static str {
    match schema_version {
        "tos_provenance_event_v2_lab_v1" => "provenance v2 lab authority posture widened",
        "tos_semantic_annotation_v2_lab_manifest_v1" => {
            "semantic annotation lab authority posture widened"
        }
        "tos_translation_alignment_v1_lab_manifest_v1" => {
            "translation alignment lab authority posture widened"
        }
        "tos_source_text_unit_v1_lab_manifest_v1" => {
            "source text unit lab authority posture widened"
        }
        _ => "laboratory authority posture widened",
    }
}

fn manifest_limits_message(schema_version: &str) -> &'static str {
    match schema_version {
        "tos_provenance_event_v2_lab_v1" => "provenance v2 lab authority limits widened",
        "tos_semantic_annotation_v2_lab_manifest_v1" => {
            "semantic annotation lab authority limits widened"
        }
        "tos_translation_alignment_v1_lab_manifest_v1" => {
            "translation alignment lab authority limits widened"
        }
        "tos_source_text_unit_v1_lab_manifest_v1" => {
            "source text unit lab authority limits widened"
        }
        _ => "laboratory authority limits widened",
    }
}

fn binding(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    manifest_path: &str,
    value: &Value,
    field: &str,
    retained_ok: bool,
    limits: ItemLimits,
) -> Result<Option<String>, ItemRefusal> {
    checked_binding(
        state,
        source,
        manifest_path,
        value,
        field,
        retained_ok,
        limits,
    )
}

fn check_ref(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    location: &str,
    path: &str,
    expected_sha256: Option<&str>,
    limits: ItemLimits,
) -> Result<Option<Vec<u8>>, ItemRefusal> {
    let Some(raw) = state.read(source, path, limits)? else {
        state.issue(location, format!("file is missing: {path}"), limits)?;
        return Ok(None);
    };
    if expected_sha256.is_some_and(|expected| Digest256::of_bytes(&raw).to_hex() != expected) {
        state.issue(location, format!("file digest drifted: {path}"), limits)?;
    }
    Ok(Some(raw))
}

fn check_packet_variants(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    manifest_path: &str,
    manifest: &Value,
    ref_field: &str,
    digest_field: &str,
    contract: &str,
    metadata_profile: Option<&str>,
    label: &str,
    mut inspect: impl FnMut(
        &mut DirectState,
        &mut dyn LayerFamilySource,
        &str,
        &str,
        &Value,
        &[u8],
        &Value,
    ) -> Result<Value, ItemRefusal>,
    limits: ItemLimits,
) -> Result<Vec<Value>, ItemRefusal> {
    let Some(variants) = manifest.get("variants").and_then(Value::as_array) else {
        state.issue(
            manifest_path,
            format!("{label} variants are not a list"),
            limits,
        )?;
        return Ok(Vec::new());
    };
    let ids: Vec<_> = variants
        .iter()
        .filter_map(|row| row.get("variant_id").and_then(Value::as_str))
        .collect();
    if ids != ["A", "B", "C"] {
        state.issue(manifest_path, variant_order_message(label), limits)?;
    }
    let mut output = Vec::new();
    for row in variants {
        source.checkpoint(limits.deadline)?;
        let Some(variant_id) = row.get("variant_id").and_then(Value::as_str) else {
            state.issue(manifest_path, variant_identity_message(label), limits)?;
            continue;
        };
        let Some(path) = row.get(ref_field).and_then(Value::as_str) else {
            state.issue(manifest_path, variant_identity_message(label), limits)?;
            continue;
        };
        let Some(raw) = state.read(source, path, limits)? else {
            state.issue(path, variant_fixity_message(label), limits)?;
            continue;
        };
        if Digest256::of_bytes(&raw).to_hex()
            != row.get(digest_field).and_then(Value::as_str).unwrap_or("")
        {
            state.issue(path, variant_fixity_message(label), limits)?;
        }
        let value: Value = match serde_json::from_slice::<Value>(&raw) {
            Ok(value) if value.is_object() => value,
            Ok(_) => {
                state.issue(path, "JSON root must be an object", limits)?;
                continue;
            }
            Err(error) => {
                state.issue(path, json_parse_owner_message(&error), limits)?;
                continue;
            }
        };
        state.reserve_decoded(&value, limits)?;
        let expected_valid = row.get("expected_schema_valid").and_then(Value::as_bool);
        let report_slot =
            expected_valid.map(|_| format!("/variants/{}/schema_valid", output.len()));
        state.schema_check_with_result(
            path,
            contract,
            &value,
            None,
            expected_valid,
            None,
            report_slot.as_deref(),
            expected_valid.map(|_| "schema result differs from frozen A/B/C expectation".into()),
            limits,
        )?;
        if matches!(
            label,
            "semantic" | "translation alignment" | "source text unit"
        ) && expected_valid.is_some()
        {
            state.set_last_rejection_reasons_slot(output.len(), limits)?;
        }
        if let Some(profile) = metadata_profile {
            append_text_metadata(state, source, path, &raw, profile, limits)?;
        }
        let inspected = inspect(state, source, variant_id, path, &value, &raw, row)?;
        source.checkpoint(limits.deadline)?;
        output.push(inspected);
    }
    Ok(output)
}

fn variant_order_message(label: &str) -> &'static str {
    match label {
        "provenance" => "provenance variants must be ordered exactly A, B, C",
        "semantic" => "semantic variants must be ordered exactly A, B, C",
        "translation alignment" => "translation alignment variants must be ordered exactly A, B, C",
        "source text unit" => "source text unit variants must be ordered exactly A, B, C",
        _ => "variants must be ordered exactly A, B, C",
    }
}

fn variant_identity_message(label: &str) -> &'static str {
    match label {
        "provenance" => "provenance variant identity is invalid",
        "semantic" => "semantic variant is not an object",
        "translation alignment" => "translation alignment variant identity is invalid",
        "source text unit" => "source text unit variant identity is invalid",
        _ => "variant identity is invalid",
    }
}

fn variant_fixity_message(label: &str) -> &'static str {
    match label {
        "provenance" => "provenance event record fixity drifted",
        "semantic" => "semantic variant packet fixity drifted",
        "translation alignment" => "translation alignment variant fixity drifted",
        "source text unit" => "source text unit variant fixity drifted",
        _ => "variant fixity drifted",
    }
}

fn named_outputs(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    plan_path: &str,
    plan: &Value,
    outputs: &[(&str, &str)],
    limits: ItemLimits,
) -> Result<std::collections::BTreeMap<String, (String, Value, Vec<u8>)>, ItemRefusal> {
    let mut result = std::collections::BTreeMap::new();
    state.reserve(
        std::mem::size_of::<std::collections::BTreeMap<String, (String, Value, Vec<u8>)>>(),
        limits,
    )?;
    for (label, contract) in outputs {
        let Some(path) = plan
            .pointer("/outputs")
            .and_then(|value| value.get(*label))
            .and_then(Value::as_str)
        else {
            state.issue(
                plan_path,
                format!("{label} output reference is absent"),
                limits,
            )?;
            continue;
        };
        let Some((value, raw)) = state.read_json(source, path, limits)? else {
            continue;
        };
        state.schema_check_with_result(
            path, contract, &value, None, None, None, None, None, limits,
        )?;
        if let Some(prefix) = named_output_schema_prefix(label) {
            state.set_last_schema_message_prefix(prefix, limits)?;
        }
        state.reserve(
            estimate_text_storage(label)?
                .checked_add(estimate_text_storage(path)?)
                .and_then(|bytes| {
                    bytes
                        .checked_add(std::mem::size_of::<(String, (String, Value, Vec<u8>))>() + 64)
                })
                .ok_or(ItemRefusal::Budget)?,
            limits,
        )?;
        result.insert((*label).to_owned(), (path.to_owned(), value, raw));
    }
    Ok(result)
}

fn named_output_schema_prefix(field: &str) -> Option<&'static str> {
    match field {
        "source_anchor_ref" => Some("anchor schema: "),
        "source_text_layer_ref" => Some("layer schema: "),
        "source_text_unit_packet_ref" => Some("unit schema: "),
        "collation_packet_ref" => Some("collation schema: "),
        "anchor_ref" => Some("anchor schema: "),
        "raw_layer_ref" => Some("raw_layer schema: "),
        "normalized_layer_ref" => Some("normalized_layer schema: "),
        "unit_packet_ref" => Some("unit schema: "),
        "bridge_ref" => Some("bridge schema: "),
        "provenance_event_ref" => Some("event schema: "),
        _ => None,
    }
}

fn exact_value_issue(
    state: &mut DirectState,
    path: &str,
    actual: Option<&Value>,
    expected: Value,
    message: &str,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    if !owner_values_equal(actual.unwrap_or(&Value::Null), &expected)? {
        state.issue(path, message, limits)?;
    }
    Ok(())
}

fn metadata_leak(
    value: &Value,
    forbidden_keys: &[&str],
    absolute_prefixes: &[&str],
) -> Option<String> {
    let mut stack = vec![value];
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(map) => {
                if let Some(key) = forbidden_keys.iter().find(|key| map.contains_key(**key)) {
                    return Some((*key).to_owned());
                }
                stack.extend(map.values());
            }
            Value::Array(rows) => stack.extend(rows),
            Value::String(text)
                if absolute_prefixes
                    .iter()
                    .any(|prefix| text.starts_with(prefix)) =>
            {
                return Some("absolute owner-local path".into());
            }
            _ => {}
        }
    }
    None
}

fn authored_canon_inventory_and_closure(
    state: &mut DirectState,
    source: &mut impl LayerFamilySource,
    plan_path: &str,
    plan: &Value,
    plan_raw: &[u8],
    outputs: &std::collections::BTreeMap<String, (String, Value, Vec<u8>)>,
    current_paths: &dyn SourceFoundationDefaultPaths,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    let authored = plan.get("authored_surfaces").unwrap_or(&Value::Null);
    let bridge = outputs
        .get("bridge_ref")
        .map(|row| &row.1)
        .unwrap_or(&Value::Null);
    let bridge_path = outputs
        .get("bridge_ref")
        .map(|row| row.0.as_str())
        .unwrap_or(plan_path);
    let bridge_authored = bridge.get("authored_surfaces").unwrap_or(&Value::Null);
    let mut current_hashes = std::collections::BTreeMap::<String, String>::new();
    for (bridge_field, plan_field) in [
        ("source_node", "source_node_ref"),
        ("alignment_witness", "alignment_witness_ref"),
        ("relation_pack", "relation_pack_ref"),
    ] {
        let reference = authored.get(plan_field).and_then(Value::as_str);
        let Some(reference) = reference else {
            state.issue(
                plan_path,
                format!("authored surface is absent: {plan_field}"),
                limits,
            )?;
            continue;
        };
        if !current_paths.contains(reference)? {
            state.issue(
                plan_path,
                format!("authored surface is absent: {plan_field}"),
                limits,
            )?;
            continue;
        }
        let Some(raw) = state.read(source, reference, limits)? else {
            state.issue(
                plan_path,
                format!("authored surface is absent: {plan_field}"),
                limits,
            )?;
            continue;
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        current_hashes.insert(reference.to_owned(), digest.clone());
        let expected = json!({"ref":reference,"sha256":digest});
        if !owner_values_equal(
            bridge_authored.get(bridge_field).unwrap_or(&Value::Null),
            &expected,
        )? {
            state.issue(
                bridge_path,
                format!("authored surface binding drifted: {bridge_field}"),
                limits,
            )?;
        }
    }

    let review_refs = authored
        .get("review_record_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut expected_review_bindings = Vec::new();
    for value in &review_refs {
        let Some(reference) = value.as_str() else {
            state.issue(plan_path, "legacy review record is absent: ", limits)?;
            continue;
        };
        if !current_paths.contains(reference)? {
            state.issue(
                plan_path,
                format!("legacy review record is absent: {reference}"),
                limits,
            )?;
            continue;
        }
        let Some(raw) = state.read(source, reference, limits)? else {
            state.issue(
                plan_path,
                format!("legacy review record is absent: {reference}"),
                limits,
            )?;
            continue;
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        current_hashes.insert(reference.to_owned(), digest.clone());
        expected_review_bindings.push(json!({"ref":reference,"sha256":digest}));
    }
    if !owner_values_equal(
        bridge_authored
            .get("review_records")
            .unwrap_or(&Value::Null),
        &Value::Array(expected_review_bindings),
    )? {
        state.issue(bridge_path, "legacy review-record binding drifted", limits)?;
    }

    let source_node_ref = authored.get("source_node_ref").and_then(Value::as_str);
    let source_node = if let Some(reference) = source_node_ref {
        if current_paths.contains(reference)? {
            state
                .read_json(source, reference, limits)?
                .map(|(value, raw)| {
                    current_hashes.insert(reference.to_owned(), Digest256::of_bytes(&raw).to_hex());
                    value
                })
        } else {
            None
        }
    } else {
        None
    };
    let mut expected_witnesses = Vec::new();
    if let Some(source_node) = source_node.as_ref() {
        if let Some(witnesses) = source_node
            .get("language_witnesses")
            .and_then(Value::as_array)
        {
            for witness in witnesses {
                let language = witness.get("language").cloned().unwrap_or(Value::Null);
                let empty_segments = Value::Array(Vec::new());
                let segments = witness.get("segments").unwrap_or(&empty_segments);
                let segment_count = segments.as_array().map_or(0, Vec::len);
                let content_sha256 = state.canonical_digest(segments, limits)?;
                let language_code = language.as_str().unwrap_or("");
                expected_witnesses.push(json!({
                    "language":language,
                    "legacy_role":witness.get("role").cloned().unwrap_or(Value::Null),
                    "segment_count":segment_count,
                    "content_sha256":content_sha256,
                    "authorship_posture":if language_code == "de" {"nietzsche-source-witness-role"} else {"dionysus-authored-translation-witness"},
                    "current_assurance":if language_code == "de" {"mechanically-crosswalked-not-philologically-accepted"} else {"legacy-authored-translation-not-modernly-reviewed"},
                    "modern_review_refs":[],
                }));
            }
        }
    }
    if !owner_values_equal(
        bridge.get("witness_roles").unwrap_or(&Value::Null),
        &Value::Array(expected_witnesses),
    )? {
        state.issue(
            bridge_path,
            "authored witness role or digest drifted",
            limits,
        )?;
    }

    let ids = plan.get("opaque_ids").unwrap_or(&Value::Null);
    let expected_segment_ids: Vec<String> =
        (1..=12).map(|index| format!("seg.1.1.1.{index}")).collect();
    let crosswalk = bridge.get("segment_crosswalk").and_then(Value::as_array);
    let mut crosswalk_drifted = crosswalk.map_or(true, |rows| rows.len() != 12);
    if let Some(rows) = crosswalk {
        let unit_ids = ids
            .get("authored_unit_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let anchor_ids = ids
            .get("authored_anchor_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (index, row) in rows.iter().enumerate() {
            let expected_legacy = expected_segment_ids.get(index).map(String::as_str);
            let expected_unit = unit_ids.get(index);
            let expected_anchor = anchor_ids.get(index);
            if row.get("legacy_segment_id").and_then(Value::as_str) != expected_legacy
                || !owner_values_equal(
                    row.get("source_unit_ref").unwrap_or(&Value::Null),
                    expected_unit.unwrap_or(&Value::Null),
                )?
                || !owner_values_equal(
                    row.get("source_anchor_ref").unwrap_or(&Value::Null),
                    expected_anchor.unwrap_or(&Value::Null),
                )?
                || row.get("normalized_match").and_then(Value::as_bool) != Some(true)
                || !owner_values_equal(
                    row.get("dta_normalized_sha256").unwrap_or(&Value::Null),
                    row.get("legacy_normalized_sha256").unwrap_or(&Value::Null),
                )?
                || row.get("modern_review_refs") != Some(&json!([]))
            {
                crosswalk_drifted = true;
                break;
            }
        }
    }
    if crosswalk_drifted {
        state.issue(
            bridge_path,
            "twelve-segment mechanical crosswalk drifted",
            limits,
        )?;
    }

    let mut route_node_refs = std::collections::BTreeSet::<String>::new();
    if let Some(reference) = source_node_ref {
        route_node_refs.insert(reference.to_owned());
    }
    let family_roots = [
        "analogy",
        "event",
        "lineage",
        "principle",
        "state",
        "support",
        "synthesis",
    ];
    for family in family_roots {
        let prefix =
            format!("ToS/canon/{family}/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/");
        current_paths.for_each_path(&mut |reference| {
            source.checkpoint(limits.deadline)?;
            if reference.starts_with(&prefix) && reference.ends_with("/node.json") {
                if !route_node_refs.contains(reference) {
                    state.reserve(
                        reference.len().checked_add(32).ok_or(ItemRefusal::Budget)?,
                        limits,
                    )?;
                    route_node_refs.insert(reference.to_owned());
                }
            }
            Ok(())
        })?;
    }
    for reference in [
        "ToS/canon/concept/becoming/node.json",
        "ToS/canon/concept/overcoming/node.json",
    ] {
        if route_node_refs.insert(reference.to_owned()) {
            state.reserve(
                reference.len().checked_add(32).ok_or(ItemRefusal::Budget)?,
                limits,
            )?;
        }
    }
    let mut route_node_rows = Vec::new();
    let mut route_node_ids = Vec::new();
    let mut node_kind_counts = std::collections::BTreeMap::<String, usize>::new();
    for reference in &route_node_refs {
        let Some((payload, raw)) = state.read_json(source, reference, limits)? else {
            continue;
        };
        let (Some(node_id), Some(node_type)) = (
            payload.get("node_id").and_then(Value::as_str),
            payload.get("node_type").and_then(Value::as_str),
        ) else {
            state.issue(
                reference,
                "authored route node identity or type is absent",
                limits,
            )?;
            continue;
        };
        route_node_ids.push(node_id.to_owned());
        *node_kind_counts.entry(node_type.to_owned()).or_default() += 1;
        route_node_rows.push(json!({
            "ref":reference,
            "sha256":Digest256::of_bytes(&raw).to_hex(),
            "node_id":node_id,
            "node_type":node_type,
        }));
    }
    let route_node_count = route_node_rows.len();
    let route_node_digest = state.canonical_digest(&Value::Array(route_node_rows), limits)?;
    let relation_ref = authored.get("relation_pack_ref").and_then(Value::as_str);
    let mut relation_rows = Vec::<Vec<String>>::new();
    let mut relation_ids = Vec::<String>::new();
    let mut relation_kind_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut relation_ids_digest = Digest256::of_bytes(b"\n").to_hex();
    let mut legacy_locator_count = 0usize;
    let current_relation_ref = match relation_ref {
        Some(reference) if current_paths.contains(reference)? => Some(reference),
        _ => None,
    };
    if let Some(reference) = current_relation_ref {
        if let Some(raw) = state.read(source, reference, limits)? {
            if let Some((headers, rows)) = parse_csv_records(&raw) {
                let edge_index = headers.iter().rposition(|header| header == "edge_id");
                let kind_index = headers.iter().rposition(|header| header == "edge_kind");
                let locator_index = headers
                    .iter()
                    .rposition(|header| header == "anchor_segment_ids");
                let known_locators: std::collections::BTreeSet<_> =
                    expected_segment_ids.iter().map(String::as_str).collect();
                for row in &rows {
                    let edge_id = edge_index
                        .and_then(|index| row.get(index))
                        .cloned()
                        .unwrap_or_default();
                    let edge_kind = kind_index
                        .and_then(|index| row.get(index))
                        .cloned()
                        .unwrap_or_default();
                    let locators: std::collections::BTreeSet<_> = locator_index
                        .and_then(|index| row.get(index))
                        .map(|value| value.split('|').filter(|value| !value.is_empty()).collect())
                        .unwrap_or_default();
                    if !locators.is_empty() && locators.is_subset(&known_locators) {
                        legacy_locator_count += 1;
                    } else {
                        state.issue(
                            reference,
                            format!(
                                "authored relation has invalid legacy segment locators: {edge_id}"
                            ),
                            limits,
                        )?;
                    }
                    relation_ids.push(edge_id);
                    *relation_kind_counts.entry(edge_kind).or_default() += 1;
                }
                let ids_text = format!("{}\n", relation_ids.join("\n"));
                relation_ids_digest = Digest256::of_bytes(ids_text.as_bytes()).to_hex();
                relation_rows = rows;
            } else {
                state.issue(
                    plan_path,
                    "cannot read authored relation pack: malformed CSV",
                    limits,
                )?;
            }
        } else {
            state.issue(
                plan_path,
                format!("cannot read authored relation pack: {reference}"),
                limits,
            )?;
        }
    } else {
        state.issue(
            plan_path,
            "cannot read authored relation pack: current member is absent",
            limits,
        )?;
    }
    let inventory = bridge.get("canon_inventory").unwrap_or(&Value::Null);
    let route_node_ids_unique = route_node_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == route_node_ids.len();
    let relation_ids_unique = relation_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == relation_ids.len();
    let node_counts_value =
        serde_json::to_value(node_kind_counts).map_err(|_| ItemRefusal::Budget)?;
    let relation_counts_value =
        serde_json::to_value(relation_kind_counts).map_err(|_| ItemRefusal::Budget)?;
    if route_node_count != 92
        || !route_node_ids_unique
        || !owner_values_equal(
            inventory.get("node_count").unwrap_or(&Value::Null),
            &json!(route_node_count),
        )?
        || !owner_values_equal(
            inventory.get("node_kind_counts").unwrap_or(&Value::Null),
            &node_counts_value,
        )?
        || inventory.get("node_records_sha256").and_then(Value::as_str)
            != Some(route_node_digest.as_str())
        || !owner_values_equal(
            inventory.get("relation_count").unwrap_or(&Value::Null),
            &json!(relation_rows.len()),
        )?
        || !owner_values_equal(
            inventory.get("relation_count").unwrap_or(&Value::Null),
            &json!(125),
        )?
        || !relation_ids_unique
        || !owner_values_equal(
            inventory
                .get("relation_kind_counts")
                .unwrap_or(&Value::Null),
            &relation_counts_value,
        )?
        || inventory.get("relation_ids_sha256").and_then(Value::as_str)
            != Some(relation_ids_digest.as_str())
        || !owner_values_equal(
            inventory
                .get("relations_with_legacy_segment_locators")
                .unwrap_or(&Value::Null),
            &json!(legacy_locator_count),
        )?
        || legacy_locator_count != 125
        || !owner_values_equal(
            inventory
                .get("relations_with_modern_claim_refs")
                .unwrap_or(&Value::Null),
            &json!(0),
        )?
        || !owner_values_equal(
            inventory
                .get("relations_with_modern_evidence_refs")
                .unwrap_or(&Value::Null),
            &json!(0),
        )?
        || !owner_values_equal(
            inventory
                .get("legacy_review_record_count")
                .unwrap_or(&Value::Null),
            &json!(review_refs.len()),
        )?
        || !owner_values_equal(
            inventory
                .get("machine_readable_human_attestation_count")
                .unwrap_or(&Value::Null),
            &json!(0),
        )?
    {
        state.issue(
            bridge_path,
            "authored canon inventory or modern closure drifted",
            limits,
        )?;
    }

    let event = outputs
        .get("provenance_event_ref")
        .map(|row| &row.1)
        .unwrap_or(&Value::Null);
    let event_path = outputs
        .get("provenance_event_ref")
        .map(|row| row.0.as_str())
        .unwrap_or(plan_path);
    let expected_argv = json!([
        "python",
        "scripts/build_zarathustra_authored_canon_evidence_bridge.py",
        "--build",
        "--local-input-root",
        "/srv/AbyssOS/Tree-of-Sophia",
        "--local-output-root",
        "/srv/AbyssOS/Tree-of-Sophia",
    ]);
    let expected_capture = json!({
        "disclosure":"withheld_digest_only",
        "argv":null,
        "argv_sha256":state.canonical_digest(&expected_argv, limits)?,
        "withholding_reason":"The exact build argv contains the owner-local absolute corpus root; its digest is retained without embedding a non-portable machine path."
    });
    let event_inputs = event.pointer("/entities/inputs").and_then(Value::as_array);
    let mut actual_input_refs = std::collections::BTreeSet::new();
    if let Some(rows) = event_inputs {
        for row in rows {
            if let Some(reference) = row.get("entity_ref").and_then(Value::as_str) {
                actual_input_refs.insert(reference.to_owned());
            }
        }
    }
    let scope = plan.get("scope").unwrap_or(&Value::Null);
    let authored_source_refs = [
        authored.get("source_node_ref").and_then(Value::as_str),
        authored
            .get("alignment_witness_ref")
            .and_then(Value::as_str),
        authored.get("relation_pack_ref").and_then(Value::as_str),
    ];
    let mut expected_event_inputs = std::collections::BTreeSet::<String>::new();
    if let Some(source_ref) = scope.get("source_relative_ref").and_then(Value::as_str) {
        expected_event_inputs.insert(source_ref.to_owned());
    }
    expected_event_inputs.insert(plan_path.to_owned());
    for reference in [
        plan.get("research_ref").and_then(Value::as_str),
        scope.get("rights_ref").and_then(Value::as_str),
        authored_source_refs[0],
        authored_source_refs[1],
        authored_source_refs[2],
    ] {
        if let Some(reference) = reference {
            expected_event_inputs.insert(reference.to_owned());
        }
    }
    for reference in &review_refs {
        if let Some(reference) = reference.as_str() {
            expected_event_inputs.insert(reference.to_owned());
        }
    }
    let mut closure_ok = expected_event_inputs == actual_input_refs;
    let source_input_ref = scope.get("source_relative_ref").and_then(Value::as_str);
    let source_input = source_input_ref.and_then(|reference| {
        event_inputs.and_then(|rows| {
            rows.iter()
                .rev()
                .find(|row| row.get("entity_ref").and_then(Value::as_str) == Some(reference))
        })
    });
    if source_input
        .and_then(|row| row.get("sha256"))
        .and_then(Value::as_str)
        != scope.get("file_sha256").and_then(Value::as_str)
        || source_input
            .and_then(|row| row.get("availability"))
            .and_then(Value::as_str)
            != Some("owner_local")
        || source_input
            .and_then(|row| row.get("content_disclosure"))
            .and_then(Value::as_str)
            != Some("private_content")
    {
        closure_ok = false;
    }
    for reference in expected_event_inputs
        .iter()
        .filter(|reference| Some(reference.as_str()) != source_input_ref)
    {
        if !current_paths.contains(reference)? {
            closure_ok = false;
            continue;
        }
        let expected_digest = if reference == plan_path {
            Some(Digest256::of_bytes(plan_raw).to_hex())
        } else if let Some(digest) = current_hashes.get(reference) {
            Some(digest.clone())
        } else if let Some((_, _, raw)) = outputs.values().find(|row| row.0 == *reference) {
            Some(Digest256::of_bytes(raw).to_hex())
        } else {
            match state.read(source, reference, limits)? {
                Some(raw) => Some(Digest256::of_bytes(&raw).to_hex()),
                None => None,
            }
        };
        let actual = event_inputs.and_then(|rows| {
            rows.iter().rev().find(|row| {
                row.get("entity_ref").and_then(Value::as_str) == Some(reference.as_str())
            })
        });
        if expected_digest.as_deref().is_none_or(|digest| {
            actual
                .and_then(|row| row.get("sha256"))
                .and_then(Value::as_str)
                != Some(digest)
        }) || actual
            .and_then(|row| row.get("availability"))
            .and_then(Value::as_str)
            != Some("tracked")
        {
            closure_ok = false;
        }
    }
    let event_output_rows = event.pointer("/entities/outputs").and_then(Value::as_array);
    let mut event_outputs = std::collections::BTreeMap::<String, String>::new();
    let mut event_private_refs = std::collections::BTreeSet::<String>::new();
    if let Some(rows) = event_output_rows {
        for row in rows {
            let (Some(reference), Some(digest)) = (
                row.get("entity_ref").and_then(Value::as_str),
                row.get("sha256").and_then(Value::as_str),
            ) else {
                continue;
            };
            event_outputs.insert(reference.to_owned(), digest.to_owned());
            if row.get("availability").and_then(Value::as_str) == Some("ignored_local") {
                event_private_refs.insert(reference.to_owned());
            }
        }
    }
    for label in [
        "anchor_ref",
        "raw_layer_ref",
        "normalized_layer_ref",
        "unit_packet_ref",
        "bridge_ref",
    ] {
        if let Some((reference, _, raw)) = outputs.get(label) {
            let digest = Digest256::of_bytes(raw).to_hex();
            if event_outputs.get(reference) != Some(&digest) {
                closure_ok = false;
            }
        } else {
            closure_ok = false;
        }
    }
    let outputs_obj = plan.get("outputs").unwrap_or(&Value::Null);
    let private_refs: std::collections::BTreeSet<String> = [
        "private_raw_content_ref",
        "private_normalized_content_ref",
        "private_operations_ref",
        "private_comparison_ref",
    ]
    .iter()
    .filter_map(|field| {
        outputs_obj
            .get(*field)
            .and_then(Value::as_str)
            .map(str::to_owned)
    })
    .collect();
    if event_private_refs != private_refs
        || event
            .pointer("/activity/event_type")
            .and_then(Value::as_str)
            != Some("alignment")
        || event.pointer("/method/command_capture") != Some(&expected_capture)
        || event
            .pointer("/reproducibility/classification")
            .and_then(Value::as_str)
            != Some("partially_specified")
        || event
            .pointer("/review_and_authority/human_review_status")
            .and_then(Value::as_str)
            != Some("blocked")
        || event.pointer("/review_and_authority/review_bindings") != Some(&json!([]))
        || event.pointer("/review_and_authority/accepted_uses") != Some(&json!([]))
        || event
            .pointer("/review_and_authority/promotion_authorized")
            .and_then(Value::as_bool)
            != Some(false)
        || event.pointer("/review_and_authority/competence_evidence_bindings") != Some(&json!([]))
        || event
            .pointer("/rights_and_visibility/content_visibility")
            .and_then(Value::as_str)
            != Some("local_only")
        || event
            .pointer("/rights_and_visibility/publication_authorized")
            .and_then(Value::as_bool)
            != Some(false)
        || event.pointer("/rights_and_visibility/publication_authority_bindings")
            != Some(&json!([]))
    {
        closure_ok = false;
    }
    if !closure_ok {
        state.issue(
            event_path,
            "bridge provenance closure or authority drifted",
            limits,
        )?;
    }
    Ok(())
}

fn canonical_json_encoded_len(value: &Value, limit: usize) -> Result<usize, ItemRefusal> {
    let mut counter = JsonSizeCounter { written: 0, limit };
    append_canonical_json(value, &mut counter, 0)?;
    Ok(counter.written)
}

fn canonical_json_bytes(value: &Value, limit: usize) -> Result<Vec<u8>, ItemRefusal> {
    let expected_len = canonical_json_encoded_len(value, limit)?;
    let mut output = Vec::with_capacity(expected_len);
    append_canonical_json(
        value,
        &mut BoundedVecWriter {
            output: &mut output,
            limit,
        },
        0,
    )?;
    if output.len() != expected_len {
        return Err(ItemRefusal::Budget);
    }
    Ok(output)
}

fn append_canonical_json(
    value: &Value,
    output: &mut impl Write,
    depth: usize,
) -> Result<(), ItemRefusal> {
    if depth > 128 {
        return Err(ItemRefusal::Budget);
    }
    match value {
        Value::Null => output.write_all(b"null").map_err(|_| ItemRefusal::Budget)?,
        Value::Bool(false) => output
            .write_all(b"false")
            .map_err(|_| ItemRefusal::Budget)?,
        Value::Bool(true) => output.write_all(b"true").map_err(|_| ItemRefusal::Budget)?,
        Value::Number(number) => output
            .write_all(number.to_string().as_bytes())
            .map_err(|_| ItemRefusal::Budget)?,
        Value::String(text) => {
            serde_json::to_writer(&mut *output, text).map_err(|_| ItemRefusal::Budget)?;
        }
        Value::Array(rows) => {
            output.write_all(b"[").map_err(|_| ItemRefusal::Budget)?;
            for (index, row) in rows.iter().enumerate() {
                if index != 0 {
                    output.write_all(b",").map_err(|_| ItemRefusal::Budget)?;
                }
                append_canonical_json(row, output, depth + 1)?;
            }
            output.write_all(b"]").map_err(|_| ItemRefusal::Budget)?;
        }
        Value::Object(map) => {
            output.write_all(b"{").map_err(|_| ItemRefusal::Budget)?;
            let mut prior: Option<&str> = None;
            for index in 0..map.len() {
                let key = map
                    .keys()
                    .map(String::as_str)
                    .filter(|key| prior.is_none_or(|previous| *key > previous))
                    .min()
                    .ok_or(ItemRefusal::Budget)?;
                if index != 0 {
                    output.write_all(b",").map_err(|_| ItemRefusal::Budget)?;
                }
                serde_json::to_writer(&mut *output, key).map_err(|_| ItemRefusal::Budget)?;
                output.write_all(b":").map_err(|_| ItemRefusal::Budget)?;
                append_canonical_json(map.get(key).ok_or(ItemRefusal::Budget)?, output, depth + 1)?;
                prior = Some(key);
            }
            output.write_all(b"}").map_err(|_| ItemRefusal::Budget)?;
        }
    }
    Ok(())
}

fn private_content_keys(value: &Value, forbidden: &[&str]) -> Option<String> {
    let mut stack = vec![value];
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(map) => {
                let mut leaked: Vec<_> = forbidden
                    .iter()
                    .filter(|key| map.contains_key(**key))
                    .copied()
                    .collect();
                if !leaked.is_empty() {
                    leaked.sort_unstable();
                    return Some(format!(
                        "[{}]",
                        leaked
                            .iter()
                            .map(|key| format!("'{key}'"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                stack.extend(map.values());
            }
            Value::Array(rows) => stack.extend(rows),
            _ => {}
        }
    }
    None
}

fn has_absolute_owner_path(value: &Value) -> bool {
    let mut stack = vec![value];
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(map) => stack.extend(map.values()),
            Value::Array(rows) => stack.extend(rows),
            Value::String(text)
                if ["/srv/", "/home/", "/tmp/", "/var/tmp/"]
                    .iter()
                    .any(|prefix| text.starts_with(prefix)) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn parse_csv_records(raw: &[u8]) -> Option<(Vec<String>, Vec<Vec<String>>)> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut records = Vec::<Vec<String>>::new();
    let mut row = Vec::<String>::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut just_closed_quote = false;
    let mut touched = false;
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                    just_closed_quote = true;
                }
            } else {
                field.push(ch);
            }
            touched = true;
            continue;
        }
        if just_closed_quote && !matches!(ch, ',' | '\n' | '\r') {
            return None;
        }
        match ch {
            '"' if field.is_empty() && !just_closed_quote => {
                quoted = true;
                touched = true;
            }
            '"' => return None,
            ',' => {
                row.push(std::mem::take(&mut field));
                just_closed_quote = false;
                touched = true;
            }
            '\n' | '\r' => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut row));
                just_closed_quote = false;
                touched = false;
            }
            _ => {
                field.push(ch);
                touched = true;
            }
        }
    }
    if quoted {
        return None;
    }
    if touched || !field.is_empty() || !row.is_empty() || just_closed_quote {
        row.push(field);
        records.push(row);
    }
    let mut records = records.into_iter();
    let headers = records.next()?;
    Some((headers, records.collect()))
}

fn append_text_metadata(
    state: &mut DirectState,
    source: &impl LayerFamilySource,
    path: &str,
    raw: &[u8],
    profile: &str,
    limits: ItemLimits,
) -> Result<(), ItemRefusal> {
    let metadata_limits = crate::text_metadata_rules::TextMetadataLimits {
        max_packet_bytes: limits.max_member_bytes,
        max_state_bytes: limits.max_state_bytes,
        max_issues: limits.max_issues.saturating_sub(state.issues.len()),
        deadline: limits.deadline,
    };
    let report = match profile {
        crate::text_rules::ANCHOR_V2_PROFILE => {
            crate::text_metadata_rules::inspect_source_anchor_v2_metadata(
                raw,
                path,
                metadata_limits,
                source.cancellation(),
            )?
        }
        crate::text_rules::TEXT_LAYER_PROFILE => {
            crate::text_metadata_rules::inspect_source_text_layer_metadata(
                raw,
                path,
                metadata_limits,
                source.cancellation(),
            )?
        }
        crate::text_rules::TEXT_UNIT_PROFILE => {
            crate::text_metadata_rules::inspect_source_text_unit_v1_metadata(
                raw,
                path,
                metadata_limits,
                source.cancellation(),
            )?
        }
        _ => {
            return Err(ItemRefusal::Unsupported(
                "source-foundation text metadata profile".into(),
            ));
        }
    };
    for issue in report.issues {
        state.issue(path, issue.message, limits)?;
    }
    if report.state == crate::text_metadata_rules::TextMetadataState::Unsupported {
        state.gap(
            format!("text-metadata profile unsupported for {path}"),
            limits,
        )?;
    }
    Ok(())
}

fn result(
    lab: SourceFoundationLab,
    mut state: DirectState,
    unimplemented: Vec<String>,
    limits: ItemLimits,
) -> Result<SourceFoundationLabResult, ItemRefusal> {
    for gap in unimplemented {
        state.gap(gap, limits)?;
    }
    let report_bytes = estimate_value_storage(&state.report)?;
    state.reserve(report_bytes, limits)?;
    Ok(SourceFoundationLabResult {
        lab,
        report: state.report,
        ordered_issues: state.issues,
        unimplemented: state.unimplemented,
        schema_checks: state.schema_checks,
        direct_read_bytes: state.read_bytes,
        retained_state_bytes: state.retained_bytes,
    })
}
