//! Browser human-form delivery rules.
//!
//! These checks validate a delivered selection against its exact material
//! revision and source-owned form references. They do not assess wording,
//! grant rights, authorize publication, or admit material to canon.

use tos_foundation::{
    emit_value_preserved_json, parse_json, JsonLimits, JsonMode, JsonNumber, JsonNumberKind,
    JsonString, JsonValue,
};

const ROLES: [&str; 7] = [
    "name",
    "caption",
    "hover",
    "statement",
    "grounds",
    "history",
    "technical",
];
const STATES: [&str; 5] = [
    "ready",
    "missing",
    "unavailable",
    "ambiguous",
    "over-budget",
];
const REASONS: [&str; 9] = [
    "exact-language",
    "less-specific-language",
    "automatic",
    "fallback",
    "original",
    "no-ready-form",
    "multiple-forms",
    "original-role-not-declared",
    "inspect-exact-form",
];
const CANDIDATE_STATES: [&str; 7] = [
    "ready",
    "invalid",
    "unavailable",
    "stale",
    "restricted",
    "needs-assessment",
    "over-budget",
];
const SELECTION_BYTES: usize = 512 * 1024;
const PACKET_BYTES: usize = 64 * 1024;
const IDENTITY_BYTES: usize = 16 * 1024;
const STRUCTURAL_VISITS: usize = 30_000;

type RuleResult<T> = Result<T, &'static str>;

fn limits(max_bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes,
        max_depth: 64,
        max_visits: STRUCTURAL_VISITS,
        ..Default::default()
    }
}

fn parse(raw: &[u8], max_bytes: usize, mode: JsonMode) -> RuleResult<JsonValue> {
    parse_json(raw, mode, limits(max_bytes))
        .map(|document| document.into_root())
        .map_err(|_| "invalid_human_form_json")
}

fn object(value: &JsonValue) -> Option<&[(JsonString, JsonValue)]> {
    value.as_object()
}

fn array(value: &JsonValue) -> Option<&[JsonValue]> {
    value.as_array()
}

fn str_eq(value: &JsonValue, expected: &str) -> bool {
    value.as_str() == Some(expected)
}

fn has(value: &JsonValue, key: &str) -> bool {
    value.object_get(key).is_some()
}

fn nonempty_string(value: &JsonValue) -> bool {
    matches!(value, JsonValue::String(value) if !value.units().is_empty())
}

fn ascii_alpha(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn ascii_alnum(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn content_language(value: &str) -> bool {
    if value.encode_utf16().count() > 128 {
        return false;
    }
    if matches!(value, "auto" | "original") {
        return true;
    }
    let mut parts = value.split('-');
    let Some(first) = parts.next() else {
        return false;
    };
    if first.len() >= 2 && first.len() <= 8 && ascii_alpha(first) {
        return parts.all(|part| part.len() <= 8 && ascii_alnum(part));
    }
    if matches!(first, "i" | "I" | "x" | "X") {
        let remaining: Vec<_> = parts.collect();
        return !remaining.is_empty()
            && remaining
                .iter()
                .all(|part| part.len() <= 8 && ascii_alnum(part));
    }
    false
}

fn language(value: &JsonValue) -> bool {
    if value.is_null() {
        return true;
    }
    let Some(text) = value.as_str() else {
        return false;
    };
    content_language(text) && !matches!(text, "auto" | "original")
}

fn valid_hash(value: &JsonValue) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn safe_integer(value: &JsonValue) -> Option<i64> {
    let JsonValue::Number(number) = value else {
        return None;
    };
    let parsed = number.lexeme.parse::<f64>().ok()?;
    if !parsed.is_finite()
        || parsed.fract() != 0.0
        || parsed.abs() > 9_007_199_254_740_991.0
        || parsed < i64::MIN as f64
        || parsed > i64::MAX as f64
    {
        return None;
    }
    Some(parsed as i64)
}

fn exact_ref(value: &JsonValue) -> bool {
    let Some(entries) = object(value) else {
        return false;
    };
    entries.len() == 3
        && nonempty_string(&value.object_get("id").cloned().unwrap_or(JsonValue::Null))
        && safe_integer(value.object_get("version").unwrap_or(&JsonValue::Null))
            .is_some_and(|version| version >= 1)
        && value
            .object_get("digest")
            .and_then(JsonValue::as_str)
            .is_some_and(|digest| {
                digest.len() == 71
                    && digest.starts_with("sha256:")
                    && digest[7..]
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
}

fn same_ref(left: &JsonValue, right: &JsonValue) -> bool {
    if !exact_ref(left) || !exact_ref(right) {
        return false;
    }
    let (Some(JsonValue::String(left_id)), Some(JsonValue::String(right_id))) =
        (left.object_get("id"), right.object_get("id"))
    else {
        return false;
    };
    let left_version = safe_integer(left.object_get("version").unwrap_or(&JsonValue::Null));
    let right_version = safe_integer(right.object_get("version").unwrap_or(&JsonValue::Null));
    left_id.units() == right_id.units()
        && left_version == right_version
        && left.object_get("digest") == right.object_get("digest")
}

fn generic_pointer(value: &JsonValue) -> bool {
    let JsonValue::String(pointer) = value else {
        return false;
    };
    let units = pointer.units();
    if units.len() > 2048 {
        return false;
    }
    let mut at = 0;
    while at < units.len() {
        if units[at] != b'/' as u16 {
            return false;
        }
        at += 1;
        while at < units.len() && units[at] != b'/' as u16 {
            if units[at] == b'~' as u16 {
                at += 1;
                if at >= units.len() || (units[at] != b'0' as u16 && units[at] != b'1' as u16) {
                    return false;
                }
            }
            at += 1;
        }
    }
    true
}

fn source_pointer_digits(value: &JsonValue) -> Option<&str> {
    let text = value.as_str()?;
    let tail = text.strip_prefix("/attributes/human_forms/")?;
    let digits = if tail.bytes().all(|byte| byte.is_ascii_digit()) {
        tail
    } else {
        tail.strip_suffix('\n')
            .or_else(|| tail.strip_suffix('\r'))
            .or_else(|| tail.strip_suffix('\u{2028}'))
            .or_else(|| tail.strip_suffix('\u{2029}'))?
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(digits)
}

fn source_pointer_index(value: &JsonValue) -> Option<usize> {
    source_pointer_digits(value)?.parse().ok()
}

fn js_truthy(value: Option<&JsonValue>) -> bool {
    match value {
        None | Some(JsonValue::Null) | Some(JsonValue::Bool(false)) => false,
        Some(JsonValue::Number(number)) => number
            .lexeme
            .parse::<f64>()
            .is_ok_and(|number| number != 0.0),
        Some(JsonValue::String(value)) => !value.units().is_empty(),
        Some(JsonValue::Bool(true) | JsonValue::Array(_) | JsonValue::Object(_)) => true,
    }
}

fn same_primitive(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::String(a), JsonValue::String(b)) => a.units() == b.units(),
        (JsonValue::Bool(a), JsonValue::Bool(b)) => a == b,
        (JsonValue::Number(a), JsonValue::Number(b)) => {
            a.lexeme.parse::<f64>().ok() == b.lexeme.parse::<f64>().ok()
        }
        (JsonValue::Null, JsonValue::Null) => true,
        _ => false,
    }
}

fn json_string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn json_number(value: i64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

fn field<'a>(value: &'a JsonValue, key: &str) -> &'a JsonValue {
    value.object_get(key).unwrap_or(&JsonValue::Null)
}

fn nullable_pick<'a>(
    left: Option<&'a JsonValue>,
    right: Option<&'a JsonValue>,
) -> Option<&'a JsonValue> {
    match left {
        Some(JsonValue::Null) | None => right,
        other => other,
    }
}

fn source_subject(source: &JsonValue) -> RuleResult<Option<JsonValue>> {
    let attributes = source.object_get("attributes");
    let Some(attributes) = attributes.filter(|value| object(value).is_some()) else {
        return Ok(None);
    };
    let claim = attributes
        .object_get("source_claim")
        .filter(|value| object(value).is_some());
    let record = claim.or_else(|| {
        attributes
            .object_get("source_record")
            .filter(|value| object(value).is_some())
    });
    let Some(record) = record else {
        return Ok(None);
    };
    let mut id = nullable_pick(
        claim.and_then(|value| value.object_get("claim_id")),
        nullable_pick(
            record.object_get("record_id"),
            nullable_pick(
                record.object_get("composite_id"),
                record.object_get("artifact_id"),
            ),
        ),
    );
    let schema = record
        .object_get("schema_version")
        .and_then(JsonValue::as_str);
    if schema == Some("tos_canonical_node_v1") {
        let node_type = record
            .object_get("node_type")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        let valid_type = [
            "source",
            "concept",
            "principle",
            "lineage",
            "event",
            "state",
            "support",
            "context",
            "analogy",
            "synthesis",
        ]
        .contains(&node_type);
        let node_id = record.object_get("node_id").and_then(JsonValue::as_str);
        let valid_node_id = node_id.is_some_and(|value| {
            let prefix = format!("tos.{node_type}.");
            value.starts_with(&prefix)
                && value.strip_prefix("tos.").is_some_and(|tail| {
                    let Some((kind, suffix)) = tail.split_once('.') else {
                        return false;
                    };
                    [
                        "source",
                        "concept",
                        "principle",
                        "lineage",
                        "event",
                        "state",
                        "support",
                        "context",
                        "analogy",
                        "synthesis",
                    ]
                    .contains(&kind)
                        && !suffix.is_empty()
                        && suffix.bytes().all(|byte| {
                            byte.is_ascii_lowercase()
                                || byte.is_ascii_digit()
                                || byte == b'.'
                                || byte == b'-'
                        })
                        && suffix
                            .as_bytes()
                            .first()
                            .is_some_and(u8::is_ascii_alphanumeric)
                        && suffix
                            .as_bytes()
                            .last()
                            .is_some_and(u8::is_ascii_alphanumeric)
                        && !suffix.contains("..")
                        && !suffix.contains("--")
                        && !suffix.contains(".-")
                        && !suffix.contains("-.")
                        && kind == node_type
                })
        });
        if !valid_type
            || !valid_node_id
            || has(record, "record_id")
            || safe_integer(field(record, "record_version")).is_none_or(|version| version < 1)
        {
            return Err("invalid_human_form_selection");
        }
        id = record.object_get("node_id");
    }
    let version = nullable_pick(
        claim.and_then(|value| value.object_get("claim_version")),
        record.object_get("record_version"),
    );
    let Some(id) = id else {
        return Ok(None);
    };
    let digest = match attributes.object_get("source_sha256") {
        Some(JsonValue::String(value)) if value.as_str().is_some() => {
            json_string(&format!("sha256:{}", value.as_str().unwrap_or_default()))
        }
        _ => JsonValue::Null,
    };
    let subject = JsonValue::Object(vec![
        (JsonString::from_utf8("id"), id.clone()),
        (
            JsonString::from_utf8("version"),
            version.cloned().unwrap_or(JsonValue::Null),
        ),
        (JsonString::from_utf8("digest"), digest),
    ]);
    if schema == Some("tos_canonical_node_v1") && !exact_ref(&subject) {
        return Err("invalid_human_form_selection");
    }
    Ok(exact_ref(&subject).then_some(subject))
}

fn js_trim_has_content(value: &JsonString) -> bool {
    value.units().iter().any(|unit| {
        !matches!(
            *unit,
            0x0009..=0x000d
                | 0x0020
                | 0x00a0
                | 0x1680
                | 0x2000..=0x200a
                | 0x2028..=0x2029
                | 0x202f
                | 0x205f
                | 0x3000
                | 0xfeff
        )
    })
}

fn binding(value: &JsonValue) -> bool {
    object(value).is_some()
        && exact_ref(field(value, "record"))
        && generic_pointer(field(value, "pointer"))
}

fn string_array(value: &JsonValue) -> bool {
    array(value).is_some_and(|items| {
        items
            .iter()
            .all(|item| matches!(item, JsonValue::String(_)))
    })
}

fn validate_packet(
    packet: &JsonValue,
    role: &str,
    form: &JsonValue,
    source: &JsonValue,
) -> RuleResult<()> {
    let context = field(packet, "context");
    let context_items = array(context);
    let standalone = field(packet, "standalone_reading").as_bool();
    let display_text = match field(packet, "display_text") {
        JsonValue::String(value) => Some(value),
        _ => None,
    };
    let dependencies = array(field(packet, "dependencies"));
    let issues = array(field(packet, "issues"));
    let admission = field(packet, "admission");
    let script = field(packet, "script");
    let derivation = field(packet, "derivation");
    let language_value = field(packet, "language");
    if object(packet).is_none()
        || !str_eq(
            field(packet, "schema_version"),
            "tos_human_form_materialization_v1",
        )
        || !str_eq(field(packet, "state"), "ready")
        || !str_eq(field(packet, "role"), role)
        || !same_ref(field(packet, "form"), form)
        || !exact_ref(field(packet, "subject"))
        || (js_truthy(source.object_get("entity_id"))
            && !same_primitive(
                field(packet, "subject")
                    .object_get("id")
                    .unwrap_or(&JsonValue::Null),
                field(source, "entity_id"),
            ))
        || !display_text.is_some_and(js_trim_has_content)
        || !has(packet, "language")
        || !language(language_value)
        || !has(packet, "script")
        || !(script.is_null()
            || script
                .as_str()
                .is_some_and(|value| value.len() == 4 && ascii_alpha(value)))
        || !["source-copy", "template", "freeform"]
            .iter()
            .any(|candidate| str_eq(derivation, candidate))
        || dependencies.is_none_or(|items| !items.iter().all(exact_ref))
        || issues.is_none_or(|items| {
            !items.is_empty()
                || !items
                    .iter()
                    .all(|item| matches!(item, JsonValue::String(_)))
        })
        || !has(packet, "admission")
        || !(admission.is_null() || object(admission).is_some())
        || field(packet, "performs_semantic_assessment").as_bool() != Some(false)
        || standalone.is_none()
        || context_items.is_none_or(|items| {
            items.len() > 256 || (!items.is_empty() && standalone != Some(false))
        })
    {
        return Err("invalid_human_form_selection");
    }
    for entry in context_items.unwrap_or_default() {
        if object(entry).is_none()
            || !matches!(field(entry, "slot"), JsonValue::String(_))
            || !binding(field(entry, "binding"))
            || !has(entry, "value")
        {
            return Err("invalid_human_form_selection");
        }
    }
    if has(packet, "language_context") {
        let context = field(packet, "language_context");
        let value = field(context, "value");
        let relation = field(value, "relation");
        let relation_text = relation.as_str().unwrap_or("");
        let transitive = ["translation", "transliteration", "adaptation"].contains(&relation_text);
        let source_valid = if transitive {
            binding(field(value, "source"))
        } else {
            has(value, "source") && field(value, "source").is_null()
        };
        if object(context).is_none()
            || !binding(field(context, "binding"))
            || object(value).is_none()
            || !has(value, "language")
            || !has(value, "script")
            || !language(field(value, "language"))
            || field(value, "language") != language_value
            || field(value, "script") != script
            || ![
                "unknown",
                "original",
                "translation",
                "transliteration",
                "adaptation",
            ]
            .contains(&relation_text)
            || !source_valid
        {
            return Err("invalid_human_form_selection");
        }
    }
    if has(packet, "assessment_snapshot") {
        let state = field(packet, "assessment_snapshot");
        let journal_batches = safe_integer(field(state, "journal_batches"));
        let journal_revision = field(state, "journal_revision");
        let journal_valid = match journal_batches {
            Some(0) => journal_revision.is_null(),
            Some(value) if value > 0 => valid_hash(journal_revision),
            _ => false,
        };
        if object(state).is_none()
            || !field(state, "owner_snapshot")
                .as_str()
                .is_some_and(|value| {
                    value.len() == 71
                        && value.starts_with("sha256:")
                        && value[7..]
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
            || journal_batches.is_none_or(|value| value < 0)
            || !journal_valid
            || field(state, "publication_authorized").as_bool() != Some(false)
            || field(state, "current_runtime_grant").as_bool() != Some(false)
        {
            return Err("invalid_human_form_selection");
        }
    }
    let subject = source_subject(source)?;
    if subject.is_some_and(|subject| !same_ref(field(packet, "subject"), &subject)) {
        return Err("invalid_human_form_selection");
    }
    Ok(())
}

fn candidate_role(candidate: &JsonValue, role: &str) -> bool {
    str_eq(field(candidate, "role"), role)
}

fn validate_selection(
    selection: &JsonValue,
    source: &JsonValue,
    requested: Option<&str>,
) -> RuleResult<()> {
    let selection_state = field(selection, "state");
    let selection_language = field(selection, "requested_language");
    let source_revision = field(source, "content_revision");
    let roles = field(selection, "roles");
    let role_entries = object(roles);
    let candidates = array(field(selection, "candidates"));
    if object(selection).is_none()
        || !str_eq(
            field(selection, "schema_version"),
            "tos_human_form_selection_v1",
        )
        || !valid_hash(field(selection, "content_revision"))
        || !same_primitive(field(selection, "content_revision"), source_revision)
        || !selection_language.as_str().is_some_and(content_language)
        || requested.is_some_and(|value| selection_language.as_str() != Some(value))
        || !["available", "invalid", "over-budget"]
            .iter()
            .any(|candidate| str_eq(selection_state, candidate))
        || !has(selection, "source_ref")
        || !(field(selection, "source_ref").is_null()
            || matches!(field(selection, "source_ref"), JsonValue::String(value) if value.units().len() <= 2048))
        || field(selection, "performs_translation").as_bool() != Some(false)
        || field(selection, "performs_assessment").as_bool() != Some(false)
        || !string_array(field(selection, "issues"))
        || role_entries.is_none_or(|entries| entries.len() != ROLES.len())
        || !ROLES.iter().all(|role| has(roles, role))
        || candidates.is_none_or(|items| items.len() > 32)
    {
        return Err("invalid_human_form_selection");
    }

    let mut candidate_ids: Vec<&[u16]> = Vec::new();
    for candidate in candidates.unwrap_or_default() {
        let form = field(candidate, "form");
        let id = match form.object_get("id") {
            Some(JsonValue::String(value)) => value,
            _ => return Err("invalid_human_form_selection"),
        };
        let role = field(candidate, "role");
        let role_valid = role.is_null() || ROLES.iter().any(|value| role.as_str() == Some(*value));
        if object(candidate).is_none()
            || !exact_ref(form)
            || candidate_ids.contains(&id.units())
            || !role_valid
            || !language(field(candidate, "language"))
            || !CANDIDATE_STATES
                .iter()
                .any(|candidate_state| str_eq(field(candidate, "state"), candidate_state))
            || source_pointer_digits(field(candidate, "source_pointer")).is_none()
        {
            return Err("invalid_human_form_selection");
        }
        candidate_ids.push(id.units());
    }

    for role in ROLES {
        let selected = field(roles, role);
        let state = field(selected, "state");
        let reason = field(selected, "reason");
        let form = field(selected, "form");
        let packet = field(selected, "packet");
        if object(selected).is_none()
            || !STATES.iter().any(|candidate| str_eq(state, candidate))
            || !REASONS.iter().any(|candidate| str_eq(reason, candidate))
            || !(form.is_null() || exact_ref(form))
            || !has(selected, "packet")
        {
            return Err("invalid_human_form_selection");
        }
        if str_eq(state, "ready") {
            if !str_eq(selection_state, "available") {
                return Err("invalid_human_form_selection");
            }
            validate_packet(packet, role, form, source)?;
            let packet_language = field(packet, "language");
            let matched = candidates.unwrap_or_default().iter().any(|candidate| {
                candidate_role(candidate, role)
                    && str_eq(field(candidate, "state"), "ready")
                    && same_ref(field(candidate, "form"), form)
                    && field(candidate, "language") == packet_language
            });
            if !matched {
                return Err("invalid_human_form_selection");
            }
            let selected_language = selection_language.as_str().unwrap_or_default();
            let packet_language = packet_language.as_str();
            if str_eq(reason, "exact-language")
                && packet_language.map(str::to_ascii_lowercase)
                    != Some(selected_language.to_ascii_lowercase())
            {
                return Err("invalid_human_form_selection");
            }
            if str_eq(reason, "less-specific-language")
                && !packet_language.is_some_and(|value| {
                    selected_language
                        .to_ascii_lowercase()
                        .starts_with(&(value.to_ascii_lowercase() + "-"))
                })
            {
                return Err("invalid_human_form_selection");
            }
            if str_eq(reason, "original")
                && (selected_language != "original"
                    && !str_eq(
                        field(
                            field(field(packet, "language_context"), "value"),
                            "relation",
                        ),
                        "original",
                    ))
            {
                return Err("invalid_human_form_selection");
            }
            if str_eq(reason, "automatic") && selected_language != "auto" {
                return Err("invalid_human_form_selection");
            }
        } else if !packet.is_null()
            || (str_eq(state, "over-budget") && !exact_ref(form))
            || (!str_eq(state, "over-budget") && !form.is_null())
        {
            return Err("invalid_human_form_selection");
        }
    }
    Ok(())
}

fn inspection_candidate<'a>(
    selection: &'a JsonValue,
    role: &str,
) -> Option<(&'a JsonValue, usize)> {
    let selected = field(field(selection, "roles"), role);
    if !str_eq(field(selected, "state"), "over-budget")
        || !str_eq(field(selected, "reason"), "inspect-exact-form")
        || !exact_ref(field(selected, "form"))
    {
        return None;
    }
    array(field(selection, "candidates"))?
        .iter()
        .find_map(|candidate| {
            (candidate_role(candidate, role)
                && str_eq(field(candidate, "state"), "ready")
                && same_ref(field(candidate, "form"), field(selected, "form")))
            .then(|| source_pointer_index(field(candidate, "source_pointer")))
            .flatten()
            .map(|index| (candidate, index))
        })
}

fn identity_value(selection: &JsonValue) -> JsonValue {
    let mut parts = vec![
        field(selection, "requested_language").clone(),
        field(selection, "state").clone(),
    ];
    for role in ROLES {
        let selected = field(field(selection, "roles"), role);
        let form = field(selected, "form");
        let ref_value = if exact_ref(form) {
            let version = safe_integer(field(form, "version")).unwrap_or_default();
            JsonValue::Object(vec![
                (JsonString::from_utf8("id"), field(form, "id").clone()),
                (JsonString::from_utf8("version"), json_number(version)),
                (
                    JsonString::from_utf8("digest"),
                    field(form, "digest").clone(),
                ),
            ])
        } else {
            JsonValue::Null
        };
        let packet = field(selected, "packet");
        let language = if packet.is_null() {
            JsonValue::Null
        } else {
            packet
                .object_get("language")
                .cloned()
                .unwrap_or(JsonValue::Null)
        };
        parts.push(JsonValue::Array(vec![
            json_string(role),
            field(selected, "state").clone(),
            field(selected, "reason").clone(),
            ref_value,
            language,
        ]));
    }
    JsonValue::Array(parts)
}

pub(crate) fn content_language_v1(value: &str) -> bool {
    content_language(value)
}

pub fn exact_ref_json_v1(raw: &[u8]) -> bool {
    parse(raw, raw.len().max(1), JsonMode::PublishedStrict).is_ok_and(|value| exact_ref(&value))
}

pub fn same_ref_json_v1(left: &[u8], right: &[u8]) -> bool {
    let left = parse(left, left.len().max(1), JsonMode::PublishedStrict);
    let right = parse(right, right.len().max(1), JsonMode::PublishedStrict);
    matches!((left, right), (Ok(left), Ok(right)) if same_ref(&left, &right))
}

pub(crate) fn valid_identity_v1(identity: &str, requested: Option<&str>) -> bool {
    if identity.encode_utf16().count() > IDENTITY_BYTES {
        return false;
    }
    let Ok(parts) = parse(
        identity.as_bytes(),
        identity.len().max(1),
        JsonMode::RequestLastWins,
    ) else {
        return false;
    };
    let Some(parts) = array(&parts) else {
        return false;
    };
    if parts.len() != 9
        || !parts[0].as_str().is_some_and(content_language)
        || requested.is_some_and(|value| parts[0].as_str() != Some(value))
        || !["available", "invalid", "over-budget"]
            .iter()
            .any(|candidate| str_eq(&parts[1], candidate))
    {
        return false;
    }
    ROLES.iter().enumerate().all(|(index, role)| {
        let Some(row) = array(&parts[index + 2]) else {
            return false;
        };
        row.len() == 5
            && str_eq(&row[0], role)
            && STATES.iter().any(|candidate| str_eq(&row[1], candidate))
            && REASONS.iter().any(|candidate| str_eq(&row[2], candidate))
            && (row[3].is_null() || exact_ref(&row[3]))
            && language(&row[4])
    })
}

pub struct HumanFormRuleSession {
    selection: JsonValue,
    source: JsonValue,
    forms_count: usize,
}

impl HumanFormRuleSession {
    pub fn new(
        selection_json: &[u8],
        source_json: &[u8],
        requested: Option<&str>,
        forms_count: usize,
    ) -> RuleResult<Self> {
        let selection = parse(selection_json, SELECTION_BYTES, JsonMode::PublishedStrict)?;
        let source = parse(source_json, 16 * 1024 * 1024, JsonMode::PublishedStrict)?;
        validate_selection(&selection, &source, requested)?;
        Ok(Self {
            selection,
            source,
            forms_count,
        })
    }

    pub fn identity(&self) -> RuleResult<String> {
        let bytes = emit_value_preserved_json(
            &identity_value(&self.selection),
            JsonLimits {
                max_bytes: 4 * 1024 * 1024,
                max_depth: 8,
                max_visits: 128,
                ..Default::default()
            },
        )
        .map_err(|_| "invalid_human_form_selection")?;
        String::from_utf8(bytes).map_err(|_| "invalid_human_form_selection")
    }

    pub fn inspection_index(&self, role: &str) -> Option<u32> {
        if !ROLES.contains(&role) {
            return None;
        }
        let (_, index) = inspection_candidate(&self.selection, role)?;
        (index < self.forms_count)
            .then(|| u32::try_from(index).ok())
            .flatten()
    }

    pub fn inspection_source_pointer(&self, role: &str) -> Option<String> {
        let (candidate, index) = inspection_candidate(&self.selection, role)?;
        if index >= self.forms_count {
            return None;
        }
        field(candidate, "source_pointer")
            .as_str()
            .map(str::to_owned)
    }

    pub fn validate_inspected_packet(
        &self,
        role: &str,
        index: u32,
        packet_json: &[u8],
    ) -> RuleResult<()> {
        let Some((candidate, expected_index)) = inspection_candidate(&self.selection, role) else {
            return Err("invalid_human_form_inspection");
        };
        if expected_index != index as usize || expected_index >= self.forms_count {
            return Err("invalid_human_form_inspection");
        }
        let packet = parse(packet_json, PACKET_BYTES, JsonMode::PublishedStrict)?;
        let selected = field(field(&self.selection, "roles"), role);
        validate_packet(&packet, role, field(selected, "form"), &self.source)?;
        let Some(subject) = source_subject(&self.source)? else {
            return Err("invalid_human_form_inspection");
        };
        if !same_ref(field(&packet, "form"), field(selected, "form"))
            || !same_ref(field(&packet, "subject"), &subject)
            || source_pointer_index(field(candidate, "source_pointer")) != Some(index as usize)
        {
            return Err("invalid_human_form_inspection");
        }
        Ok(())
    }
}
