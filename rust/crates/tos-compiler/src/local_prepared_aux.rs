//! Optional offline compact seeds and exact list-membership postings.
//! No request-time DDL or implicit bootstrap. The caller owns the transaction,
//! rollback, disk reservation and SQLite file/VM caps for explicit installation.

use crate::{
    Error, Result,
    d1_public_lens::compact_seed,
    local_prepared::{compact, metadata, parse, snapshot_binding},
};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;
use tos_foundation::{Digest256, JsonValue, python_lower_unicode16_v1};

const COMPACT_TABLE: &str = "knowledge_compact_lens";
const COMPACT_STATE: &str = "knowledge_compact_lens_state";
const COMPACT_SCHEMA: &str = "tos_compact_lens_carrier_v1";
const MEMBERSHIP_TABLE: &str = "knowledge_lens_memberships";
const MEMBERSHIP_STATE: &str = "knowledge_lens_membership_state";
const MEMBERSHIP_SCHEMA: &str = "tos_lens_membership_index_v1";
const MAX_ROW_BYTES: usize = 1_048_576;
const MAX_BINDING_BYTES: usize = 65_536;
const MAX_IDENTIFIER_BYTES: usize = 16_384;
const MAX_IDENTIFIER_POINTS: usize = 4096;
const MAX_MEMBERS: usize = 256;
const MAX_MEMBER_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuxPresence {
    pub compact: bool,
    pub membership: bool,
}

/// Complete installation has independent input, compact-output and posting caps.
#[derive(Clone, Copy, Debug)]
pub struct AuxInstallLimits {
    pub max_rows: u64,
    pub max_source_bytes: u64,
    pub max_seed_bytes: u64,
    pub max_entries: u64,
}

impl Default for AuxInstallLimits {
    fn default() -> Self {
        Self {
            max_rows: 200_000,
            max_source_bytes: 64 * 1024 * 1024,
            max_seed_bytes: 32 * 1024 * 1024,
            max_entries: 2_000_000,
        }
    }
}

impl AuxInstallLimits {
    fn validate(self) -> Result<()> {
        if self.max_rows == 0
            || self.max_source_bytes == 0
            || self.max_seed_bytes == 0
            || self.max_entries == 0
        {
            return Err(Error::Invalid("optional prepared-store positive budgets"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuxPutReceipt {
    pub seed_bytes: u64,
    pub entries: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuxInstallReceipt {
    pub rows: u64,
    pub source_bytes: u64,
    pub seed_bytes: u64,
    pub entries: u64,
}

fn transaction(db: &Connection) -> Result<()> {
    if db.is_autocommit() {
        return Err(Error::Invalid(
            "optional prepared-store explicit transaction",
        ));
    }
    Ok(())
}

fn exists(db: &Connection, table: &str) -> Result<bool> {
    Ok(db
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=? LIMIT 1",
            [table],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn validate_lane(db: &Connection, state: &str, schema: &str, binding: &str) -> Result<bool> {
    if !exists(db, state)? {
        return Ok(false);
    }
    let mut statement = db.prepare(&format!(
        "SELECT CASE WHEN length(CAST(schema AS BLOB))<=128 THEN schema ELSE NULL END,\
         CASE WHEN length(CAST(binding AS BLOB))<=? THEN binding ELSE NULL END,valid \
         FROM {state} WHERE singleton=1 LIMIT 2"
    ))?;
    let mut rows = statement.query([MAX_BINDING_BYTES])?;
    let row = rows
        .next()?
        .ok_or(Error::Invalid("optional prepared-store stale state"))?;
    let actual_schema: Option<String> = row.get(0)?;
    let actual_binding: Option<String> = row.get(1)?;
    let valid: i64 = row.get(2)?;
    if actual_schema.as_deref() != Some(schema)
        || actual_binding.as_deref() != Some(binding)
        || valid != 1
        || rows.next()?.is_some()
    {
        return Err(Error::Invalid(
            "optional prepared-store stale or incompatible",
        ));
    }
    Ok(true)
}

/// False means absent only. A present invalidated/incompatible lane refuses.
pub fn validate(db: &Connection, binding: &JsonValue) -> Result<AuxPresence> {
    let binding = compact(binding, MAX_BINDING_BYTES)?;
    Ok(AuxPresence {
        compact: validate_lane(db, COMPACT_STATE, COMPACT_SCHEMA, &binding)?,
        membership: validate_lane(db, MEMBERSHIP_STATE, MEMBERSHIP_SCHEMA, &binding)?,
    })
}

fn addressed(kind: &str, id: &str) -> Result<()> {
    if !matches!(kind, "node" | "relation")
        || id.is_empty()
        || id.len() > MAX_IDENTIFIER_BYTES
        || id.chars().count() > MAX_IDENTIFIER_POINTS
    {
        return Err(Error::Invalid("optional prepared-store addressed identity"));
    }
    Ok(())
}

fn member_values(item: &JsonValue, field: &str) -> Result<BTreeSet<String>> {
    let values = item
        .object_get(field)
        .and_then(JsonValue::as_array)
        .ok_or(Error::Invalid("membership normalized string array"))?;
    if values.len() > MAX_MEMBERS {
        return Err(Error::Budget("membership normalized array entries"));
    }
    let mut result = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .filter(|value| value.len() <= MAX_MEMBER_BYTES)
            .ok_or(Error::Invalid("membership normalized string value"))?;
        result.insert(value.to_owned());
    }
    Ok(result)
}

/// Maintain only previously admitted optional lanes alongside base-row writes.
/// Exact source JSON bytes own the source digest; compact omissions preserve
/// the shared form inputs and all unknown source fields.
pub fn put(
    db: &Connection,
    presence: AuxPresence,
    kind: &str,
    id: &str,
    raw: Option<&str>,
) -> Result<AuxPutReceipt> {
    transaction(db)?;
    addressed(kind, id)?;
    if !presence.compact && !presence.membership {
        return Ok(AuxPutReceipt::default());
    }
    let Some(raw) = raw else {
        if presence.compact {
            db.execute(
                &format!("DELETE FROM {COMPACT_TABLE} WHERE kind=? AND id=?"),
                params![kind, id],
            )?;
        }
        if presence.membership {
            db.execute(
                &format!("DELETE FROM {MEMBERSHIP_TABLE} WHERE kind=? AND id=?"),
                params![kind, id],
            )?;
        }
        return Ok(AuxPutReceipt::default());
    };
    let item = parse(raw, MAX_ROW_BYTES)?;
    if item.object_get("id").and_then(JsonValue::as_str) != Some(id) {
        return Err(Error::Invalid(
            "optional prepared-store source identity differs",
        ));
    }
    // Prepare every requested projection before the first write. Caller still
    // rolls back any later SQLite or whole-installation budget failure.
    let seed = if presence.compact {
        Some(compact(&compact_seed(&item)?, MAX_ROW_BYTES)?)
    } else {
        None
    };
    let members = if presence.membership {
        Some((
            member_values(&item, "view_ids")?,
            member_values(&item, "graph_layers")?,
            python_lower_unicode16_v1(
                id,
                MAX_IDENTIFIER_POINTS,
                MAX_IDENTIFIER_POINTS * 3,
                MAX_IDENTIFIER_BYTES * 3,
            )
            .map_err(|e| Error::Source(e.to_string()))?,
        ))
    } else {
        None
    };
    let mut receipt = AuxPutReceipt::default();
    if let Some(seed) = seed {
        let source_sha = Digest256::of_bytes(raw.as_bytes()).to_hex();
        let seed_sha = Digest256::of_bytes(seed.as_bytes()).to_hex();
        db.execute(
            &format!("INSERT OR REPLACE INTO {COMPACT_TABLE} VALUES (?,?,?,?,?)"),
            params![kind, id, source_sha, seed_sha, seed],
        )?;
        receipt.seed_bytes = seed.len() as u64;
    }
    if let Some((views, layers, sort_key)) = members {
        db.execute(
            &format!("DELETE FROM {MEMBERSHIP_TABLE} WHERE kind=? AND id=?"),
            params![kind, id],
        )?;
        let mut insert = db.prepare(&format!(
            "INSERT INTO {MEMBERSHIP_TABLE} VALUES (?,?,?,?,?)"
        ))?;
        for (field, values) in [("view_ids", views), ("graph_layers", layers)] {
            for value in values {
                insert.execute(params![kind, field, value, id, sort_key])?;
                receipt.entries += 1;
            }
        }
    }
    Ok(receipt)
}

/// Seal after all base and optional writes, using the newly published binding.
pub fn seal(db: &Connection, presence: AuxPresence, binding: &JsonValue) -> Result<()> {
    transaction(db)?;
    let binding = compact(binding, MAX_BINDING_BYTES)?;
    for (present, state) in [
        (presence.compact, COMPACT_STATE),
        (presence.membership, MEMBERSHIP_STATE),
    ] {
        if present
            && db.execute(
                &format!("UPDATE {state} SET binding=?,valid=1 WHERE singleton=1"),
                [&binding],
            )? != 1
        {
            return Err(Error::Invalid("optional prepared-store seal state missing"));
        }
    }
    Ok(())
}

fn create_compact(db: &Connection) -> Result<()> {
    db.execute_batch(&format!(
        "CREATE TABLE {COMPACT_TABLE}(kind TEXT NOT NULL,id TEXT NOT NULL,source_sha256 TEXT NOT NULL,\
         seed_sha256 TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id));\
         CREATE TABLE {COMPACT_STATE}(singleton INTEGER PRIMARY KEY CHECK(singleton=1),\
         schema TEXT NOT NULL,binding TEXT NOT NULL,valid INTEGER NOT NULL CHECK(valid IN (0,1)));"
    ))?;
    for kind in ["node", "relation"] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            db.execute_batch(&format!(
                "CREATE TRIGGER compact_lens_{kind}_{} AFTER {action} ON knowledge_{kind}s \
                 BEGIN UPDATE {COMPACT_STATE} SET valid=0 WHERE singleton=1; END",
                action.to_ascii_lowercase()
            ))?;
        }
    }
    Ok(())
}

fn create_membership(db: &Connection) -> Result<()> {
    db.execute_batch(&format!(
        "CREATE TABLE {MEMBERSHIP_TABLE}(kind TEXT NOT NULL,field TEXT NOT NULL,value TEXT NOT NULL,\
         id TEXT NOT NULL,sort_key TEXT NOT NULL,PRIMARY KEY(kind,field,value,id));\
         CREATE INDEX knowledge_lens_memberships_order ON {MEMBERSHIP_TABLE}(kind,field,value,sort_key,id);\
         CREATE INDEX knowledge_lens_memberships_row ON {MEMBERSHIP_TABLE}(kind,id);\
         CREATE TABLE {MEMBERSHIP_STATE}(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema TEXT NOT NULL,\
         binding TEXT NOT NULL,valid INTEGER NOT NULL CHECK(valid IN (0,1)));"
    ))?;
    for table in ["knowledge_nodes", "knowledge_relations", MEMBERSHIP_TABLE] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            db.execute_batch(&format!(
                "CREATE TRIGGER membership_{table}_{} AFTER {action} ON {table} \
                 BEGIN UPDATE {MEMBERSHIP_STATE} SET valid=0 WHERE singleton=1; END",
                action.to_ascii_lowercase()
            ))?;
        }
    }
    Ok(())
}

fn add(value: &mut u64, amount: u64, max: u64, label: &'static str) -> Result<()> {
    *value = value
        .checked_add(amount)
        .filter(|next| *next <= max)
        .ok_or(Error::Budget(label))?;
    Ok(())
}

// Expected binding comparison ignores dict field order; field order remains
// significant only for the separately stored compact state-binding bytes.
// Call after bounded compact emission so caller-built values are bounded too.
fn same_binding(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Object(left), JsonValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    right
                        .iter()
                        .find(|(other, _)| key == other)
                        .is_some_and(|(_, other)| same_binding(value, other))
                })
        }
        (JsonValue::Array(left), JsonValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_binding(left, right))
        }
        _ => left == right,
    }
}

fn install(
    db: &Connection,
    expected_binding: &JsonValue,
    limits: AuxInstallLimits,
    presence: AuxPresence,
) -> Result<AuxInstallReceipt> {
    transaction(db)?;
    limits.validate()?;
    let binding = compact(expected_binding, MAX_BINDING_BYTES)?;
    // Compare source-selected publication before creating any optional DDL.
    let actual_binding = snapshot_binding(db)?;
    let schema = actual_binding
        .object_get("read_model_schema")
        .and_then(JsonValue::as_str);
    if !matches!(
        schema,
        Some("tos_local_prepared_read_model_v1" | "tos_cloudflare_edge_read_model_v9")
    ) || !same_binding(&actual_binding, expected_binding)
    {
        return Err(Error::Invalid(
            "optional prepared-store publication binding differs",
        ));
    }
    let (state, schema) = if presence.compact {
        (COMPACT_STATE, COMPACT_SCHEMA)
    } else {
        (MEMBERSHIP_STATE, MEMBERSHIP_SCHEMA)
    };
    if exists(db, state)? {
        return Err(Error::Invalid(
            "optional prepared-store exists; explicit migration required",
        ));
    }
    if presence.compact {
        create_compact(db)?;
    } else {
        create_membership(db)?;
    }
    db.execute(
        &format!("INSERT INTO {state} VALUES(1,?,?,0)"),
        params![schema, binding],
    )?;
    let mut receipt = AuxInstallReceipt::default();
    for kind in ["node", "relation"] {
        let mut statement = db.prepare(&format!(
            "SELECT CASE WHEN length(CAST(id AS BLOB))<=? THEN id ELSE NULL END,\
             CASE WHEN length(CAST(json AS BLOB))<=? THEN json ELSE NULL END \
             FROM knowledge_{kind}s ORDER BY id LIMIT ?"
        ))?;
        let row_limit = limits
            .max_rows
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(Error::Budget("optional prepared-store row limit"))?;
        let mut rows = statement.query(params![MAX_IDENTIFIER_BYTES, MAX_ROW_BYTES, row_limit])?;
        while let Some(row) = rows.next()? {
            let id: Option<String> = row.get(0)?;
            let raw: Option<String> = row.get(1)?;
            let id = id.ok_or(Error::Budget(
                "optional prepared-store source identity bytes",
            ))?;
            let raw = raw.ok_or(Error::Budget("optional prepared-store source row bytes"))?;
            addressed(kind, &id)?;
            add(
                &mut receipt.rows,
                1,
                limits.max_rows,
                "optional prepared-store source rows",
            )?;
            add(
                &mut receipt.source_bytes,
                raw.len() as u64,
                limits.max_source_bytes,
                "optional prepared-store source bytes",
            )?;
            let digest = metadata(db, &format!("knowledge_{kind}_digest:{id}"), 1024)?;
            let expected_sha = Digest256::of_bytes(raw.as_bytes()).to_hex();
            if digest.as_object().is_none_or(|value| value.len() != 1)
                || digest.object_get("sha256").and_then(JsonValue::as_str)
                    != Some(expected_sha.as_str())
            {
                return Err(Error::Invalid(
                    "optional prepared-store source row checksum differs",
                ));
            }
            let result = put(db, presence, kind, &id, Some(&raw))?;
            add(
                &mut receipt.seed_bytes,
                result.seed_bytes,
                limits.max_seed_bytes,
                "optional prepared-store seed bytes",
            )?;
            add(
                &mut receipt.entries,
                result.entries,
                limits.max_entries,
                "optional prepared-store membership entries",
            )?;
        }
    }
    seal(db, presence, expected_binding)?;
    Ok(receipt)
}

/// Explicit complete compact installation, refusing existing stores.
pub fn install_compact(
    db: &Connection,
    expected_binding: &JsonValue,
    limits: AuxInstallLimits,
) -> Result<AuxInstallReceipt> {
    install(
        db,
        expected_binding,
        limits,
        AuxPresence {
            compact: true,
            membership: false,
        },
    )
}

/// Explicit complete membership installation, refusing existing stores.
pub fn install_membership(
    db: &Connection,
    expected_binding: &JsonValue,
    limits: AuxInstallLimits,
) -> Result<AuxInstallReceipt> {
    install(
        db,
        expected_binding,
        limits,
        AuxPresence {
            compact: false,
            membership: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> JsonValue {
        parse(r#"{"publication_epoch":1}"#, 1024).unwrap()
    }

    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "BEGIN;CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,json TEXT NOT NULL);\
             CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,json TEXT NOT NULL);",
        )
        .unwrap();
        db
    }

    fn states(db: &Connection) -> AuxPresence {
        create_compact(db).unwrap();
        create_membership(db).unwrap();
        let raw = compact(&binding(), 1024).unwrap();
        for (state, schema) in [
            (COMPACT_STATE, COMPACT_SCHEMA),
            (MEMBERSHIP_STATE, MEMBERSHIP_SCHEMA),
        ] {
            db.execute(
                &format!("INSERT INTO {state} VALUES(1,?,?,1)"),
                params![schema, raw],
            )
            .unwrap();
        }
        validate(db, &binding()).unwrap()
    }

    #[test]
    fn old_base_and_posting_writers_invalidate_until_explicit_seal() {
        let db = db();
        let presence = states(&db);
        for table in ["knowledge_nodes", "knowledge_relations"] {
            for action in [
                format!("INSERT INTO {table} VALUES('x','{{}}')"),
                format!("UPDATE {table} SET json='{{\"changed\":true}}' WHERE id='x'"),
                format!("DELETE FROM {table} WHERE id='x'"),
            ] {
                db.execute_batch(&action).unwrap();
                assert!(validate(&db, &binding()).is_err());
                seal(&db, presence, &binding()).unwrap();
                assert_eq!(validate(&db, &binding()).unwrap(), presence);
            }
        }
        for action in [
            "INSERT INTO knowledge_lens_memberships VALUES('node','view_ids','v','x','x')",
            "UPDATE knowledge_lens_memberships SET sort_key='z' WHERE id='x'",
            "DELETE FROM knowledge_lens_memberships WHERE id='x'",
        ] {
            db.execute_batch(action).unwrap();
            assert!(validate(&db, &binding()).is_err());
            seal(&db, presence, &binding()).unwrap();
        }
    }
}
