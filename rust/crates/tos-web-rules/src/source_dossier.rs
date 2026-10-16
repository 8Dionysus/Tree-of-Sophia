//! Whole metadata dossier validation; source identity and JavaScript callback
//! semantics remain intact. This operation grants no rights or source admission.
use crate::browser_value::*;
use js_sys::JsString;
use wasm_bindgen::prelude::*;

fn reference(value: &JsValue) -> bool {
    if !value.is_string() {
        return false;
    }
    let text: &JsString = value.unchecked_ref();
    if text.length() == 0 || text.length() > 2048 {
        return false;
    }
    let Some(value) = value.as_string() else {
        return false;
    };
    let Some(tail) = value.strip_prefix("tos.") else {
        return false;
    };
    let mut part = false;
    for byte in tail.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            part = true;
        } else if matches!(byte, b'.' | b'-') && part {
            part = false;
        } else {
            return false;
        }
    }
    part
}
fn string_element(value: JsValue) -> bool {
    value.is_string() && {
        let text: &JsString = value.unchecked_ref();
        text.length() > 0 && text.length() <= 2048
    }
}
fn record_element(value: JsValue) -> Result<bool, JsValue> {
    if !value.is_object() {
        return Ok(false);
    }
    Ok(!is_array(&value)?)
}
thread_local! {
    // Selected arrays may retain their predicate. These stateless callbacks
    // belong to the binding instance, rather than a freed packet session.
    static STRING_ELEMENT: JsValue = Closure::<dyn Fn(JsValue) -> bool>::new(string_element).into_js_value();
    static RECORD_ELEMENT: JsValue = Closure::<dyn Fn(JsValue) -> Result<bool, JsValue>>::new(record_element).into_js_value();
}
fn record_array(value: &JsValue) -> Result<bool, JsValue> {
    RECORD_ELEMENT
        .with(|callback| call(value, "every", callback))
        .map(|value| value.is_truthy())
}
fn refused(code: &str) -> Result<JsValue, JsValue> {
    Ok(JsValue::from_str(code))
}

#[wasm_bindgen]
pub fn source_dossier_reference_v1(value: &JsValue) -> bool {
    reference(value)
}

#[wasm_bindgen]
pub fn validate_source_dossier_request_v1(value: &JsValue, limit: &JsValue) -> bool {
    reference(value)
        && limit
            .as_f64()
            .is_some_and(|n| n.is_finite() && n.fract() == 0.0 && (1.0..=64.0).contains(&n))
}

#[wasm_bindgen]
pub fn validate_source_dossier_v1(
    packet: &JsValue,
    expected: &JsValue,
) -> Result<JsValue, JsValue> {
    // Keep the selected source snapshots ahead of schema validation.
    let summary = optional(packet, "agent_summary")?;
    let object = optional(packet, "object")?;
    if optional(packet, "schema")? != JsValue::from_str("tos_source_dossier_v1")
        || !reference(expected)
    {
        return refused("dossier");
    }
    if field(packet, "object_id")? != *expected
        || !object.is_object()
        || is_array(&object)?
        || field(&object, "node_id")? != *expected
    {
        return refused("dossier");
    }
    let kind = field(&object, "node_kind")?;
    if !["work", "expression", "edition", "item", "file", "link"]
        .iter()
        .any(|name| kind == JsValue::from_str(name))
    {
        return refused("dossier");
    }
    if !summary.is_truthy() {
        return refused("dossier");
    }
    for name in ["technical_access", "rights_posture"] {
        if !field(&summary, name)?.is_string() || !field(&summary, name)?.is_truthy() {
            return refused("dossier");
        }
    }
    for name in ["human_review_required", "can_conclude_legal_openness"] {
        if field(&summary, name)?.as_bool().is_none() {
            return refused("dossier");
        }
    }
    if field(&summary, "availability_is_license")?.as_bool() != Some(false) {
        return refused("dossier");
    }
    for (owner, name, cap, strings) in [
        (&summary, "rights_scope_refs", 64.0, true),
        (&summary, "gaps", 64.0, true),
        (packet, "relations", 64.0, false),
        (packet, "rights", 64.0, false),
        (packet, "tree_paths", 64.0, false),
        (packet, "source_refs", 1024.0, true),
    ] {
        let array = field(owner, name)?;
        if !is_array(&array)? || !(number(&field(&array, "length")?)? <= cap) {
            return refused("dossier");
        }
        let valid = if strings {
            STRING_ELEMENT
                .with(|callback| call(&array, "every", callback))?
                .is_truthy()
        } else {
            record_array(&array)?
        };
        if !valid {
            return refused("dossier");
        }
    }
    if field(packet, "truncated")?.as_bool().is_none() {
        return refused("dossier");
    }
    let authority_text = field(packet, "authority_note")?.is_string();
    let nonempty_text =
        authority_text && number(&field(&field(packet, "authority_note")?, "length")?)? > 0.0;
    if !nonempty_text {
        let note = field(packet, "authority_note")?;
        if !note.is_object() || is_array(&note)? {
            return refused("dossier");
        }
    }
    if !field(packet, "chain")?.is_truthy()
        || field(packet, "chain")?.js_typeof() != JsValue::from_str("object")
        || is_array(&field(packet, "chain")?)?
    {
        return refused("chain");
    }
    for value in values(&field(packet, "chain")?)?.iter() {
        if !is_array(&value)?
            || !(number(&field(&value, "length")?)? <= 64.0)
            || !record_array(&value)?
        {
            return refused("chain");
        }
    }
    Ok(JsValue::NULL)
}
