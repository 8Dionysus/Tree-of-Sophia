//! Mechanical reader for an already selected, private packed initial-input
//! index. This module verifies the held container and walks its authenticated
//! logical-member tree; it creates no source-cut coverage, admission token, or
//! revision authority. The caller owns selection of the two existing tree
//! descriptors and supplies their exact file digests.

use super::source_admission::{AdmissionWorkBudget, active, invalid};
use super::source_admission_packed_objects::{PackedObjectLimitsV2, PackedObjectReaderV2};
use super::source_admission_segment_v2::{
    MEMBERS_KIND, OBJECT_EXTENTS_KIND, SOURCE_ADMISSION_V2_DOMAIN,
};
use super::source_current_cut::foundation_capture as source_foundation_capture;
use rusqlite::params;
use std::{
    cell::{Cell, RefCell},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    mem::size_of,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, RelativePath,
    canonical_bytes_v1, parse_json, parse_json_with_state_budget,
};
use tos_segment_store::{
    AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1, AuthenticatedTreeIoLedgerV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, FrameCoordinate, SegmentLimits,
    SegmentOperationLimitsV1, SegmentOperationWorkV1, SegmentStore,
};
use tos_source_store::{PinnedSqliteConnection, PinnedSqliteIoBudget};

pub(crate) const DESCRIPTOR_MAX_BYTES: usize = 12 * 1024;
pub(crate) const PROFILE_SIDECAR_MAX_BYTES_V1: usize = 64 * 1024;
pub(crate) const DEPENDENCY_CLOSURE_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
const EVIDENCE_HASH_BUFFER_BYTES: usize = 8 * 1024;
const MANIFEST_LEAF: &str = "scale-input-v1.json";
const PROFILE_LEAF: &str = "scale-profile-v1.json";
const DEPENDENCY_CLOSURE_LEAF: &str = "scale-dependency-closure-v1.json";
const SEGMENT_LEAF: &str = "segment";
const MEMBERS_DESCRIPTOR_LEAF: &str = "members.v2.tree";
const OBJECTS_DESCRIPTOR_LEAF: &str = "objects.v2.tree";
const MANIFEST_SCHEMA: &str = "tos_native_scale_packed_input_v1";
const MANIFEST_PROFILE_REF: &str = PROFILE_LEAF;
const MANIFEST_TEMPLATE_COMMIT: &str = "5ad427d376b04b54f2c630b0d3859d4abdf32003";
const MANIFEST_SOURCE_STATUS: &str = "synthetic_private_fixture";
const ROOT_GUARD_BYTES: u64 = 4096;
const MEMBER_VALUE_BYTES: usize = 44;
const LOGICAL_SOURCE_PREFIX: &[u8] = b"ToS/";
// Every selected member starts with `ToS/`; `ToS0` is the first byte string
// after that prefix range in BINARY order.
const LOGICAL_SOURCE_PREFIX_END: &[u8] = b"ToS0";

/// The already selected private input container. Descriptor bytes are the
/// existing canonical AuthenticatedTreeDescriptorV2 encoding, not a new
/// index/authority format. The caller's selector supplies the two expected
/// digests and the completed census totals; this reader independently checks
/// descriptor bytes, root custody, every row, payload fixity, count and EOF.
pub(crate) struct IndexedInputSelectionV1 {
    pub(crate) named_root: PathBuf,
    pub(crate) held_root: File,
    pub(crate) held_segment_root: File,
    pub(crate) segment: SegmentStore,
    segment_limits: SegmentLimits,
    manifest_file: DescriptorFile,
    pub(crate) manifest_sha256: Digest256,
    pub(crate) composition: Option<IndexedInputCompositionV1>,
    generated_declaration: Option<SelectedGeneratedDeclarationV1>,
    max_descriptor_bytes: usize,
    max_profile_bytes: usize,
    max_dependency_closure_bytes: usize,
    pub(crate) profile_sha256: Digest256,
    pub(crate) dependency_closure_sha256: Digest256,
    profile_file: DescriptorFile,
    dependency_closure_file: DescriptorFile,
    pub(crate) members_descriptor_leaf: String,
    pub(crate) members_descriptor_sha256: Digest256,
    pub(crate) objects_descriptor_leaf: String,
    pub(crate) objects_descriptor_sha256: Digest256,
    pub(crate) member_count: u64,
    pub(crate) source_bytes: u64,
    pub(crate) unique_object_count: u64,
    pub(crate) unique_payload_bytes: u64,
    pub(crate) max_frames_per_pack: u32,
}

/// Generated declaration issued only from the held manifest/profile reader.
/// This is byte provenance, not a semantic reference-resolution verdict.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectedGeneratedDeclarationV1 {
    seed_sha256: Digest256,
    template_manifest_sha256: Digest256,
    profile_sha256: Digest256,
    generated_declaration_sha256: Digest256,
    generated_record_count: u64,
    class_counts: [u64; 5],
    composition: IndexedInputCompositionV1,
    // Private issuer field prevents a caller-created declaration from replacing
    // the one authenticated by this input reader.
    manifest_sha256: Digest256,
}

impl SelectedGeneratedDeclarationV1 {
    pub(crate) fn seed_sha256(&self) -> Digest256 {
        self.seed_sha256
    }
    pub(crate) fn template_manifest_sha256(&self) -> Digest256 {
        self.template_manifest_sha256
    }
    pub(crate) fn profile_sha256(&self) -> Digest256 {
        self.profile_sha256
    }
    pub(crate) fn generated_declaration_sha256(&self) -> Digest256 {
        self.generated_declaration_sha256
    }
    pub(crate) fn generated_record_count(&self) -> u64 {
        self.generated_record_count
    }
    pub(crate) fn class_counts(&self) -> [u64; 5] {
        self.class_counts
    }
    pub(crate) fn composition(&self) -> IndexedInputCompositionV1 {
        self.composition
    }
}

/// Exact byte-composition selector. This grants no semantic closure or admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IndexedInputCompositionV1 {
    pub(crate) authored_manifest_sha256: Digest256,
    pub(crate) generated_declaration_sha256: Digest256,
    pub(crate) auxiliary_members_sha256: Digest256,
    pub(crate) generated_record_count: u64,
    pub(crate) auxiliary_member_count: u64,
    pub(crate) auxiliary_source_bytes: u64,
    pub(crate) members_descriptor_sha256: Digest256,
}

struct ParsedScaleInputManifestV1 {
    composition: Option<IndexedInputCompositionV1>,
    seed_sha256: Digest256,
    template_manifest_sha256: Digest256,
    profile_sha256: Digest256,
    dependency_closure_sha256: Digest256,
    members_descriptor_sha256: Digest256,
    objects_descriptor_sha256: Digest256,
    member_count: u64,
    source_bytes: u64,
    unique_object_count: u64,
    unique_payload_bytes: u64,
    max_frames_per_pack: u32,
}

/// All limits are inherited from the same selected invocation. No field here
/// is a new capacity grant. `caller_retained_state_bytes` must include every
/// value the caller keeps live across a cursor or payload operation.
#[derive(Clone, Copy)]
pub(crate) struct IndexedInputLimitsV1 {
    pub(crate) member_tree: AuthenticatedTreeLimitsV1,
    pub(crate) packed_objects: PackedObjectLimitsV2,
    pub(crate) max_members: u64,
    pub(crate) max_member_bytes: u64,
    pub(crate) max_source_bytes: u64,
    pub(crate) max_descriptor_bytes: usize,
    pub(crate) caller_retained_state_bytes: usize,
}

/// Optional explicit input selector for the same initial-cut operation. Its
/// limits are selected by the caller from the existing Native V2 profile.
pub(crate) struct IndexedInputRequestV1 {
    pub(crate) named_root: PathBuf,
    pub(crate) segment_limits: SegmentLimits,
    pub(crate) max_profile_bytes: usize,
    pub(crate) max_dependency_closure_bytes: usize,
    pub(crate) reader_limits: IndexedInputLimitsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    device: u64,
    inode: u64,
    mode: u32,
    length: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl FileStamp {
    fn read(file: &File) -> io::Result<Self> {
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o022 != 0
        {
            return Err(invalid("indexed-input file owner, mode, or type differs"));
        }
        Ok(Self::from_metadata(&metadata))
    }

    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            length: metadata.len(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        }
    }
}

struct DescriptorFile {
    leaf: String,
    expected_sha256: Digest256,
    stamp: FileStamp,
    file: File,
}

impl DescriptorFile {
    fn open_and_verify(
        root: &File,
        leaf: String,
        expected_sha256: Digest256,
        max_bytes: usize,
        io_budget: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut dyn FnMut() -> bool,
    ) -> io::Result<(Self, AuthenticatedTreeDescriptorV2)> {
        let (selected, raw, digest) =
            Self::open_hashed(root, leaf, max_bytes, io_budget, deadline, cancelled, work)?;
        if digest != expected_sha256 {
            return Err(invalid("indexed-input descriptor digest differs"));
        }
        let descriptor = AuthenticatedTreeDescriptorV2::decode(&raw, max_bytes).map_err(invalid)?;
        if descriptor.encode(max_bytes).map_err(invalid)?.as_slice() != raw {
            return Err(invalid(
                "indexed-input descriptor encoding is not canonical",
            ));
        }
        Ok((selected, descriptor))
    }

    fn open_hashed(
        root: &File,
        leaf: String,
        max_bytes: usize,
        io_budget: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut dyn FnMut() -> bool,
    ) -> io::Result<(Self, Vec<u8>, Digest256)> {
        check_leaf(&leaf)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let mut file = tos_fd_open::open_regular_at(root, Path::new(&leaf)).map_err(invalid)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let stamp = FileStamp::read(&file)?;
        if stamp.length > max_bytes as u64 {
            return Err(invalid(
                "indexed-input descriptor exceeds its selected bound",
            ));
        }
        let raw = read_bounded_descriptor(&mut file, max_bytes, io_budget, work)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        if FileStamp::read(&file)? != stamp {
            return Err(invalid("indexed-input descriptor changed while opening"));
        }
        let digest = Digest256::of_bytes(&raw);
        Ok((
            Self {
                leaf,
                expected_sha256: digest,
                stamp,
                file,
            },
            raw,
            digest,
        ))
    }

    fn open_stream_hashed(
        root: &File,
        leaf: String,
        expected_sha256: Digest256,
        max_bytes: usize,
        io_budget: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut dyn FnMut() -> bool,
    ) -> io::Result<Self> {
        check_leaf(&leaf)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let mut file = tos_fd_open::open_regular_at(root, Path::new(&leaf)).map_err(invalid)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let stamp = FileStamp::read(&file)?;
        if stamp.length > max_bytes as u64 {
            return Err(invalid("indexed-input evidence exceeds its selected bound"));
        }
        let digest = hash_bounded_file(&mut file, max_bytes, io_budget, deadline, cancelled, work)?;
        if digest != expected_sha256 {
            return Err(invalid("indexed-input evidence digest differs"));
        }
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named = tos_fd_open::open_regular_at(root, Path::new(&leaf)).map_err(invalid)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let held_stamp = FileStamp::read(&file)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named_stamp = FileStamp::read(&named)?;
        if held_stamp != stamp || named_stamp != stamp {
            return Err(invalid("indexed-input evidence changed while opening"));
        }
        Ok(Self {
            leaf,
            expected_sha256,
            stamp,
            file,
        })
    }

    fn verify(
        &mut self,
        root: &File,
        max_bytes: usize,
        io_budget: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut dyn FnMut() -> bool,
    ) -> io::Result<()> {
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named = tos_fd_open::open_regular_at(root, Path::new(&self.leaf)).map_err(invalid)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named_stamp = FileStamp::read(&named)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let held_stamp = FileStamp::read(&self.file)?;
        if named_stamp != self.stamp || held_stamp != self.stamp {
            return Err(invalid("indexed-input descriptor name or stamp changed"));
        }
        let raw = read_bounded_descriptor(&mut self.file, max_bytes, io_budget, work)?;
        if Digest256::of_bytes(&raw) != self.expected_sha256 {
            return Err(invalid("indexed-input descriptor bytes changed"));
        }
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input descriptor work exhausted"));
        }
        charge_root_guard(io_budget)?;
        let held_after = FileStamp::read(&self.file)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input descriptor work exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named_after = FileStamp::read(&named)?;
        if held_after != self.stamp || named_after != self.stamp {
            return Err(invalid("indexed-input descriptor changed while verifying"));
        }
        active(deadline, cancelled)
    }

    fn verify_stream_hashed(
        &mut self,
        root: &File,
        max_bytes: usize,
        io_budget: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut dyn FnMut() -> bool,
    ) -> io::Result<()> {
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named = tos_fd_open::open_regular_at(root, Path::new(&self.leaf)).map_err(invalid)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named_stamp = FileStamp::read(&named)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let held_stamp = FileStamp::read(&self.file)?;
        if named_stamp != self.stamp || held_stamp != self.stamp {
            return Err(invalid("indexed-input evidence name or stamp changed"));
        }
        let digest = hash_bounded_file(
            &mut self.file,
            max_bytes,
            io_budget,
            deadline,
            cancelled,
            work,
        )?;
        if digest != self.expected_sha256 {
            return Err(invalid("indexed-input evidence bytes changed"));
        }
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let held_after = FileStamp::read(&self.file)?;
        active(deadline, cancelled)?;
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        charge_root_guard(io_budget)?;
        let named_after = FileStamp::read(&named)?;
        if held_after != self.stamp || named_after != self.stamp {
            return Err(invalid("indexed-input evidence changed while verifying"));
        }
        active(deadline, cancelled)
    }

    fn retained_state_bytes(&self) -> io::Result<usize> {
        size_of::<Self>()
            .checked_add(self.leaf.capacity())
            .ok_or_else(|| invalid("indexed-input descriptor state overflow"))
    }
}

/// Open the selected producer artifact through held directory descriptors and
/// parse only its bounded non-authoritative custody manifest. The caller
/// supplies the segment-store limits from this same native invocation.
pub(crate) fn open_selection_from_manifest_v1(
    named_root: &Path,
    segment_limits: SegmentLimits,
    max_manifest_bytes: usize,
    max_profile_bytes: usize,
    max_dependency_closure_bytes: usize,
    io_budget: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &AdmissionWorkBudget,
) -> io::Result<IndexedInputSelectionV1> {
    open_selection_from_manifest_with_composition_v1(
        named_root,
        segment_limits,
        max_manifest_bytes,
        max_profile_bytes,
        max_dependency_closure_bytes,
        io_budget,
        deadline,
        cancelled,
        work,
        None,
        0,
    )
}

pub(crate) fn open_selection_from_manifest_with_composition_v1(
    named_root: &Path,
    segment_limits: SegmentLimits,
    max_manifest_bytes: usize,
    max_profile_bytes: usize,
    max_dependency_closure_bytes: usize,
    io_budget: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &AdmissionWorkBudget,
    expected_composition: Option<IndexedInputCompositionV1>,
    max_composition_state_bytes: usize,
) -> io::Result<IndexedInputSelectionV1> {
    if max_manifest_bytes == 0
        || max_manifest_bytes > DESCRIPTOR_MAX_BYTES
        || max_profile_bytes == 0
        || max_profile_bytes > PROFILE_SIDECAR_MAX_BYTES_V1
        || max_dependency_closure_bytes == 0
        || max_dependency_closure_bytes > DEPENDENCY_CLOSURE_MAX_BYTES_V1
    {
        return Err(invalid(
            "indexed-input metadata bound exceeds selected profile",
        ));
    }
    if expected_composition.is_some()
        && (max_composition_state_bytes == 0 || max_composition_state_bytes == usize::MAX)
    {
        return Err(invalid(
            "indexed-input composition state slice is not finite",
        ));
    }
    active(deadline, cancelled)?;
    let root_walk_count = absolute_path_component_count(named_root)?;
    work.charge_many(
        root_walk_count
            .checked_add(2)
            .ok_or_else(|| invalid("indexed-input root path work overflow"))?,
    )?;
    for _ in 0..root_walk_count {
        charge_root_guard(io_budget)?;
    }
    let held_root = tos_fd_open::open_absolute_directory(named_root).map_err(invalid)?;
    let mut debit = || work.charge(()).is_ok();
    verify_named_directory(
        named_root, &held_root, io_budget, deadline, cancelled, &mut debit,
    )?;
    let (manifest_file, raw, manifest_sha256) = DescriptorFile::open_hashed(
        &held_root,
        MANIFEST_LEAF.to_owned(),
        max_manifest_bytes,
        io_budget,
        deadline,
        cancelled,
        &mut debit,
    )?;
    let manifest = parse_scale_input_manifest_selected_v1(
        &raw,
        max_manifest_bytes,
        expected_composition,
        max_composition_state_bytes,
    )?;
    drop(raw);
    let mut generated_declaration = None;
    let profile_file = if let Some(composition) = manifest.composition {
        let (file, raw, digest) = DescriptorFile::open_hashed(
            &held_root,
            PROFILE_LEAF.to_owned(),
            max_profile_bytes,
            io_budget,
            deadline,
            cancelled,
            &mut debit,
        )?;
        if digest != manifest.profile_sha256 {
            return Err(invalid("indexed-input composed profile digest differs"));
        }
        let class_counts = verify_composition_profile_v1(
            &raw,
            max_profile_bytes,
            &manifest,
            max_composition_state_bytes,
        )?;
        generated_declaration = Some(SelectedGeneratedDeclarationV1 {
            seed_sha256: manifest.seed_sha256,
            template_manifest_sha256: manifest.template_manifest_sha256,
            profile_sha256: manifest.profile_sha256,
            generated_declaration_sha256: composition.generated_declaration_sha256,
            generated_record_count: composition.generated_record_count,
            class_counts,
            composition,
            manifest_sha256,
        });
        file
    } else {
        let profile_file = DescriptorFile::open_stream_hashed(
            &held_root,
            PROFILE_LEAF.to_owned(),
            manifest.profile_sha256,
            max_profile_bytes,
            io_budget,
            deadline,
            cancelled,
            &mut debit,
        )?;
        profile_file
    };
    let dependency_closure_file = DescriptorFile::open_stream_hashed(
        &held_root,
        DEPENDENCY_CLOSURE_LEAF.to_owned(),
        manifest.dependency_closure_sha256,
        max_dependency_closure_bytes,
        io_budget,
        deadline,
        cancelled,
        &mut debit,
    )?;

    active(deadline, cancelled)?;
    if !debit() {
        return Err(invalid("indexed-input segment-open work exhausted"));
    }
    charge_root_guard(io_budget)?;
    let held_segment_root =
        tos_fd_open::open_directory_at(&held_root, Path::new(SEGMENT_LEAF)).map_err(invalid)?;
    verify_named_directory(
        &named_root.join(SEGMENT_LEAF),
        &held_segment_root,
        io_budget,
        deadline,
        cancelled,
        &mut debit,
    )?;
    if manifest.max_frames_per_pack == 0
        || manifest.max_frames_per_pack > segment_limits.max_frames
        || manifest.max_frames_per_pack
            > crate::source_admission_packed_objects::MAX_PACKED_OBJECT_FRAMES_V2
        || manifest.unique_object_count == 0
        || manifest.unique_object_count > manifest.member_count
        || manifest.unique_payload_bytes > manifest.source_bytes
    {
        return Err(invalid(
            "indexed-input pack cardinality evidence exceeds selection",
        ));
    }
    // SegmentStore has no shared per-node work callback on open. Its metadata
    // open is fixed-shape and the read ledger/deadline/cancel remain shared;
    // reserve its bounded directory and metadata work before entering it.
    work.charge_many(64)?;
    let tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1> = Arc::new(IndexedTreeIo(io_budget.clone()));
    let segment = SegmentStore::open_existing_at_with_io(
        &held_segment_root,
        segment_limits,
        tree_io,
        deadline,
        cancelled,
    )
    .map_err(invalid)?;
    active(deadline, cancelled)?;
    verify_named_directory(
        &named_root.join(SEGMENT_LEAF),
        &held_segment_root,
        io_budget,
        deadline,
        cancelled,
        &mut debit,
    )?;
    verify_named_directory(
        named_root, &held_root, io_budget, deadline, cancelled, &mut debit,
    )?;
    Ok(IndexedInputSelectionV1 {
        named_root: named_root.to_path_buf(),
        held_root,
        held_segment_root,
        segment,
        segment_limits,
        manifest_file,
        manifest_sha256,
        composition: manifest.composition,
        generated_declaration,
        max_descriptor_bytes: max_manifest_bytes,
        max_profile_bytes,
        max_dependency_closure_bytes,
        profile_sha256: manifest.profile_sha256,
        dependency_closure_sha256: manifest.dependency_closure_sha256,
        profile_file,
        dependency_closure_file,
        members_descriptor_leaf: MEMBERS_DESCRIPTOR_LEAF.to_owned(),
        members_descriptor_sha256: manifest.members_descriptor_sha256,
        objects_descriptor_leaf: OBJECTS_DESCRIPTOR_LEAF.to_owned(),
        objects_descriptor_sha256: manifest.objects_descriptor_sha256,
        member_count: manifest.member_count,
        source_bytes: manifest.source_bytes,
        unique_object_count: manifest.unique_object_count,
        unique_payload_bytes: manifest.unique_payload_bytes,
        max_frames_per_pack: manifest.max_frames_per_pack,
    })
}

fn parse_scale_input_manifest_v1(
    raw: &[u8],
    max_bytes: usize,
) -> io::Result<ParsedScaleInputManifestV1> {
    parse_scale_input_manifest_selected_v1(raw, max_bytes, None, 0)
}

fn parse_scale_input_manifest_selected_v1(
    raw: &[u8],
    max_bytes: usize,
    expected_composition: Option<IndexedInputCompositionV1>,
    state_bytes: usize,
) -> io::Result<ParsedScaleInputManifestV1> {
    const MANIFEST_KEYS: [&str; 21] = [
        "schema",
        "source_status",
        "profile_ref",
        "profile_sha256",
        "seed_sha256",
        "template_source_commit",
        "template_manifest_sha256",
        "member_count",
        "source_bytes",
        "members_descriptor_leaf",
        "members_descriptor_sha256",
        "objects_descriptor_leaf",
        "objects_descriptor_sha256",
        "unique_object_count",
        "unique_payload_bytes",
        "max_frames_per_pack",
        "dependency_closure_sha256",
        "generated_dependency_edges",
        "pinned_external_dependency_edges",
        "unresolved_dependency_edges",
        "authored_route_bridge_coverage",
    ];
    let limits = JsonLimits::new(max_bytes, 8, 64, 20).map_err(invalid)?;
    let document = if expected_composition.is_some() {
        parse_composition_document_v1(raw, limits, state_bytes)?
    } else {
        let document = parse_json(raw, JsonMode::PublishedStrict, limits).map_err(invalid)?;
        if canonical_bytes_v1(
            document.root(),
            CanonicalProfile::SourceCommandInputV1,
            limits,
        )
        .map_err(invalid)?
        .as_slice()
            != raw
        {
            return Err(invalid("indexed-input manifest is not canonical JSON"));
        }
        document
    };
    let value = document.root();
    let fields = value
        .as_object()
        .ok_or_else(|| invalid("indexed-input manifest root is not an object"))?;
    if fields.len() != MANIFEST_KEYS.len() + usize::from(expected_composition.is_some())
        || fields.iter().any(|(key, _)| {
            !MANIFEST_KEYS.contains(&key.as_str().unwrap_or(""))
                && !(expected_composition.is_some()
                    && key.as_str() == Some("authored_aux_composition"))
        })
    {
        return Err(invalid("indexed-input manifest field set differs"));
    }
    let string = |key: &str| -> io::Result<&str> {
        value
            .object_get(key)
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid("indexed-input manifest string field differs"))
    };
    let parse_digest = |key: &str| -> io::Result<Digest256> {
        let text = string(key)?;
        if text.len() != 64
            || !text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid("indexed-input manifest SHA-256 differs"));
        }
        Digest256::from_hex(text).map_err(invalid)
    };
    if string("schema")? != MANIFEST_SCHEMA
        || string("source_status")? != MANIFEST_SOURCE_STATUS
        || string("profile_ref")? != MANIFEST_PROFILE_REF
        || string("members_descriptor_leaf")? != MEMBERS_DESCRIPTOR_LEAF
        || string("objects_descriptor_leaf")? != OBJECTS_DESCRIPTOR_LEAF
    {
        return Err(invalid("indexed-input manifest selection differs"));
    }
    let profile_sha256 = parse_digest("profile_sha256")?;
    let seed_sha256 = parse_digest("seed_sha256")?;
    let template_manifest_sha256 = parse_digest("template_manifest_sha256")?;
    let dependency_closure_sha256 = parse_digest("dependency_closure_sha256")?;
    let commit = string("template_source_commit")?;
    if commit != MANIFEST_TEMPLATE_COMMIT
        || commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("indexed-input template source commit differs"));
    }
    let member_count = value
        .object_get("member_count")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid("indexed-input manifest member count differs"))?;
    let source_bytes = value
        .object_get("source_bytes")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid("indexed-input manifest source byte count differs"))?;
    let unique_object_count = value
        .object_get("unique_object_count")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid("indexed-input unique object count differs"))?;
    let unique_payload_bytes = value
        .object_get("unique_payload_bytes")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid("indexed-input unique payload byte count differs"))?;
    let max_frames_per_pack = value
        .object_get("max_frames_per_pack")
        .and_then(JsonValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid("indexed-input pack frame count differs"))?;
    let generated_dependency_edges = value
        .object_get("generated_dependency_edges")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid("indexed-input generated dependency edge count differs"))?;
    let pinned_external_dependency_edges = value
        .object_get("pinned_external_dependency_edges")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid("indexed-input pinned dependency edge count differs"))?;
    let unresolved_dependency_edges = value
        .object_get("unresolved_dependency_edges")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid("indexed-input unresolved dependency edge count differs"))?;
    let authored_route_bridge_coverage = string("authored_route_bridge_coverage")?;
    generated_dependency_edges
        .checked_add(pinned_external_dependency_edges)
        .and_then(|count| count.checked_add(unresolved_dependency_edges))
        .ok_or_else(|| invalid("indexed-input dependency edge count overflow"))?;
    if member_count == 0
        || source_bytes == 0
        || unique_object_count == 0
        || unique_payload_bytes == 0
        || max_frames_per_pack == 0
        || unresolved_dependency_edges != 0
        || authored_route_bridge_coverage != "unsupported_maintained_owner"
        || unique_object_count > member_count
        || unique_payload_bytes > source_bytes
    {
        return Err(invalid(
            "indexed-input manifest empty or deduplicated totals differ",
        ));
    }
    let composition = match expected_composition {
        Some(expected) => {
            let actual = parse_composition_v1(
                value
                    .object_get("authored_aux_composition")
                    .ok_or_else(|| invalid("indexed-input explicit composition absent"))?,
            )?;
            if actual != expected
                || actual.generated_record_count == 0
                || actual.auxiliary_member_count == 0
                || actual.auxiliary_source_bytes == 0
                || actual
                    .generated_record_count
                    .checked_add(actual.auxiliary_member_count)
                    != Some(member_count)
                || actual.auxiliary_source_bytes >= source_bytes
                || actual.members_descriptor_sha256 != parse_digest("members_descriptor_sha256")?
            {
                return Err(invalid(
                    "indexed-input selected composition aggregate differs",
                ));
            }
            Some(actual)
        }
        None => None,
    };
    Ok(ParsedScaleInputManifestV1 {
        composition,
        seed_sha256,
        template_manifest_sha256,
        profile_sha256,
        dependency_closure_sha256,
        members_descriptor_sha256: parse_digest("members_descriptor_sha256")?,
        objects_descriptor_sha256: parse_digest("objects_descriptor_sha256")?,
        member_count,
        source_bytes,
        unique_object_count,
        unique_payload_bytes,
        max_frames_per_pack,
    })
}

fn parse_composition_document_v1(
    raw: &[u8],
    limits: JsonLimits,
    state: usize,
) -> io::Result<tos_foundation::JsonDocument> {
    // Reuse the maintained foundation report visitor's workspace partition:
    // selected raw/output bytes and per-visit object-reference/duplicate-key
    // workspace remain separate from the parser's allocation meter.
    let fixed = limits
        .max_bytes
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(limits.max_bytes))
        .and_then(|bytes| {
            limits
                .max_visits
                .checked_mul(64)
                .and_then(|visitor| bytes.checked_add(visitor))
        })
        .and_then(|bytes| bytes.checked_add(size_of::<ParsedScaleInputManifestV1>()))
        .and_then(|bytes| bytes.checked_add(size_of::<Option<SelectedGeneratedDeclarationV1>>()))
        .ok_or_else(|| invalid("indexed-input composition workspace overflow"))?;
    let parser_state = state
        .checked_sub(fixed)
        .ok_or_else(|| invalid("indexed-input composition workspace exhausted"))?;
    let document =
        parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, parser_state)
            .map_err(invalid)?;
    if canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceCommandInputV1,
        limits,
    )
    .map_err(invalid)?
    .as_slice()
        != raw
    {
        return Err(invalid("indexed-input composition JSON is not canonical"));
    }
    Ok(document)
}

fn composition_digest_v1(value: &JsonValue) -> io::Result<Digest256> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid("indexed-input composition digest type differs"))?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("indexed-input composition digest differs"));
    }
    Digest256::from_hex(text).map_err(invalid)
}

fn parse_composition_v1(value: &JsonValue) -> io::Result<IndexedInputCompositionV1> {
    parse_composition_fields_v1(value, None)
}

fn parse_composition_fields_v1(
    value: &JsonValue,
    profile_descriptor: Option<Digest256>,
) -> io::Result<IndexedInputCompositionV1> {
    const KEYS: [&str; 8] = [
        "coverage",
        "authored_manifest_sha256",
        "generated_declaration_sha256",
        "auxiliary_members_sha256",
        "generated_record_count",
        "auxiliary_member_count",
        "auxiliary_source_bytes",
        "members_descriptor_sha256",
    ];
    let fields = value
        .as_object()
        .ok_or_else(|| invalid("indexed-input composition type differs"))?;
    let keys = &KEYS[..KEYS.len() - usize::from(profile_descriptor.is_some())];
    if fields.len() != keys.len()
        || fields
            .iter()
            .any(|(k, _)| !keys.contains(&k.as_str().unwrap_or("")))
        || value.object_get("coverage").and_then(JsonValue::as_str)
            != Some("authenticated_byte_composition_pending_semantic_admission")
    {
        return Err(invalid(
            "indexed-input composition field set or coverage differs",
        ));
    }
    let field = |key: &str| {
        value
            .object_get(key)
            .ok_or_else(|| invalid("indexed-input composition field absent"))
    };
    let digest = |key: &str| composition_digest_v1(field(key)?);
    let number = |key: &str| {
        field(key)?
            .as_u64()
            .filter(|n| *n > 0 && *n != u64::MAX)
            .ok_or_else(|| invalid("indexed-input composition finite count differs"))
    };
    Ok(IndexedInputCompositionV1 {
        authored_manifest_sha256: digest("authored_manifest_sha256")?,
        generated_declaration_sha256: digest("generated_declaration_sha256")?,
        auxiliary_members_sha256: digest("auxiliary_members_sha256")?,
        generated_record_count: number("generated_record_count")?,
        auxiliary_member_count: number("auxiliary_member_count")?,
        auxiliary_source_bytes: number("auxiliary_source_bytes")?,
        members_descriptor_sha256: match profile_descriptor {
            Some(digest) => digest,
            None => digest("members_descriptor_sha256")?,
        },
    })
}

fn verify_composition_profile_v1(
    raw: &[u8],
    max_bytes: usize,
    manifest: &ParsedScaleInputManifestV1,
    state_bytes: usize,
) -> io::Result<[u64; 5]> {
    let limits = JsonLimits::new(max_bytes, 16, 2048, 128).map_err(invalid)?;
    let document = parse_composition_document_v1(raw, limits, state_bytes)?;
    let profile = document.root();
    let expected = manifest
        .composition
        .ok_or_else(|| invalid("indexed-input composition profile unselected"))?;
    if profile.object_get("schema").and_then(JsonValue::as_str)
        != Some("tos_native_scale_effective_profile_v2")
        || profile.object_get("status").and_then(JsonValue::as_str)
            != Some("working_hypothesis_no_admission")
        || profile
            .object_get("target_records")
            .and_then(JsonValue::as_u64)
            != Some(expected.generated_record_count)
    {
        return Err(invalid(
            "indexed-input composition semantic record profile differs",
        ));
    }
    let selection = profile
        .object_get("authored_aux_selection")
        .ok_or_else(|| invalid("indexed-input profile composition absent"))?;
    if parse_composition_fields_v1(selection, Some(expected.members_descriptor_sha256))? != expected
    {
        return Err(invalid("indexed-input profile composition binding differs"));
    }
    let classes = profile
        .object_get("classes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid("indexed-input composed class declaration absent"))?;
    let names = ["Artifact", "Claim", "EvidencePacket", "TextUnit", "Work"];
    if classes.len() != names.len() {
        return Err(invalid("indexed-input composed class count differs"));
    }
    let mut counts = [0u64; 5];
    let mut total = 0u64;
    for (index, class) in classes.iter().enumerate() {
        if class.object_get("class").and_then(JsonValue::as_str) != Some(names[index]) {
            return Err(invalid("indexed-input composed class ordering differs"));
        }
        counts[index] = class
            .object_get("count")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| invalid("indexed-input composed class count differs"))?;
        total = total
            .checked_add(counts[index])
            .ok_or_else(|| invalid("indexed-input composed class overflow"))?;
    }
    if total != expected.generated_record_count {
        return Err(invalid("indexed-input composed class total differs"));
    }
    // Fixed typed preimage, emitted directly to the digest: no second Value
    // tree or encoded declaration allocation while profile parsing stays live.
    let mut hash = Digest256Hasher::new();
    hash.update(b"{\"class_counts\":[");
    for (index, count) in counts.iter().enumerate() {
        if index != 0 {
            hash.update(b",");
        }
        feed_composition_number_v1(&mut hash, *count);
    }
    hash.update(b"],\"domain\":\"tos_scale_generated_declaration_v1\",\"generated_record_count\":");
    feed_composition_number_v1(&mut hash, expected.generated_record_count);
    hash.update(b",\"seed_sha256\":\"");
    hash.update(manifest.seed_sha256.to_hex().as_bytes());
    hash.update(b"\",\"template_manifest_sha256\":\"");
    hash.update(manifest.template_manifest_sha256.to_hex().as_bytes());
    hash.update(b"\"}");
    if hash.finalize() != expected.generated_declaration_sha256 {
        return Err(invalid(
            "indexed-input generated declaration digest differs",
        ));
    }
    Ok(counts)
}

fn feed_composition_number_v1(hash: &mut Digest256Hasher, mut value: u64) {
    let mut digits = [0u8; u64::MAX.ilog10() as usize + 1];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    hash.update(&digits[start..]);
}

/// One authenticated current logical source member. The retained-state charge
/// is returned with the value so the caller can include it in its next cursor
/// call, then release it when the value is dropped.
pub(crate) struct IndexedInputMemberV1 {
    pub(crate) path: RelativePath,
    pub(crate) sha256: Digest256,
    pub(crate) size_bytes: u64,
    pub(crate) source_mode: u32,
    pub(crate) retained_state_bytes: usize,
    input_identity: Digest256,
}

impl IndexedInputMemberV1 {
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.retained_state_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct IndexedInputCostV1 {
    pub(crate) member_rows: u64,
    pub(crate) source_bytes: u64,
    pub(crate) member_tree: AuthenticatedTreeWorkV1,
    pub(crate) object_tree: AuthenticatedTreeWorkV1,
    pub(crate) object_reads: SegmentOperationWorkV1,
    pub(crate) payload_members: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) shared_work_units: u64,
}

/// A held, read-only logical member cursor over a private packed input.
/// The reader emits no admission/currentness proof; callers still run their
/// maintained validator and final native root/fence checks.
pub(crate) struct IndexedInputReaderV1 {
    named_root: PathBuf,
    held_root: File,
    held_segment_root: File,
    root_stamp: (u64, u64, u32, i64, i64, i64, i64),
    segment_root_stamp: (u64, u64, u32, i64, i64, i64, i64),
    segment: SegmentStore,
    segment_identity: (u64, u64),
    manifest_file: DescriptorFile,
    pub(crate) composition: Option<IndexedInputCompositionV1>,
    generated_declaration: Option<SelectedGeneratedDeclarationV1>,
    max_profile_bytes: usize,
    max_dependency_closure_bytes: usize,
    profile_file: DescriptorFile,
    dependency_closure_file: DescriptorFile,
    members_file: DescriptorFile,
    objects_file: DescriptorFile,
    members: AuthenticatedTreeDescriptorV2,
    objects_descriptor: AuthenticatedTreeDescriptorV2,
    limits: IndexedInputLimitsV1,
    io_budget: PinnedSqliteIoBudget,
    extents_db: Rc<RefCell<PinnedSqliteConnection>>,
    tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    work: AdmissionWorkBudget,
    work_debits: Rc<Cell<u64>>,
    input_identity: Digest256,
    after: Option<Vec<u8>>,
    pending_payload: Option<(Digest256, u64)>,
    expected_member_count: u64,
    expected_source_bytes: u64,
    expected_unique_object_count: u64,
    expected_unique_payload_bytes: u64,
    max_frames_per_pack: u32,
    max_caller_live_state_bytes: usize,
    eof: bool,
    failed: bool,
    cost: IndexedInputCostV1,
    member_count: u64,
    source_bytes: u64,
}

impl IndexedInputReaderV1 {
    /// Return the declaration authenticated with this reader's held manifest
    /// and profile; callers cannot synthesize this issued value.
    pub(crate) fn selected_generated_declaration_v1(
        &self,
    ) -> io::Result<Option<&SelectedGeneratedDeclarationV1>> {
        if let Some(declaration) = &self.generated_declaration {
            if declaration.manifest_sha256 != self.manifest_file.expected_sha256
                || declaration.profile_sha256 != self.profile_file.expected_sha256
                || Some(declaration.composition) != self.composition
            {
                return Err(invalid(
                    "indexed-input issued generated declaration binding differs",
                ));
            }
        }
        Ok(self.generated_declaration.as_ref())
    }

    pub(crate) fn open(
        mut selection: IndexedInputSelectionV1,
        limits: IndexedInputLimitsV1,
        extents_db: Rc<RefCell<PinnedSqliteConnection>>,
        io_budget: PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
        work: AdmissionWorkBudget,
    ) -> io::Result<Self> {
        validate_limits(limits, selection.member_count, selection.source_bytes)?;
        active(deadline, &cancelled)?;
        let work_debits = Rc::new(Cell::new(0));
        let open_budget = work.clone();
        let open_meter = work_debits.clone();
        let mut open_work = move || debit_work(&open_budget, &open_meter);
        if !open_work() {
            return Err(invalid("indexed-input root-check work exhausted"));
        }
        verify_named_directory(
            &selection.named_root,
            &selection.held_root,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        if !open_work() {
            return Err(invalid("indexed-input root-check work exhausted"));
        }
        let segment_identity = selection
            .segment
            .physical_root_identity()
            .map_err(invalid)?;
        let held_segment_metadata = selection.held_segment_root.metadata()?;
        if (held_segment_metadata.dev(), held_segment_metadata.ino()) != segment_identity {
            return Err(invalid("indexed-input segment root binding differs"));
        }
        let segment_root_stamp = directory_stamp(&held_segment_metadata);
        verify_named_directory(
            &selection.named_root.join(SEGMENT_LEAF),
            &selection.held_segment_root,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        let work_meter_state = work_debit_allocation_state_bytes()?;
        let segment_heap_state = selection
            .segment
            .retained_heap_state_bytes()
            .map_err(invalid)?;
        let open_base_state = size_of::<Self>()
            .checked_add(selection.named_root.as_os_str().len())
            .and_then(|bytes| bytes.checked_add(selection.members_descriptor_leaf.capacity()))
            .and_then(|bytes| bytes.checked_add(selection.objects_descriptor_leaf.capacity()))
            .and_then(|bytes| bytes.checked_add(selection.manifest_file.leaf.capacity()))
            .and_then(|bytes| bytes.checked_add(selection.profile_file.leaf.capacity()))
            .and_then(|bytes| bytes.checked_add(selection.dependency_closure_file.leaf.capacity()))
            .and_then(|bytes| bytes.checked_add(size_of::<SegmentStore>()))
            .and_then(|bytes| bytes.checked_add(segment_heap_state))
            .and_then(|bytes| {
                bytes.checked_add(
                    limits
                        .max_descriptor_bytes
                        .checked_mul(4)?
                        .checked_add(64 * 1024)?
                        .checked_add(EVIDENCE_HASH_BUFFER_BYTES)?,
                )
            })
            .and_then(|bytes| bytes.checked_add(work_meter_state))
            .and_then(|bytes| bytes.checked_add(limits.caller_retained_state_bytes))
            .ok_or_else(|| invalid("indexed-input descriptor preflight overflow"))?;
        if open_base_state >= limits.packed_objects.max_working_state_bytes {
            return Err(invalid("indexed-input descriptor state slice exhausted"));
        }
        let (members_file, members) = DescriptorFile::open_and_verify(
            &selection.held_root,
            selection.members_descriptor_leaf,
            selection.members_descriptor_sha256,
            limits.max_descriptor_bytes,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        let (objects_file, objects_descriptor) = DescriptorFile::open_and_verify(
            &selection.held_root,
            selection.objects_descriptor_leaf,
            selection.objects_descriptor_sha256,
            limits.max_descriptor_bytes,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        let selected_segment_limits = limits.packed_objects.segment_limits;
        if selected_segment_limits.max_segment_bytes != selection.segment_limits.max_segment_bytes
            || selected_segment_limits.max_frame_bytes != selection.segment_limits.max_frame_bytes
            || selected_segment_limits.max_frames != selection.segment_limits.max_frames
            || selected_segment_limits.max_journal_bytes
                != selection.segment_limits.max_journal_bytes
        {
            return Err(invalid(
                "indexed-input segment profile differs from selected store",
            ));
        }
        if objects_descriptor.entries != selection.unique_object_count
            || selection.unique_object_count > limits.packed_objects.max_objects
            || selection.max_frames_per_pack > limits.packed_objects.max_pack_frames
            || selection.max_frames_per_pack > selection.segment_limits.max_frames
            || selection.unique_payload_bytes > limits.max_source_bytes
        {
            return Err(invalid(
                "indexed-input pack cardinality differs from selected profile",
            ));
        }
        if !open_work() {
            return Err(invalid("indexed-input root-check work exhausted"));
        }
        verify_named_directory(
            &selection.named_root,
            &selection.held_root,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        selection.manifest_file.verify(
            &selection.held_root,
            selection.max_descriptor_bytes,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        selection.profile_file.verify_stream_hashed(
            &selection.held_root,
            selection.max_profile_bytes,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        selection.dependency_closure_file.verify_stream_hashed(
            &selection.held_root,
            selection.max_dependency_closure_bytes,
            &io_budget,
            deadline,
            &cancelled,
            &mut open_work,
        )?;
        validate_tree(
            &members,
            &selection.segment,
            MEMBERS_KIND,
            selection.member_count,
            limits.member_tree,
        )?;
        validate_tree(
            &objects_descriptor,
            &selection.segment,
            OBJECT_EXTENTS_KIND,
            objects_descriptor.entries,
            limits.packed_objects.tree_limits,
        )?;
        let root_metadata = selection.held_root.metadata()?;
        let root_stamp = directory_stamp(&root_metadata);
        let members_file_state = members_file.retained_state_bytes()?;
        let objects_file_state = objects_file.retained_state_bytes()?;
        let manifest_file_state = selection.manifest_file.retained_state_bytes()?;
        let profile_file_state = selection.profile_file.retained_state_bytes()?;
        let dependency_closure_file_state =
            selection.dependency_closure_file.retained_state_bytes()?;
        let descriptor_state = members_file_state
            .checked_add(objects_file_state)
            .and_then(|bytes| bytes.checked_add(manifest_file_state))
            .and_then(|bytes| bytes.checked_add(profile_file_state))
            .and_then(|bytes| bytes.checked_add(dependency_closure_file_state))
            .ok_or_else(|| invalid("indexed-input descriptor state overflow"))?;
        let members_tree_state = tree_descriptor_state(&members)?;
        let objects_tree_state = tree_descriptor_state(&objects_descriptor)?;
        let descriptor_state = descriptor_state
            .checked_add(members_tree_state)
            .and_then(|bytes| bytes.checked_add(objects_tree_state))
            .ok_or_else(|| invalid("indexed-input descriptor state overflow"))?;
        let segment_state = size_of::<SegmentStore>()
            .checked_add(
                selection
                    .segment
                    .retained_heap_state_bytes()
                    .map_err(invalid)?,
            )
            .ok_or_else(|| invalid("indexed-input segment state overflow"))?;
        let reader_state = size_of::<Self>()
            .checked_add(selection.named_root.as_os_str().len())
            .and_then(|bytes| bytes.checked_add(descriptor_state))
            .and_then(|bytes| bytes.checked_add(segment_state))
            .and_then(|bytes| bytes.checked_add(work_meter_state))
            .ok_or_else(|| invalid("indexed-input retained state overflow"))?;
        let state_with_caller = limits
            .caller_retained_state_bytes
            .checked_add(reader_state)
            .ok_or_else(|| invalid("indexed-input retained state overflow"))?;
        if state_with_caller >= limits.packed_objects.max_working_state_bytes {
            return Err(invalid("indexed-input original state slice exhausted"));
        }
        let tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1> =
            Arc::new(IndexedTreeIo(io_budget.clone()));
        let input_identity = input_identity(
            root_stamp,
            segment_identity,
            selection.manifest_sha256,
            selection.profile_sha256,
            selection.dependency_closure_sha256,
            members_file.expected_sha256,
            objects_file.expected_sha256,
            selection.member_count,
            selection.source_bytes,
        );
        let reader = Self {
            named_root: selection.named_root,
            held_root: selection.held_root,
            held_segment_root: selection.held_segment_root,
            root_stamp,
            segment_root_stamp,
            segment: selection.segment,
            segment_identity,
            manifest_file: selection.manifest_file,
            composition: selection.composition,
            generated_declaration: selection.generated_declaration,
            max_profile_bytes: selection.max_profile_bytes,
            max_dependency_closure_bytes: selection.max_dependency_closure_bytes,
            profile_file: selection.profile_file,
            dependency_closure_file: selection.dependency_closure_file,
            members_file,
            objects_file,
            members,
            objects_descriptor,
            limits,
            io_budget,
            extents_db,
            tree_io,
            deadline,
            cancelled,
            work,
            work_debits,
            input_identity,
            after: None,
            pending_payload: None,
            expected_member_count: selection.member_count,
            expected_source_bytes: selection.source_bytes,
            expected_unique_object_count: selection.unique_object_count,
            expected_unique_payload_bytes: selection.unique_payload_bytes,
            max_frames_per_pack: selection.max_frames_per_pack,
            max_caller_live_state_bytes: 0,
            eof: false,
            failed: false,
            cost: IndexedInputCostV1::default(),
            member_count: 0,
            source_bytes: 0,
        };
        reader.verify_root_identity(&mut open_work)?;
        Ok(reader)
    }

    /// Seek one BINARY-ordered logical ToS member. The caller's retained data
    /// is checked together with this reader, the cursor, returned row and the
    /// authenticated-tree transient bound before the seek starts.
    pub(crate) fn next_member(
        &mut self,
        caller_live_state_bytes: usize,
    ) -> io::Result<Option<IndexedInputMemberV1>> {
        if self.failed || self.eof {
            return Err(invalid("indexed-input member cursor is sealed"));
        }
        if self.pending_payload.is_some() {
            return Err(invalid(
                "indexed-input prior member payload is not consumed",
            ));
        }
        let result = (|| {
            active(self.deadline, &self.cancelled)?;
            let retained = self
                .retained_state_bytes()?
                .checked_add(caller_live_state_bytes)
                .ok_or_else(|| invalid("indexed-input cursor state overflow"))?;
            let path_state = self
                .limits
                .member_tree
                .max_key_bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(self.limits.member_tree.max_value_bytes))
                .and_then(|bytes| bytes.checked_add(size_of::<AuthenticatedTreeEntryV1>()))
                .and_then(|bytes| bytes.checked_add(size_of::<IndexedInputMemberV1>()))
                .and_then(|bytes| bytes.checked_add(MEMBER_VALUE_BYTES + 320))
                .ok_or_else(|| invalid("indexed-input row state overflow"))?;
            let fixed_state = retained
                .checked_add(path_state)
                .ok_or_else(|| invalid("indexed-input cursor state overflow"))?;
            if fixed_state >= self.limits.packed_objects.max_working_state_bytes {
                return Err(invalid("indexed-input original state slice exhausted"));
            }
            let available = self
                .limits
                .packed_objects
                .max_working_state_bytes
                .checked_sub(fixed_state)
                .ok_or_else(|| invalid("indexed-input transient state overflow"))?;
            let work_budget = self.work.clone();
            let work_meter = self.work_debits.clone();
            let mut debit_work = move || debit_work(&work_budget, &work_meter);
            if !debit_work() {
                return Err(invalid("indexed-input cursor work exhausted"));
            }
            // This additive owner API invokes the same operation-wide debit
            // before each internal node/child operation, not after it.
            let (row, tree_work) = self
                .segment
                .lookup_authenticated_tree_v2_after_with_work_and_io_and_callback(
                    &self.members,
                    Some(LOGICAL_SOURCE_PREFIX),
                    Some(LOGICAL_SOURCE_PREFIX_END),
                    self.after.as_deref(),
                    self.limits.member_tree,
                    available,
                    Some(self.tree_io.clone()),
                    self.deadline,
                    &self.cancelled,
                    &mut debit_work,
                )
                .map_err(invalid)?;
            self.cost.member_tree.read_nodes = self
                .cost
                .member_tree
                .read_nodes
                .checked_add(tree_work.read_nodes)
                .ok_or_else(|| invalid("indexed-input tree work overflow"))?;
            self.cost.member_tree.read_bytes = self
                .cost
                .member_tree
                .read_bytes
                .checked_add(tree_work.read_bytes)
                .ok_or_else(|| invalid("indexed-input tree byte cost overflow"))?;
            let Some(row) = row else {
                self.eof = true;
                if self.member_count != self.expected_member_count
                    || self.source_bytes != self.expected_source_bytes
                {
                    return Err(invalid("indexed-input member count or source bytes differ"));
                }
                return Ok(None);
            };
            self.decode_member_row(row)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn decode_member_row(
        &mut self,
        row: AuthenticatedTreeEntryV1,
    ) -> io::Result<Option<IndexedInputMemberV1>> {
        if row.value.len() != MEMBER_VALUE_BYTES
            || self
                .after
                .as_deref()
                .is_some_and(|after| after >= row.key.as_slice())
        {
            return Err(invalid("indexed-input member row order or value differs"));
        }
        let path_text = std::str::from_utf8(&row.key).map_err(invalid)?;
        let path = RelativePath::parse(path_text).map_err(invalid)?;
        if path.as_str().as_bytes() != row.key.as_slice()
            || !path.as_str().starts_with("ToS/")
            || !source_foundation_capture::selected(path.as_str(), false)
        {
            return Err(invalid(
                "indexed-input row is outside the selected ToS source",
            ));
        }
        let mut digest_bytes = [0u8; 32];
        digest_bytes.copy_from_slice(&row.value[..32]);
        let digest = Digest256::from_bytes(digest_bytes);
        let size_bytes = u64::from_be_bytes(
            row.value[32..40]
                .try_into()
                .map_err(|_| invalid("indexed-input member size differs"))?,
        );
        let source_mode = u32::from_le_bytes(
            row.value[40..44]
                .try_into()
                .map_err(|_| invalid("indexed-input member mode differs"))?,
        );
        if size_bytes > self.limits.max_member_bytes {
            return Err(invalid(
                "indexed-input member exceeds its selected byte bound",
            ));
        }
        let next_count = self
            .member_count
            .checked_add(1)
            .filter(|count| {
                *count <= self.limits.max_members && *count <= self.expected_member_count
            })
            .ok_or_else(|| invalid("indexed-input member count exceeds profile"))?;
        let next_bytes = self
            .source_bytes
            .checked_add(size_bytes)
            .filter(|bytes| {
                *bytes <= self.limits.max_source_bytes && *bytes <= self.expected_source_bytes
            })
            .ok_or_else(|| invalid("indexed-input source bytes exceed profile"))?;
        let member_state = size_of::<IndexedInputMemberV1>()
            .checked_add(path.as_str().len())
            .and_then(|bytes| bytes.checked_add(32))
            .ok_or_else(|| invalid("indexed-input returned member state overflow"))?;
        self.after = Some(row.key);
        self.member_count = next_count;
        self.source_bytes = next_bytes;
        self.pending_payload = Some((digest, size_bytes));
        self.cost.member_rows = next_count;
        self.cost.source_bytes = next_bytes;
        Ok(Some(IndexedInputMemberV1 {
            path,
            sha256: digest,
            size_bytes,
            source_mode,
            retained_state_bytes: member_state,
            input_identity: self.input_identity,
        }))
    }

    /// Stream a member's actual packed payload through the authenticated
    /// placement reader. The typed metadata must have come from this cursor;
    /// frame verification and source SHA/size are both checked before success.
    pub(crate) fn read_member_payload(
        &mut self,
        member: &IndexedInputMemberV1,
        caller_live_state_bytes: usize,
        sink: &mut dyn Write,
    ) -> io::Result<SegmentOperationWorkV1> {
        if self.failed || member.input_identity != self.input_identity {
            return Err(invalid("indexed-input member belongs to another held root"));
        }
        let result = (|| {
            active(self.deadline, &self.cancelled)?;
            if self.pending_payload != Some((member.sha256, member.size_bytes)) {
                return Err(invalid("indexed-input payload is not the current member"));
            }
            let payload_state = self
                .retained_state_bytes()?
                .checked_add(caller_live_state_bytes)
                .and_then(|bytes| bytes.checked_add(member.retained_state_bytes))
                .and_then(|bytes| bytes.checked_add(4096))
                .ok_or_else(|| invalid("indexed-input payload state overflow"))?;
            if self
                .limits
                .packed_objects
                .caller_live_state_bytes
                .checked_add(payload_state)
                .is_none_or(|bytes| bytes >= self.limits.packed_objects.max_working_state_bytes)
            {
                return Err(invalid(
                    "indexed-input original payload state slice exhausted",
                ));
            }
            let work_budget = self.work.clone();
            let work_meter = self.work_debits.clone();
            let mut debit_work = move || debit_work(&work_budget, &work_meter);
            if !debit_work() {
                return Err(invalid("indexed-input payload work exhausted"));
            }
            let mut object_limits = self.limits.packed_objects;
            object_limits.max_work_units = object_limits.max_work_units.min(self.work.remaining()?);
            object_limits.max_pack_frames =
                object_limits.max_pack_frames.min(self.max_frames_per_pack);
            let mut objects = PackedObjectReaderV2::new(
                &self.segment,
                &self.objects_descriptor,
                object_limits,
                self.tree_io.clone(),
                self.deadline,
                &self.cancelled,
                payload_state,
                &mut debit_work,
            )
            .map_err(invalid)?;
            let (location, tree_work) =
                objects.lookup_with_work(member.sha256, Some(member.size_bytes))?;
            let location =
                location.ok_or_else(|| invalid("indexed-input payload placement is absent"))?;
            self.cost.object_tree = add_authenticated_tree_work(self.cost.object_tree, tree_work)?;
            if location.size != member.size_bytes {
                return Err(invalid("indexed-input payload placement size differs"));
            }
            if location.frame_count > self.max_frames_per_pack
                || location.frame_count > self.limits.packed_objects.max_pack_frames
                || location.frame_count > self.limits.packed_objects.segment_limits.max_frames
            {
                return Err(invalid(
                    "indexed-input pack frame count exceeds selected profile",
                ));
            }
            let mut hashing_sink = HashingSink {
                inner: sink,
                hasher: Digest256Hasher::new(),
                bytes: 0,
            };
            let work = objects.read_exact(
                &location,
                member.sha256,
                member.size_bytes,
                &mut hashing_sink,
            )?;
            let output_bytes = hashing_sink.bytes;
            let output_digest = hashing_sink.hasher.finalize();
            if output_bytes != member.size_bytes || output_digest != member.sha256 {
                return Err(invalid("indexed-input payload fixity or length differs"));
            }
            self.record_extent(member.sha256, location, &mut debit_work)?;
            self.max_caller_live_state_bytes = self
                .max_caller_live_state_bytes
                .max(caller_live_state_bytes);
            self.cost.object_reads.read_bytes = self
                .cost
                .object_reads
                .read_bytes
                .checked_add(work.read_bytes)
                .ok_or_else(|| invalid("indexed-input payload read cost overflow"))?;
            self.cost.object_reads.read_upper_bound_bytes = self
                .cost
                .object_reads
                .read_upper_bound_bytes
                .checked_add(work.read_upper_bound_bytes)
                .ok_or_else(|| invalid("indexed-input payload guard cost overflow"))?;
            self.cost.object_reads.work_units = self
                .cost
                .object_reads
                .work_units
                .checked_add(work.work_units)
                .ok_or_else(|| invalid("indexed-input payload work overflow"))?;
            self.cost.object_reads.work_bytes = self
                .cost
                .object_reads
                .work_bytes
                .checked_add(work.work_bytes)
                .ok_or_else(|| invalid("indexed-input payload work bytes overflow"))?;
            self.cost.payload_members = self
                .cost
                .payload_members
                .checked_add(1)
                .ok_or_else(|| invalid("indexed-input payload count overflow"))?;
            self.cost.payload_bytes = self
                .cost
                .payload_bytes
                .checked_add(output_bytes)
                .ok_or_else(|| invalid("indexed-input payload byte count overflow"))?;
            self.pending_payload = None;
            Ok(work)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn record_extent(
        &self,
        digest: Digest256,
        location: crate::source_admission_packed_objects::PackedObjectLocationV2,
        debit_work: &mut dyn FnMut() -> bool,
    ) -> io::Result<()> {
        let row_size = i64::try_from(location.size)
            .map_err(|_| invalid("indexed-input extent size exceeds SQLite range"))?;
        let segment_size = i64::try_from(location.segment_size)
            .map_err(|_| invalid("indexed-input segment size exceeds SQLite range"))?;
        let frame_index = i64::from(location.frame_index);
        let frame_count = i64::from(location.frame_count);
        let header_offset = i64::try_from(location.header_offset)
            .map_err(|_| invalid("indexed-input frame offset exceeds SQLite range"))?;
        if !debit_work() {
            return Err(invalid("indexed-input extent insert work exhausted"));
        }
        self.extents_db
            .borrow_mut()
            .execute(
                "INSERT OR IGNORE INTO source_indexed_pack_extent_v1(\
                    digest,size,segment_digest,segment_size,frame_index,frame_count,header_offset\
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    digest.as_bytes().as_slice(),
                    row_size,
                    location.segment_digest.as_bytes().as_slice(),
                    segment_size,
                    frame_index,
                    frame_count,
                    header_offset,
                ],
            )
            .map_err(|_| invalid("indexed-input extent insert refused"))?;
        active(self.deadline, &self.cancelled)?;
        if !debit_work() {
            return Err(invalid("indexed-input extent check work exhausted"));
        }
        let stored: (i64, Vec<u8>, i64, i64, i64, i64) = self
            .extents_db
            .borrow()
            .query_row(
                "SELECT size,segment_digest,segment_size,frame_index,frame_count,header_offset \
                 FROM source_indexed_pack_extent_v1 WHERE digest=?1",
                params![digest.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .map_err(|_| invalid("indexed-input extent check refused"))?;
        let stored_segment_digest: [u8; 32] = stored
            .1
            .as_slice()
            .try_into()
            .map_err(|_| invalid("indexed-input stored segment digest width differs"))?;
        if stored.0 != row_size
            || Digest256::from_bytes(stored_segment_digest) != location.segment_digest
            || stored.2 != segment_size
            || stored.3 != frame_index
            || stored.4 != frame_count
            || stored.5 != header_offset
        {
            return Err(invalid("indexed-input duplicate extent placement differs"));
        }
        Ok(())
    }

    /// Finish only after the caller consumed the complete ordered member tree.
    /// The descriptor files, named root and held segment root are rechecked;
    /// this is an input-identity fence, not admission or output authority.
    pub(crate) fn finish(&mut self) -> io::Result<IndexedInputCostV1> {
        if self.failed || !self.eof {
            return Err(invalid(
                "indexed-input stream did not reach authenticated EOF",
            ));
        }
        let result = (|| {
            if self.pending_payload.is_some()
                || self.member_count != self.expected_member_count
                || self.source_bytes != self.expected_source_bytes
                || self.cost.payload_members != self.expected_member_count
                || self.cost.payload_bytes != self.expected_source_bytes
                || self.expected_unique_object_count > self.limits.packed_objects.max_objects
                || self.expected_unique_payload_bytes > self.expected_source_bytes
            {
                return Err(invalid(
                    "indexed-input final counters differ from selected input",
                ));
            }
            let work_budget = self.work.clone();
            let work_meter = self.work_debits.clone();
            let mut debit_work = move || debit_work(&work_budget, &work_meter);
            if !debit_work() {
                return Err(invalid("indexed-input final-fence work exhausted"));
            }
            self.verify_root_identity(&mut debit_work)?;
            self.verify_extent_closure(&mut debit_work)?;
            verify_named_directory(
                &self.named_root,
                &self.held_root,
                &self.io_budget,
                self.deadline,
                &self.cancelled,
                &mut debit_work,
            )?;
            self.members_file.verify(
                &self.held_root,
                self.limits.max_descriptor_bytes,
                &self.io_budget,
                self.deadline,
                &self.cancelled,
                &mut debit_work,
            )?;
            self.objects_file.verify(
                &self.held_root,
                self.limits.max_descriptor_bytes,
                &self.io_budget,
                self.deadline,
                &self.cancelled,
                &mut debit_work,
            )?;
            self.manifest_file.verify(
                &self.held_root,
                self.limits.max_descriptor_bytes,
                &self.io_budget,
                self.deadline,
                &self.cancelled,
                &mut debit_work,
            )?;
            self.profile_file.verify_stream_hashed(
                &self.held_root,
                self.max_profile_bytes,
                &self.io_budget,
                self.deadline,
                &self.cancelled,
                &mut debit_work,
            )?;
            self.dependency_closure_file.verify_stream_hashed(
                &self.held_root,
                self.max_dependency_closure_bytes,
                &self.io_budget,
                self.deadline,
                &self.cancelled,
                &mut debit_work,
            )?;
            self.verify_root_identity(&mut debit_work)?;
            active(self.deadline, &self.cancelled)?;
            Ok(self.cost())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn verify_extent_closure(&mut self, debit_work: &mut dyn FnMut() -> bool) -> io::Result<()> {
        let rows_capacity = usize::try_from(self.max_frames_per_pack.min(64))
            .map_err(|_| invalid("indexed-input pack frame state range differs"))?;
        let extent_cursor_state = rows_capacity
            .checked_mul(2)
            .and_then(|count| count.checked_mul(size_of::<FrameCoordinate>()))
            .and_then(|bytes| bytes.checked_add(64 * 1024 + 8192))
            .and_then(|bytes| bytes.checked_add(size_of::<Vec<FrameCoordinate>>() * 2))
            .ok_or_else(|| invalid("indexed-input extent closure state overflow"))?;
        let retained_state = self
            .limits
            .packed_objects
            .caller_live_state_bytes
            .checked_add(self.retained_heap_state_upper_bound_bytes()?)
            .and_then(|bytes| bytes.checked_add(self.max_caller_live_state_bytes))
            .and_then(|bytes| bytes.checked_add(extent_cursor_state))
            .ok_or_else(|| invalid("indexed-input extent closure state overflow"))?;
        if retained_state >= self.limits.packed_objects.max_working_state_bytes {
            return Err(invalid(
                "indexed-input extent closure state slice exhausted",
            ));
        }
        if !debit_work() {
            return Err(invalid("indexed-input extent cursor work exhausted"));
        }

        let db = Rc::clone(&self.extents_db);
        let connection = db.borrow();
        let mut statement = connection
            .prepare(
                "SELECT digest,size,segment_digest,segment_size,frame_index,frame_count,header_offset \
                 FROM source_indexed_pack_extent_v1 ORDER BY segment_digest,frame_index",
            )
            .map_err(|_| invalid("indexed-input extent cursor refused"))?;
        let mut rows = statement
            .query([])
            .map_err(|_| invalid("indexed-input extent query refused"))?;
        let mut current_pack: Option<(Digest256, u64, u32)> = None;
        let mut coordinates = Vec::with_capacity(rows_capacity);
        let mut observed_objects = 0u64;
        let mut observed_payload_bytes = 0u64;
        let mut segment_count = 0u64;
        let mut closure_work = SegmentOperationWorkV1::default();
        loop {
            active(self.deadline, &self.cancelled)?;
            if !debit_work() {
                return Err(invalid("indexed-input extent row work exhausted"));
            }
            let Some(row) = rows
                .next()
                .map_err(|_| invalid("indexed-input extent row read refused"))?
            else {
                break;
            };
            let object_digest = digest_column(row, 0)?;
            let size = nonnegative_u64(
                row.get(1)
                    .map_err(|_| invalid("indexed-input extent size decode refused"))?,
                "object size",
            )?;
            let segment_digest = digest_column(row, 2)?;
            let segment_size = nonnegative_u64(
                row.get(3)
                    .map_err(|_| invalid("indexed-input segment size decode refused"))?,
                "segment size",
            )?;
            let frame_index = nonnegative_u32(
                row.get(4)
                    .map_err(|_| invalid("indexed-input frame index decode refused"))?,
                "frame index",
            )?;
            let frame_count = nonnegative_u32(
                row.get(5)
                    .map_err(|_| invalid("indexed-input frame count decode refused"))?,
                "frame count",
            )?;
            let header_offset = nonnegative_u64(
                row.get(6)
                    .map_err(|_| invalid("indexed-input frame offset decode refused"))?,
                "frame offset",
            )?;
            let pack_key = (segment_digest, segment_size, frame_count);
            if current_pack.is_some_and(|current| current.0 != segment_digest) {
                let current = current_pack
                    .ok_or_else(|| invalid("indexed-input pack group state differs"))?;
                let operation = verify_extent_pack(
                    &self.segment,
                    current.0,
                    current.1,
                    current.2,
                    &coordinates,
                    &self.tree_io,
                    SegmentOperationLimitsV1 {
                        max_working_state_bytes: self.limits.packed_objects.max_working_state_bytes,
                        caller_live_state_bytes: retained_state,
                        max_work_bytes: current.1,
                        max_work_units: self.work.remaining()?,
                    },
                    self.deadline,
                    &self.cancelled,
                    debit_work,
                )?;
                closure_work = add_segment_operation_work(closure_work, operation)?;
                segment_count = segment_count
                    .checked_add(1)
                    .ok_or_else(|| invalid("indexed-input pack count overflow"))?;
                coordinates.clear();
                current_pack = Some(pack_key);
            } else if let Some(current) = current_pack {
                if current != pack_key {
                    return Err(invalid("indexed-input pack extent metadata differs"));
                }
            } else {
                current_pack = Some(pack_key);
            }
            if frame_count == 0
                || frame_count > self.max_frames_per_pack
                || frame_count > self.limits.packed_objects.max_pack_frames
                || frame_count > self.limits.packed_objects.segment_limits.max_frames
                || segment_size == 0
                || segment_size > self.limits.packed_objects.segment_limits.max_segment_bytes
                || u32::try_from(coordinates.len()).ok() != Some(frame_index)
            {
                return Err(invalid(
                    "indexed-input frame sequence exceeds selected pack profile",
                ));
            }
            coordinates.push(FrameCoordinate {
                header_offset,
                size_bytes: size,
                sha256: object_digest,
            });
            if coordinates.len() > rows_capacity {
                return Err(invalid("indexed-input frame group exceeds bounded state"));
            }
            observed_objects = observed_objects
                .checked_add(1)
                .ok_or_else(|| invalid("indexed-input extent count overflow"))?;
            observed_payload_bytes = observed_payload_bytes
                .checked_add(size)
                .ok_or_else(|| invalid("indexed-input extent byte total overflow"))?;
            if observed_objects > self.expected_unique_object_count
                || observed_payload_bytes > self.expected_unique_payload_bytes
            {
                return Err(invalid("indexed-input extent rows exceed producer totals"));
            }
        }
        if let Some(current) = current_pack {
            let operation = verify_extent_pack(
                &self.segment,
                current.0,
                current.1,
                current.2,
                &coordinates,
                &self.tree_io,
                SegmentOperationLimitsV1 {
                    max_working_state_bytes: self.limits.packed_objects.max_working_state_bytes,
                    caller_live_state_bytes: retained_state,
                    max_work_bytes: current.1,
                    max_work_units: self.work.remaining()?,
                },
                self.deadline,
                &self.cancelled,
                debit_work,
            )?;
            closure_work = add_segment_operation_work(closure_work, operation)?;
            segment_count = segment_count
                .checked_add(1)
                .ok_or_else(|| invalid("indexed-input pack count overflow"))?;
        }
        if observed_objects != self.expected_unique_object_count
            || observed_payload_bytes != self.expected_unique_payload_bytes
            || observed_objects != self.objects_descriptor.entries
            || segment_count == 0
        {
            return Err(invalid("indexed-input extent closure totals differ"));
        }
        drop(rows);
        drop(statement);
        drop(connection);
        self.cost.object_reads = add_segment_operation_work(self.cost.object_reads, closure_work)?;
        Ok(())
    }

    pub(crate) fn cost(&self) -> IndexedInputCostV1 {
        let mut cost = self.cost;
        cost.shared_work_units = self.work_debits.get();
        cost
    }

    pub(crate) fn expected_totals(&self) -> (u64, u64) {
        (self.expected_member_count, self.expected_source_bytes)
    }

    /// Heap owned beyond the reader's inline value. The keyset cursor may grow
    /// to the selected maximum key width; charge that future capacity before
    /// a caller keeps this reader live across the import.
    pub(crate) fn retained_heap_state_upper_bound_bytes(&self) -> io::Result<usize> {
        let current = self.retained_state_bytes()?;
        let current_cursor = self.after.as_ref().map_or(0, Vec::capacity);
        let maximum_cursor = self
            .limits
            .member_tree
            .max_key_bytes
            .checked_mul(2)
            .ok_or_else(|| invalid("indexed-input cursor state overflow"))?;
        current
            .checked_sub(size_of::<Self>())
            .and_then(|bytes| bytes.checked_sub(current_cursor))
            .and_then(|bytes| bytes.checked_add(maximum_cursor))
            .ok_or_else(|| invalid("indexed-input retained heap state overflow"))
    }

    fn retained_state_bytes(&self) -> io::Result<usize> {
        let segment_state = size_of::<SegmentStore>()
            .checked_add(self.segment.retained_heap_state_bytes().map_err(invalid)?)
            .ok_or_else(|| invalid("indexed-input segment state overflow"))?;
        let member_descriptor = tree_descriptor_state(&self.members)?;
        let object_descriptor = tree_descriptor_state(&self.objects_descriptor)?;
        let cursor = self.after.as_ref().map_or(0, Vec::capacity);
        let members_file = self.members_file.retained_state_bytes()?;
        let objects_file = self.objects_file.retained_state_bytes()?;
        let manifest_file = self.manifest_file.retained_state_bytes()?;
        let profile_file = self.profile_file.retained_state_bytes()?;
        let dependency_closure_file = self.dependency_closure_file.retained_state_bytes()?;
        let work_debit_allocation = work_debit_allocation_state_bytes()?;
        size_of::<Self>()
            .checked_add(self.named_root.as_os_str().len())
            .and_then(|bytes| bytes.checked_add(segment_state))
            .and_then(|bytes| bytes.checked_add(member_descriptor))
            .and_then(|bytes| bytes.checked_add(object_descriptor))
            .and_then(|bytes| bytes.checked_add(members_file))
            .and_then(|bytes| bytes.checked_add(objects_file))
            .and_then(|bytes| bytes.checked_add(manifest_file))
            .and_then(|bytes| bytes.checked_add(profile_file))
            .and_then(|bytes| bytes.checked_add(dependency_closure_file))
            .and_then(|bytes| bytes.checked_add(cursor))
            .and_then(|bytes| bytes.checked_add(work_debit_allocation))
            .ok_or_else(|| invalid("indexed-input reader state overflow"))
    }

    fn verify_root_identity(&self, work: &mut dyn FnMut() -> bool) -> io::Result<()> {
        active(self.deadline, &self.cancelled)?;
        if !work() {
            return Err(invalid("indexed-input root-check work exhausted"));
        }
        charge_root_guard(&self.io_budget)?;
        let held = self.segment.physical_root_identity().map_err(invalid)?;
        active(self.deadline, &self.cancelled)?;
        if !work() {
            return Err(invalid("indexed-input root-check work exhausted"));
        }
        charge_root_guard(&self.io_budget)?;
        if held != self.segment_identity
            || directory_stamp(&self.held_root.metadata()?) != self.root_stamp
            || directory_stamp(&self.held_segment_root.metadata()?) != self.segment_root_stamp
        {
            return Err(invalid("indexed-input held root identity changed"));
        }
        verify_named_directory(
            &self.named_root.join(SEGMENT_LEAF),
            &self.held_segment_root,
            &self.io_budget,
            self.deadline,
            &self.cancelled,
            work,
        )?;
        active(self.deadline, &self.cancelled)
    }
}

fn verify_extent_pack(
    segment: &SegmentStore,
    segment_digest: Digest256,
    segment_size: u64,
    frame_count: u32,
    expected: &[FrameCoordinate],
    io: &Arc<dyn AuthenticatedTreeIoLedgerV1>,
    operation_limits: SegmentOperationLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    debit_work: &mut dyn FnMut() -> bool,
) -> io::Result<SegmentOperationWorkV1> {
    if frame_count == 0 || expected.len() != frame_count as usize {
        return Err(invalid("indexed-input pack frame cardinality differs"));
    }
    if !debit_work() {
        return Err(invalid("indexed-input whole-pack work exhausted"));
    }
    let mut work = SegmentOperationWorkV1::default();
    let actual = segment
        .verify_packed_segment_accounted(
            segment_digest,
            segment_size,
            frame_count,
            io.clone(),
            operation_limits,
            deadline,
            cancelled,
            debit_work,
            &mut work,
        )
        .map_err(invalid)?;
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual != expected)
    {
        return Err(invalid(
            "indexed-input extent rows do not match physical pack frames",
        ));
    }
    Ok(work)
}

fn digest_column(row: &rusqlite::Row<'_>, index: usize) -> io::Result<Digest256> {
    let bytes: Vec<u8> = row
        .get(index)
        .map_err(|_| invalid("indexed-input digest column decode refused"))?;
    let raw: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| invalid("indexed-input digest column width differs"))?;
    Ok(Digest256::from_bytes(raw))
}

fn nonnegative_u64(value: i64, field: &str) -> io::Result<u64> {
    u64::try_from(value).map_err(|_| match field {
        "object size" => invalid("indexed-input extent size is negative"),
        "segment size" => invalid("indexed-input segment size is negative"),
        _ => invalid("indexed-input frame offset is negative"),
    })
}

fn nonnegative_u32(value: i64, field: &str) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| match field {
        "frame index" => invalid("indexed-input frame index is invalid"),
        _ => invalid("indexed-input frame count is invalid"),
    })
}

fn add_segment_operation_work(
    total: SegmentOperationWorkV1,
    added: SegmentOperationWorkV1,
) -> io::Result<SegmentOperationWorkV1> {
    Ok(SegmentOperationWorkV1 {
        read_bytes: total
            .read_bytes
            .checked_add(added.read_bytes)
            .ok_or_else(|| invalid("indexed-input physical read cost overflow"))?,
        read_upper_bound_bytes: total
            .read_upper_bound_bytes
            .checked_add(added.read_upper_bound_bytes)
            .ok_or_else(|| invalid("indexed-input physical read guard overflow"))?,
        write_bytes: total
            .write_bytes
            .checked_add(added.write_bytes)
            .ok_or_else(|| invalid("indexed-input physical write cost overflow"))?,
        allocation_reserved_bytes: total
            .allocation_reserved_bytes
            .checked_add(added.allocation_reserved_bytes)
            .ok_or_else(|| invalid("indexed-input allocation reservation overflow"))?,
        allocated_bytes: total
            .allocated_bytes
            .checked_add(added.allocated_bytes)
            .ok_or_else(|| invalid("indexed-input allocated byte cost overflow"))?,
        work_units: total
            .work_units
            .checked_add(added.work_units)
            .ok_or_else(|| invalid("indexed-input segment work overflow"))?,
        work_bytes: total
            .work_bytes
            .checked_add(added.work_bytes)
            .ok_or_else(|| invalid("indexed-input segment work bytes overflow"))?,
    })
}

fn work_debit_allocation_state_bytes() -> io::Result<usize> {
    size_of::<Cell<u64>>()
        .checked_add(2 * size_of::<usize>())
        .ok_or_else(|| invalid("indexed-input work meter state overflow"))
}

struct IndexedTreeIo(PinnedSqliteIoBudget);

impl AuthenticatedTreeIoLedgerV1 for IndexedTreeIo {
    fn charge_read(&self, bytes: u64) -> bool {
        self.0.charge_read(bytes).is_ok()
    }

    fn record_read_returned(&self, bytes: u64) -> bool {
        self.0.record_read_returned(bytes).is_ok()
    }

    fn charge_write(&self, _bytes: u64) -> bool {
        false
    }

    fn record_write_returned(&self, _bytes: u64) -> bool {
        false
    }
}

struct HashingSink<'a> {
    inner: &'a mut dyn Write,
    hasher: Digest256Hasher,
    bytes: u64,
}

impl Write for HashingSink<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        self.bytes = self
            .bytes
            .checked_add(written as u64)
            .ok_or_else(|| invalid("indexed-input payload byte counter overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn validate_limits(
    limits: IndexedInputLimitsV1,
    member_count: u64,
    source_bytes: u64,
) -> io::Result<()> {
    if limits.max_members == 0
        || limits.max_members == u64::MAX
        || limits.max_member_bytes == 0
        || limits.max_member_bytes == u64::MAX
        || limits.max_source_bytes == 0
        || limits.max_source_bytes == u64::MAX
        || member_count > limits.max_members
        || source_bytes > limits.max_source_bytes
        || limits.max_descriptor_bytes == 0
        || limits.max_descriptor_bytes > DESCRIPTOR_MAX_BYTES
        || limits.caller_retained_state_bytes == usize::MAX
        || limits.packed_objects.max_working_state_bytes == 0
        || limits.packed_objects.max_working_state_bytes == usize::MAX
        || limits.packed_objects.max_work_units == 0
        || limits.packed_objects.max_work_units == u64::MAX
        || limits.packed_objects.max_objects == 0
        || limits.packed_objects.max_pack_frames == 0
    {
        return Err(invalid(
            "indexed-input profile exceeds the selected finite limits",
        ));
    }
    Ok(())
}

fn validate_tree(
    descriptor: &AuthenticatedTreeDescriptorV2,
    segment: &SegmentStore,
    kind: &[u8],
    entries: u64,
    limits: AuthenticatedTreeLimitsV1,
) -> io::Result<()> {
    if descriptor.store_id != segment.store_id()
        || descriptor.domain_digest != segment.domain_digest()
        || segment.custody_domain() != SOURCE_ADMISSION_V2_DOMAIN
        || descriptor.kind.as_slice() != kind
        || descriptor.entries != entries
        || descriptor.entries > limits.max_rows
        || descriptor.physical_root.is_none()
    {
        return Err(invalid("indexed-input tree binding differs"));
    }
    if limits.max_key_bytes == 0
        || limits.max_value_bytes < MEMBER_VALUE_BYTES
        || limits.max_nodes == 0
        || limits.max_total_bytes == 0
        || descriptor.kind.len() > limits.max_kind_bytes
        || descriptor.root.as_ref().is_some_and(|root| {
            root.min_key.len() > limits.max_key_bytes || root.max_key.len() > limits.max_key_bytes
        })
    {
        return Err(invalid(
            "indexed-input authenticated tree limits are invalid",
        ));
    }
    Ok(())
}

fn check_leaf(leaf: &str) -> io::Result<()> {
    if leaf.is_empty()
        || leaf == "."
        || leaf == ".."
        || leaf.contains('/')
        || leaf.contains('\\')
        || leaf.as_bytes().iter().any(|byte| *byte < 0x20)
    {
        return Err(invalid("indexed-input descriptor leaf is unsafe"));
    }
    Ok(())
}

fn absolute_path_component_count(path: &Path) -> io::Result<u64> {
    if !path.is_absolute() {
        return Err(invalid("indexed-input root path must be absolute"));
    }
    let mut components = 1u64;
    for component in path.components() {
        match component {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(_) => {
                components = components
                    .checked_add(1)
                    .ok_or_else(|| invalid("indexed-input root path component overflow"))?;
            }
            std::path::Component::CurDir
            | std::path::Component::ParentDir
            | std::path::Component::Prefix(_) => {
                return Err(invalid("indexed-input root path is not normalized"));
            }
        }
    }
    Ok(components)
}

fn read_bounded_descriptor(
    file: &mut File,
    maximum: usize,
    io_budget: &PinnedSqliteIoBudget,
    work: &mut dyn FnMut() -> bool,
) -> io::Result<Vec<u8>> {
    let reserve = maximum
        .checked_add(1)
        .ok_or_else(|| invalid("indexed-input descriptor bound overflow"))?;
    if !work() {
        return Err(invalid("indexed-input descriptor work exhausted"));
    }
    io_budget
        .charge_read(u64::try_from(reserve).map_err(invalid)?)
        .map_err(invalid)?;
    if let Err(error) = file.seek(SeekFrom::Start(0)) {
        // A failed seek consumes no payload bytes, but still closes the
        // previously charged read request in the shared ledger.
        let _ = io_budget.record_read_returned(0);
        return Err(error);
    }
    let mut raw = Vec::new();
    if raw.try_reserve_exact(reserve).is_err() {
        let _ = io_budget.record_read_returned(0);
        return Err(invalid("indexed-input descriptor allocation refused"));
    }
    let read = (&mut *file).take(reserve as u64).read_to_end(&mut raw);
    // Preserve the first read error while reconciling any bytes already
    // returned by the file operation.
    let accounting = io_budget
        .record_read_returned(raw.len() as u64)
        .map_err(invalid);
    if let Err(error) = read {
        return Err(error);
    }
    accounting?;
    if raw.len() > maximum {
        return Err(invalid(
            "indexed-input descriptor exceeds its selected bound",
        ));
    }
    Ok(raw)
}

fn hash_bounded_file(
    file: &mut File,
    maximum: usize,
    io_budget: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut dyn FnMut() -> bool,
) -> io::Result<Digest256> {
    let maximum = u64::try_from(maximum)
        .map_err(|_| invalid("indexed-input evidence bound exceeds range"))?;
    let read_limit = maximum
        .checked_add(1)
        .ok_or_else(|| invalid("indexed-input evidence bound overflow"))?;
    active(deadline, cancelled)?;
    if !work() {
        return Err(invalid("indexed-input evidence work ceiling exhausted"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = [0u8; EVIDENCE_HASH_BUFFER_BYTES];
    let mut hasher = Digest256Hasher::new();
    let mut total = 0u64;
    loop {
        active(deadline, cancelled)?;
        let remaining = read_limit
            .checked_sub(total)
            .ok_or_else(|| invalid("indexed-input evidence byte count overflow"))?;
        if remaining == 0 {
            return Err(invalid("indexed-input evidence exceeds its selected bound"));
        }
        if !work() {
            return Err(invalid("indexed-input evidence work ceiling exhausted"));
        }
        let requested = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| invalid("indexed-input evidence read request exceeds range"))?;
        io_budget.charge_read(requested as u64).map_err(invalid)?;
        let read = match file.read(&mut buffer[..requested]) {
            Ok(read) => read,
            Err(error) => {
                let _ = io_budget.record_read_returned(0);
                return Err(error);
            }
        };
        io_budget
            .record_read_returned(read as u64)
            .map_err(invalid)?;
        active(deadline, cancelled)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| invalid("indexed-input evidence byte count overflow"))?;
        if total > maximum {
            return Err(invalid("indexed-input evidence exceeds its selected bound"));
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize())
}

fn charge_root_guard(io_budget: &PinnedSqliteIoBudget) -> io::Result<()> {
    io_budget
        .charge_read_upper_bound(ROOT_GUARD_BYTES)
        .map_err(invalid)
}

fn debit_work(work: &AdmissionWorkBudget, local: &Cell<u64>) -> bool {
    if work.charge(()).is_err() {
        return false;
    }
    let Some(next) = local.get().checked_add(1) else {
        return false;
    };
    local.set(next);
    true
}

fn add_authenticated_tree_work(
    mut current: AuthenticatedTreeWorkV1,
    next: AuthenticatedTreeWorkV1,
) -> io::Result<AuthenticatedTreeWorkV1> {
    current.read_nodes = current
        .read_nodes
        .checked_add(next.read_nodes)
        .ok_or_else(|| invalid("indexed-input object-tree node count overflow"))?;
    current.written_nodes = current
        .written_nodes
        .checked_add(next.written_nodes)
        .ok_or_else(|| invalid("indexed-input object-tree node count overflow"))?;
    current.read_bytes = current
        .read_bytes
        .checked_add(next.read_bytes)
        .ok_or_else(|| invalid("indexed-input object-tree byte count overflow"))?;
    current.written_bytes = current
        .written_bytes
        .checked_add(next.written_bytes)
        .ok_or_else(|| invalid("indexed-input object-tree byte count overflow"))?;
    current.allocated_bytes = current
        .allocated_bytes
        .checked_add(next.allocated_bytes)
        .ok_or_else(|| invalid("indexed-input object-tree allocation count overflow"))?;
    Ok(current)
}

fn verify_named_directory(
    named_root: &Path,
    held_root: &File,
    io_budget: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut dyn FnMut() -> bool,
) -> io::Result<()> {
    active(deadline, cancelled)?;
    if !work() {
        return Err(invalid("indexed-input root-check work exhausted"));
    }
    charge_root_guard(io_budget)?;
    let held = held_root.metadata()?;
    active(deadline, cancelled)?;
    if !work() {
        return Err(invalid("indexed-input root-check work exhausted"));
    }
    charge_root_guard(io_budget)?;
    let named = fs::symlink_metadata(named_root)?;
    if !held.is_dir()
        || held.uid() != rustix::process::geteuid().as_raw()
        || held.mode() & 0o022 != 0
        || !named.is_dir()
        || named.uid() != rustix::process::geteuid().as_raw()
        || named.mode() & 0o022 != 0
        || held.dev() != named.dev()
        || held.ino() != named.ino()
        || directory_stamp(&held) != directory_stamp(&named)
    {
        return Err(invalid("indexed-input named container root changed"));
    }
    active(deadline, cancelled)
}

fn directory_stamp(metadata: &fs::Metadata) -> (u64, u64, u32, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn tree_descriptor_state(tree: &AuthenticatedTreeDescriptorV2) -> io::Result<usize> {
    let mut state = size_of::<AuthenticatedTreeDescriptorV2>()
        .checked_add(tree.kind.capacity())
        .ok_or_else(|| invalid("indexed-input tree root state overflow"))?;
    if let Some(root) = &tree.root {
        state = state
            .checked_add(size_of::<tos_segment_store::AuthenticatedTreeNodeRefV1>())
            .and_then(|bytes| bytes.checked_add(root.min_key.capacity()))
            .and_then(|bytes| bytes.checked_add(root.max_key.capacity()))
            .ok_or_else(|| invalid("indexed-input tree root state overflow"))?;
    }
    if tree.physical_root.is_some() {
        state = state
            .checked_add(size_of::<tos_segment_store::AuthenticatedTreeLocatorV2>())
            .ok_or_else(|| invalid("indexed-input physical root state overflow"))?;
    }
    Ok(state)
}

fn input_identity(
    root_stamp: (u64, u64, u32, i64, i64, i64, i64),
    segment_identity: (u64, u64),
    manifest: Digest256,
    profile: Digest256,
    dependency_closure: Digest256,
    members: Digest256,
    objects: Digest256,
    count: u64,
    source_bytes: u64,
) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-command-indexed-source-input-v1\0");
    for value in [root_stamp.0, root_stamp.1, u64::from(root_stamp.2)]
        .into_iter()
        .chain([
            root_stamp.3 as u64,
            root_stamp.4 as u64,
            root_stamp.5 as u64,
            root_stamp.6 as u64,
        ])
        .chain([segment_identity.0, segment_identity.1, count, source_bytes])
    {
        hash.update(&value.to_le_bytes());
    }
    hash.update(members.as_bytes());
    hash.update(objects.as_bytes());
    hash.update(manifest.as_bytes());
    hash.update(profile.as_bytes());
    hash.update(dependency_closure.as_bytes());
    hash.finalize()
}
