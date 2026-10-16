//! Physical producers for the existing exploration continuation. Every seek
//! and verified payload uses the caller's held, cumulative prepared Read.
use crate::compressed_search_sqlite::Read;
use crate::exploration_plan::{ExplorationNeed, ExplorationReply};
use crate::prepared_inspect::{budget, codec, corrupt, full_row_with_size, storage_error, table};
use crate::search_v2::{SearchKind, SearchV2Error};
use rusqlite::{ToSql, types::Value};
use tos_foundation::{JsonValue, emit_python_compact_json};
type Result<T> = std::result::Result<T, SearchV2Error>;

fn strings(values: &[String], cap: usize) -> Result<String> {
    let value = JsonValue::Array(
        values
            .iter()
            .map(|s| crate::compressed_search_state::string(s))
            .collect(),
    );
    String::from_utf8(emit_python_compact_json(&value, codec(cap)).map_err(|_| budget())?)
        .map_err(|_| corrupt())
}
fn bound(limit: usize, read: &Read<'_>) -> Result<i64> {
    if limit == 0 || limit > read.limits.max_rows {
        return Err(budget());
    }
    i64::try_from(limit).map_err(|_| budget())
}
fn ids(read: &mut Read<'_>, sql: &str, args: &[Value], cap: usize) -> Result<Vec<String>> {
    let sql = format!(
        "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND {cap} THEN id END FROM ({sql})"
    );
    let args = args.iter().map(|v| v as &dyn ToSql).collect::<Vec<_>>();
    read.query(&sql, &args, true, |r| r.get::<_, Option<String>>(0))
        .map_err(storage_error)?
        .into_iter()
        .map(|v| v.ok_or_else(budget))
        .collect()
}
pub(crate) fn produce(
    read: &mut Read<'_>,
    need: &ExplorationNeed,
    max_field_bytes: usize,
) -> Result<ExplorationReply> {
    read.check_abort().map_err(storage_error)?;
    let cap = max_field_bytes.min(read.limits.max_row_bytes);
    if cap == 0 {
        return Err(budget());
    }
    let reply = match need {
        ExplorationNeed::Rows {
            kind,
            ids: requested,
            ..
        } => {
            if requested.len() > read.limits.max_rows {
                return Err(budget());
            }
            let mut rows = Vec::new();
            let mut raw_bytes = Vec::new();
            let mut ambiguous_ids = Vec::new();
            // Availability is custody data. The maintained plan, not this
            // producer, applies allow_missing/allow_ambiguous semantics.
            for id in requested {
                if id.is_empty() || id.len() > cap {
                    return Err(budget());
                }
                let count = read
                    .one(
                        &format!(
                            "SELECT count(*) FROM (SELECT id FROM {} WHERE id=?1 LIMIT 2)",
                            table(*kind)
                        ),
                        &[&id],
                        false,
                        |r| r.get::<_, i64>(0),
                    )
                    .map_err(storage_error)?
                    .ok_or_else(corrupt)?;
                match count {
                    0 => (),
                    1 => {
                        let (row, size) = full_row_with_size(read, *kind, id, cap)?;
                        rows.push(row);
                        raw_bytes.push(size);
                    }
                    2 => ambiguous_ids.push(id.clone()),
                    _ => return Err(corrupt()),
                }
            }
            ExplorationReply::Rows {
                rows,
                raw_bytes,
                ambiguous_ids,
            }
        }
        ExplorationNeed::Focus {
            field,
            identifier,
            sources,
            source_priority,
            limit,
        } => {
            if identifier.is_empty() || identifier.len() > cap {
                return Err(budget());
            }
            let limit = bound(*limit, read)?;
            let index = match *field {
                "id" => "",
                "entity_id" => " INDEXED BY knowledge_nodes_identity_seek",
                "native_id" => " INDEXED BY knowledge_nodes_native_idx",
                _ => return Err(corrupt()),
            };
            let mut args = vec![
                Value::Text(identifier.clone()),
                Value::Text(strings(sources, read.limits.max_row_bytes)?),
            ];
            let order = if *field == "entity_id" && !source_priority.is_empty() {
                if source_priority.len() > read.limits.max_rows {
                    return Err(budget());
                }
                let mut order = String::from("CASE source_graph ");
                for (source, rank) in source_priority {
                    if source.len() > cap {
                        return Err(budget());
                    }
                    order.push_str("WHEN ? THEN ? ");
                    args.push(Value::Text(source.clone()));
                    args.push(Value::Integer(i64::try_from(*rank).map_err(|_| budget())?));
                }
                order.push_str("ELSE 99 END,id");
                format!(" ORDER BY {order}")
            } else if *field == "native_id" && limit <= 2 {
                // Two hits cause ambiguity before payloads are loaded. Their
                // order cannot affect that refusal; avoid an alias-wide sort.
                String::new()
            } else {
                String::from(" ORDER BY id")
            };
            args.push(Value::Integer(limit));
            let found = ids(
                read,
                &format!(
                    "SELECT id FROM knowledge_nodes{index} WHERE {field}=? AND source_graph IN (SELECT value FROM json_each(?)){order} LIMIT ?"
                ),
                &args,
                cap,
            )?;
            let matched = found.len();
            let (rows, raw_bytes) = if matched == 1 {
                let (row, size) = full_row_with_size(read, SearchKind::Nodes, &found[0], cap)?;
                (vec![row], vec![size])
            } else {
                (vec![], vec![])
            };
            ExplorationReply::Focus {
                matched,
                rows,
                raw_bytes,
            }
        }
        ExplorationNeed::IdentityPage {
            node_id,
            entity_id,
            expanded_entities,
            declared_prefix,
            after,
            sources,
            limit,
        } => {
            let limit = bound(*limit, read)?;
            if node_id.len() > cap
                || after.len() > cap
                || entity_id.as_ref().is_some_and(|v| v.len() > cap)
            {
                return Err(budget());
            }
            let sources = Value::Text(strings(sources, read.limits.max_row_bytes)?);
            let found = if let Some(entity) = entity_id {
                ids(
                    read,
                    "SELECT id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek WHERE entity_id=? AND id>? AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?",
                    &[
                        Value::Text(entity.clone()),
                        Value::Text(after.clone()),
                        sources,
                        Value::Integer(limit),
                    ],
                    cap,
                )?
            } else {
                let prefix = declared_prefix.ok_or_else(corrupt)?;
                ids(
                    read,
                    "SELECT id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek WHERE entity_id=(SELECT entity_id FROM knowledge_nodes WHERE id=? AND substr(entity_id,1,length(?))=? AND entity_id NOT IN (SELECT value FROM json_each(?))) AND id>? AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?",
                    &[
                        Value::Text(node_id.clone()),
                        Value::Text(prefix.into()),
                        Value::Text(prefix.into()),
                        Value::Text(strings(expanded_entities, read.limits.max_row_bytes)?),
                        Value::Text(after.clone()),
                        sources,
                        Value::Integer(limit),
                    ],
                    cap,
                )?
            };
            ExplorationReply::Ids(found)
        }
        ExplorationNeed::AdjacencyPage {
            node_id,
            after,
            limit,
        } => {
            let bound = bound(*limit, read)?;
            if node_id.len() > cap || after.len() > cap {
                return Err(budget());
            }
            // Limit each covering seek before merging; self loops occur once.
            let mut found = Vec::new();
            for side in ["from", "to"] {
                found.extend(ids(read, &format!("SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_{side}_seek WHERE {side}_id=? AND id>? ORDER BY id LIMIT ?"),
                    &[Value::Text(node_id.clone()), Value::Text(after.clone()), Value::Integer(bound)], cap)?);
            }
            found.sort();
            found.dedup();
            found.truncate(*limit);
            ExplorationReply::Ids(found)
        }
    };
    read.check_abort().map_err(storage_error)?;
    Ok(reply)
}
