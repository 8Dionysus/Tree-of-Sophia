//! Source-bound repository topology. Paths select ownership routes only;
//! source IDs and source array ordinals are supplied by their owner.

use crate::knowledge_base::{BaseNodeOverrides, KnowledgeBaseNormalizer};
use crate::knowledge_global_titles::CompleteBaseNodes;
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::{KnowledgeStage, NodeRow, RelationRow, WritePhase};
use crate::{Error, QueryVocabulary, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tos_foundation::{Digest256, Digest256Hasher};

const PROFILE: &str = "repository-topology-v1";
const COLLECTIONS: [&str; 3] = ["branches", "manifests", "resources"];
const ORDER_COLLECTION: &str = "source_order";

#[derive(Clone, Copy, Debug)]
pub struct TopologyLimits {
    pub max_rows: u64,
    pub max_page_rows: usize,
    pub max_row_bytes: usize,
    pub max_work_bytes: u64,
}
impl TopologyLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_rows == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_work_bytes == 0
            || self
                .max_page_rows
                .checked_mul(self.max_row_bytes)
                .is_none_or(|n| n > 64 * 1024 * 1024)
        {
            return Err(Error::Budget("topology limits"));
        }
        Ok(())
    }
}

/// `source_order` is an owner-sealed raw collection of exact
/// {collection,id,ordinal} rows, registered by the same receipt/cut.
/// The adapter checks membership, unique dense ordinals and its own root.
/// Exact selected source-home root material, including its explicit identity.
/// The producer does not derive the repository identity from a filename.
#[derive(Clone, Copy)]
pub struct RepositoryRootInput<'a> {
    pub source_cut: &'a str,
    pub material: &'a [u8],
    pub material_sha256: &'a str,
    pub identity_id: &'a str,
}

#[derive(Clone, Debug)]
pub struct RepositoryPrepareReceipt {
    pub source_graph: String,
    pub source_cut: String,
    pub descriptor_sha256: String,
    pub root_input_sha256: String,
    pub ordering_root_sha256: String,
    pub input_roots: Vec<(String, u64, String)>,
    pub nodes: u64,
    pub relations: u64,
    pub dependency_root_sha256: String,
}

pub(crate) fn text(value: Option<&Value>) -> Option<&str> {
    value?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
pub(crate) fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    text(value.get(key))
        .filter(|s| s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("topology required field"))
}
pub(crate) fn strings(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
        .collect()
}
pub(crate) fn charge(work: &mut u64, bytes: usize, limits: TopologyLimits) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .filter(|n| *n <= limits.max_work_bytes)
        .ok_or(Error::Budget("topology work bytes"))?;
    Ok(())
}
pub(crate) fn bytes(value: &Value, limits: TopologyLimits) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Invalid("topology JSON"))?;
    if bytes.len() > limits.max_row_bytes {
        return Err(Error::Budget("topology row bytes"));
    }
    Ok(bytes)
}
pub(crate) fn root_item(root: &mut Digest256Hasher, id: &str, sha: &[u8]) {
    root.update(&(id.len() as u64).to_be_bytes());
    root.update(id.as_bytes());
    root.update(sha);
}
pub(crate) fn source_for(vocab: &QueryVocabulary, profile: &str) -> Result<String> {
    let mut matches = vocab
        .sources
        .iter()
        .filter(|s| s.adapter_profile == profile);
    let source = matches
        .next()
        .ok_or(Error::Invalid("topology missing profile"))?;
    if matches.next().is_some() {
        return Err(Error::Invalid("topology ambiguous profile"));
    }
    Ok(source.source_graph_id.clone())
}

fn add_material(
    stage: &mut KnowledgeStage<'_>,
    relation: bool,
    native: &str,
    identity: &str,
    kind: &str,
    source_order: i64,
    material: &[u8],
    proof: &str,
) -> Result<()> {
    let sha = Digest256::of_bytes(material);
    let key = if relation { native } else { identity };
    stage.with_connection(WritePhase::Sort, |db| {
        db.execute(
            "INSERT INTO knowledge_repository_material VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                relation as i64,
                native,
                identity,
                kind,
                source_order,
                material.len() as i64,
                &sha.as_bytes()[..],
                material,
                proof,
                key
            ],
        )?;
        Ok(())
    })
}

fn prepared_root(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    limits: TopologyLimits,
) -> Result<(u64, u64, String)> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-repository-topology-prepare-v1\0");
    for s in [
        &receipt.source_graph,
        &receipt.source_cut,
        &receipt.descriptor_sha256,
        &receipt.root_input_sha256,
        &receipt.ordering_root_sha256,
    ] {
        root_item(&mut hash, s, b"");
    }
    for (collection, count, root) in &receipt.input_roots {
        root_item(&mut hash, collection, &count.to_be_bytes());
        root_item(&mut hash, root, b"");
    }
    let mut count = [0u64; 2];
    let mut after = (-1i64, String::new());
    let mut work = 0;
    loop {
        let page = material_page(stage, after.0, &after.1, limits)?;
        if page.is_empty() {
            break;
        }
        for row in page {
            charge(&mut work, row.material.len(), limits)?;
            count[row.relation as usize] += 1;
            if count.iter().sum::<u64>() > limits.max_rows {
                return Err(Error::Budget("repository prepared rows"));
            }
            for s in [&row.native, &row.identity, &row.kind, &row.proof] {
                root_item(&mut hash, s, b"");
            }
            hash.update(&row.order.to_be_bytes());
            hash.update(&[row.relation as u8]);
            hash.update(Digest256::of_bytes(&row.material).as_bytes());
            after = (row.relation as i64, row.key);
        }
    }
    Ok((count[0], count[1], hash.finalize().to_hex()))
}

struct Material {
    relation: bool,
    native: String,
    identity: String,
    kind: String,
    order: i64,
    material: Vec<u8>,
    proof: String,
    key: String,
}
fn material_page(
    stage: &mut KnowledgeStage<'_>,
    after_kind: i64,
    after_id: &str,
    limits: TopologyLimits,
) -> Result<Vec<Material>> {
    stage.with_connection(WritePhase::Sort,|db| {
        let mut stmt=db.prepare("SELECT relation,native,identity,kind,source_order,
            CASE WHEN material_len=length(material) AND length(material)<=?3 THEN material ELSE NULL END,
            material_sha256,proof,material_key FROM knowledge_repository_material
            WHERE relation>?1 OR (relation=?1 AND material_key>?2) ORDER BY relation,material_key LIMIT ?4")?;
        let mut rows=stmt.query(params![after_kind,after_id,limits.max_row_bytes as i64,limits.max_page_rows as i64])?;
        let mut out=Vec::new();
        while let Some(row)=rows.next()? {
            let raw:Option<Vec<u8>>=row.get(5)?; let raw=raw.ok_or(Error::Budget("repository prepared material"))?;
            let sha:Vec<u8>=row.get(6)?;
            if sha != Digest256::of_bytes(&raw).as_bytes() {return Err(Error::Invalid("repository prepared material SHA"));}
            out.push(Material{relation:row.get::<_,i64>(0)?==1,native:row.get(1)?,identity:row.get(2)?,kind:row.get(3)?,order:row.get(4)?,material:raw,proof:row.get(7)?,key:row.get(8)?});
        }
        Ok(out)
    })
}

fn prepare_inner(
    stage: &mut KnowledgeStage<'_>,
    vocab: &QueryVocabulary,
    root: RepositoryRootInput<'_>,
    limits: TopologyLimits,
) -> Result<RepositoryPrepareReceipt> {
    limits.validate()?;
    let source = source_for(vocab, PROFILE)?;
    let registration = vocab
        .sources
        .iter()
        .find(|s| s.source_graph_id == source)
        .unwrap();
    let cut = stage.exact_receipt().binding.source_cut.clone();
    if root.source_cut != cut
        || Digest256::of_bytes(root.material).to_hex() != root.material_sha256
        || root.identity_id.is_empty()
        || root.identity_id.len() > 4096
        || root.identity_id.contains('\0')
        || root.material.len() > limits.max_row_bytes
        || root.material.len() as u64 > limits.max_work_bytes
    {
        return Err(Error::Invalid("repository selected root/order binding"));
    }
    let entries = stage
        .exact_receipt()
        .collections
        .iter()
        .filter(|e| e.source_graph == source)
        .cloned()
        .collect::<Vec<_>>();
    if entries.len() != 4
        || entries.iter().any(|e| {
            (!COLLECTIONS.contains(&e.collection.as_str()) && e.collection != ORDER_COLLECTION)
                || e.adapter_profile != PROFILE
                || e.input_role != registration.input_role
        })
    {
        return Err(Error::Invalid("repository collection registration"));
    }
    let mut receipt = RepositoryPrepareReceipt {
        source_graph: source.clone(),
        source_cut: cut,
        descriptor_sha256: vocab.descriptor_sha256.clone(),
        root_input_sha256: root.material_sha256.into(),
        ordering_root_sha256: entries
            .iter()
            .find(|e| e.collection == ORDER_COLLECTION)
            .ok_or(Error::Invalid("repository order receipt"))?
            .expected_root_sha256
            .clone(),
        input_roots: Vec::new(),
        nodes: 0,
        relations: 0,
        dependency_root_sha256: String::new(),
    };
    stage.with_connection(WritePhase::Schema,|db| {
        db.execute_batch("CREATE TABLE knowledge_repository_material(
            relation INTEGER NOT NULL CHECK(relation IN (0,1)),native TEXT NOT NULL,
            identity TEXT NOT NULL,kind TEXT NOT NULL,source_order INTEGER NOT NULL,
            material_len INTEGER NOT NULL,material_sha256 BLOB NOT NULL,material BLOB NOT NULL,
            proof TEXT NOT NULL,material_key TEXT NOT NULL,PRIMARY KEY(relation,material_key)) WITHOUT ROWID;
            CREATE TABLE knowledge_repository_order(collection TEXT NOT NULL,ordinal INTEGER NOT NULL,
            raw_id TEXT NOT NULL,PRIMARY KEY(collection,ordinal),UNIQUE(collection,raw_id)) WITHOUT ROWID;
            CREATE TABLE knowledge_repository_branches(path TEXT PRIMARY KEY,branch_id TEXT NOT NULL,
            ordinal INTEGER NOT NULL) WITHOUT ROWID;")?; Ok(())
    })?;
    let root_row = SourceRow::parse(root.material, limits.max_row_bytes)?;
    let root_native = required(root_row.value(), "node_id")?;
    add_material(
        stage,
        false,
        root_native,
        root.identity_id,
        "",
        0,
        root.material,
        root.material_sha256,
    )?;
    let mut node_order = 1i64;
    let mut work = root.material.len() as u64;
    let order_entry = entries
        .iter()
        .find(|e| e.collection == ORDER_COLLECTION)
        .unwrap();
    let expected_order = entries
        .iter()
        .filter(|e| e.collection != ORDER_COLLECTION)
        .try_fold(0u64, |n, e| {
            n.checked_add(e.expected_count)
                .ok_or(Error::Budget("repository order rows"))
        })?;
    if order_entry.expected_count != expected_order || expected_order > limits.max_rows {
        return Err(Error::Invalid("repository order exact count"));
    }
    let mut after = None;
    let mut order_count = 0;
    let mut order_root = Digest256Hasher::new();
    loop {
        let page = stage.scan_input(
            &source,
            ORDER_COLLECTION,
            after.as_deref(),
            limits.max_page_rows,
        )?;
        for raw in page.rows {
            charge(&mut work, raw.payload.len(), limits)?;
            let row = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
            let collection = required(row.value(), "collection")?;
            let id = required(row.value(), "id")?;
            let ordinal = row
                .value()
                .get("ordinal")
                .and_then(Value::as_u64)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or(Error::Invalid("repository order ordinal"))?;
            let target = entries
                .iter()
                .find(|e| e.collection == collection && COLLECTIONS.contains(&collection))
                .ok_or(Error::Invalid("repository order collection"))?;
            if ordinal >= target.expected_count
                || stage.raw_by_id(&source, collection, id)?.is_none()
            {
                return Err(Error::Invalid("repository order membership"));
            }
            stage.with_connection(WritePhase::Sort, |db| {
                db.execute(
                    "INSERT INTO knowledge_repository_order VALUES (?1,?2,?3)",
                    params![collection, ordinal as i64, id],
                )?;
                Ok(())
            })?;
            root_item(
                &mut order_root,
                &raw.id,
                Digest256::of_bytes(&raw.payload).as_bytes(),
            );
            order_count += 1;
            if order_count > expected_order {
                return Err(Error::Budget("repository order rows"));
            }
        }
        match page.next_id {
            Some(id) => after = Some(id),
            None => break,
        }
    }
    if order_count != expected_order
        || order_root.finalize().to_hex() != order_entry.expected_root_sha256
    {
        return Err(Error::Invalid("repository order root"));
    }
    receipt.input_roots.push((
        ORDER_COLLECTION.into(),
        order_count,
        order_entry.expected_root_sha256.clone(),
    ));
    // Branch ownership must be indexed before manifests/resources.
    for collection in COLLECTIONS {
        let entry = entries
            .iter()
            .find(|e| e.collection == collection)
            .ok_or(Error::Invalid("repository missing collection"))?;
        if entry.expected_count > limits.max_rows {
            return Err(Error::Budget("repository source rows"));
        }
        let mut after = None;
        let mut count = 0u64;
        let mut input_root = Digest256Hasher::new();
        loop {
            let page =
                stage.scan_input(&source, collection, after.as_deref(), limits.max_page_rows)?;
            for raw in page.rows {
                charge(&mut work, raw.payload.len(), limits)?;
                let ordinal:u64=stage.with_connection(WritePhase::Sort,|db| {
                    db.query_row("SELECT ordinal FROM knowledge_repository_order WHERE collection=?1 AND raw_id=?2",params![collection,raw.id],|r|r.get(0)).optional()?.ok_or(Error::Invalid("repository missing source ordinal"))
                })?;
                if ordinal >= entry.expected_count || ordinal > i64::MAX as u64 {
                    return Err(Error::Invalid("repository source ordinal"));
                }
                let original = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
                let item = original.value();
                let mut material = item.clone();
                let native = text(item.get("id"))
                    .or_else(|| text(item.get("path")))
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{collection}:{ordinal}"));
                let identity_kind = match collection {
                    "branches" => "branch",
                    "manifests" => "manifest",
                    _ => "resource",
                };
                let identity = format!("{identity_kind}:{native}");
                let kind = match collection {
                    "branches" => "repository-branch".into(),
                    "manifests" => "repository-manifest".into(),
                    _ => format!(
                        "repository-{}",
                        text(item.get("resource_kind")).unwrap_or("resource")
                    ),
                };
                if collection == "branches" {
                    let mut views = strings(item.get("view_ids"));
                    views.insert("corpus-topology".into());
                    material["view_ids"] = json!(views);
                    if let (Some(path), Some(id)) = (text(item.get("path")), text(item.get("id"))) {
                        stage.with_connection(WritePhase::Sort,|db| {
                            db.execute("INSERT INTO knowledge_repository_branches VALUES (?1,?2,?3)
                                ON CONFLICT(path) DO UPDATE SET branch_id=excluded.branch_id,ordinal=excluded.ordinal
                                WHERE excluded.ordinal>knowledge_repository_branches.ordinal",params![path,id,ordinal as i64])?;Ok(())
                        })?;
                    }
                }
                let adapted = if collection == "branches" {
                    ordered_branch_material(&raw.payload, &material["view_ids"], limits)?
                } else {
                    raw.payload.clone()
                };
                charge(&mut work, adapted.len(), limits)?;
                add_material(
                    stage,
                    false,
                    &native,
                    &identity,
                    &kind,
                    node_order,
                    &adapted,
                    &raw.payload_sha256,
                )?;
                node_order += 1;
                root_item(
                    &mut input_root,
                    &raw.id,
                    Digest256::of_bytes(&raw.payload).as_bytes(),
                );
                count += 1;
                if count > entry.expected_count || node_order as u64 > limits.max_rows {
                    return Err(Error::Budget("repository source scan"));
                }
            }
            match page.next_id {
                Some(id) => after = Some(id),
                None => break,
            }
        }
        if count != entry.expected_count
            || input_root.finalize().to_hex() != entry.expected_root_sha256
        {
            return Err(Error::Invalid("repository exact input root"));
        }
        receipt
            .input_roots
            .push((collection.into(), count, entry.expected_root_sha256.clone()));
    }
    // Generate relations from exact originals and their owner-issued ordinal.
    let mut relation_order = 0i64;
    for collection in COLLECTIONS {
        let mut after = -1i64;
        loop {
            let rows=stage.with_connection(WritePhase::Sort,|db| {
                let mut stmt=db.prepare("SELECT o.ordinal,CASE WHEN r.payload_len=length(r.payload) AND length(r.payload)<=?5 THEN r.payload ELSE NULL END,r.payload_sha256 FROM knowledge_repository_order o
                    JOIN raw_records r ON r.collection=o.collection AND r.id=o.raw_id AND r.source_graph=?1
                    WHERE o.collection=?2 AND o.ordinal>?3 ORDER BY o.ordinal LIMIT ?4")?;
                let rows=stmt.query_map(params![source,collection,after,limits.max_page_rows as i64,limits.max_row_bytes as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<Vec<u8>>>(1)?,r.get::<_,Vec<u8>>(2)?)))?;
                rows.collect::<std::result::Result<Vec<_>,_>>().map_err(Error::from)
            })?;
            if rows.is_empty() {
                break;
            }
            for (ordinal, raw, sha) in rows {
                after = ordinal;
                let raw = raw.ok_or(Error::Budget("repository ordered raw bytes"))?;
                charge(&mut work, raw.len(), limits)?;
                if raw.len() > limits.max_row_bytes || sha != Digest256::of_bytes(&raw).as_bytes() {
                    return Err(Error::Invalid("repository ordered raw binding"));
                }
                let source_row = SourceRow::parse(&raw, limits.max_row_bytes)?;
                let item = source_row.value();
                let relation = if collection == "branches" {
                    text(item.get("id")).map(|id|json!({"edge_id":format!("corpus-topology:{id}"),"from_id":root.identity_id,"to_id":format!("branch:{id}"),"predicate_id":"contains",
                        "source_ref":text(item.get("owner_surface")).or_else(||text(item.get("path"))),"view_ids":["corpus-topology"],"authority_layer":item.get("authority_layer"),
                        "properties":{"relation_label":"contains","note":"Структурная связь получена из source_home manifest.","derivation":"source-home-branch-membership"}}))
                } else {
                    let owner = text(item.get("owner_branch"))
                        .or_else(|| text(item.get("declared_path")))
                        .or_else(|| text(item.get("path")));
                    let branch = if let Some(owner) = owner {
                        stage.with_connection(WritePhase::Sort,|db| {
                        db.query_row("SELECT branch_id FROM knowledge_repository_branches
                            WHERE path=?1 OR substr(?1,1,length(path)+1)=path||'/' ORDER BY length(path) DESC LIMIT 1",[owner],|r|r.get::<_,String>(0)).optional().map_err(Error::from)
                    })?
                    } else {
                        None
                    };
                    branch.map(|branch| {
                        let native=text(item.get("id")).or_else(||text(item.get("path"))).map(str::to_owned).unwrap_or_else(||format!("{collection}:{ordinal}"));
                        let path=text(item.get("path")).unwrap_or(&native);
                        let predicate=if collection=="manifests"{"owns_manifest"}else{"owns_resource"};
                        let identity=format!("{}:{native}",if collection=="manifests"{"manifest"}else{"resource"});
                        json!({"edge_id":format!("{predicate}:{branch}:{}",&Digest256::of_bytes(path.as_bytes()).to_hex()[..16]),"from_id":format!("branch:{branch}"),"to_id":identity,"predicate_id":predicate,"source_ref":path,"authority_layer":item.get("authority_layer"),
                            "properties":{"relation_label":predicate.replace('_'," "),"note":format!("Структурная связь выведена из owner_branch для {path}."),"derivation":"indexed-owner-branch","collection":collection,"source_order":ordinal}})
                    })
                };
                if let Some(relation) = relation {
                    let native = required(&relation, "edge_id")?;
                    let adapted = bytes(&relation, limits)?;
                    charge(&mut work, adapted.len(), limits)?;
                    add_material(
                        stage,
                        true,
                        native,
                        "",
                        "",
                        relation_order,
                        &adapted,
                        &Digest256::of_bytes(&raw).to_hex(),
                    )?;
                    relation_order += 1;
                    if node_order as u64 + relation_order as u64 > limits.max_rows {
                        return Err(Error::Budget("repository material rows"));
                    }
                }
            }
        }
    }
    let (nodes, relations, root) = prepared_root(stage, &receipt, limits)?;
    receipt.nodes = nodes;
    receipt.relations = relations;
    receipt.dependency_root_sha256 = root;
    Ok(receipt)
}

pub fn prepare_repository_topology(
    stage: &mut KnowledgeStage<'_>,
    vocab: &QueryVocabulary,
    root: RepositoryRootInput<'_>,
    limits: TopologyLimits,
) -> Result<RepositoryPrepareReceipt> {
    let result = prepare_inner(stage, vocab, root, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}
fn verify(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    limits: TopologyLimits,
) -> Result<()> {
    limits.validate()?;
    if stage.exact_receipt().binding.source_cut != receipt.source_cut {
        return Err(Error::Invalid("repository source cut"));
    }
    let (nodes, relations, root) = prepared_root(stage, receipt, limits)?;
    if nodes != receipt.nodes
        || relations != receipt.relations
        || root != receipt.dependency_root_sha256
    {
        return Err(Error::Invalid("repository prepared root"));
    }
    Ok(())
}

/// Parent runs this scan before global endpoint-placeholder/title closure.
pub fn scan_repository_relation_sources<F>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    limits: TopologyLimits,
    visit: F,
) -> Result<()>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &SourceRow) -> Result<()>,
{
    scan_repository_relation_sources_inner(stage, receipt, limits, None, visit)
}

/// The native placeholder consumer emits at most two bounded nodes per row.
pub(crate) fn scan_repository_placeholder_sources<F>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    limits: TopologyLimits,
    max_placeholder_bytes: usize,
    visit: F,
) -> Result<()>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &SourceRow) -> Result<()>,
{
    if max_placeholder_bytes == 0 || max_placeholder_bytes > 8 * 1024 * 1024 {
        return Err(Error::Budget("repository placeholder row bytes"));
    }
    scan_repository_relation_sources_inner(
        stage,
        receipt,
        limits,
        Some(max_placeholder_bytes),
        visit,
    )
}

fn scan_repository_relation_sources_inner<F>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    limits: TopologyLimits,
    max_placeholder_bytes: Option<usize>,
    mut visit: F,
) -> Result<()>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &SourceRow) -> Result<()>,
{
    let result = (|| {
        verify(stage, receipt, limits)?;
        let mut after = String::new();
        loop {
            let page_rows = if let Some(max_bytes) = max_placeholder_bytes {
                let (write_rows, write_bytes) = stage.write_page_limits();
                limits
                    .max_page_rows
                    .min(write_rows / 2)
                    .min(write_bytes as usize / (2 * max_bytes))
            } else {
                limits.max_page_rows
            };
            let mut page_limits = limits;
            page_limits.max_page_rows = page_rows;
            let rows = material_page(stage, 1, &after, page_limits)?;
            if rows.is_empty() {
                break;
            }
            let mut write_page = |stage: &mut KnowledgeStage<'_>| -> Result<()> {
                for row in &rows {
                    after = row.key.clone();
                    let source = SourceRow::parse(&row.material, limits.max_row_bytes)?;
                    visit(stage, &receipt.source_graph, &source)?;
                }
                Ok(())
            };
            if let Some(max_bytes) = max_placeholder_bytes {
                stage.with_write_page(
                    WritePhase::Normalized,
                    page_rows * 2,
                    (page_rows * 2 * max_bytes) as u64,
                    &mut write_page,
                )?;
            } else {
                write_page(stage)?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

pub fn materialize_repository_nodes(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    normalizer: &KnowledgeBaseNormalizer<'_>,
    limits: TopologyLimits,
) -> Result<u64> {
    let result = (|| {
        verify(stage, receipt, limits)?;
        let mut after = String::new();
        let mut rows = 0;
        let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
            Ok(db.query_row(
                "SELECT coalesce(max(source_order)+1,0) FROM knowledge_nodes",
                [],
                |r| r.get(0),
            )?)
        })?;
        loop {
            let batch = material_page(stage, 0, &after, limits)?;
            if batch.is_empty() {
                break;
            }
            stage.with_write_page(
                WritePhase::Normalized,
                limits.max_page_rows,
                (limits.max_page_rows * limits.max_row_bytes) as u64,
                |stage| {
                    for row in batch {
                        if row.relation {
                            break;
                        }
                        after = row.key.clone();
                        let source = SourceRow::parse(&row.material, limits.max_row_bytes)?;
                        let value = normalizer.normalize_node(
                            &source,
                            &receipt.source_graph,
                            false,
                            BaseNodeOverrides {
                                native_id: Some(&row.native),
                                identity_id: Some(&row.identity),
                                kind_id: if row.kind.is_empty() {
                                    None
                                } else {
                                    Some(&row.kind)
                                },
                            },
                        )?;
                        let payload = bytes(&value, limits)?;
                        stage.insert_node(NodeRow {
                            id: required(&value, "id")?,
                            source_graph: &receipt.source_graph,
                            native_id: Some(required(&value, "native_id")?),
                            entity_id: Some(required(&value, "entity_id")?),
                            kind_id: required(&value, "kind_id")?,
                            type_id: required(&value, "type_id")?,
                            source_order: order,
                            payload: &payload,
                        })?;
                        order += 1;
                        rows += 1;
                    }
                    Ok(())
                },
            )?;
            // Node scan must stop before the relation key space.
            let remaining=stage.with_connection(WritePhase::Sort,|db|Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_repository_material WHERE relation=0 AND material_key>?1)",[&after],|r|r.get::<_,bool>(0))?))?;
            if !remaining {
                break;
            }
        }
        if rows != receipt.nodes {
            return Err(Error::Invalid("repository node completeness"));
        }
        Ok(rows)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

pub fn materialize_repository_relations<F>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    normalizer: &KnowledgeBaseNormalizer<'_>,
    title_source_cut: &str,
    title_root_sha256: &str,
    limits: TopologyLimits,
    mut titles: F,
) -> Result<u64>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<(Value, Value)>,
{
    let result = (|| {
        verify(stage, receipt, limits)?;
        if title_source_cut != receipt.source_cut || Digest256::from_hex(title_root_sha256).is_err()
        {
            return Err(Error::Invalid("repository title closure"));
        }
        let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
            Ok(db.query_row(
                "SELECT coalesce(max(source_order)+1,0) FROM knowledge_relations",
                [],
                |r| r.get(0),
            )?)
        })?;
        let mut after = String::new();
        let mut count = 0;
        let mut work = 0;
        loop {
            let rows = material_page(stage, 1, &after, limits)?;
            if rows.is_empty() {
                break;
            }
            stage.with_write_page(
                WritePhase::Normalized,
                limits.max_page_rows,
                (limits.max_page_rows * limits.max_row_bytes) as u64,
                |stage| {
                    for row in rows {
                        after = row.key;
                        let source = SourceRow::parse(&row.material, limits.max_row_bytes)?;
                        let item = source.value();
                        let from =
                            format!("{}:{}", receipt.source_graph, required(item, "from_id")?);
                        let to = format!("{}:{}", receipt.source_graph, required(item, "to_id")?);
                        let (left, right) = titles(stage, &from, &to)?;
                        let value = normalizer.normalize_relation(
                            &source,
                            &receipt.source_graph,
                            None,
                            &left,
                            &right,
                            "derived-export",
                        )?;
                        let payload = bytes(&value, limits)?;
                        charge(&mut work, payload.len() + row.material.len(), limits)?;
                        stage.insert_relation(RelationRow {
                            id: required(&value, "id")?,
                            source_graph: &receipt.source_graph,
                            native_id: Some(required(&value, "native_id")?),
                            from_id: required(&value, "from_id")?,
                            to_id: required(&value, "to_id")?,
                            predicate_id: required(&value, "predicate_id")?,
                            relation_type_id: required(&value, "relation_type_id")?,
                            source_order: order,
                            payload: &payload,
                        })?;
                        order += 1;
                        count += 1;
                    }
                    Ok(())
                },
            )?;
        }
        if count != receipt.relations {
            return Err(Error::Invalid("repository relation completeness"));
        }
        Ok(count)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Cleanup requires the parent-held exact final node/relation roots, rechecked
/// against the stage. Global coverage/semantic acceptance remains with parent.
pub fn clear_repository_topology(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    final_nodes: &CompleteBaseNodes,
    final_relation_count: u64,
    final_relation_root: &str,
    limits: TopologyLimits,
) -> Result<()> {
    let result = (|| {
        verify(stage, receipt, limits)?;
        let roots = stage.core_roots()?;
        if final_nodes.source_cut != receipt.source_cut
            || roots.nodes != final_nodes.node_count
            || roots.node_sha256 != final_nodes.node_root_sha256
            || roots.relations != final_relation_count
            || roots.relation_sha256 != final_relation_root
        {
            return Err(Error::Invalid("repository final closure"));
        }
        let (nodes, relations): (u64, u64) = stage.with_connection(WritePhase::Sort, |db| {
            Ok((
                db.query_row(
                    "SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1",
                    [&receipt.source_graph],
                    |r| r.get(0),
                )?,
                db.query_row(
                    "SELECT count(*) FROM knowledge_relations WHERE source_graph=?1",
                    [&receipt.source_graph],
                    |r| r.get(0),
                )?,
            ))
        })?;
        if nodes != receipt.nodes || relations != receipt.relations {
            return Err(Error::Invalid("repository final family coverage"));
        }
        stage.with_connection(WritePhase::Finalize,|db|{db.execute_batch("DROP TABLE knowledge_repository_material; DROP TABLE knowledge_repository_order; DROP TABLE knowledge_repository_branches;")?;Ok(())})
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

fn ordered_branch_material(raw: &[u8], views: &Value, limits: TopologyLimits) -> Result<Vec<u8>> {
    use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
    let jl = JsonLimits {
        max_bytes: limits.max_row_bytes,
        ..JsonLimits::default()
    };
    let mut ordered = parse_json(raw, JsonMode::PublishedStrict, jl)
        .map_err(|e| Error::Source(e.to_string()))?
        .into_root();
    let addition = bytes(&json!({"view_ids":views}), limits)?;
    let JsonValue::Object(mut fields) = parse_json(&addition, JsonMode::PublishedStrict, jl)
        .map_err(|e| Error::Source(e.to_string()))?
        .into_root()
    else {
        return Err(Error::Invalid("repository view material"));
    };
    let (key, value) = fields
        .pop()
        .ok_or(Error::Invalid("repository view field"))?;
    let JsonValue::Object(target) = &mut ordered else {
        return Err(Error::Invalid("repository owner object"));
    };
    if let Some((_, v)) = target
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("view_ids"))
    {
        *v = value;
    } else {
        target.push((key, value));
    }
    let mut out = Vec::new();
    crate::knowledge_readable_context::emit_ordered(&ordered, &mut out, limits.max_row_bytes)?;
    Ok(out)
}

/// Ordered derived source witness for the parent's readable finalizer.
/// Both exact prepared root and original-input proof are rechecked.
pub fn repository_material_witness(
    stage: &mut KnowledgeStage<'_>,
    receipt: &RepositoryPrepareReceipt,
    relation: bool,
    normalized_id: &str,
    limits: TopologyLimits,
) -> Result<Vec<u8>> {
    verify(stage, receipt, limits)?;
    let key = normalized_id
        .strip_prefix(&format!("{}:", receipt.source_graph))
        .ok_or(Error::Invalid("repository witness normalized identity"))?;
    stage.with_connection(WritePhase::Sort,|db| {
        let (raw,sha):(Vec<u8>,Vec<u8>)=db.query_row("SELECT CASE WHEN material_len=length(material) AND length(material)<=?3 THEN material ELSE NULL END,material_sha256 FROM knowledge_repository_material WHERE relation=?1 AND material_key=?2",params![relation as i64,key,limits.max_row_bytes as i64],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if sha!=Digest256::of_bytes(&raw).as_bytes(){return Err(Error::Invalid("repository material witness SHA"));}Ok(raw)
    })
}
