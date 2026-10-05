//! Original component assembly from the same immutable public capture used by
//! the native snapshot. These projection bytes convey custody, not admission.
use crate::d1_public_capture::PublicCapture;
use crate::{
    CapturedCorpusOriginalPlan, CorpusOriginalSourceLimits, Error, NativeFamilyInputs,
    NativeProducerLimits, PhilosophyOriginalCollection, PhilosophyOriginalInput, QueryVocabulary,
    Result, SourceBinding, philosophy_original_rows_root,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};

pub(crate) struct OriginalPlans {
    corpus: CapturedCorpusOriginalPlan,
    header: Vec<u8>,
    nodes: Vec<Vec<u8>>,
    edges: Vec<Vec<u8>>,
    header_sha: String,
    nodes_root: String,
    edges_root: String,
    limits: crate::NavigationOriginalLimits,
    navigation: Option<NavigationOriginalPlan>,
}
struct NavigationOriginalPlan {
    rights: Vec<Vec<u8>>,
    rights_root: String,
}
pub(crate) struct BorrowedOriginalPlans<'a> {
    plans: &'a OriginalPlans,
    nodes: Vec<&'a [u8]>,
    edges: Vec<&'a [u8]>,
    rights: Option<Vec<&'a [u8]>>,
}
impl OriginalPlans {
    pub(crate) fn expected_model_abi(&self) -> &'static str {
        crate::KNOWLEDGE_CORPUS_MODEL_ABI
    }
    pub(crate) fn expected_model_abi_with_layout(
        &self,
        layout: crate::knowledge_stage::KnowledgePayloadLayout,
    ) -> &'static str {
        // Corpus V5 already dominates Philosophy V4 and optional Navigation V3;
        // the physical branch does not alter their logical receipt law.
        match layout {
            crate::knowledge_stage::KnowledgePayloadLayout::InlineV1 => self.expected_model_abi(),
            crate::knowledge_stage::KnowledgePayloadLayout::CarrierOnceV1 => {
                crate::knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI
            }
        }
    }

    pub(crate) fn borrowed_owned(
        &self,
        state: &crate::d1_public_capture::CreationState<'_>,
    ) -> Result<BorrowedOriginalPlans<'_>> {
        state.retain(
            (self.nodes.len()
                + self.edges.len()
                + self.navigation.as_ref().map_or(0, |plan| plan.rights.len()))
            .checked_mul(std::mem::size_of::<&[u8]>())
            .ok_or(Error::Budget("owned philosophy borrowed pointer slots"))?,
        )?;
        Ok(self.borrowed())
    }
    pub(crate) fn borrowed(&self) -> BorrowedOriginalPlans<'_> {
        BorrowedOriginalPlans {
            plans: self,
            nodes: self.nodes.iter().map(Vec::as_slice).collect(),
            edges: self.edges.iter().map(Vec::as_slice).collect(),
            rights: self
                .navigation
                .as_ref()
                .map(|plan| plan.rights.iter().map(Vec::as_slice).collect()),
        }
    }
}
impl BorrowedOriginalPlans<'_> {
    pub(crate) fn family_inputs(&self, limits: NativeProducerLimits) -> NativeFamilyInputs<'_> {
        let mut inputs = NativeFamilyInputs::bounded_from(limits);
        inputs.corpus_original = Some(&self.plans.corpus);
        inputs.philosophy_original = Some(PhilosophyOriginalInput {
            header: &self.plans.header,
            expected_header_sha256: &self.plans.header_sha,
            nodes: &self.nodes,
            edges: &self.edges,
            expected_nodes_root_sha256: &self.plans.nodes_root,
            expected_edges_root_sha256: &self.plans.edges_root,
            limits: self.plans.limits,
        });
        if let (Some(plan), Some(rights)) = (&self.plans.navigation, &self.rights) {
            inputs.navigation_original = Some(crate::NavigationOriginalInput {
                rights,
                expected_rights_root_sha256: &plan.rights_root,
                limits: self.plans.limits,
            });
        }
        inputs
    }
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("snapshot originals cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("snapshot originals deadline"));
    }
    Ok(())
}

fn navigation_original_plan(
    capture: &PublicCapture,
    limits: crate::NavigationOriginalLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<Option<NavigationOriginalPlan>> {
    check(deadline, cancelled)?;
    limits.validate()?;
    // Only the genuine selected complete capture binds originals. Legacy or
    // absent collections remain unavailable, never an invented empty receipt.
    for collection in [
        "source_navigation/nodes",
        "source_navigation/edges",
        "source_navigation/rights",
    ] {
        match capture
            .captured_collection_kind("corpus", collection)?
            .as_deref()
        {
            Some("array") => {}
            None => return Ok(None),
            _ => {
                return Err(Error::Invalid(
                    "snapshot navigation original collection kind",
                ));
            }
        }
    }
    let mut rights = Vec::new();
    let mut bytes = 0u64;
    capture.visit_original_rows("corpus", "source_navigation/rights", |_, raw| {
        check(deadline, cancelled)?;
        if raw.len() > limits.max_row_bytes || rights.len() as u64 >= limits.max_rows {
            return Err(Error::Budget("snapshot navigation original rights rows"));
        }
        bytes = bytes
            .checked_add(raw.len() as u64)
            .filter(|n| *n <= limits.max_total_bytes)
            .ok_or(Error::Budget("snapshot navigation original rights bytes"))?;
        if let Some(state) = state {
            state.retain(
                raw.len()
                    .checked_add(4 * std::mem::size_of::<Vec<u8>>())
                    .ok_or(Error::Budget("owned navigation original rights state"))?,
            )?;
            state.charge_work(raw.len())?;
        } else {
            capture.charge_work(raw.len() as u64)?;
        }
        rights.push(raw.to_vec());
        Ok(())
    })?;
    if let Some(state) = state {
        state.retain(
            rights
                .len()
                .checked_mul(std::mem::size_of::<&[u8]>())
                .and_then(|n| n.checked_add(64))
                .ok_or(Error::Budget("owned navigation original rights root state"))?,
        )?;
    }
    let borrowed = rights.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let rights_root =
        crate::knowledge_navigation_original::navigation_original_rights_root_with_check(
            &borrowed,
            &mut |bytes| {
                check(deadline, cancelled)?;
                match state {
                    Some(state) => state.charge_work(bytes),
                    None => capture.charge_work(bytes as u64),
                }
            },
        )?;
    check(deadline, cancelled)?;
    capture.check_custody()?;
    Ok(Some(NavigationOriginalPlan {
        rights,
        rights_root,
    }))
}
pub(crate) fn prepare(
    capture: &PublicCapture,
    binding: &SourceBinding,
    vocabulary: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<OriginalPlans> {
    prepare_inner(
        capture, binding, vocabulary, limits, deadline, cancelled, None,
    )
}

pub(crate) fn prepare_owned<'a>(
    capture: &'a PublicCapture,
    binding: &SourceBinding,
    vocabulary: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    state: &'a crate::d1_public_capture::CreationState<'a>,
) -> Result<OriginalPlans> {
    prepare_inner(
        capture,
        binding,
        vocabulary,
        limits,
        deadline,
        cancelled,
        Some(state),
    )
}

fn prepare_inner<'a>(
    capture: &'a PublicCapture,
    binding: &SourceBinding,
    vocabulary: &QueryVocabulary,
    limits: CorpusOriginalSourceLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    state: Option<&'a crate::d1_public_capture::CreationState<'a>>,
) -> Result<OriginalPlans> {
    check(deadline, cancelled)?;
    limits.originals.validate()?;
    if let Some(state) = state {
        state.retain(128 + std::mem::size_of::<OriginalPlans>())?;
    }
    let path = RelativePath::parse("ToS/derived-exports/tos_corpus_index.min.json")
        .map_err(|_| Error::Invalid("snapshot corpus path"))?;
    let corpus = match state {
        Some(state) => crate::knowledge_corpus_source::prepare_runtime_corpus_original_owned(
            capture, &path, binding, vocabulary, limits, deadline, cancelled, state,
        )?,
        None => crate::knowledge_corpus_source::prepare_runtime_corpus_original(
            capture, &path, binding, vocabulary, limits, deadline, cancelled,
        )?,
    };
    let navigation =
        navigation_original_plan(capture, limits.originals, deadline, cancelled, state)?;
    let mut header = match state {
        Some(state) => {
            capture.header_object_owned("philosophy", "", limits.originals.max_row_bytes, state)?
        }
        None => capture.header_object("philosophy", "", limits.originals.max_row_bytes)?,
    };
    let mut count = 1u64;
    let mut total = 0u64;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for (collection, output) in [("nodes", &mut nodes), ("edges", &mut edges)] {
        capture.visit_rows("philosophy", collection, |_, raw| {
            check(deadline, cancelled)?;
            count = count
                .checked_add(1)
                .filter(|n| *n <= limits.originals.max_rows)
                .ok_or(Error::Budget("snapshot philosophy original rows"))?;
            total = total
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= limits.originals.max_total_bytes)
                .ok_or(Error::Budget("snapshot philosophy original bytes"))?;
            if raw.len() > limits.originals.max_row_bytes {
                return Err(Error::Budget("snapshot philosophy original row"));
            }
            if let Some(state) = state {
                state.retain(
                    raw.len()
                        .checked_add(4 * std::mem::size_of::<Vec<u8>>())
                        .ok_or(Error::Budget("owned philosophy raw/row slots"))?,
                )?;
            }
            output.push(raw.to_vec());
            Ok(())
        })?;
    }
    // Views and clusters are original header material for the maintained raw
    // philosophy reader, not normalized node/edge families.
    for collection in ["views", "clusters", "review_packets", "graph_layers"] {
        if let Some(state) = state {
            state.retain(7)?;
        }
        match capture
            .captured_collection_kind("philosophy", collection)?
            .as_deref()
        {
            None => continue,
            Some("array") => {}
            _ => {
                return Err(Error::Invalid(
                    "snapshot philosophy original collection kind",
                ));
            }
        }
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        let mut rows_count = 0u64;
        capture.visit_rows("philosophy", collection, |_, raw| {
            check(deadline, cancelled)?;
            rows_count = rows_count
                .checked_add(1)
                .filter(|n| *n <= limits.originals.max_rows)
                .ok_or(Error::Budget("snapshot philosophy header rows"))?;
            bytes = bytes
                .checked_add(raw.len())
                .filter(|n| *n <= limits.originals.max_row_bytes)
                .ok_or(Error::Budget("snapshot philosophy header collection"))?;
            let value = if let Some(state) = state {
                state.retain(4 * std::mem::size_of::<serde_json::Value>())?;
                state.serde_owned(raw, limits.originals.max_row_bytes)?
            } else {
                serde_json::from_slice::<serde_json::Value>(raw)
                    .map_err(|_| Error::Invalid("snapshot philosophy header row"))?
            };
            rows.push(value);
            Ok(())
        })?;
        if let Some(state) = state {
            let count = header
                .as_object()
                .ok_or(Error::Invalid("snapshot philosophy header"))?
                .len();
            state.retain(
                crate::knowledge_normalization::serde_object_slots_upper(count + 1)?
                    + collection.len(),
            )?;
        }
        header
            .as_object_mut()
            .ok_or(Error::Invalid("snapshot philosophy header"))?
            .insert(collection.into(), serde_json::Value::Array(rows));
    }
    let header = match state {
        Some(state) => state.encode_canonical(&header, limits.originals.max_row_bytes)?,
        None => crate::knowledge_corpus_source::encode(&header, limits.originals.max_row_bytes)?,
    };
    total
        .checked_add(header.len() as u64)
        .filter(|n| *n <= limits.originals.max_total_bytes)
        .ok_or(Error::Budget("snapshot philosophy original total"))?;
    if let Some(state) = state {
        state.retain(
            (nodes.len() + edges.len())
                .checked_mul(std::mem::size_of::<&[u8]>())
                .and_then(|n| n.checked_add(3 * 64))
                .ok_or(Error::Budget("owned philosophy root pointer/string state"))?,
        )?;
    }
    let nodes_borrowed = nodes.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let edges_borrowed = edges.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let (nodes_root, edges_root) = if let Some(state) = state {
        let mut check = |bytes| state.charge_work(bytes);
        (
            crate::knowledge_philosophy_original::philosophy_original_rows_root_with_check(
                PhilosophyOriginalCollection::Nodes,
                &nodes_borrowed,
                &mut check,
            )?,
            crate::knowledge_philosophy_original::philosophy_original_rows_root_with_check(
                PhilosophyOriginalCollection::Edges,
                &edges_borrowed,
                &mut check,
            )?,
        )
    } else {
        (
            philosophy_original_rows_root(PhilosophyOriginalCollection::Nodes, &nodes_borrowed),
            philosophy_original_rows_root(PhilosophyOriginalCollection::Edges, &edges_borrowed),
        )
    };
    let header_sha = Digest256::of_bytes(&header).to_hex();
    check(deadline, cancelled)?;
    capture.check_custody()?;
    Ok(OriginalPlans {
        corpus,
        header,
        nodes,
        edges,
        header_sha,
        nodes_root,
        edges_root,
        limits: limits.originals,
        navigation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::d1_public_capture::{
        PublicCaptureInputPaths, PublicCaptureLimits, RuntimeCaptureRole,
    };
    use std::{fs, sync::Arc, time::Duration};

    #[test]
    fn navigation_plan_preserves_captured_rights_order_and_absence() {
        for present in [true, false] {
            let district = tempfile::tempdir().unwrap();
            let root = district.path();
            let source = root.join("corpus.json");
            let mut navigation = serde_json::json!({
                "schema_version":"tos_source_navigation_v1", "nodes":[], "edges":[],
                "counts":{"nodes":0,"edges":0,"rights":2}
            });
            if present {
                navigation["rights"] = serde_json::json!([
                    {"rights_id":"z-first","visibility":"public_metadata_only"},
                    {"rights_id":"a-second","visibility":"public_metadata_only"}
                ]);
            }
            fs::write(
                &source,
                serde_json::to_vec(&serde_json::json!({
                    "schema_version":"tos_corpus_index_v1", "source_navigation":navigation
                }))
                .unwrap(),
            )
            .unwrap();
            let absent = root.join("absent.json");
            let selected = PublicCaptureInputPaths {
                index_path: source,
                philosophy_graph_projection_path: absent.clone(),
                bibliographic_graph_path: absent.clone(),
                entity_type_registry_path: absent.clone(),
                relation_type_registry_path: absent.clone(),
                philosophy_post_planting_audit_path: absent.clone(),
                evidence_projection_path: absent,
            };
            let deadline = Instant::now() + Duration::from_secs(30);
            let capture = PublicCapture::create_runtime_carrier_selected(
                root,
                &selected,
                RuntimeCaptureRole::Corpus,
                &root.join("capture.sqlite"),
                PublicCaptureLimits {
                    max_input_bytes: 1024 * 1024,
                    max_rows: 100,
                    max_staging_bytes: 16 * 1024 * 1024,
                    max_work_bytes: 16 * 1024 * 1024,
                    max_sql_vm_steps: 1_000_000,
                    sqlite_cache_kib: 64,
                },
                deadline,
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
            let limits = crate::NavigationOriginalLimits {
                max_rows: 100,
                max_row_bytes: 1024 * 1024,
                max_total_bytes: 1024 * 1024,
            };
            let plan =
                navigation_original_plan(&capture, limits, deadline, &AtomicBool::new(false), None)
                    .unwrap();
            if !present {
                assert!(plan.is_none()); // No fabricated empty-rights component.
                continue;
            }
            let plan = plan.unwrap();
            let ids = plan
                .rights
                .iter()
                .map(|raw| {
                    serde_json::from_slice::<serde_json::Value>(raw).unwrap()["rights_id"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                })
                .collect::<Vec<_>>();
            assert_eq!(ids, ["z-first", "a-second"]);
            let mut sorted = Vec::new();
            capture
                .visit_rows("corpus", "source_navigation/rights", |_, raw| {
                    sorted.push(raw.to_vec());
                    Ok(())
                })
                .unwrap();
            sorted.sort_by_key(|raw| {
                serde_json::from_slice::<serde_json::Value>(raw).unwrap()["rights_id"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            });
            assert_ne!(plan.rights, sorted);
            let borrowed = plan.rights.iter().map(Vec::as_slice).collect::<Vec<_>>();
            assert_eq!(
                plan.rights_root,
                crate::navigation_original_rights_root(&borrowed)
            );
            assert_ne!(
                plan.rights_root,
                crate::navigation_original_rights_root(
                    &sorted.iter().map(Vec::as_slice).collect::<Vec<_>>()
                )
            );
            let tiny = crate::NavigationOriginalLimits {
                max_row_bytes: 1,
                ..limits
            };
            assert!(
                navigation_original_plan(&capture, tiny, deadline, &AtomicBool::new(false), None)
                    .is_err()
            );
        }
    }
}
