//! Thread-local composition of an owner-loaded frozen chronology and an
//! independently authorized selected CMP view. No persistence or rights grant.
use crate::source_command::{self as cmd, SourceCommandError};
use crate::temporal_chronology::LoadedChronology;
use std::{sync::atomic::AtomicBool, time::Instant};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{
    CanonicalProfile, Digest256, ExactRecordRef, FoundationError, JsonLimits, JsonMode, JsonString,
    JsonValue, OwnedState, RecordVersion, canonical_bytes_v1_with_state_budget_and_visits,
    parse_json_with_state_budget,
};
use tos_query::{
    BoundCmpKnowledge, DisclosableInspect, InspectBudget, InspectCurrentAuthority,
    InspectDisclosureLease, InspectVisitMeter, execute_selected_claim_projection,
    execute_selected_inspect,
    search_v2::{SearchKind, SearchV2Error},
};

#[derive(Debug)]
pub enum StoredChronologyProjectionError {
    Owner(SourceCommandError),
    Query(SearchV2Error),
    Json(FoundationError),
    WrongModelSource,
    MissingMember,
    AmbiguousMember,
    CorruptBinding(&'static str),
    BudgetExceeded,
}
impl From<SourceCommandError> for StoredChronologyProjectionError {
    fn from(e: SourceCommandError) -> Self {
        Self::Owner(e)
    }
}
impl From<SearchV2Error> for StoredChronologyProjectionError {
    fn from(e: SearchV2Error) -> Self {
        Self::Query(e)
    }
}
impl From<FoundationError> for StoredChronologyProjectionError {
    fn from(e: FoundationError) -> Self {
        Self::Json(e)
    }
}
type Result<T> = std::result::Result<T, StoredChronologyProjectionError>;

/// `query` is an aggregate N+1-call allowance, not a fresh allowance per member.
/// Extra bytes/visits cover composition buffers, retained response bodies and
/// forecast lease metadata. Logical state is separate from allocator/RSS caps.
#[derive(Clone, Copy, Debug)]
pub struct StoredChronologyProjectionBudget {
    pub query: InspectBudget,
    pub max_members: usize,
    pub max_extra_owned_bytes: usize,
    pub max_extra_json_visits: usize,
}

/// Available only inside the local held callback. Recheck before each actual
/// disclosure/flush; callers retain responsibility for their local transport.
pub struct StoredChronologyProjectionView<'a> {
    body: &'a [u8],
    check: &'a mut dyn FnMut() -> Result<()>,
}
impl StoredChronologyProjectionView<'_> {
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

fn corrupt(reason: &'static str) -> StoredChronologyProjectionError {
    StoredChronologyProjectionError::CorruptBinding(reason)
}
fn field<'a>(v: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    v.object_get(key)
        .ok_or_else(|| corrupt("missing binding field"))
}
fn text<'a>(v: &'a JsonValue, key: &str) -> Result<&'a str> {
    field(v, key)?
        .as_str()
        .ok_or_else(|| corrupt("binding field is not text"))
}
fn array<'a>(v: &'a JsonValue, key: &str) -> Result<&'a [JsonValue]> {
    field(v, key)?
        .as_array()
        .ok_or_else(|| corrupt("binding field is not array"))
}
fn is_exact(v: &JsonValue, expected: &ExactRecordRef) -> Result<bool> {
    cmd::exact_keys(v, &["id", "version", "digest"])?;
    Ok(text(v, "id")? == expected.id.as_str()
        && RecordVersion::new(
            field(v, "version")?
                .as_u64()
                .ok_or_else(|| corrupt("exact version must be an integer"))?,
        )? == expected.version
        && Digest256::from_prefixed(text(v, "digest")?)? == expected.digest)
}
fn reference(r: &ExactRecordRef, work: &mut Work) -> Result<JsonValue> {
    // to_prefixed owns both an intermediate hex string and its result.
    work.charge(256)?;
    let digest = r.digest.to_prefixed();
    work.object(|w| {
        Ok([
            ("id", w.string(r.id.as_str())?),
            ("version", w.number(r.version.get())?),
            ("digest", w.string(&digest)?),
        ])
    })
}

// Private to this held stack frame; neither body nor lease can escape alone.
struct QueryHold<'a> {
    body: Vec<u8>,
    lease: Box<dyn InspectDisclosureLease + 'a>,
}

struct Work {
    bytes: usize,
    visits: InspectVisitMeter,
    limits: JsonLimits,
}
impl Work {
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or(StoredChronologyProjectionError::BudgetExceeded)?;
        Ok(())
    }
    // Conservative cumulative admission: dropped temporaries do not replenish.
    fn clone_value(&mut self, v: &JsonValue) -> Result<JsonValue> {
        self.charge(v.retained_state_bytes()?)?;
        Ok(v.clone())
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>> {
        self.charge(
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(StoredChronologyProjectionError::BudgetExceeded)?,
        )?;
        Ok(Vec::with_capacity(count))
    }
    fn string(&mut self, value: &str) -> Result<JsonValue> {
        // JsonString retains UTF-8 and WTF-16. Include geometric UTF-16
        // capacity growth/minimum plus the inline slot before constructing.
        self.charge(
            value
                .len()
                .checked_mul(5)
                .and_then(|n| n.checked_add(16 + std::mem::size_of::<JsonValue>()))
                .ok_or(StoredChronologyProjectionError::BudgetExceeded)?,
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
        let entries = build(self)?; // stack array; owned values already admitted
        let mut result = self.vector::<(JsonString, JsonValue)>(N)?;
        for (key, value) in entries {
            self.charge(
                key.len()
                    .checked_mul(5)
                    .and_then(|n| n.checked_add(16))
                    .ok_or(StoredChronologyProjectionError::BudgetExceeded)?,
            )?;
            result.push((JsonString::from_utf8(key), value));
        }
        Ok(JsonValue::Object(result))
    }
    fn response_reservation(&mut self, cap: usize) -> Result<usize> {
        // The query emitter is capped before its call. Reserve Vec growth,
        // not just emitted length; check actual capacity on private unpacking.
        let reserved = cap
            .checked_mul(2)
            .and_then(|n| n.checked_add(8))
            .ok_or(StoredChronologyProjectionError::BudgetExceeded)?;
        self.charge(reserved)?;
        Ok(reserved)
    }
    fn retain<'a>(
        &mut self,
        packet: DisclosableInspect<'a>,
        reserved: usize,
    ) -> Result<QueryHold<'a>> {
        let (body, lease) = packet.into_parts();
        if body.capacity() > reserved {
            return Err(StoredChronologyProjectionError::BudgetExceeded);
        }
        Ok(QueryHold { body, lease })
    }
    fn parse(&mut self, raw: &[u8]) -> Result<JsonValue> {
        let mut limits = self.limits;
        limits.max_visits = limits.max_visits.min(self.visits.remaining());
        let doc = parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, self.bytes)?;
        self.visits.charge_original_visits(doc.visits())?;
        self.charge(doc.root().retained_state_bytes()?)?;
        Ok(doc.into_root())
    }
    fn canonical(&mut self, v: &JsonValue, profile: CanonicalProfile) -> Result<Vec<u8>> {
        let mut limits = self.limits;
        limits.max_bytes = limits.max_bytes.min(self.bytes);
        limits.max_visits = limits.max_visits.min(self.visits.remaining());
        let (bytes, visits) =
            canonical_bytes_v1_with_state_budget_and_visits(v, profile, limits, self.bytes)?;
        self.visits.charge_original_visits(visits)?;
        self.charge(bytes.capacity())?;
        Ok(bytes)
    }
}

fn call_budget(b: StoredChronologyProjectionBudget, n: usize) -> Result<InspectBudget> {
    if n == 0
        || n > b.max_members
        || b.max_members > 128
        || b.max_extra_owned_bytes == 0
        || b.max_extra_json_visits == 0
    {
        return Err(StoredChronologyProjectionError::BudgetExceeded);
    }
    let calls = n
        .checked_add(1)
        .ok_or(StoredChronologyProjectionError::BudgetExceeded)?;
    let calls64 =
        u64::try_from(calls).map_err(|_| StoredChronologyProjectionError::BudgetExceeded)?;
    let mut q = b.query;
    q.max_read_vm_steps /= calls64;
    q.max_rows /= calls64;
    q.max_decoded_bytes /= calls64;
    q.max_response_bytes /= calls;
    q.json.max_visits /= calls;
    if q.max_read_vm_steps == 0
        || q.max_rows < n as u64
        || q.max_matches < n
        || q.max_decoded_bytes == 0
        || q.max_response_bytes == 0
        || q.json.max_visits == 0
    {
        return Err(StoredChronologyProjectionError::BudgetExceeded);
    }
    // max_matches bounds rows, not logical comparisons. Open admission happens
    // once on the borrowed model; max_open_vm_steps remains the original cap.
    Ok(q)
}

fn carrier_locator(
    carrier: &JsonValue,
    graph: &str,
    expected: &ExactRecordRef,
    old_record: &JsonValue,
    work: &mut Work,
) -> Result<JsonValue> {
    if text(carrier, "source_graph")? != graph
        || text(carrier, "kind_id")? != "claim"
        || text(carrier, "type_id")? != "tos.entity.claim"
        || text(field(carrier, "type_mapping")?, "status")? != "mapped"
    {
        return Err(corrupt("member is not a mapped source Claim"));
    }
    let attributes = field(carrier, "attributes")?;
    let source = field(attributes, "source_claim")?;
    let semantics = field(field(carrier, "semantics")?, "claim")?;
    let version = field(source, "claim_version")?
        .as_u64()
        .and_then(|v| RecordVersion::new(v).ok())
        .ok_or_else(|| corrupt("source Claim version is not exact"))?;
    let raw = work.canonical(source, CanonicalProfile::SourceCommandInputV1)?;
    if text(source, "claim_id")? != expected.id.as_str()
        || version != expected.version
        || Digest256::of_bytes(&raw) != expected.digest
        || raw != work.canonical(old_record, CanonicalProfile::SourceCommandInputV1)?
        || text(semantics, "claim_id")? != expected.id.as_str()
        || field(semantics, "claim_version")?.as_u64() != Some(version.get())
        || text(semantics, "source_predicate_id")? != text(source, "predicate")?
        || text(semantics, "predicate_mapping_status")? != "mapped"
    {
        return Err(corrupt("selected source Claim differs from frozen member"));
    }
    // The normalized carrier uses a different digest profile/domain from the
    // owner source reference. Never compare content_revision to either one.
    let source_digest =
        Digest256::of_bytes(&work.canonical(source, CanonicalProfile::SourceRecordDigestV1)?);
    if Digest256::from_hex(text(attributes, "source_sha256")?)? != source_digest {
        return Err(corrupt("selected source Claim digest mismatch"));
    }
    let id = text(carrier, "id")?;
    if id.is_empty() {
        return Err(corrupt("empty selected member node"));
    }
    Digest256::from_hex(text(carrier, "content_revision")?)?;
    work.object(|w| {
        Ok([
            ("node_id", w.string(id)?),
            (
                "content_revision",
                w.string(text(carrier, "content_revision")?)?,
            ),
        ])
    })
}

// The pair and single-member entry share this exact selection/verifier core.
// Work remains private; the pair keeps its original ONE allowance across sides.
fn verified_member_with_work(
    loaded: &LoadedChronology<'_>,
    graph: &str,
    expected: &ExactRecordRef,
    carrier: &JsonValue,
    work: &mut Work,
) -> Result<JsonValue> {
    let members = &loaded.bound().chronology().member_claim_revisions;
    let evidence = array(loaded.bound().evidence(), "members")?;
    if members.len() > 128 || evidence.len() != members.len() {
        return Err(corrupt("comparison owner evidence count"));
    }
    let mut selected = None;
    for (member, owner) in members.iter().zip(evidence) {
        work.visits.charge_original_visits(1)?;
        if member == expected {
            if selected.is_some() {
                return Err(StoredChronologyProjectionError::AmbiguousMember);
            }
            if !is_exact(field(owner, "reference")?, member)? {
                return Err(corrupt("comparison owner evidence identity"));
            }
            selected = Some(owner);
        }
    }
    let owner = selected.ok_or(StoredChronologyProjectionError::MissingMember)?;
    let actual = carrier_locator(carrier, graph, expected, field(owner, "record")?, work)?;
    Ok(actual)
}

/// Caller pre-reserves this child allowance from its aggregate verifier budget.
/// The returned locator is not read authority and contains no owner evidence.
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_loaded_member(
    loaded: &LoadedChronology<'_>,
    graph: &str,
    expected: &ExactRecordRef,
    carrier: &JsonValue,
    max_owned_bytes: usize,
    max_json_visits: usize,
    limits: JsonLimits,
) -> Result<JsonValue> {
    if max_owned_bytes == 0 || max_json_visits == 0 {
        return Err(StoredChronologyProjectionError::BudgetExceeded);
    }
    let mut work = Work {
        bytes: max_owned_bytes,
        visits: InspectVisitMeter::new(max_json_visits),
        limits,
    };
    verified_member_with_work(loaded, graph, expected, carrier, &mut work)
}

/// Verify two already-disclosed private comparison carriers against real held
/// owner members. The caller reserves this entire allowance cumulatively before
/// calling; this function issues no read or disclosure authority and exposes no
/// owner evidence. Keep the original carrier verifier and its workspace private.
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_loaded_comparison_pair(
    loaded: &LoadedChronology<'_>,
    graph: &str,
    expected: [&ExactRecordRef; 2],
    requested: [&JsonValue; 2],
    carriers: [&JsonValue; 2],
    max_owned_bytes: usize,
    max_json_visits: usize,
    limits: JsonLimits,
) -> Result<()> {
    if max_owned_bytes == 0 || max_json_visits == 0 {
        return Err(StoredChronologyProjectionError::BudgetExceeded);
    }
    let members = &loaded.bound().chronology().member_claim_revisions;
    let evidence = array(loaded.bound().evidence(), "members")?;
    if members.len() > 128 || evidence.len() != members.len() {
        return Err(corrupt("comparison owner evidence count"));
    }
    let mut work = Work {
        bytes: max_owned_bytes,
        visits: InspectVisitMeter::new(max_json_visits),
        limits,
    };
    for side in 0..2 {
        let actual =
            verified_member_with_work(loaded, graph, expected[side], carriers[side], &mut work)?;
        cmd::exact_keys(requested[side], &["node_id", "content_revision"])?;
        for key in ["node_id", "content_revision"] {
            if text(&actual, key)? != text(requested[side], key)? {
                return Err(corrupt("comparison changed exact selected member"));
            }
        }
    }
    Ok(())
}

/// Both current authority implementations must already own their real read
/// scopes. This function creates neither a scope nor a source policy grant.
/// Keep all locks/leases local; LoadedChronology deliberately is not Send.
pub fn with_loaded_chronology_projection<'owner, 'lookup, 'projection, LA, PA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    projection_authority: &mut PA,
    display_perspective: &JsonValue,
    budget: StoredChronologyProjectionBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut StoredChronologyProjectionView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    PA: InspectCurrentAuthority<'projection> + ?Sized,
{
    loaded.recheck(deadline, cancelled)?;
    bound.check_model(model)?;
    let chronology = loaded.bound().chronology();
    let revision = bound.require_source_revision()?;
    if Digest256::from_hex(revision)? != chronology.selection.source_snapshot.revision.0 {
        return Err(StoredChronologyProjectionError::WrongModelSource);
    }
    let graph = bound
        .source_for_adapter("reified-bibliographic-claims-v1")
        .ok_or_else(|| corrupt("unique selected source Claim adapter unavailable"))?;
    let members = &chronology.member_claim_revisions;
    let per_call = call_budget(budget, members.len())?;
    let evidence = array(loaded.bound().evidence(), "members")?;
    if evidence.len() != members.len() {
        return Err(corrupt("owner member evidence count"));
    }
    let mut work = Work {
        bytes: budget.max_extra_owned_bytes,
        visits: InspectVisitMeter::new(budget.max_extra_json_visits),
        limits: budget.query.json,
    };
    let mut holds = work.vector::<QueryHold<'lookup>>(members.len())?;
    let mut locators = work.vector::<JsonValue>(members.len())?;
    for (member, owner) in members.iter().zip(evidence) {
        loaded.recheck(deadline, cancelled)?;
        if !is_exact(field(owner, "reference")?, member)? {
            return Err(corrupt("owner member evidence identity"));
        }
        work.charge(lookup_authority.disclosure_metadata_state_upper_bound()?)?;
        let reserved = work.response_reservation(per_call.max_response_bytes)?;
        let packet = execute_selected_inspect(
            model,
            bound,
            lookup_authority,
            SearchKind::Nodes,
            member.id.as_str(),
            0,
            per_call,
        )?;
        let packet = work.retain(packet, reserved)?;
        let value = work.parse(&packet.body)?;
        if text(&value, "source_revision")? != revision {
            return Err(corrupt("inspect source revision"));
        }
        let mut candidates = array(&value, "matches")?
            .iter()
            .filter(|c| c.object_get("source_graph").and_then(JsonValue::as_str) == Some(graph));
        let carrier = candidates
            .next()
            .ok_or(StoredChronologyProjectionError::MissingMember)?;
        if candidates.next().is_some() {
            return Err(StoredChronologyProjectionError::AmbiguousMember);
        }
        let locator = carrier_locator(carrier, graph, member, field(owner, "record")?, &mut work)?;
        // N <= 128; scan the already-admitted locator vector rather than
        // creating another heap index with unaccounted BTree nodes/strings.
        for previous in &locators {
            work.visits.charge_original_visits(1)?;
            if text(previous, "node_id")? == text(&locator, "node_id")? {
                return Err(StoredChronologyProjectionError::AmbiguousMember);
            }
        }
        locators.push(locator);
        holds.push(packet);
    }
    let request = work.object(|w| {
        Ok([
            (
                "schema_version",
                w.string("tos_claim_projection_request_v1")?,
            ),
            ("source_revision", w.string(revision)?),
            ("members", JsonValue::Array(locators)),
            ("display_perspective", w.clone_value(display_perspective)?),
        ])
    })?;
    loaded.recheck(deadline, cancelled)?;
    work.charge(projection_authority.disclosure_metadata_state_upper_bound()?)?;
    let reserved = work.response_reservation(per_call.max_response_bytes)?;
    let projection =
        execute_selected_claim_projection(model, bound, projection_authority, &request, per_call)?;
    let mut projection = work.retain(projection, reserved)?;
    let projected = work.parse(&projection.body)?;
    if text(&projected, "source_revision")? != revision {
        return Err(corrupt("projection source revision"));
    }
    let rows = array(&projected, "rows")?;
    if rows.len() != members.len() {
        return Err(corrupt("projection member count"));
    }
    let mut bindings = work.vector::<JsonValue>(members.len())?;
    for (((row, locator), member), owner) in rows
        .iter()
        .zip(array(&request, "members")?)
        .zip(members)
        .zip(evidence)
    {
        let actual = carrier_locator(
            field(field(row, "operand")?, "claim")?,
            graph,
            member,
            field(owner, "record")?,
            &mut work,
        )?;
        for key in ["node_id", "content_revision"] {
            if text(&actual, key)? != text(locator, key)?
                || text(field(row, "member")?, key)? != text(locator, key)?
            {
                return Err(corrupt("projection changed exact member sequence"));
            }
        }
        let binding = work.object(|w| {
            Ok([
                ("reference", reference(member, w)?),
                ("selected_member", actual),
            ])
        })?;
        bindings.push(binding);
    }
    // Preserve the inner authority_boundary verbatim: the query itself did
    // not persist a chronology or admit an inline/display policy.
    let output = work.object(|w| {
        Ok([
            (
                "schema_version",
                w.string("tos_stored_chronology_projection_result_v1")?,
            ),
            ("chronology_ref", reference(&chronology.reference, w)?),
            ("source_revision", w.string(revision)?),
            (
                "binding_scope",
                w.string("owner-stored-chronology-exact-selected-model-members")?,
            ),
            (
                "policy_scope",
                w.string("InlineSourceDefinitionNotAuthorization")?,
            ),
            ("members", JsonValue::Array(bindings)),
            ("projection", projected),
        ])
    })?;
    let body = work.canonical(&output, CanonicalProfile::SourceRecordDigestV1)?;
    if body.len() > budget.query.max_response_bytes {
        return Err(StoredChronologyProjectionError::BudgetExceeded);
    }
    let mut recheck = || -> Result<()> {
        loaded.recheck(deadline, cancelled)?;
        bound.check_model(model)?;
        for hold in &mut holds {
            hold.lease.recheck()?;
        }
        projection.lease.recheck()?;
        Ok(())
    };
    recheck()?;
    let result = consume(&mut StoredChronologyProjectionView {
        body: &body,
        check: &mut recheck,
    });
    recheck()?;
    result
}
