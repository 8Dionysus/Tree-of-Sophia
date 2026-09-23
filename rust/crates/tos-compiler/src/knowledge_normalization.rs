//! Exact bounded primitives shared by owner-source knowledge adapters.
//!
//! These reproduce selected helpers in the frozen Python `knowledge.py`.
//! They do not normalize a complete node or relation, admit a source, or
//! publish a graph. In particular, a caller must still derive display,
//! semantics, registry mapping, endpoint titles and owner placeholders.

use crate::{Error, Result};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, io::Write};
use tos_foundation::{Digest256Hasher, JsonLimits, JsonMode, parse_json};

const MAX_SOURCE_ROW_BYTES: usize = 8 * 1024 * 1024;
const MAX_JSON_VISITS: usize = 1_000_000;
const MAX_JSON_DEPTH: usize = 96;
const MAX_INTEGER_DIGITS: usize = 4096;
const DEFAULT_SOURCE_REF: &str = "ToS/source_home.manifest.json";

/// A strict owner carrier parsed under an explicit row-byte ceiling. All
/// methods below operate on this bounded tree; no path is used as identity.
pub struct SourceRow {
    value: Value,
    max_bytes: usize,
}

struct CappedWriter {
    bytes: usize,
    ceiling: usize,
}

impl Write for CappedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len())
            .filter(|next| *next <= self.ceiling)
            .ok_or_else(|| std::io::Error::other("normalization JSON byte ceiling"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn check_serialized(value: &Value, ceiling: usize) -> Result<()> {
    let mut writer = CappedWriter { bytes: 0, ceiling };
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::Budget("normalization JSON bytes"))
}

fn write_len(hasher: &mut Digest256Hasher, len: usize) {
    hasher.update(len.to_string().as_bytes());
}

fn write_string(hasher: &mut Digest256Hasher, value: &str) {
    hasher.update(b"s");
    write_len(hasher, value.len());
    hasher.update(b":");
    hasher.update(value.as_bytes());
}

/// Python `_stable_digest`: sorted Unicode object keys; UTF-8 string length;
/// every JSON number coerced to finite binary64 and emitted as its big-endian
/// hexadecimal bits. Null, boolean, arrays and objects retain distinct tags.
fn stable_digest_value(value: &Value, hasher: &mut Digest256Hasher) -> Result<()> {
    match value {
        Value::Null => hasher.update(b"n;"),
        Value::Bool(boolean) => hasher.update(if *boolean { b"b1;" } else { b"b0;" }),
        Value::Number(number) => {
            let mut float = number
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or(Error::Invalid("non-finite stable digest number"))?;
            if float == 0.0 {
                float = 0.0;
            }
            hasher.update(b"d");
            for byte in float.to_be_bytes() {
                hasher.update(format!("{byte:02x}").as_bytes());
            }
            hasher.update(b";");
        }
        Value::String(string) => write_string(hasher, string),
        Value::Array(items) => {
            hasher.update(b"a");
            write_len(hasher, items.len());
            hasher.update(b"[");
            for item in items {
                stable_digest_value(item, hasher)?;
            }
            hasher.update(b"]");
        }
        Value::Object(items) => {
            hasher.update(b"o");
            write_len(hasher, items.len());
            hasher.update(b"{");
            // `serde_json::Map` is sorted without `preserve_order`, but sort
            // explicitly to keep this contract independent of that feature.
            let mut keys = items.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                write_string(hasher, key);
                stable_digest_value(
                    items
                        .get(key)
                        .ok_or(Error::Invalid("stable digest object key"))?,
                    hasher,
                )?;
            }
            hasher.update(b"}");
        }
    }
    Ok(())
}

fn stable_digest(value: &Value) -> Result<String> {
    let mut hasher = Digest256Hasher::new();
    stable_digest_value(value, &mut hasher)?;
    Ok(hasher.finalize().to_hex())
}

fn string(value: Option<&Value>) -> Option<&str> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn pointer_escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

impl SourceRow {
    pub fn parse(raw: &[u8], max_bytes: usize) -> Result<Self> {
        if max_bytes == 0 || max_bytes > MAX_SOURCE_ROW_BYTES || raw.len() > max_bytes {
            return Err(Error::Budget("normalization source row bytes"));
        }
        let limits = JsonLimits::new(
            max_bytes,
            MAX_JSON_DEPTH,
            MAX_JSON_VISITS,
            MAX_INTEGER_DIGITS,
        )
        .map_err(|_| Error::Budget("normalization JSON limits"))?;
        parse_json(raw, JsonMode::PublishedStrict, limits)
            .map_err(|error| Error::Source(error.to_string()))?;
        let value: Value =
            serde_json::from_slice(raw).map_err(|_| Error::Invalid("normalization source JSON"))?;
        if !value.is_object() {
            return Err(Error::Invalid("normalization source object"));
        }
        Ok(Self { value, max_bytes })
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn stable_digest(&self) -> Result<String> {
        stable_digest(&self.value)
    }

    /// Python `_source_refs`: source_refs string members retain their bytes;
    /// named path fields are trimmed, then all references sort and dedupe.
    pub fn source_refs(&self, fallbacks: &[&str]) -> Vec<String> {
        let mut refs = BTreeSet::new();
        if let Some(values) = self.value.get("source_refs").and_then(Value::as_array) {
            for value in values {
                if let Some(text) = value.as_str().filter(|text| !text.is_empty()) {
                    refs.insert(text.to_owned());
                }
            }
        }
        for key in ["source_ref", "source_path", "path", "owner_surface"] {
            if let Some(text) = string(self.value.get(key)) {
                refs.insert(text.to_owned());
            }
        }
        refs.extend(
            fallbacks
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| (*s).to_owned()),
        );
        if refs.is_empty() {
            refs.insert(DEFAULT_SOURCE_REF.to_owned());
        }
        refs.into_iter().collect()
    }

    /// Python `_source_record` over caller-derived attributes. The payload is
    /// the complete source carrier; field_map points to its original fields.
    pub fn source_record(&self, attributes: &Map<String, Value>) -> Result<Value> {
        let properties = self.value.get("properties").and_then(Value::as_object);
        let mut fields = Map::new();
        for key in attributes.keys() {
            let prefix = if properties.is_some_and(|props| props.contains_key(key)) {
                "/properties/"
            } else {
                "/"
            };
            fields.insert(
                format!("attributes.{key}"),
                Value::String(format!("{prefix}{}", pointer_escape(key))),
            );
        }
        let mut record = Map::new();
        record.insert("payload".into(), self.value.clone());
        record.insert("digest".into(), Value::String(self.stable_digest()?));
        record.insert(
            "transform_version".into(),
            Value::String("tos-knowledge-normalization-v2".into()),
        );
        record.insert("field_map".into(), Value::Object(fields));
        let value = Value::Object(record);
        check_serialized(
            &value,
            self.max_bytes.saturating_mul(4).min(MAX_SOURCE_ROW_BYTES),
        )?;
        Ok(value)
    }
}

/// Python `_stamp_content_revision` over a bounded normalized envelope.
/// Re-stamping excludes the previous revision and restores it on failure.
pub fn stamp_content_revision(value: &mut Value, max_bytes: usize) -> Result<()> {
    if max_bytes == 0 || max_bytes > MAX_SOURCE_ROW_BYTES {
        return Err(Error::Budget("normalization content revision bytes"));
    }
    let object = value
        .as_object_mut()
        .ok_or(Error::Invalid("normalization revision object"))?;
    let previous = object.remove("content_revision");
    let result = (|| {
        check_serialized(value, max_bytes)?;
        stable_digest(value)
    })();
    match result {
        Ok(digest) => {
            let object = value.as_object_mut().expect("checked object");
            object.insert("content_revision".into(), Value::String(digest));
            if let Err(error) = check_serialized(value, max_bytes) {
                let object = value.as_object_mut().expect("checked object");
                object.remove("content_revision");
                if let Some(previous) = previous {
                    object.insert("content_revision".into(), previous);
                }
                return Err(error);
            }
            Ok(())
        }
        Err(error) => {
            if let Some(previous) = previous {
                value
                    .as_object_mut()
                    .expect("checked object")
                    .insert("content_revision".into(), previous);
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Frozen Python oracle: PYTHONPATH=access/src python3, then call
    // tos_access.knowledge._stable_digest/_source_refs/_source_record/
    // _stamp_content_revision on these exact fixture values.
    #[test]
    fn python_oracle_numbers_unicode_and_nested_values() {
        let row = SourceRow::parse(
            r#"{"z":-0.0,"a":[null,true,false,1,1.5,"Élan","a\u2028b"],"obj":{"b":2,"a":"λ"}}"#
                .as_bytes(),
            1024,
        )
        .unwrap();
        assert_eq!(
            row.stable_digest().unwrap(),
            "04690743fdd38d359a6947278fc0359b6fe8a7eed932a2319bc285b7d4723afb"
        );
        assert_eq!(row.source_refs(&[]), vec![DEFAULT_SOURCE_REF]);
    }

    #[test]
    fn python_oracle_source_refs_record_and_revision() {
        let row = SourceRow::parse(
            r#"{"node_id":"tos.work.one","node_kind":"work","label":"Élan","source_ref":" ToS/a.json ","source_refs":["z","a","z",""],"path":" ToS/p.json ","properties":{"a/b":1,"x~y":false}}"#.as_bytes(),
            1024,
        )
        .unwrap();
        assert_eq!(
            row.stable_digest().unwrap(),
            "080c50f6da3ba2ad9c2581b2a364cca9c9de6a8c933bb77914bb76774ee17e50"
        );
        assert_eq!(
            row.source_refs(&[]),
            vec!["ToS/a.json", "ToS/p.json", "a", "z"]
        );
        let attributes = json!({"a/b":1,"x~y":false,"outside":null});
        let record = row.source_record(attributes.as_object().unwrap()).unwrap();
        assert_eq!(record["payload"], *row.value());
        assert_eq!(record["digest"], row.stable_digest().unwrap());
        assert_eq!(
            record["transform_version"],
            "tos-knowledge-normalization-v2"
        );
        assert_eq!(
            record["field_map"],
            json!({
                "attributes.a/b":"/properties/a~1b",
                "attributes.x~y":"/properties/x~0y",
                "attributes.outside":"/outside"
            })
        );
        let mut normalized = json!({"id":"source-navigation:tos.work.one","source_record":record});
        stamp_content_revision(&mut normalized, 2048).unwrap();
        assert_eq!(
            normalized["content_revision"],
            "15894278e3a77618affe4bdff66dfee15a3cdd38f02a95a206fb973e4166b8b6"
        );
    }

    #[test]
    fn strict_source_and_budgets_fail_closed() {
        assert!(SourceRow::parse(br#"{"x":1,"x":2}"#, 1024).is_err());
        assert!(SourceRow::parse(br#"{"x":1}"#, 4).is_err());
        assert!(SourceRow::parse(br#"[]"#, 1024).is_err());
        let row = SourceRow::parse(br#"{"source_refs":["a","a"]}"#, 1024).unwrap();
        assert_eq!(row.source_refs(&["b", "a"]), vec!["a", "b"]);
        let mut normalized = json!({"a":"more than tiny"});
        assert!(stamp_content_revision(&mut normalized, 2).is_err());
        assert!(normalized.get("content_revision").is_none());
    }
}
