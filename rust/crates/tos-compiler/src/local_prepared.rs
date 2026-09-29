//! Offline owner-selected local publication. All lanes share one SQLite
//! transaction; no source assembly, selection or semantic admission occurs here.
pub use crate::local_prepared_read::{
    PreparedReadError, PreparedReadErrorCode, PreparedReadLimits, PreparedReadTransaction,
};
use crate::{Error, Result, local_prepared_aux as auxiliary, local_prepared_search as search};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Instant,
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    python_lower_unicode16_v1,
};

pub const SCHEMA: &str = "tos_local_prepared_read_model_v1";
pub const DESCRIPTOR_SCHEMA: &str = "tos_local_prepared_revision_v2";
pub const MAX_ADDRESS: u64 = 9_007_199_254_740_991;
const STRIDE: u64 = 1u64 << 32;
const TOP: &str = "knowledge_reader_top";
const LENS: &str = "knowledge_lens_top";
const CATALOG: &str = "knowledge_catalog";
const KINDS: [&str; 2] = ["node", "relation"];

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationLimits {
    pub max_bytes: u64,
    pub max_mutations: u64,
    pub max_row_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_changes: usize,
    pub max_change_bytes: usize,
}
impl Default for PublicationLimits {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024 * 1024,
            max_mutations: 2_000_000,
            max_row_bytes: 1_048_576,
            max_metadata_bytes: 8_388_608,
            max_changes: 4096,
            max_change_bytes: 16_777_216,
        }
    }
}
impl PublicationLimits {
    pub fn validate(self) -> Result<()> {
        if self.max_bytes == 0
            || self.max_bytes > 1u64 << 40
            || self.max_mutations == 0
            || self.max_mutations > MAX_ADDRESS
            || self.max_row_bytes == 0
            || self.max_metadata_bytes == 0
            || self.max_changes == 0
            || self.max_change_bytes == 0
        {
            Err(Error::Invalid("local prepared publication limits"))
        } else {
            Ok(())
        }
    }
}

/// Repeatable, explicit normalized rows in source encounter order. The two
/// passes are hashed independently; changing input aborts the new publication.
pub trait PreparedRows {
    fn visit(&mut self, kind: &str, sink: &mut dyn FnMut(&JsonValue) -> Result<()>) -> Result<()>;
}
/// Explicit whole-bootstrap alternatives. The caller reserves donor reads or
/// private scratch separately; neither path is an automatic fallback.
pub enum BootstrapSearch {
    Buffered,
    Reuse(crate::local_prepared_reuse::PreparedSearchReuse),
    Bulk {
        scratch_path: PathBuf,
        limits: crate::local_prepared_bulk::BulkBootstrapLimits,
    },
}

#[derive(Clone, Debug)]
pub struct PreparedChange {
    pub operation: String,
    pub kind: String,
    pub identifier: String,
    pub item: Option<JsonValue>,
    pub source_order: Option<u64>,
}

pub(crate) fn parse(raw: &str, cap: usize) -> Result<JsonValue> {
    crate::d1_public_capture::json(raw.as_bytes(), cap)
}
pub(crate) fn compact(value: &JsonValue, cap: usize) -> Result<String> {
    String::from_utf8(crate::d1_public_capture::compact(value, cap)?)
        .map_err(|_| Error::Invalid("prepared JSON UTF-8"))
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn field<'a>(item: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    item.object_get(key)
        .ok_or(Error::Invalid("prepared required field"))
}
fn required<'a>(item: &'a JsonValue, key: &str) -> Result<&'a str> {
    field(item, key)?
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("prepared required string"))
}
fn valid_digest(raw: &str) -> Result<()> {
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Invalid("prepared lowercase digest"));
    }
    Ok(())
}
fn same(a: &JsonValue, b: &JsonValue) -> Result<bool> {
    use tos_foundation::{CanonicalProfile, JsonLimits, canonical_bytes_v1};
    let limits = JsonLimits::new(8_388_608, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("prepared comparison"))?;
    Ok(
        canonical_bytes_v1(a, CanonicalProfile::SourceRecordDigestV1, limits)
            .map_err(|e| Error::Source(e.to_string()))?
            == canonical_bytes_v1(b, CanonicalProfile::SourceRecordDigestV1, limits)
                .map_err(|e| Error::Source(e.to_string()))?,
    )
}
fn digest(value: &JsonValue, cap: usize) -> Result<String> {
    Ok(Digest256::of_bytes(compact(value, cap)?.as_bytes()).to_hex())
}
fn hash_text(raw: &str) -> String {
    Digest256::of_bytes(raw.as_bytes()).to_hex()
}
fn sha_value(raw: &str) -> JsonValue {
    object(vec![("sha256", text(&hash_text(raw)))])
}
fn lower(raw: &str) -> Result<String> {
    python_lower_unicode16_v1(raw, 4096, 16_384, 16_384).map_err(|e| Error::Source(e.to_string()))
}
fn columns(kind: &str) -> Result<&'static [&'static str]> {
    match kind {
        "node" => Ok(&[
            "id",
            "entity_id",
            "native_id",
            "source_graph",
            "kind_id",
            "type_id",
        ]),
        "relation" => Ok(&[
            "id",
            "native_id",
            "source_graph",
            "from_id",
            "to_id",
            "predicate_id",
            "relation_type_id",
        ]),
        _ => Err(Error::Invalid("prepared kind")),
    }
}
fn dimensions(kind: &str) -> Result<&'static [&'static str]> {
    match kind {
        "node" => Ok(&["source_graph", "kind_id", "type_id"]),
        "relation" => Ok(&["source_graph", "predicate_id", "relation_type_id"]),
        _ => Err(Error::Invalid("prepared kind")),
    }
}
// Normalized index dimensions are strings. Missing/null/false/zero are the
// maintained empty value; refuse malformed compound index values explicitly.
pub fn index_value(item: &JsonValue, key: &str) -> Result<String> {
    match item.object_get(key) {
        None | Some(JsonValue::Null) | Some(JsonValue::Bool(false)) => Ok(String::new()),
        Some(JsonValue::Bool(true)) => Ok("True".into()),
        Some(JsonValue::String(s)) => s
            .as_str()
            .map(str::to_owned)
            .ok_or(Error::Invalid("prepared string scalar")),
        Some(JsonValue::Number(n)) => {
            let numeric = n.lexeme.parse::<f64>().ok();
            if numeric == Some(0.0) {
                Ok(String::new())
            } else {
                compact(item.object_get(key).unwrap(), 1_048_576)
            }
        }
        Some(JsonValue::Array(v)) if v.is_empty() => Ok(String::new()),
        Some(JsonValue::Object(v)) if v.is_empty() => Ok(String::new()),
        _ => Err(Error::PreparedUnsupported(
            "prepared normalized index field must be scalar",
        )),
    }
}
fn indexed(item: &JsonValue, key: &str) -> Result<String> {
    index_value(item, key)
}
fn row(kind: &str, item: &JsonValue, limits: PublicationLimits) -> Result<String> {
    let id = required(item, "id")?;
    if id.chars().count() > 4096 {
        return Err(Error::Budget("prepared identifier"));
    }
    let raw = compact(item, limits.max_row_bytes)?;
    let mut size = raw.len() + 1024;
    for key in columns(kind)? {
        size = size
            .checked_add(indexed(item, key)?.len())
            .ok_or(Error::Budget("prepared indexed row"))?;
    }
    if size > limits.max_row_bytes.max(131072) + 4096 {
        return Err(Error::Budget("prepared indexed row"));
    }
    Ok(raw)
}

pub(crate) fn metadata(db: &Connection, key: &str, cap: usize) -> Result<JsonValue> {
    let mut stmt=db.prepare("SELECT part,CASE WHEN length(CAST(json_chunk AS BLOB))<=131072 THEN json_chunk ELSE NULL END FROM edge_meta WHERE key=? ORDER BY part LIMIT 257")?;
    let mut rows = stmt.query([key])?;
    let mut raw = String::new();
    let mut part = 0;
    while let Some(r) = rows.next()? {
        let index: i64 = r.get(0)?;
        let chunk: Option<String> = r.get(1)?;
        if index != part || part >= 256 {
            return Err(Error::Invalid("prepared metadata chunks"));
        }
        let chunk = chunk.ok_or(Error::Budget("prepared metadata chunk"))?;
        if raw.len().checked_add(chunk.len()).is_none_or(|n| n > cap) {
            return Err(Error::Budget("prepared metadata"));
        }
        raw.push_str(&chunk);
        part += 1;
    }
    if part == 0 {
        return Err(Error::Invalid("prepared metadata absent"));
    }
    let value = parse(&raw, cap)?;
    if compact(&value, cap)? != raw {
        return Err(Error::Invalid("prepared metadata framing"));
    }
    Ok(value)
}
fn put_metadata(
    db: &Connection,
    key: &str,
    value: &JsonValue,
    limits: PublicationLimits,
) -> Result<()> {
    let raw = compact(
        value,
        if key == TOP {
            limits.max_metadata_bytes.min(65536)
        } else {
            limits.max_metadata_bytes
        },
    )?;
    let mut chunks = Vec::new();
    let mut start = 0;
    for (n, (position, _)) in raw.char_indices().enumerate() {
        if n > 0 && n % 32768 == 0 {
            chunks.push(&raw[start..position]);
            start = position;
        }
    }
    chunks.push(&raw[start..]);
    if chunks.len() > 256 {
        return Err(Error::Budget("prepared metadata chunks"));
    }
    db.execute("DELETE FROM edge_meta WHERE key=?", [key])?;
    let mut insert = db.prepare_cached("INSERT INTO edge_meta VALUES (?1,?2,?3)")?;
    for (part, chunk) in chunks.into_iter().enumerate() {
        insert.execute(params![key, part as i64, chunk])?;
    }
    Ok(())
}

pub(crate) fn validate_header(header: &JsonValue, catalog: &JsonValue) -> Result<()> {
    if header.as_object().is_none()
        || header.object_get("nodes").is_some()
        || header.object_get("relations").is_some()
        || required(header, "schema")? != "tos_knowledge_graph_v1"
        || required(catalog, "schema")? != "tos_knowledge_catalog_v1"
        || field(header, "source_revision")? != field(catalog, "source_revision")?
    {
        return Err(Error::Invalid("prepared coherent header/catalog"));
    }
    for key in ["source_revision"] {
        valid_digest(required(header, key)?)?;
    }
    let normalization = field(header, "normalization_binding")?;
    if normalization.as_object().is_none_or(|o| o.len() != 5)
        || required(normalization, "schema")? != "tos_knowledge_graph_normalization_binding_v1"
    {
        return Err(Error::Invalid("prepared normalization"));
    }
    for key in [
        "processor_digest",
        "entity_registry_digest",
        "relation_registry_digest",
        "configuration_digest",
    ] {
        valid_digest(required(normalization, key)?)?;
    }
    if let Some(value) = catalog.object_get("normalization_binding") {
        if !same(value, normalization)? {
            return Err(Error::Invalid("prepared catalog normalization"));
        }
    }
    let boundary = field(header, "authority_boundary")?;
    if required(boundary, "source_owner")? != "Tree-of-Sophia"
        || ["is_source", "is_canon", "writes_to_tree"]
            .iter()
            .any(|k| boundary.object_get(k) != Some(&JsonValue::Bool(false)))
    {
        return Err(Error::Invalid("prepared authority boundary"));
    }
    Ok(())
}
pub(crate) fn capabilities() -> JsonValue {
    object(vec![
        ("full_rows", JsonValue::Bool(true)),
        ("catalog", JsonValue::Bool(true)),
        ("lens", JsonValue::Bool(true)),
        ("compressed_search_v3", JsonValue::Bool(true)),
        ("legacy_search", JsonValue::Bool(false)),
        ("search_v2", JsonValue::Bool(false)),
        ("edge_runtime", JsonValue::Bool(false)),
    ])
}
#[derive(Default)]
struct Histogram {
    map: BTreeMap<[String; 3], u64>,
    bytes: usize,
}
impl std::ops::Deref for Histogram {
    type Target = BTreeMap<[String; 3], u64>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}
impl std::ops::DerefMut for Histogram {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}
fn cell(kind: &str, item: &JsonValue) -> Result<[String; 3]> {
    let d = dimensions(kind)?;
    Ok([
        indexed(item, d[0])?,
        indexed(item, d[1])?,
        indexed(item, d[2])?,
    ])
}
fn adjust(hist: &mut Histogram, cell: [String; 3], add: bool) -> Result<()> {
    let existing = hist.get(&cell).copied();
    let previous = existing.unwrap_or(0);
    let n = if add {
        previous.checked_add(1)
    } else {
        previous.checked_sub(1)
    }
    .filter(|n| *n <= MAX_ADDRESS)
    .ok_or(Error::Invalid("prepared count histogram"))?;
    if cell.iter().any(String::is_empty) {
        return Err(Error::Invalid("prepared empty histogram dimension"));
    }
    let size = |count: u64| -> Result<usize> {
        Ok(compact(
            &JsonValue::Array(vec![
                text(&cell[0]),
                text(&cell[1]),
                text(&cell[2]),
                number(count),
            ]),
            1_048_576,
        )?
        .len()
            + 1)
    };
    let before = if previous == 0 { 0 } else { size(previous)? };
    let after = if n == 0 { 0 } else { size(n)? };
    hist.bytes = hist
        .bytes
        .checked_sub(before)
        .and_then(|v| v.checked_add(after))
        .filter(|v| *v <= 1_048_576)
        .ok_or(Error::Budget("prepared histogram retained bytes"))?;
    if n == 0 {
        hist.remove(&cell);
    } else {
        hist.insert(cell, n);
    }
    if hist.len() > 16384 {
        return Err(Error::Budget("prepared count histogram"));
    }
    Ok(())
}
fn cells(hist: &Histogram) -> JsonValue {
    JsonValue::Array(
        hist.iter()
            .map(|(k, n)| JsonValue::Array(vec![text(&k[0]), text(&k[1]), text(&k[2]), number(*n)]))
            .collect(),
    )
}
fn lens(header: &JsonValue, hist: &[Histogram; 2]) -> Result<JsonValue> {
    let properties = header
        .object_get("query_properties")
        .cloned()
        .unwrap_or(JsonValue::Array(vec![]));
    if properties.as_array().is_none_or(|v| v.len() > 4096) {
        return Err(Error::Invalid("prepared query properties"));
    }
    for property in properties.as_array().unwrap() {
        if property.as_object().is_none() {
            return Err(Error::Invalid("prepared lens property framing"));
        }
        for key in ["property_id", "field", "value_type"] {
            required(property, key)?;
        }
        if !matches!(property.object_get("inherited"), Some(JsonValue::Bool(_))) {
            return Err(Error::Invalid("prepared lens property inherited"));
        }
        for key in ["applies_to", "operators"] {
            if field(property, key)?
                .as_array()
                .is_none_or(|a| a.iter().any(|v| v.as_str().is_none_or(str::is_empty)))
            {
                return Err(Error::Invalid("prepared lens property array"));
            }
        }
    }
    let value = object(vec![
        ("schema", text("tos_published_lens_metadata_v1")),
        ("execution_version", text("tos-lens-execution-v7")),
        ("source_revision", field(header, "source_revision")?.clone()),
        ("sort_key", text("python-str-or-empty-lower-v1")),
        ("unicode_version", text("16.0.0")),
        ("query_properties", properties),
        ("node_counts", cells(&hist[0])),
        ("relation_counts", cells(&hist[1])),
    ]);
    compact(&value, 1_048_576)?;
    Ok(value)
}
fn descriptor(
    header: &JsonValue,
    catalog: &JsonValue,
    tail: Vec<(&str, JsonValue)>,
    limits: PublicationLimits,
) -> Result<JsonValue> {
    let mut f = vec![
        ("schema", text(DESCRIPTOR_SCHEMA)),
        (
            "mode",
            text(if tail.first().is_some_and(|(k, _)| *k == "rows_sha256") {
                "bootstrap"
            } else {
                "delta-history"
            }),
        ),
        ("profile", text(SCHEMA)),
        ("algorithm", text(search::ALGORITHM)),
        (
            "search_storage_version",
            number(search::STORAGE_VERSION as u64),
        ),
        ("capabilities", capabilities()),
    ];
    if let Some((_, parent)) = tail.iter().find(|(k, _)| *k == "parent_data_revision") {
        f.push(("parent_data_revision", parent.clone()));
    }
    f.push(("header", header.clone()));
    f.push((
        "catalog_sha256",
        text(&digest(catalog, limits.max_metadata_bytes)?),
    ));
    f.extend(
        tail.into_iter()
            .filter(|(k, _)| *k != "parent_data_revision"),
    );
    Ok(object(f))
}
fn publish_header(
    db: &Connection,
    header: &JsonValue,
    catalog: &JsonValue,
    lens: &JsonValue,
    descriptor: &JsonValue,
    epoch: u64,
    limits: PublicationLimits,
) -> Result<JsonValue> {
    let revision = digest(descriptor, limits.max_metadata_bytes)?;
    let top = object(vec![
        ("schema", text("tos_published_knowledge_reader_v2")),
        ("read_model_schema", text(SCHEMA)),
        ("source_revision", field(header, "source_revision")?.clone()),
        ("data_revision", text(&revision)),
        ("graph_schema", field(header, "schema")?.clone()),
        (
            "normalization_binding",
            field(header, "normalization_binding")?.clone(),
        ),
        (
            "catalog_sha256",
            text(&digest(catalog, limits.max_metadata_bytes)?),
        ),
        ("row_integrity", text("sha256-emitted-json-v1")),
        (
            "authority_boundary",
            field(header, "authority_boundary")?.clone(),
        ),
        ("lens_sha256", text(&digest(lens, 1_048_576)?)),
    ]);
    for (key, value) in [(TOP, &top), (CATALOG, catalog), (LENS, lens)] {
        put_metadata(db, key, value, limits)?;
    }
    put_metadata(
        db,
        "data_revision",
        &object(vec![("sha256", text(&revision))]),
        limits,
    )?;
    db.execute(
        "INSERT OR REPLACE INTO knowledge_exploration_clock VALUES(1,?1)",
        [epoch as i64],
    )?;
    binding(&top, epoch, limits)
}
fn binding(top: &JsonValue, epoch: u64, limits: PublicationLimits) -> Result<JsonValue> {
    let mut fields = vec![
        ("schema", text("tos_published_knowledge_snapshot_v1")),
        ("publication_epoch", number(epoch)),
        (
            "metadata_sha256",
            text(&digest(top, limits.max_metadata_bytes)?),
        ),
    ];
    for key in [
        "read_model_schema",
        "source_revision",
        "data_revision",
        "graph_schema",
        "normalization_binding",
    ] {
        fields.push((key, field(top, key)?.clone()));
    }
    Ok(object(fields))
}
pub(crate) fn snapshot_binding(db: &Connection) -> Result<JsonValue> {
    let top = metadata(db, TOP, 65536)?;
    if top.as_object().is_none_or(|v| v.len() != 11) {
        return Err(Error::Invalid("prepared exact top fields"));
    }
    let epoch: u64 = db.query_row(
        "SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    if epoch > MAX_ADDRESS {
        return Err(Error::Invalid("prepared epoch"));
    }
    if !matches!(
        required(&top, "read_model_schema")?,
        SCHEMA | "tos_cloudflare_edge_read_model_v9"
    ) || required(&top, "schema")? != "tos_published_knowledge_reader_v2"
        || required(&top, "row_integrity")? != "sha256-emitted-json-v1"
    {
        return Err(Error::Invalid("prepared snapshot header"));
    }
    let header = object(vec![
        ("schema", field(&top, "graph_schema")?.clone()),
        ("source_revision", field(&top, "source_revision")?.clone()),
        (
            "normalization_binding",
            field(&top, "normalization_binding")?.clone(),
        ),
        (
            "authority_boundary",
            field(&top, "authority_boundary")?.clone(),
        ),
    ]);
    let catalog = object(vec![
        ("schema", text("tos_knowledge_catalog_v1")),
        ("source_revision", field(&top, "source_revision")?.clone()),
    ]);
    validate_header(&header, &catalog)?;
    for k in ["data_revision", "catalog_sha256", "lens_sha256"] {
        valid_digest(required(&top, k)?)?;
    }
    binding(&top, epoch, PublicationLimits::default())
}

fn cap(db: &Connection, limits: PublicationLimits, retained: Option<u64>) -> Result<u64> {
    let size: u64 = db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let old: u64 = db.query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
    let count: u64 = db.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let max = (limits.max_bytes / size)
        .min(old)
        .min(retained.unwrap_or(u64::MAX));
    if max == 0 || count > max {
        return Err(Error::Budget("whole prepared database pages"));
    }
    db.pragma_update(None, "max_page_count", max)?;
    Ok(max)
}
pub fn ensure_source_scope_indexes(db: &Connection) -> Result<()> {
    if db.is_autocommit() {
        return Err(Error::Invalid("prepared index caller transaction"));
    }
    for (name, table, columns) in [
        (
            "knowledge_nodes_source_kind_idx",
            "knowledge_nodes",
            ["source_graph", "kind_id"],
        ),
        (
            "knowledge_relations_source_predicate_idx",
            "knowledge_relations",
            ["source_graph", "predicate_id"],
        ),
    ] {
        let found: Option<String> = db
            .query_row(
                "SELECT tbl_name FROM sqlite_master WHERE type='index' AND name=?",
                [name],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(actual) = found {
            let mut q = db.prepare(&format!("PRAGMA index_info({name})"))?;
            let actual_cols = q
                .query_map([], |r| r.get::<_, String>(2))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut q = db.prepare(&format!("PRAGMA index_list({table})"))?;
            let entries = q
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(4)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if actual != table
                || actual_cols != columns
                || !entries
                    .iter()
                    .any(|(n, u, p)| n == name && *u == 0 && *p == 0)
            {
                return Err(Error::Invalid("prepared source index definition"));
            }
        } else {
            db.execute_batch(&format!(
                "CREATE INDEX {name} ON {table}({},{})",
                columns[0], columns[1]
            ))?;
        }
    }
    Ok(())
}
fn ddl(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE edge_meta(key TEXT NOT NULL,part INTEGER NOT NULL,json_chunk TEXT NOT NULL,PRIMARY KEY(key,part));CREATE TABLE knowledge_exploration_clock(singleton INTEGER PRIMARY KEY CHECK(singleton=1),epoch INTEGER NOT NULL);CREATE TABLE knowledge_lens_order(kind TEXT NOT NULL,id TEXT NOT NULL,sort_key TEXT NOT NULL,from_id TEXT NOT NULL,to_id TEXT NOT NULL,PRIMARY KEY(kind,id));CREATE TABLE prepared_documents(kind TEXT NOT NULL,id TEXT NOT NULL,doc_id INTEGER NOT NULL UNIQUE,source_order INTEGER NOT NULL,PRIMARY KEY(kind,id));CREATE TABLE prepared_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),high_water INTEGER NOT NULL,max_pages INTEGER NOT NULL,descriptor TEXT NOT NULL);")?;
    for kind in KINDS {
        let cols = columns(kind)?;
        let body = cols
            .iter()
            .map(|k| {
                format!(
                    "{k} TEXT {}",
                    if *k == "id" {
                        "PRIMARY KEY"
                    } else {
                        "NOT NULL"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        db.execute_batch(&format!(
            "CREATE TABLE knowledge_{kind}s({body},json TEXT NOT NULL);"
        ))?;
    }
    for (name, body) in [
        ("knowledge_nodes_native_idx", "knowledge_nodes(native_id)"),
        ("knowledge_nodes_entity_idx", "knowledge_nodes(entity_id)"),
        (
            "knowledge_nodes_identity_seek",
            "knowledge_nodes(entity_id,id)",
        ),
        (
            "knowledge_relations_native_idx",
            "knowledge_relations(native_id)",
        ),
        (
            "knowledge_relations_from_seek",
            "knowledge_relations(from_id,id)",
        ),
        (
            "knowledge_relations_to_seek",
            "knowledge_relations(to_id,id)",
        ),
        (
            "knowledge_lens_order_sort",
            "knowledge_lens_order(kind,sort_key,id)",
        ),
        (
            "knowledge_lens_order_from",
            "knowledge_lens_order(kind,from_id,sort_key,id)",
        ),
        (
            "knowledge_lens_order_to",
            "knowledge_lens_order(kind,to_id,sort_key,id)",
        ),
        (
            "knowledge_lens_order_pair",
            "knowledge_lens_order(kind,from_id,to_id,id)",
        ),
    ] {
        db.execute_batch(&format!("CREATE INDEX {name} ON {body};"))?;
    }
    ensure_source_scope_indexes(db)
}
fn put_row(
    db: &Connection,
    kind: &str,
    item: &JsonValue,
    raw: &str,
    limits: PublicationLimits,
) -> Result<()> {
    let cols = columns(kind)?;
    let mut values = cols
        .iter()
        .map(|k| indexed(item, k))
        .collect::<Result<Vec<_>>>()?;
    values.push(raw.to_owned());
    let placeholders = (1..=values.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    db.execute(
        &format!("INSERT OR REPLACE INTO knowledge_{kind}s VALUES({placeholders})"),
        rusqlite::params_from_iter(values),
    )?;
    let id = required(item, "id")?;
    db.execute(
        "INSERT OR REPLACE INTO knowledge_lens_order VALUES(?1,?2,?3,?4,?5)",
        params![
            kind,
            id,
            lower(id)?,
            if kind == "relation" {
                indexed(item, "from_id")?
            } else {
                String::new()
            },
            if kind == "relation" {
                indexed(item, "to_id")?
            } else {
                String::new()
            }
        ],
    )?;
    put_metadata(
        db,
        &format!("knowledge_{kind}_digest:{id}"),
        &sha_value(raw),
        limits,
    )
}
fn endpoint(db: &Connection, id: &str) -> Result<()> {
    if !db.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_nodes WHERE id=?)",
        [id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(Error::Invalid("prepared relation endpoint"));
    }
    Ok(())
}
fn frame(
    hash: &mut Digest256Hasher,
    kind: &str,
    id: &str,
    address: u64,
    token: u64,
    raw: &str,
) -> Result<()> {
    let value = JsonValue::Array(vec![
        text(kind),
        text(id),
        number(address),
        number(token),
        text(&hash_text(raw)),
    ]);
    hash.update(compact(&value, 32768)?.as_bytes());
    hash.update(b"\n");
    Ok(())
}

/// Exclusive 0600 new-file bootstrap. Failure deletes only its created inode.
pub fn publish_prepared_rows<R: PreparedRows>(
    path: &Path,
    header: &JsonValue,
    catalog: &JsonValue,
    rows: &mut R,
    limits: PublicationLimits,
) -> Result<JsonValue> {
    publish_prepared_rows_with_deadline(
        path,
        header,
        catalog,
        rows,
        limits,
        BootstrapSearch::Buffered,
        None,
    )
}
/// Native adapter variant. One absolute deadline covers both row passes and SQL.
pub fn publish_prepared_rows_until<R: PreparedRows>(
    path: &Path,
    header: &JsonValue,
    catalog: &JsonValue,
    rows: &mut R,
    limits: PublicationLimits,
    deadline: Instant,
) -> Result<JsonValue> {
    publish_prepared_rows_with_deadline(
        path,
        header,
        catalog,
        rows,
        limits,
        BootstrapSearch::Buffered,
        Some(deadline),
    )
}
pub fn publish_prepared_rows_with_search<R: PreparedRows>(
    path: &Path,
    header: &JsonValue,
    catalog: &JsonValue,
    rows: &mut R,
    limits: PublicationLimits,
    search: BootstrapSearch,
) -> Result<JsonValue> {
    publish_prepared_rows_with_deadline(path, header, catalog, rows, limits, search, None)
}
pub fn publish_prepared_rows_with_search_until<R: PreparedRows>(
    path: &Path,
    header: &JsonValue,
    catalog: &JsonValue,
    rows: &mut R,
    limits: PublicationLimits,
    search: BootstrapSearch,
    deadline: Instant,
) -> Result<JsonValue> {
    publish_prepared_rows_with_deadline(path, header, catalog, rows, limits, search, Some(deadline))
}
fn write_bootstrap_rows<R: PreparedRows>(
    db: &Connection,
    rows: &mut R,
    count: u64,
    limits: PublicationLimits,
    deadline: Option<Instant>,
    emit: &mut dyn FnMut(u64, &str, &JsonValue, u64, String) -> Result<()>,
) -> Result<(u64, String)> {
    let mut actual = Digest256Hasher::new();
    let mut address = 0u64;
    for kind in KINDS {
        let mut position = 0u64;
        rows.visit(kind, &mut |item| {
            check_deadline(deadline)?;
            address += 1;
            if address > count {
                return Err(Error::Invalid("prepared changed repeatable input"));
            }
            let token = position
                .checked_mul(STRIDE)
                .filter(|n| *n <= MAX_ADDRESS)
                .ok_or(Error::Budget("prepared source order"))?;
            position += 1;
            let raw = row(kind, item, limits)?;
            let id = required(item, "id")?;
            frame(&mut actual, kind, id, address, token, &raw)?;
            put_row(db, kind, item, &raw, limits)?;
            db.execute(
                "INSERT INTO prepared_documents VALUES(?1,?2,?3,?4)",
                params![kind, id, address as i64, token as i64],
            )?;
            if kind == "relation" {
                endpoint(db, required(item, "from_id")?)?;
                endpoint(db, required(item, "to_id")?)?;
            }
            emit(address, kind, item, token, raw)
        })?;
    }
    Ok((address, actual.finalize().to_hex()))
}

fn publish_prepared_rows_with_deadline<R: PreparedRows>(
    path: &Path,
    header: &JsonValue,
    catalog: &JsonValue,
    rows: &mut R,
    limits: PublicationLimits,
    search_mode: BootstrapSearch,
    deadline: Option<Instant>,
) -> Result<JsonValue> {
    limits.validate()?;
    validate_header(header, catalog)?;
    lens(header, &Default::default())?;
    compact(catalog, limits.max_metadata_bytes)?;
    let (mut donor, bulk) = match search_mode {
        BootstrapSearch::Buffered => (None, None),
        BootstrapSearch::Reuse(request) => (
            Some(crate::local_prepared_reuse::SearchDonor::open(
                request, limits, deadline,
            )?),
            None,
        ),
        BootstrapSearch::Bulk {
            scratch_path,
            limits,
        } => {
            limits.validate()?;
            if scratch_path == path {
                return Err(Error::Invalid("prepared scratch aliases target"));
            }
            (None, Some((scratch_path, limits)))
        }
    };
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    let mut source_bytes = 0u64;
    let mut hist: [Histogram; 2] = Default::default();
    for (k, kind) in KINDS.iter().enumerate() {
        let mut position = 0u64;
        rows.visit(kind, &mut |item| {
            check_deadline(deadline)?;
            let raw = row(kind, item, limits)?;
            source_bytes = source_bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= limits.max_bytes)
                .ok_or(Error::Budget("prepared raw rows cannot fit whole file cap"))?;
            count = count
                .checked_add(1)
                .filter(|n| *n <= limits.max_mutations.min(MAX_ADDRESS))
                .ok_or(Error::Budget("prepared row/mutations"))?;
            let token = position
                .checked_mul(STRIDE)
                .filter(|n| *n <= MAX_ADDRESS)
                .ok_or(Error::Budget("prepared source order"))?;
            position += 1;
            if let Some(donor) = &mut donor {
                donor.observe(kind, required(item, "id")?, &raw, count, token)?;
            }
            adjust(&mut hist[k], cell(kind, item)?, true)?;
            if hist[0].bytes + hist[1].bytes > 1_048_576 {
                return Err(Error::Budget("prepared lens histogram bytes"));
            }
            frame(&mut hash, kind, required(item, "id")?, count, token, &raw)
        })?;
    }
    if let Some(donor) = &mut donor {
        donor.finish_observation(count)?;
    }
    let expected = hash.finalize().to_hex();
    let descriptor = descriptor(
        header,
        catalog,
        vec![("rows_sha256", text(&expected))],
        limits,
    )?;
    let lens = lens(header, &hist)?;
    let parent = path.parent().ok_or(Error::Invalid("prepared parent"))?;
    if !parent.is_dir() || parent.is_symlink() {
        return Err(Error::Invalid("prepared parent"));
    }
    for suffix in ["-journal", "-wal", "-shm"] {
        if Path::new(&format!("{}{suffix}", path.display()))
            .symlink_metadata()
            .is_ok()
        {
            return Err(Error::Invalid("prepared sidecar exists"));
        }
    }
    let created = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    let stamp = created.metadata()?;
    drop(created);
    let result = (|| {
        let db = Connection::open(path)?;
        let current = fs::symlink_metadata(path)?;
        if current.dev() != stamp.dev() || current.ino() != stamp.ino() {
            return Err(Error::Invalid("prepared created file changed at open"));
        }
        install_deadline(&db, deadline);
        db.execute_batch("PRAGMA journal_mode=DELETE;PRAGMA synchronous=FULL;PRAGMA temp_store=MEMORY;PRAGMA cache_size=-8192;BEGIN IMMEDIATE;")?;
        let maximum = cap(&db, limits, None)?;
        ddl(&db)?;
        let binding = publish_header(&db, header, catalog, &lens, &descriptor, 1, limits)?;
        let already = db.total_changes();
        let search_max = limits
            .max_mutations
            .checked_sub(already)
            .and_then(|n| n.checked_sub(4 * count + 1))
            .ok_or(Error::Budget("prepared search mutations"))?;
        let search_bytes =
            maximum * db.query_row("PRAGMA page_size", [], |r| r.get::<_, u64>(0))?;
        let mut second = None;
        let mut scratch_mutations = 0u64;
        let search_report = if let Some(donor) = &mut donor {
            let mut replacements = Vec::new();
            let mut changed_bytes = 0usize;
            second = Some(write_bootstrap_rows(
                &db,
                rows,
                count,
                limits,
                deadline,
                &mut |address, kind, item, order, raw| {
                    if donor.should_replace(kind, required(item, "id")?) {
                        changed_bytes = changed_bytes
                            .checked_add(raw.len())
                            .filter(|n| *n <= limits.max_change_bytes)
                            .ok_or(Error::Budget("prepared donor retained changes"))?;
                        if replacements.len() >= limits.max_changes {
                            return Err(Error::Budget("prepared donor replacement count"));
                        }
                        replacements.push((address, kind.to_owned(), order, raw));
                    }
                    Ok(())
                },
            )?);
            if second
                .as_ref()
                .is_none_or(|(address, hash)| *address != count || hash != &expected)
            {
                return Err(Error::Invalid("prepared changed repeatable input"));
            }
            if replacements.len() != donor.changed_documents() {
                return Err(Error::Invalid("prepared donor replacement set differs"));
            }
            let copied = donor.copy_into(&db, maximum, search_max)?;
            let remaining = search_max
                .checked_sub(copied.mutations)
                .filter(|n| *n > 0)
                .ok_or(Error::Budget("prepared donor seal mutations"))?;
            let mut report = if search::header(donor.binding())? == search::header(&binding)? {
                if !replacements.is_empty() {
                    return Err(Error::Invalid(
                        "prepared donor equal binding with replacements",
                    ));
                }
                if db.execute(
                    "UPDATE search_header SET cursor_key=?1 WHERE singleton=1",
                    [&search::cursor_key()?[..]],
                )? != 1
                {
                    return Err(Error::Invalid("prepared donor fresh incarnation seal"));
                }
                search::SearchWriteReport {
                    mutations: 1,
                    write_calls: 1,
                    ..Default::default()
                }
            } else {
                search::apply_delta(
                    &db,
                    donor.binding(),
                    &binding,
                    replacements.into_iter().map(|(address, kind, order, raw)| {
                        check_deadline(deadline)?;
                        Ok(search::SearchChange::Update(
                            search::PreparedSearchDocument::from_item(
                                address,
                                &kind,
                                &parse(&raw, limits.max_row_bytes)?,
                                order,
                            )?,
                        ))
                    }),
                    remaining.min(20_000_000),
                )?
            };
            report.mutations = report
                .mutations
                .checked_add(copied.mutations)
                .ok_or(Error::Budget("prepared donor total mutations"))?;
            report
        } else {
            let produce = |sink: &mut dyn FnMut(search::PreparedSearchDocument) -> Result<()>| {
                second = Some(write_bootstrap_rows(
                    &db,
                    rows,
                    count,
                    limits,
                    deadline,
                    &mut |address, kind, item, order, _raw| {
                        sink(search::PreparedSearchDocument::from_item(
                            address, kind, item, order,
                        )?)
                    },
                )?);
                Ok(())
            };
            if let Some((scratch_path, scratch_limits)) = bulk {
                let report = crate::local_prepared_bulk::initialize_bulk_with(
                    &db,
                    &binding,
                    &scratch_path,
                    scratch_limits,
                    search_max,
                    search_bytes,
                    produce,
                    deadline,
                )?;
                scratch_mutations = report.scratch_mutations;
                report.search
            } else {
                search::initialize_with(&db, &binding, search_max, search_bytes, produce)?
            }
        };
        if second
            .as_ref()
            .is_none_or(|(address, hash)| *address != count || hash != &expected)
        {
            return Err(Error::Invalid("prepared changed repeatable input"));
        }
        let high: u64 = db.query_row(
            "SELECT high_water FROM search_header WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        if high != count {
            return Err(Error::Invalid("prepared search high water"));
        }
        db.execute(
            "INSERT INTO prepared_state VALUES(1,?1,?2,?3)",
            params![
                count as i64,
                maximum as i64,
                compact(&descriptor, limits.max_metadata_bytes)?
            ],
        )?;
        cap(&db, limits, Some(maximum))?;
        if db.total_changes().saturating_add(scratch_mutations) > limits.max_mutations
            || already
                .saturating_add(4 * count + 1)
                .saturating_add(search_report.mutations)
                .saturating_add(scratch_mutations)
                > limits.max_mutations
        {
            return Err(Error::Budget("whole prepared mutations"));
        }
        if let Some(donor) = &mut donor {
            donor.recheck_before_commit()?;
        }
        check_path_stamp(path, &stamp)?;
        check_deadline(deadline)?;
        db.execute_batch("COMMIT")?;
        Ok(binding)
    })();
    if result.is_err() {
        if fs::symlink_metadata(path)
            .is_ok_and(|s| s.dev() == stamp.dev() && s.ino() == stamp.ino())
        {
            fs::remove_file(path)?;
        }
    }
    result
}

fn histograms(lens: &JsonValue) -> Result<[Histogram; 2]> {
    let mut result: [Histogram; 2] = Default::default();
    for (i, name) in ["node_counts", "relation_counts"].iter().enumerate() {
        let cells = field(lens, name)?
            .as_array()
            .filter(|a| a.len() <= 16384)
            .ok_or(Error::Invalid("prepared lens histogram"))?;
        let mut previous: Option<[String; 3]> = None;
        for cell in cells {
            let parts = cell
                .as_array()
                .filter(|a| a.len() == 4)
                .ok_or(Error::Invalid("prepared lens count cell"))?;
            let key = [
                parts[0].as_str().unwrap_or("").to_owned(),
                parts[1].as_str().unwrap_or("").to_owned(),
                parts[2].as_str().unwrap_or("").to_owned(),
            ];
            let count = integer(&parts[3])?;
            if key.iter().any(String::is_empty)
                || count == 0
                || count > MAX_ADDRESS
                || previous.as_ref().is_some_and(|p| p >= &key)
            {
                return Err(Error::Invalid("prepared lens count ordering"));
            }
            previous = Some(key.clone());
            adjust(&mut result[i], key.clone(), true)?;
            let stored = 1;
            if stored < count {
                let before = compact(
                    &JsonValue::Array(vec![
                        text(&key[0]),
                        text(&key[1]),
                        text(&key[2]),
                        number(stored),
                    ]),
                    1_048_576,
                )?
                .len();
                let after = compact(
                    &JsonValue::Array(vec![
                        text(&key[0]),
                        text(&key[1]),
                        text(&key[2]),
                        number(count),
                    ]),
                    1_048_576,
                )?
                .len();
                result[i].bytes = result[i].bytes - before + after;
            }
            result[i].insert(key, count);
        }
    }
    Ok(result)
}
fn integer(value: &JsonValue) -> Result<u64> {
    match value {
        JsonValue::Number(n) if n.kind == JsonNumberKind::Int => n
            .lexeme
            .parse()
            .map_err(|_| Error::Invalid("prepared safe integer")),
        _ => Err(Error::Invalid("prepared safe integer")),
    }
}
pub(crate) fn verify_descriptor(value: &JsonValue, raw: &str, top: &JsonValue) -> Result<()> {
    if compact(value, raw.len())? != raw
        || hash_text(raw) != required(top, "data_revision")?
        || required(value, "schema")? != DESCRIPTOR_SCHEMA
        || required(value, "profile")? != SCHEMA
        || required(value, "algorithm")? != search::ALGORITHM
        || integer(field(value, "search_storage_version")?)? != search::STORAGE_VERSION as u64
        || !same(field(value, "capabilities")?, &capabilities())?
    {
        return Err(Error::Invalid("prepared descriptor migration required"));
    }
    Ok(())
}

/// Addressed normalized successor; the caller MUST rollback its whole
/// transaction on any error. No commit, graph scan, index install or migration.
pub fn apply_prepared_delta_transaction<I: IntoIterator<Item = PreparedChange>>(
    db: &Connection,
    expected: &JsonValue,
    header: &JsonValue,
    catalog: &JsonValue,
    changes: I,
    limits: PublicationLimits,
) -> Result<JsonValue> {
    apply_prepared_delta_transaction_fallible(
        db,
        expected,
        header,
        catalog,
        changes.into_iter().map(Ok),
        limits,
    )
}
/// Fallible streaming transport seam; a framing/parse error has the same
/// whole-transaction rollback obligation as a storage refusal.
pub fn apply_prepared_delta_transaction_fallible<I: IntoIterator<Item = Result<PreparedChange>>>(
    db: &Connection,
    expected: &JsonValue,
    header: &JsonValue,
    catalog: &JsonValue,
    changes: I,
    limits: PublicationLimits,
) -> Result<JsonValue> {
    if db.is_autocommit() {
        return Err(Error::Invalid("prepared caller transaction required"));
    }
    limits.validate()?;
    validate_header(header, catalog)?;
    let top = metadata(db, TOP, 65536)?;
    let actual = snapshot_binding(db)?;
    if !same(&actual, expected)? || required(&top, "read_model_schema")? != SCHEMA {
        return Err(Error::Invalid(
            "stale or foreign prepared publication binding",
        ));
    }
    if metadata(db, "data_revision", 1024)?
        != object(vec![("sha256", field(&top, "data_revision")?.clone())])
    {
        return Err(Error::Invalid("prepared data revision"));
    }
    if !same(
        field(header, "normalization_binding")?,
        field(&top, "normalization_binding")?,
    )? {
        return Err(Error::Invalid("prepared normalization bootstrap required"));
    }
    let epoch = integer(field(&actual, "publication_epoch")?)?;
    if epoch >= MAX_ADDRESS {
        return Err(Error::Budget("prepared epoch exhausted"));
    }
    let (mut high,retained,raw):(u64,u64,Option<String>)=db.query_row("SELECT high_water,max_pages,CASE WHEN length(CAST(descriptor AS BLOB))<=?1 THEN descriptor ELSE NULL END FROM prepared_state WHERE singleton=1",[limits.max_metadata_bytes as u64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let raw = raw.ok_or(Error::Budget("prepared descriptor"))?;
    verify_descriptor(&parse(&raw, limits.max_metadata_bytes)?, &raw, &top)?;
    if high > MAX_ADDRESS
        || db.query_row(
            "SELECT high_water FROM search_header WHERE singleton=1",
            [],
            |r| r.get::<_, u64>(0),
        )? != high
    {
        return Err(Error::Invalid("prepared search high water"));
    }
    let maximum = cap(db, limits, Some(retained))?;
    let before = db.total_changes();
    let aux = auxiliary::validate(db, expected)?;
    let previous_lens = metadata(db, LENS, 1_048_576)?;
    if digest(&previous_lens, 1_048_576)? != required(&top, "lens_sha256")?
        || field(&previous_lens, "source_revision")? != field(&top, "source_revision")?
    {
        return Err(Error::Invalid("prepared lens digest"));
    }
    if previous_lens.as_object().is_none_or(|o| o.len() != 8)
        || required(&previous_lens, "schema")? != "tos_published_lens_metadata_v1"
        || required(&previous_lens, "execution_version")? != "tos-lens-execution-v7"
        || required(&previous_lens, "sort_key")? != "python-str-or-empty-lower-v1"
        || required(&previous_lens, "unicode_version")? != "16.0.0"
    {
        return Err(Error::Invalid("prepared prior lens metadata profile"));
    }
    let mut hist = histograms(&previous_lens)?;
    let old_header = object(vec![
        (
            "source_revision",
            field(&previous_lens, "source_revision")?.clone(),
        ),
        (
            "query_properties",
            field(&previous_lens, "query_properties")?.clone(),
        ),
    ]);
    if compact(&lens(&old_header, &hist)?, 1_048_576)? != compact(&previous_lens, 1_048_576)? {
        return Err(Error::Invalid("prepared prior lens metadata framing"));
    }
    let mut seen = BTreeSet::new();
    let mut frames = Vec::new();
    let mut search_changes: Vec<(String, u64, String, u64, Option<String>)> = Vec::new();
    let mut deleted_nodes = Vec::new();
    let mut changed_relations = Vec::new();
    let mut changed_bytes = 0usize;
    let mut frame_bytes = 0usize;
    for change in changes {
        let change = change?;
        if seen.len() >= limits.max_changes {
            return Err(Error::Budget("prepared changes"));
        }
        let kind = change.kind.as_str();
        columns(kind)?;
        if !matches!(change.operation.as_str(), "insert" | "update" | "delete")
            || change.identifier.is_empty()
            || change.identifier.chars().count() > 4096
            || !seen.insert((kind.to_owned(), change.identifier.clone()))
        {
            return Err(Error::Invalid("prepared duplicate or invalid change"));
        }
        let found: Option<(u64, u64)> = db
            .query_row(
                "SELECT doc_id,source_order FROM prepared_documents WHERE kind=?1 AND id=?2",
                params![kind, change.identifier],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if found.is_none() != (change.operation == "insert") {
            return Err(Error::Invalid("prepared addressed operation"));
        }
        let old = if let Some((address, _)) = found {
            let old:Option<String>=db.query_row(&format!("SELECT CASE WHEN length(CAST(json AS BLOB))<=?1 THEN json ELSE NULL END FROM knowledge_{kind}s WHERE id=?2"),params![limits.max_row_bytes as u64,change.identifier],|r|r.get(0))?;
            let old = old.ok_or(Error::Budget("prepared old carrier"))?;
            if metadata(
                db,
                &format!("knowledge_{kind}_digest:{}", change.identifier),
                1024,
            )? != sha_value(&old)
            {
                return Err(Error::Invalid("prepared old carrier checksum"));
            }
            let value = parse(&old, limits.max_row_bytes)?;
            if required(&value, "id")? != change.identifier {
                return Err(Error::Invalid("prepared old identity"));
            }
            let (search_kind,id):(String,Vec<u8>)=db.query_row("SELECT CASE WHEN length(CAST(kind AS BLOB))<=8 THEN kind ELSE NULL END,CASE WHEN length(identifier)<=?1 THEN identifier ELSE NULL END FROM search_documents WHERE doc_id=?2",params![32768,address as i64],|r|Ok((r.get(0)?,r.get(1)?)))?;
            let exact_id = compact(&text(&change.identifier), 32768)?;
            if search_kind != kind || id != exact_id.as_bytes() {
                return Err(Error::Invalid("prepared search identity"));
            }
            Some(value)
        } else {
            None
        };
        let (address, token, raw) = if change.operation == "delete" {
            if change.item.is_some() || change.source_order.is_some() {
                return Err(Error::Invalid("prepared deletion replacement"));
            }
            let (address, token) = found.ok_or(Error::Invalid("prepared delete absent"))?;
            db.execute(
                &format!("DELETE FROM knowledge_{kind}s WHERE id=?"),
                [&change.identifier],
            )?;
            db.execute(
                "DELETE FROM prepared_documents WHERE kind=?1 AND id=?2",
                params![kind, change.identifier],
            )?;
            db.execute(
                "DELETE FROM knowledge_lens_order WHERE kind=?1 AND id=?2",
                params![kind, change.identifier],
            )?;
            db.execute(
                "DELETE FROM edge_meta WHERE key=?",
                [format!("knowledge_{kind}_digest:{}", change.identifier)],
            )?;
            search_changes.push((
                change.operation.clone(),
                address,
                kind.to_owned(),
                token,
                None,
            ));
            if kind == "node" {
                deleted_nodes.push(change.identifier.clone());
            }
            (address, token, None)
        } else {
            let item = change
                .item
                .as_ref()
                .ok_or(Error::Invalid("prepared replacement absent"))?;
            let raw = row(kind, item, limits)?;
            changed_bytes = changed_bytes
                .checked_add(raw.len())
                .filter(|n| *n <= limits.max_change_bytes)
                .ok_or(Error::Budget("prepared changed bytes"))?;
            if required(item, "id")? != change.identifier {
                return Err(Error::Invalid("prepared replacement identity"));
            }
            let (address, token) = if let Some((address, token)) = found {
                (address, change.source_order.unwrap_or(token))
            } else {
                high = high
                    .checked_add(1)
                    .filter(|n| *n <= MAX_ADDRESS)
                    .ok_or(Error::Budget("prepared address exhausted"))?;
                (
                    high,
                    change
                        .source_order
                        .ok_or(Error::Invalid("prepared insert order required"))?,
                )
            };
            if token > MAX_ADDRESS {
                return Err(Error::Invalid("prepared source order"));
            }
            put_row(db, kind, item, &raw, limits)?;
            db.execute("INSERT INTO prepared_documents VALUES(?1,?2,?3,?4) ON CONFLICT(kind,id) DO UPDATE SET source_order=excluded.source_order",params![kind,change.identifier,address as i64,token as i64])?;
            if kind == "relation" {
                changed_relations.push((
                    required(item, "from_id")?.to_owned(),
                    required(item, "to_id")?.to_owned(),
                ));
            }
            (address, token, Some(raw))
        };
        auxiliary::put(db, aux, kind, &change.identifier, raw.as_deref())?;
        let index = usize::from(kind == "relation");
        if let Some(old) = old.as_ref() {
            adjust(&mut hist[index], cell(kind, old)?, false)?;
        }
        if let Some(item) = change.item.as_ref() {
            adjust(&mut hist[index], cell(kind, item)?, true)?;
        }
        let next_frame = JsonValue::Array(vec![
            text(&change.operation),
            text(kind),
            text(&change.identifier),
            number(address),
            number(token),
            raw.as_ref()
                .map(|r| text(&hash_text(r)))
                .unwrap_or(JsonValue::Null),
        ]);
        frame_bytes = frame_bytes
            .checked_add(compact(&next_frame, limits.max_metadata_bytes)?.len() + 1)
            .filter(|n| *n <= limits.max_metadata_bytes)
            .ok_or(Error::Budget("prepared delta descriptor frames"))?;
        frames.push(next_frame);
        if change.operation != "delete" {
            search_changes.push((
                change.operation.clone(),
                address,
                kind.to_owned(),
                token,
                raw,
            ));
        }
    }
    for (from, to) in changed_relations {
        endpoint(db, &from)?;
        endpoint(db, &to)?;
    }
    for id in deleted_nodes {
        if db.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_relations WHERE from_id=?1 UNION ALL SELECT 1 FROM knowledge_relations WHERE to_id=?1)",[id],|r|r.get::<_,bool>(0))?{return Err(Error::Invalid("prepared node deletion incidence"));}
    }
    let lens = lens(header, &hist)?;
    let descriptor = descriptor(
        header,
        catalog,
        vec![
            (
                "parent_data_revision",
                field(&top, "data_revision")?.clone(),
            ),
            ("changes", JsonValue::Array(frames)),
        ],
        limits,
    )?;
    let binding = publish_header(db, header, catalog, &lens, &descriptor, epoch + 1, limits)?;
    auxiliary::seal(db, aux, &binding)?;
    let before_search = db.total_changes().saturating_sub(before);
    let search_max = limits
        .max_mutations
        .checked_sub(before_search)
        .and_then(|n| n.checked_sub(1))
        .ok_or(Error::Budget(
            "whole prepared delta mutations before search",
        ))?;
    let search_report = search::apply_delta(
        db,
        expected,
        &binding,
        search_changes
            .into_iter()
            .map(|(op, address, kind, token, raw)| {
                if op == "delete" {
                    return Ok(search::SearchChange::Delete { doc_id: address });
                }
                let value = parse(
                    raw.as_deref()
                        .ok_or(Error::Invalid("prepared search changed carrier"))?,
                    limits.max_row_bytes,
                )?;
                let doc = search::PreparedSearchDocument::from_item(address, &kind, &value, token)?;
                Ok(if op == "insert" {
                    search::SearchChange::Insert(doc)
                } else {
                    search::SearchChange::Update(doc)
                })
            }),
        search_max,
    )?;
    if db.query_row(
        "SELECT high_water FROM search_header WHERE singleton=1",
        [],
        |r| r.get::<_, u64>(0),
    )? != high
    {
        return Err(Error::Invalid("prepared successor high water"));
    }
    db.execute(
        "UPDATE prepared_state SET high_water=?1,max_pages=?2,descriptor=?3 WHERE singleton=1",
        params![
            high as i64,
            maximum as i64,
            compact(&descriptor, limits.max_metadata_bytes)?
        ],
    )?;
    cap(db, limits, Some(maximum))?;
    if db.total_changes().saturating_sub(before) > limits.max_mutations
        || before_search
            .saturating_add(search_report.mutations)
            .saturating_add(1)
            > limits.max_mutations
    {
        return Err(Error::Budget("whole prepared delta mutations"));
    }
    Ok(binding)
}

/// File owner wrapper. One immediate transaction rolls back all lanes on error.
pub fn apply_prepared_delta<I: IntoIterator<Item = PreparedChange>>(
    path: &Path,
    expected: &JsonValue,
    header: &JsonValue,
    catalog: &JsonValue,
    changes: I,
    limits: PublicationLimits,
) -> Result<JsonValue> {
    apply_prepared_delta_with_deadline(
        path,
        expected,
        header,
        catalog,
        changes.into_iter().map(Ok),
        limits,
        None,
    )
}
pub fn apply_prepared_delta_until<I: IntoIterator<Item = Result<PreparedChange>>>(
    path: &Path,
    expected: &JsonValue,
    header: &JsonValue,
    catalog: &JsonValue,
    changes: I,
    limits: PublicationLimits,
    deadline: Instant,
) -> Result<JsonValue> {
    apply_prepared_delta_with_deadline(
        path,
        expected,
        header,
        catalog,
        changes,
        limits,
        Some(deadline),
    )
}
fn apply_prepared_delta_with_deadline<I: IntoIterator<Item = Result<PreparedChange>>>(
    path: &Path,
    expected: &JsonValue,
    header: &JsonValue,
    catalog: &JsonValue,
    changes: I,
    limits: PublicationLimits,
    deadline: Option<Instant>,
) -> Result<JsonValue> {
    limits.validate()?;
    validate_header(header, catalog)?;
    check_deadline(deadline)?;
    let stamp = fs::symlink_metadata(path)?;
    if !stamp.is_file() || stamp.file_type().is_symlink() {
        return Err(Error::Invalid("prepared regular file"));
    }
    let db = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let current = fs::symlink_metadata(path)?;
    if current.dev() != stamp.dev() || current.ino() != stamp.ino() {
        return Err(Error::Invalid("prepared file changed at open"));
    }
    install_deadline(&db, deadline);
    db.execute_batch("BEGIN IMMEDIATE")?;
    check_path_stamp(path, &stamp)?;
    match apply_prepared_delta_transaction_fallible(&db, expected, header, catalog, changes, limits)
    {
        Ok(binding) => {
            check_path_stamp(path, &stamp)?;
            check_deadline(deadline)?;
            db.execute_batch("COMMIT")?;
            Ok(binding)
        }
        Err(error) => {
            if !db.is_autocommit() {
                let _ = db.execute_batch("ROLLBACK");
            }
            Err(error)
        }
    }
}

fn check_deadline(deadline: Option<Instant>) -> Result<()> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        Err(Error::Budget("prepared whole operation deadline"))
    } else {
        Ok(())
    }
}
fn install_deadline(db: &Connection, deadline: Option<Instant>) {
    if let Some(deadline) = deadline {
        db.progress_handler(1000, Some(move || Instant::now() >= deadline));
    }
}

fn check_path_stamp(path: &Path, expected: &fs::Metadata) -> Result<()> {
    let actual = fs::symlink_metadata(path)?;
    if !actual.is_file()
        || actual.file_type().is_symlink()
        || actual.dev() != expected.dev()
        || actual.ino() != expected.ino()
    {
        Err(Error::Invalid("prepared file changed during transaction"))
    } else {
        Ok(())
    }
}
