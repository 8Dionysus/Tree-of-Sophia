//! Offline index projection of an explicit source day under an existing owner
//! read hold. No model, SQL execution, source write or policy admission.
use crate::source_command::{self as cmd, SourceCommandError};
use crate::temporal_chronology::LoadedChronology;
use std::{sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{
    CanonicalProfile, Digest256, ExactRecordRef, FoundationError, JsonLimits, JsonString,
    JsonValue, OwnedState, RelativePath, canonical_bytes_v1_with_state_budget_and_visits,
    temporal::{TemporalBudget, TemporalError, calendar_adapters::project_postgres_range},
};

#[derive(Debug)]
pub enum OwnerDayRangeError {
    Owner(SourceCommandError),
    Json(FoundationError),
    Temporal(TemporalError),
    Unresolved(&'static str),
    Unsupported(&'static str),
    CorruptBinding(&'static str),
    NotMember,
    AmbiguousMember,
    BudgetExceeded,
}
impl From<SourceCommandError> for OwnerDayRangeError {
    fn from(value: SourceCommandError) -> Self {
        Self::Owner(value)
    }
}
impl From<FoundationError> for OwnerDayRangeError {
    fn from(value: FoundationError) -> Self {
        Self::Json(value)
    }
}
impl From<TemporalError> for OwnerDayRangeError {
    fn from(value: TemporalError) -> Self {
        Self::Temporal(value)
    }
}
type Result<T> = std::result::Result<T, OwnerDayRangeError>;

#[derive(Clone, Copy, Debug)]
pub struct OwnerDayRangeBudget {
    /// Aggregate JSON transport limits for the generated projection input/context.
    pub temporal: TemporalBudget,
    /// Maximum source chronology members, independent of JSON value-node count.
    pub max_chronology_members: usize,
    /// Logical adapter residency, not allocator/process RSS admission.
    pub max_extra_owned_bytes: usize,
    pub max_extra_json_visits: usize,
}

/// Borrowed only inside the held callback. Recheck for each actual flush.
pub struct OwnerDayRangeView<'a> {
    body: &'a [u8],
    check: &'a mut dyn FnMut() -> Result<()>,
}
impl OwnerDayRangeView<'_> {
    pub fn recheck(&mut self) -> Result<()> {
        (self.check)()
    }
    pub fn with_current_bytes<T>(&mut self, consume: impl FnOnce(&[u8]) -> Result<T>) -> Result<T> {
        self.recheck()?;
        let result = consume(self.body);
        self.recheck()?;
        result
    }
}

fn corrupt(reason: &'static str) -> OwnerDayRangeError {
    OwnerDayRangeError::CorruptBinding(reason)
}
fn field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    value
        .object_get(key)
        .ok_or(OwnerDayRangeError::Unresolved("RequiredSourceFieldAbsent"))
}
fn text<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or(OwnerDayRangeError::Unsupported("SourceFieldNotText"))
}
fn exact(value: &JsonValue, keys: &[&str]) -> Result<()> {
    if !value.as_object().is_some_and(|fields| {
        fields.len() == keys.len() && keys.iter().all(|key| value.object_get(key).is_some())
    }) {
        return Err(OwnerDayRangeError::Unsupported(
            "UnsupportedProjectionFields",
        ));
    }
    Ok(())
}
fn same_reference(value: &JsonValue, expected: &ExactRecordRef) -> Result<bool> {
    exact(value, &["id", "version", "digest"])?;
    Ok(text(value, "id")? == expected.id.as_str()
        && field(value, "version")?.as_u64() == Some(expected.version.get())
        && Digest256::from_prefixed(text(value, "digest")?)? == expected.digest)
}
fn full_day(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

struct Work {
    bytes: usize,
    visits: usize,
    limits: JsonLimits,
}
impl Work {
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or(OwnerDayRangeError::BudgetExceeded)?;
        Ok(())
    }
    fn charge_visits(&mut self, visits: usize) -> Result<()> {
        self.visits = self
            .visits
            .checked_sub(visits)
            .ok_or(OwnerDayRangeError::BudgetExceeded)?;
        Ok(())
    }
    fn visit(&mut self, value: &JsonValue, depth: usize) -> Result<()> {
        if depth > self.limits.max_depth {
            return Err(OwnerDayRangeError::BudgetExceeded);
        }
        self.charge_visits(1)?;
        match value {
            JsonValue::Object(fields) => {
                for (_, value) in fields {
                    self.charge_visits(1)?;
                    self.visit(value, depth + 1)?;
                }
            }
            JsonValue::Array(values) => {
                for value in values {
                    self.visit(value, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn clone_value(&mut self, value: &JsonValue) -> Result<JsonValue> {
        self.visit(value, 0)?;
        self.charge(value.retained_state_bytes()?)?;
        Ok(value.clone())
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>> {
        self.charge(
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(OwnerDayRangeError::BudgetExceeded)?,
        )?;
        Ok(Vec::with_capacity(count))
    }
    fn string(&mut self, value: &str) -> Result<JsonValue> {
        self.charge(
            value
                .len()
                .checked_mul(5)
                .and_then(|n| n.checked_add(16 + std::mem::size_of::<JsonValue>()))
                .ok_or(OwnerDayRangeError::BudgetExceeded)?,
        )?;
        Ok(cmd::string(value))
    }
    fn number(&mut self, value: u64) -> Result<JsonValue> {
        self.charge(20 + std::mem::size_of::<JsonValue>())?;
        Ok(cmd::number(value))
    }
    fn object<const N: usize>(
        &mut self,
        build: impl FnOnce(&mut Self) -> Result<[(&'static str, JsonValue); N]>,
    ) -> Result<JsonValue> {
        let entries = build(self)?;
        let mut fields = self.vector::<(JsonString, JsonValue)>(N)?;
        for (key, value) in entries {
            self.charge(
                key.len()
                    .checked_mul(5)
                    .and_then(|n| n.checked_add(16))
                    .ok_or(OwnerDayRangeError::BudgetExceeded)?,
            )?;
            fields.push((JsonString::from_utf8(key), value));
        }
        Ok(JsonValue::Object(fields))
    }
    fn reference(&mut self, value: &ExactRecordRef) -> Result<JsonValue> {
        self.charge(256)?;
        let digest = value.digest.to_prefixed();
        self.object(|w| {
            Ok([
                ("id", w.string(value.id.as_str())?),
                ("version", w.number(value.version.get())?),
                ("digest", w.string(&digest)?),
            ])
        })
    }
    fn canonical(&mut self, value: &JsonValue, profile: CanonicalProfile) -> Result<Vec<u8>> {
        let mut limits = self.limits;
        limits.max_bytes = limits.max_bytes.min(self.bytes);
        limits.max_visits = limits.max_visits.min(self.visits);
        let (bytes, visits) =
            canonical_bytes_v1_with_state_budget_and_visits(value, profile, limits, self.bytes)?;
        self.charge_visits(visits)?;
        self.charge(bytes.capacity())?;
        Ok(bytes)
    }
    fn pure_reservation(&mut self, input: &JsonValue, context: &JsonValue) -> Result<usize> {
        // Current pure day projection retains <=2 copies of input/context
        // subtrees; reserve four plus fixed report/string/temporary headroom.
        // No refund on early refusal. This is not a generic operation forecast.
        let before = self.visits;
        self.visit(input, 0)?;
        self.visit(context, 0)?;
        let nodes = before - self.visits;
        self.charge_visits(
            nodes
                .checked_mul(4)
                .and_then(|n| n.checked_add(512))
                .ok_or(OwnerDayRangeError::BudgetExceeded)?,
        )?;
        let bytes = input
            .retained_state_bytes()?
            .checked_add(context.retained_state_bytes()?)
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_add(16_384))
            .ok_or(OwnerDayRangeError::BudgetExceeded)?;
        self.charge(bytes)?;
        Ok(bytes)
    }
}

fn source_day<'a>(
    record: &'a JsonValue,
    supplement: &JsonValue,
) -> Result<(&'a JsonValue, &'a JsonValue)> {
    exact(
        supplement,
        &["profile", "postgres_type", "bound_form", "calendar_rules"],
    )?;
    let rules = field(supplement, "calendar_rules")?;
    exact(rules, &["calendar", "version"])?;
    if text(supplement, "profile")? != "tos_owner_day_range_projection_v1"
        || text(supplement, "postgres_type")? != "daterange"
        || text(supplement, "bound_form")? != "[)"
        || text(rules, "calendar")? != "GregorianProleptic"
        || text(rules, "version")? != "proleptic-gregorian-civil-v1"
    {
        return Err(OwnerDayRangeError::Unsupported(
            "UnsupportedProjectionSupplement",
        ));
    }
    let object = field(record, "object")?;
    if text(object, "kind")? != "date-assertion" || text(object, "role")? != "historical-time" {
        return Err(OwnerDayRangeError::Unsupported(
            "FullDateHistoricalAssertionRequired",
        ));
    }
    if object.object_get("calendar").and_then(JsonValue::as_str) != Some("GregorianProleptic")
        || object
            .object_get("year_numbering")
            .and_then(JsonValue::as_str)
            != Some("astronomical")
        || object.object_get("certainty").and_then(JsonValue::as_str) != Some("exact")
    {
        return Err(OwnerDayRangeError::Unresolved(
            "ExplicitExactGregorianSourceRequired",
        ));
    }
    let start = text(object, "value")?;
    if !full_day(start) {
        return Err(OwnerDayRangeError::Unresolved(
            "ExplicitFullSourceDayRequired",
        ));
    }
    let extension = field(field(object, "extensions")?, "temporal_day_projection")?;
    exact(
        extension,
        &[
            "profile",
            "trs_ref",
            "calendar_rules_version",
            "end_exclusive",
            "precision_assessment",
        ],
    )?;
    if text(extension, "profile")? != "tos_source_full_day_projection_v1"
        || text(extension, "calendar_rules_version")? != text(rules, "version")?
    {
        return Err(OwnerDayRangeError::Unsupported("UnsupportedSourceDayRules"));
    }
    let trs = field(extension, "trs_ref")?;
    exact(trs, &["id", "version"])?;
    for key in ["id", "version"] {
        if text(trs, key)?.trim().is_empty() {
            return Err(OwnerDayRangeError::Unresolved("VersionedSourceTrsRequired"));
        }
    }
    if !full_day(text(extension, "end_exclusive")?) {
        return Err(OwnerDayRangeError::Unsupported(
            "ExplicitFullBucketEndRequired",
        ));
    }
    Ok((object, extension))
}

/// Select one already authorized owner member. No CMP authority or new public
/// transport is required/issued. Source-only facts remain source assertions.
pub fn with_loaded_chronology_day_range<'owner, T>(
    loaded: LoadedChronology<'owner>,
    requested_member: &ExactRecordRef,
    projection_supplement: &JsonValue,
    budget: OwnerDayRangeBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut OwnerDayRangeView<'view>) -> Result<T>,
) -> Result<T> {
    loaded.recheck(deadline, cancelled)?;
    if budget.max_chronology_members == 0
        || budget.temporal.max_members == 0
        || budget.temporal.max_pairings == 0
        || budget.temporal.max_expression_bytes == 0
        || budget.max_extra_owned_bytes == 0
        || budget.max_extra_json_visits == 0
    {
        return Err(OwnerDayRangeError::BudgetExceeded);
    }
    let mut work = Work {
        bytes: budget.max_extra_owned_bytes,
        visits: budget.max_extra_json_visits,
        limits: JsonLimits {
            max_bytes: budget.max_extra_owned_bytes.min(8_388_608),
            ..JsonLimits::default()
        },
    };
    // Fixed adapter/field-lookup temporaries, not another owner-evidence copy.
    work.charge(8192)?;
    work.visit(projection_supplement, 0)?;
    let bound = loaded.bound();
    let chronology = bound.chronology();
    let members = &chronology.member_claim_revisions;
    if members.len() > budget.max_chronology_members {
        return Err(OwnerDayRangeError::BudgetExceeded);
    }
    let mut index = None;
    for (i, member) in members.iter().enumerate() {
        work.charge_visits(1)?;
        if member == requested_member {
            if index.replace(i).is_some() {
                return Err(OwnerDayRangeError::AmbiguousMember);
            }
        }
    }
    let index = index.ok_or(OwnerDayRangeError::NotMember)?;
    let evidence = field(bound.evidence(), "members")?
        .as_array()
        .ok_or(corrupt("member evidence array"))?;
    if evidence.len() != members.len() {
        return Err(corrupt("member evidence count"));
    }
    let selected = &evidence[index];
    if !same_reference(field(selected, "reference")?, requested_member)? {
        return Err(corrupt("member evidence reference"));
    }
    let record = field(selected, "record")?;
    work.visit(record, 0)?;
    if text(record, "claim_id")? != requested_member.id.as_str()
        || field(record, "claim_version")?.as_u64() != Some(requested_member.version.get())
        || !matches!(
            text(record, "visibility")?,
            "public" | "public_metadata_only"
        )
    {
        return Err(corrupt("authorized public source Claim identity"));
    }
    let source_bytes = work.canonical(record, CanonicalProfile::SourceCommandInputV1)?;
    if Digest256::of_bytes(&source_bytes) != requested_member.digest {
        return Err(corrupt("source canonical digest"));
    }
    drop(source_bytes);
    let (object, extension) = source_day(record, projection_supplement)?;
    let frozen = field(selected, "frozen_source")?;
    let source_revision = text(frozen, "source_revision")?;
    if Digest256::from_prefixed(source_revision)? != chronology.selection.source_snapshot.revision.0
    {
        return Err(corrupt("frozen source revision"));
    }
    let locator = text(frozen, "source_ref")?;
    work.charge(locator.len())?;
    let path = RelativePath::parse(locator)?;
    if !path.as_str().starts_with("ToS/source-witnesses/") {
        return Err(corrupt("public source locator"));
    }
    Digest256::from_prefixed(text(frozen, "stream_sha256")?)?;
    let stream_bytes = field(frozen, "stream_bytes")?
        .as_u64()
        .ok_or(corrupt("stream size"))?;
    let line = field(frozen, "line")?
        .as_u64()
        .filter(|n| *n > 0 && *n <= stream_bytes)
        .ok_or(corrupt("stream line"))?;
    let fixity = work.object(|w| {
        Ok([
            ("source_revision", w.string(source_revision)?),
            ("source_ref", w.string(locator)?),
            ("stream_sha256", w.string(text(frozen, "stream_sha256")?)?),
            ("stream_bytes", w.number(stream_bytes)?),
            ("line", w.number(line)?),
        ])
    })?;
    // Closed public evidence projection. Never serialize owner_evidence(),
    // creation receipts, owner configuration, principals or private journals.
    let provenance = work.object(|w| {
        Ok([
            ("source_claim_ref", w.reference(requested_member)?),
            ("chronology_ref", w.reference(&chronology.reference)?),
            (
                "version_status",
                w.string(text(selected, "version_status")?)?,
            ),
            ("frozen_source", fixity),
        ])
    })?;
    let trs = field(extension, "trs_ref")?;
    let sidecar = work.object(|w| {
        Ok([
            ("raw_claim", w.clone_value(record)?),
            ("trs_ref", w.clone_value(trs)?),
            (
                "precision_assessment",
                w.clone_value(field(extension, "precision_assessment")?)?,
            ),
            ("provenance", provenance),
            ("source_claim_ref", w.reference(requested_member)?),
            (
                "packet_representation",
                w.string("FullParsedSourceClaimWithVerifiedStreamFixity")?,
            ),
        ])
    })?;
    let extent = work.object(|w| {
        Ok([
            ("type", w.string("CalendarDayBucket")?),
            ("calendar", w.string(text(object, "calendar")?)?),
            ("start", w.string(text(object, "value")?)?),
            (
                "end_exclusive",
                w.string(text(extension, "end_exclusive")?)?,
            ),
            ("unit", w.string("day")?),
            ("resolution", w.string("day")?),
            ("trs_ref", w.clone_value(trs)?),
        ])
    })?;
    let input = work.object(|w| {
        Ok([
            ("source_extent", extent),
            ("sidecar_claim_ref", w.reference(requested_member)?),
            ("sidecar", sidecar),
            ("postgres_type", w.string("daterange")?),
            ("bound_form", w.string("[)")?),
        ])
    })?;
    let reserved = work.pure_reservation(&input, projection_supplement)?;
    loaded.recheck(deadline, cancelled)?;
    let projected = project_postgres_range(&input, projection_supplement, budget.temporal)?;
    if projected.retained_state_bytes()? > reserved {
        return Err(OwnerDayRangeError::BudgetExceeded);
    }
    let output = work.object(|w| {
        Ok([
            (
                "schema_version",
                w.string("tos_owner_day_range_projection_result_v1")?,
            ),
            ("chronology_ref", w.reference(&chronology.reference)?),
            ("source_claim_ref", w.reference(requested_member)?),
            (
                "projection_scope",
                w.string("OwnerReadSourceDeclaredDayBucketIndex")?,
            ),
            (
                "source_extraction_profile",
                w.string("tos_source_full_day_projection_v1")?,
            ),
            (
                "projection_supplement",
                w.clone_value(projection_supplement)?,
            ),
            (
                "evidence_projection",
                w.string("AuthorizedSourceAndStreamFixityOnly")?,
            ),
            (
                "policy_scope",
                w.string("SourceAssertionNotConversionPolicyAdmission")?,
            ),
            ("sql_executed", JsonValue::Bool(false)),
            ("event_extent_inferred", JsonValue::Bool(false)),
            ("projection", projected),
        ])
    })?;
    let body = work.canonical(&output, CanonicalProfile::SourceRecordDigestV1)?;
    let mut recheck = || -> Result<()> {
        loaded.recheck(deadline, cancelled)?;
        Ok(())
    };
    recheck()?;
    let result = consume(&mut OwnerDayRangeView {
        body: &body,
        check: &mut recheck,
    });
    recheck()?;
    result
}
