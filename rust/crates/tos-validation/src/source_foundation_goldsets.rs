//! Bounded checks for maintained source-witness gold-set dossiers.
//!
//! This is a current-source district report. It checks concrete owner rules
//! over exact current source, records, file memberships, and explicitly
//! observed payloads. It never grants source admission, textual acceptance,
//! rights, semantic truth, or a whole-source completeness verdict. Schema
//! diagnostics are returned as ordered requests so the caller can use its
//! diagnostic-capable schema worker without substituting a bool result.

use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::layer_family_rules::{LayerFamilySource, LayerPayload};
use crate::record_biblio_cut::BiblioCurrentRecord;
use crate::source_foundation_default_rules::{
    BorrowedDefaultRecords, SliceDefaultPaths, SourceFoundationDefaultEventLookup,
    SourceFoundationDefaultPaths, SourceFoundationDefaultRecordsLookup,
};
use crate::source_foundation_discovery::{
    PhysicalPathFacts, PhysicalResolvedTargetFacts as ResolvedTargetFacts, SourcePhysicalFacts,
};
use crate::source_witness_foundation::SourceFileMembershipIndex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::mem::size_of;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};

const SAMPLE_PLAN: &str = "sample-plan.json";
const GOLD_STATUS: &str = "gold-status.json";
const GOLD_ASSURANCE: &str = "gold-assurance.v2.json";
const TRANSLATION_PLAN: &str = "translation-samples.json";
const TRANSFER_PLAN: &str = "transfer-samples.json";
const SEMANTIC_PLAN: &str = "semantic-samples.json";
const LLM_PLAN: &str = "llm-tasks.json";
const RETRIEVAL_PLAN: &str = "retrieval-queries.json";
const GRAPH_PLAN: &str = "graph-queries.json";

const DIAG_STAGE_LOADS: u32 = 0;
const DIAG_STAGE_RECEIPT: u32 = 10;
const DIAG_STAGE_PACKET_SCHEMAS: u32 = 20;
const DIAG_STAGE_INITIAL_OBSERVATION: u32 = 30;
const DIAG_STAGE_TRANSLATION_REVIEW: u32 = 40;
const DIAG_STAGE_SOURCE_TRIANGULATION: u32 = 50;
const DIAG_STAGE_BOUNDED_INPUT: u32 = 60;
const DIAG_STAGE_SPECIALIZED_PACKETS: u32 = 65;
const DIAG_STAGE_EXPERIMENTAL_PACKETS: u32 = 70;
const DIAG_STAGE_LABORATORY_PACKETS: u32 = 80;
const DIAG_STAGE_RETRIEVAL_PLAN: u32 = 90;
const DIAG_STAGE_PROVENANCE: u32 = 100;

fn advance_diagnostic_stage(stage: u32, amount: u32) -> Result<u32, ItemRefusal> {
    stage.checked_add(amount).ok_or(ItemRefusal::Budget)
}

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

#[derive(Debug)]
enum PendingDiagnosticKind {
    Issue(String, String),
    Schema(SourceFoundationSchemaRequest),
}

#[derive(Debug)]
struct PendingDiagnostic {
    stage: u32,
    sequence: u64,
    kind: PendingDiagnosticKind,
}

#[derive(Debug, Clone)]
struct JsonDocument {
    path: String,
    value: Value,
    sha256: String,
}

#[derive(Debug, Clone)]
struct JsonLine {
    location: String,
    value: Value,
}

#[derive(Debug, Clone, Copy)]
struct PhysicalTarget<'a> {
    relative_target: Option<&'a str>,
    exists: bool,
    regular_file: bool,
    directory: bool,
    byte_size: Option<u64>,
    sha256: Option<&'a str>,
    git_tracked: Option<bool>,
    git_ignored: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
enum PhysicalTargetObservation<'a> {
    Target(PhysicalTarget<'a>),
    OutsideSelectedRoot,
    Unknown,
}

fn physical_target_observation(facts: &PhysicalPathFacts) -> PhysicalTargetObservation<'_> {
    if !facts.symlink {
        return PhysicalTargetObservation::Target(PhysicalTarget {
            relative_target: None,
            exists: facts.exists,
            regular_file: facts.regular_file,
            directory: facts.directory,
            byte_size: facts.byte_size,
            sha256: facts.sha256.as_deref(),
            git_tracked: facts.git_tracked,
            git_ignored: facts.git_ignored,
        });
    }
    match facts.resolved_target.as_ref() {
        Some(ResolvedTargetFacts::InsideSelectedRoot {
            relative_target,
            exists,
            regular_file,
            directory,
            byte_size,
            sha256,
            git_tracked,
            git_ignored,
            ..
        }) => PhysicalTargetObservation::Target(PhysicalTarget {
            relative_target: Some(relative_target),
            exists: *exists,
            regular_file: *regular_file,
            directory: *directory,
            byte_size: *byte_size,
            sha256: sha256.as_deref(),
            git_tracked: *git_tracked,
            git_ignored: *git_ignored,
        }),
        Some(ResolvedTargetFacts::OutsideSelectedRoot) => {
            PhysicalTargetObservation::OutsideSelectedRoot
        }
        Some(ResolvedTargetFacts::Unknown) | None => PhysicalTargetObservation::Unknown,
    }
}

#[derive(Default)]
struct SizeCounter(usize);

impl Write for SizeCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("serialized size overflow"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SampleBinding {
    sample: Value,
    item_ref: Value,
    file_ref: Value,
    anchor_ref: Value,
    language: Option<String>,
}

struct GoldsetChecks<'a, S: LayerFamilySource> {
    source: &'a mut S,
    paths: &'a dyn SourceFoundationDefaultPaths,
    limits: ItemLimits,
    report: SourceFoundationGoldsetsReport,
    current_digests: BTreeMap<String, String>,
    live_bytes: usize,
    diagnostic_stage: u32,
    diagnostic_sequence: u64,
    pending_issue_count: usize,
    pending_diagnostics: Vec<PendingDiagnostic>,
}

impl<'a, S: LayerFamilySource> GoldsetChecks<'a, S> {
    fn checkpoint(&mut self) -> Result<(), ItemRefusal> {
        self.source.checkpoint(self.limits.deadline)
    }

    fn set_diagnostic_stage(&mut self, stage: u32) {
        self.diagnostic_stage = stage;
    }

    fn pending_diagnostic(
        &mut self,
        kind: PendingDiagnosticKind,
        retained_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        self.charge(
            retained_bytes
                .checked_add(size_of::<PendingDiagnostic>())
                .ok_or(ItemRefusal::Budget)?,
        )?;
        self.push_pending_diagnostic(kind)
    }

    fn push_pending_diagnostic(&mut self, kind: PendingDiagnosticKind) -> Result<(), ItemRefusal> {
        let sequence = self.diagnostic_sequence;
        self.diagnostic_sequence = sequence.checked_add(1).ok_or(ItemRefusal::Budget)?;
        self.pending_diagnostics
            .try_reserve_exact(1)
            .map_err(|_| ItemRefusal::Budget)?;
        self.pending_diagnostics.push(PendingDiagnostic {
            stage: self.diagnostic_stage,
            sequence,
            kind,
        });
        Ok(())
    }

    fn finalize_diagnostics(&mut self) -> Result<(), ItemRefusal> {
        self.pending_diagnostics
            .sort_unstable_by_key(|diagnostic| (diagnostic.stage, diagnostic.sequence));
        for diagnostic in self.pending_diagnostics.drain(..) {
            match diagnostic.kind {
                PendingDiagnosticKind::Issue(location, message) => {
                    self.report
                        .ordered_issues
                        .try_reserve_exact(1)
                        .map_err(|_| ItemRefusal::Budget)?;
                    self.report.ordered_issues.push((location, message));
                }
                PendingDiagnosticKind::Schema(mut request) => {
                    request.before_issue = self.report.ordered_issues.len();
                    self.report
                        .schema_requests
                        .try_reserve_exact(1)
                        .map_err(|_| ItemRefusal::Budget)?;
                    self.report.schema_requests.push(request);
                }
            }
        }
        Ok(())
    }

    fn current_path_present(&mut self, path: &str) -> Result<bool, ItemRefusal> {
        let paths = self.paths;
        let deadline = self.limits.deadline;
        let source = &mut *self.source;
        paths.contains_with_checkpoint(path, &mut || source.checkpoint(deadline))
    }

    fn charge(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        let retained = self
            .report
            .retained_state_bytes
            .checked_add(bytes)
            .filter(|used| {
                used.checked_add(self.live_bytes)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::Budget)?;
        self.report.retained_state_bytes = retained;
        Ok(())
    }

    fn hold_live(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        self.live_bytes = self
            .live_bytes
            .checked_add(bytes)
            .filter(|live| {
                self.report
                    .retained_state_bytes
                    .checked_add(*live)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn release_live(&mut self, bytes: usize) {
        self.live_bytes = self.live_bytes.saturating_sub(bytes);
    }

    fn charge_clone(&mut self, value: &Value) -> Result<(), ItemRefusal> {
        let bytes = encoded_len(value)?
            .checked_mul(4)
            .and_then(|n| n.checked_add(size_of::<Value>() + 64))
            .ok_or(ItemRefusal::Budget)?;
        self.charge(bytes)
    }

    fn insert_string(
        &mut self,
        set: &mut BTreeSet<String>,
        value: &str,
    ) -> Result<bool, ItemRefusal> {
        if set.contains(value) {
            return Ok(false);
        }
        self.charge(value.len() + size_of::<String>() + 48)?;
        set.insert(value.to_owned());
        Ok(true)
    }

    fn repository_ref_exists(&mut self, value: &Value) -> Result<bool, ItemRefusal> {
        let Some(reference) = value.as_str() else {
            return Ok(false);
        };
        if !reference.starts_with("ToS/") {
            return Ok(true);
        }
        if RelativePath::parse(reference).is_err() {
            return Ok(false);
        }
        self.checkpoint()?;
        let exists = self.source.exists(
            reference,
            self.limits.max_member_bytes,
            self.limits.deadline,
        )?;
        self.checkpoint()?;
        Ok(exists)
    }

    fn current_digest(&mut self, path: &str) -> Result<Option<String>, ItemRefusal> {
        if RelativePath::parse(path).is_err() {
            return Ok(None);
        }
        if let Some(digest) = self.current_digests.get(path) {
            return Ok(Some(digest.clone()));
        }
        let Some(raw) = self.current(path, false)? else {
            return Ok(None);
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        self.release_live(raw.len());
        Ok(Some(digest))
    }

    fn recorded_digest_matches(
        &mut self,
        path: &str,
        expected_digest: &str,
    ) -> Result<bool, ItemRefusal> {
        self.checkpoint()?;
        if RelativePath::parse(path).is_err() {
            return Ok(false);
        }
        let raw = self.source.recorded(
            path,
            expected_digest,
            self.limits.max_member_bytes,
            self.limits.deadline,
        )?;
        self.checkpoint()?;
        let Some(raw) = raw else {
            return Ok(false);
        };
        if raw.len() > self.limits.max_member_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.hold_live(raw.len())?;
        self.report.recorded_bytes = self
            .report
            .recorded_bytes
            .checked_add(raw.len() as u64)
            .filter(|used| {
                used.checked_add(self.report.current_bytes)
                    .is_some_and(|total| total <= self.limits.max_total_bytes)
            })
            .ok_or(ItemRefusal::Budget)?;
        let actual = Digest256::of_bytes(&raw).to_hex();
        self.release_live(raw.len());
        Ok(actual == expected_digest)
    }

    fn python_equal(&mut self, left: &Value, right: &Value) -> Result<bool, ItemRefusal> {
        self.checkpoint()?;
        let equal = crate::assessment::py_equal(left, right).map_err(assessment_refusal)?;
        self.checkpoint()?;
        Ok(equal)
    }

    fn python_optional_equal(
        &mut self,
        left: Option<&Value>,
        right: Option<&Value>,
    ) -> Result<bool, ItemRefusal> {
        self.python_equal(left.unwrap_or(&Value::Null), right.unwrap_or(&Value::Null))
    }

    fn python_different(
        &mut self,
        left: Option<&Value>,
        right: Option<&Value>,
    ) -> Result<bool, ItemRefusal> {
        Ok(!self.python_optional_equal(left, right)?)
    }

    fn python_values_have_duplicates(
        &mut self,
        values: &[Option<&Value>],
    ) -> Result<bool, ItemRefusal> {
        for (index, value) in values.iter().enumerate() {
            for previous in &values[..index] {
                if self.python_optional_equal(*value, *previous)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn python_pairs_have_duplicates(
        &mut self,
        values: &[(Option<&Value>, Option<&Value>)],
    ) -> Result<bool, ItemRefusal> {
        for (index, value) in values.iter().enumerate() {
            for previous in &values[..index] {
                if self.python_optional_equal(value.0, previous.0)?
                    && self.python_optional_equal(value.1, previous.1)?
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn find_python_value<'b>(
        &mut self,
        rows: &'b [Value],
        field: &str,
        target: Option<&Value>,
    ) -> Result<Option<&'b Value>, ItemRefusal> {
        for row in rows {
            self.checkpoint()?;
            if self.python_optional_equal(row.get(field), target)? {
                return Ok(Some(row));
            }
        }
        Ok(None)
    }

    fn issue(
        &mut self,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), ItemRefusal> {
        if self.pending_issue_count >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        let location = location.into();
        let message = message.into();
        self.pending_issue_count = self
            .pending_issue_count
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        let retained = location.len() + message.len() + size_of::<(String, String)>();
        self.pending_diagnostic(PendingDiagnosticKind::Issue(location, message), retained)?;
        Ok(())
    }

    fn coverage_gap(&mut self, description: impl Into<String>) -> Result<(), ItemRefusal> {
        let description = description.into();
        self.charge(description.len() + size_of::<String>() + 32)?;
        self.report.coverage_gaps.push(description);
        Ok(())
    }

    fn current(&mut self, path: &str, required: bool) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint()?;
        if RelativePath::parse(path).is_err() {
            return Err(ItemRefusal::Unsupported("gold-set member path".into()));
        }
        let raw = self
            .source
            .current(path, self.limits.max_member_bytes, self.limits.deadline)?;
        self.checkpoint()?;
        if let Some(raw) = raw {
            if raw.len() > self.limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            self.hold_live(raw.len())?;
            self.report.current_bytes = self
                .report
                .current_bytes
                .checked_add(raw.len() as u64)
                .filter(|used| {
                    used.checked_add(self.report.recorded_bytes)
                        .is_some_and(|total| total <= self.limits.max_total_bytes)
                })
                .ok_or(ItemRefusal::Budget)?;
            let digest = Digest256::of_bytes(&raw).to_hex();
            self.charge(path.len() + digest.len() + 48)?;
            self.current_digests.insert(path.to_owned(), digest);
            Ok(Some(raw))
        } else {
            if required {
                self.issue(path, "file is missing")?;
            }
            Ok(None)
        }
    }

    fn json(&mut self, path: &str, required: bool) -> Result<Option<JsonDocument>, ItemRefusal> {
        self.json_at(path, path, required)
    }

    fn json_at(
        &mut self,
        source_path: &str,
        location_path: &str,
        required: bool,
    ) -> Result<Option<JsonDocument>, ItemRefusal> {
        let Some(raw) = self.current(source_path, required)? else {
            return Ok(None);
        };
        let parse_reserve = raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?;
        self.hold_live(parse_reserve)?;
        let decoded = match serde_json::from_slice::<Value>(&raw) {
            Ok(value) => value,
            Err(error) => {
                self.release_live(raw.len() + parse_reserve);
                self.issue(
                    location_path,
                    format!("cannot read JSON: {}", json_error_kind(&error)),
                )?;
                return Ok(None);
            }
        };
        if !decoded.is_object() {
            self.release_live(raw.len() + parse_reserve);
            self.issue(location_path, "JSON root must be an object")?;
            return Ok(None);
        }
        // Reserve the decoded tree before parsing, then retain it once here;
        // its schema-request clone is charged when queued.
        let retained = raw
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(size_of::<Value>() + location_path.len() + 64))
            .ok_or(ItemRefusal::Budget)?;
        self.charge(retained)?;
        let sha256 = Digest256::of_bytes(&raw).to_hex();
        self.release_live(raw.len() + parse_reserve);
        Ok(Some(JsonDocument {
            path: location_path.into(),
            value: decoded,
            sha256,
        }))
    }

    fn jsonl(&mut self, path: &str, required: bool) -> Result<Option<Vec<JsonLine>>, ItemRefusal> {
        let Some(raw) = self.current(path, required)? else {
            return Ok(None);
        };
        let text = match std::str::from_utf8(&raw) {
            Ok(text) => text,
            Err(_) => {
                self.release_live(raw.len());
                self.issue(path, "cannot read JSONL: input is not valid UTF-8")?;
                return Ok(Some(Vec::new()));
            }
        };
        let mut rows = Vec::new();
        for (index, line) in text.lines().enumerate() {
            self.checkpoint()?;
            let location = format!("{path}:{}", index + 1);
            if line.trim().is_empty() {
                self.issue(location, "blank JSONL line is not allowed")?;
                continue;
            }
            let parse_reserve = line.len().checked_mul(8).ok_or(ItemRefusal::Budget)?;
            self.hold_live(parse_reserve)?;
            let value = match serde_json::from_str::<Value>(line) {
                Ok(value) => value,
                Err(error) => {
                    self.release_live(parse_reserve);
                    self.issue(
                        location,
                        format!("invalid JSON: {}", json_error_kind(&error)),
                    )?;
                    continue;
                }
            };
            if !value.is_object() {
                self.release_live(parse_reserve);
                self.issue(location, "JSONL record must be an object")?;
                continue;
            }
            let retained = line
                .len()
                .checked_mul(8)
                .and_then(|n| n.checked_add(size_of::<Value>() + location.len() + 64))
                .ok_or(ItemRefusal::Budget)?;
            self.charge(retained)?;
            self.release_live(parse_reserve);
            rows.push(JsonLine { location, value });
        }
        self.release_live(raw.len());
        Ok(Some(rows))
    }

    fn schema(
        &mut self,
        location: &str,
        contract: &str,
        document: &Value,
    ) -> Result<(), ItemRefusal> {
        let retained = encoded_len(document)?
            .checked_mul(8)
            .and_then(|n| {
                n.checked_add(
                    location.len()
                        + contract.len()
                        + size_of::<SourceFoundationSchemaRequest>()
                        + 96,
                )
            })
            .ok_or(ItemRefusal::Budget)?;
        self.charge(
            retained
                .checked_add(size_of::<PendingDiagnostic>())
                .ok_or(ItemRefusal::Budget)?,
        )?;
        self.push_pending_diagnostic(PendingDiagnosticKind::Schema(
            SourceFoundationSchemaRequest {
                before_issue: 0,
                location: location.into(),
                contract: contract.into(),
                document: document.clone(),
            },
        ))?;
        Ok(())
    }
}

fn value_contains_any_text(value: &Value, forbidden: &[&str]) -> bool {
    match value {
        Value::String(text) => forbidden.iter().any(|needle| text.contains(needle)),
        Value::Array(rows) => rows
            .iter()
            .any(|row| value_contains_any_text(row, forbidden)),
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            forbidden.iter().any(|needle| key.contains(needle))
                || value_contains_any_text(value, forbidden)
        }),
        _ => false,
    }
}

fn value_contains_exact_atom(value: &Value, forbidden: &[&str]) -> bool {
    match value {
        Value::String(text) => forbidden.iter().any(|needle| text == needle),
        Value::Array(rows) => rows
            .iter()
            .any(|row| value_contains_exact_atom(row, forbidden)),
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            forbidden.iter().any(|needle| key == needle)
                || value_contains_exact_atom(value, forbidden)
        }),
        _ => false,
    }
}

/// Inspect every current gold-set root enumerated by the caller's captured
/// authored-path inventory. Record values and the frozen file-membership index
/// are the exact siblings produced by the current-source owner.
///
/// Required packet omissions remain ordinary issues (`file is missing`);
/// limit, deadline, and source failures remain the separate `ItemRefusal`
/// result. This district never implies whole-source admission.
pub fn inspect_source_foundation_goldsets(
    source: &mut impl LayerFamilySource,
    current_paths: &[String],
    source_events: &BTreeMap<String, Value>,
    current_records: &BTreeMap<String, BiblioCurrentRecord>,
    item_editions: &BTreeMap<String, String>,
    rights_ids: &BTreeSet<String>,
    file_memberships: &SourceFileMembershipIndex,
    physical: &SourcePhysicalFacts,
    require_local_payloads: bool,
    limits: ItemLimits,
) -> Result<SourceFoundationGoldsetsReport, ItemRefusal> {
    let paths = SliceDefaultPaths(current_paths);
    let declared_profile_kinds = BTreeSet::new();
    let records = BorrowedDefaultRecords {
        current_records,
        item_editions,
        rights_ids,
        file_memberships,
        declared_profile_kinds: &declared_profile_kinds,
    };
    inspect_source_foundation_goldsets_with_lookups(
        source,
        &paths,
        source_events,
        &records,
        physical,
        require_local_payloads,
        limits,
    )
}

/// Inspect Gold over the same bounded lookup kernels used by the cold owner
/// maps. Stored candidates provide these lookups from their authenticated
/// Records, Events, and path indexes without reconstructing full reports.
pub fn inspect_source_foundation_goldsets_with_lookups(
    source: &mut impl LayerFamilySource,
    paths: &dyn SourceFoundationDefaultPaths,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    physical: &SourcePhysicalFacts,
    require_local_payloads: bool,
    limits: ItemLimits,
) -> Result<SourceFoundationGoldsetsReport, ItemRefusal> {
    let (roots, roots_state_bytes) = gold_roots(source, paths, limits)?;
    let mut combined = SourceFoundationGoldsetsReport::default();
    combined.retained_state_bytes = roots_state_bytes;
    let mut prior_graph_claim_ids = BTreeSet::new();
    let mut seen_anchor_ids = BTreeSet::new();
    let mut seen_gold_event_ids = BTreeSet::new();
    for root in roots {
        let remaining = ItemLimits {
            max_member_bytes: limits.max_member_bytes,
            max_total_bytes: limits
                .max_total_bytes
                .checked_sub(combined.current_bytes)
                .and_then(|remaining| remaining.checked_sub(combined.recorded_bytes))
                .ok_or(ItemRefusal::Budget)?,
            max_state_bytes: limits
                .max_state_bytes
                .checked_sub(combined.retained_state_bytes)
                .ok_or(ItemRefusal::Budget)?,
            max_issues: limits
                .max_issues
                .checked_sub(combined.ordered_issues.len())
                .ok_or(ItemRefusal::Budget)?,
            deadline: limits.deadline,
        };
        let mut next = inspect_source_foundation_goldset_root(
            source,
            &root,
            paths,
            source_events,
            &prior_graph_claim_ids,
            &mut seen_anchor_ids,
            &mut seen_gold_event_ids,
            records,
            physical,
            require_local_payloads,
            remaining,
        )?;
        let prior_issue_count = combined.ordered_issues.len();
        for request in &mut next.schema_requests {
            request.before_issue = request
                .before_issue
                .checked_add(prior_issue_count)
                .ok_or(ItemRefusal::Budget)?;
        }
        combined.current_bytes = combined
            .current_bytes
            .checked_add(next.current_bytes)
            .ok_or(ItemRefusal::Budget)?;
        combined.recorded_bytes = combined
            .recorded_bytes
            .checked_add(next.recorded_bytes)
            .ok_or(ItemRefusal::Budget)?;
        combined.retained_state_bytes = combined
            .retained_state_bytes
            .checked_add(next.retained_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        combined
            .ordered_issues
            .try_reserve_exact(next.ordered_issues.len())
            .map_err(|_| ItemRefusal::Budget)?;
        combined
            .schema_requests
            .try_reserve_exact(next.schema_requests.len())
            .map_err(|_| ItemRefusal::Budget)?;
        combined
            .coverage_gaps
            .try_reserve_exact(next.coverage_gaps.len())
            .map_err(|_| ItemRefusal::Budget)?;
        combined
            .source_events
            .try_reserve_exact(next.source_events.len())
            .map_err(|_| ItemRefusal::Budget)?;
        combined.ordered_issues.extend(next.ordered_issues);
        combined.schema_requests.extend(next.schema_requests);
        combined.coverage_gaps.extend(next.coverage_gaps);
        combined.source_events.extend(next.source_events);
        for id in next.graph_claim_ids {
            if !prior_graph_claim_ids.contains(&id) {
                let charge = id.len() + size_of::<String>() + 32;
                let retained = combined
                    .retained_state_bytes
                    .checked_add(charge)
                    .filter(|used| *used <= limits.max_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                prior_graph_claim_ids.insert(id);
                combined.retained_state_bytes = retained;
            }
        }
    }
    Ok(combined)
}

fn gold_roots(
    source: &mut impl LayerFamilySource,
    paths: &dyn SourceFoundationDefaultPaths,
    limits: ItemLimits,
) -> Result<(BTreeSet<String>, usize), ItemRefusal> {
    let mut roots = BTreeSet::<String>::new();
    let mut state_bytes = 0usize;
    paths.for_each_path(&mut |path| {
        source.checkpoint(limits.deadline)?;
        let mut components = path.split('/');
        if components.next() != Some("ToS") || components.next() != Some("source-witnesses") {
            return Ok(());
        }
        let mut component_start = "ToS/source-witnesses/".len();
        while let Some(component) = components.next() {
            if component == "gold-sets" {
                let Some(set_id) = components.next() else {
                    break;
                };
                if set_id.is_empty() {
                    break;
                }
                let root_end = component_start
                    .checked_add("gold-sets".len())
                    .and_then(|offset| offset.checked_add(1))
                    .and_then(|offset| offset.checked_add(set_id.len()))
                    .ok_or(ItemRefusal::Budget)?;
                let Some(root) = path.get(..root_end) else {
                    break;
                };
                if !roots.contains(root) {
                    let charge = root
                        .len()
                        .checked_add(size_of::<String>() + 32)
                        .ok_or(ItemRefusal::Budget)?;
                    state_bytes = state_bytes
                        .checked_add(charge)
                        .filter(|used| *used <= limits.max_state_bytes)
                        .ok_or(ItemRefusal::Budget)?;
                    roots.insert(root.to_owned());
                }
                break;
            }
            component_start = component_start
                .checked_add(component.len())
                .and_then(|offset| offset.checked_add(1))
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    })?;
    Ok((roots, state_bytes))
}

fn inspect_source_foundation_goldset_root(
    source: &mut impl LayerFamilySource,
    root: &str,
    paths: &dyn SourceFoundationDefaultPaths,
    source_events: &dyn SourceFoundationDefaultEventLookup,
    prior_graph_claim_ids: &BTreeSet<String>,
    seen_anchor_ids: &mut BTreeSet<String>,
    seen_gold_event_ids: &mut BTreeSet<String>,
    records: &dyn SourceFoundationDefaultRecordsLookup,
    physical: &SourcePhysicalFacts,
    require_local_payloads: bool,
    limits: ItemLimits,
) -> Result<SourceFoundationGoldsetsReport, ItemRefusal> {
    let mut checks = GoldsetChecks {
        source,
        paths,
        limits,
        report: SourceFoundationGoldsetsReport::default(),
        current_digests: BTreeMap::new(),
        live_bytes: 0,
        diagnostic_stage: DIAG_STAGE_LOADS,
        diagnostic_sequence: 0,
        pending_issue_count: 0,
        pending_diagnostics: Vec::new(),
    };
    let path = |name: &str| format!("{root}/{name}");
    // These packets are the maintained foundation-pilot set loaded as source
    // inputs by the Python owner. Keep absent required packets visible.
    let sample = checks.json(&path(SAMPLE_PLAN), true)?;
    // Load only the bounded, named optional documents present in the caller's
    // captured current-path inventory. Globs are represented by sorted exact
    // members, matching the maintained source validator's deterministic
    // discovery order.
    let mut optional_documents = BTreeMap::<String, JsonDocument>::new();
    let early_optional_names = [
        "translation-source-review-plan.v2.json",
        "translation-laboratory-plan.v1.json",
        "translation-exposure-aware-plan.v1.json",
        "translation-reference-register.v1.json",
        "german-assisted-source-review.v1.json",
        "transfer-source-visible-review.jenseits-187.v1.json",
    ];
    let later_optional_names = [
        "critical-edition-citation-witness-decision.ekgwb.za-i-vorrede-1.v1.json",
        "edition-reading-admission.dta-ekgwb.za-i-vorrede-1.v1.json",
        "german-source-triangulation.ekgwb-dta-naumann.za-i-vorrede-1.v1.json",
        "bounded-translation-research-input.za-i-vorrede-1-opening-sentence.v1.json",
        "experimental-translation-candidate.admitted-ekgwb.za-i-vorrede-1-opening.variant-a.v1.json",
    ];
    let ocr_plan_path = path("ocr-visual-samples.json");
    if checks.current_path_present(&ocr_plan_path)? {
        if let Some(document) = checks.json(&ocr_plan_path, true)? {
            optional_documents.insert(ocr_plan_path, document);
        }
    }
    let status = checks.json(&path(GOLD_STATUS), true)?;
    let assurance = checks.json(&path(GOLD_ASSURANCE), true)?;
    let translation = checks.json(&path(TRANSLATION_PLAN), true)?;
    for name in early_optional_names {
        let member = path(name);
        if checks.current_path_present(&member)? {
            if let Some(document) = checks.json(&member, true)? {
                optional_documents.insert(member, document);
            }
        }
    }
    let critical_prefix = format!("{root}/critical-edition-witness.");
    let mut critical_witness_paths = Vec::<String>::new();
    let paths = checks.paths;
    paths.for_each_path(&mut |member| {
        checks.checkpoint()?;
        if member
            .strip_prefix(&critical_prefix)
            .is_some_and(|suffix| !suffix.contains('/') && suffix.ends_with(".json"))
        {
            checks.charge(member.len() + size_of::<String>() + 32)?;
            critical_witness_paths.push(member.to_owned());
        }
        Ok(())
    })?;
    critical_witness_paths.sort();
    for member in &critical_witness_paths {
        if let Some(document) = checks.json(member, true)? {
            optional_documents.insert(member.clone(), document);
        }
    }
    for name in later_optional_names {
        let member = path(name);
        if checks.current_path_present(&member)? {
            if let Some(document) = checks.json(&member, true)? {
                optional_documents.insert(member, document);
            }
        }
    }
    let episode_prefix = format!("{root}/experimental-translation-episode.");
    let mut experimental_episode_paths = Vec::<String>::new();
    let paths = checks.paths;
    paths.for_each_path(&mut |member| {
        checks.checkpoint()?;
        if member
            .strip_prefix(&episode_prefix)
            .is_some_and(|suffix| !suffix.contains('/') && suffix.ends_with(".json"))
        {
            checks.charge(member.len() + size_of::<String>() + 32)?;
            experimental_episode_paths.push(member.to_owned());
        }
        Ok(())
    })?;
    experimental_episode_paths.sort();
    for member in &experimental_episode_paths {
        if let Some(document) = checks.json(member, true)? {
            optional_documents.insert(member.clone(), document);
        }
    }
    for name in [
        "initial-sign-packet.v5.json",
        "initial-semantic-source-observation-plan.v1.json",
        "semantic-source-recurrence-plan.v1.json",
        "semantic-source-recurrence-receipt.v1.json",
    ] {
        let member = path(name);
        if let Some(document) = checks.json(&member, true)? {
            optional_documents.insert(member, document);
        }
    }
    let transfer = checks.json(&path(TRANSFER_PLAN), true)?;
    let semantic = checks.json(&path(SEMANTIC_PLAN), true)?;
    let llm = checks.json(&path(LLM_PLAN), true)?;
    let retrieval = checks.json(&path(RETRIEVAL_PLAN), true)?;
    let visual_plan_path = path("visual-retrieval-plan.v1.json");
    if checks.current_path_present(&visual_plan_path)? {
        if let Some(document) = checks.json(&visual_plan_path, true)? {
            optional_documents.insert(visual_plan_path, document);
        }
    }
    let graph = checks.json(&path(GRAPH_PLAN), true)?;

    let anchors_path = path("anchors.jsonl");
    let translation_anchors_path = path("translation-anchors.jsonl");
    // Reserve a diagnostic interval after every provenance file's parse and
    // row-validation turn. Supplementary and episode files are source members,
    // so the range is derived from the captured inventory rather than a fixed
    // ceiling that could collide with later owner stages.
    let mut provenance_file_slots = 4u32;
    for name in [
        "provenance.german-source-triangulation.jsonl",
        "provenance.bounded-translation-research-input.jsonl",
        "provenance.critical-edition-citation-decision.jsonl",
        "provenance.edition-reading-admission.jsonl",
        "provenance.translation-exposure-aware-plan.jsonl",
        "provenance.semantic-ladder-v4.jsonl",
        "provenance.semantic-source-observation-v1.jsonl",
        "provenance.semantic-source-recurrence-v1.jsonl",
    ] {
        if checks.current_path_present(&path(name))? {
            provenance_file_slots = provenance_file_slots
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    let episode_provenance_prefix = format!("{root}/provenance.experimental-translation-episodes");
    let paths = checks.paths;
    paths.for_each_path(&mut |member| {
        checks.checkpoint()?;
        if member
            .strip_prefix(&episode_provenance_prefix)
            .is_some_and(|suffix| !suffix.contains('/') && suffix.ends_with(".jsonl"))
        {
            provenance_file_slots = provenance_file_slots
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    })?;
    let mut next_anchor_stage = advance_diagnostic_stage(
        DIAG_STAGE_PROVENANCE,
        provenance_file_slots
            .checked_mul(2)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let mut anchor_validation_stages = BTreeMap::<String, u32>::new();
    checks.set_diagnostic_stage(next_anchor_stage);
    let anchor_rows = checks.jsonl(&anchors_path, true)?.unwrap_or_default();
    checks.charge(anchors_path.len() + size_of::<String>() + size_of::<u32>() + 48)?;
    anchor_validation_stages.insert(
        anchors_path.clone(),
        advance_diagnostic_stage(next_anchor_stage, 1)?,
    );
    next_anchor_stage = advance_diagnostic_stage(next_anchor_stage, 2)?;
    checks.set_diagnostic_stage(next_anchor_stage);
    let translation_anchor_rows = checks
        .jsonl(&translation_anchors_path, true)?
        .unwrap_or_default();
    checks.charge(translation_anchors_path.len() + size_of::<String>() + size_of::<u32>() + 48)?;
    anchor_validation_stages.insert(
        translation_anchors_path.clone(),
        advance_diagnostic_stage(next_anchor_stage, 1)?,
    );
    next_anchor_stage = advance_diagnostic_stage(next_anchor_stage, 2)?;
    let mut optional_anchor_rows = Vec::<(String, Vec<JsonLine>)>::new();
    for name in [
        "ocr-anchors.jsonl",
        "transfer-target-anchors.v1.jsonl",
        "semantic-source-observation-anchors.v1.jsonl",
    ] {
        let member = path(name);
        if checks.current_path_present(&member)? {
            checks.set_diagnostic_stage(next_anchor_stage);
            if let Some(rows) = checks.jsonl(&member, true)? {
                let validation_stage = advance_diagnostic_stage(next_anchor_stage, 1)?;
                checks.charge(member.len() + size_of::<String>() + size_of::<u32>() + 48)?;
                anchor_validation_stages.insert(member.clone(), validation_stage);
                optional_anchor_rows.push((member, rows));
            }
            next_anchor_stage = advance_diagnostic_stage(next_anchor_stage, 2)?;
        }
    }
    let provenance_paths = [
        path("provenance.jsonl"),
        path("graph-provenance.jsonl"),
        path("transfer-provenance.jsonl"),
        path("evaluation-provenance.jsonl"),
    ];
    let mut provenance_validation_stages = BTreeMap::<String, u32>::new();
    let mut next_provenance_stage = DIAG_STAGE_PROVENANCE;
    let mut provenance_rows = Vec::new();
    for provenance_path in &provenance_paths {
        checks.set_diagnostic_stage(next_provenance_stage);
        provenance_rows.extend(checks.jsonl(provenance_path, true)?.unwrap_or_default());
        let validation_stage = advance_diagnostic_stage(next_provenance_stage, 1)?;
        checks.charge(provenance_path.len() + size_of::<String>() + size_of::<u32>() + 48)?;
        provenance_validation_stages.insert(provenance_path.clone(), validation_stage);
        next_provenance_stage = advance_diagnostic_stage(next_provenance_stage, 2)?;
    }
    let mut supplemental_provenance_paths: Vec<String> = [
        "provenance.german-source-triangulation.jsonl",
        "provenance.bounded-translation-research-input.jsonl",
        "provenance.critical-edition-citation-decision.jsonl",
        "provenance.edition-reading-admission.jsonl",
        "provenance.translation-exposure-aware-plan.jsonl",
        "provenance.semantic-ladder-v4.jsonl",
        "provenance.semantic-source-observation-v1.jsonl",
        "provenance.semantic-source-recurrence-v1.jsonl",
    ]
    .into_iter()
    .map(|name| path(name))
    .collect();
    let mut present_supplemental_provenance_paths = Vec::new();
    for member in supplemental_provenance_paths {
        if checks.current_path_present(&member)? {
            present_supplemental_provenance_paths.push(member);
        }
    }
    let mut supplemental_provenance_paths = present_supplemental_provenance_paths;
    let mut episode_provenance_paths = Vec::<String>::new();
    let paths = checks.paths;
    paths.for_each_path(&mut |member| {
        checks.checkpoint()?;
        if member
            .strip_prefix(&episode_provenance_prefix)
            .is_some_and(|suffix| !suffix.contains('/') && suffix.ends_with(".jsonl"))
        {
            checks.charge(member.len() + size_of::<String>() + 32)?;
            episode_provenance_paths.push(member.to_owned());
        }
        Ok(())
    })?;
    episode_provenance_paths.sort();
    supplemental_provenance_paths.extend(episode_provenance_paths);
    for provenance_path in &supplemental_provenance_paths {
        checks.set_diagnostic_stage(next_provenance_stage);
        provenance_rows.extend(checks.jsonl(provenance_path, true)?.unwrap_or_default());
        let validation_stage = advance_diagnostic_stage(next_provenance_stage, 1)?;
        checks.charge(provenance_path.len() + size_of::<String>() + size_of::<u32>() + 48)?;
        provenance_validation_stages.insert(provenance_path.clone(), validation_stage);
        next_provenance_stage = advance_diagnostic_stage(next_provenance_stage, 2)?;
    }

    let sample_diagnostic_stage = next_anchor_stage;
    let transfer_diagnostic_stage = advance_diagnostic_stage(sample_diagnostic_stage, 1)?;
    let evaluations_diagnostic_stage = advance_diagnostic_stage(transfer_diagnostic_stage, 1)?;
    let ocr_diagnostic_stage = advance_diagnostic_stage(evaluations_diagnostic_stage, 1)?;
    let gold_status_diagnostic_stage = advance_diagnostic_stage(ocr_diagnostic_stage, 1)?;
    let gold_assurance_diagnostic_stage =
        advance_diagnostic_stage(gold_status_diagnostic_stage, 1)?;
    let translation_fragments_diagnostic_stage =
        advance_diagnostic_stage(gold_assurance_diagnostic_stage, 1)?;
    let retrieval_diagnostic_stage =
        advance_diagnostic_stage(translation_fragments_diagnostic_stage, 1)?;
    let visual_diagnostic_stage = advance_diagnostic_stage(retrieval_diagnostic_stage, 1)?;
    let graph_parse_diagnostic_stage = advance_diagnostic_stage(visual_diagnostic_stage, 1)?;
    let graph_claim_validation_stage = advance_diagnostic_stage(graph_parse_diagnostic_stage, 1)?;
    let graph_closure_diagnostic_stage = advance_diagnostic_stage(graph_claim_validation_stage, 1)?;
    let graph_plan_diagnostic_stage = advance_diagnostic_stage(graph_closure_diagnostic_stage, 1)?;
    let graph_provenance_diagnostic_stage =
        advance_diagnostic_stage(graph_plan_diagnostic_stage, 1)?;
    let anchor_closure_diagnostic_stage =
        advance_diagnostic_stage(graph_provenance_diagnostic_stage, 1)?;
    let local_content_diagnostic_stage =
        advance_diagnostic_stage(anchor_closure_diagnostic_stage, 1)?;
    let graph_claim_path = path("graph-claims.jsonl");
    checks.set_diagnostic_stage(graph_parse_diagnostic_stage);
    let graph_claim_rows = checks.jsonl(&graph_claim_path, true)?.unwrap_or_default();

    let receipt_path = path("transfer-source-visible-review.jenseits-187.v1.json");
    checks.set_diagnostic_stage(DIAG_STAGE_RECEIPT);
    if let Some(receipt) = optional_documents.get(&receipt_path) {
        checks.schema(
            &receipt.path,
            "ToS/contracts/transfer-source-visible-review-receipt.schema.json",
            &receipt.value,
        )?;
        let generator = receipt.value.get("generator").unwrap_or(&Value::Null);
        let generator_ref = generator.get("ref").and_then(Value::as_str);
        let generator_digest = generator.get("sha256").and_then(Value::as_str);
        // The receipt binds a retained historical generator, not whichever
        // implementation is currently installed at the old entrypoint.
        let generator_matches =
            if let (Some(reference), Some(digest)) = (generator_ref, generator_digest) {
                checks.recorded_digest_matches(reference, digest)?
            } else {
                false
            };
        if !generator_matches {
            checks.issue(
                &receipt.path,
                "source-visible review generator reference or digest drifted",
            )?;
        }
        let evidence_inputs = receipt.value.get("evidence_inputs").unwrap_or(&Value::Null);
        for side in ["source", "target"] {
            let witness = evidence_inputs.get(side).unwrap_or(&Value::Null);
            let rights_ref = witness.get("rights_ref").and_then(Value::as_str);
            let rights_digest = witness.get("rights_sha256").and_then(Value::as_str);
            let current_digest = if let Some(reference) = rights_ref {
                checks.current_digest(reference)?
            } else {
                None
            };
            if rights_ref.is_none()
                || current_digest.is_none()
                || current_digest.as_deref() != rights_digest
            {
                checks.issue(
                    &receipt.path,
                    format!("source-visible review {side} rights reference or digest drifted"),
                )?;
            }
        }
        let readiness_path = path("transfer-route-readiness.v1.json");
        let readiness_document = if checks.current_path_present(&readiness_path)? {
            checks.json(&readiness_path, true)?
        } else {
            None
        };
        let readiness = readiness_document.as_ref().map(|document| &document.value);
        let route_id = receipt
            .value
            .get("route_readiness_id")
            .unwrap_or(&Value::Null);
        let mut matching_routes: Vec<&Value> = Vec::new();
        for route in readiness
            .into_iter()
            .flat_map(|value| array(value, "routes"))
        {
            if checks.python_optional_equal(route.get("route_readiness_id"), Some(route_id))? {
                matching_routes.push(route);
            }
        }
        if matching_routes.len() != 1 {
            checks.issue(
                &receipt.path,
                "source-visible review route does not resolve exactly once in readiness projection",
            )?;
        } else {
            let route = matching_routes[0];
            for field in ["work_ref", "qualified_unit_key"] {
                if checks.python_different(route.get(field), receipt.value.get(field))? {
                    checks.issue(
                        &receipt.path,
                        format!("source-visible review {field} drifted from readiness route"),
                    )?;
                }
            }
            let mut authority_gate_open = false;
            for field in [
                "accepted_source_or_target_text",
                "source_to_target_passage_alignment",
                "eligible_for_variant_execution",
                "target_gold",
                "human_review_performed",
            ] {
                if checks.python_different(route.get(field), Some(&Value::Bool(false)))? {
                    authority_gate_open = true;
                    break;
                }
            }
            if authority_gate_open {
                checks.issue(
                    &receipt.path,
                    "source-visible review readiness route has an open authority gate",
                )?;
            }
        }
        if value_contains_any_text(&receipt.value, &["/srv/", "/home/"])
            || value_contains_exact_atom(
                &receipt.value,
                &[
                    "automatic_candidate_text",
                    "diplomatic_lines",
                    "text",
                    "finding",
                ],
            )
        {
            checks.issue(
                &receipt.path,
                "tracked source-visible review leaks text or an absolute private path",
            )?;
        }
    }

    let mut event_order = Vec::new();
    let mut local_event_ids = BTreeSet::new();
    let mut events_by_id = BTreeMap::new();
    for row in &provenance_rows {
        let row_path = row
            .location
            .rsplit_once(':')
            .map(|(source_path, _)| source_path)
            .unwrap_or(&row.location);
        if let Some(stage) = provenance_validation_stages.get(row_path) {
            checks.set_diagnostic_stage(*stage);
        }
        checks.schema(
            &row.location,
            "ToS/contracts/provenance-event.schema.json",
            &row.value,
        )?;
        for field in ["source_refs", "source_record_refs", "receipt_refs"] {
            for reference in array(&row.value, field) {
                if !checks.repository_ref_exists(reference)? {
                    checks.issue(
                        &row.location,
                        format!(
                            "repository reference does not exist: {}",
                            display(reference)
                        ),
                    )?;
                }
            }
        }
        for field in [
            "rights_ref",
            "provenance_ref",
            "forensic_report_ref",
            "resource_inventory_ref",
            "generated_from_manifest_ref",
            "item_manifest_ref",
        ] {
            if let Some(reference) = row.value.get(field) {
                if !checks.repository_ref_exists(reference)? {
                    checks.issue(
                        &row.location,
                        format!(
                            "repository reference does not exist: {}",
                            display(reference)
                        ),
                    )?;
                }
            }
        }
        if let Some(id) = row.value.get("event_id").and_then(Value::as_str) {
            let duplicate = source_events.event_contains(id)?
                || !checks.insert_string(seen_gold_event_ids, id)?;
            if duplicate {
                checks.issue(&row.location, format!("duplicate event_id: {id}"))?;
            }
            if !local_event_ids.contains(id) {
                checks.charge(
                    id.len()
                        .checked_mul(2)
                        .and_then(|bytes| bytes.checked_add(2 * size_of::<String>() + 96))
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                local_event_ids.insert(id.to_owned());
                event_order.push(id.to_owned());
            }
            checks.charge_clone(&row.value)?;
            checks.charge(id.len() + size_of::<String>() + 32)?;
            events_by_id.insert(id.to_owned(), row.value.clone());
        }
    }
    let mut anchors_by_id = BTreeMap::new();
    for row in anchor_rows
        .iter()
        .chain(translation_anchor_rows.iter())
        .chain(
            optional_anchor_rows
                .iter()
                .flat_map(|(_, rows)| rows.iter()),
        )
    {
        let row_path = row
            .location
            .rsplit_once(':')
            .map(|(source_path, _)| source_path)
            .unwrap_or(&row.location);
        if let Some(stage) = anchor_validation_stages.get(row_path) {
            checks.set_diagnostic_stage(*stage);
        }
        checks.schema(
            &row.location,
            "ToS/contracts/source-anchor.schema.json",
            &row.value,
        )?;
        let id = row.value.get("anchor_id").and_then(Value::as_str);
        if let Some(id) = id {
            checks.charge(
                id.len()
                    .checked_mul(2)
                    .and_then(|bytes| bytes.checked_add(2 * size_of::<String>() + 96))
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            if !checks.insert_string(seen_anchor_ids, id)? {
                checks.issue(&row.location, format!("duplicate anchor_id: {id}"))?;
            }
            checks.charge_clone(&row.value)?;
            anchors_by_id.insert(id.to_owned(), row.value.clone());
        }
        let item_ref = row.value.get("item_id").unwrap_or(&Value::Null);
        let item_exists = match item_ref.as_str() {
            Some(item_id) => records.current_record(item_id)?.is_some(),
            None => false,
        };
        if !item_exists {
            checks.issue(
                &row.location,
                format!("unresolved item_id: {}", display(item_ref)),
            )?;
        }
        let file_ref = row.value.get("file_id").unwrap_or(&Value::Null);
        if !records.file_contains(item_ref, file_ref)? {
            checks.issue(
                &row.location,
                format!(
                    "file_id {} does not belong to {}",
                    display(file_ref),
                    display(item_ref)
                ),
            )?;
        }
        let file_sha256 = records.file_sha256(file_ref)?;
        if checks.python_different(file_sha256.as_deref(), row.value.get("file_sha256"))? {
            checks.issue(
                &row.location,
                format!(
                    "file_sha256 does not match manifest file {}",
                    display(file_ref)
                ),
            )?;
        }
        if row
            .value
            .get("provenance_event_ref")
            .and_then(Value::as_str)
            .is_none_or(|event_ref| !local_event_ids.contains(event_ref))
        {
            checks.issue(
                &row.location,
                "anchor provenance_event_ref is absent from gold-set provenance",
            )?;
        }
    }

    // Preserve optional OCR companion presence pairing before evaluating the
    // full source-bound projection law below.
    let ocr_plan_path = path("ocr-visual-samples.json");
    let ocr_anchor_path = path("ocr-anchors.jsonl");
    let has_ocr_plan = checks.current_path_present(&ocr_plan_path)?;
    let has_ocr_anchors = checks.current_path_present(&ocr_anchor_path)?;
    let mut sample_bindings = BTreeMap::<String, SampleBinding>::new();
    let mut gold_sample_ids = BTreeSet::new();
    checks.set_diagnostic_stage(DIAG_STAGE_PACKET_SCHEMAS);
    if let Some(document) = &sample {
        checks.schema(
            &document.path,
            "ToS/contracts/laboratory-sample-plan.schema.json",
            &document.value,
        )?;
    }
    if let Some(ocr) = optional_documents.get(&ocr_plan_path) {
        checks.schema(
            &ocr.path,
            "ToS/contracts/ocr-visual-sample-plan.schema.json",
            &ocr.value,
        )?;
    }
    if has_ocr_plan != has_ocr_anchors {
        checks.issue(
            root,
            "OCR visual sample plan and OCR anchor companion must either both exist or both be absent",
        )?;
    }
    if let Some(document) = &sample {
        checks.set_diagnostic_stage(sample_diagnostic_stage);
        let groups = array(&document.value, "source_groups");
        if groups.len() != 3 {
            checks.issue(
                &document.path,
                "sample plan must have exactly three source groups",
            )?;
        }
        let mut sample_ids = BTreeSet::new();
        for group in groups {
            let Some(group) = group.as_object() else {
                continue;
            };
            let item_ref_input = group.get("item_ref").unwrap_or(&Value::Null);
            checks.charge_clone(item_ref_input)?;
            let item_ref = item_ref_input.clone();
            let file_ref_input = group.get("file_ref").unwrap_or(&Value::Null);
            checks.charge_clone(file_ref_input)?;
            let file_ref = file_ref_input.clone();
            let samples = group
                .get("samples")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let group_language = item_language(records, &item_ref)?;
            if !records.file_contains(&item_ref, &file_ref)? {
                checks.issue(
                    &document.path,
                    format!(
                        "file_ref {} does not belong to {}",
                        display(&file_ref),
                        display(&item_ref)
                    ),
                )?;
            }
            let file_sha256 = records.file_sha256(&file_ref)?;
            if checks.python_different(file_sha256.as_deref(), group.get("file_sha256"))? {
                checks.issue(
                    &document.path,
                    format!(
                        "file_sha256 differs from manifest file {}",
                        display(&file_ref)
                    ),
                )?;
            }
            if samples.len() != 12 {
                checks.issue(
                    &document.path,
                    format!("{} must have exactly 12 samples", display(&item_ref)),
                )?;
            }
            let mut group_gold = 0usize;
            for sample in samples {
                let Some(sample_obj) = sample.as_object() else {
                    continue;
                };
                let sample_id = sample_obj.get("sample_id").and_then(Value::as_str);
                let anchor_ref = sample_obj.get("anchor_ref").unwrap_or(&Value::Null);
                if sample_id.is_none() || sample_ids.contains(sample_id.unwrap_or_default()) {
                    checks.issue(
                        &document.path,
                        format!(
                            "duplicate or invalid sample_id: {}",
                            sample_obj
                                .get("sample_id")
                                .map(display)
                                .unwrap_or_else(|| "None".into())
                        ),
                    )?;
                } else if let Some(sample_id) = sample_id {
                    checks.charge_clone(sample)?;
                    checks.charge_clone(&item_ref)?;
                    checks.charge_clone(&file_ref)?;
                    checks.charge_clone(anchor_ref)?;
                    checks.charge(
                        sample_id
                            .len()
                            .checked_mul(2)
                            .and_then(|bytes| bytes.checked_add(2 * size_of::<String>() + 64))
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    if let Some(language) = &group_language {
                        checks.charge(language.len() + size_of::<String>() + 32)?;
                    }
                    checks.insert_string(&mut sample_ids, sample_id)?;
                    sample_bindings.insert(
                        sample_id.to_owned(),
                        SampleBinding {
                            sample: sample.clone(),
                            item_ref: item_ref.clone(),
                            file_ref: file_ref.clone(),
                            anchor_ref: anchor_ref.clone(),
                            language: group_language.clone(),
                        },
                    );
                }
                let anchor_id = anchor_ref.as_str();
                if let Some(anchor_id) = anchor_id {
                    let Some(anchor) = anchors_by_id.get(anchor_id) else {
                        checks.issue(
                            &document.path,
                            format!("unresolved sample anchor: {anchor_id}"),
                        )?;
                        if sample_obj.get("gold_candidate") == Some(&Value::Bool(true)) {
                            group_gold += 1;
                            if let Some(sample_id) = sample_id {
                                checks.insert_string(&mut gold_sample_ids, sample_id)?;
                            }
                        }
                        continue;
                    };
                    if checks.python_different(anchor.get("item_id"), Some(&item_ref))?
                        || checks.python_different(anchor.get("file_id"), Some(&file_ref))?
                    {
                        checks.issue(
                            &document.path,
                            format!("sample anchor {anchor_id} crosses its source group"),
                        )?;
                    }
                } else {
                    checks.issue(
                        &document.path,
                        format!("unresolved sample anchor: {}", display(&anchor_ref)),
                    )?;
                }
                if sample_obj.get("gold_candidate") == Some(&Value::Bool(true)) {
                    group_gold += 1;
                    if let Some(sample_id) = sample_id {
                        checks.insert_string(&mut gold_sample_ids, sample_id)?;
                    }
                }
            }
            if group_gold != 5 {
                checks.issue(
                    &document.path,
                    format!(
                        "{} must have exactly five gold candidates",
                        display(&item_ref)
                    ),
                )?;
            }
        }
    }

    if let Some(document) = &status {
        checks.set_diagnostic_stage(DIAG_STAGE_PACKET_SCHEMAS);
        checks.schema(
            &document.path,
            "ToS/contracts/manual-gold-status.schema.json",
            &document.value,
        )?;
        checks.set_diagnostic_stage(gold_status_diagnostic_stage);
        let units = array(&document.value, "units");
        let status_ids: BTreeSet<String> = units
            .iter()
            .filter_map(|unit| {
                unit.get("sample_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        if status_ids != gold_sample_ids {
            checks.issue(
                &document.path,
                "gold-status units differ from frozen gold candidates",
            )?;
        }
        for unit in units {
            let Some(unit) = unit.as_object() else {
                continue;
            };
            let sample_id = unit
                .get("sample_id")
                .map(display)
                .unwrap_or_else(|| "None".into());
            if unit.get("gold_status").and_then(Value::as_str) != Some("human_double_checked") {
                continue;
            }
            if !unit.get("content_sha256").is_some_and(Value::is_string) {
                checks.issue(
                    &document.path,
                    format!("{sample_id} claims gold without a content digest"),
                )?;
            }
            for field in ["human_pass_1", "human_pass_2"] {
                let review = unit.get(field).and_then(Value::as_object);
                if review.and_then(|r| r.get("status")).and_then(Value::as_str) != Some("complete")
                {
                    checks.issue(
                        &document.path,
                        format!("{sample_id} claims gold without complete {field}"),
                    )?;
                }
                for evidence_field in ["maker_ref", "completed_at", "receipt_ref"] {
                    if !review
                        .and_then(|r| r.get(evidence_field))
                        .is_some_and(Value::is_string)
                    {
                        checks.issue(
                            &document.path,
                            format!("{sample_id} claims gold without {field}.{evidence_field}"),
                        )?;
                    }
                }
            }
        }
    }

    if let Some(document) = &assurance {
        checks.set_diagnostic_stage(DIAG_STAGE_PACKET_SCHEMAS);
        checks.schema(
            &document.path,
            "ToS/contracts/manual-gold-assurance.schema.json",
            &document.value,
        )?;
        checks.set_diagnostic_stage(gold_assurance_diagnostic_stage);
        let method_path =
            "ToS/research-packets/foundation-laboratory-2026-07/HUMAN_ASSURANCE_RESEARCH.md";
        check_digest_binding(
            &mut checks,
            &document.value,
            "legacy_gold_status",
            status.as_ref(),
            &path(GOLD_STATUS),
            &document.path,
        )?;
        check_digest_binding(
            &mut checks,
            &document.value,
            "sample_plan",
            sample.as_ref(),
            &path(SAMPLE_PLAN),
            &document.path,
        )?;
        check_external_digest_binding(
            &mut checks,
            &document.value,
            "method_research",
            method_path,
            &document.path,
        )?;

        let units = array(&document.value, "units");
        let mut assurance_units = BTreeMap::new();
        for unit in units {
            if let (Some(id), Some(unit_obj)) = (
                unit.get("sample_id").and_then(Value::as_str),
                unit.as_object(),
            ) {
                let encoded = encoded_len(unit)?;
                checks.charge(
                    encoded
                        .checked_mul(4)
                        .and_then(|bytes| bytes.checked_add(id.len() + size_of::<String>() + 96))
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                assurance_units.insert(id.to_owned(), Value::Object(unit_obj.clone()));
            }
        }
        if assurance_units.keys().cloned().collect::<BTreeSet<_>>() != gold_sample_ids {
            checks.issue(
                &document.path,
                "gold-assurance units differ from frozen gold candidates",
            )?;
        }
        let schedule = document
            .value
            .get("human_work_schedule")
            .unwrap_or(&Value::Null);
        for message in human_work_schedule_issues(&assurance_units, schedule)? {
            checks.issue(&document.path, message)?;
        }
        for field in ["runtime_closure", "runtime_receipt", "source_autosave"] {
            // The owner-local artifact store is an external custody owner. This
            // current-source district leaves its optional byte check to that
            // owner rather than treating a declaration as custody evidence.
            let _ = document
                .value
                .get("human_work_schedule")
                .and_then(|s| s.get(field));
        }
        let scopes = array(&document.value, "language_scopes");
        let mut language_scopes = BTreeMap::<String, Value>::new();
        for scope in scopes {
            if let Some(language) = scope.get("language").and_then(Value::as_str) {
                checks.charge(
                    encoded_len(scope)?
                        .checked_mul(4)
                        .and_then(|bytes| {
                            bytes.checked_add(language.len() + size_of::<String>() + 64)
                        })
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                language_scopes.insert(language.to_owned(), scope.clone());
            }
        }
        if language_scopes.len() != scopes.len() {
            checks.issue(&document.path, "language scopes must be unique")?;
        }
        for scope in language_scopes.values() {
            if let Some(message) = language_scope_overlap_issue(scope) {
                checks.issue(&document.path, message)?;
            }
        }
        let minimum_delay = document
            .value
            .get("solo_recheck_policy")
            .and_then(|v| v.get("minimum_delay_hours"))
            .unwrap_or(&Value::Null);
        let mut legacy_units = BTreeMap::<String, Value>::new();
        if let Some(status) = &status {
            for unit in array(&status.value, "units") {
                let Some(id) = unit.get("sample_id").and_then(Value::as_str) else {
                    continue;
                };
                checks.charge(
                    encoded_len(unit)?
                        .checked_mul(4)
                        .and_then(|bytes| bytes.checked_add(id.len() + size_of::<String>() + 96))
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                legacy_units.insert(id.to_owned(), unit.clone());
            }
        }
        let mut observed_languages = BTreeSet::new();
        for (sample_id, unit) in &assurance_units {
            let binding = sample_bindings.get(sample_id);
            if checks.python_different(unit.get("anchor_ref"), binding.map(|b| &b.anchor_ref))? {
                checks.issue(
                    &document.path,
                    format!("{sample_id} assurance anchor differs from the frozen sample"),
                )?;
            }
            let expected_language = binding.and_then(|b| b.language.as_deref());
            let actual_language = unit.get("language").and_then(Value::as_str);
            if actual_language != expected_language {
                checks.issue(
                    &document.path,
                    format!("{sample_id} assurance language differs from its source group"),
                )?;
            }
            if let Some(language) = actual_language {
                observed_languages.insert(language.to_owned());
            }
            let scope = actual_language.and_then(|language| language_scopes.get(language));
            if scope.is_none() {
                checks.issue(
                    &document.path,
                    format!("{sample_id} has no language competence scope"),
                )?;
            } else if unit.get("competence_level")
                != scope.and_then(|s| s.get("planned_competence"))
            {
                checks.issue(
                    &document.path,
                    format!("{sample_id} competence differs from its language scope"),
                )?;
            }
            let assurance_kind = unit.get("current_assurance").and_then(Value::as_str);
            let evidence = unit.get("review_evidence").and_then(Value::as_object);
            if matches!(
                assurance_kind,
                Some("unreviewed" | "language_competence_blocked")
            ) && evidence.is_some_and(|values| values.values().any(|value| !value.is_null()))
            {
                checks.issue(
                    &document.path,
                    format!("{sample_id} has review evidence before review"),
                )?;
            }
            if assurance_kind == Some("language_competence_blocked")
                && scope
                    .and_then(|s| s.get("text_review_status"))
                    .and_then(Value::as_str)
                    != Some("language_competence_blocked")
            {
                checks.issue(
                    &document.path,
                    format!("{sample_id} language block is absent from its scope"),
                )?;
            }
            if assurance_kind == Some("solo_human_delayed_rechecked") {
                if let Some(message) = solo_recheck_delay_issue(unit, minimum_delay) {
                    checks.issue(&document.path, message)?;
                }
                if evidence.and_then(|e| e.get("same_reviewer")) != Some(&Value::Bool(true)) {
                    checks.issue(
                        &document.path,
                        format!("{sample_id} solo recheck must disclose the same reviewer"),
                    )?;
                }
            }
            if unit.get("reference_use").and_then(Value::as_str)
                == Some("independent_multi_human_gold")
                && assurance_kind != Some("independent_multi_human_adjudicated")
            {
                checks.issue(
                    &document.path,
                    format!("{sample_id} claims independent multi-human gold without adjudication"),
                )?;
            }
            if let Some(message) = assurance_reference_use_issue(unit) {
                checks.issue(&document.path, message)?;
            }
            if legacy_units
                .get(sample_id)
                .and_then(|u| u.get("gold_status"))
                .and_then(Value::as_str)
                == Some("human_double_checked")
                && matches!(
                    assurance_kind,
                    Some("unreviewed" | "language_competence_blocked")
                )
            {
                checks.issue(
                    &document.path,
                    format!("{sample_id} assurance does not carry forward legacy human review"),
                )?;
            }
        }
        if language_scopes
            .keys()
            .cloned()
            .collect::<BTreeSet<String>>()
            != observed_languages
        {
            checks.issue(
                &document.path,
                "language scopes differ from the frozen gold-unit languages",
            )?;
        }
    }

    if let Some(document) = &translation {
        checks.set_diagnostic_stage(DIAG_STAGE_PACKET_SCHEMAS);
        checks.schema(
            &document.path,
            "ToS/contracts/translation-sample-plan.schema.json",
            &document.value,
        )?;
        checks.set_diagnostic_stage(translation_fragments_diagnostic_stage);
        let mut fragment_ids = BTreeSet::new();
        let fragments = array(&document.value, "fragments");
        if fragments.len() != 30 {
            checks.issue(
                &document.path,
                "translation plan must freeze exactly 30 fragments",
            )?;
        }
        for fragment in fragments {
            let fragment_id = fragment.get("fragment_id").and_then(Value::as_str);
            let anchor_ref = fragment.get("source_anchor_ref").and_then(Value::as_str);
            if fragment_id.is_none()
                || !fragment_ids.insert(fragment_id.unwrap_or_default().to_owned())
            {
                checks.issue(
                    &document.path,
                    format!(
                        "duplicate or invalid fragment_id: {}",
                        fragment
                            .get("fragment_id")
                            .map(display)
                            .unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            let Some(anchor) = anchor_ref.and_then(|id| anchors_by_id.get(id)) else {
                checks.issue(
                    &document.path,
                    format!(
                        "unresolved translation anchor: {}",
                        fragment
                            .get("source_anchor_ref")
                            .map(display)
                            .unwrap_or_else(|| "None".into())
                    ),
                )?;
                continue;
            };
            let member_selectors: Vec<&Value> = array(anchor, "selectors")
                .iter()
                .filter(|selector| {
                    selector.get("type").and_then(Value::as_str) == Some("container_member")
                })
                .collect();
            if let Some(selector) = member_selectors.first() {
                if checks.python_different(
                    selector.get("member_path"),
                    fragment.get("container_member"),
                )? || checks.python_different(
                    selector.get("member_sha256"),
                    fragment.get("member_sha256"),
                )? {
                    checks.issue(
                        &document.path,
                        format!(
                            "fragment metadata differs from anchor: {}",
                            anchor_ref.unwrap_or("None")
                        ),
                    )?;
                }
            } else {
                checks.issue(
                    &document.path,
                    format!(
                        "translation anchor lacks member selector: {}",
                        anchor_ref.unwrap_or("None")
                    ),
                )?;
            }
        }
    }

    if let Some(document) = &transfer {
        checks.set_diagnostic_stage(DIAG_STAGE_PACKET_SCHEMAS);
        checks.schema(
            &document.path,
            "ToS/contracts/golden-kernel-transfer-plan.schema.json",
            &document.value,
        )?;
        checks.set_diagnostic_stage(transfer_diagnostic_stage);
        let source_binding = document
            .value
            .get("source_sample_plan")
            .unwrap_or(&Value::Null);
        let expected_sample_path = path(SAMPLE_PLAN);
        if source_binding.get("ref").and_then(Value::as_str) != Some(expected_sample_path.as_str())
        {
            checks.issue(
                &document.path,
                "transfer plan does not cite the frozen source sample plan",
            )?;
        } else if source_binding.get("sha256").and_then(Value::as_str)
            != sample.as_ref().map(|d| d.sha256.as_str())
        {
            checks.issue(&document.path, "transfer source sample-plan digest drifted")?;
        }
        let target_source = document.value.get("target_source").unwrap_or(&Value::Null);
        let target_item = target_source
            .get("collection_item_ref")
            .cloned()
            .unwrap_or(Value::Null);
        let target_file = target_source
            .get("file_ref")
            .cloned()
            .unwrap_or(Value::Null);
        if !records.file_contains(&target_item, &target_file)? {
            checks.issue(
                &document.path,
                "transfer target file does not belong to its collection item",
            )?;
        }
        let file_sha256 = records.file_sha256(&target_file)?;
        if checks.python_different(file_sha256.as_deref(), target_source.get("file_sha256"))? {
            checks.issue(&document.path, "transfer target file digest drifted")?;
        }
        if let Some(rights_ref) = target_source
            .get("rights_record_ref")
            .and_then(Value::as_str)
        {
            if let Some(rights) = checks.json(rights_ref, true)? {
                if checks.python_different(
                    rights.value.get("assessment_status"),
                    target_source.get("rights_assessment_status"),
                )? {
                    checks.issue(&document.path, "transfer target rights assessment drifted")?;
                }
                let scope_refs = rights.value.get("scope_refs");
                if let Some(scope_refs) = scope_refs.and_then(Value::as_array) {
                    let mut scope_covers_item = false;
                    for scope_ref in scope_refs {
                        if checks.python_equal(scope_ref, &target_item)? {
                            scope_covers_item = true;
                            break;
                        }
                    }
                    if !scope_covers_item {
                        checks.issue(
                            &document.path,
                            "transfer target rights record omits its item",
                        )?;
                    }
                }
            }
        }
        let expected_transfer_ids: BTreeSet<String> = sample_bindings
            .iter()
            .filter(|(_, binding)| {
                array(&binding.sample, "strata")
                    .iter()
                    .any(|s| s.as_str() == Some("cross-work-transfer"))
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut actual_transfer_ids = BTreeSet::new();
        for unit in array(&document.value, "scouting_units") {
            let id = unit.get("sample_id").and_then(Value::as_str);
            let Some(id) = id else {
                checks.issue(
                    &document.path,
                    "duplicate or invalid transfer sample_id: None",
                )?;
                continue;
            };
            if !actual_transfer_ids.insert(id.to_owned()) {
                checks.issue(
                    &document.path,
                    format!("duplicate or invalid transfer sample_id: {id}"),
                )?;
                continue;
            }
            let Some(binding) = sample_bindings.get(id) else {
                checks.issue(
                    &document.path,
                    format!("transfer scouting unit is not in sample plan: {id}"),
                )?;
                continue;
            };
            for (field, expected) in [
                ("anchor_ref", &binding.anchor_ref),
                ("item_ref", &binding.item_ref),
                ("file_ref", &binding.file_ref),
            ] {
                if checks.python_different(unit.get(field), Some(expected))? {
                    checks.issue(
                        &document.path,
                        format!("{id}.{field} drifted from sample plan"),
                    )?;
                }
            }
            if checks.python_different(
                unit.get("source_review_status"),
                binding.sample.get("source_review_status"),
            )? {
                checks.issue(
                    &document.path,
                    format!("{id}.source_review_status drifted from sample plan"),
                )?;
            }
            let anchor = binding
                .anchor_ref
                .as_str()
                .and_then(|ref_id| anchors_by_id.get(ref_id));
            let selectors = anchor.map(|a| array(a, "selectors")).unwrap_or(&[]);
            let pages: Vec<&Value> = selectors
                .iter()
                .filter(|s| s.get("type").and_then(Value::as_str) == Some("page_region"))
                .collect();
            if pages.len() != 1
                || checks.python_different(
                    unit.get("page"),
                    pages.first().and_then(|page| page.get("page")),
                )?
            {
                checks.issue(
                    &document.path,
                    format!("{id}.page drifted from its exact anchor"),
                )?;
            }
        }
        if actual_transfer_ids != expected_transfer_ids {
            checks.issue(
                &document.path,
                "transfer scouting units do not close over the exact cross-work sample stratum",
            )?;
        }
        let variant_labels: Vec<&str> = array(&document.value, "variants")
            .iter()
            .filter_map(|v| v.get("label").and_then(Value::as_str))
            .collect();
        if variant_labels != ["A", "B", "C"] {
            checks.issue(&document.path, "transfer variants are not ordered A/B/C")?;
        }
        let expected_metrics: BTreeSet<&str> = [
            "annotation-accuracy",
            "annotation-speed",
            "manual-correction-minutes",
            "traceable-proposal-rate",
            "hallucinated-relation-rate",
            "reusable-sign-utility",
            "zarathustra-ontology-imposition-rate",
            "machine-cost",
        ]
        .into_iter()
        .collect();
        let actual_metrics: BTreeSet<&str> = array(&document.value, "metrics")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if actual_metrics != expected_metrics {
            checks.issue(&document.path, "transfer metric set is incomplete")?;
        }
        let actual_gold_count = status
            .as_ref()
            .map(|d| {
                array(&d.value, "units")
                    .iter()
                    .filter(|u| {
                        u.get("gold_status").and_then(Value::as_str) == Some("human_double_checked")
                    })
                    .count()
            })
            .unwrap_or(0);
        let gate = document
            .value
            .get("kernel_evidence_gate")
            .unwrap_or(&Value::Null);
        if checks.python_different(
            gate.get("human_double_checked_gold_units"),
            Some(&Value::from(actual_gold_count as u64)),
        )? {
            checks.issue(&document.path, "transfer kernel gold count drifted")?;
        }
        let target_units: Vec<&Value> = array(&document.value, "target_units")
            .iter()
            .filter(|unit| unit.is_object())
            .collect();
        checks.charge(
            target_units
                .len()
                .checked_mul(size_of::<&Value>() + size_of::<Option<&Value>>() + 32)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let target_ids: Vec<Option<&Value>> = target_units
            .iter()
            .map(|target| target.get("unit_id"))
            .collect();
        let mut target_gold = 0usize;
        for target in target_units {
            if target.get("target_gold_status").and_then(Value::as_str)
                == Some("human_double_checked")
            {
                target_gold += 1;
            }
        }
        if checks.python_values_have_duplicates(&target_ids)? {
            checks.issue(
                &document.path,
                "transfer target-unit identities are not unique",
            )?;
        }
        if checks.python_different(
            gate.get("human_double_checked_target_units"),
            Some(&Value::from(target_gold as u64)),
        )? {
            checks.issue(&document.path, "transfer target-gold count drifted")?;
        }
        let candidate_units: Vec<&Value> = array(&document.value, "candidate_target_units")
            .iter()
            .filter(|unit| unit.is_object())
            .collect();
        checks.charge(
            candidate_units
                .len()
                .checked_mul(size_of::<&Value>() * 2 + size_of::<Option<&Value>>() * 3 + 64)
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let candidate_unit_ids: Vec<Option<&Value>> = candidate_units
            .iter()
            .map(|unit| unit.get("unit_id"))
            .collect();
        let candidate_pages: Vec<(Option<&Value>, Option<&Value>)> = candidate_units
            .iter()
            .map(|unit| (unit.get("file_ref"), unit.get("page")))
            .collect();
        let candidate_content_refs: Vec<Option<&Value>> = candidate_units
            .iter()
            .map(|unit| unit.get("source_content_ref"))
            .collect();
        if checks.python_values_have_duplicates(&candidate_unit_ids)? {
            checks.issue(
                &document.path,
                "transfer candidate-unit identities are not unique",
            )?;
        }
        if checks.python_pairs_have_duplicates(&candidate_pages)? {
            checks.issue(&document.path, "transfer candidate pages are not unique")?;
        }
        if checks.python_values_have_duplicates(&candidate_content_refs)? {
            checks.issue(
                &document.path,
                "transfer candidate local-content refs are not unique",
            )?;
        }
        let mut candidate_target_overlap = false;
        'overlap: for candidate_id in &candidate_unit_ids {
            for target_id in &target_ids {
                if checks.python_optional_equal(*candidate_id, *target_id)? {
                    candidate_target_overlap = true;
                    break 'overlap;
                }
            }
        }
        if candidate_target_overlap {
            checks.issue(
                &document.path,
                "prepared candidate and eligible target identities overlap",
            )?;
        }
        let candidate_preparation = document
            .value
            .get("candidate_preparation")
            .unwrap_or(&Value::Null);
        if !candidate_units.is_empty() {
            let builder = candidate_preparation.get("builder").unwrap_or(&Value::Null);
            let builder_ref = builder.get("ref").and_then(Value::as_str);
            let expected_builder = "scripts/build_golden_kernel_transfer_candidates.py";
            if builder_ref != Some(expected_builder)
                || checks.current_digest(expected_builder)?.is_none()
            {
                checks.issue(
                    &document.path,
                    "transfer candidate builder ref is unresolved",
                )?;
            } else {
                let builder_digest = builder.get("sha256").and_then(Value::as_str);
                let matches = if let Some(digest) = builder_digest {
                    checks.recorded_digest_matches(expected_builder, digest)?
                } else {
                    false
                };
                if !matches {
                    checks.issue(&document.path, "transfer candidate builder digest drifted")?;
                }
            }
            let actual_strata: BTreeMap<&str, usize> = ["random", "hard"]
                .into_iter()
                .map(|stratum| {
                    (
                        stratum,
                        candidate_units
                            .iter()
                            .filter(|unit| {
                                unit.get("stratum").and_then(Value::as_str) == Some(stratum)
                            })
                            .count(),
                    )
                })
                .collect();
            if actual_strata.get("random") != Some(&10) || actual_strata.get("hard") != Some(&10) {
                checks.issue(
                    &document.path,
                    "transfer candidate strata are not exactly 10 random and 10 hard",
                )?;
            }
            let mut expected_quotas = BTreeMap::<String, (Option<&Value>, Option<&Value>)>::new();
            for quota in array(candidate_preparation, "work_quotas") {
                if let Some(work_ref) = quota.get("work_ref").and_then(Value::as_str) {
                    expected_quotas.insert(
                        work_ref.to_owned(),
                        (quota.get("random"), quota.get("hard")),
                    );
                }
            }
            let mut quota_drifted = false;
            for (work_ref, (expected_random, expected_hard)) in &expected_quotas {
                let random = candidate_units
                    .iter()
                    .filter(|unit| {
                        unit.get("work_ref").and_then(Value::as_str) == Some(work_ref)
                            && unit.get("stratum").and_then(Value::as_str) == Some("random")
                    })
                    .count() as u64;
                let hard = candidate_units
                    .iter()
                    .filter(|unit| {
                        unit.get("work_ref").and_then(Value::as_str) == Some(work_ref)
                            && unit.get("stratum").and_then(Value::as_str) == Some("hard")
                    })
                    .count() as u64;
                let actual_random = Value::from(random);
                let actual_hard = Value::from(hard);
                if checks.python_optional_equal(Some(&actual_random), *expected_random)? == false
                    || checks.python_optional_equal(Some(&actual_hard), *expected_hard)? == false
                {
                    quota_drifted = true;
                    break;
                }
            }
            if quota_drifted {
                checks.issue(&document.path, "transfer candidate work quotas drifted")?;
            }
        }
        for candidate in candidate_units.iter().copied() {
            let unit_id = candidate
                .get("unit_id")
                .map(display)
                .unwrap_or_else(|| "None".into());
            let anchor = candidate
                .get("anchor_ref")
                .and_then(Value::as_str)
                .and_then(|anchor_id| anchors_by_id.get(anchor_id));
            let page_selectors: Vec<&Value> = anchor
                .map(|anchor| {
                    array(anchor, "selectors")
                        .iter()
                        .filter(|selector| {
                            selector.get("type").and_then(Value::as_str) == Some("page_region")
                        })
                        .collect()
                })
                .unwrap_or_default();
            if page_selectors.len() != 1
                || checks.python_different(
                    page_selectors
                        .first()
                        .and_then(|selector| selector.get("page")),
                    candidate.get("page"),
                )?
            {
                checks.issue(
                    &document.path,
                    format!("{unit_id} does not resolve to its exact whole-page anchor"),
                )?;
            } else {
                let selector = page_selectors[0];
                let mut page_shape_drifted = false;
                for (field, expected) in [("x", 0), ("y", 0), ("width", 1), ("height", 1)] {
                    if checks.python_different(selector.get(field), Some(&Value::from(expected)))? {
                        page_shape_drifted = true;
                        break;
                    }
                }
                if page_shape_drifted
                    || selector.get("coordinate_space").and_then(Value::as_str)
                        != Some("normalized_0_1")
                {
                    checks.issue(
                        &document.path,
                        format!("{unit_id} anchor is not the exact whole page"),
                    )?;
                }
            }
            if checks.python_different(
                anchor.and_then(|anchor| anchor.get("item_id")),
                candidate.get("item_ref"),
            )? {
                checks.issue(
                    &document.path,
                    format!("{unit_id} item differs from its anchor"),
                )?;
            }
            if checks.python_different(
                anchor.and_then(|anchor| anchor.get("file_id")),
                candidate.get("file_ref"),
            )? {
                checks.issue(
                    &document.path,
                    format!("{unit_id} file differs from its anchor"),
                )?;
            }
            let work_ref = candidate.get("work_ref").and_then(Value::as_str);
            let work = match work_ref {
                Some(work_ref) => records.current_record(work_ref)?,
                None => None,
            };
            if work.as_deref().is_none_or(|record| record.kind != "work") {
                checks.issue(&document.path, format!("{unit_id} work_ref is unresolved"))?;
            }
            let expression_ref = candidate.get("expression_ref").and_then(Value::as_str);
            let expression = match expression_ref {
                Some(expression_ref) => records.current_record(expression_ref)?,
                None => None,
            };
            let expression_wrong = match expression.as_deref() {
                Some(record) => {
                    record.kind != "expression"
                        || checks.python_different(
                            record.value.get("work_ref"),
                            candidate.get("work_ref"),
                        )?
                }
                None => true,
            };
            if expression_wrong {
                checks.issue(
                    &document.path,
                    format!("{unit_id} expression does not belong to its work"),
                )?;
            }
            if !records.file_contains(
                candidate.get("item_ref").unwrap_or(&Value::Null),
                candidate.get("file_ref").unwrap_or(&Value::Null),
            )? {
                checks.issue(
                    &document.path,
                    format!("{unit_id} file does not belong to its item"),
                )?;
            }
            let expected_page_id = candidate
                .get("page")
                .map(python_int)
                .unwrap_or(Some(0))
                .map(|page| format!("pdf-page-{page:04}"));
            if candidate.get("page_resource_id").and_then(Value::as_str)
                != expected_page_id.as_deref()
            {
                checks.issue(
                    &document.path,
                    format!("{unit_id} page resource identity drifted"),
                )?;
            }
            let content_ref = candidate.get("source_content_ref").and_then(Value::as_str);
            let content_route_is_safe = content_ref.is_some_and(|reference| {
                RelativePath::parse(reference).is_ok()
                    && reference.starts_with(&format!("{root}/local-content/transfer-targets/v1/"))
            });
            if !content_route_is_safe {
                checks.issue(
                    &document.path,
                    format!("{unit_id} content escaped the private transfer lane"),
                )?;
            }
            if let Some(content_ref) = content_ref {
                let facts = physical.private_paths.get(content_ref);
                if facts.and_then(|facts| facts.git_ignored) != Some(true) {
                    checks.issue(
                        &document.path,
                        format!("{unit_id} local content is not protected by Git ignore"),
                    )?;
                }
                if let Some(facts) = facts {
                    match physical_target_observation(facts) {
                        PhysicalTargetObservation::Target(target) => {
                            let target_is_transfer_lane =
                                target.relative_target.is_none_or(|relative_target| {
                                    RelativePath::parse(relative_target).is_ok()
                                        && relative_target
                                            .strip_prefix(&format!("{root}/local-content/"))
                                            .is_some_and(|suffix| {
                                                suffix.starts_with("transfer-targets/v1/")
                                            })
                                });
                            if content_route_is_safe && !target_is_transfer_lane {
                                checks.issue(
                                    &document.path,
                                    format!("{unit_id} content escaped the private transfer lane"),
                                )?;
                            }
                            if target.exists && target.regular_file {
                                if let Some(byte_size) = target.byte_size {
                                    if checks.python_different(
                                        Some(&Value::from(byte_size)),
                                        candidate.get("source_content_bytes"),
                                    )? {
                                        checks.issue(
                                            content_ref,
                                            "transfer candidate byte count drifted",
                                        )?;
                                    }
                                } else {
                                    checks.coverage_gap(format!(
                                        "{content_ref}: transfer candidate byte-size observation is unavailable"
                                    ))?;
                                }
                                if let Some(actual_digest) = target.sha256 {
                                    if Some(actual_digest)
                                        != candidate
                                            .get("source_content_sha256")
                                            .and_then(Value::as_str)
                                    {
                                        checks.issue(
                                            content_ref,
                                            "transfer candidate digest drifted",
                                        )?;
                                    }
                                } else {
                                    checks.coverage_gap(format!(
                                        "{content_ref}: transfer candidate digest observation is unavailable"
                                    ))?;
                                }
                            } else if require_local_payloads {
                                checks.issue(
                                    content_ref,
                                    "required local transfer candidate is missing",
                                )?;
                            }
                        }
                        PhysicalTargetObservation::OutsideSelectedRoot => {
                            if content_route_is_safe {
                                checks.issue(
                                    &document.path,
                                    format!("{unit_id} content escaped the private transfer lane"),
                                )?;
                            }
                        }
                        PhysicalTargetObservation::Unknown => {
                            checks.coverage_gap(format!(
                                "{content_ref}: transfer candidate symlink target is not observed"
                            ))?;
                        }
                    }
                } else {
                    checks.coverage_gap(format!(
                        "{content_ref}: transfer candidate physical observation is unavailable"
                    ))?;
                }
            }
        }
        let source_gate = optional_documents
            .get(&path("translation-laboratory-plan.v1.json"))
            .and_then(|document| document.value.get("source_review_gate"));
        if checks.python_different(
            gate.get("accepted_source_units"),
            source_gate.and_then(|value| value.get("current_human_accepted_units")),
        )? {
            checks.issue(&document.path, "transfer accepted-source count drifted")?;
        }
        let transfer_event = document
            .value
            .get("provenance_event_ref")
            .and_then(Value::as_str)
            .and_then(|event_id| events_by_id.get(event_id));
        if transfer_event.is_none() {
            checks.issue(
                &document.path,
                "transfer provenance_event_ref is absent from gold-set provenance",
            )?;
        } else if let Some(event) = transfer_event {
            let output = array(event, "outputs").iter().find(|output| {
                output.get("ref").and_then(Value::as_str) == Some(document.path.as_str())
            });
            if output.is_none() {
                checks.issue(&document.path, "transfer provenance omits its plan")?;
            } else if output.and_then(|v| v.get("sha256")).and_then(Value::as_str)
                != Some(document.sha256.as_str())
            {
                checks.issue(&document.path, "transfer provenance digest drifted")?;
            }
            if !candidate_units.is_empty() {
                let mut candidate_output_events = vec![event];
                if event
                    .get("method")
                    .and_then(|method| method.get("configuration"))
                    .and_then(|configuration| configuration.get("content_candidates_rebuilt"))
                    == Some(&Value::Bool(false))
                {
                    let superseded_ref = event.get("supersedes_event_ref").and_then(Value::as_str);
                    let superseded_event = superseded_ref.and_then(|id| events_by_id.get(id));
                    if let Some(superseded_event) = superseded_event {
                        candidate_output_events.push(superseded_event);
                    } else {
                        checks.issue(
                            &document.path,
                            "rights-only transfer provenance has no superseded candidate event",
                        )?;
                    }
                }
                let candidate_anchor_path = path("transfer-target-anchors.v1.jsonl");
                let candidate_anchor_digest = checks.current_digest(&candidate_anchor_path)?;
                let anchor_output_found = candidate_output_events
                    .iter()
                    .flat_map(|event| array(event, "outputs"))
                    .any(|output| {
                        output.get("ref").and_then(Value::as_str)
                            == Some(candidate_anchor_path.as_str())
                            && output.get("role").and_then(Value::as_str)
                                == Some("proposed-whole-page-transfer-candidate-anchors")
                            && output.get("sha256").and_then(Value::as_str)
                                == candidate_anchor_digest.as_deref()
                    });
                if !anchor_output_found {
                    checks.issue(
                        &document.path,
                        "transfer provenance candidate-anchor output drifted",
                    )?;
                }
                let expected_local_outputs: BTreeSet<_> = candidate_units
                    .iter()
                    .map(|candidate| {
                        (
                            candidate
                                .get("source_content_ref")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            Some("gitignored-local-only-pdftotext-page-candidate".to_owned()),
                            candidate
                                .get("source_content_sha256")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        )
                    })
                    .collect();
                let actual_local_outputs: BTreeSet<_> = candidate_output_events
                    .iter()
                    .flat_map(|event| array(event, "outputs"))
                    .map(|output| {
                        (
                            output.get("ref").and_then(Value::as_str).map(str::to_owned),
                            output
                                .get("role")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            output
                                .get("sha256")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        )
                    })
                    .collect();
                if !expected_local_outputs.is_subset(&actual_local_outputs) {
                    checks.issue(
                        &document.path,
                        "transfer provenance private candidate outputs drifted",
                    )?;
                }
            }
        }
    }

    if let Some(document) = optional_documents.get(&path("initial-sign-packet.v5.json")) {
        let initial_location = &document.path;
        checks.set_diagnostic_stage(DIAG_STAGE_PACKET_SCHEMAS);
        checks.schema(
            initial_location,
            "ToS/contracts/semantic-ladder-packet.schema.json",
            &document.value,
        )?;
        checks.set_diagnostic_stage(DIAG_STAGE_INITIAL_OBSERVATION);
        for message in semantic_ladder_identity_issues(&document.value)? {
            checks.issue(initial_location, message)?;
        }
        if document.value.get("packet_status").and_then(Value::as_str)
            != Some("observational-analysis")
        {
            checks.issue(
                initial_location,
                "initial sign packet must remain observational without semantic promotion",
            )?;
        }
        let observation_event_ref =
            "tos.event.annotation.zarathustra-semantic-source-observation-v1.2026-08-10";
        let observation_anchor_refs = [
            "tos.anchor.zarathustra-semantic-source-observation-v1.o001",
            "tos.anchor.zarathustra-semantic-source-observation-v1.o002",
            "tos.anchor.zarathustra-semantic-source-observation-v1.o003",
            "tos.anchor.zarathustra-semantic-source-observation-v1.o004",
        ];
        let observation_occurrence_refs = [
            "tos.occurrence.lexical-zarathustra-dta-v1.edad72babd2957782d70834e42cf5cd8baa861ace4ee183a4a0d8c779c087c01",
            "tos.occurrence.lexical-zarathustra-dta-v1.6e1e0006ee44752bf6c33bc00af3cc48ffdcd30771f1ca0da49edb67ffb772b4",
            "tos.occurrence.lexical-zarathustra-dta-v1.3395c4e11df51cfe5ccd7633e2f05077dcdf9f03fc1d94bdb84df79fbb3ff794",
            "tos.occurrence.lexical-zarathustra-dta-v1.06fe0ae445facb6351810717570d11c83131d2f22a710b571bd09c03cb738cc6",
        ];
        let expected_anchor_values: Vec<Value> = observation_anchor_refs
            .iter()
            .map(|value| Value::String((*value).into()))
            .collect();
        let expected_anchor_value = Value::Array(expected_anchor_values.clone());
        let edition_admission = optional_documents
            .get(&path(
                "edition-reading-admission.dta-ekgwb.za-i-vorrede-1.v1.json",
            ))
            .map(|admission| &admission.value);
        let admission_target_anchor = edition_admission
            .and_then(|admission| admission.get("target"))
            .and_then(|target| target.get("context_anchor_ref"))
            .cloned()
            .unwrap_or(Value::Null);
        let mut expected_source_anchor_refs = vec![admission_target_anchor];
        expected_source_anchor_refs.extend(expected_anchor_values.iter().cloned());
        let source_gate = document
            .value
            .get("task_specific_source_gate")
            .unwrap_or(&Value::Null);
        let admission_path = path("edition-reading-admission.dta-ekgwb.za-i-vorrede-1.v1.json");
        let admission_digest = optional_documents
            .get(&admission_path)
            .map(|admission| admission.sha256.as_str());
        let source_event_drifted = checks.python_different(
            source_gate.get("source_review_event_ref"),
            edition_admission.and_then(|admission| admission.get("provenance_event_ref")),
        )?;
        let source_anchor_drifted = checks.python_different(
            source_gate.get("source_anchor_refs"),
            Some(&Value::Array(expected_source_anchor_refs)),
        )?;
        let local_source_digest_drifted = checks.python_different(
            source_gate.get("local_source_sha256"),
            edition_admission
                .and_then(|admission| admission.get("source_identity"))
                .and_then(|identity| identity.get("file_sha256")),
        )?;
        if source_gate.get("gate_status").and_then(Value::as_str) != Some("satisfied")
            || source_gate
                .get("source_reading_status")
                .and_then(Value::as_str)
                != Some("edition-attested")
            || source_gate
                .get("source_observation_allowed")
                .and_then(Value::as_bool)
                != Some(true)
            || source_gate
                .get("edition_reading_admission_ref")
                .and_then(Value::as_str)
                != Some(admission_path.as_str())
            || source_gate
                .get("edition_reading_admission_sha256")
                .and_then(Value::as_str)
                != admission_digest
            || source_event_drifted
            || source_anchor_drifted
            || local_source_digest_drifted
            || source_gate
                .get("local_source_tracked")
                .and_then(Value::as_bool)
                != Some(false)
            || source_gate
                .get("language_competence_status")
                .and_then(Value::as_str)
                != Some("blocked")
            || checks.python_different(
                source_gate.get("language_competence_evidence_refs"),
                Some(&Value::Array(Vec::new())),
            )?
            || source_gate
                .get("linguistic_claim_review_allowed")
                .and_then(Value::as_bool)
                != Some(false)
        {
            checks.issue(
                initial_location,
                "initial sign packet edition-reading and competence separation drifted",
            )?;
        }
        if document
            .value
            .get("candidate_ref")
            .is_some_and(|value| !value.is_null())
            || document
                .value
                .get("accepted_sign_ref")
                .is_some_and(|value| !value.is_null())
            || checks.python_different(
                document.value.get("translation_evidence"),
                Some(&Value::Array(Vec::new())),
            )?
        {
            checks.issue(
                initial_location,
                "initial sign packet manufactured semantic or translation content",
            )?;
        }
        let source_forms = document.value.get("source_forms").unwrap_or(&Value::Null);
        let expected_local_root =
            format!("{root}/local-content/semantic-source-observation/initial-v1/");
        let expected_source_forms = serde_json::json!({
            "diplomatic_local_ref": format!("{expected_local_root}diplomatic-form.txt"),
            "diplomatic_sha256": "7524ff4cef9ab57270250af698a31e2c50f5d6de92e3aaa1377f7ca3659c1ec1",
            "normalized_local_ref": format!("{expected_local_root}normalized-form.txt"),
            "normalized_sha256": "7524ff4cef9ab57270250af698a31e2c50f5d6de92e3aaa1377f7ca3659c1ec1",
            "source_values_tracked": false,
        });
        if checks.python_different(Some(source_forms), Some(&expected_source_forms))? {
            checks.issue(
                initial_location,
                "initial sign packet source-withholding form binding drifted",
            )?;
        }
        let stages = array(&document.value, "stages");
        let active_stages = &stages[..stages.len().min(3)];
        let later_stages = &stages[stages.len().min(3)..];
        let mut stages_drifted = stages.len() != 15;
        for stage in active_stages {
            if stage.get("status").and_then(Value::as_str) != Some("source-observed")
                || checks.python_different(
                    stage.get("source_anchor_refs"),
                    Some(&expected_anchor_value),
                )?
                || stage.get("source_return_verified").and_then(Value::as_bool) != Some(true)
                || stage.get("provenance_event_ref").and_then(Value::as_str)
                    != Some(observation_event_ref)
                || stage.get("review_status").and_then(Value::as_str) != Some("unreviewed")
                || checks
                    .python_different(stage.get("blocker_refs"), Some(&Value::Array(Vec::new())))?
                || stage
                    .get("maker")
                    .and_then(|maker| maker.get("performed_by_real_human"))
                    .and_then(Value::as_bool)
                    != Some(false)
            {
                stages_drifted = true;
            }
        }
        for stage in later_stages {
            if stage.get("status").and_then(Value::as_str) != Some("blocked")
                || checks.python_different(
                    stage.get("source_anchor_refs"),
                    Some(&Value::Array(Vec::new())),
                )?
                || checks.python_different(stage.get("body"), Some(&serde_json::json!({})))?
                || stage.get("maker").is_some_and(|maker| !maker.is_null())
                || stage
                    .get("provenance_event_ref")
                    .is_some_and(|event| !event.is_null())
            {
                stages_drifted = true;
            }
        }
        if stages_drifted {
            checks.issue(
                initial_location,
                "initial sign packet observational and blocked stage boundary drifted",
            )?;
        } else {
            let body = |index: usize| stages[index].get("body").unwrap_or(&Value::Null);
            let expected_occurrences = Value::Array(
                observation_occurrence_refs
                    .iter()
                    .map(|value| Value::String((*value).into()))
                    .collect(),
            );
            if checks
                .python_different(body(0).get("occurrence_refs"), Some(&expected_occurrences))?
                || checks.python_different(
                    body(1).get("occurrence_refs"),
                    body(0).get("occurrence_refs"),
                )?
                || checks.python_different(
                    body(2).get("occurrence_refs"),
                    body(0).get("occurrence_refs"),
                )?
                || body(0).get("exact_form_sha256").and_then(Value::as_str)
                    != Some("0007489cd4b0a84b926a341d3540ae1e8a2ff9cfc2062069dbf5a4e994f6ef37")
                || checks.python_different(body(1).get("count"), Some(&Value::from(4)))?
                || checks.python_different(
                    body(1).get("frequency_only_basis"),
                    Some(&Value::Bool(false)),
                )?
                || body(2).get("context_sha256").and_then(Value::as_str)
                    != Some("fe1cd840554ab13e2fed5fea9f46b6e5f5e730cd1d100b4a57b010894d16c7a1")
            {
                checks.issue(
                    initial_location,
                    "initial exact-form, frequency, or context observation drifted",
                )?;
            }
        }
        let observation_plan_path = path("initial-semantic-source-observation-plan.v1.json");
        let observation_plan = optional_documents
            .get(&observation_plan_path)
            .map(|plan| &plan.value);
        let expected_observation_order = serde_json::json!([
            "section-occurrence-count-descending",
            "first-source-token-ordinal-ascending",
            "exact-form-utf8-bytes-ascending"
        ]);
        let observation_plan_drifted = match observation_plan {
            None => true,
            Some(plan) => {
                plan.get("status").and_then(Value::as_str) != Some("frozen-before-selection")
                    || plan
                        .get("source_scope")
                        .and_then(|scope| scope.get("section_resource_id"))
                        .and_then(Value::as_str)
                        != Some("tei-div-0003")
                    || checks.python_different(
                        plan.get("selection_policy")
                            .and_then(|policy| policy.get("ordering")),
                        Some(&expected_observation_order),
                    )?
                    || plan
                        .get("selection_policy")
                        .and_then(|policy| policy.get("semantic_filter_used"))
                        .and_then(Value::as_bool)
                        != Some(false)
                    || plan
                        .get("selection_policy")
                        .and_then(|policy| policy.get("selection_is_sign_nomination"))
                        .and_then(Value::as_bool)
                        != Some(false)
            }
        };
        if observation_plan_drifted {
            checks.issue(
                &observation_plan_path,
                "initial semantic source-observation selection plan drifted",
            )?;
        }
        let result = document.value.get("result").unwrap_or(&Value::Null);
        let mut empty_result_drifted = false;
        for field in [
            "human_decision_refs",
            "relation_refs",
            "concept_refs",
            "claim_refs",
            "graph_projection_refs",
        ] {
            if checks.python_different(result.get(field), Some(&Value::Array(Vec::new())))? {
                empty_result_drifted = true;
                break;
            }
        }
        if document
            .value
            .get("assurance_policy")
            .and_then(|policy| policy.get("human_work_scheduled"))
            .and_then(Value::as_bool)
            != Some(false)
            || empty_result_drifted
            || result.get("promotion_authorized").and_then(Value::as_bool) != Some(false)
        {
            checks.issue(
                initial_location,
                "initial sign packet created human debt or promotion output",
            )?;
        }
    }

    let recurrence_plan_path = path("semantic-source-recurrence-plan.v1.json");
    let recurrence_receipt_path = path("semantic-source-recurrence-receipt.v1.json");
    if let (Some(plan_doc), Some(receipt_doc)) = (
        optional_documents.get(&recurrence_plan_path),
        optional_documents.get(&recurrence_receipt_path),
    ) {
        checks.set_diagnostic_stage(DIAG_STAGE_INITIAL_OBSERVATION);
        let plan_location = &plan_doc.path;
        let receipt_location = &receipt_doc.path;
        let expected_hash = "0007489cd4b0a84b926a341d3540ae1e8a2ff9cfc2062069dbf5a4e994f6ef37";
        let expected_tuple = serde_json::json!({
            "occurrence_count": 145,
            "part_range": 4,
            "section_range": 59,
            "page_range": 96,
            "part_dp_millionths": 111588,
            "maximum_part_share_millionths": 351724,
            "source_editorial_occurrence_count": 0,
            "unsectioned_occurrence_count": 0,
        });
        let expected_local_ref = format!(
            "{root}/local-content/semantic-source-observation/initial-v1/work-recurrence-bundle.json"
        );
        let selected = plan_doc
            .value
            .get("selected_source_observation")
            .unwrap_or(&Value::Null);
        let plan_recurrence = plan_doc
            .value
            .get("recurrence_input")
            .unwrap_or(&Value::Null);
        let initial_sign_path = path("initial-sign-packet.v5.json");
        let initial_sign_digest = optional_documents
            .get(&initial_sign_path)
            .map(|document| document.sha256.as_str());
        if plan_doc.value.get("status").and_then(Value::as_str) != Some("frozen-before-output")
            || selected.get("exact_form_sha256").and_then(Value::as_str) != Some(expected_hash)
            || selected.get("packet_ref").and_then(Value::as_str)
                != Some(initial_sign_path.as_str())
            || selected.get("packet_sha256").and_then(Value::as_str) != initial_sign_digest
            || selected.get("selection_reopened").and_then(Value::as_bool) != Some(false)
            || checks.python_different(
                plan_doc.value.get("expected_tracked_recurrence_tuple"),
                Some(&expected_tuple),
            )?
            || plan_doc
                .value
                .get("local_output")
                .and_then(|output| output.get("ref"))
                .and_then(Value::as_str)
                != Some(expected_local_ref.as_str())
            || plan_doc
                .value
                .get("verification_policy")
                .and_then(|policy| policy.get("raw_source_return_sampling"))
                .and_then(Value::as_str)
                != Some("none-verify-all-occurrences")
            || plan_doc
                .value
                .get("verification_policy")
                .and_then(|policy| policy.get("tracked_occurrence_positions"))
                .and_then(Value::as_bool)
                != Some(false)
        {
            checks.issue(
                plan_location,
                "selected-form source-recurrence plan drifted",
            )?;
        }
        for (ref_field, digest_field, label) in [
            (
                "recurrence_plan_ref",
                "recurrence_plan_sha256",
                "recurrence plan",
            ),
            (
                "recurrence_projection_ref",
                "recurrence_projection_sha256",
                "recurrence projection",
            ),
            (
                "lexical_projection_ref",
                "lexical_projection_sha256",
                "lexical projection",
            ),
        ] {
            let reference = plan_recurrence.get(ref_field).and_then(Value::as_str);
            let expected_digest = plan_recurrence.get(digest_field).and_then(Value::as_str);
            let current_digest = if let Some(reference) = reference {
                checks.current_digest(reference)?
            } else {
                None
            };
            if reference.is_none()
                || current_digest.is_none()
                || current_digest.as_deref() != expected_digest
            {
                checks.issue(
                    plan_location,
                    format!("selected-form {label} binding drifted"),
                )?;
            }
        }
        if physical
            .private_paths
            .get(&expected_local_ref)
            .and_then(|facts| facts.git_ignored)
            != Some(true)
        {
            checks.issue(
                plan_location,
                "private source-recurrence bundle is not Git-ignored",
            )?;
        }

        let receipt = &receipt_doc.value;
        let generator = receipt.get("generator").unwrap_or(&Value::Null);
        let generator_ref = generator.get("ref").and_then(Value::as_str);
        let generator_sha = generator.get("sha256").and_then(Value::as_str);
        let generator_resolves = match (generator_ref, generator_sha) {
            (Some(reference), Some(digest)) => checks.recorded_digest_matches(reference, digest)?,
            _ => false,
        };
        let receipt_selected = receipt
            .get("selected_source_observation")
            .unwrap_or(&Value::Null);
        let receipt_recurrence = receipt.get("recurrence_sources").unwrap_or(&Value::Null);
        let plan_recurrence_projection = plan_recurrence.get("recurrence_projection_ref");
        let plan_recurrence_projection_digest = plan_recurrence.get("recurrence_projection_sha256");
        let local_bundle = receipt.get("local_bundle").unwrap_or(&Value::Null);
        if receipt.get("status").and_then(Value::as_str)
            != Some("completed-source-observation-no-promotion")
            || checks.python_different(
                receipt.get("plan"),
                Some(&serde_json::json!({
                    "ref": recurrence_plan_path,
                    "sha256": plan_doc.sha256,
                })),
            )?
            || !generator_resolves
            || receipt_selected
                .get("exact_form_sha256")
                .and_then(Value::as_str)
                != Some(expected_hash)
            || receipt_selected.get("packet_ref").and_then(Value::as_str)
                != Some(initial_sign_path.as_str())
            || receipt_selected
                .get("packet_sha256")
                .and_then(Value::as_str)
                != initial_sign_digest
            || receipt_selected
                .get("selection_reopened")
                .and_then(Value::as_bool)
                != Some(false)
            || receipt_selected
                .get("source_value_tracked")
                .and_then(Value::as_bool)
                != Some(false)
            || checks.python_different(
                receipt_recurrence.get("recurrence_projection_ref"),
                plan_recurrence_projection,
            )?
            || checks.python_different(
                receipt_recurrence.get("recurrence_projection_sha256"),
                plan_recurrence_projection_digest,
            )?
            || checks.python_different(receipt.get("observed_tuple"), Some(&expected_tuple))?
            || local_bundle.get("ref").and_then(Value::as_str) != Some(expected_local_ref.as_str())
            || local_bundle.get("mode").and_then(Value::as_str) != Some("0600")
            || checks.python_different(
                local_bundle.get("occurrence_count"),
                Some(&Value::from(145)),
            )?
        {
            checks.issue(
                receipt_location,
                "selected-form source-recurrence receipt drifted",
            )?;
        }
        let expected_parts = [
            (1, 17, 14, 12),
            (2, 32, 20, 12),
            (3, 45, 26, 13),
            (4, 51, 36, 22),
        ];
        checks.charge(
            encoded_len(receipt.get("parts").unwrap_or(&Value::Null))?
                .checked_mul(4)
                .and_then(|bytes| bytes.checked_add(128))
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let observed_parts = Value::Array(
            array(receipt, "parts")
                .iter()
                .filter_map(|part| {
                    part.is_object().then(|| {
                        Value::Array(
                            [
                                "part_order",
                                "occurrence_count",
                                "page_count",
                                "section_count",
                            ]
                            .into_iter()
                            .map(|field| part.get(field).cloned().unwrap_or(Value::Null))
                            .collect(),
                        )
                    })
                })
                .collect(),
        );
        let expected_parts = serde_json::json!([
            [1, 17, 14, 12],
            [2, 32, 20, 12],
            [3, 45, 26, 13],
            [4, 51, 36, 22]
        ]);
        let verification = receipt.get("verification").unwrap_or(&Value::Null);
        let packet_effect = receipt.get("packet_effect").unwrap_or(&Value::Null);
        let boundary = receipt.get("authority_boundary").unwrap_or(&Value::Null);
        if checks.python_different(Some(&observed_parts), Some(&expected_parts))?
            || verification
                .get("complete_occurrence_census")
                .and_then(Value::as_bool)
                != Some(true)
            || checks.python_different(
                verification.get("raw_offset_return_count"),
                Some(&Value::from(145)),
            )?
            || checks.python_different(
                verification.get("raw_offset_return_match_count"),
                Some(&Value::from(145)),
            )?
            || checks.python_different(
                verification.get("source_payload_fixity_match_count"),
                Some(&Value::from(4)),
            )?
            || verification
                .get("tracked_recurrence_tuple_match")
                .and_then(Value::as_bool)
                != Some(true)
            || verification
                .get("independent_part_size_aware_recalculation")
                .and_then(Value::as_bool)
                != Some(true)
            || [
                "packet_changed",
                "ladder_stage_changed",
                "human_work_scheduled",
                "promotion_authorized",
            ]
            .iter()
            .any(|field| packet_effect.get(field).and_then(Value::as_bool) != Some(false))
            || boundary
                .get("source_observation_only")
                .and_then(Value::as_bool)
                != Some(true)
            || boundary
                .get("packet_stage_change_authorized")
                .and_then(Value::as_bool)
                != Some(false)
            || [
                "accepted_german",
                "morphology",
                "lemma",
                "sense",
                "motif",
                "philosophical_importance",
                "translation",
                "sign_candidate",
                "human_task",
                "semantic_claim",
                "graph_effect",
                "canon_effect",
                "transfer",
                "publication",
            ]
            .iter()
            .any(|field| boundary.get(field).and_then(Value::as_bool) != Some(false))
        {
            checks.issue(
                receipt_location,
                "selected-form recurrence verification or authority boundary drifted",
            )?;
        }
        if value_contains_any_text(&receipt_doc.value, &["/srv/", "/home/"])
            || value_contains_exact_atom(
                &receipt_doc.value,
                &[
                    "exact_form",
                    "normalized_form",
                    "occurrence_id",
                    "text_node_path",
                    "token_ordinal",
                    "start_offset",
                    "end_offset",
                ],
            )
        {
            checks.issue(
                receipt_location,
                "tracked source-recurrence receipt leaks source values, positions, or an absolute private path",
            )?;
        }
    }

    let review_plan_path = path("translation-source-review-plan.v2.json");
    let laboratory_plan_path = path("translation-laboratory-plan.v1.json");
    let current_accepted = optional_documents
        .get(&laboratory_plan_path)
        .and_then(|document| document.value.get("source_review_gate"))
        .and_then(|gate| gate.get("current_human_accepted_units"));
    let actual_gold_count = status
        .as_ref()
        .map(|document| {
            array(&document.value, "units")
                .iter()
                .filter(|unit| {
                    unit.get("gold_status").and_then(Value::as_str) == Some("human_double_checked")
                })
                .count() as u64
        })
        .unwrap_or(0);

    checks.set_diagnostic_stage(DIAG_STAGE_INITIAL_OBSERVATION);
    for (document, contract, expected_kind, expected_experiment) in [
        (
            semantic.as_ref(),
            "ToS/contracts/source-gated-semantic-evaluation-plan.schema.json",
            "semantic-annotation",
            "tos-semantic-annotation-v1",
        ),
        (
            llm.as_ref(),
            "ToS/contracts/source-gated-llm-evaluation-plan.schema.json",
            "llm-assistance",
            "tos-llm-assistance-v1",
        ),
    ] {
        if let Some(document) = document {
            checks.charge(
                encoded_len(&document.value)?
                    .checked_mul(2)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            checks.schema(&document.path, contract, &document.value)?;
            checks.set_diagnostic_stage(evaluations_diagnostic_stage);
            if document.value.get("plan_kind").and_then(Value::as_str) != Some(expected_kind) {
                checks.issue(&document.path, "source-gated plan kind drifted")?;
            }
            if document.value.get("experiment_id").and_then(Value::as_str)
                != Some(expected_experiment)
            {
                checks.issue(&document.path, "source-gated experiment_id drifted")?;
            }

            let review_plan_digest = optional_documents
                .get(&review_plan_path)
                .map(|source| source.sha256.as_str());
            let laboratory_plan_digest = optional_documents
                .get(&laboratory_plan_path)
                .map(|source| source.sha256.as_str());
            let expected_review_plan_ref = Value::String(review_plan_path.clone());
            let expected_laboratory_plan_ref = Value::String(laboratory_plan_path.clone());
            let expected_review_plan_digest = review_plan_digest
                .map(|digest| Value::String(digest.to_owned()))
                .unwrap_or(Value::Null);
            let expected_laboratory_plan_digest = laboratory_plan_digest
                .map(|digest| Value::String(digest.to_owned()))
                .unwrap_or(Value::Null);
            let expected_gold_status_ref = Value::String(path(GOLD_STATUS));
            let expected_gold_status_digest = status
                .as_ref()
                .map(|source| Value::String(source.sha256.clone()))
                .unwrap_or(Value::Null);
            let expected_gold_count = Value::from(actual_gold_count);
            let expected_false = Value::Bool(false);
            let evaluation_is_semantic = expected_kind == "semantic-annotation";

            if evaluation_is_semantic
                && document.value.get("schema_version").and_then(Value::as_str)
                    == Some("tos_source_gated_evaluation_plan_v1")
            {
                let source_gate = document.value.get("source_gate").unwrap_or(&Value::Null);
                for (field, expected) in [
                    ("review_plan_ref", &expected_review_plan_ref),
                    ("review_plan_sha256", &expected_review_plan_digest),
                    ("laboratory_plan_ref", &expected_laboratory_plan_ref),
                    ("laboratory_plan_sha256", &expected_laboratory_plan_digest),
                ] {
                    if !checks.python_optional_equal(source_gate.get(field), Some(expected))? {
                        checks.issue(&document.path, format!("source gate {field} drifted"))?;
                    }
                }
                if !checks.python_optional_equal(
                    source_gate.get("current_human_accepted_units"),
                    current_accepted,
                )? {
                    checks.issue(&document.path, "source-gate accepted count drifted")?;
                }

                let human_gold_gate = document
                    .value
                    .get("human_gold_gate")
                    .unwrap_or(&Value::Null);
                if !checks.python_optional_equal(
                    human_gold_gate.get("gold_status_ref"),
                    Some(&expected_gold_status_ref),
                )? {
                    checks.issue(&document.path, "human-gold status ref drifted")?;
                }
                if !checks.python_optional_equal(
                    human_gold_gate.get("gold_status_sha256"),
                    Some(&expected_gold_status_digest),
                )? {
                    checks.issue(&document.path, "human-gold status digest drifted")?;
                }
                if !checks.python_optional_equal(
                    human_gold_gate.get("current_human_double_checked_units"),
                    Some(&expected_gold_count),
                )? {
                    checks.issue(&document.path, "human-gold count drifted")?;
                }
            } else if evaluation_is_semantic {
                let tasks = document
                    .value
                    .get("tasks")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let source_ready_count = tasks
                    .iter()
                    .filter(|task| {
                        task.is_object()
                            && task.get("source_anchor_refs").is_some_and(python_truthy)
                            && task
                                .get("accepted_source_sha256")
                                .is_some_and(Value::is_string)
                            && task
                                .get("source_review_event_ref")
                                .is_some_and(Value::is_string)
                            && task.get("local_content_ref").is_some_and(Value::is_string)
                            && task
                                .get("local_content_sha256")
                                .is_some_and(Value::is_string)
                    })
                    .count() as u64;
                let expected_source_ready_count = Value::from(source_ready_count);
                let task_specific_source_gate = document
                    .value
                    .get("task_specific_source_gate")
                    .unwrap_or(&Value::Null);
                if !checks.python_optional_equal(
                    task_specific_source_gate.get("current_eligible_tasks"),
                    Some(&expected_source_ready_count),
                )? {
                    checks.issue(
                        &document.path,
                        "task-specific semantic source-evidence count drifted",
                    )?;
                }

                let selection_contract = document
                    .value
                    .get("selection_contract")
                    .unwrap_or(&Value::Null);
                if !tasks.is_empty() {
                    let mut family_counts = serde_json::Map::new();
                    let mut stratum_counts = serde_json::Map::new();
                    let mut task_anchor_refs = BTreeSet::new();
                    for task in tasks {
                        if !task.is_object() {
                            continue;
                        }
                        for (field, counts) in [
                            ("task_family", &mut family_counts),
                            ("stratum", &mut stratum_counts),
                        ] {
                            if let Some(key) = task.get(field).and_then(Value::as_str) {
                                let count = counts
                                    .get(key)
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0)
                                    .checked_add(1)
                                    .ok_or(ItemRefusal::Budget)?;
                                counts.insert(key.to_owned(), Value::from(count));
                            }
                        }
                        if let Some(anchors) = task.get("source_anchor_refs") {
                            task_anchor_refs.extend(python_iterable_string_values(anchors));
                        }
                    }
                    let family_counts = Value::Object(family_counts);
                    if !checks.python_optional_equal(
                        Some(&family_counts),
                        selection_contract.get("required_tasks_per_family"),
                    )? {
                        checks.issue(
                            &document.path,
                            "semantic task-family counts drifted from frozen selection",
                        )?;
                    }

                    let mut strata_drifted = stratum_counts.len() != 2
                        || !stratum_counts.contains_key("random")
                        || !stratum_counts.contains_key("hard");
                    for field in ["random", "hard"] {
                        let actual = stratum_counts.get(field);
                        let expected = selection_contract.get(&format!("{field}_tasks"));
                        if !checks.python_optional_equal(actual, expected)? {
                            strata_drifted = true;
                        }
                    }
                    if strata_drifted {
                        checks.issue(
                            &document.path,
                            "semantic task strata drifted from frozen selection",
                        )?;
                    }

                    let prepared_anchor_refs = document
                        .value
                        .get("prepared_task_contract")
                        .and_then(|contract| contract.get("source_anchor_refs"))
                        .map(python_iterable_string_values)
                        .unwrap_or_default();
                    if prepared_anchor_refs != task_anchor_refs {
                        checks.issue(
                            &document.path,
                            "semantic prepared anchor set differs from task anchors",
                        )?;
                    }
                }

                let historical_gate = document
                    .value
                    .get("historical_gate_snapshot")
                    .unwrap_or(&Value::Null);
                for (field, expected) in [
                    ("source_review_plan_ref", &expected_review_plan_ref),
                    ("source_review_plan_sha256", &expected_review_plan_digest),
                    ("laboratory_plan_ref", &expected_laboratory_plan_ref),
                    ("laboratory_plan_sha256", &expected_laboratory_plan_digest),
                    ("gold_status_ref", &expected_gold_status_ref),
                    ("gold_status_sha256", &expected_gold_status_digest),
                    (
                        "observed_human_accepted_source_units",
                        current_accepted.unwrap_or(&Value::Null),
                    ),
                    (
                        "observed_human_double_checked_gold_units",
                        &expected_gold_count,
                    ),
                    ("scheduling_authority", &expected_false),
                    ("execution_authority", &expected_false),
                ] {
                    if !checks.python_optional_equal(historical_gate.get(field), Some(expected))? {
                        checks.issue(
                            &document.path,
                            format!("historical gate snapshot {field} drifted"),
                        )?;
                    }
                }
            } else {
                let tasks = document
                    .value
                    .get("tasks")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let source_ready_count = tasks
                    .iter()
                    .filter(|task| {
                        task.is_object()
                            && task.get("source_anchor_refs").is_some_and(python_truthy)
                            && task
                                .get("accepted_source_sha256")
                                .is_some_and(Value::is_string)
                            && task
                                .get("source_review_event_ref")
                                .is_some_and(Value::is_string)
                            && task.get("local_content_ref").is_some_and(Value::is_string)
                            && task
                                .get("local_content_sha256")
                                .is_some_and(Value::is_string)
                    })
                    .count() as u64;
                let baseline_ready_count = tasks
                    .iter()
                    .filter(|task| {
                        task.is_object()
                            && task
                                .get("human_baseline_ref")
                                .and_then(Value::as_str)
                                .is_some_and(|reference| !reference.is_empty())
                            && task
                                .get("human_baseline_sha256")
                                .is_some_and(Value::is_string)
                    })
                    .count() as u64;
                let expected_source_ready_count = Value::from(source_ready_count);
                let expected_baseline_ready_count = Value::from(baseline_ready_count);
                let source_evidence_gate = document
                    .value
                    .get("source_evidence_gate")
                    .unwrap_or(&Value::Null);
                if !checks.python_optional_equal(
                    source_evidence_gate.get("current_eligible_source_units"),
                    Some(&expected_source_ready_count),
                )? {
                    checks.issue(
                        &document.path,
                        "task-specific source-evidence count drifted",
                    )?;
                }
                let human_baseline_gate = document
                    .value
                    .get("human_baseline_gate")
                    .unwrap_or(&Value::Null);
                if !checks.python_optional_equal(
                    human_baseline_gate.get("current_frozen_baseline_units"),
                    Some(&expected_baseline_ready_count),
                )? {
                    checks.issue(&document.path, "task-specific human-baseline count drifted")?;
                }

                let historical_gate = document
                    .value
                    .get("historical_gate_snapshot")
                    .unwrap_or(&Value::Null);
                for (field, expected) in [
                    ("source_review_plan_ref", &expected_review_plan_ref),
                    ("source_review_plan_sha256", &expected_review_plan_digest),
                    ("laboratory_plan_ref", &expected_laboratory_plan_ref),
                    ("laboratory_plan_sha256", &expected_laboratory_plan_digest),
                    ("gold_status_ref", &expected_gold_status_ref),
                    ("gold_status_sha256", &expected_gold_status_digest),
                    (
                        "observed_human_accepted_source_units",
                        current_accepted.unwrap_or(&Value::Null),
                    ),
                    (
                        "observed_human_double_checked_gold_units",
                        &expected_gold_count,
                    ),
                    ("scheduling_authority", &expected_false),
                    ("execution_authority", &expected_false),
                ] {
                    if !checks.python_optional_equal(historical_gate.get(field), Some(expected))? {
                        checks.issue(
                            &document.path,
                            format!("historical gate snapshot {field} drifted"),
                        )?;
                    }
                }
            }

            let evaluation_event = document
                .value
                .get("provenance_event_ref")
                .and_then(Value::as_str)
                .and_then(|event_id| events_by_id.get(event_id));
            if evaluation_event.is_none() {
                checks.issue(&document.path, "evaluation provenance_event_ref is absent")?;
            } else if let Some(event) = evaluation_event {
                let output = array(event, "outputs").iter().find(|output| {
                    output.get("ref").and_then(Value::as_str) == Some(document.path.as_str())
                });
                if output.is_none() {
                    checks.issue(&document.path, "evaluation provenance omits plan")?;
                } else if output.and_then(|v| v.get("sha256")).and_then(Value::as_str)
                    != Some(document.sha256.as_str())
                {
                    checks.issue(&document.path, "evaluation provenance digest drifted")?;
                }
            }
        }
    }

    let source_review_path = path("translation-source-review-plan.v2.json");
    if let Some(review) = optional_documents.get(&source_review_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_TRANSLATION_REVIEW);
        checks.schema(
            &review.path,
            "ToS/contracts/translation-source-review-plan.schema.json",
            &review.value,
        )?;
        let supersedes = review.value.get("supersedes").unwrap_or(&Value::Null);
        let v1_path = path(TRANSLATION_PLAN);
        if supersedes.get("v1_plan_ref").and_then(Value::as_str) != Some(v1_path.as_str()) {
            checks.issue(
                &review.path,
                "v2 source review plan does not cite its v1 plan",
            )?;
        }
        if translation.is_some()
            && supersedes.get("v1_plan_sha256").and_then(Value::as_str)
                != translation
                    .as_ref()
                    .map(|document| document.sha256.as_str())
        {
            checks.issue(&review.path, "v1 translation plan digest drifted")?;
        }
        let inspection_ref = supersedes.get("v1_inspection_ref").and_then(Value::as_str);
        let mut inspection = None;
        if let Some(reference) =
            inspection_ref.filter(|reference| RelativePath::parse(reference).is_ok())
        {
            if let Some(document) = checks.json(reference, false)? {
                if document.sha256.as_str()
                    != supersedes
                        .get("v1_inspection_sha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                {
                    checks.issue(&review.path, "v1 selector inspection digest drifted")?;
                }
                inspection = Some(document);
            } else {
                checks.issue(
                    &review.path,
                    "v1 selector inspection reference is unresolved",
                )?;
            }
        } else {
            checks.issue(
                &review.path,
                "v1 selector inspection reference is unresolved",
            )?;
        }
        let units = array(&review.value, "units");
        let fragments = translation
            .as_ref()
            .map(|document| array(&document.value, "fragments"))
            .unwrap_or(&[]);
        let inspection_records = inspection
            .as_ref()
            .map(|document| array(&document.value, "records"))
            .unwrap_or(&[]);
        if units.len() != fragments.len() || units.len() != inspection_records.len() {
            checks.issue(
                &review.path,
                "v2 units do not close over all v1 fragments and inspections",
            )?;
        }
        let strategy_keys = [
            (
                "confirm-visible-complete-prose-unit-without-reusing-v1-text",
                "confirm_visible_prose",
            ),
            (
                "identify-first-new-complete-prose-unit-using-page-triplet",
                "first_new_unit_after_tail",
            ),
            (
                "identify-first-prose-unit-after-visible-heading",
                "first_prose_after_heading",
            ),
            (
                "resolve-cross-page-boundary-before-unit-selection",
                "resolve_cross_page_boundary",
            ),
        ];
        let mut strategy_counts: BTreeMap<String, u64> = [
            "confirm_visible_prose",
            "first_new_unit_after_tail",
            "first_prose_after_heading",
            "resolve_cross_page_boundary",
        ]
        .into_iter()
        .map(|key| (key.into(), 0))
        .collect();
        let mut seen_review_ids = BTreeSet::new();
        for (index, (unit, fragment, inspection_record)) in units
            .iter()
            .zip(fragments)
            .zip(inspection_records)
            .map(|((a, b), c)| (a, b, c))
            .enumerate()
        {
            if !unit.is_object() || !fragment.is_object() || !inspection_record.is_object() {
                continue;
            }
            let unit_id = unit.get("review_unit_id");
            if unit_id.is_some_and(|value| !seen_review_ids.insert(display(value))) {
                checks.issue(
                    &review.path,
                    format!(
                        "duplicate review_unit_id: {}",
                        unit_id.map(display).unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            let expected_unit_id = format!("tos-translation-source-review-v2-{:03}", index + 1);
            if unit_id.and_then(Value::as_str) != Some(expected_unit_id.as_str()) {
                checks.issue(
                    &review.path,
                    format!("unit {} must be {expected_unit_id}", index + 1),
                )?;
            }
            for (key, expected) in [
                ("supersedes_fragment_id", fragment.get("fragment_id")),
                ("context_anchor_ref", fragment.get("source_anchor_ref")),
                ("container_member", fragment.get("container_member")),
                ("member_sha256", fragment.get("member_sha256")),
                ("v1_inspection_decision", inspection_record.get("decision")),
            ] {
                if checks.python_different(unit.get(key), expected)? {
                    checks.issue(
                        &review.path,
                        format!(
                            "{}.{key} drifted from v1 evidence",
                            unit_id.map(display).unwrap_or_else(|| "None".into())
                        ),
                    )?;
                }
            }
            let current_page = fragment
                .get("page_member_index")
                .and_then(Value::as_i64)
                .and_then(|index| index.checked_add(1));
            let expected_context = current_page.map(|page| {
                serde_json::json!({
                    "previous_pdf_page": page - 1,
                    "current_pdf_page": page,
                    "next_pdf_page": page + 1,
                })
            });
            if checks.python_different(expected_context.as_ref(), unit.get("visual_context"))? {
                checks.issue(
                    &review.path,
                    format!(
                        "{}.visual_context is not the exact page triplet",
                        unit_id.map(display).unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            let relevant_signal_set = [
                "heading-selected",
                "page-start-tail",
                "ocr-contamination",
                "page-start-boundary-unverified",
                "transcription-uncertain",
            ];
            let mut relevant_signals: Vec<Value> = array(inspection_record, "signals")
                .iter()
                .filter(|signal| {
                    signal
                        .as_str()
                        .is_some_and(|signal| relevant_signal_set.contains(&signal))
                })
                .cloned()
                .collect();
            if inspection_record.get("decision").and_then(Value::as_str)
                == Some("accept-with-limits")
            {
                relevant_signals.insert(0, Value::String("candidate-usable-with-limits".into()));
            }
            if checks.python_different(
                unit.get("v1_failure_signals"),
                Some(&Value::Array(relevant_signals)),
            )? {
                checks.issue(
                    &review.path,
                    format!(
                        "{}.v1_failure_signals drifted",
                        unit_id.map(display).unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            let strategy = unit
                .get("selection_instruction")
                .and_then(Value::as_str)
                .and_then(|instruction| {
                    strategy_keys
                        .iter()
                        .find(|(expected, _)| *expected == instruction)
                        .map(|(_, key)| *key)
                });
            if let Some(strategy) = strategy {
                if let Some(count) = strategy_counts.get_mut(strategy) {
                    *count += 1;
                }
            }
        }
        let expected_strategy_counts: BTreeMap<String, Value> = strategy_counts
            .iter()
            .map(|(key, value)| (key.clone(), Value::from(*value)))
            .collect();
        let expected_strategy_counts =
            Value::Object(expected_strategy_counts.into_iter().collect());
        if checks.python_different(
            review.value.get("selection_strategy_counts"),
            Some(&expected_strategy_counts),
        )? {
            checks.issue(
                &review.path,
                "selection_strategy_counts do not match v2 units",
            )?;
        }
        if let Some(translation) = &translation {
            let expected_source_witness = serde_json::json!({
                "item_ref": translation.value.get("source_item_ref"),
                "file_ref": translation.value.get("source_file_ref"),
                "file_sha256": translation.value.get("source_file_sha256"),
                "role": "automatic-ocr-derivative-not-source-truth",
            });
            if checks.python_different(
                review.value.get("source_witness"),
                Some(&expected_source_witness),
            )? {
                checks.issue(
                    &review.path,
                    "v2 source witness drifted from v1 source identity",
                )?;
            }
            for key in ["expression_ref", "item_ref", "visibility", "reveal_stage"] {
                if checks.python_different(
                    review
                        .value
                        .get("recognized_comparator")
                        .and_then(|v| v.get(key)),
                    translation
                        .value
                        .get("recognized_comparator")
                        .and_then(|v| v.get(key)),
                )? {
                    checks.issue(&review.path, format!("recognized comparator {key} drifted"))?;
                }
            }
        }
    }

    let assisted_path = path("german-assisted-source-review.v1.json");
    if let Some(assisted) = optional_documents.get(&assisted_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_SOURCE_TRIANGULATION);
        checks.schema(
            &assisted.path,
            "ToS/contracts/german-assisted-source-review.schema.json",
            &assisted.value,
        )?;
        let assisted_research_path =
            "ToS/research-packets/foundation-laboratory-2026-07/GERMAN_ASSISTED_REVIEW_RESEARCH.md";
        check_external_digest_binding(
            &mut checks,
            &assisted.value,
            "method_research",
            assisted_research_path,
            &assisted.path,
        )?;
        check_external_digest_binding(
            &mut checks,
            &assisted.value,
            "source_review_plan",
            &source_review_path,
            &assisted.path,
        )?;
        let review_units = optional_documents
            .get(&source_review_path)
            .map(|document| array(&document.value, "units"))
            .unwrap_or(&[]);
        let current_state = assisted.value.get("current_state").unwrap_or(&Value::Null);
        if checks.python_different(
            current_state.get("prepared_units"),
            Some(&Value::from(review_units.len() as u64)),
        )? {
            checks.issue(
                &assisted.path,
                "German assisted-review prepared_units differ from source-review units",
            )?;
        }
        let witness_refs = array(&assisted.value, "critical_edition_witness_packets");
        for (index, binding) in witness_refs.iter().enumerate() {
            let reference = binding.get("ref").and_then(Value::as_str);
            let Some(reference) = reference
                .filter(|reference| critical_witness_paths.iter().any(|path| path == reference))
            else {
                checks.issue(
                    &assisted.path,
                    "German assisted-review names an unresolved critical-edition packet",
                )?;
                continue;
            };
            let actual_digest = checks.current_digest(reference)?;
            if actual_digest.is_none()
                || actual_digest.as_deref() != binding.get("sha256").and_then(Value::as_str)
            {
                checks.issue(
                    &assisted.path,
                    format!("critical_edition_witness_packets[{index}] digest drifted"),
                )?;
            }
        }
        if checks.python_different(
            current_state.get("prepared_critical_edition_witness_packets"),
            Some(&Value::from(witness_refs.len() as u64)),
        )? {
            checks.issue(
                &assisted.path,
                "German assisted-review prepared critical-edition packet count drifted",
            )?;
        }
    }

    let triangulation_path =
        path("german-source-triangulation.ekgwb-dta-naumann.za-i-vorrede-1.v1.json");
    if let Some(triangulation) = optional_documents.get(&triangulation_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_SOURCE_TRIANGULATION);
        checks.schema(
            &triangulation.path,
            "ToS/contracts/german-source-triangulation.schema.json",
            &triangulation.value,
        )?;
        let unique_witness_path =
            (critical_witness_paths.len() == 1).then(|| critical_witness_paths[0].as_str());
        for (field, expected_path) in [
            ("assisted_review_plan", Some(assisted_path.as_str())),
            ("source_review_plan", Some(source_review_path.as_str())),
            ("metadata_critical_witness_packet", unique_witness_path),
        ] {
            let Some(expected_path) = expected_path else {
                checks.issue(
                    &triangulation.path,
                    format!("{field} has no unique owner artifact"),
                )?;
                continue;
            };
            check_external_digest_binding(
                &mut checks,
                triangulation.value.get("bindings").unwrap_or(&Value::Null),
                field,
                expected_path,
                &triangulation.path,
            )?;
        }
        let target = triangulation.value.get("target").unwrap_or(&Value::Null);
        let review_units = optional_documents
            .get(&source_review_path)
            .map(|document| array(&document.value, "units"))
            .unwrap_or(&[]);
        let target_unit = checks.find_python_value(
            review_units,
            "review_unit_id",
            target.get("review_unit_id"),
        )?;
        if let Some(target_unit) = target_unit {
            if checks.python_different(
                target_unit.get("context_anchor_ref"),
                target.get("context_anchor_ref"),
            )? {
                checks.issue(&triangulation.path, "triangulation target anchor drifted")?;
            }
        } else {
            checks.issue(
                &triangulation.path,
                "triangulation target is absent from source-review plan",
            )?;
        }
        if critical_witness_paths.len() == 1 {
            if let Some(witness) = optional_documents.get(&critical_witness_paths[0]) {
                let critical_target = witness.value.get("target").unwrap_or(&Value::Null);
                for field in [
                    "review_unit_id",
                    "context_anchor_ref",
                    "critical_locator_siglum",
                ] {
                    if checks.python_different(target.get(field), critical_target.get(field))? {
                        checks.issue(
                            &triangulation.path,
                            format!("triangulation target {field} drifted from metadata witness"),
                        )?;
                    }
                }
            }
        }
        let expected_ekgwb_local_ref = format!(
            "{root}/local-content/translation/source-review/critical-edition-candidates/ekgwb/za-i/static-html-include.response.html"
        );
        if triangulation
            .value
            .get("inputs")
            .and_then(|inputs| inputs.get("ekgwb"))
            .and_then(|witness| witness.get("local_payload_ref"))
            .and_then(Value::as_str)
            != Some(expected_ekgwb_local_ref.as_str())
        {
            checks.issue(
                &triangulation.path,
                "triangulation eKGWB local-only payload route drifted",
            )?;
        }
        for witness_name in ["dta_tei", "naumann_auto_epub"] {
            let witness = triangulation
                .value
                .get("inputs")
                .and_then(|inputs| inputs.get(witness_name))
                .unwrap_or(&Value::Null);
            if !witness.is_object() {
                continue;
            }
            for (binding_key, invalid_message, unresolved_message) in [
                (
                    "item_manifest",
                    "item manifest reference is invalid",
                    "item manifest is unresolved",
                ),
                (
                    "rights_record",
                    "rights reference is invalid",
                    "rights record is unresolved",
                ),
            ] {
                let binding = witness.get(binding_key).unwrap_or(&Value::Null);
                let Some(reference) = binding.get("ref").and_then(Value::as_str) else {
                    checks.issue(
                        &triangulation.path,
                        format!("{witness_name} {invalid_message}"),
                    )?;
                    continue;
                };
                let digest = checks.current_digest(reference)?;
                if digest.is_none() {
                    checks.issue(
                        &triangulation.path,
                        format!("{witness_name} {unresolved_message}"),
                    )?;
                } else if digest.as_deref() != binding.get("sha256").and_then(Value::as_str) {
                    checks.issue(
                        &triangulation.path,
                        format!("inputs.{witness_name}.{binding_key} digest drifted"),
                    )?;
                }
                if binding_key == "item_manifest" && RelativePath::parse(reference).is_ok() {
                    if let Some(manifest) = checks.json(reference, false)? {
                        let item_ref = witness.get("item_ref");
                        let file_ref = witness.get("file_ref");
                        let file_sha = witness.get("file_sha256");
                        let payload_relative = witness
                            .get("payload_relative_path")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let item_root = reference.strip_suffix("/item.manifest.json");
                        let relative_under_item = item_root.and_then(|item_root| {
                            let source_root = "ToS/source-witnesses/";
                            let root_rel = item_root.strip_prefix(source_root)?;
                            payload_relative.strip_prefix(&format!("{source_root}{root_rel}/"))
                        });
                        let expected_relative =
                            relative_under_item.map(|path| Value::String(path.to_owned()));
                        let mut matching = 0usize;
                        for row in array(&manifest.value, "payload_files") {
                            if checks.python_optional_equal(row.get("file_id"), file_ref)?
                                && checks.python_optional_equal(row.get("sha256"), file_sha)?
                                && checks.python_optional_equal(
                                    row.get("relative_path"),
                                    expected_relative.as_ref(),
                                )?
                            {
                                matching += 1;
                            }
                        }
                        if checks.python_different(manifest.value.get("item_id"), item_ref)?
                            || matching != 1
                        {
                            checks.issue(
                                &triangulation.path,
                                format!("{witness_name} does not close over its item manifest"),
                            )?;
                        }
                    }
                }
            }
        }
        if let Some(assisted) = optional_documents.get(&assisted_path) {
            let state = assisted.value.get("current_state").unwrap_or(&Value::Null);
            let expected_unit = target.get("review_unit_id");
            if assisted.value.get("status").and_then(Value::as_str) != Some("in_progress") {
                checks.issue(
                    &triangulation.path,
                    "machine triangulation requires in-progress assisted review",
                )?;
            }
            let expected_selected_unit_ids =
                Value::Array(vec![expected_unit.cloned().unwrap_or(Value::Null)]);
            if checks.python_different(
                state.get("selected_unit_ids"),
                Some(&expected_selected_unit_ids),
            )? {
                checks.issue(
                    &triangulation.path,
                    "assisted-review selected unit drifted from triangulation",
                )?;
            }
            if checks.python_different(state.get("runs"), Some(&Value::from(1)))?
                || checks.python_different(
                    state.get("machine_triangulated_units"),
                    Some(&Value::from(1)),
                )?
            {
                checks.issue(
                    &triangulation.path,
                    "assisted-review machine-run counters drifted",
                )?;
            }
            if checks.python_different(state.get("accepted_german_units"), Some(&Value::from(0)))?
                || state.get("promotion_authorized").and_then(Value::as_bool) != Some(false)
            {
                checks.issue(
                    &triangulation.path,
                    "machine triangulation crossed a German-acceptance or promotion gate",
                )?;
            }
        }
    }

    let bounded_input_path =
        path("bounded-translation-research-input.za-i-vorrede-1-opening-sentence.v1.json");
    if let Some(bounded) = optional_documents.get(&bounded_input_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_BOUNDED_INPUT);
        checks.schema(
            &bounded.path,
            "ToS/contracts/bounded-translation-research-input.schema.json",
            &bounded.value,
        )?;
        let lab_path = path("translation-laboratory-plan.v1.json");
        let evidence = bounded.value.get("bindings").unwrap_or(&Value::Null);
        for (field, expected_path) in [
            (
                "corpus_decision",
                "docs/decisions/TOS-D-0020-corpus-evidence-spine-and-witness-storage.md",
            ),
            (
                "admission_research",
                "ToS/research-packets/foundation-laboratory-2026-07/BOUNDED_TRANSLATION_SOURCE_ADMISSION_RESEARCH.md",
            ),
            ("source_review_plan", source_review_path.as_str()),
            ("accepted_translation_plan", lab_path.as_str()),
            ("german_source_triangulation", triangulation_path.as_str()),
            (
                "preexisting_authored_canon_node",
                "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json",
            ),
            (
                "preexisting_public_compatibility_mirror",
                "ToS/public-compatibility/source_node.example.json",
            ),
            (
                "dta_item_manifest",
                "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/item.manifest.json",
            ),
            (
                "dta_rights_record",
                "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/rights.json",
            ),
        ] {
            check_external_digest_binding(
                &mut checks,
                evidence,
                field,
                expected_path,
                &bounded.path,
            )?;
        }
        let target = bounded.value.get("target").unwrap_or(&Value::Null);
        let review_units = optional_documents
            .get(&source_review_path)
            .map(|document| array(&document.value, "units"))
            .unwrap_or(&[]);
        let target_unit = checks.find_python_value(
            review_units,
            "review_unit_id",
            target.get("review_unit_id"),
        )?;
        if let Some(unit) = target_unit {
            if checks.python_different(
                unit.get("context_anchor_ref"),
                target.get("context_anchor_ref"),
            )? {
                checks.issue(&bounded.path, "bounded input target anchor drifted")?;
            }
        } else {
            checks.issue(
                &bounded.path,
                "bounded input target is absent from source-review plan",
            )?;
        }
        let expected_local = format!(
            "{root}/local-content/translation/research-inputs/za-i-vorrede-1-opening-sentence.v1.json"
        );
        let actual_local = bounded
            .value
            .get("local_artifact")
            .and_then(|artifact| artifact.get("ref"))
            .and_then(Value::as_str);
        if actual_local != Some(expected_local.as_str()) {
            checks.issue(
                &bounded.path,
                "bounded input local-only artifact route drifted",
            )?;
        } else if physical
            .private_paths
            .get(&expected_local)
            .and_then(|facts| facts.git_ignored)
            != Some(true)
        {
            checks.issue(
                &bounded.path,
                "bounded input source artifact is not protected by Git ignore",
            )?;
        }
        if let Some(lab) = optional_documents.get(&lab_path) {
            let gate = lab.value.get("source_review_gate").unwrap_or(&Value::Null);
            let blind_lanes = lab.value.get("blind_lanes").unwrap_or(&Value::Null);
            if checks.python_different(
                gate.get("current_human_accepted_units"),
                Some(&Value::from(0)),
            )? || gate.get("gate_state").and_then(Value::as_str)
                != Some("blocked-awaiting-real-human-source-review")
                || blind_lanes.as_object().is_some_and(|lanes| {
                    lanes.values().any(|lane| {
                        lane.is_object()
                            && lane.get("state").and_then(Value::as_str)
                                != Some("blocked-on-source-acceptance")
                    })
                })
            {
                checks.issue(
                    &bounded.path,
                    "bounded calibration input altered the accepted translation plan",
                )?;
            }
        }
        let effects = bounded.value.get("gate_effects").unwrap_or(&Value::Null);
        if checks.python_different(effects.get("human_debt_units"), Some(&Value::from(0)))?
            || checks
                .python_different(effects.get("accepted_german_units"), Some(&Value::from(0)))?
            || !array(effects, "accepted_translation_lanes_opened").is_empty()
            || checks
                .python_different(effects.get("semantic_tasks_opened"), Some(&Value::from(0)))?
            || effects
                .get("canon_or_graph_promotion_authorized")
                .and_then(Value::as_bool)
                != Some(false)
        {
            checks.issue(
                &bounded.path,
                "bounded calibration input crossed an authority gate",
            )?;
        }
    }

    let citation_decision_path =
        path("critical-edition-citation-witness-decision.ekgwb.za-i-vorrede-1.v1.json");
    let reference_register_path = path("translation-reference-register.v1.json");
    let ocr_plan_path = path("ocr-visual-samples.json");
    for witness_path in &critical_witness_paths {
        let Some(witness) = optional_documents.get(witness_path) else {
            continue;
        };
        checks.set_diagnostic_stage(DIAG_STAGE_SPECIALIZED_PACKETS);
        checks.schema(
            &witness.path,
            "ToS/contracts/critical-edition-witness-admission.schema.json",
            &witness.value,
        )?;
        check_external_digest_binding(
            &mut checks,
            &witness.value,
            "source_review_plan",
            &source_review_path,
            &witness.path,
        )?;
        if witness
            .value
            .get("assisted_review_plan_ref")
            .and_then(Value::as_str)
            != Some(assisted_path.as_str())
        {
            checks.issue(
                &witness.path,
                "critical-edition witness does not cite its assisted-review plan",
            )?;
        }
        let register_binding = witness
            .value
            .get("reference_register")
            .unwrap_or(&Value::Null);
        if register_binding.get("ref").and_then(Value::as_str)
            != Some(reference_register_path.as_str())
        {
            checks.issue(
                &witness.path,
                "critical-edition witness does not cite its translation reference register",
            )?;
        }
        let registered_entries = optional_documents
            .get(&reference_register_path)
            .map(|document| array(&document.value, "entries"))
            .unwrap_or(&[]);
        let registered_entry = checks.find_python_value(
            registered_entries,
            "reference_id",
            register_binding.get("reference_id"),
        )?;
        if let Some(entry) = registered_entry {
            if !array(entry.get("tos_refs").unwrap_or(&Value::Null), "path_refs")
                .iter()
                .any(|reference| reference.as_str() == Some(witness_path.as_str()))
            {
                checks.issue(
                    &witness.path,
                    "translation reference entry does not return to the critical-edition witness packet",
                )?;
            }
        } else {
            checks.issue(
                &witness.path,
                "critical-edition witness reference_id is absent from the register",
            )?;
        }
        let target = witness.value.get("target").unwrap_or(&Value::Null);
        let review_units = optional_documents
            .get(&source_review_path)
            .map(|document| array(&document.value, "units"))
            .unwrap_or(&[]);
        let target_unit = checks.find_python_value(
            review_units,
            "review_unit_id",
            target.get("review_unit_id"),
        )?;
        if let Some(unit) = target_unit {
            if checks.python_different(
                unit.get("context_anchor_ref"),
                target.get("context_anchor_ref"),
            )? {
                checks.issue(
                    &witness.path,
                    "critical-edition witness context anchor drifted from its source-review unit",
                )?;
            }
        } else {
            checks.issue(
                &witness.path,
                "critical-edition witness target unit is absent from the source-review plan",
            )?;
        }
        let source_review = optional_documents
            .get(&source_review_path)
            .map(|document| &document.value);
        let ocr_sample_plan = optional_documents
            .get(&ocr_plan_path)
            .map(|document| &document.value);
        for message in critical_edition_local_structural_context_issues(
            &witness.value,
            target_unit,
            source_review,
            ocr_sample_plan,
        )? {
            checks.issue(&witness.path, message)?;
        }
        let admitted_by_decision =
            optional_documents
                .get(&citation_decision_path)
                .is_some_and(|decision| {
                    decision.value.get("status").and_then(Value::as_str)
                        == Some("human_admitted_with_limits")
                });
        if witness.value.get("status").and_then(Value::as_str) != Some("citation_witness_admitted")
            && !admitted_by_decision
        {
            if let Some(assisted) = optional_documents.get(&assisted_path) {
                let state = assisted.value.get("current_state").unwrap_or(&Value::Null);
                if checks.python_different(
                    state.get("admitted_critical_edition_units"),
                    Some(&Value::from(0)),
                )? {
                    checks.issue(
                        &witness.path,
                        "pending critical-edition witness inflated the admitted unit count",
                    )?;
                }
                if !array(state, "translation_lanes_opened").is_empty() {
                    checks.issue(
                        &witness.path,
                        "pending critical-edition witness opened a translation lane",
                    )?;
                }
            }
        }
    }

    if let Some(decision) = optional_documents.get(&citation_decision_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_SPECIALIZED_PACKETS);
        checks.schema(
            &decision.path,
            "ToS/contracts/critical-edition-citation-witness-decision.schema.json",
            &decision.value,
        )?;
        let historical_path =
            (critical_witness_paths.len() == 1).then(|| critical_witness_paths[0].as_str());
        let institutional_path = "ToS/source-witnesses/discovery/runs/ekgwb-za-i-vorrede-1-institutional-corroboration.2026-07-30.v3.json";
        let rights_path = path("rights.ekgwb.za-i-vorrede-1.v1.json");
        let research_path = "ToS/research-packets/foundation-laboratory-2026-07/EKGWB_CITATION_WITNESS_DECISION_REFRESH_2026-08-08.md";
        for (field, expected_path) in [
            ("historical_metadata_packet", historical_path),
            (
                "machine_triangulation_packet",
                Some(triangulation_path.as_str()),
            ),
            (
                "institutional_corroboration_record",
                Some(institutional_path),
            ),
            ("rights_record", Some(rights_path.as_str())),
            ("ordered_research_refresh", Some(research_path)),
        ] {
            let Some(expected_path) = expected_path else {
                checks.issue(
                    &decision.path,
                    format!("{field} has no current owner artifact"),
                )?;
                continue;
            };
            if checks.current_digest(expected_path)?.is_none() {
                checks.issue(
                    &decision.path,
                    format!("{field} has no current owner artifact"),
                )?;
                continue;
            }
            check_external_digest_binding(
                &mut checks,
                decision.value.get("evidence").unwrap_or(&Value::Null),
                field,
                expected_path,
                &decision.path,
            )?;
        }
        let decision_target = decision.value.get("target").unwrap_or(&Value::Null);
        let triangulation_target = optional_documents
            .get(&triangulation_path)
            .and_then(|document| document.value.get("target"));
        for field in [
            "review_unit_id",
            "context_anchor_ref",
            "critical_locator_siglum",
        ] {
            if checks.python_different(
                decision_target.get(field),
                triangulation_target.and_then(|target| target.get(field)),
            )? {
                checks.issue(
                    &decision.path,
                    format!("citation-witness decision target {field} drifted from triangulation"),
                )?;
            }
        }
        let fixity = decision
            .value
            .get("transport_and_fixity")
            .unwrap_or(&Value::Null);
        let triangulation_result = optional_documents
            .get(&triangulation_path)
            .and_then(|document| document.value.get("results"))
            .and_then(|results| results.get("ekgwb_reference"))
            .and_then(|reference| reference.get("normalized_sequence_sha256"));
        if checks.python_different(
            fixity.get("normalized_sequence_sha256"),
            triangulation_result,
        )? {
            checks.issue(
                &decision.path,
                "citation-witness normalized sequence digest drifted from triangulation",
            )?;
        }
        let institutional = checks.json(institutional_path, false)?;
        let mut selected_block_digests = BTreeSet::new();
        if let Some(institutional) = institutional {
            let selected_ids = string_set(institutional.value.get("selected_result_ids"));
            for channel in array(&institutional.value, "channels") {
                for result in array(channel, "results") {
                    if result
                        .get("result_id")
                        .and_then(Value::as_str)
                        .is_none_or(|id| !selected_ids.contains(id))
                    {
                        continue;
                    }
                    for identifier in array(result, "identifiers") {
                        if identifier.get("scheme").and_then(Value::as_str)
                            == Some("exact target block SHA-256")
                        {
                            if let Some(value) = identifier.get("value").and_then(Value::as_str) {
                                selected_block_digests.insert(value.to_owned());
                            }
                        }
                    }
                }
            }
        }
        let expected_block_digest = fixity
            .get("exact_target_block_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .into_iter()
            .collect::<BTreeSet<_>>();
        if selected_block_digests != expected_block_digest {
            checks.issue(
                &decision.path,
                "citation-witness target-block digest lacks exact selected-channel closure",
            )?;
        }
        if let Some(assisted) = optional_documents.get(&assisted_path) {
            let state = assisted.value.get("current_state").unwrap_or(&Value::Null);
            let admitted = decision.value.get("status").and_then(Value::as_str)
                == Some("human_admitted_with_limits");
            let expected_units = u64::from(admitted);
            let expected_lanes: Vec<Value> = if admitted {
                vec![
                    Value::String("ai_only".into()),
                    Value::String("ai_human".into()),
                ]
            } else {
                Vec::new()
            };
            if checks.python_different(
                state.get("admitted_critical_edition_units"),
                Some(&Value::from(expected_units)),
            )? {
                checks.issue(
                    &decision.path,
                    "citation-witness decision and assisted-review admitted count disagree",
                )?;
            }
            if checks.python_different(
                state.get("translation_lanes_opened"),
                Some(&Value::Array(expected_lanes)),
            )? {
                checks.issue(
                    &decision.path,
                    "citation-witness decision and assisted-review lanes disagree",
                )?;
            }
        }
    }

    let edition_admission_path = path("edition-reading-admission.dta-ekgwb.za-i-vorrede-1.v1.json");
    if let Some(admission) = optional_documents.get(&edition_admission_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_SPECIALIZED_PACKETS);
        checks.schema(
            &admission.path,
            "ToS/contracts/edition-reading-admission.schema.json",
            &admission.value,
        )?;
        let dta_item_root = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5";
        let research_path = "ToS/research-packets/foundation-laboratory-2026-07/GERMAN_EDITION_READING_ADMISSION_RESEARCH_2026-08-10.md";
        for (field, expected_path) in [
            (
                "item_manifest",
                format!("{dta_item_root}/item.manifest.json"),
            ),
            (
                "source_metadata_snapshot",
                format!("{dta_item_root}/source-metadata-snapshot.json"),
            ),
            ("rights_record", format!("{dta_item_root}/rights.json")),
            ("triangulation_packet", triangulation_path.clone()),
            ("critical_witness_decision", citation_decision_path.clone()),
            ("ordered_research", research_path.into()),
        ] {
            if checks.current_digest(&expected_path)?.is_none() {
                checks.issue(
                    &admission.path,
                    format!("evidence.{field} has no current owner artifact"),
                )?;
                continue;
            }
            check_external_digest_binding(
                &mut checks,
                admission.value.get("evidence").unwrap_or(&Value::Null),
                field,
                &expected_path,
                &admission.path,
            )?;
        }
        let manifest_path = format!("{dta_item_root}/item.manifest.json");
        let manifest = checks.json(&manifest_path, false)?;
        let source_identity = admission
            .value
            .get("source_identity")
            .unwrap_or(&Value::Null);
        if let Some(manifest) = manifest {
            if checks.python_different(
                source_identity.get("item_ref"),
                manifest.value.get("item_id"),
            )? {
                checks.issue(
                    &admission.path,
                    "edition-reading Item identity drifted from the DTA manifest",
                )?;
            }
            if checks.python_different(
                source_identity.get("edition_ref"),
                manifest.value.get("embodiment_ref"),
            )? {
                checks.issue(
                    &admission.path,
                    "edition-reading Edition identity drifted from the DTA manifest",
                )?;
            }
            let payloads = array(&manifest.value, "payload_files");
            let payload = (payloads.len() == 1).then(|| &payloads[0]);
            for (field, manifest_field) in [("file_ref", "file_id"), ("file_sha256", "sha256")] {
                if checks.python_different(
                    source_identity.get(field),
                    payload.and_then(|p| p.get(manifest_field)),
                )? {
                    checks.issue(
                        &admission.path,
                        format!("edition-reading {field} drifted from the DTA manifest"),
                    )?;
                }
            }
        }
        let triangulation = optional_documents
            .get(&triangulation_path)
            .map(|doc| &doc.value);
        let triangulation_target = triangulation.and_then(|doc| doc.get("target"));
        let target = admission.value.get("target").unwrap_or(&Value::Null);
        for field in [
            "review_unit_id",
            "context_anchor_ref",
            "critical_locator_siglum",
        ] {
            if checks.python_different(
                target.get(field),
                triangulation_target.and_then(|value| value.get(field)),
            )? {
                checks.issue(
                    &admission.path,
                    format!("edition-reading target {field} drifted from triangulation"),
                )?;
            }
        }
        let dta_result = triangulation
            .and_then(|triangulation| triangulation.get("results"))
            .and_then(|results| results.get("dta_exact_comparison"));
        let reading = admission
            .value
            .get("reading_evidence")
            .unwrap_or(&Value::Null);
        for (field, expected_field) in [
            ("source_selector", "section_selector"),
            ("paragraph_count", "paragraphs"),
            ("normalized_token_count", "normalized_tokens"),
            ("normalized_sequence_sha256", "normalized_sequence_sha256"),
            (
                "exact_after_source_aware_normalization",
                "exact_after_source_aware_normalization",
            ),
        ] {
            if checks.python_different(
                reading.get(field),
                dta_result.and_then(|result| result.get(expected_field)),
            )? {
                checks.issue(
                    &admission.path,
                    format!("edition-reading {field} drifted from exact DTA comparison"),
                )?;
            }
        }
        if checks.python_different(
            target.get("source_selector"),
            reading.get("source_selector"),
        )? {
            checks.issue(
                &admission.path,
                "edition-reading target selector and reading selector disagree",
            )?;
        }
        let decision_digest = optional_documents
            .get(&citation_decision_path)
            .and_then(|decision| decision.value.get("transport_and_fixity"))
            .and_then(|fixity| fixity.get("normalized_sequence_sha256"));
        if checks.python_different(reading.get("normalized_sequence_sha256"), decision_digest)? {
            checks.issue(
                &admission.path,
                "edition-reading sequence is not closed by the admitted critical witness",
            )?;
        }
        let rights_path = format!("{dta_item_root}/rights.json");
        if let Some(rights) = checks.json(&rights_path, false)? {
            let visibility = admission
                .value
                .get("rights_and_visibility")
                .unwrap_or(&Value::Null);
            if checks
                .python_different(visibility.get("rights_ref"), rights.value.get("rights_id"))?
            {
                checks.issue(
                    &admission.path,
                    "edition-reading rights identity drifted from the DTA record",
                )?;
            }
            if checks.python_different(
                visibility.get("rights_assessment_status"),
                rights.value.get("assessment_status"),
            )? {
                checks.issue(
                    &admission.path,
                    "edition-reading rights posture drifted from the DTA record",
                )?;
            }
        }
    }

    if let Some(candidate) = optional_documents.get(&path(
        "experimental-translation-candidate.admitted-ekgwb.za-i-vorrede-1-opening.variant-a.v1.json",
    )) {
        checks.set_diagnostic_stage(DIAG_STAGE_EXPERIMENTAL_PACKETS);
        checks.schema(
            &candidate.path,
            "ToS/contracts/experimental-translation-candidate.schema.json",
            &candidate.value,
        )?;
        let admission = candidate
            .value
            .get("admission")
            .unwrap_or(&Value::Null);
        for (field, expected_path) in [
            ("citation_witness_decision", citation_decision_path.as_str()),
            ("current_source_return_overlay", bounded_input_path.as_str()),
        ] {
            check_external_digest_binding(
                &mut checks,
                admission,
                field,
                expected_path,
                &candidate.path,
            )?;
        }
        let decision = optional_documents.get(&citation_decision_path);
        if decision.is_none_or(|decision| {
            decision.value.get("status").and_then(Value::as_str)
                != Some("human_admitted_with_limits")
                || !array(
                    decision
                        .value
                        .get("gate_effects")
                        .unwrap_or(&Value::Null),
                    "translation_lanes_opened",
                )
                .iter()
                .any(|lane| lane.as_str() == Some("ai_only"))
        }) {
            checks.issue(
                &candidate.path,
                "experimental translation candidate lacks an admitted AI-only citation-witness route",
            )?;
        } else if let Some(decision) = decision {
            let target = decision.value.get("target").unwrap_or(&Value::Null);
            let expected_target = serde_json::json!({
                "review_unit_id": target.get("review_unit_id"),
                "context_anchor_ref": target.get("context_anchor_ref"),
                "critical_locator_siglum": target.get("critical_locator_siglum"),
            });
            if checks.python_different(
                candidate.value.get("target"),
                Some(&expected_target),
            )? {
                checks.issue(
                    &candidate.path,
                    "experimental translation candidate target drifted from the admitted citation witness",
                )?;
            }
        }
    }

    for episode_path in &experimental_episode_paths {
        let Some(episode_doc) = optional_documents.get(episode_path) else {
            continue;
        };
        checks.set_diagnostic_stage(DIAG_STAGE_EXPERIMENTAL_PACKETS);
        checks.schema(
            &episode_doc.path,
            "ToS/contracts/experimental-translation-episode.schema.json",
            &episode_doc.value,
        )?;
        let episode = &episode_doc.value;
        let admission = episode.get("admission").unwrap_or(&Value::Null);
        let model_id = episode
            .get("method_freeze")
            .and_then(|freeze| freeze.get("model"))
            .and_then(|model| model.get("model_id"))
            .and_then(Value::as_str);
        let research_refresh_name = match model_id {
            Some("OpenVINO/Qwen3-8B-int4-ov") => {
                "LOCAL_LLM_TRANSLATION_QWEN3_8B_CPU_REFRESH_2026-08-08.md"
            }
            Some("google/madlad400-3b-mt") => "LOCAL_LLM_ADMISSION.md",
            _ => "LOCAL_LLM_TRANSLATION_CANDIDATE_REFRESH_2026-08-08.md",
        };
        let research_refresh_path =
            format!("ToS/research-packets/foundation-laboratory-2026-07/{research_refresh_name}");
        for (field, expected_path) in [
            ("citation_witness_decision", citation_decision_path.as_str()),
            ("current_source_return_overlay", bounded_input_path.as_str()),
            ("ordered_research_refresh", research_refresh_path.as_str()),
        ] {
            check_external_digest_binding(
                &mut checks,
                admission,
                field,
                expected_path,
                &episode_doc.path,
            )?;
        }
        if model_id == Some("google/madlad400-3b-mt") {
            check_external_digest_binding(
                &mut checks,
                admission,
                "specialized_mt_challenger_admission",
                "ToS/research-packets/foundation-laboratory-2026-07/SPECIALIZED_MT_CHALLENGER_ADMISSION_2026-08-10.md",
                &episode_doc.path,
            )?;
        }
        let decision = optional_documents.get(&citation_decision_path);
        if decision.is_none_or(|decision| {
            decision.value.get("status").and_then(Value::as_str)
                != Some("human_admitted_with_limits")
                || !array(
                    decision.value.get("gate_effects").unwrap_or(&Value::Null),
                    "translation_lanes_opened",
                )
                .iter()
                .any(|lane| lane.as_str() == Some("ai_only"))
        }) {
            checks.issue(
                &episode_doc.path,
                "experimental translation episode lacks an admitted AI-only citation-witness route",
            )?;
        } else if let Some(decision) = decision {
            let target = decision.value.get("target").unwrap_or(&Value::Null);
            let expected_target = serde_json::json!({
                "review_unit_id": target.get("review_unit_id"),
                "context_anchor_ref": target.get("context_anchor_ref"),
                "critical_locator_siglum": target.get("critical_locator_siglum"),
            });
            if checks.python_different(episode.get("target"), Some(&expected_target))? {
                checks.issue(
                    &episode_doc.path,
                    "experimental translation episode target drifted from the admitted citation witness",
                )?;
            }
        }
        if let Some(bounded) = optional_documents.get(&bounded_input_path) {
            let local = bounded.value.get("local_artifact").unwrap_or(&Value::Null);
            let private_run = episode.get("private_run").unwrap_or(&Value::Null);
            if checks.python_different(
                private_run.get("local_source_artifact_sha256"),
                local.get("artifact_sha256"),
            )? {
                checks.issue(
                    &episode_doc.path,
                    "experimental translation episode local source artifact digest drifted",
                )?;
            }
            if checks.python_different(
                private_run.get("source_text_sha256"),
                local.get("source_text_sha256"),
            )? {
                checks.issue(
                    &episode_doc.path,
                    "experimental translation episode source-text digest drifted",
                )?;
            }
        }
    }

    let laboratory_path = path("translation-laboratory-plan.v1.json");
    if let Some(lab) = optional_documents.get(&laboratory_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_LABORATORY_PACKETS);
        checks.schema(
            &lab.path,
            "ToS/contracts/translation-laboratory-plan.schema.json",
            &lab.value,
        )?;
        if lab
            .value
            .get("source_review_gate")
            .and_then(|gate| gate.get("review_plan_ref"))
            .and_then(Value::as_str)
            != Some(source_review_path.as_str())
        {
            checks.issue(
                &lab.path,
                "translation laboratory does not cite its v2 source-review plan",
            )?;
        }
        if optional_documents.contains_key(&source_review_path)
            && lab
                .value
                .get("source_review_gate")
                .and_then(|gate| gate.get("review_plan_sha256"))
                .and_then(Value::as_str)
                != optional_documents
                    .get(&source_review_path)
                    .map(|review| review.sha256.as_str())
        {
            checks.issue(&lab.path, "translation source-review plan digest drifted")?;
        }
        if let Some(translation) = &translation {
            if checks
                .python_different(lab.value.get("work_ref"), translation.value.get("work_ref"))?
            {
                checks.issue(&lab.path, "translation laboratory work_ref drifted")?;
            }
            if checks.python_different(
                lab.value.get("source_expression_ref"),
                translation.value.get("source_expression_ref"),
            )? {
                checks.issue(
                    &lab.path,
                    "translation laboratory source_expression_ref drifted",
                )?;
            }
            for key in ["expression_ref", "item_ref", "visibility"] {
                if checks.python_different(
                    lab.value
                        .get("recognized_comparator")
                        .and_then(|comparator| comparator.get(key)),
                    translation
                        .value
                        .get("recognized_comparator")
                        .and_then(|comparator| comparator.get(key)),
                )? {
                    checks.issue(
                        &lab.path,
                        format!("translation laboratory comparator {key} drifted"),
                    )?;
                }
            }
        }
        for field in [
            "translation_packet_schema_ref",
            "semantic_ladder_schema_ref",
        ] {
            let reference = lab.value.get(field).and_then(Value::as_str);
            let resolved = if let Some(reference) = reference {
                checks.repository_ref_exists(&Value::String(reference.into()))?
            } else {
                false
            };
            if !resolved {
                checks.issue(
                    &lab.path,
                    format!("translation laboratory {field} is unresolved"),
                )?;
            }
        }
        let gate = lab.value.get("source_review_gate").unwrap_or(&Value::Null);
        if gate
            .get("current_human_accepted_units")
            .and_then(Value::as_i64)
            .is_some_and(|count| count < 30)
        {
            if lab.value.get("status").and_then(Value::as_str)
                != Some("frozen-blocked-on-human-source-acceptance")
            {
                checks.issue(
                    &lab.path,
                    "pre-acceptance translation laboratory is not blocked",
                )?;
            }
            if let Some(lanes) = lab.value.get("blind_lanes").and_then(Value::as_object) {
                for (lane_name, lane) in lanes {
                    if !lane.is_object()
                        || lane.get("state").and_then(Value::as_str)
                            != Some("blocked-on-source-acceptance")
                    {
                        checks.issue(
                            &lab.path,
                            format!("pre-acceptance lane {lane_name} is not blocked"),
                        )?;
                    }
                }
            }
            let comparator = lab
                .value
                .get("recognized_comparator")
                .unwrap_or(&Value::Null);
            if comparator.get("visibility").and_then(Value::as_str) != Some("sealed")
                || ["content_consulted", "content_emitted"]
                    .iter()
                    .any(|field| comparator.get(field).and_then(Value::as_bool) != Some(false))
            {
                checks.issue(
                    &lab.path,
                    "pre-acceptance comparator is not completely sealed",
                )?;
            }
        }
    }

    let exposure_path = path("translation-exposure-aware-plan.v1.json");
    if let Some(exposure) = optional_documents.get(&exposure_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_LABORATORY_PACKETS);
        checks.schema(
            &exposure.path,
            "ToS/contracts/translation-exposure-aware-plan.schema.json",
            &exposure.value,
        )?;
        let research_path = "ToS/research-packets/foundation-laboratory-2026-07/TRANSLATION_EXPOSURE_AWARE_SOLO_AI_RESEARCH_2026-08-12.md";
        let current_project_path =
            path("antonovsky-2007-1911-opening-sentence-collation.plan.v1.json");
        let route = exposure
            .value
            .get("assisted_source_route")
            .unwrap_or(&Value::Null);
        let evidence = exposure
            .value
            .get("participant_exposures")
            .and_then(|participants| participants.get("human_operator"))
            .and_then(|operator| operator.get("current_project_comparator_exposure"))
            .and_then(|exposure| exposure.get("evidence"));
        let expected_bindings: [(&str, Option<&Value>, &str); 6] = [
            (
                "base_plan",
                exposure.value.get("base_plan"),
                laboratory_path.as_str(),
            ),
            (
                "method_research",
                exposure.value.get("method_research"),
                research_path,
            ),
            (
                "assisted_source_route.german_assisted_review",
                route.get("german_assisted_review"),
                assisted_path.as_str(),
            ),
            (
                "assisted_source_route.citation_witness_decision",
                route.get("citation_witness_decision"),
                citation_decision_path.as_str(),
            ),
            (
                "assisted_source_route.edition_reading_admission",
                route.get("edition_reading_admission"),
                edition_admission_path.as_str(),
            ),
            (
                "participant_exposures.human_operator.current_project_comparator_exposure.evidence",
                evidence,
                current_project_path.as_str(),
            ),
        ];
        for (field, binding, expected_path) in expected_bindings {
            if checks.current_digest(expected_path)?.is_none() {
                checks.issue(
                    &exposure.path,
                    format!("{field} has no current owner artifact"),
                )?;
                continue;
            }
            check_external_digest_binding(
                &mut checks,
                &exposure.value,
                field,
                expected_path,
                &exposure.path,
            )?;
        }
        if let Some(lab) = optional_documents.get(&laboratory_path) {
            let historical = lab
                .value
                .get("recognized_comparator")
                .unwrap_or(&Value::Null);
            let comparator = exposure
                .value
                .get("recognized_comparator")
                .unwrap_or(&Value::Null);
            for key in ["expression_ref", "item_ref"] {
                if checks.python_different(comparator.get(key), historical.get(key))? {
                    checks.issue(
                        &exposure.path,
                        format!("exposure-aware comparator {key} drifted from the frozen plan"),
                    )?;
                }
            }
        }
        if let Some(assisted) = optional_documents.get(&assisted_path) {
            let assisted_state = assisted.value.get("current_state").unwrap_or(&Value::Null);
            for (field, source_field) in [
                ("experimental_lanes_opened", "translation_lanes_opened"),
                ("accepted_german_units", "accepted_german_units"),
                ("promotion_authorized", "promotion_authorized"),
            ] {
                if checks.python_different(route.get(field), assisted_state.get(source_field))? {
                    checks.issue(
                        &exposure.path,
                        format!(
                            "assisted_source_route.{field} drifted from the German-assisted review"
                        ),
                    )?;
                }
            }
        }
    }

    let reference_register = optional_documents.get(&reference_register_path);
    if optional_documents.contains_key(&laboratory_path) && reference_register.is_none() {
        checks.issue(
            root,
            "translation laboratory has no translation-reference-register.v1.json",
        )?;
    }
    if let Some(register) = reference_register {
        checks.set_diagnostic_stage(DIAG_STAGE_LABORATORY_PACKETS);
        checks.schema(
            &register.path,
            "ToS/contracts/translation-reference-register.schema.json",
            &register.value,
        )?;
        if register
            .value
            .get("laboratory_plan_ref")
            .and_then(Value::as_str)
            != Some(laboratory_path.as_str())
        {
            checks.issue(
                &register.path,
                "translation reference register does not cite its laboratory plan",
            )?;
        }
        if let Some(lab) = optional_documents.get(&laboratory_path) {
            if checks.python_different(register.value.get("work_ref"), lab.value.get("work_ref"))? {
                checks.issue(
                    &register.path,
                    "translation reference register work_ref drifted",
                )?;
            }
        }
        let entries = array(&register.value, "entries");
        let required_categories = string_set(register.value.get("required_categories"));
        let mut actual_categories = BTreeSet::new();
        let mut reference_ids = BTreeSet::new();
        let mut bibliographic_reviews = 0_u64;
        let mut rights_reviews = 0_u64;
        for entry in entries {
            if !entry.is_object() {
                continue;
            }
            let reference_id = entry
                .get("reference_id")
                .map(display)
                .unwrap_or_else(|| "None".into());
            if !reference_ids.insert(reference_id.clone()) {
                checks.issue(
                    &register.path,
                    format!("duplicate translation reference_id: {reference_id}"),
                )?;
            }
            if let Some(category) = entry.get("category").and_then(Value::as_str) {
                actual_categories.insert(category.to_owned());
            }
            let access = entry.get("access").unwrap_or(&Value::Null);
            if access.is_object() {
                let access_state = access.get("access_state").and_then(Value::as_str);
                let request_required = access
                    .get("access_request_required_before_content_use")
                    .and_then(Value::as_bool);
                let contact_routes = array(access, "contact_routes");
                let request_state = access.get("request_state").and_then(Value::as_str);
                if request_required == Some(true) && contact_routes.is_empty() {
                    checks.issue(
                        &register.path,
                        format!("{reference_id} requires access but has no contact route"),
                    )?;
                }
                if request_required == Some(true)
                    && request_state == Some("not-required-for-web-consultation")
                {
                    checks.issue(
                        &register.path,
                        format!("{reference_id} suppresses its required access route"),
                    )?;
                }
                if matches!(access_state, Some("open-web" | "open-download-candidate"))
                    && request_required != Some(false)
                {
                    checks.issue(
                        &register.path,
                        format!("{reference_id} marks open consultation as request-gated"),
                    )?;
                }
            }
            let rights = entry.get("rights").unwrap_or(&Value::Null);
            if rights.is_object() {
                let assessment = rights.get("assessment").and_then(Value::as_str);
                if assessment == Some("declared-license")
                    && !rights
                        .get("license_uri")
                        .is_some_and(|license_uri| !license_uri.is_null() && license_uri != "")
                {
                    checks.issue(
                        &register.path,
                        format!("{reference_id} declares a license without its URI"),
                    )?;
                }
                if matches!(assessment, Some("in-copyright" | "copyright-not-evaluated"))
                    && (rights.get("redistribution").and_then(Value::as_str)
                        == Some("authorized-with-conditions")
                        || rights.get("derivative_use").and_then(Value::as_str)
                            == Some("allowed-with-conditions"))
                {
                    checks.issue(
                        &register.path,
                        format!("{reference_id} grants reuse without a reviewed license"),
                    )?;
                }
            }
            let admission = entry.get("admission").unwrap_or(&Value::Null);
            bibliographic_reviews +=
                if admission.get("human_bibliographic_review") == Some(&Value::Bool(true)) {
                    1
                } else {
                    0
                };
            rights_reviews += if admission.get("human_rights_review") == Some(&Value::Bool(true)) {
                1
            } else {
                0
            };
            let tos_refs = entry.get("tos_refs").unwrap_or(&Value::Null);
            for record_ref in array(tos_refs, "record_refs") {
                let Some(record_ref) = record_ref.as_str() else {
                    continue;
                };
                let known = if record_ref.starts_with("tos.rights.") {
                    records.rights_contains(record_ref)?
                } else {
                    records.current_record(record_ref)?.is_some()
                };
                if !known {
                    checks.issue(
                        &register.path,
                        format!("{reference_id} has unresolved record ref: {record_ref}"),
                    )?;
                }
            }
            for path_ref in array(tos_refs, "path_refs") {
                let path_text = path_ref.as_str();
                let exists = if let Some(path_text) = path_text {
                    if RelativePath::parse(path_text).is_err() {
                        false
                    } else {
                        checks.checkpoint()?;
                        let exists = checks.source.exists(
                            path_text,
                            checks.limits.max_member_bytes,
                            checks.limits.deadline,
                        )?;
                        checks.checkpoint()?;
                        exists
                    }
                } else {
                    false
                };
                if !exists {
                    checks.issue(
                        &register.path,
                        format!(
                            "{reference_id} has unresolved path ref: {}",
                            display(path_ref)
                        ),
                    )?;
                }
            }
        }
        let missing_categories: Vec<String> = required_categories
            .difference(&actual_categories)
            .cloned()
            .collect();
        if !missing_categories.is_empty() {
            let rendered = python_string_list(&missing_categories);
            checks.issue(
                &register.path,
                format!("translation reference categories are missing: {rendered}"),
            )?;
        }
        if let Some(coverage) = register
            .value
            .get("coverage")
            .filter(|value| value.is_object())
        {
            let expected_coverage = serde_json::json!({
                "required_category_count": required_categories.len(),
                "entry_count": entries.len(),
                "all_required_categories_present": missing_categories.is_empty(),
                "content_admitted_entries": 0,
                "human_bibliographic_reviews": bibliographic_reviews,
                "human_rights_reviews": rights_reviews,
                "permission_requests_sent": 0,
            });
            if coverage != &expected_coverage {
                checks.issue(
                    &register.path,
                    "translation reference coverage summary drifted",
                )?;
            }
        }
        if let Some(lab) = optional_documents.get(&laboratory_path) {
            let comparator = lab
                .value
                .get("recognized_comparator")
                .unwrap_or(&Value::Null);
            let expression_ref = comparator.get("expression_ref");
            let item_ref = comparator.get("item_ref");
            let matches = entries
                .iter()
                .filter(|entry| {
                    entry.get("category").and_then(Value::as_str)
                        == Some("recognized_ru_translation_candidate")
                        && [expression_ref, item_ref]
                            .into_iter()
                            .flatten()
                            .all(|reference| {
                                array(entry.get("tos_refs").unwrap_or(&Value::Null), "record_refs")
                                    .contains(reference)
                            })
                })
                .count();
            if matches != 1 {
                checks.issue(
                    &register.path,
                    "sealed recognized comparator must resolve to exactly one reference entry",
                )?;
            }
        }
    }

    let ocr_plan_path = path("ocr-visual-samples.json");
    let ocr_anchor_path = path("ocr-anchors.jsonl");
    let mut ocr_sample_ids = BTreeSet::new();
    let mut ocr_plan_anchor_ids = BTreeSet::new();
    if let Some(ocr) = optional_documents.get(&ocr_plan_path) {
        checks.set_diagnostic_stage(ocr_diagnostic_stage);
        if ocr
            .value
            .get("projection_of_plan_ref")
            .and_then(Value::as_str)
            != Some(path(SAMPLE_PLAN).as_str())
        {
            checks.issue(
                &ocr.path,
                "OCR visual plan does not project the frozen general sample plan",
            )?;
        }
        let discovery_ref = ocr
            .value
            .get("source_discovery_event_ref")
            .and_then(Value::as_str);
        let unresolved_discovery = match discovery_ref {
            Some(event_ref) => {
                !source_events.event_contains(event_ref)?
                    && !seen_gold_event_ids.contains(event_ref)
            }
            None => true,
        };
        if unresolved_discovery {
            checks.issue(&ocr.path, "OCR source_discovery_event_ref is unresolved")?;
        }
        let projection_ref = ocr
            .value
            .get("provenance_event_ref")
            .and_then(Value::as_str);
        let projection_event = projection_ref.and_then(|event_ref| events_by_id.get(event_ref));
        if projection_event.is_none() {
            checks.issue(
                &ocr.path,
                "OCR provenance_event_ref is absent from gold-set provenance",
            )?;
        } else if let Some(event) = projection_event {
            for output_path in [&ocr_plan_path, &ocr_anchor_path] {
                let output_ref = output_path.as_str();
                let output = array(event, "outputs")
                    .iter()
                    .find(|output| output.get("ref").and_then(Value::as_str) == Some(output_ref));
                if output.is_none() {
                    checks.issue(
                        &ocr.path,
                        format!("OCR provenance omits output: {output_ref}"),
                    )?;
                } else {
                    let actual_digest = checks.current_digest(output_ref)?;
                    if actual_digest.is_none()
                        || output
                            .and_then(|value| value.get("sha256"))
                            .and_then(Value::as_str)
                            != actual_digest.as_deref()
                    {
                        checks.issue(
                            &ocr.path,
                            format!("OCR provenance digest differs: {output_ref}"),
                        )?;
                    }
                }
            }
        }
        let render_ref = ocr
            .value
            .get("render_specification")
            .and_then(|specification| specification.get("render_manifest_ref"))
            .and_then(Value::as_str);
        let local_prefix = format!("{root}/local-content/");
        if render_ref.is_none_or(|reference| {
            RelativePath::parse(reference).is_err() || !reference.starts_with(&local_prefix)
        }) {
            checks.issue(
                &ocr.path,
                "OCR render manifest must stay under gold-set local-content",
            )?;
        }
        if let Some(render_ref) = render_ref {
            let facts = physical.private_paths.get(render_ref);
            if facts.and_then(|facts| facts.git_ignored) != Some(true) {
                checks.issue(
                    &ocr.path,
                    "planned OCR render manifest is not inside the ignored local-content lane",
                )?;
            }
            match facts.map(physical_target_observation) {
                Some(PhysicalTargetObservation::Target(target)) => {
                    if let Some(relative_target) = target.relative_target {
                        if RelativePath::parse(relative_target).is_err()
                            || !relative_target.starts_with(&local_prefix)
                        {
                            checks.issue(
                                &ocr.path,
                                "OCR render manifest must stay under gold-set local-content",
                            )?;
                        }
                    }
                }
                Some(PhysicalTargetObservation::OutsideSelectedRoot) => checks.issue(
                    &ocr.path,
                    "OCR render manifest must stay under gold-set local-content",
                )?,
                Some(PhysicalTargetObservation::Unknown) => checks.coverage_gap(format!(
                    "{render_ref}: OCR render-manifest symlink target is not observed"
                ))?,
                None => checks.coverage_gap(format!(
                    "{render_ref}: OCR render-manifest physical observation is unavailable"
                ))?,
            }
        }
        for item_ref in array(
            ocr.value
                .get("reference_witness_reveal_law")
                .unwrap_or(&Value::Null),
            "reference_item_refs",
        ) {
            let target = match item_ref.as_str() {
                Some(reference) => records.current_record(reference)?,
                None => None,
            };
            if target.as_deref().is_none_or(|record| record.kind != "item") {
                checks.issue(
                    &ocr.path,
                    format!(
                        "unresolved OCR reference witness item: {}",
                        display(item_ref)
                    ),
                )?;
            }
        }
        let groups = array(&ocr.value, "source_groups");
        if groups.len() != 3 {
            checks.issue(
                &ocr.path,
                "OCR visual plan must have exactly three source groups",
            )?;
        }
        let mut group_ids = BTreeSet::new();
        let mut group_items = BTreeSet::new();
        let mut replacement_count = 0usize;
        for group in groups {
            if !group.is_object() {
                continue;
            }
            let group_id = group.get("group_id").and_then(Value::as_str);
            let item_ref = group.get("item_ref").and_then(Value::as_str);
            let file_ref = group.get("file_ref").cloned().unwrap_or(Value::Null);
            if group_id.is_none_or(|id| !group_ids.insert(id.to_owned())) {
                checks.issue(
                    &ocr.path,
                    format!(
                        "duplicate or invalid OCR group_id: {}",
                        group
                            .get("group_id")
                            .map(display)
                            .unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            if item_ref.is_none_or(|id| !group_items.insert(id.to_owned())) {
                checks.issue(
                    &ocr.path,
                    format!(
                        "duplicate or invalid OCR source item: {}",
                        group
                            .get("item_ref")
                            .map(display)
                            .unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            let item_value = group.get("item_ref").cloned().unwrap_or(Value::Null);
            if !records.file_contains(&item_value, &file_ref)? {
                checks.issue(
                    &ocr.path,
                    format!(
                        "OCR file_ref {} does not belong to {}",
                        display(&file_ref),
                        display(&item_value)
                    ),
                )?;
            }
            let file_sha256 = records.file_sha256(&file_ref)?;
            if checks.python_different(file_sha256.as_deref(), group.get("file_sha256"))? {
                checks.issue(
                    &ocr.path,
                    format!(
                        "OCR file_sha256 differs from manifest file {}",
                        display(&file_ref)
                    ),
                )?;
            }
            let samples = array(group, "samples");
            if samples.len() != 12 {
                checks.issue(
                    &ocr.path,
                    format!("{} must have exactly 12 OCR samples", display(&item_value)),
                )?;
            }
            let mut group_gold = 0usize;
            for sample in samples {
                if !sample.is_object() {
                    continue;
                }
                let sample_id = sample.get("sample_id").and_then(Value::as_str);
                let source_sample_id = sample.get("source_sample_id");
                let anchor_ref = sample.get("anchor_ref");
                let page = sample.get("page");
                let projection_change = sample.get("projection_change").and_then(Value::as_str);
                let unique_sample_id = if let Some(id) = sample_id {
                    checks.insert_string(&mut ocr_sample_ids, id)?
                } else {
                    false
                };
                if !unique_sample_id {
                    checks.issue(
                        &ocr.path,
                        format!(
                            "duplicate or invalid OCR sample_id: {}",
                            sample
                                .get("sample_id")
                                .map(display)
                                .unwrap_or_else(|| "None".into())
                        ),
                    )?;
                }
                if let Some(anchor_ref) = anchor_ref.and_then(Value::as_str) {
                    ocr_plan_anchor_ids.insert(anchor_ref.to_owned());
                }
                if sample.get("gold_candidate") == Some(&Value::Bool(true)) {
                    group_gold += 1;
                }
                let anchor = anchor_ref
                    .and_then(Value::as_str)
                    .and_then(|reference| anchors_by_id.get(reference));
                if anchor.is_none() {
                    checks.issue(
                        &ocr.path,
                        format!(
                            "unresolved OCR anchor: {}",
                            anchor_ref.map(display).unwrap_or_else(|| "None".into())
                        ),
                    )?;
                } else if let Some(anchor) = anchor {
                    if checks.python_different(anchor.get("item_id"), Some(&item_value))?
                        || checks.python_different(anchor.get("file_id"), Some(&file_ref))?
                    {
                        checks.issue(
                            &ocr.path,
                            format!(
                                "OCR anchor {} crosses its source group",
                                anchor_ref.map(display).unwrap_or_else(|| "None".into())
                            ),
                        )?;
                    }
                    let page_selectors: Vec<&Value> = array(anchor, "selectors")
                        .iter()
                        .filter(|selector| {
                            selector.get("type").and_then(Value::as_str) == Some("page_region")
                        })
                        .collect();
                    if page_selectors.len() != 1 {
                        checks.issue(
                            &ocr.path,
                            format!(
                                "OCR anchor {} must have one page selector",
                                anchor_ref.map(display).unwrap_or_else(|| "None".into())
                            ),
                        )?;
                    } else {
                        let selector = page_selectors[0];
                        if checks.python_different(selector.get("page"), page)? {
                            checks.issue(
                                &ocr.path,
                                format!(
                                    "OCR sample page differs from anchor: {}",
                                    anchor_ref.map(display).unwrap_or_else(|| "None".into())
                                ),
                            )?;
                        }
                        let page_shape = [("x", 0.0), ("y", 0.0), ("width", 1.0), ("height", 1.0)];
                        if page_shape.iter().any(|(field, expected)| {
                            selector.get(*field).and_then(numeric) != Some(*expected)
                        }) || selector.get("coordinate_space").and_then(Value::as_str)
                            != Some("normalized_0_1")
                        {
                            checks.issue(
                                &ocr.path,
                                format!(
                                    "OCR anchor is not a full normalized page: {}",
                                    anchor_ref.map(display).unwrap_or_else(|| "None".into())
                                ),
                            )?;
                        }
                    }
                }
                let source_id = source_sample_id.and_then(Value::as_str);
                let source_binding = source_id.and_then(|id| sample_bindings.get(id));
                if source_binding.is_none() {
                    checks.issue(
                        &ocr.path,
                        format!(
                            "OCR sample does not resolve to a frozen source sample: {}",
                            source_sample_id
                                .map(display)
                                .unwrap_or_else(|| "None".into())
                        ),
                    )?;
                    continue;
                }
                let source_binding = source_binding.unwrap();
                let source_anchor = source_binding
                    .anchor_ref
                    .as_str()
                    .and_then(|reference| anchors_by_id.get(reference));
                if source_anchor.is_none() {
                    checks.issue(
                        &ocr.path,
                        format!(
                            "OCR source sample anchor is unresolved: {}",
                            source_sample_id
                                .map(display)
                                .unwrap_or_else(|| "None".into())
                        ),
                    )?;
                    continue;
                }
                let source_anchor = source_anchor.unwrap();
                match projection_change {
                    Some("same_visual_unit") => {
                        if source_binding.item_ref != item_value
                            || source_binding.file_ref != file_ref
                            || source_binding.anchor_ref
                                != anchor_ref.cloned().unwrap_or(Value::Null)
                        {
                            checks.issue(
                                &ocr.path,
                                format!(
                                    "same_visual_unit changed source binding: {}",
                                    sample_id.unwrap_or("None")
                                ),
                            )?;
                        }
                    }
                    Some("same_scan_page_for_epub_member") => {
                        if source_binding.item_ref == item_value {
                            checks.issue(
                                &ocr.path,
                                format!(
                                    "scan projection did not move to a distinct source item: {}",
                                    sample_id.unwrap_or("None")
                                ),
                            )?;
                        }
                        let source_edition = match source_binding.item_ref.as_str() {
                            Some(id) => records.item_edition(id)?,
                            None => None,
                        };
                        let target_edition = match item_ref {
                            Some(id) => records.item_edition(id)?,
                            None => None,
                        };
                        if source_edition.as_deref() != target_edition.as_deref() {
                            checks.issue(
                                &ocr.path,
                                format!(
                                    "scan projection crosses edition identity: {}",
                                    sample_id.unwrap_or("None")
                                ),
                            )?;
                        }
                        let member_path = array(source_anchor, "selectors")
                            .iter()
                            .find(|selector| {
                                selector.get("type").and_then(Value::as_str)
                                    == Some("container_member")
                            })
                            .and_then(|selector| selector.get("member_path"))
                            .and_then(Value::as_str);
                        let mapped_page = member_path
                            .and_then(page_member_index)
                            .and_then(|index| index.checked_add(1));
                        if mapped_page.is_none()
                            || numeric(page.unwrap_or(&Value::Null))
                                != mapped_page.map(|v| v as f64)
                        {
                            checks.issue(
                                &ocr.path,
                                format!(
                                    "EPUB member to PDF page mapping differs: {}",
                                    sample_id.unwrap_or("None")
                                ),
                            )?;
                        }
                    }
                    Some("replacement_for_nonvisual_unit") => {
                        replacement_count += 1;
                        let source_edition = match source_binding.item_ref.as_str() {
                            Some(id) => records.item_edition(id)?,
                            None => None,
                        };
                        let target_edition = match item_ref {
                            Some(id) => records.item_edition(id)?,
                            None => None,
                        };
                        if source_edition.as_deref() != target_edition.as_deref() {
                            checks.issue(
                                &ocr.path,
                                format!(
                                    "OCR replacement crosses edition identity: {}",
                                    sample_id.unwrap_or("None")
                                ),
                            )?;
                        }
                        let source_members: Vec<&Value> = array(source_anchor, "selectors")
                            .iter()
                            .filter(|selector| {
                                selector.get("type").and_then(Value::as_str)
                                    == Some("container_member")
                            })
                            .filter_map(|selector| selector.get("member_path"))
                            .collect();
                        let source_pages: Vec<&Value> = array(source_anchor, "selectors")
                            .iter()
                            .filter(|selector| {
                                selector.get("type").and_then(Value::as_str) == Some("page_region")
                            })
                            .collect();
                        if source_members.len() != 1
                            || source_members[0].as_str() != Some("EPUB/notice.html")
                            || !source_pages.is_empty()
                            || numeric(page.unwrap_or(&Value::Null)) != Some(2.0)
                        {
                            checks.issue(
                                &ocr.path,
                                format!("OCR replacement is not the declared notice-to-page-2 substitution: {}", sample_id.unwrap_or("None")),
                            )?;
                        }
                    }
                    _ => checks.issue(
                        &ocr.path,
                        format!(
                            "unsupported OCR projection change: {}",
                            sample
                                .get("projection_change")
                                .map(display)
                                .unwrap_or_else(|| "None".into())
                        ),
                    )?,
                }
            }
            if group_gold != 5 {
                checks.issue(
                    &ocr.path,
                    format!(
                        "{} must have exactly five OCR gold candidates",
                        display(&item_value)
                    ),
                )?;
            }
        }
        if ocr_sample_ids.len() != 36 {
            checks.issue(
                &ocr.path,
                "OCR visual plan must contain exactly 36 unique sample IDs",
            )?;
        }
        let declared_replacements = ocr
            .value
            .get("projection_law")
            .and_then(|law| law.get("replacement_count"));
        if checks.python_different(
            declared_replacements,
            Some(&Value::from(replacement_count as u64)),
        )? {
            checks.issue(
                &ocr.path,
                "actual OCR replacement count differs from projection law",
            )?;
        }
    }

    checks.set_diagnostic_stage(DIAG_STAGE_RETRIEVAL_PLAN);
    if let Some(document) = &retrieval {
        checks.schema(
            &document.path,
            "ToS/contracts/retrieval-query-plan.schema.json",
            &document.value,
        )?;
        checks.set_diagnostic_stage(retrieval_diagnostic_stage);
        // Query-local identities and anchor closure are source rules; retrieval
        // ranking quality and all optional visual routes remain other owners.
        let mut query_ids = BTreeSet::new();
        if document
            .value
            .get("provenance_event_ref")
            .and_then(Value::as_str)
            .is_none_or(|id| !local_event_ids.contains(id))
        {
            checks.issue(
                &document.path,
                "retrieval provenance_event_ref is absent from gold-set provenance",
            )?;
        }
        for query in array(&document.value, "queries") {
            let id = query.get("query_id").and_then(Value::as_str);
            if id.is_none() || !query_ids.insert(id.unwrap_or_default().to_owned()) {
                checks.issue(
                    &document.path,
                    format!(
                        "duplicate or invalid query_id: {}",
                        query
                            .get("query_id")
                            .map(display)
                            .unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            for field in ["expected_source_anchor_refs", "hard_negative_anchor_refs"] {
                for anchor in array(query, field) {
                    if let Some(anchor) = anchor.as_str() {
                        if !anchors_by_id.contains_key(anchor) {
                            checks.issue(
                                &document.path,
                                format!("unresolved retrieval anchor: {anchor}"),
                            )?;
                        }
                    }
                }
            }
        }
        if query_ids.len() != 20 {
            checks.issue(
                &document.path,
                "retrieval query plan must contain exactly 20 unique IDs",
            )?;
        }
        let content_ref = document
            .value
            .get("query_content_ref")
            .map(display)
            .unwrap_or_else(|| "None".into());
        let content_path = format!("{root}/{content_ref}");
        let expected_local_prefix = format!("{root}/local-content/");
        let content_path_is_safe = RelativePath::parse(&content_path).is_ok()
            && content_path.starts_with(&expected_local_prefix);
        if !content_path_is_safe {
            checks.issue(
                &document.path,
                "query content must stay under gold-set local-content",
            )?;
        }
        let content_facts = physical.private_paths.get(&content_path);
        let mut query_content = None;
        if content_path_is_safe {
            if let Some(facts) = content_facts {
                match physical_target_observation(facts) {
                    PhysicalTargetObservation::Target(target) => {
                        let read_path = if let Some(relative_target) = target.relative_target {
                            if RelativePath::parse(relative_target).is_err()
                                || !relative_target.starts_with(&expected_local_prefix)
                            {
                                checks.issue(
                                    &document.path,
                                    "query content must stay under gold-set local-content",
                                )?;
                                None
                            } else {
                                Some(relative_target)
                            }
                        } else {
                            Some(content_path.as_str())
                        };
                        if !target.exists || !target.regular_file {
                            if require_local_payloads {
                                checks.issue(
                                    &content_path,
                                    "required local retrieval query content is missing",
                                )?;
                            }
                        } else if let Some(read_path) = read_path {
                            let parsed = checks.json_at(read_path, &content_path, false)?;
                            if parsed.is_none() && !checks.current_digests.contains_key(read_path) {
                                checks.coverage_gap(format!(
                                    "{content_path}: local retrieval query bytes are outside the captured source read"
                                ))?;
                            }
                            if parsed.as_ref().is_some_and(|parsed| {
                                target
                                    .sha256
                                    .is_some_and(|expected| parsed.sha256 != expected)
                            }) {
                                return Err(ItemRefusal::Source(
                                    "gold-set query-content observations disagree".into(),
                                ));
                            }
                            query_content = parsed;
                        }
                    }
                    PhysicalTargetObservation::OutsideSelectedRoot => {
                        checks.issue(
                            &document.path,
                            "query content must stay under gold-set local-content",
                        )?;
                    }
                    PhysicalTargetObservation::Unknown => {
                        checks.coverage_gap(format!(
                            "{content_path}: local query-content symlink target is not observed"
                        ))?;
                    }
                }
            } else {
                checks.coverage_gap(format!(
                    "{content_path}: exact physical query-content observation is unavailable"
                ))?;
            }
        }
        if let Some(query_content) = query_content {
            let expected_content_digest = Some(
                document
                    .value
                    .get("query_content_sha256")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            );
            if Some(query_content.sha256.as_str()) != expected_content_digest {
                checks.issue(
                    &content_path,
                    "query content digest differs from retrieval plan",
                )?;
            }
            if query_content
                .value
                .get("frozen_before_variant_outputs")
                .and_then(Value::as_bool)
                != Some(true)
            {
                checks.issue(&content_path, "query content was not frozen before outputs")?;
            }
            let content_ids: BTreeSet<String> = array(&query_content.value, "queries")
                .iter()
                .filter_map(|query| {
                    query
                        .get("query_id")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect();
            if content_ids != query_ids || array(&query_content.value, "queries").len() != 20 {
                checks.issue(
                    &content_path,
                    "local query IDs differ from tracked retrieval plan",
                )?;
            }
        }
        if content_facts.and_then(|facts| facts.git_ignored) != Some(true) {
            checks.issue(
                &content_path,
                "local retrieval query content is not ignored",
            )?;
        }
    }

    let visual_plan_path = path("visual-retrieval-plan.v1.json");
    if let Some(visual) = optional_documents.get(&visual_plan_path) {
        checks.set_diagnostic_stage(DIAG_STAGE_RETRIEVAL_PLAN);
        checks.schema(
            &visual.path,
            "ToS/contracts/visual-retrieval-plan.schema.json",
            &visual.value,
        )?;
        for message in visual_retrieval_plan_issues(
            &visual.value,
            retrieval.as_ref().map(|document| &document.value),
            sample.as_ref().map(|document| &document.value),
            optional_documents
                .get(&ocr_plan_path)
                .map(|document| &document.value),
        )? {
            checks.issue(&visual.path, message)?;
        }
        checks.set_diagnostic_stage(visual_diagnostic_stage);
        let projection = visual.value.get("query_projection").unwrap_or(&Value::Null);
        let query_content_path = format!("{root}/local-content/retrieval/queries.v1.json");
        let expected_query_plan_digest =
            retrieval.as_ref().map(|document| document.sha256.as_str());
        if projection
            .get("source_query_plan_ref")
            .and_then(Value::as_str)
            != Some(path(RETRIEVAL_PLAN).as_str())
        {
            checks.issue(
                &visual.path,
                "visual retrieval source query-plan reference drifted",
            )?;
        } else if projection
            .get("source_query_plan_sha256")
            .and_then(Value::as_str)
            != expected_query_plan_digest
        {
            checks.issue(
                &visual.path,
                "visual retrieval source query-plan digest drifted",
            )?;
        }
        if projection.get("query_content_ref").and_then(Value::as_str)
            != Some(query_content_path.as_str())
        {
            checks.issue(
                &visual.path,
                "visual retrieval local query-content reference drifted",
            )?;
        }
        if let Some(retrieval) = retrieval.as_ref() {
            if checks.python_different(
                projection.get("query_content_sha256"),
                retrieval.value.get("query_content_sha256"),
            )? {
                checks.issue(
                    &visual.path,
                    "visual retrieval query-content digest differs from the text retrieval plan",
                )?;
            }
        }
        let crosswalk = projection
            .get("source_to_visual_anchor_crosswalk")
            .unwrap_or(&Value::Null);
        let visual_doc = optional_documents.get(&ocr_plan_path);
        let sample_path = path(SAMPLE_PLAN);
        let expected_crosswalk = [
            ("source_sample_plan_ref", sample_path.as_str()),
            (
                "source_sample_plan_sha256",
                sample
                    .as_ref()
                    .map(|document| document.sha256.as_str())
                    .unwrap_or_default(),
            ),
            ("visual_sample_plan_ref", ocr_plan_path.as_str()),
        ];
        for (field, expected) in expected_crosswalk {
            if crosswalk.get(field).and_then(Value::as_str) != Some(expected) {
                checks.issue(
                    &visual.path,
                    format!("visual retrieval crosswalk {field} drifted"),
                )?;
            }
        }
        let visual_digest = visual_doc.map(|document| document.sha256.as_str());
        if crosswalk
            .get("visual_sample_plan_sha256")
            .and_then(Value::as_str)
            != visual_digest
        {
            checks.issue(
                &visual.path,
                "visual retrieval crosswalk visual_sample_plan_sha256 drifted",
            )?;
        }
        let page_corpus = visual
            .value
            .get("page_image_corpus")
            .unwrap_or(&Value::Null);
        for (field, expected) in [
            ("visual_sample_plan_ref", ocr_plan_path.as_str()),
            (
                "visual_sample_plan_sha256",
                visual_digest.unwrap_or_default(),
            ),
        ] {
            if page_corpus.get(field).and_then(Value::as_str) != Some(expected) {
                checks.issue(
                    &visual.path,
                    format!("visual retrieval page-image corpus {field} drifted"),
                )?;
            }
        }
        let expected_render_ref = visual_doc
            .and_then(|document| document.value.get("render_specification"))
            .and_then(|render| render.get("render_manifest_ref"));
        if checks.python_different(page_corpus.get("render_manifest_ref"), expected_render_ref)? {
            checks.issue(
                &visual.path,
                "visual retrieval page-image corpus render_manifest_ref drifted",
            )?;
        }
        let render_ref = expected_render_ref.and_then(Value::as_str);
        if let Some(render_ref) = render_ref {
            let render_path = if render_ref.starts_with("ToS/") {
                render_ref.to_owned()
            } else {
                format!("{root}/{render_ref}")
            };
            if let Some(facts) = physical.private_paths.get(&render_path) {
                match physical_target_observation(facts) {
                    PhysicalTargetObservation::Target(target) => {
                        let target_is_local = target.relative_target.is_none_or(|path| {
                            RelativePath::parse(path).is_ok()
                                && path.starts_with(&format!("{root}/local-content/"))
                        });
                        if target.exists && target.regular_file && target_is_local {
                            if let Some(actual_digest) = target.sha256 {
                                if Some(actual_digest)
                                    != page_corpus
                                        .get("render_manifest_sha256")
                                        .and_then(Value::as_str)
                                {
                                    checks.issue(
                                        &visual.path,
                                        "visual retrieval ignored render-manifest digest drifted",
                                    )?;
                                }
                            } else {
                                checks.coverage_gap(format!(
                                    "{render_path}: render manifest digest observation is unavailable"
                                ))?;
                            }
                        }
                    }
                    PhysicalTargetObservation::OutsideSelectedRoot => {
                        // The earlier OCR render-reference check reports the
                        // maintained local-content containment issue.
                    }
                    PhysicalTargetObservation::Unknown => checks.coverage_gap(format!(
                        "{render_path}: render manifest symlink target is not observed"
                    ))?,
                }
            } else {
                checks.coverage_gap(format!(
                    "{render_path}: selected local render-manifest observation is unavailable"
                ))?;
            }
        }
        let visual_event = visual
            .value
            .get("provenance_event_ref")
            .and_then(Value::as_str)
            .and_then(|event_id| events_by_id.get(event_id));
        if visual_event.is_none() {
            checks.issue(
                &visual.path,
                "visual retrieval provenance_event_ref is absent",
            )?;
        } else if let Some(event) = visual_event {
            let sample_ref = path(SAMPLE_PLAN);
            let visual_sample_ref = path("ocr-visual-samples.json");
            let expected_inputs = BTreeSet::from([
                (
                    Some(path(RETRIEVAL_PLAN)),
                    Some("frozen-text-retrieval-query-and-anchor-plan".to_owned()),
                    retrieval.as_ref().map(|document| document.sha256.clone()),
                ),
                (
                    Some(sample_ref.clone()),
                    Some("frozen-source-sample-plan".to_owned()),
                    sample.as_ref().map(|document| document.sha256.clone()),
                ),
                (
                    Some(visual_sample_ref),
                    Some("frozen-page-image-sample-projection".to_owned()),
                    visual_doc.map(|document| document.sha256.clone()),
                ),
            ]);
            let actual_inputs: BTreeSet<_> = array(event, "inputs")
                .iter()
                .map(|input| {
                    (
                        input.get("ref").and_then(Value::as_str).map(str::to_owned),
                        input.get("role").and_then(Value::as_str).map(str::to_owned),
                        input
                            .get("sha256")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    )
                })
                .collect();
            let contract_path = "ToS/contracts/visual-retrieval-plan.schema.json";
            let contract_digest = checks.current_digest(contract_path)?;
            let expected_outputs = BTreeSet::from([
                (
                    Some(contract_path.to_owned()),
                    Some("direct-page-image-retrieval-plan-contract".to_owned()),
                    contract_digest,
                ),
                (
                    Some(visual.path.clone()),
                    Some("frozen-direct-page-image-retrieval-plan".to_owned()),
                    Some(visual.sha256.clone()),
                ),
            ]);
            let actual_outputs: BTreeSet<_> = array(event, "outputs")
                .iter()
                .map(|output| {
                    (
                        output.get("ref").and_then(Value::as_str).map(str::to_owned),
                        output
                            .get("role")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        output
                            .get("sha256")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    )
                })
                .collect();
            if !expected_inputs.is_subset(&actual_inputs) {
                checks.issue(&visual.path, "visual retrieval provenance inputs drifted")?;
            }
            if !expected_outputs.is_subset(&actual_outputs) {
                checks.issue(&visual.path, "visual retrieval provenance outputs drifted")?;
            }
        }

        let artifact_ref = page_corpus
            .get("render_artifact_ref")
            .and_then(Value::as_str);
        if artifact_ref.is_none_or(|reference| {
            reference.starts_with('/') || RelativePath::parse(reference).is_err()
        }) {
            checks.issue(
                &visual.path,
                "visual retrieval owner artifact reference is unsafe",
            )?;
        } else if let Some(reference) = artifact_ref {
            let facts = physical.artifact_paths.get(reference);
            if facts.is_none() {
                checks.coverage_gap(format!(
                    "{reference}: exact visual retrieval owner artifact observation is unavailable"
                ))?;
            } else if let Some(facts) = facts {
                match physical_target_observation(facts) {
                    PhysicalTargetObservation::Target(target)
                        if target.exists && target.regular_file =>
                    {
                        if let Some(actual_digest) = target.sha256 {
                            if Some(actual_digest)
                                != page_corpus
                                    .get("render_manifest_sha256")
                                    .and_then(Value::as_str)
                            {
                                checks.issue(
                                    &visual.path,
                                    "visual retrieval owner render-manifest digest drifted",
                                )?;
                            }
                        } else {
                            checks.coverage_gap(format!(
                                "{reference}: visual retrieval owner artifact digest observation is unavailable"
                            ))?;
                        }
                    }
                    PhysicalTargetObservation::Target(_) => {}
                    PhysicalTargetObservation::OutsideSelectedRoot => {
                        checks.coverage_gap(format!(
                            "{reference}: visual retrieval owner artifact target escapes the selected artifact root"
                        ))?;
                    }
                    PhysicalTargetObservation::Unknown => {
                        checks.coverage_gap(format!(
                            "{reference}: visual retrieval owner artifact symlink target is not observed"
                        ))?;
                    }
                }
            }
        }

        for control in array(&visual.value, "fixed_controls") {
            if !control.is_object() {
                continue;
            }
            let label = control
                .get("label")
                .map(display)
                .unwrap_or_else(|| "None".into());
            let run_ref = control.get("run_ref").and_then(Value::as_str);
            if run_ref.is_none_or(|reference| {
                reference.starts_with('/') || RelativePath::parse(reference).is_err()
            }) {
                checks.issue(
                    &visual.path,
                    format!("visual retrieval control {label} artifact reference is unsafe"),
                )?;
                continue;
            }
            let run_ref = run_ref.unwrap();
            let Some(run_facts) = physical.artifact_paths.get(run_ref) else {
                checks.coverage_gap(format!(
                    "{run_ref}: visual retrieval control artifact observation is unavailable"
                ))?;
                continue;
            };
            let run_exists = match physical_target_observation(run_facts) {
                PhysicalTargetObservation::Target(target) => target.exists,
                PhysicalTargetObservation::OutsideSelectedRoot => {
                    checks.coverage_gap(format!(
                        "{run_ref}: visual retrieval control target escapes the selected artifact root"
                    ))?;
                    continue;
                }
                PhysicalTargetObservation::Unknown => {
                    checks.coverage_gap(format!(
                        "{run_ref}: visual retrieval control path symlink target is not observed"
                    ))?;
                    continue;
                }
            };
            if !run_exists {
                continue;
            }
            let receipt_ref = format!("{}/run.receipt.json", run_ref.trim_end_matches('/'));
            let Some(receipt_facts) = physical.artifact_paths.get(&receipt_ref) else {
                checks.coverage_gap(format!(
                    "{receipt_ref}: visual retrieval control receipt observation is unavailable"
                ))?;
                continue;
            };
            match physical_target_observation(receipt_facts) {
                PhysicalTargetObservation::Target(target)
                    if target.exists && target.regular_file =>
                {
                    if let Some(actual_digest) = target.sha256 {
                        if control.get("run_receipt_sha256").and_then(Value::as_str)
                            != Some(actual_digest)
                        {
                            checks.issue(
                                &visual.path,
                                format!("visual retrieval control {label} receipt digest drifted"),
                            )?;
                        }
                    } else {
                        checks.coverage_gap(format!(
                            "{receipt_ref}: visual retrieval control receipt digest observation is unavailable"
                        ))?;
                    }
                }
                PhysicalTargetObservation::Target(_) => checks.issue(
                    &visual.path,
                    format!("visual retrieval control {label} run receipt is missing"),
                )?,
                PhysicalTargetObservation::OutsideSelectedRoot => {
                    checks.coverage_gap(format!(
                        "{receipt_ref}: visual retrieval control receipt target escapes the selected artifact root"
                    ))?;
                }
                PhysicalTargetObservation::Unknown => checks.coverage_gap(format!(
                    "{receipt_ref}: visual retrieval control receipt symlink target is not observed"
                ))?,
            }
        }
    }

    checks.set_diagnostic_stage(DIAG_STAGE_RETRIEVAL_PLAN);
    if let Some(graph) = &graph {
        checks.schema(
            &graph.path,
            "ToS/contracts/graph-query-plan.schema.json",
            &graph.value,
        )?;
    }

    let mut graph_claim_ids = BTreeSet::new();
    let mut graph_predicates = BTreeSet::new();
    for row in &graph_claim_rows {
        checks.set_diagnostic_stage(graph_claim_validation_stage);
        checks.schema(
            &row.location,
            "ToS/contracts/claim-packet.schema.json",
            &row.value,
        )?;
        if let Some(id) = row.value.get("claim_id").and_then(Value::as_str) {
            if !checks.insert_string(&mut graph_claim_ids, id)? {
                checks.issue(
                    &row.location,
                    format!("duplicate or invalid graph claim_id: {id}"),
                )?;
            } else if prior_graph_claim_ids.contains(id) {
                checks.issue(&row.location, format!("duplicate claim_id: {id}"))?;
            }
        } else {
            checks.issue(
                &row.location,
                format!(
                    "duplicate or invalid graph claim_id: {}",
                    row.value
                        .get("claim_id")
                        .map(display)
                        .unwrap_or_else(|| "None".into())
                ),
            )?;
        }
        if let Some(predicate) = row.value.get("predicate").and_then(Value::as_str) {
            checks.insert_string(&mut graph_predicates, predicate)?;
        }
        let event_ref = row
            .value
            .get("provenance_event_ref")
            .and_then(Value::as_str);
        if event_ref.is_none_or(|id| !local_event_ids.contains(id)) {
            checks.issue(
                &row.location,
                "graph claim provenance event is absent from the gold set",
            )?;
        }
        for evidence_ref in array(&row.value, "evidence_refs") {
            if evidence_ref
                .as_str()
                .is_some_and(|reference| reference.starts_with("ToS/"))
                && !checks.repository_ref_exists(evidence_ref)?
            {
                checks.issue(
                    &row.location,
                    format!(
                        "graph claim evidence path is missing: {}",
                        display(evidence_ref)
                    ),
                )?;
            }
        }
    }

    checks.set_diagnostic_stage(graph_closure_diagnostic_stage);
    for row in &graph_claim_rows {
        for field in ["subject_ref", "object"] {
            if let Some(reference) = row.value.get(field).and_then(Value::as_str) {
                let known = records.current_record(reference)?.is_some()
                    || anchors_by_id.contains_key(reference)
                    || source_events.event_contains(reference)?
                    || seen_gold_event_ids.contains(reference)
                    || local_event_ids.contains(reference)
                    || records.rights_contains(reference)?
                    || prior_graph_claim_ids.contains(reference)
                    || graph_claim_ids.contains(reference);
                if reference.starts_with("tos.") && !known {
                    checks.issue(
                        &row.location,
                        format!("unresolved graph claim {field}: {reference}"),
                    )?;
                }
            }
        }
        for alternative_ref in array(&row.value, "alternative_claim_refs") {
            if alternative_ref.as_str().is_none_or(|id| {
                !graph_claim_rows
                    .iter()
                    .any(|other| other.value.get("claim_id").and_then(Value::as_str) == Some(id))
            }) {
                checks.issue(
                    &row.location,
                    format!(
                        "unresolved graph alternative claim: {}",
                        display(alternative_ref)
                    ),
                )?;
            }
        }
    }

    if let Some(document) = &graph {
        checks.set_diagnostic_stage(graph_plan_diagnostic_stage);
        let plan_claim_ref = document.value.get("claim_set_ref").and_then(Value::as_str);
        if plan_claim_ref != Some(graph_claim_path.as_str()) {
            checks.issue(
                &document.path,
                "graph query plan claim_set_ref does not own graph-claims.jsonl",
            )?;
        }
        if document
            .value
            .get("claim_set_sha256")
            .and_then(Value::as_str)
            != checks
                .current_digests
                .get(&graph_claim_path)
                .map(String::as_str)
        {
            checks.issue(
                &document.path,
                "graph claim-set digest differs from graph query plan",
            )?;
        }
        let expected_layers: BTreeSet<&str> =
            ["bibliographic", "textual", "provenance", "interpretive"]
                .into_iter()
                .collect();
        let actual_layers: BTreeSet<&str> = array(&document.value, "graph_layers")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if actual_layers != expected_layers {
            checks.issue(
                &document.path,
                "graph query plan must keep exactly four logical layers",
            )?;
        }
        let declared_predicates: BTreeSet<&str> = array(&document.value, "allowed_predicates")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let actual_predicates: BTreeSet<&str> =
            graph_predicates.iter().map(String::as_str).collect();
        if declared_predicates != actual_predicates {
            checks.issue(
                &document.path,
                "allowed graph predicates differ from frozen claims",
            )?;
        }
        let mut query_ids = BTreeSet::new();
        for query in array(&document.value, "queries") {
            let id = query.get("query_id").and_then(Value::as_str);
            if id.is_none() || !query_ids.insert(id.unwrap_or_default().to_owned()) {
                checks.issue(
                    &document.path,
                    format!(
                        "duplicate or invalid graph query_id: {}",
                        query
                            .get("query_id")
                            .map(display)
                            .unwrap_or_else(|| "None".into())
                    ),
                )?;
            }
            let missing_claims: Vec<String> = array(query, "expected_claim_refs")
                .iter()
                .filter_map(Value::as_str)
                .filter(|claim_ref| !graph_claim_ids.contains(*claim_ref))
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if !missing_claims.is_empty() {
                checks.issue(
                    &document.path,
                    format!(
                        "graph query references claims outside frozen set: {}",
                        python_string_list(&missing_claims)
                    ),
                )?;
            }
            if query.get("operation").and_then(Value::as_str) == Some("claim_family") {
                let root = query
                    .get("parameters")
                    .and_then(|p| p.get("seed_claim_ref"))
                    .and_then(Value::as_str);
                if root.is_none_or(|id| !graph_claim_ids.contains(id)) {
                    checks.issue(
                        &document.path,
                        format!(
                            "graph claim-family root is unresolved: {}",
                            root.unwrap_or("None")
                        ),
                    )?;
                }
            }
        }
        if query_ids.len() != 10 {
            checks.issue(
                &document.path,
                "graph query plan must contain exactly 10 unique IDs",
            )?;
        }
    }

    let graph_provenance_prefix = format!("{}:", path("graph-provenance.jsonl"));
    checks.set_diagnostic_stage(graph_provenance_diagnostic_stage);
    for row in provenance_rows
        .iter()
        .filter(|row| row.location.starts_with(&graph_provenance_prefix))
    {
        for output in array(&row.value, "outputs") {
            let reference = output.get("ref").and_then(Value::as_str);
            let Some(reference) = reference.filter(|reference| reference.starts_with("ToS/"))
            else {
                continue;
            };
            let expected_digest = output.get("sha256").and_then(Value::as_str);
            let actual_digest = checks.current_digest(reference)?;
            if actual_digest.is_none() {
                checks.issue(
                    &path("graph-provenance.jsonl"),
                    format!("graph provenance output is missing: {reference}"),
                )?;
            } else if expected_digest
                .is_some_and(|expected| actual_digest.as_deref() != Some(expected))
            {
                checks.issue(
                    &path("graph-provenance.jsonl"),
                    format!("graph provenance output digest differs: {reference}"),
                )?;
            }
        }
    }

    checks.set_diagnostic_stage(anchor_closure_diagnostic_stage);
    let mut referenced_anchor_ids = BTreeSet::<String>::new();
    for binding in sample_bindings.values() {
        if let Some(anchor_ref) = binding.anchor_ref.as_str() {
            referenced_anchor_ids.insert(anchor_ref.to_owned());
        }
    }
    referenced_anchor_ids.extend(ocr_plan_anchor_ids);
    if let Some(document) = &retrieval {
        for query in array(&document.value, "queries") {
            for field in ["expected_source_anchor_refs", "hard_negative_anchor_refs"] {
                referenced_anchor_ids.extend(
                    array(query, field)
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                );
            }
        }
    }
    for row in &graph_claim_rows {
        for reference in ["subject_ref", "object"]
            .into_iter()
            .filter_map(|field| row.value.get(field).and_then(Value::as_str))
            .chain(
                array(&row.value, "evidence_refs")
                    .iter()
                    .filter_map(Value::as_str),
            )
        {
            if reference.starts_with("tos.anchor.") {
                referenced_anchor_ids.insert(reference.to_owned());
            }
        }
    }
    if let Some(document) = optional_documents.get(&path("initial-sign-packet.v5.json")) {
        let source_gate = document
            .value
            .get("task_specific_source_gate")
            .unwrap_or(&Value::Null);
        referenced_anchor_ids.extend(
            array(source_gate, "source_anchor_refs")
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned),
        );
        for stage in array(&document.value, "stages") {
            referenced_anchor_ids.extend(
                array(stage, "source_anchor_refs")
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned),
            );
        }
    }
    if let Some(document) = &translation {
        referenced_anchor_ids.extend(
            array(&document.value, "fragments")
                .iter()
                .filter_map(|fragment| fragment.get("source_anchor_ref").and_then(Value::as_str))
                .map(str::to_owned),
        );
    }
    if let Some(document) = &transfer {
        referenced_anchor_ids.extend(
            array(&document.value, "candidate_target_units")
                .iter()
                .filter_map(|unit| unit.get("anchor_ref").and_then(Value::as_str))
                .map(str::to_owned),
        );
    }
    let unreferenced: Vec<String> = anchors_by_id
        .keys()
        .filter(|anchor_id| !referenced_anchor_ids.contains(*anchor_id))
        .cloned()
        .collect();
    if !unreferenced.is_empty() {
        checks.issue(
            root,
            format!(
                "unreferenced anchors: {}",
                python_string_list(&unreferenced)
            ),
        )?;
    }

    let local_content_root = path("local-content");
    checks.set_diagnostic_stage(local_content_diagnostic_stage);
    let route_card = format!("{local_content_root}/README.md");
    let route_facts = physical.authored_paths.get(&route_card);
    match route_facts.map(physical_target_observation) {
        Some(PhysicalTargetObservation::Target(target)) if target.exists && target.regular_file => {
            if route_facts.and_then(|facts| facts.git_tracked) == Some(false)
                || route_facts.and_then(|facts| facts.git_ignored) == Some(true)
            {
                checks.issue(&route_card, "local-content route card must remain tracked")?;
            } else if route_facts.and_then(|facts| facts.git_tracked) != Some(true) {
                checks.coverage_gap(format!(
                    "{route_card}: Git tracking observation is unavailable"
                ))?;
            }
        }
        Some(PhysicalTargetObservation::Target(_)) => {
            checks.issue(&route_card, "local-content route card is missing")?;
        }
        Some(PhysicalTargetObservation::OutsideSelectedRoot) => checks.coverage_gap(format!(
            "{route_card}: route-card symlink target escapes the selected source root"
        ))?,
        Some(PhysicalTargetObservation::Unknown) | None => checks.coverage_gap(format!(
            "{route_card}: route-card physical target observation is unavailable"
        ))?,
    }
    if let Some(local_content_files) = physical.private_inventories.get(&local_content_root) {
        for local_path in local_content_files {
            checks.checkpoint()?;
            if local_path == &route_card {
                continue;
            }
            let Some(facts) = physical.private_paths.get(local_path) else {
                checks.coverage_gap(format!(
                    "{local_path}: exact physical and Git observation is unavailable"
                ))?;
                continue;
            };
            if !facts.exists {
                checks.coverage_gap(format!(
                    "{local_path}: local-content inventory and physical snapshot disagree"
                ))?;
            } else {
                match physical_target_observation(facts) {
                    PhysicalTargetObservation::Target(target)
                        if target.exists && target.regular_file =>
                    {
                        if facts.git_ignored != Some(true) {
                            checks.issue(
                                local_path,
                                "restricted local-content file is not ignored",
                            )?;
                        }
                    }
                    PhysicalTargetObservation::Target(_) => {}
                    PhysicalTargetObservation::OutsideSelectedRoot => {
                        checks.coverage_gap(format!(
                            "{local_path}: local-content symlink target escapes the selected source root"
                        ))?;
                    }
                    PhysicalTargetObservation::Unknown => checks.coverage_gap(format!(
                        "{local_path}: local-content symlink target type is not observed"
                    ))?,
                }
            }
        }
    } else {
        checks.coverage_gap(format!(
            "{local_content_root}: complete physical local-content inventory was not supplied"
        ))?;
    }

    for id in graph_claim_ids {
        checks.charge(id.len() + size_of::<String>() + 32)?;
        checks.report.graph_claim_ids.push(id);
    }

    for event_id in event_order {
        if let Some(event) = events_by_id.remove(&event_id) {
            checks.charge(event_id.len() + encoded_len(&event)? * 2 + 64)?;
            checks.report.source_events.push((event_id, event));
        }
    }
    checks.finalize_diagnostics()?;
    Ok(checks.report)
}

/// Minimum-delay invariant shared with the maintained Python helper.
pub fn solo_recheck_delay_issue(unit: &Value, minimum_delay: &Value) -> Option<String> {
    if unit.get("current_assurance").and_then(Value::as_str) != Some("solo_human_delayed_rechecked")
    {
        return None;
    }
    let observed = unit
        .get("review_evidence")
        .and_then(|v| v.get("observed_delay_hours"));
    let minimum = numeric(minimum_delay);
    let observed = numeric(observed.unwrap_or(&Value::Null));
    if let (Some(minimum), Some(observed)) = (minimum, observed) {
        if observed < minimum {
            return Some(format!(
                "{} solo recheck is below the declared delay floor",
                unit.get("sample_id")
                    .map(display)
                    .unwrap_or_else(|| "None".into())
            ));
        }
    } else if numeric(minimum_delay).is_some() && observed.is_none() {
        return Some(format!(
            "{} solo recheck is below the declared delay floor",
            unit.get("sample_id")
                .map(display)
                .unwrap_or_else(|| "None".into())
        ));
    }
    None
}

/// Ensures a language scope does not permit and block the same claim.
pub fn language_scope_overlap_issue(scope: &Value) -> Option<String> {
    let allowed: BTreeSet<&str> = array(scope, "allowed_claims")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let blocked: BTreeSet<&str> = array(scope, "blocked_claims")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let overlap: Vec<&str> = allowed.intersection(&blocked).copied().collect();
    if overlap.is_empty() {
        return None;
    }
    Some(format!(
        "{} language scope both allows and blocks: {}",
        scope
            .get("language")
            .map(display)
            .unwrap_or_else(|| "None".into()),
        overlap.join(", ")
    ))
}

/// Keeps the declared reference use within the human-assurance state.
pub fn assurance_reference_use_issue(unit: &Value) -> Option<String> {
    let assurance = unit.get("current_assurance").and_then(Value::as_str)?;
    let expected = match assurance {
        "unreviewed" | "language_competence_blocked" | "rejected" | "uncertain" => "none",
        "single_human_source_visible" => "criteria_evidence_only",
        "solo_human_delayed_rechecked" => "calibration_metrics_with_disclosure",
        "independent_multi_human_adjudicated" => "independent_multi_human_gold",
        _ => return None,
    };
    if unit.get("reference_use").and_then(Value::as_str) == Some(expected) {
        return None;
    }
    Some(format!(
        "{} reference use {} is invalid for {}",
        unit.get("sample_id")
            .map(display)
            .unwrap_or_else(|| "None".into()),
        unit.get("reference_use")
            .map(display)
            .unwrap_or_else(|| "None".into()),
        assurance
    ))
}

/// Checks the frozen calibration/unscheduled partition and keeps selected
/// sparse-calibration observations from being promoted.
pub fn human_work_schedule_issues(
    units: &BTreeMap<String, Value>,
    schedule: &Value,
) -> Result<Vec<String>, ItemRefusal> {
    let Some(schedule) = schedule.as_object() else {
        return Ok(vec!["human-work schedule is not an object".into()]);
    };
    let selected = string_set(schedule.get("selected_calibration_unit_ids"));
    let unscheduled = string_set(schedule.get("unscheduled_unit_ids"));
    let mut issues = Vec::new();
    if !selected.is_disjoint(&unscheduled) {
        issues.push("selected calibration and unscheduled units overlap".into());
    }
    let partition: BTreeSet<String> = selected.union(&unscheduled).cloned().collect();
    if partition != units.keys().cloned().collect() {
        issues.push("human-work schedule does not partition the frozen packet".into());
    }
    if python_json_different(schedule.get("human_debt_units"), Some(&Value::from(0)))? {
        issues.push("closed sparse calibration must report zero human debt".into());
    }
    for sample_id in selected {
        let unit = units.get(&sample_id);
        if unit
            .and_then(|u| u.get("current_assurance"))
            .and_then(Value::as_str)
            != Some("unreviewed")
            || unit
                .and_then(|u| u.get("reference_use"))
                .and_then(Value::as_str)
                != Some("none")
            || unit
                .and_then(|u| u.get("next_route"))
                .and_then(Value::as_str)
                != Some("none")
        {
            issues.push(format!(
                "{sample_id} sparse calibration observation was promoted"
            ));
        }
    }
    Ok(issues)
}

/// Preserves the maintained packet-identity closure across semantic-ladder
/// stages. The result is a source-shape predicate only; it does not admit a
/// sign, relation, concept, claim, or graph projection.
pub fn semantic_ladder_identity_issues(payload: &Value) -> Result<Vec<String>, ItemRefusal> {
    if !payload.is_object() {
        return Ok(vec!["semantic ladder packet is not an object".into()]);
    }
    let stages: BTreeMap<&str, &Value> = array(payload, "stages")
        .iter()
        .filter_map(|stage| Some((stage.get("stage")?.as_str()?, stage)))
        .collect();
    let result = payload.get("result").unwrap_or(&Value::Null);
    let mut issues = Vec::new();

    let candidate_stage = stages
        .get("stable_sign_candidate")
        .copied()
        .unwrap_or(&Value::Null);
    let candidate_body = candidate_stage.get("body").unwrap_or(&Value::Null);
    if candidate_body.is_object()
        && !matches!(
            candidate_stage.get("status").and_then(Value::as_str),
            Some("blocked" | "not-started")
        )
    {
        if python_json_different(
            candidate_body.get("candidate_ref"),
            payload.get("candidate_ref"),
        )? {
            issues.push("stable-sign candidate identity differs from packet candidate_ref".into());
        }
        let earlier_names = [
            "exact_form",
            "frequency_and_concordance",
            "context",
            "morphology",
            "lemma",
            "recurrence_within_section",
            "recurrence_within_work",
            "recurrence_within_author_corpus",
        ];
        let earlier_occurrences: BTreeSet<&str> = earlier_names
            .iter()
            .filter_map(|name| stages.get(name).copied())
            .flat_map(|stage| array(stage.get("body").unwrap_or(&Value::Null), "occurrence_refs"))
            .filter_map(Value::as_str)
            .collect();
        let candidate_occurrences: BTreeSet<&str> = array(candidate_body, "occurrence_refs")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if candidate_occurrences.is_empty()
            || !candidate_occurrences.is_subset(&earlier_occurrences)
        {
            issues.push(
                "stable-sign candidate does not resolve to earlier occurrence evidence".into(),
            );
        }
    }

    let manual_stage = stages
        .get("manual_confirmation_or_rejection")
        .copied()
        .unwrap_or(&Value::Null);
    let manual_body = manual_stage.get("body").unwrap_or(&Value::Null);
    if manual_stage.get("status").and_then(Value::as_str) == Some("human-accepted") {
        if !manual_body.is_object()
            || python_json_different(
                manual_body.get("accepted_sign_ref"),
                payload.get("accepted_sign_ref"),
            )?
        {
            issues
                .push("human sign decision identity differs from packet accepted_sign_ref".into());
        }
        let mut receipt_found = false;
        if let Some(receipt) = manual_body.get("review_receipt_ref") {
            for result_receipt in array(result, "human_decision_refs") {
                if python_json_equal(result_receipt, receipt)? {
                    receipt_found = true;
                    break;
                }
            }
        }
        if !receipt_found {
            issues.push("human sign decision receipt is absent from packet result".into());
        }
    }

    let relation_stage = stages
        .get("relations_between_signs")
        .copied()
        .unwrap_or(&Value::Null);
    let relation_body = relation_stage.get("body").unwrap_or(&Value::Null);
    let relation_records = array(relation_body, "relation_records");
    let relation_refs: BTreeSet<&str> = relation_records
        .iter()
        .filter_map(|record| record.get("relation_ref")?.as_str())
        .collect();
    let relation_claim_refs: BTreeSet<&str> = relation_records
        .iter()
        .filter_map(|record| record.get("claim_ref")?.as_str())
        .collect();
    let declared_sign_refs: BTreeSet<&str> = array(relation_body, "sign_refs")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if !relation_records.is_empty() {
        let record_sign_refs: BTreeSet<&str> = relation_records
            .iter()
            .flat_map(|record| {
                [
                    record.get("subject_sign_ref"),
                    record.get("object_sign_ref"),
                ]
            })
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if record_sign_refs != declared_sign_refs {
            issues.push("relation record sign endpoints differ from declared sign_refs".into());
        }
        let result_relation_refs = string_set(result.get("relation_refs"));
        let result_claim_refs = string_set(result.get("claim_refs"));
        if relation_refs
            .iter()
            .any(|reference| !result_relation_refs.contains(*reference))
        {
            issues.push("relation identities are absent from packet result".into());
        }
        if relation_claim_refs
            .iter()
            .any(|reference| !result_claim_refs.contains(*reference))
        {
            issues.push("relation claims are absent from packet result".into());
        }
    }

    let concept_stage = stages
        .get("conceptual_interpretations")
        .copied()
        .unwrap_or(&Value::Null);
    let concept_body = concept_stage.get("body").unwrap_or(&Value::Null);
    if concept_body.is_object()
        && !matches!(
            concept_stage.get("status").and_then(Value::as_str),
            Some("blocked" | "not-started")
        )
    {
        if python_json_different(
            concept_body.get("accepted_sign_ref"),
            payload.get("accepted_sign_ref"),
        )? {
            issues.push("concept interpretation sign differs from packet accepted_sign_ref".into());
        }
        for (field, result_field, message) in [
            (
                "concept_refs",
                "concept_refs",
                "concept identities are absent from packet result",
            ),
            (
                "claim_refs",
                "claim_refs",
                "concept claims are absent from packet result",
            ),
        ] {
            let result_refs = string_set(result.get(result_field));
            if string_set(concept_body.get(field))
                .iter()
                .any(|reference| !result_refs.contains(reference))
            {
                issues.push(message.into());
            }
        }
    }

    let counter_stage = stages
        .get("competing_readings")
        .copied()
        .unwrap_or(&Value::Null);
    let counter_body = counter_stage.get("body").unwrap_or(&Value::Null);
    if counter_body.is_object()
        && !matches!(
            counter_stage.get("status").and_then(Value::as_str),
            Some("blocked" | "not-started")
        )
    {
        let mut counter_claim_refs = string_set(counter_body.get("primary_claim_refs"));
        counter_claim_refs.extend(string_set(counter_body.get("competing_claim_refs")));
        let result_claim_refs = string_set(result.get("claim_refs"));
        if counter_claim_refs
            .iter()
            .any(|reference| !result_claim_refs.contains(reference))
        {
            issues.push("competing-reading claims are absent from packet result".into());
        }
    }

    let graph_stage = stages
        .get("graph_projection")
        .copied()
        .unwrap_or(&Value::Null);
    let graph_body = graph_stage.get("body").unwrap_or(&Value::Null);
    if graph_stage.get("status").and_then(Value::as_str) == Some("projected")
        && graph_body.is_object()
    {
        if string_set(graph_body.get("relation_refs"))
            .iter()
            .any(|reference| !relation_refs.contains(reference.as_str()))
        {
            issues.push("graph relation identities do not resolve to relation records".into());
        }
        let result_claim_refs = string_set(result.get("claim_refs"));
        if string_set(graph_body.get("claim_refs"))
            .iter()
            .any(|reference| !result_claim_refs.contains(reference))
        {
            issues.push("graph claims are absent from packet result".into());
        }
        let mut projection_found = false;
        if let Some(projection) = graph_body.get("projection_ref") {
            for result_projection in array(result, "graph_projection_refs") {
                if python_json_equal(result_projection, projection)? {
                    projection_found = true;
                    break;
                }
            }
        }
        if !projection_found {
            issues.push("graph projection is absent from packet result".into());
        }
    }
    Ok(issues)
}

pub fn critical_edition_local_structural_context_issues(
    packet: &Value,
    target_unit: Option<&Value>,
    source_review_plan: Option<&Value>,
    ocr_sample_plan: Option<&Value>,
) -> Result<Vec<String>, ItemRefusal> {
    if !packet.is_object() {
        return Ok(vec!["critical-edition witness is not an object".into()]);
    }
    let Some(target_unit) = target_unit.filter(|value| value.is_object()) else {
        return Ok(vec![
            "critical-edition local structure has no source-review target".into(),
        ]);
    };
    let Some(source_review_plan) = source_review_plan.filter(|value| value.is_object()) else {
        return Ok(vec![
            "critical-edition local structure has no source-review plan".into(),
        ]);
    };
    let Some(ocr_sample_plan) = ocr_sample_plan.filter(|value| value.is_object()) else {
        return Ok(vec![
            "critical-edition local structure has no visual sample plan".into(),
        ]);
    };
    let context = packet
        .get("local_structural_context")
        .unwrap_or(&Value::Null);
    if !context.is_object() {
        return Ok(vec![
            "critical-edition local_structural_context is not an object".into(),
        ]);
    }
    let mut issues = Vec::new();
    if python_json_different(
        context.get("source_expression_ref"),
        source_review_plan.get("source_expression_ref"),
    )? {
        issues.push("critical-edition local structure drifted from the source expression".into());
    }
    let epub = context.get("epub_witness").unwrap_or(&Value::Null);
    let expected_epub = source_review_plan
        .get("source_witness")
        .unwrap_or(&Value::Null);
    if !epub.is_object() || !expected_epub.is_object() {
        issues.push("critical-edition local EPUB witness is malformed".into());
    } else {
        for field in ["item_ref", "file_ref", "file_sha256"] {
            if python_json_different(epub.get(field), expected_epub.get(field))? {
                issues.push(format!(
                    "critical-edition local EPUB {field} drifted from the source-review plan"
                ));
            }
        }
        let start = epub.get("section_start_member").unwrap_or(&Value::Null);
        if !start.is_object()
            || python_json_different(start.get("path"), target_unit.get("container_member"))?
            || python_json_different(start.get("sha256"), target_unit.get("member_sha256"))?
        {
            issues.push(
                "critical-edition local section start drifted from the target source unit".into(),
            );
        }
        let boundary = epub
            .get("next_section_boundary_member")
            .unwrap_or(&Value::Null);
        let start_index = start
            .get("path")
            .and_then(Value::as_str)
            .and_then(epub_page_member_index);
        let boundary_index = boundary
            .get("path")
            .and_then(Value::as_str)
            .and_then(epub_page_member_index);
        if start_index.is_none()
            || boundary_index.is_none()
            || boundary_index != start_index.and_then(|index| index.checked_add(1))
        {
            issues
                .push("critical-edition local section boundary is not the next EPUB member".into());
        }
    }

    let visual = context.get("visual_witness").unwrap_or(&Value::Null);
    let expected_visual = source_review_plan
        .get("visual_witness")
        .unwrap_or(&Value::Null);
    let visual_context = target_unit.get("visual_context").unwrap_or(&Value::Null);
    if !visual.is_object() || !expected_visual.is_object() || !visual_context.is_object() {
        issues.push("critical-edition local visual witness is malformed".into());
    } else {
        for field in ["item_ref", "file_ref", "file_sha256"] {
            if python_json_different(visual.get(field), expected_visual.get(field))? {
                issues.push(format!(
                    "critical-edition local visual {field} drifted from the source-review plan"
                ));
            }
        }
        if python_json_different(
            visual.get("section_start_pdf_page"),
            visual_context.get("current_pdf_page"),
        )? {
            issues.push(
                "critical-edition local section start page drifted from the target source unit"
                    .into(),
            );
        }
        if python_json_different(
            visual.get("next_section_pdf_page"),
            visual_context.get("next_pdf_page"),
        )? {
            issues.push(
                "critical-edition local next-section page drifted from the target source unit"
                    .into(),
            );
        }
        let section_start_page = visual.get("section_start_pdf_page");
        let anchor_ref = visual.get("section_start_anchor_ref");
        let mut matches = 0usize;
        for group in array(ocr_sample_plan, "source_groups") {
            if !group.is_object() {
                continue;
            }
            for unit in array(group, "samples") {
                if unit.is_object()
                    && python_json_optional_equal(unit.get("page"), section_start_page)?
                    && python_json_optional_equal(unit.get("anchor_ref"), anchor_ref)?
                {
                    matches += 1;
                }
            }
        }
        if matches != 1 {
            issues.push(
                "critical-edition local section start anchor is absent or ambiguous in the visual sample plan"
                    .into(),
            );
        }
    }
    Ok(issues)
}

pub fn visual_retrieval_plan_issues(
    payload: &Value,
    source_query_plan: Option<&Value>,
    source_sample_plan: Option<&Value>,
    visual_sample_plan: Option<&Value>,
) -> Result<Vec<String>, ItemRefusal> {
    if !payload.is_object() {
        return Ok(vec!["visual retrieval plan is not an object".into()]);
    }
    let Some(source_query_plan) = source_query_plan.filter(|value| value.is_object()) else {
        return Ok(vec![
            "visual retrieval plan has no source query plan".into(),
        ]);
    };
    let Some(source_sample_plan) = source_sample_plan.filter(|value| value.is_object()) else {
        return Ok(vec![
            "visual retrieval plan has no source sample plan".into(),
        ]);
    };
    let Some(visual_sample_plan) = visual_sample_plan.filter(|value| value.is_object()) else {
        return Ok(vec![
            "visual retrieval plan has no visual sample plan".into(),
        ]);
    };

    let mut issues = Vec::new();
    let mut source_anchor_to_sample = BTreeMap::<String, String>::new();
    for group in array(source_sample_plan, "source_groups") {
        for sample in array(group, "samples") {
            let anchor_ref = sample.get("anchor_ref").and_then(Value::as_str);
            let sample_id = sample.get("sample_id").and_then(Value::as_str);
            if let (Some(anchor_ref), Some(sample_id)) = (anchor_ref, sample_id) {
                if source_anchor_to_sample.contains_key(anchor_ref) {
                    issues.push(format!(
                        "source anchor has multiple sample identities: {anchor_ref}"
                    ));
                }
                source_anchor_to_sample.insert(anchor_ref.into(), sample_id.into());
            }
        }
    }
    let mut source_sample_to_visual_anchor = BTreeMap::<String, String>::new();
    let mut visual_anchors = BTreeSet::new();
    for group in array(visual_sample_plan, "source_groups") {
        for sample in array(group, "samples") {
            let source_sample_id = sample.get("source_sample_id").and_then(Value::as_str);
            let visual_anchor_ref = sample.get("anchor_ref").and_then(Value::as_str);
            if let Some(visual_anchor_ref) = visual_anchor_ref {
                visual_anchors.insert(visual_anchor_ref.to_owned());
            }
            if let (Some(source_sample_id), Some(visual_anchor_ref)) =
                (source_sample_id, visual_anchor_ref)
            {
                if source_sample_to_visual_anchor.contains_key(source_sample_id) {
                    issues.push(format!(
                        "source sample has multiple visual anchors: {source_sample_id}"
                    ));
                }
                source_sample_to_visual_anchor
                    .insert(source_sample_id.into(), visual_anchor_ref.into());
            }
        }
    }
    let mut query_ids = BTreeSet::new();
    let mut unresolved_query_ids = Vec::new();
    for query in array(source_query_plan, "queries") {
        let Some(query_id) = query.get("query_id").and_then(Value::as_str) else {
            continue;
        };
        query_ids.insert(query_id.to_owned());
        let query_unresolved = ["expected_source_anchor_refs", "hard_negative_anchor_refs"]
            .iter()
            .flat_map(|field| array(query, field))
            .filter_map(Value::as_str)
            .any(|anchor_ref| {
                source_anchor_to_sample
                    .get(anchor_ref)
                    .and_then(|sample_id| source_sample_to_visual_anchor.get(sample_id))
                    .is_none_or(|visual_anchor| !visual_anchors.contains(visual_anchor))
            });
        if query_unresolved {
            unresolved_query_ids.push(query_id.to_owned());
        }
    }
    let projection = payload.get("query_projection").unwrap_or(&Value::Null);
    if !projection.is_object() {
        issues.push("visual retrieval query_projection is not an object".into());
        return Ok(issues);
    }
    if python_json_different(
        projection.get("query_count"),
        Some(&Value::from(query_ids.len() as u64)),
    )? {
        issues.push("visual retrieval query count differs from source plan".into());
    }
    if python_json_different(
        projection.get("resolved_query_count"),
        Some(&Value::from(
            (query_ids.len() - unresolved_query_ids.len()) as u64,
        )),
    )? {
        issues.push("visual retrieval resolved query count drifted".into());
    }
    let expected_unresolved: Vec<Value> = unresolved_query_ids
        .iter()
        .map(|query_id| Value::String(query_id.clone()))
        .collect();
    if python_json_different(
        projection.get("unresolved_query_ids"),
        Some(&Value::Array(expected_unresolved)),
    )? {
        issues.push("visual retrieval unresolved query IDs drifted".into());
    }
    let crosswalk = projection
        .get("source_to_visual_anchor_crosswalk")
        .unwrap_or(&Value::Null);
    if !crosswalk.is_object() {
        issues.push("visual retrieval anchor crosswalk is not an object".into());
    } else {
        if python_json_different(
            crosswalk.get("source_anchor_count"),
            Some(&Value::from(source_anchor_to_sample.len() as u64)),
        )? {
            issues.push("visual retrieval source anchor count drifted".into());
        }
        if python_json_different(
            crosswalk.get("visual_anchor_count"),
            Some(&Value::from(visual_anchors.len() as u64)),
        )? {
            issues.push("visual retrieval visual anchor count drifted".into());
        }
        if crosswalk.get("one_to_one_for_frozen_queries")
            != Some(&Value::Bool(unresolved_query_ids.is_empty()))
        {
            issues.push("visual retrieval one-to-one query crosswalk drifted".into());
        }
    }
    Ok(issues)
}

fn epub_page_member_index(path: &str) -> Option<u64> {
    let digits = path.strip_prefix("EPUB/page_")?.strip_suffix(".html")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn page_member_index(path: &str) -> Option<u64> {
    let filename = path.rsplit('/').next()?;
    let digits = filename.strip_prefix("page_")?.strip_suffix(".html")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn check_digest_binding<S: LayerFamilySource>(
    checks: &mut GoldsetChecks<'_, S>,
    packet: &Value,
    field: &str,
    expected: Option<&JsonDocument>,
    expected_path: &str,
    location: &str,
) -> Result<(), ItemRefusal> {
    let binding = field
        .split('.')
        .try_fold(packet, |current, component| current.get(component));
    if !binding.is_some_and(Value::is_object) {
        checks.issue(location, format!("{field} is not a digest-bound reference"))?;
        return Ok(());
    }
    if binding.and_then(|v| v.get("ref")).and_then(Value::as_str) != Some(expected_path) {
        checks.issue(
            location,
            format!("{field} does not cite the current owner artifact"),
        )?;
    } else if binding
        .and_then(|v| v.get("sha256"))
        .and_then(Value::as_str)
        != expected.map(|d| d.sha256.as_str())
    {
        checks.issue(location, format!("{field} digest drifted"))?;
    }
    Ok(())
}

fn check_external_digest_binding<S: LayerFamilySource>(
    checks: &mut GoldsetChecks<'_, S>,
    packet: &Value,
    field: &str,
    expected_path: &str,
    location: &str,
) -> Result<(), ItemRefusal> {
    let binding = packet.get(field);
    if !binding.is_some_and(Value::is_object) {
        checks.issue(location, format!("{field} is not a digest-bound reference"))?;
        return Ok(());
    }
    if binding.and_then(|v| v.get("ref")).and_then(Value::as_str) != Some(expected_path) {
        checks.issue(
            location,
            format!("{field} does not cite the current owner artifact"),
        )?;
        return Ok(());
    }
    let Some(raw) = checks.current(expected_path, false)? else {
        checks.issue(
            location,
            format!("{field} referenced owner artifact is missing or linked"),
        )?;
        return Ok(());
    };
    let digest = Digest256::of_bytes(&raw).to_hex();
    checks.release_live(raw.len());
    if binding
        .and_then(|v| v.get("sha256"))
        .and_then(Value::as_str)
        != Some(digest.as_str())
    {
        checks.issue(location, format!("{field} digest drifted"))?;
    }
    Ok(())
}

fn assessment_refusal(error: crate::assessment::AssessmentRefusal) -> ItemRefusal {
    use crate::assessment::AssessmentRefusal;
    match error {
        AssessmentRefusal::Budget => ItemRefusal::Budget,
        AssessmentRefusal::Deadline => ItemRefusal::Deadline,
        AssessmentRefusal::Cancelled => ItemRefusal::Source("gold-set comparison cancelled".into()),
        AssessmentRefusal::Schema(error) => error,
        AssessmentRefusal::InvalidInput(message) | AssessmentRefusal::Unsupported(message) => {
            ItemRefusal::Unsupported(message)
        }
    }
}

fn python_json_equal(left: &Value, right: &Value) -> Result<bool, ItemRefusal> {
    crate::assessment::py_equal(left, right).map_err(assessment_refusal)
}

fn python_json_optional_equal(
    left: Option<&Value>,
    right: Option<&Value>,
) -> Result<bool, ItemRefusal> {
    python_json_equal(left.unwrap_or(&Value::Null), right.unwrap_or(&Value::Null))
}

fn python_json_different(left: Option<&Value>, right: Option<&Value>) -> Result<bool, ItemRefusal> {
    Ok(!python_json_optional_equal(left, right)?)
}

fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => {
            number.as_i64().is_some_and(|value| value != 0)
                || number.as_u64().is_some_and(|value| value != 0)
                || number.as_f64().is_some_and(|value| value != 0.0)
        }
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn python_int(value: &Value) -> Option<i128> {
    match value {
        Value::Bool(value) => Some(if *value { 1 } else { 0 }),
        Value::Number(number) => number
            .as_i64()
            .map(i128::from)
            .or_else(|| number.as_u64().map(i128::from))
            .or_else(|| {
                number
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .map(|value| value.trunc() as i128)
            }),
        Value::String(text) => {
            let text = text.trim();
            let digits = text
                .strip_prefix('+')
                .or_else(|| text.strip_prefix('-'))
                .unwrap_or(text);
            if digits.is_empty()
                || digits.starts_with('_')
                || digits.ends_with('_')
                || digits.contains("__")
                || digits
                    .chars()
                    .any(|character| character != '_' && !character.is_ascii_digit())
            {
                return None;
            }
            text.replace('_', "").parse().ok()
        }
        _ => None,
    }
}

/// Python iterates strings by Unicode scalar value and mappings by their keys.
/// This small projection is used only where the maintained validator builds a
/// set from an arbitrary iterable field; arrays remain the ordinary case.
fn python_iterable_string_values(value: &Value) -> BTreeSet<String> {
    match value {
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Value::String(value) => value
            .chars()
            .map(|character| character.to_string())
            .collect(),
        Value::Object(values) => values.keys().cloned().collect(),
        _ => BTreeSet::new(),
    }
}

fn array<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn python_string_list(values: &[String]) -> String {
    let rendered = values
        .iter()
        .map(|value| format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'")))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}

fn numeric(value: &Value) -> Option<f64> {
    if value.is_boolean() {
        None
    } else {
        value.as_f64()
    }
}

fn display(value: &Value) -> String {
    match value {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn json_error_kind(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Eof => "unexpected end of JSON input",
        serde_json::error::Category::Syntax => "invalid JSON syntax",
        serde_json::error::Category::Data => "invalid JSON value",
        serde_json::error::Category::Io => "JSON input error",
    }
}

fn item_language(
    records: &dyn SourceFoundationDefaultRecordsLookup,
    item_id: &Value,
) -> Result<Option<String>, ItemRefusal> {
    let Some(item_id) = item_id.as_str() else {
        return Ok(None);
    };
    let Some(item) = records.current_record(item_id)? else {
        return Ok(None);
    };
    if item.kind != "item" {
        return Ok(None);
    }
    let Some(edition_ref) = item.value.get("embodiment_ref").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(edition) = records.current_record(edition_ref)? else {
        return Ok(None);
    };
    if edition.kind != "edition" {
        return Ok(None);
    }
    let mut language: Option<String> = None;
    let mut multiple_languages = false;
    for expression_ref in array(&edition.value, "embodies_expression_refs") {
        let Some(expression_id) = expression_ref.as_str() else {
            continue;
        };
        if let Some(expression) = records.current_record(expression_id)?
            && expression.kind == "expression"
            && let Some(next_language) = expression.value.get("language").and_then(Value::as_str)
        {
            match language.as_deref() {
                Some(previous) if previous != next_language => multiple_languages = true,
                Some(_) => {}
                None => language = Some(next_language.to_owned()),
            }
        }
    }
    Ok((!multiple_languages).then_some(language).flatten())
}

fn encoded_len(value: &Value) -> Result<usize, ItemRefusal> {
    let mut counter = SizeCounter::default();
    serde_json::to_writer(&mut counter, value).map_err(|_| ItemRefusal::Budget)?;
    Ok(counter.0)
}
