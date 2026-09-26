//! Immutable carrier adapter for layer-family owner mechanics. Retained bytes
//! can satisfy exact digest-bound input references; they never replace current
//! record membership or current contracts. Complete EOF is carrier evidence.
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::layer_family_rules::{
    LayerFamilyReport, LayerFamilyRules, LayerFamilySource, LayerPayload,
};
use crate::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

pub struct CutLayerFamilySource<'a, S: CutSchemaExecutor> {
    pub cut: &'a CorpusCutReader,
    pub schemas: &'a mut S,
    pub cancelled: &'a AtomicBool,
    pub max_read_bytes: u64,
    pub read_bytes: u64,
    pub payloads: &'a mut dyn CutLayerPayloadReader,
}
impl<S: CutSchemaExecutor> LayerFamilySource for CutLayerFamilySource<'_, S> {
    fn current(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint(deadline)?;
        let path = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("layer-family source path".into()))?;
        if self.cut.current().member(&path).is_none() {
            return Ok(None);
        }
        let member = self
            .cut
            .read_member(
                self.cut.current().revision(),
                &path,
                max_bytes as u64,
                deadline,
                self.cancelled,
            )
            .map_err(store_error)?;
        self.read_bytes = self
            .read_bytes
            .checked_add(member.raw.len() as u64)
            .filter(|n| *n <= self.max_read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(Some(member.raw))
    }
    fn recorded(
        &mut self,
        path: &str,
        digest: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint(deadline)?;
        let expected = Digest256::from_hex(digest)
            .map_err(|_| ItemRefusal::Unsupported("layer-family input digest".into()))?;
        let path = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("layer-family retained path".into()))?;
        // Every opened revision belongs to the exact retained base chain.
        // Digest comparison, not availability, selects historical bytes.
        for snapshot in self.cut.revisions() {
            self.checkpoint(deadline)?;
            if snapshot.member(&path).is_none() {
                continue;
            }
            let member = self
                .cut
                .read_member(
                    snapshot.revision(),
                    &path,
                    max_bytes as u64,
                    deadline,
                    self.cancelled,
                )
                .map_err(store_error)?;
            self.read_bytes = self
                .read_bytes
                .checked_add(member.raw.len() as u64)
                .filter(|n| *n <= self.max_read_bytes)
                .ok_or(ItemRefusal::Budget)?;
            if Digest256::of_bytes(&member.raw) == expected {
                return Ok(Some(member.raw));
            }
        }
        Ok(None)
    }
    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        self.checkpoint(deadline)?;
        // Missing current schemas cannot acquire authority from a retained base.
        let relative = RelativePath::parse(contract)
            .map_err(|_| ItemRefusal::Unsupported("layer-family contract path".into()))?;
        if self.cut.current().member(&relative).is_none() {
            return Err(ItemRefusal::Unsupported(
                "missing current layer-family schema".into(),
            ));
        }
        self.schemas
            .check(path, raw, contract, deadline, self.cancelled)
    }
    fn exists(&mut self, path: &str, _: usize, deadline: Instant) -> Result<bool, ItemRefusal> {
        self.checkpoint(deadline)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("layer source-ref path".into()))?;
        Ok(self
            .cut
            .presence(self.cut.current().revision(), &relative)
            .is_some())
    }
    fn discovered_item_manifest(
        &mut self,
        path: &str,
        _: usize,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        self.checkpoint(deadline)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("layer manifest path".into()))?;
        Ok(path.starts_with("ToS/source-witnesses/")
            && path.ends_with("/item.manifest.json")
            && self.cut.current().member(&relative).is_some())
    }
    fn payload(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<LayerPayload, ItemRefusal> {
        self.checkpoint(deadline)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("layer payload path".into()))?;
        let observation = self
            .payloads
            .inspect(path, max_bytes, deadline, self.cancelled)?;
        self.checkpoint(deadline)?;
        match observation {
            LayerPayload::File {
                byte_size,
                sha256,
                sha1,
                jpeg_dimensions,
                ..
            } => {
                self.read_bytes = self
                    .read_bytes
                    .checked_add(byte_size)
                    .filter(|n| *n <= self.max_read_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                Ok(LayerPayload::File {
                    byte_size,
                    sha256,
                    sha1,
                    jpeg_dimensions,
                    source_member: self.cut.current().member(&relative).is_some(),
                })
            }
            other => Ok(other),
        }
    }
    fn cancellation(&self) -> &AtomicBool {
        self.cancelled
    }
    fn generation(&self) -> String {
        self.cut.current().revision().0.to_hex()
    }
    fn checkpoint(&self, deadline: Instant) -> Result<(), ItemRefusal> {
        check(deadline, self.cancelled)
    }
}
#[derive(Debug)]
pub struct SourceCutLayerFamilyReport {
    pub revision: SourceRevision,
    pub carrier_membership: SourceMembershipV1,
    pub layer_family: LayerFamilyReport,
}
/// Traverse current ordered bytes to EOF before executing selected family
/// predicates. This does not claim complete Python source-foundation coverage.
pub fn inspect_layers_from_cut(
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<SourceCutLayerFamilyReport, ItemRefusal> {
    let mut payloads = UnavailableLayerPayloads;
    inspect_layers_with_payloads_from_cut(cut, limits, cancelled, schemas, &mut payloads, false)
}
pub trait CutLayerPayloadReader {
    /// Selected immutable regular-file custody; enforce the supplied cap,
    /// streaming fixity and deadline/cancellation within the reader itself.
    fn inspect(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<LayerPayload, ItemRefusal>;
}
pub struct UnavailableLayerPayloads;
impl CutLayerPayloadReader for UnavailableLayerPayloads {
    fn inspect(
        &mut self,
        _: &str,
        _: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<LayerPayload, ItemRefusal> {
        check(deadline, cancelled)?;
        Ok(LayerPayload::Unavailable)
    }
}
pub fn inspect_layers_with_payloads_from_cut(
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    schemas: &mut CutWorkerSchemaExecutor,
    payloads: &mut dyn CutLayerPayloadReader,
    require_local_payloads: bool,
) -> Result<SourceCutLayerFamilyReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let revision = cut.current().revision();
    if schemas.source_revision() != revision {
        return Err(ItemRefusal::Source(
            "layer schema worker belongs to another corpus cut".into(),
        ));
    }
    let mut stream = cut.stream(revision).map_err(store_error)?;
    let mut paths = Vec::new();
    let mut bytes = 0u64;
    let mut index_bytes = 0usize;
    while let Some(member) = stream
        .next_member(limits.deadline, cancelled)
        .map_err(store_error)?
    {
        check(limits.deadline, cancelled)?;
        bytes = bytes
            .checked_add(member.raw.len() as u64)
            .filter(|n| *n <= limits.max_total_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let path = member.path.as_str();
        if !(path.starts_with("ToS/source-witnesses/") || path.starts_with("ToS/research-packets/"))
            || !path.ends_with(".json")
        {
            continue;
        }
        if member.raw.len() > limits.max_member_bytes
            || member
                .raw
                .len()
                .checked_mul(8)
                .is_none_or(|n| n > limits.max_state_bytes.saturating_sub(index_bytes))
        {
            return Err(ItemRefusal::Budget);
        }
        // Ordinary family loaders use Python decoded-field JSON. A malformed
        // selected path must remain visible rather than vanish in filtering.
        let selected_path = (path.starts_with("ToS/source-witnesses/server-import/plans/")
            && path
                .strip_prefix("ToS/source-witnesses/server-import/plans/")
                .is_some_and(|name| !name.contains('/')))
            || path.ends_with("/artifact-witness.json")
            || path.ends_with("/composite-witness.json")
            || ((path.starts_with("ToS/source-witnesses/artifacts/")
                || path.starts_with("ToS/source-witnesses/scholarly-composites/"))
                && path.ends_with("/representation.json"));
        // Refuse an unrepresentable decoded value rather than silently lose a
        // code-owned schema_version route. Known paths are decoded by rules.
        let selected = selected_path
            || match crate::native_decoded_value(&member.raw, limits.max_member_bytes) {
                Ok(v) => matches!(
                    v["schema_version"].as_str(),
                    Some(
                        "tos_source_text_unit_packet_v1"
                            | "tos_source_text_layer_v1"
                            | "tos_source_anchor_v2"
                            | "tos_semantic_annotation_packet_v2"
                            | "tos_translation_alignment_packet_v1"
                            | "tos_semantic_ladder_packet_v4"
                            | "tos_transfer_candidate_structural_crosswalk_v1"
                    )
                ),
                Err(ItemRefusal::Source(_)) => false,
                Err(reason) => return Err(reason),
            };
        if selected {
            index_bytes = index_bytes
                .checked_add(path.len() + 64)
                .filter(|n| *n <= limits.max_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            paths.push(path.to_owned());
        }
    }
    let membership = stream
        .coverage()
        .ok_or_else(|| ItemRefusal::Source("layer-family incomplete carrier EOF".into()))?;
    // Artifact identity closure must precede composite and representation reads.
    paths.sort_by_key(|path| {
        if path.ends_with("/artifact-witness.json") {
            0
        } else if path.ends_with("/composite-witness.json") {
            1
        } else {
            2
        }
    });
    let mut rule_limits = limits;
    rule_limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(index_bytes)
        .ok_or(ItemRefusal::Budget)?;
    rule_limits.max_total_bytes = limits
        .max_total_bytes
        .checked_sub(bytes)
        .ok_or(ItemRefusal::Budget)?;
    let mut source = CutLayerFamilySource {
        cut,
        schemas,
        cancelled,
        max_read_bytes: rule_limits.max_total_bytes,
        read_bytes: 0,
        payloads,
    };
    let mut rules = LayerFamilyRules::new(rule_limits);
    rules.require_local_payloads(require_local_payloads);
    rules.record_gap("ToS/research-packets/foundation-laboratory-2026-07","named-foundation-laboratory-history-assurance-private-evidence-selection-and-review-bundle-predicates")?;
    for path in paths {
        check(limits.deadline, cancelled)?;
        rules.inspect(&mut source, &path)?;
    }
    check(limits.deadline, cancelled)?;
    Ok(SourceCutLayerFamilyReport {
        revision,
        carrier_membership: membership,
        layer_family: rules.finish(),
    })
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        Err(ItemRefusal::Source("layer-family cancelled".into()))
    } else if Instant::now() >= deadline {
        Err(ItemRefusal::Deadline)
    } else {
        Ok(())
    }
}
fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    match error.code {
        tos_source_store::StoreErrorCode::BudgetExceeded => ItemRefusal::Budget,
        tos_source_store::StoreErrorCode::UnsupportedFormat
        | tos_source_store::StoreErrorCode::UnsupportedPlatform => {
            ItemRefusal::Unsupported(error.to_string())
        }
        _ => ItemRefusal::Source(error.to_string()),
    }
}
