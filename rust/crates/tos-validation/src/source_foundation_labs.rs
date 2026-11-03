//! Source-foundation report contracts shared with ordinary selected-source validation.
//! Historical laboratories and dossier audits are available with the `research` feature.

pub use crate::source_foundation_schema::SourceFoundationSchemaCheck;
pub use crate::text_collation_rules::witness_text_collation_semantic_messages;
use serde_json::Value;
use tos_foundation::RelativePath;

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

#[cfg(feature = "research")]
mod research;
#[cfg(feature = "research")]
pub use research::*;
