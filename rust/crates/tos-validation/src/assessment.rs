//! Bounded native application of the maintained knowledge-assessment rules.
//!
//! These inputs are observations supplied by the protected owner reader. This
//! module authenticates nobody, reads no journal, and issues no permission.
//! In particular a rendered `can_use` is the deterministic current view of
//! authored judgments, not a publication/admission token. CMD must reconstruct
//! and reevaluate its protected snapshot under its current source/config/head
//! fence. Source grounding, native content custody and history authenticity
//! cannot be established by assessment prose or by this pure calculation.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Instant;

use serde_json::{Value, json};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonNumberKind, JsonValue,
    SourceRevision, canonical_bytes_v1, canonical_raw_bytes_v1, parse_json,
    python_casefold_unicode16_v1, python_strip_unicode16_v1,
};

use crate::FormatProfile;
use crate::executor::BatchBudget;
use crate::item_rules::ItemRefusal;
use crate::retirement_rules::observed_instant_order;
use crate::source_cut::{
    CutExecutionBinding, CutSchemaCheck, CutSchemaExecutor, CutWorkerSchemaExecutor,
};

pub const MAX_ASSESSMENTS: usize = 1024;
pub const MAX_RECORD_BYTES: usize = 1_048_576;
pub const MAX_REQUIRED_ADMISSIONS: usize = 64;

/// Raw `{id,version,payload,origin_id}` owner envelopes. Payload record
/// digests are derived here using the existing Python record profile.
#[derive(Debug, Clone)]
pub struct AssessmentRecordInput {
    pub envelope: Vec<u8>,
}

/// The principal and execution binding are supplied by the authenticated
/// adapter; the original committed scope comes from its verified batch.
#[derive(Debug, Clone)]
pub struct AssessmentSubmissionInput {
    pub assessment: Vec<u8>,
    pub principal_id: String,
    pub execution_profile: Vec<u8>,
    pub committed_scope: Option<Vec<u8>>,
}

/// Current source-layer comparison observations supplied by the selected
/// native owner adapter after the exact private source comparison and its
/// currentness recheck. These values are never accepted from a command
/// request or read as caller-provided readiness/eligibility from configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssessmentLayerQualityObservation {
    /// The selected source contract requires an exact source comparison.
    pub source_comparison_required: bool,
    /// That required comparison is present in the current selected source cut.
    pub source_comparison_present: bool,
    /// Current exact source comparison and layer policy permit positive use.
    /// The native adapter derives this; assessment prose and request fields do
    /// not grant it.
    pub positive_use_allowed: bool,
}

#[derive(Debug, Clone)]
pub struct AssessmentReadInput {
    /// Independently selected current authored source/schema cut. Protected
    /// owner configuration and journal bytes remain outside this namespace.
    pub source_revision: SourceRevision,
    pub policy: AssessmentRecordInput,
    pub authorities: Vec<AssessmentRecordInput>,
    pub competencies: Vec<AssessmentRecordInput>,
    pub records: Vec<AssessmentRecordInput>,
    /// Independently resolved authored source bindings; inline owner records
    /// cannot shadow their IDs. Native adapters remain separately identified.
    pub source_records: Vec<AssessmentRecordInput>,
    pub native_records: Vec<AssessmentRecordInput>,
    pub source_route: AssessmentSourceRoute,
    pub subject_id: String,
    /// Actual configured scope including its exact `record` binding. It is
    /// selected by the protected owner configuration, never by a reviewer.
    pub configured_scope: Vec<u8>,
    /// Exact refs selected by the actual source-owner dependency resolver.
    pub required_source_refs: Vec<Vec<u8>>,
    /// Existing owner-derived quality basis envelopes, with their actual
    /// `can_use`/`limits` observations. A caller cannot substitute a bool.
    pub required_admission_bases: Vec<AssessmentRecordInput>,
    /// Present only for the selected native LayerQuality adapter after exact
    /// comparison and currentness recheck. Never supplied by the command
    /// request/configuration or inferred from assessment prose.
    pub layer_quality: Option<AssessmentLayerQualityObservation>,
    /// Content-ready is derived from the separate `native_records` and their
    /// entire native_binding/content_verified observations. The private owner
    /// resolver must produce these after an actual content read; authored or
    /// configured payloads cannot populate that vector as read observations.
    pub reviews: Vec<AssessmentSubmissionInput>,
    pub trusted_history: Vec<AssessmentSubmissionInput>,
    pub observed_now: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssessmentSourceRoute {
    SelectedSource,
    SourceBoundClaim,
    /// A selected native adapter supplies current layer/comparison
    /// observations and derived quality bases. It does not obtain eligibility
    /// from assessment prose or command-request booleans.
    LayerQuality,
}

#[derive(Debug, Clone, Copy)]
pub struct AssessmentLimits {
    pub max_input_bytes: usize,
    pub max_work: usize,
    pub batch: BatchBudget,
    pub deadline: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssessmentRefusal {
    InvalidInput(String),
    Unsupported(String),
    Budget,
    Cancelled,
    Deadline,
    Schema(ItemRefusal),
}

/// Private construction ensures this result always comes from the native
/// calculation. It is deliberately not usable as an authorization token.
#[derive(Debug, Clone)]
pub struct AssessmentMechanicsReport {
    admission: Value,
    scope: Value,
    required_sources: Vec<Value>,
    required_admissions: Vec<Value>,
    source_read_ready: bool,
    source_read_required: bool,
    input_sha256: Digest256,
    schema_binding: CutExecutionBinding,
    observed_now: String,
    work_used: usize,
    input_bytes_used: usize,
}
impl AssessmentMechanicsReport {
    pub fn current_admission(&self) -> &Value {
        &self.admission
    }
    pub fn scope(&self) -> &Value {
        &self.scope
    }
    pub fn required_sources(&self) -> &[Value] {
        &self.required_sources
    }
    pub fn required_admissions(&self) -> &[Value] {
        &self.required_admissions
    }
    pub fn source_read_ready(&self) -> bool {
        self.source_read_ready
    }
    /// The maintained describe response omits `source_read` entirely when
    /// no native binding is required. In particular Sign must not turn the
    /// vacuous true readiness of an empty closure into a present read gate.
    pub fn source_read_required(&self) -> bool {
        self.source_read_required
    }
    pub fn input_sha256(&self) -> Digest256 {
        self.input_sha256
    }
    pub fn schema_binding(&self) -> &CutExecutionBinding {
        &self.schema_binding
    }
    pub fn observed_now(&self) -> &str {
        &self.observed_now
    }
    /// Consumption by this exact evaluation. A protected native caller can
    /// subtract it from one operation-wide budget before evaluating another
    /// subject; reports do not reset the caller's shared budget.
    pub fn work_used(&self) -> usize {
        self.work_used
    }
    pub fn input_bytes_used(&self) -> usize {
        self.input_bytes_used
    }
}

type Result<T> = std::result::Result<T, AssessmentRefusal>;

struct Work<'a> {
    limits: AssessmentLimits,
    cancelled: &'a AtomicBool,
    bytes: usize,
    work: usize,
}
impl Work<'_> {
    fn tick(&mut self, count: usize) -> Result<()> {
        if self.cancelled.load(AtomicOrdering::Relaxed) {
            return Err(AssessmentRefusal::Cancelled);
        }
        if Instant::now() >= self.limits.deadline {
            return Err(AssessmentRefusal::Deadline);
        }
        self.work = self
            .work
            .checked_add(count)
            .filter(|n| *n <= self.limits.max_work)
            .ok_or(AssessmentRefusal::Budget)?;
        Ok(())
    }
    fn raw(&mut self, raw: &[u8]) -> Result<Value> {
        self.charge(raw.len())?;
        decoded(raw)
    }
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.tick(1)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_input_bytes)
            .ok_or(AssessmentRefusal::Budget)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Record {
    id: String,
    version: Value,
    body: Value,
    canonical: Vec<u8>,
    origin: Option<String>,
    reference: Value,
}
impl Record {
    fn read(input: &AssessmentRecordInput, work: &mut Work<'_>) -> Result<Self> {
        let envelope = work.raw(&input.envelope)?;
        let Some(fields) = envelope.as_object() else {
            return invalid("assessment owner envelope must be an object");
        };
        if fields.len() != 4
            || ["id", "version", "payload", "origin_id"]
                .iter()
                .any(|key| !fields.contains_key(*key))
        {
            return invalid("assessment owner envelope fields differ from maintained contract");
        }
        let id = text(&envelope, "id")?.to_owned();
        if stripped(&id)?.is_empty() {
            return invalid("record.id must be nonempty");
        }
        let original =
            parse_json(&input.envelope, JsonMode::PublishedStrict, raw_limits()).map_err(codec)?;
        let version = original
            .root()
            .object_get("version")
            .ok_or_else(|| bad("record.version missing"))?;
        match version {
            JsonValue::Number(n)
                if n.kind == JsonNumberKind::Int
                    && !n.lexeme.starts_with('-')
                    && n.lexeme != "0" =>
            {
                ()
            }
            _ => return invalid("record.version must be a positive integer"),
        }
        let payload = original
            .root()
            .object_get("payload")
            .ok_or_else(|| bad("record.payload missing"))?;
        if payload.as_object().is_none() {
            return invalid("record.payload must be a JSON object");
        }
        let canonical = canonical_bytes_v1(
            payload,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default(),
        )
        .map_err(codec)?;
        if canonical.len() > MAX_RECORD_BYTES {
            return Err(AssessmentRefusal::Budget);
        }
        let body: Value = serde_json::from_slice(&canonical)
            .map_err(|_| unsupported("native scalar representation"))?;
        let version = field(&envelope, "version")?.clone();
        let origin = match envelope.get("origin_id") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if !stripped(s)?.is_empty() => Some(s.clone()),
            _ => return invalid("record.origin_id must be nonempty when present"),
        };
        let reference = json!({"id":id,"version":version,"digest":Digest256::of_bytes(&canonical).to_prefixed()});
        Ok(Self {
            id,
            version,
            body,
            canonical,
            origin,
            reference,
        })
    }
    fn assessment(id: &str, body: &Value) -> Result<Value> {
        let canonical = canonical(body)?;
        if canonical.len() > MAX_RECORD_BYTES {
            return Err(AssessmentRefusal::Budget);
        }
        Ok(json!({"id":id,"version":1,"digest":Digest256::of_bytes(&canonical).to_prefixed()}))
    }
}

#[derive(Clone)]
struct Submission {
    body: Value,
    principal: String,
    executor: Value,
    scope: Option<Value>,
    canonical: Vec<u8>,
    schema_valid: bool,
}
impl Submission {
    fn read(input: &AssessmentSubmissionInput, work: &mut Work<'_>) -> Result<Self> {
        work.charge(input.principal_id.len())?;
        let body = work.raw(&input.assessment)?;
        if !body.is_object() {
            return invalid("submission assessment must be an object");
        }
        let executor = work.raw(&input.execution_profile)?;
        let scope = input
            .committed_scope
            .as_ref()
            .map(|raw| work.raw(raw))
            .transpose()?;
        if let Some(scope) = &scope {
            validate_scope(scope, false)?;
        }
        let canonical = canonical(&body)?;
        Ok(Self {
            body,
            principal: input.principal_id.clone(),
            executor,
            scope,
            canonical,
            schema_valid: false,
        })
    }
}

struct Context {
    subject: Record,
    scope: Value,
    languages: Vec<String>,
    required: Vec<Record>,
    admissions: Vec<Record>,
    source_ready: bool,
    positive: bool,
}
struct Engine {
    policy: Record,
    authorities: BTreeMap<String, Record>,
    competencies: BTreeMap<String, Record>,
    records: BTreeMap<String, Record>,
    profiles: BTreeMap<String, Value>,
}

/// Current-view calculation on exact raw owner inputs and current selected
/// grammar. No weaker/schema-free adapter is accepted. The protected caller
/// must preserve this input binding and reevaluate while holding its fences.
pub fn evaluate_current_assessment(
    input: &AssessmentReadInput,
    schemas: &mut CutWorkerSchemaExecutor,
    limits: AssessmentLimits,
    cancelled: &AtomicBool,
) -> Result<AssessmentMechanicsReport> {
    if limits.max_input_bytes == 0 || limits.max_work == 0 {
        return Err(AssessmentRefusal::Budget);
    }
    if schemas.execution_binding().schema_profile != FormatProfile::AssertedSourceCandidateV1 {
        return Err(unsupported(
            "assessment requires asserted source grammar formats",
        ));
    }
    if schemas.execution_binding().source_revision != input.source_revision {
        return invalid(
            "assessment source/schema cut revision differs from selected current source",
        );
    }
    if input.authorities.len() > MAX_ASSESSMENTS
        || input.competencies.len() > MAX_ASSESSMENTS
        || input
            .records
            .len()
            .checked_add(input.source_records.len())
            .and_then(|n| n.checked_add(input.native_records.len()))
            .is_none_or(|n| n > MAX_ASSESSMENTS)
        || input.required_source_refs.len() > MAX_ASSESSMENTS
        || input.required_admission_bases.len() > MAX_REQUIRED_ADMISSIONS
        || input
            .reviews
            .len()
            .checked_add(input.trusted_history.len())
            .is_none_or(|n| n > MAX_ASSESSMENTS)
    {
        return Err(AssessmentRefusal::Budget);
    }
    let mut work = Work {
        limits,
        cancelled,
        bytes: 0,
        work: 0,
    };
    match (input.source_route, input.layer_quality) {
        (AssessmentSourceRoute::LayerQuality, None) => {
            return Err(unsupported(
                "layer quality requires selected native comparison observations",
            ));
        }
        (
            AssessmentSourceRoute::SelectedSource | AssessmentSourceRoute::SourceBoundClaim,
            Some(_),
        ) => {
            return invalid("layer quality observation is outside the selected source route");
        }
        _ => (),
    }
    compare_time(&input.observed_now, &input.observed_now)?;
    work.charge(input.subject_id.len())?;
    work.charge(input.observed_now.len())?;
    let policy = Record::read(&input.policy, &mut work)?;
    let mut checks = vec![schema_check(&policy.id, &policy.canonical, "-policy")];
    let mut authorities = BTreeMap::new();
    let mut competencies = BTreeMap::new();
    let mut records = BTreeMap::new();
    for raw in &input.records {
        insert(&mut records, Record::read(raw, &mut work)?)?;
    }
    let inline_ids: BTreeSet<String> = records.keys().cloned().collect();
    let mut sourced = BTreeMap::new();
    let mut native = BTreeMap::new();
    for raw in &input.source_records {
        let record = Record::read(raw, &mut work)?;
        if inline_ids.contains(&record.id) {
            return invalid("inline owner record shadows selected source");
        }
        insert(&mut sourced, record.clone())?;
        insert(&mut records, record)?;
    }
    for raw in &input.native_records {
        let record = Record::read(raw, &mut work)?;
        if inline_ids.contains(&record.id) {
            return invalid("inline owner record shadows selected native source");
        }
        insert(&mut sourced, record.clone())?;
        insert(&mut native, record.clone())?;
        insert(&mut records, record)?;
    }
    insert(&mut records, policy.clone())?;
    for (inputs, target, suffix) in [
        (&input.authorities, &mut authorities, "-authority"),
        (&input.competencies, &mut competencies, "-competence"),
    ] {
        for raw in inputs {
            let record = Record::read(raw, &mut work)?;
            checks.push(schema_check(&record.id, &record.canonical, suffix));
            insert(target, record.clone())?;
            insert(&mut records, record)?;
        }
    }
    let owner_checks = checks.len();
    let mut history = input
        .trusted_history
        .iter()
        .map(|s| Submission::read(s, &mut work))
        .collect::<Result<Vec<_>>>()?;
    let mut reviews = input
        .reviews
        .iter()
        .map(|s| Submission::read(s, &mut work))
        .collect::<Result<Vec<_>>>()?;
    let mut submission_checks = Vec::new();
    for (ordinal, s) in history.iter().chain(&reviews).enumerate() {
        if s.canonical.len() > MAX_RECORD_BYTES {
            submission_checks.push(None);
        } else {
            submission_checks.push(Some(checks.len()));
            checks.push(schema_check(
                &format!("assessment:{ordinal}"),
                &s.canonical,
                "",
            ));
        }
    }
    let outcomes = check_all(schemas, &checks, &mut work)?;
    if outcomes[..owner_checks].iter().any(|valid| !valid) {
        return invalid("trusted assessment policy/authority/competence schema");
    }
    for (s, index) in history
        .iter_mut()
        .chain(&mut reviews)
        .zip(submission_checks)
    {
        s.schema_valid = index.is_some_and(|index| outcomes[index]);
    }
    for (name, collection) in [
        ("policy", vec![&policy]),
        ("authority", authorities.values().collect()),
        ("competence", competencies.values().collect()),
    ] {
        for record in collection {
            work.tick(1)?;
            if text(&record.body, &format!("{name}_id"))? != record.id
                || !py_equal(
                    field(&record.body, &format!("{name}_version"))?,
                    &record.version,
                )?
            {
                return invalid("trusted assessment envelope disagrees with its record");
            }
            if name != "policy"
                && compare_time(
                    text(&record.body, "valid_from")?,
                    text(&record.body, "valid_until")?,
                )? != Ordering::Less
            {
                return invalid("empty or reversed grant validity interval");
            }
        }
    }
    let mut profiles = BTreeMap::new();
    for profile in array(&policy.body, "profiles")? {
        work.tick(1)?;
        let id = text(profile, "profile_id")?.to_owned();
        if profiles.insert(id, profile.clone()).is_some() {
            return invalid("duplicate assessment profile");
        }
        if integer_cmp(
            field(profile, "min_independence_groups")?,
            field(profile, "min_reviewers")?,
        )? == Ordering::Greater
        {
            return invalid("independence groups exceed required reviewers");
        }
    }
    let scope = work.raw(&input.configured_scope)?;
    validate_scope(&scope, true)?;
    let subject = records
        .get(&input.subject_id)
        .cloned()
        .ok_or_else(|| bad("current subject missing"))?;
    if canonical(field(&scope, "record")?)? != canonical(&subject.reference)? {
        return invalid("configured subject is stale");
    }
    let mut required_index = BTreeMap::new();
    for raw in &input.required_source_refs {
        let reference = work.raw(raw)?;
        let record = resolve(&reference, &records)?
            .cloned()
            .ok_or_else(|| bad("required current source missing or stale"))?;
        if record.id == subject.id {
            return invalid("required source is the subject");
        }
        insert(&mut required_index, record)?;
    }
    let mut admission_index = BTreeMap::new();
    for raw in &input.required_admission_bases {
        let record = Record::read(raw, &mut work)?;
        if record.id == subject.id {
            return invalid("required admission is the subject");
        }
        if !field(&record.body, "can_use")?.is_boolean() {
            return invalid("quality basis eligibility must be an explicit boolean observation");
        }
        for limit in strings(&record.body, "limits")? {
            if stripped(&limit)?.is_empty() {
                return invalid("empty quality basis limit");
            }
        }
        // The basis must already be in the independently supplied current
        // engine snapshot. A dependency envelope cannot inject itself there.
        if resolve(&record.reference, &records)?.is_none() {
            return invalid("required quality basis absent from current owner records");
        }
        insert(&mut admission_index, record)?;
    }
    let required: Vec<Record> = required_index.into_values().collect();
    if input.source_route == AssessmentSourceRoute::SourceBoundClaim
        && !sourced.contains_key(&subject.id)
    {
        return invalid("source-bound Claim absent from selected source bindings");
    }
    if sourced.contains_key(&subject.id) {
        source_owned_scope(&subject, &scope)?;
    }
    if scope.get("form_language_context").is_some() {
        if !sourced.contains_key(&subject.id)
            || subject.body.get("schema_version").and_then(Value::as_str)
                != Some("tos_human_form_v1")
        {
            return invalid("form linguistic context is outside a selected source-form scope");
        }
        // Describe retains this authored form context. Its language grammar
        // is checked by the existing materialization route when consumed;
        // imposing that assertion here would strengthen the maintained read.
    }
    let (native_source_read_required, native_source_ready) = native_source_ready(
        &subject,
        &required,
        &sourced,
        &native,
        input.source_route,
        &mut work,
    )?;
    let (source_read_required, source_ready, positive) = match input.source_route {
        AssessmentSourceRoute::SelectedSource | AssessmentSourceRoute::SourceBoundClaim => {
            (native_source_read_required, native_source_ready, true)
        }
        AssessmentSourceRoute::LayerQuality => {
            let observation = input.layer_quality.ok_or_else(|| {
                unsupported("layer quality requires selected native comparison observations")
            })?;
            let comparison_ready =
                !observation.source_comparison_required || observation.source_comparison_present;
            (
                native_source_read_required || observation.source_comparison_required,
                native_source_ready && comparison_ready,
                observation.positive_use_allowed,
            )
        }
    };
    let context = Context {
        languages: strings(&scope, "languages")?,
        subject,
        scope,
        required,
        admissions: admission_index.into_values().collect(),
        source_ready,
        positive,
    };
    let engine = Engine {
        policy,
        authorities,
        competencies,
        records,
        profiles,
    };
    let admission =
        engine.evaluate(&context, &reviews, &history, &input.observed_now, &mut work)?;
    work.tick(1)?;
    let input_binding = input_binding(input);
    work.tick(1)?;
    Ok(AssessmentMechanicsReport {
        admission,
        scope: context.scope.clone(),
        required_sources: context
            .required
            .iter()
            .map(|r| r.reference.clone())
            .collect(),
        required_admissions: context
            .admissions
            .iter()
            .map(|r| r.reference.clone())
            .collect(),
        source_read_ready: context.source_ready,
        source_read_required,
        input_sha256: input_binding,
        schema_binding: schemas.execution_binding(),
        observed_now: input.observed_now.clone(),
        work_used: work.work,
        input_bytes_used: work.bytes,
    })
}

fn schema_check(path: &str, raw: &[u8], suffix: &str) -> CutSchemaCheck {
    CutSchemaCheck {
        path: path.to_owned(),
        raw: raw.to_vec(),
        contract: format!("ToS/contracts/knowledge-assessment{suffix}.schema.json"),
    }
}

fn check_all(
    schemas: &mut CutWorkerSchemaExecutor,
    checks: &[CutSchemaCheck],
    work: &mut Work<'_>,
) -> Result<Vec<bool>> {
    let budget = work.limits.batch;
    if budget.max_units == 0
        || budget.max_units > BatchBudget::MAX_UNITS
        || budget.max_total_raw_bytes == 0
        || budget.max_total_raw_bytes > BatchBudget::MAX_RAW_BYTES
    {
        return Err(AssessmentRefusal::Budget);
    }
    let mut results = Vec::with_capacity(checks.len());
    let mut start = 0;
    while start < checks.len() {
        work.tick(1)?;
        let mut end = start;
        let mut bytes = 0usize;
        while end < checks.len() && end - start < budget.max_units {
            let length = checks[end].raw.len();
            if length > MAX_RECORD_BYTES || length > budget.max_total_raw_bytes {
                return Err(AssessmentRefusal::Budget);
            }
            if length > budget.max_total_raw_bytes.saturating_sub(bytes) {
                break;
            }
            bytes += length;
            end += 1;
        }
        if end == start {
            return Err(AssessmentRefusal::Budget);
        }
        let batch = schemas
            .check_batch(
                &checks[start..end],
                budget,
                work.limits.deadline,
                work.cancelled,
            )
            .map_err(AssessmentRefusal::Schema)?;
        if batch.len() != end - start {
            return Err(unsupported("incomplete assessment schema coverage"));
        }
        results.extend(batch);
        start = end;
        work.tick(1)?;
    }
    Ok(results)
}

fn source_owned_scope(record: &Record, scope: &Value) -> Result<()> {
    let body = &record.body;
    if body.get("claim_id").is_some() && body.get("claim_version").is_some() {
        if field(scope, "assertion_layer")? != body.get("assertion_layer").unwrap_or(&Value::Null)
            || field(scope, "maker_id")?
                != body
                    .get("maker")
                    .and_then(|maker| maker.get("agent_ref"))
                    .unwrap_or(&Value::Null)
        {
            return invalid("configured scope disagrees with source-owned Claim layer or maker");
        }
        if matches!(
            body.get("predicate").and_then(Value::as_str),
            Some("identity_transition_proposal" | "subject_identity_transition_proposal")
        ) && (text(scope, "risk")? != "high" || text(scope, "requested_use")? != "research")
        {
            return invalid("identity proposal requires high-risk research assessment");
        }
    } else if body.get("schema_version").and_then(Value::as_str) == Some("tos_human_form_v1")
        && (text(scope, "assertion_layer")? != "human_projection"
            || scope.get("maker_id") != body.get("creator_id"))
    {
        return invalid("configured scope disagrees with source-owned form layer or maker");
    }
    Ok(())
}

fn native_source_ready(
    subject: &Record,
    required: &[Record],
    sourced: &BTreeMap<String, Record>,
    native: &BTreeMap<String, Record>,
    route: AssessmentSourceRoute,
    work: &mut Work<'_>,
) -> Result<(bool, bool)> {
    let rows: Vec<&Record> = if route == AssessmentSourceRoute::SourceBoundClaim {
        required.iter().collect()
    } else {
        sourced.values().collect()
    };
    let mut bindings: Vec<&Value> = rows
        .iter()
        .filter_map(|row| row.body.get("native_text_binding"))
        .collect();
    if subject.body.get("native_text_binding").is_some()
        || subject.body.get("schema_version").and_then(Value::as_str)
            == Some("tos_occurrence_description_record_v1")
    {
        bindings.push(
            subject
                .body
                .get("native_text_binding")
                .unwrap_or(&Value::Null),
        );
    }
    let required = !bindings.is_empty();
    let mut ready = true;
    for binding in bindings {
        work.tick(1)?;
        if !binding.is_object() {
            ready = false;
            continue;
        }
        let selected = canonical(binding)?;
        let mut found = false;
        for record in native.values() {
            work.tick(1)?;
            if record.body.get("content_verified").and_then(Value::as_bool) == Some(true)
                && canonical(record.body.get("native_binding").unwrap_or(&Value::Null))? == selected
            {
                found = true;
            }
        }
        ready &= found;
    }
    Ok((required, ready))
}

fn validate_scope(scope: &Value, configured: bool) -> Result<()> {
    let Some(object) = scope.as_object() else {
        return invalid("assessment scope must be an object");
    };
    let fields = if configured {
        vec![
            "record",
            "assertion_layer",
            "risk",
            "languages",
            "maker_id",
            "requested_use",
            "access_allowed",
        ]
    } else {
        vec![
            "assertion_layer",
            "risk",
            "languages",
            "maker_id",
            "requested_use",
        ]
    };
    if fields.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| {
            !fields.contains(&key.as_str()) && !(configured && key == "form_language_context")
        })
    {
        return invalid("assessment scope has missing or unrecognized fields");
    }
    if configured && !field(scope, "access_allowed")?.is_boolean() {
        return invalid("configured access observation must be boolean");
    }
    for key in ["assertion_layer", "risk", "maker_id", "requested_use"] {
        if text(scope, key)?.is_empty() {
            return invalid("assessment scope is incomplete");
        }
    }
    let languages = strings(scope, "languages")?;
    if languages.is_empty() || languages.iter().any(String::is_empty) {
        return invalid("assessment scope languages are incomplete");
    }
    Ok(())
}

fn scope_matches(scope: &Value, context: &Context) -> Result<bool> {
    for key in ["assertion_layer", "risk", "maker_id", "requested_use"] {
        if field(scope, key)? != field(&context.scope, key)? {
            return Ok(false);
        }
    }
    Ok(folded(&strings(scope, "languages")?)? == folded(&context.languages)?)
}
fn submission_scope_matches(s: &Submission, context: &Context) -> Result<bool> {
    s.scope
        .as_ref()
        .map(|scope| scope_matches(scope, context))
        .transpose()
        .map(|matches| matches.unwrap_or(true))
}
fn folded(values: &[String]) -> Result<BTreeSet<String>> {
    values
        .iter()
        .map(|value| {
            python_casefold_unicode16_v1(
                value,
                MAX_RECORD_BYTES,
                MAX_RECORD_BYTES,
                MAX_RECORD_BYTES,
            )
            .map_err(codec)
        })
        .collect()
}
fn languages_fit(required: &[String], allowed: &[String], work: &mut Work<'_>) -> Result<bool> {
    work.tick(
        required
            .len()
            .checked_add(allowed.len())
            .ok_or(AssessmentRefusal::Budget)?,
    )?;
    let accepted = folded(allowed)?;
    if accepted.contains("*") {
        return Ok(true);
    }
    Ok(folded(required)?.is_subset(&accepted))
}
fn stripped(value: &str) -> Result<&str> {
    python_strip_unicode16_v1(value, MAX_RECORD_BYTES).map_err(codec)
}

fn insert(index: &mut BTreeMap<String, Record>, record: Record) -> Result<()> {
    if index.get(&record.id).is_some_and(|old| old != &record) {
        return invalid("snapshot has conflicting current records");
    }
    index.insert(record.id.clone(), record);
    Ok(())
}
fn resolve<'a>(
    reference: &Value,
    records: &'a BTreeMap<String, Record>,
) -> Result<Option<&'a Record>> {
    let id = reference
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("exact reference lacks string id"))?;
    records
        .get(id)
        .map(|record| py_equal(&record.reference, reference).map(|same| same.then_some(record)))
        .transpose()
        .map(Option::flatten)
}

fn support_count(records: &[&Record], work: &mut Work<'_>) -> Result<usize> {
    let mut components: Vec<BTreeSet<String>> = Vec::new();
    for record in records {
        work.tick(1)?;
        let Some(origin) = &record.origin else {
            continue;
        };
        let mut keys: BTreeSet<String> = [
            format!("origin:{origin}"),
            format!("bytes:{}", text(&record.reference, "digest")?),
        ]
        .into_iter()
        .collect();
        let mut separate = Vec::new();
        for component in components {
            work.tick(1)?;
            if component.is_disjoint(&keys) {
                separate.push(component);
            } else {
                keys.extend(component);
            }
        }
        separate.push(keys);
        components = separate;
    }
    Ok(components.len())
}

fn independent_seats(
    options: &BTreeMap<String, BTreeSet<String>>,
    work: &mut Work<'_>,
) -> Result<usize> {
    let mut actor_group = BTreeMap::<String, String>::new();
    let mut group_actor = BTreeMap::<String, String>::new();
    for actor in options.keys() {
        let mut queue = VecDeque::from([actor.clone()]);
        let mut seen = BTreeSet::from([actor.clone()]);
        let mut parent = BTreeMap::<String, String>::new();
        let mut found = false;
        while let Some(current) = queue.pop_front() {
            for group in &options[&current] {
                work.tick(1)?;
                if parent.contains_key(group) {
                    continue;
                }
                parent.insert(group.clone(), current.clone());
                if !group_actor.contains_key(group) {
                    let mut step = Some(group.clone());
                    while let Some(group) = step {
                        work.tick(1)?;
                        let owner = parent[&group].clone();
                        let previous = actor_group.insert(owner.clone(), group.clone());
                        group_actor.insert(group, owner);
                        step = previous;
                    }
                    found = true;
                    break;
                }
                let next = group_actor[group].clone();
                if seen.insert(next.clone()) {
                    queue.push_back(next);
                }
            }
            if found {
                break;
            }
        }
    }
    Ok(actor_group.len())
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| bad(&format!("missing assessment field {key}")))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| bad(&format!("assessment field {key} must be string")))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    field(value, key)?
        .as_array()
        .ok_or_else(|| bad(&format!("assessment field {key} must be array")))
}
fn strings(value: &Value, key: &str) -> Result<Vec<String>> {
    array(value, key)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad("expected string array"))
        })
        .collect()
}
fn boolean(value: &Value, key: &str) -> Result<bool> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| bad("expected boolean observation"))
}
fn contains_text(value: &Value, key: &str, needle: &str) -> Result<bool> {
    Ok(strings(value, key)?.iter().any(|item| item == needle))
}
fn contains_value(value: &Value, key: &str, needle: &Value) -> Result<bool> {
    for item in array(value, key)? {
        if py_equal(item, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn is_positive(decision: &str) -> bool {
    matches!(decision, "admit" | "admit-with-limits")
}

fn raw_limits() -> JsonLimits {
    JsonLimits {
        max_bytes: 8 * MAX_RECORD_BYTES,
        ..JsonLimits::default()
    }
}
fn decoded(raw: &[u8]) -> Result<Value> {
    let canonical =
        canonical_raw_bytes_v1(raw, CanonicalProfile::SourceRecordDigestV1, raw_limits())
            .map_err(codec)?;
    serde_json::from_slice(&canonical).map_err(|_| unsupported("native scalar representation"))
}
fn canonical(value: &Value) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(value).map_err(|_| bad("non-JSON assessment value"))?;
    canonical_raw_bytes_v1(&raw, CanonicalProfile::SourceRecordDigestV1, raw_limits())
        .map_err(codec)
}
fn compare_time(left: &str, right: &str) -> Result<Ordering> {
    observed_instant_order(left, right).map_err(|error| match error {
        crate::retirement_rules::ObservedDateTimeError::Budget => AssessmentRefusal::Budget,
        crate::retirement_rules::ObservedDateTimeError::Invalid => {
            bad("assessment instant requires a valid aware timestamp")
        }
    })
}
fn bad(message: &str) -> AssessmentRefusal {
    AssessmentRefusal::InvalidInput(message.to_owned())
}
fn unsupported(message: &str) -> AssessmentRefusal {
    AssessmentRefusal::Unsupported(message.to_owned())
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(bad(message))
}
fn codec(error: tos_foundation::FoundationError) -> AssessmentRefusal {
    use tos_foundation::FoundationErrorCode as Code;
    match error.code {
        Code::BudgetExceeded => AssessmentRefusal::Budget,
        Code::InvalidUnicodeScalar | Code::UnsupportedFormat | Code::UnsupportedCanonicalNumber => {
            unsupported(&error.to_string())
        }
        _ => bad(&error.to_string()),
    }
}

/// Length-framed raw inputs, including trusted history purpose and native
/// source classification. This is an observation binding, not a grant.
fn input_binding(input: &AssessmentReadInput) -> Digest256 {
    fn push(hash: &mut Digest256Hasher, raw: &[u8]) {
        hash.update(&(raw.len() as u64).to_be_bytes());
        hash.update(raw);
    }
    fn records(hash: &mut Digest256Hasher, inputs: &[AssessmentRecordInput]) {
        hash.update(&(inputs.len() as u64).to_be_bytes());
        for input in inputs {
            push(hash, &input.envelope);
        }
    }
    fn submissions(hash: &mut Digest256Hasher, inputs: &[AssessmentSubmissionInput]) {
        hash.update(&(inputs.len() as u64).to_be_bytes());
        for input in inputs {
            push(hash, &input.assessment);
            push(hash, input.principal_id.as_bytes());
            push(hash, &input.execution_profile);
            hash.update(&[u8::from(input.committed_scope.is_some())]);
            if let Some(scope) = &input.committed_scope {
                push(hash, scope);
            }
        }
    }
    let mut hash = Digest256Hasher::new();
    hash.update(match input.source_route {
        AssessmentSourceRoute::LayerQuality => b"tos-current-assessment-input-v2\0",
        AssessmentSourceRoute::SelectedSource | AssessmentSourceRoute::SourceBoundClaim => {
            b"tos-current-assessment-input-v1\0"
        }
    });
    hash.update(input.source_revision.0.as_bytes());
    push(&mut hash, &input.policy.envelope);
    records(&mut hash, &input.authorities);
    records(&mut hash, &input.competencies);
    records(&mut hash, &input.records);
    records(&mut hash, &input.source_records);
    records(&mut hash, &input.native_records);
    hash.update(&[match input.source_route {
        AssessmentSourceRoute::SelectedSource => 0,
        AssessmentSourceRoute::SourceBoundClaim => 1,
        AssessmentSourceRoute::LayerQuality => 2,
    }]);
    push(&mut hash, input.subject_id.as_bytes());
    push(&mut hash, &input.configured_scope);
    hash.update(&(input.required_source_refs.len() as u64).to_be_bytes());
    for reference in &input.required_source_refs {
        push(&mut hash, reference);
    }
    records(&mut hash, &input.required_admission_bases);
    if let Some(observation) = input.layer_quality {
        hash.update(&[
            u8::from(observation.source_comparison_required),
            u8::from(observation.source_comparison_present),
            u8::from(observation.positive_use_allowed),
        ]);
    }
    submissions(&mut hash, &input.reviews);
    submissions(&mut hash, &input.trusted_history);
    push(&mut hash, input.observed_now.as_bytes());
    hash.finalize()
}

// Python's exact-ref dictionary equality is numeric equality (1 == 1.0),
// whereas evidence membership intentionally uses canonical bytes (1 != 1.0
// there). Do not round large owner integers through f64 to implement either.
pub(crate) fn py_equal(left: &Value, right: &Value) -> Result<bool> {
    match (left, right) {
        (Value::Number(_) | Value::Bool(_), Value::Number(_) | Value::Bool(_)) => {
            let li = integer_key(left)?;
            let ri = integer_key(right)?;
            match (li, ri) {
                (Some(a), Some(b)) => Ok(a == b),
                (None, None) => Ok(floating(left)? == floating(right)?),
                _ => Ok(false),
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (left, right) in a.iter().zip(b) {
                if !py_equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (key, left) in a {
                let Some(right) = b.get(key) else {
                    return Ok(false);
                };
                if !py_equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(left == right),
    }
}
fn floating(value: &Value) -> Result<f64> {
    let value = match value {
        Value::Bool(b) => return Ok(if *b { 1.0 } else { 0.0 }),
        Value::Number(number) => number
            .to_string()
            .parse::<f64>()
            .map_err(|_| unsupported("floating representation"))?,
        _ => return invalid("expected numeric value"),
    };
    if !value.is_finite() {
        return Err(unsupported("nonfinite numeric observation"));
    }
    Ok(value)
}
fn integer_key(value: &Value) -> Result<Option<String>> {
    let lexeme = match value {
        Value::Bool(value) => return Ok(Some(if *value { "1" } else { "0" }.to_owned())),
        Value::Number(number) => number.to_string(),
        _ => return invalid("expected numeric value"),
    };
    if !lexeme.bytes().any(|ch| matches!(ch, b'.' | b'e' | b'E')) {
        return Ok(Some(if lexeme == "-0" {
            "0".to_owned()
        } else {
            lexeme
        }));
    }
    let value = floating(value)?;
    if value.fract() != 0.0 {
        return Ok(None);
    }
    if value == 0.0 {
        return Ok(Some("0".to_owned()));
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = bits & ((1u64 << 52) - 1);
    let power = if exponent == 0 {
        -1074
    } else {
        mantissa |= 1u64 << 52;
        exponent - 1023 - 52
    };
    let mut digits: Vec<u8>;
    if power < 0 {
        let shift = (-power) as u32;
        if shift >= 64 || mantissa & ((1u64 << shift) - 1) != 0 {
            return Ok(None);
        }
        digits = (mantissa >> shift)
            .to_string()
            .bytes()
            .map(|byte| byte - b'0')
            .collect();
    } else {
        digits = mantissa
            .to_string()
            .bytes()
            .map(|byte| byte - b'0')
            .collect();
        for _ in 0..power {
            let mut carry = 0u8;
            for digit in digits.iter_mut().rev() {
                let next = *digit * 2 + carry;
                *digit = next % 10;
                carry = next / 10;
            }
            if carry != 0 {
                digits.insert(0, carry);
            }
        }
    }
    let mut out = String::with_capacity(digits.len() + 1);
    if bits >> 63 != 0 {
        out.push('-');
    }
    for digit in digits {
        out.push(char::from(b'0' + digit));
    }
    Ok(Some(out))
}
fn integer_cmp(left: &Value, right: &Value) -> Result<Ordering> {
    let left = integer_key(left)?.ok_or_else(|| bad("expected integer threshold"))?;
    let right = integer_key(right)?.ok_or_else(|| bad("expected integer threshold"))?;
    let negative_left = left.starts_with('-');
    let negative_right = right.starts_with('-');
    if negative_left != negative_right {
        return Ok(if negative_left {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let l = left.trim_start_matches('-');
    let r = right.trim_start_matches('-');
    let order = l.len().cmp(&r.len()).then_with(|| l.cmp(r));
    Ok(if negative_left {
        order.reverse()
    } else {
        order
    })
}
fn meets(count: usize, threshold: &Value) -> Result<bool> {
    Ok(integer_cmp(&json!(count), threshold)? != Ordering::Less)
}

impl Engine {
    fn qualify(
        &self,
        submission: &Submission,
        context: &Context,
        now: &str,
        work: &mut Work<'_>,
    ) -> Result<Vec<String>> {
        work.tick(1)?;
        if !submission.schema_valid {
            return Ok(vec!["assessment.schema".into()]);
        }
        let a = &submission.body;
        let mut reasons = BTreeSet::new();
        macro_rules! reason {
            ($condition:expr,$name:expr) => {
                if $condition {
                    reasons.insert($name.to_owned());
                }
            };
        }
        reason!(
            !py_equal(field(a, "subject")?, &context.subject.reference)?
                || resolve(field(a, "subject")?, &self.records)?.is_none(),
            "subject.stale-or-mismatched"
        );
        reason!(
            !py_equal(field(a, "policy")?, &self.policy.reference)?,
            "policy.stale-or-mismatched"
        );
        let reviewer = field(a, "reviewer")?;
        reason!(
            submission.principal != text(reviewer, "actor_id")?,
            "reviewer.authentication"
        );
        reason!(
            context.scope.get("access_allowed").and_then(Value::as_bool) != Some(true),
            "subject.access-denied"
        );
        reason!(!context.source_ready, "subject.exact-source-unverified");
        reason!(
            submission
                .scope
                .as_ref()
                .map(|scope| scope_matches(scope, context))
                .transpose()?
                .is_some_and(|matches| !matches),
            "assessment.committed-scope-mismatch"
        );
        reason!(
            context.languages.is_empty() || text(&context.scope, "maker_id")?.is_empty(),
            "subject.scope-incomplete"
        );
        let issued = text(a, "issued_at")?;
        reason!(
            compare_time(issued, now)? == Ordering::Greater,
            "assessment.future"
        );
        let profile = self.profiles.get(text(a, "profile_id")?);
        let grant = resolve(field(a, "authority")?, &self.authorities)?;
        let competence = resolve(field(a, "competence")?, &self.competencies)?;
        reason!(profile.is_none(), "profile.unregistered");
        reason!(grant.is_none(), "authority.stale-or-missing");
        reason!(competence.is_none(), "competence.stale-or-missing");
        let (Some(profile), Some(grant), Some(competence)) = (profile, grant, competence) else {
            return Ok(reasons.into_iter().collect());
        };
        let authority = &grant.body;
        let calibration = &competence.body;
        let mut languages = context.languages.clone();
        languages.push(text(a, "language")?.into());
        let layer = text(&context.scope, "assertion_layer")?;
        let risk = text(&context.scope, "risk")?;
        let use_ = text(&context.scope, "requested_use")?;
        let actor = text(reviewer, "actor_id")?;
        reason!(
            !contains_text(profile, "assertion_layers", layer)?
                || !contains_text(profile, "risk_tiers", risk)?
                || !contains_text(profile, "uses", use_)?
                || !contains_text(profile, "reviewer_kinds", text(reviewer, "kind")?)?
                || !languages_fit(&languages, &strings(profile, "languages")?, work)?,
            "profile.outside-scope"
        );
        reason!(
            !boolean(profile, "allow_self_review")? && text(&context.scope, "maker_id")? == actor,
            "reviewer.self-review"
        );
        reason!(
            text(authority, "actor_id")? != actor
                || text(authority, "actor_kind")? != text(reviewer, "kind")?
                || !py_equal(field(authority, "policy")?, &self.policy.reference)?
                || !contains_value(authority, "competence_refs", field(a, "competence")?)?
                || !contains_text(authority, "decisions", text(a, "decision")?)?
                || !contains_text(authority, "uses", use_)?
                || !strings(authority, "subject_prefixes")?
                    .iter()
                    .any(|prefix| context.subject.id.starts_with(prefix)),
            "authority.binding-or-scope"
        );
        for (prefix, record, state) in [
            ("authority", authority, "active"),
            ("competence", calibration, "verified"),
        ] {
            work.tick(1)?;
            reason!(
                text(record, "state")? != state,
                &format!("{prefix}.inactive")
            );
            reason!(
                compare_time(text(record, "valid_from")?, issued)? == Ordering::Greater
                    || compare_time(issued, now)? == Ordering::Greater
                    || compare_time(now, text(record, "valid_until")?)? != Ordering::Less,
                &format!("{prefix}.outside-validity")
            );
            reason!(
                text(record, "actor_id")? != actor
                    || !contains_text(record, "profile_ids", text(a, "profile_id")?)?
                    || !contains_text(record, "assertion_layers", layer)?
                    || !languages_fit(&languages, &strings(record, "languages")?, work)?,
                &format!("{prefix}.outside-scope")
            );
        }
        let mut evidence_stale = false;
        for reference in array(calibration, "evidence_refs")? {
            work.tick(1)?;
            evidence_stale |= resolve(reference, &self.records)?.is_none();
        }
        reason!(evidence_stale, "competence.evidence-stale");
        let method = field(a, "method")?;
        let executor = resolve(&submission.executor, &self.records)?;
        let qualified_execution = if let Some(executor) = executor {
            py_equal(field(method, "execution_profile")?, &submission.executor)?
                && contains_value(calibration, "execution_profiles", &submission.executor)?
                && py_equal(
                    field(method, "procedure_ref")?,
                    executor.body.get("procedure_ref").unwrap_or(&Value::Null),
                )?
                && py_equal(
                    field(method, "model_ref")?,
                    executor.body.get("model_ref").unwrap_or(&Value::Null),
                )?
        } else {
            false
        };
        reason!(!qualified_execution, "method.unqualified-execution");
        let evidence = array(a, "evidence")?;
        let mut evidence_refs = BTreeSet::new();
        for item in evidence {
            work.tick(1)?;
            evidence_refs.insert(canonical(field(item, "record")?)?);
        }
        for dependency in context.required.iter().chain(&context.admissions) {
            work.tick(1)?;
            reason!(
                resolve(&dependency.reference, &self.records)?.is_none(),
                "source-dependency.stale-or-missing"
            );
            reason!(
                !evidence_refs.contains(&canonical(&dependency.reference)?),
                "evidence.required-source-omitted"
            );
        }
        let positive = is_positive(text(a, "decision")?);
        reason!(
            positive
                && (!context.positive
                    || context.admissions.iter().any(|r| r
                        .body
                        .get("can_use")
                        .and_then(Value::as_bool)
                        != Some(true))),
            "source-quality.not-admitted"
        );
        let mut supporting = Vec::new();
        for item in evidence {
            work.tick(1)?;
            match resolve(field(item, "record")?, &self.records)? {
                None => {
                    reasons.insert("evidence.stale-or-missing".into());
                }
                Some(record) if text(item, "stance")? == "supports" => {
                    reason!(record.id == context.subject.id, "evidence.circular-support");
                    reason!(record.origin.is_none(), "evidence.origin-missing");
                    supporting.push(record);
                }
                _ => (),
            }
        }
        if positive {
            reason!(
                !meets(
                    support_count(&supporting, work)?,
                    field(profile, "min_supporting_origins")?
                )?,
                "evidence.insufficient-origins"
            );
            reason!(
                boolean(profile, "require_counterevidence_search")?
                    && text(field(a, "counterevidence_search")?, "status")? != "searched",
                "evidence.countersearch-required"
            );
        }
        Ok(reasons.into_iter().collect())
    }

    fn evaluate(
        &self,
        context: &Context,
        reviews: &[Submission],
        trusted_history: &[Submission],
        now: &str,
        work: &mut Work<'_>,
    ) -> Result<Value> {
        let mut history = BTreeMap::<String, &Submission>::new();
        let mut refs = BTreeMap::<String, Value>::new();
        for s in trusted_history {
            work.tick(1)?;
            let a = &s.body;
            if !s.schema_valid {
                return invalid("trusted history assessment schema");
            }
            if text(field(a, "subject")?, "id")? != context.subject.id
                || text(field(a, "reviewer")?, "actor_id")? != s.principal
                || !py_equal(
                    field(field(a, "method")?, "execution_profile")?,
                    &s.executor,
                )?
                || compare_time(text(a, "issued_at")?, now)? == Ordering::Greater
            {
                return invalid("trusted history has an inconsistent binding");
            }
            let id = text(a, "assessment_id")?.to_owned();
            let reference = Record::assessment(&id, a)?;
            if refs
                .get(&id)
                .map(|r| py_equal(r, &reference))
                .transpose()?
                .is_some_and(|same| !same)
            {
                return invalid("trusted history identity collision");
            }
            history.insert(id.clone(), s);
            refs.insert(id, reference);
        }
        let mut permanent = BTreeSet::new();
        for s in history.values() {
            for reference in array(&s.body, "supersedes")? {
                work.tick(1)?;
                let target = text(reference, "id")?;
                let Some(previous) = history.get(target) else {
                    return invalid("trusted history missing exact supersession target");
                };
                if !refs
                    .get(target)
                    .map(|r| py_equal(r, reference))
                    .transpose()?
                    .unwrap_or(false)
                {
                    return invalid("trusted history missing exact supersession target");
                }
                if submission_scope_matches(s, context)?
                    && submission_scope_matches(previous, context)?
                {
                    permanent.insert(target.to_owned());
                }
            }
        }
        let mut grouped = BTreeMap::<String, Vec<&Submission>>::new();
        for s in trusted_history.iter().chain(reviews) {
            let id = s
                .body
                .get("assessment_id")
                .and_then(Value::as_str)
                .unwrap_or("<invalid-id>");
            grouped.entry(id.to_owned()).or_default().push(s);
        }
        let mut qualified = BTreeMap::<String, &Submission>::new();
        let mut invalids = BTreeMap::<String, Vec<String>>::new();
        for (id, variants) in grouped {
            work.tick(1)?;
            let mut signatures = BTreeSet::new();
            for s in &variants {
                work.tick(1)?;
                signatures.insert(canonical(&json!({"assessment":s.body,"principal_id":s.principal,"execution_profile":s.executor}))?);
            }
            let failure = if signatures.iter().any(|s| s.len() > MAX_RECORD_BYTES) {
                Some("assessment.size-limit")
            } else if signatures.len() != 1 {
                Some("assessment.identity-collision")
            } else {
                None
            };
            if let Some(failure) = failure {
                invalids.insert(id, vec![failure.into()]);
                continue;
            }
            let s = variants[0];
            let reasons = self.qualify(s, context, now, work)?;
            if !reasons.is_empty() {
                invalids.insert(id, reasons);
            } else {
                refs.insert(id.clone(), Record::assessment(&id, &s.body)?);
                qualified.insert(id, s);
            }
        }
        let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        for (id, s) in &qualified {
            let deps = dependencies.entry(id.clone()).or_default();
            for reference in array(&s.body, "supersedes")? {
                work.tick(1)?;
                let target = text(reference, "id")?;
                let previous = qualified.get(target).or_else(|| history.get(target));
                let exact_target = refs
                    .get(target)
                    .map(|r| py_equal(r, reference))
                    .transpose()?
                    .unwrap_or(false);
                let valid = if let Some(previous) = previous {
                    exact_target
                        && target != id
                        && py_equal(
                            field(&previous.body, "subject")?,
                            field(&s.body, "subject")?,
                        )?
                        && compare_time(
                            text(&previous.body, "issued_at")?,
                            text(&s.body, "issued_at")?,
                        )? != Ordering::Greater
                } else {
                    false
                };
                if !valid {
                    invalids
                        .entry(id.clone())
                        .or_default()
                        .push("supersession.invalid-target".into());
                    continue;
                }
                let previous = previous.expect("checked target");
                if !submission_scope_matches(previous, context)? {
                    invalids
                        .entry(id.clone())
                        .or_default()
                        .push("supersession.outside-committed-scope".into());
                }
                let authority = &self.authorities[text(field(&s.body, "authority")?, "id")?].body;
                if previous.principal != s.principal && !boolean(authority, "can_supersede_others")?
                {
                    invalids
                        .entry(id.clone())
                        .or_default()
                        .push("supersession.unauthorized".into());
                }
                deps.insert(target.to_owned());
            }
        }
        let mut remaining: BTreeSet<String> = qualified
            .keys()
            .filter(|id| !invalids.contains_key(*id))
            .cloned()
            .collect();
        let mut ordered = Vec::new();
        while !remaining.is_empty() {
            work.tick(1)?;
            let mut children = BTreeSet::new();
            for id in &remaining {
                for target in &dependencies[id] {
                    work.tick(1)?;
                    if invalids.contains_key(target) && !history.contains_key(target) {
                        children.insert(id.clone());
                    }
                }
            }
            for id in children {
                remaining.remove(&id);
                invalids.insert(id, vec!["supersession.invalid-target".into()]);
            }
            let mut ready = Vec::new();
            for id in &remaining {
                let mut waiting = false;
                for target in &dependencies[id] {
                    work.tick(1)?;
                    waiting |= remaining.contains(target);
                }
                if !waiting {
                    ready.push(id.clone());
                }
            }
            if ready.is_empty() {
                for id in &remaining {
                    invalids.insert(
                        id.clone(),
                        vec!["supersession.cycle-or-invalid-ancestor".into()],
                    );
                }
                break;
            }
            for id in ready {
                remaining.remove(&id);
                ordered.push(id);
            }
        }
        let mut superseded = permanent;
        for id in &ordered {
            superseded.extend(dependencies[id].iter().cloned());
        }
        let active: Vec<&Submission> = ordered
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|id| !superseded.contains(*id))
            .map(|id| qualified[id])
            .collect();
        let judgments: Vec<&Submission> = active
            .iter()
            .copied()
            .filter(|s| s.body["decision"] != "withdraw")
            .collect();
        let positives: Vec<&Submission> = judgments
            .iter()
            .copied()
            .filter(|s| is_positive(s.body["decision"].as_str().unwrap_or("")))
            .collect();
        let negatives = judgments.iter().any(|s| s.body["decision"] == "reject");
        let disputes = judgments.iter().any(|s| s.body["decision"] == "dispute");
        let mut quorum = false;
        for (id, profile) in &self.profiles {
            let mut options = BTreeMap::<String, BTreeSet<String>>::new();
            for s in &positives {
                work.tick(1)?;
                if text(&s.body, "profile_id")? == id {
                    let group = text(
                        &self.authorities[text(field(&s.body, "authority")?, "id")?].body,
                        "independence_group",
                    )?;
                    options
                        .entry(s.principal.clone())
                        .or_default()
                        .insert(group.to_owned());
                }
            }
            if meets(options.len(), field(profile, "min_reviewers")?)?
                && meets(
                    independent_seats(&options, work)?,
                    field(profile, "min_independence_groups")?,
                )?
            {
                quorum = true;
            }
        }
        let mut limits = BTreeSet::new();
        let mut kinds = BTreeSet::new();
        for s in &judgments {
            work.tick(1)?;
            limits.extend(strings(&s.body, "limits")?);
            kinds.insert(text(field(&s.body, "reviewer")?, "kind")?.to_owned());
        }
        for dependency in &context.admissions {
            limits.extend(strings(&dependency.body, "limits")?);
        }
        let status = if disputes || (!positives.is_empty() && negatives) {
            "disputed"
        } else if negatives {
            "rejected"
        } else if quorum {
            if limits.is_empty() {
                "admitted"
            } else {
                "admitted-with-limits"
            }
        } else if !judgments.is_empty() {
            "deferred"
        } else {
            "unreviewed"
        };
        let active_refs: Vec<Value> = active
            .iter()
            .map(|s| refs[s.body["assessment_id"].as_str().expect("qualified id")].clone())
            .collect();
        let superseded_refs: Vec<Value> = superseded.iter().map(|id| refs[id].clone()).collect();
        let invalids:Vec<Value> = invalids.into_iter().map(|(id,reasons)| json!({"assessment_id":id,"reasons":reasons.into_iter().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>()})).collect();
        Ok(
            json!({"schema_version":"tos_knowledge_admission_v1","subject":context.subject.reference,
            "policy":self.policy.reference,"use":text(&context.scope,"requested_use")?,"status":status,
            "can_use":matches!(status,"admitted"|"admitted-with-limits"),"is_semantic_evaluation":false,
            "reviewer_kinds":kinds.into_iter().collect::<Vec<_>>(),"assessment_refs":active_refs,
            "superseded_assessment_refs":superseded_refs,"invalid_assessments":invalids,"limits":limits.into_iter().collect::<Vec<_>>() }),
        )
    }
}
