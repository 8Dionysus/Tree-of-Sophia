//! Whole browser packet contracts. Held values preserve native collection,
//! coercion, callback and presentation behavior without an observation protocol.
use crate::{browser_value::*, client_inspection::check_items};
use js_sys::{Array, Function};
use wasm_bindgen::prelude::*;

fn text(value: &JsValue, allowed: &[&str], code: &'static str) -> Checked<()> {
    require(
        allowed.iter().any(|text| *value == JsValue::from_str(text)),
        code,
    )
}
fn integer(value: &JsValue, safe: bool) -> bool {
    value.as_f64().is_some_and(|value| {
        value.is_finite() && value.fract() == 0.0 && (!safe || value.abs() <= 9007199254740991.0)
    })
}
fn member(index: &JsValue, value: &JsValue) -> Result<bool, JsValue> {
    Ok(call(index, "has", value)?.is_truthy())
}
fn localizer(host: &JsValue) -> Result<Function, JsValue> {
    field(host, "localized")?
        .dyn_into()
        .map_err(|_| js_sys::TypeError::new("Missing localizer").into())
}
fn revision(packet: &JsValue, expected: &JsValue) -> Checked<()> {
    require(
        digest(&or_empty(optional(packet, "source_revision")?), false)?,
        "missing_revision",
    )?;
    if expected.is_truthy() {
        require(field(packet, "source_revision")? == *expected, "revision")?;
    }
    Ok(())
}
fn area(packet: &JsValue, v2: bool, host: &JsValue) -> Checked<JsValue> {
    for name in ["is_source", "is_canon", "writes_to_tree"] {
        require(
            optional(&field(packet, "authority_boundary")?, name)?.as_bool() == Some(false),
            "area",
        )?;
    }
    require(is_array(&field(packet, "nodes")?)?, "budget")?;
    require(is_array(&field(packet, "relations")?)?, "budget")?;
    require(
        !(number(&field(&field(packet, "nodes")?, "length")?)? > if v2 { 302.0 } else { 40.0 }),
        "budget",
    )?;
    require(
        !(number(&field(&field(packet, "relations")?, "length")?)? > if v2 { 101.0 } else { 80.0 }),
        "budget",
    )?;
    let localized = localizer(host)?;
    let ids: JsValue = check_items(&field(packet, "nodes")?, true, &localized)?.into();
    check_items(&field(packet, "relations")?, false, &localized)?;
    let area_ids = ids.clone();
    let predicate =
        Closure::<dyn Fn(JsValue) -> Result<bool, JsValue>>::new(move |relation: JsValue| {
            if !member(&area_ids, &field(&relation, "from_id")?)? {
                return Ok(true);
            }
            Ok(!member(&area_ids, &field(&relation, "to_id")?)?)
        })
        .into_js_value();
    require(
        !call(&field(packet, "relations")?, "some", &predicate)?.is_truthy(),
        "endpoints",
    )?;
    if field(packet, "focus")?.is_truthy() {
        require(
            member(&ids, &field(&field(packet, "focus")?, "node_id")?)?,
            "focus",
        )?;
    }
    Ok(ids)
}
fn item_map(items: &JsValue) -> Result<JsValue, JsValue> {
    let predicate = Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(|item: JsValue| {
        Ok(Array::of2(&field(&item, "id")?, &item).into())
    })
    .into_js_value();
    collection("Map", &call(items, "map", &predicate)?)
}
fn item_ids(items: &JsValue) -> Result<JsValue, JsValue> {
    let predicate = Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(|item: JsValue| {
        field(&item, "id")
    })
    .into_js_value();
    collection("Set", &call(items, "map", &predicate)?)
}
fn missing_members(values: &JsValue, items: &JsValue) -> Result<bool, JsValue> {
    let items = items.clone();
    let predicate = Closure::<dyn Fn(JsValue) -> Result<bool, JsValue>>::new(move |id: JsValue| {
        Ok(!member(&items, &id)?)
    })
    .into_js_value();
    Ok(call(values, "some", &predicate)?.is_truthy())
}
fn v1_partition_ids(page: &JsValue) -> Checked<JsValue> {
    let result = Array::new();
    for name in ["primary_node_ids", "context_node_ids"] {
        for_each(&field(page, name)?, |id| {
            result.push(&id);
            Ok(())
        })?;
    }
    Ok(result.into())
}
fn partition(primary: &JsValue, context: &JsValue, items: &JsValue, host: &JsValue) -> Checked<()> {
    require(is_array(primary)?, "exploration")?;
    require(is_array(context)?, "exploration")?;
    require(
        invoke(
            host,
            "add",
            &[&field(primary, "length")?, &field(context, "length")?],
        )? == field(items, "size")?,
        "exploration",
    )?;
    let combined = spread(&[primary, context])?;
    require(
        field(&collection("Set", &combined)?, "size")? == field(items, "size")?,
        "exploration",
    )?;
    require(
        !missing_members(&spread(&[primary, context])?, items)?,
        "exploration",
    )
}
fn prior_page(page: &JsValue, previous: &JsValue, host: &JsValue) -> Checked<()> {
    require(
        field(page, "number")?
            == invoke(
                host,
                "add",
                &[
                    &field(&field(previous, "page")?, "number")?,
                    &JsValue::from_f64(1.0),
                ],
            )?,
        "revision",
    )
}
fn exploration(packet: &JsValue, v2: bool, previous: &JsValue, host: &JsValue) -> Checked<()> {
    let (nodes, relations, origin, page, query, ids) = if v2 {
        let nodes = item_map(&field(packet, "nodes")?)?;
        let relations = item_map(&field(packet, "relations")?)?;
        let origin = field(packet, "origin")?;
        let page = field(packet, "page")?;
        let query = field(packet, "query")?;
        (nodes, relations, origin, page, query, JsValue::UNDEFINED)
    } else {
        let page = field(packet, "page")?;
        let ids = item_ids(&field(packet, "nodes")?)?;
        text(
            &field(packet, "schema")?,
            &["tos_exploration_result_v1"],
            "exploration",
        )?;
        (
            JsValue::UNDEFINED,
            JsValue::UNDEFINED,
            JsValue::UNDEFINED,
            page,
            JsValue::UNDEFINED,
            ids,
        )
    };
    require(
        field(packet, "writes_to_tree")?.as_bool() == Some(false),
        "exploration",
    )?;
    let snapshot = field(packet, "snapshot_revision")?;
    require(
        digest(&if v2 { snapshot } else { or_empty(snapshot) }, v2)?,
        "exploration",
    )?;
    if v2 {
        text(
            &field(packet, "execution_version")?,
            &[
                "tos-exploration-execution-v6",
                "tos-exploration-d1-execution-v6",
            ],
            "exploration",
        )?;
    }
    text(
        &field(packet, "status")?,
        &["paused", "limit_reached", "complete"],
        "exploration",
    )?;
    if v2 {
        require(origin.is_truthy(), "exploration")?;
        text(
            &field(&origin, "kind")?,
            &["node", "relation"],
            "exploration",
        )?;
        require(
            digest(&field(&origin, "content_revision")?, true)?,
            "exploration",
        )?;
        require(field(&origin, "id")?.is_string(), "exploration")?;
        require(field(&origin, "id")?.is_truthy(), "exploration")?;
        require(query.is_truthy(), "exploration")?;
        text(
            &field(&query, "schema_version")?,
            &["tos_exploration_request_v2"],
            "exploration",
        )?;
        require(
            field(&query, "source_revision")? == field(packet, "source_revision")?,
            "exploration",
        )?;
        require(field(&query, "origin")?.is_truthy(), "exploration")?;
        for name in ["kind", "id", "content_revision"] {
            require(
                field(&field(&query, "origin")?, name)? == field(&origin, name)?,
                "exploration",
            )?;
        }
        require(
            !has_own(packet, &JsValue::from_str("focus"))?,
            "exploration",
        )?;
    } else {
        require(field(packet, "focus")?.is_truthy(), "exploration")?;
    }
    require(integer(&optional(&page, "number")?, v2), "exploration")?;
    require(!(number(&field(&page, "number")?)? < 1.0), "exploration")?;
    text(
        &field(&page, "scope")?,
        &["resumable-neighborhood"],
        "exploration",
    )?;
    require(
        field(&page, "returned_nodes")?
            == if v2 {
                field(&nodes, "size")?
            } else {
                field(&field(packet, "nodes")?, "length")?
            },
        "exploration",
    )?;
    require(
        field(&page, "returned_relations")?
            == if v2 {
                field(&relations, "size")?
            } else {
                field(&field(packet, "relations")?, "length")?
            },
        "exploration",
    )?;
    if v2 {
        require(integer(&field(&page, "work_units")?, true), "exploration")?;
        require(
            !(number(&field(&page, "work_units")?)? < 0.0),
            "exploration",
        )?;
        require(
            !(number(&field(&page, "work_units")?)? > 512.0),
            "exploration",
        )?;
    } else {
        // V1 rereads the page arrays at each original observation.
        require(is_array(&field(&page, "primary_node_ids")?)?, "exploration")?;
        require(is_array(&field(&page, "context_node_ids")?)?, "exploration")?;
        require(
            invoke(
                host,
                "add",
                &[
                    &field(&field(&page, "primary_node_ids")?, "length")?,
                    &field(&field(&page, "context_node_ids")?, "length")?,
                ],
            )? == field(&ids, "size")?,
            "exploration",
        )?;
        require(
            field(&collection("Set", &v1_partition_ids(&page)?)?, "size")? == field(&ids, "size")?,
            "exploration",
        )?;
        require(
            !missing_members(&v1_partition_ids(&page)?, &ids)?,
            "exploration",
        )?;
    }
    text(
        &optional(&field(packet, "counts")?, "scope")?,
        &["cumulative-discovered-not-global-total"],
        "exploration",
    )?;
    text(
        &optional(&field(packet, "inclusion")?, "authority")?,
        &["query-execution-not-semantic-proof"],
        "exploration",
    )?;
    if field(packet, "status")? == JsValue::from_str("paused") {
        let cursor = field(&page, "next_cursor")?;
        require(
            digest(&if v2 { cursor } else { or_empty(cursor) }, v2)?,
            "exploration",
        )?;
    } else {
        require(field(&page, "next_cursor")?.is_null(), "exploration")?;
    }
    if v2 {
        if field(packet, "status")? == JsValue::from_str("limit_reached") {
            text(
                &field(packet, "limit_reason")?,
                &["session_nodes", "session_relations"],
                "exploration",
            )?;
        } else {
            require(field(packet, "limit_reason")?.is_null(), "exploration")?;
        }
        for (kind, items) in [("node", &nodes), ("relation", &relations)] {
            let primary = field(&page, &format!("primary_{kind}_ids"))?;
            let context = field(&page, &format!("context_{kind}_ids"))?;
            partition(&primary, &context, items, host)?;
            let inclusion = field(
                &field(packet, "inclusion")?,
                if kind == "node" { "nodes" } else { "relations" },
            )?;
            require(inclusion.is_truthy(), "exploration")?;
            require(
                JsValue::from_f64(keys(&inclusion)?.length() as f64) == field(items, "size")?,
                "exploration",
            )?;
            let selected = inclusion.clone();
            let predicate =
                Closure::<dyn Fn(JsValue) -> Result<bool, JsValue>>::new(move |id: JsValue| {
                    Ok(!has_own(&selected, &id)?)
                })
                .into_js_value();
            require(
                !call(&spread(&[&call0(items, "keys")?])?, "some", &predicate)?.is_truthy(),
                "exploration",
            )?;
        }
        let item = call(
            if field(&origin, "kind")? == JsValue::from_str("node") {
                &nodes
            } else {
                &relations
            },
            "get",
            &field(&origin, "id")?,
        )?;
        require(item.is_truthy(), "exploration")?;
        require(
            field(&item, "content_revision")? == field(&origin, "content_revision")?,
            "exploration",
        )?;
        let node_origin = field(&origin, "kind")? == JsValue::from_str("node");
        if node_origin {
            require(
                !has_own(&origin, &JsValue::from_str("endpoints"))?,
                "exploration",
            )?;
            require(
                !field(&field(&page, "context_relation_ids")?, "length")?.is_truthy(),
                "exploration",
            )?;
            require(
                call(
                    &field(&page, "context_node_ids")?,
                    "includes",
                    &field(&origin, "id")?,
                )?
                .is_truthy(),
                "exploration",
            )?;
        } else {
            require(
                field(&field(&page, "context_relation_ids")?, "length")? == JsValue::from_f64(1.0),
                "exploration",
            )?;
            require(
                field(&field(&page, "context_relation_ids")?, "0")? == field(&origin, "id")?,
                "exploration",
            )?;
        }
        text(
            &optional(
                &get(
                    &field(
                        &field(packet, "inclusion")?,
                        if node_origin { "nodes" } else { "relations" },
                    )?,
                    &field(&origin, "id")?,
                )?,
                "kind",
            )?,
            &["origin"],
            "exploration",
        )?;
        if !node_origin {
            for side in ["from", "to"] {
                let endpoint = optional(&field(&origin, "endpoints")?, side)?;
                let node = call(&nodes, "get", &field(&item, &format!("{side}_id"))?)?;
                require(endpoint.is_truthy(), "exploration")?;
                require(node.is_truthy(), "exploration")?;
                for (left, right) in [
                    ("node_id", "id"),
                    ("content_revision", "content_revision"),
                    ("entity_id", "entity_id"),
                ] {
                    require(
                        field(&endpoint, left)? == field(&node, right)?,
                        "exploration",
                    )?;
                }
                require(
                    call(
                        &field(&page, "context_node_ids")?,
                        "includes",
                        &field(&node, "id")?,
                    )?
                    .is_truthy(),
                    "exploration",
                )?;
                text(
                    &optional(
                        &get(
                            &field(&field(packet, "inclusion")?, "nodes")?,
                            &field(&node, "id")?,
                        )?,
                        "kind",
                    )?,
                    &["origin-endpoint"],
                    "exploration",
                )?;
            }
        }
    }
    if previous.is_truthy() {
        if v2 {
            require(
                field(previous, "schema")? == field(packet, "schema")?,
                "revision",
            )?;
            text(&field(previous, "status")?, &["paused"], "revision")?;
            require(
                field(packet, "source_revision")? == field(previous, "source_revision")?,
                "revision",
            )?;
        }
        require(
            field(packet, "snapshot_revision")? == field(previous, "snapshot_revision")?,
            "revision",
        )?;
        if v2 {
            require(
                field(packet, "execution_version")? == field(previous, "execution_version")?,
                "revision",
            )?;
        } else {
            require(
                field(&field(packet, "focus")?, "node_id")?
                    == field(&field(previous, "focus")?, "node_id")?,
                "revision",
            )?;
        }
        prior_page(&page, previous, host)?;
        if v2 {
            if field(packet, "status")? == JsValue::from_str("paused") {
                require(
                    field(&page, "next_cursor")?
                        != field(&field(previous, "page")?, "next_cursor")?,
                    "revision",
                )?;
            }
            for name in ["origin", "query"] {
                require(
                    invoke(
                        host,
                        "sameJson",
                        &[&field(packet, name)?, &field(previous, name)?],
                    )?
                    .is_truthy(),
                    "revision",
                )?;
            }
        } else {
            require(
                stringify(&field(packet, "query")?)? == stringify(&field(previous, "query")?)?,
                "revision",
            )?;
        }
    }
    if v2 {
        let focus = if field(&origin, "kind")? == JsValue::from_str("node") {
            field(&origin, "id")?
        } else {
            JsValue::NULL
        };
        // Only the existing scene construction boundary intentionally turns
        // presentation/source failures into an incomplete-page refusal.
        let scene = (|| -> Result<JsValue, JsValue> {
            let nodes = field(packet, "nodes")?;
            let relations = field(packet, "relations")?;
            let relation = if field(&origin, "kind")? == JsValue::from_str("relation") {
                field(&origin, "id")?
            } else {
                JsValue::NULL
            };
            invoke(host, "scene", &[&nodes, &relations, &focus, &relation])
        })()
        .map_err(|_| Failure::Contract("exploration"))?;
        require(
            invoke(host, "sameJson", &[&field(packet, "scene")?, &scene])?.is_truthy(),
            "exploration",
        )?;
    }
    Ok(())
}
fn search(packet: &JsValue, indexed: bool, options: &JsValue, host: &JsValue) -> Checked<()> {
    text(
        &field(packet, "schema")?,
        &[if indexed {
            "tos_knowledge_search_indexed_v2"
        } else {
            "tos_knowledge_search_compressed_v3"
        }],
        "search",
    )?;
    for kind in ["nodes", "relations"] {
        require(
            !(number(&optional(&field(packet, kind)?, "length")?)?
                > number(&field(options, "limit")?)?),
            "search",
        )?;
    }
    require(
        optional(&field(packet, "page")?, "cursor")? == field(options, "cursor")?,
        "search",
    )?;
    require(
        field(&field(packet, "page")?, "limit_per_kind")? == field(options, "limit")?,
        "search",
    )?;
    require(
        field(&field(packet, "page")?, "has_more")?
            .as_bool()
            .is_some(),
        "search",
    )?;
    if field(&field(packet, "page")?, "has_more")?.is_truthy() {
        require(
            field(&field(packet, "page")?, "next_cursor")?.is_string(),
            "search",
        )?;
        require(
            field(&field(packet, "page")?, "next_cursor")?.is_truthy(),
            "search",
        )?;
    } else {
        require(
            field(&field(packet, "page")?, "next_cursor")?.is_null(),
            "search",
        )?;
    }
    require(
        optional(&field(packet, "authority_boundary")?, "writes_to_tree")?.as_bool() == Some(false),
        "search",
    )?;
    let localized = localizer(host)?;
    check_items(&field(packet, "nodes")?, true, &localized)?;
    check_items(&field(packet, "relations")?, false, &localized)?;
    Ok(())
}
#[wasm_bindgen]
pub fn validate_client_packet_v1(
    packet: &JsValue,
    mode: u8,
    expected: &JsValue,
    previous: &JsValue,
    options: &JsValue,
    host: &JsValue,
) -> Result<JsValue, JsValue> {
    finish((|| {
        let indexed = field(options, "mode")? == JsValue::from_str("indexed");
        if mode == 2 {
            text(
                &optional(packet, "schema")?,
                &["tos_lens_result_v1"],
                "lens",
            )?;
        }
        let v2 = mode == 3
            && optional(packet, "schema")? == JsValue::from_str("tos_exploration_result_v2");
        revision(packet, expected)?;
        match mode {
            0 => (),
            1 | 2 | 3 => {
                area(packet, v2, host)?;
                if mode == 3 {
                    exploration(packet, v2, previous, host)?;
                }
            }
            5 => search(packet, indexed, options, host)?,
            _ => return Err(Failure::Contract("invalid_packet_mode")),
        }
        Ok(JsValue::NULL)
    })())
}
#[wasm_bindgen]
pub fn validate_client_search_request_v1(
    cursor: &JsValue,
    mode: &JsValue,
    limit: &JsValue,
) -> Result<JsValue, JsValue> {
    finish((|| {
        if !cursor.is_null() {
            require(cursor.is_string(), "search")?;
            require(number(&field(cursor, "length")?)? <= 65536.0, "search")?;
            require(mode.is_truthy(), "search")?;
        }
        require(integer(limit, true), "search")?;
        let value = number(limit)?;
        require((1.0..=6.0).contains(&value), "search")?;
        Ok(JsValue::NULL)
    })())
}
#[wasm_bindgen]
pub fn validate_client_exploration_request_v1(
    packet: &JsValue,
    query: &JsValue,
    previous: &JsValue,
    host: &JsValue,
) -> Result<JsValue, JsValue> {
    finish((|| {
        if !previous.is_truthy() {
            require(
                invoke(host, "requestMatches", &[&field(packet, "query")?, query])?.is_truthy(),
                "request_area",
            )?;
        }
        Ok(JsValue::NULL)
    })())
}
#[wasm_bindgen]
pub fn client_request_set_key_v1(value: &JsValue) -> bool {
    *value == JsValue::from_str("sources") || *value == JsValue::from_str("predicate_ids")
}

#[wasm_bindgen]
pub fn validate_client_material_request_v1(
    kind: &JsValue,
    id: &JsValue,
    language: &JsValue,
    relation: &JsValue,
    expected: &JsValue,
) -> Result<JsValue, JsValue> {
    finish((|| {
        text(kind, &["node", "relation"], "material_request")?;
        require(id.is_string(), "material_request")?;
        require(id.is_truthy(), "material_request")?;
        require(
            language
                .as_string()
                .is_some_and(|value| crate::human_forms::content_language_v1(&value)),
            "material_request",
        )?;
        let inspect = *kind == JsValue::from_str("relation")
            && (!relation.is_truthy() || field(relation, "id")? != *id || !expected.is_truthy());
        Ok(JsValue::from_bool(inspect))
    })())
}
#[wasm_bindgen]
pub fn validate_client_material_packet_v1(
    packet: &JsValue,
    kind: &JsValue,
    id: &JsValue,
    content_revision: &JsValue,
) -> Result<JsValue, JsValue> {
    finish((|| {
        let relation = *kind == JsValue::from_str("relation");
        let id_for_match = id.clone();
        let predicate =
            Closure::<dyn Fn(JsValue) -> Result<bool, JsValue>>::new(move |item: JsValue| {
                Ok(field(&item, "id")? == id_for_match)
            })
            .into_js_value();
        let selected = call(
            &field(packet, if relation { "relations" } else { "nodes" })?,
            "find",
            &predicate,
        )?;
        require(selected.is_truthy(), "match")?;
        if content_revision.is_truthy() {
            require(
                field(&selected, "content_revision")? == *content_revision,
                "revision",
            )?;
        }
        let values = if relation {
            Array::of2(&field(&selected, "from_id")?, &field(&selected, "to_id")?)
        } else {
            Array::of1(id)
        };
        let allowed = collection("Set", values.as_ref())?;
        require(
            field(&field(packet, "nodes")?, "length")? == field(&allowed, "size")?,
            "material_scope",
        )?;
        let predicate =
            Closure::<dyn Fn(JsValue) -> Result<bool, JsValue>>::new(move |item: JsValue| {
                Ok(!member(&allowed, &field(&item, "id")?)?)
            })
            .into_js_value();
        require(
            !call(&field(packet, "nodes")?, "some", &predicate)?.is_truthy(),
            "material_scope",
        )?;
        require(
            field(&field(packet, "relations")?, "length")?
                == JsValue::from_f64(if relation { 1.0 } else { 0.0 }),
            "material_scope",
        )?;
        Ok(object(&[("match", &selected)])?)
    })())
}
fn scope_items(
    items: &JsValue,
    closure: &JsValue,
    key: &'static str,
    code: &'static str,
) -> Checked<()> {
    require(
        field(items, "length")? == field(&field(closure, key)?, "length")?,
        code,
    )?;
    let closure = closure.clone();
    let predicate =
        Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(move |item: JsValue| {
            call(&field(&closure, key)?, "includes", &field(&item, "id")?)
        })
        .into_js_value();
    require(call(items, "every", &predicate)?.is_truthy(), code)
}
#[wasm_bindgen]
pub fn validate_client_claim_scope_v1(
    packet: &JsValue,
    closure: &JsValue,
) -> Result<JsValue, JsValue> {
    finish((|| {
        scope_items(
            &field(packet, "nodes")?,
            closure,
            "nodeIds",
            "material_scope",
        )?;
        scope_items(
            &field(packet, "relations")?,
            closure,
            "relationIds",
            "material_scope",
        )?;
        Ok(JsValue::NULL)
    })())
}
fn wrapped_ids(ids: &JsValue) -> Result<JsValue, JsValue> {
    let predicate = Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(|id: JsValue| {
        object(&[("id", &id)])
    })
    .into_js_value();
    call(ids, "map", &predicate)
}
#[wasm_bindgen]
pub fn validate_client_claim_path_v1(
    packet: &JsValue,
    reference: &JsValue,
    closure: &JsValue,
    host: &JsValue,
) -> Result<JsValue, JsValue> {
    finish((|| {
        let selected = invoke(
            host,
            "claimPathFor",
            &[packet, &field(reference, "claimId")?],
        )?;
        require(selected.is_truthy(), "form")?;
        let returned = invoke(host, "claimPathClosure", &[packet, &selected])?;
        require(
            field(&selected, "id")? == field(reference, "pathId")?,
            "form",
        )?;
        require(
            field(&selected, "relation_type_id")? == field(reference, "relationType")?,
            "form",
        )?;
        require(
            stringify(&field(&selected, "node_ids")?)? == stringify(&field(reference, "nodeIds")?)?,
            "form",
        )?;
        require(
            stringify(&field(&selected, "relation_ids")?)?
                == stringify(&field(reference, "relationIds")?)?,
            "form",
        )?;
        scope_items(
            &wrapped_ids(&field(&returned, "nodeIds")?)?,
            closure,
            "nodeIds",
            "form",
        )?;
        scope_items(
            &wrapped_ids(&field(&returned, "relationIds")?)?,
            closure,
            "relationIds",
            "form",
        )?;
        Ok(object(&[("selected", &selected), ("returned", &returned)])?)
    })())
}
