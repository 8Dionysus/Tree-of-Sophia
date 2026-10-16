//! Source-foundation report contracts shared with ordinary selected-source validation.
//! Historical laboratories and dossier audits are available with the `research` feature.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationSchemaRequest {
    /// Insert the complete schema diagnostics immediately before this many
    /// already-produced owner issues.
    pub before_issue: usize,
    pub location: String,
    pub contract: String,
    pub document: Value,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceFoundationGoldsetsReport {
    /// Ordered `(location, message)` owner-rule issues. Schema diagnostics are
    /// deliberately absent until the caller evaluates `schema_requests`.
    pub ordered_issues: Vec<(String, String)>,
    pub schema_requests: Vec<SourceFoundationSchemaRequest>,
    /// Owner predicates whose evidence requires a capability not present in
    /// the current `LayerFamilySource` contract. These prevent callers from
    /// interpreting an empty issue list as a complete dossier verdict.
    pub coverage_gaps: Vec<String>,
    /// Event writes in Python dictionary insertion order, with duplicate IDs
    /// retaining their original slot and final local value. The source-event
    /// connector merges this bounded owner contribution with sibling families.
    pub source_events: Vec<(String, Value)>,
    /// Local graph claim identities retained only so successive gold-set roots
    /// can enforce the maintained sequential duplicate rule.
    graph_claim_ids: Vec<String>,
    pub current_bytes: u64,
    pub recorded_bytes: u64,
    /// Conservative retained-state accounting charged to `ItemLimits`.
    pub retained_state_bytes: usize,
}

#[cfg(feature = "research")]
mod research;
#[cfg(feature = "research")]
pub use research::*;
