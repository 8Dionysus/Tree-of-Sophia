//! Point reads from one selected immutable base + authenticated addressed tree.
//! Current disclosure remains under CMD's owner/source/policy transaction.
use crate::managed_source::ManagedProducerProof;
use crate::source_bibliographic_render::encode;
use crate::{Error, ManagedManifestV2, Result, VerifiedKnowledgeModel, VersionsCatalogEpochV2};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_segment_store::{
    AuditedStoreRoot, AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, SegmentError, SegmentStore,
};

const NAVIGATION_KIND: &[u8] = b"tos.cmp.managed-navigation-delta.v2";

#[derive(Clone, Copy, Debug, Default)]
pub struct ManagedOverlayLookupWorkV2 {
    pub tree: AuthenticatedTreeWorkV1,
    /// Logical bytes actually read/encoded by the existing base point seeker;
    /// excludes SQLite internal page I/O and does not repeat cold admission.
    pub base_charged_bytes: u64,
    pub base_candidate_rows: u64,
}

/// A present entry with no payload is an authenticated tombstone. Missing
/// entry means base fallback, never deletion. Canonical encoding preserves
/// this distinction through cold reopen/recovery.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NavigationOverlayValueV2 {
    pub schema: String,
    pub collection: String,
    pub id: String,
    pub payload: Option<Value>,
}

pub(crate) fn navigation_key(collection: &str, id: &str) -> Result<Vec<u8>> {
    if !matches!(collection, "nodes" | "edges")
        || id.is_empty()
        || id.len() > 4096
        || id.contains('\0')
    {
        return Err(Error::Invalid("managed overlay navigation key"));
    }
    let mut key = collection.as_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    Ok(key)
}

/// Only genuine full producer + cold-admitted immutable base construction
/// below can issue this outcome. Descriptive manifest JSON is not a producer.
pub struct CompletedManagedOverlayBaseV2 {
    manifest: ManagedManifestV2,
    digest: tos_foundation::Digest256,
    manifest_work: AuthenticatedTreeWorkV1,
    base_digest_read_bytes: u64,
    base_transport_work: AuthenticatedTreeWorkV1,
    base_transport_read_bytes: u64,
    base_validation_charged_bytes: u64,
    catalog_records: u64,
    catalog_total_entries: u64,
}
impl CompletedManagedOverlayBaseV2 {
    pub fn manifest(&self) -> &ManagedManifestV2 {
        &self.manifest
    }
    pub fn manifest_digest(&self) -> tos_foundation::Digest256 {
        self.digest
    }
    pub fn manifest_work(&self) -> AuthenticatedTreeWorkV1 {
        self.manifest_work
    }
    pub fn base_transport_work(&self) -> AuthenticatedTreeWorkV1 {
        self.base_transport_work
    }
    pub fn base_transport_read_bytes(&self) -> u64 {
        self.base_transport_read_bytes
    }
    pub fn base_digest_read_bytes(&self) -> u64 {
        self.base_digest_read_bytes
    }
    pub fn base_validation_charged_bytes(&self) -> u64 {
        self.base_validation_charged_bytes
    }
    pub fn catalog_records(&self) -> u64 {
        self.catalog_records
    }
    pub fn catalog_total_entries(&self) -> u64 {
        self.catalog_total_entries
    }
}

/// Explicit cold preparation. It preserves the real old full source proof and
/// admitted SQLite base; the tree is a genuine catalog epoch, never inventory
/// root relabelling. The returned manifest is durable but still unselected
/// until the independent CMD same-transaction source-fenced CAS completes.
#[allow(clippy::too_many_arguments)]
pub fn prepare_managed_overlay_base_v2<P: crate::ManagedProducerProof>(
    producer: &crate::managed_agent_producer::CompletedManagedAgentProducer<P>,
    base: &VerifiedKnowledgeModel<'_>,
    catalog: &VersionsCatalogEpochV2,
    source: crate::ManagedOverlaySourceBindingV2,
    store: &SegmentStore,
    audited: &AuditedStoreRoot,
    tree_limits: AuthenticatedTreeLimitsV1,
    manifest_limits: crate::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CompletedManagedOverlayBaseV2> {
    source.validate()?;
    // The sealed producer supplies its actual basis variant. V1 is refused,
    // never reserialized or relabeled as addressed provenance.
    let basis = producer.source_proof().basis();
    let proof = basis.managed_source_v2().ok_or(Error::Invalid(
        "overlay cold requires genuine addressed V2 producer",
    ))?;
    proof.validate()?;
    let g = &proof.generation;
    if proof.delta.is_some()
        || base.source_basis() != &proof.basis()
        || encode(
            &serde_json::to_value(base.selection()).map_err(|e| Error::Source(e.to_string()))?,
            1_048_576,
        )? != encode(
            &serde_json::to_value(producer.expectation())
                .map_err(|e| Error::Source(e.to_string()))?,
            1_048_576,
        )?
        || g.domain != source.domain
        || g.store_id != source.store_id
        || g.database_oid != source.database_oid
        || g.state_profile_sha256 != source.state_profile_sha256
        || g.log_sha256 != source.log_sha256
        || g.bootstrap_source_revision != source.bootstrap_source_revision
        || g.bootstrap_membership_sha256 != source.bootstrap_membership_sha256
        || g.bootstrap_members != source.bootstrap_members
        || g.domain_sha256 != source.domain_sha256
        || g.installed_generation_sha256 != source.selected_generation_digest
        || g.selected_audit_generation != source.selected_audit_generation
        || g.through_commit_seq != source.through_commit_seq
        || g.epoch != source.epoch
        || g.definition_sha256 != source.definition_sha256
        || g.schema_profile_sha256 != source.schema_profile_sha256
        || g.addressed_current_tree_sha256 != source.addressed_current_tree_sha256
        || g.addressed_history_tree_sha256 != source.addressed_history_tree_sha256
        || g.addressed_metadata_tree_sha256 != source.addressed_metadata_tree_sha256
        || g.addressed_inventory_root_sha256 != source.addressed_inventory_root_sha256
        || g.current_members != source.current_source_members
        || g.history_members != source.historical_source_members
        || g.addressed_metadata_members != source.metadata_members
    {
        return Err(Error::Invalid(
            "overlay cold genuine producer/base/source differs",
        ));
    }
    base.check_pin()?;
    catalog.require_cold_producer_catalog(producer.source_catalog_root_sha256(), store)?;
    let (navigation, empty_work) = store
        .build_authenticated_tree_v2_with_work(
            NAVIGATION_KIND,
            std::iter::empty::<std::result::Result<AuthenticatedTreeEntryV1, SegmentError>>(),
            tree_limits,
            deadline,
            cancelled,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
    if empty_work.read_nodes != 0 || empty_work.written_nodes != 0 {
        return Err(Error::Invalid(
            "empty navigation delta unexpectedly performed node I/O",
        ));
    }
    let (base_transport, base_transport_work) =
        crate::managed_model_manifest::ManagedBaseTransportV2::import_pinned(
            base,
            producer.stage(),
            store,
            manifest_limits.max_retained_logical_bound_bytes,
            deadline,
            cancelled,
        )?;
    let retained = base
        .selection()
        .model_size_bytes
        .checked_add(base_transport.retained_encoded_bound()?)
        .and_then(|n| n.checked_add(catalog.retained_node_logical_bound()))
        .and_then(|n| n.checked_add(manifest_limits.max_manifest_bytes as u64))
        .ok_or(Error::Budget("overlay cold retained logical byte bound"))?;
    let manifest = ManagedManifestV2 {
        schema: "tos_managed_model_manifest_v2".into(),
        base: base.selection().clone(),
        base_provenance: proof.clone(),
        base_transport,
        schema_worker_sha256: producer.schema_worker_sha256().to_owned(),
        schema_set_sha256: producer.schema_set_sha256().to_owned(),
        source,
        catalog_epoch: catalog.descriptor(65_536)?,
        navigation_tree: navigation
            .encode(65_536)
            .map_err(|e| Error::Source(e.to_string()))?,
        delta: None,
        recovery_rebind: None,
        recovery_rebinds: 0,
        retained_generations: 1,
        retained_logical_bound_bytes: retained,
    };
    let (digest, manifest_work) =
        manifest.persist(store, audited, manifest_limits, deadline, cancelled)?;
    base.check_pin()?;
    audited
        .require_store(store)
        .map_err(|e| Error::Source(e.to_string()))?;
    Ok(CompletedManagedOverlayBaseV2 {
        manifest,
        digest,
        manifest_work,
        base_digest_read_bytes: base.cold_digest_read_bytes(),
        base_transport_work,
        base_transport_read_bytes: base.selection().model_size_bytes,
        base_validation_charged_bytes: base.cold_validation_charged_bytes(),
        catalog_records: catalog.record_entries(),
        catalog_total_entries: catalog.total_entries(),
    })
}

/// The caller supplies the genuinely admitted/pinned base once. Warm point
/// reads neither reopen nor rehash the full SQLite base. Each reader is scoped
/// to one exact immutable manifest; a historical manifest is never silently
/// substituted for the current PG selection.
pub struct ManagedOverlayReaderV2<'lease, 'base> {
    base: &'lease VerifiedKnowledgeModel<'base>,
    manifest: &'lease ManagedManifestV2,
    catalog: VersionsCatalogEpochV2,
    navigation: AuthenticatedTreeDescriptorV2,
    store: SegmentStore,
    retention: AuditedStoreRoot,
}
impl<'lease, 'base> ManagedOverlayReaderV2<'lease, 'base> {
    pub fn new(
        base: &'lease VerifiedKnowledgeModel<'base>,
        manifest: &'lease ManagedManifestV2,
        store: &SegmentStore,
        audited: &AuditedStoreRoot,
    ) -> Result<Self> {
        audited
            .require_store(store)
            .map_err(|e| Error::Source(e.to_string()))?;
        base.check_pin()?;
        if encode(
            &serde_json::to_value(base.selection()).map_err(|e| Error::Source(e.to_string()))?,
            1_048_576,
        )? != encode(
            &serde_json::to_value(&manifest.base).map_err(|e| Error::Source(e.to_string()))?,
            1_048_576,
        )? || base.source_basis() != &manifest.base_provenance.basis()
        {
            return Err(Error::Invalid(
                "overlay admitted immutable base/provenance differs",
            ));
        }
        let catalog = manifest.catalog_epoch(store, audited)?;
        let navigation = AuthenticatedTreeDescriptorV2::decode(&manifest.navigation_tree, 65_536)
            .map_err(|e| Error::Source(e.to_string()))?;
        if navigation.kind != NAVIGATION_KIND
            || navigation.store_id != store.store_id()
            || navigation.domain_digest != store.domain_digest()
        {
            return Err(Error::Invalid("overlay navigation actual custody differs"));
        }
        Ok(Self {
            base,
            manifest,
            catalog,
            navigation,
            store: store.clone(),
            retention: audited.clone(),
        })
    }

    /// Exact Versions V2 entry; old full JSONL ordinal/SHA are not invented.
    pub fn catalog_member(
        &self,
        category: &str,
        id: &str,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<Value>, AuthenticatedTreeWorkV1)> {
        self.base.check_pin()?;
        let result = self
            .catalog
            .lookup_with_work(category, id, limits, deadline, cancelled)?;
        self.base.check_pin()?;
        self.retention
            .require_store(&self.store)
            .map_err(|e| Error::Source(e.to_string()))?;
        Ok(result)
    }

    /// Return exact original selected navigation bytes. Unchanged base members
    /// retain their genuine historical base provenance; addressed members carry
    /// the V2 epoch issued by their actual producer. This is not V1 re-export or
    /// a grant of current source use, and unsupported full search/lens/export
    /// still require FullOnly implementations.
    #[allow(clippy::too_many_arguments)]
    pub fn navigation_member(
        &self,
        collection: &str,
        id: &str,
        validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
        l: crate::source_bibliographic::BibliographicLimits,
        base_limits: crate::knowledge_stage::StageLimits,
        tree_limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<Value>, ManagedOverlayLookupWorkV2)> {
        self.base.check_pin()?;
        self.retention
            .require_store(&self.store)
            .map_err(|e| Error::Source(e.to_string()))?;
        let (raw, tree) = self
            .store
            .lookup_authenticated_tree_v2_with_work(
                &self.navigation,
                &navigation_key(collection, id)?,
                tree_limits,
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
        let mut work = ManagedOverlayLookupWorkV2 {
            tree,
            ..Default::default()
        };
        let result = if let Some(raw) = raw {
            let value: NavigationOverlayValueV2 = serde_json::from_slice(&raw)
                .map_err(|_| Error::Invalid("overlay navigation value shape"))?;
            if value.schema != "tos_managed_navigation_member_v2"
                || value.collection != collection
                || value.id != id
                || encode(
                    &serde_json::to_value(&value).map_err(|e| Error::Source(e.to_string()))?,
                    tree_limits.max_value_bytes,
                )? != raw
            {
                return Err(Error::Invalid(
                    "overlay navigation canonical identity differs",
                ));
            }
            if let Some(payload) = &value.payload {
                let field = if collection == "nodes" {
                    "node_id"
                } else {
                    "edge_id"
                };
                if payload[field].as_str() != Some(id) {
                    return Err(Error::Invalid(
                        "overlay navigation payload identity differs",
                    ));
                }
            }
            // Present tombstone => no fallback to an older immutable base.
            value.payload
        } else {
            crate::managed_agent_producer::parent_navigation_member(
                self.base,
                validator,
                collection,
                id,
                l,
                base_limits,
                &mut work.base_charged_bytes,
                &mut work.base_candidate_rows,
            )?
        };
        self.base.check_pin()?;
        self.retention
            .require_store(&self.store)
            .map_err(|e| Error::Source(e.to_string()))?;
        Ok((result, work))
    }

    /// Existing Versions response at its exact addressed navigation key.
    /// The reference is content, not authority; current disclosure still needs
    /// the CMD source/owner hold surrounding this reader.
    #[allow(clippy::too_many_arguments)]
    pub fn record_version_member(
        &self,
        reference: &Value,
        validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
        l: crate::source_bibliographic::BibliographicLimits,
        base_limits: crate::knowledge_stage::StageLimits,
        tree_limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<Value>, ManagedOverlayLookupWorkV2)> {
        let id = format!(
            "record-version:{}",
            crate::source_bibliographic_render::digest(reference, l.catalog.max_row_bytes,)?
        );
        let (node, work) = self.navigation_member(
            "nodes",
            &id,
            validator,
            l,
            base_limits,
            tree_limits,
            deadline,
            cancelled,
        )?;
        if let Some(node) = &node {
            let view = &node["properties"]["record_version_view"];
            if node["node_kind"] != "record-version"
                || view["record_ref"] != *reference
                || view["grants_current_use"] != false
                || view["performs_assessment"] != false
            {
                return Err(Error::Invalid("overlay exact Versions response differs"));
            }
        }
        Ok((node, work))
    }

    pub fn source_binding(&self) -> &crate::ManagedOverlaySourceBindingV2 {
        &self.manifest.source
    }
}

/// One bounded genuine committed source member delivered by the CMD source
/// owner together with its addressed inventory projection. These bytes grant
/// no authority on their own; CMD retains complete delta/current source gates.
pub struct ManagedOverlayChangedMemberV2<'a> {
    pub path: &'a str,
    pub raw: &'a [u8],
    pub projection: Value,
}

/// Actual opt-in initial-Agent producer outcome. The CMD caller must supply
/// these inputs through its genuine committed-package and addressed projection
/// gate; descriptive delta fields alone do not attest a source transaction.
pub struct CompletedManagedOverlaySuccessorV2 {
    manifest: ManagedManifestV2,
    digest: tos_foundation::Digest256,
    pub catalog_work: AuthenticatedTreeWorkV1,
    pub navigation_work: AuthenticatedTreeWorkV1,
    pub manifest_work: AuthenticatedTreeWorkV1,
    pub base_candidate_rows: u64,
    pub base_charged_bytes: u64,
    pub changed_source_members: u64,
    pub changed_catalog_records: u64,
    pub source_input_bytes_consumed: u64,
    changed_member_inputs: Vec<(String, String, u64)>,
    created_record_input: (String, String, u64),
    created_forms_input: Option<(String, String, u64)>,
}
impl CompletedManagedOverlaySuccessorV2 {
    pub fn changed_member_inputs(&self) -> &[(String, String, u64)] {
        &self.changed_member_inputs
    }

    pub fn created_record_input(&self) -> (&str, &str, u64) {
        (
            &self.created_record_input.0,
            &self.created_record_input.1,
            self.created_record_input.2,
        )
    }
    pub fn created_forms_input(&self) -> Option<(&str, &str, u64)> {
        self.created_forms_input
            .as_ref()
            .map(|(path, sha, bytes)| (path.as_str(), sha.as_str(), *bytes))
    }
    pub fn manifest(&self) -> &ManagedManifestV2 {
        &self.manifest
    }
    pub fn manifest_digest(&self) -> tos_foundation::Digest256 {
        self.digest
    }
}

// Closed role validation over the existing source-owner initial Agent package.
// The source transaction/registered delta remains authority; captured runtime
// descriptions and receipt statements acquire no new grant here.
#[allow(clippy::too_many_arguments)]
fn verify_initial_agent_changed_members(
    members: &[ManagedOverlayChangedMemberV2<'_>],
    record_path: &str,
    record_raw: &[u8],
    form_path: &str,
    forms_raw: Option<&[u8]>,
    record_projection: &Value,
    record: &Value,
    entity_registry_sha256: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    use tos_foundation::Digest256;
    let home = record_path
        .rsplit_once('/')
        .ok_or(Error::Invalid("overlay Agent home"))?
        .0;
    let get = |name: &str| {
        members
            .iter()
            .find(|member| member.path == format!("{home}/{name}"))
            .ok_or(Error::Invalid("overlay Agent capture role absent"))
    };
    // The supported Agent registry currently contributes these exact two
    // source-owned baseline profiles to every member. New profile consumers
    // require the complete route; auxiliary files cannot introduce them.
    let profiles = record_projection["source_profiles"]
        .as_object()
        .ok_or(Error::Invalid("overlay source profile map"))?;
    let registry_schema = profiles
        .get("ToS/contracts/semantic-entity-type-registry.schema.json")
        .and_then(Value::as_str)
        .and_then(|digest| Digest256::from_hex(digest).ok());
    if profiles.len() != 2
        || profiles
            .get("ToS/doctrine/semantic-interchange/entity-types.v1.json")
            .and_then(Value::as_str)
            != Some(entity_registry_sha256)
        || registry_schema.is_none()
        || members.iter().any(|member| {
            member.projection["source_profiles"] != record_projection["source_profiles"]
        })
    {
        return Err(Error::ManagedSourceUnsupported(
            "overlay changed source profile contribution; FullOnly required",
        ));
    }
    let request_member = get("source-create-request.json")?;
    let environment_member = get("source-create-environment.json")?;
    let event_member = get("source-create-provenance.jsonl")?;
    let receipt_member = get("source-create-receipt.json")?;
    let request = crate::knowledge_normalization::SourceRow::parse(request_member.raw, max_bytes)?;
    let receipt = crate::knowledge_normalization::SourceRow::parse(receipt_member.raw, max_bytes)?;
    let event = crate::knowledge_normalization::SourceRow::parse(event_member.raw, max_bytes)?;
    // Environment is byte evidence only. It is parsed as a bounded object and
    // bound to the event, never treated as execution/publication authority.
    crate::knowledge_normalization::SourceRow::parse(environment_member.raw, max_bytes)?;
    // The source-owner receipt retains the exact metadata subject reference,
    // not the complete record body. Use its maintained kernel so identity,
    // version and canonical body digest keep the same source-owned meaning.
    let subject_limits = tos_foundation::JsonLimits::new(max_bytes, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("overlay receipt subject limits"))?;
    let source_record = tos_foundation::parse_json(
        record_raw,
        tos_foundation::JsonMode::PublishedStrict,
        subject_limits,
    )
    .map_err(|e| Error::Source(e.to_string()))?
    .into_root();
    let subject =
        tos_validation::source_forms::source_copy_kernel::metadata_subject(&source_record)
            .map_err(|_| Error::Invalid("overlay receipt maintained metadata subject"))?;
    let subject_raw = tos_foundation::canonical_bytes_v1(
        &subject,
        tos_foundation::CanonicalProfile::SourceCommandInputV1,
        subject_limits,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    let expected_subject: Value = serde_json::from_slice(&subject_raw)
        .map_err(|_| Error::Invalid("overlay receipt metadata subject JSON"))?;
    if request.value()["operation"] != "source.create"
        || request.value()["record"] != *record
        || request.value()["command_id"]
            .as_str()
            .is_none_or(str::is_empty)
        || receipt.value()["schema_version"] != "tos_local_source_create_receipt_v1"
        || receipt.value()["source_path"] != record_path
        || receipt.value()["source"] != expected_subject
        || receipt.value()["command_id"] != request.value()["command_id"]
        || receipt.value()["grants_admission"] != false
        || event.value()["schema_version"] != "tos_provenance_event_v2"
        || event.value()["event_version"] != 1
        || event.value()["record_binding"]["manifest_ref"] != receipt_member.path
    {
        return Err(Error::ManagedSourceUnsupported(
            "overlay initial Agent capture roles differ; FullOnly required",
        ));
    }
    if !tos_validation::provenance_rules::semantic_issues(event.value(), 128, deadline)
        .map_err(|_| Error::Invalid("overlay provenance semantic execution"))?
        .is_empty()
    {
        return Err(Error::Invalid("overlay provenance existing semantic rules"));
    }
    let canonical_request = tos_foundation::canonical_raw_bytes_v1(
        request_member.raw,
        tos_foundation::CanonicalProfile::SourceCommandInputV1,
        tos_foundation::JsonLimits::new(max_bytes, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("overlay capture canonical limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if receipt.value()["request_digest"] != Digest256::of_bytes(&canonical_request).to_prefixed() {
        return Err(Error::Invalid("overlay receipt/request byte binding"));
    }
    let files = receipt.value()["files"]
        .as_object()
        .ok_or(Error::Invalid("overlay receipt files"))?;
    if files.len() + 1 != members.len() {
        return Err(Error::Invalid("overlay receipt complete file coverage"));
    }
    for member in members {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Budget(
                "overlay changed members deadline/cancellation",
            ));
        }
        if member.raw.len() > max_bytes
            || member.projection["schema_version"] != "tos_managed_agent_inventory_member_v1"
            || member.projection["path"] != member.path
            || member.projection["raw_sha256"] != Digest256::of_bytes(member.raw).to_hex()
            || !member.projection["anchors"]
                .as_object()
                .is_some_and(serde_json::Map::is_empty)
        {
            return Err(Error::Invalid(
                "overlay changed member projection/raw binding",
            ));
        }
        if member.path == record_path {
            if member.raw != record_raw || &member.projection != record_projection {
                return Err(Error::Invalid("overlay record role differs"));
            }
        } else if !member.projection["records"]
            .as_object()
            .is_some_and(|records| {
                records
                    .values()
                    .all(|rows| rows.as_array().is_some_and(Vec::is_empty))
            })
        {
            return Err(Error::ManagedSourceUnsupported(
                "overlay auxiliary record contribution; FullOnly required",
            ));
        }
        if member.path == form_path {
            if forms_raw != Some(member.raw)
                || member.projection["form"]["raw_sha256"]
                    != Digest256::of_bytes(member.raw).to_prefixed()
            {
                return Err(Error::Invalid("overlay forms role differs"));
            }
        } else if !member.projection["form"].is_null() {
            return Err(Error::ManagedSourceUnsupported(
                "overlay auxiliary form contribution; FullOnly required",
            ));
        }
        let events = member.projection["events"]
            .as_object()
            .ok_or(Error::Invalid("overlay member events"))?;
        if member.path == event_member.path {
            let event_id = event.value()["event_id"]
                .as_str()
                .ok_or(Error::Invalid("overlay provenance identity"))?;
            let indexed = events
                .get(event_id)
                .ok_or(Error::Invalid("overlay provenance indexed role"))?;
            let canonical = tos_foundation::canonical_raw_bytes_v1(
                member.raw,
                tos_foundation::CanonicalProfile::SourceRecordDigestV1,
                tos_foundation::JsonLimits::new(max_bytes, 96, 1_000_000, 4096)
                    .map_err(|_| Error::Budget("overlay event canonical limits"))?,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
            if events.len() != 1
                || indexed["payload"] != *event.value()
                || indexed["source_ref"] != member.path
                || indexed["source_line"] != 1
                || indexed["source_sha256"] != Digest256::of_bytes(&canonical).to_hex()
            {
                return Err(Error::Invalid(
                    "overlay provenance projection/source binding",
                ));
            }
        } else if !events.is_empty() {
            return Err(Error::ManagedSourceUnsupported(
                "overlay auxiliary evidence contribution; FullOnly required",
            ));
        }
        if member.path != receipt_member.path {
            let name = member
                .path
                .strip_prefix(&format!("{home}/"))
                .ok_or(Error::Invalid("overlay same-home role"))?;
            let entry = files
                .get(name)
                .ok_or(Error::Invalid("overlay receipt member missing"))?;
            if entry["sha256"] != Digest256::of_bytes(member.raw).to_prefixed()
                || entry["bytes"] != member.raw.len() as u64
            {
                return Err(Error::Invalid("overlay receipt member byte binding"));
            }
        }
    }
    let mut observed = std::collections::BTreeSet::new();
    for group in ["inputs", "outputs", "byproducts"] {
        for entity in event.value()["entities"][group]
            .as_array()
            .ok_or(Error::Invalid("overlay provenance entities"))?
        {
            let path = entity["entity_ref"]
                .as_str()
                .ok_or(Error::Invalid("overlay provenance entity path"))?;
            let member = members
                .iter()
                .find(|member| member.path == path)
                .ok_or(Error::Invalid("overlay provenance entity absent"))?;
            if path == event_member.path
                || path == receipt_member.path
                || !observed.insert(path)
                || entity["sha256"] != Digest256::of_bytes(member.raw).to_hex()
                || entity["size_bytes"] != member.raw.len() as u64
                || entity["fixity_verified"] != false
            {
                return Err(Error::Invalid("overlay provenance entity byte coverage"));
            }
        }
    }
    if observed.len() + 2 != members.len()
        || event.value()["method"]["configuration_binding"]["ref"] != request_member.path
        || event.value()["method"]["configuration_binding"]["sha256"]
            != Digest256::of_bytes(request_member.raw).to_hex()
        || event.value()["method"]["environment"]["environment_profile_binding"]["ref"]
            != environment_member.path
        || event.value()["method"]["environment"]["environment_profile_binding"]["sha256"]
            != Digest256::of_bytes(environment_member.raw).to_hex()
    {
        return Err(Error::Invalid("overlay capture closed entity coverage"));
    }
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Budget(
            "overlay changed members deadline/cancellation",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_managed_overlay_initial_agent_v2(
    previous: &ManagedManifestV2,
    previous_digest: tos_foundation::Digest256,
    base: &VerifiedKnowledgeModel<'_>,
    source: crate::ManagedOverlaySourceBindingV2,
    delta: crate::ManagedManifestDeltaV2,
    committed_changed_members: &[ManagedOverlayChangedMemberV2<'_>],
    record_projection: &Value,
    record_path: &str,
    record_raw: &[u8],
    forms_raw: Option<&[u8]>,
    forms: &mut dyn crate::source_bibliographic::BibliographicForms,
    validator: &crate::source_witness_catalog::SourceCatalogValidator<'_>,
    schema_revision: tos_foundation::SourceRevision,
    entity_raw: &[u8],
    l: crate::source_bibliographic::BibliographicLimits,
    base_limits: crate::knowledge_stage::StageLimits,
    store: &SegmentStore,
    audited: &AuditedStoreRoot,
    tree_limits: AuthenticatedTreeLimitsV1,
    manifest_limits: crate::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CompletedManagedOverlaySuccessorV2> {
    use crate::source_bibliographic_render::text;
    use tos_foundation::Digest256;
    use tos_validation::source_cut::CutSchemaExecutor;
    source.validate()?;
    l.validate()?;
    base_limits.validate()?;
    let parent = ManagedOverlayReaderV2::new(base, previous, store, audited)?;
    if delta.parent_manifest_digest != previous_digest.to_hex()
        || delta.parent_source_binding_sha256 != previous.source.digest()?.to_hex()
        || delta.parent_through_commit_seq != previous.source.through_commit_seq
        || previous.source.through_commit_seq.checked_add(1) != Some(source.through_commit_seq)
        || source.selected_generation_digest == previous.source.selected_generation_digest
        || source.domain != previous.source.domain
        || source.store_id != previous.source.store_id
        || source.database_oid != previous.source.database_oid
        || source.state_profile_sha256 != previous.source.state_profile_sha256
        || source.bootstrap_source_revision != previous.source.bootstrap_source_revision
        || source.bootstrap_membership_sha256 != previous.source.bootstrap_membership_sha256
        || source.bootstrap_members != previous.source.bootstrap_members
        || source.domain_sha256 != previous.source.domain_sha256
        || source.epoch != previous.source.epoch
        || source.definition_sha256 != previous.source.definition_sha256
        || source.schema_profile_sha256 != previous.source.schema_profile_sha256
        || Digest256::of_bytes(entity_raw).to_hex() != previous.base.entity_registry_sha256
        || validator.worker.sha256.to_hex() != previous.schema_worker_sha256
    {
        return Err(Error::Invalid(
            "overlay initial Agent parent/software/source interval",
        ));
    }
    for digest in [
        &delta.committed_delta_sha256,
        &delta.committed_member_root_sha256,
    ] {
        Digest256::from_hex(digest)
            .map_err(|_| Error::Invalid("overlay committed delta identity"))?;
    }
    if record_raw.len() > l.catalog.max_row_bytes {
        return Err(Error::Budget("overlay addressed Agent source bytes"));
    }
    tos_foundation::RelativePath::parse(record_path)
        .map_err(|_| Error::Invalid("overlay Agent source path"))?;
    let record =
        crate::knowledge_normalization::SourceRow::parse(record_raw, l.catalog.max_row_bytes)?
            .value()
            .clone();
    let id = text(&record, "record_id")?;
    let records = record_projection["records"]
        .as_object()
        .ok_or(Error::Invalid("overlay addressed projection records"))?;
    let entries = records
        .get("agent")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("overlay addressed projection Agent entries"))?;
    if records
        .iter()
        .any(|(kind, entries)| kind != "agent" && !entries.as_array().is_some_and(Vec::is_empty))
    {
        return Err(Error::ManagedSourceUnsupported(
            "overlay requires only initial Agent projection; FullOnly required",
        ));
    }
    if record_projection["schema_version"] != "tos_managed_agent_inventory_member_v1"
        || record_projection["path"] != record_path
        || record_projection["raw_sha256"] != Digest256::of_bytes(record_raw).to_hex()
        || entries.len() != 1
        || entries[0]["record_id"] != id
        || record["record_type"] != "agent"
        || record["record_version"] != 1
    {
        return Err(Error::ManagedSourceUnsupported(
            "overlay requires exact initial Agent projection; FullOnly required",
        ));
    }
    let entry = &entries[0];
    if entry["source_record_ref"] != record_path
        || crate::source_witness_catalog::render_catalog_record(
            &record,
            record_path,
            entry.get("source_schema_ref").and_then(Value::as_str),
            l.catalog.max_row_bytes,
        )? != *entry
    {
        return Err(Error::Invalid(
            "overlay Agent addressed catalog/source differs",
        ));
    }
    let form_path = format!(
        "{}.human-forms.json",
        record_path
            .strip_suffix(".json")
            .ok_or(Error::Invalid("overlay Agent basename"))?
    );
    let home = record_path
        .rsplit_once('/')
        .ok_or(Error::Invalid("overlay source home"))?
        .0;
    if forms_raw.is_none() {
        return Err(Error::ManagedSourceUnsupported(
            "overlay initial Agent forms role absent; FullOnly required",
        ));
    }
    let mut expected = std::iter::once(record_path.to_owned())
        .chain(forms_raw.map(|_| form_path.clone()))
        .collect::<std::collections::BTreeSet<_>>();
    for name in [
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
        "source-create-receipt.json",
    ] {
        expected.insert(format!("{home}/{name}"));
    }
    let supplied = committed_changed_members
        .iter()
        .map(|member| member.path.to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    if supplied != expected || supplied.len() != committed_changed_members.len() {
        return Err(Error::ManagedSourceUnsupported(
            "overlay changed-member coverage incomplete; FullOnly required",
        ));
    }
    verify_initial_agent_changed_members(
        committed_changed_members,
        record_path,
        record_raw,
        &form_path,
        forms_raw,
        record_projection,
        &record,
        &previous.base.entity_registry_sha256,
        l.catalog.max_row_bytes,
        deadline,
        cancelled,
    )?;
    let materialized = {
        let mut schemas = validator.schemas(schema_revision)?;
        if schemas.execution_binding().schema_set_sha256.to_hex() != previous.schema_set_sha256
            || !schemas
                .check(
                    record_path,
                    record_raw,
                    "ToS/contracts/corpus-record.schema.json",
                    deadline,
                    cancelled,
                )
                .map_err(|e| Error::Source(format!("overlay Agent schema:{e:?}")))?
        {
            return Err(Error::Invalid("overlay Agent exact schema closure"));
        }
        let event_member = committed_changed_members
            .iter()
            .find(|member| member.path == format!("{home}/source-create-provenance.jsonl"))
            .ok_or(Error::Invalid("overlay provenance role absent"))?;
        if !schemas
            .check(
                event_member.path,
                event_member.raw,
                "ToS/contracts/provenance-event-v2.schema.json",
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(format!("overlay provenance schema:{e:?}")))?
        {
            return Err(Error::Invalid("overlay provenance exact schema closure"));
        }
        if let Some(raw) = forms_raw {
            if raw.len() > l.catalog.max_row_bytes
                || !schemas
                    .check(
                        &form_path,
                        raw,
                        "ToS/contracts/human-form-set.schema.json",
                        deadline,
                        cancelled,
                    )
                    .map_err(|e| Error::Source(format!("overlay Agent form schema:{e:?}")))?
            {
                return Err(Error::Invalid("overlay Agent form exact schema closure"));
            }
        }
        drop(schemas);
        forms_raw
            .map(|raw| {
                crate::source_bibliographic::materialize_checked_form_set(
                    raw, &form_path, &record, forms, l,
                )
            })
            .transpose()?
            .flatten()
    };
    let binding = parent.catalog.next_overlay_binding(
        source.through_commit_seq,
        &source.selected_generation_digest,
    )?;
    let (catalog, mut catalog_work) = parent.catalog.insert_created_agent_with_work(
        binding,
        entry,
        tree_limits,
        deadline,
        cancelled,
    )?;
    let entities = crate::knowledge_normalization::SourceRow::parse(
        entity_raw,
        base_limits.sqlite.max_row_bytes,
    )?
    .value()
    .clone();
    let (projection, render_work) =
        crate::source_bibliographic_navigation::project_managed_initial_agent_with_epoch_and_work(
            entry,
            &record,
            record_raw,
            &entities,
            &catalog,
            tree_limits,
            deadline,
            cancelled,
            materialized
                .as_ref()
                .map(|(reference, value)| (reference.as_str(), value)),
            l,
        )?;
    catalog_work.read_nodes = catalog_work
        .read_nodes
        .checked_add(render_work.read_nodes)
        .ok_or(Error::Budget("overlay catalog node work"))?;
    catalog_work.read_bytes = catalog_work
        .read_bytes
        .checked_add(render_work.read_bytes)
        .ok_or(Error::Budget("overlay catalog read work"))?;
    if !projection.diagnostics.is_empty() {
        return Err(Error::Invalid(
            "overlay Agent unresolved navigation diagnostics",
        ));
    }
    let mut changes = std::collections::BTreeMap::new();
    let mut base_candidate_rows = 0u64;
    let mut base_charged_bytes = 0u64;
    let mut navigation_reads = AuthenticatedTreeWorkV1::default();
    for (collection, field, values) in [
        ("nodes", "node_id", projection.nodes),
        ("edges", "edge_id", projection.edges),
    ] {
        for payload in values {
            let id = text(&payload, field)?.to_owned();
            let (existing, work) = parent.navigation_member(
                collection,
                &id,
                validator,
                l,
                base_limits,
                tree_limits,
                deadline,
                cancelled,
            )?;
            if existing.is_some() {
                return Err(Error::Invalid(
                    "initial Agent overwrites retained navigation member",
                ));
            }
            base_candidate_rows = base_candidate_rows
                .checked_add(work.base_candidate_rows)
                .ok_or(Error::Budget("overlay base row work"))?;
            base_charged_bytes = base_charged_bytes
                .checked_add(work.base_charged_bytes)
                .ok_or(Error::Budget("overlay base byte work"))?;
            navigation_reads.read_nodes = navigation_reads
                .read_nodes
                .checked_add(work.tree.read_nodes)
                .ok_or(Error::Budget("overlay navigation node work"))?;
            navigation_reads.read_bytes = navigation_reads
                .read_bytes
                .checked_add(work.tree.read_bytes)
                .ok_or(Error::Budget("overlay navigation read work"))?;
            let value = NavigationOverlayValueV2 {
                schema: "tos_managed_navigation_member_v2".into(),
                collection: collection.into(),
                id: id.clone(),
                payload: Some(payload),
            };
            if changes
                .insert(
                    navigation_key(collection, &id)?,
                    encode(
                        &serde_json::to_value(value).map_err(|e| Error::Source(e.to_string()))?,
                        tree_limits.max_value_bytes,
                    )?,
                )
                .is_some()
            {
                return Err(Error::Invalid("overlay duplicate produced identity"));
            }
        }
    }
    let (navigation, mut navigation_work) = store
        .apply_authenticated_tree_delta_v2_with_work(
            &parent.navigation,
            changes.into_iter().map(|(key, value)| {
                Ok(tos_segment_store::AuthenticatedTreeDeltaV1 {
                    key,
                    value: Some(value),
                })
            }),
            tree_limits,
            deadline,
            cancelled,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
    let retained = previous
        .retained_logical_bound_bytes
        .checked_add(
            catalog
                .retained_node_logical_bound()
                .checked_sub(parent.catalog.retained_node_logical_bound())
                .ok_or(Error::Invalid("overlay catalog retained bound regression"))?,
        )
        .and_then(|n| n.checked_add(navigation_work.read_bytes))
        .and_then(|n| n.checked_add(navigation_work.written_bytes))
        .and_then(|n| n.checked_add(manifest_limits.max_manifest_bytes as u64))
        .ok_or(Error::Budget("overlay retained generation byte bound"))?;
    navigation_work.read_nodes = navigation_work
        .read_nodes
        .checked_add(navigation_reads.read_nodes)
        .ok_or(Error::Budget("overlay navigation node work"))?;
    navigation_work.read_bytes = navigation_work
        .read_bytes
        .checked_add(navigation_reads.read_bytes)
        .ok_or(Error::Budget("overlay navigation read work"))?;
    let manifest = ManagedManifestV2 {
        schema: previous.schema.clone(),
        base: previous.base.clone(),
        base_provenance: previous.base_provenance.clone(),
        base_transport: previous.base_transport.clone(),
        schema_worker_sha256: previous.schema_worker_sha256.clone(),
        schema_set_sha256: previous.schema_set_sha256.clone(),
        source,
        catalog_epoch: catalog.descriptor(65_536)?,
        navigation_tree: navigation
            .encode(65_536)
            .map_err(|e| Error::Source(e.to_string()))?,
        delta: Some(delta),
        recovery_rebind: None,
        recovery_rebinds: previous.recovery_rebinds,
        retained_generations: previous
            .retained_generations
            .checked_add(1)
            .ok_or(Error::Budget("overlay retained generation count"))?,
        retained_logical_bound_bytes: retained,
    };
    manifest.require_successor_of(previous, previous_digest)?;
    validator.finish()?;
    let (digest, manifest_work) =
        manifest.persist(store, audited, manifest_limits, deadline, cancelled)?;
    base.check_pin()?;
    audited
        .require_store(store)
        .map_err(|e| Error::Source(e.to_string()))?;
    let source_input_bytes_consumed = committed_changed_members
        .iter()
        .try_fold(0u64, |bytes, member| {
            bytes.checked_add(member.raw.len() as u64)
        })
        .ok_or(Error::Budget("overlay source input byte work"))?;
    let changed_member_inputs = committed_changed_members
        .iter()
        .map(|member| {
            if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(Error::Budget(
                    "overlay completed member hash deadline/cancellation",
                ));
            }
            Ok((
                member.path.to_owned(),
                Digest256::of_bytes(member.raw).to_hex(),
                member.raw.len() as u64,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let created_record_input = (
        record_path.to_owned(),
        Digest256::of_bytes(record_raw).to_hex(),
        record_raw.len() as u64,
    );
    let created_forms_input = forms_raw.map(|raw| {
        (
            form_path,
            Digest256::of_bytes(raw).to_hex(),
            raw.len() as u64,
        )
    });
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Budget("overlay completion deadline/cancellation"));
    }
    Ok(CompletedManagedOverlaySuccessorV2 {
        manifest,
        digest,
        catalog_work,
        navigation_work,
        manifest_work,
        base_candidate_rows,
        base_charged_bytes,
        changed_source_members: supplied.len() as u64,
        changed_catalog_records: 1,
        source_input_bytes_consumed,
        changed_member_inputs,
        created_record_input,
        created_forms_input,
    })
}

/// No source change is manufactured by a recovery. The exact immutable model
/// roots/proof/base remain; only witnessed source authority is rebound by CMD.
pub struct CompletedManagedOverlayRecoveryV2 {
    manifest: ManagedManifestV2,
    digest: tos_foundation::Digest256,
    manifest_work: AuthenticatedTreeWorkV1,
}
impl CompletedManagedOverlayRecoveryV2 {
    pub fn manifest(&self) -> &ManagedManifestV2 {
        &self.manifest
    }
    pub fn manifest_digest(&self) -> tos_foundation::Digest256 {
        self.digest
    }
    pub fn manifest_work(&self) -> AuthenticatedTreeWorkV1 {
        self.manifest_work
    }
}
#[allow(clippy::too_many_arguments)]
pub fn prepare_managed_overlay_recovery_rebind_v2(
    previous: &ManagedManifestV2,
    previous_digest: tos_foundation::Digest256,
    base: &VerifiedKnowledgeModel<'_>,
    source: crate::ManagedOverlaySourceBindingV2,
    recovery: crate::ManagedManifestRecoveryRebindV2,
    store: &SegmentStore,
    audited: &AuditedStoreRoot,
    limits: crate::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CompletedManagedOverlayRecoveryV2> {
    source.validate()?;
    // The existing real base and both actual tree descriptors must already
    // have been cold-admitted by the restored operation's owner.
    let _ = ManagedOverlayReaderV2::new(base, previous, store, audited)?;
    let mut manifest = previous.clone();
    manifest.source = source;
    manifest.delta = None;
    manifest.recovery_rebind = Some(recovery);
    manifest.recovery_rebinds = previous
        .recovery_rebinds
        .checked_add(1)
        .ok_or(Error::Budget("managed recovery count"))?;
    manifest.retained_generations = previous
        .retained_generations
        .checked_add(1)
        .ok_or(Error::Budget("managed recovery retained depth"))?;
    manifest.retained_logical_bound_bytes = previous
        .retained_logical_bound_bytes
        .checked_add(limits.max_manifest_bytes as u64)
        .ok_or(Error::Budget("managed recovery manifest bound"))?;
    manifest.require_successor_of(previous, previous_digest)?;
    let (digest, manifest_work) = manifest.persist(store, audited, limits, deadline, cancelled)?;
    base.check_pin()?;
    Ok(CompletedManagedOverlayRecoveryV2 {
        manifest,
        digest,
        manifest_work,
    })
}
