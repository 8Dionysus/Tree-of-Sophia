//! Opt-in predicates over supplemental source assertions, not historical truth.
//! The provider authenticates selected Claims, revisions and current access.
//! about_ref always names an explicit external record; Claim/self/subject/basis
//! and explanation provenance never supply missing identity, axis or cutoffs.
//! Legacy historicalDate is NotExamined; no calendar, clock, history or writes.
use super::claim_carrier::TemporalClaimCarrier;
use super::*;
use crate::{Digest256, RecordVersion, SourceRevision, StableId};

#[derive(Clone, Copy, Debug)]
struct Refusal {
    domain: &'static str,
    reason: &'static str,
}
type Domain<T> = Result<T, Refusal>;
fn unsupported(reason: &'static str) -> Refusal {
    Refusal {
        domain: "Unsupported",
        reason,
    }
}
fn unresolved(reason: &'static str) -> Refusal {
    Refusal {
        domain: "Unresolved",
        reason,
    }
}
fn strict(v: &JsonValue, allowed: &[&str]) -> Domain<()> {
    let m = v.as_object().ok_or(unsupported("ObjectRequired"))?;
    for (i, (k, _)) in m.iter().enumerate() {
        if !allowed.iter().any(|s| k.as_str() == Some(*s)) {
            return Err(unsupported("UnsupportedProfileField"));
        }
        if m[..i].iter().any(|(other, _)| other == k) {
            return Err(unsupported("DuplicateProfileField"));
        }
    }
    Ok(())
}
fn required<'a>(v: &'a JsonValue, k: &'static str) -> Domain<&'a JsonValue> {
    v.object_get(k).ok_or(unresolved(k))
}
fn word<'a>(v: &'a JsonValue, k: &'static str) -> Domain<&'a str> {
    required(v, k)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or(unsupported(k))
}
fn exact_ref(v: &JsonValue) -> Domain<ExactRecordRef> {
    strict(v, &["id", "version", "digest"])?;
    Ok(ExactRecordRef {
        id: StableId::parse(word(v, "id")?).map_err(|_| unsupported("InvalidExactReference"))?,
        version: RecordVersion::new(
            required(v, "version")?
                .as_u64()
                .ok_or(unsupported("InvalidExactReference"))?,
        )
        .map_err(|_| unsupported("InvalidExactReference"))?,
        digest: Digest256::from_hex(word(v, "digest")?)
            .map_err(|_| unsupported("InvalidExactReference"))?,
    })
}
fn coordinate(v: &JsonValue) -> Domain<Position> {
    strict(v, &["type", "resolution", "value", "trs", "unit", "origin"])?;
    if word(v, "type")? != "CoordinatePosition" || word(v, "resolution")? != "exact" {
        return Err(unsupported("ExactCoordinateRequired"));
    }
    let t = required(v, "trs")?;
    strict(t, &["id", "version"])?;
    Ok(Position::Coordinate {
        value: integer(required(v, "value")?).ok_or(unsupported("ExactI64CoordinateRequired"))?,
        unit: word(v, "unit")?.into(),
        origin: word(v, "origin")?.into(),
        trs: TrsRef {
            id: word(t, "id")?.into(),
            version: word(t, "version")?.into(),
        },
    })
}
fn query_basis(v: &JsonValue) -> Domain<()> {
    // Basis extensions/raw evidence are source metadata, not interpretation.
    let members = v.as_object().ok_or(unsupported("BasisRequired"))?;
    for (i, (key, _)) in members.iter().enumerate() {
        if members[..i].iter().any(|(other, _)| other == key) {
            return Err(unsupported("DuplicateProfileField"));
        }
    }
    exact_ref(required(v, "source")?)?;
    let loc = required(v, "source_locator")?;
    if !loc.is_null() && loc.as_str().is_none() {
        return Err(unsupported("InvalidSourceLocator"));
    }
    required(v, "raw_evidence")?;
    Ok(())
}
fn source_role(v: &JsonValue) -> Domain<TemporalRole> {
    match v.as_str() {
        Some("AttributedKnowledge") => Ok(TemporalRole::AttributedKnowledge),
        Some("SourcePublication") => Ok(TemporalRole::SourcePublication),
        Some("SystemTransaction") => Ok(TemporalRole::SystemTransaction),
        _ => Err(unsupported("UnsupportedSourceRole")),
    }
}
fn knower(v: &JsonValue, role: TemporalRole) -> Domain<Option<ExactRecordRef>> {
    if role == TemporalRole::AttributedKnowledge {
        exact_ref(required(v, "knower_ref")?).map(Some)
    } else if v.object_get("knower_ref").is_some() {
        Err(unsupported("KnowerNotApplicable"))
    } else {
        Ok(None)
    }
}
fn bounded(v: &JsonValue, b: TemporalBudget) -> Result<(), TemporalError> {
    crate::emit_value_preserved_json(
        v,
        crate::JsonLimits {
            max_bytes: b.max_expression_bytes,
            max_depth: 64,
            max_visits: b.max_members,
            max_integer_digits: b.max_expression_bytes,
        },
    )
    .map(|_| ())
    .map_err(|e| {
        if e.code == crate::FoundationErrorCode::BudgetExceeded {
            TemporalError::ResourceExhausted("coordinate filter byte/member/depth limit")
        } else {
            TemporalError::MalformedRequest("invalid coordinate filter JSON")
        }
    })
}
#[derive(Debug)]
struct RolePredicate {
    index: usize,
    role: TemporalRole,
    about: ExactRecordRef,
    knower: Option<ExactRecordRef>,
    cutoff: Position,
}
#[derive(Debug)]
struct WorldPredicate {
    about: ExactRecordRef,
    at: Position,
}
struct Filter {
    roles: Vec<RolePredicate>,
    world: Option<WorldPredicate>,
}
fn parse_filter(v: &JsonValue, b: TemporalBudget) -> Result<Filter, TemporalError> {
    bounded(v, b)?;
    let parse = || -> Domain<Filter> {
        strict(
            v,
            &[
                "profile_ref",
                "assertion_scope",
                "assertion_selection",
                "non_total",
                "roles",
                "world_valid_time",
            ],
        )?;
        if word(v, "profile_ref")? != "tos_claim_coordinate_filter_v1"
            || word(v, "assertion_scope")? != "SupplementalCarrierOnly"
            || word(v, "assertion_selection")? != "ExactlyOnePerPredicate"
            || word(v, "non_total")? != "RefusePage"
        {
            return Err(unsupported("UnsupportedFilterProfile"));
        }
        let rows = required(v, "roles")?
            .as_array()
            .ok_or(unsupported("RolePredicatesRequired"))?;
        let mut roles = Vec::new();
        for (index, p) in rows.iter().enumerate() {
            strict(
                p,
                &[
                    "role",
                    "about_ref",
                    "knower_ref",
                    "operator",
                    "cutoff",
                    "basis",
                ],
            )?;
            let role = source_role(required(p, "role")?)?;
            if word(p, "operator")? != "LessOrEqual" {
                return Err(unsupported("UnsupportedCutoffOperator"));
            }
            roles.push(RolePredicate {
                index,
                role,
                about: exact_ref(required(p, "about_ref")?)?,
                knower: knower(p, role)?,
                cutoff: coordinate(required(p, "cutoff")?)?,
            });
            query_basis(required(p, "basis")?)?;
        }
        let world = match required(v, "world_valid_time")? {
            JsonValue::Null => None,
            p => {
                strict(p, &["profile_ref", "about_ref", "at", "basis"])?;
                if word(p, "profile_ref")? != "tos_world_coordinate_at_filter_v1" {
                    return Err(unsupported("UnsupportedWorldFilterProfile"));
                }
                let world = WorldPredicate {
                    about: exact_ref(required(p, "about_ref")?)?,
                    at: coordinate(required(p, "at")?)?,
                };
                query_basis(required(p, "basis")?)?;
                Some(world)
            }
        };
        if roles.is_empty() && world.is_none() {
            return Err(unsupported("EmptyTemporalPredicate"));
        }
        Ok(Filter { roles, world })
    };
    parse().map_err(|e| TemporalError::MalformedRequest(e.reason))
}
/// Validate the whole query even for empty sources or metadata NoMatch. This
/// shares the evaluator parser and requires no fabricated Claim provenance.
pub fn validate_claim_coordinate_filter(
    filter: &JsonValue,
    budget: TemporalBudget,
) -> Result<(), TemporalError> {
    parse_filter(filter, budget).map(|_| ())
}
struct Work {
    left: usize,
    output_left: usize,
    budget: TemporalBudget,
}
impl Work {
    fn charge(&mut self) -> Result<(), TemporalError> {
        self.left = self
            .left
            .checked_sub(1)
            .ok_or(TemporalError::ResourceExhausted(
                "coordinate filter comparison/scan limit",
            ))?;
        Ok(())
    }
}
fn compare(a: &Position, b: &Position) -> Domain<PositionComparison> {
    match compare_positions(a, b) {
        PositionComparison::Incomparable(reason) => Err(Refusal {
            domain: "Incomparable",
            reason,
        }),
        PositionComparison::Unresolved(reason) => Err(unresolved(reason)),
        r => Ok(r),
    }
}
fn exact_ref_json(r: &ExactRecordRef) -> JsonValue {
    obj(vec![
        ("id", text(r.id.as_str())),
        (
            "version",
            JsonValue::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme: r.version.get().to_string(),
            }),
        ),
        ("digest", text(&r.digest.to_hex())),
    ])
}
fn role_name(r: TemporalRole) -> &'static str {
    match r {
        TemporalRole::AttributedKnowledge => "AttributedKnowledge",
        TemporalRole::SourcePublication => "SourcePublication",
        TemporalRole::SystemTransaction => "SystemTransaction",
        TemporalRole::WorldOccurrence => "WorldOccurrence",
        TemporalRole::ActualInterval => "ActualInterval",
        _ => "Unsupported",
    }
}
fn proof(
    carrier: &TemporalClaimCarrier,
    index: usize,
    query: &JsonValue,
    kind: &str,
    outcome: &str,
    comparisons: &[&str],
    work: &mut Work,
) -> Result<JsonValue, TemporalError> {
    let source = field(carrier.original_packet(), "roles")
        .as_array()
        .and_then(|v| v.get(index))
        .expect("immutable decoded carrier index");
    let limits = crate::JsonLimits {
        max_bytes: work.budget.max_expression_bytes,
        max_depth: 64,
        max_visits: work.budget.max_members,
        max_integer_digits: work.budget.max_expression_bytes,
    };
    let source_bytes = crate::emit_value_preserved_json(source, limits)
        .map_err(|_| TemporalError::ResourceExhausted("coordinate filter explanation limit"))?
        .len();
    let query_bytes = crate::emit_value_preserved_json(query, limits)
        .map_err(|_| TemporalError::ResourceExhausted("coordinate filter explanation limit"))?
        .len();
    let charge = source_bytes
        .checked_add(query_bytes)
        .and_then(|n| n.checked_add(512))
        .ok_or(TemporalError::ResourceExhausted(
            "coordinate filter explanation limit",
        ))?;
    work.output_left =
        work.output_left
            .checked_sub(charge)
            .ok_or(TemporalError::ResourceExhausted(
                "coordinate filter explanation limit",
            ))?;
    Ok(obj(vec![
        ("kind", text(kind)),
        ("assertion_index", number(index as i64)),
        ("source_binding", source.clone()),
        ("query_predicate", query.clone()),
        ("outcome", text(outcome)),
        (
            "comparisons",
            JsonValue::Array(comparisons.iter().map(|c| text(c)).collect()),
        ),
    ]))
}
fn comparison_name(c: &PositionComparison) -> &'static str {
    match c {
        PositionComparison::Less => "Less",
        PositionComparison::Equal => "Equal",
        PositionComparison::Greater => "Greater",
        _ => "Refused",
    }
}
fn evaluate_role(
    carrier: &TemporalClaimCarrier,
    p: &RolePredicate,
    query: &JsonValue,
    work: &mut Work,
) -> Result<Domain<(bool, JsonValue)>, TemporalError> {
    let mut selected = None;
    for (index, binding) in carrier.roles().unwrap_or(&[]).iter().enumerate() {
        work.charge()?;
        if binding.role != p.role {
            continue;
        }
        let parsed = || -> Domain<(ExactRecordRef, Option<ExactRecordRef>, Position)> {
            let a = &binding.assertion;
            strict(a, &["profile_ref", "about_ref", "knower_ref", "position"])?;
            if word(a, "profile_ref")? != "tos_role_coordinate_assertion_v1" {
                return Err(unsupported("UnsupportedRoleAssertionProfile"));
            }
            Ok((
                exact_ref(required(a, "about_ref")?)?,
                knower(a, p.role)?,
                coordinate(required(a, "position")?)?,
            ))
        };
        let (about, knower, position) = match parsed() {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        if about != p.about || knower != p.knower {
            continue;
        }
        if selected.is_some() {
            return Ok(Err(unresolved("MultipleRoleAssertions")));
        }
        selected = Some((index, position));
    }
    let Some((index, position)) = selected else {
        return Ok(Err(unresolved("MissingRoleAssertion")));
    };
    work.charge()?;
    let c = match compare(&position, &p.cutoff) {
        Ok(c) => c,
        Err(e) => return Ok(Err(e)),
    };
    let yes = matches!(c, PositionComparison::Less | PositionComparison::Equal);
    Ok(Ok((
        yes,
        proof(
            carrier,
            index,
            query,
            role_name(p.role),
            if yes { "Match" } else { "NoMatch" },
            &[comparison_name(&c)],
            work,
        )?,
    )))
}
struct WorldAssertion {
    about: ExactRecordRef,
    beginning: Position,
    end: Option<Position>,
    beginning_inclusive: bool,
    end_inclusive: bool,
}
fn world_assertion(a: &JsonValue, role: TemporalRole) -> Domain<WorldAssertion> {
    strict(a, &["profile_ref", "about_ref", "extent"])?;
    if word(a, "profile_ref")? != "tos_world_coordinate_extent_v1" {
        return Err(unsupported("UnsupportedWorldAssertionProfile"));
    }
    let about = exact_ref(required(a, "about_ref")?)?;
    let e = required(a, "extent")?;
    if role == TemporalRole::WorldOccurrence {
        strict(e, &["type", "position"])?;
        if word(e, "type")? != "Instant" {
            return Err(unsupported("WorldRoleExtentMismatch"));
        }
        Ok(WorldAssertion {
            about,
            beginning: coordinate(required(e, "position")?)?,
            end: None,
            beginning_inclusive: true,
            end_inclusive: true,
        })
    } else {
        strict(e, &["type", "beginning", "end"])?;
        if word(e, "type")? != "Interval" {
            return Err(unsupported("WorldRoleExtentMismatch"));
        }
        let boundary = |v: &JsonValue| -> Domain<(Position, bool)> {
            strict(v, &["position", "inclusivity"])?;
            let inclusive = match word(v, "inclusivity")? {
                "Inclusive" => true,
                "Exclusive" => false,
                _ => return Err(unsupported("ExplicitInclusivityRequired")),
            };
            Ok((coordinate(required(v, "position")?)?, inclusive))
        };
        let (beginning, beginning_inclusive) = boundary(required(e, "beginning")?)?;
        let (end, end_inclusive) = boundary(required(e, "end")?)?;
        Ok(WorldAssertion {
            about,
            beginning,
            end: Some(end),
            beginning_inclusive,
            end_inclusive,
        })
    }
}
fn evaluate_world(
    carrier: &TemporalClaimCarrier,
    p: &WorldPredicate,
    query: &JsonValue,
    work: &mut Work,
) -> Result<Domain<(bool, JsonValue)>, TemporalError> {
    if carrier.validity().is_some() {
        return Ok(Err(unresolved("PendingValidityReconciliation")));
    }
    let mut selected = None;
    for (index, binding) in carrier.roles().unwrap_or(&[]).iter().enumerate() {
        work.charge()?;
        if !matches!(
            binding.role,
            TemporalRole::WorldOccurrence | TemporalRole::ActualInterval
        ) {
            continue;
        }
        let a = match world_assertion(&binding.assertion, binding.role) {
            Ok(a) => a,
            Err(e) => return Ok(Err(e)),
        };
        // Validate a finite interval even when its explicit external record is
        // different; unsupported same-role assertions may not disappear.
        if let Some(end) = &a.end {
            work.charge()?;
            match compare(&a.beginning, end) {
                Ok(PositionComparison::Less) => {}
                Ok(_) => return Ok(Err(unresolved("NonProperWorldInterval"))),
                Err(e) => return Ok(Err(e)),
            }
        }
        if a.about != p.about {
            continue;
        }
        if selected.is_some() {
            return Ok(Err(unresolved("MultipleWorldAssertions")));
        }
        selected = Some((index, a));
    }
    let Some((index, a)) = selected else {
        return Ok(Err(unresolved("MissingWorldAssertion")));
    };
    work.charge()?;
    let left = match compare(&a.beginning, &p.at) {
        Ok(c) => c,
        Err(e) => return Ok(Err(e)),
    };
    let mut names = vec![comparison_name(&left)];
    let yes = if let Some(end) = &a.end {
        work.charge()?;
        let right = match compare(&p.at, end) {
            Ok(c) => c,
            Err(e) => return Ok(Err(e)),
        };
        names.push(comparison_name(&right));
        (matches!(left, PositionComparison::Less)
            || (a.beginning_inclusive && matches!(left, PositionComparison::Equal)))
            && (matches!(right, PositionComparison::Less)
                || (a.end_inclusive && matches!(right, PositionComparison::Equal)))
    } else {
        matches!(left, PositionComparison::Equal)
    };
    Ok(Ok((
        yes,
        proof(
            carrier,
            index,
            query,
            "WorldAt",
            if yes { "Match" } else { "NoMatch" },
            &names,
            work,
        )?,
    )))
}
/// Whole-query validation precedes any source result. Refused dominates NoMatch;
/// the provider must propagate it as a non-total page, without a success cursor.
/// SourceRevision uses its sha256-prefixed digest representation; nested exact
/// record refs deliberately use the carrier's barehex digest representation.
pub fn evaluate_claim_coordinate_filter(
    carrier: Option<&TemporalClaimCarrier>,
    filter: &JsonValue,
    claim_ref: &ExactRecordRef,
    source_revision: SourceRevision,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let parsed = parse_filter(filter, budget)?;
    if let Some(c) = carrier {
        bounded(c.original_packet(), budget)?;
    }
    if claim_ref.id.as_str().len() > budget.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted(
            "coordinate filter provenance limit",
        ));
    }
    let claim_wire = exact_ref_json(claim_ref);
    bounded(&claim_wire, budget)?;
    let limits = crate::JsonLimits {
        max_bytes: budget.max_expression_bytes,
        max_depth: 64,
        max_visits: budget.max_members,
        max_integer_digits: budget.max_expression_bytes,
    };
    let filter_bytes = crate::emit_value_preserved_json(filter, limits)
        .map_err(|_| TemporalError::ResourceExhausted("coordinate filter result limit"))?
        .len();
    let provenance_bytes = crate::emit_value_preserved_json(&claim_wire, limits)
        .map_err(|_| TemporalError::ResourceExhausted("coordinate filter result limit"))?
        .len();
    let fixed = filter_bytes
        .checked_add(provenance_bytes)
        .and_then(|n| n.checked_add(512))
        .ok_or(TemporalError::ResourceExhausted(
            "coordinate filter result limit",
        ))?;
    let mut work = Work {
        left: budget.max_pairings,
        output_left: budget.max_expression_bytes.checked_sub(fixed).ok_or(
            TemporalError::ResourceExhausted("coordinate filter result limit"),
        )?,
        budget,
    };
    let mut comparisons = Vec::new();
    let mut refusal = None;
    let mut all = true;
    if let Some(c) = carrier {
        for p in &parsed.roles {
            let query = &field(filter, "roles").as_array().expect("validated roles")[p.index];
            match evaluate_role(c, p, query, &mut work)? {
                Ok((yes, proof)) => {
                    all &= yes;
                    comparisons.push(proof);
                }
                Err(e) => {
                    if refusal.is_none() {
                        refusal = Some(e);
                    }
                }
            }
        }
        if let Some(p) = &parsed.world {
            match evaluate_world(c, p, field(filter, "world_valid_time"), &mut work)? {
                Ok((yes, proof)) => {
                    all &= yes;
                    comparisons.push(proof);
                }
                Err(e) => {
                    if refusal.is_none() {
                        refusal = Some(e);
                    }
                }
            }
        }
    } else {
        refusal = Some(unresolved("MissingTemporalCarrier"));
    }
    let mut result = obj(vec![
        (
            "schema_version",
            text("tos_claim_coordinate_filter_result_v1"),
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
        ("claim_ref", claim_wire),
        ("source_revision", text(&source_revision.0.to_prefixed())),
        ("comparisons", JsonValue::Array(comparisons)),
        ("preserved_filter", filter.clone()),
    ]);
    if let Some(e) = refusal {
        put(&mut result, "domain", text(e.domain));
        put(&mut result, "reason", text(e.reason));
    }
    bounded(&result, budget)?;
    Ok(result)
}
