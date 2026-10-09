//! Exact ordered philosophy projection input retained under the selected lease.
//! Mechanical custody only: authored source and current policy retain authority.
use crate::d1_public_capture::CreationState;
use crate::knowledge_selected::{ColdOpenLimits, KnowledgeSelectedExpectation};
use crate::knowledge_selected::{
    owned_schema_equal, owned_schema_sql_error, owned_schema_step, with_owned_schema_statement,
};
use crate::knowledge_stage::{
    InputCollectionReceipt, KNOWLEDGE_CARRIER_ONCE_MODEL_ABI, KnowledgePayloadLayout,
    KnowledgeStage, WritePhase,
};
use crate::{Error, NavigationOriginalLimits, PhilosophyPrepareReceipt, QueryVocabulary, Result};
use rusqlite::{Connection, OptionalExtension, params};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, JsonMode, parse_json};

pub const PHILOSOPHY_ORIGINAL_PROFILE: &str = "tos_philosophy_original_v1";
pub const KNOWLEDGE_PHILOSOPHY_MODEL_ABI: &str = tos_foundation::KNOWLEDGE_MODEL_ABI_V4_POSTINGS_V1;
pub(crate) const META_TABLE: &str = "philosophy_original_meta";
pub(crate) const ROW_TABLE: &str = "philosophy_original_rows";
pub(crate) const META_DDL: &str = "CREATE TABLE philosophy_original_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1),receipt BLOB NOT NULL)";
pub(crate) const ROW_DDL: &str = "CREATE TABLE philosophy_original_rows(collection TEXT NOT NULL,ordinal INTEGER NOT NULL CHECK(ordinal>=0),id TEXT NOT NULL,packet_len INTEGER NOT NULL,packet_sha256 BLOB NOT NULL,packet BLOB NOT NULL,PRIMARY KEY(collection,ordinal),UNIQUE(collection,id)) WITHOUT ROWID";
pub(crate) const ROW_DDL_CARRIER: &str = "CREATE TABLE philosophy_original_rows(collection TEXT NOT NULL,ordinal INTEGER NOT NULL CHECK(ordinal>=0),id TEXT NOT NULL,packet_len INTEGER NOT NULL,packet_sha256 BLOB NOT NULL,PRIMARY KEY(collection,ordinal),UNIQUE(collection,id)) WITHOUT ROWID";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhilosophyOriginalCollection {
    Header,
    Nodes,
    Edges,
}
impl PhilosophyOriginalCollection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Nodes => "nodes",
            Self::Edges => "edges",
        }
    }
}
pub struct PhilosophyOriginalInput<'a> {
    pub header: &'a [u8],
    pub expected_header_sha256: &'a str,
    pub nodes: &'a [&'a [u8]],
    pub edges: &'a [&'a [u8]],
    pub expected_nodes_root_sha256: &'a str,
    pub expected_edges_root_sha256: &'a str,
    /// Same physical resource profile as the retained navigation originals.
    pub limits: NavigationOriginalLimits,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhilosophyOriginalReceipt {
    pub profile: String,
    pub descriptor_sha256: String,
    pub source_cut: String,
    pub membership_root: String,
    pub source_graph: String,
    pub nodes: u64,
    pub edges: u64,
    pub node_input_root_sha256: String,
    pub edge_input_root_sha256: String,
    pub header_sha256: String,
    pub nodes_root_sha256: String,
    pub edges_root_sha256: String,
    pub component_root_sha256: String,
    pub total_bytes: u64,
}
#[derive(Debug)]
pub struct PhilosophyOriginalRow {
    pub ordinal: u64,
    pub raw_sha256: String,
    pub raw: Vec<u8>,
}
#[derive(Debug)]
pub struct PhilosophyOriginalPage {
    pub rows: Vec<PhilosophyOriginalRow>,
    pub next_ordinal: Option<u64>,
    pub decoded_bytes: u64,
    /// Zero under the caller's retained SQLite progress handler.
    pub vm_steps: u64,
}
fn text(h: &mut Digest256Hasher, s: &str) {
    h.update(&(s.len() as u64).to_be_bytes());
    h.update(s.as_bytes());
}
fn ordered_hash(collection: &str) -> Digest256Hasher {
    let mut h = Digest256Hasher::new();
    text(&mut h, PHILOSOPHY_ORIGINAL_PROFILE);
    text(&mut h, collection);
    h
}
fn ordered_item(h: &mut Digest256Hasher, ordinal: u64, raw: &[u8]) {
    h.update(&ordinal.to_be_bytes());
    h.update(&(raw.len() as u64).to_be_bytes());
    h.update(Digest256::of_bytes(raw).as_bytes());
}
/// Independent producer ordering declaration over exactly the supplied input rows.
pub fn philosophy_original_rows_root(
    collection: PhilosophyOriginalCollection,
    rows: &[&[u8]],
) -> String {
    philosophy_original_rows_root_with_check(collection, rows, &mut |_| Ok(()))
        .expect("infallible original hash callback")
}

pub(crate) fn philosophy_original_rows_root_with_check(
    collection: PhilosophyOriginalCollection,
    rows: &[&[u8]],
    check: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<String> {
    check(0)?;
    let mut h = ordered_hash(collection.as_str());
    for (i, raw) in rows.iter().enumerate() {
        check(raw.len())?;
        ordered_item(&mut h, i as u64, raw);
    }
    check(0)?;
    Ok(h.finalize().to_hex())
}

fn root(r: &PhilosophyOriginalReceipt) -> Result<String> {
    let mut h = Digest256Hasher::new();
    for s in [
        &r.profile,
        &r.descriptor_sha256,
        &r.source_cut,
        &r.membership_root,
        &r.source_graph,
    ] {
        if s.is_empty() || s.len() > 4096 {
            return Err(Error::Invalid("philosophy original binding text"));
        }
        text(&mut h, s);
    }
    for n in [r.nodes, r.edges, r.total_bytes] {
        h.update(&n.to_be_bytes());
    }
    for sha in [
        &r.node_input_root_sha256,
        &r.edge_input_root_sha256,
        &r.header_sha256,
        &r.nodes_root_sha256,
        &r.edges_root_sha256,
    ] {
        h.update(
            Digest256::from_hex(sha)
                .map_err(|_| Error::Invalid("philosophy original digest"))?
                .as_bytes(),
        );
    }
    Ok(h.finalize().to_hex())
}
fn object(raw: &[u8], cap: usize) -> Result<serde_json::Value> {
    let l = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("philosophy original JSON limits"))?;
    parse_json(raw, JsonMode::PublishedStrict, l).map_err(|e| Error::Source(e.to_string()))?;
    let v: serde_json::Value =
        serde_json::from_slice(raw).map_err(|_| Error::Invalid("philosophy original JSON"))?;
    if !v.is_object() {
        return Err(Error::Invalid("philosophy original object"));
    }
    Ok(v)
}
fn header(raw: &[u8], r: &PhilosophyOriginalReceipt, cap: usize) -> Result<()> {
    let v = object(raw, cap)?;
    if v.get("nodes").is_some()
        || v.get("edges").is_some()
        || v["schema_version"].as_str().is_none_or(str::is_empty)
        || v["counts"]["nodes"].as_u64() != Some(r.nodes)
        || v["counts"]["edges"].as_u64() != Some(r.edges)
        || Digest256::of_bytes(raw).to_hex() != r.header_sha256
    {
        return Err(Error::Invalid("philosophy original detached header"));
    }
    Ok(())
}
pub(crate) fn validate_producer_receipt(
    r: &PhilosophyOriginalReceipt,
    inputs: &[InputCollectionReceipt],
) -> Result<()> {
    if r.profile != PHILOSOPHY_ORIGINAL_PROFILE || root(r)? != r.component_root_sha256 {
        return Err(Error::Invalid("philosophy original producer root"));
    }
    for (collection, count, sha) in [
        ("nodes", r.nodes, &r.node_input_root_sha256),
        ("edges", r.edges, &r.edge_input_root_sha256),
    ] {
        let matches = inputs
            .iter()
            .filter(|i| i.source_graph == r.source_graph && i.collection == collection)
            .collect::<Vec<_>>();
        if matches.len() != 1
            || matches[0].adapter_profile != "philosophy-node-edge-v1"
            || matches[0].expected_count != count
            || &matches[0].expected_root_sha256 != sha
        {
            return Err(Error::Invalid("philosophy original input receipt"));
        }
    }
    Ok(())
}
/// Retain the real input ordering before normalization; raw_records has no order.
/// Every supplied row must exactly match a complete declared same-cut stage input.
pub fn retain_philosophy_original(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    prepared: &PhilosophyPrepareReceipt,
    input: &PhilosophyOriginalInput<'_>,
) -> Result<PhilosophyOriginalReceipt> {
    let result = (|| {
        input.limits.validate()?;
        let binding = stage.exact_receipt()?.binding.clone();
        let source = vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == "philosophy-node-edge-v1")
            .collect::<Vec<_>>();
        if source.len() != 1
            || source[0].source_graph_id != prepared.source_graph
            || source[0].input_role != prepared.input_role
            || prepared.source_cut != binding.source_cut
            || prepared.nodes != input.nodes.len() as u64
            || prepared.edges != input.edges.len() as u64
        {
            return Err(Error::Invalid("philosophy original selected producer"));
        }
        if prepared
            .nodes
            .checked_add(prepared.edges)
            .and_then(|n| n.checked_add(1))
            .is_none_or(|n| n > input.limits.max_rows)
        {
            return Err(Error::Budget("philosophy original rows"));
        }
        let mut total = input.header.len() as u64;
        if input.header.is_empty() || input.header.len() > input.limits.max_row_bytes {
            return Err(Error::Budget("philosophy original header bytes"));
        }
        for (collection, key, rows, expected) in [
            (
                "nodes",
                "node_id",
                input.nodes,
                input.expected_nodes_root_sha256,
            ),
            (
                "edges",
                "edge_id",
                input.edges,
                input.expected_edges_root_sha256,
            ),
        ] {
            let mut seen = std::collections::BTreeSet::new();
            let mut h = ordered_hash(collection);
            for (i, raw) in rows.iter().enumerate() {
                let v = object(raw, input.limits.max_row_bytes)?;
                let id = v[key]
                    .as_str()
                    .filter(|id| !id.is_empty() && id.len() <= 4096)
                    .ok_or(Error::Invalid("philosophy original row ID"))?;
                if !seen.insert(id.to_owned()) {
                    return Err(Error::Invalid("philosophy original duplicate ID"));
                }
                let selected = stage
                    .raw_by_id(&prepared.source_graph, collection, id)?
                    .ok_or(Error::Invalid("philosophy original absent stage row"))?;
                if selected.payload.as_slice() != *raw {
                    return Err(Error::Invalid("philosophy original stage bytes differ"));
                }
                total = total
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= input.limits.max_total_bytes)
                    .ok_or(Error::Budget("philosophy original total bytes"))?;
                ordered_item(&mut h, i as u64, raw);
                stage.charge(raw)?;
            }
            if h.finalize().to_hex() != expected {
                return Err(Error::Invalid("philosophy original independent order root"));
            }
        }
        if total > input.limits.max_total_bytes {
            return Err(Error::Budget("philosophy original total bytes"));
        }
        let mut r = PhilosophyOriginalReceipt {
            profile: PHILOSOPHY_ORIGINAL_PROFILE.into(),
            descriptor_sha256: vocabulary.descriptor_sha256.clone(),
            source_cut: binding.source_cut,
            membership_root: binding.membership_root,
            source_graph: prepared.source_graph.clone(),
            nodes: prepared.nodes,
            edges: prepared.edges,
            node_input_root_sha256: prepared.node_input_root_sha256.clone(),
            edge_input_root_sha256: prepared.edge_input_root_sha256.clone(),
            header_sha256: input.expected_header_sha256.into(),
            nodes_root_sha256: input.expected_nodes_root_sha256.into(),
            edges_root_sha256: input.expected_edges_root_sha256.into(),
            component_root_sha256: String::new(),
            total_bytes: total,
        };
        header(input.header, &r, input.limits.max_row_bytes)?;
        r.component_root_sha256 = root(&r)?;
        validate_producer_receipt(&r, &stage.exact_receipt()?.collections)?;
        let receipt = serde_json::to_vec(&r)
            .map_err(|_| Error::Invalid("philosophy original receipt encoding"))?;
        if receipt.len() > JsonLimits::default().max_bytes {
            return Err(Error::Budget("philosophy original receipt bytes"));
        }
        stage.charge(input.header)?;
        let layout = stage.payload_layout();
        let physical_bytes = if layout.uses_carriers() {
            let mut bytes = 0u64;
            let header_rows = [input.header];
            for (collection, key, rows) in [
                ("header", "", header_rows.as_slice()),
                ("nodes", "node_id", input.nodes),
                ("edges", "edge_id", input.edges),
            ] {
                for raw in rows {
                    // Planning and insertion each parse twice; insertion also hashes.
                    // Carrier retention owns its separate hash and physical-row debit.
                    stage.charge_preparation_work(
                        (raw.len() as u64)
                            .checked_mul(5)
                            .ok_or(Error::Budget("philosophy original carrier planning work"))?,
                    )?;
                    let value = object(raw, input.limits.max_row_bytes)?;
                    let id = if key.is_empty() {
                        ""
                    } else {
                        value[key]
                            .as_str()
                            .filter(|id| !id.is_empty() && id.len() <= 4096)
                            .ok_or(Error::Invalid("philosophy original row ID"))?
                    };
                    let row_bytes = 48u64
                        .checked_add(collection.len() as u64)
                        .and_then(|n| n.checked_add(id.len() as u64))
                        .ok_or(Error::Budget("philosophy original metadata bytes"))?;
                    stage.retain_exact_source_carrier_for_family(collection, raw)?;
                    bytes = bytes
                        .checked_add(row_bytes)
                        .ok_or(Error::Budget("philosophy original metadata bytes"))?;
                }
            }
            bytes
        } else {
            total
        };
        stage.charge_materialized(
            r.nodes + r.edges + 2,
            physical_bytes
                .checked_add(receipt.len() as u64)
                .ok_or(Error::Budget("philosophy original materialized bytes"))?,
        )?;
        stage.with_connection(WritePhase::Finalize, |db| {
            let tx = db.transaction()?;
            tx.execute_batch(META_DDL)?;
            tx.execute_batch(if layout.uses_carriers() {
                ROW_DDL_CARRIER
            } else {
                ROW_DDL
            })?;
            tx.execute(
                "INSERT INTO philosophy_original_meta VALUES(1,?1)",
                [receipt.as_slice()],
            )?;
            {
                let mut q = tx.prepare(if layout.uses_carriers() {
                    "INSERT INTO philosophy_original_rows VALUES(?1,?2,?3,?4,?5)"
                } else {
                    "INSERT INTO philosophy_original_rows VALUES(?1,?2,?3,?4,?5,?6)"
                })?;
                let header_rows = [input.header];
                for (collection, key, rows) in [
                    ("header", "", header_rows.as_slice()),
                    ("nodes", "node_id", input.nodes),
                    ("edges", "edge_id", input.edges),
                ] {
                    for (ordinal, raw) in rows.iter().enumerate() {
                        let v = object(raw, input.limits.max_row_bytes)?;
                        let id = if key.is_empty() {
                            ""
                        } else {
                            v[key]
                                .as_str()
                                .ok_or(Error::Invalid("philosophy original row ID"))?
                        };
                        let digest = Digest256::of_bytes(raw);
                        if layout.uses_carriers() {
                            q.execute(params![
                                collection,
                                ordinal as i64,
                                id,
                                raw.len() as i64,
                                digest.as_bytes().as_slice()
                            ])?;
                        } else {
                            q.execute(params![
                                collection,
                                ordinal as i64,
                                id,
                                raw.len() as i64,
                                digest.as_bytes().as_slice(),
                                raw
                            ])?;
                        }
                    }
                }
            }
            tx.commit()?;
            Ok(())
        })?;
        Ok(r)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
pub(crate) fn present(db: &Connection) -> Result<bool> {
    let mut n = 0;
    for table in [META_TABLE, ROW_TABLE] {
        n += usize::from(
            db.query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some(),
        );
    }
    if n != 0 && n != 2 {
        return Err(Error::Invalid("philosophy original partial tables"));
    }
    Ok(n == 2)
}
pub(crate) fn verify_ddl(db: &Connection) -> Result<()> {
    verify_ddl_with_layout(db, KnowledgePayloadLayout::InlineV1)
}
pub(crate) fn verify_ddl_with_layout(
    db: &Connection,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    for (table, ddl) in [
        (META_TABLE, META_DDL),
        (
            ROW_TABLE,
            if layout.uses_carriers() {
                ROW_DDL_CARRIER
            } else {
                ROW_DDL
            },
        ),
    ] {
        let actual:Option<String>=db.query_row("SELECT CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=4096 THEN sql ELSE NULL END FROM sqlite_master WHERE type='table' AND name=?1",[table],|r|r.get(0)).optional()?.flatten();
        if actual.as_deref() != Some(ddl) {
            return Err(Error::Invalid("philosophy original DDL"));
        }
    }
    Ok(())
}
pub(crate) fn receipt(db: &Connection) -> Result<PhilosophyOriginalReceipt> {
    let count: i64 = db.query_row("SELECT count(*) FROM philosophy_original_meta", [], |r| {
        r.get(0)
    })?;
    if count != 1 {
        return Err(Error::Invalid("philosophy original receipt coverage"));
    }
    let raw:Option<Vec<u8>>=db.query_row("SELECT CASE WHEN typeof(receipt)='blob' AND length(receipt)<=?1 THEN receipt ELSE NULL END FROM philosophy_original_meta WHERE singleton=1",[JsonLimits::default().max_bytes as i64],|r|r.get(0))?;
    let raw = raw.ok_or(Error::Budget("philosophy original receipt bytes"))?;
    parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default())
        .map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(&raw).map_err(|_| Error::Invalid("philosophy original receipt"))
}
pub(crate) fn page(
    db: &Connection,
    collection: PhilosophyOriginalCollection,
    after: Option<u64>,
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
) -> Result<PhilosophyOriginalPage> {
    page_with_layout(
        db,
        collection,
        after,
        max_rows,
        max_row_bytes,
        max_page_bytes,
        KnowledgePayloadLayout::InlineV1,
    )
}
pub(crate) fn page_with_layout(
    db: &Connection,
    collection: PhilosophyOriginalCollection,
    after: Option<u64>,
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
    layout: KnowledgePayloadLayout,
) -> Result<PhilosophyOriginalPage> {
    let mut decode_work = 0;
    let work_cap = crate::knowledge_original_rows::page_decode_work_limit(max_page_bytes)?;
    page_with_layout_and_work(
        db,
        collection,
        after,
        max_rows,
        max_row_bytes,
        max_page_bytes,
        layout,
        &mut decode_work,
        work_cap,
    )
}
fn page_with_layout_and_work(
    db: &Connection,
    collection: PhilosophyOriginalCollection,
    after: Option<u64>,
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
    layout: KnowledgePayloadLayout,
    decode_work: &mut u64,
    work_cap: u64,
) -> Result<PhilosophyOriginalPage> {
    // Identical physical page envelope to the existing navigation carrier.
    crate::knowledge_original_rows::page_limits(max_rows, max_row_bytes, max_page_bytes)?;
    if after.is_some_and(|n| n > i64::MAX as u64) {
        return Err(Error::Budget("philosophy original page ordinal"));
    }
    let sql = if layout.uses_carriers() {
        "SELECT ordinal,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 1 AND ?3 AND length(packet)<=packet_len+17 THEN packet ELSE NULL END FROM philosophy_original_rows LEFT JOIN knowledge_source_carriers USING(packet_sha256,packet_len) WHERE collection=?1 AND ordinal>?2 ORDER BY ordinal LIMIT ?4"
    } else {
        "SELECT ordinal,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND packet_len=length(packet) AND length(packet)<=?3 THEN packet ELSE NULL END FROM philosophy_original_rows WHERE collection=?1 AND ordinal>?2 ORDER BY ordinal LIMIT ?4"
    };
    let mut q = db.prepare(sql)?;
    let mut scan = q.query(params![
        collection.as_str(),
        after.map_or(-1, |n| n as i64),
        max_row_bytes as i64,
        max_rows as i64
    ])?;
    let (rows, decoded_bytes) = crate::knowledge_original_rows::read(
        db,
        &mut scan,
        max_page_bytes,
        max_row_bytes,
        layout,
        decode_work,
        work_cap,
    )?;
    let rows = rows
        .into_iter()
        .map(|(ordinal, raw)| {
            Ok(PhilosophyOriginalRow {
                ordinal: u64::try_from(ordinal)
                    .map_err(|_| Error::Invalid("philosophy original ordinal"))?,
                raw_sha256: Digest256::of_bytes(&raw).to_hex(),
                raw,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let next_ordinal = if rows.len() == max_rows {
        rows.last().map(|r| r.ordinal)
    } else {
        None
    };
    Ok(PhilosophyOriginalPage {
        rows,
        next_ordinal,
        decoded_bytes,
        vm_steps: 0,
    })
}
fn verify_rows(
    db: &Connection,
    r: &PhilosophyOriginalReceipt,
    l: NavigationOriginalLimits,
    work: &mut u64,
    work_cap: u64,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    l.validate()?;
    if r.nodes
        .checked_add(r.edges)
        .and_then(|n| n.checked_add(1))
        .is_none_or(|n| n > l.max_rows)
        || r.total_bytes > l.max_total_bytes
    {
        return Err(Error::Budget("philosophy original aggregate limits"));
    }
    let mut total = 0u64;
    for (collection, wanted, sha, input_sha) in [
        (
            PhilosophyOriginalCollection::Header,
            1,
            &r.header_sha256,
            None,
        ),
        (
            PhilosophyOriginalCollection::Nodes,
            r.nodes,
            &r.nodes_root_sha256,
            Some(&r.node_input_root_sha256),
        ),
        (
            PhilosophyOriginalCollection::Edges,
            r.edges,
            &r.edges_root_sha256,
            Some(&r.edge_input_root_sha256),
        ),
    ] {
        let mut count = 0u64;
        let mut after = None;
        let mut h = ordered_hash(collection.as_str());
        loop {
            let p = page_with_layout_and_work(
                db,
                collection,
                after,
                1,
                l.max_row_bytes,
                l.max_row_bytes as u64,
                layout,
                work,
                work_cap,
            )?;
            for row in &p.rows {
                if row.ordinal != count {
                    return Err(Error::Invalid("philosophy original ordinal coverage"));
                }
                if collection == PhilosophyOriginalCollection::Header {
                    header(&row.raw, r, l.max_row_bytes)?;
                } else {
                    ordered_item(&mut h, row.ordinal, &row.raw);
                }
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= wanted)
                    .ok_or(Error::Invalid("philosophy original excess rows"))?;
                total = total
                    .checked_add(row.raw.len() as u64)
                    .filter(|n| *n <= l.max_total_bytes)
                    .ok_or(Error::Budget("philosophy original bytes"))?;
                *work = work
                    .checked_add(row.raw.len() as u64 + 40)
                    .filter(|n| *n <= work_cap)
                    .ok_or(Error::Budget("philosophy original cold work"))?;
            }
            after = p.next_ordinal;
            if after.is_none() {
                break;
            }
        }
        if count != wanted
            || collection != PhilosophyOriginalCollection::Header && &h.finalize().to_hex() != sha
        {
            return Err(Error::Invalid("philosophy original count/order root"));
        }
        if let Some(expected) = input_sha {
            let mut h = Digest256Hasher::new();
            let mut q=db.prepare("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 4096 THEN id ELSE NULL END,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END FROM philosophy_original_rows WHERE collection=?1 ORDER BY id")?;
            let mut scan = q.query([collection.as_str()])?;
            while let Some(row) = scan.next()? {
                let id: String = row
                    .get::<_, Option<String>>(0)?
                    .ok_or(Error::Invalid("philosophy original input ID"))?;
                let sha: Vec<u8> = row
                    .get::<_, Option<Vec<u8>>>(1)?
                    .ok_or(Error::Invalid("philosophy original input SHA"))?;
                text(&mut h, &id);
                h.update(&sha);
                *work = work
                    .checked_add(id.len() as u64 + 32)
                    .filter(|n| *n <= work_cap)
                    .ok_or(Error::Budget("philosophy original input cold work"))?;
            }
            if &h.finalize().to_hex() != expected {
                return Err(Error::Invalid("philosophy original input root"));
            }
        }
    }
    let all: i64 = db.query_row("SELECT count(*) FROM philosophy_original_rows", [], |r| {
        r.get(0)
    })?;
    if total != r.total_bytes || all as u64 != r.nodes + r.edges + 1 {
        return Err(Error::Invalid("philosophy original total coverage"));
    }
    Ok(())
}
pub(crate) fn verify_stage(
    stage: &mut KnowledgeStage<'_>,
    descriptor: Option<&str>,
) -> Result<Option<PhilosophyOriginalReceipt>> {
    let layout = stage.payload_layout();
    let binding = stage.exact_receipt()?.binding.clone();
    let inputs = stage.exact_receipt()?.collections.clone();
    stage.with_connection(WritePhase::Finalize, |db| {
        if !present(db)? {
            return Ok(None);
        }
        verify_ddl_with_layout(db, layout)?;
        let r = receipt(db)?;
        validate_producer_receipt(&r, &inputs)?;
        if r.source_cut != binding.source_cut
            || r.membership_root != binding.membership_root
            || descriptor.is_some_and(|d| d != r.descriptor_sha256)
        {
            return Err(Error::Invalid("philosophy original stage binding"));
        }
        verify_rows(
            db,
            &r,
            crate::knowledge_original_rows::maximum_limits(),
            &mut 0,
            crate::knowledge_original_rows::MAX_COLD_WORK,
            layout,
        )?;
        if descriptor.is_none() {
            sealed_root(db, &r)?;
            let descriptor:Option<String>=db.query_row("SELECT CASE WHEN length(CAST(value AS BLOB))=64 THEN CAST(value AS TEXT) ELSE NULL END FROM metadata WHERE key='descriptor_sha256'",[],|row|row.get(0)).optional()?.flatten();
            if descriptor.as_deref()!=Some(r.descriptor_sha256.as_str()){return Err(Error::Invalid("philosophy original finish descriptor"));}
            let abi: String = db.query_row(
                "SELECT CAST(value AS TEXT) FROM metadata WHERE key='model_abi'",
                [],
                |r| r.get(0),
            )?;
            let abi_matches = if let Some(expected) = layout.carrier_model_abi() {
                abi == expected
            } else {
                [KNOWLEDGE_PHILOSOPHY_MODEL_ABI, crate::KNOWLEDGE_CORPUS_MODEL_ABI].contains(&abi.as_str())
            };
            if !abi_matches {
                return Err(Error::Invalid("philosophy original finish ABI"));
            }
        }
        Ok(Some(r))
    })
}
fn sealed_root(db: &Connection, r: &PhilosophyOriginalReceipt) -> Result<()> {
    let v:Option<String>=db.query_row("SELECT CASE WHEN length(CAST(value AS BLOB))=64 THEN CAST(value AS TEXT) ELSE NULL END FROM metadata WHERE key='philosophy_original_root_sha256'",[],|r|r.get(0)).optional()?.flatten();
    if v.as_deref() != Some(r.component_root_sha256.as_str()) {
        return Err(Error::Invalid("philosophy original sealed root"));
    }
    Ok(())
}
pub(crate) fn verify(
    db: &Connection,
    e: &KnowledgeSelectedExpectation,
    l: ColdOpenLimits,
    work: &mut u64,
) -> Result<Option<PhilosophyOriginalReceipt>> {
    let layout = crate::knowledge_stage::KnowledgePayloadLayout::from_model_abi(&e.model_abi);
    let found = present(db)?;
    if found != e.philosophy_original_root_sha256.is_some()
        || found
            && ![
                KNOWLEDGE_PHILOSOPHY_MODEL_ABI,
                crate::KNOWLEDGE_CORPUS_MODEL_ABI,
                KNOWLEDGE_CARRIER_ONCE_MODEL_ABI,
                tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1,
                tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2,
            ]
            .contains(&e.model_abi.as_str())
    {
        return Err(Error::Invalid("philosophy original ABI/expected coverage"));
    }
    if !found {
        let n: i64 = db.query_row(
            "SELECT count(*) FROM metadata WHERE key='philosophy_original_root_sha256'",
            [],
            |r| r.get(0),
        )?;
        if n != 0 {
            return Err(Error::Invalid("philosophy original phantom seal"));
        }
        return Ok(None);
    }
    verify_ddl_with_layout(db, layout)?;
    let r = receipt(db)?;
    if r.profile != PHILOSOPHY_ORIGINAL_PROFILE
        || e.philosophy_original_root_sha256.as_deref() != Some(r.component_root_sha256.as_str())
        || root(&r)? != r.component_root_sha256
        || r.descriptor_sha256 != e.descriptor_sha256
        || r.source_cut != e.source_cut
        || r.membership_root != e.membership_root
        || !e.source_scopes.iter().any(|s| {
            s.source_graph == r.source_graph && s.adapter_profile == "philosophy-node-edge-v1"
        })
    {
        return Err(Error::Invalid("philosophy original selected binding"));
    }
    *work = work
        .checked_add(
            serde_json::to_vec(&r)
                .map_err(|_| Error::Invalid("philosophy original receipt"))?
                .len() as u64,
        )
        .filter(|n| *n <= l.max_work_bytes)
        .ok_or(Error::Budget("philosophy original receipt cold work"))?;
    verify_rows(
        db,
        &r,
        NavigationOriginalLimits {
            max_rows: l.max_rows.min(crate::knowledge_original_rows::MAX_ROWS),
            max_row_bytes: l
                .max_row_bytes
                .min(crate::knowledge_original_rows::MAX_ROW_BYTES),
            max_total_bytes: l
                .max_work_bytes
                .min(crate::knowledge_original_rows::MAX_TOTAL_BYTES),
        },
        work,
        l.max_work_bytes,
        layout,
    )?;
    sealed_root(db, &r)?;
    Ok(Some(r))
}

// Owned cold verification path; reused selected-state SQL authority.
fn root_with_state(
    r: &PhilosophyOriginalReceipt,
    state: Option<&CreationState<'_>>,
) -> Result<String> {
    if let Some(state) = state {
        let bytes = [
            &r.profile,
            &r.descriptor_sha256,
            &r.source_cut,
            &r.membership_root,
            &r.source_graph,
            &r.node_input_root_sha256,
            &r.edge_input_root_sha256,
            &r.header_sha256,
            &r.nodes_root_sha256,
            &r.edges_root_sha256,
        ]
        .into_iter()
        .try_fold(24usize, |n, s| {
            n.checked_add(s.len())
                .ok_or(Error::Budget("philosophy original root work"))
        })?;
        state.charge_work(bytes)?;
        state.retain(64)?;
    }
    root(r)
}

fn with_object<T>(
    raw: &[u8],
    cap: usize,
    state: Option<&CreationState<'_>>,
    operation: impl FnOnce(&serde_json::Value) -> Result<T>,
) -> Result<T> {
    if let Some(state) = state {
        let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("philosophy original JSON limits"))?;
        state.with_serde_owned_with_limits(raw, limits, |value| {
            if !value.is_object() {
                return Err(Error::Invalid("philosophy original object"));
            }
            operation(value)
        })
    } else {
        operation(&object(raw, cap)?)
    }
}

fn header_with_state(
    raw: &[u8],
    r: &PhilosophyOriginalReceipt,
    cap: usize,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    let _digest_hold = state.map(|s| s.hold(64)).transpose()?;
    with_object(raw, cap, state, |v| {
        if let Some(state) = state {
            state.charge_work(raw.len())?;
        }
        if v.get("nodes").is_some()
            || v.get("edges").is_some()
            || v["schema_version"].as_str().is_none_or(str::is_empty)
            || v["counts"]["nodes"].as_u64() != Some(r.nodes)
            || v["counts"]["edges"].as_u64() != Some(r.edges)
            || Digest256::of_bytes(raw).to_hex() != r.header_sha256
        {
            return Err(Error::Invalid("philosophy original detached header"));
        }
        Ok(())
    })
}

fn scalar_owned(db: &Connection, sql: &std::ffi::CStr, state: &CreationState<'_>) -> Result<i64> {
    with_owned_schema_statement(db, sql, state, |q| {
        if !owned_schema_step(q, state)? {
            return Err(Error::Invalid("original scalar missing"));
        }
        q.integer(0).map_err(owned_schema_sql_error)
    })
}

fn present_with_state(db: &Connection, state: Option<&CreationState<'_>>) -> Result<bool> {
    let Some(state) = state else {
        return present(db);
    };
    let n=scalar_owned(db,c"SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('philosophy_original_meta','philosophy_original_rows')",state)?;
    if n != 0 && n != 2 {
        return Err(Error::Invalid("philosophy original partial tables"));
    }
    Ok(n == 2)
}

fn bounded_text<'a>(
    value: rusqlite::types::ValueRef<'a>,
    cap: usize,
    state: &CreationState<'_>,
) -> Result<&'a str> {
    let bytes = match value {
        rusqlite::types::ValueRef::Text(v) => v,
        _ => return Err(Error::Invalid("original bounded text kind")),
    };
    if bytes.len() > cap {
        return Err(Error::Budget("original bounded text bytes"));
    }
    state.charge_work(bytes.len())?;
    std::str::from_utf8(bytes).map_err(|_| Error::Invalid("original bounded text UTF8"))
}

pub(crate) fn verify_ddl_with_owned_state_and_layout(
    db: &Connection,
    state: Option<&CreationState<'_>>,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    let Some(state) = state else {
        return verify_ddl_with_layout(db, layout);
    };
    let _sql = state.hold(
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(),
    )?;
    for (table, ddl) in [
        (META_TABLE, META_DDL),
        (
            ROW_TABLE,
            if layout.uses_carriers() {
                ROW_DDL_CARRIER
            } else {
                ROW_DDL
            },
        ),
    ] {
        with_owned_schema_statement(
            db,
            c"SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            state,
            |q| {
                state.charge_work(table.len())?;
                state.active()?;
                q.bind_text(1, table).map_err(owned_schema_sql_error)?;
                if !owned_schema_step(q, state)? {
                    return Err(Error::Invalid("philosophy original DDL missing"));
                }
                let actual =
                    bounded_text(q.value_ref(0).map_err(owned_schema_sql_error)?, 4096, state)?;
                if !owned_schema_equal(state, actual.as_bytes(), ddl.as_bytes())? {
                    return Err(Error::Invalid("philosophy original DDL"));
                }
                Ok(())
            },
        )?;
    }
    state.active()
}

fn metadata_matches(
    db: &Connection,
    key: &str,
    expected: &str,
    state: Option<&CreationState<'_>>,
) -> Result<bool> {
    if let Some(state) = state {
        return with_owned_schema_statement(
            db,
            c"SELECT value FROM metadata WHERE key=?1",
            state,
            |q| {
                state.charge_work(key.len())?;
                state.active()?;
                q.bind_text(1, key).map_err(owned_schema_sql_error)?;
                if !owned_schema_step(q, state)? {
                    return Ok(false);
                }
                let bytes = match q.value_ref(0).map_err(owned_schema_sql_error)? {
                    rusqlite::types::ValueRef::Text(v) | rusqlite::types::ValueRef::Blob(v) => v,
                    _ => return Err(Error::Invalid("philosophy original metadata bytes")),
                };
                owned_schema_equal(state, bytes, expected.as_bytes())
            },
        );
    }
    let _hold = state.map(|s| s.hold(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())).transpose()?;
    let mut q = db.prepare("SELECT value FROM metadata WHERE key=?1")?;
    let mut rows = q.query([key])?;
    let Some(row) = rows.next()? else {
        return Ok(false);
    };
    let value = row.get_ref(0)?;
    let bytes = match value {
        rusqlite::types::ValueRef::Text(v) | rusqlite::types::ValueRef::Blob(v) => v,
        _ => return Err(Error::Invalid("philosophy original metadata bytes")),
    };
    if let Some(state) = state {
        state.charge_work(bytes.len())?;
    }
    Ok(bytes == expected.as_bytes())
}

pub(crate) fn receipt_with_state(
    db: &Connection,
    state: Option<&CreationState<'_>>,
) -> Result<PhilosophyOriginalReceipt> {
    let Some(state) = state else {
        return receipt(db);
    };
    let _sql = state.hold(
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(),
    )?;
    let count = scalar_owned(db, c"SELECT count(*) FROM philosophy_original_meta", state)?;
    if count != 1 {
        return Err(Error::Invalid("philosophy original receipt coverage"));
    }
    with_owned_schema_statement(
        db,
        c"SELECT receipt FROM philosophy_original_meta WHERE singleton=1",
        state,
        |q| {
            if !owned_schema_step(q, state)? {
                return Err(Error::Invalid("philosophy original receipt missing"));
            }
            let raw = q
                .value_ref(0)
                .map_err(owned_schema_sql_error)?
                .as_blob()
                .map_err(|_| Error::Invalid("original packet column type"))?;
            if raw.len() > JsonLimits::default().max_bytes {
                return Err(Error::Budget("philosophy original receipt bytes"));
            }
            // Receipt strings are decoded from this input. Its byte length bounds all
            // copied UTF-8 text; the typed owner is retained separately from the DOM.
            state.retain(
                raw.len()
                    .checked_add(std::mem::size_of::<PhilosophyOriginalReceipt>())
                    .ok_or(Error::Budget("philosophy original typed receipt"))?,
            )?;
            state.with_serde_owned_value_with_limits(raw, JsonLimits::default(), |value| {
                state.charge_work(raw.len())?;
                serde_json::from_value(value)
                    .map_err(|_| Error::Invalid("philosophy original receipt"))
            })
        },
    )
}

fn verify_rows_owned(
    db: &Connection,
    r: &PhilosophyOriginalReceipt,
    l: NavigationOriginalLimits,
    work: &mut u64,
    work_cap: u64,
    state: &CreationState<'_>,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    l.validate()?;
    if r.nodes
        .checked_add(r.edges)
        .and_then(|n| n.checked_add(1))
        .is_none_or(|n| n > l.max_rows)
        || r.total_bytes > l.max_total_bytes
    {
        return Err(Error::Budget("philosophy original aggregate limits"));
    }
    let _sql = state.hold(
        2 * tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(
        ),
    )?;
    let mut total = 0u64;
    for (collection, wanted, expected, input_sha) in [
        (
            PhilosophyOriginalCollection::Header,
            1,
            &r.header_sha256,
            None,
        ),
        (
            PhilosophyOriginalCollection::Nodes,
            r.nodes,
            &r.nodes_root_sha256,
            Some(&r.node_input_root_sha256),
        ),
        (
            PhilosophyOriginalCollection::Edges,
            r.edges,
            &r.edges_root_sha256,
            Some(&r.edge_input_root_sha256),
        ),
    ] {
        let sql = if layout.uses_carriers() {
            c"SELECT ordinal,packet_len,packet_sha256,packet FROM philosophy_original_rows LEFT JOIN knowledge_source_carriers USING(packet_sha256,packet_len) WHERE collection=?1 ORDER BY ordinal"
        } else {
            c"SELECT ordinal,packet_len,packet_sha256,packet FROM philosophy_original_rows WHERE collection=?1 ORDER BY ordinal"
        };
        let mut count = 0u64;
        let mut h = ordered_hash(collection.as_str());
        with_owned_schema_statement(db, sql, state, |q| {
            state.charge_work(collection.as_str().len())?;
            state.active()?;
            q.bind_text(1, collection.as_str())
                .map_err(owned_schema_sql_error)?;
            while owned_schema_step(q, state)? {
                state.active()?;
                let ordinal = q.integer(0).map_err(owned_schema_sql_error)?;
                let declared = q.unsigned_integer(1).map_err(owned_schema_sql_error)?;
                let digest: [u8; 32] = q
                    .value_ref(2)
                    .map_err(owned_schema_sql_error)?
                    .as_blob()
                    .map_err(|_| Error::Invalid("original packet column type"))?
                    .try_into()
                    .map_err(|_| Error::Invalid("philosophy original row digest"))?;
                let raw = q
                    .value_ref(3)
                    .map_err(owned_schema_sql_error)?
                    .as_blob()
                    .map_err(|_| Error::Invalid("original packet column type"))?;
                if ordinal < 0 || ordinal as u64 != count || declared > l.max_row_bytes as u64 {
                    return Err(Error::Invalid(
                        "philosophy original ordinal/length coverage",
                    ));
                }
                layout.with_sql_decoded(
                    db,
                    state,
                    raw,
                    Some(declared as usize),
                    l.max_row_bytes,
                    |raw| {
                        let row_work = raw.len() as u64 + 40;
                        *work = work
                            .checked_add(row_work)
                            .filter(|n| *n <= work_cap)
                            .ok_or(Error::Budget("philosophy original cold work"))?;
                        state.charge_work(
                            raw.len()
                                .checked_mul(2)
                                .and_then(|n| n.checked_add(40))
                                .ok_or(Error::Budget("philosophy original hash work"))?,
                        )?;
                        if Digest256::of_bytes(raw).as_bytes() != &digest {
                            return Err(Error::Invalid("philosophy original row digest differs"));
                        }
                        if collection == PhilosophyOriginalCollection::Header {
                            header_with_state(raw, r, l.max_row_bytes, Some(state))?;
                        } else {
                            ordered_item(&mut h, count, raw);
                        }
                        count = count
                            .checked_add(1)
                            .filter(|n| *n <= wanted)
                            .ok_or(Error::Invalid("philosophy original excess rows"))?;
                        total = total
                            .checked_add(raw.len() as u64)
                            .filter(|n| *n <= l.max_total_bytes)
                            .ok_or(Error::Budget("philosophy original bytes"))?;
                        Ok(())
                    },
                )?;
            }
            Ok(())
        })?;
        state.charge_work(64)?;
        let _hash = state.hold(64)?;
        if count != wanted
            || collection != PhilosophyOriginalCollection::Header
                && &h.finalize().to_hex() != expected
        {
            return Err(Error::Invalid("philosophy original count/order root"));
        }
        if let Some(expected) = input_sha {
            let mut h = Digest256Hasher::new();
            with_owned_schema_statement(db,c"SELECT id,packet_sha256 FROM philosophy_original_rows WHERE collection=?1 ORDER BY id",state,|q| {
            state.charge_work(collection.as_str().len())?;state.active()?;
            q.bind_text(1,collection.as_str()).map_err(owned_schema_sql_error)?;
            while owned_schema_step(q,state)? {
                let bytes = match q.value_ref(0).map_err(owned_schema_sql_error)? {rusqlite::types::ValueRef::Text(v)=>v,
                    _=>return Err(Error::Invalid("philosophy original input ID"))};
                let sha = q.value_ref(1).map_err(owned_schema_sql_error)?.as_blob().map_err(|_|Error::Invalid("original packet column type"))?;
                if bytes.is_empty() || bytes.len() > 4096 || sha.len() != 32 { return Err(Error::Invalid("philosophy original input ID/SHA")); }
                *work = work.checked_add(bytes.len() as u64 + 32).filter(|n| *n <= work_cap).ok_or(Error::Budget("philosophy original input cold work"))?;
                state.charge_work(bytes.len()+32)?;
                let id=std::str::from_utf8(bytes).map_err(|_|Error::Invalid("philosophy original input UTF8"))?;
                text(&mut h,id); h.update(sha);
            }
            Ok(())
            })?;
            if &h.finalize().to_hex() != expected {
                return Err(Error::Invalid("philosophy original input root"));
            }
        }
    }
    let all = scalar_owned(db, c"SELECT count(*) FROM philosophy_original_rows", state)?;
    if total != r.total_bytes || all < 0 || all as u64 != r.nodes + r.edges + 1 {
        return Err(Error::Invalid("philosophy original total coverage"));
    }
    state.active()
}

fn verify_rows_with_state(
    db: &Connection,
    r: &PhilosophyOriginalReceipt,
    l: NavigationOriginalLimits,
    work: &mut u64,
    work_cap: u64,
    state: Option<&CreationState<'_>>,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    if let Some(state) = state {
        verify_rows_owned(db, r, l, work, work_cap, state, layout)
    } else {
        verify_rows(db, r, l, work, work_cap, layout)
    }
}

fn sealed_root_with_state(
    db: &Connection,
    r: &PhilosophyOriginalReceipt,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    if r.component_root_sha256.len() != 64
        || !metadata_matches(
            db,
            "philosophy_original_root_sha256",
            &r.component_root_sha256,
            state,
        )?
    {
        return Err(Error::Invalid("philosophy original sealed root"));
    }
    Ok(())
}

pub(crate) fn verify_with_owned_state(
    db: &Connection,
    e: &KnowledgeSelectedExpectation,
    l: ColdOpenLimits,
    work: &mut u64,
    state: Option<&CreationState<'_>>,
) -> Result<Option<PhilosophyOriginalReceipt>> {
    let layout = crate::knowledge_stage::KnowledgePayloadLayout::from_model_abi(&e.model_abi);
    let _presence_sql = state.map(|s|s.hold(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())).transpose()?;
    let found = present_with_state(db, state)?;
    if found != e.philosophy_original_root_sha256.is_some()
        || found
            && ![
                KNOWLEDGE_PHILOSOPHY_MODEL_ABI,
                crate::KNOWLEDGE_CORPUS_MODEL_ABI,
                KNOWLEDGE_CARRIER_ONCE_MODEL_ABI,
                tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1,
                tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2,
            ]
            .contains(&e.model_abi.as_str())
    {
        return Err(Error::Invalid("philosophy original ABI/expected coverage"));
    }
    if !found {
        let n: i64 = if let Some(state) = state {
            scalar_owned(
                db,
                c"SELECT count(*) FROM metadata WHERE key='philosophy_original_root_sha256'",
                state,
            )?
        } else {
            db.query_row(
                "SELECT count(*) FROM metadata WHERE key='philosophy_original_root_sha256'",
                [],
                |r| r.get(0),
            )?
        };
        if n != 0 {
            return Err(Error::Invalid("philosophy original phantom seal"));
        }
        return Ok(None);
    }
    verify_ddl_with_owned_state_and_layout(db, state, layout)?;
    let r = receipt_with_state(db, state)?;
    if r.profile != PHILOSOPHY_ORIGINAL_PROFILE
        || e.philosophy_original_root_sha256.as_deref() != Some(r.component_root_sha256.as_str())
        || root_with_state(&r, state)? != r.component_root_sha256
        || r.descriptor_sha256 != e.descriptor_sha256
        || r.source_cut != e.source_cut
        || r.membership_root != e.membership_root
        || !e.source_scopes.iter().any(|s| {
            s.source_graph == r.source_graph && s.adapter_profile == "philosophy-node-edge-v1"
        })
    {
        return Err(Error::Invalid("philosophy original selected binding"));
    }
    let receipt_bytes = if let Some(state) = state {
        state.with_json_encoded(&r, JsonLimits::default().max_bytes, |raw| {
            Ok(raw.len() as u64)
        })?
    } else {
        serde_json::to_vec(&r)
            .map_err(|_| Error::Invalid("philosophy original receipt"))?
            .len() as u64
    };
    *work = work
        .checked_add(receipt_bytes)
        .filter(|n| *n <= l.max_work_bytes)
        .ok_or(Error::Budget("philosophy original receipt cold work"))?;
    verify_rows_with_state(
        db,
        &r,
        NavigationOriginalLimits {
            max_rows: l.max_rows.min(crate::knowledge_original_rows::MAX_ROWS),
            max_row_bytes: l
                .max_row_bytes
                .min(crate::knowledge_original_rows::MAX_ROW_BYTES),
            max_total_bytes: l
                .max_work_bytes
                .min(crate::knowledge_original_rows::MAX_TOTAL_BYTES),
        },
        work,
        l.max_work_bytes,
        state,
        layout,
    )?;
    sealed_root_with_state(db, &r, state)?;
    Ok(Some(r))
}
