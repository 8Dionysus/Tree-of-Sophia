//! Source-declared temporal envelopes only: no calendar conversion, inferred
//! Claims, prose parsing, relative-anchor inference or historical assessment.
use crate::{
    search_v2::{SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, text},
};
use std::collections::BTreeSet;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonNumberKind, JsonValue, canonical_bytes_v1,
    python_strip_unicode16_v1,
};
pub const TEMPORAL_OPERATION: &str = "tos.knowledge.temporal.compare";
pub const TEMPORAL_INTENDED_USE: &str = "read_only_public_knowledge_temporal_compare_v1";
fn err(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn corrupt() -> SearchV2Error {
    err(
        SearchV2ErrorCode::CorruptSelectedCarrier,
        "temporal operand carrier invalid",
    )
}
fn field<'a>(value: &'a JsonValue, key: &str) -> &'a JsonValue {
    value.object_get(key).unwrap_or(&JsonValue::Null)
}
fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    field(value, key).as_str()
}
fn path<'a>(value: &'a JsonValue, keys: &[&str]) -> &'a JsonValue {
    keys.iter().fold(value, |v, k| field(v, k))
}
fn bare(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn safe_integer(value: &JsonValue) -> Option<i64> {
    let JsonValue::Number(number) = value else {
        return None;
    };
    if number.kind == JsonNumberKind::Int {
        let n = number.lexeme.parse::<i64>().ok()?;
        return (n.unsigned_abs() <= 9_007_199_254_740_991).then_some(n);
    }
    let n = number.lexeme.parse::<f64>().ok()?;
    (n.is_finite() && n.abs() <= 9_007_199_254_740_991.0 && n.fract() == 0.0).then_some(n as i64)
}
fn same_json(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Number(a), JsonValue::Number(b)) => match (a.kind, b.kind) {
            (JsonNumberKind::Int, JsonNumberKind::Int) => {
                a.lexeme.trim_start_matches('-').trim_start_matches('0')
                    == b.lexeme.trim_start_matches('-').trim_start_matches('0')
                    && (a.lexeme.starts_with('-') == b.lexeme.starts_with('-')
                        || a.lexeme.trim_start_matches('-').bytes().all(|c| c == b'0'))
            }
            (JsonNumberKind::Float, JsonNumberKind::Float) => {
                a.lexeme.parse::<f64>().ok() == b.lexeme.parse::<f64>().ok()
            }
            _ => {
                let (integer, float) = if a.kind == JsonNumberKind::Int {
                    (a, b)
                } else {
                    (b, a)
                };
                let Ok(n) = float.lexeme.parse::<f64>() else {
                    return false;
                };
                if !n.is_finite() || n.fract() != 0.0 {
                    return false;
                }
                let decimal = format!("{n:.0}");
                let id = integer
                    .lexeme
                    .trim_start_matches('-')
                    .trim_start_matches('0');
                let fd = decimal.trim_start_matches('-').trim_start_matches('0');
                id == fd
                    && (id.is_empty()
                        || integer.lexeme.starts_with('-') == decimal.starts_with('-'))
            }
        },
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.iter()
                        .find(|(k, _)| k == key)
                        .is_some_and(|(_, v)| same_json(value, v))
                })
        }
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_json(a, b))
        }
        _ => left == right,
    }
}
fn containers(value: &JsonValue) -> Result<(), SearchV2Error> {
    if ["attributes", "semantics", "type_mapping"]
        .iter()
        .any(|k| field(value, k).as_object().is_none())
    {
        return Err(corrupt());
    }
    for key in ["claim", "time"] {
        if let Some(value) = field(value, "semantics").object_get(key) {
            if value.as_object().is_none() {
                return Err(corrupt());
            }
        }
    }
    Ok(())
}
fn unique_issues(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
fn canonical(value: &JsonValue, limits: JsonLimits) -> Result<Vec<u8>, SearchV2Error> {
    canonical_bytes_v1(value, CanonicalProfile::SourceRecordDigestV1, limits).map_err(|_| corrupt())
}
fn sha(value: &JsonValue, limits: JsonLimits) -> Result<String, SearchV2Error> {
    Ok(Digest256::of_bytes(&canonical(value, limits)?).to_hex())
}
struct Operand {
    packet: JsonValue,
    status: &'static str,
    issues: Vec<String>,
}
fn operand_result(
    claim: JsonValue,
    value: JsonValue,
    time: JsonValue,
    status: &'static str,
    issues: Vec<String>,
) -> Operand {
    Operand {
        packet: object(vec![
            ("claim", claim),
            ("value", value),
            ("normalized_time", time),
        ]),
        status,
        issues,
    }
}
fn documentary<L>(
    claim: &JsonValue,
    value: &JsonValue,
    source: &JsonValue,
    semantics: &JsonValue,
    time: &JsonValue,
    source_graph: &str,
    lookup: &mut L,
    limits: JsonLimits,
) -> Result<Option<&'static str>, SearchV2Error>
where
    L: FnMut(&str) -> Result<Vec<JsonValue>, SearchV2Error>,
{
    let profile = field(semantics, "source_claim_profile");
    let raw = field(source, "object");
    let attribution = path(source, &["qualifiers", "catalogue_attribution"]);
    let schemas = field(profile, "schemas").as_array();
    if get(source, "predicate") != Some("document_catalogue_date")
        || get(source, "schema_version") != Some("tos_document_catalogue_claim_v1")
        || get(source, "assertion_layer") != Some("bibliographic_assertion")
        || get(semantics, "relation_type_id") != Some("tos.relation.document-catalogue-date")
        || get(profile, "reader") != Some("document-catalogue-temporal-v1")
        || field(profile, "assertion_layers")
            != &JsonValue::Array(vec![text("bibliographic_assertion")])
        || !schemas.is_some_and(|values| {
            values.len() == 1
                && get(&values[0], "schema_version") == get(source, "schema_version")
                && get(&values[0], "schema_ref")
                    == Some("ToS/contracts/document-catalogue-claim.schema.json")
        })
        || get(raw, "role") != Some("catalogue-assigned-document-date")
        || field(time, "role") != field(raw, "role")
        || !matches!(
            get(raw, "kind"),
            Some("date-assertion" | "interval-assertion" | "unknown-date")
        )
        || get(attribution, "field_role") != Some("assigned-date")
        || !get(attribution, "source_field").is_some_and(|value| !value.trim().is_empty())
        || !field(source, "evidence_refs")
            .as_array()
            .is_some_and(|refs| refs.contains(field(attribution, "evidence_ref")))
        || !same_json(
            field(attribution, "source_wording"),
            field(raw, "source_wording"),
        )
    {
        return Ok(Some("document-catalogue-profile-binding-inconsistent"));
    }
    let subjects = match get(semantics, "subject_node_id") {
        Some(id) => lookup(id)?,
        None => vec![],
    };
    if subjects.len() != 1
        || field(&subjects[0], "entity_id") != field(source, "subject_ref")
        || get(&subjects[0], "source_graph") != Some(source_graph)
        || get(field(&subjects[0], "type_mapping"), "status") != Some("mapped")
        || !path(&subjects[0], &["semantics", "type_ancestors"])
            .as_array()
            .is_some_and(|values| values.contains(&text("tos.entity.document")))
    {
        return Ok(Some("document-catalogue-subject-binding-inconsistent"));
    }
    let bytes = canonical(source, limits)?;
    let digest = Digest256::of_bytes(&bytes).to_hex();
    let value_digest = sha(raw, limits)?;
    let left = field(claim, "attributes");
    let right = field(value, "attributes");
    let literal = object(vec![
        ("claim_ref", field(source, "claim_id").clone()),
        ("value", raw.clone()),
    ]);
    let expected_native = format!("literal:sha256:{}", sha(&literal, limits)?);
    if bytes.len() > 262144
        || get(semantics, "source_canonical_json") != std::str::from_utf8(&bytes).ok()
        || get(left, "source_sha256") != Some(digest.as_str())
        || get(right, "source_sha256") != Some(digest.as_str())
        || get(right, "value_sha256") != Some(value_digest.as_str())
        || sha(field(right, "value"), limits)? != value_digest
        || sha(field(time, "raw"), limits)? != value_digest
        || get(value, "native_id") != Some(expected_native.as_str())
        || !matches!(field(left,"source_line"),JsonValue::Number(n) if n.kind==JsonNumberKind::Int && !n.lexeme.starts_with('-') && n.lexeme.bytes().any(|b| b != b'0'))
        || !(same_json(field(left, "source_line"), field(right, "source_line"))
            || (matches!(field(right, "source_line"), JsonValue::Bool(true))
                && matches!(field(left, "source_line"), JsonValue::Number(n) if n.lexeme == "1")))
        || field(claim, "source_refs") != field(value, "source_refs")
    {
        return Ok(Some("document-catalogue-exact-source-binding-inconsistent"));
    }
    Ok(None)
}
fn operand<L>(
    reference: &JsonValue,
    source_graph: &str,
    lookup: &mut L,
    limits: JsonLimits,
) -> Result<Operand, SearchV2Error>
where
    L: FnMut(&str) -> Result<Vec<JsonValue>, SearchV2Error>,
{
    let claims = lookup(get(reference, "node_id").ok_or_else(corrupt)?)?;
    if claims.len() != 1 {
        return Err(err(
            SearchV2ErrorCode::UnknownIdentifier,
            "expected one exact temporal Claim",
        ));
    }
    let claim = claims[0].clone();
    if field(&claim, "content_revision") != field(reference, "content_revision") {
        return Err(err(
            SearchV2ErrorCode::StaleSelection,
            "selected Claim content changed",
        ));
    }
    containers(&claim)?;
    let mut value = JsonValue::Null;
    let mut time = JsonValue::Null;
    macro_rules! stop {
        ($status:expr,$issue:expr) => {
            return Ok(operand_result(
                claim,
                value,
                time,
                $status,
                vec![$issue.to_owned()],
            ));
        };
    }
    if get(&claim, "source_graph") != Some(source_graph)
        || get(&claim, "kind_id") != Some("claim")
        || get(&claim, "type_id") != Some("tos.entity.claim")
        || get(field(&claim, "type_mapping"), "status") != Some("mapped")
    {
        stop!("unsupported", "selected-node-is-not-a-source-claim");
    }
    let semantics = path(&claim, &["semantics", "claim"]);
    let source = path(&claim, &["attributes", "source_claim"]);
    if source.as_object().is_none()
        || get(source, "claim_id").is_none()
        || field(source, "claim_id") != field(semantics, "claim_id")
        || !safe_integer(field(source, "claim_version")).is_some_and(|n| n > 0)
        || safe_integer(field(semantics, "claim_version")).is_none()
        || !same_json(
            field(source, "claim_version"),
            field(semantics, "claim_version"),
        )
        || field(source, "predicate") != field(semantics, "source_predicate_id")
        || get(semantics, "predicate_mapping_status") != Some("mapped")
    {
        stop!("undetermined", "claim-source-binding-inconsistent");
    }
    let Some(id) = get(semantics, "object_node_id") else {
        stop!("undetermined", "claim-object-binding-unavailable");
    };
    let values = lookup(id)?;
    if values.len() != 1 {
        stop!("undetermined", "claim-object-unavailable-or-ambiguous");
    }
    value = values[0].clone();
    containers(&value)?;
    if get(&value, "source_graph") != Some(source_graph)
        || get(&value, "type_id") != Some("tos.entity.temporal-assertion")
        || get(field(&value, "type_mapping"), "status") != Some("mapped")
    {
        stop!(
            "unsupported",
            "claim-object-is-not-a-declared-temporal-assertion"
        );
    }
    let attributes = field(&value, "attributes");
    let doc = get(source, "predicate") == Some("document_catalogue_date")
        || get(source, "schema_version") == Some("tos_document_catalogue_claim_v1")
        || get(path(&value, &["semantics", "time"]), "role")
            == Some("catalogue-assigned-document-date");
    if field(attributes, "claim_ref") != field(source, "claim_id")
        || attributes.object_get("value").is_none()
        || (!doc && !same_json(field(attributes, "value"), field(source, "object")))
    {
        stop!(
            "undetermined",
            "temporal-object-source-binding-inconsistent"
        );
    }
    let declared = path(&value, &["semantics", "time"]);
    if declared.as_object().is_none() {
        stop!("undetermined", "temporal-normalization-unavailable");
    }
    time = declared.clone();
    if time.object_get("raw").is_none()
        || (!doc && !same_json(field(&time, "raw"), field(attributes, "value")))
    {
        stop!(
            "undetermined",
            "temporal-normalization-source-binding-inconsistent"
        );
    }
    if doc {
        if let Some(issue) = documentary(
            &claim,
            &value,
            source,
            semantics,
            &time,
            source_graph,
            lookup,
            limits,
        )? {
            stop!("undetermined", issue);
        }
    }
    let mut issues = match time.object_get("issues") {
        None => vec![],
        Some(JsonValue::Array(values))
            if values
                .iter()
                .all(|v| v.as_str().is_some_and(|v| !v.is_empty())) =>
        {
            values
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        }
        _ => {
            stop!("unsupported", "temporal-normalization-issues-invalid");
        }
    };
    if !matches!(
        get(&time, "kind"),
        Some("date-assertion" | "interval-assertion" | "relative-order" | "unknown-date")
    ) {
        stop!("unsupported", "unsupported-temporal-normalization-kind");
    }
    let mut unsupported = issues
        .iter()
        .filter(|value| {
            value.starts_with("conflicting-")
                || matches!(
                    value.as_str(),
                    "reversed-interval" | "invalid-date-parts" | "unparsed-date-value"
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    if !field(&time, "calendar").is_null()
        && !matches!(
            get(&time, "calendar"),
            Some("gregorian" | "proleptic-gregorian")
        )
    {
        unsupported.push("unsupported-declared-calendar".into());
    }
    if !field(&time, "declared_year_numbering").is_null()
        && !matches!(get(&time, "declared_year_numbering"), Some("astronomical"))
    {
        unsupported.push("unsupported-declared-year-numbering".into());
    }
    if !unsupported.is_empty() {
        issues.extend(unsupported);
        return Ok(operand_result(
            claim,
            value,
            time,
            "unsupported",
            unique_issues(issues),
        ));
    }
    if !matches!(
        get(&time, "kind"),
        Some("date-assertion" | "interval-assertion")
    ) {
        issues.push("no-absolute-date-bounds".into());
    }
    if field(&time, "calendar").is_null() {
        issues.push("declared-calendar-unavailable".into());
    }
    if field(&time, "declared_year_numbering").is_null() {
        issues.push("declared-year-numbering-unavailable".into());
    }
    if matches!(
        get(&time, "precision"),
        Some("approximate" | "uncertain" | "unknown")
    ) {
        issues.push("non-exact-date-precision".into());
    }
    for (key, expected, issue) in [
        ("certainty", "exact", "explicit-exact-certainty-unavailable"),
        (
            "comparison_calendar",
            "proleptic-gregorian",
            "comparison-calendar-unavailable",
        ),
        (
            "year_numbering",
            "astronomical",
            "comparison-year-numbering-unavailable",
        ),
    ] {
        if get(&time, key) != Some(expected) {
            issues.push(issue.into());
        }
    }
    match (
        safe_integer(field(&time, "sort_start")),
        safe_integer(field(&time, "sort_end")),
    ) {
        (Some(a), Some(b)) if a > b => issues.push("reversed-date-envelope".into()),
        (Some(_), Some(_)) => {}
        _ => issues.push("absolute-date-envelope-unavailable".into()),
    }
    let status = if issues.is_empty() {
        "comparable"
    } else {
        "undetermined"
    };
    Ok(operand_result(
        claim,
        value,
        time,
        status,
        unique_issues(issues),
    ))
}
/// Maintained request validation before any carrier I/O. Revision equality
/// remains a selected snapshot check in the comparator.
pub fn validate_temporal_request(request: &JsonValue) -> Result<(), SearchV2Error> {
    if !request.as_object().is_some_and(|fields| fields.len() == 4)
        || !["schema_version", "source_revision", "left", "right"]
            .iter()
            .all(|key| request.object_get(key).is_some())
        || get(request, "schema_version") != Some("tos_temporal_comparison_request_v1")
        || !get(request, "source_revision").is_some_and(bare)
    {
        return Err(err(
            SearchV2ErrorCode::InvalidRequest,
            "invalid temporal request",
        ));
    }
    for side in ["left", "right"] {
        let reference = field(request, side);
        if !reference
            .as_object()
            .is_some_and(|fields| fields.len() == 2)
            || !matches!(field(reference, "node_id"), JsonValue::String(id) if {
                // Python request normalization accepts lone surrogates as string
                // code points. They remain unaddressable carriers (503), rather
                // than becoming a request-shape error (400). Replacement here
                // preserves length and whitespace solely for normalization;
                // operand lookup still requires the original valid UTF-8 ID.
                let normalized = String::from_utf16_lossy(id.units());
                !normalized.is_empty()
                    && python_strip_unicode16_v1(&normalized, 1024)
                        .is_ok_and(|stripped| normalized == stripped)
            })
            || !get(reference, "content_revision").is_some_and(bare)
        {
            return Err(err(
                SearchV2ErrorCode::InvalidRequest,
                "invalid temporal operand reference",
            ));
        }
    }
    Ok(())
}

/// Transport-neutral comparator; selected source graph is descriptor-derived.
/// All lookup requests use exact normalized IDs, with no alias fallback.
pub fn compare_temporal_operands<L>(
    revision: &str,
    request: &JsonValue,
    claim_source_graph: &str,
    mut lookup: L,
    limits: JsonLimits,
) -> Result<JsonValue, SearchV2Error>
where
    L: FnMut(&str) -> Result<Vec<JsonValue>, SearchV2Error>,
{
    validate_temporal_request(request)?;
    if get(request, "source_revision") != Some(revision) {
        return Err(err(
            SearchV2ErrorCode::StaleSelection,
            "temporal source revision changed",
        ));
    }
    let left = operand(
        field(request, "left"),
        claim_source_graph,
        &mut lookup,
        limits,
    )?;
    let right = operand(
        field(request, "right"),
        claim_source_graph,
        &mut lookup,
        limits,
    )?;
    let mut status = if left.status == "unsupported" || right.status == "unsupported" {
        "unsupported"
    } else if left.status == "undetermined" || right.status == "undetermined" {
        "undetermined"
    } else {
        "comparable"
    };
    let mut reasons = vec![];
    for (side, operand) in [("left", &left), ("right", &right)] {
        for issue in &operand.issues {
            reasons.push(object(vec![("side", text(side)), ("code", text(issue))]));
        }
    }
    let mut relation = JsonValue::Null;
    if status == "comparable" {
        let a = field(&left.packet, "normalized_time");
        let b = field(&right.packet, "normalized_time");
        let left_role = get(a, "role").filter(|role| !role.is_empty());
        let right_role = get(b, "role").filter(|role| !role.is_empty());
        let issue = if left_role.is_none() || right_role.is_none() {
            status = "undetermined";
            Some("time-role-unavailable")
        } else if left_role != right_role {
            status = "unsupported";
            Some("different-time-roles")
        } else if !matches!(
            left_role,
            Some("historical-time" | "catalogue-assigned-document-date")
        ) {
            status = "unsupported";
            Some("unsupported-time-role")
        } else {
            None
        };
        if let Some(issue) = issue {
            reasons.push(object(vec![("side", text("pair")), ("code", text(issue))]));
        } else {
            let (a, b, c, d) = (
                safe_integer(field(a, "sort_start")).unwrap(),
                safe_integer(field(a, "sort_end")).unwrap(),
                safe_integer(field(b, "sort_start")).unwrap(),
                safe_integer(field(b, "sort_end")).unwrap(),
            );
            relation = text(if b < c {
                "before"
            } else if d < a {
                "after"
            } else if a == c && b == d {
                "equal"
            } else if a <= c && b >= d {
                "contains"
            } else if c <= a && d >= b {
                "contained-by"
            } else {
                "overlaps"
            });
        }
    }
    let mut refs = BTreeSet::new();
    for operand in [&left, &right] {
        for key in ["claim", "value"] {
            if let Some(values) = field(field(&operand.packet, key), "source_refs").as_array() {
                for value in values {
                    if let Some(value) = value.as_str() {
                        refs.insert(value.to_owned());
                    }
                }
            }
        }
    }
    Ok(object(vec![
        ("schema_version", text("tos_temporal_comparison_result_v1")),
        ("source_revision", text(revision)),
        // The maintained normalizer fixes root order while preserving each
        // accepted operand reference's member order. Native canonical output
        // remains identical; published compact output retains this order.
        (
            "request",
            object(vec![
                ("schema_version", field(request, "schema_version").clone()),
                ("source_revision", field(request, "source_revision").clone()),
                ("left", field(request, "left").clone()),
                ("right", field(request, "right").clone()),
            ]),
        ),
        (
            "comparison",
            object(vec![
                ("status", text(status)),
                ("relation", relation),
                ("reasons", JsonValue::Array(reasons)),
                ("basis", text("normalized-source-date-envelopes")),
            ]),
        ),
        ("left", left.packet),
        ("right", right.packet),
        (
            "source_refs",
            JsonValue::Array(refs.into_iter().map(|v| text(&v)).collect()),
        ),
        (
            "authority_boundary",
            object(vec![
                ("is_source", JsonValue::Bool(false)),
                ("writes_to_tree", JsonValue::Bool(false)),
                ("performs_assessment", JsonValue::Bool(false)),
                ("creates_inferred_claim", JsonValue::Bool(false)),
                ("comparison_basis", text("normalized-source-date-envelopes")),
                (
                    "note",
                    text(
                        "Relations describe date envelopes only. They do not establish event simultaneity, duration, causality, identity or the truth/admission of either Claim. Numeric keys are ordering keys, not timestamps or elapsed-time quantities.",
                    ),
                ),
            ]),
        ),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tos_foundation::{JsonMode, parse_json};

    const REVISION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const CONTENT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const RAW: &str =
        r#"{"kind":"date","wording":"Synthetic disputed date","extension":[false,null,[]]}"#;

    fn parsed(raw: &str) -> JsonValue {
        parse_json(
            raw.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .expect("fixture JSON is valid")
        .into_root()
    }

    fn record_with_raw(
        side: &str,
        raw: &str,
        start: i64,
        end: i64,
        role: &str,
        calendar: &str,
        numbering: &str,
        precision: &str,
        certainty: &str,
        issues: &str,
    ) -> (JsonValue, JsonValue) {
        let claim_id = format!("claim-{side}");
        let value_id = format!("value-{side}");
        let claim = parsed(&format!(
            r#"{{
                "source_graph":"fixture","kind_id":"claim","type_id":"tos.entity.claim",
                "type_mapping":{{"status":"mapped"}},"content_revision":"{CONTENT}",
                "attributes":{{"source_claim":{{"claim_id":"{claim_id}","claim_version":1,
                    "predicate":"historical_dating","object":{raw},"polarity":"negative",
                    "review_status":"unreviewed","qualifiers":{{"extension":[false,null,[]]}}}}}},
                "semantics":{{"claim":{{"claim_id":"{claim_id}","claim_version":1,
                    "object_node_id":"{value_id}","source_predicate_id":"historical_dating",
                    "predicate_mapping_status":"mapped"}}}},
                "source_refs":["witness:synthetic"],"native_id":"{claim_id}"
            }}"#,
            CONTENT = CONTENT,
        ));
        let value = parsed(&format!(
            r#"{{
                "source_graph":"fixture","type_id":"tos.entity.temporal-assertion",
                "type_mapping":{{"status":"mapped"}},"native_id":"{value_id}",
                "attributes":{{"claim_ref":"{claim_id}","value":{raw}}},
                "semantics":{{"time":{{"raw":{raw},"kind":"date-assertion","issues":{issues},
                    "calendar":"{calendar}","declared_year_numbering":"{numbering}",
                    "precision":"{precision}","certainty":"{certainty}",
                    "comparison_calendar":"proleptic-gregorian","year_numbering":"astronomical",
                    "sort_start":{start},"sort_end":{end},"role":"{role}"}}}},
                "source_refs":["witness:synthetic","witness:shared"]
            }}"#,
        ));
        (claim, value)
    }

    fn record(
        side: &str,
        start: i64,
        end: i64,
        role: &str,
        calendar: &str,
        numbering: &str,
        precision: &str,
        certainty: &str,
        issues: &str,
    ) -> (JsonValue, JsonValue) {
        record_with_raw(
            side, RAW, start, end, role, calendar, numbering, precision, certainty, issues,
        )
    }

    fn request_with_revision(source_revision: &str, left_id: &str, right_id: &str) -> JsonValue {
        object(vec![
            ("schema_version", text("tos_temporal_comparison_request_v1")),
            ("source_revision", text(source_revision)),
            (
                "left",
                object(vec![
                    ("node_id", text(left_id)),
                    ("content_revision", text(CONTENT)),
                ]),
            ),
            (
                "right",
                object(vec![
                    ("node_id", text(right_id)),
                    ("content_revision", text(CONTENT)),
                ]),
            ),
        ])
    }

    fn request(left_id: &str, right_id: &str) -> JsonValue {
        request_with_revision(REVISION, left_id, right_id)
    }

    fn compare(left: (JsonValue, JsonValue), right: (JsonValue, JsonValue)) -> JsonValue {
        let rows = BTreeMap::from([
            ("claim-left".to_owned(), vec![left.0]),
            ("value-left".to_owned(), vec![left.1]),
            ("claim-right".to_owned(), vec![right.0]),
            ("value-right".to_owned(), vec![right.1]),
        ]);
        compare_temporal_operands(
            REVISION,
            &request("claim-left", "claim-right"),
            "fixture",
            |id| Ok(rows.get(id).cloned().unwrap_or_default()),
            JsonLimits::default(),
        )
        .expect("valid selected fixture")
    }

    fn fixture(side: &str, start: i64, end: i64) -> (JsonValue, JsonValue) {
        record(
            side,
            start,
            end,
            "historical-time",
            "gregorian",
            "astronomical",
            "exact",
            "exact",
            "[]",
        )
    }

    fn comparison(result: &JsonValue) -> (&str, Option<&str>) {
        let value = field(result, "comparison");
        (
            get(value, "status").expect("comparison status"),
            field(value, "relation").as_str(),
        )
    }

    #[test]
    fn declared_envelopes_cover_all_relations_and_inclusive_touching_bounds() {
        for (left, right, expected) in [
            ((1, 2), (3, 4), "before"),
            ((3, 4), (1, 2), "after"),
            ((1, 2), (1, 2), "equal"),
            ((1, 5), (2, 4), "contains"),
            ((2, 4), (1, 5), "contained-by"),
            ((1, 3), (3, 5), "overlaps"),
            ((1, 4), (3, 6), "overlaps"),
        ] {
            let result = compare(
                fixture("left", left.0, left.1),
                fixture("right", right.0, right.1),
            );
            assert_eq!(comparison(&result), ("comparable", Some(expected)));
            assert_eq!(
                field(
                    &field(&result, "authority_boundary"),
                    "creates_inferred_claim"
                ),
                &JsonValue::Bool(false)
            );
            assert_eq!(
                field(&field(&result, "authority_boundary"), "performs_assessment"),
                &JsonValue::Bool(false)
            );
        }
    }

    #[test]
    fn uncertainty_roles_and_declared_calendars_never_become_false_relations() {
        let uncertain = record(
            "left",
            1,
            2,
            "historical-time",
            "gregorian",
            "astronomical",
            "approximate",
            "exact",
            "[]",
        );
        assert_eq!(
            comparison(&compare(uncertain, fixture("right", 3, 4))),
            ("undetermined", None)
        );

        let unsupported_calendar = record(
            "left",
            1,
            2,
            "historical-time",
            "julian",
            "astronomical",
            "exact",
            "exact",
            "[]",
        );
        assert_eq!(
            comparison(&compare(unsupported_calendar, fixture("right", 3, 4))).0,
            "unsupported"
        );

        let mismatched_role = record(
            "left",
            1,
            2,
            "witness-time",
            "gregorian",
            "astronomical",
            "exact",
            "exact",
            "[]",
        );
        let result = compare(mismatched_role, fixture("right", 3, 4));
        assert_eq!(comparison(&result), ("unsupported", None));
        assert!(
            field(&field(&result, "comparison"), "reasons")
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| get(reason, "code") == Some("different-time-roles"))
        );

        let same_unknown_role = record(
            "left",
            1,
            2,
            "invented-role",
            "gregorian",
            "astronomical",
            "exact",
            "exact",
            "[]",
        );
        let same_unknown_role_right = record(
            "right",
            3,
            4,
            "invented-role",
            "gregorian",
            "astronomical",
            "exact",
            "exact",
            "[]",
        );
        let result = compare(same_unknown_role, same_unknown_role_right);
        assert_eq!(comparison(&result), ("unsupported", None));
        assert!(
            field(&field(&result, "comparison"), "reasons")
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| get(reason, "code") == Some("unsupported-time-role"))
        );
    }

    #[test]
    fn exact_selection_preserves_source_details_and_rejects_alias_or_revision_drift() {
        let result = compare(fixture("left", 1, 2), fixture("right", 3, 4));
        let claim = field(&field(&result, "left"), "claim");
        assert_eq!(field(claim, "native_id"), &text("claim-left"));
        assert_eq!(field(claim, "content_revision"), &text(CONTENT));
        assert_eq!(
            field(&field(claim, "attributes"), "source_claim")
                .object_get("polarity")
                .unwrap(),
            &text("negative")
        );
        assert_eq!(
            field(&field(claim, "attributes"), "source_claim")
                .object_get("qualifiers")
                .unwrap()
                .object_get("extension")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            field(&field(&result, "left"), "normalized_time")
                .object_get("raw")
                .unwrap(),
            &parsed(RAW)
        );
        assert_eq!(
            field(&field(&result, "authority_boundary"), "is_source"),
            &JsonValue::Bool(false)
        );

        let left = fixture("left", 1, 2);
        let right = fixture("right", 3, 4);
        let rows = BTreeMap::from([
            ("claim-left".to_owned(), vec![left.0]),
            ("value-left".to_owned(), vec![left.1]),
            ("claim-right".to_owned(), vec![right.0]),
            ("value-right".to_owned(), vec![right.1]),
        ]);
        let alias_request = request("claim-left-native-alias", "claim-right");
        assert_eq!(
            compare_temporal_operands(
                REVISION,
                &alias_request,
                "fixture",
                |id| Ok(rows.get(id).cloned().unwrap_or_default()),
                JsonLimits::default(),
            )
            .unwrap_err()
            .code,
            SearchV2ErrorCode::UnknownIdentifier,
        );
        let stale = request_with_revision(CONTENT, "claim-left", "claim-right");
        assert_eq!(
            compare_temporal_operands(
                REVISION,
                &stale,
                "fixture",
                |id| Ok(rows.get(id).cloned().unwrap_or_default()),
                JsonLimits::default(),
            )
            .unwrap_err()
            .code,
            SearchV2ErrorCode::StaleSelection,
        );
    }

    #[test]
    fn malformed_request_and_temporal_provenance_fail_closed() {
        let malformed = object(vec![
            ("schema_version", text("tos_temporal_comparison_request_v1")),
            ("source_revision", text(REVISION)),
            (
                "left",
                request("claim-left", "claim-right")
                    .object_get("left")
                    .unwrap()
                    .clone(),
            ),
            (
                "right",
                request("claim-left", "claim-right")
                    .object_get("right")
                    .unwrap()
                    .clone(),
            ),
            ("alias", JsonValue::Null),
        ]);
        assert_eq!(
            validate_temporal_request(&malformed).unwrap_err().code,
            SearchV2ErrorCode::InvalidRequest
        );

        let (claim, _) = fixture("left", 1, 2);
        let mismatched_value = record_with_raw(
            "left",
            r#"{"kind":"changed"}"#,
            1,
            2,
            "historical-time",
            "gregorian",
            "astronomical",
            "exact",
            "exact",
            "[]",
        )
        .1;
        let result = compare((claim, mismatched_value), fixture("right", 3, 4));
        assert_eq!(comparison(&result), ("undetermined", None));
        assert!(
            !field(&field(&result, "comparison"), "reasons")
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn json_boolean_aliases_and_unsafe_numeric_envelopes_are_refused() {
        assert!(same_json(&parsed("1"), &parsed("1.0")));
        assert!(!same_json(&parsed("true"), &parsed("1")));
        assert!(!same_json(&parsed("1"), &parsed("1.5")));
        assert_eq!(
            safe_integer(&parsed("9007199254740991")),
            Some(9_007_199_254_740_991)
        );
        assert_eq!(safe_integer(&parsed("9007199254740992")), None);
        assert_eq!(safe_integer(&parsed("true")), None);

        let unsafe_envelope = record(
            "left",
            9_007_199_254_740_992,
            9_007_199_254_740_993,
            "historical-time",
            "gregorian",
            "astronomical",
            "exact",
            "exact",
            "[]",
        );
        assert_eq!(
            comparison(&compare(unsafe_envelope, fixture("right", 3, 4))),
            ("undetermined", None)
        );
    }
}
