//! Bounded JSON and state-accounting kernels shared by native cut adapters
//! and the portable validation profile. These functions do not read a source
//! store or grant source admission.

use crate::item_rules::ItemRefusal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub(crate) fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source("bibliography cancelled".into()));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}
pub(crate) fn decoded_wire_size(
    value: &serde_json::Value,
    limit: usize,
) -> Result<usize, ItemRefusal> {
    serialized_wire_size(limit, |writer| serde_json::to_writer(writer, value))
}
pub(crate) fn serialized_wire_size(
    limit: usize,
    write: impl FnOnce(&mut dyn std::io::Write) -> serde_json::Result<()>,
) -> Result<usize, ItemRefusal> {
    struct Counter {
        bytes: usize,
        limit: usize,
        exhausted: bool,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self.bytes.checked_add(bytes.len());
            if next.is_none_or(|n| n > self.limit) {
                self.exhausted = true;
                return Err(std::io::Error::other("logical serialization budget"));
            }
            self.bytes = next.unwrap();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter {
        bytes: 0,
        limit,
        exhausted: false,
    };
    let outcome = write(&mut counter);
    if counter.exhausted {
        return Err(ItemRefusal::BudgetCheck {
            check: "decoded JSON serialization bytes",
            used: None,
            limit: Some(limit as u64),
        });
    }
    outcome.map_err(|_| ItemRefusal::Unsupported("decoded JSON serialization".into()))?;
    Ok(counter.bytes)
}

// Logical state counts Rust value slots and their retained string/byte payloads.
// BTree/HashMap allocator nodes, buckets, alignment and allocator rounding are
// deliberately not claimed as RSS. Codec byte/depth/visit limits bound parsing
// separately; callers must count each simultaneously retained representation.
pub(crate) fn decoded_state(value: &serde_json::Value) -> Result<usize, ItemRefusal> {
    fn heap(value: &serde_json::Value) -> Option<usize> {
        use serde_json::Value;
        match value {
            Value::Number(n) => Some(n.as_str().len()),
            Value::String(s) => Some(s.len()),
            Value::Array(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<Value>())?,
                |sum, item| sum.checked_add(heap(item)?),
            ),
            Value::Object(items) => items.iter().try_fold(
                items
                    .len()
                    .checked_mul(std::mem::size_of::<(String, Value)>())?,
                |sum, (key, item)| sum.checked_add(key.len())?.checked_add(heap(item)?),
            ),
            _ => Some(0),
        }
    }
    std::mem::size_of::<serde_json::Value>()
        .checked_add(heap(value).ok_or(crate::item_budget_origin!())?)
        .ok_or(crate::item_budget_origin!())
}
pub(crate) fn ordered_state(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
    fn string(s: &tos_foundation::JsonString) -> Option<usize> {
        s.units()
            .len()
            .checked_mul(std::mem::size_of::<u16>())?
            .checked_add(s.as_str().map_or(0, str::len))
    }
    fn heap(value: &tos_foundation::JsonValue) -> Option<usize> {
        use tos_foundation::JsonValue;
        match value {
            JsonValue::Number(n) => Some(n.lexeme.len()),
            JsonValue::String(s) => string(s),
            JsonValue::Array(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<JsonValue>())?,
                |sum, item| sum.checked_add(heap(item)?),
            ),
            JsonValue::Object(items) => items.iter().try_fold(
                items
                    .len()
                    .checked_mul(std::mem::size_of::<(tos_foundation::JsonString, JsonValue)>())?,
                |sum, (key, item)| sum.checked_add(string(key)?)?.checked_add(heap(item)?),
            ),
            _ => Some(0),
        }
    }
    std::mem::size_of::<tos_foundation::JsonValue>()
        .checked_add(heap(value).ok_or(crate::item_budget_origin!())?)
        .ok_or(crate::item_budget_origin!())
}
// Peak logical strict-parser tree plus duplicate-key index slots/payloads.
// Ancestor object indexes can coexist; summing the actual object indexes is a
// structural upper bound, independent of corpus size or serialized multipliers.
pub(crate) fn ordered_codec_state(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
    fn indexes(value: &tos_foundation::JsonValue) -> Option<usize> {
        use tos_foundation::JsonValue;
        match value {
            JsonValue::Object(items) => items.iter().try_fold(
                std::mem::size_of::<std::collections::HashMap<Vec<u16>, usize>>(),
                |n, (key, value)| {
                    n.checked_add(std::mem::size_of::<(Vec<u16>, usize)>())?
                        .checked_add(key.units().len().checked_mul(std::mem::size_of::<u16>())?)?
                        .checked_add(indexes(value)?)
                },
            ),
            JsonValue::Array(items) => items
                .iter()
                .try_fold(0usize, |n, v| n.checked_add(indexes(v)?)),
            _ => Some(0),
        }
    }
    ordered_state(value)?
        .checked_add(indexes(value).ok_or(crate::item_budget_origin!())?)
        .ok_or(crate::item_budget_origin!())
}
// Canonical emission keeps borrowed duplicate-key and sorted-entry indexes.
// Nested object indexes can coexist; this prices their logical slots, without
// cloning keys or interpreting allocator buckets as retained payloads.
pub(crate) fn ordered_emit_state(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
    use tos_foundation::{JsonString, JsonValue};
    fn indexes(value: &JsonValue) -> Option<usize> {
        match value {
            JsonValue::Object(items) => items.iter().try_fold(
                std::mem::size_of::<std::collections::HashSet<&Vec<u16>>>()
                    + std::mem::size_of::<Vec<&(JsonString, JsonValue)>>()
                    + items.len().checked_mul(
                        std::mem::size_of::<&Vec<u16>>()
                            + std::mem::size_of::<&(JsonString, JsonValue)>(),
                    )?,
                |n, (_, value)| n.checked_add(indexes(value)?),
            ),
            JsonValue::Array(items) => items
                .iter()
                .try_fold(0usize, |n, value| n.checked_add(indexes(value)?)),
            _ => Some(0),
        }
    }
    indexes(value).ok_or(crate::item_budget_origin!())
}
pub(crate) fn bounded_ordered(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<tos_foundation::JsonValue, ItemRefusal> {
    bounded_ordered_mode(
        raw,
        limits,
        available,
        deadline,
        cancelled,
        tos_foundation::JsonMode::PublishedStrict,
        false,
        false,
    )
}
fn bounded_ordered_mode(
    raw: &[u8],
    mut limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    mode: tos_foundation::JsonMode,
    syntax_as_source: bool,
    incremental_state: bool,
) -> Result<tos_foundation::JsonValue, ItemRefusal> {
    check(deadline, cancelled)?;
    // During Foundation parsing a decoded key may retain UTF16 both in the
    // ordered tree and the duplicate-key index, plus its cached UTF8. Their
    // total lengths cannot exceed two UTF16 copies and one UTF8 copy of input.
    // Each value visit can own one value slot, one key, one index entry and one
    // object index header. These are logical slots, not hash bucket/RSS bounds.
    if !incremental_state {
        let strings = raw
            .len()
            .checked_mul(2 * std::mem::size_of::<u16>() + std::mem::size_of::<u8>())
            .ok_or(crate::item_budget_origin!())?;
        let slot = std::mem::size_of::<tos_foundation::JsonValue>()
            + std::mem::size_of::<tos_foundation::JsonString>()
            + std::mem::size_of::<(Vec<u16>, usize)>()
            + std::mem::size_of::<std::collections::HashMap<Vec<u16>, usize>>();
        let remaining = available
            .checked_sub(strings)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "strict JSON logical string workspace",
                used: Some(strings as u64),
                limit: Some(available as u64),
            })?;
        limits.max_visits = limits.max_visits.min(remaining / slot);
        if limits.max_visits == 0 {
            return Err(ItemRefusal::BudgetCheck {
                check: "strict JSON logical node workspace",
                used: Some(slot as u64),
                limit: Some(remaining as u64),
            });
        }
    }
    let parsed = if incremental_state {
        tos_foundation::parse_json_with_state_budget(raw, mode, limits, available)
    } else {
        tos_foundation::parse_json(raw, mode, limits)
    };
    let result = parsed
        .map_err(|e| {
            if e.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                if incremental_state && e.detail == "JSON parser state budget exceeded" {
                    ItemRefusal::BudgetCheck {
                        check: "Item JSON parser workspace",
                        used: None,
                        limit: Some(available as u64),
                    }
                } else {
                    ItemRefusal::BudgetCheck {
                        check: "strict JSON codec bytes/depth/visits/integer",
                        used: None,
                        limit: None,
                    }
                }
            } else if syntax_as_source
                && matches!(
                    e.code,
                    tos_foundation::FoundationErrorCode::InvalidUtf8
                        | tos_foundation::FoundationErrorCode::InvalidJson
                        | tos_foundation::FoundationErrorCode::InvalidUnicodeScalar
                        | tos_foundation::FoundationErrorCode::InvalidNumber
                        | tos_foundation::FoundationErrorCode::NonfiniteFloat
                )
            {
                ItemRefusal::Source("invalid finite native JSON".into())
            } else {
                ItemRefusal::Unsupported(format!("strict JSON: {e:?}"))
            }
        })?
        .into_root();
    check(deadline, cancelled)?;
    let state = ordered_state(&result)?;
    if state > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "strict JSON retained ordered state",
            used: Some(state as u64),
            limit: Some(available as u64),
        });
    }
    Ok(result)
}
// Existing legacy JSON consumers keep last-key-wins; this bounds the same
// codec workspace without imposing PublishedStrict duplicate-key semantics.
// Integer text was bounded only by the member bytes in that serde route.
pub(crate) fn bounded_legacy_decoded_state(
    raw: &[u8],
    max_bytes: usize,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    let limits =
        tos_foundation::JsonLimits::new(max_bytes, 128, available.max(1), max_bytes.max(1))
            .map_err(|_| crate::item_budget_origin!())?;
    bounded_legacy_decoded_state_inner(raw, limits, available, deadline, cancelled, false, false)
}
// The named source-layer caller has the original native decoded-field JSON
// profile: malformed finite JSON is an issue, while a valid value outside
// serde's representable scalar strings remains explicitly unsupported.
pub(crate) fn bounded_legacy_decoded_state_with_limits(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    bounded_legacy_decoded_state_inner(raw, limits, available, deadline, cancelled, true, false)
}
// Only the actual Item legacy route uses incremental parser workspace.
// Other native/selected-layer profiles preserve their existing admission.
pub(crate) fn bounded_legacy_item_decoded_state(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    bounded_legacy_decoded_state_inner(raw, limits, available, deadline, cancelled, true, true)
}
fn bounded_legacy_decoded_state_inner(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    malformed_as_source: bool,
    incremental_state: bool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    drop(bounded_ordered_mode(
        raw,
        limits,
        available,
        deadline,
        cancelled,
        tos_foundation::JsonMode::RequestLastWins,
        malformed_as_source,
        incremental_state,
    )?);
    let value = serde_json::from_slice(raw)
        .map_err(|_| ItemRefusal::Unsupported("decoded JSON representation".into()))?;
    let state = decoded_state(&value)?;
    if state > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "legacy JSON retained decoded state",
            used: Some(state as u64),
            limit: Some(available as u64),
        });
    }
    check(deadline, cancelled)?;
    Ok((value, state))
}
pub(crate) fn bounded_decoded_state(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    // Strict validation and its duplicate-key index finish before decoding;
    // there is no simultaneous retained Foundation and serde tree here.
    drop(bounded_ordered(
        raw, limits, available, deadline, cancelled,
    )?);
    let value = serde_json::from_slice(raw)
        .map_err(|_| ItemRefusal::Unsupported("decoded JSON representation".into()))?;
    let state = decoded_state(&value)?;
    if state > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "strict JSON retained decoded state",
            used: Some(state as u64),
            limit: Some(available as u64),
        });
    }
    check(deadline, cancelled)?;
    Ok((value, state))
}
