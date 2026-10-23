//! Pure, source-preserving qualifier codec. No Claim writes, time resolution,
//! recurrence expansion, or authority/compatibility inference occurs here.
//!
//! This packet contains supplemental source assertions. Existing readers retain
//! their legacy historicalDate object authority. Decoding qualifiers cannot
//! examine that object or settle contradictions with it; no packet gets priority.
//! Role assertions/patterns remain opaque, never admitted knowledge time.
//!
//! Losslessness here means JsonValue equality, including retained numeric
//! lexemes, not original JSON byte/escape spelling. The existing writer's
//! SourceCommandInputV1/PythonCompact may normalize Float via f64 and -0 to 0.
//! Carry exact raw decimal/lexical evidence as strings or an explicit opaque
//! bytes carrier. This codec cannot promise spelling after a later Claim write.
use super::*;
use crate::{Digest256, RecordVersion, StableId};

pub const TEMPORAL_CLAIM_CARRIER_KEY: &str = "temporal_claim_carrier";
pub const TEMPORAL_CLAIM_CARRIER_SCHEMA: &str = "tos_temporal_claim_carrier_v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimCarrierError {
    Malformed(&'static str),
    Unsupported(&'static str),
    ResourceLimit,
}
type CarrierResult<T> = Result<T, ClaimCarrierError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CarrierAuthority {
    SupplementalSourceAssertions,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyObjectAssessment {
    NotExamined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarrierBasis {
    pub source: ExactRecordRef,
    pub source_locator: Option<String>,
    /// Exact raw wording/decimal spelling belongs in strings or opaque bytes;
    /// numeric metadata can be normalized by a later writer (see module docs).
    pub raw_evidence: JsonValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// Additive wire capability; adding `Interval` requires downstream exhaustive
/// Rust matches to be updated. No interval comparison capability is implied.
pub enum CarrierValidity {
    /// Preserved beginning and unknown end, with caller-supplied, unverified
    /// source basis. Calendar lexical text does not resolve calendar/precision.
    Interval {
        beginning: Position,
        end: Boundary,
        basis: CarrierBasis,
    },
    Unknown {
        basis: CarrierBasis,
    },
    RightCensored {
        beginning: Option<Position>,
        observed_through: Position,
        cutoff_basis: CarrierBasis,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarrierRoleBinding {
    pub role: TemporalRole,
    /// Preserved source assertion, not an admitted comparable temporal value.
    pub assertion: JsonValue,
    pub basis: CarrierBasis,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreservedRecurrence {
    pub profile_ref: String,
    /// Opaque original pattern, never expanded or interpreted by this codec.
    pub pattern: JsonValue,
    pub basis: CarrierBasis,
}
/// The immutable typed view and retained packet always describe the same input.
/// Revisions are supplied as a new explicit packet and decoded again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalClaimCarrier {
    relative_relations: Option<Vec<SourceRelationAssertion>>,
    validity: Option<CarrierValidity>,
    roles: Option<Vec<CarrierRoleBinding>>,
    recurrence: Option<PreservedRecurrence>,
    original_packet: JsonValue,
}
impl TemporalClaimCarrier {
    pub fn authority(&self) -> CarrierAuthority {
        CarrierAuthority::SupplementalSourceAssertions
    }
    pub fn legacy_object_assessment(&self) -> LegacyObjectAssessment {
        LegacyObjectAssessment::NotExamined
    }
    pub fn relative_relations(&self) -> Option<&[SourceRelationAssertion]> {
        self.relative_relations.as_deref()
    }
    pub fn validity(&self) -> Option<&CarrierValidity> {
        self.validity.as_ref()
    }
    pub fn roles(&self) -> Option<&[CarrierRoleBinding]> {
        self.roles.as_deref()
    }
    pub fn recurrence(&self) -> Option<&PreservedRecurrence> {
        self.recurrence.as_ref()
    }
    pub fn original_packet(&self) -> &JsonValue {
        &self.original_packet
    }
}
fn budget_check(value: &JsonValue, budget: TemporalBudget) -> CarrierResult<()> {
    // Existing bounded preserving emitter validates public JsonValue numeric
    // lexemes without converting/rounding them. No emitted bytes become source.
    crate::emit_value_preserved_json(
        value,
        crate::JsonLimits {
            max_bytes: budget.max_expression_bytes,
            max_depth: 64,
            max_visits: budget.max_members,
            max_integer_digits: budget.max_expression_bytes,
        },
    )
    .map(|_| ())
    .map_err(|e| {
        if e.code == crate::FoundationErrorCode::BudgetExceeded {
            ClaimCarrierError::ResourceLimit
        } else {
            ClaimCarrierError::Malformed("InvalidCarrierJson")
        }
    })
}
fn object(v: &JsonValue) -> CarrierResult<()> {
    let members = v
        .as_object()
        .ok_or(ClaimCarrierError::Malformed("ObjectRequired"))?;
    // All typed traversal follows budget admission. Duplicate member names
    // cannot pick an arbitrary schema/version or hide a required typed field.
    for (i, (key, _)) in members.iter().enumerate() {
        if members[..i].iter().any(|(other, _)| key == other) {
            return Err(ClaimCarrierError::Malformed("DuplicateCarrierMember"));
        }
    }
    Ok(())
}
fn source_only(v: &JsonValue) -> CarrierResult<()> {
    if ["resolved", "admitted"]
        .iter()
        .any(|k| v.object_get(k).is_some())
    {
        Err(ClaimCarrierError::Malformed(
            "CarrierAuthorityFlagNotAllowed",
        ))
    } else {
        Ok(())
    }
}
fn required<'a>(v: &'a JsonValue, key: &'static str) -> CarrierResult<&'a JsonValue> {
    v.object_get(key).ok_or(ClaimCarrierError::Malformed(key))
}
fn nonempty(v: &JsonValue) -> CarrierResult<&str> {
    v.as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or(ClaimCarrierError::Malformed("NonemptyStringRequired"))
}
fn text_field<'a>(v: &'a JsonValue, key: &'static str) -> CarrierResult<&'a str> {
    nonempty(required(v, key)?)
}
fn optional_text(v: Option<&JsonValue>) -> CarrierResult<Option<String>> {
    match v {
        None | Some(JsonValue::Null) => Ok(None),
        Some(v) => Ok(Some(nonempty(v)?.into())),
    }
}
fn id(v: &JsonValue) -> CarrierResult<StableId> {
    StableId::parse(nonempty(v)?).map_err(|_| ClaimCarrierError::Malformed("InvalidStableId"))
}
fn exact_ref(v: &JsonValue) -> CarrierResult<ExactRecordRef> {
    object(v)?;
    source_only(v)?;
    let version = RecordVersion::new(
        required(v, "version")?
            .as_u64()
            .ok_or(ClaimCarrierError::Malformed("InvalidRecordVersion"))?,
    )
    .map_err(|_| ClaimCarrierError::Malformed("InvalidRecordVersion"))?;
    Ok(ExactRecordRef {
        id: id(required(v, "id")?)?,
        version,
        digest: Digest256::from_hex(text_field(v, "digest")?)
            .map_err(|_| ClaimCarrierError::Malformed("InvalidRecordDigest"))?,
    })
}
fn optional_ref(v: Option<&JsonValue>) -> CarrierResult<Option<ExactRecordRef>> {
    match v {
        None | Some(JsonValue::Null) => Ok(None),
        Some(v) => exact_ref(v).map(Some),
    }
}
fn basis(v: &JsonValue) -> CarrierResult<CarrierBasis> {
    object(v)?;
    source_only(v)?;
    Ok(CarrierBasis {
        source: exact_ref(required(v, "source")?)?,
        source_locator: optional_text(Some(required(v, "source_locator")?))?,
        raw_evidence: required(v, "raw_evidence")?.clone(),
    })
}
fn trs(v: &JsonValue) -> CarrierResult<TrsRef> {
    object(v)?;
    source_only(v)?;
    Ok(TrsRef {
        id: text_field(v, "id")?.into(),
        version: text_field(v, "version")?.into(),
    })
}
fn resolution(v: &JsonValue) -> CarrierResult<Resolution> {
    if v.as_object().is_some() {
        object(v)?;
        source_only(v)?;
    }
    Ok(match v.as_str() {
        Some("year") => Resolution::Year,
        Some("season") => Resolution::Season,
        Some("month") => Resolution::Month,
        Some("day") => Resolution::Day,
        Some("hour") => Resolution::Hour,
        Some("minute") => Resolution::Minute,
        Some("second") => Resolution::Second,
        Some("Unknown") => Resolution::Unknown,
        _ if string(v, "type") == Some("Fraction") => Resolution::Fraction(
            required(v, "digits")?
                .as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or(ClaimCarrierError::Malformed("InvalidFractionDigits"))?,
        ),
        _ if string(v, "type") == Some("NamedUnit") => {
            Resolution::NamedUnit(text_field(v, "unit")?.into())
        }
        _ => return Err(ClaimCarrierError::Unsupported("UnsupportedResolution")),
    })
}
fn position(v: &JsonValue) -> CarrierResult<Position> {
    object(v)?;
    source_only(v)?;
    // Do not expose an exact typed position while hiding a source precision
    // qualifier in the retained packet. Unsupported forms remain in caller input.
    if v.object_get("uncertainty").is_some()
        || v.object_get("qualifier")
            .is_some_and(|q| q.as_str() != Some("None"))
    {
        return Err(ClaimCarrierError::Unsupported(
            "UnsupportedPositionPrecision",
        ));
    }
    match text_field(v, "type")? {
        "CalendarPosition" => Ok(Position::Calendar {
            lexical: text_field(v, "lexical")?.into(),
            resolution: resolution(required(v, "resolution")?)?,
            trs: match required(v, "trs")? {
                JsonValue::Null => None,
                t => Some(trs(t)?),
            },
            era: optional_text(v.object_get("era"))?,
        }),
        "CoordinatePosition" => {
            if v.object_get("resolution")
                .is_some_and(|r| r.as_str() != Some("exact"))
            {
                return Err(ClaimCarrierError::Unsupported(
                    "UnsupportedPositionPrecision",
                ));
            }
            Ok(Position::Coordinate {
                value: integer(required(v, "value")?)
                    .ok_or(ClaimCarrierError::Unsupported("ExactI64CoordinateRequired"))?,
                unit: text_field(v, "unit")?.into(),
                origin: text_field(v, "origin")?.into(),
                trs: trs(required(v, "trs")?)?,
            })
        }
        "OrdinalPosition" => {
            if v.object_get("resolution")
                .is_some_and(|r| r.as_str() != Some("exact"))
            {
                return Err(ClaimCarrierError::Unsupported(
                    "UnsupportedPositionPrecision",
                ));
            }
            Ok(Position::Ordinal {
                node: text_field(v, "node")?.into(),
                rank: match required(v, "rank")? {
                    JsonValue::Null => None,
                    n => Some(
                        integer(n).ok_or(ClaimCarrierError::Unsupported("ExactI64RankRequired"))?,
                    ),
                },
                trs: trs(required(v, "trs")?)?,
                ordering: match text_field(v, "ordering")? {
                    "Total" => OrderingCapability::Total,
                    "Partial" => OrderingCapability::Partial,
                    "OrdinalOnly" => OrderingCapability::OrdinalOnly,
                    "Unknown" => OrderingCapability::Unknown,
                    _ => {
                        return Err(ClaimCarrierError::Unsupported(
                            "UnsupportedOrderingCapability",
                        ));
                    }
                },
            })
        }
        _ => Err(ClaimCarrierError::Unsupported("UnsupportedPositionKind")),
    }
}
fn unknown_interval_end(v: &JsonValue) -> CarrierResult<Boundary> {
    object(v)?;
    source_only(v)?;
    if text_field(v, "type")? != "Unknown" {
        return Err(ClaimCarrierError::Unsupported("UnsupportedIntervalEndKind"));
    }
    if [
        "position",
        "inclusivity",
        "precision",
        "explicit_basis_ref",
        "source_profile",
        "lexical_marker",
        "observed_through",
        "cutoff_basis",
    ]
    .iter()
    .any(|k| v.object_get(k).is_some())
    {
        return Err(ClaimCarrierError::Malformed(
            "ConflictingUnknownIntervalEnd",
        ));
    }
    Ok(Boundary::Unknown)
}
fn validity(v: &JsonValue) -> CarrierResult<CarrierValidity> {
    object(v)?;
    source_only(v)?;
    match text_field(v, "type")? {
        "Interval" => {
            if ["observed_through", "cutoff_basis"]
                .iter()
                .any(|k| v.object_get(k).is_some())
            {
                return Err(ClaimCarrierError::Malformed("ConflictingIntervalValidity"));
            }
            Ok(CarrierValidity::Interval {
                beginning: position(required(v, "beginning")?)?,
                end: unknown_interval_end(required(v, "end")?)?,
                basis: basis(required(v, "basis")?)?,
            })
        }
        "Unknown" => {
            if ["beginning", "observed_through", "cutoff_basis"]
                .iter()
                .any(|k| v.object_get(k).is_some())
            {
                return Err(ClaimCarrierError::Malformed("ConflictingUnknownValidity"));
            }
            Ok(CarrierValidity::Unknown {
                basis: basis(required(v, "basis")?)?,
            })
        }
        "RightCensored" => Ok(CarrierValidity::RightCensored {
            beginning: match v.object_get("beginning") {
                None | Some(JsonValue::Null) => None,
                Some(p) => Some(position(p)?),
            },
            observed_through: position(required(v, "observed_through")?)?,
            cutoff_basis: basis(required(v, "cutoff_basis")?)?,
        }),
        _ => Err(ClaimCarrierError::Unsupported("UnsupportedValidityKind")),
    }
}
fn role(v: &JsonValue) -> CarrierResult<TemporalRole> {
    Ok(match nonempty(v)? {
        "WorldOccurrence" => TemporalRole::WorldOccurrence,
        "ActualInterval" => TemporalRole::ActualInterval,
        "Beginning" => TemporalRole::Beginning,
        "End" => TemporalRole::End,
        "SourceCreation" => TemporalRole::SourceCreation,
        "SourcePublication" => TemporalRole::SourcePublication,
        "AttributedKnowledge" => TemporalRole::AttributedKnowledge,
        "SystemTransaction" => TemporalRole::SystemTransaction,
        _ => return Err(ClaimCarrierError::Unsupported("UnsupportedTemporalRole")),
    })
}
fn relation(v: &JsonValue) -> CarrierResult<SourceRelationAssertion> {
    object(v)?;
    source_only(v)?;
    let relation = match text_field(v, "relation")? {
        "before" => Relation::Before,
        "meets" => Relation::Meets,
        "overlaps" => Relation::Overlaps,
        "starts" => Relation::Starts,
        "during" => Relation::During,
        "finishes" => Relation::Finishes,
        "equals" => Relation::Equals,
        "after" => Relation::After,
        "metBy" => Relation::MetBy,
        "overlappedBy" => Relation::OverlappedBy,
        "startedBy" => Relation::StartedBy,
        "contains" => Relation::Contains,
        "finishedBy" => Relation::FinishedBy,
        _ => {
            return Err(ClaimCarrierError::Unsupported(
                "UnsupportedRelativeRelation",
            ));
        }
    };
    Ok(SourceRelationAssertion {
        subject_a: id(required(v, "subject_a")?)?,
        subject_b: id(required(v, "subject_b")?)?,
        relation,
        source: exact_ref(required(v, "source")?)?,
        asserted_by: match v.object_get("asserted_by") {
            None | Some(JsonValue::Null) => None,
            Some(v) => Some(id(v)?),
        },
        wording_ref: optional_ref(v.object_get("wording_ref"))?,
        epistemic_status: text_field(v, "epistemic_status")?.into(),
    })
}
fn list<T>(v: &JsonValue, f: impl Fn(&JsonValue) -> CarrierResult<T>) -> CarrierResult<Vec<T>> {
    v.as_array()
        .ok_or(ClaimCarrierError::Malformed("ArrayRequired"))?
        .iter()
        .map(f)
        .collect()
}
/// None means absent key only. Unsupported/malformed packets stay untouched in
/// the borrowed input; they never become a partial or default typed carrier.
pub fn decode_claim_carrier(
    qualifiers: &JsonValue,
    budget: TemporalBudget,
) -> CarrierResult<Option<TemporalClaimCarrier>> {
    budget_check(qualifiers, budget)?;
    object(qualifiers)?;
    let Some(packet) = qualifiers.object_get(TEMPORAL_CLAIM_CARRIER_KEY) else {
        return Ok(None);
    };
    object(packet)?;
    source_only(packet)?;
    if text_field(packet, "schema_version")? != TEMPORAL_CLAIM_CARRIER_SCHEMA {
        return Err(ClaimCarrierError::Unsupported("UnsupportedSchemaVersion"));
    }
    if !["relative_relations", "validity", "roles", "recurrence"]
        .iter()
        .any(|k| packet.object_get(k).is_some())
    {
        return Err(ClaimCarrierError::Malformed("CarrierPayloadRequired"));
    }
    let relative_relations = packet
        .object_get("relative_relations")
        .map(|v| list(v, relation))
        .transpose()?;
    let validity = packet.object_get("validity").map(validity).transpose()?;
    let roles = packet
        .object_get("roles")
        .map(|v| {
            list(v, |v| {
                object(v)?;
                source_only(v)?;
                Ok(CarrierRoleBinding {
                    role: role(required(v, "role")?)?,
                    assertion: required(v, "assertion")?.clone(),
                    basis: basis(required(v, "basis")?)?,
                })
            })
        })
        .transpose()?;
    let recurrence = packet
        .object_get("recurrence")
        .map(|v| -> CarrierResult<PreservedRecurrence> {
            object(v)?;
            source_only(v)?;
            Ok(PreservedRecurrence {
                profile_ref: text_field(v, "profile_ref")?.into(),
                pattern: required(v, "pattern")?.clone(),
                basis: basis(required(v, "basis")?)?,
            })
        })
        .transpose()?;
    Ok(Some(TemporalClaimCarrier {
        relative_relations,
        validity,
        roles,
        recurrence,
        original_packet: packet.clone(),
    }))
}
/// Returns a qualifier value for the existing Claim owner's patch path. This
/// function neither creates a revision nor reads/writes source files.
pub fn encode_claim_carrier(
    qualifiers: &JsonValue,
    carrier: &TemporalClaimCarrier,
    budget: TemporalBudget,
) -> CarrierResult<JsonValue> {
    budget_check(qualifiers, budget)?;
    object(qualifiers)?;
    budget_check(&carrier.original_packet, budget)?;
    // Each source is bounded before cloning, so temporary owned storage is at
    // most two admitted inputs; the merged output is checked independently.
    let mut result = qualifiers.clone();
    put(
        &mut result,
        TEMPORAL_CLAIM_CARRIER_KEY,
        carrier.original_packet.clone(),
    );
    budget_check(&result, budget)?;
    Ok(result)
}
