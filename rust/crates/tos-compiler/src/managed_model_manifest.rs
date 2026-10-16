//! Opt-in immutable model manifests; current source authority stays with CMD's
//! same-transaction selection/fences. An object digest alone grants no read.
use crate::source_bibliographic_render::encode;
use crate::{Error, KnowledgeSelectedExpectation, ManagedSourceProofV2, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;
use tos_segment_store::{
    AuditedStoreRoot, AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1, SegmentStore,
};

const KIND: &[u8] = b"tos.cmp.managed-model-manifest.v2";
const SCHEMA: &str = "tos_managed_model_manifest_v2";

const BASE_CHUNK_KIND: &[u8] = b"tos.cmp.managed-model-base-chunk.v2";
const BASE_CHUNK_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManagedBaseTransportV2 {
    stage: crate::knowledge_stage::StageReceipt,
    // Exact ordered STO object digests and payload lengths, not raw file paths.
    chunks: Vec<(String, u64)>,
}
impl ManagedBaseTransportV2 {
    pub(crate) fn import_pinned(
        base: &crate::VerifiedKnowledgeModel<'_>,
        stage: &crate::knowledge_stage::StageReceipt,
        store: &SegmentStore,
        max_base_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Self, AuthenticatedTreeWorkV1)> {
        if stage.sqlite_size_bytes == 0
            || stage.sqlite_size_bytes > max_base_bytes
            || stage.sqlite_size_bytes != base.selection().model_size_bytes
            || stage.sqlite_sha256 != base.selection().model_sha256
        {
            return Err(Error::Invalid("managed cold base transport identity/bound"));
        }
        let mut chunks = Vec::new();
        let mut offset = 0u64;
        let mut digest = tos_foundation::Digest256Hasher::new();
        let mut work = AuthenticatedTreeWorkV1::default();
        while offset < stage.sqlite_size_bytes {
            if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(Error::Budget("managed cold base transport deadline"));
            }
            let len = (stage.sqlite_size_bytes - offset).min(BASE_CHUNK_BYTES as u64) as usize;
            let mut raw = vec![0; len];
            base.read_pinned_chunk(offset, &mut raw)?;
            digest.update(&raw);
            let (object, object_work) = store
                .install_authenticated_object_v1_with_work(
                    BASE_CHUNK_KIND,
                    &raw,
                    BASE_CHUNK_BYTES + 65_536,
                    deadline,
                    cancelled,
                )
                .map_err(|e| Error::Source(e.to_string()))?;
            work.read_nodes = work
                .read_nodes
                .checked_add(object_work.read_nodes)
                .ok_or(Error::Budget("base transport nodes"))?;
            work.read_bytes = work
                .read_bytes
                .checked_add(object_work.read_bytes)
                .ok_or(Error::Budget("base transport bytes"))?;
            work.written_nodes = work
                .written_nodes
                .checked_add(object_work.written_nodes)
                .ok_or(Error::Budget("base transport nodes"))?;
            work.written_bytes = work
                .written_bytes
                .checked_add(object_work.written_bytes)
                .ok_or(Error::Budget("base transport bytes"))?;
            chunks.push((object.to_hex(), len as u64));
            offset += len as u64;
        }
        if digest.finalize().to_hex() != stage.sqlite_sha256 {
            return Err(Error::Invalid("managed cold pinned base digest changed"));
        }
        base.check_pin()?;
        Ok((
            Self {
                stage: stage.clone(),
                chunks,
            },
            work,
        ))
    }
    fn consume_chunks(
        &self,
        store: &SegmentStore,
        mut output: Option<&mut std::fs::File>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeWorkV1> {
        use std::io::Write;
        let mut digest = tos_foundation::Digest256Hasher::new();
        let mut work = AuthenticatedTreeWorkV1::default();
        for (expected, len) in &self.chunks {
            let expected = Digest256::from_hex(expected)
                .map_err(|_| Error::Invalid("retained base chunk SHA"))?;
            let (raw, chunk_work) = store
                .read_authenticated_object_v1_with_work(
                    BASE_CHUNK_KIND,
                    expected,
                    BASE_CHUNK_BYTES + 65_536,
                    deadline,
                    cancelled,
                )
                .map_err(|e| Error::Source(e.to_string()))?;
            if raw.len() as u64 != *len {
                return Err(Error::Invalid("retained base chunk length"));
            }
            digest.update(&raw);
            if let Some(file) = output.as_deref_mut() {
                file.write_all(&raw)?;
            }
            work.read_nodes = work
                .read_nodes
                .checked_add(chunk_work.read_nodes)
                .ok_or(Error::Budget("retained base chunk nodes"))?;
            work.read_bytes = work
                .read_bytes
                .checked_add(chunk_work.read_bytes)
                .ok_or(Error::Budget("retained base chunk bytes"))?;
        }
        if digest.finalize().to_hex() != self.stage.sqlite_sha256 {
            return Err(Error::Invalid("retained base exact digest differs"));
        }
        Ok(work)
    }
    pub(crate) fn retained_encoded_bound(&self) -> Result<u64> {
        (self.chunks.len() as u64)
            .checked_mul(65_536)
            .and_then(|overhead| self.stage.sqlite_size_bytes.checked_add(overhead))
            .ok_or(Error::Budget("managed retained base encoded bound"))
    }
    fn validate(&self, base: &KnowledgeSelectedExpectation, max_bytes: u64) -> Result<()> {
        if self.stage.sqlite_size_bytes == 0
            || self.stage.sqlite_size_bytes > max_bytes
            || self.stage.sqlite_sha256 != base.model_sha256
            || self.stage.sqlite_size_bytes != base.model_size_bytes
            || self.chunks.is_empty()
        {
            return Err(Error::Invalid("managed retained base identity/bound"));
        }
        let mut total = 0u64;
        for (i, (digest, bytes)) in self.chunks.iter().enumerate() {
            Digest256::from_hex(digest).map_err(|_| Error::Invalid("managed base chunk digest"))?;
            if *bytes == 0
                || *bytes > BASE_CHUNK_BYTES as u64
                || (i + 1 < self.chunks.len() && *bytes != BASE_CHUNK_BYTES as u64)
            {
                return Err(Error::Invalid("managed base chunk extent"));
            }
            total = total
                .checked_add(*bytes)
                .ok_or(Error::Budget("managed base bytes"))?;
            if total > max_bytes {
                return Err(Error::Budget("managed base transport bound"));
            }
        }
        if total != base.model_size_bytes {
            return Err(Error::Invalid("managed base transport length"));
        }
        Ok(())
    }
}

/// Logical retained-object bounds are distinct from the host's whole physical
/// storage reservation/quota. Cold compaction is explicit when either bound
/// is reached; it must not retire a historical model with a live pin.
#[derive(Clone, Copy)]
pub struct ManagedManifestLimitsV2 {
    pub max_manifest_bytes: usize,
    pub max_retained_generations: u64,
    pub max_retained_logical_bound_bytes: u64,
}
impl ManagedManifestLimitsV2 {
    fn validate(self) -> Result<()> {
        if self.max_manifest_bytes == 0
            || self.max_manifest_bytes > 1_048_576
            || self.max_retained_generations == 0
            || self.max_retained_generations > 256
            || self.max_retained_logical_bound_bytes == 0
        {
            return Err(Error::Budget("managed manifest limits"));
        }
        Ok(())
    }
}

/// Separate versioned delta: never reinterpret V1 parent-model SHA/size as a
/// manifest identity. Genuine source commit evidence is supplied by CMD.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedManifestDeltaV2 {
    pub parent_manifest_digest: String,
    pub parent_source_binding_sha256: String,
    pub parent_through_commit_seq: u64,
    pub committed_delta_sha256: String,
    pub committed_member_root_sha256: String,
}

/// Actual addressed source selection facts. This descriptor is not a full
/// catalog-transcript proof and cannot issue current source authority alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedOverlaySourceBindingV2 {
    pub schema: String,
    pub domain: String,
    pub store_id: [u8; 16],
    pub domain_sha256: String,
    pub selected_generation_digest: String,
    pub database_oid: u64,
    pub state_profile_sha256: String,
    pub log_sha256: String,
    pub bootstrap_source_revision: String,
    pub bootstrap_membership_sha256: String,
    pub bootstrap_members: u64,
    pub selected_audit_generation: u64,
    pub through_commit_seq: u64,
    pub epoch: u64,
    pub definition_sha256: String,
    pub schema_profile_sha256: String,
    pub addressed_current_tree_sha256: String,
    pub addressed_history_tree_sha256: String,
    pub addressed_metadata_tree_sha256: String,
    pub addressed_inventory_root_sha256: String,
    pub current_source_members: u64,
    pub historical_source_members: u64,
    pub metadata_members: u64,
}
impl ManagedOverlaySourceBindingV2 {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.schema != "tos_managed_model_source_binding_v2"
            || self.domain.is_empty()
            || self.domain.len() > 4096
            || self.epoch == 0
            || self.database_oid == 0
        {
            return Err(Error::Invalid("managed addressed source binding shape"));
        }
        for digest in [
            &self.domain_sha256,
            &self.state_profile_sha256,
            &self.log_sha256,
            &self.bootstrap_source_revision,
            &self.bootstrap_membership_sha256,
            &self.selected_generation_digest,
            &self.definition_sha256,
            &self.schema_profile_sha256,
            &self.addressed_current_tree_sha256,
            &self.addressed_history_tree_sha256,
            &self.addressed_metadata_tree_sha256,
            &self.addressed_inventory_root_sha256,
        ] {
            Digest256::from_hex(digest)
                .map_err(|_| Error::Invalid("managed addressed source binding digest"))?;
        }
        Ok(())
    }
    pub(crate) fn digest(&self) -> Result<Digest256> {
        self.validate()?;
        Ok(Digest256::of_bytes(&encode(
            &serde_json::to_value(self).map_err(|e| Error::Source(e.to_string()))?,
            65_536,
        )?))
    }
}

/// Descriptive transition facts only. CMD requires the non-deserializable
/// witness from its successful existing restore before publishing this kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedManifestRecoveryRebindV2 {
    pub parent_manifest_digest: String,
    pub parent_source_binding_sha256: String,
    pub old_generation_digest: String,
    pub backup_receipt_sha256: String,
    pub restored_metadata_sha256: String,
    pub store_inventory_sha256: String,
    pub restored_cut_digest: String,
    pub old_database_oid: u64,
    pub new_database_oid: u64,
    pub old_audit_generation: u64,
    pub new_audit_generation: u64,
}
impl ManagedManifestRecoveryRebindV2 {
    fn validate(&self) -> Result<()> {
        for value in [
            &self.parent_manifest_digest,
            &self.parent_source_binding_sha256,
            &self.old_generation_digest,
            &self.backup_receipt_sha256,
            &self.restored_metadata_sha256,
            &self.store_inventory_sha256,
            &self.restored_cut_digest,
        ] {
            Digest256::from_hex(value)
                .map_err(|_| Error::Invalid("managed recovery rebind digest"))?;
        }
        if self.old_database_oid == 0
            || self.new_database_oid == 0
            || self.old_database_oid == self.new_database_oid
            || self.new_audit_generation < self.old_audit_generation
        {
            return Err(Error::Invalid(
                "managed recovery distinct database identity",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedManifestV2 {
    pub(crate) schema: String,
    pub(crate) base: KnowledgeSelectedExpectation,
    pub(crate) base_provenance: ManagedSourceProofV2,
    pub(crate) base_transport: ManagedBaseTransportV2,
    pub(crate) schema_worker_sha256: String,
    pub(crate) schema_set_sha256: String,
    pub(crate) source: ManagedOverlaySourceBindingV2,
    pub(crate) catalog_epoch: Vec<u8>,
    pub(crate) navigation_tree: Vec<u8>,
    pub(crate) delta: Option<ManagedManifestDeltaV2>,
    pub(crate) recovery_rebind: Option<ManagedManifestRecoveryRebindV2>,
    pub(crate) recovery_rebinds: u64,
    pub(crate) retained_generations: u64,
    /// Includes the immutable SQLite base plus all retained authored object
    /// frames and tree nodes; a conservative logical upper bound, not blocks.
    pub(crate) retained_logical_bound_bytes: u64,
}
impl ManagedManifestV2 {
    pub fn source_binding_digest(&self) -> Result<Digest256> {
        self.source.digest()
    }
    pub fn source_binding(&self) -> &ManagedOverlaySourceBindingV2 {
        &self.source
    }
    pub fn require_successor_of(&self, parent: &Self, parent_digest: Digest256) -> Result<()> {
        let transition_matches = match (&self.delta, &self.recovery_rebind) {
            (Some(delta), None) => {
                delta.parent_manifest_digest == parent_digest.to_hex()
                    && delta.parent_source_binding_sha256 == parent.source.digest()?.to_hex()
                    && delta.parent_through_commit_seq == parent.source.through_commit_seq
                    && parent.source.through_commit_seq.checked_add(1)
                        == Some(self.source.through_commit_seq)
                    && self.recovery_rebinds == parent.recovery_rebinds
                    && self.source.database_oid == parent.source.database_oid
            }
            (None, Some(recovery)) => {
                recovery.validate()?;
                let mut expected = parent.source.clone();
                expected.database_oid = self.source.database_oid;
                expected.selected_audit_generation = self.source.selected_audit_generation;
                expected.selected_generation_digest =
                    self.source.selected_generation_digest.clone();
                recovery.parent_manifest_digest == parent_digest.to_hex()
                    && recovery.parent_source_binding_sha256 == parent.source.digest()?.to_hex()
                    && recovery.old_generation_digest == parent.source.selected_generation_digest
                    && recovery.old_database_oid == parent.source.database_oid
                    && recovery.new_database_oid == self.source.database_oid
                    && recovery.old_audit_generation == parent.source.selected_audit_generation
                    && recovery.new_audit_generation == self.source.selected_audit_generation
                    && recovery.restored_cut_digest == self.source.log_sha256
                    && self.source.selected_generation_digest
                        != parent.source.selected_generation_digest
                    && self.source == expected
                    && self.catalog_epoch == parent.catalog_epoch
                    && self.navigation_tree == parent.navigation_tree
                    && parent.recovery_rebinds.checked_add(1) == Some(self.recovery_rebinds)
            }
            _ => false,
        };
        if !transition_matches
            || parent.retained_generations.checked_add(1) != Some(self.retained_generations)
            || encode(
                &serde_json::to_value(&parent.base).map_err(|e| Error::Source(e.to_string()))?,
                1_048_576,
            )? != encode(
                &serde_json::to_value(&self.base).map_err(|e| Error::Source(e.to_string()))?,
                1_048_576,
            )?
            || encode(
                &serde_json::to_value(&parent.base_transport)
                    .map_err(|e| Error::Source(e.to_string()))?,
                1_048_576,
            )? != encode(
                &serde_json::to_value(&self.base_transport)
                    .map_err(|e| Error::Source(e.to_string()))?,
                1_048_576,
            )?
            || parent.base_provenance != self.base_provenance
            || parent.schema_worker_sha256 != self.schema_worker_sha256
            || parent.schema_set_sha256 != self.schema_set_sha256
            || self.retained_logical_bound_bytes < parent.retained_logical_bound_bytes
        {
            return Err(Error::Invalid(
                "overlay immutable parent/complete interval differs",
            ));
        }
        Ok(())
    }
    pub fn delta(&self) -> Option<&ManagedManifestDeltaV2> {
        self.delta.as_ref()
    }
    pub fn recovery_rebind(&self) -> Option<&ManagedManifestRecoveryRebindV2> {
        self.recovery_rebind.as_ref()
    }
    pub fn parent_manifest_digest(&self) -> Option<&str> {
        self.delta
            .as_ref()
            .map(|d| d.parent_manifest_digest.as_str())
            .or_else(|| {
                self.recovery_rebind
                    .as_ref()
                    .map(|r| r.parent_manifest_digest.as_str())
            })
    }
    pub fn base_selection(&self) -> &KnowledgeSelectedExpectation {
        &self.base
    }

    pub(crate) fn catalog_epoch(
        &self,
        store: &SegmentStore,
        audited: &AuditedStoreRoot,
    ) -> Result<crate::VersionsCatalogEpochV2> {
        let descriptor: serde_json::Value = serde_json::from_slice(&self.catalog_epoch)
            .map_err(|_| Error::Invalid("managed catalog epoch descriptor"))?;
        let tree_raw: Vec<u8> = serde_json::from_value(descriptor["tree"].clone())
            .map_err(|_| Error::Invalid("managed catalog tree descriptor"))?;
        let tree = AuthenticatedTreeDescriptorV2::decode(&tree_raw, 65_536)
            .map_err(|e| Error::Source(e.to_string()))?;
        crate::VersionsCatalogEpochV2::from_selected_manifest(
            store,
            audited,
            &self.catalog_epoch,
            tree.commitment,
            65_536,
        )
    }
    fn validate(&self, limits: ManagedManifestLimitsV2) -> Result<()> {
        limits.validate()?;
        self.source.validate()?;
        self.base_provenance.validate()?;
        if self.schema != SCHEMA
            || self.base.model_size_bytes == 0
            || self.catalog_epoch.is_empty()
            || self.catalog_epoch.len() > 65_536
            || self.navigation_tree.is_empty()
            || self.navigation_tree.len() > 65_536
            || self.retained_generations == 0
            || self.retained_generations > limits.max_retained_generations
            || self.retained_logical_bound_bytes < self.base.model_size_bytes
            || self.retained_logical_bound_bytes > limits.max_retained_logical_bound_bytes
        {
            return Err(Error::ManagedSourceUnsupported(
                "managed manifest bounds/ABI; cold compaction or FullOnly required",
            ));
        }
        let base_source_root = self.base_provenance.root_sha256()?;
        let base_generation = &self.base_provenance.generation;
        if self.base.managed_source_root_sha256.as_deref() != Some(base_source_root.as_str())
            || base_generation.domain != self.source.domain
            || base_generation.store_id != self.source.store_id
            || base_generation.domain_sha256 != self.source.domain_sha256
            || base_generation.epoch != self.source.epoch
            || base_generation.definition_sha256 != self.source.definition_sha256
            || base_generation.schema_profile_sha256 != self.source.schema_profile_sha256
            || base_generation.state_profile_sha256 != self.source.state_profile_sha256
            || base_generation.bootstrap_source_revision != self.source.bootstrap_source_revision
            || base_generation.bootstrap_membership_sha256
                != self.source.bootstrap_membership_sha256
            || base_generation.bootstrap_members != self.source.bootstrap_members
            || base_generation.through_commit_seq > self.source.through_commit_seq
        {
            return Err(Error::Invalid(
                "managed manifest immutable base/source context",
            ));
        }
        for digest in [&self.schema_worker_sha256, &self.schema_set_sha256] {
            Digest256::from_hex(digest)
                .map_err(|_| Error::Invalid("managed manifest software identity"))?;
        }
        self.base_transport
            .validate(&self.base, limits.max_retained_logical_bound_bytes)?;
        Digest256::from_hex(&self.base.model_sha256)
            .map_err(|_| Error::Invalid("managed immutable base identity"))?;
        if self.delta.is_some() && self.recovery_rebind.is_some()
            || self.recovery_rebinds >= self.retained_generations
        {
            return Err(Error::Invalid(
                "managed manifest exclusive transition/depth",
            ));
        }
        if let Some(recovery) = &self.recovery_rebind {
            recovery.validate()?;
            if recovery.new_database_oid != self.source.database_oid {
                return Err(Error::Invalid("managed recovery selected database differs"));
            }
        }
        if let Some(delta) = &self.delta {
            for digest in [
                &delta.parent_manifest_digest,
                &delta.parent_source_binding_sha256,
                &delta.committed_delta_sha256,
                &delta.committed_member_root_sha256,
            ] {
                Digest256::from_hex(digest)
                    .map_err(|_| Error::Invalid("managed manifest delta digest"))?;
            }
            if delta.parent_through_commit_seq.checked_add(1)
                != Some(self.source.through_commit_seq)
            {
                return Err(Error::Invalid("managed manifest source sequence gap"));
            }
        } else if self.recovery_rebind.is_none()
            && (self.retained_generations != 1
                || self.recovery_rebinds != 0
                || base_generation.through_commit_seq != self.source.through_commit_seq
                || base_generation.installed_generation_sha256
                    != self.source.selected_generation_digest
                || base_generation.selected_audit_generation
                    != self.source.selected_audit_generation
                || base_generation.database_oid != self.source.database_oid)
        {
            return Err(Error::Invalid(
                "managed initial manifest coverage/retention",
            ));
        }
        if self
            .source
            .through_commit_seq
            .checked_sub(base_generation.through_commit_seq)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_add(self.recovery_rebinds))
            != Some(self.retained_generations)
        {
            return Err(Error::Invalid(
                "managed manifest covered interval/retention differs",
            ));
        }
        let navigation = AuthenticatedTreeDescriptorV2::decode(&self.navigation_tree, 65_536)
            .map_err(|e| Error::Source(e.to_string()))?;
        if navigation.kind != b"tos.cmp.managed-navigation-delta.v2"
            || navigation.store_id != self.source.store_id
            || navigation.domain_digest.to_hex() != self.source.domain_sha256
        {
            return Err(Error::Invalid("managed navigation tree custody/kind"));
        }
        Ok(())
    }

    /// Called only after the genuine changed-record/navigation invocation has
    /// completed. Durable object first; PG publisher subsequently performs CAS.
    pub(crate) fn persist(
        &self,
        store: &SegmentStore,
        audited: &AuditedStoreRoot,
        limits: ManagedManifestLimitsV2,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Digest256, AuthenticatedTreeWorkV1)> {
        self.validate(limits)?;
        audited
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        if self.source.store_id != store.store_id()
            || self.source.domain_sha256 != store.domain_digest().to_hex()
            || self.source.domain.as_bytes() != store.custody_domain()
        {
            return Err(Error::Invalid("managed manifest actual store binding"));
        }
        let raw = encode(
            &serde_json::to_value(self).map_err(|e| Error::Source(e.to_string()))?,
            limits.max_manifest_bytes,
        )?;
        store
            .install_authenticated_object_v1_with_work(
                KIND,
                &raw,
                limits.max_manifest_bytes,
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))
    }

    /// Restore into the caller's exclusively owned empty private regular FD.
    /// Partial output is a staging candidate only. The caller then uses the
    /// original stage receipt with existing fs-verity preparation/cold opener;
    /// this method does not select a model or issue source/disclosure authority.
    pub fn restore_base_to_empty_file(
        &self,
        store: &SegmentStore,
        audited: &AuditedStoreRoot,
        output: &mut std::fs::File,
        limits: ManagedManifestLimitsV2,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(
        crate::knowledge_stage::StageReceipt,
        AuthenticatedTreeWorkV1,
    )> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        self.validate(limits)?;
        audited
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        let before = output.metadata()?;
        if !before.is_file()
            || before.len() != 0
            || before.nlink() != 1
            || before.uid() != unsafe { libc::geteuid() }
            || before.permissions().mode() & 0o077 != 0
        {
            return Err(Error::Invalid("managed restored base private empty FD"));
        }
        use std::io::Seek;
        output.seek(std::io::SeekFrom::Start(0))?;
        let work = self
            .base_transport
            .consume_chunks(store, Some(output), deadline, cancelled)?;
        output.sync_all()?;
        let after = output.metadata()?;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || after.len() != self.base.model_size_bytes
            || after.nlink() != 1
            || !after.is_file()
            || after.uid() != before.uid()
            || after.permissions().mode() & 0o077 != 0
        {
            return Err(Error::Invalid(
                "managed restored base inode/private custody changed",
            ));
        }
        audited
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Budget("managed restored base final deadline"));
        }
        Ok((self.base_transport.stage.clone(), work))
    }

    /// Explicit cold/recovery admission of the complete immutable interval.
    /// Warm readers use their already admitted operation-owned manifest/base;
    /// this global cold work must be included in their preparation receipt.
    pub fn read_retained_cold(
        store: &SegmentStore,
        audited: &AuditedStoreRoot,
        expected: &[Digest256],
        limits: ManagedManifestLimitsV2,
        tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(
        Vec<(std::sync::Arc<Self>, Vec<Digest256>)>,
        AuthenticatedTreeWorkV1,
    )> {
        audited
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        let mut work = AuthenticatedTreeWorkV1::default();
        let mut manifests = std::collections::BTreeMap::new();
        let mut verified = std::collections::HashSet::new();
        let mut admitted_bases = std::collections::HashSet::new();
        if expected.len() as u64 > tree_limits.max_rows {
            return Err(Error::Budget("managed retained manifest output rows"));
        }
        let mut result = Vec::new();
        let mut output_ref_bytes = 0u64;
        for &expected in expected {
            let selected = read_cold_manifest_once(
                store,
                audited,
                expected,
                limits,
                tree_limits,
                deadline,
                cancelled,
                &mut manifests,
                &mut work,
            )?;
            let base_descriptor = encode(
                &serde_json::to_value(&selected.base_transport)
                    .map_err(|e| Error::Source(e.to_string()))?,
                limits.max_manifest_bytes,
            )?;
            if admitted_bases.insert(Digest256::of_bytes(&base_descriptor).to_hex()) {
                let base_work = selected
                    .base_transport
                    .consume_chunks(store, None, deadline, cancelled)?;
                cold_charge_work(&mut work, base_work, tree_limits)?;
            }
            let mut visited_manifests = vec![expected];
            let mut current = selected.clone();
            let mut generations = 0u64;
            loop {
                generations = generations
                    .checked_add(1)
                    .ok_or(Error::Budget("managed cold manifest interval"))?;
                if generations > limits.max_retained_generations {
                    return Err(Error::Budget("managed cold manifest generations"));
                }
                let descriptor: serde_json::Value = serde_json::from_slice(&current.catalog_epoch)
                    .map_err(|_| Error::Invalid("managed cold catalog descriptor"))?;
                let catalog_raw: Vec<u8> = serde_json::from_value(descriptor["tree"].clone())
                    .map_err(|_| Error::Invalid("managed cold catalog tree shape"))?;
                for raw in [&catalog_raw, &current.navigation_tree] {
                    let tree = AuthenticatedTreeDescriptorV2::decode(raw, 65_536)
                        .map_err(|e| Error::Source(e.to_string()))?;
                    if tree.store_id != store.store_id()
                        || tree.domain_digest != store.domain_digest()
                        || (raw == &current.navigation_tree
                            && tree.kind != b"tos.cmp.managed-navigation-delta.v2")
                    {
                        return Err(Error::Invalid("managed cold tree custody/kind differs"));
                    }
                    // Same semantic commitment may have been rebound to a
                    // different physical packed cut. Cold recovery must verify
                    // each distinct descriptor, not deduplicate on semantics.
                    // V1/V2 decoding above rejects noncanonical descriptors, so
                    // hashing this exact wire also preserves the physical root.
                    let physical_identity = Digest256::of_bytes(raw).to_hex();
                    if verified.insert(physical_identity) {
                        let coverage = store
                            .verify_authenticated_tree_v2(&tree, tree_limits, deadline, cancelled)
                            .map_err(|e| Error::Source(e.to_string()))?;
                        work.read_nodes = work
                            .read_nodes
                            .checked_add(coverage.work.read_nodes)
                            .ok_or(Error::Budget("managed cold tree nodes"))?;
                        work.read_bytes = work
                            .read_bytes
                            .checked_add(coverage.work.read_bytes)
                            .ok_or(Error::Budget("managed cold tree bytes"))?;
                    }
                }
                if work.read_nodes > tree_limits.max_nodes
                    || work.read_bytes > tree_limits.max_total_bytes
                {
                    return Err(Error::Budget("managed cold whole closure work"));
                }
                let Some(parent_digest) = current.parent_manifest_digest() else {
                    break;
                };
                let parent_digest = Digest256::from_hex(parent_digest)
                    .map_err(|_| Error::Invalid("managed cold parent digest"))?;
                let parent = read_cold_manifest_once(
                    store,
                    audited,
                    parent_digest,
                    limits,
                    tree_limits,
                    deadline,
                    cancelled,
                    &mut manifests,
                    &mut work,
                )?;
                current.require_successor_of(&parent, parent_digest)?;
                visited_manifests.push(parent_digest);
                current = parent;
            }
            if generations != selected.retained_generations
                || work.read_nodes > tree_limits.max_nodes
                || work.read_bytes > tree_limits.max_total_bytes
            {
                return Err(Error::Invalid("managed cold complete interval differs"));
            }
            audited
                .require_store(store)
                .map_err(|e| Error::Source(e.to_string()))?;
            output_ref_bytes = output_ref_bytes
                .checked_add(
                    (visited_manifests.len() as u64)
                        .checked_mul(32)
                        .and_then(|n| n.checked_add(32))
                        .ok_or(Error::Budget("managed retained interval refs"))?,
                )
                .ok_or(Error::Budget("managed retained interval refs"))?;
            if output_ref_bytes > tree_limits.max_total_bytes {
                return Err(Error::Budget("managed retained interval output bound"));
            }
            result.push((selected, visited_manifests));
        }
        Ok((result, work))
    }

    /// The caller gets expected identity from the actual selected PG row under
    /// source fences. Arbitrary descriptive manifests cannot select themselves.
    pub fn read_selected(
        store: &SegmentStore,
        audited: &AuditedStoreRoot,
        expected: Digest256,
        limits: ManagedManifestLimitsV2,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Self, AuthenticatedTreeWorkV1)> {
        limits.validate()?;
        audited
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        let (raw, work) = store
            .read_authenticated_object_v1_with_work(
                KIND,
                expected,
                limits.max_manifest_bytes,
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
        let manifest: Self = serde_json::from_slice(&raw)
            .map_err(|_| Error::Invalid("managed selected manifest shape"))?;
        manifest.validate(limits)?;
        if manifest.source.store_id != store.store_id()
            || manifest.source.domain_sha256 != store.domain_digest().to_hex()
            || manifest.source.domain.as_bytes() != store.custody_domain()
            || encode(
                &serde_json::to_value(&manifest).map_err(|e| Error::Source(e.to_string()))?,
                limits.max_manifest_bytes,
            )? != raw
        {
            return Err(Error::Invalid(
                "managed selected manifest store/canonical binding",
            ));
        }
        manifest.catalog_epoch(store, audited)?;
        Ok((manifest, work))
    }
}

fn cold_charge_work(
    work: &mut AuthenticatedTreeWorkV1,
    added: AuthenticatedTreeWorkV1,
    limits: tos_segment_store::AuthenticatedTreeLimitsV1,
) -> Result<()> {
    work.read_nodes = work
        .read_nodes
        .checked_add(added.read_nodes)
        .ok_or(Error::Budget("managed cold read nodes"))?;
    work.read_bytes = work
        .read_bytes
        .checked_add(added.read_bytes)
        .ok_or(Error::Budget("managed cold read bytes"))?;
    if work.read_nodes > limits.max_nodes || work.read_bytes > limits.max_total_bytes {
        return Err(Error::Budget("managed cold whole admission bound"));
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn read_cold_manifest_once(
    store: &SegmentStore,
    audited: &AuditedStoreRoot,
    digest: Digest256,
    limits: ManagedManifestLimitsV2,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    admitted: &mut std::collections::BTreeMap<String, std::sync::Arc<ManagedManifestV2>>,
    work: &mut AuthenticatedTreeWorkV1,
) -> Result<std::sync::Arc<ManagedManifestV2>> {
    if let Some(manifest) = admitted.get(&digest.to_hex()) {
        return Ok(manifest.clone());
    }
    let (manifest, read) =
        ManagedManifestV2::read_selected(store, audited, digest, limits, deadline, cancelled)?;
    cold_charge_work(work, read, tree_limits)?;
    let manifest = std::sync::Arc::new(manifest);
    admitted.insert(digest.to_hex(), manifest.clone());
    Ok(manifest)
}
