//! Addressed immutable roots used by the existing Claim addition caller.
//! COW parts are unselected candidates. No selected root is written here.
use super::source_claim_publication_bytes as bytes;
use flate2::{Compression, GzBuilder, read::MultiGzDecoder};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tos_compiler::{Error, Result};

const ROOT_BYTES: usize = 262144;
const INDEX_BYTES: usize = 131072;
const PART_BYTES: usize = 8388608;

#[derive(Clone, Copy)]
pub(super) struct MutationLimits {
    pub changes: usize,
    pub input: usize,
    pub parts: usize,
    pub stored: usize,
    pub decoded: usize,
    pub keys: usize,
    pub written_parts: usize,
    pub written_decoded: usize,
    pub written_stored: usize,
    pub result: usize,
}
impl Default for MutationLimits {
    fn default() -> Self {
        Self {
            changes: 4096,
            input: 16777216,
            parts: 256,
            stored: 16777216,
            decoded: 16777216,
            keys: 4096,
            written_parts: 256,
            written_decoded: 16777216,
            written_stored: 16777216,
            result: 16777216,
        }
    }
}
#[derive(Default)]
pub(super) struct Usage {
    changes: usize,
    input: usize,
    parts: usize,
    stored: usize,
    decoded: usize,
    keys: usize,
    written_parts: usize,
    written_decoded: usize,
    written_stored: usize,
    result: usize,
    hits: usize,
}
fn take(value: &mut usize, amount: usize, limit: usize) -> Result<()> {
    *value = value
        .checked_add(amount)
        .filter(|n| *n <= limit)
        .ok_or(Error::Budget("Claim raw projection work"))?;
    Ok(())
}
pub(super) struct Snapshot {
    pub raw: Vec<u8>,
    pub path: PathBuf,
    pub manifest: Value,
}
impl Snapshot {
    pub fn parse(raw: Vec<u8>, path: PathBuf, expected: &str) -> Result<Self> {
        bytes::sha(expected)?;
        if bytes::digest(&raw) != expected || !path.is_absolute() {
            return Err(Error::Invalid("Claim immutable root binding"));
        }
        let manifest = bytes::parse(&raw, ROOT_BYTES)?;
        if manifest["schema_version"] != "tos_partitioned_projection_v1"
            || manifest["logical_schema"] != manifest["header"]["schema_version"]
            || !manifest["header"].is_object()
            || manifest.as_object().map(|o| o.len()) != Some(5)
            || manifest["limits"]
                != json!({"root_bytes":ROOT_BYTES,"index_bytes":INDEX_BYTES,
                "part_bytes":PART_BYTES,"key_bytes":4096})
        {
            return Err(Error::Invalid("Claim partitioned root metadata"));
        }
        let snapshot = Self {
            raw,
            path,
            manifest,
        };
        let collections = snapshot.manifest["collections"]
            .as_object()
            .filter(|o| !o.is_empty())
            .ok_or(Error::Invalid("Claim root collections"))?;
        for (name, spec) in collections {
            if name.is_empty()
                || spec.as_object().map(|o| o.len()) != Some(3)
                || !spec["key_field"].is_string()
                || spec["order_fields"] != json!([bytes::text(spec, "key_field")?])
            {
                return Err(Error::Invalid("Claim addressed root collection profile"));
            }
            snapshot.descriptor(&spec["root"], "")?;
        }
        Ok(snapshot)
    }
    fn descriptor(&self, d: &Value, prefix: &str) -> Result<PathBuf> {
        if d.as_object().map(|o| o.len()) != Some(8)
            || d["prefix"] != prefix
            || prefix.len() > 64
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid("Claim raw part descriptor"));
        }
        let kind = bytes::text(d, "kind")?;
        let (bound, suffix) = match kind {
            "data" => (PART_BYTES, ".jsonl.gz"),
            "index" if prefix.len() < 64 => (INDEX_BYTES, ".index.json"),
            _ => return Err(Error::Invalid("Claim raw part kind/depth")),
        };
        let sha = bytes::text(d, "sha256")?;
        bytes::sha(sha)?;
        bytes::sha(bytes::text(d, "decoded_sha256")?)?;
        if bytes::number(d, "size_bytes")? > (bound + 65536) as u64
            || bytes::number(d, "decoded_bytes")? > bound as u64
            || (kind == "index"
                && (d["size_bytes"] != d["decoded_bytes"] || d["sha256"] != d["decoded_sha256"]))
        {
            return Err(Error::Invalid("Claim raw descriptor byte bounds"));
        }
        bytes::number(d, "count")?;
        let stem = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or(Error::Invalid("Claim raw namespace basename"))?;
        let relative = format!("{stem}.parts/{}/{sha}{suffix}", &sha[..2]);
        if d["path"] != relative {
            return Err(Error::Invalid("Claim raw part namespace"));
        }
        Ok(PathBuf::from(relative))
    }
}
type PartStamp = (u64, u64, u64, i64, i64, i64, i64);
fn part_stamp(m: &std::fs::Metadata) -> PartStamp {
    use std::os::unix::fs::MetadataExt;
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
struct PartPin {
    path: PathBuf,
    file: File,
    stamp: PartStamp,
}
impl PartPin {
    fn verify(&self) -> Result<()> {
        let named = tos_fd_open::open_absolute_regular(&self.path, self.stamp.2)
            .map_err(|e| Error::Source(e.to_string()))?;
        if part_stamp(&self.file.metadata()?) != self.stamp
            || part_stamp(&named.metadata()?) != self.stamp
        {
            return Err(Error::Invalid("source addressed part identity changed"));
        }
        Ok(())
    }
}

pub(super) struct Roots {
    pub snapshots: BTreeMap<String, Snapshot>,
    limits: MutationLimits,
    usage: Usage,
    cache: BTreeMap<(PathBuf, String, Vec<u8>), Arc<[u8]>>,
    // Only read-only SourceRead retains named/held part identity. Mutation
    // callers keep the existing COW candidate behavior through new().
    read_pins: Option<BTreeMap<PathBuf, PartPin>>,
}
impl Roots {
    pub fn new(snapshots: BTreeMap<String, Snapshot>, limits: MutationLimits) -> Result<Self> {
        let mut this = Self {
            snapshots,
            limits,
            usage: Usage::default(),
            cache: BTreeMap::new(),
            read_pins: None,
        };
        for snapshot in this.snapshots.values() {
            take(&mut this.usage.decoded, snapshot.raw.len(), limits.decoded)?;
        }
        Ok(this)
    }
    pub fn new_retained(
        snapshots: BTreeMap<String, Snapshot>,
        limits: MutationLimits,
    ) -> Result<Self> {
        let mut result = Self::new(snapshots, limits)?;
        result.read_pins = Some(BTreeMap::new());
        Ok(result)
    }
    fn load(&mut self, role: &str, d: &Value, prefix: &str) -> Result<Arc<[u8]>> {
        let snapshot = self
            .snapshots
            .get(role)
            .ok_or(Error::Invalid("Claim raw root role"))?;
        // Validate the full descriptor even on a hit. Cache is scoped to one
        // exact namespace and prefix, never a live authored-source cache.
        let relative = snapshot.descriptor(d, prefix)?;
        let key = (
            snapshot.path.clone(),
            prefix.to_owned(),
            bytes::canonical(d, 65536)?,
        );
        if let Some(raw) = self.cache.get(&key) {
            if let Some(pins) = &self.read_pins {
                let path = snapshot.path.parent().unwrap().join(&relative);
                pins.get(&path)
                    .ok_or(Error::Invalid("source part pin absent"))?
                    .verify()?;
            }
            self.usage.hits += 1;
            return Ok(Arc::clone(raw));
        }
        let stored = bytes::number(d, "size_bytes")? as usize;
        let decoded = bytes::number(d, "decoded_bytes")? as usize;
        take(&mut self.usage.parts, 1, self.limits.parts)?;
        take(&mut self.usage.stored, stored + 1, self.limits.stored)?;
        take(&mut self.usage.decoded, decoded + 1, self.limits.decoded)?;
        if d["kind"] == "data" {
            take(
                &mut self.usage.keys,
                bytes::number(d, "count")? as usize,
                self.limits.keys,
            )?;
        }
        let base = tos_fd_open::open_absolute_directory(
            snapshot
                .path
                .parent()
                .ok_or(Error::Invalid("Claim raw namespace parent"))?,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        let mut directory = base;
        let mut components = relative.components().peekable();
        let mut file: Option<File> = None;
        while let Some(component) = components.next() {
            if components.peek().is_none() {
                file = Some(
                    tos_fd_open::open_regular_at(&directory, Path::new(component.as_os_str()))
                        .map_err(|e| Error::Source(e.to_string()))?,
                );
            } else {
                directory =
                    tos_fd_open::open_directory_at(&directory, Path::new(component.as_os_str()))
                        .map_err(|e| Error::Source(e.to_string()))?;
            }
        }
        let file = file.ok_or(Error::Invalid("Claim raw part path"))?;
        let before = file.metadata()?;
        if before.len() != stored as u64 {
            return Err(Error::Invalid("Claim raw physical size"));
        }
        if self.read_pins.is_some() {
            use std::os::unix::fs::MetadataExt;
            if before.uid() != rustix::process::getuid().as_raw() || before.mode() & 0o022 != 0 {
                return Err(Error::Invalid("source part owner or mode"));
            }
        }
        let mut encoded = Vec::new();
        (&file)
            .take((stored + 1) as u64)
            .read_to_end(&mut encoded)?;
        if encoded.len() != stored || bytes::digest(&encoded) != d["sha256"].as_str().unwrap_or("")
        {
            return Err(Error::Invalid("Claim raw stored identity"));
        }
        let raw = if d["kind"] == "data" {
            let mut raw = Vec::new();
            MultiGzDecoder::new(encoded.as_slice())
                .take((decoded + 1) as u64)
                .read_to_end(&mut raw)?;
            raw
        } else {
            encoded
        };
        if raw.len() != decoded || bytes::digest(&raw) != d["decoded_sha256"].as_str().unwrap_or("")
        {
            return Err(Error::Invalid("Claim raw decoded identity"));
        }
        if let Some(pins) = &mut self.read_pins {
            if part_stamp(&file.metadata()?) != part_stamp(&before) {
                return Err(Error::Invalid("source part changed during read"));
            }
            let path = snapshot.path.parent().unwrap().join(&relative);
            if let Some(pin) = pins.get(&path) {
                pin.verify()?;
                if part_stamp(&before) != pin.stamp {
                    return Err(Error::Invalid("source part reread inode changed"));
                }
            } else {
                let pin = PartPin {
                    file,
                    path: path.clone(),
                    stamp: part_stamp(&before),
                };
                pin.verify()?;
                pins.insert(path, pin);
            }
        }
        let raw: Arc<[u8]> = raw.into();
        self.cache.insert(key, Arc::clone(&raw));
        Ok(raw)
    }
    fn children(&mut self, role: &str, d: &Value, prefix: &str) -> Result<BTreeMap<String, Value>> {
        let raw = self.load(role, d, prefix)?;
        let index = bytes::parse(&raw, INDEX_BYTES)?;
        if index.as_object().map(|o| o.len()) != Some(4)
            || index["schema_version"] != "tos_projection_partition_index_v1"
            || index["prefix"] != prefix
            || index["count"] != d["count"]
        {
            return Err(Error::Invalid("Claim raw partition index"));
        }
        let children = index["children"]
            .as_object()
            .filter(|o| !o.is_empty())
            .ok_or(Error::Invalid("Claim raw children"))?;
        let mut count = 0u64;
        for (digit, child) in children {
            if digit.len() != 1
                || !digit
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Error::Invalid("Claim raw branch digit"));
            }
            self.snapshots[role].descriptor(child, &format!("{prefix}{digit}"))?;
            count = count
                .checked_add(bytes::number(child, "count")?)
                .ok_or(Error::Budget("Claim raw index count"))?;
        }
        if count != bytes::number(d, "count")? {
            return Err(Error::Invalid("Claim raw aggregate count"));
        }
        Ok(children
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
    fn rows(
        &mut self,
        role: &str,
        collection: &str,
        d: &Value,
        prefix: &str,
    ) -> Result<BTreeMap<String, Value>> {
        let raw = self.load(role, d, prefix)?;
        let field = bytes::text(
            &self.snapshots[role].manifest["collections"][collection],
            "key_field",
        )?
        .to_owned();
        let mut rows = BTreeMap::new();
        let mut previous: Option<String> = None;
        for line in raw.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            if rows.len() >= bytes::number(d, "count")? as usize {
                return Err(Error::Invalid("Claim raw excess rows"));
            }
            let row = bytes::parse(line, PART_BYTES)?;
            let key = bytes::text(&row, "key")?.to_owned();
            if row.as_object().map(|o| o.len()) != Some(2)
                || previous.as_ref().is_some_and(|p| p >= &key)
                || !bytes::digest(key.as_bytes()).starts_with(prefix)
                || row["value"][&field] != key
            {
                return Err(Error::Invalid("Claim raw row order/identity/prefix"));
            }
            previous = Some(key.clone());
            rows.insert(key, row["value"].clone());
        }
        if rows.len() != bytes::number(d, "count")? as usize {
            return Err(Error::Invalid("Claim raw row count"));
        }
        Ok(rows)
    }
    pub fn get(&mut self, role: &str, collection: &str, key: &str) -> Result<Option<Value>> {
        if key.is_empty() || key.len() > 4096 {
            return Err(Error::Invalid("Claim raw addressed key"));
        }
        let mut d = self
            .snapshots
            .get(role)
            .and_then(|s| s.manifest["collections"].get(collection))
            .ok_or(Error::Invalid("Claim raw collection"))?["root"]
            .clone();
        let hash = bytes::digest(key.as_bytes());
        let mut prefix = String::new();
        while d["kind"] == "index" {
            let digit = &hash[prefix.len()..prefix.len() + 1];
            let children = self.children(role, &d, &prefix)?;
            let Some(child) = children.get(digit) else {
                return Ok(None);
            };
            d = child.clone();
            prefix.push_str(digit);
        }
        Ok(self.rows(role, collection, &d, &prefix)?.remove(key))
    }
    /// Revalidate the exact addressed parts consumed by a retained snapshot.
    /// Retained root bytes were authenticated by Snapshot::parse and the source
    /// vector; its locator is not a selected-current projection pointer file.
    /// No mutation/current-publication caller can enter this read-only route.
    /// All encoded/decoded rereads remain inside the same cumulative budgets.
    pub fn verify_retained_reads(
        &mut self,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        let check = || {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed)
                || std::time::Instant::now() >= deadline
            {
                Err(Error::Invalid("source part read interrupted"))
            } else {
                Ok(())
            }
        };
        for pin in self
            .read_pins
            .as_ref()
            .ok_or(Error::Invalid("retained read guard mode absent"))?
            .values()
        {
            check()?;
            pin.verify()?;
        }
        let selected = std::mem::take(&mut self.cache);
        for ((path, prefix, descriptor), expected) in selected {
            check()?;
            let role = self
                .snapshots
                .iter()
                .find(|(_, snapshot)| snapshot.path == path)
                .map(|(role, _)| role.clone())
                .ok_or(Error::Invalid("source root cache namespace"))?;
            let descriptor = bytes::parse(&descriptor, 65536)?;
            let actual = self.load(&role, &descriptor, &prefix)?;
            if actual.as_ref() != expected.as_ref() {
                return Err(Error::Invalid("source root selected part changed"));
            }
        }
        for pin in self.read_pins.as_ref().unwrap().values() {
            check()?;
            pin.verify()?;
        }
        check()
    }
    pub fn accounting(&self) -> Value {
        json!({"opened_parts":self.usage.parts,"stored_read_bytes":self.usage.stored,
            "decoded_bytes":self.usage.decoded,"keys":self.usage.keys,"cache_hits":self.usage.hits,
            "cached_bytes":self.cache.values().map(|v|v.len()).sum::<usize>()})
    }
    pub fn get_with_material(
        &mut self,
        role: &str,
        collection: &str,
        key: &str,
    ) -> Result<Option<(Value, Vec<u8>)>> {
        let Some(value) = self.get(role, collection, key)? else {
            return Ok(None);
        };
        let mut descriptor =
            self.snapshots[role].manifest["collections"][collection]["root"].clone();
        let hash = bytes::digest(key.as_bytes());
        let mut prefix = String::new();
        while descriptor["kind"] == "index" {
            let digit = &hash[prefix.len()..prefix.len() + 1];
            descriptor = self
                .children(role, &descriptor, &prefix)?
                .remove(digit)
                .ok_or(Error::Invalid("Claim ordered raw lookup changed"))?;
            prefix.push_str(digit);
        }
        let raw = self.load(role, &descriptor, &prefix)?;
        let limits = tos_foundation::JsonLimits::new(PART_BYTES, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Claim ordered raw JSON limits"))?;
        for line in raw.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            let document =
                tos_foundation::parse_json(line, tos_foundation::JsonMode::PublishedStrict, limits)
                    .map_err(|e| Error::Source(e.to_string()))?;
            if document
                .root()
                .object_get("key")
                .and_then(tos_foundation::JsonValue::as_str)
                == Some(key)
            {
                let payload = document
                    .root()
                    .object_get("value")
                    .ok_or(Error::Invalid("Claim ordered raw value"))?;
                let material = tos_foundation::emit_value_preserved_json(payload, limits)
                    .map_err(|e| Error::Source(e.to_string()))?;
                return Ok(Some((value, material)));
            }
        }
        Err(Error::Invalid(
            "Claim ordered raw row missing after verified lookup",
        ))
    }
}

pub(super) struct Change {
    pub collection: String,
    pub key: String,
    pub before_sha256: Option<String>,
    pub after: Value,
}
impl Roots {
    pub fn stage(
        &mut self,
        role: &str,
        changes: Vec<Change>,
        header: Option<Value>,
        target: usize,
    ) -> Result<Snapshot> {
        if !(256..=PART_BYTES).contains(&target) {
            return Err(Error::Invalid("Claim COW target size"));
        }
        let original = self
            .snapshots
            .get(role)
            .ok_or(Error::Invalid("Claim COW root"))?;
        let path = original.path.clone();
        let before = bytes::digest(&original.raw);
        let mut manifest = original.manifest.clone();
        if let Some(header) = header {
            if header["schema_version"] != manifest["logical_schema"] || !header.is_object() {
                return Err(Error::Invalid("Claim COW logical header"));
            }
            take(
                &mut self.usage.input,
                bytes::canonical(&header, ROOT_BYTES)?.len(),
                self.limits.input,
            )?;
            manifest["header"] = header;
        }
        let mut grouped: BTreeMap<String, BTreeMap<String, Change>> = BTreeMap::new();
        for change in changes {
            take(&mut self.usage.changes, 1, self.limits.changes)?;
            if change.key.is_empty()
                || change.key.len() > 4096
                || manifest["collections"].get(&change.collection).is_none()
                || change.after
                    [bytes::text(&manifest["collections"][&change.collection], "key_field")?]
                    != change.key
            {
                return Err(Error::Invalid("Claim COW addressed change"));
            }
            if let Some(sha) = &change.before_sha256 {
                bytes::sha(sha)?;
            }
            let frame = json!({"collection":change.collection,"key":change.key,
                "before":{"present":change.before_sha256.is_some(),"sha256":change.before_sha256},
                "after":{"present":true,"value":change.after,
                    "sha256":bytes::row_digest(&change.after,PART_BYTES)?}});
            take(
                &mut self.usage.input,
                bytes::canonical(&frame, PART_BYTES)?.len(),
                self.limits.input,
            )?;
            if grouped
                .entry(change.collection.clone())
                .or_default()
                .insert(change.key.clone(), change)
                .is_some()
            {
                return Err(Error::Invalid("Claim COW duplicate target"));
            }
        }
        let mut parts = BTreeMap::new();
        for (collection, updates) in grouped {
            let root = manifest["collections"][&collection]["root"].clone();
            manifest["collections"][&collection]["root"] = self.walk(
                role,
                &collection,
                Some(&root),
                "",
                updates,
                target,
                &mut parts,
            )?;
        }
        let root = bytes::canonical(&manifest, ROOT_BYTES)?;
        take(&mut self.usage.result, root.len(), self.limits.result)?;
        let after = bytes::digest(&root);
        let snapshot = Snapshot::parse(root, path.clone(), &after)?;
        // Immutable input roots are not selected files. Their exact retained
        // bytes remain the fence; current source files have their own guard.
        if bytes::digest(&self.snapshots[role].raw) != before {
            return Err(Error::Invalid("Claim COW immutable baseline changed"));
        }
        for (relative, raw) in parts {
            self.install(&path, &relative, &raw)?;
        }
        Ok(snapshot)
    }
    fn emit(
        &mut self,
        role: &str,
        raw: Vec<u8>,
        kind: &str,
        prefix: &str,
        count: usize,
        parts: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<Value> {
        if raw.len()
            > if kind == "data" {
                PART_BYTES
            } else {
                INDEX_BYTES
            }
        {
            return Err(Error::Budget("Claim COW part format"));
        }
        let stored = if kind == "data" {
            let mut encoder = GzBuilder::new()
                .mtime(0)
                .operating_system(255)
                .write(Vec::new(), Compression::new(6));
            encoder.write_all(&raw)?;
            encoder.finish()?
        } else {
            raw.clone()
        };
        let digest = bytes::digest(&stored);
        let stem = self.snapshots[role]
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or(Error::Invalid("Claim COW namespace"))?;
        let suffix = if kind == "data" {
            ".jsonl.gz"
        } else {
            ".index.json"
        };
        let relative = PathBuf::from(format!("{stem}.parts/{}/{digest}{suffix}", &digest[..2]));
        let descriptor = json!({"kind":kind,"prefix":prefix,"path":relative.to_str(),"sha256":digest,
            "size_bytes":stored.len(),"decoded_bytes":raw.len(),"decoded_sha256":bytes::digest(&raw),"count":count});
        if !parts.contains_key(&relative) {
            take(&mut self.usage.written_parts, 1, self.limits.written_parts)?;
            take(
                &mut self.usage.written_decoded,
                raw.len(),
                self.limits.written_decoded,
            )?;
            take(
                &mut self.usage.written_stored,
                stored.len(),
                self.limits.written_stored,
            )?;
            parts.insert(relative, stored);
        }
        Ok(descriptor)
    }
    fn index(
        &mut self,
        role: &str,
        children: BTreeMap<String, Value>,
        prefix: &str,
        parts: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<Value> {
        let mut count = 0usize;
        for child in children.values() {
            count = count
                .checked_add(bytes::number(child, "count")? as usize)
                .ok_or(Error::Budget("Claim COW child count"))?;
        }
        let raw = bytes::canonical(
            &json!({"schema_version":"tos_projection_partition_index_v1",
            "prefix":prefix,"count":count,"children":children}),
            INDEX_BYTES,
        )?;
        self.emit(role, raw, "index", prefix, count, parts)
    }
    fn partition(
        &mut self,
        role: &str,
        rows: BTreeMap<String, Value>,
        prefix: &str,
        target: usize,
        parts: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<Value> {
        let mut raw = Vec::new();
        for (key, value) in &rows {
            let row = bytes::canonical(&json!({"key":key,"value":value}), PART_BYTES)?;
            if raw
                .len()
                .checked_add(row.len())
                .is_none_or(|n| n > self.limits.written_decoded)
            {
                return Err(Error::Budget("Claim COW partition construction"));
            }
            raw.extend_from_slice(&row);
        }
        if rows.len() <= 1 || prefix.len() >= 2 && raw.len() <= target {
            return self.emit(role, raw, "data", prefix, rows.len(), parts);
        }
        drop(raw);
        if prefix.len() >= 64 {
            return Err(Error::Invalid("Claim COW key hash depth"));
        }
        let mut groups: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
        for (key, value) in rows {
            let hash = bytes::digest(key.as_bytes());
            groups
                .entry(hash[prefix.len()..prefix.len() + 1].to_owned())
                .or_default()
                .insert(key, value);
        }
        let mut children = BTreeMap::new();
        for (digit, rows) in groups {
            let child = self.partition(role, rows, &format!("{prefix}{digit}"), target, parts)?;
            children.insert(digit, child);
        }
        self.index(role, children, prefix, parts)
    }
    fn walk(
        &mut self,
        role: &str,
        collection: &str,
        d: Option<&Value>,
        prefix: &str,
        updates: BTreeMap<String, Change>,
        target: usize,
        parts: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<Value> {
        if let Some(d) = d.filter(|d| d["kind"] == "index") {
            let mut children = self.children(role, d, prefix)?;
            let mut groups: BTreeMap<String, BTreeMap<String, Change>> = BTreeMap::new();
            for (key, change) in updates {
                let hash = bytes::digest(key.as_bytes());
                groups
                    .entry(hash[prefix.len()..prefix.len() + 1].to_owned())
                    .or_default()
                    .insert(key, change);
            }
            for (digit, updates) in groups {
                let old = children.get(&digit).cloned();
                let child = self.walk(
                    role,
                    collection,
                    old.as_ref(),
                    &format!("{prefix}{digit}"),
                    updates,
                    target,
                    parts,
                )?;
                children.insert(digit, child);
            }
            return self.index(role, children, prefix, parts);
        }
        let mut rows = match d {
            Some(d) => self.rows(role, collection, d, prefix)?,
            None => BTreeMap::new(),
        };
        for (key, change) in &updates {
            let actual = rows
                .get(key)
                .map(|row| bytes::row_digest(row, PART_BYTES))
                .transpose()?;
            if actual != change.before_sha256 {
                return Err(Error::Invalid("Claim COW predecessor row"));
            }
        }
        for (key, change) in updates {
            rows.insert(key, change.after);
        }
        self.partition(role, rows, prefix, target, parts)
    }
    fn install(&mut self, path: &Path, relative: &Path, raw: &[u8]) -> Result<()> {
        use rustix::fs::{AtFlags, Mode, OFlags};
        use rustix::io::Errno;
        let base = tos_fd_open::open_absolute_directory(
            path.parent().ok_or(Error::Invalid("Claim COW base"))?,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        let components: Vec<_> = relative
            .components()
            .map(|c| c.as_os_str().to_owned())
            .collect();
        if components.len() != 3 {
            return Err(Error::Invalid("Claim COW install path"));
        }
        let mkdir = |parent: &File, name: &std::ffi::OsStr| -> Result<()> {
            match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(Errno::EXIST) => Ok(()),
                Err(e) => Err(Error::Io(e.into())),
            }
        };
        mkdir(&base, &components[0])?;
        let namespace = tos_fd_open::open_directory_at(&base, Path::new(&components[0]))
            .map_err(|e| Error::Source(e.to_string()))?;
        mkdir(&namespace, &components[1])?;
        let directory = tos_fd_open::open_directory_at(&namespace, Path::new(&components[1]))
            .map_err(|e| Error::Source(e.to_string()))?;
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!(".cow-{}-{sequence}", std::process::id());
        let mut temporary: File = rustix::fs::openat(
            &directory,
            name.as_str(),
            OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|e| Error::Io(e.into()))?;
        struct Cleanup<'a>(&'a File, String, bool);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                if self.2 {
                    let _ = rustix::fs::unlinkat(self.0, self.1.as_str(), AtFlags::empty());
                }
            }
        }
        let mut cleanup = Cleanup(&directory, name, true);
        temporary.write_all(raw)?;
        temporary.sync_all()?;
        match rustix::fs::linkat(
            &directory,
            cleanup.1.as_str(),
            &directory,
            &components[2],
            AtFlags::empty(),
        ) {
            Ok(()) => (),
            Err(Errno::EXIST) => {
                take(&mut self.usage.parts, 1, self.limits.parts)?;
                take(&mut self.usage.stored, raw.len() + 1, self.limits.stored)?;
                let mut existing =
                    tos_fd_open::open_regular_at(&directory, Path::new(&components[2]))
                        .map_err(|e| Error::Source(e.to_string()))?;
                if existing.metadata()?.len() != raw.len() as u64 {
                    return Err(Error::Invalid("Claim COW object conflict"));
                }
                let mut bytes = Vec::new();
                (&mut existing)
                    .take((raw.len() + 1) as u64)
                    .read_to_end(&mut bytes)?;
                if bytes != raw {
                    return Err(Error::Invalid("Claim COW existing bytes differ"));
                }
                existing.sync_all()?;
            }
            Err(e) => return Err(Error::Io(e.into())),
        }
        rustix::fs::unlinkat(&directory, cleanup.1.as_str(), AtFlags::empty())
            .map_err(|e| Error::Io(e.into()))?;
        cleanup.2 = false;
        drop(cleanup);
        directory.sync_all()?;
        namespace.sync_all()?;
        base.sync_all()?;
        Ok(())
    }
}
