//! Native, unselected source-witness catalog candidates from an exact owner cut.
//!
//! Input is authored metadata and physical JSONL files, never a normalized
//! graph. The stage owner seals membership; the pinned VAL worker checks exact
//! local schemas. Neither a worker verdict nor this catalog grants admission.
//! Original source bytes remain in `raw_records` for bibliographic continuation.

use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{Error, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::cell::{RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, SourceRevision,
    canonical_raw_bytes_v1,
};
use tos_source_store::CorpusCutReader;
use tos_validation::executor::{
    BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget, VerifiedWorkerImageHandle,
};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor};
use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};

pub const CATALOG_SOURCE: &str = "source-witness-catalog";
pub const SOURCE_FILES: &str = "source-files";
pub const CONTRACT_FILES: &str = "contracts";
/// Optional exact source-owned evidence/forms cut for the bibliographic producer.
pub const BIBLIOGRAPHIC_FILES: &str = "bibliographic-dependencies";
pub const NATIVE_IDENTITIES: &str = "native-identity-packets";
pub const NATIVE_TEXT: &str = "native-text-bindings";
pub(crate) fn input_role(name: &str) -> Result<(&'static str, &'static str)> {
    match name {
        SOURCE_FILES => Ok((
            "authored-source-files",
            "tos.source-catalog.source-files.v1",
        )),
        CONTRACT_FILES => Ok(("source-contracts", "tos.source-catalog.contracts.v1")),
        NATIVE_IDENTITIES => Ok((
            "native-identity-inventory",
            "tos.source-catalog.native-identities.v1",
        )),
        NATIVE_TEXT => Ok((
            "native-text-dependencies",
            "tos.source-catalog.native-text.v1",
        )),
        BIBLIOGRAPHIC_FILES => Ok((
            "bibliographic-source-dependencies",
            "tos.source-catalog.bibliographic-files.v1",
        )),
        _ => Err(Error::Invalid("source catalog collection name")),
    }
}
const ENTITY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const ENTITY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const RELATION_SCHEMA: &str = "ToS/contracts/semantic-relation-type-registry.schema.json";
const CORPUS: &str = "ToS/contracts/corpus-record.schema.json";
const CLAIM: &str = "ToS/contracts/claim-packet.schema.json";
const ROOT: &str = "ToS/source-witnesses/catalog/";
const BASE: &[(&str, &str)] = &[
    ("agent", "agents.jsonl"),
    ("place", "places.jsonl"),
    ("organization", "organizations.jsonl"),
    ("work", "works.jsonl"),
    ("expression", "expressions.jsonl"),
    ("edition", "editions.jsonl"),
    ("collection", "collections.jsonl"),
    ("item", "items.jsonl"),
    ("link", "links.jsonl"),
];
const LEGACY_CLAIMS: &[&str] = &[
    "membership-claims.jsonl",
    "responsibility-claims.jsonl",
    "publication-claims.jsonl",
    "provision-activity-claims.jsonl",
    "work-chronology-claims.jsonl",
    "work-expression-claims.jsonl",
    "expression-edition-claims.jsonl",
    "edition-item-claims.jsonl",
    "expression-derivation-claims.jsonl",
    "object-link-claims.jsonl",
    "historical-claims.jsonl",
];
const LINKS: &[&str] = &[
    "work_ref",
    "expression_claim_refs",
    "responsibility_claim_refs",
    "chronology_claim_refs",
    "embodiment_claim_refs",
    "derivation_claim_refs",
    "embodies_expression_refs",
    "publication_claim_refs",
    "provision_activity_claim_refs",
    "exemplar_claim_refs",
    "collection_ref",
    "membership_claim_refs",
    "item_manifest_ref",
    "association_claim_refs",
];

/// Cold membership uses the maintained producer's exact basenames. Registry
/// schema, mappings and profile semantics are checked by `contracts` before a
/// catalog receipt exists; discovering a path here is no profile admission.
pub(crate) fn source_basenames(entities: &Value) -> Result<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BASE
        .iter()
        .map(|(kind, _)| format!("{kind}.json"))
        .chain(LEGACY_CLAIMS.iter().map(|name| (*name).to_owned()))
        .chain(
            [
                "artifact-witness.json",
                "composite-witness.json",
                "source-claims.jsonl",
            ]
            .map(str::to_owned),
        )
        .collect();
    let entries = array(entities, "types")?;
    if entries.len() > 4096 {
        return Err(Error::Budget("catalog cold record profiles"));
    }
    for entry in entries {
        if let Some(profile) = entry.get("source_record_profile") {
            let kind = text(profile, "record_type")?;
            let basename = text(profile, "source_basename")?;
            if kind.len() > 4096 || basename != format!("{kind}.json") || basename.contains('/') {
                return Err(Error::Invalid("catalog cold record basename"));
            }
            names.insert(basename.to_owned());
        }
    }
    Ok(names)
}

#[derive(Clone, Copy, Debug)]
pub struct SourceCatalogLimits {
    pub max_files: u64,
    pub max_rows: u64,
    pub max_file_bytes: usize,
    pub max_row_bytes: usize,
    pub max_contract_bytes: usize,
    pub max_output_row_bytes: usize,
}
impl SourceCatalogLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_files == 0
            || self.max_rows == 0
            || self.max_file_bytes == 0
            || self.max_file_bytes > 16 * 1024 * 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 1024 * 1024
            || self.max_contract_bytes == 0
            || self.max_contract_bytes > 16 * 1024 * 1024
            || self.max_output_row_bytes == 0
            || self.max_output_row_bytes > 4 * 1024 * 1024
        {
            return Err(Error::Budget("source catalog limits"));
        }
        Ok(())
    }
}

/// Pin the independent native worker, its kernel budgets and cancellation.
/// There is no permissive validator implementation or token-based shortcut.
pub struct SourceCatalogValidator<'a> {
    pub worker: &'a ExactWorkerIdentity,
    pub budget: ExecutorBudget,
    pub cancelled: &'a AtomicBool,
    schemas: RefCell<CutWorkerSchemaExecutor>,
    worker_pin: ExactWorkerIdentity,
    budget_pin: ExecutorBudget,
    deadline: Instant,
}

#[derive(Clone)]
struct Profile {
    descriptor: Value,
    routes: BTreeMap<String, String>,
}
struct Contracts {
    values: BTreeMap<String, Value>,
    resources: Vec<SchemaResource>,
    ids: BTreeMap<String, String>,
    records: Vec<(String, Profile)>,
    claims: BTreeMap<String, Profile>,
}

#[derive(Debug)]
pub struct SourceCatalogReceipt {
    pub record_count: u64,
    pub claim_count: u64,
    pub source_slot_count: u64,
    pub manifest: Value,
    pub file_sha256: BTreeMap<String, String>,
    pub row_root_sha256: String,
    pub input_binding: crate::SourceBinding,
    pub worker_sha256: String,
    // Private seals bind all publicly inspectable summaries to exact inputs.
    input_root: String,
    manifest_sha256: String,
    row_count: u64,
    summary_sha256: String,
}

/// The receiver writes a private candidate only. Calls are streaming and
/// ordered; any failure invalidates the whole candidate. Selection belongs CMD.
pub trait SourceCatalogSink {
    fn begin_file(&mut self, source_ref: &str) -> Result<()>;
    fn file_bytes(&mut self, bytes: &[u8]) -> Result<()>;
    fn end_file(&mut self, source_ref: &str, sha256: &str) -> Result<()>;
    fn addressed_row(&mut self, collection: &str, row: &[u8]) -> Result<()>;
    fn manifest(&mut self, manifest: &Value) -> Result<()>;
}

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("source catalog required string"))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("source catalog required array"))
}
fn json_limits(cap: usize) -> Result<JsonLimits> {
    JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("source catalog JSON limits"))
}
fn canonical(raw: &[u8], cap: usize) -> Result<Vec<u8>> {
    canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::SourceRecordDigestV1,
        json_limits(cap)?,
    )
    .map_err(|e| Error::Source(e.to_string()))
}
fn encode(v: &Value, cap: usize) -> Result<Vec<u8>> {
    // Count before allocating the serialized row.
    struct Count {
        n: usize,
        cap: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.n = self
                .n
                .checked_add(b.len())
                .filter(|n| *n <= self.cap)
                .ok_or_else(|| std::io::Error::other("catalog row cap"))?;
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(&mut Count { n: 0, cap }, v)
        .map_err(|_| Error::Budget("source catalog output row"))?;
    let raw = serde_json::to_vec(v).map_err(|_| Error::Invalid("catalog JSON encode"))?;
    canonical(&raw, cap)
}
pub(crate) fn source_ref(ref_: &str) -> Result<&str> {
    let parts = ref_.split('/').collect::<Vec<_>>();
    if ref_.len() > 4096
        || parts.len() < 4
        || parts[..2] != ["ToS", "source-witnesses"]
        || ref_.contains(['\\', '\0'])
        || parts.iter().any(|p| {
            p.is_empty()
                || p.starts_with('.')
                || [
                    "catalog",
                    "payload",
                    "private",
                    "owner-local",
                    "local-content",
                ]
                .contains(p)
        })
    {
        return Err(Error::Invalid("catalog public source locator"));
    }
    parts
        .last()
        .copied()
        .ok_or(Error::Invalid("catalog source filename"))
}
fn identity(id: &str, kind: &str) -> Result<()> {
    let prefix = format!("tos.{kind}.");
    let suffix = id
        .strip_prefix(&prefix)
        .ok_or(Error::Invalid("catalog stable identity family"))?;
    if id.len() > 4096
        || suffix.is_empty()
        || suffix.split(['.', '-']).any(|s| {
            s.is_empty()
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
    {
        return Err(Error::Invalid("catalog stable identity grammar"));
    }
    Ok(())
}
fn version(v: &Value, key: &str) -> Result<u64> {
    v.get(key)
        .and_then(Value::as_u64)
        .filter(|n| (1..=9_007_199_254_740_991).contains(n))
        .ok_or(Error::Invalid("catalog source version"))
}
fn public(v: &Value) -> Result<()> {
    if !matches!(
        v.get("visibility").and_then(Value::as_str),
        Some("public" | "public_metadata_only")
    ) {
        return Err(Error::Invalid("catalog source visibility"));
    }
    Ok(())
}
fn root_item(h: &mut Digest256Hasher, id: &str, sha: &Digest256) {
    h.update(&(id.len() as u64).to_be_bytes());
    h.update(id.as_bytes());
    h.update(sha.as_bytes());
}
fn binding_value(b: &crate::SourceBinding) -> Value {
    json!({"owner_profile":b.owner_profile,"source_cut":b.source_cut,"through_commit_seq":b.through_commit_seq,
        "membership_root":b.membership_root,"index_generation":b.index_generation,"route_map_version":b.route_map_version,
        "reader_abi":b.reader_abi,"projection_root_sha256":b.projection_root_sha256,"complete":b.complete})
}
fn summary(receipt: &SourceCatalogReceipt, l: SourceCatalogLimits) -> Result<String> {
    let raw = encode(
        &json!({"records":receipt.record_count,"claims":receipt.claim_count,
        "slots":receipt.source_slot_count,"files":receipt.file_sha256,"rows":receipt.row_count,
        "row_root":receipt.row_root_sha256,"worker":receipt.worker_sha256,
        "binding":binding_value(&receipt.input_binding),"manifest":receipt.manifest_sha256,
        "inputs":receipt.input_root}),
        l.max_output_row_bytes,
    )?;
    Ok(Digest256::of_bytes(&raw).to_hex())
}

/// Scan actual capped bytes, not just stored digest metadata, before trusting a
/// cut. Exact receipts also prevent skipped over-budget rows from looking empty.
fn input_root(stage: &KnowledgeStage<'_>, l: SourceCatalogLimits) -> Result<String> {
    let receipt = stage.exact_receipt();
    let mut complete = Digest256Hasher::new();
    if !matches!(receipt.collections.len(), 4 | 5) {
        return Err(Error::Invalid("source catalog input collection closure"));
    }
    let mut names = vec![SOURCE_FILES, CONTRACT_FILES, NATIVE_IDENTITIES, NATIVE_TEXT];
    if receipt.collections.len() == 5 {
        names.push(BIBLIOGRAPHIC_FILES);
    }
    for name in names {
        let entry = receipt
            .collections
            .iter()
            .find(|e| e.source_graph == CATALOG_SOURCE && e.collection == name)
            .ok_or(Error::Invalid(
                "source catalog exact collection registration",
            ))?;
        let (role, profile) = input_role(name)?;
        if entry.input_role != role
            || entry.adapter_profile != profile
            || entry.expected_count > l.max_files
        {
            return Err(Error::Invalid("source catalog input role/profile/count"));
        }
        let mut count = 0;
        let mut hash = Digest256Hasher::new();
        let mut after = None;
        loop {
            let page = stage.scan_input(CATALOG_SOURCE, name, after.as_deref(), 1)?;
            for row in page.rows {
                if row.payload.len() > l.max_file_bytes {
                    return Err(Error::Budget("catalog source file"));
                }
                let sha = Digest256::of_bytes(&row.payload);
                root_item(&mut hash, &row.id, &sha);
                count += 1;
                if count > l.max_files {
                    return Err(Error::Budget("catalog files"));
                }
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        if count != entry.expected_count || hash.finalize().to_hex() != entry.expected_root_sha256 {
            return Err(Error::Invalid("source catalog raw input count/root"));
        }
        root_item(
            &mut complete,
            name,
            &Digest256::from_hex(&entry.expected_root_sha256)
                .map_err(|_| Error::Invalid("catalog input digest"))?,
        );
    }
    let binding = encode(&binding_value(&receipt.binding), l.max_output_row_bytes)?;
    complete.update(&binding);
    Ok(complete.finalize().to_hex())
}

impl<'a> SourceCatalogValidator<'a> {
    /// Prepare one exact cut/worker image and bounded isolated operation.
    /// Supplied bytes remain distinct from decoded
    /// worker instances and neither receipt grants source admission.
    pub fn from_cut(
        cut: &CorpusCutReader,
        worker: &'a ExactWorkerIdentity,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        operation: BatchStreamBudget,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        Self::from_cut_inner(
            cut, worker, None, budget, limits, operation, deadline, cancelled,
        )
    }

    /// Prepare the catalog operation from a previously admitted immutable
    /// worker image. Image admission can have its own bounded allowance while
    /// each schema adapter retains its original execution budget.
    pub fn from_cut_with_image(
        cut: &CorpusCutReader,
        image: &'a VerifiedWorkerImageHandle,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        operation: BatchStreamBudget,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        let deadline = deadline.min(image.operation_deadline());
        Self::from_cut_inner(
            cut,
            image.identity(),
            Some(image),
            budget,
            limits,
            operation,
            deadline,
            cancelled,
        )
    }

    fn from_cut_inner(
        cut: &CorpusCutReader,
        worker: &'a ExactWorkerIdentity,
        image: Option<&'a VerifiedWorkerImageHandle>,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        operation: BatchStreamBudget,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        let mut schemas = match image {
            Some(image) => CutWorkerSchemaExecutor::from_cut_with_image(
                cut,
                FormatProfile::LegacyPythonObserved20260923,
                image,
                budget,
                limits,
                deadline,
                cancelled,
            ),
            None => CutWorkerSchemaExecutor::from_cut(
                cut,
                FormatProfile::LegacyPythonObserved20260923,
                worker.clone(),
                budget,
                limits,
                deadline,
                cancelled,
            ),
        }
        .map_err(|e| Error::Source(format!("catalog exact cut executor:{e:?}")))?;
        schemas
            .set_operation_budget(operation)
            .map_err(|e| Error::Source(format!("catalog schema operation budget:{e:?}")))?;
        Ok(Self {
            worker,
            budget,
            cancelled,
            schemas: RefCell::new(schemas),
            worker_pin: worker.clone(),
            budget_pin: budget,
            deadline,
        })
    }

    /// Close the shared schema operation before returning successful owner output.
    /// Call this once after all catalog/navigation phases sharing this executor.
    pub fn finish(&self) -> Result<()> {
        self.guard()?;
        self.schemas
            .try_borrow_mut()
            .map_err(|_| Error::Invalid("catalog executor already in use"))?
            .finish(self.deadline, self.cancelled)
            .map_err(|e| Error::Source(format!("catalog schema operation finish:{e:?}")))
    }

    fn guard(&self) -> Result<()> {
        if self.worker.sha256 != self.worker_pin.sha256
            || self.worker.absolute_path != self.worker_pin.absolute_path
            || self.budget.execution_wall != self.budget_pin.execution_wall
            || self.budget.cleanup_grace != self.budget_pin.cleanup_grace
            || self.budget.cpu_seconds != self.budget_pin.cpu_seconds
            || self.budget.address_space_bytes != self.budget_pin.address_space_bytes
        {
            return Err(Error::Invalid("catalog prepared worker/budget pin changed"));
        }
        Ok(())
    }

    pub(crate) fn schemas(
        &self,
        revision: SourceRevision,
    ) -> Result<RefMut<'_, CutWorkerSchemaExecutor>> {
        self.guard()?;
        let schemas = self
            .schemas
            .try_borrow_mut()
            .map_err(|_| Error::Invalid("catalog executor already in use"))?;
        let binding = schemas.execution_binding();
        if binding.source_revision != revision
            || binding.worker_sha256 != self.worker_pin.sha256
            || binding.schema_profile != FormatProfile::LegacyPythonObserved20260923
        {
            return Err(Error::Invalid("catalog exact cut execution binding"));
        }
        Ok(schemas)
    }

    fn bind_contracts(&self, c: &Contracts) -> Result<()> {
        self.guard()?;
        let resources = SchemaBackendProbe::new(
            c.resources.clone(),
            FormatProfile::LegacyPythonObserved20260923,
        )
        .map_err(|e| Error::Source(format!("catalog selected schema inventory:{e:?}")))?;
        let schemas = self
            .schemas
            .try_borrow()
            .map_err(|_| Error::Invalid("catalog executor already in use"))?;
        if resources.schema_set_digest() != schemas.execution_binding().schema_set_sha256 {
            return Err(Error::Invalid("catalog stage/cut schema closure differs"));
        }
        Ok(())
    }

    fn check(&self, c: &Contracts, schema_ref: &str, fragment: &str, raw: &[u8]) -> Result<()> {
        // This helper also checks derived field/form instances. A diagnostic
        // digest address identifies supplied bytes without calling it an
        // authored member or using the schema filename as a source address.
        let instance = format!(
            "catalog-generated-instance/{}",
            Digest256::of_bytes(raw).to_hex()
        );
        self.check_at(c, &instance, schema_ref, fragment, raw)
    }

    fn check_at(
        &self,
        c: &Contracts,
        path: &str,
        schema_ref: &str,
        fragment: &str,
        raw: &[u8],
    ) -> Result<()> {
        self.guard()?;
        let _ = profile_schema_uri(c, schema_ref)?;
        let contract = format!("{schema_ref}{fragment}");
        let mut schemas = self
            .schemas
            .try_borrow_mut()
            .map_err(|_| Error::Invalid("catalog executor already in use"))?;
        let valid = schemas.check(path, raw, &contract, self.deadline, self.cancelled)
            .map_err(|e| Error::Source(format!("catalog schema execution incomplete: path={path}; contract={contract}; reason={e:?}")))?;
        if !valid {
            return Err(Error::Invalid(
                "source catalog exact native schema rejected",
            ));
        }
        Ok(())
    }
}

// source_record_profiles._schema_route binds consumed profile resources to
// their declared owner path. The complete selected contracts inventory also
// contains schemas with other genuine declared $ids; their identity is the
// exact selected bytes and $id, checked by the schema worker.
fn profile_schema_uri<'a>(c: &'a Contracts, schema_ref: &str) -> Result<&'a str> {
    let uri = c
        .ids
        .get(schema_ref)
        .ok_or(Error::Invalid("catalog exact schema absent"))?;
    if uri != &format!("https://tree-of-sophia.local/{schema_ref}")
        && uri != &format!("https://treeofsophia.local/{schema_ref}")
    {
        return Err(Error::Invalid("catalog schema owner identity"));
    }
    Ok(uri)
}

fn routes(profile: &Value, c: &Contracts) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for route in array(profile, "schemas")? {
        let schema = text(route, "schema_ref")?;
        profile_schema_uri(c, schema)?;
        for dependency in array(route, "schema_dependencies")? {
            profile_schema_uri(
                c,
                dependency
                    .as_str()
                    .ok_or(Error::Invalid("catalog schema dependency"))?,
            )?;
        }
        if result
            .insert(text(route, "schema_version")?.into(), schema.into())
            .is_some()
        {
            return Err(Error::Invalid("catalog duplicate schema-version route"));
        }
    }
    if result.is_empty() {
        return Err(Error::Invalid("catalog empty schema routes"));
    }
    Ok(result)
}
fn ancestry(types: &BTreeMap<String, &Value>, id: &str) -> Result<BTreeSet<String>> {
    let mut visited = BTreeSet::new();
    let mut pending = vec![id.to_owned()];
    while let Some(next) = pending.pop() {
        if !visited.insert(next.clone()) {
            continue;
        }
        if visited.len() > 4096 {
            return Err(Error::Budget("catalog type ancestry"));
        }
        let node = types
            .get(&next)
            .ok_or(Error::Invalid("catalog type parent missing"))?;
        for parent in array(node, "parent_type_ids")? {
            pending.push(
                parent
                    .as_str()
                    .ok_or(Error::Invalid("catalog type parent"))?
                    .into(),
            );
        }
    }
    Ok(visited)
}

fn contracts(
    stage: &KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: SourceCatalogLimits,
) -> Result<Contracts> {
    let mut c = Contracts {
        values: BTreeMap::new(),
        resources: Vec::new(),
        ids: BTreeMap::new(),
        records: Vec::new(),
        claims: BTreeMap::new(),
    };
    let mut after = None;
    let mut total = 0usize;
    let mut uri_seen = BTreeSet::new();
    loop {
        let page = stage.scan_input(CATALOG_SOURCE, CONTRACT_FILES, after.as_deref(), 1)?;
        for row in page.rows {
            if row.id.len() > 4096
                || row.id.contains(['\\', '\0'])
                || row
                    .id
                    .split('/')
                    .any(|s| s.is_empty() || s.starts_with('.'))
                || !(row.id.starts_with("ToS/contracts/") || row.id == ENTITY || row.id == RELATION)
            {
                return Err(Error::Invalid("catalog contract owner locator"));
            }
            total = total
                .checked_add(row.payload.len())
                .filter(|n| *n <= l.max_contract_bytes)
                .ok_or(Error::Budget("catalog contract closure bytes"))?;
            let value = SourceRow::parse(&row.payload, l.max_row_bytes)?
                .value()
                .clone();
            if row.id.ends_with(".schema.json") {
                let uri = text(&value, "$id")?.to_owned();
                if !uri_seen.insert(uri.clone()) {
                    return Err(Error::Invalid("catalog duplicate schema owner URI"));
                }
                c.ids.insert(row.id.clone(), uri.clone());
                c.resources.push(SchemaResource {
                    uri,
                    raw: row.payload,
                });
            }
            c.values.insert(row.id, value);
        }
        after = page.next_id;
        if after.is_none() {
            break;
        }
    }
    validator.bind_contracts(&c)?;
    for (ref_, schema) in [(ENTITY, ENTITY_SCHEMA), (RELATION, RELATION_SCHEMA)] {
        let value = c
            .values
            .get(ref_)
            .ok_or(Error::Invalid("catalog source registry missing"))?;
        validator.check_at(
            &c,
            &format!("{ref_}#catalog-decoded"),
            schema,
            "",
            &encode(value, l.max_row_bytes)?,
        )?;
    }
    let entity = c
        .values
        .get(ENTITY)
        .ok_or(Error::Invalid("catalog entity registry"))?;
    let entries = array(entity, "types")?;
    if entries.len() > 4096 {
        return Err(Error::Budget("catalog entity profiles"));
    }
    let mut types = BTreeMap::new();
    let mut mappings = BTreeMap::new();
    for entry in entries {
        if types
            .insert(text(entry, "type_id")?.into(), entry)
            .is_some()
        {
            return Err(Error::Invalid("catalog duplicate type identity"));
        }
        for m in array(entry, "source_mappings")? {
            if matches!(
                m.get("source_graph").and_then(Value::as_str),
                Some("source-claims" | "source-navigation")
            ) {
                let key = (
                    text(m, "source_graph")?.to_owned(),
                    text(m, "source_kind_id")?.to_owned(),
                );
                if mappings.insert(key, text(entry, "type_id")?).is_some() {
                    return Err(Error::Invalid("catalog duplicate source kind mapping"));
                }
            }
        }
    }
    let mut names = BTreeSet::new();
    let mut files = BTreeSet::new();
    for entry in entries {
        let Some(profile) = entry.get("source_record_profile") else {
            continue;
        };
        let kind = text(profile, "record_type")?;
        let type_id = text(entry, "type_id")?;
        let reader = text(profile, "reader")?;
        let (role, family) = match reader {
            "corpus-metadata-v1" => ("identity", "tos.entity.identity"),
            "semantic-metadata-v1" => ("semantic", "tos.entity.semantic-object"),
            _ => return Err(Error::Invalid("catalog source record reader")),
        };
        let retained = kind == "composite"
            && type_id == "tos.entity.composite"
            && reader == "corpus-metadata-v1"
            && profile
                .get("retained_native_adapter")
                .and_then(Value::as_str)
                == Some("scholarly-composite-v1")
            && text(profile, "catalog_filename")? == "composites.jsonl";
        if entry.get("abstract") != Some(&json!(false))
            || text(entry, "object_role")? != role
            || type_id == family
            || !ancestry(&types, type_id)?.contains(family)
            || text(profile, "id_prefix")? != format!("tos.{kind}.")
            || text(profile, "source_basename")? != format!("{kind}.json")
            || (!retained
                && (BASE.iter().any(|(k, _)| *k == kind)
                    || ["artifact", "composite"].contains(&kind)))
            || (profile.get("retained_native_adapter").is_some() && !retained)
            || !names.insert(kind.to_owned())
            || !files.insert(text(profile, "catalog_filename")?.to_owned())
        {
            return Err(Error::Invalid(
                "catalog source record profile identity/role/collision",
            ));
        }
        for graph in ["source-claims", "source-navigation"] {
            if mappings.get(&(graph.to_owned(), kind.to_owned())) != Some(&type_id) {
                return Err(Error::Invalid(
                    "catalog source profile exact reader mapping",
                ));
            }
        }
        let filename = text(profile, "catalog_filename")?;
        if filename.contains('/')
            || !filename.ends_with(".jsonl")
            || filename == "claims.jsonl"
            || BASE.iter().any(|(_, f)| *f == filename)
            || filename == "artifacts.jsonl"
        {
            return Err(Error::Invalid("catalog profile output filename collision"));
        }
        let profile = Profile {
            descriptor: profile.clone(),
            routes: routes(profile, &c)?,
        };
        c.records.push((kind.to_owned(), profile));
    }
    let relation = c
        .values
        .get(RELATION)
        .ok_or(Error::Invalid("catalog relation registry"))?;
    let entries = array(relation, "relations")?;
    if entries.len() > 4096 {
        return Err(Error::Budget("catalog relation profiles"));
    }
    let mut predicates = BTreeMap::new();
    let mut relation_ids = BTreeSet::new();
    for entry in entries {
        if !relation_ids.insert(text(entry, "relation_type_id")?) {
            return Err(Error::Invalid("catalog duplicate relation identity"));
        }
        for m in array(entry, "source_mappings")? {
            if m.get("source_graph").and_then(Value::as_str) == Some("source-claims")
                && m.get("scope").and_then(Value::as_str) == Some("claim-predicate")
                && predicates
                    .insert(text(m, "source_predicate_id")?, entry)
                    .is_some()
            {
                return Err(Error::Invalid("catalog duplicate Claim predicate mapping"));
            }
        }
    }
    for (predicate, entry) in predicates {
        let Some(profile) = entry.get("source_claim_profile") else {
            continue;
        };
        if entry.get("abstract") != Some(&json!(false))
            || text(entry, "assertion_mode")? != "reified-claim"
            || entry.get("evidence_required") != Some(&json!(true))
        {
            return Err(Error::Invalid("catalog Claim profile reification/evidence"));
        }
        let mapped = array(entry, "source_mappings")?
            .iter()
            .filter(|m| {
                m.get("source_graph").and_then(Value::as_str) == Some("source-claims")
                    && m.get("scope").and_then(Value::as_str) == Some("claim-predicate")
            })
            .count();
        if mapped != 1 {
            return Err(Error::Invalid("catalog Claim profile predicate mapping"));
        }
        c.claims.insert(
            predicate.to_owned(),
            Profile {
                descriptor: profile.clone(),
                routes: routes(profile, &c)?,
            },
        );
    }
    Ok(c)
}

fn schema_route<'a>(p: &'a Profile, source: &Value) -> Result<&'a str> {
    p.routes
        .get(text(source, "schema_version")?)
        .map(String::as_str)
        .ok_or(Error::Invalid(
            "catalog source profile schema-version route",
        ))
}

fn entry_record(
    stage: &KnowledgeStage<'_>,
    raw: &[u8],
    ref_: &str,
    c: &Contracts,
    validator: &SourceCatalogValidator<'_>,
    l: SourceCatalogLimits,
) -> Result<Value> {
    let basename = source_ref(ref_)?;
    let source = SourceRow::parse(raw, l.max_row_bytes)?;
    let v = source.value();
    let sha = Digest256::of_bytes(&canonical(raw, l.max_row_bytes)?).to_hex();
    let mut schema = None;
    let mut label_pointer = None;
    let (kind, id, label, status) = if basename == "artifact-witness.json" {
        if !ref_.starts_with("ToS/source-witnesses/artifacts/") {
            return Err(Error::Invalid("catalog artifact owner path"));
        }
        let route = match text(v, "schema_version")? {
            "tos_artifact_source_witness_v1" => "ToS/contracts/artifact-source-witness.schema.json",
            "tos_artifact_source_witness_v2" => {
                "ToS/contracts/artifact-source-witness-v2.schema.json"
            }
            _ => return Err(Error::Invalid("catalog physical artifact schema")),
        };
        validator.check_at(c, ref_, route, "", raw)?;
        schema = Some(route);
        label_pointer = Some("/custody/inventory_numbers/0");
        (
            "artifact",
            text(v, "artifact_id")?,
            v.pointer("/custody/inventory_numbers/0")
                .cloned()
                .ok_or(Error::Invalid("catalog inventory label"))?,
            Value::Null,
        )
    } else if basename == "composite-witness.json" {
        if !ref_.starts_with("ToS/source-witnesses/scholarly-composites/")
            || text(v, "schema_version")? != "tos_scholarly_composite_witness_v1"
        {
            return Err(Error::Invalid("catalog scholarly composite owner contract"));
        }
        let route = "ToS/contracts/scholarly-composite-witness.schema.json";
        validator.check_at(c, ref_, route, "", raw)?;
        schema = Some(route);
        (
            "composite",
            text(v, "composite_id")?,
            v["preferred_label"].clone(),
            v["identity_status"].clone(),
        )
    } else {
        let kind = text(v, "record_type")?;
        if basename != format!("{kind}.json") {
            return Err(Error::Invalid("catalog record filename/type"));
        }
        let id = text(v, "record_id")?;
        if let Some((_, p)) = c.records.iter().find(|(k, _)| k == kind) {
            // The maintained profile reader requires explicit public metadata.
            // Native Corpus/Link records use their separate exact schemas;
            // those contracts do not contain a visibility property.
            public(v)?;
            if let Some(adapter) = p.descriptor.get("native_binding_adapter") {
                if adapter != "source-text-unit-v1" {
                    return Err(Error::Invalid(
                        "catalog unknown native text binding adapter",
                    ));
                }
                crate::source_bibliographic_native_text::resolve_native_text_binding(
                    stage,
                    &v["native_text_binding"],
                    validator,
                    l,
                )?;
            } else if v.get("native_text_binding").is_some() {
                return Err(Error::Invalid("catalog undeclared native text binding"));
            }
            let route = schema_route(p, v)?;
            validator.check_at(c, ref_, route, "", raw)?;
            validator.check_at(
                c,
                ref_,
                "ToS/contracts/source-metadata-record.schema.json",
                "",
                raw,
            )?;
            schema = Some(route);
            for field in [
                "preferred_label",
                "variant_labels",
                "field_languages",
                "identity_status",
                "source_refs",
                "external_identifiers",
                "same_as_posture",
                "record_version",
                "notes",
            ] {
                if let Some(value) = v.get(field) {
                    validator.check(
                        c,
                        CORPUS,
                        &format!("#/properties/{field}"),
                        &encode(value, l.max_row_bytes)?,
                    )?;
                }
            }
            for field in [
                "preferred_label",
                "identity_status",
                "source_refs",
                "external_identifiers",
                "same_as_posture",
                "record_version",
            ] {
                if v.get(field).is_none() {
                    return Err(Error::Invalid("catalog common source metadata missing"));
                }
            }
        } else if kind == "link" {
            if !ref_.starts_with("ToS/source-witnesses/links/") {
                return Err(Error::Invalid("catalog native Link owner path"));
            }
            validator.check_at(c, ref_, "ToS/contracts/source-link.schema.json", "", raw)?;
        } else if BASE.iter().any(|(k, _)| *k == kind) {
            validator.check_at(c, ref_, CORPUS, "", raw)?;
        } else {
            return Err(Error::Invalid("catalog undeclared record family"));
        }
        (
            kind,
            id,
            v.get("preferred_label").cloned().unwrap_or(json!("")),
            v.get("identity_status").cloned().unwrap_or(json!("")),
        )
    };
    identity(id, kind)?;
    version(v, "record_version")?;
    // Schema/profile checks retain their scope; the shared renderer owns bytes.
    let _ = (label, status, label_pointer);
    let entry = render_catalog_record(v, ref_, schema, l.max_output_row_bytes)?;
    if entry["record_sha256"] != sha {
        return Err(Error::Invalid("catalog exact numeric source transport"));
    }
    Ok(entry)
}

fn entry_claim(
    raw: &[u8],
    ref_: &str,
    line: u64,
    c: &Contracts,
    validator: &SourceCatalogValidator<'_>,
    l: SourceCatalogLimits,
) -> Result<Value> {
    let basename = source_ref(ref_)?;
    let source = SourceRow::parse(raw, l.max_row_bytes)?;
    let v = source.value();
    public(v)?;
    identity(text(v, "claim_id")?, "claim")?;
    version(v, "claim_version")?;
    let mut extension = None;
    if basename == "source-claims.jsonl" {
        for shared in [
            CLAIM,
            "ToS/contracts/knowledge-assessment.schema.json",
            "ToS/contracts/source-claim-record.schema.json",
        ] {
            profile_schema_uri(c, shared)?;
        }
        let p = c
            .claims
            .get(text(v, "predicate")?)
            .ok_or(Error::Invalid("catalog source Claim profile missing"))?;
        let reader = text(&p.descriptor, "reader")?;
        if !matches!(
            reader,
            "identity-relation-v1"
                | "historical-temporal-v1"
                | "document-catalogue-temporal-v1"
                | "structured-value-v1"
                | "structured-reference-value-v1"
                | "identity-transition-v1"
                | "identity-transition-v2"
        ) {
            return Err(Error::Invalid(
                "catalog Claim owner value adapter not connected",
            ));
        }
        if text(v, "claim_type")? != "relation"
            || !array(&p.descriptor, "assertion_layers")?.contains(&v["assertion_layer"])
            || !v["subject_ref"].is_string()
            || (reader == "identity-relation-v1" && !v["object"].is_string())
            || (reader != "identity-relation-v1" && !v["object"].is_object())
            || v["claim_id"] == v["subject_ref"]
            || v["claim_id"] == v["object"]
        {
            return Err(Error::Invalid(
                "catalog source Claim profile layer/endpoints",
            ));
        }
        let route = schema_route(p, v)?;
        validator.check_at(c, &format!("{ref_}#line={line}"), route, "", raw)?;
        validator.check_at(
            c,
            &format!("{ref_}#line={line}"),
            "ToS/contracts/source-claim-record.schema.json",
            "",
            raw,
        )?;
        extension = Some(route);
        if matches!(
            reader,
            "historical-temporal-v1" | "document-catalogue-temporal-v1"
        ) {
            validator.check(
                c,
                if reader == "historical-temporal-v1" {
                    "ToS/contracts/historical-claim.schema.json"
                } else {
                    "ToS/contracts/document-catalogue-claim.schema.json"
                },
                if reader == "historical-temporal-v1" {
                    "#/$defs/historicalDate"
                } else {
                    "#/$defs/documentDate"
                },
                &encode(&v["object"], l.max_row_bytes)?,
            )?;
        }
        if matches!(
            reader,
            "structured-value-v1"
                | "structured-reference-value-v1"
                | "identity-transition-v1"
                | "identity-transition-v2"
        ) {
            validator.check(
                c,
                "ToS/contracts/source-structured-value.schema.json",
                "",
                &encode(&v["object"], l.max_row_bytes)?,
            )?;
            if v["object"]["kind"] != p.descriptor["value_kind"] {
                return Err(Error::Invalid("catalog declared structured value kind"));
            }
            crate::source_bibliographic_values::members(v, &p.descriptor)?;
            if p.descriptor
                .pointer("/object_reference_set/structure_adapter")
                .and_then(Value::as_str)
                == Some("scoped-members-v1")
            {
                validator.check(
                    c,
                    "ToS/contracts/scoped-member-structure.schema.json",
                    "",
                    &encode(&v["object"], l.max_row_bytes)?,
                )?;
            }
        }
    } else if LEGACY_CLAIMS.contains(&basename) && v["schema_version"] == "tos_historical_claim_v1"
    {
        let route = "ToS/contracts/historical-claim.schema.json";
        validator.check_at(c, &format!("{ref_}#line={line}"), route, "", raw)?;
        extension = Some(route);
    } else if LEGACY_CLAIMS.contains(&basename) {
        // The full legacy collector preserves these packets; base Claim shape
        // is additionally checked by the source-addressed catalog boundary.
        let route = match text(v, "schema_version")? {
            "tos_claim_packet_v1" => CLAIM,
            "tos_object_link_claim_v1" => "ToS/contracts/object-link-claim.schema.json",
            "tos_object_link_claim_v2" => "ToS/contracts/object-link-claim-v2.schema.json",
            _ => {
                return Err(Error::Invalid(
                    "catalog legacy Claim schema adapter not connected",
                ));
            }
        };
        validator.check_at(c, &format!("{ref_}#line={line}"), route, "", raw)?;
    } else {
        return Err(Error::Invalid("catalog Claim source filename"));
    }
    let entry = render_catalog_claim(v, ref_, line, extension, l.max_output_row_bytes)?;
    if entry["claim_sha256"] != Digest256::of_bytes(&canonical(raw, l.max_row_bytes)?).to_hex() {
        return Err(Error::Invalid("catalog exact numeric Claim transport"));
    }
    Ok(entry)
}

fn insert(
    stage: &mut KnowledgeStage<'_>,
    category: &str,
    kind: &str,
    id: &str,
    row: &Value,
    l: SourceCatalogLimits,
) -> Result<()> {
    let raw = encode(row, l.max_output_row_bytes)?;
    let sha = Digest256::of_bytes(&raw);
    stage.charge_materialized(1, raw.len() as u64)?;
    stage.with_connection(WritePhase::Catalog, |db| {
        db.execute("INSERT INTO source_catalog_rows(category,kind,id,payload_len,payload_sha256,payload) VALUES(?1,?2,?3,?4,?5,?6)",
            params![category, kind, id, raw.len() as i64, sha.as_bytes().as_slice(), raw])?;
        Ok(())
    })
}

fn native_inventory(
    stage: &mut KnowledgeStage<'_>,
    c: &Contracts,
    validator: &SourceCatalogValidator<'_>,
    l: SourceCatalogLimits,
) -> Result<()> {
    let mut after = None;
    let mut packet_count = 0usize;
    loop {
        let page = stage.scan_input(CATALOG_SOURCE, NATIVE_IDENTITIES, after.as_deref(), 1)?;
        for row in page.rows {
            packet_count += 1;
            if packet_count > 1024 {
                return Err(Error::Budget("catalog native identity inventory packets"));
            }
            let basename = source_ref(&row.id)?;
            if !basename.starts_with("semantic-annotation") || !basename.ends_with(".json") {
                return Err(Error::Invalid("catalog native identity inventory locator"));
            }
            let packet = SourceRow::parse(&row.payload, l.max_row_bytes)?;
            if text(packet.value(), "schema_version")? != "tos_semantic_annotation_packet_v2" {
                return Err(Error::Invalid("catalog native identity packet schema"));
            }
            validator.check_at(
                c,
                &row.id,
                "ToS/contracts/semantic-annotation-packet-v2.schema.json",
                "",
                &row.payload,
            )?;
            for entity in array(packet.value(), "entities")? {
                let id = text(entity, "entity_id")?;
                if id.len() > 4096 {
                    return Err(Error::Budget("catalog native identity"));
                }
                stage.with_connection(WritePhase::Catalog, |db| {
                    db.execute(
                        "INSERT OR IGNORE INTO source_catalog_reserved(id) VALUES(?1)",
                        [id],
                    )?;
                    Ok(())
                })?;
            }
        }
        after = page.next_id;
        if after.is_none() {
            break;
        }
    }
    // Native text closure is validated from each exact declared description,
    // metadata only; unused sealed dependencies acquire no graph/rights role.

    Ok(())
}

fn source_files(
    stage: &mut KnowledgeStage<'_>,
    c: &Contracts,
    validator: &SourceCatalogValidator<'_>,
    l: SourceCatalogLimits,
) -> Result<()> {
    let mut after = None;
    let mut count = 0u64;
    loop {
        let page = stage.scan_input(CATALOG_SOURCE, SOURCE_FILES, after.as_deref(), 1)?;
        for file in page.rows {
            let basename = source_ref(&file.id)?;
            let slot_kind =
                if LEGACY_CLAIMS.contains(&basename) || basename == "source-claims.jsonl" {
                    Some("claim")
                } else if basename.ends_with(".jsonl") && basename.contains("provenance") {
                    Some("provenance_event")
                } else if basename.ends_with(".jsonl") && basename.contains("anchor") {
                    Some("anchor")
                } else {
                    None
                };
            if let Some(kind) = slot_kind {
                let file_sha = Digest256::of_bytes(&file.payload).to_hex();
                let mut offset = 0usize;
                let mut line = 0u64;
                while offset < file.payload.len() {
                    line += 1;
                    let start = offset;
                    while offset < file.payload.len()
                        && !matches!(file.payload[offset], b'\r' | b'\n')
                    {
                        offset += 1;
                    }
                    let end = offset;
                    let delimiter = if offset == file.payload.len() {
                        "eof"
                    } else if file.payload[offset] == b'\n' {
                        offset += 1;
                        "lf"
                    } else if file.payload.get(offset + 1) == Some(&b'\n') {
                        offset += 2;
                        "crlf"
                    } else {
                        offset += 1;
                        "cr"
                    };
                    let raw = &file.payload[start..end];
                    let string = std::str::from_utf8(raw)
                        .map_err(|_| Error::Invalid("catalog JSONL UTF8"))?;
                    // Python splitlines includes these nonphysical separators;
                    // the addressed producer rejects that ambiguous inventory.
                    if string.contains([
                        '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
                        '\u{2029}',
                    ]) {
                        return Err(Error::Invalid("catalog nonphysical JSONL separator"));
                    }
                    if string.trim().is_empty() {
                        continue;
                    }
                    count = count
                        .checked_add(1)
                        .filter(|n| *n <= l.max_rows)
                        .ok_or(Error::Budget("catalog source rows"))?;
                    let v = SourceRow::parse(raw, l.max_row_bytes)?;
                    let field = match kind {
                        "claim" => "claim_id",
                        "provenance_event" => "event_id",
                        _ => "anchor_id",
                    };
                    let Some(id) = v.value().get(field).and_then(Value::as_str) else {
                        if kind == "claim" {
                            return Err(Error::Invalid("catalog Claim slot identity"));
                        }
                        continue;
                    };
                    if id.is_empty() || id.len() > 4096 {
                        return Err(Error::Invalid("catalog source slot identity"));
                    }
                    if kind == "provenance_event"
                        && v.value()["schema_version"] == "tos_provenance_event_v2"
                        && !matches!(
                            v.value()
                                .pointer("/rights_and_visibility/content_visibility")
                                .and_then(Value::as_str),
                            Some("tracked_public_metadata" | "public_content" | "public_synthetic")
                        )
                    {
                        return Err(Error::Invalid("catalog provenance visibility"));
                    }
                    let key =
                        String::from_utf8(encode(&json!([kind, id]), l.max_output_row_bytes)?)
                            .map_err(|_| Error::Invalid("catalog slot key UTF8"))?;
                    let canonical_sha =
                        Digest256::of_bytes(&canonical(raw, l.max_row_bytes)?).to_hex();
                    let slot = json!({"source_slot_key":key,"kind":kind,"identity":id,"source":{
                        "source_ref":file.id,"source_line":line,"byte_offset":start,"row_bytes":raw.len(),
                        "raw_row_sha256":Digest256::of_bytes(raw).to_hex(),"delimiter":delimiter,
                        "file_sha256":file_sha,"file_bytes":file.payload.len(),"canonical_sha256":canonical_sha}});
                    insert(stage, "slots", kind, &key, &slot, l)?;
                    if kind == "claim" {
                        let entry = entry_claim(raw, &file.id, line, c, validator, l)?;
                        let addressed = json!({"claim_id":id,"entry":entry,"source_slot_key":key,
                            "claim_ref":{"id":id,"version":version(v.value(),"claim_version")?,"digest":format!("sha256:{canonical_sha}")}});
                        insert(stage, "claims", "claim", id, &addressed, l)?;
                    }
                }
            } else {
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= l.max_rows)
                    .ok_or(Error::Budget("catalog source rows"))?;
                let entry = entry_record(stage, &file.payload, &file.id, c, validator, l)?;
                let id = text(&entry, "record_id")?;
                let kind = text(&entry, "record_type")?;
                let reserved = stage.with_connection(WritePhase::Catalog, |db| {
                    Ok(db.query_row(
                        "SELECT EXISTS(SELECT 1 FROM source_catalog_reserved WHERE id=?1)",
                        [id],
                        |r| r.get::<_, bool>(0),
                    )?)
                })?;
                if reserved {
                    return Err(Error::Invalid(
                        "catalog identity already owned by native semantic packet",
                    ));
                }
                let source = SourceRow::parse(&file.payload, l.max_row_bytes)?;
                let addressed = json!({"record_id":id,"entry":entry,"source":{
                    "source_ref":file.id,"raw_sha256":file.payload_sha256,"raw_bytes":file.payload.len(),
                    "record_ref":{"id":id,"version":version(source.value(),"record_version")?,"digest":format!("sha256:{}",entry["record_sha256"].as_str().ok_or(Error::Invalid("catalog source digest"))?)}}});
                insert(stage, "records", kind, id, &addressed, l)?;
            }
        }
        after = page.next_id;
        if after.is_none() {
            break;
        }
    }
    Ok(())
}

/// Visit verified, bounded stage rows in one exact category/kind order.
fn visit_rows(
    stage: &mut KnowledgeStage<'_>,
    category: &str,
    kind: Option<&str>,
    l: SourceCatalogLimits,
    mut visit: impl FnMut(&str, &[u8], &Value) -> Result<()>,
) -> Result<u64> {
    stage.with_connection(WritePhase::Catalog, |db| {
        let mut statement = db.prepare("SELECT CASE WHEN length(CAST(id AS BLOB))<=8192 THEN id ELSE NULL END,
            CASE WHEN length(payload)<=?3 AND payload_len=length(payload) THEN payload ELSE NULL END,payload_sha256
            FROM source_catalog_rows WHERE category=?1 AND (?2 IS NULL OR kind=?2) ORDER BY id")?;
        let mut rows = statement.query(params![category, kind, l.max_output_row_bytes as i64])?;
        let mut count = 0;
        while let Some(row) = rows.next()? {
            let id: Option<String> = row.get(0)?; let raw: Option<Vec<u8>> = row.get(1)?;
            let sha: Vec<u8> = row.get(2)?;
            let (Some(id), Some(raw)) = (id, raw) else { return Err(Error::Budget("catalog retained row bytes")); };
            if Digest256::of_bytes(&raw).as_bytes().as_slice() != sha { return Err(Error::Invalid("catalog retained row digest")); }
            let v = SourceRow::parse(&raw, l.max_output_row_bytes.min(8 * 1024 * 1024))?;
            visit(&id, &raw, v.value())?; count += 1;
            if count > l.max_rows { return Err(Error::Budget("catalog retained rows")); }
        }
        Ok(count)
    })
}

fn row_root(stage: &mut KnowledgeStage<'_>, l: SourceCatalogLimits) -> Result<(u64, String)> {
    let mut hash = Digest256Hasher::new();
    let mut total = 0;
    for category in ["records", "claims", "slots"] {
        hash.update(category.as_bytes());
        hash.update(b"\0");
        total += visit_rows(stage, category, None, l, |id, raw, v| {
            let field = match category {
                "records" => "record_id",
                "claims" => "claim_id",
                _ => "source_slot_key",
            };
            if text(v, field)? != id {
                return Err(Error::Invalid("catalog retained identity binding"));
            }
            root_item(&mut hash, id, &Digest256::of_bytes(raw));
            Ok(())
        })?;
    }
    Ok((total, hash.finalize().to_hex()))
}

fn outputs(
    stage: &mut KnowledgeStage<'_>,
    c: &Contracts,
    l: SourceCatalogLimits,
) -> Result<(Value, BTreeMap<String, String>, u64, u64)> {
    let mut families = BASE
        .iter()
        .map(|(k, f)| (k.to_string(), f.to_string()))
        .collect::<Vec<_>>();
    for (kind, p) in &c.records {
        let present = stage.with_connection(WritePhase::Catalog, |db| {
            Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM source_catalog_rows WHERE category='records' AND kind=?1)", [kind], |r| r.get::<_, bool>(0))?)
        })?;
        if present {
            families.push((
                kind.clone(),
                text(&p.descriptor, "catalog_filename")?.into(),
            ));
        }
    }
    for (kind, filename) in [
        ("artifact", "artifacts.jsonl"),
        ("composite", "composites.jsonl"),
    ] {
        let present = stage.with_connection(WritePhase::Catalog, |db| {
            Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM source_catalog_rows WHERE category='records' AND kind=?1)", [kind], |r| r.get::<_, bool>(0))?)
        })?;
        if present && !families.iter().any(|(k, _)| k == kind) {
            families.push((kind.into(), filename.into()));
        }
    }
    let mut catalog_hash = Digest256Hasher::new();
    let mut file_sha = BTreeMap::new();
    let mut counts = Map::new();
    let mut record_files = Map::new();
    let mut extensions = BTreeSet::new();
    let mut record_count = 0;
    for (kind, filename) in &families {
        let mut file_hash = Digest256Hasher::new();
        catalog_hash.update(kind.as_bytes());
        catalog_hash.update(b"\0");
        let count = visit_rows(stage, "records", Some(kind), l, |_, _, row| {
            let entry = &row["entry"];
            let raw = encode(entry, l.max_output_row_bytes)?;
            file_hash.update(&raw);
            file_hash.update(b"\n");
            catalog_hash.update(&raw);
            catalog_hash.update(b"\n");
            if let Some(schema) = entry.get("source_schema_ref").and_then(Value::as_str) {
                extensions.insert(schema.to_owned());
            }
            Ok(())
        })?;
        record_count += count;
        counts.insert(kind.clone(), json!(count));
        let ref_ = format!("{ROOT}{filename}");
        record_files.insert(kind.clone(), json!(ref_));
        file_sha.insert(ref_, file_hash.finalize().to_hex());
    }
    catalog_hash.update(b"claim\0");
    let mut claim_hash = Digest256Hasher::new();
    let claim_count = visit_rows(stage, "claims", None, l, |_, _, row| {
        let entry = &row["entry"];
        let raw = encode(entry, l.max_output_row_bytes)?;
        claim_hash.update(&raw);
        claim_hash.update(b"\n");
        catalog_hash.update(&raw);
        catalog_hash.update(b"\n");
        if let Some(schema) = entry.get("source_schema_ref").and_then(Value::as_str) {
            extensions.insert(schema.to_owned());
        }
        Ok(())
    })?;
    file_sha.insert(
        format!("{ROOT}claims.jsonl"),
        claim_hash.finalize().to_hex(),
    );
    counts.insert("object_total".into(), json!(record_count));
    counts.insert("claim".into(), json!(claim_count));
    counts.insert("total".into(), json!(record_count + claim_count));
    let mut manifest = json!({"schema_version":"tos_source_witness_catalog_v3","owner_repo":"Tree-of-Sophia",
        "source_root":"ToS/source-witnesses","generated_by":"scripts/build_source_witness_catalog.py",
        "record_schema_ref":CORPUS,"claim_schema_ref":CLAIM,"record_files":record_files,
        "claim_file":format!("{ROOT}claims.jsonl"),"counts":counts,"catalog_sha256":catalog_hash.finalize().to_hex(),
        "authority_boundary":"This generated catalog provides navigation to the tracked object and claim records that own its contents."});
    if !extensions.is_empty() {
        manifest["extension_schema_refs"] = json!(extensions);
    }
    // Preserve the maintained carrier's producer field for byte parity; the
    // external receipt identifies this actual native implementation/worker.
    // No selected_metadata_publication token is fabricated.
    Ok((manifest, file_sha, record_count, claim_count))
}

/// Complete applicable catalog family preparation. Unsupported owner adapters
/// fail closed before a candidate receipt is issued; schema success alone is
/// insufficient for native text, scoped structure or identity transitions.
pub fn prepare_source_witness_catalog(
    stage: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: SourceCatalogLimits,
) -> Result<SourceCatalogReceipt> {
    let result = (|| {
        l.validate()?;
        let input = input_root(stage, l)?;
        let c = contracts(stage, validator, l)?;
        stage.with_connection(WritePhase::Schema, |db| {
            db.execute_batch("CREATE TABLE source_catalog_rows(category TEXT NOT NULL,kind TEXT NOT NULL,id TEXT NOT NULL,
                payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL,
                PRIMARY KEY(category,id)) WITHOUT ROWID;
                CREATE INDEX source_catalog_rows_kind ON source_catalog_rows(category,kind,id);
                CREATE TABLE source_catalog_reserved(id TEXT PRIMARY KEY) WITHOUT ROWID;")?; Ok(())
        })?;
        native_inventory(stage, &c, validator, l)?;
        source_files(stage, &c, validator, l)?;
        let (manifest, file_sha256, record_count, claim_count) = outputs(stage, &c, l)?;
        let source_slot_count = visit_rows(stage, "slots", None, l, |_, _, _| Ok(()))?;
        let (row_count, row_root_sha256) = row_root(stage, l)?;
        if input_root(stage, l)? != input {
            return Err(Error::Invalid(
                "catalog input cut changed during preparation",
            ));
        }
        let manifest_sha256 =
            Digest256::of_bytes(&encode(&manifest, l.max_output_row_bytes)?).to_hex();
        let mut receipt = SourceCatalogReceipt {
            record_count,
            claim_count,
            source_slot_count,
            manifest,
            file_sha256,
            row_root_sha256,
            input_binding: stage.exact_receipt().binding.clone(),
            worker_sha256: validator.worker.sha256.to_hex(),
            input_root: input,
            manifest_sha256,
            row_count,
            summary_sha256: String::new(),
        };
        receipt.summary_sha256 = summary(&receipt, l)?;
        Ok(receipt)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Verify retained root and sealed input bytes, then stream a private candidate.
pub fn render_source_witness_catalog(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SourceCatalogReceipt,
    l: SourceCatalogLimits,
    sink: &mut impl SourceCatalogSink,
) -> Result<()> {
    let result = (|| {
        l.validate()?;
        if summary(receipt, l)? != receipt.summary_sha256
            || input_root(stage, l)? != receipt.input_root
            || binding_value(&stage.exact_receipt().binding)
                != binding_value(&receipt.input_binding)
            || row_root(stage, l)? != (receipt.row_count, receipt.row_root_sha256.clone())
            || Digest256::of_bytes(&encode(&receipt.manifest, l.max_output_row_bytes)?).to_hex()
                != receipt.manifest_sha256
        {
            return Err(Error::Invalid("catalog candidate exact seal mismatch"));
        }
        let files = receipt.manifest["record_files"]
            .as_object()
            .ok_or(Error::Invalid("catalog record file manifest"))?;
        for (kind, value) in files {
            let ref_ = value
                .as_str()
                .ok_or(Error::Invalid("catalog output file ref"))?;
            sink.begin_file(ref_)?;
            let mut hash = Digest256Hasher::new();
            visit_rows(stage, "records", Some(kind), l, |_, _, row| {
                let raw = encode(&row["entry"], l.max_output_row_bytes)?;
                sink.file_bytes(&raw)?;
                sink.file_bytes(b"\n")?;
                hash.update(&raw);
                hash.update(b"\n");
                Ok(())
            })?;
            let digest = hash.finalize().to_hex();
            if receipt.file_sha256.get(ref_) != Some(&digest) {
                return Err(Error::Invalid("catalog output file digest"));
            }
            sink.end_file(ref_, &digest)?;
        }
        let claim_ref = text(&receipt.manifest, "claim_file")?;
        sink.begin_file(claim_ref)?;
        let mut hash = Digest256Hasher::new();
        visit_rows(stage, "claims", None, l, |_, _, row| {
            let raw = encode(&row["entry"], l.max_output_row_bytes)?;
            sink.file_bytes(&raw)?;
            sink.file_bytes(b"\n")?;
            hash.update(&raw);
            hash.update(b"\n");
            Ok(())
        })?;
        let digest = hash.finalize().to_hex();
        if receipt.file_sha256.get(claim_ref) != Some(&digest) {
            return Err(Error::Invalid("catalog Claim file digest"));
        }
        sink.end_file(claim_ref, &digest)?;
        for category in ["records", "claims", "slots"] {
            visit_rows(stage, category, None, l, |_, raw, _| {
                sink.addressed_row(category, raw)
            })?;
        }
        sink.manifest(&receipt.manifest)?;
        Ok(())
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Remove only this family's private staging rows after candidate consumption.
pub fn clear_source_witness_catalog(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SourceCatalogReceipt,
    l: SourceCatalogLimits,
) -> Result<()> {
    let result = (|| {
        l.validate()?;
        if summary(receipt, l)? != receipt.summary_sha256
            || input_root(stage, l)? != receipt.input_root
            || row_root(stage, l)? != (receipt.row_count, receipt.row_root_sha256.clone())
        {
            return Err(Error::Invalid("catalog cleanup exact seal mismatch"));
        }
        stage.with_connection(WritePhase::Finalize, |db| {
            db.execute_batch(
                "DROP TABLE source_catalog_rows; DROP TABLE source_catalog_reserved;",
            )?;
            Ok(())
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Recheck the privately sealed preparation before any downstream source read.
pub(crate) fn verify_catalog(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SourceCatalogReceipt,
    l: SourceCatalogLimits,
) -> Result<()> {
    l.validate()?;
    if summary(receipt, l)? != receipt.summary_sha256
        || input_root(stage, l)? != receipt.input_root
        || binding_value(&stage.exact_receipt().binding) != binding_value(&receipt.input_binding)
        || row_root(stage, l)? != (receipt.row_count, receipt.row_root_sha256.clone())
    {
        return Err(Error::Invalid("bibliographic catalog exact seal"));
    }
    Ok(())
}

/// Indexed, capped retained catalog seek. Never deserialize an unchecked BLOB.
pub(crate) fn catalog_row(
    stage: &mut KnowledgeStage<'_>,
    category: &str,
    id: &str,
    l: SourceCatalogLimits,
) -> Result<Option<Value>> {
    stage.with_connection(WritePhase::Catalog, |db| {
        let row = db.query_row("SELECT CASE WHEN length(payload)<=?3 AND payload_len=length(payload) THEN payload ELSE NULL END,payload_sha256
            FROM source_catalog_rows WHERE category=?1 AND id=?2", params![category,id,l.max_output_row_bytes as i64],
            |r| Ok((r.get::<_,Option<Vec<u8>>>(0)?,r.get::<_,Vec<u8>>(1)?))).optional()?;
        row.map(|(raw, sha)| {
            let raw = raw.ok_or(Error::Budget("bibliographic catalog retained row"))?;
            if Digest256::of_bytes(&raw).as_bytes().as_slice() != sha { return Err(Error::Invalid("bibliographic catalog row digest")); }
            Ok(SourceRow::parse(&raw,l.max_output_row_bytes)?.value().clone())
        }).transpose()
    })
}

/// One indexed key at a time, retaining memory independent of source population.
pub(crate) fn catalog_next(
    stage: &mut KnowledgeStage<'_>,
    category: &str,
    after: Option<&str>,
) -> Result<Option<String>> {
    stage.with_connection(WritePhase::Catalog, |db| {
        let row = db.query_row("SELECT CASE WHEN length(CAST(id AS BLOB))<=8192 THEN id ELSE NULL END FROM source_catalog_rows
            WHERE category=?1 AND (?2 IS NULL OR id>?2) ORDER BY id LIMIT 1",params![category,after], |r|r.get::<_,Option<String>>(0)).optional()?;
        row.map(|id| id.ok_or(Error::Budget("bibliographic catalog key"))).transpose()
    })
}

pub(crate) fn check_catalog_schema(
    stage: &KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    l: SourceCatalogLimits,
    schema: &str,
    fragment: &str,
    raw: &[u8],
) -> Result<()> {
    let c = contracts(stage, validator, l)?;
    validator.check(&c, schema, fragment, raw)
}

/// Pure catalog projection after the caller has verified exact owner grammar,
/// source path, visibility, identity inventory and profile binding. A renderer
/// neither resolves native attachments nor admits a source. `source_schema_ref`
/// is supplied only by an exact adaptive/native schema route, never inferred.
pub fn render_catalog_record(
    source: &Value,
    reference: &str,
    source_schema_ref: Option<&str>,
    max_row_bytes: usize,
) -> Result<Value> {
    source_ref(reference)?;
    let (id, kind, label, status, pointer) = if reference.ends_with("/artifact-witness.json") {
        (
            text(source, "artifact_id")?,
            "artifact",
            source
                .pointer("/custody/inventory_numbers/0")
                .cloned()
                .ok_or(Error::Invalid("catalog artifact label"))?,
            Value::Null,
            Some("/custody/inventory_numbers/0"),
        )
    } else if reference.ends_with("/composite-witness.json") {
        (
            text(source, "composite_id")?,
            "composite",
            source["preferred_label"].clone(),
            source["identity_status"].clone(),
            None,
        )
    } else {
        (
            text(source, "record_id")?,
            text(source, "record_type")?,
            source.get("preferred_label").cloned().unwrap_or(json!("")),
            source.get("identity_status").cloned().unwrap_or(json!("")),
            None,
        )
    };
    let mut links = Map::new();
    if pointer.is_none() && !reference.ends_with("/composite-witness.json") {
        for field in LINKS {
            if let Some(value) = source.get(*field) {
                links.insert((*field).into(), value.clone());
            }
        }
    }
    let mut entry = json!({"schema_version":"tos_source_witness_catalog_entry_v1","record_id":id,"record_type":kind,"preferred_label":label,"identity_status":status,"source_record_ref":reference,"record_sha256":Digest256::of_bytes(&encode(source,max_row_bytes)?).to_hex(),"links":links});
    if let Some(schema) = source_schema_ref {
        entry["source_schema_ref"] = json!(schema);
    }
    if let Some(pointer) = pointer {
        entry["label_source_pointer"] = json!(pointer);
    }
    encode(&entry, max_row_bytes)?;
    Ok(entry)
}

/// Exact borrowed source Claim renderer. Source schema/profile/line closure is
/// the caller's responsibility; no absent source field acquires a value here.
pub fn render_catalog_claim(
    source: &Value,
    reference: &str,
    line: u64,
    source_schema_ref: Option<&str>,
    max_row_bytes: usize,
) -> Result<Value> {
    source_ref(reference)?;
    if line == 0 {
        return Err(Error::Invalid("catalog Claim physical line"));
    }
    let mut entry = json!({"schema_version":"tos_source_witness_claim_catalog_entry_v1","source_claim_file_ref":reference,"source_claim_line":line,"claim_sha256":Digest256::of_bytes(&encode(source,max_row_bytes)?).to_hex()});
    for field in [
        "claim_id",
        "claim_type",
        "assertion_layer",
        "subject_ref",
        "predicate",
        "object",
        "evidence_refs",
        "maker",
        "provenance_event_ref",
        "epistemic_status",
        "review_status",
        "visibility",
        "claim_version",
    ] {
        entry[field] = source.get(field).cloned().unwrap_or(Value::Null);
    }
    entry["review_refs"] = json!(
        source
            .get("reviews")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|review| review.get("review_id")?.as_str())
            .collect::<Vec<_>>()
    );
    for field in ["supersedes_claim_ref", "qualifiers"] {
        if let Some(value) = source.get(field) {
            entry[field] = value.clone();
        }
    }
    let schema = source_schema_ref.or_else(|| {
        if source["schema_version"] == "tos_historical_claim_v1" {
            Some("ToS/contracts/historical-claim.schema.json")
        } else {
            None
        }
    });
    if let Some(schema) = schema {
        entry["source_schema_ref"] = json!(schema);
    }
    encode(&entry, max_row_bytes)?;
    Ok(entry)
}
