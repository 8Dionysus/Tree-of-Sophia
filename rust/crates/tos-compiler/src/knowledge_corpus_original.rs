//! Named original corpus projection custody. Capture identity is distinct from
//! authored-source identity; neither projection custody nor selection admits it.
use crate::d1_public_capture::CreationState;
use crate::knowledge_selected::{ColdOpenLimits, KnowledgeSelectedExpectation};
use crate::knowledge_selected::{
    owned_schema_equal, owned_schema_sql_error, owned_schema_step, with_owned_schema_statement,
};
use crate::knowledge_stage::{
    KNOWLEDGE_CARRIER_ONCE_MODEL_ABI, KnowledgePayloadLayout, KnowledgeStage, WritePhase,
};
use crate::{Error, QueryVocabulary, Result, SourceBinding};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::Value;
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, RelativePath, canonical_bytes_v1, parse_json,
};
use tos_foundation::{Digest256, Digest256Hasher};

pub const CORPUS_ORIGINAL_PROFILE: &str = "tos_corpus_original_v1";
pub const NATIVE_CORPUS_ORIGINAL_PROFILE: &str = "tos_corpus_original_native_v1";
pub const KNOWLEDGE_CORPUS_MODEL_ABI: &str = tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1;
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
pub(crate) const ROW_DDL_CARRIER: &str = "CREATE TABLE corpus_original_rows(collection TEXT NOT NULL,ordinal INTEGER NOT NULL CHECK(ordinal>=0),packet_len INTEGER NOT NULL,packet_sha256 BLOB NOT NULL,node_id TEXT,from_id TEXT,to_id TEXT,pack_id TEXT,view_id TEXT,resource_kind TEXT,owner_branch TEXT,PRIMARY KEY(collection,ordinal)) WITHOUT ROWID";
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
    component_root_with_state(r, None)
}
pub(crate) fn component_root_with_state(
    r: &CorpusOriginalReceipt,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<String> {
    if let Some(state) = state {
        if r.origin.native_producer.is_some() {
            return Err(Error::Invalid("controlled captured corpus origin required"));
        }
        let mut upper = std::mem::size_of::<CorpusOriginalReceipt>();
        for value in [
            &r.profile,
            &r.descriptor_sha256,
            &r.source_cut,
            &r.membership_root,
            &r.header_sha256,
            &r.component_root_sha256,
            &r.origin.profile,
            &r.origin.source_path,
            &r.origin.source_sha256,
            &r.origin.member_root_sha256,
        ] {
            upper = upper
                .checked_add(value.len())
                .ok_or(Error::Budget("owned corpus receipt strings"))?;
        }
        for value in [
            &r.origin.source_git_commit,
            &r.origin.source_git_tree,
            &r.origin.capture_manifest_sha256,
        ]
        .into_iter()
        .flatten()
        {
            upper = upper
                .checked_add(value.len())
                .ok_or(Error::Budget("owned corpus receipt optional strings"))?;
        }
        for member in &r.origin.members {
            upper = upper
                .checked_add(std::mem::size_of::<CorpusOriginalMember>())
                .and_then(|n| n.checked_add(member.path.len()))
                .and_then(|n| n.checked_add(member.sha256.len()))
                .ok_or(Error::Budget("owned corpus receipt members"))?;
        }
        for collection in &r.collections {
            upper = upper
                .checked_add(std::mem::size_of::<CorpusOriginalCollectionReceipt>())
                .and_then(|n| n.checked_add(collection.collection.len()))
                .and_then(|n| n.checked_add(collection.ordered_root_sha256.len()))
                .ok_or(Error::Budget("owned corpus receipt collections"))?;
        }
        state.retain(upper)?;
    }
    let mut body = r.clone();
    body.component_root_sha256.clear();
    if let Some(state) = state {
        return state.with_json_encoded(&body, JsonLimits::default().max_bytes, |raw| {
            let document = state.json(raw, JsonLimits::default().max_bytes)?;
            state.with_foundation_canonical_bytes(&document, JsonLimits::default(), |canonical| {
                state.charge_work(canonical.len())?;
                state.retain(64)?;
                let mut h = Digest256Hasher::new();
                h.update(b"tos-corpus-original-component-v1\0");
                h.update(canonical);
                Ok(h.finalize().to_hex())
            })
        });
    }
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
/// Digest of exactly the captured corpus member closure, not the full public
/// capture manifest. The preimage is the canonical SourceRecordDigestV1 flat
/// path-to-SHA256 map; sizes remain independently bound by member_root_sha256.
pub fn captured_runtime_input_manifest_digest(
    input_bindings: &Value,
    cap: usize,
) -> Result<Digest256> {
    let entries = input_bindings
        .as_object()
        .ok_or(Error::Invalid("runtime capture input bindings"))?;
    if entries.is_empty() || entries.len() > 65_536 {
        return Err(Error::Budget("runtime capture member count"));
    }
    for (path, sha) in entries {
        RelativePath::parse(path).map_err(|_| Error::Invalid("runtime capture member path"))?;
        Digest256::from_hex(
            sha.as_str()
                .ok_or(Error::Invalid("runtime capture member SHA"))?,
        )
        .map_err(|_| Error::Invalid("runtime capture member SHA"))?;
    }
    Ok(Digest256::of_bytes(
        &crate::knowledge_corpus_source::encode(input_bindings, cap)?,
    ))
}

pub(crate) fn validate_receipt(r: &CorpusOriginalReceipt) -> Result<()> {
    validate_receipt_with_state(r, None)
}
pub(crate) fn validate_receipt_with_state(
    r: &CorpusOriginalReceipt,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<()> {
    if let Some(state) = state {
        let max_path = r
            .origin
            .members
            .iter()
            .map(|member| member.path.len())
            .chain(std::iter::once(r.origin.source_path.len()))
            .max()
            .unwrap_or(0);
        state.retain(
            max_path
                .checked_mul(1 + std::mem::size_of::<String>())
                .and_then(|n| n.checked_add(64))
                .ok_or(Error::Budget("owned corpus validation paths"))?,
        )?;
    }
    if ![CORPUS_ORIGINAL_PROFILE, NATIVE_CORPUS_ORIGINAL_PROFILE].contains(&r.profile.as_str())
        || r.collections.len() != CorpusOriginalCollection::ROWS.len()
        || r.total_bytes == 0
        || r.total_bytes > crate::knowledge_original_rows::MAX_TOTAL_BYTES
        || component_root_with_state(r, state)? != r.component_root_sha256
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
    } else if r.origin.profile == "captured-runtime-projection-v1" {
        if r.profile != CORPUS_ORIGINAL_PROFILE
            || r.origin.source_git_commit.is_some()
            || r.origin.source_git_tree.is_some()
            || r.origin.native_producer.is_some()
        {
            return Err(Error::Invalid("mixed runtime corpus capture origin"));
        }
        if let Some(state) = state {
            let mut upper =
                crate::knowledge_normalization::serde_object_slots_upper(r.origin.members.len())?;
            for member in &r.origin.members {
                upper = upper
                    .checked_add(member.path.len())
                    .and_then(|n| n.checked_add(member.sha256.len()))
                    .ok_or(Error::Budget("owned corpus validation manifest strings"))?;
            }
            state.retain(
                upper
                    .checked_add(64)
                    .ok_or(Error::Budget("owned corpus validation manifest"))?,
            )?;
        }
        let mut entries = serde_json::Map::new();
        for member in &r.origin.members {
            if entries
                .insert(member.path.clone(), Value::String(member.sha256.clone()))
                .is_some()
            {
                return Err(Error::Invalid("duplicate runtime corpus member"));
            }
        }
        let entries = Value::Object(entries);
        let cap = crate::knowledge_original_rows::MAX_TOTAL_BYTES as usize;
        let digest = if let Some(state) = state {
            Digest256::of_bytes(&state.encode_canonical(&entries, cap)?)
        } else {
            captured_runtime_input_manifest_digest(&entries, cap)?
        };
        if r.origin.capture_manifest_sha256.as_deref() != Some(digest.to_hex().as_str()) {
            return Err(Error::Invalid("runtime corpus capture manifest SHA"));
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
/// Same indexed semantic row, with exact packet bytes already retained by
/// the Stage carrier owner before the metadata transaction starts.
pub(crate) fn insert_original_row_with_layout(
    insert: &mut rusqlite::Statement<'_>,
    collection: CorpusOriginalCollection,
    ordinal: u64,
    raw: &[u8],
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    if layout == KnowledgePayloadLayout::InlineV1 {
        return insert_original_row(insert, collection, ordinal, raw);
    }
    let value = values(raw, crate::knowledge_original_rows::MAX_ROW_BYTES)?;
    let fields = indexed_fields(collection, &value)?;
    insert.execute(params![
        collection.as_str(),
        ordinal as i64,
        raw.len() as i64,
        Digest256::of_bytes(raw).as_bytes().as_slice(),
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
/// Exact physical metadata size; packet bytes belong to the carrier owner.
/// Caller precharges the two parsing passes here, two insertion parsing passes
/// and insertion digest pass before this planning traversal (carrier hash and
/// collision comparison are charged separately by the carrier primitive).
pub(crate) fn preload_original_carrier(
    stage: &mut KnowledgeStage<'_>,
    collection: CorpusOriginalCollection,
    raw: &[u8],
) -> Result<u64> {
    let work = (raw.len() as u64)
        .checked_mul(5)
        .ok_or(Error::Budget("corpus carrier metadata work"))?;
    stage.charge_preparation_work(work)?;
    let value = values(raw, crate::knowledge_original_rows::MAX_ROW_BYTES)?;
    let fields = indexed_fields(collection, &value)?;
    let mut bytes = 48u64
        .checked_add(collection.as_str().len() as u64)
        .ok_or(Error::Budget("corpus carrier metadata bytes"))?;
    for field in fields.into_iter().flatten() {
        bytes = bytes
            .checked_add(field.len() as u64)
            .ok_or(Error::Budget("corpus carrier metadata bytes"))?;
    }
    stage.retain_exact_source_carrier_for_family(collection.as_str(), raw)?;
    Ok(bytes)
}

pub(crate) const INSERT_ROW_CARRIER: &str =
    "INSERT INTO corpus_original_rows VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)";

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
        let binding = &stage.exact_receipt()?.binding;
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
        let layout = stage.payload_layout();
        let physical_bytes = if layout.uses_carriers() {
            let mut bytes =
                preload_original_carrier(stage, CorpusOriginalCollection::Header, &plan.header)?;
            for (collection, packets) in &plan.rows {
                for packet in packets {
                    bytes = bytes
                        .checked_add(preload_original_carrier(stage, *collection, packet)?)
                        .ok_or(Error::Budget("corpus carrier metadata total"))?;
                }
            }
            bytes
        } else {
            plan.receipt.total_bytes
        };
        stage.charge_materialized(
            rows + 1,
            physical_bytes
                .checked_add(raw.len() as u64)
                .ok_or(Error::Budget("corpus original materialized bytes"))?,
        )?;
        stage.with_connection(WritePhase::Finalize, |db| {
            let tx = db.transaction()?;
            tx.execute_batch(META_DDL)?;
            tx.execute_batch(if layout.uses_carriers() {
                ROW_DDL_CARRIER
            } else {
                ROW_DDL
            })?;
            for (_, ddl) in INDEXES {
                tx.execute_batch(ddl)?;
            }
            tx.execute(
                "INSERT INTO corpus_original_meta VALUES(1,?1)",
                [raw.as_slice()],
            )?;
            let mut insert = tx.prepare(if layout.uses_carriers() {
                INSERT_ROW_CARRIER
            } else {
                INSERT_ROW
            })?;
            let header = vec![plan.header.clone()];
            for (collection, rows) in std::iter::once((CorpusOriginalCollection::Header, &header))
                .chain(plan.rows.iter().map(|(c, r)| (*c, r)))
            {
                for (ordinal, raw) in rows.iter().enumerate() {
                    insert_original_row_with_layout(
                        &mut insert,
                        collection,
                        ordinal as u64,
                        raw,
                        layout,
                    )?;
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
    verify_ddl_with_layout(db, KnowledgePayloadLayout::InlineV1)
}
pub(crate) fn verify_ddl_with_layout(
    db: &Connection,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    for (kind, name, ddl) in [
        ("table", META_TABLE, META_DDL),
        (
            "table",
            ROW_TABLE,
            if layout.uses_carriers() {
                ROW_DDL_CARRIER
            } else {
                ROW_DDL
            },
        ),
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
pub(crate) fn selection_index(selector: &CorpusOriginalSelector) -> &'static str {
    use CorpusOriginalSelector as S;
    match selector {
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
    }
}

/// Forecast the existing selector builder's SQL and argument copies before
/// its allocation. This creates no ledger and cannot grant a row read.
pub(crate) fn selection_workspace(selector: &CorpusOriginalSelector) -> Result<usize> {
    use CorpusOriginalSelector as S;
    let mut count = 0usize;
    let mut bytes = 0usize;
    let mut add = |value: &str| -> Result<()> {
        if value.len() > MAX_INDEX_TEXT_BYTES {
            return Err(Error::Budget("corpus selector bytes"));
        }
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("corpus selector count"))?;
        bytes = bytes
            .checked_add(value.len())
            .ok_or(Error::Budget("corpus selector state"))?;
        Ok(())
    };
    match selector {
        S::All => (),
        S::NodeId(v) | S::PackId(v) | S::ViewId(v) | S::OwnerBranch(v) => add(v)?,
        S::IncidentNode(v) => {
            add(v)?;
            add(v)?;
            add("relation_edges")?;
            add("relation_edges")?;
        }
        S::NodeIds(vs) | S::PackIds(vs) => {
            if vs.len() > 1024 {
                return Err(Error::Budget("corpus selector set"));
            }
            for v in vs {
                add(v)?;
            }
        }
        S::Resources {
            resource_kind,
            owner_branch,
        } => {
            if let Some(v) = resource_kind {
                add(v)?;
            }
            if let Some(v) = owner_branch {
                add(v)?;
            }
        }
    }
    bytes
        .checked_mul(3)
        .and_then(|n| n.checked_add(1024))
        .and_then(|n| {
            count
                .checked_mul(
                    4 * (std::mem::size_of::<String>()
                        + std::mem::size_of::<rusqlite::types::Value>())
                        + 8,
                )
                .and_then(|f| n.checked_add(f))
        })
        .ok_or(Error::Budget("corpus selector workspace"))
}

pub(crate) fn selection(
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
    page_with_layout(
        db,
        collection,
        selector,
        after,
        max_rows,
        max_row_bytes,
        max_page_bytes,
        KnowledgePayloadLayout::InlineV1,
    )
}
pub(crate) fn page_with_layout(
    db: &Connection,
    collection: CorpusOriginalCollection,
    selector: &CorpusOriginalSelector,
    after: Option<u64>,
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
    layout: KnowledgePayloadLayout,
) -> Result<CorpusOriginalPage> {
    let mut decode_work = 0;
    let work_cap = crate::knowledge_original_rows::page_decode_work_limit(max_page_bytes)?;
    page_with_layout_and_work(db, collection, selector, after, max_rows, max_row_bytes, max_page_bytes, layout, &mut decode_work, work_cap)
}
fn page_with_layout_and_work(
    db: &Connection,
    collection: CorpusOriginalCollection,
    selector: &CorpusOriginalSelector,
    after: Option<u64>,
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
    layout: KnowledgePayloadLayout,
    decode_work: &mut u64,
    work_cap: u64,
) -> Result<CorpusOriginalPage> {
    crate::knowledge_original_rows::page_limits(max_rows, max_row_bytes, max_page_bytes)?;
    if after.is_some_and(|n| n > i64::MAX as u64) {
        return Err(Error::Budget("corpus page ordinal"));
    }
    let (where_sql, args) = selection(collection, selector)?;
    use CorpusOriginalSelector as S;
    let index = selection_index(selector);
    let carrier_join = if layout.uses_carriers() {
        " LEFT JOIN knowledge_source_carriers USING(packet_sha256,packet_len)"
    } else {
        ""
    };
    let sql = format!(
        "SELECT ordinal,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND packet_len>=1 AND packet_len<=? AND length(packet)<=packet_len+17 THEN packet ELSE NULL END FROM corpus_original_rows{index}{carrier_join} WHERE collection=? AND ordinal>? AND ({where_sql}) ORDER BY ordinal LIMIT ?"
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
    let (rows, bytes) = crate::knowledge_original_rows::read(db, &mut scan, max_page_bytes, max_row_bytes, layout, decode_work, work_cap)?;
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

/// One maintained Selector::All row without a growing page/parameter container.
/// SQLite statement bookkeeping is outside logical owned buffers, as for the
/// selected model; the borrowed blob is copied only after state admission.
pub(crate) fn all_row_with_state_budget(
    db: &Connection,
    collection: CorpusOriginalCollection,
    after: Option<u64>,
    max_row_bytes: usize,
    max_page_bytes: u64,
    available: usize,
) -> Result<Option<CorpusOriginalRow>> {
    all_row_with_state_budget_and_layout(
        db,
        collection,
        after,
        max_row_bytes,
        max_page_bytes,
        available,
        KnowledgePayloadLayout::InlineV1,
    )
}
pub(crate) fn all_row_with_state_budget_and_layout(
    db: &Connection,
    collection: CorpusOriginalCollection,
    after: Option<u64>,
    max_row_bytes: usize,
    max_page_bytes: u64,
    available: usize,
    layout: KnowledgePayloadLayout,
) -> Result<Option<CorpusOriginalRow>> {
    crate::knowledge_original_rows::page_limits(1, max_row_bytes, max_page_bytes)?;
    if after.is_some_and(|n| n > i64::MAX as u64) {
        return Err(Error::Budget("corpus page ordinal"));
    }
    let sql = if layout.uses_carriers() {
        "SELECT ordinal,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 1 AND ?1 AND length(packet)<=packet_len+17 THEN packet ELSE NULL END FROM corpus_original_rows LEFT JOIN knowledge_source_carriers USING(packet_sha256,packet_len) WHERE collection=?2 AND ordinal>?3 ORDER BY ordinal LIMIT 1"
    } else {
        "SELECT ordinal,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND packet_len=length(packet) AND length(packet)<=?1 THEN packet ELSE NULL END FROM corpus_original_rows WHERE collection=?2 AND ordinal>?3 ORDER BY ordinal LIMIT 1"
    };
    let mut statement = db.prepare(sql)?;
    let mut scan = statement.query(params![
        max_row_bytes as i64,
        collection.as_str(),
        after.map_or(-1, |n| n as i64)
    ])?;
    let Some(row) = scan.next()? else {
        return Ok(None);
    };
    let ordinal: i64 = row.get(0)?;
    let size: i64 = row.get(1)?;
    let digest = row
        .get_ref(2)?
        .as_blob()
        .map_err(|_| Error::Invalid("original projection row digest"))?;
    let borrowed = row
        .get_ref(3)?
        .as_blob()
        .map_err(|_| Error::Budget("original projection row bytes"))?;
    if ordinal < 0
        || size < 0
        || size as u64 > max_page_bytes
        || size as usize > max_row_bytes
        || digest.len() != 32
    {
        return Err(Error::Invalid("original projection row identity"));
    }
    // The returned row's 64-byte hex digest and typed owner coexist with raw.
    let overhead = std::mem::size_of::<CorpusOriginalRow>()
        .checked_add(64)
        .ok_or(Error::Budget("corpus original state overflow"))?;
    let raw_available = available
        .checked_sub(overhead)
        .ok_or(Error::Budget("corpus original row state"))?;
    let mut decode_work = 0;
    let work_cap = crate::knowledge_original_rows::page_decode_work_limit(max_page_bytes)?;
    let raw = crate::knowledge_original_rows::decode_packet_from_connection(
        Some(db), borrowed, size as usize, max_row_bytes, raw_available, layout, &mut decode_work, work_cap,
    )?;
    crate::knowledge_original_rows::charge_decode_work(&mut decode_work, work_cap, raw.len())?;
    if digest != Digest256::of_bytes(&raw).as_bytes() {
        return Err(Error::Invalid("original projection row identity"));
    }
    let raw_sha256 = Digest256::of_bytes(&raw).to_hex();
    if raw
        .capacity()
        .checked_add(raw_sha256.capacity())
        .and_then(|n| n.checked_add(std::mem::size_of::<CorpusOriginalRow>()))
        .is_none_or(|n| n > available)
    {
        return Err(Error::Budget("corpus original row capacity"));
    }
    Ok(Some(CorpusOriginalRow {
        ordinal: ordinal as u64,
        raw_sha256,
        raw,
    }))
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
    layout: KnowledgePayloadLayout,
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
            let p = page_with_layout_and_work(
                db,
                collection,
                &CorpusOriginalSelector::All,
                after,
                1,
                l.max_row_bytes,
                l.max_row_bytes as u64,
                layout,
                work,
                cap,
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
    let layout = stage.payload_layout();
    let binding = stage.exact_receipt()?.binding.clone();
    stage.with_connection(WritePhase::Finalize, |db| {
        if !present(db)? {
            return Ok(None);
        }
        verify_ddl_with_layout(db, layout)?;
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
            layout,
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
            if abi
                != layout.carrier_model_abi().unwrap_or(KNOWLEDGE_CORPUS_MODEL_ABI)
                || sha != r.descriptor_sha256
            {
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
    let layout = crate::knowledge_stage::KnowledgePayloadLayout::from_model_abi(&e.model_abi);
    let found = present(db)?;
    if found != e.corpus_original_root_sha256.is_some()
        || found
            != ([KNOWLEDGE_CORPUS_MODEL_ABI, KNOWLEDGE_CARRIER_ONCE_MODEL_ABI, tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1, tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2]
                .contains(&e.model_abi.as_str()))
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
    verify_ddl_with_layout(db, layout)?;
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
        layout,
    )?;
    sealed_root(db, &r)?;
    Ok(Some(r))
}

// Existing captured source API is retained without a second implementation.
pub use retain_corpus_original as retain_captured_corpus_original;

// Owned cold verification path; reused selected-state SQL authority.
fn with_values<T>(
    raw: &[u8],
    cap: usize,
    state: Option<&CreationState<'_>>,
    operation: impl FnOnce(&Value) -> Result<T>,
) -> Result<T> {
    if let Some(state) = state {
        let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("corpus original JSON bounds"))?;
        state.with_serde_owned_with_limits(raw, limits, operation)
    } else {
        operation(&values(raw, cap)?)
    }
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
    let n=scalar_owned(db,c"SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('corpus_original_meta','corpus_original_rows')",state)?;
    if n != 0 && n != 2 {
        return Err(Error::Invalid("corpus original partial tables"));
    }
    Ok(n == 2)
}

fn receipt_with_state(
    db: &Connection,
    state: Option<&CreationState<'_>>,
) -> Result<CorpusOriginalReceipt> {
    let Some(state) = state else {
        return receipt(db);
    };
    let _sql = state.hold(
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(),
    )?;
    let n = scalar_owned(db, c"SELECT count(*) FROM corpus_original_meta", state)?;
    if n != 1 {
        return Err(Error::Invalid("corpus original receipt count"));
    }
    let r = with_owned_schema_statement(
        db,
        c"SELECT receipt FROM corpus_original_meta WHERE singleton=1",
        state,
        |q| {
            if !owned_schema_step(q, state)? {
                return Err(Error::Invalid("corpus receipt missing"));
            }
            let raw = q
                .value_ref(0)
                .map_err(owned_schema_sql_error)?
                .as_blob()
                .map_err(|_| Error::Invalid("original packet column type"))?;
            if raw.len() > JsonLimits::default().max_bytes {
                return Err(Error::Budget("corpus original receipt bytes"));
            }
            let r = state.with_serde_owned_value_with_limits(raw, JsonLimits::default(), |v| {
                let members = v["origin"]["members"].as_array().map_or(0, Vec::len);
                let collections = v["collections"].as_array().map_or(0, Vec::len);
                let bytes = raw
                    .len()
                    .checked_add(std::mem::size_of::<CorpusOriginalReceipt>())
                    .and_then(|n| {
                        n.checked_add(
                            members
                                .max(4)
                                .checked_mul(4 * std::mem::size_of::<CorpusOriginalMember>())?,
                        )
                    })
                    .and_then(|n| {
                        n.checked_add(collections.max(4).checked_mul(
                            4 * std::mem::size_of::<CorpusOriginalCollectionReceipt>(),
                        )?)
                    })
                    .ok_or(Error::Budget("corpus original typed receipt"))?;
                state.retain(bytes)?;
                state.charge_work(raw.len())?;
                serde_json::from_value(v).map_err(|_| Error::Invalid("corpus original receipt"))
            })?;
            Ok(r)
        },
    )?;
    validate_receipt_with_state(&r, Some(state))?;
    Ok(r)
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
    for (kind, name, ddl) in [
        ("table", META_TABLE, META_DDL),
        (
            "table",
            ROW_TABLE,
            if layout.uses_carriers() {
                ROW_DDL_CARRIER
            } else {
                ROW_DDL
            },
        ),
    ]
    .into_iter()
    .chain(INDEXES.into_iter().map(|(n, d)| ("index", n, d)))
    {
        with_owned_schema_statement(
            db,
            c"SELECT sql FROM sqlite_master WHERE type=?1 AND name=?2",
            state,
            |q| {
                state.charge_work(
                    kind.len()
                        .checked_add(name.len())
                        .ok_or(Error::Budget("original DDL bindings"))?,
                )?;
                state.active()?;
                q.bind_text(1, kind).map_err(owned_schema_sql_error)?;
                q.bind_text(2, name).map_err(owned_schema_sql_error)?;
                if !owned_schema_step(q, state)? {
                    return Err(Error::Invalid("corpus original DDL missing"));
                }
                let actual =
                    bounded_text(q.value_ref(0).map_err(owned_schema_sql_error)?, 4096, state)?;
                if !owned_schema_equal(state, actual.as_bytes(), ddl.as_bytes())? {
                    return Err(Error::Invalid("corpus original DDL"));
                }
                Ok(())
            },
        )?;
    }
    state.active()
}

fn verify_rows_owned(
    db: &Connection,
    r: &CorpusOriginalReceipt,
    l: crate::NavigationOriginalLimits,
    work: &mut u64,
    cap: u64,
    state: &CreationState<'_>,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    l.validate()?;
    let _sql = state.hold(
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(),
    )?;
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
        let mut n = 0u64;
        let sql = if layout.uses_carriers() {
            c"SELECT ordinal,packet_len,packet_sha256,packet,node_id,from_id,to_id,pack_id,view_id,resource_kind,owner_branch FROM corpus_original_rows LEFT JOIN knowledge_source_carriers USING(packet_sha256,packet_len) WHERE collection=?1 ORDER BY ordinal"
        } else {
            c"SELECT ordinal,packet_len,packet_sha256,packet,node_id,from_id,to_id,pack_id,view_id,resource_kind,owner_branch FROM corpus_original_rows WHERE collection=?1 ORDER BY ordinal"
        };
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
                    .map_err(|_| Error::Invalid("corpus original packet digest"))?;
                let raw = q
                    .value_ref(3)
                    .map_err(owned_schema_sql_error)?
                    .as_blob()
                    .map_err(|_| Error::Invalid("original packet column type"))?;
                if ordinal < 0
                    || ordinal as u64 != n
                    || declared > l.max_row_bytes as u64
                {
                    return Err(Error::Invalid("corpus original ordinal/length coverage"));
                }
                let mut index = [None; 7];
                let mut key_bytes = 0usize;
                for i in 0..FIELDS.len() {
                    index[i] = match q
                        .value_ref((i + 4) as i32)
                        .map_err(owned_schema_sql_error)?
                    {
                        rusqlite::types::ValueRef::Null => None,
                        rusqlite::types::ValueRef::Text(v) => {
                            if v.len() > 4096 {
                                return Err(Error::Budget("corpus original lookup bytes"));
                            }
                            *work = work
                                .checked_add(v.len() as u64)
                                .filter(|n| *n <= cap)
                                .ok_or(Error::Budget("corpus original lookup cold work"))?;
                            state.charge_work(v.len())?;
                            let v = std::str::from_utf8(v)
                                .map_err(|_| Error::Invalid("corpus original lookup UTF8"))?;
                            Some(v)
                        }
                        _ => return Err(Error::Invalid("corpus original lookup kind")),
                    };
                    key_bytes = key_bytes
                        .checked_add(index[i].map_or(0, str::len))
                        .ok_or(Error::Budget("corpus original key bytes"))?;
                }
                layout.with_sql_decoded(db, state, raw, Some(declared as usize), l.max_row_bytes, |raw| {
                let row_work = (raw.len() as u64)
                    .checked_add(40)
                    .ok_or(Error::Budget("corpus original row work"))?;
                *work = work
                    .checked_add(row_work)
                    .filter(|n| *n <= cap)
                    .ok_or(Error::Budget("corpus original cold work"))?;
                state.charge_work(
                    raw.len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(40))
                        .ok_or(Error::Budget("corpus original hash work"))?,
                )?;
                if Digest256::of_bytes(raw).as_bytes() != &digest {
                    return Err(Error::Invalid("corpus original packet digest differs"));
                }
                let _header_digest = state.hold(64)?;
                with_values(raw, l.max_row_bytes, Some(state), |v| {
                    if collection == CorpusOriginalCollection::Header {
                        if !v.is_object()
                            || v["schema_version"] != "tos_corpus_index_v1"
                            || Digest256::from_bytes(digest).to_hex() != r.header_sha256
                            || CorpusOriginalCollection::ROWS
                                .iter()
                                .any(|c| v.get(c.as_str()).is_some())
                            || v.get("source_navigation").is_some()
                        {
                            return Err(Error::Invalid("corpus detached original header"));
                        }
                    } else {
                        order_item(&mut hash, n, raw);
                    }
                    if index != indexed_fields(collection, v)? {
                        return Err(Error::Invalid("corpus original lookup binding"));
                    }
                    Ok(())
                })?;
                total = total
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= l.max_total_bytes)
                    .ok_or(Error::Budget("corpus original cold bytes"))?;
                all = all
                    .checked_add(1)
                    .filter(|n| *n <= l.max_rows)
                    .ok_or(Error::Budget("corpus original cold rows"))?;
                n = n
                    .checked_add(1)
                    .filter(|n| *n <= target)
                    .ok_or(Error::Invalid("corpus excess rows"))?;
                Ok(())
                })?;
            }
            Ok(())
        })?;
        let _digest = state.hold(64)?;
        if n != target {
            return Err(Error::Invalid("corpus original count coverage"));
        }
        if collection != CorpusOriginalCollection::Header
            && hash.finalize().to_hex()
                != r.collections
                    .iter()
                    .find(|c| c.collection == collection.as_str())
                    .ok_or(Error::Invalid("corpus collection receipt"))?
                    .ordered_root_sha256
        {
            return Err(Error::Invalid("corpus original order root"));
        }
    }
    let actual = scalar_owned(db, c"SELECT count(*) FROM corpus_original_rows", state)?;
    if actual < 0 || actual as u64 != all || total != r.total_bytes {
        return Err(Error::Invalid("corpus original whole coverage"));
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
            c"SELECT CAST(value AS TEXT) FROM metadata WHERE key=?1",
            state,
            |q| {
                state.charge_work(key.len())?;
                state.active()?;
                q.bind_text(1, key).map_err(owned_schema_sql_error)?;
                if !owned_schema_step(q, state)? {
                    return Ok(false);
                }
                let value =
                    bounded_text(q.value_ref(0).map_err(owned_schema_sql_error)?, 4096, state)?;
                owned_schema_equal(state, value.as_bytes(), expected.as_bytes())
            },
        );
    }
    let _hold=state.map(|s|s.hold(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())).transpose()?;
    let mut q = db.prepare("SELECT CAST(value AS TEXT) FROM metadata WHERE key=?1")?;
    let mut rows = q.query([key])?;
    let Some(row) = rows.next()? else {
        return Ok(false);
    };
    let value = if let Some(state) = state {
        bounded_text(row.get_ref(0)?, 4096, state)?
    } else {
        row.get_ref(0)?
            .as_str()
            .map_err(|_| Error::Invalid("original text column type"))?
    };
    Ok(value == expected)
}

fn sealed_root_with_state(
    db: &Connection,
    r: &CorpusOriginalReceipt,
    state: Option<&CreationState<'_>>,
) -> Result<()> {
    let Some(state) = state else {
        return sealed_root(db, r);
    };
    let _sql = state.hold(
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(),
    )?;
    with_owned_schema_statement(
        db,
        c"SELECT value FROM metadata WHERE key='corpus_original_root_sha256'",
        state,
        |q| {
            if !owned_schema_step(q, state)? {
                return Err(Error::Invalid("corpus original sealed root missing"));
            }
            let value = bounded_text(q.value_ref(0).map_err(owned_schema_sql_error)?, 64, state)?;
            if value.len() != 64
                || !owned_schema_equal(state, value.as_bytes(), r.component_root_sha256.as_bytes())?
            {
                return Err(Error::Invalid("corpus original sealed root"));
            }
            Ok(())
        },
    )
}

pub(crate) fn verify_with_owned_state(
    db: &Connection,
    e: &KnowledgeSelectedExpectation,
    l: ColdOpenLimits,
    work: &mut u64,
    state: Option<&CreationState<'_>>,
) -> Result<Option<CorpusOriginalReceipt>> {
    let layout = crate::knowledge_stage::KnowledgePayloadLayout::from_model_abi(&e.model_abi);
    let _presence_sql = state.map(|s|s.hold(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())).transpose()?;
    let found = present_with_state(db, state)?;
    if found != e.corpus_original_root_sha256.is_some()
        || found
            != ([KNOWLEDGE_CORPUS_MODEL_ABI, KNOWLEDGE_CARRIER_ONCE_MODEL_ABI, tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1, tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2]
                .contains(&e.model_abi.as_str()))
    {
        return Err(Error::Invalid("corpus original ABI/expected presence"));
    }
    if !found {
        let n: i64 = if let Some(state) = state {
            scalar_owned(
                db,
                c"SELECT count(*) FROM metadata WHERE key='corpus_original_root_sha256'",
                state,
            )?
        } else {
            db.query_row(
                "SELECT count(*) FROM metadata WHERE key='corpus_original_root_sha256'",
                [],
                |r| r.get(0),
            )?
        };
        if n != 0 {
            return Err(Error::Invalid("corpus phantom seal"));
        }
        return Ok(None);
    }
    verify_ddl_with_owned_state_and_layout(db, state, layout)?;
    let r = receipt_with_state(db, state)?;
    if e.corpus_original_root_sha256.as_deref() != Some(r.component_root_sha256.as_str())
        || r.descriptor_sha256 != e.descriptor_sha256
        || r.source_cut != e.source_cut
        || r.membership_root != e.membership_root
    {
        return Err(Error::Invalid("corpus selected composition"));
    }
    let size = if let Some(state) = state {
        state.with_json_encoded(&r, JsonLimits::default().max_bytes, |raw| {
            Ok(raw.len() as u64)
        })?
    } else {
        serde_json::to_vec(&r)
            .map_err(|_| Error::Invalid("corpus receipt encoding"))?
            .len() as u64
    };
    *work = work
        .checked_add(size)
        .filter(|n| *n <= l.max_work_bytes)
        .ok_or(Error::Budget("corpus receipt cold work"))?;
    verify_rows_with_state(
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
        state,
        layout,
    )?;
    sealed_root_with_state(db, &r, state)?;
    Ok(Some(r))
}

fn verify_rows_with_state(
    db: &Connection,
    r: &CorpusOriginalReceipt,
    l: crate::NavigationOriginalLimits,
    work: &mut u64,
    cap: u64,
    state: Option<&CreationState<'_>>,
    layout: KnowledgePayloadLayout,
) -> Result<()> {
    if let Some(state) = state {
        verify_rows_owned(db, r, l, work, cap, state, layout)
    } else {
        verify_rows(db, r, l, work, cap, layout)
    }
}
