//! Full, bounded catalog reduction over normalized knowledge rows.
//!
//! The caller supplies one sealed graph header, registries and authored vocabulary.
//! This module only creates a derived candidate. It does not select a publication.

use crate::{
    Error, QueryVocabulary, Result,
    d1_public_capture::{CreationState, CreationStateHold},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::cell::Cell;
use std::io::{self, Write};
use tos_foundation::{Digest256, JsonLimits};

const SCHEMA: &str = "tos_knowledge_catalog_v1";
const NODE: &str = "node";
const RELATION: &str = "relation";

#[derive(Clone, Copy, Debug)]
pub struct CatalogLimits {
    pub max_rows: u64,
    pub max_row_bytes: usize,
    pub max_catalog_bytes: usize,
    pub max_catalog_entries: u64,
    pub max_staging_pages: u64,
}

impl Default for CatalogLimits {
    fn default() -> Self {
        Self {
            max_rows: 10_000_000,
            max_row_bytes: 8 * 1024 * 1024,
            max_catalog_bytes: 16 * 1024 * 1024,
            max_catalog_entries: 100_000,
            max_staging_pages: 1_000_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CatalogReceipt {
    pub catalog: Value,
    pub sha256: String,
    pub node_count: u64,
    pub relation_count: u64,
}

struct ByteBudget {
    used: Cell<usize>,
    max: usize,
}

impl ByteBudget {
    fn new(max: usize) -> Self {
        Self {
            used: Cell::new(0),
            max,
        }
    }
    fn charge(&self, bytes: usize) -> Result<()> {
        let next = self
            .used
            .get()
            .checked_add(bytes)
            .ok_or(Error::Budget("catalog decoded bytes"))?;
        if next > self.max {
            return Err(Error::Budget("catalog decoded bytes"));
        }
        self.used.set(next);
        Ok(())
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max: usize,
}

impl Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("catalog byte overflow"))?;
        if next > self.max {
            return Err(io::Error::other("catalog byte cap"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn input_preflight(
    header: &Value,
    entity: &Value,
    relation: &Value,
    lenses: &[Value],
    descriptor: &[u8],
    max: usize,
    creation: Option<&CreationState<'_>>,
) -> Result<()> {
    if descriptor.len() > max {
        return Err(Error::Budget("catalog input bytes"));
    }
    if let Some(creation) = creation {
        creation.charge_work(descriptor.len())?;
        creation.with_json_encoded(
            &(header, entity, relation, lenses),
            max - descriptor.len(),
            |_| Ok(()),
        )?;
    } else {
        let mut writer = BoundedWriter {
            bytes: Vec::new(),
            max: max - descriptor.len(),
        };
        serde_json::to_writer(&mut writer, &(header, entity, relation, lenses))
            .map_err(|_| Error::Budget("catalog input bytes"))?;
    }
    Ok(())
}

#[derive(Clone)]
struct Route {
    id: String,
    kinds: Vec<String>,
    predicates: Vec<String>,
    types: Vec<String>,
    relation_types: Vec<String>,
}

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("catalog required string"))
}

fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("catalog required array"))
}

fn strings(v: &Value, key: &str) -> Result<Vec<String>> {
    let values = array(v, key)?;
    let mut out = Vec::new();
    out.try_reserve_exact(values.len())
        .map_err(|_| Error::Budget("catalog string array"))?;
    for value in values {
        out.push(
            value
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or(Error::Invalid("catalog string array"))?,
        );
    }
    Ok(out)
}

fn path<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    name.split('.')
        .try_fold(v, |at, key| at.as_object()?.get(key))
}

fn py_string(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}
struct OwnedText<'state, 'budget> {
    value: String,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
fn py_string_owned<'state, 'budget>(
    value: &Value,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<OwnedText<'state, 'budget>> {
    let Some(owner) = creation else {
        return Ok(OwnedText {
            value: py_string(value),
            _hold: None,
        });
    };
    match value {
        Value::Null => {
            let hold = owner.hold(4 + std::mem::size_of::<String>())?;
            owner.charge_work(4)?;
            Ok(OwnedText {
                value: "None".to_owned(),
                _hold: Some(hold),
            })
        }
        Value::Bool(value) => {
            let text = if *value { "True" } else { "False" };
            let hold = owner.hold(text.len() + std::mem::size_of::<String>())?;
            owner.charge_work(text.len())?;
            Ok(OwnedText {
                value: text.to_owned(),
                _hold: Some(hold),
            })
        }
        Value::String(text) => {
            let hold = owner.hold(text.len() + std::mem::size_of::<String>())?;
            owner.charge_work(text.len())?;
            Ok(OwnedText {
                value: text.clone(),
                _hold: Some(hold),
            })
        }
        _ => owner.with_json_encoded(value, owner.remaining(0)?, |bytes| {
            owner.charge_work(bytes.len())?;
            let text =
                std::str::from_utf8(bytes).map_err(|_| Error::Invalid("catalog string value"))?;
            let hold = owner.hold(text.len() + std::mem::size_of::<String>())?;
            owner.charge_work(text.len())?;
            Ok(OwnedText {
                value: text.to_owned(),
                _hold: Some(hold),
            })
        }),
    }
}
fn static_owned_text<'state, 'budget>(
    value: &'static str,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<OwnedText<'state, 'budget>> {
    let hold = creation
        .map(|owner| owner.hold(value.len() + std::mem::size_of::<String>()))
        .transpose()?;
    if let Some(owner) = creation {
        owner.charge_work(value.len())?;
    }
    Ok(OwnedText {
        value: value.to_owned(),
        _hold: hold,
    })
}
fn py_string_optional<'state, 'budget>(
    value: Option<&Value>,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<OwnedText<'state, 'budget>> {
    match value {
        Some(value) => py_string_owned(value, creation),
        None => static_owned_text("None", creation),
    }
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn ordered_strings(v: &Value, key: &str) -> Result<Vec<String>> {
    strings(v, key)
}

fn owned_bytes(state: Option<&CreationState<'_>>, bytes: usize) -> Result<()> {
    if let Some(state) = state {
        state.retain(bytes)?;
        state.charge_work(bytes)?;
    }
    Ok(())
}
fn owned_value(value: &Value, state: Option<&CreationState<'_>>) -> Result<Value> {
    match state {
        Some(state) => state.clone_value(value),
        None => Ok(value.clone()),
    }
}
fn owned_vec<T>(capacity: usize, state: Option<&CreationState<'_>>) -> Result<Vec<T>> {
    if let Some(state) = state {
        owned_bytes(
            Some(state),
            crate::knowledge_normalization::serde_array_slots_upper(capacity)?,
        )?;
    }
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| Error::Budget("catalog output array"))?;
    Ok(out)
}
fn owned_string_values(values: &[String], state: Option<&CreationState<'_>>) -> Result<Value> {
    let bytes = values.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(value.len())
            .ok_or(Error::Budget("catalog output strings"))
    })?;
    owned_bytes(state, bytes)?;
    let mut out = owned_vec(values.len(), state)?;
    out.extend(values.iter().cloned().map(Value::String));
    Ok(Value::Array(out))
}
fn owned_object(state: Option<&CreationState<'_>>, fields: &[&str]) -> Result<()> {
    let bytes = crate::knowledge_normalization::serde_object_slots_upper(fields.len())?
        .checked_add(fields.iter().map(|key| key.len()).sum::<usize>())
        .ok_or(Error::Budget("catalog output object"))?;
    owned_bytes(state, bytes)
}
fn owned_object_shape(
    state: Option<&CreationState<'_>>,
    slots: usize,
    key_bytes: usize,
) -> Result<()> {
    let bytes = crate::knowledge_normalization::serde_object_slots_upper(slots)?
        .checked_add(key_bytes)
        .ok_or(Error::Budget("catalog output object"))?;
    owned_bytes(state, bytes)
}
fn owned_string(value: &str, state: Option<&CreationState<'_>>) -> Result<Value> {
    owned_bytes(state, value.len())?;
    Ok(Value::String(value.to_owned()))
}
fn owned_sorted_strings(
    value: &Value,
    key: &str,
    state: Option<&CreationState<'_>>,
) -> Result<Value> {
    let source = array(value, key)?;
    let capacity = crate::knowledge_normalization::serde_array_slots_upper(source.len())?;
    let hold = state.map(|owner| owner.hold(capacity)).transpose()?;
    if let Some(owner) = state {
        owner.charge_work(capacity)?;
    }
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(source.len())
        .map_err(|_| Error::Budget("catalog sorted fields"))?;
    for field in source {
        let text = field
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or(Error::Invalid("catalog string array"))?;
        if let Some(owner) = state {
            owner.charge_work(text.len())?;
        }
        fields.push(text);
    }
    let max_len = fields.iter().map(|field| field.len()).max().unwrap_or(0);
    let sort_levels = usize::BITS as usize - fields.len().max(1).leading_zeros() as usize;
    let sort_work = fields
        .len()
        .checked_mul(sort_levels)
        .and_then(|n| n.checked_mul(max_len))
        .ok_or(Error::Budget("catalog sorted fields"))?;
    if let Some(owner) = state {
        owner.charge_work(sort_work)?;
    }
    fields.sort_unstable();
    let mut out = owned_vec(fields.len(), state)?;
    for field in fields {
        out.push(owned_string(field, state)?);
    }
    drop(hold);
    Ok(Value::Array(out))
}
fn static_object(state: Option<&CreationState<'_>>, fields: &[(&str, &str)]) -> Result<Value> {
    let bytes = crate::knowledge_normalization::serde_object_slots_upper(fields.len())?
        .checked_add(fields.iter().map(|(key, _)| key.len()).sum::<usize>())
        .ok_or(Error::Budget("catalog output object"))?;
    owned_bytes(
        state,
        bytes
            .checked_add(fields.iter().map(|(_, value)| value.len()).sum::<usize>())
            .ok_or(Error::Budget("catalog output object"))?,
    )?;
    let mut out = Map::new();
    for (key, value) in fields {
        out.insert((*key).into(), Value::String((*value).into()));
    }
    Ok(Value::Object(out))
}

struct RegistryEntries<'a, 'state, 'budget> {
    entries: Vec<(&'a str, &'a Value)>,
    _hold: Option<CreationStateHold<'state, 'budget>>,
    walk_bytes: usize,
    parent_edges: usize,
}
impl RegistryEntries<'_, '_, '_> {
    fn len(&self) -> usize {
        self.entries.len()
    }
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .binary_search_by(|(candidate, _)| (*candidate).cmp(key))
            .ok()
            .map(|index| self.entries[index].1)
    }
    fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

fn registry_entries<'a, 'state, 'budget>(
    registry: &'a Value,
    field: &str,
    key: &str,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<RegistryEntries<'a, 'state, 'budget>> {
    let source = array(registry, field)?;
    let mut parent_edges = 0usize;
    let mut max_id_bytes = 0usize;
    for entry in source {
        let parents = entry.get("parent_type_ids").and_then(Value::as_array);
        let count = parents.map_or(0, |values| values.len());
        if let Some(creation) = creation {
            creation.charge_work(
                count
                    .checked_mul(std::mem::size_of::<&Value>())
                    .ok_or(Error::Budget("catalog registry work"))?,
            )?;
        }
        parent_edges = parent_edges
            .checked_add(count)
            .ok_or(Error::Budget("catalog registry parents"))?;
    }
    let walk_bytes = parent_edges
        .checked_add(1)?
        .checked_mul(std::mem::size_of::<&str>())?
        .checked_add(
            source
                .len()
                .checked_add(1)?
                .checked_mul(std::mem::size_of::<&str>())?,
        )
        .ok_or(Error::Budget("catalog registry walk"))?;
    let slots = source
        .len()
        .checked_mul(std::mem::size_of::<(&str, &Value)>())
        .ok_or(Error::Budget("catalog registry entries"))?;
    let hold = creation.map(|owner| owner.hold(slots)).transpose()?;
    if let Some(creation) = creation {
        creation.charge_work(slots)?;
    }
    let mut out = Vec::new();
    out.try_reserve_exact(source.len())
        .map_err(|_| Error::Budget("catalog registry entries"))?;
    for entry in source {
        let id = text(entry, key)?;
        max_id_bytes = max_id_bytes.max(id.len());
        if let Some(creation) = creation {
            creation.charge_work(id.len())?;
        }
        out.push((id, entry));
    }
    let sort_levels = usize::BITS as usize - source.len().max(1).leading_zeros() as usize;
    let sort_work = source
        .len()
        .checked_mul(sort_levels)
        .and_then(|n| n.checked_mul(max_id_bytes))
        .ok_or(Error::Budget("catalog registry sort work"))?;
    if let Some(creation) = creation {
        creation.charge_work(sort_work)?;
    }
    out.sort_unstable_by(|left, right| left.0.cmp(right.0));
    if out.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::Invalid("duplicate catalog registry ID"));
    }
    Ok(RegistryEntries {
        entries: out,
        _hold: hold,
        walk_bytes,
        parent_edges,
    })
}

fn type_is_a(
    id: &str,
    allowed: &[String],
    entries: &RegistryEntries<'_, '_, '_>,
    creation: Option<&CreationState<'_>>,
) -> Result<bool> {
    let _walk_hold = creation
        .map(|owner| owner.hold(entries.walk_bytes))
        .transpose()?;
    let mut todo = Vec::new();
    todo.try_reserve_exact(entries.parent_edges.saturating_add(1))
        .map_err(|_| Error::Budget("catalog registry walk"))?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(entries.len().saturating_add(1))
        .map_err(|_| Error::Budget("catalog registry walk"))?;
    todo.push(id);
    while let Some(item) = todo.pop() {
        if let Some(creation) = creation {
            let levels = usize::BITS as usize - entries.len().max(1).leading_zeros() as usize;
            creation.charge_work(
                item.len()
                    .checked_mul(
                        seen.len()
                            .saturating_add(allowed.len())
                            .saturating_add(levels)
                            .saturating_add(1),
                    )
                    .ok_or(Error::Budget("catalog registry work"))?,
            )?;
        }
        if allowed.iter().any(|x| x.as_str() == item) {
            return Ok(true);
        }
        if seen.contains(&item) {
            continue;
        }
        seen.push(item);
        if let Some(parents) = entries
            .get(item)
            .and_then(|e| e.get("parent_type_ids"))
            .and_then(Value::as_array)
        {
            if let Some(creation) = creation {
                creation.charge_work(
                    parents
                        .len()
                        .checked_mul(std::mem::size_of::<Value>())
                        .ok_or(Error::Budget("catalog registry walk work"))?,
                )?;
            }
            todo.extend(parents.iter().filter_map(Value::as_str));
        }
    }
    Ok(false)
}

fn count(db: &Connection, kind: &str, metric: &str, a: &str, b: &str, c: &str) -> Result<()> {
    db.execute(
        "INSERT INTO temp.cmp_catalog_counts(kind,metric,a,b,c,n) VALUES(?1,?2,?3,?4,?5,1)
        ON CONFLICT(kind,metric,a,b,c) DO UPDATE SET n=n+1",
        params![kind, metric, a, b, c],
    )?;
    Ok(())
}

fn post(
    db: &Connection,
    kind: &str,
    metric: &str,
    field: &str,
    value: &Value,
    order: i64,
    position: usize,
    creation: Option<&CreationState<'_>>,
) -> Result<()> {
    if let Some(creation) = creation {
        return creation.with_json_encoded(value, creation.remaining(0)?, |bytes| {
            creation.charge_work(bytes.len())?;
            let encoded =
                std::str::from_utf8(bytes).map_err(|_| Error::Invalid("catalog post JSON"))?;
            post_encoded(db, kind, metric, field, encoded, order, position)
        });
    }
    let encoded = serde_json::to_string(value).map_err(|_| Error::Invalid("catalog post JSON"))?;
    post_encoded(db, kind, metric, field, &encoded, order, position)
}

fn post_encoded(
    db: &Connection,
    kind: &str,
    metric: &str,
    field: &str,
    encoded: &str,
    order: i64,
    position: usize,
) -> Result<()> {
    db.execute("INSERT INTO temp.cmp_catalog_posts(kind,metric,field,value,source_order,position)
        VALUES(?1,?2,?3,?4,?5,?6)
        ON CONFLICT(kind,metric,field,value) DO UPDATE SET
        source_order=MIN(source_order,excluded.source_order),
        position=CASE WHEN excluded.source_order<source_order THEN excluded.position ELSE position END",
        params![kind,metric,field,encoded,order,position as i64])?;
    Ok(())
}

fn first_values(
    db: &Connection,
    kind: &str,
    metric: &str,
    field: &str,
    limit: usize,
    budget: &ByteBudget,
    creation: Option<&CreationState<'_>>,
) -> Result<Vec<Value>> {
    let sql_limit = i64::try_from(limit).map_err(|_| Error::Budget("catalog SQL limit"))?;
    let available: i64 = db.query_row(
        "SELECT count(*) FROM (SELECT 1 FROM temp.cmp_catalog_posts
         WHERE kind=?1 AND metric=?2 AND field=?3 ORDER BY source_order,position LIMIT ?4)",
        params![kind, metric, field, sql_limit],
        |row| row.get(0),
    )?;
    if available < 0 {
        return Err(Error::Budget("catalog SQL limit"));
    }
    let available = usize::try_from(available).map_err(|_| Error::Budget("catalog SQL limit"))?;
    let mut stmt = db.prepare(
        "SELECT value,length(CAST(value AS BLOB)) FROM temp.cmp_catalog_posts
        WHERE kind=?1 AND metric=?2 AND field=?3 ORDER BY source_order,position LIMIT ?4",
    )?;
    let mut rows = stmt.query(params![kind, metric, field, sql_limit])?;
    let mut out = owned_vec(available, creation)?;
    while let Some(row) = rows.next()? {
        let raw_len: i64 = row.get(1)?;
        if raw_len < 0 {
            return Err(Error::Budget("catalog stored post bytes"));
        }
        let raw_bytes =
            usize::try_from(raw_len).map_err(|_| Error::Budget("catalog stored post bytes"))?;
        let _raw_hold = creation
            .map(|owner| {
                owner.hold(
                    raw_bytes
                        .checked_add(std::mem::size_of::<String>() + 32)
                        .ok_or(Error::Budget("catalog stored post bytes"))?,
                )
            })
            .transpose()?;
        budget.charge(raw_bytes.saturating_add(32))?;
        if let Some(owner) = creation {
            owner.charge_work(raw_bytes)?;
        }
        let raw: String = row.get(0)?;
        let value = if let Some(owner) = creation {
            let limits = JsonLimits::new(raw.len(), 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("catalog post JSON limits"))?;
            owner.serde_owned_with_limits(raw.as_bytes(), limits)?
        } else {
            serde_json::from_str(&raw).map_err(|_| Error::Invalid("catalog stored post JSON"))?
        };
        out.push(value);
    }
    Ok(out)
}

struct FoldedFacet<'state, 'budget> {
    folded: String,
    value: String,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
pub(crate) fn python_casefold_with_state<'state, 'budget>(
    value: &str,
    creation: &'state CreationState<'budget>,
    max_output_bytes: usize,
) -> Result<(String, CreationStateHold<'state, 'budget>)> {
    let error_floor = tos_foundation::python_casefold_unicode16_v1_error_state_upper_bound();
    let floor_hold = creation.hold(error_floor)?;
    let fixed = tos_foundation::python_casefold_unicode16_v1_fixed_state_upper_bound();
    let workspace_hold = creation.hold(fixed)?;
    let result = {
        let mut check = || {
            creation.charge_work(1).map_err(|_| {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "catalog facet casefold cutoff",
                )
            })
        };
        let _check_hold = creation.hold(std::mem::size_of_val(&check))?;
        let available = creation
            .remaining(0)?
            .checked_add(fixed)
            .ok_or(Error::Budget("catalog facet casefold workspace"))?;
        tos_foundation::python_casefold_unicode16_v1_with_state_budget_and_check(
            value,
            value.len(),
            max_output_bytes,
            max_output_bytes,
            available,
            &mut check,
        )
    };
    let folded = result.map_err(|_| Error::Budget("catalog facet casefold bytes"))?;
    let output_hold = creation.hold(folded.capacity())?;
    drop(workspace_hold);
    drop(floor_hold);
    Ok((folded, output_hold))
}

fn python_casefold<'state, 'budget>(
    value: String,
    budget: &ByteBudget,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<FoldedFacet<'state, 'budget>> {
    let output_cap = value.len().saturating_mul(3).max(1);
    let (folded, hold) = if let Some(owner) = creation {
        let (folded, hold) = python_casefold_with_state(&value, owner, output_cap)?;
        (folded, Some(hold))
    } else {
        let input_points = value.chars().count();
        (
            tos_foundation::python_casefold_unicode16_v1(
                &value,
                input_points,
                output_cap,
                output_cap,
            )
            .map_err(|_| Error::Budget("catalog facet casefold bytes"))?,
            None,
        )
    };
    budget.charge(folded.len())?;
    Ok(FoldedFacet {
        folded,
        value,
        _hold: hold,
    })
}

struct FacetNames<'source, 'state, 'budget> {
    names: Vec<&'source str>,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
fn facet_names<'source, 'state, 'budget>(
    vocab: &'source Value,
    kind: &str,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<FacetNames<'source, 'state, 'budget>> {
    let fields = array(
        vocab
            .get("catalog")
            .ok_or(Error::Invalid("catalog vocabulary missing"))?,
        "facets",
    )?;
    let bytes = crate::knowledge_normalization::serde_array_slots_upper(fields.len())?;
    let hold = creation.map(|owner| owner.hold(bytes)).transpose()?;
    if let Some(owner) = creation {
        owner.charge_work(bytes)?;
    }
    let mut names = Vec::new();
    names
        .try_reserve_exact(fields.len())
        .map_err(|_| Error::Budget("catalog facets"))?;
    for field in fields {
        let name = field
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or(Error::Invalid("catalog facet field"))?;
        names.push(if kind == RELATION {
            match name {
                "kind_id" => "predicate_id",
                "type_id" => "relation_type_id",
                "type_mapping.status" => "predicate_mapping.status",
                _ => name,
            }
        } else {
            name
        });
    }
    Ok(FacetNames { names, _hold: hold })
}

fn attribute_allowed(field: &str) -> bool {
    let Some(suffix) = field.strip_prefix("attributes.") else {
        return false;
    };
    let bytes = suffix.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'_' | b'.' | b'-'))
        && !suffix
            .split('.')
            .any(|part| matches!(part, "__proto__" | "prototype" | "constructor"))
}

fn form_key(key: &str) -> bool {
    if matches!(key, "default" | "original") {
        return true;
    }
    let mut parts = key.split('-');
    let first = parts.next().unwrap_or("");
    if first.eq_ignore_ascii_case("i") || first.eq_ignore_ascii_case("x") {
        let mut seen = false;
        for part in parts {
            seen = true;
            if part.is_empty() || part.len() > 8 || !part.bytes().all(|b| b.is_ascii_alphanumeric())
            {
                return false;
            }
        }
        return seen;
    }
    (2..=8).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_alphabetic())
        && parts
            .all(|p| !p.is_empty() && p.len() <= 8 && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn surface(
    db: &Connection,
    kind: &str,
    item: &Value,
    order: i64,
    facets: &[&str],
    creation: Option<&CreationState<'_>>,
) -> Result<()> {
    fn attributes<'state, 'budget>(
        db: &Connection,
        kind: &str,
        at: &Value,
        prefix: &str,
        order: i64,
        creation: Option<&'state CreationState<'budget>>,
    ) -> Result<()> {
        let Some(obj) = at.as_object() else {
            return Ok(());
        };
        if let Some(owner) = creation {
            owner.charge_work(
                obj.len()
                    .checked_mul(std::mem::size_of::<(String, Value)>())
                    .ok_or(Error::Budget("catalog attribute iteration work"))?,
            )?;
        }
        for (key, value) in obj {
            let field_len = prefix
                .len()
                .checked_add(key.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Error::Budget("catalog attribute field"))?;
            let field_hold = creation
                .map(|owner| owner.hold(field_len + std::mem::size_of::<String>()))
                .transpose()?;
            if let Some(owner) = creation {
                owner.charge_work(field_len)?;
            }
            let field = format!("{prefix}.{key}");
            if !attribute_allowed(&field) {
                continue;
            }
            count(db, kind, "attribute", &field, "", "")?;
            count(
                db,
                kind,
                "attribute-value-type",
                &field,
                value_kind(value),
                "",
            )?;
            if let Some(source) = item_source(db, kind, order, creation)? {
                count(db, kind, "attribute-source", &field, &source.value, "")?;
            }
            let members: &[Value] = value
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_else(|| std::slice::from_ref(value));
            if let Some(owner) = creation {
                owner.charge_work(
                    members
                        .len()
                        .checked_mul(std::mem::size_of::<Value>())
                        .ok_or(Error::Budget("catalog attribute iteration work"))?,
                )?;
            }
            let example_bytes = 5usize
                .checked_mul(std::mem::size_of::<(
                    String,
                    Option<CreationStateHold<'state, 'budget>>,
                )>())
                .ok_or(Error::Budget("catalog attribute examples"))?;
            let example_hold = creation
                .map(|owner| owner.hold(example_bytes))
                .transpose()?;
            if let Some(owner) = creation {
                owner.charge_work(example_bytes)?;
            }
            let mut examples: Vec<(String, Option<CreationStateHold<'state, 'budget>>)> =
                Vec::new();
            examples
                .try_reserve_exact(5)
                .map_err(|_| Error::Budget("catalog attribute examples"))?;
            for (position, candidate) in members.iter().enumerate() {
                if value.is_array() {
                    count(
                        db,
                        kind,
                        "attribute-array-type",
                        &field,
                        value_kind(candidate),
                        "",
                    )?;
                }
                if candidate.is_null()
                    || candidate.is_array()
                    || candidate.is_object()
                    || examples.len() == 5
                {
                    continue;
                }
                if let Some(owner) = creation {
                    owner.with_json_encoded(candidate, owner.remaining(0)?, |bytes| {
                        owner.charge_work(bytes.len())?;
                        let serialized = std::str::from_utf8(bytes)
                            .map_err(|_| Error::Invalid("attribute example"))?;
                        let compare_work = serialized
                            .len()
                            .checked_mul(
                                examples
                                    .len()
                                    .checked_add(1)
                                    .ok_or(Error::Budget("attribute example work"))?,
                            )
                            .ok_or(Error::Budget("attribute example work"))?;
                        owner.charge_work(compare_work)?;
                        if serialized.chars().count() <= 180
                            && !examples.iter().any(|(seen, _)| seen == serialized)
                        {
                            let hold =
                                owner.hold(serialized.len() + std::mem::size_of::<String>())?;
                            examples.push((serialized.to_owned(), Some(hold)));
                            post_encoded(
                                db, kind, "examples", &field, serialized, order, position,
                            )?;
                        }
                        Ok(())
                    })?;
                } else {
                    let serialized = serde_json::to_string(candidate)
                        .map_err(|_| Error::Invalid("attribute example"))?;
                    if serialized.chars().count() <= 180
                        && !examples.iter().any(|(seen, _)| seen == &serialized)
                    {
                        examples.push((serialized.clone(), None));
                        post_encoded(db, kind, "examples", &field, &serialized, order, position)?;
                    }
                }
            }
            if value.is_object() {
                attributes(db, kind, value, &field, order, creation)?;
            }
            drop(example_hold);
            drop(field_hold);
        }
        Ok(())
    }
    attributes(
        db,
        kind,
        item.get("attributes").unwrap_or(&Value::Null),
        "attributes",
        order,
        creation,
    )?;
    if let Some(display) = item.get("display").and_then(Value::as_object) {
        if let Some(owner) = creation {
            owner.charge_work(
                display
                    .len()
                    .checked_mul(std::mem::size_of::<(String, Value)>())
                    .ok_or(Error::Budget("catalog display iteration work"))?,
            )?;
        }
        for (field, forms) in display {
            let allowed = if kind == NODE {
                matches!(field.as_str(), "title" | "kind_label" | "summary")
            } else {
                matches!(
                    field.as_str(),
                    "label" | "inverse_label" | "statement" | "explanation"
                )
            };
            if !allowed {
                continue;
            }
            if let Some(forms) = forms.as_object() {
                if let Some(owner) = creation {
                    owner.charge_work(
                        forms
                            .len()
                            .checked_mul(std::mem::size_of::<(String, Value)>())
                            .ok_or(Error::Budget("catalog display forms work"))?,
                    )?;
                }
                for (language, value) in forms {
                    if form_key(language) && value.as_str().is_some_and(|s| !s.is_empty()) {
                        let key_len = "display."
                            .len()
                            .checked_add(field.len())
                            .and_then(|n| n.checked_add(language.len()))
                            .and_then(|n| n.checked_add(1))
                            .ok_or(Error::Budget("catalog display field"))?;
                        let key_hold = creation
                            .map(|owner| owner.hold(key_len + std::mem::size_of::<String>()))
                            .transpose()?;
                        if let Some(owner) = creation {
                            owner.charge_work(key_len)?;
                        }
                        count(
                            db,
                            kind,
                            "display",
                            &format!("display.{field}.{language}"),
                            "",
                            "",
                        )?;
                        drop(key_hold);
                    }
                }
            }
        }
    }
    for &field in facets {
        let raw = path(item, field).unwrap_or(&Value::Null);
        let values: &[Value] = raw
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_else(|| std::slice::from_ref(raw));
        if let Some(owner) = creation {
            owner.charge_work(
                values
                    .len()
                    .checked_mul(std::mem::size_of::<Value>())
                    .ok_or(Error::Budget("catalog facet iteration work"))?,
            )?;
        }
        for (position, value) in values.iter().enumerate() {
            if value.is_null() {
                continue;
            }
            let OwnedText {
                value: s,
                _hold: string_hold,
            } = py_string_owned(value, creation)?;
            if s.is_empty() {
                continue;
            }
            count(db, kind, "facet", field, &s, "")?;
            let facet_value = Value::String(s);
            post(
                db,
                kind,
                "facet-order",
                field,
                &facet_value,
                order,
                position,
                creation,
            )?;
            drop(string_hold);
        }
    }
    Ok(())
}

fn item_source<'state, 'budget>(
    db: &Connection,
    kind: &str,
    order: i64,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<Option<OwnedText<'state, 'budget>>> {
    let table = if kind == NODE {
        "knowledge_nodes"
    } else {
        "knowledge_relations"
    };
    let sql = if kind == NODE {
        "SELECT source_graph,length(CAST(source_graph AS BLOB)) FROM knowledge_nodes WHERE source_order=?1"
    } else {
        "SELECT source_graph,length(CAST(source_graph AS BLOB)) FROM knowledge_relations WHERE source_order=?1"
    };
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.query([order])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let raw_len: i64 = row.get(1)?;
    if raw_len < 0 {
        return Err(Error::Budget("catalog source text bytes"));
    }
    let raw_len =
        usize::try_from(raw_len).map_err(|_| Error::Budget("catalog source text bytes"))?;
    let hold = creation
        .map(|owner| {
            owner.hold(
                raw_len
                    .checked_add(std::mem::size_of::<String>())
                    .ok_or(Error::Budget("catalog source text bytes"))?,
            )
        })
        .transpose()?;
    if let Some(owner) = creation {
        owner.charge_work(raw_len)?;
    }
    let value: String = row.get(0)?;
    if value.len() != raw_len {
        return Err(Error::Budget("catalog source text bytes"));
    }
    Ok(Some(OwnedText { value, _hold: hold }))
}

fn routes(vocab: &Value) -> Result<Vec<Route>> {
    let raw = array(
        vocab
            .get("overview")
            .ok_or(Error::Invalid("overview vocabulary"))?,
        "routes",
    )?;
    let mut out = Vec::new();
    out.try_reserve_exact(raw.len())
        .map_err(|_| Error::Budget("catalog routes"))?;
    for route in raw {
        out.push(Route {
            id: text(route, "route_id")?.into(),
            kinds: ordered_strings(route, "candidate_kind_ids")?,
            predicates: ordered_strings(route, "confirming_predicate_ids")?,
            types: ordered_strings(route, "candidate_type_ids")?,
            relation_types: ordered_strings(route, "confirming_relation_type_ids")?,
        });
    }
    Ok(out)
}

fn stage(db: &Connection, limits: CatalogLimits) -> Result<()> {
    if limits.max_rows == 0
        || limits.max_row_bytes == 0
        || limits.max_catalog_bytes == 0
        || limits.max_catalog_entries == 0
        || limits.max_staging_pages == 0
    {
        return Err(Error::Budget("catalog limits"));
    }
    let existing: u64 = db.query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))?;
    let ceiling = existing.min(limits.max_staging_pages);
    if ceiling == 0 {
        return Err(Error::Budget("catalog staging pages"));
    }
    let actual: u64 = db.query_row(&format!("PRAGMA temp.max_page_count={ceiling}"), [], |r| {
        r.get(0)
    })?;
    if actual > ceiling {
        return Err(Error::Budget("catalog staging page ceiling"));
    }
    db.execute_batch("CREATE TEMP TABLE cmp_catalog_counts(kind TEXT,metric TEXT,a TEXT,b TEXT,c TEXT,n INTEGER NOT NULL,
        PRIMARY KEY(kind,metric,a,b,c)) WITHOUT ROWID;
        CREATE TEMP TABLE cmp_catalog_posts(kind TEXT,metric TEXT,field TEXT,value TEXT,source_order INTEGER,position INTEGER,
        PRIMARY KEY(kind,metric,field,value)) WITHOUT ROWID;
        CREATE INDEX cmp_catalog_posts_first ON cmp_catalog_posts(kind,metric,field,source_order,position);
        CREATE TEMP TABLE cmp_catalog_node_routes(id TEXT PRIMARY KEY,legacy TEXT NOT NULL,typed TEXT NOT NULL) WITHOUT ROWID;")?;
    Ok(())
}

fn ensure_row(
    item: &Value,
    id: &str,
    source: &str,
    raw: &[u8],
    stored_len: i64,
    stored_sha: &[u8],
    limits: CatalogLimits,
) -> Result<()> {
    if raw.len() > limits.max_row_bytes || stored_len < 0 || raw.len() != stored_len as usize {
        return Err(Error::Budget("catalog row bytes"));
    }
    if stored_sha != &Digest256::of_bytes(raw).as_bytes()[..] {
        return Err(Error::Invalid("catalog row digest"));
    }
    if text(item, "id")? != id || text(item, "source_graph")? != source {
        return Err(Error::Invalid("catalog row identity"));
    }
    if !item.get("source_refs").is_some_and(Value::is_array) {
        return Err(Error::Invalid("catalog row source refs"));
    }
    Ok(())
}

fn with_catalog_item<T>(
    raw: &[u8],
    max_bytes: usize,
    invalid: &'static str,
    creation: Option<&CreationState<'_>>,
    operation: impl FnOnce(&Value) -> Result<T>,
) -> Result<T> {
    if let Some(creation) = creation {
        let limits = JsonLimits::new(max_bytes, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("catalog row JSON limits"))?;
        creation.with_serde_owned_with_limits(raw, limits, operation)
    } else {
        let item: Value = serde_json::from_slice(raw).map_err(|_| Error::Invalid(invalid))?;
        operation(&item)
    }
}

fn valid_order(
    previous: &mut Option<(String, String, i64)>,
    source: &str,
    id: &str,
    order: i64,
) -> Result<()> {
    if order < 0
        || previous
            .as_ref()
            .is_some_and(|(s, i, n)| (source, id) <= (s.as_str(), i.as_str()) || order <= *n)
    {
        return Err(Error::Invalid("catalog source order"));
    }
    *previous = Some((source.into(), id.into(), order));
    Ok(())
}

fn ingest_nodes(
    db: &Connection,
    vocab: &Value,
    entity_entries: &RegistryEntries<'_, '_, '_>,
    fallback_type: &str,
    route_defs: &[Route],
    limits: CatalogLimits,
    creation: Option<&CreationState<'_>>,
) -> Result<u64> {
    let facets = facet_names(vocab, NODE, creation)?;
    let mut stmt = db.prepare(
        "SELECT id,source_graph,kind_id,type_id,source_order,length(payload),payload,payload_len,payload_sha256,
            length(CAST(id AS BLOB)),length(CAST(source_graph AS BLOB)),length(CAST(kind_id AS BLOB)),length(CAST(type_id AS BLOB)),length(payload_sha256)
        FROM knowledge_nodes ORDER BY source_order",
    )?;
    let mut rows = stmt.query([])?;
    let registered = array(vocab, "sources")?;
    for source in registered {
        let source_id = text(source, "source_graph_id")?;
        if let Some(creation) = creation {
            creation.charge_work(source_id.len())?;
        }
    }
    let mut prior = None;
    let mut prior_hold = None;
    let mut n = 0u64;
    while let Some(r) = rows.next()? {
        n = n.checked_add(1).ok_or(Error::Budget("catalog rows"))?;
        if n > limits.max_rows {
            return Err(Error::Budget("catalog rows"));
        }
        let actual_len: i64 = r.get(5)?;
        if actual_len < 0 || actual_len as u64 > limits.max_row_bytes as u64 {
            return Err(Error::Budget("catalog row bytes"));
        }
        let field_lengths = [
            r.get::<_, i64>(9)?,
            r.get(10)?,
            r.get(11)?,
            r.get(12)?,
            r.get(13)?,
        ];
        if field_lengths.iter().any(|length| *length < 0) {
            return Err(Error::Budget("catalog row metadata bytes"));
        }
        let payload_bytes =
            usize::try_from(actual_len).map_err(|_| Error::Budget("catalog row allocation"))?;
        let row_bytes = field_lengths
            .iter()
            .try_fold(
                std::mem::size_of::<(String, String, String, String, Vec<u8>)>()
                    .checked_add(payload_bytes)
                    .ok_or(Error::Budget("catalog row allocation"))?,
                |sum, length| {
                    sum.checked_add(*length as usize)
                        .ok_or(Error::Budget("catalog row allocation"))
                },
            )
            .and_then(|bytes| {
                bytes
                    .checked_add(128)
                    .ok_or(Error::Budget("catalog row allocation"))
            })?;
        let _row_hold = creation.map(|owner| owner.hold(row_bytes)).transpose()?;
        if let Some(creation) = creation {
            creation.charge_work(row_bytes)?;
        }
        let (id, source, kind, type_id, order, raw, len, sha): (
            String,
            String,
            String,
            String,
            i64,
            Vec<u8>,
            i64,
            Vec<u8>,
        ) = (
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(6)?,
            r.get(7)?,
            r.get(8)?,
        );
        let next_order_hold = creation
            .map(|owner| {
                owner.hold(
                    id.len()
                        .checked_add(source.len())
                        .and_then(|n| n.checked_add(64))
                        .ok_or(Error::Budget("catalog source order allocation"))?,
                )
            })
            .transpose()?;
        valid_order(&mut prior, &source, &id, order)?;
        prior_hold = next_order_hold;
        if let Some(owner) = creation {
            let membership_work = registered
                .len()
                .checked_mul(
                    source
                        .len()
                        .checked_add(std::mem::size_of::<&Value>() + 8)
                        .ok_or(Error::Budget("catalog source lookup work"))?,
                )
                .ok_or(Error::Budget("catalog source lookup work"))?;
            owner.charge_work(membership_work)?;
        }
        let is_registered = registered.iter().any(|entry| {
            entry.get("source_graph_id").and_then(Value::as_str) == Some(source.as_str())
        });
        if !is_registered {
            return Err(Error::Invalid("unregistered catalog node source"));
        }
        with_catalog_item(
            &raw,
            limits.max_row_bytes,
            "catalog node JSON",
            creation,
            |item| {
                ensure_row(item, &id, &source, &raw, len, &sha, limits)?;
                if text(item, "kind_id")? != kind || text(item, "type_id")? != type_id {
                    return Err(Error::Invalid("catalog node columns"));
                }
                if !entity_entries.contains_key(type_id.as_str()) && type_id != fallback_type {
                    return Err(Error::Invalid("node type bypasses registry fallback"));
                }
                surface(db, NODE, item, order, &facets.names, creation)?;
                count(db, NODE, "total", "", "", "")?;
                count(db, NODE, "group", &kind, "", "")?;
                count(db, NODE, "type", &type_id, "", "")?;
                count(db, NODE, "group-type", &kind, &type_id, "")?;
                let status = py_string_optional(path(item, "type_mapping.status"), creation)?;
                count(db, NODE, "group-status", &kind, &status.value, "")?;
                if matches!(status.value.as_str(), "mapped" | "unmapped") {
                    count(db, NODE, "mapping", &status.value, "", "")?;
                }
                post(
                    db,
                    NODE,
                    "representative",
                    &kind,
                    path(item, "display.kind_label").ok_or(Error::Invalid("node kind label"))?,
                    order,
                    0,
                    creation,
                )?;
                count(db, NODE, "source", &source, "", "")?;
                let state = py_string_optional(path(item, "display.summary_state"), creation)?;
                count(db, NODE, "summary-state", &state.value, "", "")?;
                if path(item, "display.provenance.source_summary_available")
                    == Some(&Value::Bool(false))
                {
                    count(db, NODE, "without-source", "", "", "")?;
                }
                if let Some(claim_type) = path(item, "semantics.claim.relation_type_id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    count(db, NODE, "claim-type", claim_type, "", "")?;
                }
                let route_hold_bytes = route_defs.iter().try_fold(
                    route_defs
                        .len()
                        .checked_mul(2 * std::mem::size_of::<String>())
                        .ok_or(Error::Budget("catalog route row"))?,
                    |sum, route| {
                        sum.checked_add(
                            route
                                .id
                                .len()
                                .checked_mul(2)
                                .ok_or(Error::Budget("catalog route row"))?,
                        )
                        .ok_or(Error::Budget("catalog route row"))
                    },
                )?;
                let _route_row_hold = creation
                    .map(|owner| owner.hold(route_hold_bytes))
                    .transpose()?;
                if let Some(owner) = creation {
                    owner.charge_work(route_hold_bytes)?;
                }
                let mut legacy = Vec::new();
                legacy
                    .try_reserve_exact(route_defs.len())
                    .map_err(|_| Error::Budget("catalog route row"))?;
                let mut typed = Vec::new();
                typed
                    .try_reserve_exact(route_defs.len())
                    .map_err(|_| Error::Budget("catalog route row"))?;
                for route in route_defs {
                    if route.kinds.iter().any(|k| k == &kind) {
                        legacy.push(route.id.clone());
                    }
                    if !route.types.is_empty()
                        && type_is_a(&type_id, &route.types, entity_entries, creation)?
                    {
                        typed.push(route.id.clone());
                        count(db, NODE, "route-type", &route.id, &type_id, "")?;
                    }
                }
                if let Some(owner) = creation {
                    owner.with_json_encoded(&legacy, owner.remaining(0)?, |legacy_bytes| {
                        owner.charge_work(legacy_bytes.len())?;
                        owner.with_json_encoded(&typed, owner.remaining(0)?, |typed_bytes| {
                            owner.charge_work(typed_bytes.len())?;
                            let legacy = std::str::from_utf8(legacy_bytes)
                                .map_err(|_| Error::Invalid("catalog routes JSON"))?;
                            let typed = std::str::from_utf8(typed_bytes)
                                .map_err(|_| Error::Invalid("catalog routes JSON"))?;
                            db.execute(
                                "INSERT INTO temp.cmp_catalog_node_routes VALUES(?1,?2,?3)",
                                params![id, legacy, typed],
                            )?;
                            Ok(())
                        })
                    })?;
                } else {
                    let legacy_json = serde_json::to_string(&legacy)
                        .map_err(|_| Error::Invalid("catalog routes JSON"))?;
                    let typed_json = serde_json::to_string(&typed)
                        .map_err(|_| Error::Invalid("catalog routes JSON"))?;
                    db.execute(
                        "INSERT INTO temp.cmp_catalog_node_routes VALUES(?1,?2,?3)",
                        params![id, legacy_json, typed_json],
                    )?;
                }
                Ok(())
            },
        )?;
    }
    Ok(n)
}

fn endpoint_route_refs<'source, 'state, 'budget>(
    value: &'source Value,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<(
    Vec<&'source str>,
    Option<CreationStateHold<'state, 'budget>>,
)> {
    let values = value
        .as_array()
        .ok_or(Error::Invalid("catalog endpoint routes"))?;
    let bytes = values
        .len()
        .checked_mul(std::mem::size_of::<&str>())
        .ok_or(Error::Budget("catalog endpoint routes"))?;
    let hold = creation.map(|owner| owner.hold(bytes)).transpose()?;
    if let Some(owner) = creation {
        owner.charge_work(bytes)?;
    }
    let mut out = Vec::new();
    out.try_reserve_exact(values.len())
        .map_err(|_| Error::Budget("catalog endpoint routes"))?;
    for value in values {
        let route = value
            .as_str()
            .ok_or(Error::Invalid("catalog endpoint routes"))?;
        if let Some(owner) = creation {
            owner.charge_work(route.len())?;
        }
        out.push(route);
    }
    Ok((out, hold))
}

fn with_endpoint_routes<T>(
    db: &Connection,
    id: &str,
    creation: Option<&CreationState<'_>>,
    operation: impl FnOnce(&[&str], &[&str]) -> Result<T>,
) -> Result<T> {
    let mut stmt = db.prepare(
        "SELECT legacy,typed,length(CAST(legacy AS BLOB)),length(CAST(typed AS BLOB))
        FROM temp.cmp_catalog_node_routes WHERE id=?1",
    )?;
    let mut rows = stmt.query([id])?;
    let row = rows
        .next()?
        .ok_or(Error::Invalid("catalog relation endpoint absent"))?;
    let legacy_len: i64 = row.get(2)?;
    let typed_len: i64 = row.get(3)?;
    if legacy_len < 0 || typed_len < 0 {
        return Err(Error::Budget("catalog endpoint route bytes"));
    }
    let legacy_len =
        usize::try_from(legacy_len).map_err(|_| Error::Budget("catalog endpoint route bytes"))?;
    let typed_len =
        usize::try_from(typed_len).map_err(|_| Error::Budget("catalog endpoint route bytes"))?;
    let raw_bytes = legacy_len
        .checked_add(typed_len)
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<String>()))
        .ok_or(Error::Budget("catalog endpoint route bytes"))?;
    let _raw_hold = creation.map(|owner| owner.hold(raw_bytes)).transpose()?;
    if let Some(owner) = creation {
        owner.charge_work(raw_bytes)?;
    }
    let legacy: String = row.get(0)?;
    let typed: String = row.get(1)?;
    if legacy.len() != legacy_len || typed.len() != typed_len {
        return Err(Error::Budget("catalog endpoint route bytes"));
    }
    drop(rows);
    drop(stmt);
    match creation {
        Some(owner) => {
            let legacy_limits = JsonLimits::new(legacy.len(), 96, 100_000, 4096)
                .map_err(|_| Error::Budget("catalog endpoint route limits"))?;
            let typed_limits = JsonLimits::new(typed.len(), 96, 100_000, 4096)
                .map_err(|_| Error::Budget("catalog endpoint route limits"))?;
            owner.with_serde_owned_with_limits(legacy.as_bytes(), legacy_limits, |legacy_value| {
                let (legacy_refs, _legacy_hold) = endpoint_route_refs(legacy_value, Some(owner))?;
                owner.with_serde_owned_with_limits(typed.as_bytes(), typed_limits, |typed_value| {
                    let (typed_refs, _typed_hold) = endpoint_route_refs(typed_value, Some(owner))?;
                    operation(&legacy_refs, &typed_refs)
                })
            })
        }
        None => {
            let legacy: Vec<String> = serde_json::from_str(&legacy)
                .map_err(|_| Error::Invalid("catalog endpoint routes"))?;
            let typed: Vec<String> = serde_json::from_str(&typed)
                .map_err(|_| Error::Invalid("catalog endpoint routes"))?;
            let legacy_refs: Vec<&str> = legacy.iter().map(String::as_str).collect();
            let typed_refs: Vec<&str> = typed.iter().map(String::as_str).collect();
            operation(&legacy_refs, &typed_refs)
        }
    }
}

fn ingest_relations(
    db: &Connection,
    vocab: &Value,
    route_defs: &[Route],
    relation_entries: &RegistryEntries<'_, '_, '_>,
    fallback_type: &str,
    cross_source_ids: &[Value],
    limits: CatalogLimits,
    creation: Option<&CreationState<'_>>,
) -> Result<u64> {
    let facets = facet_names(vocab, RELATION, creation)?;
    let mut stmt=db.prepare("SELECT id,source_graph,from_id,to_id,predicate_id,relation_type_id,source_order,length(payload),payload,payload_len,payload_sha256,
        length(CAST(id AS BLOB)),length(CAST(source_graph AS BLOB)),length(CAST(from_id AS BLOB)),length(CAST(to_id AS BLOB)),length(CAST(predicate_id AS BLOB)),length(CAST(relation_type_id AS BLOB)),length(payload_sha256)
        FROM knowledge_relations ORDER BY source_order")?;
    let mut rows = stmt.query([])?;
    let registered = array(vocab, "sources")?;
    let mut prior = None;
    let mut prior_hold = None;
    let mut n = 0u64;
    while let Some(r) = rows.next()? {
        n = n.checked_add(1).ok_or(Error::Budget("catalog rows"))?;
        if n > limits.max_rows {
            return Err(Error::Budget("catalog rows"));
        }
        let actual_len: i64 = r.get(7)?;
        if actual_len < 0 || actual_len as u64 > limits.max_row_bytes as u64 {
            return Err(Error::Budget("catalog row bytes"));
        }
        let field_lengths = [
            r.get::<_, i64>(11)?,
            r.get(12)?,
            r.get(13)?,
            r.get(14)?,
            r.get(15)?,
            r.get(16)?,
        ];
        if field_lengths.iter().any(|length| *length < 0) {
            return Err(Error::Budget("catalog row metadata bytes"));
        }
        let payload_bytes =
            usize::try_from(actual_len).map_err(|_| Error::Budget("catalog row allocation"))?;
        let row_bytes = field_lengths
            .iter()
            .try_fold(
                std::mem::size_of::<(String, String, String, String, String, String, Vec<u8>)>()
                    .checked_add(payload_bytes)
                    .ok_or(Error::Budget("catalog row allocation"))?,
                |sum, length| {
                    sum.checked_add(*length as usize)
                        .ok_or(Error::Budget("catalog row allocation"))
                },
            )
            .and_then(|bytes| {
                bytes
                    .checked_add(128)
                    .ok_or(Error::Budget("catalog row allocation"))
            })?;
        let _row_hold = creation.map(|owner| owner.hold(row_bytes)).transpose()?;
        if let Some(creation) = creation {
            creation.charge_work(row_bytes)?;
        }
        let (id, source, from, to, predicate, type_id, order, raw, len, sha): (
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            Vec<u8>,
            i64,
            Vec<u8>,
        ) = (
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(8)?,
            r.get(9)?,
            r.get(10)?,
        );
        let next_order_hold = creation
            .map(|owner| {
                owner.hold(
                    id.len()
                        .checked_add(source.len())
                        .and_then(|n| n.checked_add(64))
                        .ok_or(Error::Budget("catalog source order allocation"))?,
                )
            })
            .transpose()?;
        valid_order(&mut prior, &source, &id, order)?;
        prior_hold = next_order_hold;
        if let Some(owner) = creation {
            let membership_work = registered
                .len()
                .checked_mul(
                    source
                        .len()
                        .checked_add(std::mem::size_of::<&Value>() + 8)
                        .ok_or(Error::Budget("catalog source lookup work"))?,
                )
                .ok_or(Error::Budget("catalog source lookup work"))?;
            owner.charge_work(membership_work)?;
        }
        let is_registered = registered.iter().any(|entry| {
            entry.get("source_graph_id").and_then(Value::as_str) == Some(source.as_str())
        });
        if !is_registered {
            return Err(Error::Invalid("unregistered catalog relation source"));
        }
        with_catalog_item(
            &raw,
            limits.max_row_bytes,
            "catalog relation JSON",
            creation,
            |item| {
                ensure_row(item, &id, &source, &raw, len, &sha, limits)?;
                for (key, expected) in [
                    ("from_id", &from),
                    ("to_id", &to),
                    ("predicate_id", &predicate),
                    ("relation_type_id", &type_id),
                ] {
                    if text(item, key)? != expected {
                        return Err(Error::Invalid("catalog relation columns"));
                    }
                }
                if !relation_entries.contains_key(type_id.as_str()) && type_id != fallback_type {
                    return Err(Error::Invalid("relation type bypasses registry fallback"));
                }
                surface(db, RELATION, item, order, &facets.names, creation)?;
                count(db, RELATION, "total", "", "", "")?;
                count(db, RELATION, "group", &predicate, "", "")?;
                count(db, RELATION, "type", &type_id, "", "")?;
                count(db, RELATION, "group-type", &predicate, &type_id, "")?;
                let status = py_string_optional(path(item, "predicate_mapping.status"), creation)?;
                count(db, RELATION, "group-status", &predicate, &status.value, "")?;
                if matches!(status.value.as_str(), "mapped" | "unmapped") {
                    count(db, RELATION, "mapping", &status.value, "", "")?;
                }
                post(
                    db,
                    RELATION,
                    "representative",
                    &predicate,
                    path(item, "display.label").ok_or(Error::Invalid("relation label"))?,
                    order,
                    0,
                    creation,
                )?;
                let state = py_string_optional(path(item, "display.explanation_state"), creation)?;
                count(db, RELATION, "explanation-state", &state.value, "", "")?;
                if path(item, "display.provenance.source_explanation_available")
                    == Some(&Value::Bool(false))
                {
                    count(db, RELATION, "without-source", "", "", "")?;
                }
                if let Some(owner) = creation {
                    let cross_source_work = cross_source_ids
                        .len()
                        .checked_mul(
                            source
                                .len()
                                .checked_add(std::mem::size_of::<Value>() + 32)
                                .ok_or(Error::Budget("catalog cross-source lookup work"))?,
                        )
                        .ok_or(Error::Budget("catalog cross-source lookup work"))?;
                    owner.charge_work(cross_source_work)?;
                }
                if cross_source_ids.iter().any(|source_definition| {
                    source_definition.get("input_role").and_then(Value::as_str)
                        == Some("derived-cross-source-join")
                        && source_definition
                            .get("source_graph_id")
                            .and_then(Value::as_str)
                            == Some(source.as_str())
                }) {
                    count(db, RELATION, "cross-layer", "", "", "")?;
                }
                with_endpoint_routes(db, &from, creation, |left_l, left_t| {
                    with_endpoint_routes(db, &to, creation, |right_l, right_t| {
                        if let Some(owner) = creation {
                            owner.charge_work(
                                route_defs
                                    .len()
                                    .checked_mul(std::mem::size_of::<Route>())
                                    .ok_or(Error::Budget("catalog relation route work"))?,
                            )?;
                            let refs = left_l
                                .len()
                                .checked_add(right_l.len())
                                .and_then(|n| n.checked_add(left_t.len()))
                                .and_then(|n| n.checked_add(right_t.len()))
                                .ok_or(Error::Budget("catalog relation route work"))?;
                            let mut route_work = 0usize;
                            for route in route_defs {
                                let predicate_work =
                                    route.predicates.iter().try_fold(0usize, |sum, value| {
                                        sum.checked_add(value.len())
                                            .and_then(|n| n.checked_add(predicate.len()))
                                            .ok_or(Error::Budget("catalog relation route work"))
                                    })?;
                                let relation_type_work = route.relation_types.iter().try_fold(
                                    0usize,
                                    |sum, value| {
                                        sum.checked_add(value.len())
                                            .and_then(|n| n.checked_add(type_id.len()))
                                            .ok_or(Error::Budget("catalog relation route work"))
                                    },
                                )?;
                                let endpoint_work = refs
                                    .checked_mul(
                                        route
                                            .id
                                            .len()
                                            .checked_add(8)
                                            .ok_or(Error::Budget("catalog relation route work"))?,
                                    )
                                    .ok_or(Error::Budget("catalog relation route work"))?;
                                route_work = route_work
                                    .checked_add(predicate_work)
                                    .and_then(|n| n.checked_add(relation_type_work))
                                    .and_then(|n| n.checked_add(endpoint_work))
                                    .ok_or(Error::Budget("catalog relation route work"))?;
                            }
                            owner.charge_work(route_work)?;
                        }
                        for route in route_defs {
                            if route.predicates.iter().any(|v| v == &predicate)
                                && (left_l.contains(&route.id.as_str())
                                    || right_l.contains(&route.id.as_str()))
                            {
                                count(db, RELATION, "route-predicate", &route.id, &predicate, "")?;
                            }
                            if route.relation_types.iter().any(|v| v == &type_id)
                                && (left_t.contains(&route.id.as_str())
                                    || right_t.contains(&route.id.as_str()))
                            {
                                count(db, RELATION, "route-type", &route.id, &type_id, "")?;
                            }
                        }
                        Ok(())
                    })
                })?;
                Ok(())
            },
        )?;
    }
    Ok(n)
}

struct OrderedCounts<'state, 'budget> {
    rows: Vec<(String, u64)>,
    _hold: Option<CreationStateHold<'state, 'budget>>,
}
impl OrderedCounts<'_, '_> {
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    fn get(&self, key: &str) -> Option<&u64> {
        self.rows
            .binary_search_by(|(candidate, _)| candidate.as_str().cmp(key))
            .ok()
            .map(|index| &self.rows[index].1)
    }
    fn keys(&self) -> impl Iterator<Item = &String> {
        self.rows.iter().map(|(key, _)| key)
    }
    fn values(&self) -> impl Iterator<Item = &u64> {
        self.rows.iter().map(|(_, value)| value)
    }
    fn into_json_keys(self, creation: Option<&CreationState<'_>>) -> Result<Value> {
        let Self { rows, _hold } = self;
        let key_bytes = rows.iter().try_fold(0usize, |sum, (key, _)| {
            sum.checked_add(key.len())
                .ok_or(Error::Budget("catalog output count keys"))
        })?;
        owned_bytes(creation, key_bytes)?;
        let mut out = owned_vec(rows.len(), creation)?;
        out.extend(rows.into_iter().map(|(key, _)| Value::String(key)));
        drop(_hold);
        Ok(Value::Array(out))
    }
    fn json_keys(&self, creation: Option<&CreationState<'_>>) -> Result<Value> {
        let key_bytes = self.rows.iter().try_fold(0usize, |sum, (key, _)| {
            sum.checked_add(key.len())
                .ok_or(Error::Budget("catalog output count keys"))
        })?;
        owned_bytes(creation, key_bytes)?;
        let mut out = owned_vec(self.rows.len(), creation)?;
        for (key, _) in &self.rows {
            out.push(Value::String(key.clone()));
        }
        Ok(Value::Array(out))
    }
}

fn counts_query<'state, 'budget, P: rusqlite::Params + Clone>(
    db: &Connection,
    summary_sql: &str,
    rows_sql: &str,
    params: P,
    budget: &ByteBudget,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<OrderedCounts<'state, 'budget>> {
    let (count, key_bytes): (i64, i64) = db.query_row(summary_sql, params.clone(), |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    if count < 0 || key_bytes < 0 {
        return Err(Error::Budget("catalog count rows"));
    }
    let count = usize::try_from(count).map_err(|_| Error::Budget("catalog count rows"))?;
    let key_bytes = usize::try_from(key_bytes).map_err(|_| Error::Budget("catalog count bytes"))?;
    let geometry = count
        .checked_mul(std::mem::size_of::<(String, u64)>() + std::mem::size_of::<usize>() * 4)
        .and_then(|bytes| bytes.checked_add(key_bytes))
        .ok_or(Error::Budget("catalog count geometry"))?;
    let hold = creation.map(|owner| owner.hold(geometry)).transpose()?;
    if let Some(owner) = creation {
        owner.charge_work(geometry)?;
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Error::Budget("catalog count rows"))?;
    let mut stmt = db.prepare(rows_sql)?;
    let mapped = stmt.query_map(params, |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, u64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in mapped {
        let (key, value, len) = row?;
        if len < 0 || len as usize != key.len() {
            return Err(Error::Budget("catalog count key bytes"));
        }
        budget.charge(key.len().saturating_add(32))?;
        rows.push((key, value));
    }
    if rows.len() != count {
        return Err(Error::Invalid("catalog count row drift"));
    }
    Ok(OrderedCounts { rows, _hold: hold })
}

fn top_counts<'state, 'budget>(
    db: &Connection,
    kind: &str,
    metric: &str,
    budget: &ByteBudget,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<OrderedCounts<'state, 'budget>> {
    counts_query(
        db,
        "SELECT count(*),coalesce(sum(length(CAST(a AS BLOB))),0)
         FROM temp.cmp_catalog_counts WHERE kind=?1 AND metric=?2 AND b='' AND c=''",
        "SELECT a,n,length(CAST(a AS BLOB)) FROM temp.cmp_catalog_counts
         WHERE kind=?1 AND metric=?2 AND b='' AND c='' ORDER BY a",
        params![kind, metric],
        budget,
        creation,
    )
}

fn sub_counts<'state, 'budget>(
    db: &Connection,
    kind: &str,
    metric: &str,
    a: &str,
    budget: &ByteBudget,
    creation: Option<&'state CreationState<'budget>>,
) -> Result<OrderedCounts<'state, 'budget>> {
    counts_query(
        db,
        "SELECT count(*),coalesce(sum(length(CAST(b AS BLOB))),0)
         FROM temp.cmp_catalog_counts WHERE kind=?1 AND metric=?2 AND a=?3 AND c=''",
        "SELECT b,n,length(CAST(b AS BLOB)) FROM temp.cmp_catalog_counts
         WHERE kind=?1 AND metric=?2 AND a=?3 AND c='' ORDER BY b",
        params![kind, metric, a],
        budget,
        creation,
    )
}

fn counts_object(
    mut counts: OrderedCounts<'_, '_>,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let key_bytes = counts.rows.iter().try_fold(0usize, |sum, (key, _)| {
        sum.checked_add(key.len())
            .ok_or(Error::Budget("catalog output count keys"))
    })?;
    owned_object_shape(creation, counts.len(), key_bytes)?;
    let mut out = Map::new();
    for (key, count) in counts.rows.drain(..) {
        out.insert(key, json!(count));
    }
    drop(counts);
    Ok(Value::Object(out))
}

fn field_catalog(
    db: &Connection,
    kind: &str,
    metric: &str,
    budget: &ByteBudget,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let fields = top_counts(db, kind, metric, budget, creation)?;
    let mut out = owned_vec(fields.len(), creation)?;
    for (field, count) in fields.rows.iter() {
        if metric == "display" {
            owned_object(creation, &["field", "available_item_count"])?;
            owned_bytes(creation, field.len())?;
            let mut row = Map::new();
            row.insert("field".into(), Value::String(field.clone()));
            row.insert("available_item_count".into(), json!(count));
            out.push(Value::Object(row));
            continue;
        }
        let value_types = counts_object(
            sub_counts(db, kind, "attribute-value-type", field, budget, creation)?,
            creation,
        )?;
        let array_item_types = counts_object(
            sub_counts(db, kind, "attribute-array-type", field, budget, creation)?,
            creation,
        )?;
        let sources = sub_counts(db, kind, "attribute-source", field, budget, creation)?
            .into_json_keys(creation)?;
        let examples = first_values(db, kind, "examples", field, 5, budget, creation)?;
        owned_object(
            creation,
            &[
                "field",
                "item_count",
                "value_types",
                "array_item_types",
                "sources",
                "examples",
            ],
        )?;
        owned_bytes(creation, field.len())?;
        let mut row = Map::new();
        row.insert("field".into(), Value::String(field.clone()));
        row.insert("item_count".into(), json!(count));
        row.insert("value_types".into(), value_types);
        row.insert("array_item_types".into(), array_item_types);
        row.insert("sources".into(), sources);
        row.insert("examples".into(), Value::Array(examples));
        out.push(Value::Object(row));
    }
    drop(fields);
    Ok(Value::Array(out))
}

fn facets(
    db: &Connection,
    vocab: &Value,
    kind: &str,
    max_entries: u64,
    budget: &ByteBudget,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let fields = facet_names(vocab, kind, creation)?;
    let key_bytes = fields.names.iter().try_fold(0usize, |sum, field| {
        sum.checked_add(field.len())
            .ok_or(Error::Budget("catalog facet key bytes"))
    })?;
    owned_object_shape(creation, fields.names.len(), key_bytes)?;
    let mut result = Map::new();
    for field in fields.names.iter().copied() {
        let item_counts = sub_counts(db, kind, "facet", field, budget, creation)?;
        let limit =
            usize::try_from(max_entries).map_err(|_| Error::Budget("catalog facet limit"))?;
        let mut values = first_values(db, kind, "facet-order", field, limit, budget, creation)?;
        let decoration_bytes = values
            .len()
            .checked_mul(std::mem::size_of::<FoldedFacet<'_, '_>>())
            .ok_or(Error::Budget("catalog facet sort workspace"))?;
        let decoration_hold = creation
            .map(|owner| owner.hold(decoration_bytes))
            .transpose()?;
        if let Some(owner) = creation {
            owner.charge_work(decoration_bytes)?;
        }
        let mut decorated = Vec::new();
        decorated
            .try_reserve_exact(values.len())
            .map_err(|_| Error::Budget("catalog facet sort workspace"))?;
        let mut max_fold_bytes = 0usize;
        for value in values.drain(..) {
            let s = match value {
                Value::String(value) => value,
                _ => return Err(Error::Invalid("catalog facet value")),
            };
            let folded = python_casefold(s, budget, creation)?;
            max_fold_bytes = max_fold_bytes.max(folded.folded.len());
            decorated.push(folded);
        }
        let sort_levels = usize::BITS as usize - decorated.len().max(1).leading_zeros() as usize;
        let sort_work = decorated
            .len()
            .checked_mul(sort_levels)
            .and_then(|n| n.checked_mul(max_fold_bytes))
            .ok_or(Error::Budget("catalog facet sort work"))?;
        let sort_bytes = decorated
            .len()
            .checked_mul(std::mem::size_of::<FoldedFacet<'_, '_>>())
            .ok_or(Error::Budget("catalog facet sort workspace"))?;
        let _sort_hold = creation.map(|owner| owner.hold(sort_bytes)).transpose()?;
        if let Some(owner) = creation {
            owner.charge_work(sort_work)?;
        }
        decorated.sort_by(|a, b| a.folded.cmp(&b.folded));
        drop(_sort_hold);
        let mut rows = owned_vec(decorated.len(), creation)?;
        for folded in decorated {
            let FoldedFacet { value, _hold, .. } = folded;
            let count = item_counts.get(&value).copied().unwrap_or(0);
            owned_object(creation, &["count", "value"])?;
            owned_bytes(creation, value.len())?;
            let mut row = Map::new();
            row.insert("count".into(), json!(count));
            row.insert("value".into(), Value::String(value));
            rows.push(Value::Object(row));
            drop(_hold);
        }
        result.insert(field.to_owned(), Value::Array(rows));
        drop(item_counts);
        drop(decoration_hold);
    }
    Ok(Value::Object(result))
}

fn registry_metadata(
    registry: &Value,
    entries: &RegistryEntries<'_, '_, '_>,
    type_counts: &OrderedCounts<'_, '_>,
    fallback_key: &str,
    kind: &str,
    node_count: u64,
    claim_counts: Option<&OrderedCounts<'_, '_>>,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let fallback = text(registry, fallback_key)?;
    let mut rows = owned_vec(entries.len(), creation)?;
    for (id, entry) in &entries.entries {
        let fields = entry
            .as_object()
            .ok_or(Error::Invalid("registry entry object"))?
            .len();
        let extra = if kind == NODE { 1 } else { 3 };
        let mut object = match owned_value(entry, creation)? {
            Value::Object(object) => object,
            _ => return Err(Error::Invalid("registry entry object")),
        };
        let growth = crate::knowledge_normalization::serde_object_slots_upper(
            fields
                .checked_add(extra)
                .ok_or(Error::Budget("catalog registry metadata"))?,
        )?;
        owned_bytes(
            creation,
            growth
                .checked_add(if kind == NODE { 15 } else { 59 })
                .ok_or(Error::Budget("catalog registry metadata"))?,
        )?;
        let instance = type_counts.get(id).copied().unwrap_or(0);
        if kind == NODE {
            object.insert("instance_count".into(), json!(instance));
        } else {
            let claims = claim_counts.and_then(|m| m.get(id)).copied().unwrap_or(0);
            object.insert("edge_instance_count".into(), json!(instance));
            object.insert("claim_instance_count".into(), json!(claims));
            object.insert("instance_count".into(), json!(instance + claims));
        }
        rows.push(Value::Object(object));
    }
    let mapped = node_count.saturating_sub(type_counts.get(fallback).copied().unwrap_or(0));
    let key_mapped = if kind == NODE {
        "mapped_instance_count"
    } else {
        "mapped_edge_instance_count"
    };
    let key_unmapped = if kind == NODE {
        "unmapped_instance_count"
    } else {
        "unmapped_edge_instance_count"
    };
    owned_object(
        creation,
        &[
            "registry_id",
            "registry_version",
            "source_refs",
            fallback_key,
            key_mapped,
            key_unmapped,
            "entries",
        ],
    )?;
    let mut out = Map::new();
    for key in ["registry_id", "registry_version"] {
        out.insert(
            key.into(),
            match registry.get(key) {
                Some(value) => owned_value(value, creation)?,
                None => Value::Null,
            },
        );
    }
    out.insert(
        "source_refs".into(),
        match registry.get("source_refs") {
            Some(value) => owned_value(value, creation)?,
            None => Value::Array(Vec::new()),
        },
    );
    owned_bytes(creation, fallback.len())?;
    out.insert(fallback_key.into(), Value::String(fallback.into()));
    out.insert(key_mapped.into(), json!(mapped));
    out.insert(
        key_unmapped.into(),
        json!(type_counts.get(fallback).copied().unwrap_or(0)),
    );
    out.insert("entries".into(), Value::Array(rows));
    Ok(Value::Object(out))
}

fn group_catalog(
    db: &Connection,
    kind: &str,
    relation_entries: &RegistryEntries<'_, '_, '_>,
    budget: &ByteBudget,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let groups = top_counts(db, kind, "group", budget, creation)?;
    let mut out = owned_vec(groups.len(), creation)?;
    for (group, n) in &groups.rows {
        let types = sub_counts(db, kind, "group-type", group, budget, creation)?;
        let statuses = sub_counts(db, kind, "group-status", group, budget, creation)?;
        let display = first_values(db, kind, "representative", group, 1, budget, creation)?
            .into_iter()
            .next()
            .ok_or(Error::Invalid("catalog representative absent"))?;
        let type_values = types.json_keys(creation)?;
        let status_values = statuses.into_json_keys(creation)?;
        let id_key = if kind == NODE {
            "kind_id"
        } else {
            "predicate_id"
        };
        let types_key = if kind == NODE {
            "type_ids"
        } else {
            "relation_type_ids"
        };
        let mut object = Map::new();
        if kind == RELATION {
            owned_object(
                creation,
                &[
                    id_key,
                    "display",
                    "count",
                    types_key,
                    "mapping_statuses",
                    "semantic_definitions",
                ],
            )?;
            let mut defs = owned_vec(types.len(), creation)?;
            for type_id in types.keys() {
                if let Some(entry) = relation_entries.get(type_id.as_str()) {
                    owned_object(
                        creation,
                        &[
                            "relation_type_id",
                            "labels",
                            "definition",
                            "domain_type_ids",
                            "range_type_ids",
                        ],
                    )?;
                    let mut definition = Map::new();
                    owned_bytes(creation, type_id.len())?;
                    definition.insert("relation_type_id".into(), Value::String(type_id.clone()));
                    for field in ["labels", "definition", "domain_type_ids", "range_type_ids"] {
                        let value = entry
                            .get(field)
                            .map(|value| owned_value(value, creation))
                            .transpose()?
                            .unwrap_or(Value::Null);
                        definition.insert(field.into(), value);
                    }
                    defs.push(Value::Object(definition));
                }
            }
            owned_bytes(creation, group.len())?;
            object.insert(id_key.into(), Value::String(group.clone()));
            object.insert("display".into(), display);
            object.insert("count".into(), json!(n));
            object.insert(types_key.into(), type_values);
            object.insert("mapping_statuses".into(), status_values);
            object.insert("semantic_definitions".into(), Value::Array(defs));
        } else {
            owned_object(
                creation,
                &[id_key, "display", "count", types_key, "mapping_statuses"],
            )?;
            owned_bytes(creation, group.len())?;
            object.insert(id_key.into(), Value::String(group.clone()));
            object.insert("display".into(), display);
            object.insert("count".into(), json!(n));
            object.insert(types_key.into(), type_values);
            object.insert("mapping_statuses".into(), status_values);
        }
        out.push(Value::Object(object));
        drop(types);
        drop(statuses);
    }
    drop(groups);
    Ok(Value::Array(out))
}

fn route_catalog(
    db: &Connection,
    route_defs: &[Route],
    entity_entries: &RegistryEntries<'_, '_, '_>,
    budget: &ByteBudget,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let kinds = top_counts(db, NODE, "group", budget, creation)?;
    let mut out = owned_vec(route_defs.len(), creation)?;
    for route in route_defs {
        let mut available_kinds = owned_vec(route.kinds.len(), creation)?;
        let mut available_node_count = 0u64;
        for kind in &route.kinds {
            let count = kinds.get(kind.as_str()).copied().unwrap_or(0);
            if count > 0 {
                available_node_count = available_node_count
                    .checked_add(count)
                    .ok_or(Error::Budget("catalog route node count"))?;
                available_kinds.push(owned_string(kind, creation)?);
            }
        }
        let typed = sub_counts(db, NODE, "route-type", &route.id, budget, creation)?;
        let legacy = sub_counts(db, RELATION, "route-predicate", &route.id, budget, creation)?;
        let typed_relations = sub_counts(db, RELATION, "route-type", &route.id, budget, creation)?;
        let semantic_mode = !entity_entries.is_empty() && !route.types.is_empty();
        let available = if semantic_mode {
            !typed.is_empty()
        } else {
            !available_kinds.is_empty()
        };
        let availability = if available {
            "available"
        } else {
            "not_projected"
        };
        let has_confirmation = if semantic_mode {
            !typed_relations.is_empty()
        } else {
            !legacy.is_empty()
        };
        let expects = if semantic_mode {
            !route.relation_types.is_empty()
        } else {
            !route.predicates.is_empty()
        };
        let readiness = if !available {
            "not_projected"
        } else if expects && !has_confirmation {
            "kind_only"
        } else {
            "confirmed"
        };
        let note = if !available {
            "No trustworthy nodes of these kinds are projected; the backend will not synthesize them from unrelated predicates."
        } else if readiness == "kind_only" {
            "Kinds are projected, but no confirming relation currently establishes the requested contextual role."
        } else if !route.predicates.is_empty() {
            "Kinds select candidate entities; predicates establish contextual roles such as authorship."
        } else {
            "Kinds are source-derived candidates and retain their exact kind_id on every node."
        };
        let node_count: u64 = if semantic_mode {
            typed.values().sum()
        } else {
            available_node_count
        };
        let mut object = Map::new();
        owned_object(
            creation,
            &[
                "route_id",
                "candidate_kind_ids",
                "available_kind_ids",
                "confirming_predicate_ids",
                "available_confirming_predicate_ids",
                "confirming_relation_count",
                "candidate_type_ids",
                "available_type_ids",
                "confirming_relation_type_ids",
                "available_confirming_relation_type_ids",
                "semantic_confirming_relation_count",
                "node_count",
                "availability",
                "role_readiness",
                "note",
            ],
        )?;
        object.insert("route_id".into(), owned_string(&route.id, creation)?);
        object.insert(
            "candidate_kind_ids".into(),
            owned_string_values(&route.kinds, creation)?,
        );
        object.insert("available_kind_ids".into(), Value::Array(available_kinds));
        object.insert(
            "confirming_predicate_ids".into(),
            owned_string_values(&route.predicates, creation)?,
        );
        object.insert(
            "available_confirming_predicate_ids".into(),
            legacy.json_keys(creation)?,
        );
        object.insert(
            "confirming_relation_count".into(),
            json!(legacy.values().sum::<u64>()),
        );
        object.insert(
            "candidate_type_ids".into(),
            owned_string_values(&route.types, creation)?,
        );
        object.insert("available_type_ids".into(), typed.json_keys(creation)?);
        object.insert(
            "confirming_relation_type_ids".into(),
            owned_string_values(&route.relation_types, creation)?,
        );
        object.insert(
            "available_confirming_relation_type_ids".into(),
            typed_relations.json_keys(creation)?,
        );
        object.insert(
            "semantic_confirming_relation_count".into(),
            json!(typed_relations.values().sum::<u64>()),
        );
        object.insert("node_count".into(), json!(node_count));
        object.insert("availability".into(), owned_string(availability, creation)?);
        object.insert("role_readiness".into(), owned_string(readiness, creation)?);
        object.insert("note".into(), owned_string(note, creation)?);
        out.push(Value::Object(object));
        drop(typed);
        drop(legacy);
        drop(typed_relations);
    }
    drop(kinds);
    Ok(Value::Array(out))
}

fn presentation(entity_registry: &Value, creation: Option<&CreationState<'_>>) -> Result<Value> {
    let Some(value) = entity_registry.get("context_presentation") else {
        return Ok(Value::Null);
    };
    let id = text(value, "presentation_id")?;
    let version = owned_value(
        value
            .get("presentation_version")
            .ok_or(Error::Invalid("presentation version"))?,
        creation,
    )?;
    let payload = owned_value(value, creation)?;
    owned_object(
        creation,
        &["id", "version", "source_ref", "digest", "payload"],
    )?;
    let source_ref = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
    owned_bytes(creation, id.len() + source_ref.len())?;
    let digest_hex = if let Some(owner) = creation {
        owner.with_json_encoded(value, owner.remaining(0)?, |bytes| {
            let limits = JsonLimits::default();
            owner.with_foundation_owned_with_limits(bytes, limits, |document| {
                owner.with_foundation_canonical_bytes(document, limits, |canonical| {
                    owner.charge_work(canonical.len())?;
                    let digest = Digest256::of_bytes(canonical);
                    owner.retain(71)?;
                    let mut text = String::with_capacity(71);
                    text.push_str("sha256:");
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    for byte in digest.as_bytes() {
                        text.push(HEX[(byte >> 4) as usize] as char);
                        text.push(HEX[(byte & 0x0f) as usize] as char);
                    }
                    Ok(text)
                })
            })
        })?
    } else {
        let encoded = crate::prepared_catalog_semantics::encoded(value)?;
        format!(
            "sha256:{}",
            Digest256::of_bytes(encoded.as_bytes()).to_hex()
        )
    };
    let mut out = Map::new();
    out.insert("id".into(), Value::String(id.into()));
    out.insert("version".into(), version);
    out.insert("source_ref".into(), Value::String(source_ref.into()));
    out.insert("digest".into(), Value::String(digest_hex));
    out.insert("payload".into(), payload);
    Ok(Value::Object(out))
}

fn capabilities(
    db: &Connection,
    vocab: &Value,
    max_entries: u64,
    budget: &ByteBudget,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let overview = vocab
        .get("overview")
        .ok_or(Error::Invalid("catalog overview"))?;
    let filters = vocab
        .get("filters")
        .ok_or(Error::Invalid("catalog filters"))?;
    const STATIC: &str = r#"{"execution_version":"tos-lens-execution-v7","property_filters":{"selector":"property_id","scope":"node-query-and-path-node-query","binding":"same-graph-snapshot","field_and_property_id":"mutually-exclusive","unknown_value":"does-not-match-except-exists-false","outside_applicable_type":"does-not-match","unknown_property":"error","operators":"declared-per-property","string_comparison":"exact-codepoints-no-casefold-or-normalization","units_and_languages":"source-declared-no-implicit-conversion"},"path_query":{"conditions":4,"steps_per_condition":4,"quantifiers":["exists","not_exists"],"combination":"all","scope":"node-selector-roots-and-selected-sources","walks_may_revisit_nodes":true},"inclusion":{"request_field":"explain","authority":"query-execution-not-semantic-proof"},"pagination":{"request_field":"pagination","scope":"bounded-lens-result","snapshot_bound":true,"historical_snapshot_retention":false,"reexecutes_bounded_lens":true,"maximum_primary_nodes":100,"maximum_relations":100,"context_endpoints_may_repeat":true,"changed_query_or_snapshot_http_status":409},"neighborhood_profiles":[{"profile":"overview","definition":"Bibliographic and conceptual overview; dense text units, anchors and record-maker/provenance links are inspected separately. Shared record production does not establish semantic proximity. Source-filtered carriers of one declared ToS entity expand at zero distance before a relation hop, within node budgets.","identity_expansion":"declared-tos-entity-id-zero-distance","excluded_predicates":null,"excluded_relation_type_ids":null},{"profile":"all","definition":"All declared relation kinds, including detailed text structure; result limits still apply.","excluded_predicates":[]}],"sources":null,"filter_operators":null,"operator_value_contracts":{"eq":"scalar","neq":"scalar","in":"scalar-or-scalar-array","contains":"scalar-or-scalar-array","prefix":"string","exists":"boolean","gt":"number","gte":"number","lt":"number","lte":"number"},"node_fields":null,"relation_fields":null,"human_languages":{"key_pattern":"^(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|[iIxX](?:-[A-Za-z0-9]{1,8})+)$(?![\\s\\S])","reserved_roles":["default","original"],"registration_verified":false,"node_fields":null,"relation_fields":null,"fallback_order":["default","ru","en","original","remaining-keys-sorted"],"boundary":"Availability is not translation, semantic quality, or interface-language equivalence."},"attribute_field_pattern":"^(?:attributes|semantics)\\.[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$","node_attribute_fields":null,"relation_attribute_fields":null,"facets":null,"layouts":["auto","organic","timeline","flow","evidence","semantic","infrastructure","hierarchical","radial","matrix"],"endpoint_policies":["both","either","independent"],"focus":{"seed_field":"seed.focus_node_id","resolution_order":["id","entity_id","unique_native_id"],"shared_entity_id_resolution":"source-priority-then-node-id","ambiguous_native_id":"rejected","default_depth":1,"default_direction":"either","default_layout":"radial"},"maximums":{"filters_per_item_kind":32,"traversal_depth":5,"nodes":1000,"relations":2000,"groups":200},"entity_routes":null}"#;
    let mut result = if let Some(owner) = creation {
        owner.serde_owned_with_limits(
            STATIC.as_bytes(),
            JsonLimits::new(STATIC.len(), 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("catalog capability JSON limits"))?,
        )?
    } else {
        serde_json::from_str(STATIC).map_err(|_| Error::Invalid("catalog capabilities"))?
    };
    let object = result
        .as_object_mut()
        .ok_or(Error::Invalid("catalog capabilities"))?;
    let source_values = array(vocab, "sources")?;
    let mut sources = owned_vec(source_values.len(), creation)?;
    for source in source_values {
        let source_id = text(source, "source_graph_id")?;
        owned_bytes(creation, source_id.len())?;
        sources.push(Value::String(source_id.to_owned()));
    }
    let ordered_node_fields = owned_sorted_strings(filters, "node_fields", creation)?;
    let ordered_relation_fields = owned_sorted_strings(filters, "relation_fields", creation)?;
    let filter_operators = match filters.get("operators") {
        Some(value) => owned_value(value, creation)?,
        None => Value::Null,
    };
    object.insert("sources".into(), Value::Array(sources));
    object.insert("filter_operators".into(), filter_operators);
    object.insert("node_fields".into(), ordered_node_fields);
    object.insert("relation_fields".into(), ordered_relation_fields);
    let mut profile = object
        .get_mut("neighborhood_profiles")
        .and_then(Value::as_array_mut)
        .and_then(|profiles| profiles.get_mut(0))
        .and_then(Value::as_object_mut)
        .ok_or(Error::Invalid("catalog overview profile"))?;
    profile.insert(
        "excluded_predicates".into(),
        overview
            .get("excluded_predicate_ids")
            .map(|value| owned_value(value, creation))
            .transpose()?
            .unwrap_or(Value::Null),
    );
    profile.insert(
        "excluded_relation_type_ids".into(),
        overview
            .get("excluded_relation_type_ids")
            .map(|value| owned_value(value, creation))
            .transpose()?
            .unwrap_or(Value::Null),
    );
    let human = object
        .get_mut("human_languages")
        .and_then(Value::as_object_mut)
        .ok_or(Error::Invalid("catalog human language capability"))?;
    human.insert(
        "node_fields".into(),
        field_catalog(db, NODE, "display", budget, creation)?,
    );
    human.insert(
        "relation_fields".into(),
        field_catalog(db, RELATION, "display", budget, creation)?,
    );
    object.insert(
        "node_attribute_fields".into(),
        field_catalog(db, NODE, "attribute", budget, creation)?,
    );
    object.insert(
        "relation_attribute_fields".into(),
        field_catalog(db, RELATION, "attribute", budget, creation)?,
    );
    let mut facets_object = Map::new();
    owned_object(creation, &["nodes", "relations"])?;
    facets_object.insert(
        "nodes".into(),
        facets(db, vocab, NODE, max_entries, budget, creation)?,
    );
    facets_object.insert(
        "relations".into(),
        facets(db, vocab, RELATION, max_entries, budget, creation)?,
    );
    object.insert("facets".into(), Value::Object(facets_object));
    Ok(result)
}

fn managed_basis_value(header: &Value, creation: Option<&CreationState<'_>>) -> Result<Value> {
    let Some(owner) = creation else {
        let basis = crate::managed_source::header_basis(header)?;
        return serde_json::to_value(basis).map_err(|error| Error::Source(error.to_string()));
    };
    let managed = header.get("schema").and_then(Value::as_str)
        == Some(crate::managed_source::MANAGED_GRAPH_SCHEMA);
    let basis_input = if managed {
        header
            .get("source_basis")
            .ok_or(Error::Invalid("managed graph source basis"))?
    } else {
        header
            .get("source_revision")
            .ok_or(Error::Invalid("knowledge graph cut revision"))?
    };
    owner.with_json_encoded(basis_input, owner.remaining(0)?, |wire| {
        // header_basis clones this source shape and its proof validator holds
        // a bounded writer plus parsed/canonical representations at once.
        let shape = owner.value_clone_state_upper_bound(basis_input)?;
        let workspace = shape
            .checked_mul(2)
            .and_then(|n| wire.len().checked_mul(4).and_then(|m| n.checked_add(m)))
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<
                    crate::managed_source::KnowledgeSourceBasis,
                >())
            })
            .ok_or(Error::Budget("catalog managed basis workspace"))?;
        let _hold = owner.hold(workspace)?;
        owner.charge_work(workspace)?;
        let basis = crate::managed_source::header_basis(header)?;
        owner.with_json_encoded(&basis, owner.remaining(0)?, |basis_wire| {
            let limits = JsonLimits::new(basis_wire.len(), 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("catalog managed basis limits"))?;
            owner.serde_owned_with_limits(basis_wire, limits)
        })
    })
}

fn render(
    db: &Connection,
    header: &Value,
    entity_registry: &Value,
    relation_registry: &Value,
    lenses: &[Value],
    vocab: &Value,
    routes: &[Route],
    entity_entries: &RegistryEntries<'_, '_, '_>,
    relation_entries: &RegistryEntries<'_, '_, '_>,
    node_count: u64,
    relation_count: u64,
    max_entries: u64,
    budget: &ByteBudget,
    creation: Option<&CreationState<'_>>,
) -> Result<Value> {
    let node_types = top_counts(db, NODE, "type", budget, creation)?;
    let relation_types = top_counts(db, RELATION, "type", budget, creation)?;
    let claim_types = top_counts(db, NODE, "claim-type", budget, creation)?;
    let mut caps = capabilities(db, vocab, max_entries, budget, creation)?;
    caps.as_object_mut()
        .ok_or(Error::Invalid("catalog capabilities"))?
        .insert(
            "entity_routes".into(),
            route_catalog(db, routes, entity_entries, budget, creation)?,
        );
    let contract_refs = static_object(
        creation,
        &[
            ("public_bundle", "/api/knowledge/contracts"),
            ("knowledge_api", "access/contracts/knowledge-api.v1.json"),
            ("lens_spec", "access/contracts/lens-spec.v1.schema.json"),
            ("lens_result", "access/contracts/lens-result.v1.schema.json"),
            (
                "temporal_comparison_request",
                "access/contracts/temporal-comparison-request.v1.schema.json",
            ),
            (
                "temporal_comparison_result",
                "access/contracts/temporal-comparison-result.v1.schema.json",
            ),
            (
                "knowledge_graph",
                "access/contracts/knowledge-graph.v1.schema.json",
            ),
            (
                "readable_context",
                "access/contracts/readable-context.v1.schema.json",
            ),
            (
                "entity_type_registry_schema",
                "ToS/contracts/semantic-entity-type-registry.schema.json",
            ),
            (
                "relation_type_registry_schema",
                "ToS/contracts/semantic-relation-type-registry.schema.json",
            ),
            (
                "entity_type_registry",
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            ),
            (
                "relation_type_registry",
                "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            ),
        ],
    )?;
    let counts = match header.get("counts") {
        Some(value) => owned_value(value, creation)?,
        None => Value::Object(Map::new()),
    };
    let source_revision = header
        .get("source_revision")
        .map(|value| owned_value(value, creation))
        .transpose()?;
    let properties = match entity_registry.get("property_definitions") {
        Some(value) => owned_value(value, creation)?,
        None => Value::Array(Vec::new()),
    };
    let authority = match header.get("authority_boundary") {
        Some(value) => owned_value(value, creation)?,
        None => Value::Object(Map::new()),
    };
    let mut lenses_out = owned_vec(lenses.len(), creation)?;
    for lens in lenses {
        lenses_out.push(owned_value(lens, creation)?);
    }
    owned_object(
        creation,
        &[
            "schema",
            "source_revision",
            "context_presentation",
            "contract_refs",
            "counts",
            "node_kinds",
            "predicates",
            "semantic_registries",
            "lenses",
            "capabilities",
            "authority_boundary",
            "source_basis",
        ],
    )?;
    owned_object(creation, &["properties", "entity_types", "relation_types"])?;
    if let Some(n) = counts.get("nodes").and_then(Value::as_u64) {
        if n != node_count {
            return Err(Error::Invalid("catalog graph node count"));
        }
    }
    if let Some(n) = counts.get("relations").and_then(Value::as_u64) {
        if n != relation_count {
            return Err(Error::Invalid("catalog graph relation count"));
        }
    }
    let mut registries = Map::new();
    registries.insert("properties".into(), properties);
    registries.insert(
        "entity_types".into(),
        registry_metadata(
            entity_registry,
            entity_entries,
            &node_types,
            "fallback_type_id",
            NODE,
            node_count,
            None,
            creation,
        )?,
    );
    registries.insert(
        "relation_types".into(),
        registry_metadata(
            relation_registry,
            relation_entries,
            &relation_types,
            "fallback_relation_type_id",
            RELATION,
            relation_count,
            Some(&claim_types),
            creation,
        )?,
    );
    owned_bytes(creation, SCHEMA.len())?;
    let mut packet = Map::new();
    packet.insert("schema".into(), Value::String(SCHEMA.into()));
    packet.insert(
        "source_revision".into(),
        source_revision.unwrap_or(Value::Null),
    );
    packet.insert(
        "context_presentation".into(),
        presentation(entity_registry, creation)?,
    );
    packet.insert("contract_refs".into(), contract_refs);
    packet.insert("counts".into(), counts);
    packet.insert(
        "node_kinds".into(),
        group_catalog(db, NODE, relation_entries, budget, creation)?,
    );
    packet.insert(
        "predicates".into(),
        group_catalog(db, RELATION, relation_entries, budget, creation)?,
    );
    packet.insert("semantic_registries".into(), Value::Object(registries));
    packet.insert("lenses".into(), Value::Array(lenses_out));
    packet.insert("capabilities".into(), caps);
    packet.insert("authority_boundary".into(), authority);
    let mut packet = Value::Object(packet);
    if header.get("schema").and_then(Value::as_str)
        == Some(crate::managed_source::MANAGED_GRAPH_SCHEMA)
    {
        let basis = managed_basis_value(header, creation)?;
        let object = packet
            .as_object_mut()
            .ok_or(Error::Invalid("catalog object"))?;
        owned_bytes(
            creation,
            crate::managed_source::MANAGED_CATALOG_SCHEMA.len(),
        )?;
        object.insert(
            "schema".into(),
            Value::String(crate::managed_source::MANAGED_CATALOG_SCHEMA.into()),
        );
        object.remove("source_revision");
        object.insert("source_basis".into(), basis);
    }
    Ok(packet)
}

/// Reduce a full normalized graph into a private catalog candidate. The caller
/// verifies the sealed source cut and registry roots before invoking this.
/// All source rows are read in order; the only in-memory maps are the bounded
/// authored registries and the bounded final catalog.
pub fn compile_catalog(
    db: &mut Connection,
    graph_header: &Value,
    entity_registry: &Value,
    relation_registry: &Value,
    saved_lenses: &[Value],
    vocabulary: &QueryVocabulary,
    authored_descriptor: &[u8],
    limits: CatalogLimits,
) -> Result<CatalogReceipt> {
    compile_catalog_with_state(
        db,
        graph_header,
        entity_registry,
        relation_registry,
        saved_lenses,
        vocabulary,
        authored_descriptor,
        limits,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_catalog_with_state(
    db: &mut Connection,
    graph_header: &Value,
    entity_registry: &Value,
    relation_registry: &Value,
    saved_lenses: &[Value],
    vocabulary: &QueryVocabulary,
    authored_descriptor: &[u8],
    limits: CatalogLimits,
    creation: Option<&CreationState<'_>>,
) -> Result<CatalogReceipt> {
    let _digest_hold = creation.map(|owner| owner.hold(64)).transpose()?;
    if let Some(owner) = creation {
        owner.charge_work(authored_descriptor.len())?;
    }
    if Digest256::of_bytes(authored_descriptor).to_hex() != vocabulary.descriptor_sha256 {
        return Err(Error::Invalid("catalog vocabulary digest"));
    }
    input_preflight(
        graph_header,
        entity_registry,
        relation_registry,
        saved_lenses,
        authored_descriptor,
        limits.max_catalog_bytes,
        creation,
    )?;
    if let Some(creation) = creation {
        let json_limits = JsonLimits::new(limits.max_catalog_bytes, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("catalog vocabulary JSON limits"))?;
        creation.with_serde_owned_value_with_limits(
            authored_descriptor,
            json_limits,
            |descriptor| {
                compile_catalog_from_descriptor(
                    db,
                    graph_header,
                    entity_registry,
                    relation_registry,
                    saved_lenses,
                    vocabulary,
                    &descriptor,
                    limits,
                    Some(creation),
                )
            },
        )
    } else {
        let descriptor: Value = serde_json::from_slice(authored_descriptor)
            .map_err(|_| Error::Invalid("catalog vocabulary JSON"))?;
        compile_catalog_from_descriptor(
            db,
            graph_header,
            entity_registry,
            relation_registry,
            saved_lenses,
            vocabulary,
            &descriptor,
            limits,
            None,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_catalog_from_descriptor(
    db: &mut Connection,
    graph_header: &Value,
    entity_registry: &Value,
    relation_registry: &Value,
    saved_lenses: &[Value],
    vocabulary: &QueryVocabulary,
    descriptor: &Value,
    limits: CatalogLimits,
    creation: Option<&CreationState<'_>>,
) -> Result<CatalogReceipt> {
    if text(entity_registry, "registry_id")? != vocabulary.entity_registry_id
        || text(relation_registry, "registry_id")? != vocabulary.relation_registry_id
    {
        return Err(Error::Invalid("catalog registry identity"));
    }
    if descriptor
        .get("catalog")
        .and_then(|v| v.get("canonical_order"))
        .and_then(Value::as_str)
        != Some("source-graph-id-v1")
    {
        return Err(Error::Invalid("catalog source order profile"));
    }
    let entity_entries = registry_entries(entity_registry, "types", "type_id", creation)?;
    let relation_entries =
        registry_entries(relation_registry, "relations", "relation_type_id", creation)?;
    let route_values = array(
        descriptor
            .get("overview")
            .ok_or(Error::Invalid("overview vocabulary"))?,
        "routes",
    )?;
    let mut route_bytes = route_values
        .len()
        .checked_mul(std::mem::size_of::<Route>())
        .ok_or(Error::Budget("catalog route allocation"))?;
    for route in route_values {
        for field in [
            "route_id",
            "candidate_kind_ids",
            "confirming_predicate_ids",
            "candidate_type_ids",
            "confirming_relation_type_ids",
        ] {
            let value = route
                .get(field)
                .ok_or(Error::Invalid("catalog route field"))?;
            let mut bytes = 0usize;
            match value {
                Value::String(text) => {
                    bytes = text.len();
                }
                Value::Array(values) => {
                    bytes = values
                        .len()
                        .checked_mul(std::mem::size_of::<String>())
                        .ok_or(Error::Budget("catalog route allocation"))?;
                    for value in values {
                        let text = value
                            .as_str()
                            .ok_or(Error::Invalid("catalog route string"))?;
                        bytes = bytes
                            .checked_add(text.len())
                            .ok_or(Error::Budget("catalog route allocation"))?;
                    }
                }
                _ => return Err(Error::Invalid("catalog route field")),
            }
            route_bytes = route_bytes
                .checked_add(bytes)
                .ok_or(Error::Budget("catalog route allocation"))?;
        }
    }
    let _route_hold = creation.map(|owner| owner.hold(route_bytes)).transpose()?;
    if let Some(creation) = creation {
        creation.charge_work(route_bytes)?;
    }
    let route_defs = routes(descriptor)?;
    if route_defs
        .iter()
        .map(|r| &r.id)
        .ne(vocabulary.overview_route_ids.iter())
    {
        return Err(Error::Invalid("catalog vocabulary route mismatch"));
    }
    let original_temp_page_ceiling: u64 =
        db.query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))?;
    db.execute_batch("SAVEPOINT cmp_catalog_build")?;
    let result = (|| {
        stage(db, limits)?;
        let nodes = ingest_nodes(
            db,
            descriptor,
            &entity_entries,
            text(entity_registry, "fallback_type_id")?,
            &route_defs,
            limits,
            creation,
        )?;
        let cross_source_ids = array(descriptor, "sources")?;
        let relations = ingest_relations(
            db,
            descriptor,
            &route_defs,
            &relation_entries,
            text(relation_registry, "fallback_relation_type_id")?,
            &cross_source_ids,
            CatalogLimits {
                max_rows: limits
                    .max_rows
                    .checked_sub(nodes)
                    .ok_or(Error::Budget("catalog rows"))?,
                ..limits
            },
            creation,
        )?;
        if nodes
            .checked_add(relations)
            .ok_or(Error::Budget("catalog rows"))?
            > limits.max_rows
        {
            return Err(Error::Budget("catalog rows"));
        }
        let entries: u64 = db.query_row(
            "SELECT
            (SELECT count(*) FROM temp.cmp_catalog_counts)+
            (SELECT count(*) FROM temp.cmp_catalog_posts)+
            (SELECT count(*) FROM temp.cmp_catalog_node_routes)",
            [],
            |r| r.get(0),
        )?;
        if entries > limits.max_catalog_entries {
            return Err(Error::Budget("catalog aggregate entries"));
        }
        let aggregate_bytes: i64 = db.query_row(
            "SELECT
            coalesce((SELECT sum(length(CAST(kind AS BLOB))+length(CAST(metric AS BLOB))+
                length(CAST(a AS BLOB))+length(CAST(b AS BLOB))+length(CAST(c AS BLOB))+32)
                FROM temp.cmp_catalog_counts),0)+
            coalesce((SELECT sum(length(CAST(kind AS BLOB))+length(CAST(metric AS BLOB))+
                length(CAST(field AS BLOB))+length(CAST(value AS BLOB))+32)
                FROM temp.cmp_catalog_posts),0)+
            coalesce((SELECT sum(length(CAST(id AS BLOB))+length(CAST(legacy AS BLOB))+
                length(CAST(typed AS BLOB))+32) FROM temp.cmp_catalog_node_routes),0)",
            [],
            |r| r.get(0),
        )?;
        let reserved = entries
            .checked_mul(32)
            .ok_or(Error::Budget("catalog aggregate bytes"))?;
        if aggregate_bytes < 0
            || (aggregate_bytes as u64)
                .checked_add(reserved)
                .ok_or(Error::Budget("catalog aggregate bytes"))?
                > limits.max_catalog_bytes as u64
        {
            return Err(Error::Budget("catalog aggregate bytes"));
        }
        let (catalog, sha256) = {
            let budget = ByteBudget::new(limits.max_catalog_bytes);
            let catalog = render(
                db,
                graph_header,
                entity_registry,
                relation_registry,
                saved_lenses,
                descriptor,
                &route_defs,
                &entity_entries,
                &relation_entries,
                nodes,
                relations,
                limits.max_catalog_entries,
                &budget,
                creation,
            )?;
            let sha256 = if let Some(creation) = creation {
                creation.with_json_encoded(&catalog, limits.max_catalog_bytes, |bytes| {
                    creation.retain(64)?;
                    creation.charge_work(
                        bytes
                            .len()
                            .checked_add(64)
                            .ok_or(Error::Budget("catalog digest work"))?,
                    )?;
                    Ok(Digest256::of_bytes(bytes).to_hex())
                })?
            } else {
                let mut writer = BoundedWriter {
                    bytes: Vec::new(),
                    max: limits.max_catalog_bytes,
                };
                serde_json::to_writer(&mut writer, &catalog)
                    .map_err(|_| Error::Budget("catalog output bytes"))?;
                Digest256::of_bytes(&writer.bytes).to_hex()
            };
            (catalog, sha256)
        };
        let pages: u64 = db.query_row("PRAGMA temp.page_count", [], |r| r.get(0))?;
        if pages > limits.max_staging_pages {
            return Err(Error::Budget("catalog staging pages"));
        }
        Ok(CatalogReceipt {
            catalog,
            sha256,
            node_count: nodes,
            relation_count: relations,
        })
    })();
    let rollback = db.execute_batch("ROLLBACK TO cmp_catalog_build; RELEASE cmp_catalog_build");
    let restored: rusqlite::Result<u64> = db.query_row(
        &format!("PRAGMA temp.max_page_count={original_temp_page_ceiling}"),
        [],
        |r| r.get(0),
    );
    rollback?;
    if restored? != original_temp_page_ceiling {
        return Err(Error::Budget("catalog staging page ceiling restore"));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn insert_node(db: &Connection, id: &str, kind: &str, type_id: &str, order: i64) {
        let item = json!({"id":id,"source_graph":"canon","kind_id":kind,"type_id":type_id,
            "type_mapping":{"status":"mapped"},"source_refs":["ToS/canon/example.json"],
            "display":{"kind_label":{"default":kind},"title":{"default":id},
                "summary_state":"source","provenance":{"source_summary_available":true}},
            "epistemic":{"authority_layer":"canon","canon_status":"accepted","review_posture":"accepted"},
            "graph_layers":["canon"],"view_ids":["route-graph"]});
        let raw = serde_json::to_vec(&item).unwrap();
        db.execute(
            "INSERT INTO knowledge_nodes VALUES(?1,'canon',?2,?3,?4,?5,?6,?7)",
            params![
                id,
                kind,
                type_id,
                order,
                &raw,
                raw.len() as i64,
                Digest256::of_bytes(&raw).as_bytes().as_slice()
            ],
        )
        .unwrap();
    }

    fn fixture(kind: &str) -> (Connection, Value, Value, Value, QueryVocabulary) {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,source_graph TEXT,kind_id TEXT,type_id TEXT,
            source_order INTEGER,payload BLOB,payload_len INTEGER,payload_sha256 BLOB);
            CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,source_graph TEXT,from_id TEXT,to_id TEXT,
            predicate_id TEXT,relation_type_id TEXT,source_order INTEGER,payload BLOB,payload_len INTEGER,payload_sha256 BLOB);").unwrap();
        insert_node(&db, "canon:a", "work", "tos.entity.work", 0);
        insert_node(&db, "canon:b", kind, "tos.entity.concept", 1);
        let edge = json!({"id":"canon:r","source_graph":"canon","from_id":"canon:a","to_id":"canon:b",
            "predicate_id":"has_expression","relation_type_id":"tos.relation.has-expression",
            "predicate_mapping":{"status":"mapped"},"source_refs":["ToS/canon/relation.json"],
            "display":{"label":{"default":"has expression"},"explanation_state":"source",
                "provenance":{"source_explanation_available":true}},
            "epistemic":{"authority_layer":"canon"},"graph_layers":["canon"],"view_ids":["route-graph"]});
        let raw = serde_json::to_vec(&edge).unwrap();
        db.execute(
            "INSERT INTO knowledge_relations VALUES('canon:r','canon','canon:a','canon:b',
            'has_expression','tos.relation.has-expression',0,?1,?2,?3)",
            params![
                &raw,
                raw.len() as i64,
                Digest256::of_bytes(&raw).as_bytes().as_slice()
            ],
        )
        .unwrap();
        let entity = json!({"registry_id":"tos.semantic.entity-types","registry_version":1,
            "fallback_type_id":"tos.entity.unmapped","source_refs":[],"property_definitions":[],
            "types":[{"type_id":"tos.entity.work","parent_type_ids":[]},
                {"type_id":"tos.entity.concept","parent_type_ids":[]},
                {"type_id":"tos.entity.unmapped","parent_type_ids":[]}]});
        let relation = json!({"registry_id":"tos.semantic.relation-types","registry_version":1,
            "fallback_relation_type_id":"tos.relation.unmapped","source_refs":[],
            "relations":[{"relation_type_id":"tos.relation.has-expression","labels":{"default":"has expression"},
                "definition":"fixture","domain_type_ids":[],"range_type_ids":[]},
                {"relation_type_id":"tos.relation.unmapped"}]});
        let header = json!({"source_revision":"fixture-revision","counts":{"nodes":2,"relations":1},
            "authority_boundary":{"is_source":false}});
        let vocab = QueryVocabulary::parse(VOCAB, ADAPTERS).unwrap();
        (db, header, entity, relation, vocab)
    }

    #[test]
    fn full_catalog_is_deterministic_and_preserves_route_confirmation() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        let initial_page_ceiling: u64 = db
            .query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))
            .unwrap();
        let a = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            CatalogLimits::default(),
        )
        .unwrap();
        let b = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            CatalogLimits::default(),
        )
        .unwrap();
        assert_eq!(a.sha256, b.sha256);
        let final_page_ceiling: u64 = db
            .query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))
            .unwrap();
        assert_eq!(initial_page_ceiling, final_page_ceiling);
        assert_eq!((a.node_count, a.relation_count), (2, 1));
        assert_eq!(
            a.catalog["capabilities"]["facets"]["nodes"]["kind_id"][0]["value"],
            "concept"
        );
        let routes = a.catalog["capabilities"]["entity_routes"]
            .as_array()
            .unwrap();
        let work = routes.iter().find(|r| r["route_id"] == "work").unwrap();
        assert_eq!(work["role_readiness"], "confirmed");
        assert_eq!(work["semantic_confirming_relation_count"], 1);
        assert_eq!(
            a.catalog["semantic_registries"]["entity_types"]["mapped_instance_count"],
            2
        );
    }

    #[test]
    fn unicode_facets_preserve_python_casefold_and_stable_ties() {
        let (mut db, mut header, entity, relation, vocab) = fixture("Straße");
        for (position, kind) in ["STRASSE", "ς", "Σ", "İ", "é"].iter().enumerate() {
            insert_node(
                &db,
                &format!("canon:unicode-{position}"),
                kind,
                "tos.entity.concept",
                position as i64 + 2,
            );
        }
        header["counts"]["nodes"] = json!(7);
        let receipt = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            CatalogLimits::default(),
        )
        .unwrap();
        let values = receipt.catalog["capabilities"]["facets"]["nodes"]["kind_id"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["value"].as_str().unwrap())
            .collect::<Vec<_>>();
        // Maintained Python stable sorted(values,key=str.casefold): Straße and
        // STRASSE, and final/ordinary sigma retain original source encounter order.
        assert_eq!(values, ["İ", "Straße", "STRASSE", "work", "é", "ς", "Σ"]);
    }

    #[test]
    fn hostile_aggregate_count_refuses_before_render() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        let limits = CatalogLimits {
            max_catalog_entries: 1,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog aggregate entries"));
    }

    #[test]
    fn combined_row_limit_refuses_before_relation_payload_decode() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        db.execute(
            "UPDATE knowledge_relations SET payload=zeroblob(200000) WHERE id='canon:r'",
            [],
        )
        .unwrap();
        let limits = CatalogLimits {
            max_rows: 2,
            max_row_bytes: 1024,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog rows"));
    }

    #[test]
    fn oversized_blob_refuses_on_sql_length_before_decode() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        db.execute(
            "UPDATE knowledge_nodes SET payload=zeroblob(200000) WHERE id='canon:a'",
            [],
        )
        .unwrap();
        let limits = CatalogLimits {
            max_row_bytes: 1024,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog row bytes"));
    }

    #[test]
    fn large_facet_value_refuses_before_render() {
        let (mut db, header, entity, relation, vocab) = fixture(&"x".repeat(200_000));
        let limits = CatalogLimits {
            max_catalog_bytes: 64 * 1024,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog aggregate bytes"));
    }

    #[test]
    fn unregistered_source_and_tampered_carrier_refuse() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        db.execute(
            "UPDATE knowledge_nodes SET source_graph='unknown' WHERE id='canon:b'",
            [],
        )
        .unwrap();
        assert!(
            compile_catalog(
                &mut db,
                &header,
                &entity,
                &relation,
                &[],
                &vocab,
                VOCAB,
                CatalogLimits::default()
            )
            .is_err()
        );
        db.execute("UPDATE knowledge_nodes SET source_graph='canon',payload_sha256=zeroblob(32) WHERE id='canon:b'",[]).unwrap();
        assert!(
            compile_catalog(
                &mut db,
                &header,
                &entity,
                &relation,
                &[],
                &vocab,
                VOCAB,
                CatalogLimits::default()
            )
            .is_err()
        );
    }
}
