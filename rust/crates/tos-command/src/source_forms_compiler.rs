//! Native compiler callbacks over the maintained source form producer.
//! Compiler owns selected source/schema/set identity and sealed output checks.
//! This adapter only converts exact decoded JSON and enforces output budgets.

use crate::source_forms::{
    apply_form_changes, materialize_source_forms, metadata_subject, prepare_form_change,
};
use serde_json::Value;
use tos_compiler::{Error, Result};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json};

fn decode(value: &Value, max_bytes: usize) -> Result<JsonValue> {
    let raw = serde_json::to_vec(value).map_err(|_| Error::Invalid("source form input codec"))?;
    if raw.len() > max_bytes {
        return Err(Error::Budget("source form adapter input bytes"));
    }
    Ok(parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Error::Invalid("source form input typed JSON"))?
    .into_root())
}

fn encode(value: &JsonValue, max_bytes: usize) -> Result<(Value, usize)> {
    let raw = emit_value_preserved_json(
        value,
        JsonLimits {
            max_bytes,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Error::Budget("source form output codec"))?;
    let value = serde_json::from_slice(&raw)
        .map_err(|_| Error::Invalid("source form result decoded JSON"))?;
    Ok((value, raw.len()))
}

/// Shared catalog/canon callback. This neither reads a source path nor obtains
/// current rights or admission from the read-only compiler.
pub fn materialize_compiler_forms(
    source: &Value,
    set: &Value,
    max_output_bytes: usize,
) -> Result<Vec<Value>> {
    if max_output_bytes == 0 || max_output_bytes > 262_144 {
        return Err(Error::Budget("source form adapter output limit"));
    }
    let source = decode(source, 8_388_608)?;
    let set = decode(set, 2_097_152)?;
    let packets = materialize_source_forms(&source, &set)
        .map_err(|error| Error::Source(format!("source form producer: {error:?}")))?;
    let mut total = 0usize;
    let mut output = Vec::with_capacity(packets.len());
    for packet in packets {
        let (packet, bytes) = encode(&packet, max_output_bytes)?;
        total = total
            .checked_add(bytes)
            .filter(|size| *size <= max_output_bytes)
            .ok_or(Error::Budget("source form adapter output bytes"))?;
        output.push(packet);
    }
    Ok(output)
}

/// Reconstruct the maintained retained form successor through the same pure
/// producer used by command handlers. Compiler owns historical schema, exact
/// selection coverage and receipt/archive bindings; no current grant is read.
pub fn reconstruct_compiler_revision_forms(
    revised_source: &Value,
    prior_set: Option<&Value>,
    principal: &str,
    selections: &Value,
    max_output_bytes: usize,
) -> Result<Value> {
    if max_output_bytes == 0 || max_output_bytes > 262_144 {
        return Err(Error::Budget("source form adapter output limit"));
    }
    let source = decode(revised_source, 8_388_608)?;
    let prior = prior_set.map(|set| decode(set, 2_097_152)).transpose()?;
    let selections = decode(selections, 65_536)?;
    let selections = selections
        .as_array()
        .ok_or(Error::Invalid("source form selections array"))?;
    if selections.is_empty() || selections.len() > 32 || principal.len() > 4096 {
        return Err(Error::Budget("source form revision selection budget"));
    }
    let mut changes = Vec::with_capacity(selections.len());
    for selection in selections {
        let fields = selection
            .as_object()
            .ok_or(Error::Invalid("source form selection object"))?;
        if fields.len() != 2
            || fields
                .iter()
                .any(|(key, _)| !matches!(key.as_str(), Some("form_id" | "field_id")))
        {
            return Err(Error::Invalid("source form selection fields"));
        }
        let id = selection
            .object_get("form_id")
            .and_then(JsonValue::as_str)
            .ok_or(Error::Invalid("source form selection id"))?;
        let field = selection
            .object_get("field_id")
            .and_then(JsonValue::as_str)
            .ok_or(Error::Invalid("source form selection field"))?;
        changes.push(
            prepare_form_change(&source, prior.as_ref(), principal, id, field).map_err(
                |error| Error::Source(format!("source form revision producer: {error:?}")),
            )?,
        );
    }
    let subject = metadata_subject(&source)
        .map_err(|error| Error::Source(format!("source form revision subject: {error:?}")))?;
    let successor = apply_form_changes(prior.as_ref(), &subject, &changes)
        .map_err(|error| Error::Source(format!("source form retained revision: {error:?}")))?;
    Ok(encode(&successor, max_output_bytes)?.0)
}

pub struct NativeBibliographicForms;
impl tos_compiler::source_bibliographic::BibliographicForms for NativeBibliographicForms {
    fn materialize(
        &mut self,
        source: &Value,
        set: &Value,
        max_output_bytes: usize,
    ) -> Result<Vec<Value>> {
        materialize_compiler_forms(source, set, max_output_bytes)
    }
    fn reconstruct_revision_forms(
        &mut self,
        revised_source: &Value,
        prior_set: Option<&Value>,
        principal: &str,
        selections: &Value,
        max_output_bytes: usize,
    ) -> Result<Value> {
        reconstruct_compiler_revision_forms(
            revised_source,
            prior_set,
            principal,
            selections,
            max_output_bytes,
        )
    }
}
