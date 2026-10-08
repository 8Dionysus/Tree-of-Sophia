//! Bounded producer inputs and writer for the existing ManagedLocal native
//! data-manifest law.
//!
//! This emits mechanics and provenance only. It does not admit source, rights,
//! canon, semantics, publication, or a current release. The host still owns
//! semantic admission and the outer private pair CAS.
use crate::{COMPILER_VERSION, Error, NativeKnowledgeSelection, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath,
    canonical_bytes_v1, parse_json,
};

pub const NATIVE_DATA_SCHEMA: &str = "tos_access_native_data_snapshot_v1";
pub const NATIVE_SELECTION_PATH: &str = "data/native-selection.json";
pub const NATIVE_MODEL_PATH: &str = "data/native-knowledge.sqlite3";
pub const RUNTIME_DATA_DECLARATION_PATH: &str = "access/contracts/runtime-data.v1.json";
pub const RUNTIME_DATA_DECLARATION: &[u8] =
    include_bytes!("../../../../access/contracts/runtime-data.v1.json");
pub const CORPUS_INDEX_PATH: &str = "ToS/derived-exports/tos_corpus_index.min.json";
pub const PHILOSOPHY_GRAPH_PATH: &str = "ToS/derived-exports/philosophy_graph_projection.min.json";
pub const CLAIM_GRAPH_PATH: &str =
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json";
pub const EVIDENCE_PROJECTION_PATH: &str =
    "ToS/derived-exports/epistemic_evidence_projection.min.json";
/// Bind this reviewed source separately from the data bundle: it is not a
/// member of the admitted runtime-data closure and must not be copied in.
pub const EVIDENCE_SCENES_PATH: &str =
    "ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json";
pub const EVIDENCE_SCENES_SOURCE: &[u8] =
    include_bytes!("../../../../ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json");

const EXECUTING_IMAGE_LOGICAL_PATH: &str = "tos-command/current-executable";
// These exact producer code and producer-config inputs are embedded into the
// running image. The held executing ELF hash separately fingerprints the
// complete built image; this selected set is intentionally incomplete and is
// not a reproducible-source or full dependency-closure claim.
const EMBEDDED_PRODUCER_INPUTS: [(&str, &[u8]); 12] = [
    (
        "rust/crates/tos-compiler/src/d1_public_capture.rs",
        include_bytes!("d1_public_capture.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/d1_public_graph.rs",
        include_bytes!("d1_public_graph.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/d1_public_header.rs",
        include_bytes!("d1_public_header.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/d1_public_semantics.rs",
        include_bytes!("d1_public_semantics.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/knowledge_corpus_source.rs",
        include_bytes!("knowledge_corpus_source.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/native_cold_resources.rs",
        include_bytes!("native_cold_resources.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/native_snapshot.rs",
        include_bytes!("native_snapshot.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/native_snapshot_originals.rs",
        include_bytes!("native_snapshot_originals.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/native_snapshot_manifest.rs",
        include_bytes!("native_snapshot_manifest.rs"),
    ),
    (
        "rust/crates/tos-command/src/bin/tos-native-owner-command.rs",
        include_bytes!("../../tos-command/src/bin/tos-native-owner-command.rs"),
    ),
    (
        "rust/crates/tos-command/src/managed_native_original_cli.rs",
        include_bytes!("../../tos-command/src/managed_native_original_cli.rs"),
    ),
    (
        "ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json",
        EVIDENCE_SCENES_SOURCE,
    ),
];
const MAX_MANIFEST_BYTES: usize = 1_048_576;
const MAX_PRODUCER_FILES: usize = 4096;
const MAX_PRODUCER_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PRODUCER_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_EXECUTING_IMAGE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PRODUCER_TOTAL_BYTES: u64 = MAX_PRODUCER_SOURCE_BYTES + MAX_EXECUTING_IMAGE_BYTES;
const MAX_DATA_MEMBERS: usize = 4090;
const MAX_DATA_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CAPTURE_MEMBER_BYTES: usize = 64 * 1024 * 1024;
const MAX_CAPTURE_CLOSURE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CAPTURE_STAGING_BYTES: u64 = 512 * 1024 * 1024;
const MAX_NATIVE_MODEL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_NATIVE_STAGE_TEMP_BYTES: u64 = 512 * 1024 * 1024;
const MIN_STAGE_QUOTA_BYTES: u64 = 4 * MAX_CAPTURE_STAGING_BYTES + 64 * 1024 * 1024;
pub const NATIVE_PRODUCER_MIN_TMPFS_QUOTA_BYTES: u64 = MIN_STAGE_QUOTA_BYTES;
pub const NATIVE_PRODUCER_MEMBER_READ_CAP_BYTES: usize = MAX_CAPTURE_MEMBER_BYTES;
pub const NATIVE_PRODUCER_MAX_DATA_BYTES: u64 = MAX_DATA_BYTES;
pub const NATIVE_PRODUCER_MAX_MODEL_BYTES: u64 = MAX_NATIVE_MODEL_BYTES;
pub const NATIVE_PRODUCER_MAX_SOURCE_CLOSURE_BYTES: u64 = MAX_CAPTURE_CLOSURE_BYTES;
pub const NATIVE_PRODUCER_MAX_MEMBERS: usize = MAX_DATA_MEMBERS;
/// Explicit independently expected source selection. Evidence refs remain opaque;
/// this profile selects data, never installed code or an ambient checkout.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSelectedSnapshotProfile {
    pub schema_version: String,
    pub runtime_data_root: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub corpus_revision: String,
    pub data_revision: String,
    pub runtime_data_declaration_sha256: String,
    pub evidence_scenes_sha256: String,
    pub excluded_compiled_model: SelectedExcludedSourceMember,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedExcludedSourceMember {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

fn runtime_data_declaration() -> Result<Value> {
    let limits = JsonLimits::new(
        64 * 1024,
        MAX_JSON_DEPTH,
        MAX_JSON_VISITS,
        MAX_JSON_INTEGER_DIGITS,
    )
    .map_err(|_| Error::Budget("runtime declaration limits"))?;
    parse_json(RUNTIME_DATA_DECLARATION, JsonMode::PublishedStrict, limits)
        .map_err(|error| Error::Source(error.to_string()))?;
    serde_json::from_slice(RUNTIME_DATA_DECLARATION)
        .map_err(|_| Error::Invalid("runtime data declaration JSON"))
}

impl NativeSelectedSnapshotProfile {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != "tos_native_selected_snapshot_profile_v1" {
            return Err(Error::Invalid("selected snapshot profile schema"));
        }
        for path in [&self.runtime_data_root, &self.manifest_path] {
            if path.len() > MAX_MEMBER_PATH_BYTES
                || !Path::new(path).is_absolute()
                || Path::new(path)
                    .components()
                    .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
            {
                return Err(Error::Invalid("selected snapshot explicit path"));
            }
        }
        for digest in [
            &self.manifest_sha256,
            &self.corpus_revision,
            &self.data_revision,
            &self.runtime_data_declaration_sha256,
            &self.evidence_scenes_sha256,
            &self.excluded_compiled_model.sha256,
        ] {
            sha_text(digest, "selected snapshot expected digest invalid")?;
        }
        // The owner declaration identifies the disposable legacy compiler output.
        // Its exact size/hash are independently selected, never a suffix/glob rule.
        let declaration = runtime_data_declaration()?;
        let outputs = declaration
            .get("compiled_subjects")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("runtime compiled subject declaration"))?;
        let excluded = &self.excluded_compiled_model;
        relative(&excluded.path, "selected excluded member path")?;
        if excluded.size_bytes == 0
            || !outputs.iter().any(|row| {
                row.get("output_path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| excluded.path == format!("data/{path}"))
            })
        {
            return Err(Error::Invalid(
                "selected exclusion is not declared compiler output",
            ));
        }
        if Digest256::of_bytes(RUNTIME_DATA_DECLARATION).to_hex()
            != self.runtime_data_declaration_sha256
            || Digest256::of_bytes(EVIDENCE_SCENES_SOURCE).to_hex() != self.evidence_scenes_sha256
        {
            return Err(Error::Invalid(
                "selected software source definition differs",
            ));
        }
        Ok(())
    }

    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        [
            &self.schema_version,
            &self.runtime_data_root,
            &self.manifest_path,
            &self.manifest_sha256,
            &self.corpus_revision,
            &self.data_revision,
            &self.runtime_data_declaration_sha256,
            &self.evidence_scenes_sha256,
            &self.excluded_compiled_model.path,
            &self.excluded_compiled_model.sha256,
        ]
        .into_iter()
        .try_fold(std::mem::size_of::<Self>(), |n, value| {
            n.checked_add(value.capacity())
                .ok_or(Error::Budget("selected profile state"))
        })
    }
}
const MAX_MEMBER_PATH_BYTES: usize = 4096;
const MAX_MEMBER_PATH_TOTAL_BYTES: usize = 512 * 1024;
const MAX_BINDING_PATH_TOTAL_BYTES: usize = 128 * 1024;
const MAX_COMPILER_PATH_TOTAL_BYTES: usize = 256 * 1024;
const MAX_JSON_VISITS: usize = 300_000;
const MAX_JSON_DEPTH: usize = 64;
const MAX_JSON_INTEGER_DIGITS: usize = 4300;
const NATIVE_COMPILER_FINGERPRINT_DOMAIN: &[u8] =
    b"tos-native-snapshot-embedded-producer-image-v1\0";
const SELECTED_MANIFEST_MAX_BYTES: usize = 2 * 1024 * 1024;

/// Compose the maintained FullKnowledge profile from the production portable
/// public limits, replacing no pipeline limit with a test-fixture shortcut.
/// The stage/capture byte caps are per resource; callers must separately
/// preflight their simultaneous tmpfs, RAM, filesystem and whole-deadline cost.
pub fn portable_native_snapshot_limits(
    max_build_seconds: u64,
) -> Result<crate::native_snapshot::NativeSnapshotLimits> {
    let mut base = crate::portable_public_d1_limits(max_build_seconds)?;
    base.capture.max_input_bytes = base.capture.max_input_bytes.min(MAX_CAPTURE_CLOSURE_BYTES);
    base.capture.max_staging_bytes = base
        .capture
        .max_staging_bytes
        .min(MAX_CAPTURE_STAGING_BYTES);
    base.stage.sqlite.max_output_bytes = base
        .stage
        .sqlite
        .max_output_bytes
        .min(MAX_NATIVE_MODEL_BYTES);
    base.stage.max_temp_bytes = base.stage.max_temp_bytes.min(MAX_NATIVE_STAGE_TEMP_BYTES);
    Ok(crate::native_snapshot::NativeSnapshotLimits {
        capture: base.capture,
        stage: base.stage,
        native: base.native,
        full: crate::FullKnowledgeLimits {
            scope: base.scope,
            catalog: base.catalog,
            catalog_index: crate::CatalogIndexLimits::default(),
            search: base.search,
            seal: crate::SealLimits {
                max_header_bytes: crate::knowledge_seal::MAX_GRAPH_HEADER_BYTES,
            },
            max_registry_bytes: 4 * 1024 * 1024,
        },
        originals: crate::CorpusOriginalSourceLimits {
            originals: crate::knowledge_original_rows::maximum_limits(),
            max_members: MAX_DATA_MEMBERS,
            max_work_bytes: crate::knowledge_original_rows::MAX_COLD_WORK,
        },
        max_transfer_work_bytes: base.max_stage_transfer_work_bytes,
        max_declaration_bytes: MAX_MANIFEST_BYTES,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCompilerFingerprint {
    /// SHA-256 over the sorted input-path, file-size, and digest sequence.
    compiler_sha256: String,
    /// Sorted embedded producer-code and producer-config inputs. The runtime
    /// data declaration remains a top-level source binding.
    compiler_paths: Vec<String>,
    /// Actual SHA-256 for every selected input represented in `compiler_paths`.
    input_bindings: BTreeMap<String, String>,
    code_sizes: BTreeMap<String, u64>,
    code_bytes: u64,
}
impl NativeCompilerFingerprint {
    /// Retained typed Rust owners only; the embedded/executing source bytes
    /// remain read/hash IO and are not represented as resident payloads here.
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            + self.compiler_sha256.capacity()
            + self.compiler_paths.capacity() * std::mem::size_of::<String>();
        let string_node =
            11 * std::mem::size_of::<(String, String)>() + 16 * std::mem::size_of::<usize>();
        let size_node =
            11 * std::mem::size_of::<(String, u64)>() + 16 * std::mem::size_of::<usize>();
        bytes = bytes
            .checked_add(
                self.input_bindings
                    .len()
                    .checked_mul(string_node)
                    .ok_or(Error::Budget("compiler fingerprint tree slots"))?,
            )
            .and_then(|n| n.checked_add(self.code_sizes.len().checked_mul(size_node)?))
            .ok_or(Error::Budget("compiler fingerprint retained slots"))?;
        for path in &self.compiler_paths {
            bytes = bytes
                .checked_add(path.capacity())
                .ok_or(Error::Budget("compiler fingerprint retained strings"))?;
        }
        for (path, digest) in &self.input_bindings {
            bytes = bytes
                .checked_add(path.capacity())
                .and_then(|n| n.checked_add(digest.capacity()))
                .ok_or(Error::Budget("compiler fingerprint retained strings"))?;
        }
        for path in self.code_sizes.keys() {
            bytes = bytes
                .checked_add(path.capacity())
                .ok_or(Error::Budget("compiler fingerprint retained strings"))?;
        }
        Ok(bytes)
    }
    pub fn compiler_sha256(&self) -> &str {
        &self.compiler_sha256
    }
    pub fn compiler_paths(&self) -> &[String] {
        &self.compiler_paths
    }
    pub fn input_bindings(&self) -> &BTreeMap<String, String> {
        &self.input_bindings
    }
    pub fn code_bytes(&self) -> u64 {
        self.code_bytes
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeDataManifestLimits {
    pub max_manifest_bytes: usize,
    /// Keep this below the host's admitted held-file descriptor count. The
    /// paired producer/controller path separately charges its fixed handles.
    pub max_members: usize,
    pub max_member_bytes: u64,
    /// Includes `data/manifest.json`, as does the private pair controller.
    pub max_total_data_bytes: u64,
    pub deadline: Instant,
}
impl NativeDataManifestLimits {
    fn validate(self) -> Result<()> {
        if self.max_manifest_bytes == 0
            || self.max_manifest_bytes > MAX_MANIFEST_BYTES
            || self.max_members == 0
            || self.max_members > MAX_DATA_MEMBERS
            || self.max_member_bytes == 0
            || self.max_member_bytes > MAX_DATA_BYTES
            || self.max_total_data_bytes == 0
            || self.max_total_data_bytes > MAX_DATA_BYTES
        {
            return Err(Error::Budget("native data manifest limits"));
        }
        active(self.deadline)
    }
}

#[derive(Clone)]
pub struct NativeDataSnapshotManifestInput<'a> {
    /// Selected admitted corpus revision. Keep this separate from the
    /// producer selection's `native-projection:<source revision>` source cut.
    pub corpus_revision: &'a str,
    pub selected_profile: &'a NativeSelectedSnapshotProfile,
    pub selected_census: &'a NativeSelectedSnapshotCensus,
    /// The selected model ABI from `NativeKnowledgeSelection::expectation()`.
    pub model_abi: &'a str,
    pub compiler: &'a NativeCompilerFingerprint,
    /// Compact authenticated bindings from the maintained capture source selector. The EvidenceLens scene definition is held and
    /// checked separately because it is not a member of the admitted bundle.
    pub source_bindings: &'a BTreeMap<String, String>,
    /// Exact paths below `data_root`, all prefixed with `data/`.
    pub member_paths: &'a [String],
    /// Exact path below `data_root` for the encoded producer selection.
    pub native_selection: &'a str,
}

#[derive(Clone, Debug)]
pub struct NativeDataManifestReceipt {
    pub manifest_sha256: String,
    pub data_revision: String,
    pub manifest_bytes: u64,
    pub member_count: u64,
    pub member_bytes: u64,
}

#[derive(Clone, Debug)]
struct MemberDigest {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    dev: u64,
    ino: u64,
    mode: u32,
    size: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCapturedMember {
    pub source_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoricalMember {
    sha256: String,
    size_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalManifestV1 {
    schema_version: String,
    corpus_revision: String,
    data_revision: String,
    input_bindings: BTreeMap<String, String>,
    compiler: HistoricalCompiler,
    members: Vec<HistoricalManifestMember>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalCompiler {
    schema: String,
    compiler_version: String,
    compiler_sha256: String,
    compiler_paths: Vec<String>,
    input_bindings: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalManifestMember {
    path: String,
    size_bytes: u64,
    sha256: String,
}

/// Held metadata-only view of the exact historical runtime snapshot. Its
/// payload paths are censused before capture/build; the compiler later checks
/// the retained PublicCapture closure against these rows before copying.
pub struct NativeSelectedSnapshotCensus {
    profile: NativeSelectedSnapshotProfile,
    source_root: PathBuf,
    manifest_path: PathBuf,
    manifest_file: File,
    manifest_stamp: FileStamp,
    manifest_sha256: String,
    members: BTreeMap<String, HistoricalMember>,
    member_bytes: u64,
    source_bindings: BTreeMap<String, String>,
}

impl NativeSelectedSnapshotCensus {
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        // One complete BTree node per member conservatively covers all
        // leaf/internal slots and child edges without inventing payload bytes.
        let node = 11 * std::mem::size_of::<(String, HistoricalMember)>()
            + 16 * std::mem::size_of::<usize>();
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.profile.retained_state_upper_bound()?)
            .and_then(|n| n.checked_add(self.source_root.capacity()))
            .and_then(|n| n.checked_add(self.manifest_path.capacity()))
            .and_then(|n| n.checked_add(self.manifest_sha256.capacity()))
            .and_then(|n| n.checked_add(self.members.len().checked_mul(node)?))
            .ok_or(Error::Budget("historical census retained slots"))?;
        for (path, member) in &self.members {
            bytes = bytes
                .checked_add(path.capacity())
                .and_then(|n| n.checked_add(member.sha256.capacity()))
                .ok_or(Error::Budget("historical census retained strings"))?;
        }
        let binding_node =
            11 * std::mem::size_of::<(String, String)>() + 16 * std::mem::size_of::<usize>();
        bytes = bytes
            .checked_add(
                self.source_bindings
                    .len()
                    .checked_mul(binding_node)
                    .ok_or(Error::Budget("selected binding slots"))?,
            )
            .ok_or(Error::Budget("selected binding slots"))?;
        for (path, digest) in &self.source_bindings {
            bytes = bytes
                .checked_add(path.capacity())
                .and_then(|n| n.checked_add(digest.capacity()))
                .ok_or(Error::Budget("selected binding strings"))?;
        }
        Ok(bytes)
    }
    pub fn source_bindings(&self) -> &BTreeMap<String, String> {
        &self.source_bindings
    }
    pub fn source_root(&self) -> &Path {
        &self.source_root
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
    pub fn member_bytes(&self) -> u64 {
        self.member_bytes
    }

    /// Rehash the same held old manifest and compare its current name binding.
    /// The manifest is metadata, not an admitted source payload.
    pub fn recheck_manifest(&mut self, deadline: Instant) -> Result<()> {
        active(deadline)?;
        let held = FileStamp::from(&self.manifest_file.metadata()?);
        let named_metadata = fs::symlink_metadata(&self.manifest_path)?;
        if held != self.manifest_stamp
            || FileStamp::from(&named_metadata) != self.manifest_stamp
            || named_metadata.file_type().is_symlink()
        {
            return Err(Error::Invalid("historical manifest custody changed"));
        }
        self.manifest_file.seek(SeekFrom::Start(0))?;
        let mut hash = Digest256Hasher::new();
        let mut total = 0u64;
        let mut block = [0u8; 64 * 1024];
        loop {
            active(deadline)?;
            let count = self.manifest_file.read(&mut block)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|value| *value <= SELECTED_MANIFEST_MAX_BYTES as u64)
                .ok_or(Error::Budget("historical manifest bytes"))?;
            hash.update(&block[..count]);
        }
        if total != self.manifest_stamp.size
            || hash.finalize().to_hex() != self.manifest_sha256
            || FileStamp::from(&self.manifest_file.metadata()?) != self.manifest_stamp
            || FileStamp::from(&fs::symlink_metadata(&self.manifest_path)?) != self.manifest_stamp
        {
            return Err(Error::Invalid("historical manifest changed after census"));
        }
        active(deadline)
    }

    /// Require exact historical source members plus only the current compiled
    /// runtime companions. The excluded Python SQLite output never enters the
    /// captured closure or candidate data tree.
    pub fn validate_capture_closure(
        &self,
        capture: &crate::PublicCapture,
        deadline: Instant,
    ) -> Result<Vec<NativeCapturedMember>> {
        active(deadline)?;
        let retained = capture.retained_input_members()?;
        if retained.len() > MAX_DATA_MEMBERS || retained.is_empty() {
            return Err(Error::Budget("native retained closure member count"));
        }
        let mut observed = BTreeMap::<String, HistoricalMember>::new();
        let mut observed_bytes = 0u64;
        for (path, digest, size_bytes) in retained {
            active(deadline)?;
            relative(&path, "native captured closure path invalid")?;
            observed_bytes = observed_bytes
                .checked_add(size_bytes)
                .filter(|value| *value <= 64 * 1024 * 1024)
                .ok_or(Error::Budget("native retained closure byte ceiling"))?;
            if observed
                .insert(
                    path,
                    HistoricalMember {
                        sha256: digest.to_hex(),
                        size_bytes,
                    },
                )
                .is_some()
            {
                return Err(Error::Invalid("native captured closure duplicate path"));
            }
        }
        let mut expected = self.members.clone();
        for (path, raw) in crate::d1_public_capture::runtime_companions() {
            let current = HistoricalMember {
                sha256: Digest256::of_bytes(raw).to_hex(),
                size_bytes: raw.len() as u64,
            };
            if observed.get(path) != Some(&current) {
                return Err(Error::Invalid(
                    "current compiled runtime companion absent or differs",
                ));
            }
            expected.insert(path.to_owned(), current);
        }
        if expected.len() != observed.len() || expected != observed {
            return Err(Error::Invalid(
                "captured closure differs from historical census",
            ));
        }
        for (path, digest) in &self.source_bindings {
            if expected.get(path).map(|member| &member.sha256) != Some(digest) {
                return Err(Error::Invalid("captured selected root differs"));
            }
        }
        let mut members = expected
            .into_iter()
            .map(|(source_path, member)| NativeCapturedMember {
                source_path,
                sha256: member.sha256,
                size_bytes: member.size_bytes,
            })
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        Ok(members)
    }
}

/// Metadata-only census of the old full runtime manifest and every retained
/// path. It skips only the independently pinned declared legacy output without opening its payload.
pub fn census_selected_runtime_closure(
    profile: &NativeSelectedSnapshotProfile,
    deadline: Instant,
) -> Result<NativeSelectedSnapshotCensus> {
    active(deadline)?;
    profile.validate()?;
    let source_root = PathBuf::from(&profile.runtime_data_root);
    no_symlink_path(&source_root)?;
    if !fs::symlink_metadata(&source_root)?.is_dir() {
        return Err(Error::Invalid("historical runtime source root"));
    }
    let manifest_path = PathBuf::from(&profile.manifest_path);
    no_symlink_path(&manifest_path)?;
    let mut manifest_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(&manifest_path)?;
    let manifest_metadata = manifest_file.metadata()?;
    let manifest_stamp = FileStamp::from(&manifest_metadata);
    if !manifest_metadata.is_file()
        || manifest_metadata.len() == 0
        || manifest_metadata.len() > SELECTED_MANIFEST_MAX_BYTES as u64
        || FileStamp::from(&fs::symlink_metadata(&manifest_path)?) != manifest_stamp
    {
        return Err(Error::Invalid("historical manifest size/type custody"));
    }
    let mut raw = Vec::with_capacity(manifest_stamp.size as usize);
    let mut hash = Digest256Hasher::new();
    let mut total = 0u64;
    let mut block = [0u8; 64 * 1024];
    loop {
        active(deadline)?;
        let count = manifest_file.read(&mut block)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|value| *value <= SELECTED_MANIFEST_MAX_BYTES as u64)
            .ok_or(Error::Budget("historical manifest read ceiling"))?;
        hash.update(&block[..count]);
        raw.extend_from_slice(&block[..count]);
    }
    let manifest_sha256 = hash.finalize().to_hex();
    if total != manifest_stamp.size
        || manifest_sha256 != profile.manifest_sha256
        || FileStamp::from(&manifest_file.metadata()?) != manifest_stamp
        || FileStamp::from(&fs::symlink_metadata(&manifest_path)?) != manifest_stamp
    {
        return Err(Error::Invalid("historical manifest digest/custody"));
    }
    let json_limits = JsonLimits::new(
        SELECTED_MANIFEST_MAX_BYTES,
        MAX_JSON_DEPTH,
        700_000,
        MAX_JSON_INTEGER_DIGITS,
    )
    .map_err(|_| Error::Budget("historical manifest JSON limits"))?;
    parse_json(&raw, JsonMode::PublishedStrict, json_limits)
        .map_err(|error| Error::Source(error.to_string()))?;
    let old: HistoricalManifestV1 =
        serde_json::from_slice(&raw).map_err(|error| Error::Source(error.to_string()))?;
    drop(raw);
    if old.schema_version != "tos_access_data_snapshot_v1"
        || old.corpus_revision != profile.corpus_revision
        || old.data_revision != profile.data_revision
        || old.members.is_empty()
        || old.members.len() > MAX_DATA_MEMBERS + 1
        || old.input_bindings.len() > MAX_DATA_MEMBERS
        || old.compiler.compiler_paths.is_empty()
        || old.compiler.compiler_paths.len() > MAX_PRODUCER_FILES
        || old
            .compiler
            .compiler_paths
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || old
            .compiler
            .compiler_paths
            .iter()
            .any(|path| relative(path, "historical compiler path invalid").is_err())
        || old.compiler.compiler_version.is_empty()
        || old.compiler.schema.is_empty()
        || sha_text(
            &old.compiler.compiler_sha256,
            "historical compiler hash invalid",
        )
        .is_err()
    {
        return Err(Error::Invalid("historical snapshot manifest identity"));
    }
    validate_map(
        &old.compiler.input_bindings,
        "historical compiler input binding invalid",
        MAX_PRODUCER_FILES,
        MAX_COMPILER_PATH_TOTAL_BYTES,
    )?;
    // The authenticated manifest owns historical compiler data/config bindings.
    for (path, digest) in &old.compiler.input_bindings {
        if old.input_bindings.get(path) != Some(digest) {
            return Err(Error::Invalid("selected compiler config binding differs"));
        }
    }
    validate_map(
        &old.input_bindings,
        "selected input binding invalid",
        MAX_DATA_MEMBERS,
        MAX_MEMBER_PATH_TOTAL_BYTES,
    )?;
    let mut total_member_bytes = 0u64;
    let mut retained_bytes = 0u64;
    let mut excluded_seen = false;
    let mut previous_path = String::new();
    let mut members = BTreeMap::new();
    for member in old.members {
        active(deadline)?;
        if member.path.len() > MAX_MEMBER_PATH_BYTES
            || !member.path.is_ascii()
            || !member.path.starts_with("data/")
            || member.path == "data/manifest.json"
            || member.path <= previous_path
        {
            return Err(Error::Invalid("historical member path/order"));
        }
        relative(&member.path, "historical member path invalid")?;
        previous_path = member.path.clone();
        sha_text(&member.sha256, "historical member digest invalid")?;
        total_member_bytes = total_member_bytes
            .checked_add(member.size_bytes)
            .ok_or(Error::Budget("historical member byte arithmetic"))?;
        if member.path == profile.excluded_compiled_model.path {
            if excluded_seen
                || member.size_bytes != profile.excluded_compiled_model.size_bytes
                || member.sha256 != profile.excluded_compiled_model.sha256
                || old.input_bindings.contains_key(
                    profile
                        .excluded_compiled_model
                        .path
                        .strip_prefix("data/")
                        .ok_or(Error::Invalid("excluded data prefix"))?,
                )
            {
                return Err(Error::Invalid(
                    "historical compiled model exclusion differs",
                ));
            }
            excluded_seen = true;
            continue;
        }
        let source_path = member
            .path
            .strip_prefix("data/")
            .ok_or(Error::Invalid("historical member prefix"))?
            .to_owned();
        if old.input_bindings.get(&source_path) != Some(&member.sha256) {
            return Err(Error::Invalid("historical member/input binding differs"));
        }
        retained_bytes = retained_bytes
            .checked_add(member.size_bytes)
            .filter(|bytes| *bytes <= MAX_CAPTURE_CLOSURE_BYTES)
            .ok_or(Error::Budget("selected source byte ceiling"))?;
        let path = source_root.join(&source_path);
        no_symlink_path(&path)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.len() != member.size_bytes {
            return Err(Error::Invalid("historical source path metadata differs"));
        }
        if members
            .insert(
                source_path,
                HistoricalMember {
                    sha256: member.sha256,
                    size_bytes: member.size_bytes,
                },
            )
            .is_some()
        {
            return Err(Error::Invalid("historical member duplicate path"));
        }
    }
    if old.input_bindings.len() != members.len() || !excluded_seen {
        return Err(Error::Invalid("selected source closure census differs"));
    }
    let mut source_bindings = BTreeMap::new();
    let declaration = runtime_data_declaration()?;
    let subjects = declaration
        .get("subjects")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("runtime source subjects"))?;
    let selected = crate::PublicCaptureInputPaths::runtime(&source_root);
    for (path, required) in selected.selected_paths() {
        let relative = path
            .strip_prefix(&source_root)
            .map_err(|_| Error::Invalid("selected input root"))?
            .to_str()
            .ok_or(Error::Invalid("selected input UTF8"))?;
        if let Some(member) = members.get(relative) {
            source_bindings.insert(relative.to_owned(), member.sha256.clone());
        } else if required
            || subjects.iter().any(|row| {
                row.get("source_path").and_then(Value::as_str) == Some(relative)
                    && row.get("required").and_then(Value::as_bool) == Some(true)
            })
        {
            return Err(Error::Invalid("selected source root missing"));
        }
    }
    active(deadline)?;
    Ok(NativeSelectedSnapshotCensus {
        profile: profile.clone(),
        source_root,
        manifest_path,
        manifest_file,
        manifest_stamp,
        manifest_sha256,
        members,
        member_bytes: retained_bytes,
        source_bindings,
    })
}

impl From<&fs::Metadata> for FileStamp {
    fn from(m: &fs::Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            mode: m.mode(),
            size: m.len(),
            mtime: m.mtime(),
            mtime_nsec: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_nsec: m.ctime_nsec(),
        }
    }
}

fn active(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(Error::Budget("native data manifest deadline"));
    }
    Ok(())
}
fn sha_text(value: &str, label: &'static str) -> Result<()> {
    Digest256::from_hex(value)
        .map(|_| ())
        .map_err(|_| Error::Invalid(label))
}
fn relative(value: &str, label: &'static str) -> Result<()> {
    RelativePath::parse(value)
        .map(|_| ())
        .map_err(|_| Error::Invalid(label))
}
fn no_symlink_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(Error::Invalid(
            "native producer path must be absolute and normalized",
        ));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part.as_os_str());
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::Invalid("native producer path contains symlink"));
        }
    }
    Ok(())
}
fn private_directory(path: &Path) -> Result<()> {
    no_symlink_path(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
        return Err(Error::Invalid(
            "native data output directory is not private",
        ));
    }
    Ok(())
}
fn file_stamp(path: &Path, file: &File) -> Result<FileStamp> {
    let descriptor = file.metadata()?;
    let name = fs::symlink_metadata(path)?;
    if !descriptor.is_file()
        || name.file_type().is_symlink()
        || !name.is_file()
        || FileStamp::from(&descriptor) != FileStamp::from(&name)
    {
        return Err(Error::Invalid("native producer regular-file custody"));
    }
    Ok(FileStamp::from(&descriptor))
}
fn read_digest(path: &Path, cap: u64, deadline: Instant) -> Result<(u64, Digest256)> {
    no_symlink_path(path)?;
    let mut file = crate::safe_open::open_regular(path, cap)?;
    let before = file_stamp(path, &file)?;
    if before.size > cap {
        return Err(Error::Budget("native producer file byte cap"));
    }
    let mut count = 0u64;
    let mut hash = Digest256Hasher::new();
    let mut block = [0u8; 64 * 1024];
    loop {
        active(deadline)?;
        let n = file.read(&mut block)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .filter(|value| *value <= cap)
            .ok_or(Error::Budget("native producer file byte cap"))?;
        hash.update(&block[..n]);
    }
    let after = file_stamp(path, &file)?;
    if before != after || count != before.size {
        return Err(Error::Invalid("native producer file changed while hashing"));
    }
    Ok((count, hash.finalize()))
}
fn validate_map(
    map: &BTreeMap<String, String>,
    label: &'static str,
    max_entries: usize,
    max_path_bytes: usize,
) -> Result<()> {
    if map.is_empty() {
        return Err(Error::Invalid(label));
    }
    if map.len() > max_entries {
        return Err(Error::Budget("native manifest input binding count"));
    }
    let mut path_bytes = 0usize;
    for (path, digest) in map {
        if path.len() > MAX_MEMBER_PATH_BYTES || !path.is_ascii() {
            return Err(Error::Budget("native manifest input binding path bytes"));
        }
        path_bytes = path_bytes
            .checked_add(path.len())
            .filter(|value| *value <= max_path_bytes)
            .ok_or(Error::Budget(
                "native manifest input binding path aggregate",
            ))?;
        relative(path, label)?;
        sha_text(digest, label)?;
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExecutableStamp {
    dev: u64,
    ino: u64,
    size: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
impl From<&fs::Metadata> for ExecutableStamp {
    fn from(value: &fs::Metadata) -> Self {
        Self {
            dev: value.dev(),
            ino: value.ino(),
            size: value.len(),
            mtime: value.mtime(),
            mtime_nsec: value.mtime_nsec(),
            ctime: value.ctime(),
            ctime_nsec: value.ctime_nsec(),
        }
    }
}
fn executing_image_digest(deadline: Instant) -> Result<(String, u64)> {
    let named_path = std::env::current_exe()?;
    let mut held = File::open("/proc/self/exe")?;
    let before = held.metadata()?;
    let stamp = ExecutableStamp::from(&before);
    if !before.is_file() || stamp.size == 0 || stamp.size > MAX_EXECUTING_IMAGE_BYTES {
        return Err(Error::Budget("native executing image byte ceiling"));
    }
    if ExecutableStamp::from(&fs::metadata(&named_path)?) != stamp {
        return Err(Error::Invalid("native executing image held/name identity"));
    }
    held.seek(SeekFrom::Start(0))?;
    let mut header = [0u8; 4];
    held.read_exact(&mut header)?;
    if header != *b"\x7fELF" {
        return Err(Error::Invalid("native executing image is not ELF"));
    }
    let mut hash = Digest256Hasher::new();
    hash.update(&header);
    let mut total = header.len() as u64;
    let mut block = [0u8; 64 * 1024];
    loop {
        active(deadline)?;
        let count = held.read(&mut block)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|value| *value <= MAX_EXECUTING_IMAGE_BYTES)
            .ok_or(Error::Budget("native executing image read ceiling"))?;
        hash.update(&block[..count]);
    }
    if total != stamp.size
        || ExecutableStamp::from(&held.metadata()?) != stamp
        || ExecutableStamp::from(&fs::metadata(&named_path)?) != stamp
    {
        return Err(Error::Invalid(
            "native executing image changed while hashing",
        ));
    }
    Ok((hash.finalize().to_hex(), total))
}

/// Fingerprint a fixed embedded producer-code/config projection and the held
/// running ELF. The selected list is truthful but intentionally incomplete;
/// `tos-command/current-executable` binds the actual image. The separate
/// runtime-data declaration is a top-level source binding. Call before and
/// after the build and cold-open; equality is byte stability only.
pub fn fingerprint_native_compiler_source(deadline: Instant) -> Result<NativeCompilerFingerprint> {
    active(deadline)?;
    let (image_digest, image_bytes) = executing_image_digest(deadline)?;
    let mut code_inputs = BTreeMap::<String, (String, u64)>::new();
    let mut source_bytes = 0u64;
    for (path, raw) in EMBEDDED_PRODUCER_INPUTS {
        active(deadline)?;
        relative(path, "native compiler source path invalid")?;
        let size = raw.len() as u64;
        if size == 0 || size > MAX_PRODUCER_FILE_BYTES {
            return Err(Error::Budget("native embedded producer source bytes"));
        }
        source_bytes = source_bytes
            .checked_add(size)
            .filter(|value| *value <= MAX_PRODUCER_SOURCE_BYTES)
            .ok_or(Error::Budget("native embedded producer source aggregate"))?;
        if code_inputs
            .insert(path.to_owned(), (Digest256::of_bytes(raw).to_hex(), size))
            .is_some()
        {
            return Err(Error::Invalid("native compiler source path duplicate"));
        }
    }
    code_inputs.insert(
        EXECUTING_IMAGE_LOGICAL_PATH.to_owned(),
        (image_digest, image_bytes),
    );
    if code_inputs.len() > MAX_PRODUCER_FILES {
        return Err(Error::Budget("native compiler source file count"));
    }
    let code_bytes = source_bytes
        .checked_add(image_bytes)
        .filter(|value| *value <= MAX_PRODUCER_TOTAL_BYTES)
        .ok_or(Error::Budget("native compiler code plus image bytes"))?;
    if code_bytes == 0 {
        return Err(Error::Invalid("native compiler source list empty"));
    }
    let compiler_paths = code_inputs.keys().cloned().collect::<Vec<_>>();
    let mut compiler_path_bytes = 0usize;
    let mut input_bindings = BTreeMap::new();
    let mut code_sizes = BTreeMap::new();
    let mut hasher = Digest256Hasher::new();
    hasher.update(NATIVE_COMPILER_FINGERPRINT_DOMAIN);
    for (path, (digest, size)) in code_inputs {
        active(deadline)?;
        compiler_path_bytes = compiler_path_bytes
            .checked_add(path.len())
            .filter(|value| *value <= MAX_COMPILER_PATH_TOTAL_BYTES)
            .ok_or(Error::Budget("native compiler input path aggregate"))?;
        hasher.update(&(path.len() as u64).to_be_bytes());
        hasher.update(path.as_bytes());
        hasher.update(&size.to_be_bytes());
        hasher.update(
            Digest256::from_hex(&digest)
                .map_err(|_| Error::Invalid("native embedded producer digest"))?
                .as_bytes(),
        );
        input_bindings.insert(path.clone(), digest);
        code_sizes.insert(path, size);
    }
    Ok(NativeCompilerFingerprint {
        compiler_sha256: hasher.finalize().to_hex(),
        compiler_paths,
        input_bindings,
        code_sizes,
        code_bytes,
    })
}

/// Require identical selected producer-code/config and executing-image inputs
/// before using the fingerprint in a candidate manifest.
pub fn require_stable_native_compiler_source(
    before: &NativeCompilerFingerprint,
    after: &NativeCompilerFingerprint,
) -> Result<()> {
    if before != after {
        return Err(Error::Invalid(
            "native compiler source changed during production",
        ));
    }
    Ok(())
}

fn validate_compiler_fingerprint(
    compiler: &NativeCompilerFingerprint,
    deadline: Instant,
) -> Result<()> {
    if compiler.compiler_paths.is_empty()
        || compiler.compiler_paths.len() > MAX_PRODUCER_FILES
        || compiler
            .compiler_paths
            .windows(2)
            .any(|window| window[0] >= window[1])
        || compiler.compiler_paths.iter().any(|path| {
            path.len() > MAX_MEMBER_PATH_BYTES
                || !path.is_ascii()
                || relative(path, "native manifest compiler path invalid").is_err()
        })
    {
        return Err(Error::Invalid("native manifest compiler paths invalid"));
    }
    validate_map(
        &compiler.input_bindings,
        "native manifest compiler input invalid",
        MAX_PRODUCER_FILES,
        MAX_COMPILER_PATH_TOTAL_BYTES,
    )?;
    sha_text(
        &compiler.compiler_sha256,
        "native manifest compiler digest invalid",
    )?;
    if compiler.compiler_paths.len() != compiler.code_sizes.len()
        || compiler.code_sizes.len() != compiler.input_bindings.len()
        || compiler.compiler_paths.iter().any(|path| {
            !compiler.code_sizes.contains_key(path) || !compiler.input_bindings.contains_key(path)
        })
    {
        return Err(Error::Invalid(
            "native manifest compiler fingerprint inputs differ",
        ));
    }
    let mut total = 0u64;
    let mut source_total = 0u64;
    let mut image_seen = false;
    let mut hasher = Digest256Hasher::new();
    hasher.update(NATIVE_COMPILER_FINGERPRINT_DOMAIN);
    for path in &compiler.compiler_paths {
        active(deadline)?;
        let digest = compiler
            .input_bindings
            .get(path)
            .ok_or(Error::Invalid("native compiler path binding absent"))?;
        let bytes = Digest256::from_hex(digest)
            .map_err(|_| Error::Invalid("native compiler path binding invalid"))?;
        let size = compiler
            .code_sizes
            .get(path)
            .copied()
            .ok_or(Error::Invalid("native compiler path size absent"))?;
        if path == EXECUTING_IMAGE_LOGICAL_PATH {
            if image_seen || size == 0 || size > MAX_EXECUTING_IMAGE_BYTES {
                return Err(Error::Budget("native executing image size ceiling"));
            }
            image_seen = true;
        } else {
            source_total = source_total
                .checked_add(size)
                .filter(|value| *value <= MAX_PRODUCER_SOURCE_BYTES)
                .ok_or(Error::Budget("native embedded producer source aggregate"))?;
            if size == 0 || size > MAX_PRODUCER_FILE_BYTES {
                return Err(Error::Budget("native embedded producer source bytes"));
            }
        }
        total = total
            .checked_add(size)
            .filter(|value| *value <= MAX_PRODUCER_TOTAL_BYTES)
            .ok_or(Error::Budget("native compiler source aggregate bytes"))?;
        hasher.update(&(path.len() as u64).to_be_bytes());
        hasher.update(path.as_bytes());
        hasher.update(&size.to_be_bytes());
        hasher.update(bytes.as_bytes());
    }
    let expected_paths = EMBEDDED_PRODUCER_INPUTS
        .iter()
        .map(|(path, _)| (*path).to_owned())
        .chain(std::iter::once(EXECUTING_IMAGE_LOGICAL_PATH.to_owned()))
        .collect::<std::collections::BTreeSet<_>>();
    if !image_seen
        || expected_paths.len() != compiler.compiler_paths.len()
        || compiler
            .compiler_paths
            .iter()
            .any(|path| !expected_paths.contains(path))
        || total != compiler.code_bytes
        || hasher.finalize().to_hex() != compiler.compiler_sha256
    {
        return Err(Error::Invalid(
            "native manifest compiler fingerprint digest differs",
        ));
    }
    Ok(())
}

fn validate_manifest_input(
    input: &NativeDataSnapshotManifestInput<'_>,
    limits: NativeDataManifestLimits,
) -> Result<Vec<String>> {
    limits.validate()?;
    sha_text(input.corpus_revision, "native corpus revision invalid")?;
    input.selected_profile.validate()?;
    if &input.selected_census.profile != input.selected_profile
        || input.selected_census.manifest_sha256 != input.selected_profile.manifest_sha256
        || input.selected_census.source_root != Path::new(&input.selected_profile.runtime_data_root)
        || input.selected_census.manifest_path != Path::new(&input.selected_profile.manifest_path)
    {
        return Err(Error::Invalid(
            "selected manifest census/profile binding differs",
        ));
    }
    if input.corpus_revision != input.selected_profile.corpus_revision {
        return Err(Error::Invalid("historical corpus revision changed"));
    }
    if !matches!(
        input.model_abi,
        crate::KNOWLEDGE_CORPUS_MODEL_ABI
            | crate::knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI
            | tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1 | tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2
    ) || input.model_abi.len() > 256
    {
        return Err(Error::Invalid("native manifest model ABI"));
    }
    validate_map(
        input.source_bindings,
        "native manifest source binding invalid",
        MAX_DATA_MEMBERS,
        MAX_BINDING_PATH_TOTAL_BYTES,
    )?;
    validate_compiler_fingerprint(input.compiler, limits.deadline)?;
    if input.native_selection != NATIVE_SELECTION_PATH {
        return Err(Error::Invalid("native selection manifest path invalid"));
    }
    relative(
        input.native_selection,
        "native selection manifest path invalid",
    )?;
    if input.member_paths.is_empty() || input.member_paths.len() > limits.max_members {
        return Err(Error::Budget("native manifest member count"));
    }
    let mut path_bytes = 0usize;
    for path in input.member_paths {
        active(limits.deadline)?;
        if path.len() > MAX_MEMBER_PATH_BYTES || !path.is_ascii() {
            return Err(Error::Budget("native manifest member path bytes"));
        }
        path_bytes = path_bytes
            .checked_add(path.len())
            .filter(|value| *value <= MAX_MEMBER_PATH_TOTAL_BYTES)
            .ok_or(Error::Budget("native manifest member path aggregate"))?;
    }
    let mut paths = input.member_paths.to_vec();
    paths.sort();
    if paths.is_empty()
        || paths.len() > limits.max_members
        || paths.windows(2).any(|pair| pair[0] == pair[1])
        || paths.iter().any(|path| {
            !path.starts_with("data/")
                || path == "data/manifest.json"
                || relative(path, "native manifest member path invalid").is_err()
        })
        || !paths.iter().any(|path| path == input.native_selection)
    {
        return Err(Error::Invalid("native manifest member paths invalid"));
    }
    if paths
        .iter()
        .any(|path| path == &format!("data/{EVIDENCE_SCENES_PATH}"))
    {
        return Err(Error::Invalid(
            "EvidenceLens scene definition is source-only",
        ));
    }
    for source in input.source_bindings.keys() {
        if source == RUNTIME_DATA_DECLARATION_PATH || source == EVIDENCE_SCENES_PATH {
            continue;
        }
        let member = format!("data/{source}");
        if !paths.iter().any(|path| path == &member) {
            return Err(Error::Invalid("native source binding lacks data member"));
        }
    }
    for (path, expected) in &input.selected_census.source_bindings {
        if input.source_bindings.get(path) != Some(expected) {
            return Err(Error::Invalid("selected source root binding changed"));
        }
    }
    if !input
        .selected_census
        .source_bindings
        .contains_key(CORPUS_INDEX_PATH)
    {
        return Err(Error::Invalid("selected corpus root binding absent"));
    }
    let declaration_digest = Digest256::of_bytes(RUNTIME_DATA_DECLARATION).to_hex();
    if input
        .source_bindings
        .get(RUNTIME_DATA_DECLARATION_PATH)
        .map(String::as_str)
        != Some(declaration_digest.as_str())
        || input
            .source_bindings
            .get(EVIDENCE_SCENES_PATH)
            .map(String::as_str)
            != Some(input.selected_profile.evidence_scenes_sha256.as_str())
        || Digest256::of_bytes(EVIDENCE_SCENES_SOURCE).to_hex()
            != input.selected_profile.evidence_scenes_sha256
    {
        return Err(Error::Invalid(
            "native software/source-definition binding differs",
        ));
    }
    Ok(paths)
}

fn manifest_value(
    input: &NativeDataSnapshotManifestInput<'_>,
    members: &[MemberDigest],
    data_revision: Option<&str>,
) -> Result<Value> {
    let member_values = members
        .iter()
        .map(|member| {
            json!({
                "path": member.path,
                "size_bytes": member.size_bytes,
                "sha256": member.sha256,
            })
        })
        .collect::<Vec<_>>();
    let mut value = json!({
        "schema_version": NATIVE_DATA_SCHEMA,
        "corpus_revision": input.corpus_revision,
        "input_bindings": input.source_bindings,
        "compiler": {
            "schema": input.model_abi,
            "compiler_version": COMPILER_VERSION,
            "compiler_sha256": input.compiler.compiler_sha256,
            "compiler_paths": input.compiler.compiler_paths,
            "input_bindings": input.compiler.input_bindings,
        },
        "members": member_values,
        "native_selection": input.native_selection,
    });
    if let Some(revision) = data_revision {
        value
            .as_object_mut()
            .ok_or(Error::Invalid("native data manifest object"))?
            .insert("data_revision".into(), Value::String(revision.to_owned()));
    }
    Ok(value)
}
fn canonical_value(value: &Value, cap: usize) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(value).map_err(|e| Error::Source(e.to_string()))?;
    let limits = JsonLimits::new(
        cap,
        MAX_JSON_DEPTH,
        MAX_JSON_VISITS,
        MAX_JSON_INTEGER_DIGITS,
    )
    .map_err(|_| Error::Budget("native data manifest JSON limits"))?;
    let document = parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    canonical_bytes_v1(
        &document.into_root(),
        CanonicalProfile::CorpusSnapshotV1,
        limits,
    )
    .map_err(|e| Error::Source(e.to_string()))
}

/// Metadata-only ceiling check. Use immediately after the old manifest closure
/// census and before copying source members or constructing a candidate.
pub fn preflight_native_data_manifest(
    input: &NativeDataSnapshotManifestInput<'_>,
    limits: NativeDataManifestLimits,
) -> Result<usize> {
    let paths = validate_manifest_input(input, limits)?;
    // Build only the fixed metadata base (the member array is empty), then
    // account each validated ASCII path arithmetically. This keeps hostile
    // 4,090-path inputs from constructing a proportional serde DOM first.
    let empty = manifest_value(input, &[], Some(&"f".repeat(64)))?;
    let base = canonical_value(&empty, limits.max_manifest_bytes)?;
    let mut projected = base.len() as u64;
    for (index, path) in paths.iter().enumerate() {
        active(limits.deadline)?;
        let escaped_path_bytes = path.bytes().try_fold(0u64, |sum, byte| {
            sum.checked_add(1 + (byte == b'"') as u64)
                .ok_or(Error::Budget("native data manifest path arithmetic"))
        })?;
        let item = escaped_path_bytes
            .checked_add(121)
            .and_then(|value| value.checked_add(if index == 0 { 0 } else { 1 }))
            .ok_or(Error::Budget("native data manifest metadata arithmetic"))?;
        projected = projected
            .checked_add(item)
            .filter(|value| *value <= limits.max_manifest_bytes as u64)
            .ok_or(Error::Budget("native data manifest metadata ceiling"))?;
    }
    let projected = usize::try_from(projected)
        .map_err(|_| Error::Budget("native data manifest metadata ceiling"))?;
    if projected > limits.max_manifest_bytes {
        return Err(Error::Budget("native data manifest metadata ceiling"));
    }
    Ok(projected)
}

fn member_path(data_root: &Path, relative_path: &str) -> Result<PathBuf> {
    let rel = RelativePath::parse(relative_path)
        .map_err(|_| Error::Invalid("native manifest member path invalid"))?;
    let path = data_root.join(rel.as_str());
    no_symlink_path(&path)?;
    Ok(path)
}

/// Write the exact native manifest accepted by the current ManagedLocal
/// reader. The caller must have already copied a real completed model, encoded
/// selection, source closure, and descriptor/registry members into `data_root`.
fn write_native_data_manifest(
    data_root: &Path,
    input: &NativeDataSnapshotManifestInput<'_>,
    selection: &NativeKnowledgeSelection,
    limits: NativeDataManifestLimits,
) -> Result<NativeDataManifestReceipt> {
    let paths = validate_manifest_input(input, limits)?;
    let projected = preflight_native_data_manifest(input, limits)?;
    if projected > limits.max_manifest_bytes {
        return Err(Error::Budget("native data manifest metadata ceiling"));
    }
    private_directory(data_root)?;
    let data_dir = data_root.join("data");
    private_directory(&data_dir)?;
    if !data_root.is_dir() || !data_dir.is_dir() {
        return Err(Error::Invalid(
            "native data root must be prepared directories",
        ));
    }
    if selection.expectation().model_abi != input.model_abi
        || selection.producer().corpus_original.is_none()
        || selection.producer().philosophy_original.is_none()
        || selection.producer().managed_source.is_some()
        || selection.producer().managed_source_v2.is_some()
    {
        return Err(Error::Invalid("native Original producer selection differs"));
    }
    if selection.paths().model != NATIVE_MODEL_PATH
        || selection.paths().descriptor
            != "data/ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        || selection.paths().entity_registry
            != "data/ToS/doctrine/semantic-interchange/entity-types.v1.json"
        || selection.paths().relation_registry
            != "data/ToS/doctrine/semantic-interchange/relation-types.v1.json"
    {
        return Err(Error::Invalid(
            "native selection paths differ from current contract",
        ));
    }
    for path in [
        &selection.paths().model,
        &selection.paths().descriptor,
        &selection.paths().entity_registry,
        &selection.paths().relation_registry,
    ] {
        if !paths.iter().any(|member| member == path) {
            return Err(Error::Invalid("native selection companion member absent"));
        }
    }
    let selected_raw = selection.encode(limits.max_manifest_bytes)?;
    let mut members = Vec::with_capacity(paths.len());
    let mut member_bytes = 0u64;
    let mut member_digests = BTreeMap::new();
    for relative in paths {
        active(limits.deadline)?;
        let path = member_path(data_root, &relative)?;
        let (size_bytes, digest) = read_digest(&path, limits.max_member_bytes, limits.deadline)?;
        member_bytes = member_bytes
            .checked_add(size_bytes)
            .filter(|value| *value <= limits.max_total_data_bytes)
            .ok_or(Error::Budget("native data snapshot byte ceiling"))?;
        let digest_text = digest.to_hex();
        if relative == input.native_selection && digest != Digest256::of_bytes(&selected_raw) {
            return Err(Error::Invalid(
                "native selection bytes differ from producer",
            ));
        }
        if let Some(source_path) = relative.strip_prefix("data/") {
            if let Some(expected) = input.source_bindings.get(source_path) {
                if expected != &digest_text {
                    return Err(Error::Invalid(
                        "native source binding/member digest differs",
                    ));
                }
            }
        }
        member_digests.insert(relative.clone(), digest_text.clone());
        members.push(MemberDigest {
            path: relative,
            size_bytes,
            sha256: digest_text,
        });
    }
    members.sort_by(|a, b| a.path.cmp(&b.path));
    let evidence_member = member_digests
        .get(&format!("data/{EVIDENCE_PROJECTION_PATH}"))
        .ok_or(Error::Invalid(
            "native EvidenceLens projection member absent",
        ))?;
    if input.source_bindings.get(EVIDENCE_PROJECTION_PATH) != Some(evidence_member)
        || !input.source_bindings.contains_key(EVIDENCE_SCENES_PATH)
        || !input
            .source_bindings
            .contains_key(RUNTIME_DATA_DECLARATION_PATH)
    {
        return Err(Error::Invalid("native EvidenceLens source binding differs"));
    }
    for (source, expected) in input.source_bindings {
        if source == RUNTIME_DATA_DECLARATION_PATH || source == EVIDENCE_SCENES_PATH {
            continue;
        }
        if member_digests.get(&format!("data/{source}")) != Some(expected) {
            return Err(Error::Invalid("native source binding/member absent"));
        }
    }
    let body = manifest_value(input, &members, None)?;
    let body_raw = canonical_value(&body, limits.max_manifest_bytes)?;
    let data_revision = Digest256::of_bytes(&body_raw).to_hex();
    let final_value = manifest_value(input, &members, Some(&data_revision))?;
    let final_raw = canonical_value(&final_value, limits.max_manifest_bytes)?;
    let final_total = member_bytes
        .checked_add(final_raw.len() as u64)
        .filter(|value| *value <= limits.max_total_data_bytes)
        .ok_or(Error::Budget("native data snapshot byte ceiling"))?;
    if final_raw.len() > limits.max_manifest_bytes || final_total > limits.max_total_data_bytes {
        return Err(Error::Budget("native data manifest output ceiling"));
    }
    active(limits.deadline)?;
    let manifest_path = data_dir.join("manifest.json");
    if manifest_path.exists() || manifest_path.is_symlink() {
        return Err(Error::Invalid(
            "native data manifest destination is not fresh",
        ));
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(&manifest_path)?;
    output.write_all(&final_raw)?;
    output.sync_all()?;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(&data_dir)?
        .sync_all()?;
    let manifest_sha256 = Digest256::of_bytes(&final_raw).to_hex();
    active(limits.deadline)?;
    Ok(NativeDataManifestReceipt {
        manifest_sha256,
        data_revision,
        manifest_bytes: final_raw.len() as u64,
        member_count: members.len() as u64,
        member_bytes,
    })
}

fn same_json<T: serde::Serialize>(left: &T, right: &T) -> Result<bool> {
    let left = serde_json::to_value(left).map_err(|e| Error::Source(e.to_string()))?;
    let right = serde_json::to_value(right).map_err(|e| Error::Source(e.to_string()))?;
    Ok(left == right)
}

/// Maintained entry from a completed native snapshot to its current Access
/// manifest. It rejects selection receipts detached from the completed stage,
/// its full seal, or its Original pair before writing the manifest.
pub fn write_completed_native_data_manifest(
    completed: &crate::native_snapshot::CompletedNativeSnapshot,
    data_root: &Path,
    input: &NativeDataSnapshotManifestInput<'_>,
    selection: &NativeKnowledgeSelection,
    limits: NativeDataManifestLimits,
) -> Result<NativeDataManifestReceipt> {
    let producer = completed.producer();
    let corpus = producer
        .corpus_original
        .as_ref()
        .ok_or(Error::Invalid("completed native corpus Original absent"))?;
    if completed.expectation().model_abi != input.model_abi
        || completed.declaration_sha256() != Digest256::of_bytes(RUNTIME_DATA_DECLARATION)
        || corpus.origin.profile != "captured-runtime-projection-v1"
        || corpus.origin.source_path != CORPUS_INDEX_PATH
        || corpus.origin.source_git_commit.is_some()
        || corpus.origin.source_git_tree.is_some()
        || input.source_bindings.get(CORPUS_INDEX_PATH) != Some(&corpus.origin.source_sha256)
        || completed.expectation().source_cut
            != format!("native-projection:{}", completed.source_revision())
        || corpus.source_cut != completed.expectation().source_cut
        || !same_json(completed.expectation(), selection.expectation())?
        || !same_json(completed.stage(), &selection.producer().stage)?
        || !same_json(&completed.components().seal, &selection.producer().seal)?
        || !same_json(
            &producer.navigation_original,
            &selection.producer().navigation_original,
        )?
        || !same_json(
            &producer.philosophy_original,
            &selection.producer().philosophy_original,
        )?
        || !same_json(
            &producer.corpus_original,
            &selection.producer().corpus_original,
        )?
    {
        return Err(Error::Invalid(
            "native selection detached from completed producer",
        ));
    }
    write_native_data_manifest(data_root, input, selection, limits)
}

#[cfg(test)]
mod selected_snapshot_tests {
    use super::*;

    fn fixture(root: &Path) -> NativeSelectedSnapshotProfile {
        fs::create_dir_all(root).unwrap();
        let mut bindings = BTreeMap::new();
        let mut rows = BTreeMap::new();
        let selected = crate::PublicCaptureInputPaths::runtime(root);
        for (path, _) in selected.selected_paths() {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"{}").unwrap();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            let digest = Digest256::of_bytes(b"{}").to_hex();
            bindings.insert(relative.clone(), digest.clone());
            rows.insert(
                format!("data/{relative}"),
                json!({"path": format!("data/{relative}"),
                "size_bytes": 2, "sha256": digest}),
            );
        }
        let declaration = runtime_data_declaration().unwrap();
        let output = declaration["compiled_subjects"][0]["output_path"]
            .as_str()
            .unwrap();
        let excluded = SelectedExcludedSourceMember {
            path: format!("data/{output}"),
            sha256: Digest256::of_bytes(b"excluded old SQL").to_hex(),
            size_bytes: 100,
        };
        rows.insert(
            excluded.path.clone(),
            json!({"path": excluded.path,
            "sha256": excluded.sha256, "size_bytes": excluded.size_bytes}),
        );
        // No excluded SQL payload exists: census must retain only its exact metadata.
        let corpus_revision = Digest256::of_bytes(b"fixture corpus").to_hex();
        let data_revision = Digest256::of_bytes(b"fixture data").to_hex();
        let manifest = serde_json::to_vec(&json!({
            "schema_version": "tos_access_data_snapshot_v1", "corpus_revision": corpus_revision,
            "data_revision": data_revision, "input_bindings": bindings,
            "compiler": {"schema": "fixture", "compiler_version": "fixture",
                "compiler_sha256": Digest256::of_bytes(b"old producer").to_hex(),
                "compiler_paths": ["software/producer.rs"], "input_bindings": bindings},
            "members": rows.into_values().collect::<Vec<_>>()
        }))
        .unwrap();
        let manifest_path = root.parent().unwrap().join("explicit-manifest.json");
        fs::write(&manifest_path, &manifest).unwrap();
        NativeSelectedSnapshotProfile {
            schema_version: "tos_native_selected_snapshot_profile_v1".into(),
            runtime_data_root: root.to_str().unwrap().into(),
            manifest_path: manifest_path.to_str().unwrap().into(),
            manifest_sha256: Digest256::of_bytes(&manifest).to_hex(),
            corpus_revision,
            data_revision,
            runtime_data_declaration_sha256: Digest256::of_bytes(RUNTIME_DATA_DECLARATION).to_hex(),
            evidence_scenes_sha256: Digest256::of_bytes(EVIDENCE_SCENES_SOURCE).to_hex(),
            excluded_compiled_model: excluded,
        }
    }

    #[test]
    fn selected_snapshot_relocation_uses_authenticated_census_and_exact_exclusion() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let a = fixture(&first.path().join("selected-data"));
        let b = fixture(&second.path().join("other-explicit-data"));
        assert_eq!(a.manifest_sha256, b.manifest_sha256);
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let mut left = census_selected_runtime_closure(&a, deadline).unwrap();
        let right = census_selected_runtime_closure(&b, deadline).unwrap();
        assert_eq!(left.members, right.members);
        assert_eq!(left.member_bytes(), right.member_bytes());
        assert_eq!(left.member_count(), left.source_bindings().len());
        left.recheck_manifest(deadline).unwrap();
        let mut wrong = b.clone();
        wrong.manifest_sha256 = Digest256::of_bytes(b"wrong manifest").to_hex();
        assert!(census_selected_runtime_closure(&wrong, deadline).is_err());
        wrong = b.clone();
        wrong.excluded_compiled_model.sha256 = Digest256::of_bytes(b"wrong SQL identity").to_hex();
        assert!(census_selected_runtime_closure(&wrong, deadline).is_err());
        wrong = b.clone();
        wrong.excluded_compiled_model.path = "data/other.sqlite3".into();
        assert!(census_selected_runtime_closure(&wrong, deadline).is_err());
        fs::write(&b.manifest_path, b"{}").unwrap();
        assert!(census_selected_runtime_closure(&b, deadline).is_err());
    }
}
