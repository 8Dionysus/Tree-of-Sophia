//! Browser source-copy form session. The owner command remains the sole write
//! authority; this state holds one exact pending request across uncertain I/O.

use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};
use wasm_bindgen::prelude::*;

const RESPONSE_BYTES: usize = 4_194_304;
const STATE_BYTES: usize = 16_777_216;
const REQUEST_BYTES: usize = 1_048_576;
const RESULT_SCHEMA: &str = "tos_local_source_command_result_v1";

fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn string<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    get(value, key)?.as_str()
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn object(items: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        items
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn array_contains(value: &JsonValue, name: &str, target: &str) -> bool {
    get(value, name)
        .and_then(JsonValue::as_array)
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(target)))
}
fn parse(raw: &[u8]) -> Result<JsonValue, JsValue> {
    parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: RESPONSE_BYTES,
            ..JsonLimits::default()
        },
    )
    .map(|value| value.into_root())
    .map_err(|_| JsValue::from_str("invalid_owner_response"))
}
fn emit(value: &JsonValue, limit: usize) -> Result<Vec<u8>, JsValue> {
    emit_value_preserved_json(
        value,
        JsonLimits {
            max_bytes: limit,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| JsValue::from_str("source_form_budget"))
}
fn valid_context(value: &JsonValue) -> bool {
    string(value, "schema_version") == Some(RESULT_SCHEMA)
        && ["describe", "prepare", "apply"]
            .iter()
            .all(|op| array_contains(value, "command_operations", op))
        && get(value, "source_fields")
            .and_then(JsonValue::as_array)
            .is_some()
        && get(value, "allowed_form_ids")
            .and_then(JsonValue::as_array)
            .is_some()
        && get(value, "source").is_some_and(|source| source.as_object().is_some())
        && string(value, "owner_configuration").is_some()
}

#[wasm_bindgen]
pub struct BrowserSourceFormSession {
    current: Option<JsonValue>,
    prepared: Option<JsonValue>,
    pending: Option<JsonValue>,
    result: Option<JsonValue>,
    uncertain: bool,
}

#[wasm_bindgen]
impl BrowserSourceFormSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            current: None,
            prepared: None,
            pending: None,
            result: None,
            uncertain: false,
        }
    }

    pub fn accept_describe(&mut self, response: &[u8]) -> Result<(), JsValue> {
        self.ensure_no_pending()?;
        let value = parse(response)?;
        if !valid_context(&value) {
            return Err(JsValue::from_str("unsupported_owner"));
        }
        self.current = Some(value);
        self.prepared = None;
        self.result = None;
        Ok(())
    }

    pub fn ensure_no_pending(&self) -> Result<(), JsValue> {
        if self.pending.is_some() {
            return Err(JsValue::from_str("pending_command"));
        }
        Ok(())
    }

    pub fn prepare_allowed(&self, form_id: &str, field_id: &str) -> Result<(), JsValue> {
        if self.pending.is_some() {
            return Err(JsValue::from_str("pending_command"));
        }
        let current = self
            .current
            .as_ref()
            .ok_or_else(|| JsValue::from_str("no_owner_context"))?;
        if !array_contains(current, "allowed_form_ids", form_id)
            || !get(current, "source_fields")
                .and_then(JsonValue::as_array)
                .is_some_and(|fields| {
                    fields
                        .iter()
                        .any(|field| string(field, "field_id") == Some(field_id))
                })
        {
            return Err(JsValue::from_str("undelegated_form_or_field"));
        }
        Ok(())
    }

    pub fn accept_prepare(&mut self, form_id: &str, response: &[u8]) -> Result<(), JsValue> {
        if self.pending.is_some() {
            return Err(JsValue::from_str("pending_command"));
        }
        let value = parse(response)?;
        if !valid_context(&value) {
            return Err(JsValue::from_str("unsupported_owner"));
        }
        let change =
            get(&value, "prepared_change").ok_or_else(|| JsValue::from_str("wrong_preparation"))?;
        if !matches!(
            string(change, "operation"),
            Some("form.create" | "form.revise")
        ) || get(change, "form").and_then(|form| string(form, "form_id")) != Some(form_id)
            || get(change, "form")
                .and_then(|form| get(form, "content"))
                .and_then(|content| string(content, "kind"))
                != Some("source-copy")
        {
            return Err(JsValue::from_str("wrong_preparation"));
        }
        let prepared = change.clone();
        self.current = Some(value);
        self.prepared = Some(prepared);
        self.result = None;
        Ok(())
    }

    pub fn begin_commit(&mut self, command_id: &str) -> Result<Vec<u8>, JsValue> {
        if self.prepared.is_none() {
            return Err(JsValue::from_str("not_prepared"));
        }
        if self.pending.is_none() {
            if command_id.is_empty() || command_id.len() > 256 {
                return Err(JsValue::from_str("invalid_command_id"));
            }
            let current = self
                .current
                .as_ref()
                .ok_or_else(|| JsValue::from_str("no_owner_context"))?;
            let source =
                get(current, "source").ok_or_else(|| JsValue::from_str("no_owner_context"))?;
            let revision =
                get(current, "revision").ok_or_else(|| JsValue::from_str("no_owner_context"))?;
            let configuration = get(current, "owner_configuration")
                .ok_or_else(|| JsValue::from_str("no_owner_context"))?;
            let candidate = object(vec![
                ("schema_version", text("tos_local_source_command_v1")),
                ("operation", text("apply")),
                ("command_id", text(command_id)),
                ("expected_source", source.clone()),
                ("expected_revision", revision.clone()),
                ("expected_configuration", configuration.clone()),
                (
                    "changes",
                    JsonValue::Array(vec![self.prepared.as_ref().unwrap().clone()]),
                ),
            ]);
            // A locally unencodable command was never submitted. Retain the
            // exact ID only after the bounded bytes are ready for transport.
            let request = emit(&candidate, REQUEST_BYTES)?;
            self.pending = Some(candidate);
            return Ok(request);
        }
        emit(self.pending.as_ref().unwrap(), REQUEST_BYTES)
    }

    pub fn accept_commit(&mut self, response: &[u8]) -> Result<(), JsValue> {
        let value = parse(response)?;
        let pending = self
            .pending
            .as_ref()
            .ok_or_else(|| JsValue::from_str("no_pending_command"))?;
        if string(&value, "schema_version") != Some(RESULT_SCHEMA)
            || get(&value, "receipt").and_then(|receipt| string(receipt, "command_id"))
                != string(pending, "command_id")
        {
            self.uncertain = true;
            return Err(JsValue::from_str("unconfirmed_owner_receipt"));
        }
        self.result = Some(value);
        self.uncertain = false;
        Ok(())
    }

    pub fn mark_uncertain(&mut self) {
        if self.pending.is_some() {
            self.uncertain = true;
        }
    }
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn state_packet(&self) -> Result<Vec<u8>, JsValue> {
        emit(
            &object(vec![
                ("current", self.current.clone().unwrap_or(JsonValue::Null)),
                ("prepared", self.prepared.clone().unwrap_or(JsonValue::Null)),
                ("pending", self.pending.clone().unwrap_or(JsonValue::Null)),
                ("result", self.result.clone().unwrap_or(JsonValue::Null)),
                ("uncertain", JsonValue::Bool(self.uncertain)),
            ]),
            STATE_BYTES,
        )
    }
    pub fn retained_command(&self) -> Result<Vec<u8>, JsValue> {
        emit(
            self.pending.as_ref().unwrap_or(&JsonValue::Null),
            REQUEST_BYTES,
        )
    }
}
