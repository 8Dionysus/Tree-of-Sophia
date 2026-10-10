//! Prepared SQLite producers for the maintained published LensPlan.
//! All physical work shares the caller's retained Read; the plan owns semantics
//! and its sole bounded parsed-row cache. Optional compact/membership stores
//! are not present in the maintained local publication profile.
use crate::InspectBudget;
use crate::compressed_search_sqlite::Read;
use crate::knowledge_lens::LensBudget;
use crate::lens_plan::*;
use crate::prepared_inspect::{budget, codec, corrupt, full_row_with_size, storage_error, table};
use crate::search_v2::{SearchKind, SearchV2Error};
use rusqlite::{ToSql, types::Value};
use tos_compiler::local_prepared::PreparedReadTransaction;
use tos_foundation::{
    Digest256, JsonMode, JsonValue, emit_python_compact_json, parse_json, python_lower_unicode16_v1,
};
type Result<T> = std::result::Result<T, SearchV2Error>;

pub(crate) fn lens(
    read: &mut Read<'_>,
    view: &PreparedReadTransaction<'_>,
    spec: &JsonValue,
) -> Result<JsonValue> {
    let limits = read.limits;
    emit_python_compact_json(spec, codec(65_536.min(limits.max_row_bytes)))
        .map_err(|_| budget())?;
    let metadata = metadata(read, "knowledge_lens_top")?;
    let raw = &metadata.0;
    let metadata = &metadata.1;
    let top = view.top();
    let revision = top
        .object_get("source_revision")
        .and_then(JsonValue::as_str)
        .ok_or_else(corrupt)?;
    if top.object_get("lens_sha256").and_then(JsonValue::as_str)
        != Some(Digest256::of_bytes(raw).to_hex().as_str())
    {
        return Err(corrupt());
    }
    let keys = [
        "schema",
        "execution_version",
        "source_revision",
        "sort_key",
        "unicode_version",
        "query_properties",
        "node_counts",
        "relation_counts",
    ];
    if metadata.as_object().is_none_or(|fields| {
        fields.len() != keys.len()
            || fields
                .iter()
                .any(|(key, _)| !keys.contains(&key.as_str().unwrap_or("")))
    }) || [
        ("schema", "tos_published_lens_metadata_v1"),
        ("execution_version", "tos-lens-execution-v7"),
        ("source_revision", revision),
        ("sort_key", "python-str-or-empty-lower-v1"),
        ("unicode_version", "16.0.0"),
    ]
    .iter()
    .any(|(key, value)| metadata.object_get(key).and_then(JsonValue::as_str) != Some(*value))
    {
        return Err(corrupt());
    }
    let inspect = InspectBudget {
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
    // Retain the existing published profile; every cap is additionally narrowed
    // by the actual caller's shared request allowance.
    let plan_budget = PublishedLensBudget {
        lens: LensBudget {
            inspect,
            max_candidates: 2048.min(limits.max_rows),
            max_path_steps: 100_000.min(limits.max_vm_steps as usize),
            max_adjacency_rows: limits.max_rows,
            block_size: 16.min(limits.max_rows),
        },
        max_callbacks: 32768.min(limits.max_vm_steps as usize),
        max_sort_bytes: (4 * 1024 * 1024).min(limits.max_response_bytes),
        max_cache_bytes: (2 * 1024 * 1024).min(limits.max_bytes),
        max_cache_entries: 64.min(limits.max_rows),
    };
    let mut plan = LensPlan::published_with_abort(
        spec,
        metadata,
        top,
        revision,
        Some(view.binding()),
        plan_budget,
        read.abort_handle(),
    )?;
    loop {
        read.check_abort().map_err(storage_error)?;
        if plan.advance()? {
            break;
        }
        read.check_abort().map_err(storage_error)?;
        let need = plan.need().ok_or_else(corrupt)?;
        let reply = produce(read, &need, inspect.max_field_bytes)?;
        drop(need);
        plan.resume(reply)?;
    }
    let packet = plan.finish()?;
    emit_python_compact_json(&packet, codec(limits.max_response_bytes)).map_err(|_| budget())?;
    read.check_abort().map_err(storage_error)?;
    Ok(packet)
}
fn metadata(read: &mut Read<'_>, key: &str) -> Result<(Vec<u8>, JsonValue)> {
    let probes = read.query("SELECT part,typeof(json_chunk),length(CAST(json_chunk AS BLOB)) FROM edge_meta WHERE key=?1 ORDER BY part LIMIT 257", &[&key], false,
        |r| Ok((r.get::<_,i64>(0)?, r.get::<_,String>(1)?, r.get::<_,i64>(2)?))).map_err(storage_error)?;
    if probes.is_empty() || probes.len() > 256 {
        return Err(corrupt());
    }
    let mut bytes = 0usize;
    for (i, (part, kind, len)) in probes.iter().enumerate() {
        if *part != i as i64 || kind != "text" {
            return Err(corrupt());
        }
        let len = usize::try_from(*len).map_err(|_| corrupt())?;
        if len > 131072 {
            return Err(budget());
        }
        bytes = bytes
            .checked_add(len)
            .filter(|n| *n <= read.limits.max_row_bytes)
            .ok_or_else(budget)?;
    }
    read.charge_bytes(bytes).map_err(storage_error)?;
    let chunks = read
        .query(
            "SELECT part,json_chunk FROM edge_meta WHERE key=?1 ORDER BY part LIMIT 257",
            &[&key],
            false,
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(storage_error)?;
    if chunks.len() != probes.len() {
        return Err(corrupt());
    }
    let mut raw = Vec::with_capacity(bytes);
    for (i, (part, chunk)) in chunks.into_iter().enumerate() {
        if part != i as i64 || chunk.len() as i64 != probes[i].2 {
            return Err(corrupt());
        }
        raw.extend_from_slice(chunk.as_bytes());
    }
    let value = parse_json(
        &raw,
        JsonMode::PublishedStrict,
        codec(read.limits.max_row_bytes),
    )
    .map_err(|_| corrupt())?
    .into_root();
    Ok((raw, value))
}
fn encoded(values: &[String], cap: usize) -> Result<String> {
    let array = JsonValue::Array(
        values
            .iter()
            .map(|s| crate::compressed_search_state::string(s))
            .collect(),
    );
    let bytes = emit_python_compact_json(&array, codec(cap)).map_err(|_| budget())?;
    String::from_utf8(bytes).map_err(|_| corrupt())
}
fn index(kind: SearchKind, field: &str) -> Result<&'static str> {
    match (kind, field) {
        (_, "id") => Ok(if kind == SearchKind::Nodes {
            "sqlite_autoindex_knowledge_nodes_1"
        } else {
            "sqlite_autoindex_knowledge_relations_1"
        }),
        (SearchKind::Nodes, "entity_id") => Ok("knowledge_nodes_identity_seek"),
        (SearchKind::Nodes, "native_id") => Ok("knowledge_nodes_native_idx"),
        (SearchKind::Relations, "native_id") => Ok("knowledge_relations_native_idx"),
        _ => Err(corrupt()),
    }
}
fn refs(args: &[Value]) -> Vec<&dyn ToSql> {
    args.iter().map(|v| v as &dyn ToSql).collect()
}
fn ids(read: &mut Read<'_>, sql: &str, args: &[Value], cap: usize) -> Result<Vec<String>> {
    // CASE rejects an oversized value before Rust allocates the SQL string.
    let sql = format!(
        "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND {cap} THEN id END FROM ({sql})"
    );
    let result = read
        .query(&sql, &refs(args), true, |r| r.get::<_, Option<String>>(0))
        .map_err(storage_error)?;
    result.into_iter().map(|v| v.ok_or_else(budget)).collect()
}
fn source(sources: &[String], args: &mut Vec<Value>, cap: usize) -> Result<String> {
    args.push(Value::Text(encoded(sources, cap)?));
    Ok("r.source_graph IN (SELECT value FROM json_each(?))".into())
}
fn condition(query: &LensHeaderQuery, args: &mut Vec<Value>, cap: usize) -> Result<String> {
    let mut sql = source(&query.sources, args, cap)?;
    if let Some(cells) = &query.dimensions {
        let value = JsonValue::Array(
            cells
                .iter()
                .map(|c| {
                    JsonValue::Array(
                        c.iter()
                            .map(|v| crate::compressed_search_state::string(v))
                            .collect(),
                    )
                })
                .collect(),
        );
        let value = emit_python_compact_json(&value, codec(cap)).map_err(|_| budget())?;
        args.push(Value::Text(
            String::from_utf8(value).map_err(|_| corrupt())?,
        ));
        let dims = if query.kind == SearchKind::Nodes {
            ["source_graph", "kind_id", "type_id"]
        } else {
            ["source_graph", "predicate_id", "relation_type_id"]
        };
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM json_each(?) cell WHERE {})",
            dims.iter()
                .enumerate()
                .map(|(i, f)| format!("r.{f}=json_extract(cell.value,'$[{i}]')"))
                .collect::<Vec<_>>()
                .join(" AND ")
        ));
    }
    for (field, values, negative) in [
        ("predicate_id", &query.predicate_ids, false),
        ("predicate_id", &query.excluded_predicates, true),
        ("relation_type_id", &query.excluded_relation_types, true),
    ] {
        if !values.is_empty() {
            sql.push_str(&format!(
                " AND r.{field} {}IN (SELECT value FROM json_each(?))",
                if negative { "NOT " } else { "" }
            ));
            args.push(Value::Text(encoded(values, cap)?));
        }
    }
    // Auxiliary stores are explicitly absent; the shared plan performs its
    // maintained full-row fallback and must never request membership SQL.
    if query.membership.is_some() {
        return Err(corrupt());
    }
    if let Some(endpoint) = &query.endpoint {
        let (field, id) = match endpoint {
            LensEndpoint::From(id) => ("from_id", id),
            LensEndpoint::To(id) => ("to_id", id),
        };
        sql.push_str(&format!(" AND r.{field}=?"));
        args.push(Value::Text(id.clone()));
    }
    if let Some(eligible) = &query.eligible {
        let (basis, traversed, both) = match eligible {
            LensEligibility::Both {
                basis, traversed, ..
            } => (basis, traversed, true),
            LensEligibility::Either { basis, traversed } => (basis, traversed, false),
        };
        sql.push_str(&format!(" AND ((r.from_id IN (SELECT value FROM json_each(?)) {} r.to_id IN (SELECT value FROM json_each(?))) OR r.id IN (SELECT value FROM json_each(?)))",if both {"AND"} else {"OR"}));
        args.extend([
            Value::Text(encoded(basis, cap)?),
            Value::Text(encoded(basis, cap)?),
            Value::Text(encoded(traversed, cap)?),
        ]);
    }
    Ok(sql)
}
fn produce(read: &mut Read<'_>, need: &LensNeed, cap: usize) -> Result<LensReply> {
    let payload_cap = read.limits.max_row_bytes;
    match need {
        LensNeed::Auxiliary { .. } => Ok(LensReply::Stores(LensStores::default())),
        LensNeed::ExactRows {
            kind,
            ids,
            representation,
        } => {
            if !matches!(representation, LensRepresentation::Full) {
                return Err(corrupt());
            }
            let values = ids
                .iter()
                .map(|id| full_row_with_size(read, *kind, id, cap))
                .collect::<Result<Vec<_>>>()?;
            let (rows, raw_bytes) = values.into_iter().unzip();
            Ok(LensReply::Rows { rows, raw_bytes })
        }
        LensNeed::LookupRows {
            field,
            identifier,
            limit,
        } => {
            let idx = index(SearchKind::Nodes, field)?;
            let mut selected = ids(
                read,
                &format!("SELECT id FROM knowledge_nodes INDEXED BY {idx} WHERE {field}=? LIMIT ?"),
                &[
                    Value::Text(identifier.clone()),
                    Value::Integer((*limit + 1) as i64),
                ],
                cap,
            )?;
            if selected.len() > *limit {
                return Err(budget());
            }
            selected.sort();
            let (rows, raw_bytes) = selected
                .iter()
                .map(|id| full_row_with_size(read, SearchKind::Nodes, id, cap))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .unzip();
            Ok(LensReply::Rows { rows, raw_bytes })
        }
        LensNeed::CandidateIds {
            kind,
            sources,
            identities,
            index: choice,
            after,
            limit,
        } => {
            let mut args = vec![];
            let mut where_sql = source(sources, &mut args, payload_cap)?;
            for group in identities {
                let mut clauses = vec![];
                for term in &group.terms {
                    index(*kind, &term.field)?;
                    clauses.push(format!(
                        "r.{} IN (SELECT value FROM json_each(?))",
                        term.field
                    ));
                    args.push(Value::Text(encoded(&term.values, payload_cap)?));
                }
                where_sql.push_str(&format!(
                    " AND ({})",
                    if clauses.is_empty() {
                        "0".into()
                    } else {
                        clauses.join(if group.all { " AND " } else { " OR " })
                    }
                ));
            }
            let after = match after {
                None => "",
                Some(LensCandidateCursor::Id(id)) => id,
                _ => return Err(corrupt()),
            };
            args.extend([Value::Text(after.to_owned()), Value::Integer(*limit as i64)]);
            let idx = match choice {
                LensCandidateIndex::Identity(f) => index(*kind, f)?,
                _ => {
                    if *kind == SearchKind::Nodes {
                        "knowledge_nodes_source_kind_idx"
                    } else {
                        "knowledge_relations_source_predicate_idx"
                    }
                }
            };
            let selected = ids(
                read,
                &format!(
                    "SELECT r.id FROM {} r INDEXED BY {idx} WHERE {where_sql} AND r.id>? ORDER BY r.id LIMIT ?",
                    table(*kind)
                ),
                &args,
                cap,
            )?;
            Ok(LensReply::Candidates(LensCandidatePage {
                rows: selected
                    .into_iter()
                    .map(|id| LensCandidate {
                        id,
                        source: None,
                        position: None,
                    })
                    .collect(),
            }))
        }
        LensNeed::FocusIds {
            field,
            identifier,
            sources,
            source_priority,
            limit,
        } => {
            let idx = index(SearchKind::Nodes, field)?;
            let mut args = vec![];
            let scope = source(sources, &mut args, payload_cap)?;
            args.push(Value::Text(identifier.clone()));
            let order = if field == "entity_id" {
                args.push(Value::Text(encoded(source_priority, payload_cap)?));
                "coalesce((SELECT CAST(key AS INTEGER) FROM json_each(?) WHERE value=r.source_graph),99),r.id"
            } else {
                "r.id"
            };
            args.push(Value::Integer(*limit as i64));
            Ok(LensReply::Ids(ids(
                read,
                &format!(
                    "SELECT r.id FROM knowledge_nodes r INDEXED BY {idx} WHERE {scope} AND r.{field}=? ORDER BY {order} LIMIT ?"
                ),
                &args,
                cap,
            )?))
        }
        LensNeed::IdentityIds {
            identifier,
            sources,
            after,
            limit,
        } => {
            let mut args = vec![Value::Text(identifier.clone())];
            let scope = source(sources, &mut args, payload_cap)?;
            args.extend([Value::Text(after.clone()), Value::Integer(*limit as i64)]);
            Ok(LensReply::Ids(ids(
                read,
                &format!(
                    "SELECT r.id FROM knowledge_nodes r INDEXED BY knowledge_nodes_identity_seek WHERE r.entity_id=? AND {scope} AND r.id>? ORDER BY r.id LIMIT ?"
                ),
                &args,
                cap,
            )?))
        }
        LensNeed::IncidentIds {
            identifier,
            after,
            limit,
        } => {
            let sql = "SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id=? AND id>? UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id=? AND id>?) ORDER BY id LIMIT ?";
            Ok(LensReply::Ids(ids(
                read,
                sql,
                &[
                    Value::Text(identifier.clone()),
                    Value::Text(after.clone()),
                    Value::Text(identifier.clone()),
                    Value::Text(after.clone()),
                    Value::Integer(*limit as i64),
                ],
                cap,
            )?))
        }
        LensNeed::EntityAliasIds {
            entities,
            sources,
            exclude,
            limit,
        } => {
            let mut args = vec![Value::Text(encoded(entities, payload_cap)?)];
            let scope = source(sources, &mut args, payload_cap)?;
            args.extend([
                Value::Text(encoded(exclude, payload_cap)?),
                Value::Integer(*limit as i64),
            ]);
            Ok(LensReply::Ids(ids(
                read,
                &format!(
                    "SELECT r.id FROM knowledge_nodes r INDEXED BY knowledge_nodes_identity_seek WHERE r.entity_id IN (SELECT value FROM json_each(?)) AND {scope} AND r.id NOT IN (SELECT value FROM json_each(?)) ORDER BY r.id LIMIT ?"
                ),
                &args,
                cap,
            )?))
        }
        LensNeed::NodeSources { ids: selected } => {
            let selected = ids(
                read,
                "SELECT id FROM knowledge_nodes WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?",
                &[
                    Value::Text(encoded(selected, payload_cap)?),
                    Value::Integer((selected.len() + 1) as i64),
                ],
                cap,
            )?;
            let mut values = vec![];
            for id in selected {
                let (row, _) = full_row_with_size(read, SearchKind::Nodes, &id, cap)?;
                let source = row
                    .object_get("source_graph")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(corrupt)?
                    .to_owned();
                values.push((id, source));
            }
            Ok(LensReply::Sources(values))
        }
        LensNeed::Count { query } => {
            let mut args = vec![];
            let condition = condition(query, &mut args, payload_cap)?;
            let count = read
                .one(
                    &format!(
                        "SELECT count(*) FROM {} r WHERE {condition}",
                        table(query.kind)
                    ),
                    &refs(&args),
                    false,
                    |r| r.get::<_, i64>(0),
                )
                .map_err(storage_error)?
                .and_then(|n| u64::try_from(n).ok())
                .ok_or_else(corrupt)?;
            Ok(LensReply::Count(count))
        }
        LensNeed::OrderedHeaders {
            query,
            after,
            limit,
        } => {
            let mut args = vec![Value::Text(
                if query.kind == SearchKind::Nodes {
                    "node"
                } else {
                    "relation"
                }
                .into(),
            )];
            let condition = condition(query, &mut args, payload_cap)?;
            let (key, id) = after
                .as_ref()
                .map(|(k, i)| (k.as_str(), i.as_str()))
                .unwrap_or(("", ""));
            args.extend([
                Value::Text(key.into()),
                Value::Text(id.into()),
                Value::Integer(*limit as i64),
            ]);
            let sql = format!(
                "SELECT l.id FROM knowledge_lens_order l INDEXED BY knowledge_lens_order_sort CROSS JOIN {} r ON r.id=l.id WHERE l.kind=? AND {condition} AND (l.sort_key,l.id)>(?,?) ORDER BY l.sort_key,l.id LIMIT ?",
                table(query.kind)
            );
            let selected = ids(read, &sql, &args, cap)?;
            let mut headers = vec![];
            for id in selected {
                let (row, _) = full_row_with_size(read, query.kind, &id, cap)?;
                let sql = format!(
                    "SELECT CASE WHEN typeof(sort_key)='text' AND length(CAST(sort_key AS BLOB))<={cap} THEN sort_key END,CASE WHEN typeof(from_id)='text' AND length(CAST(from_id AS BLOB))<={cap} THEN from_id END,CASE WHEN typeof(to_id)='text' AND length(CAST(to_id AS BLOB))<={cap} THEN to_id END FROM knowledge_lens_order WHERE kind=?1 AND id=?2 LIMIT 2"
                );
                let (key, from, to) = read
                    .one(
                        &sql,
                        &[
                            &if query.kind == SearchKind::Nodes {
                                "node"
                            } else {
                                "relation"
                            },
                            &id,
                        ],
                        true,
                        |r| {
                            Ok((
                                r.get::<_, Option<String>>(0)?,
                                r.get::<_, Option<String>>(1)?,
                                r.get::<_, Option<String>>(2)?,
                            ))
                        },
                    )
                    .map_err(storage_error)?
                    .ok_or_else(corrupt)?;
                let (key, from, to) = (
                    key.ok_or_else(budget)?,
                    from.ok_or_else(budget)?,
                    to.ok_or_else(budget)?,
                );
                let expected =
                    python_lower_unicode16_v1(&id, cap, cap, cap).map_err(|_| budget())?;
                let endpoint = |field| {
                    if query.kind == SearchKind::Nodes {
                        Ok("")
                    } else {
                        row.object_get(field)
                            .and_then(JsonValue::as_str)
                            .ok_or_else(corrupt)
                    }
                };
                if key != expected || from != endpoint("from_id")? || to != endpoint("to_id")? {
                    return Err(corrupt());
                }
                headers.push(LensHeader {
                    id,
                    sort_key: key,
                    from_id: from,
                    to_id: to,
                });
            }
            Ok(LensReply::Headers(headers))
        }
    }
}
