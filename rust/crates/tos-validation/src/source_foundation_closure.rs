//! Bounded cross-stream closure for the maintained source-witness foundation.
//!
//! This module checks only the current source cut supplied by its caller. It
//! is a source-only closure district, not a family-complete report or a source
//! admission result. Current record identity comes from the independently
//! checked record district; current path membership comes from the captured
//! source cut. The caller binds the record district's revision and membership
//! to that cut before supplying its maps. Ordinary schema and byte reads stay
//! behind `LayerFamilySource`; addressed historical and Artifact replay checks
//! retain their own conditional owner routes.

use crate::biblio_rules::BiblioClaim;
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::layer_family_rules::LayerFamilySource;
use crate::record_biblio_cut::BiblioCurrentRecord;
use crate::source_witness_foundation::SourceFileMembershipIndex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::Ordering;
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, RelativePath, SourceRevision, canonical_bytes_v1,
};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

const SOURCE_HOME: &str = "ToS/source-witnesses/";
const CLAIM_SCHEMA: &str = "ToS/contracts/claim-packet.schema.json";
const PROVENANCE_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const ANCHOR_SCHEMA: &str = "ToS/contracts/source-anchor.schema.json";
const BOUNDARY_MAP_SCHEMA: &str = "ToS/contracts/collection-work-boundary-map.schema.json";
const DERIVATION_SCHEMA: &str = "ToS/contracts/expression-derivation.schema.json";
const CHRONOLOGY_SCHEMA: &str = "ToS/contracts/first-publication-chronology.schema.json";
const OBJECT_LINK_SCHEMA: &str = "ToS/contracts/object-link-claim.schema.json";
const PROVISION_SCHEMA: &str = "ToS/contracts/provision-activity.schema.json";
const TOPOLOGY_EVENT: &str =
    "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31";
const TOPOLOGY_PROVENANCE: &str = "ToS/source-witnesses/relations/provenance.jsonl";
const DERIVATION_CLAIMS: &str =
    "ToS/source-witnesses/relations/expression-derivation/expression-derivation-claims.jsonl";
const DERIVATION_PROVENANCE: &str =
    "ToS/source-witnesses/relations/expression-derivation/provenance.jsonl";
const DERIVATION_EVENT: &str =
    "tos.event.annotation.expression-derivation.antonovsky-revision-lineage.2026-08-01";
const CHRONOLOGY_CLAIMS: &str = "ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication/work-chronology-claims.jsonl";
const CHRONOLOGY_PROVENANCE: &str =
    "ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication/provenance.jsonl";
const CHRONOLOGY_EVENT: &str =
    "tos.event.annotation.friedrich-nietzsche.first-publication-chronology.2026-07-31";
const WORK_CHRONOLOGY_SCHEMA: &str = "ToS/contracts/first-publication-chronology.schema.json";
const PROVISION_EVENT_BASENAME: &str = "provision-activity-provenance.jsonl";

const TOPOLOGY_ROUTES: [(&str, &str, &str, &str, &str, &str); 3] = [
    (
        "ToS/source-witnesses/relations/work-expression/work-expression-claims.jsonl",
        "has_expression",
        "work",
        "expression",
        "expression_claim_refs",
        "unreviewed-work-expression-topology-claims",
    ),
    (
        "ToS/source-witnesses/relations/expression-edition/expression-edition-claims.jsonl",
        "embodied_by",
        "expression",
        "edition",
        "embodiment_claim_refs",
        "unreviewed-expression-edition-topology-claims",
    ),
    (
        "ToS/source-witnesses/relations/edition-item/edition-item-claims.jsonl",
        "exemplified_by",
        "edition",
        "item",
        "exemplar_claim_refs",
        "unreviewed-edition-item-topology-claims",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFoundationClosureCost {
    /// Bytes returned by the exact-cut current and retained read adapter.
    pub current_bytes_read: u64,
    pub recorded_bytes_read: u64,
    pub files_read: u64,
    pub schema_requests: u64,
    pub decoded_rows: u64,
    pub reserved_state_bytes: usize,
    pub emitted_issues: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationClosureGap {
    pub location: String,
    pub profile: String,
}

/// Ordered source-only findings. `requires_bibliographic` is a route hint for
/// the caller, based on source-declared profile kinds and exact current file
/// membership; it is not a validation verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureReport {
    pub issues: Vec<(String, String)>,
    /// Exact decoded documents awaiting the caller's diagnostic-v2 schema
    /// worker. Schema output is inserted immediately before `before_issue`.
    pub schema_requests: Vec<SourceFoundationClosureSchemaRequest>,
    pub unsupported: Vec<SourceFoundationClosureGap>,
    pub requires_bibliographic: bool,
    pub cost: SourceFoundationClosureCost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationClosureSchemaRequest {
    pub before_issue: usize,
    pub location: String,
    pub contract: String,
    pub document: Value,
}

#[derive(Debug, Clone)]
struct LoadedRows {
    digest: String,
    rows: Vec<(usize, Value)>,
}

#[derive(Debug, Clone)]
struct ClaimRef {
    location: String,
    subject: String,
    predicate: String,
    object: String,
    event: String,
    native: bool,
}

/// Check the cross-stream closure district over an exact current source cut.
/// The caller supplies the earlier-district event map, path list, and
/// source-declared profile kinds from the captured cut and the completed
/// record district. No repository walk, mutable checkout read, catalog read,
/// or source admission is performed here.
pub fn inspect_source_foundation_closure<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    cut: &CorpusCutReader,
    source_events: &BTreeMap<String, Value>,
    current_records: &BTreeMap<String, BiblioCurrentRecord>,
    item_editions: &BTreeMap<String, String>,
    current_paths: &[String],
    file_memberships: &SourceFileMembershipIndex,
    rights_ids: &BTreeSet<String>,
    declared_profile_kinds: &BTreeSet<String>,
    bibliographic_claims: &[BiblioClaim],
    limits: ItemLimits,
) -> Result<SourceFoundationClosureReport, ItemRefusal> {
    let mut rules = ClosureRules::new(
        source,
        cut,
        source_events,
        current_records,
        item_editions,
        current_paths,
        file_memberships,
        rights_ids,
        limits,
    )?;
    let requires_bibliographic =
        source_foundation_requires_bibliographic(current_paths, declared_profile_kinds);
    rules.check_records_map()?;
    rules.collect_events()?;
    rules.check_boundary_maps_and_anchors()?;
    rules.check_claim_streams(bibliographic_claims)?;
    rules.check_topology()?;
    rules.check_derivation()?;
    rules.check_responsibility_claims()?;
    rules.check_publication_claims()?;
    rules.check_provision_activity()?;
    rules.check_chronology()?;
    rules.check_object_links()?;
    rules.check_record_backlinks()?;

    Ok(SourceFoundationClosureReport {
        cost: rules.cost,
        issues: rules.issues,
        schema_requests: rules.schema_requests,
        unsupported: rules.unsupported,
        requires_bibliographic,
    })
}

/// Exact source-derived condition used by the maintained Python route for its
/// optional bibliographic graph boundary.
pub fn source_foundation_requires_bibliographic(
    current_paths: &[String],
    declared_profile_kinds: &BTreeSet<String>,
) -> bool {
    !declared_profile_kinds.is_empty()
        || current_paths.iter().any(|path| {
            path.starts_with("ToS/source-witnesses/")
                && (path.ends_with("/historical-claims.jsonl")
                    || path.ends_with("/source-claims.jsonl"))
        })
}

struct ClosureRules<'a, S: LayerFamilySource + ?Sized> {
    source: &'a mut S,
    cut: &'a CorpusCutReader,
    source_revision: SourceRevision,
    current_membership: SourceMembershipV1,
    limits: ItemLimits,
    paths: BTreeSet<String>,
    all_paths: BTreeSet<String>,
    records: &'a BTreeMap<String, BiblioCurrentRecord>,
    file_memberships: &'a SourceFileMembershipIndex,
    rights_ids: &'a BTreeSet<String>,
    source_events: &'a BTreeMap<String, Value>,
    links: BTreeMap<String, (String, Value)>,
    issues: Vec<(String, String)>,
    schema_requests: Vec<SourceFoundationClosureSchemaRequest>,
    unsupported: Vec<SourceFoundationClosureGap>,
    cost: SourceFoundationClosureCost,
    retained_state_bytes: usize,
    temporary_state_bytes: usize,
    loaded: BTreeMap<String, LoadedRows>,
    digests: BTreeMap<String, String>,
    recorded_checks: BTreeMap<(String, String), bool>,
    event_ids: BTreeSet<String>,
    events: BTreeMap<String, Value>,
    claim_ids: BTreeSet<String>,
    anchors: BTreeSet<String>,
    boundary_membership_refs: BTreeSet<String>,
    boundary_responsibility_refs: BTreeSet<String>,
    membership: BTreeMap<String, ClaimRef>,
    responsibility: BTreeMap<String, ClaimRef>,
    publication: BTreeMap<String, ClaimRef>,
    provision: BTreeMap<String, ClaimRef>,
    provision_values: BTreeMap<String, Value>,
    provision_event_ids: BTreeSet<String>,
    chronology: BTreeMap<String, ClaimRef>,
    object_links: BTreeMap<String, ClaimRef>,
    topology: BTreeMap<String, ClaimRef>,
    derivation: BTreeMap<String, ClaimRef>,
    item_edition_by_id: &'a BTreeMap<String, String>,
    file_digests: BTreeMap<String, String>,
}

impl<'a, S: LayerFamilySource + ?Sized> ClosureRules<'a, S> {
    fn new(
        source: &'a mut S,
        cut: &'a CorpusCutReader,
        source_events: &'a BTreeMap<String, Value>,
        records: &'a BTreeMap<String, BiblioCurrentRecord>,
        item_editions: &'a BTreeMap<String, String>,
        current_paths: &[String],
        file_memberships: &'a SourceFileMembershipIndex,
        rights_ids: &'a BTreeSet<String>,
        limits: ItemLimits,
    ) -> Result<Self, ItemRefusal> {
        check(limits.deadline, source.cancellation())?;
        let source_revision = cut.current().revision();
        let current_membership = cut
            .stream(source_revision)
            .map_err(|_| {
                ItemRefusal::Source("source-foundation exact cut membership unavailable".into())
            })?
            .expectation();
        let mut paths = BTreeSet::new();
        let mut all_paths = BTreeSet::new();
        let mut state_bytes = 0usize;
        for path in current_paths {
            if all_paths.contains(path) {
                return Err(ItemRefusal::Source(
                    "source-foundation closure received duplicate current path".into(),
                ));
            }
            let owned_path = path
                .len()
                .checked_add(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?;
            let copies = if path.starts_with(SOURCE_HOME) { 2 } else { 1 };
            state_bytes = state_bytes
                .checked_add(owned_path.checked_mul(copies).ok_or(ItemRefusal::Budget)?)
                .ok_or(ItemRefusal::Budget)?;
            if state_bytes > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure current path index",
                    used: Some(state_bytes as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            all_paths.insert(path.clone());
            if path.starts_with(SOURCE_HOME) {
                paths.insert(path.clone());
            }
        }
        for id in source_events.keys() {
            state_bytes = state_bytes
                .checked_add(
                    id.len()
                        .checked_add(std::mem::size_of::<String>())
                        .ok_or(ItemRefusal::Budget)?,
                )
                .ok_or(ItemRefusal::Budget)?;
            if state_bytes > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure event index",
                    used: Some(state_bytes as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
        }
        if all_paths.len() != cut.current().member_count()
            || cut
                .current()
                .members()
                .any(|member| !all_paths.contains(member.path.as_str()))
        {
            return Err(ItemRefusal::Source(
                "source-foundation current paths differ from the exact cut membership".into(),
            ));
        }
        Ok(Self {
            source,
            cut,
            source_revision,
            current_membership,
            limits,
            paths,
            all_paths,
            records,
            file_memberships,
            rights_ids,
            source_events,
            links: BTreeMap::new(),
            issues: Vec::new(),
            schema_requests: Vec::new(),
            unsupported: Vec::new(),
            cost: SourceFoundationClosureCost {
                reserved_state_bytes: state_bytes,
                ..SourceFoundationClosureCost::default()
            },
            retained_state_bytes: state_bytes,
            temporary_state_bytes: 0,
            loaded: BTreeMap::new(),
            digests: BTreeMap::new(),
            recorded_checks: BTreeMap::new(),
            event_ids: source_events.keys().cloned().collect(),
            events: BTreeMap::new(),
            claim_ids: BTreeSet::new(),
            anchors: BTreeSet::new(),
            boundary_membership_refs: BTreeSet::new(),
            boundary_responsibility_refs: BTreeSet::new(),
            membership: BTreeMap::new(),
            responsibility: BTreeMap::new(),
            publication: BTreeMap::new(),
            provision: BTreeMap::new(),
            provision_values: BTreeMap::new(),
            provision_event_ids: BTreeSet::new(),
            chronology: BTreeMap::new(),
            object_links: BTreeMap::new(),
            topology: BTreeMap::new(),
            derivation: BTreeMap::new(),
            item_edition_by_id: item_editions,
            file_digests: BTreeMap::new(),
        })
    }

    fn reserve(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        self.retained_state_bytes = self
            .retained_state_bytes
            .checked_add(amount)
            .filter(|used| {
                used.checked_add(self.temporary_state_bytes)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation closure state",
                used: None,
                limit: Some(self.limits.max_state_bytes as u64),
            })?;
        self.cost.reserved_state_bytes = self
            .cost
            .reserved_state_bytes
            .max(self.retained_state_bytes + self.temporary_state_bytes);
        Ok(())
    }

    fn reserve_temporary(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        self.temporary_state_bytes = self
            .temporary_state_bytes
            .checked_add(amount)
            .filter(|used| {
                self.retained_state_bytes
                    .checked_add(*used)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::BudgetCheck {
                check: "source-foundation closure temporary state",
                used: None,
                limit: Some(self.limits.max_state_bytes as u64),
            })?;
        self.cost.reserved_state_bytes = self
            .cost
            .reserved_state_bytes
            .max(self.retained_state_bytes + self.temporary_state_bytes);
        Ok(())
    }

    fn release_temporary_since(&mut self, baseline: usize) {
        self.temporary_state_bytes = baseline;
    }

    fn issue(
        &mut self,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if self.issues.len() >= self.limits.max_issues {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation closure issue count",
                used: Some(self.issues.len() as u64 + 1),
                limit: Some(self.limits.max_issues as u64),
            });
        }
        let location = location.into();
        let message = message.into();
        self.reserve(location.len() + message.len() + 2 * std::mem::size_of::<String>())?;
        self.issues.push((location, message));
        self.cost.emitted_issues = self.issues.len();
        Ok(())
    }

    fn python_equal(&self, left: &Value, right: &Value) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let equal = crate::assessment::py_equal(left, right).map_err(assessment_refusal)?;
        check(self.limits.deadline, self.source.cancellation())?;
        Ok(equal)
    }

    fn gap(
        &mut self,
        location: impl Into<String>,
        profile: impl Into<String>,
    ) -> Result<(), ItemRefusal> {
        let location = location.into();
        let profile = profile.into();
        self.reserve(
            location.len() + profile.len() + std::mem::size_of::<SourceFoundationClosureGap>(),
        )?;
        self.unsupported
            .push(SourceFoundationClosureGap { location, profile });
        Ok(())
    }

    fn current_raw(&mut self, path: &str) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if !self.all_paths.contains(path) {
            return Ok(None);
        }
        let raw = self
            .source
            .current(path, self.limits.max_member_bytes, self.limits.deadline)?;
        if let Some(bytes) = &raw {
            if bytes.len() > self.limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            self.cost.current_bytes_read = self
                .cost
                .current_bytes_read
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= self.limits.max_total_bytes)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure current metadata bytes",
                    used: None,
                    limit: Some(self.limits.max_total_bytes),
                })?;
            self.cost.files_read = self
                .cost
                .files_read
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            let digest = Digest256::of_bytes(bytes).to_hex();
            self.digests.insert(path.to_owned(), digest.clone());
            self.file_digests.insert(path.to_owned(), digest);
        }
        Ok(raw)
    }

    fn json_rows(
        &mut self,
        path: &str,
        schema: &str,
        required: bool,
    ) -> Result<Option<LoadedRows>, ItemRefusal> {
        if self.loaded.contains_key(path) {
            let clone_cost = self
                .loaded
                .get(path)
                .map(loaded_clone_cost)
                .transpose()?
                .unwrap_or_default();
            self.reserve(clone_cost)?;
            return Ok(self.loaded.get(path).cloned());
        }
        if !self.all_paths.contains(path) {
            if required {
                self.issue(path, "required source member is missing")?;
            }
            return Ok(None);
        }
        let Some(raw) = self.current_raw(path)? else {
            if required {
                self.issue(path, "required source member is missing")?;
            }
            return Ok(None);
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        let mut rows = Vec::new();
        let jsonl = path.ends_with(".jsonl");
        let state_cost = raw.len().checked_mul(6).ok_or(ItemRefusal::Budget)?;
        self.reserve(state_cost)?;
        if jsonl {
            let segments: Vec<&[u8]> = raw.split(|byte| *byte == b'\n').collect();
            for (zero_index, bytes) in segments.iter().enumerate() {
                check(self.limits.deadline, self.source.cancellation())?;
                if bytes.iter().all(u8::is_ascii_whitespace) {
                    if zero_index + 1 == segments.len() && raw.ends_with(b"\n") {
                        continue;
                    }
                    self.issue(
                        format!("{path}:{}", zero_index + 1),
                        "blank JSONL line is not allowed",
                    )?;
                    continue;
                }
                let line = zero_index + 1;
                match serde_json::from_slice::<Value>(bytes) {
                    Ok(value) => {
                        self.request_schema(&format!("{path}:{line}"), schema, &value)?;
                        self.reserve(std::mem::size_of::<Value>())?;
                        rows.push((line, value));
                    }
                    Err(error) => self.issue(
                        format!("{path}:{line}"),
                        format!("invalid JSON: {}", json_parse_reason(&error)),
                    )?,
                }
            }
        } else {
            match serde_json::from_slice::<Value>(&raw) {
                Ok(value) => {
                    self.request_schema(path, schema, &value)?;
                    rows.push((1, value));
                }
                Err(error) => {
                    self.issue(path, format!("invalid JSON: {}", json_parse_reason(&error)))?
                }
            }
        }
        self.cost.decoded_rows = self
            .cost
            .decoded_rows
            .checked_add(rows.len() as u64)
            .ok_or(ItemRefusal::Budget)?;
        let loaded = LoadedRows { digest, rows };
        self.loaded.insert(path.to_owned(), loaded.clone());
        Ok(Some(loaded))
    }

    fn unchecked_jsonl_rows(&mut self, path: &str) -> Result<Option<LoadedRows>, ItemRefusal> {
        if self.loaded.contains_key(path) {
            let clone_cost = self
                .loaded
                .get(path)
                .map(loaded_clone_cost)
                .transpose()?
                .unwrap_or_default();
            self.reserve(clone_cost)?;
            return Ok(self.loaded.get(path).cloned());
        }
        if !self.paths.contains(path) || !path.ends_with(".jsonl") {
            return Ok(None);
        }
        let Some(raw) = self.current_raw(path)? else {
            return Ok(None);
        };
        let digest = Digest256::of_bytes(&raw).to_hex();
        self.reserve(raw.len().checked_mul(6).ok_or(ItemRefusal::Budget)?)?;
        let mut rows = Vec::new();
        let segments: Vec<&[u8]> = raw.split(|byte| *byte == b'\n').collect();
        for (zero_index, bytes) in segments.iter().enumerate() {
            check(self.limits.deadline, self.source.cancellation())?;
            if bytes.iter().all(u8::is_ascii_whitespace) {
                if zero_index + 1 == segments.len() && raw.ends_with(b"\n") {
                    continue;
                }
                self.issue(
                    format!("{path}:{}", zero_index + 1),
                    "blank JSONL line is not allowed",
                )?;
                continue;
            }
            let line = zero_index + 1;
            match serde_json::from_slice::<Value>(bytes) {
                Ok(value) => {
                    self.reserve(std::mem::size_of::<Value>())?;
                    rows.push((line, value));
                }
                Err(error) => self.issue(
                    format!("{path}:{line}"),
                    format!("invalid JSON: {}", json_parse_reason(&error)),
                )?,
            }
        }
        self.cost.decoded_rows = self
            .cost
            .decoded_rows
            .checked_add(rows.len() as u64)
            .ok_or(ItemRefusal::Budget)?;
        let loaded = LoadedRows { digest, rows };
        self.loaded.insert(path.to_owned(), loaded.clone());
        Ok(Some(loaded))
    }

    fn expect_ref(
        &mut self,
        owner: &str,
        reference: Option<&str>,
        expected_kind: &str,
    ) -> Result<(), ItemRefusal> {
        // Maintained require_record ignores non-string references; the actual
        // schema request owns their type failure. All kinds, including Item
        // and Link, resolve through the same first-selected current map.
        let Some(reference) = reference else {
            return Ok(());
        };
        let records = self.records;
        match records.get(reference) {
            None => self.issue(
                owner,
                format!("unresolved {expected_kind} reference: {reference}"),
            )?,
            Some(record) if text(&record.value, "record_type") != Some(expected_kind) => {
                let kind = record.value.get("record_type").unwrap_or(&Value::Null);
                let length = crate::source_foundation_records::python_value_string_len(kind)?;
                self.reserve(length)?;
                let displayed = crate::source_foundation_records::python_value_string(kind);
                self.issue(
                    owner,
                    format!("{reference} resolves to {displayed}, expected {expected_kind}"),
                )?;
            }
            Some(_) => {}
        }
        Ok(())
    }

    fn digest_for(&mut self, path: &str) -> Result<Option<String>, ItemRefusal> {
        if let Some(value) = self.digests.get(path) {
            return Ok(Some(value.clone()));
        }
        if !self.all_paths.contains(path) {
            return Ok(None);
        }
        let Some(raw) = self.current_raw(path)? else {
            return Ok(None);
        };
        Ok(Some(Digest256::of_bytes(&raw).to_hex()))
    }

    fn request_schema(
        &mut self,
        location: &str,
        contract: &str,
        document: &Value,
    ) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let retained = serde_json::to_vec(document)
            .map_err(|_| ItemRefusal::Budget)?
            .len()
            .checked_mul(8)
            .and_then(|bytes| {
                bytes.checked_add(
                    location.len()
                        + contract.len()
                        + std::mem::size_of::<SourceFoundationClosureSchemaRequest>()
                        + 96,
                )
            })
            .ok_or(ItemRefusal::Budget)?;
        self.reserve(retained)?;
        self.schema_requests
            .push(SourceFoundationClosureSchemaRequest {
                before_issue: self.issues.len(),
                location: location.to_owned(),
                contract: contract.to_owned(),
                document: document.clone(),
            });
        self.cost.schema_requests = self
            .cost
            .schema_requests
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn recorded_matches(&mut self, path: &str, digest: &str) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        let key = (path.to_owned(), digest.to_owned());
        if let Some(matches) = self.recorded_checks.get(&key) {
            return Ok(*matches);
        }
        self.reserve(path.len() + digest.len() + 96)?;
        let raw = self.source.recorded(
            path,
            digest,
            self.limits.max_member_bytes,
            self.limits.deadline,
        )?;
        if let Some(bytes) = &raw {
            self.cost.recorded_bytes_read = self
                .cost
                .recorded_bytes_read
                .checked_add(bytes.len() as u64)
                .filter(|n| n <= &self.limits.max_total_bytes)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "source-foundation closure recorded metadata bytes",
                    used: None,
                    limit: Some(self.limits.max_total_bytes),
                })?;
            self.cost.files_read = self
                .cost
                .files_read
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        let matches = raw.is_some_and(|bytes| Digest256::of_bytes(&bytes).to_hex() == digest);
        self.recorded_checks.insert(key, matches);
        Ok(matches)
    }

    fn current_exists(&mut self, path: &str) -> Result<bool, ItemRefusal> {
        check(self.limits.deadline, self.source.cancellation())?;
        if !self.all_paths.contains(path) {
            return Ok(false);
        }
        self.source
            .exists(path, self.limits.max_member_bytes, self.limits.deadline)
    }

    fn claim_id(&mut self, location: &str, row: &Value) -> Result<Option<String>, ItemRefusal> {
        let Some(id) = row.get("claim_id").and_then(Value::as_str) else {
            return Ok(None);
        };
        self.reserve(id.len() + std::mem::size_of::<String>())?;
        if !self.claim_ids.insert(id.to_owned()) {
            self.issue(location, format!("duplicate claim_id: {id}"))?;
        }
        Ok(Some(id.to_owned()))
    }

    fn event(&self, id: &str) -> Option<&Value> {
        self.source_events.get(id).or_else(|| self.events.get(id))
    }

    fn check_records_map(&mut self) -> Result<(), ItemRefusal> {
        // Records owns schema, reference and duplicate findings. This boundary
        // only verifies caller map shape and builds the Link join operand;
        // repeating that owner's checks would change issue coverage/order.
        let records = self.records;
        for (id, record) in records {
            check(self.limits.deadline, self.source.cancellation())?;
            if record.value.get("record_id").and_then(Value::as_str) != Some(id.as_str())
                || !self.paths.contains(&record.path)
            {
                return Err(ItemRefusal::Source(
                    "source-foundation record map differs from its selected source input".into(),
                ));
            }
            if record.path.ends_with("/link.json") {
                let bytes = crate::record_biblio_cut::decoded_state(&record.value)
                    .map_err(|_| ItemRefusal::Budget)?;
                let clone_state = id
                    .len()
                    .checked_add(record.path.len())
                    .and_then(|n| n.checked_add(bytes))
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<(String, (String, Value))>() + 64)
                    })
                    .ok_or(ItemRefusal::Budget)?;
                self.reserve(clone_state)?;
                self.links
                    .insert(id.clone(), (record.path.clone(), record.value.clone()));
            }
        }
        Ok(())
    }

    fn collect_events(&mut self) -> Result<(), ItemRefusal> {
        let mut event_key_mismatch = false;
        for (id, event) in self.source_events {
            check(self.limits.deadline, self.source.cancellation())?;
            if text(event, "event_id") != Some(id.as_str()) {
                event_key_mismatch = true;
                break;
            }
        }
        if event_key_mismatch {
            self.issue(
                SOURCE_HOME,
                "earlier-district event map key differs from event_id",
            )?;
        }
        let mut event_paths = BTreeSet::from([
            TOPOLOGY_PROVENANCE.to_owned(),
            DERIVATION_PROVENANCE.to_owned(),
            CHRONOLOGY_PROVENANCE.to_owned(),
        ]);
        event_paths.extend(
            self.paths
                .iter()
                .filter(|path| path.ends_with(PROVISION_EVENT_BASENAME))
                .cloned(),
        );
        for path in event_paths {
            if !self.paths.contains(&path) {
                continue;
            }
            let Some(loaded) = self.json_rows(&path, PROVENANCE_SCHEMA, false)? else {
                continue;
            };
            for (line, event) in loaded.rows {
                check(self.limits.deadline, self.source.cancellation())?;
                self.validate_source_refs(&format!("{path}:{line}"), &event)?;
                let Some(id) = text(&event, "event_id").map(str::to_owned) else {
                    continue;
                };
                self.reserve(id.len() + std::mem::size_of::<String>())?;
                if !self.event_ids.insert(id.clone()) {
                    self.issue(
                        format!("{path}:{line}"),
                        format!("duplicate event_id: {id}"),
                    )?;
                } else if !self.source_events.contains_key(&id) {
                    self.events.insert(id.clone(), event);
                }
                if path.ends_with(PROVISION_EVENT_BASENAME) {
                    self.provision_event_ids.insert(id);
                }
            }
        }
        Ok(())
    }

    fn check_claim_streams(
        &mut self,
        bibliographic_claims: &[BiblioClaim],
    ) -> Result<(), ItemRefusal> {
        let mut supplied: BTreeMap<String, BTreeMap<usize, &BiblioClaim>> = BTreeMap::new();
        for claim in bibliographic_claims {
            check(self.limits.deadline, self.source.cancellation())?;
            if !claim.path.starts_with(SOURCE_HOME) || !claim.path.ends_with("-claims.jsonl") {
                self.issue(
                    &claim.path,
                    "bibliography report contains a non-source Claim route",
                )?;
                continue;
            }
            if !self.paths.contains(&claim.path) {
                self.issue(
                    &claim.path,
                    "bibliography Claim row is outside the captured source membership",
                )?;
                continue;
            }
            if claim.line == 0
                || supplied
                    .entry(claim.path.clone())
                    .or_default()
                    .insert(claim.line, claim)
                    .is_some()
            {
                self.issue(
                    &claim.path,
                    format!(
                        "bibliography report repeats or misnumbers Claim line {}",
                        claim.line
                    ),
                )?;
            }
        }
        let native_lines: BTreeSet<(String, usize)> = bibliographic_claims
            .iter()
            .filter(|claim| claim.native)
            .map(|claim| (claim.path.clone(), claim.line))
            .collect();

        let claim_paths: Vec<String> = self
            .paths
            .iter()
            .filter(|path| path.ends_with("-claims.jsonl"))
            .cloned()
            .collect();
        for path in claim_paths {
            let supplied_rows = supplied.remove(&path);
            let rows = if let Some(supplied_rows) = supplied_rows {
                let Some(current) = self.unchecked_jsonl_rows(&path)? else {
                    self.issue(
                        &path,
                        "bibliography Claim file is absent from the current cut",
                    )?;
                    continue;
                };
                let expected_lines: BTreeSet<usize> =
                    current.rows.iter().map(|(line, _)| *line).collect();
                let supplied_lines: BTreeSet<usize> = supplied_rows.keys().copied().collect();
                if expected_lines != supplied_lines {
                    self.issue(
                        &path,
                        "bibliography Claim rows do not cover the exact current file lines",
                    )?;
                }
                for claim in supplied_rows.values() {
                    if claim.raw_sha256 != current.digest {
                        self.issue(
                            &path,
                            "bibliography Claim digest differs from the exact current file",
                        )?;
                        break;
                    }
                    let current_row = current
                        .rows
                        .iter()
                        .find(|(line, _)| *line == claim.line)
                        .map(|(_, value)| value);
                    if !current_row
                        .map(|current| self.python_equal(current, &claim.value))
                        .transpose()?
                        .unwrap_or(false)
                    {
                        self.issue(
                            format!("{}:{}", path, claim.line),
                            "bibliography Claim value differs from the exact current line",
                        )?;
                    }
                }
                current.rows
            } else if path.ends_with("/source-claims.jsonl")
                || path.ends_with("/historical-claims.jsonl")
            {
                let Some(current) = self.unchecked_jsonl_rows(&path)? else {
                    self.issue(&path, "source Claim file is absent from the current cut")?;
                    continue;
                };
                if !current.rows.is_empty() {
                    self.gap(&path, "this profile requires the exact-cut biblio_rules Claim report for source-declared profile and native compound validation")?;
                    continue;
                }
                current.rows
            } else {
                let contract = if path.ends_with("/object-link-claims.jsonl") {
                    OBJECT_LINK_SCHEMA
                } else {
                    CLAIM_SCHEMA
                };
                let Some(current) = self.json_rows(&path, contract, true)? else {
                    continue;
                };
                current.rows
            };

            for (line, claim) in rows {
                self.register_claim(
                    &path,
                    line,
                    &claim,
                    native_lines.contains(&(path.clone(), line)),
                )?;
            }
        }
        for extra_path in supplied.keys() {
            self.issue(
                extra_path,
                "bibliography report contains a Claim stream absent from the captured path list",
            )?;
        }
        for reference in self.boundary_membership_refs.clone() {
            if !self.membership.contains_key(&reference) {
                self.issue(
                    SOURCE_HOME,
                    format!(
                        "work-boundary maps reference missing membership claims: [{reference}]"
                    ),
                )?;
            }
        }
        for reference in self.boundary_responsibility_refs.clone() {
            if !self.responsibility.contains_key(&reference) {
                self.issue(
                    SOURCE_HOME,
                    format!(
                        "work-boundary maps reference missing responsibility claims: [{reference}]"
                    ),
                )?;
            }
        }
        Ok(())
    }

    fn check_topology(&mut self) -> Result<(), ItemRefusal> {
        let Some(events) = self.json_rows(TOPOLOGY_PROVENANCE, PROVENANCE_SCHEMA, true)? else {
            return Ok(());
        };
        if events.rows.len() != 1 {
            self.issue(
                TOPOLOGY_PROVENANCE,
                "bibliographic topology must have exactly one batch provenance event",
            )?;
        }
        let event = events.rows.first().map(|(_, event)| event.clone());
        if let Some(event) = &event {
            if text(event, "event_id") != Some(TOPOLOGY_EVENT) {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance event_id differs from the owned route",
                )?;
            }
            if text(event, "event_type") != Some("annotation") {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance must be annotation",
                )?;
            }
            if value_strings(event, "agent_refs") != vec!["model:codex".to_owned()] {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance agent must be model:codex",
                )?;
            }
            let method = event.get("method").unwrap_or(&Value::Null);
            if text(method, "maker_type") != Some("model")
                || text(method, "name") != Some("declared-bibliographic-topology-materialization")
                || text(method, "version") != Some("1")
            {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance method identity drifted",
                )?;
            }
            if text(event, "status") != Some("completed_with_warnings") {
                self.issue(
                    TOPOLOGY_PROVENANCE,
                    "bibliographic topology provenance must retain completed-with-warnings posture",
                )?;
            }
        }

        let mut counts = BTreeMap::<String, u64>::new();
        for (path, predicate, subject_kind, object_kind, backref, role) in TOPOLOGY_ROUTES {
            let actual_paths: Vec<String> = self
                .paths
                .iter()
                .filter(|candidate| candidate.ends_with(path.rsplit('/').next().unwrap_or(path)))
                .cloned()
                .collect();
            if actual_paths.len() != 1 || actual_paths.first().map(String::as_str) != Some(path) {
                self.issue(path, "bibliographic topology claim basename must exist only at its owned relation route")?;
            }
            let Some(claim_file) = self.json_rows(path, CLAIM_SCHEMA, true)? else {
                continue;
            };
            if let Some(event) = &event {
                if !output_binds(event, path, role, &claim_file.digest) {
                    self.issue(path, "bibliographic topology provenance event does not digest-bind the claim file")?;
                }
            }
            for (line, claim) in &claim_file.rows {
                let location = format!("{path}:{line}");
                *counts.entry((*predicate).to_owned()).or_default() += 1;
                let claim_id = text(claim, "claim_id").unwrap_or_default();
                let subject_ref = text(claim, "subject_ref");
                let object_ref = text(claim, "object");
                if text(claim, "claim_type") != Some("bibliographic") {
                    self.issue(
                        &location,
                        "bibliographic topology claim_type must be bibliographic",
                    )?;
                }
                if text(claim, "assertion_layer") != Some("bibliographic_assertion") {
                    self.issue(
                        &location,
                        "bibliographic topology assertion_layer must be bibliographic_assertion",
                    )?;
                }
                if text(claim, "predicate") != Some(predicate) {
                    self.issue(
                        &location,
                        format!(
                            "{} predicate must be {predicate}",
                            path.rsplit('/').next().unwrap_or(path)
                        ),
                    )?;
                }
                self.expect_ref(&location, subject_ref, subject_kind)?;
                self.expect_ref(&location, object_ref, object_kind)?;
                if !self.python_equal(
                    claim.get("maker").unwrap_or(&Value::Null),
                    &serde_json::json!({"maker_type":"model","agent_ref":"model:codex"}),
                )? {
                    self.issue(
                        &location,
                        "bibliographic topology maker must be model:codex",
                    )?;
                }
                if text(claim, "provenance_event_ref") != Some(TOPOLOGY_EVENT) {
                    self.issue(
                        &location,
                        "bibliographic topology claim cites the wrong provenance event",
                    )?;
                }
                if text(claim, "epistemic_status") != Some("observed") {
                    self.issue(
                        &location,
                        "declared topology materialization must remain observed",
                    )?;
                }
                if text(claim, "review_status") != Some("unreviewed")
                    || !claim
                        .get("reviews")
                        .and_then(Value::as_array)
                        .is_some_and(Vec::is_empty)
                {
                    self.issue(
                        &location,
                        "bibliographic topology claims must remain unreviewed",
                    )?;
                }
                if text(claim, "visibility") != Some("public_metadata_only") {
                    self.issue(
                        &location,
                        "bibliographic topology claims must remain public metadata only",
                    )?;
                }

                // The maintained evidence lookup stringifies even malformed
                // endpoints; require_record above separately ignores their
                // non-string type. Preserve that negative-row distinction.
                let subject_value = claim.get("subject_ref").unwrap_or(&Value::Null);
                let object_value = claim.get("object").unwrap_or(&Value::Null);
                let endpoint_state =
                    crate::source_foundation_records::python_value_string_len(subject_value)?
                        .checked_add(crate::source_foundation_records::python_value_string_len(
                            object_value,
                        )?)
                        .ok_or(ItemRefusal::Budget)?;
                self.reserve(endpoint_state)?;
                let subject_key =
                    crate::source_foundation_records::python_value_string(subject_value);
                let object_key =
                    crate::source_foundation_records::python_value_string(object_value);
                let mut expected_evidence = BTreeSet::new();
                for endpoint in [&subject_key, &object_key] {
                    if let Some(record) = self.records.get(endpoint) {
                        expected_evidence.insert(record.path.clone());
                    }
                }
                if object_kind == "item" {
                    if let Some(manifest_ref) = self
                        .records
                        .get(&object_key)
                        .and_then(|record| text(&record.value, "item_manifest_ref"))
                    {
                        expected_evidence.insert(manifest_ref.to_owned());
                    }
                    if let (Some(item_id), Some(edition_id)) = (object_ref, subject_ref) {
                        if self.item_edition_by_id.get(item_id).map(String::as_str)
                            != Some(edition_id)
                        {
                            self.issue(&location, "edition-item topology differs from the current item manifest embodiment")?;
                        }
                    }
                }
                let actual_evidence: BTreeSet<String> =
                    value_strings(claim, "evidence_refs").into_iter().collect();
                if actual_evidence != expected_evidence {
                    self.issue(&location, "bibliographic topology evidence must be the exact linked records and item manifest")?;
                }
                if let Some(event) = &event {
                    for evidence_ref in expected_evidence {
                        let matches: Vec<&Value> = event
                            .get("inputs")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter(|input| text(input, "ref") == Some(evidence_ref.as_str()))
                            .collect();
                        let evidence_exists = self.current_exists(&evidence_ref)?;
                        let recorded = if matches.len() == 1 {
                            if let Some(digest) =
                                matches.first().and_then(|input| text(input, "sha256"))
                            {
                                self.recorded_matches(&evidence_ref, digest)?
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if matches.len() != 1 || !evidence_exists {
                            self.issue(&location, format!("bibliographic topology provenance does not digest-bind evidence input: {evidence_ref}"))?;
                        } else if !recorded {
                            self.issue(&location, format!("bibliographic topology provenance evidence bytes are unresolved: {evidence_ref}"))?;
                        }
                    }
                }
                if !claim_id.is_empty() {
                    if let Some(reference) = self.topology.get(claim_id) {
                        if reference.subject != subject_ref.unwrap_or_default()
                            || reference.predicate != predicate
                            || reference.object != object_ref.unwrap_or_default()
                        {
                            self.issue(
                                &location,
                                "bibliographic topology claim differs from its source Claim row",
                            )?;
                        }
                    }
                }
            }
            self.check_topology_backrefs(subject_kind, backref, predicate)?;
        }

        if let Some(event) = &event {
            let expected_configuration = serde_json::json!({
                "work_expression_claims_materialized": counts.get("has_expression").copied().unwrap_or_default(),
                "expression_edition_claims_materialized": counts.get("embodied_by").copied().unwrap_or_default(),
                "edition_item_claims_materialized": counts.get("exemplified_by").copied().unwrap_or_default(),
                "topology_claims_reviewed": 0,
                "source_text_admitted": false,
                "human_review_performed": false,
                "textual_equivalence_claims_created": 0,
                "semantic_claims_created": 0,
                "canon_promotion_performed": false,
            });
            if !self.python_equal(
                event
                    .get("method")
                    .and_then(|method| method.get("configuration"))
                    .unwrap_or(&Value::Null),
                &expected_configuration,
            )? {
                self.issue(TOPOLOGY_PROVENANCE, "bibliographic topology provenance configuration differs from exact legacy batch counts and authority limits")?;
            }
        }
        Ok(())
    }

    fn check_topology_backrefs(
        &mut self,
        subject_kind: &str,
        field: &str,
        predicate: &str,
    ) -> Result<(), ItemRefusal> {
        let ids_by_subject: BTreeMap<String, BTreeSet<String>> = self
            .topology
            .iter()
            .filter(|(_, claim)| claim.predicate == predicate)
            .fold(BTreeMap::new(), |mut map, (id, claim)| {
                map.entry(claim.subject.clone())
                    .or_default()
                    .insert(id.clone());
                map
            });
        let records: Vec<_> = self
            .records
            .iter()
            .filter(|(_, record)| record.kind == subject_kind)
            .map(|(id, record)| {
                (
                    id.clone(),
                    record.path.clone(),
                    value_strings(&record.value, field),
                )
            })
            .collect();
        for (id, path, actual) in records {
            let expected = ids_by_subject.get(&id).cloned().unwrap_or_default();
            let actual = actual.into_iter().collect::<BTreeSet<_>>();
            if actual != expected {
                self.issue(
                    path,
                    format!("{field} does not close over the exact outgoing {predicate} claims"),
                )?;
            }
        }
        Ok(())
    }

    fn check_derivation(&mut self) -> Result<(), ItemRefusal> {
        let claim_path = DERIVATION_CLAIMS;
        let claim_paths: Vec<String> = self
            .paths
            .iter()
            .filter(|path| path.ends_with("/expression-derivation-claims.jsonl"))
            .cloned()
            .collect();
        if claim_paths.len() != 1 || claim_paths.first().map(String::as_str) != Some(claim_path) {
            self.issue(
                claim_path,
                "Expression-derivation claim basename must exist only at its owned route",
            )?;
        }
        let Some(loaded) = self.json_rows(claim_path, CLAIM_SCHEMA, true)? else {
            return Ok(());
        };
        let claim_digest = loaded.digest.clone();
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut pairs = BTreeSet::new();
        let mut subjects = BTreeMap::new();
        let mut evidence_paths = BTreeSet::new();
        let mut revision_count = 0u64;
        let mut collated_count = 0u64;
        let mut reviewed_count = 0u64;
        let mut endpoint_refs = BTreeSet::new();

        for (line, claim) in &loaded.rows {
            let location = format!("{claim_path}:{line}");
            let Some(claim_id) = text(claim, "claim_id").map(str::to_owned) else {
                self.issue(&location, "Expression derivation claim_id is missing")?;
                continue;
            };
            if self.claim_ids.contains(&claim_id) && !self.derivation.contains_key(&claim_id) {
                self.issue(&location, format!("duplicate claim_id: {claim_id}"))?;
            }
            let subject_ref = text(claim, "subject_ref").unwrap_or_default().to_owned();
            let object_ref = text(claim, "object").unwrap_or_default().to_owned();
            if text(claim, "claim_type") != Some("relation") {
                self.issue(
                    &location,
                    "Expression derivation claim_type must be relation",
                )?;
            }
            if text(claim, "assertion_layer") != Some("bibliographic_assertion") {
                self.issue(&location, "Expression derivation must remain bibliographic")?;
            }
            if text(claim, "predicate") != Some("is_derivative_of") {
                self.issue(&location, "Expression derivation predicate drifted")?;
            }
            self.expect_ref(&location, Some(&subject_ref), "expression")?;
            self.expect_ref(&location, Some(&object_ref), "expression")?;
            if subject_ref == object_ref {
                self.issue(&location, "Expression derivation is irreflexive")?;
            }
            if let (Some(subject), Some(object)) = (
                self.records.get(&subject_ref),
                self.records.get(&object_ref),
            ) {
                if text(&subject.value, "work_ref") != text(&object.value, "work_ref") {
                    self.issue(
                        &location,
                        "v1 Expression derivation endpoints must realize the same Work",
                    )?;
                }
            }
            if !pairs.insert((subject_ref.clone(), object_ref.clone())) {
                self.issue(&location, "duplicate Expression-derivation endpoint pair")?;
            }
            edges
                .entry(subject_ref.clone())
                .or_default()
                .insert(object_ref.clone());
            endpoint_refs.insert(subject_ref.clone());
            endpoint_refs.insert(object_ref.clone());
            subjects.insert(claim_id.clone(), subject_ref.clone());

            if !self.python_equal(
                claim.get("maker").unwrap_or(&Value::Null),
                &serde_json::json!({"maker_type":"model","agent_ref":"model:codex"}),
            )? {
                self.issue(&location, "Expression derivation maker must be model:codex")?;
            }
            if text(claim, "provenance_event_ref") != Some(DERIVATION_EVENT) {
                self.issue(
                    &location,
                    "Expression derivation cites the wrong provenance event",
                )?;
            }
            if text(claim, "epistemic_status") != Some("reported") {
                self.issue(
                    &location,
                    "current Expression derivation claims must remain reported",
                )?;
            }
            if text(claim, "review_status") != Some("unreviewed")
                || !claim
                    .get("reviews")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
            {
                self.issue(
                    &location,
                    "Expression derivation claims must remain unreviewed",
                )?;
            }
            if text(claim, "visibility") != Some("public_metadata_only") {
                self.issue(
                    &location,
                    "Expression derivation must remain public metadata only",
                )?;
            }
            let qualifiers = claim.get("qualifiers").unwrap_or(&Value::Null);
            self.request_schema(
                &format!("{location}['qualifiers']"),
                DERIVATION_SCHEMA,
                qualifiers,
            )?;
            if text(qualifiers, "derivation_kind") == Some("revision") {
                revision_count += 1;
            }
            if text(qualifiers, "collation_status") != Some("not_collated") {
                collated_count += 1;
            }
            if text(claim, "review_status") != Some("unreviewed") {
                reviewed_count += 1;
            }
            let evidence = value_strings(claim, "evidence_refs");
            if !evidence
                .iter()
                .any(|reference| reference.starts_with("tos.anchor."))
            {
                self.issue(
                    &location,
                    "Expression derivation lacks exact source-anchor return",
                )?;
            }
            for evidence_ref in evidence {
                if evidence_ref.starts_with("tos.anchor.") {
                    if !self.anchors.contains(&evidence_ref) {
                        self.issue(
                            &location,
                            format!("unresolved derivation anchor: {evidence_ref}"),
                        )?;
                    }
                } else if evidence_ref.starts_with("ToS/") {
                    if !self.current_exists(&evidence_ref)? {
                        self.issue(
                            &location,
                            format!("unresolved derivation evidence: {evidence_ref}"),
                        )?;
                    }
                    evidence_paths.insert(evidence_ref);
                }
            }
        }

        let mut visited = BTreeMap::<String, u8>::new();
        let mut cycle = false;
        for start in edges.keys() {
            if visited.get(start).copied().unwrap_or_default() != 0 {
                continue;
            }
            let mut stack = vec![(start.clone(), false)];
            while let Some((node, leaving)) = stack.pop() {
                check(self.limits.deadline, self.source.cancellation())?;
                if leaving {
                    visited.insert(node, 2);
                    continue;
                }
                match visited.get(&node).copied().unwrap_or_default() {
                    1 => {
                        cycle = true;
                        continue;
                    }
                    2 => continue,
                    _ => {}
                }
                visited.insert(node.clone(), 1);
                stack.push((node.clone(), true));
                if let Some(children) = edges.get(&node) {
                    for child in children.iter().rev() {
                        match visited.get(child).copied().unwrap_or_default() {
                            1 => cycle = true,
                            0 => stack.push((child.clone(), false)),
                            _ => {}
                        }
                    }
                }
            }
        }
        if cycle {
            self.issue(claim_path, "Expression derivation cycle detected")?;
        }

        let clone_cost = self.records.iter().try_fold(0usize, |used, (id, record)| {
            used.checked_add(id.len())
                .and_then(|bytes| {
                    bytes.checked_add(std::mem::size_of::<(String, BiblioCurrentRecord)>())
                })
                .and_then(|bytes| bytes.checked_add(record.path.len() + record.kind.len()))
                .and_then(|bytes| {
                    crate::record_biblio_cut::decoded_state(&record.value)
                        .ok()
                        .and_then(|size| bytes.checked_add(size))
                })
                .ok_or(ItemRefusal::Budget)
        })?;
        self.reserve(clone_cost)?;
        let current_records: Vec<(String, BiblioCurrentRecord)> = self
            .records
            .iter()
            .map(|(id, record)| (id.clone(), record.clone()))
            .collect();
        for (record_id, record) in current_records {
            if record.kind != "expression" {
                continue;
            }
            let expected: BTreeSet<String> = subjects
                .iter()
                .filter(|(_, subject)| subject.as_str() == record_id.as_str())
                .map(|(claim_id, _)| claim_id.clone())
                .collect();
            let actual = value_strings(&record.value, "derivation_claim_refs")
                .into_iter()
                .collect::<BTreeSet<_>>();
            if expected != actual {
                self.issue(
                    &record.path,
                    "derivation_claim_refs do not close over exact outgoing derivation claims",
                )?;
            }
        }

        let event_rows = self.json_rows(DERIVATION_PROVENANCE, PROVENANCE_SCHEMA, true)?;
        if let Some(events) = event_rows {
            if events.rows.len() != 1 {
                self.issue(
                    DERIVATION_PROVENANCE,
                    "Expression derivation must have exactly one batch provenance event",
                )?;
            }
            if let Some((_, event)) = events.rows.first() {
                if text(event, "event_id") != Some(DERIVATION_EVENT)
                    || text(event, "event_type") != Some("annotation")
                    || value_strings(event, "agent_refs") != vec!["model:codex".to_owned()]
                    || text(event, "status") != Some("completed_with_warnings")
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance posture drifted",
                    )?;
                }
                let method = event.get("method").unwrap_or(&Value::Null);
                if text(method, "maker_type") != Some("model")
                    || text(method, "name")
                        != Some("source-reported-expression-derivation-materialization")
                    || text(method, "version") != Some("1")
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance method identity drifted",
                    )?;
                }
                let outputs = event
                    .get("outputs")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if outputs.len() != 1
                    || !output_binds(
                        event,
                        claim_path,
                        "unreviewed-source-reported-expression-derivation-claims",
                        &claim_digest,
                    )
                {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance output digest drifted",
                    )?;
                }

                let mut expected_inputs = evidence_paths;
                for endpoint in &endpoint_refs {
                    if let Some(record) = self.records.get(endpoint) {
                        expected_inputs.insert(record.path.clone());
                    }
                }
                expected_inputs.insert(CLAIM_SCHEMA.to_owned());
                expected_inputs.insert(DERIVATION_SCHEMA.to_owned());
                let actual_inputs: BTreeSet<String> = event
                    .get("inputs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| text(entry, "ref").map(str::to_owned))
                    .collect();
                let input_count = event
                    .get("inputs")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                if actual_inputs != expected_inputs || input_count != expected_inputs.len() {
                    self.issue(DERIVATION_PROVENANCE, "Expression-derivation provenance inputs differ from exact evidence and endpoints")?;
                }
                for input in event
                    .get("inputs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(reference) = text(input, "ref") else {
                        continue;
                    };
                    let digest = text(input, "sha256").unwrap_or_default();
                    if !self.current_exists(reference)?
                        || !self.recorded_matches(reference, digest)?
                    {
                        self.issue(
                            DERIVATION_PROVENANCE,
                            format!(
                                "Expression-derivation provenance input digest drifted: {reference}"
                            ),
                        )?;
                    }
                }
                let expected_configuration = serde_json::json!({
                    "expression_identities_materialized": endpoint_refs.len(),
                    "derivation_claims_materialized": subjects.len(),
                    "revision_claims_materialized": revision_count,
                    "claims_collated": collated_count,
                    "claims_reviewed": reviewed_count,
                    "unsupported_1911_to_1907_edge_created": false,
                    "unsupported_2007_to_1911_edge_created": false,
                    "source_text_admitted": false,
                    "human_review_performed": false,
                    "equivalence_claims_created": 0,
                    "semantic_claims_created": 0,
                    "canon_promotion_performed": false,
                });
                if !self.python_equal(
                    event
                        .get("method")
                        .and_then(|method| method.get("configuration"))
                        .unwrap_or(&Value::Null),
                    &expected_configuration,
                )? {
                    self.issue(
                        DERIVATION_PROVENANCE,
                        "Expression-derivation provenance configuration drifted",
                    )?;
                }
            }
        }
        Ok(())
    }

    fn check_responsibility_claims(&mut self) -> Result<(), ItemRefusal> {
        let claims: Vec<ClaimRef> = self.responsibility.values().cloned().collect();
        let mut validated_events = BTreeSet::new();
        for claim in claims {
            check(self.limits.deadline, self.source.cancellation())?;
            let predicate_allowed = matches!(
                claim.predicate.as_str(),
                "authored_by"
                    | "contributed_by"
                    | "translated_by"
                    | "edited_by"
                    | "afterword_by"
                    | "designed_by"
            );
            if !predicate_allowed {
                self.issue(
                    &claim.location,
                    format!("unsupported responsibility predicate: {}", claim.predicate),
                )?;
            }
            let expected_kind = match claim.predicate.as_str() {
                "authored_by" | "contributed_by" => Some("work"),
                "translated_by" => Some("expression"),
                "edited_by" | "afterword_by" | "designed_by" => Some("edition"),
                _ => None,
            };
            if let Some(expected_kind) = expected_kind {
                self.expect_ref(&claim.location, Some(&claim.subject), expected_kind)?;
            }
            self.expect_ref(&claim.location, Some(&claim.object), "agent")?;
            if claim.native {
                continue;
            }
            let Some((claim_path, line_text)) = claim.location.rsplit_once(':') else {
                continue;
            };
            let line = line_text.parse::<usize>().unwrap_or_default();
            let value = self.loaded.get(claim_path).and_then(|rows| {
                rows.rows
                    .iter()
                    .find(|(candidate, _)| *candidate == line)
                    .map(|(_, value)| value.clone())
            });
            let Some(value) = value else {
                continue;
            };
            if text(&value, "claim_type") != Some("bibliographic") {
                self.issue(
                    &claim.location,
                    "responsibility claim claim_type must be bibliographic",
                )?;
            }
            if !matches!(
                text(&value, "assertion_layer"),
                Some("bibliographic_assertion" | "scholarly_report")
            ) {
                self.issue(&claim.location, "responsibility claim assertion_layer must be bibliographic_assertion or scholarly_report")?;
            }
            let Some(event) = self.event(&claim.event).cloned() else {
                self.issue(
                    &claim.location,
                    format!("unresolved provenance_event_ref: {}", claim.event),
                )?;
                continue;
            };
            let Some(digest) = self.digest_for(claim_path)? else {
                self.issue(
                    &claim.location,
                    "responsibility Claim file is absent from the current cut",
                )?;
                continue;
            };
            let output_role = event
                .get("outputs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|output| {
                    text(output, "ref") == Some(claim_path)
                        && text(output, "sha256") == Some(digest.as_str())
                        && matches!(
                            text(output, "role"),
                            Some(
                                "unreviewed-translation-responsibility-claims"
                                    | "unreviewed-evidence-bearing-responsibility-claims"
                            )
                        )
                });
            if !output_role {
                self.issue(
                    &claim.location,
                    "responsibility claim provenance event does not digest-bind the claim file",
                )?;
            }
            if validated_events.insert(claim.event.clone()) {
                self.check_event_input_bindings(
                    &claim.location,
                    &event,
                    "responsibility claim provenance input",
                )?;
            }
        }
        Ok(())
    }

    fn check_publication_claims(&mut self) -> Result<(), ItemRefusal> {
        let claims: Vec<ClaimRef> = self.publication.values().cloned().collect();
        let mut validated_events = BTreeSet::new();
        for claim in claims {
            check(self.limits.deadline, self.source.cancellation())?;
            if !self.records.contains_key(&claim.subject) {
                continue;
            }
            let owner_path = claim
                .location
                .rsplit_once(':')
                .map(|(path, _)| path)
                .unwrap_or(&claim.location);
            let owner_path = owner_path
                .rsplit_once('/')
                .map(|(parent, _)| format!("{parent}/edition.json"));
            let owner_id = owner_path.as_ref().and_then(|path| {
                self.records
                    .values()
                    .find(|candidate| candidate.path == *path)
                    .and_then(|candidate| text(&candidate.value, "record_id"))
            });
            if claim.native {
                continue;
            }
            let Some((claim_path, _)) = claim.location.rsplit_once(':') else {
                continue;
            };
            let (_, claim_line) = claim.location.rsplit_once(':').unwrap_or((claim_path, ""));
            let location = format!("{claim_path}:{claim_line}");
            let Some(rows) = self.loaded.get(claim_path) else {
                continue;
            };
            let line = claim_line.parse::<usize>().unwrap_or_default();
            let Some(value) = rows
                .rows
                .iter()
                .find(|(candidate, _)| *candidate == line)
                .map(|(_, value)| value.clone())
            else {
                continue;
            };
            if text(&value, "claim_type") != Some("bibliographic") {
                self.issue(
                    &location,
                    "publication claim claim_type must be bibliographic",
                )?;
            }
            if !matches!(
                text(&value, "assertion_layer"),
                Some("bibliographic_assertion" | "scholarly_report")
            ) {
                self.issue(&location, "publication claim assertion_layer must be bibliographic_assertion or scholarly_report")?;
            }
            if owner_id != Some(claim.subject.as_str()) {
                self.issue(
                    &location,
                    "publication claim subject_ref differs from sibling edition.json",
                )?;
            }
            if claim.object.starts_with("tos.")
                && !self.records.contains_key(&claim.object)
                && !self.links.contains_key(&claim.object)
                && !self.event_ids.contains(&claim.object)
                && !self.rights_ids.contains(&claim.object)
            {
                self.issue(
                    &location,
                    format!("unresolved publication claim object: {}", claim.object),
                )?;
            }
            let Some(event) = self.event(&claim.event).cloned() else {
                self.issue(
                    &location,
                    format!("unresolved provenance_event_ref: {}", claim.event),
                )?;
                continue;
            };
            let Some(digest) = self.digest_for(claim_path)? else {
                self.issue(
                    &location,
                    "publication Claim file is absent from the current cut",
                )?;
                continue;
            };
            if !output_binds(
                &event,
                claim_path,
                "unreviewed-evidence-bearing-publication-claims",
                &digest,
            ) {
                self.issue(
                    &location,
                    "publication claim provenance event does not digest-bind the claim file",
                )?;
            }
            if validated_events.insert(claim.event.clone()) {
                self.check_event_input_bindings(
                    &location,
                    &event,
                    "publication claim provenance input",
                )?;
            }
        }
        Ok(())
    }

    fn check_provision_activity(&mut self) -> Result<(), ItemRefusal> {
        let claims: Vec<(String, ClaimRef, Value)> = self
            .provision
            .iter()
            .filter_map(|(id, reference)| {
                self.provision_values
                    .get(id)
                    .map(|value| (id.clone(), reference.clone(), value.clone()))
            })
            .collect();
        let mut validated_events = BTreeSet::new();
        let mut used_events = BTreeSet::new();
        for (_id, reference, claim) in claims {
            check(self.limits.deadline, self.source.cancellation())?;
            if reference.native {
                continue;
            }
            let location = reference.location.clone();
            if text(&claim, "claim_type") != Some("bibliographic") {
                self.issue(
                    &location,
                    "provision-activity claim_type must be bibliographic",
                )?;
            }
            if text(&claim, "assertion_layer") != Some("bibliographic_assertion") {
                self.issue(
                    &location,
                    "provision-activity assertion_layer must be bibliographic_assertion",
                )?;
            }
            if text(&claim, "predicate") != Some("provision_activity") {
                self.issue(
                    &location,
                    "provision-activity predicate must be provision_activity",
                )?;
            }
            let owner_path = reference
                .location
                .rsplit_once(':')
                .map(|(path, _)| path)
                .unwrap_or(&reference.location);
            let owner_path = owner_path
                .rsplit_once('/')
                .map(|(parent, _)| format!("{parent}/edition.json"));
            let owner_id = owner_path.as_ref().and_then(|path| {
                self.records
                    .values()
                    .find(|candidate| candidate.path == *path)
                    .and_then(|candidate| text(&candidate.value, "record_id"))
            });
            if owner_id != Some(reference.subject.as_str()) {
                self.issue(
                    &location,
                    "provision-activity subject_ref differs from sibling edition.json",
                )?;
            }

            let Some(activity) = claim.get("object") else {
                self.issue(&location, "provision-activity object must be an object")?;
                continue;
            };
            self.request_schema(&format!("{location}#object"), PROVISION_SCHEMA, activity)?;
            if let Some(temporal) = activity.get("temporal") {
                if text(temporal, "kind") == Some("interval") {
                    if let (Some(start), Some(end)) =
                        (text(temporal, "start"), text(temporal, "end"))
                    {
                        if start > end {
                            self.issue(
                                &location,
                                "provision-activity interval starts after it ends",
                            )?;
                        }
                    }
                }
            }
            let kind = text(activity, "provision_kind").unwrap_or_default();
            let (place_role, agent_roles): (&str, &[&str]) = match kind {
                "publication" => ("publication_place", &["publisher"]),
                "production" => ("production_place", &["producer"]),
                "distribution" => ("distribution_place", &["distributor"]),
                "manufacture" => ("manufacture_place", &["manufacturer", "printer"]),
                _ => ("", &[]),
            };
            for place in activity
                .get("places")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(role) = text(place, "role") {
                    if !place_role.is_empty() && role != place_role {
                        self.issue(
                            &location,
                            format!("{kind} provision has incompatible place role: {role}"),
                        )?;
                    }
                }
                if let Some(reference) = text(place, "normalized_place_ref") {
                    self.expect_ref(&location, Some(reference), "place")?;
                }
            }
            for agent in activity
                .get("agents")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(role) = text(agent, "role") {
                    if !agent_roles.is_empty() && !agent_roles.contains(&role) {
                        self.issue(
                            &location,
                            format!("{kind} provision has incompatible agent role: {role}"),
                        )?;
                    }
                }
                if let Some(reference) = text(agent, "normalized_agent_ref") {
                    match self.records.get(reference) {
                        None => self.issue(
                            &location,
                            format!("unresolved provision agent reference: {reference}"),
                        )?,
                        Some(record)
                            if !matches!(record.kind.as_str(), "agent" | "organization") =>
                        {
                            self.issue(
                                &location,
                                format!(
                                    "{reference} resolves to {}, expected agent or organization",
                                    record.kind
                                ),
                            )?
                        }
                        Some(_) => {}
                    }
                }
            }
            if text(activity, "event_posture") == Some("source_statement_only")
                && activity.get("temporal").is_some_and(Value::is_object)
                && text(activity.get("temporal").unwrap_or(&Value::Null), "role")
                    != Some("statement_date")
            {
                self.issue(
                    &location,
                    "source_statement_only provision must keep its temporal role at statement_date",
                )?;
            }

            let Some(event) = self.event(&reference.event).cloned() else {
                self.issue(
                    &location,
                    format!(
                        "unresolved provision-activity provenance_event_ref: {}",
                        reference.event
                    ),
                )?;
                continue;
            };
            let claim_path = location
                .rsplit_once(':')
                .map(|(path, _)| path)
                .unwrap_or(&location);
            let Some(digest) = self.digest_for(claim_path)? else {
                self.issue(
                    &location,
                    "provision-activity Claim file is absent from the current cut",
                )?;
                continue;
            };
            if !output_binds(
                &event,
                claim_path,
                "unreviewed-evidence-bearing-provision-activity-claims",
                &digest,
            ) {
                self.issue(
                    &location,
                    "provision-activity provenance event does not digest-bind the claim file",
                )?;
            }
            used_events.insert(reference.event.clone());
            if validated_events.insert(reference.event.clone()) {
                self.check_event_input_bindings(
                    &location,
                    &event,
                    "provision-activity provenance input",
                )?;
            }
            for evidence in value_strings(&claim, "evidence_refs") {
                if evidence.starts_with("ToS/") && !self.current_exists(&evidence)? {
                    self.issue(
                        &location,
                        format!("unresolved repository evidence ref: {evidence}"),
                    )?;
                } else if evidence.starts_with("tos.")
                    && !self.records.contains_key(&evidence)
                    && !self.links.contains_key(&evidence)
                {
                    self.issue(
                        &location,
                        format!("unresolved identity evidence ref: {evidence}"),
                    )?;
                }
            }
        }
        let unused: Vec<String> = self
            .provision_event_ids
            .difference(&used_events)
            .cloned()
            .collect();
        if !unused.is_empty() {
            self.issue(
                SOURCE_HOME,
                format!(
                    "provision-activity provenance events are not referenced by claims: {}",
                    python_string_list(&unused)
                ),
            )?;
        }
        Ok(())
    }

    fn check_event_input_bindings(
        &mut self,
        location: &str,
        event: &Value,
        prefix: &str,
    ) -> Result<(), ItemRefusal> {
        for input in event
            .get("inputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(reference) = text(input, "ref") else {
                continue;
            };
            if !reference.starts_with("ToS/") {
                continue;
            }
            let digest = text(input, "sha256").unwrap_or_default();
            if !self.current_exists(reference)? {
                self.issue(location, format!("{prefix} is missing: {reference}"))?;
            } else if !self.recorded_matches(reference, digest)? {
                self.issue(location, format!("{prefix} digest drifted: {reference}"))?;
            }
        }
        Ok(())
    }

    fn check_chronology(&mut self) -> Result<(), ItemRefusal> {
        let paths: Vec<String> = self
            .paths
            .iter()
            .filter(|path| path.ends_with("/work-chronology-claims.jsonl"))
            .cloned()
            .collect();
        if paths.len() != 1 || paths.first().map(String::as_str) != Some(CHRONOLOGY_CLAIMS) {
            self.issue(
                CHRONOLOGY_CLAIMS,
                "work chronology claim basename must exist only at its owned route",
            )?;
        }
        let Some(claims) = self.json_rows(CHRONOLOGY_CLAIMS, CLAIM_SCHEMA, true)? else {
            return Ok(());
        };
        let event_rows = self.json_rows(CHRONOLOGY_PROVENANCE, PROVENANCE_SCHEMA, true)?;
        let event = event_rows
            .as_ref()
            .and_then(|rows| rows.rows.first())
            .map(|(_, event)| event.clone());
        if event_rows.as_ref().is_some_and(|rows| rows.rows.len() != 1) {
            self.issue(
                CHRONOLOGY_PROVENANCE,
                "work chronology must have exactly one batch provenance event",
            )?;
        }
        if let Some(event) = &event {
            if text(event, "event_id") != Some(CHRONOLOGY_EVENT)
                || text(event, "event_type") != Some("annotation")
                || value_strings(event, "agent_refs") != vec!["model:codex".to_owned()]
                || text(event, "status") != Some("completed_with_warnings")
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance posture drifted",
                )?;
            }
            let method = event.get("method").unwrap_or(&Value::Null);
            if text(method, "maker_type") != Some("model")
                || text(method, "name")
                    != Some("faceted-first-publication-chronology-materialization")
                || text(method, "version") != Some("1")
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance method identity drifted",
                )?;
            }
        }
        let Some(claim_digest) = self.digest_for(CHRONOLOGY_CLAIMS)? else {
            return Ok(());
        };
        let mut evidence_paths = BTreeSet::new();
        let mut subjects = BTreeMap::<String, String>::new();
        let mut staged_count = 0u64;
        let mut single_count = 0u64;
        for (line, claim) in &claims.rows {
            let location = format!("{CHRONOLOGY_CLAIMS}:{line}");
            let claim_id = text(claim, "claim_id").unwrap_or_default().to_owned();
            let subject = text(claim, "subject_ref").unwrap_or_default().to_owned();
            subjects.insert(claim_id, subject.clone());
            if text(claim, "claim_type") != Some("bibliographic")
                || text(claim, "assertion_layer") != Some("scholarly_report")
                || text(claim, "predicate") != Some("first_publication_chronology")
            {
                self.issue(&location, "work chronology claim profile drifted")?;
            }
            self.expect_ref(&location, Some(&subject), "work")?;
            if !self.python_equal(
                claim.get("maker").unwrap_or(&Value::Null),
                &serde_json::json!({"maker_type":"model","agent_ref":"model:codex"}),
            )? {
                self.issue(&location, "work chronology maker must be model:codex")?;
            }
            if text(claim, "provenance_event_ref") != Some(CHRONOLOGY_EVENT)
                || text(claim, "epistemic_status") != Some("reported")
                || text(claim, "review_status") != Some("unreviewed")
                || !claim
                    .get("reviews")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
                || text(claim, "visibility") != Some("public_metadata_only")
            {
                self.issue(&location, "work chronology claim authority posture drifted")?;
            }
            let refs = value_strings(claim, "evidence_refs");
            for reference in &refs {
                if !reference.starts_with("ToS/") {
                    self.issue(
                        &location,
                        "work chronology evidence must be a tracked repository path",
                    )?;
                } else if !self.current_exists(reference)? {
                    self.issue(
                        &location,
                        format!("unresolved work chronology evidence: {reference}"),
                    )?;
                }
                if reference.starts_with("ToS/") {
                    evidence_paths.insert(reference.clone());
                }
            }
            if !refs
                .iter()
                .any(|reference| reference.contains("authorial-witness-route"))
            {
                self.issue(
                    &location,
                    "work chronology lacks its ordered discovery receipt",
                )?;
            }
            if !refs
                .iter()
                .any(|reference| reference.contains("AUTHORIAL_WITNESS_ROUTE.md"))
            {
                self.issue(&location, "work chronology lacks its documentary synthesis")?;
            }
            let chronology = claim.get("object").unwrap_or(&Value::Null);
            self.request_schema(
                &format!("{location}['object']"),
                CHRONOLOGY_SCHEMA,
                chronology,
            )?;
            let interval = chronology.get("interval").unwrap_or(&Value::Null);
            let start = text(interval, "start");
            let end = text(interval, "end");
            if start.zip(end).is_some_and(|(start, end)| start > end) {
                self.issue(&location, "work chronology interval starts after it ends")?;
            }
            let stages = chronology
                .get("stages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let dates: Vec<String> = stages
                .iter()
                .filter_map(|stage| text(stage, "date").map(str::to_owned))
                .collect();
            if dates.windows(2).any(|pair| pair[0] > pair[1]) {
                self.issue(&location, "work chronology stages are not date ordered")?;
            }
            let posture = text(chronology, "sequence_posture");
            let boundary = text(interval, "boundary_meaning");
            if posture == Some("single_event") {
                single_count += 1;
                if stages.len() != 1 || boundary != Some("single_stage") {
                    self.issue(
                        &location,
                        "single-event chronology must contain one single-stage boundary",
                    )?;
                }
            } else if posture == Some("staged_sequence") {
                staged_count += 1;
                if stages.len() < 2 || boundary != Some("earliest_stage_to_sequence_completion") {
                    self.issue(
                        &location,
                        "staged chronology must retain multiple sequence stages",
                    )?;
                }
            }
            if let Some(first) = dates.first() {
                if start.is_some_and(|start| !first.starts_with(start)) {
                    self.issue(
                        &location,
                        "chronology interval start differs from first stage",
                    )?;
                }
            }
            if let Some(last) = dates.last() {
                if end.is_some_and(|end| !last.starts_with(end)) {
                    self.issue(&location, "chronology interval end differs from last stage")?;
                }
            }
            for stage in &stages {
                let Some(edition_ref) = text(stage, "edition_ref") else {
                    continue;
                };
                self.expect_ref(&location, Some(edition_ref), "edition")?;
                let Some(edition) = self.records.get(edition_ref) else {
                    continue;
                };
                let same_work = value_strings(&edition.value, "embodies_expression_refs")
                    .iter()
                    .any(|expression_ref| {
                        self.records.get(expression_ref).is_some_and(|expression| {
                            text(&expression.value, "work_ref") == Some(subject.as_str())
                        })
                    });
                if !same_work {
                    self.issue(
                        &location,
                        format!("chronology stage edition belongs to another Work: {edition_ref}"),
                    )?;
                }
            }
        }
        let current_works: BTreeSet<String> = self
            .records
            .iter()
            .filter(|(_, record)| {
                record.kind == "work"
                    && record
                        .path
                        .starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")
            })
            .map(|(id, _)| id.clone())
            .collect();
        let chronology_works: BTreeSet<String> = subjects.values().cloned().collect();
        if chronology_works != current_works {
            self.issue(
                CHRONOLOGY_CLAIMS,
                "work chronology subjects do not close over the current Nietzsche Works",
            )?;
        }
        if let Some(event) = &event {
            if !output_binds(
                event,
                CHRONOLOGY_CLAIMS,
                "unreviewed-evidence-bearing-work-chronology-claims",
                &claim_digest,
            ) || event
                .get("outputs")
                .and_then(Value::as_array)
                .is_none_or(|outputs| outputs.len() != 1)
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance does not digest-bind the exact claim file",
                )?;
            }
            let mut expected_inputs = evidence_paths;
            expected_inputs.insert(CLAIM_SCHEMA.to_owned());
            expected_inputs.insert(WORK_CHRONOLOGY_SCHEMA.to_owned());
            let inputs = event
                .get("inputs")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let actual_inputs: BTreeSet<String> = inputs
                .iter()
                .filter_map(|row| text(row, "ref").map(str::to_owned))
                .collect();
            if actual_inputs != expected_inputs || inputs.len() != expected_inputs.len() {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance inputs differ from the exact claim evidence set",
                )?;
            }
            self.check_event_input_bindings(
                CHRONOLOGY_PROVENANCE,
                event,
                "work chronology provenance input",
            )?;
            let expected_configuration = serde_json::json!({
                "works_materialized": 7,
                "chronology_claims_materialized": 7,
                "staged_sequence_claims": 1,
                "single_event_claims": 6,
                "chronology_claims_reviewed": 0,
                "composition_claims_created": 0,
                "source_text_admitted": false,
                "human_review_performed": false,
                "semantic_claims_created": 0,
                "canon_promotion_performed": false
            });
            if !self.python_equal(
                event
                    .get("method")
                    .and_then(|method| method.get("configuration"))
                    .unwrap_or(&Value::Null),
                &expected_configuration,
            )? || subjects.len() != 7
                || staged_count != 1
                || single_count != 6
            {
                self.issue(
                    CHRONOLOGY_PROVENANCE,
                    "work chronology provenance configuration differs from the bounded profile",
                )?;
            }
        }
        Ok(())
    }

    fn check_object_links(&mut self) -> Result<(), ItemRefusal> {
        let claims: Vec<(String, ClaimRef)> = self
            .object_links
            .iter()
            .map(|(id, claim)| (id.clone(), claim.clone()))
            .collect();
        let mut targets = BTreeMap::<String, String>::new();
        let mut events = BTreeMap::<String, String>::new();
        for (claim_id, claim) in claims {
            check(self.limits.deadline, self.source.cancellation())?;
            let location = &claim.location;
            if !self.records.contains_key(&claim.subject) && !claim.native {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            } else if self
                .records
                .get(&claim.subject)
                .is_some_and(|record| record.kind == "link")
            {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            } else if claim.native
                && !self.records.get(&claim.subject).is_some_and(|record| {
                    matches!(
                        record.kind.as_str(),
                        "work" | "expression" | "edition" | "collection" | "item" | "artifact"
                    )
                })
            {
                self.issue(
                    location,
                    format!(
                        "unresolved or invalid native object-Link subject: {}",
                        claim.subject
                    ),
                )?;
            }
            if !self.links.contains_key(&claim.object) {
                self.issue(
                    location,
                    format!("unresolved Link object: {}", claim.object),
                )?;
            }
            if !self.event_ids.contains(&claim.event) {
                self.issue(
                    location,
                    format!(
                        "unresolved object-Link provenance_event_ref: {}",
                        claim.event
                    ),
                )?;
            }
            targets.insert(claim_id.clone(), claim.object.clone());
            events.insert(claim_id, claim.event.clone());
        }
        let links: Vec<(String, String, Value)> = self
            .links
            .iter()
            .map(|(id, (path, value))| (id.clone(), path.clone(), value.clone()))
            .collect();
        for (link_id, path, link) in links {
            let location = path;
            let refs = value_strings(&link, "association_claim_refs");
            let ref_set: BTreeSet<String> = refs.iter().cloned().collect();
            let missing: Vec<String> = ref_set
                .iter()
                .filter(|id| !self.object_links.contains_key(*id))
                .cloned()
                .collect();
            if !missing.is_empty() {
                self.issue(
                    &location,
                    format!(
                        "unresolved object-Link claims: {}",
                        python_string_list(&missing)
                    ),
                )?;
            }
            let misbound: Vec<String> = ref_set
                .iter()
                .filter(|id| {
                    self.object_links.contains_key(*id)
                        && targets.get(*id).map(String::as_str) != Some(link_id.as_str())
                })
                .cloned()
                .collect();
            if !misbound.is_empty() {
                self.issue(
                    &location,
                    format!(
                        "object-Link claims target another Link: {}",
                        python_string_list(&misbound)
                    ),
                )?;
            }
            let event_ref = text(&link, "provenance_event_ref").unwrap_or_default();
            let event_mismatch: Vec<String> = ref_set
                .iter()
                .filter(|id| {
                    self.object_links.contains_key(*id)
                        && events.get(*id).map(String::as_str) != Some(event_ref)
                })
                .cloned()
                .collect();
            if !event_mismatch.is_empty() {
                self.issue(
                    &location,
                    format!(
                        "object-Link claims cite another provenance event: {}",
                        python_string_list(&event_mismatch)
                    ),
                )?;
            }
            let unreferenced: Vec<String> = targets
                .iter()
                .filter(|(id, target)| {
                    target.as_str() == link_id.as_str() && !ref_set.contains(*id)
                })
                .map(|(id, _)| id.clone())
                .collect();
            if !unreferenced.is_empty() {
                self.issue(
                    &location,
                    format!(
                        "object-Link claims are not referenced by Link: {}",
                        python_string_list(&unreferenced)
                    ),
                )?;
            }
        }
        Ok(())
    }

    fn check_record_backlinks(&mut self) -> Result<(), ItemRefusal> {
        let records: Vec<(String, BiblioCurrentRecord)> = self
            .records
            .iter()
            .map(|(id, record)| (id.clone(), record.clone()))
            .collect();
        for (id, record) in records {
            check(self.limits.deadline, self.source.cancellation())?;
            let location = record.path.clone();
            if record.kind == "collection" {
                let actual: BTreeSet<String> =
                    value_strings(&record.value, "membership_claim_refs")
                        .into_iter()
                        .collect();
                let valid_ids: BTreeSet<String> = self
                    .membership
                    .keys()
                    .filter(|claim_id| {
                        self.membership
                            .get(*claim_id)
                            .is_some_and(|claim| claim.subject == id)
                    })
                    .cloned()
                    .collect();
                if actual != valid_ids
                    || value_strings(&record.value, "membership_claim_refs").len() != actual.len()
                {
                    self.issue(&location, "unresolved or mismatched membership claims: Collection membership refs do not close over all verified current Claims")?;
                }
            }
            if matches!(record.kind.as_str(), "work" | "expression" | "edition") {
                self.check_exact_backrefs(
                    &location,
                    &record.value,
                    "responsibility_claim_refs",
                    &id,
                    "responsibility",
                )?;
                if record.kind == "work"
                    && location.starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")
                {
                    let actual: BTreeSet<String> =
                        value_strings(&record.value, "responsibility_claim_refs")
                            .into_iter()
                            .collect();
                    let authored: Vec<String> = actual
                        .iter()
                        .filter(|claim_id| {
                            self.responsibility
                                .get(*claim_id)
                                .is_some_and(|claim| claim.predicate == "authored_by")
                        })
                        .cloned()
                        .collect();
                    if authored.len() != 1 {
                        self.issue(
                            &location,
                            format!(
                                "current Nietzsche Work must reference exactly one authored_by claim; found {}",
                                python_string_list(&authored)
                            ),
                        )?;
                    } else if self
                        .responsibility
                        .get(&authored[0])
                        .map(|claim| claim.object.as_str())
                        != Some("tos.agent.friedrich-nietzsche")
                    {
                        self.issue(&location, "current Nietzsche Work authored_by claim must resolve to tos.agent.friedrich-nietzsche")?;
                    }
                }
            }
            if record.kind == "edition" {
                self.check_exact_backrefs(
                    &location,
                    &record.value,
                    "publication_claim_refs",
                    &id,
                    "publication",
                )?;
                self.check_exact_backrefs(
                    &location,
                    &record.value,
                    "provision_activity_claim_refs",
                    &id,
                    "provision-activity",
                )?;
            }
            if record.kind == "work"
                && location.starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")
            {
                let expected: BTreeSet<String> = self
                    .chronology
                    .iter()
                    .filter(|(_, claim)| claim.subject == id)
                    .map(|(claim_id, _)| claim_id.clone())
                    .collect();
                let refs = value_strings(&record.value, "chronology_claim_refs");
                let actual: BTreeSet<String> = refs.iter().cloned().collect();
                if actual != expected || expected.len() != 1 || refs.len() != actual.len() {
                    self.issue(
                        &location,
                        format!(
                            "current Nietzsche Work must reference exactly one first_publication_chronology claim; found {}",
                            python_string_list(&actual.iter().cloned().collect::<Vec<_>>())
                        ),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn check_exact_backrefs(
        &mut self,
        location: &str,
        record: &Value,
        field: &str,
        record_id: &str,
        label: &str,
    ) -> Result<(), ItemRefusal> {
        let refs = value_strings(record, field);
        let actual: BTreeSet<String> = refs.iter().cloned().collect();
        let (missing, misbound, unreferenced) = {
            let claims = match label {
                "responsibility" => &self.responsibility,
                "publication" => &self.publication,
                "provision-activity" => &self.provision,
                _ => unreachable!("fixed backref claim families"),
            };
            let known: BTreeSet<String> = claims.keys().cloned().collect();
            let missing = actual.difference(&known).cloned().collect::<Vec<_>>();
            let misbound = actual
                .intersection(&known)
                .filter(|claim_id| {
                    claims
                        .get(*claim_id)
                        .is_some_and(|claim| claim.subject != record_id)
                })
                .cloned()
                .collect::<Vec<_>>();
            let unreferenced = claims
                .iter()
                .filter(|(_, claim)| claim.subject == record_id)
                .filter(|(claim_id, _)| !actual.contains(*claim_id))
                .map(|(claim_id, _)| claim_id.clone())
                .collect::<Vec<_>>();
            (missing, misbound, unreferenced)
        };
        if !missing.is_empty() {
            self.issue(
                location,
                format!(
                    "unresolved {label} claims: {}",
                    python_string_list(&missing)
                ),
            )?;
        }
        if !misbound.is_empty() {
            self.issue(
                location,
                format!(
                    "{label} claims belong to another subject: {}",
                    python_string_list(&misbound)
                ),
            )?;
        }
        if !unreferenced.is_empty() {
            self.issue(
                location,
                format!(
                    "subject {label} claims are not referenced: {}",
                    python_string_list(&unreferenced)
                ),
            )?;
        }
        if refs.len() != actual.len() {
            self.issue(
                location,
                format!("{field} contains duplicate claim references"),
            )?;
        }
        Ok(())
    }

    fn register_claim(
        &mut self,
        path: &str,
        line: usize,
        claim: &Value,
        native: bool,
    ) -> Result<(), ItemRefusal> {
        let location = format!("{path}:{line}");
        let id = self.claim_id(&location, claim)?;
        let subject = text(claim, "subject_ref").unwrap_or_default().to_owned();
        let predicate = text(claim, "predicate").unwrap_or_default().to_owned();
        let object = text(claim, "object").unwrap_or_default().to_owned();
        let event = text(claim, "provenance_event_ref")
            .unwrap_or_default()
            .to_owned();

        if !event.is_empty() && !self.event_ids.contains(&event) {
            self.issue(
                &location,
                format!("unresolved provenance_event_ref: {event}"),
            )?;
        }
        for evidence in value_strings(claim, "evidence_refs") {
            if evidence.starts_with("tos.anchor.") && !self.anchors.contains(&evidence) {
                self.issue(
                    &location,
                    format!("unresolved source evidence anchor: {evidence}"),
                )?;
            } else if evidence.starts_with("ToS/") && !self.current_exists(&evidence)? {
                self.issue(
                    &location,
                    format!("unresolved repository evidence ref: {evidence}"),
                )?;
            }
        }
        let Some(id) = id else {
            return Ok(());
        };
        self.reserve(
            subject.len() + predicate.len() + object.len() + event.len() + location.len() + 128,
        )?;
        let reference = ClaimRef {
            location: location.clone(),
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: object.clone(),
            event: event.clone(),
            native,
        };

        if path.ends_with("/membership-claims.jsonl") || predicate == "contains_work" {
            self.expect_ref(&location, Some(&subject), "collection")?;
            self.expect_ref(&location, Some(&object), "work")?;
            self.membership.insert(id.clone(), reference.clone());
        }
        if path.ends_with("/responsibility-claims.jsonl")
            || matches!(
                predicate.as_str(),
                "authored_by"
                    | "contributed_by"
                    | "translated_by"
                    | "edited_by"
                    | "afterword_by"
                    | "designed_by"
            )
        {
            let expected_subject = match predicate.as_str() {
                "authored_by" | "contributed_by" => "work",
                "translated_by" => "expression",
                "edited_by" | "afterword_by" | "designed_by" => "edition",
                _ => "",
            };
            if !expected_subject.is_empty() {
                self.expect_ref(&location, Some(&subject), expected_subject)?;
            }
            self.expect_ref(&location, Some(&object), "agent")?;
            self.responsibility.insert(id.clone(), reference.clone());
        }
        if path.ends_with("/publication-claims.jsonl") {
            self.expect_ref(&location, Some(&subject), "edition")?;
            self.publication.insert(id.clone(), reference.clone());
        }
        if path.ends_with("/provision-activity-claims.jsonl") {
            self.expect_ref(&location, Some(&subject), "edition")?;
            self.reserve(crate::record_biblio_cut::decoded_state(claim)?)?;
            self.provision_values.insert(id.clone(), claim.clone());
            self.provision.insert(id.clone(), reference.clone());
        }
        if path == CHRONOLOGY_CLAIMS {
            self.expect_ref(&location, Some(&subject), "work")?;
            self.chronology.insert(id.clone(), reference.clone());
        }
        if path.ends_with("/object-link-claims.jsonl")
            || claim.get("schema_version").and_then(Value::as_str)
                == Some("tos_object_link_claim_v2")
        {
            self.object_links.insert(id.clone(), reference.clone());
        }
        if TOPOLOGY_ROUTES.iter().any(|(route, ..)| *route == path)
            || matches!(
                predicate.as_str(),
                "has_expression" | "embodied_by" | "exemplified_by"
            )
        {
            self.topology.insert(id.clone(), reference.clone());
        }
        if path == DERIVATION_CLAIMS || predicate == "is_derivative_of" {
            self.derivation.insert(id.clone(), reference);
        }
        Ok(())
    }

    fn validate_source_refs(&mut self, location: &str, value: &Value) -> Result<(), ItemRefusal> {
        for field in ["inputs", "outputs", "evidence_refs"] {
            let Some(rows) = value.get(field).and_then(Value::as_array) else {
                continue;
            };
            for row in rows {
                let (reference, expected_digest) = if let Some(object) = row.as_object() {
                    (
                        object.get("ref").and_then(Value::as_str),
                        object.get("sha256").and_then(Value::as_str),
                    )
                } else {
                    (row.as_str(), None)
                };
                let Some(reference) = reference.filter(|reference| reference.starts_with("ToS/"))
                else {
                    continue;
                };
                if !self.current_exists(reference)? {
                    self.issue(location, format!("unresolved source ref: {reference}"))?;
                    continue;
                }
                if let Some(expected_digest) = expected_digest {
                    if !self.recorded_matches(reference, expected_digest)? {
                        self.issue(
                            location,
                            format!("source ref digest is unresolved: {reference}"),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn check_boundary_maps_and_anchors(&mut self) -> Result<(), ItemRefusal> {
        let map_paths: Vec<String> = self
            .paths
            .iter()
            .filter(|path| path.ends_with("/work-boundary-map.json"))
            .cloned()
            .collect();
        let mut boundary_anchor_ids = BTreeSet::new();
        for map_path in &map_paths {
            let Some(loaded) = self.json_rows(map_path, BOUNDARY_MAP_SCHEMA, true)? else {
                continue;
            };
            let Some((_, boundary_map)) = loaded.rows.first() else {
                continue;
            };
            self.expect_ref(map_path, text(boundary_map, "collection_ref"), "collection")?;
            self.expect_ref(map_path, text(boundary_map, "edition_ref"), "edition")?;
            self.expect_ref(map_path, text(boundary_map, "item_ref"), "item")?;

            if let Some(inventory_ref) = text(boundary_map, "resource_inventory_ref") {
                let inventory_ref = inventory_ref.to_owned();
                let expected = text(boundary_map, "resource_inventory_sha256")
                    .unwrap_or_default()
                    .to_owned();
                if !self.current_exists(&inventory_ref)? {
                    self.issue(
                        map_path,
                        format!("work-boundary resource inventory is missing: {inventory_ref}"),
                    )?;
                } else if self.digest_for(&inventory_ref)?.as_deref() != Some(expected.as_str()) {
                    self.issue(map_path, "work-boundary resource inventory digest drifted")?;
                }
            }

            let item_ref = boundary_map.get("item_ref").unwrap_or(&Value::Null);
            let file_id = boundary_map.get("file_id").unwrap_or(&Value::Null);
            if !self.file_memberships.contains(item_ref, file_id) {
                self.issue(map_path, "work-boundary file does not belong to its item")?;
            }
            let manifest_sha = self
                .file_memberships
                .sha256_for(file_id)
                .unwrap_or(&Value::Null);
            let map_sha = boundary_map.get("file_sha256").unwrap_or(&Value::Null);
            if !self.python_equal(manifest_sha, map_sha)? {
                self.issue(
                    map_path,
                    "work-boundary file digest differs from the item manifest",
                )?;
            }
            if !self
                .event_ids
                .contains(text(boundary_map, "provenance_event_ref").unwrap_or_default())
            {
                self.issue(map_path, "work-boundary provenance event is unresolved")?;
            }

            let anchor_path = map_path
                .rsplit_once('/')
                .map(|(parent, _)| format!("{parent}/anchors.jsonl"))
                .unwrap_or_else(|| "anchors.jsonl".to_owned());
            let anchors = self.json_rows(&anchor_path, ANCHOR_SCHEMA, true)?;
            let mut local_ids = BTreeSet::new();
            let mut page_by_id = BTreeMap::new();
            if let Some(anchors) = anchors {
                for (line, anchor) in anchors.rows {
                    let location = format!("{anchor_path}:{line}");
                    self.register_anchor(
                        &location,
                        &anchor,
                        &mut boundary_anchor_ids,
                        &mut local_ids,
                        &mut page_by_id,
                    )?;
                    if !self
                        .python_equal(anchor.get("item_id").unwrap_or(&Value::Null), item_ref)?
                    {
                        self.issue(&location, "boundary anchor item_id differs from map")?;
                    }
                    if !self.python_equal(anchor.get("file_id").unwrap_or(&Value::Null), file_id)? {
                        self.issue(&location, "boundary anchor file_id differs from map")?;
                    }
                    if text(&anchor, "file_sha256") != text(boundary_map, "file_sha256") {
                        self.issue(&location, "boundary anchor file digest differs from map")?;
                    }
                    if text(&anchor, "provenance_event_ref")
                        != text(boundary_map, "provenance_event_ref")
                    {
                        self.issue(&location, "boundary anchor provenance differs from map")?;
                    }
                    if page_by_id
                        .get(text(&anchor, "anchor_id").unwrap_or_default())
                        .is_some_and(|page| {
                            boundary_map
                                .get("page_count")
                                .and_then(Value::as_u64)
                                .is_some_and(|count| *page > count)
                        })
                    {
                        self.issue(&location, "boundary anchor page exceeds page_count")?;
                    }
                }
            }

            let members = boundary_map
                .get("members")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let sequences: Vec<u64> = members
                .iter()
                .filter_map(|member| member.get("sequence").and_then(Value::as_u64))
                .collect();
            if sequences != (1..=members.len() as u64).collect::<Vec<_>>() {
                self.issue(
                    map_path,
                    "work-boundary member sequence is not contiguous from 1",
                )?;
            }
            let coverage = text(boundary_map, "coverage_posture");
            let explicit_coverage = coverage.is_some();
            let source_sequences: Vec<u64> = members
                .iter()
                .filter_map(|member| member.get("source_sequence").and_then(Value::as_u64))
                .collect();
            if coverage == Some("partial_membership_representation")
                && (source_sequences.len() != members.len()
                    || source_sequences.windows(2).any(|pair| pair[0] >= pair[1]))
            {
                self.issue(map_path, "partial work-boundary source_sequence values must be present, strictly increasing and unique")?;
            }
            let mut previous_end = None;
            let mut represented_ranges: Vec<(u64, u64, String)> = Vec::new();
            for member in &members {
                self.check_boundary_member(
                    map_path,
                    member,
                    &local_ids,
                    &page_by_id,
                    &mut previous_end,
                    explicit_coverage,
                    &mut represented_ranges,
                )?;
            }
            let non_member_sections = boundary_map
                .get("non_member_sections")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if explicit_coverage {
                previous_end = None;
            }
            for section in &non_member_sections {
                self.check_boundary_section(
                    map_path,
                    section,
                    &local_ids,
                    &page_by_id,
                    "non-member",
                    &mut previous_end,
                    &mut represented_ranges,
                )?;
            }
            let unrepresented = boundary_map
                .get("unrepresented_sections")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if coverage == Some("complete_membership_representation") && !unrepresented.is_empty() {
                self.issue(
                    map_path,
                    "complete work-boundary representation cannot contain unrepresented sections",
                )?;
            }
            if coverage == Some("partial_membership_representation") && unrepresented.is_empty() {
                self.issue(map_path, "partial work-boundary representation requires at least one unrepresented section")?;
            }
            for section in &unrepresented {
                self.check_boundary_section(
                    map_path,
                    section,
                    &local_ids,
                    &page_by_id,
                    "unrepresented",
                    &mut previous_end,
                    &mut represented_ranges,
                )?;
            }
            if explicit_coverage {
                represented_ranges.sort();
                let mut coverage_end = 0u64;
                for (start, end, label) in represented_ranges {
                    if start != coverage_end.saturating_add(1) {
                        self.issue(map_path, format!("explicit work-boundary coverage has a gap or overlap before {label}"))?;
                    }
                    coverage_end = coverage_end.max(end);
                }
                if Some(coverage_end) != boundary_map.get("page_count").and_then(Value::as_u64) {
                    self.issue(
                        map_path,
                        "explicit work-boundary coverage does not cover the exact page_count",
                    )?;
                }
            } else if previous_end != boundary_map.get("page_count").and_then(Value::as_u64) {
                self.issue(
                    map_path,
                    "work and non-member boundaries do not cover the exact page_count",
                )?;
            }
            for reference in value_strings(boundary_map, "crosscheck_anchor_refs") {
                if !local_ids.contains(&reference) {
                    self.issue(
                        map_path,
                        format!("unresolved boundary crosscheck anchor: {reference}"),
                    )?;
                }
            }
            let mut membership_refs = BTreeSet::new();
            for member in members {
                if let Some(reference) = text(&member, "membership_claim_ref") {
                    membership_refs.insert(reference.to_owned());
                }
                if let Some(reference) = text(&member, "responsibility_claim_ref")
                    .or_else(|| text(&member, "translation_responsibility_claim_ref"))
                {
                    self.boundary_responsibility_refs
                        .insert(reference.to_owned());
                }
            }
            self.boundary_membership_refs.extend(membership_refs);
        }

        let non_boundary_anchor_paths: Vec<String> = self
            .paths
            .iter()
            .filter(|path| path.ends_with("/anchors.jsonl"))
            .filter(|path| {
                let map_path = path
                    .rsplit_once('/')
                    .map(|(parent, _)| format!("{parent}/work-boundary-map.json"));
                !map_path.is_some_and(|candidate| map_paths.contains(&candidate))
            })
            .cloned()
            .collect();
        let mut evidence_anchor_ids = boundary_anchor_ids;
        for anchor_path in non_boundary_anchor_paths {
            if let Some(loaded) = self.json_rows(&anchor_path, ANCHOR_SCHEMA, false)? {
                for (line, anchor) in loaded.rows {
                    let location = format!("{anchor_path}:{line}");
                    let id = text(&anchor, "anchor_id").map(str::to_owned);
                    if let Some(id) = id {
                        self.reserve(id.len() + std::mem::size_of::<String>())?;
                        if !evidence_anchor_ids.insert(id.clone()) {
                            self.issue(
                                &location,
                                format!("duplicate source evidence anchor_id: {id}"),
                            )?;
                        }
                        self.anchors.insert(id);
                    }
                    self.expect_ref(&location, text(&anchor, "item_id"), "item")?;
                    if let Some(event_ref) = text(&anchor, "provenance_event_ref") {
                        if !self.event_ids.contains(event_ref) {
                            self.issue(
                                &location,
                                format!("unresolved source-anchor provenance event: {event_ref}"),
                            )?;
                        }
                    }
                }
            }
        }
        self.anchors.extend(evidence_anchor_ids);
        Ok(())
    }

    fn register_anchor(
        &mut self,
        location: &str,
        anchor: &Value,
        all_ids: &mut BTreeSet<String>,
        local_ids: &mut BTreeSet<String>,
        page_by_id: &mut BTreeMap<String, u64>,
    ) -> Result<(), ItemRefusal> {
        let Some(id) = text(anchor, "anchor_id").map(str::to_owned) else {
            return Ok(());
        };
        self.reserve(id.len() + std::mem::size_of::<String>())?;
        if !all_ids.insert(id.clone()) {
            self.issue(location, format!("duplicate boundary anchor_id: {id}"))?;
        }
        local_ids.insert(id.clone());
        let page_selectors: Vec<&Value> = anchor
            .get("selectors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|selector| text(selector, "type") == Some("page_region"))
            .collect();
        if page_selectors.len() == 1 {
            if let Some(page) = page_selectors[0].get("page").and_then(Value::as_u64) {
                page_by_id.insert(id, page);
            }
        } else {
            self.issue(
                location,
                "boundary anchor must have exactly one page selector",
            )?;
        }
        Ok(())
    }

    fn check_boundary_member(
        &mut self,
        location: &str,
        member: &Value,
        local_ids: &BTreeSet<String>,
        page_by_id: &BTreeMap<String, u64>,
        previous_end: &mut Option<u64>,
        explicit_coverage: bool,
        represented_ranges: &mut Vec<(u64, u64, String)>,
    ) -> Result<(), ItemRefusal> {
        self.expect_ref(location, text(member, "work_ref"), "work")?;
        self.expect_ref(location, text(member, "expression_ref"), "expression")?;
        if let (Some(work_ref), Some(expression_ref)) =
            (text(member, "work_ref"), text(member, "expression_ref"))
        {
            if self
                .records
                .get(expression_ref)
                .and_then(|record| text(&record.value, "work_ref"))
                != Some(work_ref)
            {
                self.issue(
                    location,
                    format!("work-boundary expression belongs to another work: {expression_ref}"),
                )?;
            }
        }
        let start = member.get("start_page").and_then(Value::as_u64);
        let end = member.get("end_page").and_then(Value::as_u64);
        if let (Some(start), Some(end)) = (start, end) {
            if start > end {
                self.issue(
                    location,
                    format!(
                        "work-boundary start exceeds end for sequence {}",
                        member.get("sequence").unwrap_or(&Value::Null)
                    ),
                )?;
            }
            if !explicit_coverage
                && previous_end.is_some_and(|previous| previous.checked_add(1) != Some(start))
            {
                self.issue(
                    location,
                    format!(
                        "work-boundary members are not contiguous at sequence {}",
                        member.get("sequence").unwrap_or(&Value::Null)
                    ),
                )?;
            }
            *previous_end = Some(end);
            represented_ranges.push((
                start,
                end,
                format!(
                    "member sequence {}",
                    member
                        .get("sequence")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                ),
            ));
        }
        self.check_boundary_anchor_ref(
            location,
            text(member, "title_page_anchor_ref"),
            start,
            local_ids,
            page_by_id,
            "title-page",
        )?;
        for anchor_ref in value_strings(member, "boundary_evidence_anchor_refs") {
            if !local_ids.contains(&anchor_ref) {
                self.issue(
                    location,
                    format!("unresolved member boundary anchor: {anchor_ref}"),
                )?;
            }
        }
        if let Some(reference) = text(member, "membership_claim_ref") {
            self.boundary_membership_refs.insert(reference.to_owned());
        }
        if let Some(reference) = text(member, "responsibility_claim_ref")
            .or_else(|| text(member, "translation_responsibility_claim_ref"))
        {
            self.boundary_responsibility_refs
                .insert(reference.to_owned());
        }
        Ok(())
    }

    fn check_boundary_section(
        &mut self,
        location: &str,
        section: &Value,
        local_ids: &BTreeSet<String>,
        page_by_id: &BTreeMap<String, u64>,
        label: &str,
        previous_end: &mut Option<u64>,
        represented_ranges: &mut Vec<(u64, u64, String)>,
    ) -> Result<(), ItemRefusal> {
        let start = section.get("start_page").and_then(Value::as_u64);
        let end = section.get("end_page").and_then(Value::as_u64);
        if label != "unrepresented" {
            if let Some(start) = start {
                if previous_end.is_some_and(|previous| previous.checked_add(1) != Some(start)) {
                    self.issue(
                        location,
                        format!(
                            "{label} section is not contiguous: {}",
                            text(section, "label").unwrap_or("")
                        ),
                    )?;
                }
            }
        }
        if let (Some(start), Some(end)) = (start, end) {
            if start > end {
                self.issue(
                    location,
                    format!(
                        "{label} section start exceeds end: {}",
                        text(section, "label").unwrap_or("")
                    ),
                )?;
            }
            *previous_end = Some(end);
            represented_ranges.push((
                start,
                end,
                format!("{label} section {}", text(section, "label").unwrap_or("")),
            ));
        }
        self.check_boundary_anchor_ref(
            location,
            text(section, "boundary_anchor_ref"),
            start,
            local_ids,
            page_by_id,
            label,
        )?;
        Ok(())
    }

    fn check_boundary_anchor_ref(
        &mut self,
        location: &str,
        anchor_ref: Option<&str>,
        start_page: Option<u64>,
        local_ids: &BTreeSet<String>,
        page_by_id: &BTreeMap<String, u64>,
        label: &str,
    ) -> Result<(), ItemRefusal> {
        let Some(anchor_ref) = anchor_ref else {
            self.issue(location, format!("missing {label} anchor reference"))?;
            return Ok(());
        };
        if !local_ids.contains(anchor_ref) {
            self.issue(location, format!("unresolved {label} anchor: {anchor_ref}"))?;
        } else if page_by_id.get(anchor_ref).copied() != start_page {
            self.issue(
                location,
                format!("{label} anchor does not match start_page: {anchor_ref}"),
            )?;
        }
        Ok(())
    }
}

fn loaded_clone_cost(rows: &LoadedRows) -> Result<usize, ItemRefusal> {
    rows.rows.iter().try_fold(
        rows.digest
            .len()
            .checked_add(std::mem::size_of::<LoadedRows>())
            .ok_or(ItemRefusal::Budget)?,
        |used, (line, value)| {
            used.checked_add(std::mem::size_of::<(usize, Value)>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(line)))
                .and_then(|bytes| {
                    crate::record_biblio_cut::decoded_state(value)
                        .ok()
                        .and_then(|size| bytes.checked_add(size))
                })
                .ok_or(ItemRefusal::Budget)
        },
    )
}

fn check(deadline: Instant, cancelled: &std::sync::atomic::AtomicBool) -> Result<(), ItemRefusal> {
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "source-foundation closure cancelled".into(),
        ));
    }
    Ok(())
}

fn assessment_refusal(error: crate::assessment::AssessmentRefusal) -> ItemRefusal {
    use crate::assessment::AssessmentRefusal;
    match error {
        AssessmentRefusal::Budget => ItemRefusal::Budget,
        AssessmentRefusal::Deadline => ItemRefusal::Deadline,
        AssessmentRefusal::Cancelled => {
            ItemRefusal::Source("source-foundation closure cancelled during Python equality".into())
        }
        AssessmentRefusal::Schema(ItemRefusal::Budget) => ItemRefusal::Budget,
        AssessmentRefusal::Schema(ItemRefusal::BudgetCheck { check, used, limit }) => {
            ItemRefusal::BudgetCheck { check, used, limit }
        }
        AssessmentRefusal::Schema(ItemRefusal::Deadline) => ItemRefusal::Deadline,
        AssessmentRefusal::Schema(ItemRefusal::Source(_)) => {
            ItemRefusal::Source("source-foundation equality source check failed".into())
        }
        AssessmentRefusal::Schema(ItemRefusal::Unsupported(_))
        | AssessmentRefusal::Unsupported(_) => ItemRefusal::Unsupported(
            "source-foundation Python equality profile is unsupported".into(),
        ),
        AssessmentRefusal::InvalidInput(_) => ItemRefusal::Unsupported(
            "source-foundation Python equality input is outside its maintained profile".into(),
        ),
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn value_strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn python_string_list(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| python_string_repr(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn python_string_repr(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut rendered = String::with_capacity(value.len().saturating_add(2));
    rendered.push(quote);
    for character in value.chars() {
        match character {
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            value if value == quote => {
                rendered.push('\\');
                rendered.push(value);
            }
            value if value.is_control() => rendered.push_str(&format!("\\u{:04x}", value as u32)),
            value => rendered.push(value),
        }
    }
    rendered.push(quote);
    rendered
}

fn json_parse_reason(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Io => "I/O error",
        serde_json::error::Category::Syntax => "syntax error",
        serde_json::error::Category::Data => "data error",
        serde_json::error::Category::Eof => "incomplete input",
    }
}

fn output_binds(event: &Value, reference: &str, role: &str, digest: &str) -> bool {
    event
        .get("outputs")
        .and_then(Value::as_array)
        .is_some_and(|outputs| {
            outputs.iter().any(|output| {
                text(output, "ref") == Some(reference)
                    && text(output, "role") == Some(role)
                    && text(output, "sha256") == Some(digest)
            })
        })
}
