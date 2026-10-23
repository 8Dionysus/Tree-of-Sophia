//! Held frozen metadata pages. Source selection, current rights and inline
//! source policies remain separate authorities; this adapter grants none.
use crate::source_command::{self as cmd, SourceCommandError};
use crate::temporal_chronology::LoadedChronology;
use crate::temporal_chronology_projection::{
    StoredChronologyProjectionError, verify_loaded_member,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::query_context::{
    AttributedKnowledge, ContextResolution, Cutoff, EffectiveQueryContext, KnowledgeSelection,
    MismatchDimension, QueryContext, SourceSnapshot, SystemState, resolve_query_context,
};
use tos_foundation::temporal::{
    TemporalBudget, TemporalError, claim_carrier::ClaimCarrierError, decode_claim_carrier,
    evaluate_claim_world_window_filter, validate_claim_world_window_filter,
};
use tos_foundation::{
    CanonicalProfile, Digest256, ExactRecordRef, FoundationError, FoundationErrorCode, JsonLimits,
    JsonMode, JsonString, JsonValue, OwnedState, canonical_bytes_v1_with_state_budget_and_visits,
    parse_json_with_state_budget,
};
use tos_query::{
    BoundCmpKnowledge, DisclosableInspect, InspectBudget, InspectCurrentAuthority,
    InspectDisclosureLease, InspectVisitMeter, execute_selected_claim_list,
    execute_selected_inspect,
    search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode},
};

const INPUT_CAP: usize = 65_536;
const BODY_CAP: usize = 2 * 1024 * 1024;
#[derive(Debug)]
pub enum StoredChronologyListError {
    Owner(SourceCommandError),
    Query(SearchV2Error),
    Binding(StoredChronologyProjectionError),
    InvalidRequest(&'static str),
    NeedsBinding(&'static str),
    ContextMismatch(Vec<MismatchDimension>),
    WrongModelSource,
    MissingMember,
    AmbiguousMember,
    CorruptBinding(&'static str),
    Resource(&'static str),
    Json(FoundationError),
    Temporal(TemporalError),
    Cancelled,
    DeadlineExceeded,
}
type Result<T> = std::result::Result<T, StoredChronologyListError>;
impl From<SourceCommandError> for StoredChronologyListError {
    fn from(e: SourceCommandError) -> Self {
        Self::Owner(e)
    }
}
impl From<SearchV2Error> for StoredChronologyListError {
    fn from(e: SearchV2Error) -> Self {
        Self::Query(e)
    }
}
impl From<FoundationError> for StoredChronologyListError {
    fn from(e: FoundationError) -> Self {
        if e.code == FoundationErrorCode::BudgetExceeded {
            resource()
        } else {
            Self::Json(e)
        }
    }
}
impl From<StoredChronologyProjectionError> for StoredChronologyListError {
    fn from(e: StoredChronologyProjectionError) -> Self {
        match e {
            StoredChronologyProjectionError::BudgetExceeded => resource(),
            StoredChronologyProjectionError::Json(e)
                if e.code == FoundationErrorCode::BudgetExceeded =>
            {
                resource()
            }
            StoredChronologyProjectionError::Query(e)
                if e.code == SearchV2ErrorCode::BudgetExceeded =>
            {
                resource()
            }
            other => Self::Binding(other),
        }
    }
}
fn resource() -> StoredChronologyListError {
    StoredChronologyListError::Resource("held list cumulative budget")
}
fn invalid(s: &'static str) -> StoredChronologyListError {
    StoredChronologyListError::InvalidRequest(s)
}
fn corrupt(s: &'static str) -> StoredChronologyListError {
    StoredChronologyListError::CorruptBinding(s)
}
fn temporal(e: TemporalError) -> StoredChronologyListError {
    match e {
        TemporalError::ResourceExhausted(_) => resource(),
        other => StoredChronologyListError::Temporal(other),
    }
}

/// All allowances are explicit. Verification is a prepaid subset of extra state.
/// Query and temporal allowances are aggregate, never defaults reset per row.
#[derive(Clone, Copy, Debug)]
pub struct StoredChronologyListBudget {
    pub query: InspectBudget,
    pub temporal: TemporalBudget,
    pub max_members: usize,
    pub max_extra_owned_bytes: usize,
    pub max_extra_json_visits: usize,
    pub max_verification_owned_bytes: usize,
    pub max_verification_json_visits: usize,
}
pub struct StoredChronologyListView<'a> {
    body: &'a [u8],
    check: &'a mut dyn FnMut() -> Result<()>,
}
impl StoredChronologyListView<'_> {
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
struct Hold<'a> {
    body: Vec<u8>,
    lease: Box<dyn InspectDisclosureLease + 'a>,
}
struct Work {
    bytes: usize,
    visits: InspectVisitMeter,
    limits: JsonLimits,
}
impl Work {
    fn charge(&mut self, n: usize) -> Result<()> {
        self.bytes = self.bytes.checked_sub(n).ok_or_else(resource)?;
        Ok(())
    }
    fn visit(&mut self, v: &JsonValue, depth: usize) -> Result<()> {
        self.visits
            .charge_original_visits(1)
            .map_err(|_| resource())?;
        if depth > 64 {
            return Err(resource());
        }
        match v {
            JsonValue::Array(a) => {
                for x in a {
                    self.visit(x, depth + 1)?;
                }
            }
            JsonValue::Object(a) => {
                for (_, x) in a {
                    self.visit(x, depth + 1)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    fn reserve_clone(&mut self, v: &JsonValue) -> Result<()> {
        self.visit(v, 0)?;
        self.charge(
            v.retained_state_bytes()?
                .checked_mul(2)
                .and_then(|n| n.checked_add(64))
                .ok_or_else(resource)?,
        )
    }
    fn clone_value(&mut self, v: &JsonValue) -> Result<JsonValue> {
        self.reserve_clone(v)?;
        Ok(v.clone())
    }
    fn vector<T>(&mut self, n: usize) -> Result<Vec<T>> {
        self.charge(
            n.checked_mul(std::mem::size_of::<T>())
                .ok_or_else(resource)?,
        )?;
        Ok(Vec::with_capacity(n))
    }
    fn string(&mut self, s: &str) -> Result<JsonValue> {
        self.charge(
            s.len()
                .checked_mul(5)
                .and_then(|n| n.checked_add(32 + std::mem::size_of::<JsonValue>()))
                .ok_or_else(resource)?,
        )?;
        Ok(cmd::string(s))
    }
    fn number(&mut self, n: u64) -> Result<JsonValue> {
        self.charge(20 + std::mem::size_of::<JsonValue>())?;
        Ok(cmd::number(n))
    }
    fn object<const N: usize>(
        &mut self,
        make: impl FnOnce(&mut Self) -> Result<[(&'static str, JsonValue); N]>,
    ) -> Result<JsonValue> {
        let fields = make(self)?;
        let mut out = self.vector::<(JsonString, JsonValue)>(N)?;
        for (key, v) in fields {
            self.charge(
                key.len()
                    .checked_mul(5)
                    .and_then(|n| n.checked_add(32))
                    .ok_or_else(resource)?,
            )?;
            out.push((JsonString::from_utf8(key), v));
        }
        Ok(JsonValue::Object(out))
    }
    fn parse(&mut self, raw: &[u8]) -> Result<JsonValue> {
        let mut l = self.limits;
        l.max_visits = l.max_visits.min(self.visits.remaining());
        let doc = parse_json_with_state_budget(raw, JsonMode::PublishedStrict, l, self.bytes)?;
        self.visits
            .charge_original_visits(doc.visits())
            .map_err(|_| resource())?;
        self.charge(doc.root().retained_state_bytes()?)?;
        Ok(doc.into_root())
    }
    fn canonical(&mut self, v: &JsonValue, cap: usize) -> Result<Vec<u8>> {
        let mut l = self.limits;
        l.max_bytes = l.max_bytes.min(cap).min(self.bytes);
        l.max_visits = l.max_visits.min(self.visits.remaining());
        let (b, n) = canonical_bytes_v1_with_state_budget_and_visits(
            v,
            CanonicalProfile::SourceCommandInputV1,
            l,
            self.bytes,
        )?;
        self.visits
            .charge_original_visits(n)
            .map_err(|_| resource())?;
        self.charge(b.capacity())?;
        Ok(b)
    }
    // Caller-owned JSON may be constructed programmatically, so canonicalization
    // itself can discover malformed/duplicate input before the shape validator.
    // Keep resource exhaustion distinct; never apply this to stored/response data.
    fn canonical_input(&mut self, v: &JsonValue, cap: usize) -> Result<Vec<u8>> {
        self.canonical(v, cap).map_err(|e| match e {
            StoredChronologyListError::Json(_) => invalid("caller JSON canonicalization"),
            other => other,
        })
    }
    fn digest(&mut self, v: &JsonValue, cap: usize) -> Result<JsonValue> {
        let raw = self.canonical(v, cap)?;
        self.charge(256)?;
        self.string(&Digest256::of_bytes(&raw).to_prefixed())
    }
    fn ref_wire(&mut self, r: &ExactRecordRef) -> Result<JsonValue> {
        self.charge(256)?;
        let digest = r.digest.to_prefixed();
        self.object(|w| {
            Ok([
                ("id", w.string(r.id.as_str())?),
                ("version", w.number(r.version.get())?),
                ("digest", w.string(&digest)?),
            ])
        })
    }
    fn response_reservation<'a, A: InspectCurrentAuthority<'a> + ?Sized>(
        &mut self,
        a: &A,
        q: InspectBudget,
    ) -> Result<usize> {
        self.charge(a.disclosure_metadata_state_upper_bound()?)?;
        let cap = q
            .max_response_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(8))
            .ok_or_else(resource)?;
        self.charge(cap)?;
        Ok(cap)
    }
    fn retain<'a>(&mut self, p: DisclosableInspect<'a>, reserved: usize) -> Result<Hold<'a>> {
        let (body, lease) = p.into_parts();
        if body.capacity() > reserved {
            return Err(resource());
        }
        Ok(Hold { body, lease })
    }
    // Pure calls own decoded/response trees without accepting this adapter's
    // state meter. Prepay a conservative node/string/canonical-output forecast.
    // Dropped temporaries do not replenish the cumulative allowance.
    fn pure_reservation(&mut self, b: TemporalBudget, inputs: &[&JsonValue]) -> Result<()> {
        // Clone-bearing output consists of subtrees of these actual inputs,
        // typed decoder wrappers, and finite framing. Bound traversal FIRST;
        // do not turn the generous visitor ceiling into an enormous fictitious
        // all-nodes-live tree. The pure APIs independently enforce their visit /
        // duplicate-key / pairing ceilings; these remain distinct CPU limits.
        let mut retained = 0usize;
        for input in inputs {
            self.visit(input, 0)?;
            retained = retained
                .checked_add(input.retained_state_bytes()?)
                .and_then(|n| n.checked_add(std::mem::size_of::<JsonValue>()))
                .ok_or_else(resource)?;
        }
        // Eight copies cover original-packet plus typed field trees, parsed
        // exact refs/strings, source/query result clones and overlap. Twelve
        // emitted-byte capacities cover the pure bounded emit/hash forecasts;
        // fixed framing includes the <=21 result fields and typed wrappers.
        let tree = retained.checked_mul(8).ok_or_else(resource)?;
        let buffers = b
            .max_expression_bytes
            .checked_mul(12)
            .ok_or_else(resource)?;
        self.charge(
            tree.checked_add(buffers)
                .and_then(|n| n.checked_add(65_536))
                .ok_or_else(resource)?,
        )
    }
}
fn field<'a>(v: &'a JsonValue, k: &str) -> Result<&'a JsonValue> {
    get(v, k).ok_or_else(|| corrupt("missing response field"))
}
fn text<'a>(v: &'a JsonValue, k: &str) -> Result<&'a str> {
    field(v, k)?
        .as_str()
        .ok_or_else(|| corrupt("response field type"))
}
fn array<'a>(v: &'a JsonValue, k: &str) -> Result<&'a [JsonValue]> {
    field(v, k)?
        .as_array()
        .ok_or_else(|| corrupt("response array type"))
}
fn exact_keys(v: &JsonValue, keys: &[&str]) -> Result<()> {
    let a = v.as_object().ok_or_else(|| invalid("object required"))?;
    if a.len() != keys.len() {
        return Err(invalid("unknown, duplicate or missing key"));
    }
    for (i, (k, _)) in a.iter().enumerate() {
        let k = k.as_str().ok_or_else(|| invalid("text key required"))?;
        if !keys.contains(&k) || a[..i].iter().any(|(p, _)| p.as_str() == Some(k)) {
            return Err(invalid("unknown or duplicate key"));
        }
    }
    Ok(())
}
fn response_keys(v: &JsonValue, keys: &[&str]) -> Result<()> {
    exact_keys(v, keys).map_err(|_| corrupt("native response shape"))
}
fn stop(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(StoredChronologyListError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(StoredChronologyListError::DeadlineExceeded);
    }
    Ok(())
}
fn cutoff(w: &mut Work, c: &Cutoff) -> Result<JsonValue> {
    w.object(|w| {
        Ok([
            ("value", w.clone_value(&c.value)?),
            (
                "trs",
                match &c.trs {
                    Some(r) => w.ref_wire(r)?,
                    None => JsonValue::Null,
                },
            ),
        ])
    })
}
fn attribution(w: &mut Work, a: &AttributedKnowledge) -> Result<JsonValue> {
    w.object(|w| {
        Ok([
            ("knower", w.string(a.knower.as_str())?),
            ("cutoff", cutoff(w, &a.cutoff)?),
        ])
    })
}
fn selection(w: &mut Work, s: &KnowledgeSelection) -> Result<JsonValue> {
    w.object(|w| {
        Ok([
            (
                "source_snapshot",
                w.object(|w| {
                    w.charge(256)?;
                    Ok([
                        ("id", w.string(s.source_snapshot.id.as_str())?),
                        (
                            "revision",
                            w.string(&s.source_snapshot.revision.0.to_prefixed())?,
                        ),
                    ])
                })?,
            ),
            ("system_as_of", cutoff(w, &s.system_as_of)?),
            (
                "attributed_knowledge",
                match &s.attributed_knowledge {
                    Some(a) => attribution(w, a)?,
                    None => JsonValue::Null,
                },
            ),
            (
                "source_publication_as_of",
                match &s.source_publication_as_of {
                    Some(c) => cutoff(w, c)?,
                    None => JsonValue::Null,
                },
            ),
        ])
    })
}
fn effective(w: &mut Work, c: &EffectiveQueryContext) -> Result<JsonValue> {
    w.object(|w| {
        Ok([
            ("selection", selection(w, &c.selection)?),
            (
                "chronology_version",
                match &c.chronology_version {
                    Some(r) => w.ref_wire(r)?,
                    None => JsonValue::Null,
                },
            ),
            (
                "world_valid_time",
                match &c.world_valid_time {
                    Some(v) => w.clone_value(v)?,
                    None => JsonValue::Null,
                },
            ),
            (
                "display_perspective",
                match &c.display_perspective {
                    Some(v) => w.clone_value(v)?,
                    None => JsonValue::Null,
                },
            ),
        ])
    })
}
fn tag(w: &mut Work, v: Option<JsonValue>) -> Result<JsonValue> {
    w.object(|w| {
        Ok([
            (
                "tag",
                w.string(if v.is_some() { "Present" } else { "Omitted" })?,
            ),
            ("value", v.unwrap_or(JsonValue::Null)),
        ])
    })
}
fn requested(w: &mut Work, c: &QueryContext) -> Result<JsonValue> {
    let system = match &c.system_state {
        None => None,
        Some(SystemState::Current) => {
            Some(w.object(|w| Ok([("tag", w.string("Current")?), ("cutoff", JsonValue::Null)]))?)
        }
        Some(SystemState::AsOf(v)) => {
            Some(w.object(|w| Ok([("tag", w.string("AsOf")?), ("cutoff", cutoff(w, v)?)]))?)
        }
    };
    let attributed = match &c.attributed_knowledge {
        None => None,
        Some(None) => Some(JsonValue::Null),
        Some(Some(a)) => Some(attribution(w, a)?),
    };
    let publication = match &c.source_publication_as_of {
        None => None,
        Some(None) => Some(JsonValue::Null),
        Some(Some(v)) => Some(cutoff(w, v)?),
    };
    let world = c
        .world_valid_time
        .as_ref()
        .map(|v| w.clone_value(v))
        .transpose()?;
    let display = c
        .display_perspective
        .as_ref()
        .map(|v| w.clone_value(v))
        .transpose()?;
    w.object(|w| {
        Ok([
            ("system_state", tag(w, system)?),
            ("attributed_knowledge", tag(w, attributed)?),
            ("source_publication_as_of", tag(w, publication)?),
            ("world_valid_time", tag(w, world)?),
            ("display_perspective", tag(w, display)?),
        ])
    })
}
fn quotas(
    b: StoredChronologyListBudget,
    n: usize,
    world: bool,
) -> Result<(InspectBudget, TemporalBudget, usize, usize)> {
    if n == 0 {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedFrozenEmpty",
        ));
    }
    if n > b.max_members
        || b.max_members > 128
        || b.max_extra_owned_bytes == 0
        || b.max_extra_owned_bytes > 64 * 1024 * 1024
        || b.max_extra_json_visits == 0
        || b.max_extra_json_visits > 4_000_000
        || b.max_verification_owned_bytes == 0
        || b.max_verification_owned_bytes > b.max_extra_owned_bytes
        || b.max_verification_json_visits == 0
        || b.max_verification_json_visits > b.max_extra_json_visits
    {
        return Err(resource());
    }
    let t = b.temporal;
    if t.max_expression_bytes == 0
        || t.max_expression_bytes > BODY_CAP
        || t.max_members == 0
        || t.max_members > 1_000_000
        || t.max_pairings == 0
        || t.max_pairings > 1_000_000
    {
        return Err(resource());
    }
    let calls = n.checked_add(1).ok_or_else(resource)?;
    let calls64 = u64::try_from(calls).map_err(|_| resource())?;
    let mut q = b.query;
    q.max_read_vm_steps /= calls64;
    q.max_rows /= calls64;
    q.max_decoded_bytes /= calls64;
    q.max_response_bytes /= calls;
    q.json.max_visits /= calls;
    q.json.max_bytes = q.json.max_bytes.min(BODY_CAP);
    q.json.max_depth = q.json.max_depth.min(64);
    q.max_response_bytes = q.max_response_bytes.min(BODY_CAP);
    q.max_payload_bytes = q.max_payload_bytes.min(1024 * 1024);
    q.max_field_bytes = q.max_field_bytes.min(16 * 1024);
    if q.max_read_vm_steps == 0
        || q.max_rows == 0
        || q.max_decoded_bytes == 0
        || q.max_response_bytes == 0
        || q.json.max_visits == 0
        || q.max_matches == 0
        || q.max_payload_bytes == 0
        || q.max_field_bytes == 0
    {
        return Err(resource());
    }
    let vc = n.checked_mul(2).ok_or_else(resource)?;
    let vb = b.max_verification_owned_bytes / vc;
    let vv = b.max_verification_json_visits / vc;
    if vb == 0 || vv == 0 {
        return Err(resource());
    }
    let wc = if world {
        vc.checked_add(1).ok_or_else(resource)?
    } else {
        1
    };
    let wt = TemporalBudget {
        max_expression_bytes: t.max_expression_bytes / wc,
        max_members: t.max_members / wc,
        max_pairings: t.max_pairings / wc,
    };
    if wt.max_expression_bytes == 0 || wt.max_members == 0 || wt.max_pairings == 0 {
        return Err(resource());
    }
    Ok((q, wt, vb, vv))
}
fn get<'a>(v: &'a JsonValue, k: &str) -> Option<&'a JsonValue> {
    v.as_object()?
        .iter()
        .find(|(key, _)| key.as_str() == Some(k))
        .map(|(_, value)| value)
}
fn optional_digest(w: &mut Work, v: Option<&JsonValue>) -> Result<JsonValue> {
    match v {
        Some(v) => w.digest(v, INPUT_CAP),
        None => Ok(JsonValue::Null),
    }
}
fn refusal(
    w: &mut Work,
    r: &ExactRecordRef,
    revision: &JsonValue,
    domain: &str,
    reason: &str,
) -> Result<JsonValue> {
    w.object(|w| {
        Ok([
            ("frozen_member_ref", w.ref_wire(r)?),
            ("source_revision", w.clone_value(revision)?),
            ("domain", w.string(domain)?),
            ("reason", w.string(reason)?),
        ])
    })
}
fn add(w: &mut Work, v: &mut JsonValue, key: &'static str, value: JsonValue) -> Result<()> {
    // Root owns an exact up-front capacity for the larger success shape.
    w.charge(
        key.len()
            .checked_mul(5)
            .and_then(|n| n.checked_add(32))
            .ok_or_else(resource)?,
    )?;
    let JsonValue::Object(a) = v else {
        return Err(corrupt("output root"));
    };
    if a.len() == a.capacity() {
        return Err(resource());
    }
    a.push((JsonString::from_utf8(key), value));
    Ok(())
}
fn cursor_check(w: &mut Work, after: &JsonValue, binding: &JsonValue) -> Result<JsonValue> {
    if after.is_null() {
        return Ok(JsonValue::Null);
    }
    exact_keys(
        after,
        &[
            "schema_version",
            "chronology_ref",
            "member_set_sha256",
            "context_sha256",
            "filter_sha256",
            "world_filter_sha256",
            "world_rules_context_sha256",
            "page_size",
            "scan_limit",
            "provider_after",
        ],
    )?;
    if get(after, "schema_version")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid("cursor schema type"))?
        != "tos_stored_chronology_list_cursor_v1"
    {
        return Err(invalid("cursor schema"));
    }
    // Compare canonical values, not order-sensitive JsonValue object equality.
    for key in [
        "chronology_ref",
        "member_set_sha256",
        "context_sha256",
        "filter_sha256",
        "world_filter_sha256",
        "world_rules_context_sha256",
        "page_size",
        "scan_limit",
    ] {
        if w.canonical_input(field(after, key)?, INPUT_CAP)?
            != w.canonical(field(binding, key)?, INPUT_CAP)?
        {
            return Err(invalid("cursor binding changed"));
        }
    }
    w.clone_value(field(after, "provider_after")?)
}

/// Both authorities are caller-owned actual scopes; this function creates none.
/// All public bytes are available only under the held synchronous callback.
#[allow(clippy::too_many_arguments)]
pub fn with_loaded_chronology_metadata_list<'owner, 'lookup, 'list, LA, MA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    list_authority: &mut MA,
    snapshot: &SourceSnapshot,
    context: &QueryContext,
    world_rules_context: Option<&JsonValue>,
    request: &JsonValue,
    budget: StoredChronologyListBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut StoredChronologyListView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    MA: InspectCurrentAuthority<'list> + ?Sized,
{
    held_metadata_list(
        loaded,
        model,
        bound,
        lookup_authority,
        list_authority,
        snapshot,
        context,
        world_rules_context,
        request,
        budget,
        deadline,
        cancelled,
        None,
        None,
        consume,
    )
}

#[allow(clippy::too_many_arguments)]
fn held_metadata_list<'owner, 'lookup, 'list, LA, MA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    list_authority: &mut MA,
    snapshot: &SourceSnapshot,
    context: &QueryContext,
    world_rules_context: Option<&JsonValue>,
    request: &JsonValue,
    budget: StoredChronologyListBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    declarations: Option<&ManifestDeclarationView<'_>>,
    mut attributed: Option<&mut AttributedPredicate<'_>>,
    consume: impl for<'view> FnOnce(&mut StoredChronologyListView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    MA: InspectCurrentAuthority<'list> + ?Sized,
{
    stop(deadline, cancelled)?;
    let chronology = loaded.bound().chronology();
    let members = &chronology.member_claim_revisions;
    let world = context.world_valid_time.as_ref();
    let (q, wt, vbytes, vvisits) = quotas(budget, members.len(), world.is_some())?;
    let mut limits = budget.query.json;
    limits.max_bytes = limits.max_bytes.min(BODY_CAP);
    limits.max_depth = limits.max_depth.min(64);
    let mut w = Work {
        bytes: budget.max_extra_owned_bytes,
        visits: InspectVisitMeter::new(budget.max_extra_json_visits),
        limits,
    };
    // Reserve all 2N verifier children ONCE from the parent's cumulative state.
    w.charge(budget.max_verification_owned_bytes)?;
    w.visits
        .charge_original_visits(budget.max_verification_json_visits)
        .map_err(|_| resource())?;
    w.canonical_input(request, INPUT_CAP)?;
    exact_keys(
        request,
        &[
            "schema_version",
            "filter",
            "page_size",
            "scan_limit",
            "after",
        ],
    )?;
    if get(request, "schema_version")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid("request schema type"))?
        != "tos_stored_chronology_metadata_list_request_v1"
    {
        return Err(invalid("request schema"));
    }
    let filter = field(request, "filter")?;
    let fm = filter.as_object().ok_or_else(|| invalid("filter object"))?;
    if fm.len() > 5 {
        return Err(invalid("metadata filter field count"));
    }
    for (i, (key, v)) in fm.iter().enumerate() {
        let key = key.as_str().ok_or_else(|| invalid("filter text key"))?;
        if ![
            "subject_ref",
            "predicate",
            "claim_type",
            "assertion_layer",
            "visibility",
        ]
        .contains(&key)
            || fm[..i].iter().any(|(p, _)| p.as_str() == Some(key))
            || !v.as_str().is_some_and(|s| !s.is_empty())
        {
            return Err(invalid("metadata filter"));
        }
    }
    let count = |key| -> Result<usize> {
        field(request, key)?
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("positive integer page/scan required"))
    };
    let page_size = count("page_size")?;
    let scan_limit = count("scan_limit")?;
    if page_size > q.max_matches
        || u64::try_from(scan_limit.checked_mul(2).ok_or_else(resource)?).map_err(|_| resource())?
            > q.max_rows
    {
        return Err(resource());
    }
    if context.display_perspective.is_some() {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedDisplaySelection",
        ));
    }
    let rules = match (world, world_rules_context) {
        (None, None) => None,
        (None, Some(_)) => return Err(invalid("unused world rules")),
        (Some(_), None) => {
            return Err(StoredChronologyListError::NeedsBinding("MissingWorldRules"));
        }
        (Some(f), Some(r)) => {
            w.canonical_input(f, INPUT_CAP)?;
            w.canonical_input(r, INPUT_CAP)?;
            if get(f, "profile_ref").and_then(JsonValue::as_str)
                != Some("tos_world_civil_year_window_filter_v1")
            {
                return Err(StoredChronologyListError::NeedsBinding(
                    "UnsupportedWorldSelection",
                ));
            }
            w.pure_reservation(wt, &[f, r])?;
            validate_claim_world_window_filter(f, r, wt).map_err(|e| match e {
                TemporalError::MalformedRequest(s) => invalid(s),
                other => temporal(other),
            })?;
            Some(r)
        }
    };
    // Build tagged borrowed inputs with admission; then prepay the resolver's
    // typed clones and their overlap before it runs (it has no budget argument).
    // Preserve resolver semantics: an unused supplied current_cutoff is only
    // a descriptor bound into the private cursor. For Current the resolver
    // compares it; omitted/AsOf may legitimately select an earlier frozen cut.
    let req_context = requested(&mut w, context)?;
    let snap_wire = w.object(|w| {
        w.charge(256)?;
        Ok([
            (
                "reference",
                w.object(|w| {
                    Ok([
                        ("id", w.string(snapshot.reference.id.as_str())?),
                        (
                            "revision",
                            w.string(&snapshot.reference.revision.0.to_prefixed())?,
                        ),
                    ])
                })?,
            ),
            ("current_cutoff", cutoff(w, &snapshot.current_cutoff)?),
        ])
    })?;
    w.canonical_input(&req_context, INPUT_CAP)?;
    w.canonical_input(&snap_wire, INPUT_CAP)?;
    let selected_wire = selection(&mut w, &chronology.selection)?;
    w.reserve_clone(&req_context)?;
    w.reserve_clone(&snap_wire)?;
    w.reserve_clone(&selected_wire)?;
    w.charge(
        std::mem::size_of::<EffectiveQueryContext>()
            + 5 * std::mem::size_of::<MismatchDimension>()
            + 1024,
    )?;
    let resolved = resolve_query_context(snapshot, context, Some(chronology))
        .map_err(|_| invalid("invalid context cutoff"))?;
    let resolved = match resolved {
        ContextResolution::Resolved(c) => c,
        ContextResolution::MissingSelection => {
            return Err(StoredChronologyListError::NeedsBinding("MissingSelection"));
        }
        ContextResolution::ContextMismatch(d) => {
            return Err(StoredChronologyListError::ContextMismatch(d));
        }
    };
    let effective_wire = effective(&mut w, &resolved)?;
    w.canonical(&effective_wire, INPUT_CAP)?;
    let tagged_context = w.object(|w| {
        Ok([
            ("requested", req_context),
            ("snapshot", snap_wire),
            ("effective", w.clone_value(&effective_wire)?),
        ])
    })?;
    let context_digest = w.digest(&tagged_context, 3 * INPUT_CAP)?;
    let revision = chronology.selection.source_snapshot.revision;
    w.charge(256)?;
    let source_revision = w.string(&revision.0.to_prefixed())?;
    let revision_bare = bound.require_source_revision()?;
    if Digest256::from_hex(revision_bare)? != revision.0 {
        return Err(StoredChronologyListError::WrongModelSource);
    }
    let graph = bound
        .source_for_adapter("reified-bibliographic-claims-v1")
        .ok_or_else(|| corrupt("unique source Claim adapter required"))?;
    let mut member_refs = w.vector::<JsonValue>(members.len())?;
    for member in members {
        member_refs.push(w.ref_wire(member)?);
    }
    let member_digest = w.digest(&JsonValue::Array(member_refs), BODY_CAP)?;
    let filter_digest = w.digest(filter, INPUT_CAP)?;
    let world_digest = optional_digest(&mut w, world)?;
    let rules_digest = optional_digest(&mut w, rules)?;
    let binding = w.object(|w| {
        Ok([
            (
                "schema_version",
                w.string("tos_stored_chronology_list_cursor_v1")?,
            ),
            ("chronology_ref", w.ref_wire(&chronology.reference)?),
            ("member_set_sha256", member_digest),
            ("context_sha256", context_digest),
            ("filter_sha256", filter_digest),
            ("world_filter_sha256", world_digest),
            ("world_rules_context_sha256", w.clone_value(&rules_digest)?),
            (
                "page_size",
                w.number(u64::try_from(page_size).map_err(|_| resource())?)?,
            ),
            (
                "scan_limit",
                w.number(u64::try_from(scan_limit).map_err(|_| resource())?)?,
            ),
        ])
    })?;
    let after = field(request, "after")?;
    w.canonical_input(after, INPUT_CAP)?;
    let native_after = cursor_check(&mut w, after, &binding)?;
    loaded.recheck(deadline, cancelled)?;
    bound.check_model(model)?;
    let mut holds = w.vector::<Hold<'lookup>>(members.len())?;
    let mut locators = w.vector::<JsonValue>(members.len())?;
    let mut world_results = w.vector::<JsonValue>(members.len())?;
    let mut refusals = w.vector::<JsonValue>(members.len())?;
    let mut world_match = w.vector::<bool>(members.len())?;
    for member in members {
        stop(deadline, cancelled)?;
        loaded.recheck(deadline, cancelled)?;
        // InspectRequest owns the identifier and its normalization workspace.
        w.charge(
            member
                .id
                .as_str()
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(1024))
                .ok_or_else(resource)?,
        )?;
        if attributed.is_some() && lookup_authority.disclosure_metadata_state_upper_bound()? > 4096
        {
            return Err(resource());
        }
        let reserved = w.response_reservation(lookup_authority, q)?;
        if attributed.is_some() {
            lookup_authority.observe_inspect_budget(
                "begin",
                ["member0", "member1", "member2", "member3"][holds.len()],
                [None; 4],
            );
        }
        let packet = execute_selected_inspect(
            model,
            bound,
            lookup_authority,
            SearchKind::Nodes,
            member.id.as_str(),
            0,
            q,
        );
        if attributed.is_some()
            && matches!(&packet, Err(reason) if reason.code == SearchV2ErrorCode::BudgetExceeded)
        {
            // Covers native authority and prelude errors outside these source
            // paths. The owner records it ONLY if no earlier fault exists.
            lookup_authority.observe_inspect_budget(
                "fallback",
                "unclassified_native_budget",
                [None; 4],
            );
        }
        let packet = packet?;
        let hold = w.retain(packet, reserved)?;
        let value = w.parse(&hold.body)?;
        if text(&value, "source_revision")? != revision_bare {
            return Err(corrupt("inspect source revision"));
        }
        let mut found = None;
        for carrier in array(&value, "matches")? {
            w.visits.charge_original_visits(1).map_err(|_| resource())?;
            if get(carrier, "source_graph").and_then(JsonValue::as_str) == Some(graph)
                && get(carrier, "kind_id").and_then(JsonValue::as_str) == Some("claim")
            {
                if found.is_some() {
                    return Err(StoredChronologyListError::AmbiguousMember);
                }
                found = Some(carrier);
            }
        }
        let carrier = found.ok_or(StoredChronologyListError::MissingMember)?;
        let locator =
            verify_loaded_member(&loaded, graph, member, carrier, vbytes, vvisits, q.json)?;
        for old in &locators {
            w.visits.charge_original_visits(1).map_err(|_| resource())?;
            if text(old, "node_id")? == text(&locator, "node_id")? {
                return Err(StoredChronologyListError::AmbiguousMember);
            }
        }
        locators.push(locator); // capacity/output already prepaid in helper child
        if let (Some(f), Some(r)) = (world, rules) {
            let source = field(field(carrier, "attributes")?, "source_claim")?;
            let qualifiers = get(source, "qualifiers");
            // Absence of qualifiers needs no synthetic object or codec call.
            // Present null is SOURCE data and remains a codec refusal.
            let decoded = if let Some(qualifiers) = qualifiers {
                w.pure_reservation(wt, &[qualifiers])?;
                match decode_claim_carrier(qualifiers, wt) {
                    Ok(c) => Some(c),
                    Err(ClaimCarrierError::ResourceLimit) => return Err(resource()),
                    Err(ClaimCarrierError::Malformed(s))
                    | Err(ClaimCarrierError::Unsupported(s)) => {
                        refusals.push(refusal(
                            &mut w,
                            member,
                            &source_revision,
                            "ClaimCarrierCodec",
                            s,
                        )?);
                        None
                    }
                }
            } else {
                Some(None)
            };
            if let Some(decoded) = decoded {
                match qualifiers {
                    Some(qualifiers) => w.pure_reservation(wt, &[f, r, qualifiers])?,
                    None => w.pure_reservation(wt, &[f, r])?,
                }
                let result = evaluate_claim_world_window_filter(
                    decoded.as_ref(),
                    f,
                    r,
                    member,
                    revision,
                    wt,
                )
                .map_err(temporal)?;
                if let Some(declarations) = declarations {
                    if matches!(text(&result, "outcome")?, "Match" | "NoMatch") {
                        check_manifest_declaration(
                            &mut w,
                            declarations,
                            member,
                            &source_revision,
                            &result,
                        )?;
                    }
                }
                match text(&result, "outcome")? {
                    "Match" => world_match.push(true),
                    "NoMatch" => world_match.push(false),
                    "Refused" => {
                        world_match.push(false);
                        refusals.push(refusal(
                            &mut w,
                            member,
                            &source_revision,
                            "WorldWindow",
                            text(&result, "reason")?,
                        )?);
                    }
                    _ => return Err(corrupt("world outcome")),
                }
                world_results.push(w.object(|w| {
                    Ok([
                        ("frozen_member_ref", w.ref_wire(member)?),
                        ("result", result),
                    ])
                })?);
            } else {
                world_match.push(false);
            }
        } else {
            world_match.push(attributed.is_none());
        }
        if let Some(predicate) = attributed.as_deref_mut() {
            predicate.observe(member, carrier)?;
        }
        holds.push(hold);
    }
    // ONE real list page, including on semantic source refusal. Its native
    // body stays private; all examined candidates still pass current authority.
    let native_request = w.object(|w| {
        Ok([
            ("schema_version", w.string("tos_claim_list_request_v1")?),
            ("source_revision", w.string(revision_bare)?),
            ("filter", w.clone_value(filter)?),
            (
                "page_size",
                w.number(u64::try_from(page_size).map_err(|_| resource())?)?,
            ),
            (
                "scan_limit",
                w.number(u64::try_from(scan_limit).map_err(|_| resource())?)?,
            ),
            ("after", native_after),
        ])
    })?;
    stop(deadline, cancelled)?;
    loaded.recheck(deadline, cancelled)?;
    let metadata = list_authority.disclosure_metadata_state_upper_bound()?;
    if attributed.is_some() && metadata > 4096 {
        return Err(resource());
    }
    w.charge(
        metadata
            .checked_mul(16)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(resource)?,
    )?;
    w.reserve_clone(&native_request)?;
    // Parser/canonical request overlap and framing are bounded by the wrapper's
    // admitted request, not the caller's possibly larger JSON ceiling.
    w.charge(INPUT_CAP.checked_mul(4).ok_or_else(resource)?)?;
    let list_q = if attributed.is_some() {
        // This private entry has already admitted exactly four custody members,
        // predicate-only historical_dating, no world filter, and a five-row page.
        // Prepay the copied budget/scalar headers and bounded borrowed scans.
        let workspace = std::mem::size_of::<InspectBudget>()
            .checked_add(
                16usize
                    .checked_mul(std::mem::size_of::<usize>())
                    .ok_or_else(resource)?,
            )
            .ok_or_else(resource)?;
        w.charge(workspace)?;
        w.visits
            .charge_original_visits(32)
            .map_err(|_| resource())?;
        if members.len() != 4 || holds.len() != 4 || locators.len() != 4 {
            return Err(corrupt("attributed response member count"));
        }
        let string_bound = |s: &str| -> Result<usize> {
            s.len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(2))
                .ok_or_else(resource)
        };
        let mut inspected = 0usize;
        for hold in &holds {
            inspected = inspected
                .checked_add(hold.body.len())
                .ok_or_else(resource)?;
        }
        // The full same-profile carrier is a subtree of its held inspect body.
        // Row framing/ref values add 243+J(node)+J(claim); the complete native
        // page/cursor adds 872+2J(receipt)+J(graph), where J(s)=6*UTF8len(s)+2.
        // Four rows therefore fit B=S+1844+sum J(node/claim)+2J(receipt)+J(graph).
        let mut response_bound = inspected.checked_add(1844).ok_or_else(resource)?;
        for (locator, member) in locators.iter().zip(members) {
            let node_bound = string_bound(text(locator, "node_id")?)?;
            let claim_bound = string_bound(member.id.as_str())?;
            response_bound = response_bound
                .checked_add(node_bound)
                .and_then(|n| n.checked_add(claim_bound))
                .ok_or_else(resource)?;
        }
        let receipt_bound = string_bound(bound.owner_receipt_id())?
            .checked_mul(2)
            .ok_or_else(resource)?;
        let graph_bound = string_bound(graph)?;
        response_bound = response_bound
            .checked_add(receipt_bound)
            .and_then(|n| n.checked_add(graph_bound))
            .ok_or_else(resource)?;
        let remaining_wire = budget
            .query
            .max_response_bytes
            .checked_sub(inspected)
            .ok_or_else(resource)?;
        if response_bound == 0
            || response_bound > remaining_wire
            || response_bound > BODY_CAP
            || response_bound > q.json.max_bytes
        {
            return Err(resource());
        }
        // Wire consumption is cumulative; this NEVER refunds owned state.
        // The four old capacity forecasts and leases stay charged and held.
        let mut list_q = q;
        list_q.max_response_bytes = response_bound;
        list_q
    } else {
        q
    };
    // Charge the entire enlarged list capacity BEFORE native delegation from
    // the same remaining parent, without credit for earlier unused ceilings.
    let reserved = w.response_reservation(list_authority, list_q)?;
    if attributed.is_some() {
        list_authority.observe_inspect_budget("begin", "list", [None; 4]);
    }
    let packet = execute_selected_claim_list(model, bound, list_authority, &native_request, list_q);
    if attributed.is_some()
        && matches!(&packet, Err(reason) if reason.code == SearchV2ErrorCode::BudgetExceeded)
    {
        list_authority.observe_inspect_budget("fallback", "unclassified_native_budget", [None; 4]);
    }
    let packet = packet?;
    let mut list_hold = w.retain(packet, reserved)?;
    let page = w.parse(&list_hold.body)?;
    response_keys(
        &page,
        &[
            "schema_version",
            "source_revision",
            "selected_model_receipt_id",
            "filter",
            "rows",
            "examined_candidates",
            "exhausted",
            "next",
            "projection_scope",
            "cursor_scope",
            "binds_persisted_chronology",
        ],
    )?;
    if text(&page, "schema_version")? != "tos_claim_list_result_v1"
        || text(&page, "source_revision")? != revision_bare
        || text(&page, "selected_model_receipt_id")? != bound.owner_receipt_id()
    {
        return Err(corrupt("native list binding"));
    }
    if text(&page, "projection_scope")? != "exact-selected-model-source-keyset-page"
        || text(&page, "cursor_scope")? != "untrusted-suffix-position"
        || field(&page, "binds_persisted_chronology")?.as_bool() != Some(false)
        || w.canonical(field(&page, "filter")?, INPUT_CAP)? != w.canonical(filter, INPUT_CAP)?
    {
        return Err(corrupt("native list scope or filter"));
    }
    let exhausted = field(&page, "exhausted")?
        .as_bool()
        .ok_or_else(|| corrupt("native exhaustion"))?;
    let examined = field(&page, "examined_candidates")?
        .as_u64()
        .ok_or_else(|| corrupt("native examined count"))?;
    if examined > u64::try_from(scan_limit).map_err(|_| resource())? {
        return Err(corrupt("native scan count"));
    }
    let native_next = field(&page, "next")?;
    if exhausted && !native_next.is_null() || !exhausted && native_next.is_null() {
        return Err(corrupt("native suffix consistency"));
    }
    let rows = array(&page, "rows")?;
    if rows.len() > page_size {
        return Err(corrupt("native list rows"));
    }
    let mut selected = w.vector::<JsonValue>(rows.len())?;
    let mut seen = w.vector::<bool>(members.len())?;
    seen.resize(members.len(), false);
    for row in rows {
        stop(deadline, cancelled)?;
        response_keys(
            row,
            &["node_id", "content_revision", "claim_ref", "carrier"],
        )?;
        let provider_ref = field(row, "claim_ref")?;
        response_keys(provider_ref, &["id", "version", "digest"])?;
        let id = text(provider_ref, "id")?;
        let mut selected_member = None;
        for (i, member) in members.iter().enumerate() {
            w.visits.charge_original_visits(1).map_err(|_| resource())?;
            if member.id.as_str() == id {
                if selected_member.is_some() {
                    return Err(StoredChronologyListError::AmbiguousMember);
                }
                selected_member = Some((i, member));
            }
        }
        let Some((index, member)) = selected_member else {
            continue;
        };
        if seen[index] {
            return Err(corrupt("duplicate frozen row"));
        }
        seen[index] = true;
        if field(provider_ref, "version")?.as_u64() != Some(member.version.get()) {
            return Err(corrupt("frozen stable id changed version"));
        }
        let carrier = field(row, "carrier")?;
        let actual =
            verify_loaded_member(&loaded, graph, member, carrier, vbytes, vvisits, q.json)?;
        for key in ["node_id", "content_revision"] {
            if text(&actual, key)? != text(&locators[index], key)?
                || text(row, key)? != text(&actual, key)?
            {
                return Err(corrupt("frozen locator changed"));
            }
        }
        // SourceRecordDigestV1 was independently recomputed by the common
        // verifier. Never compare this provider digest to owner.digest.
        let attributes = field(carrier, "attributes")?;
        let provider_digest = Digest256::from_prefixed(text(provider_ref, "digest")?)?;
        if provider_digest != Digest256::from_hex(text(attributes, "source_sha256")?)? {
            return Err(corrupt("provider source digest domain"));
        }
        if world_match[index] {
            selected.push(w.object(|w| {
                Ok([
                    ("node_id", w.clone_value(field(row, "node_id")?)?),
                    (
                        "content_revision",
                        w.clone_value(field(row, "content_revision")?)?,
                    ),
                    ("frozen_member_ref", w.ref_wire(member)?),
                    ("provider_claim_ref", w.clone_value(provider_ref)?),
                    (
                        "provider_claim_digest_profile",
                        w.string("SourceRecordDigestV1")?,
                    ),
                    ("carrier", w.clone_value(carrier)?),
                ])
            })?);
        }
    }
    // Closed public shape with one up-front root capacity. No native envelope,
    // nonmember row or private owner packet/evidence/config is embedded.
    let output = if let Some(predicate) = attributed.as_deref_mut() {
        predicate.output(
            chronology,
            revision,
            bound.owner_receipt_id(),
            &effective_wire,
            &seen,
        )?
    } else {
        let mut output = JsonValue::Object(w.vector::<(JsonString, JsonValue)>(25)?);
        let has_refusals = !refusals.is_empty();
        let common = [
            (
                "schema_version",
                w.string("tos_stored_chronology_metadata_list_result_v1")?,
            ),
            (
                "outcome",
                w.string(if has_refusals {
                    "NonTotalRefusal"
                } else {
                    "Page"
                })?,
            ),
            ("chronology_ref", w.ref_wire(&chronology.reference)?),
            ("source_revision", source_revision),
            (
                "selected_model_receipt_id",
                w.string(bound.owner_receipt_id())?,
            ),
            ("effective_context", effective_wire),
            ("filter", w.clone_value(filter)?),
            (
                "world_filter",
                match world {
                    Some(f) => w.clone_value(f)?,
                    None => JsonValue::Null,
                },
            ),
            ("world_rules_context_sha256", rules_digest),
            (
                "binding_scope",
                w.string("OwnerStoredChronologyExactSelectedModelMembers")?,
            ),
            (
                "policy_scope",
                w.string("InlineSourceDefinitionNotAuthorization")?,
            ),
            (
                "rules_scope",
                if world.is_some() {
                    w.string("ExplicitCallerRulesSourceMatchedNotPolicyAdmission")?
                } else {
                    JsonValue::Null
                },
            ),
            ("assertion_scope", w.string("SupplementalCarrierOnly")?),
            ("legacy_object_assessment", w.string("NotExamined")?),
            ("event_extent_inferred", JsonValue::Bool(false)),
            ("a3_scope", w.string("NotEstablished")?),
        ];
        for (key, value) in common {
            add(&mut w, &mut output, key, value)?;
        }
        if has_refusals {
            add(&mut w, &mut output, "refusals", JsonValue::Array(refusals))?;
        } else {
            let next = native_next;
            let next = if next.is_null() {
                JsonValue::Null
            } else {
                let mut cursor = w.clone_value(&binding)?;
                // clone capacity is exact length; pre-admit the added entry and
                // reserve growth BEFORE push (no hidden Vec reallocation).
                if let JsonValue::Object(ref mut fields) = cursor {
                    w.charge(
                        (fields.len() + 1)
                            .checked_mul(std::mem::size_of::<(JsonString, JsonValue)>())
                            .ok_or_else(resource)?,
                    )?;
                    fields.reserve_exact(1);
                }
                let native = w.clone_value(next)?;
                add(&mut w, &mut cursor, "provider_after", native)?;
                w.canonical(&cursor, INPUT_CAP)?;
                cursor
            };
            add(&mut w, &mut output, "rows", JsonValue::Array(selected))?;
            let examined = w.number(examined)?;
            add(&mut w, &mut output, "examined_candidates", examined)?;
            add(&mut w, &mut output, "exhausted", JsonValue::Bool(exhausted))?;
            add(&mut w, &mut output, "next", next)?;
            let scope = w.string("untrusted-suffix-position")?;
            add(&mut w, &mut output, "cursor_scope", scope)?;
            add(
                &mut w,
                &mut output,
                "world_assessments",
                JsonValue::Array(world_results),
            )?;
        }
        output
    };
    let body = w.canonical(&output, budget.query.max_response_bytes.min(BODY_CAP))?;
    let mut recheck = || -> Result<()> {
        stop(deadline, cancelled)?;
        loaded.recheck(deadline, cancelled)?;
        bound.check_model(model)?;
        for hold in &mut holds {
            hold.lease.recheck()?;
        }
        list_hold.lease.recheck()?;
        Ok(())
    };
    recheck()?;
    let result = consume(&mut StoredChronologyListView {
        body: &body,
        check: &mut recheck,
    });
    recheck()?;
    result
}

/// Five explicit caller mappings, not an alias registry, identity credential or
/// source admission. Concrete values must match the retained selection/filter.
/// The supplemental TRS uses its existing ID/version domain, not a record digest.
pub struct ListEnvelopeAliases<'a> {
    pub snapshot: (
        &'a str,
        &'a tos_foundation::query_context::SourceSnapshotRef,
    ),
    pub revision_fence: (&'a str, tos_foundation::SourceRevision),
    pub chronology: (&'a str, &'a ExactRecordRef),
    pub world_about: (&'a str, &'a ExactRecordRef),
    pub world_trs: (&'a str, &'a str, &'a str),
}

// Not a permission token or public plan. The sole owned field remains charged
// for the entire delegated operation; all other execution arguments are borrowed.
struct BoundEnvelopeRequest {
    context: QueryContext,
}
fn envelope_text<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str> {
    get(value, key)
        .and_then(JsonValue::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("envelope nonempty text required"))
}
fn envelope_known(value: &JsonValue, known: &[&str]) -> Result<()> {
    let fields = value
        .as_object()
        .ok_or_else(|| invalid("envelope object required"))?;
    for (i, (key, _)) in fields.iter().enumerate() {
        let key = key
            .as_str()
            .ok_or_else(|| invalid("envelope text key required"))?;
        if !known.contains(&key)
            || fields[..i]
                .iter()
                .any(|(previous, _)| previous.as_str() == Some(key))
        {
            return Err(invalid("ExpectedRequestEnvelopeSubprotocol"));
        }
    }
    Ok(())
}
fn envelope_probe(probe: Option<&dyn tos_query::AbortProbe>) -> Result<()> {
    let (code, message) = match probe.and_then(|probe| probe.reason()) {
        None => return Ok(()),
        Some(tos_query::AbortReason::Cancelled) => {
            (SearchV2ErrorCode::Cancelled, "envelope caller cancelled")
        }
        Some(tos_query::AbortReason::DeadlineExceeded) => (
            SearchV2ErrorCode::DeadlineExceeded,
            "envelope caller deadline exceeded",
        ),
    };
    Err(StoredChronologyListError::Query(SearchV2Error {
        code,
        message,
    }))
}
fn bind_list_envelope(
    w: &mut Work,
    loaded: &LoadedChronology<'_>,
    bound: &BoundCmpKnowledge<'_>,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ListEnvelopeAliases<'_>,
    world: &JsonValue,
) -> Result<BoundEnvelopeRequest> {
    bind_envelope_fields(
        w,
        loaded,
        bound,
        snapshot,
        input,
        aliases,
        world,
        &["operation", "source_profile", "subject", "request_envelope"],
    )
}

fn bind_envelope_fields(
    w: &mut Work,
    loaded: &LoadedChronology<'_>,
    bound: &BoundCmpKnowledge<'_>,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ListEnvelopeAliases<'_>,
    world: &JsonValue,
    top_keys: &[&str],
) -> Result<BoundEnvelopeRequest> {
    exact_keys(input, top_keys)?;
    // Validate the entire described caller shape before capability refusals.
    // Unsupported selector values remain opaque: this is not a selector parser.
    let operation = envelope_text(input, "operation")?;
    let profile = envelope_text(input, "source_profile")?;
    let subject_symbol = envelope_text(input, "subject")?;
    let envelope =
        get(input, "request_envelope").ok_or_else(|| invalid("request envelope required"))?;
    envelope_known(
        envelope,
        &["source_snapshot_ref", "temporal", "principal_auth_context"],
    )?;
    let source =
        get(envelope, "source_snapshot_ref").ok_or_else(|| invalid("source snapshot required"))?;
    envelope_known(
        source,
        &["snapshot_id", "revision_fence", "resolved_system_cutoff"],
    )?;
    let snapshot_symbol = envelope_text(source, "snapshot_id")?;
    let fence_symbol = envelope_text(source, "revision_fence")?;
    let temporal = get(envelope, "temporal").ok_or_else(|| invalid("temporal required"))?;
    envelope_known(
        temporal,
        &[
            "chronology_version",
            "world_valid_time",
            "system_state",
            "attributed_knowledge_as_of",
            "source_publication_as_of",
            "display_perspective",
        ],
    )?;
    let chronology_symbol = if get(temporal, "chronology_version").is_some() {
        Some(envelope_text(temporal, "chronology_version")?)
    } else {
        None
    };
    let selection = if let Some(selection) = get(temporal, "world_valid_time") {
        exact_keys(selection, &["position", "trs_ref"])?;
        Some((
            envelope_text(selection, "position")?,
            envelope_text(selection, "trs_ref")?,
        ))
    } else {
        None
    };
    let principal = get(envelope, "principal_auth_context");
    if let Some(principal) = principal {
        exact_keys(principal, &["principal_ref", "authorization"])?;
        envelope_text(principal, "principal_ref")?;
        envelope_text(principal, "authorization")?;
    }
    if operation != "list_claims" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeOperation",
        ));
    }
    if profile != "ToS-Temporal-Core-Contract-v1" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeProfile",
        ));
    }
    if principal.is_some() {
        return Err(StoredChronologyListError::NeedsBinding(
            "CurrentPrincipalEvidence",
        ));
    }
    if get(source, "resolved_system_cutoff").is_some() {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedSnapshotCutoff",
        ));
    }
    if [
        "system_state",
        "attributed_knowledge_as_of",
        "source_publication_as_of",
        "display_perspective",
    ]
    .iter()
    .any(|key| get(temporal, key).is_some())
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeSelector",
        ));
    }
    let (Some(chronology_symbol), Some((lexical, trs_symbol))) = (chronology_symbol, selection)
    else {
        return Err(StoredChronologyListError::NeedsBinding(
            "FrozenWorldEnvelopeRequired",
        ));
    };

    // Fixed borrowed slots have no lookup allocations. These are mappings, not
    // authenticators of names, clocks, Event truth or current caller identity.
    let mut alias_bytes = 0usize;
    for text in [
        aliases.snapshot.0,
        aliases.snapshot.1.id.as_str(),
        aliases.revision_fence.0,
        aliases.chronology.0,
        aliases.chronology.1.id.as_str(),
        aliases.world_about.0,
        aliases.world_about.1.id.as_str(),
        aliases.world_trs.0,
        aliases.world_trs.1,
        aliases.world_trs.2,
    ] {
        if text.is_empty() || text.len() > 1024 {
            return Err(resource());
        }
        alias_bytes = alias_bytes.checked_add(text.len()).ok_or_else(resource)?;
    }
    if alias_bytes > 8192 {
        return Err(resource());
    }
    if snapshot_symbol != aliases.snapshot.0
        || fence_symbol != aliases.revision_fence.0
        || chronology_symbol != aliases.chronology.0
        || subject_symbol != aliases.world_about.0
        || trs_symbol != aliases.world_trs.0
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnresolvedEnvelopeAlias",
        ));
    }
    let chronology = loaded.bound().chronology();
    if aliases.snapshot.1 != &snapshot.reference
        || aliases.revision_fence.1 != snapshot.reference.revision
        || aliases.chronology.1 != &chronology.reference
    {
        return Err(invalid("EnvelopeBindingMismatch"));
    }
    let frozen = &chronology.selection.source_snapshot;
    if snapshot.reference != *frozen {
        // At most two source dimensions. Pre-admit even the error container.
        w.charge(
            std::mem::size_of::<Vec<MismatchDimension>>()
                + 2 * std::mem::size_of::<MismatchDimension>(),
        )?;
        let mut dimensions = Vec::with_capacity(2);
        if snapshot.reference.id != frozen.id {
            dimensions.push(MismatchDimension::SourceSnapshotRef);
        }
        if snapshot.reference.revision != frozen.revision {
            dimensions.push(MismatchDimension::SourceRevisionFence);
        }
        return Err(StoredChronologyListError::ContextMismatch(dimensions));
    }
    if Digest256::from_hex(bound.require_source_revision()?)? != frozen.revision.0 {
        return Err(StoredChronologyListError::WrongModelSource);
    }
    let about = get(world, "about_ref").ok_or_else(|| invalid("world about required"))?;
    exact_keys(about, &["id", "version", "digest"])?;
    let digest = Digest256::from_hex(envelope_text(about, "digest")?)
        .map_err(|_| invalid("world about digest"))?;
    if envelope_text(about, "id")? != aliases.world_about.1.id.as_str()
        || get(about, "version").and_then(JsonValue::as_u64)
            != Some(aliases.world_about.1.version.get())
        || digest != aliases.world_about.1.digest
    {
        return Err(invalid("EnvelopeBindingMismatch"));
    }
    let position = get(world, "position").ok_or_else(|| invalid("world position required"))?;
    let trs = get(position, "trs").ok_or_else(|| invalid("world TRS required"))?;
    exact_keys(trs, &["id", "version"])?;
    if envelope_text(position, "lexical")? != lexical
        || envelope_text(trs, "id")? != aliases.world_trs.1
        || envelope_text(trs, "version")? != aliases.world_trs.2
    {
        return Err(invalid("EnvelopeBindingMismatch"));
    }
    // Full world/rules validation, actual source parsing and current authority
    // remain with the existing ONE held operation. No extra pure call here.
    let world = w.clone_value(world)?;
    Ok(BoundEnvelopeRequest {
        context: QueryContext {
            world_valid_time: Some(world),
            ..QueryContext::default()
        },
    })
}

/// Execute only the original59 frozen-world RequestEnvelope subprotocol.
/// Whole manifests/principal credentials are neither accepted nor manufactured.
/// The result, leases and native cursor are exactly the existing held API's.
#[allow(clippy::too_many_arguments)]
pub fn with_loaded_chronology_request_envelope<'owner, 'lookup, 'list, LA, MA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    list_authority: &mut MA,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ListEnvelopeAliases<'_>,
    page_request: &JsonValue,
    world_supplement: &JsonValue,
    world_rules_context: &JsonValue,
    budget: StoredChronologyListBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut StoredChronologyListView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    MA: InspectCurrentAuthority<'list> + ?Sized,
{
    stop(deadline, cancelled)?;
    // Preserve the existing member/query/temporal admission, then reserve the
    // SAME parent's verifier portion while the prefix uses only the remainder.
    quotas(
        budget,
        loaded.bound().chronology().member_claim_revisions.len(),
        true,
    )?;
    let build_bytes = budget
        .max_extra_owned_bytes
        .checked_sub(budget.max_verification_owned_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let build_visits = budget
        .max_extra_json_visits
        .checked_sub(budget.max_verification_json_visits)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let mut limits = budget.query.json;
    limits.max_bytes = limits.max_bytes.min(INPUT_CAP);
    limits.max_depth = limits.max_depth.min(64);
    let mut prefix = Work {
        bytes: build_bytes,
        visits: InspectVisitMeter::new(build_visits),
        limits,
    };
    // Fixed private-plan/context/budget headers and both transport probe handles
    // are prepaid before construction/acquisition. Delegated owner internals
    // retain their existing bounds; this is not an RSS or allocator-peak claim.
    prefix.charge(
        std::mem::size_of::<BoundEnvelopeRequest>()
            .checked_add(std::mem::size_of::<StoredChronologyListBudget>())
            .and_then(|n| {
                n.checked_add(
                    2 * std::mem::size_of::<Option<std::sync::Arc<dyn tos_query::AbortProbe>>>(),
                )
            })
            .ok_or_else(resource)?,
    )?;
    let lookup_abort = lookup_authority.abort_probe();
    let list_abort = list_authority.abort_probe();
    for value in [input, page_request, world_supplement, world_rules_context] {
        stop(deadline, cancelled)?;
        envelope_probe(lookup_abort.as_deref())?;
        envelope_probe(list_abort.as_deref())?;
        // SourceCommandInputV1 is used only for bounded caller validation here;
        // no emitted envelope or alias digest is inserted into the cursor.
        prefix.canonical_input(value, INPUT_CAP)?;
    }
    let plan = bind_list_envelope(
        &mut prefix,
        &loaded,
        bound,
        snapshot,
        input,
        aliases,
        world_supplement,
    )?;
    stop(deadline, cancelled)?;
    envelope_probe(lookup_abort.as_deref())?;
    envelope_probe(list_abort.as_deref())?;
    let remaining = StoredChronologyListBudget {
        max_extra_owned_bytes: prefix
            .bytes
            .checked_add(budget.max_verification_owned_bytes)
            .ok_or_else(resource)?,
        max_extra_json_visits: prefix
            .visits
            .remaining()
            .checked_add(budget.max_verification_json_visits)
            .ok_or_else(resource)?,
        ..budget
    };
    // Do not refund the prefix's retained world clone, buffers or work even
    // when temporaries die. Inner Work starts with parent minus ALL charges.
    with_loaded_chronology_metadata_list(
        loaded,
        model,
        bound,
        lookup_authority,
        list_authority,
        snapshot,
        &plan.context,
        Some(world_rules_context),
        page_request,
        remaining,
        deadline,
        cancelled,
        consume,
    )
}

/// Exact two-member declaration mappings. No identity, clock or policy grant.
/// Distinct slots must bind different actual Claim StableIds and exact refs.
pub struct ListManifestAliases<'a> {
    pub envelope: ListEnvelopeAliases<'a>,
    pub members: [(&'a str, &'a ExactRecordRef); 2],
}
struct DeclaredMember<'a> {
    reference: &'a ExactRecordRef,
    lexical: &'a str,
}
struct BoundManifestRequest<'a> {
    envelope: BoundEnvelopeRequest,
    declarations: [DeclaredMember<'a>; 2],
}

// The native evaluator already selected and validated this actual assertion.
// Checking only Match/NoMatch would fail to distinguish two disjoint source years.
fn check_declared_source(
    w: &mut Work,
    declarations: &[DeclaredMember<'_>; 2],
    member: &ExactRecordRef,
    source_revision: &JsonValue,
    result: &JsonValue,
) -> Result<()> {
    // FixedTwo N=2: two bounded declaration scans plus the shared49-field
    // native lexical tail (see General proof below). Tighten old undercharge;
    // public API/data/semantics stay unchanged, no old budget is refunded.
    w.visits
        .charge_original_visits(68)
        .map_err(|_| resource())?;
    let mut found = declarations.iter().filter(|d| d.reference == member);
    let declaration = found
        .next()
        .ok_or_else(|| corrupt("undeclared frozen member"))?;
    if found.next().is_some() {
        return Err(corrupt("ambiguous frozen declaration"));
    }
    check_declared_lexical(member, source_revision, result, declaration.lexical)
}
fn check_declared_lexical(
    member: &ExactRecordRef,
    source_revision: &JsonValue,
    result: &JsonValue,
    lexical: &str,
) -> Result<()> {
    let reference = field(result, "claim_ref")?;
    if text(reference, "id")? != member.id.as_str()
        || get(reference, "version").and_then(JsonValue::as_u64) != Some(member.version.get())
        || Digest256::from_hex(text(reference, "digest")?)
            .map_err(|_| corrupt("native member digest"))?
            != member.digest
        || field(result, "source_revision")? != source_revision
    {
        return Err(corrupt("native declaration provenance"));
    }
    let assertion = field(result, "source_assertion")?;
    let position = field(assertion, "position")?;
    if text(position, "lexical")? != lexical {
        return Err(invalid("DeclaredMemberAssertionMismatch"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn bind_manifest_envelope<'a>(
    w: &mut Work,
    loaded: &LoadedChronology<'_>,
    bound: &BoundCmpKnowledge<'_>,
    snapshot: &SourceSnapshot,
    input: &'a JsonValue,
    aliases: &ListManifestAliases<'a>,
    world: &JsonValue,
) -> Result<BoundManifestRequest<'a>> {
    const KEYS: &[&str] = &[
        "operation",
        "source_profile",
        "subject",
        "request_envelope",
        "snapshot_manifest",
        "claim_world_valid_times",
    ];
    exact_keys(input, KEYS)?;
    let manifest = get(input, "snapshot_manifest").ok_or_else(|| invalid("manifest required"))?;
    exact_keys(
        manifest,
        &["chronology_version_id", "frozen_knowledge_selection"],
    )?;
    let cv_symbol = envelope_text(manifest, "chronology_version_id")?;
    let selection = get(manifest, "frozen_knowledge_selection")
        .ok_or_else(|| invalid("manifest selection required"))?;
    exact_keys(
        selection,
        &[
            "system_state",
            "source_snapshot_ref",
            "source_revision_fence",
            "member_claim_revisions",
        ],
    )?;
    let system =
        get(selection, "system_state").ok_or_else(|| invalid("manifest system required"))?;
    exact_keys(system, &["type", "cutoff"])?;
    let system_type = envelope_text(system, "type")?;
    let cutoff = envelope_text(system, "cutoff")?;
    if cutoff.trim().is_empty() {
        return Err(invalid("manifest concrete cutoff required"));
    }
    let source = get(selection, "source_snapshot_ref")
        .ok_or_else(|| invalid("manifest snapshot required"))?;
    exact_keys(source, &["snapshot_id"])?;
    let snapshot_symbol = envelope_text(source, "snapshot_id")?;
    let fence_symbol = envelope_text(selection, "source_revision_fence")?;
    let members = get(selection, "member_claim_revisions")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid("manifest member array required"))?;
    if members.len() != 2 {
        return Err(invalid("two declared member symbols required"));
    }
    let member_symbol = |i: usize| {
        members[i]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("manifest member text required"))
    };
    let symbols = [member_symbol(0)?, member_symbol(1)?];
    if symbols[0] == symbols[1] {
        return Err(invalid("distinct declared member symbols required"));
    }
    let times = get(input, "claim_world_valid_times")
        .ok_or_else(|| invalid("declared world times required"))?;
    exact_keys(times, &symbols)?;
    let lexical = [
        envelope_text(times, symbols[0])?,
        envelope_text(times, symbols[1])?,
    ];
    // All full-manifest shape/types/keys have now been checked. The common
    // envelope validator checks every described envelope field before refusing
    // unsupported semantics; no principal/selector capability is added here.
    let envelope = bind_envelope_fields(
        w,
        loaded,
        bound,
        snapshot,
        input,
        &aliases.envelope,
        world,
        KEYS,
    )?;
    if system_type != "SystemAsOf" || cutoff == "Current" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedDeclaredSystemState",
        ));
    }
    let mut alias_bytes = 0usize;
    for value in [
        aliases.envelope.snapshot.0,
        aliases.envelope.snapshot.1.id.as_str(),
        aliases.envelope.revision_fence.0,
        aliases.envelope.chronology.0,
        aliases.envelope.chronology.1.id.as_str(),
        aliases.envelope.world_about.0,
        aliases.envelope.world_about.1.id.as_str(),
        aliases.envelope.world_trs.0,
        aliases.envelope.world_trs.1,
        aliases.envelope.world_trs.2,
        aliases.members[0].0,
        aliases.members[0].1.id.as_str(),
        aliases.members[1].0,
        aliases.members[1].1.id.as_str(),
    ] {
        if value.is_empty() || value.len() > 1024 {
            return Err(resource());
        }
        alias_bytes = alias_bytes.checked_add(value.len()).ok_or_else(resource)?;
    }
    if alias_bytes > 8192 {
        return Err(resource());
    }
    if cv_symbol != aliases.envelope.chronology.0
        || snapshot_symbol != aliases.envelope.snapshot.0
        || fence_symbol != aliases.envelope.revision_fence.0
        || symbols[0] != aliases.members[0].0
        || symbols[1] != aliases.members[1].0
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnresolvedManifestAlias",
        ));
    }
    let chronology = loaded.bound().chronology();
    let actual = &chronology.member_claim_revisions;
    w.visits
        .charge_original_visits(16)
        .map_err(|_| resource())?;
    if aliases.members[0].1.id == aliases.members[1].1.id
        || aliases.members[0].1 == aliases.members[1].1
        || actual.len() != 2
        || actual[0] != *aliases.members[0].1
        || actual[1] != *aliases.members[1].1
        || chronology.selection.system_as_of.value.as_str() != Some(cutoff)
        || chronology.selection.system_as_of.trs.is_some()
        || chronology.selection.attributed_knowledge.is_some()
        || chronology.selection.source_publication_as_of.is_some()
    {
        return Err(invalid("DeclaredManifestMismatch"));
    }
    Ok(BoundManifestRequest {
        envelope,
        declarations: [
            DeclaredMember {
                reference: aliases.members[0].1,
                lexical: lexical[0],
            },
            DeclaredMember {
                reference: aliases.members[1].1,
                lexical: lexical[1],
            },
        ],
    })
}

/// Bind the complete original59 two-member declaration to actual stored source.
/// Cutoff is source-declared, not authenticated physical clock evidence.
/// No entry-origin tag or new permission is added to the existing cursor.
#[allow(clippy::too_many_arguments)]
pub fn with_loaded_chronology_manifest_envelope<'owner, 'lookup, 'list, LA, MA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    list_authority: &mut MA,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ListManifestAliases<'_>,
    page_request: &JsonValue,
    world_supplement: &JsonValue,
    world_rules_context: &JsonValue,
    budget: StoredChronologyListBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut StoredChronologyListView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    MA: InspectCurrentAuthority<'list> + ?Sized,
{
    stop(deadline, cancelled)?;
    // Preserve the existing member/query/temporal admission, then reserve the
    // SAME parent's verifier portion while the prefix uses only the remainder.
    quotas(
        budget,
        loaded.bound().chronology().member_claim_revisions.len(),
        true,
    )?;
    let build_bytes = budget
        .max_extra_owned_bytes
        .checked_sub(budget.max_verification_owned_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let build_visits = budget
        .max_extra_json_visits
        .checked_sub(budget.max_verification_json_visits)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let mut limits = budget.query.json;
    limits.max_bytes = limits.max_bytes.min(INPUT_CAP);
    limits.max_depth = limits.max_depth.min(64);
    let mut prefix = Work {
        bytes: build_bytes,
        visits: InspectVisitMeter::new(build_visits),
        limits,
    };
    // Fixed private-plan/context/budget headers and both transport probe handles
    // are prepaid before construction/acquisition. Delegated owner internals
    // retain their existing bounds; this is not an RSS or allocator-peak claim.
    prefix.charge(
        std::mem::size_of::<BoundManifestRequest<'_>>()
            .checked_add(std::mem::size_of::<ManifestDeclarationView<'_>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<StoredChronologyListBudget>()))
            .and_then(|n| {
                n.checked_add(
                    2 * std::mem::size_of::<Option<std::sync::Arc<dyn tos_query::AbortProbe>>>(),
                )
            })
            .ok_or_else(resource)?,
    )?;
    let lookup_abort = lookup_authority.abort_probe();
    let list_abort = list_authority.abort_probe();
    for value in [input, page_request, world_supplement, world_rules_context] {
        stop(deadline, cancelled)?;
        envelope_probe(lookup_abort.as_deref())?;
        envelope_probe(list_abort.as_deref())?;
        // SourceCommandInputV1 is used only for bounded caller validation here;
        // no emitted envelope or alias digest is inserted into the cursor.
        prefix.canonical_input(value, INPUT_CAP)?;
    }
    let plan = bind_manifest_envelope(
        &mut prefix,
        &loaded,
        bound,
        snapshot,
        input,
        aliases,
        world_supplement,
    )?;
    stop(deadline, cancelled)?;
    envelope_probe(lookup_abort.as_deref())?;
    envelope_probe(list_abort.as_deref())?;
    let remaining = StoredChronologyListBudget {
        max_extra_owned_bytes: prefix
            .bytes
            .checked_add(budget.max_verification_owned_bytes)
            .ok_or_else(resource)?,
        max_extra_json_visits: prefix
            .visits
            .remaining()
            .checked_add(budget.max_verification_json_visits)
            .ok_or_else(resource)?,
        ..budget
    };
    // Do not refund the prefix's retained world clone, buffers or work even
    // when temporaries die. Inner Work starts with parent minus ALL charges.
    held_metadata_list(
        loaded,
        model,
        bound,
        lookup_authority,
        list_authority,
        snapshot,
        &plan.envelope.context,
        Some(world_rules_context),
        page_request,
        remaining,
        deadline,
        cancelled,
        Some(&ManifestDeclarationView::FixedTwo(&plan.declarations)),
        None,
        consume,
    )
}

/// Explicit symbols for this configured-command-owner subprotocol only.
/// Configured names are not human credentials or principal-specific list grants.
pub struct ConfiguredOwnerEnvelopeAliases<'a> {
    pub snapshot: (
        &'a str,
        &'a tos_foundation::query_context::SourceSnapshotRef,
    ),
    pub revision_fence: (&'a str, tos_foundation::SourceRevision),
    pub chronology: (&'a str, &'a ExactRecordRef),
    pub subject: (&'a str, &'a str),
    /// Symbol, exact configured principal_id, exact configured authority_ref.
    pub principal: (&'a str, &'a str, &'a str),
}
struct BoundConfiguredOwnerRequest {
    context: QueryContext,
}
fn bind_configured_owner_envelope(
    w: &mut Work,
    loaded: &LoadedChronology<'_>,
    bound: &BoundCmpKnowledge<'_>,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ConfiguredOwnerEnvelopeAliases<'_>,
    page_request: &JsonValue,
) -> Result<BoundConfiguredOwnerRequest> {
    exact_keys(
        input,
        &["operation", "source_profile", "subject", "request_envelope"],
    )
    .map_err(|_| invalid("ExpectedConfiguredOwnerEnvelopeSubprotocol"))?;
    let operation = envelope_text(input, "operation")?;
    let profile = envelope_text(input, "source_profile")?;
    let subject_symbol = envelope_text(input, "subject")?;
    let envelope = get(input, "request_envelope")
        .ok_or_else(|| invalid("configured owner envelope required"))?;
    exact_keys(
        envelope,
        &["source_snapshot_ref", "temporal", "principal_auth_context"],
    )?;
    let source =
        get(envelope, "source_snapshot_ref").ok_or_else(|| invalid("source snapshot required"))?;
    envelope_known(
        source,
        &["snapshot_id", "revision_fence", "resolved_system_cutoff"],
    )?;
    let snapshot_symbol = envelope_text(source, "snapshot_id")?;
    let fence_symbol = envelope_text(source, "revision_fence")?;
    let temporal = get(envelope, "temporal").ok_or_else(|| invalid("temporal required"))?;
    envelope_known(
        temporal,
        &[
            "chronology_version",
            "world_valid_time",
            "system_state",
            "attributed_knowledge_as_of",
            "source_publication_as_of",
            "display_perspective",
        ],
    )?;
    let chronology_symbol = envelope_text(temporal, "chronology_version")?;
    let principal = get(envelope, "principal_auth_context")
        .ok_or_else(|| invalid("configured principal context required"))?;
    exact_keys(principal, &["principal_ref", "authorization"])?;
    let principal_symbol = envelope_text(principal, "principal_ref")?;
    let authorization = envelope_text(principal, "authorization")?;
    // Validate all described fields before capability refusal. Unsupported selector
    // values remain opaque; there is no general selector or world parser here.
    for text in [
        operation,
        profile,
        subject_symbol,
        snapshot_symbol,
        fence_symbol,
        chronology_symbol,
        principal_symbol,
        authorization,
    ] {
        if text.len() > 1024 {
            return Err(resource());
        }
    }
    if operation != "list_claims" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeOperation",
        ));
    }
    if profile != "ToS-Temporal-Core-Contract-v1" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeProfile",
        ));
    }
    if authorization != "permitted" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedConfiguredOwnerAuthorization",
        ));
    }
    if get(source, "resolved_system_cutoff").is_some()
        || [
            "world_valid_time",
            "system_state",
            "attributed_knowledge_as_of",
            "source_publication_as_of",
            "display_perspective",
        ]
        .iter()
        .any(|key| get(temporal, key).is_some())
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedConfiguredOwnerEnvelopeSelector",
        ));
    }
    let mut alias_bytes = 0usize;
    for text in [
        aliases.snapshot.0,
        aliases.snapshot.1.id.as_str(),
        aliases.revision_fence.0,
        aliases.chronology.0,
        aliases.chronology.1.id.as_str(),
        aliases.subject.0,
        aliases.subject.1,
        aliases.principal.0,
        aliases.principal.1,
        aliases.principal.2,
    ] {
        if text.is_empty() || text.len() > 1024 {
            return Err(resource());
        }
        alias_bytes = alias_bytes.checked_add(text.len()).ok_or_else(resource)?;
    }
    if alias_bytes > 8192 {
        return Err(resource());
    }
    if snapshot_symbol != aliases.snapshot.0
        || fence_symbol != aliases.revision_fence.0
        || chronology_symbol != aliases.chronology.0
        || subject_symbol != aliases.subject.0
        || principal_symbol != aliases.principal.0
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnresolvedEnvelopeAlias",
        ));
    }
    let chronology = loaded.bound().chronology();
    if aliases.snapshot.1 != &snapshot.reference
        || aliases.revision_fence.1 != snapshot.reference.revision
        || aliases.chronology.1 != &chronology.reference
    {
        return Err(invalid("EnvelopeBindingMismatch"));
    }
    let frozen = &chronology.selection.source_snapshot;
    if snapshot.reference != *frozen {
        w.charge(
            std::mem::size_of::<Vec<MismatchDimension>>()
                + 2 * std::mem::size_of::<MismatchDimension>(),
        )?;
        let mut dimensions = Vec::with_capacity(2);
        if snapshot.reference.id != frozen.id {
            dimensions.push(MismatchDimension::SourceSnapshotRef);
        }
        if snapshot.reference.revision != frozen.revision {
            dimensions.push(MismatchDimension::SourceRevisionFence);
        }
        return Err(StoredChronologyListError::ContextMismatch(dimensions));
    }
    if Digest256::from_hex(bound.require_source_revision()?)? != frozen.revision.0 {
        return Err(StoredChronologyListError::WrongModelSource);
    }
    let filter = get(page_request, "filter").ok_or_else(|| invalid("metadata filter required"))?;
    if envelope_text(filter, "subject_ref")? != aliases.subject.1 {
        return Err(invalid("EnvelopeBindingMismatch"));
    }
    // The same retained Loaded is moved into the unchanged held operation next.
    // These child allowances are charged in full BEFORE its private parse and
    // never refunded; this comparison does not mint a current permission token.
    w.charge(262_144)?;
    w.visits
        .charge_original_visits(8192)
        .map_err(|_| resource())?;
    loaded.compare_retained_owner_identity(aliases.principal.1, aliases.principal.2, 262_144, 8192)
        .map_err(|error| match error {
            crate::temporal_chronology::ConfiguredOwnerIdentityError::Mismatch =>
                StoredChronologyListError::Owner(SourceCommandError::Denied("configured owner identity mismatch")),
            crate::temporal_chronology::ConfiguredOwnerIdentityError::Resource => resource(),
            crate::temporal_chronology::ConfiguredOwnerIdentityError::CorruptRetainedConfiguration =>
                StoredChronologyListError::Owner(SourceCommandError::Conflict("retained owner configuration contradiction")),
        })?;
    Ok(BoundConfiguredOwnerRequest {
        context: QueryContext::default(),
    })
}

/// Compare the configured command owner, then use the SAME genuine owner hold
/// and independent query holders. This does not authenticate a human/current
/// caller, establish a principal-specific ACL, or accept the full original45.
#[allow(clippy::too_many_arguments)]
pub fn with_loaded_chronology_configured_owner_envelope<'owner, 'lookup, 'list, LA, MA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    list_authority: &mut MA,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ConfiguredOwnerEnvelopeAliases<'_>,
    page_request: &JsonValue,
    budget: StoredChronologyListBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut StoredChronologyListView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    MA: InspectCurrentAuthority<'list> + ?Sized,
{
    stop(deadline, cancelled)?;
    quotas(
        budget,
        loaded.bound().chronology().member_claim_revisions.len(),
        false,
    )?;
    let build_bytes = budget
        .max_extra_owned_bytes
        .checked_sub(budget.max_verification_owned_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let build_visits = budget
        .max_extra_json_visits
        .checked_sub(budget.max_verification_json_visits)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let mut limits = budget.query.json;
    limits.max_bytes = limits.max_bytes.min(INPUT_CAP);
    limits.max_depth = limits.max_depth.min(64);
    let mut prefix = Work {
        bytes: build_bytes,
        visits: InspectVisitMeter::new(build_visits),
        limits,
    };
    prefix.charge(
        std::mem::size_of::<BoundConfiguredOwnerRequest>()
            .checked_add(std::mem::size_of::<StoredChronologyListBudget>())
            // Separate from the config child's own4096: fixed borrowed-slot,
            // shape lookup, error/result and default-context construction work.
            .and_then(|n| n.checked_add(4096))
            .and_then(|n| {
                n.checked_add(
                    2 * std::mem::size_of::<Option<std::sync::Arc<dyn tos_query::AbortProbe>>>(),
                )
            })
            .ok_or_else(resource)?,
    )?;
    let lookup_abort = lookup_authority.abort_probe();
    let list_abort = list_authority.abort_probe();
    for value in [input, page_request] {
        stop(deadline, cancelled)?;
        envelope_probe(lookup_abort.as_deref())?;
        envelope_probe(list_abort.as_deref())?;
        prefix.canonical_input(value, INPUT_CAP)?;
    }
    let plan = bind_configured_owner_envelope(
        &mut prefix,
        &loaded,
        bound,
        snapshot,
        input,
        aliases,
        page_request,
    )?;
    stop(deadline, cancelled)?;
    envelope_probe(lookup_abort.as_deref())?;
    envelope_probe(list_abort.as_deref())?;
    let remaining = StoredChronologyListBudget {
        max_extra_owned_bytes: prefix
            .bytes
            .checked_add(budget.max_verification_owned_bytes)
            .ok_or_else(resource)?,
        max_extra_json_visits: prefix
            .visits
            .remaining()
            .checked_add(budget.max_verification_json_visits)
            .ok_or_else(resource)?,
        ..budget
    };
    // No refund of the retained plan or consumed parse/canonical work. Default
    // requested context and None rules keep the original cursor tags unchanged.
    with_loaded_chronology_metadata_list(
        loaded,
        model,
        bound,
        lookup_authority,
        list_authority,
        snapshot,
        &plan.context,
        None,
        page_request,
        remaining,
        deadline,
        cancelled,
        consume,
    )
}

/// Finite metadata/world mappings. Neither variant creates owner or query rights.
pub enum ListManifestEnvelopeAliases<'a> {
    Metadata {
        snapshot: (
            &'a str,
            &'a tos_foundation::query_context::SourceSnapshotRef,
        ),
        revision_fence: (&'a str, tos_foundation::SourceRevision),
        chronology: (&'a str, &'a ExactRecordRef),
        subject: (&'a str, &'a str),
    },
    World(ListEnvelopeAliases<'a>),
}
pub struct ListManifestSliceAliases<'a> {
    pub envelope: ListManifestEnvelopeAliases<'a>,
    pub members: &'a [(&'a str, &'a ExactRecordRef)],
}
struct BorrowedManifestDeclarations<'a> {
    members: &'a [(&'a str, &'a ExactRecordRef)],
    times: &'a JsonValue,
}
enum ManifestDeclarationView<'a> {
    FixedTwo(&'a [DeclaredMember<'a>; 2]),
    General(&'a BorrowedManifestDeclarations<'a>),
}
struct BoundSliceManifestRequest<'a> {
    envelope: BoundEnvelopeRequest,
    declarations: Option<BorrowedManifestDeclarations<'a>>,
}
fn check_manifest_declaration(
    w: &mut Work,
    view: &ManifestDeclarationView<'_>,
    member: &ExactRecordRef,
    source_revision: &JsonValue,
    result: &JsonValue,
) -> Result<()> {
    match view {
        ManifestDeclarationView::FixedTwo(declarations) => {
            check_declared_source(w, declarations, member, source_revision, result)
        }
        ManifestDeclarationView::General(declarations) => {
            // Actual per-result work, separate from prepaid binder loops:
            // <=N alias probes +N time-map probes. Native total result is built
            // with claim_ref/source_revision/source_assertion at8/9/14. Its
            // ordered ref id/version/digest take1+2+3 field probes; the strict
            // source assertion/position have5/7 fields. Tail <=49 probes:
            // 8+1+2+3+9+14+5+7.64 also covers outcome/control probes. No
            // duplicate-key/binder slack is refunded for this actual lookup.
            w.visits
                .charge_original_visits(
                    declarations
                        .members
                        .len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(64))
                        .ok_or_else(resource)?,
                )
                .map_err(|_| resource())?;
            let mut found = declarations
                .members
                .iter()
                .filter(|(_, reference)| *reference == member);
            let (symbol, _) = found
                .next()
                .ok_or_else(|| corrupt("undeclared frozen member"))?;
            if found.next().is_some() {
                return Err(corrupt("ambiguous frozen declaration"));
            }
            let lexical = envelope_text(declarations.times, symbol)?;
            check_declared_lexical(member, source_revision, result, lexical)
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn bind_metadata_manifest_envelope(
    w: &mut Work,
    loaded: &LoadedChronology<'_>,
    bound: &BoundCmpKnowledge<'_>,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ListManifestEnvelopeAliases<'_>,
    page_request: &JsonValue,
) -> Result<BoundEnvelopeRequest> {
    let ListManifestEnvelopeAliases::Metadata {
        snapshot: snapshot_alias,
        revision_fence,
        chronology: chronology_alias,
        subject,
    } = aliases
    else {
        return Err(invalid("ManifestModeMismatch"));
    };
    let operation = envelope_text(input, "operation")?;
    let profile = envelope_text(input, "source_profile")?;
    let subject_symbol = envelope_text(input, "subject")?;
    let envelope =
        get(input, "request_envelope").ok_or_else(|| invalid("request envelope required"))?;
    exact_keys(envelope, &["source_snapshot_ref", "temporal"])?;
    let source =
        get(envelope, "source_snapshot_ref").ok_or_else(|| invalid("source snapshot required"))?;
    envelope_known(
        source,
        &["snapshot_id", "revision_fence", "resolved_system_cutoff"],
    )?;
    let snapshot_symbol = envelope_text(source, "snapshot_id")?;
    let fence_symbol = envelope_text(source, "revision_fence")?;
    let temporal = get(envelope, "temporal").ok_or_else(|| invalid("temporal required"))?;
    envelope_known(
        temporal,
        &[
            "chronology_version",
            "world_valid_time",
            "system_state",
            "attributed_knowledge_as_of",
            "source_publication_as_of",
            "display_perspective",
        ],
    )?;
    let chronology_symbol = envelope_text(temporal, "chronology_version")?;
    if get(temporal, "world_valid_time").is_some() {
        return Err(invalid("ManifestModeMismatch"));
    }
    if operation != "list_claims" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeOperation",
        ));
    }
    if profile != "ToS-Temporal-Core-Contract-v1" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeProfile",
        ));
    }
    if get(source, "resolved_system_cutoff").is_some()
        || [
            "system_state",
            "attributed_knowledge_as_of",
            "source_publication_as_of",
            "display_perspective",
        ]
        .iter()
        .any(|key| get(temporal, key).is_some())
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedEnvelopeSelector",
        ));
    }
    if snapshot_symbol != snapshot_alias.0
        || fence_symbol != revision_fence.0
        || chronology_symbol != chronology_alias.0
        || subject_symbol != subject.0
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnresolvedEnvelopeAlias",
        ));
    }
    let chronology = loaded.bound().chronology();
    if snapshot_alias.1 != &snapshot.reference
        || revision_fence.1 != snapshot.reference.revision
        || chronology_alias.1 != &chronology.reference
    {
        return Err(invalid("EnvelopeBindingMismatch"));
    }
    let frozen = &chronology.selection.source_snapshot;
    if snapshot.reference != *frozen {
        w.charge(
            std::mem::size_of::<Vec<MismatchDimension>>()
                + 2 * std::mem::size_of::<MismatchDimension>(),
        )?;
        let mut dimensions = Vec::with_capacity(2);
        if snapshot.reference.id != frozen.id {
            dimensions.push(MismatchDimension::SourceSnapshotRef);
        }
        if snapshot.reference.revision != frozen.revision {
            dimensions.push(MismatchDimension::SourceRevisionFence);
        }
        return Err(StoredChronologyListError::ContextMismatch(dimensions));
    }
    if Digest256::from_hex(bound.require_source_revision()?)? != frozen.revision.0 {
        return Err(StoredChronologyListError::WrongModelSource);
    }
    let filter = get(page_request, "filter").ok_or_else(|| invalid("metadata filter required"))?;
    if envelope_text(filter, "subject_ref")? != subject.1 {
        return Err(invalid("EnvelopeBindingMismatch"));
    }
    Ok(BoundEnvelopeRequest {
        context: QueryContext::default(),
    })
}
#[allow(clippy::too_many_arguments)]
fn bind_slice_manifest_envelope<'a>(
    w: &mut Work,
    loaded: &LoadedChronology<'_>,
    bound: &BoundCmpKnowledge<'_>,
    snapshot: &SourceSnapshot,
    input: &'a JsonValue,
    aliases: &ListManifestSliceAliases<'a>,
    page_request: &JsonValue,
    world: Option<&JsonValue>,
) -> Result<BoundSliceManifestRequest<'a>> {
    let world_mode = matches!(&aliases.envelope, ListManifestEnvelopeAliases::World(_));
    let metadata_keys = [
        "operation",
        "source_profile",
        "subject",
        "request_envelope",
        "snapshot_manifest",
    ];
    let world_keys = [
        "operation",
        "source_profile",
        "subject",
        "request_envelope",
        "snapshot_manifest",
        "claim_world_valid_times",
    ];
    exact_keys(
        input,
        if world_mode {
            &world_keys
        } else {
            &metadata_keys
        },
    )?;
    let manifest = get(input, "snapshot_manifest").ok_or_else(|| invalid("manifest required"))?;
    exact_keys(
        manifest,
        &["chronology_version_id", "frozen_knowledge_selection"],
    )?;
    let cv_symbol = envelope_text(manifest, "chronology_version_id")?;
    let selection = get(manifest, "frozen_knowledge_selection")
        .ok_or_else(|| invalid("manifest selection required"))?;
    exact_keys(
        selection,
        &[
            "system_state",
            "source_snapshot_ref",
            "source_revision_fence",
            "member_claim_revisions",
        ],
    )?;
    let system =
        get(selection, "system_state").ok_or_else(|| invalid("manifest system required"))?;
    exact_keys(system, &["type", "cutoff"])?;
    let system_type = envelope_text(system, "type")?;
    let cutoff = envelope_text(system, "cutoff")?;
    if cutoff.trim().is_empty() {
        return Err(invalid("manifest concrete cutoff required"));
    }
    let source = get(selection, "source_snapshot_ref")
        .ok_or_else(|| invalid("manifest snapshot required"))?;
    exact_keys(source, &["snapshot_id"])?;
    let snapshot_symbol = envelope_text(source, "snapshot_id")?;
    let fence_symbol = envelope_text(selection, "source_revision_fence")?;
    let members = get(selection, "member_claim_revisions")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid("manifest member array required"))?;
    let actual = &loaded.bound().chronology().member_claim_revisions;
    // Pre-admit the maximum observed cardinality BEFORE any dynamic scan. This
    // includes malformed-but-bounded declared counts, not only Loaded's N.
    let n = actual.len().max(members.len()).max(aliases.members.len());
    if n == 0 || n > 128 {
        return Err(resource());
    }
    let visits = n
        .checked_mul(n)
        .and_then(|v| v.checked_mul(8))
        .and_then(|v| n.checked_mul(32).and_then(|linear| v.checked_add(linear)))
        .and_then(|v| v.checked_add(128))
        .ok_or_else(resource)?;
    w.visits
        .charge_original_visits(visits)
        .map_err(|_| resource())?;
    for (i, symbol) in members.iter().enumerate() {
        let symbol = symbol
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("manifest member text required"))?;
        if symbol.len() > 1024 {
            return Err(resource());
        }
        if members[..i].iter().any(|old| old.as_str() == Some(symbol)) {
            return Err(invalid("distinct declared member symbols required"));
        }
    }
    let times = if world_mode {
        let times = get(input, "claim_world_valid_times")
            .ok_or_else(|| invalid("declared world times required"))?;
        let fields = times
            .as_object()
            .ok_or_else(|| invalid("declared world times object required"))?;
        if fields.len() != members.len() {
            return Err(invalid("declared world time count"));
        }
        for (i, (key, value)) in fields.iter().enumerate() {
            let key = key
                .as_str()
                .ok_or_else(|| invalid("world time text key required"))?;
            if fields[..i].iter().any(|(old, _)| old.as_str() == Some(key))
                || !members.iter().any(|symbol| symbol.as_str() == Some(key))
                || !value.as_str().is_some_and(|s| !s.is_empty())
            {
                return Err(invalid("declared world time keys or text"));
            }
        }
        Some(times)
    } else {
        None
    };
    // General modes admit no principal wrapper: that separate owner entry is
    // not silently composed here. Keep fixed-two's old grammar unchanged.
    let request_envelope =
        get(input, "request_envelope").ok_or_else(|| invalid("request envelope required"))?;
    exact_keys(request_envelope, &["source_snapshot_ref", "temporal"])?;
    if world_mode {
        let temporal =
            get(request_envelope, "temporal").ok_or_else(|| invalid("temporal required"))?;
        if get(temporal, "world_valid_time").is_none() {
            return Err(invalid("ManifestModeMismatch"));
        }
    }
    // Validate all manifest shape before common envelope capability refusals.
    let envelope = match (&aliases.envelope, world) {
        (ListManifestEnvelopeAliases::World(base), Some(world)) => {
            bind_envelope_fields(w, loaded, bound, snapshot, input, base, world, &world_keys)?
        }
        (ListManifestEnvelopeAliases::Metadata { .. }, None) => bind_metadata_manifest_envelope(
            w,
            loaded,
            bound,
            snapshot,
            input,
            &aliases.envelope,
            page_request,
        )?,
        _ => return Err(invalid("ManifestModeMismatch")),
    };
    if system_type != "SystemAsOf" || cutoff == "Current" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedDeclaredSystemState",
        ));
    }
    let mut alias_bytes = 0usize;
    let mut admit_text = |text: &str| -> Result<()> {
        if text.is_empty() || text.len() > 1024 {
            return Err(resource());
        }
        alias_bytes = alias_bytes.checked_add(text.len()).ok_or_else(resource)?;
        Ok(())
    };
    let (snapshot_alias, fence_alias, cv_alias) = match &aliases.envelope {
        ListManifestEnvelopeAliases::World(base) => {
            for text in [
                base.snapshot.0,
                base.snapshot.1.id.as_str(),
                base.revision_fence.0,
                base.chronology.0,
                base.chronology.1.id.as_str(),
                base.world_about.0,
                base.world_about.1.id.as_str(),
                base.world_trs.0,
                base.world_trs.1,
                base.world_trs.2,
            ] {
                admit_text(text)?;
            }
            (base.snapshot.0, base.revision_fence.0, base.chronology.0)
        }
        ListManifestEnvelopeAliases::Metadata {
            snapshot,
            revision_fence,
            chronology,
            subject,
        } => {
            for text in [
                snapshot.0,
                snapshot.1.id.as_str(),
                revision_fence.0,
                chronology.0,
                chronology.1.id.as_str(),
                subject.0,
                subject.1,
            ] {
                admit_text(text)?;
            }
            (snapshot.0, revision_fence.0, chronology.0)
        }
    };
    for (i, (symbol, reference)) in aliases.members.iter().enumerate() {
        admit_text(symbol)?;
        admit_text(reference.id.as_str())?;
        if aliases.members[..i]
            .iter()
            .any(|(previous, old)| previous == symbol || old.id == reference.id)
        {
            return Err(invalid("distinct manifest aliases and StableIds required"));
        }
    }
    if alias_bytes > 8192 {
        return Err(resource());
    }
    if cv_symbol != cv_alias || snapshot_symbol != snapshot_alias || fence_symbol != fence_alias {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnresolvedManifestAlias",
        ));
    }
    if members.len() != actual.len() || aliases.members.len() != actual.len() {
        return Err(invalid("DeclaredManifestMismatch"));
    }
    for (i, member) in actual.iter().enumerate() {
        let (symbol, reference) = aliases.members[i];
        if members[i].as_str() != Some(symbol) {
            return Err(StoredChronologyListError::NeedsBinding(
                "UnresolvedManifestAlias",
            ));
        }
        if reference != member {
            return Err(invalid("DeclaredManifestMismatch"));
        }
    }
    let chronology = loaded.bound().chronology();
    if chronology.selection.system_as_of.value.as_str() != Some(cutoff)
        || chronology.selection.system_as_of.trs.is_some()
        || chronology.selection.attributed_knowledge.is_some()
        || chronology.selection.source_publication_as_of.is_some()
    {
        return Err(invalid("DeclaredManifestMismatch"));
    }
    Ok(BoundSliceManifestRequest {
        envelope,
        declarations: times.map(|times| BorrowedManifestDeclarations {
            members: aliases.members,
            times,
        }),
    })
}

/// Cardinality-general, capacity-conditional full supported manifest binding.
/// No new authority, policy or clock admission; fixed-two entry remains separate.
#[allow(clippy::too_many_arguments)]
pub fn with_loaded_chronology_manifest_slice_envelope<'owner, 'lookup, 'list, LA, MA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    list_authority: &mut MA,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    aliases: &ListManifestSliceAliases<'_>,
    page_request: &JsonValue,
    world_supplement: Option<&JsonValue>,
    world_rules_context: Option<&JsonValue>,
    budget: StoredChronologyListBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut StoredChronologyListView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    MA: InspectCurrentAuthority<'list> + ?Sized,
{
    stop(deadline, cancelled)?;
    let world_mode = matches!(&aliases.envelope, ListManifestEnvelopeAliases::World(_));
    quotas(
        budget,
        loaded.bound().chronology().member_claim_revisions.len(),
        world_mode,
    )?;
    let bytes = budget
        .max_extra_owned_bytes
        .checked_sub(budget.max_verification_owned_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let visits = budget
        .max_extra_json_visits
        .checked_sub(budget.max_verification_json_visits)
        .filter(|n| *n > 0)
        .ok_or_else(resource)?;
    let mut limits = budget.query.json;
    limits.max_bytes = limits.max_bytes.min(INPUT_CAP);
    limits.max_depth = limits.max_depth.min(64);
    let mut prefix = Work {
        bytes,
        visits: InspectVisitMeter::new(visits),
        limits,
    };
    // This mandatory fixed-header charge precedes even abort-probe acquisition
    // and input canonicalization. All owned/borrowed private view headers live
    // within this reservation; prior Loaded/caller parsing retains its own bounds.
    prefix.charge(
        std::mem::size_of::<BoundSliceManifestRequest<'_>>()
            .checked_add(std::mem::size_of::<ManifestDeclarationView<'_>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<StoredChronologyListBudget>()))
            .and_then(|n| n.checked_add(4096))
            .and_then(|n| {
                n.checked_add(
                    2 * std::mem::size_of::<Option<std::sync::Arc<dyn tos_query::AbortProbe>>>(),
                )
            })
            .ok_or_else(resource)?,
    )?;
    let lookup_abort = lookup_authority.abort_probe();
    let list_abort = list_authority.abort_probe();
    if world_mode != world_supplement.is_some() || world_mode != world_rules_context.is_some() {
        return Err(invalid("ManifestModeMismatch"));
    }
    for value in [
        Some(input),
        Some(page_request),
        world_supplement,
        world_rules_context,
    ]
    .into_iter()
    .flatten()
    {
        stop(deadline, cancelled)?;
        envelope_probe(lookup_abort.as_deref())?;
        envelope_probe(list_abort.as_deref())?;
        prefix.canonical_input(value, INPUT_CAP)?;
    }
    let plan = bind_slice_manifest_envelope(
        &mut prefix,
        &loaded,
        bound,
        snapshot,
        input,
        aliases,
        page_request,
        world_supplement,
    )?;
    stop(deadline, cancelled)?;
    envelope_probe(lookup_abort.as_deref())?;
    envelope_probe(list_abort.as_deref())?;
    let remaining = StoredChronologyListBudget {
        max_extra_owned_bytes: prefix
            .bytes
            .checked_add(budget.max_verification_owned_bytes)
            .ok_or_else(resource)?,
        max_extra_json_visits: prefix
            .visits
            .remaining()
            .checked_add(budget.max_verification_json_visits)
            .ok_or_else(resource)?,
        ..budget
    };
    let declarations = plan
        .declarations
        .as_ref()
        .map(ManifestDeclarationView::General);
    held_metadata_list(
        loaded,
        model,
        bound,
        lookup_authority,
        list_authority,
        snapshot,
        &plan.envelope.context,
        world_rules_context,
        page_request,
        remaining,
        deadline,
        cancelled,
        declarations.as_ref(),
        None,
        consume,
    )
}

/// Borrowed explicit source symbols, never credentials or assessed knowledge.
/// Both target/witness pairs must cover the complete four-member custody CV.
pub struct AttributedCutoffBindings<'a> {
    pub snapshot: (
        &'a str,
        &'a tos_foundation::query_context::SourceSnapshotRef,
    ),
    pub revision_fence: (&'a str, tos_foundation::SourceRevision),
    pub knower: (&'a str, &'a JsonValue),
    pub trs: (&'a str, &'a JsonValue),
    pub targets: [(&'a str, &'a ExactRecordRef, &'a ExactRecordRef); 2],
    pub answer_field: &'a str,
}
struct AttributedPredicate<'a> {
    work: Work,
    bindings: &'a AttributedCutoffBindings<'a>,
    original: &'a JsonValue,
    requested_cutoff: &'a str,
    rows: [&'a JsonValue; 2],
    knower_ref: ExactRecordRef,
    trs_ref: ExactRecordRef,
    answers: [Option<bool>; 2],
    details: [Option<JsonValue>; 2],
    reasons: [Option<&'static str>; 2],
}
// ALL new grammar/source lookups meter every examined field, not only matches.
fn ag_get<'a>(w: &mut Work, v: &'a JsonValue, key: &str) -> Result<Option<&'a JsonValue>> {
    let Some(fields) = v.as_object() else {
        return Ok(None);
    };
    let mut found = None;
    for (name, value) in fields {
        w.visits.charge_original_visits(1).map_err(|_| resource())?;
        if name.as_str() == Some(key) {
            if found.is_some() {
                return Err(invalid("duplicate attributed field"));
            }
            found = Some(value);
        }
    }
    Ok(found)
}
fn ag_field<'a>(w: &mut Work, v: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    ag_get(w, v, key)?.ok_or_else(|| invalid("missing attributed field"))
}
fn ag_text<'a>(w: &mut Work, v: &'a JsonValue, key: &str) -> Result<&'a str> {
    ag_field(w, v, key)?
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .ok_or_else(|| invalid("bounded attributed text required"))
}
fn ag_shape(w: &mut Work, v: &JsonValue, keys: &[&str]) -> Result<()> {
    let fields = v
        .as_object()
        .ok_or_else(|| invalid("attributed object required"))?;
    if fields.len() != keys.len() {
        return Err(invalid("attributed object keys"));
    }
    for (i, (key, _)) in fields.iter().enumerate() {
        let key = key.as_str().ok_or_else(|| invalid("attributed key text"))?;
        let mut known = false;
        for expected in keys {
            w.visits.charge_original_visits(1).map_err(|_| resource())?;
            known |= key == *expected;
        }
        if !known {
            return Err(invalid("unknown attributed key"));
        }
        for (old, _) in &fields[..i] {
            w.visits.charge_original_visits(1).map_err(|_| resource())?;
            if old.as_str() == Some(key) {
                return Err(invalid("duplicate attributed key"));
            }
        }
    }
    Ok(())
}
fn ag_equal(w: &mut Work, a: &JsonValue, b: &JsonValue, cap: usize) -> Result<bool> {
    Ok(w.canonical(a, cap)? == w.canonical(b, cap)?)
}
fn ag_ref(w: &mut Work, v: &JsonValue) -> Result<ExactRecordRef> {
    ag_shape(w, v, &["id", "version", "digest"])?;
    let id = ag_text(w, v, "id")?;
    let version = ag_field(w, v, "version")?
        .as_u64()
        .ok_or_else(|| invalid("reference version"))?;
    let digest = ag_text(w, v, "digest")?;
    w.charge(
        id.len()
            .checked_add(std::mem::size_of::<ExactRecordRef>())
            .ok_or_else(resource)?,
    )?;
    Ok(ExactRecordRef {
        id: tos_foundation::StableId::parse(id).map_err(|_| invalid("reference id"))?,
        version: tos_foundation::RecordVersion::new(version)
            .map_err(|_| invalid("reference version"))?,
        digest: Digest256::from_prefixed(digest).map_err(|_| invalid("reference digest"))?,
    })
}
fn ag_definition(w: &mut Work, value: &JsonValue, knower: bool) -> Result<ExactRecordRef> {
    if knower {
        ag_shape(
            w,
            value,
            &[
                "schema_version",
                "record_id",
                "record_version",
                "kind",
                "label",
            ],
        )?;
        if ag_text(w, value, "schema_version")? != "tos_source_declared_knower_v1"
            || ag_text(w, value, "kind")? != "SourceDeclaredKnower"
        {
            return Err(invalid("knower definition profile"));
        }
        ag_text(w, value, "label")?;
    } else {
        ag_shape(
            w,
            value,
            &[
                "schema_version",
                "record_id",
                "record_version",
                "trs",
                "calendar_algorithm",
                "year_numbering",
                "rules_version",
                "unit",
                "origin",
            ],
        )?;
        if ag_text(w, value, "schema_version")? != "tos_source_trs_definition_v1" {
            return Err(invalid("TRS definition profile"));
        }
        let trs = ag_field(w, value, "trs")?;
        ag_shape(w, trs, &["id", "version"])?;
        ag_text(w, trs, "id")?;
        ag_text(w, trs, "version")?;
        for (key, expected) in [
            ("calendar_algorithm", "GregorianProleptic"),
            ("year_numbering", "astronomical"),
            ("unit", "civil-year"),
            ("origin", "astronomical-year-number"),
        ] {
            if ag_text(w, value, key)? != expected {
                return Err(invalid("unsupported attributed TRS"));
            }
        }
        ag_text(w, value, "rules_version")?;
    }
    let id = ag_text(w, value, "record_id")?;
    let version = ag_field(w, value, "record_version")?
        .as_u64()
        .ok_or_else(|| invalid("definition version"))?;
    w.charge(
        id.len()
            .checked_add(std::mem::size_of::<ExactRecordRef>())
            .ok_or_else(resource)?,
    )?;
    // No stored native K/R record is invented: hash the entire inline definition.
    let raw = w.canonical_input(value, 1024)?;
    Ok(ExactRecordRef {
        id: tos_foundation::StableId::parse(id).map_err(|_| invalid("definition id"))?,
        version: tos_foundation::RecordVersion::new(version)
            .map_err(|_| invalid("definition version"))?,
        digest: Digest256::of_bytes(&raw),
    })
}
impl AttributedPredicate<'_> {
    fn observe(&mut self, member: &ExactRecordRef, carrier: &JsonValue) -> Result<()> {
        self.work
            .visits
            .charge_original_visits(2)
            .map_err(|_| resource())?;
        let Some(index) = self
            .bindings
            .targets
            .iter()
            .position(|(_, _, d)| *d == member)
        else {
            return Ok(());
        };
        if self.answers[index].is_some() || self.reasons[index].is_some() {
            self.reasons[index] = Some("AmbiguousDeclaration");
            return Ok(());
        }
        // Structural/semantic source failures become NonTotal, but resource and
        // already-established physical corruption retain their native domains.
        match self.declaration(index, carrier) {
            Ok((answer, details)) => {
                self.answers[index] = Some(answer);
                self.details[index] = Some(details);
            }
            Err(StoredChronologyListError::InvalidRequest(_)) => {
                self.reasons[index] = Some("UnsupportedOrMismatchedDeclaration")
            }
            Err(other) => return Err(other),
        }
        Ok(())
    }
    fn declaration(&mut self, i: usize, carrier: &JsonValue) -> Result<(bool, JsonValue)> {
        let w = &mut self.work;
        let attributes = ag_field(w, carrier, "attributes")?;
        let source = ag_field(w, attributes, "source_claim")?;
        if source.as_object().is_none_or(|v| v.len() > 32) {
            return Err(invalid("source root cardinality"));
        }
        let qualifiers = ag_field(w, source, "qualifiers")?;
        if qualifiers.as_object().is_none_or(|v| v.len() > 8) {
            return Err(invalid("qualifier cardinality"));
        }
        let d = ag_field(w, qualifiers, "attributed_knowledge_by_cutoff")?;
        w.canonical(d, 4096)?;
        ag_shape(
            w,
            d,
            &[
                "profile_ref",
                "knower_ref",
                "about_ref",
                "cutoff",
                "answer",
                "basis",
                "declared_ingest",
                "knower_definition",
                "trs_definition",
            ],
        )?;
        if ag_text(w, d, "profile_ref")? != "tos_attributed_knowledge_by_cutoff_assertion_v1" {
            return Err(invalid("declaration profile"));
        }
        let k = ag_reference_field(w, d, "knower_ref")?;
        let about = ag_reference_field(w, d, "about_ref")?;
        let cutoff = ag_field(w, d, "cutoff")?;
        ag_shape(w, cutoff, &["value", "trs"])?;
        let r = ag_reference_field(w, cutoff, "trs")?;
        if k != self.knower_ref
            || r != self.trs_ref
            || about != *self.bindings.targets[i].1
            || ag_text(w, cutoff, "value")? != self.requested_cutoff
        {
            return Err(invalid("declaration predicate mismatch"));
        }
        for (key, expected, definition) in [
            (
                "knower_definition",
                &self.knower_ref,
                self.bindings.knower.1,
            ),
            ("trs_definition", &self.trs_ref, self.bindings.trs.1),
        ] {
            let envelope = ag_field(w, d, key)?;
            ag_shape(w, envelope, &["reference", "definition", "scope"])?;
            let actual = ag_reference_field(w, envelope, "reference")?;
            if &actual != expected
                || ag_text(w, envelope, "scope")? != "InlineSourceDefinitionNotAuthorization"
                || !ag_equal_field(w, envelope, "definition", definition, 1024)?
            {
                return Err(invalid("source definition mismatch"));
            }
        }
        let answer = ag_field(w, d, "answer")?
            .as_bool()
            .ok_or_else(|| invalid("Boolean answer required"))?;
        let expected = ag_field(w, self.rows[i], self.bindings.answer_field)?
            .as_bool()
            .ok_or_else(|| invalid("Boolean request answer"))?;
        let ingest = ag_field(w, d, "declared_ingest")?;
        ag_shape(w, ingest, &["lexical", "scope"])?;
        if answer != expected
            || ag_text(w, ingest, "lexical")? != ag_text(w, self.rows[i], "system_ingested_at")?
            || ag_text(w, ingest, "scope")? != "SourceDeclaredNotOwnerTransactionTime"
        {
            return Err(invalid("source assertion differs from original row"));
        }
        let basis = ag_field(w, d, "basis")?;
        ag_shape(w, basis, &["source", "source_locator", "raw_evidence"])?;
        ag_reference_field(w, basis, "source")?;
        let locator = ag_field(w, basis, "source_locator")?;
        if !locator.is_null() && !locator.as_str().is_some_and(|v| v.len() <= 1024) {
            return Err(invalid("source locator"));
        }
        if !ag_field(w, basis, "raw_evidence")?
            .as_str()
            .is_some_and(|v| !v.is_empty() && v.len() <= 2048)
        {
            return Err(invalid("source evidence"));
        }
        let details = w.object(|w| {
            Ok([
                ("symbol", w.string(self.bindings.targets[i].0)?),
                ("target_ref", w.ref_wire(self.bindings.targets[i].1)?),
                ("declaration_ref", w.ref_wire(self.bindings.targets[i].2)?),
                (
                    "profile_ref",
                    w.string("tos_attributed_knowledge_by_cutoff_assertion_v1")?,
                ),
                ("knower_ref", w.ref_wire(&self.knower_ref)?),
                ("about_ref", w.ref_wire(self.bindings.targets[i].1)?),
                ("cutoff", w.clone_value(cutoff)?),
                ("basis", w.clone_value(basis)?),
                ("answer", JsonValue::Bool(answer)),
            ])
        })?;
        Ok((answer, details))
    }
    fn output(
        &mut self,
        chronology: &tos_foundation::query_context::ChronologyVersion,
        revision: tos_foundation::SourceRevision,
        receipt: &str,
        effective: &JsonValue,
        seen: &[bool],
    ) -> Result<JsonValue> {
        let w = &mut self.work;
        let complete = seen.len() == 4 && seen.iter().all(|v| *v);
        let total = complete
            && self.reasons.iter().all(Option::is_none)
            && self.answers.iter().all(Option::is_some);
        let mut selected = w.vector::<JsonValue>(2)?;
        let mut excluded = w.vector::<JsonValue>(2)?;
        let mut details = w.vector::<JsonValue>(2)?;
        let mut ingests = w.vector::<JsonValue>(2)?;
        let mut refusals = w.vector::<JsonValue>(2)?;
        let mut custody = w.vector::<JsonValue>(4)?;
        for member in &chronology.member_claim_revisions {
            custody.push(w.ref_wire(member)?);
        }
        for i in 0..2 {
            if total {
                let pair = w.object(|w| {
                    Ok([
                        ("symbol", w.string(self.bindings.targets[i].0)?),
                        ("target_ref", w.ref_wire(self.bindings.targets[i].1)?),
                        ("declaration_ref", w.ref_wire(self.bindings.targets[i].2)?),
                    ])
                })?;
                if self.answers[i] == Some(true) {
                    selected.push(pair);
                } else {
                    excluded.push(pair);
                }
            } else {
                let reason = if !complete {
                    "IncompleteProviderCoverage"
                } else {
                    self.reasons[i].unwrap_or("IncompleteDeclarationSet")
                };
                refusals.push(w.object(|w| {
                    Ok([
                        ("target_ref", w.ref_wire(self.bindings.targets[i].1)?),
                        ("declaration_ref", w.ref_wire(self.bindings.targets[i].2)?),
                        ("reason", w.string(reason)?),
                    ])
                })?);
            }
            if let Some(detail) = self.details[i].take() {
                details.push(detail);
            }
            if self.answers[i].is_some() && self.reasons[i].is_none() {
                let ingest_lexical = ag_text(w, self.rows[i], "system_ingested_at")?;
                ingests.push(w.object(|w| {
                    Ok([
                        ("target_ref", w.ref_wire(self.bindings.targets[i].1)?),
                        ("lexical", w.string(ingest_lexical)?),
                        ("scope", w.string("SourceDeclaredNotOwnerTransactionTime")?),
                    ])
                })?);
            }
        }
        w.object(|w| {
            Ok([
                (
                    "schema_version",
                    w.string("tos_stored_attributed_knowledge_by_cutoff_result_v1")?,
                ),
                ("scope", w.string("SupplementalSourceDeclarationsOnly")?),
                (
                    "outcome",
                    w.string(if total { "Resolved" } else { "NonTotalRefusal" })?,
                ),
                ("source_revision", w.string(&revision.0.to_prefixed())?),
                ("selected_model_receipt_id", w.string(receipt)?),
                ("chronology_ref", w.ref_wire(&chronology.reference)?),
                ("custody_members", JsonValue::Array(custody)),
                ("effective_context", w.clone_value(effective)?),
                ("original_request", w.clone_value(self.original)?),
                ("selected_targets", JsonValue::Array(selected)),
                ("excluded_targets", JsonValue::Array(excluded)),
                ("declaration_bindings", JsonValue::Array(details)),
                ("declared_ingest", JsonValue::Array(ingests)),
                ("refusals", JsonValue::Array(refusals)),
            ])
        })
    }
}

/// Source-attributed Boolean research predicate, with no world-time inference,
/// clock admission, credential or reusable permission token.
#[allow(clippy::too_many_arguments)]
pub fn with_loaded_chronology_attributed_knowledge_by_cutoff<'owner, 'lookup, 'list, LA, MA, T>(
    loaded: LoadedChronology<'owner>,
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    lookup_authority: &mut LA,
    list_authority: &mut MA,
    snapshot: &SourceSnapshot,
    input: &JsonValue,
    bindings: &AttributedCutoffBindings<'_>,
    page_request: &JsonValue,
    budget: StoredChronologyListBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'view> FnOnce(&mut StoredChronologyListView<'view>) -> Result<T>,
) -> Result<T>
where
    LA: InspectCurrentAuthority<'lookup> + ?Sized,
    MA: InspectCurrentAuthority<'list> + ?Sized,
{
    stop(deadline, cancelled)?;
    let (q, _, _, _) = quotas(budget, 4, false)?;
    let mut remaining = budget;
    remaining.max_extra_owned_bytes = budget
        .max_extra_owned_bytes
        .checked_sub(1_048_576)
        .filter(|n| *n > budget.max_verification_owned_bytes)
        .ok_or_else(resource)?;
    remaining.max_extra_json_visits = budget
        .max_extra_json_visits
        .checked_sub(32_768)
        .filter(|n| *n > budget.max_verification_json_visits)
        .ok_or_else(resource)?;
    let mut limits = budget.query.json;
    limits.max_bytes = 2048;
    limits.max_depth = 16;
    limits.max_visits = 32768;
    let mut w = Work {
        bytes: 1_048_576,
        visits: InspectVisitMeter::new(32768),
        limits,
    };
    w.charge(16_384)?;
    let lookup_abort = lookup_authority.abort_probe();
    let list_abort = list_authority.abort_probe();
    envelope_probe(lookup_abort.as_deref())?;
    envelope_probe(list_abort.as_deref())?;
    w.canonical_input(input, 2048)?;
    ag_shape(
        &mut w,
        input,
        &["operation", "source_profile", "input", "context"],
    )?;
    if ag_text(&mut w, input, "operation")? != "resolve_temporal_query_context"
        || ag_text(&mut w, input, "source_profile")? != "ToS-Temporal-Core-Contract-v1"
    {
        return Err(invalid("attributed operation/profile"));
    }
    let caller_context = ag_field(&mut w, input, "context")?;
    ag_shape(&mut w, caller_context, &[])?;
    let body = ag_field(&mut w, input, "input")?;
    ag_shape(&mut w, body, &["request_envelope", "source_claims"])?;
    let envelope = ag_field(&mut w, body, "request_envelope")?;
    ag_shape(&mut w, envelope, &["source_snapshot_ref", "temporal"])?;
    let source = ag_field(&mut w, envelope, "source_snapshot_ref")?;
    ag_shape(
        &mut w,
        source,
        &["snapshot_id", "resolved_system_cutoff", "revision_fence"],
    )?;
    let snapshot_symbol = ag_text(&mut w, source, "snapshot_id")?;
    let cutoff_lexical = ag_text(&mut w, source, "resolved_system_cutoff")?;
    let fence_symbol = ag_text(&mut w, source, "revision_fence")?;
    let temporal = ag_field(&mut w, envelope, "temporal")?;
    ag_shape(
        &mut w,
        temporal,
        &["system_state", "attributed_knowledge_as_of"],
    )?;
    let system = ag_text(&mut w, temporal, "system_state")?;
    let requested = ag_field(&mut w, temporal, "attributed_knowledge_as_of")?;
    ag_shape(&mut w, requested, &["knower", "cutoff", "trs_ref"])?;
    let knower_symbol = ag_text(&mut w, requested, "knower")?;
    let knowledge_cutoff = ag_text(&mut w, requested, "cutoff")?;
    let trs_symbol = ag_text(&mut w, requested, "trs_ref")?;
    for symbol in [
        bindings.snapshot.0,
        bindings.snapshot.1.id.as_str(),
        bindings.revision_fence.0,
        bindings.knower.0,
        bindings.trs.0,
    ] {
        if symbol.is_empty() || symbol.len() > 128 {
            return Err(resource());
        }
    }
    if bindings.answer_field.is_empty() || bindings.answer_field.len() > 128 {
        return Err(resource());
    }
    let rows = ag_field(&mut w, body, "source_claims")?
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| invalid("two original target rows required"))?;
    for row in rows {
        ag_shape(
            &mut w,
            row,
            &["id", "system_ingested_at", bindings.answer_field],
        )?;
        ag_text(&mut w, row, "id")?;
        ag_text(&mut w, row, "system_ingested_at")?;
        if ag_field(&mut w, row, bindings.answer_field)?
            .as_bool()
            .is_none()
        {
            return Err(invalid("original Boolean answer required"));
        }
    }
    for (symbol, target, witness) in &bindings.targets {
        for text in [*symbol, target.id.as_str(), witness.id.as_str()] {
            if text.is_empty() || text.len() > 128 {
                return Err(resource());
            }
        }
    }
    // Two distinct source symbols must biject to two distinct target identities.
    // One fixed comparison is charged from the SAME prefix before delegation.
    w.visits.charge_original_visits(1).map_err(|_| resource())?;
    if bindings.targets[0].0 == bindings.targets[1].0 {
        return Err(invalid("distinct attributed target symbols required"));
    }
    for i in 0..2 {
        if ag_text(&mut w, &rows[i], "id")? != bindings.targets[i].0 {
            return Err(StoredChronologyListError::NeedsBinding(
                "UnresolvedAttributedTarget",
            ));
        }
    }
    if system != "Current" {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnsupportedAttributedSelector",
        ));
    }
    if snapshot_symbol != bindings.snapshot.0
        || fence_symbol != bindings.revision_fence.0
        || knower_symbol != bindings.knower.0
        || trs_symbol != bindings.trs.0
    {
        return Err(StoredChronologyListError::NeedsBinding(
            "UnresolvedAttributedAlias",
        ));
    }
    w.limits.max_depth = 8;
    let knower_ref = ag_definition(&mut w, bindings.knower.1, true)?;
    let trs_ref = ag_definition(&mut w, bindings.trs.1, false)?;
    w.limits.max_depth = 64;
    w.limits.max_bytes = INPUT_CAP;
    let chronology = loaded.bound().chronology();
    let expected = [
        bindings.targets[0].1,
        bindings.targets[1].1,
        bindings.targets[0].2,
        bindings.targets[1].2,
    ];
    if chronology.member_claim_revisions.len() != 4 {
        return Err(invalid("four custody members required"));
    }
    for i in 0..4 {
        w.visits.charge_original_visits(4).map_err(|_| resource())?;
        if chronology.member_claim_revisions[i] != *expected[i]
            || expected[..i].iter().any(|r| r.id == expected[i].id)
        {
            return Err(invalid("distinct target/witness partition required"));
        }
    }
    if bindings.snapshot.1 != &snapshot.reference
        || bindings.revision_fence.1 != snapshot.reference.revision
        || chronology.selection.source_snapshot != snapshot.reference
        || chronology.selection.system_as_of.value.as_str() != Some(cutoff_lexical)
        || chronology.selection.system_as_of.trs.is_some()
        || snapshot.current_cutoff != chronology.selection.system_as_of
        || chronology.selection.source_publication_as_of.is_some()
    {
        return Err(invalid("attributed frozen source mismatch"));
    }
    if Digest256::from_hex(bound.require_source_revision()?)? != snapshot.reference.revision.0 {
        return Err(StoredChronologyListError::WrongModelSource);
    }
    let attributed = chronology
        .selection
        .attributed_knowledge
        .as_ref()
        .ok_or_else(|| invalid("frozen attribution required"))?;
    if attributed.knower != knower_ref.id
        || attributed.cutoff.value.as_str() != Some(knowledge_cutoff)
        || attributed.cutoff.trs.as_ref() != Some(&trs_ref)
    {
        return Err(invalid("frozen attribution mismatch"));
    }
    // Prepay the typed context's owned references/strings before cloning. Core
    // performs the ONE resolver call and retains its existing exhaustive checks.
    w.charge(131_072)?;
    let context = QueryContext {
        system_state: Some(SystemState::Current),
        attributed_knowledge: Some(Some(attributed.clone())),
        ..QueryContext::default()
    };
    let filter = ag_field(&mut w, page_request, "filter")?;
    ag_shape(&mut w, filter, &["predicate"])?;
    // This entry deliberately supports two targets, four custody members,
    // Current, historical_dating and a five-row coverage page. The cutoff is
    // opaque source text, not a calendar value; scan admission uses the actual
    // N+1 query partition rather than a fixture's selected-candidate count.
    let scan_limit = ag_field(&mut w, page_request, "scan_limit")?
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("positive integer scan required"))?;
    if u64::try_from(scan_limit.checked_mul(2).ok_or_else(resource)?).map_err(|_| resource())?
        > q.max_rows
    {
        return Err(resource());
    }
    if ag_text(&mut w, filter, "predicate")? != "historical_dating"
        || ag_field(&mut w, page_request, "page_size")?.as_u64() != Some(5)
        || !ag_field(&mut w, page_request, "after")?.is_null()
    {
        return Err(invalid("finite attributed page required"));
    }
    stop(deadline, cancelled)?;
    let mut predicate = AttributedPredicate {
        work: w,
        bindings,
        original: input,
        requested_cutoff: knowledge_cutoff,
        rows: [&rows[0], &rows[1]],
        knower_ref,
        trs_ref,
        answers: [None, None],
        details: [None, None],
        reasons: [None, None],
    };
    held_metadata_list(
        loaded,
        model,
        bound,
        lookup_authority,
        list_authority,
        snapshot,
        &context,
        None,
        page_request,
        remaining,
        deadline,
        cancelled,
        None,
        Some(&mut predicate),
        consume,
    )
}

fn ag_reference_field(w: &mut Work, object: &JsonValue, key: &str) -> Result<ExactRecordRef> {
    let value = ag_field(w, object, key)?;
    ag_ref(w, value)
}
fn ag_equal_field(
    w: &mut Work,
    object: &JsonValue,
    key: &str,
    other: &JsonValue,
    cap: usize,
) -> Result<bool> {
    let value = ag_field(w, object, key)?;
    ag_equal(w, value, other, cap)
}
