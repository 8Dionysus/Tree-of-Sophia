//! Local constructor authoring rules. The browser owns storage and listeners;
//! source witnesses, rights, review and canon remain with their source owners.
use std::collections::{HashMap, HashSet};
use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};

const SCHEMA: &str = "tos_constructor_workspace_v1";
const MAX_BYTES: usize = 1_000_000;
const MAX_LIBRARY_BYTES: usize = 16_000_000;
const MAX_REVISION: u64 = 1_000_000_000;
const SOURCES: &[&str] = &["work", "part", "chapter", "fragment", "dossier"];
const DRAFTS: &[&str] = &["concept", "interpretation", "question", "note", "excerpt"];
const MATERIALS: &[&str] = &[
    "work",
    "part",
    "chapter",
    "fragment",
    "dossier",
    "concept",
    "interpretation",
    "question",
    "note",
    "excerpt",
    "figure",
    "symbol",
    "tradition",
    "character",
];
const RELATIONS: &[&str] = &[
    "contains",
    "supports",
    "questions",
    "relates",
    "interprets",
    "contrasts",
    "echoes",
    "develops",
    "compares",
    "translates",
];
type RuleResult<T> = Result<T, String>;
fn fail<T>(message: impl Into<String>) -> RuleResult<T> {
    Err(message.into())
}
fn get<'a>(v: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    v.object_get(key)
}
fn word<'a>(v: &'a JsonValue, key: &str) -> Option<&'a str> {
    get(v, key)?.as_str()
}
fn string(v: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(v))
}
fn integer(v: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: v.to_string(),
    })
}
fn obj(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn array<'a>(v: &'a JsonValue, key: &str) -> Option<&'a [JsonValue]> {
    get(v, key)?.as_array()
}
fn array_mut<'a>(v: &'a mut JsonValue, key: &str) -> &'a mut Vec<JsonValue> {
    let JsonValue::Object(fields) = v else {
        unreachable!()
    };
    let (_, JsonValue::Array(items)) = fields
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some(key))
        .expect("validated array")
    else {
        unreachable!()
    };
    items
}
fn set(v: &mut JsonValue, key: &str, value: JsonValue) {
    let JsonValue::Object(fields) = v else {
        unreachable!()
    };
    if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *old = value;
    } else {
        fields.push((JsonString::from_utf8(key), value));
    }
}
fn exact(v: &JsonValue, required: &[&str], optional: &[&str], name: &str) -> RuleResult<()> {
    let Some(fields) = v.as_object() else {
        return fail(format!("{name} has unsupported or missing fields"));
    };
    if required.iter().any(|k| get(v, k).is_none())
        || fields.iter().any(|(k, _)| {
            !k.as_str()
                .is_some_and(|s| required.contains(&s) || optional.contains(&s))
        })
    {
        return fail(format!("{name} has unsupported or missing fields"));
    }
    Ok(())
}
fn text(v: &JsonValue, name: &str, max: usize, empty: bool) -> RuleResult<String> {
    let Some(s) = v.as_str() else {
        return fail(format!("{name} must be text"));
    };
    if s.encode_utf16().count() > max || (!empty && s.trim_matches(js_trim).is_empty()) {
        return fail(format!(
            "{name} must be nonempty text of at most {max} characters"
        ));
    }
    Ok(s.to_owned())
}
fn js_trim(ch: char) -> bool {
    matches!(ch, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}'
        | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
fn field_text(v: &JsonValue, key: &str, name: &str, max: usize, empty: bool) -> RuleResult<String> {
    text(get(v, key).unwrap_or(&JsonValue::Null), name, max, empty)
}
fn parse(raw: &[u8], max: usize, problem: &str) -> RuleResult<JsonValue> {
    if raw.len() > max {
        return fail(problem);
    }
    parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: max,
            ..JsonLimits::default()
        },
    )
    .map(|v| v.into_root())
    .map_err(|_| problem.to_owned())
}
fn emit(v: &JsonValue) -> RuleResult<Vec<u8>> {
    emit_value_preserved_json(
        v,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "constructor packet exceeds the 1 MB storage limit".to_owned())
}
fn bounded(v: &JsonValue) -> RuleResult<Vec<u8>> {
    let bytes = emit(v)?;
    if std::str::from_utf8(&bytes).map_or(true, |s| s.encode_utf16().count() * 2 > MAX_BYTES) {
        return fail("constructor packet exceeds the 1 MB storage limit");
    }
    Ok(bytes)
}
fn pos(v: &JsonValue) -> RuleResult<JsonValue> {
    let Some(a) = v.as_array() else {
        return fail("position must contain three finite coordinates between -100000 and 100000");
    };
    if a.len() != 3
        || a.iter()
            .any(|n| float(n).is_none_or(|x| !x.is_finite() || x.abs() > 100_000.0))
    {
        return fail("position must contain three finite coordinates between -100000 and 100000");
    }
    Ok(v.clone())
}
fn number(v: f64) -> JsonValue {
    let lexeme = v.to_string();
    let kind = if lexeme.contains(['.', 'e', 'E']) {
        JsonNumberKind::Float
    } else {
        JsonNumberKind::Int
    };
    JsonValue::Number(JsonNumber { kind, lexeme })
}
fn float(v: &JsonValue) -> Option<f64> {
    if let JsonValue::Number(n) = v {
        n.lexeme.parse().ok()
    } else {
        None
    }
}
fn radial(index: usize, count: usize, center: [f64; 3], radius: f64) -> JsonValue {
    let angle =
        -std::f64::consts::FRAC_PI_2 + index as f64 * std::f64::consts::TAU / count.max(1) as f64;
    JsonValue::Array(vec![
        number(center[0] + angle.cos() * radius),
        number(center[1] + angle.sin() * radius),
        number(center[2] + (index as i32 % 3 - 1) as f64 * 22.0),
    ])
}
fn coords(v: &JsonValue) -> [f64; 3] {
    let a = v.as_array().expect("validated position");
    [0, 1, 2].map(|i| float(&a[i]).expect("validated finite"))
}
fn unique_ids(v: &JsonValue, max: usize, name: &str) -> RuleResult<Vec<String>> {
    let Some(a) = v.as_array() else {
        return fail(format!(
            "{name} must contain at most {max} unique material IDs"
        ));
    };
    if a.len() > max {
        return fail(format!("{name} exceeds limit"));
    }
    let mut seen = HashSet::new();
    let mut ids = Vec::with_capacity(a.len());
    for item in a {
        let Some(id) = item.as_str() else {
            return fail(format!("{name} contains an invalid ID"));
        };
        if !seen.insert(id) {
            return fail(format!("{name} contains duplicate ID"));
        }
        ids.push(id.to_owned());
    }
    Ok(ids)
}

pub struct ConstructorMachine {
    materials: HashMap<String, JsonValue>,
    children: HashMap<String, Vec<String>>,
    atlas_edges: HashMap<String, JsonValue>,
    atlas_order: Vec<String>,
    atlas_defaults: Option<Vec<String>>,
    root: String,
    fingerprint: String,
    state: JsonValue,
    undo: Vec<JsonValue>,
    redo: Vec<JsonValue>,
    serial: u64,
}

impl ConstructorMachine {
    fn new(library: JsonValue) -> RuleResult<Self> {
        if word(&library, "schema") != Some("tos_constructor_library_v1")
            || array(&library, "nodes").is_none()
        {
            return fail("constructor library schema is invalid");
        }
        let fingerprint = field_text(&library, "fingerprint", "library fingerprint", 64, false)?;
        if fingerprint.len() != 64
            || !fingerprint
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return fail("library fingerprint must be a SHA-256 content digest");
        }
        let mut materials = HashMap::new();
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        for m in array(&library, "nodes").unwrap() {
            let Some(kind) = word(m, "kind") else {
                return fail("library material kind is invalid");
            };
            if !MATERIALS.contains(&kind) {
                return fail("library material kind is invalid");
            }
            let id = field_text(m, "id", "library material id", 220, false)?;
            if materials.contains_key(&id) {
                return fail(format!("duplicate library material: {id}"));
            }
            let demo = get(m, "demo");
            if (!SOURCES.contains(&kind) && demo != Some(&JsonValue::Bool(true)))
                || demo.is_some_and(|d| !matches!(d, JsonValue::Bool(_)))
            {
                return fail("mockup material kinds must declare demo: true");
            }
            if demo == Some(&JsonValue::Bool(true)) {
                for field in ["title", "body"] {
                    for lang in ["ru", "en"] {
                        let max = if field == "title" { 512 } else { 4_000 };
                        text(
                            get(get(m, field).unwrap_or(&JsonValue::Null), lang)
                                .unwrap_or(&JsonValue::Null),
                            &format!("mockup {field}.{lang}"),
                            max,
                            field == "body",
                        )?;
                    }
                }
                pos(get(m, "position").unwrap_or(&JsonValue::Null))?;
            }
            materials.insert(id, m.clone());
        }
        let root = word(&library, "rootId")
            .ok_or("library root does not exist")?
            .to_owned();
        if !materials.contains_key(&root) {
            return fail("library root does not exist");
        }
        // The authored array is the same insertion order used by the previous
        // JavaScript Map. Append children in that order instead of sorting each
        // sibling list with a repeated scan of the whole library.
        for m in array(&library, "nodes").unwrap() {
            let id = word(m, "id").expect("validated library material id");
            let parent =
                get(m, "parentId").ok_or_else(|| format!("library parent does not exist: {id}"))?;
            if !parent.is_null() {
                let Some(p) = parent.as_str() else {
                    return fail(format!("library parent does not exist: {id}"));
                };
                if !materials.contains_key(p) {
                    return fail(format!("library parent does not exist: {id}"));
                }
                if p == id {
                    return fail(format!("library material cannot parent itself: {id}"));
                }
                children
                    .entry(p.to_owned())
                    .or_default()
                    .push(id.to_owned());
            }
        }
        // A verified ancestor chain cannot gain a cycle in this immutable
        // library. Visit each material at most once across all chains.
        let mut verified = HashSet::new();
        for m in array(&library, "nodes").unwrap() {
            let id = word(m, "id").unwrap();
            if verified.contains(id) {
                continue;
            }
            let mut seen = HashSet::new();
            let mut path = Vec::new();
            let mut parent = Some(id);
            while let Some(p) = parent {
                if verified.contains(p) {
                    break;
                }
                if !seen.insert(p) {
                    return fail("library hierarchy contains a cycle");
                }
                path.push(p);
                parent = word(materials.get(p).expect("known parent"), "parentId");
            }
            verified.extend(path);
        }
        let mut atlas_edges = HashMap::new();
        let mut atlas_order = Vec::new();
        let mut atlas_defaults = None;
        if let Some(atlas) = get(&library, "atlas") {
            let Some(edges) = array(atlas, "edges") else {
                return fail("atlas must contain an edges array");
            };
            let defaults = unique_ids(
                get(atlas, "defaultIds").unwrap_or(&JsonValue::Null),
                200,
                "atlas defaultIds",
            )?;
            if defaults.iter().any(|id| !materials.contains_key(id)) {
                return fail("atlas defaultIds contains an unknown material");
            }
            atlas_defaults = Some(defaults);
            for edge in edges {
                let id = field_text(edge, "id", "atlas edge id", 220, false)?;
                if atlas_edges.contains_key(&id) {
                    return fail(format!("duplicate atlas edge: {id}"));
                }
                let from = word(edge, "from");
                let to = word(edge, "to");
                let kind = word(edge, "kind");
                if from == to
                    || !from.is_some_and(|v| materials.contains_key(v))
                    || !to.is_some_and(|v| materials.contains_key(v))
                {
                    return fail("atlas edge endpoints must be two known, distinct materials");
                }
                if !kind.is_some_and(|v| RELATIONS.contains(&v)) {
                    return fail("atlas edge kind is invalid");
                }
                atlas_order.push(id.clone());
                atlas_edges.insert(id, edge.clone());
            }
        }
        let state = obj(vec![
            ("schema", string(SCHEMA)),
            ("version", integer(1)),
            ("libraryFingerprint", string(&fingerprint)),
            ("title", string("Tree of Sophia")),
            ("nodes", JsonValue::Array(vec![])),
            ("edges", JsonValue::Array(vec![])),
            ("revision", integer(0)),
        ]);
        Ok(Self {
            materials,
            children,
            atlas_edges,
            atlas_order,
            atlas_defaults,
            root,
            fingerprint,
            state,
            undo: vec![],
            redo: vec![],
            serial: 0,
        })
    }
    fn validate(&self, c: &JsonValue) -> RuleResult<()> {
        exact(
            c,
            &[
                "schema",
                "version",
                "libraryFingerprint",
                "title",
                "nodes",
                "edges",
                "revision",
            ],
            &[],
            "constructor packet",
        )?;
        if word(c, "schema") != Some(SCHEMA)
            || get(c, "version").and_then(JsonValue::as_u64) != Some(1)
        {
            return fail("constructor schema or version is unsupported");
        }
        if word(c, "libraryFingerprint") != Some(self.fingerprint.as_str()) {
            return fail(
                "constructor library fingerprint does not match; material references cannot be rebound to another library",
            );
        }
        field_text(c, "title", "tree title", 512, false)?;
        if get(c, "revision")
            .and_then(JsonValue::as_u64)
            .is_none_or(|n| n > MAX_REVISION)
        {
            return fail("revision is invalid");
        }
        let nodes = array(c, "nodes").ok_or("tree supports at most 200 nodes")?;
        let edges = array(c, "edges").ok_or("tree supports at most 600 edges")?;
        if nodes.len() > 200 {
            return fail("tree supports at most 200 nodes");
        }
        if edges.len() > 600 {
            return fail("tree supports at most 600 edges");
        }
        let mut node_ids = HashSet::new();
        let mut used_materials = HashSet::new();
        let mut node_index = HashMap::new();
        for node in nodes {
            let Some(_) = node.as_object() else {
                return fail("node is invalid");
            };
            if let Some(mid) = get(node, "materialId") {
                exact(
                    node,
                    &["id", "materialId", "kind", "sourceIds", "position"],
                    &[],
                    "material node",
                )?;
                let material =
                    self.material(word(node, "materialId").ok_or("material node is invalid")?)?;
                let id = word(node, "id").ok_or("material node is invalid")?;
                if id != format!("material:{}", word(material, "id").unwrap())
                    || word(node, "kind") != word(material, "kind")
                {
                    return fail("material node must match its library identity and kind");
                }
                if !used_materials.insert(mid.as_str().unwrap()) {
                    return fail("a material can occur only once in one tree");
                }
            } else {
                exact(
                    node,
                    &[
                        "id",
                        "kind",
                        "title",
                        "body",
                        "sourceIds",
                        "position",
                        "localOnly",
                        "source",
                        "reviewed",
                        "canon",
                    ],
                    &[],
                    "draft node",
                )?;
                if !word(node, "kind").is_some_and(|k| DRAFTS.contains(&k)) {
                    return fail("draft kind is invalid");
                }
                for (k, v) in [
                    ("localOnly", true),
                    ("source", false),
                    ("reviewed", false),
                    ("canon", false),
                ] {
                    if get(node, k).and_then(JsonValue::as_bool) != Some(v) {
                        return fail(
                            "draft authority must remain local-only, unreviewed and non-canonical",
                        );
                    }
                }
                field_text(node, "title", "draft title", 512, false)?;
                field_text(node, "body", "draft body", 4_000, true)?;
                if !word(node, "id").is_some_and(|id| id.starts_with("draft:")) {
                    return fail("draft id must use the draft namespace");
                }
            }
            let id = field_text(node, "id", "node id", 256, false)?;
            pos(get(node, "position").unwrap_or(&JsonValue::Null))?;
            if !node_ids.insert(id.clone()) {
                return fail(format!("duplicate node: {id}"));
            }
            let refs = unique_ids(
                get(node, "sourceIds").unwrap_or(&JsonValue::Null),
                200,
                "sourceIds",
            )?;
            if get(node, "materialId").is_some() && !refs.is_empty() {
                return fail("material source references come from the library");
            }
            node_index.insert(id, node);
        }
        for node in nodes {
            for source in array(node, "sourceIds").unwrap() {
                let id = source.as_str().unwrap();
                if !node_index.contains_key(id) || Some(id) == word(node, "id") {
                    return fail(format!(
                        "source node does not exist or references itself: {id}"
                    ));
                }
            }
        }
        let mut edge_ids = HashSet::new();
        let mut relation_keys = HashSet::new();
        for edge in edges {
            exact(
                edge,
                &["id", "from", "to", "kind", "origin"],
                &["label"],
                "edge",
            )?;
            let id = field_text(edge, "id", "edge id", 256, false)?;
            let from = word(edge, "from").ok_or("edge endpoints are invalid")?;
            let to = word(edge, "to").ok_or("edge endpoints are invalid")?;
            let kind = word(edge, "kind").ok_or("edge kind is invalid")?;
            let origin = word(edge, "origin").ok_or("edge origin is invalid")?;
            if from == to || !node_index.contains_key(from) || !node_index.contains_key(to) {
                return fail("edge endpoints must be two existing, distinct nodes");
            }
            if !RELATIONS.contains(&kind) {
                return fail("edge kind is invalid");
            }
            if let Some(label) = get(edge, "label") {
                text(label, "edge label", 512, true)?;
            }
            if origin != "structure" && origin != "draft" {
                return fail("edge origin is invalid");
            }
            if let Some(aid) = id.strip_prefix("atlas:") {
                let owned = self
                    .atlas_edges
                    .get(aid)
                    .ok_or("atlas edge must match its exact library relation")?;
                if from != format!("material:{}", word(owned, "from").unwrap())
                    || to != format!("material:{}", word(owned, "to").unwrap())
                    || Some(kind) != word(owned, "kind")
                    || origin != "draft"
                    || get(edge, "label").is_some()
                {
                    return fail("atlas edge must match its exact library relation");
                }
            }
            if origin == "structure" {
                let parent = get(node_index[from], "materialId").and_then(JsonValue::as_str);
                let child = get(node_index[to], "materialId").and_then(JsonValue::as_str);
                if kind != "contains"
                    || parent.is_none()
                    || child.is_none()
                    || word(self.material(child.unwrap())?, "parentId") != parent
                    || get(edge, "label").is_some()
                {
                    return fail("structural edge must match the exact library hierarchy");
                }
            }
            let relation = (
                from.to_owned(),
                to.to_owned(),
                kind.to_owned(),
                origin.to_owned(),
                id.starts_with("atlas:")
                    .then(|| id.clone())
                    .unwrap_or_else(|| word(edge, "label").unwrap_or("").to_owned()),
                id.starts_with("atlas:"),
            );
            if !edge_ids.insert(id) || !relation_keys.insert(relation) {
                return fail("duplicate edge identity or relation");
            }
        }
        bounded(c)?;
        Ok(())
    }
    fn material(&self, id: &str) -> RuleResult<&JsonValue> {
        self.materials
            .get(id)
            .ok_or_else(|| format!("material does not exist in this library: {id}"))
    }
    fn node<'a>(draft: &'a JsonValue, id: &str) -> RuleResult<&'a JsonValue> {
        array(draft, "nodes")
            .unwrap()
            .iter()
            .find(|n| word(n, "id") == Some(id))
            .ok_or_else(|| format!("node does not exist: {id}"))
    }
    fn revision(&self) -> RuleResult<u64> {
        let n = get(&self.state, "revision")
            .and_then(JsonValue::as_u64)
            .unwrap();
        if n >= MAX_REVISION {
            fail("constructor revision limit reached; export this tree before starting another")
        } else {
            Ok(n + 1)
        }
    }
    fn push(stack: &mut Vec<JsonValue>, state: JsonValue) {
        stack.push(state);
        if stack.len() > 64 {
            stack.remove(0);
        }
    }
    fn commit(&mut self, mut candidate: JsonValue) -> RuleResult<bool> {
        if candidate == self.state {
            return Ok(false);
        }
        set(&mut candidate, "revision", integer(self.revision()?));
        self.validate(&candidate)?;
        Self::push(&mut self.undo, self.state.clone());
        self.redo.clear();
        self.state = candidate;
        Ok(true)
    }
    fn transition(&mut self, undo: bool) -> RuleResult<bool> {
        let stack = if undo { &self.undo } else { &self.redo };
        let Some(mut candidate) = stack.last().cloned() else {
            return Ok(false);
        };
        set(&mut candidate, "revision", integer(self.revision()?));
        self.validate(&candidate)?;
        if undo {
            self.undo.pop();
            Self::push(&mut self.redo, self.state.clone())
        } else {
            self.redo.pop();
            Self::push(&mut self.undo, self.state.clone())
        }
        self.state = candidate;
        Ok(true)
    }
    fn fresh(&mut self, prefix: &str, draft: &JsonValue) -> String {
        loop {
            self.serial += 1;
            let id = format!("{prefix}:{}", self.serial);
            if !array(draft, "nodes")
                .unwrap()
                .iter()
                .chain(array(draft, "edges").unwrap())
                .any(|v| word(v, "id") == Some(id.as_str()))
            {
                return id;
            }
        }
    }
    fn add_edge(
        &mut self,
        draft: &mut JsonValue,
        from: &str,
        to: &str,
        kind: &str,
        origin: &str,
        label: Option<&str>,
    ) -> RuleResult<String> {
        Self::node(draft, from)?;
        Self::node(draft, to)?;
        if let Some(edge) = array(draft, "edges").unwrap().iter().find(|e| {
            !word(e, "id").unwrap_or("").starts_with("atlas:")
                && word(e, "from") == Some(from)
                && word(e, "to") == Some(to)
                && word(e, "kind") == Some(kind)
                && word(e, "origin") == Some(origin)
                && word(e, "label").unwrap_or("") == label.unwrap_or("")
        }) {
            return Ok(word(edge, "id").unwrap().to_owned());
        }
        let id = self.fresh("edge", draft);
        let mut fields = vec![
            ("id", string(&id)),
            ("from", string(from)),
            ("to", string(to)),
            ("kind", string(kind)),
            ("origin", string(origin)),
        ];
        if let Some(label) = label.filter(|s| !s.is_empty()) {
            fields.push(("label", string(label)))
        }
        array_mut(draft, "edges").push(obj(fields));
        Ok(id)
    }
    fn add_material(
        &mut self,
        draft: &mut JsonValue,
        id: &str,
        position: Option<JsonValue>,
        parent: Option<&str>,
    ) -> RuleResult<String> {
        let material = self.material(id)?;
        let material_kind = word(material, "kind").unwrap().to_owned();
        let library_parent = word(material, "parentId").map(str::to_owned);
        if let Some(p) = &position {
            pos(p)?;
        }
        if let Some(p) = parent {
            Self::node(draft, p)?;
        }
        let node_id = format!("material:{id}");
        if !array(draft, "nodes")
            .unwrap()
            .iter()
            .any(|n| word(n, "id") == Some(node_id.as_str()))
        {
            let at = position.unwrap_or_else(|| {
                radial(array(draft, "nodes").unwrap().len(), 7, [0.0; 3], 110.0)
            });
            array_mut(draft, "nodes").push(obj(vec![
                ("id", string(&node_id)),
                ("materialId", string(id)),
                ("kind", string(&material_kind)),
                ("sourceIds", JsonValue::Array(vec![])),
                ("position", at),
            ]));
        }
        let parent = parent.map(str::to_owned).or_else(|| {
            array(draft, "nodes")
                .unwrap()
                .iter()
                .find(|n| word(n, "materialId") == library_parent.as_deref())
                .and_then(|n| word(n, "id"))
                .map(str::to_owned)
        });
        if let Some(parent) = parent {
            let structural =
                word(Self::node(draft, &parent)?, "materialId") == library_parent.as_deref();
            self.add_edge(
                draft,
                &parent,
                &node_id,
                "contains",
                if structural { "structure" } else { "draft" },
                None,
            )?;
        }
        Ok(node_id)
    }
    fn grow(&mut self, draft: &mut JsonValue, input: &JsonValue) -> RuleResult<()> {
        if self.atlas_defaults.is_none() {
            return fail("this library has no atlas presets");
        }
        exact(input, &["nodeIds"], &["edgeIds"], "atlas selection")?;
        let ids = unique_ids(get(input, "nodeIds").unwrap(), 200, "atlas nodeIds")?;
        for id in &ids {
            self.material(id)?;
        }
        let edges = if let Some(v) = get(input, "edgeIds") {
            let ids = unique_ids(v, 600, "atlas edgeIds")?;
            for id in &ids {
                if !self.atlas_edges.contains_key(id) {
                    return fail(format!("unknown atlas edge: {id}"));
                }
            }
            Some(ids)
        } else {
            None
        };
        for id in &ids {
            let position = self
                .material(id)
                .ok()
                .and_then(|m| get(m, "position"))
                .cloned();
            self.add_material(draft, id, position, None)?;
        }
        for id in &ids {
            self.add_material(draft, id, None, None)?;
        }
        let visible: HashSet<String> = array(draft, "nodes")
            .unwrap()
            .iter()
            .filter_map(|n| word(n, "id").map(str::to_owned))
            .collect();
        let chosen = edges.unwrap_or_else(|| {
            self.atlas_order
                .iter()
                .filter(|id| {
                    let e = &self.atlas_edges[*id];
                    visible.contains(&format!("material:{}", word(e, "from").unwrap()))
                        && visible.contains(&format!("material:{}", word(e, "to").unwrap()))
                })
                .cloned()
                .collect()
        });
        for id in chosen {
            let edge = &self.atlas_edges[&id];
            let from = format!("material:{}", word(edge, "from").unwrap());
            let to = format!("material:{}", word(edge, "to").unwrap());
            if !visible.contains(&from) || !visible.contains(&to) {
                return fail(format!(
                    "atlas edge requires both materials in the workspace: {id}"
                ));
            }
            let atlas_id = format!("atlas:{id}");
            if !array(draft, "edges")
                .unwrap()
                .iter()
                .any(|e| word(e, "id") == Some(atlas_id.as_str()))
            {
                let kind = word(edge, "kind").unwrap().to_owned();
                array_mut(draft, "edges").push(obj(vec![
                    ("id", string(&atlas_id)),
                    ("from", string(&from)),
                    ("to", string(&to)),
                    ("kind", string(&kind)),
                    ("origin", string("draft")),
                ]));
            }
        }
        Ok(())
    }
    fn command(&mut self, command: &JsonValue) -> RuleResult<JsonValue> {
        let kind = word(command, "kind").ok_or("constructor command kind is invalid")?;
        if kind == "import" {
            let packet = word(command, "packet").ok_or("constructor packet is not valid JSON")?;
            let imported = parse(
                packet.as_bytes(),
                MAX_BYTES,
                "constructor packet is not valid JSON",
            )?;
            if packet.encode_utf16().count() * 2 > MAX_BYTES {
                return fail("constructor packet exceeds the 1 MB storage limit");
            }
            self.validate(&imported)?;
            // Object.assign onto the existing JS state retained its member order.
            // A reordered imported packet must not manufacture a new edit.
            let mut draft = self.state.clone();
            for key in [
                "schema",
                "version",
                "libraryFingerprint",
                "title",
                "nodes",
                "edges",
            ] {
                set(&mut draft, key, get(&imported, key).unwrap().clone());
            }
            let changed = self.commit(draft)?;
            return Ok(obj(vec![
                ("changed", JsonValue::Bool(changed)),
                ("value", JsonValue::Bool(changed)),
            ]));
        }
        if kind == "undo" || kind == "redo" {
            let changed = self.transition(kind == "undo")?;
            return Ok(obj(vec![
                ("changed", JsonValue::Bool(changed)),
                ("value", JsonValue::Bool(changed)),
            ]));
        }
        let mut draft = self.state.clone();
        let mut result = JsonValue::Null;
        match kind {
            "material.add" => {
                let id = word(command, "id").ok_or("material id is invalid")?;
                let options = get(command, "options").unwrap_or(&JsonValue::Null);
                let parent = word(options, "parentId");
                if get(options, "parentId").is_some() && parent.is_none() {
                    return fail("node does not exist: invalid parentId");
                }
                let position = get(options, "position").cloned();
                result = string(&self.add_material(&mut draft, id, position, parent)?);
            }
            "material.expand" => {
                let id = word(command, "id").ok_or("node id is invalid")?;
                let node = Self::node(&draft, id)?;
                let mid = word(node, "materialId")
                    .ok_or("only library material nodes have structural children")?
                    .to_owned();
                let center = coords(get(node, "position").unwrap());
                let children = self.children.get(&mid).cloned().unwrap_or_default();
                let mut added = vec![];
                for (index, child) in children.iter().enumerate() {
                    let cid = format!("material:{child}");
                    if !array(&draft, "nodes")
                        .unwrap()
                        .iter()
                        .any(|n| word(n, "id") == Some(cid.as_str()))
                    {
                        added.push(string(&cid))
                    }
                    self.add_material(
                        &mut draft,
                        child,
                        Some(radial(index, children.len(), center, 110.0)),
                        Some(id),
                    )?;
                }
                result = JsonValue::Array(added);
            }
            "draft.add" | "draft.context" => {
                let input = get(command, "input").ok_or("draft input is invalid")?;
                exact(
                    input,
                    &["kind", "title"],
                    &["body", "sourceIds", "position"],
                    "draft input",
                )?;
                let options = get(command, "options").unwrap_or(&JsonValue::Null);
                if kind == "draft.context" {
                    if !options.is_null() {
                        exact(
                            options,
                            &[],
                            &["sourceId", "materialId", "relationKind", "reverse"],
                            "draft context",
                        )?;
                    }
                    if get(options, "sourceId").is_some() && get(options, "materialId").is_some() {
                        return fail(
                            "draft context must specify either sourceId or materialId, not both",
                        );
                    }
                    if get(options, "reverse").is_some()
                        && get(options, "reverse")
                            .and_then(JsonValue::as_bool)
                            .is_none()
                    {
                        return fail("draft context reverse must be boolean");
                    }
                    if get(options, "sourceId").is_some() && word(options, "sourceId").is_none() {
                        return fail("node does not exist: invalid sourceId");
                    }
                    if get(options, "materialId").is_some() && word(options, "materialId").is_none()
                    {
                        return fail("material does not exist in this library: invalid materialId");
                    }
                    if get(options, "relationKind").is_some_and(|v| !v.is_null())
                        && word(options, "relationKind").is_none()
                    {
                        return fail("draft context relation kind is invalid");
                    }
                    if !RELATIONS.contains(&word(options, "relationKind").unwrap_or("relates")) {
                        return fail("draft context relation kind is invalid");
                    }
                }
                let source = if let Some(id) = word(options, "sourceId") {
                    Some(Self::node(&draft, id).map(|_| id.to_owned())?)
                } else if let Some(id) = word(options, "materialId") {
                    Some(self.add_material(&mut draft, id, None, None)?)
                } else {
                    None
                };
                let mut refs = get(input, "sourceIds")
                    .cloned()
                    .unwrap_or(JsonValue::Array(vec![]));
                let JsonValue::Array(ref mut ids) = refs else {
                    return fail("sourceIds must be an array of node IDs");
                };
                if let Some(source) = &source {
                    if !ids.iter().any(|v| v.as_str() == Some(source)) {
                        ids.push(string(source));
                    }
                }
                let id = self.fresh("draft", &draft);
                let position = if let Some(p) = get(input, "position") {
                    p.clone()
                } else if let Some(source) = &source {
                    let center = coords(get(Self::node(&draft, source)?, "position").unwrap());
                    JsonValue::Array(vec![
                        number(center[0] + 150.0),
                        number(center[1] + 105.0),
                        number(center[2] + 20.0),
                    ])
                } else {
                    radial(array(&draft, "nodes").unwrap().len(), 7, [0.0; 3], 110.0)
                };
                let mut fields = vec![
                    ("id", string(&id)),
                    ("kind", get(input, "kind").unwrap().clone()),
                    ("title", get(input, "title").unwrap().clone()),
                    ("body", get(input, "body").cloned().unwrap_or(string(""))),
                    ("sourceIds", refs),
                    ("position", position),
                ];
                fields.extend([
                    ("localOnly", JsonValue::Bool(true)),
                    ("source", JsonValue::Bool(false)),
                    ("reviewed", JsonValue::Bool(false)),
                    ("canon", JsonValue::Bool(false)),
                ]);
                array_mut(&mut draft, "nodes").push(obj(fields));
                if let Some(source) = source {
                    let relation = word(options, "relationKind").unwrap_or("relates");
                    let reverse =
                        get(options, "reverse").and_then(JsonValue::as_bool) == Some(true);
                    self.add_edge(
                        &mut draft,
                        if reverse { &id } else { &source },
                        if reverse { &source } else { &id },
                        relation,
                        "draft",
                        None,
                    )?;
                }
                result = string(&id);
            }
            "draft.edit" => {
                let id = word(command, "id").ok_or("node id is invalid")?;
                let patch = get(command, "patch").ok_or("draft edit is invalid")?;
                exact(patch, &[], &["title", "body"], "draft edit")?;
                let node = Self::node(&draft, id)?;
                if get(node, "materialId").is_some() {
                    return fail("library material cannot be edited as a draft");
                }
                let node = array_mut(&mut draft, "nodes")
                    .iter_mut()
                    .find(|n| word(n, "id") == Some(id))
                    .unwrap();
                if let Some(v) = get(patch, "title") {
                    set(node, "title", v.clone())
                }
                if let Some(v) = get(patch, "body") {
                    set(node, "body", v.clone())
                }
            }
            "edge.connect" => {
                let from = word(command, "from").ok_or("edge source is invalid")?;
                let to = word(command, "to").ok_or("edge target is invalid")?;
                let relation = word(command, "relation").ok_or("edge kind is invalid")?;
                let label = get(command, "label")
                    .map(|v| text(v, "edge label", 512, true))
                    .transpose()?;
                result = string(&self.add_edge(
                    &mut draft,
                    from,
                    to,
                    relation,
                    "draft",
                    label.as_deref(),
                )?);
            }
            "node.remove" => {
                let id = word(command, "id").ok_or("node id is invalid")?;
                Self::node(&draft, id)?;
                array_mut(&mut draft, "nodes").retain(|n| word(n, "id") != Some(id));
                array_mut(&mut draft, "edges")
                    .retain(|e| word(e, "from") != Some(id) && word(e, "to") != Some(id));
                for node in array_mut(&mut draft, "nodes") {
                    array_mut(node, "sourceIds").retain(|v| v.as_str() != Some(id));
                }
            }
            "edge.remove" => {
                let id = word(command, "id").ok_or("edge id is invalid")?;
                if !array(&draft, "edges")
                    .unwrap()
                    .iter()
                    .any(|e| word(e, "id") == Some(id))
                {
                    return fail(format!("edge does not exist: {id}"));
                }
                array_mut(&mut draft, "edges").retain(|e| word(e, "id") != Some(id));
            }
            "node.move" => {
                let id = word(command, "id").ok_or("node id is invalid")?;
                let position = pos(get(command, "position").unwrap_or(&JsonValue::Null))?;
                let node = array_mut(&mut draft, "nodes")
                    .iter_mut()
                    .find(|n| word(n, "id") == Some(id))
                    .ok_or_else(|| format!("node does not exist: {id}"))?;
                set(node, "position", position);
            }
            "title.rename" => {
                let title = get(command, "title").ok_or("tree title is invalid")?;
                text(title, "tree title", 512, false)?;
                set(&mut draft, "title", title.clone());
            }
            "clear" => {
                set(&mut draft, "nodes", JsonValue::Array(vec![]));
                set(&mut draft, "edges", JsonValue::Array(vec![]));
            }
            "atlas.grow" => {
                self.grow(
                    &mut draft,
                    get(command, "selection").ok_or("atlas selection is invalid")?,
                )?;
            }
            "atlas.seed" => {
                let ids = self
                    .atlas_defaults
                    .clone()
                    .ok_or("this library has no atlas presets")?;
                self.grow(
                    &mut draft,
                    &obj(vec![(
                        "nodeIds",
                        JsonValue::Array(ids.iter().map(|id| string(id)).collect()),
                    )]),
                )?;
            }
            "seed" => {
                let root = self.root.clone();
                let root_id = self.add_material(
                    &mut draft,
                    &root,
                    Some(JsonValue::Array(vec![integer(0), integer(0), integer(0)])),
                    None,
                )?;
                let parts = self
                    .children
                    .get(&root)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|id| word(&self.materials[id], "kind") == Some("part"))
                    .take(4)
                    .collect::<Vec<_>>();
                for (index, part) in parts.iter().enumerate() {
                    self.add_material(
                        &mut draft,
                        part,
                        Some(radial(index, parts.len(), [0.0; 3], 140.0)),
                        Some(&root_id),
                    )?;
                }
            }
            _ => return fail("constructor command kind is invalid"),
        }
        let changed = self.commit(draft)?;
        if result.is_null() {
            result = JsonValue::Bool(changed)
        }
        Ok(obj(vec![
            ("changed", JsonValue::Bool(changed)),
            ("value", result),
        ]))
    }
    fn source_refs(&self, id: &str, seen: &mut HashSet<String>) -> RuleResult<Vec<JsonValue>> {
        if !seen.insert(id.to_owned()) {
            return Ok(vec![]);
        }
        let node = Self::node(&self.state, id)?;
        if let Some(mid) = word(node, "materialId") {
            let m = self.material(mid)?;
            if get(m, "demo") == Some(&JsonValue::Bool(true)) {
                return Ok(vec![]);
            }
            let refs = match get(m, "sourceRefs") {
                Some(JsonValue::Null) | None => &[][..],
                Some(JsonValue::Array(items)) => items.as_slice(),
                _ => return fail("library sourceRefs must be an array"),
            };
            return Ok(refs
                .iter()
                .map(|s| get(s, "ref").cloned().unwrap_or(JsonValue::Null))
                .collect());
        }
        let mut refs = vec![];
        let mut unique = HashSet::new();
        for source in array(node, "sourceIds").unwrap() {
            for r in self.source_refs(source.as_str().unwrap(), seen)? {
                let key = emit(&r)?;
                if unique.insert(key) {
                    refs.push(r)
                }
            }
        }
        Ok(refs)
    }
    fn local(v: Option<&JsonValue>) -> String {
        let Some(v) = v else { return String::new() };
        if let Some(s) = v.as_str() {
            return s.to_owned();
        }
        word(v, "ru")
            .or_else(|| word(v, "en"))
            .unwrap_or("")
            .to_owned()
    }
    fn research_plan(&self) -> RuleResult<Vec<u8>> {
        let nodes = array(&self.state, "nodes").unwrap();
        let relations = array(&self.state, "edges")
            .unwrap()
            .iter()
            .filter(|e| word(e, "origin") == Some("draft"))
            .collect::<Vec<_>>();
        let drafts = nodes
            .iter()
            .filter(|n| {
                word(n, "materialId").is_none_or(|id| {
                    get(&self.materials[id], "demo") == Some(&JsonValue::Bool(true))
                })
            })
            .count();
        if drafts + relations.len() > 256 {
            return fail(
                "research export supports at most 256 draft nodes and relations combined; use the constructor packet for this larger tree",
            );
        }
        let mut actions = vec![];
        for node in nodes {
            let id = word(node, "id").unwrap();
            let material = word(node, "materialId").map(|mid| &self.materials[mid]);
            let mockup = material.is_some_and(|m| get(m, "demo") == Some(&JsonValue::Bool(true)));
            if material.is_none() || mockup {
                let title = if mockup {
                    Self::local(material.and_then(|m| get(m, "title")))
                } else {
                    word(node, "title").unwrap().to_owned()
                };
                let body = if mockup {
                    Self::local(material.and_then(|m| get(m, "body")))
                } else {
                    word(node, "body").unwrap().to_owned()
                };
                actions.push(obj(vec![
                    ("kind", string("hypothesis")),
                    (
                        "input",
                        obj(vec![
                            ("id", string(id)),
                            ("title", string(&title)),
                            ("body", string(if body.is_empty() { &title } else { &body })),
                            ("targetId", string(id)),
                        ]),
                    ),
                ]));
            }
            if material.is_some() || !array(node, "sourceIds").unwrap().is_empty() {
                let mut fields = vec![("constructorNodeId", string(id))];
                if let Some(mid) = word(node, "materialId") {
                    fields.push(("libraryMaterialId", string(mid)));
                    if mockup {
                        fields.extend([
                            ("mockupMaterial", JsonValue::Bool(true)),
                            ("source", JsonValue::Bool(false)),
                            ("reviewed", JsonValue::Bool(false)),
                            ("canon", JsonValue::Bool(false)),
                        ]);
                    }
                }
                fields.push(("sourceNodeIds", get(node, "sourceIds").unwrap().clone()));
                fields.push((
                    "librarySourceRefs",
                    JsonValue::Array(self.source_refs(id, &mut HashSet::new())?),
                ));
                let body = String::from_utf8(emit(&obj(fields))?)
                    .map_err(|_| "research note encoding failed".to_owned())?;
                if body.encode_utf16().count() > 4_000 {
                    return fail(format!(
                        "research source note exceeds 4000 characters for {id}; use the constructor packet to retain all references"
                    ));
                }
                actions.push(obj(vec![
                    ("kind", string("note")),
                    (
                        "input",
                        obj(vec![
                            ("id", string(&format!("ref:{id}"))),
                            ("targetId", string(id)),
                            ("body", string(&body)),
                        ]),
                    ),
                ]));
            }
        }
        for edge in relations {
            let id = word(edge, "id").unwrap();
            let atlas = id
                .strip_prefix("atlas:")
                .and_then(|id| self.atlas_edges.get(id));
            let title = word(edge, "label")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    let s = Self::local(atlas.and_then(|a| get(a, "label")));
                    if s.is_empty() {
                        word(edge, "kind").unwrap().to_owned()
                    } else {
                        s
                    }
                });
            let body = Self::local(atlas.and_then(|a| get(a, "body")));
            let body = if body.is_empty() { title.clone() } else { body };
            actions.push(obj(vec![
                ("kind", string("hypothesis")),
                (
                    "input",
                    obj(vec![
                        ("id", string(&format!("relation:{id}"))),
                        ("title", string(&title)),
                        ("body", string(&body)),
                        ("fromId", get(edge, "from").unwrap().clone()),
                        ("toId", get(edge, "to").unwrap().clone()),
                        ("predicateLabel", get(edge, "kind").unwrap().clone()),
                    ]),
                ),
            ]));
        }
        emit(&obj(vec![("actions", JsonValue::Array(actions))]))
    }
}

#[cfg(feature = "wasm")]
mod browser {
    use super::*;
    use wasm_bindgen::prelude::*;
    fn js(e: String) -> JsValue {
        JsValue::from_str(&e)
    }
    #[wasm_bindgen]
    pub struct BrowserConstructorSession {
        machine: ConstructorMachine,
    }
    #[wasm_bindgen]
    impl BrowserConstructorSession {
        #[wasm_bindgen(constructor)]
        pub fn new(library: &[u8]) -> Result<Self, JsValue> {
            let value = parse(
                library,
                MAX_LIBRARY_BYTES,
                "constructor library schema is invalid",
            )
            .map_err(js)?;
            Ok(Self {
                machine: ConstructorMachine::new(value).map_err(js)?,
            })
        }
        pub fn import_saved(&mut self, packet: &str) -> Result<(), JsValue> {
            if packet.encode_utf16().count() * 2 > MAX_BYTES {
                return Err(js(
                    "constructor packet exceeds the 1 MB storage limit".to_owned()
                ));
            }
            let value = parse(
                packet.as_bytes(),
                MAX_BYTES,
                "constructor packet is not valid JSON",
            )
            .map_err(js)?;
            self.machine.validate(&value).map_err(js)?;
            self.machine.state = value;
            self.machine.undo.clear();
            self.machine.redo.clear();
            Ok(())
        }
        pub fn apply(&mut self, command: &[u8]) -> Result<Vec<u8>, JsValue> {
            let value = parse(
                command,
                MAX_BYTES * 2,
                "constructor command is not valid JSON",
            )
            .map_err(js)?;
            let result = self.machine.command(&value).map_err(js)?;
            emit(&result).map_err(js)
        }
        pub fn state_packet(&self) -> Result<Vec<u8>, JsValue> {
            bounded(&self.machine.state).map_err(js)
        }
        pub fn export_packet(&self) -> Result<Vec<u8>, JsValue> {
            bounded(&self.machine.state).map_err(js)
        }
        pub fn research_plan(&self) -> Result<Vec<u8>, JsValue> {
            self.machine.research_plan().map_err(js)
        }
        pub fn can_undo(&self) -> bool {
            !self.machine.undo.is_empty()
        }
        pub fn can_redo(&self) -> bool {
            !self.machine.redo.is_empty()
        }
    }
}
