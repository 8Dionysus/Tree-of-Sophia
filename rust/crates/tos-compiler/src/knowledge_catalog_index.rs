//! Disk-indexed catalog lookup rows derived from one bounded catalog receipt.
//! These rows are a private read model, not source admission or publication.

use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{
    Error, QueryVocabulary, Result,
    catalog::{CatalogPacketRef, CatalogReceipt, python_casefold_with_state},
    d1_public_capture::{CreationState, CreationStateHold},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::io::{self, Write};
use tos_foundation::{Digest256, Digest256Hasher};

const SCHEMA: &str = "tos_catalog_index_v1";
pub(crate) const ORDER_PROFILE: &str = "python-str-casefold-unicode16-v1";
pub(crate) const LEGACY_ORDER_PROFILE: &str = "python-str-casefold-v1-ascii-domain";

#[derive(Clone, Copy, Debug)]
pub struct CatalogIndexLimits {
    pub max_packet_bytes: usize,
    pub max_row_bytes: usize,
    pub max_index_rows: u64,
    pub max_index_bytes: u64,
    pub max_decoded_bytes: u64,
}
impl Default for CatalogIndexLimits {
    fn default() -> Self {
        Self {
            max_packet_bytes: 16 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_index_rows: 100_000,
            max_index_bytes: 64 * 1024 * 1024,
            max_decoded_bytes: 256 * 1024 * 1024,
        }
    }
}
impl CatalogIndexLimits {
    fn validate(self) -> Result<()> {
        if self.max_packet_bytes == 0
            || self.max_row_bytes == 0
            || self.max_index_rows == 0
            || self.max_index_bytes == 0
            || self.max_decoded_bytes == 0
        {
            return Err(Error::Budget("catalog index limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CatalogIndexReceipt {
    pub descriptor_sha256: String,
    pub catalog_packet_sha256: String,
    pub catalog_index_root_sha256: String,
    pub source_count: u64,
    pub facet_field_count: u64,
    pub facet_value_count: u64,
    pub route_count: u64,
    pub packet_bytes: u64,
}

struct CappedWriter {
    bytes: Vec<u8>,
    max: usize,
}
impl Write for CappedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("catalog packet overflow"))?;
        if next > self.max {
            return Err(io::Error::other("catalog packet cap"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Work<'state, 'budget> {
    limits: CatalogIndexLimits,
    rows: u64,
    bytes: u64,
    creation: Option<&'state CreationState<'budget>>,
}

struct DecodeBudget<'state, 'budget> {
    used: u64,
    max: u64,
    creation: Option<&'state CreationState<'budget>>,
}
impl<'state, 'budget> DecodeBudget<'state, 'budget> {
    fn new(max: u64, creation: Option<&'state CreationState<'budget>>) -> Self {
        Self {
            used: 0,
            max,
            creation,
        }
    }
    fn charge(&mut self, bytes: u64) -> Result<()> {
        if let Some(creation) = self.creation {
            creation.charge_work(
                usize::try_from(bytes).map_err(|_| Error::Budget("catalog index original work"))?,
            )?;
        }
        self.used = self
            .used
            .checked_add(bytes)
            .ok_or(Error::Budget("catalog index decoded bytes"))?;
        if self.used > self.max {
            return Err(Error::Budget("catalog index decoded bytes"));
        }
        Ok(())
    }
    fn charge_text(&mut self, parts: &[&str], overhead: u64) -> Result<()> {
        let mut total = overhead;
        for part in parts {
            total = total
                .checked_add(part.len() as u64)
                .ok_or(Error::Budget("catalog index decoded bytes"))?;
        }
        self.charge(total)
    }
    fn hold(&self, bytes: usize) -> Result<Option<CreationStateHold<'state, 'budget>>> {
        self.creation
            .map(|creation| creation.hold(bytes))
            .transpose()
    }
}
impl<'state, 'budget> Work<'state, 'budget> {
    fn new(limits: CatalogIndexLimits, creation: Option<&'state CreationState<'budget>>) -> Self {
        Self {
            limits,
            rows: 0,
            bytes: 0,
            creation,
        }
    }
    fn charge(&mut self, parts: &[&[u8]]) -> Result<()> {
        self.rows = self
            .rows
            .checked_add(1)
            .ok_or(Error::Budget("catalog index rows"))?;
        if self.rows > self.limits.max_index_rows {
            return Err(Error::Budget("catalog index rows"));
        }
        let mut n = 48u64;
        for part in parts {
            if part.len() > self.limits.max_row_bytes {
                return Err(Error::Budget("catalog index cell bytes"));
            }
            n = n
                .checked_add(part.len() as u64)
                .ok_or(Error::Budget("catalog index bytes"))?;
        }
        self.bytes = self
            .bytes
            .checked_add(n)
            .ok_or(Error::Budget("catalog index bytes"))?;
        if self.bytes > self.limits.max_index_bytes {
            return Err(Error::Budget("catalog index bytes"));
        }
        if let Some(creation) = self.creation {
            creation.charge_work(
                usize::try_from(n).map_err(|_| Error::Budget("catalog index original work"))?,
            )?;
        }
        Ok(())
    }
    fn charge_packet(&mut self, packet: &[u8]) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(packet.len() as u64)
            .ok_or(Error::Budget("catalog index bytes"))?;
        if self.bytes > self.limits.max_index_bytes {
            return Err(Error::Budget("catalog index bytes"));
        }
        if let Some(creation) = self.creation {
            creation.charge_work(packet.len())?;
        }
        Ok(())
    }
}

struct EncodedPacket<'state, 'budget> {
    bytes: Vec<u8>,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
impl std::ops::Deref for EncodedPacket<'_, '_> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

fn encode_packet<'state, 'budget, T: serde::Serialize + ?Sized>(
    value: &T,
    max: usize,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<EncodedPacket<'state, 'budget>> {
    if let Some(creation) = creation {
        return creation.with_json_encoded(value, max, |encoded| {
            let copy_and_check = encoded
                .len()
                .checked_mul(2)
                .ok_or(Error::Budget("catalog index packet work"))?;
            creation.charge_work(copy_and_check)?;
            let hold = creation.hold(encoded.len())?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(encoded.len())
                .map_err(|_| Error::Budget("catalog index packet allocation"))?;
            bytes.extend_from_slice(encoded);
            Ok(EncodedPacket {
                bytes,
                _hold: Some(hold),
            })
        });
    }
    let mut writer = CappedWriter {
        bytes: Vec::new(),
        max,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| Error::Budget("catalog index packet bytes"))?;
    Ok(EncodedPacket {
        bytes: writer.bytes,
        _hold: None,
    })
}

fn object<'a>(value: &'a Value, key: &str) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("catalog index object"))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value]> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("catalog index array"))
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("catalog index string"))
}
fn number(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("catalog index count"))
}
fn signed(n: u64) -> Result<i64> {
    i64::try_from(n).map_err(|_| Error::Budget("catalog index signed count"))
}
fn scalar_json<'state, 'budget>(
    value: &str,
    max_bytes: usize,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<(String, Option<CreationStateHold<'state, 'budget>>)> {
    if let Some(creation) = creation {
        return creation.with_json_encoded(value, max_bytes, |bytes| {
            let copy_and_check = bytes
                .len()
                .checked_mul(2)
                .ok_or(Error::Budget("catalog facet scalar work"))?;
            creation.charge_work(copy_and_check)?;
            let hold = creation.hold(bytes.len())?;
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| Error::Budget("catalog facet scalar allocation"))?;
            owned.extend_from_slice(bytes);
            let value = String::from_utf8(owned)
                .map_err(|_| Error::Invalid("catalog facet scalar UTF-8"))?;
            Ok((value, Some(hold)))
        });
    }
    let mut writer = CappedWriter {
        bytes: Vec::new(),
        max: max_bytes,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| Error::Budget("catalog facet scalar bytes"))?;
    let value = String::from_utf8(writer.bytes)
        .map_err(|_| Error::Invalid("catalog facet scalar UTF-8"))?;
    Ok((value, None))
}
fn hash_text(hash: &mut Digest256Hasher, value: &str) {
    hash.update(&(value.len() as u64).to_be_bytes());
    hash.update(value.as_bytes());
}
fn hash_number(hash: &mut Digest256Hasher, value: u64) {
    hash.update(&value.to_be_bytes());
}

const TABLES: &str = r#"
CREATE TABLE catalog_index_meta(
 descriptor_sha256 TEXT PRIMARY KEY, catalog_packet_sha256 TEXT NOT NULL,
 index_schema TEXT NOT NULL, order_profile TEXT NOT NULL,
 source_count INTEGER NOT NULL, facet_field_count INTEGER NOT NULL,
 facet_value_count INTEGER NOT NULL, route_count INTEGER NOT NULL,
 catalog_index_root_sha256 TEXT NOT NULL, packet_len INTEGER NOT NULL,
 packet_sha256 BLOB NOT NULL, packet BLOB NOT NULL) WITHOUT ROWID;
CREATE TABLE catalog_facet_fields(
 descriptor_sha256 TEXT NOT NULL, domain TEXT NOT NULL, field_id TEXT NOT NULL,
 value_count INTEGER NOT NULL, total_count INTEGER NOT NULL,
 PRIMARY KEY(descriptor_sha256,domain,field_id)) WITHOUT ROWID;
CREATE TABLE catalog_facets(
 descriptor_sha256 TEXT NOT NULL, domain TEXT NOT NULL, field_id TEXT NOT NULL,
 ordinal INTEGER NOT NULL, value_json TEXT NOT NULL, item_count INTEGER NOT NULL,
 PRIMARY KEY(descriptor_sha256,domain,field_id,ordinal),
 UNIQUE(descriptor_sha256,domain,field_id,value_json)) WITHOUT ROWID;
CREATE TABLE catalog_routes(
 descriptor_sha256 TEXT NOT NULL, route_id TEXT NOT NULL, ordinal INTEGER NOT NULL,
 node_count INTEGER NOT NULL, confirming_relation_count INTEGER NOT NULL,
 semantic_confirming_relation_count INTEGER NOT NULL,
 availability TEXT NOT NULL, role_readiness TEXT NOT NULL,
 packet_len INTEGER NOT NULL, packet_sha256 BLOB NOT NULL, packet BLOB NOT NULL,
 PRIMARY KEY(descriptor_sha256,route_id),
 UNIQUE(descriptor_sha256,ordinal)) WITHOUT ROWID;
CREATE TABLE catalog_source_counts(
 descriptor_sha256 TEXT NOT NULL, source_graph_id TEXT NOT NULL,
 node_count INTEGER NOT NULL, relation_count INTEGER NOT NULL,
 PRIMARY KEY(descriptor_sha256,source_graph_id)) WITHOUT ROWID;
"#;

/// Materialize a complete indexed companion to a catalog packet. The stage
/// host guard checks storage quota before and after the callback, and any
/// failure poisons the stage so that `finish` cannot publish partial rows.
pub fn materialize_catalog(
    stage: &mut KnowledgeStage<'_>,
    receipt: &CatalogReceipt,
    vocabulary: &QueryVocabulary,
    limits: CatalogIndexLimits,
) -> Result<CatalogIndexReceipt> {
    materialize_catalog_packet(stage, &receipt.as_packet(), vocabulary, limits)
}

pub(crate) fn materialize_catalog_packet(
    stage: &mut KnowledgeStage<'_>,
    receipt: &CatalogPacketRef<'_>,
    vocabulary: &QueryVocabulary,
    limits: CatalogIndexLimits,
) -> Result<CatalogIndexReceipt> {
    let creation = stage.owned_creation_state();
    let result = (|| {
        limits.validate()?;
        let packet = encode_packet(receipt, limits.max_packet_bytes, creation)?;
        let packet_sha = Digest256::of_bytes(&packet);
        let receipt_sha = Digest256::from_hex(receipt.sha256)
            .map_err(|_| Error::Invalid("catalog index receipt digest"))?;
        if packet_sha != receipt_sha {
            return Err(Error::Invalid("catalog index receipt digest"));
        }
        Digest256::from_hex(&vocabulary.descriptor_sha256)
            .map_err(|_| Error::Invalid("catalog index descriptor digest"))?;
        stage.with_connection(WritePhase::Catalog, |db| {
            db.execute_batch("SAVEPOINT cmp_catalog_index")?;
            let value = materialize_inner(
                db,
                receipt,
                vocabulary,
                limits,
                &packet,
                &packet_sha,
                creation,
            );
            if value.is_ok() {
                db.execute_batch("RELEASE cmp_catalog_index")?;
            } else {
                db.execute_batch("ROLLBACK TO cmp_catalog_index; RELEASE cmp_catalog_index")?;
            }
            value
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

fn materialize_inner(
    db: &mut Connection,
    receipt: &CatalogPacketRef<'_>,
    vocabulary: &QueryVocabulary,
    limits: CatalogIndexLimits,
    packet: &[u8],
    packet_sha: &Digest256,
    creation: Option<&CreationState<'_>>,
) -> Result<CatalogIndexReceipt> {
    db.execute_batch(TABLES)?;
    let mut work = Work::new(limits, creation);
    let mut decoded = DecodeBudget::new(limits.max_decoded_bytes, creation);
    work.charge_packet(packet)?;
    let desc = &vocabulary.descriptor_sha256;
    if ![
        "tos_knowledge_catalog_v1",
        crate::managed_source::MANAGED_CATALOG_SCHEMA,
    ]
    .contains(&string(receipt.body, "schema")?)
    {
        return Err(Error::Invalid("catalog index packet schema"));
    }
    if let Some(counts) = receipt.counts() {
        for (key, expected) in [
            ("nodes", receipt.node_count),
            ("relations", receipt.relation_count),
        ] {
            if counts.get(key).is_some() && number(counts, key)? != expected {
                return Err(Error::Invalid("catalog index packet count"));
            }
        }
    }
    let caps = receipt
        .body
        .get("capabilities")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid("catalog capabilities absent"))?;
    let facets = caps
        .get("facets")
        .ok_or(Error::Invalid("catalog facets absent"))?;
    let mut field_count = 0u64;
    let mut value_count = 0u64;
    for (packet_domain, domain, required) in [
        ("nodes", "node", &["source_graph", "kind_id", "type_id"][..]),
        (
            "relations",
            "relation",
            &["source_graph", "predicate_id", "relation_type_id"][..],
        ),
    ] {
        let fields = object(facets, packet_domain)?;
        for field in required {
            if !fields.contains_key(*field) {
                return Err(Error::Invalid("catalog required facet field absent"));
            }
        }
        for (field, values) in fields {
            if field.is_empty() || field.len() > limits.max_row_bytes {
                return Err(Error::Budget("catalog facet field bytes"));
            }
            let values = values
                .as_array()
                .ok_or(Error::Invalid("catalog facet array"))?;
            let mut total = 0u64;
            let mut previous_fold: Option<String> = None;
            let mut previous_fold_hold = None;
            for (ordinal, row) in values.iter().enumerate() {
                let value = string(row, "value")?;
                if value.is_empty() {
                    return Err(Error::Invalid("empty catalog facet value"));
                }
                let local_remaining = usize::try_from(decoded.max.saturating_sub(decoded.used))
                    .map_err(|_| Error::Budget("catalog casefold addressable bytes"))?;
                let original_remaining = creation
                    .map(|owner| owner.remaining(0))
                    .transpose()?
                    .unwrap_or(usize::MAX);
                let output_cap = value
                    .len()
                    .checked_mul(3)
                    .ok_or(Error::Budget("catalog casefold addressable bytes"))?
                    .min(limits.max_row_bytes)
                    .min(local_remaining)
                    .min(original_remaining);
                let (folded, fold_hold) = if let Some(owner) = creation {
                    let (folded, hold) = python_casefold_with_state(value, owner, output_cap)?;
                    (folded, Some(hold))
                } else {
                    let folded = tos_foundation::python_casefold_unicode16_v1(
                        value,
                        limits.max_row_bytes,
                        output_cap,
                        output_cap,
                    )
                    .map_err(|_| Error::Budget("catalog facet casefold bytes"))?;
                    (folded, None)
                };
                if previous_fold
                    .as_ref()
                    .is_some_and(|previous| previous > &folded)
                {
                    return Err(Error::Invalid("catalog facet casefold order"));
                }
                decoded.charge(folded.len() as u64)?;
                drop(previous_fold.take());
                drop(previous_fold_hold.take());
                previous_fold = Some(folded);
                previous_fold_hold = fold_hold;
                let (value_json, _value_json_hold) =
                    scalar_json(value, limits.max_row_bytes, creation)?;
                let count = number(row, "count")?;
                if count == 0 {
                    return Err(Error::Invalid("zero catalog facet value"));
                }
                total = total
                    .checked_add(count)
                    .ok_or(Error::Budget("catalog facet total"))?;
                work.charge(&[
                    desc.as_bytes(),
                    domain.as_bytes(),
                    field.as_bytes(),
                    value_json.as_bytes(),
                ])?;
                db.execute(
                    "INSERT INTO catalog_facets VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        desc,
                        domain,
                        field,
                        signed(ordinal as u64)?,
                        value_json,
                        signed(count)?
                    ],
                )?;
                value_count = value_count
                    .checked_add(1)
                    .ok_or(Error::Budget("catalog facet values"))?;
            }
            work.charge(&[desc.as_bytes(), domain.as_bytes(), field.as_bytes()])?;
            db.execute(
                "INSERT INTO catalog_facet_fields VALUES(?1,?2,?3,?4,?5)",
                params![
                    desc,
                    domain,
                    field,
                    signed(values.len() as u64)?,
                    signed(total)?
                ],
            )?;
            field_count = field_count
                .checked_add(1)
                .ok_or(Error::Budget("catalog facet fields"))?;
            let expected_total = match (domain, field.as_str()) {
                ("node", "source_graph" | "kind_id" | "type_id") => Some(receipt.node_count),
                ("relation", "source_graph" | "predicate_id" | "relation_type_id") => {
                    Some(receipt.relation_count)
                }
                _ => None,
            };
            if expected_total.is_some_and(|expected| expected != total) {
                return Err(Error::Invalid("catalog required facet total"));
            }
        }
    }

    let routes = array(caps, "entity_routes")?;
    if routes.len() != vocabulary.overview_route_ids.len() {
        return Err(Error::Invalid("catalog route coverage"));
    }
    for (ordinal, (route, expected_id)) in routes
        .iter()
        .zip(&vocabulary.overview_route_ids)
        .enumerate()
    {
        let id = string(route, "route_id")?;
        if id != expected_id {
            return Err(Error::Invalid("catalog route order"));
        }
        let route_packet = encode_packet(route, limits.max_row_bytes, creation)?;
        let route_sha = Digest256::of_bytes(&route_packet);
        let node_count = number(route, "node_count")?;
        let confirming = number(route, "confirming_relation_count")?;
        let semantic = number(route, "semantic_confirming_relation_count")?;
        if node_count > receipt.node_count
            || confirming > receipt.relation_count
            || semantic > receipt.relation_count
        {
            return Err(Error::Invalid("catalog route count exceeds graph"));
        }
        let availability = string(route, "availability")?;
        let readiness = string(route, "role_readiness")?;
        if !matches!(availability, "available" | "not_projected")
            || !matches!(readiness, "not_projected" | "kind_only" | "confirmed")
        {
            return Err(Error::Invalid("catalog route state"));
        }
        work.charge(&[
            desc.as_bytes(),
            id.as_bytes(),
            availability.as_bytes(),
            readiness.as_bytes(),
            &route_packet.bytes,
        ])?;
        db.execute(
            "INSERT INTO catalog_routes VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                desc,
                id,
                signed(ordinal as u64)?,
                signed(node_count)?,
                signed(confirming)?,
                signed(semantic)?,
                availability,
                readiness,
                signed(route_packet.len() as u64)?,
                route_sha.as_bytes().as_slice(),
                route_packet.bytes.as_slice()
            ],
        )?;
        verify_route_packet(db, desc, id, limits.max_row_bytes, &route_sha, &mut decoded)?;
    }

    let scope_count: u64 = db.query_row("SELECT count(*) FROM source_scope", [], |r| r.get(0))?;
    if scope_count != vocabulary.sources.len() as u64 {
        return Err(Error::Invalid("catalog source scope coverage"));
    }
    let mut source_nodes = 0u64;
    let mut source_relations = 0u64;
    for source in &vocabulary.sources {
        let id = &source.source_graph_id;
        let scoped: Option<(u64,u64)> = db.query_row(
            "SELECT expected_node_count,expected_relation_count FROM source_scope WHERE source_graph=?1",
            [id],|r| Ok((r.get(0)?,r.get(1)?))).optional()?;
        let (scoped_nodes, scoped_relations) = scoped.ok_or(Error::Invalid(
            "catalog registered source absent from scope",
        ))?;
        let nodes: u64 = db.query_row(
            "SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1",
            [id],
            |r| r.get(0),
        )?;
        let relations: u64 = db.query_row(
            "SELECT count(*) FROM knowledge_relations WHERE source_graph=?1",
            [id],
            |r| r.get(0),
        )?;
        if (nodes, relations) != (scoped_nodes, scoped_relations) {
            return Err(Error::Invalid("catalog source scope count mismatch"));
        }
        for (domain, count) in [("node", nodes), ("relation", relations)] {
            let (value_json, _value_json_hold) = scalar_json(id, limits.max_row_bytes, creation)?;
            let facet: Option<u64> = db.query_row(
                "SELECT item_count FROM catalog_facets WHERE descriptor_sha256=?1 AND domain=?2 AND field_id='source_graph' AND value_json=?3",
                params![desc,domain,value_json],|r|r.get(0)).optional()?;
            if facet.unwrap_or(0) != count {
                return Err(Error::Invalid("catalog source facet count mismatch"));
            }
        }
        work.charge(&[desc.as_bytes(), id.as_bytes()])?;
        db.execute(
            "INSERT INTO catalog_source_counts VALUES(?1,?2,?3,?4)",
            params![desc, id, signed(nodes)?, signed(relations)?],
        )?;
        source_nodes = source_nodes
            .checked_add(nodes)
            .ok_or(Error::Budget("catalog source nodes"))?;
        source_relations = source_relations
            .checked_add(relations)
            .ok_or(Error::Budget("catalog source relations"))?;
    }
    if (source_nodes, source_relations) != (receipt.node_count, receipt.relation_count) {
        return Err(Error::Invalid("catalog normalized row count"));
    }
    let index_root = index_root(
        db,
        desc,
        receipt,
        field_count,
        value_count,
        routes.len() as u64,
        vocabulary.sources.len() as u64,
        packet_sha,
        &mut decoded,
    )?;
    work.charge(&[
        desc.as_bytes(),
        receipt.sha256.as_bytes(),
        index_root.as_bytes(),
    ])?;
    db.execute(
        "INSERT INTO catalog_index_meta VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            desc,
            receipt.sha256,
            SCHEMA,
            ORDER_PROFILE,
            signed(vocabulary.sources.len() as u64)?,
            signed(field_count)?,
            signed(value_count)?,
            signed(routes.len() as u64)?,
            index_root,
            signed(packet.len() as u64)?,
            packet_sha.as_bytes().as_slice(),
            packet
        ],
    )?;
    // A consumer must make the same pre-BLOB length check before transfer.
    let returned = read_packet_owned(db, desc, limits.max_packet_bytes, &mut decoded)?;
    if returned.bytes != packet {
        return Err(Error::Invalid("catalog index packet readback"));
    }
    if let Some(creation) = creation {
        let output_bytes = std::mem::size_of::<CatalogIndexReceipt>()
            .checked_add(desc.len())
            .and_then(|n| n.checked_add(receipt.sha256.len()))
            .and_then(|n| n.checked_add(index_root.len()))
            .ok_or(Error::Budget("catalog index receipt allocation"))?;
        creation.retain(output_bytes)?;
        creation.charge_work(output_bytes)?;
    }
    Ok(CatalogIndexReceipt {
        descriptor_sha256: desc.clone(),
        catalog_packet_sha256: receipt.sha256.clone(),
        catalog_index_root_sha256: index_root,
        source_count: vocabulary.sources.len() as u64,
        facet_field_count: field_count,
        facet_value_count: value_count,
        route_count: routes.len() as u64,
        packet_bytes: packet.len() as u64,
    })
}

fn read_packet_owned<'state, 'budget>(
    db: &Connection,
    descriptor: &str,
    max_bytes: usize,
    decoded: &mut DecodeBudget<'state, 'budget>,
) -> Result<EncodedPacket<'state, 'budget>> {
    let (actual,declared,digest_len): (i64,i64,i64) = db.query_row(
        "SELECT length(packet),packet_len,length(packet_sha256) FROM catalog_index_meta WHERE descriptor_sha256=?1",
        [descriptor],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    if actual < 0 || declared != actual || actual as u64 > max_bytes as u64 || digest_len != 32 {
        return Err(Error::Budget("catalog indexed packet length"));
    }
    let owned = usize::try_from(actual)
        .ok()
        .and_then(|n| n.checked_add(32))
        .ok_or(Error::Budget("catalog index decoded bytes"))?;
    decoded.charge(owned as u64)?;
    let hold = decoded.hold(owned)?;
    let digest: Vec<u8> = db.query_row(
        "SELECT packet_sha256 FROM catalog_index_meta WHERE descriptor_sha256=?1",
        [descriptor],
        |r| r.get(0),
    )?;
    let bytes: Vec<u8> = db.query_row(
        "SELECT packet FROM catalog_index_meta WHERE descriptor_sha256=?1",
        [descriptor],
        |r| r.get(0),
    )?;
    if bytes.len() != actual as usize
        || Digest256::of_bytes(&bytes).as_bytes().as_slice() != digest.as_slice()
    {
        return Err(Error::Invalid("catalog indexed packet digest"));
    }
    Ok(EncodedPacket { bytes, _hold: hold })
}

fn read_packet(
    db: &Connection,
    descriptor: &str,
    max_bytes: usize,
    decoded: &mut DecodeBudget<'_, '_>,
) -> Result<Vec<u8>> {
    Ok(read_packet_owned(db, descriptor, max_bytes, decoded)?.bytes)
}

fn verify_route_packet(
    db: &Connection,
    descriptor: &str,
    route_id: &str,
    max_bytes: usize,
    expected: &Digest256,
    decoded: &mut DecodeBudget<'_, '_>,
) -> Result<()> {
    let (actual, declared, digest_len): (i64, i64, i64) = db.query_row(
        "SELECT length(packet),packet_len,length(packet_sha256) FROM catalog_routes
         WHERE descriptor_sha256=?1 AND route_id=?2",
        params![descriptor, route_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if actual < 0 || declared != actual || actual as u64 > max_bytes as u64 || digest_len != 32 {
        return Err(Error::Budget("catalog indexed route length"));
    }
    let owned = usize::try_from(actual)
        .ok()
        .and_then(|n| n.checked_add(32))
        .ok_or(Error::Budget("catalog index decoded bytes"))?;
    decoded.charge(owned as u64)?;
    let _hold = decoded.hold(owned)?;
    let (digest, bytes): (Vec<u8>, Vec<u8>) = db.query_row(
        "SELECT packet_sha256,packet FROM catalog_routes
         WHERE descriptor_sha256=?1 AND route_id=?2",
        params![descriptor, route_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if bytes.len() != actual as usize
        || digest.as_slice() != expected.as_bytes()
        || Digest256::of_bytes(&bytes) != *expected
    {
        return Err(Error::Invalid("catalog indexed route digest"));
    }
    Ok(())
}

fn index_root(
    db: &Connection,
    desc: &str,
    receipt: &CatalogReceipt,
    fields: u64,
    values: u64,
    routes: u64,
    sources: u64,
    packet_sha: &Digest256,
    decoded: &mut DecodeBudget<'_, '_>,
) -> Result<String> {
    let mut hash = Digest256Hasher::new();
    hash_text(&mut hash, SCHEMA);
    hash_text(&mut hash, ORDER_PROFILE);
    hash_text(&mut hash, desc);
    hash_text(&mut hash, &receipt.sha256);
    hash.update(packet_sha.as_bytes());
    for n in [
        receipt.node_count,
        receipt.relation_count,
        fields,
        values,
        routes,
        sources,
    ] {
        hash_number(&mut hash, n);
    }
    let mut seen = [0u64; 4];
    let mut stmt = db.prepare(
        "SELECT domain,field_id,value_count,total_count,length(CAST(domain AS BLOB)),length(CAST(field_id AS BLOB)) FROM catalog_facet_fields
        WHERE descriptor_sha256=?1 ORDER BY domain,field_id",
    )?;
    let mut rows = stmt.query([desc])?;
    while let Some(row) = rows.next()? {
        let domain_len: i64 = row.get(4)?;
        let field_len: i64 = row.get(5)?;
        if domain_len < 0 || field_len < 0 {
            return Err(Error::Invalid("catalog index root row length"));
        }
        let owned = usize::try_from(domain_len)
            .ok()
            .and_then(|n| n.checked_add(usize::try_from(field_len).ok()?))
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, String, u64, u64)>() + 32))
            .ok_or(Error::Budget("catalog index decoded bytes"))?;
        let _hold = decoded.hold(owned)?;
        decoded.charge(owned as u64)?;
        let domain: String = row.get(0)?;
        let field: String = row.get(1)?;
        hash.update(b"F");
        hash_text(&mut hash, &domain);
        hash_text(&mut hash, &field);
        for index in 2..4 {
            hash_number(&mut hash, row.get::<_, u64>(index)?);
        }
        seen[0] += 1;
    }
    let mut stmt = db.prepare(
        "SELECT domain,field_id,ordinal,value_json,item_count,length(CAST(domain AS BLOB)),length(CAST(field_id AS BLOB)),length(CAST(value_json AS BLOB)) FROM catalog_facets
        WHERE descriptor_sha256=?1 ORDER BY domain,field_id,ordinal",
    )?;
    let mut rows = stmt.query([desc])?;
    while let Some(row) = rows.next()? {
        let domain_len: i64 = row.get(5)?;
        let field_len: i64 = row.get(6)?;
        let value_len: i64 = row.get(7)?;
        if domain_len < 0 || field_len < 0 || value_len < 0 {
            return Err(Error::Invalid("catalog index root row length"));
        }
        let owned = [domain_len, field_len, value_len]
            .into_iter()
            .try_fold(
                std::mem::size_of::<(String, String, String, u64, u64)>() + 64,
                |sum, len| sum.checked_add(usize::try_from(len).ok()?),
            )
            .ok_or(Error::Budget("catalog index decoded bytes"))?;
        let _hold = decoded.hold(owned)?;
        decoded.charge(owned as u64)?;
        let domain: String = row.get(0)?;
        let field: String = row.get(1)?;
        let value_json: String = row.get(3)?;
        hash.update(b"V");
        hash_text(&mut hash, &domain);
        hash_text(&mut hash, &field);
        hash_number(&mut hash, row.get::<_, u64>(2)?);
        hash_text(&mut hash, &value_json);
        hash_number(&mut hash, row.get::<_, u64>(4)?);
        seen[1] += 1;
    }
    let mut stmt = db.prepare(
        "SELECT route_id,ordinal,node_count,confirming_relation_count,
        semantic_confirming_relation_count,availability,role_readiness,packet_len,
        length(packet_sha256),packet_sha256,length(CAST(route_id AS BLOB)),length(CAST(availability AS BLOB)),length(CAST(role_readiness AS BLOB))
        FROM catalog_routes WHERE descriptor_sha256=?1 ORDER BY ordinal",
    )?;
    let mut rows = stmt.query([desc])?;
    while let Some(row) = rows.next()? {
        let route_len: i64 = row.get(10)?;
        let availability_len: i64 = row.get(11)?;
        let readiness_len: i64 = row.get(12)?;
        if route_len < 0 || availability_len < 0 || readiness_len < 0 {
            return Err(Error::Invalid("catalog route row length"));
        }
        let owned = [
            route_len,
            availability_len,
            readiness_len,
            row.get::<_, i64>(8)?,
        ]
        .into_iter()
        .try_fold(
            std::mem::size_of::<(
                String,
                u64,
                u64,
                u64,
                u64,
                String,
                String,
                u64,
                i64,
                Vec<u8>,
            )>() + 64,
            |sum, len| sum.checked_add(usize::try_from(len).ok()?),
        )
        .ok_or(Error::Budget("catalog index decoded bytes"))?;
        let _hold = decoded.hold(owned)?;
        decoded.charge(owned as u64)?;
        let route_id: String = row.get(0)?;
        let availability: String = row.get(5)?;
        let readiness: String = row.get(6)?;
        if row.get::<_, i64>(8)? != 32 {
            return Err(Error::Invalid("catalog route digest length"));
        }
        hash.update(b"R");
        hash_text(&mut hash, &route_id);
        for index in 1..5 {
            hash_number(&mut hash, row.get::<_, u64>(index)?);
        }
        hash_text(&mut hash, &availability);
        hash_text(&mut hash, &readiness);
        hash_number(&mut hash, row.get::<_, u64>(7)?);
        let digest: Vec<u8> = row.get(9)?;
        if digest.len() != 32 {
            return Err(Error::Invalid("catalog route digest length"));
        }
        hash.update(&digest);
        seen[2] += 1;
    }
    let mut stmt = db.prepare(
        "SELECT source_graph_id,node_count,relation_count,length(CAST(source_graph_id AS BLOB)) FROM catalog_source_counts
        WHERE descriptor_sha256=?1 ORDER BY source_graph_id",
    )?;
    let mut rows = stmt.query([desc])?;
    while let Some(row) = rows.next()? {
        let source_len: i64 = row.get(3)?;
        if source_len < 0 {
            return Err(Error::Invalid("catalog source row length"));
        }
        let owned = usize::try_from(source_len)
            .ok()
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, u64, u64)>() + 32))
            .ok_or(Error::Budget("catalog index decoded bytes"))?;
        let _hold = decoded.hold(owned)?;
        decoded.charge(owned as u64)?;
        let source: String = row.get(0)?;
        hash.update(b"S");
        hash_text(&mut hash, &source);
        hash_number(&mut hash, row.get::<_, u64>(1)?);
        hash_number(&mut hash, row.get::<_, u64>(2)?);
        seen[3] += 1;
    }
    if seen != [fields, values, routes, sources] {
        return Err(Error::Invalid("catalog index row count/root"));
    }
    decoded.charge(64)?;
    if let Some(creation) = decoded.creation {
        creation.retain(64)?;
    }
    Ok(hash.finalize().to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, StageIsolation, StageLimits, StageOwner,
    };
    use crate::{Limits, SourceBinding};
    use serde_json::json;
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    const VOCAB: &[u8] = include_bytes!("../tests/fixtures/query-vocabulary.v1.json");
    const ADAPTERS: &[&str] = &[
        "philosophy-node-edge-v1",
        "canon-node-relation-v1",
        "candidate-relation-v1",
        "source-navigation-node-edge-v1",
        "reified-bibliographic-claims-v1",
        "declared-identity-and-source-ref-joins-v1",
        "repository-topology-v1",
        "indexed-node-edge-v1",
    ];

    fn fixture(scope_mismatch: bool) -> (Connection, CatalogReceipt, QueryVocabulary, Vec<u8>) {
        let vocab = QueryVocabulary::parse(VOCAB, ADAPTERS).unwrap();
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE source_scope(
            source_graph TEXT PRIMARY KEY,expected_node_count INTEGER NOT NULL,
            expected_relation_count INTEGER NOT NULL) WITHOUT ROWID;
            CREATE TABLE knowledge_nodes(source_graph TEXT NOT NULL);
            CREATE TABLE knowledge_relations(source_graph TEXT NOT NULL);",
        )
        .unwrap();
        db.execute("INSERT INTO knowledge_nodes VALUES('canon')", [])
            .unwrap();
        for source in &vocab.sources {
            let nodes = if source.source_graph_id == "canon" {
                if scope_mismatch { 2 } else { 1 }
            } else {
                0
            };
            db.execute(
                "INSERT INTO source_scope VALUES(?1,?2,0)",
                params![source.source_graph_id, nodes],
            )
            .unwrap();
        }
        let routes: Vec<Value> = vocab
            .overview_route_ids
            .iter()
            .map(|id| {
                json!({
            "route_id":id,"node_count":0,"confirming_relation_count":0,
            "semantic_confirming_relation_count":0,"availability":"not_projected",
            "role_readiness":"not_projected"})
            })
            .collect();
        let catalog = json!({"schema":"tos_knowledge_catalog_v1",
            "counts":{"nodes":1,"relations":0},"capabilities":{"facets":{
            "nodes":{
                "source_graph":[{"value":"canon","count":1}],
                "kind_id":[{"value":"work","count":1}],
                "type_id":[{"value":"tos.entity.work","count":1}]},
            "relations":{"source_graph":[],"predicate_id":[],"relation_type_id":[]}},
            "entity_routes":routes}});
        let packet = serde_json::to_vec(&catalog).unwrap();
        let receipt = CatalogReceipt {
            catalog,
            sha256: Digest256::of_bytes(&packet).to_hex(),
            node_count: 1,
            relation_count: 0,
        };
        (db, receipt, vocab, packet)
    }

    #[test]
    fn complete_zero_sources_fields_routes_and_exact_seek_have_stable_root() {
        let mut roots = Vec::new();
        for _ in 0..2 {
            let (mut db, receipt, vocab, packet) = fixture(false);
            let digest = Digest256::of_bytes(&packet);
            let index = materialize_inner(
                &mut db,
                &receipt,
                &vocab,
                CatalogIndexLimits::default(),
                &packet,
                &digest,
                None,
            )
            .unwrap();
            assert_eq!(index.source_count, vocab.sources.len() as u64);
            assert_eq!(index.facet_field_count, 6);
            assert_eq!(index.facet_value_count, 3);
            assert_eq!(index.route_count, vocab.overview_route_ids.len() as u64);
            let zeros: u64 = db
                .query_row(
                    "SELECT count(*) FROM catalog_source_counts
                WHERE node_count=0 AND relation_count=0",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(zeros, vocab.sources.len() as u64 - 1);
            let empty_field: u64 = db
                .query_row(
                    "SELECT value_count FROM catalog_facet_fields
                WHERE domain='relation' AND field_id='predicate_id'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(empty_field, 0);
            let exact: u64 = db
                .query_row(
                    "SELECT item_count FROM catalog_facets
                WHERE domain='node' AND field_id='source_graph' AND value_json='\"canon\"'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exact, 1);
            assert_eq!(
                read_packet(
                    &db,
                    &vocab.descriptor_sha256,
                    packet.len(),
                    &mut DecodeBudget::new(u64::MAX, None)
                )
                .unwrap(),
                packet
            );
            roots.push(index.catalog_index_root_sha256);
        }
        assert_eq!(roots[0], roots[1]);
    }

    #[test]
    fn scope_count_and_packet_length_mismatch_refuse() {
        let (mut db, receipt, vocab, packet) = fixture(true);
        let digest = Digest256::of_bytes(&packet);
        let error = materialize_inner(
            &mut db,
            &receipt,
            &vocab,
            CatalogIndexLimits::default(),
            &packet,
            &digest,
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("scope count mismatch"));

        let (mut db, receipt, vocab, packet) = fixture(false);
        let digest = Digest256::of_bytes(&packet);
        materialize_inner(
            &mut db,
            &receipt,
            &vocab,
            CatalogIndexLimits::default(),
            &packet,
            &digest,
            None,
        )
        .unwrap();
        db.execute("UPDATE catalog_index_meta SET packet=zeroblob(200000)", [])
            .unwrap();
        let error = read_packet(
            &db,
            &vocab.descriptor_sha256,
            packet.len(),
            &mut DecodeBudget::new(u64::MAX, None),
        )
        .unwrap_err();
        assert!(error.to_string().contains("packet length"));
    }

    #[test]
    fn undersized_decoded_budget_refuses_during_casefold_before_blob_transfer() {
        let (mut db, receipt, vocab, packet) = fixture(false);
        let digest = Digest256::of_bytes(&packet);
        let limits = CatalogIndexLimits {
            max_decoded_bytes: 1,
            ..CatalogIndexLimits::default()
        };
        let error = materialize_inner(&mut db, &receipt, &vocab, limits, &packet, &digest, None)
            .unwrap_err();
        assert!(error.to_string().contains("catalog facet casefold bytes"));
    }

    #[test]
    fn unicode_facet_preserves_indexed_original_value() {
        let (mut db, mut receipt, vocab, _) = fixture(false);
        receipt.catalog["capabilities"]["facets"]["nodes"]["kind_id"][0]["value"] = json!("Straße");
        let packet = serde_json::to_vec(&receipt.catalog).unwrap();
        receipt.sha256 = Digest256::of_bytes(&packet).to_hex();
        let digest = Digest256::of_bytes(&packet);
        materialize_inner(
            &mut db,
            &receipt,
            &vocab,
            CatalogIndexLimits::default(),
            &packet,
            &digest,
            None,
        )
        .unwrap();
        let stored:String=db.query_row("SELECT value_json FROM catalog_facets WHERE domain='node' AND field_id='kind_id' AND ordinal=0",[],|r|r.get(0)).unwrap();
        assert_eq!(serde_json::from_str::<String>(&stored).unwrap(), "Straße");
    }

    #[test]
    fn self_hashed_wrong_catalog_schema_refuses() {
        let (mut db, mut receipt, vocab, _) = fixture(false);
        receipt.catalog["schema"] = json!("other_catalog");
        let packet = serde_json::to_vec(&receipt.catalog).unwrap();
        receipt.sha256 = Digest256::of_bytes(&packet).to_hex();
        let digest = Digest256::of_bytes(&packet);
        let error = materialize_inner(
            &mut db,
            &receipt,
            &vocab,
            CatalogIndexLimits::default(),
            &packet,
            &digest,
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("packet schema"));
    }

    #[test]
    fn zero_count_unicode_registered_source_retains_exact_codepoints() {
        let (mut db, receipt, mut vocab, packet) = fixture(false);
        let previous = vocab
            .sources
            .iter()
            .find(|source| source.source_graph_id != "canon")
            .unwrap()
            .source_graph_id
            .clone();
        vocab
            .sources
            .iter_mut()
            .find(|source| source.source_graph_id == previous)
            .unwrap()
            .source_graph_id = "Café".into();
        db.execute(
            "UPDATE source_scope SET source_graph='Café' WHERE source_graph=?1",
            [previous],
        )
        .unwrap();
        let digest = Digest256::of_bytes(&packet);
        materialize_inner(
            &mut db,
            &receipt,
            &vocab,
            CatalogIndexLimits::default(),
            &packet,
            &digest,
            None,
        )
        .unwrap();
        let count: u64 = db
            .query_row(
                "SELECT node_count FROM catalog_source_counts WHERE source_graph_id='Café'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        let order: Vec<String> = db
            .prepare("SELECT source_graph_id FROM catalog_source_counts ORDER BY source_graph_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(order.first().unwrap(), "Café");
    }

    struct TestOwner;
    impl StageOwner for TestOwner {
        fn verify_receipt(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
        fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
    }
    struct TestQuota;
    impl StageIsolation for TestQuota {
        fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn callback_failure_poison_prevents_stage_finish() {
        let (_, receipt, vocab, _) = fixture(false);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("tos-catalog-index-{}-{unique}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let candidate = dir.join("stage.sqlite3");
        let owner = TestOwner;
        let quota = TestQuota;
        let exact = ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "fixture-cut".into(),
                through_commit_seq: 1,
                membership_root: "0".repeat(64),
                index_generation: "fixture-generation".into(),
                route_map_version: "fixture-routes".into(),
                reader_abi: "fixture-reader".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: vec![InputCollectionReceipt {
                source_graph: "fixture.graph".into(),
                collection: "fixture/raw".into(),
                input_role: "fixture".into(),
                adapter_profile: "fixture-adapter".into(),
                expected_count: 0,
                expected_root_sha256: Digest256::of_bytes(b"").to_hex(),
            }],
        };
        let limits = StageLimits {
            sqlite: Limits::default(),
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 8,
            max_seek_bytes: 1024,
        };
        let mut stage = KnowledgeStage::create(&candidate, limits, exact, &owner, &quota).unwrap();
        assert!(
            materialize_catalog(&mut stage, &receipt, &vocab, CatalogIndexLimits::default())
                .is_err()
        );
        assert!(stage.finish().unwrap_err().to_string().contains("poisoned"));
        fs::remove_dir(&dir).unwrap();
    }
}
