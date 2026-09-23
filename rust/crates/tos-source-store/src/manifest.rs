//! Exact, bounded reads of the existing `tos_corpus_snapshot_v1` carrier.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use tos_foundation::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode, JsonMode,
    JsonValue, RelativePath, SourceRevision, canonical_bytes_v1, parse_json,
};

use crate::error::{Result, StoreError, StoreErrorCode as Code};
use crate::limits::ReadLimits;
use crate::object::{verify_selected_object, verify_without_copy};

const SNAPSHOT_SCHEMA: &str = "tos_corpus_snapshot_v1";
const POINTER_SCHEMA: &str = "tos_corpus_pointer_v1";

/// A read-only corpus root. Opening does not create directories or select current.
#[derive(Clone, Debug)]
pub struct CorpusReader {
    root: PathBuf,
    limits: ReadLimits,
}

/// One member's manifest-bound metadata, without any content-read grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberMetadata {
    pub path: RelativePath,
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub mode: u32,
}

/// An exact v1 snapshot with disposable lookup indexes. It is not admission evidence.
#[derive(Clone, Debug)]
pub struct Snapshot {
    root: PathBuf,
    revision: SourceRevision,
    base_revision: Option<SourceRevision>,
    validator_sha256: Digest256,
    files: BTreeMap<RelativePath, MemberMetadata>,
    identities: BTreeMap<String, RelativePath>,
    dependencies: BTreeMap<RelativePath, Vec<RelativePath>>,
    retirement_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum Selector<'a> {
    SourceId(&'a str),
    Path(&'a RelativePath),
}

/// An exact selected v1 member. A descriptor alone conveys no read authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusDescriptor {
    pub revision: SourceRevision,
    pub path: RelativePath,
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub mode: u32,
}

impl Snapshot {
    pub fn revision(&self) -> SourceRevision { self.revision }
    pub fn base_revision(&self) -> Option<SourceRevision> { self.base_revision }
    pub fn validator_sha256(&self) -> Digest256 { self.validator_sha256 }
    pub fn member_count(&self) -> usize { self.files.len() }
    pub fn identity_count(&self) -> usize { self.identities.len() }
    pub fn retirement_count(&self) -> usize { self.retirement_count }
    pub fn member(&self, path: &RelativePath) -> Option<&MemberMetadata> { self.files.get(path) }
    pub fn identity_path(&self, id: &str) -> Option<&RelativePath> { self.identities.get(id) }
    /// These are stored index claims, not a proof that the owner validator found every dependency.
    pub fn indexed_dependencies(&self, path: &RelativePath) -> Option<&[RelativePath]> {
        self.dependencies.get(path).map(Vec::as_slice)
    }
}

impl CorpusReader {
    pub fn open_existing(root: &Path, limits: ReadLimits) -> Result<Self> {
        let limits = limits.validate()?;
        if !root.is_absolute() || root.canonicalize().ok().as_deref() != Some(root) {
            return Err(StoreError::new(Code::InvalidRoot, "corpus root must be an existing absolute unlinked directory"));
        }
        if !fs::metadata(root).map_err(|error| StoreError::io("cannot stat corpus root", error))?.is_dir() {
            return Err(StoreError::new(Code::InvalidRoot, "corpus root is not a directory"));
        }
        check_directory(&root.join("revisions"))?;
        check_directory(&root.join("objects"))?;
        Ok(Self { root: root.to_owned(), limits })
    }

    /// Explicitly inspect the mutable pointer. It is never consulted by `load_exact`.
    pub fn select_current(&self) -> Result<Option<SourceRevision>> {
        let path = self.root.join("current.json");
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StoreError::io("cannot stat current pointer", error)),
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                return Err(StoreError::new(Code::UnsafePath, "current pointer is not a regular file"));
            }
            Ok(_) => {}
        }
        let value = self.read_canonical(&path)?;
        exact_keys(&value, &["schema_version", "current", "previous"], Code::UnsupportedFormat)?;
        if string_field(&value, "schema_version", Code::UnsupportedFormat)? != POINTER_SCHEMA {
            return Err(StoreError::new(Code::UnsupportedFormat, "unsupported corpus pointer"));
        }
        let current = digest_field(&value, "current", Code::UnsupportedFormat)?;
        optional_revision(&value, "previous", Code::UnsupportedFormat)?;
        Ok(Some(SourceRevision(current)))
    }

    /// Validate exactly one canonical manifest, without hashing unrelated objects.
    pub fn load_exact(&self, revision: SourceRevision) -> Result<Snapshot> {
        let dir = self.root.join("revisions").join(revision.0.to_hex());
        match check_directory(&dir) {
            Err(error) if error.code == Code::Io && error.source.as_ref().is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) => {
                return Err(StoreError::new(Code::MissingRevision, "exact corpus revision is absent"));
            }
            other => other?,
        }
        let manifest_path = dir.join("snapshot.json");
        let value = match self.read_canonical(&manifest_path) {
            Err(error) if error.code == Code::Io && error.source.as_ref().is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) => {
                return Err(StoreError::new(Code::MissingRevision, "exact corpus snapshot is absent"));
            }
            other => other?,
        };
        exact_keys(&value, &[
            "schema_version", "base_revision", "validator_sha256", "files", "identities",
            "dependencies", "retirements", "revision",
        ], Code::UnsupportedFormat)?;
        if string_field(&value, "schema_version", Code::UnsupportedFormat)? != SNAPSHOT_SCHEMA {
            return Err(StoreError::new(Code::UnsupportedFormat, "unsupported corpus snapshot"));
        }
        let body = value.without_top_field("revision").map_err(canonical_error)?;
        let body_bytes = canonical_bytes_v1(&body, CanonicalProfile::CorpusSnapshotV1, self.limits.json)
            .map_err(canonical_error)?;
        if digest_field(&value, "revision", Code::RevisionMismatch)? != revision.0
            || Digest256::of_bytes(&body_bytes) != revision.0
        {
            return Err(StoreError::new(Code::RevisionMismatch, "corpus revision digest differs"));
        }
        let validator_sha256 = digest_field(&value, "validator_sha256", Code::UnsupportedFormat)?;
        let base_revision = optional_revision(&value, "base_revision", Code::UnsupportedFormat)?;
        let entries = array_field(&value, "files", Code::InvalidMemberIndex)?;
        check_count(entries.len(), self.limits.max_manifest_entries)?;
        let mut files = BTreeMap::new();
        let mut previous: Option<RelativePath> = None;
        for item in entries {
            exact_keys(item, &["path", "sha256", "size_bytes", "mode"], Code::InvalidMemberIndex)?;
            let path = path_field(item, "path", Code::InvalidMemberIndex)?;
            if previous.as_ref().is_some_and(|prior| path <= *prior) {
                return Err(StoreError::new(Code::InvalidMemberIndex, "snapshot members must be strictly sorted and unique"));
            }
            let member = MemberMetadata {
                path: path.clone(),
                sha256: digest_field(item, "sha256", Code::InvalidMemberIndex)?,
                size_bytes: uint_field(item, "size_bytes", Code::InvalidMemberIndex)?,
                mode: mode_field(item, "mode", Code::InvalidMemberIndex)?,
            };
            files.insert(path.clone(), member);
            previous = Some(path);
        }
        let identities = parse_identities(&value, &files, self.limits.max_manifest_entries)?;
        let dependencies = parse_dependencies(&value, &files, self.limits.max_manifest_entries)?;
        let retirement_count = validate_retirements(&value, self.limits.max_manifest_entries)?;
        Ok(Snapshot {
            root: self.root.clone(), revision, base_revision, validator_sha256,
            files, identities, dependencies, retirement_count,
        })
    }

    /// Resolve exactly one member and verify only its selected immutable object.
    pub fn resolve(&self, snapshot: &Snapshot, selector: Selector<'_>) -> Result<CorpusDescriptor> {
        self.check_snapshot(snapshot)?;
        let path = match selector {
            Selector::SourceId(id) => {
                if id.trim().is_empty() { return Err(StoreError::new(Code::InvalidSelector, "source ID is empty")); }
                snapshot.identities.get(id).ok_or_else(|| StoreError::new(Code::MissingMember, "source ID is absent from exact revision"))?
            }
            Selector::Path(path) => path,
        };
        let member = snapshot.files.get(path).ok_or_else(|| StoreError::new(Code::MissingMember, "path is absent from exact revision"))?;
        verify_without_copy(&self.root, member.sha256, member.size_bytes, self.limits.max_selected_object_bytes)?;
        Ok(CorpusDescriptor {
            revision: snapshot.revision,
            path: member.path.clone(), sha256: member.sha256,
            size_bytes: member.size_bytes, mode: member.mode,
        })
    }

    /// Write bytes only to a caller-owned unpublished stage; discard it on any error.
    pub fn read_selected(&self, snapshot: &Snapshot, descriptor: &CorpusDescriptor,
                         max_bytes: u64, sink: &mut impl Write) -> Result<u64> {
        self.check_snapshot(snapshot)?;
        let member = snapshot.files.get(&descriptor.path)
            .ok_or_else(|| StoreError::new(Code::MissingMember, "path is absent from exact revision"))?;
        if descriptor.revision != snapshot.revision || descriptor.sha256 != member.sha256
            || descriptor.size_bytes != member.size_bytes || descriptor.mode != member.mode
        {
            return Err(StoreError::new(Code::DescriptorMismatch, "descriptor differs from exact revision"));
        }
        verify_selected_object(&self.root, member.sha256, member.size_bytes,
                               max_bytes.min(self.limits.max_selected_object_bytes), sink)
    }

    fn check_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        if self.root != snapshot.root {
            return Err(StoreError::new(Code::DescriptorMismatch, "snapshot belongs to another corpus root"));
        }
        Ok(())
    }

    fn read_canonical(&self, path: &Path) -> Result<JsonValue> {
        let metadata = fs::symlink_metadata(path).map_err(|error| StoreError::io("cannot stat corpus manifest", error))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StoreError::new(Code::UnsafePath, "corpus manifest is not a regular file"));
        }
        let mut file = File::open(path).map_err(|error| StoreError::io("cannot open corpus manifest", error))?;
        let cap = self.limits.max_manifest_bytes.min(self.limits.json.max_bytes);
        let mut raw = Vec::new();
        file.by_ref().take((cap as u64).saturating_add(1)).read_to_end(&mut raw)
            .map_err(|error| StoreError::io("cannot read corpus manifest", error))?;
        if raw.len() > cap { return Err(StoreError::new(Code::BudgetExceeded, "corpus manifest exceeds read limit")); }
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, self.limits.json).map_err(canonical_error)?;
        let canonical = canonical_bytes_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, self.limits.json)
            .map_err(canonical_error)?;
        if raw != canonical {
            return Err(StoreError::new(Code::InvalidCanonicalSnapshot, "corpus manifest is not canonical v1 JSON"));
        }
        Ok(parsed.into_root())
    }
}

fn check_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| StoreError::io("cannot stat corpus directory", error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(StoreError::new(Code::UnsafePath, "corpus directory is linked or not a directory"));
    }
    Ok(())
}

fn canonical_error(error: FoundationError) -> StoreError {
    let code = if error.code == FoundationErrorCode::BudgetExceeded { Code::BudgetExceeded }
        else { Code::InvalidCanonicalSnapshot };
    StoreError::new(code, "invalid canonical corpus JSON")
}

fn exact_keys(value: &JsonValue, expected: &[&str], code: Code) -> Result<()> {
    let entries = value.as_object().ok_or_else(|| StoreError::new(code, "expected corpus JSON object"))?;
    if entries.len() != expected.len() || entries.iter().any(|(key, _)| !key.as_str().is_some_and(|name| expected.contains(&name))) {
        return Err(StoreError::new(code, "corpus JSON object has unexpected fields"));
    }
    Ok(())
}

fn string_field<'a>(value: &'a JsonValue, field: &str, code: Code) -> Result<&'a str> {
    value.object_get(field).and_then(JsonValue::as_str)
        .ok_or_else(|| StoreError::new(code, "corpus string field is missing or invalid"))
}

fn digest_field(value: &JsonValue, field: &str, code: Code) -> Result<Digest256> {
    Digest256::from_hex(string_field(value, field, code)?)
        .map_err(|_| StoreError::new(code, "corpus digest field is invalid"))
}

fn optional_revision(value: &JsonValue, field: &str, code: Code) -> Result<Option<SourceRevision>> {
    let entry = value.object_get(field).ok_or_else(|| StoreError::new(code, "corpus revision field is missing"))?;
    if entry.is_null() { Ok(None) } else { Ok(Some(SourceRevision(digest_field(value, field, code)?))) }
}

fn array_field<'a>(value: &'a JsonValue, field: &str, code: Code) -> Result<&'a [JsonValue]> {
    value.object_get(field).and_then(JsonValue::as_array)
        .ok_or_else(|| StoreError::new(code, "corpus array field is missing or invalid"))
}

fn uint_field(value: &JsonValue, field: &str, code: Code) -> Result<u64> {
    value.object_get(field).and_then(JsonValue::as_u64)
        .ok_or_else(|| StoreError::new(code, "corpus unsigned integer field is missing or invalid"))
}

fn mode_field(value: &JsonValue, field: &str, code: Code) -> Result<u32> {
    match uint_field(value, field, code)? { 0o644 => Ok(0o644), 0o755 => Ok(0o755),
        _ => Err(StoreError::new(code, "corpus member mode is invalid")) }
}

fn path_field(value: &JsonValue, field: &str, code: Code) -> Result<RelativePath> {
    RelativePath::parse(string_field(value, field, code)?)
        .map_err(|_| StoreError::new(code, "corpus member path is invalid"))
}

fn check_count(count: usize, maximum: usize) -> Result<()> {
    if count > maximum { Err(StoreError::new(Code::BudgetExceeded, "corpus index exceeds entry limit")) }
    else { Ok(()) }
}

fn parse_identities(value: &JsonValue, files: &BTreeMap<RelativePath, MemberMetadata>, maximum: usize)
    -> Result<BTreeMap<String, RelativePath>> {
    let entries = value.object_get("identities").and_then(JsonValue::as_object)
        .ok_or_else(|| StoreError::new(Code::InvalidIdentityIndex, "identity index is not an object"))?;
    check_count(entries.len(), maximum)?;
    let mut result = BTreeMap::new();
    for (id, item) in entries {
        let id = id.as_str().ok_or_else(|| StoreError::new(Code::InvalidIdentityIndex, "identity is not a Unicode scalar string"))?;
        if id.trim().is_empty() { return Err(StoreError::new(Code::InvalidIdentityIndex, "identity is empty")); }
        let path = RelativePath::parse(item.as_str().ok_or_else(|| StoreError::new(Code::InvalidIdentityIndex, "identity path is invalid"))?)
            .map_err(|_| StoreError::new(Code::InvalidIdentityIndex, "identity path is unsafe"))?;
        if !files.contains_key(&path) { return Err(StoreError::new(Code::InvalidIdentityIndex, "identity points outside snapshot")); }
        result.insert(id.to_owned(), path);
    }
    Ok(result)
}

fn parse_dependencies(value: &JsonValue, files: &BTreeMap<RelativePath, MemberMetadata>, maximum: usize)
    -> Result<BTreeMap<RelativePath, Vec<RelativePath>>> {
    let entries = value.object_get("dependencies").and_then(JsonValue::as_object)
        .ok_or_else(|| StoreError::new(Code::InvalidDependencyIndex, "dependency index is not an object"))?;
    check_count(entries.len(), maximum)?;
    let mut result = BTreeMap::new();
    for (source, targets) in entries {
        let source = RelativePath::parse(source.as_str().ok_or_else(|| StoreError::new(Code::InvalidDependencyIndex, "dependency source is invalid"))?)
            .map_err(|_| StoreError::new(Code::InvalidDependencyIndex, "dependency source is unsafe"))?;
        if !files.contains_key(&source) { return Err(StoreError::new(Code::InvalidDependencyIndex, "dependency source is absent")); }
        let targets = targets.as_array().ok_or_else(|| StoreError::new(Code::InvalidDependencyIndex, "dependency targets are not an array"))?;
        check_count(targets.len(), maximum)?;
        let mut parsed = Vec::with_capacity(targets.len());
        let mut previous: Option<RelativePath> = None;
        for target in targets {
            let target = RelativePath::parse(target.as_str().ok_or_else(|| StoreError::new(Code::InvalidDependencyIndex, "dependency target is invalid"))?)
                .map_err(|_| StoreError::new(Code::InvalidDependencyIndex, "dependency target is unsafe"))?;
            if !files.contains_key(&target) || previous.as_ref().is_some_and(|prior| target <= *prior) {
                return Err(StoreError::new(Code::InvalidDependencyIndex, "dependency targets are absent, duplicate or unsorted"));
            }
            previous = Some(target.clone());
            parsed.push(target);
        }
        result.insert(source, parsed);
    }
    Ok(result)
}

fn validate_retirements(value: &JsonValue, maximum: usize) -> Result<usize> {
    let entries = array_field(value, "retirements", Code::InvalidRetirementIndex)?;
    check_count(entries.len(), maximum)?;
    let mut seen = BTreeSet::new();
    for item in entries {
        exact_keys(item, &["path", "sha256", "event_ref", "event_sha256", "event_size_bytes"], Code::InvalidRetirementIndex)?;
        let path = path_field(item, "path", Code::InvalidRetirementIndex)?;
        let sha = digest_field(item, "sha256", Code::InvalidRetirementIndex)?;
        let event_ref = path_field(item, "event_ref", Code::InvalidRetirementIndex)?;
        let event_sha = digest_field(item, "event_sha256", Code::InvalidRetirementIndex)?;
        uint_field(item, "event_size_bytes", Code::InvalidRetirementIndex)?;
        if path == event_ref || !seen.insert((path, sha, event_ref, event_sha)) {
            return Err(StoreError::new(Code::InvalidRetirementIndex, "retirement is self-referential or duplicate"));
        }
    }
    Ok(entries.len())
}
