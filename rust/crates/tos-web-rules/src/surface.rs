//! The surface an agent shapes on the person's page while they watch
//! (TOS-D-0089, access/contracts/surface-spec.v1.json): blocks in a few
//! places, bound to data the page already reads, changed by small atomic
//! operations. The same rules run in the browser and at the bridge, so a patch
//! is checked once and drawn the same wherever the person watches.
//!
//! A surface is presentation only. It never writes ToS source, review, rights
//! or canon; it carries text, addresses and choices from the contract, never
//! markup, styles, scripts or outside links; every block is the work of who
//! shaped it, and the person freezes, undoes or clears it at any time.
//!
//! An agent works it like clay: `patch` applies a list of operations all or
//! none and answers with what changed; `shape` turns a source's answer into
//! rows; `view` turns a block and its rows into exactly what the page draws,
//! naming every field a template asked for and no row had.

use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::sync::OnceLock;

const CONTRACT: &str = include_str!("../../../../access/contracts/surface-spec.v1.json");
pub const SURFACE_SCHEMA: &str = "tos.surface.v1";

const JOURNAL_VIEWS: &[&str] = &[
    "summary", "act", "target", "board", "review", "decided", "leases", "presence", "history",
    "kinds", "squads",
];
const FIELD_SHAPES: &[&str] = &["thought", "link", "lens", "route"];
const WHERE_OPS: &[&str] = &["eq", "ne", "gt", "gte", "lt", "lte", "has", "in"];
const TRANSFORM_KEYS: &[&str] = &["source", "path", "where", "sort", "count_by", "limit"];
const OPS: &[&str] = &["put", "set", "move", "remove", "clear", "title"];
const DEFAULT_ROWS: usize = 50;
const MAX_AUTHORS: usize = 8;
const MAX_TITLE: usize = 200;
const MAX_ADDRESS: usize = 1_024;
const MAX_INPUT_BYTES: usize = 4_096;
const MAX_IN_VALUES: usize = 64;
const MAX_WINDOW_MS: u64 = 30 * 86_400_000;
const NOTE_ROWS: usize = 20;

// ---------------------------------------------------------------- contract

#[derive(Clone, Copy, Debug)]
pub struct SurfaceLimits {
    pub blocks: usize,
    pub depth: usize,
    pub ops_per_patch: usize,
    pub surface_bytes: usize,
    pub text_chars: usize,
    pub template_chars: usize,
    pub id_chars: usize,
    pub members: usize,
    pub rows: usize,
    pub columns: usize,
    pub where_terms: usize,
    pub history: usize,
}

struct BlockRule {
    kind: String,
    required: Vec<String>,
    optional: Vec<String>,
}

struct Contract {
    raw: Value,
    places: Vec<(String, Vec<String>)>,
    blocks: Vec<BlockRule>,
    tones: Vec<String>,
    commands: Vec<String>,
    sources: Vec<String>,
    limits: SurfaceLimits,
}

fn names(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn contract() -> &'static Contract {
    static CELL: OnceLock<Contract> = OnceLock::new();
    CELL.get_or_init(|| {
        let raw: Value = serde_json::from_str(CONTRACT).expect("surface-spec.v1.json is JSON");
        let limit = |name: &str| raw["limits"][name].as_u64().unwrap_or(0) as usize;
        let rows = |key: &str| raw[key].as_array().cloned().unwrap_or_default();
        Contract {
            places: rows("places")
                .iter()
                .map(|place| {
                    (
                        place["name"].as_str().unwrap_or_default().to_owned(),
                        names(&place["blocks"]),
                    )
                })
                .collect(),
            blocks: rows("blocks")
                .iter()
                .map(|block| BlockRule {
                    kind: block["kind"].as_str().unwrap_or_default().to_owned(),
                    required: names(&block["required"]),
                    optional: names(&block["optional"]),
                })
                .collect(),
            tones: names(&raw["tones"]),
            commands: names(&raw["commands"]),
            sources: rows("sources")
                .iter()
                .filter_map(|source| source["name"].as_str().map(str::to_owned))
                .collect(),
            limits: SurfaceLimits {
                blocks: limit("blocks"),
                depth: limit("depth"),
                ops_per_patch: limit("ops_per_patch"),
                surface_bytes: limit("surface_bytes"),
                text_chars: limit("text_chars"),
                template_chars: limit("template_chars"),
                id_chars: limit("id_chars"),
                members: limit("members"),
                rows: limit("rows"),
                columns: limit("columns"),
                where_terms: limit("where"),
                history: limit("history"),
            },
            raw,
        }
    })
}

/// The limits of the surface contract.
pub fn surface_limits() -> SurfaceLimits {
    contract().limits
}

/// The surface contract as the agent reads it first.
pub fn surface_contract() -> &'static Value {
    &contract().raw
}

// ---------------------------------------------------------------- errors

/// A refused patch: a code, a sentence that says what to do instead, the
/// operation it stopped at and where in the surface.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceError {
    pub code: &'static str,
    pub message: String,
    pub op: Option<usize>,
    pub at: String,
}

impl SurfaceError {
    fn new(code: &'static str, at: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            op: None,
            at: at.into(),
        }
    }
    fn at_op(mut self, op: usize) -> Self {
        self.op.get_or_insert(op);
        self
    }
    pub fn to_json(&self) -> Value {
        let mut out = json!({"error": self.code, "message": self.message});
        if let Some(op) = self.op {
            out["op"] = json!(op);
        }
        if !self.at.is_empty() {
            out["at"] = json!(self.at);
        }
        out
    }
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_json())
    }
}

type Result<T> = std::result::Result<T, SurfaceError>;

fn listing(items: &[impl AsRef<str>]) -> String {
    items
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------- checking

fn valid_id(id: &str, limit: usize) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= limit
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn plain(value: &Value, at: &str, name: &str, limit: usize, required: bool) -> Result<String> {
    let text = value.as_str().ok_or_else(|| {
        SurfaceError::new("surface_text", at, format!("{name} is text; give a string"))
    })?;
    if text.chars().count() > limit {
        return Err(SurfaceError::new(
            "surface_text",
            at,
            format!("{name} holds at most {limit} characters"),
        ));
    }
    if required && text.trim().is_empty() {
        return Err(SurfaceError::new(
            "surface_text",
            at,
            format!("{name} may not be empty"),
        ));
    }
    if text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(SurfaceError::new(
            "surface_text",
            at,
            format!("{name} carries no control characters"),
        ));
    }
    Ok(text.to_owned())
}

/// The placeholders of a template, or why it does not read.
fn placeholders(template: &str) -> std::result::Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) if c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '#') => {
                            name.push(c)
                        }
                        Some(c) => return Err(format!("'{c}' cannot stand in a placeholder")),
                        None => return Err("a placeholder is not closed with }".to_owned()),
                    }
                }
                if name.is_empty() {
                    return Err("an empty placeholder {}; write {{ for a brace".to_owned());
                }
                out.push(name);
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
            }
            '}' => return Err("a lone }; write }} for a brace".to_owned()),
            _ => {}
        }
    }
    Ok(out)
}

fn template(value: &Value, at: &str, name: &str, limit: usize, required: bool) -> Result<String> {
    let text = plain(value, at, name, limit, required)?;
    placeholders(&text)
        .map_err(|why| SurfaceError::new("surface_template", at, format!("{name}: {why}")))?;
    Ok(text)
}

fn address(value: &Value, at: &str) -> Result<String> {
    let text = value
        .as_str()
        .filter(|text| {
            !text.trim().is_empty()
                && text.len() <= MAX_ADDRESS
                && !text.chars().any(char::is_control)
        })
        .ok_or_else(|| {
            SurfaceError::new(
                "surface_address",
                at,
                "an address is a non-empty string without control characters",
            )
        })?;
    Ok(text.trim().to_owned())
}

fn path_name(value: &Value, at: &str, name: &str) -> Result<String> {
    let text = value
        .as_str()
        .filter(|text| {
            !text.is_empty()
                && text.len() <= 200
                && text
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '$'))
        })
        .ok_or_else(|| {
            SurfaceError::new(
                "surface_field_name",
                at,
                format!("{name} is a field name or a dotted path such as groups or author.id"),
            )
        })?;
    Ok(text.to_owned())
}

fn scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

struct Checker<'a> {
    c: &'a Contract,
    ids: HashSet<String>,
    count: usize,
}

impl Checker<'_> {
    fn rule(&self, kind: &str) -> Option<&BlockRule> {
        self.c.blocks.iter().find(|rule| rule.kind == kind)
    }

    fn block(&mut self, value: &Value, place: &str, depth: usize, at: &str) -> Result<Value> {
        let limits = self.c.limits;
        let object = value.as_object().ok_or_else(|| {
            SurfaceError::new("surface_block", at, "a block is an object {id, kind, …}")
        })?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| valid_id(id, limits.id_chars))
            .ok_or_else(|| {
                SurfaceError::new(
                    "surface_id",
                    at,
                    format!(
                        "a block needs an id of letters, digits, '.', '_' or '-', at most {} characters, starting with a letter or digit",
                        limits.id_chars
                    ),
                )
            })?
            .to_owned();
        let at = format!("{at}/{id}");
        if !self.ids.insert(id.clone()) {
            return Err(SurfaceError::new(
                "surface_id_taken",
                &at,
                format!(
                    "another block already has the id {id}; choose another or put to replace it"
                ),
            ));
        }
        self.count += 1;
        if self.count > limits.blocks {
            return Err(SurfaceError::new(
                "surface_blocks",
                &at,
                format!("a surface holds at most {} blocks", limits.blocks),
            ));
        }
        if depth > limits.depth {
            return Err(SurfaceError::new(
                "surface_depth",
                &at,
                format!("groups nest at most {} deep", limits.depth),
            ));
        }
        let allowed = &self
            .c
            .places
            .iter()
            .find(|(name, _)| name == place)
            .map(|(_, kinds)| kinds.clone())
            .unwrap_or_default();
        let kind = object.get("kind").and_then(Value::as_str).unwrap_or("");
        let Some(rule) = self.rule(kind) else {
            return Err(SurfaceError::new(
                "surface_kind",
                &at,
                format!(
                    "'{kind}' is not a block; blocks of {place}: {}",
                    listing(allowed)
                ),
            ));
        };
        if !allowed.iter().any(|name| name == kind) {
            return Err(SurfaceError::new(
                "surface_kind",
                &at,
                format!(
                    "{kind} does not stand in {place}; blocks of {place}: {}",
                    listing(allowed)
                ),
            ));
        }
        let fields: Vec<&String> = rule.required.iter().chain(&rule.optional).collect();
        for key in object.keys() {
            if key != "id" && key != "kind" && !fields.iter().any(|name| *name == key) {
                return Err(SurfaceError::new(
                    "surface_field",
                    &at,
                    format!(
                        "{kind} has no field '{key}'; its fields: {}",
                        listing(&fields)
                    ),
                ));
            }
        }
        for name in &rule.required {
            if object.get(name).is_none_or(Value::is_null) {
                return Err(SurfaceError::new(
                    "surface_required",
                    &at,
                    format!("{kind} needs {name}; it needs {}", listing(&rule.required)),
                ));
            }
        }
        let mut out = Map::new();
        out.insert("id".into(), json!(id));
        out.insert("kind".into(), json!(kind));
        for (key, value) in object {
            if key == "id" || key == "kind" || value.is_null() {
                continue;
            }
            let field_at = format!("{at}.{key}");
            let normal = self.field(kind, key, value, place, depth, &field_at)?;
            out.insert(key.clone(), normal);
        }
        let has = |name: &str| out.contains_key(name);
        let needs = |ok: bool, message: &str| {
            if ok {
                Ok(())
            } else {
                Err(SurfaceError::new("surface_required", &at, message))
            }
        };
        match kind {
            "metric" => needs(
                has("value") || has("bind"),
                "metric needs a value or a bind",
            )?,
            "chips" => needs(
                has("members") || (has("bind") && has("target")),
                "chips need members, or a bind with a target template",
            )?,
            "highlight" => needs(
                has("members") || (has("bind") && has("target")),
                "highlight needs members, or a bind with a target template",
            )?,
            "links" => needs(
                has("relations") || (has("bind") && has("target")),
                "links need relations, or a bind with a target template",
            )?,
            _ => {}
        }
        Ok(Value::Object(out))
    }

    fn field(
        &mut self,
        kind: &str,
        name: &str,
        value: &Value,
        place: &str,
        depth: usize,
        at: &str,
    ) -> Result<Value> {
        let limits = self.c.limits;
        Ok(match (kind, name) {
            ("bars", "value") => json!(path_name(value, at, "value")?),
            ("metric", "value") => match value {
                Value::Number(_) => value.clone(),
                _ => json!(plain(value, at, "value", MAX_TITLE, true)?),
            },
            (_, "text") => json!(template(value, at, name, limits.text_chars, true)?),
            (_, "title" | "label" | "meta" | "target" | "empty") => {
                json!(template(
                    value,
                    at,
                    name,
                    limits.template_chars,
                    name != "meta"
                )?)
            }
            (_, "tone") => {
                let tone = value.as_str().unwrap_or("");
                if !self.c.tones.iter().any(|t| t == tone) {
                    return Err(SurfaceError::new(
                        "surface_tone",
                        at,
                        format!("tone is one of {}", listing(&self.c.tones)),
                    ));
                }
                json!(tone)
            }
            (_, "field") => json!(path_name(value, at, "field")?),
            (_, "open" | "dim_rest") => json!(value.as_bool().ok_or_else(|| {
                SurfaceError::new("surface_flag", at, format!("{name} is true or false"))
            })?),
            (_, "command") => {
                let command = value.as_str().unwrap_or("");
                if !self.c.commands.iter().any(|c| c == command) {
                    return Err(SurfaceError::new(
                        "surface_command",
                        at,
                        format!("a button runs one of {}", listing(&self.c.commands)),
                    ));
                }
                json!(command)
            }
            (_, "input") => {
                if !value.is_object() || value.to_string().len() > MAX_INPUT_BYTES {
                    return Err(SurfaceError::new(
                        "surface_input",
                        at,
                        format!("input is an object of at most {MAX_INPUT_BYTES} bytes"),
                    ));
                }
                value.clone()
            }
            (_, "members" | "relations") => {
                let items = value.as_array().ok_or_else(|| {
                    SurfaceError::new(
                        "surface_members",
                        at,
                        format!("{name} is a list of addresses"),
                    )
                })?;
                if items.len() > limits.members {
                    return Err(SurfaceError::new(
                        "surface_members",
                        at,
                        format!("{name} holds at most {} addresses", limits.members),
                    ));
                }
                let mut seen = BTreeSet::new();
                let mut out = Vec::new();
                for item in items {
                    let item = address(item, at)?;
                    if seen.insert(item.clone()) {
                        out.push(json!(item));
                    }
                }
                Value::Array(out)
            }
            (_, "columns") => {
                let items = value
                    .as_array()
                    .filter(|items| !items.is_empty() && items.len() <= limits.columns)
                    .ok_or_else(|| {
                        SurfaceError::new(
                            "surface_columns",
                            at,
                            format!(
                                "columns are 1 to {} of {{field, title}} or field names",
                                limits.columns
                            ),
                        )
                    })?;
                let mut out = Vec::new();
                for item in items {
                    let (field, title) = match item {
                        Value::String(_) => (path_name(item, at, "a column")?, None),
                        Value::Object(column) => (
                            path_name(
                                column.get("field").unwrap_or(&Value::Null),
                                at,
                                "a column's field",
                            )?,
                            column
                                .get("title")
                                .map(|title| plain(title, at, "a column's title", MAX_TITLE, true))
                                .transpose()?,
                        ),
                        _ => {
                            return Err(SurfaceError::new(
                                "surface_columns",
                                at,
                                "a column is {field, title} or a field name",
                            ));
                        }
                    };
                    out.push(
                        json!({"title": title.unwrap_or_else(|| field.clone()), "field": field}),
                    );
                }
                Value::Array(out)
            }
            (_, "bind") => self.bind(value, at)?,
            ("group", "children") => {
                let items = value.as_array().ok_or_else(|| {
                    SurfaceError::new("surface_children", at, "children is a list of blocks")
                })?;
                let mut out = Vec::new();
                for item in items {
                    out.push(self.block(item, place, depth + 1, at)?);
                }
                Value::Array(out)
            }
            _ => {
                return Err(SurfaceError::new(
                    "surface_field",
                    at,
                    format!("{kind} has no field '{name}'"),
                ));
            }
        })
    }

    fn bind(&self, value: &Value, at: &str) -> Result<Value> {
        let limits = self.c.limits;
        let object = value.as_object().ok_or_else(|| {
            SurfaceError::new("surface_bind", at, "bind is an object {source, …}")
        })?;
        let source = object.get("source").and_then(Value::as_str).unwrap_or("");
        if !self.c.sources.iter().any(|name| name == source) {
            return Err(SurfaceError::new(
                "surface_source",
                at,
                format!("bind.source is one of {}", listing(&self.c.sources)),
            ));
        }
        let params: &[&str] = match source {
            "journal" => &["view", "address", "id", "window_ms"],
            "search" => &["query"],
            "item" => &["id"],
            "field" => &["shape", "kind"],
            "rows" => &["rows"],
            _ => &[],
        };
        for key in object.keys() {
            if !TRANSFORM_KEYS.contains(&key.as_str()) && !params.contains(&key.as_str()) {
                let mut known: Vec<&str> = params.to_vec();
                known.extend(&TRANSFORM_KEYS[1..]);
                return Err(SurfaceError::new(
                    "surface_bind",
                    at,
                    format!(
                        "a {source} bind has no '{key}'; it takes {}",
                        listing(&known)
                    ),
                ));
            }
        }
        let mut out = Map::new();
        out.insert("source".into(), json!(source));
        let text = |key: &str, required: bool| -> Result<Option<String>> {
            match object.get(key) {
                None | Some(Value::Null) if required => Err(SurfaceError::new(
                    "surface_bind",
                    at,
                    format!("a {source} bind needs {key}"),
                )),
                None | Some(Value::Null) => Ok(None),
                Some(value) => Ok(Some(address(value, &format!("{at}.{key}"))?)),
            }
        };
        match source {
            "journal" => {
                let view = text("view", true)?.unwrap_or_default();
                if !JOURNAL_VIEWS.contains(&view.as_str()) {
                    return Err(SurfaceError::new(
                        "surface_bind",
                        at,
                        format!(
                            "a journal bind reads one view of {}",
                            JOURNAL_VIEWS.join(", ")
                        ),
                    ));
                }
                out.insert("view".into(), json!(view));
                for key in ["address", "id"] {
                    if let Some(value) = text(key, false)? {
                        out.insert(key.into(), json!(value));
                    }
                }
                if let Some(window) = object.get("window_ms").filter(|v| !v.is_null()) {
                    let ms = window
                        .as_u64()
                        .filter(|ms| *ms <= MAX_WINDOW_MS)
                        .ok_or_else(|| {
                            SurfaceError::new(
                                "surface_bind",
                                at,
                                "window_ms is milliseconds up to 30 days",
                            )
                        })?;
                    out.insert("window_ms".into(), json!(ms));
                }
            }
            "search" => {
                let query = plain(
                    object.get("query").unwrap_or(&Value::Null),
                    at,
                    "query",
                    200,
                    true,
                )?;
                out.insert("query".into(), json!(query));
            }
            "item" => {
                out.insert("id".into(), json!(text("id", true)?));
            }
            "field" => {
                if let Some(shape) = object.get("shape").filter(|v| !v.is_null()) {
                    let shape = shape
                        .as_str()
                        .filter(|s| FIELD_SHAPES.contains(s))
                        .ok_or_else(|| {
                            SurfaceError::new(
                                "surface_bind",
                                at,
                                format!(
                                    "a field bind's shape is one of {}",
                                    FIELD_SHAPES.join(", ")
                                ),
                            )
                        })?;
                    out.insert("shape".into(), json!(shape));
                }
                if let Some(kind) = object.get("kind").filter(|v| !v.is_null()) {
                    let kind = kind
                        .as_str()
                        .filter(|k| k.starts_with("x.") && k.len() <= 200)
                        .ok_or_else(|| {
                            SurfaceError::new(
                                "surface_bind",
                                at,
                                "a field bind's kind is an x.* kind",
                            )
                        })?;
                    out.insert("kind".into(), json!(kind));
                }
            }
            "rows" => {
                let rows = object
                    .get("rows")
                    .and_then(Value::as_array)
                    .filter(|rows| rows.len() <= limits.rows && rows.iter().all(Value::is_object))
                    .ok_or_else(|| {
                        SurfaceError::new(
                            "surface_bind",
                            at,
                            format!(
                                "a rows bind gives rows: a list of at most {} objects",
                                limits.rows
                            ),
                        )
                    })?;
                out.insert("rows".into(), Value::Array(rows.clone()));
            }
            _ => {}
        }
        if let Some(path) = object.get("path").filter(|v| !v.is_null()) {
            out.insert("path".into(), json!(path_name(path, at, "path")?));
        }
        if let Some(terms) = object.get("where").filter(|v| !v.is_null()) {
            let terms = terms
                .as_array()
                .filter(|terms| terms.len() <= limits.where_terms)
                .ok_or_else(|| {
                    SurfaceError::new(
                        "surface_where",
                        at,
                        format!(
                            "where is a list of at most {} {{field, op, value}}",
                            limits.where_terms
                        ),
                    )
                })?;
            let mut normal = Vec::new();
            for term in terms {
                let field = path_name(&term["field"], at, "a where field")?;
                let op = term["op"].as_str().unwrap_or("eq");
                if !WHERE_OPS.contains(&op) {
                    return Err(SurfaceError::new(
                        "surface_where",
                        at,
                        format!("a where op is one of {}", WHERE_OPS.join(", ")),
                    ));
                }
                let value = term.get("value").cloned().unwrap_or(Value::Null);
                let fits = if op == "in" {
                    value.as_array().is_some_and(|items| {
                        items.len() <= MAX_IN_VALUES && items.iter().all(scalar)
                    })
                } else {
                    scalar(&value)
                };
                if !fits {
                    return Err(SurfaceError::new(
                        "surface_where",
                        at,
                        format!(
                            "a where value is text, a number, true, false or null; for in, a list of at most {MAX_IN_VALUES} of them"
                        ),
                    ));
                }
                normal.push(json!({"field": field, "op": op, "value": value}));
            }
            out.insert("where".into(), Value::Array(normal));
        }
        if let Some(sort) = object.get("sort").filter(|v| !v.is_null()) {
            let (field, desc) = match sort {
                Value::String(text) => match text.strip_prefix('-') {
                    Some(rest) => (path_name(&json!(rest), at, "sort")?, true),
                    None => (path_name(sort, at, "sort")?, false),
                },
                _ => (
                    path_name(&sort["field"], at, "sort.field")?,
                    sort["desc"].as_bool().unwrap_or(false),
                ),
            };
            out.insert("sort".into(), json!({"field": field, "desc": desc}));
        }
        if let Some(count_by) = object.get("count_by").filter(|v| !v.is_null()) {
            out.insert(
                "count_by".into(),
                json!(path_name(count_by, at, "count_by")?),
            );
        }
        if let Some(limit) = object.get("limit").filter(|v| !v.is_null()) {
            let limit = limit
                .as_u64()
                .filter(|n| (1..=limits.rows as u64).contains(n))
                .ok_or_else(|| {
                    SurfaceError::new("surface_bind", at, format!("limit is 1 to {}", limits.rows))
                })?;
            out.insert("limit".into(), json!(limit));
        }
        Ok(Value::Object(out))
    }
}

// ---------------------------------------------------------------- the surface

#[derive(Clone, Debug, Default, PartialEq)]
struct Doc {
    title: String,
    places: Vec<(String, Vec<Value>)>,
}

impl Doc {
    fn empty() -> Self {
        Self {
            title: String::new(),
            places: contract()
                .places
                .iter()
                .map(|(name, _)| (name.clone(), Vec::new()))
                .collect(),
        }
    }

    fn place(&self, name: &str) -> Option<usize> {
        self.places.iter().position(|(place, _)| place == name)
    }

    /// Check every block and return the surface as the contract writes it.
    fn normalized(&self) -> Result<Doc> {
        let c = contract();
        let mut checker = Checker {
            c,
            ids: HashSet::new(),
            count: 0,
        };
        let mut out = Doc {
            title: self.title.clone(),
            places: Vec::new(),
        };
        for (name, blocks) in &self.places {
            let mut normal = Vec::with_capacity(blocks.len());
            for block in blocks {
                normal.push(checker.block(block, name, 1, name)?);
            }
            out.places.push((name.clone(), normal));
        }
        let bytes = out.places_json().to_string().len();
        if bytes > c.limits.surface_bytes {
            return Err(SurfaceError::new(
                "surface_bytes",
                "",
                format!(
                    "the surface would hold {bytes} bytes; it holds at most {}",
                    c.limits.surface_bytes
                ),
            ));
        }
        Ok(out)
    }

    fn places_json(&self) -> Value {
        Value::Object(
            self.places
                .iter()
                .map(|(name, blocks)| (name.clone(), Value::Array(blocks.clone())))
                .collect(),
        )
    }

    fn from_json(value: &Value) -> Result<Doc> {
        let mut doc = Doc::empty();
        if let Some(title) = value.get("title").filter(|v| !v.is_null()) {
            doc.title = plain(title, "title", "title", MAX_TITLE, false)?;
        }
        let places = value
            .get("places")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                SurfaceError::new(
                    "surface_places",
                    "places",
                    "a surface is {title, places: {dock: [...], …}}",
                )
            })?;
        for (name, blocks) in places {
            let index = doc.place(name).ok_or_else(|| {
                SurfaceError::new(
                    "surface_place",
                    name.as_str(),
                    format!("places are {}", listing(&place_names())),
                )
            })?;
            doc.places[index].1 = blocks
                .as_array()
                .ok_or_else(|| {
                    SurfaceError::new(
                        "surface_places",
                        name.as_str(),
                        "a place holds a list of blocks",
                    )
                })?
                .clone();
        }
        doc.normalized()
    }

    /// Where a block stands: its place and the indexes down to it through groups.
    fn locate(&self, id: &str) -> Option<(usize, Vec<usize>)> {
        fn walk(list: &[Value], id: &str, trail: &mut Vec<usize>) -> bool {
            for (index, block) in list.iter().enumerate() {
                trail.push(index);
                if block["id"].as_str() == Some(id) {
                    return true;
                }
                if let Some(children) = block["children"].as_array() {
                    if walk(children, id, trail) {
                        return true;
                    }
                }
                trail.pop();
            }
            false
        }
        for (place, (_, blocks)) in self.places.iter().enumerate() {
            let mut trail = Vec::new();
            if walk(blocks, id, &mut trail) {
                return Some((place, trail));
            }
        }
        None
    }

    /// The list a trail's last index stands in.
    fn list_mut(&mut self, place: usize, trail: &[usize]) -> &mut Vec<Value> {
        let mut list = &mut self.places[place].1;
        for index in &trail[..trail.len().saturating_sub(1)] {
            list = list[*index]
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .expect("a trail passes through groups");
        }
        list
    }

    fn take(&mut self, place: usize, trail: &[usize]) -> Value {
        let last = *trail.last().expect("a block's trail is not empty");
        self.list_mut(place, trail).remove(last)
    }

    fn block_mut(&mut self, place: usize, trail: &[usize]) -> &mut Value {
        let last = *trail.last().expect("a block's trail is not empty");
        &mut self.list_mut(place, trail)[last]
    }

    fn ids_in(block: &Value, out: &mut Vec<String>) {
        if let Some(id) = block["id"].as_str() {
            out.push(id.to_owned());
        }
        for child in block["children"].as_array().into_iter().flatten() {
            Self::ids_in(child, out);
        }
    }
}

fn place_names() -> Vec<String> {
    contract()
        .places
        .iter()
        .map(|(name, _)| name.clone())
        .collect()
}

/// Where an op puts a block: a place or a group, and a position in it.
struct Placement {
    place: Option<String>,
    parent: Option<String>,
    after: Option<String>,
    before: Option<String>,
    at: Option<usize>,
}

impl Placement {
    fn of(op: &Value) -> Self {
        let text = |key: &str| op.get(key).and_then(Value::as_str).map(str::to_owned);
        Self {
            place: text("in"),
            parent: text("parent"),
            after: text("after"),
            before: text("before"),
            at: op.get("at").and_then(Value::as_u64).map(|n| n as usize),
        }
    }
    fn given(&self) -> bool {
        self.place.is_some()
            || self.parent.is_some()
            || self.after.is_some()
            || self.before.is_some()
            || self.at.is_some()
    }
}

fn insert(
    doc: &mut Doc,
    block: Value,
    how: &Placement,
    home: Option<(usize, Vec<usize>)>,
) -> Result<()> {
    // A place or a group decides the list; else a sibling named by after or before;
    // else the list the block stood in (at its old position unless `at` is given); else the dock.
    let sibling = how.after.as_deref().or(how.before.as_deref());
    let (place, trail, old) = if let Some(parent) = &how.parent {
        let (place, mut trail) = doc.locate(parent).ok_or_else(|| {
            SurfaceError::new(
                "surface_parent",
                parent.as_str(),
                format!("no group {parent} on the surface"),
            )
        })?;
        let group = doc.block_mut(place, &trail);
        if group["kind"] != "group" {
            return Err(SurfaceError::new(
                "surface_parent",
                parent.as_str(),
                format!("{parent} is not a group"),
            ));
        }
        if group.get("children").and_then(Value::as_array).is_none() {
            group["children"] = json!([]);
        }
        trail.push(usize::MAX);
        (place, trail, None)
    } else if let Some(name) = &how.place {
        let place = doc.place(name).ok_or_else(|| {
            SurfaceError::new(
                "surface_place",
                name.as_str(),
                format!("places are {}", listing(&place_names())),
            )
        })?;
        (place, vec![usize::MAX], None)
    } else if let Some(sibling) = sibling {
        let (place, trail) = doc.locate(sibling).ok_or_else(|| {
            SurfaceError::new(
                "surface_sibling",
                sibling,
                format!("no block {sibling} on the surface"),
            )
        })?;
        (place, trail, None)
    } else if let Some((place, trail)) = home {
        let old = trail.last().copied();
        (place, trail, old)
    } else {
        (doc.place("dock").unwrap_or(0), vec![usize::MAX], None)
    };
    let list = doc.list_mut(place, &trail);
    let position = |id: &str| list.iter().position(|item| item["id"].as_str() == Some(id));
    let at = if let Some(after) = &how.after {
        position(after).map(|i| i + 1).ok_or_else(|| {
            SurfaceError::new(
                "surface_sibling",
                after.as_str(),
                format!("{after} does not stand in that list"),
            )
        })?
    } else if let Some(before) = &how.before {
        position(before).ok_or_else(|| {
            SurfaceError::new(
                "surface_sibling",
                before.as_str(),
                format!("{before} does not stand in that list"),
            )
        })?
    } else {
        how.at.or(old).unwrap_or(list.len())
    };
    let at = at.min(list.len());
    list.insert(at, block);
    Ok(())
}

/// Set one field of a block by a dotted name; null removes it.
fn set_field(block: &mut Value, field: &str, value: Value) -> Result<()> {
    let parts: Vec<&str> = field.split('.').collect();
    if parts.iter().any(|part| part.is_empty()) || matches!(parts[0], "id") {
        return Err(SurfaceError::new(
            "surface_set",
            field,
            "set names a field such as title, bind.limit or columns.0.title; an id changes with put or move",
        ));
    }
    let mut node = block;
    for (depth, part) in parts.iter().enumerate() {
        let last = depth + 1 == parts.len();
        let index = part.parse::<usize>().ok();
        match (node, index) {
            (Value::Array(items), Some(index)) => {
                if index >= items.len() {
                    return Err(SurfaceError::new(
                        "surface_set",
                        field,
                        format!("{field}: no item {index}"),
                    ));
                }
                if last {
                    if value.is_null() {
                        items.remove(index);
                    } else {
                        items[index] = value;
                    }
                    return Ok(());
                }
                node = &mut items[index];
            }
            (Value::Object(map), _) => {
                if last {
                    if value.is_null() {
                        map.remove(*part);
                    } else {
                        map.insert((*part).to_owned(), value);
                    }
                    return Ok(());
                }
                node = map
                    .entry((*part).to_owned())
                    .or_insert_with(|| Value::Object(Map::new()));
            }
            _ => {
                return Err(SurfaceError::new(
                    "surface_set",
                    field,
                    format!("{field}: {part} is not inside an object or a list"),
                ));
            }
        }
    }
    Ok(())
}

fn op_text<'a>(op: &'a Value, key: &str) -> Result<&'a str> {
    op.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| SurfaceError::new("surface_op", key, format!("this op needs {key}")))
}

/// Apply one operation to a draft surface; `changed` and `removed` gather the ids it touched.
fn apply(
    doc: &mut Doc,
    op: &Value,
    changed: &mut Vec<String>,
    removed: &mut Vec<String>,
) -> Result<()> {
    let name = op.get("op").and_then(Value::as_str).unwrap_or("");
    match name {
        "put" => {
            let block = op.get("block").filter(|b| b.is_object()).ok_or_else(|| {
                SurfaceError::new("surface_op", "block", "put needs block {id, kind, …}")
            })?;
            let id = block["id"].as_str().unwrap_or("").to_owned();
            let mut how = Placement::of(op);
            let old = doc.locate(&id);
            // Put again in the place it already stands in, it keeps its position there.
            if let Some((place, trail)) = &old {
                let only_place = how.parent.is_none()
                    && how.after.is_none()
                    && how.before.is_none()
                    && how.at.is_none();
                if only_place
                    && trail.len() == 1
                    && how.place.as_deref() == Some(doc.places[*place].0.as_str())
                {
                    how.place = None;
                }
            }
            if let Some((place, trail)) = &old {
                doc.take(*place, trail);
            }
            insert(doc, block.clone(), &how, old)?;
            changed.push(id);
        }
        "set" => {
            let id = op_text(op, "id")?;
            let field = op_text(op, "field")?;
            let (place, trail) = doc.locate(id).ok_or_else(|| {
                SurfaceError::new(
                    "surface_unknown",
                    id,
                    format!("no block {id} on the surface"),
                )
            })?;
            set_field(
                doc.block_mut(place, &trail),
                field,
                op.get("value").cloned().unwrap_or(Value::Null),
            )?;
            changed.push(id.to_owned());
        }
        "move" => {
            let id = op_text(op, "id")?;
            let how = Placement::of(op);
            if !how.given() {
                return Err(SurfaceError::new(
                    "surface_op",
                    id,
                    "move needs in, parent, after, before or at",
                ));
            }
            let (place, trail) = doc.locate(id).ok_or_else(|| {
                SurfaceError::new(
                    "surface_unknown",
                    id,
                    format!("no block {id} on the surface"),
                )
            })?;
            let block = doc.take(place, &trail);
            let home = Some((place, trail));
            if how.parent.as_deref().is_some_and(|parent| {
                let mut inside = Vec::new();
                Doc::ids_in(&block, &mut inside);
                inside.iter().any(|item| item == parent)
            }) {
                return Err(SurfaceError::new(
                    "surface_op",
                    id,
                    "a group cannot move into itself",
                ));
            }
            insert(doc, block, &how, home)?;
            changed.push(id.to_owned());
        }
        "remove" => {
            let id = op_text(op, "id")?;
            let (place, trail) = doc.locate(id).ok_or_else(|| {
                SurfaceError::new(
                    "surface_unknown",
                    id,
                    format!("no block {id} on the surface"),
                )
            })?;
            let block = doc.take(place, &trail);
            Doc::ids_in(&block, removed);
        }
        "clear" => {
            let only = op.get("in").and_then(Value::as_str);
            if let Some(name) = only {
                if doc.place(name).is_none() {
                    return Err(SurfaceError::new(
                        "surface_place",
                        name,
                        format!("places are {}", listing(&place_names())),
                    ));
                }
            }
            for (place, blocks) in &mut doc.places {
                if only.is_none_or(|name| name == place) {
                    for block in blocks.drain(..) {
                        Doc::ids_in(&block, removed);
                    }
                }
            }
        }
        "title" => {
            doc.title = plain(
                op.get("text").unwrap_or(&Value::Null),
                "title",
                "title",
                MAX_TITLE,
                false,
            )?;
        }
        _ => {
            return Err(SurfaceError::new(
                "surface_op",
                name,
                format!("op is one of {}", OPS.join(", ")),
            ));
        }
    }
    Ok(())
}

fn author(value: &Value) -> Result<Value> {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty() && id.len() <= 128)
        .ok_or_else(|| {
            SurfaceError::new(
                "surface_author",
                "author",
                "a patch names its author {id, model?}",
            )
        })?;
    let mut out = json!({"id": id});
    for key in ["model", "type"] {
        if let Some(text) = value
            .get(key)
            .and_then(Value::as_str)
            .filter(|t| t.len() <= 128)
        {
            out[key] = json!(text);
        }
    }
    Ok(out)
}

/// One surface with its revision, whether the person froze it, who shaped it
/// and the states it can return to.
#[derive(Clone, Debug)]
pub struct Surface {
    doc: Doc,
    revision: u64,
    frozen: bool,
    authors: Vec<Value>,
    history: VecDeque<(Doc, Vec<Value>)>,
}

impl Default for Surface {
    fn default() -> Self {
        Self::new()
    }
}

impl Surface {
    pub fn new() -> Self {
        Self {
            doc: Doc::empty(),
            revision: 0,
            frozen: false,
            authors: Vec::new(),
            history: VecDeque::new(),
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    pub fn is_empty(&self) -> bool {
        self.doc.places.iter().all(|(_, blocks)| blocks.is_empty())
    }

    /// The surface as the page draws it and an agent reads it.
    pub fn state(&self) -> Value {
        let mut blocks = Vec::new();
        for (_, list) in &self.doc.places {
            for block in list {
                Doc::ids_in(block, &mut blocks);
            }
        }
        json!({
            "schema": SURFACE_SCHEMA,
            "revision": self.revision,
            "title": self.doc.title,
            "frozen": self.frozen,
            "authors": self.authors,
            "blocks": blocks.len(),
            "can_undo": self.history.len(),
            "places": self.doc.places_json(),
        })
    }

    /// What a kept surface holds: its title and places.
    pub fn content(&self) -> Value {
        json!({"title": self.doc.title, "places": self.doc.places_json()})
    }

    fn commit(&mut self, doc: Doc, by: Option<Value>) {
        let limit = contract().limits.history;
        self.history
            .push_back((std::mem::replace(&mut self.doc, doc), self.authors.clone()));
        while self.history.len() > limit {
            self.history.pop_front();
        }
        if let Some(by) = by {
            self.authors.retain(|known| known["id"] != by["id"]);
            self.authors.insert(0, by);
            self.authors.truncate(MAX_AUTHORS);
        }
        self.revision += 1;
    }

    /// Apply a patch {ops, author, expect_revision?} all or none.
    pub fn patch(&mut self, request: &Value) -> Result<Value> {
        let limits = contract().limits;
        if self.frozen {
            return Err(SurfaceError::new(
                "surface_frozen",
                "",
                "the person froze this surface; it takes no patch until they let it go",
            ));
        }
        let by = author(request.get("author").unwrap_or(&Value::Null))?;
        if let Some(expected) = request.get("expect_revision").and_then(Value::as_u64) {
            if expected != self.revision {
                let mut error = SurfaceError::new(
                    "surface_revision",
                    "",
                    format!(
                        "the surface is at revision {}; read it and patch again",
                        self.revision
                    ),
                );
                error.at = self.revision.to_string();
                return Err(error);
            }
        }
        let ops = request
            .get("ops")
            .and_then(Value::as_array)
            .filter(|ops| !ops.is_empty() && ops.len() <= limits.ops_per_patch)
            .ok_or_else(|| {
                SurfaceError::new(
                    "surface_ops",
                    "ops",
                    format!("a patch carries 1 to {} ops", limits.ops_per_patch),
                )
            })?;
        let mut doc = self.doc.clone();
        let (mut changed, mut removed) = (Vec::new(), Vec::new());
        for (index, op) in ops.iter().enumerate() {
            apply(&mut doc, op, &mut changed, &mut removed).map_err(|e| e.at_op(index))?;
            doc = doc.normalized().map_err(|e| e.at_op(index))?;
        }
        changed.sort();
        changed.dedup();
        removed.sort();
        removed.dedup();
        removed.retain(|id| doc.locate(id).is_none());
        changed.retain(|id| doc.locate(id).is_some());
        if doc != self.doc {
            self.commit(doc, Some(by));
        }
        Ok(json!({
            "revision": self.revision,
            "changed": changed,
            "removed": removed,
            "state": self.state(),
        }))
    }

    /// Put a whole surface in place (a kept one opened again); undo returns to the one before.
    pub fn open(&mut self, content: &Value, by: &Value) -> Result<Value> {
        if self.frozen {
            return Err(SurfaceError::new(
                "surface_frozen",
                "",
                "the person froze this surface",
            ));
        }
        let by = author(by)?;
        let doc = Doc::from_json(content)?;
        if doc != self.doc {
            self.commit(doc, Some(by));
        }
        Ok(self.state())
    }

    /// Take a state another host holds (the bridge's) as this surface's, history and all reset.
    pub fn mirror(&mut self, state: &Value) -> Result<Value> {
        let doc = Doc::from_json(state)?;
        self.doc = doc;
        self.revision = state["revision"].as_u64().unwrap_or(self.revision);
        self.frozen = state["frozen"].as_bool().unwrap_or(false);
        self.authors = state["authors"].as_array().cloned().unwrap_or_default();
        self.authors.truncate(MAX_AUTHORS);
        self.history.clear();
        Ok(self.state())
    }

    /// The person's controls: freeze or let go, undo, clear.
    pub fn freeze(&mut self, frozen: bool) -> Value {
        if self.frozen != frozen {
            self.frozen = frozen;
            self.revision += 1;
        }
        self.state()
    }

    pub fn undo(&mut self) -> Value {
        if let Some((doc, authors)) = self.history.pop_back() {
            self.doc = doc;
            self.authors = authors;
            self.revision += 1;
        }
        self.state()
    }

    pub fn clear(&mut self) -> Value {
        if !self.is_empty() || !self.doc.title.is_empty() {
            self.commit(Doc::empty(), None);
            self.authors.clear();
        }
        self.state()
    }

    /// The blocks that read data, with their place and bind, so the page knows what to fetch.
    pub fn binds(&self) -> Value {
        fn walk(list: &[Value], place: &str, out: &mut Vec<Value>) {
            for block in list {
                if let Some(bind) = block.get("bind") {
                    let selected = bind
                        .as_object()
                        .is_some_and(|b| b.values().any(|v| v.as_str() == Some("$selected")));
                    out.push(json!({"id": block["id"], "place": place, "bind": bind, "selected": selected || place == "card"}));
                }
                if let Some(children) = block["children"].as_array() {
                    walk(children, place, out);
                }
            }
        }
        let mut out = Vec::new();
        for (place, blocks) in &self.doc.places {
            walk(blocks, place, &mut out);
        }
        Value::Array(out)
    }

    /// A block by id.
    pub fn block(&self, id: &str) -> Option<&Value> {
        let (place, trail) = self.doc.locate(id)?;
        let mut list = &self.doc.places[place].1;
        for index in &trail[..trail.len() - 1] {
            list = list[*index]["children"].as_array()?;
        }
        list.get(*trail.last()?)
    }
}

// ---------------------------------------------------------------- data

/// A value at a dotted path; numeric parts index lists.
fn at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut node = value;
    for part in path.split('.') {
        node = match node {
            Value::Object(map) => map.get(part)?,
            Value::Array(items) => items.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(node)
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    }
}

/// How a value reads in a template.
pub fn display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_f64() {
            Some(x) if x.fract() == 0.0 && x.abs() < 1e15 => format!("{}", x as i64),
            Some(x) => {
                let text = format!("{x:.2}");
                text.trim_end_matches('0').trim_end_matches('.').to_owned()
            }
            None => n.to_string(),
        },
        Value::Array(items) if items.iter().all(scalar) => {
            items.iter().map(display).collect::<Vec<_>>().join(", ")
        }
        Value::Array(items) => format!("[{}]", items.len()),
        Value::Object(map) => ["label", "title", "name", "id"]
            .iter()
            .find_map(|key| map.get(*key).and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| "{…}".to_owned()),
    }
}

/// Fill a template from a row; `n` is the row's position from 0.
pub fn fill(template: &str, row: Option<&Value>, n: usize) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let name: String = chars.by_ref().take_while(|c| *c != '}').collect();
                if name == "#" {
                    out.push_str(&(n + 1).to_string());
                } else if let Some(value) = row.and_then(|row| at_path(row, &name)) {
                    out.push_str(&display(value));
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn compare(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a.filter(|v| !v.is_null()), b.filter(|v| !v.is_null())) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => match (number(a), number(b)) {
            (Some(x), Some(y)) if !a.is_string() || !b.is_string() => {
                x.partial_cmp(&y).unwrap_or(Ordering::Equal)
            }
            _ => display(a).cmp(&display(b)),
        },
    }
}

fn matches(row: &Value, term: &Value) -> bool {
    let field = term["field"].as_str().unwrap_or("");
    let value = at_path(row, field);
    let wanted = &term["value"];
    let order = || compare(value, Some(wanted));
    match term["op"].as_str().unwrap_or("eq") {
        "eq" if wanted.is_null() => value.is_none_or(Value::is_null),
        "eq" => value.is_some_and(|v| v == wanted || display(v) == display(wanted)),
        "ne" => !matches(row, &json!({"field": field, "op": "eq", "value": wanted})),
        "gt" => value.is_some() && order().is_gt(),
        "gte" => value.is_some() && order().is_ge(),
        "lt" => value.is_some() && order().is_lt(),
        "lte" => value.is_some() && order().is_le(),
        "has" => match value {
            Some(Value::String(text)) => text
                .to_lowercase()
                .contains(&display(wanted).to_lowercase()),
            Some(Value::Array(items)) => items
                .iter()
                .any(|item| item == wanted || display(item) == display(wanted)),
            Some(Value::Object(map)) => wanted.as_str().is_some_and(|key| map.contains_key(key)),
            _ => false,
        },
        "in" => wanted.as_array().is_some_and(|items| {
            value.is_some_and(|v| {
                items
                    .iter()
                    .any(|item| item == v || display(item) == display(v))
            })
        }),
        _ => false,
    }
}

/// Rows from a source's answer as a bind shapes them: the list at its path,
/// filtered, counted, sorted and cut to its limit.
#[derive(Clone, Debug, Default)]
pub struct Shaped {
    pub rows: Vec<Value>,
    pub total: usize,
    pub fields: Vec<String>,
    pub notes: Vec<String>,
}

pub fn shape(bind: &Value, data: &Value) -> Shaped {
    let mut notes = Vec::new();
    let base = match bind.get("path").and_then(Value::as_str) {
        Some(path) => match at_path(data, path) {
            Some(found) => found.clone(),
            None => {
                notes.push(format!("the answer has nothing at {path}"));
                Value::Null
            }
        },
        None if bind["source"] == "rows" => bind["rows"].clone(),
        None => data.clone(),
    };
    let mut rows: Vec<Value> = match base {
        Value::Array(items) => items
            .into_iter()
            .map(|item| {
                if item.is_object() {
                    item
                } else {
                    json!({"value": item})
                }
            })
            .collect(),
        Value::Null => Vec::new(),
        Value::Object(_) => vec![base],
        other => vec![json!({"value": other})],
    };
    if bind.get("path").is_none() && rows.len() == 1 {
        let lists: Vec<&String> = rows[0]
            .as_object()
            .map(|row| {
                row.iter()
                    .filter(|(_, v)| v.is_array())
                    .map(|(k, _)| k)
                    .collect()
            })
            .unwrap_or_default();
        if !lists.is_empty() {
            notes.push(format!(
                "the answer is one row holding lists at {}; path picks one as the rows",
                listing(&lists)
            ));
        }
    }
    if let Some(terms) = bind["where"].as_array() {
        rows.retain(|row| terms.iter().all(|term| matches(row, term)));
    }
    if let Some(field) = bind["count_by"].as_str() {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for row in &rows {
            let key = at_path(row, field).map(display).unwrap_or_default();
            match counts.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += 1,
                None => counts.push((key, 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        rows = counts
            .into_iter()
            .map(|(value, count)| json!({"value": value, "count": count}))
            .collect();
    }
    if let Some(field) = bind["sort"]["field"].as_str() {
        let desc = bind["sort"]["desc"].as_bool().unwrap_or(false);
        rows.sort_by(|a, b| {
            let order = compare(at_path(a, field), at_path(b, field));
            if desc && at_path(a, field).is_some() && at_path(b, field).is_some() {
                order.reverse()
            } else {
                order
            }
        });
    }
    let total = rows.len();
    let limit = bind["limit"].as_u64().map_or(DEFAULT_ROWS, |n| n as usize);
    rows.truncate(limit.min(contract().limits.rows));
    let mut fields = BTreeSet::new();
    for row in rows.iter().take(NOTE_ROWS) {
        if let Some(row) = row.as_object() {
            fields.extend(row.keys().cloned());
        }
    }
    Shaped {
        rows,
        total,
        fields: fields.into_iter().collect(),
        notes,
    }
}

/// The fields a block reads from its rows.
fn reads(block: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["text", "title", "meta", "target", "label"] {
        if let Some(text) = block[key].as_str() {
            out.extend(
                placeholders(text)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|n| n != "#"),
            );
        }
    }
    if block["kind"] == "bars" {
        out.extend(block["value"].as_str().map(str::to_owned));
    }
    out.extend(block["field"].as_str().map(str::to_owned));
    for column in block["columns"].as_array().into_iter().flatten() {
        out.extend(column["field"].as_str().map(str::to_owned));
    }
    out.sort();
    out.dedup();
    out
}

/// Exactly what the page draws for a block from its source's answer (`data`
/// is ignored for blocks without a bind), with the rows it read and what the
/// agent should know: fields no row had, lists a path could pick.
pub fn view(block: &Value, data: &Value) -> Value {
    let kind = block["kind"].as_str().unwrap_or("");
    let bind = block.get("bind");
    let shaped = bind.map(|bind| shape(bind, data)).unwrap_or_default();
    let rows = &shaped.rows;
    let first = rows.first();
    let text = |key: &str| block[key].as_str().unwrap_or("");
    let filled = |key: &str, row: &Value, n: usize| {
        block[key]
            .as_str()
            .map(|t| fill(t, Some(row), n))
            .filter(|t| !t.is_empty())
    };
    let addresses = |key: &str, template: &str| -> Vec<Value> {
        let mut seen = BTreeSet::new();
        let from_list = block[key].as_array().cloned().unwrap_or_default();
        let from_rows = rows.iter().enumerate().filter_map(|(n, row)| {
            block[template]
                .as_str()
                .map(|t| json!(fill(t, Some(row), n)))
        });
        from_list
            .into_iter()
            .chain(from_rows)
            .filter(|v| {
                v.as_str()
                    .is_some_and(|s| !s.is_empty() && seen.insert(s.to_owned()))
            })
            .take(contract().limits.members)
            .collect()
    };
    let mut out = json!({"id": block["id"], "kind": kind});
    match kind {
        "heading" => out["text"] = json!(text("text")),
        "text" => {
            out["text"] = json!(fill(text("text"), first, 0));
            out["tone"] = json!(block["tone"].as_str().unwrap_or("plain"));
        }
        "metric" => {
            out["label"] = json!(fill(text("label"), first, 0));
            out["value"] = json!(match (block["field"].as_str(), bind) {
                (Some(field), Some(_)) => first
                    .and_then(|row| at_path(row, field))
                    .map(display)
                    .unwrap_or_default(),
                (None, Some(_)) => shaped.total.to_string(),
                _ => display(&block["value"]),
            });
        }
        "list" => {
            out["items"] = Value::Array(
                rows.iter()
                    .enumerate()
                    .map(|(n, row)| {
                        let mut item = json!({"title": fill(text("title"), Some(row), n)});
                        if let Some(meta) = filled("meta", row, n) {
                            item["meta"] = json!(meta);
                        }
                        if let Some(target) = filled("target", row, n) {
                            item["target"] = json!(target);
                        }
                        item
                    })
                    .collect(),
            );
        }
        "table" => {
            let columns = block["columns"].as_array().cloned().unwrap_or_default();
            out["head"] = Value::Array(columns.iter().map(|c| c["title"].clone()).collect());
            out["rows"] = Value::Array(
                rows.iter()
                    .enumerate()
                    .map(|(n, row)| {
                        let cells: Vec<Value> = columns
                            .iter()
                            .map(|c| {
                                json!(
                                    at_path(row, c["field"].as_str().unwrap_or(""))
                                        .map(display)
                                        .unwrap_or_default()
                                )
                            })
                            .collect();
                        let mut line = json!({"cells": cells});
                        if let Some(target) = filled("target", row, n) {
                            line["target"] = json!(target);
                        }
                        line
                    })
                    .collect(),
            );
        }
        "bars" => {
            let field = text("value");
            let values: Vec<f64> = rows
                .iter()
                .map(|row| at_path(row, field).and_then(number).unwrap_or(0.0))
                .collect();
            let max = values.iter().cloned().fold(0.0_f64, f64::max);
            out["items"] = Value::Array(
                rows.iter()
                    .zip(&values)
                    .enumerate()
                    .map(|(n, (row, value))| {
                        let mut item = json!({
                            "label": fill(text("label"), Some(row), n),
                            "value": display(&json!(value)),
                            "share": if max > 0.0 { (value / max * 1000.0).round() / 1000.0 } else { 0.0 },
                        });
                        if let Some(target) = filled("target", row, n) {
                            item["target"] = json!(target);
                        }
                        item
                    })
                    .collect(),
            );
        }
        "chips" => {
            let mut items: Vec<Value> = block["members"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|m| json!({"target": m, "label": m}))
                .collect();
            for (n, row) in rows.iter().enumerate() {
                if let Some(target) = filled("target", row, n) {
                    let label = filled("label", row, n).unwrap_or_else(|| target.clone());
                    items.push(json!({"target": target, "label": label}));
                }
            }
            out["items"] = Value::Array(items);
        }
        "button" => {
            out["label"] = json!(text("label"));
            out["command"] = block["command"].clone();
            out["input"] = block.get("input").cloned().unwrap_or_else(|| json!({}));
        }
        "group" => {
            out["title"] = json!(text("title"));
            out["open"] = json!(block["open"].as_bool().unwrap_or(true));
        }
        "highlight" => {
            out["members"] = Value::Array(addresses("members", "target"));
            out["dim_rest"] = json!(block["dim_rest"].as_bool().unwrap_or(false));
            out["title"] = json!(text("title"));
        }
        "links" => {
            out["relations"] = Value::Array(addresses("relations", "target"));
            out["title"] = json!(text("title"));
        }
        _ => {}
    }
    if bind.is_some() {
        if rows.is_empty() {
            if let Some(empty) = block["empty"].as_str() {
                out["empty"] = json!(empty);
            }
        }
        // A block of many rows hears that its answer is one row holding lists; a block that reads one
        // row (a metric, a text) hears it only when a field it reads is not there.
        let many = matches!(
            kind,
            "list" | "table" | "bars" | "chips" | "highlight" | "links"
        );
        let (lists, mut notes): (Vec<String>, Vec<String>) = shaped
            .notes
            .iter()
            .cloned()
            .partition(|note| note.starts_with("the answer is one row holding lists"));
        let mut missing = false;
        if !rows.is_empty() {
            for name in reads(block) {
                let found = rows
                    .iter()
                    .take(NOTE_ROWS)
                    .any(|row| at_path(row, &name).is_some_and(|v| !v.is_null()));
                if !found {
                    missing = true;
                    notes.push(format!(
                        "no row has {name}; rows have {}",
                        listing(&shaped.fields)
                    ));
                }
            }
        }
        if many || missing {
            notes.extend(lists);
        }
        out["count"] = json!(rows.len());
        out["total"] = json!(shaped.total);
        out["fields"] = json!(shaped.fields);
        if !notes.is_empty() {
            out["notes"] = json!(notes);
        }
    }
    out
}

// ---------------------------------------------------------------- byte API

fn parse(bytes: &[u8], limit: usize) -> std::result::Result<Value, String> {
    if bytes.len() > limit {
        return Err(
            SurfaceError::new("surface_bytes", "", format!("at most {limit} bytes")).to_string(),
        );
    }
    serde_json::from_slice(bytes)
        .map_err(|e| SurfaceError::new("surface_json", "", format!("not JSON: {e}")).to_string())
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

const MAX_REQUEST_BYTES: usize = 262_144;
const MAX_DATA_BYTES: usize = 8 * 1_048_576;

impl Surface {
    pub fn patch_bytes(&mut self, request: &[u8]) -> std::result::Result<Vec<u8>, String> {
        let request = parse(request, MAX_REQUEST_BYTES)?;
        self.patch(&request)
            .map(|v| bytes(&v))
            .map_err(|e| e.to_string())
    }
    pub fn open_bytes(
        &mut self,
        content: &[u8],
        by: &[u8],
    ) -> std::result::Result<Vec<u8>, String> {
        let content = parse(content, MAX_REQUEST_BYTES)?;
        let by = parse(by, 4_096)?;
        self.open(&content, &by)
            .map(|v| bytes(&v))
            .map_err(|e| e.to_string())
    }
    pub fn mirror_bytes(&mut self, state: &[u8]) -> std::result::Result<Vec<u8>, String> {
        let state = parse(state, MAX_REQUEST_BYTES)?;
        self.mirror(&state)
            .map(|v| bytes(&v))
            .map_err(|e| e.to_string())
    }
    /// What the page draws for one block from its source's answer.
    pub fn view_bytes(&self, id: &str, data: &[u8]) -> std::result::Result<Vec<u8>, String> {
        let block = self.block(id).ok_or_else(|| {
            SurfaceError::new(
                "surface_unknown",
                id,
                format!("no block {id} on the surface"),
            )
            .to_string()
        })?;
        let data = if data.is_empty() {
            Value::Null
        } else {
            parse(data, MAX_DATA_BYTES)?
        };
        Ok(bytes(&view(block, &data)))
    }
    /// Try a bind on a source's answer without placing it.
    pub fn try_bind_bytes(bind: &[u8], data: &[u8]) -> std::result::Result<Vec<u8>, String> {
        let bind = parse(bind, MAX_REQUEST_BYTES)?;
        let checker = Checker {
            c: contract(),
            ids: HashSet::new(),
            count: 0,
        };
        let bind = checker.bind(&bind, "bind").map_err(|e| e.to_string())?;
        let data = if data.is_empty() {
            Value::Null
        } else {
            parse(data, MAX_DATA_BYTES)?
        };
        let shaped = shape(&bind, &data);
        Ok(bytes(&json!({
            "bind": bind,
            "rows": shaped.rows.iter().take(5).collect::<Vec<_>>(),
            "count": shaped.rows.len(),
            "total": shaped.total,
            "fields": shaped.fields,
            "notes": shaped.notes,
        })))
    }
}

#[cfg(feature = "wasm")]
pub mod browser {
    use super::{Surface, surface_contract};
    use wasm_bindgen::prelude::*;

    /// The surface an agent shapes, in the browser (see `Surface`).
    #[wasm_bindgen]
    pub struct SurfaceV1 {
        surface: Surface,
    }

    impl Default for SurfaceV1 {
        fn default() -> Self {
            Self::new()
        }
    }

    #[wasm_bindgen]
    impl SurfaceV1 {
        #[wasm_bindgen(constructor)]
        pub fn new() -> SurfaceV1 {
            Self {
                surface: Surface::new(),
            }
        }
        pub fn contract() -> Vec<u8> {
            serde_json::to_vec(surface_contract()).unwrap_or_default()
        }
        pub fn state(&self) -> Vec<u8> {
            serde_json::to_vec(&self.surface.state()).unwrap_or_default()
        }
        pub fn content(&self) -> Vec<u8> {
            serde_json::to_vec(&self.surface.content()).unwrap_or_default()
        }
        pub fn binds(&self) -> Vec<u8> {
            serde_json::to_vec(&self.surface.binds()).unwrap_or_default()
        }
        pub fn patch(&mut self, request: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.surface
                .patch_bytes(request)
                .map_err(|error| JsValue::from_str(&error))
        }
        pub fn open(&mut self, content: &[u8], author: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.surface
                .open_bytes(content, author)
                .map_err(|error| JsValue::from_str(&error))
        }
        pub fn mirror(&mut self, state: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.surface
                .mirror_bytes(state)
                .map_err(|error| JsValue::from_str(&error))
        }
        pub fn view(&self, id: &str, data: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.surface
                .view_bytes(id, data)
                .map_err(|error| JsValue::from_str(&error))
        }
        #[wasm_bindgen(js_name = tryBind)]
        pub fn try_bind(bind: &[u8], data: &[u8]) -> Result<Vec<u8>, JsValue> {
            Surface::try_bind_bytes(bind, data).map_err(|error| JsValue::from_str(&error))
        }
        pub fn freeze(&mut self, frozen: bool) -> Vec<u8> {
            serde_json::to_vec(&self.surface.freeze(frozen)).unwrap_or_default()
        }
        pub fn undo(&mut self) -> Vec<u8> {
            serde_json::to_vec(&self.surface.undo()).unwrap_or_default()
        }
        pub fn clear(&mut self) -> Vec<u8> {
            serde_json::to_vec(&self.surface.clear()).unwrap_or_default()
        }
        pub fn revision(&self) -> f64 {
            self.surface.revision() as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by() -> Value {
        json!({"id": "a1", "model": "m"})
    }
    fn patch(surface: &mut Surface, ops: Value) -> Result<Value> {
        surface.patch(&json!({"author": by(), "ops": ops}))
    }

    #[test]
    fn the_contract_reads_whole() {
        let c = contract();
        assert_eq!(place_names(), ["dock", "strip", "card", "sky"]);
        assert!(c.blocks.iter().any(|b| b.kind == "list"));
        assert!(c.limits.blocks > 0 && c.limits.history > 0 && c.limits.rows > 0);
        // Every kind a place names is a block, and every block stands somewhere.
        for (_, kinds) in &c.places {
            for kind in kinds {
                assert!(c.blocks.iter().any(|b| &b.kind == kind), "{kind}");
            }
        }
        for rule in &c.blocks {
            assert!(
                c.places.iter().any(|(_, kinds)| kinds.contains(&rule.kind)),
                "{}",
                rule.kind
            );
        }
    }

    #[test]
    fn a_patch_puts_sets_moves_and_removes_all_or_none() {
        let mut s = Surface::new();
        let out = patch(&mut s, json!([
            {"op": "title", "text": "Review at a glance"},
            {"op": "put", "block": {"id": "h", "kind": "heading", "text": "Queue"}},
            {"op": "put", "block": {"id": "q", "kind": "list", "title": "{group}", "meta": "{open_count} open",
                "bind": {"source": "journal", "view": "review", "path": "groups", "sort": "-open_count", "limit": 5}}},
            {"op": "put", "in": "strip", "block": {"id": "m", "kind": "metric", "label": "open", "bind": {"source": "journal", "view": "review"}, "field": "open_total"}}
        ])).unwrap();
        assert_eq!(out["revision"], 1);
        assert_eq!(out["changed"], json!(["h", "m", "q"]));
        let state = s.state();
        assert_eq!(state["title"], "Review at a glance");
        assert_eq!(
            state["places"]["dock"][1]["bind"]["sort"],
            json!({"field": "open_count", "desc": true})
        );
        assert_eq!(state["places"]["strip"][0]["id"], "m");

        // One bad op refuses the whole patch and names it.
        let error = patch(
            &mut s,
            json!([
                {"op": "set", "id": "q", "field": "bind.limit", "value": 3},
                {"op": "put", "block": {"id": "c", "kind": "chart"}}
            ]),
        )
        .unwrap_err();
        assert_eq!((error.code, error.op), ("surface_kind", Some(1)));
        assert!(error.message.contains("list"), "{}", error.message);
        assert_eq!(s.revision(), 1);
        assert_eq!(s.state()["places"]["dock"][1]["bind"]["limit"], 5);

        // Set, move before, then remove.
        patch(
            &mut s,
            json!([
                {"op": "set", "id": "q", "field": "bind.limit", "value": 3},
                {"op": "move", "id": "q", "before": "h"}
            ]),
        )
        .unwrap();
        let dock = &s.state()["places"]["dock"];
        assert_eq!(
            (dock[0]["id"].clone(), dock[0]["bind"]["limit"].clone()),
            (json!("q"), json!(3))
        );
        let out = patch(&mut s, json!([{"op": "remove", "id": "q"}])).unwrap();
        assert_eq!(out["removed"], json!(["q"]));
        assert_eq!(s.revision(), 3);
        assert_eq!(s.state()["authors"][0]["id"], "a1");
    }

    #[test]
    fn a_replaced_block_keeps_its_place_and_groups_hold_children() {
        let mut s = Surface::new();
        patch(&mut s, json!([
            {"op": "put", "block": {"id": "a", "kind": "text", "text": "one"}},
            {"op": "put", "block": {"id": "g", "kind": "group", "title": "G", "children": [{"id": "g1", "kind": "text", "text": "in"}]}},
            {"op": "put", "block": {"id": "b", "kind": "text", "text": "two"}}
        ])).unwrap();
        patch(&mut s, json!([{"op": "put", "block": {"id": "a", "kind": "text", "text": "one again", "tone": "note"}}])).unwrap();
        assert_eq!(s.state()["places"]["dock"][0]["text"], "one again");
        patch(&mut s, json!([{"op": "put", "in": "dock", "block": {"id": "a", "kind": "text", "text": "once more"}}])).unwrap();
        assert_eq!(s.state()["places"]["dock"][0]["text"], "once more");
        patch(
            &mut s,
            json!([{"op": "move", "id": "b", "parent": "g", "at": 0}]),
        )
        .unwrap();
        let group = s.block("g").unwrap();
        assert_eq!(group["children"][0]["id"], "b");
        assert!(patch(&mut s, json!([{"op": "move", "id": "g", "parent": "g"}])).is_err());
        let out = patch(&mut s, json!([{"op": "remove", "id": "g"}])).unwrap();
        assert_eq!(out["removed"], json!(["b", "g", "g1"]));
        // Ids are one each across the surface.
        let error = patch(&mut s, json!([{"op": "put", "in": "card", "block": {"id": "x", "kind": "group", "title": "t", "children": [{"id": "a", "kind": "text", "text": "dup"}]}}])).unwrap_err();
        assert_eq!(error.code, "surface_id_taken");
    }

    #[test]
    fn the_contract_bounds_what_a_surface_carries() {
        let mut s = Surface::new();
        let cases = [
            (
                json!({"id": "s", "kind": "highlight", "members": ["x"]}),
                "surface_kind",
            ),
            (
                json!({"id": "t", "kind": "text", "text": "x", "style": "color:red"}),
                "surface_field",
            ),
            (
                json!({"id": "t", "kind": "text", "text": "{unclosed"}),
                "surface_template",
            ),
            (
                json!({"id": "b", "kind": "button", "label": "go", "command": "tos.act.append"}),
                "surface_command",
            ),
            (
                json!({"id": "l", "kind": "list", "title": "x", "bind": {"source": "web", "url": "https://x"}}),
                "surface_source",
            ),
            (
                json!({"id": "l", "kind": "list", "title": "x", "bind": {"source": "journal", "view": "everything"}}),
                "surface_bind",
            ),
            (
                json!({"id": "Bad id", "kind": "text", "text": "x"}),
                "surface_id",
            ),
            (json!({"id": "c", "kind": "chips"}), "surface_required"),
        ];
        for (block, code) in cases {
            let error = patch(&mut s, json!([{"op": "put", "block": block}])).unwrap_err();
            assert_eq!(error.code, code, "{block}");
        }
        // Sky blocks stand only in the sky.
        patch(&mut s, json!([{"op": "put", "in": "sky", "block": {"id": "s", "kind": "highlight", "members": ["a", "a", "b"], "dim_rest": true}}])).unwrap();
        assert_eq!(s.block("s").unwrap()["members"], json!(["a", "b"]));
        // A surface holds a bounded number of blocks and bytes.
        let many: Vec<Value> = (0..=surface_limits().blocks)
            .map(|n| json!({"op": "put", "block": {"id": format!("t{n}"), "kind": "text", "text": "x"}}))
            .collect();
        for chunk in many.chunks(60) {
            let _ = patch(&mut s, Value::Array(chunk.to_vec()));
        }
        assert!(s.state()["blocks"].as_u64().unwrap() <= surface_limits().blocks as u64);
    }

    #[test]
    fn the_person_freezes_undoes_and_clears() {
        let mut s = Surface::new();
        patch(
            &mut s,
            json!([{"op": "put", "block": {"id": "a", "kind": "text", "text": "1"}}]),
        )
        .unwrap();
        patch(
            &mut s,
            json!([{"op": "set", "id": "a", "field": "text", "value": "2"}]),
        )
        .unwrap();
        s.freeze(true);
        assert_eq!(
            patch(&mut s, json!([{"op": "remove", "id": "a"}]))
                .unwrap_err()
                .code,
            "surface_frozen"
        );
        s.freeze(false);
        s.undo();
        assert_eq!(s.block("a").unwrap()["text"], "1");
        s.clear();
        assert!(s.is_empty());
        s.undo();
        assert_eq!(s.block("a").unwrap()["text"], "1");
        // A stale revision is refused when the agent asks for one.
        let error = s
            .patch(&json!({"author": by(), "expect_revision": 1, "ops": [{"op": "remove", "id": "a"}]}))
            .unwrap_err();
        assert_eq!(error.code, "surface_revision");
        // A patch that changes nothing makes no revision.
        let at = s.revision();
        patch(
            &mut s,
            json!([{"op": "put", "block": {"id": "a", "kind": "text", "text": "1"}}]),
        )
        .unwrap();
        assert_eq!(s.revision(), at);
    }

    #[test]
    fn a_kept_surface_opens_again_and_a_mirror_takes_the_bridge_state() {
        let mut s = Surface::new();
        patch(&mut s, json!([{"op": "title", "text": "T"}, {"op": "put", "in": "card", "block": {"id": "n", "kind": "list", "title": "{label}", "target": "{id}", "bind": {"source": "item", "id": "$selected", "path": "backlinks"}}}])).unwrap();
        let kept = s.content();
        let mut other = Surface::new();
        other.open(&kept, &by()).unwrap();
        assert_eq!(other.content(), kept);
        assert_eq!(other.binds()[0]["selected"], true);
        let mut page = Surface::new();
        page.mirror(&s.state()).unwrap();
        assert_eq!((page.revision(), page.content()), (s.revision(), kept));
        assert!(
            Surface::new()
                .open(&json!({"places": {"wall": []}}), &by())
                .is_err()
        );
    }

    #[test]
    fn rows_are_shaped_and_a_view_names_what_it_could_not_read() {
        let data = json!({"open_total": 7, "groups": [
            {"group": "translation:Kind", "open_count": 4, "models": ["m1"], "category": "translation"},
            {"group": "link:3", "open_count": 2, "models": [], "category": "link"},
            {"group": "text:9", "open_count": 1, "models": ["m1", "m2"], "category": "translation"}
        ]});
        let bind = json!({"source": "journal", "view": "review", "path": "groups", "where": [{"field": "category", "op": "eq", "value": "translation"}], "sort": {"field": "open_count", "desc": true}});
        let shaped = shape(&bind, &data);
        assert_eq!(shaped.rows.len(), 2);
        assert_eq!(shaped.rows[0]["group"], "translation:Kind");
        let counted = shape(
            &json!({"source": "journal", "view": "review", "path": "groups", "count_by": "category"}),
            &data,
        );
        assert_eq!(counted.rows[0], json!({"value": "translation", "count": 2}));

        let list = json!({"id": "q", "kind": "list", "title": "{#}. {group}", "meta": "{open_count} · {models} · {missing}", "target": "{group}", "bind": bind});
        let drawn = view(&list, &data);
        assert_eq!(drawn["items"][0]["title"], "1. translation:Kind");
        assert_eq!(drawn["items"][1]["meta"], "1 · m1, m2 · ");
        assert!(
            drawn["notes"][0]
                .as_str()
                .unwrap()
                .starts_with("no row has missing")
        );

        let metric = json!({"id": "m", "kind": "metric", "label": "open", "field": "open_total", "bind": {"source": "journal", "view": "review"}});
        let drawn = view(&metric, &data);
        assert_eq!(drawn["value"], "7");
        assert!(drawn.get("notes").is_none());
        let list = json!({"id": "l", "kind": "list", "title": "{group}", "bind": {"source": "journal", "view": "review"}});
        assert!(
            view(&list, &data)["notes"][1]
                .as_str()
                .unwrap()
                .contains("path picks one")
        );

        let bars = json!({"id": "b", "kind": "bars", "label": "{group}", "value": "open_count", "bind": {"source": "journal", "view": "review", "path": "groups"}});
        assert_eq!(view(&bars, &data)["items"][1]["share"], 0.5);

        let sky = json!({"id": "s", "kind": "highlight", "members": ["a"], "target": "{group}", "bind": {"source": "journal", "view": "review", "path": "groups", "limit": 1}});
        assert_eq!(
            view(&sky, &data)["members"],
            json!(["a", "translation:Kind"])
        );

        let table = json!({"id": "t", "kind": "table", "columns": [{"field": "group", "title": "Group"}, {"field": "open_count", "title": "Open"}], "bind": {"source": "rows", "rows": [{"group": "x", "open_count": 1.5}]}});
        assert_eq!(
            view(&table, &Value::Null)["rows"][0]["cells"],
            json!(["x", "1.5"])
        );
    }

    #[test]
    fn templates_fill_and_braces_escape() {
        let row = json!({"a": {"b": [10, 20]}, "s": "x"});
        assert_eq!(
            fill("{a.b.1}/{s} {{lit}} {#}", Some(&row), 2),
            "20/x {lit} 3"
        );
        assert!(placeholders("{a b}").is_err());
        assert!(placeholders("a } b").is_err());
        assert_eq!(placeholders("{{x}} {y}").unwrap(), ["y"]);
    }
}
