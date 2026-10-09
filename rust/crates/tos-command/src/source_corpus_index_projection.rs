//! Compose the maintained Rust ToS corpus families for the standalone native
//! corpus-index owner. This module owns no invocation, capture, or publication
//! policy; the caller supplies the exact selected cut, software image, worker,
//! sealed binding, and private quota-backed stage.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_compiler::knowledge_canon_source::{
    self as source_canon_source, CanonSourceLimits, render_canon_source_plan,
};
use tos_compiler::knowledge_repository_source::{
    RepositorySourceLimits, render_repository_source_plan,
};
use tos_compiler::knowledge_stage::{
    ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage, StageIsolation,
    StageLimits, StageOwner,
};
use tos_compiler::source_bibliographic::{
    self as source_bibliographic, BibliographicLimits, BibliographicSourceCut,
};
use tos_compiler::source_corpus::{NativeCorpusLimits, NativeCorpusProjection};
use tos_compiler::source_navigation_source::project_source_navigation_from_cut;
use tos_compiler::source_witness_catalog::SourceCatalogValidator;
use tos_compiler::{
    QueryVocabulary, RepositoryRootInput, SourceBinding, SourceCatalogInputLimits,
    plan_source_catalog_inputs, render_source_bibliographic_plan,
};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SourceMembershipV1};
use tos_validation::executor::{BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::CutWorkerLimits;

const VOCABULARY_PATH: &str = "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json";
const ENTITY_TYPES_PATH: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const CANON_NODE_CONTRACT: &str = "ToS/contracts/tos-node-contract.schema.json";
const CANON_FORMS_CONTRACT: &str = "ToS/contracts/human-form-set.schema.json";
const BIBLIOGRAPHIC_GRAPH_PATH: &str =
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json";
const BIBLIOGRAPHIC_GRAPH_SCHEMA: &str =
    "ToS/contracts/source-witness-bibliographic-graph.schema.json";
const BIBLIOGRAPHIC_GENERATOR: &str = "tos-native-owner-command corpus-index";

pub(crate) struct NativeCorpusIndexProducts {
    pub corpus: NativeCorpusProjection,
    pub bibliographic_claims: Vec<u8>,
    pub bibliographic_receipt: NativeBibliographicProjectionReceipt,
}

#[derive(Clone, Debug)]
pub(crate) struct NativeBibliographicProjectionReceipt {
    pub source_revision: String,
    pub source_membership_sha256: String,
    pub source_claims: u64,
    pub nodes: u64,
    pub edges: u64,
    pub claim_traces: u64,
    pub output_sha256: String,
    pub output_bytes: u64,
}

#[derive(Default)]
struct BibliographicRows {
    nodes: Vec<serde_json::Value>,
    edges: Vec<serde_json::Value>,
    claim_traces: Vec<serde_json::Value>,
}

impl source_bibliographic::BibliographicSink for BibliographicRows {
    fn row(&mut self, collection: &str, id: &str, raw: &[u8]) -> tos_compiler::Result<()> {
        let value: serde_json::Value = serde_json::from_slice(raw)
            .map_err(|_| tos_compiler::Error::Invalid("native bibliographic row JSON"))?;
        let (rows, field) = match collection {
            "nodes" => (&mut self.nodes, "node_id"),
            "edges" => (&mut self.edges, "edge_id"),
            "claim_traces" => (&mut self.claim_traces, "claim_ref"),
            _ => {
                return Err(tos_compiler::Error::Invalid(
                    "native bibliographic collection",
                ));
            }
        };
        if value.get(field).and_then(serde_json::Value::as_str) != Some(id)
            || rows
                .last()
                .and_then(|prior| prior.get(field))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|prior| prior >= id)
        {
            return Err(tos_compiler::Error::Invalid(
                "native bibliographic sorted row identity",
            ));
        }
        rows.push(value);
        Ok(())
    }
}

fn count_field(rows: &[serde_json::Value], field: &str) -> tos_compiler::Result<serde_json::Value> {
    let mut counts = BTreeMap::<String, u64>::new();
    for row in rows {
        let value = row
            .get(field)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(tos_compiler::Error::Invalid(
                "native bibliographic count field",
            ))?;
        let count = counts.entry(value.to_owned()).or_default();
        *count = count.checked_add(1).ok_or(tos_compiler::Error::Budget(
            "native bibliographic count overflow",
        ))?;
    }
    serde_json::to_value(counts)
        .map_err(|_| tos_compiler::Error::Invalid("native bibliographic counts JSON"))
}

fn selected_bibliographic_input_digests(
    cut: &CorpusCutReader,
    max_members: u64,
) -> tos_compiler::Result<BTreeMap<String, String>> {
    let mut digests = BTreeMap::new();
    for member in cut.current().members() {
        let path = member.path.as_str();
        let selected = path.starts_with("ToS/source-witnesses/") && !path.contains("/payload/")
            || path.starts_with("ToS/contracts/")
            || path.starts_with("ToS/doctrine/");
        if !selected {
            continue;
        }
        if digests.len() as u64 >= max_members {
            return Err(tos_compiler::Error::Budget(
                "native bibliographic input digest member count",
            ));
        }
        if digests
            .insert(path.to_owned(), member.sha256.to_hex())
            .is_some()
        {
            return Err(tos_compiler::Error::Invalid(
                "duplicate native bibliographic input digest path",
            ));
        }
    }
    if digests.len() < 10 {
        return Err(tos_compiler::Error::Invalid(
            "native bibliographic source digest closure",
        ));
    }
    Ok(digests)
}

fn bibliographic_projection(
    cut: &CorpusCutReader,
    candidate: &tos_compiler::SourceBibliographicCandidate,
    stage: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    limits: BibliographicLimits,
    max_output_bytes: u64,
    max_digest_members: u64,
) -> tos_compiler::Result<(Vec<u8>, NativeBibliographicProjectionReceipt)> {
    let mut rows = BibliographicRows::default();
    source_bibliographic::render_bibliographic_graph(
        stage,
        &candidate.catalog,
        &candidate.bibliographic,
        limits,
        &mut rows,
    )?;
    if rows.nodes.len() as u64 != candidate.bibliographic.node_count
        || rows.edges.len() as u64 != candidate.bibliographic.edge_count
        || rows.claim_traces.len() as u64 != candidate.bibliographic.claim_count
    {
        return Err(tos_compiler::Error::Invalid(
            "native bibliographic receipt row counts",
        ));
    }
    let record_files = candidate
        .catalog
        .manifest
        .get("record_files")
        .and_then(serde_json::Value::as_object)
        .ok_or(tos_compiler::Error::Invalid(
            "native bibliographic record files",
        ))?;
    let counts = candidate
        .catalog
        .manifest
        .get("counts")
        .and_then(serde_json::Value::as_object)
        .ok_or(tos_compiler::Error::Invalid(
            "native bibliographic catalog counts",
        ))?;
    let mut object_catalog_refs = serde_json::Map::new();
    for (kind, path) in record_files {
        if path
            .as_str()
            .is_none_or(|path| !path.starts_with("ToS/source-witnesses/catalog/"))
        {
            return Err(tos_compiler::Error::Invalid(
                "native bibliographic object catalog path",
            ));
        }
        object_catalog_refs.insert(kind.clone(), path.clone());
    }
    let mut graph_layers = vec![serde_json::Value::String("bibliographic".into())];
    if ["historical-event", "historical-process", "historical-state"]
        .iter()
        .any(|kind| {
            counts
                .get(*kind)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                > 0
        })
        || rows.claim_traces.iter().any(|trace| {
            trace
                .get("assertion_layer")
                .and_then(serde_json::Value::as_str)
                == Some("historical")
        })
    {
        graph_layers.push(serde_json::Value::String("historical".into()));
    }
    if rows.claim_traces.iter().any(|trace| {
        trace
            .get("source_claim_file_ref")
            .and_then(serde_json::Value::as_str)
            != Some("ToS/source-witnesses/catalog/claims.jsonl")
    }) {
        graph_layers.push(serde_json::Value::String("source-profile".into()));
    }
    if counts
        .get("artifact")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        > 0
    {
        graph_layers.push(serde_json::Value::String("physical-artifact".into()));
    }
    if counts
        .get("composite")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        > 0
    {
        graph_layers.push(serde_json::Value::String("scholarly-composite".into()));
    }
    let mut review_counts = BTreeMap::<String, u64>::new();
    let mut visibility_counts = BTreeMap::<String, u64>::new();
    for trace in &rows.claim_traces {
        for (field, target) in [
            ("review_status", &mut review_counts),
            ("visibility", &mut visibility_counts),
        ] {
            let value = trace
                .get(field)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(tos_compiler::Error::Invalid(
                    "native bibliographic trace classification",
                ))?;
            *target.entry(value.to_owned()).or_default() += 1;
        }
    }
    let mut input_digests = selected_bibliographic_input_digests(cut, max_digest_members)?;
    if !input_digests.contains_key(BIBLIOGRAPHIC_GRAPH_SCHEMA) {
        return Err(tos_compiler::Error::Invalid(
            "native bibliographic schema absent from source digest closure",
        ));
    }
    let mut payload = serde_json::json!({
        "schema_version": "tos_source_witness_bibliographic_graph_v1",
        "schema_ref": BIBLIOGRAPHIC_GRAPH_SCHEMA,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_source_witness_bibliographic_claim_graph",
        "generated_by": BIBLIOGRAPHIC_GENERATOR,
        "source_refs": {
            "catalog_manifest_ref": "ToS/source-witnesses/catalog/catalog.manifest.json",
            "claim_catalog_ref": "ToS/source-witnesses/catalog/claims.jsonl",
            "object_catalog_refs": object_catalog_refs,
        },
        "input_digests": input_digests,
        "graph_layers": graph_layers,
        "relation_model": {
            "assertion_form": "reified_claim_node",
            "direct_subject_object_edges": false,
            "edge_trace_rule": "every edge starts at its claim node and carries the claim digest, evidence nodes, maker node, provenance event node, and review status",
            "runtime_owner": "abyss-stack",
        },
        "counts": {
            "source_claims": candidate.bibliographic.claim_count,
            "nodes": rows.nodes.len(),
            "edges": rows.edges.len(),
            "claim_traces": rows.claim_traces.len(),
            "direct_subject_object_edges": 0,
            "node_kinds": count_field(&rows.nodes, "node_kind")?,
            "edge_kinds": count_field(&rows.edges, "edge_kind")?,
        },
        "review_counts": review_counts,
        "visibility_counts": visibility_counts,
        "nodes": rows.nodes,
        "edges": rows.edges,
        "claim_traces": rows.claim_traces,
        "authority_boundary": {
            "authoritative_claims": "source claim packets named by source_claim_file_ref, source_claim_line, and source_claim_sha256",
            "projection_role": "generated public-safe navigation over the tracked bibliographic claim catalog; deletable and rebuildable",
            "does_not_establish": [],
        },
        "validation_refs": [
            "rust/crates/tos-compiler/src/source_bibliographic.rs",
            "rust/crates/tos-command/src/source_corpus_index_projection.rs",
            "docs/validation/validation_lanes.json",
        ],
    });
    let fingerprint_material = serde_json::to_vec(&payload)
        .map_err(|_| tos_compiler::Error::Invalid("native bibliographic fingerprint JSON"))?;
    payload["projection_fingerprint"] =
        serde_json::Value::String(Digest256::of_bytes(&fingerprint_material).to_hex());
    let mut output = serde_json::to_vec(&payload)
        .map_err(|_| tos_compiler::Error::Invalid("native bibliographic output JSON"))?;
    output.push(b'\n');
    if output.len() as u64 > max_output_bytes {
        return Err(tos_compiler::Error::Budget(
            "native bibliographic projection output bytes",
        ));
    }
    validator.check_selected_document(
        cut.current().revision(),
        BIBLIOGRAPHIC_GRAPH_PATH,
        &output,
        BIBLIOGRAPHIC_GRAPH_SCHEMA,
    )?;
    let output_sha256 = Digest256::of_bytes(&output).to_hex();
    let output_bytes = output.len() as u64;
    Ok((
        output,
        NativeBibliographicProjectionReceipt {
            source_revision: cut.current().revision().0.to_hex(),
            source_membership_sha256: cut
                .stream(cut.current().revision())
                .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
                .expectation()
                .digest
                .to_hex(),
            source_claims: candidate.bibliographic.claim_count,
            nodes: candidate.bibliographic.node_count,
            edges: candidate.bibliographic.edge_count,
            claim_traces: candidate.bibliographic.claim_count,
            output_sha256,
            output_bytes,
        },
    ))
}

/// All limits are selected by the caller's protected invocation. The compiler
/// APIs still enforce their own hard ceilings; these values are the tighter
/// per-run envelope shared with the native owner and private TMPFS ticket.
#[derive(Clone, Copy)]
pub(crate) struct CorpusIndexProjectionLimits {
    pub catalog_input: SourceCatalogInputLimits,
    pub bibliographic: BibliographicLimits,
    pub repository: RepositorySourceLimits,
    pub canon: CanonSourceLimits,
    pub stage: StageLimits,
    pub schema_worker: ExecutorBudget,
    pub schema_worker_limits: CutWorkerLimits,
    pub schema_work: BatchStreamBudget,
    pub originals: tos_compiler::NavigationOriginalLimits,
    pub max_canon_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_work_bytes: u64,
}

struct BoundSourceStageOwner<'a> {
    binding: &'a SourceBinding,
    cut: &'a CorpusCutReader,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    recheck: &'a dyn Fn() -> tos_compiler::Result<()>,
}

fn binding_matches(left: &SourceBinding, right: &SourceBinding) -> bool {
    left.owner_profile == right.owner_profile
        && left.source_cut == right.source_cut
        && left.through_commit_seq == right.through_commit_seq
        && left.membership_root == right.membership_root
        && left.index_generation == right.index_generation
        && left.route_map_version == right.route_map_version
        && left.reader_abi == right.reader_abi
        && left.projection_root_sha256 == right.projection_root_sha256
        && left.complete == right.complete
}

impl BoundSourceStageOwner<'_> {
    fn check_current(&self) -> tos_compiler::Result<()> {
        (self.recheck)()?;
        if self.cut.current().revision() != self.revision
            || self
                .cut
                .stream(self.revision)
                .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
                .expectation()
                != self.membership
            || self.binding.membership_root != self.membership.digest.to_hex()
            || self.binding.index_generation != self.revision.0.to_hex()
        {
            return Err(tos_compiler::Error::Invalid(
                "native corpus selected cut changed",
            ));
        }
        Ok(())
    }
}

impl StageOwner for BoundSourceStageOwner<'_> {
    fn verify_receipt(&self, receipt: &ExactInputReceipt) -> tos_compiler::Result<()> {
        self.check_current()?;
        if !binding_matches(&receipt.binding, self.binding)
            || receipt.collections.is_empty()
            || receipt.collections.len() > 16_384
        {
            return Err(tos_compiler::Error::Invalid(
                "native corpus stage source receipt",
            ));
        }
        Ok(())
    }

    fn recheck_sealed_cut(&self, receipt: &ExactInputReceipt) -> tos_compiler::Result<()> {
        self.verify_receipt(receipt)
    }
}

fn raw_receipt(
    source_graph: &str,
    collection: &str,
    input_role: &str,
    adapter_profile: &str,
    rows: &BTreeMap<String, Vec<u8>>,
) -> InputCollectionReceipt {
    let mut root = Digest256Hasher::new();
    for (id, raw) in rows {
        root.update(&(id.len() as u64).to_be_bytes());
        root.update(id.as_bytes());
        root.update(Digest256::of_bytes(raw).as_bytes());
    }
    InputCollectionReceipt {
        source_graph: source_graph.into(),
        collection: collection.into(),
        input_role: input_role.into(),
        adapter_profile: adapter_profile.into(),
        expected_count: rows.len() as u64,
        expected_root_sha256: root.finalize().to_hex(),
    }
}

fn selected_canon_inputs(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    limits: CorpusIndexProjectionLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_compiler::Result<(BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>)> {
    let mut source_files = BTreeMap::new();
    let mut bytes = 0u64;
    let mut count = 0u64;
    for member in cut.current().members() {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(tos_compiler::Error::Budget(
                "native corpus canon input deadline",
            ));
        }
        let path = member.path.as_str();
        let selected = path.starts_with("ToS/canon/")
            && (path.ends_with("/node.json")
                || path.ends_with("/node.human-forms.json")
                || path.ends_with("/edges.csv"))
            || path.starts_with("ToS/candidate-intake/") && path.ends_with("/edges.csv");
        if !selected {
            continue;
        }
        count = count
            .checked_add(1)
            .filter(|value| *value <= limits.canon.max_selected_members)
            .ok_or(tos_compiler::Error::Budget(
                "native corpus canon selected file count",
            ))?;
        let relative = member.path.clone();
        let raw = cut
            .read_member(
                revision,
                &relative,
                limits.canon.max_raw_row_bytes as u64,
                deadline,
                cancelled,
            )
            .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
            .raw;
        bytes = bytes
            .checked_add(raw.len() as u64)
            .filter(|value| *value <= limits.max_canon_input_bytes)
            .ok_or(tos_compiler::Error::Budget(
                "native corpus canon source bytes",
            ))?;
        if source_files.insert(path.to_owned(), raw).is_some() {
            return Err(tos_compiler::Error::Invalid(
                "duplicate native corpus canon source",
            ));
        }
    }
    if source_files.is_empty() {
        return Err(tos_compiler::Error::Invalid(
            "native corpus canon sources absent",
        ));
    }
    let mut contracts = BTreeMap::new();
    for path in [CANON_NODE_CONTRACT, CANON_FORMS_CONTRACT] {
        let relative = RelativePath::parse(path)
            .map_err(|_| tos_compiler::Error::Invalid("native corpus contract path"))?;
        let raw = cut
            .read_member(
                revision,
                &relative,
                limits.canon.max_raw_row_bytes as u64,
                deadline,
                cancelled,
            )
            .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
            .raw;
        bytes = bytes
            .checked_add(raw.len() as u64)
            .filter(|value| *value <= limits.max_canon_input_bytes)
            .ok_or(tos_compiler::Error::Budget(
                "native corpus canon contract bytes",
            ))?;
        contracts.insert(path.to_owned(), raw);
    }
    Ok((source_files, contracts))
}

fn ingest_rows(
    stage: &mut KnowledgeStage<'_>,
    source_graph: &str,
    collection: &str,
    rows: &BTreeMap<String, Vec<u8>>,
    limits: StageLimits,
) -> tos_compiler::Result<()> {
    let mut entries = rows.iter();
    loop {
        let borrowed = entries
            .by_ref()
            .take(limits.max_seek_rows)
            .map(|(id, raw)| InputRow {
                source_graph,
                collection,
                id,
                payload: raw,
            })
            .collect::<Vec<_>>();
        if borrowed.is_empty() {
            return Ok(());
        }
        stage.ingest_input_batch(&borrowed)?;
    }
}

fn ingest_catalog_rows(
    source: &mut KnowledgeStage<'_>,
    target: &mut KnowledgeStage<'_>,
    receipt: &[InputCollectionReceipt],
    limits: StageLimits,
) -> tos_compiler::Result<()> {
    for input in receipt {
        let mut after = None;
        loop {
            let page = source.scan_input(
                &input.source_graph,
                &input.collection,
                after.as_deref(),
                limits.max_seek_rows,
            )?;
            if !page.rows.is_empty() {
                let borrowed = page
                    .rows
                    .iter()
                    .map(|row| InputRow {
                        source_graph: &input.source_graph,
                        collection: &input.collection,
                        id: &row.id,
                        payload: &row.payload,
                    })
                    .collect::<Vec<_>>();
                target.ingest_input_batch(&borrowed)?;
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
    }
    Ok(())
}

fn same_catalog_source(left: &InputCollectionReceipt, right: &InputCollectionReceipt) -> bool {
    left.source_graph == right.source_graph
        && left.collection == right.collection
        && left.input_role == right.input_role
        && left.adapter_profile == right.adapter_profile
        && left.expected_count == right.expected_count
        && left.expected_root_sha256 == right.expected_root_sha256
}

fn validate_product(value: &serde_json::Value) -> tos_compiler::Result<()> {
    let counts = value
        .get("counts")
        .ok_or(tos_compiler::Error::Invalid("native corpus counts"))?;
    for (field, minimum) in [
        ("branches", 10),
        ("nodes", 1),
        ("relation_packs", 1),
        ("resources", 1),
    ] {
        if counts
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            < minimum
        {
            return Err(tos_compiler::Error::Invalid(
                "native corpus selected source coverage",
            ));
        }
    }
    if value["runtime_projection_boundary"]["runtime_owner"] != "abyss-stack" {
        return Err(tos_compiler::Error::Invalid(
            "native corpus runtime projection boundary",
        ));
    }
    let declared = value["authority_order"]
        .as_array()
        .ok_or(tos_compiler::Error::Invalid(
            "native corpus authority order",
        ))?
        .iter()
        .filter_map(|entry| entry.get("layer").and_then(serde_json::Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    let mut emitted = std::collections::BTreeSet::new();
    for field in [
        "branches",
        "manifests",
        "nodes",
        "relation_packs",
        "relation_edges",
        "resources",
    ] {
        if let Some(rows) = value[field].as_array() {
            for row in rows {
                if let Some(layer) = row
                    .get("authority_layer")
                    .and_then(serde_json::Value::as_str)
                {
                    emitted.insert(layer);
                }
            }
        }
    }
    if !emitted.is_subset(&declared) {
        return Err(tos_compiler::Error::Invalid(
            "native corpus authority layer declaration",
        ));
    }
    if value["diagnostics"].as_array().is_none_or(|rows| {
        rows.iter()
            .any(|row| row.get("level").and_then(serde_json::Value::as_str) == Some("error"))
    }) {
        return Err(tos_compiler::Error::Invalid(
            "native corpus error diagnostics",
        ));
    }
    Ok(())
}

/// Build the whole ToS corpus-index and bibliographic-claims candidates from
/// one selected Rust-owned source profile. The returned bytes are candidates
/// only; the caller owns `--check`/write routing and final source/invocation
/// fences.
#[allow(clippy::too_many_arguments)]
pub(crate) fn project(
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    binding: &SourceBinding,
    root: RepositoryRootInput<'_>,
    worker: ExactWorkerIdentity,
    limits: CorpusIndexProjectionLimits,
    isolation: &dyn StageIsolation,
    stage_root: &Path,
    recheck_selected_source: &dyn Fn() -> tos_compiler::Result<()>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_compiler::Result<NativeCorpusIndexProducts> {
    if Instant::now() >= deadline
        || binding.source_cut != root.source_cut
        || binding.index_generation != cut.current().revision().0.to_hex()
        || binding.membership_root
            != cut
                .stream(cut.current().revision())
                .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
                .expectation()
                .digest
                .to_hex()
        || limits.max_canon_input_bytes == 0
        || limits.max_output_bytes == 0
        || limits.max_work_bytes == 0
        || limits.max_work_bytes > 320 * 1024 * 1024
        || limits.stage.max_seek_rows == 0
    {
        return Err(tos_compiler::Error::Invalid(
            "native corpus selected profile/budget",
        ));
    }
    recheck_selected_source()?;
    let revision = cut.current().revision();
    let membership = cut
        .stream(revision)
        .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
        .expectation();
    let owner = BoundSourceStageOwner {
        binding,
        cut,
        revision,
        membership,
        recheck: recheck_selected_source,
    };

    let mut bibliographic = limits.bibliographic;
    bibliographic.deadline = deadline;
    let catalog_plan = plan_source_catalog_inputs(
        cut,
        revision,
        membership,
        binding,
        limits.catalog_input,
        bibliographic,
        cancelled,
    )?;
    let stage_catalog = stage_root.join("tos-corpus-index-catalog.sqlite");
    let stage_canon = stage_root.join("tos-corpus-index-canon.sqlite");
    let stage_target = stage_root.join("tos-corpus-index-target.sqlite");
    for path in [&stage_catalog, &stage_canon, &stage_target] {
        if !path.is_absolute() || path.exists() {
            return Err(tos_compiler::Error::Invalid(
                "native corpus private stage path",
            ));
        }
    }
    let mut catalog_stage = KnowledgeStage::create(
        &stage_catalog,
        limits.stage,
        catalog_plan.input_receipt(),
        &owner,
        isolation,
    )?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(tos_compiler::Error::Budget(
            "native corpus schema preparation deadline",
        ));
    }
    let worker_budget = ExecutorBudget {
        execution_wall: remaining,
        ..limits.schema_worker
    };
    let mut schema_work = limits.schema_work;
    schema_work.total_execution_wall = remaining;
    let validator = SourceCatalogValidator::from_cut(
        cut,
        &worker,
        worker_budget,
        limits.schema_worker_limits,
        schema_work,
        deadline,
        cancelled,
    )?;
    let catalog_candidate = render_source_bibliographic_plan(
        &catalog_plan,
        cut,
        revision,
        membership,
        &mut catalog_stage,
        &validator,
        &mut crate::source_forms_compiler::NativeBibliographicForms,
        bibliographic,
        limits.catalog_input.max_selected_members,
        limits.catalog_input.max_work_bytes.min(64 * 1024 * 1024) as usize,
    )?;
    let entity_path = RelativePath::parse(ENTITY_TYPES_PATH)
        .map_err(|_| tos_compiler::Error::Invalid("native corpus entity types path"))?;
    let entities_raw = cut
        .read_member(
            revision,
            &entity_path,
            limits.bibliographic.catalog.max_file_bytes as u64,
            deadline,
            cancelled,
        )
        .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
        .raw;
    let entities: serde_json::Value = serde_json::from_slice(&entities_raw)
        .map_err(|_| tos_compiler::Error::Invalid("native corpus entity types JSON"))?;
    let mut forms = crate::source_forms_compiler::NativeBibliographicForms;
    let source = BibliographicSourceCut {
        cut,
        expected_revision: revision,
        expected_membership: membership,
        stage_source_cut: &binding.source_cut,
        max_read_files: limits.catalog_input.max_selected_members,
        max_read_bytes: limits.catalog_input.max_work_bytes.min(64 * 1024 * 1024) as usize,
    };
    let navigation = project_source_navigation_from_cut(
        &mut catalog_stage,
        &catalog_candidate.catalog,
        &source,
        &validator,
        &entities,
        &mut forms,
        bibliographic,
    )?;

    let (canon_files, canon_contracts) =
        selected_canon_inputs(cut, revision, limits, deadline, cancelled)?;
    let canon_inputs = vec![
        raw_receipt(
            source_canon_source::CANON_SOURCE_CUSTODY,
            "source-files",
            source_canon_source::CANON_SOURCE_FILES_ROLE,
            source_canon_source::CANON_SOURCE_FILES_PROFILE,
            &canon_files,
        ),
        raw_receipt(
            source_canon_source::CANON_SOURCE_CUSTODY,
            "contracts",
            source_canon_source::CANON_SOURCE_CONTRACTS_ROLE,
            source_canon_source::CANON_SOURCE_CONTRACTS_PROFILE,
            &canon_contracts,
        ),
    ];
    let mut canon_stage = KnowledgeStage::create(
        &stage_canon,
        limits.stage,
        ExactInputReceipt {
            binding: binding.clone(),
            collections: canon_inputs,
        },
        &owner,
        isolation,
    )?;
    ingest_rows(
        &mut canon_stage,
        source_canon_source::CANON_SOURCE_CUSTODY,
        "source-files",
        &canon_files,
        limits.stage,
    )?;
    ingest_rows(
        &mut canon_stage,
        source_canon_source::CANON_SOURCE_CUSTODY,
        "contracts",
        &canon_contracts,
        limits.stage,
    )?;
    let vocabulary_path = RelativePath::parse(VOCABULARY_PATH)
        .map_err(|_| tos_compiler::Error::Invalid("native corpus vocabulary path"))?;
    let vocabulary_selection = software
        .select_components(std::slice::from_ref(&vocabulary_path))
        .map_err(|error| tos_compiler::Error::Source(error.to_string()))?;
    let vocabulary_raw = software
        .read_selected_component(
            &vocabulary_selection,
            &vocabulary_path,
            1_048_576,
            deadline,
            cancelled,
        )
        .map_err(|error| tos_compiler::Error::Source(error.to_string()))?;
    let vocabulary = QueryVocabulary::parse(
        &vocabulary_raw,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
    )?;
    let (repository_plan, canon_plan) =
        tos_compiler::source_corpus::plan_native_corpus_source_families(
            &mut canon_stage,
            cut,
            software,
            binding,
            &vocabulary,
            root,
            &validator,
            limits.repository,
            limits.canon,
            deadline,
            cancelled,
            crate::source_forms_compiler::materialize_compiler_forms,
        )?;
    let mut collections = repository_plan.receipt().collections.clone();
    collections.extend(canon_plan.receipt().collections.iter().map(|collection| {
        InputCollectionReceipt {
            source_graph: collection.source_graph.clone(),
            collection: collection.collection.clone(),
            input_role: collection.input_role.clone(),
            adapter_profile: collection.adapter_profile.clone(),
            expected_count: collection.count,
            expected_root_sha256: collection.root_sha256.clone(),
        }
    }));
    let catalog_collections = catalog_plan.input_receipt().collections;
    collections.extend(catalog_collections.iter().cloned());
    let target_receipt = ExactInputReceipt {
        binding: binding.clone(),
        collections,
    };
    let mut target = KnowledgeStage::create(
        &stage_target,
        limits.stage,
        target_receipt,
        &owner,
        isolation,
    )?;
    render_repository_source_plan(
        &mut target,
        &repository_plan,
        &vocabulary,
        deadline,
        cancelled,
    )?;
    render_canon_source_plan(
        &mut canon_stage,
        &canon_plan,
        cut,
        revision,
        membership,
        &mut target,
        limits.canon,
        deadline,
        cancelled,
    )?;
    let catalog_receipt_for_transfer = catalog_plan.input_receipt().collections;
    if catalog_receipt_for_transfer.len() != catalog_collections.len()
        || catalog_receipt_for_transfer
            .iter()
            .zip(&catalog_collections)
            .any(|(selected, receipt)| !same_catalog_source(selected, receipt))
    {
        return Err(tos_compiler::Error::Invalid(
            "native corpus catalog transfer receipt",
        ));
    }
    ingest_catalog_rows(
        &mut catalog_stage,
        &mut target,
        &catalog_receipt_for_transfer,
        limits.stage,
    )?;
    let (bibliographic_claims, bibliographic_receipt) = bibliographic_projection(
        cut,
        &catalog_candidate,
        &mut catalog_stage,
        &validator,
        bibliographic,
        limits.max_output_bytes,
        limits.catalog_input.max_manifest_members,
    )?;
    let mut originals = limits.originals;
    originals.max_total_bytes = originals.max_total_bytes.min(limits.max_output_bytes);
    let mut native_limits = NativeCorpusLimits {
        originals,
        max_work_bytes: limits.max_work_bytes,
        schema_work,
    };
    native_limits.schema_work.total_execution_wall =
        deadline.saturating_duration_since(Instant::now());
    let projection = tos_compiler::source_corpus::project_native_corpus_from_sources(
        &mut target,
        &vocabulary,
        cut,
        software,
        &repository_plan,
        &canon_plan,
        &navigation,
        &validator,
        native_limits,
        deadline,
        cancelled,
    )?;
    if projection.output_bytes().len() as u64 > limits.max_output_bytes {
        return Err(tos_compiler::Error::Budget(
            "native corpus output byte budget",
        ));
    }
    validate_product(projection.value())?;
    let combined_output_bytes = (projection.output_bytes().len() as u64)
        .checked_add(bibliographic_receipt.output_bytes)
        .ok_or(tos_compiler::Error::Budget(
            "native corpus companion output byte overflow",
        ))?;
    if combined_output_bytes > limits.max_output_bytes {
        return Err(tos_compiler::Error::Budget(
            "native corpus companion output byte budget",
        ));
    }
    recheck_selected_source()?;
    Ok(NativeCorpusIndexProducts {
        corpus: projection,
        bibliographic_claims,
        bibliographic_receipt,
    })
}

#[cfg(test)]
mod tests {
    use super::validate_product;
    use serde_json::json;

    fn valid_product() -> serde_json::Value {
        json!({
            "counts":{"branches":10,"nodes":1,"relation_packs":1,"resources":1},
            "runtime_projection_boundary":{"runtime_owner":"abyss-stack"},
            "authority_order":[{"layer":"source_home"}],
            "branches":[{"authority_layer":"source_home"}],
            "manifests":[],
            "nodes":[],
            "relation_packs":[],
            "relation_edges":[],
            "resources":[],
            "diagnostics":[]
        })
    }

    #[test]
    fn selected_corpus_validation_rejects_errors_outside_projection_header() {
        let mut product = valid_product();
        assert!(validate_product(&product).is_ok());

        product["diagnostics"] = json!([{
            "level":"error",
            "path":"ToS/canon/example/node.json",
            "message":"unresolved reference"
        }]);
        assert!(validate_product(&product).is_err());

        let mut missing = valid_product();
        missing.as_object_mut().unwrap().remove("diagnostics");
        assert!(validate_product(&missing).is_err());
    }
}
