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

/// A declared weighted ladder point for the measured-workload hypothesis. The
/// 5% class is an unreviewed TextUnit packet fixture; authored-route bridge
/// coverage remains excluded until its owner defines a fixture-safe contract.
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

    /// Scale the maintained five-class distribution to an explicitly declared
    /// finite record count. Largest-remainder rounding is deterministic and
    /// preserves the exact 5/40/5/15/35 mix whenever the count is divisible by
    /// 100. Native's selected file, tree, byte, inode, work, and state budgets
    /// remain the admission boundary for materialization.
    pub fn weighted_for_records(seed: Digest256, target_records: u64) -> std::io::Result<Self> {
        if target_records < 20 {
            return Err(io_invalid(
                "weighted profile requires at least one record in each class",
            ));
        }
        let mut profile = Self::fixed_100k(seed);
        let weights = profile.classes.map(|row| row.count / 1_000);
        let mut counts = [0u64; 5];
        let mut remainders = [0u128; 5];
        let mut assigned = 0u128;
        for (index, weight) in weights.into_iter().enumerate() {
            let weighted = (target_records as u128)
                .checked_mul(weight as u128)
                .ok_or_else(|| io_invalid("weighted profile class count overflow"))?;
            counts[index] = u64::try_from(weighted / 100)
                .map_err(|_| io_invalid("weighted profile class count exceeds u64"))?;
            remainders[index] = weighted % 100;
            assigned = assigned
                .checked_add(counts[index] as u128)
                .ok_or_else(|| io_invalid("weighted profile class sum overflow"))?;
        }
        let remaining = (target_records as u128)
            .checked_sub(assigned)
            .ok_or_else(|| io_invalid("weighted profile class rounding underflow"))?;
        if remaining > counts.len() as u128 {
            return Err(io_invalid("weighted profile class rounding differs"));
        }
        let mut order = [0usize, 1, 2, 3, 4];
        order.sort_by_key(|index| (Reverse(remainders[*index]), *index));
        for index in order.iter().take(remaining as usize) {
            counts[*index] = counts[*index]
                .checked_add(1)
                .ok_or_else(|| io_invalid("weighted profile class count overflow"))?;
        }
        profile.target_records = target_records;
        for (row, count) in profile.classes.iter_mut().zip(counts) {
            row.count = count;
        }
        profile.validate()?;
        Ok(profile)
    }

    /// Smallest maintained weighted point that reaches the entire selected
    /// quantile period in every class; it is a cohort input, not an admission.
    pub(crate) fn minimum_all_quantile_variant_profile_v1(
        seed: Digest256,
    ) -> std::io::Result<Self> {
        let reference = Self::fixed_100k(seed);
        let minimum = reference
            .classes
            .iter()
            .map(|row| row.count)
            .min()
            .filter(|count| *count != 0)
            .ok_or_else(|| io_invalid("weighted class minimum absent"))?;
        let total = reference
            .classes
            .iter()
            .try_fold(0u64, |n, row| n.checked_add(row.count))
            .ok_or_else(|| io_invalid("weighted class total overflow"))?;
        let target = total
            .checked_mul(SCALE_SELECTED_QUANTILE_PERIOD_V1)
            .and_then(|n| n.checked_add(minimum - 1))
            .map(|n| n / minimum)
            .ok_or_else(|| io_invalid("representative quantile point overflow"))?;
        let profile = Self::weighted_for_records(seed, target)?;
        if profile
            .classes
            .iter()
            .any(|row| row.count < SCALE_SELECTED_QUANTILE_PERIOD_V1)
        {
            return Err(io_invalid("representative quantile coverage differs"));
        }
        Ok(profile)
    }

    pub fn validate(&self) -> std::io::Result<()> {
        if self.target_records < 20
            || raw_fixture_directory_count_v1(self).is_err()
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
        // per emitted member. Shared ancestors are counted once from the
        // paths of the selected classes, matching raw fixture creation.
        let raw_input_directories = raw_fixture_directory_count_v1(self)?;
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
        let profile_1b = Self::weighted_for_records(self.seed, 1_000_000_000)?;
        let mut source_p50_1b = 0u128;
        let mut source_scenario_1b = 0u128;
        for row in &profile_1b.classes {
            let count = row.count as u128;
            let [p50_count, p95_count, max_count] = selected_quantile_counts_v1(row.count)?;
            source_p50_1b = source_p50_1b
                .checked_add(count * row.p50_bytes as u128)
                .ok_or_else(|| io_invalid("1B p50 source forecast overflow"))?;
            source_scenario_1b = source_scenario_1b
                .checked_add(
                    (p50_count as u128) * row.p50_bytes as u128
                        + (p95_count as u128) * row.p95_bytes as u128
                        + (max_count as u128) * row.max_bytes as u128,
                )
                .ok_or_else(|| io_invalid("1B source forecast overflow"))?;
        }
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
            target_records: self.target_records,
            raw_input_file_count: self.target_records,
            raw_input_directory_count: raw_input_directories,
            raw_input_inode_count: raw_input_inodes,
            source_text_unit_packet_records: source_text_unit_packet_records,
            authored_route_bridge_records: 0,
            authored_route_bridge_coverage: SCALE_AUTHORED_BRIDGE_COVERAGE_V1,
            p50_logical_source_bytes: checked_scale_u64(p50)?,
            selected_quantile_scenario_logical_source_bytes: checked_scale_u64(scenario)?,
            raw_member_leaf_input_bytes: checked_scale_u64(member_leaf_bytes)?,
            raw_object_extent_leaf_input_bytes: checked_scale_u64(object_leaf_bytes)?,
            external_sort_logical_bytes: external_sort_bytes,
            temporary_payload_spool_peak_bytes: checked_scale_u64(scenario)?,
            temporary_digest_sort_peak_bytes: external_sort_bytes,
            temporary_scratch_peak_bytes: checked_scale_u64(scratch_peak)?,
            temporary_scratch_blocks_4k_assumption: checked_scale_u64(scratch_blocks_4k)?,
            temporary_sort_run_count: sort_run_count,
            temporary_file_inode_peak: sort_run_count + 2,
            packed_object_frames_per_pack: MAX_PACKED_OBJECT_FRAMES_V2,
            packed_object_pack_count_upper: max_pack_count,
            history_change_rows: checked_scale_u64(history_change_rows)?,
            history_change_payload_scenario_bytes: checked_scale_u64(history_change_payload_bytes)?,
            p50_logical_source_bytes_at_1b: checked_scale_u64(source_p50_1b)?,
            selected_quantile_scenario_logical_source_bytes_at_1b: checked_scale_u64(
                source_scenario_1b,
            )?,
            current_snapshot_three_copy_p50_bytes: checked_scale_u64(p50 * 3)?,
            three_pins_with_backup_and_restore_no_dedup_p50_bytes: checked_scale_u64(p50 * 5)?,
            ten_full_copy_no_dedup_scenario_bytes_at_1b: checked_scale_u64(
                source_scenario_1b * 10,
            )?,
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

#[cfg(test)]
mod weighted_scale_profile_tests {
    use super::*;

    #[test]
    fn weighted_record_ladder_preserves_100k_and_prices_billion_without_a_fake_cap() {
        let seed = Digest256::of_bytes(b"weighted-record-ladder");
        let default = WeightedScaleProfileV1::weighted_for_records(seed, 100_000).unwrap();
        assert_eq!(default, WeightedScaleProfileV1::fixed_100k(seed));
        assert_eq!(max_member_path_bytes_v1(&default), 114);
        assert!(max_member_path_bytes_v1(&default) > path_for(WeightedScaleClassV1::Work, 0).len());
        let default_envelope = weighted_scale_producer_envelope_v1(&default, 4_096).unwrap();
        assert_eq!(default_envelope.maximum_source_bytes, 903_528_300);
        assert_eq!(default_envelope.temporary_logical_bytes, 908_328_300);
        assert_eq!(default_envelope.temporary_allocated_bytes, 908_447_744);
        assert_eq!(default_envelope.temporary_file_inodes, 27);
        assert_eq!(default_envelope.raw_input_allocated_bytes, 2_371_100_672);
        assert_eq!(
            default.forecast_inputs().unwrap().raw_input_directory_count,
            100_016
        );

        let billion = WeightedScaleProfileV1::weighted_for_records(seed, 1_000_000_000).unwrap();
        assert_eq!(
            billion.classes.map(|row| row.count),
            [
                50_000_000,
                400_000_000,
                50_000_000,
                150_000_000,
                350_000_000
            ]
        );
        let default_forecast = default.forecast_inputs().unwrap();
        let billion_forecast = billion.forecast_inputs().unwrap();
        assert_eq!(billion_forecast.target_records, 1_000_000_000);
        assert_eq!(
            billion_forecast.selected_quantile_scenario_logical_source_bytes,
            default_forecast
                .selected_quantile_scenario_logical_source_bytes
                .checked_mul(10_000)
                .unwrap()
        );
        assert_eq!(
            billion_forecast.selected_quantile_scenario_logical_source_bytes_at_1b,
            billion_forecast.selected_quantile_scenario_logical_source_bytes
        );
        assert!(billion_forecast.selected_quantile_scenario_logical_source_bytes_at_1b < u64::MAX);
        let envelope = weighted_scale_producer_envelope_v1(&billion, 4_096).unwrap();
        assert!(envelope.maximum_source_bytes > 1_000_000_000_000);
        assert!(envelope.raw_input_allocated_bytes > envelope.maximum_source_bytes);
        assert!(!billion_forecast.physical_fit_established);
    }

    #[test]
    fn weighted_record_ladder_rounds_deterministically_and_rejects_structural_overflow() {
        let seed = Digest256::of_bytes(b"weighted-record-rounding");
        let profile = WeightedScaleProfileV1::weighted_for_records(seed, 101).unwrap();
        assert_eq!(profile.classes.map(|row| row.count), [5, 41, 5, 15, 35]);
        assert_eq!(
            profile.classes.iter().map(|row| row.count).sum::<u64>(),
            101
        );
        let small_forecast = profile.forecast_inputs().unwrap();
        let billion_profile =
            WeightedScaleProfileV1::weighted_for_records(seed, 1_000_000_000).unwrap();
        let billion_forecast = billion_profile.forecast_inputs().unwrap();
        assert_eq!(
            small_forecast.selected_quantile_scenario_logical_source_bytes_at_1b,
            billion_forecast.selected_quantile_scenario_logical_source_bytes
        );
        assert!(
            small_forecast.selected_quantile_scenario_logical_source_bytes_at_1b
                > small_forecast.selected_quantile_scenario_logical_source_bytes
        );
        assert!(WeightedScaleProfileV1::weighted_for_records(seed, u64::MAX).is_err());
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
#[derive(Clone, Debug, PartialEq)]
pub struct WeightedScaleForecastInputsV1 {
    pub target_records: u64,
    /// Raw filesystem input for the real source census: one member file per
    /// record plus the root and the shared/per-record directory hierarchy.
    pub raw_input_file_count: u64,
    pub raw_input_directory_count: u64,
    pub raw_input_inode_count: u64,
    /// The 5% EvidencePacket class plus the 15% TextUnit class. Both use the
    /// schema-supported unreviewed TextUnit packet contract.
    pub source_text_unit_packet_records: u64,
    /// Authored-route bridge fixtures are intentionally not generated under
    /// a schema that hardcodes canon/review/match assertions.
    pub authored_route_bridge_records: u64,
    pub authored_route_bridge_coverage: &'static str,
    pub p50_logical_source_bytes: u64,
    pub selected_quantile_scenario_logical_source_bytes: u64,
    pub raw_member_leaf_input_bytes: u64,
    pub raw_object_extent_leaf_input_bytes: u64,
    /// Upper-bound scratch geometry: spool all logical member bytes before
    /// payload deduplication, then retain sorted digest runs while packing.
    pub temporary_payload_spool_peak_bytes: u64,
    pub temporary_digest_sort_peak_bytes: u64,
    pub temporary_scratch_peak_bytes: u64,
    /// A clearly labeled 4 KiB allocation-unit estimate, not measured blocks.
    pub temporary_scratch_blocks_4k_assumption: u64,
    pub temporary_sort_run_count: u64,
    /// One payload spool, one sort directory, and one inode per sorted run.
    pub temporary_file_inode_peak: u64,
    pub packed_object_frames_per_pack: u32,
    pub packed_object_pack_count_upper: u64,
    pub external_sort_logical_bytes: u64,
    pub history_change_rows: u64,
    pub history_change_payload_scenario_bytes: u64,
    pub p50_logical_source_bytes_at_1b: u64,
    pub selected_quantile_scenario_logical_source_bytes_at_1b: u64,
    pub current_snapshot_three_copy_p50_bytes: u64,
    pub three_pins_with_backup_and_restore_no_dedup_p50_bytes: u64,
    pub ten_full_copy_no_dedup_scenario_bytes_at_1b: u64,
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

/// Explicit template transformation route; absence in historical profiles keeps
/// their technical-only input law unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WeightedScaleClaimTemplateRouteV1 {
    LegacyTechnicalV1,
    WorkAuthorshipFixtureV1,
}

/// Finite caller-declared source rows; these are inputs, not an issuer token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WeightedScaleClaimTemplateRowV1 {
    pub(crate) source_path: String,
    pub(crate) source_sha256: Digest256,
    pub(crate) source_line: u64,
    pub(crate) template_sha256: Digest256,
    pub(crate) template_bytes: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WeightedScaleGeneratorAgentSelectionV1 {
    pub(crate) source_path: String,
    pub(crate) source_sha256: Digest256,
    pub(crate) record_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WeightedScaleClaimTemplateSelectionV1 {
    pub(crate) relation_registry_sha256: Digest256,
    pub(crate) generator_agent: WeightedScaleGeneratorAgentSelectionV1,
    pub(crate) templates: [WeightedScaleClaimTemplateRowV1; 3],
}
impl WeightedScaleClaimTemplateSelectionV1 {
    fn profile_value(&self) -> serde_json::Value {
        serde_json::json!({
            "route": "work_authorship_fixture_v1",
            "relation_registry_sha256": self.relation_registry_sha256.to_hex(),
            "relation_type_id": "tos.relation.authored-by",
            "subject_class": "Work",
            "generator_agent": {"source_path": self.generator_agent.source_path, "source_sha256": self.generator_agent.source_sha256.to_hex(), "record_id": self.generator_agent.record_id},
            "templates": self.templates.iter().map(|row| serde_json::json!({
                "source_path": row.source_path, "source_sha256": row.source_sha256.to_hex(),
                "source_line": row.source_line, "template_sha256": row.template_sha256.to_hex(),
                "template_bytes": row.template_bytes,
            })).collect::<Vec<_>>()
        })
    }
    fn retained_state_bytes(&self) -> std::io::Result<usize> {
        self.templates.iter().try_fold(
            size_of::<Self>()
                .checked_add(self.generator_agent.source_path.capacity())
                .and_then(|n| n.checked_add(self.generator_agent.record_id.capacity()))
                .ok_or_else(|| io_invalid("generator Agent declaration state overflow"))?,
            |bytes, row| {
                bytes
                    .checked_add(row.source_path.capacity())
                    .ok_or_else(|| io_invalid("selected Claim declaration state overflow"))
            },
        )
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRecipeWireV2 {
    schema: String,
    claim_template_selection: ClaimSelectionWireV2,
    artifact_template_selection: ArtifactSelectionWireV2,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimSelectionWireV2 {
    route: String,
    relation_registry_sha256: String,
    relation_type_id: String,
    subject_class: String,
    generator_agent: GeneratorAgentWireV2,
    templates: [ClaimTemplateWireV2; 3],
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratorAgentWireV2 {
    source_path: String,
    source_sha256: String,
    record_id: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimTemplateWireV2 {
    source_path: String,
    source_sha256: String,
    source_line: u64,
    template_sha256: String,
    template_bytes: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactSelectionWireV2 {
    route: String,
    templates: [ArtifactTemplateWireV2; 3],
    rights_template: ArtifactSourceWireV2,
    discovery_template: ArtifactSourceWireV2,
    source_policy: ArtifactSourceWireV2,
    research: ArtifactSourceWireV2,
    resource_templates: [ArtifactSourceWireV2; 3],
    generated_at: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactTemplateWireV2 {
    source_path: String,
    source_sha256: String,
    template_sha256: String,
    template_bytes: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactSourceWireV2 {
    source_path: String,
    source_sha256: String,
    source_bytes: u64,
}
fn recipe_digest_v2(raw: &str) -> std::io::Result<Digest256> {
    Digest256::from_hex(raw).map_err(|_| io_invalid("fixture recipe SHA invalid"))
}
impl ArtifactSourceWireV2 {
    fn into_owner(self) -> std::io::Result<WeightedScaleArtifactPinnedSourceV1> {
        Ok(WeightedScaleArtifactPinnedSourceV1 {
            source_path: self.source_path,
            source_sha256: recipe_digest_v2(&self.source_sha256)?,
            source_bytes: self.source_bytes,
        })
    }
}
impl FixtureRecipeWireV2 {
    fn into_owner(
        self,
    ) -> std::io::Result<(
        WeightedScaleClaimTemplateSelectionV1,
        WeightedScaleArtifactTemplateSelectionV1,
    )> {
        if self.schema != "tos_weighted_capacity_fixture_recipe_v2"
            || self.claim_template_selection.route != "work_authorship_fixture_v1"
            || self.claim_template_selection.relation_type_id != "tos.relation.authored-by"
            || self.claim_template_selection.subject_class != "Work"
            || self.artifact_template_selection.route != "synthetic_repository_resource_v1"
        {
            return Err(io_invalid("fixture recipe declared owner route differs"));
        }
        let claim = self.claim_template_selection;
        let agent = claim.generator_agent;
        let claim_row =
            |row: ClaimTemplateWireV2| -> std::io::Result<WeightedScaleClaimTemplateRowV1> {
                if row.source_line == 0 || row.template_bytes == 0 {
                    return Err(io_invalid("fixture Claim template row invalid"));
                }
                Ok(WeightedScaleClaimTemplateRowV1 {
                    source_path: row.source_path,
                    source_sha256: recipe_digest_v2(&row.source_sha256)?,
                    source_line: row.source_line,
                    template_sha256: recipe_digest_v2(&row.template_sha256)?,
                    template_bytes: row.template_bytes,
                })
            };
        let [a, b, c] = claim.templates;
        let claim = WeightedScaleClaimTemplateSelectionV1 {
            relation_registry_sha256: recipe_digest_v2(&claim.relation_registry_sha256)?,
            generator_agent: WeightedScaleGeneratorAgentSelectionV1 {
                source_path: agent.source_path,
                source_sha256: recipe_digest_v2(&agent.source_sha256)?,
                record_id: agent.record_id,
            },
            templates: [claim_row(a)?, claim_row(b)?, claim_row(c)?],
        };
        let artifact = self.artifact_template_selection;
        let artifact_row =
            |row: ArtifactTemplateWireV2| -> std::io::Result<WeightedScaleArtifactTemplateRowV1> {
                Ok(WeightedScaleArtifactTemplateRowV1 {
                    source_path: row.source_path,
                    source_sha256: recipe_digest_v2(&row.source_sha256)?,
                    template_sha256: recipe_digest_v2(&row.template_sha256)?,
                    template_bytes: row.template_bytes,
                })
            };
        let [a, b, c] = artifact.templates;
        let [r0, r1, r2] = artifact.resource_templates;
        let artifact = WeightedScaleArtifactTemplateSelectionV1 {
            templates: [artifact_row(a)?, artifact_row(b)?, artifact_row(c)?],
            rights_template: artifact.rights_template.into_owner()?,
            discovery_template: artifact.discovery_template.into_owner()?,
            source_policy: artifact.source_policy.into_owner()?,
            research: artifact.research.into_owner()?,
            resource_templates: [r0.into_owner()?, r1.into_owner()?, r2.into_owner()?],
            generated_at: artifact.generated_at,
        };
        artifact.validate()?;
        Ok((claim, artifact))
    }
}

/// Read one explicitly pinned authored recipe through the original meters.
/// The fixed arrays contain declarations, never generated record IDs.
pub(crate) fn load_declared_fixture_templates_accounted(
    root: &Path,
    relative_recipe: &str,
    expected_sha: Digest256,
    profile: &WeightedScaleProfileV1,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &AdmissionWorkBudget,
    max_state: usize,
    caller_state: usize,
) -> std::io::Result<WeightedScaleTemplateSetV1> {
    scale_active(deadline, cancelled)?;
    let relative = RelativePath::parse(relative_recipe)
        .map_err(|_| io_invalid("fixture recipe relative path invalid"))?;
    if !relative.as_str().starts_with("ToS/") {
        return Err(io_invalid("fixture recipe must be an authored ToS input"));
    }
    let path_state = root
        .as_os_str()
        .len()
        .checked_add(relative_recipe.len())
        .and_then(|n| n.checked_add(1 + size_of::<PathBuf>()))
        .and_then(|n| n.checked_add(relative_recipe.len() + size_of::<RelativePath>()))
        .ok_or_else(|| io_invalid("fixture recipe path state overflow"))?;
    let frame = max_state
        .checked_sub(caller_state)
        .and_then(|n| n.checked_sub(path_state))
        .and_then(|n| n.checked_sub(size_of::<[u8; 64 * 1024]>()))
        .ok_or_else(|| io_invalid("fixture recipe read state exhausted"))?;
    let path = root.join(relative.as_str());
    let raw = read_source_file_accounted_v1(&path, frame, io, deadline, cancelled, work)?;
    if Digest256::of_bytes(&raw) != expected_sha {
        return Err(io_invalid("fixture recipe source SHA differs"));
    }
    let source_bytes = raw.len() as u64;
    let decode_frame = max_state
        .checked_sub(caller_state)
        .and_then(|n| n.checked_sub(path_state))
        .and_then(|n| n.checked_sub(raw.capacity()))
        .and_then(|n| n.checked_sub(size_of::<FixtureRecipeWireV2>()))
        .and_then(|n| n.checked_sub(size_of::<WeightedScaleClaimTemplateSelectionV1>()))
        .and_then(|n| n.checked_sub(size_of::<WeightedScaleArtifactTemplateSelectionV1>()))
        .ok_or_else(|| io_invalid("fixture recipe decode state exhausted"))?;
    let (value, _) = tos_validation::record_biblio_cut::bounded_decoded_state(
        &raw,
        JsonLimits::default(),
        decode_frame,
        deadline,
        cancelled,
    )
    .map_err(|_| io_invalid("fixture recipe bounded decode refused"))?;
    // Consume the one bounded decoded value into fixed typed fields. Owned
    // strings transfer; there is no second freeform JSON tree or N-sized Vec.
    let wire: FixtureRecipeWireV2 = serde_json::from_value(value)
        .map_err(|_| io_invalid("fixture recipe exact declared fields differ"))?;
    let (claim, artifact) = wire.into_owner()?;
    drop(raw);
    drop(path);
    drop(relative);
    let source_state = size_of::<WeightedScaleFixtureRecipeSourceV2>()
        .checked_add(relative_recipe.len())
        .ok_or_else(|| io_invalid("fixture source state overflow"))?;
    let mut result = WeightedScaleTemplateSetV1::load_with_all5_selection_accounted(
        root,
        profile,
        claim,
        artifact,
        io,
        deadline,
        cancelled,
        work,
        max_state,
        caller_state
            .checked_add(source_state)
            .ok_or_else(|| io_invalid("fixture source caller state overflow"))?,
    )?;
    result.fixture_recipe_source = Some(WeightedScaleFixtureRecipeSourceV2 {
        source_path: relative_recipe.to_owned(),
        source_sha256: expected_sha,
        source_bytes,
    });
    if weighted_producer_state_upper_v1(&result, 0)?
        .checked_add(caller_state)
        .is_none_or(|n| n > max_state)
    {
        return Err(io_invalid("fixture recipe retained templates exceed state"));
    }
    Ok(result)
}

/// Authenticated selected recipe copy, not a lasting pathname/FD claim.
pub(crate) struct WeightedScaleFixtureRecipeSourceV2 {
    source_path: String,
    source_sha256: Digest256,
    source_bytes: u64,
}
impl WeightedScaleFixtureRecipeSourceV2 {
    pub(crate) fn source_path(&self) -> &str {
        &self.source_path
    }
    pub(crate) fn source_sha256(&self) -> Digest256 {
        self.source_sha256
    }
    pub(crate) fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
}

/// Caller-declared finite sources for the explicit synthetic Artifact recipe.
/// These data do not issue a semantic verdict or a native creation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WeightedScaleArtifactPinnedSourceV1 {
    pub(crate) source_path: String,
    pub(crate) source_sha256: Digest256,
    pub(crate) source_bytes: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WeightedScaleArtifactTemplateRowV1 {
    pub(crate) source_path: String,
    pub(crate) source_sha256: Digest256,
    pub(crate) template_sha256: Digest256,
    pub(crate) template_bytes: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WeightedScaleArtifactTemplateSelectionV1 {
    pub(crate) templates: [WeightedScaleArtifactTemplateRowV1; 3],
    pub(crate) rights_template: WeightedScaleArtifactPinnedSourceV1,
    pub(crate) discovery_template: WeightedScaleArtifactPinnedSourceV1,
    pub(crate) source_policy: WeightedScaleArtifactPinnedSourceV1,
    pub(crate) research: WeightedScaleArtifactPinnedSourceV1,
    pub(crate) resource_templates: [WeightedScaleArtifactPinnedSourceV1; 3],
    /// Configured fixture date. Actual output observations are separate.
    pub(crate) generated_at: String,
}
impl WeightedScaleArtifactTemplateSelectionV1 {
    fn profile_value(&self) -> serde_json::Value {
        let source = |s: &WeightedScaleArtifactPinnedSourceV1| {
            serde_json::json!({
            "source_path":s.source_path,"source_sha256":s.source_sha256.to_hex(),"source_bytes":s.source_bytes})
        };
        serde_json::json!({"route":"synthetic_repository_resource_v1",
            "templates":self.templates.iter().map(|s| serde_json::json!({"source_path":s.source_path,
                "source_sha256":s.source_sha256.to_hex(),"template_sha256":s.template_sha256.to_hex(),
                "template_bytes":s.template_bytes})).collect::<Vec<_>>(),
            "rights_template":source(&self.rights_template),"discovery_template":source(&self.discovery_template),
            "source_policy":source(&self.source_policy),"research":source(&self.research),
            "resource_templates":self.resource_templates.iter().map(source).collect::<Vec<_>>(),
            "generated_at":self.generated_at})
    }
    fn immutable_digest(&self) -> std::io::Result<Digest256> {
        Ok(Digest256::of_bytes(&canonical_value_bytes_v1(
            &self.profile_value(),
        )?))
    }
    fn from_issued(
        issued: &super::source_admission_indexed_input::SelectedArtifactTemplateSelectionV1,
        max_state: usize,
        caller_state: usize,
    ) -> std::io::Result<Self> {
        if issued.route() != "synthetic_repository_resource_v1" {
            return Err(io_invalid("issued Artifact recipe route differs"));
        }
        let strings = issued
            .templates()
            .iter()
            .map(|r| r.source_path().len())
            .chain(
                issued
                    .resource_templates()
                    .iter()
                    .map(|r| r.source_path().len()),
            )
            .chain([
                issued.rights_template().source_path().len(),
                issued.discovery_template().source_path().len(),
                issued.source_policy().source_path().len(),
                issued.research().source_path().len(),
                issued.generated_at().len(),
            ])
            .try_fold(size_of::<Self>(), |n, bytes| n.checked_add(bytes))
            .and_then(|n| n.checked_add(caller_state))
            .ok_or_else(|| io_invalid("issued Artifact recipe copy state overflow"))?;
        if strings > max_state {
            return Err(io_invalid("issued Artifact recipe copy exceeds state"));
        }
        let source =
            |row: &super::source_admission_indexed_input::SelectedArtifactPinnedSourceV1| {
                WeightedScaleArtifactPinnedSourceV1 {
                    source_path: row.source_path().to_owned(),
                    source_sha256: row.source_sha256(),
                    source_bytes: row.source_bytes(),
                }
            };
        let result = Self {
            templates: std::array::from_fn(|i| {
                let row = &issued.templates()[i];
                WeightedScaleArtifactTemplateRowV1 {
                    source_path: row.source_path().to_owned(),
                    source_sha256: row.source_sha256(),
                    template_sha256: row.template_sha256(),
                    template_bytes: row.template_bytes(),
                }
            }),
            rights_template: source(issued.rights_template()),
            discovery_template: source(issued.discovery_template()),
            source_policy: source(issued.source_policy()),
            research: source(issued.research()),
            resource_templates: std::array::from_fn(|i| source(&issued.resource_templates()[i])),
            generated_at: issued.generated_at().to_owned(),
        };
        result.validate()?;
        if result
            .retained_state_bytes()?
            .checked_add(caller_state)
            .is_none_or(|n| n > max_state)
        {
            return Err(io_invalid(
                "issued Artifact recipe retained copy exceeds state",
            ));
        }
        Ok(result)
    }
    fn validate(&self) -> std::io::Result<()> {
        for row in &self.templates {
            RelativePath::parse(&row.source_path)
                .map_err(|_| io_invalid("selected Artifact template path invalid"))?;
            if !row.source_path.starts_with("ToS/") || row.template_bytes == 0 {
                return Err(io_invalid("selected Artifact template declaration invalid"));
            }
        }
        for source in self.resource_templates.iter().chain([
            &self.rights_template,
            &self.discovery_template,
            &self.source_policy,
            &self.research,
        ]) {
            RelativePath::parse(&source.source_path)
                .map_err(|_| io_invalid("selected Artifact source path invalid"))?;
            if !source.source_path.starts_with("ToS/") || source.source_bytes == 0 {
                return Err(io_invalid("selected Artifact source declaration invalid"));
            }
        }
        if self.generated_at.is_empty() {
            return Err(io_invalid("selected Artifact configured date absent"));
        }
        Ok(())
    }
    fn retained_state_bytes(&self) -> std::io::Result<usize> {
        self.templates
            .iter()
            .map(|r| r.source_path.capacity())
            .chain(
                self.resource_templates
                    .iter()
                    .map(|r| r.source_path.capacity()),
            )
            .chain([
                self.rights_template.source_path.capacity(),
                self.discovery_template.source_path.capacity(),
                self.source_policy.source_path.capacity(),
                self.research.source_path.capacity(),
                self.generated_at.capacity(),
            ])
            .try_fold(size_of::<Self>(), |n, bytes| {
                n.checked_add(bytes)
                    .ok_or_else(|| io_invalid("selected Artifact recipe state overflow"))
            })
    }
}

/// Finite authenticated recipe bytes. This holder proves selected input bytes;
/// its bodies are not admitted Artifact/support facts before actual rendering
/// and the maintained semantic consumers run.
struct WeightedScaleLoadedArtifactRecipeV1 {
    selection: WeightedScaleArtifactTemplateSelectionV1,
    templates: [Vec<u8>; 3],
    resources: [Vec<u8>; 3],
    rights: Vec<u8>,
    discovery: Vec<u8>,
}
impl WeightedScaleLoadedArtifactRecipeV1 {
    fn retained_state_bytes(&self) -> std::io::Result<usize> {
        let declaration_heap = self
            .selection
            .retained_state_bytes()?
            .checked_sub(size_of::<WeightedScaleArtifactTemplateSelectionV1>())
            .ok_or_else(|| io_invalid("Artifact recipe header accounting differs"))?;
        self.templates
            .iter()
            .chain(self.resources.iter())
            .chain([&self.rights, &self.discovery])
            .try_fold(
                size_of::<Self>()
                    .checked_add(declaration_heap)
                    .ok_or_else(|| io_invalid("Artifact recipe state overflow"))?,
                |n, raw| {
                    n.checked_add(raw.capacity())
                        .ok_or_else(|| io_invalid("Artifact recipe body state overflow"))
                },
            )
    }
    fn load_accounted(
        root: &Path,
        selection: WeightedScaleArtifactTemplateSelectionV1,
        io: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &AdmissionWorkBudget,
        max_state: usize,
        caller_state: usize,
    ) -> std::io::Result<Self> {
        selection.validate()?;
        let mut result = Self {
            selection,
            templates: std::array::from_fn(|_| Vec::new()),
            resources: std::array::from_fn(|_| Vec::new()),
            rights: Vec::new(),
            discovery: Vec::new(),
        };
        // At most ten source descriptors and eight bodies are retained. Policy
        // and research are read/hash-checked then dropped, never a cohort Vec.
        for index in 0..10 {
            scale_active(deadline, cancelled)?;
            let (relative, sha, length, schema) = match index {
                0..=2 => {
                    let row = &result.selection.templates[index];
                    if row.source_sha256 != row.template_sha256 {
                        return Err(io_invalid("Artifact whole-source template digest differs"));
                    }
                    (
                        row.source_path.as_str(),
                        row.source_sha256,
                        row.template_bytes,
                        Some("tos_artifact_source_witness_v1"),
                    )
                }
                3..=5 => {
                    let row = &result.selection.resource_templates[index - 3];
                    (
                        row.source_path.as_str(),
                        row.source_sha256,
                        row.source_bytes,
                        None,
                    )
                }
                6 => {
                    let row = &result.selection.rights_template;
                    (
                        row.source_path.as_str(),
                        row.source_sha256,
                        row.source_bytes,
                        Some("tos_rights_record_v1"),
                    )
                }
                7 => {
                    let row = &result.selection.discovery_template;
                    (
                        row.source_path.as_str(),
                        row.source_sha256,
                        row.source_bytes,
                        Some("tos_material_discovery_record_v1"),
                    )
                }
                8 => {
                    let row = &result.selection.source_policy;
                    (
                        row.source_path.as_str(),
                        row.source_sha256,
                        row.source_bytes,
                        None,
                    )
                }
                _ => {
                    let row = &result.selection.research;
                    (
                        row.source_path.as_str(),
                        row.source_sha256,
                        row.source_bytes,
                        None,
                    )
                }
            };
            let declared_length = usize::try_from(length)
                .map_err(|_| io_invalid("Artifact source length exceeds usize"))?;
            let held = result
                .retained_state_bytes()?
                .checked_add(caller_state)
                .ok_or_else(|| io_invalid("Artifact recipe caller state overflow"))?;
            let path_upper = root
                .as_os_str()
                .len()
                .checked_add(relative.len())
                .and_then(|n| n.checked_add(1))
                .and_then(|n| n.checked_add(size_of::<PathBuf>()))
                .ok_or_else(|| io_invalid("Artifact recipe source path state overflow"))?;
            let available = max_state
                .checked_sub(held)
                .and_then(|n| n.checked_sub(path_upper))
                .and_then(|n| n.checked_sub(size_of::<[u8; 64 * 1024]>()))
                .ok_or_else(|| io_invalid("Artifact recipe read workspace exceeds state"))?;
            if declared_length > available {
                return Err(io_invalid("Artifact selected source exceeds read state"));
            }
            let path = root.join(relative);
            let raw = read_source_file_accounted_v1(
                &path,
                declared_length,
                io,
                deadline,
                cancelled,
                work,
            )?;
            if raw.len() != declared_length || Digest256::of_bytes(&raw) != sha {
                return Err(io_invalid("Artifact selected source byte identity differs"));
            }
            if let Some(schema) = schema {
                let decoded_available = max_state
                    .checked_sub(held)
                    .and_then(|n| n.checked_sub(path_upper))
                    .and_then(|n| n.checked_sub(raw.capacity()))
                    .ok_or_else(|| {
                        io_invalid("Artifact selected JSON decode state exceeds frame")
                    })?;
                let (value, _) = tos_validation::record_biblio_cut::bounded_decoded_state(
                    &raw,
                    JsonLimits::default(),
                    decoded_available,
                    deadline,
                    cancelled,
                )
                .map_err(|_| io_invalid("Artifact selected JSON decode refused"))?;
                if value
                    .get("schema_version")
                    .and_then(serde_json::Value::as_str)
                    != Some(schema)
                {
                    return Err(io_invalid(
                        "Artifact selected template schema route differs",
                    ));
                }
                if index <= 2
                    && (value
                        .get("path_identity")
                        .and_then(|v| v.get("basis"))
                        .and_then(serde_json::Value::as_str)
                        != Some("repository_identity")
                        || value
                            .get("artifact_kind")
                            .and_then(serde_json::Value::as_str)
                            != Some("other"))
                {
                    return Err(io_invalid(
                        "Artifact selected template referent posture differs",
                    ));
                }
                // The decoded value is dropped before transferring this body's
                // raw custody; only selected bytes remain resident.
            }
            match index {
                0..=2 => result.templates[index] = raw,
                3..=5 => result.resources[index - 3] = raw,
                6 => result.rights = raw,
                7 => result.discovery = raw,
                _ => drop(raw),
            }
            if result
                .retained_state_bytes()?
                .checked_add(caller_state)
                .is_none_or(|n| n > max_state)
            {
                return Err(io_invalid("Artifact retained recipe exceeds caller state"));
            }
        }
        Ok(result)
    }
}

/// One completed output phase, not a per-ordinal clock measurement. Only the
/// producer's post-write fold constructs this private observation.
#[derive(Clone)]
struct WeightedScaleArtifactOutputObservationV1 {
    started_at: String,
    ended_at: String,
    member_count: u64,
    source_bytes: u64,
    ordered_output_sha256: Digest256,
}
struct WeightedScaleArtifactOutputPhaseV1 {
    started_at: String,
    member_count: u64,
    source_bytes: u64,
    ordered: Digest256Hasher,
    previous_path: String,
    failed: bool,
}
impl WeightedScaleArtifactOutputPhaseV1 {
    fn begin(immutable_recipe: Digest256, seed: Digest256) -> std::io::Result<Self> {
        let started_at = super::source_serialization::instant()
            .map_err(|_| io_invalid("Artifact output phase clock unavailable"))?;
        let mut ordered = Digest256Hasher::new();
        ordered.update(b"tos_scale_artifact_completed_output_phase_v1\0");
        ordered.update(immutable_recipe.as_bytes());
        ordered.update(seed.as_bytes());
        Ok(Self {
            started_at,
            member_count: 0,
            source_bytes: 0,
            ordered,
            previous_path: String::new(),
            failed: false,
        })
    }
    fn observe_written(
        &mut self,
        path: &str,
        sha: Digest256,
        bytes: u64,
        available_phase_state: usize,
    ) -> std::io::Result<()> {
        if self.failed {
            return Err(io_invalid("Artifact output phase already refused"));
        }
        self.failed = true;
        if !self.previous_path.is_empty() && self.previous_path.as_str() >= path {
            return Err(io_invalid("Artifact completed output phase order differs"));
        }
        RelativePath::parse(path)
            .map_err(|_| io_invalid("Artifact completed output phase path invalid"))?;
        self.member_count = self
            .member_count
            .checked_add(1)
            .ok_or_else(|| io_invalid("Artifact output phase count overflow"))?;
        self.source_bytes = self
            .source_bytes
            .checked_add(bytes)
            .ok_or_else(|| io_invalid("Artifact output phase bytes overflow"))?;
        self.ordered.update(&(path.len() as u64).to_be_bytes());
        self.ordered.update(path.as_bytes());
        self.ordered.update(sha.as_bytes());
        self.ordered.update(&bytes.to_be_bytes());
        let retained = size_of::<Self>()
            .checked_add(self.started_at.capacity())
            .and_then(|n| n.checked_add(self.previous_path.capacity().max(path.len())))
            .ok_or_else(|| io_invalid("Artifact output phase retained state overflow"))?;
        if retained > available_phase_state {
            return Err(io_invalid(
                "Artifact output phase path exceeds selected state",
            ));
        }
        self.previous_path.clear();
        if self.previous_path.capacity() < path.len() {
            self.previous_path
                .try_reserve_exact(path.len())
                .map_err(|_| io_invalid("Artifact output phase path allocation failed"))?;
        }
        self.previous_path.push_str(path);
        self.failed = false;
        Ok(())
    }
    fn finish(
        self,
        expected_count: u64,
    ) -> std::io::Result<WeightedScaleArtifactOutputObservationV1> {
        if self.failed || self.member_count != expected_count {
            return Err(io_invalid(
                "Artifact completed output phase EOF count differs",
            ));
        }
        let ended_at = super::source_serialization::instant()
            .map_err(|_| io_invalid("Artifact output phase clock unavailable"))?;
        Ok(WeightedScaleArtifactOutputObservationV1 {
            started_at: self.started_at,
            ended_at,
            member_count: self.member_count,
            source_bytes: self.source_bytes,
            ordered_output_sha256: self.ordered.finalize(),
        })
    }
}

fn artifact_recipe_seed_v1(seed: Digest256, recipe: Digest256) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos_scale_synthetic_repository_artifact_identity_v1\0");
    hash.update(seed.as_bytes());
    hash.update(recipe.as_bytes());
    hash.finalize()
}
fn artifact_support_identity_v1(
    seed: Digest256,
    recipe: Digest256,
    ordinal: u64,
    kind: &str,
) -> String {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos_scale_synthetic_repository_artifact_support_identity_v1\0");
    hash.update(seed.as_bytes());
    hash.update(recipe.as_bytes());
    hash.update(kind.as_bytes());
    hash.update(&ordinal.to_be_bytes());
    format!(
        "tos.{kind}.scale-fixture.{ordinal:020}.{}",
        hash.finalize().to_hex()
    )
}
struct GeneratedDigestWriterV1 {
    digest: Digest256Hasher,
    bytes: u64,
}
impl IoWrite for GeneratedDigestWriterV1 {
    fn write(&mut self, raw: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(raw.len() as u64)
            .ok_or_else(|| io_invalid("generated byte price overflow"))?;
        self.digest.update(raw);
        Ok(raw.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl WeightedScaleLoadedArtifactRecipeV1 {
    fn resource_binding(
        &self,
        seed: Digest256,
        recipe: Digest256,
        ordinal: u64,
    ) -> std::io::Result<(u64, Digest256)> {
        let index = selected_quantile_index_v1(ordinal % SCALE_SELECTED_QUANTILE_PERIOD_V1);
        let id = scale_identity(
            artifact_recipe_seed_v1(seed, recipe),
            WeightedScaleClassV1::Artifact,
            ordinal,
        );
        let mut writer = GeneratedDigestWriterV1 {
            digest: Digest256Hasher::new(),
            bytes: 0,
        };
        write_artifact_resource_v1(
            &mut writer,
            seed,
            recipe,
            ordinal,
            &id,
            self.selection.resource_templates[index].source_sha256,
            &self.resources[index],
        )?;
        Ok((writer.bytes, writer.digest.finalize()))
    }
    fn artifact_value(
        &self,
        seed: Digest256,
        recipe: Digest256,
        ordinal: u64,
        template_raw: &[u8],
    ) -> std::io::Result<serde_json::Value> {
        let mut value: serde_json::Value = serde_json::from_slice(template_raw)
            .map_err(|_| io_invalid("selected Artifact template JSON differs"))?;
        let original_uri = value
            .pointer("/digital_catalog_record/record_url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| io_invalid("Artifact template resource URI absent"))?
            .to_owned();
        let resource_uri = format!(
            "urn:tos:capacity-fixture:resource:{}:{ordinal:020}",
            artifact_recipe_seed_v1(seed, recipe).to_hex()
        );
        fn replace_uri(value: &mut serde_json::Value, old: &str, new: &str) {
            match value {
                serde_json::Value::String(s) if s == old => {
                    s.clear();
                    s.push_str(new);
                }
                serde_json::Value::Array(rows) => {
                    for row in rows {
                        replace_uri(row, old, new);
                    }
                }
                serde_json::Value::Object(fields) => {
                    for row in fields.values_mut() {
                        replace_uri(row, old, new);
                    }
                }
                _ => {}
            }
        }
        replace_uri(&mut value, &original_uri, &resource_uri);
        let id = scale_identity(
            artifact_recipe_seed_v1(seed, recipe),
            WeightedScaleClassV1::Artifact,
            ordinal,
        );
        let (resource_bytes, resource_sha) = self.resource_binding(seed, recipe, ordinal)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| io_invalid("Artifact template object absent"))?;
        set_string(object, "artifact_id", id.clone())?;
        set_string(
            object,
            "rights_ref",
            WeightedScaleArtifactSupportRoleV1::Rights.path(ordinal),
        )?;
        set_string(
            object,
            "discovery_ref",
            WeightedScaleArtifactSupportRoleV1::Discovery.path(ordinal),
        )?;
        set_string(
            object,
            "provenance_event_ref",
            artifact_support_identity_v1(seed, recipe, ordinal, "event"),
        )?;
        set_string(
            object,
            "research_ref",
            self.selection.research.source_path.clone(),
        )?;
        object.insert(
            "philosophy_planting_refs".to_owned(),
            serde_json::json!([self.selection.source_policy.source_path]),
        );
        set_string(object, "created_at", self.selection.generated_at.clone())?;
        object.insert("maker".to_owned(), serde_json::json!({"maker_type":"software", "agent_ref":SCALE_GENERATOR_AGENT_REF_V1, "human_review_performed":false}));
        let catalog = object
            .get_mut("digital_catalog_record")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| io_invalid("Artifact template catalog absent"))?;
        set_string(catalog, "record_id", id)?;
        set_string(catalog, "checked_at", self.selection.generated_at.clone())?;
        catalog.insert("response_fingerprints".to_owned(), serde_json::json!([
            {"surface":resource_uri, "byte_size":resource_bytes, "sha256":resource_sha.to_hex(), "captured":false}
        ]));
        let custody = object
            .get_mut("custody")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| io_invalid("Artifact template custody absent"))?;
        custody.insert(
            "inventory_numbers".to_owned(),
            serde_json::json!([
                resource_uri,
                WeightedScaleArtifactSupportRoleV1::Resource.path(ordinal)
            ]),
        );
        Ok(value)
    }
    fn encoded_binding(value: &serde_json::Value) -> std::io::Result<(u64, Digest256)> {
        let mut writer = GeneratedDigestWriterV1 {
            digest: Digest256Hasher::new(),
            bytes: 0,
        };
        write_fixture_record_value_v1(&mut writer, value, WeightedScaleClassV1::Artifact)?;
        Ok((writer.bytes, writer.digest.finalize()))
    }
    fn event_value(
        &self,
        seed: Digest256,
        recipe: Digest256,
        ordinal: u64,
        observation: &WeightedScaleArtifactOutputObservationV1,
        generator_agent: &WeightedScaleGeneratorAgentSelectionV1,
        artifact_raw: &[u8],
    ) -> std::io::Result<serde_json::Value> {
        let index = selected_quantile_index_v1(ordinal % SCALE_SELECTED_QUANTILE_PERIOD_V1);
        let (_, artifact_sha) =
            Self::encoded_binding(&self.artifact_value(seed, recipe, ordinal, artifact_raw)?)?;
        let (_, rights_sha) = Self::encoded_binding(&self.rights_value(seed, recipe, ordinal)?)?;
        let (_, discovery_sha) =
            Self::encoded_binding(&self.discovery_value(seed, recipe, ordinal)?)?;
        let (_, resource_sha) = self.resource_binding(seed, recipe, ordinal)?;
        Ok(serde_json::json!({
            "schema_version":"tos_provenance_event_v1",
            "event_id":artifact_support_identity_v1(seed, recipe, ordinal, "event"),
            "event_type":"render", "started_at":observation.started_at,
            "ended_at":observation.ended_at, "agent_refs":[generator_agent.record_id],
            "inputs":[
                {"ref":generator_agent.source_path, "role":"selected provisional software responsibility bearer", "sha256":generator_agent.source_sha256.to_hex()},
                {"ref":self.selection.templates[index].source_path, "role":"selected synthetic Artifact template", "sha256":self.selection.templates[index].source_sha256.to_hex()},
                {"ref":self.selection.resource_templates[index].source_path, "role":"selected resource byte template", "sha256":self.selection.resource_templates[index].source_sha256.to_hex()},
                {"ref":self.selection.rights_template.source_path, "role":"authored metadata-only rights policy template", "sha256":self.selection.rights_template.source_sha256.to_hex()},
                {"ref":self.selection.discovery_template.source_path, "role":"authored planned discovery template", "sha256":self.selection.discovery_template.source_sha256.to_hex()}
            ],
            "outputs":[
                {"ref":path_for(WeightedScaleClassV1::Artifact, ordinal), "role":"synthetic Artifact witness", "sha256":artifact_sha.to_hex()},
                {"ref":WeightedScaleArtifactSupportRoleV1::Resource.path(ordinal), "role":"distinct repository byte resource", "sha256":resource_sha.to_hex()},
                {"ref":WeightedScaleArtifactSupportRoleV1::Rights.path(ordinal), "role":"instantiated metadata-only scope", "sha256":rights_sha.to_hex()},
                {"ref":WeightedScaleArtifactSupportRoleV1::Discovery.path(ordinal), "role":"instantiated planned resource selection", "sha256":discovery_sha.to_hex()},
                {"ref":self.selection.research.source_path, "role":"retained authored research boundary", "sha256":self.selection.research.source_sha256.to_hex()},
                {"ref":self.selection.source_policy.source_path, "role":"retained authored philosophical planting and rights boundary", "sha256":self.selection.source_policy.source_sha256.to_hex()}
            ],
            "method":{"maker_type":"software", "name":"tos weighted capacity fixture renderer", "version":"2",
                "configuration":{"seed_sha256":seed.to_hex(), "immutable_recipe_sha256":recipe.to_hex(),
                    "ordinal":ordinal, "logical_generated_at":self.selection.generated_at,
                    "completed_batch_member_count":observation.member_count,
                    "completed_batch_source_bytes":observation.source_bytes,
                    "completed_batch_ordered_output_sha256":observation.ordered_output_sha256.to_hex(),
                    "timing_scope":"shared completed output batch; no per-ordinal timing or clock trust"}},
            "status":"completed_with_warnings", "event_version":1,
            "warnings":["Synthetic repository byte referent, not an archaeological object or copied native creation package.",
                "Configured fixture dates and planned discovery fields do not claim an observed query, acquisition or per-ordinal timing.",
                "Rights metadata applies the authored model policy only; no legal, payload publication, human review or canon grant is asserted."],
            "receipt_refs":[]
        }))
    }
    fn rights_value(
        &self,
        seed: Digest256,
        recipe: Digest256,
        ordinal: u64,
    ) -> std::io::Result<serde_json::Value> {
        let mut value: serde_json::Value = serde_json::from_slice(&self.rights)
            .map_err(|_| io_invalid("selected rights template JSON differs"))?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| io_invalid("selected rights object absent"))?;
        set_string(
            object,
            "rights_id",
            artifact_support_identity_v1(seed, recipe, ordinal, "rights"),
        )?;
        object.insert(
            "scope_refs".to_owned(),
            serde_json::json!([scale_identity(
                artifact_recipe_seed_v1(seed, recipe),
                WeightedScaleClassV1::Artifact,
                ordinal
            )]),
        );
        object.insert(
            "source_refs".to_owned(),
            serde_json::json!([self.selection.source_policy.source_path]),
        );
        // Assessment identity/posture remain the actual authored model policy;
        // generated scope does not assert a legal or publication grant.
        Ok(value)
    }
    fn discovery_value(
        &self,
        seed: Digest256,
        recipe: Digest256,
        ordinal: u64,
    ) -> std::io::Result<serde_json::Value> {
        let mut value: serde_json::Value = serde_json::from_slice(&self.discovery)
            .map_err(|_| io_invalid("selected discovery template JSON differs"))?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| io_invalid("selected discovery object absent"))?;
        if object.get("status").and_then(serde_json::Value::as_str) != Some("planned") {
            return Err(io_invalid(
                "synthetic discovery must retain planned posture",
            ));
        }
        set_string(
            object,
            "discovery_id",
            artifact_support_identity_v1(seed, recipe, ordinal, "discovery"),
        )?;
        object.insert(
            "provenance_event_refs".to_owned(),
            serde_json::json!([artifact_support_identity_v1(seed, recipe, ordinal, "event")]),
        );
        let target = object
            .get_mut("target")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| io_invalid("selected discovery target absent"))?;
        set_string(target, "target_kind", "artifact".to_owned())?;
        target.insert(
            "known_tos_refs".to_owned(),
            serde_json::json!([scale_identity(
                artifact_recipe_seed_v1(seed, recipe),
                WeightedScaleClassV1::Artifact,
                ordinal
            )]),
        );
        let channels = object
            .get_mut("channels")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| io_invalid("selected discovery channels absent"))?;
        for channel in channels {
            let channel = channel
                .as_object_mut()
                .ok_or_else(|| io_invalid("selected discovery channel object absent"))?;
            set_string(
                channel,
                "endpoint_url",
                format!(
                    "urn:tos:capacity-fixture:resource:{}:{ordinal:020}",
                    artifact_recipe_seed_v1(seed, recipe).to_hex()
                ),
            )?;
            // queried_at/zero elapsed fields are authored planning placeholders;
            // no query, download or per-ordinal performance event is fabricated.
        }
        Ok(value)
    }
}

/// Same encoder is used for prewrite pricing, actual output and CompareWriter
/// verification. The selected prototype body is preserved as actual bytes.
fn write_artifact_resource_v1(
    writer: &mut impl IoWrite,
    seed: Digest256,
    immutable_recipe: Digest256,
    ordinal: u64,
    artifact_id: &str,
    source_sha: Digest256,
    body: &[u8],
) -> std::io::Result<()> {
    writeln!(writer, "tos_synthetic_repository_resource_v1")?;
    writeln!(writer, "seed_sha256={}", seed.to_hex())?;
    writeln!(
        writer,
        "immutable_recipe_sha256={}",
        immutable_recipe.to_hex()
    )?;
    writeln!(writer, "ordinal={ordinal:020}")?;
    writeln!(writer, "artifact_id={artifact_id}")?;
    writeln!(writer, "source_resource_sha256={}", source_sha.to_hex())?;
    writer.write_all(b"\n")?;
    writer.write_all(body)
}

/// Fixed generated support roles; physical support is distinct from the five
/// meaningful workload classes. A resource is custody-only, not an owner fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WeightedScaleArtifactSupportRoleV1 {
    Resource,
    Rights,
    Discovery,
    Event,
}
impl WeightedScaleArtifactSupportRoleV1 {
    pub(crate) const ALL: [Self; 4] = [Self::Resource, Self::Rights, Self::Discovery, Self::Event];
    pub(crate) fn path(self, ordinal: u64) -> String {
        match self {
            Self::Resource => format!(
                "ToS/source-witnesses/artifacts/scale-fixtures/v1/{ordinal:020}/source-resource.txt"
            ),
            Self::Rights => {
                format!("ToS/source-witnesses/rights/scale-fixtures/v1/{ordinal:020}/rights.json")
            }
            Self::Discovery => format!(
                "ToS/source-witnesses/discovery/runs/scale-fixtures/v1/{ordinal:020}/discovery.json"
            ),
            Self::Event => format!(
                "ToS/source-witnesses/discovery/events/scale-fixtures/v1/{ordinal:020}/provenance.jsonl"
            ),
        }
    }
    pub(crate) fn is_semantic(self) -> bool {
        self != Self::Resource
    }
    pub(crate) fn is_phase_one(self) -> bool {
        self != Self::Event
    }
}
fn artifact_support_member_count_v1(artifact_count: u64) -> std::io::Result<u64> {
    artifact_count
        .checked_mul(WeightedScaleArtifactSupportRoleV1::ALL.len() as u64)
        .ok_or_else(|| io_invalid("generated Artifact support count overflow"))
}
fn artifact_phase_one_member_count_v1(
    meaningful_count: u64,
    artifact_count: u64,
    auxiliary_count: u64,
) -> std::io::Result<u64> {
    let phase_roles = WeightedScaleArtifactSupportRoleV1::ALL
        .iter()
        .filter(|role| role.is_phase_one())
        .count() as u64;
    artifact_count
        .checked_mul(phase_roles)
        .and_then(|n| n.checked_add(meaningful_count))
        .and_then(|n| n.checked_add(auxiliary_count))
        .ok_or_else(|| io_invalid("generated Artifact phase count overflow"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedScaleTemplateV1 {
    pub class: WeightedScaleClassV1,
    pub source_path: std::borrow::Cow<'static, str>,
    pub source_sha256: Digest256,
    pub template_sha256: Digest256,
    pub bytes: Vec<u8>,
    external_reference_edges: Vec<ScaleReferenceEdgeV1>,
    claim_route: WeightedScaleClaimTemplateRouteV1,
    generated_author: Option<String>,
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
    claim_selection: Option<WeightedScaleClaimTemplateSelectionV1>,
    artifact_recipe: Option<WeightedScaleLoadedArtifactRecipeV1>,
    fixture_recipe_source: Option<WeightedScaleFixtureRecipeSourceV2>,
    output_observation: Option<WeightedScaleArtifactOutputObservationV1>,
}

impl WeightedScaleTemplateSetV1 {
    pub(crate) fn load_with_all5_selection_accounted(
        root: &Path,
        profile: &WeightedScaleProfileV1,
        claim: WeightedScaleClaimTemplateSelectionV1,
        artifact: WeightedScaleArtifactTemplateSelectionV1,
        io: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &AdmissionWorkBudget,
        max_state: usize,
        caller_state: usize,
    ) -> std::io::Result<Self> {
        let artifact_state = artifact.retained_state_bytes()?;
        let mut result = Self::load_with_claim_selection_accounted(
            root,
            profile,
            claim,
            io,
            deadline,
            cancelled,
            work,
            max_state,
            caller_state
                .checked_add(artifact_state)
                .ok_or_else(|| io_invalid("ALL5 selection state overflow"))?,
            true,
        )?;
        // The explicit Artifact route reads only its selected bodies. Legacy
        // loading retains its own three historical Artifact templates.
        let prior = weighted_producer_state_upper_v1(&result, 0)?;
        let mut loaded = WeightedScaleLoadedArtifactRecipeV1::load_accounted(
            root,
            artifact,
            io,
            deadline,
            cancelled,
            work,
            max_state,
            caller_state
                .checked_add(prior)
                .ok_or_else(|| io_invalid("ALL5 prior state overflow"))?,
        )?;
        for index in 0..3 {
            scale_active(deadline, cancelled)?;
            work.charge_many(1)?;
            let raw = std::mem::take(&mut loaded.templates[index]);
            let row = &loaded.selection.templates[index];
            let live = weighted_producer_state_upper_v1(&result, 0)?
                .checked_add(loaded.retained_state_bytes()?)
                .and_then(|n| n.checked_add(raw.capacity()))
                .and_then(|n| n.checked_add(caller_state))
                .ok_or_else(|| io_invalid("ALL5 template live state overflow"))?;
            let available = max_state
                .checked_sub(live)
                .ok_or_else(|| io_invalid("ALL5 template decode state exhausted"))?;
            let (value, decoded) = tos_validation::record_biblio_cut::bounded_decoded_state(
                &raw,
                JsonLimits::default(),
                available,
                deadline,
                cancelled,
            )
            .map_err(|_| io_invalid("ALL5 Artifact selected decode refused"))?;
            let (_, _, edge_upper) = reference_collection_state_upper_v1(&value)?;
            let path_upper = row.source_path.len();
            if live
                .checked_add(decoded)
                .and_then(|n| n.checked_add(edge_upper))
                .and_then(|n| n.checked_add(path_upper))
                .is_none_or(|n| n > max_state)
            {
                return Err(io_invalid("ALL5 Artifact selected edges exceed state"));
            }
            let edges = collect_reference_edges_v1(&value)?;
            result.templates[WeightedScaleClassV1::Artifact as usize].push(
                WeightedScaleTemplateV1 {
                    class: WeightedScaleClassV1::Artifact,
                    source_path: std::borrow::Cow::Owned(row.source_path.clone()),
                    source_sha256: row.source_sha256,
                    template_sha256: row.template_sha256,
                    bytes: raw,
                    external_reference_edges: edges,
                    claim_route: WeightedScaleClaimTemplateRouteV1::WorkAuthorshipFixtureV1,
                    generated_author: None,
                },
            );
        }
        result.artifact_recipe = Some(loaded);
        result.manifest_sha256 = template_manifest_digest_v1(&result.templates)?;
        result.resident_template_bytes = result
            .templates
            .iter()
            .flatten()
            .try_fold(0u64, |n, t| n.checked_add(t.bytes.len() as u64))
            .ok_or_else(|| io_invalid("ALL5 template bytes overflow"))?;
        result.resident_reference_state_bytes = result
            .templates
            .iter()
            .flatten()
            .flat_map(|t| &t.external_reference_edges)
            .try_fold(0u64, |n, e| {
                n.checked_add((e.pointer.len() + e.reference.len()) as u64)
            })
            .ok_or_else(|| io_invalid("ALL5 template references overflow"))?;
        if weighted_producer_state_upper_v1(&result, 0)?
            .checked_add(caller_state)
            .is_none_or(|n| n > max_state)
        {
            return Err(io_invalid("ALL5 retained state exceeds frame"));
        }
        Ok(result)
    }
    pub(crate) fn source_recipe(&self) -> Option<&WeightedScaleFixtureRecipeSourceV2> {
        self.fixture_recipe_source.as_ref()
    }

    pub fn load(repository_root: &Path) -> std::io::Result<Self> {
        Self::load_inner(repository_root, None, false, false, None)
    }

    pub(crate) fn load_accounted(
        repository_root: &Path,
        io: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &AdmissionWorkBudget,
    ) -> std::io::Result<Self> {
        Self::load_inner(
            repository_root,
            Some((io, deadline, cancelled, work)),
            false,
            false,
            None,
        )
    }

    fn load_inner(
        repository_root: &Path,
        accounting: Option<(
            &PinnedSqliteIoBudget,
            Instant,
            &AtomicBool,
            &AdmissionWorkBudget,
        )>,
        omit_legacy_claims: bool,
        omit_legacy_artifacts: bool,
        state_limit: Option<usize>,
    ) -> std::io::Result<Self> {
        let pins = scale_template_pins_v1();
        let mut templates: [Vec<WeightedScaleTemplateV1>; 5] = std::array::from_fn(|_| Vec::new());
        let mut resident = 0u64;
        for pin in pins {
            if (omit_legacy_claims && pin.class == WeightedScaleClassV1::Claim)
                || (omit_legacy_artifacts && pin.class == WeightedScaleClassV1::Artifact)
            {
                continue;
            }
            let retained = template_array_retained_state_v1(&templates)?;
            let path_upper = repository_root
                .as_os_str()
                .len()
                .checked_add(pin.path.len())
                .and_then(|n| n.checked_add(1 + size_of::<PathBuf>()))
                .ok_or_else(|| io_invalid("template source path state overflow"))?;
            let available = state_limit
                .map(|limit| {
                    limit
                        .checked_sub(retained)
                        .and_then(|n| n.checked_sub(path_upper))
                        .ok_or_else(|| io_invalid("template loading exceeds remaining state"))
                })
                .transpose()?;
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
                if available.is_some_and(|limit| metadata.len() > limit as u64) {
                    return Err(io_invalid("template file exceeds remaining state"));
                }
                // These bytes are read into the template payload below. The
                // guard-only upper-bound permit cannot authorize returned bytes.
                io.charge_read(metadata.len())
                    .map_err(|_| io_invalid("pinned template read budget exhausted"))?;
                let file_bytes = fs::read(&path)?;
                io.record_read_returned(file_bytes.len() as u64)
                    .map_err(|_| io_invalid("pinned template read accounting failed"))?;
                if file_bytes.len() as u64 != metadata.len() {
                    return Err(io_invalid("pinned template changed while reading"));
                }
                Self::push_template(
                    &mut templates,
                    &mut resident,
                    pin,
                    file_bytes,
                    available.map(|limit| (limit, deadline, cancelled)),
                )?;
            } else {
                let file_bytes = fs::read(&path)?;
                Self::push_template(&mut templates, &mut resident, pin, file_bytes, None)?;
            }
        }
        let mut reference_state = 0u64;
        for (class, rows) in templates.iter_mut().enumerate() {
            rows.sort_by_key(|template| template.bytes.len());
            let omitted = (omit_legacy_claims && class == WeightedScaleClassV1::Claim as usize)
                || (omit_legacy_artifacts && class == WeightedScaleClassV1::Artifact as usize);
            if rows.len() != 3 && !(omitted && rows.is_empty()) {
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
            claim_selection: None,
            artifact_recipe: None,
            fixture_recipe_source: None,
            output_observation: None,
        })
    }

    pub(crate) fn load_with_claim_selection_accounted(
        repository_root: &Path,
        profile: &WeightedScaleProfileV1,
        selection: WeightedScaleClaimTemplateSelectionV1,
        io: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &AdmissionWorkBudget,
        max_state_bytes: usize,
        caller_live_state_bytes: usize,
        omit_legacy_artifacts: bool,
    ) -> std::io::Result<Self> {
        scale_active(deadline, cancelled)?;
        let selection_state = selection.retained_state_bytes()?;
        let available = max_state_bytes
            .checked_sub(caller_live_state_bytes)
            .and_then(|bytes| bytes.checked_sub(selection_state))
            .ok_or_else(|| io_invalid("selected Claim declaration exceeds state"))?;
        profile.validate()?;
        let expected =
            WeightedScaleProfileV1::weighted_for_records(profile.seed, profile.target_records)?;
        if profile
            .classes
            .iter()
            .zip(expected.classes.iter())
            .any(|(selected, expected)| {
                selected.class != expected.class || selected.count != expected.count
            })
        {
            return Err(io_invalid(
                "selected class counts differ from maintained declaration",
            ));
        }
        let agent_path_state = selection
            .generator_agent
            .source_path
            .len()
            .checked_add(size_of::<RelativePath>())
            .and_then(|n| n.checked_add(repository_root.as_os_str().len()))
            .and_then(|n| n.checked_add(selection.generator_agent.source_path.len()))
            .and_then(|n| n.checked_add(1 + size_of::<PathBuf>()))
            .ok_or_else(|| io_invalid("generator Agent path state overflow"))?;
        let agent_available = available
            .checked_sub(agent_path_state)
            .ok_or_else(|| io_invalid("generator Agent path exceeds state"))?;
        let agent_path = RelativePath::parse(&selection.generator_agent.source_path)
            .map_err(|_| io_invalid("generator Agent source path invalid"))?;
        if !agent_path
            .as_str()
            .starts_with("ToS/source-witnesses/agents/")
            || !agent_path.as_str().ends_with("/agent.json")
            || !selection
                .generator_agent
                .record_id
                .starts_with("tos.agent.")
        {
            return Err(io_invalid("generator Agent declared owner differs"));
        }
        let agent_raw = read_source_file_accounted_v1(
            &repository_root.join(agent_path.as_str()),
            agent_available,
            io,
            deadline,
            cancelled,
            work,
        )?;
        if Digest256::of_bytes(&agent_raw) != selection.generator_agent.source_sha256 {
            return Err(io_invalid("generator Agent raw source digest differs"));
        }
        let (agent, _) = tos_validation::record_biblio_cut::bounded_decoded_state(
            &agent_raw,
            JsonLimits::default(),
            agent_available
                .checked_sub(agent_raw.capacity())
                .ok_or_else(|| io_invalid("generator Agent raw state exceeds bound"))?,
            deadline,
            cancelled,
        )
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("generator Agent bounded decoding: {e:?}"),
            )
        })?;
        if agent["schema_version"] != "tos_corpus_record_v1"
            || agent["record_type"] != "agent"
            || agent["record_id"].as_str() != Some(selection.generator_agent.record_id.as_str())
            || agent["identity_status"] != "provisional"
            || agent["same_as_posture"] != "no_equivalence_claim"
            || !agent["source_refs"]
                .as_array()
                .is_some_and(|refs| !refs.is_empty())
        {
            return Err(io_invalid("generator Agent source record differs"));
        }
        // This verifies selected source bytes and identity only. Responsibility
        // qualification, schema verdict and semantic acceptance stay with their
        // authored assessment and actual maintained downstream owners.
        drop(agent);
        drop(agent_raw);
        drop(agent_path);
        let registry_raw = read_source_file_accounted_v1(
            &repository_root.join("ToS/doctrine/semantic-interchange/relation-types.v1.json"),
            available,
            io,
            deadline,
            cancelled,
            work,
        )?;
        if Digest256::of_bytes(&registry_raw) != selection.relation_registry_sha256 {
            return Err(io_invalid(
                "selected Claim relation registry identity differs",
            ));
        }
        let (registry, registry_state) = tos_validation::record_biblio_cut::bounded_decoded_state(
            &registry_raw,
            JsonLimits::default(),
            available
                .checked_sub(registry_raw.len())
                .ok_or_else(|| io_invalid("selected Claim registry state exceeds bound"))?,
            deadline,
            cancelled,
        )
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("selected Claim registry decoding: {e:?}"),
            )
        })?;
        let relation = registry["relations"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|row| row["relation_type_id"].as_str() == Some("tos.relation.authored-by"))
            .ok_or_else(|| io_invalid("selected Work authorship relation absent"))?;
        let claim_profile = &relation["source_claim_profile"];
        let mapping = relation["source_mappings"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|row| row["source_graph"] == "source-claims" && row["scope"] == "claim-predicate")
            .ok_or_else(|| io_invalid("selected Work authorship mapping absent"))?;
        let predicate = mapping["source_predicate_id"]
            .as_str()
            .ok_or_else(|| io_invalid("selected Work authorship predicate absent"))?;
        if !relation["domain_type_ids"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == "tos.entity.work"))
            || !relation["range_type_ids"]
                .as_array()
                .is_some_and(|a| a.len() == 1 && a[0] == "tos.entity.agent")
            || claim_profile["reader"] != "identity-relation-v1"
            || relation["abstract"] != false
            || relation["assertion_mode"] != "reified-claim"
            || relation["evidence_required"] != true
        {
            return Err(io_invalid("selected Work authorship owner profile differs"));
        }
        let selected_headers = selection
            .templates
            .len()
            .checked_mul(size_of::<WeightedScaleTemplateV1>())
            .ok_or_else(|| io_invalid("selected template headers overflow"))?;
        if registry_raw
            .len()
            .checked_add(registry_state)
            .and_then(|n| n.checked_add(selected_headers))
            .is_none_or(|n| n > available)
        {
            return Err(io_invalid("selected template headers exceed state"));
        }
        let mut selected = Vec::with_capacity(selection.templates.len());
        let mut resident = selected_headers;
        for (row_index, row) in selection.templates.iter().enumerate() {
            scale_active(deadline, cancelled)?;
            work.charge_many(1)?;
            let source_path_state = row
                .source_path
                .len()
                .checked_add(size_of::<RelativePath>())
                .and_then(|n| n.checked_add(repository_root.as_os_str().len()))
                .and_then(|n| n.checked_add(row.source_path.len()))
                .and_then(|n| n.checked_add(1 + size_of::<PathBuf>()))
                .ok_or_else(|| io_invalid("selected source path state overflow"))?;
            if registry_raw
                .len()
                .checked_add(registry_state)
                .and_then(|n| n.checked_add(resident))
                .and_then(|n| n.checked_add(source_path_state))
                .is_none_or(|n| n > available)
            {
                return Err(io_invalid("selected source path exceeds state"));
            }
            let path = RelativePath::parse(&row.source_path)
                .map_err(|_| io_invalid("selected Claim path is not relative"))?;
            if !tos_source_store::is_authored_source_path_v1(path.as_str())
                || !path.as_str().starts_with("ToS/source-witnesses/")
                || path.as_str().rsplit('/').next() != Some("source-claims.jsonl")
                || row.source_line == 0
                || row.template_bytes == 0
                || row.template_bytes > SCALE_MAX_TEMPLATE_BYTES_V1 as u64
                || selection.templates[..row_index].iter().any(|previous| {
                    previous.source_path == row.source_path
                        && previous.source_line == row.source_line
                })
            {
                return Err(io_invalid("selected Claim source row declaration differs"));
            }
            let file_available = available
                .checked_sub(registry_raw.len())
                .and_then(|n| n.checked_sub(registry_state))
                .and_then(|n| n.checked_sub(resident))
                .and_then(|n| n.checked_sub(source_path_state))
                .ok_or_else(|| io_invalid("selected Claim loading state exceeds bound"))?;
            let file = read_source_file_accounted_v1(
                &repository_root.join(path.as_str()),
                file_available,
                io,
                deadline,
                cancelled,
                work,
            )?;
            if Digest256::of_bytes(&file) != row.source_sha256 {
                return Err(io_invalid("selected Claim source file digest differs"));
            }
            let mut offset = 0usize;
            let mut selected_raw = None;
            for (physical_line, bytes) in
                tos_validation::source_record_selection::source_rows(&file)
            {
                scale_active(deadline, cancelled)?;
                work.charge_many(1)?;
                let start = offset;
                offset = offset
                    .checked_add(bytes.len())
                    .ok_or_else(|| io_invalid("selected Claim row offset overflow"))?;
                if let Some(delimiter) = file
                    .get(offset)
                    .copied()
                    .filter(|byte| matches!(byte, b'\r' | b'\n'))
                {
                    offset += 1;
                    if delimiter == b'\r' && file.get(offset) == Some(&b'\n') {
                        offset += 1;
                    }
                }
                if physical_line == row.source_line {
                    selected_raw = Some(&file[start..offset]);
                    break;
                }
            }
            let raw = selected_raw.ok_or_else(|| io_invalid("selected Claim row absent"))?;
            if raw.len() as u64 != row.template_bytes
                || Digest256::of_bytes(raw) != row.template_sha256
            {
                return Err(io_invalid("selected Claim row digest or size differs"));
            }
            let (value, value_state) = tos_validation::record_biblio_cut::bounded_decoded_state(
                raw,
                JsonLimits::default(),
                file_available
                    .checked_sub(file.len())
                    .ok_or_else(|| io_invalid("selected Claim row state exceeds bound"))?,
                deadline,
                cancelled,
            )
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("selected Claim row decoding: {e:?}"),
                )
            })?;
            let version = value["schema_version"].as_str();
            if value["predicate"].as_str() != Some(predicate)
                || !claim_profile["schemas"].as_array().is_some_and(|a| {
                    a.iter().any(|route| {
                        route["schema_version"].as_str() == version
                            && route["schema_ref"]
                                == "ToS/contracts/source-relation-claim.schema.json"
                    })
                })
                || value["claim_type"] != "relation"
                || !value["subject_ref"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("tos.work."))
                || !value["object"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("tos.agent."))
                || !claim_profile["assertion_layers"]
                    .as_array()
                    .is_some_and(|a| a.contains(&value["assertion_layer"]))
            {
                return Err(io_invalid(
                    "selected Claim row does not match Work authorship profile",
                ));
            }
            let (_, _, edges_upper) = reference_collection_state_upper_v1(&value)?;
            let additional = raw
                .len()
                .checked_add(row.source_path.len())
                .and_then(|n| n.checked_add(edges_upper))
                .and_then(|n| {
                    n.checked_add(selection.generator_agent.record_id.len().checked_mul(2)?)
                })
                .ok_or_else(|| io_invalid("selected template allocation overflow"))?;
            if file
                .len()
                .checked_add(value_state)
                .and_then(|n| n.checked_add(additional))
                .is_none_or(|n| n > file_available)
            {
                return Err(io_invalid("selected template allocation exceeds state"));
            }
            let mut external_reference_edges = collect_reference_edges_v1(&value)?;
            for edge in &mut external_reference_edges {
                if edge.pointer == "/object" {
                    edge.reference = selection.generator_agent.record_id.clone();
                }
            }
            external_reference_edges.sort_unstable();
            let template = WeightedScaleTemplateV1 {
                class: WeightedScaleClassV1::Claim,
                source_path: std::borrow::Cow::Owned(row.source_path.clone()),
                source_sha256: row.source_sha256,
                template_sha256: row.template_sha256,
                bytes: raw.to_vec(),
                external_reference_edges,
                claim_route: WeightedScaleClaimTemplateRouteV1::WorkAuthorshipFixtureV1,
                generated_author: Some(selection.generator_agent.record_id.clone()),
            };
            resident = selected_headers
                .checked_add(template_rows_retained_state_v1(&selected)?)
                .and_then(|n| {
                    template_rows_retained_state_v1(std::slice::from_ref(&template))
                        .ok()
                        .and_then(|m| n.checked_add(m))
                })
                .ok_or_else(|| io_invalid("selected template retained state overflow"))?;
            selected.push(template);
        }
        drop(registry);
        drop(registry_raw);
        let mut result = Self::load_inner(
            repository_root,
            Some((io, deadline, cancelled, work)),
            true,
            omit_legacy_artifacts,
            Some(
                available
                    .checked_sub(resident)
                    .ok_or_else(|| io_invalid("selected rows exceed loading state"))?,
            ),
        )?;
        result.templates[WeightedScaleClassV1::Claim as usize] = selected;
        // The explicit opt-in route also repairs cross-family generated refs.
        // Legacy load/rendering keeps its original technical-only route.
        for template in result.templates.iter_mut().flatten() {
            template.claim_route = WeightedScaleClaimTemplateRouteV1::WorkAuthorshipFixtureV1;
        }
        // Reuse the existing producer's complete template/scratch state law
        // before rendering prices, while all selected templates remain live.
        if weighted_producer_state_upper_v1(&result, 0)? > available {
            return Err(io_invalid("selected template pricing exceeds state"));
        }
        let mut prices = Vec::with_capacity(selection.templates.len());
        for template in &result.templates[WeightedScaleClassV1::Claim as usize] {
            let value = fixture_record_value_v1(profile, template, WeightedScaleClassV1::Claim, 0)?;
            prices.push(fixture_record_encoded_length_v1(
                &value,
                WeightedScaleClassV1::Claim,
            )?);
        }
        let rows = &mut result.templates[WeightedScaleClassV1::Claim as usize];
        // Stable fixed-three ordering follows real transformed payload prices.
        let mut order = [0usize, 1, 2];
        order.sort_by_key(|index| (prices[*index], *index));
        let owned = std::mem::take(rows);
        let mut owned: [Option<WeightedScaleTemplateV1>; 3] = owned
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| io_invalid("selected Claim row count differs"))?;
        for index in order {
            rows.push(
                owned[index]
                    .take()
                    .ok_or_else(|| io_invalid("selected Claim ordering differs"))?,
            );
        }
        let mut selection = selection;
        let mut declared = selection.templates.map(Some);
        selection.templates = order.map(|index| {
            declared[index]
                .take()
                .expect("fixed distinct three-way permutation")
        });
        result.claim_selection = Some(selection);
        // Rewriting Work responsibility links can reverse the source-size
        // order. Quantiles describe generated bytes, so keep each complete
        // template paired with its generated price before assigning buckets.
        let rows = &mut result.templates[WeightedScaleClassV1::Work as usize];
        let mut prices = [0usize; 3];
        for (index, template) in rows.iter().enumerate() {
            let value = fixture_record_value_v1(profile, template, WeightedScaleClassV1::Work, 0)?;
            prices[index] = fixture_record_encoded_length_v1(&value, WeightedScaleClassV1::Work)?;
        }
        for index in 1..prices.len() {
            let mut position = index;
            while position > 0 && prices[position] < prices[position - 1] {
                prices.swap(position, position - 1);
                rows.swap(position, position - 1);
                position -= 1;
            }
        }
        result.resident_template_bytes = result
            .templates
            .iter()
            .flatten()
            .map(|t| t.bytes.len() as u64)
            .sum();
        result.resident_reference_state_bytes = result
            .templates
            .iter()
            .flatten()
            .flat_map(|t| &t.external_reference_edges)
            .try_fold(0u64, |n, e| {
                n.checked_add((e.pointer.len() + e.reference.len()) as u64)
            })
            .ok_or_else(|| io_invalid("selected Claim reference state overflow"))?;
        result.manifest_sha256 = template_manifest_digest_v1(&result.templates)?;
        Ok(result)
    }

    pub(crate) fn profile_with_selected_dimensions(
        &self,
        mut profile: WeightedScaleProfileV1,
    ) -> std::io::Result<WeightedScaleProfileV1> {
        if self.claim_selection.is_some() {
            for class_index in 0..self.templates.len() {
                let mut sizes = [0u64; 3];
                for (index, template) in self.templates[class_index].iter().enumerate() {
                    // Ordinal zero has the maximum modulo-assigned Claim fanout
                    // for a Work. Dimensions are conservative selected shape
                    // bounds; actual member/byte census remains measured.
                    let value = if template.class == WeightedScaleClassV1::Artifact {
                        if let Some(recipe) = self.artifact_recipe.as_ref() {
                            let ordinal =
                                SCALE_SELECTED_QUANTILE_BUCKETS_V1.iter().take(index).sum();
                            recipe.artifact_value(
                                profile.seed,
                                recipe.selection.immutable_digest()?,
                                ordinal,
                                &template.bytes,
                            )?
                        } else {
                            fixture_record_value_v1(&profile, template, template.class, 0)?
                        }
                    } else {
                        fixture_record_value_v1(&profile, template, template.class, 0)?
                    };
                    sizes[index] =
                        u64::try_from(fixture_record_encoded_length_v1(&value, template.class)?)
                            .map_err(|_| io_invalid("selected dimension exceeds u64"))?;
                }
                let row = &mut profile.classes[class_index];
                [row.p50_bytes, row.p95_bytes, row.max_bytes] = sizes;
            }
        }
        profile.validate()?;
        Ok(profile)
    }

    /// Reuse the reader-issued immutable route; no second profile parse or
    /// caller-authored tuple can substitute for that issued declaration.
    pub(crate) fn load_from_selected_declaration_accounted(
        repository_root: &Path,
        declaration: &super::source_admission_indexed_input::SelectedGeneratedDeclarationV1,
        io: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &AdmissionWorkBudget,
        max_state_bytes: usize,
        caller_live_state_bytes: usize,
    ) -> std::io::Result<Self> {
        let profile = WeightedScaleProfileV1::weighted_for_records(
            declaration.seed_sha256(),
            declaration.generated_record_count(),
        )?;
        match declaration.claim_template_selection() {
            None => Self::load_accounted(repository_root, io, deadline, cancelled, work),
            Some(issued) => {
                if issued.route() != WeightedScaleClaimTemplateRouteV1::WorkAuthorshipFixtureV1
                    || issued.relation_type_id() != "tos.relation.authored-by"
                    || issued.subject_class() != "Work"
                {
                    return Err(io_invalid("issued Claim selection route differs"));
                }
                let input_state = issued
                    .templates()
                    .iter()
                    .try_fold(
                        size_of::<WeightedScaleClaimTemplateSelectionV1>()
                            .checked_add(issued.generator_agent().source_path().len())
                            .and_then(|n| n.checked_add(issued.generator_agent().record_id().len()))
                            .ok_or_else(|| io_invalid("issued generator Agent state overflow"))?,
                        |n, row| n.checked_add(row.source_path().len()),
                    )
                    .ok_or_else(|| io_invalid("issued Claim input state overflow"))?;
                if input_state
                    .checked_add(caller_live_state_bytes)
                    .is_none_or(|n| n > max_state_bytes)
                {
                    return Err(io_invalid("issued Claim selection exceeds caller state"));
                }
                let selection = WeightedScaleClaimTemplateSelectionV1 {
                    relation_registry_sha256: issued.relation_registry_sha256(),
                    generator_agent: WeightedScaleGeneratorAgentSelectionV1 {
                        source_path: issued.generator_agent().source_path().to_owned(),
                        source_sha256: issued.generator_agent().source_sha256(),
                        record_id: issued.generator_agent().record_id().to_owned(),
                    },
                    templates: std::array::from_fn(|index| {
                        let row = &issued.templates()[index];
                        WeightedScaleClaimTemplateRowV1 {
                            source_path: row.source_path().to_owned(),
                            source_sha256: row.source_sha256(),
                            source_line: row.source_line(),
                            template_sha256: row.template_sha256(),
                            template_bytes: row.template_bytes(),
                        }
                    }),
                };
                let templates = if let Some(artifact) = declaration.artifact_template_selection() {
                    let artifact = WeightedScaleArtifactTemplateSelectionV1::from_issued(
                        artifact,
                        max_state_bytes,
                        caller_live_state_bytes
                            .checked_add(selection.retained_state_bytes()?)
                            .ok_or_else(|| io_invalid("issued ALL5 selection state overflow"))?,
                    )?;
                    Self::load_with_all5_selection_accounted(
                        repository_root,
                        &profile,
                        selection,
                        artifact,
                        io,
                        deadline,
                        cancelled,
                        work,
                        max_state_bytes,
                        caller_live_state_bytes,
                    )?
                } else {
                    Self::load_with_claim_selection_accounted(
                        repository_root,
                        &profile,
                        selection,
                        io,
                        deadline,
                        cancelled,
                        work,
                        max_state_bytes,
                        caller_live_state_bytes,
                        false,
                    )?
                };
                let mut templates = templates;
                if let Some(issued) = declaration.artifact_template_selection() {
                    let observed = issued.output_observation();
                    templates.output_observation = Some(WeightedScaleArtifactOutputObservationV1 {
                        started_at: observed.started_at().to_owned(),
                        ended_at: observed.ended_at().to_owned(),
                        member_count: observed.member_count(),
                        source_bytes: observed.source_bytes(),
                        ordered_output_sha256: observed.ordered_output_sha256(),
                    });
                }
                let selected_profile = templates.profile_with_selected_dimensions(profile)?;
                let row = selected_profile.classes[WeightedScaleClassV1::Claim as usize];
                if [row.p50_bytes, row.p95_bytes, row.max_bytes] != issued.claim_dimensions()
                    || selected_profile
                        .classes
                        .map(|row| [row.p50_bytes, row.p95_bytes, row.max_bytes])
                        != declaration.class_dimensions()
                    || templates.manifest_sha256 != declaration.template_manifest_sha256()
                {
                    return Err(io_invalid(
                        "issued Claim rendered dimensions or template manifest differs",
                    ));
                }
                Ok(templates)
            }
        }
    }

    fn push_template(
        templates: &mut [Vec<WeightedScaleTemplateV1>; 5],
        resident: &mut u64,
        pin: ScaleTemplatePinV1,
        file_bytes: Vec<u8>,
        bounded: Option<(usize, Instant, &AtomicBool)>,
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
        let (template_value, decoded_state): (serde_json::Value, usize) =
            if let Some((limit, deadline, cancelled)) = bounded {
                tos_validation::record_biblio_cut::bounded_decoded_state(
                    &template_bytes,
                    JsonLimits::default(),
                    limit
                        .checked_sub(template_bytes.capacity())
                        .ok_or_else(|| io_invalid("template raw state exceeds bound"))?,
                    deadline,
                    cancelled,
                )
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("template bounded decoding: {e:?}"),
                    )
                })?
            } else {
                (
                    serde_json::from_slice(&template_bytes)
                        .map_err(|_| io_invalid("pinned scale template JSON is invalid"))?,
                    0,
                )
            };
        let (_, _, edge_upper) = reference_collection_state_upper_v1(&template_value)?;
        // One selected class vector has a known finite three-template capacity.
        // Precharge its initial allocation before push, keeping legacy growth unchanged.
        let headers = if templates[pin.class_index].capacity() == 0 {
            3usize
                .checked_mul(size_of::<WeightedScaleTemplateV1>())
                .ok_or_else(|| io_invalid("template headers overflow"))?
        } else {
            0
        };
        if let Some((limit, _, _)) = bounded {
            if template_bytes
                .capacity()
                .checked_add(decoded_state)
                .and_then(|n| n.checked_add(edge_upper))
                .and_then(|n| n.checked_add(headers))
                .is_none_or(|n| n > limit)
            {
                return Err(io_invalid("template references exceed loading state"));
            }
            if templates[pin.class_index].capacity() == 0 {
                templates[pin.class_index].reserve_exact(3);
            }
        }
        let external_reference_edges = collect_reference_edges_v1(&template_value)?;
        *resident = resident
            .checked_add(template_bytes.len() as u64)
            .ok_or_else(|| io_invalid("pinned template bytes overflow"))?;
        templates[pin.class_index].push(WeightedScaleTemplateV1 {
            class: pin.class,
            source_path: std::borrow::Cow::Borrowed(pin.path),
            source_sha256,
            template_sha256,
            bytes: template_bytes,
            external_reference_edges,
            claim_route: WeightedScaleClaimTemplateRouteV1::LegacyTechnicalV1,
            generated_author: None,
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
        let value = selected_fixture_record_value_v2(
            self.profile,
            self.templates,
            class_row.class,
            ordinal,
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
        let mut source_bytes = Vec::new();
        write_fixture_record_value_v1(&mut source_bytes, &value, class_row.class)?;
        let selected_template_bytes =
            match selected_quantile_index_v1(ordinal % SCALE_SELECTED_QUANTILE_PERIOD_V1) {
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

// One encoding law serves produced bytes, exact CompareWriter verification,
// and selected dimensions. Prices never substitute compact JSON for emitted
// pretty JSON, nor retain a second encoded payload Vec.
fn write_fixture_record_value_v1(
    writer: &mut impl IoWrite,
    value: &serde_json::Value,
    class: WeightedScaleClassV1,
) -> std::io::Result<()> {
    if class == WeightedScaleClassV1::Claim {
        serde_json::to_writer(&mut *writer, value)
    } else {
        serde_json::to_writer_pretty(&mut *writer, value)
    }
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writer.write_all(b"\n")
}
fn fixture_record_encoded_length_v1(
    value: &serde_json::Value,
    class: WeightedScaleClassV1,
) -> std::io::Result<usize> {
    struct Counter(usize);
    impl IoWrite for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len())
                .ok_or_else(|| io_invalid("fixture encoded length overflow"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    write_fixture_record_value_v1(&mut counter, value, class)?;
    Ok(counter.0)
}

fn fixture_record_value_v1(
    profile: &WeightedScaleProfileV1,
    template: &WeightedScaleTemplateV1,
    class: WeightedScaleClassV1,
    ordinal: u64,
) -> std::io::Result<serde_json::Value> {
    let mut value = serde_json::from_slice(&template.bytes)
        .map_err(|_| io_invalid("pinned scale template JSON is invalid"))?;
    rewrite_fixture_record_v1(
        &mut value,
        class,
        ordinal,
        profile.seed,
        profile.classes[4].count,
        profile.classes[WeightedScaleClassV1::Claim as usize].count,
        template.claim_route == WeightedScaleClaimTemplateRouteV1::WorkAuthorshipFixtureV1,
    )?;
    if class == WeightedScaleClassV1::Claim
        && template.claim_route == WeightedScaleClaimTemplateRouteV1::WorkAuthorshipFixtureV1
    {
        let author = template
            .generated_author
            .as_deref()
            .ok_or_else(|| io_invalid("selected generated author absent"))?;
        value
            .as_object_mut()
            .ok_or_else(|| io_invalid("selected Claim object absent"))?
            .insert(
                "object".to_owned(),
                serde_json::Value::String(author.to_owned()),
            );
        value.as_object_mut().ok_or_else(|| io_invalid("selected Claim object absent"))?.insert("qualifiers".to_owned(), serde_json::json!({
                    "statement": "This synthetic Work fixture is generated by the declared provisional software responsibility bearer; it is not attributed to the original source author.",
                    "statement_language": "en", "statement_script": "Latn",
                    "scope_note": "The original pinned Claim is a structural template only. Its evidence and provenance event identify template origin; the declared generator source assessment qualifies only synthetic mechanical responsibility, not human or legal authorship. No human review was performed.",
                    "template_sha256": template.template_sha256.to_hex(),
                }));
    }

    Ok(value)
}

fn selected_fixture_record_value_v2(
    profile: &WeightedScaleProfileV1,
    templates: &WeightedScaleTemplateSetV1,
    class: WeightedScaleClassV1,
    ordinal: u64,
) -> std::io::Result<serde_json::Value> {
    let template = templates.select(class, ordinal);
    if class == WeightedScaleClassV1::Artifact {
        if let Some(recipe) = templates.artifact_recipe.as_ref() {
            return recipe.artifact_value(
                profile.seed,
                recipe.selection.immutable_digest()?,
                ordinal,
                &template.bytes,
            );
        }
    }
    fixture_record_value_v1(profile, template, class, ordinal)
}

/// Shared support encoder: pricing, actual packed payloads and live proof use
/// exactly this owner path. No completed phase is invented by a point lookup.
fn write_fixture_support_v2(
    writer: &mut impl IoWrite,
    profile: &WeightedScaleProfileV1,
    templates: &WeightedScaleTemplateSetV1,
    role: WeightedScaleArtifactSupportRoleV1,
    ordinal: u64,
    observation: Option<&WeightedScaleArtifactOutputObservationV1>,
) -> std::io::Result<()> {
    let recipe = templates
        .artifact_recipe
        .as_ref()
        .ok_or_else(|| io_invalid("generated support has no issued Artifact recipe"))?;
    let digest = recipe.selection.immutable_digest()?;
    let index = selected_quantile_index_v1(ordinal % SCALE_SELECTED_QUANTILE_PERIOD_V1);
    if role == WeightedScaleArtifactSupportRoleV1::Resource {
        let identity = scale_identity(
            artifact_recipe_seed_v1(profile.seed, digest),
            WeightedScaleClassV1::Artifact,
            ordinal,
        );
        return write_artifact_resource_v1(
            writer,
            profile.seed,
            digest,
            ordinal,
            &identity,
            recipe.selection.resource_templates[index].source_sha256,
            &recipe.resources[index],
        );
    }
    let value = match role {
        WeightedScaleArtifactSupportRoleV1::Rights => {
            recipe.rights_value(profile.seed, digest, ordinal)?
        }
        WeightedScaleArtifactSupportRoleV1::Discovery => {
            recipe.discovery_value(profile.seed, digest, ordinal)?
        }
        WeightedScaleArtifactSupportRoleV1::Event => recipe.event_value(
            profile.seed,
            digest,
            ordinal,
            observation.ok_or_else(|| io_invalid("event needs completed output observation"))?,
            &templates
                .claim_selection
                .as_ref()
                .ok_or_else(|| io_invalid("event generator selection absent"))?
                .generator_agent,
            &templates
                .select(WeightedScaleClassV1::Artifact, ordinal)
                .bytes,
        )?,
        WeightedScaleArtifactSupportRoleV1::Resource => unreachable!(),
    };
    if role == WeightedScaleArtifactSupportRoleV1::Event {
        serde_json::to_writer(&mut *writer, &value)
    } else {
        serde_json::to_writer_pretty(&mut *writer, &value)
    }
    .map_err(|_| io_invalid("generated support encoding failed"))?;
    writer.write_all(b"\n")
}

/// Exact workload-owned generated member identity. Canonical record/slot
/// bindings and semantic acceptance remain with their existing owner.
pub(crate) struct WeightedScaleGeneratedMemberV1 {
    class: WeightedScaleClassV1,
    ordinal: u64,
    identity: String,
    template_sha256: Digest256,
    raw_sha256: Digest256,
    raw_bytes: u64,
}

impl WeightedScaleGeneratedMemberV1 {
    pub(crate) fn class(&self) -> WeightedScaleClassV1 {
        self.class
    }
    pub(crate) fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub(crate) fn identity(&self) -> &str {
        &self.identity
    }
    pub(crate) fn template_sha256(&self) -> Digest256 {
        self.template_sha256
    }
    pub(crate) fn raw_sha256(&self) -> Digest256 {
        self.raw_sha256
    }
    pub(crate) fn raw_bytes(&self) -> u64 {
        self.raw_bytes
    }
}

/// Generated-ALL is issued from the authenticated input reader declaration,
/// never a prefix wildcard or a manifest containing every generated ID.
pub(crate) struct WeightedScaleGeneratedAllV1 {
    profile: WeightedScaleProfileV1,
    templates: WeightedScaleTemplateSetV1,
    auxiliary: Arc<tos_validation::source_record_selection::SourceRecordSelection>,
    auxiliary_source_bytes: u64,
    fence: super::source_admission_spooled_candidate::CandidateFence,
    selection_sha256: Digest256,
    artifact_recipe_digest: Option<Digest256>,
    retained_state_bytes: usize,
    max_owned_state_bytes: usize,
    work: AdmissionWorkBudget,
}

impl WeightedScaleGeneratedAllV1 {
    pub(crate) fn from_selected_declaration(
        declaration: &super::source_admission_indexed_input::SelectedGeneratedDeclarationV1,
        mut templates: WeightedScaleTemplateSetV1,
        auxiliary: Arc<tos_validation::source_record_selection::SourceRecordSelection>,
        fence: super::source_admission_spooled_candidate::CandidateFence,
        max_owned_state_bytes: usize,
        caller_live_state_bytes: usize,
        work: AdmissionWorkBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<Self> {
        // The set's payload arrays are owner-private; recompute retained byte
        // observations rather than trusting mutable public summary counters.
        let mut template_bytes = 0u64;
        let mut reference_bytes = 0u64;
        for rows in &templates.templates {
            for template in rows {
                template_bytes = template_bytes
                    .checked_add(template.bytes.len() as u64)
                    .ok_or_else(|| io_invalid("generated template state overflow"))?;
                for edge in &template.external_reference_edges {
                    reference_bytes = reference_bytes
                        .checked_add(edge.pointer.len() as u64)
                        .and_then(|bytes| bytes.checked_add(edge.reference.len() as u64))
                        .ok_or_else(|| io_invalid("generated template reference state overflow"))?;
                }
            }
        }
        templates.resident_template_bytes = template_bytes;
        templates.resident_reference_state_bytes = reference_bytes;
        let profile = templates.profile_with_selected_dimensions(
            WeightedScaleProfileV1::weighted_for_records(
                declaration.seed_sha256(),
                declaration.generated_record_count(),
            )?,
        )?;
        match (
            declaration.claim_template_selection(),
            templates.claim_selection.as_ref(),
        ) {
            (None, None) => {}
            (Some(issued), Some(selected)) => {
                if issued.route() != WeightedScaleClaimTemplateRouteV1::WorkAuthorshipFixtureV1
                    || issued.relation_registry_sha256() != selected.relation_registry_sha256
                    || issued.relation_type_id() != "tos.relation.authored-by"
                    || issued.subject_class() != "Work"
                    || issued.generator_agent().source_path()
                        != selected.generator_agent.source_path
                    || issued.generator_agent().source_sha256()
                        != selected.generator_agent.source_sha256
                    || issued.generator_agent().record_id() != selected.generator_agent.record_id
                    || declaration.class_dimensions()
                        != profile
                            .classes
                            .map(|row| [row.p50_bytes, row.p95_bytes, row.max_bytes])
                    || issued.claim_dimensions() != {
                        let c = profile.classes[WeightedScaleClassV1::Claim as usize];
                        [c.p50_bytes, c.p95_bytes, c.max_bytes]
                    }
                    || !issued
                        .templates()
                        .iter()
                        .zip(&selected.templates)
                        .all(|(a, b)| {
                            a.source_path() == b.source_path
                                && a.source_sha256() == b.source_sha256
                                && a.source_line() == b.source_line
                                && a.template_sha256() == b.template_sha256
                                && a.template_bytes() == b.template_bytes
                        })
                {
                    return Err(io_invalid(
                        "issued selected Claim route or provenance differs",
                    ));
                }
            }
            _ => return Err(io_invalid("issued selected Claim route custody differs")),
        }
        if declaration.class_dimensions()
            != profile
                .classes
                .map(|row| [row.p50_bytes, row.p95_bytes, row.max_bytes])
        {
            return Err(io_invalid(
                "issued class dimensions differ from same maintained renderer",
            ));
        }
        let composition = declaration.composition();
        verify_composition_against_source_record_selection_v1(
            &composition,
            &auxiliary,
            &mut || {
                scale_active(deadline, cancelled)?;
                work.charge_many(1)
            },
        )?;
        if let Some(selected) = templates.claim_selection.as_ref() {
            let selected_agent = auxiliary
                .record(&selected.generator_agent.source_path)
                .ok_or_else(|| {
                    io_invalid("generator Agent absent from finite semantic record selection")
                })?;
            if selected_agent.record_id != selected.generator_agent.record_id
                || Digest256::from_hex(&selected_agent.source.raw_sha256)
                    .map_err(|_| io_invalid("finite generator Agent record digest invalid"))?
                    != selected.generator_agent.source_sha256
            {
                return Err(io_invalid(
                    "finite generator Agent semantic binding differs",
                ));
            }
            let mut agent_member_found = false;
            for member in auxiliary.members() {
                scale_active(deadline, cancelled)?;
                work.charge_many(1)?;
                if member.source_ref == selected.generator_agent.source_path {
                    let digest = Digest256::from_hex(&member.raw_sha256)
                        .map_err(|_| io_invalid("finite generator Agent digest invalid"))?;
                    if digest != selected.generator_agent.source_sha256 {
                        return Err(io_invalid("finite generator Agent source digest differs"));
                    }
                    agent_member_found = true;
                    break;
                }
            }
            if !agent_member_found {
                return Err(io_invalid(
                    "generator Agent absent from same finite authored closure",
                ));
            }
        }
        let auxiliary_source_bytes = composition.auxiliary_source_bytes;
        let support_count = if templates.artifact_recipe.is_some() {
            artifact_support_member_count_v1(
                profile.classes[WeightedScaleClassV1::Artifact as usize].count,
            )?
        } else {
            0
        };
        let total = profile
            .target_records
            .checked_add(support_count)
            .and_then(|n| n.checked_add(auxiliary.member_count() as u64))
            .ok_or_else(|| io_invalid("generated selection member count overflow"))?;
        if profile.classes.map(|row| row.count) != declaration.class_counts()
            || templates.manifest_sha256 != declaration.template_manifest_sha256()
            || template_manifest_digest_v1(&templates.templates)? != templates.manifest_sha256
            || generated_declaration_digest_selected_v2(&profile, &templates)?
                != declaration.generated_declaration_sha256()
            || composition.generated_declaration_sha256
                != declaration.generated_declaration_sha256()
            || composition.generated_record_count != profile.target_records
            || composition.generated_support_member_count != support_count
            || composition.authored_manifest_sha256 != auxiliary.digest()
            || composition.auxiliary_member_count != auxiliary.member_count() as u64
            || composition.auxiliary_source_bytes != auxiliary_source_bytes
            || fence.membership.count != total
            || fence.source_bytes < auxiliary_source_bytes
            || max_owned_state_bytes == 0
            || max_owned_state_bytes == usize::MAX
        {
            return Err(io_invalid(
                "generated selection differs from issued declaration",
            ));
        }
        let maximum_path = auxiliary
            .members()
            .map(|member| member.source_ref.len())
            .max()
            .unwrap_or(0)
            .max(max_member_path_bytes_v1(&profile))
            .max(if support_count > 0 {
                WeightedScaleArtifactSupportRoleV1::ALL
                    .iter()
                    .map(|role| role.path(0).len())
                    .max()
                    .unwrap_or(0)
            } else {
                0
            });
        let retained_state_bytes = weighted_producer_state_upper_v1(&templates, 0)?
            // The same finite model is already charged in the Native caller
            // baseline. Only this provider's own header/Arc handle is added.
            .checked_add(size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(size_of::<WeightedScaleGeneratedTraversalV1>()))
            .and_then(|bytes| bytes.checked_add(maximum_path.checked_mul(2)?))
            .ok_or_else(|| io_invalid("generated selection retained state overflow"))?;
        if retained_state_bytes
            .checked_add(caller_live_state_bytes)
            .is_none_or(|bytes| bytes >= max_owned_state_bytes)
        {
            return Err(io_invalid("generated selection exceeds selected state"));
        }
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-scale-generated-all-selection-v1\0");
        hash.update(declaration.profile_sha256().as_bytes());
        hash.update(declaration.template_manifest_sha256().as_bytes());
        hash.update(declaration.generated_declaration_sha256().as_bytes());
        hash.update(composition.authored_manifest_sha256.as_bytes());
        hash.update(composition.auxiliary_members_sha256.as_bytes());
        hash.update(composition.members_descriptor_sha256.as_bytes());
        hash.update(fence.batch_sha256.as_bytes());
        hash.update(fence.validator_sha256.as_bytes());
        hash.update(fence.membership.digest.as_bytes());
        hash.update(&fence.membership.count.to_be_bytes());
        hash.update(&fence.source_bytes.to_be_bytes());
        hash.update(&fence.retirement_count.to_be_bytes());
        hash.update(fence.retirement_digest.as_bytes());
        if let Some(revision) = fence.base_revision {
            hash.update(&[1]);
            hash.update(revision.0.as_bytes());
        } else {
            hash.update(&[0]);
        }
        let artifact_recipe_digest = templates
            .artifact_recipe
            .as_ref()
            .map(|r| r.selection.immutable_digest())
            .transpose()?;
        Ok(Self {
            profile,
            templates,
            auxiliary,
            auxiliary_source_bytes,
            fence,
            artifact_recipe_digest,
            selection_sha256: hash.finalize(),
            retained_state_bytes,
            max_owned_state_bytes,
            work,
        })
    }

    pub(crate) fn verify_candidate_fence(
        &self,
        actual_fence: super::source_admission_spooled_candidate::CandidateFence,
    ) -> std::io::Result<()> {
        if actual_fence != self.fence {
            return Err(io_invalid("generated provider CandidateFence differs"));
        }
        Ok(())
    }

    pub(crate) fn selection_sha256(&self) -> Digest256 {
        self.selection_sha256
    }
    pub(crate) fn declared_generated_count(&self) -> u64 {
        self.profile.target_records
    }
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.retained_state_bytes
    }

    fn support_position(
        &self,
        path: &str,
    ) -> std::io::Result<Option<(WeightedScaleArtifactSupportRoleV1, u64)>> {
        if self.artifact_recipe_digest.is_none() {
            return Ok(None);
        }
        for role in WeightedScaleArtifactSupportRoleV1::ALL {
            let sample = role.path(0);
            let (prefix, suffix) = sample
                .split_once("00000000000000000000")
                .ok_or_else(|| io_invalid("support path contract differs"))?;
            let Some(tail) = path.strip_prefix(prefix) else {
                continue;
            };
            let Some(digits) = tail.strip_suffix(suffix) else {
                continue;
            };
            if digits.len() != 20 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(io_invalid("generated support ordinal shape differs"));
            }
            let ordinal = digits
                .parse::<u64>()
                .map_err(|_| io_invalid("generated support ordinal overflow"))?;
            if ordinal >= self.profile.classes[WeightedScaleClassV1::Artifact as usize].count
                || role.path(ordinal) != path
            {
                return Err(io_invalid("generated support ordinal exceeds selection"));
            }
            return Ok(Some((role, ordinal)));
        }
        Ok(None)
    }
    pub(crate) fn selects_required_member(&self, path: &str) -> std::io::Result<bool> {
        Ok(self.support_position(path)?.is_some() || self.generated_position(path)?.is_some())
    }
    pub(crate) fn selects_semantic_member(&self, path: &str) -> std::io::Result<bool> {
        if let Some((role, _)) = self.support_position(path)? {
            return Ok(role.is_semantic());
        }
        self.selects_record(path)
    }
    pub(crate) fn selects_semantic_row(&self, path: &str, line: u64) -> std::io::Result<bool> {
        Ok(line == 1
            && (self.selects_claim_row(path, line)?
                || self
                    .support_position(path)?
                    .is_some_and(|(role, _)| role == WeightedScaleArtifactSupportRoleV1::Event)))
    }
    pub(crate) fn verify_selected_member(
        &self,
        path: &str,
        raw: &[u8],
        actual_fence: super::source_admission_spooled_candidate::CandidateFence,
        caller_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<()> {
        let Some((role, ordinal)) = self.support_position(path)? else {
            return self
                .verify_record(
                    path,
                    raw,
                    actual_fence,
                    caller_live_state_bytes,
                    deadline,
                    cancelled,
                )
                .map(|_| ());
        };
        scale_active(deadline, cancelled)?;
        self.work.charge_many(1)?;
        if actual_fence != self.fence
            || raw.is_empty()
            || self
                .retained_state_bytes
                .checked_add(raw.len())
                .and_then(|n| n.checked_add(caller_live_state_bytes))
                .is_none_or(|n| n > self.max_owned_state_bytes)
        {
            return Err(io_invalid("generated support fence or state differs"));
        }
        let mut compare = GeneratedCompareWriterV1 {
            raw,
            offset: 0,
            deadline,
            cancelled,
        };
        write_fixture_support_v2(
            &mut compare,
            &self.profile,
            &self.templates,
            role,
            ordinal,
            self.templates.output_observation.as_ref(),
        )?;
        if compare.offset != raw.len() {
            return Err(io_invalid("generated support renderer EOF differs"));
        }
        Ok(())
    }

    fn generated_position(
        &self,
        path: &str,
    ) -> std::io::Result<Option<(WeightedScaleClassV1, u64)>> {
        if self.support_position(path)?.is_some() {
            return Ok(None);
        }
        for row in &self.profile.classes {
            let Some(tail) = path.strip_prefix(generated_path_prefix_v1(row.class)) else {
                continue;
            };
            let Some((digits, suffix)) = tail.split_once('/') else {
                return Err(io_invalid("generated member path shape differs"));
            };
            if digits.len() != 20
                || !digits.bytes().all(|byte| byte.is_ascii_digit())
                || suffix != row.class.suffix()
            {
                return Err(io_invalid("generated member path shape differs"));
            }
            let ordinal = digits
                .parse::<u64>()
                .map_err(|_| io_invalid("generated ordinal overflow"))?;
            if ordinal >= row.count || path_for(row.class, ordinal) != path {
                return Err(io_invalid("generated ordinal exceeds declared class"));
            }
            return Ok(Some((row.class, ordinal)));
        }
        Ok(None)
    }

    /// Scope selection is exact class/ordinal/path geometry; raw verification
    /// remains mandatory before any selected record is returned to a consumer.
    pub(crate) fn selects_record(&self, path: &str) -> std::io::Result<bool> {
        Ok(self.generated_position(path)?.is_some())
    }

    pub(crate) fn selects_claim_row(&self, path: &str, line: u64) -> std::io::Result<bool> {
        Ok(line == 1
            && self
                .generated_position(path)?
                .is_some_and(|(class, _)| class == WeightedScaleClassV1::Claim))
    }

    pub(crate) fn verify_record(
        &self,
        path: &str,
        raw: &[u8],
        actual_fence: super::source_admission_spooled_candidate::CandidateFence,
        caller_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<WeightedScaleGeneratedMemberV1> {
        scale_active(deadline, cancelled)?;
        self.work.charge_many(1)?;
        if actual_fence != self.fence {
            return Err(io_invalid("generated selection CandidateFence differs"));
        }
        let (class, ordinal) = self
            .generated_position(path)?
            .ok_or_else(|| io_invalid("record outside declared generated selection"))?;
        let row = self.profile.classes[class as usize];
        let selected_bytes =
            match selected_quantile_index_v1(ordinal % SCALE_SELECTED_QUANTILE_PERIOD_V1) {
                0 => row.p50_bytes,
                1 => row.p95_bytes,
                _ => row.max_bytes,
            }
            .checked_add(SCALE_IDENTITY_GROWTH_ALLOWANCE_V1)
            .ok_or_else(|| io_invalid("generated verification byte bound overflow"))?;
        if raw.is_empty()
            || raw.len() > SCALE_MAX_TEMPLATE_BYTES_V1
            || raw.len() as u64 > selected_bytes
            || self
                .retained_state_bytes
                .checked_add(raw.len())
                .and_then(|bytes| bytes.checked_add(caller_live_state_bytes))
                .is_none_or(|bytes| bytes > self.max_owned_state_bytes)
        {
            return Err(io_invalid(
                "generated verification exceeds selected state or member profile",
            ));
        }
        let template = self.templates.select(class, ordinal);
        let value =
            selected_fixture_record_value_v2(&self.profile, &self.templates, class, ordinal)?;
        let mut compare = GeneratedCompareWriterV1 {
            raw,
            offset: 0,
            deadline,
            cancelled,
        };
        write_fixture_record_value_v1(&mut compare, &value, class)
            .map_err(|_| io_invalid("generated member differs from maintained renderer"))?;
        if compare.offset != raw.len() {
            return Err(io_invalid("generated member renderer EOF differs"));
        }
        scale_active(deadline, cancelled)?;
        Ok(WeightedScaleGeneratedMemberV1 {
            class,
            ordinal,
            identity: if class == WeightedScaleClassV1::Artifact {
                self.templates
                    .artifact_recipe
                    .as_ref()
                    .map(|r| r.selection.immutable_digest())
                    .transpose()?
                    .map_or_else(
                        || scale_identity(self.profile.seed, class, ordinal),
                        |recipe| {
                            scale_identity(
                                artifact_recipe_seed_v1(self.profile.seed, recipe),
                                class,
                                ordinal,
                            )
                        },
                    )
            } else {
                scale_identity(self.profile.seed, class, ordinal)
            },
            template_sha256: template.template_sha256,
            raw_sha256: Digest256::of_bytes(raw),
            raw_bytes: raw.len() as u64,
        })
    }

    pub(crate) fn begin_traversal(&self) -> WeightedScaleGeneratedTraversalV1 {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-scale-generated-all-observed-members-v1\0");
        hash.update(self.selection_sha256.as_bytes());
        WeightedScaleGeneratedTraversalV1 {
            selection_sha256: self.selection_sha256,
            class_ordinals: [0; 5],
            support_ordinals: [0; 4],
            support_bytes: 0,
            phase_count: 0,
            phase_bytes: 0,
            phase_hash: self.artifact_recipe_digest.map(|recipe| {
                let mut h = Digest256Hasher::new();
                h.update(b"tos_scale_artifact_completed_output_phase_v1\0");
                h.update(recipe.as_bytes());
                h.update(self.profile.seed.as_bytes());
                h
            }),
            auxiliary_index: 0,
            generated_bytes: 0,
            auxiliary_bytes: 0,
            observed_members: 0,
            previous_path: String::new(),
            hash,
            failed: false,
        }
    }

    /// Observe the complete real physical traversal, including only exact
    /// authenticated finite auxiliary members. There is no unnamed skip branch.
    pub(crate) fn observe_member(
        &self,
        traversal: &mut WeightedScaleGeneratedTraversalV1,
        path: &str,
        raw: &[u8],
        actual_fence: super::source_admission_spooled_candidate::CandidateFence,
        caller_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<Option<WeightedScaleGeneratedMemberV1>> {
        if traversal.failed {
            return Err(io_invalid("generated traversal already refused"));
        }
        let result = self.observe_inner(
            traversal,
            path,
            raw,
            actual_fence,
            caller_live_state_bytes,
            deadline,
            cancelled,
        );
        if result.is_err() {
            traversal.failed = true;
        }
        result
    }

    fn observe_inner(
        &self,
        traversal: &mut WeightedScaleGeneratedTraversalV1,
        path: &str,
        raw: &[u8],
        actual_fence: super::source_admission_spooled_candidate::CandidateFence,
        caller_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<Option<WeightedScaleGeneratedMemberV1>> {
        scale_active(deadline, cancelled)?;
        if traversal.selection_sha256 != self.selection_sha256
            || actual_fence != self.fence
            || self
                .retained_state_bytes
                .checked_add(raw.len())
                .and_then(|bytes| bytes.checked_add(caller_live_state_bytes))
                .is_none_or(|bytes| bytes > self.max_owned_state_bytes)
            || (!traversal.previous_path.is_empty() && path <= traversal.previous_path.as_str())
        {
            return Err(io_invalid(
                "generated traversal identity or physical path order differs",
            ));
        }
        let (binding, class_tag, ordinal, digest, size) = if let Some((class, ordinal)) =
            self.generated_position(path)?
        {
            if ordinal != traversal.class_ordinals[class as usize] {
                return Err(io_invalid(
                    "generated traversal class ordinal gap or duplicate",
                ));
            }
            let binding = self.verify_record(
                path,
                raw,
                actual_fence,
                caller_live_state_bytes,
                deadline,
                cancelled,
            )?;
            traversal.class_ordinals[class as usize] = ordinal
                .checked_add(1)
                .ok_or_else(|| io_invalid("generated observed ordinal overflow"))?;
            traversal.generated_bytes = traversal
                .generated_bytes
                .checked_add(binding.raw_bytes)
                .ok_or_else(|| io_invalid("generated observed bytes overflow"))?;
            let digest = binding.raw_sha256;
            let size = binding.raw_bytes;
            (Some(binding), class as u8, ordinal, digest, size)
        } else if let Some((role, ordinal)) = self.support_position(path)? {
            let index = WeightedScaleArtifactSupportRoleV1::ALL
                .iter()
                .position(|r| *r == role)
                .ok_or_else(|| io_invalid("support role absent"))?;
            if traversal.support_ordinals[index] != ordinal {
                return Err(io_invalid("support ordinal gap or duplicate"));
            }
            self.verify_selected_member(
                path,
                raw,
                actual_fence,
                caller_live_state_bytes,
                deadline,
                cancelled,
            )?;
            traversal.support_ordinals[index] = ordinal
                .checked_add(1)
                .ok_or_else(|| io_invalid("support ordinal overflow"))?;
            traversal.support_bytes = traversal
                .support_bytes
                .checked_add(raw.len() as u64)
                .ok_or_else(|| io_invalid("support byte census overflow"))?;
            (
                None,
                6 + index as u8,
                ordinal,
                Digest256::of_bytes(raw),
                raw.len() as u64,
            )
        } else {
            self.work.charge_many(1)?;
            let aux = self
                .auxiliary
                .members()
                .nth(traversal.auxiliary_index)
                .ok_or_else(|| io_invalid("physical member outside finite auxiliary selection"))?;
            let aux_digest = Digest256::from_hex(&aux.raw_sha256)
                .map_err(|_| io_invalid("finite auxiliary raw digest differs"))?;
            if aux.source_ref != path
                || aux.raw_bytes != raw.len() as u64
                || aux_digest != Digest256::of_bytes(raw)
            {
                return Err(io_invalid(
                    "physical member differs from finite auxiliary proof",
                ));
            }
            traversal.auxiliary_index += 1;
            traversal.auxiliary_bytes = traversal
                .auxiliary_bytes
                .checked_add(aux.raw_bytes)
                .ok_or_else(|| io_invalid("auxiliary observed bytes overflow"))?;
            (
                None,
                WeightedScaleClassV1::ALL.len() as u8,
                traversal.auxiliary_index as u64 - 1,
                aux_digest,
                aux.raw_bytes,
            )
        };
        if self
            .support_position(path)?
            .is_none_or(|(role, _)| role.is_phase_one())
        {
            if let Some(hash) = traversal.phase_hash.as_mut() {
                hash.update(&(path.len() as u64).to_be_bytes());
                hash.update(path.as_bytes());
                hash.update(digest.as_bytes());
                hash.update(&size.to_be_bytes());
                traversal.phase_count = traversal
                    .phase_count
                    .checked_add(1)
                    .ok_or_else(|| io_invalid("observed phase count overflow"))?;
                traversal.phase_bytes = traversal
                    .phase_bytes
                    .checked_add(size)
                    .ok_or_else(|| io_invalid("observed phase bytes overflow"))?;
            }
        }
        traversal
            .hash
            .update(&traversal.observed_members.to_be_bytes());
        traversal.hash.update(&[class_tag]);
        traversal.hash.update(&ordinal.to_be_bytes());
        traversal.hash.update(&(path.len() as u64).to_be_bytes());
        traversal.hash.update(path.as_bytes());
        traversal.hash.update(digest.as_bytes());
        traversal.hash.update(&size.to_be_bytes());
        traversal.observed_members = traversal
            .observed_members
            .checked_add(1)
            .ok_or_else(|| io_invalid("generated total observed count overflow"))?;
        traversal.previous_path.clear();
        traversal.previous_path.push_str(path);
        Ok(binding)
    }

    /// Called only after the actual input traversal returned EOF. Its digest
    /// proves workload/dependency byte coverage, never semantic acceptance.
    pub(crate) fn finish_traversal(
        &self,
        mut traversal: WeightedScaleGeneratedTraversalV1,
        actual_fence: super::source_admission_spooled_candidate::CandidateFence,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<WeightedScaleGeneratedEofV1> {
        scale_active(deadline, cancelled)?;
        self.work.charge_many(1)?;
        if traversal.failed
            || traversal.selection_sha256 != self.selection_sha256
            || actual_fence != self.fence
            || traversal.class_ordinals != self.profile.classes.map(|row| row.count)
            || traversal.support_ordinals
                != [if self.artifact_recipe_digest.is_some() {
                    self.profile.classes[WeightedScaleClassV1::Artifact as usize].count
                } else {
                    0
                }; 4]
            || traversal.auxiliary_index != self.auxiliary.member_count()
            || traversal.auxiliary_bytes != self.auxiliary_source_bytes
            || traversal.observed_members != self.fence.membership.count
            || traversal
                .generated_bytes
                .checked_add(traversal.support_bytes)
                .and_then(|n| n.checked_add(traversal.auxiliary_bytes))
                != Some(self.fence.source_bytes)
        {
            return Err(io_invalid(
                "generated or finite auxiliary actual EOF census differs",
            ));
        }
        match (
            traversal.phase_hash.take(),
            self.templates.output_observation.as_ref(),
        ) {
            (Some(hash), Some(expected)) => {
                if hash.finalize() != expected.ordered_output_sha256
                    || traversal.phase_count != expected.member_count
                    || traversal.phase_bytes != expected.source_bytes
                {
                    return Err(io_invalid(
                        "actual output phase differs from issued event observation",
                    ));
                }
            }
            (None, None) => {}
            _ => {
                return Err(io_invalid(
                    "actual output phase observation custody differs",
                ));
            }
        }
        if self.artifact_recipe_digest.is_some() {
            traversal.hash.update(b"artifact-support-eof-v2\0");
            for count in traversal.support_ordinals {
                traversal.hash.update(&count.to_be_bytes());
            }
            traversal
                .hash
                .update(&traversal.support_bytes.to_be_bytes());
        }
        traversal.hash.update(b"complete-physical-eof\0");
        for count in traversal.class_ordinals {
            traversal.hash.update(&count.to_be_bytes());
        }
        traversal
            .hash
            .update(&traversal.generated_bytes.to_be_bytes());
        traversal
            .hash
            .update(&traversal.auxiliary_bytes.to_be_bytes());
        traversal
            .hash
            .update(&traversal.observed_members.to_be_bytes());
        Ok(WeightedScaleGeneratedEofV1 {
            selection_sha256: self.selection_sha256,
            generated_count: self.profile.target_records,
            support_count: traversal.support_ordinals.iter().sum(),
            support_bytes: traversal.support_bytes,
            generated_bytes: traversal.generated_bytes,
            auxiliary_count: traversal.auxiliary_index as u64,
            auxiliary_bytes: traversal.auxiliary_bytes,
            observed_members: traversal.observed_members,
            ordered_sha256: traversal.hash.finalize(),
        })
    }
}

struct GeneratedCompareWriterV1<'a> {
    raw: &'a [u8],
    offset: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl IoWrite for GeneratedCompareWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        scale_active(self.deadline, self.cancelled)?;
        let end = self
            .offset
            .checked_add(bytes.len())
            .filter(|end| *end <= self.raw.len())
            .ok_or_else(|| io_invalid("generated renderer exceeds actual raw EOF"))?;
        if self.raw[self.offset..end] != *bytes {
            return Err(io_invalid(
                "generated raw bytes differ from maintained renderer",
            ));
        }
        self.offset = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) struct WeightedScaleGeneratedTraversalV1 {
    selection_sha256: Digest256,
    class_ordinals: [u64; 5],
    support_ordinals: [u64; 4],
    support_bytes: u64,
    phase_count: u64,
    phase_bytes: u64,
    phase_hash: Option<Digest256Hasher>,
    auxiliary_index: usize,
    generated_bytes: u64,
    auxiliary_bytes: u64,
    observed_members: u64,
    previous_path: String,
    hash: Digest256Hasher,
    failed: bool,
}
/// Private fields prevent a caller-provided scalar from standing for real EOF.
pub(crate) struct WeightedScaleGeneratedEofV1 {
    selection_sha256: Digest256,
    generated_count: u64,
    support_count: u64,
    support_bytes: u64,
    generated_bytes: u64,
    auxiliary_count: u64,
    auxiliary_bytes: u64,
    observed_members: u64,
    ordered_sha256: Digest256,
}
impl WeightedScaleGeneratedEofV1 {
    pub(crate) fn selection_sha256(&self) -> Digest256 {
        self.selection_sha256
    }
    pub(crate) fn support_count(&self) -> u64 {
        self.support_count
    }
    pub(crate) fn support_bytes(&self) -> u64 {
        self.support_bytes
    }
    pub(crate) fn generated_count(&self) -> u64 {
        self.generated_count
    }
    pub(crate) fn generated_bytes(&self) -> u64 {
        self.generated_bytes
    }
    pub(crate) fn auxiliary_count(&self) -> u64 {
        self.auxiliary_count
    }
    pub(crate) fn auxiliary_bytes(&self) -> u64 {
        self.auxiliary_bytes
    }
    pub(crate) fn observed_members(&self) -> u64 {
        self.observed_members
    }
    pub(crate) fn ordered_sha256(&self) -> Digest256 {
        self.ordered_sha256
    }
}

fn max_member_path_bytes_v1(profile: &WeightedScaleProfileV1) -> usize {
    profile
        .classes
        .iter()
        .map(|row| path_for(row.class, row.count.saturating_sub(1)).len())
        .max()
        .unwrap_or(0)
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

// Each generated member owns one ordinal directory. All directories above
// those ordinal directories come from the selected member paths themselves.
fn raw_fixture_directory_count_v1(profile: &WeightedScaleProfileV1) -> std::io::Result<u64> {
    let mut shared = BTreeSet::new();
    shared.insert(PathBuf::new()); // the held raw fixture root
    for row in profile.classes.iter().filter(|row| row.count != 0) {
        let path = path_for(row.class, 0);
        let prefix = Path::new(&path)
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| io_invalid("raw fixture member directory geometry differs"))?;
        shared.extend(prefix.ancestors().map(Path::to_path_buf));
    }
    let shared_count = u64::try_from(shared.len())
        .map_err(|_| io_invalid("raw fixture shared directory count overflow"))?;
    profile
        .target_records
        .checked_add(shared_count)
        .ok_or_else(|| io_invalid("raw fixture directory forecast overflow"))
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

// Scalar preflight deliberately includes identity strings later filtered out.
// It owns no paths, edges or JSON copies and dominates the collector allocation.
fn reference_collection_state_upper_v1(
    value: &serde_json::Value,
) -> std::io::Result<(usize, usize, usize)> {
    fn walk(
        value: &serde_json::Value,
        path_len: usize,
        count: &mut usize,
        strings: &mut usize,
        max_path: &mut usize,
    ) -> std::io::Result<()> {
        *max_path = (*max_path).max(path_len);
        match value {
            serde_json::Value::Object(rows) => {
                for (name, child) in rows {
                    let escaped = name
                        .len()
                        .checked_add(name.bytes().filter(|b| matches!(b, b'~' | b'/')).count())
                        .ok_or_else(|| io_invalid("reference pointer overflow"))?;
                    let next = path_len
                        .checked_add(1)
                        .and_then(|n| n.checked_add(escaped))
                        .ok_or_else(|| io_invalid("reference pointer overflow"))?;
                    walk(child, next, count, strings, max_path)?;
                }
            }
            serde_json::Value::Array(rows) => {
                for (index, child) in rows.iter().enumerate() {
                    let digits = if index == 0 {
                        1
                    } else {
                        index.ilog10() as usize + 1
                    };
                    let next = path_len
                        .checked_add(1)
                        .and_then(|n| n.checked_add(digits))
                        .ok_or_else(|| io_invalid("reference pointer overflow"))?;
                    walk(child, next, count, strings, max_path)?;
                }
            }
            serde_json::Value::String(text) if looks_like_source_reference_v1(text) => {
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| io_invalid("reference count overflow"))?;
                *strings = strings
                    .checked_add(path_len)
                    .and_then(|n| n.checked_add(text.len()))
                    .ok_or_else(|| io_invalid("reference strings overflow"))?;
            }
            _ => {}
        }
        Ok(())
    }
    let (mut count, mut strings, mut max_path) = (0usize, 0usize, 0usize);
    walk(value, 0, &mut count, &mut strings, &mut max_path)?;
    let bytes = count
        .checked_mul(size_of::<ScaleReferenceEdgeV1>())
        .and_then(|n| n.checked_add(strings))
        .and_then(|n| n.checked_add(max_path))
        .ok_or_else(|| io_invalid("reference collection state overflow"))?;
    Ok((count, max_path, bytes))
}

fn collect_reference_edges_v1(
    value: &serde_json::Value,
) -> std::io::Result<Vec<ScaleReferenceEdgeV1>> {
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
                    for ch in name.chars() {
                        match ch {
                            '~' => pointer.push_str("~0"),
                            '/' => pointer.push_str("~1"),
                            _ => pointer.push(ch),
                        }
                    }
                    walk(child, pointer, Some(name), output);
                    pointer.truncate(old_len);
                }
            }
            serde_json::Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let old_len = pointer.len();
                    pointer.push('/');
                    write!(pointer, "{index}").expect("writing into String cannot fail");
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
                })
            }
            _ => {}
        }
    }
    let (count, max_path, _) = reference_collection_state_upper_v1(value)?;
    let mut output = Vec::with_capacity(count);
    walk(
        value,
        &mut String::with_capacity(max_path),
        None,
        &mut output,
    );
    output.sort_unstable();
    output.dedup();
    Ok(output)
}

fn template_rows_retained_state_v1(rows: &[WeightedScaleTemplateV1]) -> std::io::Result<usize> {
    rows.iter().try_fold(0usize, |state, template| {
        let path = match &template.source_path {
            std::borrow::Cow::Borrowed(_) => 0,
            std::borrow::Cow::Owned(path) => path.capacity(),
        };
        let edges = template.external_reference_edges.iter().try_fold(
            template
                .external_reference_edges
                .capacity()
                .checked_mul(size_of::<ScaleReferenceEdgeV1>())
                .ok_or_else(|| io_invalid("template edges overflow"))?,
            |state, edge| {
                state
                    .checked_add(edge.pointer.capacity())
                    .and_then(|n| n.checked_add(edge.reference.capacity()))
                    .ok_or_else(|| io_invalid("template edge strings overflow"))
            },
        )?;
        state
            .checked_add(
                template
                    .generated_author
                    .as_ref()
                    .map_or(0, String::capacity),
            )
            .and_then(|n| n.checked_add(template.bytes.capacity()))
            .and_then(|n| n.checked_add(path))
            .and_then(|n| n.checked_add(edges))
            .ok_or_else(|| io_invalid("template retained state overflow"))
    })
}

fn template_array_retained_state_v1(
    rows: &[Vec<WeightedScaleTemplateV1>; 5],
) -> std::io::Result<usize> {
    rows.iter()
        .try_fold(size_of::<WeightedScaleTemplateSetV1>(), |state, rows| {
            let headers = rows
                .capacity()
                .checked_mul(size_of::<WeightedScaleTemplateV1>())
                .ok_or_else(|| io_invalid("template row headers overflow"))?;
            state
                .checked_add(headers)
                .and_then(|n| {
                    template_rows_retained_state_v1(rows)
                        .ok()
                        .and_then(|m| n.checked_add(m))
                })
                .ok_or_else(|| io_invalid("template array state overflow"))
        })
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
    for edge in collect_reference_edges_v1(value)? {
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
        if template
            .external_reference_edges
            .binary_search(&edge)
            .is_ok()
        {
            closure.pinned_external_dependency_edges = closure
                .pinned_external_dependency_edges
                .checked_add(1)
                .ok_or_else(|| io_invalid("external dependency edge count overflow"))?;
            let key = ScaleReferenceUsageKeyV1 {
                class: format!("{:?}", class),
                source_path: template.source_path.to_string(),
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
    claim_count: u64,
    selected_semantic_fixture: bool,
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
                if !selected_semantic_fixture {
                    *work_ref = serde_json::Value::String(scale_identity(
                        seed,
                        WeightedScaleClassV1::Work,
                        ordinal % work_count,
                    ));
                }
                // Selected semantic packets segment the original authenticated
                // witness. Its Work/Expression/Edition/Item scope stays coherent;
                // only packet-local identities are generated.
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
            // The technical route clears stale source refs. The selected route
            // declares every actual generated Claim assigned to this Work by
            // the same maintained modulo subject algorithm.
            let responsibility = if selected_semantic_fixture {
                let assigned_count = if ordinal < claim_count {
                    (claim_count - 1 - ordinal) / work_count + 1
                } else {
                    0
                };
                let count = usize::try_from(assigned_count)
                    .map_err(|_| io_invalid("Work claim count exceeds address space"))?;
                let mut refs = Vec::with_capacity(count);
                let mut claim_ordinal = ordinal;
                while claim_ordinal < claim_count {
                    refs.push(serde_json::Value::String(scale_identity(
                        seed,
                        WeightedScaleClassV1::Claim,
                        claim_ordinal,
                    )));
                    if claim_count - claim_ordinal <= work_count {
                        break;
                    }
                    claim_ordinal = claim_ordinal
                        .checked_add(work_count)
                        .ok_or_else(|| io_invalid("Work claim ordinal overflow"))?;
                }
                refs
            } else {
                Vec::new()
            };
            object.insert(
                "responsibility_claim_refs".to_owned(),
                serde_json::Value::Array(responsibility),
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
    } else if bucket < SCALE_SELECTED_QUANTILE_BUCKETS_V1[0] + SCALE_SELECTED_QUANTILE_BUCKETS_V1[1]
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

/// Exact finite bytes selected by the authored record/slot selection owner.
/// This descriptor is not a semantic reference-resolution or rights verdict.
#[derive(Clone, Debug)]
pub(crate) struct WeightedScaleAuthoredAuxMemberV1 {
    pub(crate) path: String,
    pub(crate) raw_sha256: Digest256,
    pub(crate) raw_bytes: u64,
}

/// Producer custody of an independently authenticated authored selection.
/// The caller retains its record/slot proof; this owner binds exact byte members.
pub(crate) struct WeightedScaleAuthoredAuxSelectionV1 {
    authored_manifest_sha256: Digest256,
    members: Vec<WeightedScaleAuthoredAuxMemberV1>,
    source_bytes: u64,
    state_bytes: usize,
}
impl WeightedScaleAuthoredAuxSelectionV1 {
    pub(crate) fn from_owner_selection(
        authored_manifest_sha256: Digest256,
        mut members: Vec<WeightedScaleAuthoredAuxMemberV1>,
        max_members: usize,
        max_source_bytes: u64,
        max_owned_state_bytes: usize,
    ) -> std::io::Result<Self> {
        if max_members == 0
            || max_members == usize::MAX
            || members.is_empty()
            || members.len() > max_members
            || max_source_bytes == 0
            || max_source_bytes == u64::MAX
            || max_owned_state_bytes == 0
            || max_owned_state_bytes == usize::MAX
        {
            return Err(io_invalid("authored auxiliary selection bounds differ"));
        }
        let mut state_bytes = size_of::<Self>()
            .checked_add(
                members
                    .capacity()
                    .checked_mul(size_of::<WeightedScaleAuthoredAuxMemberV1>())
                    .ok_or_else(|| io_invalid("authored selection state overflow"))?,
            )
            .ok_or_else(|| io_invalid("authored selection state overflow"))?;
        let mut source_bytes = 0u64;
        for member in &members {
            RelativePath::parse(&member.path)
                .map_err(|_| io_invalid("authored auxiliary source path differs"))?;
            if !member.path.starts_with("ToS/")
                || member.raw_bytes == 0
                || WeightedScaleClassV1::ALL.into_iter().any(|class| {
                    member.path.starts_with(generated_path_prefix_v1(class))
                        || generated_path_prefix_v1(class)
                            .strip_prefix(member.path.as_str())
                            .is_some_and(|tail| tail.starts_with('/'))
                })
            {
                return Err(io_invalid("authored member overlaps generated namespace"));
            }
            state_bytes = state_bytes
                .checked_add(member.path.capacity())
                .ok_or_else(|| io_invalid("authored selection state overflow"))?;
            source_bytes = source_bytes
                .checked_add(member.raw_bytes)
                .ok_or_else(|| io_invalid("authored auxiliary bytes overflow"))?;
        }
        if state_bytes > max_owned_state_bytes || source_bytes > max_source_bytes {
            return Err(io_invalid("authored selection exceeds selected limits"));
        }
        members.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        if members.windows(2).any(|w| w[0].path == w[1].path) {
            return Err(io_invalid("authored auxiliary paths collide"));
        }
        for member in &members {
            for parent in Path::new(&member.path).ancestors().skip(1) {
                if members
                    .binary_search_by(|candidate| {
                        candidate.path.as_str().cmp(parent.to_str().unwrap_or(""))
                    })
                    .is_ok()
                {
                    return Err(io_invalid(
                        "authored auxiliary file/directory paths collide",
                    ));
                }
            }
        }
        Ok(Self {
            authored_manifest_sha256,
            members,
            source_bytes,
            state_bytes,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WeightedScaleCompositionBindingV1 {
    pub(crate) authored_manifest_sha256: Digest256,
    pub(crate) generated_declaration_sha256: Digest256,
    pub(crate) auxiliary_members_sha256: Digest256,
    pub(crate) generated_record_count: u64,
    pub(crate) generated_support_member_count: u64,
    pub(crate) auxiliary_member_count: u64,
    pub(crate) auxiliary_source_bytes: u64,
    pub(crate) members_descriptor_sha256: Digest256,
}

fn generated_declaration_digest_v1(
    profile: &WeightedScaleProfileV1,
    templates: Digest256,
) -> std::io::Result<Digest256> {
    // The maintained owner derives every path and ID from these inputs. No
    // generated IDs or rows are retained in an auxiliary manifest.
    let counts = profile.classes.map(|row| row.count);
    let bytes = canonical_value_bytes_v1(&serde_json::json!({
        "domain": "tos_scale_generated_declaration_v1",
        "seed_sha256": profile.seed.to_hex(),
        "template_manifest_sha256": templates.to_hex(),
        "generated_record_count": profile.target_records,
        "class_counts": counts,
    }))?;
    Ok(Digest256::of_bytes(&bytes))
}

fn completed_artifact_selection_v2(
    templates: &WeightedScaleTemplateSetV1,
) -> std::io::Result<Option<serde_json::Value>> {
    let Some(recipe) = &templates.artifact_recipe else {
        return Ok(None);
    };
    let observation = templates
        .output_observation
        .as_ref()
        .ok_or_else(|| io_invalid("Artifact selection has no completed output phase"))?;
    let mut value = recipe.selection.profile_value();
    value["immutable_recipe_sha256"] =
        serde_json::json!(recipe.selection.immutable_digest()?.to_hex());
    value["output_observation"] = serde_json::json!({
        "started_at": observation.started_at, "ended_at": observation.ended_at,
        "member_count": observation.member_count, "source_bytes": observation.source_bytes,
        "ordered_output_sha256": observation.ordered_output_sha256.to_hex(),
    });
    Ok(Some(value))
}
fn generated_declaration_digest_selected_v2(
    profile: &WeightedScaleProfileV1,
    templates: &WeightedScaleTemplateSetV1,
) -> std::io::Result<Digest256> {
    let Some(selection) = completed_artifact_selection_v2(templates)? else {
        return generated_declaration_digest_v1(profile, templates.manifest_sha256);
    };
    Ok(Digest256::of_bytes(&canonical_value_bytes_v1(
        &serde_json::json!({
            "domain": "tos_scale_generated_declaration_v2",
            "artifact_recipe_sha256": Digest256::of_bytes(&canonical_value_bytes_v1(&selection)?).to_hex(),
            "immutable_recipe_sha256": selection["immutable_recipe_sha256"],
            "output_observation": selection["output_observation"],
            "seed_sha256": profile.seed.to_hex(),
            "template_manifest_sha256": templates.manifest_sha256.to_hex(),
            "generated_record_count": profile.target_records,
            "generated_support_member_count": artifact_support_member_count_v1(profile.classes[WeightedScaleClassV1::Artifact as usize].count)?,
            "class_counts": profile.classes.map(|row| row.count),
        }),
    )?))
}

fn auxiliary_members_digest_v1(selection: &WeightedScaleAuthoredAuxSelectionV1) -> Digest256 {
    let mut hash = auxiliary_members_hasher_v1(selection.authored_manifest_sha256);
    for member in &selection.members {
        feed_auxiliary_member_v1(&mut hash, &member.path, member.raw_sha256, member.raw_bytes);
    }
    hash.finalize()
}

fn auxiliary_members_hasher_v1(manifest: Digest256) -> Digest256Hasher {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-scale-authored-aux-members-v1\0");
    hash.update(manifest.as_bytes());
    hash
}

fn feed_auxiliary_member_v1(
    hash: &mut Digest256Hasher,
    path: &str,
    raw_sha256: Digest256,
    raw_bytes: u64,
) {
    hash.update(&(path.len() as u64).to_be_bytes());
    hash.update(path.as_bytes());
    hash.update(raw_sha256.as_bytes());
    hash.update(&raw_bytes.to_be_bytes());
}

/// Bind the one already parsed finite authored selection to byte composition.
/// This does not authenticate generated members or semantic admission.
pub(crate) fn verify_composition_against_source_record_selection_v1(
    composition: &super::source_admission_indexed_input::IndexedInputCompositionV1,
    selection: &tos_validation::source_record_selection::SourceRecordSelection,
    before_member: &mut impl FnMut() -> std::io::Result<()>,
) -> std::io::Result<()> {
    if selection.digest() != composition.authored_manifest_sha256 {
        return Err(io_invalid("composed authored selection identity differs"));
    }
    let mut hash = auxiliary_members_hasher_v1(selection.digest());
    let mut count = 0u64;
    let mut source_bytes = 0u64;
    let mut previous = None;
    for member in selection.members() {
        before_member()?;
        if previous.is_some_and(|path: &str| path >= member.source_ref.as_str()) {
            return Err(io_invalid("composed authored members order differs"));
        }
        previous = Some(member.source_ref.as_str());
        count = count
            .checked_add(1)
            .ok_or_else(|| io_invalid("authored member count overflow"))?;
        source_bytes = source_bytes
            .checked_add(member.raw_bytes)
            .ok_or_else(|| io_invalid("authored member source bytes overflow"))?;
        let digest = Digest256::from_hex(&member.raw_sha256)
            .map_err(|_| io_invalid("authored member raw digest differs"))?;
        feed_auxiliary_member_v1(&mut hash, &member.source_ref, digest, member.raw_bytes);
    }
    if count != composition.auxiliary_member_count
        || source_bytes != composition.auxiliary_source_bytes
        || hash.finalize() != composition.auxiliary_members_sha256
    {
        return Err(io_invalid("composed authored members binding differs"));
    }
    Ok(())
}

fn authored_aux_directory_paths_v1(
    profile: &WeightedScaleProfileV1,
    selection: &WeightedScaleAuthoredAuxSelectionV1,
    max_state_bytes: usize,
    caller_state_bytes: usize,
) -> std::io::Result<Vec<PathBuf>> {
    // Reserve finite prefix storage before allocating it. Sorted Vec metadata
    // uses actual element/path capacities, without a tree-node cost guess.
    let mut prefix_count = 0usize;
    let mut prefix_bytes = 0usize;
    for member in &selection.members {
        for ancestor in Path::new(&member.path).ancestors().skip(1) {
            prefix_count = prefix_count
                .checked_add(1)
                .ok_or_else(|| io_invalid("authored prefix count overflow"))?;
            prefix_bytes = prefix_bytes
                .checked_add(ancestor.as_os_str().len())
                .ok_or_else(|| io_invalid("authored prefix bytes overflow"))?;
        }
    }
    let selected_peak = prefix_count
        .checked_mul(size_of::<PathBuf>())
        .and_then(|bytes| bytes.checked_add(prefix_bytes))
        .and_then(|bytes| bytes.checked_add(size_of::<Vec<PathBuf>>()))
        .and_then(|bytes| bytes.checked_add(selection.state_bytes))
        .and_then(|bytes| bytes.checked_add(caller_state_bytes))
        .ok_or_else(|| io_invalid("authored prefix state overflow"))?;
    if selected_peak > max_state_bytes {
        return Err(io_invalid("authored prefixes exceed selected state"));
    }
    let mut additional = Vec::new();
    additional
        .try_reserve_exact(prefix_count)
        .map_err(|_| io_invalid("authored prefix allocation failed"))?;
    for member in &selection.members {
        for ancestor in Path::new(&member.path).ancestors().skip(1) {
            // All shared generated ancestors come directly from the held
            // class paths. Auxiliary paths cannot enter a generated namespace.
            let shared = ancestor.as_os_str().is_empty()
                || profile
                    .classes
                    .iter()
                    .filter(|row| row.count != 0)
                    .any(|row| {
                        let path = path_for(row.class, 0);
                        Path::new(&path)
                            .parent()
                            .and_then(Path::parent)
                            .is_some_and(|parent| parent.ancestors().any(|p| p == ancestor))
                    });
            if !shared {
                additional.push(ancestor.to_path_buf());
            }
        }
    }
    let actual_state = additional.iter().try_fold(
        additional
            .capacity()
            .checked_mul(size_of::<PathBuf>())
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<PathBuf>>()))
            .ok_or_else(|| io_invalid("authored prefix capacity overflow"))?,
        |bytes, path| {
            bytes
                .checked_add(path.capacity())
                .ok_or_else(|| io_invalid("authored prefix path capacity overflow"))
        },
    )?;
    if actual_state
        .checked_add(selection.state_bytes)
        .and_then(|bytes| bytes.checked_add(caller_state_bytes))
        .is_none_or(|bytes| bytes > max_state_bytes)
    {
        return Err(io_invalid(
            "authored prefix capacity exceeds selected state",
        ));
    }
    additional.sort_unstable();
    additional.dedup();
    Ok(additional)
}

/// One dimensional price for the opt-in producer and its caller's pre-write bill.
/// Generated semantic records and auxiliary physical resources remain distinct.
pub(crate) struct WeightedScaleComposedEnvelopeV1 {
    pub(crate) envelope: WeightedScaleProducerEnvelopeV1,
    pub(crate) forecast: WeightedScaleForecastInputsV1,
    pub(crate) member_count: u64,
    pub(crate) auxiliary_member_count: u64,
    pub(crate) auxiliary_source_bytes: u64,
    pub(crate) required_member_key_bytes: usize,
    pub(crate) maximum_member_bytes: u64,
    pub(crate) directory_state_bytes: usize,
}

pub(crate) fn weighted_scale_composed_producer_envelope_v1(
    profile: &WeightedScaleProfileV1,
    selection: &WeightedScaleAuthoredAuxSelectionV1,
    allocation_unit: u64,
    raw_input_root: &Path,
    max_working_state_bytes: usize,
    caller_live_state_bytes: usize,
) -> std::io::Result<WeightedScaleComposedEnvelopeV1> {
    weighted_scale_composition_price_v1(
        profile,
        Some(selection),
        allocation_unit,
        raw_input_root,
        max_working_state_bytes,
        caller_live_state_bytes,
    )
}

/// Packed recipe inputs have physical members without a per-member raw inode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WeightedScaleRepresentationV1 {
    RawAndPackedV1,
    PackedOnlyV2,
}

pub(crate) fn weighted_scale_composed_producer_envelope_with_templates_v2(
    profile: &WeightedScaleProfileV1,
    templates: &WeightedScaleTemplateSetV1,
    selection: &WeightedScaleAuthoredAuxSelectionV1,
    representation: WeightedScaleRepresentationV1,
    allocation_unit: u64,
    raw_input_root: Option<&Path>,
    max_working_state_bytes: usize,
    caller_live_state_bytes: usize,
) -> std::io::Result<WeightedScaleComposedEnvelopeV1> {
    if representation != WeightedScaleRepresentationV1::PackedOnlyV2
        || raw_input_root.is_some()
        || templates.artifact_recipe.is_none()
        || templates.fixture_recipe_source.is_none()
    {
        return Err(io_invalid(
            "declared fixture requires packed-only recipe selection",
        ));
    }
    let mut price = weighted_scale_composition_price_v1(
        profile,
        Some(selection),
        allocation_unit,
        Path::new("/"),
        max_working_state_bytes,
        caller_live_state_bytes,
    )?;
    let add = |a: u64, b: u64| {
        a.checked_add(b)
            .ok_or_else(|| io_invalid("recipe support price overflow"))
    };
    let mul = |a: u64, b: u64| {
        a.checked_mul(b)
            .ok_or_else(|| io_invalid("recipe support price overflow"))
    };
    let artifact_count = profile.classes[WeightedScaleClassV1::Artifact as usize].count;
    // This value is private to a counting writer. Its maximal numeric widths
    // bound a future event; it is never emitted as an output observation.
    let price_only = WeightedScaleArtifactOutputObservationV1 {
        started_at: "9999-12-31T23:59:59.999999999Z".to_owned(),
        ended_at: "9999-12-31T23:59:59.999999999Z".to_owned(),
        member_count: u64::MAX,
        source_bytes: u64::MAX,
        ordered_output_sha256: Digest256::of_bytes(b"price only; never an observed output"),
    };
    let mut support_sizes = [0u64; 3];
    for (index, bucket) in [0u64, 90, 99].into_iter().enumerate() {
        // Largest u64 ordinal in this quantile has the maximal numeric width.
        let ordinal = u64::MAX - ((u64::MAX % 100 + 100 - bucket) % 100);
        for role in WeightedScaleArtifactSupportRoleV1::ALL {
            let mut writer = GeneratedDigestWriterV1 {
                digest: Digest256Hasher::new(),
                bytes: 0,
            };
            write_fixture_support_v2(
                &mut writer,
                profile,
                templates,
                role,
                ordinal,
                Some(&price_only),
            )?;
            support_sizes[index] = add(support_sizes[index], writer.bytes)?;
            price.maximum_member_bytes = price.maximum_member_bytes.max(writer.bytes);
            price.required_member_key_bytes = price
                .required_member_key_bytes
                .max(role.path(ordinal).len());
        }
    }
    let scenario = |count| -> std::io::Result<u64> {
        selected_quantile_counts_v1(count)?
            .into_iter()
            .zip(support_sizes)
            .try_fold(0, |sum, (n, bytes)| add(sum, mul(n, bytes)?))
    };
    let support_bytes = scenario(artifact_count)?;
    let support_p50 = mul(artifact_count, support_sizes[0])?;
    let support_count = artifact_support_member_count_v1(artifact_count)?;
    price.member_count = add(price.member_count, support_count)?;
    price.directory_state_bytes = 0;
    let envelope = &mut price.envelope;
    envelope.maximum_source_bytes = add(envelope.maximum_source_bytes, support_bytes)?;
    envelope.raw_input_allocated_bytes = 0;
    let sort_bytes = mul(price.member_count, SCALE_SORT_ROW_BYTES_V1)?;
    let runs = sort_run_count_v1(price.member_count)?;
    envelope.temporary_logical_bytes = add(envelope.maximum_source_bytes, sort_bytes)?;
    envelope.temporary_allocated_bytes =
        scratch_allocation_upper_v1(envelope.maximum_source_bytes, runs, allocation_unit)?;
    envelope.temporary_file_inodes = add(runs, 2)?;
    let forecast = &mut price.forecast;
    forecast.raw_input_file_count = 0;
    forecast.raw_input_directory_count = 0;
    forecast.raw_input_inode_count = 0;
    forecast.p50_logical_source_bytes = add(forecast.p50_logical_source_bytes, support_p50)?;
    forecast.selected_quantile_scenario_logical_source_bytes = add(
        forecast.selected_quantile_scenario_logical_source_bytes,
        support_bytes,
    )?;
    for role in WeightedScaleArtifactSupportRoleV1::ALL {
        forecast.raw_member_leaf_input_bytes = add(
            forecast.raw_member_leaf_input_bytes,
            mul(
                artifact_count,
                (role.path(0).len() + SCALE_MEMBER_VALUE_BYTES_V1) as u64,
            )?,
        )?;
    }
    forecast.raw_object_extent_leaf_input_bytes = add(
        forecast.raw_object_extent_leaf_input_bytes,
        mul(support_count, 32 + 76)?,
    )?;
    forecast.external_sort_logical_bytes = sort_bytes;
    forecast.temporary_payload_spool_peak_bytes =
        forecast.selected_quantile_scenario_logical_source_bytes;
    forecast.temporary_digest_sort_peak_bytes = sort_bytes;
    forecast.temporary_scratch_peak_bytes =
        add(forecast.temporary_payload_spool_peak_bytes, sort_bytes)?;
    forecast.temporary_scratch_blocks_4k_assumption = add(
        forecast.temporary_scratch_peak_bytes,
        SCALE_ALLOCATION_BLOCK_ASSUMPTION_V1 - 1,
    )? / SCALE_ALLOCATION_BLOCK_ASSUMPTION_V1;
    forecast.temporary_sort_run_count = runs;
    forecast.temporary_file_inode_peak = envelope.temporary_file_inodes;
    forecast.packed_object_pack_count_upper =
        add(price.member_count, MAX_PACKED_OBJECT_FRAMES_V2 as u64 - 1)?
            / MAX_PACKED_OBJECT_FRAMES_V2 as u64;
    forecast.current_snapshot_three_copy_p50_bytes = mul(forecast.p50_logical_source_bytes, 3)?;
    forecast.three_pins_with_backup_and_restore_no_dedup_p50_bytes =
        mul(forecast.p50_logical_source_bytes, 5)?;
    let billion_artifacts =
        WeightedScaleProfileV1::weighted_for_records(profile.seed, 1_000_000_000)?.classes
            [WeightedScaleClassV1::Artifact as usize]
            .count;
    forecast.p50_logical_source_bytes_at_1b = add(
        forecast.p50_logical_source_bytes_at_1b,
        mul(billion_artifacts, support_sizes[0])?,
    )?;
    let billion_support = scenario(billion_artifacts)?;
    forecast.selected_quantile_scenario_logical_source_bytes_at_1b = add(
        forecast.selected_quantile_scenario_logical_source_bytes_at_1b,
        billion_support,
    )?;
    forecast.ten_full_copy_no_dedup_scenario_bytes_at_1b = add(
        forecast.ten_full_copy_no_dedup_scenario_bytes_at_1b,
        mul(billion_support, 10)?,
    )?;
    // Conservative history envelope: every changed meaningful row may carry
    // a complete Artifact support bundle. Sharing must be measured separately.
    forecast.history_change_payload_scenario_bytes = add(
        forecast.history_change_payload_scenario_bytes,
        mul(
            forecast.history_change_rows,
            *support_sizes.iter().max().unwrap_or(&0),
        )?,
    )?;
    forecast.writer_staging_upper_bytes_per_client = Some(envelope.temporary_allocated_bytes);
    forecast.write_clients_staging_upper_bytes = Some(mul(
        envelope.temporary_allocated_bytes,
        forecast.expected_write_clients as u64,
    )?);
    Ok(price)
}

fn weighted_scale_composition_price_v1(
    profile: &WeightedScaleProfileV1,
    auxiliary: Option<&WeightedScaleAuthoredAuxSelectionV1>,
    allocation_unit: u64,
    raw_input_root: &Path,
    max_working_state_bytes: usize,
    caller_live_state_bytes: usize,
) -> std::io::Result<WeightedScaleComposedEnvelopeV1> {
    if max_working_state_bytes == 0
        || max_working_state_bytes == usize::MAX
        || caller_live_state_bytes >= max_working_state_bytes
    {
        return Err(io_invalid("composed price selected state differs"));
    }
    let required_member_key_bytes = auxiliary.map_or(max_member_path_bytes_v1(&profile), |s| {
        s.members
            .iter()
            .map(|m| m.path.len())
            .max()
            .unwrap_or(0)
            .max(max_member_path_bytes_v1(&profile))
    });
    let auxiliary_count = auxiliary.map_or(0, |s| s.members.len() as u64);
    let total_members = profile
        .target_records
        .checked_add(auxiliary_count)
        .ok_or_else(|| io_invalid("composed member count overflow"))?;
    let auxiliary_bytes = auxiliary.map_or(0, |s| s.source_bytes);
    let mut forecast = profile.forecast_inputs()?;
    let mut envelope = weighted_scale_producer_envelope_v1(&profile, allocation_unit)?;
    let extra_directories = match auxiliary {
        Some(selection) => authored_aux_directory_paths_v1(
            &profile,
            selection,
            max_working_state_bytes,
            caller_live_state_bytes,
        )?,
        None => Vec::new(),
    };
    let extra_directory_count = extra_directories.len() as u64;
    let directory_state = extra_directories.iter().try_fold(
        extra_directories
            .capacity()
            .checked_mul(size_of::<PathBuf>())
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<PathBuf>>()))
            .ok_or_else(|| io_invalid("authored directory vector state overflow"))?,
        |total, path| {
            // Relative prefix scratch + absolute paths retained by the RAW
            // writer. Include two vector/path capacities during a growth step.
            total
                .checked_add(path.capacity())
                .and_then(|bytes| bytes.checked_add(size_of::<PathBuf>()))
                .and_then(|bytes| bytes.checked_add(path.as_os_str().len()))
                .and_then(|bytes| bytes.checked_add(raw_input_root.as_os_str().len()))
                .and_then(|bytes| bytes.checked_add(1))
                .ok_or_else(|| io_invalid("authored directory state overflow"))
        },
    )?;
    forecast.raw_input_file_count = total_members;
    forecast.raw_input_directory_count = forecast
        .raw_input_directory_count
        .checked_add(extra_directory_count)
        .ok_or_else(|| io_invalid("composed directory count overflow"))?;
    forecast.raw_input_inode_count = total_members
        .checked_add(forecast.raw_input_directory_count)
        .ok_or_else(|| io_invalid("composed inode count overflow"))?;
    envelope.maximum_source_bytes = envelope
        .maximum_source_bytes
        .checked_add(auxiliary_bytes)
        .ok_or_else(|| io_invalid("composed source byte envelope overflow"))?;
    let composed_sort_bytes = total_members
        .checked_mul(SCALE_SORT_ROW_BYTES_V1)
        .ok_or_else(|| io_invalid("composed sort bytes overflow"))?;
    envelope.temporary_logical_bytes = envelope
        .maximum_source_bytes
        .checked_add(composed_sort_bytes)
        .ok_or_else(|| io_invalid("composed scratch bytes overflow"))?;
    let composed_runs = sort_run_count_v1(total_members)?;
    envelope.temporary_allocated_bytes = scratch_allocation_upper_v1(
        envelope.maximum_source_bytes,
        composed_runs,
        allocation_unit,
    )?;
    envelope.temporary_file_inodes = composed_runs
        .checked_add(2)
        .ok_or_else(|| io_invalid("composed scratch inode overflow"))?;
    if let Some(selection) = auxiliary {
        for member in &selection.members {
            let allocated = round_up_scale_v1(member.raw_bytes, allocation_unit)?
                .checked_add(allocation_unit)
                .ok_or_else(|| io_invalid("composed auxiliary file allocation overflow"))?;
            envelope.raw_input_allocated_bytes = envelope
                .raw_input_allocated_bytes
                .checked_add(allocated)
                .ok_or_else(|| io_invalid("composed raw allocation overflow"))?;
        }
        envelope.raw_input_allocated_bytes = envelope
            .raw_input_allocated_bytes
            .checked_add(
                extra_directory_count
                    .checked_mul(allocation_unit)
                    .and_then(|v| v.checked_mul(2))
                    .ok_or_else(|| io_invalid("composed directory allocation overflow"))?,
            )
            .ok_or_else(|| io_invalid("composed raw allocation overflow"))?;
        forecast.p50_logical_source_bytes = forecast
            .p50_logical_source_bytes
            .checked_add(auxiliary_bytes)
            .ok_or_else(|| io_invalid("composed forecast overflow"))?;
        forecast.selected_quantile_scenario_logical_source_bytes = forecast
            .selected_quantile_scenario_logical_source_bytes
            .checked_add(auxiliary_bytes)
            .ok_or_else(|| io_invalid("composed forecast overflow"))?;
        for member in &selection.members {
            forecast.raw_member_leaf_input_bytes = forecast
                .raw_member_leaf_input_bytes
                .checked_add(member.path.len() as u64 + SCALE_MEMBER_VALUE_BYTES_V1 as u64)
                .ok_or_else(|| io_invalid("composed member tree forecast overflow"))?;
        }
        forecast.raw_object_extent_leaf_input_bytes = forecast
            .raw_object_extent_leaf_input_bytes
            .checked_add(
                auxiliary_count
                    .checked_mul(32 + 76)
                    .ok_or_else(|| io_invalid("composed object tree forecast overflow"))?,
            )
            .ok_or_else(|| io_invalid("composed object tree forecast overflow"))?;
        forecast.external_sort_logical_bytes = composed_sort_bytes;
        forecast.temporary_payload_spool_peak_bytes =
            forecast.selected_quantile_scenario_logical_source_bytes;
        forecast.temporary_digest_sort_peak_bytes = composed_sort_bytes;
        forecast.temporary_scratch_peak_bytes = forecast
            .temporary_payload_spool_peak_bytes
            .checked_add(composed_sort_bytes)
            .ok_or_else(|| io_invalid("composed scratch forecast overflow"))?;
        forecast.temporary_scratch_blocks_4k_assumption = forecast
            .temporary_scratch_peak_bytes
            .checked_add(SCALE_ALLOCATION_BLOCK_ASSUMPTION_V1 - 1)
            .ok_or_else(|| io_invalid("composed scratch blocks overflow"))?
            / SCALE_ALLOCATION_BLOCK_ASSUMPTION_V1;
        forecast.current_snapshot_three_copy_p50_bytes = forecast
            .current_snapshot_three_copy_p50_bytes
            .checked_add(
                auxiliary_bytes
                    .checked_mul(3)
                    .ok_or_else(|| io_invalid("composed three-copy forecast overflow"))?,
            )
            .ok_or_else(|| io_invalid("composed three-copy forecast overflow"))?;
        forecast.three_pins_with_backup_and_restore_no_dedup_p50_bytes = forecast
            .three_pins_with_backup_and_restore_no_dedup_p50_bytes
            .checked_add(
                auxiliary_bytes
                    .checked_mul(5)
                    .ok_or_else(|| io_invalid("composed retained forecast overflow"))?,
            )
            .ok_or_else(|| io_invalid("composed retained forecast overflow"))?;
        forecast.p50_logical_source_bytes_at_1b = forecast
            .p50_logical_source_bytes_at_1b
            .checked_add(auxiliary_bytes)
            .ok_or_else(|| io_invalid("composed billion forecast overflow"))?;
        forecast.selected_quantile_scenario_logical_source_bytes_at_1b = forecast
            .selected_quantile_scenario_logical_source_bytes_at_1b
            .checked_add(auxiliary_bytes)
            .ok_or_else(|| io_invalid("composed billion forecast overflow"))?;
        forecast.ten_full_copy_no_dedup_scenario_bytes_at_1b = forecast
            .ten_full_copy_no_dedup_scenario_bytes_at_1b
            .checked_add(
                auxiliary_bytes
                    .checked_mul(10)
                    .ok_or_else(|| io_invalid("composed billion copy forecast overflow"))?,
            )
            .ok_or_else(|| io_invalid("composed billion copy forecast overflow"))?;
        forecast.temporary_sort_run_count = composed_runs;
        forecast.temporary_file_inode_peak = envelope.temporary_file_inodes;
        forecast.packed_object_pack_count_upper = total_members
            .checked_add(MAX_PACKED_OBJECT_FRAMES_V2 as u64 - 1)
            .ok_or_else(|| io_invalid("composed pack forecast overflow"))?
            / MAX_PACKED_OBJECT_FRAMES_V2 as u64;
    }
    drop(extra_directories);
    let selected_state = u64::try_from(max_working_state_bytes)
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

    let generated_max = profile.classes.iter().try_fold(0u64, |maximum, row| {
        row.max_bytes
            .checked_add(SCALE_IDENTITY_GROWTH_ALLOWANCE_V1)
            .map(|bytes| maximum.max(bytes))
            .ok_or_else(|| io_invalid("generated maximum member overflow"))
    })?;
    let maximum_member_bytes = auxiliary.map_or(generated_max, |selection| {
        selection
            .members
            .iter()
            .map(|member| member.raw_bytes)
            .max()
            .unwrap_or(0)
            .max(generated_max)
    });
    Ok(WeightedScaleComposedEnvelopeV1 {
        envelope,
        forecast,
        member_count: total_members,
        auxiliary_member_count: auxiliary_count,
        auxiliary_source_bytes: auxiliary_bytes,
        required_member_key_bytes,
        maximum_member_bytes,
        directory_state_bytes: directory_state,
    })
}

/// All resource ceilings are copied from the selected Native invocation. The
/// producer checks its conservative pre-write scratch envelope against these
/// values and the supplied shared ledgers before creating the private root.
pub(crate) struct WeightedScaleProducerRequestV1<'a> {
    pub(crate) repository_root: &'a Path,
    pub(crate) representation: WeightedScaleRepresentationV1,
    pub(crate) raw_input_root: Option<&'a Path>,
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

fn weighted_prewrite_refusal_v1(
    profile: &WeightedScaleProfileV1,
    forecast: &WeightedScaleForecastInputsV1,
    envelope: &WeightedScaleProducerEnvelopeV1,
    request: &WeightedScaleProducerRequestV1<'_>,
    segment_frame_cap: u64,
    required_member_key_bytes: usize,
) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
            "weighted producer prewrite refusal: target_records={} source_upper_bytes={} raw_files={} raw_directories={} raw_allocated_upper_bytes={} scratch_logical_upper_bytes={} scratch_allocated_upper_bytes={} scratch_inode_upper={} allocation_unit_bytes={} member_tree_rows={} object_tree_rows={} object_count={} member_frame_upper_bytes={} requested_member_path_bytes={}\nselected_caps: source_bytes={} raw_files={} raw_directories={} raw_allocated_bytes={} scratch_logical_bytes={} scratch_allocated_bytes={} scratch_inodes={} member_tree_rows={} object_tree_rows={} object_count={} member_frame_bytes={} member_key_bytes={}\nprofile_price: p50_logical_source_bytes={} selected_quantile_logical_source_bytes={} external_sort_logical_bytes={} history_change_payload_scenario_bytes={}",
            profile.target_records,
            envelope.maximum_source_bytes,
            forecast.raw_input_file_count,
            forecast.raw_input_directory_count,
            envelope.raw_input_allocated_bytes,
            envelope.temporary_logical_bytes,
            envelope.temporary_allocated_bytes,
            envelope.temporary_file_inodes,
            request.tree_io.selected_allocation_unit_bytes(),
            forecast.raw_input_file_count,
            forecast.raw_input_file_count,
            forecast.raw_input_file_count,
            profile
                .classes
                .iter()
                .map(|row| row.max_bytes)
                .max()
                .unwrap_or(0)
                .saturating_add(SCALE_IDENTITY_GROWTH_ALLOWANCE_V1),
            required_member_key_bytes,
            request.max_source_bytes,
            request.max_raw_input_files,
            request.max_raw_input_directories,
            request.max_raw_input_allocated_bytes,
            request.max_temporary_logical_bytes,
            request.max_temporary_allocated_bytes,
            request.max_temporary_inodes,
            request.member_tree_limits.max_rows,
            request.object_limits.tree_limits.max_rows,
            request.object_limits.max_objects,
            request.max_member_bytes.min(segment_frame_cap),
            request.member_tree_limits.max_key_bytes,
            forecast.p50_logical_source_bytes,
            forecast.selected_quantile_scenario_logical_source_bytes,
            forecast.external_sort_logical_bytes,
            forecast.history_change_payload_scenario_bytes,
        ),
    )
}

/// A completed private packed input. The descriptors and byte hashes are
/// mechanically derived from this held store. The receipt grants no source,
/// review, rights, canon, or admission authority.
pub(crate) struct RawScaleInputReceiptV1 {
    pub(crate) raw_input_root: PathBuf,
    pub(crate) held_raw_input_root: File,
    pub(crate) raw_input_file_count: u64,
    pub(crate) raw_input_directory_count: u64,
    pub(crate) raw_input_inode_count: u64,
    pub(crate) raw_input_source_bytes: u64,
    pub(crate) raw_input_allocated_bytes: u64,
}
pub(crate) struct PackedScaleInputReceiptV1 {
    pub(crate) representation: WeightedScaleRepresentationV1,
    pub(crate) raw_input: Option<RawScaleInputReceiptV1>,
    source_recipe: Option<WeightedScaleFixtureRecipeSourceV2>,
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
    pub(crate) composition: Option<WeightedScaleCompositionBindingV1>,
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

impl PackedScaleInputReceiptV1 {
    pub(crate) fn source_recipe(&self) -> Option<&WeightedScaleFixtureRecipeSourceV2> {
        self.source_recipe.as_ref()
    }
}

/// Materialize one declared bounded input under the selected Native resource
/// envelope. The manifest is written last; failures leave no selectable input.
pub(crate) fn produce_weighted_scale_input_v1(
    request: WeightedScaleProducerRequestV1<'_>,
    before_manifest: &mut dyn FnMut() -> std::io::Result<()>,
) -> std::io::Result<PackedScaleInputReceiptV1> {
    produce_weighted_scale_input_composed_v1(request, None, None, before_manifest)
}

pub(crate) fn produce_weighted_scale_input_with_authored_aux_v1(
    request: WeightedScaleProducerRequestV1<'_>,
    selection: &WeightedScaleAuthoredAuxSelectionV1,
    before_manifest: &mut dyn FnMut() -> std::io::Result<()>,
) -> std::io::Result<PackedScaleInputReceiptV1> {
    produce_weighted_scale_input_composed_v1(request, Some(selection), None, before_manifest)
}

/// Opt-in composition consumes already authenticated selected templates. The
/// caller derives the profile and whole bill from these same rendered shapes.
pub(crate) fn produce_weighted_scale_input_with_authored_aux_and_templates_v1(
    request: WeightedScaleProducerRequestV1<'_>,
    selection: &WeightedScaleAuthoredAuxSelectionV1,
    templates: WeightedScaleTemplateSetV1,
    before_manifest: &mut dyn FnMut() -> std::io::Result<()>,
) -> std::io::Result<PackedScaleInputReceiptV1> {
    if templates.claim_selection.is_none()
        || templates.profile_with_selected_dimensions(request.profile.clone())? != request.profile
    {
        return Err(io_invalid(
            "selected Claim profile dimensions differ before pricing",
        ));
    }
    produce_weighted_scale_input_composed_v1(
        request,
        Some(selection),
        Some(templates),
        before_manifest,
    )
}

fn produce_weighted_scale_input_composed_v1(
    request: WeightedScaleProducerRequestV1<'_>,
    auxiliary: Option<&WeightedScaleAuthoredAuxSelectionV1>,
    selected_templates: Option<WeightedScaleTemplateSetV1>,
    before_manifest: &mut dyn FnMut() -> std::io::Result<()>,
) -> std::io::Result<PackedScaleInputReceiptV1> {
    let profile = request.profile.clone();
    profile.validate()?;
    let base_profile =
        WeightedScaleProfileV1::weighted_for_records(profile.seed, profile.target_records)?;
    let expected_profile = match selected_templates.as_ref() {
        Some(templates) => templates.profile_with_selected_dimensions(base_profile)?,
        None => base_profile,
    };
    if profile != expected_profile {
        return Err(io_invalid(
            "weighted producer profile differs from maintained seed/size ladder",
        ));
    }
    if request.output_root.exists()
        || request.raw_input_root.is_some_and(Path::exists)
        || !request.output_root.is_absolute()
        || request.raw_input_root.is_some_and(|p| !p.is_absolute())
        || request.output_root.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || request.raw_input_root.is_some_and(|p| {
            p.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        })
        || (request.representation == WeightedScaleRepresentationV1::RawAndPackedV1)
            != request.raw_input_root.is_some()
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
        || request.member_tree_limits.max_value_bytes < SCALE_MEMBER_VALUE_BYTES_V1
        || request.member_tree_limits.max_total_bytes == 0
        || request.member_tree_limits.max_total_bytes == u64::MAX
        || request.object_limits.max_working_state_bytes != request.max_working_state_bytes
    {
        return Err(io_invalid("weighted producer limits or destination differ"));
    }
    verify_output_location_outside_repository_v1(request.repository_root, request.output_root)?;
    if let Some(raw_root) = request.raw_input_root {
        verify_output_location_outside_repository_v1(request.repository_root, raw_root)?;
        if raw_root.starts_with(request.output_root) || request.output_root.starts_with(raw_root) {
            return Err(io_invalid("weighted raw and packed output roots overlap"));
        }
    }

    let segment_limits = request
        .segment_limits
        .validate()
        .map_err(|_| io_invalid("weighted producer segment limits are invalid"))?;
    let price = if request.representation == WeightedScaleRepresentationV1::PackedOnlyV2 {
        weighted_scale_composed_producer_envelope_with_templates_v2(
            &profile,
            selected_templates
                .as_ref()
                .ok_or_else(|| io_invalid("packed recipe templates absent"))?,
            auxiliary.ok_or_else(|| io_invalid("packed recipe auxiliary selection absent"))?,
            request.representation,
            request.tree_io.selected_allocation_unit_bytes(),
            request.raw_input_root,
            request.max_working_state_bytes,
            request.caller_live_state_bytes,
        )?
    } else {
        weighted_scale_composition_price_v1(
            &profile,
            auxiliary,
            request.tree_io.selected_allocation_unit_bytes(),
            request
                .raw_input_root
                .ok_or_else(|| io_invalid("raw representation root absent"))?,
            request.max_working_state_bytes,
            request.caller_live_state_bytes,
        )?
    };
    let envelope = price.envelope;
    let mut forecast = price.forecast;
    let total_members = price.member_count;
    let auxiliary_count = price.auxiliary_member_count;
    let auxiliary_bytes = price.auxiliary_source_bytes;
    let required_member_key_bytes = price.required_member_key_bytes;
    let directory_state = price.directory_state_bytes;
    let maximum_source_bytes = envelope.maximum_source_bytes;
    let run_count = sort_run_count_v1(total_members)?;
    let temporary_logical_upper = envelope.temporary_logical_bytes;
    let temporary_allocated_upper = envelope.temporary_allocated_bytes;
    let inode_upper = envelope.temporary_file_inodes;
    let raw_allocation_upper = envelope.raw_input_allocated_bytes;
    let raw_directory_upper = forecast.raw_input_directory_count;
    let member_growth_exceeds_cap = price.maximum_member_bytes > request.max_member_bytes
        || price.maximum_member_bytes > segment_limits.max_frame_bytes;
    let selected_case_exceeds_profile = request.member_tree_limits.max_rows < total_members
        || request.object_limits.tree_limits.max_rows < total_members
        || request.object_limits.max_objects < total_members;
    let member_key_exceeds_cap =
        required_member_key_bytes > request.member_tree_limits.max_key_bytes;
    if maximum_source_bytes > request.max_source_bytes
        || forecast.raw_input_file_count > request.max_raw_input_files
        || raw_directory_upper > request.max_raw_input_directories
        || raw_allocation_upper > request.max_raw_input_allocated_bytes
        || temporary_logical_upper > request.max_temporary_logical_bytes
        || temporary_allocated_upper > request.max_temporary_allocated_bytes
        || inode_upper > request.max_temporary_inodes
        || selected_case_exceeds_profile
        || member_key_exceeds_cap
        || member_growth_exceeds_cap
        || request.object_limits.max_pack_frames == 0
        || request.object_limits.max_pack_frames == u32::MAX
    {
        return Err(weighted_prewrite_refusal_v1(
            &profile,
            &forecast,
            &envelope,
            &request,
            segment_limits.max_frame_bytes,
            required_member_key_bytes,
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
    let mut templates = match selected_templates {
        Some(templates) => templates,
        None => WeightedScaleTemplateSetV1::load_accounted(
            request.repository_root,
            &io_budget,
            request.deadline,
            request.cancelled,
            &request.work,
        )?,
    };
    let producer_state = weighted_producer_state_upper_v1(&templates, run_count)?
        .checked_add(auxiliary.map_or(0, |s| s.state_bytes))
        .and_then(|v| v.checked_add(directory_state.checked_mul(2)?))
        .and_then(|v| {
            v.checked_add(
                usize::try_from(auxiliary.map_or(0, |s| {
                    s.members.iter().map(|m| m.raw_bytes).max().unwrap_or(0)
                }))
                .ok()?,
            )
        })
        .and_then(|v| v.checked_add(size_of::<ComposedScaleMemberIterV1<'_, '_>>()))
        .ok_or_else(|| io_invalid("composed producer state overflow"))?;
    if producer_state
        .checked_add(request.caller_live_state_bytes)
        .is_none_or(|total| total >= request.max_working_state_bytes)
    {
        return Err(io_invalid(
            "weighted producer retained state exceeds selected bound",
        ));
    }

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
    let mut raw_fixture = if let Some(raw_root) = request.raw_input_root {
        let reserved = request
            .tree_io
            .reserve_file_allocation(raw_allocation_upper)
            .map_err(|_| io_invalid("weighted raw input allocation reservation refused"))?;
        match RawScaleFixtureV1::new(
            raw_root,
            reserved,
            total_members,
            raw_directory_upper,
            request.max_raw_input_files,
            request.max_raw_input_directories,
            request.max_raw_input_allocated_bytes,
        ) {
            Ok(raw) => Some(raw),
            Err(error) => {
                let _ = request.tree_io.release_file_allocation(reserved);
                return Err(error);
            }
        }
    } else {
        None
    };
    let mut digest_runs = DigestRunWriterV1::new(SCALE_SORT_RUN_ROWS_V1)?;
    let two_phase = templates.artifact_recipe.is_some();
    if two_phase {
        if raw_fixture.is_some() {
            return Err(io_invalid(
                "Artifact output phases require packed-only representation",
            ));
        }
        let artifact_count = profile.classes[WeightedScaleClassV1::Artifact as usize].count;
        let phase_one_count = artifact_phase_one_member_count_v1(
            profile.target_records,
            artifact_count,
            auxiliary_count,
        )?;
        let immutable_recipe = templates
            .artifact_recipe
            .as_ref()
            .ok_or_else(|| io_invalid("Artifact output recipe absent"))?
            .selection
            .immutable_digest()?;
        let phase_state = request
            .max_working_state_bytes
            .checked_sub(request.caller_live_state_bytes)
            .and_then(|n| n.checked_sub(producer_state))
            .ok_or_else(|| io_invalid("Artifact output phase state exhausted"))?;
        for phase in [ScalePhysicalPhaseV2::NonEvent, ScalePhysicalPhaseV2::Event] {
            let observation = if phase == ScalePhysicalPhaseV2::NonEvent {
                Some(WeightedScaleArtifactOutputPhaseV1::begin(
                    immutable_recipe,
                    profile.seed,
                )?)
            } else {
                None
            };
            let mut observation = observation;
            let baseline = scratch.spool_logical_bytes;
            let expected = if phase == ScalePhysicalPhaseV2::NonEvent {
                phase_one_count
            } else {
                artifact_count
            };
            {
                let mut generated = WeightedScaleMemberIterV1::new(&profile, &templates)?;
                let mut cursor = ComposedScaleMemberIterV1 {
                    generated: &mut generated,
                    auxiliary,
                    auxiliary_index: 0,
                    auxiliary_bytes: 0,
                    support_ordinals: [0; 4],
                    phase,
                    observation: templates.output_observation.as_ref(),
                    member_count: 0,
                    source_bytes: 0,
                    repository_root: request.repository_root,
                    io: &io_budget,
                    work: &request.work,
                    deadline: request.deadline,
                    cancelled: request.cancelled,
                };
                let mut rows = WeightedMemberTreeRowsV1 {
                    cursor: &mut cursor,
                    raw: None,
                    write_payload: true,
                    spool_baseline: baseline,
                    output_phase: observation.as_mut(),
                    available_phase_state: phase_state,
                    spool: &mut spool,
                    digest_runs: &mut digest_runs,
                    scratch: &mut scratch,
                    io: &io_budget,
                    work: &request.work,
                    deadline: request.deadline,
                    cancelled: request.cancelled,
                    expected_members: expected,
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
                while rows.next_entry()?.is_some() {}
                rows.ensure_finished()?;
            }
            if let Some(observation) = observation {
                spool.sync_all()?;
                templates.output_observation = Some(observation.finish(phase_one_count)?);
            }
        }
    }
    let mut generated_cursor = WeightedScaleMemberIterV1::new(&profile, &templates)?;
    let mut member_cursor = ComposedScaleMemberIterV1 {
        generated: &mut generated_cursor,
        auxiliary,
        auxiliary_index: 0,
        auxiliary_bytes: 0,
        support_ordinals: [0; 4],
        phase: ScalePhysicalPhaseV2::All,
        observation: templates.output_observation.as_ref(),
        member_count: 0,
        source_bytes: 0,
        repository_root: request.repository_root,
        io: &io_budget,
        work: &request.work,
        deadline: request.deadline,
        cancelled: request.cancelled,
    };
    let mut member_rows = WeightedMemberTreeRowsV1 {
        cursor: &mut member_cursor,
        raw: raw_fixture.as_mut(),
        write_payload: !two_phase,
        spool_baseline: 0,
        output_phase: None,
        available_phase_state: 0,
        spool: &mut spool,
        digest_runs: &mut digest_runs,
        scratch: &mut scratch,
        io: &io_budget,
        work: &request.work,
        deadline: request.deadline,
        cancelled: request.cancelled,
        expected_members: total_members,
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
    if member_rows.cursor.member_count() != total_members
        || member_rows.cursor.source_bytes() > request.max_source_bytes
        || members_descriptor.entries != total_members
    {
        return Err(io_invalid("weighted member tree census differs"));
    }
    let member_count = member_rows.cursor.member_count();
    let source_bytes = member_rows.cursor.source_bytes();
    let class_source_bytes = member_rows.cursor.class_source_bytes();
    let closure = member_rows.cursor.closure().clone();
    drop(member_rows);
    let raw_input = raw_fixture
        .map(|raw| raw.finish(&request.tree_io))
        .transpose()?;
    if raw_input
        .as_ref()
        .is_some_and(|raw| raw.file_count != member_count || raw.source_bytes != source_bytes)
    {
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
        || object_build_work.object_rows > total_members
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
    let mut profile_bytes = effective_profile_bytes_v1(&profile, base_profile_sha256, &forecast)?;
    if let Some(selection) = &templates.claim_selection {
        let mut value: serde_json::Value = serde_json::from_slice(&profile_bytes)
            .map_err(|_| io_invalid("selected Claim profile decoding failed"))?;
        value["claim_template_selection"] = selection.profile_value();
        profile_bytes = canonical_value_bytes_v1(&value)?;
    }
    if let Some(selection) = completed_artifact_selection_v2(&templates)? {
        let mut value: serde_json::Value = serde_json::from_slice(&profile_bytes)
            .map_err(|_| io_invalid("Artifact profile decoding failed"))?;
        value["artifact_template_selection"] = selection;
        profile_bytes = canonical_value_bytes_v1(&value)?;
    }
    if let Some(selection) = auxiliary {
        let mut value: serde_json::Value = serde_json::from_slice(&profile_bytes)
            .map_err(|_| io_invalid("composed profile decoding failed"))?;
        value["authored_aux_selection"] = serde_json::json!({
            "coverage": "authenticated_byte_composition_pending_semantic_admission",
            "authored_manifest_sha256": selection.authored_manifest_sha256.to_hex(),
            "generated_declaration_sha256": generated_declaration_digest_selected_v2(&profile, &templates)?.to_hex(),
            "auxiliary_members_sha256": auxiliary_members_digest_v1(selection).to_hex(),
            "generated_record_count": profile.target_records,
            "auxiliary_member_count": auxiliary_count,
            "auxiliary_source_bytes": auxiliary_bytes,
        });
        if two_phase {
            value["authored_aux_selection"]["generated_support_member_count"] =
                serde_json::json!(artifact_support_member_count_v1(
                    profile.classes[WeightedScaleClassV1::Artifact as usize].count
                )?);
        }
        profile_bytes = canonical_value_bytes_v1(&value)?;
    }
    if profile_bytes.len() > SCALE_MAX_PROFILE_BYTES_V1 {
        return Err(io_invalid(
            "effective weighted profile exceeds sidecar bound",
        ));
    }
    let profile_sha256 = Digest256::of_bytes(&profile_bytes);

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
    let composition = match auxiliary {
        Some(selection) => Some(WeightedScaleCompositionBindingV1 {
            authored_manifest_sha256: selection.authored_manifest_sha256,
            generated_declaration_sha256: generated_declaration_digest_selected_v2(
                &profile, &templates,
            )?,
            auxiliary_members_sha256: auxiliary_members_digest_v1(selection),
            generated_record_count: profile.target_records,
            generated_support_member_count: if two_phase {
                artifact_support_member_count_v1(
                    profile.classes[WeightedScaleClassV1::Artifact as usize].count,
                )?
            } else {
                0
            },
            auxiliary_member_count: auxiliary_count,
            auxiliary_source_bytes: auxiliary_bytes,
            members_descriptor_sha256,
        }),
        None => None,
    };
    let mut manifest_bytes = scale_input_manifest_bytes_v1(
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
    if let Some(binding) = &composition {
        let mut value: serde_json::Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| io_invalid("composed manifest decoding failed"))?;
        value["authored_aux_composition"] = serde_json::json!({
            "coverage": "authenticated_byte_composition_pending_semantic_admission",
            "authored_manifest_sha256": binding.authored_manifest_sha256.to_hex(),
            "generated_declaration_sha256": binding.generated_declaration_sha256.to_hex(),
            "auxiliary_members_sha256": binding.auxiliary_members_sha256.to_hex(),
            "generated_record_count": binding.generated_record_count,
            "auxiliary_member_count": binding.auxiliary_member_count,
            "auxiliary_source_bytes": binding.auxiliary_source_bytes,
            "members_descriptor_sha256": binding.members_descriptor_sha256.to_hex(),
        });
        if two_phase {
            value["authored_aux_composition"]["generated_support_member_count"] =
                serde_json::json!(binding.generated_support_member_count);
        }
        manifest_bytes = canonical_value_bytes_v1(&value)?;
    }
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
    let raw_input = if let Some(raw) = raw_input {
        let root = request
            .raw_input_root
            .ok_or_else(|| io_invalid("raw receipt root absent"))?;
        verify_private_directory_v1(root, &raw.held_root, raw.root_stamp)?;
        Some(RawScaleInputReceiptV1 {
            raw_input_root: root.to_path_buf(),
            held_raw_input_root: raw.held_root,
            raw_input_file_count: raw.file_count,
            raw_input_directory_count: raw.directory_count,
            raw_input_inode_count: raw
                .file_count
                .checked_add(raw.directory_count)
                .ok_or_else(|| io_invalid("raw source inode count overflow"))?,
            raw_input_source_bytes: raw.source_bytes,
            raw_input_allocated_bytes: raw.allocated_bytes,
        })
    } else {
        None
    };
    Ok(PackedScaleInputReceiptV1 {
        representation: request.representation,
        raw_input,
        source_recipe: templates.fixture_recipe_source.take(),
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
        composition,
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
        for (count, template_bytes) in
            selected_counts
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
        for (count, template_bytes) in
            selected_counts
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
    let directories = raw_fixture_directory_count_v1(profile)?;
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
        member: &ComposedScaleMemberV1,
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
        let ordinal_directory = if member.generated {
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

            ordinal_directory
        } else {
            self.ensure_directories(&components, work, deadline, cancelled)?;
            self.root_path.join(components.join("/"))
        };

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
        if member.generated {
            self.add_directory_allocation(&ordinal_directory)?;
        }
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
    let selection_state = templates
        .claim_selection
        .as_ref()
        .map_or(Ok(0), |s| s.retained_state_bytes())?;
    let artifact_state = templates.artifact_recipe.as_ref().map_or(Ok(0), |r| {
        r.retained_state_bytes()?
            .checked_sub(size_of::<WeightedScaleLoadedArtifactRecipeV1>())
            .ok_or_else(|| io_invalid("Artifact recipe inline state differs"))
    })?;
    let recipe_source_state = templates
        .fixture_recipe_source
        .as_ref()
        .map_or(0, |r| r.source_path.capacity());
    template_array_retained_state_v1(&templates.templates)?
        .checked_add(selection_state)
        .and_then(|bytes| bytes.checked_add(artifact_state))
        .and_then(|bytes| {
            artifact_state
                .checked_mul(4)
                .and_then(|n| bytes.checked_add(n))
        })
        .and_then(|bytes| bytes.checked_add(recipe_source_state))
        .and_then(|bytes| {
            bytes.checked_add(size_of::<WeightedScaleArtifactOutputObservationV1>() + 60)
        })
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
        "schema": "tos_native_scale_effective_profile_v2",
        "status": "working_hypothesis_no_admission",
        "template_source_commit": SCALE_TEMPLATE_SOURCE_COMMIT_V1,
        "base_profile_ref": SCALE_BASE_PROFILE_REF_V1,
        "base_profile_sha256": base_profile_sha256.to_hex(),
        "target_records": profile.target_records,
        "scenario": {
            "name": "weighted_profile_90_9_1_selected_quantiles",
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
        "authentic_bridge_replay_scope": "one real pinned source specimen, separate from the declared synthetic cohort",
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
            "p50_bytes_at_profile_size": forecast.p50_logical_source_bytes,
            "selected_quantile_bytes_at_profile_size": forecast.selected_quantile_scenario_logical_source_bytes,
            "p50_bytes_at_1b": forecast.p50_logical_source_bytes_at_1b,
            "selected_quantile_bytes_at_1b": forecast.selected_quantile_scenario_logical_source_bytes_at_1b,
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
        "authentic_bridge_replay_scope": "one real pinned source specimen, separate from the declared synthetic cohort",
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

struct ComposedScaleMemberV1 {
    path: String,
    digest: Digest256,
    source_bytes: Vec<u8>,
    mode: u32,
    generated: bool,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum ScalePhysicalPhaseV2 {
    All,
    NonEvent,
    Event,
}
struct ComposedScaleMemberIterV1<'a, 'profile> {
    generated: &'a mut WeightedScaleMemberIterV1<'profile>,
    auxiliary: Option<&'a WeightedScaleAuthoredAuxSelectionV1>,
    auxiliary_index: usize,
    auxiliary_bytes: u64,
    support_ordinals: [u64; 4],
    phase: ScalePhysicalPhaseV2,
    observation: Option<&'a WeightedScaleArtifactOutputObservationV1>,
    member_count: u64,
    source_bytes: u64,
    repository_root: &'a Path,
    io: &'a PinnedSqliteIoBudget,
    work: &'a AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl ComposedScaleMemberIterV1<'_, '_> {
    fn member_count(&self) -> u64 {
        self.member_count
    }
    fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    fn class_source_bytes(&self) -> [u64; 5] {
        self.generated.class_source_bytes()
    }
    fn closure(&self) -> &WeightedScaleClosureAccumulatorV1 {
        self.generated.closure()
    }
    fn next_member(&mut self) -> std::io::Result<Option<ComposedScaleMemberV1>> {
        scale_active(self.deadline, self.cancelled)?;
        let next_generated = if self.phase == ScalePhysicalPhaseV2::Event {
            None
        } else {
            self.generated
                .profile
                .classes
                .iter()
                .enumerate()
                .skip(self.generated.class_index)
                .find(|(index, row)| {
                    let ordinal = if *index == self.generated.class_index {
                        self.generated.ordinal
                    } else {
                        0
                    };
                    ordinal < row.count
                })
                .map(|(index, row)| {
                    path_for(
                        row.class,
                        if index == self.generated.class_index {
                            self.generated.ordinal
                        } else {
                            0
                        },
                    )
                })
        };
        let next_aux = if self.phase == ScalePhysicalPhaseV2::Event {
            None
        } else {
            self.auxiliary
                .and_then(|s| s.members.get(self.auxiliary_index))
        };
        let artifact_count =
            self.generated.profile.classes[WeightedScaleClassV1::Artifact as usize].count;
        let next_support = if self.generated.templates.artifact_recipe.is_some() {
            WeightedScaleArtifactSupportRoleV1::ALL
                .into_iter()
                .enumerate()
                .filter(|(index, role)| {
                    self.support_ordinals[*index] < artifact_count
                        && match self.phase {
                            ScalePhysicalPhaseV2::All => true,
                            ScalePhysicalPhaseV2::NonEvent => role.is_phase_one(),
                            ScalePhysicalPhaseV2::Event => !role.is_phase_one(),
                        }
                })
                .map(|(index, role)| (role.path(self.support_ordinals[index]), index, role))
                .min_by(|a, b| a.0.cmp(&b.0))
        } else {
            None
        };
        if next_support.as_ref().is_some_and(|(path, _, _)| {
            next_generated.as_ref() == Some(path) || next_aux.is_some_and(|a| &a.path == path)
        }) {
            return Err(io_invalid("generated support and selected paths collide"));
        }
        let take_support = next_support.as_ref().is_some_and(|(path, _, _)| {
            next_generated.as_ref().is_none_or(|g| path < g)
                && next_aux.is_none_or(|a| path < &a.path)
        });

        if next_aux.is_some_and(|aux| next_generated.as_ref().is_some_and(|g| *g == aux.path)) {
            return Err(io_invalid("composed generated and auxiliary paths collide"));
        }
        let member = if take_support {
            let (path, index, role) =
                next_support.ok_or_else(|| io_invalid("support cursor differs"))?;
            let ordinal = self.support_ordinals[index];
            let mut bytes = Vec::new();
            write_fixture_support_v2(
                &mut bytes,
                self.generated.profile,
                self.generated.templates,
                role,
                ordinal,
                self.observation,
            )?;
            self.support_ordinals[index] = ordinal
                .checked_add(1)
                .ok_or_else(|| io_invalid("support ordinal overflow"))?;
            ComposedScaleMemberV1 {
                path,
                digest: Digest256::of_bytes(&bytes),
                source_bytes: bytes,
                mode: 0o644,
                generated: false,
            }
        } else if next_aux.is_some_and(|aux| next_generated.as_ref().is_none_or(|g| aux.path < *g))
        {
            let aux = next_aux.ok_or_else(|| io_invalid("authored auxiliary cursor differs"))?;
            let bytes = read_authored_aux_member_v1(
                self.repository_root,
                aux,
                self.io,
                self.work,
                self.deadline,
                self.cancelled,
            )?;
            self.auxiliary_index += 1;
            self.auxiliary_bytes = self
                .auxiliary_bytes
                .checked_add(aux.raw_bytes)
                .ok_or_else(|| io_invalid("authored auxiliary observed bytes overflow"))?;
            ComposedScaleMemberV1 {
                path: aux.path.clone(),
                digest: aux.raw_sha256,
                source_bytes: bytes,
                mode: 0o644,
                generated: false,
            }
        } else if let Some(generated) = if self.phase == ScalePhysicalPhaseV2::Event {
            None
        } else {
            self.generated.next_member()?
        } {
            ComposedScaleMemberV1 {
                path: generated.path,
                digest: generated.digest,
                source_bytes: generated.source_bytes,
                mode: generated.mode,
                generated: true,
            }
        } else {
            if self.phase != ScalePhysicalPhaseV2::Event
                && (self.generated.member_count() != self.generated.profile.target_records
                    || self.auxiliary.is_some_and(|s| {
                        self.auxiliary_index != s.members.len()
                            || self.auxiliary_bytes != s.source_bytes
                    }))
            {
                return Err(io_invalid("composed cursor EOF differs"));
            }
            return Ok(None);
        };
        self.member_count = self
            .member_count
            .checked_add(1)
            .ok_or_else(|| io_invalid("composed observed member count overflow"))?;
        self.source_bytes = self
            .source_bytes
            .checked_add(member.source_bytes.len() as u64)
            .ok_or_else(|| io_invalid("composed observed source bytes overflow"))?;
        Ok(Some(member))
    }
}

fn read_authored_aux_member_v1(
    root: &Path,
    member: &WeightedScaleAuthoredAuxMemberV1,
    io: &PinnedSqliteIoBudget,
    work: &AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> std::io::Result<Vec<u8>> {
    // Walk held directory descriptors: a source ancestor replacement cannot
    // turn the selected read into a symlink-following open. Only the current
    // ancestor and its child are live, independent of selected member count.
    let relative =
        RelativePath::parse(&member.path).map_err(|_| io_invalid("auxiliary path differs"))?;
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(root)?;
    let mut components = relative.as_str().split('/').peekable();
    let mut file = loop {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        let part = components
            .next()
            .ok_or_else(|| io_invalid("auxiliary leaf absent"))?;
        let name = std::ffi::CString::new(part).map_err(|_| io_invalid("auxiliary path NUL"))?;
        let leaf = components.peek().is_none();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if leaf {
                libc::O_NONBLOCK
            } else {
                libc::O_DIRECTORY
            };
        let fd = unsafe {
            libc::openat(
                std::os::fd::AsRawFd::as_raw_fd(&directory),
                name.as_ptr(),
                flags,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let opened = unsafe { <File as std::os::fd::FromRawFd>::from_raw_fd(fd) };
        if leaf {
            break opened;
        }
        directory = opened;
    };
    drop(directory);
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() != member.raw_bytes {
        return Err(io_invalid("authored auxiliary source descriptor differs"));
    }
    let size = usize::try_from(member.raw_bytes)
        .map_err(|_| io_invalid("auxiliary bytes exceed address space"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| io_invalid("auxiliary read allocation failed"))?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        let wanted = size
            .checked_sub(bytes.len())
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| io_invalid("auxiliary read bound overflow"))?
            .min(buffer.len());
        io.charge_read(wanted as u64)
            .map_err(|_| io_invalid("auxiliary read permit refused"))?;
        let returned = file.read(&mut buffer[..wanted])?;
        io.record_read_returned(returned as u64)
            .map_err(|_| io_invalid("auxiliary read accounting failed"))?;
        if returned == 0 {
            break;
        }
        if returned > size - bytes.len() {
            return Err(io_invalid("auxiliary source grew while reading"));
        }
        bytes.extend_from_slice(&buffer[..returned]);
    }
    if bytes.len() != size
        || file.metadata()?.len() != member.raw_bytes
        || Digest256::of_bytes(&bytes) != member.raw_sha256
    {
        return Err(io_invalid("authored auxiliary full EOF or SHA differs"));
    }
    Ok(bytes)
}

struct WeightedMemberTreeRowsV1<'a, 'source, 'profile> {
    cursor: &'a mut ComposedScaleMemberIterV1<'source, 'profile>,
    raw: Option<&'a mut RawScaleFixtureV1>,
    write_payload: bool,
    spool_baseline: u64,
    output_phase: Option<&'a mut WeightedScaleArtifactOutputPhaseV1>,
    available_phase_state: usize,
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

impl WeightedMemberTreeRowsV1<'_, '_, '_> {
    fn ensure_finished(&self) -> std::io::Result<()> {
        if !self.finished
            || self.cursor.member_count() != self.expected_members
            || self.cursor.source_bytes()
                != self
                    .scratch
                    .spool_logical_bytes
                    .checked_sub(self.spool_baseline)
                    .ok_or_else(|| io_invalid("weighted spool phase baseline differs"))?
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
            .checked_add(self.spool_baseline)
            .ok_or_else(|| io_invalid("weighted member total overflow"))?;
        if source_total > self.max_source_bytes {
            return Err(io_invalid("weighted source bytes exceed selected cap"));
        }
        if size > self.max_raw_input_file_bytes {
            return Err(io_invalid(
                "weighted raw source member exceeds selected file cap",
            ));
        }
        if let Some(raw) = self.raw.as_deref_mut() {
            raw.write_member(
                &member,
                self.raw_io,
                self.raw_work,
                self.raw_deadline,
                self.raw_cancelled,
            )?;
        }
        if self.write_payload {
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
            // Observe completed writes only. The event phase is rendered from
            // this bounded post-write fold; it cannot issue its own outcome.
            if let Some(phase) = self.output_phase.as_deref_mut() {
                phase.observe_written(
                    &member.path,
                    member.digest,
                    size,
                    self.available_phase_state,
                )?;
            }
        } else if self.output_phase.is_some() || self.raw.is_some() {
            return Err(io_invalid("member-tree replay cannot issue output writes"));
        }
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

impl Iterator for WeightedMemberTreeRowsV1<'_, '_, '_> {
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
    let parent_path = path
        .parent()
        .ok_or_else(|| io_invalid("weighted source parent absent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io_invalid("weighted source filename absent"))?;
    let parent = tos_fd_open::open_absolute_directory(parent_path).map_err(std::io::Error::other)?;
    let parent_before = parent.metadata()?;
    let file = tos_fd_open::open_regular_at(&parent, Path::new(name)).map_err(std::io::Error::other)?;
    let before = file.metadata()?;
    let expected = usize::try_from(before.len())
        .map_err(|_| io_invalid("weighted source size exceeds usize"))?;
    if !before.is_file() || expected > max_bytes {
        return Err(io_invalid(
            "weighted source input is not a bounded regular file",
        ));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected)
        .map_err(|_| io_invalid("weighted source input allocation failed"))?;
    // A real one-byte probe at the selected length proves EOF. Never extend the
    // owned buffer with probe bytes or file growth beyond the selected frame.
    let mut block = [0u8; 64 * 1024];
    loop {
        scale_active(deadline, cancelled)?;
        work.charge_many(1)?;
        let remaining = expected
            .checked_sub(bytes.len())
            .ok_or_else(|| io_invalid("weighted source input size overflow"))?;
        let wanted = if remaining == 0 {
            1
        } else {
            remaining.min(block.len())
        };
        io.charge_read(wanted as u64)
            .map_err(|_| io_invalid("weighted source input read budget exhausted"))?;
        let returned = file.read_at(&mut block[..wanted], bytes.len() as u64);
        // Earlier successful prefixes have already been recorded if this
        // syscall fails; an error itself returns no bytes.
        let count = match returned {
            Ok(count) => count,
            Err(error) => {
                io.record_read_returned(0)
                    .map_err(|_| io_invalid("weighted source input read accounting failed"))?;
                return Err(error);
            }
        };
        io.record_read_returned(count as u64)
            .map_err(|_| io_invalid("weighted source input read accounting failed"))?;
        if count == 0 {
            break;
        }
        if count > remaining {
            return Err(io_invalid(
                "weighted source input grew beyond selected allocation",
            ));
        }
        bytes.extend_from_slice(&block[..count]);
    }
    let stamp = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.uid(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    let after = file.metadata()?;
    let current_parent = tos_fd_open::open_absolute_directory(parent_path).map_err(std::io::Error::other)?;
    let parent_after = current_parent.metadata()?;
    let current = tos_fd_open::open_regular_at(&current_parent, Path::new(name)).map_err(std::io::Error::other)?;
    if bytes.len() != expected
        || stamp(&before) != stamp(&after)
        || stamp(&before) != stamp(&current.metadata()?)
        || (
            parent_before.dev(),
            parent_before.ino(),
            parent_before.mode(),
            parent_before.uid(),
        ) != (
            parent_after.dev(),
            parent_after.ino(),
            parent_after.mode(),
            parent_after.uid(),
        )
    {
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
