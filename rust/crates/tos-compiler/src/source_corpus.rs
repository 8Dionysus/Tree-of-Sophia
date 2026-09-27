//! Whole native corpus composition from real maintained source-family plans.
//! This derived projection grants neither source admission nor current rights.
use crate::knowledge_canon_source::CanonSourcePlan;
use crate::knowledge_repository_source::RepositorySourcePlan;
use crate::knowledge_stage::{InputCollectionReceipt, KnowledgeStage};
use crate::source_navigation_source::NavigationSourceProjection;
use crate::source_witness_catalog::SourceCatalogValidator;
use crate::{Error, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader};
use tos_validation::executor::{BatchBudget, BatchStreamBudget};
use tos_validation::source_cut::{CutSchemaCheck, CutSchemaExecutor};

#[derive(Clone, Copy)]
pub struct NativeCorpusLimits {
    pub originals: crate::NavigationOriginalLimits,
    pub max_work_bytes: u64,
    /// Must come from the same owner declaration used to create the validator.
    pub schema_work: BatchStreamBudget,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCorpusSourceReceipt {
    pub profile: String,
    pub source_revision: String,
    pub source_membership_sha256: String,
    pub source_members: u64,
    pub source_cut: String,
    pub descriptor_sha256: String,
    pub software_commit: String,
    pub software_tree: String,
    pub software_manifest_sha256: String,
    pub owner_program_sha256: String,
    pub owner_schema_sha256: String,
    pub repository_inventory_root_sha256: String,
    pub canon_source_root_sha256: String,
    pub navigation_catalog_root_sha256: String,
    pub output_sha256: String,
    pub output_bytes: u64,
    pub schema_units: u64,
}
pub(crate) fn validate_receipt(r: &NativeCorpusSourceReceipt) -> Result<()> {
    if r.profile != "tos_native_corpus_source_v1"
        || r.source_cut.is_empty()
        || r.source_cut.len() > 4096
        || r.source_members == 0
        || r.schema_units == 0
        || r.output_bytes == 0
        || r.output_bytes > crate::knowledge_original_rows::MAX_TOTAL_BYTES
    {
        return Err(Error::Invalid(
            "native corpus producer receipt profile/limits",
        ));
    }
    for sha in [
        &r.source_revision,
        &r.source_membership_sha256,
        &r.descriptor_sha256,
        &r.software_manifest_sha256,
        &r.owner_program_sha256,
        &r.owner_schema_sha256,
        &r.repository_inventory_root_sha256,
        &r.canon_source_root_sha256,
        &r.navigation_catalog_root_sha256,
        &r.output_sha256,
    ] {
        Digest256::from_hex(sha)
            .map_err(|_| Error::Invalid("native corpus producer receipt SHA"))?;
    }
    for git in [&r.software_commit, &r.software_tree] {
        if ![40, 64].contains(&git.len())
            || !git
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid("native corpus software identity"));
        }
    }
    Ok(())
}
/// Private output, computed only after complete family roots and schema checks.
pub struct NativeCorpusProjection {
    value: Value,
    receipt: NativeCorpusSourceReceipt,
    output_bytes: Vec<u8>,
    binding: crate::SourceBinding,
}
/// Plan the repository and canon/candidate fronts in the same schema operation
/// as the catalogue and navigation phases. The planner retains independently
/// declared current raw-file custody; no second worker or schema engine is used.
pub fn plan_native_corpus_source_families<F>(
    canon_planner: &mut KnowledgeStage<'_>,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    binding: &crate::SourceBinding,
    vocabulary: &crate::QueryVocabulary,
    root: crate::RepositoryRootInput<'_>,
    validator: &SourceCatalogValidator<'_>,
    repository_limits: crate::knowledge_repository_source::RepositorySourceLimits,
    canon_limits: crate::knowledge_canon_source::CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    materialize_forms: F,
) -> Result<(RepositorySourcePlan, CanonSourcePlan)>
where
    F: FnMut(&Value, &Value, usize) -> Result<Vec<Value>>,
{
    let result = (|| {
        binding.validate()?;
        let revision = cut.current().revision();
        let membership = cut
            .stream(revision)
            .map_err(|e| Error::Source(e.to_string()))?
            .expectation();
        if canon_planner.exact_receipt().binding.source_cut != binding.source_cut
            || canon_planner.exact_receipt().binding.membership_root != membership.digest.to_hex()
            || binding.membership_root != membership.digest.to_hex()
            || root.source_cut != binding.source_cut
        {
            return Err(Error::Invalid(
                "native corpus source-family operation binding",
            ));
        }
        let mut schemas = validator.schemas(revision)?;
        let repository = crate::knowledge_repository_source::plan_repository_source_inputs(
            cut,
            revision,
            membership,
            software,
            software.selection(),
            &binding.source_cut,
            vocabulary,
            root,
            &mut *schemas,
            repository_limits,
            deadline,
            cancelled,
        )?;
        let canon = crate::knowledge_canon_source::plan_canon_source_inputs(
            canon_planner,
            cut,
            revision,
            membership,
            vocabulary,
            &mut *schemas,
            canon_limits,
            deadline,
            cancelled,
            materialize_forms,
        )?;
        Ok((repository, canon))
    })();
    if result.is_err() {
        canon_planner.poison();
    }
    result
}
impl NativeCorpusProjection {
    pub(crate) fn source_binding(&self) -> &crate::SourceBinding {
        &self.binding
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn receipt(&self) -> &NativeCorpusSourceReceipt {
        &self.receipt
    }
    pub fn output_bytes(&self) -> &[u8] {
        &self.output_bytes
    }
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("native corpus cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("native corpus deadline"));
    }
    Ok(())
}
fn charge(work: &mut u64, bytes: usize, l: NativeCorpusLimits) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .filter(|n| *n <= l.max_work_bytes)
        .ok_or(Error::Budget("native corpus aggregate work"))?;
    Ok(())
}
fn read_collection(
    stage: &KnowledgeStage<'_>,
    input: &InputCollectionReceipt,
    vocabulary: &crate::QueryVocabulary,
    l: NativeCorpusLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
) -> Result<Vec<Value>> {
    if !vocabulary.sources.iter().any(|s| {
        s.source_graph_id == input.source_graph
            && s.input_role == input.input_role
            && s.adapter_profile == input.adapter_profile
    }) {
        return Err(Error::Invalid(
            "native corpus exact registered source profile",
        ));
    }
    let declarations = stage
        .exact_receipt()
        .collections
        .iter()
        .filter(|c| c.source_graph == input.source_graph && c.collection == input.collection)
        .collect::<Vec<_>>();
    if declarations.len() != 1
        || serde_json::to_vec(declarations[0])
            .map_err(|_| Error::Invalid("native corpus declaration"))?
            != serde_json::to_vec(input)
                .map_err(|_| Error::Invalid("native corpus producer input"))?
    {
        return Err(Error::Invalid(
            "native corpus complete declared producer collection",
        ));
    }
    let mut root = Digest256Hasher::new();
    let mut after = None;
    let mut values = Vec::new();
    loop {
        check(deadline, cancelled)?;
        let page = stage.scan_input(&input.source_graph, &input.collection, after.as_deref(), 1)?;
        for row in page.rows {
            charge(work, row.payload.len(), l)?;
            if row.payload.len() > l.originals.max_row_bytes
                || values.len() as u64 >= l.originals.max_rows
            {
                return Err(Error::Budget("native corpus source row"));
            }
            root.update(&(row.id.len() as u64).to_be_bytes());
            root.update(row.id.as_bytes());
            root.update(Digest256::of_bytes(&row.payload).as_bytes());
            let limits =
                tos_foundation::JsonLimits::new(l.originals.max_row_bytes, 96, 1_000_000, 4096)
                    .map_err(|_| Error::Budget("native corpus source JSON"))?;
            tos_foundation::parse_json(
                &row.payload,
                tos_foundation::JsonMode::PublishedStrict,
                limits,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
            values.push(
                serde_json::from_slice(&row.payload)
                    .map_err(|_| Error::Invalid("native corpus source row JSON"))?,
            );
        }
        after = page.next_id;
        if after.is_none() {
            break;
        }
    }
    if values.len() as u64 != input.expected_count
        || root.finalize().to_hex() != input.expected_root_sha256
    {
        return Err(Error::Invalid("native corpus family collection coverage"));
    }
    Ok(values)
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("native corpus source order field"))
}
// The exact owner schema is pinned before using this decomposition. Growing
// top-level and nested navigation arrays have only type/items assertions;
// all other fields and the complete root/object closure stay in the header.
fn visit_instances(
    value: &Value,
    mut visit: impl FnMut(&Value, String, String) -> Result<()>,
) -> Result<()> {
    const CONTRACT: &str = "ToS/contracts/tos-corpus-index.schema.json";
    let growing = [
        "branches",
        "manifests",
        "nodes",
        "relation_packs",
        "relation_edges",
        "resources",
        "diagnostics",
    ];
    let mut header = serde_json::Map::new();
    for (key, v) in value
        .as_object()
        .ok_or(Error::Invalid("native corpus object"))?
    {
        header.insert(
            key.clone(),
            if growing.contains(&key.as_str()) {
                Value::Array(Vec::new())
            } else if key == "source_navigation" {
                let mut nav = serde_json::Map::new();
                for (k, v) in v
                    .as_object()
                    .ok_or(Error::Invalid("native corpus navigation header"))?
                {
                    nav.insert(
                        k.clone(),
                        if ["nodes", "edges", "rights"].contains(&k.as_str()) {
                            Value::Array(Vec::new())
                        } else {
                            v.clone()
                        },
                    );
                }
                Value::Object(nav)
            } else {
                v.clone()
            },
        );
    }
    visit(
        &Value::Object(header),
        "corpus:header".into(),
        CONTRACT.into(),
    )?;
    for field in growing {
        let rows = value[field]
            .as_array()
            .ok_or(Error::Invalid("native corpus growing array"))?;
        for (ordinal, row) in rows.iter().enumerate() {
            visit(
                row,
                format!("corpus:{field}/{ordinal}"),
                format!("{CONTRACT}#/properties/{field}/items"),
            )?;
        }
    }
    for field in ["nodes", "edges", "rights"] {
        let rows = value["source_navigation"][field]
            .as_array()
            .ok_or(Error::Invalid("native corpus navigation array"))?;
        for (ordinal, row) in rows.iter().enumerate() {
            visit(
                row,
                format!("corpus:source_navigation/{field}/{ordinal}"),
                format!("{CONTRACT}#/$defs/sourceNavigation/properties/{field}/items"),
            )?;
        }
    }
    Ok(())
}
fn execute_frame(
    checks: &mut Vec<CutSchemaCheck>,
    schemas: &mut impl CutSchemaExecutor,
    batch: BatchBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    executed: &mut u64,
) -> Result<()> {
    if checks.is_empty() {
        return Ok(());
    }
    let valid = schemas
        .check_batch(checks, batch, deadline, cancelled)
        .map_err(|e| Error::Source(format!("native corpus schema batch:{e:?}")))?;
    if valid.len() != checks.len() || valid.iter().any(|v| !*v) {
        return Err(Error::Invalid("native corpus complete schema outcomes"));
    }
    *executed = executed
        .checked_add(valid.len() as u64)
        .ok_or(Error::Budget("native corpus schema coverage"))?;
    checks.clear();
    Ok(())
}
fn validate_projection(
    value: &Value,
    schemas: &mut impl CutSchemaExecutor,
    l: NativeCorpusLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
) -> Result<u64> {
    let b = l.schema_work;
    let batch = b.batch;
    if batch.max_units == 0
        || batch.max_units > BatchBudget::MAX_UNITS
        || batch.max_total_raw_bytes == 0
        || batch.max_total_raw_bytes > BatchBudget::MAX_RAW_BYTES
    {
        return Err(Error::Budget("native corpus schema batch limits"));
    }
    let cap = tos_validation::SchemaBackendProbe::MAX_INSTANCE_BYTES.min(l.originals.max_row_bytes);
    let mut units = 0u64;
    let mut chunks = 0u64;
    let mut decoded = 0u64;
    let mut wire = 0u64;
    let mut receipt_bytes = 0u64;
    let mut remaining = None;
    let mut selectors = BTreeSet::new();
    let mut count = 0usize;
    let mut pending = 0u64;
    visit_instances(value, |instance, path, contract| {
        check(deadline, cancelled)?;
        let raw = crate::knowledge_corpus_source::encode(instance, cap)?;
        charge(work, raw.len(), l)?;
        let mut cost = schemas
            .schema_input_cost(&path, &raw, &contract, count as u64)
            .map_err(|e| Error::Source(format!("native corpus schema preflight:{e:?}")))?;
        if cost.decoded_instance_bytes > batch.max_total_raw_bytes as u64 {
            return Err(Error::Budget("native corpus decoded instance"));
        }
        if count == batch.max_units
            || pending
                .checked_add(cost.decoded_instance_bytes)
                .is_none_or(|n| n > batch.max_total_raw_bytes as u64)
        {
            count = 0;
            pending = 0;
            cost = schemas
                .schema_input_cost(&path, &raw, &contract, 0)
                .map_err(|e| Error::Source(format!("native corpus schema preflight:{e:?}")))?;
        }
        if count == 0 {
            chunks = chunks
                .checked_add(1)
                .filter(|n| *n <= b.max_chunks)
                .ok_or(Error::Budget("native corpus schema chunks"))?;
            wire = wire
                .checked_add(cost.frame_wire_bytes)
                .ok_or(Error::Budget("native corpus wire"))?;
        }
        if units == 0 {
            wire = wire
                .checked_add(cost.operation_wire_bytes)
                .ok_or(Error::Budget("native corpus initial wire"))?;
            remaining = Some((cost.remaining_receipts, cost.remaining_receipt_bytes));
        }
        units = units
            .checked_add(1)
            .filter(|n| *n <= b.max_total_units)
            .ok_or(Error::Budget("native corpus schema units"))?;
        decoded = decoded
            .checked_add(cost.decoded_instance_bytes)
            .filter(|n| *n <= b.max_total_raw_bytes)
            .ok_or(Error::Budget("native corpus schema raw bytes"))?;
        wire = wire
            .checked_add(cost.unit_wire_bytes)
            .filter(|n| *n <= b.max_total_wire_bytes)
            .ok_or(Error::Budget("native corpus schema wire bytes"))?;
        receipt_bytes = receipt_bytes
            .checked_add(cost.receipt_bytes)
            .ok_or(Error::Budget("native corpus receipt bytes"))?;
        selectors.insert(cost.selector);
        if selectors.len() > b.max_distinct_selectors {
            return Err(Error::Budget("native corpus schema selectors"));
        }
        count += 1;
        pending += cost.decoded_instance_bytes;
        Ok(())
    })?;
    let (available, bytes) =
        remaining.ok_or(Error::Invalid("native corpus schema header absent"))?;
    if units > available || receipt_bytes > bytes {
        return Err(Error::Budget("native corpus schema receipt capacity"));
    }
    // Only one bounded frame is retained. The same traversal and decoded
    // packing law are used after complete projection feasibility is known.
    let mut checks = Vec::with_capacity(batch.max_units);
    let mut pending = 0u64;
    let mut executed = 0u64;
    visit_instances(value, |instance, path, contract| {
        check(deadline, cancelled)?;
        let raw = crate::knowledge_corpus_source::encode(instance, cap)?;
        charge(work, raw.len(), l)?;
        let cost = schemas
            .schema_input_cost(&path, &raw, &contract, checks.len() as u64)
            .map_err(|e| Error::Source(format!("native corpus schema cost:{e:?}")))?;
        if checks.len() == batch.max_units
            || pending
                .checked_add(cost.decoded_instance_bytes)
                .is_none_or(|n| n > batch.max_total_raw_bytes as u64)
        {
            execute_frame(
                &mut checks,
                schemas,
                batch,
                deadline,
                cancelled,
                &mut executed,
            )?;
            pending = 0;
        }
        pending = pending
            .checked_add(cost.decoded_instance_bytes)
            .ok_or(Error::Budget("native corpus frame bytes"))?;
        checks.push(CutSchemaCheck {
            path,
            raw,
            contract,
        });
        Ok(())
    })?;
    execute_frame(
        &mut checks,
        schemas,
        batch,
        deadline,
        cancelled,
        &mut executed,
    )?;
    if executed != units {
        return Err(Error::Invalid("native corpus planned executed coverage"));
    }
    Ok(units)
}
/// Compose the exact maintained native families. Strict source-front refusals
/// remain refusals; a missing family never becomes an empty successful corpus.
pub fn project_native_corpus_from_sources(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &crate::QueryVocabulary,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    repository: &RepositorySourcePlan,
    canon: &CanonSourcePlan,
    navigation: &NavigationSourceProjection,
    validator: &SourceCatalogValidator<'_>,
    l: NativeCorpusLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<NativeCorpusProjection> {
    let result = (|| {
        l.originals.validate()?;
        check(deadline, cancelled)?;
        if l.max_work_bytes == 0 || l.max_work_bytes > crate::knowledge_original_rows::MAX_COLD_WORK
        {
            return Err(Error::Budget("native corpus work declaration"));
        }
        let r = repository.receipt();
        let c = canon.receipt();
        let current = cut.current().revision();
        let membership = cut
            .stream(current)
            .map_err(|e| Error::Source(e.to_string()))?
            .expectation();
        if r.source_revision != current.0.to_hex()
            || c.source_revision != r.source_revision
            || r.source_membership != membership
            || c.manifest_members != membership.count
            || c.manifest_membership_root_sha256 != membership.digest.to_hex()
            || !c.current_members_only
            || c.final_graph_rows_written
            || c.job_source_cut != stage.exact_receipt().binding.source_cut
            || membership.digest.to_hex() != stage.exact_receipt().binding.membership_root
            || repository.root_input().source_cut != c.job_source_cut
            || software.selection() != &r.inventory_selection
            || navigation.source_binding["source_revision"] != c.source_revision
            || navigation.source_binding["membership_count"] != membership.count
            || navigation.source_binding["membership_sha256"] != membership.digest.to_hex()
            || navigation.source_binding["stage_source_cut"] != c.job_source_cut
        {
            return Err(Error::Invalid(
                "native corpus same source families and selected software",
            ));
        }
        let path = RelativePath::parse(OWNER_PROGRAM_PATH)
            .map_err(|_| Error::Invalid("native corpus owner path"))?;
        let selection = software
            .select_components(&[path.clone()])
            .map_err(|e| Error::Source(e.to_string()))?;
        let program = software
            .read_selected_component(&selection, &path, 2 * 1024 * 1024, deadline, cancelled)
            .map_err(|e| Error::Source(e.to_string()))?;
        if program.as_slice() != OWNER_PROGRAM {
            return Err(Error::Invalid(
                "unsupported native corpus software contract",
            ));
        }
        let mut work = 0;
        charge(&mut work, program.len(), l)?;
        let mut payload: Value = serde_json::from_str(OWNER_HEADER)
            .map_err(|_| Error::Invalid("compiled corpus header law"))?;
        let mut order = BTreeMap::<(String, String), u64>::new();
        for input in &r.collections {
            let values =
                read_collection(stage, input, vocabulary, l, deadline, cancelled, &mut work)?;
            if input.collection == "source_order" {
                for value in values {
                    let key = (
                        text(&value, "collection")?.to_owned(),
                        text(&value, "id")?.to_owned(),
                    );
                    let ordinal = value["ordinal"]
                        .as_u64()
                        .ok_or(Error::Invalid("native corpus repository ordinal"))?;
                    if order.insert(key, ordinal).is_some() {
                        return Err(Error::Invalid("native corpus duplicate source order"));
                    }
                }
            } else if ["branches", "manifests", "resources"].contains(&input.collection.as_str()) {
                payload[&input.collection] = Value::Array(values);
            } else {
                return Err(Error::Invalid("native corpus repository collection"));
            }
        }
        let branches = payload["branches"]
            .as_array_mut()
            .ok_or(Error::Invalid("native corpus branches"))?;
        let mut ordinals = BTreeSet::new();
        for branch in branches.iter() {
            let ordinal = *order
                .get(&("branches".into(), text(branch, "id")?.into()))
                .ok_or(Error::Invalid("native corpus branch original order"))?;
            if !ordinals.insert(ordinal) {
                return Err(Error::Invalid("native corpus branch ordinal collision"));
            }
        }
        if ordinals.iter().copied().ne(0..branches.len() as u64) {
            return Err(Error::Invalid("native corpus branch ordinal coverage"));
        }
        branches.sort_by_key(|b| {
            order[&("branches".into(), b["id"].as_str().expect("checked").into())]
        });
        for name in ["manifests", "resources"] {
            let rows = payload[name]
                .as_array_mut()
                .ok_or(Error::Invalid("native corpus repository array"))?;
            for row in rows.iter() {
                text(row, "path")?;
            }
            rows.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        }
        for name in ["nodes", "relation_packs", "relation_edges"] {
            payload[name] = Value::Array(Vec::new());
        }
        for collection in &c.collections {
            let input = InputCollectionReceipt {
                source_graph: collection.source_graph.clone(),
                collection: collection.collection.clone(),
                input_role: collection.input_role.clone(),
                adapter_profile: collection.adapter_profile.clone(),
                expected_count: collection.count,
                expected_root_sha256: collection.root_sha256.clone(),
            };
            let rows =
                read_collection(stage, &input, vocabulary, l, deadline, cancelled, &mut work)?;
            payload[&collection.collection]
                .as_array_mut()
                .ok_or(Error::Invalid("native corpus canon collection"))?
                .extend(rows);
        }
        for (name, fields) in [
            ("nodes", vec!["source_path"]),
            ("relation_packs", vec!["path"]),
            ("relation_edges", vec!["pack_id", "edge_id"]),
        ] {
            let rows = payload[name].as_array_mut().expect("created");
            for row in rows.iter() {
                for field in &fields {
                    text(row, field)?;
                }
            }
            rows.sort_by(|a, b| {
                fields
                    .iter()
                    .map(|f| a[*f].as_str().expect("checked"))
                    .cmp(fields.iter().map(|f| b[*f].as_str().expect("checked")))
            });
        }
        payload["source_navigation"] = navigation.value.clone();
        payload["diagnostics"] = Value::Array(navigation.diagnostics.clone());
        let mut counts = serde_json::Map::new();
        let mut total_rows = 1u64;
        let mut total_bytes = 0u64;
        for name in [
            "branches",
            "manifests",
            "nodes",
            "relation_packs",
            "relation_edges",
            "resources",
            "diagnostics",
        ] {
            let rows = payload[name]
                .as_array()
                .ok_or(Error::Invalid("native corpus output array"))?;
            counts.insert(name.into(), Value::from(rows.len() as u64));
            for row in rows {
                check(deadline, cancelled)?;
                let raw = crate::knowledge_corpus_source::encode(row, l.originals.max_row_bytes)?;
                charge(&mut work, raw.len(), l)?;
                total_rows = total_rows
                    .checked_add(1)
                    .filter(|n| *n <= l.originals.max_rows)
                    .ok_or(Error::Budget("native corpus total rows"))?;
                total_bytes = total_bytes
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= l.originals.max_total_bytes)
                    .ok_or(Error::Budget("native corpus total bytes"))?;
            }
        }
        for name in ["nodes", "edges", "rights"] {
            let rows = payload["source_navigation"][name]
                .as_array()
                .ok_or(Error::Invalid("native corpus navigation array"))?;
            if name != "rights" && payload["source_navigation"]["counts"][name] != rows.len() as u64
            {
                return Err(Error::Invalid("native corpus navigation count"));
            }
            if name != "rights" {
                counts.insert(
                    format!("source_navigation_{name}"),
                    Value::from(rows.len() as u64),
                );
            }
            for row in rows {
                check(deadline, cancelled)?;
                let raw = crate::knowledge_corpus_source::encode(row, l.originals.max_row_bytes)?;
                charge(&mut work, raw.len(), l)?;
                total_rows = total_rows
                    .checked_add(1)
                    .filter(|n| *n <= l.originals.max_rows)
                    .ok_or(Error::Budget("native corpus total navigation rows"))?;
                total_bytes = total_bytes
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= l.originals.max_total_bytes)
                    .ok_or(Error::Budget("native corpus navigation bytes"))?;
            }
        }
        for row in payload["graph_views"]
            .as_array()
            .ok_or(Error::Invalid("native corpus graph views"))?
        {
            check(deadline, cancelled)?;
            let raw = crate::knowledge_corpus_source::encode(row, l.originals.max_row_bytes)?;
            charge(&mut work, raw.len(), l)?;
            total_rows = total_rows
                .checked_add(1)
                .filter(|n| *n <= l.originals.max_rows)
                .ok_or(Error::Budget("native corpus total view rows"))?;
            total_bytes = total_bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= l.originals.max_total_bytes)
                .ok_or(Error::Budget("native corpus view bytes"))?;
        }
        payload["counts"] = Value::Object(counts);
        let schema = stage
            .raw_by_id(
                crate::source_witness_catalog::CATALOG_SOURCE,
                crate::source_witness_catalog::CONTRACT_FILES,
                "ToS/contracts/tos-corpus-index.schema.json",
            )?
            .ok_or(Error::Invalid("native corpus current schema custody"))?;
        if schema.payload.as_slice()
            != include_bytes!("../../../../ToS/contracts/tos-corpus-index.schema.json")
        {
            return Err(Error::Invalid(
                "unsupported native corpus schema decomposition",
            ));
        }
        // All output serialization/caps precede the first projection schema
        // frame; no large late value can consume child work before refusing.
        let mut raw = crate::knowledge_corpus_source::encode(
            &payload,
            usize::try_from(l.originals.max_total_bytes)
                .map_err(|_| Error::Budget("native corpus output usize"))?,
        )?;
        if raw.len() as u64 >= l.originals.max_total_bytes {
            return Err(Error::Budget("native corpus rendered newline"));
        }
        raw.push(b'\n');
        charge(&mut work, raw.len(), l)?;
        let mut schemas = validator.schemas(current)?;
        let schema_units =
            validate_projection(&payload, &mut *schemas, l, deadline, cancelled, &mut work)?;
        drop(schemas);
        // This is the last projection schema phase in the shared operation.
        validator.finish()?;
        Ok(NativeCorpusProjection {
            receipt: NativeCorpusSourceReceipt {
                profile: "tos_native_corpus_source_v1".into(),
                source_revision: c.source_revision.clone(),
                source_membership_sha256: membership.digest.to_hex(),
                source_members: membership.count,
                source_cut: c.job_source_cut.clone(),
                descriptor_sha256: vocabulary.descriptor_sha256.clone(),
                software_commit: software.selection().source_git_commit.clone(),
                software_tree: software.selection().source_git_tree.clone(),
                software_manifest_sha256: software.selection().capture_manifest_sha256.to_hex(),
                owner_program_sha256: Digest256::of_bytes(&program).to_hex(),
                owner_schema_sha256: Digest256::of_bytes(&schema.payload).to_hex(),
                repository_inventory_root_sha256: r.inventory_root_sha256.clone(),
                canon_source_root_sha256: c.selected_source_root_sha256.clone(),
                navigation_catalog_root_sha256: navigation.catalog_root_sha256.clone(),
                output_sha256: Digest256::of_bytes(&raw).to_hex(),
                output_bytes: raw.len() as u64,
                schema_units,
            },
            value: payload,
            output_bytes: raw,
            binding: stage.exact_receipt().binding.clone(),
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

const OWNER_PROGRAM_PATH: &str = "scripts/tos_corpus_index_common.py";
const OWNER_PROGRAM: &[u8] = include_bytes!("../../../../scripts/tos_corpus_index_common.py");
// Maintained _build_payload owner constants, bound to OWNER_PROGRAM bytes below.
const OWNER_HEADER: &str = r#"{"authority_order":[{"layer":"source_home","meaning":"Tree of Sophia home surface, source-home manifest, and top-level home route cards","owner_branch":"ToS"},{"layer":"source_witness","meaning":"source-facing witness and provenance surfaces","owner_branch":"ToS/source-witnesses"},{"layer":"golden_route_orientation","meaning":"golden Zarathustra orientation route for the project's current living entry","owner_branch":"ToS/zarathustra"},{"layer":"canon","meaning":"reviewed authored nodes, relation packs, and registries","owner_branch":"ToS/canon"},{"layer":"doctrine","meaning":"current ToS knowledge law, node contracts, templates, and interpretation discipline","owner_branch":"ToS/doctrine"},{"layer":"contract","meaning":"public structural contracts for ToS-owned surfaces","owner_branch":"ToS/contracts"},{"layer":"domain_topology","meaning":"branch-shaped philosophy topology and local graph workbench routes","owner_branch":"ToS/philosophy"},{"layer":"candidate_intake","meaning":"provisional extraction and promotion residue","owner_branch":"ToS/candidate-intake"},{"layer":"research_packet","meaning":"non-authoritative research scaffolds for later review","owner_branch":"ToS/research-packets"},{"layer":"review_evidence","meaning":"dated inspection notes and review evidence for corpus growth","owner_branch":"ToS/review-ledger"},{"layer":"public_compatibility","meaning":"public-safe mirrors and compatibility examples","owner_branch":"ToS/public-compatibility"},{"layer":"derived_export","meaning":"generated downstream read models subordinate to ToS authority","owner_branch":"ToS/derived-exports"},{"layer":"runtime_projection","meaning":"runtime access, visualization, MCP, UI, and projection stores only","owner_branch":"abyss-stack"}],"graph_views":[{"entry_surface":"ToS/source_home.manifest.json","layout_hint":"elk-layered-or-graphviz-dot","purpose":"show the whole ToS home as a branch-shaped tree","view_id":"corpus-topology"},{"entry_surface":"ToS/source_home.manifest.json","layout_hint":"layered-filter","purpose":"switch corpus visibility by witness, research, candidate, canon, compatibility, and export layers","view_id":"authority-layers"},{"entry_surface":"ToS/canon/relations","layout_hint":"directed-route-graph","purpose":"inspect a concrete relation pack without losing its owner branch and provenance","view_id":"route-graph"},{"entry_surface":"ToS/canon","layout_hint":"sigma-graphology-webgl","purpose":"expand around one node by bounded hops over the full corpus substrate","view_id":"node-neighborhood"},{"entry_surface":"ToS/source-witnesses","layout_hint":"dag","purpose":"trace source witness or research packet pressure into candidate, canon, and export surfaces","view_id":"provenance-dag"},{"entry_surface":"ToS/candidate-intake","layout_hint":"elk-layered-flow","purpose":"review candidate-intake material against canon promotion status","view_id":"promotion-flow"},{"entry_surface":"ToS/derived-exports/tos_corpus_index.min.json","layout_hint":"changed-subgraph","purpose":"compare two corpus index snapshots for review","view_id":"diff-snapshot"}],"owner_repo":"Tree-of-Sophia","runtime_projection_boundary":{"allowed":["read ToS-owned corpus surfaces","serve MCP resources and tools that point back to ToS","build runtime graph projections, UI views, and Neo4j caches","emit review diagnostics without changing ToS authority"],"not_allowed":["move canonical ToS meaning into abyss-stack","treat Neo4j, MCP, UI, or runtime cache as source truth","write ToS canon without ToS validators and explicit operator route"],"runtime_owner":"abyss-stack"},"schema_ref":"ToS/contracts/tos-corpus-index.schema.json","schema_version":"tos_corpus_index_v1","surface_kind":"derived_corpus_index","validation_refs":["scripts/build_tos_corpus_index.py","scripts/validate_tos_corpus_index.py","tests/test_tos_corpus_index.py"]}"#;
