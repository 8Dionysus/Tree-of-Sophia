//! The current Questbook obligation and dispatch compatibility surface.
//! QUESTBOOK.md owns membership and follow-through; original records and
//! schemas retain their evidence/compatibility role. This is a read-only check.

use jsonschema::paths::LocationSegment;
use jsonschema::{Draft, Validator};
use num_bigint::BigInt;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tos_foundation::JsonLimits;
use unicode_general_category::{GeneralCategory, get_general_category};
use yaml_rust2::parser::{Event, Parser, Tag};
use yaml_rust2::scanner::TScalarStyle;

const INTEGRATION: &str =
    "mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md";
const QUEST_SCHEMA: &str = "mechanics/questbook/parts/dispatch-contracts/schemas/quest.schema.json";
const DISPATCH_SCHEMA: &str =
    "mechanics/questbook/parts/dispatch-contracts/schemas/quest_dispatch.schema.json";
const CATALOG: &str =
    "mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json";
const DISPATCH: &str =
    "mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json";
const QUEST_REQUIRED: &[&str] = &[
    "schema_version",
    "id",
    "title",
    "repo",
    "owner_surface",
    "kind",
    "state",
    "band",
    "difficulty",
    "risk",
    "control_mode",
    "delegate_tier",
    "write_scope",
    "activation",
    "anchor_ref",
    "evidence",
    "opened_at",
    "touched_at",
    "public_safe",
];
const DISPATCH_REQUIRED: &[&str] = &[
    "schema_version",
    "id",
    "repo",
    "state",
    "band",
    "difficulty",
    "risk",
    "control_mode",
    "delegate_tier",
    "split_required",
    "write_scope",
    "activation_mode",
    "public_safe",
];
const FORBIDDEN: &[&str] = &["ATM10-Agent", "aoa-sdk"];

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read(root: &Path, relative: &str) -> io::Result<Vec<u8>> {
    let path = root.join(relative);
    let file = File::open(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            invalid(format!("missing required file: {relative}"))
        } else {
            error
        }
    })?;
    let limit = JsonLimits::default().max_bytes;
    if file.metadata()?.len() > limit as u64 {
        return Err(invalid(format!(
            "questbook input exceeds JSON bound: {relative}"
        )));
    }
    let mut raw = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() > limit {
        return Err(invalid(format!(
            "questbook input exceeds JSON bound: {relative}"
        )));
    }
    Ok(raw)
}

fn text(root: &Path, relative: &str) -> io::Result<String> {
    String::from_utf8(read(root, relative)?)
        .map_err(|error| invalid(format!("invalid UTF-8 in {relative}: {error}")))
}

fn json_file(root: &Path, relative: &str) -> io::Result<Value> {
    serde_json::from_slice(&read(root, relative)?)
        .map_err(|error| invalid(format!("invalid JSON in {relative}: {error}")))
}

enum YNode {
    Scalar(Value),
    Sequence(Vec<Arc<YNode>>),
    Mapping(Vec<(Arc<YNode>, Arc<YNode>)>),
    Merge,
}

struct YBudget {
    visits: usize,
    bytes: usize,
}

impl YBudget {
    fn charge(&mut self, bytes: usize) -> io::Result<()> {
        self.visits = self
            .visits
            .checked_add(1)
            .ok_or_else(|| invalid("YAML visits overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("YAML size overflow"))?;
        let limits = JsonLimits::default();
        if self.visits > limits.max_visits || self.bytes > limits.max_bytes {
            return Err(invalid("quest YAML expansion exceeds foundation bound"));
        }
        Ok(())
    }
}

fn yaml_tag(tag: Option<&Tag>) -> io::Result<Option<&str>> {
    let Some(tag) = tag else { return Ok(None) };
    if tag.handle == "tag:yaml.org,2002:" {
        return Ok(Some(tag.suffix.as_str()));
    }
    if tag.handle.is_empty() && tag.suffix == "!" {
        return Ok(None);
    }
    Err(invalid("quest YAML has an unsafe or unknown tag"))
}

#[derive(Clone, Copy)]
enum ScalarKind {
    String,
    Null,
    Boolean,
    Integer,
    Float,
    Merge,
    Timestamp,
}

fn implicit_kind(raw: &str) -> ScalarKind {
    // Match the installed SafeLoader resolver on the original plain scalar.
    // Constructor cleanup (underscores and case) happens only after selection.
    static FLOAT: OnceLock<Regex> = OnceLock::new();
    static INTEGER: OnceLock<Regex> = OnceLock::new();
    static TIMESTAMP: OnceLock<Regex> = OnceLock::new();
    if matches!(
        raw,
        "yes"
            | "Yes"
            | "YES"
            | "no"
            | "No"
            | "NO"
            | "true"
            | "True"
            | "TRUE"
            | "false"
            | "False"
            | "FALSE"
            | "on"
            | "On"
            | "ON"
            | "off"
            | "Off"
            | "OFF"
    ) {
        return ScalarKind::Boolean;
    }
    if FLOAT.get_or_init(|| Regex::new(r"^(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?|\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$").expect("maintained float resolver")).is_match(raw) {
        return ScalarKind::Float;
    }
    if INTEGER.get_or_init(|| Regex::new(r"^(?:[-+]?0b[0-1_]+|[-+]?0[0-7_]+|[-+]?(?:0|[1-9][0-9_]*)|[-+]?0x[0-9a-fA-F_]+|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)$").expect("maintained integer resolver")).is_match(raw) {
        return ScalarKind::Integer;
    }
    if raw == "<<" {
        return ScalarKind::Merge;
    }
    if matches!(raw, "" | "~" | "null" | "Null" | "NULL") {
        return ScalarKind::Null;
    }
    if TIMESTAMP.get_or_init(|| Regex::new(r"^(?:[0-9]{4}-[0-9]{2}-[0-9]{2}|[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}(?:[Tt]|[ \t]+)[0-9]{1,2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]*)?(?:[ \t]*(?:Z|[-+][0-9]{1,2}(?::[0-9]{2})?))?)$").expect("maintained timestamp resolver")).is_match(raw) {
        return ScalarKind::Timestamp;
    }
    ScalarKind::String
}

fn integer_value(raw: &str) -> io::Result<Value> {
    fn normalized_digits(raw: &str) -> String {
        raw.chars()
            .map(|character| {
                if character.is_ascii() {
                    return character;
                }
                if get_general_category(character) != GeneralCategory::DecimalNumber {
                    return character;
                }
                // Unicode 16 Nd ranges are consecutive groups of ten decimal
                // values. Python int() accepts these in every positional base.
                let point = character as u32;
                let mut start = point;
                while let Some(previous) = start.checked_sub(1).and_then(char::from_u32) {
                    if get_general_category(previous) != GeneralCategory::DecimalNumber {
                        break;
                    }
                    start -= 1;
                }
                char::from(b'0' + ((point - start) % 10) as u8)
            })
            .collect()
    }

    fn radix_value(raw: &str, radix: u32) -> Option<BigInt> {
        BigInt::parse_bytes(normalized_digits(raw.trim()).as_bytes(), radix)
    }

    fn decimal_part(raw: &str) -> io::Result<BigInt> {
        // SafeConstructor calls Python int(part) here, not a machine-word
        // conversion. Its installed decimal string limit does not apply to
        // the binary/octal/hex constructors.
        let trimmed = raw.trim();
        let (negative, digits) = if let Some(rest) = trimmed.strip_prefix('-') {
            (true, rest)
        } else {
            (false, trimmed.strip_prefix('+').unwrap_or(trimmed))
        };
        let normalized = normalized_digits(digits);
        if normalized.is_empty() || !normalized.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid("invalid explicit YAML integer"));
        }
        if normalized.len() > JsonLimits::default().max_integer_digits {
            return Err(invalid("YAML decimal integer exceeds Python digit limit"));
        }
        let value = BigInt::parse_bytes(normalized.as_bytes(), 10)
            .ok_or_else(|| invalid("invalid explicit YAML integer"))?;
        Ok(if negative { -value } else { value })
    }

    let cleaned = raw.replace('_', "");
    let (negative, digits) = if let Some(rest) = cleaned.strip_prefix('-') {
        (true, rest)
    } else {
        (false, cleaned.strip_prefix('+').unwrap_or(&cleaned))
    };
    let magnitude = if let Some(binary) = digits.strip_prefix("0b") {
        radix_value(binary, 2)
    } else if let Some(hex) = digits.strip_prefix("0x") {
        radix_value(hex, 16)
    } else if digits.starts_with('0') && digits.len() > 1 {
        radix_value(&digits, 8)
    } else if digits.contains(':') {
        let mut total = BigInt::from(0_u8);
        for part in digits.split(':') {
            total = total * 60_u32 + decimal_part(part)?;
        }
        Some(total)
    } else {
        Some(decimal_part(&digits)?)
    }
    .ok_or_else(|| invalid("invalid explicit YAML integer"))?;
    let decimal = if negative {
        (-magnitude).to_string()
    } else {
        magnitude.to_string()
    };
    let value: Value = serde_json::from_str(&decimal)
        .map_err(|_| invalid("exact YAML integer needs serde_json arbitrary_precision"))?;
    if value
        .as_number()
        .is_none_or(|number| number.to_string() != decimal)
    {
        return Err(invalid(
            "exact YAML integer needs jsonschema arbitrary-precision",
        ));
    }
    Ok(value)
}

fn float_value(raw: &str) -> io::Result<Value> {
    let value = raw.replace('_', "").to_ascii_lowercase();
    let (sign, unsigned) = if let Some(rest) = value.strip_prefix('-') {
        (-1.0, rest)
    } else {
        (1.0, value.strip_prefix('+').unwrap_or(&value))
    };
    if matches!(unsigned, ".inf" | ".nan") {
        return Err(invalid(
            "quest YAML non-finite float has no JSON representation",
        ));
    }
    let number = if unsigned.contains(':') {
        let mut number = 0.0;
        for part in unsigned.split(':') {
            number = number * 60.0
                + part
                    .parse::<f64>()
                    .map_err(|_| invalid("invalid YAML sexagesimal float"))?;
        }
        sign * number
    } else {
        value
            .parse::<f64>()
            .map_err(|_| invalid("invalid explicit YAML float"))?
    };
    serde_json::Number::from_f64(number)
        .map(Value::Number)
        .ok_or_else(|| invalid("quest YAML float has no JSON representation"))
}

fn yaml_scalar(raw: String, style: TScalarStyle, tag: Option<Tag>) -> io::Result<YNode> {
    let explicit = yaml_tag(tag.as_ref())?;
    let kind = match explicit {
        Some("str" | "value") => ScalarKind::String,
        Some("null") => ScalarKind::Null,
        Some("bool") => ScalarKind::Boolean,
        Some("int") => ScalarKind::Integer,
        Some("float") => ScalarKind::Float,
        Some("merge") => ScalarKind::Merge,
        Some("timestamp") => ScalarKind::Timestamp,
        Some(_) => return Err(invalid("quest YAML tag has no JSON representation")),
        None if style == TScalarStyle::Plain => implicit_kind(&raw),
        None => ScalarKind::String,
    };
    match kind {
        ScalarKind::String => Ok(YNode::Scalar(Value::String(raw))),
        ScalarKind::Null => Ok(YNode::Scalar(Value::Null)),
        ScalarKind::Boolean => {
            let value = match raw.to_ascii_lowercase().as_str() {
                "yes" | "true" | "on" => true,
                "no" | "false" | "off" => false,
                _ => return Err(invalid("invalid explicit YAML boolean")),
            };
            Ok(YNode::Scalar(Value::Bool(value)))
        }
        ScalarKind::Integer => integer_value(&raw).map(YNode::Scalar),
        ScalarKind::Float => float_value(&raw).map(YNode::Scalar),
        ScalarKind::Merge => Ok(YNode::Merge),
        ScalarKind::Timestamp => Err(invalid("quest YAML timestamp has no JSON representation")),
    }
}

fn next_yaml<'a>(parser: &mut Parser<std::str::Chars<'a>>) -> io::Result<Event> {
    parser
        .next_token()
        .map(|(event, _)| event)
        .map_err(|error| invalid(format!("invalid YAML: {error}")))
}

fn yaml_node<'a>(
    event: Event,
    parser: &mut Parser<std::str::Chars<'a>>,
    anchors: &mut std::collections::BTreeMap<usize, Arc<YNode>>,
    budget: &mut YBudget,
    depth: usize,
) -> io::Result<Arc<YNode>> {
    if depth > JsonLimits::default().max_depth {
        return Err(invalid("quest YAML depth exceeds foundation bound"));
    }
    budget.charge(0)?;
    let (node, anchor) = match event {
        Event::Scalar(raw, style, anchor, tag) => {
            budget.charge(raw.len())?;
            (yaml_scalar(raw, style, tag)?, anchor)
        }
        Event::SequenceStart(anchor, tag) => {
            if !matches!(yaml_tag(tag.as_ref())?, None | Some("seq")) {
                return Err(invalid("quest YAML sequence tag is unsupported"));
            }
            let mut children = Vec::new();
            loop {
                let event = next_yaml(parser)?;
                if event == Event::SequenceEnd {
                    break;
                }
                children.push(yaml_node(event, parser, anchors, budget, depth + 1)?);
            }
            (YNode::Sequence(children), anchor)
        }
        Event::MappingStart(anchor, tag) => {
            if !matches!(yaml_tag(tag.as_ref())?, None | Some("map")) {
                return Err(invalid("quest YAML mapping tag is unsupported"));
            }
            let mut pairs = Vec::new();
            loop {
                let event = next_yaml(parser)?;
                if event == Event::MappingEnd {
                    break;
                }
                let key = yaml_node(event, parser, anchors, budget, depth + 1)?;
                let value = yaml_node(next_yaml(parser)?, parser, anchors, budget, depth + 1)?;
                pairs.push((key, value));
            }
            (YNode::Mapping(pairs), anchor)
        }
        Event::Alias(anchor) => {
            return anchors
                .get(&anchor)
                .cloned()
                .ok_or_else(|| invalid("quest YAML alias is unresolved or recursive"));
        }
        _ => return Err(invalid("unexpected quest YAML event")),
    };
    let node = Arc::new(node);
    if anchor != 0 {
        anchors.insert(anchor, Arc::clone(&node));
    }
    Ok(node)
}

fn yaml_value(node: &YNode, budget: &mut YBudget, depth: usize) -> io::Result<Value> {
    if depth > JsonLimits::default().max_depth {
        return Err(invalid("quest YAML depth exceeds foundation bound"));
    }
    budget.charge(0)?;
    match node {
        YNode::Scalar(value) => {
            if let Some(value) = value.as_str() {
                budget.charge(value.len())?;
            } else if let Some(number) = value.as_number() {
                budget.charge(number.to_string().len())?;
            }
            Ok(value.clone())
        }
        YNode::Sequence(items) => items
            .iter()
            .map(|item| yaml_value(item, budget, depth + 1))
            .collect::<io::Result<Vec<_>>>()
            .map(Value::Array),
        YNode::Mapping(items) => {
            let mut result = Map::new();
            // PyYAML SafeConstructor prepends flattened merges. In a sequence,
            // the first mapping wins; explicit keys then overwrite all merges.
            for (key, value) in items {
                if !matches!(key.as_ref(), YNode::Merge) {
                    continue;
                }
                let sources: Vec<&YNode> = match value.as_ref() {
                    YNode::Mapping(_) => vec![value.as_ref()],
                    YNode::Sequence(maps) => maps.iter().rev().map(Arc::as_ref).collect(),
                    _ => {
                        return Err(invalid(
                            "quest YAML merge needs mapping or list of mappings",
                        ));
                    }
                };
                for source in sources {
                    let YNode::Mapping(_) = source else {
                        return Err(invalid("quest YAML merge needs mapping"));
                    };
                    let Value::Object(map) = yaml_value(source, budget, depth + 1)? else {
                        return Err(invalid("quest YAML merge needs mapping"));
                    };
                    for (key, value) in map {
                        budget.charge(key.len())?;
                        result.insert(key, value);
                    }
                }
            }
            for (key, value) in items {
                if matches!(key.as_ref(), YNode::Merge) {
                    continue;
                }
                let YNode::Scalar(Value::String(key)) = key.as_ref() else {
                    return Err(invalid(
                        "quest YAML mapping key has no JSON object representation",
                    ));
                };
                budget.charge(key.len())?;
                result.insert(key.clone(), yaml_value(value, budget, depth + 1)?);
            }
            Ok(Value::Object(result))
        }
        YNode::Merge => Err(invalid("quest YAML merge key outside mapping")),
    }
}

fn yaml_file(root: &Path, relative: &str) -> io::Result<Value> {
    let raw = read(root, relative)?;
    let source = std::str::from_utf8(&raw)
        .map_err(|error| invalid(format!("invalid UTF-8 in {relative}: {error}")))?;
    let mut parser = Parser::new_from_str(source);
    if next_yaml(&mut parser)? != Event::StreamStart
        || next_yaml(&mut parser)? != Event::DocumentStart
    {
        return Err(invalid(format!(
            "invalid YAML in {relative}: expected one document"
        )));
    }
    let mut parse_budget = YBudget {
        visits: 0,
        bytes: 0,
    };
    let mut anchors = std::collections::BTreeMap::new();
    let node = yaml_node(
        next_yaml(&mut parser)?,
        &mut parser,
        &mut anchors,
        &mut parse_budget,
        0,
    )?;
    if next_yaml(&mut parser)? != Event::DocumentEnd || next_yaml(&mut parser)? != Event::StreamEnd
    {
        return Err(invalid(format!(
            "invalid YAML in {relative}: expected one document"
        )));
    }
    let mut value_budget = YBudget {
        visits: 0,
        bytes: 0,
    };
    yaml_value(&node, &mut value_budget, 0)
}

fn object<'a>(value: &'a Value, label: &str) -> io::Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be a JSON object")))
}

fn field<'a>(value: &'a Value, name: &str, label: &str) -> io::Result<&'a Value> {
    value
        .get(name)
        .ok_or_else(|| invalid(format!("{label} lacks {name}")))
}

fn schema_envelope(
    schema: &Value,
    label: &str,
    title: &str,
    version: &str,
    required: &[&str],
) -> io::Result<()> {
    let value = object(schema, label)?;
    if value.get("title").and_then(Value::as_str) != Some(title) {
        return Err(invalid(format!("{label} title must equal '{title}'")));
    }
    if value.get("type").and_then(Value::as_str) != Some("object") {
        return Err(invalid(format!("{label} type must equal 'object'")));
    }
    if value.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Err(invalid(format!(
            "{label} must set additionalProperties to false"
        )));
    }
    let required_values: Vec<Value> = required.iter().map(|item| json!(item)).collect();
    if value.get("required") != Some(&Value::Array(required_values)) {
        return Err(invalid(format!(
            "{label} required fields must stay aligned with the local quest contract"
        )));
    }
    let Some(properties) = value.get("properties").and_then(Value::as_object) else {
        return Err(invalid(format!("{label} properties must be an object")));
    };
    if properties
        .get("schema_version")
        .and_then(Value::as_object)
        .and_then(|item| item.get("const"))
        .and_then(Value::as_str)
        != Some(version)
    {
        return Err(invalid(format!(
            "{label} schema_version.const must equal '{version}'"
        )));
    }
    Ok(())
}

fn validator(schema: &Value, label: &str) -> io::Result<Validator> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        // The owner calls Draft202012Validator without FormatChecker.
        .should_validate_formats(false)
        .offline()
        .build(schema)
        .map_err(|error| invalid(format!("{label} is not a valid JSON Schema: {error}")))
}

fn validate(value: &Value, validator: &Validator, payload: &str, schema: &str) -> io::Result<()> {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum PathPart {
        Index(usize),
        Property(String),
    }
    let mut first: Option<(Vec<PathPart>, String, String)> = None;
    for error in validator.iter_errors(value) {
        let segments: Vec<_> = error.instance_path().segments().collect();
        let path = segments
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(".");
        let order: Vec<_> = segments
            .into_iter()
            .map(|segment| match segment {
                LocationSegment::Index(index) => PathPart::Index(index),
                LocationSegment::Property(name) => PathPart::Property(name.into_owned()),
            })
            .collect();
        if first
            .as_ref()
            .is_none_or(|(selected, _, _)| order < *selected)
        {
            first = Some((order, path, error.to_string()));
        }
    }
    if let Some((_, path, message)) = first {
        let location = if path.is_empty() { "$" } else { &path };
        return Err(invalid(format!(
            "{payload} violates {schema} at {location}: {message}"
        )));
    }
    Ok(())
}

fn catalog_entry(id: &str, quest: &Value) -> io::Result<Value> {
    let label = format!("quest {id}");
    Ok(json!({
        "id": id,
        "title": field(quest, "title", &label)?,
        "repo": field(quest, "repo", &label)?,
        "theme_ref": quest.get("theme_ref").cloned().unwrap_or(json!("")),
        "milestone_ref": quest.get("milestone_ref").cloned().unwrap_or(json!("")),
        "state": field(quest, "state", &label)?,
        "band": field(quest, "band", &label)?,
        "kind": field(quest, "kind", &label)?,
        "difficulty": field(quest, "difficulty", &label)?,
        "risk": field(quest, "risk", &label)?,
        "owner_surface": field(quest, "owner_surface", &label)?,
        "source_path": format!("quests/{id}.yaml"),
        "public_safe": field(quest, "public_safe", &label)?,
    }))
}

pub(crate) fn dispatch_entry(id: &str, quest: &Value) -> io::Result<Value> {
    let label = format!("quest {id}");
    let activation = quest
        .get("activation")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(format!("quest {id} activation must be an object")))?;
    let mode = activation
        .get("mode")
        .and_then(Value::as_str)
        .filter(|mode| !mode.is_empty())
        .ok_or_else(|| {
            invalid(format!(
                "quest {id} activation.mode must be a non-empty string"
            ))
        })?;
    let artifacts = if quest.get("kind").and_then(Value::as_str) == Some("guardrail") {
        vec!["bounded_plan", "guardrail_check", "verification_result"]
    } else {
        vec!["bounded_plan", "work_result", "verification_result"]
    };
    Ok(json!({
        "schema_version": "quest_dispatch_v1", "id": id,
        "repo": field(quest, "repo", &label)?,
        "state": field(quest, "state", &label)?,
        "band": field(quest, "band", &label)?,
        "difficulty": field(quest, "difficulty", &label)?,
        "risk": field(quest, "risk", &label)?,
        "control_mode": field(quest, "control_mode", &label)?,
        "delegate_tier": field(quest, "delegate_tier", &label)?,
        "split_required": field(quest, "split_required", &label)?,
        "write_scope": field(quest, "write_scope", &label)?,
        "requires_artifacts": artifacts,
        "activation_mode": mode,
        "source_path": format!("quests/{id}.yaml"),
        "public_safe": field(quest, "public_safe", &label)?,
        "fallback_tier": field(quest, "fallback_tier", &label)?,
        "wrapper_class": field(quest, "wrapper_class", &label)?,
    }))
}

#[cfg(test)]
#[test]
fn dispatch_entry_reports_missing_activation_mode_before_projection_fields() {
    // Retains the package-local direct-helper assertion independently of the
    // full surface/schema check above; no unchecked field access may panic.
    let error = dispatch_entry("TOS-Q-0001", &json!({"activation": {}})).unwrap_err();
    assert!(error.to_string().contains("activation.mode"));
}

/// The existing public register owns membership and follow-through. Its
/// bounded data block is independent of headings, sample IDs and task count.
fn obligations(root: &Path, document: &str) -> io::Result<Vec<Value>> {
    let marker = "```tos-obligations-v1\n";
    let (before, tail) = document
        .split_once(marker)
        .ok_or_else(|| invalid("QUESTBOOK.md lacks obligation register"))?;
    let (raw, after) = tail
        .split_once("\n```")
        .ok_or_else(|| invalid("unterminated obligation register"))?;
    if before.contains(marker) || after.contains(marker) {
        return Err(invalid("duplicate obligation register"));
    }
    // Published membership cannot hide a duplicate key behind serde's last
    // value wins behavior. Keep foundation depth/visit/byte limits unchanged.
    tos_foundation::parse_json(
        raw.as_bytes(),
        tos_foundation::JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .map_err(|error| invalid(format!("invalid obligation register: {error}")))?;
    let register: Value = serde_json::from_str(raw)
        .map_err(|error| invalid(format!("invalid obligation register: {error}")))?;
    if register.get("schema_version").and_then(Value::as_str) != Some("tos_obligations_v1") {
        return Err(invalid("unsupported obligation register schema"));
    }
    for field in ["runtime_execution", "canon_admission"] {
        if register.get(field) != Some(&Value::Bool(false)) {
            return Err(invalid(format!(
                "obligation register {field} must remain false"
            )));
        }
    }
    let rows = register
        .get("records")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("obligation register records must be an array"))?;
    let mut ids = std::collections::BTreeSet::new();
    let mut sources = std::collections::BTreeSet::new();
    let mut source_bytes = document.len();
    let id_pattern = Regex::new(r"^TOS-Q-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*$")
        .map_err(|error| invalid(error.to_string()))?;
    for row in rows {
        let value = object(row, "obligation")?;
        let id = value.get("id").and_then(Value::as_str).unwrap_or("");
        if !id_pattern.is_match(id) || !ids.insert(id) {
            return Err(invalid("invalid or duplicate obligation id"));
        }
        if !matches!(
            value.get("state").and_then(Value::as_str),
            Some(
                "captured"
                    | "triaged"
                    | "ready"
                    | "active"
                    | "blocked"
                    | "reanchor"
                    | "deferred"
                    | "review_pending"
                    | "done"
                    | "dropped"
            )
        ) {
            return Err(invalid(format!("{id} lacks explicit obligation state")));
        }
        for field in ["owner_ref", "source_return", "record_ref"] {
            let reference = value.get(field).and_then(Value::as_str).unwrap_or("");
            let path = Path::new(reference);
            if reference.is_empty()
                || !path
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_)))
                || (field == "source_return" && !reference.starts_with("ToS/"))
            {
                return Err(invalid(format!("{id} invalid {field}")));
            }
            let target = root.join(path);
            if fs::canonicalize(&target).ok().as_ref() != Some(&target) {
                return Err(invalid(format!(
                    "{id} missing or unsafe {field}: {reference}"
                )));
            }
            if sources.insert(reference) {
                // Dynamic membership charges each distinct source once. The
                // legacy phase rereads only registered record bytes, bounded
                // by this same aggregate, plus four fixed schema/projections.
                let bytes = read(root, reference)?;
                source_bytes = source_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(|| invalid("obligation source byte overflow"))?;
                if source_bytes > JsonLimits::default().max_bytes {
                    return Err(invalid("obligation sources exceed aggregate input bound"));
                }
                if std::str::from_utf8(&bytes)
                    .map_err(|_| invalid("non-UTF-8 obligation source"))?
                    .trim()
                    .is_empty()
                {
                    return Err(invalid(format!("{id} empty {field}: {reference}")));
                }
            }
        }
        if value
            .get("return_when")
            .and_then(Value::as_str)
            .is_none_or(|s| s.trim().is_empty())
        {
            return Err(invalid(format!("{id} lacks return condition")));
        }
        let history = value
            .get("history_ref")
            .and_then(Value::as_str)
            .unwrap_or("");
        let prefix = "https://github.com/8Dionysus/Tree-of-Sophia/blob/";
        let history_tail = history.strip_prefix(prefix).unwrap_or("");
        let (revision, historical_path) = history_tail.split_once('/').unwrap_or(("", ""));
        if revision.len() != 40
            || !revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || historical_path != value["record_ref"].as_str().unwrap_or("")
        {
            return Err(invalid(format!(
                "{id} lacks exact immutable record history"
            )));
        }
        if !matches!(
            value.get("record_kind").and_then(Value::as_str),
            Some("legacy_yaml" | "authored_note")
        ) {
            return Err(invalid(format!("{id} has unsupported record kind")));
        }
    }
    // Removing a register row cannot silently abandon a retained record.
    // An absent/empty store is valid; its membership is never a fixed quartet.
    let record_refs: std::collections::BTreeSet<_> = rows
        .iter()
        .filter_map(|row| row["record_ref"].as_str())
        .collect();
    let store = root.join("quests");
    match fs::symlink_metadata(&store) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err(invalid("obligation store must be a regular directory"));
        }
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    match fs::read_dir(&store) {
        Ok(entries) => {
            for (index, entry) in entries.enumerate() {
                if index >= JsonLimits::default().max_visits {
                    return Err(invalid("obligation store entry bound exceeded"));
                }
                let entry = entry?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid("non-UTF-8 obligation filename"))?;
                if name.starts_with("TOS-Q-") && (name.ends_with(".yaml") || name.ends_with(".md"))
                {
                    let reference = format!("quests/{name}");
                    if !record_refs.contains(reference.as_str()) {
                        return Err(invalid(format!(
                            "unregistered retained obligation: {reference}"
                        )));
                    }
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    Ok(rows.clone())
}

/// Validate obligation membership first. Retained schema/examples are a narrow
/// compatibility consumer, optional for a register without legacy YAML records.
/// A pass accepts neither an obligation, source text, rights nor canon.
pub fn validate_surface(root: &Path) -> io::Result<()> {
    if !root.is_absolute() || fs::canonicalize(root)? != root {
        return Err(invalid(
            "repository root must be an absolute path without symlinks",
        ));
    }
    if text(root, INTEGRATION)?.trim().is_empty() {
        return Err(invalid("obligation integration boundary is empty"));
    }
    let records = obligations(root, &text(root, "QUESTBOOK.md")?)?;
    let legacy: Vec<_> = records
        .iter()
        .filter(|r| r["record_kind"] == "legacy_yaml")
        .collect();
    if legacy.is_empty() {
        // No example quartet is a prerequisite for a complete core register.
        for path in [CATALOG, DISPATCH] {
            if root.join(path).exists() && json_file(root, path)? != json!([]) {
                return Err(invalid(format!(
                    "{path} must stay aligned with registered legacy records"
                )));
            }
        }
        return Ok(());
    }
    let quest_schema = json_file(root, QUEST_SCHEMA)?;
    schema_envelope(
        &quest_schema,
        QUEST_SCHEMA,
        "Tree-of-Sophia work_quest_v1",
        "work_quest_v1",
        QUEST_REQUIRED,
    )?;
    let dispatch_schema = json_file(root, DISPATCH_SCHEMA)?;
    schema_envelope(
        &dispatch_schema,
        DISPATCH_SCHEMA,
        "Tree-of-Sophia quest_dispatch_v1",
        "quest_dispatch_v1",
        DISPATCH_REQUIRED,
    )?;
    let quest_validator = validator(&quest_schema, QUEST_SCHEMA)?;
    let dispatch_validator = validator(&dispatch_schema, DISPATCH_SCHEMA)?;
    let mut catalog = Vec::new();
    let mut dispatch = Vec::new();
    for record in legacy {
        let id = record["id"]
            .as_str()
            .ok_or_else(|| invalid("missing obligation id"))?;
        let path = format!("quests/{id}.yaml");
        if record["record_ref"] != path {
            return Err(invalid(format!(
                "{id} legacy record path must equal '{path}'"
            )));
        }
        let quest = yaml_file(root, &path)?;
        validate(&quest, &quest_validator, &path, QUEST_SCHEMA)?;
        if quest.get("id").and_then(Value::as_str) != Some(id) {
            return Err(invalid(format!("{path} id must equal '{id}'")));
        }
        if quest.get("repo").and_then(Value::as_str) != Some("Tree-of-Sophia") {
            return Err(invalid(format!("{id} repo must equal 'Tree-of-Sophia'")));
        }
        if quest.get("public_safe") != Some(&Value::Bool(true)) {
            return Err(invalid(format!("{id} public_safe must be true")));
        }
        if quest.get("state") != record.get("state") {
            return Err(invalid(format!(
                "{id} legacy state differs from obligation register"
            )));
        }
        let notes = quest
            .get("notes")
            .map(Value::as_str)
            .unwrap_or(Some(""))
            .ok_or_else(|| invalid(format!("{id} notes must be a string")))?;
        if FORBIDDEN.iter().any(|token| notes.contains(token)) {
            return Err(invalid(format!(
                "{id} notes must stay in scope for the current contour"
            )));
        }
        catalog.push(catalog_entry(id, &quest)?);
        let derived = dispatch_entry(id, &quest)?;
        validate(
            &derived,
            &dispatch_validator,
            &format!("derived dispatch entry for {id}"),
            DISPATCH_SCHEMA,
        )?;
        dispatch.push(derived);
    }
    if root.join(CATALOG).exists() && json_file(root, CATALOG)? != Value::Array(catalog) {
        return Err(invalid(format!(
            "{CATALOG} must stay aligned with registered legacy records"
        )));
    }
    if root.join(DISPATCH).exists() {
        let actual = json_file(root, DISPATCH)?;
        let rows = actual
            .as_array()
            .ok_or_else(|| invalid(format!("{DISPATCH} must be a JSON array")))?;
        for (index, row) in rows.iter().enumerate() {
            validate(
                row,
                &dispatch_validator,
                &format!("{DISPATCH}[{index}]"),
                DISPATCH_SCHEMA,
            )?;
        }
        if actual != Value::Array(dispatch) {
            return Err(invalid(format!(
                "{DISPATCH} must stay aligned with registered legacy records"
            )));
        }
    }
    Ok(())
}
