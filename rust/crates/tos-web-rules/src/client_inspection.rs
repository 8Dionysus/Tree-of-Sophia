//! Whole browser inspection and item contracts over the original held values.
use crate::browser_value::*;
use js_sys::{Function, Set};
use wasm_bindgen::prelude::*;

thread_local! {
    static INVALID_REF: JsValue = Closure::<dyn Fn(JsValue) -> bool>::new(|value: JsValue| {
        !value.is_string() || !value.is_truthy()
    }).into_js_value();
}

pub(crate) fn check_items(items: &JsValue, node: bool, localized: &Function) -> Checked<Set> {
    require(is_array(items)?, "items")?;
    let ids = Set::new(&JsValue::UNDEFINED);
    for_each(items, |item| {
        require(item.is_truthy(), "item")?;
        require(field(&item, "id")?.is_string(), "item")?;
        require(field(&item, "id")?.is_truthy(), "item")?;
        require(
            !call(ids.as_ref(), "has", &field(&item, "id")?)?.is_truthy(),
            "item",
        )?;
        require(field(&item, "display")?.is_truthy(), "item")?;
        let label = field(
            &field(&item, "display")?,
            if node { "title" } else { "label" },
        )?;
        require(
            localized.call1(&JsValue::UNDEFINED, &label)?.is_truthy(),
            "item",
        )?;
        require(
            digest(&or_empty(field(&item, "content_revision")?), false)?,
            "item",
        )?;
        require(is_array(&field(&item, "source_refs")?)?, "item")?;
        require(
            field(&field(&item, "source_refs")?, "length")?.is_truthy(),
            "item",
        )?;
        let invalid =
            INVALID_REF.with(|callback| call(&field(&item, "source_refs")?, "some", callback))?;
        require(!invalid.is_truthy(), "item")?;
        call(ids.as_ref(), "add", &field(&item, "id")?)?;
        Ok(())
    })?;
    Ok(ids)
}

#[wasm_bindgen]
pub fn validate_client_inspection_v1(
    packet: &JsValue,
    kind: &JsValue,
    id: &JsValue,
    expected: &JsValue,
    content_revision: &JsValue,
    localized: &Function,
) -> Result<JsValue, JsValue> {
    finish(inspect(
        packet,
        kind,
        id,
        expected,
        content_revision,
        localized,
    ))
}

fn inspect(
    packet: &JsValue,
    kind: &JsValue,
    id: &JsValue,
    expected: &JsValue,
    content_revision: &JsValue,
    localized: &Function,
) -> Checked<JsValue> {
    require(
        digest(&or_empty(optional(packet, "source_revision")?), false)?,
        "missing_revision",
    )?;
    if expected.is_truthy() {
        require(field(packet, "source_revision")? == *expected, "revision")?;
    }
    let node = *kind == JsValue::from_str("node");
    require(
        field(packet, "schema")?
            == JsValue::from_str(if node {
                "tos_knowledge_node_packet_v1"
            } else {
                "tos_knowledge_relation_packet_v1"
            }),
        "schema",
    )?;
    check_items(&field(packet, "matches")?, node, localized)?;
    let id = id.clone();
    let predicate =
        Closure::<dyn Fn(JsValue) -> Result<bool, JsValue>>::new(move |value: JsValue| {
            Ok(field(&value, "id")? == id)
        })
        .into_js_value();
    let selected = call(&field(packet, "matches")?, "find", &predicate)?;
    require(selected.is_truthy(), "match")?;
    if content_revision.is_truthy() {
        require(
            field(&selected, "content_revision")? == *content_revision,
            "revision",
        )?;
    }
    if *kind == JsValue::from_str("relation") {
        let endpoints = check_items(&field(packet, "endpoints")?, true, localized)?;
        require(
            call(endpoints.as_ref(), "has", &field(&selected, "from_id")?)?.is_truthy(),
            "endpoints",
        )?;
        require(
            call(endpoints.as_ref(), "has", &field(&selected, "to_id")?)?.is_truthy(),
            "endpoints",
        )?;
    }
    Ok(object(&[("match", &selected)])?)
}
