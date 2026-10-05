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
}
pub(crate) struct BorrowedOriginalPlans<'a> {
    plans: &'a OriginalPlans,
    nodes: Vec<&'a [u8]>,
    edges: Vec<&'a [u8]>,
}
impl OriginalPlans {
    pub(crate) fn expected_model_abi(&self) -> &'static str {
        crate::KNOWLEDGE_CORPUS_MODEL_ABI
    }
    pub(crate) fn expected_model_abi_with_layout(
        &self,
        layout: crate::knowledge_stage::KnowledgePayloadLayout,
    ) -> &'static str {
        // Both actual Corpus and Philosophy plans are retained in this owner;
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
            (self.nodes.len() + self.edges.len())
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
    })
}
