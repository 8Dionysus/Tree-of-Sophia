//! Explicit source-intent binding to the existing protected Claim PREPARE ABI.
//! No writer, CLI dispatch, publication, admission or supplemental basis verifier.
//! Plans are untrusted; owner_checked describes successful prepare mechanics only.
//! source_profile/mapping_profile are source-supplied ATTRIBUTION ONLY, not
//! algorithm dispatch, installed profile resolution or semantic admission. The
//! schema + operation + explicit field shape select this bounded v1 mapper.
use crate::source_command::{self as cmd, CommandContext, SourceCommandError};
use crate::source_creation_store::CreationFilesystem;
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{
    CanonicalProfile, Digest256, FoundationErrorCode, JsonLimits, JsonMode, JsonString, JsonValue,
    RelativePath, canonical_bytes_v1, emit_value_preserved_json, parse_json,
    temporal::{TemporalBudget, claim_carrier::decode_claim_carrier},
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

pub const INTENT_SCHEMA: &str = "tos_temporal_claim_intent_v1";
pub const BINDING_SCHEMA: &str = "tos_temporal_claim_intent_binding_v1";
pub const PREPARED_SCHEMA: &str = "tos_temporal_claim_intent_prepared_v1";
pub const TEMPLATE_SCHEMA: &str = "tos_temporal_claim_intent_commit_template_v1";
pub const BASIS_STATUS: &str = "SourceSuppliedUnverified";
pub const PROFILE_STATUS: &str = "SourceSuppliedAttributionOnly";

#[derive(Debug)]
pub enum ClaimIntentError {
    NeedsBinding(&'static str),
    UnsupportedSemanticMapping(&'static str),
    Mismatch(&'static str),
    Resource,
    /// Preserve the owner's Denied/Conflict/Unsupported/schema distinctions.
    Owner(SourceCommandError),
}
impl From<SourceCommandError> for ClaimIntentError {
    fn from(e: SourceCommandError) -> Self {
        Self::Owner(e)
    }
}
type Result<T> = std::result::Result<T, ClaimIntentError>;

/// Cumulative conservative logical workspace, NOT allocator/RSS or owner-core
/// execution accounting. Dropping temporaries never replenishes this allowance.
#[derive(Clone, Copy, Debug)]
pub struct ClaimIntentPrepareBudget {
    pub max_extra_owned_bytes: usize,
    pub max_extra_json_visits: usize,
    pub max_preview_bytes: usize,
    pub max_output_bytes: usize,
}
struct Work {
    left: usize,
    visits: usize,
    output: usize,
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(ClaimIntentError::Resource)
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(ClaimIntentError::Resource)
}
fn bad(s: &'static str) -> ClaimIntentError {
    ClaimIntentError::Mismatch(s)
}
// Borrowed UTF-8 key lookup avoids JsonValue::object_get's temporary UTF-16
// key allocation, including caller-sized symbolic aliases.
fn get<'a>(v: &'a JsonValue, k: &str) -> Option<&'a JsonValue> {
    v.as_object()?
        .iter()
        .find_map(|(name, value)| (name.as_str() == Some(k)).then_some(value))
}
fn field<'a>(v: &'a JsonValue, k: &'static str) -> Result<&'a JsonValue> {
    get(v, k).ok_or(ClaimIntentError::NeedsBinding(k))
}
fn text<'a>(v: &'a JsonValue, k: &'static str) -> Result<&'a str> {
    field(v, k)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| bad(k))
}
fn keys(v: &JsonValue, expected: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(|| bad("object required"))?;
    if rows.len() != expected.len() {
        return Err(bad("unexpected or missing fields"));
    }
    for (i, (k, _)) in rows.iter().enumerate() {
        if !expected.contains(&k.as_str().unwrap_or("")) || rows[..i].iter().any(|(p, _)| p == k) {
            return Err(bad("unknown or duplicate fields"));
        }
    }
    Ok(())
}
impl Work {
    fn new(b: ClaimIntentPrepareBudget) -> Result<Self> {
        if b.max_extra_owned_bytes == 0
            || b.max_extra_json_visits == 0
            || b.max_preview_bytes == 0
            || b.max_output_bytes == 0
        {
            return Err(ClaimIntentError::Resource);
        }
        Ok(Self {
            left: b.max_extra_owned_bytes,
            visits: b.max_extra_json_visits,
            output: b.max_output_bytes,
        })
    }
    fn charge(&mut self, n: usize) -> Result<()> {
        self.left = self.left.checked_sub(n).ok_or(ClaimIntentError::Resource)?;
        Ok(())
    }
    // Stack-only, finite traversal BEFORE clone/emission. Account UTF-16/UTF-8,
    // container slots, escapes, numeric spelling/sort workspace conservatively.
    fn bound(&mut self, v: &JsonValue, depth: usize) -> Result<usize> {
        self.visits = self
            .visits
            .checked_sub(1)
            .ok_or(ClaimIntentError::Resource)?;
        if depth > 64 {
            return Err(ClaimIntentError::Resource);
        }
        let mut n = size_of::<JsonValue>() + 64;
        match v {
            JsonValue::String(s) => n = add(n, mul(s.units().len(), 16)?)?,
            JsonValue::Number(nr) => n = add(n, add(nr.lexeme.len(), 32)?)?,
            JsonValue::Array(a) => {
                for x in a {
                    n = add(n, self.bound(x, depth + 1)?)?;
                }
            }
            JsonValue::Object(a) => {
                for (i, (k, x)) in a.iter().enumerate() {
                    // Duplicate checking work is admitted separately.
                    self.visits = self
                        .visits
                        .checked_sub(i)
                        .ok_or(ClaimIntentError::Resource)?;
                    if a[..i].iter().any(|(p, _)| p == k) {
                        return Err(bad("duplicate JSON member"));
                    }
                    n = add(
                        n,
                        add(
                            size_of::<(JsonString, JsonValue)>(),
                            mul(k.units().len(), 16)?,
                        )?,
                    )?;
                    n = add(n, self.bound(x, depth + 1)?)?;
                }
            }
            _ => {}
        }
        Ok(n)
    }
    fn clone_value(&mut self, v: &JsonValue) -> Result<JsonValue> {
        let n = self.bound(v, 0)?;
        self.charge(mul(n, 2)?)?;
        Ok(v.clone())
    }
    fn string(&mut self, s: &str) -> Result<JsonValue> {
        self.charge(add(mul(s.len(), 8)?, 128)?)?;
        Ok(cmd::string(s))
    }
    fn digest_string(&mut self, digest: Digest256) -> Result<JsonValue> {
        self.charge(80)?; // temporary prefixed digest before constructing JsonString
        self.string(&digest.to_prefixed())
    }
    fn object<const N: usize>(
        &mut self,
        rows: [(&'static str, JsonValue); N],
    ) -> Result<JsonValue> {
        self.charge(mul(N, size_of::<(JsonString, JsonValue)>())?)?;
        let mut out = Vec::with_capacity(N);
        for (k, v) in rows {
            self.charge(add(mul(k.len(), 8)?, 64)?)?;
            out.push((JsonString::from_utf8(k), v));
        }
        Ok(JsonValue::Object(out))
    }
    fn limits(&self) -> JsonLimits {
        JsonLimits {
            max_bytes: self.output,
            max_depth: 64,
            max_visits: self.visits.max(1),
            max_integer_digits: self.output,
        }
    }
    fn emit(&mut self, v: &JsonValue, preserve: bool) -> Result<Vec<u8>> {
        let n = self.bound(v, 0)?;
        // Reserve output Vec growth plus canonical sorting/number workspace.
        self.charge(mul(n, 8)?)?;
        let limits = self.limits();
        let raw = if preserve {
            emit_value_preserved_json(v, limits)
        } else {
            canonical_bytes_v1(v, CanonicalProfile::SourceCommandInputV1, limits)
        }
        .map_err(|_| ClaimIntentError::Resource)?;
        if raw.capacity() > mul(n, 2)? || raw.len() > self.output {
            return Err(ClaimIntentError::Resource);
        }
        Ok(raw)
    }
    fn parse(&mut self, raw: &[u8]) -> Result<JsonValue> {
        if raw.len() > self.output {
            return Err(ClaimIntentError::Resource);
        }
        self.charge(add(mul(raw.len(), 128)?, 128)?)?;
        let doc = parse_json(raw, JsonMode::PublishedStrict, self.limits()).map_err(|e| {
            if e.code == FoundationErrorCode::BudgetExceeded {
                ClaimIntentError::Resource
            } else {
                bad("invalid JSON")
            }
        })?;
        let v = doc.into_root();
        let _ = self.bound(&v, 0)?;
        Ok(v)
    }
    fn equal(&mut self, a: &JsonValue, b: &JsonValue) -> Result<bool> {
        // Preserved semantic JSON comparison: key order insignificant, numeric
        // lexemes exact. Never normalize Float/-0 to make a payload match.
        let _ = self.bound(a, 0)?;
        let _ = self.bound(b, 0)?;
        fn eq(a: &JsonValue, b: &JsonValue) -> bool {
            match (a, b) {
                (JsonValue::Object(a), JsonValue::Object(b)) => {
                    a.len() == b.len()
                        && a.iter().all(|(k, v)| {
                            b.iter()
                                .find(|(p, _)| p == k)
                                .is_some_and(|(_, x)| eq(v, x))
                        })
                }
                (JsonValue::Array(a), JsonValue::Array(b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| eq(a, b))
                }
                _ => a == b,
            }
        }
        Ok(eq(a, b))
    }
    fn same(&mut self, a: &JsonValue, b: &JsonValue) -> Result<()> {
        if self.equal(a, b)? {
            Ok(())
        } else {
            Err(bad("intent/payload mismatch"))
        }
    }
}

/// No constructor or mutator outside this module. Candidate plan is NOT custody.
pub struct ClaimIntentPlan {
    request: JsonValue,
    target: JsonValue,
    intent_text_sha256: Digest256,
    intent_canonical_sha256: Digest256,
    work: Work,
    preview_cap: usize,
}
/// Successful existing owner PREPARE only; neither receipt nor permission token.
pub struct PreparedClaimIntent {
    request: JsonValue,
    preview: JsonValue,
    work: Work,
    intent_text_sha256: Digest256,
    intent_canonical_sha256: Digest256,
    configuration_raw_sha256: Digest256,
    configuration_canonical_sha256: Digest256,
}
fn prefixed(v: &JsonValue, k: &'static str) -> Result<Digest256> {
    Digest256::from_prefixed(text(v, k)?).map_err(|_| bad(k))
}
fn exact(w: &mut Work, v: &JsonValue) -> Result<()> {
    keys(v, &["id", "version", "digest"])?;
    w.charge(add(mul(text(v, "id")?.len(), 2)?, 64)?)?;
    tos_foundation::StableId::parse(text(v, "id")?).map_err(|_| bad("exact ID"))?;
    if !field(v, "version")?.as_u64().is_some_and(|v| v > 0) {
        return Err(bad("exact version"));
    }
    prefixed(v, "digest")?;
    Ok(())
}
fn alias<'a>(b: &'a JsonValue, table: &'static str, symbol: &str) -> Result<&'a JsonValue> {
    get(field(b, table)?, symbol).ok_or(ClaimIntentError::NeedsBinding(table))
}
fn position(
    w: &mut Work,
    p: &JsonValue,
    lexical: &JsonValue,
    trs_symbol: &JsonValue,
    b: &JsonValue,
) -> Result<()> {
    keys(p, &["type", "lexical", "resolution", "trs"])?;
    if text(p, "type")? != "CalendarPosition" {
        return Err(bad("explicit calendar position required"));
    }
    w.same(field(p, "lexical")?, lexical)?;
    // Precision is explicitly supplied; no lexical/host inference.
    if text(p, "resolution")? != "day" {
        return Err(ClaimIntentError::UnsupportedSemanticMapping(
            "validity precision profile",
        ));
    }
    let trs = alias(
        b,
        "trs",
        trs_symbol.as_str().ok_or_else(|| bad("TRS symbol"))?,
    )?;
    keys(trs, &["id", "version"])?;
    text(trs, "id")?;
    text(trs, "version")?;
    w.same(field(p, "trs")?, trs)
}
fn unverified_basis(w: &mut Work, v: &JsonValue) -> Result<()> {
    // Evidence DATA may quote arbitrary claims; structured basis may not
    // masquerade as verified/admitted/resolved through extension fields.
    keys(v, &["source", "source_locator", "raw_evidence"])?;
    let source = field(v, "source")?;
    keys(source, &["id", "version", "digest"])?;
    w.charge(add(mul(text(source, "id")?.len(), 2)?, 64)?)?;
    tos_foundation::StableId::parse(text(source, "id")?).map_err(|_| bad("basis ID"))?;
    if !field(source, "version")?.as_u64().is_some_and(|n| n > 0) {
        return Err(bad("basis version"));
    }
    Digest256::from_hex(text(source, "digest")?).map_err(|_| bad("basis digest"))?;
    let locator = field(v, "source_locator")?;
    if locator != &JsonValue::Null && locator.as_str().is_none() {
        return Err(bad("basis locator"));
    }
    Ok(())
}
fn validate_payload(w: &mut Work, intent: &JsonValue, b: &JsonValue) -> Result<&'static str> {
    let input = field(intent, "input")?;
    let payload = field(b, "payload")?;
    if text(intent, "operation")? == "record_explicit_relation" {
        keys(
            input,
            &[
                "subject_a",
                "relation",
                "subject_b",
                "source_ref",
                "absolute_mapping",
            ],
        )?;
        if field(input, "absolute_mapping")? != &JsonValue::Null {
            return Err(ClaimIntentError::UnsupportedSemanticMapping(
                "absolute relation mapping",
            ));
        }
        keys(payload, &["schema_version", "relative_relations"])?;
        let rows = field(payload, "relative_relations")?
            .as_array()
            .ok_or_else(|| bad("relation rows"))?;
        if rows.len() != 1 {
            return Err(bad("one original source assertion"));
        }
        let r = &rows[0];
        keys(
            r,
            &[
                "subject_a",
                "subject_b",
                "relation",
                "source",
                "asserted_by",
                "wording_ref",
                "epistemic_status",
            ],
        )?;
        for k in ["subject_a", "subject_b"] {
            w.same(field(r, k)?, alias(b, "identities", text(input, k)?)?)?;
        }
        w.same(field(r, "relation")?, field(input, "relation")?)?;
        let source = alias(b, "sources", text(input, "source_ref")?)?;
        // Codec expects bare digest, unlike native owner references.
        keys(source, &["id", "version", "digest"])?;
        w.same(field(r, "source")?, source)?;
        if matches!(
            text(r, "epistemic_status")?,
            "Entailed" | "entailed" | "verified" | "admitted" | "resolved"
        ) {
            return Err(ClaimIntentError::UnsupportedSemanticMapping(
                "derived authority is not a source relation status",
            ));
        }
        return Ok("relative_relations");
    }
    if text(intent, "operation")? != "append_claim_revision" {
        return Err(ClaimIntentError::UnsupportedSemanticMapping("operation"));
    }
    let target = field(b, "target")?;
    w.same(field(input, "claim_ref")?, field(target, "alias")?)?;
    if get(input, "new_date_claim").is_some() {
        keys(
            input,
            &[
                "claim_ref",
                "previous_revision",
                "new_revision",
                "new_date_claim",
                "prior_chronology_members",
            ],
        )?;
        w.same(
            field(input, "previous_revision")?,
            field(target, "revision_alias")?,
        )?;
        w.same(
            field(input, "new_revision")?,
            field(target, "next_revision_alias")?,
        )?;
        if field(input, "prior_chronology_members")?
            .as_array()
            .is_none()
        {
            return Err(bad("prior members"));
        }
        if text(payload, "kind")? != "date-assertion" || text(payload, "role")? != "historical-time"
        {
            return Err(bad("full historicalDate object required"));
        }
        for k in ["calendar", "year_numbering", "certainty"] {
            text(payload, k)?;
        }
        let wording = field(payload, "source_wording")?;
        text(wording, "language")?;
        text(wording, "text")?;
        w.same(field(payload, "value")?, field(input, "new_date_claim")?)?;
        return Ok("object");
    }
    w.same(field(input, "revision")?, field(target, "revision_alias")?)?;
    if get(input, "validity_assertion").is_some() {
        keys(input, &["claim_ref", "revision", "validity_assertion"])?;
        let validity = field(input, "validity_assertion")?;
        if text(validity, "type")? == "Interval" {
            // Source lexical beginning + separately Unknown END. No parsing,
            // day precision, calendar/TRS/UTC or world extent is manufactured.
            keys(validity, &["type", "valid_from", "valid_to"])?;
            let beginning_source = field(validity, "valid_from")?;
            if !beginning_source
                .as_str()
                .is_some_and(|s| !s.trim().is_empty())
            {
                return Err(ClaimIntentError::UnsupportedSemanticMapping(
                    "Interval beginning requires source lexical string",
                ));
            }
            let source_end = field(validity, "valid_to")?;
            keys(source_end, &["type"])?;
            if text(source_end, "type")? != "Unknown" {
                return Err(ClaimIntentError::UnsupportedSemanticMapping(
                    "Interval mapping supports only explicit Unknown END",
                ));
            }
            keys(payload, &["schema_version", "validity"])?;
            let v = field(payload, "validity")?;
            // Missing explicit basis is a binding dependency, never filled from
            // the target Claim/ref/receipt or the system source-cut timestamp.
            let basis = field(v, "basis")?;
            keys(v, &["type", "beginning", "end", "basis"])?;
            if text(v, "type")? != "Interval" {
                return Err(bad("Interval validity type"));
            }
            let beginning = field(v, "beginning")?;
            keys(beginning, &["type", "lexical", "resolution", "trs"])?;
            if text(beginning, "type")? != "CalendarPosition"
                || text(beginning, "resolution")? != "Unknown"
                || field(beginning, "trs")? != &JsonValue::Null
            {
                return Err(bad(
                    "Interval beginning must remain unresolved lexical data",
                ));
            }
            w.same(field(beginning, "lexical")?, beginning_source)?;
            let end = field(v, "end")?;
            keys(end, &["type"])?;
            w.same(end, source_end)?;
            unverified_basis(w, basis)?;
            return Ok("validity");
        }
        if text(validity, "type")? != "RightCensored" {
            return Err(ClaimIntentError::UnsupportedSemanticMapping(
                "unsupported source validity mapping",
            ));
        }
        keys(
            validity,
            &["type", "valid_from", "observed_through", "trs_ref"],
        )?;
        keys(payload, &["schema_version", "validity"])?;
        let v = field(payload, "validity")?;
        keys(
            v,
            &["type", "beginning", "observed_through", "cutoff_basis"],
        )?;
        if text(v, "type")? != "RightCensored" {
            return Err(bad("validity type"));
        }
        position(
            w,
            field(v, "beginning")?,
            field(validity, "valid_from")?,
            field(validity, "trs_ref")?,
            b,
        )?;
        position(
            w,
            field(v, "observed_through")?,
            field(validity, "observed_through")?,
            field(validity, "trs_ref")?,
            b,
        )?;
        unverified_basis(w, field(v, "cutoff_basis")?)?;
        return Ok("validity");
    }
    if get(input, "recurrence_pattern").is_some() {
        keys(input, &["claim_ref", "revision", "recurrence_pattern"])?;
        keys(payload, &["schema_version", "recurrence"])?;
        let r = field(payload, "recurrence")?;
        keys(r, &["profile_ref", "pattern", "basis"])?;
        text(r, "profile_ref")?;
        unverified_basis(w, field(r, "basis")?)?;
        w.same(field(r, "pattern")?, field(input, "recurrence_pattern")?)?;
        return Ok("recurrence");
    }
    keys(
        input,
        &[
            "claim_ref",
            "revision",
            "event_time",
            "source_publication_time",
            "system_ingest_time",
            "historical_author_knowledge",
        ],
    )?;
    keys(payload, &["schema_version", "roles"])?;
    let roles = field(payload, "roles")?
        .as_array()
        .ok_or_else(|| bad("roles"))?;
    if roles.len() != 4 {
        return Err(bad("exact four independent roles"));
    }
    for (key, tag) in [
        ("event_time", "WorldOccurrence"),
        ("source_publication_time", "SourcePublication"),
        ("system_ingest_time", "SystemTransaction"),
        ("historical_author_knowledge", "AttributedKnowledge"),
    ] {
        let mut matching = roles
            .iter()
            .filter(|r| get(r, "role").and_then(JsonValue::as_str) == Some(tag));
        let r = matching.next().ok_or_else(|| bad("missing role"))?;
        if matching.next().is_some() {
            return Err(bad("duplicate role"));
        }
        keys(r, &["role", "assertion", "basis"])?;
        unverified_basis(w, field(r, "basis")?)?;
        w.same(field(r, "assertion")?, field(input, key)?)?;
    }
    Ok("roles")
}

/// Finite explicit-bound planner. `intent` has schema_version plus
/// operation/source_profile/input/context ONLY (no test expected/forbidden instructions); bindings schema v1
/// requires all tables, even when empty. Extras refuse, opaque source DATA stays.
pub fn plan_claim_intent(
    intent: &JsonValue,
    bindings: &JsonValue,
    target_record: &JsonValue,
    temporal: TemporalBudget,
    budget: ClaimIntentPrepareBudget,
) -> Result<ClaimIntentPlan> {
    let mut w = Work::new(budget)?;
    if temporal.max_expression_bytes == 0 || temporal.max_members == 0 {
        return Err(ClaimIntentError::Resource);
    }
    for value in [intent, bindings, target_record] {
        let before = w.visits;
        let n = w.bound(value, 0)?;
        if before - w.visits > temporal.max_members {
            return Err(ClaimIntentError::Resource);
        }
        let saved = w.output;
        w.output = w.output.min(temporal.max_expression_bytes);
        let checked = w.emit(value, true);
        w.output = saved;
        checked?;
        if n > mul(temporal.max_expression_bytes, 128)? {
            return Err(ClaimIntentError::Resource);
        }
    }
    keys(
        intent,
        &[
            "schema_version",
            "operation",
            "source_profile",
            "input",
            "context",
        ],
    )?;
    if text(intent, "schema_version")? != INTENT_SCHEMA {
        return Err(bad("intent schema"));
    }
    text(intent, "source_profile")?;
    keys(field(intent, "context")?, &[])?;
    keys(
        bindings,
        &[
            "schema_version",
            "target",
            "forms",
            "authored_reason",
            "mapping_profile",
            "payload",
            "identities",
            "sources",
            "trs",
        ],
    )?;
    if text(bindings, "schema_version")? != BINDING_SCHEMA {
        return Err(bad("binding schema"));
    }
    for k in ["identities", "sources", "trs"] {
        if field(bindings, k)?.as_object().is_none() {
            return Err(bad("alias table"));
        }
    }
    let target = field(bindings, "target")?;
    keys(
        target,
        &[
            "alias",
            "revision_alias",
            "next_revision_alias",
            "source",
            "source_path",
            "source_revision",
            "configuration_digest",
        ],
    )?;
    for k in ["alias", "revision_alias", "source_path"] {
        text(target, k)?;
    }
    if get(field(intent, "input")?, "new_date_claim").is_some() {
        text(target, "next_revision_alias")?;
    } else if field(target, "next_revision_alias")? != &JsonValue::Null {
        return Err(bad("unused next revision alias must be null"));
    }
    w.charge(add(mul(text(target, "source_path")?.len(), 8)?, 64)?)?;
    RelativePath::parse(text(target, "source_path")?).map_err(|_| bad("source path"))?;
    prefixed(target, "source_revision")?;
    prefixed(target, "configuration_digest")?;
    exact(&mut w, field(target, "source")?)?;
    // Actual native reference uses SourceCommandInputV1 canonical digest.
    w.same(
        field(field(target, "source")?, "id")?,
        field(target_record, "claim_id")?,
    )?;
    w.same(
        field(field(target, "source")?, "version")?,
        field(target_record, "claim_version")?,
    )?;
    if Digest256::of_bytes(&w.emit(target_record, false)?)
        != prefixed(field(target, "source")?, "digest")?
    {
        return Err(bad("target packet digest"));
    }
    let component = validate_payload(&mut w, intent, bindings)?;
    let payload = field(bindings, "payload")?;
    let fields = if component == "object" {
        let value = w.clone_value(payload)?;
        w.object([("object", value)])?
    } else {
        // Source assertions only: supplemental references are NOT resolved by
        // this adapter. Fixed public provenance cannot be overridden by input.
        let old = get(target_record, "qualifiers");
        let previous = old.and_then(|q| get(q, "temporal_claim_carrier"));
        let mut packet = if let Some(p) = previous {
            let q = w.clone_value(old.ok_or_else(|| bad("qualifiers"))?)?;
            let reserve = w.bound(&q, 0)?;
            w.charge(mul(reserve, 8)?)?;
            decode_claim_carrier(&q, temporal)
                .map_err(|_| ClaimIntentError::UnsupportedSemanticMapping("existing carrier"))?;
            w.clone_value(p)?
        } else {
            let schema = w.string("tos_temporal_claim_carrier_v1")?;
            w.object([("schema_version", schema)])?
        };
        let reserve = w.bound(payload, 0)?;
        w.charge(mul(reserve, 8)?)?;
        let candidate = w.clone_value(payload)?;
        let q = w.object([("temporal_claim_carrier", candidate)])?;
        decode_claim_carrier(&q, temporal)
            .map_err(|_| ClaimIntentError::UnsupportedSemanticMapping("source carrier"))?
            .ok_or_else(|| bad("missing carrier"))?;
        let replacement = field(payload, component)?;
        let value = if matches!(component, "roles" | "relative_relations") {
            let new = replacement
                .as_array()
                .ok_or_else(|| bad("assertion list"))?;
            let old = get(&packet, component)
                .map(|v| v.as_array().ok_or_else(|| bad("existing assertion list")))
                .transpose()?
                .unwrap_or(&[]);
            let count = add(old.len(), new.len())?;
            if count > temporal.max_members {
                return Err(ClaimIntentError::Resource);
            }
            w.charge(mul(count, size_of::<JsonValue>())?)?;
            let mut rows = Vec::with_capacity(count);
            for v in old.iter().chain(new) {
                rows.push(w.clone_value(v)?);
            }
            JsonValue::Array(rows)
        } else {
            w.clone_value(replacement)?
        };
        // Explicit intent replaces this one component only, preserving other
        // components/unknown keys; old packets remain in owner version history.
        let n = w.bound(&packet, 0)?;
        w.charge(mul(n, 2)?)?;
        w.charge(add(mul(component.len(), 8)?, 128)?)?;
        cmd::set(&mut packet, component, value)?;
        let q = w.object([("temporal_claim_carrier", packet)])?;
        let reserve = w.bound(&q, 0)?;
        w.charge(mul(reserve, 8)?)?;
        decode_claim_carrier(&q, temporal)
            .map_err(|_| ClaimIntentError::UnsupportedSemanticMapping("merged carrier"))?;
        w.object([("qualifiers", q)])?
    };
    let preserved = w.clone_value(intent)?;
    let reason = w.string(text(bindings, "authored_reason")?)?;
    let profile = w.string(text(bindings, "mapping_profile")?)?;
    let reason_value = w.object([
        ("authored_reason", reason),
        ("preserved_intent", preserved),
        ("binding_profile", profile),
    ])?;
    let reason_raw = w.emit(&reason_value, true)?;
    let reason_text = std::str::from_utf8(&reason_raw).map_err(|_| bad("reason UTF8"))?;
    if reason_text.chars().count() > 4096 {
        return Err(ClaimIntentError::Resource);
    }
    let intent_text_sha256 = Digest256::of_bytes(&w.emit(intent, true)?);
    let intent_canonical_sha256 = Digest256::of_bytes(&w.emit(intent, false)?);
    let schema = w.string("tos_local_source_command_v1")?;
    let operation = w.string("prepare-revise")?;
    let forms = w.clone_value(field(bindings, "forms")?)?;
    if forms.as_array().is_none() {
        return Err(bad("forms"));
    }
    let reason = w.string(reason_text)?;
    let request = w.object([
        ("schema_version", schema),
        ("operation", operation),
        ("fields", fields),
        ("forms", forms),
        ("reason", reason),
    ])?;
    let _ = w.emit(&request, false)?;
    let target = w.clone_value(target)?;
    Ok(ClaimIntentPlan {
        request,
        target,
        intent_text_sha256,
        intent_canonical_sha256,
        work: w,
        preview_cap: budget.max_preview_bytes,
    })
}
impl ClaimIntentPlan {
    pub fn basis_status(&self) -> &'static str {
        BASIS_STATUS
    }
    pub fn owner_checked(&self) -> bool {
        false
    }
}

/// No context clone by the adapter: replace request temporarily and restore it
/// even on owner error. Reserve new overlap and a context-copy forecast before
/// delegate. Existing owner internally owns complete-context/changes/reads/
/// schema/custody budgets; max_preview_bytes is a retained RESPONSE limit, NOT
/// a cap on delegated return allocation or combined process peak/RSS.
#[allow(clippy::too_many_arguments)]
pub fn prepare_claim_intent_from_captures(
    filesystem: &CreationFilesystem,
    ctx: &mut CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    mut plan: ClaimIntentPlan,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PreparedClaimIntent> {
    let w = &mut plan.work;
    if ctx.base_revision != current_cut.current().revision()
        || ctx.base_revision.0 != prefixed(&plan.target, "source_revision")?
    {
        return Err(ClaimIntentError::Owner(SourceCommandError::Conflict(
            "intent current source cut",
        )));
    }
    let mut forecast = add(
        size_of::<CommandContext>(),
        add(ctx.configuration_raw.capacity(), ctx.request_raw.capacity())?,
    )?;
    forecast = add(forecast, ctx.recorded_at.capacity())?;
    forecast = add(
        forecast,
        mul(ctx.files.capacity(), size_of::<cmd::SourceFile>())?,
    )?;
    for f in &ctx.files {
        forecast = add(
            forecast,
            add(f.raw.capacity(), mul(f.path.as_str().len(), 8)?)?,
        )?;
    }
    w.charge(mul(forecast, 2)?)?;
    let configuration = w.parse(&ctx.configuration_raw)?;
    let canonical = Digest256::of_bytes(&w.emit(&configuration, false)?);
    if canonical != prefixed(&plan.target, "configuration_digest")? {
        return Err(ClaimIntentError::Owner(SourceCommandError::Conflict(
            "intent canonical configuration",
        )));
    }
    if text(&configuration, "claim_id")? != text(field(&plan.target, "source")?, "id")?
        || text(&configuration, "source_path")? != text(&plan.target, "source_path")?
    {
        return Err(ClaimIntentError::Owner(SourceCommandError::Conflict(
            "intent owner target",
        )));
    }
    w.charge(add(mul(text(&plan.target, "source_path")?.len(), 8)?, 64)?)?;
    let path =
        RelativePath::parse(text(&plan.target, "source_path")?).map_err(|_| bad("target path"))?;
    let raw = ctx.file(&path)?.ok_or(ClaimIntentError::NeedsBinding(
        "selected current Claim stream",
    ))?;
    let mut found = false;
    for line in raw
        .split(|b| *b == b'\n')
        .filter(|s| !s.iter().all(u8::is_ascii_whitespace))
    {
        let row = w.parse(line)?;
        if text(&row, "claim_id")? == text(field(&plan.target, "source")?, "id")? {
            if found {
                return Err(bad("duplicate selected Claim"));
            }
            found = true;
            w.same(
                field(&row, "claim_version")?,
                field(field(&plan.target, "source")?, "version")?,
            )?;
            if Digest256::of_bytes(&w.emit(&row, false)?)
                != prefixed(field(&plan.target, "source")?, "digest")?
            {
                return Err(ClaimIntentError::Owner(SourceCommandError::Conflict(
                    "intent exact current Claim",
                )));
            }
        }
    }
    if !found {
        return Err(ClaimIntentError::NeedsBinding("current Claim"));
    }
    let request_raw = w.emit(&plan.request, false)?;
    // Reserve only adapter-retained response state; delegate may allocate its
    // separately bounded PreparedCommand including changes/reads before return.
    w.charge(plan.preview_cap)?;
    let saved = std::mem::replace(&mut ctx.request_raw, request_raw);
    let result = crate::source_claims::prepare_isolated_claim_revision_from_captures(
        filesystem,
        ctx,
        original_cut,
        current_cut,
        software,
        components,
        worker,
        deadline,
        cancelled,
    );
    ctx.request_raw = saved;
    let prepared = result?;
    let cmd::PreparedCommand {
        response,
        configuration_raw_sha256,
        configuration_canonical_sha256,
        changes,
        reads,
        handler_id,
        operation,
        ..
    } = prepared;
    drop((changes, reads, handler_id, operation));
    let n = w.bound(&response, 0)?;
    if n > plan.preview_cap {
        return Err(ClaimIntentError::Resource);
    }
    w.same(field(&response, "source")?, field(&plan.target, "source")?)?;
    if configuration_canonical_sha256 != canonical
        || prefixed(&response, "owner_configuration")? != canonical
    {
        return Err(bad("owner canonical configuration response"));
    }
    Ok(PreparedClaimIntent {
        request: plan.request,
        preview: response,
        work: plan.work,
        intent_text_sha256: plan.intent_text_sha256,
        intent_canonical_sha256: plan.intent_canonical_sha256,
        configuration_raw_sha256,
        configuration_canonical_sha256,
    })
}
impl PreparedClaimIntent {
    /// Descriptive prepare mechanics only, never a transferable permission.
    pub fn owner_checked(&self) -> bool {
        true
    }
    pub fn basis_status(&self) -> &'static str {
        BASIS_STATUS
    }
    pub fn prepared_packet(&mut self) -> Result<JsonValue> {
        let w = &mut self.work;
        let schema = w.string(PREPARED_SCHEMA)?;
        let status = w.string("PreparedNotCommitted")?;
        let basis = w.string(BASIS_STATUS)?;
        let profile_status = w.string(PROFILE_STATUS)?;
        let preview = w.clone_value(&self.preview)?;
        let text_sha = w.digest_string(self.intent_text_sha256)?;
        let canonical_sha = w.digest_string(self.intent_canonical_sha256)?;
        let config_raw = w.digest_string(self.configuration_raw_sha256)?;
        let config_canonical = w.digest_string(self.configuration_canonical_sha256)?;
        let output = w.object([
            ("schema_version", schema),
            ("status", status),
            ("basis_status", basis),
            ("profile_status", profile_status),
            ("owner_checked", JsonValue::Bool(true)),
            ("grants_admission", JsonValue::Bool(false)),
            ("intent_preserved_text_sha256", text_sha),
            ("intent_canonical_sha256", canonical_sha),
            ("configuration_raw_sha256", config_raw),
            ("configuration_canonical_sha256", config_canonical),
            ("owner_preview", preview),
        ])?;
        let _ = w.emit(&output, false)?;
        Ok(output)
    }
    /// No committed-result API exists. The untrusted template must still pass
    /// the EXISTING native claim.revise executor's rebuilt grants/fences/CAS.
    pub fn commit_request_template(&mut self, command_id: &str) -> Result<JsonValue> {
        if command_id.len() > 1024
            || command_id.trim().is_empty()
            || command_id.chars().count() > 256
        {
            return Err(bad("command ID"));
        }
        let w = &mut self.work;
        let mut request = w.clone_value(&self.request)?;
        let operation = w.string("claim.revise")?;
        let id = w.string(command_id)?;
        // Pre-admit Vec growth and key strings before each cmd::set mutation.
        let n = w.bound(&request, 0)?;
        w.charge(mul(n, 4)?)?;
        cmd::set(&mut request, "operation", operation)?;
        cmd::set(&mut request, "command_id", id)?;
        for (dst, src) in [
            ("expected_configuration", "owner_configuration"),
            ("expected_source", "source"),
            ("expected_revision", "revision"),
            ("expected_dependencies", "expected_dependencies"),
            ("expected_inputs", "source_bindings"),
        ] {
            let value = w.clone_value(field(&self.preview, src)?)?;
            let n = w.bound(&request, 0)?;
            w.charge(add(mul(n, 2)?, add(mul(dst.len(), 8)?, 128)?)?)?;
            cmd::set(&mut request, dst, value)?;
        }
        let request_bytes = w.emit(&request, false)?;
        let request_sha = Digest256::of_bytes(&request_bytes);
        let schema = w.string(TEMPLATE_SCHEMA)?;
        let basis = w.string(BASIS_STATUS)?;
        let profile_status = w.string(PROFILE_STATUS)?;
        let status = w.string("CommitTemplateNotCommitted")?;
        let text_sha = w.digest_string(self.intent_text_sha256)?;
        let canonical_sha = w.digest_string(self.intent_canonical_sha256)?;
        let raw_sha = w.digest_string(request_sha)?;
        let config_raw = w.digest_string(self.configuration_raw_sha256)?;
        let config_canonical = w.digest_string(self.configuration_canonical_sha256)?;
        let output = w.object([
            ("schema_version", schema),
            ("status", status),
            ("basis_status", basis),
            ("profile_status", profile_status),
            ("owner_checked", JsonValue::Bool(true)),
            ("grants_admission", JsonValue::Bool(false)),
            ("intent_preserved_text_sha256", text_sha),
            ("intent_canonical_sha256", canonical_sha),
            ("emitted_owner_request_sha256", raw_sha),
            ("configuration_raw_sha256", config_raw),
            ("configuration_canonical_sha256", config_canonical),
            ("request", request),
        ])?;
        let _ = w.emit(&output, false)?;
        Ok(output)
    }
}
