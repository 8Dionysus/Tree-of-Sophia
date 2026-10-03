//! Coverage consumes an actual compiler catalogue, never a mock profile oracle.
//! The caller keeps its captured cut, schema worker and stage isolation alive
//! through this callback and rechecks the cut/publication before disclosure.

use serde_json::Value;
use std::{collections::BTreeMap, time::Instant};
use tos_compiler::knowledge_stage::KnowledgeStage;
use tos_compiler::source_witness_catalog::{
    CATALOG_SOURCE, ColdSourceCatalogReceipt, SOURCE_FILES, SourceCatalogLimits, SourceCatalogSink,
    render_cold_source_witness_catalog,
};
use tos_compiler::{Error, Result};
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

/// Additional coverage state/work, separate from the already admitted kernel.
#[derive(Clone, Copy)]
pub struct CaptureObservationLimits {
    pub max_addressed_rows: u64,
    pub max_addressed_bytes: usize,
    pub max_source_read_bytes: u64,
    pub max_state_bytes: usize,
    pub deadline: Instant,
}

/// Exact source bytes selected by the genuine catalogue. This borrowed value
/// cannot outlive the operation callback and carries no admission authority.
pub struct CatalogueSourceRow<'a> {
    pub kind: &'a str,
    pub identity: &'a str,
    pub source_ref: &'a str,
    pub source_line: Option<u64>,
    pub source_file_sha256: Digest256,
    pub source_record: Value,
    pub catalogue_entry: &'a Value,
}

struct AddressedSink {
    rows: Vec<Value>,
    bytes: usize,
    limits: CaptureObservationLimits,
    row_cap: usize,
}

fn decode(raw: &[u8], cap: usize) -> Result<Value> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4300)
        .map_err(|_| Error::Budget("coverage source JSON"))?;
    let doc = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let preserved = tos_foundation::emit_value_preserved_json(doc.root(), limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(&preserved).map_err(|e| Error::Source(e.to_string()))
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("coverage catalogue field"))
}

fn reserve_state(
    addressed: usize,
    rows: usize,
    transient: usize,
    limits: CaptureObservationLimits,
) -> Result<()> {
    addressed
        .checked_add(transient)
        .and_then(|n| n.checked_mul(128))
        .and_then(|n| rows.checked_mul(1024).and_then(|r| n.checked_add(r)))
        .filter(|n| *n <= limits.max_state_bytes)
        .ok_or(Error::Budget("coverage retained/temporary JSON state"))?;
    Ok(())
}

impl SourceCatalogSink for AddressedSink {
    fn begin_file(&mut self, _: &str) -> Result<()> {
        Ok(())
    }
    fn file_bytes(&mut self, _: &[u8]) -> Result<()> {
        Ok(())
    }
    fn end_file(&mut self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
    fn manifest(&mut self, _: &Value) -> Result<()> {
        Ok(())
    }
    fn addressed_row(&mut self, collection: &str, raw: &[u8]) -> Result<()> {
        if Instant::now() >= self.limits.deadline {
            return Err(Error::Budget("coverage original deadline"));
        }
        if !matches!(collection, "records" | "claims" | "slots") {
            return Err(Error::Invalid("coverage catalogue collection"));
        }
        if self.rows.len() as u64 >= self.limits.max_addressed_rows {
            return Err(Error::Budget("coverage addressed rows"));
        }
        self.bytes = self
            .bytes
            .checked_add(raw.len())
            .filter(|n| *n <= self.limits.max_addressed_bytes)
            .ok_or(Error::Budget("coverage addressed bytes"))?;
        // A conservative source-byte envelope covers both strict FND and serde
        // DOMs, output transport, vector slots and map/group indexes. The
        // callback must reserve its own retained copies separately.
        reserve_state(self.bytes, self.rows.len() + 1, raw.len(), self.limits)?;
        let mut row = decode(raw, self.row_cap)?;
        row["coverage_collection"] = Value::String(collection.to_owned());
        self.rows.push(row);
        Ok(())
    }
}

/// Render authentic, sealed catalogue rows and resolve their source bytes from
/// the same stage. The compiler verifies its receipt/input roots before render;
/// the caller must perform its original final cut/root/epoch guard after return
/// and before emitting the terminal complete summary. No schema is recompiled.
pub fn observe_owned_catalogue(
    stage: &mut KnowledgeStage<'_>,
    receipt: &ColdSourceCatalogReceipt,
    catalogue_limits: SourceCatalogLimits,
    limits: CaptureObservationLimits,
    mut observe: impl FnMut(CatalogueSourceRow<'_>) -> Result<()>,
) -> Result<()> {
    if limits.max_addressed_rows == 0
        || limits.max_addressed_bytes == 0
        || limits.max_source_read_bytes == 0
        || limits.max_state_bytes == 0
        || Instant::now() >= limits.deadline
    {
        return Err(Error::Budget("coverage capture envelope"));
    }
    let mut sink = AddressedSink {
        rows: Vec::new(),
        bytes: 0,
        limits,
        row_cap: catalogue_limits.max_output_row_bytes,
    };
    // Only successful complete rendering makes the collected addresses usable.
    render_cold_source_witness_catalog(stage, receipt, catalogue_limits, &mut sink)?;
    let mut slots = BTreeMap::new();
    for row in sink
        .rows
        .iter()
        .filter(|row| row["coverage_collection"] == "slots")
    {
        if slots.insert(text(row, "source_slot_key")?, row).is_some() {
            return Err(Error::Invalid("coverage duplicate source slot"));
        }
    }
    if slots.len() as u64 != receipt.source_slot_count {
        return Err(Error::Invalid("coverage complete source slot count"));
    }
    let mut source_read_bytes = 0u64;
    let mut records = 0u64;
    let mut claims = 0u64;
    let mut groups = BTreeMap::<&str, Vec<(&str, &str, &Value, Option<u64>, &Value)>>::new();
    for row in &sink.rows {
        if Instant::now() >= limits.deadline {
            return Err(Error::Budget("coverage original deadline"));
        }
        let collection = text(row, "coverage_collection")?;
        if collection == "slots" {
            continue;
        }
        let entry = &row["entry"];
        let (kind, identity, source, line) = if collection == "records" {
            records = records
                .checked_add(1)
                .ok_or(Error::Budget("coverage record count"))?;
            (
                text(entry, "record_type")?,
                text(row, "record_id")?,
                &row["source"],
                None,
            )
        } else {
            claims = claims
                .checked_add(1)
                .ok_or(Error::Budget("coverage Claim count"))?;
            let slot = slots
                .get(text(row, "source_slot_key")?)
                .ok_or(Error::Invalid("coverage Claim source slot absent"))?;
            let line = slot["source"]["source_line"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or(Error::Invalid("coverage Claim physical line"))?;
            ("claim", text(row, "claim_id")?, &slot["source"], Some(line))
        };
        let reference = text(source, "source_ref")?;
        groups
            .entry(reference)
            .or_default()
            .push((kind, identity, source, line, entry));
    }
    if records != receipt.record_count || claims != receipt.claim_count {
        return Err(Error::Invalid("coverage complete catalogue count"));
    }
    for (reference, members) in groups {
        let first = members
            .first()
            .ok_or(Error::Invalid("coverage empty source group"))?;
        let file_bytes = first.2[if first.3.is_some() {
            "file_bytes"
        } else {
            "raw_bytes"
        }]
        .as_u64()
        .ok_or(Error::Invalid("coverage source byte count"))?;
        if file_bytes > catalogue_limits.max_file_bytes as u64 {
            return Err(Error::Budget("coverage source file bytes"));
        }
        source_read_bytes = source_read_bytes
            .checked_add(file_bytes)
            .filter(|n| *n <= limits.max_source_read_bytes)
            .ok_or(Error::Budget("coverage cumulative source reads"))?;
        // Preflight before raw_by_id allocates the complete source file.
        reserve_state(
            sink.bytes,
            sink.rows.len(),
            usize::try_from(file_bytes).map_err(|_| Error::Budget("coverage source byte range"))?,
            limits,
        )?;
        let raw = stage
            .raw_by_id(CATALOG_SOURCE, SOURCE_FILES, reference)?
            .ok_or(Error::Invalid("coverage catalogue source absent"))?;
        if raw.payload.len() as u64 != file_bytes {
            return Err(Error::Invalid("coverage source byte count changed"));
        }
        let file_digest = Digest256::of_bytes(&raw.payload);
        for (kind, identity, source, line, entry) in members {
            if Instant::now() >= limits.deadline {
                return Err(Error::Budget("coverage original deadline"));
            }
            if source[if line.is_some() {
                "file_bytes"
            } else {
                "raw_bytes"
            }]
            .as_u64()
                != Some(file_bytes)
            {
                return Err(Error::Invalid("coverage grouped source bytes"));
            }
            let expected = text(
                source,
                if line.is_some() {
                    "file_sha256"
                } else {
                    "raw_sha256"
                },
            )?;
            if file_digest.to_hex() != expected {
                return Err(Error::Invalid("coverage exact source file digest"));
            }
            let record_raw = if line.is_some() {
                let offset = source["byte_offset"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or(Error::Invalid("coverage Claim offset"))?;
                let bytes = source["row_bytes"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or(Error::Invalid("coverage Claim bytes"))?;
                let end = offset
                    .checked_add(bytes)
                    .ok_or(Error::Budget("coverage Claim range"))?;
                let selected = raw
                    .payload
                    .get(offset..end)
                    .ok_or(Error::Invalid("coverage Claim source range"))?;
                if Digest256::of_bytes(selected).to_hex() != text(source, "raw_row_sha256")? {
                    return Err(Error::Invalid("coverage exact Claim row digest"));
                }
                selected
            } else {
                &raw.payload
            };
            observe(CatalogueSourceRow {
                kind,
                identity,
                source_ref: reference,
                source_line: line,
                source_file_sha256: file_digest,
                source_record: decode(record_raw, catalogue_limits.max_file_bytes)?,
                catalogue_entry: entry,
            })?;
        }
    }
    Ok(())
}
