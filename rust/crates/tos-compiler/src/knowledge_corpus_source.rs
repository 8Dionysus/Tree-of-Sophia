//! Exact selected public corpus compatibility input. Reads only explicitly
//! selected capture members; it never claims an authored native builder result.
use crate::knowledge_corpus_original::*;
use crate::knowledge_stage::{KnowledgePayloadLayout, KnowledgeStage, WritePhase};
use crate::{Error, NavigationOriginalLimits, QueryVocabulary, Result, SourceBinding};
use rusqlite::params;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath,
    canonical_bytes_v1, parse_json,
};
use tos_source_store::SoftwareCaptureReader;

#[derive(Clone, Copy, Debug)]
pub struct CorpusOriginalSourceLimits {
    pub originals: NavigationOriginalLimits,
    pub max_members: usize,
    pub max_work_bytes: u64,
}
enum CaptureReader<'a> {
    Software(&'a SoftwareCaptureReader),
    Public(&'a crate::d1_public_capture::PublicCapture),
}
struct Source<'a> {
    reader: CaptureReader<'a>,
    root: &'a RelativePath,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    work: u64,
    members: BTreeMap<String, CorpusOriginalMember>,
    owned: Option<&'a crate::d1_public_capture::CreationState<'a>>,
}
// Raw parts and decoded input trees belong to the current read/walk scope.
// Output packet owners are admitted independently by the sink. Drop the value
// before releasing its state reservation, including on early return.
struct ScopedCorpusInput<'a, T> {
    value: T,
    _state: Option<crate::d1_public_capture::CreationStateHold<'a, 'a>>,
}
impl<T> std::ops::Deref for ScopedCorpusInput<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<'a> Source<'a> {
    fn check(&self) -> Result<()> {
        if let Some(state) = self.owned {
            state.remaining(0)?;
        }
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(Error::Invalid("corpus capture cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("corpus capture deadline"));
        }
        Ok(())
    }
    fn charge(&mut self, bytes: u64) -> Result<()> {
        self.check()?;
        if let Some(state) = self.owned {
            state.charge_work(
                usize::try_from(bytes).map_err(|_| Error::Budget("owned corpus work width"))?,
            )?;
        }
        self.work = self
            .work
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_work_bytes)
            .ok_or(Error::Budget("corpus capture work"))?;
        Ok(())
    }
    fn read(&mut self, path: &RelativePath, cap: usize) -> Result<ScopedCorpusInput<'a, Vec<u8>>> {
        self.check()?;
        if !self.members.contains_key(path.as_str())
            && self.members.len() >= self.limits.max_members
        {
            return Err(Error::Budget("corpus capture member count"));
        }
        let mut raw_state = None;
        if let Some(state) = self.owned {
            let CaptureReader::Public(reader) = &self.reader else {
                return Err(Error::Invalid("owned corpus native capture required"));
            };
            let len = reader.retained_input_length(path.as_str())?;
            if len > cap {
                return Err(Error::Budget("owned corpus raw bytes"));
            }
            let node = 11 * std::mem::size_of::<(String, CorpusOriginalMember)>()
                + 16 * std::mem::size_of::<usize>();
            // Member path/digest metadata survives in the receipt. The raw
            // input Vec survives only until this particular caller drops it.
            state.retain(
                64usize
                    .checked_add(2 * path.as_str().len())
                    .and_then(|n| n.checked_add(node))
                    .ok_or(Error::Budget("owned corpus read state"))?,
            )?;
            raw_state = Some(state.hold(len)?);
        }
        let (raw, size_bytes, sha256) = match &self.reader {
            CaptureReader::Software(reader) => {
                let selection = reader
                    .select_components(&[path.clone()])
                    .map_err(|e| Error::Source(e.to_string()))?;
                let member = selection
                    .member(path)
                    .ok_or(Error::Invalid("corpus captured member"))?;
                if member.size_bytes > self.limits.max_work_bytes.saturating_sub(self.work) {
                    return Err(Error::Budget("corpus capture work"));
                }
                let raw = reader
                    .read_selected_component(
                        &selection,
                        path,
                        cap as u64,
                        self.deadline,
                        self.cancelled,
                    )
                    .map_err(|e| Error::Source(e.to_string()))?;
                (raw, member.size_bytes, member.sha256.to_hex())
            }
            CaptureReader::Public(reader) => {
                let remaining =
                    usize::try_from(self.limits.max_work_bytes.saturating_sub(self.work))
                        .unwrap_or(usize::MAX);
                let raw = reader.read_retained_input(path.as_str(), cap.min(remaining))?;
                let size = raw.len() as u64;
                let sha = Digest256::of_bytes(&raw).to_hex();
                (raw, size, sha)
            }
        };
        self.charge(size_bytes)?;
        self.members.insert(
            path.as_str().into(),
            CorpusOriginalMember {
                path: path.as_str().into(),
                size_bytes,
                sha256,
            },
        );
        Ok(ScopedCorpusInput {
            value: raw,
            _state: raw_state,
        })
    }
    fn json(&self, raw: &[u8], cap: usize) -> Result<ScopedCorpusInput<'a, Value>> {
        match self.owned {
            Some(state) => {
                let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
                    .map_err(|_| Error::Budget("corpus original JSON limits"))?;
                let (value, hold) = state.serde_scoped_with_limits(raw, limits)?;
                Ok(ScopedCorpusInput {
                    value,
                    _state: Some(hold),
                })
            }
            None => Ok(ScopedCorpusInput {
                value: json(raw, cap)?,
                _state: None,
            }),
        }
    }
    fn encode(&self, value: &Value, cap: usize) -> Result<Vec<u8>> {
        match self.owned {
            Some(state) => state.encode_canonical(value, cap),
            None => encode(value, cap),
        }
    }
    fn part(&mut self, d: &Value, prefix: &str) -> Result<ScopedCorpusInput<'a, Vec<u8>>> {
        keys(
            d,
            &[
                "kind",
                "prefix",
                "path",
                "sha256",
                "size_bytes",
                "decoded_bytes",
                "decoded_sha256",
                "count",
            ],
        )?;
        let kind = string(d, "kind")?;
        let cap = match kind {
            "data" => crate::legacy::PART_CAP,
            "index" => crate::legacy::INDEX_CAP,
            _ => return Err(Error::Invalid("corpus partition kind")),
        };
        if string(d, "prefix")? != prefix
            || prefix.len() > 64
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid("corpus partition prefix"));
        }
        let sha = string(d, "sha256")?;
        Digest256::from_hex(sha).map_err(|_| Error::Invalid("corpus part SHA"))?;
        let decoded_sha = string(d, "decoded_sha256")?;
        Digest256::from_hex(decoded_sha).map_err(|_| Error::Invalid("corpus decoded SHA"))?;
        let stem = self
            .root
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(Error::Invalid("corpus source leaf"))?
            .rsplit_once('.')
            .map_or(self.root.as_str().rsplit('/').next().unwrap(), |(s, _)| s);
        let suffix = if kind == "data" {
            ".jsonl.gz"
        } else {
            ".index.json"
        };
        let _path_state = self
            .owned
            .map(|state| {
                state.hold(
                    self.root
                        .as_str()
                        .len()
                        .checked_add(256)
                        .and_then(|n| n.checked_mul(4))
                        .ok_or(Error::Budget("owned corpus part paths"))?,
                )
            })
            .transpose()?;
        let relative = format!("{stem}.parts/{}/{}{suffix}", &sha[..2], sha);
        if string(d, "path")? != relative {
            return Err(Error::Invalid("corpus exact partition namespace"));
        }
        let parent = self.root.as_str().rsplit_once('/').map_or("", |(p, _)| p);
        let path = RelativePath::parse(&if parent.is_empty() {
            relative
        } else {
            format!("{parent}/{relative}")
        })
        .map_err(|_| Error::Invalid("corpus part path"))?;
        let size = number(d, "size_bytes")?;
        let decoded = number(d, "decoded_bytes")?;
        if decoded > cap as u64 || size > (cap + crate::legacy::STORED_OVERHEAD) as u64 {
            return Err(Error::Budget("corpus partition bytes"));
        }
        let stored = self.read(&path, size as usize)?;
        self.charge(decoded)?;
        let _decoder_state = self
            .owned
            .map(|state| state.hold(crate::legacy::partition_decoder_workspace_upper(kind)?))
            .transpose()?;
        let output_state = self
            .owned
            .map(|state| {
                let output = usize::try_from(decoded)
                    .map_err(|_| Error::Budget("owned corpus decoded width"))?
                    .checked_add(1)
                    .and_then(|n| n.max(32).checked_mul(3))
                    .ok_or(Error::Budget("owned corpus decoder state"))?;
                state.hold(output)
            })
            .transpose()?;
        let raw = crate::legacy::decode_partition_part(
            &stored,
            kind,
            size as usize,
            decoded as usize,
            sha,
            decoded_sha,
        )?;
        Ok(ScopedCorpusInput {
            value: raw,
            _state: output_state,
        })
    }
    fn walk(
        &mut self,
        d: &Value,
        prefix: &str,
        key: &Value,
        root_count: u64,
        sink: &mut dyn FnMut(&mut Source<'_>, &str, &Value) -> Result<()>,
    ) -> Result<u64> {
        let raw = self.part(d, prefix)?;
        let wanted = number(d, "count")?;
        if string(d, "kind")? == "index" {
            let v = self.json(&raw, crate::legacy::INDEX_CAP)?;
            keys(&v, &["schema_version", "prefix", "count", "children"])?;
            if string(&v, "schema_version")? != "tos_projection_partition_index_v1"
                || string(&v, "prefix")? != prefix
                || number(&v, "count")? != wanted
            {
                return Err(Error::Invalid("corpus partition index"));
            }
            let children = v["children"]
                .as_object()
                .filter(|v| !v.is_empty() && prefix.len() < 64)
                .ok_or(Error::Invalid("corpus partition children"))?;
            let mut n = 0u64;
            for (digit, child) in children {
                let _prefix_state = self
                    .owned
                    .map(|state| {
                        state.hold(
                            prefix
                                .len()
                                .checked_add(digit.len())
                                .ok_or(Error::Budget("owned corpus prefix"))?,
                        )
                    })
                    .transpose()?;
                if digit.len() != 1
                    || !digit
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(Error::Invalid("corpus partition digit"));
                }
                n = n
                    .checked_add(self.walk(
                        child,
                        &format!("{prefix}{digit}"),
                        key,
                        root_count,
                        sink,
                    )?)
                    .ok_or(Error::Budget("corpus partition count"))?;
            }
            if n != wanted {
                return Err(Error::Invalid("corpus partition child coverage"));
            }
            return Ok(n);
        }
        if !raw.is_empty() && raw.last() != Some(&b'\n') {
            return Err(Error::Invalid("corpus part row newline"));
        }
        let mut n = 0u64;
        let _previous_state = self.owned.map(|state| state.hold(4096 + 64)).transpose()?;
        let mut previous = None::<String>;
        let body = if raw.is_empty() {
            raw.as_slice()
        } else {
            &raw[..raw.len() - 1]
        };
        for line in body.split(|b| *b == b'\n').filter(|_| !raw.is_empty()) {
            self.check()?;
            if line.is_empty() {
                return Err(Error::Invalid("empty corpus partition row"));
            }
            let row = self.json(line, self.limits.originals.max_row_bytes)?;
            keys(&row, &["key", "value"])?;
            let id = string(&row, "key")?;
            if id.is_empty()
                || id.len() > 4096
                || previous.as_deref().is_some_and(|p| id <= p)
                || !Digest256::of_bytes(id.as_bytes())
                    .to_hex()
                    .starts_with(prefix)
            {
                return Err(Error::Invalid("corpus partition key/order"));
            }
            let value = &row["value"];
            let valid = match key {
                Value::String(field) => value.get(field).and_then(Value::as_str) == Some(id),
                Value::Array(fields) if fields.is_empty() => {
                    id.len() == 20
                        && id.bytes().all(|b| b.is_ascii_digit())
                        && id
                            .parse::<u64>()
                            .is_ok_and(|position| position < root_count)
                }
                Value::Array(fields) => {
                    let _key_state = self
                        .owned
                        .map(|state| {
                            state.hold(
                                fields
                                    .len()
                                    .checked_mul(std::mem::size_of::<&str>() + 6 * 4096)
                                    .and_then(|n| n.checked_add(32))
                                    .ok_or(Error::Budget("owned corpus composite key"))?,
                            )
                        })
                        .transpose()?;
                    let parts = fields
                        .iter()
                        .map(|f| {
                            f.as_str()
                                .and_then(|f| value.get(f))
                                .and_then(Value::as_str)
                                .filter(|s| !s.is_empty() && s.len() <= 4096)
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or(Error::Invalid("corpus composite key"))?;
                    serde_json::to_string(&parts).map_err(|_| Error::Invalid("corpus key JSON"))?
                        == id
                }
                _ => false,
            };
            if !valid {
                return Err(Error::Invalid("corpus partition key identity"));
            }
            // The row remains alive through the callback; the sink owns and
            // accounts any encoded output it retains. No intermediate clone.
            sink(self, id, value)?;
            previous = Some(id.into());
            n = n.checked_add(1).ok_or(Error::Budget("corpus leaf count"))?;
        }
        if n != wanted {
            return Err(Error::Invalid("corpus partition leaf coverage"));
        }
        Ok(n)
    }
}
fn keys(v: &Value, wanted: &[&str]) -> Result<()> {
    let m = v
        .as_object()
        .ok_or(Error::Invalid("corpus source object"))?;
    if m.len() != wanted.len() || wanted.iter().any(|k| !m.contains_key(*k)) {
        return Err(Error::Invalid("corpus source fields"));
    }
    Ok(())
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("corpus source string"))
}
fn number(v: &Value, key: &str) -> Result<u64> {
    v.get(key)
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("corpus source integer"))
}
fn json(raw: &[u8], cap: usize) -> Result<Value> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("corpus source JSON limits"))?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|_| Error::Invalid("corpus source JSON"))
}
pub(crate) fn encode(v: &Value, cap: usize) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(|_| Error::Invalid("corpus original encoding"))?;
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("corpus original JSON limits"))?;
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|e| Error::Source(e.to_string()))
}
fn check_originals(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("corpus original cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("corpus original deadline"));
    }
    Ok(())
}
// The two actual origins share packet framing and its resource contract.
fn original_packets(
    payload: &Value,
    limits: NavigationOriginalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(Vec<u8>, Vec<(CorpusOriginalCollection, Vec<Vec<u8>>)>, u64)> {
    limits.validate()?;
    let mut rows = Vec::new();
    let mut total = 0u64;
    let mut count = 1u64;
    for collection in CorpusOriginalCollection::ROWS {
        let values = payload
            .get(collection.as_str())
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("corpus original required array"))?;
        if count
            .checked_add(values.len() as u64)
            .is_none_or(|n| n > limits.max_rows)
        {
            return Err(Error::Budget("corpus original complete collection rows"));
        }
        let mut encoded = Vec::with_capacity(values.len());
        for value in values {
            check_originals(deadline, cancelled)?;
            super::knowledge_corpus_original::indexed_fields(collection, value)?;
            let raw = encode(value, limits.max_row_bytes)?;
            count = count
                .checked_add(1)
                .filter(|n| *n <= limits.max_rows)
                .ok_or(Error::Budget("corpus original rows"))?;
            total = total
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= limits.max_total_bytes)
                .ok_or(Error::Budget("corpus original bytes"))?;
            encoded.push(raw);
        }
        rows.push((collection, encoded));
    }
    let header = detached_header(payload, limits.max_row_bytes, None)?;
    total = total
        .checked_add(header.len() as u64)
        .filter(|n| *n <= limits.max_total_bytes)
        .ok_or(Error::Budget("corpus original header bytes"))?;
    Ok((header, rows, total))
}
// One source law for the finite compatibility plan and the disk-backed importer.
// The sink sees one logical row; it must not collect all source collections.
fn visit_captured_rows(
    source: &mut Source<'_>,
    root: &Value,
    sink_row: &mut dyn FnMut(
        &mut Source<'_>,
        CorpusOriginalCollection,
        &[String],
        &Value,
    ) -> Result<()>,
) -> Result<Vec<u8>> {
    let limits = source.limits;
    if root["schema_version"] == "tos_partitioned_projection_v1" {
        keys(
            &root,
            &[
                "schema_version",
                "logical_schema",
                "header",
                "limits",
                "collections",
            ],
        )?;
        if root["logical_schema"] != "tos_corpus_index_v1"
            || root["header"]["schema_version"] != "tos_corpus_index_v1"
            || !root["limits"].as_object().is_some_and(|fields| {
                fields.len() == 4
                    && [
                        ("root_bytes", crate::legacy::ROOT_CAP),
                        ("index_bytes", crate::legacy::INDEX_CAP),
                        ("part_bytes", crate::legacy::PART_CAP),
                        ("key_bytes", 4096),
                    ]
                    .iter()
                    .all(|(key, expected)| {
                        fields.get(*key).and_then(Value::as_u64) == Some(*expected as u64)
                    })
            })
        {
            return Err(Error::Invalid("corpus partition logical profile"));
        }
        let payload = &root["header"];
        let specs = root["collections"]
            .as_object()
            .ok_or(Error::Invalid("corpus partition collections"))?;
        let required: &[(&str, &[&str], &[&str])] = &[
            ("nodes", &["node_id"], &["source_path"]),
            ("resources", &["path"], &["path"]),
            ("manifests", &["path"], &["path"]),
            ("relation_packs", &["pack_id"], &["path"]),
            (
                "relation_edges",
                &["pack_id", "edge_id"],
                &["pack_id", "edge_id"],
            ),
            ("source_navigation/nodes", &["node_id"], &["node_id"]),
            ("source_navigation/edges", &["edge_id"], &["edge_id"]),
            ("source_navigation/rights", &["rights_id"], &["rights_id"]),
        ];
        if specs.len() != required.len() + usize::from(specs.contains_key("diagnostics"))
            || required.iter().any(|(n, _, _)| !specs.contains_key(*n))
        {
            return Err(Error::Invalid("corpus partition collection closure"));
        }
        for (name, spec) in specs {
            keys(spec, &["key_field", "order_fields", "root"])?;
            let (key_fields, order): (&[&str], &[&str]) = if name == "diagnostics" {
                (&[], &[])
            } else {
                let (_, key, order) = required
                    .iter()
                    .find(|(n, _, _)| *n == name)
                    .ok_or(Error::Invalid("corpus partition collection"))?;
                (*key, *order)
            };
            let key = &spec["key_field"];
            let key_matches = if key_fields.is_empty() {
                key.as_array().is_some_and(|items| items.is_empty())
            } else if key_fields.len() == 1 {
                key.as_str() == Some(key_fields[0])
            } else {
                key.as_array().is_some_and(|items| {
                    items.len() == key_fields.len()
                        && items
                            .iter()
                            .zip(key_fields)
                            .all(|(item, expected)| item.as_str() == Some(*expected))
                })
            };
            let order_matches = spec["order_fields"].as_array().is_some_and(|items| {
                items.len() == order.len()
                    && items
                        .iter()
                        .zip(order)
                        .all(|(item, expected)| item.as_str() == Some(*expected))
            });
            if !key_matches || !order_matches {
                return Err(Error::Invalid("corpus partition ordering policy"));
            }
            let keep = !name.starts_with("source_navigation/") && name != "diagnostics";
            let collection = CorpusOriginalCollection::ROWS
                .iter()
                .copied()
                .find(|c| c.as_str() == name);
            let mut sink = |source: &mut Source<'_>, _: &str, v: &Value| -> Result<()> {
                if !keep {
                    return Ok(());
                }
                let _sort_state = source
                    .owned
                    .map(|state| {
                        let bytes = order.iter().try_fold(
                            order.len() * std::mem::size_of::<String>(),
                            |n, field| {
                                n.checked_add(
                                    v.get(*field).and_then(Value::as_str).map_or(0, str::len),
                                )
                                .ok_or(Error::Budget("owned corpus ordering strings"))
                            },
                        )?;
                        state.hold(bytes)
                    })
                    .transpose()?;
                let sort = order
                    .iter()
                    .map(|f| match v.get(*f) {
                        None => Ok(String::new()),
                        Some(Value::String(s)) => Ok(s.clone()),
                        _ => Err(Error::Invalid(
                            "corpus partition order field must be string",
                        )),
                    })
                    .collect::<Result<Vec<_>>>()?;
                sink_row(
                    source,
                    collection.ok_or(Error::Invalid("corpus original collection"))?,
                    &sort,
                    v,
                )
            };
            // Unique hashed keys plus the complete root count and positional
            // range prove dense diagnostic positions, including unretained rows.
            source.walk(
                &spec["root"],
                "",
                key,
                number(&spec["root"], "count")?,
                &mut sink,
            )?;
        }

        for collection in [
            CorpusOriginalCollection::Branches,
            CorpusOriginalCollection::GraphViews,
        ] {
            for value in payload
                .get(collection.as_str())
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("corpus original required array"))?
            {
                source.check()?;
                sink_row(source, collection, &[], value)?;
            }
        }
        return detached_header(payload, limits.originals.max_row_bytes, source.owned);
    }
    if root["schema_version"] != "tos_corpus_index_v1" {
        return Err(Error::Invalid("captured corpus logical schema"));
    }
    for collection in CorpusOriginalCollection::ROWS {
        for value in root
            .get(collection.as_str())
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("corpus original required array"))?
        {
            source.check()?;
            sink_row(source, collection, &[], value)?;
        }
    }
    detached_header(root, limits.originals.max_row_bytes, source.owned)
}
fn detached_header(
    payload: &Value,
    cap: usize,
    owned: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<Vec<u8>> {
    let omitted = [
        "nodes",
        "edges",
        "rights",
        "resources",
        "manifests",
        "branches",
        "relation_edges",
        "relation_packs",
        "graph_views",
        "claim_traces",
        "input_digests",
        "source_navigation",
    ];
    let fields = payload
        .as_object()
        .ok_or(Error::Invalid("corpus original header object"))?;
    let mut holds = Vec::new();
    let mut header = serde_json::Map::new();
    if let Some(state) = owned {
        state.retain(
            (fields.len() * 8 + 4)
                * std::mem::size_of::<crate::d1_public_capture::CreationStateHold<'_, '_>>(),
        )?;
        holds.push(
            state.hold(crate::knowledge_normalization::serde_object_slots_upper(
                fields.len(),
            )?)?,
        );
    }
    for (field, value) in fields
        .iter()
        .filter(|(field, _)| !omitted.contains(&field.as_str()))
    {
        let value = if let Some(state) = owned {
            holds.push(state.hold(field.len())?);
            let (value, hold) = state.clone_value_scoped(value)?;
            holds.push(hold);
            value
        } else {
            value.clone()
        };
        header.insert(field.clone(), value);
    }
    let header = Value::Object(header);
    let encoded = match owned {
        Some(state) => state.encode_canonical(&header, cap),
        None => encode(&header, cap),
    };
    drop(header);
    drop(holds);
    encoded
}
fn captured_source<'a>(
    capture: &'a SoftwareCaptureReader,
    source_path: &'a RelativePath,
    binding: &SourceBinding,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
) -> Result<Source<'a>> {
    limits.originals.validate()?;
    if limits.max_members == 0
        || limits.max_members > 65_536
        || limits.max_work_bytes == 0
        || limits.max_work_bytes > crate::knowledge_original_rows::MAX_COLD_WORK
        || !binding.complete
    {
        return Err(Error::Budget("corpus captured source limits"));
    }
    Ok(Source {
        reader: CaptureReader::Software(capture),
        root: source_path,
        limits,
        deadline,
        cancelled,
        work: 0,
        members: BTreeMap::new(),
        owned: None,
    })
}
fn captured_receipt(
    source: &mut Source<'_>,
    binding: &SourceBinding,
    vocab: &QueryVocabulary,
    root_raw: &[u8],
    header: &[u8],
    collections: Vec<CorpusOriginalCollectionReceipt>,
    total: u64,
) -> Result<CorpusOriginalReceipt> {
    if let Some(state) = source.owned {
        let strings = source.members.values().try_fold(0usize, |n, member| {
            n.checked_add(member.path.len())
                .and_then(|n| n.checked_add(member.sha256.len()))
                .ok_or(Error::Budget("owned corpus manifest strings"))
        })?;
        let upper = source
            .members
            .len()
            .checked_mul(std::mem::size_of::<CorpusOriginalMember>())
            .and_then(|n| {
                n.checked_add(
                    crate::knowledge_normalization::serde_object_slots_upper(source.members.len())
                        .ok()?,
                )
            })
            .and_then(|n| n.checked_add(strings))
            .and_then(|n| n.checked_add(binding.source_cut.len()))
            .and_then(|n| n.checked_add(binding.membership_root.len()))
            .and_then(|n| n.checked_add(vocab.descriptor_sha256.len()))
            .and_then(|n| n.checked_add(source.root.as_str().len()))
            .and_then(|n| n.checked_add(8 * 64 + CORPUS_ORIGINAL_PROFILE.len() + 64))
            .ok_or(Error::Budget("owned corpus receipt construction"))?;
        state.retain(upper)?;
        state.charge_work(strings)?;
    }
    let members = std::mem::take(&mut source.members)
        .into_values()
        .collect::<Vec<_>>();
    let mut member_root = Digest256Hasher::new();
    text(&mut member_root, "tos-captured-corpus-members-v1");
    for m in &members {
        text(&mut member_root, &m.path);
        member_root.update(&m.size_bytes.to_be_bytes());
        member_root.update(
            Digest256::from_hex(&m.sha256)
                .map_err(|_| Error::Invalid("corpus origin SHA"))?
                .as_bytes(),
        );
    }
    let (profile, source_git_commit, source_git_tree, capture_manifest_sha256) =
        match &source.reader {
            CaptureReader::Software(reader) => {
                let pin = reader.selection();
                (
                    "captured-public-corpus-v1",
                    Some(pin.source_git_commit.clone()),
                    Some(pin.source_git_tree.clone()),
                    pin.capture_manifest_sha256.to_hex(),
                )
            }
            CaptureReader::Public(reader) => {
                reader.check_custody()?;
                let entries = members
                    .iter()
                    .map(|member| (member.path.clone(), Value::String(member.sha256.clone())))
                    .collect();
                let entries = Value::Object(entries);
                let cap = limits_manifest_cap(source.limits)?;
                let digest = if let Some(state) = source.owned {
                    Digest256::of_bytes(&state.encode_canonical(&entries, cap)?)
                } else {
                    captured_runtime_input_manifest_digest(&entries, cap)?
                };
                (
                    "captured-runtime-projection-v1",
                    None,
                    None,
                    digest.to_hex(),
                )
            }
        };
    let mut receipt = CorpusOriginalReceipt {
        profile: CORPUS_ORIGINAL_PROFILE.into(),
        descriptor_sha256: vocab.descriptor_sha256.clone(),
        source_cut: binding.source_cut.clone(),
        membership_root: binding.membership_root.clone(),
        origin: CapturedCorpusOrigin {
            profile: profile.into(),
            source_git_commit,
            source_git_tree,
            capture_manifest_sha256: Some(capture_manifest_sha256),
            native_producer: None,
            source_path: source.root.as_str().into(),
            source_sha256: Digest256::of_bytes(root_raw).to_hex(),
            source_size_bytes: root_raw.len() as u64,
            members,
            member_root_sha256: member_root.finalize().to_hex(),
        },
        header_sha256: Digest256::of_bytes(header).to_hex(),
        collections,
        component_root_sha256: String::new(),
        total_bytes: total,
    };
    receipt.component_root_sha256 =
        super::knowledge_corpus_original::component_root_with_state(&receipt, source.owned)?;
    super::knowledge_corpus_original::validate_receipt_with_state(&receipt, source.owned)?;
    Ok(receipt)
}
/// Finite compatibility plan. Partitioned inputs use the same traversal law,
/// but this older API retains all encoded rows. Prefer direct stage import.
pub fn prepare_captured_corpus_original(
    capture: &SoftwareCaptureReader,
    source_path: &RelativePath,
    binding: &SourceBinding,
    vocab: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CapturedCorpusOriginalPlan> {
    let source = captured_source(capture, source_path, binding, limits, deadline, cancelled)?;
    prepare_original_plan(source, source_path, binding, vocab, limits)
}

/// Capture-only original projection import. No Git origin or authored producer
/// identity is invented for an immutable public runtime projection.
pub(crate) fn prepare_runtime_corpus_original(
    capture: &crate::d1_public_capture::PublicCapture,
    source_path: &RelativePath,
    binding: &SourceBinding,
    vocab: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CapturedCorpusOriginalPlan> {
    limits.originals.validate()?;
    if limits.max_members == 0
        || limits.max_members > 65_536
        || limits.max_work_bytes == 0
        || limits.max_work_bytes > crate::knowledge_original_rows::MAX_COLD_WORK
        || !binding.complete
    {
        return Err(Error::Budget("corpus captured source limits"));
    }
    capture.check_custody()?;
    let source = Source {
        reader: CaptureReader::Public(capture),
        root: source_path,
        limits,
        deadline,
        cancelled,
        work: 0,
        members: BTreeMap::new(),
        owned: None,
    };
    prepare_original_plan(source, source_path, binding, vocab, limits)
}
pub(crate) fn prepare_runtime_corpus_original_owned<'a>(
    capture: &'a crate::d1_public_capture::PublicCapture,
    source_path: &'a RelativePath,
    binding: &SourceBinding,
    vocab: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    state: &'a crate::d1_public_capture::CreationState<'a>,
) -> Result<CapturedCorpusOriginalPlan> {
    limits.originals.validate()?;
    if limits.max_members == 0
        || limits.max_members > 65_536
        || limits.max_work_bytes == 0
        || limits.max_work_bytes > crate::knowledge_original_rows::MAX_COLD_WORK
        || !binding.complete
    {
        return Err(Error::Budget("corpus captured source limits"));
    }
    capture.check_custody()?;
    let source = Source {
        reader: CaptureReader::Public(capture),
        root: source_path,
        limits,
        deadline,
        cancelled,
        work: 0,
        members: BTreeMap::new(),
        owned: Some(state),
    };
    prepare_original_plan(source, source_path, binding, vocab, limits)
}
fn limits_manifest_cap(limits: CorpusOriginalSourceLimits) -> Result<usize> {
    usize::try_from(limits.originals.max_total_bytes)
        .map_err(|_| Error::Budget("corpus capture manifest cap"))
}
fn prepare_original_plan(
    mut source: Source<'_>,
    source_path: &RelativePath,
    binding: &SourceBinding,
    vocab: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
) -> Result<CapturedCorpusOriginalPlan> {
    let raw = source.read(source_path, crate::legacy::PART_CAP)?;
    let root = source.json(&raw, crate::legacy::PART_CAP)?;
    check_partition_root_size(&root, raw.len())?;
    let mut packets = BTreeMap::<String, Vec<(Vec<String>, Vec<u8>)>>::new();
    let mut count = 1u64;
    let mut total = 0u64;
    let mut sink = |source: &mut Source<'_>,
                    collection: CorpusOriginalCollection,
                    sort: &[String],
                    v: &Value|
     -> Result<()> {
        indexed_fields(collection, v)?;
        if let Some(state) = source.owned {
            let sort_bytes = sort.iter().try_fold(0usize, |n, key| {
                n.checked_add(key.capacity())
                    .ok_or(Error::Budget("owned corpus sort keys"))
            })?;
            let slots = std::mem::size_of::<(Vec<String>, Vec<u8>)>();
            state.retain(
                4 * slots
                    + sort_bytes
                    + sort.len() * std::mem::size_of::<String>()
                    + 11 * std::mem::size_of::<(String, Vec<(Vec<String>, Vec<u8>)>)>()
                    + 16 * std::mem::size_of::<usize>(),
            )?;
        }
        let raw = source.encode(v, limits.originals.max_row_bytes)?;
        charge_packet(&mut count, &mut total, &raw, limits.originals)?;
        source.charge(raw.len() as u64)?;
        packets
            .entry(collection.as_str().into())
            .or_default()
            .push((sort.to_vec(), raw));
        Ok(())
    };
    let header = visit_captured_rows(&mut source, &root, &mut sink)?;
    total = total
        .checked_add(header.len() as u64)
        .filter(|n| *n <= limits.originals.max_total_bytes)
        .ok_or(Error::Budget("corpus original header bytes"))?;
    source.charge(header.len() as u64)?;
    if let Some(state) = source.owned {
        let row_count = packets.values().try_fold(0usize, |n, rows| {
            n.checked_add(rows.len())
                .ok_or(Error::Budget("owned corpus packet row count"))
        })?;
        let row_slots = row_count
            .checked_mul(
                std::mem::size_of::<(Vec<String>, Vec<u8>)>() + std::mem::size_of::<Vec<u8>>(),
            )
            .ok_or(Error::Budget("owned corpus sort and output slots"))?;
        let fixed = CorpusOriginalCollection::ROWS
            .len()
            .checked_mul(
                std::mem::size_of::<(CorpusOriginalCollection, Vec<Vec<u8>>)>()
                    + std::mem::size_of::<CorpusOriginalCollectionReceipt>()
                    + 64
                    + 32,
            )
            .ok_or(Error::Budget("owned corpus collection receipts"))?;
        state.retain(
            row_slots
                .checked_add(fixed)
                .ok_or(Error::Budget("owned corpus completed packets"))?,
        )?;
        state.charge_work(
            usize::try_from(total).map_err(|_| Error::Budget("owned corpus packet work width"))?,
        )?;
    }
    let rows = CorpusOriginalCollection::ROWS
        .into_iter()
        .map(|c| {
            let mut p = packets.remove(c.as_str()).unwrap_or_default();
            p.sort_by(|a, b| a.0.cmp(&b.0)); // Stable encounter tie, as the maintained logical reader.
            (c, p.into_iter().map(|(_, raw)| raw).collect::<Vec<_>>())
        })
        .collect::<Vec<_>>();
    let collections = rows
        .iter()
        .map(|(c, r)| CorpusOriginalCollectionReceipt {
            collection: c.as_str().into(),
            rows: r.len() as u64,
            ordered_root_sha256: ordered_root(c.as_str(), r),
        })
        .collect();
    let receipt = captured_receipt(
        &mut source,
        binding,
        vocab,
        &raw,
        &header,
        collections,
        total,
    )?;
    if let Some(state) = source.owned {
        state.retain(
            std::mem::size_of::<SourceBinding>()
                .checked_add(binding.source_cut.len())
                .and_then(|n| n.checked_add(binding.membership_root.len()))
                .and_then(|n| n.checked_add(binding.projection_root_sha256.len()))
                .and_then(|n| n.checked_add(binding.owner_profile.len()))
                .and_then(|n| n.checked_add(binding.index_generation.len()))
                .and_then(|n| n.checked_add(binding.route_map_version.len()))
                .and_then(|n| n.checked_add(binding.reader_abi.len()))
                .ok_or(Error::Budget("owned corpus plan binding"))?,
        )?;
    }
    Ok(CapturedCorpusOriginalPlan {
        binding: binding.clone(),
        receipt,
        header,
        rows,
    })
}
fn check_partition_root_size(root: &Value, bytes: usize) -> Result<()> {
    if root["schema_version"] == "tos_partitioned_projection_v1" && bytes > crate::legacy::ROOT_CAP
    {
        return Err(Error::Budget("corpus partition root bytes"));
    }
    Ok(())
}
fn charge_packet(
    count: &mut u64,
    total: &mut u64,
    raw: &[u8],
    limits: NavigationOriginalLimits,
) -> Result<()> {
    *count = count
        .checked_add(1)
        .filter(|n| *n <= limits.max_rows)
        .ok_or(Error::Budget("corpus original rows"))?;
    *total = total
        .checked_add(raw.len() as u64)
        .filter(|n| *n <= limits.max_total_bytes)
        .ok_or(Error::Budget("corpus original bytes"))?;
    Ok(())
}

// Private external sort. It is never a selected component and is removed
// before successful receipt publication. SQLite spill remains stage-owned.
pub(crate) const PREPARATION_SCHEMA: crate::knowledge_stage::PreparationSchema = crate::knowledge_stage::preparation_schema!(
    table "corpus_capture_pending(collection TEXT NOT NULL,sort0 TEXT NOT NULL,sort1 TEXT NOT NULL,encounter INTEGER NOT NULL,packet BLOB NOT NULL,packet_sha256 BLOB NOT NULL,PRIMARY KEY(collection,sort0,sort1,encounter)) WITHOUT ROWID"
);
struct PendingRow {
    collection: CorpusOriginalCollection,
    sort0: String,
    sort1: String,
    encounter: u64,
    raw: Vec<u8>,
}
fn flush_capture_page(stage: &mut KnowledgeStage<'_>, page: &mut Vec<PendingRow>) -> Result<()> {
    if page.is_empty() {
        return Ok(());
    }
    let bytes = page
        .iter()
        .try_fold(0u64, |n, r| n.checked_add(r.raw.len() as u64))
        .ok_or(Error::Budget("corpus capture page bytes"))?;
    stage.charge_materialized(0, bytes)?;
    stage.with_connection(WritePhase::Sort, |db| {
        let tx = db.transaction()?;
        let mut insert =
            tx.prepare("INSERT INTO corpus_capture_pending VALUES(?1,?2,?3,?4,?5,?6)")?;
        for r in page.iter() {
            insert.execute(params![
                r.collection.as_str(),
                r.sort0,
                r.sort1,
                r.encounter as i64,
                r.raw,
                Digest256::of_bytes(&r.raw).as_bytes().as_slice()
            ])?;
        }
        drop(insert);
        tx.commit()?;
        Ok(())
    })?;
    page.clear();
    Ok(())
}
/// Import exact captured corpus parts into the actual selected original rows.
/// No whole logical payload is reconstructed. The existing finite original
/// carrier limits still apply; this is not a goal-scale carrier/profile grant.
pub fn retain_captured_corpus_original_from_capture(
    stage: &mut KnowledgeStage<'_>,
    capture: &SoftwareCaptureReader,
    source_path: &RelativePath,
    vocab: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CorpusOriginalReceipt> {
    let result = (|| {
        let layout = stage.payload_layout();
        let binding = stage.exact_receipt()?.binding.clone();
        let mut source =
            captured_source(capture, source_path, &binding, limits, deadline, cancelled)?;
        let root_raw = source.read(source_path, crate::legacy::PART_CAP)?;
        let root = json(&root_raw, crate::legacy::PART_CAP)?;
        check_partition_root_size(&root, root_raw.len())?;
        // Derive a page from the existing part and original-page ceilings.
        let page_rows = (crate::legacy::PART_CAP / limits.originals.max_row_bytes)
            .min(crate::knowledge_original_rows::MAX_PAGE_ROWS);
        crate::knowledge_original_rows::page_limits(
            page_rows,
            limits.originals.max_row_bytes,
            crate::legacy::PART_CAP as u64,
        )?;
        stage.create_preparation_tables(PREPARATION_SCHEMA)?;
        let mut page = Vec::new();
        let mut count = 1u64;
        let mut total = 0u64;
        let mut sink = |source: &mut Source<'_>,
                        collection: CorpusOriginalCollection,
                        sort: &[String],
                        value: &Value|
         -> Result<()> {
            check_originals(deadline, cancelled)?;
            indexed_fields(collection, value)?;
            let raw = encode(value, limits.originals.max_row_bytes)?;
            charge_packet(&mut count, &mut total, &raw, limits.originals)?;
            // Reserve each pending row before it can enter a write batch.
            source.charge(raw.len() as u64)?;
            page.push(PendingRow {
                collection,
                sort0: sort.first().cloned().unwrap_or_default(),
                sort1: sort.get(1).cloned().unwrap_or_default(),
                encounter: count - 2,
                raw,
            });
            if page.len() == page_rows {
                flush_capture_page(stage, &mut page)?;
            }
            Ok(())
        };
        let header = visit_captured_rows(&mut source, &root, &mut sink)?;
        drop(sink);
        flush_capture_page(stage, &mut page)?;
        total = total
            .checked_add(header.len() as u64)
            .filter(|n| *n <= limits.originals.max_total_bytes)
            .ok_or(Error::Budget("corpus original header bytes"))?;
        // Header is not a pending row: reserve its one actual final copy.
        source.charge(header.len() as u64)?;
        let header_physical = if layout.uses_carriers() {
            preload_original_carrier(stage, CorpusOriginalCollection::Header, &header)?
        } else {
            header.len() as u64
        };
        stage.charge_materialized(1, header_physical)?;
        stage.with_connection(WritePhase::Finalize, |db| {
            let tx = db.transaction()?;
            tx.execute_batch(META_DDL)?;
            tx.execute_batch(if layout.uses_carriers() {
                ROW_DDL_CARRIER
            } else {
                ROW_DDL
            })?;
            for (_, ddl) in INDEXES {
                tx.execute_batch(ddl)?;
            }
            let mut insert = tx.prepare(if layout.uses_carriers() {
                INSERT_ROW_CARRIER
            } else {
                INSERT_ROW
            })?;
            insert_original_row_with_layout(
                &mut insert,
                CorpusOriginalCollection::Header,
                0,
                &header,
                layout,
            )?;
            drop(insert);
            tx.commit()?;
            Ok(())
        })?;
        let mut collections = Vec::new();
        let mut emitted = 1u64;
        for collection in CorpusOriginalCollection::ROWS {
            let mut cursor = None::<(String, String, i64)>;
            let mut ordinal = 0u64;
            let mut root_hash = order_hash(collection.as_str());
            loop {
                source.check()?;
                let rows = stage.with_connection(WritePhase::Sort, |db| {
                    // Separate initial and continuation seeks; no nullable
                    // cursor OR that would repeatedly scan an old prefix.
                    let first = "SELECT sort0,sort1,encounter,packet,packet_sha256 FROM corpus_capture_pending WHERE collection=?1 ORDER BY sort0,sort1,encounter LIMIT ?2";
                    let next = "SELECT sort0,sort1,encounter,packet,packet_sha256 FROM corpus_capture_pending WHERE collection=?1 AND (sort0,sort1,encounter)>(?2,?3,?4) ORDER BY sort0,sort1,encounter LIMIT ?5";
                    let mut q = db.prepare(if cursor.is_some() { next } else { first })?;
                    let mut scan = if let Some((a,b,c)) = &cursor {
                        q.query(params![collection.as_str(),a,b,c,page_rows as i64])?
                    } else { q.query(params![collection.as_str(),page_rows as i64])? };
                    let mut rows = Vec::new();
                    let mut bytes = 0u64;
                    while let Some(row) = scan.next()? {
                        let raw: Vec<u8> = row.get(3)?;
                        let sha: Vec<u8> = row.get(4)?;
                        bytes = bytes.checked_add(raw.len() as u64)
                            .filter(|n| *n <= crate::legacy::PART_CAP as u64)
                            .ok_or(Error::Budget("corpus pending page bytes"))?;
                        if raw.len() > limits.originals.max_row_bytes
                            || Digest256::of_bytes(&raw).as_bytes().as_slice() != sha {
                            return Err(Error::Invalid("corpus pending row identity"));
                        }
                        rows.push((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,raw));
                    }
                    Ok(rows)
                })?;
                if rows.is_empty() {
                    break;
                }
                let bytes = rows.iter().map(|r| r.3.len() as u64).sum();
                source.charge(bytes)?;
                let physical_bytes = if layout.uses_carriers() {
                    let mut physical = 0u64;
                    for (_, _, _, raw) in &rows {
                        physical = physical
                            .checked_add(preload_original_carrier(stage, collection, raw)?)
                            .ok_or(Error::Budget("corpus captured metadata page bytes"))?;
                    }
                    physical
                } else {
                    bytes
                };
                stage.charge_materialized(rows.len() as u64, physical_bytes)?;
                stage.with_connection(WritePhase::Finalize, |db| {
                    let tx = db.transaction()?;
                    let mut insert = tx.prepare(if layout.uses_carriers() {
                        INSERT_ROW_CARRIER
                    } else {
                        INSERT_ROW
                    })?;
                    for (a, b, c, raw) in &rows {
                        check_originals(deadline, cancelled)?;
                        insert_original_row_with_layout(
                            &mut insert,
                            collection,
                            ordinal,
                            raw,
                            layout,
                        )?;
                        order_item(&mut root_hash, ordinal, raw);
                        ordinal += 1;
                        cursor = Some((a.clone(), b.clone(), *c));
                    }
                    drop(insert);
                    tx.commit()?;
                    Ok(())
                })?;
            }
            emitted = emitted
                .checked_add(ordinal)
                .ok_or(Error::Budget("corpus import EOF rows"))?;
            collections.push(CorpusOriginalCollectionReceipt {
                collection: collection.as_str().into(),
                rows: ordinal,
                ordered_root_sha256: root_hash.finalize().to_hex(),
            });
        }
        if emitted != count {
            return Err(Error::Invalid("corpus import EOF closure"));
        }
        let receipt = captured_receipt(
            &mut source,
            &binding,
            vocab,
            &root_raw,
            &header,
            collections,
            total,
        )?;
        let raw =
            serde_json::to_vec(&receipt).map_err(|_| Error::Invalid("corpus receipt encoding"))?;
        if raw.len() > JsonLimits::default().max_bytes {
            return Err(Error::Budget("corpus original receipt bytes"));
        }
        source.charge(raw.len() as u64)?;
        stage.charge_materialized(1, raw.len() as u64)?;
        stage.with_connection(WritePhase::Finalize, |db| {
            let tx = db.transaction()?;
            tx.execute(
                "INSERT INTO corpus_original_meta VALUES(1,?1)",
                [raw.as_slice()],
            )?;
            tx.execute_batch("DROP TABLE corpus_capture_pending")?;
            tx.commit()?;
            Ok(())
        })?;
        check_originals(deadline, cancelled)?;
        Ok(receipt)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Retain only a real opaque native composition result. Capture metadata is
/// absent: the public output member and original authored/software proofs are
/// separate identities, joined by the selected release holder.
pub fn prepare_native_corpus_original(
    projection: &crate::source_corpus::NativeCorpusProjection,
    source_path: &RelativePath,
    binding: &SourceBinding,
    vocab: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CorpusOriginalPlan> {
    check_originals(deadline, cancelled)?;
    limits.originals.validate()?;
    let producer = projection.receipt();
    crate::source_corpus::validate_receipt(producer)?;
    let mut actual_binding = projection.source_binding().clone();
    let mut target_binding = binding.clone();
    // Planner and final target have independently computed projection roots.
    // Every source-selection field must nevertheless belong to the same cut.
    actual_binding.projection_root_sha256.clear();
    target_binding.projection_root_sha256.clear();
    if serde_json::to_vec(&actual_binding)
        .map_err(|_| Error::Invalid("native corpus source binding"))?
        != serde_json::to_vec(&target_binding)
            .map_err(|_| Error::Invalid("native corpus target binding"))?
    {
        return Err(Error::Invalid(
            "native corpus actual source-selection binding",
        ));
    }
    if !binding.complete
        || limits.max_members == 0
        || limits.max_members > 65_536
        || limits.max_work_bytes == 0
        || limits.max_work_bytes > crate::knowledge_original_rows::MAX_COLD_WORK
        || producer.source_cut != binding.source_cut
        || producer.source_membership_sha256 != binding.membership_root
        || producer.descriptor_sha256 != vocab.descriptor_sha256
    {
        return Err(Error::Invalid(
            "native corpus original producer binding/limits",
        ));
    }
    let mut packet_limits = limits.originals;
    packet_limits.max_total_bytes = packet_limits
        .max_total_bytes
        .min(limits.max_work_bytes.saturating_sub(producer.output_bytes));
    let (header, rows, total) =
        original_packets(projection.value(), packet_limits, deadline, cancelled)?;
    let member = CorpusOriginalMember {
        path: source_path.as_str().into(),
        sha256: producer.output_sha256.clone(),
        size_bytes: producer.output_bytes,
    };
    let mut member_root = Digest256Hasher::new();
    text(&mut member_root, "tos-native-corpus-output-members-v1");
    text(&mut member_root, &member.path);
    member_root.update(&member.size_bytes.to_be_bytes());
    member_root.update(
        Digest256::from_hex(&member.sha256)
            .map_err(|_| Error::Invalid("native corpus output member SHA"))?
            .as_bytes(),
    );
    let mut receipt = CorpusOriginalReceipt {
        profile: NATIVE_CORPUS_ORIGINAL_PROFILE.into(),
        descriptor_sha256: vocab.descriptor_sha256.clone(),
        source_cut: binding.source_cut.clone(),
        membership_root: binding.membership_root.clone(),
        origin: CorpusOriginalOrigin {
            profile: "native-corpus-producer-v1".into(),
            source_git_commit: None,
            source_git_tree: None,
            capture_manifest_sha256: None,
            native_producer: Some(producer.clone()),
            source_path: member.path.clone(),
            source_sha256: member.sha256.clone(),
            source_size_bytes: member.size_bytes,
            members: vec![member],
            member_root_sha256: member_root.finalize().to_hex(),
        },
        header_sha256: Digest256::of_bytes(&header).to_hex(),
        collections: rows
            .iter()
            .map(|(c, rows)| CorpusOriginalCollectionReceipt {
                collection: c.as_str().into(),
                rows: rows.len() as u64,
                ordered_root_sha256: ordered_root(c.as_str(), rows),
            })
            .collect(),
        total_bytes: total,
        component_root_sha256: String::new(),
    };
    receipt.component_root_sha256 = component_root(&receipt)?;
    validate_receipt(&receipt)?;
    Ok(CorpusOriginalPlan {
        binding: binding.clone(),
        receipt,
        header,
        rows,
    })
}
