//! Bounded terminal Record replacements for a contiguous native event range.
//!
//! The range retains real event identities and owner-specific causes. Rows
//! reuse the checked per-record renderer, but the sealed envelope binds the
//! whole range, predecessor owner set, terminal owner set, and retractions.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, RelativePath, canonical_bytes_v1};

use crate::{
    Error, Result,
    source_bibliographic_render::digest as record_digest,
    source_navigation_claim_event::NavigationClaimEventRangeManifestV1,
    source_navigation_event::{
        NavigationDependencyStateV1, NavigationEventRangeCoverageV1, NavigationRecordEventBasisV1,
        NavigationRecordEventCohortV1,
    },
};

const MAX_RANGE_OWNERS: usize = 16_384;
const MAX_RANGE_CAUSES: usize = 65_536;
const MAX_RANGE_BYTES: usize = 128 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_ID_BYTES: usize = 256;

/// Stable Record key shared by exact predecessor and terminal owner lists.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct NavigationRecordEventRangeOwnerKeyV1 {
    pub source_path: String,
    pub record_id: String,
}

/// Exact persisted predecessor row used for replacement or retraction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRecordEventRangePriorOwnerInputV1 {
    /// Exact source member whose persisted dependency or direct-owner row
    /// made this predecessor cohort eligible for the current range.
    pub cause_path: String,
    pub source_path: String,
    pub record_id: String,
    pub predecessor_cohort_root: Digest256,
}

/// Receipt-backed direct create or history-backed Record revision in one
/// actual event covered by the contiguous range.
pub struct NavigationRecordEventRangeMutationCauseInputV1<'a> {
    pub event_sequence: u64,
    pub event_id: &'a str,
    pub command_id: &'a str,
    pub kind: NavigationRecordEventRangeMutationKindInputV1<'a>,
}

pub enum NavigationRecordEventRangeMutationKindInputV1<'a> {
    DirectCreate {
        source_path: &'a str,
        source_bytes: &'a [u8],
        receipt_path: &'a str,
        receipt_bytes: &'a [u8],
        provenance_event_path: &'a str,
        provenance_event_bytes: &'a [u8],
        origin_forms_path: Option<&'a str>,
        origin_forms_bytes: Option<&'a [u8]>,
    },
    Revision {
        source_path: &'a str,
        source_bytes: &'a [u8],
        history_path: &'a str,
        history_bytes: &'a [u8],
    },
}

/// One exact dependency transition and the predecessor/terminal Record
/// reverse-owner sets returned by the native projection index for that path.
/// Repeated changes to one path carry the same complete terminal owner set.
pub struct NavigationRecordEventRangeDependencyCauseInputV1<'a> {
    pub event_sequence: u64,
    pub event_id: &'a str,
    pub command_id: Option<&'a str>,
    pub changed_dependency_path: &'a str,
    pub changed_dependency_bytes: &'a [u8],
    pub predecessor_owners: &'a [NavigationRecordEventRangePriorOwnerInputV1],
    pub current_owners: &'a [NavigationRecordEventRangeOwnerKeyV1],
}

/// Link one validated Claim revision proof to its prior and current Record
/// owners. The Claim range manifest supplies its exact revision receipt and
/// command identity; this input supplies only the reverse-owner result.
pub struct NavigationRecordEventRangeClaimCauseInputV1<'a> {
    pub event_sequence: u64,
    pub event_id: &'a str,
    pub claim_id: &'a str,
    pub predecessor_owners: &'a [NavigationRecordEventRangePriorOwnerInputV1],
    pub current_owners: &'a [NavigationRecordEventRangeOwnerKeyV1],
}

/// One terminal Record owner key and every direct Record mutation in the
/// range. The callback builds its exact terminal semantic input after the
/// compiler has sealed the owner-specific cause root.
pub struct NavigationRecordEventRangeOwnerInputV1<'a> {
    pub key: NavigationRecordEventRangeOwnerKeyV1,
    /// Exact current history bytes, or `None` only for a witnessed absence.
    /// The projected row must carry the same present/absent dependency state.
    pub terminal_history_bytes: Option<&'a [u8]>,
    pub mutations: Vec<NavigationRecordEventRangeMutationCauseInputV1<'a>>,
}

/// Complete selected inputs for a bounded Record range replacement.
pub struct NavigationRecordEventRangeInputV1<'a> {
    pub coverage: &'a NavigationEventRangeCoverageV1,
    pub owners: Vec<NavigationRecordEventRangeOwnerInputV1<'a>>,
    pub prior_owners: &'a [NavigationRecordEventRangePriorOwnerInputV1],
    pub dependency_causes: &'a [NavigationRecordEventRangeDependencyCauseInputV1<'a>],
    pub claim_manifest: Option<&'a NavigationClaimEventRangeManifestV1>,
    pub claim_causes: &'a [NavigationRecordEventRangeClaimCauseInputV1<'a>],
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NavigationRecordEventRangeCauseKindV1 {
    DirectCreate,
    RecordRevision,
    Dependency,
    ClaimRevision,
}

impl NavigationRecordEventRangeCauseKindV1 {
    fn as_str(&self) -> &'static str {
        match self {
            Self::DirectCreate => "direct-create",
            Self::RecordRevision => "record-revision",
            Self::Dependency => "dependency",
            Self::ClaimRevision => "claim-revision",
        }
    }
}

/// Owner-local reference to an exact event cause. The matching global cause
/// row carries its complete predecessor/current owner closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRecordEventRangeCauseRefV1 {
    event_sequence: u64,
    event_id: String,
    command_id: Option<String>,
    kind: NavigationRecordEventRangeCauseKindV1,
    path: RelativePath,
    sha256: Digest256,
    mutation_identity: Option<NavigationRecordEventRangeMutationIdentityV1>,
    cause_root: Digest256,
}

impl NavigationRecordEventRangeCauseRefV1 {
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn command_id(&self) -> Option<&str> {
        self.command_id.as_deref()
    }

    pub fn kind(&self) -> &NavigationRecordEventRangeCauseKindV1 {
        &self.kind
    }

    pub fn path(&self) -> &RelativePath {
        &self.path
    }

    pub fn sha256(&self) -> Digest256 {
        self.sha256
    }

    /// Exact source/record identity paths for a direct create or revision.
    /// Dependency and Claim causes have no Record mutation identity.
    pub fn mutation_identity(&self) -> Option<&NavigationRecordEventRangeMutationIdentityV1> {
        self.mutation_identity.as_ref()
    }

    pub fn cause_root(&self) -> Digest256 {
        self.cause_root
    }
}

/// Typed path-to-ID bindings retained for a direct Record mutation. Create
/// causes bind their receipt, provenance event, and optional original forms;
/// revision causes bind the exact history snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRecordEventRangeMutationIdentityV1 {
    record_id: String,
    source_path: RelativePath,
    source_sha256: Digest256,
    history_path: Option<RelativePath>,
    history_sha256: Option<Digest256>,
    receipt_path: Option<RelativePath>,
    receipt_sha256: Option<Digest256>,
    provenance_event_path: Option<RelativePath>,
    provenance_event_sha256: Option<Digest256>,
    origin_forms_path: Option<RelativePath>,
    origin_forms_sha256: Option<Digest256>,
}

impl NavigationRecordEventRangeMutationIdentityV1 {
    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn source_path(&self) -> &RelativePath {
        &self.source_path
    }

    pub fn source_sha256(&self) -> Digest256 {
        self.source_sha256
    }

    pub fn history_path(&self) -> Option<&RelativePath> {
        self.history_path.as_ref()
    }

    pub fn history_sha256(&self) -> Option<Digest256> {
        self.history_sha256
    }

    pub fn receipt_path(&self) -> Option<&RelativePath> {
        self.receipt_path.as_ref()
    }

    pub fn receipt_sha256(&self) -> Option<Digest256> {
        self.receipt_sha256
    }

    pub fn provenance_event_path(&self) -> Option<&RelativePath> {
        self.provenance_event_path.as_ref()
    }

    pub fn provenance_event_sha256(&self) -> Option<Digest256> {
        self.provenance_event_sha256
    }

    pub fn origin_forms_path(&self) -> Option<&RelativePath> {
        self.origin_forms_path.as_ref()
    }

    pub fn origin_forms_sha256(&self) -> Option<Digest256> {
        self.origin_forms_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRecordEventRangePriorOwnerV1 {
    cause_path: RelativePath,
    source_path: RelativePath,
    record_id: String,
    predecessor_cohort_root: Digest256,
    retraction_cause_roots: Vec<Digest256>,
}

impl NavigationRecordEventRangePriorOwnerV1 {
    pub fn cause_path(&self) -> &RelativePath {
        &self.cause_path
    }

    pub fn source_path(&self) -> &RelativePath {
        &self.source_path
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn predecessor_cohort_root(&self) -> Digest256 {
        self.predecessor_cohort_root
    }

    pub fn retraction_cause_roots(&self) -> &[Digest256] {
        &self.retraction_cause_roots
    }
}

pub struct NavigationRecordEventRangeDependencyCauseV1 {
    event_sequence: u64,
    event_id: String,
    command_id: Option<String>,
    changed_dependency_path: RelativePath,
    changed_dependency_sha256: Digest256,
    predecessor_owners: Vec<NavigationRecordEventRangePriorOwnerV1>,
    current_owners: Vec<NavigationRecordEventRangeOwnerKeyV1>,
    root: Digest256,
}

impl NavigationRecordEventRangeDependencyCauseV1 {
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn command_id(&self) -> Option<&str> {
        self.command_id.as_deref()
    }

    pub fn changed_dependency_path(&self) -> &RelativePath {
        &self.changed_dependency_path
    }

    pub fn changed_dependency_sha256(&self) -> Digest256 {
        self.changed_dependency_sha256
    }

    pub fn predecessor_owners(&self) -> &[NavigationRecordEventRangePriorOwnerV1] {
        &self.predecessor_owners
    }

    pub fn current_owners(&self) -> &[NavigationRecordEventRangeOwnerKeyV1] {
        &self.current_owners
    }

    pub fn root(&self) -> Digest256 {
        self.root
    }
}

pub struct NavigationRecordEventRangeClaimCauseV1 {
    event_sequence: u64,
    event_id: String,
    command_id: String,
    claim_id: String,
    claim_stream_path: RelativePath,
    claim_stream_sha256: Digest256,
    receipt_path: RelativePath,
    receipt_sha256: Digest256,
    history_path: RelativePath,
    history_sha256: Digest256,
    revision_receipt_sha256: Digest256,
    predecessor_owners: Vec<NavigationRecordEventRangePriorOwnerV1>,
    current_owners: Vec<NavigationRecordEventRangeOwnerKeyV1>,
    root: Digest256,
}

impl NavigationRecordEventRangeClaimCauseV1 {
    pub fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn command_id(&self) -> &str {
        &self.command_id
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

    pub fn receipt_path(&self) -> &RelativePath {
        &self.receipt_path
    }

    pub fn receipt_sha256(&self) -> Digest256 {
        self.receipt_sha256
    }

    pub fn history_path(&self) -> &RelativePath {
        &self.history_path
    }

    pub fn history_sha256(&self) -> Digest256 {
        self.history_sha256
    }

    pub fn revision_receipt_sha256(&self) -> Digest256 {
        self.revision_receipt_sha256
    }

    pub fn predecessor_owners(&self) -> &[NavigationRecordEventRangePriorOwnerV1] {
        &self.predecessor_owners
    }

    pub fn current_owners(&self) -> &[NavigationRecordEventRangeOwnerKeyV1] {
        &self.current_owners
    }

    pub fn root(&self) -> Digest256 {
        self.root
    }
}

pub struct NavigationRecordEventRangeRowV1 {
    source_path: RelativePath,
    record_id: String,
    predecessor_cohort_root: Option<Digest256>,
    cause_root: Digest256,
    causes: Vec<NavigationRecordEventRangeCauseRefV1>,
    cohort: NavigationRecordEventCohortV1,
}

impl NavigationRecordEventRangeRowV1 {
    pub fn source_path(&self) -> &RelativePath {
        &self.source_path
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn predecessor_cohort_root(&self) -> Option<Digest256> {
        self.predecessor_cohort_root
    }

    pub fn cause_root(&self) -> Digest256 {
        self.cause_root
    }

    pub fn causes(&self) -> &[NavigationRecordEventRangeCauseRefV1] {
        &self.causes
    }

    /// Checked per-record output bound to the real terminal event identity.
    pub fn cohort(&self) -> &NavigationRecordEventCohortV1 {
        &self.cohort
    }
}

/// One complete, terminal-state Record replacement and all removals caused
/// while reconciling the exact event range.
pub struct NavigationRecordEventRangeCohortV1 {
    coverage: NavigationEventRangeCoverageV1,
    claim_range_root: Option<Digest256>,
    prior_owners: Vec<NavigationRecordEventRangePriorOwnerV1>,
    dependency_causes: Vec<NavigationRecordEventRangeDependencyCauseV1>,
    claim_causes: Vec<NavigationRecordEventRangeClaimCauseV1>,
    rows: Vec<NavigationRecordEventRangeRowV1>,
    removed_owners: Vec<NavigationRecordEventRangePriorOwnerV1>,
    root: Digest256,
}

impl NavigationRecordEventRangeCohortV1 {
    pub fn coverage(&self) -> &NavigationEventRangeCoverageV1 {
        &self.coverage
    }

    pub fn coverage_root(&self) -> Digest256 {
        self.coverage.coverage_root()
    }

    pub fn terminal_event_id(&self) -> &str {
        self.coverage.terminal_event_id()
    }

    pub fn through_sequence(&self) -> u64 {
        self.coverage.through_sequence()
    }

    pub fn claim_range_root(&self) -> Option<Digest256> {
        self.claim_range_root
    }

    pub fn prior_owners(&self) -> &[NavigationRecordEventRangePriorOwnerV1] {
        &self.prior_owners
    }

    pub fn dependency_causes(&self) -> &[NavigationRecordEventRangeDependencyCauseV1] {
        &self.dependency_causes
    }

    pub fn claim_causes(&self) -> &[NavigationRecordEventRangeClaimCauseV1] {
        &self.claim_causes
    }

    pub fn rows(&self) -> &[NavigationRecordEventRangeRowV1] {
        &self.rows
    }

    pub fn removed_owners(&self) -> &[NavigationRecordEventRangePriorOwnerV1] {
        &self.removed_owners
    }

    pub fn root(&self) -> Digest256 {
        self.root
    }

    /// Canonical range evidence bytes bound by `root()`. Durable publishers
    /// can persist this manifest and verify the exact terminal owner set,
    /// retractions, and event causes after restart.
    pub fn canonical_manifest_bytes(&self) -> Result<Vec<u8>> {
        self.verify_integrity()?;
        let bytes = canonical_manifest_bytes(&range_cohort_manifest_value(
            &self.coverage,
            self.claim_range_root,
            &self.prior_owners,
            &self.dependency_causes,
            &self.claim_causes,
            &self.rows,
            &self.removed_owners,
        )?)?;
        if Digest256::of_bytes(&bytes) != self.root {
            return Err(Error::Invalid(
                "navigation Record range canonical manifest root",
            ));
        }
        Ok(bytes)
    }

    pub fn verify_integrity(&self) -> Result<()> {
        self.coverage.verify_integrity()?;
        if self.rows.len() > MAX_RANGE_OWNERS
            || self.prior_owners.len() > MAX_RANGE_OWNERS
            || self.dependency_causes.len() > MAX_RANGE_CAUSES
            || self.claim_causes.len() > MAX_RANGE_CAUSES
            || !prior_owner_set_canonical(&self.prior_owners)
            || !prior_owner_set_canonical(&self.removed_owners)
            || self.rows.windows(2).any(|pair| {
                (&pair[0].source_path, &pair[0].record_id)
                    >= (&pair[1].source_path, &pair[1].record_id)
            })
        {
            return Err(Error::Invalid("navigation Record range cohort shape"));
        }
        validate_latest_dependency_owners(&self.rows, &self.dependency_causes)?;
        if let Some(claim_root) = self.claim_range_root {
            if claim_root == Digest256::of_bytes(&[]) {
                return Err(Error::Invalid("navigation Record range Claim root"));
            }
        } else if !self.claim_causes.is_empty() {
            return Err(Error::Invalid(
                "navigation Record range Claim causes lack root",
            ));
        }
        let terminal = self
            .rows
            .iter()
            .map(|row| (row.source_path.as_str(), row.record_id.as_str()))
            .collect::<BTreeSet<_>>();
        let predecessor = self
            .prior_owners
            .iter()
            .map(|owner| {
                (
                    owner.source_path.as_str(),
                    owner.record_id.as_str(),
                    owner.predecessor_cohort_root,
                )
            })
            .collect::<BTreeSet<_>>();
        for row in &self.rows {
            let cost = row.cohort.budget_cost()?;
            let basis = row.cohort.basis();
            let NavigationRecordEventBasisV1::RangeReconciliation {
                coverage_root,
                cause_root,
            } = basis
            else {
                return Err(Error::Invalid("navigation Record range row basis"));
            };
            if row.cohort.event_id() != self.coverage.terminal_event_id()
                || row.cohort.event_sequence() != self.coverage.through_sequence()
                || *coverage_root != self.coverage.coverage_root()
                || *cause_root != row.cause_root
                || row.cohort.source_path() != &row.source_path
                || row.cohort.record_id() != row.record_id.as_str()
                || row.cohort.predecessor_cohort_root() != row.predecessor_cohort_root
                || row.causes.is_empty()
                || cost.rows == 0
            {
                return Err(Error::Invalid("navigation Record range row binding"));
            }
            if owner_cause_root(
                &owner_key(row.source_path.as_str(), row.record_id.as_str()),
                row.predecessor_cohort_root,
                &row.causes,
            )? != row.cause_root
            {
                return Err(Error::Invalid("navigation Record range row cause root"));
            }
            match row.predecessor_cohort_root {
                Some(root)
                    if predecessor.contains(&(
                        row.source_path.as_str(),
                        row.record_id.as_str(),
                        root,
                    )) => {}
                None if !predecessor.iter().any(|(path, id, _)| {
                    *path == row.source_path.as_str() && *id == row.record_id
                }) => {}
                _ => {
                    return Err(Error::Invalid(
                        "navigation Record range predecessor binding",
                    ));
                }
            }
        }
        let mut expected_removed = self
            .prior_owners
            .iter()
            .filter(|owner| {
                !terminal.contains(&(owner.source_path.as_str(), owner.record_id.as_str()))
            })
            .cloned()
            .collect::<Vec<_>>();
        for owner in &mut expected_removed {
            owner.retraction_cause_roots =
                retraction_cause_roots(owner, &self.dependency_causes, &self.claim_causes);
            if owner.retraction_cause_roots.is_empty() {
                return Err(Error::Invalid(
                    "navigation Record range removal lacks cause",
                ));
            }
        }
        expected_removed.sort_by(owner_cmp);
        if expected_removed != self.removed_owners {
            return Err(Error::Invalid("navigation Record range removals differ"));
        }
        let expected_root = range_cohort_root(
            &self.coverage,
            self.claim_range_root,
            &self.prior_owners,
            &self.dependency_causes,
            &self.claim_causes,
            &self.rows,
            &self.removed_owners,
        )?;
        if expected_root != self.root {
            return Err(Error::Invalid("navigation Record range cohort root"));
        }
        Ok(())
    }
}

/// Seal a complete terminal Record owner set for one contiguous CMD range.
/// Every row retains the terminal event's real identity; per-event causes are
/// separately checked against the generic coverage and optional Claim proof.
pub fn project_navigation_record_range_with_rows_v1(
    mut input: NavigationRecordEventRangeInputV1<'_>,
    mut project_row: impl FnMut(
        &NavigationRecordEventRangeOwnerKeyV1,
        Option<Digest256>,
        Digest256,
    ) -> Result<NavigationRecordEventCohortV1>,
) -> Result<NavigationRecordEventRangeCohortV1> {
    input.coverage.verify_integrity()?;
    if input.owners.len() > MAX_RANGE_OWNERS
        || input.prior_owners.len() > MAX_RANGE_OWNERS
        || input.dependency_causes.len() > MAX_RANGE_CAUSES
        || input.claim_causes.len() > MAX_RANGE_CAUSES
    {
        return Err(Error::Budget("navigation Record range owner/cause count"));
    }
    let mutation_count = input.owners.iter().try_fold(0usize, |count, owner| {
        count
            .checked_add(owner.mutations.len())
            .ok_or(Error::Budget("navigation Record range mutation count"))
    })?;
    let total_cause_count = mutation_count
        .checked_add(input.dependency_causes.len())
        .and_then(|count| count.checked_add(input.claim_causes.len()))
        .ok_or(Error::Budget("navigation Record range cause count"))?;
    if total_cause_count > MAX_RANGE_CAUSES {
        return Err(Error::Budget("navigation Record range cause count"));
    }
    if let Some(manifest) = input.claim_manifest {
        manifest.verify_integrity()?;
        if manifest.coverage().coverage_root() != input.coverage.coverage_root()
            || manifest.coverage().terminal_event_id() != input.coverage.terminal_event_id()
            || manifest.coverage().through_sequence() != input.coverage.through_sequence()
        {
            return Err(Error::Invalid(
                "navigation Record range Claim coverage differs",
            ));
        }
    } else if !input.claim_causes.is_empty() {
        return Err(Error::Invalid(
            "navigation Record range Claim manifest absent",
        ));
    }

    let prior_owners = seal_prior_owners(input.prior_owners)?;
    let mut prior_by_key = BTreeMap::new();
    for owner in &prior_owners {
        let key = owner_key(owner.source_path.as_str(), &owner.record_id);
        if prior_by_key
            .insert(key, owner.predecessor_cohort_root)
            .is_some_and(|previous| previous != owner.predecessor_cohort_root)
        {
            return Err(Error::Invalid(
                "navigation Record range predecessor roots disagree",
            ));
        }
    }

    let mut dependency_causes = input
        .dependency_causes
        .iter()
        .map(|cause| seal_dependency_cause(cause, input.coverage, &prior_by_key))
        .collect::<Result<Vec<_>>>()?;
    dependency_causes.sort_by(|left, right| {
        (left.event_sequence, &left.changed_dependency_path)
            .cmp(&(right.event_sequence, &right.changed_dependency_path))
    });
    if dependency_causes.windows(2).any(|pair| {
        pair[0].event_sequence == pair[1].event_sequence
            && pair[0].changed_dependency_path == pair[1].changed_dependency_path
    }) {
        return Err(Error::Invalid(
            "navigation Record range dependency cause repeated",
        ));
    }

    let claim_range_root = input.claim_manifest.map(|manifest| manifest.root());
    let mut claim_causes = input
        .claim_causes
        .iter()
        .map(|cause| seal_claim_cause(cause, input.coverage, input.claim_manifest, &prior_by_key))
        .collect::<Result<Vec<_>>>()?;
    claim_causes.sort_by(|left, right| {
        (left.event_sequence, &left.claim_id).cmp(&(right.event_sequence, &right.claim_id))
    });
    if claim_causes.windows(2).any(|pair| {
        pair[0].event_sequence == pair[1].event_sequence && pair[0].claim_id == pair[1].claim_id
    }) {
        return Err(Error::Invalid(
            "navigation Record range Claim cause repeated",
        ));
    }

    let mut owner_keys = Vec::with_capacity(input.owners.len());
    let mut owner_key_set = BTreeSet::new();
    for owner in &input.owners {
        let key = seal_owner_key(&owner.key)?;
        if !owner_key_set.insert(key.clone()) {
            return Err(Error::Invalid(
                "navigation Record range terminal owner repeated",
            ));
        }
        owner_keys.push(key);
    }

    let mut record_mutation_causes = Vec::new();
    let mut mutation_owner_union = BTreeSet::new();
    for owner in &input.owners {
        let owner_key = seal_owner_key(&owner.key)?;
        for cause in &owner.mutations {
            let sealed = seal_record_mutation_cause(
                cause,
                &owner_key,
                input.coverage,
                owner.terminal_history_bytes,
            )?;
            mutation_owner_union.insert(owner_key.clone());
            record_mutation_causes.push((owner_key.clone(), sealed));
        }
    }

    ensure_every_create_receipt_is_bound(
        input.coverage,
        &record_mutation_causes,
        input.claim_manifest,
    )?;
    ensure_every_changed_record_member_is_bound(
        input.coverage,
        &input.owners,
        &record_mutation_causes,
    )?;

    let mut current_union = mutation_owner_union;
    for cause in &dependency_causes {
        for owner in &cause.current_owners {
            current_union.insert(owner_key(&owner.source_path, &owner.record_id));
        }
    }
    for cause in &claim_causes {
        for owner in &cause.current_owners {
            current_union.insert(owner_key(&owner.source_path, &owner.record_id));
        }
    }
    let terminal_keys = owner_keys.iter().cloned().collect::<BTreeSet<_>>();
    if current_union != terminal_keys {
        return Err(Error::Invalid(
            "navigation Record range terminal owner closure differs",
        ));
    }

    let mut expected_prior = BTreeSet::new();
    for (_, cause) in &record_mutation_causes {
        if cause.kind == NavigationRecordEventRangeCauseKindV1::RecordRevision {
            let key = owner_key(cause.path.as_str(), &cause.record_id);
            if prior_by_key.contains_key(&key) {
                expected_prior.insert(key);
            }
        }
    }
    for cause in &dependency_causes {
        for owner in &cause.predecessor_owners {
            expected_prior.insert(owner_key(owner.source_path.as_str(), &owner.record_id));
        }
    }
    for cause in &claim_causes {
        for owner in &cause.predecessor_owners {
            expected_prior.insert(owner_key(owner.source_path.as_str(), &owner.record_id));
        }
    }
    if expected_prior
        != prior_owners
            .iter()
            .map(|owner| owner_key(owner.source_path.as_str(), &owner.record_id))
            .collect::<BTreeSet<_>>()
    {
        return Err(Error::Invalid(
            "navigation Record range predecessor owner closure differs",
        ));
    }
    validate_prior_owner_causes(
        &prior_owners,
        &record_mutation_causes,
        &dependency_causes,
        &claim_causes,
    )?;

    let mut rows = Vec::with_capacity(input.owners.len());
    for owner in input.owners.drain(..) {
        let key = seal_owner_key(&owner.key)?;
        let source_path = checked_path(&key.source_path)?;
        let record_id = key.record_id.clone();
        let predecessor_cohort_root = prior_by_key.get(&key).copied();
        match (predecessor_cohort_root, prior_by_key.get(&key)) {
            (Some(observed), Some(expected)) if observed == *expected => {}
            (None, None) => {}
            _ => {
                return Err(Error::Invalid(
                    "navigation Record range owner predecessor root",
                ));
            }
        }
        let mut causes = record_mutation_causes
            .iter()
            .filter(|(cause_owner, _)| cause_owner == &key)
            .map(|(_, cause)| cause.reference())
            .collect::<Vec<_>>();
        causes.extend(
            dependency_causes
                .iter()
                .filter(|cause| {
                    cause.current_owners.iter().any(|owner| {
                        owner.source_path == key.source_path && owner.record_id == key.record_id
                    })
                })
                .map(|cause| NavigationRecordEventRangeCauseRefV1 {
                    event_sequence: cause.event_sequence,
                    event_id: cause.event_id.clone(),
                    command_id: cause.command_id.clone(),
                    kind: NavigationRecordEventRangeCauseKindV1::Dependency,
                    path: cause.changed_dependency_path.clone(),
                    sha256: cause.changed_dependency_sha256,
                    mutation_identity: None,
                    cause_root: cause.root,
                }),
        );
        causes.extend(
            claim_causes
                .iter()
                .filter(|cause| {
                    cause.current_owners.iter().any(|owner| {
                        owner.source_path == key.source_path && owner.record_id == key.record_id
                    })
                })
                .map(|cause| NavigationRecordEventRangeCauseRefV1 {
                    event_sequence: cause.event_sequence,
                    event_id: cause.event_id.clone(),
                    command_id: Some(cause.command_id.clone()),
                    kind: NavigationRecordEventRangeCauseKindV1::ClaimRevision,
                    path: cause.claim_stream_path.clone(),
                    sha256: cause.claim_stream_sha256,
                    mutation_identity: None,
                    cause_root: cause.root,
                }),
        );
        causes.sort_by(|left, right| {
            (left.event_sequence, &left.kind, &left.path, left.cause_root).cmp(&(
                right.event_sequence,
                &right.kind,
                &right.path,
                right.cause_root,
            ))
        });
        if causes.is_empty()
            || causes.windows(2).any(|pair| {
                pair[0].event_sequence == pair[1].event_sequence
                    && pair[0].kind == pair[1].kind
                    && pair[0].path == pair[1].path
                    && pair[0].cause_root == pair[1].cause_root
            })
        {
            return Err(Error::Invalid(
                "navigation Record range owner has no exact cause",
            ));
        }
        let cause_root = owner_cause_root(&key, predecessor_cohort_root, &causes)?;
        let cohort = project_row(&key, predecessor_cohort_root, cause_root)?;
        validate_projected_row(
            &cohort,
            &key,
            predecessor_cohort_root,
            cause_root,
            input.coverage,
            &dependency_causes,
            owner.terminal_history_bytes,
        )?;
        rows.push(NavigationRecordEventRangeRowV1 {
            source_path,
            record_id,
            predecessor_cohort_root,
            cause_root,
            causes,
            cohort,
        });
    }
    rows.sort_by(|left, right| {
        (&left.source_path, &left.record_id).cmp(&(&right.source_path, &right.record_id))
    });
    validate_latest_dependency_owners(&rows, &dependency_causes)?;

    let current_set = rows
        .iter()
        .map(|row| owner_key(row.source_path.as_str(), &row.record_id))
        .collect::<BTreeSet<_>>();
    let mut removed_owners = prior_owners
        .iter()
        .filter(|owner| {
            !current_set.contains(&owner_key(owner.source_path.as_str(), &owner.record_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    for owner in &mut removed_owners {
        owner.retraction_cause_roots =
            retraction_cause_roots(owner, &dependency_causes, &claim_causes);
        if owner.retraction_cause_roots.is_empty() {
            return Err(Error::Invalid(
                "navigation Record range removal lacks cause",
            ));
        }
    }
    removed_owners.sort_by(owner_cmp);
    let mut result = NavigationRecordEventRangeCohortV1 {
        coverage: input.coverage.clone(),
        claim_range_root,
        prior_owners,
        dependency_causes,
        claim_causes,
        rows,
        removed_owners,
        root: Digest256::of_bytes(&[]),
    };
    result.root = range_cohort_root(
        &result.coverage,
        result.claim_range_root,
        &result.prior_owners,
        &result.dependency_causes,
        &result.claim_causes,
        &result.rows,
        &result.removed_owners,
    )?;
    result.verify_integrity()?;
    Ok(result)
}

struct SealedRecordMutationCause {
    event_sequence: u64,
    event_id: String,
    command_id: String,
    kind: NavigationRecordEventRangeCauseKindV1,
    path: RelativePath,
    sha256: Digest256,
    record_id: String,
    mutation_identity: NavigationRecordEventRangeMutationIdentityV1,
    root: Digest256,
}

impl SealedRecordMutationCause {
    fn reference(&self) -> NavigationRecordEventRangeCauseRefV1 {
        NavigationRecordEventRangeCauseRefV1 {
            event_sequence: self.event_sequence,
            event_id: self.event_id.clone(),
            command_id: Some(self.command_id.clone()),
            kind: self.kind.clone(),
            path: self.path.clone(),
            sha256: self.sha256,
            mutation_identity: Some(self.mutation_identity.clone()),
            cause_root: self.root,
        }
    }
}

fn seal_record_mutation_cause(
    input: &NavigationRecordEventRangeMutationCauseInputV1<'_>,
    owner: &NavigationRecordEventRangeOwnerKeyV1,
    coverage: &NavigationEventRangeCoverageV1,
    terminal_history_bytes: Option<&[u8]>,
) -> Result<SealedRecordMutationCause> {
    let event = exact_event(coverage, input.event_sequence, input.event_id)?;
    if event.event_kind != "command"
        || event.command_id.as_deref() != Some(input.command_id)
        || !valid_id(input.command_id)
    {
        return Err(Error::Invalid(
            "navigation Record range mutation command identity",
        ));
    }
    match &input.kind {
        NavigationRecordEventRangeMutationKindInputV1::DirectCreate {
            source_path,
            source_bytes,
            receipt_path,
            receipt_bytes,
            provenance_event_path,
            provenance_event_bytes,
            origin_forms_path,
            origin_forms_bytes,
        } => {
            let source_path = checked_path(source_path)?;
            let receipt_path = checked_path(receipt_path)?;
            let provenance_event_path = checked_path(provenance_event_path)?;
            if source_path.as_str() != owner.source_path
                || receipt_path.as_str()
                    != adjacent(source_path.as_str(), "source-create-receipt.json")?
                || provenance_event_path.as_str()
                    != adjacent(source_path.as_str(), "source-create-provenance.jsonl")?
                || source_bytes.is_empty()
                || receipt_bytes.is_empty()
                || provenance_event_bytes.is_empty()
                || origin_forms_path.is_some() != origin_forms_bytes.is_some()
            {
                return Err(Error::Invalid("navigation Record range create cause shape"));
            }
            verify_range_member(
                coverage,
                input.event_sequence,
                source_path.as_str(),
                true,
                source_bytes,
            )?;
            verify_range_member(
                coverage,
                input.event_sequence,
                receipt_path.as_str(),
                false,
                receipt_bytes,
            )?;
            verify_range_member(
                coverage,
                input.event_sequence,
                provenance_event_path.as_str(),
                true,
                provenance_event_bytes,
            )?;
            let record = strict_value(source_bytes, MAX_RANGE_BYTES)?;
            let receipt = strict_value(receipt_bytes, MAX_RANGE_BYTES)?;
            let source_id = record_identity(&record)?;
            if source_id != owner.record_id
                || receipt.get("schema_version").and_then(Value::as_str)
                    != Some("tos_local_source_create_receipt_v1")
                || receipt.get("command_id").and_then(Value::as_str) != Some(input.command_id)
                || receipt.get("source_path").and_then(Value::as_str) != Some(source_path.as_str())
                || receipt.pointer("/source/id").and_then(Value::as_str)
                    != Some(owner.record_id.as_str())
                || receipt.pointer("/source/version").and_then(Value::as_u64) != Some(1)
                // The subject digest is canonical Record identity. Exact file
                // bytes remain bound by range coverage and receipt_file below.
                || receipt.pointer("/source/digest").and_then(Value::as_str)
                    != Some(format!("sha256:{}", record_digest(&record, MAX_RANGE_BYTES)?).as_str())
                || receipt_file(&receipt, source_path.as_str(), source_bytes).is_err()
                || receipt_file(
                    &receipt,
                    provenance_event_path.as_str(),
                    provenance_event_bytes,
                )
                .is_err()
            {
                return Err(Error::Invalid(
                    "navigation Record range create receipt binding",
                ));
            }
            let forms_path = source_path
                .as_str()
                .strip_suffix(".json")
                .map(|stem| format!("{stem}.human-forms.json"))
                .ok_or(Error::Invalid("navigation Record range create forms path"))?;
            let forms_basename = forms_path.rsplit('/').next().ok_or(Error::Invalid(
                "navigation Record range create forms basename",
            ))?;
            let declared_forms = receipt
                .get("files")
                .and_then(Value::as_object)
                .is_some_and(|files| files.contains_key(forms_basename));
            match (declared_forms, origin_forms_path, origin_forms_bytes) {
                (true, Some(path), Some(bytes)) if *path == forms_path.as_str() => {
                    verify_range_member(coverage, input.event_sequence, path, true, bytes)?;
                    receipt_file(&receipt, path, bytes)?;
                }
                (false, None, None) => {}
                _ => {
                    return Err(Error::Invalid(
                        "navigation Record range create forms binding",
                    ));
                }
            }
            let provenance_record =
                select_event_by_receipt(provenance_event_bytes, receipt_path.as_str())?;
            if provenance_record
                .pointer("/record_binding/manifest_ref")
                .and_then(Value::as_str)
                != Some(receipt_path.as_str())
                || provenance_record
                    .pointer("/record_binding/digest_scope")
                    .and_then(Value::as_str)
                    != Some("exact_event_record_bytes")
                || !event_outputs_match(&provenance_record, source_path.as_str(), source_bytes)?
            {
                return Err(Error::Invalid(
                    "navigation Record range create event output binding",
                ));
            }
            if let Some(bytes) = origin_forms_bytes {
                if !event_outputs_match(&provenance_record, &forms_path, bytes)? {
                    return Err(Error::Invalid(
                        "navigation Record range create forms event output binding",
                    ));
                }
            }
            let root = cause_digest(&json!({
                "schema":"tos_source_navigation_record_range_mutation_cause_v1",
                "kind":"direct-create",
                "event_sequence":input.event_sequence,
                "event_id":input.event_id,
                "command_id":input.command_id,
                "source_path":source_path.as_str(),
                "source_sha256":Digest256::of_bytes(source_bytes).to_hex(),
                "receipt_path":receipt_path.as_str(),
                "receipt_sha256":Digest256::of_bytes(receipt_bytes).to_hex(),
                "provenance_event_path":provenance_event_path.as_str(),
                "provenance_event_sha256":Digest256::of_bytes(provenance_event_bytes).to_hex(),
                "origin_forms_path":origin_forms_path,
                "origin_forms_sha256":origin_forms_bytes.map(Digest256::of_bytes).map(Digest256::to_hex),
            }))?;
            let source_sha256 = Digest256::of_bytes(source_bytes);
            let mutation_identity = NavigationRecordEventRangeMutationIdentityV1 {
                record_id: owner.record_id.clone(),
                source_path: source_path.clone(),
                source_sha256,
                history_path: None,
                history_sha256: None,
                receipt_path: Some(receipt_path.clone()),
                receipt_sha256: Some(Digest256::of_bytes(receipt_bytes)),
                provenance_event_path: Some(provenance_event_path.clone()),
                provenance_event_sha256: Some(Digest256::of_bytes(provenance_event_bytes)),
                origin_forms_path: origin_forms_path.map(checked_path).transpose()?,
                origin_forms_sha256: origin_forms_bytes.map(Digest256::of_bytes),
            };
            Ok(SealedRecordMutationCause {
                event_sequence: input.event_sequence,
                event_id: input.event_id.to_owned(),
                command_id: input.command_id.to_owned(),
                kind: NavigationRecordEventRangeCauseKindV1::DirectCreate,
                path: source_path,
                sha256: source_sha256,
                record_id: owner.record_id.clone(),
                mutation_identity,
                root,
            })
        }
        NavigationRecordEventRangeMutationKindInputV1::Revision {
            source_path,
            source_bytes,
            history_path,
            history_bytes,
        } => {
            let source_path = checked_path(source_path)?;
            let history_path = checked_path(history_path)?;
            if source_path.as_str() != owner.source_path
                || history_path.as_str()
                    != adjacent(source_path.as_str(), "source-revision-history.json")?
                || source_bytes.is_empty()
                || history_bytes.is_empty()
            {
                return Err(Error::Invalid(
                    "navigation Record range revision cause shape",
                ));
            }
            verify_range_member(
                coverage,
                input.event_sequence,
                source_path.as_str(),
                true,
                source_bytes,
            )?;
            verify_range_member(
                coverage,
                input.event_sequence,
                history_path.as_str(),
                true,
                history_bytes,
            )?;
            let record = strict_value(source_bytes, MAX_RANGE_BYTES)?;
            if record_identity(&record)? != owner.record_id {
                return Err(Error::Invalid("navigation Record range revision identity"));
            }
            verify_revision_history_snapshot(
                terminal_history_bytes,
                history_bytes,
                input.command_id,
                &record,
                owner.record_id.as_str(),
            )?;
            let root = cause_digest(&json!({
                "schema":"tos_source_navigation_record_range_mutation_cause_v1",
                "kind":"record-revision",
                "event_sequence":input.event_sequence,
                "event_id":input.event_id,
                "command_id":input.command_id,
                "source_path":source_path.as_str(),
                "source_sha256":Digest256::of_bytes(source_bytes).to_hex(),
                "history_path":history_path.as_str(),
                "history_sha256":Digest256::of_bytes(history_bytes).to_hex(),
            }))?;
            let source_sha256 = Digest256::of_bytes(source_bytes);
            let mutation_identity = NavigationRecordEventRangeMutationIdentityV1 {
                record_id: owner.record_id.clone(),
                source_path: source_path.clone(),
                source_sha256,
                history_path: Some(history_path.clone()),
                history_sha256: Some(Digest256::of_bytes(history_bytes)),
                receipt_path: None,
                receipt_sha256: None,
                provenance_event_path: None,
                provenance_event_sha256: None,
                origin_forms_path: None,
                origin_forms_sha256: None,
            };
            Ok(SealedRecordMutationCause {
                event_sequence: input.event_sequence,
                event_id: input.event_id.to_owned(),
                command_id: input.command_id.to_owned(),
                kind: NavigationRecordEventRangeCauseKindV1::RecordRevision,
                path: source_path,
                sha256: source_sha256,
                record_id: owner.record_id.clone(),
                mutation_identity,
                root,
            })
        }
    }
}

fn seal_dependency_cause(
    input: &NavigationRecordEventRangeDependencyCauseInputV1<'_>,
    coverage: &NavigationEventRangeCoverageV1,
    prior_by_key: &BTreeMap<NavigationRecordEventRangeOwnerKeyV1, Digest256>,
) -> Result<NavigationRecordEventRangeDependencyCauseV1> {
    let event = exact_event(coverage, input.event_sequence, input.event_id)?;
    let path = checked_path(input.changed_dependency_path)?;
    if !path.as_str().starts_with("ToS/")
        || input.changed_dependency_bytes.is_empty()
        || path.as_str().ends_with("/source-create-receipt.json")
        || path.as_str().ends_with("/source-revision-history.json")
        || event.command_id.as_deref() != input.command_id
    {
        return Err(Error::Invalid(
            "navigation Record range dependency cause shape",
        ));
    }
    verify_range_member(
        coverage,
        input.event_sequence,
        path.as_str(),
        true,
        input.changed_dependency_bytes,
    )?;
    let mut predecessor_owners = seal_prior_owners(input.predecessor_owners)?;
    let mut current_owners = seal_owner_keys(input.current_owners)?;
    for owner in &predecessor_owners {
        if owner.cause_path.as_str() != path.as_str() {
            return Err(Error::Invalid(
                "navigation Record range dependency predecessor cause path",
            ));
        }
        if prior_by_key.get(&owner_key(owner.source_path.as_str(), &owner.record_id))
            != Some(&owner.predecessor_cohort_root)
        {
            return Err(Error::Invalid(
                "navigation Record range dependency predecessor absent",
            ));
        }
    }
    predecessor_owners.sort_by(owner_cmp);
    current_owners.sort();
    if predecessor_owners.is_empty() && current_owners.is_empty() {
        return Err(Error::Invalid(
            "navigation Record range dependency has no owners",
        ));
    }
    let sha256 = Digest256::of_bytes(input.changed_dependency_bytes);
    let root = cause_digest(&json!({
        "schema":"tos_source_navigation_record_range_dependency_cause_v1",
        "event_sequence":input.event_sequence,
        "event_id":input.event_id,
        "command_id":input.command_id,
        "changed_dependency_path":path.as_str(),
        "changed_dependency_sha256":sha256.to_hex(),
        "predecessor_owners":prior_owners_json(&predecessor_owners),
        "current_owners":owner_keys_json(&current_owners),
    }))?;
    Ok(NavigationRecordEventRangeDependencyCauseV1 {
        event_sequence: input.event_sequence,
        event_id: input.event_id.to_owned(),
        command_id: input.command_id.map(str::to_owned),
        changed_dependency_path: path,
        changed_dependency_sha256: sha256,
        predecessor_owners,
        current_owners,
        root,
    })
}

fn seal_claim_cause(
    input: &NavigationRecordEventRangeClaimCauseInputV1<'_>,
    coverage: &NavigationEventRangeCoverageV1,
    manifest: Option<&NavigationClaimEventRangeManifestV1>,
    prior_by_key: &BTreeMap<NavigationRecordEventRangeOwnerKeyV1, Digest256>,
) -> Result<NavigationRecordEventRangeClaimCauseV1> {
    let manifest = manifest.ok_or(Error::Invalid(
        "navigation Record range Claim manifest absent",
    ))?;
    let event = exact_event(coverage, input.event_sequence, input.event_id)?;
    let command_id = event.command_id.as_deref().ok_or(Error::Invalid(
        "navigation Record range Claim command absent",
    ))?;
    if event.event_kind != "command" || !valid_id(input.claim_id) {
        return Err(Error::Invalid(
            "navigation Record range Claim cause identity",
        ));
    }
    let mutation = manifest
        .mutations()
        .iter()
        .find(|mutation| {
            mutation.event_sequence() == input.event_sequence
                && mutation.event_id() == input.event_id
                && mutation
                    .mutation_claim_ids()
                    .binary_search_by(|id| id.as_str().cmp(input.claim_id))
                    .is_ok()
                && mutation.revision_history_path().is_some()
        })
        .ok_or(Error::Invalid(
            "navigation Record range Claim revision proof absent",
        ))?;
    let history_path = mutation
        .revision_history_path()
        .ok_or(Error::Invalid(
            "navigation Record range Claim history absent",
        ))?
        .clone();
    let history_sha256 = mutation.revision_history_sha256().ok_or(Error::Invalid(
        "navigation Record range Claim history digest absent",
    ))?;
    let revision_receipt_sha256 = mutation.revision_receipt_sha256().ok_or(Error::Invalid(
        "navigation Record range Claim revision receipt absent",
    ))?;
    let mut predecessor_owners = seal_prior_owners(input.predecessor_owners)?;
    let mut current_owners = seal_owner_keys(input.current_owners)?;
    if predecessor_owners.iter().any(|owner| {
        owner.cause_path.as_str() != mutation.claim_stream_member_path().as_str()
            && owner.cause_path.as_str() != history_path.as_str()
    }) {
        return Err(Error::Invalid(
            "navigation Record range Claim predecessor cause path",
        ));
    }
    for owner in &predecessor_owners {
        if prior_by_key.get(&owner_key(owner.source_path.as_str(), &owner.record_id))
            != Some(&owner.predecessor_cohort_root)
        {
            return Err(Error::Invalid(
                "navigation Record range Claim predecessor absent",
            ));
        }
    }
    predecessor_owners.sort_by(owner_cmp);
    current_owners.sort();
    if predecessor_owners.is_empty() && current_owners.is_empty() {
        return Err(Error::Invalid(
            "navigation Record range Claim cause has no owners",
        ));
    }
    let root = cause_digest(&json!({
        "schema":"tos_source_navigation_record_range_claim_cause_v1",
        "claim_range_root":manifest.root().to_hex(),
        "event_sequence":input.event_sequence,
        "event_id":input.event_id,
        "command_id":command_id,
        "claim_id":input.claim_id,
        "claim_stream_path":mutation.claim_stream_member_path().as_str(),
        "claim_stream_sha256":mutation.claim_stream_sha256().to_hex(),
        "receipt_path":mutation.receipt_path().as_str(),
        "receipt_sha256":mutation.receipt_sha256().to_hex(),
        "history_path":history_path.as_str(),
        "history_sha256":history_sha256.to_hex(),
        "revision_receipt_sha256":revision_receipt_sha256.to_hex(),
        "predecessor_owners":prior_owners_json(&predecessor_owners),
        "current_owners":owner_keys_json(&current_owners),
    }))?;
    Ok(NavigationRecordEventRangeClaimCauseV1 {
        event_sequence: input.event_sequence,
        event_id: input.event_id.to_owned(),
        command_id: command_id.to_owned(),
        claim_id: input.claim_id.to_owned(),
        claim_stream_path: mutation.claim_stream_member_path().clone(),
        claim_stream_sha256: mutation.claim_stream_sha256(),
        receipt_path: mutation.receipt_path().clone(),
        receipt_sha256: mutation.receipt_sha256(),
        history_path,
        history_sha256,
        revision_receipt_sha256,
        predecessor_owners,
        current_owners,
        root,
    })
}

fn ensure_every_create_receipt_is_bound(
    coverage: &NavigationEventRangeCoverageV1,
    record_causes: &[(
        NavigationRecordEventRangeOwnerKeyV1,
        SealedRecordMutationCause,
    )],
    claim_manifest: Option<&NavigationClaimEventRangeManifestV1>,
) -> Result<()> {
    let claim_receipts = claim_manifest
        .into_iter()
        .flat_map(|manifest| manifest.mutations())
        .filter(|mutation| mutation.revision_history_path().is_none())
        .map(|mutation| (mutation.event_sequence(), mutation.receipt_path().as_str()))
        .collect::<BTreeSet<_>>();
    for event in coverage.events() {
        for member in event.members.iter().filter(|member| {
            member.source_path.starts_with("ToS/")
                && member.source_path.ends_with("/source-create-receipt.json")
        }) {
            let record_receipt = record_causes.iter().any(|(_, cause)| {
                cause.event_sequence == event.sequence
                    && cause.kind == NavigationRecordEventRangeCauseKindV1::DirectCreate
                    && adjacent(cause.path.as_str(), "source-create-receipt.json")
                        .is_ok_and(|path| path == member.source_path)
            });
            let claim_receipt =
                claim_receipts.contains(&(event.sequence, member.source_path.as_str()));
            if !record_receipt && !claim_receipt {
                return Err(Error::Invalid(
                    "navigation Record range create receipt omitted",
                ));
            }
        }
    }
    Ok(())
}

fn ensure_every_changed_record_member_is_bound(
    coverage: &NavigationEventRangeCoverageV1,
    owners: &[NavigationRecordEventRangeOwnerInputV1<'_>],
    causes: &[(
        NavigationRecordEventRangeOwnerKeyV1,
        SealedRecordMutationCause,
    )],
) -> Result<()> {
    for owner in owners {
        let key = &owner.key;
        let history_path = adjacent(&key.source_path, "source-revision-history.json")?;
        for event in coverage.events() {
            let changed = event.members.iter().any(|member| {
                member.is_source
                    && (member.source_path == key.source_path || member.source_path == history_path)
            });
            if changed
                && !causes.iter().any(|(cause_owner, cause)| {
                    cause_owner == key && cause.event_sequence == event.sequence
                })
            {
                return Err(Error::Invalid(
                    "navigation Record range mutation cause omitted",
                ));
            }
        }
    }
    Ok(())
}

fn validate_prior_owner_causes(
    prior_owners: &[NavigationRecordEventRangePriorOwnerV1],
    record_causes: &[(
        NavigationRecordEventRangeOwnerKeyV1,
        SealedRecordMutationCause,
    )],
    dependency_causes: &[NavigationRecordEventRangeDependencyCauseV1],
    claim_causes: &[NavigationRecordEventRangeClaimCauseV1],
) -> Result<()> {
    for cause_owner in dependency_causes
        .iter()
        .flat_map(|cause| cause.predecessor_owners.iter())
        .chain(
            claim_causes
                .iter()
                .flat_map(|cause| cause.predecessor_owners.iter()),
        )
    {
        if !prior_owners
            .iter()
            .any(|prior| owner_identity_eq(prior, cause_owner))
        {
            return Err(Error::Invalid(
                "navigation Record range cause predecessor absent from manifest",
            ));
        }
    }

    for prior in prior_owners {
        let dependency_bound = dependency_causes.iter().any(|cause| {
            cause.changed_dependency_path.as_str() == prior.cause_path.as_str()
                && cause
                    .predecessor_owners
                    .iter()
                    .any(|candidate| owner_identity_eq(candidate, prior))
        });
        let claim_bound = claim_causes.iter().any(|cause| {
            (cause.claim_stream_path.as_str() == prior.cause_path.as_str()
                || cause.history_path.as_str() == prior.cause_path.as_str())
                && cause
                    .predecessor_owners
                    .iter()
                    .any(|candidate| owner_identity_eq(candidate, prior))
        });
        let revision_bound = record_causes.iter().any(|(key, cause)| {
            cause.kind == NavigationRecordEventRangeCauseKindV1::RecordRevision
                && key.source_path == prior.source_path.as_str()
                && key.record_id == prior.record_id
                && (cause.mutation_identity.source_path.as_str() == prior.cause_path.as_str()
                    || cause
                        .mutation_identity
                        .history_path
                        .as_ref()
                        .is_some_and(|path| path.as_str() == prior.cause_path.as_str()))
        });
        if !dependency_bound && !claim_bound && !revision_bound {
            return Err(Error::Invalid(
                "navigation Record range predecessor cause path unbound",
            ));
        }
    }

    for (_, cause) in record_causes
        .iter()
        .filter(|(_, cause)| cause.kind == NavigationRecordEventRangeCauseKindV1::RecordRevision)
    {
        let has_predecessor = prior_owners.iter().any(|prior| {
            prior.source_path.as_str() == cause.mutation_identity.source_path.as_str()
                && prior.record_id == cause.record_id
                && (prior.cause_path.as_str() == cause.mutation_identity.source_path.as_str()
                    || cause
                        .mutation_identity
                        .history_path
                        .as_ref()
                        .is_some_and(|path| prior.cause_path.as_str() == path.as_str()))
        });
        if !has_predecessor {
            return Err(Error::Invalid(
                "navigation Record range revision predecessor path absent",
            ));
        }
    }
    Ok(())
}

fn validate_projected_row(
    cohort: &NavigationRecordEventCohortV1,
    owner: &NavigationRecordEventRangeOwnerKeyV1,
    predecessor_cohort_root: Option<Digest256>,
    cause_root: Digest256,
    coverage: &NavigationEventRangeCoverageV1,
    dependency_causes: &[NavigationRecordEventRangeDependencyCauseV1],
    terminal_history_bytes: Option<&[u8]>,
) -> Result<()> {
    let NavigationRecordEventBasisV1::RangeReconciliation {
        coverage_root,
        cause_root: observed_cause_root,
    } = cohort.basis()
    else {
        return Err(Error::Invalid("navigation Record range row basis"));
    };
    if cohort.event_id() != coverage.terminal_event_id()
        || cohort.event_sequence() != coverage.through_sequence()
        || *coverage_root != coverage.coverage_root()
        || *observed_cause_root != cause_root
        || cohort.source_path().as_str() != owner.source_path.as_str()
        || cohort.record_id() != owner.record_id.as_str()
        || cohort.predecessor_cohort_root() != predecessor_cohort_root
    {
        return Err(Error::Invalid(
            "navigation Record range projected row binding",
        ));
    }
    cohort.budget_cost()?;
    let relevant_dependency_paths = dependency_causes
        .iter()
        .filter(|cause| cause.current_owners.contains(owner))
        .map(|cause| cause.changed_dependency_path.as_str())
        .collect::<BTreeSet<_>>();
    for path in relevant_dependency_paths {
        let latest = dependency_causes
            .iter()
            .filter(|cause| cause.changed_dependency_path.as_str() == path)
            .max_by_key(|cause| cause.event_sequence)
            .ok_or(Error::Invalid(
                "navigation Record range owner dependency cause absent",
            ))?;
        let mut selected = cohort
            .dependencies()
            .iter()
            .filter(|dependency| dependency.path.as_str() == path);
        let dependency = selected.next().ok_or(Error::Invalid(
            "navigation Record range owner dependency absent",
        ))?;
        if selected.next().is_some()
            || !matches!(
                dependency.state,
                NavigationDependencyStateV1::Present { sha256, .. }
                    if sha256 == latest.changed_dependency_sha256
            )
        {
            return Err(Error::Invalid(
                "navigation Record range owner dependency differs",
            ));
        }
    }
    let history_path = adjacent(&owner.source_path, "source-revision-history.json")?;
    let mut history_dependencies = cohort
        .dependencies()
        .iter()
        .filter(|dependency| dependency.path.as_str() == history_path);
    let history_dependency = history_dependencies.next().ok_or(Error::Invalid(
        "navigation Record range history dependency absent",
    ))?;
    if history_dependencies.next().is_some() {
        return Err(Error::Invalid(
            "navigation Record range history dependency repeated",
        ));
    }
    match (terminal_history_bytes, &history_dependency.state) {
        (Some(raw), NavigationDependencyStateV1::Present { sha256, size_bytes })
            if !raw.is_empty()
                && raw.len() <= MAX_RANGE_BYTES
                && *sha256 == Digest256::of_bytes(raw)
                && *size_bytes == raw.len() as u64 => {}
        (None, NavigationDependencyStateV1::Absent { .. }) => {}
        _ => {
            return Err(Error::Invalid(
                "navigation Record range history selected bytes differ",
            ));
        }
    }
    Ok(())
}

fn validate_latest_dependency_owners(
    rows: &[NavigationRecordEventRangeRowV1],
    causes: &[NavigationRecordEventRangeDependencyCauseV1],
) -> Result<()> {
    let mut latest = BTreeMap::<&str, &NavigationRecordEventRangeDependencyCauseV1>::new();
    for cause in causes {
        latest
            .entry(cause.changed_dependency_path.as_str())
            .and_modify(|previous| {
                if cause.event_sequence > previous.event_sequence {
                    *previous = cause;
                }
            })
            .or_insert(cause);
    }
    for cause in causes {
        let terminal = latest
            .get(cause.changed_dependency_path.as_str())
            .ok_or(Error::Invalid(
                "navigation Record range dependency latest cause absent",
            ))?;
        if cause.current_owners != terminal.current_owners {
            return Err(Error::Invalid(
                "navigation Record range dependency terminal owner set differs",
            ));
        }
    }
    for (path, cause) in latest {
        let mut projected_owners = Vec::new();
        for row in rows {
            let mut matching = row
                .cohort
                .dependencies()
                .iter()
                .filter(|dependency| dependency.path.as_str() == path);
            let Some(dependency) = matching.next() else {
                continue;
            };
            if matching.next().is_some() {
                return Err(Error::Invalid(
                    "navigation Record range dependency repeated in row",
                ));
            }
            match dependency.state {
                NavigationDependencyStateV1::Present { sha256, .. }
                    if sha256 == cause.changed_dependency_sha256 =>
                {
                    projected_owners.push(owner_key(row.source_path.as_str(), &row.record_id));
                }
                NavigationDependencyStateV1::Present { .. } => {
                    return Err(Error::Invalid(
                        "navigation Record range latest dependency bytes differ",
                    ));
                }
                NavigationDependencyStateV1::Absent { .. } => {}
            }
        }
        projected_owners.sort();
        if projected_owners != cause.current_owners {
            return Err(Error::Invalid(
                "navigation Record range terminal reverse owners differ",
            ));
        }
    }
    Ok(())
}

fn verify_revision_history_snapshot(
    terminal_history_raw: Option<&[u8]>,
    event_history_raw: &[u8],
    command_id: &str,
    source_record: &Value,
    record_id: &str,
) -> Result<()> {
    let terminal_history_raw = terminal_history_raw.ok_or(Error::Invalid(
        "navigation Record range terminal history absent",
    ))?;
    let event_history = strict_value(event_history_raw, MAX_RANGE_BYTES)?;
    let terminal_history = strict_value(terminal_history_raw, MAX_RANGE_BYTES)?;
    let event_receipts = event_history
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid(
            "navigation Record range event history receipts",
        ))?;
    let terminal_receipts = terminal_history
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid(
            "navigation Record range terminal history receipts",
        ))?;
    let schema = event_history.get("schema_version").and_then(Value::as_str);
    if !matches!(
        schema,
        Some("tos_source_revision_history_v1" | "tos_source_revision_history_v2")
    ) || terminal_history
        .get("schema_version")
        .and_then(Value::as_str)
        != schema
        || event_history.get("record_id").and_then(Value::as_str) != Some(record_id)
        || terminal_history.get("record_id").and_then(Value::as_str) != Some(record_id)
        || event_receipts.is_empty()
        || event_receipts.len() > terminal_receipts.len()
        || terminal_receipts.get(..event_receipts.len()) != Some(event_receipts.as_slice())
    {
        return Err(Error::Invalid(
            "navigation Record range history snapshot prefix",
        ));
    }
    let latest = event_receipts.last().ok_or(Error::Invalid(
        "navigation Record range latest history receipt",
    ))?;
    let expected_ref = json!({
        "id": record_id,
        "version": source_record.get("record_version"),
        "digest": format!("sha256:{}", record_digest(source_record, MAX_RANGE_BYTES)?),
    });
    if latest.get("command_id").and_then(Value::as_str) != Some(command_id)
        || latest.get("source") != Some(&expected_ref)
    {
        return Err(Error::Invalid(
            "navigation Record range history event successor",
        ));
    }
    Ok(())
}

fn seal_prior_owners(
    input: &[NavigationRecordEventRangePriorOwnerInputV1],
) -> Result<Vec<NavigationRecordEventRangePriorOwnerV1>> {
    let mut result = input
        .iter()
        .map(|owner| {
            let cause_path = checked_path(&owner.cause_path)?;
            let source_path = checked_path(&owner.source_path)?;
            if !cause_path.as_str().starts_with("ToS/")
                || !valid_record_id(&owner.record_id)
                || owner.predecessor_cohort_root == Digest256::of_bytes(&[])
            {
                return Err(Error::Invalid("navigation Record range predecessor owner"));
            }
            Ok(NavigationRecordEventRangePriorOwnerV1 {
                cause_path,
                source_path,
                record_id: owner.record_id.clone(),
                predecessor_cohort_root: owner.predecessor_cohort_root,
                retraction_cause_roots: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    result.sort_by(owner_cmp);
    if result
        .windows(2)
        .any(|pair| owner_identity_eq(&pair[0], &pair[1]))
    {
        return Err(Error::Invalid(
            "navigation Record range predecessor owner identity repeated",
        ));
    }
    let mut roots = BTreeMap::new();
    for owner in &result {
        let key = owner_key(owner.source_path.as_str(), &owner.record_id);
        if roots
            .insert(key, owner.predecessor_cohort_root)
            .is_some_and(|previous| previous != owner.predecessor_cohort_root)
        {
            return Err(Error::Invalid(
                "navigation Record range predecessor roots disagree",
            ));
        }
    }
    Ok(result)
}

fn seal_owner_keys(
    input: &[NavigationRecordEventRangeOwnerKeyV1],
) -> Result<Vec<NavigationRecordEventRangeOwnerKeyV1>> {
    let mut result = input.to_vec();
    for owner in &result {
        checked_path(&owner.source_path)?;
        if !valid_record_id(&owner.record_id) {
            return Err(Error::Invalid(
                "navigation Record range current owner identity",
            ));
        }
    }
    result.sort();
    if result.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Invalid(
            "navigation Record range current owner repeated",
        ));
    }
    Ok(result)
}

fn exact_event<'a>(
    coverage: &'a NavigationEventRangeCoverageV1,
    sequence: u64,
    event_id: &str,
) -> Result<&'a crate::source_navigation_event::NavigationEventRangeEventV1> {
    let event = coverage
        .event(sequence)
        .ok_or(Error::Invalid("navigation Record range cause event absent"))?;
    if event.event_id != event_id {
        return Err(Error::Invalid(
            "navigation Record range cause event identity",
        ));
    }
    Ok(event)
}

fn verify_range_member(
    coverage: &NavigationEventRangeCoverageV1,
    sequence: u64,
    path: &str,
    require_source: bool,
    raw: &[u8],
) -> Result<()> {
    let member = coverage
        .member(sequence, path)
        .ok_or(Error::Invalid("navigation Record range member absent"))?;
    // Source records/forms require a source member. Supporting receipts may
    // themselves be source members, as in the native creation package; their
    // exact declaration and bytes remain authenticated by range coverage.
    if require_source && !member.is_source {
        return Err(Error::Invalid(
            "navigation Record range member is not a source",
        ));
    }
    if raw.is_empty()
        || raw.len() > MAX_RANGE_BYTES
        || member.size_bytes != raw.len() as u64
        || member.sha256 != Digest256::of_bytes(raw)
    {
        return Err(Error::Invalid(
            "navigation Record range member bytes differ",
        ));
    }
    Ok(())
}

fn seal_owner_key(
    input: &NavigationRecordEventRangeOwnerKeyV1,
) -> Result<NavigationRecordEventRangeOwnerKeyV1> {
    let source_path = checked_path(&input.source_path)?;
    if !valid_record_id(&input.record_id) {
        return Err(Error::Invalid("navigation Record range owner identity"));
    }
    Ok(owner_key(source_path.as_str(), &input.record_id))
}

fn record_identity(record: &Value) -> Result<&str> {
    record
        .get("record_id")
        .or_else(|| record.get("artifact_id"))
        .or_else(|| record.get("composite_id"))
        .and_then(Value::as_str)
        .filter(|value| valid_record_id(value))
        .ok_or(Error::Invalid("navigation Record range source identity"))
}

fn owner_key(path: &str, record_id: &str) -> NavigationRecordEventRangeOwnerKeyV1 {
    NavigationRecordEventRangeOwnerKeyV1 {
        source_path: path.to_owned(),
        record_id: record_id.to_owned(),
    }
}

fn owner_cmp(
    left: &NavigationRecordEventRangePriorOwnerV1,
    right: &NavigationRecordEventRangePriorOwnerV1,
) -> std::cmp::Ordering {
    (&left.cause_path, &left.source_path, &left.record_id).cmp(&(
        &right.cause_path,
        &right.source_path,
        &right.record_id,
    ))
}

fn owner_identity_eq(
    left: &NavigationRecordEventRangePriorOwnerV1,
    right: &NavigationRecordEventRangePriorOwnerV1,
) -> bool {
    left.cause_path == right.cause_path
        && left.source_path == right.source_path
        && left.record_id == right.record_id
}

fn prior_owner_set_canonical(owners: &[NavigationRecordEventRangePriorOwnerV1]) -> bool {
    if owners
        .windows(2)
        .any(|pair| owner_cmp(&pair[0], &pair[1]).is_ge())
    {
        return false;
    }
    let mut roots = BTreeMap::new();
    for owner in owners {
        if !owner.cause_path.as_str().starts_with("ToS/")
            || !valid_record_id(&owner.record_id)
            || owner.predecessor_cohort_root == Digest256::of_bytes(&[])
            || owner
                .retraction_cause_roots
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return false;
        }
        let key = owner_key(owner.source_path.as_str(), &owner.record_id);
        if roots
            .insert(key, owner.predecessor_cohort_root)
            .is_some_and(|previous| previous != owner.predecessor_cohort_root)
        {
            return false;
        }
    }
    true
}

fn retraction_cause_roots(
    owner: &NavigationRecordEventRangePriorOwnerV1,
    dependency_causes: &[NavigationRecordEventRangeDependencyCauseV1],
    claim_causes: &[NavigationRecordEventRangeClaimCauseV1],
) -> Vec<Digest256> {
    dependency_causes
        .iter()
        .filter(|cause| {
            cause.changed_dependency_path.as_str() == owner.cause_path.as_str()
                && cause
                    .predecessor_owners
                    .iter()
                    .any(|candidate| owner_identity_eq(candidate, owner))
        })
        .map(|cause| cause.root)
        .chain(
            claim_causes
                .iter()
                .filter(|cause| {
                    (cause.claim_stream_path.as_str() == owner.cause_path.as_str()
                        || cause.history_path.as_str() == owner.cause_path.as_str())
                        && cause
                            .predecessor_owners
                            .iter()
                            .any(|candidate| owner_identity_eq(candidate, owner))
                })
                .map(|cause| cause.root),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn prior_owner_json(owners: &[NavigationRecordEventRangePriorOwnerV1]) -> Value {
    Value::Array(
        owners
            .iter()
            .map(|owner| {
                json!({
                    "cause_path":owner.cause_path.as_str(),
                    "source_path":owner.source_path.as_str(),
                    "record_id":owner.record_id,
                    "predecessor_cohort_root":owner.predecessor_cohort_root.to_hex(),
                    "retraction_cause_roots":owner.retraction_cause_roots.iter().map(|digest| digest.to_hex()).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

fn prior_owners_json(owners: &[NavigationRecordEventRangePriorOwnerV1]) -> Value {
    Value::Array(
        owners
            .iter()
            .map(|owner| {
                json!({
                    "cause_path":owner.cause_path.as_str(),
                    "source_path":owner.source_path.as_str(),
                    "record_id":owner.record_id,
                    "predecessor_cohort_root":owner.predecessor_cohort_root.to_hex(),
                })
            })
            .collect(),
    )
}

fn mutation_identity_json(identity: &NavigationRecordEventRangeMutationIdentityV1) -> Value {
    json!({
        "record_id":identity.record_id,
        "source_path":identity.source_path.as_str(),
        "source_sha256":identity.source_sha256.to_hex(),
        "history_path":identity.history_path.as_ref().map(RelativePath::as_str),
        "history_sha256":identity.history_sha256.map(Digest256::to_hex),
        "receipt_path":identity.receipt_path.as_ref().map(RelativePath::as_str),
        "receipt_sha256":identity.receipt_sha256.map(Digest256::to_hex),
        "provenance_event_path":identity.provenance_event_path.as_ref().map(RelativePath::as_str),
        "provenance_event_sha256":identity.provenance_event_sha256.map(Digest256::to_hex),
        "origin_forms_path":identity.origin_forms_path.as_ref().map(RelativePath::as_str),
        "origin_forms_sha256":identity.origin_forms_sha256.map(Digest256::to_hex),
    })
}

fn owner_keys_json(owners: &[NavigationRecordEventRangeOwnerKeyV1]) -> Value {
    Value::Array(
        owners
            .iter()
            .map(|owner| json!({"source_path":owner.source_path,"record_id":owner.record_id}))
            .collect(),
    )
}

fn owner_cause_root(
    owner: &NavigationRecordEventRangeOwnerKeyV1,
    predecessor_cohort_root: Option<Digest256>,
    causes: &[NavigationRecordEventRangeCauseRefV1],
) -> Result<Digest256> {
    cause_digest(&json!({
        "schema":"tos_source_navigation_record_range_owner_cause_v1",
        "source_path":owner.source_path,
        "record_id":owner.record_id,
        "predecessor_cohort_root":predecessor_cohort_root.map(Digest256::to_hex),
        "causes":causes.iter().map(|cause| json!({
            "event_sequence":cause.event_sequence,
            "event_id":cause.event_id,
            "command_id":cause.command_id,
            "kind":cause.kind.as_str(),
            "path":cause.path.as_str(),
            "sha256":cause.sha256.to_hex(),
            "mutation_identity":cause.mutation_identity.as_ref().map(mutation_identity_json),
            "cause_root":cause.cause_root.to_hex(),
        })).collect::<Vec<_>>(),
    }))
}

fn range_cohort_root(
    coverage: &NavigationEventRangeCoverageV1,
    claim_range_root: Option<Digest256>,
    prior_owners: &[NavigationRecordEventRangePriorOwnerV1],
    dependency_causes: &[NavigationRecordEventRangeDependencyCauseV1],
    claim_causes: &[NavigationRecordEventRangeClaimCauseV1],
    rows: &[NavigationRecordEventRangeRowV1],
    removed_owners: &[NavigationRecordEventRangePriorOwnerV1],
) -> Result<Digest256> {
    cause_digest(&range_cohort_manifest_value(
        coverage,
        claim_range_root,
        prior_owners,
        dependency_causes,
        claim_causes,
        rows,
        removed_owners,
    )?)
}

fn range_cohort_manifest_value(
    coverage: &NavigationEventRangeCoverageV1,
    claim_range_root: Option<Digest256>,
    prior_owners: &[NavigationRecordEventRangePriorOwnerV1],
    dependency_causes: &[NavigationRecordEventRangeDependencyCauseV1],
    claim_causes: &[NavigationRecordEventRangeClaimCauseV1],
    rows: &[NavigationRecordEventRangeRowV1],
    removed_owners: &[NavigationRecordEventRangePriorOwnerV1],
) -> Result<Value> {
    Ok(json!({
        "schema":"tos_source_navigation_record_event_range_cohort_v1",
        "cursor_before":coverage.cursor_before(),
        "through_sequence":coverage.through_sequence(),
        "terminal_event_id":coverage.terminal_event_id(),
        "coverage_root":coverage.coverage_root().to_hex(),
        "claim_range_root":claim_range_root.map(Digest256::to_hex),
        "prior_owners":prior_owner_json(prior_owners),
        "dependency_causes":dependency_causes.iter().map(|cause| json!({
            "event_sequence":cause.event_sequence,
            "event_id":cause.event_id,
            "command_id":cause.command_id,
            "changed_dependency_path":cause.changed_dependency_path.as_str(),
            "changed_dependency_sha256":cause.changed_dependency_sha256.to_hex(),
            "predecessor_owners":prior_owners_json(&cause.predecessor_owners),
            "current_owners":owner_keys_json(&cause.current_owners),
            "root":cause.root.to_hex(),
        })).collect::<Vec<_>>(),
        "claim_causes":claim_causes.iter().map(|cause| json!({
            "event_sequence":cause.event_sequence,
            "event_id":cause.event_id,
            "command_id":cause.command_id,
            "claim_id":cause.claim_id,
            "claim_stream_path":cause.claim_stream_path.as_str(),
            "claim_stream_sha256":cause.claim_stream_sha256.to_hex(),
            "receipt_path":cause.receipt_path.as_str(),
            "receipt_sha256":cause.receipt_sha256.to_hex(),
            "history_path":cause.history_path.as_str(),
            "history_sha256":cause.history_sha256.to_hex(),
            "revision_receipt_sha256":cause.revision_receipt_sha256.to_hex(),
            "predecessor_owners":prior_owners_json(&cause.predecessor_owners),
            "current_owners":owner_keys_json(&cause.current_owners),
            "root":cause.root.to_hex(),
        })).collect::<Vec<_>>(),
        "rows":rows.iter().map(|row| json!({
            "source_path":row.source_path.as_str(),
            "record_id":row.record_id,
            "predecessor_cohort_root":row.predecessor_cohort_root.map(Digest256::to_hex),
            "cause_root":row.cause_root.to_hex(),
            "causes":row.causes.iter().map(|cause| json!({
                "event_sequence":cause.event_sequence,
                "event_id":cause.event_id,
                "command_id":cause.command_id,
                "kind":cause.kind.as_str(),
                "path":cause.path.as_str(),
                "sha256":cause.sha256.to_hex(),
                "mutation_identity":cause.mutation_identity.as_ref().map(mutation_identity_json),
                "cause_root":cause.cause_root.to_hex(),
            })).collect::<Vec<_>>(),
            "cohort_input_root":row.cohort.input_root().to_hex(),
            "cohort_output_root":row.cohort.output_root().to_hex(),
        })).collect::<Vec<_>>(),
        "removed_owners":prior_owner_json(removed_owners),
    }))
}

fn adjacent(source_path: &str, name: &str) -> Result<String> {
    let parent = source_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(Error::Invalid(
            "navigation Record range adjacent source path",
        ))?;
    checked_path(&format!("{parent}/{name}")).map(|path| path.as_str().to_owned())
}

fn checked_path(path: &str) -> Result<RelativePath> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES {
        return Err(Error::Invalid("navigation Record range path length"));
    }
    let parsed = RelativePath::parse(path)
        .map_err(|_| Error::Invalid("navigation Record range canonical path"))?;
    if parsed.as_str() != path {
        return Err(Error::Invalid("navigation Record range canonical path"));
    }
    Ok(parsed)
}

fn valid_record_id(value: &str) -> bool {
    value.starts_with("tos.")
        && value.len() <= MAX_ID_BYTES
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn strict_value(raw: &[u8], cap: usize) -> Result<Value> {
    if raw.is_empty() || raw.len() > cap {
        return Err(Error::Budget("navigation Record range exact byte limit"));
    }
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation Record range JSON limits"))?;
    let parsed = tos_foundation::parse_json(raw, tos_foundation::JsonMode::PublishedStrict, limits)
        .map_err(|_| Error::Invalid("navigation Record range strict JSON"))?;
    let normalized = tos_foundation::emit_value_preserved_json(parsed.root(), limits)
        .map_err(|_| Error::Budget("navigation Record range normalized JSON"))?;
    serde_json::from_slice(&normalized)
        .map_err(|_| Error::Invalid("navigation Record range JSON value"))
}

fn receipt_file(receipt: &Value, path: &str, raw: &[u8]) -> Result<()> {
    let basename = path
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("navigation Record range receipt basename"))?;
    let binding = receipt
        .get("files")
        .and_then(|files| files.get(basename))
        .ok_or(Error::Invalid(
            "navigation Record range receipt member absent",
        ))?;
    let digest = format!("sha256:{}", Digest256::of_bytes(raw).to_hex());
    if binding.get("sha256").and_then(Value::as_str) != Some(digest.as_str())
        || binding.get("bytes").and_then(Value::as_u64) != Some(raw.len() as u64)
    {
        return Err(Error::Invalid(
            "navigation Record range receipt member differs",
        ));
    }
    Ok(())
}

fn select_event_by_receipt(raw: &[u8], receipt_path: &str) -> Result<Value> {
    let mut selected = None;
    for (index, line) in raw.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            if index + 1 == raw.split(|byte| *byte == b'\n').count() {
                continue;
            }
            return Err(Error::Invalid(
                "navigation Record range blank provenance line",
            ));
        }
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let event = strict_value(line, MAX_RANGE_BYTES)?;
        if event
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            == Some(receipt_path)
            && selected.replace(event).is_some()
        {
            return Err(Error::Invalid(
                "navigation Record range create provenance ambiguous",
            ));
        }
    }
    selected.ok_or(Error::Invalid(
        "navigation Record range create provenance absent",
    ))
}

fn event_outputs_match(event: &Value, path: &str, raw: &[u8]) -> Result<bool> {
    let outputs = event
        .pointer("/entities/outputs")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid(
            "navigation Record range create outputs absent",
        ))?;
    let matched = outputs
        .iter()
        .filter(|output| output.get("entity_ref").and_then(Value::as_str) == Some(path))
        .collect::<Vec<_>>();
    Ok(matched.len() == 1
        && matched[0].get("sha256").and_then(Value::as_str)
            == Some(Digest256::of_bytes(raw).to_hex().as_str())
        && matched[0].get("size_bytes").and_then(Value::as_u64) == Some(raw.len() as u64))
}

fn cause_digest(value: &Value) -> Result<Digest256> {
    Ok(Digest256::of_bytes(&canonical_value_bytes(value)?))
}

fn canonical_manifest_bytes(value: &Value) -> Result<Vec<u8>> {
    canonical_value_bytes(value)
}

fn canonical_value_bytes(value: &Value) -> Result<Vec<u8>> {
    let limits = JsonLimits::new(MAX_RANGE_BYTES, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("navigation Record range canonical limits"))?;
    let raw = serde_json::to_vec(value)
        .map_err(|_| Error::Invalid("navigation Record range canonical JSON"))?;
    let typed = tos_foundation::parse_json(&raw, tos_foundation::JsonMode::PublishedStrict, limits)
        .map_err(|_| Error::Invalid("navigation Record range canonical JSON"))?;
    let canonical = canonical_bytes_v1(
        &typed.into_root(),
        CanonicalProfile::CorpusSnapshotV1,
        limits,
    )
    .map_err(|_| Error::Budget("navigation Record range canonical bytes"))?;
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_navigation_event::{
        NavigationEventRangeCoverageInputV1, NavigationEventRangeEventV1,
        NavigationEventRangeMemberV1, seal_navigation_event_range_coverage_v1,
    };

    fn creation_coverage(files: &[(&str, &[u8], bool)]) -> NavigationEventRangeCoverageV1 {
        let mut members = files
            .iter()
            .map(|(path, raw, is_source)| NavigationEventRangeMemberV1 {
                source_path: (*path).into(),
                revision: 1,
                is_source: *is_source,
                size_bytes: raw.len() as u64,
                sha256: Digest256::of_bytes(raw),
            })
            .collect::<Vec<_>>();
        members.sort_by(|a, b| a.source_path.cmp(&b.source_path));
        let event = NavigationEventRangeEventV1 {
            sequence: 1,
            event_id: "event-1".into(),
            command_id: Some("command-1".into()),
            event_kind: "command".into(),
            member_root: Digest256::of_bytes(b"creation-members"),
            members,
        };
        let raw = canonical_value_bytes(&json!({
            "schema_version":"tos_source_navigation_event_range_coverage_v1",
            "cursor_before":0,"through_sequence":1,"terminal_event_id":"event-1",
            "events":[{
                "sequence":1,"event_id":"event-1","command_id":"command-1",
                "event_kind":"command","member_root":event.member_root.to_hex(),
                "members":event.members.iter().map(|member| json!({
                    "source_path":member.source_path,"revision":member.revision,
                    "is_source":member.is_source,"size_bytes":member.size_bytes,
                    "sha256":member.sha256.to_hex(),
                })).collect::<Vec<_>>(),
            }],
        }))
        .unwrap();
        seal_navigation_event_range_coverage_v1(NavigationEventRangeCoverageInputV1 {
            cursor_before: 0,
            through_sequence: 1,
            terminal_event_id: "event-1",
            events: &[event],
            coverage_root: Digest256::of_bytes(&raw),
            manifest_raw: &raw,
        })
        .unwrap()
    }

    #[test]
    fn creation_receipt_preserves_subject_identity_and_exact_source_members() {
        let source_path = "ToS/source-witnesses/agents/range-created/agent.json";
        let receipt_path = adjacent(source_path, "source-create-receipt.json").unwrap();
        let event_path = adjacent(source_path, "source-create-provenance.jsonl").unwrap();
        let record = json!({"record_id":"tos.agent.range-created","record_version":1,
            "preferred_label":"Création"});
        let mut raw = serde_json::to_vec_pretty(&record).unwrap();
        raw.push(b'\n');
        let canonical_digest = record_digest(&record, MAX_RANGE_BYTES).unwrap();
        assert_ne!(canonical_digest, Digest256::of_bytes(&raw).to_hex());
        let event = serde_json::to_vec(&json!({
            "record_binding":{"manifest_ref":receipt_path,"digest_scope":"exact_event_record_bytes"},
            "entities":{"outputs":[{"entity_ref":source_path,
                "sha256":Digest256::of_bytes(&raw).to_hex(),"size_bytes":raw.len()}]},
        })).unwrap();
        let file_binding = |bytes: &[u8]| {
            json!({
                "sha256":format!("sha256:{}",Digest256::of_bytes(bytes).to_hex()),"bytes":bytes.len(),
            })
        };
        let receipt = json!({
            "schema_version":"tos_local_source_create_receipt_v1","command_id":"command-1",
            "source_path":source_path,"source":{"id":"tos.agent.range-created","version":1,
                "digest":format!("sha256:{canonical_digest}")},
            "files":{"agent.json":file_binding(&raw),"source-create-provenance.jsonl":file_binding(&event)},
        });
        let owner = owner_key(source_path, "tos.agent.range-created");
        let check = |receipt: &Value, source_flag, receipt_flag, supplied: &[u8]| {
            let receipt_raw = serde_json::to_vec(receipt).unwrap();
            let coverage = creation_coverage(&[
                (source_path, &raw, source_flag),
                (&receipt_path, &receipt_raw, receipt_flag),
                (&event_path, &event, true),
            ]);
            seal_record_mutation_cause(
                &NavigationRecordEventRangeMutationCauseInputV1 {
                    event_sequence: 1,
                    event_id: "event-1",
                    command_id: "command-1",
                    kind: NavigationRecordEventRangeMutationKindInputV1::DirectCreate {
                        source_path,
                        source_bytes: supplied,
                        receipt_path: &receipt_path,
                        receipt_bytes: &receipt_raw,
                        provenance_event_path: &event_path,
                        provenance_event_bytes: &event,
                        origin_forms_path: None,
                        origin_forms_bytes: None,
                    },
                },
                &owner,
                &coverage,
                None,
            )
        };
        for receipt_flag in [true, false] {
            assert!(check(&receipt, true, receipt_flag, &raw).is_ok());
        }
        assert!(check(&receipt, false, true, &raw).is_err());
        let mut changed_bytes = raw.clone();
        changed_bytes.push(b' ');
        assert!(check(&receipt, true, true, &changed_bytes).is_err());
        let mut wrong_subject = receipt.clone();
        wrong_subject["source"]["digest"] =
            json!(format!("sha256:{}", Digest256::of_bytes(&raw).to_hex()));
        assert!(check(&wrong_subject, true, true, &raw).is_err());
        let mut wrong_file = receipt.clone();
        wrong_file["files"]["agent.json"]["bytes"] = json!(raw.len() + 1);
        assert!(check(&wrong_file, true, true, &raw).is_err());
    }
}
