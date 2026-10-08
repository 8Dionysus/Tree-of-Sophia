//! Bounded exact-cut canon/candidate preparation. Private proposals retain
//! pack provenance and node-local source order; they are not final graph rows.
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{Error, QueryVocabulary, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, canonical_bytes_v1,
    parse_json,
};

pub const CANON_PROFILE: &str = "canon-node-relation-v1";
pub const CANDIDATE_PROFILE: &str = "candidate-relation-v1";
#[derive(Clone, Copy, Debug)]
pub struct CanonPrepareLimits {
    pub max_nodes: u64,
    pub max_packs: u64,
    pub max_edges: u64,
    pub max_node_relations: u64,
    pub max_page_rows: usize,
    pub max_row_bytes: usize,
    pub max_work_bytes: u64,
}
impl CanonPrepareLimits {
    fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_packs == 0
            || self.max_edges == 0
            || self.max_node_relations == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("canon preparation limits"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct CanonCollectionReceipt {
    pub collection: String,
    pub count: u64,
    pub root_sha256: String,
}
#[derive(Clone, Debug)]
pub struct CanonPrepareReceipt {
    pub source_graph: String,
    pub input_role: String,
    pub adapter_profile: String,
    pub source_cut: String,
    pub nodes: u64,
    pub packs: u64,
    pub relation_edges: u64,
    pub node_relations: u64,
    pub collections: Vec<CanonCollectionReceipt>,
    pub dependency_root_sha256: String,
    pub final_graph_rows_written: bool,
}
pub(crate) fn text(v: Option<&Value>) -> Option<&str> {
    v?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
pub(crate) fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    text(v.get(key))
        .filter(|s| s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("canon required carrier field"))
}
fn exact_id<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    let id = required(v, key)?;
    if v.get(key).and_then(Value::as_str) != Some(id) {
        return Err(Error::Invalid("canon whitespace identity"));
    }
    Ok(id)
}
pub(crate) fn framed(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
fn charge(work: &mut u64, bytes: usize, limits: CanonPrepareLimits) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .ok_or(Error::Budget("canon work"))?;
    if *work > limits.max_work_bytes {
        return Err(Error::Budget("canon work"));
    }
    Ok(())
}
/// Verify the exact retained authored source, including Python native grammar.
/// Digest canonicalization reads the original nested JSON number spellings.
pub(crate) fn canonical_digest(raw: &[u8], item: &Value, max: usize) -> Result<Option<String>> {
    canonical_digest_with_optional_properties(raw, item, max, false)
}

pub(crate) fn canonical_digest_for_prepared_projection(
    raw: &[u8],
    item: &Value,
    max: usize,
) -> Result<Option<String>> {
    canonical_digest_with_optional_properties(raw, item, max, true)
}

fn canonical_digest_with_optional_properties(
    raw: &[u8],
    item: &Value,
    max: usize,
    optional_properties: bool,
) -> Result<Option<String>> {
    let Some(props) = item.get("properties").filter(|v| v.is_object()) else {
        if optional_properties {
            return Ok(None);
        }
        return Err(Error::Invalid("canon properties"));
    };
    if props.get("node_id").is_some()
        && (props.get("node_id") != item.get("node_id")
            || props.get("node_type") != item.get("node_type"))
    {
        return Err(Error::Invalid("canon retained identity/type"));
    }
    if props.get("schema_version").and_then(Value::as_str) != Some("tos_canonical_node_v1") {
        return Ok(None);
    }
    let kind = required(props, "node_type")?;
    let id = required(props, "node_id")?;
    let prefix = format!("tos.{kind}.");
    let suffix = id
        .strip_prefix(&prefix)
        .ok_or(Error::Invalid("canon native identity"))?;
    let valid = suffix.split(['.', '-']).all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    });
    if ![
        "source",
        "concept",
        "principle",
        "lineage",
        "event",
        "state",
        "support",
        "context",
        "analogy",
        "synthesis",
    ]
    .contains(&kind)
        || !valid
        || props.get("record_id").is_some()
        || !props
            .get("record_version")
            .and_then(Value::as_u64)
            .is_some_and(|n| (1..=9007199254740991).contains(&n))
    {
        return Err(Error::Invalid("canon native metadata grammar"));
    }
    let limits = JsonLimits::new(max, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("canon source JSON"))?;
    let document = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let retained = document
        .root()
        .object_get("properties")
        .ok_or(Error::Invalid("canon retained source"))?;
    let bytes = canonical_bytes_v1(retained, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let digest = Digest256::of_bytes(&bytes).to_hex();
    if item.get("source_record_sha256").and_then(Value::as_str) != Some(digest.as_str()) {
        return Err(Error::Invalid("canon retained source digest"));
    }
    Digest256::from_hex(required(item, "source_sha256")?)
        .map_err(|_| Error::Invalid("canon source file digest"))?;
    Ok(Some(digest))
}
fn init(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.create_preparation_tables(crate::knowledge_stage::preparation_schema!(
            table r#"IF NOT EXISTS knowledge_canon_nodes(source_graph TEXT NOT NULL,node_id TEXT NOT NULL,source_path TEXT NOT NULL,raw_sha256 BLOB NOT NULL,PRIMARY KEY(source_graph,node_id)) WITHOUT ROWID"#,
            index r#"IF NOT EXISTS knowledge_canon_source_paths ON knowledge_canon_nodes(source_path,node_id)"#,
            table r#"IF NOT EXISTS knowledge_canon_packs(source_graph TEXT NOT NULL,pack_id TEXT NOT NULL,path TEXT NOT NULL,owner_branch TEXT,edge_count INTEGER,raw_sha256 BLOB NOT NULL,PRIMARY KEY(source_graph,pack_id)) WITHOUT ROWID"#,
            table r#"IF NOT EXISTS knowledge_canon_proposals(source_graph TEXT NOT NULL,identity_id TEXT NOT NULL,native_id TEXT NOT NULL,origin_collection TEXT NOT NULL,origin_id TEXT NOT NULL,pack_id TEXT,material BLOB NOT NULL,material_sha256 BLOB NOT NULL,origin_sha256 BLOB NOT NULL,PRIMARY KEY(source_graph,identity_id)) WITHOUT ROWID"#,
            index r#"IF NOT EXISTS knowledge_canon_proposal_packs ON knowledge_canon_proposals(source_graph,pack_id)"#
        ))
}

pub(crate) fn verify_public_projection_stage(stage: &KnowledgeStage<'_>) -> Result<()> {
    let binding = &stage.exact_receipt()?.binding;
    let public_projection = stage.public_build()
        && binding.owner_profile == "tos-public-projection-snapshot-v1"
        && binding
            .source_cut
            .strip_prefix("public-projection:")
            .is_some_and(|revision| !revision.is_empty())
        && binding.through_commit_seq == 0
        && binding.index_generation == "public-d1-v9"
        && binding.route_map_version == "public-d1-v9"
        && binding.reader_abi == "public-d1-v9"
        && binding.complete;
    let native_projection = !stage.public_build()
        && binding.owner_profile == "tos-native-projection-snapshot-v1"
        && binding
            .source_cut
            .strip_prefix("native-projection:")
            .is_some_and(|revision| !revision.is_empty())
        && binding.through_commit_seq == 0
        && binding.route_map_version == "tos-access-runtime-data-v1"
        && binding.complete;
    if !public_projection && !native_projection {
        return Err(Error::Invalid("canon prepared projection binding"));
    }
    Ok(())
}

fn python_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Number(value)) => value.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Object(value)) => !value.is_empty(),
        Some(Value::Bool(true)) => true,
    }
}

fn add_owner_relation_view(views: &mut std::collections::BTreeSet<String>, owner: Option<&str>) {
    match owner {
        Some("ToS/canon") => {
            views.insert("route-graph".into());
        }
        Some("ToS/candidate-intake") => {
            views.insert("promotion-flow".into());
        }
        _ => {}
    }
}

fn insert_proposal(
    stage: &mut KnowledgeStage<'_>,
    graph: &str,
    identity: &str,
    native: &str,
    collection: &str,
    origin: &str,
    pack: Option<&str>,
    material: &Value,
    sha: &[u8],
    work: &mut u64,
    limits: CanonPrepareLimits,
) -> Result<()> {
    let bytes = serde_json::to_vec(material).map_err(|_| Error::Invalid("canon proposal JSON"))?;
    if bytes.len() > limits.max_row_bytes {
        return Err(Error::Budget("canon proposal bytes"));
    }
    charge(work, bytes.len(), limits)?;
    stage.charge_materialized(1, bytes.len() as u64)?;
    stage.with_connection(WritePhase::Normalized, |db| {
        db.execute(
            "INSERT INTO knowledge_canon_proposals VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                graph,
                identity,
                native,
                collection,
                origin,
                pack,
                &bytes,
                Digest256::of_bytes(&bytes).as_bytes().as_slice(),
                sha
            ],
        )?;
        Ok(())
    })
}
pub(crate) fn dependency_root(stage: &mut KnowledgeStage<'_>, graph: &str) -> Result<String> {
    stage.with_connection(WritePhase::Sort,|db| {
        let mut hash=Digest256Hasher::new(); hash.update(b"tos-canon-prepared-v1\0"); framed(&mut hash,graph);
        for (tag,sql,columns) in [
            ("nodes","SELECT node_id,hex(raw_sha256),source_path FROM knowledge_canon_nodes WHERE source_graph=?1 ORDER BY node_id",3),
            ("packs","SELECT pack_id,hex(raw_sha256),path,coalesce(owner_branch,''),coalesce(CAST(edge_count AS TEXT),'') FROM knowledge_canon_packs WHERE source_graph=?1 ORDER BY pack_id",5),
            ("proposals","SELECT identity_id,hex(material_sha256),native_id,origin_collection,origin_id,coalesce(pack_id,''),hex(origin_sha256) FROM knowledge_canon_proposals WHERE source_graph=?1 ORDER BY identity_id",7),
        ] { framed(&mut hash,tag); let mut statement=db.prepare(sql)?; let mut rows=statement.query([graph])?;
            while let Some(row)=rows.next()? { for i in 0..columns { let s:String=row.get(i)?; framed(&mut hash,&s); } }
        } Ok(hash.finalize().to_hex())
    })
}
pub(crate) fn prepare_family(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    profile: &str,
    limits: CanonPrepareLimits,
) -> Result<CanonPrepareReceipt> {
    prepare_family_mode(stage, vocabulary, profile, limits, false)
}

fn prepare_family_mode(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    profile: &str,
    limits: CanonPrepareLimits,
    prepared_projection: bool,
) -> Result<CanonPrepareReceipt> {
    limits.validate()?;
    if prepared_projection {
        verify_public_projection_stage(stage)?;
    }
    let sources = vocabulary
        .sources
        .iter()
        .filter(|s| s.adapter_profile == profile)
        .collect::<Vec<_>>();
    if sources.len() != 1 {
        return Err(Error::Invalid("canon selected adapter"));
    }
    let selected = sources[0];
    let graph = selected.source_graph_id.clone();
    let canonical = profile == CANON_PROFILE;
    let owner = if canonical {
        "ToS/canon"
    } else {
        "ToS/candidate-intake"
    };
    let names: &[&str] = if canonical {
        &["nodes", "relation_packs", "relation_edges"]
    } else {
        &["relation_packs", "relation_edges"]
    };
    let registered = stage
        .exact_receipt()?
        .collections
        .iter()
        .filter(|c| c.source_graph == graph)
        .cloned()
        .collect::<Vec<_>>();
    if registered.len() != names.len()
        || registered.iter().any(|c| {
            !names.contains(&c.collection.as_str())
                || c.adapter_profile != profile
                || c.input_role != selected.input_role
        })
        || names
            .iter()
            .any(|name| registered.iter().filter(|c| c.collection == *name).count() != 1)
    {
        return Err(Error::Invalid("canon exact collection registration"));
    }
    let mut receipt = CanonPrepareReceipt {
        source_graph: graph.clone(),
        input_role: selected.input_role.clone(),
        adapter_profile: profile.into(),
        source_cut: stage.exact_receipt()?.binding.source_cut.clone(),
        nodes: 0,
        packs: 0,
        relation_edges: 0,
        node_relations: 0,
        collections: Vec::new(),
        dependency_root_sha256: String::new(),
        final_graph_rows_written: false,
    };
    init(stage)?;
    let mut work = 0;
    for &collection in names {
        let expected = registered
            .iter()
            .find(|r| r.collection == collection)
            .expect("checked collection");
        let cap = match collection {
            "nodes" => limits.max_nodes,
            "relation_packs" => limits.max_packs,
            _ => limits.max_edges,
        };
        if expected.expected_count > cap {
            return Err(Error::Budget("canon registered count"));
        }
        let mut count = 0u64;
        let mut root = Digest256Hasher::new();
        let mut after = None;
        loop {
            let page =
                stage.scan_input(&graph, collection, after.as_deref(), limits.max_page_rows)?;
            for raw in page.rows {
                charge(&mut work, raw.payload.len(), limits)?;
                count = count.checked_add(1).ok_or(Error::Budget("canon rows"))?;
                if count > cap
                    || raw.payload.len() > limits.max_row_bytes
                    || raw.payload_sha256 != Digest256::of_bytes(&raw.payload).to_hex()
                {
                    return Err(Error::Invalid("canon raw count/digest"));
                }
                framed(&mut root, &raw.id);
                let sha = Digest256::of_bytes(&raw.payload);
                root.update(sha.as_bytes());
                let source = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
                let item = source.value();
                match collection {
                    "nodes" => {
                        let id = exact_id(item, "node_id")?;
                        if id != raw.id {
                            return Err(Error::Invalid("canon raw node identity"));
                        }
                        if !prepared_projection {
                            required(item, "node_type")?;
                        }
                        let path = if prepared_projection {
                            text(item.get("source_path")).unwrap_or("")
                        } else {
                            required(item, "source_path")?
                        };
                        if path.len() > 4096 || path.contains('\0') {
                            return Err(Error::Invalid("canon source path"));
                        }
                        if prepared_projection {
                            canonical_digest_for_prepared_projection(
                                &raw.payload,
                                item,
                                limits.max_row_bytes,
                            )?;
                        } else {
                            canonical_digest(&raw.payload, item, limits.max_row_bytes)?;
                        }
                        stage.charge_materialized(1, (id.len() + path.len() + 32) as u64)?;
                        stage.with_connection(WritePhase::Normalized, |db| {
                            db.execute(
                                "INSERT INTO knowledge_canon_nodes VALUES(?1,?2,?3,?4)",
                                params![graph, id, path, sha.as_bytes().as_slice()],
                            )?;
                            Ok(())
                        })?;
                        let props = &item["properties"];
                        let field = if props.get("relations").is_some_and(Value::is_array) {
                            "relations"
                        } else {
                            "lineage_relations"
                        };
                        for (order, relation) in props
                            .get(field)
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter(|v| v.is_object())
                            .enumerate()
                        {
                            let (Some(predicate), Some(target)) = (
                                text(relation.get("relation")),
                                text(relation.get("target_ref")),
                            ) else {
                                continue;
                            };
                            receipt.node_relations = receipt
                                .node_relations
                                .checked_add(1)
                                .ok_or(Error::Budget("canon local relations"))?;
                            if receipt.node_relations > limits.max_node_relations {
                                return Err(Error::Budget("canon local relations"));
                            }
                            let framed = format!("{id}\0{field}\0{order}\0{predicate}\0{target}");
                            let digest = Digest256::of_bytes(framed.as_bytes()).to_hex();
                            let edge = format!("node-relation:{}", &digest[..24]);
                            let source_ref = if path.is_empty() {
                                Value::Null
                            } else {
                                json!(path)
                            };
                            let material = json!({"edge_id":edge,"from_id":id,"to_id":target,"predicate_id":predicate,"source_ref":source_ref,"graph_layers":["authored-node-relation"],"authority_layer":text(item.get("authority_layer")).unwrap_or("canon"),"properties":{"relation_label":crate::knowledge_philosophy_display::humanize(predicate),"derivation":"authored-node-contract-relation","source_field":format!("{field}[{order}]"),"review_status":"source-recorded"}});
                            insert_proposal(
                                stage,
                                &graph,
                                &edge,
                                &edge,
                                "nodes",
                                id,
                                None,
                                &material,
                                sha.as_bytes(),
                                &mut work,
                                limits,
                            )?;
                        }
                    }
                    "relation_packs" => {
                        let (id, path, pack_owner, edges) = if prepared_projection {
                            let id = item
                                .get("pack_id")
                                .and_then(Value::as_str)
                                .filter(|value| !value.trim().is_empty())
                                .ok_or(Error::Invalid("canon prepared pack identity"))?;
                            let path = item
                                .get("path")
                                .and_then(Value::as_str)
                                .filter(|value| !value.trim().is_empty())
                                .ok_or(Error::Invalid("canon prepared pack path"))?;
                            if id != raw.id || path.len() > limits.max_row_bytes {
                                return Err(Error::Invalid("canon prepared pack binding"));
                            }
                            let owner_branch = item.get("owner_branch").and_then(Value::as_str);
                            let edge_count = item
                                .get("edge_count")
                                .and_then(Value::as_u64)
                                .filter(|count| {
                                    *count <= limits.max_edges && *count <= i64::MAX as u64
                                })
                                .map(|count| count as i64);
                            (id, path, owner_branch, edge_count)
                        } else {
                            let id = exact_id(item, "pack_id")?;
                            if id != raw.id || required(item, "owner_branch")? != owner {
                                return Err(Error::Invalid("canon pack identity/owner"));
                            }
                            let path = required(item, "path")?;
                            let edges = item
                                .get("edge_count")
                                .and_then(Value::as_u64)
                                .filter(|n| *n <= limits.max_edges && *n <= i64::MAX as u64)
                                .ok_or(Error::Invalid("canon pack edge count"))?;
                            Digest256::from_hex(required(item, "sha256")?)
                                .map_err(|_| Error::Invalid("canon pack source digest"))?;
                            (id, path, Some(owner), Some(edges as i64))
                        };
                        stage.charge_materialized(
                            1,
                            (id.len() + path.len() + pack_owner.map_or(0, str::len) + 40) as u64,
                        )?;
                        stage.with_connection(WritePhase::Normalized, |db| {
                            db.execute(
                                "INSERT INTO knowledge_canon_packs VALUES(?1,?2,?3,?4,?5,?6)",
                                params![
                                    graph,
                                    id,
                                    path,
                                    pack_owner,
                                    edges,
                                    sha.as_bytes().as_slice()
                                ],
                            )?;
                            Ok(())
                        })?;
                    }
                    _ => {
                        let edge = if prepared_projection {
                            text(item.get("edge_id"))
                                .or_else(|| text(item.get("id")))
                                .ok_or(Error::Invalid("canon prepared edge identity"))?
                        } else {
                            required(item, "edge_id")?
                        };
                        let pack_value = if prepared_projection {
                            if item
                                .get("pack_id")
                                .is_some_and(|pack| !pack.is_null() && !pack.is_string())
                            {
                                return Err(Error::Invalid("canon prepared relation pack ID type"));
                            }
                            item.get("pack_id")
                                .and_then(Value::as_str)
                                .filter(|value| !value.trim().is_empty())
                        } else {
                            Some(exact_id(item, "pack_id")?)
                        };
                        let pack = pack_value.map(str::trim);
                        let identity = pack
                            .map(|pack| format!("{pack}:{edge}"))
                            .unwrap_or_else(|| edge.to_owned());
                        // Staging may use bare IDs if globally unique, or pack-qualified IDs.
                        if (prepared_projection && raw.id != identity)
                            || (!prepared_projection && raw.id != edge && raw.id != identity)
                        {
                            return Err(Error::Invalid("canon staged edge identity"));
                        }
                        if !prepared_projection && required(item, "owner_branch")? != owner {
                            return Err(Error::Invalid("canon edge owner"));
                        }
                        let path: Option<String> = if let Some(pack_key) = pack_value {
                            stage.with_connection(WritePhase::Sort, |db| {
                                db.query_row(
                                    "SELECT path FROM knowledge_canon_packs WHERE source_graph=?1 AND pack_id=?2",
                                    params![graph, pack_key],
                                    |r| r.get(0),
                                )
                                .optional()
                                .map_err(Error::from)
                            })?
                        } else {
                            None
                        };
                        if !prepared_projection && path.is_none() {
                            return Err(Error::Invalid("canon unknown relation pack"));
                        }
                        let mut material = item.clone();
                        let source_ref_is_falsey = if prepared_projection {
                            !python_truthy(item.get("source_ref"))
                        } else {
                            !item.get("source_ref").is_some_and(|value| match value {
                                Value::Null => false,
                                Value::String(value) => !value.is_empty(),
                                Value::Bool(value) => *value,
                                _ => true,
                            })
                        };
                        if source_ref_is_falsey {
                            if let Some(path) = path.as_ref().filter(|path| !path.is_empty()) {
                                material["source_ref"] = json!(path);
                            }
                        }
                        let mut views = item
                            .get("view_ids")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v.as_str())
                            .filter(|v| !v.is_empty())
                            .map(str::to_owned)
                            .collect::<std::collections::BTreeSet<_>>();
                        add_owner_relation_view(
                            &mut views,
                            item.get("owner_branch").and_then(Value::as_str),
                        );
                        material["view_ids"] = json!(views);
                        insert_proposal(
                            stage,
                            &graph,
                            &identity,
                            edge,
                            collection,
                            &raw.id,
                            pack,
                            &material,
                            sha.as_bytes(),
                            &mut work,
                            limits,
                        )?;
                    }
                }
            }
            match page.next_id {
                Some(next) => after = Some(next),
                None => break,
            }
        }
        let digest = root.finalize().to_hex();
        if count != expected.expected_count || digest != expected.expected_root_sha256 {
            return Err(Error::Invalid("canon exact input root/count"));
        }
        match collection {
            "nodes" => receipt.nodes = count,
            "relation_packs" => receipt.packs = count,
            _ => receipt.relation_edges = count,
        }
        receipt.collections.push(CanonCollectionReceipt {
            collection: collection.into(),
            count,
            root_sha256: digest,
        });
    }
    if !prepared_projection {
        stage.with_connection(WritePhase::Sort, |db| {
            let mismatch: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM knowledge_canon_packs p WHERE source_graph=?1 AND edge_count!=(SELECT count(*) FROM knowledge_canon_proposals e WHERE e.source_graph=p.source_graph AND e.pack_id=p.pack_id))",
                [&graph],
                |r| r.get(0),
            )?;
            if mismatch {
                return Err(Error::Invalid("canon pack edge-count closure"));
            }
            Ok(())
        })?;
    }
    receipt.dependency_root_sha256 = dependency_root(stage, &graph)?;
    Ok(receipt)
}
pub fn prepare_canon_inputs(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: CanonPrepareLimits,
) -> Result<CanonPrepareReceipt> {
    let result = prepare_family(stage, vocabulary, CANON_PROFILE, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

pub(crate) fn prepare_public_projection_family(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    profile: &str,
    limits: CanonPrepareLimits,
) -> Result<CanonPrepareReceipt> {
    let result = if profile == CANON_PROFILE || profile == CANDIDATE_PROFILE {
        prepare_family_mode(stage, vocabulary, profile, limits, true)
    } else {
        Err(Error::Invalid("canon prepared projection profile"))
    };
    if result.is_err() {
        stage.poison();
    }
    result
}
/// Private index cleanup follows all-source normalization/finalization.
pub fn clear_canon_prepare(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Finalize,|db|{db.execute_batch("DROP TABLE knowledge_canon_proposals;DROP TABLE knowledge_canon_packs;DROP TABLE knowledge_canon_nodes;")?;Ok(())})
}

#[cfg(test)]
mod tests {
    use super::add_owner_relation_view;
    use std::collections::BTreeSet;

    #[test]
    fn relation_views_follow_exact_declared_owner() {
        let mut views = BTreeSet::new();
        add_owner_relation_view(&mut views, Some("ToS/future"));
        add_owner_relation_view(&mut views, None);
        assert!(views.is_empty());

        add_owner_relation_view(&mut views, Some("ToS/candidate-intake"));
        assert_eq!(views.len(), 1);
        assert!(views.contains("promotion-flow"));
    }
}
