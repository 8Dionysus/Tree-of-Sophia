//! Strict compatibility reader for the current partitioned projection carrier.
//! Production STO.2 pins use a separate adapter to NavigationInput.

use crate::{Collection, Error, Limits, NavigationInput, Result, SourceBinding, safe_open};
use flate2::read::GzDecoder;
use serde_json::Value;
use std::{
    cell::Cell,
    io::Read,
    path::{Path, PathBuf},
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};

pub(crate) const ROOT_CAP: usize = 256 * 1024;
pub(crate) const INDEX_CAP: usize = 128 * 1024;
pub(crate) const PART_CAP: usize = 8 * 1024 * 1024;
pub(crate) const STORED_OVERHEAD: usize = 65536;

/// Original pinned rust_backend decoder allocation geometry. This is the
/// flate2 1.1.10/miniz_oxide 0.9.1 shipped dependency profile; backend feature
/// changes require this owner forecast to change with them.
pub(crate) fn partition_decoder_workspace_upper(kind: &str) -> Result<usize> {
    if kind == "index" {
        return Ok(0);
    }
    if kind != "data" {
        return Err(Error::Invalid("part kind"));
    }
    // Miniz InflateState's Box plus its construction frame; gzip read buffer
    // is exactly 32KiB. Two optional filename/comment Vecs grow to <=65536
    // with old/new reallocation overlap, extra is an exact u16-length Vec.
    let backend = std::mem::size_of::<miniz_oxide::inflate::stream::InflateState>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(32 * 1024))
        .and_then(|n| n.checked_add(2 * 3 * 65536 + 65535))
        .and_then(|n| n.checked_add(std::mem::size_of::<GzDecoder<&[u8]>>()))
        .ok_or(Error::Budget("partition decoder workspace"))?;
    Ok(backend)
}

/// Shared physical part decoder for existing filesystem navigation and exact
/// captured corpus inputs. Namespace and member selection stay with each owner.
pub fn decode_partition_part(
    stored: &[u8],
    kind: &str,
    stored_len: usize,
    decoded_len: usize,
    stored_sha: &str,
    decoded_sha: &str,
) -> Result<Vec<u8>> {
    let cap = match kind {
        "data" => PART_CAP,
        "index" => INDEX_CAP,
        _ => return Err(Error::Invalid("part kind")),
    };
    if stored_len > cap + STORED_OVERHEAD || decoded_len > cap {
        return Err(Error::Budget("part bytes"));
    }
    if stored.len() != stored_len || Digest256::of_bytes(stored).to_hex() != stored_sha {
        return Err(Error::Invalid("part digest mismatch"));
    }
    let raw = if kind == "data" {
        let mut decoded = Vec::with_capacity(decoded_len);
        GzDecoder::new(stored)
            .take(decoded_len as u64 + 1)
            .read_to_end(&mut decoded)?;
        decoded
    } else {
        stored.to_vec()
    };
    if raw.len() != decoded_len || Digest256::of_bytes(&raw).to_hex() != decoded_sha {
        return Err(Error::Invalid("decoded part mismatch"));
    }
    Ok(raw)
}

pub struct LegacyPartitionedNavigation {
    path: PathBuf,
    root_sha256: String,
    manifest: Value,
    limits: Limits,
    work_bytes: Cell<u64>,
}

fn strict_value(raw: &[u8], cap: usize) -> Result<Value> {
    let limits =
        JsonLimits::new(cap, 96, 1_000_000, 4300).map_err(|e| Error::Source(e.to_string()))?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("missing carrier string"))
}
fn number(v: &Value, key: &str) -> Result<u64> {
    v.get(key)
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("missing carrier count"))
}
fn leaf_key(row: &JsonValue) -> Result<&str> {
    row.object_get("key")
        .and_then(JsonValue::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(Error::Invalid("invalid partition key"))
}
fn read_bounded(path: &Path, cap: usize) -> Result<Vec<u8>> {
    let file = safe_open::open_regular(path, cap as u64)?;
    let info = file.metadata()?;
    let mut bytes = Vec::with_capacity(info.len() as usize);
    file.take(cap as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > cap || bytes.len() as u64 != info.len() {
        return Err(Error::Invalid("carrier size changed"));
    }
    Ok(bytes)
}

impl LegacyPartitionedNavigation {
    pub fn open(path: &Path, expected_root_sha256: &str, limits: Limits) -> Result<Self> {
        let expected = Digest256::from_hex(expected_root_sha256)
            .map_err(|_| Error::Invalid("invalid expected root digest"))?;
        let bytes = read_bounded(path, ROOT_CAP)?;
        if Digest256::of_bytes(&bytes) != expected {
            return Err(Error::Invalid("projection root digest mismatch"));
        }
        let manifest = strict_value(&bytes, ROOT_CAP)?;
        if string(&manifest, "schema_version")? != "tos_partitioned_projection_v1"
            || string(&manifest, "logical_schema")? != "tos_corpus_index_v1"
        {
            return Err(Error::Invalid("unsupported projection profile"));
        }
        let header = manifest
            .get("header")
            .ok_or(Error::Invalid("missing header"))?;
        if header
            .get("source_navigation")
            .and_then(|v| v.get("schema_version"))
            .and_then(Value::as_str)
            != Some("tos_source_navigation_v1")
        {
            return Err(Error::Invalid("unsupported source-navigation profile"));
        }
        for collection in Collection::ALL {
            let descriptor = manifest
                .get("collections")
                .and_then(|v| v.get(collection.legacy_name()))
                .ok_or(Error::Invalid("missing navigation collection"))?;
            if descriptor.get("key_field").and_then(Value::as_str)
                != Some(match collection {
                    Collection::Nodes => "node_id",
                    Collection::Edges => "edge_id",
                    Collection::Rights => "rights_id",
                })
            {
                return Err(Error::Invalid("navigation identity policy mismatch"));
            }
            let expected_order = match collection {
                Collection::Nodes => "node_id",
                Collection::Edges => "edge_id",
                Collection::Rights => "rights_id",
            };
            if descriptor
                .get("order_fields")
                .and_then(Value::as_array)
                .is_none_or(|v| v.len() != 1 || v[0].as_str() != Some(expected_order))
            {
                return Err(Error::Invalid("navigation order policy mismatch"));
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            root_sha256: expected_root_sha256.to_owned(),
            manifest,
            limits,
            work_bytes: Cell::new(0),
        })
    }

    fn part_path(&self, descriptor: &Value, prefix: &str) -> Result<PathBuf> {
        let kind = string(descriptor, "kind")?;
        if kind != "data" && kind != "index" {
            return Err(Error::Invalid("part kind"));
        }
        if string(descriptor, "prefix")? != prefix
            || prefix.len() > 64
            || !prefix.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::Invalid("part prefix"));
        }
        let digest = string(descriptor, "sha256")?;
        Digest256::from_hex(digest).map_err(|_| Error::Invalid("part digest"))?;
        let suffix = if kind == "data" {
            ".jsonl.gz"
        } else {
            ".index.json"
        };
        let stem = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or(Error::Invalid("projection filename"))?;
        let expected = format!("{stem}.parts/{}/{}{}", &digest[..2], digest, suffix);
        if string(descriptor, "path")? != expected {
            return Err(Error::Invalid("part outside exact namespace"));
        }
        let path = self
            .path
            .parent()
            .ok_or(Error::Invalid("root parent"))?
            .join(expected);
        Ok(path)
    }

    fn part_bytes(&self, descriptor: &Value, prefix: &str) -> Result<Vec<u8>> {
        let kind = string(descriptor, "kind")?;
        let limit = if kind == "data" { PART_CAP } else { INDEX_CAP };
        let decoded_len = usize::try_from(number(descriptor, "decoded_bytes")?)
            .map_err(|_| Error::Budget("part decoded bytes"))?;
        let stored_len = usize::try_from(number(descriptor, "size_bytes")?)
            .map_err(|_| Error::Budget("part stored bytes"))?;
        if decoded_len > limit || stored_len > limit + STORED_OVERHEAD {
            return Err(Error::Budget("part bytes"));
        }
        let path = self.part_path(descriptor, prefix)?;
        let stored = read_bounded(&path, stored_len)?;
        let work = self
            .work_bytes
            .get()
            .saturating_add(stored_len as u64)
            .saturating_add(decoded_len as u64);
        if work > self.limits.max_work_bytes {
            return Err(Error::Budget("input work bytes"));
        }
        self.work_bytes.set(work);
        decode_partition_part(
            &stored,
            kind,
            stored_len,
            decoded_len,
            string(descriptor, "sha256")?,
            string(descriptor, "decoded_sha256")?,
        )
    }

    fn walk(
        &self,
        collection: Collection,
        descriptor: &Value,
        prefix: &str,
        sink: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<u64> {
        let raw = self.part_bytes(descriptor, prefix)?;
        let expected_count = number(descriptor, "count")?;
        if string(descriptor, "kind")? == "index" {
            let index = strict_value(&raw, INDEX_CAP)?;
            if string(&index, "schema_version")? != "tos_projection_partition_index_v1"
                || string(&index, "prefix")? != prefix
                || number(&index, "count")? != expected_count
            {
                return Err(Error::Invalid("partition index mismatch"));
            }
            let children = index
                .get("children")
                .and_then(Value::as_object)
                .ok_or(Error::Invalid("partition children"))?;
            if children.is_empty() || prefix.len() >= 64 {
                return Err(Error::Invalid("empty/deep partition index"));
            }
            let mut count = 0u64;
            for (digit, child) in children {
                if digit.len() != 1 || !digit.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(Error::Invalid("partition digit"));
                }
                count = count
                    .checked_add(self.walk(collection, child, &format!("{prefix}{digit}"), sink)?)
                    .ok_or(Error::Budget("collection count"))?;
            }
            if count != expected_count {
                return Err(Error::Invalid("partition count mismatch"));
            }
            return Ok(count);
        }
        let mut count = 0u64;
        let mut previous: Option<String> = None;
        if !raw.is_empty() && raw.last() != Some(&b'\n') {
            return Err(Error::Invalid("partition row lacks line feed"));
        }
        let body = if raw.is_empty() {
            &raw[..]
        } else {
            &raw[..raw.len() - 1]
        };
        for line in body.split(|&b| b == b'\n').filter(|_| !raw.is_empty()) {
            if line.is_empty() {
                return Err(Error::Invalid("empty partition row"));
            }
            if line.len() > self.limits.max_row_bytes {
                return Err(Error::Budget("row bytes"));
            }
            let json_limits = JsonLimits::new(self.limits.max_row_bytes, 96, 1_000_000, 4300)
                .map_err(|e| Error::Source(e.to_string()))?;
            let document = parse_json(line, JsonMode::PublishedStrict, json_limits)
                .map_err(|e| Error::Source(e.to_string()))?;
            let row = document.root();
            let key = leaf_key(row)?;
            if previous.as_deref().is_some_and(|prior| key <= prior)
                || !Digest256::of_bytes(key.as_bytes())
                    .to_hex()
                    .starts_with(prefix)
            {
                return Err(Error::Invalid("partition key order/placement"));
            }
            previous = Some(key.to_owned());
            let value = row
                .object_get("value")
                .ok_or(Error::Invalid("missing row value"))?;
            let field = match collection {
                Collection::Nodes => "node_id",
                Collection::Edges => "edge_id",
                Collection::Rights => "rights_id",
            };
            if value.object_get(field).and_then(JsonValue::as_str) != Some(key) {
                return Err(Error::Invalid("partition identity mismatch"));
            }
            let carrier =
                canonical_bytes_v1(value, CanonicalProfile::SourceRecordDigestV1, json_limits)
                    .map_err(|e| Error::Source(e.to_string()))?;
            sink(&carrier)?;
            count += 1;
        }
        if count != expected_count {
            return Err(Error::Invalid("leaf count mismatch"));
        }
        Ok(count)
    }
}

/// Exact validated root-and-parts closure for any declared v1 collection
/// profile. Packaging shares the retained root and physical part readers; it
/// does not turn the carried records into admitted authored meaning.
pub fn partitioned_projection_closure(
    path: &Path,
    limits: Limits,
    deadline: std::time::Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<PathBuf>> {
    use std::collections::BTreeSet;
    let check = || {
        if cancelled() || std::time::Instant::now() >= deadline {
            Err(Error::Budget("projection closure deadline or cancellation"))
        } else {
            Ok(())
        }
    };
    check()?;
    let bytes = read_bounded(path, ROOT_CAP)?;
    let root_text =
        std::str::from_utf8(&bytes).map_err(|_| Error::Invalid("projection root UTF-8"))?;
    crate::prepared_source_binding::root_profile(
        root_text,
        path.to_str()
            .ok_or(Error::Invalid("projection root path"))?,
    )?;
    let reader = LegacyPartitionedNavigation {
        path: path.to_owned(),
        root_sha256: Digest256::of_bytes(&bytes).to_hex(),
        manifest: strict_value(&bytes, ROOT_CAP)?,
        limits,
        work_bytes: Cell::new(bytes.len() as u64),
    };
    let mut paths = BTreeSet::from([path.to_owned()]);
    let mut visits = 0usize;
    fn visit(
        reader: &LegacyPartitionedNavigation,
        descriptor: &Value,
        prefix: &str,
        spec: &Value,
        paths: &mut BTreeSet<PathBuf>,
        visits: &mut usize,
        check: &impl Fn() -> Result<()>,
    ) -> Result<u64> {
        check()?;
        *visits += 1;
        if *visits > 100_000 {
            return Err(Error::Budget("projection closure members"));
        }
        let fields = [
            "kind",
            "prefix",
            "path",
            "sha256",
            "size_bytes",
            "decoded_bytes",
            "decoded_sha256",
            "count",
        ];
        if descriptor
            .as_object()
            .is_none_or(|o| o.len() != fields.len() || fields.iter().any(|f| !o.contains_key(*f)))
            || prefix.len() > 64
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid("projection closure descriptor"));
        }
        Digest256::from_hex(string(descriptor, "sha256")?)
            .map_err(|_| Error::Invalid("projection part digest"))?;
        Digest256::from_hex(string(descriptor, "decoded_sha256")?)
            .map_err(|_| Error::Invalid("projection decoded digest"))?;
        let count = number(descriptor, "count")?;
        paths.insert(reader.part_path(descriptor, prefix)?);
        let raw = reader.part_bytes(descriptor, prefix)?;
        if string(descriptor, "kind")? == "index" {
            let index = strict_value(&raw, INDEX_CAP)?;
            if index.as_object().is_none_or(|o| {
                o.len() != 4
                    || ["schema_version", "prefix", "count", "children"]
                        .iter()
                        .any(|k| !o.contains_key(*k))
            }) || string(&index, "schema_version")? != "tos_projection_partition_index_v1"
                || string(&index, "prefix")? != prefix
                || number(&index, "count")? != count
                || prefix.len() >= 64
            {
                return Err(Error::Invalid("projection closure index"));
            }
            let children = index["children"]
                .as_object()
                .filter(|v| !v.is_empty())
                .ok_or(Error::Invalid("projection closure children"))?;
            let mut total = 0u64;
            for (digit, child) in children {
                if digit.len() != 1
                    || !digit
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(Error::Invalid("projection closure branch"));
                }
                total = total
                    .checked_add(visit(
                        reader,
                        child,
                        &format!("{prefix}{digit}"),
                        spec,
                        paths,
                        visits,
                        check,
                    )?)
                    .ok_or(Error::Budget("projection closure count"))?;
            }
            if total != count {
                return Err(Error::Invalid("projection closure index count"));
            }
        } else {
            let text =
                std::str::from_utf8(&raw).map_err(|_| Error::Invalid("projection row UTF-8"))?;
            let mut previous = None::<String>;
            let mut total = 0u64;
            for line in text.lines() {
                check()?;
                let row = strict_value(line.as_bytes(), PART_CAP)?;
                if row.as_object().is_none_or(|o| {
                    o.len() != 2 || !o.contains_key("key") || !o.contains_key("value")
                }) {
                    return Err(Error::Invalid("projection closure row"));
                }
                let key = string(&row, "key")?;
                if key.is_empty()
                    || key.len() > 4096
                    || previous.as_deref().is_some_and(|old| key <= old)
                    || !Digest256::of_bytes(key.as_bytes())
                        .to_hex()
                        .starts_with(prefix)
                {
                    return Err(Error::Invalid("projection closure row order or placement"));
                }
                let value = &row["value"];
                let field = &spec["key_field"];
                if field.as_array().is_some_and(Vec::is_empty) {
                    if key.len() != 20
                        || !key.bytes().all(|b| b.is_ascii_digit())
                        || key
                            .parse::<u64>()
                            .ok()
                            .is_none_or(|n| n >= spec["root"]["count"].as_u64().unwrap_or(0))
                    {
                        return Err(Error::Invalid("projection closure sequence position"));
                    }
                } else if !field.is_null() {
                    let expected = if let Some(fields) = field.as_array() {
                        let selected = fields
                            .iter()
                            .map(|f| {
                                value
                                    .get(f.as_str().unwrap_or(""))
                                    .and_then(Value::as_str)
                                    .filter(|s| !s.is_empty() && s.len() <= 4096)
                                    .ok_or(Error::Invalid("projection compound row key"))
                            })
                            .collect::<Result<Vec<_>>>()?;
                        serde_json::to_string(&selected)
                            .map_err(|_| Error::Invalid("projection compound key"))?
                    } else {
                        value
                            .get(field.as_str().unwrap_or(""))
                            .and_then(Value::as_str)
                            .ok_or(Error::Invalid("projection row identity"))?
                            .to_owned()
                    };
                    if expected != key {
                        return Err(Error::Invalid("projection row key mismatch"));
                    }
                }
                previous = Some(key.to_owned());
                total = total
                    .checked_add(1)
                    .ok_or(Error::Budget("projection leaf count"))?;
            }
            if total != count {
                return Err(Error::Invalid("projection closure leaf count"));
            }
        }
        Ok(count)
    }
    for spec in reader.manifest["collections"].as_object().unwrap().values() {
        visit(
            &reader,
            &spec["root"],
            "",
            spec,
            &mut paths,
            &mut visits,
            &check,
        )?;
    }
    check()?;
    if read_bounded(path, ROOT_CAP)? != bytes {
        return Err(Error::Invalid("projection root changed during closure"));
    }
    Ok(paths.into_iter().collect())
}

impl NavigationInput for LegacyPartitionedNavigation {
    fn verify_binding(&self, binding: &SourceBinding) -> Result<()> {
        if binding.projection_root_sha256 != self.root_sha256 {
            return Err(Error::Invalid("input and source binding roots differ"));
        }
        Ok(())
    }
    fn authority_boundary(&self) -> Result<String> {
        let boundary = self.manifest["header"]["source_navigation"]
            .get("authority_boundary")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(Error::Invalid("source navigation authority boundary"))?;
        Ok(boundary.to_owned())
    }
    fn visit(
        &mut self,
        collection: Collection,
        sink: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<()> {
        let root = self.manifest["collections"][collection.legacy_name()]["root"].clone();
        let count = self.walk(collection, &root, "", sink)?;
        let field = match collection {
            Collection::Nodes => "nodes",
            Collection::Edges => "edges",
            Collection::Rights => "rights",
        };
        if self.manifest["header"]["source_navigation"]["counts"][field].as_u64() != Some(count) {
            return Err(Error::Invalid(
                "navigation header count differs from verified collection",
            ));
        }
        Ok(())
    }
    fn verify_sealed_cut(&mut self) -> Result<()> {
        for collection in Collection::ALL {
            self.visit(collection, &mut |_| Ok(()))?;
        }
        let bytes = read_bounded(&self.path, ROOT_CAP)?;
        if Digest256::of_bytes(&bytes).to_hex() != self.root_sha256 {
            return Err(Error::Invalid("projection root changed"));
        }
        Ok(())
    }
}
