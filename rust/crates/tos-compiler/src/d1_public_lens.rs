//! Exact optional-store rows over each emitted public normalized carrier.
//! The source JSON and its digest remain in the base D1 row/meta lane.

use crate::{
    Error, Result,
    d1_public_capture::{MAX_ROW_BYTES, PublicCapture, compact},
    d1_public_rows::{lower_search, quoted},
    d1_public_sql::SqlSink,
};
use std::collections::BTreeSet;
use tos_foundation::{Digest256, JsonValue};

const MAX_SEED_BYTES: usize = 1_048_576;
const MAX_MEMBERS: usize = 256;

fn compact_seed(item: &JsonValue) -> Result<JsonValue> {
    let JsonValue::Object(fields) = item else {
        return Err(Error::Invalid("public compact lens row"));
    };
    let attributes = item
        .object_get("attributes")
        .and_then(JsonValue::as_object)
        .ok_or(Error::Invalid("public compact lens attributes"))?;
    let keep_forms = attributes
        .iter()
        .any(|(key, _)| key.as_str() == Some("human_forms"));
    let mut out = Vec::with_capacity(fields.len());
    for (key, value) in fields {
        match key.as_str() {
            Some("source_record" | "readable_context") => continue,
            Some("attributes") if !keep_forms => {
                out.push((key.clone(), JsonValue::Object(Vec::new())))
            }
            Some("semantics") => {
                let mut semantics = value.clone();
                if let JsonValue::Object(ref mut sem) = semantics {
                    for (name, claim) in sem {
                        if name.as_str() == Some("claim") {
                            if let JsonValue::Object(members) = claim {
                                members.retain(|(name, _)| {
                                    name.as_str() != Some("source_canonical_json")
                                });
                            }
                        }
                    }
                }
                out.push((key.clone(), semantics));
            }
            _ => out.push((key.clone(), value.clone())),
        }
    }
    Ok(JsonValue::Object(out))
}

fn member_values(item: &JsonValue, field: &str) -> Result<BTreeSet<String>> {
    let values = item
        .object_get(field)
        .and_then(JsonValue::as_array)
        .ok_or(Error::Invalid("public lens membership array"))?;
    if values.len() > MAX_MEMBERS {
        return Err(Error::Budget("public lens membership count"));
    }
    let mut result = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .filter(|value| value.len() <= 4096)
            .ok_or(Error::Invalid("public lens membership value"))?;
        result.insert(value.to_owned());
    }
    Ok(result)
}

#[derive(Default)]
pub(crate) struct LensCounts {
    pub compact_rows: u64,
    pub membership_rows: u64,
    pub auxiliary_bytes: u64,
}

pub(crate) fn emit_lens_auxiliary(
    capture: &PublicCapture,
    sink: &mut SqlSink,
    kind: &str,
    id: &str,
    source_json: &str,
    item: &JsonValue,
    max_bytes: u64,
    max_memberships: u64,
    counts: &mut LensCounts,
) -> Result<()> {
    if source_json.len() > MAX_ROW_BYTES
        || id.is_empty()
        || id.len() > 4096
        || item.object_get("id").and_then(JsonValue::as_str) != Some(id)
    {
        return Err(Error::Invalid("public lens source identity/bytes"));
    }
    let seed = compact_seed(item)?;
    let seed = String::from_utf8(compact(&seed, MAX_SEED_BYTES)?)
        .map_err(|_| Error::Invalid("public compact lens UTF-8"))?;
    let source_sha = Digest256::of_bytes(source_json.as_bytes()).to_hex();
    let seed_sha = Digest256::of_bytes(seed.as_bytes()).to_hex();
    let row_json = serde_json::to_vec(&(kind, id, &source_sha, &seed_sha, &seed))
        .map_err(|_| Error::Invalid("public compact lens row JSON"))?;
    counts.auxiliary_bytes = counts
        .auxiliary_bytes
        .checked_add(row_json.len() as u64)
        .filter(|n| *n <= max_bytes)
        .ok_or(Error::Budget("public lens auxiliary bytes"))?;
    capture.charge_work(seed.len() as u64 + source_json.len() as u64)?;
    sink.insert_chunked(
        "knowledge_compact_lens_next",
        &["kind", "id", "source_sha256", "seed_sha256", "json"],
        &[
            quoted(capture, kind)?,
            quoted(capture, id)?,
            quoted(capture, &source_sha)?,
            quoted(capture, &seed_sha)?,
            quoted(capture, &seed)?,
        ],
        &format!(
            "kind={} AND id={}",
            quoted(capture, kind)?,
            quoted(capture, id)?
        ),
        &[("json", &seed)],
    )?;
    counts.compact_rows = counts
        .compact_rows
        .checked_add(1)
        .ok_or(Error::Budget("public compact lens rows"))?;
    let sort_key = lower_search(capture, id)?;
    for field in ["view_ids", "graph_layers"] {
        for value in member_values(item, field)? {
            let row = serde_json::to_vec(&(kind, field, &value, id, &sort_key))
                .map_err(|_| Error::Invalid("public lens membership row JSON"))?;
            capture.charge_work(row.len() as u64)?;
            counts.auxiliary_bytes = counts
                .auxiliary_bytes
                .checked_add(row.len() as u64)
                .filter(|n| *n <= max_bytes)
                .ok_or(Error::Budget("public lens auxiliary bytes"))?;
            counts.membership_rows = counts
                .membership_rows
                .checked_add(1)
                .filter(|n| *n <= max_memberships)
                .ok_or(Error::Budget("public lens memberships"))?;
            sink.insert(
                "knowledge_lens_memberships_next",
                &["kind", "field", "value", "id", "sort_key"],
                &[
                    quoted(capture, kind)?,
                    quoted(capture, field)?,
                    quoted(capture, &value)?,
                    quoted(capture, id)?,
                    quoted(capture, &sort_key)?,
                ],
            )?;
        }
    }
    Ok(())
}
