//! OPS collector and guarded publisher draft for prepared dossiers.
//!
//! This is a source-only handoff for `tos-ops-mechanics-plan`. It consumes the
//! readiness owner's retained DOCX/source snapshot, fills only renderer inputs
//! that the legacy plant reads, binds complete target preimages, and publishes
//! in the renderer's Python-phase order through the caller's one
//! `ResearchExecution`. It does not grant semantic, rights, review, or canon
//! admission.

use crate::prepared_dossier_docx_adapter::NativePreparedDossierContentValidator;
use crate::prepared_dossier_native_directory::visit_selected_directory;
use crate::prepared_dossier_readiness::{
    self as readiness, PreparedDossierArtifactSnapshot, PreparedDossierDirectoryEntry,
    PreparedDossierDirectoryStatus, PreparedDossierEntryKind, PreparedDossierReadinessExecution,
    PreparedSourceProfile, ReadinessAssessment, ReadinessInputs,
};
use crate::prepared_dossier_render::{
    self, ObsoleteBranchIntent, PlantingDirectoryPreimage, PlantingOutputLeafState,
    PlantingOutputPreimage, PlantingSourcePreimage, PreparedDossierPackageInput,
    PreparedDossierPackageRefs, PreparedDossierPlantingInputs,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use tos_compiler::research_execution::ResearchExecution;
use tos_compiler::source_philosophy_dossier_docx::DocxDocument;
use tos_compiler::source_philosophy_dossier_extract::{PreparedDossier, extract_dossier};
use tos_compiler::source_philosophy_multilingual::Multilingual;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

const MASTER_ROOT: &str = "ToS/philosophy/atlas/master-tables";
const PROPOSED_NODES_ROOT: &str = "ToS/philosophy/graph-workbench/proposed-nodes";
const PROPOSED_RELATIONS_ROOT: &str = "ToS/philosophy/graph-workbench/proposed-relations";
const LANGUAGE_PACKETS_ROOT: &str = "ToS/philosophy/graph-workbench/language-packets";
const BRANCH_FRAGMENTS_ROOT: &str = "ToS/philosophy/graph-workbench/branch-fragments";
const PROMOTION_LEDGER_ROOT: &str = "ToS/philosophy/graph-workbench/promotion-ledger";
const PHILOSOPHY_TREE: &str = "ToS/philosophy";
const RESEARCH_DOSSIER_OUTPUT_DIR: &str = "ToS/research-packets/deep-research/philosophy/dossiers";
const OUTPUT_MODE: u32 = 0o644;
const FILE_CAP: u64 = 256 * 1024 * 1024;
const JSON_CAP: usize = 256 * 1024 * 1024;

// Same Linux nonblocking advisory lock used by fs2::FileExt, without adding
// a dependency: the held parent descriptor owns the lock until it is dropped.
fn try_lock_parent(parent: &File) -> std::io::Result<()> {
    let result = unsafe { libc::flock(parent.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct NativePreparedDossierInputs {
    pub repository_root: PathBuf,
    pub doc_root: PathBuf,
    pub output_root: PathBuf,
    pub source_profile: PreparedSourceProfile,
    pub selected_table_id: Option<String>,
    pub plant: bool,
    pub readiness: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectoryFence {
    status: PreparedDossierDirectoryStatus,
    entries: Vec<PreparedDossierDirectoryEntry>,
    identity: Option<(u64, u64, u32)>,
}

#[derive(Clone, Default)]
struct DirectoryFences(BTreeMap<PathBuf, DirectoryFence>);

struct ReadinessExecution<'a> {
    source: &'a ResearchExecution,
    docs: &'a ResearchExecution,
    doc_root: &'a Path,
    doc_fences: DirectoryFences,
}

impl ReadinessExecution<'_> {
    fn verify_doc_fences(&self) -> Result<(), String> {
        verify_directory_fences(self.docs, &self.doc_fences)
    }
}

impl PreparedDossierReadinessExecution for ReadinessExecution<'_> {
    fn read_file(&mut self, path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
        if path.starts_with(self.source.root()) {
            return read_selected_file(self.source, path, max_bytes).map_err(|e| {
                format!(
                    "cannot read selected repository input {}: {e}",
                    path.display()
                )
            });
        }
        if path.starts_with(self.doc_root) {
            return read_selected_file(self.docs, path, max_bytes)
                .map_err(|e| format!("cannot read selected DOCX input {}: {e}", path.display()));
        }
        Err(format!(
            "readiness input is outside selected roots: {}",
            path.display()
        ))
    }

    fn tick(&mut self, work_units: u64) -> Result<(), String> {
        self.source.tick(work_units)
    }

    fn visit_directory(
        &mut self,
        path: &Path,
        visit: &mut dyn FnMut(PreparedDossierDirectoryEntry) -> Result<(), String>,
    ) -> Result<PreparedDossierDirectoryStatus, String> {
        let mut entries = Vec::new();
        let status = visit_selected_directory(self.docs, path, &mut |entry| {
            entries.push(entry.clone());
            visit(entry)
        })?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let fence = DirectoryFence {
            status,
            entries,
            identity: directory_identity(self.docs, path, status)?,
        };
        if let Some(previous) = self.doc_fences.0.insert(path.to_owned(), fence.clone())
            && previous != fence
        {
            return Err(format!(
                "selected DOCX directory changed during readiness: {}",
                path.display()
            ));
        }
        Ok(status)
    }
}

#[derive(Default)]
struct TargetTree {
    directories: DirectoryFences,
    paths: BTreeSet<String>,
    files: BTreeSet<String>,
    unsafe_paths: BTreeSet<String>,
    branch_manifests: BTreeMap<String, Value>,
    obsolete_files: Option<Vec<PlantingSourcePreimage>>,
    obsolete_directories: Option<Vec<PlantingDirectoryPreimage>>,
    obsolete_output_preimages: Option<Vec<PlantingOutputPreimage>>,
    obsolete_file_stamps: Option<BTreeMap<String, FileStamp>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    device: u64,
    inode: u64,
    mode: u32,
    size_bytes: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VerifiedSourcePin {
    preimage: PlantingSourcePreimage,
    stamp: FileStamp,
}

fn source_mode(profile: PreparedSourceProfile) -> JsonMode {
    match profile {
        PreparedSourceProfile::PublishedStrict => JsonMode::PublishedStrict,
        PreparedSourceProfile::LegacyPythonObserved => JsonMode::LegacyPythonObserved,
    }
}

fn checked_reference(reference: &str) -> Result<&Path, String> {
    let path = Path::new(reference);
    if reference.is_empty()
        || reference.len() > 4096
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!(
            "invalid prepared-dossier relative reference: {reference}"
        ));
    }
    Ok(path)
}

fn portable_relative(root: &ResearchExecution, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root.root())
        .map_err(|_| format!("{} is outside the selected root", path.display()))?;
    let value = relative
        .to_str()
        .ok_or_else(|| format!("selected relative path is not UTF-8: {}", path.display()))?
        .replace('\\', "/");
    checked_reference(&value)?;
    Ok(value)
}

fn read_selected_file(
    root: &ResearchExecution,
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    root.check()?;
    let reference = portable_relative(root, path)?;
    let mut file = root.source_file(&reference, max_bytes)?;
    let before = file.metadata().map_err(|error| error.to_string())?;
    let bytes = root.read_file(&mut file, max_bytes)?;
    root.verify_file_unchanged(&file, &before)?;
    root.check()?;
    Ok(bytes)
}

fn parse_source_value(
    raw: &[u8],
    profile: PreparedSourceProfile,
    max_bytes: usize,
) -> Result<Value, String> {
    let limits =
        JsonLimits::new(max_bytes, 96, 2_000_000, 4300).map_err(|error| error.to_string())?;
    parse_json(raw, source_mode(profile), limits)
        .map_err(|error| format!("source JSON profile {}: {error}", profile.as_str()))?;
    serde_json::from_slice(raw)
        .map_err(|error| format!("source JSON value under {}: {error}", profile.as_str()))
}

fn read_json(
    root: &ResearchExecution,
    reference: &str,
    profile: PreparedSourceProfile,
) -> Result<(Value, PlantingSourcePreimage), String> {
    checked_reference(reference)?;
    let mut file = root.source_file(reference, FILE_CAP)?;
    let before = file.metadata().map_err(|error| error.to_string())?;
    let raw = root.read_file(&mut file, FILE_CAP)?;
    root.verify_file_unchanged(&file, &before)?;
    root.tick(raw.len() as u64)?;
    let value = parse_source_value(&raw, profile, JSON_CAP)
        .map_err(|error| format!("{reference}: {error}"))?;
    if !value.is_object() {
        return Err(format!("{reference} must contain a JSON object"));
    }
    let pin = PlantingSourcePreimage {
        reference: reference.to_owned(),
        sha256: Digest256::of_bytes(&raw).to_hex(),
        size_bytes: raw.len() as u64,
    };
    Ok((value, pin))
}

fn json_field_path(package: &Value, field: &str, table_id: &str) -> Result<String, String> {
    let value = package
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{table_id}.{field} must be a repo-relative path"))?;
    checked_reference(value)?;
    Ok(value.to_owned())
}

fn refs_for(table_id: &str, package: &Value) -> Result<PreparedDossierPackageRefs, String> {
    Ok(PreparedDossierPackageRefs {
        master_rows: format!("{MASTER_ROOT}/{table_id}/rows.jsonl"),
        master_manifest: format!("{MASTER_ROOT}/{table_id}/table.manifest.json"),
        intake_manifest: json_field_path(package, "intake_manifest_ref", table_id)?,
        extraction_coverage: json_field_path(package, "extraction_coverage_ref", table_id)?,
        proposed_nodes: format!("{PROPOSED_NODES_ROOT}/{table_id}-prepared-dossiers.jsonl"),
        proposed_relations: format!("{PROPOSED_RELATIONS_ROOT}/{table_id}-prepared-dossiers.jsonl"),
        language_packets: format!("{LANGUAGE_PACKETS_ROOT}/{table_id}-text-bearing-nodes.jsonl"),
        branch_fragments: format!(
            "{BRANCH_FRAGMENTS_ROOT}/{table_id}-prepared-dossier-branches.json"
        ),
        promotion_ledger: format!("{PROMOTION_LEDGER_ROOT}/{table_id}-prepared-dossiers.md"),
    })
}

fn configured_research_dossier_outputs(
    assessment: &ReadinessAssessment,
) -> Result<BTreeSet<String>, String> {
    let parent = Path::new(RESEARCH_DOSSIER_OUTPUT_DIR);
    let mut references = BTreeSet::new();
    for table_id in &assessment.supported_table_ids {
        let package = assessment
            .package_configs
            .get(table_id)
            .ok_or_else(|| format!("readiness package config missing: {table_id}"))?;
        let refs = refs_for(table_id, package)?;
        for reference in [refs.intake_manifest, refs.extraction_coverage] {
            let path = checked_reference(&reference)?;
            let canonical_reference = format!(
                "{RESEARCH_DOSSIER_OUTPUT_DIR}/{}",
                path.file_name()
                    .and_then(OsStr::to_str)
                    .ok_or_else(|| format!(
                        "configured research dossier output has no UTF-8 leaf: {reference}"
                    ))?
            );
            if reference != canonical_reference || path.parent() != Some(parent) {
                return Err(format!(
                    "configured research dossier output escaped its exact owner directory: {reference}"
                ));
            }
            if !references.insert(reference.clone()) {
                return Err(format!(
                    "duplicate configured research dossier output: {reference}"
                ));
            }
        }
    }
    if references.len() != 6 {
        return Err(format!(
            "expected exactly six configured research dossier outputs, found {}",
            references.len()
        ));
    }
    Ok(references)
}

fn record_research_dossier_parent_fences(
    selected: &ResearchExecution,
    references: &BTreeSet<String>,
    fences: &mut DirectoryFences,
) -> Result<(), String> {
    if references.len() != 6 {
        return Err(
            "research dossier output fences require the exact six configured leaves".into(),
        );
    }
    let mut current = String::new();
    for component in Path::new(RESEARCH_DOSSIER_OUTPUT_DIR).components() {
        let Component::Normal(name) = component else {
            return Err("invalid research dossier output directory".into());
        };
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(
            name.to_str()
                .ok_or("research dossier output directory is not UTF-8")?,
        );
        let absolute = selected.root().join(&current);
        let (status, _) = record_directory(selected, &absolute, fences)?;
        if status != PreparedDossierDirectoryStatus::Present {
            return Err(format!(
                "research dossier output parent is missing: {current}"
            ));
        }
    }
    Ok(())
}

fn descriptor_open_directory(
    root: &ResearchExecution,
    reference: &str,
) -> Result<Option<File>, String> {
    let path = checked_reference(reference)?;
    let mut current = root
        .root_directory()
        .try_clone()
        .map_err(|error| error.to_string())?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(format!("invalid directory reference: {reference}"));
        };
        root.tick(1)?;
        let leaf = CString::new(name.as_bytes()).map_err(|error| error.to_string())?;
        let fd = unsafe {
            libc::openat(
                current.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ENOENT) {
                return Ok(None);
            }
            return Err(format!("open directory {reference}: {error}"));
        }
        current = unsafe { File::from_raw_fd(fd) };
    }
    root.check()?;
    Ok(Some(current))
}

fn parent_and_leaf(
    root: &ResearchExecution,
    reference: &str,
) -> Result<Option<(File, CString)>, String> {
    let path = checked_reference(reference)?;
    let mut components = path.components().collect::<Vec<_>>();
    let leaf = components
        .pop()
        .and_then(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .ok_or_else(|| format!("invalid leaf reference: {reference}"))?;
    let parent_ref = components
        .iter()
        .map(|part| match part {
            Component::Normal(name) => name.to_string_lossy(),
            _ => std::borrow::Cow::Borrowed(""),
        })
        .collect::<Vec<_>>()
        .join("/");
    let parent = if parent_ref.is_empty() {
        Some(
            root.root_directory()
                .try_clone()
                .map_err(|error| error.to_string())?,
        )
    } else {
        descriptor_open_directory(root, &parent_ref)?
    };
    let Some(parent) = parent else {
        return Ok(None);
    };
    let leaf = CString::new(leaf.as_bytes()).map_err(|error| error.to_string())?;
    Ok(Some((parent, leaf)))
}

fn stat_at(parent: &File, leaf: &CString) -> Result<Option<libc::stat>, String> {
    let mut storage = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            storage.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) {
            return Ok(None);
        }
        return Err(error.to_string());
    }
    Ok(Some(unsafe { storage.assume_init() }))
}

fn output_state(
    root: &ResearchExecution,
    reference: &str,
) -> Result<PlantingOutputLeafState, String> {
    root.check()?;
    let Some((parent, leaf)) = parent_and_leaf(root, reference)? else {
        return Ok(PlantingOutputLeafState::Absent);
    };
    let Some(stat) = stat_at(&parent, &leaf)? else {
        return Ok(PlantingOutputLeafState::Absent);
    };
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(format!(
            "planned output is a symlink or non-regular leaf: {reference}"
        ));
    }
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(format!(
            "open planned output {reference}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let before = file.metadata().map_err(|error| error.to_string())?;
    if !before.is_file() || before.dev() != stat.st_dev as u64 || before.ino() != stat.st_ino as u64
    {
        return Err(format!(
            "planned output identity changed during observation: {reference}"
        ));
    }
    root.tick(before.len())?;
    let sha256 = root.hash_file(&mut file, FILE_CAP)?;
    root.verify_file_unchanged(&file, &before)?;
    let after = stat_at(&parent, &leaf)?
        .ok_or_else(|| format!("planned output disappeared: {reference}"))?;
    if after.st_dev != stat.st_dev
        || after.st_ino != stat.st_ino
        || after.st_size != stat.st_size
        || after.st_mtime != stat.st_mtime
        || after.st_mtime_nsec != stat.st_mtime_nsec
        || after.st_ctime != stat.st_ctime
        || after.st_ctime_nsec != stat.st_ctime_nsec
    {
        return Err(format!(
            "planned output changed during observation: {reference}"
        ));
    }
    Ok(PlantingOutputLeafState::Present {
        sha256,
        size_bytes: before.len(),
        device: before.dev(),
        inode: before.ino(),
        mode: before.mode(),
        mtime: before.mtime(),
        mtime_nsec: before.mtime_nsec(),
        ctime: before.ctime(),
        ctime_nsec: before.ctime_nsec(),
    })
}

fn directory_preimage(reference: &str, file: &File) -> Result<PlantingDirectoryPreimage, String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_dir() {
        return Err(format!("cleanup path is not a directory: {reference}"));
    }
    Ok(PlantingDirectoryPreimage {
        reference: reference.to_owned(),
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        mtime: metadata.mtime(),
        mtime_nsec: metadata.mtime_nsec(),
        ctime: metadata.ctime(),
        ctime_nsec: metadata.ctime_nsec(),
    })
}

fn directory_identity(
    selected: &ResearchExecution,
    path: &Path,
    status: PreparedDossierDirectoryStatus,
) -> Result<Option<(u64, u64, u32)>, String> {
    if status == PreparedDossierDirectoryStatus::Missing {
        return Ok(None);
    }
    let relative = path
        .strip_prefix(selected.root())
        .map_err(|_| format!("{} is outside the selected root", path.display()))?;
    let directory = if relative.as_os_str().is_empty() {
        selected
            .root_directory()
            .try_clone()
            .map_err(|error| error.to_string())?
    } else {
        let reference = portable_relative(selected, path)?;
        descriptor_open_directory(selected, &reference)?
            .ok_or_else(|| format!("selected directory disappeared: {reference}"))?
    };
    let metadata = directory.metadata().map_err(|error| error.to_string())?;
    Ok(Some((metadata.dev(), metadata.ino(), metadata.mode())))
}

fn file_stamp(metadata: &std::fs::Metadata) -> FileStamp {
    FileStamp {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        size_bytes: metadata.len(),
        mtime: metadata.mtime(),
        mtime_nsec: metadata.mtime_nsec(),
        ctime: metadata.ctime(),
        ctime_nsec: metadata.ctime_nsec(),
    }
}

fn stat_stamp(stat: &libc::stat) -> FileStamp {
    FileStamp {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
        mode: stat.st_mode as u32,
        size_bytes: stat.st_size as u64,
        mtime: stat.st_mtime,
        mtime_nsec: stat.st_mtime_nsec,
        ctime: stat.st_ctime,
        ctime_nsec: stat.st_ctime_nsec,
    }
}

fn present_output_state(sha256: String, stamp: FileStamp) -> PlantingOutputLeafState {
    PlantingOutputLeafState::Present {
        sha256,
        size_bytes: stamp.size_bytes,
        device: stamp.device,
        inode: stamp.inode,
        mode: stamp.mode,
        mtime: stamp.mtime,
        mtime_nsec: stamp.mtime_nsec,
        ctime: stamp.ctime,
        ctime_nsec: stamp.ctime_nsec,
    }
}

fn output_state_stamp(state: &PlantingOutputLeafState) -> Option<FileStamp> {
    match state {
        PlantingOutputLeafState::Absent => None,
        PlantingOutputLeafState::Present {
            size_bytes,
            device,
            inode,
            mode,
            mtime,
            mtime_nsec,
            ctime,
            ctime_nsec,
            ..
        } => Some(FileStamp {
            device: *device,
            inode: *inode,
            mode: *mode,
            size_bytes: *size_bytes,
            mtime: *mtime,
            mtime_nsec: *mtime_nsec,
            ctime: *ctime,
            ctime_nsec: *ctime_nsec,
        }),
    }
}

fn file_pin(
    root: &ResearchExecution,
    reference: &str,
) -> Result<(PlantingSourcePreimage, PlantingOutputPreimage, FileStamp), String> {
    let mut file = root.source_file(reference, FILE_CAP)?;
    let before = file.metadata().map_err(|error| error.to_string())?;
    let stamp = file_stamp(&before);
    root.tick(before.len())?;
    let sha256 = root.hash_file(&mut file, FILE_CAP)?;
    root.verify_file_unchanged(&file, &before)?;
    let after = file.metadata().map_err(|error| error.to_string())?;
    if file_stamp(&after) != stamp {
        return Err(format!("source changed while pinning: {reference}"));
    }
    let Some((parent, leaf)) = parent_and_leaf(root, reference)? else {
        return Err(format!("source disappeared while pinning: {reference}"));
    };
    if stat_at(&parent, &leaf)?.as_ref().map(stat_stamp) != Some(stamp) {
        return Err(format!("source path changed while pinning: {reference}"));
    }
    Ok((
        PlantingSourcePreimage {
            reference: reference.to_owned(),
            sha256: sha256.clone(),
            size_bytes: stamp.size_bytes,
        },
        PlantingOutputPreimage {
            reference: reference.to_owned(),
            state: present_output_state(sha256, stamp),
        },
        stamp,
    ))
}

fn is_under(reference: &str, root: &str) -> bool {
    reference == root
        || reference
            .strip_prefix(root)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn record_directory(
    selected: &ResearchExecution,
    absolute: &Path,
    fences: &mut DirectoryFences,
) -> Result<
    (
        PreparedDossierDirectoryStatus,
        Vec<PreparedDossierDirectoryEntry>,
    ),
    String,
> {
    let mut entries = Vec::new();
    let status = visit_selected_directory(selected, absolute, &mut |entry| {
        entries.push(entry);
        Ok(())
    })?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let fence = DirectoryFence {
        status,
        entries: entries.clone(),
        identity: directory_identity(selected, absolute, status)?,
    };
    if let Some(previous) = fences.0.insert(absolute.to_owned(), fence.clone())
        && previous != fence
    {
        return Err(format!(
            "selected source directory changed during inventory: {}",
            absolute.display()
        ));
    }
    Ok((status, entries))
}

fn walk_target_tree(
    selected: &ResearchExecution,
    relative: &str,
    tree: &mut TargetTree,
) -> Result<bool, String> {
    let absolute = selected.root().join(relative);
    let (status, entries) = record_directory(selected, &absolute, &mut tree.directories)?;
    if status == PreparedDossierDirectoryStatus::Missing {
        return Ok(false);
    }
    tree.paths.insert(relative.to_owned());
    if relative == prepared_dossier_render::OBSOLETE_GENERATED_BRANCH {
        let directory = descriptor_open_directory(selected, relative)?
            .ok_or("obsolete generated branch disappeared")?;
        tree.obsolete_directories = Some(vec![directory_preimage(relative, &directory)?]);
        tree.obsolete_file_stamps = Some(BTreeMap::new());
    }
    for entry in entries {
        selected.check()?;
        let name = entry
            .name
            .to_str()
            .ok_or_else(|| format!("non-UTF-8 path under {PHILOSOPHY_TREE}"))?;
        let child = format!("{relative}/{name}");
        tree.paths.insert(child.clone());
        match entry.kind {
            PreparedDossierEntryKind::Directory => {
                if name == "branch.manifest.json" {
                    return Err(format!("expected generated file is a directory: {child}"));
                }
                if is_under(&child, prepared_dossier_render::OBSOLETE_GENERATED_BRANCH) {
                    let directory = descriptor_open_directory(selected, &child)?
                        .ok_or_else(|| format!("cleanup directory disappeared: {child}"))?;
                    tree.obsolete_directories
                        .as_mut()
                        .ok_or("obsolete branch inventory lost its root")?
                        .push(directory_preimage(&child, &directory)?);
                }
                walk_target_tree(selected, &child, tree)?;
            }
            PreparedDossierEntryKind::RegularFile => {
                tree.files.insert(child.clone());
                if is_under(&child, prepared_dossier_render::OBSOLETE_GENERATED_BRANCH)
                    && Path::new(&child).file_name().and_then(OsStr::to_str)
                        != Some("branch.manifest.json")
                {
                    return Err(format!(
                        "{} contains non-generated files; refusing to remove it",
                        prepared_dossier_render::OBSOLETE_GENERATED_BRANCH
                    ));
                }
            }
            PreparedDossierEntryKind::Symlink | PreparedDossierEntryKind::Special => {
                tree.unsafe_paths.insert(child.clone());
                if name == "branch.manifest.json"
                    || is_under(&child, prepared_dossier_render::OBSOLETE_GENERATED_BRANCH)
                    || is_under(prepared_dossier_render::OBSOLETE_GENERATED_BRANCH, &child)
                {
                    return Err(format!("unsafe symlink or special source entry: {child}"));
                }
            }
        }
    }
    if relative == prepared_dossier_render::OBSOLETE_GENERATED_BRANCH {
        let mut files = Vec::new();
        let mut output_preimages = Vec::new();
        let mut stamps = BTreeMap::new();
        for reference in tree
            .files
            .iter()
            .filter(|path| is_under(path, prepared_dossier_render::OBSOLETE_GENERATED_BRANCH))
        {
            let (pin, output, stamp) = file_pin(selected, reference)?;
            files.push(pin);
            output_preimages.push(output);
            stamps.insert(reference.clone(), stamp);
        }
        files.sort_by(|a, b| a.reference.cmp(&b.reference));
        tree.obsolete_files = Some(files);
        tree.obsolete_output_preimages = Some(output_preimages);
        tree.obsolete_file_stamps = Some(stamps);
    }
    Ok(true)
}

fn verify_directory_fences(
    selected: &ResearchExecution,
    fences: &DirectoryFences,
) -> Result<(), String> {
    for (path, expected) in &fences.0 {
        selected.check()?;
        let mut entries = Vec::new();
        let status = visit_selected_directory(selected, path, &mut |entry| {
            entries.push(entry);
            Ok(())
        })?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        if status != expected.status
            || entries != expected.entries
            || directory_identity(selected, path, status)? != expected.identity
        {
            return Err(format!(
                "selected source directory changed before planting: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn fence_path(selected: &ResearchExecution, reference: &str) -> PathBuf {
    selected.root().join(reference)
}

fn insert_directory_entry(
    selected: &ResearchExecution,
    fences: &mut DirectoryFences,
    parent_ref: &str,
    name: &str,
    kind: PreparedDossierEntryKind,
) -> Result<(), String> {
    let parent_path = fence_path(selected, parent_ref);
    let parent = fences
        .0
        .get_mut(&parent_path)
        .ok_or_else(|| format!("output parent was not source-inventoried: {parent_ref}"))?;
    if parent.status != PreparedDossierDirectoryStatus::Present {
        return Err(format!("output parent is not present: {parent_ref}"));
    }
    let name = OsString::from(name);
    if parent
        .entries
        .iter()
        .any(|entry| entry.name.as_os_str() == name.as_os_str())
    {
        return Err(format!(
            "projected output path already exists in directory fence: {parent_ref}/{name:?}"
        ));
    }
    parent
        .entries
        .push(PreparedDossierDirectoryEntry { name, kind });
    parent.entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(())
}

/// Advance the expected source-tree directory view only for the leaf that was
/// just committed. This keeps per-leaf rescans useful after earlier planned
/// writes while detecting unrelated changes to the recursive branch census.
fn record_written_output(
    target: &ResearchExecution,
    fences: &mut DirectoryFences,
    reference: &str,
    was_absent: bool,
    research_outputs: &BTreeSet<String>,
) -> Result<(), String> {
    if !was_absent {
        return Ok(());
    }
    let fence_root = if reference.starts_with(&format!("{PHILOSOPHY_TREE}/")) {
        PHILOSOPHY_TREE
    } else if research_outputs.contains(reference) {
        RESEARCH_DOSSIER_OUTPUT_DIR
    } else {
        return Err(format!(
            "planned output escaped its exact fenced roots: {reference}"
        ));
    };
    let prefix = format!("{fence_root}/");
    let suffix = reference
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("planned output is outside its fenced root: {reference}"))?;
    let path = Path::new(suffix);
    let components = path
        .components()
        .map(|part| match part {
            Component::Normal(name) => name
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("planned output path is not UTF-8: {reference}")),
            _ => Err(format!("invalid planned output path: {reference}")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(format!("planned output has no leaf: {reference}"));
    }
    let mut directory_ref = fence_root.to_owned();
    let root_path = fence_path(target, &directory_ref);
    if !fences.0.contains_key(&root_path) {
        return Err(format!(
            "output root was not source-inventoried: {fence_root}"
        ));
    }
    for name in &components[..components.len() - 1] {
        let child_ref = format!("{directory_ref}/{name}");
        let child_path = fence_path(target, &child_ref);
        if !fences.0.contains_key(&child_path) {
            insert_directory_entry(
                target,
                fences,
                &directory_ref,
                name,
                PreparedDossierEntryKind::Directory,
            )?;
            let identity =
                directory_identity(target, &child_path, PreparedDossierDirectoryStatus::Present)?;
            fences.0.insert(
                child_path,
                DirectoryFence {
                    status: PreparedDossierDirectoryStatus::Present,
                    entries: Vec::new(),
                    identity,
                },
            );
        }
        directory_ref = child_ref;
    }
    insert_directory_entry(
        target,
        fences,
        &directory_ref,
        components
            .last()
            .ok_or_else(|| format!("planned output has no leaf: {reference}"))?,
        PreparedDossierEntryKind::RegularFile,
    )
}

fn record_removed_file(
    target: &ResearchExecution,
    fences: &mut DirectoryFences,
    reference: &str,
) -> Result<(), String> {
    let path = Path::new(reference);
    let parent = path
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| format!("invalid removed file path: {reference}"))?;
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("invalid removed file leaf: {reference}"))?;
    let parent = fences
        .0
        .get_mut(&fence_path(target, parent))
        .ok_or_else(|| format!("removed file parent was not source-inventoried: {reference}"))?;
    let before = parent.entries.len();
    parent.entries.retain(|entry| {
        !(entry.name.as_os_str() == OsStr::new(name)
            && entry.kind == PreparedDossierEntryKind::RegularFile)
    });
    if parent.entries.len() + 1 != before {
        return Err(format!(
            "removed file was not uniquely present in directory fence: {reference}"
        ));
    }
    Ok(())
}

fn record_removed_directory(
    target: &ResearchExecution,
    fences: &mut DirectoryFences,
    reference: &str,
) -> Result<(), String> {
    let path = Path::new(reference);
    let parent_ref = path
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| format!("invalid removed directory path: {reference}"))?;
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("invalid removed directory leaf: {reference}"))?;
    let removed = fences
        .0
        .remove(&fence_path(target, reference))
        .ok_or_else(|| format!("removed directory was not source-inventoried: {reference}"))?;
    if removed.status != PreparedDossierDirectoryStatus::Present || !removed.entries.is_empty() {
        return Err(format!(
            "removed directory fence was not empty: {reference}"
        ));
    }
    let parent = fences
        .0
        .get_mut(&fence_path(target, parent_ref))
        .ok_or_else(|| {
            format!("removed directory parent was not source-inventoried: {reference}")
        })?;
    let before = parent.entries.len();
    parent.entries.retain(|entry| {
        !(entry.name.as_os_str() == OsStr::new(name)
            && entry.kind == PreparedDossierEntryKind::Directory)
    });
    if parent.entries.len() + 1 != before {
        return Err(format!(
            "removed directory was not uniquely present in directory fence: {reference}"
        ));
    }
    Ok(())
}

fn load_target_tree(
    target: &ResearchExecution,
    profile: PreparedSourceProfile,
) -> Result<(TargetTree, Vec<PlantingSourcePreimage>), String> {
    let mut tree = TargetTree::default();
    let present = walk_target_tree(target, PHILOSOPHY_TREE, &mut tree)?;
    let mut pins = Vec::new();
    let branch_refs = tree
        .files
        .iter()
        .filter(|reference| {
            reference.ends_with("/branch.manifest.json")
                && !is_under(
                    reference,
                    prepared_dossier_render::OBSOLETE_GENERATED_BRANCH,
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    for reference in branch_refs {
        let (value, pin) = read_json(target, &reference, profile)?;
        pins.push(pin);
        let path = reference
            .strip_suffix("/branch.manifest.json")
            .ok_or("branch manifest suffix missing")?;
        tree.branch_manifests.insert(path.to_owned(), value);
    }
    if !present {
        tree.obsolete_file_stamps = None;
    }
    Ok((tree, pins))
}

fn collect_source_planting_refs(
    tree: &TargetTree,
    assessment: &ReadinessAssessment,
    supported: &[String],
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut result = BTreeMap::new();
    for table_id in supported {
        let package = assessment
            .packages
            .get(table_id)
            .ok_or_else(|| format!("readiness package snapshot missing: {table_id}"))?;
        for dossier_id in &package.route_order {
            let route = package
                .routes
                .get(dossier_id)
                .ok_or_else(|| format!("route order names a missing route: {dossier_id}"))?;
            let branch = route
                .get("branch_path")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("route branch_path missing: {dossier_id}"))?;
            checked_reference(branch)?;
            let planting_root = format!("{branch}/sources/plantings");
            let prefix = format!("{planting_root}/");
            for unsafe_path in &tree.unsafe_paths {
                let could_match = unsafe_path == &planting_root
                    || planting_root.starts_with(&format!("{unsafe_path}/"))
                    || unsafe_path.strip_prefix(&prefix).is_some_and(|tail| {
                        let mut parts = tail.split('/');
                        let Some(directory) = parts.next().filter(|part| !part.is_empty()) else {
                            return false;
                        };
                        match parts.next() {
                            None => !directory.is_empty(),
                            Some("source-planting.json") => parts.next().is_none(),
                            _ => false,
                        }
                    });
                if could_match {
                    return Err(format!(
                        "unsafe source-planting path in routed branch {dossier_id}: {unsafe_path}"
                    ));
                }
            }
            let mut refs = tree
                .paths
                .iter()
                .filter(|reference| {
                    reference.strip_prefix(&prefix).is_some_and(|tail| {
                        let mut parts = tail.split('/');
                        parts.next().is_some_and(|part| !part.is_empty())
                            && parts.next() == Some("source-planting.json")
                            && parts.next().is_none()
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            refs.sort();
            if !refs.is_empty() {
                result.insert(dossier_id.clone(), refs);
            }
        }
    }
    Ok(result)
}

fn package_input(
    source: &ResearchExecution,
    table_id: &str,
    package_value: &Value,
    snapshot: Option<&readiness::PreparedDossierPackageSnapshot>,
    supported: bool,
    profile: PreparedSourceProfile,
) -> Result<(PreparedDossierPackageInput, Vec<PlantingSourcePreimage>), String> {
    let refs = if supported {
        refs_for(table_id, package_value)?
    } else {
        PreparedDossierPackageRefs {
            master_rows: format!("{MASTER_ROOT}/{table_id}/rows.jsonl"),
            master_manifest: format!("{MASTER_ROOT}/{table_id}/table.manifest.json"),
            intake_manifest: package_value
                .get("intake_manifest_ref")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            extraction_coverage: package_value
                .get("extraction_coverage_ref")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            proposed_nodes: format!("{PROPOSED_NODES_ROOT}/{table_id}-prepared-dossiers.jsonl"),
            proposed_relations: format!(
                "{PROPOSED_RELATIONS_ROOT}/{table_id}-prepared-dossiers.jsonl"
            ),
            language_packets: format!(
                "{LANGUAGE_PACKETS_ROOT}/{table_id}-text-bearing-nodes.jsonl"
            ),
            branch_fragments: format!(
                "{BRANCH_FRAGMENTS_ROOT}/{table_id}-prepared-dossier-branches.json"
            ),
            promotion_ledger: format!("{PROMOTION_LEDGER_ROOT}/{table_id}-prepared-dossiers.md"),
        }
    };
    let (master_manifest, pins) = if supported {
        let (value, pin) = read_json(source, &refs.master_manifest, profile)?;
        (value, vec![pin])
    } else {
        (Value::Object(Map::new()), Vec::new())
    };
    let (routes, route_order, blocked, master_rows) = if supported {
        let snapshot =
            snapshot.ok_or_else(|| format!("readiness package snapshot missing: {table_id}"))?;
        if snapshot.table_id != table_id {
            return Err(format!("readiness package identity mismatch: {table_id}"));
        }
        (
            snapshot.routes.clone(),
            snapshot.route_order.clone(),
            snapshot.blocked.clone(),
            snapshot.master_rows.clone(),
        )
    } else {
        (BTreeMap::new(), Vec::new(), BTreeMap::new(), Vec::new())
    };
    Ok((
        PreparedDossierPackageInput {
            table_id: table_id.to_owned(),
            package: package_value.clone(),
            routes,
            route_order,
            blocked,
            master_rows,
            master_manifest,
            refs,
        },
        pins,
    ))
}

fn build_render_inputs(
    source: &ResearchExecution,
    target: &ResearchExecution,
    assessment: &ReadinessAssessment,
    profile: PreparedSourceProfile,
    tree: TargetTree,
    mut source_pins: Vec<PlantingSourcePreimage>,
) -> Result<
    (
        PreparedDossierPlantingInputs,
        Multilingual,
        BTreeSet<String>,
    ),
    String,
> {
    let package_order = assessment.package_order.clone();
    let supported_table_ids = assessment.supported_table_ids.clone();
    let supported = supported_table_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut packages = Vec::new();
    for table_id in &package_order {
        let package_value = assessment
            .package_configs
            .get(table_id)
            .ok_or_else(|| format!("readiness package config missing: {table_id}"))?;
        let (package, pins) = package_input(
            source,
            table_id,
            package_value,
            assessment.packages.get(table_id),
            supported.contains(table_id.as_str()),
            profile,
        )?;
        packages.push(package);
        source_pins.extend(pins);
    }
    let (atlas_manifest, atlas_pin) =
        read_json(source, prepared_dossier_render::ATLAS_MANIFEST_REF, profile)?;
    let (dossier_branch_manifest, dossier_pin) =
        read_json(source, prepared_dossier_render::DOSSIER_BRANCH_REF, profile)?;
    let (philosophy_manifest, philosophy_pin) = read_json(
        source,
        prepared_dossier_render::PHILOSOPHY_MANIFEST_REF,
        profile,
    )?;
    let (labels, label_pin) = read_json(
        source,
        tos_compiler::source_philosophy_multilingual::LABEL_LEDGER,
        profile,
    )?;
    source_pins.extend([atlas_pin, dossier_pin, philosophy_pin, label_pin]);
    let multilingual = Multilingual::from_ledger(&labels).map_err(|error| error.to_string())?;
    let mut reviewed_russian_titles = BTreeMap::new();
    for package in &packages {
        for dossier_id in package.routes.keys() {
            let labels = multilingual
                .label(
                    dossier_id,
                    dossier_id,
                    &json!({"node_type":"prepared-dossier","dossier_id":dossier_id}),
                )
                .map_err(|error| error.to_string())?;
            let reviewed = labels
                .get("label")
                .and_then(|value| value.get("ru"))
                .and_then(Value::as_str)
                .ok_or_else(|| format!("reviewed Russian title missing for {dossier_id}"))?;
            reviewed_russian_titles.insert(dossier_id.clone(), reviewed.to_owned());
        }
    }
    let source_planting_refs =
        collect_source_planting_refs(&tree, assessment, &supported_table_ids)?;
    for (dossier_id, refs) in &source_planting_refs {
        let Some(snapshot) = supported_table_ids
            .iter()
            .filter_map(|table_id| assessment.packages.get(table_id))
            .find(|snapshot| snapshot.routes.contains_key(dossier_id))
        else {
            return Err(format!(
                "source planting route lost its package: {dossier_id}"
            ));
        };
        let route = snapshot.routes.get(dossier_id).ok_or("route disappeared")?;
        let branch = route
            .get("branch_path")
            .and_then(Value::as_str)
            .ok_or("route branch missing")?;
        if refs
            .iter()
            .any(|reference| !reference.starts_with(&format!("{branch}/sources/plantings/")))
        {
            return Err(format!(
                "source planting glob escaped its branch: {dossier_id}"
            ));
        }
    }
    let obsolete_files = tree.obsolete_files.clone();
    let obsolete_dirs = tree.obsolete_directories.clone();
    let obsolete_outputs = tree.obsolete_output_preimages.clone();
    source_pins.extend(obsolete_files.clone().unwrap_or_default());
    source_pins.sort_by(|a, b| a.reference.cmp(&b.reference));
    let mut unique_pins = Vec::new();
    for pin in source_pins {
        if let Some(previous) = unique_pins.last() {
            let previous: &PlantingSourcePreimage = previous;
            if previous.reference == pin.reference {
                if previous != &pin {
                    return Err(format!(
                        "source changed while collecting pins: {}",
                        pin.reference
                    ));
                }
                continue;
            }
        }
        unique_pins.push(pin);
    }
    let target_source_refs = tree
        .branch_manifests
        .keys()
        .map(|path| format!("{path}/branch.manifest.json"))
        .chain(
            obsolete_files
                .iter()
                .flatten()
                .map(|pin| pin.reference.clone()),
        )
        .collect::<BTreeSet<_>>();
    let inputs = PreparedDossierPlantingInputs {
        supported_table_ids,
        packages,
        atlas_manifest,
        dossier_branch_manifest,
        philosophy_manifest,
        branch_manifests: tree.branch_manifests,
        source_planting_refs,
        existing_paths: tree.paths,
        existing_obsolete_branch_files: obsolete_files,
        existing_obsolete_branch_directories: obsolete_dirs,
        existing_obsolete_branch_output_preimages: obsolete_outputs,
        reviewed_russian_titles,
        source_preimages: unique_pins,
    };
    let _ = target;
    Ok((inputs, multilingual, target_source_refs))
}

/// Borrow the exact readiness-parsed document, never inflate the archive again.
/// The retained document's identity binds it to the immutable readiness bytes;
/// the caller charges this verification under the original work ledger.
fn retained_document(artifact: &PreparedDossierArtifactSnapshot) -> Result<&DocxDocument, String> {
    let document = artifact.parsed_docx.as_ref().ok_or_else(|| {
        format!(
            "{}: native readiness parsed document missing",
            artifact.filename
        )
    })?;
    if document.size_bytes != artifact.raw_docx.len() as u64
        || document.sha256 != Digest256::of_bytes(&artifact.raw_docx).to_hex()
    {
        return Err(format!(
            "{}: retained DOCX document/archive identity differs",
            artifact.filename
        ));
    }
    Ok(document)
}

fn extract_all(
    assessment: &ReadinessAssessment,
    supported: &[String],
    source: &ResearchExecution,
) -> Result<Vec<PreparedDossier>, String> {
    let mut dossiers = Vec::new();
    for table_id in supported {
        source.check()?;
        let package = assessment
            .packages
            .get(table_id)
            .ok_or_else(|| format!("readiness package snapshot missing: {table_id}"))?;
        for artifact in &package.artifacts {
            source.check()?;
            source.tick(artifact.raw_docx.len() as u64)?;
            let document = retained_document(artifact)?;
            let mut work = |units| source.tick(units);
            let filename = Path::new(&artifact.filename)
                .file_name()
                .and_then(OsStr::to_str)
                .ok_or_else(|| format!("DOCX filename is not UTF-8: {}", artifact.filename))?;
            let extracted = extract_dossier(
                document,
                &artifact.table_id,
                &artifact.dossier_id,
                filename,
                &artifact.section,
                &artifact.master_row,
                artifact.route.as_ref(),
                artifact.blocked.as_ref(),
                &package.package,
                &mut work,
            )
            .map_err(|issues| {
                issues
                    .into_iter()
                    .map(|issue| format!("{}: {}", issue.code, issue.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            })?;
            dossiers.push(extracted);
        }
    }
    Ok(dossiers)
}

fn verify_source_pin(
    root: &ResearchExecution,
    reference: &str,
    pin: &PlantingSourcePreimage,
) -> Result<FileStamp, String> {
    root.check()?;
    let mut file = root.source_file(reference, FILE_CAP)?;
    let before = file.metadata().map_err(|error| error.to_string())?;
    let stamp = file_stamp(&before);
    if stamp.size_bytes != pin.size_bytes {
        return Err(format!(
            "source size changed before planting: {}",
            pin.reference
        ));
    }
    root.tick(pin.size_bytes)?;
    let sha256 = root.hash_file(&mut file, FILE_CAP)?;
    root.verify_file_unchanged(&file, &before)?;
    let after = file.metadata().map_err(|error| error.to_string())?;
    if file_stamp(&after) != stamp {
        return Err(format!(
            "source stamp changed while verifying: {}",
            pin.reference
        ));
    }
    if sha256 != pin.sha256 {
        return Err(format!(
            "source fixity changed before planting: {}",
            pin.reference
        ));
    }
    verify_source_stamp(root, reference, pin, stamp)?;
    Ok(stamp)
}

fn verify_source_stamp(
    root: &ResearchExecution,
    reference: &str,
    pin: &PlantingSourcePreimage,
    expected: FileStamp,
) -> Result<(), String> {
    root.check()?;
    let file = root.source_file(reference, FILE_CAP)?;
    let before = file.metadata().map_err(|error| error.to_string())?;
    if file_stamp(&before) != expected || expected.size_bytes != pin.size_bytes {
        return Err(format!(
            "source stamp changed before planting: {}",
            pin.reference
        ));
    }
    root.tick(1)?;
    root.verify_file_unchanged(&file, &before)?;
    let after = file.metadata().map_err(|error| error.to_string())?;
    let Some((parent, leaf)) = parent_and_leaf(root, reference)? else {
        return Err(format!(
            "source path disappeared before planting: {}",
            pin.reference
        ));
    };
    if file_stamp(&after) != expected
        || stat_at(&parent, &leaf)?.as_ref().map(stat_stamp) != Some(expected)
    {
        return Err(format!(
            "source path stamp changed before planting: {}",
            pin.reference
        ));
    }
    root.check()
}

fn source_location<'a>(
    source: &'a ResearchExecution,
    docs: &'a ResearchExecution,
    doc_root: &Path,
    target: &'a ResearchExecution,
    target_source_refs: &BTreeSet<String>,
    reference: &str,
) -> Result<(&'a ResearchExecution, String), String> {
    if target_source_refs.contains(reference) {
        return Ok((target, reference.to_owned()));
    }
    if Path::new(reference).extension().and_then(OsStr::to_str) == Some("docx") {
        if docs.root() == doc_root {
            return Ok((docs, reference.to_owned()));
        }
        let doc_prefix = doc_root
            .strip_prefix("/")
            .map_err(|_| "DOCX root is not absolute")?;
        let relative = checked_reference(reference)?;
        let anchored = doc_prefix
            .join(relative)
            .to_string_lossy()
            .replace('\\', "/");
        return Ok((docs, anchored));
    }
    Ok((source, reference.to_owned()))
}

fn verify_source_pins_once(
    source: &ResearchExecution,
    docs: &ResearchExecution,
    doc_root: &Path,
    target: &ResearchExecution,
    target_source_refs: &BTreeSet<String>,
    pins: &[PlantingSourcePreimage],
    already_hashed_stamps: &BTreeMap<String, FileStamp>,
) -> Result<BTreeMap<String, VerifiedSourcePin>, String> {
    let mut verified = BTreeMap::new();
    for pin in pins {
        let (root, reference) = source_location(
            source,
            docs,
            doc_root,
            target,
            target_source_refs,
            &pin.reference,
        )?;
        let stamp = match already_hashed_stamps.get(&pin.reference) {
            Some(stamp) => {
                if stamp.size_bytes != pin.size_bytes {
                    return Err(format!(
                        "pre-hashed source stamp size differs: {}",
                        pin.reference
                    ));
                }
                verify_source_stamp(root, &reference, pin, *stamp)?;
                *stamp
            }
            None => verify_source_pin(root, &reference, pin)?,
        };
        verified.insert(
            pin.reference.clone(),
            VerifiedSourcePin {
                preimage: pin.clone(),
                stamp,
            },
        );
    }
    if already_hashed_stamps
        .keys()
        .any(|reference| !verified.contains_key(reference))
    {
        return Err("pre-hashed source stamp lacks a source preimage".into());
    }
    Ok(verified)
}

fn verify_source_pins_current(
    source: &ResearchExecution,
    docs: &ResearchExecution,
    doc_root: &Path,
    target: &ResearchExecution,
    target_source_refs: &BTreeSet<String>,
    pins: &BTreeMap<String, VerifiedSourcePin>,
) -> Result<(), String> {
    for pin in pins.values() {
        let (root, reference) = source_location(
            source,
            docs,
            doc_root,
            target,
            target_source_refs,
            &pin.preimage.reference,
        )?;
        verify_source_stamp(root, &reference, &pin.preimage, pin.stamp)?;
    }
    Ok(())
}

fn verify_output_preimage(
    output: &crate::prepared_dossier_render::PreparedDossierOutput,
    preimages: &[PlantingOutputPreimage],
    target: &ResearchExecution,
) -> Result<(), String> {
    let expected = preimages
        .iter()
        .find(|preimage| preimage.reference == output.reference)
        .ok_or_else(|| format!("output preimage missing: {}", output.reference))?;
    verify_output_stamp(target, &output.reference, &expected.state)?;
    Ok(())
}

fn verify_output_stamp(
    target: &ResearchExecution,
    reference: &str,
    expected: &PlantingOutputLeafState,
) -> Result<(), String> {
    target.check()?;
    let Some((parent, leaf)) = parent_and_leaf(target, reference)? else {
        return if expected == &PlantingOutputLeafState::Absent {
            Ok(())
        } else {
            Err(format!("planned output parent disappeared: {reference}"))
        };
    };
    let Some(path_stat) = stat_at(&parent, &leaf)? else {
        return if expected == &PlantingOutputLeafState::Absent {
            Ok(())
        } else {
            Err(format!("planned output disappeared: {reference}"))
        };
    };
    let Some(expected_stamp) = output_state_stamp(expected) else {
        return Err(format!(
            "planned output appeared after preflight: {reference}"
        ));
    };
    if path_stat.st_mode & libc::S_IFMT != libc::S_IFREG || stat_stamp(&path_stat) != expected_stamp
    {
        return Err(format!(
            "planned output changed after preflight: {reference}"
        ));
    }
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(format!(
            "open planned output {reference}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let before = file.metadata().map_err(|error| error.to_string())?;
    if !before.is_file() || file_stamp(&before) != expected_stamp {
        return Err(format!("planned output identity changed: {reference}"));
    }
    target.tick(1)?;
    target.verify_file_unchanged(&file, &before)?;
    if stat_at(&parent, &leaf)?.as_ref().map(stat_stamp) != Some(expected_stamp) {
        return Err(format!("planned output path changed: {reference}"));
    }
    target.check()
}

fn capture_written_stamp(
    target: &ResearchExecution,
    reference: &str,
    expected_size: u64,
    expected_mode: u32,
    expected_sha256: &str,
) -> Result<FileStamp, String> {
    target.check()?;
    let Some((parent, leaf)) = parent_and_leaf(target, reference)? else {
        return Err(format!("written output parent disappeared: {reference}"));
    };
    try_lock_parent(&parent).map_err(|error| format!("written output parent busy: {error}"))?;
    let before_path = stat_at(&parent, &leaf)?
        .ok_or_else(|| format!("written output disappeared: {reference}"))?;
    if before_path.st_mode & libc::S_IFMT != libc::S_IFREG
        || before_path.st_size as u64 != expected_size
        || before_path.st_mode as u32 & 0o7777 != expected_mode
    {
        return Err(format!("written output identity mismatch: {reference}"));
    }
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(format!(
            "open written output {reference}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let before = file.metadata().map_err(|error| error.to_string())?;
    let stamp = file_stamp(&before);
    if !before.is_file()
        || stamp.size_bytes != expected_size
        || stamp.mode & 0o7777 != expected_mode
        || stat_stamp(&before_path) != stamp
    {
        return Err(format!("written output path changed: {reference}"));
    }
    target.tick(expected_size)?;
    let actual_sha256 = target.hash_file(&mut file, expected_size)?;
    if actual_sha256 != expected_sha256 {
        return Err(format!(
            "written output content differs from plan: {reference}"
        ));
    }
    target.tick(1)?;
    target.verify_file_unchanged(&file, &before)?;
    if stat_at(&parent, &leaf)?.as_ref().map(stat_stamp) != Some(stamp) {
        return Err(format!(
            "written output changed after publication: {reference}"
        ));
    }
    target.check()?;
    Ok(stamp)
}

fn matches_directory(file: &File, expected: &PlantingDirectoryPreimage) -> Result<bool, String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    Ok(metadata.is_dir()
        && metadata.dev() == expected.device
        && metadata.ino() == expected.inode
        && metadata.mode() == expected.mode
        && metadata.mtime() == expected.mtime
        && metadata.mtime_nsec() == expected.mtime_nsec
        && metadata.ctime() == expected.ctime
        && metadata.ctime_nsec() == expected.ctime_nsec)
}

fn matches_directory_identity(
    file: &File,
    expected: &PlantingDirectoryPreimage,
) -> Result<bool, String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    Ok(metadata.is_dir()
        && metadata.dev() == expected.device
        && metadata.ino() == expected.inode
        && metadata.mode() == expected.mode)
}

fn unlink_checked_file(
    target: &ResearchExecution,
    fences: &mut DirectoryFences,
    reference: &str,
    pin: &VerifiedSourcePin,
    output_pin: &PlantingOutputPreimage,
) -> Result<(), String> {
    target.check()?;
    let Some((parent, leaf)) = parent_and_leaf(target, reference)? else {
        return Err(format!("obsolete generated leaf disappeared: {reference}"));
    };
    try_lock_parent(&parent).map_err(|error| format!("cleanup parent busy: {error}"))?;
    let expected = present_output_state(pin.preimage.sha256.clone(), pin.stamp);
    if pin.preimage.reference != reference
        || output_pin.reference != reference
        || output_pin.state != expected
    {
        return Err(format!("obsolete generated leaf changed: {reference}"));
    }
    let stat = stat_at(&parent, &leaf)?
        .ok_or_else(|| format!("obsolete generated leaf disappeared: {reference}"))?;
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG || stat_stamp(&stat) != pin.stamp {
        return Err(format!(
            "obsolete generated leaf identity changed: {reference}"
        ));
    }
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(format!(
            "open obsolete generated leaf {reference}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let before = file.metadata().map_err(|error| error.to_string())?;
    if !before.is_file() || file_stamp(&before) != pin.stamp {
        return Err(format!("obsolete generated leaf changed: {reference}"));
    }
    target.tick(1)?;
    target.verify_file_unchanged(&file, &before)?;
    if stat_at(&parent, &leaf)?.as_ref().map(stat_stamp) != Some(pin.stamp) {
        return Err(format!("obsolete generated leaf path changed: {reference}"));
    }
    target.check()?;
    if unsafe { libc::unlinkat(parent.as_raw_fd(), leaf.as_ptr(), 0) } < 0 {
        return Err(format!(
            "remove obsolete generated leaf {reference}: {}",
            std::io::Error::last_os_error()
        ));
    }
    record_removed_file(target, fences, reference)?;
    target.check()?;
    Ok(())
}

fn remove_obsolete_branch(
    target: &ResearchExecution,
    fences: &mut DirectoryFences,
    intent: &ObsoleteBranchIntent,
    source_pins: &BTreeMap<String, VerifiedSourcePin>,
) -> Result<(), String> {
    if intent.root != prepared_dossier_render::OBSOLETE_GENERATED_BRANCH {
        return Err("refusing an unrecognized obsolete branch cleanup root".into());
    }
    let intent_file_pins = intent
        .file_preimages
        .iter()
        .map(|pin| (pin.reference.as_str(), pin))
        .collect::<BTreeMap<_, _>>();
    let output_pins = intent
        .output_preimages
        .iter()
        .map(|pin| (pin.reference.as_str(), pin))
        .collect::<BTreeMap<_, _>>();
    let file_refs = intent
        .files
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let file_pin_refs = intent_file_pins.keys().copied().collect::<BTreeSet<_>>();
    let output_pin_refs = output_pins.keys().copied().collect::<BTreeSet<_>>();
    if file_refs.len() != intent.files.len()
        || file_pin_refs != file_refs
        || output_pin_refs != file_refs
    {
        return Err("obsolete branch file pins do not cover the exact cleanup file set".into());
    }
    let directory_refs = intent
        .directories
        .iter()
        .map(|item| item.reference.as_str())
        .collect::<BTreeSet<_>>();
    let observed_directory_refs = fences
        .0
        .keys()
        .filter_map(|path| path.strip_prefix(target.root()).ok())
        .filter_map(Path::to_str)
        .filter(|path| is_under(path, &intent.root))
        .collect::<BTreeSet<_>>();
    if directory_refs.len() != intent.directories.len() || directory_refs != observed_directory_refs
    {
        return Err(
            "obsolete branch directory identities do not cover the exact cleanup tree".into(),
        );
    }
    for expected in &intent.directories {
        let directory =
            descriptor_open_directory(target, &expected.reference)?.ok_or_else(|| {
                format!(
                    "obsolete generated directory disappeared: {}",
                    expected.reference
                )
            })?;
        if !matches_directory(&directory, expected)? {
            return Err(format!(
                "obsolete generated directory changed before cleanup: {}",
                expected.reference
            ));
        }
    }
    for reference in &intent.files {
        target.check()?;
        verify_directory_fences(target, fences)?;
        let pin = intent_file_pins
            .get(reference.as_str())
            .ok_or("cleanup content preimage missing")?;
        let output_pin = output_pins
            .get(reference.as_str())
            .ok_or("cleanup output identity missing")?;
        if Path::new(reference).file_name().and_then(OsStr::to_str) != Some("branch.manifest.json")
            || !is_under(reference, &intent.root)
        {
            return Err(format!(
                "refusing non-generated obsolete branch leaf: {reference}"
            ));
        }
        let source_pin = source_pins
            .get(reference)
            .ok_or("cleanup source stamp missing")?;
        if source_pin.preimage != **pin {
            return Err(format!("cleanup source preimage changed: {reference}"));
        }
        unlink_checked_file(target, fences, reference, source_pin, output_pin)?;
    }
    let mut directories = intent.directories.clone();
    directories.sort_by(|a, b| {
        b.reference
            .matches('/')
            .count()
            .cmp(&a.reference.matches('/').count())
            .then_with(|| b.reference.cmp(&a.reference))
    });
    if directories.iter().all(|item| item.reference != intent.root) {
        return Err("obsolete branch root directory identity is missing".into());
    }
    for expected in directories {
        target.check()?;
        verify_directory_fences(target, fences)?;
        if !is_under(&expected.reference, &intent.root) {
            return Err(format!(
                "cleanup directory escaped obsolete branch: {}",
                expected.reference
            ));
        }
        let Some((parent, leaf)) = parent_and_leaf(target, &expected.reference)? else {
            return Err(format!(
                "obsolete generated directory disappeared: {}",
                expected.reference
            ));
        };
        try_lock_parent(&parent).map_err(|error| format!("cleanup parent busy: {error}"))?;
        let directory =
            descriptor_open_directory(target, &expected.reference)?.ok_or_else(|| {
                format!(
                    "obsolete generated directory disappeared: {}",
                    expected.reference
                )
            })?;
        if !matches_directory_identity(&directory, &expected)? {
            return Err(format!(
                "obsolete generated directory identity changed: {}",
                expected.reference
            ));
        }
        let stat = stat_at(&parent, &leaf)?.ok_or_else(|| {
            format!(
                "obsolete generated directory disappeared: {}",
                expected.reference
            )
        })?;
        if stat.st_mode & libc::S_IFMT != libc::S_IFDIR
            || stat.st_dev as u64 != expected.device
            || stat.st_ino as u64 != expected.inode
        {
            return Err(format!(
                "obsolete generated directory path changed: {}",
                expected.reference
            ));
        }
        target.tick(1)?;
        if unsafe { libc::unlinkat(parent.as_raw_fd(), leaf.as_ptr(), libc::AT_REMOVEDIR) } < 0 {
            return Err(format!(
                "remove obsolete generated directory {}: {}",
                expected.reference,
                std::io::Error::last_os_error()
            ));
        }
        record_removed_directory(target, fences, &expected.reference)?;
        target.check()?;
    }
    if descriptor_open_directory(target, &intent.root)?.is_some() {
        return Err("obsolete generated branch remains after guarded cleanup".into());
    }
    Ok(())
}

fn prepare_plan(
    source: &ResearchExecution,
    target: &ResearchExecution,
    assessment: &ReadinessAssessment,
    profile: PreparedSourceProfile,
    tree: TargetTree,
    source_pins: Vec<PlantingSourcePreimage>,
) -> Result<
    (
        PreparedDossierPlantingInputs,
        Multilingual,
        BTreeSet<String>,
    ),
    String,
> {
    build_render_inputs(source, target, assessment, profile, tree, source_pins)
}

/// Native callable for the prepared CLI branch. `operation` was created by
/// the parent entry with its original deadline/cancellation/scratch envelope;
/// selected roots share all of those same ledgers.
pub fn run_native(
    inputs: NativePreparedDossierInputs,
    operation: &ResearchExecution,
) -> Result<Value, String> {
    if operation.root() != inputs.repository_root {
        return Err("prepared-dossier source root differs from the selected operation root".into());
    }
    let action = readiness::select_action(
        inputs.readiness,
        inputs.plant,
        inputs.selected_table_id.clone(),
    )?;
    if matches!(&action, readiness::PreparedDossierAction::PlantAggregate)
        && inputs.output_root != inputs.repository_root
    {
        return Err("prepared-dossier planting output root must equal its source root".into());
    }
    operation.check()?;
    let doc_root_probe = tos_fd_open::open_absolute_directory(&inputs.doc_root);
    operation.check()?;
    let docs = match doc_root_probe {
        Ok(probe) => {
            let selected = operation.select_directory(&inputs.doc_root)?;
            let metadata = probe.metadata().map_err(|error| error.to_string())?;
            if selected.root_identity()? != (metadata.dev(), metadata.ino()) {
                return Err("selected DOCX root changed during secure selection".into());
            }
            selected
        }
        Err(error)
            if error
                .source
                .as_ref()
                .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) =>
        {
            operation.select_directory(Path::new("/"))?
        }
        Err(error) => return Err(error.to_string()),
    };
    let readiness_inputs = ReadinessInputs {
        repository_root: inputs.repository_root.clone(),
        doc_root: inputs.doc_root.clone(),
        source_profile: inputs.source_profile,
        selected_table_id: inputs.selected_table_id.clone(),
    };
    let mut readiness_execution = ReadinessExecution {
        source: operation,
        docs: &docs,
        doc_root: &inputs.doc_root,
        doc_fences: DirectoryFences::default(),
    };
    match action {
        readiness::PreparedDossierAction::Readiness { .. } => {
            let payload = readiness::readiness_payload(
                &readiness_inputs,
                &NativePreparedDossierContentValidator,
                &mut readiness_execution,
            )?;
            return Ok(payload);
        }
        readiness::PreparedDossierAction::PlantAggregate => {}
    }
    let assessment = readiness::assess_readiness(
        &readiness_inputs,
        &NativePreparedDossierContentValidator,
        &mut readiness_execution,
    )?;
    readiness::require_aggregate_readiness(&assessment.payload)?;
    if assessment
        .payload
        .get("ready_to_plant")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("aggregate readiness did not establish the planting gate".into());
    }
    let target = operation.select_output_directory(&inputs.output_root, false)?;
    if target.root_identity()? != operation.root_identity()? {
        return Err("prepared-dossier output directory differs from selected source root".into());
    }
    let supported = assessment.supported_table_ids.clone();
    let mut source_pins = assessment
        .input_pins
        .iter()
        .map(|pin| PlantingSourcePreimage {
            reference: pin.reference.clone(),
            sha256: pin.sha256.clone(),
            size_bytes: pin.size_bytes,
        })
        .collect::<Vec<_>>();
    let research_output_refs = configured_research_dossier_outputs(&assessment)?;
    let (tree, target_pins) = load_target_tree(&target, inputs.source_profile)?;
    let prehashed_obsolete_stamps = tree.obsolete_file_stamps.clone().unwrap_or_default();
    let mut target_fences = tree.directories.clone();
    record_research_dossier_parent_fences(&target, &research_output_refs, &mut target_fences)?;
    source_pins.extend(target_pins);
    readiness_execution.verify_doc_fences()?;
    verify_directory_fences(&target, &target_fences)?;
    let (planting_inputs, multilingual, target_pin_refs) = prepare_plan(
        operation,
        &target,
        &assessment,
        inputs.source_profile,
        tree,
        source_pins,
    )?;
    let dossiers = extract_all(&assessment, &supported, operation)?;
    let mut check = |units| operation.tick(units);
    let mut plan = prepared_dossier_render::prepare_planting_plan(
        &planting_inputs,
        &dossiers,
        &multilingual,
        &mut check,
    )?;
    let planned_research_outputs = plan
        .outputs
        .iter()
        .filter(|output| research_output_refs.contains(&output.reference))
        .map(|output| output.reference.clone())
        .collect::<Vec<_>>();
    if plan.outputs.iter().any(|output| {
        !output.reference.starts_with(&format!("{PHILOSOPHY_TREE}/"))
            && !research_output_refs.contains(&output.reference)
    }) || planned_research_outputs.len() != research_output_refs.len()
        || planned_research_outputs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != research_output_refs
    {
        return Err("prepared-dossier output escaped the philosophy tree and exact configured research dossier leaves".into());
    }
    let mut source_pin_inputs = BTreeMap::new();
    for pin in &plan.source_preimages {
        if let Some(previous) = source_pin_inputs.insert(pin.reference.clone(), pin.clone())
            && previous != *pin
        {
            return Err(format!("conflicting source preimages: {}", pin.reference));
        }
    }
    let source_pin_inputs = source_pin_inputs.into_values().collect::<Vec<_>>();
    let mut source_pins = verify_source_pins_once(
        operation,
        &docs,
        &inputs.doc_root,
        &target,
        &target_pin_refs,
        &source_pin_inputs,
        &prehashed_obsolete_stamps,
    )?;
    let mut output_preimages = Vec::new();
    for output in &plan.outputs {
        operation.check()?;
        let state = match source_pins.get(&output.reference) {
            Some(pin) => present_output_state(pin.preimage.sha256.clone(), pin.stamp),
            None => output_state(&target, &output.reference)?,
        };
        output_preimages.push(PlantingOutputPreimage {
            reference: output.reference.clone(),
            state,
        });
    }
    plan.bind_output_preimages(output_preimages)?;
    readiness_execution.verify_doc_fences()?;
    verify_directory_fences(&target, &target_fences)?;
    let preimages = plan
        .output_preimages
        .as_ref()
        .ok_or("output preimages were not bound")?;
    match (
        plan.obsolete_generated_branch.as_ref(),
        plan.obsolete_generated_branch_before_output_index,
    ) {
        (Some(_), Some(index)) if index < plan.outputs.len() => {}
        (None, None) => {}
        (Some(_), _) => {
            return Err("cleanup intent lacks a valid source-ordered branch phase".into());
        }
        (None, Some(_)) => return Err("cleanup phase index exists without cleanup intent".into()),
    }
    operation.check()?;
    for (index, output) in plan.outputs.iter().enumerate() {
        operation.check()?;
        readiness_execution.verify_doc_fences()?;
        verify_directory_fences(&target, &target_fences)?;
        verify_source_pins_current(
            operation,
            &docs,
            &inputs.doc_root,
            &target,
            &target_pin_refs,
            &source_pins,
        )?;
        if plan.obsolete_generated_branch_before_output_index == Some(index) {
            let intent = plan
                .obsolete_generated_branch
                .as_ref()
                .ok_or("cleanup phase index exists without cleanup intent")?;
            remove_obsolete_branch(&target, &mut target_fences, intent, &source_pins)?;
            for reference in &intent.files {
                source_pins.remove(reference);
            }
            verify_directory_fences(&target, &target_fences)?;
        }
        verify_output_preimage(output, preimages, &target)?;
        operation.check()?;
        let expected_output = preimages
            .iter()
            .find(|preimage| preimage.reference == output.reference)
            .ok_or_else(|| format!("output preimage missing: {}", output.reference))?;
        let was_absent = expected_output.state == PlantingOutputLeafState::Absent;
        let output_mode = match &expected_output.state {
            PlantingOutputLeafState::Absent => OUTPUT_MODE,
            PlantingOutputLeafState::Present { mode, .. } => *mode & 0o7777,
        };
        let replacement_pin = if source_pins.contains_key(&output.reference) {
            operation.tick(output.bytes.len() as u64)?;
            Some(PlantingSourcePreimage {
                reference: output.reference.clone(),
                sha256: Digest256::of_bytes(&output.bytes).to_hex(),
                size_bytes: output.bytes.len() as u64,
            })
        } else {
            None
        };
        target
            .write(&output.reference, &output.bytes, output_mode, was_absent)
            .map_err(|error| format!("write {}: {error}", output.reference))?;
        record_written_output(
            &target,
            &mut target_fences,
            &output.reference,
            was_absent,
            &research_output_refs,
        )?;
        if let Some(pin) = replacement_pin {
            let stamp = capture_written_stamp(
                &target,
                &output.reference,
                output.bytes.len() as u64,
                output_mode,
                &pin.sha256,
            )?;
            source_pins.insert(
                output.reference.clone(),
                VerifiedSourcePin {
                    preimage: pin,
                    stamp,
                },
            );
        }
        operation.check()?;
    }
    readiness_execution.verify_doc_fences()?;
    verify_directory_fences(&target, &target_fences)?;
    let (admitted, _) = tos_compiler::source_philosophy_dossier_extract::admissions(&dossiers);
    Ok(json!({
        "schema_version":"tos_prepared_dossier_planting_result_v1",
        "owner_repo":"Tree-of-Sophia",
        "planting_scope":"all_supported_packages",
        "supported_table_ids":supported,
        "artifact_count":dossiers.len(),
        "admitted_count":admitted.len(),
        "output_refs":plan.outputs.iter().map(|output| output.reference.clone()).collect::<Vec<_>>(),
        "is_semantic_admission":false,
        "is_rights_admission":false,
        "is_review_admission":false,
        "is_canon_admission":false,
        "sequential_publication":"guarded per-file source-order writes; no cross-file atomicity"
    }))
}

#[cfg(test)]
mod retained_document_tests {
    use super::*;

    #[test]
    fn reuses_the_validated_document_and_rejects_changed_archive_bytes() {
        let raw = b"retained archive".to_vec();
        let document = DocxDocument {
            size_bytes: raw.len() as u64,
            sha256: Digest256::of_bytes(&raw).to_hex(),
            ..DocxDocument::default()
        };
        let mut artifact = PreparedDossierArtifactSnapshot {
            table_id: "table-i".into(),
            dossier_id: "A01".into(),
            section: "1.1".into(),
            filename: "A01.docx".into(),
            raw_docx: raw,
            parsed_docx: Some(document),
            master_row: Value::Null,
            route: None,
            blocked: None,
        };
        assert!(std::ptr::eq(
            retained_document(&artifact).unwrap(),
            artifact.parsed_docx.as_ref().unwrap()
        ));
        artifact.raw_docx[0] ^= 1;
        assert!(
            retained_document(&artifact)
                .unwrap_err()
                .contains("identity differs")
        );
        artifact.parsed_docx = None;
        assert!(
            retained_document(&artifact)
                .unwrap_err()
                .contains("parsed document missing")
        );
    }
}

#[cfg(test)]
mod retained_language_packet_tests {
    use super::*;

    fn rows(root: &Path, relative: &str) -> Vec<Value> {
        std::fs::read_to_string(root.join(relative))
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn table_one_and_two_language_packets_cover_selected_text_corpora() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for (table, count) in [("i", 347usize), ("ii", 388usize)] {
            let nodes = rows(
                &root,
                &format!(
                    "ToS/philosophy/graph-workbench/proposed-nodes/table-{table}-prepared-dossiers.jsonl"
                ),
            );
            let packets = rows(
                &root,
                &format!(
                    "ToS/philosophy/graph-workbench/language-packets/table-{table}-text-bearing-nodes.jsonl"
                ),
            );
            let source_by_id: BTreeMap<String, &Value> = nodes
                .iter()
                .filter_map(|row| row["candidate_id"].as_str().map(|id| (id.to_owned(), row)))
                .collect();
            let source_ids: BTreeSet<String> = nodes
                .iter()
                .filter(|row| row["node_kind"] == "text_corpus")
                .filter_map(|row| row["candidate_id"].as_str().map(str::to_owned))
                .collect();
            let packet_ids: BTreeSet<String> = packets
                .iter()
                .filter_map(|row| row["node_ref"]["id"].as_str().map(str::to_owned))
                .collect();
            assert_eq!(packets.len(), count);
            assert_eq!(packet_ids, source_ids);
            assert!(packets.iter().all(|row| {
                let title = &row["title_block"];
                row["schema_version"] == "tos_philosophy_text_bearing_language_packet_v1"
                    && row["node_ref"]["id_kind"] == "candidate_id"
                    && row["language_registry_ref"]
                        == "ToS/philosophy/atlas/multilingual/language-registry.json"
                    && row["text_bearing_nodes_contract_ref"]
                        == "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json"
                    && title.as_object().is_some_and(|value| {
                        value.len() == 3
                            && value.contains_key("original")
                            && value.contains_key("ru")
                            && value.contains_key("en")
                    })
                    && ["source", "reviewed", "draft", "pending"]
                        .contains(&title["ru"]["translation_status"].as_str().unwrap_or(""))
                    && ["source", "reviewed", "draft", "pending"]
                        .contains(&title["en"]["translation_status"].as_str().unwrap_or(""))
                    && row["relation_pressure"]
                        .as_array()
                        .is_some_and(|relations| {
                            relations.iter().all(|r| r["target_status"] == "unresolved")
                        })
            }));
            let first_title = &packets.first().expect("nonempty language packets")["title_block"];
            assert_eq!(first_title["original"]["attestation_status"], "unknown");
            assert_eq!(
                first_title["original"]["review_status"],
                "pending_original_witness"
            );
            let predicates: BTreeSet<String> = packets[0]["relation_pressure"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|row| row["predicate"].as_str().map(str::to_owned))
                .collect();
            assert_eq!(
                predicates,
                BTreeSet::from([
                    "has_original_language".into(),
                    "uses_script".into(),
                    "has_witness".into()
                ])
            );
            assert!(packets.iter().all(|row| {
                row["title_block"]["en"]["value"]
                    .as_str()
                    .is_some_and(|value| {
                        !value
                            .chars()
                            .any(|c| ('\u{0400}'..='\u{04ff}').contains(&c))
                    })
            }));
            if table == "ii" {
                let manual: Vec<&Value> = packets
                    .iter()
                    .filter(|row| row["review_posture"] == "manual_review_required")
                    .collect();
                assert_eq!(manual.len(), 173);
                for packet in manual {
                    let source = source_by_id
                        .get(packet["node_ref"]["id"].as_str().unwrap())
                        .unwrap();
                    for field in [
                        "review_posture",
                        "review_reason",
                        "master_status",
                        "master_confidence",
                    ] {
                        assert_eq!(packet[field], source[field]);
                    }
                }
            }
        }
    }
}
