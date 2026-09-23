//! Geometry and exact-row commitments for a candidate placement generation.
//! This module proves internal shape only. CMD must independently supply every
//! committed history row at the selected cut; VAL owns predicate coverage.

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::placement::PlacementV1;

const LEAF_TAG: &[u8] = b"tos-placement-leaf-v1";
const CATALOG_TAG: &[u8] = b"tos-placement-catalog-v1";
const MIN_ROW_BYTES: u64 = 4 + 1 + 32 + 8 + PlacementV1::ENCODED_BYTES as u64;
const LEAF_OVERHEAD_BYTES: u64 = LEAF_TAG.len() as u64 + 32 + 16;

fn empty_leaf_digest(domain_digest: Digest256) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    hasher.update(LEAF_TAG);
    hasher.update(domain_digest.as_bytes());
    hasher.update(&0u64.to_le_bytes());
    hasher.update(&LEAF_OVERHEAD_BYTES.to_le_bytes());
    hasher.finalize()
}

/// Raw unsigned byte order. The source/CMD key codec must be injective and
/// named by a separate digest; storage never interprets a key as a path/type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyComparatorV1 {
    RawUnsignedBytes,
}

/// Half-open `[lower, upper)` interval. `None` is the respective infinity.
/// A zero-row partition still occupies its exact interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionBoundsV1 {
    pub lower_inclusive: Option<Vec<u8>>,
    pub upper_exclusive: Option<Vec<u8>>,
}

impl PartitionBoundsV1 {
    pub(crate) fn validate(&self, max_key_bytes: usize) -> Result<()> {
        for endpoint in [&self.lower_inclusive, &self.upper_exclusive] {
            if endpoint
                .as_ref()
                .is_some_and(|key| key.is_empty() || key.len() > max_key_bytes)
            {
                return Err(SegmentError::new(
                    Code::InvalidFormat,
                    "partition endpoint invalid",
                ));
            }
        }
        if let (Some(lower), Some(upper)) = (&self.lower_inclusive, &self.upper_exclusive) {
            if lower >= upper {
                return Err(SegmentError::new(
                    Code::InvalidFormat,
                    "partition interval empty or reversed",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn contains(&self, key: &[u8]) -> bool {
        self.lower_inclusive
            .as_ref()
            .is_none_or(|lower| key >= lower.as_slice())
            && self
                .upper_exclusive
                .as_ref()
                .is_none_or(|upper| key < upper.as_slice())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacementGenerationRowV1 {
    /// Exact injectively encoded CMD logical history key, opaque to STO.
    pub key: Vec<u8>,
    pub logical_digest: Digest256,
    pub logical_length: u64,
    pub placement: PlacementV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacementPartitionV1 {
    pub bounds: PartitionBoundsV1,
    pub rows: u64,
    pub first_key: Option<Vec<u8>>,
    pub last_key: Option<Vec<u8>>,
    /// Canonical leaf size including tag and count/length trailer.
    pub leaf_bytes: u64,
    pub leaf_digest: Digest256,
}

#[derive(Clone, Copy, Debug)]
pub struct GenerationShapeLimits {
    pub max_partitions: usize,
    pub max_rows_per_partition: u64,
    pub max_key_bytes: usize,
    pub max_leaf_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationStreamComparison {
    pub rows: u64,
    pub stream_digest: Digest256,
}

/// Merge an independently exhaustive committed-history stream with the
/// candidate indexed stream. Equality does not prove the caller supplied an
/// exhaustive history stream, source authorization or predicate coverage.
pub fn compare_generation_streams<E, A>(
    domain_digest: Digest256,
    mut expected: E,
    mut indexed: A,
    max_rows: u64,
    max_key_bytes: usize,
    max_total_key_bytes: u64,
) -> Result<GenerationStreamComparison>
where
    E: Iterator<Item = Result<PlacementGenerationRowV1>>,
    A: Iterator<Item = Result<PlacementGenerationRowV1>>,
{
    if max_rows == 0
        || max_rows == u64::MAX
        || max_key_bytes == 0
        || max_key_bytes > u32::MAX as usize
        || max_total_key_bytes == 0
        || max_total_key_bytes == u64::MAX
    {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "invalid generation comparison limits",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-generation-comparison-v1");
    hasher.update(domain_digest.as_bytes());
    let mut rows = 0u64;
    let mut total_key_bytes = 0u64;
    let mut previous: Option<Vec<u8>> = None;
    loop {
        let left = expected.next().transpose()?;
        let right = indexed.next().transpose()?;
        match (left, right) {
            (None, None) => break,
            (Some(left), Some(right)) => {
                validate_row(&left, domain_digest, max_key_bytes, previous.as_deref())?;
                validate_row(&right, domain_digest, max_key_bytes, previous.as_deref())?;
                if left != right {
                    return Err(SegmentError::new(
                        Code::InvalidReceipt,
                        "committed and indexed generation rows differ",
                    ));
                }
                rows = rows.checked_add(1).ok_or_else(|| {
                    SegmentError::new(Code::BudgetExceeded, "generation row count overflow")
                })?;
                total_key_bytes = total_key_bytes
                    .checked_add(left.key.len() as u64)
                    .ok_or_else(|| {
                        SegmentError::new(
                            Code::BudgetExceeded,
                            "generation key byte count overflow",
                        )
                    })?;
                if rows > max_rows || total_key_bytes > max_total_key_bytes {
                    return Err(SegmentError::new(
                        Code::BudgetExceeded,
                        "generation comparison budget exceeded",
                    ));
                }
                hash_row(&mut hasher, &left);
                previous = Some(left.key);
            }
            _ => {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "generation stream membership differs",
                ));
            }
        }
    }
    hasher.update(&rows.to_le_bytes());
    Ok(GenerationStreamComparison {
        rows,
        stream_digest: hasher.finalize(),
    })
}

fn validate_row(
    row: &PlacementGenerationRowV1,
    domain_digest: Digest256,
    max_key_bytes: usize,
    previous: Option<&[u8]>,
) -> Result<()> {
    if row.key.is_empty()
        || row.key.len() > max_key_bytes
        || previous.is_some_and(|key| row.key.as_slice() <= key)
        || row.logical_length != row.placement.coordinate().size_bytes
        || row.logical_digest != row.placement.coordinate().sha256
        || row.placement.domain_digest() != domain_digest
    {
        return Err(SegmentError::new(
            Code::InvalidFormat,
            "generation row key/order/bytes differ",
        ));
    }
    Ok(())
}

fn hash_row(hasher: &mut Digest256Hasher, row: &PlacementGenerationRowV1) {
    hasher.update(&(row.key.len() as u32).to_le_bytes());
    hasher.update(&row.key);
    hasher.update(row.logical_digest.as_bytes());
    hasher.update(&row.logical_length.to_le_bytes());
    hasher.update(&row.placement.encode());
}

impl GenerationShapeLimits {
    pub(crate) fn validate(self) -> Result<Self> {
        if self.max_partitions == 0
            || self.max_partitions > u32::MAX as usize
            || self.max_rows_per_partition == 0
            || self.max_rows_per_partition == u64::MAX
            || self.max_key_bytes == 0
            || self.max_key_bytes > u32::MAX as usize
            || self.max_leaf_bytes == 0
            || self.max_leaf_bytes == u64::MAX
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "invalid generation shape limits",
            ));
        }
        Ok(self)
    }
}

/// Commit an exact ordered candidate leaf. The returned digest is mechanical
/// evidence for these supplied rows, not proof that CMD supplied all history.
/// Persistent leaf installation and full independent comparison remain separate.
pub fn describe_placement_partition<I>(
    domain_digest: Digest256,
    bounds: PartitionBoundsV1,
    rows: I,
    limits: GenerationShapeLimits,
) -> Result<PlacementPartitionV1>
where
    I: IntoIterator<Item = Result<PlacementGenerationRowV1>>,
{
    let limits = limits.validate()?;
    bounds.validate(limits.max_key_bytes)?;
    let mut hasher = Digest256Hasher::new();
    hasher.update(LEAF_TAG);
    hasher.update(domain_digest.as_bytes());
    let mut count = 0u64;
    let mut bytes = LEAF_OVERHEAD_BYTES;
    let mut first_key = None;
    let mut last_key: Option<Vec<u8>> = None;
    for row in rows {
        let row = row?;
        validate_row(
            &row,
            domain_digest,
            limits.max_key_bytes,
            last_key.as_deref(),
        )?;
        if !bounds.contains(&row.key) {
            return Err(SegmentError::new(
                Code::InvalidFormat,
                "placement leaf row outside partition",
            ));
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "leaf row count overflow"))?;
        bytes = bytes
            .checked_add(4 + row.key.len() as u64 + 32 + 8 + PlacementV1::ENCODED_BYTES as u64)
            .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "leaf byte count overflow"))?;
        if count > limits.max_rows_per_partition || bytes > limits.max_leaf_bytes {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "placement leaf budget exceeded",
            ));
        }
        hash_row(&mut hasher, &row);
        if first_key.is_none() {
            first_key = Some(row.key.clone());
        }
        last_key = Some(row.key);
    }
    hasher.update(&count.to_le_bytes());
    hasher.update(&bytes.to_le_bytes());
    if bytes > limits.max_leaf_bytes {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "placement leaf budget exceeded",
        ));
    }
    Ok(PlacementPartitionV1 {
        bounds,
        rows: count,
        first_key,
        last_key,
        leaf_bytes: bytes,
        leaf_digest: hasher.finalize(),
    })
}

/// Validate interval geometry and bind every descriptor into one candidate
/// root. The root cannot answer an absence until real leaf bytes, the complete
/// CMD history stream and the selected cut have been independently verified.
pub fn placement_catalog_shape_root(
    domain: &[u8],
    namespace: &[u8],
    scope: &[u8],
    key_codec_digest: Digest256,
    comparator: KeyComparatorV1,
    partitions: &[PlacementPartitionV1],
    limits: GenerationShapeLimits,
) -> Result<Digest256> {
    let limits = limits.validate()?;
    if domain.is_empty()
        || namespace.is_empty()
        || scope.is_empty()
        || partitions.is_empty()
        || partitions.len() > limits.max_partitions
        || domain.len() > u16::MAX as usize
        || namespace.len() > u16::MAX as usize
        || scope.len() > u16::MAX as usize
    {
        return Err(SegmentError::new(
            Code::InvalidFormat,
            "catalog identity or partition count invalid",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    hasher.update(CATALOG_TAG);
    let domain_digest = Digest256::of_bytes(domain);
    for part in [domain, namespace, scope] {
        hasher.update(&(part.len() as u16).to_le_bytes());
        hasher.update(part);
    }
    hasher.update(key_codec_digest.as_bytes());
    hasher.update(&[match comparator {
        KeyComparatorV1::RawUnsignedBytes => 1,
    }]);
    hasher.update(&(partitions.len() as u32).to_le_bytes());
    let mut previous_upper: Option<Vec<u8>> = None;
    for (index, partition) in partitions.iter().enumerate() {
        partition.bounds.validate(limits.max_key_bytes)?;
        if (index == 0 && partition.bounds.lower_inclusive.is_some())
            || (index > 0 && previous_upper.as_ref() != partition.bounds.lower_inclusive.as_ref())
            || (index > 0 && previous_upper.is_none())
            || (index + 1 == partitions.len() && partition.bounds.upper_exclusive.is_some())
            || (index + 1 < partitions.len() && partition.bounds.upper_exclusive.is_none())
            || partition.rows > limits.max_rows_per_partition
            || partition.leaf_bytes > limits.max_leaf_bytes
            || (partition.rows == 0
                && (partition.first_key.is_some()
                    || partition.last_key.is_some()
                    || partition.leaf_bytes != LEAF_OVERHEAD_BYTES
                    || partition.leaf_digest != empty_leaf_digest(domain_digest)))
            || (partition.rows > 0
                && (partition.first_key.is_none()
                    || partition.last_key.is_none()
                    || partition.leaf_bytes
                        < LEAF_OVERHEAD_BYTES
                            .saturating_add(partition.rows.saturating_mul(MIN_ROW_BYTES))))
        {
            return Err(SegmentError::new(
                Code::InvalidFormat,
                "catalog partition coverage differs",
            ));
        }
        if let (Some(first), Some(last)) = (&partition.first_key, &partition.last_key) {
            if first > last
                || (partition.rows > 1 && first == last)
                || first.len() > limits.max_key_bytes
                || last.len() > limits.max_key_bytes
                || !partition.bounds.contains(first)
                || !partition.bounds.contains(last)
            {
                return Err(SegmentError::new(
                    Code::InvalidFormat,
                    "catalog partition key bounds differ",
                ));
            }
        }
        for endpoint in [
            &partition.bounds.lower_inclusive,
            &partition.bounds.upper_exclusive,
            &partition.first_key,
            &partition.last_key,
        ] {
            match endpoint {
                None => hasher.update(&[0]),
                Some(key) => {
                    hasher.update(&[1]);
                    hasher.update(&(key.len() as u32).to_le_bytes());
                    hasher.update(key);
                }
            }
        }
        hasher.update(&partition.rows.to_le_bytes());
        hasher.update(&partition.leaf_bytes.to_le_bytes());
        hasher.update(partition.leaf_digest.as_bytes());
        previous_upper = partition.bounds.upper_exclusive.clone();
    }
    Ok(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::FrameCoordinate;

    fn limits() -> GenerationShapeLimits {
        GenerationShapeLimits {
            max_partitions: 4,
            max_rows_per_partition: 8,
            max_key_bytes: 16,
            max_leaf_bytes: 4096,
        }
    }

    fn domain_digest() -> Digest256 {
        Digest256::of_bytes(b"domain")
    }

    fn row(key: &[u8], frame: u32) -> PlacementGenerationRowV1 {
        let digest = Digest256::of_bytes(key);
        PlacementGenerationRowV1 {
            key: key.to_vec(),
            logical_digest: digest,
            logical_length: key.len() as u64,
            placement: PlacementV1 {
                store_id: [1; 16],
                domain_digest: Digest256::of_bytes(b"domain"),
                pin_id: [2; 16],
                fence_epoch: 1,
                receipt_id: Digest256::of_bytes(&frame.to_le_bytes()),
                segment_digest: Digest256::of_bytes(b"segment"),
                segment_size: 512,
                frame_index: frame,
                coordinate: FrameCoordinate {
                    header_offset: 48,
                    size_bytes: key.len() as u64,
                    sha256: digest,
                },
            },
        }
    }

    fn input(
        rows: Vec<PlacementGenerationRowV1>,
    ) -> impl Iterator<Item = Result<PlacementGenerationRowV1>> {
        rows.into_iter().map(Ok)
    }

    fn root(parts: &[PlacementPartitionV1]) -> Result<Digest256> {
        placement_catalog_shape_root(
            b"domain",
            b"history",
            b"all",
            Digest256::of_bytes(b"cmd-key-codec-v1"),
            KeyComparatorV1::RawUnsignedBytes,
            parts,
            limits(),
        )
    }

    #[test]
    fn explicit_empty_tail_covers_negative_route_without_claiming_absence() {
        let first = describe_placement_partition(
            domain_digest(),
            PartitionBoundsV1 {
                lower_inclusive: None,
                upper_exclusive: Some(b"m".to_vec()),
            },
            input(vec![row(b"a", 0), row(b"c", 1)]),
            limits(),
        )
        .unwrap();
        let empty = describe_placement_partition(
            domain_digest(),
            PartitionBoundsV1 {
                lower_inclusive: Some(b"m".to_vec()),
                upper_exclusive: None,
            },
            input(vec![]),
            limits(),
        )
        .unwrap();
        let parts = [first, empty];
        assert_ne!(root(&parts).unwrap(), Digest256::of_bytes(b"unrelated"));
        assert!(parts[1].bounds.contains(b"z"));
        assert!(parts[1].bounds.contains(b"m"));
        assert!(parts[0].bounds.contains(b"a"));
        assert_eq!(root(&parts[..1]).unwrap_err().code, Code::InvalidFormat);
        let mut false_empty = parts[1].clone();
        false_empty.leaf_digest = Digest256::of_bytes(b"invented empty leaf");
        assert_eq!(
            root(&[parts[0].clone(), false_empty]).unwrap_err().code,
            Code::InvalidFormat
        );
    }

    #[test]
    fn gap_overlap_endpoint_and_row_mutation_refuse_or_change_root() {
        let left = describe_placement_partition(
            domain_digest(),
            PartitionBoundsV1 {
                lower_inclusive: None,
                upper_exclusive: Some(b"m".to_vec()),
            },
            input(vec![row(b"a", 0)]),
            limits(),
        )
        .unwrap();
        let right = describe_placement_partition(
            domain_digest(),
            PartitionBoundsV1 {
                lower_inclusive: Some(b"m".to_vec()),
                upper_exclusive: None,
            },
            input(vec![row(b"m", 1)]),
            limits(),
        )
        .unwrap();
        let exact = root(&[left.clone(), right.clone()]).unwrap();
        let substituted = describe_placement_partition(
            domain_digest(),
            left.bounds.clone(),
            input(vec![row(b"a", 9)]),
            limits(),
        )
        .unwrap();
        assert_eq!(substituted.rows, left.rows);
        assert_ne!(root(&[substituted, right.clone()]).unwrap(), exact);
        for wrong_boundary in [b"l".to_vec(), b"n".to_vec()] {
            let mut wrong = right.clone();
            wrong.bounds.lower_inclusive = Some(wrong_boundary);
            assert_eq!(
                root(&[left.clone(), wrong]).unwrap_err().code,
                Code::InvalidFormat
            );
        }
        let mut changed = right.clone();
        changed.leaf_digest = Digest256::of_bytes(b"different leaf");
        assert_ne!(root(&[left.clone(), changed]).unwrap(), exact);
        assert_eq!(
            describe_placement_partition(
                domain_digest(),
                left.bounds.clone(),
                input(vec![row(b"m", 0)]),
                limits()
            )
            .unwrap_err()
            .code,
            Code::InvalidFormat
        );
        assert_eq!(
            describe_placement_partition(
                domain_digest(),
                left.bounds,
                input(vec![row(b"a", 0), row(b"a", 1)]),
                limits()
            )
            .unwrap_err()
            .code,
            Code::InvalidFormat
        );
        let mut foreign = row(b"a", 0);
        foreign.placement.domain_digest = Digest256::of_bytes(b"other-domain");
        assert_eq!(
            describe_placement_partition(
                domain_digest(),
                PartitionBoundsV1 {
                    lower_inclusive: None,
                    upper_exclusive: None,
                },
                input(vec![foreign]),
                limits(),
            )
            .unwrap_err()
            .code,
            Code::InvalidFormat
        );
    }

    #[test]
    fn exact_generation_comparison_detects_omission_and_same_count_substitution() {
        let committed = vec![row(b"a", 0), row(b"b", 1), row(b"c", 2)];
        let exact = compare_generation_streams(
            domain_digest(),
            input(committed.clone()),
            input(committed.clone()),
            4,
            16,
            32,
        )
        .unwrap();
        assert_eq!(exact.rows, 3);
        assert_eq!(
            compare_generation_streams(
                domain_digest(),
                input(committed.clone()),
                input(vec![row(b"a", 0), row(b"c", 2)]),
                4,
                16,
                32,
            )
            .unwrap_err()
            .code,
            Code::InvalidReceipt
        );
        assert_eq!(
            compare_generation_streams(
                domain_digest(),
                input(committed),
                input(vec![row(b"a", 0), row(b"b", 9), row(b"c", 2)]),
                4,
                16,
                32,
            )
            .unwrap_err()
            .code,
            Code::InvalidReceipt
        );
    }
}
