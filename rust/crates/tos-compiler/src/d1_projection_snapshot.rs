//! Bounded reader/differ for retained private projection snapshots used by
//! offline D1 navigation capture. It proves only exact retained bytes and a
//! complete mechanical diff; selected-root currentness, source admission,
//! rights and publication remain with the source owner.

use crate::{Error, Result};
use flate2::bufread::MultiGzDecoder;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::{Component, Path, PathBuf},
};
use tos_foundation::Digest256;

const ROOT_SCHEMA: &str = "tos_partitioned_projection_v1";
const INDEX_SCHEMA: &str = "tos_projection_partition_index_v1";
const MAX_ROOT_BYTES: usize = 256 * 1024;
const MAX_INDEX_BYTES: usize = 128 * 1024;
const MAX_PART_BYTES: usize = 8 * 1024 * 1024;
const MAX_KEY_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub struct D1ProjectionLimits {
    pub max_opened_parts: u64,
    pub max_read_bytes: u64,
    pub max_keys: u64,
    /// Maximum rows retained by an explicit full collection read.
    pub max_rows: u64,
    pub max_changes: u64,
    pub max_output_bytes: u64,
}

impl Default for D1ProjectionLimits {
    fn default() -> Self {
        Self {
            max_opened_parts: 256,
            max_read_bytes: 128 * 1024 * 1024,
            max_keys: 4096,
            max_rows: 200_000,
            max_changes: 512,
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct D1ProjectionSnapshot {
    root_bytes: Vec<u8>,
    namespace_path: PathBuf,
    snapshot_sha256: String,
}

impl D1ProjectionSnapshot {
    /// Construct a retained immutable projection view. `namespace_path` is
    /// the original projection-root filename; only its sibling `.parts`
    /// namespace is read. The root file itself need not still exist.
    pub fn new(root_bytes: Vec<u8>, namespace_path: PathBuf) -> Result<Self> {
        if root_bytes.is_empty() || root_bytes.len() > MAX_ROOT_BYTES {
            return Err(Error::Budget("D1 projection root bytes"));
        }
        if !namespace_path.is_absolute()
            || namespace_path.file_name().is_none()
            || namespace_path
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        {
            return Err(Error::Invalid("D1 projection namespace path"));
        }
        let snapshot_sha256 = sha256(&root_bytes);
        let value = strict_json(&root_bytes, MAX_ROOT_BYTES)?;
        validate_manifest(&value, &namespace_path)?;
        Ok(Self {
            root_bytes,
            namespace_path,
            snapshot_sha256,
        })
    }

    pub fn snapshot_sha256(&self) -> &str {
        &self.snapshot_sha256
    }

    pub fn root_bytes(&self) -> &[u8] {
        &self.root_bytes
    }

    pub fn namespace_path(&self) -> &Path {
        &self.namespace_path
    }

    pub fn metadata(&self) -> Result<Value> {
        let root = strict_json(&self.root_bytes, MAX_ROOT_BYTES)?;
        root.get("header")
            .cloned()
            .ok_or(Error::Invalid("D1 projection header"))
    }

    /// Read one full declared collection in partition-key order. Every part,
    /// key, row count and compressed/uncompressed digest is checked under the
    /// caller's explicit finite budget. This is intended for initial capture,
    /// not an implicit whole-projection operation.
    pub fn read_collection(
        &self,
        name: &str,
        limits: D1ProjectionLimits,
    ) -> Result<D1ProjectionCollection> {
        validate_limits(limits)?;
        let root = strict_json(&self.root_bytes, MAX_ROOT_BYTES)?;
        let mut reader = SnapshotReader::new(self, root)?;
        let manifest = reader.manifest.clone();
        let spec = manifest
            .get("collections")
            .and_then(Value::as_object)
            .and_then(|collections| collections.get(name))
            .ok_or(Error::Invalid("D1 projection collection absent"))?;
        let key_field = spec
            .get("key_field")
            .cloned()
            .ok_or(Error::Invalid("D1 projection key field"))?;
        let mut rows = BTreeMap::new();
        let mut work = Work::new(limits);
        // A requested full read has separate row and key limits; both remain
        // active so callers can accumulate each resource across collections.
        work.limits.max_keys = limits.max_rows.min(limits.max_keys);
        work.reserve(0, 0, self.root_bytes.len() as u64, 0)?;
        let mut retained_bytes = 0u64;
        let descriptor = spec
            .get("root")
            .cloned()
            .ok_or(Error::Invalid("D1 projection collection root"))?;
        reader.visit_rows(name, &descriptor, "", &mut work, &mut |key, value| {
            retained_bytes = add(
                retained_bytes,
                add(key.len() as u64, canonical_json(value)?.len() as u64)?,
            )?;
            if retained_bytes > limits.max_output_bytes {
                return Err(Error::Budget("D1 projection output budget"));
            }
            if rows.insert(key.to_owned(), value.clone()).is_some() {
                return Err(Error::Invalid("duplicate D1 projection collection key"));
            }
            Ok(())
        })?;
        work.output_bytes(retained_bytes)?;
        Ok(D1ProjectionCollection {
            name: name.to_owned(),
            key_field,
            rows,
            accounting: work.accounting(),
        })
    }

    /// Return a complete bounded difference for the same collection profile.
    /// Identical content-addressed subtrees are skipped under the caller's
    /// already-admitted baseline premise. This does not inspect those parts or
    /// establish source/currentness/target-closure authority.
    pub fn diff_collection(
        &self,
        after: &Self,
        name: &str,
        limits: D1ProjectionLimits,
    ) -> Result<D1ProjectionDiff> {
        validate_limits(limits)?;
        let before_root = strict_json(&self.root_bytes, MAX_ROOT_BYTES)?;
        let after_root = strict_json(&after.root_bytes, MAX_ROOT_BYTES)?;
        validate_compatible_collection(&before_root, &after_root, name)?;
        let mut left = SnapshotReader::new(self, before_root.clone())?;
        let mut right = SnapshotReader::new(after, after_root.clone())?;
        let left_spec = collection_spec(&before_root, name)?;
        let right_spec = collection_spec(&after_root, name)?;
        let mut work = Work::new(limits);
        work.reserve(
            0,
            0,
            add(self.root_bytes.len() as u64, after.root_bytes.len() as u64)?,
            0,
        )?;
        let mut changes = Vec::new();
        walk_diff(
            &mut left,
            &mut right,
            name,
            &left_spec["root"],
            &right_spec["root"],
            "",
            &mut work,
            &mut changes,
        )?;
        changes.sort_by(|a, b| a.key.cmp(&b.key));
        let before_header = before_root["header"].clone();
        let after_header = after_root["header"].clone();
        // Both headers are retained in the result, even when their values agree.
        let header_bytes = add(
            canonical_json(&before_header)?.len() as u64,
            canonical_json(&after_header)?.len() as u64,
        )?;
        work.output_bytes(add(header_bytes, name.len() as u64 + 128)?)?;
        Ok(D1ProjectionDiff {
            collection: name.to_owned(),
            before_sha256: self.snapshot_sha256.clone(),
            after_sha256: after.snapshot_sha256.clone(),
            before_header,
            after_header,
            changes,
            accounting: work.accounting(),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct D1ProjectionCollection {
    pub name: String,
    pub key_field: Value,
    pub rows: BTreeMap<String, Value>,
    pub accounting: D1ProjectionAccounting,
}

#[derive(Clone, Debug, Serialize)]
pub struct D1ProjectionChange {
    pub key: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct D1ProjectionDiff {
    pub collection: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub before_header: Value,
    pub after_header: Value,
    pub changes: Vec<D1ProjectionChange>,
    pub accounting: D1ProjectionAccounting,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct D1ProjectionAccounting {
    pub opened_parts: u64,
    pub stored_bytes: u64,
    pub decoded_bytes: u64,
    pub keys: u64,
    pub changes: u64,
    pub output_bytes: u64,
}

#[derive(Clone, Copy)]
struct Work {
    limits: D1ProjectionLimits,
    used: D1ProjectionAccounting,
}

impl Work {
    fn new(limits: D1ProjectionLimits) -> Self {
        Self {
            limits,
            used: D1ProjectionAccounting::default(),
        }
    }
    fn reserve(&mut self, parts: u64, stored: u64, decoded: u64, keys: u64) -> Result<()> {
        let next = D1ProjectionAccounting {
            opened_parts: add(self.used.opened_parts, parts)?,
            stored_bytes: add(self.used.stored_bytes, stored)?,
            decoded_bytes: add(self.used.decoded_bytes, decoded)?,
            keys: add(self.used.keys, keys)?,
            changes: self.used.changes,
            output_bytes: self.used.output_bytes,
        };
        if next.opened_parts > self.limits.max_opened_parts
            || add(next.stored_bytes, next.decoded_bytes)? > self.limits.max_read_bytes
            || next.keys > self.limits.max_keys
        {
            return Err(Error::Budget("D1 projection read budget"));
        }
        self.used = next;
        Ok(())
    }
    fn change(&mut self) -> Result<()> {
        self.used.changes = add(self.used.changes, 1)?;
        if self.used.changes > self.limits.max_changes {
            return Err(Error::Budget("D1 projection change budget"));
        }
        Ok(())
    }
    fn output_bytes(&mut self, amount: u64) -> Result<()> {
        self.used.output_bytes = add(self.used.output_bytes, amount)?;
        if self.used.output_bytes > self.limits.max_output_bytes {
            return Err(Error::Budget("D1 projection output budget"));
        }
        Ok(())
    }
    fn accounting(self) -> D1ProjectionAccounting {
        self.used
    }
}

#[derive(Clone)]
struct SnapshotReader {
    manifest: Value,
    namespace_path: PathBuf,
}

impl SnapshotReader {
    fn new(snapshot: &D1ProjectionSnapshot, root: Value) -> Result<Self> {
        validate_manifest(&root, &snapshot.namespace_path)?;
        Ok(Self {
            manifest: root,
            namespace_path: snapshot.namespace_path.clone(),
        })
    }

    fn descriptor(&self, value: &Value, prefix: &str) -> Result<()> {
        let object = value
            .as_object()
            .ok_or(Error::Invalid("D1 projection part descriptor"))?;
        exact_keys(
            object,
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
            "D1 projection part descriptor fields",
        )?;
        let kind = string(value, "kind")?;
        let selected_prefix = string(value, "prefix")?;
        let digest = string(value, "sha256")?;
        let decoded_digest = string(value, "decoded_sha256")?;
        if !matches!(kind, "data" | "index")
            || selected_prefix != prefix
            || prefix.len() > 64
            || !prefix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || !valid_sha256(digest)
            || !valid_sha256(decoded_digest)
        {
            return Err(Error::Invalid("D1 projection part identity"));
        }
        let bound = if kind == "data" {
            MAX_PART_BYTES
        } else {
            MAX_INDEX_BYTES
        };
        let stored = uint(value, "size_bytes")?;
        let decoded = uint(value, "decoded_bytes")?;
        let _count = uint(value, "count")?;
        if decoded > bound as u64 || stored > (bound as u64).saturating_add(65_536) {
            return Err(Error::Budget("D1 projection part descriptor bounds"));
        }
        let suffix = if kind == "data" {
            ".jsonl.gz"
        } else {
            ".index.json"
        };
        let stem = self
            .namespace_path
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or(Error::Invalid("D1 projection namespace stem"))?;
        let expected = format!("{stem}.parts/{}/{digest}{suffix}", &digest[..2]);
        if string(value, "path")? != expected {
            return Err(Error::Invalid("D1 projection content namespace"));
        }
        Ok(())
    }

    fn children(
        &mut self,
        descriptor: &Value,
        prefix: &str,
        work: &mut Work,
    ) -> Result<BTreeMap<String, Value>> {
        self.descriptor(descriptor, prefix)?;
        let _expected_count = uint(descriptor, "count")?;
        let raw = self.load(descriptor, prefix, work)?;
        let index = strict_json(&raw, MAX_INDEX_BYTES)?;
        let object = index
            .as_object()
            .ok_or(Error::Invalid("D1 projection index object"))?;
        exact_keys(
            object,
            &["schema_version", "prefix", "count", "children"],
            "D1 projection index fields",
        )?;
        if string(&index, "schema_version")? != INDEX_SCHEMA
            || string(&index, "prefix")? != prefix
            || uint(&index, "count")? != uint(descriptor, "count")?
            || prefix.len() >= 64
        {
            return Err(Error::Invalid("D1 projection index identity"));
        }
        let children = index
            .get("children")
            .and_then(Value::as_object)
            .filter(|children| !children.is_empty())
            .ok_or(Error::Invalid("D1 projection index children"))?;
        let mut result = BTreeMap::new();
        let mut total = 0u64;
        for (digit, child) in children {
            if digit.len() != 1
                || !digit
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(Error::Invalid("D1 projection index branch"));
            }
            let child_prefix = format!("{prefix}{digit}");
            self.descriptor(child, &child_prefix)?;
            total = add(total, uint(child, "count")?)?;
            result.insert(digit.clone(), child.clone());
        }
        if total != uint(descriptor, "count")? {
            return Err(Error::Invalid("D1 projection index count"));
        }
        Ok(result)
    }

    fn load(&self, descriptor: &Value, prefix: &str, work: &mut Work) -> Result<Vec<u8>> {
        self.descriptor(descriptor, prefix)?;
        let path = string(descriptor, "path")?;
        let (directory, parts_directory, shard_dir, leaf) = self.part_location(path)?;
        let _keep_dir = directory;
        let _keep_parts = parts_directory;
        let mut file = tos_fd_open::open_regular_at(&shard_dir, Path::new(&leaf))
            .map_err(|_| Error::Invalid("unsafe D1 projection part path"))?;
        let declared = uint(descriptor, "size_bytes")?;
        work.reserve(
            1,
            add(declared, 1)?,
            add(uint(descriptor, "decoded_bytes")?, 1)?,
            0,
        )?;
        if file.metadata()?.len() != declared {
            return Err(Error::Invalid("D1 projection stored part size"));
        }
        let cap = usize::try_from(declared)
            .map_err(|_| Error::Budget("D1 projection stored part size"))?;
        let mut stored = Vec::with_capacity(cap.min(1024 * 1024));
        file.take(declared.saturating_add(1))
            .read_to_end(&mut stored)?;
        if stored.len() as u64 != declared || sha256(&stored) != string(descriptor, "sha256")? {
            return Err(Error::Invalid("D1 projection stored part digest"));
        }
        let decoded = if string(descriptor, "kind")? == "index" {
            if declared != uint(descriptor, "decoded_bytes")?
                || string(descriptor, "sha256")? != string(descriptor, "decoded_sha256")?
            {
                return Err(Error::Invalid("D1 projection index encoding"));
            }
            stored
        } else {
            decode_gzip(&stored, uint(descriptor, "decoded_bytes")?)?
        };
        if decoded.len() as u64 != uint(descriptor, "decoded_bytes")?
            || sha256(&decoded) != string(descriptor, "decoded_sha256")?
        {
            return Err(Error::Invalid("D1 projection decoded part digest"));
        }
        Ok(decoded)
    }

    fn part_location(&self, relative: &str) -> Result<(File, File, File, String)> {
        let parts: Vec<_> = Path::new(relative).components().collect();
        if parts.len() != 3
            || !matches!(parts[0], Component::Normal(_))
            || !matches!(parts[1], Component::Normal(_))
            || !matches!(parts[2], Component::Normal(_))
        {
            return Err(Error::Invalid("D1 projection part path components"));
        }
        let parent_path = self
            .namespace_path
            .parent()
            .ok_or(Error::Invalid("D1 projection parent"))?;
        let directory = tos_fd_open::open_absolute_directory(parent_path)
            .map_err(|_| Error::Invalid("unsafe D1 projection parent"))?;
        let first = parts[0]
            .as_os_str()
            .to_str()
            .ok_or(Error::Invalid("D1 projection part directory"))?;
        let parts_directory = tos_fd_open::open_directory_at(&directory, Path::new(first))
            .map_err(|_| Error::Invalid("unsafe D1 projection parts directory"))?;
        let shard = parts[1]
            .as_os_str()
            .to_str()
            .ok_or(Error::Invalid("D1 projection shard"))?;
        let shard_dir = tos_fd_open::open_directory_at(&parts_directory, Path::new(shard))
            .map_err(|_| Error::Invalid("unsafe D1 projection shard"))?;
        let leaf = parts[2]
            .as_os_str()
            .to_str()
            .ok_or(Error::Invalid("D1 projection part leaf"))?
            .to_owned();
        Ok((directory, parts_directory, shard_dir, leaf))
    }

    fn visit_rows(
        &mut self,
        name: &str,
        descriptor: &Value,
        prefix: &str,
        work: &mut Work,
        sink: &mut impl FnMut(&str, &Value) -> Result<()>,
    ) -> Result<()> {
        self.descriptor(descriptor, prefix)?;
        if string(descriptor, "kind")? == "index" {
            let children = self.children(descriptor, prefix, work)?;
            for (digit, child) in children {
                let next_prefix = format!("{prefix}{digit}");
                self.visit_rows(name, &child, &next_prefix, work, sink)?;
            }
            return Ok(());
        }
        let expected_count = uint(descriptor, "count")?;
        work.reserve(0, 0, 0, expected_count)?;
        let raw = self.load(descriptor, prefix, work)?;
        let collection = collection_spec(&self.manifest, name)?;
        let key_field = collection
            .get("key_field")
            .ok_or(Error::Invalid("D1 projection key field"))?;
        let mut count = 0u64;
        let mut previous = None::<String>;
        if !raw.is_empty() && !raw.ends_with(b"\n") {
            return Err(Error::Invalid("D1 projection row newline framing"));
        }
        let mut lines = raw.split_inclusive(|byte| *byte == b'\n').peekable();
        while let Some(framed) = lines.next() {
            let line = framed.strip_suffix(b"\n").unwrap_or(framed);
            if line.is_empty() {
                return Err(Error::Invalid("D1 projection empty row framing"));
            }
            let value = strict_json(line, MAX_PART_BYTES)?;
            let object = value
                .as_object()
                .ok_or(Error::Invalid("D1 projection row object"))?;
            exact_keys(object, &["key", "value"], "D1 projection row fields")?;
            let key = value
                .get("key")
                .and_then(Value::as_str)
                .filter(|key| !key.is_empty() && key.len() <= MAX_KEY_BYTES)
                .ok_or(Error::Invalid("D1 projection row key"))?;
            if previous.as_deref().is_some_and(|prior| prior >= key)
                || !sha256(key.as_bytes()).starts_with(prefix)
            {
                return Err(Error::Invalid("D1 projection row ordering/placement"));
            }
            verify_record_key(
                key_field,
                key,
                &value["value"],
                uint(&collection["root"], "count")?,
            )?;
            let encoded = canonical_record(&value)?;
            if encoded.as_slice() != framed {
                return Err(Error::Invalid("D1 projection canonical row bytes"));
            }
            previous = Some(key.to_owned());
            count = add(count, 1)?;
            sink(key, &value["value"])?;
        }
        if count != expected_count {
            return Err(Error::Invalid("D1 projection data row count"));
        }
        Ok(())
    }
}

fn walk_diff(
    left: &mut SnapshotReader,
    right: &mut SnapshotReader,
    name: &str,
    before: &Value,
    after: &Value,
    prefix: &str,
    work: &mut Work,
    changes: &mut Vec<D1ProjectionChange>,
) -> Result<()> {
    walk_subtree_diff(
        left,
        right,
        name,
        &Subtree::Descriptor(before.clone()),
        &Subtree::Descriptor(after.clone()),
        prefix,
        work,
        changes,
    )
}

#[derive(Clone)]
enum Subtree {
    Descriptor(Value),
    Rows(BTreeMap<String, Value>),
}

fn walk_subtree_diff(
    left: &mut SnapshotReader,
    right: &mut SnapshotReader,
    name: &str,
    before: &Subtree,
    after: &Subtree,
    prefix: &str,
    work: &mut Work,
    changes: &mut Vec<D1ProjectionChange>,
) -> Result<()> {
    if let (Subtree::Descriptor(a), Subtree::Descriptor(b)) = (before, after) {
        left.descriptor(a, prefix)?;
        right.descriptor(b, prefix)?;
        if descriptor_identity(a)? == descriptor_identity(b)? {
            return Ok(());
        }
    }
    let before_index = match before {
        Subtree::Descriptor(value) => string(value, "kind")? == "index",
        Subtree::Rows(_) => false,
    };
    let after_index = match after {
        Subtree::Descriptor(value) => string(value, "kind")? == "index",
        Subtree::Rows(_) => false,
    };
    if before_index || after_index {
        let left_children = branches(left, name, before, prefix, work)?;
        let right_children = branches(right, name, after, prefix, work)?;
        let digits = left_children
            .keys()
            .chain(right_children.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for digit in digits {
            let next_prefix = format!("{prefix}{digit}");
            match (left_children.get(&digit), right_children.get(&digit)) {
                (Some(a), Some(b)) => {
                    walk_subtree_diff(left, right, name, a, b, &next_prefix, work, changes)?
                }
                (Some(a), None) => {
                    collect_subtree_changes(left, name, a, &next_prefix, work, changes, false)?
                }
                (None, Some(b)) => {
                    collect_subtree_changes(right, name, b, &next_prefix, work, changes, true)?
                }
                (None, None) => unreachable!(),
            }
        }
        return Ok(());
    }
    let old = read_subtree_rows(left, name, before, prefix, work)?;
    let new = read_subtree_rows(right, name, after, prefix, work)?;
    let keys = old
        .keys()
        .chain(new.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in keys {
        let a = old.get(&key);
        let b = new.get(&key);
        if let (Some(x), Some(y)) = (a, b) {
            if canonical_json(x)? == canonical_json(y)? {
                continue;
            }
        }
        work.change()?;
        work.output_bytes(change_output_bytes(&key, a, b)?)?;
        changes.push(D1ProjectionChange {
            key,
            before: a.cloned(),
            after: b.cloned(),
        });
    }
    Ok(())
}

fn branches(
    reader: &mut SnapshotReader,
    name: &str,
    subtree: &Subtree,
    prefix: &str,
    work: &mut Work,
) -> Result<BTreeMap<String, Subtree>> {
    if let Subtree::Descriptor(descriptor) = subtree {
        if string(descriptor, "kind")? == "index" {
            return reader.children(descriptor, prefix, work).map(|children| {
                children
                    .into_iter()
                    .map(|(digit, child)| (digit, Subtree::Descriptor(child)))
                    .collect()
            });
        }
    }
    let rows = read_subtree_rows(reader, name, subtree, prefix, work)?;
    let mut result = BTreeMap::new();
    for (key, value) in rows {
        let digest = sha256(key.as_bytes());
        let digit = digest
            .as_bytes()
            .get(prefix.len()..prefix.len() + 1)
            .ok_or(Error::Invalid("D1 projection key hash depth"))?;
        let digit =
            std::str::from_utf8(digit).map_err(|_| Error::Invalid("D1 projection key hash"))?;
        match result.entry(digit.to_owned()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Subtree::Rows(BTreeMap::from([(key, value)])));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let Subtree::Rows(rows) = entry.get_mut() else {
                    return Err(Error::Invalid("D1 projection expanded branch type"));
                };
                rows.insert(key, value);
            }
        }
    }
    Ok(result)
}

fn collect_subtree_changes(
    reader: &mut SnapshotReader,
    name: &str,
    subtree: &Subtree,
    prefix: &str,
    work: &mut Work,
    changes: &mut Vec<D1ProjectionChange>,
    insert_side: bool,
) -> Result<()> {
    let rows = read_subtree_rows(reader, name, subtree, prefix, work)?;
    for (key, value) in rows {
        work.change()?;
        let (before, after) = if insert_side {
            (None, Some(&value))
        } else {
            (Some(&value), None)
        };
        work.output_bytes(change_output_bytes(&key, before, after)?)?;
        changes.push(D1ProjectionChange {
            key,
            before: before.cloned(),
            after: after.cloned(),
        });
    }
    Ok(())
}

fn read_subtree_rows(
    reader: &mut SnapshotReader,
    name: &str,
    subtree: &Subtree,
    prefix: &str,
    work: &mut Work,
) -> Result<BTreeMap<String, Value>> {
    match subtree {
        Subtree::Rows(rows) => Ok(rows.clone()),
        Subtree::Descriptor(descriptor) => {
            let mut rows = BTreeMap::new();
            reader.visit_rows(name, descriptor, prefix, work, &mut |key, value| {
                rows.insert(key.to_owned(), value.clone());
                Ok(())
            })?;
            Ok(rows)
        }
    }
}

fn change_output_bytes(key: &str, before: Option<&Value>, after: Option<&Value>) -> Result<u64> {
    let mut total = key.len() as u64 + 128;
    if let Some(value) = before {
        total = add(total, canonical_json(value)?.len() as u64)?;
    }
    if let Some(value) = after {
        total = add(total, canonical_json(value)?.len() as u64)?;
    }
    Ok(total)
}

fn validate_manifest(root: &Value, namespace_path: &Path) -> Result<()> {
    let object = root
        .as_object()
        .ok_or(Error::Invalid("D1 projection manifest"))?;
    exact_keys(
        object,
        &[
            "schema_version",
            "logical_schema",
            "header",
            "limits",
            "collections",
        ],
        "D1 projection manifest fields",
    )?;
    if string(root, "schema_version")? != ROOT_SCHEMA
        || string(root, "logical_schema")?.is_empty()
        || root.get("header").and_then(Value::as_object).is_none()
        || root["header"].get("schema_version") != root.get("logical_schema")
        || root["limits"]
            != json!({"root_bytes":MAX_ROOT_BYTES,"index_bytes":MAX_INDEX_BYTES,
            "part_bytes":MAX_PART_BYTES,"key_bytes":MAX_KEY_BYTES})
    {
        return Err(Error::Invalid("D1 projection manifest profile"));
    }
    let collections = root
        .get("collections")
        .and_then(Value::as_object)
        .filter(|collections| !collections.is_empty())
        .ok_or(Error::Invalid("D1 projection collections"))?;
    let mut skeleton = root["header"].clone();
    for (name, spec) in collections {
        if !valid_collection_name(name) {
            return Err(Error::Invalid("D1 projection collection name"));
        }
        let fields = spec
            .as_object()
            .ok_or(Error::Invalid("D1 projection collection spec"))?;
        exact_keys(
            fields,
            &["key_field", "order_fields", "root"],
            "D1 projection collection fields",
        )?;
        validate_key_field(&spec["key_field"])?;
        if spec["order_fields"].as_array().is_none_or(|order| {
            order
                .iter()
                .any(|field| field.as_str().is_none_or(str::is_empty))
        }) {
            return Err(Error::Invalid("D1 projection order fields"));
        }
        set_collection(&mut skeleton, name)?;
        if spec["key_field"] == json!([])
            && !spec["order_fields"].as_array().is_some_and(Vec::is_empty)
        {
            return Err(Error::Invalid("D1 projection positional order fields"));
        }
        let reader = SnapshotReader {
            manifest: root.clone(),
            namespace_path: namespace_path.to_owned(),
        };
        reader.descriptor(&spec["root"], "")?;
    }
    Ok(())
}

fn validate_compatible_collection(before: &Value, after: &Value, name: &str) -> Result<()> {
    let left = collection_spec(before, name)?;
    let right = collection_spec(after, name)?;
    if string(before, "logical_schema")? != string(after, "logical_schema")?
        || before["collections"]
            .as_object()
            .map(|map| map.keys().collect::<Vec<_>>())
            != after["collections"]
                .as_object()
                .map(|map| map.keys().collect::<Vec<_>>())
        || canonical_json(&left["key_field"])? != canonical_json(&right["key_field"])?
        || canonical_json(&left["order_fields"])? != canonical_json(&right["order_fields"])?
    {
        return Err(Error::PreparedUnsupported(
            "projection profile requires bootstrap",
        ));
    }
    Ok(())
}

fn collection_spec<'a>(root: &'a Value, name: &str) -> Result<&'a Value> {
    root.get("collections")
        .and_then(Value::as_object)
        .and_then(|collections| collections.get(name))
        .ok_or(Error::Invalid("D1 projection collection absent"))
}

fn descriptor_identity(value: &Value) -> Result<Vec<u8>> {
    let mut clone = value.clone();
    clone
        .as_object_mut()
        .ok_or(Error::Invalid("D1 projection descriptor"))?
        .remove("path");
    canonical_json(&clone)
}

fn validate_limits(limits: D1ProjectionLimits) -> Result<()> {
    if limits.max_opened_parts == 0
        || limits.max_read_bytes == 0
        || limits.max_keys == 0
        || limits.max_rows == 0
        || limits.max_changes == 0
        || limits.max_output_bytes == 0
    {
        return Err(Error::Budget("D1 projection limits"));
    }
    Ok(())
}

fn set_collection(header: &mut Value, name: &str) -> Result<()> {
    let parts = name.split('/').collect::<Vec<_>>();
    let (last, parents) = parts
        .split_last()
        .ok_or(Error::Invalid("D1 projection collection path"))?;
    let mut current = header;
    for part in parents {
        let object = current
            .as_object_mut()
            .ok_or(Error::Invalid("D1 projection header collection overlap"))?;
        current = object
            .entry((*part).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if !current.is_object() {
            return Err(Error::Invalid("D1 projection header collection overlap"));
        }
    }
    let object = current
        .as_object_mut()
        .ok_or(Error::Invalid("D1 projection header collection overlap"))?;
    if object.contains_key(*last) {
        return Err(Error::Invalid("D1 projection header collection overlap"));
    }
    object.insert((*last).to_owned(), Value::Null);
    Ok(())
}

fn validate_key_field(value: &Value) -> Result<()> {
    let valid = value.is_null()
        || value.as_str().is_some_and(|field| !field.is_empty())
        || value.as_array().is_some_and(|fields| {
            fields
                .iter()
                .all(|field| field.as_str().is_some_and(|name| !name.is_empty()))
        });
    if !valid {
        return Err(Error::Invalid("D1 projection key field"));
    }
    Ok(())
}

fn verify_record_key(key_field: &Value, key: &str, value: &Value, total: u64) -> Result<()> {
    if key_field.is_null() {
        return Ok(());
    }
    if let Some(field) = key_field.as_str() {
        if value.get(field).and_then(Value::as_str) != Some(key) {
            return Err(Error::Invalid("D1 projection row identity"));
        }
        return Ok(());
    }
    if let Some(fields) = key_field.as_array() {
        if fields.is_empty() {
            if key.len() != 20
                || !key.bytes().all(|byte| byte.is_ascii_digit())
                || key
                    .parse::<u64>()
                    .ok()
                    .is_none_or(|position| position >= total)
            {
                return Err(Error::Invalid("D1 projection positional row identity"));
            }
            return Ok(());
        }
        let parts = fields
            .iter()
            .map(|field| {
                let field = field
                    .as_str()
                    .ok_or(Error::Invalid("D1 projection composite key"))?;
                value
                    .get(field)
                    .and_then(Value::as_str)
                    .ok_or(Error::Invalid("D1 projection composite key value"))
            })
            .collect::<Result<Vec<_>>>()?;
        let encoded = serde_json::to_string(&parts)
            .map_err(|_| Error::Invalid("D1 projection composite key framing"))?;
        if encoded != key {
            return Err(Error::Invalid("D1 projection composite row identity"));
        }
    }
    Ok(())
}

fn canonical_record(value: &Value) -> Result<Vec<u8>> {
    let mut raw =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("D1 projection row JSON"))?;
    raw.push(b'\n');
    Ok(raw)
}

fn canonical_json(value: &Value) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| Error::Invalid("D1 projection JSON framing"))
}

fn strict_json(raw: &[u8], cap: usize) -> Result<Value> {
    if raw.len() > cap {
        return Err(Error::Budget("D1 projection JSON bytes"));
    }
    let limits = tos_foundation::JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("D1 projection JSON limits"))?;
    tos_foundation::parse_json(raw, tos_foundation::JsonMode::PublishedStrict, limits)
        .map_err(|_| Error::Invalid("D1 projection strict JSON"))?;
    serde_json::from_slice(raw).map_err(|_| Error::Invalid("D1 projection JSON"))
}

fn decode_gzip(stored: &[u8], expected: u64) -> Result<Vec<u8>> {
    let cap = usize::try_from(expected).map_err(|_| Error::Budget("D1 projection decoded size"))?;
    if cap > MAX_PART_BYTES {
        return Err(Error::Budget("D1 projection decoded part size"));
    }
    let cursor = Cursor::new(stored);
    let buffered = BufReader::new(cursor);
    let mut decoder = MultiGzDecoder::new(buffered);
    let mut decoded = Vec::with_capacity(cap.min(1024 * 1024));
    decoder
        .by_ref()
        .take(expected.saturating_add(1))
        .read_to_end(&mut decoded)
        .map_err(|_| Error::Invalid("D1 projection gzip"))?;
    if decoded.len() as u64 != expected {
        return Err(Error::Invalid("D1 projection gzip length"));
    }
    let mut inner = decoder.into_inner();
    let position = inner
        .stream_position()
        .map_err(|_| Error::Invalid("D1 projection gzip position"))?;
    if position != stored.len() as u64 {
        return Err(Error::Invalid("D1 projection gzip trailing bytes"));
    }
    Ok(decoded)
}

fn exact_keys(object: &Map<String, Value>, keys: &[&str], reason: &'static str) -> Result<()> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error::Invalid(reason));
    }
    Ok(())
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("D1 projection string field"))
}

fn uint(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("D1 projection unsigned field"))
}

fn valid_collection_name(value: &str) -> bool {
    !value.is_empty()
        && value.split('/').all(|part| {
            let mut bytes = part.bytes();
            bytes.next().is_some_and(|first| first.is_ascii_lowercase())
                && bytes
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or(Error::Budget("D1 projection counters"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    fn snapshot(directory: &Path, name: &str, rows: &[Value]) -> D1ProjectionSnapshot {
        let mut sorted = rows
            .iter()
            .map(|row| (row["id"].as_str().unwrap().to_owned(), row.clone()))
            .collect::<Vec<_>>();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let decoded = sorted
            .iter()
            .flat_map(|(key, row)| canonical_record(&json!({"key":key,"value":row})).unwrap())
            .collect::<Vec<_>>();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&decoded).unwrap();
        let stored = encoder.finish().unwrap();
        let part_digest = sha256(&stored);
        let stem = Path::new(name).file_stem().unwrap().to_str().unwrap();
        let relative = format!("{stem}.parts/{}/{part_digest}.jsonl.gz", &part_digest[..2]);
        let part_path = directory.join(&relative);
        std::fs::create_dir_all(part_path.parent().unwrap()).unwrap();
        std::fs::write(&part_path, &stored).unwrap();
        build_snapshot(directory, name, &relative, &stored, &decoded, rows.len())
    }

    fn build_snapshot(
        directory: &Path,
        name: &str,
        relative: &str,
        stored: &[u8],
        decoded: &[u8],
        count: usize,
    ) -> D1ProjectionSnapshot {
        let logical_schema = "tos_source_navigation_v1";
        let manifest = json!({
            "schema_version": ROOT_SCHEMA,
            "logical_schema": logical_schema,
            "header": {"schema_version": logical_schema, "authority_boundary": "offline fixture"},
            "limits": {"root_bytes":MAX_ROOT_BYTES,"index_bytes":MAX_INDEX_BYTES,
                "part_bytes":MAX_PART_BYTES,"key_bytes":MAX_KEY_BYTES},
            "collections": {
                "nodes": {
                    "key_field":"id",
                    "order_fields":["id"],
                    "root": {
                        "kind":"data","prefix":"","path":relative,
                        "sha256":sha256(stored),"size_bytes":stored.len(),
                        "decoded_bytes":decoded.len(),"decoded_sha256":sha256(decoded),"count":count
                    }
                }
            }
        });
        let root_bytes = serde_json::to_vec(&manifest).unwrap();
        let path = Path::new(name);
        let root_path = directory.join(path.file_name().unwrap());
        D1ProjectionSnapshot::new(root_bytes, root_path).unwrap()
    }

    fn row(id: &str, label: &str) -> Value {
        json!({"id":id,"label":label})
    }

    #[test]
    fn reads_canonical_rows_under_count_and_output_budgets() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = snapshot(
            directory.path(),
            "/tmp/tos-d1-projection-fixture/nodes.json",
            &[row("a", "one"), row("b", "two")],
        );
        let collection = snapshot
            .read_collection("nodes", D1ProjectionLimits::default())
            .unwrap();
        assert_eq!(collection.rows.get("a"), Some(&row("a", "one")));
        assert_eq!(collection.accounting.keys, 2);

        let mut small = D1ProjectionLimits::default();
        small.max_rows = 1;
        assert!(matches!(
            snapshot.read_collection("nodes", small),
            Err(Error::Budget(_))
        ));
        let mut small = D1ProjectionLimits::default();
        small.max_output_bytes = 1;
        assert!(matches!(
            snapshot.read_collection("nodes", small),
            Err(Error::Budget(_))
        ));
    }

    #[test]
    fn identical_snapshot_still_charges_both_returned_headers() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = snapshot(
            directory.path(),
            "/tmp/tos-d1-projection-fixture/same.json",
            &[row("a", "one")],
        );
        let diff = snapshot
            .diff_collection(&snapshot, "nodes", D1ProjectionLimits::default())
            .unwrap();
        assert!(diff.changes.is_empty());
        let headers = canonical_json(&diff.before_header).unwrap().len() as u64
            + canonical_json(&diff.after_header).unwrap().len() as u64;
        assert_eq!(diff.accounting.output_bytes, headers + 133);
        let mut caps = D1ProjectionLimits::default();
        caps.max_output_bytes = 133;
        assert!(matches!(
            snapshot.diff_collection(&snapshot, "nodes", caps),
            Err(Error::Budget(_))
        ));
    }
    #[test]
    fn diffs_content_addressed_rows_and_rejects_changed_profile() {
        let directory = tempfile::tempdir().unwrap();
        let before = snapshot(
            directory.path(),
            "/tmp/tos-d1-projection-fixture/before.json",
            &[row("a", "one"), row("b", "two")],
        );
        let after = snapshot(
            directory.path(),
            "/tmp/tos-d1-projection-fixture/after.json",
            &[row("a", "changed"), row("c", "three")],
        );
        let diff = before
            .diff_collection(&after, "nodes", D1ProjectionLimits::default())
            .unwrap();
        assert_eq!(diff.changes.len(), 3);
        assert!(
            diff.changes.iter().any(|change| change.key == "a"
                && change.before.is_some()
                && change.after.is_some())
        );
        assert!(
            diff.changes
                .iter()
                .any(|change| change.key == "b" && change.after.is_none())
        );
        assert!(
            diff.changes
                .iter()
                .any(|change| change.key == "c" && change.before.is_none())
        );

        let mut changed_profile: Value = serde_json::from_slice(before.root_bytes()).unwrap();
        changed_profile["collections"]["nodes"]["order_fields"] = json!(["ordinal"]);
        let changed_profile = D1ProjectionSnapshot::new(
            serde_json::to_vec(&changed_profile).unwrap(),
            before.namespace_path.clone(),
        )
        .unwrap();
        assert!(matches!(
            before.diff_collection(&changed_profile, "nodes", D1ProjectionLimits::default()),
            Err(Error::PreparedUnsupported(_))
        ));
    }
}
