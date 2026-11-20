//! The act journal: one immutable record for every act of a person or an agent
//! in their own research (access/contracts/act-record.v1.schema.json), and the
//! folds that turn the journal into a board, a review queue, leases, presence
//! and history. A record is local personal work: it never writes ToS source,
//! review, rights or canon, and a review decision here is the person's own.
//!
//! The journal keeps only what its folds need; bodies stay with the host's
//! store (IndexedDB in the browser, the bridge's chosen store natively).

use std::collections::{HashMap, HashSet};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, jcs_bytes_v1, parse_json,
};

pub const ACT_SCHEMA: &str = "tos.act.v1";
pub const ACT_KINDS_SCHEMA: &str = "tos.act.kinds.v1";
const MAX_RECORD_BYTES: usize = 65_536;
const MAX_BATCH_BYTES: usize = 16 * 1_048_576;
const MAX_BATCH_RECORDS: usize = 5_000;
const MAX_TARGETS: usize = 64;
const MAX_GROUNDS: usize = 64;
const MAX_PARENTS: usize = 32;
const MAX_ADDRESS: usize = 1_024;
const MAX_INTENT: usize = 280;
const MAX_WITHDRAWAL_DEPTH: u8 = 32;
const MAX_LEASE_MS: u64 = 86_400_000;
const MAX_QUERY_LIMIT: usize = 1_000;
const MAX_PRESENCE_SCAN: usize = 200_000;
const MAX_REGISTRY_BYTES: usize = 1_048_576;

// ---------------------------------------------------------------- JSON helpers

fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn int(value: i128) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn boolean(value: bool) -> JsonValue {
    JsonValue::Bool(value)
}
fn array(items: Vec<JsonValue>) -> JsonValue {
    JsonValue::Array(items)
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn emit(value: &JsonValue) -> Result<Vec<u8>, String> {
    emit_value_preserved_json(
        value,
        JsonLimits {
            max_bytes: 64 * 1_048_576,
            max_visits: 10_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "act_output_budget".to_owned())
}
fn parse(raw: &[u8], max_bytes: usize) -> Result<JsonValue, String> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map(|document| document.into_root())
    .map_err(|_| "act_invalid_json".to_owned())
}
fn integer(value: &JsonValue) -> Option<i128> {
    match value {
        JsonValue::Number(number) if number.kind == JsonNumberKind::Int => {
            number.lexeme.parse().ok()
        }
        _ => None,
    }
}
fn invalid(field: &str) -> String {
    format!("act_invalid:{field}")
}
fn plain(text: &str) -> bool {
    !text.chars().any(|c| c.is_control())
}
fn bounded(text: &str, max: usize) -> bool {
    !text.is_empty() && text.chars().count() <= max && plain(text)
}
fn non_empty(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::String(text) => text.as_str().is_some_and(|text| !text.trim().is_empty()),
        JsonValue::Array(items) => !items.is_empty(),
        JsonValue::Object(fields) => !fields.is_empty(),
        _ => true,
    }
}
fn string_list(value: &JsonValue, max_items: usize, max_len: usize) -> Option<Vec<String>> {
    let items = value.as_array()?;
    if items.len() > max_items {
        return None;
    }
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let text = item.as_str()?;
        if !bounded(text, max_len) || !seen.insert(text) {
            return None;
        }
        out.push(text.to_owned());
    }
    Some(out)
}

fn valid_act_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}
fn segment(text: &str, max: usize, dots: bool) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= max
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || (dots && *b == b'.')
        })
}
fn valid_base_kind(kind: &str) -> bool {
    kind.as_bytes().first().is_some_and(u8::is_ascii_lowercase) && segment(kind, 48, false)
}
fn valid_declared_kind(kind: &str) -> bool {
    let Some(rest) = kind.strip_prefix("x.") else {
        return false;
    };
    let Some((namespace, name)) = rest.split_once('.') else {
        return false;
    };
    segment(namespace, 32, false) && segment(name, 64, true)
}

/// Milliseconds since the epoch of a canonical UTC timestamp, as JavaScript's
/// Date#toISOString writes it.
fn utc_ms(value: &str) -> Option<i64> {
    let (year, tail) = match value.as_bytes().first()? {
        b'+' | b'-' if value.len() == 27 => (value.get(0..7)?, value.get(7..)?),
        b'0'..=b'9' if value.len() == 24 => (value.get(0..4)?, value.get(4..)?),
        _ => return None,
    };
    let digits = year.trim_start_matches(['+', '-']);
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i64 = year.parse().ok()?;
    let t = tail.as_bytes();
    if t.len() != 20
        || t[0] != b'-'
        || t[3] != b'-'
        || t[6] != b'T'
        || t[9] != b':'
        || t[12] != b':'
        || t[15] != b'.'
        || t[19] != b'Z'
    {
        return None;
    }
    let part = |start: usize, end: usize| -> Option<i64> {
        tail.get(start..end)
            .filter(|text| text.bytes().all(|b| b.is_ascii_digit()))?
            .parse()
            .ok()
    };
    let (month, day, hour, minute, second, millis) = (
        part(1, 3)?,
        part(4, 6)?,
        part(7, 9)?,
        part(10, 12)?,
        part(13, 15)?,
        part(16, 19)?,
    );
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month[(month - 1) as usize]
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let ms = ((days * 24 + hour) * 60 + minute) * 60_000 + second * 1_000 + millis;
    (ms.abs() <= 8_640_000_000_000_000).then_some(ms)
}

// ---------------------------------------------------------------- registry

#[derive(Clone, Debug)]
struct KindRule {
    grounds_required: bool,
    parents_required: bool,
    body_required: Vec<String>,
    target_prefix: Vec<String>,
    description: String,
}

#[derive(Clone, Debug)]
struct Category {
    variants: Vec<String>,
    change_required: Vec<String>,
    group_by: Vec<String>,
}

#[derive(Clone, Debug)]
struct Registry {
    kinds: HashMap<String, KindRule>,
    kind_order: Vec<String>,
    decisions: Vec<String>,
    categories: HashMap<String, Category>,
    category_order: Vec<String>,
}

fn rule_from(value: &JsonValue) -> Result<KindRule, String> {
    let choice = |name: &str| -> Result<bool, String> {
        match get(value, name).and_then(JsonValue::as_str) {
            Some("required") => Ok(true),
            Some("optional") => Ok(false),
            _ => Err(format!("act_registry_invalid:{name}")),
        }
    };
    let list = |name: &str| -> Result<Vec<String>, String> {
        match get(value, name) {
            None => Ok(Vec::new()),
            Some(items) => {
                string_list(items, 32, 128).ok_or_else(|| format!("act_registry_invalid:{name}"))
            }
        }
    };
    Ok(KindRule {
        grounds_required: choice("grounds")?,
        parents_required: choice("parents")?,
        body_required: list("body_required")?,
        target_prefix: list("target_prefix")?,
        description: get(value, "description")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect(),
    })
}

fn registry_from(value: &JsonValue) -> Result<Registry, String> {
    if get(value, "schema").and_then(JsonValue::as_str) != Some(ACT_KINDS_SCHEMA) {
        return Err("act_registry_invalid:schema".to_owned());
    }
    let mut kinds = HashMap::new();
    let mut kind_order = Vec::new();
    for item in get(value, "kinds")
        .and_then(JsonValue::as_array)
        .ok_or("act_registry_invalid:kinds")?
    {
        let name = get(item, "name")
            .and_then(JsonValue::as_str)
            .filter(|name| valid_base_kind(name))
            .ok_or("act_registry_invalid:kind_name")?;
        if kinds.insert(name.to_owned(), rule_from(item)?).is_some() {
            return Err("act_registry_invalid:duplicate_kind".to_owned());
        }
        kind_order.push(name.to_owned());
    }
    let decisions = get(value, "decisions")
        .and_then(|items| string_list(items, 16, 48))
        .filter(|items| !items.is_empty())
        .ok_or("act_registry_invalid:decisions")?;
    let mut categories = HashMap::new();
    let mut category_order = Vec::new();
    for item in get(value, "categories")
        .and_then(JsonValue::as_array)
        .ok_or("act_registry_invalid:categories")?
    {
        let name = get(item, "name")
            .and_then(JsonValue::as_str)
            .filter(|name| segment(name, 48, false))
            .ok_or("act_registry_invalid:category_name")?;
        let list = |field: &str| -> Result<Vec<String>, String> {
            get(item, field)
                .map_or(Some(Vec::new()), |items| string_list(items, 32, 64))
                .ok_or_else(|| format!("act_registry_invalid:{field}"))
        };
        let category = Category {
            variants: list("variants")?,
            change_required: list("change_required")?,
            group_by: list("group_by")?,
        };
        // A stand may add a category; a later entry with the same name refines it.
        if categories.insert(name.to_owned(), category).is_none() {
            category_order.push(name.to_owned());
        }
    }
    Ok(Registry {
        kinds,
        kind_order,
        decisions,
        categories,
        category_order,
    })
}

// ---------------------------------------------------------------- journal

#[derive(Default)]
struct Interner {
    map: HashMap<Box<str>, u32>,
    items: Vec<Box<str>>,
}
impl Interner {
    fn intern(&mut self, text: &str) -> u32 {
        if let Some(index) = self.map.get(text) {
            return *index;
        }
        let index = self.items.len() as u32;
        let boxed: Box<str> = text.into();
        self.items.push(boxed.clone());
        self.map.insert(boxed, index);
        index
    }
    fn find(&self, text: &str) -> Option<u32> {
        self.map.get(text).copied()
    }
    fn text(&self, index: u32) -> &str {
        &self.items[index as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorType {
    Human,
    Agent,
    Mixed,
}
impl AuthorType {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "human" => Some(Self::Human),
            "agent" => Some(Self::Agent),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
    const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Clone, Debug)]
enum Extra {
    None,
    Ticket { category: u32 },
    Proposal { category: u32, group: u32 },
    Review { decision: u32 },
    Lease { until_ms: i64 },
    Assignment { assignee: u32 },
    Withdrawal { depth: u8 },
}

struct Act {
    seq: u64,
    kind: u32,
    known: bool,
    author_type: AuthorType,
    author: u32,
    model: Option<u32>,
    role: Option<u32>,
    squad: Option<u32>,
    at_ms: i64,
    targets: Box<[u32]>,
    parents: Box<[u32]>,
    replaces: Option<u32>,
    extra: Extra,
}

/// The fold state of one journal. Every method that changes it appends; nothing
/// is edited or removed.
pub struct ActJournal {
    registry: Registry,
    declared: HashMap<String, KindRule>,
    declared_order: Vec<String>,
    ids: Interner,
    strings: Interner,
    acts: Vec<Act>,
    next_seq: u64,
    by_target: HashMap<u32, Vec<u32>>,
    children: HashMap<u32, Vec<u32>>,
    replaced_by: HashMap<u32, Vec<u32>>,
    withdrawals: HashMap<u32, Vec<u32>>,
    group_members: HashMap<u32, Vec<u32>>,
    group_order: Vec<u32>,
    group_reviews: HashMap<u32, Vec<u32>>,
    direct_reviews: HashMap<u32, Vec<u32>>,
    assignments: HashMap<u32, Vec<u32>>,
    tickets: Vec<u32>,
    leases: Vec<u32>,
    unknown_kinds: HashMap<u32, u64>,
}

/// Digest of the record's canonical bytes without its digest field. The field is
/// taken out for the computation and put back where it was, so nothing is copied.
fn digest_of(record: &mut JsonValue) -> Result<Digest256, String> {
    let JsonValue::Object(fields) = record else {
        return Err(invalid("record"));
    };
    let position = fields
        .iter()
        .position(|(key, _)| key.as_str() == Some("digest"));
    let taken = position.map(|at| fields.remove(at));
    let bytes = jcs_bytes_v1(
        record,
        JsonLimits {
            max_bytes: MAX_RECORD_BYTES,
            ..JsonLimits::default()
        },
    );
    if let (Some(at), Some(field), JsonValue::Object(fields)) = (position, taken, &mut *record) {
        fields.insert(at, field);
    }
    let bytes = bytes.map_err(|_| invalid("canonical_bytes"))?;
    Ok(Digest256::of_bytes(&bytes))
}

/// The digest an act record carries: SHA-256 of its canonical bytes without
/// the digest field. A trace marker, never a signature.
pub fn act_digest_v1(raw: &[u8]) -> Result<String, String> {
    let mut record = parse(raw, MAX_RECORD_BYTES)?;
    Ok(digest_of(&mut record)?.to_prefixed())
}

const TOP_LEVEL: &[&str] = &[
    "schema", "id", "kind", "author", "targets", "grounds", "intent", "body", "parents",
    "replaces", "at", "posture", "digest",
];
const AUTHOR_FIELDS: &[&str] = &["type", "id", "model", "role", "squad", "session"];

impl ActJournal {
    /// A journal under the base registry (act-kinds.v1.json), possibly with a
    /// stand's categories merged in by the host.
    pub fn new(registry_json: &[u8]) -> Result<Self, String> {
        let registry = registry_from(&parse(registry_json, MAX_REGISTRY_BYTES)?)?;
        Ok(Self {
            registry,
            declared: HashMap::new(),
            declared_order: Vec::new(),
            ids: Interner::default(),
            strings: Interner::default(),
            acts: Vec::new(),
            next_seq: 1,
            by_target: HashMap::new(),
            children: HashMap::new(),
            replaced_by: HashMap::new(),
            withdrawals: HashMap::new(),
            group_members: HashMap::new(),
            group_order: Vec::new(),
            group_reviews: HashMap::new(),
            direct_reviews: HashMap::new(),
            assignments: HashMap::new(),
            tickets: Vec::new(),
            leases: Vec::new(),
            unknown_kinds: HashMap::new(),
        })
    }

    pub fn len(&self) -> usize {
        self.acts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.acts.is_empty()
    }

    fn act_index(&self, id: &str) -> Option<u32> {
        self.ids.find(id)
    }
    fn act_address(&self, address: &str) -> Option<u32> {
        address
            .strip_prefix("act:")
            .and_then(|id| self.act_index(id))
    }

    fn rule(&self, kind: &str) -> Option<&KindRule> {
        self.registry
            .kinds
            .get(kind)
            .or_else(|| self.declared.get(kind))
    }

    /// Append one record. A record without `digest` receives one; a record with
    /// one (restored from a store) must match it. Returns the stored record, its
    /// sequence number and the targets on which another author holds a lease.
    pub fn append(&mut self, raw: &[u8]) -> Result<Vec<u8>, String> {
        let record = parse(raw, MAX_RECORD_BYTES)?;
        let (stored, seq, collisions) = self.append_value(record)?;
        emit(&object(vec![
            ("seq", int(seq as i128)),
            ("record", stored),
            (
                "collisions",
                array(collisions.iter().map(|t| string(t)).collect()),
            ),
        ]))
    }

    /// Append records in order; stops at the first refused record and reports
    /// how many were appended before it.
    pub fn append_batch(&mut self, raw: &[u8]) -> Result<Vec<u8>, String> {
        let value = parse(raw, MAX_BATCH_BYTES)?;
        let JsonValue::Array(items) = value else {
            return Err(invalid("batch"));
        };
        if items.len() > MAX_BATCH_RECORDS {
            return Err(invalid("batch_size"));
        }
        let mut appended = Vec::with_capacity(items.len());
        let mut failure = None;
        for (index, item) in items.into_iter().enumerate() {
            match self.append_value(item) {
                Ok((record, seq, collisions)) => appended.push(object(vec![
                    ("seq", int(seq as i128)),
                    ("id", get(&record, "id").cloned().unwrap_or(JsonValue::Null)),
                    (
                        "digest",
                        get(&record, "digest").cloned().unwrap_or(JsonValue::Null),
                    ),
                    (
                        "collisions",
                        array(collisions.iter().map(|t| string(t)).collect()),
                    ),
                ])),
                Err(error) => {
                    failure = Some(object(vec![
                        ("index", int(index as i128)),
                        ("error", string(&error)),
                    ]));
                    break;
                }
            }
        }
        emit(&object(vec![
            ("appended", array(appended)),
            ("failure", failure.unwrap_or(JsonValue::Null)),
        ]))
    }

    fn append_value(
        &mut self,
        mut record: JsonValue,
    ) -> Result<(JsonValue, u64, Vec<String>), String> {
        let JsonValue::Object(fields) = &record else {
            return Err(invalid("record"));
        };
        for (key, _) in fields {
            if !key.as_str().is_some_and(|key| TOP_LEVEL.contains(&key)) {
                return Err(invalid("unknown_field"));
            }
        }
        if get(&record, "schema").and_then(JsonValue::as_str) != Some(ACT_SCHEMA) {
            return Err(invalid("schema"));
        }
        let id = get(&record, "id")
            .and_then(JsonValue::as_str)
            .filter(|id| valid_act_id(id))
            .ok_or_else(|| invalid("id"))?
            .to_owned();
        if self.act_index(&id).is_some() {
            return Err("act_duplicate_id".to_owned());
        }
        let kind = get(&record, "kind")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid("kind"))?
            .to_owned();
        let base = self.registry.kinds.contains_key(&kind);
        if !base && !valid_declared_kind(&kind) {
            return Err(invalid("kind"));
        }
        let rule = self.rule(&kind).cloned();
        let known = rule.is_some();

        // author
        let author = get(&record, "author").ok_or_else(|| invalid("author"))?;
        let JsonValue::Object(author_fields) = author else {
            return Err(invalid("author"));
        };
        for (key, value) in author_fields {
            if !key.as_str().is_some_and(|key| AUTHOR_FIELDS.contains(&key)) {
                return Err(invalid("author"));
            }
            if !value.as_str().is_some_and(|text| bounded(text, 128)) {
                return Err(invalid("author"));
            }
        }
        let author_type = get(author, "type")
            .and_then(JsonValue::as_str)
            .and_then(AuthorType::parse)
            .ok_or_else(|| invalid("author.type"))?;
        let author_id = get(author, "id")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid("author.id"))?
            .to_owned();
        let optional = |name: &str| {
            get(author, name)
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
        };
        let (model, role, squad) = (optional("model"), optional("role"), optional("squad"));
        if author_type == AuthorType::Agent && model.is_none() {
            return Err(invalid("author.model"));
        }
        if role.as_ref().is_some_and(|role| role.chars().count() > 64) {
            return Err(invalid("author.role"));
        }

        // addresses
        let targets = get(&record, "targets")
            .and_then(|items| string_list(items, MAX_TARGETS, MAX_ADDRESS))
            .filter(|items| !items.is_empty())
            .ok_or_else(|| invalid("targets"))?;
        let grounds = get(&record, "grounds")
            .and_then(|items| string_list(items, MAX_GROUNDS, MAX_ADDRESS))
            .ok_or_else(|| invalid("grounds"))?;
        let intent = get(&record, "intent")
            .and_then(JsonValue::as_str)
            .filter(|text| bounded(text, MAX_INTENT))
            .ok_or_else(|| invalid("intent"))?;
        let _ = intent;
        let body = get(&record, "body")
            .filter(|body| matches!(body, JsonValue::Object(_)))
            .ok_or_else(|| invalid("body"))?
            .clone();
        let body = &body;
        let parent_ids = get(&record, "parents")
            .and_then(|items| string_list(items, MAX_PARENTS, 128))
            .ok_or_else(|| invalid("parents"))?;
        let mut parents = Vec::with_capacity(parent_ids.len());
        for parent in &parent_ids {
            parents.push(self.act_index(parent).ok_or("act_missing_parent")?);
        }
        let replaces = match get(&record, "replaces") {
            None => None,
            Some(value) => {
                let replaced = value
                    .as_str()
                    .and_then(|id| self.act_index(id))
                    .ok_or("act_missing_replaced")?;
                if self.strings.text(self.acts[replaced as usize].kind) != kind {
                    return Err(invalid("replaces_kind"));
                }
                Some(replaced)
            }
        };
        let at = get(&record, "at")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid("at"))?;
        let at_ms = utc_ms(at).ok_or_else(|| invalid("at"))?;
        let posture = get(&record, "posture").ok_or_else(|| invalid("posture"))?;
        let posture_ok = matches!(posture, JsonValue::Object(items) if items.len() == 4)
            && get(posture, "local_only").and_then(JsonValue::as_bool) == Some(true)
            && get(posture, "source").and_then(JsonValue::as_bool) == Some(false)
            && get(posture, "reviewed_by_tree").and_then(JsonValue::as_bool) == Some(false)
            && get(posture, "canon").and_then(JsonValue::as_bool) == Some(false);
        if !posture_ok {
            return Err(invalid("posture"));
        }

        // kind rules
        match &rule {
            Some(rule) => {
                if rule.grounds_required && grounds.is_empty() {
                    return Err(invalid("grounds_required"));
                }
                if rule.parents_required && parents.is_empty() {
                    return Err(invalid("parents_required"));
                }
                for field in &rule.body_required {
                    if !get(body, field).is_some_and(non_empty) {
                        return Err(invalid(&format!("body.{field}")));
                    }
                }
                if !rule.target_prefix.is_empty()
                    && !targets
                        .iter()
                        .all(|t| rule.target_prefix.iter().any(|p| t.starts_with(p.as_str())))
                {
                    return Err(invalid("target_prefix"));
                }
            }
            // An undeclared kind keeps the invariants and must show its grounds.
            None if grounds.is_empty() => return Err(invalid("grounds_required")),
            None => {}
        }

        let mut extra = Extra::None;
        match kind.as_str() {
            "ticket" => {
                let category = self.category_of(body, false)?;
                extra = Extra::Ticket {
                    category: self.strings.intern(&category),
                };
            }
            "proposal" => {
                let category = self.category_of(body, true)?;
                let group = self.group_address(&category, body, &targets)?;
                extra = Extra::Proposal {
                    category: self.strings.intern(&category),
                    group: self.strings.intern(&group),
                };
            }
            "review" => {
                let decision = get(body, "decision")
                    .and_then(JsonValue::as_str)
                    .filter(|d| self.registry.decisions.iter().any(|known| known == d))
                    .ok_or_else(|| invalid("body.decision"))?
                    .to_owned();
                if decision == "accept-with-edit" && !get(body, "edit").is_some_and(non_empty) {
                    return Err(invalid("body.edit"));
                }
                for target in &targets {
                    if target.starts_with("act:") {
                        let index = self.act_address(target).ok_or("act_missing_target")?;
                        if !matches!(self.acts[index as usize].extra, Extra::Proposal { .. }) {
                            return Err(invalid("review_target"));
                        }
                    }
                }
                extra = Extra::Review {
                    decision: self.strings.intern(&decision),
                };
            }
            "withdrawal" => {
                let mut depth = 1;
                for target in &targets {
                    let index = self.act_address(target).ok_or("act_missing_target")?;
                    if let Extra::Withdrawal { depth: inner } = self.acts[index as usize].extra {
                        depth = depth.max(inner + 1);
                    }
                }
                if depth > MAX_WITHDRAWAL_DEPTH {
                    return Err(invalid("withdrawal_depth"));
                }
                extra = Extra::Withdrawal { depth };
            }
            "lease" => {
                let ttl = get(body, "ttl_ms")
                    .and_then(integer)
                    .filter(|ttl| (1..=MAX_LEASE_MS as i128).contains(ttl))
                    .ok_or_else(|| invalid("body.ttl_ms"))?;
                extra = Extra::Lease {
                    until_ms: at_ms + ttl as i64,
                };
            }
            "assignment" => {
                let assignee = get(body, "assignee")
                    .and_then(JsonValue::as_str)
                    .filter(|text| bounded(text, 128))
                    .ok_or_else(|| invalid("body.assignee"))?;
                extra = Extra::Assignment {
                    assignee: self.strings.intern(assignee),
                };
            }
            "kind-declaration" => self.check_declaration(body, &targets)?,
            _ => {}
        }

        // digest
        let digest = digest_of(&mut record)?;
        match get(&record, "digest") {
            None => {
                if let JsonValue::Object(fields) = &mut record {
                    fields.push((
                        JsonString::from_utf8("digest"),
                        string(&digest.to_prefixed()),
                    ));
                }
            }
            Some(value) => {
                let stated = value
                    .as_str()
                    .and_then(|text| Digest256::from_prefixed(text).ok())
                    .ok_or_else(|| invalid("digest"))?;
                if stated != digest {
                    return Err("act_digest_mismatch".to_owned());
                }
            }
        }

        // Advisory collisions: a live lease of another author on a target.
        let collisions: Vec<String> = targets
            .iter()
            .filter(|target| {
                self.strings.find(target).is_some_and(|t| {
                    self.live_lease_holders(t, at_ms)
                        .iter()
                        .any(|holder| *holder != author_id)
                })
            })
            .cloned()
            .collect();

        // commit
        let index = self.ids.intern(&id);
        debug_assert_eq!(index as usize, self.acts.len());
        let seq = self.next_seq;
        self.next_seq += 1;
        let kind_index = self.strings.intern(&kind);
        let target_indices: Vec<u32> = targets.iter().map(|t| self.strings.intern(t)).collect();
        let act = Act {
            seq,
            kind: kind_index,
            known,
            author_type,
            author: self.strings.intern(&author_id),
            model: model.as_deref().map(|m| self.strings.intern(m)),
            role: role.as_deref().map(|r| self.strings.intern(r)),
            squad: squad.as_deref().map(|s| self.strings.intern(s)),
            at_ms,
            targets: target_indices.clone().into_boxed_slice(),
            parents: parents.clone().into_boxed_slice(),
            replaces,
            extra: extra.clone(),
        };
        for target in &target_indices {
            self.by_target.entry(*target).or_default().push(index);
        }
        for parent in &parents {
            self.children.entry(*parent).or_default().push(index);
        }
        if let Some(replaced) = replaces {
            self.replaced_by.entry(replaced).or_default().push(index);
        }
        match &extra {
            Extra::Ticket { .. } => self.tickets.push(index),
            Extra::Proposal { group, .. } => {
                let members = self.group_members.entry(*group).or_default();
                if members.is_empty() {
                    self.group_order.push(*group);
                }
                members.push(index);
            }
            Extra::Review { .. } => {
                for target in &targets {
                    if let Some(proposal) = self.act_address(target) {
                        self.direct_reviews.entry(proposal).or_default().push(index);
                    } else if target.starts_with("group:") {
                        let group = self.strings.intern(target);
                        self.group_reviews.entry(group).or_default().push(index);
                    }
                }
            }
            Extra::Withdrawal { .. } => {
                for target in &targets {
                    if let Some(withdrawn) = self.act_address(target) {
                        self.withdrawals.entry(withdrawn).or_default().push(index);
                    }
                }
            }
            Extra::Lease { .. } => self.leases.push(index),
            Extra::Assignment { .. } => {
                for target in &targets {
                    if let Some(assigned) = self.act_address(target) {
                        self.assignments.entry(assigned).or_default().push(index);
                    }
                }
            }
            Extra::None => {}
        }
        if kind == "kind-declaration" {
            let declares = get(body, "declares").ok_or_else(|| invalid("body.declares"))?;
            let name = get(declares, "name")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_owned();
            let rule = rule_from(declares)?;
            if !self.declared.contains_key(&name) {
                self.declared_order.push(name.clone());
            }
            self.declared.insert(name, rule);
        }
        if !known {
            *self.unknown_kinds.entry(kind_index).or_default() += 1;
        }
        self.acts.push(act);
        Ok((record, seq, collisions))
    }

    fn category_of(&self, body: &JsonValue, change: bool) -> Result<String, String> {
        let category = get(body, "category")
            .and_then(JsonValue::as_str)
            .filter(|name| segment(name, 48, false))
            .ok_or_else(|| invalid("body.category"))?;
        if let Some(known) = self.registry.categories.get(category) {
            if let Some(variant) = get(body, "variant") {
                let variant = variant.as_str().ok_or_else(|| invalid("body.variant"))?;
                if !known.variants.is_empty() && !known.variants.iter().any(|v| v == variant) {
                    return Err(invalid("body.variant"));
                }
            }
            if change {
                let value = get(body, "change").ok_or_else(|| invalid("body.change"))?;
                if !matches!(value, JsonValue::Object(_)) {
                    return Err(invalid("body.change"));
                }
                for field in &known.change_required {
                    if !get(value, field).is_some_and(non_empty) {
                        return Err(invalid(&format!("body.change.{field}")));
                    }
                }
            }
        } else if change && !matches!(get(body, "change"), Some(JsonValue::Object(_))) {
            return Err(invalid("body.change"));
        }
        Ok(category.to_owned())
    }

    /// group:<category>:<32 hex>. A category that groups by fields joins every
    /// proposal making the same change; otherwise only identical proposals on
    /// the same targets share a group.
    fn group_address(
        &self,
        category: &str,
        body: &JsonValue,
        targets: &[String],
    ) -> Result<String, String> {
        let change = get(body, "change").ok_or_else(|| invalid("body.change"))?;
        let group_by = self
            .registry
            .categories
            .get(category)
            .map(|known| known.group_by.clone())
            .unwrap_or_default();
        let material = if group_by.is_empty() {
            let mut sorted = targets.to_vec();
            sorted.sort();
            object(vec![
                ("category", string(category)),
                ("targets", array(sorted.iter().map(|t| string(t)).collect())),
                ("change", change.clone()),
            ])
        } else {
            let mut fields: Vec<(&str, JsonValue)> = group_by
                .iter()
                .map(|field| {
                    (
                        field.as_str(),
                        get(change, field).cloned().unwrap_or(JsonValue::Null),
                    )
                })
                .collect();
            fields.push(("category", string(category)));
            object(fields)
        };
        let bytes = jcs_bytes_v1(
            &material,
            JsonLimits {
                max_bytes: MAX_RECORD_BYTES,
                ..JsonLimits::default()
            },
        )
        .map_err(|_| invalid("body.change"))?;
        let hex = Digest256::of_bytes(&bytes).to_hex();
        Ok(format!("group:{category}:{}", &hex[..32]))
    }

    fn check_declaration(&self, body: &JsonValue, targets: &[String]) -> Result<(), String> {
        let declares = get(body, "declares").ok_or_else(|| invalid("body.declares"))?;
        let name = get(declares, "name")
            .and_then(JsonValue::as_str)
            .filter(|name| valid_declared_kind(name))
            .ok_or_else(|| invalid("body.declares.name"))?;
        if targets.len() != 1 || targets[0] != format!("kind:{name}") {
            return Err(invalid("target_prefix"));
        }
        rule_from(declares)?;
        if self.declared.contains_key(name) {
            return Err("act_kind_already_declared".to_owned());
        }
        Ok(())
    }

    // ------------------------------------------------------------ folds

    /// Withdrawn when an effective withdrawal targets it; withdrawing a
    /// withdrawal restores. Depth is bounded at append.
    fn withdrawn(&self, index: u32) -> bool {
        self.withdrawals
            .get(&index)
            .is_some_and(|items| items.iter().any(|w| !self.withdrawn(*w)))
    }
    fn replaced(&self, index: u32) -> bool {
        self.replaced_by
            .get(&index)
            .is_some_and(|items| items.iter().any(|r| !self.withdrawn(*r)))
    }
    fn live(&self, index: u32) -> bool {
        !self.withdrawn(index) && !self.replaced(index)
    }

    fn live_lease_holders(&self, target: u32, now_ms: i64) -> Vec<String> {
        let Some(items) = self.by_target.get(&target) else {
            return Vec::new();
        };
        let mut holders: Vec<String> = Vec::new();
        for index in items {
            let act = &self.acts[*index as usize];
            let Extra::Lease { until_ms } = act.extra else {
                continue;
            };
            if until_ms <= now_ms || self.withdrawn(*index) {
                continue;
            }
            let released = items.iter().any(|other| {
                let later = &self.acts[*other as usize];
                later.seq > act.seq
                    && later.author == act.author
                    && self.strings.text(later.kind) == "release"
                    && !self.withdrawn(*other)
            });
            let holder = self.strings.text(act.author).to_owned();
            if !released && !holders.contains(&holder) {
                holders.push(holder);
            }
        }
        holders
    }

    /// The effective decision on a proposal: the latest live review that
    /// names it, or names its group after it was made.
    fn decision(&self, proposal: u32) -> Option<(u32, u32)> {
        let act = &self.acts[proposal as usize];
        let Extra::Proposal { group, .. } = act.extra else {
            return None;
        };
        let direct = self.direct_reviews.get(&proposal).into_iter().flatten();
        let grouped = self
            .group_reviews
            .get(&group)
            .into_iter()
            .flatten()
            .filter(|review| self.acts[**review as usize].seq > act.seq);
        direct
            .chain(grouped)
            .filter(|review| !self.withdrawn(**review))
            .max_by_key(|review| self.acts[**review as usize].seq)
            .and_then(|review| match self.acts[*review as usize].extra {
                Extra::Review { decision } => Some((*review, decision)),
                _ => None,
            })
    }

    fn author_value(&self, act: &Act) -> JsonValue {
        let mut fields = vec![
            ("type", string(act.author_type.as_str())),
            ("id", string(self.strings.text(act.author))),
        ];
        for (name, value) in [
            ("model", act.model),
            ("role", act.role),
            ("squad", act.squad),
        ] {
            if let Some(value) = value {
                fields.push((name, string(self.strings.text(value))));
            }
        }
        object(fields)
    }

    fn act_value(&self, index: u32) -> JsonValue {
        let act = &self.acts[index as usize];
        object(vec![
            ("id", string(self.ids.text(index))),
            ("seq", int(act.seq as i128)),
            ("kind", string(self.strings.text(act.kind))),
            ("known_kind", boolean(act.known)),
            ("author", self.author_value(act)),
            ("at_ms", int(act.at_ms as i128)),
            (
                "targets",
                array(
                    act.targets
                        .iter()
                        .map(|t| string(self.strings.text(*t)))
                        .collect(),
                ),
            ),
            ("withdrawn", boolean(self.withdrawn(index))),
            ("replaced", boolean(self.replaced(index))),
        ])
    }

    /// Answer one fold request: summary, act, target, board, review, leases,
    /// presence, history or kinds.
    pub fn query(&self, raw: &[u8]) -> Result<Vec<u8>, String> {
        let request = parse(raw, 65_536)?;
        let view = get(&request, "view")
            .and_then(JsonValue::as_str)
            .ok_or("act_query_invalid:view")?;
        let limit = match get(&request, "limit") {
            None => 100,
            Some(value) => integer(value)
                .filter(|n| (1..=MAX_QUERY_LIMIT as i128).contains(n))
                .ok_or("act_query_invalid:limit")? as usize,
        };
        let number = |name: &str| -> Result<i64, String> {
            get(&request, name)
                .and_then(integer)
                .and_then(|n| i64::try_from(n).ok())
                .ok_or_else(|| format!("act_query_invalid:{name}"))
        };
        let text = |name: &str| -> Result<&str, String> {
            get(&request, name)
                .and_then(JsonValue::as_str)
                .ok_or_else(|| format!("act_query_invalid:{name}"))
        };
        let value = match view {
            "summary" => self.summary(),
            "act" => {
                let index = self.act_index(text("id")?).ok_or("act_query_unknown")?;
                self.act_value(index)
            }
            "target" => self.target_view(text("address")?, limit),
            "board" => self.board(limit),
            "review" => self.review(limit),
            "leases" => self.lease_view(number("now_ms")?, limit),
            "presence" => self.presence(number("now_ms")?, number("window_ms")?, limit),
            "history" => self.history(self.act_index(text("id")?).ok_or("act_query_unknown")?),
            "kinds" => self.kinds(),
            _ => return Err("act_query_invalid:view".to_owned()),
        };
        emit(&value)
    }

    fn summary(&self) -> JsonValue {
        let mut by_kind: HashMap<u32, u64> = HashMap::new();
        let (mut human, mut agent, mut mixed) = (0u64, 0u64, 0u64);
        let mut agents = HashSet::new();
        let mut models = HashSet::new();
        for act in &self.acts {
            *by_kind.entry(act.kind).or_default() += 1;
            match act.author_type {
                AuthorType::Human => human += 1,
                AuthorType::Agent => {
                    agent += 1;
                    agents.insert(act.author);
                    if let Some(model) = act.model {
                        models.insert(model);
                    }
                }
                AuthorType::Mixed => mixed += 1,
            }
        }
        let mut kinds: Vec<(u32, u64)> = by_kind.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut unknown: Vec<(u32, u64)> =
            self.unknown_kinds.iter().map(|(k, v)| (*k, *v)).collect();
        unknown.sort();
        object(vec![
            ("acts", int(self.acts.len() as i128)),
            ("next_seq", int(self.next_seq as i128)),
            (
                "by_kind",
                array(
                    kinds
                        .iter()
                        .map(|(kind, count)| {
                            object(vec![
                                ("kind", string(self.strings.text(*kind))),
                                ("count", int(*count as i128)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "by_author_type",
                object(vec![
                    ("human", int(human as i128)),
                    ("agent", int(agent as i128)),
                    ("mixed", int(mixed as i128)),
                ]),
            ),
            ("agents", int(agents.len() as i128)),
            ("models", int(models.len() as i128)),
            ("tickets", int(self.tickets.len() as i128)),
            ("groups", int(self.group_order.len() as i128)),
            (
                "declared_kinds",
                array(self.declared_order.iter().map(|k| string(k)).collect()),
            ),
            (
                "unknown_kinds",
                array(
                    unknown
                        .iter()
                        .map(|(kind, count)| {
                            object(vec![
                                ("kind", string(self.strings.text(*kind))),
                                ("count", int(*count as i128)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    fn target_view(&self, address: &str, limit: usize) -> JsonValue {
        let items = self
            .strings
            .find(address)
            .and_then(|t| self.by_target.get(&t))
            .map(Vec::as_slice)
            .unwrap_or_default();
        object(vec![
            ("address", string(address)),
            ("count", int(items.len() as i128)),
            (
                "acts",
                array(
                    items
                        .iter()
                        .rev()
                        .take(limit)
                        .map(|i| self.act_value(*i))
                        .collect(),
                ),
            ),
        ])
    }

    fn ticket_status(&self, ticket: u32) -> &'static str {
        if self.withdrawn(ticket) {
            return "withdrawn";
        }
        let children = self
            .children
            .get(&ticket)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let live_children: Vec<u32> = children
            .iter()
            .copied()
            .filter(|c| !self.withdrawn(*c))
            .collect();
        let address = format!("act:{}", self.ids.text(ticket));
        let reviewed_directly = self
            .strings
            .find(&address)
            .and_then(|t| self.by_target.get(&t))
            .is_some_and(|items| {
                items.iter().any(|i| {
                    matches!(self.acts[*i as usize].extra, Extra::Review { .. })
                        && !self.withdrawn(*i)
                })
            });
        let decided = live_children.iter().any(|child| {
            self.decision(*child)
                .is_some_and(|(_, decision)| self.strings.text(decision) != "defer")
        });
        if reviewed_directly || decided {
            "reviewed"
        } else if !live_children.is_empty() {
            "returned"
        } else if self
            .assignments
            .get(&ticket)
            .is_some_and(|items| items.iter().any(|a| !self.withdrawn(*a)))
        {
            "with-agent"
        } else {
            "open"
        }
    }

    fn board(&self, limit: usize) -> JsonValue {
        let mut counts: HashMap<&'static str, u64> = HashMap::new();
        let mut items = Vec::new();
        for ticket in self.tickets.iter().rev() {
            if self.replaced(*ticket) {
                continue;
            }
            let status = self.ticket_status(*ticket);
            *counts.entry(status).or_default() += 1;
            if items.len() >= limit {
                continue;
            }
            let Extra::Ticket { category } = self.acts[*ticket as usize].extra else {
                continue;
            };
            let assignee = self.assignments.get(ticket).and_then(|items| {
                items
                    .iter()
                    .rev()
                    .find(|a| !self.withdrawn(**a))
                    .and_then(|a| match self.acts[*a as usize].extra {
                        Extra::Assignment { assignee } => Some(string(self.strings.text(assignee))),
                        _ => None,
                    })
            });
            let replies = self.children.get(ticket).map_or(0, |items| {
                items.iter().filter(|c| !self.withdrawn(**c)).count()
            });
            let JsonValue::Object(mut fields) = self.act_value(*ticket) else {
                continue;
            };
            fields.push((
                JsonString::from_utf8("category"),
                string(self.strings.text(category)),
            ));
            fields.push((JsonString::from_utf8("status"), string(status)));
            fields.push((
                JsonString::from_utf8("assignee"),
                assignee.unwrap_or(JsonValue::Null),
            ));
            fields.push((JsonString::from_utf8("replies"), int(replies as i128)));
            items.push(JsonValue::Object(fields));
        }
        let order = ["open", "with-agent", "returned", "reviewed", "withdrawn"];
        object(vec![
            (
                "counts",
                object(
                    order
                        .iter()
                        .map(|s| (*s, int(*counts.get(s).unwrap_or(&0) as i128)))
                        .collect(),
                ),
            ),
            ("tickets", array(items)),
        ])
    }

    fn review(&self, limit: usize) -> JsonValue {
        struct Group {
            address: u32,
            category: u32,
            open: Vec<u32>,
            deferred: u64,
            decided: HashMap<u32, u64>,
            targets: HashSet<u32>,
            authors: HashSet<u32>,
            models: HashSet<u32>,
            humans: bool,
            first_seq: u64,
        }
        let mut groups = Vec::new();
        let (mut open_total, mut deferred_total) = (0u64, 0u64);
        for address in &self.group_order {
            let members = &self.group_members[address];
            let mut group = Group {
                address: *address,
                category: 0,
                open: Vec::new(),
                deferred: 0,
                decided: HashMap::new(),
                targets: HashSet::new(),
                authors: HashSet::new(),
                models: HashSet::new(),
                humans: false,
                first_seq: u64::MAX,
            };
            for proposal in members {
                if !self.live(*proposal) {
                    continue;
                }
                let act = &self.acts[*proposal as usize];
                if let Extra::Proposal { category, .. } = act.extra {
                    group.category = category;
                }
                match self.decision(*proposal) {
                    Some((_, decision)) if self.strings.text(decision) == "defer" => {
                        group.deferred += 1
                    }
                    Some((_, decision)) => *group.decided.entry(decision).or_default() += 1,
                    None => {
                        group.open.push(*proposal);
                        group.first_seq = group.first_seq.min(act.seq);
                        group.targets.extend(act.targets.iter().copied());
                        group.authors.insert(act.author);
                        match (act.author_type, act.model) {
                            (AuthorType::Agent, Some(model)) => {
                                group.models.insert(model);
                            }
                            _ => group.humans = true,
                        }
                    }
                }
            }
            open_total += group.open.len() as u64;
            deferred_total += group.deferred;
            if !group.open.is_empty() || group.deferred > 0 {
                groups.push(group);
            }
        }
        // Most open proposals first, then the oldest.
        groups.sort_by(|a, b| {
            b.open
                .len()
                .cmp(&a.open.len())
                .then(a.first_seq.cmp(&b.first_seq))
        });
        let rows = groups
            .iter()
            .take(limit)
            .map(|group| {
                let independent = group.models.len() + usize::from(group.humans);
                object(vec![
                    ("group", string(self.strings.text(group.address))),
                    ("category", string(self.strings.text(group.category))),
                    ("open_count", int(group.open.len() as i128)),
                    ("deferred_count", int(group.deferred as i128)),
                    (
                        "decided_count",
                        int(group.decided.values().sum::<u64>() as i128),
                    ),
                    ("targets_count", int(group.targets.len() as i128)),
                    ("authors_count", int(group.authors.len() as i128)),
                    ("independent_count", int(independent as i128)),
                    (
                        "models",
                        array(
                            group
                                .models
                                .iter()
                                .map(|m| string(self.strings.text(*m)))
                                .collect(),
                        ),
                    ),
                    (
                        "open",
                        array(
                            group
                                .open
                                .iter()
                                .take(32)
                                .map(|p| string(self.ids.text(*p)))
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        object(vec![
            ("open_total", int(open_total as i128)),
            ("deferred_total", int(deferred_total as i128)),
            ("groups_total", int(groups.len() as i128)),
            ("groups", array(rows)),
        ])
    }

    fn lease_view(&self, now_ms: i64, limit: usize) -> JsonValue {
        let mut targets: Vec<u32> = Vec::new();
        let mut seen = HashSet::new();
        for lease in self.leases.iter().rev() {
            let act = &self.acts[*lease as usize];
            if let Extra::Lease { until_ms } = act.extra {
                if until_ms > now_ms {
                    for target in act.targets.iter() {
                        if seen.insert(*target) {
                            targets.push(*target);
                        }
                    }
                }
            }
        }
        let mut rows = Vec::new();
        let mut collisions = 0u64;
        for target in targets {
            let holders = self.live_lease_holders(target, now_ms);
            if holders.is_empty() {
                continue;
            }
            if holders.len() > 1 {
                collisions += 1;
            }
            if rows.len() < limit {
                rows.push(object(vec![
                    ("target", string(self.strings.text(target))),
                    (
                        "holders",
                        array(holders.iter().map(|h| string(h)).collect()),
                    ),
                    ("collision", boolean(holders.len() > 1)),
                ]));
            }
        }
        object(vec![
            ("collisions", int(collisions as i128)),
            ("leases", array(rows)),
        ])
    }

    fn presence(&self, now_ms: i64, window_ms: i64, limit: usize) -> JsonValue {
        let since = now_ms - window_ms.max(0);
        let mut by_target: HashMap<u32, (HashSet<u32>, HashMap<u32, u64>)> = HashMap::new();
        let mut agents = HashSet::new();
        let mut scanned = 0usize;
        for act in self.acts.iter().rev() {
            scanned += 1;
            if scanned > MAX_PRESENCE_SCAN {
                break;
            }
            // Acts arrive in near time order; a long run before the window ends the scan.
            if act.at_ms < since {
                if act.at_ms < since - window_ms.max(60_000) {
                    break;
                }
                continue;
            }
            if act.author_type != AuthorType::Agent || act.at_ms > now_ms {
                continue;
            }
            agents.insert(act.author);
            for target in act.targets.iter() {
                let entry = by_target.entry(*target).or_default();
                entry.0.insert(act.author);
                *entry.1.entry(act.role.unwrap_or(u32::MAX)).or_default() += 1;
            }
        }
        let mut rows: Vec<(u32, usize, HashMap<u32, u64>)> = by_target
            .into_iter()
            .map(|(target, (authors, roles))| (target, authors.len(), roles))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        object(vec![
            ("agents", int(agents.len() as i128)),
            ("targets_count", int(rows.len() as i128)),
            (
                "targets",
                array(
                    rows.iter()
                        .take(limit)
                        .map(|(target, agents, roles)| {
                            let mut roles: Vec<(u32, u64)> =
                                roles.iter().map(|(r, c)| (*r, *c)).collect();
                            roles.sort();
                            object(vec![
                                ("target", string(self.strings.text(*target))),
                                ("agents", int(*agents as i128)),
                                (
                                    "roles",
                                    array(
                                        roles
                                            .iter()
                                            .map(|(role, count)| {
                                                object(vec![
                                                    (
                                                        "role",
                                                        if *role == u32::MAX {
                                                            JsonValue::Null
                                                        } else {
                                                            string(self.strings.text(*role))
                                                        },
                                                    ),
                                                    ("acts", int(*count as i128)),
                                                ])
                                            })
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    fn history(&self, index: u32) -> JsonValue {
        let act = &self.acts[index as usize];
        let ids = |items: Option<&Vec<u32>>| {
            array(
                items
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .map(|i| string(self.ids.text(*i)))
                    .collect(),
            )
        };
        let decision = self
            .decision(index)
            .map_or(JsonValue::Null, |(review, decision)| {
                object(vec![
                    ("review", string(self.ids.text(review))),
                    ("decision", string(self.strings.text(decision))),
                ])
            });
        let group = match act.extra {
            Extra::Proposal { group, .. } => string(self.strings.text(group)),
            _ => JsonValue::Null,
        };
        object(vec![
            ("act", self.act_value(index)),
            (
                "parents",
                array(
                    act.parents
                        .iter()
                        .map(|p| string(self.ids.text(*p)))
                        .collect(),
                ),
            ),
            ("children", ids(self.children.get(&index))),
            (
                "replaces",
                act.replaces
                    .map_or(JsonValue::Null, |r| string(self.ids.text(r))),
            ),
            ("replaced_by", ids(self.replaced_by.get(&index))),
            ("withdrawals", ids(self.withdrawals.get(&index))),
            ("reviews", ids(self.direct_reviews.get(&index))),
            ("group", group),
            ("decision", decision),
        ])
    }

    fn kind_value(&self, name: &str, rule: &KindRule, declared: bool) -> JsonValue {
        let list = |items: &[String]| array(items.iter().map(|i| string(i)).collect());
        object(vec![
            ("name", string(name)),
            ("declared", boolean(declared)),
            ("description", string(&rule.description)),
            (
                "grounds",
                string(if rule.grounds_required {
                    "required"
                } else {
                    "optional"
                }),
            ),
            (
                "parents",
                string(if rule.parents_required {
                    "required"
                } else {
                    "optional"
                }),
            ),
            ("body_required", list(&rule.body_required)),
            ("target_prefix", list(&rule.target_prefix)),
        ])
    }

    fn kinds(&self) -> JsonValue {
        let mut kinds: Vec<JsonValue> = self
            .registry
            .kind_order
            .iter()
            .map(|name| self.kind_value(name, &self.registry.kinds[name], false))
            .collect();
        kinds.extend(
            self.declared_order
                .iter()
                .map(|name| self.kind_value(name, &self.declared[name], true)),
        );
        let list = |items: &[String]| array(items.iter().map(|i| string(i)).collect());
        object(vec![
            ("schema", string(ACT_SCHEMA)),
            ("kinds", array(kinds)),
            ("decisions", list(&self.registry.decisions)),
            (
                "categories",
                array(
                    self.registry
                        .category_order
                        .iter()
                        .map(|name| {
                            let category = &self.registry.categories[name];
                            object(vec![
                                ("name", string(name)),
                                ("variants", list(&category.variants)),
                                ("change_required", list(&category.change_required)),
                                ("group_by", list(&category.group_by)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "invariants",
                array(
                    [
                        "targets: at least one address",
                        "author: type and id; an agent also names its model",
                        "intent: one line",
                        "grounds: required unless the kind declares them optional",
                        "posture: local personal work, never source, review of the Tree or canon",
                        "immutable: a correction replaces, an undo withdraws",
                    ]
                    .iter()
                    .map(|line| string(line))
                    .collect(),
                ),
            ),
        ])
    }
}

#[cfg(feature = "wasm")]
pub mod browser {
    use super::ActJournal;
    use wasm_bindgen::prelude::*;

    /// The act journal's folds in the browser. Bodies stay in the host's store.
    #[wasm_bindgen]
    pub struct ActJournalV1 {
        journal: ActJournal,
    }

    #[wasm_bindgen]
    impl ActJournalV1 {
        #[wasm_bindgen(constructor)]
        pub fn new(registry_json: &[u8]) -> Result<ActJournalV1, JsValue> {
            ActJournal::new(registry_json)
                .map(|journal| Self { journal })
                .map_err(|error| JsValue::from_str(&error))
        }
        pub fn append(&mut self, record: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.journal
                .append(record)
                .map_err(|error| JsValue::from_str(&error))
        }
        #[wasm_bindgen(js_name = appendBatch)]
        pub fn append_batch(&mut self, records: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.journal
                .append_batch(records)
                .map_err(|error| JsValue::from_str(&error))
        }
        pub fn query(&self, request: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.journal
                .query(request)
                .map_err(|error| JsValue::from_str(&error))
        }
        pub fn len(&self) -> u32 {
            self.journal.len() as u32
        }
    }

    #[wasm_bindgen]
    pub fn act_digest_wasm_v1(record: &[u8]) -> Result<String, JsValue> {
        super::act_digest_v1(record).map_err(|error| JsValue::from_str(&error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY: &[u8] = include_bytes!("../../../../access/contracts/act-kinds.v1.json");

    fn journal() -> ActJournal {
        ActJournal::new(REGISTRY).unwrap()
    }
    fn record(id: &str, kind: &str, extra: &str) -> String {
        let base = format!(
            r#""schema":"tos.act.v1","id":"{id}","kind":"{kind}","intent":"test act","at":"2026-11-20T10:00:00.000Z","posture":{{"local_only":true,"source":false,"reviewed_by_tree":false,"canon":false}}"#
        );
        format!("{{{base},{extra}}}")
    }
    const HUMAN: &str = r#""author":{"type":"human","id":"dionysus"}"#;
    fn agent(id: &str, model: &str) -> String {
        format!(r#""author":{{"type":"agent","id":"{id}","model":"{model}","role":"reader"}}"#)
    }
    fn value(bytes: &[u8]) -> JsonValue {
        parse(bytes, 64 * 1_048_576).unwrap()
    }
    fn query(journal: &ActJournal, request: &str) -> JsonValue {
        value(&journal.query(request.as_bytes()).unwrap())
    }
    fn num(value: &JsonValue, key: &str) -> i128 {
        integer(get(value, key).unwrap()).unwrap()
    }

    #[test]
    fn utc_parsing_matches_javascript() {
        assert_eq!(utc_ms("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(utc_ms("2026-11-20T10:00:00.000Z"), Some(1_795_168_800_000));
        assert_eq!(utc_ms("2024-02-29T00:00:00.001Z"), Some(1_709_164_800_001));
        assert_eq!(utc_ms("2023-02-29T00:00:00.000Z"), None);
        assert_eq!(
            utc_ms("-000001-01-01T00:00:00.000Z"),
            Some(-62_198_755_200_000)
        );
        assert_eq!(utc_ms("2026-11-20T10:00:00Z"), None);
    }

    #[test]
    fn a_record_receives_a_stable_digest_and_restores_with_it() {
        let mut j = journal();
        let raw = record(
            "t1",
            "ticket",
            &format!(
                r#"{HUMAN},"targets":["affirmation:link:01:child"],"grounds":[],"body":{{"category":"translation","variant":"outdated","observation":"дитя"}},"parents":[]"#
            ),
        );
        let out = value(&j.append(raw.as_bytes()).unwrap());
        assert_eq!(num(&out, "seq"), 1);
        let stored = get(&out, "record").unwrap();
        let digest = get(stored, "digest")
            .and_then(JsonValue::as_str)
            .unwrap()
            .to_owned();
        assert_eq!(act_digest_v1(raw.as_bytes()).unwrap(), digest);
        // The same record restored into a fresh journal keeps its digest.
        let mut fresh = journal();
        let bytes = emit(stored).unwrap();
        assert!(fresh.append(&bytes).is_ok());
        // A changed body with the old digest is refused.
        let tampered = String::from_utf8(bytes).unwrap().replace("дитя", "ребёнок");
        assert_eq!(
            journal().append(tampered.as_bytes()).unwrap_err(),
            "act_digest_mismatch"
        );
    }

    #[test]
    fn invariants_refuse_records_without_address_author_or_posture() {
        let mut j = journal();
        let cases = [
            (
                record(
                    "a",
                    "note",
                    &format!(
                        r#"{HUMAN},"targets":[],"grounds":[],"body":{{"text":"x"}},"parents":[]"#
                    ),
                ),
                "act_invalid:targets",
            ),
            (
                record(
                    "a",
                    "note",
                    r#""author":{"type":"agent","id":"a1"},"targets":["x"],"grounds":[],"body":{"text":"x"},"parents":[]"#,
                ),
                "act_invalid:author.model",
            ),
            (
                record(
                    "a",
                    "note",
                    &format!(r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{}},"parents":[]"#),
                ),
                "act_invalid:body.text",
            ),
            (
                record(
                    "a",
                    "proposal",
                    &format!(
                        r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{"category":"open","change":{{"a":1}}}},"parents":[]"#
                    ),
                ),
                "act_invalid:grounds_required",
            ),
            (
                record(
                    "a",
                    "note",
                    &format!(
                        r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{"text":"x"}},"parents":["missing"]"#
                    ),
                ),
                "act_missing_parent",
            ),
            (
                record(
                    "a",
                    "made-up",
                    &format!(r#"{HUMAN},"targets":["x"],"grounds":["y"],"body":{{}},"parents":[]"#),
                ),
                "act_invalid:kind",
            ),
            (
                record(
                    "a",
                    "x.agents.sketch",
                    &format!(r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{}},"parents":[]"#),
                ),
                "act_invalid:grounds_required",
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(j.append(raw.as_bytes()).unwrap_err(), expected, "{raw}");
        }
        let wrong_posture = record(
            "a",
            "note",
            &format!(r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{"text":"x"}},"parents":[]"#),
        )
        .replace(r#""canon":false"#, r#""canon":true"#);
        assert_eq!(
            j.append(wrong_posture.as_bytes()).unwrap_err(),
            "act_invalid:posture"
        );
        // An undeclared kind with grounds is kept and counted as unknown.
        let unknown = record(
            "u1",
            "x.agents.sketch",
            &format!(
                r#"{HUMAN},"targets":["x"],"grounds":["y"],"body":{{"anything":[1,2]}},"parents":[]"#
            ),
        );
        assert!(j.append(unknown.as_bytes()).is_ok());
        let summary = query(&j, r#"{"view":"summary"}"#);
        assert_eq!(
            get(
                summary
                    .object_get("unknown_kinds")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .first()
                    .unwrap(),
                "kind"
            )
            .and_then(JsonValue::as_str),
            Some("x.agents.sketch")
        );
    }

    #[test]
    fn one_group_review_decides_every_same_translation_change() {
        let mut j = journal();
        for n in 0..13 {
            let raw = record(
                &format!("p{n}"),
                "proposal",
                &format!(
                    r#"{},"targets":["link-{n}"],"grounds":["tos.bonsai.p2.place-{n}"],"body":{{"category":"translation","variant":"outdated","change":{{"field":"verb","from":"дитя","to":"ребёнок"}}}},"parents":[]"#,
                    agent(
                        &format!("reader-{}", n % 4),
                        if n % 2 == 0 { "model-a" } else { "model-b" }
                    )
                ),
            );
            j.append(raw.as_bytes()).unwrap();
        }
        // A different change forms its own group.
        let other = record(
            "q",
            "proposal",
            &format!(
                r#"{HUMAN},"targets":["link-x"],"grounds":["g"],"body":{{"category":"translation","change":{{"from":"дитя","to":"дитятко"}}}},"parents":[]"#
            ),
        );
        j.append(other.as_bytes()).unwrap();
        let review = query(&j, r#"{"view":"review"}"#);
        assert_eq!(num(&review, "open_total"), 14);
        let first = &get(&review, "groups").unwrap().as_array().unwrap()[0];
        assert_eq!(num(first, "open_count"), 13);
        assert_eq!(num(first, "independent_count"), 2);
        let group = get(first, "group")
            .and_then(JsonValue::as_str)
            .unwrap()
            .to_owned();
        assert!(group.starts_with("group:translation:"));
        let decide = record(
            "r1",
            "review",
            &format!(
                r#"{HUMAN},"targets":["{group}"],"grounds":[],"body":{{"decision":"accept","reason":"the Tree's word is ребёнок"}},"parents":[]"#
            ),
        );
        j.append(decide.as_bytes()).unwrap();
        let review = query(&j, r#"{"view":"review"}"#);
        assert_eq!(num(&review, "open_total"), 1);
        // A proposal made after the group decision stays open.
        let late = record(
            "p13",
            "proposal",
            &format!(
                r#"{HUMAN},"targets":["link-13"],"grounds":["g"],"body":{{"category":"translation","change":{{"from":"дитя","to":"ребёнок"}}}},"parents":[]"#
            ),
        );
        j.append(late.as_bytes()).unwrap();
        assert_eq!(num(&query(&j, r#"{"view":"review"}"#), "open_total"), 2);
        // Withdrawing the decision reopens the thirteen; withdrawing the withdrawal restores it.
        j.append(
            record(
                "w1",
                "withdrawal",
                &format!(r#"{HUMAN},"targets":["act:r1"],"grounds":[],"body":{{}},"parents":[]"#),
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(num(&query(&j, r#"{"view":"review"}"#), "open_total"), 15);
        j.append(
            record(
                "w2",
                "withdrawal",
                &format!(r#"{HUMAN},"targets":["act:w1"],"grounds":[],"body":{{}},"parents":[]"#),
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(num(&query(&j, r#"{"view":"review"}"#), "open_total"), 2);
        let history = query(&j, r#"{"view":"history","id":"p0"}"#);
        assert_eq!(
            get(get(&history, "decision").unwrap(), "decision").and_then(JsonValue::as_str),
            Some("accept")
        );
    }

    #[test]
    fn a_ticket_moves_through_the_board() {
        let mut j = journal();
        let status = |j: &ActJournal| {
            let board = query(j, r#"{"view":"board"}"#);
            get(
                &get(&board, "tickets").unwrap().as_array().unwrap()[0],
                "status",
            )
            .and_then(JsonValue::as_str)
            .unwrap()
            .to_owned()
        };
        j.append(record("t1", "ticket", &format!(r#"{HUMAN},"targets":["child"],"grounds":[],"body":{{"category":"translation","observation":"устарело"}},"parents":[]"#)).as_bytes()).unwrap();
        assert_eq!(status(&j), "open");
        j.append(record("a1", "assignment", &format!(r#"{HUMAN},"targets":["act:t1"],"grounds":[],"body":{{"assignee":"squad:translators"}},"parents":[]"#)).as_bytes()).unwrap();
        assert_eq!(status(&j), "with-agent");
        j.append(record("p1", "proposal", &format!(r#"{},"targets":["child"],"grounds":["place"],"body":{{"category":"translation","change":{{"from":"дитя","to":"ребёнок"}}}},"parents":["t1"]"#, agent("translator-1", "model-a"))).as_bytes()).unwrap();
        assert_eq!(status(&j), "returned");
        j.append(record("r1", "review", &format!(r#"{HUMAN},"targets":["act:p1"],"grounds":[],"body":{{"decision":"accept","reason":"yes"}},"parents":[]"#)).as_bytes()).unwrap();
        assert_eq!(status(&j), "reviewed");
        // A review can only name proposals or groups.
        let wrong = record(
            "r2",
            "review",
            &format!(
                r#"{HUMAN},"targets":["act:t1"],"grounds":[],"body":{{"decision":"accept","reason":"x"}},"parents":[]"#
            ),
        );
        assert_eq!(
            j.append(wrong.as_bytes()).unwrap_err(),
            "act_invalid:review_target"
        );
    }

    #[test]
    fn leases_report_collisions_and_end() {
        let mut j = journal();
        let lease = |id: &str, who: &str, at: &str| {
            record(id, "lease", &format!(r#"{},"targets":["chapter:III.02"],"grounds":[],"body":{{"ttl_ms":60000}},"parents":[]"#, agent(who, "model-a")))
                .replace("2026-11-20T10:00:00.000Z", at)
        };
        let first = value(
            &j.append(lease("l1", "a1", "2026-11-20T10:00:00.000Z").as_bytes())
                .unwrap(),
        );
        assert!(
            get(&first, "collisions")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        let second = value(
            &j.append(lease("l2", "a2", "2026-11-20T10:00:30.000Z").as_bytes())
                .unwrap(),
        );
        assert_eq!(
            get(&second, "collisions")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let at = utc_ms("2026-11-20T10:00:40.000Z").unwrap();
        let leases = query(&j, &format!(r#"{{"view":"leases","now_ms":{at}}}"#));
        assert_eq!(num(&leases, "collisions"), 1);
        j.append(
            record(
                "x1",
                "release",
                &format!(
                    r#"{},"targets":["chapter:III.02"],"grounds":[],"body":{{}},"parents":[]"#,
                    agent("a1", "model-a")
                ),
            )
            .replace("2026-11-20T10:00:00.000Z", "2026-11-20T10:00:41.000Z")
            .as_bytes(),
        )
        .unwrap();
        let leases = query(
            &j,
            &format!(r#"{{"view":"leases","now_ms":{}}}"#, at + 2_000),
        );
        assert_eq!(num(&leases, "collisions"), 0);
        let later = query(
            &j,
            &format!(r#"{{"view":"leases","now_ms":{}}}"#, at + 120_000),
        );
        assert!(
            get(&later, "leases")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn an_agent_declares_a_kind_and_uses_it() {
        let mut j = journal();
        let declare = record(
            "k1",
            "kind-declaration",
            &format!(
                r#"{},"targets":["kind:x.cartographer.constellation"],"grounds":[],"body":{{"declares":{{"name":"x.cartographer.constellation","grounds":"optional","parents":"optional","body_required":["stars"],"description":"A named group of stars."}}}},"parents":[]"#,
                agent("c1", "model-a")
            ),
        );
        j.append(declare.as_bytes()).unwrap();
        let missing = record(
            "c1",
            "x.cartographer.constellation",
            &format!(r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{}},"parents":[]"#),
        );
        assert_eq!(
            j.append(missing.as_bytes()).unwrap_err(),
            "act_invalid:body.stars"
        );
        let ok = record(
            "c2",
            "x.cartographer.constellation",
            &format!(
                r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{"stars":["a","b"]}},"parents":[]"#
            ),
        );
        assert!(j.append(ok.as_bytes()).is_ok());
        let kinds = query(&j, r#"{"view":"kinds"}"#);
        let names: Vec<&str> = get(&kinds, "kinds")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|k| get(k, "name").and_then(JsonValue::as_str))
            .collect();
        assert!(names.contains(&"x.cartographer.constellation"));
        // A second declaration of the same name is refused.
        let again = declare.replace(r#""id":"k1""#, r#""id":"k2""#);
        assert_eq!(
            j.append(again.as_bytes()).unwrap_err(),
            "act_kind_already_declared"
        );
    }

    #[test]
    fn presence_counts_agents_by_target_and_role() {
        let mut j = journal();
        for n in 0..20 {
            let raw = record(
                &format!("n{n}"),
                "note",
                &format!(
                    r#"{},"targets":["{}"],"grounds":[],"body":{{"text":"reading"}},"parents":[]"#,
                    agent(&format!("a{}", n % 5), "model-a"),
                    if n % 2 == 0 { "III.02" } else { "III.13" }
                ),
            );
            j.append(raw.as_bytes()).unwrap();
        }
        let now = utc_ms("2026-11-20T10:00:10.000Z").unwrap();
        let presence = query(
            &j,
            &format!(r#"{{"view":"presence","now_ms":{now},"window_ms":60000}}"#),
        );
        assert_eq!(num(&presence, "agents"), 5);
        assert_eq!(num(&presence, "targets_count"), 2);
    }

    #[test]
    fn a_batch_stops_at_the_first_refusal() {
        let mut j = journal();
        let good = record(
            "b1",
            "note",
            &format!(r#"{HUMAN},"targets":["x"],"grounds":[],"body":{{"text":"a"}},"parents":[]"#),
        );
        let bad = record(
            "b2",
            "note",
            &format!(r#"{HUMAN},"targets":[],"grounds":[],"body":{{"text":"a"}},"parents":[]"#),
        );
        let out = value(
            &j.append_batch(format!("[{good},{bad},{good}]").as_bytes())
                .unwrap(),
        );
        assert_eq!(get(&out, "appended").unwrap().as_array().unwrap().len(), 1);
        assert_eq!(num(get(&out, "failure").unwrap(), "index"), 1);
        assert_eq!(j.len(), 1);
    }
}
