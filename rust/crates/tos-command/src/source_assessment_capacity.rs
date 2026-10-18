//! Pure capacity transition shared by authenticated history readers and writers.
//! This module authenticates no bytes and grants no authority. Production callers
//! construct history state only after normal chain/schema/identity validation.

pub(crate) const MAX_EVENTS: usize = 1024;
pub(crate) const MAX_BATCH_BYTES: usize = 1_048_576;
pub(crate) const MAX_HISTORY_BYTES: usize = 33_554_432;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    batch_bytes: usize,
    history_bytes: usize,
    batches: usize,
    events: usize,
}

impl Limits {
    pub(crate) const V1: Self = Self {
        batch_bytes: MAX_BATCH_BYTES,
        history_bytes: MAX_HISTORY_BYTES,
        batches: MAX_EVENTS,
        events: MAX_EVENTS,
    };
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StorageEnvelope {
    Local,
    PgPackage,
}

impl StorageEnvelope {
    fn new_head_bytes(self) -> usize {
        match self {
            // Local history excludes the mutable head; operation input does not.
            Self::Local => 0,
            // The maintained PG writer adds a 64-byte hexadecimal head, no LF.
            // Existing raw package heads remain in observed storage_bytes.
            Self::PgPackage => 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapacityRefusal {
    BatchBytes,
    HistoryBytes,
    Batches,
    Events,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryCapacity {
    storage_bytes: usize,
    canonical_bytes: usize,
    batches: usize,
    events: usize,
}

impl HistoryCapacity {
    pub(crate) const EMPTY: Self = Self {
        storage_bytes: 0,
        canonical_bytes: 0,
        batches: 0,
        events: 0,
    };

    /// Counts are observations from the existing authenticated reader, not a
    /// persisted checkpoint. Raw/canonical/input byte units are never inferred
    /// from one another. Input-byte charging remains with the operation owner.
    pub(crate) fn observed(
        storage_bytes: usize,
        canonical_bytes: usize,
        batches: usize,
        events: usize,
        limits: Limits,
    ) -> Result<Self, CapacityRefusal> {
        let state = Self {
            storage_bytes,
            canonical_bytes,
            batches,
            events,
        };
        state.check(limits)?;
        Ok(state)
    }

    fn check_bytes(self, limits: Limits) -> Result<(), CapacityRefusal> {
        if self.storage_bytes > limits.history_bytes || self.canonical_bytes > limits.history_bytes
        {
            return Err(CapacityRefusal::HistoryBytes);
        }
        Ok(())
    }

    fn check(self, limits: Limits) -> Result<(), CapacityRefusal> {
        self.check_bytes(limits)?;
        if self.batches > limits.batches {
            return Err(CapacityRefusal::Batches);
        }
        if self.events > limits.events {
            return Err(CapacityRefusal::Events);
        }
        Ok(())
    }

    /// Admission for a NEW batch only. Command replay, expected revision,
    /// qualification, dedup and no-new-events are resolved by common prepare
    /// before this method; it cannot create an append or bypass those decisions.
    /// The returned state obeys the same predicate as observed reader state.
    pub(crate) fn append(
        self,
        payload_bytes: usize,
        committed_events: usize,
        envelope: StorageEnvelope,
        limits: Limits,
    ) -> Result<Self, CapacityRefusal> {
        if payload_bytes > limits.batch_bytes {
            return Err(CapacityRefusal::BatchBytes);
        }
        self.check(limits)?;
        let storage_bytes = self
            .storage_bytes
            .checked_add(payload_bytes)
            .and_then(|n| n.checked_add(envelope.new_head_bytes()))
            .ok_or(CapacityRefusal::HistoryBytes)?;
        let canonical_bytes = self
            .canonical_bytes
            .checked_add(payload_bytes)
            .ok_or(CapacityRefusal::HistoryBytes)?;
        // Preserve byte-before-count refusal priority, including combined
        // failures in parameterized limits. This is the reader's byte predicate.
        Self {
            storage_bytes,
            canonical_bytes,
            ..self
        }
        .check_bytes(limits)?;
        let batches = self
            .batches
            .checked_add(1)
            .filter(|n| *n <= limits.batches)
            .ok_or(CapacityRefusal::Batches)?;
        let events = self
            .events
            .checked_add(committed_events)
            .ok_or(CapacityRefusal::Events)?;
        Self::observed(storage_bytes, canonical_bytes, batches, events, limits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(history: usize) -> Limits {
        Limits {
            batch_bytes: 16,
            history_bytes: history,
            batches: 4,
            events: 6,
        }
    }

    #[test]
    fn parameterized_local_minus_exact_plus_and_reader_predicate() {
        for ceiling in [64, 128, 256] {
            let cap = limits(ceiling);
            let before = HistoryCapacity::observed(ceiling - 8, ceiling - 8, 2, 3, cap).unwrap();
            for payload in [7, 8] {
                let next = before
                    .append(payload, 1, StorageEnvelope::Local, cap)
                    .unwrap();
                assert!(next.check(cap).is_ok());
                assert_eq!(next.storage_bytes, ceiling - 8 + payload);
            }
            assert_eq!(
                before.append(9, 1, StorageEnvelope::Local, cap),
                Err(CapacityRefusal::HistoryBytes)
            );
            assert_eq!(before.storage_bytes, ceiling - 8);
        }
    }

    #[test]
    fn pg_new_head_and_all_old_raw_heads_are_charged() {
        let cap = limits(128);
        let exact = HistoryCapacity::observed(56, 8, 2, 3, cap).unwrap();
        let next = exact.append(8, 1, StorageEnvelope::PgPackage, cap).unwrap();
        assert_eq!((next.storage_bytes, next.canonical_bytes), (128, 16));
        let plus = HistoryCapacity::observed(57, 8, 2, 3, cap).unwrap();
        assert_eq!(
            plus.append(8, 1, StorageEnvelope::PgPackage, cap),
            Err(CapacityRefusal::HistoryBytes)
        );
    }

    #[test]
    fn canonical_and_raw_predicates_are_independent() {
        let cap = limits(64);
        let before = HistoryCapacity::observed(40, 60, 1, 1, cap).unwrap();
        assert_eq!(
            before.append(5, 1, StorageEnvelope::Local, cap),
            Err(CapacityRefusal::HistoryBytes)
        );
        assert_eq!(
            HistoryCapacity::observed(65, 0, 0, 0, cap),
            Err(CapacityRefusal::HistoryBytes)
        );
        assert_eq!(
            HistoryCapacity::observed(0, 65, 0, 0, cap),
            Err(CapacityRefusal::HistoryBytes)
        );
    }

    #[test]
    fn batch_limit_is_inclusive_and_independent() {
        let cap = limits(256);
        assert!(
            HistoryCapacity::EMPTY
                .append(16, 1, StorageEnvelope::Local, cap)
                .is_ok()
        );
        assert_eq!(
            HistoryCapacity::EMPTY.append(17, 1, StorageEnvelope::Local, cap),
            Err(CapacityRefusal::BatchBytes)
        );
    }

    #[test]
    fn batch_and_committed_event_counts_are_independent() {
        let cap = limits(256);
        let exact = HistoryCapacity::observed(0, 0, 3, 5, cap).unwrap();
        let next = exact.append(1, 1, StorageEnvelope::Local, cap).unwrap();
        assert_eq!((next.batches, next.events), (4, 6));
        assert_eq!(
            next.append(1, 1, StorageEnvelope::Local, cap),
            Err(CapacityRefusal::Batches)
        );
        let events_full = HistoryCapacity::observed(0, 0, 1, 6, cap).unwrap();
        assert_eq!(
            events_full.append(1, 1, StorageEnvelope::Local, cap),
            Err(CapacityRefusal::Events)
        );
    }

    #[test]
    fn caller_supplied_committed_event_count_is_charged() {
        let cap = limits(256);
        let before = HistoryCapacity::observed(0, 0, 1, 5, cap).unwrap();
        assert!(before.append(1, 1, StorageEnvelope::Local, cap).is_ok());
        assert_eq!(
            before.append(1, 2, StorageEnvelope::Local, cap),
            Err(CapacityRefusal::Events)
        );
    }

    #[test]
    fn arithmetic_overflow_is_refused_without_mutation() {
        let cap = Limits {
            batch_bytes: usize::MAX,
            history_bytes: usize::MAX,
            batches: usize::MAX,
            events: usize::MAX,
        };
        for (before, envelope, error) in [
            (
                HistoryCapacity {
                    storage_bytes: usize::MAX,
                    ..HistoryCapacity::EMPTY
                },
                StorageEnvelope::Local,
                CapacityRefusal::HistoryBytes,
            ),
            (
                HistoryCapacity {
                    storage_bytes: usize::MAX - 1,
                    ..HistoryCapacity::EMPTY
                },
                StorageEnvelope::PgPackage,
                CapacityRefusal::HistoryBytes,
            ),
            (
                HistoryCapacity {
                    canonical_bytes: usize::MAX,
                    ..HistoryCapacity::EMPTY
                },
                StorageEnvelope::Local,
                CapacityRefusal::HistoryBytes,
            ),
            (
                HistoryCapacity {
                    batches: usize::MAX,
                    ..HistoryCapacity::EMPTY
                },
                StorageEnvelope::Local,
                CapacityRefusal::Batches,
            ),
            (
                HistoryCapacity {
                    events: usize::MAX,
                    ..HistoryCapacity::EMPTY
                },
                StorageEnvelope::Local,
                CapacityRefusal::Events,
            ),
        ] {
            let saved = before;
            assert_eq!(before.append(1, 1, envelope, cap), Err(error));
            assert_eq!(before, saved);
        }
    }

    #[test]
    fn combined_failures_preserve_batch_bytes_history_bytes_batches_events_priority() {
        let cap = Limits {
            batch_bytes: usize::MAX,
            history_bytes: 64,
            batches: usize::MAX,
            events: usize::MAX,
        };
        for (storage, canonical) in [(64, 0), (0, 64)] {
            let before =
                HistoryCapacity::observed(storage, canonical, usize::MAX, usize::MAX, cap).unwrap();
            assert_eq!(
                before.append(1, 1, StorageEnvelope::Local, cap),
                Err(CapacityRefusal::HistoryBytes),
            );
        }
        let cap = Limits {
            history_bytes: usize::MAX,
            batches: 4,
            ..cap
        };
        let before = HistoryCapacity::observed(0, 0, 4, usize::MAX, cap).unwrap();
        assert_eq!(
            before.append(1, 1, StorageEnvelope::Local, cap),
            Err(CapacityRefusal::Batches),
        );
        let cap = Limits {
            batch_bytes: 0,
            history_bytes: 0,
            ..cap
        };
        assert_eq!(
            HistoryCapacity::EMPTY.append(1, 1, StorageEnvelope::Local, cap),
            Err(CapacityRefusal::BatchBytes),
        );
    }

    #[test]
    fn production_v1_constants_remain_exact() {
        assert_eq!(
            (MAX_BATCH_BYTES, MAX_HISTORY_BYTES, MAX_EVENTS),
            (1_048_576, 33_554_432, 1024)
        );
    }
}
