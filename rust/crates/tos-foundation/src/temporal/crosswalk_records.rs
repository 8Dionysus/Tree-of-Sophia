//! Bounded verification of supplied immutable crosswalk records. Preparing a
//! candidate neither admits nor persists source; no catalog, writer or CAS is
//! invented here. Every conversion pins one exact supplied revision.
use super::*;
use crate::{Digest256, RecordVersion, SourceRevision, StableId};

struct Verified {
    reference: ExactRecordRef,
    revision: SourceRevision,
    payload: JsonValue,
}
fn malformed(message: &'static str) -> TemporalError {
    TemporalError::MalformedRequest(message)
}
fn exact_ref(v: &JsonValue) -> Result<ExactRecordRef, TemporalError> {
    Ok(ExactRecordRef {
        id: StableId::parse(string(v, "id").ok_or(malformed("record id required"))?)
            .map_err(|_| malformed("invalid record id"))?,
        version: RecordVersion::new(
            field(v, "version")
                .as_u64()
                .ok_or(malformed("record version required"))?,
        )
        .map_err(|_| malformed("invalid record version"))?,
        digest: Digest256::from_prefixed(
            string(v, "digest").ok_or(malformed("record digest required"))?,
        )
        .map_err(|_| malformed("invalid record digest"))?,
    })
}
fn revision(v: &JsonValue) -> Result<SourceRevision, TemporalError> {
    Ok(SourceRevision(
        Digest256::from_prefixed(v.as_str().ok_or(malformed("source revision required"))?)
            .map_err(|_| malformed("invalid source revision"))?,
    ))
}
fn trs(v: &JsonValue) -> Result<TrsRef, TemporalError> {
    let id = string(v, "id")
        .filter(|s| !s.is_empty())
        .ok_or(malformed("TRS id required"))?;
    let version = string(v, "version")
        .filter(|s| !s.is_empty())
        .ok_or(malformed("TRS version required"))?;
    Ok(TrsRef {
        id: id.into(),
        version: version.into(),
    })
}
fn load(
    env: &JsonValue,
    requested: &JsonValue,
    expected: &JsonValue,
    budget: TemporalBudget,
) -> Result<Verified, TemporalError> {
    let reference = exact_ref(field(env, "record_ref"))?;
    if reference != exact_ref(requested)? {
        return Err(malformed("requested record reference mismatch"));
    }
    let rev = revision(field(env, "source_revision"))?;
    if rev != revision(expected)? {
        return Err(malformed("source revision fence mismatch"));
    }
    let raw = string(env, "record_bytes").ok_or(malformed("UTF8 record bytes required"))?;
    if raw.len() > budget.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted(
            "crosswalk record byte limit",
        ));
    }
    if Digest256::of_bytes(raw.as_bytes()) != reference.digest {
        return Err(malformed("record byte digest mismatch"));
    }
    let limits = crate::JsonLimits {
        max_bytes: budget.max_expression_bytes,
        max_depth: 32,
        max_visits: budget.max_members.saturating_mul(16).max(1),
        max_integer_digits: 20,
    };
    let payload = crate::parse_json(raw.as_bytes(), crate::JsonMode::PublishedStrict, limits)
        .map_err(|_| malformed("invalid crosswalk record JSON"))?
        .root()
        .clone();
    if payload.as_object().is_none() {
        return Err(malformed("crosswalk payload object required"));
    }
    Ok(Verified {
        reference,
        revision: rev,
        payload,
    })
}
fn validate(v: &Verified, budget: TemporalBudget) -> Result<Option<JsonValue>, TemporalError> {
    let p = &v.payload;
    trs(field(p, "source_trs"))?;
    trs(field(p, "target_trs"))?;
    if string(p, "rule_profile") != Some("finite-mapping-v1") {
        return Ok(Some(reason(
            "Unsupported",
            "UnsupportedCrosswalkRuleProfile",
        )));
    }
    string(p, "rules_version")
        .filter(|s| !s.is_empty())
        .ok_or(malformed("rules version required"))?;
    let a = field(p, "applicability");
    let lo = integer(field(a, "min")).ok_or(malformed("integer applicability min required"))?;
    let hi = integer(field(a, "max")).ok_or(malformed("integer applicability max required"))?;
    if lo > hi {
        return Err(malformed("inverted crosswalk applicability"));
    }
    for key in ["unit", "origin"] {
        string(a, key)
            .filter(|s| !s.is_empty())
            .ok_or(malformed("crosswalk axis unit and origin required"))?;
    }
    let rows = field(p, "rows")
        .as_object()
        .ok_or(malformed("mapping rows object required"))?;
    if rows.len() > budget.max_members {
        return Err(TemporalError::ResourceExhausted("crosswalk row limit"));
    }
    for (key, value) in rows {
        let s = key.as_str().ok_or(malformed("mapping key must be UTF8"))?;
        let n: i64 = s
            .parse()
            .map_err(|_| malformed("mapping key must be integer"))?;
        if n.to_string() != s {
            return Err(malformed("mapping key must use canonical integer spelling"));
        }
        if n < lo || n > hi {
            return Err(malformed("mapping row outside declared applicability"));
        }
        if integer(value).is_none() {
            return Ok(Some(reason(
                "Unsupported",
                "NonIntegerFiniteMappingOutcome",
            )));
        }
    }
    if field(p, "provenance").as_object().is_none()
        || field(p, "provenance").as_object().unwrap().is_empty()
    {
        return Err(malformed("crosswalk provenance object required"));
    }
    match (v.reference.version.get(), field(p, "predecessor").is_null()) {
        (1, false) => return Err(malformed("first crosswalk version cannot have predecessor")),
        (1, true) => {}
        (_, true) => return Err(malformed("crosswalk predecessor reference required")),
        (_, false) => {
            let prior = exact_ref(field(p, "predecessor"))?;
            if prior.id != v.reference.id
                || prior.version.get().checked_add(1) != Some(v.reference.version.get())
            {
                return Err(malformed(
                    "crosswalk predecessor stream or version mismatch",
                ));
            }
        }
    }
    Ok(None)
}
fn finish(mut out: JsonValue, input: &JsonValue, context: &JsonValue) -> JsonValue {
    put(&mut out, "preserved_input", input.clone());
    put(&mut out, "preserved_context", context.clone());
    put(
        &mut out,
        "adapter_version",
        text("tos-supplied-crosswalk-record-v1"),
    );
    put(&mut out, "persisted", JsonValue::Bool(false));
    put(&mut out, "scope", text("supplied-record-verification-only"));
    out
}
fn check_shapes(input: &JsonValue, context: &JsonValue) -> Result<(), TemporalError> {
    if input.as_object().is_none() || context.as_object().is_none() {
        return Err(malformed("input and context must be objects"));
    }
    Ok(())
}

pub fn prepare_crosswalk_version(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    check_shapes(input, context)?;
    let env = field(input, "record");
    let expected = field(input, "expected_source_revision");
    let v = load(env, field(input, "requested_ref"), expected, budget)?;
    if let Some(r) = validate(&v, budget)? {
        return Ok(finish(r, input, context));
    }
    if v.reference.version.get() > 1 {
        let prior = load(
            field(input, "predecessor_record"),
            field(&v.payload, "predecessor"),
            expected,
            budget,
        )?;
        if let Some(r) = validate(&prior, budget)? {
            return Ok(finish(r, input, context));
        }
        if prior.reference.id != v.reference.id
            || prior.reference.version.get().checked_add(1) != Some(v.reference.version.get())
        {
            return Err(malformed(
                "crosswalk predecessor stream or version mismatch",
            ));
        }
        if prior.revision != v.revision {
            return Err(malformed("crosswalk predecessor fence mismatch"));
        }
        for key in ["source_trs", "target_trs"] {
            if trs(field(&prior.payload, key))? != trs(field(&v.payload, key))? {
                return Err(malformed("crosswalk predecessor edge mismatch"));
            }
        }
        for key in ["unit", "origin"] {
            if string(field(&prior.payload, "applicability"), key)
                != string(field(&v.payload, "applicability"), key)
            {
                return Err(malformed("crosswalk predecessor axis mismatch"));
            }
        }
    } else if !field(input, "predecessor_record").is_null() {
        return Err(malformed(
            "unexpected predecessor envelope for first version",
        ));
    }
    Ok(finish(
        obj(vec![
            ("status", text("PreparedCrosswalkVersionCandidate")),
            ("candidate_record", env.clone()),
            ("candidate_payload", v.payload),
            ("writer_binding", text("PendingOwnerBinding")),
        ]),
        input,
        context,
    ))
}

pub fn convert_crosswalk_record(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    check_shapes(input, context)?;
    let env = field(input, "record");
    let v = load(
        env,
        field(input, "requested_ref"),
        field(input, "expected_source_revision"),
        budget,
    )?;
    if let Some(r) = validate(&v, budget)? {
        return Ok(finish(r, input, context));
    }
    let p = field(input, "position");
    if trs(field(p, "trs_ref"))? != trs(field(&v.payload, "source_trs"))?
        || trs(field(input, "target_trs"))? != trs(field(&v.payload, "target_trs"))?
    {
        return Ok(finish(
            reason("Incomparable", "CrosswalkTrsMismatch"),
            input,
            context,
        ));
    }
    if p.object_get("qualifier")
        .is_some_and(|q| q.as_str() != Some("None"))
        || p.object_get("uncertainty").is_some()
        || p.object_get("resolution")
            .is_some_and(|r| r.as_str() != Some("exact"))
    {
        return Ok(finish(
            reason("Unresolved", "NonExactPosition"),
            input,
            context,
        ));
    }
    let Some(n) = integer(field(p, "value")) else {
        return Ok(finish(
            reason("Unsupported", "IntegerCrosswalkCoordinateRequired"),
            input,
            context,
        ));
    };
    let a = field(&v.payload, "applicability");
    if string(p, "unit") != string(a, "unit") || string(p, "origin") != string(a, "origin") {
        return Ok(finish(
            reason("Incomparable", "CrosswalkAxisMismatch"),
            input,
            context,
        ));
    }
    let lo = integer(field(a, "min")).unwrap();
    let hi = integer(field(a, "max")).unwrap();
    if n < lo || n > hi {
        return Ok(finish(
            reason("OutOfCoverage", "CrosswalkApplicabilityExceeded"),
            input,
            context,
        ));
    }
    let Some(value) = field(&v.payload, "rows").object_get(&n.to_string()) else {
        return Ok(finish(
            reason("OutOfCoverage", "CrosswalkEntryAbsent"),
            input,
            context,
        ));
    };
    Ok(finish(
        obj(vec![
            ("status", text("Unique")),
            (
                "position",
                obj(vec![
                    ("value", value.clone()),
                    ("trs_ref", field(&v.payload, "target_trs").clone()),
                ]),
            ),
            ("crosswalk_record_ref", field(env, "record_ref").clone()),
            ("source_revision", field(env, "source_revision").clone()),
            ("rules_version", field(&v.payload, "rules_version").clone()),
            ("provenance", field(&v.payload, "provenance").clone()),
            ("precision_loss", JsonValue::Array(vec![])),
        ]),
        input,
        context,
    ))
}
