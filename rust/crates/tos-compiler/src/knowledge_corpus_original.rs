//! Named original corpus projection custody. Capture identity is distinct from
//! authored-source identity; neither projection custody nor selection admits it.
use crate::knowledge_selected::{ColdOpenLimits, KnowledgeSelectedExpectation};
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{Error, QueryVocabulary, Result, SourceBinding};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::Value;
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, RelativePath, canonical_bytes_v1, parse_json,
};
use tos_foundation::{Digest256, Digest256Hasher};

pub const CORPUS_ORIGINAL_PROFILE: &str = "tos_corpus_original_v1";
pub const NATIVE_CORPUS_ORIGINAL_PROFILE: &str = "tos_corpus_original_native_v1";
pub const KNOWLEDGE_CORPUS_MODEL_ABI: &str = "tos_knowledge_read_model_v5";
const MAX_INDEX_TEXT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorpusOriginalCollection {
    Header,
    Nodes,
    Resources,
    Manifests,
    Branches,
    GraphViews,
    RelationPacks,
    RelationEdges,
}
impl CorpusOriginalCollection {
    pub const ROWS: [Self; 7] = [
        Self::Nodes,
        Self::Resources,
        Self::Manifests,
        Self::Branches,
        Self::GraphViews,
        Self::RelationPacks,
        Self::RelationEdges,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Nodes => "nodes",
            Self::Resources => "resources",
            Self::Manifests => "manifests",
            Self::Branches => "branches",
            Self::GraphViews => "graph_views",
            Self::RelationPacks => "relation_packs",
            Self::RelationEdges => "relation_edges",
        }
    }
}
/// Only selectors used by maintained corpus reads; no caller SQL or field DSL.
pub enum CorpusOriginalSelector {
    All,
    NodeId(String),
    IncidentNode(String),
    PackId(String),
    ViewId(String),
    Resources {
        resource_kind: Option<String>,
        owner_branch: Option<String>,
    },
    OwnerBranch(String),
    NodeIds(Vec<String>),
    PackIds(Vec<String>),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusOriginalCollectionReceipt {
    pub collection: String,
    pub rows: u64,
    pub ordered_root_sha256: String,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusOriginalMember {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusOriginalOrigin {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_git_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_git_tree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_producer: Option<crate::source_corpus::NativeCorpusSourceReceipt>,
    pub source_path: String,
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub members: Vec<CorpusOriginalMember>,
    pub member_root_sha256: String,
}
pub type CapturedCorpusOrigin = CorpusOriginalOrigin;
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusOriginalReceipt {
    pub profile: String,
    pub descriptor_sha256: String,
    pub source_cut: String,
    pub membership_root: String,
    pub origin: CorpusOriginalOrigin,
    pub header_sha256: String,
    pub collections: Vec<CorpusOriginalCollectionReceipt>,
    pub component_root_sha256: String,
    pub total_bytes: u64,
}
#[derive(Debug)]
pub struct CorpusOriginalRow {
    pub ordinal: u64,
    pub raw_sha256: String,
    pub raw: Vec<u8>,
}
#[derive(Debug)]
pub struct CorpusOriginalPage {
    pub rows: Vec<CorpusOriginalRow>,
    pub next_ordinal: Option<u64>,
    pub decoded_bytes: u64,
    /// The existing caller progress handler accounts aggregate VM work.
    pub vm_steps: u64,
}
/// Cold-verified GraphViews index identity, without the original packet body.
#[derive(Debug)]
pub struct CorpusOriginalViewIdentity {
    pub ordinal: u64,
    pub view_id: Option<String>,
    pub raw_sha256: String,
}
#[derive(Debug)]
pub struct CorpusOriginalViewIdentityPage {
    pub rows: Vec<CorpusOriginalViewIdentity>,
    pub next_ordinal: Option<u64>,
    /// Logical returned identity bytes: ordinal, hex digest and optional ID.
    pub decoded_bytes: u64,
    /// The existing caller progress handler accounts aggregate VM work.
    pub vm_steps: u64,
}
pub(crate) fn text(h: &mut Digest256Hasher, value: &str) {
    h.update(&(value.len() as u64).to_be_bytes());
    h.update(value.as_bytes());
}
pub(crate) fn order_hash(collection: &str) -> Digest256Hasher {
    let mut h = Digest256Hasher::new();
    text(&mut h, CORPUS_ORIGINAL_PROFILE);
    text(&mut h, collection);
    h
}
pub(crate) fn order_item(h: &mut Digest256Hasher, ordinal: u64, raw: &[u8]) {
    h.update(&ordinal.to_be_bytes());
    h.update(&(raw.len() as u64).to_be_bytes());
    h.update(Digest256::of_bytes(raw).as_bytes());
}
pub(crate) fn ordered_root(collection: &str, rows: &[Vec<u8>]) -> String {
    let mut h = order_hash(collection);
    for (i, raw) in rows.iter().enumerate() {
        order_item(&mut h, i as u64, raw);
    }
    h.finalize().to_hex()
}
/// Opaque exact captured projection plan; there is no constructor from SQL or
/// normalized records. The selected model composition binds distinct origins.
pub struct CorpusOriginalPlan {
    pub(crate) binding: SourceBinding,
    pub(crate) receipt: CorpusOriginalReceipt,
    pub(crate) header: Vec<u8>,
    pub(crate) rows: Vec<(CorpusOriginalCollection, Vec<Vec<u8>>)>,
}
impl CorpusOriginalPlan {
    pub fn receipt(&self) -> &CorpusOriginalReceipt {
        &self.receipt
    }
}

pub type CapturedCorpusOriginalPlan = CorpusOriginalPlan;

pub(crate) const META_TABLE: &str = "corpus_original_meta";
pub(crate) const ROW_TABLE: &str = "corpus_original_rows";
pub(crate) const META_DDL: &str = "CREATE TABLE corpus_original_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1),receipt BLOB NOT NULL)";
pub(crate) const ROW_DDL: &str = "CREATE TABLE corpus_original_rows(collection TEXT NOT NULL,ordinal INTEGER NOT NULL CHECK(ordinal>=0),packet_len INTEGER NOT NULL,packet_sha256 BLOB NOT NULL,packet BLOB NOT NULL,node_id TEXT,from_id TEXT,to_id TEXT,pack_id TEXT,view_id TEXT,resource_kind TEXT,owner_branch TEXT,PRIMARY KEY(collection,ordinal)) WITHOUT ROWID";
pub(crate) const INDEXES: [(&str, &str); 7] = [
    (
        "corpus_original_node",
        "CREATE INDEX corpus_original_node ON corpus_original_rows(collection,node_id,ordinal)",
    ),
    (
        "corpus_original_from",
        "CREATE INDEX corpus_original_from ON corpus_original_rows(collection,from_id,ordinal)",
    ),
    (
        "corpus_original_to",
        "CREATE INDEX corpus_original_to ON corpus_original_rows(collection,to_id,ordinal)",
    ),
    (
        "corpus_original_pack",
        "CREATE INDEX corpus_original_pack ON corpus_original_rows(collection,pack_id,ordinal)",
    ),
    (
        "corpus_original_view",
        "CREATE INDEX corpus_original_view ON corpus_original_rows(collection,view_id,ordinal)",
    ),
    (
        "corpus_original_resource",
        "CREATE INDEX corpus_original_resource ON corpus_original_rows(collection,resource_kind,owner_branch,ordinal)",
    ),
    (
        "corpus_original_owner",
        "CREATE INDEX corpus_original_owner ON corpus_original_rows(collection,owner_branch,ordinal)",
    ),
];
const FIELDS: [&str; 7] = [
    "node_id",
    "from_id",
    "to_id",
    "pack_id",
    "view_id",
    "resource_kind",
    "owner_branch",
];
/// Lookup indexes cover the named captured string-identity profile. Other
/// scalar identity kinds refuse explicitly; nonobject entries retain ordinals.
pub(crate) fn indexed_fields(
    collection: CorpusOriginalCollection,
    v: &Value,
) -> Result<[Option<&str>; 7]> {
    use CorpusOriginalCollection as C;
    let used: &[usize] = match collection {
        C::Nodes => &[0],
        C::Resources => &[5, 6],
        C::GraphViews => &[4],
        C::RelationPacks => &[3, 6],
        C::RelationEdges => &[1, 2, 3, 6],
        C::Header | C::Branches | C::Manifests => &[],
    };
    let mut fields = [None; 7];
    for &i in used {
        fields[i] = match v.get(FIELDS[i]) {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) if value.len() <= MAX_INDEX_TEXT_BYTES => {
                Some(value.as_str())
            }
            Some(Value::String(_)) => return Err(Error::Budget("corpus indexed key bytes")),
            _ => return Err(Error::Invalid("unsupported corpus indexed scalar kind")),
        };
    }
    Ok(fields)
}
fn values(raw: &[u8], cap: usize) -> Result<Value> {
    let l = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("corpus original JSON bounds"))?;
    parse_json(raw, JsonMode::PublishedStrict, l).map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|_| Error::Invalid("corpus original JSON"))
}
pub(crate) fn component_root(r: &CorpusOriginalReceipt) -> Result<String> {
    let mut body = r.clone();
    body.component_root_sha256.clear();
    let raw = serde_json::to_vec(&body)
        .map_err(|_| Error::Invalid("corpus original receipt encoding"))?;
    let l = JsonLimits::default();
    let parsed =
        parse_json(&raw, JsonMode::PublishedStrict, l).map_err(|e| Error::Source(e.to_string()))?;
    let canonical = canonical_bytes_v1(parsed.root(), CanonicalProfile::SourceRecordDigestV1, l)
        .map_err(|e| Error::Source(e.to_string()))?;
    let mut h = Digest256Hasher::new();
    h.update(b"tos-corpus-original-component-v1\0");
    h.update(&canonical);
    Ok(h.finalize().to_hex())
}
pub(crate) fn validate_receipt(r: &CorpusOriginalReceipt) -> Result<()> {
    if ![CORPUS_ORIGINAL_PROFILE, NATIVE_CORPUS_ORIGINAL_PROFILE].contains(&r.profile.as_str())
        || r.collections.len() != CorpusOriginalCollection::ROWS.len()
        || r.total_bytes == 0
        || r.total_bytes > crate::knowledge_original_rows::MAX_TOTAL_BYTES
        || component_root(r)? != r.component_root_sha256
    {
        return Err(Error::Invalid("corpus original receipt profile/root"));
    }
    for s in [&r.source_cut, &r.membership_root, &r.origin.source_path] {
        if s.is_empty() || s.len() > 4096 {
            return Err(Error::Invalid("corpus original identity"));
        }
    }
    RelativePath::parse(&r.origin.source_path).map_err(|_| Error::Invalid("corpus origin path"))?;
    for sha in [
        &r.descriptor_sha256,
        &r.header_sha256,
        &r.origin.source_sha256,
        &r.origin.member_root_sha256,
    ] {
        Digest256::from_hex(sha).map_err(|_| Error::Invalid("corpus origin digest"))?;
    }
    let native = r.origin.profile == "native-corpus-producer-v1";
    if native {
        if r.profile != NATIVE_CORPUS_ORIGINAL_PROFILE
            || r.origin.source_git_commit.is_some()
            || r.origin.source_git_tree.is_some()
            || r.origin.capture_manifest_sha256.is_some()
        {
            return Err(Error::Invalid("mixed native corpus capture origin"));
        }
        let producer = r
            .origin
            .native_producer
            .as_ref()
            .ok_or(Error::Invalid("native corpus producer missing"))?;
        crate::source_corpus::validate_receipt(producer)?;
        if producer.source_cut != r.source_cut
            || producer.descriptor_sha256 != r.descriptor_sha256
            || producer.source_membership_sha256 != r.membership_root
            || producer.output_sha256 != r.origin.source_sha256
            || producer.output_bytes != r.origin.source_size_bytes
        {
            return Err(Error::Invalid("native corpus producer selected binding"));
        }
    } else {
        if r.profile != CORPUS_ORIGINAL_PROFILE
            || r.origin.profile != "captured-public-corpus-v1"
            || r.origin.native_producer.is_some()
        {
            return Err(Error::Invalid("corpus captured profile"));
        }
        for git in [&r.origin.source_git_commit, &r.origin.source_git_tree] {
            let git = git
                .as_ref()
                .ok_or(Error::Invalid("corpus captured Git identity missing"))?;
            if ![40, 64].contains(&git.len())
                || !git
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Error::Invalid("corpus captured Git identity"));
            }
        }
        Digest256::from_hex(
            r.origin
                .capture_manifest_sha256
                .as_deref()
                .ok_or(Error::Invalid("corpus capture SHA missing"))?,
        )
        .map_err(|_| Error::Invalid("corpus capture SHA"))?;
    }
    let mut count = 1u64;
    for (c, expected) in r.collections.iter().zip(CorpusOriginalCollection::ROWS) {
        if c.collection != expected.as_str() {
            return Err(Error::Invalid("corpus original collection closure"));
        }
        Digest256::from_hex(&c.ordered_root_sha256)
            .map_err(|_| Error::Invalid("corpus original order SHA"))?;
        count = count
            .checked_add(c.rows)
            .filter(|n| *n <= crate::knowledge_original_rows::MAX_ROWS)
            .ok_or(Error::Budget("corpus original total rows"))?;
    }
    let mut previous = None;
    let mut h = Digest256Hasher::new();
    text(
        &mut h,
        if native {
            "tos-native-corpus-output-members-v1"
        } else {
            "tos-captured-corpus-members-v1"
        },
    );
    let mut source_found = false;
    for m in &r.origin.members {
        if m.path.len() > 4096 || previous.is_some_and(|p: &str| m.path.as_str() <= p) {
            return Err(Error::Invalid("corpus captured member order"));
        }
        RelativePath::parse(&m.path).map_err(|_| Error::Invalid("corpus captured member path"))?;
        let sha = Digest256::from_hex(&m.sha256)
            .map_err(|_| Error::Invalid("corpus captured member SHA"))?;
        text(&mut h, &m.path);
        h.update(&m.size_bytes.to_be_bytes());
        h.update(sha.as_bytes());
        previous = Some(m.path.as_str());
        if m.path == r.origin.source_path {
            source_found =
                m.sha256 == r.origin.source_sha256 && m.size_bytes == r.origin.source_size_bytes;
        }
    }
    if !source_found || h.finalize().to_hex() != r.origin.member_root_sha256 {
        return Err(Error::Invalid("corpus captured closure root"));
    }
    Ok(())
}
// The finite plan and streaming captured importer write the same rows/indexes.
pub(crate) fn insert_original_row(
    insert: &mut rusqlite::Statement<'_>,
    collection: CorpusOriginalCollection,
    ordinal: u64,
    raw: &[u8],
) -> Result<()> {
    let v = values(raw, crate::knowledge_original_rows::MAX_ROW_BYTES)?;
    let fields = indexed_fields(collection, &v)?;
    insert.execute(params![
        collection.as_str(),
        ordinal as i64,
        raw.len() as i64,
        Digest256::of_bytes(raw).as_bytes().as_slice(),
        raw,
        fields[0],
        fields[1],
        fields[2],
        fields[3],
        fields[4],
        fields[5],
        fields[6]
    ])?;
    Ok(())
}
pub(crate) const INSERT_ROW: &str =
    "INSERT INTO corpus_original_rows VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)";

/// The opaque capture plan's original authority stays separate from the stage.
/// This comparison is release composition, not a fabricated source ancestry.
pub fn retain_corpus_original(
    stage: &mut KnowledgeStage<'_>,
    vocab: &QueryVocabulary,
    plan: &CorpusOriginalPlan,
) -> Result<CorpusOriginalReceipt> {
    let result = (|| {
        let binding = &stage.exact_receipt().binding;
        if serde_json::to_vec(binding).map_err(|_| Error::Invalid("corpus composition binding"))?
            != serde_json::to_vec(&plan.binding)
                .map_err(|_| Error::Invalid("corpus plan binding"))?
            || vocab.descriptor_sha256 != plan.receipt.descriptor_sha256
        {
            return Err(Error::Invalid("corpus original selected composition"));
        }
        validate_receipt(&plan.receipt)?;
        let raw = serde_json::to_vec(&plan.receipt)
            .map_err(|_| Error::Invalid("corpus receipt encoding"))?;
        if raw.len() > JsonLimits::default().max_bytes {
            return Err(Error::Budget("corpus original receipt bytes"));
        }
        let rows = plan
            .rows
            .iter()
            .try_fold(1u64, |n, (_, r)| n.checked_add(r.len() as u64))
            .ok_or(Error::Budget("corpus original row count"))?;
        stage.charge_materialized(rows + 1, plan.receipt.total_bytes + raw.len() as u64)?;
        stage.with_connection(WritePhase::Finalize, |db| {
            let tx = db.transaction()?;
            tx.execute_batch(META_DDL)?;
            tx.execute_batch(ROW_DDL)?;
            for (_, ddl) in INDEXES {
                tx.execute_batch(ddl)?;
            }
            tx.execute(
                "INSERT INTO corpus_original_meta VALUES(1,?1)",
                [raw.as_slice()],
            )?;
            let mut insert = tx.prepare(INSERT_ROW)?;
            let header = vec![plan.header.clone()];
            for (collection, rows) in std::iter::once((CorpusOriginalCollection::Header, &header))
                .chain(plan.rows.iter().map(|(c, r)| (*c, r)))
            {
                for (ordinal, raw) in rows.iter().enumerate() {
                    insert_original_row(&mut insert, collection, ordinal as u64, raw)?;
                }
            }
            drop(insert);
            tx.commit()?;
            Ok(())
        })?;
        Ok(plan.receipt.clone())
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
pub(crate) fn present(db: &Connection) -> Result<bool> {
    let n:i64=db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('corpus_original_meta','corpus_original_rows')",[],|r|r.get(0))?;
    if n != 0 && n != 2 {
        return Err(Error::Invalid("corpus original partial tables"));
    }
    Ok(n == 2)
}
pub(crate) fn verify_ddl(db: &Connection) -> Result<()> {
    for (kind, name, ddl) in [
        ("table", META_TABLE, META_DDL),
        ("table", ROW_TABLE, ROW_DDL),
    ]
    .into_iter()
    .chain(INDEXES.into_iter().map(|(n, d)| ("index", n, d)))
    {
        let sql:Option<String>=db.query_row("SELECT CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=4096 THEN sql ELSE NULL END FROM sqlite_master WHERE type=?1 AND name=?2",params![kind,name],|r|r.get(0)).optional()?.flatten();
        if sql.as_deref() != Some(ddl) {
            return Err(Error::Invalid("corpus original DDL"));
        }
    }
    Ok(())
}
fn receipt(db: &Connection) -> Result<CorpusOriginalReceipt> {
    let n: i64 = db.query_row("SELECT count(*) FROM corpus_original_meta", [], |r| {
        r.get(0)
    })?;
    if n != 1 {
        return Err(Error::Invalid("corpus original receipt count"));
    }
    let raw:Option<Vec<u8>>=db.query_row("SELECT CASE WHEN typeof(receipt)='blob' AND length(receipt)<=?1 THEN receipt ELSE NULL END FROM corpus_original_meta WHERE singleton=1",[JsonLimits::default().max_bytes as i64],|r|r.get(0))?;
    let raw = raw.ok_or(Error::Budget("corpus original receipt bytes"))?;
    parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default())
        .map_err(|e| Error::Source(e.to_string()))?;
    let r = serde_json::from_slice(&raw).map_err(|_| Error::Invalid("corpus original receipt"))?;
    validate_receipt(&r)?;
    Ok(r)
}
fn selection(
    collection: CorpusOriginalCollection,
    s: &CorpusOriginalSelector,
) -> Result<(String, Vec<String>)> {
    let valid = |v: &str| -> Result<String> {
        if v.len() > MAX_INDEX_TEXT_BYTES {
            return Err(Error::Budget("corpus selector bytes"));
        }
        Ok(v.into())
    };
    let scalar = |field: &str, v: &str| Ok((format!("{field}=?"), vec![valid(v)?]));
    let set = |field: &str, vs: &[String]| -> Result<(String, Vec<String>)> {
        if vs.len() > 1024 {
            return Err(Error::Budget("corpus selector set"));
        }
        let vs = vs.iter().map(|v| valid(v)).collect::<Result<Vec<_>>>()?;
        Ok((
            if vs.is_empty() {
                "0".into()
            } else {
                format!("{field} IN ({})", vec!["?"; vs.len()].join(","))
            },
            vs,
        ))
    };
    use CorpusOriginalCollection as C;
    use CorpusOriginalSelector as S;
    match s {
        S::All=>Ok(("1".into(),Vec::new())),
        S::NodeId(id) if collection==C::Nodes=>scalar("node_id",id),
        S::IncidentNode(id) if collection==C::RelationEdges=>Ok(("ordinal IN (SELECT ordinal FROM corpus_original_rows INDEXED BY corpus_original_from WHERE collection=? AND from_id=? UNION SELECT ordinal FROM corpus_original_rows INDEXED BY corpus_original_to WHERE collection=? AND to_id=?)".into(),vec![collection.as_str().into(),valid(id)?,collection.as_str().into(),valid(id)?])),
        S::PackId(id) if [C::RelationPacks,C::RelationEdges].contains(&collection)=>scalar("pack_id",id),
        S::ViewId(id) if collection==C::GraphViews=>scalar("view_id",id),
        S::NodeIds(ids) if collection==C::Nodes=>set("node_id",ids),
        S::PackIds(ids) if [C::RelationPacks,C::RelationEdges].contains(&collection)=>set("pack_id",ids),
        S::OwnerBranch(id) if [C::RelationPacks,C::RelationEdges].contains(&collection)=>scalar("owner_branch",id),
        S::Resources{resource_kind,owner_branch} if collection==C::Resources=>{
            let mut clauses=Vec::new();let mut args=Vec::new();
            if let Some(v)=resource_kind{clauses.push("resource_kind=?");args.push(valid(v)?);}
            if let Some(v)=owner_branch{clauses.push("owner_branch=?");args.push(valid(v)?);}
            Ok((if clauses.is_empty(){"1".into()}else{clauses.join(" AND ")},args))
        },_=>Err(Error::Invalid("corpus selector collection")),
    }
}
pub(crate) fn page(
    db: &Connection,
    collection: CorpusOriginalCollection,
    selector: &CorpusOriginalSelector,
    after: Option<u64>,
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
) -> Result<CorpusOriginalPage> {
    crate::knowledge_original_rows::page_limits(max_rows, max_row_bytes, max_page_bytes)?;
    if after.is_some_and(|n| n > i64::MAX as u64) {
        return Err(Error::Budget("corpus page ordinal"));
    }
    let (where_sql, args) = selection(collection, selector)?;
    use CorpusOriginalSelector as S;
    let index = match selector {
        S::NodeId(_) | S::NodeIds(_) => " INDEXED BY corpus_original_node",
        S::PackId(_) | S::PackIds(_) => " INDEXED BY corpus_original_pack",
        S::ViewId(_) => " INDEXED BY corpus_original_view",
        S::OwnerBranch(_) => " INDEXED BY corpus_original_owner",
        S::Resources {
            resource_kind: Some(_),
            ..
        } => " INDEXED BY corpus_original_resource",
        S::Resources {
            resource_kind: None,
            owner_branch: Some(_),
        } => " INDEXED BY corpus_original_owner",
        _ => "",
    };
    let sql = format!(
        "SELECT ordinal,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND packet_len=length(packet) AND length(packet)<=? THEN packet ELSE NULL END FROM corpus_original_rows{index} WHERE collection=? AND ordinal>? AND ({where_sql}) ORDER BY ordinal LIMIT ?"
    );
    let mut values = vec![
        rusqlite::types::Value::Integer(max_row_bytes as i64),
        rusqlite::types::Value::Text(collection.as_str().into()),
        rusqlite::types::Value::Integer(after.map_or(-1, |n| n as i64)),
    ];
    values.extend(args.into_iter().map(rusqlite::types::Value::Text));
    values.push(rusqlite::types::Value::Integer(max_rows as i64));
    let mut q = db.prepare(&sql)?;
    let mut scan = q.query(params_from_iter(values))?;
    let (rows, bytes) = crate::knowledge_original_rows::read(&mut scan, max_page_bytes)?;
    let next_ordinal = if rows.len() == max_rows {
        rows.last().map(|r| r.0 as u64)
    } else {
        None
    };
    let rows = rows
        .into_iter()
        .map(|(n, raw)| CorpusOriginalRow {
            ordinal: n as u64,
            raw_sha256: Digest256::of_bytes(&raw).to_hex(),
            raw,
        })
        .collect();
    Ok(CorpusOriginalPage {
        rows,
        next_ordinal,
        decoded_bytes: bytes,
        vm_steps: 0,
    })
}

pub(crate) fn view_identities(
    db: &Connection,
    after: Option<u64>,
    max_rows: usize,
    max_id_bytes: usize,
    max_page_bytes: u64,
) -> Result<CorpusOriginalViewIdentityPage> {
    let max_id_bytes = max_id_bytes.min(MAX_INDEX_TEXT_BYTES);
    if max_id_bytes == 0 {
        return Err(Error::Budget("corpus view identity ID bytes"));
    }
    let row_bytes = max_id_bytes
        .checked_add(std::mem::size_of::<u64>() + 64)
        .ok_or(Error::Budget("corpus view identity row bytes"))?;
    crate::knowledge_original_rows::page_limits(max_rows, row_bytes, max_page_bytes)?;
    if after.is_some_and(|n| n > i64::MAX as u64) {
        return Err(Error::Budget("corpus view identity ordinal"));
    }
    // The selected cold opener already proves these index values against each
    // original packet and root. Re-reading bodies would defeat this projection.
    let mut query = db.prepare(
        "SELECT ordinal,CASE WHEN view_id IS NULL THEN NULL WHEN typeof(view_id)='text' THEN length(CAST(view_id AS BLOB)) ELSE -1 END,CASE WHEN typeof(view_id)='text' AND length(CAST(view_id AS BLOB))<=?1 THEN view_id ELSE NULL END,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN lower(hex(packet_sha256)) ELSE NULL END FROM corpus_original_rows WHERE collection='graph_views' AND ordinal>?2 ORDER BY ordinal LIMIT ?3",
    )?;
    let mut scan = query.query(params![
        max_id_bytes as i64,
        after.map_or(-1, |n| n as i64),
        max_rows as i64
    ])?;
    let mut rows = Vec::new();
    let mut decoded_bytes = 0u64;
    let mut previous = after;
    while let Some(row) = scan.next()? {
        let ordinal: i64 = row.get(0)?;
        if ordinal < 0 || previous.is_some_and(|n| ordinal as u64 <= n) {
            return Err(Error::Invalid("corpus view identity order"));
        }
        let id_bytes: Option<i64> = row.get(1)?;
        if id_bytes.is_some_and(|n| n < 0) {
            return Err(Error::Invalid("corpus view identity scalar kind"));
        }
        if id_bytes.is_some_and(|n| n as u64 > max_id_bytes as u64) {
            return Err(Error::Budget("corpus view identity ID bytes"));
        }
        let view_id: Option<String> = row.get(2)?;
        if id_bytes != view_id.as_ref().map(|s| s.len() as i64) {
            return Err(Error::Invalid("corpus view identity ID length"));
        }
        let raw_sha256: String = row
            .get::<_, Option<String>>(3)?
            .ok_or(Error::Invalid("corpus view identity digest"))?;
        let bytes =
            std::mem::size_of::<u64>() + raw_sha256.len() + view_id.as_ref().map_or(0, String::len);
        decoded_bytes = decoded_bytes
            .checked_add(bytes as u64)
            .filter(|n| *n <= max_page_bytes)
            .ok_or(Error::Budget("corpus view identity page bytes"))?;
        previous = Some(ordinal as u64);
        rows.push(CorpusOriginalViewIdentity {
            ordinal: ordinal as u64,
            view_id,
            raw_sha256,
        });
    }
    Ok(CorpusOriginalViewIdentityPage {
        next_ordinal: (rows.len() == max_rows).then_some(previous).flatten(),
        rows,
        decoded_bytes,
        vm_steps: 0,
    })
}
fn verify_rows(
    db: &Connection,
    r: &CorpusOriginalReceipt,
    l: crate::NavigationOriginalLimits,
    work: &mut u64,
    cap: u64,
) -> Result<()> {
    l.validate()?;
    let mut total = 0u64;
    let mut all = 0u64;
    for collection in
        std::iter::once(CorpusOriginalCollection::Header).chain(CorpusOriginalCollection::ROWS)
    {
        let target = if collection == CorpusOriginalCollection::Header {
            1
        } else {
            r.collections
                .iter()
                .find(|c| c.collection == collection.as_str())
                .ok_or(Error::Invalid("corpus collection receipt"))?
                .rows
        };
        let mut hash = order_hash(collection.as_str());
        let mut after = None;
        let mut n = 0;
        loop {
            let p = page(
                db,
                collection,
                &CorpusOriginalSelector::All,
                after,
                1,
                l.max_row_bytes,
                l.max_row_bytes as u64,
            )?;
            for row in p.rows {
                if row.ordinal != n {
                    return Err(Error::Invalid("corpus original ordinal coverage"));
                }
                let v = values(&row.raw, l.max_row_bytes)?;
                if collection == CorpusOriginalCollection::Header {
                    if !v.is_object()
                        || v["schema_version"] != "tos_corpus_index_v1"
                        || row.raw_sha256 != r.header_sha256
                        || CorpusOriginalCollection::ROWS
                            .iter()
                            .any(|c| v.get(c.as_str()).is_some())
                        || v.get("source_navigation").is_some()
                    {
                        return Err(Error::Invalid("corpus detached original header"));
                    }
                } else {
                    order_item(&mut hash, n, &row.raw);
                }
                let fields = indexed_fields(collection, &v)?;
                // Stored lookup keys are mechanical indexes of exact originals,
                // verified before they can omit or substitute addressed rows.
                let mut q=db.prepare("SELECT CASE WHEN node_id IS NULL OR (typeof(node_id)='text' AND length(CAST(node_id AS BLOB))<=4096) THEN node_id ELSE X'00' END,CASE WHEN from_id IS NULL OR (typeof(from_id)='text' AND length(CAST(from_id AS BLOB))<=4096) THEN from_id ELSE X'00' END,CASE WHEN to_id IS NULL OR (typeof(to_id)='text' AND length(CAST(to_id AS BLOB))<=4096) THEN to_id ELSE X'00' END,CASE WHEN pack_id IS NULL OR (typeof(pack_id)='text' AND length(CAST(pack_id AS BLOB))<=4096) THEN pack_id ELSE X'00' END,CASE WHEN view_id IS NULL OR (typeof(view_id)='text' AND length(CAST(view_id AS BLOB))<=4096) THEN view_id ELSE X'00' END,CASE WHEN resource_kind IS NULL OR (typeof(resource_kind)='text' AND length(CAST(resource_kind AS BLOB))<=4096) THEN resource_kind ELSE X'00' END,CASE WHEN owner_branch IS NULL OR (typeof(owner_branch)='text' AND length(CAST(owner_branch AS BLOB))<=4096) THEN owner_branch ELSE X'00' END FROM corpus_original_rows WHERE collection=?1 AND ordinal=?2")?;
                let index = q.query_row(params![collection.as_str(), n as i64], |row| {
                    let mut keys = Vec::new();
                    for i in 0..FIELDS.len() {
                        keys.push(row.get::<_, Option<String>>(i)?);
                    }
                    Ok(keys)
                })?;
                if index.iter().zip(fields).any(|(a, b)| a.as_deref() != b) {
                    return Err(Error::Invalid("corpus original lookup binding"));
                }
                let key_bytes = index.iter().flatten().map(|v| v.len() as u64).sum::<u64>();
                *work = work
                    .checked_add(row.raw.len() as u64 + key_bytes + 40)
                    .filter(|n| *n <= cap)
                    .ok_or(Error::Budget("corpus original cold work"))?;
                total = total
                    .checked_add(row.raw.len() as u64)
                    .filter(|n| *n <= l.max_total_bytes)
                    .ok_or(Error::Budget("corpus original cold bytes"))?;
                all = all
                    .checked_add(1)
                    .filter(|n| *n <= l.max_rows)
                    .ok_or(Error::Budget("corpus original cold rows"))?;
                n += 1;
                if n > target {
                    return Err(Error::Invalid("corpus excess rows"));
                }
            }
            after = p.next_ordinal;
            if after.is_none() {
                break;
            }
        }
        if n != target {
            return Err(Error::Invalid("corpus original count coverage"));
        }
        if collection != CorpusOriginalCollection::Header
            && hash.finalize().to_hex()
                != r.collections
                    .iter()
                    .find(|c| c.collection == collection.as_str())
                    .unwrap()
                    .ordered_root_sha256
        {
            return Err(Error::Invalid("corpus original order root"));
        }
    }
    let actual: i64 = db.query_row("SELECT count(*) FROM corpus_original_rows", [], |r| {
        r.get(0)
    })?;
    if actual as u64 != all || total != r.total_bytes {
        return Err(Error::Invalid("corpus original whole coverage"));
    }
    Ok(())
}
fn sealed_root(db: &Connection, r: &CorpusOriginalReceipt) -> Result<()> {
    let root:Option<String>=db.query_row("SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))=64 THEN value ELSE NULL END FROM metadata WHERE key='corpus_original_root_sha256'",[],|r|r.get(0)).optional()?.flatten();
    if root.as_deref() != Some(r.component_root_sha256.as_str()) {
        return Err(Error::Invalid("corpus original sealed root"));
    }
    Ok(())
}
pub(crate) fn verify_stage(
    stage: &mut KnowledgeStage<'_>,
    descriptor: Option<&str>,
) -> Result<Option<CorpusOriginalReceipt>> {
    let binding = stage.exact_receipt().binding.clone();
    stage.with_connection(WritePhase::Finalize, |db| {
        if !present(db)? {
            return Ok(None);
        }
        verify_ddl(db)?;
        let r = receipt(db)?;
        if r.source_cut != binding.source_cut
            || r.membership_root != binding.membership_root
            || descriptor.is_some_and(|d| d != r.descriptor_sha256)
        {
            return Err(Error::Invalid("corpus stage composition"));
        }
        verify_rows(
            db,
            &r,
            crate::knowledge_original_rows::maximum_limits(),
            &mut 0,
            crate::knowledge_original_rows::MAX_COLD_WORK,
        )?;
        if descriptor.is_none() {
            sealed_root(db, &r)?;
            let abi: String = db.query_row(
                "SELECT CAST(value AS TEXT) FROM metadata WHERE key='model_abi'",
                [],
                |r| r.get(0),
            )?;
            let sha: String = db.query_row(
                "SELECT CAST(value AS TEXT) FROM metadata WHERE key='descriptor_sha256'",
                [],
                |r| r.get(0),
            )?;
            if abi != KNOWLEDGE_CORPUS_MODEL_ABI || sha != r.descriptor_sha256 {
                return Err(Error::Invalid("corpus finish ABI/descriptor"));
            }
        }
        Ok(Some(r))
    })
}
pub(crate) fn verify(
    db: &Connection,
    e: &KnowledgeSelectedExpectation,
    l: ColdOpenLimits,
    work: &mut u64,
) -> Result<Option<CorpusOriginalReceipt>> {
    let found = present(db)?;
    if found != e.corpus_original_root_sha256.is_some()
        || found != (e.model_abi == KNOWLEDGE_CORPUS_MODEL_ABI)
    {
        return Err(Error::Invalid("corpus original ABI/expected presence"));
    }
    if !found {
        let n: i64 = db.query_row(
            "SELECT count(*) FROM metadata WHERE key='corpus_original_root_sha256'",
            [],
            |r| r.get(0),
        )?;
        if n != 0 {
            return Err(Error::Invalid("corpus phantom seal"));
        }
        return Ok(None);
    }
    verify_ddl(db)?;
    let r = receipt(db)?;
    if e.corpus_original_root_sha256.as_deref() != Some(r.component_root_sha256.as_str())
        || r.descriptor_sha256 != e.descriptor_sha256
        || r.source_cut != e.source_cut
        || r.membership_root != e.membership_root
    {
        return Err(Error::Invalid("corpus selected composition"));
    }
    let size = serde_json::to_vec(&r)
        .map_err(|_| Error::Invalid("corpus receipt encoding"))?
        .len() as u64;
    *work = work
        .checked_add(size)
        .filter(|n| *n <= l.max_work_bytes)
        .ok_or(Error::Budget("corpus receipt cold work"))?;
    verify_rows(
        db,
        &r,
        crate::NavigationOriginalLimits {
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
    )?;
    sealed_root(db, &r)?;
    Ok(Some(r))
}

// Existing captured source API is retained without a second implementation.
pub use retain_corpus_original as retain_captured_corpus_original;
