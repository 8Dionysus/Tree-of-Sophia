//! Bounded producer inputs and writer for the existing ManagedLocal native
//! data-manifest law.
//!
//! This emits mechanics and provenance only. It does not admit source, rights,
//! canon, semantics, publication, or a current release. The host still owns
//! semantic admission and the outer private pair CAS.
use crate::{COMPILER_VERSION, Error, NativeKnowledgeSelection, Result, SourceBinding};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
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
/// Reserved legacy-compatible input-binding key for the full source/software
/// profile. It is metadata, not a packaged file; the current native reader
/// already accepts non-member bindings for the runtime declaration and scenes.
pub const NATIVE_SOURCE_PROFILE_BINDING_PATH: &str = "native/source-profile-v1";

const EXECUTING_IMAGE_LOGICAL_PATH: &str = "tos-command/current-executable";
// These exact producer code and producer-config inputs are embedded into the
// running image. The held executing ELF hash separately fingerprints the
// complete built image; this selected set is intentionally incomplete and is
// not a reproducible-source or full dependency-closure claim.
const EMBEDDED_PRODUCER_INPUTS: [(&str, &[u8]); 22] = [
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
        "rust/crates/tos-compiler/src/source_bibliographic.rs",
        include_bytes!("source_bibliographic.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/source_corpus.rs",
        include_bytes!("source_corpus.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/source_navigation_source.rs",
        include_bytes!("source_navigation_source.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/source_witness_catalog.rs",
        include_bytes!("source_witness_catalog.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/knowledge_canon_source.rs",
        include_bytes!("knowledge_canon_source.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/knowledge_repository_source.rs",
        include_bytes!("knowledge_repository_source.rs"),
    ),
    (
        "rust/crates/tos-compiler/src/epistemic_evidence.rs",
        include_bytes!("epistemic_evidence.rs"),
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
        "rust/crates/tos-command/src/source_corpus_index_projection.rs",
        include_bytes!("../../tos-command/src/source_corpus_index_projection.rs"),
    ),
    (
        "rust/crates/tos-ops-mechanics-plan/src/philosophy_products.rs",
        include_bytes!("../../tos-ops-mechanics-plan/src/philosophy_products.rs"),
    ),
    (
        "rust/crates/tos-ops-mechanics-plan/src/philosophy_graph_views.rs",
        include_bytes!("../../tos-ops-mechanics-plan/src/philosophy_graph_views.rs"),
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
const MAX_CACHE_TREE_DEPTH: usize = 64;
const MAX_CACHE_TREE_ENTRIES: usize = 65_536;
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

/// One path-ordered row from the authenticated native source cut. Unlike the
/// historical data-snapshot profile below, these rows describe current source
/// membership directly and never name or exclude a Python-compiled database.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSourceMemberBinding {
    pub path: String,
    pub mode: u32,
    pub size_bytes: u64,
    pub sha256: String,
}

/// Explicit identity of a selected software capture and its exact producer
/// components. These are provenance inputs, not code admission.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSelectedSoftwareBinding {
    pub source_git_commit: String,
    pub source_git_tree: String,
    pub capture_manifest_sha256: String,
    pub components: Vec<NativeSourceMemberBinding>,
    /// Captured source path and digest for the worker's authored implementation.
    pub schema_worker_path: String,
    pub schema_worker_sha256: String,
    /// Digest of the separately selected and held executable image.
    pub schema_worker_image_sha256: String,
}

impl NativeSelectedSoftwareBinding {
    pub fn from_capture(
        reader: &tos_source_store::SoftwareCaptureReader,
        components: &tos_source_store::SoftwareComponentSelectionV1,
        schema_worker_path: &str,
        schema_worker_image_sha256: &str,
    ) -> Result<Self> {
        if components.capture() != reader.selection() {
            return Err(Error::Invalid(
                "native selected software component capture differs",
            ));
        }
        let worker_path = RelativePath::parse(schema_worker_path)
            .map_err(|_| Error::Invalid("native selected schema worker path invalid"))?;
        let members = components
            .members()
            .map(|member| NativeSourceMemberBinding {
                path: member.path.as_str().to_owned(),
                mode: member.mode,
                size_bytes: member.size_bytes,
                sha256: member.sha256.to_hex(),
            })
            .collect::<Vec<_>>();
        let worker_sha256 = members
            .iter()
            .find(|member| member.path == worker_path.as_str())
            .map(|member| member.sha256.clone())
            .ok_or(Error::Invalid(
                "native selected schema worker is not a captured component",
            ))?;
        let worker_image_sha256 = Digest256::from_hex(schema_worker_image_sha256)
            .map_err(|_| Error::Invalid("native selected schema worker image digest invalid"))?
            .to_hex();
        Ok(Self {
            source_git_commit: reader.selection().source_git_commit.clone(),
            source_git_tree: reader.selection().source_git_tree.clone(),
            capture_manifest_sha256: reader.selection().capture_manifest_sha256.to_hex(),
            components: members,
            schema_worker_path: worker_path.as_str().to_owned(),
            schema_worker_sha256: worker_sha256,
            schema_worker_image_sha256: worker_image_sha256,
        })
    }
}

/// In-memory source-only profile derived from an opened current CorpusCut and
/// one selected software capture. Its canonical digest covers exact ordered
/// source membership, software capture/component identity, captured worker source,
/// and separately selected executable image; the SourceBinding is supplied to
/// the source projection owners.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSelectedRuntimeSourceProfile {
    pub schema_version: String,
    pub source_revision: String,
    pub membership_root: String,
    pub member_count: u64,
    pub source_bytes: u64,
    pub members: Vec<NativeSourceMemberBinding>,
    pub software: NativeSelectedSoftwareBinding,
    /// Exact fixed runtime products produced from the selected source cut and
    /// software capture. Empty is permitted only while composing in memory;
    /// the snapshot writer requires the complete fixed set.
    pub products: Vec<NativeSourceMemberBinding>,
    pub profile_sha256: String,
}

pub fn required_native_runtime_product_paths() -> [&'static str; 6] {
    [
        CORPUS_INDEX_PATH,
        crate::source_philosophy_views::ATLAS_REF,
        crate::source_philosophy_graph::VIEWS_REF,
        PHILOSOPHY_GRAPH_PATH,
        CLAIM_GRAPH_PATH,
        EVIDENCE_PROJECTION_PATH,
    ]
}

fn source_profile_digest(
    source_revision: &str,
    membership_root: &str,
    source_bytes: u64,
    members: &[NativeSourceMemberBinding],
    software: &NativeSelectedSoftwareBinding,
    products: &[NativeSourceMemberBinding],
) -> Result<String> {
    let value = serde_json::json!({
        "schema_version": NativeSelectedRuntimeSourceProfile::SCHEMA_VERSION,
        "source_revision": source_revision,
        "membership_root": membership_root,
        "member_count": members.len() as u64,
        "source_bytes": source_bytes,
        "members": members,
        "software": software,
        "products": products,
    });
    let raw = canonical_value(&value, MAX_MANIFEST_BYTES)?;
    Ok(Digest256::of_bytes(&raw).to_hex())
}

impl NativeSelectedRuntimeSourceProfile {
    pub const SCHEMA_VERSION: &'static str = "tos_native_selected_runtime_source_profile_v1";
    pub const OWNER_PROFILE: &'static str = "tos-native-selected-runtime-source-v1";
    pub const ROUTE_MAP_VERSION: &'static str = "native-source-cut-v1";
    pub const READER_ABI: &'static str = "tos-source-store-cut-v1";

    /// Derive the profile from the already opened current cut. The caller must
    /// separately retain and recheck the same cut/software readers throughout
    /// projection and capture; a profile value alone is never authority.
    pub fn from_current_cut(
        cut: &tos_source_store::CorpusCutReader,
        software: NativeSelectedSoftwareBinding,
    ) -> Result<Self> {
        let snapshot = cut.current();
        let revision = snapshot.revision();
        let members = snapshot
            .members()
            .map(|member| NativeSourceMemberBinding {
                path: member.path.as_str().to_owned(),
                mode: member.mode,
                size_bytes: member.size_bytes,
                sha256: member.sha256.to_hex(),
            })
            .collect::<Vec<_>>();
        let expected = cut
            .stream(revision)
            .map_err(|error| Error::Source(error.to_string()))?
            .expectation();
        let profile = Self::new(
            revision.0.to_hex(),
            expected.digest.to_hex(),
            members,
            software,
        )?;
        if profile.member_count != expected.count {
            return Err(Error::Invalid(
                "native current source membership count differs",
            ));
        }
        Ok(profile)
    }

    pub fn new(
        source_revision: String,
        membership_root: String,
        members: Vec<NativeSourceMemberBinding>,
        software: NativeSelectedSoftwareBinding,
    ) -> Result<Self> {
        sha_text(&source_revision, "native source revision invalid")?;
        sha_text(&membership_root, "native source membership root invalid")?;
        if members.is_empty() || members.len() > MAX_DATA_MEMBERS {
            return Err(Error::Budget("native selected source member count"));
        }
        validate_source_member_rows(&members)?;
        if members
            .iter()
            .any(|member| member.path == NATIVE_SOURCE_PROFILE_BINDING_PATH)
        {
            return Err(Error::Invalid(
                "native selected source collides with reserved profile binding",
            ));
        }
        if software.components.is_empty() || software.components.len() > 128 {
            return Err(Error::Budget("native selected software component count"));
        }
        validate_source_member_rows(&software.components)?;
        for object_id in [&software.source_git_commit, &software.source_git_tree] {
            git_object_id(object_id, "native selected software Git identity invalid")?;
        }
        sha_text(
            &software.capture_manifest_sha256,
            "native selected software manifest digest invalid",
        )?;
        relative(
            &software.schema_worker_path,
            "native selected schema worker path invalid",
        )?;
        sha_text(
            &software.schema_worker_sha256,
            "native selected schema worker source digest invalid",
        )?;
        sha_text(
            &software.schema_worker_image_sha256,
            "native selected schema worker image digest invalid",
        )?;
        if !software.components.iter().any(|component| {
            component.path == software.schema_worker_path
                && component.sha256 == software.schema_worker_sha256
        }) {
            return Err(Error::Invalid(
                "native selected schema worker is not a selected component",
            ));
        }

        let (observed_root, source_bytes) = source_membership(&members)?;
        if observed_root != membership_root {
            return Err(Error::Invalid("native source membership root differs"));
        }
        let products = Vec::new();
        let profile_sha256 = source_profile_digest(
            &source_revision,
            &membership_root,
            source_bytes,
            &members,
            &software,
            &products,
        )?;
        Ok(Self {
            schema_version: Self::SCHEMA_VERSION.to_owned(),
            source_revision,
            membership_root,
            member_count: members.len() as u64,
            source_bytes,
            members,
            software,
            products,
            profile_sha256,
        })
    }

    pub fn with_runtime_products(
        mut self,
        products: Vec<NativeSourceMemberBinding>,
    ) -> Result<Self> {
        validate_source_member_rows(&products)?;
        let required = required_native_runtime_product_paths();
        if products.len() != required.len()
            || required.iter().any(|path| {
                products
                    .binary_search_by(|member| member.path.as_str().cmp(path))
                    .is_err()
            })
        {
            return Err(Error::Invalid("native runtime product set differs"));
        }
        self.profile_sha256 = source_profile_digest(
            &self.source_revision,
            &self.membership_root,
            self.source_bytes,
            &self.members,
            &self.software,
            &products,
        )?;
        self.products = products;
        Ok(self)
    }

    pub fn source_binding(&self) -> SourceBinding {
        SourceBinding {
            owner_profile: Self::OWNER_PROFILE.to_owned(),
            source_cut: format!("native-source:{}", self.source_revision),
            through_commit_seq: 0,
            membership_root: self.membership_root.clone(),
            index_generation: self.source_revision.clone(),
            route_map_version: Self::ROUTE_MAP_VERSION.to_owned(),
            reader_abi: Self::READER_ABI.to_owned(),
            projection_root_sha256: self.profile_sha256.clone(),
            complete: true,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let expected = Self::new(
            self.source_revision.clone(),
            self.membership_root.clone(),
            self.members.clone(),
            self.software.clone(),
        )?;
        let expected_digest = source_profile_digest(
            &self.source_revision,
            &self.membership_root,
            self.source_bytes,
            &self.members,
            &self.software,
            &self.products,
        )?;
        if expected.schema_version != self.schema_version
            || expected.member_count != self.member_count
            || expected.source_bytes != self.source_bytes
            || expected_digest != self.profile_sha256
        {
            return Err(Error::Invalid(
                "native selected source profile digest differs",
            ));
        }
        if !self.products.is_empty() {
            validate_source_member_rows(&self.products)?;
            let required = required_native_runtime_product_paths();
            if self.products.len() != required.len()
                || required.iter().any(|path| {
                    self.products
                        .binary_search_by(|member| member.path.as_str().cmp(path))
                        .is_err()
                })
            {
                return Err(Error::Invalid("native runtime product set differs"));
            }
        }
        Ok(())
    }

    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        let row = std::mem::size_of::<NativeSourceMemberBinding>() + MAX_MEMBER_PATH_BYTES + 64;
        let components = self
            .software
            .components
            .len()
            .checked_mul(row)
            .ok_or(Error::Budget("native selected software retained rows"))?;
        std::mem::size_of::<Self>()
            .checked_add(self.schema_version.capacity())
            .and_then(|n| n.checked_add(self.source_revision.capacity()))
            .and_then(|n| n.checked_add(self.membership_root.capacity()))
            .and_then(|n| n.checked_add(self.members.len().checked_mul(row)?))
            .and_then(|n| n.checked_add(components))
            .and_then(|n| n.checked_add(self.software.source_git_commit.capacity()))
            .and_then(|n| n.checked_add(self.software.source_git_tree.capacity()))
            .and_then(|n| n.checked_add(self.software.capture_manifest_sha256.capacity()))
            .and_then(|n| n.checked_add(self.software.schema_worker_path.capacity()))
            .and_then(|n| n.checked_add(self.software.schema_worker_sha256.capacity()))
            .and_then(|n| n.checked_add(self.software.schema_worker_image_sha256.capacity()))
            .and_then(|n| n.checked_add(self.products.len().checked_mul(row)?))
            .and_then(|n| n.checked_add(self.profile_sha256.capacity()))
            .ok_or(Error::Budget("native selected source profile state"))
    }
}

fn validate_source_member_rows(rows: &[NativeSourceMemberBinding]) -> Result<()> {
    let mut previous: Option<&str> = None;
    let mut path_bytes = 0usize;
    for row in rows {
        relative(&row.path, "native selected source path invalid")?;
        if row.path.len() > MAX_MEMBER_PATH_BYTES
            || !row.path.is_ascii()
            || row.mode & !0o777 != 0
            || row.mode & 0o600 == 0
            || row.sha256.len() != 64
            || previous.is_some_and(|prior| prior >= row.path.as_str())
        {
            return Err(Error::Invalid("native selected source row invalid"));
        }
        sha_text(&row.sha256, "native selected source row digest invalid")?;
        path_bytes = path_bytes
            .checked_add(row.path.len())
            .filter(|value| *value <= MAX_BINDING_PATH_TOTAL_BYTES)
            .ok_or(Error::Budget("native selected source path aggregate"))?;
        previous = Some(&row.path);
    }
    Ok(())
}

fn source_membership(rows: &[NativeSourceMemberBinding]) -> Result<(String, u64)> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-val-full-membership-v1\0");
    let mut bytes = 0u64;
    for row in rows {
        let digest = Digest256::from_hex(&row.sha256)
            .map_err(|_| Error::Invalid("native source row digest invalid"))?;
        hash.update(&(row.path.len() as u64).to_be_bytes());
        hash.update(row.path.as_bytes());
        hash.update(&row.size_bytes.to_be_bytes());
        hash.update(digest.as_bytes());
        bytes = bytes
            .checked_add(row.size_bytes)
            .filter(|value| *value <= MAX_CAPTURE_CLOSURE_BYTES)
            .ok_or(Error::Budget("native selected source byte ceiling"))?;
    }
    Ok((hash.finalize().to_hex(), bytes))
}

fn git_object_id(value: &str, label: &'static str) -> Result<()> {
    if !matches!(value.len(), 40 | 64)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::Invalid(label));
    }
    Ok(())
}

fn absolute_normalized(value: &str, label: &'static str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty()
        || value.len() > MAX_MEMBER_PATH_BYTES
        || !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(Error::Invalid(label));
    }
    Ok(())
}

/// Explicit prior native data candidate selected only as a model cache. The
/// manifest digest and revision bind the caller's chosen artifact; they do not
/// grant source or release authority.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePreviousDataSnapshotProfile {
    pub schema_version: String,
    pub root: String,
    pub manifest_sha256: String,
    pub data_revision: String,
}
impl NativePreviousDataSnapshotProfile {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != "tos_native_previous_data_snapshot_profile_v1"
            || self.root.len() > MAX_MEMBER_PATH_BYTES
            || !Path::new(&self.root).is_absolute()
            || Path::new(&self.root)
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        {
            return Err(Error::Invalid("previous native data snapshot profile"));
        }
        sha_text(&self.manifest_sha256, "previous native manifest digest")?;
        sha_text(&self.data_revision, "previous native data revision")
    }

    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        [
            &self.schema_version,
            &self.root,
            &self.manifest_sha256,
            &self.data_revision,
        ]
        .into_iter()
        .try_fold(std::mem::size_of::<Self>(), |bytes, text| {
            bytes
                .checked_add(text.capacity())
                .ok_or(Error::Budget("previous native profile retained state"))
        })
    }
}

/// A prior native model whose complete data tree, canonical manifest,
/// compiler/source identity, selection packet and member hashes were checked.
/// The caller must still cold-open the copied model under the current owner.
pub struct VerifiedNativeDataCache {
    model_path: PathBuf,
    model_sha256: String,
    model_size_bytes: u64,
    selection: NativeKnowledgeSelection,
    selection_source_bytes: usize,
}
impl VerifiedNativeDataCache {
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }
    pub fn model_sha256(&self) -> &str {
        &self.model_sha256
    }
    pub fn model_size_bytes(&self) -> u64 {
        self.model_size_bytes
    }
    pub fn selection(&self) -> &NativeKnowledgeSelection {
        &self.selection
    }

    /// Bound typed selection state from its source packet. Small JSON packets
    /// can expand into many owned strings and tree nodes after decoding.
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        let decoded = self
            .selection_source_bytes
            .checked_mul(32)
            .ok_or(Error::Budget("native cache selection state"))?;
        std::mem::size_of::<Self>()
            .checked_add(self.model_path.as_os_str().len())
            .and_then(|n| n.checked_add(self.model_sha256.capacity()))
            .and_then(|n| n.checked_add(decoded))
            .ok_or(Error::Budget("native cache retained state"))
    }
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
    // A source row may contribute an intermediate route even when the final
    // catalog contains only a few facet/overview values. Bound that SQL state
    // separately; retain the published output entry and byte ceilings.
    base.catalog.max_aggregate_entries = base.catalog.max_rows;
    base.catalog.max_aggregate_bytes =
        usize::try_from(base.stage.max_temp_bytes.min(256 * 1024 * 1024))
            .map_err(|_| Error::Budget("native catalog aggregate bytes"))?;
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
    /// Exactly one source choice is active. The historical reader remains
    /// byte-for-byte distinct from the source-only current-cut profile.
    pub selected_profile: Option<&'a NativeSelectedSnapshotProfile>,
    pub selected_census: Option<&'a NativeSelectedSnapshotCensus>,
    pub selected_runtime_source: Option<&'a NativeSelectedRuntimeSourceProfile>,
    pub source_only_census: Option<&'a NativeSourceOnlySnapshotCensus>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceTreeMember {
    mode: u32,
    size_bytes: u64,
    sha256: String,
}

/// Held exact source-only runtime closure census. The full profile authenticates
/// the source cut and selected software; this smaller table is the exact
/// allowlisted PublicCapture closure plus native products and runtime companions.
pub struct NativeSourceOnlySnapshotCensus {
    profile: NativeSelectedRuntimeSourceProfile,
    members: BTreeMap<String, SourceTreeMember>,
    source_bindings: BTreeMap<String, String>,
    member_bytes: u64,
}

fn native_runtime_projection_path(path: &str) -> bool {
    matches!(
        path,
        CORPUS_INDEX_PATH
            | PHILOSOPHY_GRAPH_PATH
            | CLAIM_GRAPH_PATH
            | EVIDENCE_PROJECTION_PATH
            | crate::source_philosophy_views::ATLAS_REF
            | crate::source_philosophy_graph::VIEWS_REF
    )
}

impl NativeSourceOnlySnapshotCensus {
    pub fn open(
        profile: NativeSelectedRuntimeSourceProfile,
        capture: &crate::PublicCapture,
        deadline: Instant,
    ) -> Result<Self> {
        profile.validate()?;
        let required_products = required_native_runtime_product_paths();
        if profile.products.len() != required_products.len() {
            return Err(Error::Invalid("native runtime product profile absent"));
        }
        let product_rows = profile
            .products
            .iter()
            .map(|member| (member.path.as_str(), member))
            .collect::<BTreeMap<_, _>>();
        let profile_members = profile
            .members
            .iter()
            .map(|member| (member.path.as_str(), member))
            .collect::<BTreeMap<_, _>>();
        let mut members = BTreeMap::new();
        let mut source_bindings = BTreeMap::new();
        let mut member_bytes = 0u64;
        for (path, digest, size_bytes) in capture.retained_input_members()? {
            active(deadline)?;
            relative(&path, "native source-only captured path invalid")?;
            if path == NATIVE_SOURCE_PROFILE_BINDING_PATH {
                return Err(Error::Invalid(
                    "native captured source collides with reserved profile binding",
                ));
            }
            let runtime = crate::d1_public_capture::runtime_companions()
                .find(|(selected, _)| *selected == path.as_str())
                .map(|(_, raw)| raw);
            let generated = native_runtime_projection_path(&path);
            if generated {
                let product = product_rows
                    .get(path.as_str())
                    .ok_or(Error::Invalid("native runtime product not selected"))?;
                if product.sha256 != digest.to_hex() || product.size_bytes != size_bytes {
                    return Err(Error::Invalid("native runtime product digest differs"));
                }
            }
            if let Some(original) = profile_members.get(path.as_str()) {
                if !generated
                    && (original.sha256 != digest.to_hex() || original.size_bytes != size_bytes)
                {
                    return Err(Error::Invalid("native source-only cut member changed"));
                }
            } else if !generated && runtime.is_none() {
                return Err(Error::Invalid(
                    "native source-only closure includes an unselected source",
                ));
            }
            if let Some(raw) = runtime
                && (Digest256::of_bytes(raw) != digest || raw.len() as u64 != size_bytes)
            {
                return Err(Error::Invalid("native runtime companion differs"));
            }
            member_bytes = member_bytes
                .checked_add(size_bytes)
                .filter(|bytes| *bytes <= MAX_CAPTURE_CLOSURE_BYTES)
                .ok_or(Error::Budget("native source-only closure bytes"))?;
            if members
                .insert(
                    path.clone(),
                    SourceTreeMember {
                        mode: 0,
                        size_bytes,
                        sha256: digest.to_hex(),
                    },
                )
                .is_some()
            {
                return Err(Error::Invalid("native source-only closure path duplicate"));
            }
            source_bindings.insert(path, digest.to_hex());
            if members.len() > MAX_DATA_MEMBERS {
                return Err(Error::Budget("native source-only closure members"));
            }
        }
        if members.is_empty()
            || required_products
                .iter()
                .any(|path| !members.contains_key(*path))
        {
            return Err(Error::Invalid("native source-only runtime product absent"));
        }
        source_bindings.insert(
            RUNTIME_DATA_DECLARATION_PATH.to_owned(),
            Digest256::of_bytes(RUNTIME_DATA_DECLARATION).to_hex(),
        );
        source_bindings.insert(
            EVIDENCE_SCENES_PATH.to_owned(),
            Digest256::of_bytes(EVIDENCE_SCENES_SOURCE).to_hex(),
        );
        source_bindings.insert(
            NATIVE_SOURCE_PROFILE_BINDING_PATH.to_owned(),
            profile.profile_sha256.clone(),
        );
        Ok(Self {
            profile,
            members,
            source_bindings,
            member_bytes,
        })
    }

    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        let node = 11 * std::mem::size_of::<(String, SourceTreeMember)>()
            + 16 * std::mem::size_of::<usize>();
        let binding_node =
            11 * std::mem::size_of::<(String, String)>() + 16 * std::mem::size_of::<usize>();
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.profile.retained_state_upper_bound()?)
            .and_then(|n| n.checked_add(self.members.len().checked_mul(node)?))
            .and_then(|n| n.checked_add(self.source_bindings.len().checked_mul(binding_node)?))
            .ok_or(Error::Budget("native source-only census retained slots"))?;
        for (path, member) in &self.members {
            bytes = bytes
                .checked_add(path.capacity())
                .and_then(|n| n.checked_add(member.sha256.capacity()))
                .ok_or(Error::Budget("native source-only census retained strings"))?;
        }
        for (path, digest) in &self.source_bindings {
            bytes = bytes
                .checked_add(path.capacity())
                .and_then(|n| n.checked_add(digest.capacity()))
                .ok_or(Error::Budget("native source-only binding strings"))?;
        }
        Ok(bytes)
    }

    pub fn source_bindings(&self) -> &BTreeMap<String, String> {
        &self.source_bindings
    }
    pub fn profile(&self) -> &NativeSelectedRuntimeSourceProfile {
        &self.profile
    }
    pub fn profile_sha256(&self) -> &str {
        &self.profile.profile_sha256
    }
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
    pub fn member_bytes(&self) -> u64 {
        self.member_bytes
    }

    /// Recheck the exact captured closure after owner generation. The capture
    /// owns held source descriptors and rejects both byte drift and name drift.
    pub fn recheck_capture(&self, capture: &crate::PublicCapture, deadline: Instant) -> Result<()> {
        self.profile.validate()?;
        let current = Self::open(self.profile.clone(), capture, deadline)?;
        if current.members != self.members
            || current.source_bindings != self.source_bindings
            || current.member_bytes != self.member_bytes
        {
            return Err(Error::Invalid(
                "native source-only capture changed after census",
            ));
        }
        Ok(())
    }

    pub fn validate_capture_closure(
        &self,
        capture: &crate::PublicCapture,
        deadline: Instant,
    ) -> Result<Vec<NativeCapturedMember>> {
        self.recheck_capture(capture, deadline)?;
        let mut members = self
            .members
            .iter()
            .map(|(source_path, member)| NativeCapturedMember {
                source_path: source_path.clone(),
                sha256: member.sha256.clone(),
                size_bytes: member.size_bytes,
            })
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        Ok(members)
    }
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

fn scan_native_source_tree(
    root: &Path,
    deadline: Instant,
) -> Result<(BTreeMap<String, SourceTreeMember>, u64)> {
    private_directory(root)?;
    let root_metadata = fs::symlink_metadata(root)?;
    let mut members = BTreeMap::new();
    let mut entry_count = 0usize;
    let mut total_bytes = 0u64;
    scan_native_source_tree_at(
        root,
        root,
        root_metadata.uid(),
        &mut members,
        &mut entry_count,
        &mut total_bytes,
        0,
        deadline,
    )?;
    if members.is_empty() {
        return Err(Error::Invalid("native source-only tree is empty"));
    }
    Ok((members, total_bytes))
}

#[allow(clippy::too_many_arguments)]
fn scan_native_source_tree_at(
    root: &Path,
    directory: &Path,
    uid: u32,
    members: &mut BTreeMap<String, SourceTreeMember>,
    entry_count: &mut usize,
    total_bytes: &mut u64,
    depth: usize,
    deadline: Instant,
) -> Result<()> {
    active(deadline)?;
    if depth > MAX_CACHE_TREE_DEPTH {
        return Err(Error::Budget("native source-only tree depth"));
    }
    for entry in fs::read_dir(directory)? {
        active(deadline)?;
        *entry_count = (*entry_count)
            .checked_add(1)
            .filter(|count| *count <= MAX_CACHE_TREE_ENTRIES)
            .ok_or(Error::Budget("native source-only tree entry count"))?;
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.uid() != uid || metadata.file_type().is_symlink() {
            return Err(Error::Invalid("native source-only tree custody"));
        }
        if metadata.is_dir() {
            scan_native_source_tree_at(
                root,
                &path,
                uid,
                members,
                entry_count,
                total_bytes,
                depth + 1,
                deadline,
            )?;
            continue;
        }
        if !metadata.is_file() {
            return Err(Error::Invalid("native source-only tree special file"));
        }
        if members.len() >= MAX_DATA_MEMBERS {
            return Err(Error::Budget("native source-only tree member count"));
        }
        let relative_path = path
            .strip_prefix(root)
            .map_err(|_| Error::Invalid("native source-only tree escaped root"))?
            .to_str()
            .ok_or(Error::Invalid("native source-only path is not UTF8"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if relative_path.len() > MAX_MEMBER_PATH_BYTES || !relative_path.is_ascii() {
            return Err(Error::Budget("native source-only path bytes"));
        }
        relative(&relative_path, "native source-only relative path invalid")?;
        let (size_bytes, digest) = read_digest(&path, MAX_CAPTURE_MEMBER_BYTES as u64, deadline)?;
        *total_bytes = total_bytes
            .checked_add(size_bytes)
            .filter(|bytes| *bytes <= MAX_CAPTURE_CLOSURE_BYTES)
            .ok_or(Error::Budget("native source-only tree byte ceiling"))?;
        if members
            .insert(
                relative_path,
                SourceTreeMember {
                    mode: metadata.mode() & 0o777,
                    size_bytes,
                    sha256: digest.to_hex(),
                },
            )
            .is_some()
        {
            return Err(Error::Invalid("native source-only duplicate member"));
        }
    }
    Ok(())
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
    let selected_source_bindings = match (
        input.selected_profile,
        input.selected_census,
        input.selected_runtime_source,
        input.source_only_census,
    ) {
        (Some(profile), Some(census), None, None) => {
            profile.validate()?;
            if &census.profile != profile
                || census.manifest_sha256 != profile.manifest_sha256
                || census.source_root != Path::new(&profile.runtime_data_root)
                || census.manifest_path != Path::new(&profile.manifest_path)
            {
                return Err(Error::Invalid(
                    "selected manifest census/profile binding differs",
                ));
            }
            if input.corpus_revision != profile.corpus_revision {
                return Err(Error::Invalid("historical corpus revision changed"));
            }
            &census.source_bindings
        }
        (None, None, Some(profile), Some(census)) => {
            profile.validate()?;
            if &census.profile != profile
                || input.corpus_revision != profile.source_revision
                || census
                    .source_bindings
                    .get(NATIVE_SOURCE_PROFILE_BINDING_PATH)
                    != Some(&profile.profile_sha256)
            {
                return Err(Error::Invalid(
                    "selected source-only census/profile binding differs",
                ));
            }
            &census.source_bindings
        }
        _ => {
            return Err(Error::Invalid(
                "exactly one selected native source profile is required",
            ));
        }
    };
    if !matches!(
        input.model_abi,
        crate::KNOWLEDGE_CORPUS_MODEL_ABI
            | crate::knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI
            | tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1
            | tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2
    ) || input.model_abi.len() > 256
    {
        return Err(Error::Invalid("native manifest model ABI"));
    }
    validate_map(
        input.source_bindings,
        "native manifest source binding invalid",
        MAX_DATA_MEMBERS + 3,
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
        if source == RUNTIME_DATA_DECLARATION_PATH
            || source == EVIDENCE_SCENES_PATH
            || source == NATIVE_SOURCE_PROFILE_BINDING_PATH
        {
            continue;
        }
        let member = format!("data/{source}");
        if !paths.iter().any(|path| path == &member) {
            return Err(Error::Invalid("native source binding lacks data member"));
        }
    }
    for (path, expected) in selected_source_bindings {
        if input.source_bindings.get(path) != Some(expected) {
            return Err(Error::Invalid("selected source root binding changed"));
        }
    }
    if !selected_source_bindings.contains_key(CORPUS_INDEX_PATH) {
        return Err(Error::Invalid("selected corpus root binding absent"));
    }
    let evidence_scenes_sha256 = match input.selected_profile {
        Some(profile) => profile.evidence_scenes_sha256.clone(),
        None => Digest256::of_bytes(EVIDENCE_SCENES_SOURCE).to_hex(),
    };
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
            != Some(evidence_scenes_sha256.as_str())
        || Digest256::of_bytes(EVIDENCE_SCENES_SOURCE).to_hex() != evidence_scenes_sha256
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

fn read_bounded(path: &Path, cap: u64, deadline: Instant) -> Result<Vec<u8>> {
    no_symlink_path(path)?;
    let mut file = crate::safe_open::open_regular(path, cap)?;
    let before = file_stamp(path, &file)?;
    if before.size > cap {
        return Err(Error::Budget("native cache member byte cap"));
    }
    let capacity = usize::try_from(before.size)
        .map_err(|_| Error::Budget("native cache member address space"))?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut block = [0u8; 64 * 1024];
    loop {
        active(deadline)?;
        let count = file.read(&mut block)?;
        if count == 0 {
            break;
        }
        if bytes
            .len()
            .checked_add(count)
            .map_or(true, |next| next > capacity)
        {
            return Err(Error::Budget("native cache member byte cap"));
        }
        bytes.extend_from_slice(&block[..count]);
    }
    if file_stamp(path, &file)? != before || bytes.len() as u64 != before.size {
        return Err(Error::Invalid("native cache member changed while reading"));
    }
    Ok(bytes)
}

fn exact_object_keys(value: &Value, expected: &[&str], label: &'static str) -> Result<()> {
    let object = value.as_object().ok_or(Error::Invalid(label))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error::Invalid(label));
    }
    Ok(())
}

fn scan_cache_tree(
    root: &Path,
    directory: &Path,
    declared: &BTreeMap<String, MemberDigest>,
    found_files: &mut BTreeSet<String>,
    found_directories: &mut BTreeSet<String>,
    entries_seen: &mut usize,
    depth: usize,
    uid: u32,
    deadline: Instant,
) -> Result<()> {
    active(deadline)?;
    if depth > MAX_CACHE_TREE_DEPTH {
        return Err(Error::Budget("native cache tree depth"));
    }
    for entry in fs::read_dir(directory)? {
        active(deadline)?;
        *entries_seen = (*entries_seen)
            .checked_add(1)
            .filter(|count| *count <= MAX_CACHE_TREE_ENTRIES)
            .ok_or(Error::Budget("native cache tree entry count"))?;
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Error::Invalid("native cache tree escaped root"))?
            .to_str()
            .ok_or(Error::Invalid("native cache path is not UTF8"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.mode() & 0o077 != 0
        {
            return Err(Error::Invalid("native cache private member custody"));
        }
        if metadata.is_dir() {
            if !relative.starts_with("data/") && relative != "data" {
                return Err(Error::Invalid("native cache unexpected directory"));
            }
            if !declared
                .keys()
                .any(|file| file.starts_with(&format!("{relative}/")))
                && relative != "data"
            {
                return Err(Error::Invalid("native cache empty or unexpected directory"));
            }
            found_directories.insert(relative);
            scan_cache_tree(
                root,
                &path,
                declared,
                found_files,
                found_directories,
                entries_seen,
                depth + 1,
                uid,
                deadline,
            )?;
        } else if metadata.is_file() {
            if relative != "data/manifest.json" && !declared.contains_key(&relative) {
                return Err(Error::Invalid("native cache undeclared member"));
            }
            found_files.insert(relative);
        } else {
            return Err(Error::Invalid("native cache special member"));
        }
    }
    Ok(())
}

fn native_cache_member_rows(value: &Value) -> Result<BTreeMap<String, MemberDigest>> {
    let rows = value
        .get("members")
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty() && rows.len() <= MAX_DATA_MEMBERS)
        .ok_or(Error::Invalid("native cache members"))?;
    let mut members = BTreeMap::new();
    let mut previous = String::new();
    let mut total = 0u64;
    for row in rows {
        exact_object_keys(
            row,
            &["path", "size_bytes", "sha256"],
            "native cache member row",
        )?;
        let path = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("native cache member path"))?;
        relative(path, "native cache member path")?;
        if !path.starts_with("data/") || path == "data/manifest.json" || path <= previous.as_str() {
            return Err(Error::Invalid("native cache member ordering"));
        }
        let size_bytes = row
            .get("size_bytes")
            .and_then(Value::as_u64)
            .ok_or(Error::Invalid("native cache member size"))?;
        let cap = match path {
            NATIVE_MODEL_PATH => MAX_NATIVE_MODEL_BYTES,
            NATIVE_SELECTION_PATH => MAX_MANIFEST_BYTES as u64,
            _ => MAX_CAPTURE_MEMBER_BYTES as u64,
        };
        if size_bytes > cap
            || matches!(path, NATIVE_MODEL_PATH | NATIVE_SELECTION_PATH) && size_bytes == 0
        {
            return Err(Error::Budget("native cache member size ceiling"));
        }
        let sha256 = row
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("native cache member digest"))?;
        sha_text(sha256, "native cache member digest")?;
        total = total
            .checked_add(size_bytes)
            .filter(|bytes| *bytes <= MAX_DATA_BYTES)
            .ok_or(Error::Budget("native cache total bytes"))?;
        previous = path.to_owned();
        members.insert(
            path.to_owned(),
            MemberDigest {
                path: path.to_owned(),
                size_bytes,
                sha256: sha256.to_owned(),
            },
        );
    }
    if !members.contains_key(NATIVE_MODEL_PATH) || !members.contains_key(NATIVE_SELECTION_PATH) {
        return Err(Error::Invalid("native cache model or selection absent"));
    }
    Ok(members)
}

fn native_cache_bindings(value: &Value) -> Result<BTreeMap<String, String>> {
    let bindings: BTreeMap<String, String> = serde_json::from_value(
        value
            .get("input_bindings")
            .cloned()
            .ok_or(Error::Invalid("native cache source bindings absent"))?,
    )
    .map_err(|_| Error::Invalid("native cache source bindings"))?;
    validate_map(
        &bindings,
        "native cache source bindings",
        MAX_DATA_MEMBERS,
        MAX_MEMBER_PATH_TOTAL_BYTES,
    )?;
    Ok(bindings)
}

fn verify_cache_selection_bindings(
    selection: &NativeKnowledgeSelection,
    members: &BTreeMap<String, MemberDigest>,
    bindings: &BTreeMap<String, String>,
) -> Result<()> {
    if selection.paths().model != NATIVE_MODEL_PATH
        || selection.paths().descriptor
            != "data/ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        || selection.paths().entity_registry
            != "data/ToS/doctrine/semantic-interchange/entity-types.v1.json"
        || selection.paths().relation_registry
            != "data/ToS/doctrine/semantic-interchange/relation-types.v1.json"
        || selection.producer().corpus_original.is_none()
        || selection.producer().philosophy_original.is_none()
        || selection.producer().managed_source.is_some()
        || selection.producer().managed_source_v2.is_some()
    {
        return Err(Error::Invalid("native cache selected Original pair"));
    }
    let model = members
        .get(NATIVE_MODEL_PATH)
        .ok_or(Error::Invalid("native cache model absent"))?;
    let expectation = selection.expectation();
    let stage = &selection.producer().stage;
    if expectation.model_sha256 != model.sha256
        || expectation.model_size_bytes != model.size_bytes
        || stage.sqlite_sha256 != model.sha256
        || stage.sqlite_size_bytes != model.size_bytes
        || stage.source_cut != expectation.source_cut
        || selection.producer().seal.model_abi != expectation.model_abi
    {
        return Err(Error::Invalid("native cache model receipt differs"));
    }
    let corpus = selection
        .producer()
        .corpus_original
        .as_ref()
        .ok_or(Error::Invalid("native cache corpus Original absent"))?;
    if corpus.source_cut != expectation.source_cut
        || corpus.origin.profile != "captured-runtime-projection-v1"
        || corpus.origin.source_path != CORPUS_INDEX_PATH
        || corpus.origin.source_git_commit.is_some()
        || corpus.origin.source_git_tree.is_some()
        || bindings.get(CORPUS_INDEX_PATH) != Some(&corpus.origin.source_sha256)
    {
        return Err(Error::Invalid("native cache corpus source binding differs"));
    }
    let top = members
        .get(&format!("data/{}", corpus.origin.source_path))
        .ok_or(Error::Invalid("native cache corpus source member absent"))?;
    if top.sha256 != corpus.origin.source_sha256
        || top.size_bytes != corpus.origin.source_size_bytes
    {
        return Err(Error::Invalid("native cache corpus top member differs"));
    }
    let mut seen = BTreeSet::new();
    for original in &corpus.origin.members {
        relative(&original.path, "native cache Original member path")?;
        if !seen.insert(original.path.as_str()) {
            return Err(Error::Invalid("native cache Original member duplicate"));
        }
        let member = members
            .get(&format!("data/{}", original.path))
            .ok_or(Error::Invalid("native cache Original member absent"))?;
        if member.sha256 != original.sha256 || member.size_bytes != original.size_bytes {
            return Err(Error::Invalid("native cache Original member differs"));
        }
    }
    Ok(())
}

/// Verify one explicitly selected persistent native artifact. A valid cache
/// with a different source/compiler/cut is a normal miss; malformed, replaced,
/// or internally inconsistent artifacts are refusals. On a hit, the caller
/// must copy and cold-open the exact selected model before publishing output.
pub fn verify_previous_native_data_snapshot(
    profile: &NativePreviousDataSnapshotProfile,
    input: &NativeDataSnapshotManifestInput<'_>,
    expected_source_revision: &str,
    limits: NativeDataManifestLimits,
) -> Result<Option<VerifiedNativeDataCache>> {
    profile.validate()?;
    preflight_native_data_manifest(input, limits)?;
    let root = Path::new(&profile.root);
    private_directory(root)?;
    let uid = unsafe { libc::geteuid() } as u32;
    if fs::symlink_metadata(root)?.uid() != uid {
        return Err(Error::Invalid("native cache root owner differs"));
    }
    let data_dir = root.join("data");
    private_directory(&data_dir)?;
    if fs::symlink_metadata(&data_dir)?.uid() != uid {
        return Err(Error::Invalid("native cache data owner differs"));
    }
    let manifest_path = data_dir.join("manifest.json");
    let raw = read_bounded(
        &manifest_path,
        limits.max_manifest_bytes as u64,
        limits.deadline,
    )?;
    if Digest256::of_bytes(&raw).to_hex() != profile.manifest_sha256 {
        return Err(Error::Invalid("native cache manifest digest differs"));
    }
    let limits_json = JsonLimits::new(
        limits.max_manifest_bytes,
        MAX_JSON_DEPTH,
        MAX_JSON_VISITS,
        MAX_JSON_INTEGER_DIGITS,
    )
    .map_err(|_| Error::Budget("native cache manifest JSON limits"))?;
    let _document = parse_json(&raw, JsonMode::PublishedStrict, limits_json)
        .map_err(|error| Error::Source(error.to_string()))?;
    let value: Value =
        serde_json::from_slice(&raw).map_err(|_| Error::Invalid("native cache manifest JSON"))?;
    if canonical_value(&value, limits.max_manifest_bytes)? != raw {
        return Err(Error::Invalid("native cache manifest is not canonical"));
    }
    exact_object_keys(
        &value,
        &[
            "schema_version",
            "corpus_revision",
            "input_bindings",
            "compiler",
            "members",
            "data_revision",
            "native_selection",
        ],
        "native cache manifest keys",
    )?;
    if value.get("schema_version").and_then(Value::as_str) != Some(NATIVE_DATA_SCHEMA)
        || value.get("native_selection").and_then(Value::as_str) != Some(NATIVE_SELECTION_PATH)
        || value.get("data_revision").and_then(Value::as_str)
            != Some(profile.data_revision.as_str())
    {
        return Err(Error::Invalid("native cache manifest profile binding"));
    }
    let mut body = value.clone();
    body.as_object_mut()
        .ok_or(Error::Invalid("native cache manifest object"))?
        .remove("data_revision");
    let computed_revision =
        Digest256::of_bytes(&canonical_value(&body, limits.max_manifest_bytes)?).to_hex();
    if computed_revision != profile.data_revision {
        return Err(Error::Invalid("native cache data revision mismatch"));
    }
    let members = native_cache_member_rows(&value)?;
    let mut expected_files = members.clone();
    expected_files.insert(
        "data/manifest.json".into(),
        MemberDigest {
            path: "data/manifest.json".into(),
            size_bytes: raw.len() as u64,
            sha256: profile.manifest_sha256.clone(),
        },
    );
    let mut found_files = BTreeSet::new();
    let mut found_directories = BTreeSet::new();
    let mut entries_seen = 0usize;
    scan_cache_tree(
        root,
        root,
        &expected_files,
        &mut found_files,
        &mut found_directories,
        &mut entries_seen,
        0,
        uid,
        limits.deadline,
    )?;
    if found_files != expected_files.keys().cloned().collect()
        || !found_directories.contains("data")
    {
        return Err(Error::Invalid("native cache exact directory set differs"));
    }
    let mut member_bytes = 0u64;
    for member in members.values() {
        active(limits.deadline)?;
        let path = member_path(root, &member.path)?;
        let cap = match member.path.as_str() {
            NATIVE_MODEL_PATH => MAX_NATIVE_MODEL_BYTES,
            NATIVE_SELECTION_PATH => MAX_MANIFEST_BYTES as u64,
            _ => MAX_CAPTURE_MEMBER_BYTES as u64,
        }
        .min(limits.max_member_bytes);
        let (size, digest) = read_digest(&path, cap, limits.deadline)?;
        if size != member.size_bytes || digest.to_hex() != member.sha256 {
            return Err(Error::Invalid("native cache member integrity differs"));
        }
        member_bytes = member_bytes
            .checked_add(size)
            .filter(|sum| *sum <= limits.max_total_data_bytes)
            .ok_or(Error::Budget("native cache data byte ceiling"))?;
    }
    if member_bytes
        .checked_add(raw.len() as u64)
        .map_or(true, |sum| sum > limits.max_total_data_bytes)
    {
        return Err(Error::Budget(
            "native cache manifest and member byte ceiling",
        ));
    }
    let bindings = native_cache_bindings(&value)?;
    let model_abi = value
        .get("compiler")
        .and_then(|compiler| compiler.get("schema"))
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("native cache compiler schema"))?;
    let expected_metadata = manifest_value(input, &[], None)?;
    let expected_compiler = expected_metadata
        .get("compiler")
        .ok_or(Error::Invalid("native cache expected compiler"))?;
    let expected_members = input.member_paths.iter().cloned().collect::<BTreeSet<_>>();
    let actual_members = members.keys().cloned().collect::<BTreeSet<_>>();
    if value.get("corpus_revision").and_then(Value::as_str) != Some(input.corpus_revision)
        || value.get("input_bindings") != expected_metadata.get("input_bindings")
        || value.get("compiler") != Some(expected_compiler)
        || actual_members != expected_members
    {
        return Ok(None);
    }
    // Runtime companions are embedded in the current producer image. Their
    // member hashes must bind to those exact bytes before they can authorize
    // decoding the cached selection packet.
    for (path, raw) in crate::d1_public_capture::runtime_companions() {
        let member = members
            .get(&format!("data/{path}"))
            .ok_or(Error::Invalid("native cache runtime companion absent"))?;
        if member.size_bytes != raw.len() as u64
            || member.sha256 != Digest256::of_bytes(raw).to_hex()
        {
            return Err(Error::Invalid("native cache runtime companion differs"));
        }
    }
    let selection_path = member_path(root, NATIVE_SELECTION_PATH)?;
    let descriptor = read_bounded(
        &member_path(
            root,
            "data/ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
        )?,
        MAX_CAPTURE_MEMBER_BYTES as u64,
        limits.deadline,
    )?;
    let entity = read_bounded(
        &member_path(
            root,
            "data/ToS/doctrine/semantic-interchange/entity-types.v1.json",
        )?,
        MAX_CAPTURE_MEMBER_BYTES as u64,
        limits.deadline,
    )?;
    let relation = read_bounded(
        &member_path(
            root,
            "data/ToS/doctrine/semantic-interchange/relation-types.v1.json",
        )?,
        MAX_CAPTURE_MEMBER_BYTES as u64,
        limits.deadline,
    )?;
    let selection_raw = read_bounded(&selection_path, MAX_MANIFEST_BYTES as u64, limits.deadline)?;
    let selection_source_bytes = [
        selection_raw.len(),
        descriptor.len(),
        entity.len(),
        relation.len(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
    .ok_or(Error::Budget("native cache selection source bytes"))?;
    let selection = NativeKnowledgeSelection::decode(
        &selection_raw,
        &descriptor,
        &entity,
        &relation,
        crate::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
        MAX_MANIFEST_BYTES,
    )?;
    verify_cache_selection_bindings(&selection, &members, &bindings)?;
    if selection.expectation().model_abi != model_abi {
        return Err(Error::Invalid("native cache ABI and selection differ"));
    }

    let selected_source_revision = format!("native-projection:{expected_source_revision}");
    let selected_declaration = Digest256::of_bytes(RUNTIME_DATA_DECLARATION).to_hex();
    let identity_matches = selection.expectation().source_cut == selected_source_revision
        && selection.expectation().owner_receipt_id
            == format!("native-snapshot:{expected_source_revision}:{selected_declaration}");
    if !identity_matches {
        return Ok(None);
    }
    Ok(Some(VerifiedNativeDataCache {
        model_path: member_path(root, NATIVE_MODEL_PATH)?,
        model_sha256: members.get(NATIVE_MODEL_PATH).unwrap().sha256.clone(),
        model_size_bytes: members.get(NATIVE_MODEL_PATH).unwrap().size_bytes,
        selection,
        selection_source_bytes,
    }))
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

/// Maintained manifest entry for a verified, source/compiler-identical prior
/// native model. It ties the new selection and output members to the cache
/// carrier without claiming a fresh FullKnowledgeReceipt.
pub fn write_reused_native_data_manifest(
    reused: &crate::native_snapshot::ReusedNativeSnapshot,
    data_root: &Path,
    input: &NativeDataSnapshotManifestInput<'_>,
    selection: &NativeKnowledgeSelection,
    limits: NativeDataManifestLimits,
) -> Result<NativeDataManifestReceipt> {
    let producer = reused.selection().producer();
    let corpus = producer
        .corpus_original
        .as_ref()
        .ok_or(Error::Invalid("reused native corpus Original absent"))?;
    if reused.expectation().model_abi != input.model_abi
        || reused.declaration_sha256() != Digest256::of_bytes(RUNTIME_DATA_DECLARATION)
        || corpus.origin.profile != "captured-runtime-projection-v1"
        || corpus.origin.source_path != CORPUS_INDEX_PATH
        || corpus.origin.source_git_commit.is_some()
        || corpus.origin.source_git_tree.is_some()
        || input.source_bindings.get(CORPUS_INDEX_PATH) != Some(&corpus.origin.source_sha256)
        || reused.expectation().source_cut
            != format!("native-projection:{}", reused.source_revision())
        || corpus.source_cut != reused.expectation().source_cut
        || !same_json(reused.expectation(), selection.expectation())?
        || !same_json(reused.stage(), &selection.producer().stage)?
        || !same_json(&producer.seal, &selection.producer().seal)?
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
            "native selection detached from reused model",
        ));
    }
    write_native_data_manifest(data_root, input, selection, limits)
}

#[cfg(test)]
mod selected_snapshot_tests {
    use super::*;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    #[test]
    fn source_only_profile_binds_canonical_cut_and_selected_worker() {
        let source_members = vec![NativeSourceMemberBinding {
            path: "ToS/source_home.manifest.json".into(),
            mode: 0o644,
            size_bytes: 4,
            sha256: Digest256::of_bytes(b"home").to_hex(),
        }];
        let (membership_root, _) = source_membership(&source_members).unwrap();
        let mut products = required_native_runtime_product_paths()
            .into_iter()
            .map(|path| NativeSourceMemberBinding {
                path: path.into(),
                mode: 0o644,
                size_bytes: 1,
                sha256: Digest256::of_bytes(path.as_bytes()).to_hex(),
            })
            .collect::<Vec<_>>();
        products.sort_by(|left, right| left.path.cmp(&right.path));
        let software = NativeSelectedSoftwareBinding {
            source_git_commit: "1".repeat(40),
            source_git_tree: "2".repeat(40),
            capture_manifest_sha256: Digest256::of_bytes(b"capture").to_hex(),
            components: vec![NativeSourceMemberBinding {
                path: "scripts/schema_worker.py".into(),
                mode: 0o755,
                size_bytes: b"worker source".len() as u64,
                sha256: Digest256::of_bytes(b"worker source").to_hex(),
            }],
            schema_worker_path: "scripts/schema_worker.py".into(),
            schema_worker_sha256: Digest256::of_bytes(b"worker source").to_hex(),
            schema_worker_image_sha256: Digest256::of_bytes(b"worker image").to_hex(),
        };
        let profile = NativeSelectedRuntimeSourceProfile::new(
            Digest256::of_bytes(b"source-revision").to_hex(),
            membership_root,
            source_members,
            software,
        )
        .unwrap()
        .with_runtime_products(products)
        .unwrap();
        profile.validate().unwrap();
        assert_eq!(
            profile.source_binding().projection_root_sha256,
            profile.profile_sha256
        );
        let mut other_worker_image = profile.software.clone();
        other_worker_image.schema_worker_image_sha256 =
            Digest256::of_bytes(b"another worker image").to_hex();
        let other_worker_profile = NativeSelectedRuntimeSourceProfile::new(
            profile.source_revision.clone(),
            profile.membership_root.clone(),
            profile.members.clone(),
            other_worker_image,
        )
        .unwrap();
        assert_ne!(other_worker_profile.profile_sha256, profile.profile_sha256);

        let mut tampered = profile.clone();
        tampered.products[0].sha256 = Digest256::of_bytes(b"different product").to_hex();
        assert!(tampered.validate().is_err());

        let reserved = vec![NativeSourceMemberBinding {
            path: NATIVE_SOURCE_PROFILE_BINDING_PATH.into(),
            mode: 0o644,
            size_bytes: 1,
            sha256: Digest256::of_bytes(b"x").to_hex(),
        }];
        let (reserved_root, _) = source_membership(&reserved).unwrap();
        assert!(
            NativeSelectedRuntimeSourceProfile::new(
                profile.source_revision.clone(),
                reserved_root,
                reserved,
                profile.software.clone(),
            )
            .is_err()
        );
    }

    fn write_json_fixture(root: &Path, relative: &str, value: Value) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    }

    fn materialize_runtime_fixture(root: &Path) {
        write_json_fixture(
            root,
            CORPUS_INDEX_PATH,
            json!({
                "schema_version": "tos_corpus_index_v1",
                "owner_repo": "Tree-of-Sophia",
                "surface_kind": "derived",
                "counts": {"nodes": 0},
                "nodes": [],
                "resources": [],
                "manifests": [],
                "branches": [],
                "relation_edges": [],
                "relation_packs": [],
                "graph_views": [],
                "authority_order": ["ToS/canon"],
                "runtime_projection_boundary": {"runtime_owner": "abyss-stack"},
                "source_navigation": {
                    "schema_version": "tos_source_navigation_v1",
                    "authority_boundary": "fixture source authority",
                    "counts": {"nodes": 0, "edges": 0, "rights": 1},
                    "nodes": [],
                    "edges": [],
                    "rights": [{
                        "rights_id": "tos.rights.fixture",
                        "visibility": "public_metadata_only"
                    }]
                }
            }),
        );
        write_json_fixture(
            root,
            PHILOSOPHY_GRAPH_PATH,
            json!({
                "schema_version": "tos_philosophy_graph_projection_v2",
                "owner_repo": "Tree-of-Sophia",
                "surface_kind": "derived",
                "counts": {"nodes": 0, "edges": 0},
                "nodes": [],
                "edges": [],
                "views": [],
                "clusters": [],
                "review_packets": [],
                "graph_layers": [],
                "runtime_projection_boundary": {"runtime_owner": "abyss-stack"},
                "snapshot_review": {
                    "snapshot_schema_version": "tos_philosophy_graph_projection_snapshot_v1"
                },
                "unresolved_review_surfaces": []
            }),
        );
        write_json_fixture(
            root,
            CLAIM_GRAPH_PATH,
            json!({
                "schema_version": "tos_source_witness_bibliographic_graph_v1",
                "nodes": [],
                "edges": [],
                "claim_traces": []
            }),
        );
        for (relative, raw) in [
            (
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
                include_bytes!(
                    "../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json"
                )
                .as_slice(),
            ),
            (
                "ToS/doctrine/semantic-interchange/relation-types.v1.json",
                include_bytes!(
                    "../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json"
                )
                .as_slice(),
            ),
        ] {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, raw).unwrap();
        }
        for (relative, raw) in crate::d1_public_capture::runtime_companions() {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, raw).unwrap();
        }
    }

    fn historical_runtime_fixture(root: &Path) -> NativeSelectedSnapshotProfile {
        let mut bindings = BTreeMap::new();
        let mut rows = Vec::new();
        for (path, _) in crate::PublicCaptureInputPaths::runtime(root).selected_paths() {
            if !path.exists() {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            let raw = fs::read(path).unwrap();
            let digest = Digest256::of_bytes(&raw).to_hex();
            bindings.insert(relative.clone(), digest.clone());
            rows.push(json!({
                "path": format!("data/{relative}"),
                "size_bytes": raw.len(),
                "sha256": digest,
            }));
        }
        rows.sort_by(|left, right| {
            left.get("path")
                .and_then(Value::as_str)
                .cmp(&right.get("path").and_then(Value::as_str))
        });
        let declaration = runtime_data_declaration().unwrap();
        let output = declaration["compiled_subjects"][0]["output_path"]
            .as_str()
            .unwrap();
        let excluded = SelectedExcludedSourceMember {
            path: format!("data/{output}"),
            sha256: Digest256::of_bytes(b"excluded old SQL").to_hex(),
            size_bytes: 100,
        };
        rows.push(json!({
            "path": excluded.path,
            "sha256": excluded.sha256,
            "size_bytes": excluded.size_bytes,
        }));
        rows.sort_by(|left, right| {
            left.get("path")
                .and_then(Value::as_str)
                .cmp(&right.get("path").and_then(Value::as_str))
        });
        let corpus_revision = Digest256::of_bytes(b"fixture corpus").to_hex();
        let data_revision = Digest256::of_bytes(b"fixture data").to_hex();
        let manifest = serde_json::to_vec(&json!({
            "schema_version": "tos_access_data_snapshot_v1",
            "corpus_revision": corpus_revision,
            "data_revision": data_revision,
            "input_bindings": bindings,
            "compiler": {
                "schema": "fixture",
                "compiler_version": "fixture",
                "compiler_sha256": Digest256::of_bytes(b"old producer").to_hex(),
                "compiler_paths": ["software/producer.rs"],
                "input_bindings": {},
            },
            "members": rows,
        }))
        .unwrap();
        let manifest_path = root
            .parent()
            .unwrap()
            .join("selected-runtime-manifest.json");
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

    fn create_private_dir(path: &Path) {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn write_private_member(root: &Path, relative: &str, raw: &[u8]) {
        let path = root.join(relative);
        let parent = path.parent().unwrap();
        fs::create_dir_all(parent).unwrap();
        let mut current = parent;
        loop {
            fs::set_permissions(current, fs::Permissions::from_mode(0o700)).unwrap();
            if current == root {
                break;
            }
            current = current.parent().unwrap();
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(raw).unwrap();
        file.sync_all().unwrap();
    }

    struct FixtureIsolation;
    impl crate::knowledge_stage::StageIsolation for FixtureIsolation {
        fn verify(
            &self,
            _: &Path,
            _: crate::knowledge_stage::StageLimits,
            _: crate::knowledge_stage::WritePhase,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn write_cache_tree(
        cache_root: &Path,
        capture: &crate::PublicCapture,
        members: &[NativeCapturedMember],
        completed: &crate::native_snapshot::CompletedNativeSnapshot,
        input: &NativeDataSnapshotManifestInput<'_>,
        limits: NativeDataManifestLimits,
        cold: crate::ColdOpenLimits,
        process: crate::NativeProcessLimits,
    ) -> (NativeDataManifestReceipt, NativeKnowledgeSelection) {
        create_private_dir(cache_root);
        let data_dir = cache_root.join("data");
        create_private_dir(&data_dir);
        for member in members {
            let raw = capture
                .read_retained_input(&member.source_path, MAX_CAPTURE_MEMBER_BYTES)
                .unwrap();
            assert_eq!(raw.len() as u64, member.size_bytes);
            assert_eq!(Digest256::of_bytes(&raw).to_hex(), member.sha256);
            write_private_member(cache_root, &format!("data/{}", member.source_path), &raw);
        }
        let model_path = cache_root.join(NATIVE_MODEL_PATH);
        fs::copy(completed.artifact_path(), &model_path).unwrap();
        fs::set_permissions(&model_path, fs::Permissions::from_mode(0o600)).unwrap();
        let paths = crate::NativeSelectionPaths {
            model: NATIVE_MODEL_PATH.into(),
            descriptor: "data/ToS/doctrine/semantic-interchange/query-vocabulary.v1.json".into(),
            entity_registry: "data/ToS/doctrine/semantic-interchange/entity-types.v1.json".into(),
            relation_registry: "data/ToS/doctrine/semantic-interchange/relation-types.v1.json"
                .into(),
        };
        let selection = completed
            .selection_for_copied_model(&model_path, paths, cold, process, MAX_MANIFEST_BYTES)
            .unwrap();
        let selection_raw = selection.encode(MAX_MANIFEST_BYTES).unwrap();
        write_private_member(cache_root, NATIVE_SELECTION_PATH, &selection_raw);
        let receipt =
            write_completed_native_data_manifest(completed, cache_root, input, &selection, limits)
                .unwrap();
        (receipt, selection)
    }

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

    #[test]
    fn persistent_native_cache_hit_miss_corruption_and_deterministic_writer() {
        let temp = tempfile::tempdir().unwrap();
        let source_root = temp.path().join("runtime-source");
        fs::create_dir(&source_root).unwrap();
        materialize_runtime_fixture(&source_root);
        let profile = historical_runtime_fixture(&source_root);
        let deadline = Instant::now() + std::time::Duration::from_secs(300);
        let mut census = census_selected_runtime_closure(&profile, deadline).unwrap();
        let limits = portable_native_snapshot_limits(300).unwrap();
        let capture_path = temp.path().join("current-capture.sqlite3");
        let capture = crate::PublicCapture::create_runtime(
            &source_root,
            &capture_path,
            limits.capture,
            deadline,
        )
        .unwrap();
        let captured_members = census.validate_capture_closure(&capture, deadline).unwrap();
        let completed = crate::native_snapshot::build_native_snapshot_from_capture(
            &capture,
            &temp.path().join("native-model.sqlite3"),
            RUNTIME_DATA_DECLARATION,
            &FixtureIsolation,
            limits,
        )
        .unwrap();
        assert_eq!(
            completed
                .producer()
                .corpus_original
                .as_ref()
                .unwrap()
                .origin
                .profile,
            "captured-runtime-projection-v1"
        );
        let mut source_bindings = census.source_bindings().clone();
        source_bindings.insert(
            RUNTIME_DATA_DECLARATION_PATH.into(),
            Digest256::of_bytes(RUNTIME_DATA_DECLARATION).to_hex(),
        );
        source_bindings.insert(
            EVIDENCE_SCENES_PATH.into(),
            profile.evidence_scenes_sha256.clone(),
        );
        let mut member_paths = captured_members
            .iter()
            .map(|member| format!("data/{}", member.source_path))
            .collect::<Vec<_>>();
        member_paths.extend([NATIVE_MODEL_PATH.into(), NATIVE_SELECTION_PATH.into()]);
        member_paths.sort();
        let compiler = fingerprint_native_compiler_source(deadline).unwrap();
        let input = NativeDataSnapshotManifestInput {
            corpus_revision: &profile.corpus_revision,
            selected_profile: Some(&profile),
            selected_census: Some(&census),
            selected_runtime_source: None,
            source_only_census: None,
            model_abi: &completed.expectation().model_abi,
            compiler: &compiler,
            source_bindings: &source_bindings,
            member_paths: &member_paths,
            native_selection: NATIVE_SELECTION_PATH,
        };
        let manifest_limits = NativeDataManifestLimits {
            max_manifest_bytes: MAX_MANIFEST_BYTES,
            max_members: NATIVE_PRODUCER_MAX_MEMBERS,
            max_member_bytes: NATIVE_PRODUCER_MAX_DATA_BYTES,
            max_total_data_bytes: NATIVE_PRODUCER_MAX_DATA_BYTES,
            deadline,
        };
        let cold = crate::knowledge_full_fixture::native_fixture_cold_limits(
            limits.full,
            completed.components().source_scope.source_count,
        );
        let process = crate::knowledge_full_fixture::NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS;
        let first_root = temp.path().join("cache-first");
        let second_root = temp.path().join("cache-second");
        let (first, first_selection) = write_cache_tree(
            &first_root,
            &capture,
            &captured_members,
            &completed,
            &input,
            manifest_limits,
            cold,
            process,
        );
        let (second, _) = write_cache_tree(
            &second_root,
            &capture,
            &captured_members,
            &completed,
            &input,
            manifest_limits,
            cold,
            process,
        );
        let first_manifest = fs::read(first_root.join("data/manifest.json")).unwrap();
        let second_manifest = fs::read(second_root.join("data/manifest.json")).unwrap();
        assert_eq!(first_manifest, second_manifest);
        assert_eq!(first.manifest_sha256, second.manifest_sha256);
        assert_eq!(first.data_revision, second.data_revision);
        assert!(
            write_completed_native_data_manifest(
                &completed,
                &first_root,
                &input,
                &first_selection,
                manifest_limits,
            )
            .is_err()
        );
        let selected = NativePreviousDataSnapshotProfile {
            schema_version: "tos_native_previous_data_snapshot_profile_v1".into(),
            root: first_root.to_str().unwrap().into(),
            manifest_sha256: first.manifest_sha256.clone(),
            data_revision: first.data_revision.clone(),
        };
        let source_revision = capture.core_source_revision().unwrap();
        let verified_cache = verify_previous_native_data_snapshot(
            &selected,
            &input,
            &source_revision,
            manifest_limits,
        )
        .unwrap()
        .expect("current native cache must be reusable");
        let reused = crate::native_snapshot::reuse_native_snapshot_from_verified_cache(
            &capture,
            verified_cache,
            &source_revision,
            RUNTIME_DATA_DECLARATION,
            &completed.expectation().model_abi,
        )
        .unwrap();
        assert_eq!(
            reused.stage().sqlite_sha256,
            completed.stage().sqlite_sha256
        );
        assert!(
            verify_previous_native_data_snapshot(
                &selected,
                &input,
                "changed-source-cut",
                manifest_limits,
            )
            .unwrap()
            .is_none()
        );
        let corrupt_path = first_root.join(format!("data/{CORPUS_INDEX_PATH}"));
        let mut corrupt = fs::read(&corrupt_path).unwrap();
        corrupt.push(b' ');
        fs::write(&corrupt_path, corrupt).unwrap();
        assert!(
            verify_previous_native_data_snapshot(
                &selected,
                &input,
                &source_revision,
                manifest_limits,
            )
            .is_err()
        );
    }
}
