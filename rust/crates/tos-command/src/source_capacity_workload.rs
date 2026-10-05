//! Deterministic supplied workload data, not a source namespace or admission grant.
//!
//! The protected creator authenticates templates and maps these data IDs to its
//! selected namespace, resolves contributors, and executes the maintained kernels.
//! Counts here concern emitted inputs, never Navigation rows or admitted records.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fmt::Write;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write as IoWrite};
use std::mem::size_of;
use std::os::unix::fs::{DirBuilderExt, FileExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};

const STRATA: usize = 64;
const ID_BYTES: usize = 128;
const SCALAR_COMPONENT_BYTES: u128 = 78;
const ROW_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityWorkloadError {
    InvalidProfile(&'static str),
    Budget(&'static str),
    Allocation,
    Cancelled,
    Deadline,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn profile() -> CapacityWorkloadProfile {
        CapacityWorkloadProfile {
            seed: Digest256::of_bytes(b"declared synthetic fixture, not an admission"),
            strata: vec![CapacityStratum {
                template_key: RelativePath::parse("fixtures/selected-template.json").unwrap(),
                template_sha256: Digest256::of_bytes(b"owner must authenticate the template"),
                record_count: 2,
                evidence_count: 2,
                claim_count: 2,
                revision_count: 4,
                text_fragment:
                    "Declared synthetic workload uses a supplied representative passage.".into(),
                text_mode: CapacityTextMode::Repeat,
                min_content_chars: 16,
                min_text_bytes: 80,
                max_text_bytes: 120,
                min_evidence_fanout: 1,
                max_evidence_fanout: 2,
                history_depth: 2,
                skew: CapacitySkew::Hub {
                    hub_count: 1,
                    period: 2,
                },
            }],
            limits: CapacityWorkloadLimits {
                max_items: 10,
                max_item_bytes: 8192,
                max_text_bytes: 4096,
                max_ref_bytes: 1024,
                max_refs_per_item: 2,
                max_state_bytes: 65536,
                max_emitted_bytes: 65536,
            },
        }
    }

    #[test]
    fn supplied_nonempty_inputs_have_deterministic_counts_refs_and_history() {
        let mut profile = profile();
        profile.strata[0].max_text_bytes = profile.strata[0].min_text_bytes;
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        profile.limits.max_emitted_bytes =
            CapacityWorkloadIter::new(&profile, deadline, &cancelled)
                .unwrap()
                .receipt()
                .bill
                .maximum_component_bytes;
        let mut first = CapacityWorkloadIter::new(&profile, deadline, &cancelled).unwrap();
        let mut second = CapacityWorkloadIter::new(&profile, deadline, &cancelled).unwrap();
        let mut evidence = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        let mut components = 0;
        let mut versions = Vec::new();
        while let Some(row) = first.next_item().unwrap() {
            let paired = second.next_item().unwrap().unwrap();
            assert_eq!(row.stable_id, paired.stable_id);
            assert_eq!(row.substitutions.text, paired.substitutions.text);
            assert!(!row.substitutions.text.trim().is_empty());
            assert!(ids.insert(row.stable_id.clone()));
            components += row.component_bytes;
            if row.kind == CapacityWorkloadKind::Evidence {
                evidence.push(row.stable_id);
            }
            if row.kind == CapacityWorkloadKind::Claim {
                assert!(
                    row.substitutions
                        .evidence_refs
                        .iter()
                        .all(|r| evidence.contains(r))
                );
            }
            if row.kind == CapacityWorkloadKind::Revision {
                versions.push((
                    row.substitutions.record_target.unwrap(),
                    row.substitutions.version,
                ));
            }
        }
        assert!(second.next_item().unwrap().is_none());
        let receipt = first.receipt();
        assert!(receipt.coverage_complete);
        assert_eq!(receipt.counts[0].emitted, [2, 2, 2, 4]);
        assert_eq!(receipt.emitted_operations, 10);
        assert_eq!(receipt.emitted_component_bytes, components);
        assert!(
            components >= receipt.bill.minimum_component_bytes
                && components <= receipt.bill.maximum_component_bytes
        );
        assert_eq!(
            receipt.transcript_sha256,
            second.receipt().transcript_sha256
        );
        assert_eq!(versions[0].0, versions[2].0);
        assert_eq!(versions[1].0, versions[3].0);
        assert_eq!(
            versions.iter().map(|v| v.1).collect::<Vec<_>>(),
            [2, 2, 3, 3]
        );
        assert!(!receipt.bill.physical_fit_established);
    }

    #[test]
    fn supplied_fragment_mix_preserves_lengths_diversity_and_bank_binding() {
        let mut profile = profile();
        let s = &mut profile.strata[0];
        s.record_count = 10;
        s.evidence_count = 0;
        s.claim_count = 0;
        s.revision_count = 0;
        s.min_evidence_fanout = 0;
        s.max_evidence_fanout = 0;
        s.skew = CapacitySkew::Uniform;
        s.max_text_bytes = s.min_text_bytes;
        s.text_mode = CapacityTextMode::SeededFragmentMix {
            fragments: vec![
                "Selected synthetic evidence distinguishes the cited source and its scope.".into(),
                "Declared synthetic review preserves uncertainty and independent provenance."
                    .into(),
            ],
        };
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut first = CapacityWorkloadIter::new(&profile, deadline, &cancelled).unwrap();
        let mut second = CapacityWorkloadIter::new(&profile, deadline, &cancelled).unwrap();
        let mut low = None;
        while let Some(row) = first.next_item().unwrap() {
            let paired = second.next_item().unwrap().unwrap();
            assert_eq!(row.substitutions.text, paired.substitutions.text);
            assert_eq!(row.substitutions.text.len(), 80);
            if row.ordinal == 0 {
                low = Some(row.substitutions.text.clone());
            }
            if row.ordinal == 2 || row.ordinal == 8 {
                assert_ne!(Some(&row.substitutions.text), low.as_ref());
            }
        }
        assert!(second.next_item().unwrap().is_none());
        let receipt = first.receipt();
        assert!(receipt.coverage_complete);
        assert_eq!(receipt.counts[0].text_modes, [2, 6, 2]);
        assert_eq!(receipt.counts[0].fragment_bank_count, 2);
        assert!(receipt.counts[0].fragment_bank_sha256.is_some());
        assert_eq!(
            receipt.transcript_sha256,
            second.receipt().transcript_sha256
        );
        if let CapacityTextMode::SeededFragmentMix { fragments } = &mut profile.strata[0].text_mode
        {
            fragments[0] = "                     ".into();
        }
        assert!(CapacityWorkloadIter::new(&profile, deadline, &cancelled).is_err());
    }

    #[test]
    fn unavailable_evidence_and_whitespace_are_not_valid_inputs() {
        let mut profile = profile();
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        profile.strata[0].evidence_count = 1;
        assert!(CapacityWorkloadIter::new(&profile, deadline, &cancelled).is_err());
        profile.strata[0].evidence_count = 2;
        profile.strata[0].text_fragment = " \n\t".into();
        assert!(CapacityWorkloadIter::new(&profile, deadline, &cancelled).is_err());
    }

    #[test]
    fn budget_or_cancellation_cannot_mint_complete_generation() {
        let mut profile = profile();
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        profile.limits.max_emitted_bytes = 1;
        assert!(CapacityWorkloadIter::new(&profile, deadline, &cancelled).is_err());
        profile.limits.max_emitted_bytes = 65536;
        let mut iter = CapacityWorkloadIter::new(&profile, deadline, &cancelled).unwrap();
        iter.next_item().unwrap().unwrap();
        cancelled.store(true, Ordering::Release);
        assert!(matches!(
            iter.next_item(),
            Err(CapacityWorkloadError::Cancelled)
        ));
        assert_eq!(iter.receipt().emitted_operations, 1);
        assert!(!iter.receipt().coverage_complete);
        cancelled.store(false, Ordering::Release);
        assert!(matches!(
            iter.next_item(),
            Err(CapacityWorkloadError::Failed)
        ));
    }

    #[test]
    fn profile_receipt_and_requested_item_share_one_state_budget() {
        let mut profile = profile();
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        let bill = CapacityWorkloadIter::new(&profile, deadline, &cancelled)
            .unwrap()
            .receipt()
            .bill;
        profile.limits.max_state_bytes = bill.borrowed_profile_bytes
            + bill.iterator_state_bytes
            + bill.maximum_requested_item_allocation_bytes
            - 1;
        assert!(matches!(
            CapacityWorkloadIter::new(&profile, deadline, &cancelled),
            Err(CapacityWorkloadError::Budget(
                "profile/iterator/receipt/item overlap"
            ))
        ));
    }
}
type Result<T> = std::result::Result<T, CapacityWorkloadError>;

#[derive(Clone, Copy, Debug)]
pub struct CapacityWorkloadLimits {
    pub max_items: u64,
    pub max_item_bytes: usize,
    pub max_text_bytes: usize,
    pub max_ref_bytes: usize,
    pub max_refs_per_item: usize,
    pub max_state_bytes: usize,
    pub max_emitted_bytes: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum CapacitySkew {
    Uniform,
    /// Every `period`th Claim starts its evidence window within `hub_count` IDs.
    Hub {
        hub_count: u64,
        period: u64,
    },
}

/// Declared synthetic diversity, not measured entropy or compression credit.
pub enum CapacityTextMode {
    Repeat,
    /// Per kind, ordinal mod 10 selects 20% Repeat, 60% alternating supplied
    /// passage/bank fragments, and 20% full bank mixing. Owner supplies content.
    SeededFragmentMix {
        fragments: Vec<String>,
    },
}

pub struct CapacityStratum {
    pub template_key: RelativePath,
    pub template_sha256: Digest256,
    pub record_count: u64,
    pub claim_count: u64,
    pub evidence_count: u64,
    pub revision_count: u64,
    /// Supplied representative material. This module does not judge its semantics.
    pub text_fragment: String,
    pub text_mode: CapacityTextMode,
    pub min_content_chars: usize,
    pub min_text_bytes: usize,
    pub max_text_bytes: usize,
    pub min_evidence_fanout: usize,
    pub max_evidence_fanout: usize,
    pub history_depth: u32,
    pub skew: CapacitySkew,
}

pub struct CapacityWorkloadProfile {
    pub seed: Digest256,
    pub strata: Vec<CapacityStratum>,
    pub limits: CapacityWorkloadLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityWorkloadKind {
    Record,
    Evidence,
    Claim,
    Revision,
}
impl CapacityWorkloadKind {
    fn index(self) -> usize {
        match self {
            Self::Record => 0,
            Self::Evidence => 1,
            Self::Claim => 2,
            Self::Revision => 3,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Evidence => "evidence",
            Self::Claim => "claim",
            Self::Revision => "revision",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SyntheticProvenance {
    pub seed: Digest256,
    pub stratum: u8,
    pub ordinal: u64,
    pub template_sha256: Digest256,
}

pub struct CapacityWorkloadSubstitutions {
    pub text: String,
    pub record_target: Option<String>,
    /// Stable data IDs only. The owner must resolve and grant every real ref.
    pub evidence_refs: Vec<String>,
    pub version: u32,
    pub provenance: SyntheticProvenance,
}

pub struct CapacityWorkloadItem<'a> {
    pub stratum: u8,
    pub ordinal: u64,
    pub kind: CapacityWorkloadKind,
    pub stable_id: String,
    pub template_key: &'a RelativePath,
    pub template_sha256: Digest256,
    pub substitutions: CapacityWorkloadSubstitutions,
    pub component_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CapacityStratumCounts {
    pub emitted: [u64; 4],
    /// Actual Repeat / mixed-template / full-bank counts, independent of kind.
    pub text_modes: [u64; 3],
    pub fragment_bank_count: usize,
    pub fragment_bank_sha256: Option<Digest256>,
    pub text_bytes: u64,
    pub ref_bytes: u64,
    pub component_bytes: u64,
    pub min_text_bytes: usize,
    pub max_text_bytes: usize,
    pub min_fanout: usize,
    pub max_fanout: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct CapacityWorkloadBill {
    pub declared_operations: [u64; 4],
    pub total_operations: u64,
    pub minimum_text_bytes: u64,
    pub maximum_text_bytes: u64,
    pub minimum_component_bytes: u64,
    pub maximum_component_bytes: u64,
    pub borrowed_profile_bytes: usize,
    pub iterator_state_bytes: usize,
    pub maximum_requested_item_allocation_bytes: usize,
    /// Not a physical fit claim; request encoding, stores, indexes, histories,
    /// stage, publication, restore and retained old/new versions belong to owners.
    pub physical_fit_established: bool,
}

pub struct CapacityWorkloadReceipt {
    pub seed: Digest256,
    pub strata_used: usize,
    pub counts: [CapacityStratumCounts; STRATA],
    pub emitted_operations: u64,
    pub emitted_component_bytes: u64,
    pub transcript_sha256: Digest256,
    pub coverage_complete: bool,
    pub bill: CapacityWorkloadBill,
}

pub struct CapacityWorkloadIter<'a> {
    profile: &'a CapacityWorkloadProfile,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    stratum: usize,
    phase: usize,
    ordinal: u64,
    counts: [CapacityStratumCounts; STRATA],
    emitted: u64,
    bytes: u64,
    transcript: Digest256Hasher,
    bill: CapacityWorkloadBill,
    complete: bool,
    failed: bool,
}

fn checked_u64(value: u128) -> Result<u64> {
    u64::try_from(value).map_err(|_| CapacityWorkloadError::Budget("u64 count/byte overflow"))
}
fn checked_usize(value: u128) -> Result<usize> {
    usize::try_from(value).map_err(|_| CapacityWorkloadError::Budget("allocation overflow"))
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(CapacityWorkloadError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(CapacityWorkloadError::Deadline);
    }
    Ok(())
}
fn declared(s: &CapacityStratum) -> [u64; 4] {
    [
        s.record_count,
        s.evidence_count,
        s.claim_count,
        s.revision_count,
    ]
}
fn data_id_len(kind: CapacityWorkloadKind) -> usize {
    14 + 64 + 1 + 2 + 1 + kind.name().len() + 1 + 20
}

fn allocation(text: usize, refs: usize, template: usize) -> Result<usize> {
    // Includes formatting reserves, optional target, Vec slots, and each owned ID.
    checked_usize(
        size_of::<CapacityWorkloadItem<'static>>() as u128
            + template as u128
            + text as u128
            + (3 * ID_BYTES) as u128
            + refs as u128 * (size_of::<String>() + 2 * ID_BYTES) as u128,
    )
}

impl<'a> CapacityWorkloadIter<'a> {
    pub fn new(
        profile: &'a CapacityWorkloadProfile,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        active(deadline, cancelled)?;
        let l = profile.limits;
        if profile.strata.is_empty() || profile.strata.len() > STRATA {
            return Err(CapacityWorkloadError::InvalidProfile(
                "requires 1..=64 strata",
            ));
        }
        if l.max_items == 0
            || l.max_item_bytes == 0
            || l.max_item_bytes > ROW_BYTES
            || l.max_text_bytes == 0
            || l.max_text_bytes > l.max_item_bytes
            || l.max_ref_bytes == 0
            || l.max_ref_bytes > l.max_item_bytes
            || l.max_refs_per_item == 0
            || l.max_refs_per_item > 128
            || l.max_state_bytes == 0
            || l.max_emitted_bytes == 0
        {
            return Err(CapacityWorkloadError::InvalidProfile(
                "finite limits/maintained row and evidence ceilings",
            ));
        }
        let mut bill = CapacityWorkloadBill {
            declared_operations: [0; 4],
            total_operations: 0,
            minimum_text_bytes: 0,
            maximum_text_bytes: 0,
            minimum_component_bytes: 0,
            maximum_component_bytes: 0,
            borrowed_profile_bytes: checked_usize(
                size_of::<CapacityWorkloadProfile>() as u128
                    + profile.strata.capacity() as u128 * size_of::<CapacityStratum>() as u128,
            )?,
            iterator_state_bytes: size_of::<Self>() + size_of::<CapacityWorkloadReceipt>(),
            maximum_requested_item_allocation_bytes: 0,
            physical_fit_established: false,
        };
        // Reject state and borrowed descriptor lengths before scanning any text.
        if bill.iterator_state_bytes > l.max_state_bytes {
            return Err(CapacityWorkloadError::Budget("fixed iterator state"));
        }
        let mut counts = [CapacityStratumCounts::default(); STRATA];
        for (stratum, s) in profile.strata.iter().enumerate() {
            active(deadline, cancelled)?;
            bill.borrowed_profile_bytes = checked_usize(
                bill.borrowed_profile_bytes as u128
                    + s.text_fragment.capacity() as u128
                    + s.template_key.as_str().len() as u128,
            )?;
            if bill.borrowed_profile_bytes as u128 + bill.iterator_state_bytes as u128
                > l.max_state_bytes as u128
                || s.text_fragment.len() > l.max_text_bytes
            {
                return Err(CapacityWorkloadError::Budget("borrowed profile/text bytes"));
            }
            if let CapacityTextMode::SeededFragmentMix { fragments } = &s.text_mode {
                // Bound the bank before traversal; then charge every borrowed
                // Vec/String capacity before inspecting or rendering content.
                if fragments.is_empty() || fragments.len() > 64 {
                    return Err(CapacityWorkloadError::InvalidProfile("fragment bank count"));
                }
                let mut bytes = 0u128;
                let mut capacities = fragments.capacity() as u128 * size_of::<String>() as u128;
                for fragment in fragments {
                    active(deadline, cancelled)?;
                    bytes += fragment.len() as u128;
                    capacities += fragment.capacity() as u128;
                    if fragment.is_empty()
                        || fragment.len() > 4096
                        || fragment.len() > s.min_text_bytes
                    {
                        return Err(CapacityWorkloadError::InvalidProfile(
                            "fragment bank entry length",
                        ));
                    }
                }
                bill.borrowed_profile_bytes =
                    checked_usize(bill.borrowed_profile_bytes as u128 + capacities)?;
                if bytes > 64 * 1024
                    || bill.borrowed_profile_bytes as u128 + bill.iterator_state_bytes as u128
                        > l.max_state_bytes as u128
                {
                    return Err(CapacityWorkloadError::Budget("borrowed fragment bank"));
                }
                let mut fingerprint = Digest256Hasher::new();
                fingerprint.update(b"supplied-fragment-bank-v1\0");
                fingerprint.update(&(fragments.len() as u64).to_le_bytes());
                for fragment in fragments {
                    let mut content = 0usize;
                    for (i, ch) in fragment.chars().enumerate() {
                        if i % 1024 == 0 {
                            active(deadline, cancelled)?;
                        }
                        if !ch.is_whitespace() {
                            content += 1;
                        }
                    }
                    if content < s.min_content_chars || s.min_content_chars == 0 {
                        return Err(CapacityWorkloadError::InvalidProfile(
                            "nonwhitespace fragment bank",
                        ));
                    }
                    fingerprint.update(&(fragment.len() as u64).to_le_bytes());
                    fingerprint.update(fragment.as_bytes());
                    active(deadline, cancelled)?;
                }
                counts[stratum].fragment_bank_count = fragments.len();
                counts[stratum].fragment_bank_sha256 = Some(fingerprint.finalize());
            }
            let mut content = 0usize;
            for (i, ch) in s.text_fragment.chars().enumerate() {
                if i % 1024 == 0 {
                    active(deadline, cancelled)?;
                }
                if !ch.is_whitespace() {
                    content += 1;
                }
            }
            if s.min_content_chars == 0
                || content < s.min_content_chars
                || s.min_text_bytes < s.text_fragment.len()
                || s.min_text_bytes == 0
                || s.max_text_bytes < s.min_text_bytes
                || s.max_text_bytes > l.max_text_bytes
            {
                return Err(CapacityWorkloadError::InvalidProfile(
                    "nonwhitespace supplied content/text range",
                ));
            }
            if s.max_evidence_fanout < s.min_evidence_fanout
                || s.max_evidence_fanout > l.max_refs_per_item
                || s.max_evidence_fanout as u128 > s.evidence_count as u128
                || (s.claim_count > 0 && (s.record_count == 0 || s.min_evidence_fanout == 0))
            {
                return Err(CapacityWorkloadError::InvalidProfile(
                    "Claim target and available evidence fanout",
                ));
            }
            if s.history_depth == u32::MAX
                || s.revision_count > 0 && (s.record_count == 0 || s.history_depth == 0)
                || s.revision_count as u128 > s.record_count as u128 * s.history_depth as u128
            {
                return Err(CapacityWorkloadError::InvalidProfile(
                    "revision target/depth",
                ));
            }
            if let CapacitySkew::Hub { hub_count, period } = s.skew {
                if hub_count == 0 || hub_count > s.evidence_count || period == 0 {
                    return Err(CapacityWorkloadError::InvalidProfile(
                        "declared hub schedule",
                    ));
                }
            }
            let item = allocation(
                s.max_text_bytes,
                s.max_evidence_fanout,
                s.template_key.as_str().len(),
            )?;
            if item > l.max_item_bytes
                || s.max_evidence_fanout as u128 * ID_BYTES as u128 > l.max_ref_bytes as u128
            {
                return Err(CapacityWorkloadError::Budget(
                    "per-item requested allocation/ref bytes",
                ));
            }
            bill.maximum_requested_item_allocation_bytes =
                bill.maximum_requested_item_allocation_bytes.max(item);
            let counts = declared(s);
            let mut stratum_total = 0u64;
            for (i, count) in counts.into_iter().enumerate() {
                bill.declared_operations[i] =
                    checked_u64(bill.declared_operations[i] as u128 + count as u128)?;
                stratum_total = checked_u64(stratum_total as u128 + count as u128)?;
            }
            if stratum_total == 0 {
                return Err(CapacityWorkloadError::InvalidProfile("empty stratum"));
            }
            for (kind, count) in [
                CapacityWorkloadKind::Record,
                CapacityWorkloadKind::Evidence,
                CapacityWorkloadKind::Claim,
                CapacityWorkloadKind::Revision,
            ]
            .into_iter()
            .zip(counts)
            {
                let id = data_id_len(kind) as u128;
                let target = if matches!(
                    kind,
                    CapacityWorkloadKind::Claim | CapacityWorkloadKind::Revision
                ) {
                    data_id_len(CapacityWorkloadKind::Record) as u128
                } else {
                    0
                };
                let prefix =
                    id + target + s.template_key.as_str().len() as u128 + SCALAR_COMPONENT_BYTES;
                let min_refs = if kind == CapacityWorkloadKind::Claim {
                    s.min_evidence_fanout as u128
                        * data_id_len(CapacityWorkloadKind::Evidence) as u128
                } else {
                    0
                };
                let max_refs = if kind == CapacityWorkloadKind::Claim {
                    s.max_evidence_fanout as u128
                        * data_id_len(CapacityWorkloadKind::Evidence) as u128
                } else {
                    0
                };
                bill.minimum_component_bytes = checked_u64(
                    bill.minimum_component_bytes as u128
                        + count as u128 * (prefix + s.min_text_bytes as u128 + min_refs),
                )?;
                bill.maximum_component_bytes = checked_u64(
                    bill.maximum_component_bytes as u128
                        + count as u128 * (prefix + s.max_text_bytes as u128 + max_refs),
                )?;
            }
            bill.total_operations =
                checked_u64(bill.total_operations as u128 + stratum_total as u128)?;
            bill.minimum_text_bytes = checked_u64(
                bill.minimum_text_bytes as u128 + stratum_total as u128 * s.min_text_bytes as u128,
            )?;
            bill.maximum_text_bytes = checked_u64(
                bill.maximum_text_bytes as u128 + stratum_total as u128 * s.max_text_bytes as u128,
            )?;
        }
        if bill.borrowed_profile_bytes as u128
            + bill.iterator_state_bytes as u128
            + bill.maximum_requested_item_allocation_bytes as u128
            > l.max_state_bytes as u128
        {
            return Err(CapacityWorkloadError::Budget(
                "profile/iterator/receipt/item overlap",
            ));
        }
        if bill.total_operations > l.max_items || bill.minimum_component_bytes > l.max_emitted_bytes
        {
            return Err(CapacityWorkloadError::Budget(
                "declared item/minimum text bill",
            ));
        }
        active(deadline, cancelled)?;
        let mut transcript = Digest256Hasher::new();
        transcript.update(b"supplied-synthetic-capacity-input-v1\0");
        transcript.update(profile.seed.as_bytes());
        Ok(Self {
            profile,
            deadline,
            cancelled,
            stratum: 0,
            phase: 0,
            ordinal: 0,
            counts,
            emitted: 0,
            bytes: 0,
            transcript,
            bill,
            complete: false,
            failed: false,
        })
    }

    fn sample(&self, ordinal: u64, tag: u8) -> u64 {
        let mut h = Digest256Hasher::new();
        h.update(self.profile.seed.as_bytes());
        h.update(&[self.stratum as u8, self.phase as u8, tag]);
        h.update(&ordinal.to_le_bytes());
        let digest = h.finalize();
        u64::from_le_bytes(
            digest.as_bytes()[..8]
                .try_into()
                .expect("fixed digest width"),
        )
    }
    fn id(&self, kind: CapacityWorkloadKind, ordinal: u64) -> Result<String> {
        let mut id = String::new();
        id.try_reserve_exact(ID_BYTES)
            .map_err(|_| CapacityWorkloadError::Allocation)?;
        write!(
            &mut id,
            "capacity-data:{}:{:02}:{}:{:020}",
            self.profile.seed.to_hex(),
            self.stratum,
            kind.name(),
            ordinal
        )
        .map_err(|_| CapacityWorkloadError::Allocation)?;
        if id.len() != data_id_len(kind) || id.len() > ID_BYTES || id.capacity() > ID_BYTES {
            return Err(CapacityWorkloadError::Budget("stable data ID allocation"));
        }
        Ok(id)
    }

    /// Caller consumes/drops each bounded input before requesting the next one.
    /// On any error this iterator is permanently failed, with no complete receipt.
    pub fn next_item(&mut self) -> Result<Option<CapacityWorkloadItem<'a>>> {
        if self.failed {
            return Err(CapacityWorkloadError::Failed);
        }
        let result = self.next_inner();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn next_inner(&mut self) -> Result<Option<CapacityWorkloadItem<'a>>> {
        active(self.deadline, self.cancelled)?;
        while self.stratum < self.profile.strata.len() {
            let s = &self.profile.strata[self.stratum];
            if self.phase == 4 {
                self.stratum += 1;
                self.phase = 0;
                self.ordinal = 0;
                continue;
            }
            if self.ordinal == declared(s)[self.phase] {
                self.phase += 1;
                self.ordinal = 0;
                continue;
            }
            break;
        }
        if self.stratum == self.profile.strata.len() {
            self.complete = true;
            active(self.deadline, self.cancelled)?;
            return Ok(None);
        }
        let s = &self.profile.strata[self.stratum];
        let kind = [
            CapacityWorkloadKind::Record,
            CapacityWorkloadKind::Evidence,
            CapacityWorkloadKind::Claim,
            CapacityWorkloadKind::Revision,
        ][self.phase];
        let text_len = s.min_text_bytes
            + (self.sample(self.ordinal, 0) as u128
                % (s.max_text_bytes - s.min_text_bytes + 1) as u128) as usize;
        let fanout = if kind == CapacityWorkloadKind::Claim {
            s.min_evidence_fanout
                + (self.sample(self.ordinal, 1) as u128
                    % (s.max_evidence_fanout - s.min_evidence_fanout + 1) as u128)
                    as usize
        } else {
            0
        };
        let requested = allocation(text_len, fanout, s.template_key.as_str().len())?;
        let l = self.profile.limits;
        let component_upper = checked_u64(
            text_len as u128
                + data_id_len(kind) as u128
                + if matches!(
                    kind,
                    CapacityWorkloadKind::Claim | CapacityWorkloadKind::Revision
                ) {
                    data_id_len(CapacityWorkloadKind::Record) as u128
                } else {
                    0
                }
                + s.template_key.as_str().len() as u128
                + fanout as u128 * data_id_len(CapacityWorkloadKind::Evidence) as u128
                + SCALAR_COMPONENT_BYTES,
        )?;
        if self.bill.borrowed_profile_bytes as u128
            + self.bill.iterator_state_bytes as u128
            + requested as u128
            > l.max_state_bytes as u128
            || requested > l.max_item_bytes
            || fanout as u128 * ID_BYTES as u128 > l.max_ref_bytes as u128
            || self.bytes as u128 + component_upper as u128 > l.max_emitted_bytes as u128
        {
            return Err(CapacityWorkloadError::Budget(
                "precharged item/emitted bytes",
            ));
        }
        active(self.deadline, self.cancelled)?;
        let mut text = String::new();
        text.try_reserve_exact(text_len)
            .map_err(|_| CapacityWorkloadError::Allocation)?;
        let mode = match &s.text_mode {
            CapacityTextMode::Repeat => 0,
            CapacityTextMode::SeededFragmentMix { .. } => match self.ordinal % 10 {
                0..=1 => 0,
                2..=7 => 1,
                _ => 2,
            },
        };
        let mut chunk = 0u64;
        while text.len() < text_len {
            active(self.deadline, self.cancelled)?;
            let fragment = match &s.text_mode {
                CapacityTextMode::SeededFragmentMix { fragments }
                    if mode == 2 || mode == 1 && chunk % 2 == 1 =>
                {
                    let mut h = Digest256Hasher::new();
                    h.update(b"supplied-fragment-choice-v1\0");
                    h.update(self.profile.seed.as_bytes());
                    h.update(&[self.stratum as u8, self.phase as u8]);
                    h.update(&self.ordinal.to_le_bytes());
                    h.update(&chunk.to_le_bytes());
                    let digest = h.finalize();
                    let choice = u64::from_le_bytes(
                        digest.as_bytes()[..8]
                            .try_into()
                            .expect("fixed digest width"),
                    );
                    &fragments[(choice % fragments.len() as u64) as usize]
                }
                _ => &s.text_fragment,
            };
            let remaining = text_len - text.len();
            if fragment.len() <= remaining {
                text.push_str(fragment);
            } else {
                for (i, ch) in fragment.chars().enumerate() {
                    if i % 1024 == 0 {
                        active(self.deadline, self.cancelled)?;
                    }
                    if ch.len_utf8() > text_len - text.len() {
                        break;
                    }
                    text.push(ch);
                }
                break;
            }
            chunk = chunk
                .checked_add(1)
                .ok_or(CapacityWorkloadError::Budget("text chunk count"))?;
        }
        while text.len() < text_len {
            active(self.deadline, self.cancelled)?;
            text.push(' ');
        }
        let stable_id = self.id(kind, self.ordinal)?;
        let target = if matches!(
            kind,
            CapacityWorkloadKind::Claim | CapacityWorkloadKind::Revision
        ) {
            Some(self.id(CapacityWorkloadKind::Record, self.ordinal % s.record_count)?)
        } else {
            None
        };
        let version = if kind == CapacityWorkloadKind::Revision {
            u32::try_from(2u128 + self.ordinal as u128 / s.record_count as u128)
                .map_err(|_| CapacityWorkloadError::Budget("revision version"))?
        } else {
            1
        };
        let mut refs = Vec::new();
        refs.try_reserve_exact(fanout)
            .map_err(|_| CapacityWorkloadError::Allocation)?;
        if fanout > 0 {
            let modulus = match s.skew {
                CapacitySkew::Hub { hub_count, period } if self.ordinal % period == 0 => hub_count,
                _ => s.evidence_count,
            };
            let first = self.sample(self.ordinal, 2) % modulus;
            for i in 0..fanout {
                active(self.deadline, self.cancelled)?;
                let ordinal = ((first as u128 + i as u128) % s.evidence_count as u128) as u64;
                refs.push(self.id(CapacityWorkloadKind::Evidence, ordinal)?);
            }
        }
        let ref_bytes = checked_u64(refs.iter().map(|r| r.len() as u128).sum())?;
        let component_bytes = checked_u64(
            text.len() as u128
                + stable_id.len() as u128
                + target.as_ref().map_or(0, |r| r.len()) as u128
                + ref_bytes as u128
                + s.template_key.as_str().len() as u128
                + SCALAR_COMPONENT_BYTES,
        )?;
        active(self.deadline, self.cancelled)?;
        let mut row = self.counts[self.stratum];
        let previous = row.emitted.iter().any(|n| *n > 0);
        row.emitted[kind.index()] += 1;
        row.text_modes[mode] += 1;
        row.text_bytes = checked_u64(row.text_bytes as u128 + text.len() as u128)?;
        row.ref_bytes = checked_u64(row.ref_bytes as u128 + ref_bytes as u128)?;
        row.component_bytes = checked_u64(row.component_bytes as u128 + component_bytes as u128)?;
        row.min_text_bytes = if previous {
            row.min_text_bytes.min(text.len())
        } else {
            text.len()
        };
        row.max_text_bytes = row.max_text_bytes.max(text.len());
        if kind == CapacityWorkloadKind::Claim {
            row.min_fanout = if row.emitted[kind.index()] == 1 {
                fanout
            } else {
                row.min_fanout.min(fanout)
            };
            row.max_fanout = row.max_fanout.max(fanout);
        }
        let mut transcript = self.transcript.clone();
        transcript.update(&[self.stratum as u8, kind.index() as u8]);
        transcript.update(&self.ordinal.to_le_bytes());
        for bytes in [
            stable_id.as_bytes(),
            s.template_key.as_str().as_bytes(),
            text.as_bytes(),
            target.as_ref().map_or(&[][..], |t| t.as_bytes()),
        ] {
            transcript.update(&(bytes.len() as u64).to_le_bytes());
            transcript.update(bytes);
        }
        transcript.update(s.template_sha256.as_bytes());
        transcript.update(&version.to_le_bytes());
        transcript.update(&(refs.len() as u64).to_le_bytes());
        for r in &refs {
            transcript.update(&(r.len() as u64).to_le_bytes());
            transcript.update(r.as_bytes());
        }
        active(self.deadline, self.cancelled)?;
        self.transcript = transcript;
        self.counts[self.stratum] = row;
        self.bytes = checked_u64(self.bytes as u128 + component_bytes as u128)?;
        self.emitted += 1;
        let ordinal = self.ordinal;
        self.ordinal += 1;
        Ok(Some(CapacityWorkloadItem {
            stratum: self.stratum as u8,
            ordinal,
            kind,
            stable_id,
            template_key: &s.template_key,
            template_sha256: s.template_sha256,
            component_bytes,
            substitutions: CapacityWorkloadSubstitutions {
                text,
                record_target: target,
                evidence_refs: refs,
                version,
                provenance: SyntheticProvenance {
                    seed: self.profile.seed,
                    stratum: self.stratum as u8,
                    ordinal,
                    template_sha256: s.template_sha256,
                },
            },
        }))
    }

    /// Generated UTF-8/component counts are not encoded creation-request bytes.
    /// The adapter observes requested/returned/admitted/serialized counts separately.
    pub fn receipt(&self) -> CapacityWorkloadReceipt {
        CapacityWorkloadReceipt {
            seed: self.profile.seed,
            strata_used: self.profile.strata.len(),
            counts: self.counts,
            emitted_operations: self.emitted,
            emitted_component_bytes: self.bytes,
            transcript_sha256: self.transcript.clone().finalize(),
            coverage_complete: self.complete && !self.failed,
            bill: self.bill,
        }
    }
}

// The generic iterator above is the historical source-capacity precursor. It
// remains available for its original mechanics tests, but it is not used as a
// source-record generator. The typed path below expands pinned owner shapes
// into a private fixture namespace and marks generated records as
// unreviewed/provisional where the owner schema permits it. Pinned provenance
// is not inherited authorship, review, rights, or canon authority.

use super::source_admission::{AdmissionWorkBudget, active as scale_active};
use super::source_admission_packed_objects::{
    MAX_PACKED_OBJECT_FRAMES_V2, PackedObjectBuildWorkV2, PackedObjectLimitsV2,
    PackedObjectSourceV2, PackedObjectWriterV2,
};
use super::source_admission_segment_v2::{
    MEMBERS_KIND, NativeV2TreeIo, OBJECT_EXTENTS_KIND, SOURCE_ADMISSION_V2_DOMAIN,
};
use tos_foundation::{CanonicalProfile, JsonLimits, JsonMode, canonical_bytes_v1, parse_json};
use tos_segment_store::{
    AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1, AuthenticatedTreeIoLedgerV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, SegmentLimits, SegmentStore,
};
use tos_source_store::{
    PinnedSqliteIoBudget, PinnedSqliteSpaceBudget, PinnedSqliteSpaceReservation,
};

pub const SCALE_TEMPLATE_SOURCE_COMMIT_V1: &str = "5ad427d376b04b54f2c630b0d3859d4abdf32003";
const SCALE_BASE_PROFILE_REF_V1: &str = ".scale-completion-r1/profile-candidate-r3.json";
const SCALE_BASE_PROFILE_SHA256_V1: &str =
    "067c8e7fce9ceb13508d18e34fbee79459f6670553056d68d3be2710c889d798";
pub const SCALE_PROFILE_REF_V1: &str = "scale-profile-v1.json";
pub const SCALE_INPUT_SCHEMA_V1: &str = "tos_native_scale_packed_input_v1";
pub const SCALE_INPUT_SOURCE_STATUS_V1: &str = "synthetic_private_fixture";
pub const SCALE_INPUT_MEMBERS_LEAF_V1: &str = "members.v2.tree";
pub const SCALE_INPUT_OBJECTS_LEAF_V1: &str = "objects.v2.tree";
pub const SCALE_INPUT_MANIFEST_LEAF_V1: &str = "scale-input-v1.json";
pub const SCALE_DEPENDENCY_CLOSURE_LEAF_V1: &str = "scale-dependency-closure-v1.json";
const SCALE_FIXTURE_ROOT_V1: &str = "ToS/source-witnesses";
const SCALE_MAX_TEMPLATE_BYTES_V1: usize = 262_144;
const SCALE_SORT_RUN_ROWS_V1: usize = 4096;
const SCALE_SELECTED_QUANTILE_PERIOD_V1: u64 = 100;
const SCALE_SELECTED_QUANTILE_BUCKETS_V1: [u64; 3] = [90, 9, 1];
const SCALE_MEMBER_VALUE_BYTES_V1: usize = 44;
const SCALE_MAX_MANIFEST_BYTES_V1: usize = 12 * 1024;
const SCALE_MAX_PROFILE_BYTES_V1: usize = 64 * 1024;
const SCALE_MAX_CLOSURE_BYTES_V1: usize = 4 * 1024 * 1024;
const SCALE_AUTHENTIC_BRIDGE_FORM_REF_V1: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/authored-route-evidence-bridge.za-i-vorrede-1.v1.json";
const SCALE_AUTHENTIC_BRIDGE_REPLAY_ROUTE_V1: &str = "scripts/build_zarathustra_authored_canon_evidence_bridge.py::run + scripts/validate_source_witness_foundation.py::validate_zarathustra_authored_canon_evidence_bridge";
const SCALE_AUTHORED_BRIDGE_COVERAGE_V1: &str = "unsupported_maintained_owner";
const SCALE_ALLOCATION_BLOCK_ASSUMPTION_V1: u64 = 4096;
const SCALE_HISTORY_REVISIONS_V1: u32 = 8;
const SCALE_CHANGED_ROWS_PER_REVISION_V1: u64 = 100;
const SCALE_POLICY_CHURN_PER_REVISION_V1: u64 = 10;
const SCALE_HOT_HUB_DEGREE_V1: u64 = 100_000;
const SCALE_NORMAL_OUT_DEGREE_V1: u32 = 8;
const SCALE_AGENT_COUNT_V1: u32 = 256;
const SCALE_READ_PERCENT_V1: u8 = 90;
const SCALE_WRITE_PERCENT_V1: u8 = 10;
const SCALE_SORT_ROW_BYTES_V1: u64 = 48;

/// Class order is also BINARY member-path order. Keep this enum and the
/// directories in `path_for` aligned so the authenticated member tree can be
/// built in one pass without an all-record sort.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WeightedScaleClassV1 {
    Artifact,
    Claim,
    /// A separately counted, unreviewed source-text-unit fixture used as the
    /// evidence-packet workload class. It is not an authored-route bridge.
    EvidencePacket,
    TextUnit,
    Work,
}

impl WeightedScaleClassV1 {
    const ALL: [Self; 5] = [
        Self::Artifact,
        Self::Claim,
        Self::EvidencePacket,
        Self::TextUnit,
        Self::Work,
    ];

    pub fn directory(self) -> &'static str {
        match self {
            Self::Artifact => "artifacts",
            Self::Claim => "claims",
            Self::EvidencePacket => "evidence-packets",
            Self::TextUnit => "text-units",
            Self::Work => "works",
        }
    }

    fn id_kind(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Claim => "claim",
            Self::EvidencePacket => "evidence-packet",
            Self::TextUnit => "source-text-unit-packet",
            Self::Work => "work",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Artifact => "artifact-witness.json",
            Self::Claim => "source-claims.jsonl",
            Self::EvidencePacket => "source-text-unit-packet.v1.json",
            Self::TextUnit => "source-text-unit-packet.v1.json",
            Self::Work => "work.json",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeightedScaleClassCountV1 {
    pub class: WeightedScaleClassV1,
    pub count: u64,
    pub p50_bytes: u64,
    pub p95_bytes: u64,
    pub max_bytes: u64,
}

/// Fixed 100K ladder point for the measured-workload hypothesis. The 5% class
/// is an unreviewed TextUnit packet fixture; authored-route bridge coverage is
/// deliberately excluded until its owner defines a fixture-safe contract.
/// Forecast inputs are not an OPS reservation or physical-fit result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedScaleProfileV1 {
    pub seed: Digest256,
    pub target_records: u64,
    pub classes: [WeightedScaleClassCountV1; 5],
}

impl WeightedScaleProfileV1 {
    pub fn fixed_100k(seed: Digest256) -> Self {
        Self {
            seed,
            target_records: 100_000,
            classes: [
                WeightedScaleClassCountV1 {
                    class: WeightedScaleClassV1::Artifact,
                    count: 5_000,
                    p50_bytes: 6_441,
                    p95_bytes: 8_571,
                    max_bytes: 9_054,
                },
                WeightedScaleClassCountV1 {
                    class: WeightedScaleClassV1::Claim,
                    count: 40_000,
                    p50_bytes: 1_440,
                    p95_bytes: 2_461,
                    max_bytes: 5_279,
                },
                WeightedScaleClassCountV1 {
                    class: WeightedScaleClassV1::EvidencePacket,
                    count: 5_000,
                    p50_bytes: 13_677,
                    p95_bytes: 34_041,
                    max_bytes: 86_393,
                },
                WeightedScaleClassCountV1 {
                    class: WeightedScaleClassV1::TextUnit,
                    count: 15_000,
                    p50_bytes: 13_677,
                    p95_bytes: 34_041,
                    max_bytes: 86_393,
                },
                WeightedScaleClassCountV1 {
                    class: WeightedScaleClassV1::Work,
                    count: 35_000,
                    p50_bytes: 1_977,
                    p95_bytes: 2_965,
                    max_bytes: 4_198,
                },
            ],
        }
    }

    pub fn validate(&self) -> std::io::Result<()> {
        if self.target_records == 0
            || self.target_records > 1_000_000
            || self
                .classes
                .iter()
                .map(|row| row.count as u128)
                .sum::<u128>()
                != self.target_records as u128
            || self
                .classes
                .iter()
                .zip(WeightedScaleClassV1::ALL)
                .any(|(row, class)| {
                    row.class != class
                        || row.count == 0
                        || row.p50_bytes == 0
                        || row.p95_bytes < row.p50_bytes
                        || row.max_bytes < row.p95_bytes
                        || row.max_bytes > SCALE_MAX_TEMPLATE_BYTES_V1 as u64
                })
        {
            return Err(io_invalid("weighted scale profile geometry differs"));
        }
        Ok(())
    }

    pub fn history(&self) -> WeightedScaleHistoryPlanV1 {
        WeightedScaleHistoryPlanV1 {
            revisions: SCALE_HISTORY_REVISIONS_V1,
            changed_rows_per_revision: SCALE_CHANGED_ROWS_PER_REVISION_V1,
            policy_churn_rows_per_revision: SCALE_POLICY_CHURN_PER_REVISION_V1,
            retained_pinned_revisions: [0, 3, 7],
        }
    }

    pub fn forecast_inputs(&self) -> std::io::Result<WeightedScaleForecastInputsV1> {
        self.validate()?;
        let mut p50 = 0u128;
        let mut scenario = 0u128;
        let mut member_leaf_bytes = 0u128;
        let mut object_leaf_bytes = 0u128;
        let source_text_unit_packet_records = self.classes[2]
            .count
            .checked_add(self.classes[3].count)
            .ok_or_else(|| io_invalid("TextUnit packet forecast overflow"))?;
        for row in &self.classes {
            let count = row.count as u128;
            // Price the exact same ordinal buckets used by the fixed producer
            // cursor, including a short final 100-row cycle for non-ladder callers.
            let [p50_count, p95_count, max_count] = selected_quantile_counts_v1(row.count)?;
            p50 += count * row.p50_bytes as u128;
            scenario += p50_count as u128 * row.p50_bytes as u128
                + p95_count as u128 * row.p95_bytes as u128
                + max_count as u128 * row.max_bytes as u128;
            let path_bytes = path_for(row.class, 0).len() as u128;
            member_leaf_bytes += count * (path_bytes + 44);
            object_leaf_bytes += count * (32 + 76);
        }
        // Include the raw fixture root, the shared ToS/source-witnesses
        // ancestors, all class-specific ancestors, and one record directory
        // per emitted member. The fixed prefix union is nineteen directories.
        let raw_input_directories = self
            .target_records
            .checked_add(19)
            .ok_or_else(|| io_invalid("raw fixture directory forecast overflow"))?;
        let raw_input_inodes = raw_input_directories
            .checked_add(self.target_records)
            .ok_or_else(|| io_invalid("raw fixture inode forecast overflow"))?;
        let history = self.history();
        let history_change_rows = (history.revisions as u128)
            .checked_mul(
                history.changed_rows_per_revision as u128
                    + history.policy_churn_rows_per_revision as u128,
            )
            .ok_or_else(|| io_invalid("history row forecast overflow"))?;
        let mean_template_bytes = scenario / self.target_records as u128;
        let history_change_payload_bytes = history_change_rows
            .checked_mul(mean_template_bytes)
            .ok_or_else(|| io_invalid("history payload forecast overflow"))?;
        let source_p50_1b = p50
            .checked_mul(10_000)
            .ok_or_else(|| io_invalid("1B p50 source forecast overflow"))?;
        let source_scenario_1b = scenario
            .checked_mul(10_000)
            .ok_or_else(|| io_invalid("1B source forecast overflow"))?;
        let external_sort_bytes = self
            .target_records
            .checked_mul(SCALE_SORT_ROW_BYTES_V1)
            .ok_or_else(|| io_invalid("external-sort forecast overflow"))?;
        let sort_run_count = self
            .target_records
            .checked_add(SCALE_SORT_RUN_ROWS_V1 as u64 - 1)
            .ok_or_else(|| io_invalid("external-sort run count overflow"))?
            / SCALE_SORT_RUN_ROWS_V1 as u64;
        let scratch_peak = scenario
            .checked_add(external_sort_bytes as u128)
            .ok_or_else(|| io_invalid("scratch peak forecast overflow"))?;
        let scratch_blocks_4k = scratch_peak
            .checked_add(SCALE_ALLOCATION_BLOCK_ASSUMPTION_V1 as u128 - 1)
            .ok_or_else(|| io_invalid("scratch block forecast overflow"))?
            / SCALE_ALLOCATION_BLOCK_ASSUMPTION_V1 as u128;
        let max_pack_count = self
            .target_records
            .checked_add(MAX_PACKED_OBJECT_FRAMES_V2 as u64 - 1)
            .ok_or_else(|| io_invalid("packed object count forecast overflow"))?
            / MAX_PACKED_OBJECT_FRAMES_V2 as u64;
        Ok(WeightedScaleForecastInputsV1 {
            target_records_100k: self.target_records,
            raw_input_file_count_100k: self.target_records,
            raw_input_directory_count_100k: raw_input_directories,
            raw_input_inode_count_100k: raw_input_inodes,
            source_text_unit_packet_records_100k: source_text_unit_packet_records,
            authored_route_bridge_records_100k: 0,
            authored_route_bridge_coverage: SCALE_AUTHORED_BRIDGE_COVERAGE_V1,
            p50_logical_source_bytes_100k: checked_scale_u64(p50)?,
            selected_quantile_scenario_logical_source_bytes_100k: checked_scale_u64(scenario)?,
            raw_member_leaf_input_bytes_100k: checked_scale_u64(member_leaf_bytes)?,
            raw_object_extent_leaf_input_bytes_100k: checked_scale_u64(object_leaf_bytes)?,
            external_sort_logical_bytes_100k: external_sort_bytes,
            temporary_payload_spool_peak_bytes_100k: checked_scale_u64(scenario)?,
            temporary_digest_sort_peak_bytes_100k: external_sort_bytes,
            temporary_scratch_peak_bytes_100k: checked_scale_u64(scratch_peak)?,
            temporary_scratch_blocks_4k_assumption_100k: checked_scale_u64(scratch_blocks_4k)?,
            temporary_sort_run_count_100k: sort_run_count,
            temporary_file_inode_peak_100k: sort_run_count + 2,
            packed_object_frames_per_pack: MAX_PACKED_OBJECT_FRAMES_V2,
            packed_object_pack_count_upper_100k: max_pack_count,
            history_change_rows_100k: checked_scale_u64(history_change_rows)?,
            history_change_payload_scenario_bytes_100k: checked_scale_u64(
                history_change_payload_bytes,
            )?,
            p50_logical_source_bytes_1b: checked_scale_u64(source_p50_1b)?,
            selected_quantile_scenario_logical_source_bytes_1b: checked_scale_u64(
                source_scenario_1b,
            )?,
            current_snapshot_three_copy_p50_bytes_100k: checked_scale_u64(p50 * 3)?,
            three_pins_with_backup_and_restore_no_dedup_p50_bytes_100k: checked_scale_u64(p50 * 5)?,
            ten_full_copy_no_dedup_scenario_bytes_1b: checked_scale_u64(source_scenario_1b * 10)?,
            revisions: history.revisions,
            changed_rows_per_revision: history.changed_rows_per_revision,
            policy_churn_rows_per_revision: history.policy_churn_rows_per_revision,
            retained_pinned_revisions: history.retained_pinned_revisions,
            normal_out_degree: SCALE_NORMAL_OUT_DEGREE_V1,
            hot_hub_out_degree: SCALE_HOT_HUB_DEGREE_V1,
            concurrent_clients: SCALE_AGENT_COUNT_V1,
            expected_read_clients: 230,
            expected_write_clients: 26,
            selected_native_state_bytes_per_client: None,
            read_clients_state_upper_bytes: None,
            writer_staging_upper_bytes_per_client: None,
            write_clients_staging_upper_bytes: None,
            writer_callback_state_upper_bytes_per_client: None,
            write_clients_callback_state_upper_bytes: None,
            full_256_peak_established: false,
            read_percent: SCALE_READ_PERCENT_V1,
            write_percent: SCALE_WRITE_PERCENT_V1,
            unique_payload_ratio: None,
            physical_fit_established: false,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeightedScaleHistoryPlanV1 {
    pub revisions: u32,
    pub changed_rows_per_revision: u64,
    pub policy_churn_rows_per_revision: u64,
    pub retained_pinned_revisions: [u32; 3],
}

/// Explicit inputs for the eventual OPS bill. `None` means the scale producer
/// must measure payload sharing; it cannot claim compression or CAS savings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedScaleForecastInputsV1 {
    pub target_records_100k: u64,
    /// Raw filesystem input for the real source census: one member file per
    /// record plus the root and the shared/per-record directory hierarchy.
    pub raw_input_file_count_100k: u64,
    pub raw_input_directory_count_100k: u64,
    pub raw_input_inode_count_100k: u64,
    /// The 5% EvidencePacket class plus the 15% TextUnit class. Both use the
    /// schema-supported unreviewed TextUnit packet contract.
    pub source_text_unit_packet_records_100k: u64,
    /// Authored-route bridge fixtures are intentionally not generated under
    /// a schema that hardcodes canon/review/match assertions.
    pub authored_route_bridge_records_100k: u64,
    pub authored_route_bridge_coverage: &'static str,
    pub p50_logical_source_bytes_100k: u64,
    pub selected_quantile_scenario_logical_source_bytes_100k: u64,
    pub raw_member_leaf_input_bytes_100k: u64,
    pub raw_object_extent_leaf_input_bytes_100k: u64,
    /// Upper-bound scratch geometry: spool all logical member bytes before
    /// payload deduplication, then retain sorted digest runs while packing.
    pub temporary_payload_spool_peak_bytes_100k: u64,
    pub temporary_digest_sort_peak_bytes_100k: u64,
    pub temporary_scratch_peak_bytes_100k: u64,
    /// A clearly labeled 4 KiB allocation-unit estimate, not measured blocks.
    pub temporary_scratch_blocks_4k_assumption_100k: u64,
    pub temporary_sort_run_count_100k: u64,
    /// One payload spool, one sort directory, and one inode per sorted run.
    pub temporary_file_inode_peak_100k: u64,
    pub packed_object_frames_per_pack: u32,
    pub packed_object_pack_count_upper_100k: u64,
    pub external_sort_logical_bytes_100k: u64,
    pub history_change_rows_100k: u64,
    pub history_change_payload_scenario_bytes_100k: u64,
    pub p50_logical_source_bytes_1b: u64,
    pub selected_quantile_scenario_logical_source_bytes_1b: u64,
    pub current_snapshot_three_copy_p50_bytes_100k: u64,
    pub three_pins_with_backup_and_restore_no_dedup_p50_bytes_100k: u64,
    pub ten_full_copy_no_dedup_scenario_bytes_1b: u64,
    pub revisions: u32,
    pub changed_rows_per_revision: u64,
    pub policy_churn_rows_per_revision: u64,
    pub retained_pinned_revisions: [u32; 3],
    pub normal_out_degree: u32,
    pub hot_hub_out_degree: u64,
    pub concurrent_clients: u32,
    pub expected_read_clients: u32,
    pub expected_write_clients: u32,
    /// Each reader and writer uses its own selected Native state/work limits.
    /// These bounds are populated from that operation at materialization.
    pub selected_native_state_bytes_per_client: Option<u64>,
    pub read_clients_state_upper_bytes: Option<u64>,
    pub writer_staging_upper_bytes_per_client: Option<u64>,
    pub write_clients_staging_upper_bytes: Option<u64>,
    pub writer_callback_state_upper_bytes_per_client: Option<u64>,
    pub write_clients_callback_state_upper_bytes: Option<u64>,
    pub full_256_peak_established: bool,
    pub read_percent: u8,
    pub write_percent: u8,
    pub unique_payload_ratio: Option<f64>,
    pub physical_fit_established: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WeightedScaleProducerEnvelopeV1 {
    pub(crate) maximum_source_bytes: u64,
    pub(crate) temporary_logical_bytes: u64,
    pub(crate) temporary_allocated_bytes: u64,
    pub(crate) temporary_file_inodes: u64,
    pub(crate) raw_input_allocated_bytes: u64,
}

pub(crate) fn weighted_scale_producer_envelope_v1(
    profile: &WeightedScaleProfileV1,
    allocation_unit: u64,
) -> std::io::Result<WeightedScaleProducerEnvelopeV1> {
    profile.validate()?;
    let maximum_source_bytes = weighted_source_upper_bound_v1(profile)?;
    let run_count = sort_run_count_v1(profile.target_records)?;
    let sort_logical_bytes = profile
        .target_records
        .checked_mul(SCALE_SORT_ROW_BYTES_V1)
        .ok_or_else(|| io_invalid("weighted producer sort bytes overflow"))?;
    let temporary_logical_bytes = maximum_source_bytes
        .checked_add(sort_logical_bytes)
        .ok_or_else(|| io_invalid("weighted producer temporary bound overflow"))?;
    Ok(WeightedScaleProducerEnvelopeV1 {
        maximum_source_bytes,
        temporary_logical_bytes,
        temporary_allocated_bytes: scratch_allocation_upper_v1(
            maximum_source_bytes,
            run_count,
            allocation_unit,
        )?,
        temporary_file_inodes: run_count
            .checked_add(2)
            .ok_or_else(|| io_invalid("weighted producer inode bound overflow"))?,
        raw_input_allocated_bytes: raw_fixture_allocation_upper_v1(profile, allocation_unit)?,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedScaleTemplateV1 {
    pub class: WeightedScaleClassV1,
    pub source_path: &'static str,
    pub source_sha256: Digest256,
    pub template_sha256: Digest256,
    pub bytes: Vec<u8>,
    external_reference_edges: BTreeSet<ScaleReferenceEdgeV1>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScaleReferenceEdgeV1 {
    pointer: String,
    reference: String,
}

/// At most fifteen verified templates are resident; generated member payloads
/// are returned one at a time and never accumulated in a corpus-sized Vec.
pub struct WeightedScaleTemplateSetV1 {
    templates: [Vec<WeightedScaleTemplateV1>; 5],
    pub manifest_sha256: Digest256,
    pub resident_template_bytes: u64,
    pub resident_reference_state_bytes: u64,
}

impl WeightedScaleTemplateSetV1 {
    pub fn load(repository_root: &Path) -> std::io::Result<Self> {
        Self::load_inner(repository_root, None)
    }

    fn load_accounted(
        repository_root: &Path,
        io: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &AdmissionWorkBudget,
    ) -> std::io::Result<Self> {
        Self::load_inner(repository_root, Some((io, deadline, cancelled, work)))
    }

    fn load_inner(
        repository_root: &Path,
        accounting: Option<(
            &PinnedSqliteIoBudget,
            Instant,
            &AtomicBool,
            &AdmissionWorkBudget,
        )>,
    ) -> std::io::Result<Self> {
        let pins = scale_template_pins_v1();
        let mut templates: [Vec<WeightedScaleTemplateV1>; 5] = std::array::from_fn(|_| Vec::new());
        let mut resident = 0u64;
        for pin in pins {
            let path = repository_root.join(pin.path);
            if let Some((io, deadline, cancelled, work)) = accounting {
                scale_active(deadline, cancelled)?;
                work.charge_many(1)?;
                let metadata = fs::symlink_metadata(&path)?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(io_invalid("pinned scale template is not a regular file"));
                }
                if metadata.len() > SCALE_MAX_TEMPLATE_BYTES_V1 as u64 {
                    return Err(io_invalid("pinned scale template exceeds selected bound"));
                }
                io.charge_read_upper_bound(metadata.len())
                    .map_err(|_| io_invalid("pinned template read budget exhausted"))?;
                let file_bytes = fs::read(&path)?;
                io.record_read_returned(file_bytes.len() as u64)
                    .map_err(|_| io_invalid("pinned template read accounting failed"))?;
                if file_bytes.len() as u64 != metadata.len() {
                    return Err(io_invalid("pinned template changed while reading"));
                }
                Self::push_template(&mut templates, &mut resident, pin, file_bytes)?;
            } else {
                let file_bytes = fs::read(&path)?;
                Self::push_template(&mut templates, &mut resident, pin, file_bytes)?;
            }
        }
        let mut reference_state = 0u64;
        for rows in &mut templates {
            rows.sort_by_key(|template| template.bytes.len());
            if rows.len() != 3 {
                return Err(io_invalid("scale class template count differs"));
            }
            for template in rows {
                for edge in &template.external_reference_edges {
                    reference_state = reference_state
                        .checked_add((edge.pointer.len() + edge.reference.len()) as u64)
                        .ok_or_else(|| io_invalid("scale reference state overflow"))?;
                }
            }
        }
        let manifest_sha256 = template_manifest_digest_v1(&templates)?;
        Ok(Self {
            templates,
            manifest_sha256,
            resident_template_bytes: resident,
            resident_reference_state_bytes: reference_state,
        })
    }

    fn push_template(
        templates: &mut [Vec<WeightedScaleTemplateV1>; 5],
        resident: &mut u64,
        pin: ScaleTemplatePinV1,
        file_bytes: Vec<u8>,
    ) -> std::io::Result<()> {
        if file_bytes.len() > SCALE_MAX_TEMPLATE_BYTES_V1
            || Digest256::of_bytes(&file_bytes).to_hex() != pin.file_sha256
        {
            return Err(io_invalid("pinned scale template file identity differs"));
        }
        let source_sha256 = Digest256::of_bytes(&file_bytes);
        let template_bytes = if let Some(line) = pin.line {
            file_bytes
                .split_inclusive(|byte| *byte == b'\n')
                .nth(line as usize - 1)
                .ok_or_else(|| io_invalid("pinned Claim row absent"))?
                .to_vec()
        } else {
            file_bytes
        };
        let template_sha256 = Digest256::of_bytes(&template_bytes);
        if template_sha256.to_hex() != pin.template_sha256 {
            return Err(io_invalid("pinned scale template digest differs"));
        }
        let template_value: serde_json::Value = serde_json::from_slice(&template_bytes)
            .map_err(|_| io_invalid("pinned scale template JSON is invalid"))?;
        let external_reference_edges = collect_reference_edges_v1(&template_value);
        let external_reference_edges = external_reference_edges.into_iter().collect();
        *resident = resident
            .checked_add(template_bytes.len() as u64)
            .ok_or_else(|| io_invalid("pinned template bytes overflow"))?;
        templates[pin.class_index].push(WeightedScaleTemplateV1 {
            class: pin.class,
            source_path: pin.path,
            source_sha256,
            template_sha256,
            bytes: template_bytes,
            external_reference_edges,
        });
        Ok(())
    }

    fn select(&self, class: WeightedScaleClassV1, ordinal: u64) -> &WeightedScaleTemplateV1 {
        let bucket = ordinal % SCALE_SELECTED_QUANTILE_PERIOD_V1;
        let index = selected_quantile_index_v1(bucket);
        let templates = &self.templates[class as usize];
        &templates[if templates.len() == 1 { 0 } else { index }]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedScaleMemberV1 {
    pub class: WeightedScaleClassV1,
    pub ordinal: u64,
    pub path: String,
    pub digest: Digest256,
    pub source_bytes: Vec<u8>,
    pub template_sha256: Digest256,
    pub mode: u32,
}

/// Path-ordered source payload cursor for the authenticated member tree. It
/// emits bounded owner-shaped JSON/JSONL records, not generic workload tokens.
pub struct WeightedScaleMemberIterV1<'a> {
    profile: &'a WeightedScaleProfileV1,
    templates: &'a WeightedScaleTemplateSetV1,
    class_index: usize,
    ordinal: u64,
    member_count: u64,
    source_bytes: u64,
    class_source_bytes: [u64; 5],
    closure: WeightedScaleClosureAccumulatorV1,
    failed: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScaleReferenceUsageKeyV1 {
    class: String,
    source_path: String,
    template_sha256: String,
    pointer: String,
    reference: String,
}

#[derive(Clone, Debug, Default)]
struct WeightedScaleClosureAccumulatorV1 {
    generated_dependency_edges: u64,
    pinned_external_dependency_edges: u64,
    generator_reference_edges: u64,
    unresolved_dependency_edges: u64,
    external_uses: BTreeMap<ScaleReferenceUsageKeyV1, u64>,
}

impl<'a> WeightedScaleMemberIterV1<'a> {
    pub fn new(
        profile: &'a WeightedScaleProfileV1,
        templates: &'a WeightedScaleTemplateSetV1,
    ) -> std::io::Result<Self> {
        profile.validate()?;
        Ok(Self {
            profile,
            templates,
            class_index: 0,
            ordinal: 0,
            member_count: 0,
            source_bytes: 0,
            class_source_bytes: [0; 5],
            closure: WeightedScaleClosureAccumulatorV1::default(),
            failed: false,
        })
    }

    pub fn member_count(&self) -> u64 {
        self.member_count
    }

    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }

    pub fn class_source_bytes(&self) -> [u64; 5] {
        self.class_source_bytes
    }

    fn closure(&self) -> &WeightedScaleClosureAccumulatorV1 {
        &self.closure
    }

    pub fn next_member(&mut self) -> std::io::Result<Option<WeightedScaleMemberV1>> {
        if self.failed {
            return Err(io_invalid("weighted scale cursor already failed"));
        }
        let result = self.next_inner();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn next_inner(&mut self) -> std::io::Result<Option<WeightedScaleMemberV1>> {
        while self.class_index < self.profile.classes.len()
            && self.ordinal == self.profile.classes[self.class_index].count
        {
            self.class_index += 1;
            self.ordinal = 0;
        }
        if self.class_index == self.profile.classes.len() {
            if self.member_count != self.profile.target_records {
                return Err(io_invalid("weighted scale cursor count differs"));
            }
            return Ok(None);
        }
        let class_row = self.profile.classes[self.class_index];
        let ordinal = self.ordinal;
        let template = self.templates.select(class_row.class, ordinal);
        let mut value: serde_json::Value = serde_json::from_slice(&template.bytes)
            .map_err(|_| io_invalid("pinned scale template JSON is invalid"))?;
        rewrite_fixture_record_v1(
            &mut value,
            class_row.class,
            ordinal,
            self.profile.seed,
            self.profile.classes[4].count,
        )?;
        audit_fixture_references_v1(
            &value,
            template,
            self.profile,
            self.profile.seed,
            class_row.class,
            ordinal,
            &mut self.closure,
        )?;
        // Claim templates are selected from a JSONL source. Keep each generated
        // line a single JSON value so the raw fixture is consumable by the same
        // owner parser as its pinned source shape.
        let mut source_bytes = if class_row.class == WeightedScaleClassV1::Claim {
            serde_json::to_vec(&value)
        } else {
            serde_json::to_vec_pretty(&value)
        }
        .map_err(|_| io_invalid("weighted source record encoding failed"))?;
        source_bytes.push(b'\n');
        let selected_template_bytes = match selected_quantile_index_v1(
            ordinal % SCALE_SELECTED_QUANTILE_PERIOD_V1,
        ) {
            0 => class_row.p50_bytes,
            1 => class_row.p95_bytes,
            _ => class_row.max_bytes,
        };
        let selected_member_upper = selected_template_bytes
            .checked_add(SCALE_IDENTITY_GROWTH_ALLOWANCE_V1)
            .ok_or_else(|| io_invalid("weighted member growth fence overflow"))?;
        let generated_member_bytes = u64::try_from(source_bytes.len())
            .map_err(|_| io_invalid("weighted member size exceeds u64"))?;
        if source_bytes.is_empty()
            || source_bytes.len() > SCALE_MAX_TEMPLATE_BYTES_V1
            || generated_member_bytes > selected_member_upper
        {
            return Err(io_invalid(
                "weighted source member exceeds selected growth fence",
            ));
        }
        let member_count = self
            .member_count
            .checked_add(1)
            .ok_or_else(|| io_invalid("weighted source member count overflow"))?;
        let source_bytes_total = self
            .source_bytes
            .checked_add(source_bytes.len() as u64)
            .ok_or_else(|| io_invalid("weighted source bytes overflow"))?;
        self.class_source_bytes[class_row.class as usize] = self.class_source_bytes
            [class_row.class as usize]
            .checked_add(source_bytes.len() as u64)
            .ok_or_else(|| io_invalid("weighted class source bytes overflow"))?;
        let digest = Digest256::of_bytes(&source_bytes);
        let path = path_for(class_row.class, ordinal);
        self.member_count = member_count;
        self.source_bytes = source_bytes_total;
        self.ordinal += 1;
        Ok(Some(WeightedScaleMemberV1 {
            class: class_row.class,
            ordinal,
            path,
            digest,
            source_bytes,
            template_sha256: template.template_sha256,
            mode: 0o644,
        }))
    }
}

fn path_for(class: WeightedScaleClassV1, ordinal: u64) -> String {
    match class {
        WeightedScaleClassV1::Artifact => format!(
            "{SCALE_FIXTURE_ROOT_V1}/artifacts/scale-fixtures/v1/{ordinal:020}/artifact-witness.json"
        ),
        WeightedScaleClassV1::Claim => format!(
            "{SCALE_FIXTURE_ROOT_V1}/relations/scale-fixtures/v1/claims/{ordinal:020}/source-claims.jsonl"
        ),
        WeightedScaleClassV1::EvidencePacket => format!(
            "{SCALE_FIXTURE_ROOT_V1}/works/scale-fixtures/v1/evidence-packets/{ordinal:020}/source-text-unit-packet.v1.json"
        ),
        WeightedScaleClassV1::TextUnit => format!(
            "{SCALE_FIXTURE_ROOT_V1}/works/scale-fixtures/v1/text-units/{ordinal:020}/source-text-unit-packet.v1.json"
        ),
        WeightedScaleClassV1::Work => {
            format!("{SCALE_FIXTURE_ROOT_V1}/works/scale-fixtures/v1/works/{ordinal:020}/work.json")
        }
    }
}

fn generated_path_prefix_v1(class: WeightedScaleClassV1) -> &'static str {
    match class {
        WeightedScaleClassV1::Artifact => "ToS/source-witnesses/artifacts/scale-fixtures/v1/",
        WeightedScaleClassV1::Claim => "ToS/source-witnesses/relations/scale-fixtures/v1/claims/",
        WeightedScaleClassV1::EvidencePacket => {
            "ToS/source-witnesses/works/scale-fixtures/v1/evidence-packets/"
        }
        WeightedScaleClassV1::TextUnit => {
            "ToS/source-witnesses/works/scale-fixtures/v1/text-units/"
        }
        WeightedScaleClassV1::Work => "ToS/source-witnesses/works/scale-fixtures/v1/works/",
    }
}

fn generated_identity_field_v1(key: &str, pointer: &str) -> bool {
    matches!(
        key,
        "artifact_id"
            | "claim_id"
            | "record_id"
            | "packet_id"
            | "unit_id"
            | "scheme_id"
            | "segmentation_id"
            | "review_id"
            | "projection_id"
    ) || (key == "anchor_ref" && pointer.starts_with("/anchors/"))
}

fn collect_reference_edges_v1(value: &serde_json::Value) -> Vec<ScaleReferenceEdgeV1> {
    fn walk(
        value: &serde_json::Value,
        pointer: &mut String,
        key: Option<&str>,
        output: &mut Vec<ScaleReferenceEdgeV1>,
    ) {
        match value {
            serde_json::Value::Object(object) => {
                for (name, child) in object {
                    let old_len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&name.replace('~', "~0").replace('/', "~1"));
                    walk(child, pointer, Some(name), output);
                    pointer.truncate(old_len);
                }
            }
            serde_json::Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let old_len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&index.to_string());
                    walk(child, pointer, None, output);
                    pointer.truncate(old_len);
                }
            }
            serde_json::Value::String(reference)
                if looks_like_source_reference_v1(reference)
                    && !key.is_some_and(|name| generated_identity_field_v1(name, pointer)) =>
            {
                output.push(ScaleReferenceEdgeV1 {
                    pointer: pointer.clone(),
                    reference: reference.clone(),
                });
            }
            _ => {}
        }
    }

    let mut output = Vec::new();
    walk(value, &mut String::new(), None, &mut output);
    output.sort();
    output.dedup();
    output
}

fn looks_like_source_reference_v1(value: &str) -> bool {
    value.starts_with("tos.")
        || value.starts_with("ToS/")
        || value.starts_with("https://")
        || value.starts_with("http://")
        || value.starts_with("software:")
        || value.starts_with("model:")
        || value.starts_with("agent:")
        || value.starts_with("human:")
}

fn collect_generated_ids_v1(value: &serde_json::Value) -> BTreeSet<String> {
    fn walk(
        value: &serde_json::Value,
        pointer: &mut String,
        key: Option<&str>,
        output: &mut BTreeSet<String>,
    ) {
        match value {
            serde_json::Value::Object(object) => {
                for (name, child) in object {
                    let old_len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&name.replace('~', "~0").replace('/', "~1"));
                    walk(child, pointer, Some(name), output);
                    pointer.truncate(old_len);
                }
            }
            serde_json::Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let old_len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&index.to_string());
                    walk(child, pointer, None, output);
                    pointer.truncate(old_len);
                }
            }
            serde_json::Value::String(id)
                if id.starts_with("tos.")
                    && key.is_some_and(|name| generated_identity_field_v1(name, pointer)) =>
            {
                output.insert(id.clone());
            }
            _ => {}
        }
    }
    let mut output = BTreeSet::new();
    walk(value, &mut String::new(), None, &mut output);
    output
}

fn generated_path_exists_v1(reference: &str, profile: &WeightedScaleProfileV1) -> bool {
    WeightedScaleClassV1::ALL.into_iter().any(|class| {
        let Some(rest) = reference.strip_prefix(generated_path_prefix_v1(class)) else {
            return false;
        };
        let Some((ordinal, suffix)) = rest.split_once('/') else {
            return false;
        };
        ordinal.len() == 20
            && ordinal.bytes().all(|byte| byte.is_ascii_digit())
            && ordinal
                .parse::<u64>()
                .ok()
                .is_some_and(|index| index < profile.classes[class as usize].count)
            && suffix == class.suffix()
    })
}

fn generated_id_exists_v1(
    reference: &str,
    profile: &WeightedScaleProfileV1,
    seed: Digest256,
    local_generated_ids: &BTreeSet<String>,
) -> bool {
    if local_generated_ids.contains(reference) {
        return true;
    }
    [
        WeightedScaleClassV1::Artifact,
        WeightedScaleClassV1::Claim,
        WeightedScaleClassV1::Work,
    ]
    .into_iter()
    .any(|class| {
        let prefix = format!("tos.{}.scale-fixture.", class.id_kind());
        let Some(rest) = reference.strip_prefix(&prefix) else {
            return false;
        };
        let Some((ordinal, suffix)) = rest.split_once('.') else {
            return false;
        };
        ordinal.len() == 20
            && ordinal.bytes().all(|byte| byte.is_ascii_digit())
            && ordinal.parse::<u64>().ok().is_some_and(|index| {
                index < profile.classes[class as usize].count
                    && scale_identity(seed, class, index) == reference
            })
            && suffix.len() == 64
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn audit_fixture_references_v1(
    value: &serde_json::Value,
    template: &WeightedScaleTemplateV1,
    profile: &WeightedScaleProfileV1,
    seed: Digest256,
    class: WeightedScaleClassV1,
    _ordinal: u64,
    closure: &mut WeightedScaleClosureAccumulatorV1,
) -> std::io::Result<()> {
    let local_generated_ids = collect_generated_ids_v1(value);
    for edge in collect_reference_edges_v1(value) {
        if edge.reference == SCALE_GENERATOR_AGENT_REF_V1 && edge.pointer.ends_with("/agent_ref") {
            closure.generator_reference_edges = closure
                .generator_reference_edges
                .checked_add(1)
                .ok_or_else(|| io_invalid("generator reference count overflow"))?;
            continue;
        }
        if (edge.reference.starts_with("tos.")
            && generated_id_exists_v1(&edge.reference, profile, seed, &local_generated_ids))
            || (edge.reference.starts_with("ToS/")
                && generated_path_exists_v1(&edge.reference, profile))
        {
            closure.generated_dependency_edges = closure
                .generated_dependency_edges
                .checked_add(1)
                .ok_or_else(|| io_invalid("generated dependency edge count overflow"))?;
            continue;
        }
        if template.external_reference_edges.contains(&edge) {
            closure.pinned_external_dependency_edges = closure
                .pinned_external_dependency_edges
                .checked_add(1)
                .ok_or_else(|| io_invalid("external dependency edge count overflow"))?;
            let key = ScaleReferenceUsageKeyV1 {
                class: format!("{:?}", class),
                source_path: template.source_path.to_owned(),
                template_sha256: template.template_sha256.to_hex(),
                pointer: edge.pointer,
                reference: edge.reference,
            };
            let count = closure.external_uses.entry(key).or_default();
            *count = count
                .checked_add(1)
                .ok_or_else(|| io_invalid("external dependency usage count overflow"))?;
            continue;
        }
        closure.unresolved_dependency_edges = closure
            .unresolved_dependency_edges
            .checked_add(1)
            .ok_or_else(|| io_invalid("unresolved dependency edge count overflow"))?;
        return Err(io_invalid(
            "generated source reference is outside pinned closure",
        ));
    }
    Ok(())
}

fn scale_identity(seed: Digest256, class: WeightedScaleClassV1, ordinal: u64) -> String {
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-native-scale-fixture-id-v1\0");
    hasher.update(seed.as_bytes());
    hasher.update(class.id_kind().as_bytes());
    hasher.update(&ordinal.to_be_bytes());
    let digest = hasher.finalize().to_hex();
    if matches!(
        class,
        WeightedScaleClassV1::EvidencePacket | WeightedScaleClassV1::TextUnit
    ) {
        format!("tos.source-text-unit-packet.sid-{}", &digest[..32])
    } else {
        format!(
            "tos.{}.scale-fixture.{ordinal:020}.{}",
            class.id_kind(),
            &digest[..64]
        )
    }
}

fn rewrite_fixture_record_v1(
    value: &mut serde_json::Value,
    class: WeightedScaleClassV1,
    ordinal: u64,
    seed: Digest256,
    work_count: u64,
) -> std::io::Result<()> {
    if matches!(
        class,
        WeightedScaleClassV1::EvidencePacket | WeightedScaleClassV1::TextUnit
    ) {
        rewrite_text_unit_ids_v1(value, seed, class, ordinal)?;
    }
    let object = value
        .as_object_mut()
        .ok_or_else(|| io_invalid("scale template root is not an object"))?;
    match class {
        WeightedScaleClassV1::Artifact => {
            set_string(object, "artifact_id", scale_identity(seed, class, ordinal))?;
            set_number(object, "record_version", 1)?;
            if let Some(maker) = object
                .get_mut("maker")
                .and_then(serde_json::Value::as_object_mut)
            {
                maker.insert(
                    "maker_type".to_owned(),
                    serde_json::Value::String("software".to_owned()),
                );
                maker.insert(
                    "agent_ref".to_owned(),
                    serde_json::Value::String(SCALE_GENERATOR_AGENT_REF_V1.to_owned()),
                );
                maker.insert(
                    "human_review_performed".to_owned(),
                    serde_json::Value::Bool(false),
                );
            }
            if let Some(note) = object
                .get_mut("path_identity")
                .and_then(serde_json::Value::as_object_mut)
                .and_then(|identity| identity.get_mut("note"))
            {
                *note = serde_json::Value::String(
                    "Private synthetic workload fixture; identity is generated from a pinned artifact-witness shape and does not identify a physical object.".to_owned(),
                );
            }
        }
        WeightedScaleClassV1::Claim => {
            set_string(object, "claim_id", scale_identity(seed, class, ordinal))?;
            set_string(
                object,
                "subject_ref",
                scale_identity(seed, WeightedScaleClassV1::Work, ordinal % work_count),
            )?;
            set_string(object, "epistemic_status", "uncertain".to_owned())?;
            set_string(object, "review_status", "unreviewed".to_owned())?;
            set_string(object, "visibility", "local_only".to_owned())?;
            set_number(object, "claim_version", 1)?;
            object.insert("reviews".to_owned(), serde_json::Value::Array(Vec::new()));
            object.insert("supersedes_claim_ref".to_owned(), serde_json::Value::Null);
            if let Some(maker) = object
                .get_mut("maker")
                .and_then(serde_json::Value::as_object_mut)
            {
                maker.insert(
                    "maker_type".to_owned(),
                    serde_json::Value::String("software".to_owned()),
                );
                maker.insert(
                    "agent_ref".to_owned(),
                    serde_json::Value::String(SCALE_GENERATOR_AGENT_REF_V1.to_owned()),
                );
            }
        }
        WeightedScaleClassV1::EvidencePacket | WeightedScaleClassV1::TextUnit => {
            set_string(object, "packet_id", scale_identity(seed, class, ordinal))?;
            set_number(object, "packet_version", 1)?;
            object.insert("supersedes_packet_ref".to_owned(), serde_json::Value::Null);
            object.insert("reviews".to_owned(), serde_json::Value::Array(Vec::new()));
            if let Some(source_scope) = object
                .get_mut("source_scope")
                .and_then(serde_json::Value::as_object_mut)
            {
                let work_ref = source_scope
                    .get_mut("work_ref")
                    .ok_or_else(|| io_invalid("TextUnit source work reference absent"))?;
                *work_ref = serde_json::Value::String(scale_identity(
                    seed,
                    WeightedScaleClassV1::Work,
                    ordinal % work_count,
                ));
            }
            if let Some(method) = object
                .get_mut("method")
                .and_then(serde_json::Value::as_object_mut)
            {
                method.insert(
                    "maker_kind".to_owned(),
                    serde_json::Value::String("synthetic_fixture".to_owned()),
                );
                method.insert(
                    "agent_ref".to_owned(),
                    serde_json::Value::String(SCALE_GENERATOR_AGENT_REF_V1.to_owned()),
                );
            }
        }
        WeightedScaleClassV1::Work => {
            set_string(object, "record_id", scale_identity(seed, class, ordinal))?;
            set_string(object, "identity_status", "provisional".to_owned())?;
            set_number(object, "record_version", 1)?;
            object.insert("supersedes_ref".to_owned(), serde_json::Value::Null);
            // Source claim references in a template identify the real record.
            // Clear them rather than leaving stale refs after this fixture's
            // generated Work identity changes; generated Claims point back to
            // the generated Work deterministically.
            object.insert(
                "responsibility_claim_refs".to_owned(),
                serde_json::Value::Array(Vec::new()),
            );
            object.insert(
                "expression_claim_refs".to_owned(),
                serde_json::Value::Array(Vec::new()),
            );
        }
    }
    Ok(())
}

const SCALE_GENERATOR_AGENT_REF_V1: &str =
    "software:tos-native-scale-workload-fixture-generator-v1";

fn rewrite_text_unit_ids_v1(
    value: &mut serde_json::Value,
    seed: Digest256,
    class: WeightedScaleClassV1,
    ordinal: u64,
) -> std::io::Result<()> {
    fn collect(
        value: &serde_json::Value,
        pointer: &mut String,
        key: Option<&str>,
        seed: Digest256,
        class: WeightedScaleClassV1,
        ordinal: u64,
        identities: &mut BTreeMap<String, String>,
    ) -> std::io::Result<()> {
        match value {
            serde_json::Value::Object(object) => {
                for (name, child) in object {
                    let old_len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&name.replace('~', "~0").replace('/', "~1"));
                    collect(child, pointer, Some(name), seed, class, ordinal, identities)?;
                    pointer.truncate(old_len);
                }
            }
            serde_json::Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let old_len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&index.to_string());
                    collect(child, pointer, None, seed, class, ordinal, identities)?;
                    pointer.truncate(old_len);
                }
            }
            serde_json::Value::String(old) if old.starts_with("tos.") => {
                let kind = match (key, pointer.as_str()) {
                    (Some("anchor_ref"), path) if path.starts_with("/anchors/") => Some("anchor"),
                    (Some("scheme_id"), path) if path.starts_with("/schemes/") => Some("scheme"),
                    (Some("segmentation_id"), path) if path.starts_with("/segmentations/") => {
                        Some("segmentation")
                    }
                    (Some("unit_id"), path) if path.starts_with("/units/") => Some("unit"),
                    _ => None,
                };
                if let Some(kind) = kind {
                    let mut hasher = Digest256Hasher::new();
                    hasher.update(b"tos-native-scale-fixture-nested-id-v1\0");
                    hasher.update(seed.as_bytes());
                    hasher.update(&[class as u8]);
                    hasher.update(&ordinal.to_be_bytes());
                    hasher.update(kind.as_bytes());
                    hasher.update(old.as_bytes());
                    let digest = hasher.finalize().to_hex();
                    let new = match kind {
                        "anchor" => format!("tos.anchor.scale-fixture.{ordinal:020}.{digest}"),
                        "scheme" => format!("tos.text-unit-scheme.sid-{}", &digest[..32]),
                        "segmentation" => format!("tos.text-segmentation.sid-{}", &digest[..32]),
                        "unit" => format!("tos.text-unit.sid-{}", &digest[..32]),
                        _ => return Err(io_invalid("unknown generated TextUnit identity kind")),
                    };
                    if identities.insert(old.clone(), new).is_some() {
                        return Err(io_invalid("TextUnit template repeats an identity field"));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn replace(value: &mut serde_json::Value, identities: &BTreeMap<String, String>) {
        match value {
            serde_json::Value::Object(object) => {
                for child in object.values_mut() {
                    replace(child, identities);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    replace(child, identities);
                }
            }
            serde_json::Value::String(reference) => {
                if let Some(replacement) = identities.get(reference) {
                    *reference = replacement.clone();
                }
            }
            _ => {}
        }
    }

    let mut identities = BTreeMap::new();
    collect(
        value,
        &mut String::new(),
        None,
        seed,
        class,
        ordinal,
        &mut identities,
    )?;
    replace(value, &identities);
    fn mark_fixture_makers(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                let is_method =
                    object.contains_key("maker_kind") && object.contains_key("agent_ref");
                if is_method {
                    object.insert(
                        "maker_kind".to_owned(),
                        serde_json::Value::String("synthetic_fixture".to_owned()),
                    );
                    object.insert(
                        "agent_ref".to_owned(),
                        serde_json::Value::String(SCALE_GENERATOR_AGENT_REF_V1.to_owned()),
                    );
                }
                for child in object.values_mut() {
                    mark_fixture_makers(child);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    mark_fixture_makers(child);
                }
            }
            _ => {}
        }
    }
    mark_fixture_makers(value);
    Ok(())
}

fn set_string(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: String,
) -> std::io::Result<()> {
    let target = object
        .get_mut(key)
        .ok_or_else(|| io_invalid("scale template identity field absent"))?;
    *target = serde_json::Value::String(value);
    Ok(())
}

fn set_number(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: u64,
) -> std::io::Result<()> {
    let target = object
        .get_mut(key)
        .ok_or_else(|| io_invalid("scale template version field absent"))?;
    *target = serde_json::Value::Number(value.into());
    Ok(())
}

fn checked_scale_u64(value: u128) -> std::io::Result<u64> {
    u64::try_from(value).map_err(|_| io_invalid("weighted scale forecast exceeds u64"))
}

fn io_invalid(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

fn selected_quantile_index_v1(bucket: u64) -> usize {
    if bucket < SCALE_SELECTED_QUANTILE_BUCKETS_V1[0] {
        0
    } else if bucket
        < SCALE_SELECTED_QUANTILE_BUCKETS_V1[0] + SCALE_SELECTED_QUANTILE_BUCKETS_V1[1]
    {
        1
    } else {
        2
    }
}

fn selected_quantile_counts_v1(count: u64) -> std::io::Result<[u64; 3]> {
    let cycles = count / SCALE_SELECTED_QUANTILE_PERIOD_V1;
    let tail = count % SCALE_SELECTED_QUANTILE_PERIOD_V1;
    let p50 = SCALE_SELECTED_QUANTILE_BUCKETS_V1[0];
    let p95 = SCALE_SELECTED_QUANTILE_BUCKETS_V1[1];
    let maximum = SCALE_SELECTED_QUANTILE_BUCKETS_V1[2];
    let p50_count = cycles
        .checked_mul(p50)
        .and_then(|rows| rows.checked_add(tail.min(p50)))
        .ok_or_else(|| io_invalid("weighted p50 selection count overflow"))?;
    let p95_count = cycles
        .checked_mul(p95)
        .and_then(|rows| rows.checked_add(tail.saturating_sub(p50).min(p95)))
        .ok_or_else(|| io_invalid("weighted p95 selection count overflow"))?;
    let max_count = cycles
        .checked_mul(maximum)
        .and_then(|rows| rows.checked_add(tail.saturating_sub(p50 + p95)))
        .ok_or_else(|| io_invalid("weighted maximum selection count overflow"))?;
    Ok([p50_count, p95_count, max_count])
}

#[derive(Clone, Copy)]
struct ScaleTemplatePinV1 {
    class: WeightedScaleClassV1,
    class_index: usize,
    path: &'static str,
    file_sha256: &'static str,
    line: Option<u32>,
    template_sha256: &'static str,
}

fn scale_template_pins_v1() -> [ScaleTemplatePinV1; 15] {
    use WeightedScaleClassV1 as C;
    [
        ScaleTemplatePinV1 {
            class: C::Artifact,
            class_index: C::Artifact as usize,
            path: "ToS/source-witnesses/artifacts/egyptian/deir-el-medina/museo-egizio-cgt-54014/artifact-witness.json",
            file_sha256: "fd628326efae6b11901337747981707ef2b234d3113e0b27a97d27e4ffb92928",
            line: None,
            template_sha256: "fd628326efae6b11901337747981707ef2b234d3113e0b27a97d27e4ffb92928",
        },
        ScaleTemplatePinV1 {
            class: C::Artifact,
            class_index: C::Artifact as usize,
            path: "ToS/source-witnesses/artifacts/egyptian/unknown/papyrus-berlin-p3024/artifact-witness.json",
            file_sha256: "578e9b59a80c94c1a4702210ae111997dcad5ec232ec67fde66b8cc1c0a78ddf",
            line: None,
            template_sha256: "578e9b59a80c94c1a4702210ae111997dcad5ec232ec67fde66b8cc1c0a78ddf",
        },
        ScaleTemplatePinV1 {
            class: C::Artifact,
            class_index: C::Artifact as usize,
            path: "ToS/source-witnesses/artifacts/old-babylonian/susa/hammurabi-stele-sb-8/artifact-witness.json",
            file_sha256: "0a717b8b46580effeb6958d2dcc575af979e144e271d9656974236524df786fe",
            line: None,
            template_sha256: "0a717b8b46580effeb6958d2dcc575af979e144e271d9656974236524df786fe",
        },
        ScaleTemplatePinV1 {
            class: C::Claim,
            class_index: C::Claim as usize,
            path: "ToS/source-witnesses/relations/edition-item/edition-item-claims.jsonl",
            file_sha256: "c1e160b418d9ef43a7ec15b1ab7696d924397458816d645219888f028b4b0a5e",
            line: Some(7),
            template_sha256: "c7689a82b91318821ce496d2bfd118936163bf2040ae859df3b01293c39920b4",
        },
        ScaleTemplatePinV1 {
            class: C::Claim,
            class_index: C::Claim as usize,
            path: "ToS/source-witnesses/relations/nietzsche-letter-705/source-claims.jsonl",
            file_sha256: "d6523351bb089239112eb9a7a5b8a4579e62b7250fafd84a8c070739966011cc",
            line: Some(3),
            template_sha256: "014b5f4c64814b4cd3e81ee8db32e52fb7e135c395db742834a5d364b89e6b6b",
        },
        ScaleTemplatePinV1 {
            class: C::Claim,
            class_index: C::Claim as usize,
            path: "ToS/source-witnesses/relations/mysl-1996-volume-2-member-order/source-claims.jsonl",
            file_sha256: "e1e02155f0f4bc662d809af785a81245c9cdebcc4f91015f137fc13fd47b8397",
            line: Some(1),
            template_sha256: "e1e02155f0f4bc662d809af785a81245c9cdebcc4f91015f137fc13fd47b8397",
        },
        ScaleTemplatePinV1 {
            class: C::EvidencePacket,
            class_index: C::EvidencePacket as usize,
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.antonovsky-2007-p011-opening-sentence-proposal.v1.json",
            file_sha256: "ab0f4a5a971d2500561e99a2ac0bb8b4f069e08876892961ba46b4f98a2aa93f",
            line: None,
            template_sha256: "ab0f4a5a971d2500561e99a2ac0bb8b4f069e08876892961ba46b4f98a2aa93f",
        },
        ScaleTemplatePinV1 {
            class: C::EvidencePacket,
            class_index: C::EvidencePacket as usize,
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-layout.v1.json",
            file_sha256: "eb1947576475abe92370e3b233fd0aa24f1e36e3536ee5e499ccc6a71a612081",
            line: None,
            template_sha256: "eb1947576475abe92370e3b233fd0aa24f1e36e3536ee5e499ccc6a71a612081",
        },
        ScaleTemplatePinV1 {
            class: C::EvidencePacket,
            class_index: C::EvidencePacket as usize,
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-section.dta-authored-crosswalk.v1.json",
            file_sha256: "123ba3ad10907740322acd176aa3abc05a81739f5412525f5063021c39aaa018",
            line: None,
            template_sha256: "123ba3ad10907740322acd176aa3abc05a81739f5412525f5063021c39aaa018",
        },
        ScaleTemplatePinV1 {
            class: C::TextUnit,
            class_index: C::TextUnit as usize,
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.antonovsky-2007-p011-opening-sentence-proposal.v1.json",
            file_sha256: "ab0f4a5a971d2500561e99a2ac0bb8b4f069e08876892961ba46b4f98a2aa93f",
            line: None,
            template_sha256: "ab0f4a5a971d2500561e99a2ac0bb8b4f069e08876892961ba46b4f98a2aa93f",
        },
        ScaleTemplatePinV1 {
            class: C::TextUnit,
            class_index: C::TextUnit as usize,
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-layout.v1.json",
            file_sha256: "eb1947576475abe92370e3b233fd0aa24f1e36e3536ee5e499ccc6a71a612081",
            line: None,
            template_sha256: "eb1947576475abe92370e3b233fd0aa24f1e36e3536ee5e499ccc6a71a612081",
        },
        ScaleTemplatePinV1 {
            class: C::TextUnit,
            class_index: C::TextUnit as usize,
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-section.dta-authored-crosswalk.v1.json",
            file_sha256: "123ba3ad10907740322acd176aa3abc05a81739f5412525f5063021c39aaa018",
            line: None,
            template_sha256: "123ba3ad10907740322acd176aa3abc05a81739f5412525f5063021c39aaa018",
        },
        ScaleTemplatePinV1 {
            class: C::Work,
            class_index: C::Work as usize,
            path: "ToS/source-witnesses/works/old-babylonian-scholarship/sumerian-extract-tablets-and-scribal-education/work.json",
            file_sha256: "e4245f59451299b880450cd22484f338826a913e5986ba44dffd234215be5aee",
            line: None,
            template_sha256: "e4245f59451299b880450cd22484f338826a913e5986ba44dffd234215be5aee",
        },
        ScaleTemplatePinV1 {
            class: C::Work,
            class_index: C::Work as usize,
            path: "ToS/source-witnesses/works/mesopotamian-scholarship/ancient-mesopotamia-speaks-highlights-of-the-yale-babylonian-collection/work.json",
            file_sha256: "aec51ee9e1ba8773ad7a9ffd2bc0be26347ee9c332f62c328f618f270b8cb8e5",
            line: None,
            template_sha256: "aec51ee9e1ba8773ad7a9ffd2bc0be26347ee9c332f62c328f618f270b8cb8e5",
        },
        ScaleTemplatePinV1 {
            class: C::Work,
            class_index: C::Work as usize,
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json",
            file_sha256: "f2eb1fa908f98c25e2d7bbbeaaaa49ced810a912d49bdc7c6cfa2822ca731b51",
            line: None,
            template_sha256: "f2eb1fa908f98c25e2d7bbbeaaaa49ced810a912d49bdc7c6cfa2822ca731b51",
        },
    ]
}

fn template_manifest_digest_v1(
    templates: &[Vec<WeightedScaleTemplateV1>; 5],
) -> std::io::Result<Digest256> {
    let rows = templates
        .iter()
        .flat_map(|rows| rows.iter())
        .map(|row| {
            serde_json::json!({
                "class": format!("{:?}", row.class),
                "source_path": row.source_path,
                "source_sha256": row.source_sha256.to_hex(),
                "template_sha256": row.template_sha256.to_hex(),
                "template_bytes": row.bytes.len(),
            })
        })
        .collect::<Vec<_>>();
    let raw = serde_json::to_vec(&serde_json::json!({
        "schema": "tos_native_scale_template_manifest_v1",
        "template_source_commit": SCALE_TEMPLATE_SOURCE_COMMIT_V1,
        "templates": rows,
    }))
    .map_err(|_| io_invalid("scale template manifest encoding failed"))?;
    let document = parse_json(&raw, JsonMode::RequestLastWins, JsonLimits::default())
        .map_err(|_| io_invalid("scale template manifest parse failed"))?;
    let canonical = canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits::default(),
    )
    .map_err(|_| io_invalid("scale template manifest canonicalization failed"))?;
    Ok(Digest256::of_bytes(&canonical))
}

/// All resource ceilings are copied from the selected Native invocation. The
/// producer checks its conservative pre-write scratch envelope against these
/// values and the supplied shared ledgers before creating the private root.
pub(crate) struct WeightedScaleProducerRequestV1<'a> {
    pub(crate) repository_root: &'a Path,
    pub(crate) raw_input_root: &'a Path,
    pub(crate) output_root: &'a Path,
    pub(crate) profile: WeightedScaleProfileV1,
    pub(crate) segment_limits: SegmentLimits,
    pub(crate) member_tree_limits: AuthenticatedTreeLimitsV1,
    pub(crate) object_limits: PackedObjectLimitsV2,
    pub(crate) max_member_bytes: u64,
    pub(crate) max_source_bytes: u64,
    pub(crate) max_raw_input_files: u64,
    pub(crate) max_raw_input_directories: u64,
    pub(crate) max_raw_input_allocated_bytes: u64,
    pub(crate) max_temporary_logical_bytes: u64,
    pub(crate) max_temporary_allocated_bytes: u64,
    pub(crate) max_temporary_inodes: u64,
    pub(crate) max_working_state_bytes: usize,
    pub(crate) caller_live_state_bytes: usize,
    pub(crate) deadline: Instant,
    pub(crate) cancelled: &'a AtomicBool,
    /// This is the same handle retained by the Native source operation.
    pub(crate) work: AdmissionWorkBudget,
    /// This is the same physical-space pool used by the selected operation.
    pub(crate) space: PinnedSqliteSpaceBudget,
    /// This is the same IO/allocation adapter used by native V2 storage work.
    pub(crate) tree_io: Arc<NativeV2TreeIo>,
}

/// A completed private packed input. The descriptors and byte hashes are
/// mechanically derived from this held store. The receipt grants no source,
/// review, rights, canon, or admission authority.
pub(crate) struct PackedScaleInputReceiptV1 {
    pub(crate) raw_input_root: PathBuf,
    pub(crate) held_raw_input_root: File,
    pub(crate) raw_input_file_count: u64,
    pub(crate) raw_input_directory_count: u64,
    pub(crate) raw_input_inode_count: u64,
    pub(crate) raw_input_source_bytes: u64,
    pub(crate) raw_input_allocated_bytes: u64,
    pub(crate) named_root: PathBuf,
    pub(crate) held_root: File,
    pub(crate) segment: SegmentStore,
    pub(crate) profile_sha256: Digest256,
    pub(crate) dependency_closure_sha256: Digest256,
    pub(crate) manifest_sha256: Digest256,
    pub(crate) members_descriptor: AuthenticatedTreeDescriptorV2,
    pub(crate) members_descriptor_sha256: Digest256,
    pub(crate) objects_descriptor: AuthenticatedTreeDescriptorV2,
    pub(crate) objects_descriptor_sha256: Digest256,
    pub(crate) member_count: u64,
    pub(crate) source_bytes: u64,
    pub(crate) class_source_bytes: [u64; 5],
    pub(crate) unique_object_count: u64,
    pub(crate) unique_payload_bytes: u64,
    pub(crate) max_frames_per_pack: u32,
    pub(crate) generated_dependency_edges: u64,
    pub(crate) pinned_external_dependency_edges: u64,
    pub(crate) unresolved_dependency_edges: u64,
    pub(crate) member_tree_work: AuthenticatedTreeWorkV1,
    pub(crate) object_build_work: PackedObjectBuildWorkV2,
    pub(crate) measured_temporary_logical_bytes: u64,
    pub(crate) measured_temporary_allocated_peak_bytes: u64,
    pub(crate) measured_temporary_file_inode_peak: u64,
    pub(crate) forecast: WeightedScaleForecastInputsV1,
}

/// Materialize one bounded 100K input under the selected Native resource
/// envelope. The manifest is written last; failures leave no selectable input.
pub(crate) fn produce_weighted_scale_input_v1(
    request: WeightedScaleProducerRequestV1<'_>,
    before_manifest: &mut dyn FnMut() -> std::io::Result<()>,
) -> std::io::Result<PackedScaleInputReceiptV1> {
    let profile = request.profile.clone();
    profile.validate()?;
    if profile != WeightedScaleProfileV1::fixed_100k(profile.seed) {
        return Err(io_invalid(
            "weighted producer profile is not the frozen 100K mix",
        ));
    }
    if request.output_root.exists()
        || request.raw_input_root.exists()
        || !request.output_root.is_absolute()
        || !request.raw_input_root.is_absolute()
        || request.output_root.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || request.raw_input_root.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || request.max_member_bytes == 0
        || request.max_member_bytes == u64::MAX
        || request.max_source_bytes == 0
        || request.max_source_bytes == u64::MAX
        || request.max_raw_input_files == 0
        || request.max_raw_input_files == u64::MAX
        || request.max_raw_input_directories == 0
        || request.max_raw_input_directories == u64::MAX
        || request.max_raw_input_allocated_bytes == 0
        || request.max_raw_input_allocated_bytes == u64::MAX
        || request.max_temporary_logical_bytes == 0
        || request.max_temporary_logical_bytes == u64::MAX
        || request.max_temporary_allocated_bytes == 0
        || request.max_temporary_allocated_bytes == u64::MAX
        || request.max_temporary_inodes == 0
        || request.max_temporary_inodes == u64::MAX
        || request.max_working_state_bytes == 0
        || request.max_working_state_bytes == usize::MAX
        || request.caller_live_state_bytes >= request.max_working_state_bytes
        || request.tree_io.max_working_state_bytes() != request.max_working_state_bytes
        || request.member_tree_limits.max_rows < profile.target_records
        || request.member_tree_limits.max_key_bytes < path_for(WeightedScaleClassV1::Work, 0).len()
        || request.member_tree_limits.max_value_bytes < SCALE_MEMBER_VALUE_BYTES_V1
        || request.member_tree_limits.max_total_bytes == 0
        || request.member_tree_limits.max_total_bytes == u64::MAX
        || request.object_limits.tree_limits.max_rows < profile.target_records
        || request.object_limits.max_objects < profile.target_records
        || request.object_limits.max_working_state_bytes != request.max_working_state_bytes
    {
        return Err(io_invalid("weighted producer limits or destination differ"));
    }
    verify_output_location_outside_repository_v1(request.repository_root, request.output_root)?;
    verify_output_location_outside_repository_v1(request.repository_root, request.raw_input_root)?;
    if request.raw_input_root.starts_with(request.output_root)
        || request.output_root.starts_with(request.raw_input_root)
    {
        return Err(io_invalid("weighted raw and packed output roots overlap"));
    }

    let segment_limits = request
        .segment_limits
        .validate()
        .map_err(|_| io_invalid("weighted producer segment limits are invalid"))?;
    let mut forecast = profile.forecast_inputs()?;
    let allocation_unit = request.tree_io.selected_allocation_unit_bytes();
    let envelope = weighted_scale_producer_envelope_v1(&profile, allocation_unit)?;
    let maximum_source_bytes = envelope.maximum_source_bytes;
    let run_count = sort_run_count_v1(profile.target_records)?;
    let temporary_logical_upper = envelope.temporary_logical_bytes;
    let temporary_allocated_upper = envelope.temporary_allocated_bytes;
    let inode_upper = envelope.temporary_file_inodes;
    let raw_allocation_upper = envelope.raw_input_allocated_bytes;
    let raw_directory_upper = forecast.raw_input_directory_count_100k;
    let selected_state = u64::try_from(request.max_working_state_bytes)
        .map_err(|_| io_invalid("selected Native state ceiling exceeds u64"))?;
    forecast.selected_native_state_bytes_per_client = Some(selected_state);
    forecast.read_clients_state_upper_bytes = Some(
        selected_state
            .checked_mul(forecast.expected_read_clients as u64)
            .ok_or_else(|| io_invalid("256-client read state forecast overflow"))?,
    );
    forecast.writer_staging_upper_bytes_per_client = Some(envelope.temporary_allocated_bytes);
    forecast.write_clients_staging_upper_bytes = Some(
        envelope
            .temporary_allocated_bytes
            .checked_mul(forecast.expected_write_clients as u64)
            .ok_or_else(|| io_invalid("256-client writer staging forecast overflow"))?,
    );
    forecast.writer_callback_state_upper_bytes_per_client = Some(selected_state);
    forecast.write_clients_callback_state_upper_bytes = Some(
        selected_state
            .checked_mul(forecast.expected_write_clients as u64)
            .ok_or_else(|| io_invalid("256-client callback state forecast overflow"))?,
    );
    forecast.full_256_peak_established = false;
    if maximum_source_bytes > request.max_source_bytes
        || profile.target_records > request.max_raw_input_files
        || raw_directory_upper > request.max_raw_input_directories
        || raw_allocation_upper > request.max_raw_input_allocated_bytes
        || temporary_logical_upper > request.max_temporary_logical_bytes
        || temporary_allocated_upper > request.max_temporary_allocated_bytes
        || inode_upper > request.max_temporary_inodes
        || request.object_limits.max_pack_frames == 0
        || request.object_limits.max_pack_frames == u32::MAX
    {
        return Err(io_invalid(
            "weighted producer exceeds the selected pre-write envelope",
        ));
    }
    scale_active(request.deadline, request.cancelled)?;
    request.work.charge_many(1)?;
    let scratch_reservation = request
        .space
        .reserve(temporary_allocated_upper)
        .map_err(|_| io_invalid("weighted producer scratch reservation refused"))?;

    let io_budget = request.tree_io.io_budget().clone();
    let base_profile_bytes = read_source_file_accounted_v1(
        request
            .repository_root
            .join(SCALE_BASE_PROFILE_REF_V1)
            .as_path(),
        SCALE_MAX_PROFILE_BYTES_V1,
        &io_budget,
        request.deadline,
        request.cancelled,
        &request.work,
    )?;
    let base_profile_sha256 = Digest256::of_bytes(&base_profile_bytes);
    if base_profile_sha256.to_hex() != SCALE_BASE_PROFILE_SHA256_V1 {
        return Err(io_invalid("base scale profile source identity differs"));
    }
    let templates = WeightedScaleTemplateSetV1::load_accounted(
        request.repository_root,
        &io_budget,
        request.deadline,
        request.cancelled,
        &request.work,
    )?;
    let producer_state = weighted_producer_state_upper_v1(&templates, run_count)?;
    if producer_state >= request.max_working_state_bytes {
        return Err(io_invalid(
            "weighted producer retained state exceeds selected bound",
        ));
    }
    let profile_bytes = effective_profile_bytes_v1(&profile, base_profile_sha256, &forecast)?;
    if profile_bytes.len() > SCALE_MAX_PROFILE_BYTES_V1 {
        return Err(io_invalid(
            "effective weighted profile exceeds sidecar bound",
        ));
    }
    let profile_sha256 = Digest256::of_bytes(&profile_bytes);

    create_private_dir_v1(request.output_root)?;
    let held_root = open_private_dir_v1(request.output_root)?;
    let root_stamp = ScaleDirectoryStampV1::from_file(&held_root)?;
    let segment_path = request.output_root.join("segment");
    create_private_dir_v1(&segment_path)?;
    let segment_root = open_private_dir_v1(&segment_path)?;
    let segment = SegmentStore::initialize_empty_at_with_io(
        &segment_root,
        SOURCE_ADMISSION_V2_DOMAIN,
        segment_limits,
        request.tree_io.clone(),
        request.deadline,
        request.cancelled,
    )
    .map_err(|_| io_invalid("weighted producer segment initialization failed"))?;
    if profile.classes.iter().any(|row| {
        row.max_bytes
            .checked_add(SCALE_IDENTITY_GROWTH_ALLOWANCE_V1)
            .is_none_or(|upper| {
                upper > request.max_member_bytes || upper > segment_limits.max_frame_bytes
            })
    }) {
        return Err(io_invalid(
            "weighted source member exceeds selected frame cap",
        ));
    }

    let scratch_path = request.output_root.join(".scale-source-spool-v1");
    let mut scratch = ScaleScratchLeaseV1::new(
        &scratch_path,
        scratch_reservation,
        request.max_temporary_logical_bytes,
        request.max_temporary_allocated_bytes,
        request.max_temporary_inodes,
    )?;
    let spool_path = scratch_path.join("payload.spool");
    let mut spool = open_new_private_file_v1(&spool_path)?;
    let raw_input_reserved = request
        .tree_io
        .reserve_file_allocation(raw_allocation_upper)
        .map_err(|_| io_invalid("weighted raw input allocation reservation refused"))?;
    let mut raw_fixture = match RawScaleFixtureV1::new(
        request.raw_input_root,
        raw_input_reserved,
        profile.target_records,
        raw_directory_upper,
        request.max_raw_input_files,
        request.max_raw_input_directories,
        request.max_raw_input_allocated_bytes,
    ) {
        Ok(raw) => raw,
        Err(error) => {
            let _ = request.tree_io.release_file_allocation(raw_input_reserved);
            return Err(error);
        }
    };
    let mut digest_runs = DigestRunWriterV1::new(SCALE_SORT_RUN_ROWS_V1)?;
    let mut member_cursor = WeightedScaleMemberIterV1::new(&profile, &templates)?;
    let mut member_rows = WeightedMemberTreeRowsV1 {
        cursor: &mut member_cursor,
        raw: &mut raw_fixture,
        spool: &mut spool,
        digest_runs: &mut digest_runs,
        scratch: &mut scratch,
        io: &io_budget,
        work: &request.work,
        deadline: request.deadline,
        cancelled: request.cancelled,
        expected_members: profile.target_records,
        max_member_bytes: request.max_member_bytes,
        max_source_bytes: request.max_source_bytes,
        max_raw_input_file_bytes: request.max_member_bytes,
        raw_io: &io_budget,
        raw_work: &request.work,
        raw_deadline: request.deadline,
        raw_cancelled: request.cancelled,
        max_key_bytes: request.member_tree_limits.max_key_bytes,
        max_temp_logical_bytes: request.max_temporary_logical_bytes,
        finished: false,
    };
    let member_additional_state = request
        .caller_live_state_bytes
        .checked_add(producer_state)
        .and_then(|bytes| bytes.checked_add(size_of::<AuthenticatedTreeWorkV1>()))
        .ok_or_else(|| io_invalid("weighted member-tree caller state overflow"))?;
    let mut member_tree_debit = {
        let work = request.work.clone();
        move || work.charge_many(1).is_ok()
    };
    let (members_descriptor, member_tree_work) = segment
        .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
            MEMBERS_KIND,
            &mut member_rows,
            request.member_tree_limits,
            Some(request.tree_io.clone()),
            request.max_working_state_bytes,
            member_additional_state,
            request.deadline,
            request.cancelled,
            &mut member_tree_debit,
        )
        .map_err(|_| io_invalid("weighted member authenticated tree build failed"))?;
    member_rows.ensure_finished()?;
    if member_rows.cursor.member_count() != profile.target_records
        || member_rows.cursor.source_bytes() > request.max_source_bytes
        || members_descriptor.entries != profile.target_records
    {
        return Err(io_invalid("weighted member tree census differs"));
    }
    let member_count = member_rows.cursor.member_count();
    let source_bytes = member_rows.cursor.source_bytes();
    let class_source_bytes = member_rows.cursor.class_source_bytes();
    let closure = member_rows.cursor.closure().clone();
    drop(member_rows);
    let raw_input = raw_fixture.finish(&request.tree_io)?;
    if raw_input.file_count != member_count || raw_input.source_bytes != source_bytes {
        return Err(io_invalid(
            "raw source fixture differs from indexed member census",
        ));
    }
    digest_runs.finish(
        &mut scratch,
        &spool,
        &io_budget,
        &request.work,
        request.deadline,
        request.cancelled,
    )?;
    spool.sync_all()?;
    let spool = Arc::new(spool);

    let max_frames_per_pack = request
        .object_limits
        .max_pack_frames
        .min(MAX_PACKED_OBJECT_FRAMES_V2)
        .min(segment_limits.max_frames);
    let mut object_limits = request.object_limits;
    object_limits.max_pack_frames = max_frames_per_pack;
    object_limits.caller_live_state_bytes = request
        .caller_live_state_bytes
        .checked_add(producer_state)
        .ok_or_else(|| io_invalid("weighted object caller state overflow"))?;
    object_limits.max_work_units = object_limits.max_work_units.min(request.work.remaining()?);
    if object_limits.max_work_units == 0 {
        return Err(io_invalid("weighted object work ceiling is exhausted"));
    }
    let mut object_cursor = DigestSortedPackedSourcesV1::new(
        &scratch.run_paths,
        Arc::clone(&spool),
        io_budget.clone(),
        request.work.clone(),
        request.deadline,
        request.cancelled,
    )?;
    let mut object_tree_debit = {
        let work = request.work.clone();
        move || work.charge_many(1).is_ok()
    };
    let (objects_descriptor, object_build_work) = PackedObjectWriterV2::build(
        &segment,
        &mut object_cursor,
        object_limits,
        request.tree_io.clone(),
        request.deadline,
        request.cancelled,
        &mut object_tree_debit,
    )?;
    if !object_cursor.complete()
        || objects_descriptor.entries != object_build_work.object_rows
        || object_build_work.object_rows > profile.target_records
    {
        return Err(io_invalid("weighted object extent census differs"));
    }
    let unique_object_count = object_build_work.object_rows;
    let unique_payload_bytes = object_build_work.payload_bytes;
    drop(object_cursor);
    drop(spool);
    let measured_temporary_logical_bytes = scratch.logical_bytes()?;
    let measured_temporary_allocated_peak_bytes = scratch.peak_allocated_bytes;
    let measured_temporary_file_inode_peak = scratch.file_inode_peak;
    scratch.cleanup()?;

    let members_bytes = members_descriptor
        .encode(12 * 1024)
        .map_err(|_| io_invalid("member descriptor encoding failed"))?;
    let objects_bytes = objects_descriptor
        .encode(12 * 1024)
        .map_err(|_| io_invalid("object descriptor encoding failed"))?;
    let members_descriptor_sha256 = Digest256::of_bytes(&members_bytes);
    let objects_descriptor_sha256 = Digest256::of_bytes(&objects_bytes);
    let closure_bytes = dependency_closure_bytes_v1(profile_sha256, &closure)?;
    if closure_bytes.len() > SCALE_MAX_CLOSURE_BYTES_V1 || closure.unresolved_dependency_edges != 0
    {
        return Err(io_invalid(
            "weighted dependency closure is incomplete or too large",
        ));
    }
    let dependency_closure_sha256 = Digest256::of_bytes(&closure_bytes);
    write_private_leaf_accounted_v1(
        request.output_root,
        SCALE_PROFILE_REF_V1,
        &profile_bytes,
        &request.tree_io,
        request.deadline,
        request.cancelled,
        &request.work,
    )?;
    write_private_leaf_accounted_v1(
        request.output_root,
        SCALE_DEPENDENCY_CLOSURE_LEAF_V1,
        &closure_bytes,
        &request.tree_io,
        request.deadline,
        request.cancelled,
        &request.work,
    )?;
    write_private_leaf_accounted_v1(
        request.output_root,
        SCALE_INPUT_MEMBERS_LEAF_V1,
        &members_bytes,
        &request.tree_io,
        request.deadline,
        request.cancelled,
        &request.work,
    )?;
    write_private_leaf_accounted_v1(
        request.output_root,
        SCALE_INPUT_OBJECTS_LEAF_V1,
        &objects_bytes,
        &request.tree_io,
        request.deadline,
        request.cancelled,
        &request.work,
    )?;
    let manifest_bytes = scale_input_manifest_bytes_v1(
        profile.seed,
        templates.manifest_sha256,
        member_count,
        source_bytes,
        members_descriptor_sha256,
        objects_descriptor_sha256,
        unique_object_count,
        unique_payload_bytes,
        max_frames_per_pack,
        profile_sha256,
        dependency_closure_sha256,
        &closure,
    )?;
    if manifest_bytes.len() > SCALE_MAX_MANIFEST_BYTES_V1 {
        return Err(io_invalid("weighted input manifest exceeds sidecar bound"));
    }
    before_manifest()?;
    verify_private_directory_v1(request.output_root, &held_root, root_stamp)?;
    let (segment_device, segment_inode) = segment
        .physical_root_identity()
        .map_err(|_| io_invalid("weighted segment root identity unavailable"))?;
    let segment_stamp = ScaleDirectoryStampV1::from_file(&segment_root)?;
    if (segment_device, segment_inode) != (segment_stamp.device, segment_stamp.inode) {
        return Err(io_invalid("weighted segment root identity changed"));
    }
    write_private_leaf_accounted_v1(
        request.output_root,
        SCALE_INPUT_MANIFEST_LEAF_V1,
        &manifest_bytes,
        &request.tree_io,
        request.deadline,
        request.cancelled,
        &request.work,
    )?;
    verify_private_directory_v1(request.output_root, &held_root, root_stamp)?;
    verify_private_directory_v1(
        request.raw_input_root,
        &raw_input.held_root,
        raw_input.root_stamp,
    )?;
    Ok(PackedScaleInputReceiptV1 {
        raw_input_root: request.raw_input_root.to_path_buf(),
        held_raw_input_root: raw_input.held_root,
        raw_input_file_count: raw_input.file_count,
        raw_input_directory_count: raw_input.directory_count,
        raw_input_inode_count: raw_input
            .file_count
            .checked_add(raw_input.directory_count)
            .ok_or_else(|| io_invalid("raw source inode count overflow"))?,
        raw_input_source_bytes: raw_input.source_bytes,
        raw_input_allocated_bytes: raw_input.allocated_bytes,
        named_root: request.output_root.to_path_buf(),
        held_root,
        segment,
        profile_sha256,
        dependency_closure_sha256,
        manifest_sha256: Digest256::of_bytes(&manifest_bytes),
        members_descriptor: members_descriptor.clone(),
        members_descriptor_sha256,
        objects_descriptor: objects_descriptor.clone(),
        objects_descriptor_sha256,
        member_count,
        source_bytes,
        class_source_bytes,
        unique_object_count,
        unique_payload_bytes,
        max_frames_per_pack,
        generated_dependency_edges: closure.generated_dependency_edges,
        pinned_external_dependency_edges: closure.pinned_external_dependency_edges,
        unresolved_dependency_edges: closure.unresolved_dependency_edges,
        member_tree_work,
        object_build_work,
        measured_temporary_logical_bytes,
        measured_temporary_allocated_peak_bytes,
        measured_temporary_file_inode_peak,
        forecast,
    })
}

fn weighted_source_upper_bound_v1(profile: &WeightedScaleProfileV1) -> std::io::Result<u64> {
    let mut total = 0u128;
    for row in profile.classes {
        let selected_counts = selected_quantile_counts_v1(row.count)?;
        for (count, template_bytes) in selected_counts
            .into_iter()
            .zip([row.p50_bytes, row.p95_bytes, row.max_bytes])
        {
            let member_upper = template_bytes
                .checked_add(SCALE_IDENTITY_GROWTH_ALLOWANCE_V1)
                .ok_or_else(|| io_invalid("weighted source member growth overflow"))?;
            total = total
                .checked_add((count as u128) * (member_upper as u128))
                .ok_or_else(|| io_invalid("weighted source upper bound overflow"))?;
        }
    }
    checked_scale_u64(total)
}

fn raw_fixture_allocation_upper_v1(
    profile: &WeightedScaleProfileV1,
    allocation_unit: u64,
) -> std::io::Result<u64> {
    if allocation_unit == 0 || allocation_unit == u64::MAX {
        return Err(io_invalid("weighted raw allocation unit is invalid"));
    }
    let mut files = 0u128;
    for row in &profile.classes {
        let selected_counts = selected_quantile_counts_v1(row.count)?;
        for (count, template_bytes) in selected_counts
            .into_iter()
            .zip([row.p50_bytes, row.p95_bytes, row.max_bytes])
        {
            let logical_upper = template_bytes
                .checked_add(SCALE_IDENTITY_GROWTH_ALLOWANCE_V1)
                .ok_or_else(|| io_invalid("weighted raw member allocation overflow"))?;
            let per_file = round_up_scale_v1(logical_upper, allocation_unit)?
                .checked_add(allocation_unit)
                .ok_or_else(|| io_invalid("weighted raw per-file allocation overflow"))?;
            files = files
                .checked_add((count as u128) * (per_file as u128))
                .ok_or_else(|| io_invalid("weighted raw file allocation overflow"))?;
        }
    }
    let directories = profile
        .target_records
        .checked_add(19)
        .ok_or_else(|| io_invalid("weighted raw directory allocation count overflow"))?;
    let directory_bytes = (directories as u128)
        .checked_mul((allocation_unit as u128) * 2)
        .ok_or_else(|| io_invalid("weighted raw directory allocation overflow"))?;
    checked_scale_u64(
        files
            .checked_add(directory_bytes)
            .ok_or_else(|| io_invalid("weighted raw allocation sum overflow"))?,
    )
}

struct RawScaleFixtureReceiptV1 {
    held_root: File,
    root_stamp: ScaleDirectoryStampV1,
    file_count: u64,
    directory_count: u64,
    source_bytes: u64,
    allocated_bytes: u64,
}

/// Writes the exact same typed member bytes that feed the authenticated
/// member tree. The root remains an ordinary filesystem source cut so the
/// admission caller's source census, not the sidecar manifest, establishes
/// the member tuples.
struct RawScaleFixtureV1 {
    root_path: PathBuf,
    root: File,
    root_stamp: ScaleDirectoryStampV1,
    current_class_root: Option<PathBuf>,
    fixed_directories: Vec<PathBuf>,
    file_count: u64,
    directory_count: u64,
    source_bytes: u64,
    allocated_bytes: u64,
    expected_files: u64,
    expected_directories: u64,
    max_files: u64,
    max_directories: u64,
    max_allocated_bytes: u64,
    allocation_reservation: u64,
}

impl RawScaleFixtureV1 {
    #[allow(clippy::too_many_arguments)]
    fn new(
        root_path: &Path,
        allocation_reservation: u64,
        expected_files: u64,
        expected_directories: u64,
        max_files: u64,
        max_directories: u64,
        max_allocated_bytes: u64,
    ) -> std::io::Result<Self> {
        create_private_dir_v1(root_path)?;
        let root = open_private_dir_v1(root_path)?;
        let root_stamp = ScaleDirectoryStampV1::from_file(&root)?;
        Ok(Self {
            root_path: root_path.to_path_buf(),
            root,
            root_stamp,
            current_class_root: None,
            fixed_directories: vec![root_path.to_path_buf()],
            file_count: 0,
            directory_count: 1,
            source_bytes: 0,
            allocated_bytes: 0,
            expected_files,
            expected_directories,
            max_files,
            max_directories,
            max_allocated_bytes,
            allocation_reservation,
        })
    }

    fn write_member(
        &mut self,
        member: &WeightedScaleMemberV1,
        io: &PinnedSqliteIoBudget,
        work: &AdmissionWorkBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<()> {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        let relative = RelativePath::parse(&member.path)
            .map_err(|_| io_invalid("weighted raw source path is invalid"))?;
        let mut components = relative.as_str().split('/').collect::<Vec<_>>();
        if components.len() < 3 {
            return Err(io_invalid("weighted raw source path is too shallow"));
        }
        let filename = components
            .pop()
            .ok_or_else(|| io_invalid("weighted raw source filename absent"))?;
        let ordinal = components
            .pop()
            .ok_or_else(|| io_invalid("weighted raw source ordinal directory absent"))?;
        if ordinal.len() != 20 || !ordinal.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(io_invalid("weighted raw source ordinal directory differs"));
        }
        let class_root_relative = components.join("/");
        let class_root = self.root_path.join(&class_root_relative);
        if self.current_class_root.as_ref() != Some(&class_root) {
            self.ensure_directories(&components, work, deadline, cancelled)?;
            self.current_class_root = Some(class_root.clone());
        }
        let ordinal_directory = class_root.join(ordinal);
        if self.directory_count >= self.max_directories {
            return Err(io_invalid("weighted raw source directory cap exhausted"));
        }
        let mut builder = DirBuilder::new();
        builder.mode(0o755);
        builder.create(&ordinal_directory)?;
        self.directory_count = self
            .directory_count
            .checked_add(1)
            .ok_or_else(|| io_invalid("weighted raw directory count overflow"))?;

        if self.file_count >= self.max_files {
            return Err(io_invalid("weighted raw source file cap exhausted"));
        }
        let member_size = u64::try_from(member.source_bytes.len())
            .map_err(|_| io_invalid("weighted raw source size exceeds u64"))?;
        let next_source_bytes = self
            .source_bytes
            .checked_add(member_size)
            .ok_or_else(|| io_invalid("weighted raw source byte count overflow"))?;
        let output_path = ordinal_directory.join(filename);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(member.mode)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&output_path)?;
        file.set_permissions(fs::Permissions::from_mode(member.mode))?;
        write_all_accounted_v1(
            &mut file,
            &member.source_bytes,
            io,
            deadline,
            cancelled,
            work,
        )?;
        file.sync_all()?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.len() != member_size
            || metadata.mode() & 0o777 != member.mode
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err(io_invalid("weighted raw source file identity differs"));
        }
        self.add_allocated(metadata.blocks().saturating_mul(512))?;
        self.add_directory_allocation(&ordinal_directory)?;
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(&ordinal_directory)?;
        parent.sync_all()?;
        self.file_count = self
            .file_count
            .checked_add(1)
            .ok_or_else(|| io_invalid("weighted raw source file count overflow"))?;
        self.source_bytes = next_source_bytes;
        Ok(())
    }

    fn ensure_directories(
        &mut self,
        components: &[&str],
        work: &AdmissionWorkBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<()> {
        let mut current = self.root_path.clone();
        for component in components {
            scale_active(deadline, cancelled)?;
            work.charge_many(1)?;
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(io_invalid("weighted raw fixture parent is unsafe"));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if self.directory_count >= self.max_directories {
                        return Err(io_invalid("weighted raw source directory cap exhausted"));
                    }
                    let mut builder = DirBuilder::new();
                    builder.mode(0o755);
                    builder.create(&current)?;
                    self.directory_count = self
                        .directory_count
                        .checked_add(1)
                        .ok_or_else(|| io_invalid("weighted raw directory count overflow"))?;
                    self.fixed_directories.push(current.clone());
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn add_directory_allocation(&mut self, path: &Path) -> std::io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io_invalid("weighted raw source directory changed"));
        }
        self.add_allocated(metadata.blocks().saturating_mul(512))
    }

    fn add_allocated(&mut self, bytes: u64) -> std::io::Result<()> {
        self.allocated_bytes = self
            .allocated_bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.max_allocated_bytes)
            .ok_or_else(|| io_invalid("weighted raw fixture allocation cap exceeded"))?;
        Ok(())
    }

    fn finish(self, tree_io: &NativeV2TreeIo) -> std::io::Result<RawScaleFixtureReceiptV1> {
        let fixed_directory_bytes =
            self.fixed_directories
                .iter()
                .try_fold(0u64, |total, path| {
                    let metadata = fs::symlink_metadata(path)?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(io_invalid("weighted raw fixed directory changed"));
                    }
                    total
                        .checked_add(metadata.blocks().saturating_mul(512))
                        .ok_or_else(|| io_invalid("weighted raw directory allocation overflow"))
                })?;
        let allocated_bytes = self
            .allocated_bytes
            .checked_add(fixed_directory_bytes)
            .filter(|total| *total <= self.max_allocated_bytes)
            .ok_or_else(|| io_invalid("weighted raw fixture allocation cap exceeded"))?;
        if self.file_count != self.expected_files
            || self.directory_count != self.expected_directories
            || self.file_count > self.max_files
            || self.directory_count > self.max_directories
            || allocated_bytes > self.max_allocated_bytes
        {
            return Err(io_invalid("weighted raw source fixture census differs"));
        }
        verify_private_directory_v1(&self.root_path, &self.root, self.root_stamp)?;
        self.root.sync_all()?;
        tree_io.reconcile_file_allocation(self.allocation_reservation, allocated_bytes)?;
        Ok(RawScaleFixtureReceiptV1 {
            held_root: self.root,
            root_stamp: self.root_stamp,
            file_count: self.file_count,
            directory_count: self.directory_count,
            source_bytes: self.source_bytes,
            allocated_bytes,
        })
    }
}

const SCALE_IDENTITY_GROWTH_ALLOWANCE_V1: u64 = 4096;

fn sort_run_count_v1(rows: u64) -> std::io::Result<u64> {
    rows.checked_add(SCALE_SORT_RUN_ROWS_V1 as u64 - 1)
        .map(|count| count / SCALE_SORT_RUN_ROWS_V1 as u64)
        .ok_or_else(|| io_invalid("weighted sort run count overflow"))
}

fn scratch_allocation_upper_v1(
    payload_bytes: u64,
    run_count: u64,
    allocation_unit: u64,
) -> std::io::Result<u64> {
    if allocation_unit == 0 || allocation_unit == u64::MAX {
        return Err(io_invalid("weighted producer allocation unit is invalid"));
    }
    let sort_bytes = (SCALE_SORT_RUN_ROWS_V1 as u64)
        .checked_mul(SCALE_SORT_ROW_BYTES_V1)
        .ok_or_else(|| io_invalid("weighted sort allocation overflow"))?;
    let full_run_alloc = round_up_scale_v1(sort_bytes, allocation_unit)?;
    let run_alloc = run_count
        .checked_mul(full_run_alloc)
        .ok_or_else(|| io_invalid("weighted run allocation upper bound overflow"))?;
    round_up_scale_v1(payload_bytes, allocation_unit)?
        .checked_add(run_alloc)
        .and_then(|bytes| bytes.checked_add(allocation_unit))
        .ok_or_else(|| io_invalid("weighted scratch allocation upper bound overflow"))
}

fn round_up_scale_v1(value: u64, unit: u64) -> std::io::Result<u64> {
    value
        .checked_add(unit - 1)
        .and_then(|sum| sum.checked_div(unit))
        .and_then(|blocks| blocks.checked_mul(unit))
        .ok_or_else(|| io_invalid("weighted allocation rounding overflow"))
}

fn weighted_producer_state_upper_v1(
    templates: &WeightedScaleTemplateSetV1,
    run_count: u64,
) -> std::io::Result<usize> {
    let runs = usize::try_from(run_count)
        .map_err(|_| io_invalid("weighted sort run count exceeds address space"))?;
    let run_state = runs
        .checked_mul(size_of::<File>() + size_of::<u64>() + size_of::<DigestRunEntryV1>())
        .ok_or_else(|| io_invalid("weighted sort cursor state overflow"))?;
    let mut closure_key_state = 0usize;
    for rows in &templates.templates {
        for template in rows {
            for edge in &template.external_reference_edges {
                // The sidecar map owns a second copy of each external key.
                // Include its string payloads, node/key overhead, and the
                // largest class label; source-template keys stay pinned.
                let key_bytes = edge
                    .pointer
                    .len()
                    .checked_add(edge.reference.len())
                    .and_then(|bytes| bytes.checked_add(template.source_path.len()))
                    .and_then(|bytes| bytes.checked_add(64 + "EvidencePacket".len() + 160))
                    .ok_or_else(|| io_invalid("weighted closure key state overflow"))?;
                closure_key_state = closure_key_state
                    .checked_add(key_bytes)
                    .ok_or_else(|| io_invalid("weighted closure state bound overflow"))?;
            }
        }
    }
    let template_bytes = usize::try_from(templates.resident_template_bytes)
        .map_err(|_| io_invalid("weighted template state exceeds address space"))?;
    let reference_state = usize::try_from(templates.resident_reference_state_bytes)
        .map_err(|_| io_invalid("weighted reference state exceeds address space"))?;
    template_bytes
        .checked_add(reference_state)
        .and_then(|bytes| bytes.checked_add(closure_key_state))
        .and_then(|bytes| bytes.checked_add(run_state))
        .and_then(|bytes| bytes.checked_add(SCALE_SORT_RUN_ROWS_V1 * size_of::<DigestRunEntryV1>()))
        .and_then(|bytes| bytes.checked_add(4 * SCALE_MAX_TEMPLATE_BYTES_V1))
        .and_then(|bytes| bytes.checked_add(256 * 1024))
        .and_then(|bytes| bytes.checked_add(size_of::<WeightedScaleMemberIterV1<'_>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<RawScaleFixtureV1>()))
        .and_then(|bytes| bytes.checked_add(size_of::<NativeV2TreeIo>()))
        .and_then(|bytes| bytes.checked_add(size_of::<SegmentStore>()))
        .ok_or_else(|| io_invalid("weighted producer state bound overflow"))
}

fn effective_profile_bytes_v1(
    profile: &WeightedScaleProfileV1,
    base_profile_sha256: Digest256,
    forecast: &WeightedScaleForecastInputsV1,
) -> std::io::Result<Vec<u8>> {
    let classes = profile
        .classes
        .iter()
        .map(|row| {
            serde_json::json!({
                "class": format!("{:?}", row.class),
                "count": row.count,
                "p50_bytes": row.p50_bytes,
                "p95_bytes": row.p95_bytes,
                "max_bytes": row.max_bytes,
            })
        })
        .collect::<Vec<_>>();
    canonical_value_bytes_v1(&serde_json::json!({
        "schema": "tos_native_scale_effective_profile_v1",
        "status": "working_hypothesis_no_admission",
        "template_source_commit": SCALE_TEMPLATE_SOURCE_COMMIT_V1,
        "base_profile_ref": SCALE_BASE_PROFILE_REF_V1,
        "base_profile_sha256": base_profile_sha256.to_hex(),
        "target_records": profile.target_records,
        "scenario": {
            "name": "weighted_100k_selected_quantiles",
            "p50_percent": 90,
            "p95_percent": 9,
            "max_percent": 1,
        },
        "classes": classes,
        "substitution": {
            "from_class": "evidence_bridge",
            "to_class": "EvidencePacket",
            "records": profile.classes[2].count,
            "reason": "unreviewed_source_text_unit_packet_schema_only",
        },
        "authored_route_bridge_coverage": SCALE_AUTHORED_BRIDGE_COVERAGE_V1,
        "authentic_bridge_form_ref": SCALE_AUTHENTIC_BRIDGE_FORM_REF_V1,
        "authentic_bridge_replay_route": SCALE_AUTHENTIC_BRIDGE_REPLAY_ROUTE_V1,
        "authentic_bridge_replay_scope": "one real pinned source specimen, separate from the 100K synthetic cohort",
        "history": {
            "revisions": forecast.revisions,
            "changed_rows_per_revision": forecast.changed_rows_per_revision,
            "policy_churn_rows_per_revision": forecast.policy_churn_rows_per_revision,
            "pinned_revisions": forecast.retained_pinned_revisions,
        },
        "graph": {
            "normal_out_degree": forecast.normal_out_degree,
            "hot_hub_out_degree": forecast.hot_hub_out_degree,
        },
        "concurrency": {
            "clients": forecast.concurrent_clients,
            "reads_percent": forecast.read_percent,
            "writes_percent": forecast.write_percent,
            "selected_native_state_bytes_per_client": forecast.selected_native_state_bytes_per_client,
            "read_clients_state_upper_bytes": forecast.read_clients_state_upper_bytes,
            "writer_staging_upper_bytes_per_client": forecast.writer_staging_upper_bytes_per_client,
            "write_clients_staging_upper_bytes": forecast.write_clients_staging_upper_bytes,
            "writer_callback_state_upper_bytes_per_client": forecast.writer_callback_state_upper_bytes_per_client,
            "write_clients_callback_state_upper_bytes": forecast.write_clients_callback_state_upper_bytes,
            "full_256_peak_established": forecast.full_256_peak_established,
        },
        "logical_source_forecast": {
            "p50_bytes_100k": forecast.p50_logical_source_bytes_100k,
            "selected_quantile_bytes_100k": forecast.selected_quantile_scenario_logical_source_bytes_100k,
            "p50_bytes_1b": forecast.p50_logical_source_bytes_1b,
            "selected_quantile_bytes_1b": forecast.selected_quantile_scenario_logical_source_bytes_1b,
            "unique_payload_ratio": null,
            "physical_fit_established": false,
        },
    }))
}

fn dependency_closure_bytes_v1(
    profile_sha256: Digest256,
    closure: &WeightedScaleClosureAccumulatorV1,
) -> std::io::Result<Vec<u8>> {
    let external_dependencies = closure
        .external_uses
        .iter()
        .map(|(key, count)| {
            serde_json::json!({
                "class": key.class,
                "source_path": key.source_path,
                "template_sha256": key.template_sha256,
                "pointer": key.pointer,
                "reference": key.reference,
                "use_count": count,
            })
        })
        .collect::<Vec<_>>();
    canonical_value_bytes_v1(&serde_json::json!({
        "schema": "tos_native_scale_dependency_closure_v1",
        "template_source_commit": SCALE_TEMPLATE_SOURCE_COMMIT_V1,
        "profile_sha256": profile_sha256.to_hex(),
        "generator_policy": "rewritten_fixture_ids_and_empty_reviews_unreviewed_provisional_local_only",
        "generator_agent_ref": SCALE_GENERATOR_AGENT_REF_V1,
        "generator_reference_role": "technical_fixture_maker_not_source_authority",
        "generator_reference_edges": closure.generator_reference_edges,
        "generated_dependency_edges": closure.generated_dependency_edges,
        "pinned_external_dependency_edges": closure.pinned_external_dependency_edges,
        "unresolved_dependency_edges": closure.unresolved_dependency_edges,
        "authored_route_bridge_coverage": SCALE_AUTHORED_BRIDGE_COVERAGE_V1,
        "authentic_bridge_form_ref": SCALE_AUTHENTIC_BRIDGE_FORM_REF_V1,
        "authentic_bridge_replay_route": SCALE_AUTHENTIC_BRIDGE_REPLAY_ROUTE_V1,
        "authentic_bridge_replay_scope": "one real pinned source specimen, separate from the 100K synthetic cohort",
        "external_dependencies": external_dependencies,
    }))
}

fn scale_input_manifest_bytes_v1(
    seed: Digest256,
    template_manifest_sha256: Digest256,
    member_count: u64,
    source_bytes: u64,
    members_descriptor_sha256: Digest256,
    objects_descriptor_sha256: Digest256,
    unique_object_count: u64,
    unique_payload_bytes: u64,
    max_frames_per_pack: u32,
    profile_sha256: Digest256,
    dependency_closure_sha256: Digest256,
    closure: &WeightedScaleClosureAccumulatorV1,
) -> std::io::Result<Vec<u8>> {
    canonical_value_bytes_v1(&serde_json::json!({
        "schema": SCALE_INPUT_SCHEMA_V1,
        "source_status": SCALE_INPUT_SOURCE_STATUS_V1,
        "profile_ref": SCALE_PROFILE_REF_V1,
        "profile_sha256": profile_sha256.to_hex(),
        "seed_sha256": seed.to_hex(),
        "template_source_commit": SCALE_TEMPLATE_SOURCE_COMMIT_V1,
        "template_manifest_sha256": template_manifest_sha256.to_hex(),
        "member_count": member_count,
        "source_bytes": source_bytes,
        "members_descriptor_leaf": SCALE_INPUT_MEMBERS_LEAF_V1,
        "members_descriptor_sha256": members_descriptor_sha256.to_hex(),
        "objects_descriptor_leaf": SCALE_INPUT_OBJECTS_LEAF_V1,
        "objects_descriptor_sha256": objects_descriptor_sha256.to_hex(),
        "unique_object_count": unique_object_count,
        "unique_payload_bytes": unique_payload_bytes,
        "max_frames_per_pack": max_frames_per_pack,
        "dependency_closure_sha256": dependency_closure_sha256.to_hex(),
        "generated_dependency_edges": closure.generated_dependency_edges,
        "pinned_external_dependency_edges": closure.pinned_external_dependency_edges,
        "unresolved_dependency_edges": closure.unresolved_dependency_edges,
        "authored_route_bridge_coverage": SCALE_AUTHORED_BRIDGE_COVERAGE_V1,
    }))
}

fn canonical_value_bytes_v1(value: &serde_json::Value) -> std::io::Result<Vec<u8>> {
    let raw = serde_json::to_vec(value)
        .map_err(|_| io_invalid("weighted metadata JSON encoding failed"))?;
    let document = parse_json(&raw, JsonMode::RequestLastWins, JsonLimits::default())
        .map_err(|_| io_invalid("weighted metadata JSON parse failed"))?;
    canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits::default(),
    )
    .map_err(|_| io_invalid("weighted metadata canonicalization failed"))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DigestRunEntryV1 {
    digest: [u8; 32],
    size: u64,
    offset: u64,
}

struct DigestRunWriterV1 {
    row_limit: usize,
    rows: Vec<DigestRunEntryV1>,
    rows_seen: u64,
    run_sequence: u64,
}

impl DigestRunWriterV1 {
    fn new(row_limit: usize) -> std::io::Result<Self> {
        if row_limit == 0 {
            return Err(io_invalid("weighted digest-run row limit is zero"));
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(row_limit)
            .map_err(|_| io_invalid("weighted digest-run row allocation failed"))?;
        Ok(Self {
            row_limit,
            rows,
            rows_seen: 0,
            run_sequence: 0,
        })
    }

    fn push(
        &mut self,
        row: DigestRunEntryV1,
        scratch: &mut ScaleScratchLeaseV1,
        spool: &File,
        io: &PinnedSqliteIoBudget,
        work: &AdmissionWorkBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<()> {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        self.rows_seen = self
            .rows_seen
            .checked_add(1)
            .ok_or_else(|| io_invalid("weighted digest-run input count overflow"))?;
        if self.rows.len() == self.row_limit {
            self.flush(scratch, spool, io, work, deadline, cancelled)?;
        }
        self.rows.push(row);
        if self.rows.len() == self.row_limit {
            self.flush(scratch, spool, io, work, deadline, cancelled)?;
        }
        Ok(())
    }

    fn finish(
        &mut self,
        scratch: &mut ScaleScratchLeaseV1,
        spool: &File,
        io: &PinnedSqliteIoBudget,
        work: &AdmissionWorkBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<()> {
        if !self.rows.is_empty() {
            self.flush(scratch, spool, io, work, deadline, cancelled)?;
        }
        if self.rows_seen != scratch.spool_rows {
            return Err(io_invalid(
                "weighted digest-run rows differ from spool census",
            ));
        }
        Ok(())
    }

    fn flush(
        &mut self,
        scratch: &mut ScaleScratchLeaseV1,
        _spool: &File,
        io: &PinnedSqliteIoBudget,
        work: &AdmissionWorkBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<()> {
        if self.rows.is_empty() {
            return Ok(());
        }
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        self.rows.sort_unstable();
        let length = self
            .rows
            .len()
            .checked_mul(SCALE_SORT_ROW_BYTES_V1 as usize)
            .ok_or_else(|| io_invalid("weighted digest-run bytes overflow"))?;
        scratch.reserve_sort_run(self.rows.len() as u64, length as u64)?;
        let leaf = format!("digest-run-{:08}.bin", self.run_sequence);
        self.run_sequence = self
            .run_sequence
            .checked_add(1)
            .ok_or_else(|| io_invalid("weighted digest-run sequence overflow"))?;
        let path = scratch.directory.join(&leaf);
        scratch.run_paths.push(path.clone());
        let mut file = open_new_private_file_v1(&path)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| io_invalid("weighted digest-run encoding allocation failed"))?;
        for row in &self.rows {
            bytes.extend_from_slice(&row.digest);
            bytes.extend_from_slice(&row.size.to_be_bytes());
            bytes.extend_from_slice(&row.offset.to_be_bytes());
        }
        write_all_accounted_v1(&mut file, &bytes, io, deadline, cancelled, work)?;
        file.sync_data()?;
        scratch.record_sort_run(&path, &file)?;
        self.rows.clear();
        Ok(())
    }
}

struct WeightedMemberTreeRowsV1<'a, 'profile> {
    cursor: &'a mut WeightedScaleMemberIterV1<'profile>,
    raw: &'a mut RawScaleFixtureV1,
    spool: &'a mut File,
    digest_runs: &'a mut DigestRunWriterV1,
    scratch: &'a mut ScaleScratchLeaseV1,
    io: &'a PinnedSqliteIoBudget,
    work: &'a AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    expected_members: u64,
    max_member_bytes: u64,
    max_source_bytes: u64,
    max_raw_input_file_bytes: u64,
    raw_io: &'a PinnedSqliteIoBudget,
    raw_work: &'a AdmissionWorkBudget,
    raw_deadline: Instant,
    raw_cancelled: &'a AtomicBool,
    max_key_bytes: usize,
    max_temp_logical_bytes: u64,
    finished: bool,
}

impl WeightedMemberTreeRowsV1<'_, '_> {
    fn ensure_finished(&self) -> std::io::Result<()> {
        if !self.finished
            || self.cursor.member_count() != self.expected_members
            || self.cursor.source_bytes() != self.scratch.spool_logical_bytes
        {
            return Err(io_invalid(
                "weighted member cursor did not reach verified EOF",
            ));
        }
        Ok(())
    }

    fn next_entry(&mut self) -> std::io::Result<Option<AuthenticatedTreeEntryV1>> {
        if self.finished {
            return Ok(None);
        }
        scale_active(self.deadline, self.cancelled)?;
        self.work.charge_many(1)?;
        let Some(member) = self.cursor.next_member()? else {
            if self.cursor.member_count() != self.expected_members {
                return Err(io_invalid("weighted member cursor stopped before target"));
            }
            self.finished = true;
            return Ok(None);
        };
        let size = u64::try_from(member.source_bytes.len())
            .map_err(|_| io_invalid("weighted member size exceeds u64"))?;
        if size == 0
            || size > self.max_member_bytes
            || member.path.len() > self.max_key_bytes
            || member.digest != Digest256::of_bytes(&member.source_bytes)
        {
            return Err(io_invalid(
                "weighted member payload or path exceeds selection",
            ));
        }
        let source_total = self
            .cursor
            .source_bytes()
            .checked_add(0)
            .ok_or_else(|| io_invalid("weighted member total overflow"))?;
        if source_total > self.max_source_bytes {
            return Err(io_invalid("weighted source bytes exceed selected cap"));
        }
        if size > self.max_raw_input_file_bytes {
            return Err(io_invalid(
                "weighted raw source member exceeds selected file cap",
            ));
        }
        self.raw.write_member(
            &member,
            self.raw_io,
            self.raw_work,
            self.raw_deadline,
            self.raw_cancelled,
        )?;
        self.scratch
            .reserve_spool_bytes(size, self.max_temp_logical_bytes)?;
        let offset = self.scratch.spool_logical_bytes;
        write_all_accounted_v1(
            self.spool,
            &member.source_bytes,
            self.io,
            self.deadline,
            self.cancelled,
            self.work,
        )?;
        self.scratch.record_spool_write(self.spool, size)?;
        self.digest_runs.push(
            DigestRunEntryV1 {
                digest: *member.digest.as_bytes(),
                size,
                offset,
            },
            self.scratch,
            self.spool,
            self.io,
            self.work,
            self.deadline,
            self.cancelled,
        )?;
        let mut value = [0u8; SCALE_MEMBER_VALUE_BYTES_V1];
        value[..32].copy_from_slice(member.digest.as_bytes());
        value[32..40].copy_from_slice(&size.to_be_bytes());
        value[40..44].copy_from_slice(&member.mode.to_le_bytes());
        RelativePath::parse(&member.path)
            .map_err(|_| io_invalid("weighted member path is invalid"))?;
        Ok(Some(AuthenticatedTreeEntryV1 {
            key: member.path.into_bytes(),
            value: value.to_vec(),
        }))
    }
}

impl Iterator for WeightedMemberTreeRowsV1<'_, '_> {
    type Item = tos_segment_store::Result<AuthenticatedTreeEntryV1>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_entry() {
            Ok(Some(row)) => Some(Ok(row)),
            Ok(None) => None,
            Err(error) => Some(Err(tos_segment_store::SegmentError::io(
                "weighted member source cursor failed",
                error,
            ))),
        }
    }
}

struct ScaleScratchLeaseV1 {
    directory: PathBuf,
    spool_leaf: PathBuf,
    run_paths: Vec<PathBuf>,
    reservation: PinnedSqliteSpaceReservation,
    max_logical_bytes: u64,
    max_allocated_bytes: u64,
    max_inodes: u64,
    spool_logical_bytes: u64,
    sort_logical_bytes: u64,
    spool_rows: u64,
    sort_run_allocated_bytes: u64,
    directory_allocated_bytes: u64,
    peak_allocated_bytes: u64,
    file_inode_peak: u64,
    cleaned: bool,
}

impl ScaleScratchLeaseV1 {
    fn new(
        directory: &Path,
        reservation: PinnedSqliteSpaceReservation,
        max_logical_bytes: u64,
        max_allocated_bytes: u64,
        max_inodes: u64,
    ) -> std::io::Result<Self> {
        create_private_dir_v1(directory)?;
        let mut lease = Self {
            directory: directory.to_path_buf(),
            spool_leaf: directory.join("payload.spool"),
            run_paths: Vec::new(),
            reservation,
            max_logical_bytes,
            max_allocated_bytes,
            max_inodes,
            spool_logical_bytes: 0,
            sort_logical_bytes: 0,
            spool_rows: 0,
            sort_run_allocated_bytes: 0,
            directory_allocated_bytes: 0,
            peak_allocated_bytes: 0,
            file_inode_peak: 0,
            cleaned: false,
        };
        lease.refresh_directory_allocation()?;
        Ok(lease)
    }

    fn reserve_spool_bytes(&self, bytes: u64, max_logical: u64) -> std::io::Result<()> {
        let next = self
            .spool_logical_bytes
            .checked_add(bytes)
            .filter(|value| *value <= max_logical && *value <= self.max_logical_bytes)
            .ok_or_else(|| io_invalid("weighted payload spool exceeds selected logical cap"))?;
        let next_total = next
            .checked_add(self.sort_logical_bytes)
            .filter(|value| *value <= self.max_logical_bytes)
            .ok_or_else(|| io_invalid("weighted scratch exceeds selected logical cap"))?;
        let _ = next_total;
        Ok(())
    }

    fn record_spool_write(&mut self, spool: &File, bytes: u64) -> std::io::Result<()> {
        self.spool_logical_bytes = self
            .spool_logical_bytes
            .checked_add(bytes)
            .ok_or_else(|| io_invalid("weighted payload spool count overflow"))?;
        self.spool_rows = self
            .spool_rows
            .checked_add(1)
            .ok_or_else(|| io_invalid("weighted payload spool row count overflow"))?;
        self.refresh_spool_allocation(spool)
    }

    fn reserve_sort_run(&self, rows: u64, bytes: u64) -> std::io::Result<()> {
        let next = self
            .sort_logical_bytes
            .checked_add(bytes)
            .filter(|value| {
                self.spool_logical_bytes
                    .checked_add(*value)
                    .is_some_and(|total| total <= self.max_logical_bytes)
            })
            .ok_or_else(|| io_invalid("weighted digest runs exceed selected logical cap"))?;
        let inodes = (self.run_paths.len() as u64)
            .checked_add(3)
            .ok_or_else(|| io_invalid("weighted scratch inode count overflow"))?;
        if inodes > self.max_inodes || rows == 0 || bytes == 0 {
            return Err(io_invalid("weighted scratch inode or run bound exceeded"));
        }
        let _ = next;
        Ok(())
    }

    fn record_sort_run(&mut self, path: &Path, file: &File) -> std::io::Result<()> {
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io_invalid("weighted digest run is not a regular file"));
        }
        self.sort_logical_bytes = self
            .sort_logical_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| io_invalid("weighted digest run bytes overflow"))?;
        self.sort_run_allocated_bytes = self
            .sort_run_allocated_bytes
            .checked_add(metadata.blocks().saturating_mul(512))
            .ok_or_else(|| io_invalid("weighted digest run allocation overflow"))?;
        self.file_inode_peak = (self.run_paths.len() as u64)
            .checked_add(2)
            .ok_or_else(|| io_invalid("weighted scratch inode peak overflow"))?;
        if self.file_inode_peak > self.max_inodes
            || self.spool_logical_bytes + self.sort_logical_bytes > self.max_logical_bytes
        {
            return Err(io_invalid("weighted scratch exceeds selected bound"));
        }
        let _ = path;
        self.refresh_directory_allocation()?;
        self.update_actual_allocation()
    }

    fn refresh_spool_allocation(&mut self, spool: &File) -> std::io::Result<()> {
        let metadata = spool.metadata()?;
        if !metadata.is_file() || metadata.len() != self.spool_logical_bytes {
            return Err(io_invalid("weighted payload spool file stamp differs"));
        }
        self.update_actual_allocation_with_spool(metadata.blocks().saturating_mul(512))
    }

    fn refresh_directory_allocation(&mut self) -> std::io::Result<()> {
        let metadata = fs::symlink_metadata(&self.directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io_invalid("weighted scratch directory changed"));
        }
        self.directory_allocated_bytes = metadata.blocks().saturating_mul(512);
        self.update_actual_allocation()
    }

    fn update_actual_allocation(&mut self) -> std::io::Result<()> {
        let spool_allocated = if self.spool_logical_bytes == 0 {
            0
        } else {
            fs::symlink_metadata(&self.spool_leaf)?
                .blocks()
                .saturating_mul(512)
        };
        self.update_actual_allocation_with_spool(spool_allocated)
    }

    fn update_actual_allocation_with_spool(
        &mut self,
        spool_allocated_bytes: u64,
    ) -> std::io::Result<()> {
        let actual = spool_allocated_bytes
            .checked_add(self.sort_run_allocated_bytes)
            .and_then(|bytes| bytes.checked_add(self.directory_allocated_bytes))
            .ok_or_else(|| io_invalid("weighted scratch allocation total overflow"))?;
        if actual > self.max_allocated_bytes {
            return Err(io_invalid(
                "weighted scratch allocation exceeds selected cap",
            ));
        }
        self.reservation
            .update_actual_allocated(actual)
            .map_err(|_| io_invalid("weighted scratch allocation reconciliation failed"))?;
        self.peak_allocated_bytes = self.peak_allocated_bytes.max(actual);
        Ok(())
    }

    fn logical_bytes(&self) -> std::io::Result<u64> {
        self.spool_logical_bytes
            .checked_add(self.sort_logical_bytes)
            .ok_or_else(|| io_invalid("weighted scratch logical byte total overflow"))
    }

    fn cleanup(&mut self) -> std::io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        let mut first_error = None;
        for path in &self.run_paths {
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound && first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Err(error) = fs::remove_file(&self.spool_leaf) {
            if error.kind() != std::io::ErrorKind::NotFound && first_error.is_none() {
                first_error = Some(error);
            }
        }
        if let Err(error) = fs::remove_dir(&self.directory) {
            if error.kind() != std::io::ErrorKind::NotFound && first_error.is_none() {
                first_error = Some(error);
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        self.reservation
            .update_actual_allocated(0)
            .map_err(|_| io_invalid("weighted scratch release reconciliation failed"))?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for ScaleScratchLeaseV1 {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

struct DigestSortedPackedSourcesV1<'a> {
    spool: Arc<File>,
    io: PinnedSqliteIoBudget,
    work: AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    files: Vec<File>,
    positions: Vec<u64>,
    heap: BinaryHeap<Reverse<(DigestRunEntryV1, usize)>>,
    pending: Option<DigestRunEntryV1>,
    complete: bool,
    failed: bool,
}

impl<'a> DigestSortedPackedSourcesV1<'a> {
    fn new(
        paths: &[PathBuf],
        spool: Arc<File>,
        io: PinnedSqliteIoBudget,
        work: AdmissionWorkBudget,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> std::io::Result<Self> {
        let mut files = Vec::new();
        let mut positions = Vec::new();
        let mut heap = BinaryHeap::new();
        files
            .try_reserve_exact(paths.len())
            .map_err(|_| io_invalid("weighted run reader allocation failed"))?;
        positions
            .try_reserve_exact(paths.len())
            .map_err(|_| io_invalid("weighted run cursor allocation failed"))?;
        heap.try_reserve(paths.len())
            .map_err(|_| io_invalid("weighted merge heap allocation failed"))?;
        for (run_index, path) in paths.iter().enumerate() {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)?;
            let metadata = file.metadata()?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() == 0
                || metadata.len() % SCALE_SORT_ROW_BYTES_V1 != 0
            {
                return Err(io_invalid("weighted digest run shape differs"));
            }
            files.push(file);
            positions.push(SCALE_SORT_ROW_BYTES_V1);
            let row =
                read_digest_run_entry_v1(&files[run_index], 0, &io, &work, deadline, cancelled)?
                    .ok_or_else(|| io_invalid("weighted digest run is empty"))?;
            heap.push(Reverse((row, run_index)));
        }
        Ok(Self {
            spool,
            io,
            work,
            deadline,
            cancelled,
            files,
            positions,
            heap,
            pending: None,
            complete: paths.is_empty(),
            failed: false,
        })
    }

    fn complete(&self) -> bool {
        self.complete && !self.failed
    }

    fn pop_entry(&mut self) -> std::io::Result<Option<DigestRunEntryV1>> {
        scale_active(self.deadline, self.cancelled)?;
        let Some(Reverse((row, run_index))) = self.heap.pop() else {
            return Ok(None);
        };
        self.work.charge_many(1)?;
        let offset = self.positions[run_index];
        let length = self.files[run_index].metadata()?.len();
        if offset < length {
            let next = read_digest_run_entry_v1(
                &self.files[run_index],
                offset,
                &self.io,
                &self.work,
                self.deadline,
                self.cancelled,
            )?
            .ok_or_else(|| io_invalid("weighted run ended inside a row"))?;
            self.positions[run_index] = offset
                .checked_add(SCALE_SORT_ROW_BYTES_V1)
                .ok_or_else(|| io_invalid("weighted run cursor overflow"))?;
            if next < row {
                return Err(io_invalid("weighted digest run is not sorted"));
            }
            self.heap.push(Reverse((next, run_index)));
        } else if offset != length {
            return Err(io_invalid("weighted digest run cursor escaped EOF"));
        }
        Ok(Some(row))
    }

    fn next_unique(&mut self) -> std::io::Result<Option<PackedObjectSourceV2>> {
        if self.failed {
            return Err(io_invalid("weighted digest cursor already failed"));
        }
        let result = self.next_unique_inner();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn next_unique_inner(&mut self) -> std::io::Result<Option<PackedObjectSourceV2>> {
        let first = match self.pending.take() {
            Some(row) => row,
            None => match self.pop_entry()? {
                Some(row) => row,
                None => {
                    self.complete = true;
                    return Ok(None);
                }
            },
        };
        loop {
            let Some(next) = self.pop_entry()? else {
                self.complete = true;
                break;
            };
            if next.digest == first.digest {
                compare_duplicate_spool_rows_v1(
                    &self.spool,
                    first,
                    next,
                    &self.io,
                    &self.work,
                    self.deadline,
                    self.cancelled,
                )?;
            } else {
                self.pending = Some(next);
                break;
            }
        }
        Ok(Some(PackedObjectSourceV2::from_slice(
            Digest256::from_bytes(first.digest),
            first.size,
            Arc::clone(&self.spool),
            first.offset,
        )))
    }
}

impl Iterator for DigestSortedPackedSourcesV1<'_> {
    type Item = std::io::Result<PackedObjectSourceV2>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_unique() {
            Ok(Some(source)) => Some(Ok(source)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }
}

fn read_digest_run_entry_v1(
    file: &File,
    offset: u64,
    io: &PinnedSqliteIoBudget,
    work: &AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> std::io::Result<Option<DigestRunEntryV1>> {
    scale_active(deadline, cancelled)?;
    let length = file.metadata()?.len();
    if offset >= length {
        return Ok(None);
    }
    if length - offset < SCALE_SORT_ROW_BYTES_V1 {
        return Err(io_invalid("weighted digest run is truncated"));
    }
    work.charge_many(1)?;
    let mut raw = [0u8; SCALE_SORT_ROW_BYTES_V1 as usize];
    read_exact_at_accounted_v1(file, &mut raw, offset, io, deadline, cancelled, work)?;
    Ok(Some(DigestRunEntryV1 {
        digest: raw[..32]
            .try_into()
            .map_err(|_| io_invalid("weighted digest run key width differs"))?,
        size: u64::from_be_bytes(
            raw[32..40]
                .try_into()
                .map_err(|_| io_invalid("weighted digest run size width differs"))?,
        ),
        offset: u64::from_be_bytes(
            raw[40..48]
                .try_into()
                .map_err(|_| io_invalid("weighted digest run offset width differs"))?,
        ),
    }))
}

fn compare_duplicate_spool_rows_v1(
    spool: &File,
    left: DigestRunEntryV1,
    right: DigestRunEntryV1,
    io: &PinnedSqliteIoBudget,
    work: &AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> std::io::Result<()> {
    if left.size != right.size {
        return Err(io_invalid(
            "equal object digest has different declared size",
        ));
    }
    let mut left_bytes = [0u8; 64 * 1024];
    let mut right_bytes = [0u8; 64 * 1024];
    let mut position = 0u64;
    while position < left.size {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        let count = usize::try_from((left.size - position).min(left_bytes.len() as u64))
            .map_err(|_| io_invalid("duplicate payload comparison size differs"))?;
        let left_at = left
            .offset
            .checked_add(position)
            .ok_or_else(|| io_invalid("duplicate left payload offset overflow"))?;
        let right_at = right
            .offset
            .checked_add(position)
            .ok_or_else(|| io_invalid("duplicate right payload offset overflow"))?;
        read_exact_at_accounted_v1(
            spool,
            &mut left_bytes[..count],
            left_at,
            io,
            deadline,
            cancelled,
            work,
        )?;
        read_exact_at_accounted_v1(
            spool,
            &mut right_bytes[..count],
            right_at,
            io,
            deadline,
            cancelled,
            work,
        )?;
        if left_bytes[..count] != right_bytes[..count] {
            return Err(io_invalid(
                "equal object digest has different payload bytes",
            ));
        }
        position = position
            .checked_add(count as u64)
            .ok_or_else(|| io_invalid("duplicate payload comparison cursor overflow"))?;
    }
    Ok(())
}

fn read_exact_at_accounted_v1(
    file: &File,
    mut bytes: &mut [u8],
    mut offset: u64,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &AdmissionWorkBudget,
) -> std::io::Result<()> {
    while !bytes.is_empty() {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        let wanted = bytes.len().min(64 * 1024);
        io.charge_read(wanted as u64)
            .map_err(|_| io_invalid("weighted scratch read budget exhausted"))?;
        let read = file.read_at(&mut bytes[..wanted], offset)?;
        io.record_read_returned(read as u64)
            .map_err(|_| io_invalid("weighted scratch read accounting failed"))?;
        if read == 0 {
            return Err(io_invalid("weighted scratch file ended early"));
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or_else(|| io_invalid("weighted scratch read offset overflow"))?;
        bytes = &mut bytes[read..];
    }
    Ok(())
}

fn read_source_file_accounted_v1(
    path: &Path,
    max_bytes: usize,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &AdmissionWorkBudget,
) -> std::io::Result<Vec<u8>> {
    scale_active(deadline, cancelled)?;
    work.charge_many(1)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max_bytes as u64
    {
        return Err(io_invalid(
            "weighted source input is not a bounded regular file",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    io.charge_read(metadata.len())
        .map_err(|_| io_invalid("weighted source input read budget exhausted"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(metadata.len() as usize)
        .map_err(|_| io_invalid("weighted source input allocation failed"))?;
    let mut reader = file;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        scale_active(deadline, cancelled)?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    io.record_read_returned(bytes.len() as u64)
        .map_err(|_| io_invalid("weighted source input read accounting failed"))?;
    if bytes.len() as u64 != metadata.len() || reader.metadata()?.len() != metadata.len() {
        return Err(io_invalid("weighted source input changed while reading"));
    }
    Ok(bytes)
}

fn write_all_accounted_v1(
    file: &mut File,
    mut bytes: &[u8],
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &AdmissionWorkBudget,
) -> std::io::Result<()> {
    while !bytes.is_empty() {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        let wanted = bytes.len().min(64 * 1024);
        io.charge_write(wanted as u64)
            .map_err(|_| io_invalid("weighted output write budget exhausted"))?;
        let written = file.write(&bytes[..wanted])?;
        io.record_write_returned(written as u64)
            .map_err(|_| io_invalid("weighted output write accounting failed"))?;
        if written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "weighted output write returned zero",
            ));
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ScaleDirectoryStampV1 {
    device: u64,
    inode: u64,
    mode: u32,
}

impl ScaleDirectoryStampV1 {
    fn from_file(file: &File) -> std::io::Result<Self> {
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
            return Err(io_invalid("weighted output root is not private"));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        })
    }
}

fn verify_private_directory_v1(
    path: &Path,
    held: &File,
    expected: ScaleDirectoryStampV1,
) -> std::io::Result<()> {
    let held_stamp = ScaleDirectoryStampV1::from_file(held)?;
    let named = fs::symlink_metadata(path)?;
    if !named.is_dir()
        || named.file_type().is_symlink()
        || named.dev() != expected.device
        || named.ino() != expected.inode
        || named.mode() != expected.mode
        || held_stamp.device != expected.device
        || held_stamp.inode != expected.inode
        || held_stamp.mode != expected.mode
    {
        return Err(io_invalid("weighted output root identity changed"));
    }
    Ok(())
}

fn create_private_dir_v1(path: &Path) -> std::io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.mode(0o700);
    builder.create(path)?;
    Ok(())
}

fn open_private_dir_v1(path: &Path) -> std::io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir()
        || metadata.mode() & 0o077 != 0
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err(io_invalid(
            "weighted directory is not caller-owned private state",
        ));
    }
    Ok(file)
}

fn open_new_private_file_v1(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

fn verify_output_location_outside_repository_v1(
    repository_root: &Path,
    output_root: &Path,
) -> std::io::Result<()> {
    let repository = fs::canonicalize(repository_root)
        .map_err(|_| io_invalid("weighted repository root is unavailable"))?;
    let parent = output_root
        .parent()
        .ok_or_else(|| io_invalid("weighted output parent is absent"))?;
    let parent = fs::canonicalize(parent)
        .map_err(|_| io_invalid("weighted output parent is unavailable"))?;
    let name = output_root
        .file_name()
        .ok_or_else(|| io_invalid("weighted output root name is absent"))?;
    let destination = parent.join(name);
    if destination.starts_with(&repository) || repository.starts_with(&destination) {
        return Err(io_invalid(
            "weighted fixture output must be outside the authored repository",
        ));
    }
    Ok(())
}

fn write_private_leaf_accounted_v1(
    root: &Path,
    leaf: &str,
    bytes: &[u8],
    tree_io: &NativeV2TreeIo,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &AdmissionWorkBudget,
) -> std::io::Result<()> {
    if leaf.is_empty() || leaf.contains('/') || leaf == "." || leaf == ".." {
        return Err(io_invalid("weighted output leaf is unsafe"));
    }
    scale_active(deadline, cancelled)?;
    work.charge_many(1)?;
    let reserved = tree_io.reserve_file_allocation(bytes.len() as u64)?;
    let path = root.join(leaf);
    let mut file = match open_new_private_file_v1(&path) {
        Ok(file) => file,
        Err(error) => {
            let _ = tree_io.release_file_allocation(reserved);
            return Err(error);
        }
    };
    let io = tree_io.io_budget();
    let write_result = write_all_accounted_v1(&mut file, bytes, io, deadline, cancelled, work)
        .and_then(|()| file.sync_all());
    let actual = file.metadata()?.blocks().saturating_mul(512);
    let reconcile_result = tree_io.reconcile_file_allocation(reserved, actual);
    write_result?;
    reconcile_result?;
    Ok(())
}
