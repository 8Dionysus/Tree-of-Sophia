//! Pinned software companions from the existing `corpus_archive.py` Git
//! capture/restore contract. Never reads a worktree or treats scripts as ToS
//! authored members. The caller selects the trusted software revision and exact
//! capture digest; a capture's self-declared Git identity is not an admission.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::manifest::{
    array_field, digest_field, exact_keys, mode_field, path_field, string_field, uint_field,
};
use crate::secure_open::map_open;
use crate::{MemberMetadata, ReadLimits, Result, StoreError, StoreErrorCode as Code};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonMode, JsonValue, RelativePath, canonical_bytes_v1, parse_json,
};

const CODE: Code = Code::UnsupportedFormat;
pub const SOFTWARE_COMPANION_PROFILE_V1: &str = "tos.source-provenance.software-capture.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoftwareCaptureSelectionV1 {
    pub source_git_commit: String,
    pub source_git_tree: String,
    pub capture_manifest_sha256: Digest256,
}

/// A bounded component subset of one already selected, sealed software
/// capture. Component paths are run inputs, never namespace or code authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoftwareComponentSelectionV1 {
    capture: SoftwareCaptureSelectionV1,
    members: BTreeMap<RelativePath, MemberMetadata>,
}

impl SoftwareComponentSelectionV1 {
    pub fn capture(&self) -> &SoftwareCaptureSelectionV1 {
        &self.capture
    }
    pub fn member(&self, path: &RelativePath) -> Option<&MemberMetadata> {
        self.members.get(path)
    }
    pub fn members(&self) -> impl Iterator<Item = &MemberMetadata> {
        self.members.values()
    }
}

/// Separate selected software namespace. The exact selected capture and its
/// restored regular files provide byte evidence only, never runtime/code
/// authority, source semantic acceptance or a current-use rights lease.
#[derive(Debug)]
pub struct SoftwareCaptureReader {
    restored_root: File,
    selection: SoftwareCaptureSelectionV1,
    members: BTreeMap<RelativePath, MemberMetadata>,
    includes: Vec<String>,
    excludes: Vec<String>,
    excluded_parts: Vec<String>,
    limits: ReadLimits,
}

impl SoftwareCaptureReader {
    pub fn profile(&self) -> &'static str {
        SOFTWARE_COMPANION_PROFILE_V1
    }
    pub fn selection(&self) -> &SoftwareCaptureSelectionV1 {
        &self.selection
    }
    /// Bind explicit producer components to exact current capture membership.
    /// The event itself does not choose or issue this selection.
    pub fn select_components(
        &self,
        paths: &[RelativePath],
    ) -> Result<SoftwareComponentSelectionV1> {
        if paths.is_empty() || paths.len() > 128 || paths.len() > self.limits.max_manifest_entries {
            return Err(error(
                Code::BudgetExceeded,
                "software component subset count exceeds budget",
            ));
        }
        let mut members = BTreeMap::new();
        let mut total = 0u64;
        for path in paths {
            let member = self.members.get(path).ok_or_else(|| {
                error(
                    Code::MissingMember,
                    "software component is absent from selected capture",
                )
            })?;
            total = total.checked_add(member.size_bytes).ok_or_else(|| {
                error(
                    Code::BudgetExceeded,
                    "software component byte count overflow",
                )
            })?;
            if total > self.limits.max_selected_object_bytes {
                return Err(error(
                    Code::BudgetExceeded,
                    "software component subset exceeds byte budget",
                ));
            }
            if members.insert(path.clone(), member.clone()).is_some() {
                return Err(error(
                    Code::InvalidSelector,
                    "software component subset repeats a member",
                ));
            }
        }
        Ok(SoftwareComponentSelectionV1 {
            capture: self.selection.clone(),
            members,
        })
    }

    /// Read a current component only from its exact selected capture. No
    /// archived input, ambient worktree or claimed executable is substituted.
    pub fn read_selected_component(
        &self,
        components: &SoftwareComponentSelectionV1,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        check_time(deadline, cancelled)?;
        if components.capture != self.selection {
            return Err(error(
                Code::DescriptorMismatch,
                "software component capture selection differs",
            ));
        }
        let member = components.member(path).ok_or_else(|| {
            error(
                Code::InvalidSelector,
                "software component was not explicitly selected",
            )
        })?;
        if self.members.get(path) != Some(member) {
            return Err(error(
                Code::DescriptorMismatch,
                "software component membership differs",
            ));
        }
        self.read_companion(path, max_bytes, deadline, cancelled)?
            .ok_or_else(|| error(Code::MissingMember, "selected software component is absent"))
    }
    pub fn open(
        capture_root: &Path,
        restored_root: &Path,
        selection: SoftwareCaptureSelectionV1,
        limits: ReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        let limits = limits.validate()?;
        oid(&selection.source_git_commit)?;
        oid(&selection.source_git_tree)?;
        check_time(deadline, cancelled)?;
        let capture = open_root(capture_root)?;
        let restored = open_root(restored_root)?;
        let manifest_raw = read_at(
            &capture,
            "capture.json",
            limits.max_manifest_bytes as u64,
            deadline,
            cancelled,
        )?;
        if Digest256::of_bytes(&manifest_raw) != selection.capture_manifest_sha256 {
            return Err(error(
                Code::DescriptorMismatch,
                "selected software capture digest differs",
            ));
        }
        let manifest = canonical(&manifest_raw, limits)?;
        let version = string_field(&manifest, "schema_version", CODE)?;
        let mut keys = vec![
            "schema_version",
            "source_git_commit",
            "source_git_tree",
            "include_prefixes",
            "member_count",
            "source_bytes",
            "members_sha256",
            "archive_sha256",
            "archive_size_bytes",
        ];
        if version == "tos_corpus_capture_v2" {
            keys.extend(["exclude_prefixes", "exclude_path_parts"]);
        } else if version != "tos_corpus_capture_v1" {
            return Err(error(CODE, "unsupported software Git capture profile"));
        }
        exact_keys(&manifest, &keys, CODE)?;
        if string_field(&manifest, "source_git_commit", CODE)? != selection.source_git_commit
            || string_field(&manifest, "source_git_tree", CODE)? != selection.source_git_tree
        {
            return Err(error(
                Code::DescriptorMismatch,
                "software capture revision differs",
            ));
        }
        let includes = strings(&manifest, "include_prefixes", limits, false)?;
        let excludes = if version == "tos_corpus_capture_v2" {
            strings(&manifest, "exclude_prefixes", limits, false)?
        } else {
            Vec::new()
        };
        let excluded_parts = if version == "tos_corpus_capture_v2" {
            strings(&manifest, "exclude_path_parts", limits, true)?
        } else {
            Vec::new()
        };
        let member_count = uint_field(&manifest, "member_count", CODE)?;
        if member_count > limits.max_manifest_entries as u64 {
            return Err(error(
                Code::BudgetExceeded,
                "software member index exceeds budget",
            ));
        }
        let source_bytes = uint_field(&manifest, "source_bytes", CODE)?;
        // Archive fixity fields belong to the capture/restore transport route.
        // This read-only adapter verifies the selected restored bytes directly.
        digest_field(&manifest, "archive_sha256", CODE)?;
        uint_field(&manifest, "archive_size_bytes", CODE)?;
        let index_raw = read_at(
            &capture,
            "members.jsonl",
            limits.max_manifest_bytes as u64,
            deadline,
            cancelled,
        )?;
        if Digest256::of_bytes(&index_raw) != digest_field(&manifest, "members_sha256", CODE)? {
            return Err(error(
                Code::DescriptorMismatch,
                "software member index digest differs",
            ));
        }
        let mut members = BTreeMap::new();
        let mut last = None;
        let mut total = 0u64;
        for line in index_raw.split_inclusive(|b| *b == b'\n') {
            check_time(deadline, cancelled)?;
            if !line.ends_with(b"\n") || members.len() >= limits.max_manifest_entries {
                return Err(error(
                    Code::InvalidMemberIndex,
                    "software member line incomplete or over budget",
                ));
            }
            let value = canonical(line, limits)?;
            exact_keys(
                &value,
                &["path", "git_blob_oid", "size_bytes", "sha256", "mode"],
                CODE,
            )?;
            let path = path_field(&value, "path", CODE)?;
            if last.as_ref().is_some_and(|previous| &path <= previous)
                || !matches_prefix(path.as_str(), &includes)
                || matches_prefix(path.as_str(), &excludes)
                || path
                    .as_str()
                    .split('/')
                    .any(|part| excluded_parts.iter().any(|p| p == part))
            {
                return Err(error(
                    Code::InvalidMemberIndex,
                    "software capture member selection differs",
                ));
            }
            oid(string_field(&value, "git_blob_oid", CODE)?)?;
            let size_bytes = uint_field(&value, "size_bytes", CODE)?;
            total = total.checked_add(size_bytes).ok_or_else(|| {
                error(Code::BudgetExceeded, "software source byte count overflow")
            })?;
            let metadata = MemberMetadata {
                path: path.clone(),
                size_bytes,
                sha256: digest_field(&value, "sha256", CODE)?,
                mode: mode_field(&value, "mode", CODE)?,
            };
            members.insert(path.clone(), metadata);
            last = Some(path);
        }
        if members.len() as u64 != member_count || total != source_bytes {
            return Err(error(
                Code::DescriptorMismatch,
                "software capture membership totals differ",
            ));
        }
        let receipt = canonical(
            &read_at(
                &restored,
                "restore-receipt.json",
                limits.max_manifest_bytes as u64,
                deadline,
                cancelled,
            )?,
            limits,
        )?;
        exact_keys(
            &receipt,
            &[
                "schema_version",
                "source_git_commit",
                "member_count",
                "source_bytes",
                "manifest_sha256",
            ],
            CODE,
        )?;
        if string_field(&receipt, "schema_version", CODE)? != "tos_corpus_restore_receipt_v1"
            || string_field(&receipt, "source_git_commit", CODE)? != selection.source_git_commit
            || uint_field(&receipt, "member_count", CODE)? != member_count
            || uint_field(&receipt, "source_bytes", CODE)? != source_bytes
            || digest_field(&receipt, "manifest_sha256", CODE)? != selection.capture_manifest_sha256
        {
            return Err(error(
                Code::DescriptorMismatch,
                "software restore receipt differs from selected capture",
            ));
        }
        check_time(deadline, cancelled)?;
        Ok(Self {
            restored_root: restored,
            selection,
            members,
            includes,
            excludes,
            excluded_parts,
            limits,
        })
    }

    /// Only exact current software companions are served here. Retained
    /// builder/schema fallback is a source-owned named capture lookup, handled
    /// by the provenance rule against its separate immutable corpus cut.
    pub fn read_current(
        &self,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<u8>>> {
        check_time(deadline, cancelled)?;
        if !path.as_str().starts_with("scripts/") {
            return Ok(None);
        }
        self.read_companion(path, max_bytes, deadline, cancelled)
    }

    fn read_companion(
        &self,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<u8>>> {
        check_time(deadline, cancelled)?;
        if !matches_prefix(path.as_str(), &self.includes)
            || matches_prefix(path.as_str(), &self.excludes)
            || path
                .as_str()
                .split('/')
                .any(|part| self.excluded_parts.iter().any(|p| p == part))
        {
            return Err(error(
                CODE,
                "software companion is outside selected capture scope",
            ));
        }
        let Some(member) = self.members.get(path) else {
            return Ok(None);
        };
        let cap = max_bytes.min(self.limits.max_selected_object_bytes);
        if member.size_bytes > cap {
            return Err(error(
                Code::BudgetExceeded,
                "software companion exceeds read budget",
            ));
        }
        let raw = read_at(
            &self.restored_root,
            path.as_str(),
            member.size_bytes,
            deadline,
            cancelled,
        )?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(error(
                Code::CorruptSelectedObject,
                "software companion fixity differs",
            ));
        }
        Ok(Some(raw))
    }
}

fn open_root(path: &Path) -> Result<File> {
    if path == Path::new("/") {
        return Err(error(
            Code::InvalidRoot,
            "filesystem root is not a software capture",
        ));
    }
    tos_fd_open::open_absolute_directory(path).map_err(|e| {
        map_open(
            e,
            Code::InvalidRoot,
            "cannot securely open software capture root",
        )
    })
}
fn read_at(
    root: &File,
    path: &str,
    cap: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    check_time(deadline, cancelled)?;
    RelativePath::parse(path)
        .map_err(|_| error(Code::UnsafePath, "software member path is invalid"))?;
    // The shared FD opener deliberately accepts one component per call.
    // Retain each opened directory rather than joining an absolute pathname
    // or weakening its no-follow rule for nested script refs.
    let mut directory = root
        .try_clone()
        .map_err(|e| StoreError::io("cannot retain software root descriptor", e))?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            break;
        }
        check_time(deadline, cancelled)?;
        directory = tos_fd_open::open_directory_at(&directory, Path::new(part)).map_err(|e| {
            map_open(
                e,
                Code::UnsafePath,
                "cannot securely open software member directory",
            )
        })?;
    }
    let leaf = path
        .rsplit('/')
        .next()
        .expect("validated relative path is nonempty");
    let mut file = tos_fd_open::open_regular_at(&directory, Path::new(leaf)).map_err(|e| {
        map_open(
            e,
            Code::UnsafePath,
            "cannot securely open software capture member",
        )
    })?;
    if file
        .metadata()
        .map_err(|e| StoreError::io("cannot stat opened software member", e))?
        .len()
        > cap
    {
        return Err(error(
            Code::BudgetExceeded,
            "software capture member exceeds byte budget",
        ));
    }
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        check_time(deadline, cancelled)?;
        let remaining = cap.saturating_sub(raw.len() as u64);
        let request = remaining.saturating_add(1).min(chunk.len() as u64) as usize;
        let count = file
            .read(&mut chunk[..request])
            .map_err(|e| StoreError::io("cannot read opened software member", e))?;
        if count == 0 {
            break;
        }
        if count as u64 > remaining {
            return Err(error(
                Code::BudgetExceeded,
                "software capture member grew beyond budget",
            ));
        }
        raw.extend_from_slice(&chunk[..count]);
    }
    check_time(deadline, cancelled)?;
    Ok(raw)
}
fn canonical(raw: &[u8], limits: ReadLimits) -> Result<JsonValue> {
    let parsed = parse_json(raw, JsonMode::PublishedStrict, limits.json)
        .map_err(|_| error(CODE, "software capture JSON is invalid or over budget"))?;
    let canonical = canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::CorpusSnapshotV1,
        limits.json,
    )
    .map_err(|_| error(CODE, "software capture JSON cannot be canonicalized"))?;
    if canonical != raw {
        return Err(error(
            Code::InvalidCanonicalSnapshot,
            "software capture JSON is not canonical",
        ));
    }
    Ok(parsed.into_root())
}
fn strings(value: &JsonValue, field: &str, limits: ReadLimits, part: bool) -> Result<Vec<String>> {
    let rows = array_field(value, field, CODE)?;
    if rows.len() > limits.max_manifest_entries {
        return Err(error(
            Code::BudgetExceeded,
            "software capture selection exceeds budget",
        ));
    }
    let mut result = Vec::new();
    for row in rows {
        let text = row
            .as_str()
            .ok_or_else(|| error(CODE, "software capture selection is not a string"))?;
        if part {
            if text.is_empty()
                || matches!(text, "." | "..")
                || text.contains(['/', '\\'])
                || text.chars().any(|c| (c as u32) < 32 || c == '\u{7f}')
            {
                return Err(error(
                    CODE,
                    "software capture excluded component is invalid",
                ));
            }
        } else {
            RelativePath::parse(text)
                .map_err(|_| error(CODE, "software capture selection path is invalid"))?;
        }
        if text.ends_with('/')
            || result
                .last()
                .is_some_and(|previous: &String| previous.as_str() >= text)
        {
            return Err(error(
                CODE,
                "software capture selection is not normalized and unique",
            ));
        }
        result.push(text.to_owned());
    }
    Ok(result)
}
fn matches_prefix(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| {
        path == p
            || path
                .strip_prefix(p)
                .is_some_and(|tail| tail.starts_with('/'))
    })
}
fn oid(value: &str) -> Result<()> {
    if value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(error(CODE, "software capture Git identity is invalid"))
    }
}
fn check_time(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        Err(error(
            Code::BudgetExceeded,
            "software companion read cancelled or expired",
        ))
    } else {
        Ok(())
    }
}
fn error(code: Code, detail: &'static str) -> StoreError {
    StoreError::new(code, detail)
}
