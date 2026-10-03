//! Native release companion for independently produced selected expectations.
//! The release holder owns current selection and disclosure policy. This
//! carrier proves producer mechanics and kernel custody, not source admission.

use crate::{
    ColdOpenLimits, Error, ImmutableKnowledgeCustody, KnowledgeRegistry, KnowledgeSealReceipt,
    KnowledgeSelectedExpectation, NavigationOriginalReceipt, QueryVocabulary, Result,
    knowledge_selected, knowledge_stage::StageReceipt,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs::File, io::Write, os::fd::AsRawFd, path::Path};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, RelativePath, canonical_bytes_v1, parse_json,
};

const SCHEMA: &str = "tos_access_native_knowledge_selection_v1";
const PHILOSOPHY_SCHEMA: &str = "tos_access_native_knowledge_selection_v2";
const CORPUS_SCHEMA: &str = "tos_access_native_knowledge_selection_v3";
const PROFILE: &str = "managed-local-linux-fsverity-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSelectionPaths {
    pub model: String,
    pub descriptor: String,
    pub entity_registry: String,
    pub relation_registry: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSelectionProducer {
    pub stage: StageReceipt,
    pub seal: KnowledgeSealReceipt,
    pub navigation_original: Option<NavigationOriginalReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub philosophy_original: Option<crate::PhilosophyOriginalReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corpus_original: Option<crate::CorpusOriginalReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_source: Option<crate::ManagedSourceProofV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_source_v2: Option<crate::ManagedSourceProofV2>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFsVerityMeasurement {
    pub algorithm: String,
    pub digest: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeProcessLimits {
    pub address_space_bytes: u64,
    pub file_size_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    schema: String,
    profile: String,
    paths: NativeSelectionPaths,
    producer: NativeSelectionProducer,
    expectation: KnowledgeSelectedExpectation,
    fs_verity: NativeFsVerityMeasurement,
    cold_limits: ColdOpenLimits,
    process_limits: NativeProcessLimits,
}
#[derive(Clone, Debug)]
pub struct NativeKnowledgeSelection {
    packet: Packet,
    vocabulary: QueryVocabulary,
}
impl NativeKnowledgeSelection {
    /// All receipts are producer outputs, captured before selected cold open.
    /// No expectation is read from the SQLite file being verified.
    pub fn from_producer(
        paths: NativeSelectionPaths,
        producer: NativeSelectionProducer,
        expectation: KnowledgeSelectedExpectation,
        fs_verity: NativeFsVerityMeasurement,
        cold_limits: ColdOpenLimits,
        process_limits: NativeProcessLimits,
        descriptor_raw: &[u8],
        entity_raw: &[u8],
        relation_raw: &[u8],
        supported_profiles: &[&str],
        max_bytes: usize,
    ) -> Result<Self> {
        let packet = Packet {
            schema: if producer.managed_source.is_some() || producer.managed_source_v2.is_some() {
                crate::managed_source::MANAGED_SELECTION_SCHEMA
            } else if producer.corpus_original.is_some() {
                CORPUS_SCHEMA
            } else if producer.philosophy_original.is_some() {
                PHILOSOPHY_SCHEMA
            } else {
                SCHEMA
            }
            .into(),
            profile: PROFILE.into(),
            paths,
            producer,
            expectation,
            fs_verity,
            cold_limits,
            process_limits,
        };
        let raw = encode_packet(&packet, max_bytes)?;
        Self::decode(
            &raw,
            descriptor_raw,
            entity_raw,
            relation_raw,
            supported_profiles,
            max_bytes,
        )
    }
    pub fn decode(
        raw: &[u8],
        descriptor_raw: &[u8],
        entity_raw: &[u8],
        relation_raw: &[u8],
        supported_profiles: &[&str],
        max_bytes: usize,
    ) -> Result<Self> {
        strict(raw, max_bytes)?;
        let packet: Packet =
            serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))?;
        validate_packet(&packet)?;
        if supported_profiles
            .iter()
            .any(|p| !crate::NATIVE_KNOWLEDGE_ADAPTER_PROFILES.contains(p))
        {
            return Err(Error::Invalid(
                "native unsupported compiled adapter capability",
            ));
        }
        let vocabulary = QueryVocabulary::parse(descriptor_raw, supported_profiles)?;
        let registry = KnowledgeRegistry::parse(entity_raw, relation_raw)?;
        let e = &packet.expectation;
        if vocabulary.descriptor_sha256 != e.descriptor_sha256
            || vocabulary.descriptor_version != e.descriptor_version
            || vocabulary.semantic_primitive_profile != e.semantic_primitive_profile
            || vocabulary.entity_registry_id != e.entity_registry_id
            || vocabulary.relation_registry_id != e.relation_registry_id
            || registry.entity_sha256 != e.entity_registry_sha256
            || registry.relation_sha256 != e.relation_registry_sha256
            || registry.entity_registry_id != e.entity_registry_id
            || registry.relation_registry_id != e.relation_registry_id
            || registry.entity_registry_version.to_string() != e.entity_registry_version
            || registry.relation_registry_version.to_string() != e.relation_registry_version
            || vocabulary.registered_source_ids.len() != e.source_scopes.len()
        {
            return Err(Error::Invalid(
                "native companion descriptor/registry binding",
            ));
        }
        for (id, scope) in vocabulary
            .registered_source_ids
            .iter()
            .zip(&e.source_scopes)
        {
            let source = vocabulary
                .sources
                .iter()
                .find(|s| &s.source_graph_id == id)
                .ok_or(Error::Invalid("native companion source registration"))?;
            if id != &scope.source_graph
                || source.input_role != scope.input_role
                || source.adapter_profile != scope.adapter_profile
            {
                return Err(Error::Invalid("native companion exact source scope"));
            }
        }
        Ok(Self { packet, vocabulary })
    }
    pub fn encode(&self, max_bytes: usize) -> Result<Vec<u8>> {
        encode_packet(&self.packet, max_bytes)
    }
    pub fn paths(&self) -> &NativeSelectionPaths {
        &self.packet.paths
    }
    pub fn producer(&self) -> &NativeSelectionProducer {
        &self.packet.producer
    }
    pub fn expectation(&self) -> &KnowledgeSelectedExpectation {
        &self.packet.expectation
    }
    pub fn vocabulary(&self) -> &QueryVocabulary {
        &self.vocabulary
    }
    pub fn cold_limits(&self) -> ColdOpenLimits {
        self.packet.cold_limits
    }
    pub fn process_limits(&self) -> NativeProcessLimits {
        self.packet.process_limits
    }
    pub fn fs_verity(&self) -> &NativeFsVerityMeasurement {
        &self.packet.fs_verity
    }
}
fn strict(raw: &[u8], max_bytes: usize) -> Result<()> {
    if max_bytes == 0 || max_bytes > JsonLimits::default().max_bytes {
        return Err(Error::Budget("native companion metadata limit"));
    }
    let mut limits = JsonLimits::default();
    limits.max_bytes = max_bytes;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    Ok(())
}
fn measurement(value: &NativeFsVerityMeasurement) -> Result<()> {
    if value.algorithm != "sha256" {
        return Err(Error::Invalid("native fs-verity algorithm"));
    }
    Digest256::from_hex(&value.digest).map_err(|_| Error::Invalid("native fs-verity digest"))?;
    Ok(())
}
fn process(value: NativeProcessLimits) -> Result<()> {
    if value.address_space_bytes == 0
        || value.file_size_bytes == 0
        || value.address_space_bytes >= libc::RLIM_INFINITY as u64
        || value.file_size_bytes >= libc::RLIM_INFINITY as u64
    {
        return Err(Error::Budget("native finite process limits"));
    }
    Ok(())
}
fn validate_packet(p: &Packet) -> Result<()> {
    let schema = if p.expectation.model_abi == crate::KNOWLEDGE_MANAGED_MODEL_ABI {
        crate::managed_source::MANAGED_SELECTION_SCHEMA
    } else if p.expectation.model_abi == crate::KNOWLEDGE_CORPUS_MODEL_ABI {
        CORPUS_SCHEMA
    } else if p.expectation.model_abi == crate::KNOWLEDGE_PHILOSOPHY_MODEL_ABI {
        PHILOSOPHY_SCHEMA
    } else {
        SCHEMA
    };
    if p.schema != schema || p.profile != PROFILE {
        return Err(Error::Invalid("native companion version/profile"));
    }
    let mut paths = BTreeSet::new();
    for path in [
        &p.paths.model,
        &p.paths.descriptor,
        &p.paths.entity_registry,
        &p.paths.relation_registry,
    ] {
        RelativePath::parse(path).map_err(|_| Error::Invalid("native companion member path"))?;
        if !paths.insert(path) {
            return Err(Error::Invalid("native companion distinct members"));
        }
    }
    measurement(&p.fs_verity)?;
    process(p.process_limits)?;
    knowledge_selected::validate(&p.expectation, p.cold_limits)?;
    let e = &p.expectation;
    let s = &p.producer.stage;
    let seal = &p.producer.seal;
    s.binding.validate()?;
    if s.binding.source_cut != e.source_cut
        || s.source_cut != e.source_cut
        || s.binding.membership_root != e.membership_root
        || s.membership_root != e.membership_root
        || s.binding.through_commit_seq != e.through_commit_seq
        || s.binding.index_generation != e.index_generation
        || s.binding.route_map_version != e.route_map_version
        || s.binding.reader_abi != e.reader_abi
        || s.sqlite_sha256 != e.model_sha256
        || s.sqlite_size_bytes != e.model_size_bytes
        || s.node_rows != e.node_count
        || s.relation_rows != e.relation_count
        || seal.model_abi != e.model_abi
        || seal.managed_source_root_sha256 != e.managed_source_root_sha256
        || seal.node_count != e.node_count
        || seal.relation_count != e.relation_count
        || s.node_root_sha256 != seal.node_root_sha256
        || s.relation_root_sha256 != seal.relation_root_sha256
        || seal.graph_root_sha256 != e.graph_root_sha256
        || seal.catalog_packet_sha256 != e.catalog_packet_sha256
        || seal.catalog_index_root_sha256 != e.catalog_index_root_sha256
        || seal.source_scope_root_sha256 != e.source_scope_root_sha256
        || seal.search_index_root_sha256 != e.search_index_root_sha256
        || seal.navigation_original_root_sha256 != e.navigation_original_root_sha256
        || seal.philosophy_original_root_sha256 != e.philosophy_original_root_sha256
        || seal.corpus_original_root_sha256 != e.corpus_original_root_sha256
        || s.input_collections != s.verified_inputs.len()
    {
        return Err(Error::Invalid(
            "native independent producer receipt binding",
        ));
    }
    match (
        &p.producer.managed_source,
        &p.producer.managed_source_v2,
        &e.managed_source_root_sha256,
    ) {
        (None, None, None) => (),
        (Some(proof), None, Some(root)) if &proof.root_sha256()? == root => {
            proof.check_binding(&e.source_cut, &e.membership_root, e.through_commit_seq)?;
        }
        (None, Some(proof), Some(root)) if &proof.root_sha256()? == root => {
            proof.check_binding(&e.source_cut, &e.membership_root, e.through_commit_seq)?;
        }
        _ => {
            return Err(Error::Invalid(
                "native managed source independent proof binding",
            ));
        }
    }
    let exact = crate::knowledge_stage::ExactInputReceipt {
        binding: s.binding.clone(),
        collections: s.verified_inputs.clone(),
    };
    exact.validate()?;
    let rows = s
        .verified_inputs
        .iter()
        .try_fold(0u64, |sum, r| sum.checked_add(r.expected_count))
        .ok_or(Error::Budget("native producer input count"))?;
    if rows != s.input_rows {
        return Err(Error::Invalid("native producer complete input count"));
    }
    for root in [
        &seal.graph_header_sha256,
        &seal.node_root_sha256,
        &seal.relation_root_sha256,
    ] {
        Digest256::from_hex(root).map_err(|_| Error::Invalid("native producer digest"))?;
    }
    match (
        &p.producer.navigation_original,
        &e.navigation_original_root_sha256,
    ) {
        (None, None) => (),
        (Some(r), Some(root))
            if &r.component_root_sha256 == root
                && r.profile == crate::NAVIGATION_ORIGINAL_PROFILE
                && r.descriptor_sha256 == e.descriptor_sha256
                && r.source_cut == e.source_cut
                && r.membership_root == e.membership_root =>
        {
            crate::knowledge_navigation_original::validate_producer_receipt(r, &s.verified_inputs)?;
        }
        _ => return Err(Error::Invalid("native producer original receipt binding")),
    }
    match (
        &p.producer.philosophy_original,
        &e.philosophy_original_root_sha256,
    ) {
        (None, None) => (),
        (Some(r), Some(root))
            if &r.component_root_sha256 == root
                && r.descriptor_sha256 == e.descriptor_sha256
                && r.source_cut == e.source_cut
                && r.membership_root == e.membership_root =>
        {
            crate::knowledge_philosophy_original::validate_producer_receipt(r, &s.verified_inputs)?;
        }
        _ => {
            return Err(Error::Invalid(
                "native producer philosophy original receipt binding",
            ));
        }
    }
    match (&p.producer.corpus_original, &e.corpus_original_root_sha256) {
        (None, None) => (),
        (Some(r), Some(root))
            if &r.component_root_sha256 == root
                && r.descriptor_sha256 == e.descriptor_sha256
                && r.source_cut == e.source_cut
                && r.membership_root == e.membership_root =>
        {
            crate::knowledge_corpus_original::validate_receipt(r)?
        }
        _ => {
            return Err(Error::Invalid(
                "native producer corpus original receipt binding",
            ));
        }
    }
    Ok(())
}
struct CappedWriter {
    bytes: Vec<u8>,
    max: usize,
}
impl Write for CappedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(data.len())
            .is_none_or(|n| n > self.max)
        {
            return Err(std::io::Error::other("native companion bytes"));
        }
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode_packet(p: &Packet, max_bytes: usize) -> Result<Vec<u8>> {
    validate_packet(p)?;
    let mut writer = CappedWriter {
        bytes: Vec::new(),
        max: max_bytes,
    };
    serde_json::to_writer(&mut writer, p).map_err(|_| Error::Budget("native companion bytes"))?;
    strict(&writer.bytes, max_bytes)?;
    let mut limits = JsonLimits::default();
    limits.max_bytes = max_bytes;
    let parsed = parse_json(&writer.bytes, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let mut raw = canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if raw.len() >= max_bytes {
        return Err(Error::Budget("native companion newline bytes"));
    }
    raw.push(b'\n');
    Ok(raw)
}

/// Concrete kernel-backed custody for the managed native profile. Measurement
/// is checked on the retained FD; raw SHA is checked once by the cold opener.
pub struct LinuxFsVerityCustody {
    measurement: NativeFsVerityMeasurement,
    process: NativeProcessLimits,
}
impl LinuxFsVerityCustody {
    pub fn new(value: NativeFsVerityMeasurement, limits: NativeProcessLimits) -> Result<Self> {
        measurement(&value)?;
        process(limits)?;
        Ok(Self {
            measurement: value,
            process: limits,
        })
    }
}
impl ImmutableKnowledgeCustody for LinuxFsVerityCustody {
    fn verify(&self, pinned: &File, _: &KnowledgeSelectedExpectation) -> Result<()> {
        finite_soft_limit(libc::RLIMIT_AS as u32, self.process.address_space_bytes)?;
        finite_soft_limit(libc::RLIMIT_FSIZE as u32, self.process.file_size_bytes)?;
        if measured(pinned)? != self.measurement {
            return Err(Error::Invalid("native fs-verity measurement changed"));
        }
        Ok(())
    }
    fn verify_cold_resources(&self, _: ColdOpenLimits) -> Result<()> {
        finite_soft_limit(libc::RLIMIT_AS as u32, self.process.address_space_bytes)?;
        finite_soft_limit(libc::RLIMIT_FSIZE as u32, self.process.file_size_bytes)
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn finite_soft_limit(resource: u32, maximum: u64) -> Result<()> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(resource as _, &mut limit) } != 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    if limit.rlim_cur == 0
        || limit.rlim_cur == libc::RLIM_INFINITY
        || limit.rlim_cur as u64 > maximum
    {
        return Err(Error::Budget("native live process limit absent or wider"));
    }
    Ok(())
}
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn finite_soft_limit(_: u32, _: u64) -> Result<()> {
    Err(Error::Invalid("native fs-verity host profile unsupported"))
}
#[repr(C)]
struct VerityDigest {
    algorithm: u16,
    size: u16,
    digest: [u8; 32],
}
fn measured(file: &File) -> Result<NativeFsVerityMeasurement> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let mut digest = VerityDigest {
            algorithm: 0,
            size: 32,
            digest: [0; 32],
        };
        // Linux x86_64 UAPI _IOWR('f',134,struct fsverity_digest), whose
        // flexible-array header is four bytes, not this output buffer's size.
        if unsafe { libc::ioctl(file.as_raw_fd(), 0xc0046686 as libc::c_ulong, &mut digest) } != 0 {
            return Err(Error::Io(std::io::Error::last_os_error()));
        }
        if digest.algorithm != 1 || digest.size != 32 {
            return Err(Error::Invalid("native measured fs-verity profile"));
        }
        let hex = digest
            .digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        return Ok(NativeFsVerityMeasurement {
            algorithm: "sha256".into(),
            digest: hex,
        });
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = file;
        Err(Error::Invalid("native fs-verity host profile unsupported"))
    }
}
/// Software-level preparation only. The caller must close writers first and
/// supply a private admitted candidate. This never changes a release pointer.
pub fn prepare_native_knowledge_artifact(
    path: &Path,
    receipt: &StageReceipt,
) -> Result<NativeFsVerityMeasurement> {
    if !path.is_absolute() {
        return Err(Error::Invalid("native artifact absolute path"));
    }
    let mut file = crate::safe_open::open_regular(path, receipt.sqlite_size_bytes)?;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        #[repr(C)]
        struct Enable {
            version: u32,
            algorithm: u32,
            block: u32,
            salt_size: u32,
            salt: u64,
            signature_size: u32,
            reserved: u32,
            signature: u64,
            unused: [u64; 11],
        }
        let arg = Enable {
            version: 1,
            algorithm: 1,
            block: 4096,
            salt_size: 0,
            salt: 0,
            signature_size: 0,
            reserved: 0,
            signature: 0,
            unused: [0; 11],
        };
        // Linux x86_64 UAPI _IOW('f',133,struct fsverity_enable_arg), size128.
        if unsafe { libc::ioctl(file.as_raw_fd(), 0x40806685 as libc::c_ulong, &arg) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EEXIST) {
                return Err(Error::Io(error));
            }
        }
    }
    let measurement = measured(&file)?;
    let (sha, size) = crate::stream_digest(&mut file)?;
    if sha != receipt.sqlite_sha256 || size != receipt.sqlite_size_bytes {
        return Err(Error::Invalid(
            "native verity-frozen producer artifact bytes",
        ));
    }
    file.sync_all()?;
    File::open(
        path.parent()
            .ok_or(Error::Invalid("native artifact parent"))?,
    )?
    .sync_all()?;
    Ok(measurement)
}
