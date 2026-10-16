//! Framed adapter for concrete joined maintenance file owners.
use super::prepared_publication::{
    acknowledge, decode_change, exact, field, line, parse, path, text, uint,
};
use std::{io::BufRead, time::Instant};
use tos_compiler::{
    local_prepared::{PreparedChange, PublicationLimits},
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::{CatalogInputs, SourceOrderProfile},
    prepared_maintenance::MaintenanceReceipt,
    prepared_maintenance_file as owner,
    prepared_semantic_index::{SemanticMaintenanceLimits, SemanticRows},
};
use tos_foundation::{JsonNumber, JsonNumberKind, JsonString, JsonValue};

const CONTROL_CAP: usize = 16_843_008;
fn size(value: &JsonValue, key: &str) -> Result<usize, String> {
    usize::try_from(uint(field(value, key)?)?).map_err(|_| "maintenance limit range".into())
}
fn boolean(value: &JsonValue, key: &str) -> Result<bool, String> {
    match field(value, key)? {
        JsonValue::Bool(v) => Ok(*v),
        _ => Err("maintenance boolean required".into()),
    }
}
fn catalog_limits(frame: &JsonValue) -> Result<CatalogMaintenanceLimits, String> {
    let v = field(frame, "catalog_limits")?;
    exact(
        v,
        &[
            "max_changes",
            "max_incident_relations",
            "max_row_bytes",
            "max_delta_bytes",
            "max_catalog_entries",
            "max_aggregate_bytes",
            "max_catalog_bytes",
            "max_index_bytes",
        ],
    )?;
    let result = CatalogMaintenanceLimits {
        max_changes: size(v, "max_changes")?,
        max_incident_relations: size(v, "max_incident_relations")?,
        max_row_bytes: size(v, "max_row_bytes")?,
        max_delta_bytes: size(v, "max_delta_bytes")?,
        max_catalog_entries: size(v, "max_catalog_entries")?,
        max_aggregate_bytes: size(v, "max_aggregate_bytes")?,
        max_catalog_bytes: size(v, "max_catalog_bytes")?,
        max_index_bytes: uint(field(v, "max_index_bytes")?)?,
    };
    result.validate().map_err(|e| e.to_string())?;
    Ok(result)
}
fn semantic_limits(frame: &JsonValue) -> Result<SemanticMaintenanceLimits, String> {
    let v = field(frame, "semantic_limits")?;
    exact(
        v,
        &[
            "max_changes",
            "max_rows",
            "max_queries",
            "max_writes",
            "max_read_bytes",
            "max_input_bytes",
            "max_input_values",
            "max_row_bytes",
            "max_output_bytes",
            "max_output_items",
            "max_bytes",
        ],
    )?;
    let result = SemanticMaintenanceLimits {
        max_changes: size(v, "max_changes")?,
        max_rows: uint(field(v, "max_rows")?)?,
        max_queries: uint(field(v, "max_queries")?)?,
        max_writes: uint(field(v, "max_writes")?)?,
        max_read_bytes: size(v, "max_read_bytes")?,
        max_input_bytes: size(v, "max_input_bytes")?,
        max_input_values: size(v, "max_input_values")?,
        max_row_bytes: size(v, "max_row_bytes")?,
        max_output_bytes: size(v, "max_output_bytes")?,
        max_output_items: size(v, "max_output_items")?,
        max_bytes: uint(field(v, "max_bytes")?)?,
    };
    result.validate().map_err(|e| e.to_string())?;
    Ok(result)
}
fn inputs(input: &mut dyn BufRead, key: &str, deadline: Instant) -> Result<CatalogInputs, String> {
    let raw = line(input, CONTROL_CAP, deadline)?;
    let frame = parse(&raw, CONTROL_CAP)?;
    exact(&frame, &[key])?;
    let v = field(&frame, key)?;
    exact(
        v,
        &[
            "header",
            "entity_registry",
            "relation_registry",
            "lenses",
            "source_order_profile",
        ],
    )?;
    let result = CatalogInputs {
        header: field(v, "header")?.clone(),
        entity_registry: field(v, "entity_registry")?.clone(),
        relation_registry: field(v, "relation_registry")?.clone(),
        lenses: field(v, "lenses")?
            .as_array()
            .ok_or("maintenance lens array")?
            .to_vec(),
        source_order_profile: match text(v, "source_order_profile")? {
            "owner-sequence-v1" => SourceOrderProfile::OwnerSequence,
            "source-graph-id-v1" => SourceOrderProfile::SourceGraphId,
            _ => return Err("maintenance source order profile".into()),
        },
    };
    result.validate().map_err(|e| e.to_string())?;
    Ok(result)
}
struct Rows<'a> {
    input: &'a mut dyn BufRead,
    deadline: Instant,
    cap: usize,
    remaining: u64,
}
impl SemanticRows for Rows<'_> {
    fn visit(
        &mut self,
        _kind: &str,
        sink: &mut dyn FnMut(&JsonValue) -> tos_compiler::Result<()>,
    ) -> tos_compiler::Result<()> {
        loop {
            let cap = self
                .cap
                .checked_add(32)
                .ok_or(tos_compiler::Error::Budget("maintenance row framing"))?;
            let raw = line(self.input, cap, self.deadline).map_err(tos_compiler::Error::Source)?;
            let v = parse(&raw, cap).map_err(tos_compiler::Error::Source)?;
            if v.object_get("end") == Some(&JsonValue::Bool(true)) {
                exact(&v, &["end"]).map_err(tos_compiler::Error::Source)?;
                return Ok(());
            }
            exact(&v, &["row"]).map_err(tos_compiler::Error::Source)?;
            self.remaining = self
                .remaining
                .checked_sub(1)
                .ok_or(tos_compiler::Error::Budget("maintenance row count"))?;
            sink(field(&v, "row").map_err(tos_compiler::Error::Source)?)?;
        }
    }
}
fn changes(
    input: &mut dyn BufRead,
    limits: PublicationLimits,
    deadline: Instant,
) -> Result<Vec<PreparedChange>, String> {
    let cap = limits
        .max_row_bytes
        .checked_add(32768)
        .ok_or("maintenance change frame cap")?;
    let total_cap = limits
        .max_change_bytes
        .checked_add(limits.max_metadata_bytes)
        .ok_or("maintenance change total cap")?;
    let mut result = Vec::new();
    let mut bytes = 0usize;
    loop {
        let raw = line(input, cap, deadline)?;
        if raw == b"{\"end\":true}" {
            break;
        }
        if result.len() >= limits.max_changes {
            return Err("maintenance change count".into());
        }
        bytes = bytes
            .checked_add(raw.len())
            .filter(|n| *n <= total_cap)
            .ok_or("maintenance retained change bytes")?;
        result.push(decode_change(&raw, cap)?);
    }
    Ok(result)
}
fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn receipt(value: MaintenanceReceipt) -> JsonValue {
    let mut fields = vec![
        ("binding", value.binding),
        ("catalog_digest", string(&value.catalog_digest)),
        ("sql_mutations", number(value.sql_mutations)),
        (
            "publication_changed",
            JsonValue::Bool(value.publication_changed),
        ),
        (
            "consumer_switched",
            JsonValue::Bool(value.consumer_switched),
        ),
        (
            "source_transition_verified",
            JsonValue::Bool(value.source_transition_verified),
        ),
        (
            "semantic_acceptance",
            JsonValue::Bool(value.semantic_acceptance),
        ),
    ];
    if let Some(v) = value.source_header {
        fields.push(("source_header", v));
    }
    if let Some(v) = value.catalog {
        fields.push(("catalog", v));
    }
    if let Some(v) = value.semantic_report {
        fields.push(("semantic_report", v));
    }
    if let Some(v) = value.semantic_report_sha256 {
        fields.push(("semantic_report_sha256", string(&v)));
    }
    object(fields)
}
pub(crate) fn run(
    frame: &JsonValue,
    input: &mut dyn BufRead,
    limits: PublicationLimits,
    deadline: Instant,
    progress_fd: Option<i32>,
) -> Result<(JsonValue, usize), String> {
    exact(
        frame,
        &[
            "operation",
            "path",
            "limits",
            "catalog_limits",
            "semantic_limits",
            "expected_binding",
            "max_seconds",
            "ordered_rows",
            "normalization_processor_sha256",
        ],
    )?;
    let catalog = catalog_limits(frame)?;
    let semantic = semantic_limits(frame)?;
    let output_cap = limits
        .max_metadata_bytes
        .checked_add(catalog.max_catalog_bytes)
        .and_then(|n| {
            semantic
                .max_output_bytes
                .checked_mul(2)
                .and_then(|s| n.checked_add(s))
        })
        .and_then(|n| n.checked_add(65536))
        .ok_or("maintenance result cap overflow")?;
    let fd = progress_fd.ok_or("maintenance source precommit acknowledgement required")?;
    let mut precommit = || {
        acknowledge(
            &object(vec![
                ("phase", string("maintenance_precommit")),
                ("committed", JsonValue::Bool(false)),
            ]),
            fd,
            deadline,
        )
    };
    let path = path(text(frame, "path")?)?;
    let expected = field(frame, "expected_binding")?;
    let processor = field(frame, "normalization_processor_sha256")?;
    let ordered = boolean(frame, "ordered_rows")?;
    let result = match text(frame, "operation")? {
        "maintenance-bootstrap" => {
            let inputs = inputs(input, "inputs", deadline)?;
            let mut rows = Rows {
                input,
                deadline,
                cap: limits.max_row_bytes.min(semantic.max_row_bytes),
                remaining: semantic.max_rows,
            };
            owner::bootstrap_prepared_maintenance_file(
                &path,
                expected,
                &inputs,
                limits,
                catalog,
                semantic,
                if ordered {
                    Some(&mut rows as &mut dyn SemanticRows)
                } else {
                    None
                },
                processor.as_str().ok_or("maintenance processor digest")?,
                deadline,
                &mut precommit,
            )
        }
        operation @ ("catalogued-delta" | "semantic-delta") => {
            if ordered {
                return Err("maintenance delta cannot carry bootstrap rows".into());
            }
            let before = inputs(input, "before_inputs", deadline)?;
            let after = inputs(input, "after_inputs", deadline)?;
            let changes = changes(input, limits, deadline)?;
            if operation == "catalogued-delta" {
                if *processor != JsonValue::Null {
                    return Err("catalogue-only processor must be absent".into());
                }
                owner::apply_catalogued_prepared_delta_file(
                    &path,
                    expected,
                    &before,
                    &after,
                    &changes,
                    limits,
                    catalog,
                    deadline,
                    &mut precommit,
                )
            } else {
                owner::apply_semantic_prepared_delta_file(
                    &path,
                    expected,
                    &before,
                    &after,
                    &changes,
                    limits,
                    catalog,
                    semantic,
                    processor.as_str().ok_or("maintenance processor digest")?,
                    deadline,
                    &mut precommit,
                )
            }
        }
        _ => return Err("maintenance operation".into()),
    }
    .map_err(|e| e.to_string())?;
    Ok((receipt(result), output_cap))
}
