//! Held owner-member binding to the existing selected source-envelope comparator.
//! No event relation derivation, evidence-policy admission, persistence or grant.
use crate::source_command::SourceCommandError;
use crate::temporal_chronology::LoadedChronology;
use crate::temporal_chronology_projection::{
    StoredChronologyProjectionError, verify_loaded_comparison_pair,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{
    CanonicalProfile, Digest256, ExactRecordRef, FoundationError, JsonLimits, JsonMode, JsonString,
    JsonValue, OwnedState, canonical_bytes_v1_with_state_budget_and_visits,
    parse_json_with_state_budget,
};
use tos_query::{
    BoundCmpKnowledge, InspectBudget, InspectCurrentAuthority, execute_selected_temporal,
    search_v2::SearchV2Error,
};

#[derive(Debug)]
pub enum HeldClaimComparisonError {
    Owner(SourceCommandError),
    Query(SearchV2Error),
    Json(FoundationError),
    Binding(StoredChronologyProjectionError),
    WrongModelSource,
    MissingMember,
    AmbiguousMember,
    InvalidBinding(&'static str),
    BudgetExceeded,
    Cancelled,
    DeadlineExceeded,
}
impl From<SourceCommandError> for HeldClaimComparisonError {
    fn from(e: SourceCommandError) -> Self {
        Self::Owner(e)
    }
}
impl From<SearchV2Error> for HeldClaimComparisonError {
    fn from(e: SearchV2Error) -> Self {
        Self::Query(e)
    }
}
impl From<FoundationError> for HeldClaimComparisonError {
    fn from(e: FoundationError) -> Self {
        Self::Json(e)
    }
}
impl From<StoredChronologyProjectionError> for HeldClaimComparisonError {
    fn from(e: StoredChronologyProjectionError) -> Self {
        Self::Binding(e)
    }
}
type Result<T> = std::result::Result<T, HeldClaimComparisonError>;

#[derive(Clone, Copy, Debug)]
pub struct HeldClaimComparisonBudget {
    /// ONE delegated comparison; its existing VM/read/decoded/response limits apply.
    pub query: InspectBudget,
    /// Cumulative adapter state/work admission, not process or allocator RSS.
    pub max_extra_owned_bytes: usize,
    pub max_extra_json_visits: usize,
    /// Both sides share this verifier allowance, reserved from the above totals.
    pub max_verification_owned_bytes: usize,
    pub max_verification_json_visits: usize,
}

/// Bytes and their owner/model/query guards cannot escape separately. Actual
/// synchronous delivery/flush must happen in this callback's borrow lifetime.
pub struct HeldClaimComparisonView<'a> {
    body: &'a [u8],
    check: &'a mut dyn FnMut() -> Result<()>,
}
impl HeldClaimComparisonView<'_> {
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
fn invalid(reason: &'static str) -> HeldClaimComparisonError {
    HeldClaimComparisonError::InvalidBinding(reason)
}
fn field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    value
        .object_get(key)
        .ok_or_else(|| invalid("missing comparison binding field"))
}
fn text<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| invalid("comparison binding field is not text"))
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(HeldClaimComparisonError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(HeldClaimComparisonError::DeadlineExceeded);
    }
    Ok(())
}
fn same_locator(actual: &JsonValue, requested: &JsonValue) -> Result<()> {
    if !actual.as_object().is_some_and(|o| o.len() == 2) {
        return Err(invalid("invalid returned comparison locator"));
    }
    for key in ["node_id", "content_revision"] {
        if text(actual, key)? != text(requested, key)? {
            return Err(invalid("comparison request locator changed"));
        }
    }
    Ok(())
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
            .ok_or(HeldClaimComparisonError::BudgetExceeded)?;
        Ok(())
    }
    fn visits(&mut self, count: usize) -> Result<()> {
        self.visits = self
            .visits
            .checked_sub(count)
            .ok_or(HeldClaimComparisonError::BudgetExceeded)?;
        Ok(())
    }
    fn walk(&mut self, value: &JsonValue, depth: usize) -> Result<()> {
        if depth > self.limits.max_depth {
            return Err(HeldClaimComparisonError::BudgetExceeded);
        }
        self.visits(1)?;
        match value {
            JsonValue::Object(values) => {
                for (_, value) in values {
                    self.visits(1)?;
                    self.walk(value, depth + 1)?;
                }
            }
            JsonValue::Array(values) => {
                for value in values {
                    self.walk(value, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn clone_value(&mut self, value: &JsonValue) -> Result<JsonValue> {
        self.walk(value, 0)?;
        let reserved = value
            .retained_state_bytes()?
            .checked_mul(2)
            .and_then(|n| n.checked_add(64))
            .ok_or(HeldClaimComparisonError::BudgetExceeded)?;
        self.charge(reserved)?;
        let cloned = value.clone();
        if cloned.retained_state_bytes()? > reserved {
            return Err(HeldClaimComparisonError::BudgetExceeded);
        }
        Ok(cloned)
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>> {
        let slots = count
            .checked_mul(2)
            .and_then(|n| n.checked_add(8))
            .ok_or(HeldClaimComparisonError::BudgetExceeded)?;
        self.charge(
            slots
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(HeldClaimComparisonError::BudgetExceeded)?,
        )?;
        let result = Vec::with_capacity(count);
        if result.capacity() > slots {
            return Err(HeldClaimComparisonError::BudgetExceeded);
        }
        Ok(result)
    }
    fn string(&mut self, value: &str) -> Result<JsonValue> {
        self.charge(
            value
                .len()
                .checked_mul(5)
                .and_then(|n| n.checked_add(16 + std::mem::size_of::<JsonValue>()))
                .ok_or(HeldClaimComparisonError::BudgetExceeded)?,
        )?;
        Ok(JsonValue::String(JsonString::from_utf8(value)))
    }
    fn number(&mut self, value: u64) -> Result<JsonValue> {
        self.charge(20 + std::mem::size_of::<JsonValue>())?;
        Ok(crate::source_command::number(value))
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
                    .ok_or(HeldClaimComparisonError::BudgetExceeded)?,
            )?;
            fields.push((JsonString::from_utf8(key), value));
        }
        Ok(JsonValue::Object(fields))
    }
    fn reference(&mut self, reference: &ExactRecordRef) -> Result<JsonValue> {
        self.charge(256)?;
        let digest = reference.digest.to_prefixed();
        self.object(|w| {
            Ok([
                ("id", w.string(reference.id.as_str())?),
                ("version", w.number(reference.version.get())?),
                ("digest", w.string(&digest)?),
            ])
        })
    }
    fn parse(&mut self, raw: &[u8]) -> Result<JsonValue> {
        let mut limits = self.limits;
        limits.max_visits = limits.max_visits.min(self.visits);
        let document =
            parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, self.bytes)?;
        self.visits(document.visits())?;
        self.charge(document.root().retained_state_bytes()?)?;
        Ok(document.into_root())
    }
    fn canonical(&mut self, value: &JsonValue) -> Result<Vec<u8>> {
        let mut limits = self.limits;
        limits.max_bytes = limits.max_bytes.min(self.bytes);
        limits.max_visits = limits.max_visits.min(self.visits);
        let (body, visits) = canonical_bytes_v1_with_state_budget_and_visits(
            value,
            CanonicalProfile::SourceRecordDigestV1,
            limits,
            self.bytes,
        )?;
        self.visits(visits)?;
        self.charge(body.capacity())?;
        Ok(body)
    }
}

/// The authority is supplied by the existing owner, not made from caller policy
/// strings. A missing/wrong Temporal scope is refused by the existing comparator.
/// Explicit reflexive pairs are valid; missing sides never fall back to self.
#[allow(clippy::too_many_arguments)]
pub fn with_loaded_chronology_envelope_comparison<'owner, 'hold, A, T>(
    loaded: LoadedChronology<'owner>,
    left_member: &ExactRecordRef,
    right_member: &ExactRecordRef,
    left_locator: &JsonValue,
    right_locator: &JsonValue,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    temporal_authority: &mut A,
    budget: HeldClaimComparisonBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut HeldClaimComparisonView<'view>) -> Result<T>,
) -> Result<T>
where
    A: InspectCurrentAuthority<'hold> + ?Sized,
{
    active(deadline, cancelled)?;
    loaded.recheck(deadline, cancelled)?;
    bound.check_model(model)?;
    let chronology = loaded.bound().chronology();
    let revision = bound.require_source_revision()?;
    if Digest256::from_hex(revision)? != chronology.selection.source_snapshot.revision.0 {
        return Err(HeldClaimComparisonError::WrongModelSource);
    }
    let graph = bound
        .source_for_adapter("reified-bibliographic-claims-v1")
        .ok_or_else(|| invalid("unique selected source Claim adapter unavailable"))?;
    if budget.max_extra_owned_bytes == 0
        || budget.max_extra_json_visits == 0
        || budget.max_verification_owned_bytes == 0
        || budget.max_verification_json_visits == 0
        || budget.query.max_response_bytes == 0
        || budget.query.json.max_bytes == 0
    {
        return Err(HeldClaimComparisonError::BudgetExceeded);
    }
    let mut limits = budget.query.json;
    limits.max_depth = limits.max_depth.min(64);
    limits.max_bytes = limits.max_bytes.min(budget.query.max_response_bytes);
    let mut work = Work {
        bytes: budget.max_extra_owned_bytes,
        visits: budget.max_extra_json_visits,
        limits,
    };
    // Both verifier sides share one suballowance, removed from the operation
    // totals before any delegated query. Dropped temporaries never refund it.
    work.charge(budget.max_verification_owned_bytes)?;
    work.visits(budget.max_verification_json_visits)?;
    if chronology.member_claim_revisions.len() > 128 {
        return Err(HeldClaimComparisonError::BudgetExceeded);
    }
    for expected in [left_member, right_member] {
        let mut count = 0usize;
        for member in &chronology.member_claim_revisions {
            work.visits(1)?;
            if member == expected {
                count += 1;
            }
        }
        match count {
            0 => return Err(HeldClaimComparisonError::MissingMember),
            1 => {}
            _ => return Err(HeldClaimComparisonError::AmbiguousMember),
        }
    }
    let request = work.object(|w| {
        Ok([
            (
                "schema_version",
                w.string("tos_temporal_comparison_request_v1")?,
            ),
            ("source_revision", w.string(revision)?),
            ("left", w.clone_value(left_locator)?),
            ("right", w.clone_value(right_locator)?),
        ])
    })?;
    // Bound all caller strings before the existing request validator allocates
    // normalized ID strings. Emitted query state remains its own bounded owner.
    let request_bound = work.canonical(&request)?;
    if request_bound.len() > budget.query.max_payload_bytes {
        return Err(HeldClaimComparisonError::BudgetExceeded);
    }
    // The public validator materializes UTF-8 even for a WTF-16 ID. Reserve
    // its temporary conversion before invoking it; strip itself is borrowed.
    for locator in [left_locator, right_locator] {
        if let Some(JsonValue::String(id)) = locator.object_get("node_id") {
            work.charge(
                id.units()
                    .len()
                    .checked_mul(12)
                    .and_then(|n| n.checked_add(64))
                    .ok_or(HeldClaimComparisonError::BudgetExceeded)?,
            )?;
        }
    }
    tos_query::validate_temporal_request(&request)?;
    work.charge(temporal_authority.disclosure_metadata_state_upper_bound()?)?;
    let response_reserved = budget
        .query
        .max_response_bytes
        .checked_mul(2)
        .and_then(|n| n.checked_add(8))
        .ok_or(HeldClaimComparisonError::BudgetExceeded)?;
    work.charge(response_reserved)?;
    active(deadline, cancelled)?;
    loaded.recheck(deadline, cancelled)?;
    let packet =
        execute_selected_temporal(model, bound, temporal_authority, &request, budget.query)?;
    let (private_body, mut lease) = packet.into_parts();
    if private_body.capacity() > response_reserved {
        return Err(HeldClaimComparisonError::BudgetExceeded);
    }
    active(deadline, cancelled)?;
    loaded.recheck(deadline, cancelled)?;
    bound.check_model(model)?;
    lease.recheck()?;
    let native = work.parse(&private_body)?;
    if text(&native, "schema_version")? != "tos_temporal_comparison_result_v1"
        || text(&native, "source_revision")? != revision
        || text(field(&native, "request")?, "schema_version")?
            != "tos_temporal_comparison_request_v1"
        || text(field(&native, "request")?, "source_revision")? != revision
    {
        return Err(invalid("comparison response source or schema changed"));
    }
    same_locator(field(field(&native, "request")?, "left")?, left_locator)?;
    same_locator(field(field(&native, "request")?, "right")?, right_locator)?;
    verify_loaded_comparison_pair(
        &loaded,
        graph,
        [left_member, right_member],
        [left_locator, right_locator],
        [
            field(field(&native, "left")?, "claim")?,
            field(field(&native, "right")?, "claim")?,
        ],
        budget.max_verification_owned_bytes,
        budget.max_verification_json_visits,
        limits,
    )?;
    // Closed public provenance only. The native comparison is moved intact;
    // its source envelope relation is never promoted into an A3 event outcome.
    let output = work.object(|w| {
        Ok([
            (
                "schema_version",
                w.string("tos_stored_claim_envelope_comparison_v1")?,
            ),
            (
                "scope",
                w.string("OwnerMemberVerifiedSourceEnvelopeComparison")?,
            ),
            ("a3_assessment", w.string("NotEstablished")?),
            (
                "policy_scope",
                w.string("SourceSelectionProvenanceNotAuthorization")?,
            ),
            ("chronology_ref", w.reference(&chronology.reference)?),
            ("source_revision", w.string(revision)?),
            ("left_member_ref", w.reference(left_member)?),
            ("right_member_ref", w.reference(right_member)?),
            (
                "evidence_policy_ref",
                w.reference(&chronology.evidence_policy)?,
            ),
            (
                "selection_policy_ref",
                w.reference(&chronology.selection_policy)?,
            ),
            ("native_comparison", native),
        ])
    })?;
    let body = work.canonical(&output)?;
    if body.len() > budget.query.max_response_bytes {
        return Err(HeldClaimComparisonError::BudgetExceeded);
    }
    let mut recheck = || -> Result<()> {
        active(deadline, cancelled)?;
        loaded.recheck(deadline, cancelled)?;
        bound.check_model(model)?;
        lease.recheck()?;
        active(deadline, cancelled)
    };
    recheck()?;
    let result = consume(&mut HeldClaimComparisonView {
        body: &body,
        check: &mut recheck,
    });
    recheck()?;
    result
}
