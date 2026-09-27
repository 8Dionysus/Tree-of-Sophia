//! Exact selected public corpus compatibility input. Reads only explicitly
//! selected capture members; it never claims an authored native builder result.
use crate::knowledge_corpus_original::*;
use crate::{Error, NavigationOriginalLimits, QueryVocabulary, Result, SourceBinding};
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
struct Source<'a> {
    reader: &'a SoftwareCaptureReader,
    root: &'a RelativePath,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    work: u64,
    members: BTreeMap<String, CorpusOriginalMember>,
}
impl Source<'_> {
    fn check(&self) -> Result<()> {
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
        self.work = self
            .work
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_work_bytes)
            .ok_or(Error::Budget("corpus capture work"))?;
        Ok(())
    }
    fn read(&mut self, path: &RelativePath, cap: usize) -> Result<Vec<u8>> {
        self.check()?;
        let selection = self
            .reader
            .select_components(&[path.clone()])
            .map_err(|e| Error::Source(e.to_string()))?;
        let member = selection
            .member(path)
            .ok_or(Error::Invalid("corpus captured member"))?;
        if !self.members.contains_key(path.as_str())
            && self.members.len() >= self.limits.max_members
        {
            return Err(Error::Budget("corpus capture member count"));
        }
        let raw = self
            .reader
            .read_selected_component(&selection, path, cap as u64, self.deadline, self.cancelled)
            .map_err(|e| Error::Source(e.to_string()))?;
        self.charge(raw.len() as u64)?;
        self.members.insert(
            path.as_str().into(),
            CorpusOriginalMember {
                path: path.as_str().into(),
                size_bytes: member.size_bytes,
                sha256: member.sha256.to_hex(),
            },
        );
        Ok(raw)
    }
    fn part(&mut self, d: &Value, prefix: &str) -> Result<Vec<u8>> {
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
        let raw = crate::legacy::decode_partition_part(
            &stored,
            kind,
            size as usize,
            decoded as usize,
            sha,
            decoded_sha,
        )?;
        self.charge(raw.len() as u64)?;
        Ok(raw)
    }
    fn walk(
        &mut self,
        d: &Value,
        prefix: &str,
        key: &Value,
        root_count: u64,
        sink: &mut dyn FnMut(&str, Value) -> Result<()>,
    ) -> Result<u64> {
        let raw = self.part(d, prefix)?;
        let wanted = number(d, "count")?;
        if string(d, "kind")? == "index" {
            let v = json(&raw, crate::legacy::INDEX_CAP)?;
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
            let row = json(line, self.limits.originals.max_row_bytes)?;
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
            sink(id, value.clone())?;
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
/// Exact existing captured member origin, distinct from the selected authored
/// cut. Caller-selected release composition is checked again before retention.
pub fn prepare_captured_corpus_original(
    capture: &SoftwareCaptureReader,
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
    let mut source = Source {
        reader: capture,
        root: source_path,
        limits,
        deadline,
        cancelled,
        work: 0,
        members: BTreeMap::new(),
    };
    let raw = source.read(source_path, crate::legacy::PART_CAP)?;
    let root = json(&raw, crate::legacy::PART_CAP)?;
    let mut payload = root.clone();
    if root["schema_version"] == "tos_partitioned_projection_v1" {
        if raw.len() > crate::legacy::ROOT_CAP {
            return Err(Error::Budget("corpus partition root bytes"));
        }
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
            || root["limits"]
                != serde_json::json!({"root_bytes":crate::legacy::ROOT_CAP,"index_bytes":crate::legacy::INDEX_CAP,
                "part_bytes":crate::legacy::PART_CAP,"key_bytes":4096})
        {
            return Err(Error::Invalid("corpus partition logical profile"));
        }
        payload = root["header"].clone();
        let specs = root["collections"]
            .as_object()
            .ok_or(Error::Invalid("corpus partition collections"))?;
        let required = [
            ("nodes", serde_json::json!("node_id"), vec!["source_path"]),
            ("resources", serde_json::json!("path"), vec!["path"]),
            ("manifests", serde_json::json!("path"), vec!["path"]),
            ("relation_packs", serde_json::json!("pack_id"), vec!["path"]),
            (
                "relation_edges",
                serde_json::json!(["pack_id", "edge_id"]),
                vec!["pack_id", "edge_id"],
            ),
            (
                "source_navigation/nodes",
                serde_json::json!("node_id"),
                vec!["node_id"],
            ),
            (
                "source_navigation/edges",
                serde_json::json!("edge_id"),
                vec!["edge_id"],
            ),
            (
                "source_navigation/rights",
                serde_json::json!("rights_id"),
                vec!["rights_id"],
            ),
        ];
        if specs.len() != required.len() + usize::from(specs.contains_key("diagnostics"))
            || required.iter().any(|(n, _, _)| !specs.contains_key(*n))
        {
            return Err(Error::Invalid("corpus partition collection closure"));
        }
        let mut kept_bytes = 0u64;
        let mut kept_rows = 1u64;
        for (name, spec) in specs {
            keys(spec, &["key_field", "order_fields", "root"])?;
            let (key, order) = if name == "diagnostics" {
                (serde_json::json!([]), Vec::new())
            } else {
                let (_, key, order) = required
                    .iter()
                    .find(|(n, _, _)| *n == name)
                    .ok_or(Error::Invalid("corpus partition collection"))?;
                (key.clone(), order.clone())
            };
            if spec["key_field"] != key || spec["order_fields"] != serde_json::json!(order) {
                return Err(Error::Invalid("corpus partition ordering policy"));
            }
            let mut rows = Vec::<(Vec<String>, Value)>::new();
            let keep = !name.starts_with("source_navigation/") && name != "diagnostics";
            let mut sink = |_: &str, v: Value| -> Result<()> {
                if !keep {
                    return Ok(());
                }
                let raw = encode(&v, limits.originals.max_row_bytes)?;
                kept_bytes = kept_bytes
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= limits.originals.max_total_bytes)
                    .ok_or(Error::Budget("corpus partition logical bytes"))?;
                kept_rows = kept_rows
                    .checked_add(1)
                    .filter(|n| *n <= limits.originals.max_rows)
                    .ok_or(Error::Budget("corpus partition logical rows"))?;
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
                rows.push((sort, v));
                Ok(())
            };
            // Unique hashed keys plus the complete root count and positional
            // range prove dense diagnostic positions, including unretained rows.
            source.walk(
                &spec["root"],
                "",
                &key,
                number(&spec["root"], "count")?,
                &mut sink,
            )?;
            if keep {
                rows.sort_by(|a, b| a.0.cmp(&b.0));
                payload[name] = Value::Array(rows.into_iter().map(|(_, v)| v).collect());
            }
        }
    }
    if payload["schema_version"] != "tos_corpus_index_v1" {
        return Err(Error::Invalid("captured corpus logical schema"));
    }
    let mut rows = Vec::new();
    let mut total = 0u64;
    let mut count = 1u64;
    for collection in CorpusOriginalCollection::ROWS {
        let values = payload
            .get(collection.as_str())
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("corpus original required array"))?;
        let mut encoded = Vec::with_capacity(values.len());
        for value in values {
            source.check()?;
            super::knowledge_corpus_original::indexed_fields(collection, value)?;
            let raw = encode(value, limits.originals.max_row_bytes)?;
            count = count
                .checked_add(1)
                .filter(|n| *n <= limits.originals.max_rows)
                .ok_or(Error::Budget("corpus original rows"))?;
            total = total
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= limits.originals.max_total_bytes)
                .ok_or(Error::Budget("corpus original bytes"))?;
            source.charge(raw.len() as u64)?;
            encoded.push(raw);
        }
        rows.push((collection, encoded));
    }
    let mut header = payload
        .as_object()
        .ok_or(Error::Invalid("corpus original header object"))?
        .clone();
    for field in [
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
    ] {
        header.remove(field);
    }
    let header = encode(&Value::Object(header), limits.originals.max_row_bytes)?;
    total = total
        .checked_add(header.len() as u64)
        .filter(|n| *n <= limits.originals.max_total_bytes)
        .ok_or(Error::Budget("corpus original header bytes"))?;
    source.charge(header.len() as u64)?;
    let collections = rows
        .iter()
        .map(|(c, r)| CorpusOriginalCollectionReceipt {
            collection: c.as_str().into(),
            rows: r.len() as u64,
            ordered_root_sha256: ordered_root(c.as_str(), r),
        })
        .collect();
    let members = source.members.into_values().collect::<Vec<_>>();
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
    let pin = capture.selection();
    let mut receipt = CorpusOriginalReceipt {
        profile: CORPUS_ORIGINAL_PROFILE.into(),
        descriptor_sha256: vocab.descriptor_sha256.clone(),
        source_cut: binding.source_cut.clone(),
        membership_root: binding.membership_root.clone(),
        origin: CapturedCorpusOrigin {
            profile: "captured-public-corpus-v1".into(),
            source_git_commit: pin.source_git_commit.clone(),
            source_git_tree: pin.source_git_tree.clone(),
            capture_manifest_sha256: pin.capture_manifest_sha256.to_hex(),
            source_path: source_path.as_str().into(),
            source_sha256: Digest256::of_bytes(&raw).to_hex(),
            source_size_bytes: raw.len() as u64,
            members,
            member_root_sha256: member_root.finalize().to_hex(),
        },
        header_sha256: Digest256::of_bytes(&header).to_hex(),
        collections,
        component_root_sha256: String::new(),
        total_bytes: total,
    };
    receipt.component_root_sha256 = super::knowledge_corpus_original::component_root(&receipt)?;
    super::knowledge_corpus_original::validate_receipt(&receipt)?;
    Ok(CapturedCorpusOriginalPlan {
        binding: binding.clone(),
        receipt,
        header,
        rows,
    })
}
