//! Supplemental possible-occurrence year buckets; never event duration/time.
//! Explicitly user-approved Possible-occurrence semantics require source
//! calendar/TRS/rules/basis. No list/auth/custody or writes.
//! All/Disjoint only for this year-only profile; no Some runtime capability.
use super::claim_carrier::{CarrierBasis, TemporalClaimCarrier};
use super::*;
use crate::{Digest256, ExactRecordRef, RecordVersion, SourceRevision, StableId};

type Domain<T> = Result<T, &'static str>;
fn strict(v: &JsonValue, keys: &[&'static str]) -> Domain<()> {
    let m = v.as_object().ok_or("ObjectRequired")?;
    for (i, (k, _)) in m.iter().enumerate() {
        if !keys.iter().any(|s| k.as_str() == Some(*s)) {
            return Err("UnsupportedProfileField");
        }
        if m[..i].iter().any(|(p, _)| p == k) {
            return Err("DuplicateProfileField");
        }
    }
    for k in keys {
        required(v, k)?;
    }
    Ok(())
}
fn required<'a>(v: &'a JsonValue, k: &'static str) -> Domain<&'a JsonValue> {
    v.object_get(k).ok_or(k)
}
fn word<'a>(v: &'a JsonValue, k: &'static str) -> Domain<&'a str> {
    required(v, k)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or(k)
}
fn tagged(v: &JsonValue, k: &'static str, expected: &str) -> Domain<()> {
    if word(v, k)? == expected {
        Ok(())
    } else {
        Err(k)
    }
}
fn reference(v: &JsonValue) -> Domain<ExactRecordRef> {
    strict(v, &["id", "version", "digest"])?;
    Ok(ExactRecordRef {
        id: StableId::parse(word(v, "id")?).map_err(|_| "InvalidExactReference")?,
        version: RecordVersion::new(
            required(v, "version")?
                .as_u64()
                .ok_or("InvalidExactReference")?,
        )
        .map_err(|_| "InvalidExactReference")?,
        digest: Digest256::from_hex(word(v, "digest")?).map_err(|_| "InvalidExactReference")?,
    })
}
fn basis(v: &JsonValue) -> Domain<()> {
    strict(v, &["source", "source_locator", "raw_evidence"])?;
    reference(required(v, "source")?)?;
    let l = required(v, "source_locator")?;
    if !l.is_null() && l.as_str().is_none() {
        return Err("InvalidSourceLocator");
    }
    Ok(())
}
struct Work {
    bytes: usize,
    visits: usize,
    comparisons: usize,
    limits: TemporalBudget,
}
impl Work {
    fn new(b: TemporalBudget) -> Result<Self, TemporalError> {
        if b.max_pairings == 0 || b.max_expression_bytes == 0 || b.max_members == 0 {
            return Err(exhausted());
        }
        Ok(Self {
            bytes: b.max_expression_bytes,
            visits: b.max_members,
            comparisons: b.max_pairings,
            limits: b,
        })
    }
    fn visit(&mut self, v: &JsonValue, depth: usize) -> Result<(), TemporalError> {
        self.visits = self.visits.checked_sub(1).ok_or_else(exhausted)?;
        if depth > 64 {
            return Err(exhausted());
        }
        match v {
            JsonValue::Array(a) => {
                for x in a {
                    self.visit(x, depth + 1)?;
                }
            }
            JsonValue::Object(m) => {
                for (i, (_, x)) in m.iter().enumerate() {
                    // Charge duplicate-key work as well as node visits.
                    self.visits = self.visits.checked_sub(i).ok_or_else(exhausted)?;
                    self.visit(x, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn admit(&mut self, v: &JsonValue) -> Result<Vec<u8>, TemporalError> {
        self.visit(v, 0)?;
        let raw = crate::emit_value_preserved_json(
            v,
            crate::JsonLimits {
                max_bytes: self.bytes,
                max_depth: 64,
                max_visits: self.limits.max_members,
                max_integer_digits: self.bytes,
            },
        )
        .map_err(|e| {
            if e.code == crate::FoundationErrorCode::BudgetExceeded {
                exhausted()
            } else {
                TemporalError::MalformedRequest("InvalidWorldWindowJson")
            }
        })?;
        self.bytes = self.bytes.checked_sub(raw.len()).ok_or_else(exhausted)?;
        Ok(raw)
    }
    fn charge(&mut self) -> Result<(), TemporalError> {
        self.comparisons = self.comparisons.checked_sub(1).ok_or_else(exhausted)?;
        Ok(())
    }
}
fn exhausted() -> TemporalError {
    TemporalError::ResourceExhausted("world window cumulative budget")
}
#[derive(Eq, PartialEq)]
struct Axis<'a> {
    id: &'a str,
    version: &'a str,
    rules: &'a str,
}
struct Bucket<'a> {
    start: calendar::Civil,
    end: calendar::Civil,
    axis: Axis<'a>,
}
fn bucket<'a>(v: &'a JsonValue, ctx: &JsonValue) -> Domain<Bucket<'a>> {
    strict(
        v,
        &[
            "type",
            "lexical",
            "resolution",
            "trs",
            "calendar_algorithm",
            "year_numbering",
            "rules_version",
        ],
    )?;
    tagged(v, "type", "CalendarPosition")?;
    tagged(v, "resolution", "year")?;
    tagged(v, "calendar_algorithm", "GregorianProleptic")?;
    tagged(v, "year_numbering", "astronomical")?;
    let trs = required(v, "trs")?;
    strict(trs, &["id", "version"])?;
    let axis = Axis {
        id: word(trs, "id")?,
        version: word(trs, "version")?,
        rules: word(v, "rules_version")?,
    };
    strict(ctx, &["temporal_systems"])?;
    let rules = calendar::admitted_rules(axis.id, ctx)?;
    strict(
        rules,
        &[
            "calendar_algorithm",
            "year_numbering",
            "rules_version",
            "trs_version",
        ],
    )?;
    if word(rules, "trs_version")? != axis.version || word(rules, "rules_version")? != axis.rules {
        return Err("ConflictingCalendarDeclaration");
    }
    let raw = word(v, "lexical")?;
    let (start, res) = calendar::parse_civil(raw).ok_or("MalformedYear")?;
    if res != "year" {
        return Err("YearPrecisionRequired");
    }
    // End must remain within the same explicitly supported four-digit domain.
    if start.0 >= 9999 {
        return Err("CalendarOutOfCoverage");
    }
    let end = calendar::next(start, "year").ok_or("CalendarOutOfCoverage")?;
    Ok(Bucket { start, end, axis })
}
struct Query<'a> {
    about: ExactRecordRef,
    bucket: Bucket<'a>,
}
fn query<'a>(v: &'a JsonValue, ctx: &JsonValue) -> Domain<Query<'a>> {
    strict(
        v,
        &[
            "profile_ref",
            "assertion_scope",
            "assertion_selection",
            "non_total",
            "predicate",
            "coverage_rule",
            "about_ref",
            "position",
            "basis",
        ],
    )?;
    tagged(v, "profile_ref", "tos_world_civil_year_window_filter_v1")?;
    tagged(v, "assertion_scope", "SupplementalCarrierOnly")?;
    tagged(v, "assertion_selection", "ExactlyOnePerPredicate")?;
    tagged(v, "non_total", "RefusePage")?;
    tagged(v, "predicate", "Possible")?;
    tagged(v, "coverage_rule", "GregorianCivilYearHalfOpenV1")?;
    basis(required(v, "basis")?)?;
    Ok(Query {
        about: reference(required(v, "about_ref")?)?,
        bucket: bucket(required(v, "position")?, ctx)?,
    })
}
/// Whole-query validation without fabricated Claim provenance; use even for an
/// empty list or metadata NoMatch. Context/rules are explicit admitted inputs.
pub fn validate_claim_world_window_filter(
    filter: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<(), TemporalError> {
    let mut work = Work::new(budget)?;
    work.admit(filter)?;
    work.admit(context)?;
    query(filter, context)
        .map(|_| ())
        .map_err(TemporalError::MalformedRequest)
}
fn ref_wire(r: &ExactRecordRef) -> JsonValue {
    obj(vec![
        ("id", text(r.id.as_str())),
        (
            "version",
            JsonValue::Number(crate::JsonNumber {
                kind: crate::JsonNumberKind::Int,
                lexeme: r.version.get().to_string(),
            }),
        ),
        ("digest", text(&r.digest.to_hex())),
    ])
}
fn basis_wire(b: &CarrierBasis) -> JsonValue {
    obj(vec![
        ("source", ref_wire(&b.source)),
        (
            "source_locator",
            b.source_locator
                .as_deref()
                .map(text)
                .unwrap_or(JsonValue::Null),
        ),
        ("raw_evidence", b.raw_evidence.clone()),
    ])
}
/// Pure supplemental predicate; caller authenticates member/current rights.
/// Refused is non-total and must dominate NoMatch in provider composition.
pub fn evaluate_claim_world_window_filter(
    carrier: Option<&TemporalClaimCarrier>,
    filter: &JsonValue,
    context: &JsonValue,
    claim_ref: &ExactRecordRef,
    source_revision: SourceRevision,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let mut work = Work::new(budget)?;
    let filter_raw = work.admit(filter)?;
    work.admit(context)?;
    let q = query(filter, context).map_err(TemporalError::MalformedRequest)?;
    if claim_ref.id.as_str().len() > work.bytes {
        return Err(exhausted());
    }
    let claim = ref_wire(claim_ref);
    let claim_size = work.admit(&claim)?.len();
    let mut refusal = None;
    let mut selected = None;
    let mut packet_digest = None;
    let mut packet_size = 0usize;
    if let Some(c) = carrier {
        let raw = work.admit(c.original_packet())?;
        packet_size = raw.len();
        packet_digest = Some(Digest256::of_bytes(&raw));
        if c.validity().is_some() {
            refusal = Some("PendingValidityReconciliation");
        }
        for (index, binding) in c.roles().unwrap_or(&[]).iter().enumerate() {
            work.charge()?;
            if !matches!(
                binding.role,
                TemporalRole::WorldOccurrence | TemporalRole::ActualInterval
            ) {
                continue;
            }
            let a = &binding.assertion;
            let parsed = (|| {
                if binding.role != TemporalRole::WorldOccurrence {
                    return Err("UnsupportedWorldAssertionCategory");
                }
                strict(
                    a,
                    &[
                        "profile_ref",
                        "category",
                        "coverage_rule",
                        "about_ref",
                        "position",
                    ],
                )?;
                tagged(a, "profile_ref", "tos_world_civil_year_occurrence_v1")?;
                tagged(a, "category", "OccurrencePossibleTimeBucket")?;
                tagged(a, "coverage_rule", "GregorianCivilYearHalfOpenV1")?;
                Ok((
                    reference(required(a, "about_ref")?)?,
                    bucket(required(a, "position")?, context)?,
                ))
            })();
            let (about, b) = match parsed {
                Ok(v) => v,
                Err(e) => {
                    refusal.get_or_insert(e);
                    continue;
                }
            };
            if about != q.about {
                continue;
            }
            if selected.is_some() {
                refusal.get_or_insert("MultipleWorldAssertions");
                continue;
            }
            if b.axis != q.bucket.axis {
                refusal.get_or_insert("IncompatibleCalendarAxes");
                continue;
            }
            work.charge()?;
            // Exact civil year buckets: overlapping implies equal coverage.
            let all = b.start == q.bucket.start && b.end == q.bucket.end;
            selected = Some((index, binding, all));
        }
        if selected.is_none() {
            refusal.get_or_insert("MissingWorldAssertion");
        }
    } else {
        refusal = Some("MissingTemporalCarrier");
    }
    // Reserve cumulative output duplication BEFORE copying any evidence/filter.
    // Assertion and basis are disjoint immutable codec subtrees, bounded by
    // one full emitted packet. The actual emitted ref includes escaped ID bytes;
    // its admission buffer above drops immediately, leaving only its usize size.
    let selected_size = if selected.is_some() { packet_size } else { 0 };
    let reserve = filter_raw
        .len()
        .checked_add(selected_size)
        .and_then(|n| n.checked_add(claim_size))
        .and_then(|n| n.checked_add(2048))
        .ok_or_else(exhausted)?;
    if reserve > work.bytes {
        return Err(exhausted());
    }
    // Forecast cloned subtrees and fixed result framing BEFORE cloning. This
    // probe does not reset the cumulative final-output allowance.
    let mut forecast = Work {
        bytes: work.bytes,
        visits: work.visits,
        comparisons: work.comparisons,
        limits: work.limits,
    };
    forecast.visit(filter, 1)?;
    if refusal.is_none() && selected.is_some() {
        forecast.visit(carrier.expect("selected carrier").original_packet(), 1)?;
    }
    // At most 21 framing fields (duplicate-key checks included), ref/basis
    // wrappers and digest nodes; conservative bound independent of raw size.
    forecast.visits.checked_sub(512).ok_or_else(exhausted)?;
    let all = selected.is_some_and(|(_, _, all)| all);
    let mut result = obj(vec![
        (
            "schema_version",
            text("tos_world_civil_year_window_result_v1"),
        ),
        (
            "outcome",
            text(if refusal.is_some() {
                "Refused"
            } else if all {
                "Match"
            } else {
                "NoMatch"
            }),
        ),
        ("assertion_scope", text("SupplementalCarrierOnly")),
        ("legacy_object_assessment", text("NotExamined")),
        ("event_extent_inferred", JsonValue::Bool(false)),
        ("predicate", text("Possible")),
        ("coverage_rule", text("GregorianCivilYearHalfOpenV1")),
        ("claim_ref", claim),
        ("source_revision", text(&source_revision.0.to_prefixed())),
        (
            "query_emitted_json_sha256",
            text(&Digest256::of_bytes(&filter_raw).to_prefixed()),
        ),
        ("preserved_filter", filter.clone()),
    ]);
    if let Some(e) = refusal {
        put(&mut result, "reason", text(e));
    } else if let Some((index, binding, yes)) = selected {
        put(
            &mut result,
            "classification",
            text(if yes { "All" } else { "Disjoint" }),
        );
        put(
            &mut result,
            "assertion_index",
            number(i64::try_from(index).map_err(|_| exhausted())?),
        );
        put(&mut result, "source_assertion", binding.assertion.clone());
        put(&mut result, "source_basis", basis_wire(&binding.basis));
    }
    if let Some(d) = packet_digest {
        put(
            &mut result,
            "source_packet_emitted_json_sha256",
            text(&d.to_prefixed()),
        );
    }
    // Final output shares the admitted output/node/work allowance, not defaults.
    work.admit(&result)?;
    Ok(result)
}
