//! Bounded, exact-cut schema diagnostics for the maintained source foundation.
//!
//! This adapter selects only the source-foundation root contracts listed below
//! from the caller's immutable current cut. Schema resources are read from that
//! same cut so local `$ref` resolution cannot silently fall back to checkout or
//! host files. Diagnostics are opt-in protocol v2 results; the legacy v1
//! boolean executor remains a separate transport and must not be run for the
//! same check.

use crate::executor::{
    BatchBudget, BatchCoverageExpectation, BatchUnit, BoundedSchemaExecutor,
    DiagnosticsInputProfile, DiagnosticsUnitInputMode, ExactWorkerIdentity, ExceptionalSchemaUsage,
    ExchangeFailureContext, ExecutorFailure, MixedDiagnosticsBatchUnit,
    SchemaDiagnosticsCheckpoint, SchemaDiagnosticsOutcome, SharedSchemaWorkerQuota,
    VerifiedWorkerImageHandle, schema_diagnostics,
};
use crate::record_biblio_cut::SourceCutInputWithIdentity;
use crate::{FormatProfile, SchemaBackendProbe, SchemaResource};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, StreamedCorpusCutReaderV1};

const CONTRACT_PREFIX: &str = "ToS/contracts/";
const CONTRACT_SUFFIX: &str = ".schema.json";
const SOURCE_FOUNDATION_CATALOG_PATH: &str = "ToS/contracts/source-witness-catalog.schema.json";
const CATALOG_CLAIM_ENTRY_SCHEMA_URI: &str =
    "https://tree-of-sophia.local/.well-known/source-foundation-v2/catalog-claim-entry.schema.json";
pub(crate) const MAX_LOCATION_BYTES: usize = 4096;
const MAX_CONTRACTS: usize = SchemaBackendProbe::MAX_RESOURCES;
const MAX_SCHEMA_RESOURCE_BYTES: usize = SchemaBackendProbe::MAX_RESOURCE_BYTES;
const MAX_SCHEMA_TOTAL_BYTES: usize = SchemaBackendProbe::MAX_TOTAL_BYTES;
const MAX_SOURCE_FOUNDATION_CHECKS: usize = 65_536;
const MAX_SOURCE_FOUNDATION_CHUNKS: usize = 1_024;
const MAX_SOURCE_FOUNDATION_INSTANCE_BYTES: usize = 128 * 1024 * 1024;
const MAX_SOURCE_FOUNDATION_CPU_SECONDS: u64 = 3_600;
const SOURCE_FOUNDATION_PROFILE: FormatProfile = FormatProfile::LegacyPythonObserved20260923;

/// Root contracts explicitly selected by the maintained source-foundation
/// validator. This list is a route boundary: a record-plan or another ToS
/// schema family must add its own exact selector instead of borrowing this one.
pub const SOURCE_FOUNDATION_CONTRACT_PATHS: &[&str] = &[
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/source-link.schema.json",
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/source-resource-inventory.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/artifact-source-witness.schema.json",
    "ToS/contracts/artifact-source-witness-v2.schema.json",
    "ToS/contracts/artifact-visual-representation.schema.json",
    "ToS/contracts/scholarly-composite-witness.schema.json",
    "ToS/contracts/scholarly-composite-file-representation.schema.json",
    "ToS/contracts/provenance-event.schema.json",
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/object-link-claim.schema.json",
    "ToS/contracts/expression-derivation.schema.json",
    "ToS/contracts/provision-activity.schema.json",
    "ToS/contracts/first-publication-chronology.schema.json",
    "ToS/contracts/source-witness-catalog.schema.json",
    "ToS/contracts/source-anchor.schema.json",
    "ToS/contracts/source-anchor-v2.schema.json",
    "ToS/contracts/source-text-layer.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/semantic-annotation-packet-v2.schema.json",
    "ToS/contracts/translation-alignment-packet-v1.schema.json",
    "ToS/contracts/witness-text-collation-packet-v1.schema.json",
    "ToS/contracts/authored-route-evidence-bridge-v1.schema.json",
    "ToS/contracts/source-text-unit-packet-v1.schema.json",
    "ToS/contracts/collection-work-boundary-map.schema.json",
    "ToS/contracts/laboratory-sample-plan.schema.json",
    "ToS/contracts/ocr-visual-sample-plan.schema.json",
    "ToS/contracts/manual-gold-status.schema.json",
    "ToS/contracts/manual-gold-assurance.schema.json",
    "ToS/contracts/translation-sample-plan.schema.json",
    "ToS/contracts/translation-source-review-plan.schema.json",
    "ToS/contracts/translation-laboratory-plan.schema.json",
    "ToS/contracts/translation-exposure-aware-plan.schema.json",
    "ToS/contracts/translation-reference-register.schema.json",
    "ToS/contracts/translation-pre-draft-analysis.schema.json",
    "ToS/contracts/translation-packet.schema.json",
    "ToS/contracts/semantic-ladder-packet.schema.json",
    "ToS/contracts/golden-kernel-transfer-plan.schema.json",
    "ToS/contracts/source-gated-evaluation-plan.schema.json",
    "ToS/contracts/source-gated-semantic-evaluation-plan.schema.json",
    "ToS/contracts/source-gated-llm-evaluation-plan.schema.json",
    "ToS/contracts/material-discovery-record.schema.json",
    "ToS/contracts/access-request.schema.json",
    "ToS/contracts/server-import-contract.schema.json",
    "ToS/contracts/retrieval-query-plan.schema.json",
    "ToS/contracts/visual-retrieval-plan.schema.json",
    "ToS/contracts/graph-query-plan.schema.json",
    "ToS/contracts/private-laboratory-evidence-handoff.schema.json",
    "ToS/contracts/public-laboratory-evidence-derivative.schema.json",
    "ToS/contracts/manual-error-ledger-record.schema.json",
    "ToS/contracts/transfer-candidate-structural-crosswalk.schema.json",
    "ToS/contracts/hierarchical-target-numbered-unit-page-map.schema.json",
    "ToS/contracts/transfer-candidate-target-structural-crosswalk.schema.json",
    "ToS/contracts/german-assisted-source-review.schema.json",
    "ToS/contracts/private-transfer-source-visible-review-bundle.schema.json",
    "ToS/contracts/transfer-source-visible-review-receipt.schema.json",
    "ToS/contracts/critical-edition-witness-admission.schema.json",
    "ToS/contracts/critical-edition-citation-witness-decision.schema.json",
    "ToS/contracts/edition-reading-admission.schema.json",
    "ToS/contracts/german-source-triangulation.schema.json",
    "ToS/contracts/bounded-translation-research-input.schema.json",
    "ToS/contracts/experimental-translation-candidate.schema.json",
    "ToS/contracts/experimental-translation-episode.schema.json",
];

/// Caller-supplied ceilings for schema selection and one complete source
/// foundation diagnostics call. The worker's lower protocol caps remain
/// authoritative. This type has no hidden defaults so the source owner chooses
/// the complete finite envelope.
#[derive(Debug, Clone, Copy)]
pub struct SourceFoundationSchemaLimits {
    pub max_schema_resources: usize,
    pub max_schema_resource_bytes: usize,
    pub max_total_schema_bytes: usize,
    pub max_checks: usize,
    pub max_chunks: usize,
    pub max_total_cpu_seconds: u64,
    /// Per-instance source-input ceiling. Decoded finite inputs larger than
    /// the ordinary one-MiB probe limit use the opt-in selected finite
    /// diagnostics-v2 profile, capped by the existing 32-MiB batch limit.
    pub max_instance_bytes: usize,
    pub max_total_instance_bytes: usize,
    pub max_total_issues: usize,
    pub max_total_report_bytes: usize,
    /// One aggregate controller-side stdin+stdout byte ceiling across every
    /// diagnostics-v2 chunk in this source-foundation call.
    pub max_total_worker_wire_bytes: u64,
    pub batch: BatchBudget,
}

impl SourceFoundationSchemaLimits {
    pub fn validate(self) -> bool {
        let batch = self.batch;
        self.max_schema_resources > 0
            && self.max_schema_resources <= MAX_CONTRACTS
            && self.max_schema_resource_bytes > 0
            && self.max_schema_resource_bytes <= MAX_SCHEMA_RESOURCE_BYTES
            && self.max_total_schema_bytes > 0
            && self.max_total_schema_bytes <= MAX_SCHEMA_TOTAL_BYTES
            && self.max_checks > 0
            && self.max_checks <= MAX_SOURCE_FOUNDATION_CHECKS
            && self.max_chunks > 0
            && self.max_chunks <= MAX_SOURCE_FOUNDATION_CHUNKS
            && self
                .max_chunks
                .checked_mul(batch.max_units)
                .is_some_and(|capacity| self.max_checks <= capacity)
            && self.max_total_cpu_seconds > 0
            && self.max_total_cpu_seconds <= MAX_SOURCE_FOUNDATION_CPU_SECONDS
            && u64::try_from(self.max_chunks)
                .ok()
                .and_then(|chunks| chunks.checked_mul(batch.cpu_seconds))
                .is_some_and(|capacity| self.max_total_cpu_seconds <= capacity)
            && self.max_instance_bytes > 0
            && self.max_instance_bytes <= BatchBudget::MAX_RAW_BYTES
            && self.max_total_instance_bytes > 0
            && self.max_total_instance_bytes <= MAX_SOURCE_FOUNDATION_INSTANCE_BYTES
            && self
                .max_chunks
                .checked_mul(batch.max_total_raw_bytes)
                .is_some_and(|capacity| self.max_total_instance_bytes <= capacity)
            && self.max_total_issues > 0
            && self.max_total_issues
                <= self.max_checks * schema_diagnostics::MAX_ISSUES_PER_UNIT as usize
            && self.max_total_report_bytes > 0
            && self.max_total_report_bytes <= schema_diagnostics::MAX_RESPONSE_BYTES
            && self.max_total_worker_wire_bytes > 0
            && self.max_total_worker_wire_bytes < u64::MAX
            && batch.total_execution_wall > Duration::ZERO
            && batch.total_execution_wall <= Duration::from_secs(3600)
            && batch.startup_wall > Duration::ZERO
            && batch.startup_wall <= batch.total_execution_wall
            && batch.per_unit_wall > Duration::ZERO
            && batch.per_unit_wall <= batch.total_execution_wall
            && batch.cleanup_grace <= Duration::from_secs(1)
            && batch.cpu_seconds > 0
            && batch.cpu_seconds <= 3600
            && batch.address_space_bytes >= 64 * 1024 * 1024
            && batch.address_space_bytes <= 8 * 1024 * 1024 * 1024
            && batch.max_units > 0
            && batch.max_units <= BatchBudget::MAX_UNITS
            && batch.max_total_raw_bytes > 0
            && batch.max_total_raw_bytes <= BatchBudget::MAX_RAW_BYTES
    }

    /// Stable owner-envelope identity, carried beside the worker's diagnostics
    /// caps digest so a report cannot be detached from the caller's limits.
    pub fn digest(self) -> Digest256 {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-source-foundation-schema-limits-v2\0");
        for value in [
            self.max_schema_resources as u64,
            self.max_schema_resource_bytes as u64,
            self.max_total_schema_bytes as u64,
            self.max_checks as u64,
            self.max_chunks as u64,
            self.max_total_cpu_seconds,
            self.max_instance_bytes as u64,
            self.max_total_instance_bytes as u64,
            self.max_total_issues as u64,
            self.max_total_report_bytes as u64,
            self.max_total_worker_wire_bytes,
            batch_nanos(self.batch.total_execution_wall),
            batch_nanos(self.batch.startup_wall),
            batch_nanos(self.batch.per_unit_wall),
            batch_nanos(self.batch.cleanup_grace),
            self.batch.cpu_seconds,
            self.batch.address_space_bytes,
            self.batch.max_units as u64,
            self.batch.max_total_raw_bytes as u64,
        ] {
            hash.update(&value.to_be_bytes());
        }
        hash.finalize()
    }
}

fn batch_nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationSchemaLoadFailure {
    InvalidLimits,
    Deadline,
    Cancelled,
    CutRead,
    SchemaResource,
    SchemaSet,
    ContractSelection,
}

/// Exact source-cut schema resources and approved source-foundation root
/// contracts. The digest covers every selected ToS contract resource, including
/// resources used only through `$ref` by a root contract.
#[derive(Clone)]
pub struct SourceFoundationSchemaSet {
    source_revision: SourceRevision,
    profile: FormatProfile,
    resources: Vec<SchemaResource>,
    source_resources: Vec<SelectedSchemaResource>,
    contracts: BTreeMap<String, (String, Digest256)>,
    contract_selection_sha256: Digest256,
    schema_set_sha256: Digest256,
    limits_sha256: Digest256,
    schema_bytes: usize,
    catalog_entry_schema_present: bool,
    catalog_claim_entry_schema_present: bool,
    deadline: Instant,
}

#[derive(Clone)]
pub(crate) struct SelectedSchemaResource {
    pub(crate) path: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: Digest256,
}

/// Borrowed identity of one source member included in the complete selected
/// schema-resource closure. The path and fixity came from the same cut that
/// supplied the bytes parsed by `SourceFoundationSchemaSet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFoundationSelectedSchemaResource<'a> {
    pub path: &'a str,
    pub size_bytes: u64,
    pub sha256: Digest256,
}

fn selected_source_resource_metadata_state_from_members(
    members: &[(String, RelativePath, u64, Digest256)],
    descriptor_capacity: usize,
) -> Option<usize> {
    let slots = descriptor_capacity.checked_mul(std::mem::size_of::<SelectedSchemaResource>())?;
    let paths = members.iter().try_fold(0usize, |total, (path, _, _, _)| {
        total.checked_add(path.capacity())
    })?;
    slots.checked_add(paths)
}

pub(crate) fn selected_source_resource_metadata_state(
    resources: &[SelectedSchemaResource],
    descriptor_capacity: usize,
) -> Option<usize> {
    let slots = descriptor_capacity.checked_mul(std::mem::size_of::<SelectedSchemaResource>())?;
    let paths = resources.iter().try_fold(0usize, |total, resource| {
        total.checked_add(resource.path.capacity())
    })?;
    slots.checked_add(paths)
}

pub(crate) fn selected_source_resource_metadata_state_upper_bound(
    resource_count: usize,
) -> Option<usize> {
    let slots = resource_count.checked_mul(std::mem::size_of::<SelectedSchemaResource>())?;
    let paths = resource_count.checked_mul(MAX_LOCATION_BYTES)?;
    slots.checked_add(paths)
}

impl SourceFoundationSchemaSet {
    pub fn from_cut(
        cut: &CorpusCutReader,
        profile: FormatProfile,
        limits: SourceFoundationSchemaLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, SourceFoundationSchemaLoadFailure> {
        if !limits.validate() {
            return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
        }
        check_load_active(deadline, cancelled)?;
        if profile != SOURCE_FOUNDATION_PROFILE {
            return Err(SourceFoundationSchemaLoadFailure::SchemaSet);
        }
        let source_revision = cut.current().revision();
        let mut members = cut
            .current()
            .members()
            .filter(|member| {
                let path = member.path.as_str();
                path.starts_with(CONTRACT_PREFIX) && path.ends_with(CONTRACT_SUFFIX)
            })
            .map(|member| {
                (
                    member.path.as_str().to_owned(),
                    member.path.clone(),
                    member.size_bytes,
                    member.sha256,
                )
            })
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.0.cmp(&right.0));
        if members.is_empty() || members.len() > limits.max_schema_resources {
            return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
        }

        let expected_contracts = SOURCE_FOUNDATION_CONTRACT_PATHS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if expected_contracts.len() != SOURCE_FOUNDATION_CONTRACT_PATHS.len() {
            return Err(SourceFoundationSchemaLoadFailure::ContractSelection);
        }
        let mut resources = Vec::with_capacity(members.len());
        let mut source_resources = Vec::new();
        source_resources
            .try_reserve_exact(members.len())
            .map_err(|_| SourceFoundationSchemaLoadFailure::InvalidLimits)?;
        let mut contracts = BTreeMap::new();
        let mut total_bytes = 0usize;
        let mut catalog_entry_schema_present = false;
        let mut catalog_claim_entry_schema_present = false;
        for (path, relative, expected_size, expected_digest) in members {
            check_load_active(deadline, cancelled)?;
            let member = match cut.read_member(
                source_revision,
                &relative,
                limits.max_schema_resource_bytes as u64,
                deadline,
                cancelled,
            ) {
                Ok(member) => member,
                Err(_) => {
                    check_load_active(deadline, cancelled)?;
                    return Err(SourceFoundationSchemaLoadFailure::CutRead);
                }
            };
            if member.raw.len() as u64 != expected_size
                || Digest256::of_bytes(&member.raw) != expected_digest
                || member.raw.len() > limits.max_schema_resource_bytes
            {
                return Err(SourceFoundationSchemaLoadFailure::CutRead);
            }
            total_bytes = total_bytes
                .checked_add(member.raw.len())
                .filter(|used| *used <= limits.max_total_schema_bytes)
                .ok_or(SourceFoundationSchemaLoadFailure::InvalidLimits)?;
            let value: Value =
                crate::published_value(&member.raw, limits.max_schema_resource_bytes)
                    .map_err(|_| SourceFoundationSchemaLoadFailure::SchemaResource)?;
            if path == SOURCE_FOUNDATION_CATALOG_PATH {
                let definitions = value.get("$defs").and_then(Value::as_object);
                catalog_entry_schema_present = definitions
                    .and_then(|definitions| definitions.get("entry"))
                    .is_some_and(Value::is_object);
                catalog_claim_entry_schema_present = definitions
                    .and_then(|definitions| definitions.get("claim_entry"))
                    .is_some_and(Value::is_object);
            }
            let uri = source_foundation_schema_resource_uri(&value)
                .ok_or(SourceFoundationSchemaLoadFailure::SchemaResource)?
                .to_owned();
            let digest = Digest256::of_bytes(&member.raw);
            if expected_contracts.contains(path.as_str()) {
                contracts.insert(path.clone(), (uri.clone(), digest));
            }
            resources.push(SchemaResource {
                uri,
                raw: member.raw,
            });
            source_resources.push(SelectedSchemaResource {
                path,
                size_bytes: expected_size,
                sha256: expected_digest,
            });
        }
        if contracts.len() != expected_contracts.len()
            || expected_contracts
                .iter()
                .any(|path| !contracts.contains_key(*path))
        {
            return Err(SourceFoundationSchemaLoadFailure::ContractSelection);
        }
        check_load_active(deadline, cancelled)?;
        // Retain and bind exact resources even when their semantics are
        // outside the exceptional evaluator's closed subset; that evaluator
        // reports Indeterminate for such a check. General schema probes keep
        // their stricter SchemaBackendProbe keyword gate.
        let schema_set_sha256 = match schema_resource_set_digest(&resources) {
            Some(digest) => digest,
            None => {
                check_load_active(deadline, cancelled)?;
                return Err(SourceFoundationSchemaLoadFailure::SchemaSet);
            }
        };
        check_load_active(deadline, cancelled)?;
        Ok(Self {
            source_revision,
            profile,
            resources,
            source_resources,
            contracts,
            contract_selection_sha256: contract_selection_digest(&expected_contracts),
            schema_set_sha256,
            limits_sha256: limits.digest(),
            schema_bytes: total_bytes,
            catalog_entry_schema_present,
            catalog_claim_entry_schema_present,
            deadline,
        })
    }

    /// Select the same complete contract-resource closure from the bounded
    /// streamed source index. The cursor scans all current membership rows so
    /// selected schema resources cannot be omitted by an incomplete member
    /// view; only the finite contract metadata authorized by `limits` is held.
    pub fn from_streamed_cut(
        cut: &StreamedCorpusCutReaderV1,
        profile: FormatProfile,
        limits: SourceFoundationSchemaLimits,
        max_source_resource_metadata_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, SourceFoundationSchemaLoadFailure> {
        if !limits.validate() {
            return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
        }
        check_load_active(deadline, cancelled)?;
        if profile != SOURCE_FOUNDATION_PROFILE {
            return Err(SourceFoundationSchemaLoadFailure::SchemaSet);
        }
        let source_revision = cut.current_revision();
        let mut members = Vec::new();
        members
            .try_reserve_exact(limits.max_schema_resources)
            .map_err(|_| SourceFoundationSchemaLoadFailure::InvalidLimits)?;
        let row_limit = cut.manifest_row_byte_limit();
        check_load_active(deadline, cancelled)?;
        let current_revision = cut.revision_at(0);
        check_load_active(deadline, cancelled)?;
        let expected_member_count = current_revision
            .map_err(|_| SourceFoundationSchemaLoadFailure::CutRead)?
            .ok_or(SourceFoundationSchemaLoadFailure::CutRead)?
            .member_count;
        let mut after: Option<RelativePath> = None;
        let mut scanned = 0u64;
        loop {
            check_load_active(deadline, cancelled)?;
            let next = cut.member_after(source_revision, after.as_ref());
            check_load_active(deadline, cancelled)?;
            let Some(member) = next.map_err(|_| SourceFoundationSchemaLoadFailure::CutRead)? else {
                break;
            };
            scanned = scanned
                .checked_add(1)
                .ok_or(SourceFoundationSchemaLoadFailure::InvalidLimits)?;
            let path = member.path.as_str();
            if path.starts_with(CONTRACT_PREFIX) && path.ends_with(CONTRACT_SUFFIX) {
                if path.len() > MAX_LOCATION_BYTES || path.len() > row_limit {
                    return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
                }
                if members.len() >= limits.max_schema_resources {
                    return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
                }
                members.push((
                    path.to_owned(),
                    member.path.clone(),
                    member.size_bytes,
                    member.sha256,
                ));
            }
            after = Some(member.path);
        }
        if scanned != expected_member_count || members.is_empty() {
            return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
        }
        let requested_metadata_state =
            selected_source_resource_metadata_state_from_members(&members, members.len())
                .ok_or(SourceFoundationSchemaLoadFailure::InvalidLimits)?;
        if requested_metadata_state > max_source_resource_metadata_state_bytes {
            return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
        }

        let expected_contracts = SOURCE_FOUNDATION_CONTRACT_PATHS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if expected_contracts.len() != SOURCE_FOUNDATION_CONTRACT_PATHS.len() {
            return Err(SourceFoundationSchemaLoadFailure::ContractSelection);
        }
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(members.len())
            .map_err(|_| SourceFoundationSchemaLoadFailure::InvalidLimits)?;
        let mut source_resources = Vec::new();
        source_resources
            .try_reserve_exact(members.len())
            .map_err(|_| SourceFoundationSchemaLoadFailure::InvalidLimits)?;
        if selected_source_resource_metadata_state_from_members(
            &members,
            source_resources.capacity(),
        )
        .is_none_or(|state| state > max_source_resource_metadata_state_bytes)
        {
            return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
        }
        let mut contracts = BTreeMap::new();
        let mut total_bytes = 0usize;
        let mut catalog_entry_schema_present = false;
        let mut catalog_claim_entry_schema_present = false;
        for (path, relative, expected_size, expected_digest) in members {
            check_load_active(deadline, cancelled)?;
            if expected_size > limits.max_schema_resource_bytes as u64 {
                return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
            }
            let member = match cut.read_member(
                source_revision,
                &relative,
                limits.max_schema_resource_bytes as u64,
                deadline,
                cancelled,
            ) {
                Ok(member) => member,
                Err(_) => {
                    check_load_active(deadline, cancelled)?;
                    return Err(SourceFoundationSchemaLoadFailure::CutRead);
                }
            };
            if member.raw.len() as u64 != expected_size
                || Digest256::of_bytes(&member.raw) != expected_digest
                || member.raw.len() > limits.max_schema_resource_bytes
            {
                return Err(SourceFoundationSchemaLoadFailure::CutRead);
            }
            total_bytes = total_bytes
                .checked_add(member.raw.len())
                .filter(|used| *used <= limits.max_total_schema_bytes)
                .ok_or(SourceFoundationSchemaLoadFailure::InvalidLimits)?;
            let value: Value =
                crate::published_value(&member.raw, limits.max_schema_resource_bytes)
                    .map_err(|_| SourceFoundationSchemaLoadFailure::SchemaResource)?;
            if path == SOURCE_FOUNDATION_CATALOG_PATH {
                let definitions = value.get("$defs").and_then(Value::as_object);
                catalog_entry_schema_present = definitions
                    .and_then(|definitions| definitions.get("entry"))
                    .is_some_and(Value::is_object);
                catalog_claim_entry_schema_present = definitions
                    .and_then(|definitions| definitions.get("claim_entry"))
                    .is_some_and(Value::is_object);
            }
            let uri = source_foundation_schema_resource_uri(&value)
                .ok_or(SourceFoundationSchemaLoadFailure::SchemaResource)?
                .to_owned();
            let digest = Digest256::of_bytes(&member.raw);
            if expected_contracts.contains(path.as_str()) {
                contracts.insert(path.clone(), (uri.clone(), digest));
            }
            resources.push(SchemaResource {
                uri,
                raw: member.raw,
            });
            source_resources.push(SelectedSchemaResource {
                path,
                size_bytes: expected_size,
                sha256: expected_digest,
            });
        }
        if resources.len() > limits.max_schema_resources
            || contracts.len() != expected_contracts.len()
            || expected_contracts
                .iter()
                .any(|path| !contracts.contains_key(*path))
        {
            return Err(SourceFoundationSchemaLoadFailure::ContractSelection);
        }
        check_load_active(deadline, cancelled)?;
        let schema_set_sha256 = match schema_resource_set_digest(&resources) {
            Some(digest) => digest,
            None => {
                check_load_active(deadline, cancelled)?;
                return Err(SourceFoundationSchemaLoadFailure::SchemaSet);
            }
        };
        check_load_active(deadline, cancelled)?;
        Ok(Self {
            source_revision,
            profile,
            resources,
            source_resources,
            contracts,
            contract_selection_sha256: contract_selection_digest(&expected_contracts),
            schema_set_sha256,
            limits_sha256: limits.digest(),
            schema_bytes: total_bytes,
            catalog_entry_schema_present,
            catalog_claim_entry_schema_present,
            deadline,
        })
    }

    pub fn source_revision(&self) -> SourceRevision {
        self.source_revision
    }

    pub fn schema_set_sha256(&self) -> Digest256 {
        self.schema_set_sha256
    }

    pub fn contract_selection_sha256(&self) -> Digest256 {
        self.contract_selection_sha256
    }

    pub fn schema_bytes(&self) -> usize {
        self.schema_bytes
    }

    pub fn schema_resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Exact ascending source-member identity for every resource used to
    /// prepare this schema set. The selected raw bytes were size/hash checked
    /// against these same tuples before `SchemaBackendProbe` was constructed.
    pub fn source_resources(
        &self,
    ) -> impl Iterator<Item = SourceFoundationSelectedSchemaResource<'_>> {
        self.source_resources
            .iter()
            .map(|resource| SourceFoundationSelectedSchemaResource {
                path: &resource.path,
                size_bytes: resource.size_bytes,
                sha256: resource.sha256,
            })
    }

    /// Additional retained state from the exact source-resource cursor. The
    /// schema-set struct header is counted by its owner; this covers the
    /// vector capacity and each owned path allocation.
    pub fn source_resource_metadata_state_bytes(&self) -> Option<usize> {
        selected_source_resource_metadata_state(
            &self.source_resources,
            self.source_resources.capacity(),
        )
    }

    /// Digest of one selected root contract, if it belongs to this exact
    /// schema set. Fragments select within that same root resource.
    pub fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        let base = contract.split_once('#').map_or(contract, |(base, _)| base);
        self.contracts.get(base).map(|(_, digest)| *digest)
    }

    /// True when the selected catalog schema contains `$defs.entry` as an
    /// object, matching the maintained validator's conditional branch.
    pub fn catalog_entry_schema_present(&self) -> bool {
        self.catalog_entry_schema_present
    }

    /// True when the selected catalog schema contains `$defs.claim_entry` as
    /// an object, matching the maintained validator's conditional branch.
    pub fn catalog_claim_entry_schema_present(&self) -> bool {
        self.catalog_claim_entry_schema_present
    }

    pub fn limits_sha256(&self) -> Digest256 {
        self.limits_sha256
    }

    pub fn profile(&self) -> FormatProfile {
        self.profile
    }
}

/// Exact candidate-current schema resources selected through the same source
/// input and opaque fence as the candidate Records receiver. Unlike a retained
/// corpus cut this value has no `SourceRevision`; the caller's typed identity
/// remains the only candidate binding.
pub struct CandidateSourceFoundationSchemaSet<I> {
    pub(crate) input_identity: I,
    pub(crate) profile: FormatProfile,
    pub(crate) resources: Vec<SchemaResource>,
    pub(crate) source_resources: Vec<SelectedSchemaResource>,
    pub(crate) contracts: BTreeMap<String, (String, Digest256)>,
    pub(crate) contract_selection_sha256: Digest256,
    pub(crate) schema_set_sha256: Digest256,
    pub(crate) limits_sha256: Digest256,
    pub(crate) schema_bytes: usize,
    pub(crate) catalog_entry_schema_present: bool,
    pub(crate) catalog_claim_entry_schema_present: bool,
}

impl<I: Copy + Eq> CandidateSourceFoundationSchemaSet<I> {
    /// Scan the complete candidate membership once. Schema membership, raw
    /// bytes, metadata size and the final currentness fence all come from the
    /// same opaque source input; no accepted-base revision can enter this set.
    pub fn from_input(
        input: &dyn SourceCutInputWithIdentity<I>,
        profile: FormatProfile,
        limits: SourceFoundationSchemaLimits,
        max_source_resource_metadata_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, SourceFoundationSchemaLoadFailure> {
        if !limits.validate() {
            return Err(SourceFoundationSchemaLoadFailure::InvalidLimits);
        }
        check_load_active(deadline, cancelled)?;
        if profile != SOURCE_FOUNDATION_PROFILE {
            return Err(SourceFoundationSchemaLoadFailure::SchemaSet);
        }

        let input_identity = *input.input_identity();
        let source = input.source_input();
        let expected_contracts = SOURCE_FOUNDATION_CONTRACT_PATHS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if expected_contracts.len() != SOURCE_FOUNDATION_CONTRACT_PATHS.len() {
            return Err(SourceFoundationSchemaLoadFailure::ContractSelection);
        }

        let mut resources = Vec::new();
        let mut source_resources = Vec::new();
        let mut contracts = BTreeMap::new();
        let mut observed_members = 0u64;
        let mut observed_source_bytes = 0u64;
        let mut total_schema_bytes = 0usize;
        let mut catalog_entry_schema_present = false;
        let mut catalog_claim_entry_schema_present = false;
        let mut selection_failure = None;
        let coverage = source.for_each_current_member(deadline, cancelled, &mut |meta, raw| {
            if let Err(failure) = check_load_active(deadline, cancelled) {
                selection_failure = Some(failure);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            observed_members = match observed_members.checked_add(1) {
                Some(count) => count,
                None => {
                    selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                    return Err(crate::item_rules::ItemRefusal::Budget);
                }
            };
            let raw_size = match u64::try_from(raw.len()) {
                Ok(size) => size,
                Err(_) => {
                    selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                    return Err(crate::item_rules::ItemRefusal::Budget);
                }
            };
            observed_source_bytes = match observed_source_bytes.checked_add(raw_size) {
                Some(total) => total,
                None => {
                    selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                    return Err(crate::item_rules::ItemRefusal::Budget);
                }
            };
            if raw_size != meta.size_bytes {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::CutRead);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }

            if !meta.path.starts_with(CONTRACT_PREFIX) || !meta.path.ends_with(CONTRACT_SUFFIX) {
                return Ok(());
            }
            if meta.path.len() > MAX_LOCATION_BYTES
                || raw.len() > limits.max_schema_resource_bytes
                || source_resources.len() >= limits.max_schema_resources
            {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            if source_resources
                .last()
                .is_some_and(|previous: &SelectedSchemaResource| {
                    previous.path.as_str() >= meta.path
                })
            {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::ContractSelection);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            total_schema_bytes = match total_schema_bytes
                .checked_add(raw.len())
                .filter(|used| *used <= limits.max_total_schema_bytes)
            {
                Some(total) => total,
                None => {
                    selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                    return Err(crate::item_rules::ItemRefusal::Budget);
                }
            };

            let next_source_resource_count = match source_resources.len().checked_add(1) {
                Some(count) => count,
                None => {
                    selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                    return Err(crate::item_rules::ItemRefusal::Budget);
                }
            };
            let requested_descriptor_capacity =
                source_resources.capacity().max(next_source_resource_count);
            let requested_metadata_state = selected_source_resource_metadata_state(
                &source_resources,
                requested_descriptor_capacity,
            )
            .and_then(|state| state.checked_add(meta.path.len()));
            if requested_metadata_state
                .is_none_or(|state| state > max_source_resource_metadata_state_bytes)
            {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }

            let value = match crate::published_value(raw, limits.max_schema_resource_bytes) {
                Ok(value) => value,
                Err(_) => {
                    selection_failure = Some(SourceFoundationSchemaLoadFailure::SchemaResource);
                    return Err(crate::item_rules::ItemRefusal::Budget);
                }
            };
            if meta.path == SOURCE_FOUNDATION_CATALOG_PATH {
                let definitions = value.get("$defs").and_then(Value::as_object);
                catalog_entry_schema_present = definitions
                    .and_then(|definitions| definitions.get("entry"))
                    .is_some_and(Value::is_object);
                catalog_claim_entry_schema_present = definitions
                    .and_then(|definitions| definitions.get("claim_entry"))
                    .is_some_and(Value::is_object);
            }
            let uri = match source_foundation_schema_resource_uri(&value) {
                Some(uri) => uri.to_owned(),
                None => {
                    selection_failure = Some(SourceFoundationSchemaLoadFailure::SchemaResource);
                    return Err(crate::item_rules::ItemRefusal::Budget);
                }
            };
            let digest = Digest256::of_bytes(raw);
            let mut selected_path = String::new();
            if selected_path.try_reserve_exact(meta.path.len()).is_err() {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            selected_path.push_str(meta.path);
            if resources.try_reserve_exact(1).is_err()
                || source_resources.try_reserve_exact(1).is_err()
            {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            let actual_metadata_state = selected_source_resource_metadata_state(
                &source_resources,
                source_resources.capacity(),
            )
            .and_then(|state| state.checked_add(selected_path.capacity()));
            if actual_metadata_state
                .is_none_or(|state| state > max_source_resource_metadata_state_bytes)
            {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            if expected_contracts.contains(meta.path)
                && contracts
                    .insert(selected_path.clone(), (uri.clone(), digest))
                    .is_some()
            {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::ContractSelection);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            let mut resource_raw = Vec::new();
            if resource_raw.try_reserve_exact(raw.len()).is_err() {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            resource_raw.extend_from_slice(raw);
            resources.push(SchemaResource {
                uri,
                raw: resource_raw,
            });
            source_resources.push(SelectedSchemaResource {
                path: selected_path,
                size_bytes: meta.size_bytes,
                sha256: digest,
            });
            let metadata_state = selected_source_resource_metadata_state(
                &source_resources,
                source_resources.capacity(),
            );
            if metadata_state.is_none_or(|state| state > max_source_resource_metadata_state_bytes) {
                selection_failure = Some(SourceFoundationSchemaLoadFailure::InvalidLimits);
                return Err(crate::item_rules::ItemRefusal::Budget);
            }
            Ok(())
        });
        let coverage = match coverage {
            Ok(coverage) => coverage,
            Err(_) => {
                return Err(selection_failure.unwrap_or(SourceFoundationSchemaLoadFailure::CutRead));
            }
        };
        check_load_active(deadline, cancelled)?;
        source
            .verify_current_fence(&coverage, deadline, cancelled)
            .map_err(|_| SourceFoundationSchemaLoadFailure::CutRead)?;
        if input.input_identity() != &input_identity
            || coverage.member_count() != observed_members
            || coverage.source_bytes_read() != observed_source_bytes
        {
            return Err(SourceFoundationSchemaLoadFailure::CutRead);
        }
        if resources.is_empty()
            || contracts.len() != expected_contracts.len()
            || expected_contracts
                .iter()
                .any(|path| !contracts.contains_key(*path))
        {
            return Err(SourceFoundationSchemaLoadFailure::ContractSelection);
        }
        check_load_active(deadline, cancelled)?;
        let schema_set_sha256 = match schema_resource_set_digest(&resources) {
            Some(digest) => digest,
            None => {
                check_load_active(deadline, cancelled)?;
                return Err(SourceFoundationSchemaLoadFailure::SchemaSet);
            }
        };
        check_load_active(deadline, cancelled)?;
        Ok(Self {
            input_identity,
            profile,
            resources,
            source_resources,
            contracts,
            contract_selection_sha256: contract_selection_digest(&expected_contracts),
            schema_set_sha256,
            limits_sha256: limits.digest(),
            schema_bytes: total_schema_bytes,
            catalog_entry_schema_present,
            catalog_claim_entry_schema_present,
        })
    }

    pub fn input_identity(&self) -> &I {
        &self.input_identity
    }

    pub fn profile(&self) -> FormatProfile {
        self.profile
    }

    pub fn schema_set_sha256(&self) -> Digest256 {
        self.schema_set_sha256
    }

    pub fn contract_selection_sha256(&self) -> Digest256 {
        self.contract_selection_sha256
    }

    pub fn schema_bytes(&self) -> usize {
        self.schema_bytes
    }

    pub fn source_resource_count(&self) -> usize {
        self.resources.len()
    }

    pub fn source_resources(
        &self,
    ) -> impl Iterator<Item = SourceFoundationSelectedSchemaResource<'_>> {
        self.source_resources
            .iter()
            .map(|resource| SourceFoundationSelectedSchemaResource {
                path: &resource.path,
                size_bytes: resource.size_bytes,
                sha256: resource.sha256,
            })
    }

    pub fn source_resource_metadata_state_bytes(&self) -> Option<usize> {
        selected_source_resource_metadata_state(
            &self.source_resources,
            self.source_resources.capacity(),
        )
    }

    pub fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        let base = contract.split_once('#').map_or(contract, |(base, _)| base);
        self.contracts.get(base).map(|(_, digest)| *digest)
    }

    pub fn limits_sha256(&self) -> Digest256 {
        self.limits_sha256
    }

    pub fn catalog_entry_schema_present(&self) -> bool {
        self.catalog_entry_schema_present
    }

    pub fn catalog_claim_entry_schema_present(&self) -> bool {
        self.catalog_claim_entry_schema_present
    }

    pub(crate) fn resources(&self) -> &[SchemaResource] {
        &self.resources
    }

    pub(crate) fn contracts(&self) -> &BTreeMap<String, (String, Digest256)> {
        &self.contracts
    }
}

/// One caller-owned decoded instance. `location` is the stable source label to
/// which the Python-compatible path suffix is appended; no instance value is
/// copied into diagnostics or emitted from the worker.
#[derive(Clone, Copy)]
pub struct SourceFoundationSchemaInput<'a> {
    pub location: &'a str,
    pub contract: &'a str,
    pub decoded_instance: &'a Value,
}

/// One source-owned raw JSON instance for the explicitly selected legacy
/// Python observed decoder. Bytes are retained unchanged in the diagnostic
/// unit and are never copied into issue text.
#[derive(Clone, Copy)]
pub struct SourceFoundationLegacySchemaInput<'a> {
    pub location: &'a str,
    pub contract: &'a str,
    pub raw_instance: &'a [u8],
}

/// One encounter-ordered finite or raw legacy unit for a single diagnostics-v2
/// batch. The variant selects only the input representation; schema selection,
/// exact worker, budgets and report binding remain shared.
#[derive(Clone, Copy)]
pub enum SourceFoundationMixedSchemaInput<'a> {
    Decoded(SourceFoundationSchemaInput<'a>),
    Legacy(SourceFoundationLegacySchemaInput<'a>),
}

/// The three source-witness-catalog schema targets used by the maintained
/// source-foundation validator. This closed selector intentionally does not
/// accept arbitrary JSON Pointers or fragment URIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceFoundationCatalogSchemaTarget {
    Manifest,
    Entry,
    ClaimEntry,
}

impl SourceFoundationCatalogSchemaTarget {
    const fn selector(self) -> &'static str {
        match self {
            Self::Manifest => "catalog_manifest",
            Self::Entry => "catalog_entry",
            Self::ClaimEntry => "catalog_claim_entry",
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::Manifest => 1,
            Self::Entry => 2,
            Self::ClaimEntry => 3,
        }
    }
}

/// One decoded catalog instance selected through the closed maintained
/// catalog-schema target set.
#[derive(Clone, Copy)]
pub struct SourceFoundationCatalogSchemaInput<'a> {
    pub location: &'a str,
    pub target: SourceFoundationCatalogSchemaTarget,
    pub decoded_instance: &'a Value,
}

trait SourceFoundationSchemaCheckInput {
    fn location(&self) -> &str;
    fn contract_key(&self) -> &str;
    fn report_contract(&self) -> &str;
    fn instance(&self) -> SourceFoundationSchemaInstance<'_>;
    fn input_profile(&self) -> SourceFoundationInputProfile;
}

#[derive(Clone, Copy)]
enum SourceFoundationSchemaInstance<'a> {
    Decoded(&'a Value),
    LegacyPythonRaw(&'a [u8]),
}

impl SourceFoundationSchemaCheckInput for SourceFoundationSchemaInput<'_> {
    fn location(&self) -> &str {
        self.location
    }

    fn contract_key(&self) -> &str {
        self.contract
    }

    fn report_contract(&self) -> &str {
        self.contract
    }

    fn instance(&self) -> SourceFoundationSchemaInstance<'_> {
        SourceFoundationSchemaInstance::Decoded(self.decoded_instance)
    }

    fn input_profile(&self) -> SourceFoundationInputProfile {
        SourceFoundationInputProfile::FiniteJson
    }
}

impl SourceFoundationSchemaCheckInput for SourceFoundationLegacySchemaInput<'_> {
    fn location(&self) -> &str {
        self.location
    }

    fn contract_key(&self) -> &str {
        self.contract
    }

    fn report_contract(&self) -> &str {
        self.contract
    }

    fn instance(&self) -> SourceFoundationSchemaInstance<'_> {
        SourceFoundationSchemaInstance::LegacyPythonRaw(self.raw_instance)
    }

    fn input_profile(&self) -> SourceFoundationInputProfile {
        SourceFoundationInputProfile::LegacyPythonObserved
    }
}

impl SourceFoundationSchemaCheckInput for SourceFoundationCatalogSchemaInput<'_> {
    fn location(&self) -> &str {
        self.location
    }

    fn contract_key(&self) -> &str {
        self.target.selector()
    }

    fn report_contract(&self) -> &str {
        self.target.selector()
    }

    fn instance(&self) -> SourceFoundationSchemaInstance<'_> {
        SourceFoundationSchemaInstance::Decoded(self.decoded_instance)
    }

    fn input_profile(&self) -> SourceFoundationInputProfile {
        SourceFoundationInputProfile::FiniteJson
    }
}

impl SourceFoundationSchemaCheckInput for SourceFoundationMixedSchemaInput<'_> {
    fn location(&self) -> &str {
        match self {
            Self::Decoded(input) => input.location,
            Self::Legacy(input) => input.location,
        }
    }

    fn contract_key(&self) -> &str {
        match self {
            Self::Decoded(input) => input.contract,
            Self::Legacy(input) => input.contract,
        }
    }

    fn report_contract(&self) -> &str {
        self.contract_key()
    }

    fn instance(&self) -> SourceFoundationSchemaInstance<'_> {
        match self {
            Self::Decoded(input) => SourceFoundationSchemaInstance::Decoded(input.decoded_instance),
            Self::Legacy(input) => {
                SourceFoundationSchemaInstance::LegacyPythonRaw(input.raw_instance)
            }
        }
    }

    fn input_profile(&self) -> SourceFoundationInputProfile {
        match self {
            Self::Decoded(_) => SourceFoundationInputProfile::FiniteJson,
            Self::Legacy(_) => SourceFoundationInputProfile::LegacyPythonObserved,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationSchemaFailure {
    InvalidLimits,
    Deadline,
    Cancelled,
    InvalidLocation,
    ContractNotSelected,
    InputProfileMismatch,
    CatalogSchemaSelection,
    InputBudget,
    Worker(ExecutorFailure),
    IncompleteDiagnostic,
    TruncatedDiagnostics,
    DiagnosticReportBudget,
    DiagnosticBinding,
    UnsupportedInputSemantics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationSchemaIssue {
    pub location: String,
    pub schema_keyword: String,
    pub reason: schema_diagnostics::Reason,
    /// Closed static prose, except the two exact compatibility messages below.
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationSchemaCheckReport {
    pub chunk_index: usize,
    pub location: String,
    pub contract: String,
    pub diagnostic: schema_diagnostics::Report,
    pub issues: Vec<SourceFoundationSchemaIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceFoundationSchemaCost {
    pub checks: usize,
    /// Sum of configured per-child CPU ceilings, not measured CPU use.
    pub worker_cpu_budget_seconds: u64,
    /// Sum of actual wait4 user+system CPU observations so far. It is partial
    /// on incomplete reports and complete only when every checkpoint is known;
    /// `None` means no usage observation exists, not zero CPU.
    pub worker_cpu_micros: Option<u64>,
    pub metadata_bytes: usize,
    /// Input payload bytes retained for worker units: canonical compact bytes
    /// for decoded inputs and the exact borrowed source-byte length for the
    /// legacy raw lane.
    pub decoded_instance_bytes: usize,
    pub diagnostic_issues: usize,
    pub estimated_report_bytes: usize,
    pub schema_resource_bytes: usize,
    /// Exact aggregate bytes written to diagnostics-worker stdin.
    pub worker_request_bytes: u64,
    /// Exact aggregate bytes received from diagnostics-worker stdout,
    /// including protocol acknowledgements and final records.
    pub worker_response_bytes: u64,
    /// Checked sum of `worker_request_bytes` and `worker_response_bytes`.
    pub worker_wire_bytes: u64,
}

/// The source revision, worker, complete selected schema set, caller limits,
/// worker caps, ordered manifest, result stream, and observed worker transport
/// and CPU costs are bound into this report. A report is local schema evidence
/// only, never source admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationSchemaReport {
    pub source_revision: SourceRevision,
    pub profile: FormatProfile,
    pub worker_sha256: Digest256,
    pub schema_set_sha256: Digest256,
    pub contract_selection_sha256: Digest256,
    pub limits_sha256: Digest256,
    pub caps_sha256: Digest256,
    /// Selected whole-call CPU ceiling, in microseconds.
    pub max_total_cpu_micros: u64,
    /// Largest per-child CPU ceiling selected by the caller, in seconds.
    pub max_child_cpu_seconds: u64,
    /// The exact whole-call transport ceiling represented by `limits_sha256`.
    pub max_total_worker_wire_bytes: u64,
    /// Initial mixed profile-2 exceptional budget; `None` for unchanged
    /// finite-only and raw-only diagnostics profiles.
    pub exceptional_schema_budget: Option<ExceptionalSchemaUsage>,
    pub checkpoints: Vec<SchemaDiagnosticsCheckpoint>,
    pub binding_sha256: Option<Digest256>,
    pub expected_check_count: usize,
    pub checks: Vec<SourceFoundationSchemaCheckReport>,
    pub cost: SourceFoundationSchemaCost,
    exchange_failure_context: Option<ExchangeFailureContext>,
}

impl SourceFoundationSchemaReport {
    /// Transport boundary and natural child status for an incomplete worker
    /// exchange, without paths, payloads, or child diagnostic text.
    pub fn exchange_failure_context(&self) -> Option<ExchangeFailureContext> {
        self.exchange_failure_context
    }

    pub fn is_complete(&self) -> bool {
        if self.exchange_failure_context.is_some()
            || self
                .exceptional_schema_budget
                .is_some_and(|budget| budget != ExceptionalSchemaUsage::whole())
        {
            return false;
        }
        if self.checkpoints.is_empty()
            || self.checkpoints.len() > MAX_SOURCE_FOUNDATION_CHUNKS
            || self.expected_check_count == 0
            || self.expected_check_count > MAX_SOURCE_FOUNDATION_CHECKS
            || self.checks.len() != self.expected_check_count
            || self.cost.checks != self.expected_check_count
            || self.max_total_cpu_micros == 0
            || self.max_total_cpu_micros
                > MAX_SOURCE_FOUNDATION_CPU_SECONDS.saturating_mul(1_000_000)
            || self.max_child_cpu_seconds == 0
            || self.max_child_cpu_seconds > MAX_SOURCE_FOUNDATION_CPU_SECONDS
            || self
                .cost
                .worker_cpu_micros
                .is_none_or(|micros| micros > self.max_total_cpu_micros)
            || self.cost.decoded_instance_bytes > MAX_SOURCE_FOUNDATION_INSTANCE_BYTES
            || self.cost.metadata_bytes > schema_diagnostics::MAX_RESPONSE_BYTES
            || self.cost.estimated_report_bytes > schema_diagnostics::MAX_RESPONSE_BYTES
            || self.cost.schema_resource_bytes > MAX_SCHEMA_TOTAL_BYTES
            || self.max_total_worker_wire_bytes == 0
            || self.max_total_worker_wire_bytes == u64::MAX
            || self.cost.worker_request_bytes == 0
            || self.cost.worker_response_bytes == 0
            || self.cost.worker_wire_bytes == 0
            || self
                .cost
                .worker_request_bytes
                .checked_add(self.cost.worker_response_bytes)
                != Some(self.cost.worker_wire_bytes)
            || self.cost.worker_wire_bytes > self.max_total_worker_wire_bytes
            || self
                .cost
                .metadata_bytes
                .checked_add(self.cost.estimated_report_bytes)
                .is_none_or(|bytes| bytes > schema_diagnostics::MAX_RESPONSE_BYTES)
        {
            return false;
        }
        let Some(checkpoint_count) = u64::try_from(self.checkpoints.len()).ok() else {
            return false;
        };
        let Some(max_cpu_budget_seconds) = checkpoint_count.checked_mul(self.max_child_cpu_seconds)
        else {
            return false;
        };
        if self.cost.worker_cpu_budget_seconds < checkpoint_count
            || self.cost.worker_cpu_budget_seconds > max_cpu_budget_seconds
        {
            return false;
        }
        let Some(issue_count) = self.checks.iter().try_fold(0usize, |count, check| {
            count.checked_add(check.diagnostic.issues.len())
        }) else {
            return false;
        };
        if issue_count != self.cost.diagnostic_issues {
            return false;
        }
        let mut offset = 0usize;
        let mut exceptional_remaining = self.exceptional_schema_budget;
        let mut worker_request_bytes = 0u64;
        let mut worker_response_bytes = 0u64;
        let mut worker_cpu_micros = 0u64;
        for (chunk_index, checkpoint) in self.checkpoints.iter().enumerate() {
            let chunk_count = checkpoint.completed_count as usize;
            let Some(end) = offset.checked_add(chunk_count) else {
                return false;
            };
            let exceptional_binding_valid = match (
                exceptional_remaining,
                checkpoint.exceptional_remaining,
                checkpoint.exceptional_usage,
            ) {
                (Some(remaining), Some(requested), Some(usage)) if remaining == requested => {
                    match remaining.checked_sub(usage) {
                        Some(next) => {
                            exceptional_remaining = Some(next);
                            true
                        }
                        None => false,
                    }
                }
                (None, None, None) => true,
                _ => false,
            };
            let Some(next_request_bytes) =
                worker_request_bytes.checked_add(checkpoint.worker_request_bytes)
            else {
                return false;
            };
            let Some(next_response_bytes) =
                worker_response_bytes.checked_add(checkpoint.worker_response_bytes)
            else {
                return false;
            };
            let Some(checkpoint_cpu_micros) = checkpoint.worker_cpu_micros else {
                return false;
            };
            let Some(next_cpu_micros) = worker_cpu_micros.checked_add(checkpoint_cpu_micros) else {
                return false;
            };
            if chunk_count == 0
                || chunk_count > BatchBudget::MAX_UNITS
                || end > self.checks.len()
                || checkpoint.worker_sha256 != self.worker_sha256
                || checkpoint.profile != self.profile
                || checkpoint.schema_set_sha256 != self.schema_set_sha256
                || checkpoint.caps_sha256 != self.caps_sha256
                || checkpoint.worker_request_bytes == 0
                || checkpoint.worker_response_bytes == 0
                || next_cpu_micros > self.max_total_cpu_micros
                || !exceptional_binding_valid
                || self.checks[offset..end].iter().any(|check| {
                    check.chunk_index != chunk_index
                        || !diagnostic_is_well_formed(
                            &check.diagnostic,
                            self.worker_sha256,
                            checkpoint.request_sha256,
                            self.schema_set_sha256,
                            self.caps_sha256,
                        )
                        || !matches!(
                            check.diagnostic.status,
                            schema_diagnostics::Status::Valid | schema_diagnostics::Status::Invalid
                        )
                        || check
                            .diagnostic
                            .issues
                            .windows(2)
                            .any(|pair| pair[0] > pair[1])
                        || check.issues.len() != check.diagnostic.issues.len()
                        || check
                            .issues
                            .iter()
                            .zip(source_foundation_schema_issues(
                                &check.location,
                                &check.diagnostic.issues,
                            ))
                            .any(|(actual, expected)| actual != &expected)
                })
                || checkpoint.ordered_manifest_sha256 != manifest_digest(&self.checks[offset..end])
            {
                return false;
            }
            worker_request_bytes = next_request_bytes;
            worker_response_bytes = next_response_bytes;
            worker_cpu_micros = next_cpu_micros;
            offset = end;
        }
        offset == self.expected_check_count
            && worker_request_bytes == self.cost.worker_request_bytes
            && worker_response_bytes == self.cost.worker_response_bytes
            && Some(worker_cpu_micros) == self.cost.worker_cpu_micros
            && self.binding_sha256 == Some(binding_digest(self))
    }

    /// True only for a fully covered batch in which every checked schema
    /// report is valid. Invalid, truncated, rejected or incomplete reports
    /// can never pass this predicate.
    pub fn is_valid(&self) -> bool {
        self.is_complete() && self.checks.iter().all(|check| check.diagnostic.is_valid())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFoundationSchemaOutcome {
    Complete(SourceFoundationSchemaReport),
    Incomplete {
        report: SourceFoundationSchemaReport,
        reason: SourceFoundationSchemaFailure,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceFoundationInputProfile {
    FiniteJson,
    LegacyPythonObserved,
    Mixed,
}

/// Evaluate one source-foundation diagnostics call against an exact source
/// cut. The function may use bounded protocol batches, all under one caller
/// deadline/cancellation flag and the aggregate limits below. `BatchBudget`
/// supplies each disposable worker's wall/CPU/address-space envelope.
pub fn evaluate_source_foundation_schema_checks(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceFoundationSchemaOutcome {
    evaluate_source_foundation_schema_checks_with_optional_quota(
        schema_set, worker, checks, limits, deadline, cancelled, None, None,
    )
}

/// Finite-input variant attached to the same invocation-wide quota used by
/// other explicitly selected diagnostics-v2 schema lanes.
pub fn evaluate_source_foundation_schema_checks_with_shared_quota(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_schema_checks_with_optional_quota(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        None,
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

/// Finite-input source-foundation check using the invocation's shared quota
/// and already verified immutable worker image.
pub fn evaluate_source_foundation_schema_checks_with_shared_quota_and_image(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
    image: &VerifiedWorkerImageHandle,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_schema_checks_with_optional_quota(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        Some(image),
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

fn evaluate_source_foundation_schema_checks_with_optional_quota(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: Option<&SharedSchemaWorkerQuota>,
    image: Option<&VerifiedWorkerImageHandle>,
) -> SourceFoundationSchemaOutcome {
    let selection = SchemaEvaluationSelection {
        resources: &schema_set.resources,
        contracts: &schema_set.contracts,
        schema_set_sha256: schema_set.schema_set_sha256,
        contract_selection_sha256: schema_set.contract_selection_sha256,
        schema_bytes: schema_set.schema_bytes,
    };
    evaluate_source_foundation_schema_checks_inner(
        schema_set,
        &selection,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        SourceFoundationInputProfile::FiniteJson,
        quota,
        image,
    )
}

/// Evaluate raw legacy-Python observed instances against the exact selected
/// source-foundation contracts. The diagnostics-v2 request binds the original
/// bytes and closed input-profile marker; unsupported Python-only values yield
/// an incomplete report and are never reduced to schema-valid/schema-invalid.
pub fn evaluate_source_foundation_legacy_schema_checks(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationLegacySchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceFoundationSchemaOutcome {
    evaluate_source_foundation_legacy_schema_checks_with_optional_quota(
        schema_set, worker, checks, limits, deadline, cancelled, None, None,
    )
}

/// Raw LegacyPythonObserved variant attached to the shared invocation quota.
pub fn evaluate_source_foundation_legacy_schema_checks_with_shared_quota(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationLegacySchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_legacy_schema_checks_with_optional_quota(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        None,
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

/// Raw LegacyPythonObserved checks sharing both quota and sealed worker image.
pub fn evaluate_source_foundation_legacy_schema_checks_with_shared_quota_and_image(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationLegacySchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
    image: &VerifiedWorkerImageHandle,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_legacy_schema_checks_with_optional_quota(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        Some(image),
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

fn evaluate_source_foundation_legacy_schema_checks_with_optional_quota(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationLegacySchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: Option<&SharedSchemaWorkerQuota>,
    image: Option<&VerifiedWorkerImageHandle>,
) -> SourceFoundationSchemaOutcome {
    let selection = SchemaEvaluationSelection {
        resources: &schema_set.resources,
        contracts: &schema_set.contracts,
        schema_set_sha256: schema_set.schema_set_sha256,
        contract_selection_sha256: schema_set.contract_selection_sha256,
        schema_bytes: schema_set.schema_bytes,
    };
    evaluate_source_foundation_schema_checks_inner(
        schema_set,
        &selection,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        SourceFoundationInputProfile::LegacyPythonObserved,
        quota,
        image,
    )
}

/// Evaluate finite decoded and raw LegacyPythonObserved checks in one ordered
/// diagnostics-v2 request. The unit mode is part of the request digest; finite
/// members keep the published finite serializer and raw members retain their
/// exact borrowed bytes.
pub fn evaluate_source_foundation_mixed_schema_checks(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationMixedSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceFoundationSchemaOutcome {
    evaluate_source_foundation_mixed_schema_checks_with_optional_quota(
        schema_set, worker, checks, limits, deadline, cancelled, None, None,
    )
}

/// Ordered mixed finite/raw variant attached to the shared invocation quota.
pub fn evaluate_source_foundation_mixed_schema_checks_with_shared_quota(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationMixedSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_mixed_schema_checks_with_optional_quota(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        None,
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

/// Ordered mixed finite/raw checks sharing both quota and sealed worker image.
pub fn evaluate_source_foundation_mixed_schema_checks_with_shared_quota_and_image(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationMixedSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
    image: &VerifiedWorkerImageHandle,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_mixed_schema_checks_with_optional_quota(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        Some(image),
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

fn evaluate_source_foundation_mixed_schema_checks_with_optional_quota(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationMixedSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: Option<&SharedSchemaWorkerQuota>,
    image: Option<&VerifiedWorkerImageHandle>,
) -> SourceFoundationSchemaOutcome {
    let selection = SchemaEvaluationSelection {
        resources: &schema_set.resources,
        contracts: &schema_set.contracts,
        schema_set_sha256: schema_set.schema_set_sha256,
        contract_selection_sha256: schema_set.contract_selection_sha256,
        schema_bytes: schema_set.schema_bytes,
    };
    evaluate_source_foundation_schema_checks_inner(
        schema_set,
        &selection,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        SourceFoundationInputProfile::Mixed,
        quota,
        image,
    )
}

/// Evaluate the maintained source-witness catalog manifest, record-entry,
/// and claim-entry schema targets through the same bounded diagnostics-v2
/// worker kernel as ordinary source-foundation contract checks.
pub fn evaluate_source_foundation_catalog_schema_checks(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationCatalogSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceFoundationSchemaOutcome {
    evaluate_source_foundation_catalog_schema_checks_inner(
        schema_set, worker, checks, limits, deadline, cancelled, None, None,
    )
}

/// Catalog-tail variant attached to the invocation quota shared with record,
/// item, layer, and ordinary source-foundation schema workers.
pub fn evaluate_source_foundation_catalog_schema_checks_with_shared_quota(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationCatalogSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_catalog_schema_checks_inner(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        None,
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

/// Catalog-tail checks sharing the invocation quota and exact sealed image.
pub fn evaluate_source_foundation_catalog_schema_checks_with_shared_quota_and_image(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationCatalogSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: &SharedSchemaWorkerQuota,
    image: &VerifiedWorkerImageHandle,
) -> SourceFoundationSchemaOutcome {
    let outcome = evaluate_source_foundation_catalog_schema_checks_inner(
        schema_set,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        Some(quota),
        Some(image),
    );
    if matches!(&outcome, SourceFoundationSchemaOutcome::Incomplete { .. }) {
        quota.poison();
    }
    outcome
}

fn evaluate_source_foundation_catalog_schema_checks_inner(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    checks: &[SourceFoundationCatalogSchemaInput<'_>],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    quota: Option<&SharedSchemaWorkerQuota>,
    image: Option<&VerifiedWorkerImageHandle>,
) -> SourceFoundationSchemaOutcome {
    if image.is_some() && quota.is_none() {
        let report = empty_schema_report(
            schema_set,
            worker,
            checks.len(),
            limits,
            schema_set.schema_set_sha256,
            schema_set.contract_selection_sha256,
            schema_set.schema_bytes,
        );
        return incomplete(report, SourceFoundationSchemaFailure::InvalidLimits);
    }
    if let Some(image) = image
        && (worker.sha256 != image.identity().sha256
            || worker.absolute_path != image.identity().absolute_path
            || deadline > image.operation_deadline())
    {
        let report = empty_schema_report(
            schema_set,
            worker,
            checks.len(),
            limits,
            schema_set.schema_set_sha256,
            schema_set.contract_selection_sha256,
            schema_set.schema_bytes,
        );
        return incomplete(
            report,
            SourceFoundationSchemaFailure::Worker(ExecutorFailure::WorkerIdentity),
        );
    }
    if !limits.validate()
        || limits.digest() != schema_set.limits_sha256
        || deadline > schema_set.deadline
        || checks.is_empty()
        || checks.len() > limits.max_checks
    {
        let report = empty_schema_report(
            schema_set,
            worker,
            checks.len(),
            limits,
            schema_set.schema_set_sha256,
            schema_set.contract_selection_sha256,
            schema_set.schema_bytes,
        );
        return incomplete(report, SourceFoundationSchemaFailure::InvalidLimits);
    }
    if let Err(reason) = check_active(deadline, cancelled) {
        let report = empty_schema_report(
            schema_set,
            worker,
            checks.len(),
            limits,
            schema_set.schema_set_sha256,
            schema_set.contract_selection_sha256,
            schema_set.schema_bytes,
        );
        return incomplete(report, reason);
    }

    let mut targets = BTreeSet::new();
    for check in checks {
        if let Err(reason) = check_active(deadline, cancelled) {
            let report = empty_schema_report(
                schema_set,
                worker,
                checks.len(),
                limits,
                schema_set.schema_set_sha256,
                schema_set.contract_selection_sha256,
                schema_set.schema_bytes,
            );
            return incomplete(report, reason);
        }
        targets.insert(check.target);
    }
    let catalog_selection =
        match derive_catalog_schema_selection(schema_set, &targets, limits, deadline, cancelled) {
            Ok(selection) => selection,
            Err(reason) => {
                let report = empty_schema_report(
                    schema_set,
                    worker,
                    checks.len(),
                    limits,
                    schema_set.schema_set_sha256,
                    schema_set.contract_selection_sha256,
                    schema_set.schema_bytes,
                );
                return incomplete(report, reason);
            }
        };

    let selected_resources = if catalog_selection.derived_resources.is_empty() {
        None
    } else {
        let mut resources = Vec::with_capacity(
            schema_set
                .resources
                .len()
                .saturating_add(catalog_selection.derived_resources.len()),
        );
        resources.extend(schema_set.resources.iter().cloned());
        resources.extend(catalog_selection.derived_resources);
        Some(resources)
    };
    let resources = selected_resources
        .as_deref()
        .unwrap_or(&schema_set.resources);
    let schema_set_sha256 = match schema_resource_set_digest(resources) {
        Some(digest) => digest,
        None => {
            let report = empty_schema_report(
                schema_set,
                worker,
                checks.len(),
                limits,
                schema_set.schema_set_sha256,
                schema_set.contract_selection_sha256,
                schema_set.schema_bytes,
            );
            return incomplete(
                report,
                SourceFoundationSchemaFailure::CatalogSchemaSelection,
            );
        }
    };
    let schema_bytes = resources
        .iter()
        .try_fold(0usize, |total, resource| {
            total.checked_add(resource.raw.len())
        })
        .unwrap_or(usize::MAX);
    if resources.len() > limits.max_schema_resources || schema_bytes > limits.max_total_schema_bytes
    {
        let report = empty_schema_report(
            schema_set,
            worker,
            checks.len(),
            limits,
            schema_set.schema_set_sha256,
            schema_set.contract_selection_sha256,
            schema_set.schema_bytes,
        );
        return incomplete(
            report,
            SourceFoundationSchemaFailure::CatalogSchemaSelection,
        );
    }
    let contract_selection_sha256 = catalog_target_selection_digest(
        schema_set.contract_selection_sha256,
        &catalog_selection.bindings,
    );
    let selection = SchemaEvaluationSelection {
        resources,
        contracts: &catalog_selection.contracts,
        schema_set_sha256,
        contract_selection_sha256,
        schema_bytes,
    };
    evaluate_source_foundation_schema_checks_inner(
        schema_set,
        &selection,
        worker,
        checks,
        limits,
        deadline,
        cancelled,
        SourceFoundationInputProfile::FiniteJson,
        quota,
        image,
    )
}

struct CatalogSchemaTargetBinding {
    target: SourceFoundationCatalogSchemaTarget,
    root_uri: String,
    source_schema_sha256: Digest256,
    target_schema_sha256: Digest256,
}

struct CatalogSchemaSelection {
    contracts: BTreeMap<String, (String, Digest256)>,
    derived_resources: Vec<SchemaResource>,
    bindings: Vec<CatalogSchemaTargetBinding>,
}

fn derive_catalog_schema_selection(
    schema_set: &SourceFoundationSchemaSet,
    targets: &BTreeSet<SourceFoundationCatalogSchemaTarget>,
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CatalogSchemaSelection, SourceFoundationSchemaFailure> {
    check_active(deadline, cancelled)?;
    if targets.is_empty() {
        return Err(SourceFoundationSchemaFailure::InvalidLimits);
    }
    let (catalog_uri, catalog_sha256) = schema_set
        .contracts
        .get(SOURCE_FOUNDATION_CATALOG_PATH)
        .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
    let catalog_resource = schema_set
        .resources
        .iter()
        .find(|resource| resource.uri == *catalog_uri)
        .filter(|resource| Digest256::of_bytes(&resource.raw) == *catalog_sha256)
        .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
    if catalog_resource.raw.len() > limits.max_schema_resource_bytes {
        return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
    }
    let derived_capacity = targets
        .iter()
        .filter(|target| **target == SourceFoundationCatalogSchemaTarget::ClaimEntry)
        .count();
    let effective_resource_count = schema_set
        .resources
        .len()
        .checked_add(derived_capacity)
        .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
    let derived_byte_budget = limits
        .max_total_schema_bytes
        .checked_sub(schema_set.schema_bytes)
        .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
    if effective_resource_count > limits.max_schema_resources
        || effective_resource_count > SchemaBackendProbe::MAX_RESOURCES
        || (derived_capacity > 0 && derived_byte_budget == 0)
    {
        return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
    }
    let catalog_schema = if targets
        .iter()
        .any(|target| *target != SourceFoundationCatalogSchemaTarget::Manifest)
    {
        let schema =
            crate::published_value(&catalog_resource.raw, limits.max_schema_resource_bytes)
                .map_err(|_| SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
        check_active(deadline, cancelled)?;
        if schema.get("$schema").and_then(Value::as_str)
            != Some("https://json-schema.org/draft/2020-12/schema")
        {
            return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
        }
        Some(schema)
    } else {
        None
    };

    let mut selection = CatalogSchemaSelection {
        contracts: BTreeMap::new(),
        derived_resources: Vec::with_capacity(derived_capacity),
        bindings: Vec::with_capacity(targets.len()),
    };
    let mut derived_total_bytes = 0usize;
    for target in targets {
        check_active(deadline, cancelled)?;
        let (root_uri, target_sha256) = match target {
            SourceFoundationCatalogSchemaTarget::Manifest => (catalog_uri.clone(), *catalog_sha256),
            SourceFoundationCatalogSchemaTarget::Entry => {
                let catalog_schema = catalog_schema
                    .as_ref()
                    .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
                let definitions = catalog_schema
                    .get("$defs")
                    .and_then(Value::as_object)
                    .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
                if !definitions.get("entry").is_some_and(Value::is_object) {
                    return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
                }
                let fragment = "#/$defs/entry";
                if catalog_uri
                    .len()
                    .checked_add(fragment.len())
                    .is_none_or(|length| length > MAX_LOCATION_BYTES)
                {
                    return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
                }
                // Resolve the actual selected subtree through the catalog
                // resource so enclosing `$id` and reference scopes remain
                // identical to the maintained Draft 2020-12 target.
                (format!("{catalog_uri}{fragment}"), *catalog_sha256)
            }
            SourceFoundationCatalogSchemaTarget::ClaimEntry => {
                let catalog_schema = catalog_schema
                    .as_ref()
                    .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
                let definitions = catalog_schema
                    .get("$defs")
                    .and_then(Value::as_object)
                    .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
                let claim_id = definitions
                    .get("tosId")
                    .filter(|definition| definition.is_object())
                    .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
                let claim_entry = definitions
                    .get("claim_entry")
                    .filter(|definition| definition.is_object())
                    .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
                if !catalog_claim_refs_are_local(claim_entry)
                    || !catalog_claim_refs_are_local(claim_id)
                {
                    return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
                }
                if schema_set
                    .resources
                    .iter()
                    .any(|resource| resource.uri == CATALOG_CLAIM_ENTRY_SCHEMA_URI)
                {
                    return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
                }
                // The maintained loader validates a copied claim-entry schema
                // with its root `$defs` replaced by only the catalog's exact
                // `tosId` definition. This fixed URI gives local refs that
                // fragment scope without introducing an ambient registry.
                let resource = derive_catalog_subschema(
                    catalog_schema
                        .get("$schema")
                        .and_then(Value::as_str)
                        .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?,
                    CATALOG_CLAIM_ENTRY_SCHEMA_URI,
                    claim_entry,
                    Some(claim_id),
                    limits
                        .max_schema_resource_bytes
                        .min(derived_byte_budget.saturating_sub(derived_total_bytes)),
                )?;
                let digest = Digest256::of_bytes(&resource.raw);
                derived_total_bytes = derived_total_bytes
                    .checked_add(resource.raw.len())
                    .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
                selection.derived_resources.push(resource);
                (CATALOG_CLAIM_ENTRY_SCHEMA_URI.to_owned(), digest)
            }
        };
        if !selection
            .contracts
            .insert(
                target.selector().to_owned(),
                (root_uri.clone(), target_sha256),
            )
            .is_none()
        {
            return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
        }
        selection.bindings.push(CatalogSchemaTargetBinding {
            target: *target,
            root_uri,
            source_schema_sha256: *catalog_sha256,
            target_schema_sha256: target_sha256,
        });
    }
    let effective_schema_bytes = schema_set
        .schema_bytes
        .checked_add(derived_total_bytes)
        .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
    if effective_resource_count > limits.max_schema_resources
        || effective_resource_count > SchemaBackendProbe::MAX_RESOURCES
        || effective_schema_bytes > limits.max_total_schema_bytes
        || effective_schema_bytes > SchemaBackendProbe::MAX_TOTAL_BYTES
    {
        return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
    }
    Ok(selection)
}

fn catalog_claim_refs_are_local(schema: &Value) -> bool {
    match schema {
        Value::Object(object) => {
            if object.contains_key("$id")
                || object.contains_key("$dynamicRef")
                || object.contains_key("$recursiveRef")
                || object
                    .get("$ref")
                    .is_some_and(|reference| reference.as_str() != Some("#/$defs/tosId"))
            {
                return false;
            }
            object.values().all(catalog_claim_refs_are_local)
        }
        Value::Array(values) => values.iter().all(catalog_claim_refs_are_local),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => true,
    }
}

fn derive_catalog_subschema(
    schema_version: &str,
    uri: &str,
    schema: &Value,
    tos_id: Option<&Value>,
    max_bytes: usize,
) -> Result<SchemaResource, SourceFoundationSchemaFailure> {
    let mut derived = schema
        .as_object()
        .cloned()
        .ok_or(SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
    if derived.contains_key("$schema") || derived.contains_key("$id") {
        return Err(SourceFoundationSchemaFailure::CatalogSchemaSelection);
    }
    derived.insert(
        "$schema".to_owned(),
        Value::String(schema_version.to_owned()),
    );
    derived.insert("$id".to_owned(), Value::String(uri.to_owned()));
    if let Some(tos_id) = tos_id {
        let mut definitions = serde_json::Map::new();
        definitions.insert("tosId".to_owned(), tos_id.clone());
        derived.insert("$defs".to_owned(), Value::Object(definitions));
    }
    let mut output = BoundedSchemaBytes {
        bytes: Vec::with_capacity(max_bytes.min(16 * 1024)),
        max_bytes,
    };
    serde_json::to_writer(&mut output, &Value::Object(derived))
        .map_err(|_| SourceFoundationSchemaFailure::CatalogSchemaSelection)?;
    Ok(SchemaResource {
        uri: uri.to_owned(),
        raw: output.bytes,
    })
}

struct BoundedSchemaBytes {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl Write for BoundedSchemaBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > self.max_bytes)
        {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "source-foundation derived-schema bound",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn source_foundation_schema_resource_uri(value: &Value) -> Option<&str> {
    if value.get("$schema").and_then(Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
    {
        return None;
    }
    value.get("$id").and_then(Value::as_str).filter(|uri| {
        !uri.is_empty()
            && uri.len() <= MAX_LOCATION_BYTES
            && uri.starts_with("https://")
            && !uri.contains('#')
    })
}

pub(crate) fn schema_resource_set_digest(resources: &[SchemaResource]) -> Option<Digest256> {
    let mut digests = BTreeMap::new();
    for resource in resources {
        if digests
            .insert(resource.uri.as_str(), Digest256::of_bytes(&resource.raw))
            .is_some()
        {
            return None;
        }
    }
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-schema-set-v1\0");
    for (uri, digest) in digests {
        hash.update(&(uri.len() as u64).to_be_bytes());
        hash.update(uri.as_bytes());
        hash.update(digest.as_bytes());
    }
    Some(hash.finalize())
}

fn catalog_target_selection_digest(
    contract_selection_sha256: Digest256,
    bindings: &[CatalogSchemaTargetBinding],
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-foundation-catalog-target-selection-v1\0");
    hash.update(contract_selection_sha256.as_bytes());
    hash.update(&(bindings.len() as u64).to_be_bytes());
    for binding in bindings {
        hash.update(&[binding.target.code()]);
        hash.update(&(binding.target.selector().len() as u64).to_be_bytes());
        hash.update(binding.target.selector().as_bytes());
        hash.update(&(SOURCE_FOUNDATION_CATALOG_PATH.len() as u64).to_be_bytes());
        hash.update(SOURCE_FOUNDATION_CATALOG_PATH.as_bytes());
        hash.update(binding.source_schema_sha256.as_bytes());
        hash.update(&(binding.root_uri.len() as u64).to_be_bytes());
        hash.update(binding.root_uri.as_bytes());
        hash.update(binding.target_schema_sha256.as_bytes());
    }
    hash.finalize()
}

struct SchemaEvaluationSelection<'a> {
    resources: &'a [SchemaResource],
    contracts: &'a BTreeMap<String, (String, Digest256)>,
    schema_set_sha256: Digest256,
    contract_selection_sha256: Digest256,
    schema_bytes: usize,
}

fn evaluate_source_foundation_schema_checks_inner<C: SourceFoundationSchemaCheckInput>(
    schema_set: &SourceFoundationSchemaSet,
    selection: &SchemaEvaluationSelection<'_>,
    worker: &ExactWorkerIdentity,
    checks: &[C],
    limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    input_profile: SourceFoundationInputProfile,
    shared_quota: Option<&SharedSchemaWorkerQuota>,
    shared_image: Option<&VerifiedWorkerImageHandle>,
) -> SourceFoundationSchemaOutcome {
    let caps_sha256 = schema_diagnostics::Caps::CURRENT.digest();
    let mut report = empty_schema_report(
        schema_set,
        worker,
        checks.len(),
        limits,
        selection.schema_set_sha256,
        selection.contract_selection_sha256,
        selection.schema_bytes,
    );
    if matches!(input_profile, SourceFoundationInputProfile::Mixed) {
        report.exceptional_schema_budget = Some(ExceptionalSchemaUsage::whole());
    }
    let mut exceptional_remaining = report.exceptional_schema_budget;
    if shared_image.is_some() && shared_quota.is_none() {
        return incomplete(report, SourceFoundationSchemaFailure::InvalidLimits);
    }
    if let Some(image) = shared_image
        && (worker.sha256 != image.identity().sha256
            || worker.absolute_path != image.identity().absolute_path
            || deadline > image.operation_deadline())
    {
        return incomplete(
            report,
            SourceFoundationSchemaFailure::Worker(ExecutorFailure::WorkerIdentity),
        );
    }
    if !limits.validate()
        || limits.digest() != schema_set.limits_sha256
        || deadline > schema_set.deadline
        || checks.is_empty()
        || checks.len() > limits.max_checks
    {
        return incomplete(report, SourceFoundationSchemaFailure::InvalidLimits);
    }
    if let Err(reason) = check_active(deadline, cancelled) {
        return incomplete(report, reason);
    }

    let mut units = Vec::with_capacity(checks.len());
    let mut unit_modes = if matches!(input_profile, SourceFoundationInputProfile::Mixed) {
        Vec::with_capacity(checks.len())
    } else {
        Vec::new()
    };
    let mut total_instance_bytes = 0usize;
    let mut total_metadata_bytes = 0usize;
    for check in checks {
        if let Err(reason) = check_active(deadline, cancelled) {
            return incomplete(report, reason);
        }
        if check.location().len() > MAX_LOCATION_BYTES
            || RelativePath::parse(check.location()).is_err()
        {
            return incomplete(report, SourceFoundationSchemaFailure::InvalidLocation);
        }
        if check.contract_key().len() > MAX_LOCATION_BYTES {
            return incomplete(report, SourceFoundationSchemaFailure::ContractNotSelected);
        }
        let Some((root_uri, _)) = selection.contracts.get(check.contract_key()) else {
            return incomplete(report, SourceFoundationSchemaFailure::ContractNotSelected);
        };
        let Some(next_metadata_bytes) = total_metadata_bytes
            .checked_add(check.location().len())
            .and_then(|total| total.checked_add(check.report_contract().len()))
            .and_then(|total| total.checked_add(root_uri.len()))
            .and_then(|total| total.checked_add(64))
        else {
            return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
        };
        if next_metadata_bytes
            .checked_add(report.cost.estimated_report_bytes)
            .is_none_or(|total| total > limits.max_total_report_bytes)
        {
            return incomplete(
                report,
                SourceFoundationSchemaFailure::DiagnosticReportBudget,
            );
        }
        total_metadata_bytes = next_metadata_bytes;
        let check_profile = check.input_profile();
        let unit_profile = match input_profile {
            SourceFoundationInputProfile::Mixed => check_profile,
            expected if expected == check_profile => expected,
            _ => return incomplete(report, SourceFoundationSchemaFailure::InputProfileMismatch),
        };
        let (raw_instance, next_instance_bytes) = match (unit_profile, check.instance()) {
            (
                SourceFoundationInputProfile::FiniteJson,
                SourceFoundationSchemaInstance::Decoded(value),
            ) => {
                // The decoded finite route normally keeps the independent
                // one-MiB probe ceiling. A source-foundation caller may opt
                // into a larger finite JSON unit by selecting
                // `max_instance_bytes`; constrain encoding by every
                // remaining aggregate raw-byte budget before growing the
                // owned request bytes.
                let remaining_total_instance_bytes = match limits
                    .max_total_instance_bytes
                    .checked_sub(total_instance_bytes)
                {
                    Some(remaining) if remaining > 0 => remaining,
                    _ => return incomplete(report, SourceFoundationSchemaFailure::InputBudget),
                };
                let finite_instance_limit = limits
                    .max_instance_bytes
                    .min(remaining_total_instance_bytes)
                    .min(limits.batch.max_total_raw_bytes)
                    .min(BatchBudget::MAX_RAW_BYTES);
                let raw = match encode_instance_bounded(value, finite_instance_limit) {
                    Ok(raw) => raw,
                    Err(()) => {
                        return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
                    }
                };
                if raw.len() > finite_instance_limit {
                    return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
                }
                let next_total = match total_instance_bytes.checked_add(raw.len()) {
                    Some(total) if total <= limits.max_total_instance_bytes => total,
                    _ => return incomplete(report, SourceFoundationSchemaFailure::InputBudget),
                };
                (raw, next_total)
            }
            (
                SourceFoundationInputProfile::LegacyPythonObserved,
                SourceFoundationSchemaInstance::LegacyPythonRaw(raw),
            ) => {
                if raw.len() > limits.max_instance_bytes || raw.len() > BatchBudget::MAX_RAW_BYTES {
                    return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
                }
                let next_total = match total_instance_bytes.checked_add(raw.len()) {
                    Some(total) if total <= limits.max_total_instance_bytes => total,
                    _ => return incomplete(report, SourceFoundationSchemaFailure::InputBudget),
                };
                // Charge the aggregate source-copy budget before allocating the
                // BatchUnit-owned copy; the original caller bytes remain untouched.
                (raw.to_vec(), next_total)
            }
            (SourceFoundationInputProfile::Mixed, _) => {
                return incomplete(report, SourceFoundationSchemaFailure::InputProfileMismatch);
            }
            _ => return incomplete(report, SourceFoundationSchemaFailure::InputProfileMismatch),
        };
        total_instance_bytes = next_instance_bytes;
        let ordinal = units.len() as u64;
        units.push(BatchUnit {
            ordinal,
            member_id: format!("source-foundation-schema:{ordinal}"),
            relative_path: check.location().to_owned(),
            root_uri: root_uri.clone(),
            raw_instance,
        });
        if matches!(input_profile, SourceFoundationInputProfile::Mixed) {
            unit_modes.push(match unit_profile {
                SourceFoundationInputProfile::FiniteJson => {
                    if units.last().is_some_and(|unit| {
                        unit.raw_instance.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES
                    }) {
                        DiagnosticsUnitInputMode::FiniteJsonSelected
                    } else {
                        DiagnosticsUnitInputMode::FiniteJson
                    }
                }
                SourceFoundationInputProfile::LegacyPythonObserved => {
                    DiagnosticsUnitInputMode::LegacyPythonObserved
                }
                SourceFoundationInputProfile::Mixed => {
                    return incomplete(report, SourceFoundationSchemaFailure::InputProfileMismatch);
                }
            });
        }
        report.cost.checks = units.len();
        report.cost.metadata_bytes = total_metadata_bytes;
        report.cost.decoded_instance_bytes = total_instance_bytes;
    }
    let total_unit_count = units.len();
    let mut total_issues = 0usize;
    let mut estimated_bytes = report.cost.estimated_report_bytes;
    let mut remaining_cpu_micros = report.max_total_cpu_micros;
    let mut remaining_worker_wire_bytes = limits.max_total_worker_wire_bytes;
    let mut chunk_start = 0usize;
    while chunk_start < total_unit_count {
        if let Err(reason) = check_active(deadline, cancelled) {
            return incomplete(report, reason);
        }
        if report.checkpoints.len() >= limits.max_chunks || remaining_cpu_micros == 0 {
            return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
        }
        let mut chunk_count = 0usize;
        let mut chunk_raw_bytes = 0usize;
        for unit in units.iter().take(limits.batch.max_units) {
            let next_size = unit.raw_instance.len();
            let Some(next_total) = chunk_raw_bytes.checked_add(next_size) else {
                return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
            };
            if next_total > limits.batch.max_total_raw_bytes {
                break;
            }
            chunk_raw_bytes = next_total;
            chunk_count += 1;
        }
        if chunk_count == 0 {
            return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
        }
        let chunk_end = match chunk_start.checked_add(chunk_count) {
            Some(end) if end <= total_unit_count => end,
            _ => return incomplete(report, SourceFoundationSchemaFailure::InputBudget),
        };
        let chunk_index = report.checkpoints.len();
        let chunk_units = units
            .drain(..chunk_count)
            .enumerate()
            .map(|(local_ordinal, mut unit)| {
                unit.ordinal = local_ordinal as u64;
                unit
            })
            .collect::<Vec<_>>();
        let chunk_modes = if matches!(input_profile, SourceFoundationInputProfile::Mixed) {
            unit_modes.drain(..chunk_count).collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let selected_finite_profile =
            matches!(input_profile, SourceFoundationInputProfile::FiniteJson)
                && chunk_units
                    .iter()
                    .any(|unit| unit.raw_instance.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES);
        let expected_input_profile = match input_profile {
            SourceFoundationInputProfile::FiniteJson if selected_finite_profile => {
                DiagnosticsInputProfile::FiniteJsonSelected
            }
            SourceFoundationInputProfile::FiniteJson => DiagnosticsInputProfile::FiniteJson,
            SourceFoundationInputProfile::LegacyPythonObserved => {
                DiagnosticsInputProfile::LegacyPythonObserved
            }
            SourceFoundationInputProfile::Mixed => DiagnosticsInputProfile::MixedSourceFoundation,
        };
        let expected_unit_modes = matches!(input_profile, SourceFoundationInputProfile::Mixed)
            .then_some(chunk_modes.as_slice());
        let expected = match BatchCoverageExpectation::from_diagnostics_units(
            &chunk_units,
            expected_input_profile,
            expected_unit_modes,
        ) {
            Ok(expected) => expected,
            Err(reason) => {
                return incomplete(report, SourceFoundationSchemaFailure::Worker(reason));
            }
        };
        let remaining = match deadline.checked_duration_since(Instant::now()) {
            Some(remaining) if !remaining.is_zero() => remaining,
            _ => return incomplete(report, SourceFoundationSchemaFailure::Deadline),
        };
        let mut batch = limits.batch;
        batch.total_execution_wall = batch.total_execution_wall.min(remaining);
        batch.startup_wall = batch.startup_wall.min(batch.total_execution_wall);
        batch.per_unit_wall = batch.per_unit_wall.min(batch.total_execution_wall);
        let Some(remaining_cpu_seconds_ceil) = remaining_cpu_micros
            .checked_add(999_999)
            .map(|micros| micros / 1_000_000)
        else {
            return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
        };
        batch.cpu_seconds = batch.cpu_seconds.min(remaining_cpu_seconds_ceil);
        if let Some(quota) = shared_quota {
            let usage = match quota.usage() {
                Ok(usage) => usage,
                Err(reason) => {
                    return incomplete(report, SourceFoundationSchemaFailure::Worker(reason));
                }
            };
            let Some(shared_remaining_cpu_micros) = usage
                .max_total_cpu_micros
                .checked_sub(usage.worker_cpu_micros)
                .filter(|remaining| *remaining > 0)
            else {
                return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
            };
            let Some(shared_remaining_cpu_seconds_ceil) = shared_remaining_cpu_micros
                .checked_add(999_999)
                .map(|micros| micros / 1_000_000)
            else {
                return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
            };
            batch.cpu_seconds = batch
                .cpu_seconds
                .min(60)
                .min(shared_remaining_cpu_seconds_ceil);
        }
        if batch.cpu_seconds == 0 {
            return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
        }
        report.cost.worker_cpu_budget_seconds = match report
            .cost
            .worker_cpu_budget_seconds
            .checked_add(batch.cpu_seconds)
            .filter(|total| {
                report
                    .checkpoints
                    .len()
                    .checked_add(1)
                    .and_then(|chunks| u64::try_from(chunks).ok())
                    .and_then(|chunks| chunks.checked_mul(report.max_child_cpu_seconds))
                    .is_some_and(|maximum| *total <= maximum)
            }) {
            Some(total) => total,
            _ => return incomplete(report, SourceFoundationSchemaFailure::InputBudget),
        };

        let outcome = if let Some(image) = shared_image {
            let Some(quota) = shared_quota else {
                return incomplete(report, SourceFoundationSchemaFailure::InvalidLimits);
            };
            let diagnostics_profile = match input_profile {
                SourceFoundationInputProfile::FiniteJson if selected_finite_profile => {
                    DiagnosticsInputProfile::FiniteJsonSelected
                }
                SourceFoundationInputProfile::FiniteJson => DiagnosticsInputProfile::FiniteJson,
                SourceFoundationInputProfile::LegacyPythonObserved => {
                    DiagnosticsInputProfile::LegacyPythonObserved
                }
                SourceFoundationInputProfile::Mixed => {
                    DiagnosticsInputProfile::MixedSourceFoundation
                }
            };
            let unit_modes = if matches!(input_profile, SourceFoundationInputProfile::Mixed) {
                if chunk_modes.len() != chunk_units.len() {
                    return incomplete(report, SourceFoundationSchemaFailure::InputProfileMismatch);
                }
                Some(chunk_modes)
            } else {
                None
            };
            let exceptional_remaining =
                if matches!(input_profile, SourceFoundationInputProfile::Mixed) {
                    match exceptional_remaining {
                        Some(remaining) => Some(remaining),
                        None => {
                            return incomplete(
                                report,
                                SourceFoundationSchemaFailure::InputProfileMismatch,
                            );
                        }
                    }
                } else {
                    None
                };
            BoundedSchemaExecutor::evaluate_source_foundation_diagnostics_with_image(
                worker,
                selection.resources,
                schema_set.profile,
                diagnostics_profile,
                chunk_units,
                unit_modes,
                exceptional_remaining,
                expected,
                batch,
                remaining_worker_wire_bytes,
                quota,
                image,
                cancelled,
            )
        } else {
            match input_profile {
                SourceFoundationInputProfile::FiniteJson => {
                    if let Some(quota) = shared_quota {
                        if selected_finite_profile {
                            BoundedSchemaExecutor::evaluate_batch_with_selected_finite_diagnostics_wire_limited_shared_quota_cancellable(
                            worker,
                            selection.resources,
                            schema_set.profile,
                            chunk_units,
                            expected,
                            batch,
                            remaining_worker_wire_bytes,
                            quota,
                            cancelled,
                        )
                        } else {
                            BoundedSchemaExecutor::evaluate_batch_with_diagnostics_wire_limited_shared_quota_cancellable(
                            worker,
                            selection.resources,
                            schema_set.profile,
                            chunk_units,
                            expected,
                            batch,
                            remaining_worker_wire_bytes,
                            quota,
                            cancelled,
                        )
                        }
                    } else {
                        if selected_finite_profile {
                            BoundedSchemaExecutor::evaluate_batch_with_selected_finite_diagnostics_wire_limited_cancellable(
                            worker,
                            selection.resources,
                            schema_set.profile,
                            chunk_units,
                            expected,
                            batch,
                            remaining_worker_wire_bytes,
                            cancelled,
                        )
                        } else {
                            BoundedSchemaExecutor::evaluate_batch_with_diagnostics_wire_limited_cancellable(
                            worker,
                            selection.resources,
                            schema_set.profile,
                            chunk_units,
                            expected,
                            batch,
                            remaining_worker_wire_bytes,
                            cancelled,
                        )
                        }
                    }
                }
                SourceFoundationInputProfile::LegacyPythonObserved => {
                    if let Some(quota) = shared_quota {
                        BoundedSchemaExecutor::evaluate_batch_with_legacy_python_diagnostics_wire_limited_shared_quota_cancellable(
                        worker,
                        selection.resources,
                        schema_set.profile,
                        chunk_units,
                        expected,
                        batch,
                        remaining_worker_wire_bytes,
                        quota,
                        cancelled,
                    )
                    } else {
                        BoundedSchemaExecutor::evaluate_batch_with_legacy_python_diagnostics_wire_limited_cancellable(
                        worker,
                        selection.resources,
                        schema_set.profile,
                        chunk_units,
                        expected,
                        batch,
                        remaining_worker_wire_bytes,
                        cancelled,
                    )
                    }
                }
                SourceFoundationInputProfile::Mixed => {
                    if chunk_modes.len() != chunk_units.len() {
                        return incomplete(
                            report,
                            SourceFoundationSchemaFailure::InputProfileMismatch,
                        );
                    }
                    let mixed_units = chunk_units
                        .into_iter()
                        .zip(chunk_modes)
                        .map(|(unit, input_mode)| MixedDiagnosticsBatchUnit { unit, input_mode });
                    let exceptional_remaining = match exceptional_remaining {
                        Some(remaining) => remaining,
                        None => {
                            return incomplete(
                                report,
                                SourceFoundationSchemaFailure::InputProfileMismatch,
                            );
                        }
                    };
                    if let Some(quota) = shared_quota {
                        BoundedSchemaExecutor::evaluate_batch_with_mixed_source_foundation_diagnostics_wire_limited_shared_quota_cancellable(
                        worker,
                        selection.resources,
                        schema_set.profile,
                        mixed_units,
                        expected,
                        exceptional_remaining,
                        batch,
                        remaining_worker_wire_bytes,
                        quota,
                        cancelled,
                    )
                    } else {
                        BoundedSchemaExecutor::evaluate_batch_with_mixed_source_foundation_diagnostics_wire_limited_cancellable(
                        worker,
                        selection.resources,
                        schema_set.profile,
                        mixed_units,
                        expected,
                        exceptional_remaining,
                        batch,
                        remaining_worker_wire_bytes,
                        cancelled,
                    )
                    }
                }
            }
        };
        let (diagnostic_units, checkpoint, worker_failure, exchange_failure_context) = match outcome
        {
            SchemaDiagnosticsOutcome::Incomplete {
                checkpoint,
                reason,
                exchange,
            } => (None, checkpoint, Some(reason), exchange),
            SchemaDiagnosticsOutcome::Complete { units, checkpoint } => {
                (Some(units), checkpoint, None, None)
            }
        };
        report.exchange_failure_context = exchange_failure_context;
        let observed_chunk_cpu_micros =
            record_worker_cpu_cost(&mut report, &checkpoint, remaining_cpu_micros);
        let chunk_wire_bytes =
            match record_worker_wire_cost(&mut report, &checkpoint, remaining_worker_wire_bytes) {
                Ok(bytes) => bytes,
                Err(reason) => {
                    report.checkpoints.push(checkpoint);
                    return incomplete(report, reason);
                }
            };
        let observed_chunk_cpu_micros = match observed_chunk_cpu_micros {
            Ok(micros) => micros,
            Err(reason) => {
                report.checkpoints.push(checkpoint);
                return incomplete(report, reason);
            }
        };
        if worker_failure.is_none()
            && (checkpoint.worker_request_bytes == 0 || checkpoint.worker_response_bytes == 0)
        {
            report.checkpoints.push(checkpoint);
            return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
        }
        if let Some(reason) = worker_failure {
            report.checkpoints.push(checkpoint);
            return incomplete(report, SourceFoundationSchemaFailure::Worker(reason));
        }
        let Some(observed_chunk_cpu_micros) = observed_chunk_cpu_micros else {
            report.checkpoints.push(checkpoint);
            return incomplete(
                report,
                SourceFoundationSchemaFailure::Worker(ExecutorFailure::ResourceLimitUnknown),
            );
        };
        let Some(next_remaining_cpu_micros) =
            remaining_cpu_micros.checked_sub(observed_chunk_cpu_micros)
        else {
            report.checkpoints.push(checkpoint);
            return incomplete(report, SourceFoundationSchemaFailure::InputBudget);
        };
        remaining_cpu_micros = next_remaining_cpu_micros;
        let Some(next_remaining_worker_wire_bytes) =
            remaining_worker_wire_bytes.checked_sub(chunk_wire_bytes)
        else {
            report.checkpoints.push(checkpoint);
            return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
        };
        remaining_worker_wire_bytes = next_remaining_worker_wire_bytes;
        let Some(diagnostic_units) = diagnostic_units else {
            report.checkpoints.push(checkpoint);
            return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
        };
        match (
            input_profile,
            exceptional_remaining,
            checkpoint.exceptional_remaining,
            checkpoint.exceptional_usage,
        ) {
            (SourceFoundationInputProfile::Mixed, Some(remaining), Some(requested), Some(used))
                if remaining == requested =>
            {
                let Some(next_remaining) = remaining.checked_sub(used) else {
                    report.checkpoints.push(checkpoint);
                    return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
                };
                exceptional_remaining = Some(next_remaining);
            }
            (SourceFoundationInputProfile::Mixed, _, _, _) => {
                report.checkpoints.push(checkpoint);
                return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
            }
            (_, None, None, None) => {}
            _ => {
                report.checkpoints.push(checkpoint);
                return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
            }
        }
        if diagnostic_units.len() != chunk_end - chunk_start
            || checkpoint.completed_count != (chunk_end - chunk_start) as u64
            || checkpoint.worker_sha256 != worker.sha256
            || checkpoint.profile != schema_set.profile
            || checkpoint.schema_set_sha256 != selection.schema_set_sha256
            || checkpoint.ordered_manifest_sha256 != expected.ordered_manifest_sha256
            || checkpoint.caps_sha256 != caps_sha256
        {
            report.checkpoints.push(checkpoint);
            return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
        }
        report.checkpoints.push(checkpoint);
        let mut diagnostic_failure = None;
        for (local_ordinal, (unit, check)) in diagnostic_units
            .iter()
            .zip(&checks[chunk_start..chunk_end])
            .enumerate()
        {
            if unit.ordinal != local_ordinal as u64
                || unit.relative_path != check.location()
                || unit.root_uri != selection.contracts[check.contract_key()].0
                || unit.report.worker_sha256 != worker.sha256
                || unit.report.schema_set_sha256 != selection.schema_set_sha256
                || unit.report.caps_sha256() != caps_sha256
                || !diagnostic_is_well_formed(
                    &unit.report,
                    worker.sha256,
                    checkpoint.request_sha256,
                    selection.schema_set_sha256,
                    caps_sha256,
                )
            {
                return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
            }
            let status_failure = match (unit.report.status, unit.report.failure) {
                (
                    schema_diagnostics::Status::Indeterminate,
                    schema_diagnostics::Failure::UnsupportedInputSemantics,
                ) => Some(SourceFoundationSchemaFailure::UnsupportedInputSemantics),
                (schema_diagnostics::Status::Truncated, _) => {
                    Some(SourceFoundationSchemaFailure::TruncatedDiagnostics)
                }
                (
                    schema_diagnostics::Status::Indeterminate
                    | schema_diagnostics::Status::InputRejected,
                    _,
                ) => Some(SourceFoundationSchemaFailure::IncompleteDiagnostic),
                _ => None,
            };
            let mapped = map_issues(check.location(), &unit.report.issues);
            let Some(next_issues) = total_issues.checked_add(mapped.len()) else {
                return incomplete(
                    report,
                    SourceFoundationSchemaFailure::DiagnosticReportBudget,
                );
            };
            if next_issues > limits.max_total_issues {
                return incomplete(
                    report,
                    SourceFoundationSchemaFailure::DiagnosticReportBudget,
                );
            }
            let Some(next_estimated_bytes) =
                estimated_bytes.checked_add(estimated_check_report_bytes(
                    check.location(),
                    check.report_contract(),
                    &mapped,
                    &unit.report,
                ))
            else {
                return incomplete(
                    report,
                    SourceFoundationSchemaFailure::DiagnosticReportBudget,
                );
            };
            if next_estimated_bytes
                .checked_add(report.cost.metadata_bytes)
                .is_none_or(|total| total > limits.max_total_report_bytes)
            {
                return incomplete(
                    report,
                    SourceFoundationSchemaFailure::DiagnosticReportBudget,
                );
            }
            total_issues = next_issues;
            estimated_bytes = next_estimated_bytes;
            if diagnostic_failure.is_none() {
                diagnostic_failure = status_failure;
            }
            report.checks.push(SourceFoundationSchemaCheckReport {
                chunk_index,
                location: check.location().to_owned(),
                contract: check.report_contract().to_owned(),
                diagnostic: unit.report.clone(),
                issues: mapped,
            });
            report.cost.diagnostic_issues = total_issues;
            report.cost.estimated_report_bytes = estimated_bytes;
        }
        if let Some(reason) = diagnostic_failure {
            return incomplete(report, reason);
        }
        if let Err(reason) = check_active(deadline, cancelled) {
            return incomplete(report, reason);
        }
        chunk_start = chunk_end;
    }
    if let Err(reason) = check_active(deadline, cancelled) {
        return incomplete(report, reason);
    }
    report.binding_sha256 = Some(binding_digest(&report));
    if !report.is_complete() {
        return incomplete(report, SourceFoundationSchemaFailure::DiagnosticBinding);
    }
    SourceFoundationSchemaOutcome::Complete(report)
}

fn empty_schema_report(
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    expected_check_count: usize,
    limits: SourceFoundationSchemaLimits,
    schema_set_sha256: Digest256,
    contract_selection_sha256: Digest256,
    schema_bytes: usize,
) -> SourceFoundationSchemaReport {
    SourceFoundationSchemaReport {
        source_revision: schema_set.source_revision,
        profile: schema_set.profile,
        worker_sha256: worker.sha256,
        schema_set_sha256,
        contract_selection_sha256,
        limits_sha256: limits.digest(),
        caps_sha256: schema_diagnostics::Caps::CURRENT.digest(),
        max_total_cpu_micros: limits.max_total_cpu_seconds * 1_000_000,
        max_child_cpu_seconds: limits.batch.cpu_seconds,
        max_total_worker_wire_bytes: limits.max_total_worker_wire_bytes,
        exceptional_schema_budget: None,
        checkpoints: Vec::new(),
        binding_sha256: None,
        expected_check_count,
        checks: Vec::new(),
        cost: SourceFoundationSchemaCost {
            estimated_report_bytes: std::mem::size_of::<Option<ExchangeFailureContext>>(),
            schema_resource_bytes: schema_bytes,
            ..SourceFoundationSchemaCost::default()
        },
        exchange_failure_context: None,
    }
}

fn incomplete(
    report: SourceFoundationSchemaReport,
    reason: SourceFoundationSchemaFailure,
) -> SourceFoundationSchemaOutcome {
    SourceFoundationSchemaOutcome::Incomplete { report, reason }
}

fn record_worker_wire_cost(
    report: &mut SourceFoundationSchemaReport,
    checkpoint: &SchemaDiagnosticsCheckpoint,
    remaining_worker_wire_bytes: u64,
) -> Result<u64, SourceFoundationSchemaFailure> {
    let Some(chunk_wire_bytes) = checkpoint
        .worker_request_bytes
        .checked_add(checkpoint.worker_response_bytes)
    else {
        report.cost.worker_request_bytes = u64::MAX;
        report.cost.worker_response_bytes = u64::MAX;
        report.cost.worker_wire_bytes = u64::MAX;
        return Err(SourceFoundationSchemaFailure::Worker(
            ExecutorFailure::InputBudget,
        ));
    };
    let Some(request_bytes) = report
        .cost
        .worker_request_bytes
        .checked_add(checkpoint.worker_request_bytes)
    else {
        report.cost.worker_request_bytes = u64::MAX;
        report.cost.worker_response_bytes = u64::MAX;
        report.cost.worker_wire_bytes = u64::MAX;
        return Err(SourceFoundationSchemaFailure::Worker(
            ExecutorFailure::InputBudget,
        ));
    };
    let Some(response_bytes) = report
        .cost
        .worker_response_bytes
        .checked_add(checkpoint.worker_response_bytes)
    else {
        report.cost.worker_request_bytes = u64::MAX;
        report.cost.worker_response_bytes = u64::MAX;
        report.cost.worker_wire_bytes = u64::MAX;
        return Err(SourceFoundationSchemaFailure::Worker(
            ExecutorFailure::InputBudget,
        ));
    };
    let Some(wire_bytes) = request_bytes.checked_add(response_bytes) else {
        report.cost.worker_request_bytes = u64::MAX;
        report.cost.worker_response_bytes = u64::MAX;
        report.cost.worker_wire_bytes = u64::MAX;
        return Err(SourceFoundationSchemaFailure::Worker(
            ExecutorFailure::InputBudget,
        ));
    };
    report.cost.worker_request_bytes = request_bytes;
    report.cost.worker_response_bytes = response_bytes;
    report.cost.worker_wire_bytes = wire_bytes;
    if chunk_wire_bytes > remaining_worker_wire_bytes
        || wire_bytes > report.max_total_worker_wire_bytes
    {
        return Err(SourceFoundationSchemaFailure::Worker(
            ExecutorFailure::InputBudget,
        ));
    }
    Ok(chunk_wire_bytes)
}

fn record_worker_cpu_cost(
    report: &mut SourceFoundationSchemaReport,
    checkpoint: &SchemaDiagnosticsCheckpoint,
    remaining_cpu_micros: u64,
) -> Result<Option<u64>, SourceFoundationSchemaFailure> {
    let Some(chunk_cpu_micros) = checkpoint.worker_cpu_micros else {
        return Ok(None);
    };
    let Some(total_cpu_micros) = report
        .cost
        .worker_cpu_micros
        .unwrap_or(0)
        .checked_add(chunk_cpu_micros)
    else {
        report.cost.worker_cpu_micros = None;
        return Err(SourceFoundationSchemaFailure::Worker(
            ExecutorFailure::InputBudget,
        ));
    };
    report.cost.worker_cpu_micros = Some(total_cpu_micros);
    if chunk_cpu_micros > remaining_cpu_micros || total_cpu_micros > report.max_total_cpu_micros {
        return Err(SourceFoundationSchemaFailure::Worker(
            ExecutorFailure::InputBudget,
        ));
    }
    Ok(Some(chunk_cpu_micros))
}

fn check_active(
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), SourceFoundationSchemaFailure> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(SourceFoundationSchemaFailure::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(SourceFoundationSchemaFailure::Deadline);
    }
    Ok(())
}

fn encode_instance_bounded(value: &Value, max_bytes: usize) -> Result<Vec<u8>, ()> {
    let mut output = BoundedInstanceBytes {
        bytes: Vec::with_capacity(max_bytes.min(16 * 1024)),
        max_bytes,
    };
    serde_json::to_writer(&mut output, value).map_err(|_| ())?;
    Ok(output.bytes)
}

struct BoundedInstanceBytes {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl Write for BoundedInstanceBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > self.max_bytes)
        {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "source foundation schema instance bound",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn check_load_active(
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), SourceFoundationSchemaLoadFailure> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(SourceFoundationSchemaLoadFailure::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(SourceFoundationSchemaLoadFailure::Deadline);
    }
    Ok(())
}

fn diagnostic_is_well_formed(
    report: &schema_diagnostics::Report,
    worker_sha256: Digest256,
    request_sha256: Digest256,
    schema_set_sha256: Digest256,
    caps_sha256: Digest256,
) -> bool {
    report.protocol_version == schema_diagnostics::PROTOCOL_VERSION
        && report.worker_sha256 == worker_sha256
        && report.request_sha256 == request_sha256
        && report.schema_set_sha256 == schema_set_sha256
        && report.caps_sha256() == caps_sha256
        && schema_diagnostics::status_is_well_formed(
            report.status,
            report.failure,
            report.total_issue_count,
            report.truncated,
            report.issues.len(),
        )
        && !report.issues.windows(2).any(|pair| pair[0] > pair[1])
        && schema_diagnostics::issues_digest(&report.issues)
            .is_some_and(|digest| digest == report.issues_sha256)
        && report.report_sha256
            == schema_diagnostics::report_digest(
                report.worker_sha256,
                report.request_sha256,
                report.unit_sha256,
                report.schema_set_sha256,
                report.caps,
                report.status,
                report.failure,
                report.total_issue_count,
                report.truncated,
                report.issues_sha256,
            )
}

fn binding_digest(report: &SourceFoundationSchemaReport) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-foundation-schema-report-v4\0");
    hash.update(&schema_diagnostics::PROTOCOL_VERSION.to_be_bytes());
    hash.update(report.source_revision.0.as_bytes());
    hash.update(report.profile.id().as_bytes());
    hash.update(report.worker_sha256.as_bytes());
    hash.update(report.schema_set_sha256.as_bytes());
    hash.update(report.contract_selection_sha256.as_bytes());
    hash.update(report.limits_sha256.as_bytes());
    hash.update(report.caps_sha256.as_bytes());
    hash.update(&report.max_total_cpu_micros.to_be_bytes());
    hash.update(&report.max_child_cpu_seconds.to_be_bytes());
    hash.update(&report.max_total_worker_wire_bytes.to_be_bytes());
    hash_exceptional_usage(&mut hash, report.exceptional_schema_budget);
    hash.update(&(report.checkpoints.len() as u64).to_be_bytes());
    for checkpoint in &report.checkpoints {
        hash.update(checkpoint.request_sha256.as_bytes());
        hash.update(checkpoint.ordered_manifest_sha256.as_bytes());
        hash.update(checkpoint.result_stream_sha256.as_bytes());
        hash.update(&checkpoint.completed_count.to_be_bytes());
        hash.update(&checkpoint.worker_request_bytes.to_be_bytes());
        hash.update(&checkpoint.worker_response_bytes.to_be_bytes());
        hash_optional_u64(&mut hash, checkpoint.worker_cpu_micros);
        hash_exceptional_usage(&mut hash, checkpoint.exceptional_usage);
        hash_exceptional_usage(&mut hash, checkpoint.exceptional_remaining);
    }
    hash.update(&(report.expected_check_count as u64).to_be_bytes());
    for value in [
        report.cost.checks as u64,
        report.cost.worker_cpu_budget_seconds,
        report.cost.metadata_bytes as u64,
        report.cost.decoded_instance_bytes as u64,
        report.cost.diagnostic_issues as u64,
        report.cost.estimated_report_bytes as u64,
        report.cost.schema_resource_bytes as u64,
        report.cost.worker_request_bytes,
        report.cost.worker_response_bytes,
        report.cost.worker_wire_bytes,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash_optional_u64(&mut hash, report.cost.worker_cpu_micros);
    for check in &report.checks {
        hash.update(&(check.chunk_index as u64).to_be_bytes());
        hash.update(&(check.location.len() as u64).to_be_bytes());
        hash.update(check.location.as_bytes());
        hash.update(&(check.contract.len() as u64).to_be_bytes());
        hash.update(check.contract.as_bytes());
        hash.update(check.diagnostic.report_sha256.as_bytes());
    }
    hash.finalize()
}

fn hash_optional_u64(hash: &mut Digest256Hasher, value: Option<u64>) {
    match value {
        Some(value) => {
            hash.update(&[1]);
            hash.update(&value.to_be_bytes());
        }
        None => hash.update(&[0]),
    }
}

fn hash_exceptional_usage(hash: &mut Digest256Hasher, usage: Option<ExceptionalSchemaUsage>) {
    let Some(usage) = usage else {
        hash.update(&[0]);
        return;
    };
    hash.update(&[1]);
    for value in [
        usage.schema_scan_work,
        usage.schema_scan_bytes,
        usage.pattern_compile_count,
        usage.pattern_bytes,
        usage.evaluation_work,
        usage.evaluation_bytes,
        usage.reference_steps,
        usage.regex_checks,
        usage.regex_bytes,
    ] {
        hash.update(&value.to_be_bytes());
    }
}

fn manifest_digest(checks: &[SourceFoundationSchemaCheckReport]) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-val2-batch-manifest-v1\0");
    for check in checks {
        hash.update(check.diagnostic.unit_sha256.as_bytes());
    }
    hash.finalize()
}

fn contract_selection_digest(paths: &BTreeSet<&str>) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-foundation-schema-contract-selection-v1\0");
    hash.update(&(paths.len() as u64).to_be_bytes());
    for path in paths {
        hash.update(&(path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
    }
    hash.finalize()
}

fn map_issues(
    location: &str,
    issues: &[schema_diagnostics::Issue],
) -> Vec<SourceFoundationSchemaIssue> {
    source_foundation_schema_issues(location, issues).collect()
}

/// Map the same closed prose and Python-compatible location suffix for a
/// separately authenticated diagnostic-v2 unit. The caller must first verify
/// its complete binding and admit the copied strings; this iterator provides
/// presentation only and cannot turn an incomplete unit into a verdict.
pub fn source_foundation_schema_issues<'a>(
    location: &'a str,
    issues: &'a [schema_diagnostics::Issue],
) -> impl Iterator<Item = SourceFoundationSchemaIssue> + 'a {
    issues.iter().map(move |issue| SourceFoundationSchemaIssue {
        location: format!("{location}{}", python_path_suffix(&issue.instance_path)),
        schema_keyword: issue.schema_keyword.clone(),
        reason: issue.reason,
        message: issue
            .compatibility_text()
            .unwrap_or_else(|| reason_prose(issue.reason)),
    })
}

/// Closed replacement prose for diagnostic-v2. It intentionally avoids
/// serializing the rejected value or depending on backend error formatting.
pub const fn reason_prose(reason: schema_diagnostics::Reason) -> &'static str {
    use schema_diagnostics::Reason;
    match reason {
        Reason::AdditionalItems => "additional array items are not allowed",
        Reason::AdditionalProperties => "additional object properties are not allowed",
        Reason::AnyOf => "value does not satisfy any permitted schema alternative",
        Reason::Pattern => "string does not satisfy the schema pattern",
        Reason::Constant => "value does not match the required constant",
        Reason::Contains => "array does not contain the required matching item",
        Reason::ContentEncoding => "value does not satisfy the schema content encoding",
        Reason::ContentMediaType => "value does not satisfy the schema content media type",
        Reason::CustomKeyword => "value does not satisfy a schema extension keyword",
        Reason::Enum => "value is not one of the permitted values",
        Reason::ExclusiveMaximum => "number is not below the exclusive maximum",
        Reason::ExclusiveMinimum => "number is not above the exclusive minimum",
        Reason::FalseSchema => "value is rejected by a false schema",
        Reason::Format => "value does not satisfy the required format",
        Reason::MaximumItems => "array contains more items than permitted",
        Reason::Maximum => "number exceeds the permitted maximum",
        Reason::MaximumLength => "string exceeds the permitted length",
        Reason::MaximumProperties => "object contains more properties than permitted",
        Reason::MinimumItems => "array contains fewer items than required",
        Reason::Minimum => "number is below the permitted minimum",
        Reason::MinimumLength => "string is shorter than the required length",
        Reason::MinimumProperties => "object contains fewer properties than required",
        Reason::MultipleOf => "number is not a permitted multiple",
        Reason::Not => "value matches a schema that must not match",
        Reason::OneOf => "value does not match exactly one permitted schema alternative",
        Reason::PropertyNames => "object property name does not satisfy its schema",
        Reason::Required => "object is missing a required property",
        Reason::Type => "value has the wrong JSON type",
        Reason::UnevaluatedItems => "array contains an item not covered by its schema",
        Reason::UnevaluatedProperties => "object contains a property not covered by its schema",
        Reason::UniqueItems => "array contains duplicate items",
        Reason::BacktrackLimit => "schema pattern evaluation exceeded its resource bound",
        Reason::RegexEngineFailure => "schema pattern evaluation failed",
        Reason::ReferenceFailure => "schema reference resolution failed",
        Reason::UnknownValidationFailure => "schema validation did not produce a supported result",
    }
}

/// Render the Python `repr` suffix used by the maintained source validator for
/// JSON property names and array indices. Issue ordering remains the structured
/// diagnostics-v2 canonical order; this renderer only maps path identity to the
/// legacy location suffix shape.
pub fn python_path_suffix(path: &[schema_diagnostics::PathSegment]) -> String {
    let mut suffix = String::new();
    for segment in path {
        suffix.push('[');
        match segment {
            schema_diagnostics::PathSegment::Property(value) => {
                suffix.push_str(&python_string_repr(value));
            }
            schema_diagnostics::PathSegment::Index(value) => {
                suffix.push_str(&value.to_string());
            }
        }
        suffix.push(']');
    }
    suffix
}

fn python_string_repr(value: &str) -> String {
    use unicode_general_category::{GeneralCategory, get_general_category};
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push(quote);
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            value if value == quote => {
                output.push('\\');
                output.push(value);
            }
            value if !python_printable(value, get_general_category(value)) => {
                push_python_escape(&mut output, value);
            }
            value => output.push(value),
        }
    }
    output.push(quote);
    output
}

fn python_printable(value: char, category: unicode_general_category::GeneralCategory) -> bool {
    use unicode_general_category::GeneralCategory;
    value == ' '
        || !matches!(
            category,
            GeneralCategory::Control
                | GeneralCategory::Format
                | GeneralCategory::Surrogate
                | GeneralCategory::PrivateUse
                | GeneralCategory::Unassigned
                | GeneralCategory::SpaceSeparator
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
        )
}

fn push_python_escape(output: &mut String, value: char) {
    let scalar = value as u32;
    if scalar <= 0xff {
        output.push_str(&format!("\\x{scalar:02x}"));
    } else if scalar <= 0xffff {
        output.push_str(&format!("\\u{scalar:04x}"));
    } else {
        output.push_str(&format!("\\U{scalar:08x}"));
    }
}

fn estimated_check_report_bytes(
    location: &str,
    contract: &str,
    issues: &[SourceFoundationSchemaIssue],
    diagnostic: &schema_diagnostics::Report,
) -> usize {
    let base = location
        .len()
        .saturating_add(contract.len())
        .saturating_add(std::mem::size_of::<SourceFoundationSchemaCheckReport>());
    let mapped_bytes = issues.iter().fold(base, |total, issue| {
        total
            .saturating_add(issue.location.len())
            .saturating_add(issue.schema_keyword.len())
            .saturating_add(issue.message.len())
            .saturating_add(std::mem::size_of::<SourceFoundationSchemaIssue>())
    });
    diagnostic.issues.iter().fold(mapped_bytes, |total, issue| {
        let instance_path_bytes = issue.instance_path.iter().fold(0usize, |bytes, segment| {
            bytes.saturating_add(match segment {
                schema_diagnostics::PathSegment::Property(value) => value.len(),
                schema_diagnostics::PathSegment::Index(_) => std::mem::size_of::<u64>(),
            })
        });
        let schema_path_bytes = issue.schema_path.iter().fold(0usize, |bytes, segment| {
            bytes.saturating_add(match segment {
                schema_diagnostics::PathSegment::Property(value) => value.len(),
                schema_diagnostics::PathSegment::Index(_) => std::mem::size_of::<u64>(),
            })
        });
        total
            .saturating_add(std::mem::size_of::<schema_diagnostics::Issue>())
            .saturating_add(issue.schema_keyword.len().saturating_mul(2))
            .saturating_add(instance_path_bytes.saturating_mul(2))
            .saturating_add(schema_path_bytes.saturating_mul(2))
    })
}
