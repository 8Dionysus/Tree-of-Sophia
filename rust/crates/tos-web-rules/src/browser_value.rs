//! JavaScript value custody for whole browser operations; no packet rules.
use js_sys::{Array, Function, JsString, Object, Reflect, Symbol, TypeError};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = Object)]
    fn boxed(value: &JsValue) -> Object;
    #[wasm_bindgen(js_namespace = Reflect, js_name = get, catch)]
    fn property(target: &Object, key: &JsValue, receiver: &JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_name = String, catch)]
    fn string(value: &JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = Object, js_name = hasOwn, catch)]
    pub(crate) fn has_own(value: &JsValue, key: &JsValue) -> Result<bool, JsValue>;
    #[wasm_bindgen(js_namespace = Array, js_name = isArray, catch)]
    pub(crate) fn is_array(value: &JsValue) -> Result<bool, JsValue>;
    #[wasm_bindgen(js_name = Number, catch)]
    pub(crate) fn number(value: &JsValue) -> Result<f64, JsValue>;
    #[wasm_bindgen(js_namespace = Object, js_name = values, catch)]
    pub(crate) fn values(value: &JsValue) -> Result<Array, JsValue>;
    #[wasm_bindgen(js_namespace = Object, js_name = keys, catch)]
    pub(crate) fn keys(value: &JsValue) -> Result<Array, JsValue>;
    #[wasm_bindgen(js_namespace = JSON, js_name = stringify, catch)]
    pub(crate) fn stringify(value: &JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_name = structuredClone, catch)]
    pub(crate) fn cloned(value: &JsValue) -> Result<JsValue, JsValue>;
}

pub(crate) fn get(value: &JsValue, key: &JsValue) -> Result<JsValue, JsValue> {
    if value.is_null_or_undefined() {
        return Err(TypeError::new("Cannot read a property of null or undefined").into());
    }
    property(&boxed(value), key, value)
}

pub(crate) fn field(value: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    get(value, &JsValue::from_str(key))
}

pub(crate) fn optional(value: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    if value.is_null_or_undefined() {
        Ok(JsValue::UNDEFINED)
    } else {
        field(value, key)
    }
}

pub(crate) fn call(value: &JsValue, method: &str, argument: &JsValue) -> Result<JsValue, JsValue> {
    field(value, method)?
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from(TypeError::new("Selected property is not callable")))?
        .call1(value, argument)
}

pub(crate) fn call0(value: &JsValue, method: &str) -> Result<JsValue, JsValue> {
    function(field(value, method)?)?.call0(value)
}

pub(crate) fn collection(name: &str, iterable: &JsValue) -> Result<JsValue, JsValue> {
    Ok(Reflect::construct(
        &function(field(&js_sys::global(), name)?)?,
        &Array::of1(iterable),
    )?
    .into())
}

pub(crate) fn spread(values: &[&JsValue]) -> Checked<JsValue> {
    let result = Array::new();
    for value in values {
        for_each(value, |item| {
            result.push(&item);
            Ok(())
        })?;
    }
    Ok(result.into())
}

pub(crate) fn invoke(host: &JsValue, name: &str, args: &[&JsValue]) -> Result<JsValue, JsValue> {
    let values = Array::new();
    for value in args {
        values.push(value);
    }
    function(field(host, name)?)?.apply(&JsValue::UNDEFINED, &values)
}

pub(crate) fn object(fields: &[(&str, &JsValue)]) -> Result<JsValue, JsValue> {
    let value = Object::new();
    for (key, field) in fields {
        Reflect::set(&value, &JsValue::from_str(key), field)?;
    }
    Ok(value.into())
}

pub(crate) fn empty_items(state: &str) -> Result<JsValue, JsValue> {
    object(&[
        ("state", &JsValue::from_str(state)),
        ("items", &Array::new().into()),
    ])
}

pub(crate) enum Failure {
    Contract(&'static str),
    Host(JsValue),
}
impl From<JsValue> for Failure {
    fn from(value: JsValue) -> Self {
        Self::Host(value)
    }
}
pub(crate) type Checked<T> = Result<T, Failure>;

pub(crate) fn finish(value: Checked<JsValue>) -> Result<JsValue, JsValue> {
    match value {
        Ok(value) => Ok(value),
        Err(Failure::Contract(code)) => Ok(JsValue::from_str(code)),
        Err(Failure::Host(error)) => Err(error),
    }
}

pub(crate) fn require(valid: bool, code: &'static str) -> Checked<()> {
    if valid {
        Ok(())
    } else {
        Err(Failure::Contract(code))
    }
}

pub(crate) fn or_empty(value: JsValue) -> JsValue {
    if value.is_truthy() {
        value
    } else {
        JsValue::from_str("")
    }
}

pub(crate) fn digest(value: &JsValue, strict: bool) -> Result<bool, JsValue> {
    if strict && !value.is_string() {
        return Ok(false);
    }
    // RegExp.test's ToString refuses symbols, including before length checks.
    if value.is_symbol() {
        return Err(TypeError::new("Cannot convert a Symbol value to a string").into());
    }
    let text = string(value)?;
    let text: &JsString = text.unchecked_ref();
    Ok(text.length() == 64
        && (0..64).all(|i| {
            let unit = text.char_code_at(i) as u16;
            (48..=57).contains(&unit) || (97..=102).contains(&unit)
        }))
}

fn function(value: JsValue) -> Result<Function, JsValue> {
    value
        .dyn_into::<Function>()
        .map_err(|_| TypeError::new("Selected property is not callable").into())
}

fn js_object(value: &JsValue) -> bool {
    value.is_object() || value.is_function()
}

// Own the native iterator protocol, including IteratorClose only after an
// abrupt loop body. Its return error cannot replace the original refusal.
pub(crate) fn for_each(
    value: &JsValue,
    mut visit: impl FnMut(JsValue) -> Checked<()>,
) -> Checked<()> {
    let iterator = function(get(value, &Symbol::iterator().into())?)?.call0(value)?;
    require_object(&iterator)?;
    let next = function(field(&iterator, "next")?)?;
    loop {
        let step = next.call0(&iterator)?;
        require_object(&step)?;
        if field(&step, "done")?.is_truthy() {
            return Ok(());
        }
        let item = field(&step, "value")?;
        if let Err(error) = visit(item) {
            if let Ok(close) = field(&iterator, "return").and_then(function) {
                let _ = close.call0(&iterator);
            }
            return Err(error);
        }
    }
}

fn require_object(value: &JsValue) -> Result<(), JsValue> {
    if js_object(value) {
        Ok(())
    } else {
        Err(TypeError::new("Iterator result is not an object").into())
    }
}
