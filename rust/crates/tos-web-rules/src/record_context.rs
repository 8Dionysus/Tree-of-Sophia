//! One declared-context operation over held JavaScript values. Source values
//! retain their identity until the host's structured clone; no JSON round trip.
use crate::browser_value::{self as js, *};
use js_sys::JsString;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;

fn tokens(pointer: &JsValue) -> Option<Vec<Vec<u16>>> {
    if !pointer.is_string() {
        return None;
    }
    let text: &JsString = pointer.unchecked_ref();
    let units: Vec<u16> = (0..text.length())
        .map(|i| text.char_code_at(i) as u16)
        .collect();
    if units.is_empty() {
        return Some(Vec::new());
    }
    if units[0] != 47 {
        return None;
    }
    let mut result = Vec::new();
    for part in units[1..].split(|u| *u == 47) {
        let mut token = Vec::new();
        let mut at = 0;
        while at < part.len() {
            if part[at] == 126 {
                at += 1;
                token.push(match part.get(at)? {
                    48 => 126,
                    49 => 47,
                    _ => return None,
                });
            } else {
                token.push(part[at]);
            }
            at += 1;
        }
        result.push(token);
    }
    Some(result)
}

fn key(units: &[u16]) -> JsValue {
    // Keep native call arguments finite without changing accepted key length.
    let mut value = JsString::from("");
    for chunk in units.chunks(8192) {
        value = value.concat(&JsString::from_char_code(chunk));
    }
    value.into()
}

fn resolve(raw: &JsValue, pointer: &JsValue) -> Result<Option<JsValue>, JsValue> {
    let Some(tokens) = tokens(pointer) else {
        return Ok(None);
    };
    let mut value = raw.clone();
    for token in tokens {
        let key = key(&token);
        if !value.is_object() || !has_own(&value, &key)? {
            return Ok(None);
        }
        if is_array(&value)?
            && !(token == [48]
                || !token.is_empty()
                    && (49..=57).contains(&token[0])
                    && token[1..].iter().all(|u| (48..=57).contains(u)))
        {
            return Ok(None);
        }
        value = get(&value, &key)?;
    }
    if value.is_undefined() {
        Ok(None)
    } else {
        Ok(Some(cloned(&value)?))
    }
}

#[wasm_bindgen]
pub fn essential_context_v1(raw: &JsValue) -> Result<JsValue, JsValue> {
    let selection = optional(raw, "display_selection")?;
    let declared = selection.is_truthy()
        && has_own(&selection, &JsValue::from_str("essential_context_pointers"))?;
    let schema_valid =
        optional(&selection, "schema_version")? == JsValue::from_str("tos_display_selection_v1");
    let revision_same =
        optional(&selection, "content_revision")? == optional(raw, "content_revision")?;
    let pointers_array = is_array(&optional(&selection, "essential_context_pointers")?)?;
    if !declared {
        return empty_items("not-declared");
    }
    if !schema_valid || !revision_same || !pointers_array {
        return empty_items("unavailable");
    }
    let incomplete = Rc::new(Cell::new(false));
    let flag = Rc::clone(&incomplete);
    let raw = raw.clone();
    let callback =
        Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(move |pointer: JsValue| {
            if let Some(value) = resolve(&raw, &pointer)? {
                object(&[
                    ("pointer", &pointer),
                    ("state", &JsValue::from_str("available")),
                    ("value", &value),
                ])
            } else {
                flag.set(true);
                object(&[
                    ("pointer", &pointer),
                    ("state", &JsValue::from_str("unavailable")),
                ])
            }
        })
        .into_js_value();
    // Use the selected native map semantics, including sparse declarations.
    let items = call(
        &field(&selection, "essential_context_pointers")?,
        "map",
        &callback,
    )?;
    js::object(&[
        (
            "state",
            &JsValue::from_str(if incomplete.get() {
                "incomplete"
            } else {
                "available"
            }),
        ),
        ("items", &items),
    ])
}
