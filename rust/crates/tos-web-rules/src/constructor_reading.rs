//! Mechanical binding of authored constructor readings; this never assesses meaning.
use std::collections::HashSet;
use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue as V, emit_value_preserved_json, parse_json,
};
type R<T> = Result<T, String>;
fn err<T>(s: impl Into<String>) -> R<T> {
    Err(s.into())
}
fn get<'a>(v: &'a V, k: &str) -> &'a V {
    v.object_get(k).unwrap_or(&V::Null)
}
fn text(v: &V) -> &str {
    v.as_str().unwrap_or("")
}
fn word<'a>(v: &'a V, k: &str) -> &'a str {
    text(get(v, k))
}
fn arr(v: &V) -> &[V] {
    v.as_array().unwrap_or(&[])
}
fn fields(v: &V) -> Vec<(String, V)> {
    v.as_object()
        .map(|a| {
            a.iter()
                .map(|(k, v)| (k.as_str().unwrap_or("").into(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}
fn s(v: impl AsRef<str>) -> V {
    V::String(JsonString::from_utf8(v.as_ref()))
}
fn object(a: Vec<(String, V)>) -> V {
    V::Object(
        a.into_iter()
            .map(|(k, v)| (JsonString::from_utf8(&k), v))
            .collect(),
    )
}
fn set(v: &mut V, k: &str, a: V) {
    let mut f = fields(v);
    if let Some((_, v)) = f.iter_mut().find(|(n, _)| n == k) {
        *v = a
    } else {
        f.push((k.into(), a))
    }
    *v = object(f);
}
fn truthy(v: &V) -> bool {
    match v {
        V::Null | V::Bool(false) => false,
        V::String(s) => s.as_str().is_some_and(|s| !s.is_empty()),
        V::Number(n) => n.lexeme.parse::<f64>().is_ok_and(|n| n != 0.0),
        _ => true,
    }
}
fn bilingual(v: &V, label: &str) -> R<()> {
    for c in ["ru", "en"] {
        if word(v, c).trim().is_empty() {
            return err(format!("Missing {c} inquiry: {label}"));
        }
    }
    Ok(())
}
fn source<'a>(refs: &'a V, id: &str) -> R<&'a V> {
    refs.object_get(id)
        .ok_or_else(|| format!("Unknown reading source: {id}"))
}
fn grounds(v: &V, label: &str, refs: &V) -> R<()> {
    let a = arr(v);
    if a.is_empty() {
        return err(format!("Missing textual grounds: {label}"));
    }
    let mut seen = HashSet::new();
    for g in a {
        let id = get(g, "ref")
            .as_str()
            .ok_or_else(|| format!("Invalid or duplicate textual ground: {label}"))?;
        if !seen.insert(id) {
            return err(format!("Invalid or duplicate textual ground: {label}"));
        }
        source(refs, id)?;
        bilingual(get(g, "focus"), &format!("{label} source focus"))?;
    }
    Ok(())
}
fn coverage(actual: Vec<String>, expected: &V, label: &str) -> R<()> {
    let a = arr(expected);
    let ids: HashSet<_> = a.iter().map(text).collect();
    if ids.len() != a.len()
        || actual.len() != ids.len()
        || actual.iter().any(|id| !ids.contains(id.as_str()))
    {
        return err(format!("Inquiry coverage differs from prepared {label}"));
    }
    Ok(())
}
fn anchors(item: &V, refs: &V) -> R<V> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for g in arr(get(item, "grounds"))
        .iter()
        .chain(arr(get(get(item, "counterReading"), "grounds")))
    {
        let id = word(source(refs, word(g, "ref"))?, "passageId");
        if !id.is_empty() && seen.insert(id) {
            result.push(object(vec![
                ("passageId".into(), s(id)),
                ("focus".into(), get(g, "focus").clone()),
            ]));
        }
    }
    Ok(V::Array(result))
}
fn bind(req: &V, refs: &V) -> R<V> {
    let mut nodes = fields(get(req, "sources"));
    nodes.extend(fields(get(req, "readings")));
    coverage(
        nodes.iter().map(|(id, _)| id.clone()).collect(),
        get(req, "materialIds"),
        "materials",
    )?;
    let ids: HashSet<_> = nodes.iter().map(|(id, _)| id).collect();
    if ids.len() != nodes.len() {
        return err("Duplicate authored material inquiry");
    }
    let relations = fields(get(req, "relations"));
    coverage(
        relations.iter().map(|(id, _)| id.clone()).collect(),
        get(req, "edgeIds"),
        "relations",
    )?;
    for (id, item) in &mut nodes {
        bilingual(get(item, "argument"), &format!("{id} argument"))?;
        grounds(get(item, "grounds"), id, refs)?;
        if item.object_get("counterpoint").is_some() {
            return err(format!("Unattributed counterpoint is not a reading: {id}"));
        }
        if truthy(get(item, "question")) {
            bilingual(get(item, "question"), &format!("{id} question"))?
        }
        if truthy(get(item, "experiment")) {
            for k in ["setup", "question"] {
                bilingual(
                    get(get(item, "experiment"), k),
                    &format!("{id} experiment {k}"),
                )?
            }
        }
        if truthy(get(item, "counterReading")) {
            bilingual(
                get(get(item, "counterReading"), "text"),
                &format!("{id} alternative reading"),
            )?;
            grounds(
                get(get(item, "counterReading"), "grounds"),
                &format!("{id} alternative reading"),
                refs,
            )?
        }
        let bound_anchors = anchors(item, refs)?;
        set(item, "anchors", bound_anchors);
    }
    for (id, item) in &relations {
        bilingual(get(item, "warrant"), &format!("{id} warrant"))?;
        grounds(get(item, "grounds"), id, refs)?;
        if item.object_get("challenge").is_some() {
            return err(format!("Unattributed relation challenge: {id}"));
        }
        for k in ["limit", "question"] {
            if truthy(get(item, k)) {
                bilingual(get(item, k), &format!("{id} {k}"))?
            }
        }
    }
    Ok(object(vec![
        ("nodes".into(), object(nodes)),
        ("relations".into(), object(relations)),
    ]))
}
fn routes(req: &V, refs: &V) -> R<V> {
    let materials: HashSet<_> = arr(get(get(req, "library"), "nodes"))
        .iter()
        .map(|v| word(v, "id"))
        .collect();
    let edges = arr(get(get(get(req, "library"), "atlas"), "edges"));
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in arr(get(req, "routes")) {
        let id = word(r, "id");
        if id.is_empty()
            || !seen.insert(id)
            || ["title", "question", "description", "conclusion"]
                .iter()
                .any(|k| bilingual(get(r, k), "").is_err())
        {
            return err("Invalid research route identity or wording");
        }
        grounds(
            get(r, "grounds"),
            &format!("{id} introduction and conclusion"),
            refs,
        )?;
        if truthy(get(r, "investigation"))
            && ["startingPoint", "stakes", "carryForward"]
                .iter()
                .any(|k| bilingual(get(get(r, "investigation"), k), "").is_err())
        {
            return err(format!("Route {id} has incomplete investigation guidance"));
        }
        let steps = arr(get(r, "steps"));
        let transitions = arr(get(r, "transitions"));
        if steps.len() < 2 || steps.len() > 20 || transitions.len() != steps.len() - 1 {
            return err(format!("Route {id} has no complete sequence"));
        }
        let mut bound = Vec::new();
        for step in steps {
            let nid = word(step, "nodeId");
            if !materials.contains(nid)
                || bilingual(get(step, "title"), "").is_err()
                || bilingual(get(step, "body"), "").is_err()
            {
                return err(format!("Route {id} has an invalid step"));
            }
            grounds(get(step, "grounds"), &format!("{id} {nid}"), refs)?;
            let mut step = step.clone();
            set(&mut step, "graphNodeId", s(format!("material:{nid}")));
            bound.push(step);
        }
        let mut joined = Vec::new();
        for (i, t) in transitions.iter().enumerate() {
            let from = word(&steps[i], "nodeId");
            let to = word(&steps[i + 1], "nodeId");
            let Some(edge) = edges.iter().find(|e| word(e, "id") == word(t, "edgeId")) else {
                return err(format!("Route {id} has an ungrounded transition"));
            };
            if word(t, "from") != from
                || word(t, "to") != to
                || bilingual(get(t, "body"), "").is_err()
            {
                return err(format!("Route {id} has an ungrounded transition"));
            }
            let forward = word(edge, "from") == from && word(edge, "to") == to;
            if !forward && !(word(edge, "to") == from && word(edge, "from") == to) {
                return err(format!(
                    "Route {id} transition does not follow its relation"
                ));
            }
            let mut t = t.clone();
            set(
                &mut t,
                "graphEdgeId",
                s(format!("atlas:{}", word(edge, "id"))),
            );
            set(
                &mut t,
                "direction",
                s(if forward { "forward" } else { "reverse" }),
            );
            joined.push(t);
        }
        let mut r = r.clone();
        set(&mut r, "steps", V::Array(bound));
        set(&mut r, "transitions", V::Array(joined));
        out.push(r);
    }
    Ok(V::Array(out))
}
fn contexts(req: &V, refs: &V) -> R<V> {
    let mut out: Vec<(String, V)> = arr(get(req, "nodes"))
        .iter()
        .map(|node| {
            let contexts = arr(get(get(node, "inquiry"), "anchors"))
                .iter()
                .map(|a| (word(a, "passageId").into(), get(a, "focus").clone()))
                .collect();
            (word(node, "id").into(), object(contexts))
        })
        .collect();
    let mut add = |id: &str, gs: &V| -> R<()> {
        let (_, contexts) = out
            .iter_mut()
            .find(|(n, _)| n == id)
            .ok_or_else(|| format!("Unknown reading context: {id}"))?;
        for g in arr(gs) {
            let sid = word(source(refs, word(g, "ref"))?, "passageId");
            if !sid.is_empty() && contexts.object_get(sid).is_none() {
                set(contexts, sid, get(g, "focus").clone());
            }
        }
        Ok(())
    };
    for r in arr(get(req, "routes")) {
        let steps = arr(get(r, "steps"));
        for st in steps {
            add(word(st, "nodeId"), get(st, "grounds"))?
        }
        let last = steps
            .last()
            .ok_or_else(|| "Missing route steps".to_owned())?;
        add(word(last, "nodeId"), get(r, "grounds"))?;
    }
    Ok(object(out))
}
fn valid(point: &V, routes: &V) -> bool {
    let Some(r) = arr(routes)
        .iter()
        .find(|r| word(r, "id") == word(point, "routeId"))
    else {
        return false;
    };
    let count = arr(get(r, "steps")).len();
    let Some(n) = get(point, "index")
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
    else {
        return false;
    };
    let finished = get(point, "finished");
    n < count && (finished == &V::Bool(false) || (finished == &V::Bool(true) && n + 1 == count))
}
pub fn constructor_reading_v1(raw: &[u8]) -> R<Vec<u8>> {
    let limits = JsonLimits {
        max_bytes: 16_000_000,
        ..JsonLimits::default()
    };
    let req = parse_json(raw, JsonMode::RequestLastWins, limits)
        .map_err(|e| e.to_string())?
        .into_root();
    let refs = get(&req, "references");
    let value = match word(&req, "operation") {
        "grounds" => {
            grounds(get(&req, "grounds"), word(&req, "label"), refs)?;
            V::Null
        }
        "bind" => bind(&req, refs)?,
        "routes" => routes(&req, refs)?,
        "textReferences" => {
            let passages = arr(get(get(&req, "catalog"), "passages"));
            for node in arr(get(&req, "nodes")) {
                for anchor in arr(get(get(node, "inquiry"), "anchors")) {
                    let id = word(anchor, "passageId");
                    if !passages
                        .iter()
                        .any(|p| word(p, "id") == id && word(p, "status") == "available")
                    {
                        return err(format!(
                            "Inquiry text reference is unavailable: {} → {id}",
                            word(node, "id")
                        ));
                    }
                }
            }
            V::Null
        }
        "contexts" => contexts(&req, refs)?,
        "groundContext" => {
            let r = get(&req, "route");
            let steps = arr(get(r, "steps"));
            let node = steps
                .iter()
                .find(|st| {
                    arr(get(st, "grounds"))
                        .iter()
                        .any(|g| word(g, "ref") == word(&req, "ref"))
                })
                .or(steps.last())
                .ok_or_else(|| "Missing route steps".to_owned())?;
            s(word(node, "nodeId"))
        }
        "graphInput" | "path" => {
            let r = get(&req, "route");
            let path = word(&req, "operation") == "path";
            let mut seen = HashSet::new();
            let ns = arr(get(r, "steps"))
                .iter()
                .map(|v| word(v, if path { "graphNodeId" } else { "nodeId" }))
                .filter(|s| seen.insert(s.to_string()))
                .map(s)
                .collect();
            seen.clear();
            let es = arr(get(r, "transitions"))
                .iter()
                .map(|v| word(v, if path { "graphEdgeId" } else { "edgeId" }))
                .filter(|s| seen.insert(s.to_string()))
                .map(s)
                .collect();
            let mut f = vec![
                ("nodeIds".into(), V::Array(ns)),
                ("edgeIds".into(), V::Array(es)),
            ];
            if path {
                f.push(("restrictEdges".into(), V::Bool(true)))
            }
            object(f)
        }
        "validPoint" => V::Bool(valid(get(&req, "point"), get(&req, "routes"))),
        _ => return err("Unknown constructor reading operation"),
    };
    emit_value_preserved_json(
        &value,
        JsonLimits {
            max_bytes: 16_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|e| e.to_string())
}
#[cfg(feature = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn constructor_reading_wasm_v1(raw: &[u8]) -> Result<Vec<u8>, wasm_bindgen::JsValue> {
    constructor_reading_v1(raw).map_err(|e| wasm_bindgen::JsValue::from_str(&e))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_exact_catalog_and_rejects_link_only() {
        let ok=br#"{"operation":"textReferences","nodes":[{"id":"a","inquiry":{"anchors":[{"passageId":"one"}]}}],"catalog":{"passages":[{"id":"one","status":"available"}]}}"#;
        assert_eq!(constructor_reading_v1(ok).unwrap(), b"null");
        let bad = String::from_utf8(ok.to_vec())
            .unwrap()
            .replace("available", "link-only");
        assert!(
            constructor_reading_v1(bad.as_bytes())
                .unwrap_err()
                .contains("unavailable: a → one")
        );
    }
    #[test]
    fn return_point_bounds_and_finished_step_are_native() {
        let base = r#"{"operation":"validPoint","routes":[{"id":"a","steps":[{},{}]}],"point":{"routeId":"a","index":0,"finished":true}}"#;
        assert_eq!(constructor_reading_v1(base.as_bytes()).unwrap(), b"false");
        assert_eq!(
            constructor_reading_v1(base.replace("\"index\":0", "\"index\":1").as_bytes()).unwrap(),
            b"true"
        );
        assert_eq!(
            constructor_reading_v1(base.replace("\"index\":0", "\"index\":2").as_bytes()).unwrap(),
            b"false"
        );
    }
    #[test]
    fn source_reference_must_be_authored_and_bilingual() {
        let req = r#"{"operation":"grounds","label":"stop","grounds":[{"ref":"one","focus":{"ru":"Источник","en":"Source"}}],"references":{"one":{"passageId":"exact"}}}"#;
        assert!(constructor_reading_v1(req.as_bytes()).is_ok());
        let bad = req
            .to_owned()
            .replace("\"references\":{\"one\"", "\"references\":{\"two\"");
        assert_eq!(
            constructor_reading_v1(bad.as_bytes()).unwrap_err(),
            "Unknown reading source: one"
        );
    }
}
