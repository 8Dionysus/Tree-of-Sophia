//! Bounded full-reference comparison for independently sourced placement rows.
//! The caller must provide the authoritative committed-history stream and the
//! candidate index stream separately. Equal streams are mechanical evidence,
//! not a source admission, predicate-coverage or rights certificate.

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::placement::PlacementV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacementAuditRow {
    /// Exact source/CMD-defined logical lookup key; this module never parses it.
    pub key: Vec<u8>,
    pub placement: PlacementV1,
}

#[derive(Clone, Copy, Debug)]
pub struct PlacementAuditLimits {
    pub max_rows: u64,
    pub max_key_bytes: usize,
    pub max_total_key_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacementComparison {
    pub rows: u64,
    pub stream_digest: Digest256,
}

/// Stream-compare all exact placements in sorted raw-byte key order. Memory
/// retained here is at most two caller-bounded rows, independent of corpus
/// cardinality. This reference audit does not run per warm request/commit.
pub fn compare_placement_streams<E, A>(
    mut expected: E,
    mut actual: A,
    limits: PlacementAuditLimits,
) -> Result<PlacementComparison>
where
    E: Iterator<Item = Result<PlacementAuditRow>>,
    A: Iterator<Item = Result<PlacementAuditRow>>,
{
    if limits.max_rows == 0
        || limits.max_rows == u64::MAX
        || limits.max_key_bytes == 0
        || limits.max_key_bytes == usize::MAX
        || limits.max_key_bytes > u32::MAX as usize
        || limits.max_total_key_bytes == 0
        || limits.max_total_key_bytes == u64::MAX
    {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "invalid placement audit limits",
        ));
    }
    let mut count = 0u64;
    let mut key_bytes = 0u64;
    let mut prior_key: Option<Vec<u8>> = None;
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-placement-comparison-v1");
    loop {
        let left = expected.next().transpose()?;
        let right = actual.next().transpose()?;
        match (left, right) {
            (None, None) => break,
            (Some(left), Some(right)) => {
                if left.key.is_empty()
                    || left.key.len() > limits.max_key_bytes
                    || right.key.is_empty()
                    || right.key.len() > limits.max_key_bytes
                    || prior_key.as_ref().is_some_and(|prior| left.key <= *prior)
                    || prior_key.as_ref().is_some_and(|prior| right.key <= *prior)
                {
                    return Err(SegmentError::new(
                        Code::InvalidFormat,
                        "placement stream order or key bounds differ",
                    ));
                }
                if left != right {
                    return Err(SegmentError::new(
                        Code::InvalidReceipt,
                        "committed and indexed placements differ",
                    ));
                }
                count = count.checked_add(1).ok_or_else(|| {
                    SegmentError::new(Code::BudgetExceeded, "placement audit row count overflow")
                })?;
                key_bytes = key_bytes
                    .checked_add(left.key.len() as u64)
                    .ok_or_else(|| {
                        SegmentError::new(
                            Code::BudgetExceeded,
                            "placement audit key bytes overflow",
                        )
                    })?;
                if count > limits.max_rows || key_bytes > limits.max_total_key_bytes {
                    return Err(SegmentError::new(
                        Code::BudgetExceeded,
                        "placement audit budget exceeded",
                    ));
                }
                hasher.update(&(left.key.len() as u32).to_le_bytes());
                hasher.update(&left.key);
                hasher.update(&left.placement.encode());
                prior_key = Some(left.key);
            }
            _ => {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "placement stream membership differs",
                ));
            }
        }
    }
    hasher.update(&count.to_le_bytes());
    Ok(PlacementComparison {
        rows: count,
        stream_digest: hasher.finalize(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::FrameCoordinate;

    fn placement(frame: u32) -> PlacementV1 {
        let digest = Digest256::of_bytes(b"bytes");
        PlacementV1 {
            store_id: [1; 16],
            domain_digest: digest,
            pin_id: [2; 16],
            fence_epoch: 1,
            receipt_id: Digest256::of_bytes(&frame.to_le_bytes()),
            segment_digest: digest,
            segment_size: 200,
            frame_index: frame,
            coordinate: FrameCoordinate {
                header_offset: 48,
                size_bytes: 5,
                sha256: digest,
            },
        }
    }

    fn row(key: &[u8], frame: u32) -> PlacementAuditRow {
        PlacementAuditRow {
            key: key.to_vec(),
            placement: placement(frame),
        }
    }

    fn input(rows: Vec<PlacementAuditRow>) -> impl Iterator<Item = Result<PlacementAuditRow>> {
        rows.into_iter().map(Ok)
    }

    fn limits() -> PlacementAuditLimits {
        PlacementAuditLimits {
            max_rows: 4,
            max_key_bytes: 8,
            max_total_key_bytes: 16,
        }
    }

    #[test]
    fn independent_stream_comparison_detects_missing_changed_and_duplicate_rows() {
        let expected = vec![row(b"a", 0), row(b"b", 1), row(b"c", 2)];
        let exact =
            compare_placement_streams(input(expected.clone()), input(expected.clone()), limits())
                .unwrap();
        assert_eq!(exact.rows, 3);
        assert_eq!(
            compare_placement_streams(
                input(expected.clone()),
                input(vec![row(b"a", 0), row(b"c", 2)]),
                limits()
            )
            .unwrap_err()
            .code,
            Code::InvalidReceipt
        );
        assert_eq!(
            compare_placement_streams(
                input(expected.clone()),
                input(vec![row(b"a", 0), row(b"b", 9), row(b"c", 2)]),
                limits()
            )
            .unwrap_err()
            .code,
            Code::InvalidReceipt
        );
        assert_eq!(
            compare_placement_streams(
                input(expected.clone()),
                input(vec![row(b"a", 0), row(b"a", 0), row(b"c", 2)]),
                limits()
            )
            .unwrap_err()
            .code,
            Code::InvalidFormat
        );
        assert_eq!(
            compare_placement_streams(
                input(expected.clone()),
                input(expected),
                PlacementAuditLimits {
                    max_rows: 2,
                    ..limits()
                }
            )
            .unwrap_err()
            .code,
            Code::BudgetExceeded
        );
    }
}
