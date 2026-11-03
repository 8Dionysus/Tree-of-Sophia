//! Shared selected legacy JSON parsing for in-process and historical worker execution.

use crate::executor::{LegacySelectedDiagnosticsLimits, schema_diagnostics};
use tos_foundation::{
    FoundationErrorCode, JsonLimits, JsonMode, JsonNumberKind, JsonValue,
    parse_json_with_state_budget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacySelectedFailure {
    InvalidJson,
    InputBudget,
    UnsupportedInputSemantics,
}

#[derive(Default)]
struct LegacySelectedConversionShape {
    values: usize,
    object_entries: usize,
    array_elements: usize,
    cloned_text_bytes: usize,
    number_storage_bytes: usize,
}

impl LegacySelectedConversionShape {
    fn total_state_bytes(&self) -> Result<usize, LegacySelectedFailure> {
        let map_nodes = self
            .object_entries
            .checked_mul(768)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        let value_slots = self
            .values
            .checked_mul(std::mem::size_of::<serde_json::Value>())
            .ok_or(LegacySelectedFailure::InputBudget)?;
        let frame_bytes = std::mem::size_of::<JsonValue>()
            .checked_add(std::mem::size_of::<serde_json::Value>())
            .and_then(|bytes| bytes.checked_add(64))
            .ok_or(LegacySelectedFailure::InputBudget)?;
        let conversion_stack = (LegacySelectedDiagnosticsLimits::MAX_DEPTH + 1)
            .checked_mul(frame_bytes)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        [
            map_nodes,
            self.cloned_text_bytes,
            self.number_storage_bytes,
            value_slots,
            conversion_stack,
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or(LegacySelectedFailure::InputBudget)
    }
}

fn legacy_selected_conversion_shape(
    value: &JsonValue,
    depth: usize,
    shape: &mut LegacySelectedConversionShape,
) -> Result<(), LegacySelectedFailure> {
    if depth > LegacySelectedDiagnosticsLimits::MAX_DEPTH {
        return Err(LegacySelectedFailure::InputBudget);
    }
    shape.values = shape
        .values
        .checked_add(1)
        .ok_or(LegacySelectedFailure::InputBudget)?;
    match value {
        JsonValue::Null | JsonValue::Bool(_) => {}
        JsonValue::Number(number) => {
            // Integer Numbers may retain their arbitrary-precision lexeme;
            // float conversion has a bounded shortest representation.
            let bytes = number
                .lexeme
                .len()
                .checked_add(if number.kind == JsonNumberKind::Float {
                    32
                } else {
                    0
                })
                .ok_or(LegacySelectedFailure::InputBudget)?;
            shape.number_storage_bytes = shape
                .number_storage_bytes
                .checked_add(bytes)
                .ok_or(LegacySelectedFailure::InputBudget)?;
        }
        JsonValue::String(string) => {
            let value = string
                .as_str()
                .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
            shape.cloned_text_bytes = shape
                .cloned_text_bytes
                .checked_add(value.len())
                .ok_or(LegacySelectedFailure::InputBudget)?;
        }
        JsonValue::Array(items) => {
            shape.array_elements = shape
                .array_elements
                .checked_add(items.len())
                .ok_or(LegacySelectedFailure::InputBudget)?;
            for item in items {
                legacy_selected_conversion_shape(item, depth + 1, shape)?;
            }
        }
        JsonValue::Object(entries) => {
            shape.object_entries = shape
                .object_entries
                .checked_add(entries.len())
                .ok_or(LegacySelectedFailure::InputBudget)?;
            for (key, item) in entries {
                let key = key
                    .as_str()
                    .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                shape.cloned_text_bytes = shape
                    .cloned_text_bytes
                    .checked_add(key.len())
                    .ok_or(LegacySelectedFailure::InputBudget)?;
                legacy_selected_conversion_shape(item, depth + 1, shape)?;
            }
        }
    }
    Ok(())
}

struct LegacySelectedConversionMeter {
    max_bytes: u64,
    shape: LegacySelectedConversionShape,
    used_bytes: u64,
    values_seen: usize,
    entries_charged: usize,
    arrays_reserved: usize,
    text_bytes_charged: usize,
    number_bytes_charged: usize,
}

impl LegacySelectedConversionMeter {
    fn new(
        shape: LegacySelectedConversionShape,
        max_bytes: u64,
    ) -> Result<Self, LegacySelectedFailure> {
        let value_slots = shape
            .values
            .checked_mul(std::mem::size_of::<serde_json::Value>())
            .ok_or(LegacySelectedFailure::InputBudget)?;
        let frame_bytes = std::mem::size_of::<JsonValue>()
            .checked_add(std::mem::size_of::<serde_json::Value>())
            .and_then(|bytes| bytes.checked_add(64))
            .ok_or(LegacySelectedFailure::InputBudget)?;
        let stack_bytes = (LegacySelectedDiagnosticsLimits::MAX_DEPTH + 1)
            .checked_mul(frame_bytes)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        let reserved = u64::try_from(
            value_slots
                .checked_add(stack_bytes)
                .ok_or(LegacySelectedFailure::InputBudget)?,
        )
        .map_err(|_| LegacySelectedFailure::InputBudget)?;
        if reserved > max_bytes {
            return Err(LegacySelectedFailure::InputBudget);
        }
        Ok(Self {
            max_bytes,
            shape,
            used_bytes: reserved,
            values_seen: 0,
            entries_charged: 0,
            arrays_reserved: 0,
            text_bytes_charged: 0,
            number_bytes_charged: 0,
        })
    }

    fn charge(&mut self, bytes: usize) -> Result<(), LegacySelectedFailure> {
        self.used_bytes = self
            .used_bytes
            .checked_add(u64::try_from(bytes).map_err(|_| LegacySelectedFailure::InputBudget)?)
            .filter(|used| *used <= self.max_bytes)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        Ok(())
    }

    fn visit_value(&mut self) -> Result<(), LegacySelectedFailure> {
        self.values_seen = self
            .values_seen
            .checked_add(1)
            .filter(|seen| *seen <= self.shape.values)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        Ok(())
    }

    fn charge_text(&mut self, bytes: usize) -> Result<(), LegacySelectedFailure> {
        self.text_bytes_charged = self
            .text_bytes_charged
            .checked_add(bytes)
            .filter(|charged| *charged <= self.shape.cloned_text_bytes)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        self.charge(bytes)
    }

    fn charge_number(
        &mut self,
        number: &tos_foundation::JsonNumber,
    ) -> Result<(), LegacySelectedFailure> {
        let bytes = number
            .lexeme
            .len()
            .checked_add(if number.kind == JsonNumberKind::Float {
                32
            } else {
                0
            })
            .ok_or(LegacySelectedFailure::InputBudget)?;
        self.number_bytes_charged = self
            .number_bytes_charged
            .checked_add(bytes)
            .filter(|charged| *charged <= self.shape.number_storage_bytes)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        self.charge(bytes)
    }

    fn before_array_reserve(&mut self, elements: usize) -> Result<(), LegacySelectedFailure> {
        self.arrays_reserved = self
            .arrays_reserved
            .checked_add(elements)
            .filter(|reserved| *reserved <= self.shape.array_elements)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        Ok(())
    }

    fn charge_map_entry(&mut self) -> Result<(), LegacySelectedFailure> {
        self.entries_charged = self
            .entries_charged
            .checked_add(1)
            .filter(|charged| *charged <= self.shape.object_entries)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        self.charge(768)
    }

    fn finish(&self) -> Result<(), LegacySelectedFailure> {
        if self.values_seen != self.shape.values
            || self.entries_charged != self.shape.object_entries
            || self.arrays_reserved != self.shape.array_elements
            || self.text_bytes_charged != self.shape.cloned_text_bytes
            || self.number_bytes_charged != self.shape.number_storage_bytes
        {
            return Err(LegacySelectedFailure::InputBudget);
        }
        let planned = self.shape.total_state_bytes()?;
        if u64::try_from(planned).map_err(|_| LegacySelectedFailure::InputBudget)?
            != self.used_bytes
        {
            return Err(LegacySelectedFailure::InputBudget);
        }
        Ok(())
    }
}

fn clone_legacy_selected_string(
    value: &str,
    meter: &mut LegacySelectedConversionMeter,
) -> Result<String, LegacySelectedFailure> {
    meter.charge_text(value.len())?;
    let mut cloned = String::new();
    cloned
        .try_reserve_exact(value.len())
        .map_err(|_| LegacySelectedFailure::InputBudget)?;
    cloned.push_str(value);
    Ok(cloned)
}

fn convert_legacy_selected_value(
    value: &JsonValue,
    meter: &mut LegacySelectedConversionMeter,
) -> Result<serde_json::Value, LegacySelectedFailure> {
    meter.visit_value()?;
    match value {
        JsonValue::Null => Ok(serde_json::Value::Null),
        JsonValue::Bool(value) => Ok(serde_json::Value::Bool(*value)),
        JsonValue::Number(number) => {
            meter.charge_number(number)?;
            let number = match number.kind {
                JsonNumberKind::Int => number
                    .lexeme
                    .parse::<serde_json::Number>()
                    .map_err(|_| LegacySelectedFailure::UnsupportedInputSemantics)?,
                JsonNumberKind::Float => {
                    let value = number
                        .as_python_float()
                        .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                    if !value.is_finite() {
                        return Err(LegacySelectedFailure::UnsupportedInputSemantics);
                    }
                    serde_json::Number::from_f64(value)
                        .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?
                }
            };
            Ok(serde_json::Value::Number(number))
        }
        JsonValue::String(value) => {
            let value = value
                .as_str()
                .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
            Ok(serde_json::Value::String(clone_legacy_selected_string(
                value, meter,
            )?))
        }
        JsonValue::Array(items) => {
            // The complete tree's `values * size_of::<Value>()` slots are
            // prepaid by the meter before conversion. Reserve exactly
            // this array's share before visiting its children, so Vec
            // growth cannot exceed that already-admitted slot budget.
            meter.before_array_reserve(items.len())?;
            let mut converted = Vec::new();
            converted
                .try_reserve_exact(items.len())
                .map_err(|_| LegacySelectedFailure::InputBudget)?;
            for item in items {
                converted.push(convert_legacy_selected_value(item, meter)?);
            }
            Ok(serde_json::Value::Array(converted))
        }
        JsonValue::Object(entries) => {
            let mut converted = serde_json::Map::new();
            for (key, item) in entries {
                let key = key
                    .as_str()
                    .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                let key = clone_legacy_selected_string(key, meter)?;
                let item = convert_legacy_selected_value(item, meter)?;
                meter.charge_map_entry()?;
                converted.insert(key, item);
            }
            Ok(serde_json::Value::Object(converted))
        }
    }
}

pub(crate) fn parse_convert_selected_legacy(
    raw: &[u8],
    limits: LegacySelectedDiagnosticsLimits,
) -> Result<serde_json::Value, LegacySelectedFailure> {
    if raw.len() > limits.max_instance_bytes {
        return Err(LegacySelectedFailure::InputBudget);
    }
    let parser_state_bytes = usize::try_from(limits.parser_state_bytes)
        .map_err(|_| LegacySelectedFailure::InputBudget)?;
    let json_limits = JsonLimits::new(
        limits.max_instance_bytes,
        LegacySelectedDiagnosticsLimits::MAX_DEPTH,
        limits.max_visits as usize,
        LegacySelectedDiagnosticsLimits::MAX_INTEGER_DIGITS,
    )
    .map_err(|_| LegacySelectedFailure::InputBudget)?;
    let document = parse_json_with_state_budget(
        raw,
        JsonMode::LegacyPythonObserved,
        json_limits,
        parser_state_bytes,
    )
    .map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            LegacySelectedFailure::InputBudget
        } else {
            LegacySelectedFailure::InvalidJson
        }
    })?;
    let value = document.into_root();
    let mut shape = LegacySelectedConversionShape::default();
    legacy_selected_conversion_shape(&value, 0, &mut shape)?;
    let conversion_state_bytes = shape.total_state_bytes()?;
    if u64::try_from(conversion_state_bytes).map_err(|_| LegacySelectedFailure::InputBudget)?
        > limits.conversion_state_bytes
    {
        return Err(LegacySelectedFailure::InputBudget);
    }
    let mut meter = LegacySelectedConversionMeter::new(shape, limits.conversion_state_bytes)?;
    let converted = convert_legacy_selected_value(&value, &mut meter)?;
    meter.finish()?;
    Ok(converted)
}

pub(crate) fn in_process_selected_legacy(
    raw: &[u8],
    limits: LegacySelectedDiagnosticsLimits,
) -> Result<serde_json::Value, schema_diagnostics::Failure> {
    limits
        .validate()
        .map_err(|_| schema_diagnostics::Failure::InputBudget)?;
    parse_convert_selected_legacy(raw, limits).map_err(|failure| match failure {
        LegacySelectedFailure::InvalidJson => schema_diagnostics::Failure::InvalidJson,
        LegacySelectedFailure::InputBudget => schema_diagnostics::Failure::InputBudget,
        LegacySelectedFailure::UnsupportedInputSemantics => {
            schema_diagnostics::Failure::UnsupportedInputSemantics
        }
    })
}
