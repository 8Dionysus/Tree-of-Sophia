//! Composed Claim successor/history over the existing claim_v1 source wording.
//! The fixture contains a v2 Claim but no Claim predecessor package: that exact
//! incomplete input must refuse. The positive baseline below is explicitly a
//! synthetic initial version of its body, not invented historical v1 evidence.
use super::command_form_cases::{context, open_cut, schemas, successor};
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_command::source_claims::{
    CLAIM_GROUNDING_RULE_INPUTS, CLAIM_REVISION_RULE_INPUTS,
    execute_isolated_claim_creation_from_captures, execute_isolated_claim_revision_from_captures,
    prepare_isolated_claim_creation_from_captures, prepare_isolated_claim_revision_from_captures,
    run_claim_command, run_claim_command_from_captures,
};
use tos_command::source_command::{PreparedCommand, SourceCommandError};
use tos_command::source_forms::metadata_subject;
use tos_command::source_operation::{SourceOperationError, bind_selected_candidate};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::operation::OperationLimits;
use tos_validation::{FormatProfile, executor::ExecutorBudget};

fn selected_context(
    files: &BTreeMap<String, Vec<u8>>,
    software: &BTreeMap<String, Vec<u8>>,
    configuration: &Value,
    request: &Value,
    base: SourceRevision,
) -> tos_command::source_command::CommandContext {
    let mut inputs = files.clone();
    for (name, raw) in software {
        assert!(inputs.insert(name.clone(), raw.clone()).is_none());
    }
    context(
        &inputs,
        serde_json::to_vec(configuration).unwrap(),
        serde_json::to_vec(request).unwrap(),
        base,
    )
}
fn checked_command(
    ctx: &tos_command::source_command::CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PreparedCommand, SourceCommandError> {
    run_claim_command_from_captures(ctx, cut, software, components, worker, deadline, cancel)
}

// Private, input-only reference for the three simple Claim comparisons below.
// This prices logical test construction and cooperative work, not allocator/RSS
// or delegated native owner work. The richer identity/order readers stay separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClaimReferenceError {
    Limit,
    Deadline,
    Cancelled,
    InvalidInput,
    UnsupportedProfile,
    MissingSource,
    AmbiguousIdentity,
    WitnessMismatch,
}
type ClaimReferenceResult<T> = Result<T, ClaimReferenceError>;
struct ClaimReferenceBudget<'a> {
    state: usize,
    visits: usize,
    byte_work: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl<'a> ClaimReferenceBudget<'a> {
    fn new(deadline: Instant, cancelled: &'a AtomicBool) -> Self {
        Self {
            state: 8_388_608, // existing selected-member ceiling; never replenished
            visits: JsonLimits::default().max_visits,
            byte_work: 33_554_432, // existing complete Claim source ceiling
            deadline,
            cancelled,
        }
    }
    fn active(&self) -> ClaimReferenceResult<()> {
        if self.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ClaimReferenceError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(ClaimReferenceError::Deadline);
        }
        Ok(())
    }
    fn work(&mut self, bytes: usize, visits: usize) -> ClaimReferenceResult<()> {
        self.active()?;
        self.visits = self
            .visits
            .checked_sub(visits)
            .ok_or(ClaimReferenceError::Limit)?;
        self.byte_work = self
            .byte_work
            .checked_sub(bytes)
            .ok_or(ClaimReferenceError::Limit)?;
        Ok(())
    }
    fn own(&mut self, bytes: usize) -> ClaimReferenceResult<()> {
        self.work(0, 8)?;
        self.state = self
            .state
            .checked_sub(bytes)
            .ok_or(ClaimReferenceError::Limit)?;
        Ok(())
    }
    fn same_bytes(&mut self, left: &[u8], right: &[u8]) -> ClaimReferenceResult<bool> {
        self.work(
            left.len()
                .checked_add(right.len())
                .ok_or(ClaimReferenceError::Limit)?,
            8,
        )?;
        if left.len() != right.len() {
            return Ok(false);
        }
        for (a, b) in left.chunks(4096).zip(right.chunks(4096)) {
            self.work(0, 1)?;
            if a != b {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn codec_state(&mut self, bytes: usize) -> ClaimReferenceResult<usize> {
        let reserve = bytes
            .checked_mul(128)
            .and_then(|n| n.checked_add(16_384))
            .ok_or(ClaimReferenceError::Limit)?;
        self.own(reserve)?; // includes bridge/error workspace before codec entry
        Ok(reserve)
    }
    fn parse(&mut self, raw: &[u8]) -> ClaimReferenceResult<JsonValue> {
        self.parse_reserved(raw, 128)
    }
    fn parse_reserved(&mut self, raw: &[u8], scale: usize) -> ClaimReferenceResult<JsonValue> {
        use std::cell::Cell;
        use tos_foundation::{FoundationError, FoundationErrorCode};
        self.work(raw.len(), 16)?;
        if raw.len() > JsonLimits::default().max_bytes {
            return Err(ClaimReferenceError::Limit);
        }
        self.work(0, 16)?;
        if scale != 16 && scale != 128 {
            return Err(ClaimReferenceError::UnsupportedProfile);
        }
        let state = raw
            .len()
            .checked_mul(scale)
            .and_then(|n| n.checked_add(16_384))
            .ok_or(ClaimReferenceError::Limit)?;
        self.own(state)?;
        let limits = JsonLimits {
            max_visits: self.visits,
            ..JsonLimits::default()
        };
        let first = Cell::new(None);
        let deadline = self.deadline;
        let cancelled = self.cancelled;
        let mut check = || {
            let error = if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                Some(ClaimReferenceError::Cancelled)
            } else if Instant::now() >= deadline {
                Some(ClaimReferenceError::Deadline)
            } else {
                None
            };
            if let Some(error) = error {
                if first.get().is_none() {
                    first.set(Some(error));
                }
                Err(FoundationError {
                    code: FoundationErrorCode::BudgetExceeded,
                    byte_offset: None,
                    detail: String::new(),
                })
            } else {
                Ok(())
            }
        };
        let mut admit = |bytes, visits| {
            self.work(bytes, visits).map_err(|e| {
                if first.get().is_none() {
                    first.set(Some(e));
                }
                FoundationError {
                    code: FoundationErrorCode::BudgetExceeded,
                    byte_offset: None,
                    detail: String::new(),
                }
            })
        };
        let parsed = tos_foundation::parse_json_with_state_budget_and_admission(
            raw,
            JsonMode::PublishedStrict,
            limits,
            state,
            &mut check,
            &mut admit,
        );
        if let Some(error) = first.get() {
            return Err(error);
        }
        parsed.map(|d| d.into_root()).map_err(|e| {
            if e.code == FoundationErrorCode::BudgetExceeded {
                ClaimReferenceError::Limit
            } else {
                ClaimReferenceError::InvalidInput
            }
        })
    }
    fn canonical(&mut self, value: &JsonValue, raw_bound: usize) -> ClaimReferenceResult<Vec<u8>> {
        use std::cell::Cell;
        use tos_foundation::{FoundationError, FoundationErrorCode};
        let state = self.codec_state(raw_bound)?;
        let limits = JsonLimits {
            max_visits: self.visits,
            ..JsonLimits::default()
        };
        let first = Cell::new(None);
        let deadline = self.deadline;
        let cancelled = self.cancelled;
        let mut check = || {
            let error = if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                Some(ClaimReferenceError::Cancelled)
            } else if Instant::now() >= deadline {
                Some(ClaimReferenceError::Deadline)
            } else {
                None
            };
            if let Some(error) = error {
                if first.get().is_none() {
                    first.set(Some(error));
                }
                Err(FoundationError {
                    code: FoundationErrorCode::BudgetExceeded,
                    byte_offset: None,
                    detail: String::new(),
                })
            } else {
                Ok(())
            }
        };
        let mut admit = |bytes, visits| {
            self.work(bytes, visits).map_err(|e| {
                if first.get().is_none() {
                    first.set(Some(e));
                }
                FoundationError {
                    code: FoundationErrorCode::BudgetExceeded,
                    byte_offset: None,
                    detail: String::new(),
                }
            })
        };
        let encoded = tos_foundation::canonical_bytes_v1_with_state_budget_and_visits_and_admission(
            value,
            CanonicalProfile::SourceCommandInputV1,
            limits,
            state,
            &mut check,
            &mut admit,
        );
        if let Some(error) = first.get() {
            return Err(error);
        }
        encoded.map(|v| v.0).map_err(|e| {
            if e.code == FoundationErrorCode::BudgetExceeded {
                ClaimReferenceError::Limit
            } else {
                ClaimReferenceError::InvalidInput
            }
        })
    }
}
#[derive(Clone, Copy)]
struct DeclaredClaimObject<'a> {
    id: &'a str,
    path: &'a str,
    raw: &'a [u8],
    identity_field: &'a str,
}
struct DeclaredRepoEvidence<'a> {
    reference: &'a str,
    path: &'a str,
    raw: &'a [u8],
}
struct DeclaredSimpleClaimInputs<'a> {
    objects: &'a [DeclaredClaimObject<'a>],
    evidence: &'a [DeclaredRepoEvidence<'a>],
    claims: &'a [&'a Value],
}
fn claim_reference_field<'a>(
    value: &'a JsonValue,
    key: &str,
    b: &mut ClaimReferenceBudget<'_>,
) -> ClaimReferenceResult<Option<&'a JsonValue>> {
    let fields = value.as_object().ok_or(ClaimReferenceError::InvalidInput)?;
    for (name, child) in fields {
        b.work(
            name.units()
                .len()
                .checked_mul(2)
                .ok_or(ClaimReferenceError::Limit)?,
            4,
        )?;
        if name.as_str() == Some(key) {
            return Ok(Some(child));
        }
    }
    Ok(None)
}
fn claim_reference_path(path: &str, b: &mut ClaimReferenceBudget<'_>) -> ClaimReferenceResult<()> {
    b.work(path.len(), 8)?;
    if !path.starts_with("ToS/")
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ClaimReferenceError::InvalidInput);
    }
    Ok(())
}
fn claim_reference_same(
    value: &JsonValue,
    declared: &Value,
    b: &mut ClaimReferenceBudget<'_>,
    depth: usize,
) -> ClaimReferenceResult<bool> {
    b.work(0, 8)?;
    if depth > JsonLimits::default().max_depth {
        return Err(ClaimReferenceError::Limit);
    }
    Ok(match (value, declared) {
        (JsonValue::Null, Value::Null) => true,
        (JsonValue::Bool(a), Value::Bool(c)) => a == c,
        (JsonValue::String(a), Value::String(c)) => {
            b.work(
                a.units()
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(c.len()))
                    .ok_or(ClaimReferenceError::Limit)?,
                4,
            )?;
            a.as_str() == Some(c.as_str())
        }
        (JsonValue::Number(a), Value::Number(c)) => {
            b.work(a.lexeme.len(), 4)?;
            if let Some(c) = c.as_u64() {
                a.lexeme.parse::<u64>().ok() == Some(c)
            } else if let Some(c) = c.as_i64() {
                a.lexeme.parse::<i64>().ok() == Some(c)
            } else {
                return Err(ClaimReferenceError::UnsupportedProfile);
            }
        }
        (JsonValue::Array(a), Value::Array(c)) => {
            if a.len() != c.len() {
                false
            } else {
                let mut equal = true;
                for (a, c) in a.iter().zip(c) {
                    equal &= claim_reference_same(a, c, b, depth + 1)?;
                }
                equal
            }
        }
        (JsonValue::Object(a), Value::Object(c)) => {
            if a.len() != c.len() {
                false
            } else {
                let mut equal = true;
                for (key, child) in c {
                    b.work(key.len(), 4)?;
                    if let Some(a) = claim_reference_field(value, key, b)? {
                        equal &= claim_reference_same(a, child, b, depth + 1)?;
                    } else {
                        equal = false;
                    }
                }
                equal
            }
        }
        _ => false,
    })
}
fn check_simple_claim_selected_inputs(
    inputs: &DeclaredSimpleClaimInputs<'_>,
    selected: &BTreeMap<String, Vec<u8>>,
    b: &mut ClaimReferenceBudget<'_>,
) -> ClaimReferenceResult<()> {
    for (path, raw) in inputs
        .objects
        .iter()
        .map(|v| (v.path, v.raw))
        .chain(inputs.evidence.iter().map(|v| (v.path, v.raw)))
    {
        claim_reference_path(path, b)?;
        b.work(
            raw.len(),
            selected
                .len()
                .checked_add(8)
                .ok_or(ClaimReferenceError::Limit)?,
        )?;
        let actual = selected
            .get(path)
            .ok_or(ClaimReferenceError::MissingSource)?;
        if !b.same_bytes(actual, raw)? {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    Ok(())
}
fn expected_simple_claim_bindings(
    inputs: &DeclaredSimpleClaimInputs<'_>,
    b: &mut ClaimReferenceBudget<'_>,
) -> ClaimReferenceResult<Value> {
    b.work(0, 128)?;
    // The selected simple profile has two independently declared endpoints and
    // one repository evidence. No profile fallback or actual-result membership.
    if inputs.objects.len() != 2 || inputs.evidence.len() != 1 || inputs.claims.len() != 1 {
        return Err(ClaimReferenceError::UnsupportedProfile);
    }
    let claim = inputs.claims[0];
    b.work(0, 32)?;
    let subject = claim
        .get("subject_ref")
        .and_then(Value::as_str)
        .ok_or(ClaimReferenceError::InvalidInput)?;
    let object = claim
        .get("object")
        .and_then(Value::as_str)
        .ok_or(ClaimReferenceError::UnsupportedProfile)?;
    let evidence = claim
        .get("evidence_refs")
        .and_then(Value::as_array)
        .ok_or(ClaimReferenceError::InvalidInput)?;
    if subject == object
        || evidence.len() != 1
        || claim
            .get("counterevidence_refs")
            .is_some_and(|v| v.as_array().is_none_or(|v| !v.is_empty()))
    {
        return Err(ClaimReferenceError::UnsupportedProfile);
    }
    for (i, row) in inputs.objects.iter().enumerate() {
        claim_reference_path(row.path, b)?;
        b.work(
            row.id
                .len()
                .checked_add(row.identity_field.len())
                .ok_or(ClaimReferenceError::Limit)?,
            24,
        )?;
        if row.identity_field != "record_id" || (row.id != subject && row.id != object) {
            return Err(ClaimReferenceError::InvalidInput);
        }
        for other in &inputs.objects[..i] {
            b.work(
                other
                    .id
                    .len()
                    .checked_add(other.path.len())
                    .ok_or(ClaimReferenceError::Limit)?,
                8,
            )?;
            if row.id == other.id || row.path == other.path {
                return Err(ClaimReferenceError::AmbiguousIdentity);
            }
        }
    }
    let repo = &inputs.evidence[0];
    claim_reference_path(repo.path, b)?;
    b.work(repo.reference.len(), 16)?;
    if repo.reference != repo.path || evidence[0].as_str() != Some(repo.reference) {
        return Err(ClaimReferenceError::InvalidInput);
    }
    // Fresh closed construction: 2 root keys, 2 ID keys, 10 object fields,
    // 1 evidence key, 4 evidence fields. Pay all slots, text/escaping and
    // temporary scalar/digest copies BEFORE any expected Map/String exists.
    let text = inputs.objects.iter().try_fold(
        repo.path
            .len()
            .checked_mul(2)
            .ok_or(ClaimReferenceError::Limit)?,
        |n, v| {
            n.checked_add(v.id.len())
                .and_then(|n| n.checked_add(v.path.len()))
                .and_then(|n| n.checked_add(v.raw.len()))
                .ok_or(ClaimReferenceError::Limit)
        },
    )?;
    let output_bound = text
        .checked_mul(6)
        .and_then(|n| n.checked_add(4096))
        .ok_or(ClaimReferenceError::Limit)?;
    if output_bound > JsonLimits::default().max_bytes {
        return Err(ClaimReferenceError::Limit);
    }
    b.own(
        output_bound
            .checked_mul(8)
            .and_then(|n| n.checked_add(32 * 256))
            .ok_or(ClaimReferenceError::Limit)?,
    )?;
    let mut objects = serde_json::Map::new();
    for row in inputs.objects {
        let record = b.parse(row.raw)?;
        if claim_reference_field(&record, row.identity_field, b)?.and_then(JsonValue::as_str)
            != Some(row.id)
        {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        let scalar =
            |name: &str, b: &mut ClaimReferenceBudget<'_>| -> ClaimReferenceResult<Value> {
                Ok(match claim_reference_field(&record, name, b)? {
                    None | Some(JsonValue::Null) => Value::Null,
                    Some(JsonValue::String(v)) if name == "schema_version" => Value::String(
                        v.as_str()
                            .ok_or(ClaimReferenceError::InvalidInput)?
                            .to_owned(),
                    ),
                    Some(JsonValue::Number(v)) if name == "record_version" => {
                        b.work(v.lexeme.len(), 8)?;
                        Value::from(
                            v.lexeme
                                .parse::<u64>()
                                .map_err(|_| ClaimReferenceError::UnsupportedProfile)?,
                        )
                    }
                    _ => return Err(ClaimReferenceError::UnsupportedProfile),
                })
            };
        let schema = scalar("schema_version", b)?;
        let version = scalar("record_version", b)?;
        let canonical = b.canonical(&record, row.raw.len())?;
        b.work(
            row.raw
                .len()
                .checked_add(canonical.len())
                .ok_or(ClaimReferenceError::Limit)?,
            32,
        )?;
        objects.insert(
            row.id.to_owned(),
            serde_json::json!({
                "source_ref":row.path,
                "source_sha256":Digest256::of_bytes(row.raw).to_prefixed(),
                "canonical_record_sha256":Digest256::of_bytes(&canonical).to_prefixed(),
                "schema_version":schema,"record_version":version
            }),
        );
    }
    b.work(repo.raw.len(), 32)?;
    let mut bound_evidence = serde_json::Map::new();
    bound_evidence.insert(
        repo.reference.to_owned(),
        serde_json::json!({
            "source_ref":repo.path,"source_sha256":Digest256::of_bytes(repo.raw).to_prefixed(),
            "source_line":null,"evidence_kind":"repo_path"
        }),
    );
    Ok(serde_json::json!({"objects":objects,"evidence":bound_evidence}))
}
fn check_declared_claim_context(
    ctx: &tos_command::source_command::CommandContext,
    files: &BTreeMap<String, Vec<u8>>,
    software: &BTreeMap<String, Vec<u8>>,
    config: &Value,
    request: &Value,
    base: SourceRevision,
    b: &mut ClaimReferenceBudget<'_>,
) -> ClaimReferenceResult<(Digest256, Digest256, Digest256)> {
    b.work(0, 64)?;
    if ctx.files.len() > 4096 {
        return Err(ClaimReferenceError::Limit);
    }
    if ctx.base_revision != base
        || config.get("uid").and_then(Value::as_u64) != Some(ctx.effective_uid)
        || ctx.files.len()
            != files
                .len()
                .checked_add(software.len())
                .ok_or(ClaimReferenceError::Limit)?
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let mut previous = None;
    for member in &ctx.files {
        let path = member.path.as_str();
        b.work(
            path.len(),
            files
                .len()
                .checked_add(software.len())
                .and_then(|n| n.checked_add(8))
                .ok_or(ClaimReferenceError::Limit)?,
        )?;
        if previous.is_some_and(|old| old >= path)
            || (files.contains_key(path) && software.contains_key(path))
        {
            return Err(ClaimReferenceError::AmbiguousIdentity);
        }
        previous = Some(path);
        let raw = files
            .get(path)
            .or_else(|| software.get(path))
            .ok_or(ClaimReferenceError::MissingSource)?;
        if raw.len() > 8_388_608 || member.raw.len() > 8_388_608 {
            return Err(ClaimReferenceError::Limit);
        }
        if !b.same_bytes(raw, &member.raw)? {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    let config_value = b.parse(&ctx.configuration_raw)?;
    let request_value = b.parse(&ctx.request_raw)?;
    if !claim_reference_same(&config_value, config, b, 0)?
        || !claim_reference_same(&request_value, request, b, 0)?
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let config_bytes = b.canonical(&config_value, ctx.configuration_raw.len())?;
    let request_bytes = b.canonical(&request_value, ctx.request_raw.len())?;
    b.work(
        config_bytes
            .len()
            .checked_add(request_bytes.len())
            .ok_or(ClaimReferenceError::Limit)?,
        8,
    )?;
    b.work(ctx.configuration_raw.len(), 8)?;
    Ok((
        Digest256::of_bytes(&config_bytes),
        Digest256::of_bytes(&request_bytes),
        Digest256::of_bytes(&ctx.configuration_raw),
    ))
}

fn claim_reference_equal(
    a: &Value,
    c: &Value,
    b: &mut ClaimReferenceBudget<'_>,
    depth: usize,
) -> ClaimReferenceResult<()> {
    b.work(0, 8)?;
    if depth > 64 {
        return Err(ClaimReferenceError::Limit);
    }
    let equal = match (a, c) {
        (Value::String(a), Value::String(c)) => {
            b.work(
                a.len()
                    .checked_add(c.len())
                    .ok_or(ClaimReferenceError::Limit)?,
                4,
            )?;
            a == c
        }
        (Value::Array(a), Value::Array(c)) if a.len() == c.len() => {
            for (a, c) in a.iter().zip(c) {
                claim_reference_equal(a, c, b, depth + 1)?;
            }
            true
        }
        (Value::Object(a), Value::Object(c)) if a.len() == c.len() => {
            for key in c.keys() {
                b.work(key.len(), 4)?;
            }
            for (key, value) in a {
                b.work(
                    key.len()
                        .checked_mul(c.len().checked_add(1).ok_or(ClaimReferenceError::Limit)?)
                        .ok_or(ClaimReferenceError::Limit)?,
                    8,
                )?;
                claim_reference_equal(
                    value,
                    c.get(key).ok_or(ClaimReferenceError::WitnessMismatch)?,
                    b,
                    depth + 1,
                )?;
            }
            true
        }
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(c)) => a == c,
        (Value::Number(a), Value::Number(c)) => a.as_u64().is_some() && a.as_u64() == c.as_u64(),
        _ => false,
    };
    if equal {
        Ok(())
    } else {
        Err(ClaimReferenceError::WitnessMismatch)
    }
}
fn claim_reference_controls(
    inputs: &DeclaredSimpleClaimInputs<'_>,
    selected: &BTreeMap<String, Vec<u8>>,
    expected: &Value,
    b: &mut ClaimReferenceBudget<'_>,
) -> ClaimReferenceResult<()> {
    b.work(0, 64)?;
    let original = &inputs.objects[0];
    let capacity = original
        .raw
        .len()
        .checked_add(2)
        .ok_or(ClaimReferenceError::Limit)?;
    b.own(capacity.checked_mul(2).ok_or(ClaimReferenceError::Limit)?)?;
    let mut spaced = Vec::new();
    spaced
        .try_reserve_exact(capacity)
        .map_err(|_| ClaimReferenceError::Limit)?;
    if spaced.capacity() > capacity.checked_mul(2).ok_or(ClaimReferenceError::Limit)? {
        return Err(ClaimReferenceError::Limit);
    }
    b.work(original.raw.len(), 8)?;
    spaced.push(b' ');
    spaced.extend_from_slice(original.raw);
    spaced.push(b'\n');
    let changed = [
        DeclaredClaimObject {
            raw: &spaced,
            ..*original
        },
        DeclaredClaimObject {
            ..inputs.objects[1]
        },
    ];
    let variant = DeclaredSimpleClaimInputs {
        objects: &changed,
        evidence: inputs.evidence,
        claims: inputs.claims,
    };
    let changed_expected = expected_simple_claim_bindings(&variant, b)?;
    // Whitespace changes raw fixity, never the full canonical record digest.
    let id = original.id;
    b.work(id.len(), 32)?;
    claim_reference_equal(
        &expected["objects"][id]["canonical_record_sha256"],
        &changed_expected["objects"][id]["canonical_record_sha256"],
        b,
        0,
    )?;
    assert_eq!(
        claim_reference_equal(expected, &changed_expected, b, 0),
        Err(ClaimReferenceError::WitnessMismatch)
    );
    assert_eq!(
        check_simple_claim_selected_inputs(&variant, selected, b),
        Err(ClaimReferenceError::WitnessMismatch)
    );
    let duplicate = [
        DeclaredClaimObject { ..*original },
        DeclaredClaimObject {
            id: original.id,
            ..inputs.objects[1]
        },
    ];
    let invalid = DeclaredSimpleClaimInputs {
        objects: &duplicate,
        evidence: inputs.evidence,
        claims: inputs.claims,
    };
    assert!(matches!(
        expected_simple_claim_bindings(&invalid, b),
        Err(ClaimReferenceError::AmbiguousIdentity)
    ));
    let absent = DeclaredSimpleClaimInputs {
        objects: inputs.objects,
        evidence: &[],
        claims: inputs.claims,
    };
    assert!(matches!(
        expected_simple_claim_bindings(&absent, b),
        Err(ClaimReferenceError::UnsupportedProfile)
    ));
    Ok(())
}

fn check_historical_simple_inputs(
    inputs: &DeclaredSimpleClaimInputs<'_>,
    provenance_raw: &[u8],
    oracle_id: &str,
    config: &Value,
    request: &Value,
    historical_request: bool,
    root: &Path,
    b: &mut ClaimReferenceBudget<'_>,
) -> ClaimReferenceResult<()> {
    b.work(provenance_raw.len(), 64)?;
    if inputs.objects.len() != 2 || inputs.evidence.len() != 1 || inputs.claims.len() != 1 {
        return Err(ClaimReferenceError::UnsupportedProfile);
    }
    // Static checked provenance has wide metadata objects, not dense arbitrary
    // arrays. Lower this ONE parser state child; its actual state guard still
    // refuses, and all callbacks share the original case work. No validation
    // or visit pass is removed and no reservation is refunded.
    let witness = b.parse_reserved(provenance_raw, 16)?;
    if claim_reference_field(&witness, "schema_version", b)?.and_then(JsonValue::as_str)
        != Some("tos_legacy_python_oracle_capture_v1")
        || claim_reference_field(&witness, "oracle_id", b)?.and_then(JsonValue::as_str)
            != Some(oracle_id)
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let input = claim_reference_field(&witness, "input_witness", b)?
        .ok_or(ClaimReferenceError::MissingSource)?;
    let inventory = claim_reference_field(input, "selected_source_inputs", b)?
        .ok_or(ClaimReferenceError::MissingSource)?;
    let number = |v: &JsonValue| -> ClaimReferenceResult<usize> {
        match v {
            JsonValue::Number(v) => v
                .lexeme
                .parse::<usize>()
                .map_err(|_| ClaimReferenceError::InvalidInput),
            _ => Err(ClaimReferenceError::InvalidInput),
        }
    };
    let rows = claim_reference_field(inventory, "files", b)?
        .and_then(JsonValue::as_array)
        .ok_or(ClaimReferenceError::InvalidInput)?;
    let count = number(
        claim_reference_field(inventory, "entry_count", b)?
            .ok_or(ClaimReferenceError::InvalidInput)?,
    )?;
    let total = number(
        claim_reference_field(inventory, "total_bytes", b)?
            .ok_or(ClaimReferenceError::InvalidInput)?,
    )?;
    if count != rows.len() || count > 4096 || total > 33_554_432 {
        return Err(ClaimReferenceError::Limit);
    }
    let mut previous = None;
    let mut sum = 0usize;
    let mut found = [false; 3];
    for row in rows {
        b.work(0, 64)?;
        let path = claim_reference_field(row, "path", b)?
            .and_then(JsonValue::as_str)
            .ok_or(ClaimReferenceError::InvalidInput)?;
        let bytes = number(
            claim_reference_field(row, "bytes", b)?.ok_or(ClaimReferenceError::InvalidInput)?,
        )?;
        let sha = claim_reference_field(row, "sha256", b)?
            .and_then(JsonValue::as_str)
            .ok_or(ClaimReferenceError::InvalidInput)?;
        b.work(
            path.len()
                .checked_add(sha.len())
                .ok_or(ClaimReferenceError::Limit)?,
            16,
        )?;
        if previous.is_some_and(|old| old >= path)
            || bytes > 8_388_608
            || sha.len() != 64
            || !sha
                .bytes()
                .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
        {
            return Err(ClaimReferenceError::InvalidInput);
        }
        previous = Some(path);
        sum = sum.checked_add(bytes).ok_or(ClaimReferenceError::Limit)?;
        for (index, (declared, raw)) in inputs
            .objects
            .iter()
            .map(|v| (v.path, v.raw))
            .chain(inputs.evidence.iter().map(|v| (v.path, v.raw)))
            .enumerate()
        {
            b.work(declared.len(), 8)?;
            if declared == path {
                b.work(raw.len(), 16)?;
                b.own(64)?; // fixed fresh hex digest, before its String construction
                if found[index] || raw.len() != bytes || Digest256::of_bytes(raw).to_hex() != sha {
                    return Err(ClaimReferenceError::WitnessMismatch);
                }
                found[index] = true;
            }
        }
    }
    if sum != total || found.iter().any(|v| !v) {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let old = claim_reference_field(input, "configuration", b)?
        .ok_or(ClaimReferenceError::MissingSource)?;
    let current = config
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?;
    b.work(0, 32)?;
    if old
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?
        .len()
        != current.len()
        || config.get("source_root").and_then(Value::as_str) != root.to_str()
        || !root.is_absolute()
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    for (key, value) in current {
        b.work(key.len(), 16)?;
        let original =
            claim_reference_field(old, key, b)?.ok_or(ClaimReferenceError::MissingSource)?;
        let same = match key.as_str() {
            "source_root" => original.as_str() == Some("<isolated-source-root>"),
            "uid" => number(original)? == 1000 && value.as_u64().is_some(),
            _ => claim_reference_same(original, value, b, 0)?,
        };
        if !same {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    if historical_request {
        let old = claim_reference_field(input, "request", b)?
            .ok_or(ClaimReferenceError::MissingSource)?;
        if !claim_reference_same(old, request, b, 0)? {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    // A current temporal correction is separately checked against its complete
    // authored request by check_declared_claim_context, never relabelled H.
    Ok(())
}

// Exact output of the historical Claim oracle, captured with its selected
// fixture bytes and source hashes in the adjacent provenance record.
fn maintained_oracle(oracle_id: &str) -> Value {
    crate::frozen_legacy_python_oracle(oracle_id)
}

// Generate the synthetic root's catalog with the existing native compiler
// producer. The separate catalog conformance test retains its existing
// independent owner oracle for catalog content, manifest semantics and graph
// rows.
fn publish_fixture_catalog(repository: &Path, root: &Path, deadline: Instant) {
    super::compiler_source_cases::publish_native_catalog_fixture(repository, root, deadline);
}

fn source_value(value: &Value) -> JsonValue {
    parse_json(
        &serde_json::to_vec(value).unwrap(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}
fn source_bytes(value: &JsonValue) -> Vec<u8> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits::default(),
    )
    .unwrap()
}
fn response(prepared: &PreparedCommand) -> Value {
    serde_json::from_slice(&source_bytes(&prepared.response)).unwrap()
}
fn source_ref(value: &Value) -> Value {
    serde_json::from_slice(&source_bytes(
        &metadata_subject(&source_value(value)).unwrap(),
    ))
    .unwrap()
}
fn apply_changes(files: &mut BTreeMap<String, Vec<u8>>, prepared: &PreparedCommand) {
    for change in &prepared.changes {
        assert_eq!(
            change.before,
            files
                .get(change.path.as_str())
                .map(|raw| Digest256::of_bytes(raw))
        );
        match &change.after {
            Some(raw) => {
                files.insert(change.path.as_str().into(), raw.clone());
            }
            None => {
                files.remove(change.path.as_str());
            }
        }
    }
}
pub(super) fn claim_fixture() -> (BTreeMap<String, Vec<u8>>, Value, Value, String) {
    let repository = super::validation_cut_cases::repository();
    let packet =
        repository.join("rust/crates/tos-command/tests/fixtures/source_forms_shadow/claim_v1");
    let source: Value =
        serde_json::from_slice(&fs::read(packet.join("source.json")).unwrap()).unwrap();
    let old_owner: Value =
        serde_json::from_slice(&fs::read(packet.join("owner.json")).unwrap()).unwrap();
    let source_path = old_owner["source_path"].as_str().unwrap().to_owned();
    let evidence = source["evidence_refs"][0].as_str().unwrap();
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            files.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    for name in ["entity-types.v1.json", "relation-types.v1.json"] {
        let path = format!("ToS/doctrine/semantic-interchange/{name}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    files.insert(
        evidence.into(),
        b"Synthetic command fixture evidence; not historical verification.\n".to_vec(),
    );
    // Bounded endpoint scaffolding uses the fixture's exact IDs and actual
    // selected owner schemas. These stubs are synthetic test metadata.
    for (kind, id, schema) in [
        (
            "letter",
            source["subject_ref"].as_str().unwrap(),
            "tos_document_record_v1",
        ),
        (
            "agent",
            source["object"].as_str().unwrap(),
            "tos_corpus_record_v1",
        ),
    ] {
        let mut record = serde_json::json!({"schema_version":schema,"record_type":kind,"record_id":id,"preferred_label":"Synthetic command fixture endpoint; no assessment","identity_status":"provisional","source_refs":[evidence],"external_identifiers":[],"same_as_posture":"no_equivalence_claim","record_version":1});
        if kind == "letter" {
            record["visibility"] = Value::String("public_metadata_only".into());
        }
        files.insert(
            format!("ToS/source-witnesses/{kind}s/command-claim-fixture/{kind}.json"),
            source_bytes(&source_value(&record)),
        );
    }
    let configuration = serde_json::json!({"schema_version":"tos_local_claim_revision_owner_v1","uid":1000,"principal_id":old_owner["principal_id"],"source_root":"synthetic:anchored-command-fixture","source_path":source_path,"authority_ref":"synthetic:claim-revision-conformance-no-admission","expires_at":"2099-01-01T00:00:00Z","claim_id":source["claim_id"],"allowed_operations":["claim.revise"],"allowed_fields":["qualifiers"],"allowed_evidence_refs":source["evidence_refs"],"allowed_form_ids":old_owner["allowed_form_ids"],"allowed_form_field_ids":["claim.statement","claim.name","claim.caption","claim.hover"]});
    (files, source, configuration, source_path)
}

#[test]
fn claim_successor_retains_bytes_replays_current_scope_and_refuses_unissued_admission() {
    let (mut files, fixture, mut configuration, source_path) = claim_fixture();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("claim-store");
    configuration["source_root"] = Value::String(root.to_str().unwrap().into());
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(180);
    // Do not silently reinterpret a v2 fixture as an initial package.
    files.insert(
        source_path.clone(),
        [source_bytes(&source_value(&fixture)), b"\n".to_vec()].concat(),
    );
    let missing_root = temporary.path().join("incomplete-existing-fixture-store");
    let missing_base = super::validation_cut_cases::write_cut_store(&files, &missing_root);
    let missing_cut = open_cut(&missing_root, missing_base, deadline, &cancel);
    let mut worker = schemas(&missing_cut, deadline, &cancel);
    let describe =
        serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"});
    assert!(matches!(
        run_claim_command(
            &context(
                &files,
                serde_json::to_vec(&configuration).unwrap(),
                serde_json::to_vec(&describe).unwrap(),
                missing_base
            ),
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    // Explicit synthetic baseline preserves every authored fixture qualifier.
    let mut initial = fixture.clone();
    initial["claim_version"] = Value::from(1);
    let mut initial_stream = b" \r\n".to_vec();
    initial_stream.extend(source_bytes(&source_value(&initial)));
    initial_stream.extend(b"\r\n\t\n");
    files.insert(source_path.clone(), initial_stream.clone());
    // Independent synthetic initial cut: never attach an invented predecessor
    // or a v2 -> v1 regression to the incomplete existing fixture's ancestry.
    let base = super::validation_cut_cases::write_cut_store(&files, &root);
    let cut = open_cut(&root, base, deadline, &cancel);
    let mut worker = schemas(&cut, deadline, &cancel);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let commit_output = std::process::Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .unwrap();
    assert!(commit_output.status.success());
    assert!(commit_output.stdout.len() <= 41);
    let commit = String::from_utf8(commit_output.stdout)
        .unwrap()
        .trim()
        .to_owned();
    let rule_names: std::collections::BTreeSet<&str> = CLAIM_GROUNDING_RULE_INPUTS
        .iter()
        .chain(CLAIM_REVISION_RULE_INPUTS)
        .copied()
        .filter(|name| !name.starts_with("ToS/"))
        .collect();
    let capture = super::source_cut_cases::captured_software_fixture(
        &repository,
        &commit,
        &rule_names.iter().copied().collect::<Vec<_>>(),
    );
    let software = SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 128,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
        deadline,
        &cancel,
    )
    .unwrap();
    let components = software
        .select_components(
            &rule_names
                .iter()
                .map(|name| RelativePath::parse(name).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let mut software_files = BTreeMap::new();
    for name in rule_names {
        let path = RelativePath::parse(name).unwrap();
        let raw = software
            .read_selected_component(&components, &path, 1_048_576, deadline, &cancel)
            .unwrap();
        assert_eq!(
            raw,
            fs::read(repository.join(name)).unwrap(),
            "oracle rule bytes differ from selected capture"
        );
        software_files.insert(name.into(), raw);
    }
    let ids = configuration["allowed_form_ids"].as_array().unwrap();
    let selections: Vec<Value> = ids
        .iter()
        .zip([
            "claim.statement",
            "claim.name",
            "claim.caption",
            "claim.hover",
        ])
        .map(|(id, field)| serde_json::json!({"form_id":id,"field_id":field}))
        .collect();
    let wording = format!(
        "{} [command fixture successor]",
        initial["qualifiers"]["statement"].as_str().unwrap()
    );
    let proposal = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-revise","fields":{"qualifiers":{"statement":wording}},"forms":selections,"reason":"Synthetic source-copy Claim successor conformance; no assessment."});
    let mut claim_reference_budget = ClaimReferenceBudget::new(deadline, &cancel);
    claim_reference_budget.own(32_768).unwrap(); // fixed borrowed-input/control workspace before arrays
    let simple_objects = [
        DeclaredClaimObject {
            id: initial["subject_ref"].as_str().unwrap(),
            path: "ToS/source-witnesses/letters/command-claim-fixture/letter.json",
            raw: &files["ToS/source-witnesses/letters/command-claim-fixture/letter.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: initial["object"].as_str().unwrap(),
            path: "ToS/source-witnesses/agents/command-claim-fixture/agent.json",
            raw: &files["ToS/source-witnesses/agents/command-claim-fixture/agent.json"],
            identity_field: "record_id",
        },
    ];
    let simple_evidence = [DeclaredRepoEvidence {
        reference: initial["evidence_refs"][0].as_str().unwrap(),
        path: initial["evidence_refs"][0].as_str().unwrap(),
        raw: &files[initial["evidence_refs"][0].as_str().unwrap()],
    }];
    let simple_claims = [&initial];
    let simple_inputs = DeclaredSimpleClaimInputs {
        objects: &simple_objects,
        evidence: &simple_evidence,
        claims: &simple_claims,
    };
    check_simple_claim_selected_inputs(&simple_inputs, &files, &mut claim_reference_budget)
        .unwrap();
    let simple_reference =
        expected_simple_claim_bindings(&simple_inputs, &mut claim_reference_budget).unwrap();
    claim_reference_controls(
        &simple_inputs,
        &files,
        &simple_reference,
        &mut claim_reference_budget,
    )
    .unwrap();
    check_historical_simple_inputs(
        &simple_inputs,
        include_bytes!("legacy-python-oracles-v1/claim-revision-initial.provenance.json"),
        "claim-revision-initial",
        &configuration,
        &proposal,
        true,
        &root,
        &mut claim_reference_budget,
    )
    .unwrap();
    let preview_ctx = selected_context(&files, &software_files, &configuration, &proposal, base);
    let simple_context_digests = check_declared_claim_context(
        &preview_ctx,
        &files,
        &software_files,
        &configuration,
        &proposal,
        base,
        &mut claim_reference_budget,
    )
    .unwrap();
    assert!(
        matches!(
            run_claim_command(&preview_ctx, &mut worker, deadline, &cancel),
            Err(SourceCommandError::Unsupported(_))
        ),
        "a selected byte list cannot establish complete Claim inventory"
    );
    let mut partial = preview_ctx.clone();
    partial
        .files
        .retain(|input| !input.path.as_str().ends_with("/letter.json"));
    assert!(
        matches!(
            checked_command(
                &partial,
                &cut,
                &software,
                &components,
                &mut worker,
                deadline,
                &cancel
            ),
            Err(SourceCommandError::Unsupported(_))
        ),
        "complete current inventory cannot be inferred from omitted selected metadata"
    );
    let preview = checked_command(
        &preview_ctx,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    let preview_response = response(&preview);
    let oracle = maintained_oracle("claim-revision-initial");
    claim_reference_equal(
        &simple_reference,
        &oracle["source_bindings"],
        &mut claim_reference_budget,
        0,
    )
    .unwrap();
    assert_eq!(
        preview.configuration_canonical_sha256,
        simple_context_digests.0
    );
    assert_eq!(preview.request_canonical_sha256, simple_context_digests.1);
    assert_eq!(preview.configuration_raw_sha256, simple_context_digests.2);
    claim_reference_equal(
        &preview_response["source_bindings"],
        &simple_reference,
        &mut claim_reference_budget,
        0,
    )
    .unwrap();
    // The frozen Python dependency fingerprint is history; a current command
    // must use the native capture prepared from this authenticated source cut.
    assert_eq!(
        preview_response["source_bindings"], oracle["source_bindings"],
        "maintained declared source bindings"
    );
    let mut request = proposal.clone();
    request["operation"] = Value::String("claim.revise".into());
    request["command_id"] = Value::String("conformance:claim-successor".into());
    for (request_key, response_key) in [
        ("expected_configuration", "owner_configuration"),
        ("expected_source", "source"),
        ("expected_revision", "revision"),
        ("expected_dependencies", "expected_dependencies"),
        ("expected_inputs", "source_bindings"),
    ] {
        request[request_key] = preview_response[response_key].clone();
    }
    let historical_dependencies = oracle["expected_dependencies"].clone();
    assert_ne!(
        request["expected_dependencies"], historical_dependencies,
        "owner migration must change the live Claim dependency fingerprint"
    );
    let mut stale_request = request.clone();
    stale_request["expected_dependencies"] = historical_dependencies;
    let mut stale_context = selected_context(
        &files,
        &software_files,
        &configuration,
        &stale_request,
        base,
    );
    stale_context.request_raw = serde_json::to_vec(&stale_request).unwrap();
    let mut stale_worker = schemas(&cut, deadline, &cancel);
    assert!(matches!(
        checked_command(
            &stale_context,
            &cut,
            &software,
            &components,
            &mut stale_worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    drop(stale_worker);

    let ctx = selected_context(&files, &software_files, &configuration, &request, base);
    let prepared = checked_command(
        &ctx,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    let result = response(&prepared);
    assert!(!prepared.replayed);
    assert_eq!(result["receipt"]["previous_source"], source_ref(&initial));
    assert_eq!(result["receipt"]["source"]["version"], 2);
    assert_eq!(
        prepared.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    // Independent expected row update protects richer qualifier preservation,
    // sibling whitespace and the exact maintained canonical row convention.
    let mut expected = initial.clone();
    expected["qualifiers"]["statement"] = proposal["fields"]["qualifiers"]["statement"].clone();
    expected["claim_version"] = Value::from(2);
    let mut expected_stream = b" \r\n".to_vec();
    expected_stream.extend(source_bytes(&source_value(&expected)));
    expected_stream.extend(b"\r\n\t\n");
    let write = prepared
        .changes
        .iter()
        .find(|change| change.path.as_str() == source_path)
        .unwrap();
    assert_eq!(write.after.as_ref().unwrap(), &expected_stream);
    let archive = result["receipt"]["archive_path"].as_str().unwrap();
    let archive_stream = format!(
        "{archive}/{}.blob",
        Digest256::of_bytes(&initial_stream).to_hex()
    );
    assert_eq!(
        prepared
            .changes
            .iter()
            .find(|change| change.path.as_str() == archive_stream)
            .unwrap()
            .after
            .as_ref()
            .unwrap(),
        &initial_stream
    );
    let manifest: Value = serde_json::from_slice(
        prepared
            .changes
            .iter()
            .find(|change| change.path.as_str() == format!("{archive}/manifest.json"))
            .unwrap()
            .after
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest["files"],
        serde_json::json!({"source-claims.jsonl":{"blob":format!("{}.blob",Digest256::of_bytes(&initial_stream).to_hex()),"sha256":Digest256::of_bytes(&initial_stream).to_prefixed(),"bytes":initial_stream.len()}})
    );
    let history_path = format!(
        "{}/claim-revision-history.json",
        source_path.rsplit_once('/').unwrap().0
    );
    assert!(
        prepared
            .changes
            .iter()
            .any(|change| change.path.as_str() == history_path)
    );
    apply_changes(&mut files, &prepared);
    let candidate = successor(&files, &root, base);
    let candidate_cut = open_cut(&root, candidate, deadline, &cancel);
    let bound = bind_selected_candidate(
        &ctx,
        prepared,
        &cut,
        &software,
        &components,
        &candidate_cut,
        RelativePath::parse("protected-owner/claim-command.json").unwrap(),
        OperationLimits {
            max_member_bytes: 8_388_608,
            max_total_bytes: 33_554_432,
            max_state_bytes: 33_554_432,
            max_reads: 8192,
            max_changes: 64,
            deadline,
        },
        &cancel,
    )
    .unwrap();
    assert_eq!(bound.binding().base_revision(), base);
    assert_eq!(bound.binding().candidate_revision(), candidate);
    assert!(matches!(
        bound.commit(),
        Err(SourceOperationError::MissingFullSourceAdmission)
    ));
    let mut worker = schemas(&candidate_cut, deadline, &cancel);
    let replay_ctx = selected_context(&files, &software_files, &configuration, &request, candidate);
    let replay = checked_command(
        &replay_ctx,
        &candidate_cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    assert!(replay.replayed);
    assert!(replay.changes.is_empty());
    assert_eq!(response(&replay)["receipt"], result["receipt"]);
    let inspect = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-version","source":source_ref(&initial)});
    let inspected = checked_command(
        &selected_context(&files, &software_files, &configuration, &inspect, candidate),
        &candidate_cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancel,
    )
    .unwrap();
    assert_eq!(response(&inspected)["record"], initial);
    assert_eq!(
        response(&inspected)["files"]["source-claims.jsonl"],
        serde_json::json!({"archive_path":archive_stream,"sha256":Digest256::of_bytes(&initial_stream).to_prefixed(),"bytes":initial_stream.len()})
    );
    assert!(inspected.changes.is_empty());
    // A bound candidate and retained receipt do not restore current authority.
    let mut revoked = configuration.clone();
    revoked["allowed_operations"] = serde_json::json!([]);
    let revoked_ctx = selected_context(&files, &software_files, &revoked, &request, candidate);
    assert!(matches!(
        checked_command(
            &revoked_ctx,
            &candidate_cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Denied(_))
    ));
    let mut reused = request.clone();
    reused["reason"] = Value::String("Different request sharing retained command identity.".into());
    assert!(matches!(
        checked_command(
            &selected_context(&files, &software_files, &configuration, &reused, candidate),
            &candidate_cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    let mut corrupted = files.clone();
    corrupted.insert(archive_stream, b"{}\n".to_vec());
    let corrupt_revision = successor(&corrupted, &root, candidate);
    let corrupt_cut = open_cut(&root, corrupt_revision, deadline, &cancel);
    let mut corrupt_worker = schemas(&corrupt_cut, deadline, &cancel);
    assert!(matches!(
        checked_command(
            &selected_context(
                &corrupted,
                &software_files,
                &configuration,
                &inspect,
                corrupt_revision
            ),
            &corrupt_cut,
            &software,
            &components,
            &mut corrupt_worker,
            deadline,
            &cancel
        ),
        Err(SourceCommandError::Conflict(_))
    ));
}

#[test]
fn initial_claim_creation_publishes_five_native_files_and_cold_replays() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let (mut files, mut claim, mut revision_owner, source_path) = claim_fixture();
    claim["claim_version"] = Value::from(1);

    // R ab89a0145 / source contract ee06b7a6: caller prepayment is cumulative,
    // separate from owner/worker quotas; no refund when a pure plan is dropped.
    use std::cell::Cell;
    use tos_command::temporal_claim_intent::{ClaimIntentPrepareBudget, plan_claim_intent};
    use tos_foundation::temporal::{TemporalBudget, claim_carrier::decode_claim_carrier};
    let natural_parent = Cell::new((33_554_432usize, 1_000_000usize));
    let natural_debit = |bytes: usize, visits: usize| {
        let (left_bytes, left_visits) = natural_parent.get();
        natural_parent.set((
            left_bytes.checked_sub(bytes).unwrap(),
            left_visits.checked_sub(visits).unwrap(),
        ));
    };
    natural_debit(8_388_608, 65_536);
    let natural_retained = Cell::new((8_388_608usize, 65_536usize));
    let natural_raw_admit = |bytes: usize| {
        let (left, visits) = natural_retained.get();
        natural_retained.set((left.checked_sub(bytes).unwrap(), visits));
    };
    let natural_admit = |raw_bytes: usize, copies: usize| {
        assert!(raw_bytes <= 16_384 && copies > 0);
        let (bytes, visits) = natural_retained.get();
        let charge = raw_bytes
            .checked_mul(128)
            .unwrap()
            .checked_add(4096)
            .unwrap()
            .checked_mul(copies)
            .unwrap();
        natural_retained.set((
            bytes.checked_sub(charge).unwrap(),
            visits
                .checked_sub(512usize.checked_mul(copies).unwrap())
                .unwrap(),
        ));
    };
    // Known finite JSON construction: price 256 bytes per value/container
    // slot plus 16x UTF-8 text/output, rather than treating raw Vec copies as
    // new parser trees. The 8 MiB retained child is still never replenished.
    let natural_preallocate = |raw_bound: usize, values: usize, copies: usize| {
        assert!(raw_bound <= 16_384 && values <= 512 && copies > 0);
        let charge = raw_bound
            .checked_mul(16)
            .unwrap()
            .checked_add(values.checked_mul(256).unwrap())
            .unwrap()
            .checked_add(4096)
            .unwrap()
            .checked_mul(copies)
            .unwrap();
        let (left, visits) = natural_retained.get();
        natural_retained.set((
            left.checked_sub(charge).unwrap(),
            visits
                .checked_sub(values.checked_mul(copies).unwrap())
                .unwrap(),
        ));
    };
    let natural_shape = |root: &Value, cap: usize| {
        let mut stack = [None; 512]; // borrowed, fixed stack; no traversal Vec
        stack[0] = Some(root);
        let mut pending = 1;
        let mut visited = 0usize;
        while pending != 0 {
            pending -= 1;
            let value = stack[pending].take().unwrap();
            visited = visited.checked_add(1).unwrap();
            assert!(visited <= cap && cap <= 512);
            match value {
                Value::Object(rows) => {
                    for child in rows.values() {
                        assert!(pending < stack.len());
                        stack[pending] = Some(child);
                        pending += 1;
                    }
                }
                Value::Array(rows) => {
                    for child in rows {
                        assert!(pending < stack.len());
                        stack[pending] = Some(child);
                        pending += 1;
                    }
                }
                _ => {}
            }
        }
        visited
    };
    let natural_limits = JsonLimits {
        max_bytes: 16_384,
        max_depth: 64,
        max_visits: 512,
        max_integer_digits: 39,
    };
    let natural_parse = |raw: &[u8]| {
        natural_admit(raw.len(), 1);
        parse_json(raw, JsonMode::PublishedStrict, natural_limits)
            .unwrap()
            .into_root()
    };
    // Borrow immutable oracle bytes; parse only the four expected-free excerpts.
    let natural_oracle =
        include_bytes!("../foundation-wave-v1/oracles/temporal-acceptance-vectors.json");
    assert_eq!(natural_oracle.len(), 170_774);
    assert_eq!(
        Digest256::of_bytes(natural_oracle).to_hex(),
        "fda08fd958914a3605afab12e7b78012a3f5534ecfdffe2ab31cb2cb3a26eecd"
    );
    let natural_intent_literals = [
        r#"{"context":{},"input":{"absolute_mapping":null,"relation":"after","source_ref":"source-1","subject_a":"event-A","subject_b":"reign-R"},"operation":"record_explicit_relation","schema_version":"tos_temporal_claim_intent_v1","source_profile":"ToS-Temporal-Core-Contract-v1"}"#,
        r#"{"context":{},"input":{"claim_ref":"state-A","revision":"r1","validity_assertion":{"observed_through":"2026-10-19","trs_ref":"TRS:GregorianProleptic:fixture-v1","type":"RightCensored","valid_from":"2020-01-01"}},"operation":"append_claim_revision","schema_version":"tos_temporal_claim_intent_v1","source_profile":"ToS-Temporal-Core-Contract-v1"}"#,
        r#"{"context":{},"input":{"claim_ref":"recurrence-A","recurrence_pattern":{"calendar":"GregorianProleptic","exceptions":[],"frequency":"monthly","rule":"first day of local month","zone":"UTC"},"revision":"r1"},"operation":"append_claim_revision","schema_version":"tos_temporal_claim_intent_v1","source_profile":"ToS-Temporal-Core-Contract-v1"}"#,
        r#"{"context":{},"input":{"claim_ref":"event-1500","event_time":{"position":"1500","trs_ref":"TRS:GregorianProleptic:fixture-v1"},"historical_author_knowledge":{"type":"Unknown"},"revision":"r1","source_publication_time":{"position":"1800","trs_ref":"TRS:GregorianProleptic:fixture-v1"},"system_ingest_time":"2026-01-01T00:00:00Z"},"operation":"append_claim_revision","schema_version":"tos_temporal_claim_intent_v1","source_profile":"ToS-Temporal-Core-Contract-v1"}"#,
    ];
    let natural_intents = natural_intent_literals.map(|raw| {
        let parsed = natural_parse(raw.as_bytes());
        natural_admit(raw.len(), 2); // serde view and independent native equality view
        let value: Value = serde_json::from_slice(raw.as_bytes()).unwrap();
        assert_eq!(parsed, source_value(&value));
        value
    });
    let natural_seed_literals = [
        (
            "ToS/source-witnesses/history/command-temporal-intent-fixture/historical-event.json",
            r#"{"external_identifiers":[],"identity_status":"provisional","notes":"Fixture identity only; no original-world identity, chronology or source truth is admitted.","preferred_label":"Synthetic event-A referent for original temporal component storage","record_id":"tos.historical-event.temporal-intent-event-a","record_type":"historical-event","record_version":1,"same_as_posture":"no_equivalence_claim","schema_version":"tos_historical_record_v1","source_refs":["tests/conformance/foundation-wave-v1/oracles/temporal-acceptance-vectors.json"],"visibility":"public_metadata_only"}
"#,
            "4e03b7c85e174b9029a11c1a695edc7686aa11c7f945618d430d34170a7af356",
            "a2089a02562299405a6533f1a4aba7174494a9154d04ac96e007107488209ac0",
        ),
        (
            "ToS/source-witnesses/history/command-temporal-intent-fixture/historical-state.json",
            r#"{"external_identifiers":[],"identity_status":"provisional","notes":"Fixture identity only; no named historical reign, extent or date is asserted.","preferred_label":"Synthetic reign-R referent for original temporal component storage","record_id":"tos.historical-state.temporal-intent-reign-r","record_type":"historical-state","record_version":1,"same_as_posture":"no_equivalence_claim","schema_version":"tos_historical_record_v1","source_refs":["tests/conformance/foundation-wave-v1/oracles/temporal-acceptance-vectors.json"],"visibility":"public_metadata_only"}
"#,
            "8bb6f8449fd33babeeedd83485ab247be5427c0d39be294dfd00c0858347972b",
            "f8940d58a0e373d56d55e7698908a14d8821bbb6d214b1c47374c0b8183d590f",
        ),
        (
            "ToS/source-witnesses/history/command-temporal-intent-fixture/historical-process.json",
            r#"{"extensions":{"binding_supplements":{"censor_resolution":"day","censor_trs":{"id":"TRS:GregorianProleptic","version":"fixture-v1"},"recurrence_profile_ref":"fixture:opaque-pattern-v1","relative_asserted_by":null,"source_status":"SourceSuppliedUnverified"},"declarations":{"event_time":"{\"position\":\"1500\",\"trs_ref\":\"TRS:GregorianProleptic:fixture-v1\"}","historical_author_knowledge":"{\"type\":\"Unknown\"}","recurrence_preserve":"{\"calendar\":\"GregorianProleptic\",\"exceptions\":[],\"frequency\":\"monthly\",\"rule\":\"first day of local month\",\"zone\":\"UTC\"}","relative_constraint":"{\"absolute_mapping\":null,\"relation\":\"after\",\"source_ref\":\"source-1\",\"subject_a\":\"event-A\",\"subject_b\":\"reign-R\"}","right_censored_validity":"{\"claim_ref\":\"state-A\",\"revision\":\"r1\",\"validity_assertion\":{\"observed_through\":\"2026-10-19\",\"trs_ref\":\"TRS:GregorianProleptic:fixture-v1\",\"type\":\"RightCensored\",\"valid_from\":\"2020-01-01\"}}","source_publication_time":"{\"position\":\"1800\",\"trs_ref\":\"TRS:GregorianProleptic:fixture-v1\"}","system_ingest_time":"\"2026-01-01T00:00:00Z\""}},"external_identifiers":[],"identity_status":"provisional","notes":"New fixture source assertions copied from the original oracle inputs; not external historical evidence or verified observation.","preferred_label":"Synthetic source-1 declaration process for original temporal component storage","record_id":"tos.historical-process.temporal-intent-source-1","record_type":"historical-process","record_version":1,"same_as_posture":"no_equivalence_claim","schema_version":"tos_historical_record_v1","source_refs":["tests/conformance/foundation-wave-v1/oracles/temporal-acceptance-vectors.json"],"visibility":"public_metadata_only"}
"#,
            "9c80579380f97281bc976f6f0f5dcc0578c4bfea591e021f27b1391aed130a98",
            "ec492f3c90e8e00765db634a96db65efc38d7561da39d2aa575b4e1c46f760ab",
        ),
    ];
    let mut natural_seed_refs = Vec::with_capacity(3);
    let mut natural_source = Value::Null;
    let mut natural_seed_bytes = 0usize;
    for (index, (path, raw, raw_sha, canonical_sha)) in natural_seed_literals.iter().enumerate() {
        assert!(raw.len() <= 2048);
        let parsed = natural_parse(raw.as_bytes());
        natural_admit(raw.len(), 2); // native ref/serde view, canonical and stored raw
        let value: Value = serde_json::from_slice(raw.as_bytes()).unwrap();
        assert_eq!(Digest256::of_bytes(raw.as_bytes()).to_hex(), *raw_sha);
        let mut canonical = canonical_bytes_v1(
            &parsed,
            CanonicalProfile::SourceCommandInputV1,
            natural_limits,
        )
        .unwrap();
        assert_eq!(Digest256::of_bytes(&canonical).to_hex(), *canonical_sha);
        canonical.push(b'\n');
        assert_eq!(canonical, raw.as_bytes());
        let actual_ref = metadata_subject(&parsed).unwrap();
        let mut codec_ref: Value = serde_json::from_slice(&source_bytes(&actual_ref)).unwrap();
        assert_eq!(codec_ref["id"], value["record_id"]);
        assert_eq!(codec_ref["version"], value["record_version"]);
        assert_eq!(codec_ref["digest"], format!("sha256:{canonical_sha}"));
        codec_ref["digest"] = Value::String(
            codec_ref["digest"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap()
                .to_owned(),
        );
        natural_seed_refs.push(codec_ref);
        if index == 2 {
            natural_source = value;
        }
        natural_seed_bytes = natural_seed_bytes.checked_add(canonical.len()).unwrap();
        assert!(files.insert((*path).into(), canonical).is_none());
    }
    assert_eq!(natural_seed_bytes, 2897);
    // All subsequent file/context clones include these exact paid source facts.
    natural_raw_admit(
        natural_seed_bytes
            .checked_mul(16)
            .unwrap()
            .checked_add(4096)
            .unwrap(),
    );

    let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
        .iter()
        .chain(CLAIM_REVISION_RULE_INPUTS)
        .copied()
        .filter(|name| !name.starts_with("ToS/"))
        .collect::<Vec<_>>();
    software_names.extend([
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
    ]);
    software_names.sort_unstable();
    software_names.dedup();
    for name in software_names {
        files.insert(name.into(), fs::read(repository.join(name)).unwrap());
    }
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    let (_capture, software, components) =
        super::command_record_cases::captured_components(&files, deadline, &cancellation);
    let temporary = tempfile::tempdir().unwrap();
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let store = temporary.path().join("selected-store");
    let base = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = open_cut(&store, base, deadline, &cancellation);
    // One sealed executable image; every adapter still authenticates its own cut.
    let mut image_budget = ExecutorBudget::laboratory();
    image_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!image_budget.execution_wall.is_zero());
    let worker_image =
        super::command_form_cases::schema_image(image_budget, deadline, &cancellation);
    let claim_worker = || {
        let mut budget = ExecutorBudget::laboratory();
        budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!budget.execution_wall.is_zero());
        super::command_form_cases::schemas_for_profile_with_image(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            budget,
            deadline,
            &cancellation,
        )
    };
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
    for (name, raw) in &files {
        let target = isolated.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    fs::create_dir_all(
        isolated
            .path()
            .join(&source_path)
            .parent()
            .unwrap()
            .parent()
            .unwrap(),
    )
    .unwrap();
    let uid = fs::metadata(isolated.path()).unwrap().uid();
    let configuration = serde_json::json!({
        "schema_version":"tos_local_claim_create_owner_v1", "uid":uid,
        "principal_id":claim["maker"]["agent_ref"], "maker_type":claim["maker"]["maker_type"],
        "source_root":isolated.path(), "source_path":source_path,
        "authority_ref":"synthetic-test-only:claim-creation-not-assessment",
        "expires_at":"2099-01-01T00:00:00Z", "provenance_event_id":claim["provenance_event_ref"],
        "allowed_operations":["claims.create"], "allowed_claim_ids":[claim["claim_id"]],
        "allowed_subject_refs":[claim["subject_ref"]], "allowed_object_refs":[claim["object"]],
        "allowed_predicates":[claim["predicate"]], "allowed_evidence_refs":claim["evidence_refs"]
    });
    let config_raw = source_bytes(&source_value(&configuration));
    let owner = isolated.path().join("owner.json");
    fs::write(&owner, &config_raw).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let filesystem =
        CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation).unwrap();
    let preview_request = serde_json::json!({
        "schema_version":"tos_local_source_command_v1", "operation":"prepare-create",
        "claims":[claim.clone()]
    });
    let mut context = selected_context(
        &authored,
        &files
            .iter()
            .filter(|(name, _)| !name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect(),
        &configuration,
        &preview_request,
        base,
    );
    context.effective_uid = u64::from(uid);
    context.configuration_raw = config_raw.clone();
    let mut claim_reference_budget = ClaimReferenceBudget::new(deadline, &cancellation);
    claim_reference_budget.own(32_768).unwrap(); // SAME case account also covers retained-history reference
    let simple_objects = [
        DeclaredClaimObject {
            id: claim["subject_ref"].as_str().unwrap(),
            path: "ToS/source-witnesses/letters/command-claim-fixture/letter.json",
            raw: &files["ToS/source-witnesses/letters/command-claim-fixture/letter.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: claim["object"].as_str().unwrap(),
            path: "ToS/source-witnesses/agents/command-claim-fixture/agent.json",
            raw: &files["ToS/source-witnesses/agents/command-claim-fixture/agent.json"],
            identity_field: "record_id",
        },
    ];
    let simple_evidence = [DeclaredRepoEvidence {
        reference: claim["evidence_refs"][0].as_str().unwrap(),
        path: claim["evidence_refs"][0].as_str().unwrap(),
        raw: &files[claim["evidence_refs"][0].as_str().unwrap()],
    }];
    let simple_claims = [&claim];
    let simple_inputs = DeclaredSimpleClaimInputs {
        objects: &simple_objects,
        evidence: &simple_evidence,
        claims: &simple_claims,
    };
    check_simple_claim_selected_inputs(&simple_inputs, &authored, &mut claim_reference_budget)
        .unwrap();
    let simple_reference =
        expected_simple_claim_bindings(&simple_inputs, &mut claim_reference_budget).unwrap();
    check_historical_simple_inputs(
        &simple_inputs,
        include_bytes!("legacy-python-oracles-v1/claim-creation.provenance.json"),
        "claim-creation",
        &configuration,
        &preview_request,
        true,
        isolated.path(),
        &mut claim_reference_budget,
    )
    .unwrap();
    let reference_no_software = BTreeMap::new();
    let simple_context_digests = check_declared_claim_context(
        &context,
        &files,
        &reference_no_software,
        &configuration,
        &preview_request,
        base,
        &mut claim_reference_budget,
    )
    .unwrap();
    let mut worker = claim_worker();
    let preview = checked_command(
        &context,
        &cut,
        &software,
        &components,
        &mut worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    let expected = response(&preview);
    assert_eq!(
        expected["prepared_files"]["source-claims.jsonl"]["sha256"],
        serde_json::json!(
            Digest256::of_bytes(&[source_bytes(&source_value(&claim)), b"\n".to_vec()].concat())
                .to_prefixed()
        )
    );
    let oracle = crate::frozen_legacy_python_oracle("claim-creation");
    claim_reference_equal(
        &simple_reference,
        &oracle["bindings"],
        &mut claim_reference_budget,
        0,
    )
    .unwrap();
    claim_reference_equal(
        &expected["source_bindings"],
        &simple_reference,
        &mut claim_reference_budget,
        0,
    )
    .unwrap();
    assert_eq!(
        preview.configuration_canonical_sha256,
        simple_context_digests.0
    );
    assert_eq!(preview.request_canonical_sha256, simple_context_digests.1);
    assert_eq!(preview.configuration_raw_sha256, simple_context_digests.2);
    // The live request binds to the native preview; the captured Python
    // dependency fingerprint remains historical packet data.
    assert_eq!(expected["source_bindings"], oracle["bindings"]);
    assert_eq!(expected["prepared_files"], oracle["files"]);
    let mut request = preview_request;
    request["operation"] = Value::String("claims.create".into());
    request["command_id"] = Value::String("synthetic:claim-creation-first".into());
    request["expected_configuration"] = expected["owner_configuration"].clone();
    request["expected_revision"] = Value::Null;
    request["expected_dependencies"] = expected["expected_dependencies"].clone();
    request["expected_inputs"] = expected["source_bindings"].clone();
    context.request_raw = serde_json::to_vec(&request).unwrap();
    let historical_dependencies = oracle["dependencies"].clone();
    assert_ne!(
        request["expected_dependencies"], historical_dependencies,
        "owner migration must change the live Claim dependency fingerprint"
    );
    let mut stale_request = request.clone();
    stale_request["expected_dependencies"] = historical_dependencies;
    let mut stale_context = context.clone();
    stale_context.request_raw = serde_json::to_vec(&stale_request).unwrap();
    let mut stale_worker = claim_worker();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &stale_context,
            &cut,
            &cut,
            &software,
            &components,
            &mut stale_worker,
            None,
            deadline,
            &cancellation,
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    drop(stale_worker);
    let (created, publication, result) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &cut,
        &software,
        &components,
        &mut worker,
        None,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(worker);
    assert!(!publication.replayed);
    assert_eq!(created.files().len(), 5);
    assert_eq!(
        created.command().commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    for (name, raw) in created.files() {
        assert_eq!(
            fs::read(isolated.path().join(created.home().as_str()).join(name)).unwrap(),
            *raw
        );
    }
    let result = serde_json::from_slice::<Value>(&source_bytes(&result)).unwrap();
    assert_eq!(
        result["receipt"]["schema_version"],
        "tos_local_claim_create_receipt_v1"
    );
    assert_eq!(result["receipt"]["grants_admission"], false);
    // The operational sidecar is private metadata outside the five authored
    // creation files; native writer cases exercise its lock exclusion.
    let form_name = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim["claim_id"].as_str().unwrap().as_bytes()).to_hex()
    );
    let lock_path = isolated
        .path()
        .join(&source_path)
        .with_file_name(format!(".{form_name}.writer.lock"));
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&lock_path)
        .unwrap();
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        fs::metadata(&lock_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(fs::read(&lock_path).unwrap().is_empty());
    let mut current_files = authored.clone();
    for (name, raw) in created.files() {
        current_files.insert(format!("{}/{name}", created.home().as_str()), raw.clone());
    }
    let current = successor(&current_files, &store, base);
    let current_cut = open_cut(&store, current, deadline, &cancellation);
    let original_creation_files = created.files().clone();
    let creation_home = created.home().as_str().to_owned();
    drop(created);
    let mut replay_worker = claim_worker();
    let (restored, replayed, replay_result) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut replay_worker,
        None,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(replay_worker);
    assert!(replayed.replayed);
    assert!(fs::read(&lock_path).unwrap().is_empty());
    assert!(restored.command().changes.is_empty());
    assert_eq!(replay_result, restored.command().response);
    assert_eq!(
        restored.files()["source-create-receipt.json"],
        fs::read(
            isolated
                .path()
                .join(restored.home().as_str())
                .join("source-create-receipt.json")
        )
        .unwrap()
    );
    drop(restored);
    fs::write(&lock_path, b"unexpected operational lock data").unwrap();
    let mut nonempty_lock_worker = claim_worker();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut nonempty_lock_worker,
            None,
            deadline,
            &cancellation,
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    drop(nonempty_lock_worker);
    fs::write(&lock_path, b"").unwrap();
    let evidence_path = isolated
        .path()
        .join(claim["evidence_refs"][0].as_str().unwrap());
    let evidence_original = fs::read(&evidence_path).unwrap();
    fs::write(&evidence_path, b"Changed after the selected Claim cut.\n").unwrap();
    let mut stale_source_worker = claim_worker();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut stale_source_worker,
            None,
            deadline,
            &cancellation,
        ),
        Err(SourceCommandError::Conflict(_))
    ));
    drop(stale_source_worker);
    fs::write(&evidence_path, evidence_original).unwrap();
    let prior = fs::read(&owner).unwrap();
    let mut revoked = configuration;
    revoked["allowed_operations"] = serde_json::json!([]);
    fs::write(&owner, source_bytes(&source_value(&revoked))).unwrap();
    let mut revoked_worker = claim_worker();
    assert!(matches!(
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut revoked_worker,
            None,
            deadline,
            &cancellation,
        ),
        Err(SourceCommandError::Conflict(_) | SourceCommandError::Denied(_))
    ));
    drop(revoked_worker);
    fs::write(&owner, prior).unwrap();

    // Correct the genuinely published Claim through its native isolated owner.
    // The maintained producer is an independent proposal oracle only; it
    // never mutates this root or supplies the native publication authority.
    revision_owner["uid"] = serde_json::json!(uid);
    revision_owner["source_root"] = serde_json::json!(isolated.path());
    let revision_owner_path = isolated.path().join("revision-owner.json");
    fs::write(
        &revision_owner_path,
        serde_json::to_vec(&revision_owner).unwrap(),
    )
    .unwrap();
    fs::set_permissions(&revision_owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let revised_wording = format!(
        "{} [retained correction; no assessment]",
        claim["qualifiers"]["statement"].as_str().unwrap()
    );
    // Author the four exact components against the actual physical v1. This
    // is one native test composition, not four generic template commits.
    for (path, raw, _, _) in &natural_seed_literals {
        let physical = isolated.path().join(path);
        assert_eq!(fs::metadata(&physical).unwrap().len(), raw.len() as u64);
        natural_raw_admit(raw.len().checked_mul(2).unwrap().checked_add(4096).unwrap());
        assert_eq!(fs::read(&physical).unwrap(), raw.as_bytes());
        assert_eq!(current_files[*path], raw.as_bytes());
    }
    let natural_v1_path = isolated.path().join(&source_path);
    assert!(fs::metadata(&natural_v1_path).unwrap().len() <= 16_384);
    natural_raw_admit(
        usize::try_from(fs::metadata(&natural_v1_path).unwrap().len())
            .unwrap()
            .checked_mul(2)
            .unwrap()
            .checked_add(4096)
            .unwrap(),
    ); // read Vec before allocation
    let natural_v1_raw = fs::read(&natural_v1_path).unwrap();
    let natural_target = natural_parse(&natural_v1_raw);
    natural_admit(natural_v1_raw.len(), 2);
    let natural_v1: Value = serde_json::from_slice(&natural_v1_raw).unwrap();
    assert_eq!(natural_v1, claim);
    let natural_target_ref = metadata_subject(&natural_target).unwrap();
    let natural_target_ref: Value =
        serde_json::from_slice(&source_bytes(&natural_target_ref)).unwrap();
    assert_eq!(natural_target_ref, result["receipt"]["claims"][0]);
    // Supplement values come from actual selected source metadata, not receipts.
    let declarations = &natural_source["extensions"]["declarations"];
    let supplements = &natural_source["extensions"]["binding_supplements"];
    for (key, value) in [
        ("relative_constraint", &natural_intents[0]["input"]),
        ("right_censored_validity", &natural_intents[1]["input"]),
        (
            "recurrence_preserve",
            &natural_intents[2]["input"]["recurrence_pattern"],
        ),
        ("event_time", &natural_intents[3]["input"]["event_time"]),
        (
            "source_publication_time",
            &natural_intents[3]["input"]["source_publication_time"],
        ),
        (
            "system_ingest_time",
            &natural_intents[3]["input"]["system_ingest_time"],
        ),
        (
            "historical_author_knowledge",
            &natural_intents[3]["input"]["historical_author_knowledge"],
        ),
    ] {
        natural_raw_admit(
            declarations[key]
                .as_str()
                .unwrap()
                .len()
                .checked_mul(128)
                .unwrap()
                .checked_add(4096)
                .unwrap(),
        );
        assert_eq!(
            declarations[key].as_str().unwrap().as_bytes(),
            source_bytes(&source_value(value))
        );
    }
    natural_preallocate(4096, 128, 2); // disjoint components + full packet
    let natural_basis = |key: &str| {
        serde_json::json!({
            "source":natural_seed_refs[2], "source_locator":format!("/extensions/declarations/{key}"),
            "raw_evidence":declarations[key]
        })
    };
    let natural_trs = &supplements["censor_trs"];
    let natural_censor = &natural_intents[1]["input"]["validity_assertion"];
    let natural_packet = serde_json::json!({
        "schema_version":"tos_temporal_claim_carrier_v1",
        "relative_relations":[{"subject_a":natural_seed_refs[0]["id"],"subject_b":natural_seed_refs[1]["id"],
            "relation":natural_intents[0]["input"]["relation"],"source":natural_seed_refs[2],
            "asserted_by":supplements["relative_asserted_by"],"wording_ref":natural_seed_refs[2],
            "epistemic_status":supplements["source_status"]}],
        "validity":{"type":"RightCensored",
            "beginning":{"type":"CalendarPosition","lexical":natural_censor["valid_from"],
                "resolution":supplements["censor_resolution"],"trs":natural_trs},
            "observed_through":{"type":"CalendarPosition","lexical":natural_censor["observed_through"],
                "resolution":supplements["censor_resolution"],"trs":natural_trs},
            "cutoff_basis":natural_basis("right_censored_validity")},
        "recurrence":{"profile_ref":supplements["recurrence_profile_ref"],
            "pattern":natural_intents[2]["input"]["recurrence_pattern"],"basis":natural_basis("recurrence_preserve")},
        "roles":[
            {"role":"WorldOccurrence","assertion":natural_intents[3]["input"]["event_time"],"basis":natural_basis("event_time")},
            {"role":"SourcePublication","assertion":natural_intents[3]["input"]["source_publication_time"],"basis":natural_basis("source_publication_time")},
            {"role":"SystemTransaction","assertion":natural_intents[3]["input"]["system_ingest_time"],"basis":natural_basis("system_ingest_time")},
            {"role":"AttributedKnowledge","assertion":natural_intents[3]["input"]["historical_author_knowledge"],"basis":natural_basis("historical_author_knowledge")}
        ]
    });
    natural_shape(&natural_packet, 128);
    assert_eq!(source_bytes(&source_value(&natural_packet)).len(), 3485);
    assert!(
        revision_owner["allowed_form_ids"][0]
            .as_str()
            .unwrap()
            .len()
            <= 128
    );
    let natural_codec = TemporalBudget::default();
    let natural_config_sha = Digest256::of_bytes(&source_bytes(&source_value(&revision_owner)));
    // R655ad4c1 confirms a source/ELF reconstruction, not runtime telemetry:
    // roles changes from 2 to 3 MiB; the other children decrease, total stays
    // exactly 8 MiB. No unused child credit is refunded to this same parent.
    assert_eq!(std::mem::size_of::<JsonValue>(), 48);
    assert_eq!(
        std::mem::size_of::<(tos_foundation::JsonString, JsonValue)>(),
        96
    );
    let natural_plan_shares = [1_572_864usize, 1_835_008, 1_835_008, 3_145_728];
    assert_eq!(
        natural_plan_shares
            .iter()
            .try_fold(0usize, |sum, share| sum.checked_add(*share))
            .unwrap(),
        8_388_608
    );
    for (index, component, alias) in [
        (0, "relative_relations", "explicit-relative-storage-target"),
        (1, "validity", "state-A"),
        (2, "recurrence", "recurrence-A"),
        (3, "roles", "event-1500"),
    ] {
        natural_debit(natural_plan_shares[index], 131_072); // whole share before any call
        natural_preallocate(4096, 128, 2); // binding plus serialization view
        let mut payload = serde_json::json!({"schema_version":"tos_temporal_claim_carrier_v1"});
        payload[component] = natural_packet[component].clone();
        let identities = if index == 0 {
            serde_json::json!({"event-A":natural_seed_refs[0]["id"],"reign-R":natural_seed_refs[1]["id"]})
        } else {
            serde_json::json!({})
        };
        let sources = if index == 0 {
            serde_json::json!({"source-1":natural_seed_refs[2]})
        } else {
            serde_json::json!({})
        };
        let trs = if index == 1 {
            serde_json::json!({"TRS:GregorianProleptic:fixture-v1":natural_trs})
        } else {
            serde_json::json!({})
        };
        let binding = serde_json::json!({"schema_version":"tos_temporal_claim_intent_binding_v1",
            "target":{"alias":alias,"revision_alias":"r1","next_revision_alias":null,
                "source":natural_target_ref,"source_path":source_path,"source_revision":current.0.to_prefixed(),
                "configuration_digest":natural_config_sha.to_prefixed()},
            "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
            "authored_reason":"Explicit synthetic source-component storage; not historical verification.",
            "mapping_profile":"fixture:explicit-original-component-storage-v1",
            "payload":payload,"identities":identities,"sources":sources,"trs":trs});
        natural_shape(&binding, 128);
        let binding_raw = source_bytes(&source_value(&binding));
        assert!(binding_raw.len() <= 4096);
        let binding_value = natural_parse(&binding_raw);
        let intent_value = natural_parse(natural_intent_literals[index].as_bytes());
        let plan = plan_claim_intent(
            &intent_value,
            &binding_value,
            &natural_target,
            natural_codec,
            ClaimIntentPrepareBudget {
                max_extra_owned_bytes: natural_plan_shares[index],
                max_extra_json_visits: 131_072,
                max_preview_bytes: 16_384,
                max_output_bytes: 16_384,
            },
        )
        .unwrap();
        assert!(!plan.owner_checked());
        assert_eq!(plan.basis_status(), "SourceSuppliedUnverified");
        assert_eq!(binding["payload"][component], natural_packet[component]);
        drop(plan);
    }
    assert_eq!(natural_parent.get(), (16_777_216, 410_176));
    // Retained full-source/context overlap is admitted before existing clones;
    // it does not replenish the four spent pure-plan children.
    let natural_context_bytes = current_files
        .values()
        .chain(
            files
                .iter()
                .filter(|(name, _)| !name.starts_with("ToS/"))
                .map(|(_, raw)| raw),
        )
        .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
        .unwrap();
    natural_debit(
        natural_context_bytes
            .checked_mul(2)
            .unwrap()
            .checked_add(65_536)
            .unwrap(),
        4096,
    );
    natural_preallocate(2048, 128, 2); // exact original reason and retained view
    natural_preallocate(8192, 256, 2); // native patch and codec view BEFORE construction
    let natural_reason = serde_json::to_string(&serde_json::json!({
        "authored_reason":"Retained source-copy correction in an isolated Claim corpus; no assessment.",
        "preserved_intents":natural_intents})).unwrap();
    assert!(natural_reason.chars().count() <= 4096);
    let correction = serde_json::json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-revise",
        "fields":{"qualifiers":{"statement":revised_wording,"temporal_claim_carrier":natural_packet}},
        "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
        "reason":natural_reason
    });
    assert_eq!(
        correction["fields"]["qualifiers"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(correction["fields"].as_object().unwrap().len(), 1);
    natural_shape(&correction, 256);
    let natural_patch_raw = source_bytes(&source_value(&correction));
    assert!(natural_patch_raw.len() <= 8192);
    // Separate parser reservation; the old codec-view reservation stays intact.
    // The authored request is already in the native canonical wire domain.
    // No budget is refunded when this temporary expected DOM is dropped.
    let (natural_retained_bytes, natural_retained_visits) = natural_retained.get();
    natural_retained.set((
        natural_retained_bytes.checked_sub(105_496).unwrap(),
        natural_retained_visits.checked_sub(512).unwrap(),
    ));
    let natural_expected_document = tos_foundation::parse_json_with_state_budget(
        &natural_patch_raw,
        JsonMode::PublishedStrict,
        natural_limits,
        105_496,
    )
    .unwrap();
    let natural_expected_qualifiers = natural_expected_document
        .root()
        .object_get("fields")
        .unwrap()
        .object_get("qualifiers")
        .unwrap();
    let natural_expected_typed = decode_claim_carrier(natural_expected_qualifiers, natural_codec)
        .unwrap()
        .unwrap();
    drop(natural_expected_document);
    assert_eq!(natural_expected_typed.roles().unwrap().len(), 4);
    assert_eq!(
        natural_expected_typed.relative_relations().unwrap().len(),
        1
    );
    assert!(
        natural_expected_typed.validity().is_some()
            && natural_expected_typed.recurrence().is_some()
    );
    eprintln!(
        "natural original components: pure_plans=4 static_records=3 static_bytes=2897 constructors=9 owner_entries=10 chain=3; source-only binding, no admission"
    );
    let revision_filesystem = CreationFilesystem::select_isolated(
        &isolated,
        &revision_owner_path,
        deadline,
        &cancellation,
    )
    .unwrap();
    let revision_software = files
        .iter()
        .filter(|(name, _)| !name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut revision_context = selected_context(
        &current_files,
        &revision_software,
        &revision_owner,
        &correction,
        current,
    );
    revision_context.effective_uid = u64::from(uid);
    let mut preview_budget = ExecutorBudget::laboratory();
    preview_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!preview_budget.execution_wall.is_zero());
    let mut revision_preview_worker = super::command_form_cases::schemas_for_profile_with_image(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        preview_budget,
        deadline,
        &cancellation,
    );
    check_simple_claim_selected_inputs(&simple_inputs, &current_files, &mut claim_reference_budget)
        .unwrap();
    let retained_simple_reference =
        expected_simple_claim_bindings(&simple_inputs, &mut claim_reference_budget).unwrap();
    claim_reference_equal(
        &retained_simple_reference,
        &simple_reference,
        &mut claim_reference_budget,
        0,
    )
    .unwrap();
    check_historical_simple_inputs(
        &simple_inputs,
        include_bytes!("legacy-python-oracles-v1/claim-revision-selected-history.provenance.json"),
        "claim-revision-selected-history",
        &revision_owner,
        &correction,
        false,
        isolated.path(),
        &mut claim_reference_budget,
    )
    .unwrap();
    let retained_context_digests = check_declared_claim_context(
        &revision_context,
        &current_files,
        &revision_software,
        &revision_owner,
        &correction,
        current,
        &mut claim_reference_budget,
    )
    .unwrap();
    let prepared_correction = prepare_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_preview_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_preview_worker);
    let prepared = response(&prepared_correction);
    let independent = maintained_oracle("claim-revision-selected-history");
    claim_reference_equal(
        &retained_simple_reference,
        &independent["source_bindings"],
        &mut claim_reference_budget,
        0,
    )
    .unwrap();
    claim_reference_equal(
        &prepared["source_bindings"],
        &retained_simple_reference,
        &mut claim_reference_budget,
        0,
    )
    .unwrap();
    assert_eq!(
        prepared_correction.configuration_canonical_sha256,
        retained_context_digests.0
    );
    assert_eq!(
        prepared_correction.request_canonical_sha256,
        retained_context_digests.1
    );
    assert_eq!(
        prepared_correction.configuration_raw_sha256,
        retained_context_digests.2
    );
    // Current expected dependencies come from the native owner/source cut;
    // `independent` retains the frozen historical Python packet.
    assert_eq!(prepared["source_bindings"], independent["source_bindings"]);
    let mut revision_request = correction;
    revision_request["operation"] = Value::String("claim.revise".into());
    revision_request["command_id"] =
        Value::String("synthetic:claim-creation-retained-correction".into());
    revision_request["expected_configuration"] = prepared["owner_configuration"].clone();
    revision_request["expected_source"] = prepared["source"].clone();
    revision_request["expected_revision"] = prepared["revision"].clone();
    revision_request["expected_dependencies"] = prepared["expected_dependencies"].clone();
    revision_request["expected_inputs"] = prepared["source_bindings"].clone();
    natural_preallocate(16_384, 256, 1); // exact request/digest and retained response
    natural_shape(&revision_request, 256);
    let natural_request_digest =
        Digest256::of_bytes(&source_bytes(&source_value(&revision_request)));
    revision_context.request_raw = serde_json::to_vec(&revision_request).unwrap();
    let mut revision_budget = ExecutorBudget::laboratory();
    revision_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!revision_budget.execution_wall.is_zero());
    let mut revision_worker = super::command_form_cases::schemas_for_profile_with_image(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        revision_budget,
        deadline,
        &cancellation,
    );
    let (revised_plan, revision_publication) = execute_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_worker);
    assert!(!revision_publication.replayed);
    assert_eq!(
        revised_plan.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    let correction = serde_json::json!({"result":response(&revised_plan)});
    assert_eq!(correction["result"]["replayed"], false);
    assert_eq!(correction["result"]["source"]["version"], 2);
    assert_eq!(correction["result"]["receipt"]["grants_admission"], false);
    assert_eq!(
        correction["result"]["receipt"]["previous_source"],
        result["receipt"]["claims"][0]
    );
    let revised_home = isolated.path().join(&creation_home);
    let expected_names = original_creation_files
        .keys()
        .cloned()
        .chain([
            form_name.clone(),
            "claim-revision-history.json".into(),
            format!(".{form_name}.writer.lock"),
        ])
        .collect::<std::collections::BTreeSet<_>>();
    let actual_names = fs::read_dir(&revised_home)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_names, expected_names);
    assert!(fs::read(&lock_path).unwrap().is_empty());
    let mut corrected_files = current_files;
    for name in &actual_names {
        if name.ends_with(".writer.lock") {
            continue;
        }
        let current_path = revised_home.join(name);
        assert_eq!(
            fs::metadata(&current_path).unwrap().permissions().mode() & 0o7777,
            0o600,
            "maintained corrected metadata uses private inode mode"
        );
        let raw = fs::read(current_path).unwrap();
        if name.starts_with("source-create-") {
            assert_eq!(&raw, &original_creation_files[name]);
        }
        corrected_files.insert(format!("{creation_home}/{name}"), raw);
    }
    // Read back actual v2, not the original creation replay response.
    let natural_current_raw = &corrected_files[&source_path];
    assert!(natural_current_raw.len() <= 8192);
    let natural_current = natural_parse(natural_current_raw);
    natural_admit(natural_current_raw.len(), 1); // serde view; reuse native tree for actual ref
    natural_raw_admit(
        natural_current_raw
            .len()
            .checked_mul(4)
            .unwrap()
            .checked_add(4096)
            .unwrap(),
    );
    let natural_current_value: Value = serde_json::from_slice(natural_current_raw).unwrap();
    assert_eq!(natural_current_value["claim_version"], 2);
    assert_eq!(
        natural_current_value["qualifiers"]["statement"],
        revised_wording
    );
    assert_eq!(
        natural_current_value["qualifiers"]["temporal_claim_carrier"],
        natural_packet
    );
    assert_eq!(
        natural_current_value["qualifiers"]
            .as_object()
            .unwrap()
            .len(),
        claim["qualifiers"].as_object().unwrap().len() + 1
    );
    for (key, value) in claim["qualifiers"].as_object().unwrap() {
        if key != "statement" {
            assert_eq!(&natural_current_value["qualifiers"][key], value);
        }
    }
    let current_qualifiers = natural_current.object_get("qualifiers").unwrap();
    assert_eq!(
        decode_claim_carrier(current_qualifiers, natural_codec)
            .unwrap()
            .unwrap(),
        natural_expected_typed
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&source_bytes(
            &metadata_subject(&natural_current).unwrap()
        ))
        .unwrap(),
        correction["result"]["receipt"]["source"]
    );
    assert_eq!(
        correction["result"]["receipt"]["request_digest"],
        natural_request_digest.to_prefixed()
    );
    assert_eq!(correction["result"]["receipt"]["reason"], natural_reason);
    let form: Value =
        serde_json::from_slice(&corrected_files[&format!("{creation_home}/{form_name}")]).unwrap();
    assert_eq!(form["forms"][0]["role"], "statement");
    assert_eq!(
        form["forms"][0]["bindings"]["wording"]["pointer"],
        "/qualifiers/statement"
    );
    let history: Value = serde_json::from_slice(
        &corrected_files[&format!("{creation_home}/claim-revision-history.json")],
    )
    .unwrap();
    assert_eq!(history["receipts"].as_array().unwrap().len(), 1);
    assert_eq!(history["receipts"][0], correction["result"]["receipt"]);
    let archive = correction["result"]["receipt"]["archive_path"]
        .as_str()
        .unwrap();
    let archive_dir = isolated.path().join(archive);
    let manifest: Value =
        serde_json::from_slice(&fs::read(archive_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["files"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        original_creation_files
            .keys()
            .cloned()
            .chain([format!(".{form_name}.writer.lock")])
            .collect::<std::collections::BTreeSet<_>>()
    );
    for (name, original_raw) in &original_creation_files {
        let blob = manifest["files"][name]["blob"].as_str().unwrap();
        assert_eq!(fs::read(archive_dir.join(blob)).unwrap(), *original_raw);
    }
    let lock_name = format!(".{form_name}.writer.lock");
    let lock_blob = manifest["files"][lock_name.as_str()]["blob"]
        .as_str()
        .unwrap();
    assert!(fs::read(archive_dir.join(lock_blob)).unwrap().is_empty());
    let archive_entries = fs::read_dir(&archive_dir).unwrap().collect::<Vec<_>>();
    assert!(archive_entries.len() <= 65);
    let mut archive_bytes = 0u64;
    for entry in archive_entries {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let metadata = entry.metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() <= 2_097_152);
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
        archive_bytes += metadata.len();
        assert!(archive_bytes <= 10_485_760);
        corrected_files.insert(format!("{archive}/{name}"), fs::read(entry.path()).unwrap());
    }
    let corrected = successor(&corrected_files, &store, current);
    let corrected_cut = open_cut(&store, corrected, deadline, &cancellation);
    let mut corrected_worker = claim_worker();
    let (restored_after_correction, replay_after_correction, corrected_result) =
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &corrected_cut,
            &software,
            &components,
            &mut corrected_worker,
            None,
            deadline,
            &cancellation,
        )
        .unwrap();
    drop(corrected_worker);
    assert!(replay_after_correction.replayed);
    assert!(restored_after_correction.command().changes.is_empty());
    assert_eq!(restored_after_correction.files(), &original_creation_files);
    assert_eq!(
        corrected_result,
        restored_after_correction.command().response
    );
    assert!(fs::read(&lock_path).unwrap().is_empty());
}

#[test]
fn initial_identity_proposals_retain_selected_catalog_and_cold_replay() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    for (version, predicate, schema) in [
        (
            "v1",
            "identity_transition_proposal",
            "tos_source_identity_transition_claim_v1",
        ),
        (
            "v2",
            "subject_identity_transition_proposal",
            "tos_subject_identity_transition_claim_v1",
        ),
    ] {
        let (mut files, old_claim, _, _) = claim_fixture();
        let agent_path = "ToS/source-witnesses/agents/command-claim-fixture/agent.json";
        let first: Value = serde_json::from_slice(&files[agent_path]).unwrap();
        let mut records = vec![first];
        for (suffix, label) in [
            ("second", "Second synthetic identity endpoint"),
            ("third", "Third synthetic identity endpoint"),
        ] {
            let mut record = records[0].clone();
            record["record_id"] = Value::String(format!("tos.agent.command-identity-{suffix}"));
            record["preferred_label"] = Value::String(label.into());
            let path = format!("ToS/source-witnesses/agents/command-identity-{suffix}/agent.json");
            files.insert(path, source_bytes(&source_value(&record)));
            records.push(record);
        }
        let refs = records.iter().map(source_ref).collect::<Vec<_>>();
        let predecessor = refs[0].clone();
        let successors = refs[1..].to_vec();
        let identities = refs
            .iter()
            .map(|reference| reference["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        // The structured value retains its own attributed wording; the
        // qualifier statement below remains a separate Claim assertion.
        let object = serde_json::json!({
            "kind":"identity-transition-proposal", "operation":"split",
            "source_wording":{
                "text":"Synthetic source wording proposes one provisional Agent identity splitting into two; no transition is performed.",
                "language":"en", "script":"Latn"
            },
            "members":identities, "predecessors":[predecessor], "successors":successors,
            "mapping":[
                {"predecessor":refs[0]["id"],"successor":refs[1]["id"]},
                {"predecessor":refs[0]["id"],"successor":refs[2]["id"]}
            ],
            "grounds":"Synthetic exact metadata versions; no identity admission.",
            "counterreading":"The three endpoints may remain distinct.",
            "scope":"Proposal for source metadata comparison only.",
            "unresolved_links":[], "supersedes_proposal":null
        });
        let mut claim = old_claim;
        claim["claim_id"] = Value::String(format!("tos.claim.synthetic-identity-{version}"));
        claim["claim_version"] = Value::from(1);
        claim["claim_type"] = Value::String("relation".into());
        claim["assertion_layer"] = Value::String("identity_assertion".into());
        claim["schema_version"] = Value::String(schema.into());
        claim["predicate"] = Value::String(predicate.into());
        claim["subject_ref"] = refs[0]["id"].clone();
        claim["object"] = object.clone();
        claim["qualifiers"] = serde_json::json!({
            "statement":"The selected metadata supports only a proposed split.",
            "statement_language":"en", "statement_script":"Latn"
        });
        claim["assessment_refs"] = serde_json::json!([]);
        claim["supersedes_claim_ref"] = Value::Null;
        let source_path = format!(
            "ToS/source-witnesses/relations/command-identity-{version}/source-claims.jsonl"
        );
        let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
            .iter()
            .chain(CLAIM_REVISION_RULE_INPUTS)
            .copied()
            .filter(|name| !name.starts_with("ToS/"))
            .collect::<Vec<_>>();
        software_names.extend([
            "rust/crates/tos-command/src/source_claims.rs",
            "rust/crates/tos-command/src/source_serialization.rs",
        ]);
        software_names.sort_unstable();
        software_names.dedup();
        for name in software_names {
            files.insert(name.into(), fs::read(repository.join(name)).unwrap());
        }
        let cancellation = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(240);
        let (_capture, software, components) =
            super::command_record_cases::captured_components(&files, deadline, &cancellation);
        let temporary = tempfile::tempdir().unwrap();
        let authored = files
            .iter()
            .filter(|(name, _)| name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        let store = temporary.path().join("selected-store");
        let base = super::validation_cut_cases::write_cut_store(&authored, &store);
        let cut = open_cut(&store, base, deadline, &cancellation);
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
        for (name, raw) in &files {
            let target = isolated.path().join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, raw).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        }
        fs::create_dir_all(
            isolated
                .path()
                .join(&source_path)
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .unwrap();
        publish_fixture_catalog(&repository, isolated.path(), deadline);
        let uid = fs::metadata(isolated.path()).unwrap().uid();
        let configuration = serde_json::json!({
            "schema_version":format!("tos_local_identity_proposal_create_owner_{version}"), "uid":uid,
            "principal_id":claim["maker"]["agent_ref"],
            "maker_type":claim["maker"]["maker_type"],
            "source_root":isolated.path(), "source_path":source_path,
            "authority_ref":"synthetic-test-only:identity-proposal-no-admission",
            "expires_at":"2099-01-01T00:00:00Z",
            "provenance_event_id":claim["provenance_event_ref"],
            "allowed_operations":["claims.create"],
            "allowed_claim_ids":[claim["claim_id"]],
            "allowed_subject_refs":[claim["subject_ref"]],
            "allowed_object_refs":refs.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
            "allowed_object_values":[object],
            "allowed_related_claim_refs":[],
            "allowed_predicates":[claim["predicate"]],
            "allowed_evidence_refs":claim["evidence_refs"]
        });
        let owner = isolated.path().join("identity-owner.json");
        fs::write(&owner, source_bytes(&source_value(&configuration))).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation)
                .unwrap();
        let software_files = files
            .iter()
            .filter(|(name, _)| !name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        let preview_request = serde_json::json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"prepare-create", "claims":[claim]
        });
        let mut context = selected_context(
            &authored,
            &software_files,
            &configuration,
            &preview_request,
            base,
        );
        context.configuration_raw = fs::read(&owner).unwrap();
        context.effective_uid = u64::from(uid);
        let mut budget = ExecutorBudget::laboratory();
        budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!budget.execution_wall.is_zero());
        // The same operation keeps its sealed image through creation and replay.
        // Schema receipts and cut state remain private to each fresh adapter.
        let worker_image = super::command_form_cases::schema_image(budget, deadline, &cancellation);
        let mut worker = super::command_form_cases::schemas_for_profile_with_image(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            budget,
            deadline,
            &cancellation,
        );
        let preview = prepare_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        let preview = response(&preview);
        assert_eq!(preview["grants_admission"], false);
        assert_eq!(
            preview["source_bindings"]["identity_proposals"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
        let mut request = preview_request;
        request["operation"] = Value::String("claims.create".into());
        request["command_id"] =
            Value::String(format!("synthetic:identity-proposal-create-{version}"));
        request["expected_configuration"] = preview["owner_configuration"].clone();
        request["expected_revision"] = Value::Null;
        request["expected_dependencies"] = preview["expected_dependencies"].clone();
        request["expected_inputs"] = preview["source_bindings"].clone();
        context.request_raw = serde_json::to_vec(&request).unwrap();
        let (created, publication, _) = execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &cut,
            &software,
            &components,
            &mut worker,
            None,
            deadline,
            &cancellation,
        )
        .unwrap();
        drop(worker);
        assert!(!publication.replayed);
        assert_eq!(created.files().len(), 5);
        let original = created.files().clone();
        let mut current_files = authored;
        for (name, raw) in created.files() {
            current_files.insert(format!("{}/{}", created.home().as_str(), name), raw.clone());
        }
        let current = successor(&current_files, &store, base);
        let current_cut = open_cut(&store, current, deadline, &cancellation);
        publish_fixture_catalog(&repository, isolated.path(), deadline);
        let mut replay_budget = ExecutorBudget::laboratory();
        replay_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!replay_budget.execution_wall.is_zero());
        let mut replay_worker = super::command_form_cases::schemas_for_profile_with_image(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            replay_budget,
            deadline,
            &cancellation,
        );
        let mut current_budget = ExecutorBudget::laboratory();
        current_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!current_budget.execution_wall.is_zero());
        let mut current_worker = super::command_form_cases::schemas_for_profile_with_image(
            &current_cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            current_budget,
            deadline,
            &cancellation,
        );
        let (restored, replayed, result) = execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut replay_worker,
            Some(&mut current_worker),
            deadline,
            &cancellation,
        )
        .unwrap();
        assert!(replayed.replayed);
        assert_eq!(restored.files(), &original);
        assert_eq!(result, restored.command().response);
        assert_eq!(
            response(restored.command())["receipt"]["grants_admission"],
            false
        );
        drop(restored);
        drop(replay_worker);
        drop(current_worker);

        // Revise the profiled Claim through the same protected native owner.
        // The original catalogue companion remains bound to creation while
        // the current generated catalogue and cut advance with this command.
        let form_name = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(claim["claim_id"].as_str().unwrap().as_bytes()).to_hex()
        );
        let lock_name = format!(".{form_name}.writer.lock");
        let home = created.home().as_str();
        let lock_path = isolated.path().join(home).join(&lock_name);
        fs::write(&lock_path, b"").unwrap();
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();
        let revision_owner = serde_json::json!({
            "schema_version":format!("tos_local_identity_proposal_revision_owner_{version}"),
            "uid":uid, "principal_id":claim["maker"]["agent_ref"],
            "source_root":isolated.path(), "source_path":source_path,
            "authority_ref":"synthetic-test-only:identity-correction-no-admission",
            "expires_at":"2099-01-01T00:00:00Z", "claim_id":claim["claim_id"],
            "allowed_operations":["claim.revise"], "allowed_fields":["qualifiers"],
            "allowed_evidence_refs":claim["evidence_refs"],
            "allowed_form_ids":[format!("tos.form.synthetic-identity-{version}")],
            "allowed_form_field_ids":["claim.statement"],
            "allowed_object_values":[claim["object"]],
            "allowed_object_refs":refs.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
            "allowed_related_claim_refs":[]
        });
        let revision_owner_path = isolated.path().join("identity-revision-owner.json");
        fs::write(
            &revision_owner_path,
            source_bytes(&source_value(&revision_owner)),
        )
        .unwrap();
        fs::set_permissions(&revision_owner_path, fs::Permissions::from_mode(0o600)).unwrap();
        let revision_filesystem = CreationFilesystem::select_isolated(
            &isolated,
            &revision_owner_path,
            deadline,
            &cancellation,
        )
        .unwrap();
        let correction = serde_json::json!({
            "schema_version":"tos_local_source_command_v1", "operation":"prepare-revise",
            "fields":{"qualifiers":{"statement":format!(
                "{} [retained identity wording correction; no admission]",
                claim["qualifiers"]["statement"].as_str().unwrap()
            )}},
            "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
            "reason":"Retain an isolated identity proposal wording correction."
        });
        let mut revision_context = selected_context(
            &current_files,
            &software_files,
            &revision_owner,
            &correction,
            current,
        );
        revision_context.configuration_raw = fs::read(&revision_owner_path).unwrap();
        revision_context.effective_uid = u64::from(uid);
        let mut preview_budget = ExecutorBudget::laboratory();
        preview_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!preview_budget.execution_wall.is_zero());
        let mut revision_preview_worker = super::command_form_cases::schemas_for_profile_with_image(
            &current_cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            preview_budget,
            deadline,
            &cancellation,
        );
        let revision_preview = prepare_isolated_claim_revision_from_captures(
            &revision_filesystem,
            &revision_context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut revision_preview_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        drop(revision_preview_worker);
        let prepared = response(&revision_preview);
        let independent = maintained_oracle(&format!("claim-revision-identity-history-{version}"));
        // Keep the historical packet intact after owner identity migration.
        assert_eq!(prepared["source_bindings"], independent["source_bindings"]);
        let mut revision_request = correction;
        revision_request["operation"] = Value::String("claim.revise".into());
        revision_request["command_id"] =
            Value::String(format!("synthetic:identity-proposal-correction-{version}"));
        revision_request["expected_configuration"] = prepared["owner_configuration"].clone();
        revision_request["expected_source"] = prepared["source"].clone();
        revision_request["expected_revision"] = prepared["revision"].clone();
        revision_request["expected_dependencies"] = prepared["expected_dependencies"].clone();
        revision_request["expected_inputs"] = prepared["source_bindings"].clone();
        revision_context.request_raw = serde_json::to_vec(&revision_request).unwrap();
        let mut revision_budget = ExecutorBudget::laboratory();
        revision_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!revision_budget.execution_wall.is_zero());
        let mut revision_worker = super::command_form_cases::schemas_for_profile_with_image(
            &current_cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            revision_budget,
            deadline,
            &cancellation,
        );
        let (revised, revision_publication) = execute_isolated_claim_revision_from_captures(
            &revision_filesystem,
            &revision_context,
            &cut,
            &current_cut,
            &software,
            &components,
            &mut revision_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        drop(revision_worker);
        assert!(!revision_publication.replayed);
        assert_eq!(
            revised.commit(),
            Err(SourceCommandError::MissingProductionAdmission)
        );
        let revised_result = response(&revised);
        assert_eq!(revised_result["receipt"]["grants_admission"], false);
        let archive = revised_result["receipt"]["archive_path"].as_str().unwrap();
        assert!(
            isolated
                .path()
                .join(archive)
                .join("manifest.json")
                .is_file()
        );
        let mut corrected_files = current_files;
        apply_changes(&mut corrected_files, &revised);
        for change in &revised.changes {
            if let Some(raw) = &change.after {
                assert_eq!(
                    fs::read(isolated.path().join(change.path.as_str())).unwrap(),
                    *raw
                );
            }
        }
        let corrected = successor(&corrected_files, &store, current);
        let corrected_cut = open_cut(&store, corrected, deadline, &cancellation);
        publish_fixture_catalog(&repository, isolated.path(), deadline);
        let mut original_budget = ExecutorBudget::laboratory();
        original_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!original_budget.execution_wall.is_zero());
        let mut original_worker = super::command_form_cases::schemas_for_profile_with_image(
            &cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            original_budget,
            deadline,
            &cancellation,
        );
        let mut latest_budget = ExecutorBudget::laboratory();
        latest_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!latest_budget.execution_wall.is_zero());
        let mut latest_worker = super::command_form_cases::schemas_for_profile_with_image(
            &corrected_cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            latest_budget,
            deadline,
            &cancellation,
        );
        let (restored_after_revision, original_replayed, original_result) =
            execute_isolated_claim_creation_from_captures(
                &filesystem,
                &context,
                &cut,
                &corrected_cut,
                &software,
                &components,
                &mut original_worker,
                Some(&mut latest_worker),
                deadline,
                &cancellation,
            )
            .unwrap();
        assert!(original_replayed.replayed);
        assert_eq!(restored_after_revision.files(), &original);
        assert_eq!(original_result, restored_after_revision.command().response);
        assert!(fs::read(&lock_path).unwrap().is_empty());
        drop(original_worker);
        drop(latest_worker);
        let mut retry_context = selected_context(
            &corrected_files,
            &software_files,
            &revision_owner,
            &revision_request,
            corrected,
        );
        retry_context.configuration_raw = fs::read(&revision_owner_path).unwrap();
        retry_context.effective_uid = u64::from(uid);
        let mut retry_budget = ExecutorBudget::laboratory();
        retry_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
        assert!(!retry_budget.execution_wall.is_zero());
        let mut retry_worker = super::command_form_cases::schemas_for_profile_with_image(
            &corrected_cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            retry_budget,
            deadline,
            &cancellation,
        );
        let (retried, retry_publication) = execute_isolated_claim_revision_from_captures(
            &revision_filesystem,
            &retry_context,
            &cut,
            &corrected_cut,
            &software,
            &components,
            &mut retry_worker,
            deadline,
            &cancellation,
        )
        .unwrap();
        assert!(retry_publication.replayed);
        assert!(retried.changes.is_empty());
        assert_eq!(response(&retried)["receipt"], revised_result["receipt"]);
    }
}

#[test]
fn initial_collection_order_binds_retained_version_and_cold_replays() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::{CreationFilesystem, IsolatedCreationRoot};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let (mut files, _, _, _) = claim_fixture();
    let collection = "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996";
    for leaf in [
        "collection.json",
        "collection.human-forms.json",
        "membership-claims.jsonl",
        "responsibility-claims.jsonl",
        "source-revision-history.json",
        "structure/work-boundaries/work-boundary-map.json",
        "structure/work-boundaries/anchors.jsonl",
    ] {
        let path = format!("{collection}/{leaf}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    let history: Value =
        serde_json::from_slice(&files[&format!("{collection}/source-revision-history.json")])
            .unwrap();
    let retained_history = super::retained_history_capture::retained_history(&repository);
    let retained_root = retained_history.path().join("source");
    for receipt in history["receipts"].as_array().unwrap() {
        let path = receipt["archive_path"].as_str().unwrap();
        let mut members = fs::read_dir(retained_root.join(path))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        members.sort();
        assert!(members.len() <= 64);
        for member in members {
            let relative = member
                .strip_prefix(&retained_root)
                .unwrap()
                .to_str()
                .unwrap();
            files.insert(relative.into(), fs::read(&member).unwrap());
        }
        // A historical Collection version is selected with its original
        // committed transport, not just its archived record package. Keep
        // the exact retained manifest, blobs and completion in both cuts so
        // the real native compound reader can authenticate this transition.
        let transaction_id = receipt["publication"]["transaction_id"]
            .as_str()
            .unwrap()
            .strip_prefix("sha256:")
            .unwrap();
        assert_eq!(transaction_id.len(), 64);
        assert!(transaction_id.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let transaction = format!("ToS/source-witnesses/.metadata-transactions/{transaction_id}");
        let mut selected = fs::read_dir(retained_root.join(&transaction))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        selected.sort();
        assert!(selected.len() <= 64);
        for member in selected {
            assert!(member.is_file());
            let relative = member
                .strip_prefix(&retained_root)
                .unwrap()
                .to_str()
                .unwrap();
            assert!(
                files
                    .insert(relative.into(), fs::read(&member).unwrap())
                    .is_none()
            );
        }
    }
    for work in [
        "also-sprach-zarathustra",
        "jenseits-von-gut-und-boese",
        "zur-genealogie-der-moral",
        "der-fall-wagner",
        "goetzen-daemmerung",
        "der-antichrist",
        "ecce-homo",
    ] {
        let path = format!("ToS/source-witnesses/works/friedrich-nietzsche/{work}/work.json");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    let source_claim =
        "ToS/source-witnesses/relations/mysl-1996-volume-2-member-order/source-claims.jsonl";
    let mut claim: Value =
        serde_json::from_slice(&fs::read(repository.join(source_claim)).unwrap()).unwrap();
    claim["claim_id"] = Value::String("tos.claim.synthetic-collection-order".into());
    claim["claim_version"] = Value::from(1);
    claim["provenance_event_ref"] = Value::String("tos.event.synthetic-collection-order".into());
    for evidence in claim["evidence_refs"].as_array().unwrap() {
        let path = evidence.as_str().unwrap();
        if !files.contains_key(path) {
            files.insert(path.into(), fs::read(repository.join(path)).unwrap());
        }
    }
    let source_path = "ToS/source-witnesses/relations/command-collection-order/source-claims.jsonl";
    let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
        .iter()
        .chain(CLAIM_REVISION_RULE_INPUTS)
        .copied()
        .filter(|name| !name.starts_with("ToS/"))
        .collect::<Vec<_>>();
    software_names.extend([
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
    ]);
    software_names.sort_unstable();
    software_names.dedup();
    for name in software_names {
        files.insert(name.into(), fs::read(repository.join(name)).unwrap());
    }
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(240);
    let (_capture, software, components) =
        super::command_record_cases::captured_components(&files, deadline, &cancellation);
    let temporary = tempfile::tempdir().unwrap();
    let authored = files
        .iter()
        .filter(|(name, _)| name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let store = temporary.path().join("selected-store");
    let base = super::validation_cut_cases::write_cut_store(&authored, &store);
    let cut = open_cut(&store, base, deadline, &cancellation);
    let isolated = IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
    for (name, raw) in &files {
        let target = isolated.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    }
    fs::create_dir_all(
        isolated
            .path()
            .join(source_path)
            .parent()
            .unwrap()
            .parent()
            .unwrap(),
    )
    .unwrap();
    publish_fixture_catalog(&repository, isolated.path(), deadline);
    let uid = fs::metadata(isolated.path()).unwrap().uid();
    let configuration = serde_json::json!({
        "schema_version":"tos_local_claim_create_owner_v4", "uid":uid,
        "principal_id":claim["maker"]["agent_ref"],
        "maker_type":claim["maker"]["maker_type"],
        "source_root":isolated.path(), "source_path":source_path,
        "authority_ref":"synthetic-test-only:collection-order-no-membership-grant",
        "expires_at":"2099-01-01T00:00:00Z",
        "provenance_event_id":claim["provenance_event_ref"],
        "allowed_operations":["claims.create"], "allowed_claim_ids":[claim["claim_id"]],
        "allowed_subject_refs":[claim["subject_ref"]],
        "allowed_object_refs":claim["object"]["members"],
        "allowed_object_values":[claim["object"]],
        "allowed_predicates":[claim["predicate"]],
        "allowed_evidence_refs":claim["evidence_refs"]
    });
    let owner = isolated.path().join("order-owner.json");
    let owner_raw = source_bytes(&source_value(&configuration));
    fs::write(&owner, &owner_raw).unwrap();
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
    let filesystem =
        CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation).unwrap();
    let software_files = files
        .iter()
        .filter(|(name, _)| !name.starts_with("ToS/"))
        .map(|(name, raw)| (name.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    let preview_request = serde_json::json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"prepare-create", "claims":[claim]
    });
    let mut context = selected_context(
        &authored,
        &software_files,
        &configuration,
        &preview_request,
        base,
    );
    context.effective_uid = u64::from(uid);
    context.configuration_raw = owner_raw.clone();
    let archive_manifest = format!(
        "{}/manifest.json",
        history["receipts"][0]["archive_path"].as_str().unwrap()
    );
    let collection_path = format!("{collection}/collection.json");
    let mut broken_authored = authored.clone();
    assert!(
        broken_authored
            .insert(archive_manifest, b"{}\n".to_vec())
            .is_some()
    );
    let broken_store = temporary.path().join("broken-history-store");
    let broken_revision =
        super::validation_cut_cases::write_cut_store(&broken_authored, &broken_store);
    let broken_cut = open_cut(&broken_store, broken_revision, deadline, &cancellation);
    let mut broken_context = selected_context(
        &broken_authored,
        &software_files,
        &configuration,
        &preview_request,
        broken_revision,
    );
    broken_context.effective_uid = u64::from(uid);
    broken_context.configuration_raw = owner_raw.clone();
    let mut broken_budget = ExecutorBudget::laboratory();
    broken_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!broken_budget.execution_wall.is_zero());
    // Broken/current/history cuts share code, never schema results or child state.
    let worker_image =
        super::command_form_cases::schema_image(broken_budget, deadline, &cancellation);
    let mut broken_worker = super::command_form_cases::schemas_for_profile_with_image(
        &broken_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        broken_budget,
        deadline,
        &cancellation,
    );
    let broken_archive = prepare_isolated_claim_creation_from_captures(
        &filesystem,
        &broken_context,
        &broken_cut,
        &software,
        &components,
        &mut broken_worker,
        deadline,
        &cancellation,
    );
    assert!(
        matches!(
            &broken_archive,
            Err(SourceCommandError::SchemaExecution {
                path,
                root,
                reason: tos_validation::item_rules::ItemRefusal::Source(reason),
            }) if path == &collection_path
                && root.as_str() == "selected Collection historical version"
                && reason.as_str() == "native compound: schema_version"
        ),
        "broken Collection archive refusal: {:?}",
        broken_archive.as_ref().err(),
    );
    drop(broken_worker);
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!budget.execution_wall.is_zero());
    let mut worker = super::command_form_cases::schemas_for_profile_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        budget,
        deadline,
        &cancellation,
    );
    let preview = response(
        &prepare_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &software,
            &components,
            &mut worker,
            deadline,
            &cancellation,
        )
        .unwrap(),
    );
    assert_eq!(preview["grants_admission"], false);
    let binding = &preview["source_bindings"]["collection_orders"];
    assert_eq!(binding.as_object().unwrap().len(), 1);
    assert_eq!(
        binding["tos.claim.synthetic-collection-order"]["establishes_membership"],
        false
    );
    assert_eq!(
        binding["tos.claim.synthetic-collection-order"]["collection"]["version_status"],
        "historical"
    );
    let mut request = preview_request;
    request["operation"] = Value::String("claims.create".into());
    request["command_id"] = Value::String("synthetic:collection-order-create".into());
    request["expected_configuration"] = preview["owner_configuration"].clone();
    request["expected_revision"] = Value::Null;
    request["expected_dependencies"] = preview["expected_dependencies"].clone();
    request["expected_inputs"] = preview["source_bindings"].clone();
    context.request_raw = serde_json::to_vec(&request).unwrap();
    let catalog_path = isolated
        .path()
        .join("ToS/source-witnesses/catalog/collections.jsonl");
    let catalog_raw = fs::read(&catalog_path).unwrap();
    fs::write(&catalog_path, b"{}\n").unwrap();
    let corrupted_catalog_result = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &cut,
        &software,
        &components,
        &mut worker,
        None,
        deadline,
        &cancellation,
    );
    assert!(
        matches!(
            &corrupted_catalog_result,
            Err(SourceCommandError::Conflict(_)
                | SourceCommandError::Invalid(_)
                | SourceCommandError::InvalidAt {
                    reason: "missing object field",
                    file: "source_claims.rs",
                    ..
                })
        ),
        "corrupt Collection catalog should fail closed; got error {:?}",
        corrupted_catalog_result.as_ref().err()
    );
    assert!(!isolated.path().join(source_path).parent().unwrap().exists());
    fs::write(&catalog_path, catalog_raw).unwrap();
    drop(worker);
    let mut create_budget = ExecutorBudget::laboratory();
    create_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!create_budget.execution_wall.is_zero());
    let mut worker = super::command_form_cases::schemas_for_profile_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        create_budget,
        deadline,
        &cancellation,
    );
    let (created, publication, _) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &cut,
        &software,
        &components,
        &mut worker,
        None,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(worker);
    assert!(!publication.replayed);
    let original = created.files().clone();
    let mut current_files = authored;
    for (name, raw) in created.files() {
        current_files.insert(format!("{}/{}", created.home().as_str(), name), raw.clone());
    }
    let current = successor(&current_files, &store, base);
    let current_cut = open_cut(&store, current, deadline, &cancellation);
    publish_fixture_catalog(&repository, isolated.path(), deadline);
    let mut original_budget = ExecutorBudget::laboratory();
    original_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!original_budget.execution_wall.is_zero());
    let mut original_worker = super::command_form_cases::schemas_for_profile_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        original_budget,
        deadline,
        &cancellation,
    );
    let mut current_budget = ExecutorBudget::laboratory();
    current_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!current_budget.execution_wall.is_zero());
    let mut current_worker = super::command_form_cases::schemas_for_profile_with_image(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        current_budget,
        deadline,
        &cancellation,
    );
    let (restored, replayed, result) = execute_isolated_claim_creation_from_captures(
        &filesystem,
        &context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut original_worker,
        Some(&mut current_worker),
        deadline,
        &cancellation,
    )
    .unwrap();
    assert!(replayed.replayed);
    assert_eq!(restored.files(), &original);
    assert_eq!(result, restored.command().response);
    assert_eq!(
        response(restored.command())["receipt"]["grants_admission"],
        false
    );
    drop(restored);
    drop(original_worker);
    drop(current_worker);

    // The historical Collection basis remains fixed while the attributed
    // order Claim itself receives one native source-copy correction.
    let form_name = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim["claim_id"].as_str().unwrap().as_bytes()).to_hex()
    );
    let lock_name = format!(".{form_name}.writer.lock");
    let home = created.home().as_str();
    let lock_path = isolated.path().join(home).join(&lock_name);
    fs::write(&lock_path, b"").unwrap();
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();
    let revision_owner = serde_json::json!({
        "schema_version":"tos_local_claim_revision_owner_v4", "uid":uid,
        "principal_id":claim["maker"]["agent_ref"],
        "source_root":isolated.path(), "source_path":source_path,
        "authority_ref":"synthetic-test-only:collection-order-correction-no-membership-grant",
        "expires_at":"2099-01-01T00:00:00Z", "claim_id":claim["claim_id"],
        "allowed_operations":["claim.revise"], "allowed_fields":["qualifiers"],
        "allowed_evidence_refs":claim["evidence_refs"],
        "allowed_object_refs":claim["object"]["members"],
        "allowed_object_values":[claim["object"]],
        "allowed_form_ids":["tos.form.synthetic-collection-order"],
        "allowed_form_field_ids":["claim.statement"]
    });
    let revision_owner_path = isolated.path().join("order-revision-owner.json");
    let revision_owner_raw = source_bytes(&source_value(&revision_owner));
    fs::write(&revision_owner_path, &revision_owner_raw).unwrap();
    fs::set_permissions(&revision_owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let revision_filesystem = CreationFilesystem::select_isolated(
        &isolated,
        &revision_owner_path,
        deadline,
        &cancellation,
    )
    .unwrap();
    let correction = serde_json::json!({
        "schema_version":"tos_local_source_command_v1", "operation":"prepare-revise",
        "fields":{"qualifiers":{"statement":format!(
            "{} [retained order wording correction; no membership grant]",
            claim["qualifiers"]["statement"].as_str().unwrap()
        )}},
        "forms":[{"form_id":"tos.form.synthetic-collection-order","field_id":"claim.statement"}],
        "reason":"Retain an isolated attributed order wording correction."
    });
    let mut revision_context = selected_context(
        &current_files,
        &software_files,
        &revision_owner,
        &correction,
        current,
    );
    revision_context.effective_uid = u64::from(uid);
    revision_context.configuration_raw = revision_owner_raw.clone();
    let mut preview_budget = ExecutorBudget::laboratory();
    preview_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!preview_budget.execution_wall.is_zero());
    let mut revision_preview_worker = super::command_form_cases::schemas_for_profile_with_image(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        preview_budget,
        deadline,
        &cancellation,
    );
    let revision_preview = prepare_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_preview_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_preview_worker);
    let prepared = response(&revision_preview);
    assert_eq!(
        prepared["source_bindings"]["collection_orders"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    let independent = maintained_oracle("claim-revision-catalog-history");
    // Current expected dependencies come from the native owner/source cut;
    // `independent` retains the frozen historical Python packet.
    assert_eq!(prepared["source_bindings"], independent["source_bindings"]);
    let mut revision_request = correction;
    revision_request["operation"] = Value::String("claim.revise".into());
    revision_request["command_id"] = Value::String("synthetic:collection-order-correction".into());
    revision_request["expected_configuration"] = prepared["owner_configuration"].clone();
    revision_request["expected_source"] = prepared["source"].clone();
    revision_request["expected_revision"] = prepared["revision"].clone();
    revision_request["expected_dependencies"] = prepared["expected_dependencies"].clone();
    revision_request["expected_inputs"] = prepared["source_bindings"].clone();
    revision_context.request_raw = serde_json::to_vec(&revision_request).unwrap();
    let mut revision_budget = ExecutorBudget::laboratory();
    revision_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!revision_budget.execution_wall.is_zero());
    let mut revision_worker = super::command_form_cases::schemas_for_profile_with_image(
        &current_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        revision_budget,
        deadline,
        &cancellation,
    );
    let (revised, revision_publication) = execute_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &revision_context,
        &cut,
        &current_cut,
        &software,
        &components,
        &mut revision_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    drop(revision_worker);
    assert!(!revision_publication.replayed);
    assert_eq!(
        revised.commit(),
        Err(SourceCommandError::MissingProductionAdmission)
    );
    let revised_result = response(&revised);
    assert_eq!(revised_result["receipt"]["grants_admission"], false);
    let archive = revised_result["receipt"]["archive_path"].as_str().unwrap();
    assert!(
        isolated
            .path()
            .join(archive)
            .join("manifest.json")
            .is_file()
    );
    let mut corrected_files = current_files;
    apply_changes(&mut corrected_files, &revised);
    for change in &revised.changes {
        if let Some(raw) = &change.after {
            assert_eq!(
                fs::read(isolated.path().join(change.path.as_str())).unwrap(),
                *raw
            );
        }
    }
    let corrected = successor(&corrected_files, &store, current);
    let corrected_cut = open_cut(&store, corrected, deadline, &cancellation);
    publish_fixture_catalog(&repository, isolated.path(), deadline);
    let mut original_budget = ExecutorBudget::laboratory();
    original_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!original_budget.execution_wall.is_zero());
    let mut original_worker = super::command_form_cases::schemas_for_profile_with_image(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        original_budget,
        deadline,
        &cancellation,
    );
    let mut latest_budget = ExecutorBudget::laboratory();
    latest_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!latest_budget.execution_wall.is_zero());
    let mut latest_worker = super::command_form_cases::schemas_for_profile_with_image(
        &corrected_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        latest_budget,
        deadline,
        &cancellation,
    );
    let (restored_after_revision, original_replayed, original_result) =
        execute_isolated_claim_creation_from_captures(
            &filesystem,
            &context,
            &cut,
            &corrected_cut,
            &software,
            &components,
            &mut original_worker,
            Some(&mut latest_worker),
            deadline,
            &cancellation,
        )
        .unwrap();
    assert!(original_replayed.replayed);
    assert_eq!(restored_after_revision.files(), &original);
    assert_eq!(original_result, restored_after_revision.command().response);
    assert!(fs::read(&lock_path).unwrap().is_empty());
    drop(original_worker);
    drop(latest_worker);
    let mut retry_context = selected_context(
        &corrected_files,
        &software_files,
        &revision_owner,
        &revision_request,
        corrected,
    );
    retry_context.effective_uid = u64::from(uid);
    retry_context.configuration_raw = revision_owner_raw.clone();
    let mut retry_budget = ExecutorBudget::laboratory();
    retry_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    assert!(!retry_budget.execution_wall.is_zero());
    let mut retry_worker = super::command_form_cases::schemas_for_profile_with_image(
        &corrected_cut,
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        retry_budget,
        deadline,
        &cancellation,
    );
    let (retried, retry_publication) = execute_isolated_claim_revision_from_captures(
        &revision_filesystem,
        &retry_context,
        &cut,
        &corrected_cut,
        &software,
        &components,
        &mut retry_worker,
        deadline,
        &cancellation,
    )
    .unwrap();
    assert!(retry_publication.replayed);
    assert!(retried.changes.is_empty());
    assert_eq!(response(&retried)["receipt"], revised_result["receipt"]);
}

#[test]
fn native_claim_cli_creates_revises_and_cold_replays_original_bytes() {
    use super::command_text_cases::{
        alignment_image_digest, alignment_native_cli, authored_text_files,
    };
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation_store::IsolatedCreationRoot;
    use tos_foundation::query_context::{
        ContextResolution, Cutoff, MismatchDimension, QueryContext, SourceSnapshot,
        SourceSnapshotRef, SystemState, create_chronology_version, resolve_query_context,
    };
    use tos_foundation::temporal::{
        CarrierAuthority, CarrierValidity, LegacyObjectAssessment, Relation, TemporalBudget,
        TemporalRole, decode_claim_carrier, encode_claim_carrier,
    };
    use tos_foundation::{ExactRecordRef, RecordVersion, StableId};
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    for date_variant in [false, true] {
        let cancellation = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(240);
        let (mut files, mut claim, mut revision_owner, mut source_path) = claim_fixture();
        claim["claim_version"] = Value::from(1);
        let date_object = |year: &str| {
            serde_json::json!({"kind":"date-assertion","role":"historical-time",
        "value":year,"calendar":"proleptic-gregorian","year_numbering":"astronomical","certainty":"exact",
        "source_wording":{"language":"en","text":year}})
        };
        if date_variant {
            source_path =
                "ToS/source-witnesses/relations/synthetic-date-history/source-claims.jsonl".into();
            let subject = "tos.historical-event.synthetic.date-history";
            let evidence = claim["evidence_refs"].clone();
            let maker = claim["maker"].clone();
            claim = serde_json::json!({"schema_version":"tos_source_temporal_claim_v1",
            "claim_id":"tos.claim.synthetic.date-history","claim_type":"relation","claim_version":1,
            "assertion_layer":"scholarly_report","subject_ref":subject,"predicate":"historical_dating",
            "object":date_object("1945"),"evidence_refs":evidence,"maker":maker,
            "provenance_event_ref":"tos.event.source-creation.synthetic-date-history",
            "epistemic_status":"reported","review_status":"unreviewed","visibility":"public_metadata_only",
            "qualifiers":{"statement":"Explicit synthetic historical dating scenario; preserve the exact predecessor when corrected.",
                "statement_language":"en","statement_script":"Latn"}});
            let event = serde_json::json!({"schema_version":"tos_historical_record_v1","record_type":"historical-event",
            "record_id":subject,"record_version":1,"preferred_label":"Synthetic date history scenario",
            "identity_status":"provisional","source_refs":evidence,"external_identifiers":[],
            "same_as_posture":"no_equivalence_claim","visibility":"public_metadata_only"});
            files.insert(
                "ToS/source-witnesses/history/synthetic-date-history/historical-event.json".into(),
                source_bytes(&source_value(&event)),
            );
            revision_owner["schema_version"] =
                serde_json::json!("tos_local_claim_revision_owner_v2");
            revision_owner["source_path"] = serde_json::json!(source_path);
            revision_owner["claim_id"] = claim["claim_id"].clone();
            revision_owner["allowed_fields"] = serde_json::json!(["object"]);
            revision_owner["allowed_object_values"] =
                serde_json::json!([date_object("1945"), date_object("1946")]);
            revision_owner["allowed_object_refs"] = serde_json::json!([]);
            // This ID is a fixture request/grant, not an already-created form.
            revision_owner["allowed_form_ids"] =
                serde_json::json!(["tos.form.synthetic-date-history.statement"]);
            revision_owner["allowed_form_field_ids"] = serde_json::json!(["claim.statement"]);
        }

        let raw_lexical = "synthetic raw evidence: 0.100000000000000005 / -0 / λ";
        let opaque_sibling = serde_json::json!({"future":null,"large_integer":9007199254740993u64,
        "raw_decimal":raw_lexical,"scope":"synthetic fixture assertion; not source authority"});
        if !date_variant {
            // Explicit NEW precursor content, authored before v1 exists. The empty
            // recognized collection admits the carrier without inventing a relation.
            claim["qualifiers"]["statement"] = serde_json::json!(format!(
                "{} [synthetic preservation seed; no assessment]",
                claim["qualifiers"]["statement"].as_str().unwrap()
            ));
            claim["qualifiers"]["carrier_fixture_sibling"] = opaque_sibling.clone();
            claim["qualifiers"]["temporal_claim_carrier"] = serde_json::json!({
                "schema_version":"tos_temporal_claim_carrier_v1",
                "relative_relations":[],"future_packet":opaque_sibling});
        }

        let mut software_names = CLAIM_GROUNDING_RULE_INPUTS
            .iter()
            .chain(CLAIM_REVISION_RULE_INPUTS)
            .copied()
            .filter(|name| !name.starts_with("ToS/"))
            .collect::<Vec<_>>();
        software_names.extend([
            "rust/crates/tos-command/src/source_claims.rs",
            "rust/crates/tos-command/src/source_serialization.rs",
        ]);
        software_names.sort_unstable();
        software_names.dedup();
        for name in software_names {
            files.insert(name.into(), fs::read(repository.join(name)).unwrap());
        }
        // Cheap physical input bounds precede capture/corpus/publication writes.
        let fixture_bytes = files
            .values()
            .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
            .unwrap();
        assert!(fixture_bytes <= 33_554_432 && files.len() <= 2048);
        let native = PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH")
                .expect("OPS must select the protected native Claim image"),
        );
        assert!(native.is_absolute());
        let worker_image = super::validation_cut_cases::selected_worker_path();
        let native_bytes = fs::metadata(&native).unwrap().len();
        let worker_bytes = fs::metadata(&worker_image).unwrap().len();
        let consumer_bytes = fs::metadata(std::env::current_exe().unwrap())
            .unwrap()
            .len();
        assert!(
            native_bytes <= 536_870_912
                && consumer_bytes <= 536_870_912
                && worker_bytes <= 536_870_912
        );
        assert!(Instant::now() < deadline);
        eprintln!(
            "Claim CLI date_variant={} F_fixture={} E_native={} C_consumer={} W_worker={} processes<={} workers<={}",
            date_variant,
            fixture_bytes,
            native_bytes,
            consumer_bytes,
            worker_bytes,
            if date_variant { 9 } else { 12 },
            if date_variant { 12 } else { 20 }
        );
        let (_capture, software, components) =
            super::command_record_cases::captured_components(&files, deadline, &cancellation);
        let temporary = tempfile::tempdir().unwrap();
        let authored = files
            .iter()
            .filter(|(name, _)| name.starts_with("ToS/"))
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        let store = temporary.path().join("selected-store");
        let base = super::validation_cut_cases::write_cut_store(&authored, &store);
        let cut = open_cut(&store, base, deadline, &cancellation);
        drop(cut);
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
        for (name, raw) in &files {
            let target = isolated.path().join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, raw).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        }
        fs::create_dir_all(
            isolated
                .path()
                .join(&source_path)
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .unwrap();
        let uid = fs::metadata(isolated.path()).unwrap().uid();
        let mut configuration = serde_json::json!({
            "schema_version":"tos_local_claim_create_owner_v1", "uid":uid,
            "principal_id":claim["maker"]["agent_ref"], "maker_type":claim["maker"]["maker_type"],
            "source_root":isolated.path(), "source_path":source_path,
            "authority_ref":"synthetic-test-only:claim-creation-not-assessment",
            "expires_at":"2099-01-01T00:00:00Z", "provenance_event_id":claim["provenance_event_ref"],
            "allowed_operations":["claims.create"], "allowed_claim_ids":[claim["claim_id"]],
            "allowed_subject_refs":[claim["subject_ref"]], "allowed_object_refs":[claim["object"]],
            "allowed_predicates":[claim["predicate"]], "allowed_evidence_refs":claim["evidence_refs"]
        });
        if date_variant {
            configuration["schema_version"] = serde_json::json!("tos_local_claim_create_owner_v2");
            configuration["allowed_object_refs"] = serde_json::json!([]);
            configuration["allowed_object_values"] = serde_json::json!([date_object("1945")]);
        }
        let config_raw = source_bytes(&source_value(&configuration));
        let owner = isolated.path().join("owner.json");
        fs::write(&owner, &config_raw).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();

        let invocation_path = temporary.path().join("native-claim-invocation.json");
        let mut invocation = serde_json::json!({
            "schema_version":"tos_local_native_claim_invocation_v1", "owner_config":owner,
            "native_executable":native, "native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
            "corpus_store":store,"source_revision":base.0.to_prefixed(),"original_source_revision":base.0.to_prefixed(),
            "software_capture":_capture.capture,"software_restored_root":_capture.restored,
            "software_selection":{"source_git_commit":_capture.selection.source_git_commit,
                "source_git_tree":_capture.selection.source_git_tree,
                "capture_manifest_sha256":_capture.selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|member| member.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":worker_image,"sha256":alignment_image_digest(&worker_image).to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
                "max_member_bytes":8388608,"max_schema_receipts":128,"max_schema_receipt_bytes":262144,
                "worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}
        });
        let write_invocation = |value: &Value| {
            fs::write(&invocation_path, serde_json::to_vec(value).unwrap()).unwrap();
            fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
        };
        write_invocation(&invocation);
        let preview_request = serde_json::json!({"schema_version":"tos_local_source_command_v1",
        "operation":"prepare-create","claims":[claim.clone()]});
        let preview = alignment_native_cli(
            &repository,
            &owner,
            &invocation_path,
            &preview_request,
            deadline,
        );
        assert_eq!(preview["grants_admission"], false);
        let prepared = &preview["result"];
        let mut request = preview_request;
        request["operation"] = serde_json::json!("claims.create");
        request["command_id"] = serde_json::json!("synthetic:whole-native-claim-cli");
        request["expected_configuration"] = prepared["owner_configuration"].clone();
        request["expected_revision"] = Value::Null;
        request["expected_dependencies"] = prepared["expected_dependencies"].clone();
        request["expected_inputs"] = prepared["source_bindings"].clone();
        let created =
            alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
        assert_eq!(created["result"]["replayed"], false);
        assert_eq!(created["result"]["receipt"]["grants_admission"], false);
        let home = Path::new(&source_path).parent().unwrap();
        let original_files = fs::read_dir(isolated.path().join(home))
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().into_string().unwrap(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(original_files.len(), 5);
        if date_variant {
            // Public initial creation owns exactly the stream, request,
            // environment, provenance and receipt; forms first materialize
            // through the existing explicitly granted revision selector.
            assert!(
                original_files
                    .keys()
                    .all(|name| !name.ends_with(".human-forms.json"))
            );
            let receipt: Value =
                serde_json::from_slice(&original_files["source-create-receipt.json"]).unwrap();
            assert_eq!(receipt, created["result"]["receipt"]);
            let refs = receipt["files"].as_object().unwrap();
            assert_eq!(refs.len(), 4); // receipt cannot include its own digest
            for (name, binding) in refs {
                let raw = &original_files[name];
                assert_eq!(binding["sha256"], Digest256::of_bytes(raw).to_prefixed());
                assert_eq!(binding["bytes"].as_u64(), Some(raw.len() as u64));
            }
        }

        let mut current_files = authored_text_files(isolated.path());
        current_files.remove("ToS/source-witnesses/.historical-create.writer.lock");
        let current = successor(&current_files, &store, base);
        invocation["source_revision"] = serde_json::json!(current.0.to_prefixed());
        write_invocation(&invocation);
        let cold = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
        assert_eq!(cold["result"]["replayed"], true);
        for (name, raw) in &original_files {
            assert_eq!(
                &fs::read(isolated.path().join(home).join(name)).unwrap(),
                raw
            );
        }
        // Freeze actual published v1 metadata and its real post-creation source cut
        // before publishing v2. No historical Claim/ref is invented by this probe.
        let v1_raw = original_files["source-claims.jsonl"].clone();
        let v1_record: Value = serde_json::from_slice(&v1_raw).unwrap();
        let v1_owner_ref = created["result"]["receipt"]["claims"][0].clone();
        assert_eq!(source_ref(&v1_record), v1_owner_ref);
        let exact_ref = |reference: &Value| ExactRecordRef {
            id: StableId::parse(reference["id"].as_str().unwrap()).unwrap(),
            version: RecordVersion::new(reference["version"].as_u64().unwrap()).unwrap(),
            digest: Digest256::from_prefixed(reference["digest"].as_str().unwrap()).unwrap(),
        };
        let snapshot = SourceSnapshot {
            reference: SourceSnapshotRef {
                id: StableId::parse(&format!(
                    "fixture:claim-cut:{}",
                    v1_owner_ref["id"].as_str().unwrap()
                ))
                .unwrap(),
                revision: current,
            },
            current_cutoff: Cutoff {
                value: source_value(&created["result"]["receipt"]["recorded_at"]),
                trs: None,
            },
        };
        // Explicit detached fixture descriptors bind real member/cut metadata. They
        // are not persisted chronology records or admitted source policy objects.
        let fixture_policy = source_ref(&serde_json::json!({
            "schema_version":"fixture_detached_chronology_policy_v1",
            "record_id":"fixture:claim-chronology-policy", "record_version":1,
            "scope":"Synthetic immutable selection demonstration; no admission",
            "member":v1_owner_ref, "source_revision":current.0.to_prefixed()
        }));
        let fixture_chronology = source_ref(&serde_json::json!({
            "schema_version":"fixture_detached_chronology_v1",
            "record_id":"fixture:claim-chronology", "record_version":1,
            "member":v1_owner_ref, "source_revision":current.0.to_prefixed(),
            "policy":fixture_policy
        }));
        let current_context = QueryContext {
            system_state: Some(SystemState::Current),
            ..QueryContext::default()
        };
        let frozen = create_chronology_version(
            exact_ref(&fixture_chronology),
            &snapshot,
            &current_context,
            exact_ref(&fixture_policy),
            exact_ref(&fixture_policy),
            vec![exact_ref(&v1_owner_ref)],
        )
        .unwrap()
        .unwrap();
        assert_eq!(frozen.selection.source_snapshot.revision, current);
        assert_eq!(
            frozen.member_claim_revisions,
            vec![exact_ref(&v1_owner_ref)]
        );

        // Supplemental assertions use real previous refs/bytes/receipts; their
        // temporal scenario is explicitly synthetic and never replaces the object.
        let codec_budget = TemporalBudget::default();
        let codec_reference = |reference: &Value| {
            serde_json::json!({
                "id":reference["id"],"version":reference["version"],
                "digest":Digest256::from_prefixed(reference["digest"].as_str().unwrap()).unwrap().to_hex()
            })
        };
        let actual_basis = |reference: &Value, raw: &[u8], receipt: &Value, cut: SourceRevision| {
            let fragment = std::str::from_utf8(raw)
                .unwrap()
                .chars()
                .take(128)
                .collect::<String>();
            serde_json::json!({"source":codec_reference(reference),
            "source_locator":format!("{}:line1;unicode-range=0..{}", source_path, fragment.chars().count()),
            "raw_evidence":{"actual_source_fragment":fragment,
                "actual_stream_sha256":Digest256::of_bytes(raw).to_prefixed(),
                "actual_receipt_command":receipt["command_id"],"actual_source_cut":cut.0.to_prefixed(),
                "synthetic_assertion_raw":raw_lexical},"future_basis":null})
        };
        let unknown_packet = serde_json::json!({"schema_version":"tos_temporal_claim_carrier_v1",
        "validity":{"type":"Unknown","basis":actual_basis(&v1_owner_ref,&v1_raw,&created["result"]["receipt"],current)},
        "future_packet":opaque_sibling});
        let unknown = decode_claim_carrier(
            &source_value(&serde_json::json!({
        "temporal_claim_carrier":unknown_packet})),
            codec_budget,
        )
        .unwrap()
        .unwrap();
        let encoded = encode_claim_carrier(
            &source_value(&v1_record["qualifiers"]),
            &unknown,
            codec_budget,
        )
        .unwrap();
        let mut corrected_qualifiers: Value =
            serde_json::from_slice(&source_bytes(&encoded)).unwrap();
        corrected_qualifiers["statement"] = serde_json::json!(format!(
            "{} [native CLI retained correction; no assessment]",
            claim["qualifiers"]["statement"].as_str().unwrap()
        ));
        corrected_qualifiers["carrier_fixture_sibling"] = opaque_sibling.clone();
        // The owner publishes SourceCommandInputV1 canonical JSON. Object
        // member order is not a cross-publication carrier equality contract.
        let corrected_unknown =
            decode_claim_carrier(&source_value(&corrected_qualifiers), codec_budget)
                .unwrap()
                .unwrap();
        assert_eq!(
            source_bytes(corrected_unknown.original_packet()),
            source_bytes(unknown.original_packet())
        );
        assert!(matches!(
            corrected_unknown.validity(),
            Some(CarrierValidity::Unknown { .. })
        ));
        revision_owner["uid"] = serde_json::json!(uid);
        revision_owner["source_root"] = serde_json::json!(isolated.path());
        let revision_path = isolated.path().join("revision-owner.json");
        fs::write(&revision_path, source_bytes(&source_value(&revision_owner))).unwrap();
        fs::set_permissions(&revision_path, fs::Permissions::from_mode(0o600)).unwrap();
        invocation["owner_config"] = serde_json::json!(revision_path);
        write_invocation(&invocation);
        let unknown_commit = if !date_variant {
            // Generic bridge PREPARE/TEMPLATE only: real published v1 custody,
            // explicit synthetic alias; no new CLI call, revision or publication.
            use std::io::Read;
            use tos_command::source_creation_store::CreationFilesystem;
            use tos_command::temporal_claim_intent::{
                ClaimIntentError, ClaimIntentPrepareBudget, plan_claim_intent,
                prepare_claim_intent_from_captures,
            };
            use tos_foundation::emit_value_preserved_json;
            use tos_validation::executor::ExactWorkerIdentity;
            use tos_validation::source_cut::{
                CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor,
            };
            let mut oracle_raw = Vec::new();
            fs::File::open(repository.join(
                "tests/conformance/foundation-wave-v1/oracles/temporal-acceptance-vectors.json",
            ))
            .unwrap()
            .take(262_145)
            .read_to_end(&mut oracle_raw)
            .unwrap();
            assert!(oracle_raw.len() <= 262_144);
            assert_eq!(
                Digest256::of_bytes(&oracle_raw).to_hex(),
                "fda08fd958914a3605afab12e7b78012a3f5534ecfdffe2ab31cb2cb3a26eecd"
            );
            let oracle: Value = serde_json::from_slice(&oracle_raw).unwrap();
            let vector = |id: &str| {
                oracle["vectors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["id"].as_str() == Some(id))
                    .unwrap()
            };
            let intent_for = |id: &str| {
                let row = vector(id);
                serde_json::json!({"schema_version":"tos_temporal_claim_intent_v1",
                    "operation":row["operation"],"source_profile":row["source_profile"],
                    "input":row["input"],"context":row["context"]})
            };
            let intent = intent_for("recurrence_preserve");
            let basis = actual_basis(
                &v1_owner_ref,
                &v1_raw,
                &created["result"]["receipt"],
                current,
            );
            // Strict new basis has exactly these three fields. The helper's
            // optional future_basis is neither evidence nor a verification flag.
            let basis = serde_json::json!({"source":basis["source"],
                "source_locator":basis["source_locator"],"raw_evidence":basis["raw_evidence"]});
            let configuration_raw = fs::read(&revision_path).unwrap();
            let configuration_sha =
                Digest256::of_bytes(&source_bytes(&source_value(&revision_owner)));
            let bindings = serde_json::json!({"schema_version":"tos_temporal_claim_intent_binding_v1",
                "target":{"alias":"recurrence-A","revision_alias":"r1","next_revision_alias":null,
                    "source":v1_owner_ref,"source_path":source_path,
                    "source_revision":current.0.to_prefixed(),"configuration_digest":configuration_sha.to_prefixed()},
                "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
                "authored_reason":"Explicit synthetic generic prepare/template only; never publish.",
                "mapping_profile":"fixture:opaque-recurrence-preservation-v1",
                "payload":{"schema_version":"tos_temporal_claim_carrier_v1",
                    "recurrence":{"profile_ref":"fixture:opaque-pattern-v1",
                        "pattern":intent["input"]["recurrence_pattern"],"basis":basis}},
                "identities":{},"sources":{},"trs":{}});
            let budget = ClaimIntentPrepareBudget {
                max_extra_owned_bytes: 134_217_728,
                max_extra_json_visits: 1_000_000,
                max_preview_bytes: 4_194_304,
                max_output_bytes: 1_048_576,
            };
            let target = source_value(&v1_record);
            let intent_value = source_value(&intent);
            let binding_value = source_value(&bindings);
            let plan =
                plan_claim_intent(&intent_value, &binding_value, &target, codec_budget, budget)
                    .unwrap();
            assert!(!plan.owner_checked());
            assert_eq!(plan.basis_status(), "SourceSuppliedUnverified");
            // Pure negative rows do not call the owner or spawn another worker.
            let mut mismatched = bindings.clone();
            mismatched["payload"]["recurrence"]["pattern"]["rule"] =
                serde_json::json!("different rule");
            assert!(matches!(
                plan_claim_intent(
                    &intent_value,
                    &source_value(&mismatched),
                    &target,
                    codec_budget,
                    budget
                ),
                Err(ClaimIntentError::Mismatch(_))
            ));
            let unknown_source = intent_for("unknown_validity_end");
            let unknown_intent = source_value(&unknown_source);
            let mut unknown_binding = bindings.clone();
            unknown_binding["target"]["alias"] = serde_json::json!("state-B");
            unknown_binding["authored_reason"] = serde_json::json!(
                "Explicit NEW synthetic Unknown-END preservation through the existing owner; no calendar, validity reconciliation or admission."
            );
            unknown_binding["mapping_profile"] =
                serde_json::json!("fixture:explicit-unknown-end-owner-commit-v1");
            unknown_binding["payload"] = serde_json::json!({"schema_version":"tos_temporal_claim_carrier_v1",
                "validity":{"type":"Interval","beginning":{"type":"CalendarPosition",
                    "lexical":unknown_source["input"]["validity_assertion"]["valid_from"],
                    "resolution":"Unknown","trs":null},"end":{"type":"Unknown"},
                    "basis":bindings["payload"]["recurrence"]["basis"]}});
            let mut wrong_lexical = unknown_binding.clone();
            wrong_lexical["payload"]["validity"]["beginning"]["lexical"] =
                serde_json::json!("2020-01-02");
            assert!(matches!(
                plan_claim_intent(
                    &unknown_intent,
                    &source_value(&wrong_lexical),
                    &target,
                    codec_budget,
                    budget
                ),
                Err(ClaimIntentError::Mismatch(_))
            ));
            let mut missing_basis = unknown_binding.clone();
            missing_basis["payload"]["validity"]
                .as_object_mut()
                .unwrap()
                .remove("basis");
            assert!(matches!(
                plan_claim_intent(
                    &unknown_intent,
                    &source_value(&missing_basis),
                    &target,
                    codec_budget,
                    budget
                ),
                Err(ClaimIntentError::NeedsBinding("basis"))
            ));
            let mut unsupported_end = unknown_source.clone();
            unsupported_end["input"]["validity_assertion"]["valid_to"]["type"] =
                serde_json::json!("Unbounded");
            assert!(matches!(
                plan_claim_intent(
                    &source_value(&unsupported_end),
                    &source_value(&unknown_binding),
                    &target,
                    codec_budget,
                    budget
                ),
                Err(ClaimIntentError::UnsupportedSemanticMapping(_))
            ));
            // Admission only: zero-owned does NOT exercise parser exhaustion.
            assert!(matches!(
                plan_claim_intent(
                    &intent_value,
                    &binding_value,
                    &target,
                    codec_budget,
                    ClaimIntentPrepareBudget {
                        max_extra_owned_bytes: 0,
                        ..budget
                    }
                ),
                Err(ClaimIntentError::Resource)
            ));
            let original_cut = open_cut(&store, base, deadline, &cancellation);
            let current_cut = open_cut(&store, current, deadline, &cancellation);
            assert_eq!(current_cut.current().revision(), current);
            let filesystem = CreationFilesystem::select_isolated(
                &isolated,
                &revision_path,
                deadline,
                &cancellation,
            )
            .unwrap();
            let sentinel = source_bytes(&source_value(&serde_json::json!({
                "schema_version":"tos_local_source_command_v1","operation":"describe"})));
            // Explicit physical config/raw packet, real UID and actual observed
            // receipt timestamp; no helper's synthetic clock/config serialization.
            let selected_inputs = || {
                current_files
                    .iter()
                    .chain(files.iter().filter(|(name, _)| !name.starts_with("ToS/")))
            };
            let preclone_bytes = selected_inputs()
                .try_fold(0usize, |n, (_, raw)| n.checked_add(raw.len()))
                .unwrap();
            assert!(preclone_bytes <= 33_554_432 && selected_inputs().count() <= 2048);
            let mut direct_context = tos_command::source_command::CommandContext {
                base_revision: current,
                configuration_raw: configuration_raw.clone(),
                request_raw: sentinel.clone(),
                recorded_at: created["result"]["receipt"]["recorded_at"]
                    .as_str()
                    .unwrap()
                    .into(),
                effective_uid: u64::from(uid),
                files: current_files
                    .iter()
                    .chain(files.iter().filter(|(name, _)| !name.starts_with("ToS/")))
                    .map(|(name, raw)| tos_command::source_command::SourceFile {
                        path: tos_foundation::RelativePath::parse(name).unwrap(),
                        raw: raw.clone(),
                    })
                    .collect(),
            };
            let context_bytes = direct_context
                .files
                .iter()
                .try_fold(0usize, |n, f| n.checked_add(f.raw.len()))
                .unwrap();
            assert!(context_bytes <= 33_554_432 && direct_context.files.len() <= 2048);
            let control_path = isolated
                .path()
                .join("ToS/source-witnesses/.metadata-publication.json");
            let read_control = || match fs::File::open(&control_path) {
                Ok(file) => {
                    let mut raw = Vec::new();
                    file.take(8193).read_to_end(&mut raw).unwrap();
                    assert!(raw.len() <= 8192);
                    Some(raw)
                }
                Err(e) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
                    None
                }
            };
            let control_before = read_control();
            let mut worker_budget = ExecutorBudget::laboratory();
            worker_budget.execution_wall = deadline
                .saturating_duration_since(Instant::now())
                .min(worker_budget.execution_wall);
            assert!(!worker_budget.execution_wall.is_zero());
            let mut direct_worker = CutWorkerSchemaExecutor::from_cut(
                &current_cut,
                FormatProfile::LegacyPythonObserved20260923,
                ExactWorkerIdentity {
                    absolute_path: worker_image.clone(),
                    sha256: Digest256::from_prefixed(
                        invocation["schema_worker"]["sha256"].as_str().unwrap(),
                    )
                    .unwrap(),
                },
                worker_budget,
                CutWorkerLimits {
                    max_receipts: 128,
                    max_receipt_bytes: 262_144,
                },
                deadline,
                &cancellation,
            )
            .unwrap();
            let mut generic = prepare_claim_intent_from_captures(
                &filesystem,
                &mut direct_context,
                &original_cut,
                &current_cut,
                &software,
                &components,
                &mut direct_worker,
                plan,
                deadline,
                &cancellation,
            )
            .unwrap();
            assert_eq!(direct_context.request_raw, sentinel);
            assert_eq!(direct_context.configuration_raw, configuration_raw);
            assert!(generic.owner_checked());
            assert_eq!(generic.basis_status(), "SourceSuppliedUnverified");
            let prepared_packet = generic.prepared_packet().unwrap();
            let prepared_packet: Value =
                serde_json::from_slice(&source_bytes(&prepared_packet)).unwrap();
            assert_eq!(prepared_packet["status"], "PreparedNotCommitted");
            assert_eq!(
                prepared_packet["schema_version"],
                "tos_temporal_claim_intent_prepared_v1"
            );
            assert_eq!(prepared_packet["owner_checked"], true);
            assert_eq!(prepared_packet["basis_status"], "SourceSuppliedUnverified");
            assert_eq!(
                prepared_packet["profile_status"],
                "SourceSuppliedAttributionOnly"
            );
            assert_eq!(prepared_packet["grants_admission"], false);
            // 257 is over the actual native256-character limit. Never execute
            // either template: all subsequent native CLI calls remain original.
            assert!(matches!(
                generic.commit_request_template(&"x".repeat(257)),
                Err(ClaimIntentError::Mismatch(_))
            ));
            let template = generic
                .commit_request_template("synthetic:generic-recurrence-template-only")
                .unwrap();
            let template: Value = serde_json::from_slice(&source_bytes(&template)).unwrap();
            assert_eq!(
                template["schema_version"],
                "tos_temporal_claim_intent_commit_template_v1"
            );
            assert_eq!(template["status"], "CommitTemplateNotCommitted");
            for key in [
                "owner_checked",
                "basis_status",
                "profile_status",
                "grants_admission",
            ] {
                assert_eq!(template[key], prepared_packet[key]);
            }
            let request = &template["request"];
            assert_eq!(request["schema_version"], "tos_local_source_command_v1");
            assert_eq!(request["operation"], "claim.revise");
            assert!(request["command_id"].as_str().unwrap().chars().count() <= 256);
            for (dst, src) in [
                ("expected_configuration", "owner_configuration"),
                ("expected_source", "source"),
                ("expected_revision", "revision"),
                ("expected_dependencies", "expected_dependencies"),
                ("expected_inputs", "source_bindings"),
            ] {
                assert_eq!(
                    source_bytes(&source_value(&request[dst])),
                    source_bytes(&source_value(&prepared_packet["owner_preview"][src]))
                );
            }
            assert_eq!(
                source_bytes(&source_value(&request["expected_source"])),
                source_bytes(&source_value(&v1_owner_ref))
            );
            assert_eq!(
                request["fields"]["qualifiers"]["temporal_claim_carrier"]["recurrence"]["pattern"],
                intent["input"]["recurrence_pattern"]
            );
            assert_eq!(
                request["fields"]["qualifiers"].as_object().unwrap().len(),
                1
            );
            assert!(request["fields"]["qualifiers"].get("statement").is_none());
            assert!(
                request["fields"]["qualifiers"]
                    .get("carrier_fixture_sibling")
                    .is_none()
            );
            for key in ["relative_relations", "future_packet"] {
                assert_eq!(
                    request["fields"]["qualifiers"]["temporal_claim_carrier"][key],
                    v1_record["qualifiers"]["temporal_claim_carrier"][key]
                );
            }
            assert_eq!(request["forms"], bindings["forms"]);
            let reason = request["reason"].as_str().unwrap();
            assert!(reason.chars().count() <= 4096);
            let reason: Value = serde_json::from_str(reason).unwrap();
            assert_eq!(reason["preserved_intent"], intent);
            let preserved =
                emit_value_preserved_json(&intent_value, JsonLimits::default()).unwrap();
            assert_eq!(
                template["intent_preserved_text_sha256"],
                Digest256::of_bytes(&preserved).to_prefixed()
            );
            assert_eq!(
                template["intent_canonical_sha256"],
                Digest256::of_bytes(&source_bytes(&intent_value)).to_prefixed()
            );
            assert_eq!(
                template["emitted_owner_request_sha256"],
                Digest256::of_bytes(&source_bytes(&source_value(request))).to_prefixed()
            );
            assert_eq!(
                template["configuration_raw_sha256"],
                Digest256::of_bytes(&configuration_raw).to_prefixed()
            );
            assert_eq!(
                template["configuration_canonical_sha256"],
                configuration_sha.to_prefixed()
            );
            direct_worker.finish(deadline, &cancellation).unwrap();
            assert_eq!(read_control(), control_before);
            for (name, raw) in &original_files {
                assert_eq!(
                    &fs::read(isolated.path().join(home).join(name)).unwrap(),
                    raw
                );
            }
            let mut unchanged = authored_text_files(isolated.path());
            unchanged.remove("ToS/source-witnesses/.historical-create.writer.lock");
            assert_eq!(unchanged, current_files);
            assert_eq!(direct_context.request_raw, sentinel);
            drop(template);
            drop(prepared_packet);
            drop(generic);
            drop(direct_worker);
            drop(direct_context);
            drop(filesystem);
            drop(current_cut);
            drop(original_cut);
            // Construct the existing pure Unknown-END plan only after the
            // recurrence owner/context lifetime ends, then consume it below.
            let unknown_plan = plan_claim_intent(
                &unknown_intent,
                &source_value(&unknown_binding),
                &target,
                codec_budget,
                budget,
            )
            .unwrap();
            assert!(!unknown_plan.owner_checked());
            assert_eq!(unknown_plan.basis_status(), "SourceSuppliedUnverified");
            Some((unknown_plan, unknown_source, unknown_binding))
        } else {
            None
        };
        let (owner_preview, revision, history_template) = if date_variant {
            // Consume the generic history TEMPLATE through the SAME native
            // writer below. Explicit synthetic object supplement, not inferred
            // Gregorian/year numbering/wording for the original year string.
            use std::io::Read;
            use tos_command::source_creation_store::CreationFilesystem;
            use tos_command::temporal_claim_intent::{
                ClaimIntentPrepareBudget, plan_claim_intent, prepare_claim_intent_from_captures,
            };
            use tos_foundation::emit_value_preserved_json;
            use tos_validation::executor::ExactWorkerIdentity;
            use tos_validation::source_cut::{
                CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor,
            };
            let mut oracle_raw = Vec::new();
            fs::File::open(repository.join(
                "tests/conformance/foundation-wave-v1/oracles/temporal-acceptance-vectors.json",
            ))
            .unwrap()
            .take(262_145)
            .read_to_end(&mut oracle_raw)
            .unwrap();
            assert!(oracle_raw.len() <= 262_144);
            assert_eq!(
                Digest256::of_bytes(&oracle_raw).to_hex(),
                "fda08fd958914a3605afab12e7b78012a3f5534ecfdffe2ab31cb2cb3a26eecd"
            );
            let oracle: Value = serde_json::from_slice(&oracle_raw).unwrap();
            let original = oracle["vectors"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == "history_revision")
                .unwrap();
            let intent = serde_json::json!({"schema_version":"tos_temporal_claim_intent_v1",
                "operation":original["operation"],"source_profile":original["source_profile"],
                "input":original["input"],"context":original["context"]});
            // This alias table is fixture evidence, not a mapper resolver or
            // persisted chronology. r1 points to the actual retained v1 packet.
            assert_eq!(
                intent["input"]["prior_chronology_members"],
                serde_json::json!(["event-A@r1"])
            );
            assert_eq!(
                frozen.member_claim_revisions,
                vec![exact_ref(&v1_owner_ref)]
            );
            assert_eq!(frozen.selection.source_snapshot.revision, current);
            let configuration_raw = fs::read(&revision_path).unwrap();
            let configuration_sha =
                Digest256::of_bytes(&source_bytes(&source_value(&revision_owner)));
            let bindings = serde_json::json!({"schema_version":"tos_temporal_claim_intent_binding_v1",
                "target":{"alias":"event-A","revision_alias":"r1","next_revision_alias":"r2",
                    "source":v1_owner_ref,"source_path":source_path,"source_revision":current.0.to_prefixed(),
                    "configuration_digest":configuration_sha.to_prefixed()},
                "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
                "authored_reason":"Explicit synthetic history alias and complete existing fixture Gregorian object supplement; no persisted chronology or historical-source admission.",
                "mapping_profile":"fixture:explicit-historical-date-object-supplement-v1",
                "payload":date_object("1946"),"identities":{},"sources":{},"trs":{}});
            let budget = ClaimIntentPrepareBudget {
                max_extra_owned_bytes: 134_217_728,
                max_extra_json_visits: 1_000_000,
                max_preview_bytes: 4_194_304,
                max_output_bytes: 1_048_576,
            };
            let intent_value = source_value(&intent);
            let plan = plan_claim_intent(
                &intent_value,
                &source_value(&bindings),
                &source_value(&v1_record),
                codec_budget,
                budget,
            )
            .unwrap();
            assert!(!plan.owner_checked());
            assert_eq!(plan.basis_status(), "SourceSuppliedUnverified");
            let original_cut = open_cut(&store, base, deadline, &cancellation);
            let current_cut = open_cut(&store, current, deadline, &cancellation);
            let filesystem = CreationFilesystem::select_isolated(
                &isolated,
                &revision_path,
                deadline,
                &cancellation,
            )
            .unwrap();
            let sentinel = source_bytes(&source_value(&serde_json::json!({
                "schema_version":"tos_local_source_command_v1","operation":"describe"})));
            let selected_inputs = || {
                current_files
                    .iter()
                    .chain(files.iter().filter(|(name, _)| !name.starts_with("ToS/")))
            };
            assert!(selected_inputs().count() <= 2048);
            assert!(
                selected_inputs()
                    .try_fold(0usize, |n, (_, raw)| n.checked_add(raw.len()))
                    .unwrap()
                    <= 33_554_432
            );
            let mut direct_context = tos_command::source_command::CommandContext {
                base_revision: current,
                configuration_raw: configuration_raw.clone(),
                request_raw: sentinel.clone(),
                recorded_at: created["result"]["receipt"]["recorded_at"]
                    .as_str()
                    .unwrap()
                    .into(),
                effective_uid: u64::from(uid),
                files: selected_inputs()
                    .map(|(name, raw)| tos_command::source_command::SourceFile {
                        path: tos_foundation::RelativePath::parse(name).unwrap(),
                        raw: raw.clone(),
                    })
                    .collect(),
            };
            let control_path = isolated
                .path()
                .join("ToS/source-witnesses/.metadata-publication.json");
            let read_control = || match fs::File::open(&control_path) {
                Ok(file) => {
                    let mut raw = Vec::new();
                    file.take(8193).read_to_end(&mut raw).unwrap();
                    assert!(raw.len() <= 8192);
                    Some(raw)
                }
                Err(e) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
                    None
                }
            };
            let control_before = read_control();
            let mut worker_budget = ExecutorBudget::laboratory();
            worker_budget.execution_wall = deadline
                .saturating_duration_since(Instant::now())
                .min(worker_budget.execution_wall);
            assert!(!worker_budget.execution_wall.is_zero());
            let mut direct_worker = CutWorkerSchemaExecutor::from_cut(
                &current_cut,
                FormatProfile::LegacyPythonObserved20260923,
                ExactWorkerIdentity {
                    absolute_path: worker_image.clone(),
                    sha256: Digest256::from_prefixed(
                        invocation["schema_worker"]["sha256"].as_str().unwrap(),
                    )
                    .unwrap(),
                },
                worker_budget,
                CutWorkerLimits {
                    max_receipts: 128,
                    max_receipt_bytes: 262_144,
                },
                deadline,
                &cancellation,
            )
            .unwrap();
            let mut generic = prepare_claim_intent_from_captures(
                &filesystem,
                &mut direct_context,
                &original_cut,
                &current_cut,
                &software,
                &components,
                &mut direct_worker,
                plan,
                deadline,
                &cancellation,
            )
            .unwrap();
            assert_eq!(direct_context.request_raw, sentinel);
            assert_eq!(direct_context.configuration_raw, configuration_raw);
            assert!(generic.owner_checked());
            let packet = generic.prepared_packet().unwrap();
            let packet: Value = serde_json::from_slice(&source_bytes(&packet)).unwrap();
            assert_eq!(
                packet["schema_version"],
                "tos_temporal_claim_intent_prepared_v1"
            );
            assert_eq!(packet["status"], "PreparedNotCommitted");
            let template = generic
                .commit_request_template("synthetic:whole-native-claim-cli-revision")
                .unwrap();
            let template: Value = serde_json::from_slice(&source_bytes(&template)).unwrap();
            assert_eq!(
                template["schema_version"],
                "tos_temporal_claim_intent_commit_template_v1"
            );
            assert_eq!(template["status"], "CommitTemplateNotCommitted");
            for envelope in [&packet, &template] {
                assert_eq!(envelope["owner_checked"], true);
                assert_eq!(envelope["basis_status"], "SourceSuppliedUnverified");
                assert_eq!(envelope["profile_status"], "SourceSuppliedAttributionOnly");
                assert_eq!(envelope["grants_admission"], false);
            }
            let request = &template["request"];
            assert_eq!(request["operation"], "claim.revise");
            assert_eq!(
                request["fields"],
                serde_json::json!({"object":date_object("1946")})
            );
            assert_eq!(request["forms"], bindings["forms"]);
            for (dst, src) in [
                ("expected_configuration", "owner_configuration"),
                ("expected_source", "source"),
                ("expected_revision", "revision"),
                ("expected_dependencies", "expected_dependencies"),
                ("expected_inputs", "source_bindings"),
            ] {
                assert_eq!(
                    source_bytes(&source_value(&request[dst])),
                    source_bytes(&source_value(&packet["owner_preview"][src]))
                );
            }
            assert_eq!(
                source_bytes(&source_value(&request["expected_source"])),
                source_bytes(&source_value(&v1_owner_ref))
            );
            let reason = request["reason"].as_str().unwrap();
            assert!(reason.chars().count() <= 4096);
            let reason: Value = serde_json::from_str(reason).unwrap();
            assert_eq!(reason["preserved_intent"], intent);
            assert_eq!(
                template["intent_preserved_text_sha256"],
                Digest256::of_bytes(
                    &emit_value_preserved_json(&intent_value, JsonLimits::default()).unwrap()
                )
                .to_prefixed()
            );
            assert_eq!(
                template["intent_canonical_sha256"],
                Digest256::of_bytes(&source_bytes(&intent_value)).to_prefixed()
            );
            assert_eq!(
                template["emitted_owner_request_sha256"],
                Digest256::of_bytes(&source_bytes(&source_value(request))).to_prefixed()
            );
            assert_eq!(
                template["configuration_raw_sha256"],
                Digest256::of_bytes(&configuration_raw).to_prefixed()
            );
            assert_eq!(
                template["configuration_canonical_sha256"],
                configuration_sha.to_prefixed()
            );
            direct_worker.finish(deadline, &cancellation).unwrap();
            assert_eq!(read_control(), control_before);
            for (name, raw) in &original_files {
                assert_eq!(
                    &fs::read(isolated.path().join(home).join(name)).unwrap(),
                    raw
                );
            }
            let mut unchanged = authored_text_files(isolated.path());
            unchanged.remove("ToS/source-witnesses/.historical-create.writer.lock");
            assert_eq!(unchanged, current_files);
            // Honest normalized owner_preview, NOT a fabricated native result
            // envelope. Consume exact template.request, without field rewrites.
            (
                packet["owner_preview"].clone(),
                request.clone(),
                Some(template),
            )
        } else {
            // ONE generic Unknown-END prepare replaces the former native prepare.
            // The existing commit/replay below consumes the exact template request.
            use std::io::Read;
            use tos_command::source_creation_store::CreationFilesystem;
            use tos_command::temporal_claim_intent::prepare_claim_intent_from_captures;
            use tos_foundation::emit_value_preserved_json;
            use tos_validation::executor::ExactWorkerIdentity;
            use tos_validation::source_cut::{
                CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor,
            };
            let (plan, intent, bindings) = unknown_commit.unwrap();
            let intent_value = source_value(&intent);
            let configuration_raw = fs::read(&revision_path).unwrap();
            let configuration_sha =
                Digest256::of_bytes(&source_bytes(&source_value(&revision_owner)));
            let original_cut = open_cut(&store, base, deadline, &cancellation);
            let current_cut = open_cut(&store, current, deadline, &cancellation);
            let filesystem = CreationFilesystem::select_isolated(
                &isolated,
                &revision_path,
                deadline,
                &cancellation,
            )
            .unwrap();
            let sentinel = source_bytes(&source_value(&serde_json::json!({
                "schema_version":"tos_local_source_command_v1","operation":"describe"})));
            let selected_inputs = || {
                current_files
                    .iter()
                    .chain(files.iter().filter(|(name, _)| !name.starts_with("ToS/")))
            };
            assert!(selected_inputs().count() <= 2048);
            assert!(
                selected_inputs()
                    .try_fold(0usize, |n, (_, raw)| n.checked_add(raw.len()))
                    .unwrap()
                    <= 33_554_432
            );
            let mut direct_context = tos_command::source_command::CommandContext {
                base_revision: current,
                configuration_raw: configuration_raw.clone(),
                request_raw: sentinel.clone(),
                recorded_at: created["result"]["receipt"]["recorded_at"]
                    .as_str()
                    .unwrap()
                    .into(),
                effective_uid: u64::from(uid),
                files: selected_inputs()
                    .map(|(name, raw)| tos_command::source_command::SourceFile {
                        path: tos_foundation::RelativePath::parse(name).unwrap(),
                        raw: raw.clone(),
                    })
                    .collect(),
            };
            assert!(
                direct_context.files.len() <= 2048
                    && direct_context
                        .files
                        .iter()
                        .try_fold(0usize, |n, f| n.checked_add(f.raw.len()))
                        .unwrap()
                        <= 33_554_432
            );
            let control_path = isolated
                .path()
                .join("ToS/source-witnesses/.metadata-publication.json");
            let read_control = || match fs::File::open(&control_path) {
                Ok(file) => {
                    let mut raw = Vec::new();
                    file.take(8193).read_to_end(&mut raw).unwrap();
                    assert!(raw.len() <= 8192);
                    Some(raw)
                }
                Err(e) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
                    None
                }
            };
            let control_before = read_control();
            let mut worker_budget = ExecutorBudget::laboratory();
            worker_budget.execution_wall = deadline
                .saturating_duration_since(Instant::now())
                .min(worker_budget.execution_wall);
            assert!(!worker_budget.execution_wall.is_zero());
            let mut direct_worker = CutWorkerSchemaExecutor::from_cut(
                &current_cut,
                FormatProfile::LegacyPythonObserved20260923,
                ExactWorkerIdentity {
                    absolute_path: worker_image.clone(),
                    sha256: Digest256::from_prefixed(
                        invocation["schema_worker"]["sha256"].as_str().unwrap(),
                    )
                    .unwrap(),
                },
                worker_budget,
                CutWorkerLimits {
                    max_receipts: 128,
                    max_receipt_bytes: 262_144,
                },
                deadline,
                &cancellation,
            )
            .unwrap();
            let mut generic = prepare_claim_intent_from_captures(
                &filesystem,
                &mut direct_context,
                &original_cut,
                &current_cut,
                &software,
                &components,
                &mut direct_worker,
                plan,
                deadline,
                &cancellation,
            )
            .unwrap();
            assert_eq!(direct_context.request_raw, sentinel);
            assert_eq!(direct_context.configuration_raw, configuration_raw);
            assert!(generic.owner_checked());
            let packet = generic.prepared_packet().unwrap();
            let packet: Value = serde_json::from_slice(&source_bytes(&packet)).unwrap();
            assert_eq!(
                packet["schema_version"],
                "tos_temporal_claim_intent_prepared_v1"
            );
            assert_eq!(packet["status"], "PreparedNotCommitted");
            let template = generic
                .commit_request_template("synthetic:whole-native-claim-cli-revision")
                .unwrap();
            let template: Value = serde_json::from_slice(&source_bytes(&template)).unwrap();
            assert_eq!(
                template["schema_version"],
                "tos_temporal_claim_intent_commit_template_v1"
            );
            assert_eq!(template["status"], "CommitTemplateNotCommitted");
            for envelope in [&packet, &template] {
                assert_eq!(envelope["owner_checked"], true);
                assert_eq!(envelope["basis_status"], "SourceSuppliedUnverified");
                assert_eq!(envelope["profile_status"], "SourceSuppliedAttributionOnly");
                assert_eq!(envelope["grants_admission"], false);
            }
            let request = &template["request"];
            assert_eq!(request["operation"], "claim.revise");
            assert_eq!(request["fields"].as_object().unwrap().len(), 1);
            assert!(request["fields"].get("qualifiers").is_some());
            // The owner merges this one-key qualifier PATCH into the record.
            // Only the full carrier appears here; sibling/statement stay in v1.
            assert!(v1_raw.len() <= 16_384);
            let mut expected_carrier = v1_record["qualifiers"]["temporal_claim_carrier"].clone();
            expected_carrier["validity"] = bindings["payload"]["validity"].clone();
            assert_eq!(
                request["fields"]["qualifiers"].as_object().unwrap().len(),
                1
            );
            assert_eq!(
                request["fields"]["qualifiers"]["temporal_claim_carrier"],
                expected_carrier
            );
            assert!(request["fields"]["qualifiers"].get("statement").is_none());
            assert!(
                request["fields"]["qualifiers"]
                    .get("carrier_fixture_sibling")
                    .is_none()
            );
            assert_eq!(request["forms"], bindings["forms"]);
            for (dst, src) in [
                ("expected_configuration", "owner_configuration"),
                ("expected_source", "source"),
                ("expected_revision", "revision"),
                ("expected_dependencies", "expected_dependencies"),
                ("expected_inputs", "source_bindings"),
            ] {
                assert_eq!(
                    source_bytes(&source_value(&request[dst])),
                    source_bytes(&source_value(&packet["owner_preview"][src]))
                );
            }
            assert_eq!(
                source_bytes(&source_value(&request["expected_source"])),
                source_bytes(&source_value(&v1_owner_ref))
            );
            let reason = request["reason"].as_str().unwrap();
            assert!(reason.chars().count() <= 4096);
            let reason: Value = serde_json::from_str(reason).unwrap();
            assert_eq!(reason["preserved_intent"], intent);
            assert_eq!(
                template["intent_preserved_text_sha256"],
                Digest256::of_bytes(
                    &emit_value_preserved_json(&intent_value, JsonLimits::default()).unwrap()
                )
                .to_prefixed()
            );
            assert_eq!(
                template["intent_canonical_sha256"],
                Digest256::of_bytes(&source_bytes(&intent_value)).to_prefixed()
            );
            assert_eq!(
                template["emitted_owner_request_sha256"],
                Digest256::of_bytes(&source_bytes(&source_value(request))).to_prefixed()
            );
            assert_eq!(
                template["configuration_raw_sha256"],
                Digest256::of_bytes(&configuration_raw).to_prefixed()
            );
            assert_eq!(
                template["configuration_canonical_sha256"],
                configuration_sha.to_prefixed()
            );
            direct_worker.finish(deadline, &cancellation).unwrap();
            drop(generic);
            drop(direct_worker);
            assert_eq!(read_control(), control_before);
            for (name, raw) in &original_files {
                assert_eq!(
                    &fs::read(isolated.path().join(home).join(name)).unwrap(),
                    raw
                );
            }
            let mut unchanged = authored_text_files(isolated.path());
            unchanged.remove("ToS/source-witnesses/.historical-create.writer.lock");
            assert_eq!(unchanged, current_files);
            assert_eq!(direct_context.request_raw, sentinel);
            drop(direct_context);
            // Honest normalized owner_preview, NOT a fabricated native result
            // envelope. Consume exact template.request, without field rewrites.
            (
                packet["owner_preview"].clone(),
                request.clone(),
                Some(template),
            )
        };
        let revised = alignment_native_cli(
            &repository,
            &revision_path,
            &invocation_path,
            &revision,
            deadline,
        );
        assert_eq!(revised["result"]["replayed"], false);
        assert_eq!(revised["result"]["source"]["version"], 2);
        assert_eq!(revised["result"]["receipt"]["grants_admission"], false);
        if let Some(template) = &history_template {
            let receipt = &revised["result"]["receipt"];
            assert_eq!(
                receipt["request_digest"],
                template["emitted_owner_request_sha256"]
            );
            assert_eq!(
                source_bytes(&source_value(&receipt["request"])),
                source_bytes(&source_value(&revision))
            );
            assert_eq!(receipt["reason"], revision["reason"]);
            assert_eq!(
                source_bytes(&source_value(&receipt["previous_source"])),
                source_bytes(&source_value(&v1_owner_ref))
            );
            // Explicit symbolic aliases bind actual returned refs, not invented
            // records; the original chronology token remains a pure fixture.
            if date_variant {
                let aliases = serde_json::json!({"event-A@r1":v1_owner_ref,"event-A@r2":revised["result"]["source"]});
                assert_eq!(
                    exact_ref(&aliases["event-A@r1"]),
                    frozen.member_claim_revisions[0]
                );
                assert_eq!(aliases["event-A@r2"]["id"], aliases["event-A@r1"]["id"]);
                assert_eq!(aliases["event-A@r1"]["version"], 1);
                assert_eq!(aliases["event-A@r2"]["version"], 2);
            } else {
                // The original Unknown-END input has only state-B/r1, no r2 alias.
                let aliases = serde_json::json!({"state-B@r1":v1_owner_ref});
                assert_eq!(
                    exact_ref(&aliases["state-B@r1"]),
                    frozen.member_claim_revisions[0]
                );
                assert_eq!(aliases["state-B@r1"]["version"], 1);
                assert_eq!(
                    revised["result"]["source"]["id"],
                    aliases["state-B@r1"]["id"]
                );
                assert_eq!(revised["result"]["source"]["version"], 2);
            }
        }
        let mut successor_files = authored_text_files(isolated.path());
        successor_files.remove("ToS/source-witnesses/.historical-create.writer.lock");
        let revised_cut = successor(&successor_files, &store, current);
        invocation["source_revision"] = serde_json::json!(revised_cut.0.to_prefixed());
        write_invocation(&invocation);
        let retry = alignment_native_cli(
            &repository,
            &revision_path,
            &invocation_path,
            &revision,
            deadline,
        );
        assert_eq!(retry["result"]["replayed"], true);
        assert_eq!(retry["result"]["receipt"], revised["result"]["receipt"]);
        // Actual cold public owner reads against the newly written v2 cut, then
        // independently inspect the exact v1 member frozen before the real revision.
        let describe = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"});
        let described = alignment_native_cli(
            &repository,
            &revision_path,
            &invocation_path,
            &describe,
            deadline,
        );
        let v2_raw = fs::read(isolated.path().join(&source_path)).unwrap();
        let v2_record: Value = serde_json::from_slice(&v2_raw).unwrap();
        assert_eq!(v2_record["claim_id"], v1_record["claim_id"]);
        assert_eq!(v2_record["claim_version"], 2);
        if date_variant {
            assert_eq!(v1_record["object"], date_object("1945"));
            assert_eq!(v2_record["object"], date_object("1946"));
            // Locate the actual published set by its schema/subject, then bind
            // its selected form to the actual revision receipt. No filename
            // convention supplies the identity or pretends initial creation.
            let published_sets = fs::read_dir(isolated.path().join(home))
                .unwrap()
                .filter_map(|entry| {
                    let entry = entry.unwrap();
                    if !entry.file_type().unwrap().is_file() {
                        return None;
                    }
                    let raw = fs::read(entry.path()).unwrap();
                    let set: Value = serde_json::from_slice(&raw).ok()?;
                    (set["schema_version"] == "tos_human_form_set_v1").then_some(set)
                })
                .collect::<Vec<_>>();
            assert_eq!(published_sets.len(), 1);
            let set = &published_sets[0];
            assert_eq!(set["subject"], source_ref(&v2_record));
            assert!(set["prior_forms"].as_array().unwrap().is_empty());
            let forms = set["forms"].as_array().unwrap();
            assert_eq!(forms.len(), 1);
            let form = &forms[0];
            assert_eq!(form["subject"], source_ref(&v2_record));
            assert_eq!(form["role"], "statement");
            assert_eq!(form["form_id"], revision_owner["allowed_form_ids"][0]);
            assert_eq!(form["form_version"], 1);
            let receipt_forms = revised["result"]["receipt"]["forms"].as_array().unwrap();
            assert_eq!(receipt_forms.len(), 1);
            assert_eq!(
                receipt_forms[0],
                serde_json::json!({"id":form["form_id"],
                "version":form["form_version"],"digest":Digest256::of_bytes(&source_bytes(&source_value(form))).to_prefixed()})
            );
            assert_eq!(
                owner_preview["prepared_forms"],
                revised["result"]["receipt"]["forms"]
            );
            let views = described["result"]["materializations"].as_array().unwrap();
            assert_eq!(views.len(), 1);
            assert_eq!(views[0]["state"], "ready");

            assert_eq!(v2_record["qualifiers"], v1_record["qualifiers"]);
            for (key, value) in v1_record.as_object().unwrap() {
                if key != "object" && key != "claim_version" {
                    assert_eq!(&v2_record[key], value);
                }
            }
        } else {
            assert_eq!(
                v2_record["qualifiers"]["statement"],
                v1_record["qualifiers"]["statement"]
            );
        }
        assert_eq!(described["result"]["source"], revised["result"]["source"]);
        assert_eq!(described["result"]["source"], source_ref(&v2_record));
        assert_ne!(Digest256::of_bytes(&v2_raw), Digest256::of_bytes(&v1_raw));
        assert_eq!(described["grants_admission"], false);
        let stored_interval = if date_variant {
            assert!(
                decode_claim_carrier(&source_value(&v2_record["qualifiers"]), codec_budget)
                    .unwrap()
                    .is_none()
            );
            None
        } else {
            let stored_unknown =
                decode_claim_carrier(&source_value(&v2_record["qualifiers"]), codec_budget)
                    .unwrap()
                    .unwrap();
            let expected = decode_claim_carrier(
                &source_value(&revision["fields"]["qualifiers"]),
                codec_budget,
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                source_bytes(stored_unknown.original_packet()),
                source_bytes(expected.original_packet())
            );
            assert_eq!(
                stored_unknown.authority(),
                CarrierAuthority::SupplementalSourceAssertions
            );
            assert_eq!(
                stored_unknown.legacy_object_assessment(),
                LegacyObjectAssessment::NotExamined
            );
            let Some(CarrierValidity::Interval {
                beginning,
                end,
                basis,
            }) = stored_unknown.validity()
            else {
                panic!(
                    "actual v2 must retain the explicit beginning and Unknown end, not wholeUnknown or RightCensored"
                )
            };
            let tos_foundation::temporal::Position::Calendar {
                lexical,
                resolution,
                trs,
                era,
            } = beginning
            else {
                panic!("original lexical beginning must stay a CalendarPosition")
            };
            assert_eq!(lexical, "2020-01-01");
            assert_eq!(resolution, &tos_foundation::temporal::Resolution::Unknown);
            assert!(trs.is_none() && era.is_none());
            assert!(matches!(end, tos_foundation::temporal::Boundary::Unknown));
            assert!(
                stored_unknown
                    .relative_relations()
                    .is_some_and(|rows| rows.is_empty())
            );
            assert!(stored_unknown.roles().is_none() && stored_unknown.recurrence().is_none());
            assert_eq!(basis.source, exact_ref(&v1_owner_ref));
            assert_eq!(
                basis
                    .raw_evidence
                    .object_get("actual_stream_sha256")
                    .unwrap()
                    .as_str(),
                Some(Digest256::of_bytes(&v1_raw).to_prefixed().as_str())
            );
            assert_eq!(
                basis
                    .raw_evidence
                    .object_get("actual_source_cut")
                    .unwrap()
                    .as_str(),
                Some(current.0.to_prefixed().as_str())
            );
            assert_eq!(
                basis
                    .raw_evidence
                    .object_get("actual_receipt_command")
                    .unwrap()
                    .as_str(),
                created["result"]["receipt"]["command_id"].as_str()
            );
            assert_eq!(
                basis
                    .raw_evidence
                    .object_get("synthetic_assertion_raw")
                    .unwrap()
                    .as_str(),
                Some(raw_lexical)
            );
            assert_eq!(
                v2_record["qualifiers"]["temporal_claim_carrier"]["future_packet"]["large_integer"]
                    .as_u64(),
                Some(9007199254740993)
            );
            assert!(
                v2_record["qualifiers"]["temporal_claim_carrier"]["future_packet"]["future"]
                    .is_null()
            );
            assert_eq!(
                v2_record["qualifiers"]["temporal_claim_carrier"]["future_packet"]["raw_decimal"],
                raw_lexical
            );
            assert_eq!(v2_record["object"], v1_record["object"]);
            assert_eq!(
                v2_record["qualifiers"]["carrier_fixture_sibling"],
                opaque_sibling
            );
            assert_eq!(
                v2_record["qualifiers"]["statement"],
                v1_record["qualifiers"]["statement"]
            );
            for (key, value) in v1_record["qualifiers"].as_object().unwrap() {
                if key != "temporal_claim_carrier" {
                    assert_eq!(&v2_record["qualifiers"][key], value);
                }
            }
            for (key, value) in v1_record["qualifiers"]["temporal_claim_carrier"]
                .as_object()
                .unwrap()
            {
                assert_eq!(
                    &v2_record["qualifiers"]["temporal_claim_carrier"][key],
                    value
                );
            }

            Some(stored_unknown)
        };
        let frozen_member = &frozen.member_claim_revisions[0];
        let frozen_owner_ref = serde_json::json!({
            "id":frozen_member.id.as_str(), "version":frozen_member.version.get(),
            "digest":frozen_member.digest.to_prefixed()
        });
        assert_eq!(frozen_owner_ref, v1_owner_ref);
        let inspect = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-version","source":frozen_owner_ref});
        let inspected = alignment_native_cli(
            &repository,
            &revision_path,
            &invocation_path,
            &inspect,
            deadline,
        );
        assert_eq!(inspected["result"]["record"], v1_record);
        assert_eq!(inspected["result"]["inspected_source"], v1_owner_ref);
        assert_eq!(inspected["result"]["source"], described["result"]["source"]);
        assert_eq!(inspected["grants_admission"], false);
        let raw_binding = &inspected["result"]["files"]["source-claims.jsonl"];
        let archive_path = raw_binding["archive_path"].as_str().unwrap();
        assert!(archive_path.starts_with(
            &format!("{}/", revised["result"]["receipt"]["archive_path"].as_str().unwrap())
        ));
        let archived_raw = fs::read(isolated.path().join(archive_path)).unwrap();
        assert_eq!(archived_raw, v1_raw);
        assert_eq!(
            raw_binding["sha256"],
            Digest256::of_bytes(&archived_raw).to_prefixed()
        );
        assert_eq!(raw_binding["bytes"], serde_json::json!(archived_raw.len()));
        assert_eq!(
            fs::read(isolated.path().join(&source_path)).unwrap(),
            v2_raw
        );

        let ContextResolution::Resolved(still_frozen) =
            resolve_query_context(&snapshot, &QueryContext::default(), Some(&frozen)).unwrap()
        else {
            panic!("the original selected cut must retain its frozen v1 reading")
        };
        assert_eq!(still_frozen.selection, frozen.selection);
        assert_eq!(
            still_frozen.chronology_version,
            Some(frozen.reference.clone())
        );
        assert_eq!(
            frozen.member_claim_revisions[0],
            exact_ref(&inspected["result"]["inspected_source"])
        );
        let revised_snapshot = SourceSnapshot {
            reference: SourceSnapshotRef {
                id: snapshot.reference.id.clone(),
                revision: revised_cut,
            },
            current_cutoff: Cutoff {
                value: source_value(&revised["result"]["receipt"]["recorded_at"]),
                trs: None,
            },
        };
        assert_eq!(
            resolve_query_context(&revised_snapshot, &QueryContext::default(), Some(&frozen))
                .unwrap(),
            ContextResolution::ContextMismatch(vec![MismatchDimension::SourceRevisionFence])
        );
        let ContextResolution::Resolved(new_current) =
            resolve_query_context(&revised_snapshot, &current_context, None).unwrap()
        else {
            panic!("explicit Current must resolve solely against the supplied actual v2 cut")
        };
        assert_eq!(new_current.selection.source_snapshot.revision, revised_cut);

        // Fresh process creation replay still reconstructs v1 after the real v2
        // publication, using its retained predecessor archive and original cut.
        invocation["owner_config"] = serde_json::json!(owner);
        write_invocation(&invocation);
        let old = alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
        assert_eq!(old["result"]["replayed"], true);
        assert_eq!(old["result"]["receipt"], created["result"]["receipt"]);
        if !date_variant {
            // A single extra successor fits the unchanged base->v1->v2->v3 cap of four.
            let v2_owner_ref = revised["result"]["source"].clone();
            let basis_v2 = actual_basis(
                &v2_owner_ref,
                &v2_raw,
                &revised["result"]["receipt"],
                revised_cut,
            );
            let roles = [
                "WorldOccurrence",
                "ActualInterval",
                "Beginning",
                "End",
                "SourceCreation",
                "SourcePublication",
                "AttributedKnowledge",
                "SystemTransaction",
            ]
            .iter()
            .map(|role| {
                let mut assertion =
                    serde_json::json!({"synthetic":true,"tag":role,"raw":raw_lexical});
                if *role == "SystemTransaction" {
                    assertion["actual_owner_recorded_at"] =
                        revised["result"]["receipt"]["recorded_at"].clone();
                }
                serde_json::json!({"role":role,"assertion":assertion,"basis":basis_v2})
            })
            .collect::<Vec<_>>();
            let censored_packet = serde_json::json!({"schema_version":"tos_temporal_claim_carrier_v1",
        "relative_relations":[{"subject_a":created["result"]["receipt"]["command_id"],
            "subject_b":revised["result"]["receipt"]["command_id"],"relation":"before",
            "source":codec_reference(&v2_owner_ref),"epistemic_status":"explicit-synthetic-source-assertion",
            "asserted_by":revision_owner["principal_id"],"wording_ref":null,"future_relation":null}],
        "validity":{"type":"RightCensored","beginning":null,
            "observed_through":{"type":"CoordinatePosition","value":v2_owner_ref["version"],
                "unit":"fixture-owner-revision","origin":v2_owner_ref["id"],
                "trs":{"id":"fixture:explicit-owner-revision-order","version":"fixture-v1"}},
            "cutoff_basis":basis_v2,"future_validity":null},
        "roles":roles,
        "recurrence":{"profile_ref":"fixture:opaque-pattern-v1",
            "pattern":{"raw":"R/../explicit synthetic recurrence; never expanded",
                "raw_decimal":raw_lexical,"future":null},"basis":basis_v2},
        "future_packet":opaque_sibling});
            let combined = decode_claim_carrier(
                &source_value(&serde_json::json!({
        "temporal_claim_carrier":censored_packet})),
                codec_budget,
            )
            .unwrap()
            .unwrap();
            let encoded = encode_claim_carrier(
                &source_value(&v2_record["qualifiers"]),
                &combined,
                codec_budget,
            )
            .unwrap();
            let encoded: Value = serde_json::from_slice(&source_bytes(&encoded)).unwrap();
            invocation["owner_config"] = serde_json::json!(revision_path);
            write_invocation(&invocation);
            let carrier_prepare = serde_json::json!({"schema_version":"tos_local_source_command_v1",
        "operation":"prepare-revise","fields":{"qualifiers":{"temporal_claim_carrier":encoded["temporal_claim_carrier"]}},
        "forms":[{"form_id":revision_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
        "reason":"Persist explicit synthetic supplemental temporal assertions using real preceding Claim evidence; legacy object unchanged."});
            let preview = alignment_native_cli(
                &repository,
                &revision_path,
                &invocation_path,
                &carrier_prepare,
                deadline,
            );
            let mut carrier_request = carrier_prepare;
            carrier_request["operation"] = serde_json::json!("claim.revise");
            carrier_request["command_id"] =
                serde_json::json!("synthetic:whole-native-claim-carrier-v3");
            for (field, result_field) in [
                ("expected_configuration", "owner_configuration"),
                ("expected_source", "source"),
                ("expected_revision", "revision"),
                ("expected_dependencies", "expected_dependencies"),
                ("expected_inputs", "source_bindings"),
            ] {
                carrier_request[field] = preview["result"][result_field].clone();
            }
            let written = alignment_native_cli(
                &repository,
                &revision_path,
                &invocation_path,
                &carrier_request,
                deadline,
            );
            assert_eq!(written["result"]["replayed"], false);
            assert_eq!(written["result"]["source"]["version"], 3);
            assert_eq!(
                written["result"]["receipt"]["previous_source"],
                v2_owner_ref
            );
            assert_eq!(written["result"]["receipt"]["grants_admission"], false);
            let v3_raw = fs::read(isolated.path().join(&source_path)).unwrap();
            let v3_record: Value = serde_json::from_slice(&v3_raw).unwrap();
            assert_eq!(source_ref(&v3_record), written["result"]["source"]);
            let stored_combined =
                decode_claim_carrier(&source_value(&v3_record["qualifiers"]), codec_budget)
                    .unwrap()
                    .unwrap();
            assert_eq!(
                source_bytes(stored_combined.original_packet()),
                source_bytes(combined.original_packet())
            );
            assert_eq!(
                source_bytes(stored_combined.original_packet()),
                source_bytes(&source_value(&censored_packet))
            );
            assert_eq!(
                stored_combined.authority(),
                CarrierAuthority::SupplementalSourceAssertions
            );
            assert_eq!(
                stored_combined.legacy_object_assessment(),
                LegacyObjectAssessment::NotExamined
            );
            let Some(CarrierValidity::RightCensored {
                beginning,
                observed_through,
                cutoff_basis,
            }) = stored_combined.validity()
            else {
                panic!(
                    "actual v3 bytes must retain explicit RightCensored, not Unknown or unbounded"
                )
            };
            assert!(beginning.is_none());
            assert_eq!(cutoff_basis.source, exact_ref(&v2_owner_ref));
            assert!(matches!(
                observed_through,
                tos_foundation::temporal::Position::Coordinate { value: 2, .. }
            ));
            let relative = &stored_combined.relative_relations().unwrap()[0];
            assert_eq!(relative.source, exact_ref(&v2_owner_ref));
            assert_eq!(relative.relation, Relation::Before);
            assert_eq!(
                relative.subject_a.as_str(),
                created["result"]["receipt"]["command_id"].as_str().unwrap()
            );
            assert_eq!(
                relative.subject_b.as_str(),
                revised["result"]["receipt"]["command_id"].as_str().unwrap()
            );
            assert_eq!(
                stored_combined
                    .roles()
                    .unwrap()
                    .iter()
                    .map(|role| role.role)
                    .collect::<Vec<_>>(),
                vec![
                    TemporalRole::WorldOccurrence,
                    TemporalRole::ActualInterval,
                    TemporalRole::Beginning,
                    TemporalRole::End,
                    TemporalRole::SourceCreation,
                    TemporalRole::SourcePublication,
                    TemporalRole::AttributedKnowledge,
                    TemporalRole::SystemTransaction
                ]
            );
            assert_eq!(
                source_bytes(&stored_combined.recurrence().unwrap().pattern),
                source_bytes(&source_value(&censored_packet["recurrence"]["pattern"]))
            );
            assert!(censored_packet["recurrence"].get("occurrences").is_none());
            assert_eq!(
                v3_record["qualifiers"]["carrier_fixture_sibling"],
                opaque_sibling
            );
            assert_eq!(
                v3_record["qualifiers"]["temporal_claim_carrier"]["future_packet"]["large_integer"]
                    .as_u64(),
                Some(9007199254740993)
            );
            assert!(
                v3_record["qualifiers"]["temporal_claim_carrier"]["future_packet"]["future"]
                    .is_null()
            );
            assert_eq!(
                v3_record["qualifiers"]["temporal_claim_carrier"]["future_packet"]["raw_decimal"],
                raw_lexical
            );
            for (key, value) in v2_record["qualifiers"].as_object().unwrap() {
                if key != "temporal_claim_carrier" {
                    assert_eq!(&v3_record["qualifiers"][key], value);
                }
            }
            for key in [
                "claim_id",
                "object",
                "subject_ref",
                "predicate",
                "evidence_refs",
                "epistemic_status",
                "review_status",
            ] {
                assert_eq!(v3_record[key], v2_record[key]);
            }
            // Published canonical bytes and retained raw evidence strings have distinct checks.
            assert_eq!(
                v3_raw,
                [source_bytes(&source_value(&v3_record)), b"\n".to_vec()].concat()
            );
            assert_ne!(Digest256::of_bytes(&v3_raw), Digest256::of_bytes(&v2_raw));
            let mut carrier_files = authored_text_files(isolated.path());
            carrier_files.remove("ToS/source-witnesses/.historical-create.writer.lock");
            let carrier_cut = successor(&carrier_files, &store, revised_cut);
            invocation["source_revision"] = serde_json::json!(carrier_cut.0.to_prefixed());
            write_invocation(&invocation);
            let current_carrier = alignment_native_cli(
                &repository,
                &revision_path,
                &invocation_path,
                &describe,
                deadline,
            );
            assert_eq!(current_carrier["result"]["source"], source_ref(&v3_record));
            let previous = alignment_native_cli(
                &repository,
                &revision_path,
                &invocation_path,
                &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-version","source":v2_owner_ref}),
                deadline,
            );
            assert_eq!(previous["result"]["record"], v2_record);
            assert_eq!(previous["result"]["inspected_source"], v2_owner_ref);
            assert_eq!(previous["result"]["source"], source_ref(&v3_record));
            assert_eq!(previous["grants_admission"], false);
            let prior = decode_claim_carrier(
                &source_value(&previous["result"]["record"]["qualifiers"]),
                codec_budget,
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                source_bytes(prior.original_packet()),
                source_bytes(stored_interval.as_ref().unwrap().original_packet())
            );
            let Some(CarrierValidity::Interval {
                beginning,
                end,
                basis,
            }) = prior.validity()
            else {
                panic!("cold archived v2 must retain the full Interval")
            };
            let tos_foundation::temporal::Position::Calendar {
                lexical,
                resolution,
                trs,
                era,
            } = beginning
            else {
                panic!("cold archived beginning must retain the original lexical CalendarPosition")
            };
            assert_eq!(lexical, "2020-01-01");
            assert_eq!(resolution, &tos_foundation::temporal::Resolution::Unknown);
            assert!(trs.is_none() && era.is_none());
            assert!(matches!(end, tos_foundation::temporal::Boundary::Unknown));
            assert_eq!(basis.source, exact_ref(&v1_owner_ref));
            let binding = &previous["result"]["files"]["source-claims.jsonl"];
            let previous_raw = fs::read(
                isolated
                    .path()
                    .join(binding["archive_path"].as_str().unwrap()),
            )
            .unwrap();
            assert_eq!(previous_raw, v2_raw);
            assert_eq!(binding["bytes"].as_u64(), Some(previous_raw.len() as u64));
            assert_eq!(
                binding["sha256"],
                Digest256::of_bytes(&previous_raw).to_prefixed()
            );
            assert_eq!(
                fs::read(isolated.path().join(archive_path)).unwrap(),
                v1_raw
            );
            assert_eq!(
                fs::read(isolated.path().join(&source_path)).unwrap(),
                v3_raw
            );
            assert!(Instant::now() < deadline);
        }
        drop(software);
    }
}
