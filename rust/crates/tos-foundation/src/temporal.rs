//! Source-preserving temporal values and bounded, explicit-axis operations.
//!
//! Calendar buckets describe possible positions, never the duration of an
//! event. This module grants no source authority and performs no writes.
use crate::{ExactRecordRef, JsonNumber, JsonNumberKind, JsonString, JsonValue};

#[path = "temporal/calendar.rs"]
mod calendar;
#[path = "temporal/calendar_adapters.rs"]
pub mod calendar_adapters;
#[path = "temporal/claim_carrier.rs"]
pub mod claim_carrier;
#[path = "temporal/claim_filters.rs"]
pub mod claim_filters;
#[path = "temporal/codecs.rs"]
pub mod codecs;
#[path = "temporal/comparison.rs"]
mod comparison;
#[path = "temporal/crosswalk_records.rs"]
pub mod crosswalk_records;
#[path = "temporal/decimal_difference_wire.rs"]
pub mod decimal_difference_wire;
#[path = "temporal/difference.rs"]
pub mod difference;
#[path = "temporal/expression.rs"]
mod expression;
#[path = "temporal/integer_wire.rs"]
pub mod integer_wire;
#[path = "temporal/network.rs"]
pub mod network;
#[path = "temporal/stored_crosswalk.rs"]
pub mod stored_crosswalk;
#[path = "temporal/world_window.rs"]
pub mod world_window;
pub use claim_carrier::{
    CarrierAuthority, CarrierBasis, CarrierRoleBinding, CarrierValidity, ClaimCarrierError,
    LegacyObjectAssessment, PreservedRecurrence, TEMPORAL_CLAIM_CARRIER_KEY,
    TEMPORAL_CLAIM_CARRIER_SCHEMA, TemporalClaimCarrier, decode_claim_carrier,
    encode_claim_carrier,
};
pub use claim_filters::{evaluate_claim_coordinate_filter, validate_claim_coordinate_filter};
pub use comparison::{compare_positions, relate_exact_extents};
pub use expression::{capture_expression, serialize_expression};
pub use world_window::{evaluate_claim_world_window_filter, validate_claim_world_window_filter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrsRef {
    pub id: String,
    pub version: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderingCapability {
    Total,
    Partial,
    OrdinalOnly,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrsKind {
    Calendar,
    CalendarClock,
    EraNumbering,
    ElapsedTimeScale,
    CoordinateScale,
    OrdinalScale,
    Composite,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceSystem {
    pub reference: TrsRef,
    pub kind: TrsKind,
    pub ordering: OrderingCapability,
    pub rules_ref: Option<String>,
    pub source_ref: Option<ExactRecordRef>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resolution {
    Year,
    Season,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Fraction(u8),
    NamedUnit(String),
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Inclusivity {
    Inclusive,
    Exclusive,
    Unspecified,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qualifier {
    None,
    Uncertain,
    Approximate,
    UncertainAndApproximate,
    Unknown,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrecisionAssessment {
    pub target: String,
    pub assessment: String,
    pub method_ref: Option<String>,
    pub source_ref: Option<ExactRecordRef>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Position {
    Calendar {
        lexical: String,
        resolution: Resolution,
        trs: Option<TrsRef>,
        era: Option<String>,
    },
    Coordinate {
        value: i64,
        unit: String,
        origin: String,
        trs: TrsRef,
    },
    Ordinal {
        node: String,
        rank: Option<i64>,
        trs: TrsRef,
        ordering: OrderingCapability,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Boundary {
    Known {
        position: Position,
        inclusivity: Inclusivity,
        precision: Option<PrecisionAssessment>,
    },
    NoBoundDeclared {
        source_profile: String,
        lexical_marker: String,
    },
    Unbounded {
        explicit_basis_ref: ExactRecordRef,
    },
    Unknown,
    Unspecified,
    NotApplicable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidityAssertion {
    Bounded {
        beginning: Boundary,
        end: Boundary,
    },
    RightCensored {
        beginning: Option<Position>,
        observed_through: Position,
        cutoff_basis_ref: ExactRecordRef,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityExtent {
    Instant(Option<Position>),
    Interval { beginning: Boundary, end: Boundary },
    Unknown,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Possibility {
    Position {
        position: Position,
        qualifier: Qualifier,
    },
    Bounded {
        lower: Boundary,
        upper: Boundary,
    },
    ExactlyOneOf(Vec<Possibility>),
    AllOf(Vec<Possibility>),
    Unresolved {
        reason: String,
        expression_ref: Option<ExactRecordRef>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Duration {
    Elapsed {
        magnitude: String,
        unit: String,
        scale: TrsRef,
    },
    Calendar {
        magnitude: String,
        unit: String,
        calendar: TrsRef,
        arithmetic_policy_ref: Option<String>,
    },
}
/// Temporal roles are independently sourced, never inferred from ingest time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalRole {
    WorldOccurrence,
    ActualInterval,
    Beginning,
    End,
    SourceCreation,
    SourcePublication,
    AttributedKnowledge,
    SystemTransaction,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatingAssertion {
    pub revision: ExactRecordRef,
    pub subject: crate::StableId,
    pub role: TemporalRole,
    pub source: ExactRecordRef,
    pub source_locator: Option<String>,
    pub asserted_by: Option<crate::StableId>,
    pub normalized_by: Option<crate::StableId>,
    pub dating_method_ref: Option<ExactRecordRef>,
    pub expression: TemporalExpression,
    pub possibility: Possibility,
    pub qualifier: Qualifier,
    pub precision: Vec<PrecisionAssessment>,
    /// Opaque, explicitly supplied assessment; never calculated from syntax.
    pub confidence_assessment: Option<JsonValue>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedRelation {
    pub input_claim_revisions: Vec<ExactRecordRef>,
    pub outcome: RelationOutcome,
    pub rule_version: String,
    pub trs_versions: Vec<TrsRef>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRelationAssertion {
    pub subject_a: crate::StableId,
    pub subject_b: crate::StableId,
    pub relation: Relation,
    pub source: ExactRecordRef,
    pub asserted_by: Option<crate::StableId>,
    pub wording_ref: Option<ExactRecordRef>,
    pub epistemic_status: String,
}
/// Source assertion and computation are independently optional. Affirmation
/// does not supply an Entailed computation, nor does a computation edit source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalRelationAssertion {
    pub source_assertion: Option<SourceRelationAssertion>,
    pub derivation: Option<DerivedRelation>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseState {
    Complete,
    Partial,
    Ambiguous,
    Unparsed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionProfile {
    Edtf {
        level: u8,
        version: String,
    },
    Opaque {
        name: String,
        version: Option<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalExpression {
    pub raw: String,
    pub profile: ExpressionProfile,
    pub locale: Option<String>,
    pub parser_version: &'static str,
    pub state: ParseState,
    pub syntax: JsonValue,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemporalBudget {
    pub max_expression_bytes: usize,
    pub max_members: usize,
    pub max_pairings: usize,
}
impl Default for TemporalBudget {
    fn default() -> Self {
        Self {
            max_expression_bytes: 16_384,
            max_members: 512,
            max_pairings: 4096,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemporalError {
    MalformedRequest(&'static str),
    ResourceExhausted(&'static str),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Relation {
    Before,
    Meets,
    Overlaps,
    Starts,
    During,
    Finishes,
    Equals,
    After,
    MetBy,
    OverlappedBy,
    StartedBy,
    Contains,
    FinishedBy,
}
impl Relation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::Meets => "meets",
            Self::Overlaps => "overlaps",
            Self::Starts => "starts",
            Self::During => "during",
            Self::Finishes => "finishes",
            Self::Equals => "equals",
            Self::After => "after",
            Self::MetBy => "metBy",
            Self::OverlappedBy => "overlappedBy",
            Self::StartedBy => "startedBy",
            Self::Contains => "contains",
            Self::FinishedBy => "finishedBy",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PositionComparison {
    Less,
    Equal,
    Greater,
    Incomparable(&'static str),
    Unresolved(&'static str),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationOutcome {
    Entailed(Relation),
    Possible(Vec<Relation>),
    Incomparable(&'static str),
    Unresolved(&'static str),
    Contradicted(&'static str),
    Unsupported(&'static str),
}
/// Explicit common axis for exact finite positions. No calendar conversion is
/// implied by a numeric coordinate. Unit and origin are part of compatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Axis {
    pub trs: TrsRef,
    pub unit: String,
    pub origin: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactExtent {
    Instant {
        axis: Axis,
        position: i64,
    },
    Interval {
        axis: Axis,
        beginning: i64,
        end: i64,
        beginning_inclusivity: Inclusivity,
        end_inclusivity: Inclusivity,
    },
    Unknown,
}

pub(super) fn obj(values: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
pub(super) fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
pub(super) fn number(n: i64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
pub(super) fn field<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
pub(super) fn string<'a>(v: &'a JsonValue, k: &str) -> Option<&'a str> {
    field(v, k).as_str()
}
pub(super) fn integer(v: &JsonValue) -> Option<i64> {
    if let JsonValue::Number(n) = v {
        if n.kind == JsonNumberKind::Int {
            return n.lexeme.parse().ok();
        }
    }
    None
}
pub(super) fn status(s: &str) -> JsonValue {
    obj(vec![("status", text(s))])
}
pub(super) fn reason(s: &str, r: &str) -> JsonValue {
    obj(vec![("status", text(s)), ("reason", text(r))])
}
pub(super) fn put(v: &mut JsonValue, k: &str, value: JsonValue) {
    if let JsonValue::Object(v) = v {
        if let Some((_, old)) = v.iter_mut().find(|(key, _)| key.as_str() == Some(k)) {
            *old = value;
        } else {
            v.push((JsonString::from_utf8(k), value));
        }
    }
}
pub(super) fn relation_json(r: RelationOutcome) -> JsonValue {
    match r {
        RelationOutcome::Entailed(r) => obj(vec![
            ("status", text("Entailed")),
            ("relation", text(r.as_str())),
        ]),
        RelationOutcome::Possible(rs) => obj(vec![
            ("status", text("Possible")),
            (
                "relations",
                JsonValue::Array(rs.iter().map(|r| text(r.as_str())).collect()),
            ),
        ]),
        RelationOutcome::Incomparable(r) => reason("Incomparable", r),
        RelationOutcome::Unresolved(r) => reason("Unresolved", r),
        RelationOutcome::Contradicted(r) => reason("ContradictoryBounds", r),
        RelationOutcome::Unsupported(r) => reason("Unsupported", r),
    }
}

fn transport_limit() -> TemporalError {
    TemporalError::ResourceExhausted("temporal input/context byte/member/depth limit")
}

/// Cheap borrowed lower bound before the preserved writer examines keys or
/// number lexemes. Exact escaped bytes are admitted separately below.
fn preflight_transport(
    value: &JsonValue,
    depth: usize,
    members: &mut usize,
    bytes: &mut usize,
) -> Result<(), TemporalError> {
    if depth > 64 {
        return Err(transport_limit());
    }
    *members = members.checked_sub(1).ok_or_else(transport_limit)?;
    let minimum = match value {
        JsonValue::String(s) => s.units().len().checked_add(1).ok_or_else(transport_limit)?,
        JsonValue::Number(n) => n.lexeme.len(),
        _ => 1,
    };
    *bytes = bytes.checked_sub(minimum).ok_or_else(transport_limit)?;
    match value {
        JsonValue::Array(items) => {
            if items.len() > *members {
                return Err(transport_limit());
            }
            for item in items {
                preflight_transport(item, depth + 1, members, bytes)?;
            }
        }
        JsonValue::Object(entries) => {
            if entries.len() > *members {
                return Err(transport_limit());
            }
            for (key, item) in entries {
                *bytes = bytes
                    .checked_sub(key.units().len())
                    .ok_or_else(transport_limit)?;
                preflight_transport(item, depth + 1, members, bytes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn admit_transport(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<(), TemporalError> {
    let mut members = budget.max_members;
    let mut lower_bytes = budget.max_expression_bytes;
    preflight_transport(input, 0, &mut members, &mut lower_bytes)?;
    preflight_transport(context, 0, &mut members, &mut lower_bytes)?;

    // Same aggregate allowance for BOTH roots. Preserved emission retains the
    // native numeric/Unicode domain, without imposing canonical/wire safety.
    let mut bytes = budget.max_expression_bytes;
    for value in [input, context] {
        if bytes == 0 {
            return Err(transport_limit());
        }
        let raw = crate::emit_value_preserved_json(
            value,
            crate::JsonLimits {
                max_bytes: bytes,
                max_depth: 64,
                max_visits: budget.max_members,
                max_integer_digits: budget.max_expression_bytes,
            },
        )
        .map_err(|error| {
            if error.code == crate::FoundationErrorCode::BudgetExceeded {
                transport_limit()
            } else {
                TemporalError::MalformedRequest("invalid temporal input/context JSON")
            }
        })?;
        bytes = bytes.checked_sub(raw.len()).ok_or_else(transport_limit)?;
        // Drop the bounded temporary before admitting the next root.
    }
    Ok(())
}

/// Bounded transport adapter. The complete input plus context share the byte
/// (compact preserved JSON) and Value-node allowances, with depth at most 64.
/// This is input admission, not an output-size, retained-capacity or RSS cap.
/// It accepts operation/input/context only, never case IDs or expected answers.
/// Unknown operations disclose lack of support after the same admission.
pub fn execute_temporal_operation(
    operation: &str,
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    if input.as_object().is_none() || context.as_object().is_none() {
        return Err(TemporalError::MalformedRequest(
            "input and context must be objects",
        ));
    }
    admit_transport(input, context, budget)?;
    let mut result = match operation {
        "resolve_temporal_query_context" => {
            crate::query_context_wire::resolve_context_json_admitted(input, context, budget)?
        }
        "create_chronology_version" => {
            crate::query_context_wire::create_chronology_json_admitted(input, context, budget)?
        }
        "capture_expression" => {
            if string(input, "xml_fragment").is_some() {
                codecs::capture_tei_admitted(input, context, budget)?
            } else {
                let raw =
                    string(input, "raw").ok_or(TemporalError::MalformedRequest("raw required"))?;
                let level = field(input, "declared_level")
                    .as_u64()
                    .ok_or(TemporalError::MalformedRequest("declared_level required"))?;
                if level > u8::MAX as u64 {
                    return Err(TemporalError::MalformedRequest("invalid level"));
                }
                if raw.len() > budget.max_expression_bytes {
                    return Err(TemporalError::ResourceExhausted("expression byte limit"));
                }
                let e = capture_expression(
                    raw,
                    ExpressionProfile::Edtf {
                        level: level as u8,
                        version: "2019-02-04".into(),
                    },
                    None,
                    budget,
                );
                let s = match e.state {
                    ParseState::Complete => "Complete",
                    ParseState::Partial => "Partial",
                    ParseState::Ambiguous => "Ambiguous",
                    ParseState::Unparsed => "Unparsed",
                };
                obj(vec![
                    ("status", text(s)),
                    ("value", e.syntax),
                    ("preserved_raw", text(&e.raw)),
                    (
                        "diagnostics",
                        JsonValue::Array(e.diagnostics.iter().map(|s| text(s)).collect()),
                    ),
                ])
            }
        }
        "project_to_postgres_range" => {
            calendar_adapters::project_postgres_range_admitted(input, context, budget)?
        }
        "difference" => difference::execute_difference_admitted(input, context, budget)?,
        "prepare_crosswalk_version" => {
            crosswalk_records::prepare_crosswalk_version_admitted(input, context, budget)?
        }
        "convert_crosswalk_record" => {
            crosswalk_records::convert_crosswalk_record_admitted(input, context, budget)?
        }
        "add_crosswalk_version" => reason("Unsupported", "PendingOwnerBinding"),
        "add_duration" => calendar_adapters::add_duration_admitted(input, context, budget)?,
        "convert" => calendar_adapters::convert_admitted(input, context, budget)?,
        "resolve_position"
            if !field(input, "local_date_time").is_null()
                || !field(input, "zone_ref").is_null() =>
        {
            calendar_adapters::resolve_position_admitted(input, context, budget)?
        }
        "resolve_position" => calendar::resolve(input, context),
        "derive_extent" if field(input, "position").object_get("season_code").is_some() => {
            calendar_adapters::derive_named_extent_admitted(input, context, budget)?
        }
        "derive_extent" => calendar::derive(input, context),
        "compare_positions" => comparison::compare_json(input),
        "relate_extents" => comparison::relate_json(input, context, budget),
        "relate_claims" => comparison::relate_claims_json(input, context, budget),
        "check_network_consistency" => network::execute_network(input, context)?,
        _ => reason("Unsupported", "OperationNotImplemented"),
    };
    if result.object_get("rule_version").is_none() {
        put(&mut result, "rule_version", text("tos-temporal-core-v1"));
    }
    put(
        &mut result,
        "envelope_version",
        text("tos-temporal-operation-v1"),
    );
    Ok(result)
}

/// Opt-in JSON/client boundary. Native temporal APIs retain their own numeric
/// domain; this envelope refuses unsafe JSON numbers before executing them.
pub fn execute_temporal_integer_wire(
    request: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    use integer_wire::{
        IntegerWireSafety, check_integer_wire_value, encode_integer_wire_response,
        parse_integer_wire_request, refuse_integer_wire_value, validate_elapsed_magnitude_string,
    };
    let wire = parse_integer_wire_request(request, budget)?;
    if wire.integer_encoding == "CanonicalDecimalStringV1" && wire.operation == "difference" {
        return decimal_difference_wire::execute_decimal_difference_wire(request, budget);
    }
    if wire.integer_encoding != "SafeJsonNumberV1" {
        return refuse_integer_wire_value(
            wire.operation,
            "IntegerEncodingNotImplemented",
            request,
            budget,
        );
    }
    if let IntegerWireSafety::Unsupported(reason) = check_integer_wire_value(request, budget)? {
        return refuse_integer_wire_value(wire.operation, reason, request, budget);
    }
    if !wire
        .payload
        .as_object()
        .is_some_and(|fields| fields.len() == 2)
        || wire.payload.object_get("input").is_none()
        || wire.payload.object_get("context").is_none()
    {
        return Err(TemporalError::MalformedRequest(
            "integer wire payload requires input and context",
        ));
    }
    let value = execute_temporal_operation(
        wire.operation,
        field(wire.payload, "input"),
        field(wire.payload, "context"),
        budget,
    )?;
    // This field is an existing typed decimal-string contract, not a generic
    // instruction to reinterpret arbitrary source strings as numbers.
    if wire.operation == "difference" {
        let duration = field(&value, "duration");
        if string(duration, "kind") == Some("Elapsed") {
            if let IntegerWireSafety::Unsupported(reason) =
                validate_elapsed_magnitude_string(field(duration, "magnitude"))
            {
                return refuse_integer_wire_value(wire.operation, reason, &value, budget);
            }
        }
    }
    encode_integer_wire_response(wire.operation, &value, budget)
}
