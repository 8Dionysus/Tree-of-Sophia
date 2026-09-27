//! Selected authored philosophy cut -> frozen bounded native raw plan.
//! Metadata custody is explicit because maintained authored JSONL files can
//! exceed the stage's raw-row limit. Original bytes are read through the cut.
use crate::knowledge_stage::{InputCollectionReceipt, InputRow, KnowledgeStage, WritePhase};
use crate::source_philosophy_atlas::{
    self, ATLAS_SCHEMA, AtlasLimits, NODE_SOURCES, RELATION_SOURCES,
};
use crate::source_philosophy_graph::{
    self, CLUSTER_CONTRACT, GRAPH_REF, GRAPH_SCHEMA, GraphLimits, REVIEW_CONTRACT, VIEWS_REF,
};
use crate::source_philosophy_multilingual::{LABEL_LEDGER, Multilingual, MultilingualLimits};
use crate::source_philosophy_support::{array, bytes, object, required, strings};
use crate::source_philosophy_views::{self, ATLAS_REF, VIEW_CONTRACT, VIEWS_SCHEMA, ViewLimits};
use crate::{Error, QueryVocabulary, Result, SourceBinding};
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, MemberMetadata, SourceMembershipV1};
use tos_validation::SchemaBackendProbe;
use tos_validation::executor::{BatchBudget, BatchStreamBudget};
use tos_validation::source_cut::{CutSchemaCheck, CutSchemaExecutor};
pub const PHILOSOPHY_SOURCE_CUSTODY: &str = "philosophy-source-custody";
pub const PHILOSOPHY_MEMBERS_ROLE: &str = "authored-philosophy-current-members";
pub const PHILOSOPHY_MEMBERS_PROFILE: &str = "tos.philosophy-source.current-members.v1";
pub const PHILOSOPHY_CONTRACTS_ROLE: &str = "philosophy-projection-contracts";
pub const PHILOSOPHY_CONTRACTS_PROFILE: &str = "tos.philosophy-source.contracts.v1";
const PROFILE: &str = "philosophy-node-edge-v1";
const MATERIAL_COLLECTIONS: [(&str, &str); 5] = [
    ("views", "view_id"),
    ("clusters", "cluster_id"),
    ("review_packets", "packet_id"),
    ("unresolved_review_surfaces", "surface_id"),
    ("header", "header_id"),
];
#[derive(Clone, Copy, Debug)]
pub struct PhilosophySourceLimits {
    pub max_manifest_members: u64,
    pub max_custody_members: u64,
    pub max_source_file_bytes: usize,
    pub max_raw_row_bytes: usize,
    pub max_page_rows: usize,
    pub max_page_bytes: usize,
    pub max_work_bytes: u64,
    pub atlas: AtlasLimits,
    pub views: ViewLimits,
    pub graph: GraphLimits,
    pub multilingual: MultilingualLimits,
    pub schema_batches: BatchStreamBudget,
}
impl Default for PhilosophySourceLimits {
    fn default() -> Self {
        Self {
            max_manifest_members: 100_000,
            max_custody_members: 10_000,
            max_source_file_bytes: 32 * 1024 * 1024,
            max_raw_row_bytes: 8 * 1024 * 1024,
            max_page_rows: 1,
            max_page_bytes: 8 * 1024 * 1024,
            max_work_bytes: 1024 * 1024 * 1024,
            atlas: AtlasLimits::default(),
            views: ViewLimits::default(),
            graph: GraphLimits::default(),
            multilingual: MultilingualLimits::default(),
            schema_batches: BatchStreamBudget::laboratory(),
        }
    }
}
impl PhilosophySourceLimits {
    fn validate(self) -> Result<()> {
        if self.max_manifest_members == 0
            || self.max_custody_members == 0
            || self.max_source_file_bytes == 0
            || self.max_source_file_bytes > 32 * 1024 * 1024
            || self.max_raw_row_bytes == 0
            || self.max_raw_row_bytes > 8 * 1024 * 1024
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self
                .max_page_rows
                .checked_mul(self.max_raw_row_bytes)
                .is_none_or(|n| n > self.max_page_bytes)
            || self.max_work_bytes == 0
            || self.schema_batches.max_chunks == 0
            || self.schema_batches.max_total_units == 0
            || self.schema_batches.max_total_raw_bytes == 0
            || self.schema_batches.total_execution_wall.is_zero()
            || self.schema_batches.operation_cpu_seconds == 0
            || self.schema_batches.operation_address_space_bytes == 0
            || self.schema_batches.max_total_wire_bytes == 0
            || self.schema_batches.max_distinct_selectors == 0
            || self.schema_batches.batch.max_units == 0
            || self.schema_batches.batch.max_units > BatchBudget::MAX_UNITS
            || self.schema_batches.batch.max_total_raw_bytes == 0
            || self.schema_batches.batch.max_total_raw_bytes > BatchBudget::MAX_RAW_BYTES
        {
            return Err(Error::Budget("philosophy source limits"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct PhilosophySourceCollection {
    pub source_graph: String,
    pub input_role: String,
    pub adapter_profile: String,
    pub collection: String,
    pub count: u64,
    pub root_sha256: String,
}
#[derive(Clone, Debug)]
pub struct PhilosophySourceReceipt {
    pub source_revision: String,
    pub job_source_cut: String,
    pub manifest_members: u64,
    pub manifest_membership_root_sha256: String,
    pub selected_members_read: u64,
    pub selected_source_root_sha256: String,
    pub raw_collections: Vec<PhilosophySourceCollection>,
    pub material_collections: Vec<PhilosophySourceCollection>,
    pub atlas_counts: Value,
    pub graph_counts: Value,
    pub work_bytes: u64,
    pub schema_batches: u64,
    pub schema_units: u64,
    pub schema_raw_bytes: u64,
    pub current_members_only: bool,
    pub final_graph_rows_written: bool,
}
/// The source owner supplies the original independently selected member EOF.
/// This packet describes exact manifest metadata; it does not pretend to be
/// whole-file source bytes or confer source/canon admission.
pub fn philosophy_source_member_packet(
    revision: SourceRevision,
    membership: SourceMembershipV1,
    member: &MemberMetadata,
) -> Value {
    json!({"schema_version":"tos_philosophy_source_member_v1","source_revision":revision.0.to_hex(),"manifest_members":membership.count,"manifest_membership_root_sha256":membership.digest.to_hex(),"path":member.path.as_str(),"size_bytes":member.size_bytes,"sha256":member.sha256.to_hex(),"mode":member.mode})
}
pub struct PhilosophySourcePlan {
    receipt: PhilosophySourceReceipt,
    binding: Value,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    inputs: Vec<InputCollectionReceipt>,
    vocabulary: QueryVocabulary,
}
impl PhilosophySourcePlan {
    pub fn receipt(&self) -> &PhilosophySourceReceipt {
        &self.receipt
    }
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("philosophy source cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("philosophy source deadline"));
    }
    Ok(())
}
fn charge(work: &mut u64, n: usize, l: PhilosophySourceLimits) -> Result<()> {
    *work = work
        .checked_add(n as u64)
        .ok_or(Error::Budget("philosophy source work"))?;
    if *work > l.max_work_bytes {
        return Err(Error::Budget("philosophy source work"));
    }
    Ok(())
}
fn framed(h: &mut Digest256Hasher, id: &str) {
    h.update(&(id.len() as u64).to_be_bytes());
    h.update(id.as_bytes());
}
fn binding(b: &SourceBinding, transport: bool) -> Value {
    let mut v = json!({"owner_profile":b.owner_profile,"source_cut":b.source_cut,"through_commit_seq":b.through_commit_seq,"membership_root":b.membership_root,"index_generation":b.index_generation,"route_map_version":b.route_map_version,"reader_abi":b.reader_abi,"complete":b.complete});
    if transport {
        v["projection_root_sha256"] = json!(b.projection_root_sha256);
    }
    v
}
fn source_fields(v: &Value) -> Value {
    json!({"owner_profile":v["owner_profile"],"source_cut":v["source_cut"],"through_commit_seq":v["through_commit_seq"],"membership_root":v["membership_root"],"index_generation":v["index_generation"],"route_map_version":v["route_map_version"],"reader_abi":v["reader_abi"],"complete":v["complete"]})
}
fn input_snapshot(c: &InputCollectionReceipt) -> Value {
    json!({"source_graph":c.source_graph,"collection":c.collection,"role":c.input_role,"profile":c.adapter_profile,"count":c.expected_count,"root":c.expected_root_sha256})
}
fn selected(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    l: PhilosophySourceLimits,
) -> Result<()> {
    if cut.current().revision() != revision
        || cut
            .stream(revision)
            .map_err(|e| Error::Source(e.to_string()))?
            .expectation()
            != membership
    {
        return Err(Error::Invalid("philosophy independent selected cut"));
    }
    if membership.count > l.max_manifest_members {
        return Err(Error::Budget("philosophy manifest members"));
    }
    Ok(())
}
fn source_path(path: &str) -> bool {
    path.starts_with("ToS/philosophy/")
        && !path.split('/').any(|p| p == "payload")
        && (path.ends_with(".json") || path.ends_with(".jsonl") || path.ends_with(".md"))
}
fn schema_path(path: &str) -> bool {
    [ATLAS_SCHEMA, VIEWS_SCHEMA, GRAPH_SCHEMA].contains(&path)
}
fn custody(
    stage: &KnowledgeStage<'_>,
    cut: &CorpusCutReader,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    l: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
) -> Result<Vec<InputCollectionReceipt>> {
    if stage
        .exact_receipt()
        .collections
        .iter()
        .filter(|c| c.source_graph == PHILOSOPHY_SOURCE_CUSTODY)
        .count()
        != 2
    {
        return Err(Error::Invalid("philosophy custody collection closure"));
    }
    let mut inputs = Vec::new();
    for (name, role, profile) in [
        (
            "current-members",
            PHILOSOPHY_MEMBERS_ROLE,
            PHILOSOPHY_MEMBERS_PROFILE,
        ),
        (
            "contracts",
            PHILOSOPHY_CONTRACTS_ROLE,
            PHILOSOPHY_CONTRACTS_PROFILE,
        ),
    ] {
        let entries = stage
            .exact_receipt()
            .collections
            .iter()
            .filter(|c| {
                c.source_graph == PHILOSOPHY_SOURCE_CUSTODY
                    && c.collection == name
                    && c.input_role == role
                    && c.adapter_profile == profile
            })
            .collect::<Vec<_>>();
        if entries.len() != 1 {
            return Err(Error::Invalid("philosophy custody registration"));
        }
        let entry = entries[0];
        if entry.expected_count > l.max_custody_members {
            return Err(Error::Budget("philosophy custody members"));
        }
        let mut after = None;
        let mut count = 0u64;
        let mut hash = Digest256Hasher::new();
        loop {
            check(deadline, cancelled)?;
            let page = stage.scan_input(&entry.source_graph, name, after.as_deref(), 1)?;
            for row in page.rows {
                charge(work, row.payload.len(), l)?;
                let path =
                    RelativePath::parse(&row.id).map_err(|e| Error::Source(e.to_string()))?;
                let metadata = cut
                    .current()
                    .member(&path)
                    .ok_or(Error::Invalid("philosophy custody current member"))?;
                if name == "current-members" {
                    if !source_path(&row.id)
                        || object(&row.payload, 8192)?
                            != philosophy_source_member_packet(revision, membership, metadata)
                    {
                        return Err(Error::Invalid("philosophy exact selected member packet"));
                    }
                } else if !schema_path(&row.id)
                    || metadata.size_bytes != row.payload.len() as u64
                    || metadata.sha256 != Digest256::of_bytes(&row.payload)
                {
                    return Err(Error::Invalid("philosophy selected schema bytes"));
                }
                count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("philosophy custody rows"))?;
                if count > l.max_custody_members {
                    return Err(Error::Budget("philosophy custody rows"));
                }
                framed(&mut hash, &row.id);
                hash.update(Digest256::of_bytes(&row.payload).as_bytes());
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        if count != entry.expected_count || hash.finalize().to_hex() != entry.expected_root_sha256 {
            return Err(Error::Invalid("philosophy independent custody root/count"));
        }
        inputs.push(entry.clone());
    }
    for path in [ATLAS_SCHEMA, VIEWS_SCHEMA, GRAPH_SCHEMA] {
        check(deadline, cancelled)?;
        let row = stage
            .raw_by_id(PHILOSOPHY_SOURCE_CUSTODY, "contracts", path)?
            .ok_or(Error::Invalid("philosophy required schema custody"))?;
        charge(work, row.payload.len(), l)?;
    }
    let mut seen = 0;
    for _ in cut.current().members() {
        check(deadline, cancelled)?;
        seen += 1;
        if seen > l.max_manifest_members {
            return Err(Error::Budget("philosophy current membership EOF"));
        }
    }
    if seen != membership.count {
        return Err(Error::Invalid("philosophy complete current membership EOF"));
    }
    Ok(inputs)
}
struct SourceRead<'a, 'stage> {
    stage: &'a KnowledgeStage<'stage>,
    cut: &'a CorpusCutReader,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    limits: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    work: u64,
    seen: BTreeSet<String>,
}
impl SourceRead<'_, '_> {
    fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        check(self.deadline, self.cancelled)?;
        if !source_path(path) {
            return Err(Error::Invalid("philosophy exact source recipe path"));
        }
        let relative = RelativePath::parse(path).map_err(|e| Error::Source(e.to_string()))?;
        let metadata = self
            .cut
            .current()
            .member(&relative)
            .ok_or(Error::Invalid("philosophy required current source missing"))?;
        let packet = self
            .stage
            .raw_by_id(PHILOSOPHY_SOURCE_CUSTODY, "current-members", path)?
            .ok_or(Error::Invalid(
                "philosophy used source lacks independent custody",
            ))?;
        charge(&mut self.work, packet.payload.len(), self.limits)?;
        if object(&packet.payload, 8192)?
            != philosophy_source_member_packet(self.revision, self.membership, metadata)
        {
            return Err(Error::Invalid("philosophy selected source packet drift"));
        }
        let member = self
            .cut
            .read_member(
                self.revision,
                &relative,
                self.limits.max_source_file_bytes as u64,
                self.deadline,
                self.cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
        charge(&mut self.work, member.raw.len(), self.limits)?;
        if member.raw.len() as u64 != metadata.size_bytes
            || Digest256::of_bytes(&member.raw) != metadata.sha256
        {
            return Err(Error::Invalid("philosophy source raw fixity"));
        }
        self.seen.insert(path.into());
        if self.seen.len() as u64 > self.limits.max_custody_members {
            return Err(Error::Budget("philosophy selected sources"));
        }
        Ok(member.raw)
    }
}
#[derive(Default)]
struct SchemaWork {
    batches: u64,
    units: u64,
    raw_bytes: u64,
}
fn execute_schema_batch(
    executor: &mut impl CutSchemaExecutor,
    checks: &mut Vec<CutSchemaCheck>,
    budget: BatchBudget,
    aggregate: BatchStreamBudget,
    schema_work: &mut SchemaWork,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    if checks.is_empty() {
        return Ok(());
    }
    check(deadline, cancelled)?;
    schema_work.batches = schema_work
        .batches
        .checked_add(1)
        .filter(|n| *n <= aggregate.max_chunks)
        .ok_or(Error::Budget("philosophy schema aggregate batches"))?;
    schema_work.units = schema_work
        .units
        .checked_add(checks.len() as u64)
        .filter(|n| *n <= aggregate.max_total_units)
        .ok_or(Error::Budget("philosophy schema aggregate units"))?;
    let results = executor
        .check_batch(checks, budget, deadline, cancelled)
        .map_err(|e| Error::Source(format!("philosophy selected schema batch: {e:?}")))?;
    check(deadline, cancelled)?;
    if results.len() != checks.len() {
        return Err(Error::Invalid("philosophy generated schema batch coverage"));
    }
    if results.iter().any(|valid| !valid) {
        return Err(Error::Invalid("philosophy generated schema instance"));
    }
    checks.clear();
    Ok(())
}
// Exact current schemas have a flat top-level object and independent array
// items. Check real fields/items with their named selected subschemas, plus
// container assertions; do not manufacture an empty whole projection to pass.
fn visit_projection_instances(
    stage: &KnowledgeStage<'_>,
    value: &Value,
    path: &str,
    contract: &str,
    l: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
    mut visit: impl FnMut(&Value, String, String, usize, &mut u64) -> Result<()>,
) -> Result<()> {
    let row = stage
        .raw_by_id(PHILOSOPHY_SOURCE_CUSTODY, "contracts", contract)?
        .ok_or(Error::Invalid("philosophy schema custody"))?;
    charge(work, row.payload.len(), l)?;
    let schema = object(&row.payload, 1 << 20)?;
    let top = schema
        .as_object()
        .ok_or(Error::Invalid("philosophy schema object"))?;
    let allowed = BTreeSet::from([
        "$schema",
        "$id",
        "title",
        "description",
        "$comment",
        "type",
        "additionalProperties",
        "required",
        "properties",
        "$defs",
    ]);
    if top.keys().any(|k| !allowed.contains(k.as_str()))
        || schema["type"] != "object"
        || schema["additionalProperties"] != false
    {
        return Err(Error::Source(
            "philosophy selected schema has undecomposed top-level assertions".into(),
        ));
    }
    let properties = schema["properties"]
        .as_object()
        .ok_or(Error::Invalid("philosophy schema properties"))?;
    let actual = value
        .as_object()
        .ok_or(Error::Invalid("philosophy projection object"))?;
    if strings(&schema["required"])?
        .iter()
        .any(|k| !actual.contains_key(k))
        || actual.keys().any(|k| !properties.contains_key(k))
    {
        return Err(Error::Invalid("philosophy projection exact header closure"));
    }
    for (key, instance) in actual {
        check(deadline, cancelled)?;
        let selector = format!(
            "{contract}#/properties/{}",
            key.replace('~', "~0").replace('/', "~1")
        );
        let definition = &properties[key];
        if definition["type"] == "array" && instance.is_array() {
            let d = definition
                .as_object()
                .ok_or(Error::Invalid("philosophy array schema"))?;
            if d.keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "type"
                        | "minItems"
                        | "maxItems"
                        | "items"
                        | "description"
                        | "title"
                        | "$comment"
                )
            }) {
                return Err(Error::Source(
                    "philosophy selected array schema has undecomposed assertions".into(),
                ));
            }
            let items = instance.as_array().expect("array");
            let bound = |key: &str, default: u64| -> Result<u64> {
                definition.get(key).map_or(Ok(default), |v| {
                    v.as_u64()
                        .ok_or(Error::Invalid("philosophy schema integer array bound"))
                })
            };
            let min = bound("minItems", 0)?;
            let max = bound("maxItems", u64::MAX)?;
            if (items.len() as u64) < min
                || (items.len() as u64) > max
                || !definition.get("items").is_some_and(Value::is_object)
            {
                return Err(Error::Invalid("philosophy schema array bounds/items"));
            }
            for (index, item) in items.iter().enumerate() {
                check(deadline, cancelled)?;
                visit(
                    item,
                    format!("{path}#/{key}/{index}"),
                    format!("{selector}/items"),
                    l.max_raw_row_bytes,
                    work,
                )?;
            }
        } else {
            visit(
                instance,
                format!("{path}#/{key}"),
                selector,
                SchemaBackendProbe::MAX_INSTANCE_BYTES,
                work,
            )?;
        }
    }
    Ok(())
}
fn schema_batch_full(
    count: usize,
    pending_bytes: usize,
    next_bytes: usize,
    budget: BatchBudget,
) -> bool {
    count == budget.max_units
        || pending_bytes
            .checked_add(next_bytes)
            .is_none_or(|n| n > budget.max_total_raw_bytes)
}

fn validate_projection(
    stage: &KnowledgeStage<'_>,
    executor: &mut impl CutSchemaExecutor,
    value: &Value,
    path: &str,
    contract: &str,
    l: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
    schema_work: &mut SchemaWork,
) -> Result<()> {
    let mut checks = Vec::with_capacity(l.schema_batches.batch.max_units);
    let mut pending_bytes = 0usize;
    let mut submit = |path: String, raw: Vec<u8>, contract: String, work: &mut u64| -> Result<()> {
        if raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES {
            return Err(Error::Budget("philosophy schema batch instance"));
        }
        let cost = executor
            .schema_input_cost(&path, &raw, &contract, checks.len() as u64)
            .map_err(|e| Error::Source(format!("philosophy schema input cost:{e:?}")))?;
        let decoded_bytes = usize::try_from(cost.decoded_instance_bytes)
            .map_err(|_| Error::Budget("philosophy schema decoded instance"))?;
        charge(work, decoded_bytes, l)?;
        if decoded_bytes > l.schema_batches.batch.max_total_raw_bytes {
            return Err(Error::Budget("philosophy schema decoded instance"));
        }
        if schema_batch_full(
            checks.len(),
            pending_bytes,
            decoded_bytes,
            l.schema_batches.batch,
        ) {
            execute_schema_batch(
                executor,
                &mut checks,
                l.schema_batches.batch,
                l.schema_batches,
                schema_work,
                deadline,
                cancelled,
            )?;
            pending_bytes = 0;
        }
        pending_bytes = pending_bytes
            .checked_add(decoded_bytes)
            .ok_or(Error::Budget("philosophy schema batch bytes"))?;
        schema_work.raw_bytes = schema_work
            .raw_bytes
            .checked_add(cost.decoded_instance_bytes)
            .filter(|n| *n <= l.schema_batches.max_total_raw_bytes)
            .ok_or(Error::Budget("philosophy schema aggregate raw bytes"))?;
        checks.push(CutSchemaCheck {
            path,
            raw,
            contract,
        });
        Ok(())
    };
    visit_projection_instances(
        stage,
        value,
        path,
        contract,
        l,
        deadline,
        cancelled,
        work,
        |instance, path, contract, cap, work| {
            let raw = bytes(instance, cap)?;
            charge(work, raw.len(), l)?;
            submit(path, raw, contract, work)
        },
    )?;
    drop(submit);
    execute_schema_batch(
        executor,
        &mut checks,
        l.schema_batches.batch,
        l.schema_batches,
        schema_work,
        deadline,
        cancelled,
    )
}

fn insert(
    stage: &mut KnowledgeStage<'_>,
    collection: &str,
    id: &str,
    ordinal: usize,
    raw: &[u8],
    l: PhilosophySourceLimits,
    work: &mut u64,
) -> Result<()> {
    if id.is_empty() || id.len() > 4096 || id.contains('\0') || raw.len() > l.max_raw_row_bytes {
        return Err(Error::Budget("philosophy planned raw row"));
    }
    charge(work, raw.len(), l)?;
    stage.charge_materialized(1, (raw.len() + id.len() + collection.len() + 48) as u64)?;
    stage.with_connection(WritePhase::Normalized, |db| {
        db.execute(
            "INSERT INTO source_philosophy_rows VALUES(?1,?2,?3,?4,?5)",
            params![
                collection,
                id,
                ordinal as i64,
                raw,
                Digest256::of_bytes(raw).as_bytes().as_slice()
            ],
        )?;
        Ok(())
    })
}
fn insert_projection_rows(
    stage: &mut KnowledgeStage<'_>,
    collection: &str,
    key: &str,
    rows: &[Value],
    l: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
) -> Result<()> {
    let mut pending: Vec<(String, usize, Vec<u8>)> = Vec::with_capacity(l.max_page_rows);
    let mut pending_bytes = 0usize;
    let mut flush = |pending: &mut Vec<(String, usize, Vec<u8>)>| -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        check(deadline, cancelled)?;
        for (id, _, raw) in pending.iter() {
            charge(work, raw.len(), l)?;
            stage.charge_materialized(1, (raw.len() + id.len() + collection.len() + 48) as u64)?;
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for (id, ordinal, raw) in pending.iter() {
                check(deadline, cancelled)?;
                tx.execute(
                    "INSERT INTO source_philosophy_rows VALUES(?1,?2,?3,?4,?5)",
                    params![
                        collection,
                        id,
                        *ordinal as i64,
                        raw,
                        Digest256::of_bytes(raw).as_bytes().as_slice()
                    ],
                )?;
            }
            check(deadline, cancelled)?;
            tx.commit()?;
            Ok(())
        })?;
        pending.clear();
        Ok(())
    };
    for (ordinal, row) in rows.iter().enumerate() {
        check(deadline, cancelled)?;
        let id = required(row, key)?;
        if id.is_empty() || id.len() > 4096 || id.contains('\0') {
            return Err(Error::Budget("philosophy planned raw row"));
        }
        let raw = bytes(row, l.max_raw_row_bytes)?;
        if pending.len() == l.max_page_rows
            || pending_bytes
                .checked_add(raw.len())
                .is_none_or(|n| n > l.max_page_bytes)
        {
            flush(&mut pending)?;
            pending_bytes = 0;
        }
        if raw.len() > l.max_page_bytes {
            return Err(Error::Budget("philosophy raw insert chunk bytes"));
        }
        pending_bytes = pending_bytes
            .checked_add(raw.len())
            .ok_or(Error::Budget("philosophy raw insert chunk bytes"))?;
        pending.push((id.to_owned(), ordinal, raw));
    }
    flush(&mut pending)
}
const INITIAL_ROW_PAGE: &str = "SELECT id,CASE WHEN length(payload)<=?3 THEN payload ELSE NULL END,payload_sha256 FROM source_philosophy_rows WHERE collection=?1 ORDER BY id LIMIT ?4";
const CONTINUED_ROW_PAGE: &str = "SELECT id,CASE WHEN length(payload)<=?3 THEN payload ELSE NULL END,payload_sha256 FROM source_philosophy_rows WHERE collection=?1 AND id>?2 ORDER BY id LIMIT ?4";
// Keep continuation as a direct primary-key range. A nullable OR cursor
// makes SQLite revisit the collection prefix on every bounded page.
fn row_page(
    db: &rusqlite::Connection,
    collection: &str,
    after_id: Option<&str>,
    limits: PhilosophySourceLimits,
) -> Result<Vec<(String, Vec<u8>, Vec<u8>)>> {
    let sql = if after_id.is_some() {
        CONTINUED_ROW_PAGE
    } else {
        INITIAL_ROW_PAGE
    };
    let mut query = db.prepare(sql)?;
    let rows = query.query_map(
        params![
            collection,
            after_id,
            limits.max_raw_row_bytes as i64,
            limits.max_page_rows as i64
        ],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)
}
fn material_page(
    planner: &mut KnowledgeStage<'_>,
    collection: &str,
    after_id: Option<&str>,
    limits: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<PhilosophyMaterialRow>> {
    check(deadline, cancelled)?;
    let rows = planner.with_connection(WritePhase::Sort, |db| {
        row_page(db, collection, after_id, limits)
    })?;
    rows.into_iter()
        .map(|(id, payload, sha)| {
            check(deadline, cancelled)?;
            let digest = Digest256::of_bytes(&payload);
            if digest.as_bytes() != sha.as_slice() {
                return Err(Error::Invalid("philosophy material row digest"));
            }
            Ok(PhilosophyMaterialRow {
                id,
                payload,
                payload_sha256: digest.to_hex(),
            })
        })
        .collect()
}
fn visit_rows<F>(
    stage: &mut KnowledgeStage<'_>,
    collection: &str,
    l: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
    mut sink: F,
) -> Result<(u64, String)>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &[u8]) -> Result<()>,
{
    let mut after: Option<String> = None;
    let mut count = 0u64;
    let mut hash = Digest256Hasher::new();
    loop {
        check(deadline, cancelled)?;
        let batch = stage.with_connection(WritePhase::Sort, |db| {
            row_page(db, collection, after.as_deref(), l)
        })?;
        if batch.is_empty() {
            break;
        }
        for (id, raw, sha) in batch {
            check(deadline, cancelled)?;
            if sha.as_slice() != Digest256::of_bytes(&raw).as_bytes() {
                return Err(Error::Invalid("philosophy plan row digest"));
            }
            charge(work, raw.len(), l)?;
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("philosophy plan rows"))?;
            let cap = match collection {
                "nodes" => l.graph.max_nodes,
                "edges" => l.graph.max_edges,
                "views" | "review_packets" => l.graph.max_views,
                "clusters" | "unresolved_review_surfaces" => l.graph.max_clusters,
                "header" | "view_catalog" => 1,
                _ => return Err(Error::Invalid("philosophy plan collection")),
            };
            if count > cap as u64 {
                return Err(Error::Budget("philosophy plan rows"));
            }
            framed(&mut hash, &id);
            hash.update(&sha);
            sink(stage, &id, &raw)?;
            after = Some(id);
        }
    }
    Ok((count, hash.finalize().to_hex()))
}
fn collection(
    source: &str,
    role: &str,
    name: &str,
    count: u64,
    root: String,
) -> PhilosophySourceCollection {
    PhilosophySourceCollection {
        source_graph: source.into(),
        input_role: role.into(),
        adapter_profile: PROFILE.into(),
        collection: name.into(),
        count,
        root_sha256: root,
    }
}
fn target_receipts(stage: &KnowledgeStage<'_>, receipt: &PhilosophySourceReceipt) -> Result<()> {
    for c in &receipt.raw_collections {
        let entries = stage
            .exact_receipt()
            .collections
            .iter()
            .filter(|r| r.source_graph == c.source_graph && r.collection == c.collection)
            .collect::<Vec<_>>();
        if entries.len() != 1
            || entries[0].input_role != c.input_role
            || entries[0].adapter_profile != c.adapter_profile
            || entries[0].expected_count != c.count
            || entries[0].expected_root_sha256 != c.root_sha256
        {
            return Err(Error::Invalid("philosophy independent target raw receipts"));
        }
    }
    let graph = &receipt
        .raw_collections
        .first()
        .ok_or(Error::Invalid("philosophy raw plan closure"))?
        .source_graph;
    if stage
        .exact_receipt()
        .collections
        .iter()
        .filter(|c| &c.source_graph == graph)
        .count()
        != 2
    {
        return Err(Error::Invalid("philosophy target collection closure"));
    }
    Ok(())
}
/// Actual authored atlas -> source view catalog -> complete graph projection.
/// Custody packets have independent selected-current member roots, not fabricated
/// file bytes. No native graph output receipt is needed before this derivation.
pub fn plan_philosophy_source_inputs(
    planner: &mut KnowledgeStage<'_>,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    vocabulary: &QueryVocabulary,
    executor: &mut impl CutSchemaExecutor,
    limits: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PhilosophySourcePlan> {
    let result = (|| {
        limits.validate()?;
        check(deadline, cancelled)?;
        selected(cut, expected_revision, expected_membership, limits)?;
        let sources = vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == PROFILE)
            .collect::<Vec<_>>();
        if sources.len() != 1 {
            return Err(Error::Invalid("philosophy authored profile selection"));
        }
        let source = sources[0];
        let mut work = 0;
        let inputs = custody(
            planner,
            cut,
            expected_revision,
            expected_membership,
            limits,
            deadline,
            cancelled,
            &mut work,
        )?;
        let mut view_refs = Vec::new();
        let mut optional_refs = BTreeSet::new();
        for metadata in cut.current().members() {
            check(deadline, cancelled)?;
            let path = metadata.path.as_str();
            if let Some(name) = path
                .strip_prefix("ToS/philosophy/graph-workbench/views/")
                .and_then(|p| p.strip_suffix(".graph.md"))
                .filter(|p| !p.contains('/'))
            {
                let _ = name;
                view_refs.push(path.to_owned());
            }
            if NODE_SOURCES.contains(&path) || RELATION_SOURCES.contains(&path) {
                optional_refs.insert(path.to_owned());
            }
        }
        let (atlas, catalog, graph, read_members, next_work) = {
            let mut reader = SourceRead {
                stage: planner,
                cut,
                revision: expected_revision,
                membership: expected_membership,
                limits,
                deadline,
                cancelled,
                work,
                seen: BTreeSet::new(),
            };
            let ledger = object(
                &reader.read(LABEL_LEDGER)?,
                limits.multilingual.max_ledger_bytes,
            )?;
            let multi = Multilingual::from_ledger_with_limits(&ledger, limits.multilingual)?;
            let atlas = source_philosophy_atlas::build_atlas(
                &mut |path| reader.read(path),
                &optional_refs,
                &view_refs,
                &multi,
                limits.atlas,
                deadline,
                cancelled,
            )?;
            check(deadline, cancelled)?;
            let catalog = source_philosophy_views::build_views(
                &mut |path| reader.read(path),
                array(&atlas, "nodes")?,
                array(&atlas, "edges")?,
                limits.views,
            )?;
            let view_contract =
                object(&reader.read(VIEW_CONTRACT)?, limits.views.max_source_bytes)?;
            let clusters = object(
                &reader.read(CLUSTER_CONTRACT)?,
                limits.views.max_source_bytes,
            )?;
            let reviews = object(
                &reader.read(REVIEW_CONTRACT)?,
                limits.views.max_source_bytes,
            )?;
            let graph = source_philosophy_graph::build_graph(
                &atlas,
                &catalog,
                &view_contract,
                &clusters,
                &reviews,
                &multi,
                limits.graph,
                deadline,
                cancelled,
            )?;
            // Navigation paths are real current source returns too. Reading them binds
            // their bytes without making their fixed topology labels source facts.
            let mut navigation = BTreeSet::new();
            for name in ["nodes", "edges"] {
                for row in array(&atlas, name)?
                    .iter()
                    .chain(array(&graph, name)?.iter())
                {
                    navigation.insert(required(row, "source_ref")?.to_owned());
                }
            }
            for path in navigation {
                if !reader.seen.contains(&path) {
                    reader.read(&path)?;
                }
            }
            (atlas, catalog, graph, reader.seen, reader.work)
        };
        work = next_work;
        let schema_deadline = Instant::now()
            .checked_add(limits.schema_batches.total_execution_wall)
            .ok_or(Error::Budget("philosophy schema aggregate deadline"))?
            .min(deadline);
        // Validate all actual selectors/container assertions and the complete
        // unit-count ceiling before launching any schema child. Reuse the same
        // visit law as execution; $ref-only arrays remain whole-field instances.
        let preflight_start_work = work;
        let mut planned = SchemaWork::default();
        let mut planned_selectors = BTreeSet::new();
        let mut planned_wire = 0u64;
        let mut planned_receipt_bytes = 0u64;
        let mut operation_wire_charged = false;
        for (projection, path, contract) in [
            (&atlas, ATLAS_REF, ATLAS_SCHEMA),
            (&catalog, VIEWS_REF, VIEWS_SCHEMA),
            (&graph, GRAPH_REF, GRAPH_SCHEMA),
        ] {
            let mut count = 0usize;
            let mut pending_bytes = 0usize;
            visit_projection_instances(
                planner,
                projection,
                path,
                contract,
                limits,
                schema_deadline,
                cancelled,
                &mut work,
                |instance, path, contract, cap, work| {
                    // Use the execution serialization law, retaining only one
                    // bounded instance at a time. No complete serialized copy
                    // or schema child is created by this preflight.
                    let raw = bytes(instance, cap)?;
                    charge(work, raw.len(), limits)?;
                    if raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES {
                        return Err(Error::Budget("philosophy schema preflight instance"));
                    }
                    let mut cost = executor
                        .schema_input_cost(&path, &raw, &contract, count as u64)
                        .map_err(|e| {
                            Error::Source(format!("philosophy schema preflight cost:{e:?}"))
                        })?;
                    let decoded_bytes = usize::try_from(cost.decoded_instance_bytes)
                        .map_err(|_| Error::Budget("philosophy schema preflight decoded bytes"))?;
                    charge(work, decoded_bytes, limits)?;
                    if decoded_bytes > limits.schema_batches.batch.max_total_raw_bytes {
                        return Err(Error::Budget(
                            "philosophy schema preflight decoded instance",
                        ));
                    }
                    planned_selectors.insert(cost.selector.clone());
                    if planned_selectors.len() > limits.schema_batches.max_distinct_selectors {
                        return Err(Error::Budget("philosophy schema preflight selectors"));
                    }
                    planned.units = planned
                        .units
                        .checked_add(1)
                        .filter(|n| *n <= limits.schema_batches.max_total_units)
                        .ok_or(Error::Budget("philosophy schema preflight units"))?;
                    planned.raw_bytes = planned
                        .raw_bytes
                        .checked_add(cost.decoded_instance_bytes)
                        .filter(|n| *n <= limits.schema_batches.max_total_raw_bytes)
                        .ok_or(Error::Budget("philosophy schema preflight raw bytes"))?;
                    if schema_batch_full(
                        count,
                        pending_bytes,
                        decoded_bytes,
                        limits.schema_batches.batch,
                    ) {
                        planned.batches = planned
                            .batches
                            .checked_add(1)
                            .filter(|n| *n <= limits.schema_batches.max_chunks)
                            .ok_or(Error::Budget("philosophy schema preflight chunks"))?;
                        count = 0;
                        pending_bytes = 0;
                        cost = executor
                            .schema_input_cost(&path, &raw, &contract, 0)
                            .map_err(|e| {
                                Error::Source(format!("philosophy schema preflight cost:{e:?}"))
                            })?;
                        charge(work, decoded_bytes, limits)?;
                    }
                    if !operation_wire_charged {
                        planned_wire = cost.operation_wire_bytes;
                        operation_wire_charged = true;
                    }
                    let frame_wire = if count == 0 { cost.frame_wire_bytes } else { 0 };
                    planned_wire = planned_wire
                        .checked_add(frame_wire)
                        .and_then(|n| n.checked_add(cost.unit_wire_bytes))
                        .filter(|n| *n <= limits.schema_batches.max_total_wire_bytes)
                        .ok_or(Error::Budget("philosophy schema preflight wire bytes"))?;
                    planned_receipt_bytes =
                        planned_receipt_bytes
                            .checked_add(cost.receipt_bytes)
                            .ok_or(Error::Budget("philosophy schema preflight receipt bytes"))?;
                    if planned.units > cost.remaining_receipts
                        || planned_receipt_bytes > cost.remaining_receipt_bytes
                    {
                        return Err(Error::Budget(
                            "philosophy schema preflight receipt capacity",
                        ));
                    }
                    count += 1;
                    pending_bytes = pending_bytes
                        .checked_add(decoded_bytes)
                        .ok_or(Error::Budget("philosophy schema preflight pending bytes"))?;
                    Ok(())
                },
            )?;
            if count != 0 {
                planned.batches = planned
                    .batches
                    .checked_add(1)
                    .filter(|n| *n <= limits.schema_batches.max_chunks)
                    .ok_or(Error::Budget("philosophy schema preflight chunks"))?;
            }
        }
        // Reserve the repeated execution serialization work before the first
        // child too. Actual execution still charges and enforces the same cap.
        work.checked_add(work - preflight_start_work)
            .filter(|n| *n <= limits.max_work_bytes)
            .ok_or(Error::Budget("philosophy schema execution work preflight"))?;
        executor
            .set_operation_budget(limits.schema_batches)
            .map_err(|e| Error::Source(format!("philosophy schema operation budget:{e:?}")))?;
        let mut schema_work = SchemaWork::default();
        validate_projection(
            planner,
            executor,
            &atlas,
            ATLAS_REF,
            ATLAS_SCHEMA,
            limits,
            schema_deadline,
            cancelled,
            &mut work,
            &mut schema_work,
        )?;
        validate_projection(
            planner,
            executor,
            &catalog,
            VIEWS_REF,
            VIEWS_SCHEMA,
            limits,
            schema_deadline,
            cancelled,
            &mut work,
            &mut schema_work,
        )?;
        validate_projection(
            planner,
            executor,
            &graph,
            GRAPH_REF,
            GRAPH_SCHEMA,
            limits,
            schema_deadline,
            cancelled,
            &mut work,
            &mut schema_work,
        )?;
        if schema_work.units != planned.units {
            return Err(Error::Invalid("philosophy schema planned unit coverage"));
        }
        executor
            .finish(schema_deadline, cancelled)
            .map_err(|e| Error::Source(format!("philosophy schema operation finish:{e:?}")))?;
        planner.with_connection(WritePhase::Schema,|db|{db.execute_batch("CREATE TABLE source_philosophy_rows(collection TEXT NOT NULL,id TEXT NOT NULL,ordinal INTEGER NOT NULL CHECK(ordinal>=0),payload BLOB NOT NULL,payload_sha256 BLOB NOT NULL CHECK(length(payload_sha256)=32),PRIMARY KEY(collection,id)) WITHOUT ROWID;CREATE UNIQUE INDEX source_philosophy_order ON source_philosophy_rows(collection,ordinal);")?;Ok(())})?;
        let mut raw_collections = Vec::new();
        for (name, key) in [("nodes", "node_id"), ("edges", "edge_id")] {
            insert_projection_rows(
                planner,
                name,
                key,
                array(&graph, name)?,
                limits,
                deadline,
                cancelled,
                &mut work,
            )?;
            let (count, root) = visit_rows(
                planner,
                name,
                limits,
                deadline,
                cancelled,
                &mut work,
                |_, _, _| Ok(()),
            )?;
            raw_collections.push(collection(
                &source.source_graph_id,
                &source.input_role,
                name,
                count,
                root,
            ));
        }
        let excluded = [
            "nodes",
            "edges",
            "views",
            "clusters",
            "review_packets",
            "unresolved_review_surfaces",
        ];
        let mut header = Value::Object(
            graph
                .as_object()
                .expect("projection")
                .iter()
                .filter(|(key, _)| !excluded.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        let mut material_collections = Vec::new();
        for (name, key) in MATERIAL_COLLECTIONS {
            if name == "header" {
                continue;
            }
            insert_projection_rows(
                planner,
                name,
                key,
                array(&graph, name)?,
                limits,
                deadline,
                cancelled,
                &mut work,
            )?;
            header.as_object_mut().expect("projection").remove(name);
            let (count, root) = visit_rows(
                planner,
                name,
                limits,
                deadline,
                cancelled,
                &mut work,
                |_, _, _| Ok(()),
            )?;
            material_collections.push(collection(
                &source.source_graph_id,
                "philosophy-projection-material",
                name,
                count,
                root,
            ));
        }
        header.as_object_mut().expect("projection").remove("nodes");
        header.as_object_mut().expect("projection").remove("edges");
        for (name, id, material) in [
            ("header", "projection-header", &header),
            ("view_catalog", "view-catalog", &catalog),
        ] {
            insert(
                planner,
                name,
                id,
                0,
                &bytes(material, limits.max_raw_row_bytes)?,
                limits,
                &mut work,
            )?;
            let (count, root) = visit_rows(
                planner,
                name,
                limits,
                deadline,
                cancelled,
                &mut work,
                |_, _, _| Ok(()),
            )?;
            material_collections.push(collection(
                &source.source_graph_id,
                "philosophy-projection-material",
                name,
                count,
                root,
            ));
        }
        let mut source_root = Digest256Hasher::new();
        source_root.update(b"tos-philosophy-selected-source-v1\0");
        source_root.update(expected_revision.0.as_bytes());
        for path in &read_members {
            check(deadline, cancelled)?;
            let relative = RelativePath::parse(path).map_err(|e| Error::Source(e.to_string()))?;
            let metadata = cut
                .current()
                .member(&relative)
                .ok_or(Error::Invalid("philosophy read member disappeared"))?;
            framed(&mut source_root, path);
            source_root.update(&metadata.size_bytes.to_be_bytes());
            source_root.update(metadata.sha256.as_bytes());
            source_root.update(&metadata.mode.to_be_bytes());
        }
        Ok(PhilosophySourcePlan {
            receipt: PhilosophySourceReceipt {
                source_revision: expected_revision.0.to_hex(),
                job_source_cut: planner.exact_receipt().binding.source_cut.clone(),
                manifest_members: expected_membership.count,
                manifest_membership_root_sha256: expected_membership.digest.to_hex(),
                selected_members_read: read_members.len() as u64,
                selected_source_root_sha256: source_root.finalize().to_hex(),
                raw_collections,
                material_collections,
                atlas_counts: atlas["counts"].clone(),
                graph_counts: graph["counts"].clone(),
                work_bytes: work,
                schema_batches: schema_work.batches,
                schema_units: schema_work.units,
                schema_raw_bytes: schema_work.raw_bytes,
                current_members_only: true,
                final_graph_rows_written: false,
            },
            binding: binding(&planner.exact_receipt().binding, true),
            revision: expected_revision,
            membership: expected_membership,
            inputs,
            vocabulary: vocabulary.clone(),
        })
    })();
    if result.is_err() {
        planner.poison();
    }
    result
}
fn verify_plan(
    planner: &mut KnowledgeStage<'_>,
    plan: &PhilosophySourcePlan,
    cut: &CorpusCutReader,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    vocabulary: &QueryVocabulary,
    l: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
) -> Result<()> {
    l.validate()?;
    check(deadline, cancelled)?;
    selected(cut, revision, membership, l)?;
    if plan.revision != revision
        || plan.membership != membership
        || plan.vocabulary != *vocabulary
        || binding(&planner.exact_receipt().binding, true) != plan.binding
    {
        return Err(Error::Invalid("philosophy frozen source plan binding"));
    }
    let inputs = custody(
        planner, cut, revision, membership, l, deadline, cancelled, work,
    )?;
    if inputs.len() != plan.inputs.len()
        || inputs
            .iter()
            .zip(&plan.inputs)
            .any(|(a, b)| input_snapshot(a) != input_snapshot(b))
    {
        return Err(Error::Invalid("philosophy frozen custody receipts"));
    }
    for c in plan
        .receipt
        .raw_collections
        .iter()
        .chain(&plan.receipt.material_collections)
    {
        let (count, root) = visit_rows(
            planner,
            &c.collection,
            l,
            deadline,
            cancelled,
            work,
            |_, _, _| Ok(()),
        )?;
        if count != c.count || root != c.root_sha256 {
            return Err(Error::Invalid("philosophy frozen raw/material root/count"));
        }
    }
    Ok(())
}
/// Copy actual raw graph rows after separately creating the native target from
/// the frozen raw receipts. Material stays frozen for bounded view/export reads;
/// caller drops the private planner when all consumers have completed.
pub fn render_philosophy_source_plan(
    planner: &mut KnowledgeStage<'_>,
    plan: &PhilosophySourcePlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    vocabulary: &QueryVocabulary,
    target: &mut KnowledgeStage<'_>,
    limits: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PhilosophySourceReceipt> {
    let result = (|| {
        let mut work = plan.receipt.work_bytes;
        verify_plan(
            planner,
            plan,
            cut,
            expected_revision,
            expected_membership,
            vocabulary,
            limits,
            deadline,
            cancelled,
            &mut work,
        )?;
        if binding(&target.exact_receipt().binding, false) != source_fields(&plan.binding) {
            return Err(Error::Invalid("philosophy target source identity"));
        }
        target_receipts(target, &plan.receipt)?;
        for c in &plan.receipt.raw_collections {
            let mut pending: Vec<(String, Vec<u8>)> = Vec::with_capacity(limits.max_page_rows);
            let mut pending_bytes = 0usize;
            let mut flush = |pending: &mut Vec<(String, Vec<u8>)>| -> Result<()> {
                if pending.is_empty() {
                    return Ok(());
                }
                check(deadline, cancelled)?;
                let rows = pending
                    .iter()
                    .map(|(id, raw)| InputRow {
                        source_graph: &c.source_graph,
                        collection: &c.collection,
                        id,
                        payload: raw,
                    })
                    .collect::<Vec<_>>();
                target.ingest_input_batch(&rows)?;
                check(deadline, cancelled)?;
                pending.clear();
                Ok(())
            };
            let (count, root) = visit_rows(
                planner,
                &c.collection,
                limits,
                deadline,
                cancelled,
                &mut work,
                |_, id, raw| {
                    if pending.len() == limits.max_page_rows
                        || pending_bytes
                            .checked_add(raw.len())
                            .is_none_or(|n| n > limits.max_page_bytes)
                    {
                        flush(&mut pending)?;
                        pending_bytes = 0;
                    }
                    if raw.len() > limits.max_page_bytes {
                        return Err(Error::Budget("philosophy raw transfer chunk bytes"));
                    }
                    pending_bytes = pending_bytes
                        .checked_add(raw.len())
                        .ok_or(Error::Budget("philosophy raw transfer chunk bytes"))?;
                    pending.push((id.to_owned(), raw.to_vec()));
                    Ok(())
                },
            )?;
            if count != c.count || root != c.root_sha256 {
                return Err(Error::Invalid("philosophy rendered raw count/root"));
            }
            flush(&mut pending)?;
        }
        let mut receipt = plan.receipt.clone();
        receipt.work_bytes = work;
        Ok(receipt)
    })();
    if result.is_err() {
        planner.poison();
        target.poison();
    }
    result
}
#[derive(Clone, Debug)]
pub struct PhilosophyMaterialRow {
    pub id: String,
    pub payload: Vec<u8>,
    pub payload_sha256: String,
}
/// A bounded material page. The private plan owns its collection identities;
/// the caller cannot substitute a generated-file loader or undeclared table.
pub fn scan_philosophy_source_material(
    planner: &mut KnowledgeStage<'_>,
    plan: &PhilosophySourcePlan,
    collection: &str,
    after_id: Option<&str>,
    limits: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<PhilosophyMaterialRow>> {
    let result = (|| {
        limits.validate()?;
        check(deadline, cancelled)?;
        if binding(&planner.exact_receipt().binding, true) != plan.binding
            || !plan
                .receipt
                .material_collections
                .iter()
                .any(|c| c.collection == collection)
        {
            return Err(Error::Invalid(
                "philosophy material plan binding/collection",
            ));
        }
        let expected = plan
            .receipt
            .material_collections
            .iter()
            .find(|c| c.collection == collection)
            .expect("checked material collection");
        let mut work = 0;
        let (count, root) = visit_rows(
            planner,
            collection,
            limits,
            deadline,
            cancelled,
            &mut work,
            |_, _, _| Ok(()),
        )?;
        if count != expected.count || root != expected.root_sha256 {
            return Err(Error::Invalid("philosophy material frozen root/count"));
        }
        material_page(planner, collection, after_id, limits, deadline, cancelled)
    })();
    if result.is_err() {
        planner.poison();
    }
    result
}
/// Emit the full maintained graph projection without allocating its complete
/// serialization. Array ordinals retain authored lens/review/family order;
/// raw roots remain sorted native-ID witnesses, independently checked above.
pub fn render_philosophy_projection<F>(
    planner: &mut KnowledgeStage<'_>,
    plan: &PhilosophySourcePlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    vocabulary: &QueryVocabulary,
    limits: PhilosophySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut sink: F,
) -> Result<String>
where
    F: FnMut(&[u8]) -> Result<()>,
{
    let result = (|| {
        let mut work = plan.receipt.work_bytes;
        verify_plan(
            planner,
            plan,
            cut,
            expected_revision,
            expected_membership,
            vocabulary,
            limits,
            deadline,
            cancelled,
            &mut work,
        )?;
        // verify_plan has just checked every collection root/count under the
        // exclusive mutable planner borrow. Do not repeat that whole closure
        // check through the independently callable public material scan.
        let page = material_page(planner, "header", None, limits, deadline, cancelled)?;
        if page.len() != 1 {
            return Err(Error::Invalid("philosophy projection header closure"));
        }
        let header = object(&page[0].payload, limits.max_raw_row_bytes)?;
        let arrays = [
            "nodes",
            "edges",
            "views",
            "clusters",
            "review_packets",
            "unresolved_review_surfaces",
        ];
        let fields = header
            .as_object()
            .expect("header")
            .keys()
            .cloned()
            .chain(arrays.iter().map(|s| s.to_string()))
            .collect::<BTreeSet<_>>();
        let mut hash = Digest256Hasher::new();
        let mut output = 0usize;
        let mut emit = |raw: &[u8]| -> Result<()> {
            check(deadline, cancelled)?;
            output = output
                .checked_add(raw.len())
                .ok_or(Error::Budget("philosophy projection output"))?;
            if output > limits.graph.max_material_bytes {
                return Err(Error::Budget("philosophy projection output"));
            }
            hash.update(raw);
            sink(raw)
        };
        emit(b"{")?;
        for (position, key) in fields.iter().enumerate() {
            if position > 0 {
                emit(b",")?;
            }
            emit(&bytes(&json!(key), 8192)?)?;
            emit(b":")?;
            if arrays.contains(&key.as_str()) {
                emit(b"[")?;
                let mut after = -1i64;
                let mut first = true;
                loop {
                    check(deadline, cancelled)?;
                    let batch:Vec<(i64,Vec<u8>,Vec<u8>)>=planner.with_connection(WritePhase::Sort,|db|{let mut q=db.prepare("SELECT ordinal,CASE WHEN length(payload)<=?3 THEN payload ELSE NULL END,payload_sha256 FROM source_philosophy_rows WHERE collection=?1 AND ordinal>?2 ORDER BY ordinal LIMIT ?4")?;let rows=q.query_map(params![key,after,limits.max_raw_row_bytes as i64,limits.max_page_rows as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;rows.collect::<std::result::Result<Vec<_>,_>>().map_err(Error::from)})?;
                    if batch.is_empty() {
                        break;
                    }
                    for (ordinal, raw, sha) in batch {
                        if sha.as_slice() != Digest256::of_bytes(&raw).as_bytes() {
                            return Err(Error::Invalid("philosophy projection ordered row digest"));
                        }
                        charge(&mut work, raw.len(), limits)?;
                        if !first {
                            emit(b",")?;
                        }
                        emit(&raw)?;
                        first = false;
                        after = ordinal;
                    }
                }
                emit(b"]")?;
            } else {
                emit(&bytes(&header[key], limits.max_raw_row_bytes)?)?;
            }
        }
        emit(b"}\n")?;
        drop(emit);
        Ok(hash.finalize().to_hex())
    })();
    if result.is_err() {
        planner.poison();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicU64};

    #[test]
    fn philosophy_material_continuation_seeks_within_one_vm_budget() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE source_philosophy_rows(collection TEXT NOT NULL,id TEXT NOT NULL,ordinal INTEGER NOT NULL,payload BLOB NOT NULL,payload_sha256 BLOB NOT NULL,PRIMARY KEY(collection,id)) WITHOUT ROWID;CREATE UNIQUE INDEX source_philosophy_order ON source_philosophy_rows(collection,ordinal);").unwrap();
        let raw = b"{}";
        let sha = Digest256::of_bytes(raw);
        for ordinal in 0..256 {
            db.execute(
                "INSERT INTO source_philosophy_rows VALUES(?1,?2,?3,?4,?5)",
                params![
                    "nodes",
                    format!("node:{ordinal:03}"),
                    ordinal,
                    raw.as_slice(),
                    sha.as_bytes().as_slice()
                ],
            )
            .unwrap();
        }
        // Both pages and EOF share this counter. The old nullable cursor
        // predicate walks the late prefix and cannot complete within this cap.
        let used = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&used);
        db.progress_handler(
            1,
            Some(move || counter.fetch_add(1, Ordering::Relaxed) >= 512),
        );
        let limits = PhilosophySourceLimits {
            max_page_rows: 8,
            max_raw_row_bytes: 1024,
            max_page_bytes: 8192,
            ..PhilosophySourceLimits::default()
        };
        let initial = row_page(&db, "nodes", None, limits).unwrap();
        let continued = row_page(&db, "nodes", Some("node:247"), limits).unwrap();
        assert_eq!(initial.len(), 8);
        assert_eq!(initial[0].0, "node:000");
        assert_eq!(continued.len(), 8);
        assert_eq!(continued[0].0, "node:248");
        assert_eq!(continued[7].0, "node:255");
        assert!(
            continued
                .iter()
                .all(|(_, payload, digest)| payload.as_slice() == raw
                    && digest.as_slice() == sha.as_bytes())
        );
        assert!(
            row_page(&db, "nodes", Some("node:255"), limits)
                .unwrap()
                .is_empty()
        );
        assert!(used.load(Ordering::Relaxed) < 512);
        db.progress_handler(0, None::<fn() -> bool>);
        for (name, sql) in [
            ("initial", INITIAL_ROW_PAGE),
            ("continued", CONTINUED_ROW_PAGE),
        ] {
            let mut statement = db.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let plans = statement
                .query_map(params!["nodes", "node:247", 1024, 8], |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            eprintln!("phi {name} query plan: {plans:?}");
        }
        eprintln!(
            "phi initial+late continuation+EOF VM steps: {} / 512",
            used.load(Ordering::Relaxed)
        );
        assert!(
            row_page(
                &db,
                "nodes",
                None,
                PhilosophySourceLimits {
                    max_raw_row_bytes: 1,
                    ..limits
                }
            )
            .is_err()
        );
    }
}
