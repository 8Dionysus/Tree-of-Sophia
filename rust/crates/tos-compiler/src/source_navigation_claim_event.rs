//! Bounded event-local Claim graph contributions.
//!
//! The source owner supplies one exact current Claim, the complete current ID
//! set for its shared Claim stream, and its already-resolved evidence closure.
//! This renderer rechecks the stream set, Claim row, schema, forms, event,
//! registries and identity sources before calling the existing Claim renderer.
//! The returned cohort is a derived replacement contribution, not source
//! admission, semantic acceptance, rights review, or global corpus completeness.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, RelativePath, canonical_bytes_v1};

use crate::{
    Error, Result,
    source_bibliographic::{
        BibliographicDocumentLimits, BibliographicForms, BibliographicLimits,
        SuppliedBibliographicClaimInputs, SuppliedEvidenceResolution, materialize_checked_form_set,
        render_supplied_claim, supplied_bibliographic_event, supplied_bibliographic_evidence,
        supplied_bibliographic_identity, supplied_bibliographic_maker_with_fallback,
        supplied_claim_navigation_descriptor,
    },
    source_bibliographic_render::digest as source_digest,
    source_bibliographic_selected_files::SelectedBibliographicFilesV1,
    source_bibliographic_versions::resolve_selected_claim_version_v1,
    source_navigation_event::{
        NavigationDependencyRoleV1, NavigationDependencyStateV1, NavigationDependencyV1,
    },
    source_witness_catalog::{SourceCatalogValidator, render_catalog_claim, render_catalog_record},
};

const PATH_BYTES: usize = 4096;
const EVENT_ID_BYTES: usize = 256;
const CLAIM_LINE_BYTES: usize = 1_048_576;
const TOTAL_DEPENDENCY_BYTES: usize = 128 * 1024 * 1024;
const MAX_CLAIM_EVENT_OWNERS: usize = 16_384;
const MAX_CLAIM_EVENT_STREAMS: usize = 4096;
const MAX_CLAIM_EVENT_TOTAL_OWNERS: usize = 65_536;
const MAX_CLAIM_REFRESH_CAUSES: usize = 256;

/// Exact selected source member. A present dependency must have one matching
/// member here; an absent dependency must not.
pub struct NavigationClaimEventSourceFileV1<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
}

/// One exact current Claim stream selected into the event cut. The inventory
/// parser validates every physical row once and retains only bounded rows and
/// stable IDs; callers share the result across all per-Claim render calls.
#[derive(Clone, Copy, Debug)]
pub struct NavigationClaimEventOwnerStreamInputV1<'a> {
    pub source_path: &'a str,
    pub source_bytes: &'a [u8],
}

/// Work charged once for one exact selected Claim stream inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimEventStreamInventoryCostV1 {
    pub source_path: String,
    pub stream_sha256: Digest256,
    pub rows_scanned: u64,
    pub owner_ids: Vec<String>,
}

/// One complete Claim stream in the affected-owner closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimEventAffectedOwnerStreamV1 {
    source_path: String,
    stream_sha256: Digest256,
    rows_scanned: u64,
    owner_ids: Vec<String>,
}

impl NavigationClaimEventAffectedOwnerStreamV1 {
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub fn stream_sha256(&self) -> Digest256 {
        self.stream_sha256
    }

    pub fn rows_scanned(&self) -> u64 {
        self.rows_scanned
    }

    pub fn owner_ids(&self) -> &[String] {
        &self.owner_ids
    }
}

/// Sealed stream-complete Claim owner closure for one mutation event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimEventAffectedOwnerManifestV1 {
    inventory_root: Digest256,
    mutation_source_path: String,
    mutation_claim_ids: Vec<String>,
    reverse_claim_ids: Vec<String>,
    streams: Vec<NavigationClaimEventAffectedOwnerStreamV1>,
    root: Digest256,
}

impl NavigationClaimEventAffectedOwnerManifestV1 {
    pub fn inventory_root(&self) -> Digest256 {
        self.inventory_root
    }

    pub fn mutation_source_path(&self) -> &str {
        &self.mutation_source_path
    }

    pub fn mutation_claim_ids(&self) -> &[String] {
        &self.mutation_claim_ids
    }

    pub fn reverse_claim_ids(&self) -> &[String] {
        &self.reverse_claim_ids
    }

    pub fn streams(&self) -> &[NavigationClaimEventAffectedOwnerStreamV1] {
        &self.streams
    }

    pub fn root(&self) -> Digest256 {
        self.root
    }

    /// Verify the exact stream-complete affected-owner manifest before a
    /// publisher binds it into its atomic event set.
    pub fn verify_integrity(&self) -> Result<()> {
        if !sorted_unique(&self.mutation_claim_ids)
            || self.mutation_claim_ids.is_empty()
            || self.mutation_claim_ids.len() > 32
            || !sorted_unique(&self.reverse_claim_ids)
            || self.reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
            || self
                .mutation_claim_ids
                .iter()
                .chain(&self.reverse_claim_ids)
                .any(|id| !id.starts_with("tos.claim.") || id.len() > PATH_BYTES)
            || self.streams.is_empty()
            || self.streams.len() > MAX_CLAIM_EVENT_STREAMS
            || self
                .streams
                .windows(2)
                .any(|pair| pair[0].source_path >= pair[1].source_path)
        {
            return Err(Error::Invalid(
                "navigation Claim affected-owner manifest shape",
            ));
        }
        let mut all_ids = BTreeSet::new();
        let mut mutation_stream_found = false;
        for stream in &self.streams {
            checked_path(&stream.source_path)?;
            if !is_claim_stream_path(&stream.source_path)
                || !sorted_unique(&stream.owner_ids)
                || stream.owner_ids.is_empty()
                || stream.owner_ids.len() > MAX_CLAIM_EVENT_OWNERS
                || stream.rows_scanned != stream.owner_ids.len() as u64
                || stream
                    .owner_ids
                    .iter()
                    .any(|id| !id.starts_with("tos.claim.") || id.len() > PATH_BYTES)
                || stream
                    .owner_ids
                    .iter()
                    .any(|id| !id.starts_with("tos.claim.") || id.len() > PATH_BYTES)
                || stream
                    .owner_ids
                    .iter()
                    .any(|id| !all_ids.insert(id.as_str()))
            {
                return Err(Error::Invalid(
                    "navigation Claim affected-owner stream shape",
                ));
            }
            if stream.source_path == self.mutation_source_path {
                mutation_stream_found = true;
                if self
                    .mutation_claim_ids
                    .iter()
                    .any(|id| stream.owner_ids.binary_search(id).is_err())
                {
                    return Err(Error::Invalid(
                        "navigation Claim affected-owner mutation members",
                    ));
                }
            }
        }
        if !mutation_stream_found
            || self
                .reverse_claim_ids
                .iter()
                .any(|id| !all_ids.contains(id.as_str()))
        {
            return Err(Error::Invalid(
                "navigation Claim affected-owner closure incomplete",
            ));
        }
        let root = affected_owner_manifest_root(
            self.inventory_root,
            &self.mutation_source_path,
            &self.mutation_claim_ids,
            &self.reverse_claim_ids,
            &self.streams,
        )?;
        if root != self.root {
            return Err(Error::Invalid(
                "navigation Claim affected-owner manifest root",
            ));
        }
        Ok(())
    }
}

/// Sealed current Claim-stream closure for a non-Claim dependency event.
/// Unlike a mutation manifest, it has no mutation stream or receipt members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimDependencyRefreshOwnerManifestV1 {
    inventory_root: Digest256,
    changed_dependency_path: RelativePath,
    changed_dependency_sha256: Digest256,
    closure_root: Digest256,
    reverse_claim_ids: Vec<String>,
    streams: Vec<NavigationClaimEventAffectedOwnerStreamV1>,
    root: Digest256,
}

/// One exact non-Claim dependency changed by a committed source event, with
/// that dependency's sorted reverse-Claim result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimDependencyRefreshCauseInputV1<'a> {
    pub changed_dependency_path: &'a str,
    pub changed_dependency_sha256: Digest256,
    pub reverse_claim_ids: &'a [String],
}

/// Canonical cause retained by a sealed multi-path refresh manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimDependencyRefreshCauseV1 {
    changed_dependency_path: RelativePath,
    changed_dependency_sha256: Digest256,
    reverse_claim_ids: Vec<String>,
}

impl NavigationClaimDependencyRefreshCauseV1 {
    pub fn changed_dependency_path(&self) -> &RelativePath {
        &self.changed_dependency_path
    }

    pub fn changed_dependency_sha256(&self) -> Digest256 {
        self.changed_dependency_sha256
    }

    pub fn reverse_claim_ids(&self) -> &[String] {
        &self.reverse_claim_ids
    }
}

/// Complete deduplicated current-owner replacement closure for a compound
/// non-Claim event. Every dependency cause retains its own reverse result;
/// `reverse_claim_ids` is their sorted union and `streams` contains every
/// current owner in each affected Claim stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimDependencyRefreshOwnerManifestSetV1 {
    inventory_root: Digest256,
    closure_root: Digest256,
    causes: Vec<NavigationClaimDependencyRefreshCauseV1>,
    reverse_claim_ids: Vec<String>,
    streams: Vec<NavigationClaimEventAffectedOwnerStreamV1>,
    root: Digest256,
}

impl NavigationClaimDependencyRefreshOwnerManifestSetV1 {
    pub fn inventory_root(&self) -> Digest256 {
        self.inventory_root
    }

    pub fn closure_root(&self) -> Digest256 {
        self.closure_root
    }

    pub fn causes(&self) -> &[NavigationClaimDependencyRefreshCauseV1] {
        &self.causes
    }

    pub fn reverse_claim_ids(&self) -> &[String] {
        &self.reverse_claim_ids
    }

    pub fn streams(&self) -> &[NavigationClaimEventAffectedOwnerStreamV1] {
        &self.streams
    }

    pub fn root(&self) -> Digest256 {
        self.root
    }

    pub fn verify_integrity(&self) -> Result<()> {
        if self.causes.is_empty()
            || self.causes.len() > MAX_CLAIM_REFRESH_CAUSES
            || self
                .causes
                .windows(2)
                .any(|pair| pair[0].changed_dependency_path >= pair[1].changed_dependency_path)
            || !sorted_unique(&self.reverse_claim_ids)
            || self.reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
            || self.streams.len() > MAX_CLAIM_EVENT_STREAMS
            || self
                .streams
                .windows(2)
                .any(|pair| pair[0].source_path >= pair[1].source_path)
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh set shape",
            ));
        }
        let mut union = BTreeSet::new();
        for cause in &self.causes {
            checked_path(cause.changed_dependency_path.as_str())?;
            if is_claim_stream_path(cause.changed_dependency_path.as_str())
                || !sorted_unique(&cause.reverse_claim_ids)
                || cause.reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
                || cause
                    .reverse_claim_ids
                    .iter()
                    .any(|id| !id.starts_with("tos.claim.") || id.len() > PATH_BYTES)
            {
                return Err(Error::Invalid(
                    "navigation Claim dependency-refresh cause shape",
                ));
            }
            for id in &cause.reverse_claim_ids {
                union.insert(id.as_str());
            }
        }
        let expected_union = union.into_iter().map(str::to_owned).collect::<Vec<_>>();
        if expected_union != self.reverse_claim_ids {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh reverse union",
            ));
        }
        let mut all_ids = BTreeSet::new();
        for stream in &self.streams {
            checked_path(&stream.source_path)?;
            if !is_claim_stream_path(&stream.source_path)
                || !sorted_unique(&stream.owner_ids)
                || stream.owner_ids.is_empty()
                || stream.owner_ids.len() > MAX_CLAIM_EVENT_OWNERS
                || stream.rows_scanned != stream.owner_ids.len() as u64
                || stream.owner_ids.iter().any(|id| {
                    !id.starts_with("tos.claim.")
                        || id.len() > PATH_BYTES
                        || !all_ids.insert(id.as_str())
                })
            {
                return Err(Error::Invalid(
                    "navigation Claim dependency-refresh affected stream shape",
                ));
            }
        }
        if self.reverse_claim_ids.is_empty() != self.streams.is_empty()
            || self
                .reverse_claim_ids
                .iter()
                .any(|id| !all_ids.contains(id.as_str()))
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh affected closure incomplete",
            ));
        }
        let root = dependency_refresh_owner_manifest_set_root(
            self.inventory_root,
            self.closure_root,
            &self.causes,
            &self.reverse_claim_ids,
            &self.streams,
        )?;
        if root != self.root {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh set root",
            ));
        }
        Ok(())
    }
}

impl NavigationClaimDependencyRefreshOwnerManifestV1 {
    pub fn inventory_root(&self) -> Digest256 {
        self.inventory_root
    }

    pub fn changed_dependency_path(&self) -> &RelativePath {
        &self.changed_dependency_path
    }

    pub fn changed_dependency_sha256(&self) -> Digest256 {
        self.changed_dependency_sha256
    }

    pub fn closure_root(&self) -> Digest256 {
        self.closure_root
    }

    pub fn reverse_claim_ids(&self) -> &[String] {
        &self.reverse_claim_ids
    }

    pub fn streams(&self) -> &[NavigationClaimEventAffectedOwnerStreamV1] {
        &self.streams
    }

    pub fn root(&self) -> Digest256 {
        self.root
    }

    pub fn verify_integrity(&self) -> Result<()> {
        if is_claim_stream_path(self.changed_dependency_path.as_str())
            || !sorted_unique(&self.reverse_claim_ids)
            || self.reverse_claim_ids.is_empty()
            || self.reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
            || self.streams.is_empty()
            || self.streams.len() > MAX_CLAIM_EVENT_STREAMS
            || self
                .streams
                .windows(2)
                .any(|pair| pair[0].source_path >= pair[1].source_path)
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh manifest shape",
            ));
        }
        let mut all_ids = BTreeSet::new();
        for stream in &self.streams {
            checked_path(&stream.source_path)?;
            if !is_claim_stream_path(&stream.source_path)
                || !sorted_unique(&stream.owner_ids)
                || stream.owner_ids.is_empty()
                || stream.owner_ids.len() > MAX_CLAIM_EVENT_OWNERS
                || stream.rows_scanned != stream.owner_ids.len() as u64
                || stream
                    .owner_ids
                    .iter()
                    .any(|id| !all_ids.insert(id.as_str()))
            {
                return Err(Error::Invalid(
                    "navigation Claim dependency-refresh stream shape",
                ));
            }
        }
        if self
            .reverse_claim_ids
            .iter()
            .any(|id| !all_ids.contains(id.as_str()))
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh closure incomplete",
            ));
        }
        let root = dependency_refresh_owner_manifest_root(
            self.inventory_root,
            &self.changed_dependency_path,
            self.changed_dependency_sha256,
            self.closure_root,
            &self.reverse_claim_ids,
            &self.streams,
        )?;
        if root != self.root {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh manifest root",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct NavigationClaimEventOwnerRowV1 {
    source_line: u64,
    exact_row_bytes: Vec<u8>,
    claim: Value,
}

/// Borrowed exact owner row from a parsed selected Claim stream.
#[derive(Clone, Copy, Debug)]
pub struct NavigationClaimEventOwnerRowViewV1<'a> {
    pub source_path: &'a str,
    pub claim_id: &'a str,
    pub source_line: u64,
    pub exact_row_bytes: &'a [u8],
    pub claim: &'a Value,
}

#[derive(Clone, Debug)]
struct NavigationClaimEventOwnerStreamV1 {
    stream_sha256: Digest256,
    rows: BTreeMap<String, NavigationClaimEventOwnerRowV1>,
}

/// Immutable, parsed current Claim stream set shared by every owner renderer
/// in one exact event read cut. Duplicate IDs across selected streams refuse.
#[derive(Clone, Debug)]
pub struct NavigationClaimEventOwnerInventoryV1 {
    streams: BTreeMap<String, NavigationClaimEventOwnerStreamV1>,
    owner_paths: BTreeMap<String, String>,
    historical_claims: BTreeMap<String, NavigationClaimEventOwnerRowV1>,
    historical_owner_paths: BTreeMap<String, String>,
    scan_costs: Vec<NavigationClaimEventStreamInventoryCostV1>,
    root: Digest256,
}

impl NavigationClaimEventOwnerInventoryV1 {
    pub fn root(&self) -> Digest256 {
        self.root
    }

    pub fn scan_costs(&self) -> &[NavigationClaimEventStreamInventoryCostV1] {
        &self.scan_costs
    }

    pub fn owner_ids(&self, source_path: &str) -> Option<Vec<String>> {
        self.streams
            .get(source_path)
            .map(|stream| stream.rows.keys().cloned().collect())
    }

    /// Return one exact owner row without reparsing its selected stream.
    pub fn owner<'a>(
        &'a self,
        source_path: &'a str,
        claim_id: &'a str,
    ) -> Option<NavigationClaimEventOwnerRowViewV1<'a>> {
        let row = self.row(source_path, claim_id)?;
        Some(NavigationClaimEventOwnerRowViewV1 {
            source_path,
            claim_id,
            source_line: row.source_line,
            exact_row_bytes: &row.exact_row_bytes,
            claim: &row.claim,
        })
    }

    /// Resolve a stable Claim ID to its exact selected owner row.
    pub fn claim_by_id<'a>(
        &'a self,
        claim_id: &'a str,
    ) -> Option<NavigationClaimEventOwnerRowViewV1<'a>> {
        if let Some(source_path) = self.owner_paths.get(claim_id) {
            return self.owner(source_path, claim_id);
        }
        let row = self.historical_claims.get(claim_id)?;
        let source_path = self.historical_owner_paths.get(claim_id)?;
        Some(NavigationClaimEventOwnerRowViewV1 {
            source_path,
            claim_id,
            source_line: row.source_line,
            exact_row_bytes: &row.exact_row_bytes,
            claim: &row.claim,
        })
    }

    /// Expand exact reverse-index Claim IDs into complete, deduplicated stream
    /// owners, always including the stream changed by the mutation receipt.
    pub fn affected_owner_streams(
        &self,
        reverse_claim_ids: &[String],
        mutation_source_path: &str,
        mutation_claim_ids: &[String],
    ) -> Result<NavigationClaimEventAffectedOwnerManifestV1> {
        if !sorted_unique(reverse_claim_ids)
            || reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
            || !sorted_unique(mutation_claim_ids)
            || mutation_claim_ids.is_empty()
            || mutation_claim_ids.len() > 32
        {
            return Err(Error::Invalid(
                "navigation Claim affected-owner input shape",
            ));
        }
        let mutation_source_path = checked_path(mutation_source_path)?;
        let mutation_stream = self
            .stream(mutation_source_path.as_str())
            .ok_or(Error::Invalid(
                "navigation Claim mutation stream absent from inventory",
            ))?;
        if mutation_claim_ids
            .iter()
            .any(|id| !mutation_stream.rows.contains_key(id))
        {
            return Err(Error::Invalid(
                "navigation Claim mutation members absent from stream",
            ));
        }
        let mut paths = BTreeSet::from([mutation_source_path.as_str().to_owned()]);
        for claim_id in reverse_claim_ids {
            let path = self.owner_paths.get(claim_id).ok_or(Error::Invalid(
                "navigation Claim reverse owner absent from inventory",
            ))?;
            paths.insert(path.clone());
        }
        let streams = paths
            .iter()
            .map(|path| {
                let inventory_stream = self
                    .stream(path)
                    .ok_or(Error::Invalid("navigation Claim affected stream absent"))?;
                Ok(NavigationClaimEventAffectedOwnerStreamV1 {
                    source_path: path.clone(),
                    stream_sha256: inventory_stream.stream_sha256,
                    rows_scanned: inventory_stream.rows.len() as u64,
                    owner_ids: inventory_stream.rows.keys().cloned().collect(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let root = affected_owner_manifest_root(
            self.root,
            mutation_source_path.as_str(),
            mutation_claim_ids,
            reverse_claim_ids,
            &streams,
        )?;
        let manifest = NavigationClaimEventAffectedOwnerManifestV1 {
            inventory_root: self.root,
            mutation_source_path: mutation_source_path.as_str().to_owned(),
            mutation_claim_ids: mutation_claim_ids.to_vec(),
            reverse_claim_ids: reverse_claim_ids.to_vec(),
            streams,
            root,
        };
        manifest.verify_integrity()?;
        Ok(manifest)
    }

    /// Expand exact reverse-index Claim IDs into complete current Claim
    /// streams for a non-Claim dependency refresh. No changed Claim stream or
    /// mutation receipt is invented for this cause.
    pub fn affected_dependency_streams(
        &self,
        reverse_claim_ids: &[String],
        changed_dependency_path: &str,
        changed_dependency_sha256: Digest256,
        closure_root: Digest256,
    ) -> Result<NavigationClaimDependencyRefreshOwnerManifestV1> {
        let changed_dependency_path = checked_path(changed_dependency_path)?;
        if is_claim_stream_path(changed_dependency_path.as_str())
            || !sorted_unique(reverse_claim_ids)
            || reverse_claim_ids.is_empty()
            || reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh input shape",
            ));
        }
        let mut paths = BTreeSet::new();
        for claim_id in reverse_claim_ids {
            let path = self.owner_paths.get(claim_id).ok_or(Error::Invalid(
                "navigation Claim dependency-refresh owner absent",
            ))?;
            paths.insert(path.clone());
        }
        let streams = paths
            .iter()
            .map(|path| {
                let stream = self
                    .stream(path)
                    .ok_or(Error::Invalid("navigation Claim refresh stream absent"))?;
                Ok(NavigationClaimEventAffectedOwnerStreamV1 {
                    source_path: path.clone(),
                    stream_sha256: stream.stream_sha256,
                    rows_scanned: stream.rows.len() as u64,
                    owner_ids: stream.rows.keys().cloned().collect(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let root = dependency_refresh_owner_manifest_root(
            self.root,
            &changed_dependency_path,
            changed_dependency_sha256,
            closure_root,
            reverse_claim_ids,
            &streams,
        )?;
        let manifest = NavigationClaimDependencyRefreshOwnerManifestV1 {
            inventory_root: self.root,
            changed_dependency_path,
            changed_dependency_sha256,
            closure_root,
            reverse_claim_ids: reverse_claim_ids.to_vec(),
            streams,
            root,
        };
        manifest.verify_integrity()?;
        Ok(manifest)
    }

    /// Build one complete owner-set manifest for a compound dependency event.
    /// Causes are path-sorted and preserve each reverse-ID set independently;
    /// affected streams are the deduplicated union of every reverse owner.
    /// Empty per-cause reverse sets are valid, and an event with no reverse
    /// Claim owners returns an authenticated manifest with no streams.
    pub fn affected_dependency_stream_set(
        &self,
        causes: &[NavigationClaimDependencyRefreshCauseInputV1<'_>],
        closure_root: Digest256,
    ) -> Result<NavigationClaimDependencyRefreshOwnerManifestSetV1> {
        if causes.is_empty() || causes.len() > MAX_CLAIM_REFRESH_CAUSES {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh cause count",
            ));
        }
        let mut sealed_causes = Vec::with_capacity(causes.len());
        let mut reverse_union = BTreeSet::new();
        let mut stream_paths = BTreeSet::new();
        let mut prior_path: Option<RelativePath> = None;
        for cause in causes {
            let path = checked_path(cause.changed_dependency_path)?;
            if is_claim_stream_path(path.as_str())
                || prior_path.as_ref().is_some_and(|prior| prior >= &path)
                || !sorted_unique(cause.reverse_claim_ids)
                || cause.reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
                || cause
                    .reverse_claim_ids
                    .iter()
                    .any(|id| !id.starts_with("tos.claim.") || id.len() > PATH_BYTES)
            {
                return Err(Error::Invalid(
                    "navigation Claim dependency-refresh cause input shape",
                ));
            }
            for claim_id in cause.reverse_claim_ids {
                let owner_path = self.owner_paths.get(claim_id).ok_or(Error::Invalid(
                    "navigation Claim dependency-refresh owner absent from inventory",
                ))?;
                reverse_union.insert(claim_id.clone());
                stream_paths.insert(owner_path.clone());
            }
            prior_path = Some(path.clone());
            sealed_causes.push(NavigationClaimDependencyRefreshCauseV1 {
                changed_dependency_path: path,
                changed_dependency_sha256: cause.changed_dependency_sha256,
                reverse_claim_ids: cause.reverse_claim_ids.to_vec(),
            });
        }
        let reverse_claim_ids = reverse_union.into_iter().collect::<Vec<_>>();
        let streams = stream_paths
            .iter()
            .map(|path| {
                let stream = self
                    .stream(path)
                    .ok_or(Error::Invalid("navigation Claim refresh stream absent"))?;
                Ok(NavigationClaimEventAffectedOwnerStreamV1 {
                    source_path: path.clone(),
                    stream_sha256: stream.stream_sha256,
                    rows_scanned: stream.rows.len() as u64,
                    owner_ids: stream.rows.keys().cloned().collect(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let root = dependency_refresh_owner_manifest_set_root(
            self.root,
            closure_root,
            &sealed_causes,
            &reverse_claim_ids,
            &streams,
        )?;
        let manifest = NavigationClaimDependencyRefreshOwnerManifestSetV1 {
            inventory_root: self.root,
            closure_root,
            causes: sealed_causes,
            reverse_claim_ids,
            streams,
            root,
        };
        manifest.verify_integrity()?;
        Ok(manifest)
    }

    fn row(&self, source_path: &str, claim_id: &str) -> Option<&NavigationClaimEventOwnerRowV1> {
        self.streams.get(source_path)?.rows.get(claim_id)
    }

    fn row_at_line(
        &self,
        source_path: &str,
        source_line: u64,
    ) -> Option<&NavigationClaimEventOwnerRowV1> {
        self.streams
            .get(source_path)?
            .rows
            .values()
            .find(|row| row.source_line == source_line)
    }

    fn stream(&self, source_path: &str) -> Option<&NavigationClaimEventOwnerStreamV1> {
        self.streams.get(source_path)
    }
}

/// Exact current source identity consumed by the Claim resolver.
pub struct NavigationClaimIdentitySourceV1<'a> {
    pub path: &'a str,
    pub schema_path: &'a str,
    pub schema_bytes: &'a [u8],
    pub source_bytes: &'a [u8],
    pub forms_path: &'a str,
    pub forms_schema_path: Option<&'a str>,
    pub forms_schema_bytes: Option<&'a [u8]>,
    pub forms_bytes: Option<&'a [u8]>,
}

/// Checked per-Claim source-owner envelope for one committed event.
///
/// The producer supplies the resolved inputs used by the existing
/// bibliographic Claim renderer. This function re-derives the Claim source
/// entry from the exact stream line, materializes the exact form set, verifies
/// current identity records against their selected bytes, and binds every
/// selected byte to the dependency manifest before sealing the projection.
pub struct NavigationClaimEventInputV1<'a> {
    pub validator: &'a SourceCatalogValidator<'a>,
    pub event_id: &'a str,
    pub event_sequence: u64,
    pub source_head_sequence: u64,
    pub predecessor_source_revision: Digest256,
    pub predecessor_cohort_root: Option<Digest256>,
    /// Exact Claim stream named by the event's mutation receipt. `source_path`
    /// may name another stream in the reverse-dependent replacement closure.
    pub mutation_source_path: &'a str,
    pub affected_owner_manifest: &'a NavigationClaimEventAffectedOwnerManifestV1,
    pub source_path: &'a str,
    pub source_line: u64,
    pub source_bytes: &'a [u8],
    /// Complete sorted Claim ID set produced by the owner readset's scan of
    /// this exact current stream. The renderer re-parses `source_bytes` and
    /// requires this set to match every current stream member.
    pub owner_claim_ids: &'a [String],
    pub claim_schema_path: &'a str,
    pub claim_schema_bytes: &'a [u8],
    pub forms_path: &'a str,
    pub forms_schema_path: &'a str,
    pub forms_schema_bytes: Option<&'a [u8]>,
    pub forms_bytes: Option<&'a [u8]>,
    pub history_path: &'a str,
    pub history_bytes: Option<&'a [u8]>,
    pub provenance_event_path: &'a str,
    pub provenance_event_bytes: &'a [u8],
    pub provenance_event_schema_path: &'a str,
    pub provenance_event_schema_bytes: &'a [u8],
    pub provenance_receipt_path: &'a str,
    pub provenance_receipt_bytes: &'a [u8],
    pub entity_registry_path: &'a str,
    pub entity_registry_bytes: &'a [u8],
    pub entity_registry_schema_path: &'a str,
    pub entity_registry_schema_bytes: &'a [u8],
    pub relation_registry_path: &'a str,
    pub relation_registry_bytes: &'a [u8],
    pub relation_registry_schema_path: &'a str,
    pub relation_registry_schema_bytes: &'a [u8],
    /// Exact mutation receipt from the CMD event payload. Its create claims or
    /// revise source identity derives only the IDs directly changed by this
    /// event; full reverse-dependent closure is resolved through the shared
    /// owner inventory before these per-stream cohorts are produced.
    pub mutation_receipt_path: &'a str,
    pub mutation_receipt_bytes: &'a [u8],
    pub identity_sources: &'a [NavigationClaimIdentitySourceV1<'a>],
    pub source_files: &'a [NavigationClaimEventSourceFileV1<'a>],
    pub dependencies: &'a [NavigationDependencyV1],
    /// Exact selected current/history/archive/form files and explicit
    /// absence observations used to resolve every external Claim endpoint.
    pub selected_files: &'a SelectedBibliographicFilesV1,
    pub forms_materializer: &'a mut dyn BibliographicForms,
    pub claim_inputs: SuppliedBibliographicClaimInputs<'a>,
}

/// Semantic source closure shared by Claim mutation and dependency refresh.
/// The refresh input nests this type but deliberately has no mutation receipt.
pub struct NavigationClaimEventCommonInputV1<'a> {
    pub validator: &'a SourceCatalogValidator<'a>,
    pub event_id: &'a str,
    pub event_sequence: u64,
    pub source_head_sequence: u64,
    pub predecessor_source_revision: Digest256,
    pub predecessor_cohort_root: Option<Digest256>,
    pub source_path: &'a str,
    pub source_line: u64,
    pub source_bytes: &'a [u8],
    pub owner_claim_ids: &'a [String],
    pub claim_schema_path: &'a str,
    pub claim_schema_bytes: &'a [u8],
    pub forms_path: &'a str,
    pub forms_schema_path: &'a str,
    pub forms_schema_bytes: Option<&'a [u8]>,
    pub forms_bytes: Option<&'a [u8]>,
    pub history_path: &'a str,
    pub history_bytes: Option<&'a [u8]>,
    pub provenance_event_path: &'a str,
    pub provenance_event_bytes: &'a [u8],
    pub provenance_event_schema_path: &'a str,
    pub provenance_event_schema_bytes: &'a [u8],
    pub provenance_receipt_path: &'a str,
    pub provenance_receipt_bytes: &'a [u8],
    pub entity_registry_path: &'a str,
    pub entity_registry_bytes: &'a [u8],
    pub entity_registry_schema_path: &'a str,
    pub entity_registry_schema_bytes: &'a [u8],
    pub relation_registry_path: &'a str,
    pub relation_registry_bytes: &'a [u8],
    pub relation_registry_schema_path: &'a str,
    pub relation_registry_schema_bytes: &'a [u8],
    pub identity_sources: &'a [NavigationClaimIdentitySourceV1<'a>],
    pub source_files: &'a [NavigationClaimEventSourceFileV1<'a>],
    pub dependencies: &'a [NavigationDependencyV1],
    pub selected_files: &'a SelectedBibliographicFilesV1,
    pub forms_materializer: &'a mut dyn BibliographicForms,
    pub claim_inputs: SuppliedBibliographicClaimInputs<'a>,
}

/// Exact cause and reverse-closure binding for a non-Claim source event.
pub struct NavigationClaimDependencyRefreshInputV1<'a> {
    pub common: NavigationClaimEventCommonInputV1<'a>,
    pub changed_dependency_path: &'a str,
    pub changed_dependency_sha256: Digest256,
    /// Opaque root from the exact selected CMD event's
    /// SourceRevisionClaimNavigationClosure.
    pub closure_root: Digest256,
    pub affected_owner_manifest: &'a NavigationClaimDependencyRefreshOwnerManifestV1,
}

/// Refresh cause for a compound event. The owner manifest set carries all
/// changed paths, their individual reverse results, and the complete stream
/// union, so the producer supplies it once for every Claim contribution.
pub struct NavigationClaimDependencyRefreshSetInputV1<'a> {
    pub common: NavigationClaimEventCommonInputV1<'a>,
    pub affected_owner_manifest_set: &'a NavigationClaimDependencyRefreshOwnerManifestSetV1,
}

enum NavigationClaimEventCauseInputV1<'a> {
    Mutation {
        mutation_source_path: &'a str,
        mutation_receipt_path: &'a str,
        mutation_receipt_bytes: &'a [u8],
        affected_owner_manifest: &'a NavigationClaimEventAffectedOwnerManifestV1,
    },
    DependencyRefresh {
        affected_owner_manifest_set: &'a NavigationClaimDependencyRefreshOwnerManifestSetV1,
    },
}

impl<'a> NavigationClaimEventInputV1<'a> {
    fn into_common(self) -> NavigationClaimEventCommonInputV1<'a> {
        NavigationClaimEventCommonInputV1 {
            validator: self.validator,
            event_id: self.event_id,
            event_sequence: self.event_sequence,
            source_head_sequence: self.source_head_sequence,
            predecessor_source_revision: self.predecessor_source_revision,
            predecessor_cohort_root: self.predecessor_cohort_root,
            source_path: self.source_path,
            source_line: self.source_line,
            source_bytes: self.source_bytes,
            owner_claim_ids: self.owner_claim_ids,
            claim_schema_path: self.claim_schema_path,
            claim_schema_bytes: self.claim_schema_bytes,
            forms_path: self.forms_path,
            forms_schema_path: self.forms_schema_path,
            forms_schema_bytes: self.forms_schema_bytes,
            forms_bytes: self.forms_bytes,
            history_path: self.history_path,
            history_bytes: self.history_bytes,
            provenance_event_path: self.provenance_event_path,
            provenance_event_bytes: self.provenance_event_bytes,
            provenance_event_schema_path: self.provenance_event_schema_path,
            provenance_event_schema_bytes: self.provenance_event_schema_bytes,
            provenance_receipt_path: self.provenance_receipt_path,
            provenance_receipt_bytes: self.provenance_receipt_bytes,
            entity_registry_path: self.entity_registry_path,
            entity_registry_bytes: self.entity_registry_bytes,
            entity_registry_schema_path: self.entity_registry_schema_path,
            entity_registry_schema_bytes: self.entity_registry_schema_bytes,
            relation_registry_path: self.relation_registry_path,
            relation_registry_bytes: self.relation_registry_bytes,
            relation_registry_schema_path: self.relation_registry_schema_path,
            relation_registry_schema_bytes: self.relation_registry_schema_bytes,
            identity_sources: self.identity_sources,
            source_files: self.source_files,
            dependencies: self.dependencies,
            selected_files: self.selected_files,
            forms_materializer: self.forms_materializer,
            claim_inputs: self.claim_inputs,
        }
    }
}

impl NavigationClaimDependencyRefreshCohortV1 {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
    pub fn source_event_id(&self) -> &str {
        &self.source_event_id
    }
    pub fn source_event_version(&self) -> u64 {
        self.source_event_version
    }
    pub fn source_revision(&self) -> Digest256 {
        self.source_revision
    }
    pub fn predecessor_source_revision(&self) -> Digest256 {
        self.predecessor_source_revision
    }
    pub fn predecessor_cohort_root(&self) -> Option<Digest256> {
        self.predecessor_cohort_root
    }
    pub fn source_path(&self) -> &RelativePath {
        &self.source_path
    }
    pub fn record_id(&self) -> &str {
        &self.record_id
    }
    pub fn source_line(&self) -> u64 {
        self.source_line
    }
    pub fn dependency_root(&self) -> Digest256 {
        self.dependency_root
    }
    pub fn input_root(&self) -> Digest256 {
        self.input_root
    }
    pub fn output_root(&self) -> Digest256 {
        self.output_root
    }
    pub fn event_basis_root(&self) -> Digest256 {
        self.event_basis_root
    }
    pub fn owner_inventory_root(&self) -> Digest256 {
        self.owner_inventory_root
    }
    pub fn owner_inventory_scan_costs(&self) -> &[NavigationClaimEventStreamInventoryCostV1] {
        &self.owner_inventory_scan_costs
    }
    pub fn event_claim_owner_ids(&self) -> &[String] {
        &self.event_claim_owner_ids
    }
    pub fn claim_stream_sha256(&self) -> Digest256 {
        self.claim_stream_sha256
    }
    pub fn claim_stream_rows_scanned(&self) -> u64 {
        self.claim_stream_rows_scanned
    }
    pub fn proof(&self) -> &NavigationClaimDependencyRefreshProofV1 {
        &self.proof
    }
    pub fn dependencies(&self) -> &[NavigationDependencyV1] {
        &self.dependencies
    }
    pub fn projection(&self) -> &NavigationClaimEventProjectionV1 {
        &self.projection
    }
    pub fn external_endpoint_ids(&self) -> &[String] {
        &self.external_endpoint_ids
    }
    pub fn node_visibility(&self) -> &[bool] {
        &self.node_visibility
    }
    pub fn edge_visibility(&self) -> &[bool] {
        &self.edge_visibility
    }

    pub fn verify_integrity(&self) -> Result<NavigationClaimDependencyRefreshIntegrityV1<'_>> {
        if !sorted_unique(&self.event_claim_owner_ids)
            || self.event_claim_owner_ids.is_empty()
            || self.event_claim_owner_ids.len() > MAX_CLAIM_EVENT_OWNERS
            || self
                .event_claim_owner_ids
                .binary_search(&self.record_id)
                .is_err()
            || self.node_visibility.len() != self.projection.nodes.len()
            || self.edge_visibility.len() != self.projection.edges.len()
            || self.proof.event_id != self.event_id
            || self.proof.event_sequence != self.event_sequence
            || self.proof.claim_stream_path != self.source_path
            || self.proof.claim_stream_sha256 != self.claim_stream_sha256
            || self.proof.claim_id != self.record_id
            || self.proof.claim_source_line != self.source_line
            || self.proof.claim_version == 0
            || !sorted_unique(&self.proof.reverse_claim_ids)
            || self.proof.reverse_claim_ids.is_empty()
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh cohort shape",
            ));
        }
        if self.proof.binding_root != dependency_refresh_proof_root(&self.proof)?
            || self.event_basis_root != self.proof.binding_root
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh proof root",
            ));
        }
        let inventory_root = owner_inventory_root(&self.owner_inventory_scan_costs)?;
        if inventory_root != self.owner_inventory_root
            || !self.owner_inventory_scan_costs.iter().any(|cost| {
                cost.source_path == self.source_path.as_str()
                    && cost.stream_sha256 == self.claim_stream_sha256
                    && cost.owner_ids == self.event_claim_owner_ids
            })
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh owner inventory binding",
            ));
        }
        if self.proof.changed_dependencies.iter().any(|cause| {
            dependency_present_sha256_any_role(&self.dependencies, cause.changed_dependency_path())
                .map(|sha256| sha256 != cause.changed_dependency_sha256())
                .unwrap_or(true)
        }) || dependency_present_digest(
            &self.dependencies,
            &self.proof.claim_stream_path,
            NavigationDependencyRoleV1::Record,
        )? != self.proof.claim_stream_sha256
            || dependency_present_digest(
                &self.dependencies,
                &self.proof.provenance_event_path,
                NavigationDependencyRoleV1::Event,
            )? != self.proof.provenance_event_sha256
            || dependency_present_digest(
                &self.dependencies,
                &self.proof.provenance_receipt_path,
                NavigationDependencyRoleV1::EventReceipt,
            )? != self.proof.provenance_receipt_sha256
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh evidence dependency",
            ));
        }
        match (
            self.proof.history_path.as_ref(),
            self.proof.history_sha256,
            self.proof.latest_revision_receipt_sha256,
        ) {
            (None, None, None) if self.proof.claim_version == 1 => {}
            (Some(_), Some(_), None) if self.proof.claim_version == 1 => {}
            (Some(path), Some(sha256), Some(_)) => {
                if dependency_present_digest(
                    &self.dependencies,
                    path,
                    NavigationDependencyRoleV1::RecordHistory,
                )? != sha256
                {
                    return Err(Error::Invalid(
                        "navigation Claim dependency-refresh history dependency",
                    ));
                }
            }
            _ => {
                return Err(Error::Invalid(
                    "navigation Claim dependency-refresh retained history shape",
                ));
            }
        }
        let dependency_root = dependency_root(&self.dependencies)?;
        let (rows, external) = output_rows(
            &self.projection,
            &self.node_visibility,
            &self.edge_visibility,
        )?;
        if external != self.external_endpoint_ids {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh external endpoint set",
            ));
        }
        let output_root = output_root(
            &rows,
            &self.projection.diagnostics,
            self.trace_sha256,
            &self.external_endpoint_ids,
        )?;
        let input_root = dependency_refresh_input_root(
            &self.event_id,
            self.event_sequence,
            &self.source_event_id,
            self.source_event_version,
            self.predecessor_source_revision,
            self.source_revision,
            self.predecessor_cohort_root,
            &self.source_path,
            self.source_line,
            &self.record_id,
            dependency_root,
            output_root,
            self.event_basis_root,
            self.owner_inventory_root,
            self.claim_stream_rows_scanned,
        )?;
        if dependency_root != self.dependency_root
            || output_root != self.output_root
            || input_root != self.input_root
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh cohort root mismatch",
            ));
        }
        Ok(NavigationClaimDependencyRefreshIntegrityV1 {
            event_id: &self.event_id,
            event_sequence: self.event_sequence,
            source_event_id: &self.source_event_id,
            source_event_version: self.source_event_version,
            predecessor_source_revision: self.predecessor_source_revision,
            source_revision: self.source_revision,
            predecessor_cohort_root: self.predecessor_cohort_root,
            source_path: self.source_path.as_str(),
            record_id: &self.record_id,
            source_line: self.source_line,
            dependency_root,
            input_root,
            output_root,
            event_basis_root: self.event_basis_root,
            owner_inventory_root: self.owner_inventory_root,
            owner_inventory_scan_costs: &self.owner_inventory_scan_costs,
            proof: &self.proof,
            external_endpoint_ids: &self.external_endpoint_ids,
            rows,
            node_visibility: &self.node_visibility,
            edge_visibility: &self.edge_visibility,
        })
    }

    pub fn budget_cost(&self) -> Result<NavigationClaimEventBudgetCostV1> {
        self.verify_integrity()?;
        navigation_claim_event_budget_cost(
            &self.projection,
            &self.dependencies,
            &self.source_path,
            self.claim_stream_sha256,
            self.claim_stream_rows_scanned,
        )
    }
}

impl NavigationClaimEventContributionV1 {
    pub fn basis(&self) -> NavigationClaimEventBasisV1 {
        match self {
            Self::Mutation(_) => NavigationClaimEventBasisV1::Mutation,
            Self::DependencyRefresh(_) => NavigationClaimEventBasisV1::DependencyRefresh,
        }
    }
    pub fn event_id(&self) -> &str {
        match self {
            Self::Mutation(cohort) => cohort.event_id(),
            Self::DependencyRefresh(cohort) => cohort.event_id(),
        }
    }
    pub fn event_sequence(&self) -> u64 {
        match self {
            Self::Mutation(cohort) => cohort.event_sequence(),
            Self::DependencyRefresh(cohort) => cohort.event_sequence(),
        }
    }
    pub fn source_revision(&self) -> Digest256 {
        match self {
            Self::Mutation(cohort) => cohort.source_revision(),
            Self::DependencyRefresh(cohort) => cohort.source_revision(),
        }
    }
    pub fn predecessor_source_revision(&self) -> Digest256 {
        match self {
            Self::Mutation(cohort) => cohort.predecessor_source_revision(),
            Self::DependencyRefresh(cohort) => cohort.predecessor_source_revision(),
        }
    }
    pub fn source_path(&self) -> &RelativePath {
        match self {
            Self::Mutation(cohort) => cohort.source_path(),
            Self::DependencyRefresh(cohort) => cohort.source_path(),
        }
    }
    pub fn record_id(&self) -> &str {
        match self {
            Self::Mutation(cohort) => cohort.record_id(),
            Self::DependencyRefresh(cohort) => cohort.record_id(),
        }
    }
    pub fn verify_integrity(&self) -> Result<NavigationClaimEventContributionIntegrityV1<'_>> {
        match self {
            Self::Mutation(cohort) => Ok(NavigationClaimEventContributionIntegrityV1::Mutation(
                cohort.verify_integrity()?,
            )),
            Self::DependencyRefresh(cohort) => Ok(
                NavigationClaimEventContributionIntegrityV1::DependencyRefresh(
                    cohort.verify_integrity()?,
                ),
            ),
        }
    }
    pub fn budget_cost(&self) -> Result<NavigationClaimEventBudgetCostV1> {
        match self {
            Self::Mutation(cohort) => cohort.budget_cost(),
            Self::DependencyRefresh(cohort) => cohort.budget_cost(),
        }
    }
}

/// Prepare every exact selected Claim stream once for a multi-owner event.
/// The input is normally the final event context's present Claim stream
/// members; it may also contain addressed Claim streams used as edge targets.
pub fn prepare_navigation_claim_event_owner_inventory_v1(
    source_files: &[NavigationClaimEventSourceFileV1<'_>],
    limits: BibliographicLimits,
) -> Result<NavigationClaimEventOwnerInventoryV1> {
    prepare_navigation_claim_event_owner_inventory_with_endpoints_v1(source_files, &[], limits)
}

/// Prepare current Claim owners plus only the exact historical Claim endpoint
/// streams selected by the authenticated Claim identity lookup. Historical
/// rows support edge visibility lookup; they never join current replacement
/// owner manifests.
pub fn prepare_navigation_claim_event_owner_inventory_with_endpoints_v1(
    source_files: &[NavigationClaimEventSourceFileV1<'_>],
    historical_endpoint_paths: &[String],
    limits: BibliographicLimits,
) -> Result<NavigationClaimEventOwnerInventoryV1> {
    limits.validate()?;
    if source_files.is_empty() || source_files.len() > limits.catalog.max_files {
        return Err(Error::Budget(
            "navigation Claim inventory source file count",
        ));
    }
    let mut selected = BTreeMap::<String, &[u8]>::new();
    let mut total_source_bytes = 0usize;
    for file in source_files {
        let path = checked_path(file.path)?;
        total_source_bytes = total_source_bytes
            .checked_add(file.bytes.len())
            .filter(|bytes| *bytes <= TOTAL_DEPENDENCY_BYTES)
            .ok_or(Error::Budget("navigation Claim inventory source bytes"))?;
        if selected
            .insert(path.as_str().to_owned(), file.bytes)
            .is_some()
        {
            return Err(Error::Invalid(
                "navigation Claim inventory duplicate source file",
            ));
        }
    }
    if !sorted_unique(historical_endpoint_paths)
        || historical_endpoint_paths.len() > MAX_CLAIM_EVENT_STREAMS
    {
        return Err(Error::Invalid(
            "navigation Claim historical endpoint path shape",
        ));
    }
    let streams = selected
        .iter()
        .filter(|(path, _)| is_claim_stream_path(path))
        .map(|(path, bytes)| NavigationClaimEventOwnerStreamInputV1 {
            source_path: path.as_str(),
            source_bytes: bytes,
        })
        .collect::<Vec<_>>();
    let mut inventory = owner_inventory_from_streams(&streams, limits.catalog.max_row_bytes)?;
    let mut historical_claims = BTreeMap::new();
    let mut historical_owner_paths = BTreeMap::new();
    for path_text in historical_endpoint_paths {
        let path = checked_path(path_text)?;
        if !is_historical_claim_endpoint_path(path.as_str()) {
            return Err(Error::Invalid(
                "navigation Claim historical endpoint owner path",
            ));
        }
        let bytes = selected.get(path.as_str()).copied().ok_or(Error::Invalid(
            "navigation Claim historical endpoint bytes absent",
        ))?;
        let (rows, rows_scanned) =
            parse_historical_claim_endpoint_stream(bytes, limits.catalog.max_row_bytes)?;
        for (id, row) in rows {
            if inventory.owner_paths.contains_key(&id)
                || historical_claims.insert(id.clone(), row).is_some()
            {
                return Err(Error::Invalid(
                    "navigation Claim duplicate historical endpoint ID",
                ));
            }
            historical_owner_paths.insert(id, path.as_str().to_owned());
        }
        let owner_ids = historical_claims
            .iter()
            .filter(|(_, row)| row.source_line > 0)
            .filter(|(id, _)| {
                historical_owner_paths
                    .get(*id)
                    .is_some_and(|p| p == path.as_str())
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if owner_ids.is_empty() {
            return Err(Error::Invalid(
                "navigation Claim historical endpoint stream empty",
            ));
        }
        inventory
            .scan_costs
            .push(NavigationClaimEventStreamInventoryCostV1 {
                source_path: path.as_str().to_owned(),
                stream_sha256: Digest256::of_bytes(bytes),
                rows_scanned,
                owner_ids,
            });
    }
    inventory
        .scan_costs
        .sort_by(|left, right| left.source_path.cmp(&right.source_path));
    if inventory.scan_costs.len() > MAX_CLAIM_EVENT_STREAMS {
        return Err(Error::Budget("navigation Claim inventory stream count"));
    }
    inventory.root = owner_inventory_root(&inventory.scan_costs)?;
    inventory.historical_claims = historical_claims;
    inventory.historical_owner_paths = historical_owner_paths;
    Ok(inventory)
}

/// Prepare an owner inventory directly from a bounded list of exact Claim
/// stream members. `source_files` callers can use the wrapper above when they
/// already hold the full dependency closure.
pub fn prepare_navigation_claim_event_owner_streams_v1(
    streams: &[NavigationClaimEventOwnerStreamInputV1<'_>],
    max_row_bytes: usize,
) -> Result<NavigationClaimEventOwnerInventoryV1> {
    owner_inventory_from_streams(streams, max_row_bytes)
}

fn owner_inventory_from_streams(
    streams: &[NavigationClaimEventOwnerStreamInputV1<'_>],
    max_row_bytes: usize,
) -> Result<NavigationClaimEventOwnerInventoryV1> {
    if streams.is_empty() || streams.len() > MAX_CLAIM_EVENT_STREAMS || max_row_bytes == 0 {
        return Err(Error::Budget("navigation Claim inventory stream count"));
    }
    let mut parsed_streams = BTreeMap::new();
    let mut owner_paths = BTreeMap::new();
    let mut scan_costs = Vec::new();
    let mut total_bytes = 0usize;
    let mut total_owners = 0usize;
    for input in streams {
        let path = checked_path(input.source_path)?;
        if !is_claim_stream_path(path.as_str())
            || input.source_bytes.is_empty()
            || input.source_bytes.len() > 1_048_576
        {
            return Err(Error::Budget("navigation Claim inventory stream bounds"));
        }
        total_bytes = total_bytes
            .checked_add(input.source_bytes.len())
            .filter(|bytes| *bytes <= TOTAL_DEPENDENCY_BYTES)
            .ok_or(Error::Budget("navigation Claim inventory bytes"))?;
        if parsed_streams.contains_key(path.as_str()) {
            return Err(Error::Invalid(
                "navigation Claim inventory duplicate stream",
            ));
        }
        let (rows, rows_scanned) = parse_claim_owner_stream(input.source_bytes, max_row_bytes)?;
        total_owners = total_owners
            .checked_add(rows.len())
            .filter(|count| *count <= MAX_CLAIM_EVENT_TOTAL_OWNERS)
            .ok_or(Error::Budget("navigation Claim inventory owner count"))?;
        for claim_id in rows.keys() {
            if owner_paths
                .insert(claim_id.clone(), path.as_str().to_owned())
                .is_some()
            {
                return Err(Error::Invalid("navigation Claim inventory duplicate ID"));
            }
        }
        let stream_sha256 = Digest256::of_bytes(input.source_bytes);
        let owner_ids = rows.keys().cloned().collect::<Vec<_>>();
        scan_costs.push(NavigationClaimEventStreamInventoryCostV1 {
            source_path: path.as_str().to_owned(),
            stream_sha256,
            rows_scanned,
            owner_ids,
        });
        parsed_streams.insert(
            path.as_str().to_owned(),
            NavigationClaimEventOwnerStreamV1 {
                stream_sha256,
                rows,
            },
        );
    }
    let root = owner_inventory_root(&scan_costs)?;
    Ok(NavigationClaimEventOwnerInventoryV1 {
        streams: parsed_streams,
        owner_paths,
        historical_claims: BTreeMap::new(),
        historical_owner_paths: BTreeMap::new(),
        scan_costs,
        root,
    })
}

fn owner_inventory_root(costs: &[NavigationClaimEventStreamInventoryCostV1]) -> Result<Digest256> {
    if costs.is_empty()
        || costs.len() > MAX_CLAIM_EVENT_STREAMS
        || costs
            .windows(2)
            .any(|pair| pair[0].source_path >= pair[1].source_path)
        || costs.iter().any(|cost| {
            cost.rows_scanned != cost.owner_ids.len() as u64
                || cost.owner_ids.is_empty()
                || !sorted_unique(&cost.owner_ids)
        })
    {
        return Err(Error::Invalid(
            "navigation Claim owner inventory cost shape",
        ));
    }
    let mut all_ids = BTreeSet::new();
    if costs
        .iter()
        .flat_map(|cost| cost.owner_ids.iter())
        .any(|id| !all_ids.insert(id.as_str()))
    {
        return Err(Error::Invalid(
            "navigation Claim inventory repeats owner ID",
        ));
    }
    let streams = costs
        .iter()
        .map(|cost| {
            json!({
                "source_path":cost.source_path,
                "stream_sha256":cost.stream_sha256.to_hex(),
                "rows_scanned":cost.rows_scanned,
                "owner_ids":cost.owner_ids
            })
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({"schema":"tos_source_navigation_claim_owner_inventory_v1","streams":streams}),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn affected_owner_manifest_root(
    inventory_root: Digest256,
    mutation_source_path: &str,
    mutation_claim_ids: &[String],
    reverse_claim_ids: &[String],
    streams: &[NavigationClaimEventAffectedOwnerStreamV1],
) -> Result<Digest256> {
    if !sorted_unique(mutation_claim_ids)
        || mutation_claim_ids.is_empty()
        || mutation_claim_ids.len() > 32
        || !sorted_unique(reverse_claim_ids)
        || reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
        || streams.is_empty()
        || streams.len() > MAX_CLAIM_EVENT_STREAMS
        || streams
            .windows(2)
            .any(|pair| pair[0].source_path >= pair[1].source_path)
    {
        return Err(Error::Invalid(
            "navigation Claim affected-owner manifest root shape",
        ));
    }
    let stream_rows = streams
        .iter()
        .map(|stream| {
            json!({
                "source_path":stream.source_path,
                "stream_sha256":stream.stream_sha256.to_hex(),
                "rows_scanned":stream.rows_scanned,
                "owner_ids":stream.owner_ids
            })
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_affected_owner_manifest_v1",
            "inventory_root":inventory_root.to_hex(),
            "mutation_source_path":mutation_source_path,
            "mutation_claim_ids":mutation_claim_ids,
            "reverse_claim_ids":reverse_claim_ids,
            "streams":stream_rows
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn dependency_refresh_owner_manifest_root(
    inventory_root: Digest256,
    changed_dependency_path: &RelativePath,
    changed_dependency_sha256: Digest256,
    closure_root: Digest256,
    reverse_claim_ids: &[String],
    streams: &[NavigationClaimEventAffectedOwnerStreamV1],
) -> Result<Digest256> {
    if is_claim_stream_path(changed_dependency_path.as_str())
        || !sorted_unique(reverse_claim_ids)
        || reverse_claim_ids.is_empty()
        || reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
        || streams.is_empty()
        || streams.len() > MAX_CLAIM_EVENT_STREAMS
        || streams
            .windows(2)
            .any(|pair| pair[0].source_path >= pair[1].source_path)
    {
        return Err(Error::Invalid(
            "navigation Claim dependency-refresh manifest root shape",
        ));
    }
    let stream_rows = streams
        .iter()
        .map(|stream| {
            json!({
                "source_path":stream.source_path,
                "stream_sha256":stream.stream_sha256.to_hex(),
                "rows_scanned":stream.rows_scanned,
                "owner_ids":stream.owner_ids
            })
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_dependency_refresh_owner_manifest_v1",
            "inventory_root":inventory_root.to_hex(),
            "changed_dependency_path":changed_dependency_path.as_str(),
            "changed_dependency_sha256":changed_dependency_sha256.to_hex(),
            "closure_root":closure_root.to_hex(),
            "reverse_claim_ids":reverse_claim_ids,
            "streams":stream_rows
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn dependency_refresh_owner_manifest_set_root(
    inventory_root: Digest256,
    closure_root: Digest256,
    causes: &[NavigationClaimDependencyRefreshCauseV1],
    reverse_claim_ids: &[String],
    streams: &[NavigationClaimEventAffectedOwnerStreamV1],
) -> Result<Digest256> {
    if causes.is_empty()
        || causes.len() > MAX_CLAIM_REFRESH_CAUSES
        || causes
            .windows(2)
            .any(|pair| pair[0].changed_dependency_path >= pair[1].changed_dependency_path)
        || !sorted_unique(reverse_claim_ids)
        || reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
        || streams.len() > MAX_CLAIM_EVENT_STREAMS
        || streams
            .windows(2)
            .any(|pair| pair[0].source_path >= pair[1].source_path)
    {
        return Err(Error::Invalid(
            "navigation Claim dependency-refresh set root shape",
        ));
    }
    let cause_rows = causes
        .iter()
        .map(|cause| {
            json!({
                "changed_dependency_path":cause.changed_dependency_path.as_str(),
                "changed_dependency_sha256":cause.changed_dependency_sha256.to_hex(),
                "reverse_claim_ids":cause.reverse_claim_ids
            })
        })
        .collect::<Vec<_>>();
    let stream_rows = streams
        .iter()
        .map(|stream| {
            json!({
                "source_path":stream.source_path,
                "stream_sha256":stream.stream_sha256.to_hex(),
                "rows_scanned":stream.rows_scanned,
                "owner_ids":stream.owner_ids
            })
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_dependency_refresh_owner_manifest_set_v1",
            "inventory_root":inventory_root.to_hex(),
            "closure_root":closure_root.to_hex(),
            "causes":cause_rows,
            "reverse_claim_ids":reverse_claim_ids,
            "streams":stream_rows
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn dependency_refresh_manifest_set_from_single(
    manifest: &NavigationClaimDependencyRefreshOwnerManifestV1,
    closure_root: Digest256,
) -> Result<NavigationClaimDependencyRefreshOwnerManifestSetV1> {
    manifest.verify_integrity()?;
    let set = NavigationClaimDependencyRefreshOwnerManifestSetV1 {
        inventory_root: manifest.inventory_root,
        closure_root,
        causes: vec![NavigationClaimDependencyRefreshCauseV1 {
            changed_dependency_path: manifest.changed_dependency_path.clone(),
            changed_dependency_sha256: manifest.changed_dependency_sha256,
            reverse_claim_ids: manifest.reverse_claim_ids.clone(),
        }],
        reverse_claim_ids: manifest.reverse_claim_ids.clone(),
        streams: manifest.streams.clone(),
        root: Digest256::of_bytes(&[]),
    };
    let root = dependency_refresh_owner_manifest_set_root(
        set.inventory_root,
        set.closure_root,
        &set.causes,
        &set.reverse_claim_ids,
        &set.streams,
    )?;
    let set = NavigationClaimDependencyRefreshOwnerManifestSetV1 { root, ..set };
    set.verify_integrity()?;
    Ok(set)
}

/// Canonical graph rows owned by one stable Claim ID.
#[derive(Clone, Debug)]
pub struct NavigationClaimEventProjectionV1 {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub diagnostics: Vec<Value>,
}

#[derive(Clone, Copy, Debug)]
struct NavigationClaimEventCohortSeal;

/// Sealed event-local Claim replacement. Only the checked renderer can create
/// a value accepted by the PG batch publisher.
#[derive(Clone, Debug)]
pub struct NavigationClaimEventCohortV1 {
    event_id: String,
    event_sequence: u64,
    source_event_id: String,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    mutation_source_path: RelativePath,
    source_path: RelativePath,
    record_id: String,
    source_line: u64,
    predecessor_cohort_root: Option<Digest256>,
    dependency_root: Digest256,
    input_root: Digest256,
    output_root: Digest256,
    event_mutation_claim_ids: Vec<String>,
    event_claim_owner_ids: Vec<String>,
    claim_stream_sha256: Digest256,
    claim_stream_rows_scanned: u64,
    mutation_receipt_path: RelativePath,
    mutation_receipt_role: NavigationDependencyRoleV1,
    event_claim_set_binding: Digest256,
    event_claim_set_root: Digest256,
    affected_owner_manifest_root: Digest256,
    owner_inventory_root: Digest256,
    owner_inventory_scan_costs: Vec<NavigationClaimEventStreamInventoryCostV1>,
    revision_proof: Option<NavigationClaimRevisionProofV1>,
    trace_sha256: Digest256,
    dependencies: Vec<NavigationDependencyV1>,
    projection: NavigationClaimEventProjectionV1,
    external_endpoint_ids: Vec<String>,
    node_visibility: Vec<bool>,
    edge_visibility: Vec<bool>,
    _sealed: NavigationClaimEventCohortSeal,
}

/// Row commitment returned after recomputing the entire sealed cohort.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimEventRowV1 {
    pub collection: String,
    pub id: String,
    pub sha256: Digest256,
    pub visible: bool,
}

/// Recomputed integrity view for publisher-side event-context binding.
pub struct NavigationClaimEventIntegrityV1<'a> {
    pub dependency_root: Digest256,
    pub input_root: Digest256,
    pub output_root: Digest256,
    pub mutation_source_path: &'a str,
    pub mutation_receipt_role: NavigationDependencyRoleV1,
    /// Compatibility alias for mutation IDs. New publishers should use the
    /// two explicit sets below.
    pub event_claim_ids: &'a [String],
    pub event_mutation_claim_ids: &'a [String],
    pub event_claim_owner_ids: &'a [String],
    pub claim_stream_sha256: Digest256,
    pub claim_stream_rows_scanned: u64,
    pub event_claim_set_binding: Digest256,
    pub event_claim_set_root: Digest256,
    pub affected_owner_manifest_root: Digest256,
    pub owner_inventory_root: Digest256,
    pub owner_inventory_scan_costs: &'a [NavigationClaimEventStreamInventoryCostV1],
    pub revision_proof_root: Option<Digest256>,
    pub external_endpoint_ids: &'a [String],
    pub rows: Vec<NavigationClaimEventRowV1>,
    pub node_visibility: &'a [bool],
    pub edge_visibility: &'a [bool],
}

/// Exact, checked proof that a Claim revision history appended one mutation
/// and retained its predecessor package and original creation evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimRevisionProofV1 {
    event_id: String,
    event_sequence: u64,
    claim_id: String,
    claim_stream_path: RelativePath,
    claim_stream_sha256: Digest256,
    history_path: RelativePath,
    history_sha256: Digest256,
    revision_receipt_sha256: Digest256,
    previous_source_sha256: Digest256,
    successor_source_sha256: Digest256,
    previous_source_ref: Value,
    successor_source_ref: Value,
    archive_path: RelativePath,
    archive_manifest_sha256: Digest256,
    archive_package_root: Digest256,
    provenance_event_path: RelativePath,
    provenance_event_id: String,
    provenance_event_version: u64,
    provenance_event_sha256: Digest256,
    provenance_receipt_path: RelativePath,
    provenance_receipt_sha256: Digest256,
    mutation_claim_ids: Vec<String>,
    binding_root: Digest256,
}

/// Authenticated refresh cause and the exact current Claim evidence retained
/// when a Record/schema event revalidates a reverse-dependent Claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationClaimDependencyRefreshProofV1 {
    event_id: String,
    event_sequence: u64,
    changed_dependencies: Vec<NavigationClaimDependencyRefreshCauseV1>,
    closure_root: Digest256,
    affected_owner_manifest_root: Digest256,
    reverse_claim_ids: Vec<String>,
    claim_stream_path: RelativePath,
    claim_stream_sha256: Digest256,
    claim_id: String,
    claim_version: u64,
    claim_source_line: u64,
    claim_row_sha256: Digest256,
    current_source_ref_sha256: Digest256,
    provenance_event_path: RelativePath,
    provenance_event_id: String,
    provenance_event_version: u64,
    provenance_event_sha256: Digest256,
    provenance_receipt_path: RelativePath,
    provenance_receipt_sha256: Digest256,
    history_path: Option<RelativePath>,
    history_sha256: Option<Digest256>,
    latest_revision_receipt_sha256: Option<Digest256>,
    binding_root: Digest256,
}

impl NavigationClaimDependencyRefreshProofV1 {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
    pub fn changed_dependencies(&self) -> &[NavigationClaimDependencyRefreshCauseV1] {
        &self.changed_dependencies
    }
    pub fn closure_root(&self) -> Digest256 {
        self.closure_root
    }
    pub fn affected_owner_manifest_root(&self) -> Digest256 {
        self.affected_owner_manifest_root
    }
    pub fn reverse_claim_ids(&self) -> &[String] {
        &self.reverse_claim_ids
    }
    pub fn claim_stream_path(&self) -> &RelativePath {
        &self.claim_stream_path
    }
    pub fn claim_stream_sha256(&self) -> Digest256 {
        self.claim_stream_sha256
    }
    pub fn claim_id(&self) -> &str {
        &self.claim_id
    }
    pub fn claim_version(&self) -> u64 {
        self.claim_version
    }
    pub fn claim_source_line(&self) -> u64 {
        self.claim_source_line
    }
    pub fn claim_row_sha256(&self) -> Digest256 {
        self.claim_row_sha256
    }
    pub fn current_source_ref_sha256(&self) -> Digest256 {
        self.current_source_ref_sha256
    }
    pub fn provenance_event_path(&self) -> &RelativePath {
        &self.provenance_event_path
    }
    pub fn provenance_event_id(&self) -> &str {
        &self.provenance_event_id
    }
    pub fn provenance_event_version(&self) -> u64 {
        self.provenance_event_version
    }
    pub fn provenance_event_sha256(&self) -> Digest256 {
        self.provenance_event_sha256
    }
    pub fn provenance_receipt_path(&self) -> &RelativePath {
        &self.provenance_receipt_path
    }
    pub fn provenance_receipt_sha256(&self) -> Digest256 {
        self.provenance_receipt_sha256
    }
    pub fn history_path(&self) -> Option<&RelativePath> {
        self.history_path.as_ref()
    }
    pub fn history_sha256(&self) -> Option<Digest256> {
        self.history_sha256
    }
    pub fn latest_revision_receipt_sha256(&self) -> Option<Digest256> {
        self.latest_revision_receipt_sha256
    }
    pub fn binding_root(&self) -> Digest256 {
        self.binding_root
    }
}

#[derive(Clone, Debug)]
pub struct NavigationClaimDependencyRefreshCohortV1 {
    event_id: String,
    event_sequence: u64,
    source_event_id: String,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    predecessor_cohort_root: Option<Digest256>,
    source_path: RelativePath,
    record_id: String,
    source_line: u64,
    dependency_root: Digest256,
    input_root: Digest256,
    output_root: Digest256,
    event_basis_root: Digest256,
    owner_inventory_root: Digest256,
    owner_inventory_scan_costs: Vec<NavigationClaimEventStreamInventoryCostV1>,
    event_claim_owner_ids: Vec<String>,
    claim_stream_sha256: Digest256,
    claim_stream_rows_scanned: u64,
    proof: NavigationClaimDependencyRefreshProofV1,
    trace_sha256: Digest256,
    dependencies: Vec<NavigationDependencyV1>,
    projection: NavigationClaimEventProjectionV1,
    external_endpoint_ids: Vec<String>,
    node_visibility: Vec<bool>,
    edge_visibility: Vec<bool>,
    _sealed: NavigationClaimEventCohortSeal,
}

pub struct NavigationClaimDependencyRefreshIntegrityV1<'a> {
    pub event_id: &'a str,
    pub event_sequence: u64,
    pub source_event_id: &'a str,
    pub source_event_version: u64,
    pub predecessor_source_revision: Digest256,
    pub source_revision: Digest256,
    pub predecessor_cohort_root: Option<Digest256>,
    pub source_path: &'a str,
    pub record_id: &'a str,
    pub source_line: u64,
    pub dependency_root: Digest256,
    pub input_root: Digest256,
    pub output_root: Digest256,
    pub event_basis_root: Digest256,
    pub owner_inventory_root: Digest256,
    pub owner_inventory_scan_costs: &'a [NavigationClaimEventStreamInventoryCostV1],
    pub proof: &'a NavigationClaimDependencyRefreshProofV1,
    pub external_endpoint_ids: &'a [String],
    pub rows: Vec<NavigationClaimEventRowV1>,
    pub node_visibility: &'a [bool],
    pub edge_visibility: &'a [bool],
}

#[derive(Clone, Debug)]
pub enum NavigationClaimEventContributionV1 {
    Mutation(NavigationClaimEventCohortV1),
    DependencyRefresh(NavigationClaimDependencyRefreshCohortV1),
}

pub enum NavigationClaimEventContributionIntegrityV1<'a> {
    Mutation(NavigationClaimEventIntegrityV1<'a>),
    DependencyRefresh(NavigationClaimDependencyRefreshIntegrityV1<'a>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationClaimEventBasisV1 {
    Mutation,
    DependencyRefresh,
}

impl NavigationClaimRevisionProofV1 {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
    pub fn claim_id(&self) -> &str {
        &self.claim_id
    }
    pub fn claim_stream_path(&self) -> &RelativePath {
        &self.claim_stream_path
    }
    pub fn claim_stream_sha256(&self) -> Digest256 {
        self.claim_stream_sha256
    }
    pub fn history_path(&self) -> &RelativePath {
        &self.history_path
    }
    pub fn history_sha256(&self) -> Digest256 {
        self.history_sha256
    }
    pub fn mutation_receipt_path(&self) -> &RelativePath {
        &self.history_path
    }
    pub fn mutation_receipt_sha256(&self) -> Digest256 {
        self.history_sha256
    }
    pub fn revision_receipt_sha256(&self) -> Digest256 {
        self.revision_receipt_sha256
    }
    pub fn previous_source_sha256(&self) -> Digest256 {
        self.previous_source_sha256
    }
    pub fn successor_source_sha256(&self) -> Digest256 {
        self.successor_source_sha256
    }
    pub fn previous_source_ref(&self) -> &Value {
        &self.previous_source_ref
    }
    pub fn successor_source_ref(&self) -> &Value {
        &self.successor_source_ref
    }
    pub fn archive_path(&self) -> &RelativePath {
        &self.archive_path
    }
    pub fn archive_manifest_sha256(&self) -> Digest256 {
        self.archive_manifest_sha256
    }
    pub fn archive_package_root(&self) -> Digest256 {
        self.archive_package_root
    }
    pub fn provenance_event_path(&self) -> &RelativePath {
        &self.provenance_event_path
    }
    pub fn provenance_event_id(&self) -> &str {
        &self.provenance_event_id
    }
    pub fn provenance_event_version(&self) -> u64 {
        self.provenance_event_version
    }
    pub fn provenance_event_sha256(&self) -> Digest256 {
        self.provenance_event_sha256
    }
    pub fn provenance_receipt_path(&self) -> &RelativePath {
        &self.provenance_receipt_path
    }
    pub fn provenance_receipt_sha256(&self) -> Digest256 {
        self.provenance_receipt_sha256
    }
    pub fn mutation_claim_ids(&self) -> &[String] {
        &self.mutation_claim_ids
    }
    pub fn binding_root(&self) -> Digest256 {
        self.binding_root
    }
}

struct NavigationClaimCreationBindingV1 {
    event_path: RelativePath,
    event_id: String,
    event_version: u64,
    event_sha256: Digest256,
    receipt_path: RelativePath,
    receipt_sha256: Digest256,
}

struct NavigationClaimArchiveBindingV1 {
    manifest_sha256: Digest256,
    package_root: Digest256,
}

/// Deterministic incremental admission cost for one sealed Claim cohort.
/// `encoded_bytes` matches the PG publisher's persisted-row accounting:
/// canonical output rows plus dependency path bytes. Dependency bytes and
/// actual Claim-stream row scans are reported separately; physical I/O is not
/// observed by this compiler-side accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NavigationClaimEventBudgetCostV1 {
    pub rows: u64,
    pub encoded_bytes: u64,
    pub dependency_bytes: u64,
    pub claim_stream_rows_scanned: u64,
}

impl NavigationClaimEventCohortV1 {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
    pub fn source_event_id(&self) -> &str {
        &self.source_event_id
    }
    pub fn source_event_version(&self) -> u64 {
        self.source_event_version
    }
    pub fn predecessor_source_revision(&self) -> Digest256 {
        self.predecessor_source_revision
    }
    pub fn source_revision(&self) -> Digest256 {
        self.source_revision
    }
    pub fn mutation_source_path(&self) -> &RelativePath {
        &self.mutation_source_path
    }
    pub fn source_path(&self) -> &RelativePath {
        &self.source_path
    }
    pub fn record_id(&self) -> &str {
        &self.record_id
    }
    pub fn source_line(&self) -> u64 {
        self.source_line
    }
    pub fn predecessor_cohort_root(&self) -> Option<Digest256> {
        self.predecessor_cohort_root
    }
    pub fn dependency_root(&self) -> Digest256 {
        self.dependency_root
    }
    pub fn input_root(&self) -> Digest256 {
        self.input_root
    }
    pub fn output_root(&self) -> Digest256 {
        self.output_root
    }
    pub fn event_claim_ids(&self) -> &[String] {
        &self.event_mutation_claim_ids
    }
    pub fn event_mutation_claim_ids(&self) -> &[String] {
        &self.event_mutation_claim_ids
    }
    pub fn event_claim_owner_ids(&self) -> &[String] {
        &self.event_claim_owner_ids
    }
    pub fn claim_stream_sha256(&self) -> Digest256 {
        self.claim_stream_sha256
    }
    pub fn mutation_receipt_path(&self) -> &RelativePath {
        &self.mutation_receipt_path
    }
    pub fn mutation_receipt_role(&self) -> NavigationDependencyRoleV1 {
        self.mutation_receipt_role
    }
    pub fn event_claim_set_binding(&self) -> Digest256 {
        self.event_claim_set_binding
    }
    pub fn event_claim_set_root(&self) -> Digest256 {
        self.event_claim_set_root
    }
    pub fn affected_owner_manifest_root(&self) -> Digest256 {
        self.affected_owner_manifest_root
    }
    pub fn owner_inventory_root(&self) -> Digest256 {
        self.owner_inventory_root
    }
    pub fn owner_inventory_scan_costs(&self) -> &[NavigationClaimEventStreamInventoryCostV1] {
        &self.owner_inventory_scan_costs
    }
    pub fn revision_proof(&self) -> Option<&NavigationClaimRevisionProofV1> {
        self.revision_proof.as_ref()
    }
    pub fn trace_sha256(&self) -> Digest256 {
        self.trace_sha256
    }
    pub fn dependencies(&self) -> &[NavigationDependencyV1] {
        &self.dependencies
    }
    pub fn projection(&self) -> &NavigationClaimEventProjectionV1 {
        &self.projection
    }
    pub fn external_endpoint_ids(&self) -> &[String] {
        &self.external_endpoint_ids
    }
    pub fn node_visibility(&self) -> &[bool] {
        &self.node_visibility
    }
    pub fn edge_visibility(&self) -> &[bool] {
        &self.edge_visibility
    }

    /// Recheck roots, row identities, Claim-set completeness and visibility
    /// alignment before a batch publisher binds the cohort to a CMD event.
    pub fn verify_integrity(&self) -> Result<NavigationClaimEventIntegrityV1<'_>> {
        if !sorted_unique(&self.event_mutation_claim_ids)
            || !sorted_unique(&self.event_claim_owner_ids)
            || self.event_claim_owner_ids.is_empty()
            || self.event_claim_owner_ids.len() > MAX_CLAIM_EVENT_OWNERS
            || self
                .event_claim_owner_ids
                .binary_search(&self.record_id)
                .is_err()
            || self.node_visibility.len() != self.projection.nodes.len()
            || self.edge_visibility.len() != self.projection.edges.len()
        {
            return Err(Error::Invalid("navigation Claim cohort integrity shape"));
        }
        let owner_inventory_root = owner_inventory_root(&self.owner_inventory_scan_costs)?;
        if owner_inventory_root != self.owner_inventory_root
            || !self.owner_inventory_scan_costs.iter().any(|cost| {
                cost.source_path == self.source_path.as_str()
                    && cost.stream_sha256 == self.claim_stream_sha256
                    && cost.owner_ids == self.event_claim_owner_ids
            })
            || !self.owner_inventory_scan_costs.iter().any(|cost| {
                cost.source_path == self.mutation_source_path.as_str()
                    && self
                        .event_mutation_claim_ids
                        .iter()
                        .all(|id| cost.owner_ids.binary_search(id).is_ok())
            })
            || self.source_path == self.mutation_source_path
                && self
                    .event_mutation_claim_ids
                    .iter()
                    .any(|id| self.event_claim_owner_ids.binary_search(id).is_err())
        {
            return Err(Error::Invalid("navigation Claim owner inventory binding"));
        }
        let revision_proof_root = self
            .revision_proof
            .as_ref()
            .map(revision_proof_binding_root)
            .transpose()?;
        if let Some(proof) = &self.revision_proof {
            let archive_manifest_path =
                checked_path(&format!("{}/manifest.json", proof.archive_path.as_str()))?;
            if proof.event_id != self.event_id
                || proof.event_sequence != self.event_sequence
                || proof.claim_id != self.record_id
                || proof.claim_stream_path != self.mutation_source_path
                || proof.claim_stream_path != self.source_path
                || proof.claim_stream_sha256 != self.claim_stream_sha256
                || proof.history_path != self.mutation_receipt_path
                || proof.history_sha256 != self.event_claim_set_binding
                || proof.mutation_claim_ids != self.event_mutation_claim_ids
                || proof.binding_root != revision_proof_root.expect("proof root present")
                || proof.mutation_claim_ids.len() != 1
                || proof.mutation_claim_ids.first().map(String::as_str)
                    != Some(proof.claim_id.as_str())
                || proof.previous_source_ref.get("id").and_then(Value::as_str)
                    != Some(proof.claim_id.as_str())
                || proof.successor_source_ref.get("id").and_then(Value::as_str)
                    != Some(proof.claim_id.as_str())
                || claim_reference_digest(&proof.previous_source_ref)?
                    != proof.previous_source_sha256
                || claim_reference_digest(&proof.successor_source_ref)?
                    != proof.successor_source_sha256
                || dependency_present_digest(
                    &self.dependencies,
                    &proof.history_path,
                    self.mutation_receipt_role,
                )? != proof.history_sha256
                || dependency_present_digest(
                    &self.dependencies,
                    &archive_manifest_path,
                    NavigationDependencyRoleV1::VersionSource,
                )? != proof.archive_manifest_sha256
                || dependency_present_digest(
                    &self.dependencies,
                    &proof.provenance_receipt_path,
                    NavigationDependencyRoleV1::EventReceipt,
                )? != proof.provenance_receipt_sha256
            {
                return Err(Error::Invalid("navigation Claim revision proof binding"));
            }
        }
        let dependency_root = dependency_root(&self.dependencies)?;
        if dependency_present_digest(
            &self.dependencies,
            &self.source_path,
            NavigationDependencyRoleV1::Record,
        )? != self.claim_stream_sha256
            || dependency_present_digest(
                &self.dependencies,
                &self.mutation_receipt_path,
                self.mutation_receipt_role,
            )? != self.event_claim_set_binding
        {
            return Err(Error::Invalid(
                "navigation Claim stream or receipt dependency binding",
            ));
        }
        let (rows, external) = output_rows(
            &self.projection,
            &self.node_visibility,
            &self.edge_visibility,
        )?;
        if external != self.external_endpoint_ids {
            return Err(Error::Invalid("navigation Claim external endpoint set"));
        }
        let output_root = output_root(
            &rows,
            &self.projection.diagnostics,
            self.trace_sha256,
            &self.external_endpoint_ids,
        )?;
        let event_claim_set_root = claim_set_root(
            &self.event_id,
            self.event_sequence,
            &self.mutation_receipt_path,
            self.event_claim_set_binding,
            self.affected_owner_manifest_root,
        )?;
        let input_root = input_root(
            &self.event_id,
            self.event_sequence,
            &self.source_event_id,
            self.source_event_version,
            self.predecessor_source_revision,
            self.source_revision,
            self.predecessor_cohort_root,
            &self.source_path,
            self.source_line,
            &self.record_id,
            &self.mutation_receipt_path,
            &self.mutation_source_path,
            dependency_root,
            output_root,
            event_claim_set_root,
            owner_inventory_root,
            revision_proof_root,
            self.claim_stream_rows_scanned,
        )?;
        if dependency_root != self.dependency_root
            || output_root != self.output_root
            || input_root != self.input_root
            || event_claim_set_root != self.event_claim_set_root
        {
            return Err(Error::Invalid("navigation Claim cohort root mismatch"));
        }
        Ok(NavigationClaimEventIntegrityV1 {
            dependency_root,
            input_root,
            output_root,
            mutation_source_path: self.mutation_source_path.as_str(),
            mutation_receipt_role: self.mutation_receipt_role,
            event_claim_ids: &self.event_mutation_claim_ids,
            event_mutation_claim_ids: &self.event_mutation_claim_ids,
            event_claim_owner_ids: &self.event_claim_owner_ids,
            claim_stream_sha256: self.claim_stream_sha256,
            claim_stream_rows_scanned: self.claim_stream_rows_scanned,
            event_claim_set_binding: self.event_claim_set_binding,
            event_claim_set_root,
            affected_owner_manifest_root: self.affected_owner_manifest_root,
            owner_inventory_root,
            owner_inventory_scan_costs: &self.owner_inventory_scan_costs,
            revision_proof_root,
            external_endpoint_ids: &self.external_endpoint_ids,
            rows,
            node_visibility: &self.node_visibility,
            edge_visibility: &self.edge_visibility,
        })
    }

    /// Return a stable per-cohort cost so a producer can enforce aggregate
    /// row, encoded-byte, and work ceilings before retaining this cohort.
    /// Publishers can recompute the same values before accepting the set.
    pub fn budget_cost(&self) -> Result<NavigationClaimEventBudgetCostV1> {
        self.verify_integrity()?;
        navigation_claim_event_budget_cost(
            &self.projection,
            &self.dependencies,
            &self.source_path,
            self.claim_stream_sha256,
            self.claim_stream_rows_scanned,
        )
    }
}

/// Resolve one exact current Claim into its complete graph replacement
/// contribution. The caller remains responsible for the owner-side source
/// selection and for binding the mutation receipt to the committed event.
pub fn project_navigation_claim_event_v1(
    input: NavigationClaimEventInputV1<'_>,
    limits: BibliographicLimits,
) -> Result<NavigationClaimEventCohortV1> {
    let inventory = prepare_navigation_claim_event_owner_inventory_v1(input.source_files, limits)?;
    project_navigation_claim_event_with_inventory_v1(input, &inventory, limits)
}

/// Resolve one exact current Claim using an inventory shared across all
/// owners in the selected event cut. Full-stream parsing is charged once in
/// the inventory; this call accounts only owner-specific historical scans.
pub fn project_navigation_claim_event_with_inventory_v1(
    input: NavigationClaimEventInputV1<'_>,
    inventory: &NavigationClaimEventOwnerInventoryV1,
    limits: BibliographicLimits,
) -> Result<NavigationClaimEventCohortV1> {
    let cause = NavigationClaimEventCauseInputV1::Mutation {
        mutation_source_path: input.mutation_source_path,
        mutation_receipt_path: input.mutation_receipt_path,
        mutation_receipt_bytes: input.mutation_receipt_bytes,
        affected_owner_manifest: input.affected_owner_manifest,
    };
    match project_navigation_claim_event_core_v1(input.into_common(), cause, inventory, limits)? {
        NavigationClaimEventContributionV1::Mutation(cohort) => Ok(cohort),
        NavigationClaimEventContributionV1::DependencyRefresh(_) => Err(Error::Invalid(
            "navigation Claim mutation renderer returned refresh cohort",
        )),
    }
}

/// Resolve one Claim owner for a non-Claim event whose exact source dependency
/// changed. The event cause is a separately sealed reverse-closure basis and
/// carries no Claim mutation receipt.
pub fn project_navigation_claim_dependency_refresh_with_inventory_v1(
    input: NavigationClaimDependencyRefreshInputV1<'_>,
    inventory: &NavigationClaimEventOwnerInventoryV1,
    limits: BibliographicLimits,
) -> Result<NavigationClaimDependencyRefreshCohortV1> {
    let manifest_set = dependency_refresh_manifest_set_from_single(
        input.affected_owner_manifest,
        input.closure_root,
    )?;
    project_navigation_claim_dependency_refresh_set_with_inventory_v1(
        NavigationClaimDependencyRefreshSetInputV1 {
            common: input.common,
            affected_owner_manifest_set: &manifest_set,
        },
        inventory,
        limits,
    )
}

/// Resolve one Claim owner for a compound non-Claim event. All changed
/// dependency causes and reverse closures are supplied by one sealed set.
pub fn project_navigation_claim_dependency_refresh_set_with_inventory_v1(
    input: NavigationClaimDependencyRefreshSetInputV1<'_>,
    inventory: &NavigationClaimEventOwnerInventoryV1,
    limits: BibliographicLimits,
) -> Result<NavigationClaimDependencyRefreshCohortV1> {
    let cause = NavigationClaimEventCauseInputV1::DependencyRefresh {
        affected_owner_manifest_set: input.affected_owner_manifest_set,
    };
    match project_navigation_claim_event_core_v1(input.common, cause, inventory, limits)? {
        NavigationClaimEventContributionV1::DependencyRefresh(cohort) => Ok(cohort),
        NavigationClaimEventContributionV1::Mutation(_) => Err(Error::Invalid(
            "navigation Claim refresh renderer returned mutation cohort",
        )),
    }
}

fn project_navigation_claim_event_core_v1(
    mut input: NavigationClaimEventCommonInputV1<'_>,
    cause: NavigationClaimEventCauseInputV1<'_>,
    inventory: &NavigationClaimEventOwnerInventoryV1,
    limits: BibliographicLimits,
) -> Result<NavigationClaimEventContributionV1> {
    limits.validate()?;
    let selected_source_revision = input.validator.selected_event_source_revision()?;
    let source_revision = selected_source_revision;
    if !valid_event_id(input.event_id)
        || input.event_sequence == 0
        || input.source_head_sequence != input.event_sequence
        || input.predecessor_source_revision == source_revision
    {
        return Err(Error::Invalid("navigation Claim event identity"));
    }
    let source_path = checked_path(input.source_path)?;
    let (
        mutation_source_path,
        mutation_receipt_path,
        mutation_receipt_bytes,
        event_mutation_claim_ids,
        event_claim_set_binding,
        affected_owner_manifest_root,
        refresh_manifest,
    ) = match &cause {
        NavigationClaimEventCauseInputV1::Mutation {
            mutation_source_path,
            mutation_receipt_path,
            mutation_receipt_bytes,
            affected_owner_manifest,
        } => {
            let mutation_source_path = checked_path(mutation_source_path)?;
            let mutation_receipt_path = checked_path(mutation_receipt_path)?;
            let (ids, binding) =
                mutation_claim_ids(mutation_receipt_bytes, limits.catalog.max_row_bytes)?;
            if ids.is_empty() || ids.len() > 32 {
                return Err(Error::Invalid("navigation Claim mutation receipt members"));
            }
            affected_owner_manifest.verify_integrity()?;
            let source_stream = affected_owner_manifest
                .streams()
                .iter()
                .find(|stream| stream.source_path() == input.source_path)
                .ok_or(Error::Invalid(
                    "navigation Claim mutation affected stream absent",
                ))?;
            if affected_owner_manifest.inventory_root() != inventory.root()
                || affected_owner_manifest.mutation_source_path() != mutation_source_path.as_str()
                || affected_owner_manifest.mutation_claim_ids() != ids
                || source_stream.owner_ids() != input.owner_claim_ids
                || source_stream.stream_sha256() != Digest256::of_bytes(input.source_bytes)
            {
                return Err(Error::Invalid(
                    "navigation Claim mutation affected-owner manifest binding",
                ));
            }
            (
                Some(mutation_source_path),
                Some(mutation_receipt_path),
                Some(*mutation_receipt_bytes),
                ids,
                Some(binding),
                Some(affected_owner_manifest.root()),
                None,
            )
        }
        NavigationClaimEventCauseInputV1::DependencyRefresh {
            affected_owner_manifest_set,
        } => {
            affected_owner_manifest_set.verify_integrity()?;
            if affected_owner_manifest_set.inventory_root() != inventory.root() {
                return Err(Error::Invalid(
                    "navigation Claim dependency-refresh closure binding",
                ));
            }
            (
                None,
                None,
                None,
                Vec::new(),
                None,
                None,
                Some(affected_owner_manifest_set),
            )
        }
    };
    if !source_path
        .as_str()
        .starts_with("ToS/source-witnesses/relations/")
        || !source_path.as_str().ends_with("/source-claims.jsonl")
        || source_path.as_str().contains("/owner-local/")
        || source_path.as_str().contains("/private/")
    {
        return Err(Error::Invalid("navigation Claim owner path"));
    }
    if let Some(mutation_source_path) = mutation_source_path.as_ref() {
        if !is_claim_stream_path(mutation_source_path.as_str()) {
            return Err(Error::Invalid("navigation Claim mutation owner path"));
        }
        let mutation_stream =
            inventory
                .stream(mutation_source_path.as_str())
                .ok_or(Error::Invalid(
                    "navigation Claim mutation stream missing from inventory",
                ))?;
        if event_mutation_claim_ids
            .iter()
            .any(|candidate| !mutation_stream.rows.contains_key(candidate))
        {
            return Err(Error::Invalid(
                "navigation Claim mutation members absent from exact stream",
            ));
        }
    }
    if let Some(refresh_manifest) = refresh_manifest {
        let stream = refresh_manifest
            .streams()
            .iter()
            .find(|stream| stream.source_path() == source_path.as_str())
            .ok_or(Error::Invalid(
                "navigation Claim refresh owner stream absent",
            ))?;
        if stream.owner_ids() != input.owner_claim_ids
            || stream.stream_sha256() != Digest256::of_bytes(input.source_bytes)
        {
            return Err(Error::Invalid(
                "navigation Claim refresh complete owner stream differs",
            ));
        }
    }
    let mut claim_stream_rows_scanned = 0u64;
    let claim_schema_path = checked_path(input.claim_schema_path)?;
    if claim_schema_path.as_str() != "ToS/contracts/source-claim-record.schema.json" {
        return Err(Error::Invalid("navigation Claim schema owner path"));
    }
    let owner_stream = inventory
        .stream(source_path.as_str())
        .ok_or(Error::Invalid(
            "navigation Claim owner stream missing from inventory",
        ))?;
    if owner_stream.stream_sha256 != Digest256::of_bytes(input.source_bytes) {
        return Err(Error::Invalid(
            "navigation Claim inventory stream digest differs",
        ));
    }
    let claim_owner_ids = owner_stream.rows.keys().cloned().collect::<Vec<_>>();
    if claim_owner_ids != input.owner_claim_ids {
        return Err(Error::Invalid("navigation Claim owner inventory differs"));
    }
    let current_owner = inventory
        .row_at_line(source_path.as_str(), input.source_line)
        .filter(|row| row.source_line == input.source_line)
        .ok_or(Error::Invalid("navigation Claim stream line absent"))?;
    let claim_bytes = current_owner.exact_row_bytes.clone();
    let claim = current_owner.claim.clone();
    let id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= PATH_BYTES)
        .ok_or(Error::Invalid("navigation Claim identity"))?
        .to_owned();
    let claim_stream_sha256 = owner_stream.stream_sha256;
    let event_claim_owner_ids = input.owner_claim_ids.to_vec();
    if mutation_source_path.as_ref() == Some(&source_path)
        && event_mutation_claim_ids
            .iter()
            .any(|candidate| !owner_stream.rows.contains_key(candidate))
        || owner_stream.rows.get(&id).map(|row| &row.claim) != Some(&claim)
        || owner_stream.rows.get(&id).map(|row| row.source_line) != Some(input.source_line)
    {
        return Err(Error::Invalid(
            "navigation Claim current owner or mutation member set",
        ));
    }
    let entry = render_catalog_claim(
        &claim,
        source_path.as_str(),
        input.source_line,
        Some(claim_schema_path.as_str()),
        limits.catalog.max_output_row_bytes,
    )?;
    if !same_canonical(
        &entry,
        input.claim_inputs.entry,
        limits.catalog.max_output_row_bytes,
    )? || !same_canonical(
        &claim,
        input.claim_inputs.claim,
        limits.catalog.max_row_bytes,
    )? {
        return Err(Error::Invalid("navigation Claim exact stream binding"));
    }
    input.validator.check_selected_event_document_exact(
        selected_source_revision,
        source_path.as_str(),
        claim_schema_path.as_str(),
        input.claim_schema_bytes,
        &claim_bytes,
    )?;
    let expected_forms_path = claim_forms_path(source_path.as_str(), &id)?;
    let forms_path = checked_path(input.forms_path)?;
    if forms_path != expected_forms_path {
        return Err(Error::Invalid("navigation Claim forms path binding"));
    }
    let forms = match (input.forms_bytes, input.forms_schema_bytes) {
        (Some(bytes), Some(schema_bytes)) => {
            let schema_path = checked_path(input.forms_schema_path)?;
            if schema_path.as_str() != "ToS/contracts/human-form-set.schema.json" {
                return Err(Error::Invalid("navigation Claim forms schema path"));
            }
            input.validator.check_selected_event_document_exact(
                selected_source_revision,
                forms_path.as_str(),
                schema_path.as_str(),
                schema_bytes,
                bytes,
            )?;
            materialize_checked_form_set(
                bytes,
                forms_path.as_str(),
                &claim,
                input.forms_materializer,
                limits,
            )?
        }
        (None, None) => None,
        _ => return Err(Error::Invalid("navigation Claim forms source state")),
    };
    if !same_option_canonical(
        forms.as_ref().map(|(_, value)| value),
        input.claim_inputs.forms.as_ref().map(|(_, value)| value),
        limits.max_claim_cohort_bytes,
    )? || forms.as_ref().map(|(path, _)| path.as_str())
        != input
            .claim_inputs
            .forms
            .as_ref()
            .map(|(path, _)| path.as_str())
    {
        return Err(Error::Invalid("navigation Claim materialized forms differ"));
    }
    let (dependencies, source_files) = validate_dependencies(
        &input,
        limits.catalog.max_files,
        limits.max_claim_cohort_bytes,
    )?;
    validate_selected_files_match_dependencies(input.selected_files, &dependencies, &source_files)?;
    validate_inventory_dependencies(inventory, &dependencies, &source_files)?;
    let historical_schema_path = checked_path("ToS/contracts/historical-claim.schema.json")?;
    let historical_schema_bytes = if inventory
        .scan_costs
        .iter()
        .any(|cost| is_historical_claim_endpoint_path(&cost.source_path))
    {
        let bytes = source_files
            .get(historical_schema_path.as_str())
            .copied()
            .ok_or(Error::Invalid(
                "navigation historical Claim schema bytes absent",
            ))?;
        validate_required_dependency(
            &dependencies,
            &historical_schema_path,
            NavigationDependencyRoleV1::Additional,
            bytes,
        )?;
        Some(bytes)
    } else {
        None
    };
    if let Some(schema_bytes) = historical_schema_bytes {
        for cost in inventory
            .scan_costs
            .iter()
            .filter(|cost| is_historical_claim_endpoint_path(&cost.source_path))
        {
            let path = checked_path(&cost.source_path)?;
            let bytes = source_files
                .get(path.as_str())
                .copied()
                .ok_or(Error::Invalid(
                    "navigation historical Claim endpoint bytes absent",
                ))?;
            input.validator.check_selected_event_document_exact(
                selected_source_revision,
                path.as_str(),
                historical_schema_path.as_str(),
                schema_bytes,
                bytes,
            )?;
        }
    }
    if let Some(manifest_set) = refresh_manifest {
        for cause in manifest_set.causes() {
            let path = cause.changed_dependency_path();
            let bytes = source_files
                .get(path.as_str())
                .copied()
                .ok_or(Error::Invalid(
                    "navigation Claim refresh dependency bytes absent",
                ))?;
            let dependency_sha256 = dependency_present_sha256_any_role(&dependencies, path)?;
            if bytes.is_empty()
                || Digest256::of_bytes(bytes) != cause.changed_dependency_sha256()
                || dependency_sha256 != cause.changed_dependency_sha256()
            {
                return Err(Error::Invalid(
                    "navigation Claim refresh dependency digest differs",
                ));
            }
        }
    }
    validate_required_dependency(
        &dependencies,
        &source_path,
        NavigationDependencyRoleV1::Record,
        input.source_bytes,
    )?;
    validate_required_dependency(
        &dependencies,
        &claim_schema_path,
        NavigationDependencyRoleV1::RecordSchema,
        input.claim_schema_bytes,
    )?;
    validate_optional_dependency(
        &dependencies,
        &forms_path,
        NavigationDependencyRoleV1::HumanForms,
        input.forms_bytes,
    )?;
    let forms_schema_path = checked_path(input.forms_schema_path)?;
    if forms_schema_path.as_str() != "ToS/contracts/human-form-set.schema.json" {
        return Err(Error::Invalid("navigation Claim forms schema owner path"));
    }
    match (input.forms_bytes, input.forms_schema_bytes) {
        (Some(_), Some(schema_bytes)) => validate_required_dependency(
            &dependencies,
            &forms_schema_path,
            NavigationDependencyRoleV1::FormsSchema,
            schema_bytes,
        )?,
        (None, None) => {}
        _ => return Err(Error::Invalid("navigation Claim forms schema state")),
    }
    let history_path = checked_path(input.history_path)?;
    if mutation_receipt_path.as_ref() == Some(&history_path) {
        let receipt_bytes = mutation_receipt_bytes.ok_or(Error::Invalid(
            "navigation Claim mutation receipt bytes absent",
        ))?;
        if input.history_bytes != Some(receipt_bytes) {
            return Err(Error::Invalid(
                "navigation Claim mutation member is not exact history bytes",
            ));
        }
        validate_required_dependency(
            &dependencies,
            &history_path,
            NavigationDependencyRoleV1::RecordHistory,
            receipt_bytes,
        )?;
    } else {
        validate_optional_dependency(
            &dependencies,
            &history_path,
            NavigationDependencyRoleV1::RecordHistory,
            input.history_bytes,
        )?;
    }
    let provenance_event_path = checked_path(input.provenance_event_path)?;
    let provenance_event_schema_path = checked_path(input.provenance_event_schema_path)?;
    let provenance_receipt_path = checked_path(input.provenance_receipt_path)?;
    if history_path == provenance_receipt_path {
        return Err(Error::Invalid(
            "navigation Claim history aliases creation receipt",
        ));
    }
    let mutation_receipt_role = match mutation_receipt_path.as_ref() {
        Some(path) if path == &history_path => Some(NavigationDependencyRoleV1::RecordHistory),
        Some(path) if path == &provenance_receipt_path => {
            if mutation_receipt_bytes != Some(input.provenance_receipt_bytes) {
                return Err(Error::Invalid(
                    "navigation Claim mutation member differs from creation receipt",
                ));
            }
            Some(NavigationDependencyRoleV1::EventReceipt)
        }
        Some(_) => Some(NavigationDependencyRoleV1::Additional),
        None => None,
    };
    let entity_registry_path = checked_path(input.entity_registry_path)?;
    let entity_registry_schema_path = checked_path(input.entity_registry_schema_path)?;
    let relation_registry_path = checked_path(input.relation_registry_path)?;
    let relation_registry_schema_path = checked_path(input.relation_registry_schema_path)?;
    let expected_history_path = checked_path(&format!(
        "{}/claim-revision-history.json",
        source_path
            .as_str()
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .ok_or(Error::Invalid("navigation Claim source owner directory"))?
    ))?;
    if history_path != expected_history_path
        || entity_registry_path.as_str() != "ToS/doctrine/semantic-interchange/entity-types.v1.json"
        || entity_registry_schema_path.as_str()
            != "ToS/contracts/semantic-entity-type-registry.schema.json"
        || relation_registry_path.as_str()
            != "ToS/doctrine/semantic-interchange/relation-types.v1.json"
        || relation_registry_schema_path.as_str()
            != "ToS/contracts/semantic-relation-type-registry.schema.json"
    {
        return Err(Error::Invalid("navigation Claim owner dependency path"));
    }
    if let (Some(path), Some(role), Some(bytes)) = (
        mutation_receipt_path.as_ref(),
        mutation_receipt_role,
        mutation_receipt_bytes,
    ) {
        if role == NavigationDependencyRoleV1::Additional {
            validate_required_dependency(&dependencies, path, role, bytes)?;
        }
    }
    for (path, role, bytes) in [
        (
            &provenance_event_path,
            NavigationDependencyRoleV1::Event,
            input.provenance_event_bytes,
        ),
        (
            &provenance_event_schema_path,
            NavigationDependencyRoleV1::EventSchema,
            input.provenance_event_schema_bytes,
        ),
        (
            &provenance_receipt_path,
            NavigationDependencyRoleV1::EventReceipt,
            input.provenance_receipt_bytes,
        ),
        (
            &entity_registry_path,
            NavigationDependencyRoleV1::EntityRegistry,
            input.entity_registry_bytes,
        ),
        (
            &entity_registry_schema_path,
            NavigationDependencyRoleV1::EntityRegistrySchema,
            input.entity_registry_schema_bytes,
        ),
        (
            &relation_registry_path,
            NavigationDependencyRoleV1::Additional,
            input.relation_registry_bytes,
        ),
        (
            &relation_registry_schema_path,
            NavigationDependencyRoleV1::Additional,
            input.relation_registry_schema_bytes,
        ),
    ] {
        validate_required_dependency(&dependencies, path, role, bytes)?;
    }
    let entities = strict_value(input.entity_registry_bytes, limits.catalog.max_row_bytes)?;
    let relations = strict_value(input.relation_registry_bytes, limits.catalog.max_row_bytes)?;
    let _entity_schema = strict_value(
        input.entity_registry_schema_bytes,
        limits.catalog.max_row_bytes,
    )?;
    let _relation_schema = strict_value(
        input.relation_registry_schema_bytes,
        limits.catalog.max_row_bytes,
    )?;
    input.validator.check_selected_event_document_exact(
        selected_source_revision,
        entity_registry_path.as_str(),
        entity_registry_schema_path.as_str(),
        input.entity_registry_schema_bytes,
        input.entity_registry_bytes,
    )?;
    input.validator.check_selected_event_document_exact(
        selected_source_revision,
        relation_registry_path.as_str(),
        relation_registry_schema_path.as_str(),
        input.relation_registry_schema_bytes,
        input.relation_registry_bytes,
    )?;
    validate_identity_sources(
        &mut input,
        &dependencies,
        &source_files,
        selected_source_revision,
        limits,
    )?;
    let creation_binding =
        validate_provenance_event(&input, &claim, selected_source_revision, limits)?;
    let history_validation = validate_claim_history(
        input.history_bytes,
        &history_path,
        &source_path,
        owner_stream,
        &id,
        &claim,
        mutation_receipt_path.as_ref(),
        event_claim_set_binding,
        &event_mutation_claim_ids,
        input.event_id,
        input.event_sequence,
        &creation_binding,
        &dependencies,
        &source_files,
        limits,
        &mut claim_stream_rows_scanned,
    )?;
    let history_sha256 = history_validation.history_sha256;
    let latest_revision_receipt_sha256 = history_validation.latest_revision_receipt_sha256;
    let current_source_ref_sha256 = history_validation.current_source_ref_sha256;
    let revision_proof = history_validation.revision_proof;
    let descriptor = supplied_claim_navigation_descriptor(
        &claim,
        &input.claim_inputs.subject,
        &input.claim_inputs.object,
        &relations,
        &entities,
        BibliographicDocumentLimits {
            input_document_bytes: limits.catalog.max_row_bytes,
            output_row_bytes: limits.catalog.max_output_row_bytes,
        },
    )?;
    if !same_option_canonical(
        descriptor.as_ref(),
        input.claim_inputs.descriptor.as_ref(),
        limits.catalog.max_output_row_bytes,
    )? {
        return Err(Error::Invalid("navigation Claim descriptor binding"));
    }
    let (maker, maker_identity) = supplied_bibliographic_maker_with_fallback(
        &claim,
        input.claim_inputs.maker_identity.clone(),
        text(&entry, "source_claim_file_ref")?,
        text(&entry, "claim_sha256")?,
        limits.catalog.max_output_row_bytes,
    )?;
    if !same_canonical(
        &maker,
        &input.claim_inputs.maker,
        limits.catalog.max_output_row_bytes,
    )? || !same_option_canonical(
        maker_identity.as_ref(),
        input.claim_inputs.maker_identity.as_ref(),
        limits.catalog.max_output_row_bytes,
    )? {
        return Err(Error::Invalid("navigation Claim maker binding"));
    }
    let rendered = render_supplied_claim(input.claim_inputs, limits)?;
    let trace_sha256 = digest_json(&rendered.trace, limits.max_claim_cohort_bytes)?;
    let projection = NavigationClaimEventProjectionV1 {
        nodes: rendered.nodes,
        edges: rendered.edges,
        diagnostics: Vec::new(),
    };
    validate_external_claim_endpoints(
        &projection,
        inventory,
        input.selected_files,
        input.validator,
        &mut *input.forms_materializer,
        selected_source_revision,
        limits,
    )?;
    let (node_visibility, edge_visibility, external_endpoint_ids) =
        projection_visibility(&projection, inventory)?;
    let dependency_root = dependency_root(&dependencies)?;
    let rows = output_rows(&projection, &node_visibility, &edge_visibility)?.0;
    let output_root = output_root(
        &rows,
        &projection.diagnostics,
        trace_sha256,
        &external_endpoint_ids,
    )?;
    let owner_inventory_root = inventory.root;
    let owner_inventory_scan_costs = inventory.scan_costs.clone();
    let (source_event_id, source_event_version) = provenance_identity(
        &input.provenance_event_bytes,
        &input.provenance_receipt_path,
        limits,
    )?;
    match cause {
        NavigationClaimEventCauseInputV1::Mutation { .. } => {
            let mutation_source_path = mutation_source_path.ok_or(Error::Invalid(
                "navigation Claim mutation source path absent",
            ))?;
            let mutation_receipt_path = mutation_receipt_path.ok_or(Error::Invalid(
                "navigation Claim mutation receipt path absent",
            ))?;
            let mutation_receipt_role = mutation_receipt_role.ok_or(Error::Invalid(
                "navigation Claim mutation receipt role absent",
            ))?;
            let event_claim_set_binding = event_claim_set_binding.ok_or(Error::Invalid(
                "navigation Claim mutation receipt binding absent",
            ))?;
            let affected_owner_manifest_root = affected_owner_manifest_root.ok_or(
                Error::Invalid("navigation Claim affected-owner manifest root absent"),
            )?;
            let event_claim_set_root = claim_set_root(
                input.event_id,
                input.event_sequence,
                &mutation_receipt_path,
                event_claim_set_binding,
                affected_owner_manifest_root,
            )?;
            let input_root = input_root(
                input.event_id,
                input.event_sequence,
                &source_event_id,
                source_event_version,
                input.predecessor_source_revision,
                source_revision,
                input.predecessor_cohort_root,
                &source_path,
                input.source_line,
                &id,
                &mutation_receipt_path,
                &mutation_source_path,
                dependency_root,
                output_root,
                event_claim_set_root,
                owner_inventory_root,
                revision_proof.as_ref().map(|proof| proof.binding_root),
                claim_stream_rows_scanned,
            )?;
            let cohort = NavigationClaimEventCohortV1 {
                event_id: input.event_id.to_owned(),
                event_sequence: input.event_sequence,
                source_event_id,
                source_event_version,
                predecessor_source_revision: input.predecessor_source_revision,
                source_revision,
                mutation_source_path,
                source_path,
                record_id: id,
                source_line: input.source_line,
                mutation_receipt_path,
                mutation_receipt_role,
                predecessor_cohort_root: input.predecessor_cohort_root,
                dependency_root,
                input_root,
                output_root,
                event_mutation_claim_ids,
                event_claim_owner_ids,
                claim_stream_sha256,
                claim_stream_rows_scanned,
                event_claim_set_binding,
                event_claim_set_root,
                affected_owner_manifest_root,
                owner_inventory_root,
                owner_inventory_scan_costs,
                revision_proof,
                trace_sha256,
                dependencies,
                projection,
                external_endpoint_ids,
                node_visibility,
                edge_visibility,
                _sealed: NavigationClaimEventCohortSeal,
            };
            cohort.verify_integrity()?;
            Ok(NavigationClaimEventContributionV1::Mutation(cohort))
        }
        NavigationClaimEventCauseInputV1::DependencyRefresh { .. } => {
            let affected_owner_manifest_set = refresh_manifest.ok_or(Error::Invalid(
                "navigation Claim refresh owner manifest absent",
            ))?;
            let claim_version = claim
                .get("claim_version")
                .and_then(Value::as_u64)
                .filter(|version| *version > 0)
                .ok_or(Error::Invalid(
                    "navigation Claim refresh current version absent",
                ))?;
            let mut proof = NavigationClaimDependencyRefreshProofV1 {
                event_id: input.event_id.to_owned(),
                event_sequence: input.event_sequence,
                changed_dependencies: affected_owner_manifest_set.causes().to_vec(),
                closure_root: affected_owner_manifest_set.closure_root(),
                affected_owner_manifest_root: affected_owner_manifest_set.root(),
                reverse_claim_ids: affected_owner_manifest_set.reverse_claim_ids().to_vec(),
                claim_stream_path: source_path.clone(),
                claim_stream_sha256,
                claim_id: id.clone(),
                claim_version,
                claim_source_line: input.source_line,
                claim_row_sha256: Digest256::of_bytes(&claim_bytes),
                current_source_ref_sha256,
                provenance_event_path: creation_binding.event_path,
                provenance_event_id: creation_binding.event_id,
                provenance_event_version: creation_binding.event_version,
                provenance_event_sha256: creation_binding.event_sha256,
                provenance_receipt_path: creation_binding.receipt_path,
                provenance_receipt_sha256: creation_binding.receipt_sha256,
                history_path: input.history_bytes.map(|_| history_path.clone()),
                history_sha256,
                latest_revision_receipt_sha256,
                binding_root: Digest256::of_bytes(&[]),
            };
            proof.binding_root = dependency_refresh_proof_root(&proof)?;
            let event_basis_root = proof.binding_root;
            let input_root = dependency_refresh_input_root(
                input.event_id,
                input.event_sequence,
                &source_event_id,
                source_event_version,
                input.predecessor_source_revision,
                source_revision,
                input.predecessor_cohort_root,
                &source_path,
                input.source_line,
                &id,
                dependency_root,
                output_root,
                event_basis_root,
                owner_inventory_root,
                claim_stream_rows_scanned,
            )?;
            let cohort = NavigationClaimDependencyRefreshCohortV1 {
                event_id: input.event_id.to_owned(),
                event_sequence: input.event_sequence,
                source_event_id,
                source_event_version,
                predecessor_source_revision: input.predecessor_source_revision,
                source_revision,
                predecessor_cohort_root: input.predecessor_cohort_root,
                source_path,
                record_id: id,
                source_line: input.source_line,
                dependency_root,
                input_root,
                output_root,
                event_basis_root,
                owner_inventory_root,
                owner_inventory_scan_costs,
                event_claim_owner_ids,
                claim_stream_sha256,
                claim_stream_rows_scanned,
                proof,
                trace_sha256,
                dependencies,
                projection,
                external_endpoint_ids,
                node_visibility,
                edge_visibility,
                _sealed: NavigationClaimEventCohortSeal,
            };
            cohort.verify_integrity()?;
            Ok(NavigationClaimEventContributionV1::DependencyRefresh(
                cohort,
            ))
        }
    }
}

fn valid_event_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= EVENT_ID_BYTES
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn checked_path(value: &str) -> Result<RelativePath> {
    if value.is_empty() || value.len() > PATH_BYTES {
        return Err(Error::Invalid("navigation Claim dependency path bounds"));
    }
    RelativePath::parse(value).map_err(|_| Error::Invalid("navigation Claim dependency path"))
}

fn mutation_claim_ids(raw: &[u8], cap: usize) -> Result<(Vec<String>, Digest256)> {
    let receipt = strict_value(raw, cap)?;
    let mut ids = if let Some(claims) = receipt.get("claims").and_then(Value::as_array) {
        claims
            .iter()
            .map(|claim| {
                claim
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| id.starts_with("tos.claim.") && id.len() <= PATH_BYTES)
                    .map(str::to_owned)
                    .ok_or(Error::Invalid("navigation Claim mutation receipt id"))
            })
            .collect::<Result<Vec<_>>>()?
    } else if let Some(receipts) = receipt.get("receipts").and_then(Value::as_array) {
        receipts
            .last()
            .and_then(|last| last.pointer("/source/id"))
            .and_then(Value::as_str)
            .filter(|id| id.starts_with("tos.claim.") && id.len() <= PATH_BYTES)
            .map(|id| vec![id.to_owned()])
            .ok_or(Error::Invalid("navigation Claim history mutation id"))?
    } else {
        vec![
            receipt
                .pointer("/source/id")
                .and_then(Value::as_str)
                .filter(|id| id.starts_with("tos.claim.") && id.len() <= PATH_BYTES)
                .ok_or(Error::Invalid("navigation Claim revision receipt id"))?
                .to_owned(),
        ]
    };
    ids.sort();
    if ids.is_empty() || ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Invalid(
            "navigation Claim mutation receipt member set",
        ));
    }
    Ok((ids, Digest256::of_bytes(raw)))
}

/// Extract the exact, sorted current Claim IDs named by a retained CMD
/// mutation receipt. This exposes the same bounded receipt parser used by the
/// event renderer to the PG owner that prepares the complete replacement
/// closure.
pub fn selected_claim_mutation_ids_v1(raw: &[u8], max_bytes: usize) -> Result<Vec<String>> {
    mutation_claim_ids(raw, max_bytes).map(|(ids, _)| ids)
}

fn same_canonical(left: &Value, right: &Value, cap: usize) -> Result<bool> {
    Ok(canonical(left, cap)? == canonical(right, cap)?)
}

fn same_option_canonical(left: Option<&Value>, right: Option<&Value>, cap: usize) -> Result<bool> {
    match (left, right) {
        (None, None) => Ok(true),
        (Some(left), Some(right)) => same_canonical(left, right, cap),
        _ => Ok(false),
    }
}

fn strict_value(raw: &[u8], cap: usize) -> Result<Value> {
    if raw.is_empty() || raw.len() > cap {
        return Err(Error::Budget("navigation Claim source document bytes"));
    }
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation Claim JSON limits"))?;
    let document =
        tos_foundation::parse_json(raw, tos_foundation::JsonMode::PublishedStrict, limits)
            .map_err(|_| Error::Invalid("navigation Claim strict source JSON"))?;
    let normalized = tos_foundation::emit_value_preserved_json(document.root(), limits)
        .map_err(|_| Error::Budget("navigation Claim source JSON bytes"))?;
    serde_json::from_slice(&normalized)
        .map_err(|_| Error::Invalid("navigation Claim source JSON value"))
}

fn canonical(value: &Value, cap: usize) -> Result<Vec<u8>> {
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation Claim canonical limits"))?;
    let raw = serde_json::to_vec(value).map_err(|_| Error::Invalid("navigation Claim JSON"))?;
    let typed = tos_foundation::parse_json(&raw, tos_foundation::JsonMode::PublishedStrict, limits)
        .map_err(|_| Error::Invalid("navigation Claim JSON"))?;
    canonical_bytes_v1(
        &typed.into_root(),
        CanonicalProfile::CorpusSnapshotV1,
        limits,
    )
    .map_err(|_| Error::Budget("navigation Claim canonical bytes"))
}

fn digest_json(value: &Value, cap: usize) -> Result<Digest256> {
    Ok(Digest256::of_bytes(&canonical(value, cap)?))
}

fn claim_forms_path(source_path: &str, id: &str) -> Result<RelativePath> {
    let prefix = source_path
        .strip_suffix(".jsonl")
        .ok_or(Error::Invalid("navigation Claim stream extension"))?;
    checked_path(&format!(
        "{prefix}.{}.human-forms.json",
        Digest256::of_bytes(id.as_bytes()).to_hex()
    ))
}

fn validate_dependencies<'a>(
    input: &NavigationClaimEventCommonInputV1<'a>,
    max_files: usize,
    max_manifest_bytes: usize,
) -> Result<(Vec<NavigationDependencyV1>, BTreeMap<String, &'a [u8]>)> {
    if input.dependencies.is_empty()
        || input.dependencies.len() > max_files
        || input.source_files.len() > max_files
    {
        return Err(Error::Budget("navigation Claim dependency count"));
    }
    let mut dependencies = input.dependencies.to_vec();
    dependencies.sort_by(|left, right| left.path.cmp(&right.path));
    if dependencies
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path)
    {
        return Err(Error::Invalid("navigation Claim duplicate dependency"));
    }

    let mut source_files = BTreeMap::new();
    let mut total_bytes = 0usize;
    for file in input.source_files {
        let path = checked_path(file.path)?;
        total_bytes = total_bytes
            .checked_add(file.bytes.len())
            .filter(|bytes| *bytes <= TOTAL_DEPENDENCY_BYTES)
            .ok_or(Error::Budget("navigation Claim dependency bytes"))?;
        if source_files
            .insert(path.as_str().to_owned(), file.bytes)
            .is_some()
        {
            return Err(Error::Invalid(
                "navigation Claim duplicate or empty source file",
            ));
        }
    }

    let mut dependency_bytes = 0usize;
    for dependency in &dependencies {
        dependency_bytes = dependency_bytes
            .checked_add(dependency.path.as_str().len())
            .filter(|bytes| *bytes <= max_manifest_bytes)
            .ok_or(Error::Budget("navigation Claim dependency manifest"))?;
        match &dependency.state {
            NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                let bytes = source_files
                    .get(dependency.path.as_str())
                    .ok_or(Error::Invalid("navigation Claim dependency bytes absent"))?;
                dependency_bytes = dependency_bytes
                    .checked_add(72)
                    .filter(|bytes| *bytes <= max_manifest_bytes)
                    .ok_or(Error::Budget("navigation Claim dependency manifest"))?;
                if *size_bytes != bytes.len() as u64 || *sha256 != Digest256::of_bytes(bytes) {
                    return Err(Error::Invalid("navigation Claim dependency fixity differs"));
                }
            }
            NavigationDependencyStateV1::Absent { reason } => {
                dependency_bytes = dependency_bytes
                    .checked_add(reason.len())
                    .filter(|bytes| *bytes <= max_manifest_bytes)
                    .ok_or(Error::Budget("navigation Claim dependency manifest"))?;
                if reason.is_empty()
                    || reason.len() > 128
                    || source_files.contains_key(dependency.path.as_str())
                {
                    return Err(Error::Invalid("navigation Claim absent dependency witness"));
                }
            }
        }
    }
    if source_files.keys().any(|path| {
        dependencies
            .binary_search_by(|row| row.path.as_str().cmp(path.as_str()))
            .is_err()
    }) {
        return Err(Error::Invalid("navigation Claim unmanifested source file"));
    }
    Ok((dependencies, source_files))
}

fn validate_selected_files_match_dependencies(
    selected_files: &SelectedBibliographicFilesV1,
    dependencies: &[NavigationDependencyV1],
    source_files: &BTreeMap<String, &[u8]>,
) -> Result<()> {
    if selected_files.observed_path_count() != dependencies.len()
        || selected_files.paths().count() != source_files.len()
        || selected_files
            .paths()
            .any(|path| !source_files.contains_key(path))
    {
        return Err(Error::Invalid(
            "navigation Claim selected-file closure differs",
        ));
    }
    for (path, bytes) in source_files {
        if selected_files.required(path)? != *bytes {
            return Err(Error::Invalid(
                "navigation Claim selected-file bytes differ",
            ));
        }
    }
    for dependency in dependencies {
        let selected = selected_files.optional(dependency.path.as_str())?;
        match (&dependency.state, selected) {
            (NavigationDependencyStateV1::Present { .. }, Some(bytes))
                if source_files.get(dependency.path.as_str()).copied() == Some(bytes) => {}
            (NavigationDependencyStateV1::Absent { .. }, None) => {}
            _ => {
                return Err(Error::Invalid(
                    "navigation Claim selected-file observation differs",
                ));
            }
        }
    }
    Ok(())
}

fn validate_external_claim_endpoints(
    projection: &NavigationClaimEventProjectionV1,
    inventory: &NavigationClaimEventOwnerInventoryV1,
    selected_files: &SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    forms_materializer: &mut dyn BibliographicForms,
    event_revision: Digest256,
    limits: BibliographicLimits,
) -> Result<()> {
    let local_nodes = projection
        .nodes
        .iter()
        .filter_map(|node| node.get("node_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let mut endpoint_ids = BTreeSet::new();
    for edge in &projection.edges {
        let endpoint = edge
            .get("to_id")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation Claim endpoint target"))?;
        if local_nodes.contains(endpoint) {
            continue;
        }
        if let Some(claim_id) = endpoint.strip_prefix("claim:") {
            if claim_id.is_empty() || claim_id.len() > PATH_BYTES {
                return Err(Error::Invalid("navigation Claim endpoint identity"));
            }
            endpoint_ids.insert(claim_id.to_owned());
        }
    }
    for claim_id in endpoint_ids {
        let target = inventory
            .claim_by_id(&claim_id)
            .ok_or(Error::Invalid("navigation Claim endpoint source absent"))?;
        let schema_path = if is_claim_stream_path(target.source_path) {
            "ToS/contracts/source-claim-record.schema.json"
        } else if is_historical_claim_endpoint_path(target.source_path) {
            "ToS/contracts/historical-claim.schema.json"
        } else {
            return Err(Error::Invalid("navigation Claim endpoint source path"));
        };
        let entry = render_catalog_claim(
            target.claim,
            target.source_path,
            target.source_line,
            Some(schema_path),
            limits.catalog.max_output_row_bytes,
        )?;
        let exact = claim_reference(target.claim, limits.catalog.max_row_bytes)?;
        let resolved = resolve_selected_claim_version_v1(
            &entry,
            &exact,
            selected_files,
            validator,
            event_revision,
            forms_materializer,
            limits,
        )?;
        if resolved.status != "available"
            || resolved.version_status != Some("current")
            || resolved.source_path != target.source_path
            || resolved.current_record != *target.claim
            || resolved
                .record
                .as_ref()
                .is_none_or(|record| record != target.claim)
        {
            return Err(Error::Invalid(
                "navigation Claim endpoint selected version differs",
            ));
        }
    }
    Ok(())
}

fn validate_required_dependency(
    dependencies: &[NavigationDependencyV1],
    path: &RelativePath,
    role: NavigationDependencyRoleV1,
    bytes: &[u8],
) -> Result<()> {
    let dependency = dependencies
        .binary_search_by(|row| row.path.cmp(path))
        .ok()
        .map(|index| &dependencies[index])
        .ok_or(Error::Invalid(
            "navigation Claim required dependency absent",
        ))?;
    let exact_present = match &dependency.state {
        NavigationDependencyStateV1::Present { sha256, size_bytes } => {
            *sha256 == Digest256::of_bytes(bytes) && *size_bytes == bytes.len() as u64
        }
        NavigationDependencyStateV1::Absent { .. } => false,
    };
    if dependency.role != role || !exact_present {
        return Err(Error::Invalid(
            "navigation Claim required dependency differs",
        ));
    }
    Ok(())
}

fn dependency_present_digest(
    dependencies: &[NavigationDependencyV1],
    path: &RelativePath,
    role: NavigationDependencyRoleV1,
) -> Result<Digest256> {
    let dependency = dependencies
        .binary_search_by(|row| row.path.cmp(path))
        .ok()
        .map(|index| &dependencies[index])
        .ok_or(Error::Invalid("navigation Claim dependency absent"))?;
    match &dependency.state {
        NavigationDependencyStateV1::Present { sha256, size_bytes }
            if dependency.role == role && *size_bytes > 0 =>
        {
            Ok(*sha256)
        }
        _ => Err(Error::Invalid("navigation Claim dependency digest absent")),
    }
}

fn dependency_present_sha256_any_role(
    dependencies: &[NavigationDependencyV1],
    path: &RelativePath,
) -> Result<Digest256> {
    let dependency = dependencies
        .binary_search_by(|row| row.path.cmp(path))
        .ok()
        .map(|index| &dependencies[index])
        .ok_or(Error::Invalid("navigation Claim dependency absent"))?;
    match &dependency.state {
        NavigationDependencyStateV1::Present { sha256, .. } => Ok(*sha256),
        NavigationDependencyStateV1::Absent { .. } => {
            Err(Error::Invalid("navigation Claim dependency digest absent"))
        }
    }
}

fn validate_optional_dependency(
    dependencies: &[NavigationDependencyV1],
    path: &RelativePath,
    role: NavigationDependencyRoleV1,
    bytes: Option<&[u8]>,
) -> Result<()> {
    let dependency = dependencies
        .binary_search_by(|row| row.path.cmp(path))
        .ok()
        .map(|index| &dependencies[index])
        .ok_or(Error::Invalid(
            "navigation Claim optional dependency absent",
        ))?;
    if dependency.role != role {
        return Err(Error::Invalid("navigation Claim optional dependency role"));
    }
    match (bytes, &dependency.state) {
        (Some(bytes), NavigationDependencyStateV1::Present { sha256, size_bytes })
            if !bytes.is_empty()
                && *sha256 == Digest256::of_bytes(bytes)
                && *size_bytes == bytes.len() as u64 =>
        {
            Ok(())
        }
        (None, NavigationDependencyStateV1::Absent { reason })
            if !reason.is_empty() && reason.len() <= 128 =>
        {
            Ok(())
        }
        _ => Err(Error::Invalid("navigation Claim optional dependency state")),
    }
}

fn validate_inventory_dependencies(
    inventory: &NavigationClaimEventOwnerInventoryV1,
    dependencies: &[NavigationDependencyV1],
    source_files: &BTreeMap<String, &[u8]>,
) -> Result<()> {
    if owner_inventory_root(&inventory.scan_costs)? != inventory.root {
        return Err(Error::Invalid("navigation Claim inventory root differs"));
    }
    for cost in &inventory.scan_costs {
        let bytes = source_files
            .get(&cost.source_path)
            .copied()
            .ok_or(Error::Invalid(
                "navigation Claim inventory stream bytes absent",
            ))?;
        if Digest256::of_bytes(bytes) != cost.stream_sha256 {
            return Err(Error::Invalid(
                "navigation Claim inventory stream bytes differ",
            ));
        }
        let path = checked_path(&cost.source_path)?;
        let dependency = dependencies
            .binary_search_by(|row| row.path.cmp(&path))
            .ok()
            .map(|index| &dependencies[index])
            .ok_or(Error::Invalid(
                "navigation Claim inventory dependency absent",
            ))?;
        let expected_role = if is_claim_stream_path(&cost.source_path) {
            NavigationDependencyRoleV1::Record
        } else if is_historical_claim_endpoint_path(&cost.source_path) {
            NavigationDependencyRoleV1::Additional
        } else {
            return Err(Error::Invalid(
                "navigation Claim inventory stream path kind",
            ));
        };
        match &dependency.state {
            NavigationDependencyStateV1::Present { sha256, size_bytes }
                if dependency.role == expected_role
                    && *sha256 == cost.stream_sha256
                    && *size_bytes == bytes.len() as u64 => {}
            _ => {
                return Err(Error::Invalid(
                    "navigation Claim inventory dependency differs",
                ));
            }
        }
    }
    Ok(())
}

fn is_claim_stream_path(path: &str) -> bool {
    path.starts_with("ToS/source-witnesses/relations/")
        && path.ends_with("/source-claims.jsonl")
        && !path.contains("/owner-local/")
        && !path.contains("/private/")
}

fn is_historical_claim_endpoint_path(path: &str) -> bool {
    path.starts_with("ToS/source-witnesses/history/")
        && path.ends_with("/historical-claims.jsonl")
        && !path.contains("/.record-revisions/")
}

fn python_bytes_blank(raw: &[u8]) -> bool {
    raw.iter()
        .all(|byte| matches!(*byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
}

fn parse_claim_owner_stream(
    raw: &[u8],
    cap: usize,
) -> Result<(BTreeMap<String, NavigationClaimEventOwnerRowV1>, u64)> {
    if raw.is_empty() || raw.len() > 1_048_576 {
        return Err(Error::Budget("navigation Claim stream bounds"));
    }
    let mut rows = BTreeMap::new();
    let mut rows_scanned = 0u64;
    for (index, raw_line) in raw.split(|byte| *byte == b'\n').enumerate() {
        if python_bytes_blank(raw_line) {
            continue;
        }
        count_claim_stream_rows(&mut rows_scanned, 1)?;
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.len() > cap {
            return Err(Error::Budget("navigation Claim stream row bytes"));
        }
        let claim = strict_value(line, cap)?;
        let id = claim
            .get("claim_id")
            .and_then(Value::as_str)
            .filter(|id| {
                id.starts_with("tos.claim.")
                    && id.len() <= PATH_BYTES
                    && !id.bytes().any(|byte| byte.is_ascii_control())
            })
            .ok_or(Error::Invalid("navigation Claim stream member id"))?
            .to_owned();
        if claim
            .get("claim_version")
            .and_then(Value::as_u64)
            .is_none_or(|version| version == 0)
        {
            return Err(Error::Invalid("navigation Claim stream member version"));
        }
        if rows.len() >= MAX_CLAIM_EVENT_OWNERS && !rows.contains_key(&id) {
            return Err(Error::Budget("navigation Claim owner count"));
        }
        if rows
            .insert(
                id,
                NavigationClaimEventOwnerRowV1 {
                    source_line: index as u64 + 1,
                    exact_row_bytes: raw_line.to_vec(),
                    claim,
                },
            )
            .is_some()
        {
            return Err(Error::Invalid("navigation duplicate Claim stream member"));
        }
    }
    if rows.is_empty() {
        return Err(Error::Invalid("navigation Claim stream has no owners"));
    }
    Ok((rows, rows_scanned))
}

fn parse_historical_claim_endpoint_stream(
    raw: &[u8],
    cap: usize,
) -> Result<(BTreeMap<String, NavigationClaimEventOwnerRowV1>, u64)> {
    if raw.is_empty() || raw.len() > 1_048_576 {
        return Err(Error::Budget(
            "navigation historical Claim endpoint stream bounds",
        ));
    }
    let mut rows = BTreeMap::new();
    let mut rows_scanned = 0u64;
    for (index, raw_line) in raw.split(|byte| *byte == b'\n').enumerate() {
        if python_bytes_blank(raw_line) {
            continue;
        }
        count_claim_stream_rows(&mut rows_scanned, 1)?;
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.len() > cap {
            return Err(Error::Budget(
                "navigation historical Claim endpoint row bytes",
            ));
        }
        let claim = strict_value(line, cap)?;
        let id = claim
            .get("claim_id")
            .and_then(Value::as_str)
            .filter(|id| {
                id.starts_with("tos.claim.")
                    && id.len() <= PATH_BYTES
                    && !id.bytes().any(|byte| byte.is_ascii_control())
            })
            .ok_or(Error::Invalid(
                "navigation historical Claim endpoint member id",
            ))?
            .to_owned();
        if claim.get("schema_version").and_then(Value::as_str) != Some("tos_historical_claim_v1")
            || claim
                .get("claim_version")
                .and_then(Value::as_u64)
                .is_none_or(|version| version == 0)
        {
            return Err(Error::Invalid(
                "navigation historical Claim endpoint member version",
            ));
        }
        if rows.len() >= MAX_CLAIM_EVENT_OWNERS && !rows.contains_key(&id) {
            return Err(Error::Budget(
                "navigation historical Claim endpoint owner count",
            ));
        }
        if rows
            .insert(
                id,
                NavigationClaimEventOwnerRowV1 {
                    source_line: index as u64 + 1,
                    exact_row_bytes: raw_line.to_vec(),
                    claim,
                },
            )
            .is_some()
        {
            return Err(Error::Invalid(
                "navigation duplicate historical Claim endpoint member",
            ));
        }
    }
    if rows.is_empty() {
        return Err(Error::Invalid(
            "navigation historical Claim endpoint stream has no owners",
        ));
    }
    Ok((rows, rows_scanned))
}

fn claim_stream_rows(
    raw: &[u8],
    cap: usize,
    claim_stream_rows_scanned: &mut u64,
) -> Result<BTreeMap<String, Value>> {
    if raw.is_empty() || raw.len() > TOTAL_DEPENDENCY_BYTES {
        return Err(Error::Budget("navigation Claim stream bounds"));
    }
    let (rows, scanned) = parse_claim_owner_stream(raw, cap)?;
    count_claim_stream_rows(claim_stream_rows_scanned, scanned)?;
    Ok(rows.into_iter().map(|(id, row)| (id, row.claim)).collect())
}

#[cfg(test)]
fn claim_stream_inventory(
    raw: &[u8],
    owner_claim_ids: &[String],
    cap: usize,
    claim_stream_rows_scanned: &mut u64,
) -> Result<(BTreeMap<String, Value>, Digest256)> {
    if owner_claim_ids.is_empty()
        || owner_claim_ids.len() > MAX_CLAIM_EVENT_OWNERS
        || !sorted_unique(owner_claim_ids)
    {
        return Err(Error::Invalid("navigation Claim owner inventory shape"));
    }
    let rows = claim_stream_rows(raw, cap, claim_stream_rows_scanned)?;
    if rows.keys().ne(owner_claim_ids.iter()) {
        return Err(Error::Invalid(
            "navigation Claim owner inventory differs from exact stream",
        ));
    }
    Ok((rows, Digest256::of_bytes(raw)))
}

fn count_claim_stream_rows(scanned: &mut u64, count: u64) -> Result<()> {
    *scanned = scanned
        .checked_add(count)
        .ok_or(Error::Budget("navigation Claim stream scan work"))?;
    Ok(())
}

fn claim_reference(claim: &Value, cap: usize) -> Result<Value> {
    let id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("navigation Claim reference id"))?;
    let version = claim
        .get("claim_version")
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or(Error::Invalid("navigation Claim reference version"))?;
    Ok(json!({
        "id":id,
        "version":version,
        "digest":format!("sha256:{}", source_digest(claim, cap)?)
    }))
}

fn verify_claim_archive(
    archive_path: &str,
    source_path: &RelativePath,
    previous_ref: &Value,
    previous_revision: &str,
    earlier_history: &[Value],
    dependencies: &[NavigationDependencyV1],
    source_files: &BTreeMap<String, &[u8]>,
    limits: BibliographicLimits,
    claim_stream_rows_scanned: &mut u64,
) -> Result<NavigationClaimArchiveBindingV1> {
    let previous_id = previous_ref
        .get("id")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("navigation Claim archive source id"))?;
    let revision_hex = previous_revision
        .strip_prefix("sha256:")
        .ok_or(Error::Invalid("navigation Claim archive revision"))?;
    Digest256::from_hex(revision_hex)
        .map_err(|_| Error::Invalid("navigation Claim archive revision"))?;
    let expected_archive = format!(
        "ToS/source-witnesses/.record-revisions/{}-{revision_hex}",
        Digest256::of_bytes(previous_id.as_bytes()).to_hex()
    );
    if archive_path != expected_archive {
        return Err(Error::Invalid("navigation Claim archive source locator"));
    }
    let archive = checked_path(archive_path)?;
    let manifest_path = checked_path(&format!("{archive_path}/manifest.json"))?;
    let manifest_bytes = source_files
        .get(manifest_path.as_str())
        .copied()
        .ok_or(Error::Invalid("navigation Claim archive manifest absent"))?;
    validate_required_dependency(
        dependencies,
        &manifest_path,
        NavigationDependencyRoleV1::VersionSource,
        manifest_bytes,
    )?;
    let manifest = strict_value(manifest_bytes, limits.catalog.max_row_bytes)?;
    if !matches!(
        manifest.get("schema_version").and_then(Value::as_str),
        Some("tos_source_package_archive_v1" | "tos_source_package_archive_v2")
    ) || manifest.get("source_path").and_then(Value::as_str) != Some(source_path.as_str())
        || manifest.get("source") != Some(previous_ref)
        || manifest.get("revision").and_then(Value::as_str) != Some(previous_revision)
        || manifest.get("schema_version").and_then(Value::as_str)
            == Some("tos_source_package_archive_v2")
            && manifest.get("publication_protocol").and_then(Value::as_str)
                != Some("tos_selected_source_metadata_v1")
    {
        return Err(Error::Invalid("navigation Claim archive manifest binding"));
    }
    let bindings = manifest
        .get("files")
        .and_then(Value::as_object)
        .filter(|files| !files.is_empty() && files.len() <= 64)
        .ok_or(Error::Invalid("navigation Claim archive file bindings"))?;
    let stream_name = source_path
        .as_str()
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("navigation Claim archive stream name"))?;
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    let mut archive_member_paths = BTreeSet::from([manifest_path.as_str().to_owned()]);
    let mut total_bytes = manifest_bytes.len();
    for (name, binding) in bindings {
        if name.is_empty()
            || name
                .chars()
                .any(|character| matches!(character, '/' | '\\' | '\0'))
            || matches!(
                name.as_str(),
                "." | ".." | "payload" | "owner-local" | "private" | "catalog"
            )
            || name.starts_with('.')
        {
            return Err(Error::Invalid("navigation Claim archive member name"));
        }
        let sha = binding
            .get("sha256")
            .and_then(Value::as_str)
            .and_then(|value| value.strip_prefix("sha256:"))
            .ok_or(Error::Invalid("navigation Claim archive member digest"))?;
        let digest = Digest256::from_hex(sha)
            .map_err(|_| Error::Invalid("navigation Claim archive member digest"))?;
        let blob = format!("{sha}.blob");
        if binding.get("blob").and_then(Value::as_str) != Some(blob.as_str()) {
            return Err(Error::Invalid("navigation Claim archive blob name"));
        }
        let blob_path = checked_path(&format!("{archive_path}/{blob}"))?;
        let bytes = source_files
            .get(blob_path.as_str())
            .copied()
            .ok_or(Error::Invalid("navigation Claim archive blob absent"))?;
        validate_required_dependency(
            dependencies,
            &blob_path,
            NavigationDependencyRoleV1::VersionSource,
            bytes,
        )?;
        if Digest256::of_bytes(bytes) != digest
            || binding.get("bytes").and_then(Value::as_u64) != Some(bytes.len() as u64)
        {
            return Err(Error::Invalid("navigation Claim archive blob fixity"));
        }
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .filter(|count| *count <= 10 * 1024 * 1024)
            .ok_or(Error::Budget("navigation Claim archive bytes"))?;
        archive_member_paths.insert(blob_path.as_str().to_owned());
        files.insert(name.clone(), bytes.to_vec());
    }
    if files.get(stream_name).is_none() {
        return Err(Error::Invalid("navigation Claim archive stream absent"));
    }
    let expected_revision = {
        let bindings = files
            .iter()
            .map(|(name, bytes)| {
                (
                    name.clone(),
                    json!({
                        "sha256":format!("sha256:{}", Digest256::of_bytes(bytes).to_hex()),
                        "bytes":bytes.len()
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        format!(
            "sha256:{}",
            source_digest(&Value::Object(bindings), limits.max_claim_cohort_bytes)?
        )
    };
    if expected_revision != previous_revision {
        return Err(Error::Invalid("navigation Claim archive package revision"));
    }
    let archived_stream = files
        .get(stream_name)
        .ok_or(Error::Invalid("navigation Claim archive stream absent"))?;
    let old_rows = claim_stream_rows(
        archived_stream,
        limits.catalog.max_row_bytes,
        claim_stream_rows_scanned,
    )?;
    let old_claim = old_rows.get(previous_id).ok_or(Error::Invalid(
        "navigation Claim archived predecessor absent",
    ))?;
    if !same_canonical(
        &claim_reference(old_claim, limits.catalog.max_row_bytes)?,
        previous_ref,
        limits.catalog.max_row_bytes,
    )? {
        return Err(Error::Invalid(
            "navigation Claim archived predecessor differs",
        ));
    }
    if let Some(history_bytes) = files.get("claim-revision-history.json") {
        let archived_history = strict_value(history_bytes, limits.catalog.max_row_bytes)?;
        if archived_history
            .get("schema_version")
            .and_then(Value::as_str)
            != Some("tos_claim_revision_history_v1")
            || archived_history.get("source_path").and_then(Value::as_str)
                != Some(source_path.as_str())
            || archived_history
                .get("receipts")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                != Some(earlier_history)
        {
            return Err(Error::Invalid("navigation Claim archived history prefix"));
        }
    } else if !earlier_history.is_empty() {
        return Err(Error::Invalid("navigation Claim archived history absent"));
    }
    let prefix = format!("{archive_path}/");
    if source_files
        .keys()
        .filter(|path| path.starts_with(&prefix))
        .any(|path| !archive_member_paths.contains(path))
    {
        return Err(Error::Invalid(
            "navigation Claim unreferenced archive member",
        ));
    }
    let package_root = Digest256::from_hex(
        previous_revision
            .strip_prefix("sha256:")
            .ok_or(Error::Invalid("navigation Claim archive revision"))?,
    )
    .map_err(|_| Error::Invalid("navigation Claim archive revision"))?;
    Ok(NavigationClaimArchiveBindingV1 {
        manifest_sha256: Digest256::of_bytes(manifest_bytes),
        package_root,
    })
}

struct NavigationClaimHistoryValidationV1 {
    revision_proof: Option<NavigationClaimRevisionProofV1>,
    history_sha256: Option<Digest256>,
    latest_revision_receipt_sha256: Option<Digest256>,
    current_source_ref_sha256: Digest256,
}

fn validate_claim_history(
    history_bytes: Option<&[u8]>,
    history_path: &RelativePath,
    source_path: &RelativePath,
    current_stream: &NavigationClaimEventOwnerStreamV1,
    claim_id: &str,
    current_claim: &Value,
    mutation_receipt_path: Option<&RelativePath>,
    mutation_receipt_sha256: Option<Digest256>,
    mutation_claim_ids: &[String],
    event_id: &str,
    event_sequence: u64,
    creation_binding: &NavigationClaimCreationBindingV1,
    dependencies: &[NavigationDependencyV1],
    source_files: &BTreeMap<String, &[u8]>,
    limits: BibliographicLimits,
    claim_stream_rows_scanned: &mut u64,
) -> Result<NavigationClaimHistoryValidationV1> {
    if !current_stream.rows.contains_key(claim_id)
        || current_stream.rows.get(claim_id).map(|row| &row.claim) != Some(current_claim)
    {
        return Err(Error::Invalid(
            "navigation Claim history current member absent",
        ));
    }
    let Some(history_bytes) = history_bytes else {
        if current_stream
            .rows
            .values()
            .any(|row| row.claim.get("claim_version").and_then(Value::as_u64) != Some(1))
        {
            return Err(Error::Invalid("navigation Claim revision history absent"));
        }
        return Ok(NavigationClaimHistoryValidationV1 {
            revision_proof: None,
            history_sha256: None,
            latest_revision_receipt_sha256: None,
            current_source_ref_sha256: claim_reference_digest(&claim_reference(
                current_claim,
                limits.catalog.max_row_bytes,
            )?)?,
        });
    };
    let history = strict_value(history_bytes, limits.catalog.max_row_bytes)?;
    if history.get("schema_version").and_then(Value::as_str)
        != Some("tos_claim_revision_history_v1")
        || history.get("source_path").and_then(Value::as_str) != Some(source_path.as_str())
    {
        return Err(Error::Invalid("navigation Claim history owner binding"));
    }
    let receipts = history
        .get("receipts")
        .and_then(Value::as_array)
        .filter(|receipts| receipts.len() <= 128)
        .ok_or(Error::Invalid("navigation Claim history receipt bounds"))?;
    let mut latest = BTreeMap::<String, Value>::new();
    let mut commands = BTreeSet::new();
    let mut final_archive_binding = None;
    for (index, receipt) in receipts.iter().enumerate() {
        let previous = receipt
            .get("previous_source")
            .ok_or(Error::Invalid("navigation Claim history predecessor"))?;
        let successor = receipt
            .get("source")
            .ok_or(Error::Invalid("navigation Claim history successor"))?;
        let id = previous
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| id.starts_with("tos.claim."))
            .ok_or(Error::Invalid("navigation Claim history identity"))?
            .to_owned();
        let command = receipt
            .get("command_id")
            .and_then(Value::as_str)
            .filter(|command| !command.is_empty() && command.len() <= EVENT_ID_BYTES)
            .ok_or(Error::Invalid("navigation Claim history command"))?
            .to_owned();
        let previous_version = previous
            .get("version")
            .and_then(Value::as_u64)
            .filter(|version| *version > 0)
            .ok_or(Error::Invalid(
                "navigation Claim history predecessor version",
            ))?;
        let successor_version = successor
            .get("version")
            .and_then(Value::as_u64)
            .ok_or(Error::Invalid("navigation Claim history successor version"))?;
        let revision = receipt
            .get("previous_revision")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation Claim history revision"))?;
        let archive_path = receipt
            .get("archive_path")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation Claim history archive path"))?;
        let request = receipt
            .get("request")
            .ok_or(Error::Invalid("navigation Claim history request"))?;
        let expected_request_digest = format!(
            "sha256:{}",
            source_digest(request, limits.catalog.max_row_bytes)?
        );
        if !commands.insert(command)
            || previous.get("id") != successor.get("id")
            || previous_version.checked_add(1) != Some(successor_version)
            || request.get("operation").and_then(Value::as_str) != Some("claim.revise")
            || receipt.get("grants_admission") != Some(&Value::Bool(false))
            || receipt.get("request_digest").and_then(Value::as_str)
                != Some(expected_request_digest.as_str())
        {
            return Err(Error::Invalid("navigation Claim history transition shape"));
        }
        if let Some(prior) = latest.get(&id) {
            if prior != previous {
                return Err(Error::Invalid("navigation Claim history revision chain"));
            }
        } else if previous_version != 1 {
            return Err(Error::Invalid("navigation Claim history initial revision"));
        }
        let archive_binding = verify_claim_archive(
            archive_path,
            source_path,
            previous,
            revision,
            &receipts[..index],
            dependencies,
            source_files,
            limits,
            claim_stream_rows_scanned,
        )?;
        if index + 1 == receipts.len() {
            final_archive_binding = Some((archive_path.to_owned(), archive_binding));
        }
        latest.insert(id, successor.clone());
    }
    let target_ref = claim_reference(current_claim, limits.catalog.max_row_bytes)?;
    match latest.get(claim_id) {
        Some(expected) if expected == &target_ref => {}
        Some(_) => {
            return Err(Error::Invalid(
                "navigation Claim target history head differs",
            ));
        }
        None if current_claim.get("claim_version").and_then(Value::as_u64) == Some(1) => {}
        None => {
            return Err(Error::Invalid(
                "navigation Claim target revision lacks history",
            ));
        }
    }
    let history_member = source_files
        .get(history_path.as_str())
        .ok_or(Error::Invalid(
            "navigation Claim history dependency bytes absent",
        ))?;
    if Digest256::of_bytes(history_member) != Digest256::of_bytes(history_bytes) {
        return Err(Error::Invalid(
            "navigation Claim history exact bytes differ",
        ));
    }
    let history_sha256 = Digest256::of_bytes(history_bytes);
    if Some(history_path) == mutation_receipt_path
        && Some(history_sha256) != mutation_receipt_sha256
    {
        return Err(Error::Invalid(
            "navigation Claim revision receipt does not bind exact history",
        ));
    }
    let current_source_ref = claim_reference(current_claim, limits.catalog.max_row_bytes)?;
    let current_source_ref_sha256 = claim_reference_digest(&current_source_ref)?;
    let latest_revision_receipt_sha256 = receipts
        .last()
        .map(|receipt| source_digest(receipt, limits.catalog.max_row_bytes))
        .transpose()?;
    if Some(history_path) != mutation_receipt_path
        || Some(history_sha256) != mutation_receipt_sha256
        || current_claim.get("claim_version").and_then(Value::as_u64) == Some(1)
        || mutation_claim_ids.len() != 1
        || mutation_claim_ids[0] != claim_id
    {
        return Ok(NavigationClaimHistoryValidationV1 {
            revision_proof: None,
            history_sha256: Some(history_sha256),
            latest_revision_receipt_sha256,
            current_source_ref_sha256,
        });
    }
    let last = receipts
        .last()
        .ok_or(Error::Invalid("navigation Claim revision receipt absent"))?;
    let previous_source_ref = last
        .get("previous_source")
        .filter(|source| source.get("id").and_then(Value::as_str) == Some(claim_id))
        .cloned()
        .ok_or(Error::Invalid(
            "navigation Claim revision predecessor binding",
        ))?;
    let successor_source_ref = last
        .get("source")
        .filter(|source| source.get("id").and_then(Value::as_str) == Some(claim_id))
        .cloned()
        .ok_or(Error::Invalid(
            "navigation Claim revision successor binding",
        ))?;
    if claim_reference(current_claim, limits.catalog.max_row_bytes)? != successor_source_ref {
        return Err(Error::Invalid(
            "navigation Claim revision current source differs",
        ));
    }
    let (archive_path, archive_binding) = final_archive_binding.ok_or(Error::Invalid(
        "navigation Claim revision archive proof absent",
    ))?;
    let revision_receipt_sha256 = source_digest(last, limits.catalog.max_row_bytes)?;
    let previous_source_sha256 = claim_reference_digest(&previous_source_ref)?;
    let successor_source_sha256 = claim_reference_digest(&successor_source_ref)?;
    let mut proof = NavigationClaimRevisionProofV1 {
        event_id: event_id.to_owned(),
        event_sequence,
        claim_id: claim_id.to_owned(),
        claim_stream_path: source_path.clone(),
        claim_stream_sha256: current_stream.stream_sha256,
        history_path: history_path.clone(),
        history_sha256,
        revision_receipt_sha256,
        previous_source_sha256,
        successor_source_sha256,
        previous_source_ref,
        successor_source_ref,
        archive_path: checked_path(&archive_path)?,
        archive_manifest_sha256: archive_binding.manifest_sha256,
        archive_package_root: archive_binding.package_root,
        provenance_event_path: creation_binding.event_path.clone(),
        provenance_event_id: creation_binding.event_id.clone(),
        provenance_event_version: creation_binding.event_version,
        provenance_event_sha256: creation_binding.event_sha256,
        provenance_receipt_path: creation_binding.receipt_path.clone(),
        provenance_receipt_sha256: creation_binding.receipt_sha256,
        mutation_claim_ids: mutation_claim_ids.to_vec(),
        binding_root: Digest256::of_bytes(&[]),
    };
    proof.binding_root = revision_proof_binding_root(&proof)?;
    Ok(NavigationClaimHistoryValidationV1 {
        revision_proof: Some(proof),
        history_sha256: Some(history_sha256),
        latest_revision_receipt_sha256,
        current_source_ref_sha256,
    })
}

fn identity_forms_path(source_path: &str) -> Result<RelativePath> {
    let prefix = source_path
        .strip_suffix(".json")
        .ok_or(Error::Invalid("navigation Claim identity source extension"))?;
    checked_path(&format!("{prefix}.human-forms.json"))
}

fn validate_identity_sources(
    input: &mut NavigationClaimEventCommonInputV1<'_>,
    dependencies: &[NavigationDependencyV1],
    source_files: &BTreeMap<String, &[u8]>,
    revision: Digest256,
    limits: BibliographicLimits,
) -> Result<()> {
    let mut expected = BTreeMap::<String, &Value>::new();
    let mut expected_ids = BTreeSet::<String>::new();
    let mut add_expected = |node: &Value| -> Result<()> {
        if node.get("node_kind").and_then(Value::as_str) != Some("identity") {
            return Ok(());
        }
        let id = node
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| id.starts_with("identity:") && id.len() <= PATH_BYTES)
            .ok_or(Error::Invalid("navigation Claim identity node id"))?
            .to_owned();
        expected_ids.insert(id.clone());
        if expected.insert(id, node).is_some_and(|old| old != node) {
            return Err(Error::Invalid("navigation Claim conflicting identity node"));
        }
        Ok(())
    };
    add_expected(&input.claim_inputs.subject)?;
    add_expected(&input.claim_inputs.object)?;
    if let Some(node) = &input.claim_inputs.maker_identity {
        add_expected(node)?;
    }
    for node in &input.claim_inputs.members {
        add_expected(node)?;
    }
    for (_, node) in &input.claim_inputs.normalized {
        add_expected(node)?;
    }
    for node in input
        .claim_inputs
        .evidence
        .iter()
        .chain(&input.claim_inputs.counterevidence)
    {
        if node
            .pointer("/properties/evidence_kind")
            .and_then(Value::as_str)
            == Some("identity")
        {
            if node
                .pointer("/properties/resolved")
                .and_then(Value::as_bool)
                != Some(true)
            {
                return Err(Error::Invalid(
                    "navigation Claim identity evidence unresolved",
                ));
            }
            let id = node
                .pointer("/properties/identity_node_id")
                .and_then(Value::as_str)
                .filter(|id| id.starts_with("identity:") && id.len() <= PATH_BYTES)
                .ok_or(Error::Invalid("navigation Claim identity evidence target"))?
                .to_owned();
            expected_ids.insert(id);
        }
    }

    let mut checked = BTreeSet::new();
    for selected in input.identity_sources {
        let source_path = checked_path(selected.path)?;
        let schema_path = checked_path(selected.schema_path)?;
        let forms_path = checked_path(selected.forms_path)?;
        let source_bytes =
            source_files
                .get(source_path.as_str())
                .copied()
                .ok_or(Error::Invalid(
                    "navigation Claim identity source bytes absent",
                ))?;
        validate_required_dependency(
            dependencies,
            &source_path,
            NavigationDependencyRoleV1::Additional,
            source_bytes,
        )?;
        validate_required_dependency(
            dependencies,
            &schema_path,
            NavigationDependencyRoleV1::Additional,
            selected.schema_bytes,
        )?;
        input.validator.check_selected_event_document_exact(
            revision,
            source_path.as_str(),
            schema_path.as_str(),
            selected.schema_bytes,
            source_bytes,
        )?;
        let source = strict_value(source_bytes, limits.catalog.max_row_bytes)?;
        let entry = render_catalog_record(
            &source,
            source_path.as_str(),
            Some(schema_path.as_str()),
            limits.catalog.max_output_row_bytes,
        )?;
        let identity_ref = entry
            .get("record_id")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation Claim identity source reference"))?;
        let expected_form_path = identity_forms_path(source_path.as_str())?;
        if forms_path != expected_form_path {
            return Err(Error::Invalid("navigation Claim identity forms path"));
        }
        let forms_schema_path = selected.forms_schema_path.map(checked_path).transpose()?;
        let forms = match (
            selected.forms_bytes,
            selected.forms_schema_bytes,
            forms_schema_path.as_ref(),
        ) {
            (Some(bytes), Some(schema_bytes), Some(schema_path)) => {
                if schema_path.as_str() != "ToS/contracts/human-form-set.schema.json" {
                    return Err(Error::Invalid(
                        "navigation Claim identity forms schema path",
                    ));
                }
                validate_required_dependency(
                    dependencies,
                    &forms_path,
                    NavigationDependencyRoleV1::HumanForms,
                    bytes,
                )?;
                validate_required_dependency(
                    dependencies,
                    schema_path,
                    NavigationDependencyRoleV1::FormsSchema,
                    schema_bytes,
                )?;
                input.validator.check_selected_event_document_exact(
                    revision,
                    forms_path.as_str(),
                    schema_path.as_str(),
                    schema_bytes,
                    bytes,
                )?;
                materialize_checked_form_set(
                    bytes,
                    forms_path.as_str(),
                    &source,
                    input.forms_materializer,
                    limits,
                )?
            }
            (None, None, None) => {
                validate_optional_dependency(
                    dependencies,
                    &forms_path,
                    NavigationDependencyRoleV1::HumanForms,
                    None,
                )?;
                None
            }
            (None, None, Some(schema_path)) => {
                validate_optional_dependency(
                    dependencies,
                    &forms_path,
                    NavigationDependencyRoleV1::HumanForms,
                    None,
                )?;
                let _ = schema_path;
                None
            }
            _ => return Err(Error::Invalid("navigation Claim identity form state")),
        };
        let identity = supplied_bibliographic_identity(
            &entry,
            &source,
            forms.as_ref().map(|(path, value)| (path.as_str(), value)),
            BibliographicDocumentLimits {
                input_document_bytes: limits.catalog.max_row_bytes,
                output_row_bytes: limits.catalog.max_output_row_bytes,
            },
        )?;
        let id = identity
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation Claim identity projection id"))?;
        if identity_ref.len() > PATH_BYTES
            || !expected_ids.contains(id)
            || !checked.insert(id.to_owned())
        {
            return Err(Error::Invalid(
                "navigation Claim identity projection binding",
            ));
        }
        if let Some(candidate) = expected.get(id)
            && !same_canonical(&identity, candidate, limits.catalog.max_output_row_bytes)?
        {
            return Err(Error::Invalid(
                "navigation Claim identity projection binding",
            ));
        }
        for evidence_node in input
            .claim_inputs
            .evidence
            .iter()
            .chain(&input.claim_inputs.counterevidence)
            .filter(|node| {
                node.pointer("/properties/identity_node_id")
                    .and_then(Value::as_str)
                    == Some(id)
            })
        {
            let reference = evidence_node
                .pointer("/properties/evidence_ref")
                .and_then(Value::as_str)
                .ok_or(Error::Invalid("navigation Claim identity evidence ref"))?;
            let expected_evidence = supplied_bibliographic_evidence(
                reference,
                input.claim_inputs.claim,
                input.claim_inputs.entry,
                SuppliedEvidenceResolution::Identity(&identity),
                limits.catalog.max_output_row_bytes,
            )?;
            if !same_canonical(
                &expected_evidence,
                evidence_node,
                limits.catalog.max_output_row_bytes,
            )? {
                return Err(Error::Invalid("navigation Claim identity evidence binding"));
            }
        }
    }
    if checked.len() != expected_ids.len() {
        return Err(Error::Invalid("navigation Claim identity source closure"));
    }
    Ok(())
}

fn select_provenance_event_record(
    file_bytes: &[u8],
    receipt_path: &str,
    expected_event_id: &str,
    cap: usize,
) -> Result<(u64, Vec<u8>)> {
    if file_bytes.is_empty() || file_bytes.len() > TOTAL_DEPENDENCY_BYTES || receipt_path.is_empty()
    {
        return Err(Error::Budget("navigation Claim provenance event file"));
    }
    let lines = file_bytes.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    let mut selected = None;
    for (index, raw_line) in lines.iter().enumerate() {
        if raw_line.is_empty() && index + 1 == lines.len() {
            continue;
        }
        if raw_line.is_empty() {
            return Err(Error::Invalid("navigation Claim provenance blank line"));
        }
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.len() > cap {
            return Err(Error::Budget("navigation Claim provenance record bytes"));
        }
        let value = strict_value(line, cap)?;
        if value
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            == Some(receipt_path)
            && value.get("event_id").and_then(Value::as_str) == Some(expected_event_id)
        {
            if selected
                .replace((index as u64 + 1, line.to_vec()))
                .is_some()
            {
                return Err(Error::Invalid(
                    "navigation Claim provenance event is ambiguous",
                ));
            }
        }
    }
    selected.ok_or(Error::Invalid("navigation Claim provenance event absent"))
}

fn verify_provenance_file_binding(
    receipt: &Value,
    file_path: &RelativePath,
    bytes: &[u8],
) -> Result<()> {
    let name = file_path
        .as_str()
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("navigation Claim provenance file name"))?;
    let file = receipt
        .pointer(&format!("/files/{name}"))
        .ok_or(Error::Invalid("navigation Claim provenance receipt member"))?;
    let expected_digest = format!("sha256:{}", Digest256::of_bytes(bytes).to_hex());
    if file.get("sha256").and_then(Value::as_str) != Some(expected_digest.as_str())
        || file.get("bytes").and_then(Value::as_u64) != Some(bytes.len() as u64)
    {
        return Err(Error::Invalid(
            "navigation Claim provenance receipt file binding",
        ));
    }
    Ok(())
}

fn verify_claim_creation_output(
    event: &Value,
    receipt: &Value,
    source_path: &RelativePath,
) -> Result<()> {
    let source_name = source_path
        .as_str()
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("navigation Claim creation source name"))?;
    let receipt_file = receipt
        .pointer(&format!("/files/{source_name}"))
        .ok_or(Error::Invalid(
            "navigation Claim creation stream receipt absent",
        ))?;
    let outputs = event
        .pointer("/entities/outputs")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("navigation Claim creation outputs absent"))?;
    let matched = outputs
        .iter()
        .filter(|row| row.get("entity_ref").and_then(Value::as_str) == Some(source_path.as_str()))
        .collect::<Vec<_>>();
    if matched.len() != 1 {
        return Err(Error::Invalid(
            "navigation Claim creation stream output count",
        ));
    }
    let output = matched[0];
    let receipt_digest = receipt_file
        .get("sha256")
        .and_then(Value::as_str)
        .and_then(|value| value.strip_prefix("sha256:"))
        .ok_or(Error::Invalid(
            "navigation Claim creation stream receipt digest",
        ))?;
    if output.get("sha256").and_then(Value::as_str) != Some(receipt_digest)
        || output.get("size_bytes").and_then(Value::as_u64)
            != receipt_file.get("bytes").and_then(Value::as_u64)
    {
        return Err(Error::Invalid(
            "navigation Claim creation output receipt binding",
        ));
    }
    Ok(())
}

fn validate_provenance_event(
    input: &NavigationClaimEventCommonInputV1<'_>,
    claim: &Value,
    revision: Digest256,
    limits: BibliographicLimits,
) -> Result<NavigationClaimCreationBindingV1> {
    let source_path = checked_path(input.source_path)?;
    let expected_event_path = checked_path(&format!(
        "{}/source-create-provenance.jsonl",
        source_path
            .as_str()
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .ok_or(Error::Invalid("navigation Claim source owner directory"))?
    ))?;
    let expected_receipt_path = checked_path(&format!(
        "{}/source-create-receipt.json",
        source_path
            .as_str()
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .ok_or(Error::Invalid("navigation Claim source owner directory"))?
    ))?;
    let event_path = checked_path(input.provenance_event_path)?;
    let event_schema_path = checked_path(input.provenance_event_schema_path)?;
    let receipt_path = checked_path(input.provenance_receipt_path)?;
    if event_path != expected_event_path
        || receipt_path != expected_receipt_path
        || event_schema_path.as_str() != "ToS/contracts/provenance-event-v2.schema.json"
    {
        return Err(Error::Invalid("navigation Claim provenance owner paths"));
    }
    let event_id = claim
        .get("provenance_event_ref")
        .and_then(Value::as_str)
        .filter(|id| valid_event_id(id))
        .ok_or(Error::Invalid("navigation Claim provenance reference"))?;
    let (event_line, event_bytes) = select_provenance_event_record(
        input.provenance_event_bytes,
        receipt_path.as_str(),
        event_id,
        limits.catalog.max_row_bytes,
    )?;
    let event = strict_value(&event_bytes, limits.catalog.max_row_bytes)?;
    let receipt = strict_value(input.provenance_receipt_bytes, limits.catalog.max_row_bytes)?;
    let schema = strict_value(
        input.provenance_event_schema_bytes,
        limits.catalog.max_row_bytes,
    )?;
    if event.get("schema_version").and_then(Value::as_str) != Some("tos_provenance_event_v2")
        || event.get("event_id").and_then(Value::as_str) != Some(event_id)
        || event
            .get("event_version")
            .and_then(Value::as_u64)
            .is_none_or(|version| version == 0)
        || event
            .pointer("/record_binding/digest_algorithm")
            .and_then(Value::as_str)
            != Some("sha256")
        || event
            .pointer("/record_binding/digest_scope")
            .and_then(Value::as_str)
            != Some("exact_event_record_bytes")
        || event
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            != Some(receipt_path.as_str())
        || receipt.get("schema_version").and_then(Value::as_str)
            != Some("tos_local_claim_create_receipt_v1")
        || receipt.get("source_path").and_then(Value::as_str) != Some(source_path.as_str())
        || schema.get("$id").and_then(Value::as_str)
            != Some("https://treeofsophia.local/ToS/contracts/provenance-event-v2.schema.json")
    {
        return Err(Error::Invalid(
            "navigation Claim provenance identity binding",
        ));
    }
    let receipt_claims = receipt
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid(
            "navigation Claim provenance receipt claim set",
        ))?;
    let claim_id = claim
        .get("claim_id")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("navigation Claim provenance Claim id"))?;
    if !receipt_claims
        .iter()
        .any(|row| row.get("id").and_then(Value::as_str) == Some(claim_id))
    {
        return Err(Error::Invalid(
            "navigation Claim absent from creation receipt",
        ));
    }
    input.validator.check_selected_event_document_exact(
        revision,
        event_path.as_str(),
        event_schema_path.as_str(),
        input.provenance_event_schema_bytes,
        &event_bytes,
    )?;
    verify_provenance_file_binding(&receipt, &event_path, input.provenance_event_bytes)?;
    verify_claim_creation_output(&event, &receipt, &source_path)?;

    let event_location = json!({
        "source_ref": event_path.as_str(),
        "source_line": event_line,
        "canonical_sha256": source_digest(&event, limits.catalog.max_output_row_bytes)?,
    });
    let projected =
        supplied_bibliographic_event(&event, &event_location, limits.catalog.max_output_row_bytes)?;
    if !same_canonical(
        &projected,
        &input.claim_inputs.event,
        limits.catalog.max_output_row_bytes,
    )? {
        return Err(Error::Invalid(
            "navigation Claim provenance projection binding",
        ));
    }
    Ok(NavigationClaimCreationBindingV1 {
        event_path,
        event_id: event_id.to_owned(),
        event_version: event
            .get("event_version")
            .and_then(Value::as_u64)
            .ok_or(Error::Invalid("navigation Claim provenance event version"))?,
        event_sha256: Digest256::of_bytes(&event_bytes),
        receipt_path,
        receipt_sha256: Digest256::of_bytes(input.provenance_receipt_bytes),
    })
}

fn provenance_identity(
    event_file_bytes: &[u8],
    receipt_path: &str,
    limits: BibliographicLimits,
) -> Result<(String, u64)> {
    let receipt_ref = checked_path(receipt_path)?;
    let lines = event_file_bytes
        .split(|byte| *byte == b'\n')
        .collect::<Vec<_>>();
    let mut selected: Option<Value> = None;
    for (index, raw_line) in lines.iter().enumerate() {
        if raw_line.is_empty() && index + 1 == lines.len() {
            continue;
        }
        if raw_line.is_empty() {
            return Err(Error::Invalid("navigation Claim provenance blank line"));
        }
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.len() > limits.catalog.max_row_bytes {
            return Err(Error::Budget("navigation Claim provenance record bytes"));
        }
        let value = strict_value(line, limits.catalog.max_row_bytes)?;
        if value
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            == Some(receipt_ref.as_str())
        {
            if selected.replace(value).is_some() {
                return Err(Error::Invalid(
                    "navigation Claim provenance receipt ambiguous",
                ));
            }
        }
    }
    let event = selected.ok_or(Error::Invalid(
        "navigation Claim provenance identity absent",
    ))?;
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .filter(|id| valid_event_id(id))
        .ok_or(Error::Invalid("navigation Claim provenance event id"))?
        .to_owned();
    let version = event
        .get("event_version")
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or(Error::Invalid("navigation Claim provenance event version"))?;
    Ok((event_id, version))
}

fn dependency_role(role: NavigationDependencyRoleV1) -> &'static str {
    match role {
        NavigationDependencyRoleV1::Event => "event",
        NavigationDependencyRoleV1::EventSchema => "event-schema",
        NavigationDependencyRoleV1::EventReceipt => "event-receipt",
        NavigationDependencyRoleV1::Record => "record",
        NavigationDependencyRoleV1::EntityRegistry => "entity-registry",
        NavigationDependencyRoleV1::EntityRegistrySchema => "entity-registry-schema",
        NavigationDependencyRoleV1::RecordSchema => "record-schema",
        NavigationDependencyRoleV1::HumanForms => "human-forms",
        NavigationDependencyRoleV1::FormsSchema => "forms-schema",
        NavigationDependencyRoleV1::RecordHistory => "record-history",
        NavigationDependencyRoleV1::VersionSource => "version-source",
        NavigationDependencyRoleV1::VersionSchema => "version-schema",
        NavigationDependencyRoleV1::Additional => "additional",
    }
}

fn dependency_root(dependencies: &[NavigationDependencyV1]) -> Result<Digest256> {
    let rows = dependencies
        .iter()
        .map(|dependency| {
            let observation = match &dependency.state {
                NavigationDependencyStateV1::Present { sha256, size_bytes } => {
                    json!({"state":"present","sha256":sha256.to_hex(),"size_bytes":size_bytes})
                }
                NavigationDependencyStateV1::Absent { reason } => {
                    json!({"state":"absent","reason":reason})
                }
            };
            json!({
                "path":dependency.path.as_str(),
                "role":dependency_role(dependency.role),
                "observation":observation
            })
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({"schema":"tos_source_navigation_claim_dependencies_v1","dependencies":rows}),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn output_rows(
    projection: &NavigationClaimEventProjectionV1,
    node_visibility: &[bool],
    edge_visibility: &[bool],
) -> Result<(Vec<NavigationClaimEventRowV1>, Vec<String>)> {
    if node_visibility.len() != projection.nodes.len()
        || edge_visibility.len() != projection.edges.len()
    {
        return Err(Error::Invalid("navigation Claim visibility row count"));
    }
    let mut rows = BTreeMap::<(String, String), NavigationClaimEventRowV1>::new();
    let mut node_ids = BTreeSet::new();
    for (value, visible) in projection.nodes.iter().zip(node_visibility) {
        let id = value
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= PATH_BYTES)
            .ok_or(Error::Invalid("navigation Claim output node id"))?
            .to_owned();
        if !node_ids.insert(id.clone()) {
            return Err(Error::Invalid("navigation Claim duplicate output node id"));
        }
        let row = NavigationClaimEventRowV1 {
            collection: "nodes".to_owned(),
            id: id.clone(),
            sha256: digest_json(value, CLAIM_LINE_BYTES)?,
            visible: *visible,
        };
        if rows.insert(("nodes".to_owned(), id), row).is_some() {
            return Err(Error::Invalid("navigation Claim duplicate output node"));
        }
    }
    let mut external = BTreeSet::new();
    for (value, visible) in projection.edges.iter().zip(edge_visibility) {
        let id = value
            .get("edge_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= PATH_BYTES)
            .ok_or(Error::Invalid("navigation Claim output edge id"))?
            .to_owned();
        for endpoint in ["from_id", "to_id"] {
            let endpoint_id = value
                .get(endpoint)
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && id.len() <= PATH_BYTES)
                .ok_or(Error::Invalid("navigation Claim output edge endpoint"))?;
            if !node_ids.contains(endpoint_id) {
                if endpoint == "from_id" || !endpoint_id.starts_with("claim:") {
                    return Err(Error::Invalid("navigation Claim output endpoint absent"));
                }
                external.insert(endpoint_id.to_owned());
            }
        }
        let row = NavigationClaimEventRowV1 {
            collection: "edges".to_owned(),
            id: id.clone(),
            sha256: digest_json(value, CLAIM_LINE_BYTES)?,
            visible: *visible,
        };
        if rows.insert(("edges".to_owned(), id), row).is_some() {
            return Err(Error::Invalid("navigation Claim duplicate output edge"));
        }
    }
    Ok((rows.into_values().collect(), external.into_iter().collect()))
}

fn projection_visibility(
    projection: &NavigationClaimEventProjectionV1,
    inventory: &NavigationClaimEventOwnerInventoryV1,
) -> Result<(Vec<bool>, Vec<bool>, Vec<String>)> {
    let mut node_visibility = Vec::with_capacity(projection.nodes.len());
    let mut by_node_id = BTreeMap::<String, bool>::new();
    for row in &projection.nodes {
        let id = row
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(Error::Invalid("navigation Claim node visibility id"))?;
        let visible = visible_value(row)?;
        if by_node_id.insert(id.to_owned(), visible).is_some() {
            return Err(Error::Invalid("navigation Claim duplicate visibility node"));
        }
        node_visibility.push(visible);
    }
    let mut edge_visibility = Vec::with_capacity(projection.edges.len());
    let mut external = BTreeSet::new();
    for edge in &projection.edges {
        let from_id = edge
            .get("from_id")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation Claim edge visibility source"))?;
        let to_id = edge
            .get("to_id")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("navigation Claim edge visibility target"))?;
        let from_visible = by_node_id
            .get(from_id)
            .copied()
            .ok_or(Error::Invalid("navigation Claim edge source node absent"))?;
        let to_visible = if let Some(visible) = by_node_id.get(to_id) {
            *visible
        } else {
            let claim_id = to_id
                .strip_prefix("claim:")
                .filter(|id| !id.is_empty())
                .ok_or(Error::Invalid("navigation Claim external target kind"))?;
            let target = inventory
                .claim_by_id(claim_id)
                .ok_or(Error::Invalid("navigation Claim external target absent"))?;
            external.insert(to_id.to_owned());
            visible_claim(target.claim)?
        };
        edge_visibility.push(from_visible && to_visible);
    }
    Ok((
        node_visibility,
        edge_visibility,
        external.into_iter().collect(),
    ))
}

fn visible_claim(claim: &Value) -> Result<bool> {
    visible_value(&json!({"properties":claim}))
}

fn visible_value(value: &Value) -> Result<bool> {
    let raw = serde_json::to_vec(value)
        .map_err(|_| Error::Invalid("navigation Claim visibility JSON"))?;
    let limits = JsonLimits::new(raw.len().max(1), 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation Claim visibility limits"))?;
    let parsed =
        tos_foundation::parse_json(&raw, tos_foundation::JsonMode::PublishedStrict, limits)
            .map_err(|_| Error::Invalid("navigation Claim visibility JSON"))?;
    Ok(crate::visible_node(&parsed.into_root()))
}

fn output_root(
    rows: &[NavigationClaimEventRowV1],
    diagnostics: &[Value],
    trace_sha256: Digest256,
    external_endpoint_ids: &[String],
) -> Result<Digest256> {
    let diagnostics = diagnostics
        .iter()
        .map(|row| digest_json(row, CLAIM_LINE_BYTES).map(Digest256::to_hex))
        .collect::<Result<Vec<_>>>()?;
    let manifest = rows
        .iter()
        .map(|row| json!([row.collection, row.id, row.sha256.to_hex(), row.visible]))
        .collect::<Vec<_>>();
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_cohort_v1",
            "rows":manifest,
            "diagnostics":diagnostics,
            "trace_sha256":trace_sha256.to_hex(),
            "external_endpoint_ids":external_endpoint_ids
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn claim_set_root(
    event_id: &str,
    event_sequence: u64,
    mutation_receipt_path: &RelativePath,
    receipt_sha256: Digest256,
    affected_owner_manifest_root: Digest256,
) -> Result<Digest256> {
    if !valid_event_id(event_id) || event_sequence == 0 {
        return Err(Error::Invalid("navigation Claim event set identity"));
    }
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_event_members_v4",
            "event_id":event_id,
            "event_sequence":event_sequence,
            "mutation_receipt_path":mutation_receipt_path.as_str(),
            "mutation_receipt_sha256":receipt_sha256.to_hex(),
            "affected_owner_manifest_root":affected_owner_manifest_root.to_hex()
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn revision_proof_binding_root(proof: &NavigationClaimRevisionProofV1) -> Result<Digest256> {
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_revision_proof_v1",
            "event_id":proof.event_id,
            "event_sequence":proof.event_sequence,
            "claim_id":proof.claim_id,
            "claim_stream_path":proof.claim_stream_path.as_str(),
            "claim_stream_sha256":proof.claim_stream_sha256.to_hex(),
            "history_path":proof.history_path.as_str(),
            "history_sha256":proof.history_sha256.to_hex(),
            "revision_receipt_sha256":proof.revision_receipt_sha256.to_hex(),
            "previous_source_sha256":proof.previous_source_sha256.to_hex(),
            "successor_source_sha256":proof.successor_source_sha256.to_hex(),
            "previous_source_ref":proof.previous_source_ref,
            "successor_source_ref":proof.successor_source_ref,
            "archive_path":proof.archive_path.as_str(),
            "archive_manifest_sha256":proof.archive_manifest_sha256.to_hex(),
            "archive_package_root":proof.archive_package_root.to_hex(),
            "provenance_event_path":proof.provenance_event_path.as_str(),
            "provenance_event_id":proof.provenance_event_id,
            "provenance_event_version":proof.provenance_event_version,
            "provenance_event_sha256":proof.provenance_event_sha256.to_hex(),
            "provenance_receipt_path":proof.provenance_receipt_path.as_str(),
            "provenance_receipt_sha256":proof.provenance_receipt_sha256.to_hex(),
            "mutation_claim_ids":proof.mutation_claim_ids,
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn dependency_refresh_proof_root(
    proof: &NavigationClaimDependencyRefreshProofV1,
) -> Result<Digest256> {
    let mut reverse_union = BTreeSet::new();
    for cause in &proof.changed_dependencies {
        checked_path(cause.changed_dependency_path.as_str())?;
        if is_claim_stream_path(cause.changed_dependency_path.as_str())
            || !sorted_unique(&cause.reverse_claim_ids)
            || cause.reverse_claim_ids.len() > MAX_CLAIM_EVENT_TOTAL_OWNERS
            || cause
                .reverse_claim_ids
                .iter()
                .any(|id| !id.starts_with("tos.claim.") || id.len() > PATH_BYTES)
        {
            return Err(Error::Invalid(
                "navigation Claim dependency-refresh proof cause shape",
            ));
        }
        reverse_union.extend(cause.reverse_claim_ids.iter().map(String::as_str));
    }
    let reverse_union = reverse_union
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !valid_event_id(&proof.event_id)
        || proof.event_sequence == 0
        || proof.changed_dependencies.is_empty()
        || proof.changed_dependencies.len() > MAX_CLAIM_REFRESH_CAUSES
        || proof
            .changed_dependencies
            .windows(2)
            .any(|pair| pair[0].changed_dependency_path >= pair[1].changed_dependency_path)
        || !is_claim_stream_path(proof.claim_stream_path.as_str())
        || !proof.claim_id.starts_with("tos.claim.")
        || proof.claim_id.len() > PATH_BYTES
        || proof.claim_version == 0
        || proof.claim_source_line == 0
        || !sorted_unique(&proof.reverse_claim_ids)
        || proof.reverse_claim_ids.is_empty()
        || reverse_union != proof.reverse_claim_ids
        || !valid_event_id(&proof.provenance_event_id)
        || proof.provenance_event_version == 0
        || proof.history_path.is_some() != proof.history_sha256.is_some()
        || proof.claim_version > 1
            && (proof.history_path.is_none() || proof.latest_revision_receipt_sha256.is_none())
        || proof.claim_version == 1 && proof.latest_revision_receipt_sha256.is_some()
    {
        return Err(Error::Invalid(
            "navigation Claim dependency-refresh proof shape",
        ));
    }
    let changed_dependency_rows = proof
        .changed_dependencies
        .iter()
        .map(|cause| {
            json!({
                "changed_dependency_path":cause.changed_dependency_path.as_str(),
                "changed_dependency_sha256":cause.changed_dependency_sha256.to_hex(),
                "reverse_claim_ids":cause.reverse_claim_ids
            })
        })
        .collect::<Vec<_>>();
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_dependency_refresh_proof_v1",
            "event_id":proof.event_id,
            "event_sequence":proof.event_sequence,
            "changed_dependencies":changed_dependency_rows,
            "closure_root":proof.closure_root.to_hex(),
            "affected_owner_manifest_root":proof.affected_owner_manifest_root.to_hex(),
            "reverse_claim_ids":proof.reverse_claim_ids,
            "claim_stream_path":proof.claim_stream_path.as_str(),
            "claim_stream_sha256":proof.claim_stream_sha256.to_hex(),
            "claim_id":proof.claim_id,
            "claim_version":proof.claim_version,
            "claim_source_line":proof.claim_source_line,
            "claim_row_sha256":proof.claim_row_sha256.to_hex(),
            "current_source_ref_sha256":proof.current_source_ref_sha256.to_hex(),
            "provenance_event_path":proof.provenance_event_path.as_str(),
            "provenance_event_id":proof.provenance_event_id,
            "provenance_event_version":proof.provenance_event_version,
            "provenance_event_sha256":proof.provenance_event_sha256.to_hex(),
            "provenance_receipt_path":proof.provenance_receipt_path.as_str(),
            "provenance_receipt_sha256":proof.provenance_receipt_sha256.to_hex(),
            "history_path":proof.history_path.as_ref().map(RelativePath::as_str),
            "history_sha256":proof.history_sha256.map(Digest256::to_hex),
            "latest_revision_receipt_sha256":proof.latest_revision_receipt_sha256.map(Digest256::to_hex)
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn dependency_refresh_input_root(
    event_id: &str,
    event_sequence: u64,
    source_event_id: &str,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    predecessor_cohort_root: Option<Digest256>,
    source_path: &RelativePath,
    source_line: u64,
    record_id: &str,
    dependency_root: Digest256,
    output_root: Digest256,
    event_basis_root: Digest256,
    owner_inventory_root: Digest256,
    claim_stream_rows_scanned: u64,
) -> Result<Digest256> {
    if !valid_event_id(event_id)
        || event_sequence == 0
        || !valid_event_id(source_event_id)
        || source_event_version == 0
        || predecessor_source_revision == source_revision
        || !is_claim_stream_path(source_path.as_str())
        || source_line == 0
        || !record_id.starts_with("tos.claim.")
        || record_id.len() > PATH_BYTES
    {
        return Err(Error::Invalid(
            "navigation Claim dependency-refresh input root shape",
        ));
    }
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_dependency_refresh_input_v1",
            "event_id":event_id,
            "event_sequence":event_sequence,
            "source_event_id":source_event_id,
            "source_event_version":source_event_version,
            "predecessor_source_revision":predecessor_source_revision.to_hex(),
            "source_revision":source_revision.to_hex(),
            "predecessor_cohort_root":predecessor_cohort_root.map(Digest256::to_hex),
            "source_path":source_path.as_str(),
            "source_line":source_line,
            "record_id":record_id,
            "dependency_root":dependency_root.to_hex(),
            "output_root":output_root.to_hex(),
            "event_basis_root":event_basis_root.to_hex(),
            "owner_inventory_root":owner_inventory_root.to_hex(),
            "claim_stream_rows_scanned":claim_stream_rows_scanned
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn claim_reference_digest(reference: &Value) -> Result<Digest256> {
    reference
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|value| value.strip_prefix("sha256:"))
        .and_then(|hex| Digest256::from_hex(hex).ok())
        .ok_or(Error::Invalid("navigation Claim reference digest"))
}

#[allow(clippy::too_many_arguments)]
fn input_root(
    event_id: &str,
    event_sequence: u64,
    source_event_id: &str,
    source_event_version: u64,
    predecessor_source_revision: Digest256,
    source_revision: Digest256,
    predecessor_cohort_root: Option<Digest256>,
    source_path: &RelativePath,
    source_line: u64,
    record_id: &str,
    mutation_receipt_path: &RelativePath,
    mutation_source_path: &RelativePath,
    dependency_root: Digest256,
    output_root: Digest256,
    event_claim_set_root: Digest256,
    owner_inventory_root: Digest256,
    revision_proof_root: Option<Digest256>,
    claim_stream_rows_scanned: u64,
) -> Result<Digest256> {
    digest_json(
        &json!({
            "schema":"tos_source_navigation_claim_event_input_v4",
            "event_id":event_id,
            "event_sequence":event_sequence,
            "source_event_id":source_event_id,
            "source_event_version":source_event_version,
            "predecessor_source_revision":predecessor_source_revision.to_hex(),
            "source_revision":source_revision.to_hex(),
            "predecessor_cohort_root":predecessor_cohort_root.map(Digest256::to_hex),
            "source_path":source_path.as_str(),
            "source_line":source_line,
            "record_id":record_id,
            "mutation_receipt_path":mutation_receipt_path.as_str(),
            "mutation_source_path":mutation_source_path.as_str(),
            "dependency_root":dependency_root.to_hex(),
            "output_root":output_root.to_hex(),
            "event_claim_set_root":event_claim_set_root.to_hex(),
            "owner_inventory_root":owner_inventory_root.to_hex(),
            "revision_proof_root":revision_proof_root.map(Digest256::to_hex),
            "claim_stream_rows_scanned":claim_stream_rows_scanned
        }),
        TOTAL_DEPENDENCY_BYTES,
    )
}

fn navigation_claim_event_budget_cost(
    projection: &NavigationClaimEventProjectionV1,
    dependencies: &[NavigationDependencyV1],
    source_path: &RelativePath,
    claim_stream_sha256: Digest256,
    claim_stream_rows_scanned: u64,
) -> Result<NavigationClaimEventBudgetCostV1> {
    let output_rows = projection
        .nodes
        .len()
        .checked_add(projection.edges.len())
        .and_then(|count| count.checked_add(projection.diagnostics.len()))
        .ok_or(Error::Budget("navigation Claim aggregate row count"))?;
    let rows = output_rows
        .checked_add(dependencies.len())
        .ok_or(Error::Budget("navigation Claim aggregate row count"))?;
    let mut dependency_bytes = 0u64;
    let mut encoded_bytes = 0u64;
    let mut claim_stream_size = None;
    for dependency in dependencies {
        encoded_bytes = encoded_bytes
            .checked_add(dependency.path.as_str().len() as u64)
            .ok_or(Error::Budget("navigation Claim aggregate encoded bytes"))?;
        if let NavigationDependencyStateV1::Present { sha256, size_bytes } = &dependency.state {
            dependency_bytes = dependency_bytes
                .checked_add(*size_bytes)
                .ok_or(Error::Budget("navigation Claim aggregate dependency bytes"))?;
            if dependency.path == *source_path
                && dependency.role == NavigationDependencyRoleV1::Record
            {
                if *sha256 != claim_stream_sha256
                    || claim_stream_size.replace(*size_bytes).is_some()
                {
                    return Err(Error::Invalid(
                        "navigation Claim aggregate stream dependency binding",
                    ));
                }
            }
        }
    }
    if claim_stream_size.is_none() {
        return Err(Error::Invalid(
            "navigation Claim aggregate stream dependency absent",
        ));
    }
    for row in projection
        .nodes
        .iter()
        .chain(&projection.edges)
        .chain(&projection.diagnostics)
    {
        let row_bytes = canonical(row, TOTAL_DEPENDENCY_BYTES)?.len() as u64;
        encoded_bytes = encoded_bytes
            .checked_add(row_bytes)
            .ok_or(Error::Budget("navigation Claim aggregate encoded bytes"))?;
    }
    Ok(NavigationClaimEventBudgetCostV1 {
        rows: u64::try_from(rows)
            .map_err(|_| Error::Budget("navigation Claim aggregate row count"))?,
        encoded_bytes,
        dependency_bytes,
        claim_stream_rows_scanned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_stream_owner_inventory_must_cover_every_exact_current_row() {
        let source = br#"{"claim_id":"tos.claim.a","claim_version":1}
{"claim_id":"tos.claim.b","claim_version":1}
"#;
        let complete = vec!["tos.claim.a".to_owned(), "tos.claim.b".to_owned()];
        let mut scanned = 0;
        let (rows, digest) =
            claim_stream_inventory(source, &complete, CLAIM_LINE_BYTES, &mut scanned)
                .expect("exact current stream inventory");
        assert_eq!(rows.len(), 2);
        assert_eq!(digest, Digest256::of_bytes(source));
        assert_eq!(scanned, 2);

        let omitted = vec!["tos.claim.a".to_owned()];
        assert!(claim_stream_inventory(source, &omitted, CLAIM_LINE_BYTES, &mut scanned).is_err());

        let stale = vec!["tos.claim.a".to_owned(), "tos.claim.c".to_owned()];
        assert!(claim_stream_inventory(source, &stale, CLAIM_LINE_BYTES, &mut scanned).is_err());
    }

    #[test]
    fn affected_owner_closure_replaces_complete_streams_and_keeps_mutation_stream() {
        let mutation_path = "ToS/source-witnesses/relations/a/source-claims.jsonl";
        let reverse_path = "ToS/source-witnesses/relations/b/source-claims.jsonl";
        let mutation_bytes = br#"{"claim_id":"tos.claim.changed","claim_version":2}"#;
        let reverse_bytes = br#"{"claim_id":"tos.claim.referrer-a","claim_version":1}
{"claim_id":"tos.claim.referrer-b","claim_version":1}
"#;
        let inventory = prepare_navigation_claim_event_owner_streams_v1(
            &[
                NavigationClaimEventOwnerStreamInputV1 {
                    source_path: mutation_path,
                    source_bytes: mutation_bytes,
                },
                NavigationClaimEventOwnerStreamInputV1 {
                    source_path: reverse_path,
                    source_bytes: reverse_bytes,
                },
            ],
            CLAIM_LINE_BYTES,
        )
        .expect("complete selected stream inventory");
        let manifest = inventory
            .affected_owner_streams(
                &["tos.claim.referrer-a".to_owned()],
                mutation_path,
                &["tos.claim.changed".to_owned()],
            )
            .expect("stream-complete affected owner manifest");
        assert_eq!(manifest.streams().len(), 2);
        assert_eq!(
            manifest.streams()[0].owner_ids(),
            &["tos.claim.changed".to_owned()]
        );
        assert_eq!(
            manifest.streams()[1].owner_ids(),
            &[
                "tos.claim.referrer-a".to_owned(),
                "tos.claim.referrer-b".to_owned()
            ]
        );
        assert_eq!(
            manifest.reverse_claim_ids(),
            &["tos.claim.referrer-a".to_owned()]
        );
        assert_eq!(manifest.mutation_source_path(), mutation_path);
        assert_eq!(manifest.inventory_root(), inventory.root());
        manifest
            .verify_integrity()
            .expect("sealed manifest integrity");
        assert!(
            inventory
                .affected_owner_streams(
                    &["tos.claim.missing".to_owned()],
                    mutation_path,
                    &["tos.claim.changed".to_owned()],
                )
                .is_err()
        );
    }
}
