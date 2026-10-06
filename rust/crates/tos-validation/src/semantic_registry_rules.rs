//! The maintained semantic-registry change gate's mechanical law.
//! Callers supply schema-validated exact snapshots. No source assessment,
//! registry admission, reader execution, or filesystem authority is supplied.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering as AtomicOrdering};

use num_bigint::BigInt;
use serde_json::{Value, json};
use tos_foundation::{CanonicalProfile, JsonLimits, canonical_bytes_v1};

use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::validation_codec::{bounded_ordered, decoded_state, ordered_emit_state, ordered_state};

/// Strict duplicate-key and finite JSON decoding for the four named gate
/// members. Canonical emission reproduces Python's decoded binary64 values;
/// raw-byte hashes remain the caller's separate source observations.
pub fn decode_snapshot_member(
    raw: &[u8],
    limits: ItemLimits,
    cancelled: &AtomicI32,
) -> Result<(Value, usize), ItemRefusal> {
    if raw.len() > limits.max_member_bytes {
        return Err(ItemRefusal::Budget);
    }
    registry_check(limits.deadline, cancelled)?;
    let codec = JsonLimits::default();
    let ordered = bounded_ordered(
        raw,
        codec,
        limits.max_state_bytes,
        limits.deadline,
        &AtomicBool::new(false),
    )?;
    let ordered_bytes = ordered_state(&ordered)?;
    let emit_state = ordered_emit_state(&ordered)?;
    if ordered_bytes
        .checked_add(emit_state)
        .and_then(|n| n.checked_add(codec.max_bytes))
        .is_none_or(|n| n > limits.max_state_bytes)
    {
        return Err(ItemRefusal::Budget);
    }
    let canonical = canonical_bytes_v1(&ordered, CanonicalProfile::SourceCommandInputV1, codec)
        .map_err(|error| {
            ItemRefusal::Unsupported(format!("registry JSON representation: {error:?}"))
        })?;
    drop(ordered);
    registry_check(limits.deadline, cancelled)?;
    let value: Value = serde_json::from_slice(&canonical)
        .map_err(|_| ItemRefusal::Unsupported("registry JSON scalar representation".into()))?;
    let state = decoded_state(&value)?;
    if state
        .checked_add(canonical.capacity())
        .is_none_or(|n| n > limits.max_state_bytes)
    {
        return Err(ItemRefusal::Budget);
    }
    if !value.is_object() {
        return Err(ItemRefusal::Source("JSON root must be an object".into()));
    }
    registry_check(limits.deadline, cancelled)?;
    Ok((value, state))
}

struct Rules<'a> {
    limits: ItemLimits,
    cancelled: &'a AtomicI32,
    visits: usize,
    issues: BTreeSet<String>,
    issue_bytes: usize,
    state_base: usize,
}
impl Rules<'_> {
    fn visit(&mut self) -> Result<(), ItemRefusal> {
        registry_check(self.limits.deadline, self.cancelled)?;
        self.visits = self.visits.checked_add(1).ok_or(ItemRefusal::Budget)?;
        if self.visits > 4_000_000 {
            return Err(ItemRefusal::Budget);
        }
        Ok(())
    }
    fn issue(&mut self, text: impl Into<String>) -> Result<(), ItemRefusal> {
        self.visit()?;
        let text = text.into();
        if self.issues.contains(&text) {
            return Ok(());
        }
        self.issue_bytes = self
            .issue_bytes
            .checked_add(text.len() + std::mem::size_of::<String>())
            .ok_or(ItemRefusal::Budget)?;
        if self.issues.len() >= self.limits.max_issues
            || self.issue_bytes > 1_048_576
            || self
                .state_base
                .checked_add(self.issue_bytes)
                .is_none_or(|n| n > self.limits.max_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        self.issues.insert(text);
        Ok(())
    }
    fn equal(
        &mut self,
        a: &Value,
        b: &Value,
        strict_number_kind: bool,
    ) -> Result<bool, ItemRefusal> {
        self.visit()?;
        Ok(match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                let a = a.to_string();
                let b = b.to_string();
                if strict_number_kind {
                    // Vocabulary compares canonical Python JSON bytes, which
                    // retain int/float kind and the sign of floating zero.
                    a == b
                } else {
                    number_cmp(&a, &b)? == Ordering::Equal
                }
            }
            (Value::Bool(a), Value::Number(b)) | (Value::Number(b), Value::Bool(a))
                if !strict_number_kind =>
            {
                number_cmp(if *a { "1" } else { "0" }, &b.to_string())? == Ordering::Equal
            }
            (Value::Array(a), Value::Array(b)) => {
                if a.len() != b.len() {
                    false
                } else {
                    let mut same = true;
                    for (a, b) in a.iter().zip(b) {
                        if !self.equal(a, b, strict_number_kind)? {
                            same = false;
                            break;
                        }
                    }
                    same
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                if a.len() != b.len() {
                    false
                } else {
                    let mut same = true;
                    for (key, a) in a {
                        if let Some(b) = b.get(key) {
                            if !self.equal(a, b, strict_number_kind)? {
                                same = false;
                                break;
                            }
                        } else {
                            same = false;
                            break;
                        }
                    }
                    same
                }
            }
            _ => a == b,
        })
    }
}
fn registry_check(deadline: std::time::Instant, cancelled: &AtomicI32) -> Result<(), ItemRefusal> {
    if cancelled.load(AtomicOrdering::Relaxed) != 0 {
        return Err(ItemRefusal::Source(
            "semantic registry gate cancelled".into(),
        ));
    }
    if std::time::Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}
fn integer_lexeme(s: &str) -> bool {
    !s.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'))
}
// Python compares arbitrary integers with the exact value of a binary64 float,
// rather than rounding the integer to f64. All lexemes here passed Foundation.
fn number_ratio(s: &str) -> Result<(BigInt, usize), ItemRefusal> {
    if integer_lexeme(s) {
        return s
            .parse::<BigInt>()
            .map(|n| (n, 0))
            .map_err(|_| ItemRefusal::Unsupported("registry integer".into()));
    }
    let f = s
        .parse::<f64>()
        .map_err(|_| ItemRefusal::Unsupported("registry float".into()))?;
    if !f.is_finite() {
        return Err(ItemRefusal::Unsupported("nonfinite registry float".into()));
    }
    let bits = f.to_bits();
    let exponent = ((bits >> 52) & 2047) as i32;
    let mantissa = (bits & ((1u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1u64 << 52 };
    let mut n = BigInt::from(mantissa);
    if bits >> 63 != 0 {
        n = -n;
    }
    let power = if exponent == 0 {
        -1074
    } else {
        exponent - 1023 - 52
    };
    Ok(if power >= 0 {
        (n << power as usize, 0)
    } else {
        (n, (-power) as usize)
    })
}
fn number_cmp(a: &str, b: &str) -> Result<Ordering, ItemRefusal> {
    let (a, ad) = number_ratio(a)?;
    let (b, bd) = number_ratio(b)?;
    Ok((a << bd).cmp(&(b << ad)))
}
fn numeric(value: &Value) -> String {
    value
        .as_number()
        .map(ToString::to_string)
        .unwrap_or_else(|| "0".into())
}
fn increased(new: &Value, old: &Value, key: &str) -> Result<bool, ItemRefusal> {
    number_cmp(&numeric(&new[key]), &numeric(&old[key])).map(|c| c == Ordering::Greater)
}
fn strict_int(value: &Value) -> bool {
    value
        .as_number()
        .is_some_and(|n| integer_lexeme(&n.to_string()))
}
fn int_range(value: &Value, min: u64, max: u64) -> bool {
    strict_int(value) && value.as_u64().is_some_and(|n| n >= min && n <= max)
}
fn at_least(value: &Value, min: u64) -> bool {
    strict_int(value)
        && number_cmp(&numeric(value), &min.to_string()).is_ok_and(|c| c != Ordering::Less)
}
fn s(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty())
}
fn rows<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v.is_object())
}
fn strings<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a str> {
    value[key].as_array().into_iter().flatten().filter_map(s)
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => number_cmp(&n.to_string(), "0").is_ok_and(|c| c != Ordering::Equal),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
fn exact_keys(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == keys.len() && keys.iter().all(|k| o.contains_key(*k)))
}
fn python_printable(c: char) -> bool {
    use unicode_general_category::{GeneralCategory, get_general_category};
    c == ' '
        || !matches!(
            get_general_category(c),
            GeneralCategory::Control
                | GeneralCategory::Format
                | GeneralCategory::Surrogate
                | GeneralCategory::PrivateUse
                | GeneralCategory::Unassigned
                | GeneralCategory::SpaceSeparator
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
        )
}
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c)
            }
            c if python_printable(c) => out.push(c),
            c if (c as u32) < 256 => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if (c as u32) <= 0xffff => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push_str(&format!("\\U{:08x}", c as u32)),
        }
    }
    out.push(quote);
    out
}
fn hierarchy(
    r: &mut Rules<'_>,
    entries: &BTreeMap<&str, &Value>,
    field: &str,
    label: &str,
    single: bool,
) -> Result<(), ItemRefusal> {
    let mut states = BTreeMap::<&str, u8>::new();
    for &id in entries.keys() {
        r.visit()?;
        if states.get(id) == Some(&2) {
            continue;
        }
        states.insert(id, 1);
        let mut stack = vec![(id, 0usize)];
        while let Some((current, index)) = stack.last().copied() {
            r.visit()?;
            let entry = entries[current];
            let parent = if single {
                if index == 0 { s(&entry[field]) } else { None }
            } else {
                entry[field]
                    .as_array()
                    .and_then(|a| a.get(index))
                    .and_then(s)
            };
            // Schema-validated parents are nonempty strings. The direct pure
            // entry also skips nonstrings, as the maintained _strings helper.
            if !single
                && parent.is_none()
                && entry[field].as_array().is_some_and(|a| index < a.len())
            {
                stack.last_mut().unwrap().1 += 1;
                continue;
            }
            let Some(parent) = parent else {
                states.insert(current, 2);
                stack.pop();
                continue;
            };
            stack.last_mut().unwrap().1 += 1;
            if !entries.contains_key(parent) {
                r.issue(format!(
                    "{label} {current} references missing parent {parent}"
                ))?;
                continue;
            }
            if states.get(parent) == Some(&1) {
                let mut cycle = stack.iter().map(|(id, _)| *id).collect::<Vec<_>>();
                cycle.push(parent);
                r.issue(format!(
                    "{label} hierarchy contains a cycle: {}",
                    cycle.join(" -> ")
                ))?;
            } else if states.get(parent) != Some(&2) {
                states.insert(parent, 1);
                stack.push((parent, 0));
            }
        }
    }
    Ok(())
}
fn language(key: &str) -> bool {
    let mut parts = key.split('-');
    let first = parts.next().unwrap_or("");
    let has_rest = parts.clone().next().is_some();
    ((2..=8).contains(&first.len()) && first.bytes().all(|b| b.is_ascii_alphabetic())
        || matches!(first, "i" | "I" | "x" | "X") && has_rest)
        && parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}
fn languages_valid<'a>(keys: impl Iterator<Item = &'a str>, max_len: Option<usize>) -> bool {
    let mut seen = BTreeSet::new();
    keys.into_iter().all(|key| {
        language(key)
            && max_len.is_none_or(|n| key.len() <= n)
            && !matches!(
                key.to_ascii_lowercase().as_str(),
                "default" | "original" | "auto"
            )
            && seen.insert(key.to_ascii_lowercase())
    })
}
// Python str.strip additionally includes U+001C..U+001F.
fn nonblank(s: &str) -> bool {
    s.chars()
        .any(|c| !c.is_whitespace() && !('\u{1c}'..='\u{1f}').contains(&c))
}
fn labels(value: &Value, languages: &BTreeSet<&str>, max: usize) -> bool {
    value.as_object().is_some_and(|o| {
        o.len() == languages.len()
            && o.iter().all(|(key, text)| {
                languages.contains(key.as_str())
                    && text
                        .as_str()
                        .is_some_and(|text| nonblank(text) && text.chars().count() <= max)
            })
    })
}
fn template_valid(template: &Value) -> bool {
    let fields = [
        "template_id",
        "template_version",
        "reader",
        "purpose",
        "owner_ref",
        "default_language",
        "max_output_bytes",
        "marker",
        "status_labels",
        "renderings",
    ];
    let Some(o) = template.as_object() else {
        return false;
    };
    if !fields.iter().all(|f| o.contains_key(*f))
        || o.keys()
            .any(|key| !fields.contains(&key.as_str()) && key != "object_label_adapters")
    {
        return false;
    }
    let Some(id) =
        s(&template["template_id"]).and_then(|id| id.strip_prefix("tos.navigation-template."))
    else {
        return false;
    };
    if id.split(['.', '-']).any(|p| {
        p.is_empty()
            || !p
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    }) || !at_least(&template["template_version"], 1)
        || template["reader"] != "claim-navigation-v1"
        || template["purpose"] != "claim-navigation-only"
        || template["owner_ref"] != "ToS/doctrine/HUMAN_FORMS.md"
        || !int_range(&template["max_output_bytes"], 128, 16384)
    {
        return false;
    }
    if let Some(adapters) = o.get("object_label_adapters") {
        let Some(a) = adapters.as_array() else {
            return false;
        };
        let mut seen = BTreeSet::new();
        if !(1..=2).contains(&a.len())
            || !at_least(&template["template_version"], 2)
            || a.iter().any(|v| {
                !matches!(
                    v.as_str(),
                    Some(
                        "historical-time-source-wording-v1"
                            | "document-catalogue-time-source-wording-v1"
                    )
                ) || !seen.insert(v.as_str().unwrap())
                    || v == "document-catalogue-time-source-wording-v1"
                        && !at_least(&template["template_version"], 3)
            })
        {
            return false;
        }
    }
    let Some(renderings) = template["renderings"].as_object() else {
        return false;
    };
    if !(1..=16).contains(&renderings.len())
        || !languages_valid(renderings.keys().map(String::as_str), Some(64))
    {
        return false;
    }
    let languages: BTreeSet<_> = renderings.keys().map(String::as_str).collect();
    if !s(&template["default_language"]).is_some_and(|s| languages.contains(s))
        || !labels(&template["marker"], &languages, 256)
        || !exact_keys(
            &template["status_labels"],
            &["epistemic_status", "review_status"],
        )
    {
        return false;
    }
    for (key, statuses) in [
        (
            "epistemic_status",
            &[
                "observed",
                "inferred",
                "reported",
                "interpreted",
                "uncertain",
                "disputed",
            ][..],
        ),
        (
            "review_status",
            &[
                "unreviewed",
                "accepted",
                "accepted_with_limits",
                "rejected",
                "ambiguous",
                "deferred",
                "superseded",
            ][..],
        ),
    ] {
        let status = &template["status_labels"][key];
        if !exact_keys(status, statuses)
            || status
                .as_object()
                .unwrap()
                .values()
                .any(|v| !labels(v, &languages, 256))
        {
            return false;
        }
    }
    let slots = [
        "claim-marker",
        "subject-label",
        "predicate-label",
        "object-label",
        "declared-epistemic-status",
        "declared-review-status",
    ];
    for parts in renderings.values() {
        let Some(parts) = parts.as_array() else {
            return false;
        };
        if !(6..=32).contains(&parts.len())
            || !exact_keys(&parts[0], &["slot"])
            || parts[0]["slot"] != "claim-marker"
        {
            return false;
        }
        let mut seen = BTreeSet::new();
        for part in parts {
            if exact_keys(part, &["slot"]) {
                let Some(slot) = s(&part["slot"]) else {
                    return false;
                };
                if !slots.contains(&slot) || !seen.insert(slot) {
                    return false;
                }
            } else if !exact_keys(part, &["literal"])
                || !part["literal"]
                    .as_str()
                    .is_some_and(|s| (1..=256).contains(&s.chars().count()))
            {
                return false;
            }
        }
        if seen.len() != slots.len() {
            return false;
        }
    }
    true
}
fn schema_selector(s: &str) -> bool {
    let Some(s) = s.strip_prefix("tos_") else {
        return false;
    };
    let Some((name, version)) = s.rsplit_once("_v") else {
        return false;
    };
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && !version.is_empty()
        && version.bytes().all(|b| b.is_ascii_digit())
}
fn vocabulary(
    r: &mut Rules<'_>,
    registry: &Value,
    previous: Option<&Value>,
) -> Result<(), ItemRefusal> {
    let value = &registry["context_presentation"];
    let old = previous
        .map(|v| &v["context_presentation"])
        .unwrap_or(&Value::Null);
    if value.is_null() {
        if !old.is_null() {
            r.issue("context presentation cannot remove its historical owner contract")?;
        }
        return Ok(());
    }
    let expected = [
        "schema_version",
        "presentation_id",
        "presentation_version",
        "owner_ref",
        "purpose",
        "default_language",
        "languages",
        "max_output_bytes",
        "max_entries",
        "record_schema_versions",
        "field_rules",
        "unclassified",
    ];
    if !exact_keys(value, &expected)
        || value["schema_version"] != "tos_context_presentation_v1"
        || value["presentation_id"] != "tos.context-presentation.governing"
        || value["owner_ref"] != "ToS/doctrine/HUMAN_FORMS.md"
        || value["purpose"] != "source-context-reading-not-assessment"
        || !at_least(&value["presentation_version"], 1)
        || !int_range(&value["max_output_bytes"], 1024, 32768)
        || !int_range(&value["max_entries"], 1, 256)
    {
        r.issue("context presentation violates its finite owner contract")?;
        return Ok(());
    }
    let Some(langs) = value["languages"].as_array() else {
        r.issue("context presentation has invalid label languages")?;
        return Ok(());
    };
    let languages: BTreeSet<_> = langs.iter().filter_map(Value::as_str).collect();
    if !(1..=8).contains(&langs.len())
        || languages.len() != langs.len()
        || !languages_valid(languages.iter().copied(), None)
        || !s(&value["default_language"]).is_some_and(|s| languages.contains(s))
    {
        r.issue("context presentation has invalid label languages")?;
        return Ok(());
    }
    let schemas = value["record_schema_versions"].as_array();
    if !schemas.is_some_and(|a| {
        (1..=128).contains(&a.len())
            && a.iter().all(|v| v.as_str().is_some_and(schema_selector))
            && a.iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                == a.len()
    }) {
        r.issue("context presentation has invalid source-schema selectors")?;
        return Ok(());
    }
    let unknown = &value["unclassified"];
    if !exact_keys(unknown, &["label", "explanation"])
        || !labels(&unknown["label"], &languages, 1024)
        || !labels(&unknown["explanation"], &languages, 1024)
    {
        r.issue("context presentation has invalid unclassified explanation")?;
        return Ok(());
    }
    let Some(rules) = value["field_rules"]
        .as_array()
        .filter(|a| (1..=128).contains(&a.len()))
    else {
        r.issue("context presentation field rules are not bounded")?;
        return Ok(());
    };
    let mut seen = BTreeSet::new();
    for rule in rules {
        r.visit()?;
        let field = s(&rule["field"]).unwrap_or("");
        let targets = rule["targets"].as_array();
        if !exact_keys(
            rule,
            &[
                "field",
                "targets",
                "category",
                "label",
                "explanation",
                "value_labels",
            ],
        ) || field.is_empty()
            || field.len() > 96
            || !field.as_bytes()[0].is_ascii_lowercase()
            || !field
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            || !targets.is_some_and(|a| {
                !a.is_empty()
                    && a.iter().all(|v| {
                        matches!(
                            v.as_str(),
                            Some(
                                "record"
                                    | "assertion"
                                    | "language-context"
                                    | "subject-assessment"
                                    | "assessment-snapshot"
                            )
                        )
                    })
                    && a.iter()
                        .filter_map(Value::as_str)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == a.len()
            })
            || !matches!(rule["category"].as_str(), Some("governing" | "technical"))
            || !labels(&rule["label"], &languages, 1024)
            || !rule["explanation"].is_null() && !labels(&rule["explanation"], &languages, 1024)
            || !rule["value_labels"].is_null()
                && !rule["value_labels"].as_object().is_some_and(|o| {
                    !o.is_empty()
                        && o.len() <= 64
                        && o.iter().all(|(key, label)| {
                            !key.is_empty()
                                && key.chars().count() <= 128
                                && labels(label, &languages, 1024)
                        })
                })
        {
            r.issue("context presentation has an invalid field rule")?;
            continue;
        }
        if rule["category"] == "technical"
            && ![
                "schema_version",
                "record_id",
                "record_version",
                "claim_id",
                "claim_version",
                "source_record_digest",
                "source_sha256",
                "record_sha256",
                "journal_revision",
                "owner_snapshot",
                "journal_batches",
            ]
            .contains(&field)
        {
            r.issue("context presentation cannot hide a nonmechanical field")?;
        }
        for target in targets.unwrap() {
            if !seen.insert((target.as_str().unwrap(), field)) {
                r.issue("context presentation has ambiguous field rules")?;
            }
        }
    }
    if old.is_object() {
        if !r.equal(old, value, true)? && !increased(value, old, "presentation_version")? {
            r.issue("changed context presentation must increase presentation_version")?;
        }
        for key in ["schema_version", "presentation_id", "owner_ref", "purpose"] {
            if !r.equal(&old[key], &value[key], false)? {
                r.issue("context presentation cannot repurpose its owner identity")?;
            }
        }
    }
    Ok(())
}
fn allowed_field(field: &str) -> bool {
    if [
        "id",
        "entity_id",
        "native_id",
        "source_dossier_ref",
        "source_graph",
        "kind_id",
        "type_id",
        "type_mapping.status",
        "type_mapping.source_kind_id",
        "display.summary_state",
        "epistemic.authority_layer",
        "epistemic.canon_status",
        "epistemic.review_posture",
        "epistemic.confidence",
        "graph_layers",
        "view_ids",
        "source_refs",
    ]
    .contains(&field)
    {
        return true;
    }
    if let Some(rest) = field.strip_prefix("display.") {
        if let Some((kind, key)) = rest.split_once('.') {
            if matches!(kind, "title" | "kind_label" | "summary")
                && (matches!(key, "default" | "original") || language(key))
            {
                return true;
            }
        }
    }
    let rest = field
        .strip_prefix("attributes.")
        .or_else(|| field.strip_prefix("semantics."));
    rest.is_some_and(|s| {
        (1..=128).contains(&s.len())
            && s.as_bytes()[0].is_ascii_alphanumeric()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
            && !s
                .split('.')
                .any(|p| matches!(p, "__proto__" | "prototype" | "constructor"))
    })
}
fn indexed<'a>(
    r: &mut Rules<'_>,
    registry: &'a Value,
    field: &str,
    id_key: &str,
    label: &str,
) -> Result<BTreeMap<&'a str, &'a Value>, ItemRefusal> {
    let mut entries = BTreeMap::new();
    for row in rows(registry, field) {
        r.visit()?;
        let Some(id) = s(&row[id_key]) else {
            r.issue(format!(
                "{label} registry contains an entry without {id_key}"
            ))?;
            continue;
        };
        if entries.insert(id, row).is_some() {
            r.issue(format!(
                "duplicate {} {id}",
                if label == "entity" {
                    "entity type_id"
                } else {
                    "relation_type_id"
                }
            ))?;
        }
    }
    Ok(entries)
}
fn ordered_ids<'a>(registry: &'a Value, field: &str, id_key: &str) -> Vec<&'a str> {
    let mut seen = BTreeSet::new();
    rows(registry, field)
        .filter_map(|row| s(&row[id_key]))
        .filter(|id| seen.insert(*id))
        .collect()
}
fn predicates(entry: &Value) -> BTreeSet<&str> {
    rows(entry, "source_mappings")
        .filter(|m| m["source_graph"] == "source-claims" && m["scope"] == "claim-predicate")
        .filter_map(|m| s(&m["source_predicate_id"]))
        .collect()
}
fn evolution(
    r: &mut Rules<'_>,
    previous: &Value,
    current: &Value,
    entries: &BTreeMap<&str, &Value>,
    field: &str,
    id_key: &str,
) -> Result<(), ItemRefusal> {
    let profile_key = if field == "types" {
        "source_record_profile"
    } else {
        "source_claim_profile"
    };
    for entry in rows(previous, field) {
        r.visit()?;
        let Some(id) = s(&entry[id_key]) else {
            continue;
        };
        let now = entries.get(id).copied().unwrap_or(&Value::Null);
        if !entries.contains_key(id) {
            r.issue(format!(
                "registry removed historical identity {id}; retain a deprecated entry"
            ))?;
        }
        let old = &entry[profile_key];
        if !truthy(old) {
            continue;
        }
        let profile = &now[profile_key];
        if !profile.is_object() {
            r.issue(format!(
                "registry removed source reader for {id}; retain its historical routes"
            ))?;
            continue;
        }
        if !r.equal(profile, old, false)? && !increased(profile, old, "profile_version")? {
            r.issue(format!(
                "changed source profile {id} must increase profile_version"
            ))?;
        }
        for key in [
            "record_type",
            "id_prefix",
            "reader",
            "value_kind",
            "retained_native_adapter",
        ] {
            if !r.equal(&profile[key], &old[key], false)? {
                r.issue(format!(
                    "source profile {id} repurposes {key}; use an explicit successor identity"
                ))?;
            }
        }
        if field == "relations" {
            if predicates(entry) != predicates(now)
                || !r.equal(&profile["reader"], &old["reader"], false)?
            {
                r.issue(format!("source claim profile {id} repurposes its predicate or reader; use an explicit successor identity"))?;
            }
            let layers: BTreeSet<_> = strings(profile, "assertion_layers").collect();
            if strings(old, "assertion_layers").any(|layer| !layers.contains(layer)) {
                r.issue(format!("source claim profile {id} removes a historical assertion layer; use an explicit successor identity"))?;
            }
        }
        let routes: BTreeMap<_, _> = rows(profile, "schemas")
            .filter_map(|route| s(&route["schema_version"]).map(|key| (key, route)))
            .collect();
        for route in rows(old, "schemas") {
            r.visit()?;
            if let Some(key) = s(&route["schema_version"]) {
                if let Some(new) = routes.get(key) {
                    if r.equal(new, route, false)? {
                        continue;
                    }
                }
            }
            r.issue(format!(
                "source profile {id} removed or repurposed a historical schema route"
            ))?;
        }
    }
    if !r.equal(previous, current, false)? && !increased(current, previous, "registry_version")? {
        r.issue("changed registry must increase registry_version")?;
    }
    Ok(())
}

/// Mechanical law from knowledge.validate_semantic_registries on the
/// exact schema-validated gate snapshots. Indexes borrow source objects;
/// historical duplicate identities keep the maintained last-entry behavior.
pub fn validate_semantic_registries(
    entity: &Value,
    relation: &Value,
    previous: Option<(&Value, &Value)>,
    limits: ItemLimits,
    cancelled: &AtomicI32,
) -> Result<Value, ItemRefusal> {
    registry_check(limits.deadline, cancelled)?;
    // Prices the retained decoded snapshots plus borrowed BTree/DFS slots
    // conservatively by decoded payload. It is logical state, not allocator RSS.
    let mut retained = decoded_state(entity)?
        .checked_add(decoded_state(relation)?)
        .ok_or(ItemRefusal::Budget)?;
    if let Some((e, q)) = previous {
        retained = retained
            .checked_add(decoded_state(e)?)
            .and_then(|n| decoded_state(q).ok().and_then(|q| n.checked_add(q)))
            .ok_or(ItemRefusal::Budget)?;
    }
    let state_base = retained
        .checked_mul(4)
        .filter(|n| *n <= limits.max_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    let mut r = Rules {
        limits,
        cancelled,
        visits: 0,
        issues: BTreeSet::new(),
        issue_bytes: 0,
        state_base,
    };
    vocabulary(&mut r, entity, previous.map(|p| p.0))?;
    let entities = indexed(&mut r, entity, "types", "type_id", "entity")?;
    let relations = indexed(
        &mut r,
        relation,
        "relations",
        "relation_type_id",
        "relation",
    )?;
    if let Some((e, q)) = previous {
        evolution(&mut r, e, entity, &entities, "types", "type_id")?;
        evolution(
            &mut r,
            q,
            relation,
            &relations,
            "relations",
            "relation_type_id",
        )?;
    }
    if let Some(template) = relation.get("claim_navigation_template") {
        if !template_valid(template) {
            r.issue("claim navigation template violates its finite source contract")?;
        }
    }
    if let Some(old) = previous.and_then(|p| p.1.get("claim_navigation_template")) {
        let template = &relation["claim_navigation_template"];
        if !template.is_object() {
            r.issue("registry removed historical claim navigation template")?;
        } else if old.is_object() {
            if !r.equal(template, old, false)?
                && (!strict_int(&template["template_version"])
                    || !strict_int(&old["template_version"])
                    || !increased(template, old, "template_version")?)
            {
                r.issue("changed claim navigation template must increase template_version")?;
            }
            for key in ["template_id", "reader", "purpose", "owner_ref"] {
                if !r.equal(&template[key], &old[key], false)? {
                    r.issue(format!(
                        "claim navigation template repurposes {key}; explicit migration required"
                    ))?;
                }
            }
        }
    }
    let mut properties = BTreeSet::new();
    for definition in rows(entity, "property_definitions") {
        r.visit()?;
        let id = s(&definition["property_id"]);
        let label = id.unwrap_or("None");
        if id.is_none() || !properties.insert(id) {
            r.issue(format!("duplicate or missing property_id {label}"))?;
        }
        if !allowed_field(s(&definition["field"]).unwrap_or("")) {
            r.issue(format!("property {label} has an unsupported query field"))?;
        }
        for owner in strings(definition, "applies_to") {
            if !entities.contains_key(owner) {
                r.issue(format!(
                    "property {label} refers to unregistered type {owner}"
                ))?;
            }
        }
    }
    for (entries, key, label, registry) in [
        (&entities, "fallback_type_id", "entity", entity),
        (
            &relations,
            "fallback_relation_type_id",
            "relation",
            relation,
        ),
    ] {
        let fallback = s(&registry[key]);
        if !fallback.is_some_and(|id| entries.contains_key(id)) {
            r.issue(format!(
                "{label} fallback {} is not registered",
                fallback.map(py_repr).unwrap_or_else(|| "None".into())
            ))?;
        }
    }
    hierarchy(&mut r, &entities, "parent_type_ids", "entity", false)?;
    hierarchy(
        &mut r,
        &relations,
        "parent_relation_type_ids",
        "relation",
        false,
    )?;
    for (entries, field, label) in [
        (&entities, "supersedes_type_id", "entity supersession"),
        (
            &relations,
            "supersedes_relation_type_id",
            "relation supersession",
        ),
    ] {
        hierarchy(&mut r, entries, field, label, true)?;
        for (id, entry) in entries {
            r.visit()?;
            if truthy(&entry["abstract"]) && truthy(&entry["source_mappings"]) {
                r.issue(format!("abstract {label} {id} cannot map source instances"))?;
            }
        }
    }
    let mut entity_mappings = BTreeMap::new();
    for id in ordered_ids(entity, "types", "type_id") {
        let entry = entities[id];
        for mapping in rows(entry, "source_mappings") {
            r.visit()?;
            let key = (
                s(&mapping["source_graph"]).unwrap_or(""),
                s(&mapping["source_kind_id"]).unwrap_or(""),
            );
            if key.0.is_empty() || key.1.is_empty() {
                r.issue(format!("entity mapping on {id} is incomplete"))?;
                continue;
            }
            if let Some(prior) = entity_mappings.insert(key, id) {
                r.issue(format!(
                    "duplicate entity source mapping ({}, {}) on {prior} and {id}",
                    py_repr(key.0),
                    py_repr(key.1)
                ))?;
            }
        }
    }
    let mut relation_mappings = BTreeMap::new();
    for id in ordered_ids(relation, "relations", "relation_type_id") {
        let entry = relations[id];
        r.visit()?;
        for endpoint in strings(entry, "domain_type_ids").chain(strings(entry, "range_type_ids")) {
            if !entities.contains_key(endpoint) {
                r.issue(format!(
                    "relation {id} references missing entity type {endpoint}"
                ))?;
            }
        }
        for parent in strings(entry, "parent_relation_type_ids") {
            if !relations.contains_key(parent) {
                r.issue(format!("relation {id} references missing parent {parent}"))?;
            }
        }
        if let Some(inverse) = s(&entry["inverse_relation_type_id"]) {
            if let Some(other) = relations.get(inverse) {
                if s(&other["inverse_relation_type_id"]) != Some(id) {
                    r.issue(format!(
                        "relation inverse {id} -> {inverse} is not reciprocal"
                    ))?;
                }
            } else {
                r.issue(format!(
                    "relation {id} references missing inverse {inverse}"
                ))?;
            }
        }
        for (min, max) in [
            ("per_subject_min", "per_subject_max"),
            ("per_object_min", "per_object_max"),
        ] {
            let c = &entry["cardinality"];
            if strict_int(&c[min])
                && strict_int(&c[max])
                && number_cmp(&numeric(&c[min]), &numeric(&c[max]))? == Ordering::Greater
            {
                r.issue(format!("relation {id} has {min} greater than {max}"))?;
            }
        }
        for mapping in rows(entry, "source_mappings") {
            r.visit()?;
            let key = (
                s(&mapping["source_graph"]).unwrap_or(""),
                s(&mapping["source_predicate_id"]).unwrap_or(""),
                s(&mapping["scope"]).unwrap_or(""),
            );
            if key.0.is_empty() || key.1.is_empty() || key.2.is_empty() {
                r.issue(format!("relation mapping on {id} is incomplete"))?;
                continue;
            }
            if let Some(prior) = relation_mappings.insert(key, id) {
                r.issue(format!(
                    "duplicate relation source mapping ({}, {}, {}) on {prior} and {id}",
                    py_repr(key.0),
                    py_repr(key.1),
                    py_repr(key.2)
                ))?;
            }
        }
    }
    registry_check(limits.deadline, cancelled)?;
    Ok(
        json!({"valid":r.issues.is_empty(),"violations":r.issues.into_iter().collect::<Vec<_>>(),"entity_type_count":entities.len(),"relation_type_count":relations.len(),"entity_mapping_count":entity_mappings.len(),"relation_mapping_count":relation_mappings.len()}),
    )
}
