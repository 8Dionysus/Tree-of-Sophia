//! Cold current authored source-cut recipe for the maintained catalog and graph.
//! A private bounded plan freezes exact original-file roots before the caller
//! creates its independently receipted stage. Generated catalog artifacts and
//! retained archives are not relabelled as current catalog source files.
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::{ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage};
use crate::source_bibliographic::{
    self as graph, BibliographicForms, BibliographicLimits, BibliographicReceipt,
    BibliographicSourceCut,
};
use crate::source_witness_catalog::{
    self as catalog, BIBLIOGRAPHIC_FILES, CATALOG_SOURCE, CONTRACT_FILES, NATIVE_IDENTITIES,
    NATIVE_TEXT, SOURCE_FILES, SourceCatalogReceipt, SourceCatalogValidator,
};
use crate::{Error, Result, SourceBinding};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

const ENTITY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const COLLECTIONS: [&str; 5] = [
    SOURCE_FILES,
    CONTRACT_FILES,
    NATIVE_IDENTITIES,
    NATIVE_TEXT,
    BIBLIOGRAPHIC_FILES,
];

#[derive(Clone, Copy, Debug)]
pub struct SourceCatalogInputLimits {
    pub max_manifest_members: u64,
    pub max_selected_members: usize,
    /// Includes selected locator/index/queue storage. Absent paths allocate no
    /// retained state and cannot become fabricated raw input rows.
    pub max_plan_bytes: usize,
    /// Original bytes observed while planning or transferring, with repeats.
    pub max_work_bytes: u64,
}
impl SourceCatalogInputLimits {
    fn validate(self, l: BibliographicLimits) -> Result<()> {
        l.validate()?;
        l.catalog.validate()?;
        if self.max_manifest_members == 0
            || self.max_selected_members == 0
            || self.max_selected_members > 4096
            || self.max_plan_bytes == 0
            || self.max_plan_bytes > 64 * 1024 * 1024
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("cold source catalog input limits"));
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Member {
    size: u64,
    sha: Digest256,
    collections: BTreeSet<&'static str>,
}
/// Only the producer can create the selected member plan. The receipt is a
/// clone for final-stage creation; changing it cannot change this private seal.
pub struct SourceCatalogInputPlan {
    receipt: ExactInputReceipt,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    members: BTreeMap<String, Member>,
    limits: SourceCatalogInputLimits,
    work_bytes: u64,
}
impl SourceCatalogInputPlan {
    pub fn input_receipt(&self) -> ExactInputReceipt {
        self.receipt.clone()
    }
    pub fn selected_member_count(&self) -> usize {
        self.members.len()
    }
    pub fn source_revision(&self) -> SourceRevision {
        self.revision
    }
    pub fn source_membership(&self) -> SourceMembershipV1 {
        self.membership
    }
    pub fn observed_work_bytes(&self) -> u64 {
        self.work_bytes
    }
    pub(crate) fn verify_selected_member_bytes(&self, path: &str, raw: &[u8]) -> Result<()> {
        let member = self
            .members
            .get(path)
            .ok_or(Error::Invalid("source plan selected member absent"))?;
        if member.size != raw.len() as u64 || member.sha != Digest256::of_bytes(raw) {
            return Err(Error::Invalid("source plan exact selected member bytes"));
        }
        Ok(())
    }
}
pub struct SourceBibliographicCandidate {
    pub catalog: SourceCatalogReceipt,
    pub bibliographic: BibliographicReceipt,
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("cold source catalog cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("cold source catalog deadline"));
    }
    Ok(())
}
fn selected_cut(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    expected: SourceMembershipV1,
    limits: SourceCatalogInputLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    check(deadline, cancelled)?;
    if cut.current().revision() != revision
        || cut
            .stream(revision)
            .map_err(|_| Error::Invalid("cold catalog selected revision"))?
            .expectation()
            != expected
    {
        return Err(Error::Invalid("cold catalog independent source selection"));
    }
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-val-full-membership-v1\0");
    let mut count = 0u64;
    for member in cut.current().members() {
        check(deadline, cancelled)?;
        count = count
            .checked_add(1)
            .filter(|n| *n <= limits.max_manifest_members)
            .ok_or(Error::Budget("cold catalog manifest EOF"))?;
        let path = member.path.as_str();
        hash.update(&(path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
        hash.update(&member.size_bytes.to_be_bytes());
        hash.update(member.sha256.as_bytes());
    }
    if (SourceMembershipV1 {
        count,
        digest: hash.finalize(),
    }) != expected
    {
        return Err(Error::Invalid("cold catalog complete metadata membership"));
    }
    Ok(())
}
fn public_path(path: &str) -> bool {
    path.starts_with("ToS/")
        && path.len() <= 4096
        && !path.contains(['\\', '\0'])
        && path.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !matches!(
                    part,
                    "catalog" | "payload" | "private" | "owner-local" | "local-content"
                )
        })
}
struct Planning<'a> {
    cut: &'a CorpusCutReader,
    revision: SourceRevision,
    limits: SourceCatalogInputLimits,
    graph_limits: BibliographicLimits,
    cancelled: &'a AtomicBool,
    members: BTreeMap<String, Member>,
    pending: BTreeSet<String>,
    scanned: BTreeSet<String>,
    plan_bytes: usize,
    work_bytes: u64,
}
impl Planning<'_> {
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.plan_bytes = self
            .plan_bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_plan_bytes)
            .ok_or(Error::Budget("cold catalog plan state"))?;
        Ok(())
    }
    fn queue(&mut self, path: &str) -> Result<()> {
        if !self.scanned.contains(path) && !self.pending.contains(path) {
            self.charge(
                path.len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(128))
                    .ok_or(Error::Budget("cold catalog locator state"))?,
            )?;
            self.pending.insert(path.to_owned());
        }
        Ok(())
    }
    fn select(&mut self, path: &str, collection: &'static str) -> Result<bool> {
        if !public_path(path) {
            return Err(Error::Invalid("cold catalog public dependency path"));
        }
        let relative =
            RelativePath::parse(path).map_err(|_| Error::Invalid("cold catalog relative path"))?;
        let Some(metadata) = self.cut.current().member(&relative) else {
            return Ok(false);
        };
        if metadata.size_bytes
            > self
                .graph_limits
                .catalog
                .max_file_bytes
                .min(8 * 1024 * 1024) as u64
        {
            return Err(Error::Budget(
                "cold catalog original source exceeds raw stage row cap",
            ));
        }
        let size = metadata.size_bytes;
        let sha = metadata.sha256;
        if !self.members.contains_key(path) {
            if self.members.len() >= self.limits.max_selected_members {
                return Err(Error::Budget("cold catalog selected files"));
            }
            self.charge(
                path.len()
                    .checked_add(256)
                    .ok_or(Error::Budget("cold catalog member state"))?,
            )?;
            self.members.insert(
                path.to_owned(),
                Member {
                    size,
                    sha,
                    collections: BTreeSet::new(),
                },
            );
            self.queue(path)?;
        }
        if !self.members[path].collections.contains(collection) {
            self.charge(128)?;
        }
        let member = self
            .members
            .get_mut(path)
            .ok_or(Error::Invalid("cold catalog member lost"))?;
        member.collections.insert(collection);
        // The maintained readers search these unions and reject duplicate raw
        // paths. A primary file remains primary when later referenced.
        if member.collections.contains(SOURCE_FILES) {
            member.collections.remove(NATIVE_TEXT);
            member.collections.remove(BIBLIOGRAPHIC_FILES);
        }
        if member.collections.contains(CONTRACT_FILES) {
            member.collections.remove(BIBLIOGRAPHIC_FILES);
        }
        Ok(true)
    }
    fn raw(&mut self, path: &str) -> Result<Vec<u8>> {
        check(self.graph_limits.deadline, self.cancelled)?;
        let member = self
            .members
            .get(path)
            .ok_or(Error::Invalid("cold catalog raw plan absent"))?;
        self.work_bytes = self
            .work_bytes
            .checked_add(member.size)
            .filter(|n| *n <= self.limits.max_work_bytes)
            .ok_or(Error::Budget("cold catalog raw work"))?;
        let size = member.size;
        let sha = member.sha;
        let raw = self
            .cut
            .read_member(
                self.revision,
                &RelativePath::parse(path).map_err(|_| Error::Invalid("cold catalog raw path"))?,
                self.graph_limits
                    .catalog
                    .max_file_bytes
                    .min(8 * 1024 * 1024) as u64,
                self.graph_limits.deadline,
                self.cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?
            .raw;
        if raw.len() as u64 != size || Digest256::of_bytes(&raw) != sha {
            return Err(Error::Invalid("cold catalog original bytes binding"));
        }
        Ok(raw)
    }
    fn dependency(&mut self, path: &str) -> Result<()> {
        // Only actual positive public metadata members join the raw recipe.
        // Payload/corpus bytes and generated catalog carriers remain outside.
        if public_path(path) {
            self.select(path, NATIVE_TEXT)?;
            self.select(path, BIBLIOGRAPHIC_FILES)?;
        }
        Ok(())
    }
    fn value(&mut self, value: &Value, depth: usize) -> Result<()> {
        check(self.graph_limits.deadline, self.cancelled)?;
        if depth > 96 {
            return Err(Error::Budget("cold catalog dependency depth"));
        }
        match value {
            Value::String(path) if path.starts_with("ToS/") => self.dependency(path)?,
            Value::Array(values) => {
                for value in values {
                    self.value(value, depth + 1)?;
                }
            }
            Value::Object(values) => {
                for value in values.values() {
                    self.value(value, depth + 1)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    fn packet(&mut self, path: &str, raw: &[u8]) -> Result<()> {
        let source = self.members[path].collections.contains(SOURCE_FILES);
        let required = source
            || self.members[path].collections.contains(NATIVE_IDENTITIES)
            || self.members[path].collections.contains(CONTRACT_FILES);
        let cap = self.graph_limits.catalog.max_row_bytes;
        if path.ends_with(".json") {
            let parsed = SourceRow::parse(raw, cap);
            if required {
                self.value(parsed?.value(), 0)?;
            } else if let Ok(parsed) = parsed {
                self.value(parsed.value(), 0)?;
            }
            if source {
                self.dependency(&format!(
                    "{}.human-forms.json",
                    path.strip_suffix(".json")
                        .ok_or(Error::Invalid("cold catalog JSON suffix"))?
                ))?;
            }
        } else if path.ends_with(".jsonl") {
            // Original bytes stay intact. This pass discovers dependencies;
            // the owner producer verifies physical line/slot semantics later.
            let text =
                std::str::from_utf8(raw).map_err(|_| Error::Invalid("cold catalog JSONL UTF-8"))?;
            for line in text
                .split(['\r', '\n'])
                .filter(|line| !line.trim().is_empty())
            {
                let parsed = SourceRow::parse(line.as_bytes(), cap);
                if let Ok(parsed) = parsed {
                    self.value(parsed.value(), 0)?;
                    if source
                        && matches!(
                            path.rsplit('/').next(),
                            Some("source-claims.jsonl" | "historical-claims.jsonl")
                        )
                    {
                        if let Some(id) = parsed.value().get("claim_id").and_then(Value::as_str) {
                            self.dependency(&format!(
                                "{}.{}.human-forms.json",
                                path.strip_suffix(".jsonl")
                                    .ok_or(Error::Invalid("cold catalog JSONL suffix"))?,
                                Digest256::of_bytes(id.as_bytes()).to_hex()
                            ))?;
                        }
                    }
                } else if required {
                    return Err(Error::Invalid("cold catalog source JSONL row"));
                }
            }
        }
        Ok(())
    }
}
fn frame(hash: &mut Digest256Hasher, path: &str, sha: &Digest256) {
    hash.update(&(path.len() as u64).to_be_bytes());
    hash.update(path.as_bytes());
    hash.update(sha.as_bytes());
}
fn collection_receipts(
    members: &BTreeMap<String, Member>,
    l: BibliographicLimits,
) -> Result<Vec<InputCollectionReceipt>> {
    let mut result = Vec::new();
    for collection in COLLECTIONS {
        let mut hash = Digest256Hasher::new();
        let mut count = 0u64;
        for (path, member) in members {
            if member.collections.contains(collection) {
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= l.catalog.max_files)
                    .ok_or(Error::Budget("cold catalog collection files"))?;
                if collection == NATIVE_IDENTITIES && count > 1024 {
                    return Err(Error::Budget(
                        "cold catalog native identity inventory packets",
                    ));
                }
                frame(&mut hash, path, &member.sha);
            }
        }
        let (role, profile) = catalog::input_role(collection)?;
        result.push(InputCollectionReceipt {
            source_graph: CATALOG_SOURCE.into(),
            collection: collection.into(),
            input_role: role.into(),
            adapter_profile: profile.into(),
            expected_count: count,
            expected_root_sha256: hash.finalize().to_hex(),
        });
    }
    Ok(result)
}
/// Discover maintained current source families, complete native identities and
/// their actual public metadata dependency closure. No callback or worker can
/// turn this observation plan into catalog/schema/semantic acceptance.
pub fn plan_source_catalog_inputs(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    binding: &SourceBinding,
    limits: SourceCatalogInputLimits,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<SourceCatalogInputPlan> {
    limits.validate(l)?;
    binding.validate()?;
    selected_cut(
        cut,
        expected_revision,
        expected_membership,
        limits,
        l.deadline,
        cancelled,
    )?;
    let mut plan = Planning {
        cut,
        revision: expected_revision,
        limits,
        graph_limits: l,
        cancelled,
        members: BTreeMap::new(),
        pending: BTreeSet::new(),
        scanned: BTreeSet::new(),
        plan_bytes: 0,
        work_bytes: 0,
    };
    for registry in [ENTITY, RELATION] {
        if !plan.select(registry, CONTRACT_FILES)? {
            return Err(Error::Invalid("cold catalog source registry absent"));
        }
    }
    let raw = plan.raw(ENTITY)?;
    let entities = SourceRow::parse(&raw, l.catalog.max_row_bytes)?;
    let basenames = catalog::source_basenames(entities.value())?;
    for member in cut.current().members() {
        check(l.deadline, cancelled)?;
        let path = member.path.as_str();
        let basename = path.rsplit('/').next().unwrap_or("");
        if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") && public_path(path)
        {
            plan.select(path, CONTRACT_FILES)?;
        }
        if path.starts_with("ToS/source-witnesses/") {
            if basenames.contains(basename)
                || basename.ends_with(".jsonl")
                    && (basename.contains("provenance") || basename.contains("anchor"))
            {
                catalog::source_ref(path)?;
                plan.select(path, SOURCE_FILES)?;
            }
            if basename.starts_with("semantic-annotation")
                && basename.ends_with(".json")
                && !path
                    .split('/')
                    .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
            {
                catalog::source_ref(path)?;
                plan.select(path, NATIVE_IDENTITIES)?;
            }
        }
    }
    while let Some(path) = plan.pending.pop_first() {
        check(l.deadline, cancelled)?;
        plan.scanned.insert(path.clone());
        let raw = plan.raw(&path)?;
        plan.packet(&path, &raw)?;
    }
    let receipt = ExactInputReceipt {
        binding: binding.clone(),
        collections: collection_receipts(&plan.members, l)?,
    };
    Ok(SourceCatalogInputPlan {
        receipt,
        revision: expected_revision,
        membership: expected_membership,
        members: plan.members,
        limits,
        work_bytes: plan.work_bytes,
    })
}
fn same_binding(a: &SourceBinding, b: &SourceBinding) -> bool {
    a.owner_profile == b.owner_profile
        && a.source_cut == b.source_cut
        && a.through_commit_seq == b.through_commit_seq
        && a.membership_root == b.membership_root
        && a.index_generation == b.index_generation
        && a.route_map_version == b.route_map_version
        && a.reader_abi == b.reader_abi
        && a.complete == b.complete
}
/// Transfer actual original bytes into an independently created exact stage,
/// then execute the existing catalog and selected-cut bibliographic producers.
/// The caller supplies the actual native forms adapter from its owning crate;
/// compiler has no dependency on command execution or publication authority.
pub fn render_source_bibliographic_plan(
    plan: &SourceCatalogInputPlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    max_version_read_files: usize,
    max_version_read_bytes: usize,
) -> Result<SourceBibliographicCandidate> {
    let result = (|| {
        plan.limits.validate(l)?;
        if max_version_read_files == 0
            || max_version_read_files > 4096
            || max_version_read_bytes == 0
            || max_version_read_bytes > 64 * 1024 * 1024
        {
            return Err(Error::Budget("cold catalog selected version read limits"));
        }
        drop(validator.schemas(expected_revision)?);
        selected_cut(
            cut,
            expected_revision,
            expected_membership,
            plan.limits,
            l.deadline,
            validator.cancelled,
        )?;
        if plan.revision != expected_revision
            || plan.membership != expected_membership
            || !same_binding(&plan.receipt.binding, &target.exact_receipt().binding)
        {
            return Err(Error::Invalid("cold catalog plan source binding"));
        }
        let actual = collection_receipts(&plan.members, l)?;
        let target_receipt = target.exact_receipt();
        if target_receipt.collections.len() != actual.len() {
            return Err(Error::Invalid("cold catalog target collection closure"));
        }
        for expected in &actual {
            let entry = target_receipt
                .collections
                .iter()
                .find(|entry| {
                    entry.source_graph == expected.source_graph
                        && entry.collection == expected.collection
                })
                .ok_or(Error::Invalid("cold catalog target collection missing"))?;
            if entry.input_role != expected.input_role
                || entry.adapter_profile != expected.adapter_profile
                || entry.expected_count != expected.expected_count
                || entry.expected_root_sha256 != expected.expected_root_sha256
            {
                return Err(Error::Invalid("cold catalog independent target roots"));
            }
        }
        let mut work = 0u64;
        for (path, member) in &plan.members {
            check(l.deadline, validator.cancelled)?;
            let relative = RelativePath::parse(path)
                .map_err(|_| Error::Invalid("cold catalog transfer path"))?;
            let metadata = cut
                .current()
                .member(&relative)
                .ok_or(Error::Invalid("cold catalog planned member absent"))?;
            if metadata.sha256 != member.sha || metadata.size_bytes != member.size {
                return Err(Error::Invalid("cold catalog planned member changed"));
            }
            work = work
                .checked_add(member.size)
                .filter(|n| *n <= plan.limits.max_work_bytes)
                .ok_or(Error::Budget("cold catalog transfer work"))?;
            let raw = cut
                .read_member(
                    expected_revision,
                    &relative,
                    l.catalog.max_file_bytes.min(8 * 1024 * 1024) as u64,
                    l.deadline,
                    validator.cancelled,
                )
                .map_err(|e| Error::Source(e.to_string()))?
                .raw;
            if raw.len() as u64 != member.size || Digest256::of_bytes(&raw) != member.sha {
                return Err(Error::Invalid("cold catalog transfer exact bytes"));
            }
            let (max_rows, max_bytes) = target.input_batch_limits();
            let rows_per_chunk = if raw.is_empty() {
                max_rows
            } else {
                max_rows.min((max_bytes / raw.len() as u64) as usize)
            };
            if rows_per_chunk == 0 {
                for collection in &member.collections {
                    check(l.deadline, validator.cancelled)?;
                    target.ingest_input(InputRow {
                        source_graph: CATALOG_SOURCE,
                        collection,
                        id: path,
                        payload: &raw,
                    })?;
                }
            } else {
                let mut collections = member.collections.iter();
                loop {
                    let rows = collections
                        .by_ref()
                        .take(rows_per_chunk)
                        .map(|collection| {
                            check(l.deadline, validator.cancelled)?;
                            Ok(InputRow {
                                source_graph: CATALOG_SOURCE,
                                collection,
                                id: path,
                                payload: &raw,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    if rows.is_empty() {
                        break;
                    }
                    target.ingest_input_batch(&rows)?;
                    check(l.deadline, validator.cancelled)?;
                }
            }
        }
        let receipt = catalog::prepare_source_witness_catalog(target, validator, l.catalog)?;
        let source = BibliographicSourceCut {
            cut,
            expected_revision,
            expected_membership,
            stage_source_cut: &plan.receipt.binding.source_cut,
            max_read_files: max_version_read_files,
            max_read_bytes: max_version_read_bytes,
        };
        let bibliographic = graph::prepare_bibliographic_graph_from_cut(
            target, &receipt, validator, forms, l, &source,
        )?;
        Ok(SourceBibliographicCandidate {
            catalog: receipt,
            bibliographic,
        })
    })();
    if result.is_err() {
        target.poison();
    }
    result
}
