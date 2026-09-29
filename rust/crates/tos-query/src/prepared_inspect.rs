//! Prepared SQLite adapter for the existing complete InspectPlan.
//! The caller owns the retained transaction, cumulative meter and disclosure.
use crate::compressed_search_sqlite::Read;
use crate::compressed_search_state::{CompressedSearchError, CompressedSearchErrorCode};
use crate::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode};
use crate::{InspectBudget, InspectNeed, InspectPlan, InspectRequest};
use tos_compiler::local_prepared::PreparedReadTransaction;
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonValue, emit_python_compact_json, parse_json,
};

type Result<T> = std::result::Result<T, SearchV2Error>;
fn corrupt() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::CorruptSelectedCarrier,
        message: "prepared inspect carrier closure invalid",
    }
}
fn budget() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "prepared inspect budget exceeded",
    }
}
pub(crate) fn storage_error(error: CompressedSearchError) -> SearchV2Error {
    SearchV2Error {
        code: match error.code {
            CompressedSearchErrorCode::Cancelled => SearchV2ErrorCode::Cancelled,
            CompressedSearchErrorCode::DeadlineExceeded => SearchV2ErrorCode::DeadlineExceeded,
            CompressedSearchErrorCode::BudgetExceeded => SearchV2ErrorCode::BudgetExceeded,
            CompressedSearchErrorCode::StaleBinding => SearchV2ErrorCode::StaleSelection,
            CompressedSearchErrorCode::InvalidRequest => SearchV2ErrorCode::InvalidRequest,
            _ => SearchV2ErrorCode::CorruptSelectedCarrier,
        },
        message: "prepared inspect read refused",
    }
}
fn codec(maximum: usize) -> JsonLimits {
    JsonLimits {
        max_bytes: maximum,
        max_depth: 96,
        max_visits: 1_000_000,
        max_integer_digits: 4096,
    }
}
fn table(kind: SearchKind) -> &'static str {
    if kind == SearchKind::Nodes {
        "knowledge_nodes"
    } else {
        "knowledge_relations"
    }
}
fn columns(kind: SearchKind) -> &'static [&'static str] {
    if kind == SearchKind::Nodes {
        &[
            "id",
            "entity_id",
            "native_id",
            "source_graph",
            "kind_id",
            "type_id",
        ]
    } else {
        &[
            "id",
            "native_id",
            "source_graph",
            "from_id",
            "to_id",
            "predicate_id",
            "relation_type_id",
        ]
    }
}

pub(crate) fn inspect(
    read: &mut Read<'_>,
    view: &PreparedReadTransaction<'_>,
    kind: SearchKind,
    identifier: &str,
    relation_limit: usize,
) -> Result<JsonValue> {
    let limits = read.limits;
    let plan_budget = InspectBudget {
        max_open_vm_steps: limits.max_vm_steps,
        max_read_vm_steps: limits.max_vm_steps,
        max_matches: 128,
        max_rows: limits.max_rows as u64,
        max_field_bytes: 65_536.min(limits.max_row_bytes),
        max_payload_bytes: limits.max_row_bytes,
        max_decoded_bytes: limits.max_bytes as u64,
        max_response_bytes: limits.max_response_bytes,
        json: codec(limits.max_row_bytes),
    };
    let request = InspectRequest::new(kind, identifier, relation_limit, plan_budget)?;
    let revision = view
        .top()
        .object_get("source_revision")
        .and_then(JsonValue::as_str)
        .ok_or_else(corrupt)?;
    let authority = view
        .top()
        .object_get("authority_boundary")
        .ok_or_else(corrupt)?
        .clone();
    let mut plan = InspectPlan::new(request, revision.to_owned(), authority, plan_budget)?;
    while let Some(need) = plan.need().cloned() {
        read.check_abort().map_err(storage_error)?;
        let before = read.bytes;
        match need {
            InspectNeed::Lookup {
                kind,
                selector,
                identifier,
                limit,
            } => {
                let rows = lookup(
                    read,
                    kind,
                    selector,
                    &identifier,
                    limit,
                    plan_budget.max_field_bytes,
                )?;
                plan.resume_lookup(rows, (read.bytes - before) as u64, read.abort_probe())?;
            }
            InspectNeed::NodeIncident {
                ids,
                relation_limit,
            } => {
                let (count, rows) =
                    incident(read, &ids, relation_limit, plan_budget.max_field_bytes)?;
                plan.resume_incident(
                    count,
                    rows,
                    (read.bytes - before) as u64,
                    read.abort_probe(),
                )?;
            }
            InspectNeed::RelationEndpoints { ids } => {
                let mut rows = Vec::new();
                for id in ids {
                    rows.extend(lookup(
                        read,
                        SearchKind::Nodes,
                        "id",
                        &id,
                        1,
                        plan_budget.max_field_bytes,
                    )?);
                }
                plan.resume_endpoints(rows, (read.bytes - before) as u64, read.abort_probe())?;
            }
        }
    }
    read.check_abort().map_err(storage_error)?;
    let packet = plan.into_packet()?;
    emit_python_compact_json(&packet, codec(limits.max_response_bytes)).map_err(|_| budget())?;
    read.check_abort().map_err(storage_error)?;
    Ok(packet)
}

fn lookup(
    read: &mut Read<'_>,
    kind: SearchKind,
    selector: &str,
    identifier: &str,
    limit: usize,
    field_cap: usize,
) -> Result<Vec<JsonValue>> {
    let index = match (kind, selector) {
        (_, "id") => "",
        (SearchKind::Nodes, "entity_id") => " INDEXED BY knowledge_nodes_entity_idx",
        (SearchKind::Nodes, "native_id") => " INDEXED BY knowledge_nodes_native_idx",
        (SearchKind::Relations, "native_id") => " INDEXED BY knowledge_relations_native_idx",
        _ => return Err(corrupt()),
    };
    let sql = format!(
        "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND ?3 THEN id END FROM {}{index} WHERE {selector}=?1 ORDER BY id LIMIT ?2",
        table(kind)
    );
    let lookahead = limit
        .checked_add(1)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or_else(budget)?;
    let ids = read
        .query(
            &sql,
            &[&identifier, &lookahead, &(field_cap as i64)],
            true,
            |r| r.get::<_, Option<String>>(0),
        )
        .map_err(storage_error)?;
    if ids.len() > limit {
        return Err(if selector == "id" {
            corrupt()
        } else {
            budget()
        });
    }
    ids.into_iter()
        .map(|id| full_row(read, kind, &id.ok_or_else(budget)?, field_cap))
        .collect()
}

fn full_row(
    read: &mut Read<'_>,
    kind: SearchKind,
    identifier: &str,
    field_cap: usize,
) -> Result<JsonValue> {
    let names = columns(kind);
    let sizes = names
        .iter()
        .map(|name| format!("length(CAST({name} AS BLOB))"))
        .collect::<Vec<_>>()
        .join("+");
    let types = names
        .iter()
        .chain(std::iter::once(&"json"))
        .map(|name| format!("typeof({name})='text'"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let fields = names
        .iter()
        .map(|name| format!("length(CAST({name} AS BLOB))<=?2"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!(
        "SELECT CASE WHEN {types} AND {fields} THEN length(CAST(json AS BLOB)) ELSE -1 END,({sizes}) FROM {} WHERE id=?1 LIMIT 2",
        table(kind)
    );
    let (body, indexed) = read
        .one(&sql, &[&identifier, &(field_cap as i64)], false, |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(storage_error)?
        .ok_or_else(corrupt)?;
    let body = usize::try_from(body).map_err(|_| budget())?;
    let indexed = usize::try_from(indexed).map_err(|_| corrupt())?;
    if body == 0 || body > read.limits.max_row_bytes {
        return Err(budget());
    }
    // Admit the complete SQL record as well as its output body before fetching.
    // Each indexed scalar has its own cap; body bytes retain the caller payload cap.
    // The shared byte meter admits their complete sum before any SQL text copy.
    let digest_key = format!(
        "knowledge_{}_digest:{identifier}",
        if kind == SearchKind::Nodes {
            "node"
        } else {
            "relation"
        }
    );
    let (parts, digest_bytes) = read.one("SELECT count(*),coalesce(sum(CASE WHEN typeof(json_chunk)='text' THEN length(CAST(json_chunk AS BLOB)) ELSE 1025 END),0) FROM edge_meta WHERE key=?1",
        &[&digest_key], false, |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).map_err(storage_error)?.ok_or_else(corrupt)?;
    if !(1..=256).contains(&parts) || !(1..=1024).contains(&digest_bytes) {
        return Err(corrupt());
    }
    read.charge_bytes(
        body.checked_add(indexed)
            .and_then(|n| n.checked_add(digest_bytes as usize))
            .ok_or_else(budget)?,
    )
    .map_err(storage_error)?;
    let sql = format!(
        "SELECT {},json FROM {} WHERE id=?1 LIMIT 2",
        names.join(","),
        table(kind)
    );
    let (fields, raw) = read
        .one(&sql, &[&identifier], false, |r| {
            let fields = (0..names.len())
                .map(|i| r.get::<_, String>(i))
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok((fields, r.get::<_, String>(names.len())?))
        })
        .map_err(storage_error)?
        .ok_or_else(corrupt)?;
    let chunks = read
        .query(
            "SELECT part,json_chunk FROM edge_meta WHERE key=?1 ORDER BY part LIMIT 257",
            &[&digest_key],
            false,
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(storage_error)?;
    if chunks.len() != parts as usize
        || chunks
            .iter()
            .enumerate()
            .any(|(i, (part, _))| *part != i as i64)
    {
        return Err(corrupt());
    }
    let digest_raw = chunks
        .into_iter()
        .map(|(_, chunk)| chunk)
        .collect::<String>();
    if raw.len() != body || digest_raw.len() != digest_bytes as usize {
        return Err(corrupt());
    }
    let digest = parse_json(
        digest_raw.as_bytes(),
        JsonMode::PublishedStrict,
        codec(1024),
    )
    .map_err(|_| corrupt())?
    .into_root();
    if digest.as_object().is_none_or(|o| o.len() != 1)
        || digest.object_get("sha256").and_then(JsonValue::as_str)
            != Some(Digest256::of_bytes(raw.as_bytes()).to_hex().as_str())
    {
        return Err(corrupt());
    }
    let item = parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        codec(read.limits.max_row_bytes),
    )
    .map_err(|_| corrupt())?
    .into_root();
    if item.as_object().is_none()
        || names.iter().zip(fields).any(|(name, value)| {
            tos_compiler::local_prepared::index_value(&item, name)
                .ok()
                .as_deref()
                != Some(value.as_str())
        })
    {
        return Err(corrupt());
    }
    Ok(item)
}

fn incident(
    read: &mut Read<'_>,
    ids: &[String],
    limit: usize,
    field_cap: usize,
) -> Result<(u64, Vec<JsonValue>)> {
    let encoded = emit_python_compact_json(
        &JsonValue::Array(
            ids.iter()
                .map(|id| crate::compressed_search_state::string(id))
                .collect(),
        ),
        codec(read.limits.max_row_bytes),
    )
    .map_err(|_| budget())?;
    let encoded = std::str::from_utf8(&encoded).map_err(|_| corrupt())?;
    let incidence = "SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?1)) UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id IN (SELECT value FROM json_each(?1))";
    let count = read
        .one(
            &format!("SELECT count(*) FROM ({incidence})"),
            &[&encoded],
            false,
            |r| r.get::<_, i64>(0),
        )
        .map_err(storage_error)?
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(corrupt)?;
    let ids = read.query(&format!("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND ?3 THEN id END FROM ({incidence}) ORDER BY id LIMIT ?2"),
        &[&encoded, &(limit as i64), &(field_cap as i64)], true, |r| r.get::<_, Option<String>>(0)).map_err(storage_error)?;
    let rows = ids
        .into_iter()
        .map(|id| {
            full_row(
                read,
                SearchKind::Relations,
                &id.ok_or_else(budget)?,
                field_cap,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((count, rows))
}
