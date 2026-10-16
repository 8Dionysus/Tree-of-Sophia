//! Controlled, allocation-free evaluation of an explicit JSON Schema subset.
//! Unsupported assertions are errors during preparation, never ignored validators.
use crate::{FormatProfile, SchemaBackendProbe, SchemaProbeError, SchemaResource};
use std::mem::size_of;
use tos_foundation::{
    Digest256, Digest256Hasher, FoundationError, FoundationErrorCode, JsonDocument, JsonLimits,
    JsonMode, JsonString, JsonValue, parse_json_with_state_budget_and_check,
};
type Result<T> = std::result::Result<T, SchemaProbeError>;

/// Borrowed original invocation controls. There is no local counter reset or grant.
pub trait SchemaProbeControl {
    fn remaining(&self, prospective: usize) -> Result<usize>;
    fn check(&self) -> Result<()>;
    fn charge_work(&self, bytes: u64) -> Result<()>;
    fn remaining_json_visits(&self) -> Result<usize>;
    fn debit_json_visits(&self, used: usize) -> Result<()>;
}

pub struct ControlledSchemaBackendProbe {
    uri: String,
    raw_digest: Digest256,
    profile: FormatProfile,
    schema: JsonValue,
    retained: usize,
    depth: usize,
}

// The actual evaluator owns these iterators/counts during a recursive child call.
struct EvaluationFrame<'a> {
    entries: std::slice::Iter<'a, (JsonString, JsonValue)>,
    values: std::slice::Iter<'a, JsonValue>,
    count: usize,
    valid: bool,
}
fn budget() -> SchemaProbeError {
    SchemaProbeError::BudgetExceeded
}
fn unsupported() -> SchemaProbeError {
    SchemaProbeError::UnsupportedControlledSchema
}
fn incompatible() -> SchemaProbeError {
    SchemaProbeError::IncompatibleJsonRepresentation
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(budget)
}
fn work(c: &dyn SchemaProbeControl, n: usize) -> Result<()> {
    c.check()?;
    c.charge_work(u64::try_from(n).map_err(|_| budget())?)?;
    c.check()
}
// Logical typed activation census, not a compiler ABI/RSS stack measurement.
// Count real arguments, slice/Vec borrows, iterators, key/child borrows and
// checked child results for each of the three serial recursive traversals.
// The largest nonrecursive comparison/lookup workspace is held separately.
fn frames(depth: usize) -> Result<usize> {
    type Entries<'a> = std::slice::Iter<'a, (JsonString, JsonValue)>;
    type Values<'a> = std::slice::Iter<'a, JsonValue>;
    let storage = add(
        size_of::<(&JsonValue, &dyn SchemaProbeControl, usize, usize)>(),
        size_of::<(
            usize,
            &Vec<JsonValue>,
            &Vec<(JsonString, JsonValue)>,
            Values<'_>,
            Entries<'_>,
            &JsonString,
            &JsonValue,
            Result<usize>,
        )>(),
    )?;
    let inspection = add(
        size_of::<(&JsonValue, &dyn SchemaProbeControl, usize, usize)>(),
        size_of::<(
            &[(JsonString, JsonValue)],
            Entries<'_>,
            &JsonString,
            &JsonValue,
            &[JsonValue],
            std::iter::Enumerate<Values<'_>>,
            Values<'_>,
            usize,
            &JsonValue,
            &JsonValue,
            Result<()>,
        )>(),
    )?;
    let evaluation = add(
        size_of::<(
            &JsonValue,
            &JsonValue,
            &dyn SchemaProbeControl,
            usize,
            usize,
        )>(),
        add(
            size_of::<EvaluationFrame<'_>>(),
            size_of::<(
                &JsonString,
                &JsonValue,
                &str,
                std::str::Chars<'_>,
                std::str::Bytes<'_>,
                usize,
                bool,
                &Vec<JsonValue>,
                &[(JsonString, JsonValue)],
                Entries<'_>,
                std::iter::Enumerate<Values<'_>>,
                Values<'_>,
                usize,
                &JsonValue,
                &JsonValue,
                Option<&JsonValue>,
                Result<bool>,
            )>(),
        )?,
    )?;
    let lookup = size_of::<(
        &JsonValue,
        &str,
        &dyn SchemaProbeControl,
        &[(JsonString, JsonValue)],
        Entries<'_>,
        &JsonString,
        &JsonValue,
        Result<Option<&JsonValue>>,
    )>();
    let comparison = size_of::<(
        &JsonValue,
        &JsonValue,
        &dyn SchemaProbeControl,
        &[u16],
        &[u16],
        usize,
        Result<bool>,
    )>();
    let helper = size_of::<(&dyn SchemaProbeControl, usize, u64, Result<()>)>();
    let numeric_pattern = size_of::<(
        &str,
        &dyn SchemaProbeControl,
        &str,
        std::str::Bytes<'_>,
        Result<usize>,
    )>();
    add(
        depth
            .checked_add(1)
            .and_then(|n| n.checked_mul(storage.max(inspection).max(evaluation)))
            .ok_or_else(budget)?,
        add(lookup.max(comparison).max(numeric_pattern), helper)?,
    )
}
fn text<'a>(v: &'a JsonValue) -> Result<&'a str> {
    match v {
        JsonValue::String(s) => s.as_str().ok_or_else(incompatible),
        _ => Err(unsupported()),
    }
}
fn lookup<'a>(
    v: &'a JsonValue,
    name: &str,
    c: &dyn SchemaProbeControl,
) -> Result<Option<&'a JsonValue>> {
    let entries = v.as_object().ok_or_else(unsupported)?;
    for (key, value) in entries {
        work(
            c,
            add(key.as_str().ok_or_else(incompatible)?.len(), name.len())?,
        )?;
        if key.as_str() == Some(name) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}
fn equal(a: &JsonValue, b: &JsonValue, c: &dyn SchemaProbeControl) -> Result<bool> {
    c.check()?;
    match (a, b) {
        (JsonValue::String(a), JsonValue::String(b)) => {
            work(
                c,
                add(a.units().len(), b.units().len())?
                    .checked_mul(size_of::<u16>())
                    .ok_or_else(budget)?,
            )?;
            Ok(a.units() == b.units())
        }
        (JsonValue::Bool(a), JsonValue::Bool(b)) => {
            work(c, 1)?;
            Ok(a == b)
        }
        (JsonValue::Null, JsonValue::Null) => {
            work(c, 1)?;
            Ok(true)
        }
        _ => {
            work(c, 1)?;
            Ok(false)
        }
    }
}
fn scalar(v: &JsonValue) -> bool {
    matches!(
        v,
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::String(_)
    )
}
fn number(v: &JsonValue, c: &dyn SchemaProbeControl) -> Result<usize> {
    let JsonValue::Number(n) = v else {
        return Err(unsupported());
    };
    work(c, n.lexeme.len())?;
    usize::try_from(v.as_u64().ok_or_else(unsupported)?).map_err(|_| unsupported())
}
// Supported anchored lowercase hexadecimal repetitions. No regex engine state,
// backtracking, external format resources or schema-identity special cases.
fn hex_width(pattern: &str, c: &dyn SchemaProbeControl) -> Result<usize> {
    work(c, pattern.len())?;
    let digits = pattern
        .strip_prefix("^[0-9a-f]{")
        .and_then(|s| s.strip_suffix("}$"))
        .ok_or_else(unsupported)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(unsupported());
    }
    digits.parse().map_err(|_| unsupported())
}
fn storage(v: &JsonValue, c: &dyn SchemaProbeControl, depth: usize, limit: usize) -> Result<usize> {
    work(c, size_of::<JsonValue>())?;
    if depth > limit {
        return Err(budget());
    }
    match v {
        JsonValue::Null | JsonValue::Bool(_) => Ok(0),
        JsonValue::Number(n) => Ok(n.lexeme.capacity()),
        JsonValue::String(s) => {
            if s.as_str().is_none() {
                return Err(incompatible());
            }
            s.retained_storage_bytes().map_err(|_| budget())
        }
        JsonValue::Array(a) => {
            let mut total = a
                .capacity()
                .checked_mul(size_of::<JsonValue>())
                .ok_or_else(budget)?;
            for value in a {
                total = add(total, storage(value, c, depth + 1, limit)?)?;
            }
            Ok(total)
        }
        JsonValue::Object(o) => {
            let mut total = o
                .capacity()
                .checked_mul(size_of::<(JsonString, JsonValue)>())
                .ok_or_else(budget)?;
            for (key, value) in o {
                c.check()?;
                if key.as_str().is_none() {
                    return Err(incompatible());
                }
                total = add(total, key.retained_storage_bytes().map_err(|_| budget())?)?;
                total = add(total, storage(value, c, depth + 1, limit)?)?;
            }
            Ok(total)
        }
    }
}
fn parse(
    raw: &[u8],
    mut limits: JsonLimits,
    c: &dyn SchemaProbeControl,
    prospective: usize,
) -> Result<JsonValue> {
    c.check()?;
    limits.max_bytes = limits.max_bytes.min(SchemaBackendProbe::MAX_INSTANCE_BYTES);
    limits.max_depth = limits.max_depth.min(64);
    limits.max_visits = limits.max_visits.min(c.remaining_json_visits()?);
    if limits.max_visits == 0 {
        return Err(budget());
    }
    work(c, raw.len())?;
    let mut check = || {
        c.check().map_err(|_| FoundationError {
            code: FoundationErrorCode::BudgetExceeded,
            byte_offset: None,
            detail: String::new(),
        })
    };
    let caller_workspace = size_of::<(
        &[u8],
        JsonLimits,
        &dyn SchemaProbeControl,
        usize,
        usize,
        std::result::Result<JsonDocument, FoundationError>,
    )>();
    let available = c.remaining(add(
        add(prospective, caller_workspace)?,
        std::mem::size_of_val(&check),
    )?)?;
    let parsed = parse_json_with_state_budget_and_check(
        raw,
        JsonMode::PublishedStrict,
        limits,
        available,
        &mut check,
    );
    // On failure retain the admitted visit ceiling: FND does not expose partial visits.
    let visits = parsed.as_ref().map_or(limits.max_visits, |d| d.visits());
    c.debit_json_visits(visits)?;
    parsed.map(|d| d.into_root()).map_err(|e| {
        if e.code == FoundationErrorCode::BudgetExceeded {
            budget()
        } else {
            SchemaProbeError::InvalidPublishedJson(e.code)
        }
    })
}
fn inspect(
    schema: &JsonValue,
    c: &dyn SchemaProbeControl,
    depth: usize,
    limit: usize,
) -> Result<()> {
    work(c, size_of::<JsonValue>())?;
    if depth > limit {
        return Err(budget());
    }
    let entries = schema.as_object().ok_or_else(unsupported)?;
    for (key, value) in entries {
        work(c, key.as_str().ok_or_else(incompatible)?.len())?;
        match key.as_str().ok_or_else(incompatible)? {
            "$schema" => {
                if text(value)? != "https://json-schema.org/draft/2020-12/schema" {
                    return Err(SchemaProbeError::NotSchema202012);
                }
            }
            "$id" | "title" => {
                text(value)?;
            }
            "type" => {
                if !matches!(text(value)?, "object" | "array" | "string" | "boolean") {
                    return Err(unsupported());
                }
            }
            "required" => {
                let names = value.as_array().ok_or_else(unsupported)?;
                for (at, name) in names.iter().enumerate() {
                    text(name)?;
                    c.check()?;
                    for prior in &names[..at] {
                        if equal(prior, name, c)? {
                            return Err(unsupported());
                        }
                    }
                }
            }
            "properties" => {
                for (_, child) in value.as_object().ok_or_else(unsupported)? {
                    inspect(child, c, depth + 1, limit)?;
                }
            }
            "items" => inspect(value, c, depth + 1, limit)?,
            "const" => {
                if !scalar(value) {
                    return Err(unsupported());
                }
            }
            "enum" => {
                let values = value.as_array().ok_or_else(unsupported)?;
                if values.is_empty() {
                    return Err(unsupported());
                }
                for item in values {
                    c.check()?;
                    if !scalar(item) {
                        return Err(unsupported());
                    }
                }
            }
            "minLength" | "minItems" => {
                number(value, c)?;
            }
            "pattern" => {
                hex_width(text(value)?, c)?;
            }
            "additionalProperties" => {
                if value.as_bool() != Some(false) {
                    return Err(unsupported());
                }
            }
            "uniqueItems" => {
                value.as_bool().ok_or_else(unsupported)?;
                if value.as_bool() == Some(true) {
                    let items = lookup(schema, "items", c)?.ok_or_else(unsupported)?;
                    if lookup(items, "type", c)?.and_then(JsonValue::as_str) != Some("string") {
                        return Err(unsupported());
                    }
                }
            }
            _ => return Err(unsupported()),
        }
    }
    Ok(())
}
fn evaluate(
    schema: &JsonValue,
    v: &JsonValue,
    c: &dyn SchemaProbeControl,
    depth: usize,
    limit: usize,
) -> Result<bool> {
    work(c, size_of::<JsonValue>())?;
    if depth > limit {
        return Err(budget());
    }
    c.debit_json_visits(1)?;
    let mut frame = EvaluationFrame {
        entries: schema.as_object().ok_or_else(unsupported)?.iter(),
        values: [].iter(),
        count: 0,
        valid: true,
    };
    while let Some((key, rule)) = frame.entries.next() {
        work(c, key.as_str().ok_or_else(incompatible)?.len())?;
        match key.as_str().ok_or_else(incompatible)? {
            "$schema" | "$id" | "title" => {}
            "type" => {
                let valid = match text(rule)? {
                    "object" => matches!(v, JsonValue::Object(_)),
                    "array" => matches!(v, JsonValue::Array(_)),
                    "string" => matches!(v, JsonValue::String(_)),
                    "boolean" => matches!(v, JsonValue::Bool(_)),
                    _ => return Err(unsupported()),
                };
                if !valid {
                    return Ok(false);
                }
            }
            "const" => {
                if !equal(rule, v, c)? {
                    return Ok(false);
                }
            }
            "enum" => {
                frame.valid = false;
                for expected in rule.as_array().ok_or_else(unsupported)? {
                    if equal(expected, v, c)? {
                        frame.valid = true;
                        break;
                    }
                }
                if !frame.valid {
                    return Ok(false);
                }
            }
            "minLength" => {
                if let JsonValue::String(s) = v {
                    let min = number(rule, c)?;
                    let Some(s) = s.as_str() else {
                        return Err(SchemaProbeError::IncompatibleJsonRepresentation);
                    };
                    frame.count = 0;
                    work(c, s.len())?;
                    for _ in s.chars() {
                        c.check()?;
                        frame.count = add(frame.count, 1)?;
                    }
                    if frame.count < min {
                        return Ok(false);
                    }
                }
            }
            "pattern" => {
                if let JsonValue::String(s) = v {
                    let width = hex_width(text(rule)?, c)?;
                    let Some(s) = s.as_str() else {
                        return Err(incompatible());
                    };
                    work(c, s.len())?;
                    if s.len() != width
                        || !s
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    {
                        return Ok(false);
                    }
                }
            }
            "minItems" => {
                if let JsonValue::Array(a) = v {
                    if a.len() < number(rule, c)? {
                        return Ok(false);
                    }
                }
            }
            "items" => {
                if let JsonValue::Array(a) = v {
                    frame.values = a.iter();
                    for child in frame.values.by_ref() {
                        if !evaluate(rule, child, c, depth + 1, limit)? {
                            return Ok(false);
                        }
                    }
                }
            }
            "uniqueItems" => {
                if rule.as_bool() == Some(true) {
                    if let JsonValue::Array(a) = v {
                        for (i, left) in a.iter().enumerate() {
                            if !matches!(left, JsonValue::String(_)) {
                                return Ok(false);
                            }
                            for right in &a[i + 1..] {
                                if equal(left, right, c)? {
                                    return Ok(false);
                                }
                            }
                        }
                    }
                }
            }
            "required" => {
                if matches!(v, JsonValue::Object(_)) {
                    for name in rule.as_array().ok_or_else(unsupported)? {
                        if lookup(v, text(name)?, c)?.is_none() {
                            return Ok(false);
                        }
                    }
                }
            }
            "properties" => {
                if matches!(v, JsonValue::Object(_)) {
                    for (name, child_schema) in rule.as_object().ok_or_else(unsupported)? {
                        if let Some(child) = lookup(v, name.as_str().ok_or_else(incompatible)?, c)?
                        {
                            if !evaluate(child_schema, child, c, depth + 1, limit)? {
                                return Ok(false);
                            }
                        }
                    }
                }
            }
            "additionalProperties" => {
                if let JsonValue::Object(entries) = v {
                    let props = lookup(schema, "properties", c)?;
                    for (name, _) in entries {
                        if props.is_none()
                            || lookup(
                                props.ok_or_else(unsupported)?,
                                name.as_str().ok_or_else(incompatible)?,
                                c,
                            )?
                            .is_none()
                        {
                            return Ok(false);
                        }
                    }
                }
            }
            _ => return Err(unsupported()),
        }
    }
    c.check()?;
    Ok(true)
}
impl SchemaBackendProbe {
    pub fn new_controlled(
        resource: SchemaResource,
        profile: FormatProfile,
        limits: JsonLimits,
        c: &dyn SchemaProbeControl,
    ) -> Result<ControlledSchemaBackendProbe> {
        c.check()?;
        let frame = add(frames(limits.max_depth)?, size_of::<Digest256Hasher>())?;
        let owned = add(
            add(resource.uri.capacity(), resource.raw.capacity())?,
            size_of::<SchemaResource>(),
        )?;
        c.remaining(add(frame, owned)?)?;
        let mut hasher = Digest256Hasher::new();
        for chunk in resource.raw.chunks(65536) {
            work(c, chunk.len())?;
            hasher.update(chunk);
        }
        let raw_digest = hasher.finalize();
        let schema = parse(&resource.raw, limits, c, add(frame, owned)?)?;
        inspect(&schema, c, 0, limits.max_depth)?;
        if lookup(&schema, "$schema", c)?.and_then(JsonValue::as_str)
            != Some("https://json-schema.org/draft/2020-12/schema")
        {
            return Err(SchemaProbeError::NotSchema202012);
        }
        let id = lookup(&schema, "$id", c)?.ok_or(SchemaProbeError::InvalidResourceId)?;
        work(c, add(text(id)?.len(), resource.uri.len())?)?;
        if text(id)? != resource.uri || !resource.uri.starts_with("https://") {
            return Err(SchemaProbeError::InvalidResourceId);
        }
        work(c, resource.uri.len())?;
        if resource.uri.contains('#') {
            return Err(SchemaProbeError::InvalidResourceId);
        }
        let retained = add(
            add(
                size_of::<ControlledSchemaBackendProbe>(),
                resource.uri.capacity(),
            )?,
            storage(&schema, c, 0, limits.max_depth)?,
        )?;
        c.remaining(add(
            add(retained, frame)?,
            add(resource.raw.capacity(), size_of::<Vec<u8>>())?,
        )?)?;
        Ok(ControlledSchemaBackendProbe {
            uri: resource.uri,
            raw_digest,
            profile,
            schema,
            retained,
            depth: limits.max_depth,
        })
    }
}
impl ControlledSchemaBackendProbe {
    pub fn resource_digest(&self) -> Digest256 {
        self.raw_digest
    }
    pub fn profile(&self) -> FormatProfile {
        self.profile
    }
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        Ok(self.retained)
    }
    pub fn is_valid_raw(
        &self,
        root_uri: &str,
        raw: &[u8],
        limits: JsonLimits,
        c: &dyn SchemaProbeControl,
    ) -> Result<bool> {
        c.check()?;
        work(c, add(self.uri.len(), root_uri.len())?)?;
        if root_uri != self.uri {
            return Err(SchemaProbeError::MissingResource);
        }
        let limit = limits.max_depth.min(self.depth);
        let frame = add(frames(limit)?, size_of::<JsonValue>())?;
        let instance = parse(raw, limits, c, frame)?;
        c.remaining(add(frame, storage(&instance, c, 0, limits.max_depth)?)?)?;
        let result = evaluate(&self.schema, &instance, c, 0, limit);
        c.check()?;
        result
    }
}
