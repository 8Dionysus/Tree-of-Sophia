//! Exact navigation originals retained inside the selected immutable model.
//! This component binds projection custody, never current rights authority.
use crate::knowledge_selected::{ColdOpenLimits, KnowledgeSelectedExpectation};
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{Error, NavigationHeaderClaim, NavigationPrepareReceipt, QueryVocabulary, Result};
use rusqlite::{Connection, OptionalExtension, params};
use tos_foundation::{Digest256, Digest256Hasher};

pub const NAVIGATION_ORIGINAL_PROFILE: &str = "tos_navigation_original_v1";
pub const KNOWLEDGE_NAVIGATION_MODEL_ABI: &str = tos_foundation::KNOWLEDGE_MODEL_ABI_V3_POSTINGS_V1;
pub(crate) const META_TABLE: &str = "navigation_original_meta";
pub(crate) const ROW_TABLE: &str = "navigation_original_rows";
pub(crate) const MEMBER_TABLE: &str = "navigation_original_members";
pub(crate) const MEMBER_DDL: &str = "CREATE TABLE navigation_original_members(collection TEXT NOT NULL,id TEXT NOT NULL,raw_bytes INTEGER NOT NULL,raw_sha256 BLOB NOT NULL,semantic_sha256 BLOB NOT NULL,canonical_original_sha256 BLOB NOT NULL,PRIMARY KEY(collection,id)) WITHOUT ROWID";
pub(crate) const META_DDL: &str = "CREATE TABLE navigation_original_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1),profile TEXT NOT NULL,descriptor_sha256 TEXT NOT NULL,source_cut TEXT NOT NULL,membership_root TEXT NOT NULL,source_graph TEXT NOT NULL,node_count INTEGER NOT NULL,edge_count INTEGER NOT NULL,rights_count INTEGER NOT NULL,node_input_root BLOB NOT NULL,edge_input_root BLOB NOT NULL,header_sha256 BLOB NOT NULL,rights_root BLOB NOT NULL,component_root BLOB NOT NULL,total_bytes INTEGER NOT NULL,member_index_root BLOB NOT NULL,member_index_bytes INTEGER NOT NULL)";
pub(crate) const ROW_DDL: &str = "CREATE TABLE navigation_original_rows(ordinal INTEGER PRIMARY KEY,packet_len INTEGER NOT NULL,packet_sha256 BLOB NOT NULL,packet BLOB NOT NULL)";

#[derive(Clone, Copy, Debug)]
pub struct NavigationOriginalLimits {
    pub max_rows: u64,
    pub max_row_bytes: usize,
    pub max_total_bytes: u64,
}
impl NavigationOriginalLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_rows == 0
            || self.max_rows > crate::knowledge_original_rows::MAX_ROWS
            || self.max_row_bytes == 0
            || self.max_row_bytes > crate::knowledge_original_rows::MAX_ROW_BYTES
            || self.max_total_bytes == 0
            || self.max_total_bytes > crate::knowledge_original_rows::MAX_TOTAL_BYTES
        {
            return Err(Error::Budget("navigation original limits"));
        }
        Ok(())
    }
}
pub struct NavigationOriginalInput<'a> {
    pub rights: &'a [&'a [u8]],
    pub expected_rights_root_sha256: &'a str,
    pub limits: NavigationOriginalLimits,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationOriginalReceipt {
    pub profile: String,
    pub descriptor_sha256: String,
    pub source_cut: String,
    pub membership_root: String,
    pub source_graph: String,
    pub nodes: u64,
    pub edges: u64,
    pub rights: u64,
    pub node_input_root_sha256: String,
    pub edge_input_root_sha256: String,
    pub header_sha256: String,
    pub rights_root_sha256: String,
    pub component_root_sha256: String,
    pub total_bytes: u64,
    pub member_index_root_sha256: String,
    pub member_index_bytes: u64,
}
#[derive(Debug)]
pub struct NavigationOriginalPage {
    /// Complete original JSON bytes, in preserved input order. Ordinal -1 is
    /// the detached header; rights start at 0, with duplicates preserved.
    pub rows: Vec<(i64, Vec<u8>)>,
    pub next_ordinal: Option<i64>,
    pub decoded_bytes: u64,
    pub vm_steps: u64,
}
/// Membership of original raw nodes/edges, excluding generated placeholders.
/// Raw SHA binds lexical input; semantic SHA binds the maintained stable digest.
#[derive(Clone, Debug)]
pub struct NavigationOriginalMember {
    pub collection: String,
    pub id: String,
    pub raw_bytes: u64,
    pub raw_sha256: String,
    pub semantic_sha256: String,
    /// PublishedStrict + SourceRecordDigestV1 canonical original; preserves numeric kinds.
    pub canonical_original_sha256: String,
}
#[derive(Debug)]
pub struct NavigationOriginalMemberPage {
    pub rows: Vec<NavigationOriginalMember>,
    pub next_id: Option<String>,
    pub decoded_bytes: u64,
    /// Zero when the caller retains and accounts its own SQLite progress hook.
    pub vm_steps: u64,
}
fn member_charge(m: &NavigationOriginalMember) -> u64 {
    (m.collection.len() + m.id.len() + 192 + 8) as u64
}
fn member_hash() -> Digest256Hasher {
    let mut h = Digest256Hasher::new();
    hash_text(&mut h, "tos-navigation-original-members-v1");
    h
}
fn member_item(h: &mut Digest256Hasher, m: &NavigationOriginalMember) -> Result<()> {
    hash_text(h, &m.collection);
    hash_text(h, &m.id);
    h.update(&m.raw_bytes.to_be_bytes());
    for sha in [
        &m.raw_sha256,
        &m.semantic_sha256,
        &m.canonical_original_sha256,
    ] {
        h.update(
            Digest256::from_hex(sha)
                .map_err(|_| Error::Invalid("navigation original member digest"))?
                .as_bytes(),
        );
    }
    Ok(())
}
fn decode_member(row: &rusqlite::Row<'_>, collection: &str) -> Result<NavigationOriginalMember> {
    let id: String = row
        .get::<_, Option<String>>(0)?
        .ok_or(Error::Invalid("navigation original member ID"))?;
    let n: i64 = row.get(1)?;
    if n <= 0 || n > 8 * 1024 * 1024 {
        return Err(Error::Invalid("navigation original member raw length"));
    }
    Ok(NavigationOriginalMember {
        collection: collection.into(),
        id,
        raw_bytes: n as u64,
        raw_sha256: digest(
            row.get::<_, Option<Vec<u8>>>(2)?
                .ok_or(Error::Invalid("navigation original member raw SHA"))?,
        )?,
        semantic_sha256: digest(
            row.get::<_, Option<Vec<u8>>>(3)?
                .ok_or(Error::Invalid("navigation original member semantic SHA"))?,
        )?,
        canonical_original_sha256: digest(
            row.get::<_, Option<Vec<u8>>>(4)?
                .ok_or(Error::Invalid("navigation original member canonical SHA"))?,
        )?,
    })
}

/// Exact indexed membership under the already admitted immutable base.
/// Absence belongs to this same base; no catalogue scan is used.
pub(crate) fn member_exact(
    db: &Connection,
    collection: &str,
    id: &str,
    max_bytes: u64,
) -> Result<Option<NavigationOriginalMember>> {
    if !["nodes", "edges"].contains(&collection)
        || id.is_empty()
        || id.len() > 4096
        || max_bytes == 0
        || max_bytes > 64 * 1024 * 1024
        || max_bytes < (collection.len() + id.len() + 192 + 8) as u64
    {
        return Err(Error::Budget("navigation exact member limits"));
    }
    let mut stmt = db.prepare("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 4096 THEN id ELSE NULL END,raw_bytes,CASE WHEN typeof(raw_sha256)='blob' AND length(raw_sha256)=32 THEN raw_sha256 ELSE NULL END,CASE WHEN typeof(semantic_sha256)='blob' AND length(semantic_sha256)=32 THEN semantic_sha256 ELSE NULL END,CASE WHEN typeof(canonical_original_sha256)='blob' AND length(canonical_original_sha256)=32 THEN canonical_original_sha256 ELSE NULL END FROM navigation_original_members WHERE collection=?1 AND id=?2")?;
    let mut rows = stmt.query(params![collection, id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let member = decode_member(row, collection)?;
    if member.id != id || member_charge(&member) > max_bytes {
        return Err(Error::Budget("navigation exact member bytes/identity"));
    }
    Ok(Some(member))
}

pub(crate) fn member_page(
    db: &Connection,
    collection: &str,
    after: Option<&str>,
    max_rows: usize,
    max_page_bytes: u64,
) -> Result<NavigationOriginalMemberPage> {
    // A worst-case row has a 4096-byte ID, collection name, three hexadecimal
    // digests and a u64 length. Bound allocation before SQLite extracts text.
    const ROW_CAP: usize = 4096 + 5 + 192 + 8;
    if !["nodes", "edges"].contains(&collection)
        || max_rows == 0
        || max_rows > 1024
        || max_page_bytes == 0
        || max_page_bytes > 64 * 1024 * 1024
        || max_rows
            .checked_mul(ROW_CAP)
            .is_none_or(|n| n as u64 > max_page_bytes)
        || after.is_some_and(|id| id.len() > 4096)
    {
        return Err(Error::Budget("navigation original member page limits"));
    }
    let mut stmt=db.prepare("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 4096 THEN id ELSE NULL END,raw_bytes,CASE WHEN typeof(raw_sha256)='blob' AND length(raw_sha256)=32 THEN raw_sha256 ELSE NULL END,CASE WHEN typeof(semantic_sha256)='blob' AND length(semantic_sha256)=32 THEN semantic_sha256 ELSE NULL END,CASE WHEN typeof(canonical_original_sha256)='blob' AND length(canonical_original_sha256)=32 THEN canonical_original_sha256 ELSE NULL END FROM navigation_original_members WHERE collection=?1 AND id>?2 ORDER BY id LIMIT ?3")?;
    let mut scan = stmt.query(params![collection, after.unwrap_or(""), max_rows as i64])?;
    let mut rows = Vec::new();
    let mut decoded_bytes = 0u64;
    while let Some(row) = scan.next()? {
        let m = decode_member(row, collection)?;
        decoded_bytes = decoded_bytes
            .checked_add(member_charge(&m))
            .filter(|n| *n <= max_page_bytes)
            .ok_or(Error::Budget("navigation original member page bytes"))?;
        rows.push(m);
    }
    let next_id = if rows.len() == max_rows {
        rows.last().map(|m| m.id.clone())
    } else {
        None
    };
    Ok(NavigationOriginalMemberPage {
        rows,
        next_id,
        decoded_bytes,
        vm_steps: 0,
    })
}
fn verify_members(
    db: &Connection,
    r: &NavigationOriginalReceipt,
    row_cap: u64,
    work: &mut u64,
    work_cap: u64,
) -> Result<()> {
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut index = member_hash();
    for (collection, wanted, wanted_root) in [
        ("edges", r.edges, &r.edge_input_root_sha256),
        ("nodes", r.nodes, &r.node_input_root_sha256),
    ] {
        let mut after = None;
        let mut seen = 0u64;
        let mut input = Digest256Hasher::new();
        loop {
            let p = member_page(db, collection, after.as_deref(), 1, 8192)?;
            for m in &p.rows {
                member_item(&mut index, m)?;
                hash_text(&mut input, &m.id);
                input.update(
                    Digest256::from_hex(&m.raw_sha256)
                        .map_err(|_| Error::Invalid("navigation original input SHA"))?
                        .as_bytes(),
                );
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= row_cap)
                    .ok_or(Error::Budget("navigation original member count"))?;
                seen += 1;
                bytes = bytes
                    .checked_add(member_charge(m))
                    .ok_or(Error::Budget("navigation original member bytes"))?;
                *work = work
                    .checked_add(member_charge(m))
                    .filter(|n| *n <= work_cap)
                    .ok_or(Error::Budget("navigation original member cold work"))?;
            }
            after = p.next_id;
            if after.is_none() {
                break;
            }
        }
        if seen != wanted || input.finalize().to_hex() != *wanted_root {
            return Err(Error::Invalid("navigation original input coverage/root"));
        }
    }
    let total: i64 = db.query_row(
        "SELECT COUNT(*) FROM navigation_original_members",
        [],
        |row| row.get(0),
    )?;
    if total < 0
        || total as u64 != count
        || bytes != r.member_index_bytes
        || index.finalize().to_hex() != r.member_index_root_sha256
    {
        return Err(Error::Invalid("navigation original member index closure"));
    }
    Ok(())
}
fn hash_text(h: &mut Digest256Hasher, text: &str) {
    h.update(&(text.len() as u64).to_be_bytes());
    h.update(text.as_bytes());
}
fn row_hash(h: &mut Digest256Hasher, ordinal: i64, raw: &[u8]) {
    h.update(&ordinal.to_be_bytes());
    h.update(&(raw.len() as u64).to_be_bytes());
    h.update(Digest256::of_bytes(raw).as_bytes());
}
fn rights_hash() -> Digest256Hasher {
    let mut h = Digest256Hasher::new();
    hash_text(&mut h, "tos-navigation-original-rights-v1");
    h
}
fn root(r: &NavigationOriginalReceipt) -> Result<String> {
    let mut h = Digest256Hasher::new();
    for s in [
        NAVIGATION_ORIGINAL_PROFILE,
        &r.descriptor_sha256,
        &r.source_cut,
        &r.membership_root,
        &r.source_graph,
    ] {
        hash_text(&mut h, s);
    }
    for n in [
        r.nodes,
        r.edges,
        r.rights,
        r.total_bytes,
        r.member_index_bytes,
    ] {
        h.update(&n.to_be_bytes());
    }
    for s in [
        &r.node_input_root_sha256,
        &r.edge_input_root_sha256,
        &r.header_sha256,
        &r.rights_root_sha256,
        &r.member_index_root_sha256,
    ] {
        h.update(
            Digest256::from_hex(s)
                .map_err(|_| Error::Invalid("navigation original digest"))?
                .as_bytes(),
        );
    }
    Ok(h.finalize().to_hex())
}
fn header(raw: &[u8], r: &NavigationOriginalReceipt, cap: usize) -> Result<()> {
    let row = crate::knowledge_normalization::SourceRow::parse(raw, cap)?;
    let v = row.value();
    if v["schema_version"] != "tos_source_navigation_v1"
        || !v["authority_boundary"].is_string()
        || v.get("nodes").is_some()
        || v.get("edges").is_some()
        || v.get("rights").is_some()
        || v["counts"].as_object().is_none_or(|o| o.len() != 3)
        || v["counts"]["nodes"].as_u64() != Some(r.nodes)
        || v["counts"]["edges"].as_u64() != Some(r.edges)
        || v["counts"]["rights"].as_u64() != Some(r.rights)
    {
        return Err(Error::Invalid("navigation original header closure"));
    }
    Ok(())
}
fn right(raw: &[u8], cap: usize) -> Result<()> {
    let row = crate::knowledge_normalization::SourceRow::parse(raw, cap)?;
    if !row.value().is_object()
        || row.value()["rights_id"]
            .as_str()
            .is_none_or(|s| s.is_empty())
    {
        return Err(Error::Invalid("navigation original rights row"));
    }
    Ok(())
}
/// Independently supplied expected rights root binds the original ordered rows.
/// The same-cut owner must supply these originals alongside its prepared raw
/// navigation. Mechanical binding does not admit the source or current rights.
pub fn retain_navigation_original(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    prepared: &NavigationPrepareReceipt,
    claim: &NavigationHeaderClaim,
    rights: &[&[u8]],
    expected_rights_root_sha256: &str,
    limits: NavigationOriginalLimits,
) -> Result<NavigationOriginalReceipt> {
    let result = (|| {
        limits.validate()?;
        let binding = stage.exact_receipt()?.binding.clone();
        let source = vocabulary
            .sources
            .iter()
            .find(|s| s.source_graph_id == prepared.source_graph)
            .ok_or(Error::Invalid("navigation original registered source"))?;
        if vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == "source-navigation-node-edge-v1")
            .count()
            != 1
            || source.input_role != prepared.input_role
            || source.adapter_profile != "source-navigation-node-edge-v1"
            || prepared.source_cut != binding.source_cut
            || prepared.header_claim_sha256 != claim.expected_sha256
            || prepared.header_claim_rights_count != rights.len() as u64
            || rights.len() as u64 > limits.max_rows
            || Digest256::of_bytes(&claim.raw_json).to_hex() != claim.expected_sha256
        {
            return Err(Error::Invalid("navigation original prepared binding"));
        }
        for (name, count, digest) in [
            ("nodes", prepared.nodes, &prepared.node_input_root_sha256),
            ("edges", prepared.edges, &prepared.edge_input_root_sha256),
        ] {
            let entries: Vec<_> = stage
                .exact_receipt()?
                .collections
                .iter()
                .filter(|c| c.source_graph == source.source_graph_id && c.collection == name)
                .collect();
            if entries.len() != 1
                || entries[0].expected_count != count
                || &entries[0].expected_root_sha256 != digest
            {
                return Err(Error::Invalid("navigation original input closure"));
            }
        }
        let mut bytes = claim.raw_json.len() as u64;
        let mut rights_root = rights_hash();
        for (i, raw) in rights.iter().enumerate() {
            if raw.len() > limits.max_row_bytes {
                return Err(Error::Budget("navigation original row bytes"));
            }
            right(raw, limits.max_row_bytes)?;
            bytes = bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= limits.max_total_bytes)
                .ok_or(Error::Budget("navigation original bytes"))?;
            row_hash(&mut rights_root, i as i64, raw);
        }
        if claim.raw_json.len() > limits.max_row_bytes || bytes > limits.max_total_bytes {
            return Err(Error::Budget("navigation original header/total bytes"));
        }
        let actual_rights_root = rights_root.finalize().to_hex();
        if actual_rights_root != expected_rights_root_sha256 {
            return Err(Error::Invalid("navigation original expected rights root"));
        }
        let mut members = Vec::new();
        let mut member_bytes = 0u64;
        let mut index_root = member_hash();
        for collection in ["edges", "nodes"] {
            let mut after = None;
            loop {
                let page =
                    stage.scan_input(&prepared.source_graph, collection, after.as_deref(), 1)?;
                for raw in page.rows {
                    if raw.id.is_empty()
                        || raw.id.len() > 4096
                        || members.len() as u64 + rights.len() as u64 >= limits.max_rows
                    {
                        return Err(Error::Budget("navigation original member rows/ID"));
                    }
                    let parsed = crate::knowledge_normalization::SourceRow::parse(
                        &raw.payload,
                        limits.max_row_bytes,
                    )?;
                    let id_key = if collection == "nodes" {
                        "node_id"
                    } else {
                        "edge_id"
                    };
                    if parsed.value()[id_key].as_str() != Some(raw.id.as_str()) {
                        return Err(Error::Invalid("navigation original member identity"));
                    }
                    let member = NavigationOriginalMember {
                        collection: collection.into(),
                        id: raw.id,
                        raw_bytes: raw.payload.len() as u64,
                        raw_sha256: raw.payload_sha256,
                        semantic_sha256: parsed.stable_digest()?,
                        canonical_original_sha256: Digest256::of_bytes(
                            &tos_foundation::canonical_raw_bytes_v1(
                                &raw.payload,
                                tos_foundation::CanonicalProfile::SourceRecordDigestV1,
                                tos_foundation::JsonLimits::new(
                                    limits.max_row_bytes,
                                    96,
                                    1_000_000,
                                    4096,
                                )
                                .map_err(|_| {
                                    Error::Budget("navigation original canonical limits")
                                })?,
                            )
                            .map_err(|e| Error::Source(e.to_string()))?,
                        )
                        .to_hex(),
                    };
                    member_bytes = member_bytes
                        .checked_add(member_charge(&member))
                        .filter(|n| {
                            n.checked_add(bytes)
                                .is_some_and(|sum| sum <= limits.max_total_bytes)
                        })
                        .ok_or(Error::Budget("navigation original member bytes"))?;
                    member_item(&mut index_root, &member)?;
                    members.push(member);
                }
                after = page.next_id;
                if after.is_none() {
                    break;
                }
            }
        }
        let mut receipt = NavigationOriginalReceipt {
            profile: NAVIGATION_ORIGINAL_PROFILE.into(),
            descriptor_sha256: vocabulary.descriptor_sha256.clone(),
            source_cut: binding.source_cut.clone(),
            membership_root: binding.membership_root.clone(),
            source_graph: prepared.source_graph.clone(),
            nodes: prepared.nodes,
            edges: prepared.edges,
            rights: rights.len() as u64,
            node_input_root_sha256: prepared.node_input_root_sha256.clone(),
            edge_input_root_sha256: prepared.edge_input_root_sha256.clone(),
            header_sha256: claim.expected_sha256.clone(),
            rights_root_sha256: actual_rights_root,
            component_root_sha256: String::new(),
            total_bytes: bytes,
            member_index_root_sha256: index_root.finalize().to_hex(),
            member_index_bytes: member_bytes,
        };
        header(&claim.raw_json, &receipt, limits.max_row_bytes)?;
        receipt.component_root_sha256 = root(&receipt)?;
        stage.charge(&claim.raw_json)?;
        for raw in rights {
            stage.charge(raw)?;
        }
        stage.charge_materialized(
            1,
            (receipt.profile.len()
                + receipt.descriptor_sha256.len()
                + receipt.source_cut.len()
                + receipt.membership_root.len()
                + receipt.source_graph.len()
                + 6 * 32
                + 6 * 8) as u64,
        )?;
        stage.charge_materialized(members.len() as u64, member_bytes)?;
        stage.with_connection(WritePhase::Finalize, |db| {
            let tx = db.transaction()?;
            tx.execute_batch(META_DDL)?; tx.execute_batch(ROW_DDL)?; tx.execute_batch(MEMBER_DDL)?;
            tx.execute("INSERT INTO navigation_original_meta VALUES(1,?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)", params![receipt.profile,receipt.descriptor_sha256,receipt.source_cut,receipt.membership_root,receipt.source_graph,receipt.nodes as i64,receipt.edges as i64,receipt.rights as i64,Digest256::from_hex(&receipt.node_input_root_sha256).map_err(|_|Error::Invalid("navigation original input root"))?.as_bytes().as_slice(),Digest256::from_hex(&receipt.edge_input_root_sha256).map_err(|_|Error::Invalid("navigation original input root"))?.as_bytes().as_slice(),Digest256::from_hex(&receipt.header_sha256).map_err(|_|Error::Invalid("navigation original header root"))?.as_bytes().as_slice(),Digest256::from_hex(&receipt.rights_root_sha256).map_err(|_|Error::Invalid("navigation original rights root"))?.as_bytes().as_slice(),Digest256::from_hex(&receipt.component_root_sha256).map_err(|_|Error::Invalid("navigation original component root"))?.as_bytes().as_slice(),receipt.total_bytes as i64,Digest256::from_hex(&receipt.member_index_root_sha256).map_err(|_|Error::Invalid("navigation original member root"))?.as_bytes().as_slice(),receipt.member_index_bytes as i64])?;
            { let mut insert = tx.prepare("INSERT INTO navigation_original_rows VALUES(?1,?2,?3,?4)")?;
              for (ordinal, raw) in std::iter::once((-1i64, claim.raw_json.as_slice())).chain(rights.iter().enumerate().map(|(i,r)|(i as i64,*r))) {
                  insert.execute(params![ordinal, raw.len() as i64, Digest256::of_bytes(raw).as_bytes().as_slice(),raw])?;
              }
            }
            {let mut insert=tx.prepare("INSERT INTO navigation_original_members VALUES(?1,?2,?3,?4,?5,?6)")?;
             for member in &members {insert.execute(params![member.collection,member.id,member.raw_bytes as i64,Digest256::from_hex(&member.raw_sha256).map_err(|_|Error::Invalid("navigation original member SHA"))?.as_bytes().as_slice(),Digest256::from_hex(&member.semantic_sha256).map_err(|_|Error::Invalid("navigation original member semantic SHA"))?.as_bytes().as_slice(),Digest256::from_hex(&member.canonical_original_sha256).map_err(|_|Error::Invalid("navigation original member canonical SHA"))?.as_bytes().as_slice()])?;}}
            tx.commit()?; Ok(())
        })?;
        Ok(receipt)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
/// Root calculation for the same-cut producer/caller before retention. This
/// is not an admission receipt; callers must retain the independently expected
/// result with the prepared original navigation.
pub fn navigation_original_rights_root(rights: &[&[u8]]) -> String {
    navigation_original_rights_root_with_check(rights, &mut |_| Ok(()))
        .expect("unbudgeted navigation rights framing is infallible")
}

pub(crate) fn navigation_original_rights_root_with_check(
    rights: &[&[u8]],
    check: &mut impl FnMut(usize) -> Result<()>,
) -> Result<String> {
    let mut h = rights_hash();
    for (i, r) in rights.iter().enumerate() {
        check(r.len())?;
        row_hash(&mut h, i as i64, r);
    }
    Ok(h.finalize().to_hex())
}
/// Reuse the component's own framing law for the independent producer
/// companion. Source admission remains outside this mechanical consistency.
pub(crate) fn validate_producer_receipt(
    receipt: &NavigationOriginalReceipt,
    inputs: &[crate::knowledge_stage::InputCollectionReceipt],
) -> Result<()> {
    if root(receipt)? != receipt.component_root_sha256 {
        return Err(Error::Invalid(
            "navigation original producer component root",
        ));
    }
    for (collection, count, digest) in [
        ("nodes", receipt.nodes, &receipt.node_input_root_sha256),
        ("edges", receipt.edges, &receipt.edge_input_root_sha256),
    ] {
        let selected = inputs
            .iter()
            .find(|r| r.source_graph == receipt.source_graph && r.collection == collection)
            .ok_or(Error::Invalid(
                "navigation original producer input collection",
            ))?;
        if selected.expected_count != count || &selected.expected_root_sha256 != digest {
            return Err(Error::Invalid("navigation original producer input binding"));
        }
    }
    Ok(())
}

pub(crate) fn present(db: &Connection) -> Result<bool> {
    let mut seen = 0;
    for name in [META_TABLE, ROW_TABLE, MEMBER_TABLE] {
        seen += usize::from(
            db.query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some(),
        );
    }
    if seen != 0 && seen != 3 {
        return Err(Error::Invalid("navigation original partial tables"));
    }
    Ok(seen == 3)
}
pub(crate) fn verify_ddl(db: &Connection) -> Result<()> {
    for (name, expected) in [
        (META_TABLE, META_DDL),
        (ROW_TABLE, ROW_DDL),
        (MEMBER_TABLE, MEMBER_DDL),
    ] {
        let sql: Option<String> = db.query_row("SELECT CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=4096 THEN sql ELSE NULL END FROM sqlite_master WHERE type='table' AND name=?1",[name],|r|r.get(0)).optional()?.flatten();
        if sql.as_deref() != Some(expected) {
            return Err(Error::Invalid("navigation original component DDL"));
        }
    }
    Ok(())
}
fn digest(raw: Vec<u8>) -> Result<String> {
    if raw.len() != 32 {
        return Err(Error::Invalid("navigation original stored digest"));
    }
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}
pub(crate) fn receipt(db: &Connection) -> Result<NavigationOriginalReceipt> {
    if !present(db)? {
        return Err(Error::Invalid(
            "selected navigation original component unavailable",
        ));
    }
    let n: i64 = db.query_row("SELECT COUNT(*) FROM navigation_original_meta", [], |r| {
        r.get(0)
    })?;
    if n != 1 {
        return Err(Error::Invalid("navigation original metadata coverage"));
    }
    let mut result=db.query_row("SELECT CASE WHEN length(CAST(profile AS BLOB))<=128 THEN profile ELSE NULL END,CASE WHEN length(CAST(descriptor_sha256 AS BLOB))<=64 THEN descriptor_sha256 ELSE NULL END,CASE WHEN length(CAST(source_cut AS BLOB))<=4096 THEN source_cut ELSE NULL END,CASE WHEN length(CAST(membership_root AS BLOB))<=64 THEN membership_root ELSE NULL END,CASE WHEN length(CAST(source_graph AS BLOB))<=4096 THEN source_graph ELSE NULL END,node_count,edge_count,rights_count,CASE WHEN typeof(node_input_root)='blob' AND length(node_input_root)=32 THEN node_input_root ELSE NULL END,CASE WHEN typeof(edge_input_root)='blob' AND length(edge_input_root)=32 THEN edge_input_root ELSE NULL END,CASE WHEN typeof(header_sha256)='blob' AND length(header_sha256)=32 THEN header_sha256 ELSE NULL END,CASE WHEN typeof(rights_root)='blob' AND length(rights_root)=32 THEN rights_root ELSE NULL END,CASE WHEN typeof(component_root)='blob' AND length(component_root)=32 THEN component_root ELSE NULL END,total_bytes FROM navigation_original_meta WHERE singleton=1",[],|row| {
        // Bounded decoding remains in the compiler Result below.
        Ok((row.get::<_,Option<String>>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,Option<String>>(4)?,row.get::<_,i64>(5)?,row.get::<_,i64>(6)?,row.get::<_,i64>(7)?,row.get::<_,Option<Vec<u8>>>(8)?,row.get::<_,Option<Vec<u8>>>(9)?,row.get::<_,Option<Vec<u8>>>(10)?,row.get::<_,Option<Vec<u8>>>(11)?,row.get::<_,Option<Vec<u8>>>(12)?,row.get::<_,i64>(13)?))
    }).map_err(Error::from).and_then(|v| Ok(NavigationOriginalReceipt {
        profile:v.0.ok_or(Error::Budget("navigation original profile"))?,descriptor_sha256:v.1.ok_or(Error::Budget("navigation original descriptor"))?,source_cut:v.2.ok_or(Error::Budget("navigation original cut"))?,membership_root:v.3.ok_or(Error::Budget("navigation original membership"))?,source_graph:v.4.ok_or(Error::Budget("navigation original source"))?,
        nodes:u64::try_from(v.5).map_err(|_|Error::Invalid("navigation original count"))?,edges:u64::try_from(v.6).map_err(|_|Error::Invalid("navigation original count"))?,rights:u64::try_from(v.7).map_err(|_|Error::Invalid("navigation original count"))?,
        node_input_root_sha256:digest(v.8.ok_or(Error::Invalid("navigation original digest"))?)?,edge_input_root_sha256:digest(v.9.ok_or(Error::Invalid("navigation original digest"))?)?,header_sha256:digest(v.10.ok_or(Error::Invalid("navigation original digest"))?)?,rights_root_sha256:digest(v.11.ok_or(Error::Invalid("navigation original digest"))?)?,component_root_sha256:digest(v.12.ok_or(Error::Invalid("navigation original digest"))?)?,total_bytes:u64::try_from(v.13).map_err(|_|Error::Invalid("navigation original size"))?,member_index_root_sha256:String::new(),member_index_bytes:0,
    }))?;
    let (sha,size):(Option<Vec<u8>>,i64)=db.query_row("SELECT CASE WHEN typeof(member_index_root)='blob' AND length(member_index_root)=32 THEN member_index_root ELSE NULL END,member_index_bytes FROM navigation_original_meta WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
    result.member_index_root_sha256 =
        digest(sha.ok_or(Error::Invalid("navigation original member root"))?)?;
    result.member_index_bytes =
        u64::try_from(size).map_err(|_| Error::Invalid("navigation original member size"))?;
    Ok(result)
}
pub(crate) fn page(
    db: &Connection,
    after: Option<i64>,
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
) -> Result<NavigationOriginalPage> {
    crate::knowledge_original_rows::page_limits(max_rows, max_row_bytes, max_page_bytes)?;
    if after.is_some_and(|i| i < -1) {
        return Err(Error::Budget("navigation original page ordinal"));
    }
    let mut q=db.prepare("SELECT ordinal,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND packet_len=length(packet) AND length(packet)<=?2 THEN packet ELSE NULL END FROM navigation_original_rows WHERE ordinal>?1 ORDER BY ordinal LIMIT ?3")?;
    let mut scan = q.query(params![
        after.unwrap_or(-2),
        max_row_bytes as i64,
        max_rows as i64
    ])?;
    let (rows, bytes) = crate::knowledge_original_rows::read(
        db,
        &mut scan,
        max_page_bytes,
        max_row_bytes,
        crate::knowledge_stage::KnowledgePayloadLayout::InlineV1,
        &mut 0,
        crate::knowledge_original_rows::page_decode_work_limit(max_page_bytes)?,
    )?;
    let next_ordinal = if rows.len() == max_rows {
        rows.last().map(|r| r.0)
    } else {
        None
    };
    Ok(NavigationOriginalPage {
        rows,
        next_ordinal,
        decoded_bytes: bytes,
        vm_steps: 0,
    })
}
pub(crate) fn verify(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    l: ColdOpenLimits,
    work: &mut u64,
) -> Result<Option<NavigationOriginalReceipt>> {
    let present = present(db)?;
    if present != expected.navigation_original_root_sha256.is_some()
        || present
            && ![
                KNOWLEDGE_NAVIGATION_MODEL_ABI,
                crate::KNOWLEDGE_PHILOSOPHY_MODEL_ABI,
                crate::KNOWLEDGE_CORPUS_MODEL_ABI,
                crate::KNOWLEDGE_MANAGED_MODEL_ABI,
            ]
            .contains(&expected.model_abi.as_str())
    {
        return Err(Error::Invalid("navigation original model ABI coverage"));
    }
    if !present {
        let n: i64 = db.query_row(
            "SELECT COUNT(*) FROM metadata WHERE key='navigation_original_root_sha256'",
            [],
            |r| r.get(0),
        )?;
        if n != 0 {
            return Err(Error::Invalid("navigation original phantom seal"));
        }
        return Ok(None);
    }
    verify_ddl(db)?;
    let r = receipt(db)?;
    if expected.navigation_original_root_sha256.as_deref() != Some(r.component_root_sha256.as_str())
        || r.profile != NAVIGATION_ORIGINAL_PROFILE
        || r.descriptor_sha256 != expected.descriptor_sha256
        || r.source_cut != expected.source_cut
        || r.membership_root != expected.membership_root
        || r.rights.checked_add(1).is_none_or(|n| n > l.max_rows)
        || r.total_bytes > l.max_work_bytes
        || !expected.source_scopes.iter().any(|s| {
            s.source_graph == r.source_graph
                && s.adapter_profile == "source-navigation-node-edge-v1"
        })
        || root(&r)? != r.component_root_sha256
    {
        return Err(Error::Invalid("navigation original selected binding"));
    }
    *work = work
        .checked_add(
            (r.profile.len()
                + r.descriptor_sha256.len()
                + r.source_cut.len()
                + r.membership_root.len()
                + r.source_graph.len()
                + 6 * 32
                + 6 * 8) as u64,
        )
        .filter(|n| *n <= l.max_work_bytes)
        .ok_or(Error::Budget("navigation original cold binding bytes"))?;
    verify_rows(
        db,
        &r,
        NavigationOriginalLimits {
            max_rows: l.max_rows.saturating_sub(1),
            max_row_bytes: l.max_row_bytes.min(8 * 1024 * 1024),
            max_total_bytes: l.max_work_bytes.min(256 * 1024 * 1024),
        },
        work,
        l.max_work_bytes,
    )?;
    let meta:Option<String>=db.query_row("SELECT CAST(value AS TEXT) FROM metadata WHERE key='navigation_original_root_sha256' AND length(CAST(value AS BLOB))=64",[],|r|r.get(0)).optional()?;
    if meta.as_deref() != Some(&r.component_root_sha256) {
        return Err(Error::Invalid("navigation original sealed root"));
    }
    Ok(Some(r))
}

fn verify_rows(
    db: &Connection,
    r: &NavigationOriginalReceipt,
    l: NavigationOriginalLimits,
    work: &mut u64,
    work_cap: u64,
) -> Result<()> {
    if r.rights
        .checked_add(r.nodes)
        .and_then(|n| n.checked_add(r.edges))
        .is_none_or(|n| n > l.max_rows)
        || r.total_bytes
            .checked_add(r.member_index_bytes)
            .is_none_or(|n| n > l.max_total_bytes)
    {
        return Err(Error::Budget("navigation original aggregate limits"));
    }
    let mut after = None;
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut rights = rights_hash();
    loop {
        let p = page(
            db,
            after,
            1,
            l.max_row_bytes,
            (l.max_row_bytes as u64).min(64 * 1024 * 1024),
        )?;
        for (ordinal, raw) in &p.rows {
            let want = count as i64 - 1;
            if *ordinal != want {
                return Err(Error::Invalid("navigation original ordinal coverage"));
            }
            if *ordinal == -1 {
                header(raw, &r, l.max_row_bytes)?;
                if Digest256::of_bytes(raw).to_hex() != r.header_sha256 {
                    return Err(Error::Invalid("navigation original header SHA"));
                }
            } else {
                right(raw, l.max_row_bytes)?;
                row_hash(&mut rights, *ordinal, raw);
            }
            count = count
                .checked_add(1)
                .filter(|n| *n <= l.max_rows.saturating_add(1))
                .ok_or(Error::Budget("navigation original count"))?;
            bytes = bytes
                .checked_add(raw.len() as u64)
                .ok_or(Error::Budget("navigation original bytes"))?;
            *work = work
                .checked_add(raw.len() as u64 + 40)
                .filter(|n| *n <= work_cap)
                .ok_or(Error::Budget("navigation original cold work"))?;
        }
        after = p.next_ordinal;
        if after.is_none() {
            break;
        }
    }
    if count != r.rights + 1
        || bytes != r.total_bytes
        || rights.finalize().to_hex() != r.rights_root_sha256
    {
        return Err(Error::Invalid("navigation original root/coverage"));
    }
    verify_members(db, r, l.max_rows, work, work_cap)?;
    Ok(())
}
/// Recheck original component before seal and again before selected vacuum.
pub(crate) fn verify_stage(
    stage: &mut KnowledgeStage<'_>,
    descriptor: Option<&str>,
) -> Result<Option<NavigationOriginalReceipt>> {
    let layout = stage.payload_layout();
    let binding = stage.exact_receipt()?.binding.clone();
    stage.with_connection(WritePhase::Finalize, |db| {
        if !present(db)? {
            return Ok(None);
        }
        verify_ddl(db)?;
        let r = receipt(db)?;
        if r.profile != NAVIGATION_ORIGINAL_PROFILE
            || r.source_cut != binding.source_cut
            || r.membership_root != binding.membership_root
            || descriptor.is_some_and(|d| d != r.descriptor_sha256)
            || root(&r)? != r.component_root_sha256
        {
            return Err(Error::Invalid("navigation original stage binding"));
        }
        let mut work = 0;
        verify_rows(
            db,
            &r,
            crate::knowledge_original_rows::maximum_limits(),
            &mut work,
            crate::knowledge_original_rows::MAX_COLD_WORK,
        )?;
        if descriptor.is_none() {
            let abi: String=db.query_row("SELECT CAST(value AS TEXT) FROM metadata WHERE key='model_abi'",[],|r|r.get(0))?;
            let abi_matches = if let Some(expected) = layout.carrier_model_abi() {
                abi == expected
            } else {
                [KNOWLEDGE_NAVIGATION_MODEL_ABI, crate::KNOWLEDGE_PHILOSOPHY_MODEL_ABI, crate::KNOWLEDGE_CORPUS_MODEL_ABI, crate::KNOWLEDGE_MANAGED_MODEL_ABI].contains(&abi.as_str())
            };
            if !abi_matches { return Err(Error::Invalid("navigation original finish ABI")); }
            for (key,wanted) in [("descriptor_sha256",r.descriptor_sha256.as_str()),("navigation_original_root_sha256",r.component_root_sha256.as_str())] {
                let actual:Option<String>=db.query_row("SELECT CAST(value AS TEXT) FROM metadata WHERE key=?1 AND length(CAST(value AS BLOB))<=128",[key],|r|r.get(0)).optional()?;
                if actual.as_deref()!=Some(wanted){return Err(Error::Invalid("navigation original finish seal"));}
            }
        }
        Ok(Some(r))
    })
}

#[cfg(test)]
mod addressed_membership_tests {
    use super::*;

    #[test]
    fn exact_original_member_preserves_absence_and_refuses_corrupt_commitment() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(MEMBER_DDL).unwrap();
        let digest = Digest256::of_bytes(b"exact original");
        db.execute(
            "INSERT INTO navigation_original_members VALUES('nodes','agent:one',14,?1,?1,?1)",
            [digest.as_bytes().as_slice()],
        )
        .unwrap();
        let selected = member_exact(&db, "nodes", "agent:one", 8192)
            .unwrap()
            .unwrap();
        assert_eq!(selected.id, "agent:one");
        assert_eq!(selected.raw_sha256, digest.to_hex());
        assert!(
            member_exact(&db, "nodes", "agent:gap", 8192)
                .unwrap()
                .is_none()
        );
        assert!(
            member_exact(&db, "edges", "agent:one", 8192)
                .unwrap()
                .is_none()
        );
        assert!(member_exact(&db, "nodes", "agent:one", 1).is_err());
        db.execute("UPDATE navigation_original_members SET canonical_original_sha256=x'01' WHERE id='agent:one'", []).unwrap();
        assert!(member_exact(&db, "nodes", "agent:one", 8192).is_err());
    }
}

/// Same selected Navigation Original verifier under the active authentic
/// CreationState. The row and statement holds are acquired before bounded SQL
/// decode; aggregate work is charged before parsing, hashing or retaining each
/// row. The legacy unowned verifier remains unchanged for legacy callers.
pub(crate) fn verify_with_owned_state(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<Option<NavigationOriginalReceipt>> {
    use crate::d1_public_capture::CreationStateHold;
    let state = context.owned_state();
    let statement_bytes =
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound();
    let fixed = std::mem::size_of::<(
        &Connection,
        &KnowledgeSelectedExpectation,
        ColdOpenLimits,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        CreationStateHold<'_, '_>,
        Result<Option<NavigationOriginalReceipt>>,
        Option<NavigationOriginalReceipt>,
        Digest256Hasher,
        Result<String>,
        Result<()>,
    )>()
    .checked_add(
        statement_bytes
            .checked_mul(2)
            .ok_or(Error::Budget("owned navigation statement workspace"))?,
    )
    .ok_or(Error::Budget("owned navigation verifier frame"))?;
    let _fixed = state.hold(fixed)?;
    context.check()?;
    let receipt_bytes = 8 * 4096usize
        .checked_add(4096)
        .ok_or(Error::Budget("owned navigation receipt workspace"))?;
    let _receipt_hold = state.hold(receipt_bytes)?;
    // Bound the small metadata/decode frame before receipt() materializes its
    // explicitly capped strings and 32-byte digest carriers.
    state.charge_work(receipt_bytes)?;
    let present = present(db)?;
    if present != expected.navigation_original_root_sha256.is_some()
        || present
            && ![
                KNOWLEDGE_NAVIGATION_MODEL_ABI,
                crate::KNOWLEDGE_PHILOSOPHY_MODEL_ABI,
                crate::KNOWLEDGE_CORPUS_MODEL_ABI,
                crate::KNOWLEDGE_MANAGED_MODEL_ABI,
                crate::knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI,
                tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1,
                tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2,
                tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V3,
            ]
            .contains(&expected.model_abi.as_str())
    {
        return Err(Error::Invalid("navigation original model ABI coverage"));
    }
    if !present {
        let n: i64 = db.query_row(
            "SELECT COUNT(*) FROM metadata WHERE key='navigation_original_root_sha256'",
            [],
            |r| r.get(0),
        )?;
        if n != 0 {
            return Err(Error::Invalid("navigation original phantom seal"));
        }
        return Ok(None);
    }
    verify_ddl(db)?;
    let receipt = receipt(db)?;
    let receipt_work = receipt
        .profile
        .len()
        .checked_add(receipt.descriptor_sha256.len())
        .and_then(|n| n.checked_add(receipt.source_cut.len()))
        .and_then(|n| n.checked_add(receipt.membership_root.len()))
        .and_then(|n| n.checked_add(receipt.source_graph.len()))
        .and_then(|n| n.checked_add(6 * 32 + 6 * 8))
        .ok_or(Error::Budget("owned navigation receipt work"))?;
    state.charge_work(receipt_work)?;
    context.check()?;
    if expected.navigation_original_root_sha256.as_deref()
        != Some(receipt.component_root_sha256.as_str())
        || receipt.profile != NAVIGATION_ORIGINAL_PROFILE
        || receipt.descriptor_sha256 != expected.descriptor_sha256
        || receipt.source_cut != expected.source_cut
        || receipt.membership_root != expected.membership_root
        || receipt
            .rights
            .checked_add(1)
            .is_none_or(|n| n > limits.max_rows)
        || receipt.total_bytes > limits.max_work_bytes
        || !expected.source_scopes.iter().any(|scope| {
            scope.source_graph == receipt.source_graph
                && scope.adapter_profile == "source-navigation-node-edge-v1"
        })
        || root(&receipt)? != receipt.component_root_sha256
    {
        return Err(Error::Invalid("navigation original selected binding"));
    }
    let row_limits = NavigationOriginalLimits {
        max_rows: limits.max_rows.saturating_sub(1),
        max_row_bytes: limits.max_row_bytes.min(8 * 1024 * 1024),
        max_total_bytes: limits.max_work_bytes.min(256 * 1024 * 1024),
    };
    verify_rows_with_owned_state(db, &receipt, row_limits, context)?;
    let meta: Option<String> = db.query_row(
        "SELECT CAST(value AS TEXT) FROM metadata WHERE key='navigation_original_root_sha256' AND length(CAST(value AS BLOB))=64",
        [],
        |r| r.get(0),
    ).optional()?;
    state.charge_work(meta.as_ref().map_or(0, String::len))?;
    if meta.as_deref() != Some(&receipt.component_root_sha256) {
        return Err(Error::Invalid("navigation original sealed root"));
    }
    context.check()?;
    Ok(Some(receipt))
}

fn verify_rows_with_owned_state(
    db: &Connection,
    receipt: &NavigationOriginalReceipt,
    limits: NavigationOriginalLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    let state = context.owned_state();
    if receipt
        .rights
        .checked_add(receipt.nodes)
        .and_then(|n| n.checked_add(receipt.edges))
        .is_none_or(|n| n > limits.max_rows)
        || receipt
            .total_bytes
            .checked_add(receipt.member_index_bytes)
            .is_none_or(|n| n > limits.max_total_bytes)
    {
        return Err(Error::Budget("navigation original aggregate limits"));
    }
    let row_workspace = limits
        .max_row_bytes
        .checked_mul(3)
        .and_then(|n| n.checked_add(std::mem::size_of::<NavigationOriginalPage>() + 4096))
        .ok_or(Error::Budget("owned navigation row workspace"))?;
    let statement_bytes =
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound();
    let mut after = None;
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut rights = rights_hash();
    loop {
        context.check()?;
        let row_hold_bytes = row_workspace
            .checked_add(statement_bytes)
            .ok_or(Error::Budget("owned navigation row statement workspace"))?;
        let _row_hold = state.hold(row_hold_bytes)?;
        let page = page(
            db,
            after,
            1,
            limits.max_row_bytes,
            (limits.max_row_bytes as u64).min(64 * 1024 * 1024),
        )?;
        for (ordinal, raw) in &page.rows {
            // Charge the authenticated row bytes and its fixed hash fields before
            // JSON decode/canonical checks or digest work.
            state.charge_work(
                raw.len()
                    .checked_add(40)
                    .ok_or(Error::Budget("owned navigation row work"))?,
            )?;
            context.check()?;
            let want = count as i64 - 1;
            if *ordinal != want {
                return Err(Error::Invalid("navigation original ordinal coverage"));
            }
            if *ordinal == -1 {
                header(raw, receipt, limits.max_row_bytes)?;
                if Digest256::of_bytes(raw).to_hex() != receipt.header_sha256 {
                    return Err(Error::Invalid("navigation original header SHA"));
                }
            } else {
                right(raw, limits.max_row_bytes)?;
                row_hash(&mut rights, *ordinal, raw);
            }
            count = count
                .checked_add(1)
                .filter(|n| *n <= limits.max_rows.saturating_add(1))
                .ok_or(Error::Budget("navigation original count"))?;
            bytes = bytes
                .checked_add(raw.len() as u64)
                .ok_or(Error::Budget("navigation original bytes"))?;
        }
        after = page.next_ordinal;
        if after.is_none() {
            break;
        }
    }
    if count != receipt.rights + 1
        || bytes != receipt.total_bytes
        || rights.finalize().to_hex() != receipt.rights_root_sha256
    {
        return Err(Error::Invalid("navigation original root/coverage"));
    }
    verify_members_with_owned_state(db, receipt, limits.max_rows, context)
}

fn verify_members_with_owned_state(
    db: &Connection,
    receipt: &NavigationOriginalReceipt,
    row_cap: u64,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    let state = context.owned_state();
    let statement_bytes =
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound();
    let row_workspace = 8192usize
        .checked_add(16 * std::mem::size_of::<NavigationOriginalMember>() + 8192)
        .and_then(|n| n.checked_add(statement_bytes))
        .ok_or(Error::Budget("owned navigation member workspace"))?;
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut index = member_hash();
    for (collection, wanted, wanted_root) in [
        ("edges", receipt.edges, &receipt.edge_input_root_sha256),
        ("nodes", receipt.nodes, &receipt.node_input_root_sha256),
    ] {
        let mut after = None;
        let mut seen = 0u64;
        let mut input = Digest256Hasher::new();
        loop {
            context.check()?;
            let _row_hold = state.hold(row_workspace)?;
            let page = member_page(db, collection, after.as_deref(), 1, 8192)?;
            for member in &page.rows {
                let charge = member_charge(member) as usize;
                state.charge_work(charge)?;
                context.check()?;
                member_item(&mut index, member)?;
                hash_text(&mut input, &member.id);
                input.update(
                    Digest256::from_hex(&member.raw_sha256)
                        .map_err(|_| Error::Invalid("navigation original input SHA"))?
                        .as_bytes(),
                );
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= row_cap)
                    .ok_or(Error::Budget("navigation original member count"))?;
                seen += 1;
                bytes = bytes
                    .checked_add(member_charge(member))
                    .ok_or(Error::Budget("navigation original member bytes"))?;
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        if seen != wanted || input.finalize().to_hex() != *wanted_root {
            return Err(Error::Invalid("navigation original input coverage/root"));
        }
    }
    let total: i64 = db.query_row(
        "SELECT COUNT(*) FROM navigation_original_members",
        [],
        |row| row.get(0),
    )?;
    if total < 0
        || total as u64 != count
        || bytes != receipt.member_index_bytes
        || index.finalize().to_hex() != receipt.member_index_root_sha256
    {
        return Err(Error::Invalid("navigation original member index closure"));
    }
    Ok(())
}
