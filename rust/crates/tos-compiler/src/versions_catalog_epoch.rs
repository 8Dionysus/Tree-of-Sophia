//! Opt-in addressed catalog provenance. V1 JSONL line/SHA remains a separate
//! global compatibility operation; a tree commitment is never that file SHA.
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde_json::{Value, json};
use tos_foundation::Digest256;
use tos_segment_store::{
    AuditedStoreRoot, AuthenticatedTreeDeltaV1, AuthenticatedTreeDescriptorV2,
    AuthenticatedTreeEntryV1, AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, SegmentError,
    SegmentErrorCode, SegmentStore,
};

use crate::knowledge_stage::KnowledgeStage;
use crate::source_bibliographic_render::{encode, text};
use crate::source_witness_catalog::{self as catalog, SourceCatalogLimits, SourceCatalogReceipt};
use crate::{Error, Result, SourceBinding};

const KIND: &[u8] = b"tos.cmp.versions-catalog.v2";

/// Private-constructed cold epoch from the genuine sealed catalog. Selection
/// must pin this exact epoch independently of a caller's requested record.
/// The held anchored-root guard prevents abort during this operation. Durable
/// historical retention/recovery still belongs to the selected model owner.
/// Missing or corrupted historical bytes refuse.
#[derive(Clone, Debug)]
pub struct VersionsCatalogEpochV2 {
    store: SegmentStore,
    retention: AuditedStoreRoot,
    tree: AuthenticatedTreeDescriptorV2,
    binding: SourceBinding,
    catalog_seal: Option<String>,
    retained_node_logical_bound: u64,
    record_entries: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EpochDescriptorV2 {
    schema: String,
    tree: Vec<u8>,
    binding: SourceBinding,
    cold_catalog_seal: Option<String>,
    retained_node_logical_bound: u64,
    record_entries: u64,
}

impl VersionsCatalogEpochV2 {
    /// Explicit cold admission: all catalog rows are consumed once. No warm
    /// fixed-U claim is made for this migration or for V1 export.
    pub fn build(
        stage: &mut KnowledgeStage<'_>,
        receipt: &SourceCatalogReceipt,
        store: &SegmentStore,
        catalog_limits: SourceCatalogLimits,
        tree_limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        Self::build_with_work(
            stage,
            receipt,
            store,
            catalog_limits,
            tree_limits,
            deadline,
            cancelled,
        )
        .map(|(epoch, _)| epoch)
    }

    pub fn build_with_work(
        stage: &mut KnowledgeStage<'_>,
        receipt: &SourceCatalogReceipt,
        store: &SegmentStore,
        catalog_limits: SourceCatalogLimits,
        tree_limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Self, AuthenticatedTreeWorkV1)> {
        let retention = store
            .hold_audit_root()
            .map_err(|e| Error::Source(e.to_string()))?;
        catalog::verify_catalog(stage, receipt, catalog_limits)?;
        let mut category = 0;
        let mut record_entries = 0u64;
        let mut after: Option<String> = None;
        let mut failure = None;
        let mut stopped = false;
        // Byte ordering of category prefixes is claims, records.
        let rows = std::iter::from_fn(|| {
            if stopped {
                return None;
            }
            let row = (|| -> Result<Option<AuthenticatedTreeEntryV1>> {
                while category < 2 {
                    if cancelled.load(std::sync::atomic::Ordering::Relaxed)
                        || Instant::now() >= deadline
                    {
                        return Err(Error::Budget("Versions catalog cold admission deadline"));
                    }
                    let name = ["claims", "records"][category];
                    match catalog::catalog_next(stage, name, after.as_deref())? {
                        Some(id) => {
                            let row = catalog::catalog_row(stage, name, &id, catalog_limits)?
                                .ok_or(Error::Invalid("Versions catalog selected row vanished"))?;
                            let entry = &row["entry"];
                            verify_identity(name, &id, entry)?;
                            let value = encode(entry, catalog_limits.max_output_row_bytes)?;
                            let key = key(name, &id)?;
                            if name == "records" {
                                record_entries = record_entries
                                    .checked_add(1)
                                    .ok_or(Error::Budget("catalog record counter"))?;
                            }
                            after = Some(id);
                            return Ok(Some(AuthenticatedTreeEntryV1 { key, value }));
                        }
                        None => {
                            category += 1;
                            after = None;
                        }
                    }
                }
                Ok(None)
            })();
            match row {
                Ok(Some(row)) => Some(Ok(row)),
                Ok(None) => {
                    stopped = true;
                    None
                }
                Err(error) => {
                    failure = Some(error);
                    stopped = true;
                    Some(Err(SegmentError::new(
                        SegmentErrorCode::InvalidReceipt,
                        "sealed Versions catalog admission failed",
                    )))
                }
            }
        });
        let result = store.build_authenticated_tree_v2_with_work(
            KIND,
            rows,
            tree_limits,
            deadline,
            cancelled,
        );
        if let Some(error) = failure {
            return Err(error);
        }
        let (tree, work) = result.map_err(|e| Error::Source(e.to_string()))?;
        Ok((
            Self {
                store: store.clone(),
                retention,
                tree,
                binding: receipt.input_binding.clone(),
                catalog_seal: Some(receipt.row_root_sha256.clone()),
                record_entries,
                retained_node_logical_bound: work
                    .read_bytes
                    .checked_add(work.written_bytes)
                    .ok_or(Error::Budget("catalog cold retained-byte counter"))?,
            },
            work,
        ))
    }

    /// Bounded manifest member. Only a genuine durable selected manifest may
    /// authorize recovery; these serialized facts alone issue no source grant.
    pub(crate) fn descriptor(&self, max_bytes: usize) -> Result<Vec<u8>> {
        if max_bytes == 0 || max_bytes > 65_536 {
            return Err(Error::Budget("Versions epoch manifest member bytes"));
        }
        self.retention
            .require_store(&self.store)
            .map_err(|e| Error::Source(e.to_string()))?;
        let descriptor = EpochDescriptorV2 {
            schema: "tos_versions_catalog_epoch_v2".into(),
            tree: self
                .tree
                .encode(max_bytes)
                .map_err(|e| Error::Source(e.to_string()))?,
            binding: self.binding.clone(),
            cold_catalog_seal: self.catalog_seal.clone(),
            retained_node_logical_bound: self.retained_node_logical_bound,
            record_entries: self.record_entries,
        };
        encode(
            &serde_json::to_value(descriptor).map_err(|e| Error::Source(e.to_string()))?,
            max_bytes,
        )
    }

    pub(crate) fn from_selected_manifest(
        store: &SegmentStore,
        audited: &AuditedStoreRoot,
        raw: &[u8],
        expected: Digest256,
        max_bytes: usize,
    ) -> Result<Self> {
        audited
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        if max_bytes == 0 || max_bytes > 65_536 || raw.len() > max_bytes {
            return Err(Error::Budget("Versions epoch manifest member bytes"));
        }
        let descriptor: EpochDescriptorV2 = serde_json::from_slice(raw)
            .map_err(|_| Error::Invalid("Versions epoch manifest member shape"))?;
        let tree = AuthenticatedTreeDescriptorV2::decode(&descriptor.tree, max_bytes)
            .map_err(|e| Error::Source(e.to_string()))?;
        if descriptor.schema != "tos_versions_catalog_epoch_v2"
            || !descriptor.binding.complete
            || tree.kind != KIND
            || tree.store_id != store.store_id()
            || tree.domain_digest != store.domain_digest()
            || (tree.entries > 0 && descriptor.retained_node_logical_bound == 0)
            || descriptor.record_entries > tree.entries
            || tree.commitment != expected
        {
            return Err(Error::Invalid(
                "Versions epoch independent selected manifest binding",
            ));
        }
        let epoch = Self {
            store: store.clone(),
            retention: audited.clone(),
            tree,
            binding: descriptor.binding,
            catalog_seal: descriptor.cold_catalog_seal,
            retained_node_logical_bound: descriptor.retained_node_logical_bound,
            record_entries: descriptor.record_entries,
        };
        if epoch.descriptor(max_bytes)? != raw {
            return Err(Error::Invalid(
                "Versions epoch manifest member canonical bytes",
            ));
        }
        Ok(epoch)
    }

    pub(crate) fn require_cold_producer_catalog(
        &self,
        root: &str,
        store: &SegmentStore,
    ) -> Result<()> {
        self.retention
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        if self.catalog_seal.as_deref() != Some(root)
            || self.tree.store_id != store.store_id()
            || self.tree.domain_digest != store.domain_digest()
        {
            return Err(Error::Invalid(
                "cold catalog genuine producer/store differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn retained_node_logical_bound(&self) -> u64 {
        self.retained_node_logical_bound
    }

    pub fn record_entries(&self) -> u64 {
        self.record_entries
    }
    pub fn total_entries(&self) -> u64 {
        self.tree.entries
    }

    pub(crate) fn next_overlay_binding(
        &self,
        sequence: u64,
        selected_generation: &str,
    ) -> Result<SourceBinding> {
        if self.binding.through_commit_seq.checked_add(1) != Some(sequence) {
            return Err(Error::Invalid("selected catalog epoch sequence gap"));
        }
        Digest256::from_hex(selected_generation)
            .map_err(|_| Error::Invalid("selected overlay generation digest"))?;
        let mut binding = self.binding.clone();
        binding.through_commit_seq = sequence;
        binding.source_cut = format!("managed-model-v2:{selected_generation}");
        Ok(binding)
    }
    pub fn commitment(&self) -> Digest256 {
        self.tree.commitment
    }

    pub(crate) fn require_catalog(&self, receipt: &SourceCatalogReceipt) -> Result<()> {
        if self.catalog_seal.as_deref() != Some(receipt.row_root_sha256.as_str())
            || serde_json::to_value(&self.binding).map_err(|e| Error::Source(e.to_string()))?
                != serde_json::to_value(&receipt.input_binding)
                    .map_err(|e| Error::Source(e.to_string()))?
        {
            return Err(Error::Invalid(
                "Versions selected catalog epoch/source binding differs",
            ));
        }
        Ok(())
    }

    /// Initial-Agent-only COW change. This method is crate-private: the
    /// managed producer must check the genuine committed projection and
    /// source proof before invoking it. It issues no current-use grant.
    pub(crate) fn insert_created_agent(
        &self,
        binding: SourceBinding,
        entry: &Value,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        self.insert_created_agent_with_work(binding, entry, limits, deadline, cancelled)
            .map(|(epoch, _)| epoch)
    }

    pub(crate) fn insert_created_agent_with_work(
        &self,
        binding: SourceBinding,
        entry: &Value,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Self, AuthenticatedTreeWorkV1)> {
        self.retention
            .require_store(&self.store)
            .map_err(|e| Error::Source(e.to_string()))?;
        let id = text(entry, "record_id")?;
        verify_identity("records", id, entry)?;
        if entry["record_type"] != "agent"
            || !binding.complete
            || !self.binding.complete
            || self.binding.through_commit_seq.checked_add(1) != Some(binding.through_commit_seq)
            || binding.owner_profile != self.binding.owner_profile
            || binding.reader_abi != self.binding.reader_abi
            || binding.route_map_version != self.binding.route_map_version
            || binding.source_cut == self.binding.source_cut
        {
            return Err(Error::Invalid("created Agent catalog epoch generation"));
        }
        let (present, lookup_work) =
            self.lookup_with_work("records", id, limits, deadline, cancelled)?;
        if present.is_some() {
            return Err(Error::Invalid(
                "created Agent overwrites retained catalog entry",
            ));
        }
        let delta = AuthenticatedTreeDeltaV1 {
            key: key("records", id)?,
            value: Some(encode(entry, limits.max_value_bytes)?),
        };
        let (tree, work) = self
            .store
            .apply_authenticated_tree_delta_v2_with_work(
                &self.tree,
                std::iter::once(Ok(delta)),
                limits,
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
        if tree.entries
            != self
                .tree
                .entries
                .checked_add(1)
                .ok_or(Error::Budget("created Agent catalog entry count"))?
        {
            return Err(Error::Invalid("created Agent catalog count"));
        }
        let combined_work = add_work(lookup_work, work)?;
        Ok((
            Self {
                store: self.store.clone(),
                retention: self.retention.clone(),
                tree,
                binding,            // current model selection independently binds this epoch.
                catalog_seal: None, // no invented complete JSONL transcript.
                record_entries: self
                    .record_entries
                    .checked_add(1)
                    .ok_or(Error::Budget("catalog record count"))?,
                retained_node_logical_bound: self
                    .retained_node_logical_bound
                    .checked_add(work.read_bytes)
                    .and_then(|n| n.checked_add(work.written_bytes))
                    .ok_or(Error::Budget("catalog retained-byte counter"))?,
            },
            combined_work,
        ))
    }

    /// Exact addressed membership/absence in this selected immutable epoch.
    /// Storage validates the anchored store, domain, tree shape and every
    /// node on the path. Absence cannot be inferred from a different epoch.
    pub fn lookup(
        &self,
        category: &str,
        id: &str,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Value>> {
        self.lookup_with_work(category, id, limits, deadline, cancelled)
            .map(|(entry, _)| entry)
    }

    pub fn lookup_with_work(
        &self,
        category: &str,
        id: &str,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<Value>, AuthenticatedTreeWorkV1)> {
        self.retention
            .require_store(&self.store)
            .map_err(|e| Error::Source(e.to_string()))?;
        let (raw, work) = self
            .store
            .lookup_authenticated_tree_v2_with_work(
                &self.tree,
                &key(category, id)?,
                limits,
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
        let entry = raw
            .map(|raw| {
                let entry: Value = serde_json::from_slice(&raw)
                    .map_err(|_| Error::Invalid("Versions catalog entry JSON"))?;
                verify_identity(category, id, &entry)?;
                if encode(&entry, limits.max_value_bytes)? != raw {
                    return Err(Error::Invalid("Versions catalog canonical entry differs"));
                }
                Ok(entry)
            })
            .transpose()?;
        Ok((entry, work))
    }

    pub(crate) fn provenance(
        &self,
        category: &str,
        id: &str,
        expected_entry: &Value,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        self.provenance_with_work(category, id, expected_entry, limits, deadline, cancelled)
            .map(|(value, _)| value)
    }

    pub(crate) fn provenance_with_work(
        &self,
        category: &str,
        id: &str,
        expected_entry: &Value,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Value, AuthenticatedTreeWorkV1)> {
        let (entry, work) = self.lookup_with_work(category, id, limits, deadline, cancelled)?;
        let entry = entry.ok_or(Error::Invalid("Versions selected catalog entry absent"))?;
        let raw = encode(&entry, limits.max_value_bytes)?;
        if raw != encode(expected_entry, limits.max_value_bytes)? {
            return Err(Error::Invalid(
                "Versions selected catalog entry/source differs",
            ));
        }
        Ok((
            json!({
                "schema_version":"tos_versions_catalog_reference_v2",
                "epoch": {"root_sha256":self.tree.commitment.to_hex(),
                    "store_id":self.tree.store_id,"source_cut":self.binding.source_cut,
                    "through_commit_seq":self.binding.through_commit_seq},
                "category":category,"record_id":id,
                "canonical_entry_sha256":Digest256::of_bytes(&raw).to_hex()
            }),
            work,
        ))
    }
}

fn add_work(
    a: AuthenticatedTreeWorkV1,
    b: AuthenticatedTreeWorkV1,
) -> Result<AuthenticatedTreeWorkV1> {
    let add = |a: u64, b: u64| {
        a.checked_add(b)
            .ok_or(Error::Budget("catalog work counter"))
    };
    Ok(AuthenticatedTreeWorkV1 {
        read_nodes: add(a.read_nodes, b.read_nodes)?,
        read_bytes: add(a.read_bytes, b.read_bytes)?,
        written_nodes: add(a.written_nodes, b.written_nodes)?,
        written_bytes: add(a.written_bytes, b.written_bytes)?,
    })
}

fn key(category: &str, id: &str) -> Result<Vec<u8>> {
    if !matches!(category, "claims" | "records") || id.is_empty() || id.contains('\0') {
        return Err(Error::Invalid("Versions addressed catalog key"));
    }
    let mut key = category.as_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    Ok(key)
}

fn verify_identity(category: &str, id: &str, entry: &Value) -> Result<()> {
    let field = if category == "claims" {
        "claim_id"
    } else {
        "record_id"
    };
    if text(entry, field)? != id {
        return Err(Error::Invalid("Versions catalog record identity differs"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tos_segment_store::{AuthenticatedTreeDeltaV1, SegmentLimits};

    fn limits() -> AuthenticatedTreeLimitsV1 {
        AuthenticatedTreeLimitsV1 {
            max_key_bytes: 512,
            max_value_bytes: 4096,
            max_kind_bytes: 128,
            max_node_bytes: 8192,
            max_children: 16,
            max_nodes: 128,
            max_total_bytes: 1024 * 1024,
            max_rows: 16,
        }
    }
    fn binding() -> SourceBinding {
        SourceBinding {
            owner_profile: "test-cold-only".into(),
            source_cut: "initial-cut".into(),
            through_commit_seq: 1,
            membership_root: "initial-membership".into(),
            index_generation: "initial-index".into(),
            route_map_version: "test-route".into(),
            reader_abi: "test-cold-only".into(),
            projection_root_sha256: "test-projection".into(),
            complete: true,
        }
    }
    /// This exercises mechanics/selected lookup only; it does not fabricate a
    /// genuine source admission, Versions history closure, or model selection.
    #[test]
    fn addressed_catalog_changed_unchanged_historical_and_gap_are_exact() {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let store = SegmentStore::initialize_empty(
            root.path(),
            b"versions-catalog-test",
            SegmentLimits {
                max_segment_bytes: 1024 * 1024,
                max_frame_bytes: 8192,
                max_frames: 128,
                max_journal_bytes: 1024 * 1024,
            },
        )
        .unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        let cancelled = AtomicBool::new(false);
        let a = json!({"record_id":"tos.agent.a","record_type":"agent",
            "source_record_ref":"ToS/a.json","record_sha256":"old-a"});
        let b = json!({"record_id":"tos.agent.b","record_type":"agent",
            "source_record_ref":"ToS/b.json","record_sha256":"old-b"});
        let entries = [a.clone(), b.clone()].map(|entry| {
            Ok(AuthenticatedTreeEntryV1 {
                key: key("records", entry["record_id"].as_str().unwrap()).unwrap(),
                value: encode(&entry, 4096).unwrap(),
            })
        });
        let tree = store
            .build_authenticated_tree_v2(KIND, entries, limits(), deadline, &cancelled)
            .unwrap();
        let old = VersionsCatalogEpochV2 {
            store: store.clone(),
            retention: store.hold_audit_root().unwrap(),
            tree,
            binding: binding(),
            catalog_seal: Some("test-seal-not-authority".into()),
            retained_node_logical_bound: 1_048_576,
            record_entries: 2,
        };
        let created = json!({"record_id":"tos.agent.c","record_type":"agent",
            "source_record_ref":"ToS/source-witnesses/agents/c/agent.json","record_sha256":"new-c"});
        let mut next_binding = binding();
        next_binding.through_commit_seq += 1;
        next_binding.source_cut = "next-genuine-source-selection-placeholder".into();
        let inserted = old
            .insert_created_agent(
                next_binding.clone(),
                &created,
                limits(),
                deadline,
                &cancelled,
            )
            .unwrap();
        assert_eq!(
            inserted
                .lookup("records", "tos.agent.c", limits(), deadline, &cancelled)
                .unwrap(),
            Some(created)
        );
        assert_eq!(
            inserted
                .lookup("records", "tos.agent.a", limits(), deadline, &cancelled)
                .unwrap(),
            Some(a.clone())
        );
        assert!(
            old.lookup("records", "tos.agent.c", limits(), deadline, &cancelled)
                .unwrap()
                .is_none()
        );
        assert!(
            old.insert_created_agent(next_binding, &a, limits(), deadline, &cancelled)
                .is_err()
        );
        assert!(inserted.catalog_seal.is_none());
        // Generic STO mechanics below exercise change/tombstone/history. They
        // are not an admission route for the controlled initial-Agent writer.
        let encoded = inserted.descriptor(65_536).unwrap();
        let recovered = VersionsCatalogEpochV2::from_selected_manifest(
            &store,
            &old.retention,
            &encoded,
            inserted.commitment(),
            65_536,
        )
        .unwrap();
        assert_eq!(
            recovered
                .lookup("records", "tos.agent.c", limits(), deadline, &cancelled)
                .unwrap(),
            inserted
                .lookup("records", "tos.agent.c", limits(), deadline, &cancelled)
                .unwrap()
        );
        assert!(
            VersionsCatalogEpochV2::from_selected_manifest(
                &store,
                &old.retention,
                &encoded,
                old.commitment(),
                65_536,
            )
            .is_err()
        );
        let mut changed = a.clone();
        changed["record_sha256"] = json!("new-a");
        let tree = store
            .apply_authenticated_tree_delta_v2(
                &old.tree,
                [Ok(AuthenticatedTreeDeltaV1 {
                    key: key("records", "tos.agent.a").unwrap(),
                    value: Some(encode(&changed, 4096).unwrap()),
                })],
                limits(),
                deadline,
                &cancelled,
            )
            .unwrap();
        let mut current = old.clone();
        current.tree = tree;
        current.binding.through_commit_seq = 2;
        assert_ne!(old.commitment(), current.commitment());
        assert_eq!(
            current
                .lookup("records", "tos.agent.a", limits(), deadline, &cancelled)
                .unwrap(),
            Some(changed.clone())
        );
        assert_eq!(
            current
                .lookup("records", "tos.agent.b", limits(), deadline, &cancelled)
                .unwrap(),
            Some(b)
        );
        assert_eq!(
            old.lookup("records", "tos.agent.a", limits(), deadline, &cancelled)
                .unwrap(),
            Some(a.clone())
        );
        assert!(
            current
                .lookup(
                    "records",
                    "tos.agent.absent",
                    limits(),
                    deadline,
                    &cancelled
                )
                .unwrap()
                .is_none()
        );
        assert!(
            current
                .provenance("records", "tos.agent.a", &a, limits(), deadline, &cancelled)
                .is_err()
        );
        let provenance = current
            .provenance(
                "records",
                "tos.agent.a",
                &changed,
                limits(),
                deadline,
                &cancelled,
            )
            .unwrap();
        assert_eq!(
            provenance["epoch"]["root_sha256"],
            json!(current.commitment().to_hex())
        );
        assert!(provenance.get("line").is_none());
        assert!(provenance.get("sha256").is_none());
        let removed = store
            .apply_authenticated_tree_delta_v2(
                &current.tree,
                [Ok(AuthenticatedTreeDeltaV1 {
                    key: key("records", "tos.agent.a").unwrap(),
                    value: None,
                })],
                limits(),
                deadline,
                &cancelled,
            )
            .unwrap();
        let mut removed_epoch = current.clone();
        removed_epoch.tree = removed;
        assert!(
            removed_epoch
                .lookup("records", "tos.agent.a", limits(), deadline, &cancelled)
                .unwrap()
                .is_none()
        );
        assert!(
            current
                .lookup("records", "tos.agent.a", limits(), deadline, &cancelled)
                .unwrap()
                .is_some()
        );
    }
}
