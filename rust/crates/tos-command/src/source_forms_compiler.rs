//! Native compiler callbacks over the maintained source form producer.
//! Compiler owns selected source/schema/set identity and sealed output checks.
//! This adapter only converts exact decoded JSON and enforces output budgets.

use crate::source_forms::materialize_source_forms;
use serde_json::Value;
use tos_compiler::{Error, Result};
use tos_foundation::{JsonLimits, JsonMode, emit_value_preserved_json, parse_json};

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
    let source_raw =
        serde_json::to_vec(source).map_err(|_| Error::Invalid("source form source codec"))?;
    let set_raw = serde_json::to_vec(set).map_err(|_| Error::Invalid("source form set codec"))?;
    if source_raw.len() > 8_388_608 || set_raw.len() > 2_097_152 {
        return Err(Error::Budget("source form adapter input bytes"));
    }
    let source = parse_json(
        &source_raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Error::Invalid("source form source typed JSON"))?
    .into_root();
    let set = parse_json(
        &set_raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 2_097_152,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Error::Invalid("source form set typed JSON"))?
    .into_root();
    let packets = materialize_source_forms(&source, &set)
        .map_err(|error| Error::Source(format!("source form producer: {error:?}")))?;
    let mut total = 0usize;
    let mut output = Vec::with_capacity(packets.len());
    for packet in packets {
        let raw = emit_value_preserved_json(
            &packet,
            JsonLimits {
                max_bytes: 65_536,
                ..JsonLimits::default()
            },
        )
        .map_err(|_| Error::Budget("source form packet codec"))?;
        total = total
            .checked_add(raw.len())
            .filter(|size| *size <= max_output_bytes)
            .ok_or(Error::Budget("source form adapter output bytes"))?;
        output.push(
            serde_json::from_slice(&raw)
                .map_err(|_| Error::Invalid("source form result decoded JSON"))?,
        );
    }
    Ok(output)
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
}
