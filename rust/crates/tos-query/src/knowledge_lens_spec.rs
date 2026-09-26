//! Normalization and exact native LensSpec predicates. Registry semantics are
//! supplied by the selected authored vocabulary, never by request metadata.
use crate::{
    search_v2::{SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, text},
};
use std::{cmp::Ordering, collections::BTreeMap};
use tos_foundation::{Digest256Hasher, JsonNumber, JsonNumberKind, JsonValue};

#[derive(Clone, Debug)]
pub struct LensVocabulary {
    pub sources: Vec<String>,
    pub query_properties: Vec<JsonValue>,
    pub carrier_source_priority: BTreeMap<String, u64>,
    pub overview_excluded_predicates: Vec<String>,
    pub overview_excluded_relation_types: Vec<String>,
    pub shared_entity_grammars: Vec<regex::Regex>,
}
impl LensVocabulary {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_selected(
        bound: &crate::knowledge_binding::BoundCmpKnowledge<'_>,
        header: &JsonValue,
    ) -> Result<Self, SearchV2Error> {
        Self::from_descriptor(bound.descriptor(), header)
    }
    pub fn from_descriptor(d: &JsonValue, header: &JsonValue) -> Result<Self, SearchV2Error> {
        let sources = array(get(d, "sources"));
        let mut names = vec![];
        let mut priorities = BTreeMap::new();
        for source in sources {
            let id = string(get(source, "source_graph_id"));
            if id.is_empty() {
                return Err(corrupt("invalid selected source registration"));
            }
            names.push(id.to_owned());
            priorities.insert(
                id.to_owned(),
                uint(get(source, "representative_priority")) as u64,
            );
        }
        let grammar = array(field(d, "identity.shared_entity_id_grammars"));
        if grammar.is_empty() || grammar.len() > 32 {
            return Err(corrupt("invalid identity grammar count"));
        }
        let mut compiled = vec![];
        for raw in grammar {
            let s = raw
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 4096)
                .ok_or_else(|| corrupt("invalid identity grammar"))?;
            let re = regex::RegexBuilder::new(&format!("\\A(?:{s})\\z"))
                .size_limit(1_048_576)
                .dfa_size_limit(1_048_576)
                .build()
                .map_err(|_| corrupt("unsupported selected identity grammar"))?;
            compiled.push(re);
        }
        Ok(Self {
            sources: names,
            query_properties: array(get(header, "query_properties")).to_vec(),
            carrier_source_priority: priorities,
            overview_excluded_predicates: array(field(d, "overview.excluded_predicate_ids"))
                .iter()
                .map(|v| string(v).to_owned())
                .collect(),
            overview_excluded_relation_types: array(field(
                d,
                "overview.excluded_relation_type_ids",
            ))
            .iter()
            .map(|v| string(v).to_owned())
            .collect(),
            shared_entity_grammars: compiled,
        })
    }
    pub fn declared_entity(&self, id: &str) -> bool {
        id.len() <= 4096 && self.shared_entity_grammars.iter().any(|g| g.is_match(id))
    }
    pub(crate) fn priority(&self, node: &JsonValue) -> u64 {
        *self
            .carrier_source_priority
            .get(string(get(node, "source_graph")))
            .unwrap_or(&99)
    }
}
pub(crate) fn invalid(message: &'static str) -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::InvalidRequest,
        message,
    }
}
pub(crate) fn budget() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "lens execution budget exceeded",
    }
}
pub(crate) fn corrupt(message: &'static str) -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::CorruptSelectedCarrier,
        message,
    }
}
pub(crate) fn get<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue {
    v.object_get(key).unwrap_or(&JsonValue::Null)
}
pub(crate) fn string(v: &JsonValue) -> &str {
    v.as_str().unwrap_or("")
}
pub(crate) fn array(v: &JsonValue) -> &[JsonValue] {
    v.as_array().unwrap_or(&[])
}
pub(crate) fn number(v: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: v.to_string(),
    })
}
pub(crate) fn uint(v: &JsonValue) -> usize {
    if let JsonValue::Number(n) = v {
        n.lexeme.parse().unwrap_or(0)
    } else {
        0
    }
}
pub(crate) fn boolean(v: &JsonValue) -> bool {
    matches!(v, JsonValue::Bool(true))
}
pub(crate) fn set(v: &mut JsonValue, key: &str, value: JsonValue) {
    if let JsonValue::Object(o) = v {
        if let Some((_, old)) = o.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
            *old = value;
        } else {
            o.push((tos_foundation::JsonString::from_utf8(key), value));
        }
    }
}
pub(crate) fn remove(v: &mut JsonValue, key: &str) {
    if let JsonValue::Object(o) = v {
        o.retain(|(k, _)| k.as_str() != Some(key));
    }
}
pub(crate) fn field<'a>(v: &'a JsonValue, path: &str) -> &'a JsonValue {
    path.split('.').fold(v, |v, k| get(v, k))
}
pub(crate) fn lower(s: &str) -> String {
    tos_foundation::python_lower_unicode16_v1(
        s,
        s.chars().count(),
        s.chars().count().saturating_mul(3),
        s.len().saturating_mul(3),
    )
    .expect("admitted Unicode lower bounds")
}
pub(crate) fn strip(s: &str) -> &str {
    tos_foundation::python_strip_unicode16_v1(s, s.chars().count())
        .expect("admitted Unicode strip bounds")
}
fn trimmed(v: &JsonValue) -> Option<&str> {
    v.as_str().map(strip).filter(|s| !s.is_empty())
}
fn strict<'a>(
    v: &'a JsonValue,
    allowed: &[&str],
) -> Result<&'a [(tos_foundation::JsonString, JsonValue)], SearchV2Error> {
    if matches!(v, JsonValue::Null) {
        return Ok(&[]);
    }
    let o = v
        .as_object()
        .ok_or_else(|| invalid("lens member must be an object"))?;
    if o.iter()
        .any(|(k, _)| !allowed.contains(&k.as_str().unwrap_or("")))
    {
        return Err(invalid("unknown lens member"));
    }
    Ok(o)
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
}
pub(crate) fn language(s: &str) -> bool {
    if s.len() > 128 {
        return false;
    }
    let p: Vec<_> = s.split('-').collect();
    let first = p[0];
    if first.len() == 1 && matches!(first, "i" | "I" | "x" | "X") {
        return p.len() > 1
            && p[1..].iter().all(|p| {
                !p.is_empty() && p.len() <= 8 && p.bytes().all(|c| c.is_ascii_alphanumeric())
            });
    }
    (2..=8).contains(&first.len())
        && first.bytes().all(|c| c.is_ascii_alphabetic())
        && p[1..]
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 8 && p.bytes().all(|c| c.is_ascii_alphanumeric()))
}
fn form_key(s: &str) -> bool {
    matches!(s, "default" | "original") || language(s)
}
const SHARED_FIELDS: &[&str] = &[
    "id",
    "native_id",
    "source_graph",
    "epistemic.authority_layer",
    "epistemic.canon_status",
    "epistemic.review_posture",
    "epistemic.confidence",
    "graph_layers",
    "view_ids",
    "source_refs",
];
const NODE_FIELDS: &[&str] = &[
    "entity_id",
    "source_dossier_ref",
    "kind_id",
    "type_id",
    "type_mapping.status",
    "type_mapping.source_kind_id",
    "display.summary_state",
];
const RELATION_FIELDS: &[&str] = &[
    "from_id",
    "to_id",
    "predicate_id",
    "relation_type_id",
    "predicate_mapping.status",
    "predicate_mapping.source_predicate_id",
    "display.explanation_state",
];
pub(crate) fn allowed_field(s: &str, node: bool) -> bool {
    if SHARED_FIELDS.contains(&s) || (if node { NODE_FIELDS } else { RELATION_FIELDS }).contains(&s)
    {
        return true;
    }
    let parts: Vec<_> = s.split('.').collect();
    if parts.len() == 3
        && parts[0] == "display"
        && (if node {
            &["title", "kind_label", "summary"][..]
        } else {
            &["label", "inverse_label", "statement", "explanation"][..]
        })
        .contains(&parts[1])
        && form_key(parts[2])
    {
        return true;
    }
    let Some(body) = s
        .strip_prefix("attributes.")
        .or_else(|| s.strip_prefix("semantics."))
    else {
        return false;
    };
    !body.is_empty()
        && body.len() <= 128
        && body.as_bytes()[0].is_ascii_alphanumeric()
        && body
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        && !parts
            .iter()
            .any(|s| matches!(*s, "__proto__" | "prototype" | "constructor"))
}
fn bounded(
    v: &JsonValue,
    default: usize,
    min: usize,
    max: usize,
    strict_int: bool,
) -> Result<JsonValue, SearchV2Error> {
    if matches!(v, JsonValue::Null) {
        return Ok(number(default));
    }
    let n = match v {
        JsonValue::Number(n) if !strict_int || n.kind == JsonNumberKind::Int => n
            .lexeme
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| n.trunc()),
        JsonValue::String(_) if !strict_int => v
            .as_str()
            .and_then(|s| {
                let s = strip(s);
                let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
                if digits.is_empty()
                    || !digits.as_bytes()[0].is_ascii_digit()
                    || !digits.as_bytes()[digits.len() - 1].is_ascii_digit()
                    || digits.as_bytes().windows(2).any(|p| p == b"__")
                    || !digits.bytes().all(|c| c.is_ascii_digit() || c == b'_')
                {
                    return None;
                }
                s.replace('_', "").parse::<i64>().ok()
            })
            .map(|n| n as f64),
        _ => None,
    }
    .ok_or_else(|| invalid("invalid lens integer"))?;
    if n < (min as f64) || n > (max as f64) {
        return Err(invalid("lens integer outside bounds"));
    }
    Ok(number(n as usize))
}
fn strings(v: &JsonValue, max: usize, unique: bool) -> Result<JsonValue, SearchV2Error> {
    if matches!(v, JsonValue::Null) {
        return Ok(JsonValue::Array(vec![]));
    }
    let a = v
        .as_array()
        .filter(|a| a.len() <= max)
        .ok_or_else(|| invalid("invalid lens string array"))?;
    let mut out = vec![];
    for item in a {
        let s = item
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("invalid lens string array member"))?;
        if unique && out.iter().any(|v| v == &text(s)) {
            return Err(invalid("duplicate lens string array member"));
        }
        out.push(text(s));
    }
    Ok(JsonValue::Array(out))
}
fn choice(v: &JsonValue, default: &str, values: &[&str]) -> Result<JsonValue, SearchV2Error> {
    let s = trimmed(v).unwrap_or(default);
    if !values.contains(&s) {
        return Err(invalid("invalid lens enum"));
    }
    Ok(text(s))
}
fn localized(v: &JsonValue, fallback: &str) -> Result<JsonValue, SearchV2Error> {
    let mut out = object(vec![
        ("default", text(fallback)),
        ("ru", JsonValue::Null),
        ("en", JsonValue::Null),
        ("original", JsonValue::Null),
    ]);
    if matches!(v, JsonValue::Null) {
        return Ok(out);
    }
    if let Some(s) = v.as_str() {
        let s = strip(s);
        if s.is_empty() {
            return Err(invalid("empty lens localized text"));
        }
        set(&mut out, "default", text(s));
        return Ok(out);
    }
    let o = v
        .as_object()
        .ok_or_else(|| invalid("invalid lens localized text"))?;
    for (k, val) in o {
        let k = k.as_str().ok_or_else(|| invalid("invalid localized key"))?;
        if !form_key(k)
            || (!matches!(val, JsonValue::Null) && val.as_str().is_none())
            || (k == "default" && trimmed(val).is_none())
        {
            return Err(invalid("invalid localized value"));
        }
        set(
            &mut out,
            k,
            trimmed(val).map(text).unwrap_or(JsonValue::Null),
        );
    }
    // _localized_from uses the first available compatibility text as fallback.
    if get(v, "default").as_str().is_none() {
        let chosen = ["ru", "en", "original"]
            .iter()
            .find_map(|k| trimmed(get(v, k)))
            .or_else(|| {
                let mut forms: Vec<_> = o.iter().collect();
                forms.sort_by(|(a, _), (b, _)| a.as_str().cmp(&b.as_str()));
                forms.into_iter().find_map(|(_, v)| trimmed(v))
            });
        if let Some(s) = chosen {
            set(&mut out, "default", text(s));
        }
    }
    Ok(out)
}
fn group(v: &JsonValue, node: bool) -> Result<JsonValue, SearchV2Error> {
    strict(v, &["enabled", "match", "filters"])?;
    if v.object_get("match").is_some() && !matches!(string(get(v, "match")), "all" | "any") {
        return Err(invalid("invalid filter match"));
    }
    let enabled = get(v, "enabled");
    if v.object_get("enabled").is_some() && !matches!(enabled, JsonValue::Bool(_)) {
        return Err(invalid("query enabled must be boolean"));
    }
    let filters = get(v, "filters");
    let filters = if v.object_get("filters").is_none() {
        &[][..]
    } else {
        filters
            .as_array()
            .filter(|a| a.len() <= 32)
            .ok_or_else(|| invalid("invalid query filter array"))?
    };
    let mut out = vec![];
    for f in filters {
        strict(f, &["field", "property_id", "op", "value"])?;
        let prop = f.object_get("property_id");
        let raw_field = f.object_get("field");
        if prop.is_some() == raw_field.is_some() {
            return Err(invalid("filter requires one selector"));
        }
        let key = if prop.is_some() {
            "property_id"
        } else {
            "field"
        };
        let selected = trimmed(get(f, key)).ok_or_else(|| invalid("empty filter selector"))?;
        if key == "property_id" && get(f, key).as_str() != Some(selected) {
            return Err(invalid("invalid registered property selector"));
        }
        if key == "property_id" {
            let body = selected.strip_prefix("tos.property.").unwrap_or("");
            if !node
                || body.is_empty()
                || !body
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            {
                return Err(invalid("invalid registered property selector"));
            }
        } else if !allowed_field(selected, node) {
            return Err(invalid("invalid filter field"));
        }
        let op = choice(
            get(f, "op"),
            "",
            &[
                "eq", "neq", "in", "contains", "prefix", "exists", "gt", "gte", "lt", "lte",
            ],
        )?;
        let value = f
            .object_get("value")
            .ok_or_else(|| invalid("missing filter value"))?;
        let values = if let JsonValue::Array(a) = value {
            if a.len() > 100 {
                return Err(invalid("too many filter values"));
            }
            a.as_slice()
        } else {
            std::slice::from_ref(value)
        };
        if values.iter().any(|v|matches!(v,JsonValue::Object(_)|JsonValue::Array(_))||v.as_str().is_some_and(|s|s.chars().count()>1024)||matches!(v,JsonValue::Number(n) if n.lexeme.parse::<f64>().ok().is_none_or(|n|!n.is_finite()))){return Err(invalid("invalid filter scalar"))}
        if (matches!(string(&op), "eq" | "neq") && matches!(value, JsonValue::Array(_)))
            || (string(&op) == "exists" && !matches!(value, JsonValue::Bool(_)))
            || (string(&op) == "prefix" && value.as_str().is_none())
            || (matches!(string(&op), "gt" | "gte" | "lt" | "lte")
                && !matches!(value, JsonValue::Number(_)))
        {
            return Err(invalid("filter operator value mismatch"));
        }
        out.push(object(vec![
            (key, text(selected)),
            ("op", op),
            ("value", value.clone()),
        ]));
    }
    Ok(object(vec![
        (
            "enabled",
            if matches!(enabled, JsonValue::Null) {
                JsonValue::Bool(true)
            } else {
                enabled.clone()
            },
        ),
        ("match", choice(get(v, "match"), "all", &["all", "any"])?),
        ("filters", JsonValue::Array(out)),
    ]))
}
fn sorts(v: &JsonValue, node: bool) -> Result<JsonValue, SearchV2Error> {
    let default = || {
        JsonValue::Array(vec![object(vec![
            ("field", text("id")),
            ("direction", text("asc")),
        ])])
    };
    if matches!(v, JsonValue::Null) {
        return Ok(default());
    }
    let a = v
        .as_array()
        .filter(|a| a.len() <= 8)
        .ok_or_else(|| invalid("invalid lens sorts"))?;
    if a.is_empty() {
        return Ok(default());
    }
    let mut out = vec![];
    for r in a {
        strict(r, &["field", "direction"])?;
        let f = trimmed(get(r, "field")).ok_or_else(|| invalid("empty sort field"))?;
        if !allowed_field(f, node) {
            return Err(invalid("invalid sort field"));
        }
        out.push(object(vec![
            ("field", text(f)),
            (
                "direction",
                choice(get(r, "direction"), "asc", &["asc", "desc"])?,
            ),
        ]));
    }
    Ok(JsonValue::Array(out))
}
pub fn normalize_lens_spec(
    v: &JsonValue,
    vocabulary: &LensVocabulary,
) -> Result<JsonValue, SearchV2Error> {
    if v.as_object().is_none() {
        return Err(invalid("lens spec must be an object"));
    }
    strict(
        v,
        &[
            "schema_version",
            "lens_id",
            "title",
            "description",
            "language",
            "sources",
            "seed",
            "node_query",
            "relation_query",
            "traversal",
            "composition",
            "presentation",
            "limits",
            "detail",
            "path_query",
            "explain",
            "pagination",
        ],
    )?;
    if string(get(v, "schema_version")) != "tos_lens_spec_v1" {
        return Err(invalid("invalid lens schema"));
    }
    let id = trimmed(get(v, "lens_id"))
        .filter(|s| identifier(s))
        .ok_or_else(|| invalid("invalid lens id"))?;
    let sources = if v.object_get("sources").is_none() {
        JsonValue::Array(vocabulary.sources.iter().map(|s| text(s)).collect())
    } else {
        strings(get(v, "sources"), vocabulary.sources.len(), true)?
    };
    if array(&sources).is_empty()
        || array(&sources).iter().any(|s| {
            !vocabulary
                .sources
                .iter()
                .any(|registered| registered == string(s))
        })
    {
        return Err(invalid("unknown lens source"));
    }
    let seed = get(v, "seed");
    strict(seed, &["focus_node_id", "node_ids", "text_query"])?;
    let focus = get(seed, "focus_node_id");
    let focus = if matches!(focus, JsonValue::Null) {
        JsonValue::Null
    } else {
        let s = trimmed(focus)
            .filter(|s| s.chars().count() <= 1024)
            .ok_or_else(|| invalid("invalid lens focus"))?;
        text(s)
    };
    let q = trimmed(get(seed, "text_query")).unwrap_or("");
    if q.chars().count() > 256 {
        return Err(invalid("lens text query too long"));
    }
    let traversal = get(v, "traversal");
    strict(
        traversal,
        &["depth", "direction", "predicate_ids", "profile"],
    )?;
    let composition = get(v, "composition");
    strict(
        composition,
        &[
            "endpoint_policy",
            "group_by",
            "sort_nodes",
            "sort_relations",
        ],
    )?;
    let by = strings(get(composition, "group_by"), 4, true)?;
    if array(&by)
        .iter()
        .any(|f| !allowed_field(string(f), true) && !allowed_field(string(f), false))
    {
        return Err(invalid("invalid group field"));
    }
    let presentation = get(v, "presentation");
    strict(
        presentation,
        &[
            "layout",
            "color_by",
            "lane_by",
            "size_by",
            "inspector_fields",
        ],
    )?;
    let mut p = object(vec![(
        "layout",
        choice(
            get(presentation, "layout"),
            "auto",
            &[
                "auto",
                "organic",
                "timeline",
                "flow",
                "evidence",
                "semantic",
                "infrastructure",
                "hierarchical",
                "radial",
                "matrix",
            ],
        )?,
    )]);
    for key in ["color_by", "lane_by", "size_by"] {
        let f = trimmed(get(presentation, key));
        if f.is_some_and(|f| !allowed_field(f, true) && !allowed_field(f, false)) {
            return Err(invalid("invalid presentation field"));
        }
        set(&mut p, key, f.map(text).unwrap_or(JsonValue::Null));
    }
    let mut inspectors = strings(get(presentation, "inspector_fields"), 32, true)?;
    if array(&inspectors).iter().any(|f| {
        !matches!(
            string(f),
            "display" | "display.summary" | "epistemic" | "source_refs" | "attributes"
        ) && !allowed_field(string(f), true)
            && !allowed_field(string(f), false)
    }) {
        return Err(invalid("invalid inspector field"));
    }
    if array(&inspectors).is_empty() {
        inspectors = JsonValue::Array(["display", "epistemic", "source_refs"].map(text).to_vec())
    }
    set(&mut p, "inspector_fields", inspectors);
    let limits = get(v, "limits");
    strict(limits, &["nodes", "relations", "groups"])?;
    let lang = trimmed(get(v, "language")).unwrap_or("auto");
    if !matches!(lang, "auto" | "original") && !language(lang) {
        return Err(invalid("invalid lens language"));
    }
    if v.object_get("detail").is_some() && !matches!(string(get(v, "detail")), "full" | "compact") {
        return Err(invalid("invalid lens detail"));
    }
    if traversal.object_get("profile").is_some()
        && !matches!(string(get(traversal, "profile")), "all" | "overview")
    {
        return Err(invalid("invalid traversal profile"));
    }
    let explain = get(v, "explain");
    if v.object_get("explain").is_some() && !matches!(explain, JsonValue::Bool(_)) {
        return Err(invalid("invalid lens explain"));
    }
    let paths = get(v, "path_query");
    let paths = if matches!(paths, JsonValue::Null) {
        &[][..]
    } else {
        paths
            .as_array()
            .filter(|a| a.len() <= 4)
            .ok_or_else(|| invalid("invalid lens paths"))?
    };
    let mut normalized_paths = vec![];
    for path in paths {
        strict(path, &["path_id", "quantifier", "steps"])?;
        let id = trimmed(get(path, "path_id"))
            .filter(|s| identifier(s))
            .ok_or_else(|| invalid("invalid path id"))?;
        if normalized_paths
            .iter()
            .any(|p| get(p, "path_id") == &text(id))
        {
            return Err(invalid("duplicate path id"));
        }
        let steps = get(path, "steps")
            .as_array()
            .filter(|a| (1..=4).contains(&a.len()))
            .ok_or_else(|| invalid("invalid path steps"))?;
        let mut steps_out = vec![];
        if path.object_get("quantifier").is_some()
            && !matches!(string(get(path, "quantifier")), "exists" | "not_exists")
        {
            return Err(invalid("invalid path quantifier"));
        }
        for step in steps {
            strict(step, &["direction", "node_query", "relation_query"])?;
            if step.object_get("direction").is_some()
                && !matches!(
                    string(get(step, "direction")),
                    "incoming" | "outgoing" | "either"
                )
            {
                return Err(invalid("invalid path direction"));
            }
            steps_out.push(object(vec![
                (
                    "direction",
                    choice(
                        get(step, "direction"),
                        "outgoing",
                        &["incoming", "outgoing", "either"],
                    )?,
                ),
                ("node_query", group(get(step, "node_query"), true)?),
                ("relation_query", group(get(step, "relation_query"), false)?),
            ]));
        }
        normalized_paths.push(object(vec![
            ("path_id", text(id)),
            (
                "quantifier",
                choice(get(path, "quantifier"), "exists", &["exists", "not_exists"])?,
            ),
            ("steps", JsonValue::Array(steps_out)),
        ]));
    }
    let pagination = get(v, "pagination");
    let pagination = if matches!(pagination, JsonValue::Null) {
        JsonValue::Null
    } else {
        strict(pagination, &["nodes", "relations", "cursor"])?;
        for k in ["nodes", "relations"] {
            if pagination.object_get(k).is_some()
                && !matches!(get(pagination,k),JsonValue::Number(n) if n.kind==JsonNumberKind::Int)
            {
                return Err(invalid("invalid page size"));
            }
        }
        let cursor = get(pagination, "cursor");
        if !matches!(cursor, JsonValue::Null)
            && cursor.as_str().is_none_or(|s| {
                s.is_empty()
                    || s.len() > 512
                    || !s
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            })
        {
            return Err(invalid("invalid lens cursor"));
        }
        object(vec![
            (
                "nodes",
                bounded(get(pagination, "nodes"), 40, 1, 100, true)?,
            ),
            (
                "relations",
                bounded(get(pagination, "relations"), 80, 1, 100, true)?,
            ),
            ("cursor", cursor.clone()),
        ])
    };
    let mut title = String::new();
    let mut previous_space = false;
    for c in id.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !previous_space {
                title.push(' ')
            }
            previous_space = true;
        } else {
            title.push(c);
            previous_space = false;
        }
    }
    let title = strip(&title).to_owned();
    Ok(object(vec![
        ("schema_version", text("tos_lens_spec_v1")),
        ("lens_id", text(id)),
        ("title", localized(get(v, "title"), &title)?),
        (
            "description",
            localized(
                get(v, "description"),
                &format!("Declarative knowledge lens {id}."),
            )?,
        ),
        ("language", text(lang)),
        (
            "detail",
            choice(get(v, "detail"), "full", &["full", "compact"])?,
        ),
        ("explain", JsonValue::Bool(boolean(explain))),
        ("pagination", pagination),
        ("path_query", JsonValue::Array(normalized_paths)),
        ("sources", sources),
        (
            "seed",
            object(vec![
                ("focus_node_id", focus),
                ("node_ids", strings(get(seed, "node_ids"), 100, false)?),
                ("text_query", text(q)),
            ]),
        ),
        ("node_query", group(get(v, "node_query"), true)?),
        ("relation_query", group(get(v, "relation_query"), false)?),
        (
            "traversal",
            object(vec![
                ("depth", bounded(get(traversal, "depth"), 0, 0, 5, false)?),
                (
                    "direction",
                    choice(
                        get(traversal, "direction"),
                        "either",
                        &["incoming", "outgoing", "either"],
                    )?,
                ),
                (
                    "predicate_ids",
                    strings(get(traversal, "predicate_ids"), 100, true)?,
                ),
                (
                    "profile",
                    choice(get(traversal, "profile"), "all", &["all", "overview"])?,
                ),
            ]),
        ),
        (
            "composition",
            object(vec![
                (
                    "endpoint_policy",
                    choice(
                        get(composition, "endpoint_policy"),
                        "both",
                        &["both", "either", "independent"],
                    )?,
                ),
                ("group_by", by),
                ("sort_nodes", sorts(get(composition, "sort_nodes"), true)?),
                (
                    "sort_relations",
                    sorts(get(composition, "sort_relations"), false)?,
                ),
            ]),
        ),
        ("presentation", p),
        (
            "limits",
            object(vec![
                ("nodes", bounded(get(limits, "nodes"), 200, 1, 1000, false)?),
                (
                    "relations",
                    bounded(get(limits, "relations"), 400, 0, 2000, false)?,
                ),
                (
                    "groups",
                    bounded(get(limits, "groups"), 100, 1, 200, false)?,
                ),
            ]),
        ),
    ]))
}
pub(crate) fn bind_properties(
    spec: &JsonValue,
    vocabulary: &LensVocabulary,
) -> Result<JsonValue, SearchV2Error> {
    let mut defs = BTreeMap::new();
    for d in &vocabulary.query_properties {
        let id = string(get(d, "property_id"));
        if id.is_empty() || defs.insert(id, d).is_some() {
            return Err(corrupt("ambiguous snapshot property identity"));
        }
    }
    fn bind(group: &mut JsonValue, defs: &BTreeMap<&str, &JsonValue>) -> Result<(), SearchV2Error> {
        let mut filters = array(get(group, "filters")).to_vec();
        for rule in &mut filters {
            let Some(id) = rule.object_get("property_id").and_then(JsonValue::as_str) else {
                continue;
            };
            let def = *defs
                .get(id)
                .ok_or_else(|| invalid("unknown snapshot property"))?;
            let field = string(get(def, "field"));
            let op = string(get(rule, "op"));
            if !allowed_field(field, true)
                || !array(get(def, "operators")).iter().any(|v| string(v) == op)
            {
                return Err(invalid("unsupported property operation"));
            }
            if op != "exists" {
                let val = get(rule, "value");
                let vals = val.as_array().unwrap_or(std::slice::from_ref(val));
                if vals.iter().any(|v| match string(get(def, "value_type")) {
                    "string" | "string-array" => v.as_str().is_none(),
                    "number" => !matches!(v, JsonValue::Number(_)),
                    "boolean" => !matches!(v, JsonValue::Bool(_)),
                    _ => true,
                }) {
                    return Err(invalid("invalid property value type"));
                }
            }
            set(rule, "field", text(field));
            set(rule, "_property_binding", def.clone());
        }
        set(group, "filters", JsonValue::Array(filters));
        Ok(())
    }
    let mut out = spec.clone();
    let mut root = get(&out, "node_query").clone();
    bind(&mut root, &defs)?;
    set(&mut out, "node_query", root);
    let mut paths = array(get(&out, "path_query")).to_vec();
    for path in &mut paths {
        let mut steps = array(get(path, "steps")).to_vec();
        for step in &mut steps {
            let mut g = get(step, "node_query").clone();
            bind(&mut g, &defs)?;
            set(step, "node_query", g);
        }
        set(path, "steps", JsonValue::Array(steps));
    }
    set(&mut out, "path_query", JsonValue::Array(paths));
    Ok(out)
}
fn numeric(v: &JsonValue) -> Option<f64> {
    if let JsonValue::Number(n) = v {
        n.lexeme.parse().ok()
    } else {
        None
    }
}
fn int_text(v: &JsonValue) -> Option<String> {
    match v {
        JsonValue::Bool(b) => Some(if *b { "1" } else { "0" }.into()),
        JsonValue::Number(n) if n.kind == JsonNumberKind::Int => {
            let negative = n.lexeme.starts_with('-');
            let digits = n.lexeme.trim_start_matches('-').trim_start_matches('0');
            Some(if digits.is_empty() {
                "0".into()
            } else {
                format!("{}{digits}", if negative { "-" } else { "" })
            })
        }
        _ => None,
    }
}
fn float_integer_text(n: f64) -> Option<String> {
    if !n.is_finite() || n.fract() != 0. {
        return None;
    }
    Some(format!("{n:.0}"))
}
fn equal(a: &JsonValue, b: &JsonValue) -> bool {
    match (int_text(a), int_text(b)) {
        (Some(a), Some(b)) => a == b,
        (Some(a), None) => numeric(b)
            .and_then(float_integer_text)
            .is_some_and(|b| a == b),
        (None, Some(b)) => numeric(a)
            .and_then(float_integer_text)
            .is_some_and(|a| a == b),
        (None, None) => match (a, b) {
            (JsonValue::Number(_), JsonValue::Number(_)) => numeric(a) == numeric(b),
            (JsonValue::Array(a), JsonValue::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
            }
            (JsonValue::Object(a), JsonValue::Object(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(k, v)| {
                        b.iter()
                            .find(|(k2, _)| k == k2)
                            .is_some_and(|(_, v2)| equal(v, v2))
                    })
            }
            _ => a == b,
        },
    }
}
pub(crate) fn py_string(v: &JsonValue) -> String {
    match v {
        JsonValue::Null => "None".into(),
        JsonValue::Bool(b) => if *b { "True" } else { "False" }.into(),
        JsonValue::String(_) => string(v).into(),
        JsonValue::Number(_) => String::from_utf8(
            tos_foundation::canonical_bytes_v1(
                v,
                tos_foundation::CanonicalProfile::SourceRecordDigestV1,
                tos_foundation::JsonLimits::default(),
            )
            .expect("admitted scalar number"),
        )
        .expect("canonical UTF8"),
        JsonValue::Array(a) => {
            format!("[{}]", a.iter().map(py_repr).collect::<Vec<_>>().join(", "))
        }
        JsonValue::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!(
                    "{}: {}",
                    py_repr(&text(k.as_str().unwrap_or(""))),
                    py_repr(v)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
fn py_repr(v: &JsonValue) -> String {
    if let Some(s) = v.as_str() {
        let quote = if s.contains('\'') && !s.contains('"') {
            '"'
        } else {
            '\''
        };
        format!(
            "{quote}{}{quote}",
            s.replace('\\', "\\\\")
                .replace(quote, &format!("\\{quote}"))
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t")
        )
    } else {
        py_string(v)
    }
}
fn truthy(v: &JsonValue) -> bool {
    match v {
        JsonValue::Null => false,
        JsonValue::Bool(v) => *v,
        JsonValue::Number(_) => numeric(v).is_some_and(|n| n != 0.),
        JsonValue::String(_) => !string(v).is_empty(),
        JsonValue::Array(a) => !a.is_empty(),
        JsonValue::Object(o) => !o.is_empty(),
    }
}
pub(crate) fn matches_filter(item: &JsonValue, rule: &JsonValue) -> bool {
    let definition = rule.object_get("_property_binding");
    if let Some(d) = definition {
        let mut types = vec![get(item, "type_id")];
        if boolean(get(d, "inherited")) {
            types.extend(array(field(item, "semantics.type_ancestors")))
        }
        if !array(get(d, "applies_to"))
            .iter()
            .any(|t| types.contains(&t))
        {
            return false;
        }
    }
    let actual = field(item, string(get(rule, "field")));
    let expected = get(rule, "value");
    let op = string(get(rule, "op"));
    if op == "exists" {
        return !matches!(actual, JsonValue::Null) == boolean(expected);
    }
    if definition.is_some() && matches!(actual, JsonValue::Null) {
        return false;
    }
    if definition.is_some() && actual.as_str().is_some() && matches!(op, "contains" | "prefix") {
        return expected.as_str().is_some_and(|s| {
            if op == "contains" {
                string(actual).contains(s)
            } else {
                string(actual).starts_with(s)
            }
        });
    }
    let eq = || {
        equal(actual, expected)
            || actual
                .as_array()
                .is_some_and(|a| a.iter().any(|v| equal(v, expected)))
    };
    match op {
        "eq" => eq(),
        "neq" => !eq(),
        "in" => {
            let e = expected
                .as_array()
                .unwrap_or(std::slice::from_ref(expected));
            if let Some(a) = actual.as_array() {
                a.iter().any(|v| e.iter().any(|e| equal(v, e)))
            } else {
                e.iter().any(|e| equal(actual, e))
            }
        }
        "contains" => {
            if let Some(a) = actual.as_array() {
                expected
                    .as_array()
                    .unwrap_or(std::slice::from_ref(expected))
                    .iter()
                    .all(|e| a.iter().any(|v| equal(v, e)))
            } else if expected.as_array().is_some() {
                false
            } else {
                lower(&py_string(if truthy(actual) { actual } else { &text("") }))
                    .contains(&lower(&py_string(expected)))
            }
        }
        "prefix" => lower(&py_string(if truthy(actual) { actual } else { &text("") }))
            .starts_with(&lower(&py_string(expected))),
        _ => {
            if let (Some(a), Some(b)) = (numeric(actual), numeric(expected)) {
                match op {
                    "gt" => a > b,
                    "gte" => a >= b,
                    "lt" => a < b,
                    "lte" => a <= b,
                    _ => false,
                }
            } else {
                false
            }
        }
    }
}
pub(crate) fn matches_group(item: &JsonValue, group: &JsonValue) -> bool {
    let filters = array(get(group, "filters"));
    filters.is_empty()
        || if string(get(group, "match")) == "all" {
            filters.iter().all(|r| matches_filter(item, r))
        } else {
            filters.iter().any(|r| matches_filter(item, r))
        }
}
pub(crate) fn sort_cmp(a: &JsonValue, b: &JsonValue, rules: &JsonValue) -> Ordering {
    for rule in array(rules) {
        let key = |v| {
            let f = field(v, string(get(rule, "field")));
            if truthy(f) {
                lower(&py_string(f))
            } else {
                String::new()
            }
        };
        let mut ord = key(a).cmp(&key(b));
        if string(get(rule, "direction")) == "desc" {
            ord = ord.reverse()
        }
        if ord != Ordering::Equal {
            return ord;
        }
    }
    string(get(a, "id")).cmp(string(get(b, "id")))
}
pub(crate) fn sort_items(items: &mut [JsonValue], rules: &JsonValue) {
    items.sort_by(|a, b| sort_cmp(a, b, rules))
}
pub(crate) fn stable_digest(v: &JsonValue) -> Result<String, SearchV2Error> {
    fn write(v: &JsonValue, h: &mut Digest256Hasher) -> Result<(), SearchV2Error> {
        match v {
            JsonValue::Null => h.update(b"n;"),
            JsonValue::Bool(b) => h.update(if *b { b"b1;" } else { b"b0;" }),
            JsonValue::String(_) => {
                let s = string(v).as_bytes();
                h.update(format!("s{}:", s.len()).as_bytes());
                h.update(s)
            }
            JsonValue::Number(n) => {
                let mut n = n
                    .lexeme
                    .parse::<f64>()
                    .map_err(|_| corrupt("nonfinite digest number"))?;
                if !n.is_finite() {
                    return Err(corrupt("nonfinite digest number"));
                }
                if n == 0. {
                    n = 0.
                }
                h.update(format!("d{:016x};", n.to_bits()).as_bytes())
            }
            JsonValue::Array(a) => {
                h.update(format!("a{}[", a.len()).as_bytes());
                for v in a {
                    write(v, h)?
                }
                h.update(b"]")
            }
            JsonValue::Object(o) => {
                let mut o: Vec<_> = o.iter().collect();
                o.sort_by(|(a, _), (b, _)| a.as_str().cmp(&b.as_str()));
                h.update(format!("o{}{{", o.len()).as_bytes());
                for (k, v) in o {
                    write(
                        &text(k.as_str().ok_or_else(|| corrupt("invalid digest key"))?),
                        h,
                    )?;
                    write(v, h)?
                }
                h.update(b"}")
            }
        }
        Ok(())
    }
    let mut h = Digest256Hasher::new();
    write(v, &mut h)?;
    Ok(h.finalize().to_hex())
}
