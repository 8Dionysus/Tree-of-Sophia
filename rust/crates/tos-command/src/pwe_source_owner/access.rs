//! Borrowed comparisons and lookup admission. No source/identity authority.
use super::budget::CanonicalLedger;
use super::{invalid, unsupported};
use crate::source_command::SourceCommandResult;
use serde_json::Value;
use std::collections::BTreeMap;
use tos_foundation::JsonValue;

pub(super) trait AccessMeter {
    fn inspect_access(&mut self, bytes: usize, visits: usize) -> SourceCommandResult<()>;
    fn reserve_access(&mut self, bytes: usize) -> SourceCommandResult<()>;
}
impl AccessMeter for CanonicalLedger<'_> {
    fn inspect_access(&mut self, bytes: usize, visits: usize) -> SourceCommandResult<()> {
        CanonicalLedger::inspect_access(self, bytes, visits)
    }
    fn reserve_access(&mut self, bytes: usize) -> SourceCommandResult<()> {
        CanonicalLedger::reserve_access(self, bytes)
    }
}
/// Only pure borrowed-access operations are generic. HeldFrames always binds
/// them to the concrete original CanonicalLedger; no authority is generic.
pub(super) trait BorrowedAccess: AccessMeter {
    fn bytes_equal(&mut self, left: &[u8], right: &[u8]) -> SourceCommandResult<bool> {
        let bytes = left
            .len()
            .checked_add(right.len())
            .ok_or(unsupported("PWE comparison size overflow"))?;
        self.inspect_access(bytes, 1)?;
        Ok(left == right)
    }
    fn text_equal(&mut self, left: &str, right: &str) -> SourceCommandResult<bool> {
        self.bytes_equal(left.as_bytes(), right.as_bytes())
    }
    fn string_map_get<'v, T>(
        &mut self,
        map: &'v BTreeMap<String, T>,
        key: &str,
    ) -> SourceCommandResult<Option<&'v T>> {
        self.inspect_access(0, 1)?;
        for (candidate, value) in map {
            if self.text_equal(candidate, key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
    fn native_get<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> SourceCommandResult<Option<&'v JsonValue>> {
        self.inspect_access(0, 1)?;
        let Some(entries) = value.as_object() else {
            return Ok(None);
        };
        for (candidate, value) in entries {
            self.inspect_access(0, 1)?;
            if let Some(candidate) = candidate.as_str() {
                if self.text_equal(candidate, key)? {
                    self.inspect_native_number(value)?;
                    return Ok(Some(value));
                }
            }
        }
        Ok(None)
    }
    fn inspect_native_number(&mut self, value: &JsonValue) -> SourceCommandResult<()> {
        if let JsonValue::Number(number) = value {
            self.inspect_access(number.lexeme.len(), 1)?;
        }
        Ok(())
    }
    fn exact(&mut self, value: &Value) -> SourceCommandResult<tos_foundation::ExactRecordRef> {
        self.inspect_access(0, 1)?;
        if value.as_object().map(|o| o.len()) != Some(3) {
            return Err(invalid("PWE exact reference shape"));
        }
        let id = self
            .get(value, "id")?
            .and_then(Value::as_str)
            .ok_or(invalid("PWE exact ID"))?;
        self.inspect_access(
            id.len()
                .checked_mul(2)
                .ok_or(unsupported("PWE exact ID work"))?,
            1,
        )?;
        self.reserve_access(
            id.len()
                .checked_add(128)
                .ok_or(unsupported("PWE exact holder size"))?,
        )?;
        let id = tos_foundation::StableId::parse(id).map_err(|_| invalid("PWE exact ID"))?;
        let version = self
            .get(value, "version")?
            .and_then(Value::as_u64)
            .ok_or(invalid("PWE exact version"))?;
        let version = tos_foundation::RecordVersion::new(version)
            .map_err(|_| invalid("PWE exact version"))?;
        let digest = self
            .get(value, "digest")?
            .and_then(Value::as_str)
            .ok_or(invalid("PWE exact digest"))?;
        self.inspect_access(digest.len(), 1)?;
        let digest = tos_foundation::Digest256::from_prefixed(digest)
            .map_err(|_| invalid("PWE exact digest"))?;
        Ok(tos_foundation::ExactRecordRef {
            id,
            version,
            digest,
        })
    }
    fn exact_equal(
        &mut self,
        left: &tos_foundation::ExactRecordRef,
        right: &tos_foundation::ExactRecordRef,
    ) -> SourceCommandResult<bool> {
        let names = self.text_equal(left.id.as_str(), right.id.as_str())?;
        Ok(names && left.version == right.version && left.digest == right.digest)
    }
    fn reference_matches(
        &mut self,
        value: &Value,
        reference: tos_compiler::pwe_native::PweRef<'_>,
    ) -> SourceCommandResult<bool> {
        let actual = self.exact(value)?;
        let names = self.text_equal(actual.id.as_str(), reference.id())?;
        Ok(names
            && actual.version.get() == reference.version()
            && actual.digest == reference.digest())
    }
    fn native_field<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> SourceCommandResult<&'v JsonValue> {
        self.native_get(value, key)?
            .ok_or(invalid("PWE required native field absent"))
    }
    fn native_text<'v>(&mut self, value: &'v JsonValue, key: &str) -> SourceCommandResult<&'v str> {
        self.native_field(value, key)?
            .as_str()
            .ok_or(invalid("PWE native text field"))
    }
    fn native_array<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> SourceCommandResult<&'v [JsonValue]> {
        self.native_field(value, key)?
            .as_array()
            .ok_or(invalid("PWE native array field"))
    }
    fn get<'v>(&mut self, value: &'v Value, key: &str) -> SourceCommandResult<Option<&'v Value>> {
        self.inspect_access(0, 1)?;
        let Some(entries) = value.as_object() else {
            return Ok(None);
        };
        for (candidate, value) in entries {
            if self.text_equal(candidate, key)? {
                if let Value::Number(number) = value {
                    self.inspect_access(number.as_str().len(), 1)?;
                }
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
    fn field<'v>(&mut self, value: &'v Value, key: &str) -> SourceCommandResult<&'v Value> {
        self.get(value, key)?
            .ok_or(invalid("PWE required value field absent"))
    }
    fn text_is(&mut self, value: &Value, key: &str, expected: &str) -> SourceCommandResult<bool> {
        match self.get(value, key)?.and_then(Value::as_str) {
            Some(actual) => self.text_equal(actual, expected),
            None => Ok(false),
        }
    }
    fn optional_equal(
        &mut self,
        left: Option<&Value>,
        right: Option<&Value>,
    ) -> SourceCommandResult<bool> {
        match (left, right) {
            (Some(left), Some(right)) => self.equal(left, right),
            (None, None) => {
                self.inspect_access(0, 1)?;
                Ok(true)
            }
            _ => {
                self.inspect_access(0, 1)?;
                Ok(false)
            }
        }
    }
    fn equal(&mut self, left: &Value, right: &Value) -> SourceCommandResult<bool> {
        let mut visits = 0;
        self.equal_at(left, right, 0, &mut visits)
    }
    fn equal_at(
        &mut self,
        left: &Value,
        right: &Value,
        depth: usize,
        visits: &mut usize,
    ) -> SourceCommandResult<bool> {
        self.inspect_access(0, 1)?;
        if depth > 96 || *visits >= 1_000_000 {
            return Err(unsupported("PWE comparison visitor limit"));
        }
        *visits += 1;
        match (left, right) {
            (Value::Null, Value::Null) => Ok(true),
            (Value::Bool(left), Value::Bool(right)) => Ok(left == right),
            (Value::Number(left), Value::Number(right)) => {
                self.inspect_access(
                    left.as_str()
                        .len()
                        .checked_add(right.as_str().len())
                        .ok_or(unsupported("PWE numeric comparison size"))?,
                    1,
                )?;
                Ok(left == right)
            }
            (Value::String(left), Value::String(right)) => self.text_equal(left, right),
            (Value::Array(left), Value::Array(right)) => {
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (left, right) in left.iter().zip(right) {
                    if !self.equal_at(left, right, depth + 1, visits)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Value::Object(left), Value::Object(right)) => {
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (key, left) in left {
                    let mut found = None;
                    for (candidate, right) in right {
                        if self.text_equal(key, candidate)? {
                            found = Some(right);
                            break;
                        }
                    }
                    let Some(right) = found else {
                        return Ok(false);
                    };
                    if !self.equal_at(left, right, depth + 1, visits)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn contains(&mut self, values: &[Value], target: &Value) -> SourceCommandResult<bool> {
        for value in values {
            if self.equal(value, target)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn pointer<'v>(
        &mut self,
        value: &'v Value,
        pointer: &str,
    ) -> SourceCommandResult<Option<&'v Value>> {
        self.inspect_access(pointer.len(), 1)?;
        if pointer.is_empty() {
            return Ok(Some(value));
        }
        let Some(pointer) = pointer.strip_prefix('/') else {
            return Ok(None);
        };
        let mut value = value;
        for (depth, token) in pointer.split('/').enumerate() {
            if depth >= 96 {
                return Err(unsupported("PWE pointer depth"));
            }
            let cost = token
                .len()
                .checked_mul(4)
                .ok_or(unsupported("PWE pointer decoding size"))?;
            self.inspect_access(cost, 1)?;
            self.reserve_access(
                cost.checked_add(2 * std::mem::size_of::<String>())
                    .ok_or(unsupported("PWE pointer holder size"))?,
            )?;
            let token = token.replace("~1", "/").replace("~0", "~");
            value = match value {
                Value::Object(_) => match self.get(value, &token)? {
                    Some(value) => value,
                    None => return Ok(None),
                },
                Value::Array(values) => {
                    self.inspect_access(token.len(), 1)?;
                    if token.starts_with('+') || token.len() > 1 && token.starts_with('0') {
                        return Ok(None);
                    }
                    let Ok(index) = token.parse::<usize>() else {
                        return Ok(None);
                    };
                    let Some(value) = values.get(index) else {
                        return Ok(None);
                    };
                    value
                }
                _ => return Ok(None),
            };
        }
        Ok(Some(value))
    }
}

impl<T: AccessMeter> BorrowedAccess for T {}

#[cfg(test)]
mod tests {
    use super::*;
    // Numerical admission fixture only. No native frames, grants, profiles,
    // source records, schema results, assessment or coverage are constructed.
    struct NumericMeter {
        work: usize,
        bytes: usize,
        live: usize,
    }
    impl AccessMeter for NumericMeter {
        fn inspect_access(&mut self, bytes: usize, visits: usize) -> SourceCommandResult<()> {
            self.work = self
                .work
                .checked_sub(
                    bytes
                        .checked_add(visits)
                        .ok_or(unsupported("test work overflow"))?,
                )
                .ok_or(unsupported("test work exhausted"))?;
            self.bytes = self
                .bytes
                .checked_sub(bytes)
                .ok_or(unsupported("test bytes exhausted"))?;
            Ok(())
        }
        fn reserve_access(&mut self, bytes: usize) -> SourceCommandResult<()> {
            self.live = self
                .live
                .checked_sub(bytes)
                .ok_or(unsupported("test live exhausted"))?;
            Ok(())
        }
    }
    #[test]
    fn borrowed_access_preserves_values_and_spent_prefix() {
        let mut meter = NumericMeter {
            work: 100_000,
            bytes: 100_000,
            live: 100_000,
        };
        let left = serde_json::json!({"k":[1,null],"a/b":{"~k":"Ж"}});
        let right = serde_json::json!({"a/b":{"~k":"Ж"},"k":[1,null]});
        assert_eq!(meter.equal(&left, &right).unwrap(), left == right);
        assert_eq!(
            meter.pointer(&left, "/a~1b/~0k").unwrap(),
            left.pointer("/a~1b/~0k")
        );
        assert_eq!(
            meter.pointer(&left, "/k/01").unwrap(),
            left.pointer("/k/01")
        );
        assert!(meter.get(&left, "missing").unwrap().is_none());
        assert_eq!(meter.pointer(&left, "/k/1").unwrap(), Some(&Value::Null));
        let map = BTreeMap::from([("ToS/a".to_owned(), b"same".to_vec())]);
        assert_eq!(
            meter.string_map_get(&map, "ToS/a").unwrap(),
            map.get("ToS/a")
        );
        assert!(meter.bytes_equal(b"same", b"same").unwrap());
        assert!(!meter.bytes_equal(b"same", b"diff").unwrap());
        assert!(meter.work < 100_000 && meter.bytes < 100_000);
        let native = crate::source_command::parse(br#"{"k":"v"}"#).unwrap();
        assert_eq!(
            meter.native_get(&native, "k").unwrap(),
            native.object_get("k")
        );
        let mut exhausted = NumericMeter {
            work: 1,
            bytes: 100,
            live: 100,
        };
        assert!(exhausted.native_get(&native, "k").is_err());
        assert_eq!(exhausted.work, 0); // root prefix spent before key refusal
        assert!(exhausted.native_get(&native, "k").is_err());
        assert_eq!(exhausted.work, 0); // retry cannot refill the same meter
    }
}
